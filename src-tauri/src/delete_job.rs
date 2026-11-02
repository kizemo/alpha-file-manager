// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! 批量删除的后台作业。
//!
//! FORK-MODIFICATION 2026-10-22。
//!
//! 进度语义换掉了：
//!
//! - 旧：进度按**顶层路径数**推进。删一棵含 25 万个文件的目录树时，
//!   进度条在整个删除期间几乎不动（用户看到「4% 卡住 40 分钟」）。
//! - 新：先**预扫描**拿到条目总数与字节总数，删除过程中按**条目数**推进，
//!   字节数作辅助指标；进度事件按 100ms / 500 条目节流。
//!
//! 算法本体在 `delete_tree`（刻意不依赖 tauri，因此可以单测）。

use crate::delete_tree::{self, DeleteOutcome, DeleteProgress, ScanOutcome};
use crate::utils::minimize_delete_paths;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc::UnboundedSender;

/// 进度阶段：正在统计规模。
pub const DELETE_PHASE_SCANNING: &str = "scanning";

/// 进度阶段：正在删除。
pub const DELETE_PHASE_DELETING: &str = "deleting";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteJobProgressPayload {
    pub job_id: String,
    /// 条目口径的百分比（0-100）。没有分母时为 0 —— 界面此时应当显示
    /// 「不确定态」而不是一个假的百分比。
    pub percent: u32,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processed_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_count: Option<u64>,
    /// 当前阶段：`scanning` / `deleting`。
    pub phase: String,
    /// FORK-MODIFICATION 2026-11-01：扫描阶段是否已经数完。
    ///
    /// 界面判断「规模提示可以弹了」的依据。以前只能靠「阶段还是 scanning
    /// 且分母出现了」去猜，但扫描途中的进度事件也可能带分母 —— 那会让提示
    /// 提前弹出，而且文件数 / 目录数还是 0。后端显式给标志，界面就不必猜。
    pub scan_completed: bool,
    /// 已删除的条目数（文件 + 目录）。
    pub deleted_items: u64,
    /// 已释放的字节数。
    pub freed_bytes: u64,
    /// 预扫描得到的总字节。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
    /// 条目口径的百分比；没有分母时为 None。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_percent: Option<u32>,
    /// 字节口径的百分比；没有分母时为 None。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_percent: Option<u32>,
    /// 文件数。
    pub files: u64,
    /// 目录数。
    pub directories: u64,
    /// 当前正在处理的路径。
    pub current_path: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteJobFinishedPayload {
    pub job_id: String,
    pub success: bool,
    pub cancelled: bool,
    pub use_trash: bool,
    pub error: Option<String>,
    pub deleted_paths: Vec<String>,
    /// 真正删掉的条目数（文件 + 目录）。
    pub deleted_items: u64,
    /// 真正释放的字节数。
    pub freed_bytes: u64,
    /// 预扫描得到的条目总数。
    pub total_items: u64,
    /// 预扫描得到的文件数。
    pub files: u64,
    /// 预扫描得到的目录数。
    pub directories: u64,
    /// 预扫描得到的总字节。
    pub total_bytes: u64,
}

/// 作业内部流转的进度消息（还没拼上 job_id）。
struct ProgressMessage {
    phase: &'static str,
    /// 扫描阶段是否已经数完（只有「统计完的那一条」为 true）。
    scan_completed: bool,
    detail: String,
    deleted_items: u64,
    freed_bytes: u64,
    total_items: Option<u64>,
    total_bytes: Option<u64>,
    files: u64,
    directories: u64,
    current_path: String,
}

impl ProgressMessage {
    fn from_progress(phase: &'static str, progress: &DeleteProgress) -> Self {
        Self {
            phase,
            scan_completed: false,
            detail: progress.current_path.clone(),
            deleted_items: progress.deleted_items,
            freed_bytes: progress.freed_bytes,
            total_items: progress.total_items,
            total_bytes: progress.total_bytes,
            files: 0,
            directories: 0,
            current_path: progress.current_path.clone(),
        }
    }
}

fn item_percent(deleted_items: u64, total_items: Option<u64>) -> Option<u32> {
    let total = total_items.unwrap_or(0);

    if total == 0 {
        return None;
    }

    Some((((deleted_items.min(total)) * 100) / total) as u32)
}

fn byte_percent(freed_bytes: u64, total_bytes: Option<u64>) -> Option<u32> {
    let total = total_bytes.unwrap_or(0);

    if total == 0 {
        return None;
    }

    Some((((freed_bytes.min(total)) * 100) / total) as u32)
}

/// 作业阶段：正在统计规模。
const JOB_PHASE_SCANNING: u8 = 0;

/// 作业阶段：正在删除。
const JOB_PHASE_DELETING: u8 = 1;

/// 作业的控制句柄。
///
/// FORK-MODIFICATION 2026-11-02：带上阶段与去向。
///
/// 取消能不能生效，取决于**当前处在哪个阶段**：
///
/// - 统计阶段：随时能取消 —— 扫描本身带中断点。
/// - 永久删除阶段：能取消 —— 逐条目推进，每个条目都是中断点。
/// - 回收站搬运阶段：**不能取消** —— 整棵树一次性交给 shell，搬运一旦开始
///   就没有中断点。这时候点「取消」只会让界面显示「正在取消…」，而作业照常
///   把整棵树删完（2026-11-02 真机实测：50,051 项删了 45 秒，后端
///   `cancelled=false`）。
///
/// 所以取消命令必须知道自己撞上了哪种情况，不能无脑置标志位。
struct DeleteJobControl {
    cancel: Arc<AtomicBool>,
    phase: Arc<AtomicU8>,
    use_trash: bool,
}

static DELETE_JOBS: LazyLock<Mutex<HashMap<String, DeleteJobControl>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 扫描阶段与删除阶段跑在同一条阻塞线程上，中途的进度通过无界通道回传。
///
/// 返回「删除结果 + 扫描结果」——扫描只做一遍，扫描结果既用来算进度分母，
/// 也用来在结束时如实汇报规模。
///
/// FORK-MODIFICATION 2026-11-01：整条链路都在后台阻塞线程上，主线程不参与
/// 任何一次遍历。旧实现里删除**之前**还有一趟跑在主线程上的同步预扫描，
/// 那才是「删除后长时间无响应」的真正来源。
fn run_delete_blocking(
    job_id: &str,
    paths: Vec<String>,
    use_trash: bool,
    cancel: Arc<AtomicBool>,
    phase: Arc<AtomicU8>,
    progress_tx: UnboundedSender<ProgressMessage>,
) -> (DeleteOutcome, ScanOutcome) {
    // --- 阶段 1：扫描 --------------------------------------------------------
    //
    // 先把要删的树走一遍，拿到条目总数与字节总数。进度条因此有了真实的分母；
    // 界面也能在真正删除开始之前就知道「这个目录有 25 万个文件、72 GiB」。
    let scan_started = std::time::Instant::now();
    let scan = {
        let tx = progress_tx.clone();
        let cancel_ref = &cancel;

        delete_tree::scan_paths_with_progress(&paths, cancel_ref, &move |progress| {
            let _ = tx.send(ProgressMessage::from_progress(DELETE_PHASE_SCANNING, &progress));
        })
    };

    let scan_cancelled = scan.cancelled || cancel.load(Ordering::Relaxed);

    // FORK-MODIFICATION 2026-11-01：扫描阶段结束时单独记一条汇总。
    //
    // 这条是「规模到底数出来没有」的权威读数。以前只能靠翻被节流过的进度行
    // 去反推，而扫描阶段每一行都落盘（见下面的日志抽稀说明），扫 80 万条目
    // 会把 2 MB 的轮转额度一次刷光，真正有用的行全被冲掉。
    log::info!(
        "delete job {} scan finished in {:?}: cancelled={} items={} files={} directories={} bytes={}",
        job_id,
        scan_started.elapsed(),
        scan_cancelled,
        scan.total_items(),
        scan.total_files(),
        scan.total_directories(),
        scan.total_bytes()
    );

    if scan_cancelled {
        return (
            DeleteOutcome {
                cancelled: true,
                ..Default::default()
            },
            scan,
        );
    }

    // FORK-MODIFICATION 2026-11-02：进入删除阶段后，取消的含义变了。
    //
    // 统计阶段的取消一定能生效；删除阶段能不能取消取决于删除方式 ——
    // 回收站搬运是原子的（整棵树一次性交给 shell，中途没有中断点），
    // 永久删除是逐条目的。把阶段记下来，取消命令才知道该不该接受这次请求。
    phase.store(JOB_PHASE_DELETING, Ordering::Relaxed);

    let totals = Some((scan.total_items(), scan.total_bytes()));

    // 这条是「扫描数完了」的信号：阶段仍是 scanning，但分母 / 文件数 / 目录数
    // 齐了。界面的规模提示就靠它触发。
    let _ = progress_tx.send(ProgressMessage {
        phase: DELETE_PHASE_SCANNING,
        scan_completed: true,
        detail: String::new(),
        deleted_items: scan.total_items(),
        freed_bytes: scan.total_bytes(),
        total_items: totals.map(|value| value.0),
        total_bytes: totals.map(|value| value.1),
        files: scan.total_files(),
        directories: scan.total_directories(),
        current_path: String::new(),
    });

    // --- 阶段 2：真正删除 ----------------------------------------------------
    if use_trash {
        let tx = progress_tx.clone();
        let cancel_ref = &cancel;

        let outcome = delete_tree::delete_paths_to_trash(&paths, &scan, cancel_ref, &move |progress| {
            let _ = tx.send(ProgressMessage::from_progress(DELETE_PHASE_DELETING, &progress));
        });

        (outcome, scan)
    }
    else {
        let tx = progress_tx.clone();
        let cancel_ref = &cancel;

        let outcome = delete_tree::delete_paths_permanently(
            &paths,
            cancel_ref,
            totals,
            &move |progress| {
                let _ = tx.send(ProgressMessage::from_progress(DELETE_PHASE_DELETING, &progress));
            },
        );

        (outcome, scan)
    }
}

// FORK-MODIFICATION 2026-10-20：移植上游 0ae258d6。
//
// 告诉前端「这批路径里哪些不能进回收站」，前端据此把它们拆成
// 「本地进回收站」+「远程先确认再永久删除」两个作业。
#[tauri::command]
pub fn recycle_bin_unavailable_paths(paths: Vec<String>) -> Vec<String> {
    crate::utils::recycle_bin_unavailable_paths(paths)
}

// FORK-MODIFICATION 2026-11-01：`scan_delete_targets` 命令与 `DeleteTargetSummary`
// 结构体已删除。
//
// 原来有一个「删除前先看清规模」的同步命令：它把要删的树整个走一遍，把条目数
// 与字节数交给前端弹提示。它的问题不是「慢」，而是**跑在 Tauri 主线程上** ——
// 同步命令在 Tauri v2 里由主线程执行，扫一棵大树时整个窗口（含状态栏、
// 进度条、取消按钮）全部冻住，用户看到的正是「删除后长时间无响应」。
//
// 而且那趟遍历与删除作业自己的扫描阶段**完全同源** —— 同一棵树扫两遍，
// 第二遍还是全量扫。现在规模由作业的扫描阶段独家产出，既不重复扫，
// 也不占主线程。

#[tauri::command]
pub async fn start_delete_job(
    app: AppHandle,
    paths: Vec<String>,
    use_trash: bool,
    job_id: String,
) -> Result<(), String> {
    let paths = minimize_delete_paths(paths);
    if paths.is_empty() {
        return Err("No paths to delete".to_string());
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let phase = Arc::new(AtomicU8::new(JOB_PHASE_SCANNING));
    {
        let mut guard = DELETE_JOBS
            .lock()
            .map_err(|_| "Delete job registry lock failed".to_string())?;
        guard.insert(
            job_id.clone(),
            DeleteJobControl {
                cancel: cancel.clone(),
                phase: phase.clone(),
                use_trash,
            },
        );
    }

    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<ProgressMessage>();

    let app_progress = app.clone();
    let job_id_progress = job_id.clone();
    let emit_progress = tokio::spawn(async move {
        // FORK-MODIFICATION 2026-10-22：进度要落盘。
        //
        // 「进度条按条目数推进」这件事，只有日志能证明 —— 屏幕上的百分比既会
        // 被误读，也没法在事后回看。顺带这条日志也是「删除到底删了多少」的
        // 唯一权威读数（界面上那个数字是给人看的，日志里这条是给排查看的）。
        //
        // 每 10 个事件记一行：事件本身已经按 100ms / 500 条目节流过，
        // 再按 10 倍抽稀，长任务也不会把日志刷爆。
        let mut progress_event_count: u64 = 0;
        // FORK-MODIFICATION 2026-11-01：日志按「阶段切换」抽稀。
        //
        // 旧条件里的 `|| message.phase == DELETE_PHASE_SCANNING` 让**每一条**
        // 扫描进度都进日志：扫 80 万条目就是 80 万行，把 2 MB 的轮转额度一次
        // 刷光，作业开始 / 扫描汇总 / 作业结束这些真正有用的行全被冲掉。
        // 现在改成「阶段切换必记 + 每 10 条记一行」。
        let mut last_logged_phase: Option<&'static str> = None;

        while let Some(message) = progress_rx.recv().await {
            progress_event_count += 1;

            let phase_changed = last_logged_phase != Some(message.phase);
            last_logged_phase = Some(message.phase);

            if phase_changed || progress_event_count % 10 == 0 {
                log::info!(
                    "delete job {} [{}] phase={} items={} total_items={:?} freed_bytes={} total_bytes={:?} percent={:?} current={}",
                    job_id_progress,
                    progress_event_count,
                    message.phase,
                    message.deleted_items,
                    message.total_items,
                    message.freed_bytes,
                    message.total_bytes,
                    item_percent(message.deleted_items, message.total_items),
                    message.current_path
                );
            }

            let payload = DeleteJobProgressPayload {
                job_id: job_id_progress.clone(),
                percent: item_percent(message.deleted_items, message.total_items).unwrap_or(0),
                detail: message.detail.clone(),
                processed_count: Some(message.deleted_items),
                total_count: message.total_items,
                phase: message.phase.to_string(),
                scan_completed: message.scan_completed,
                deleted_items: message.deleted_items,
                freed_bytes: message.freed_bytes,
                total_bytes: message.total_bytes,
                item_percent: item_percent(message.deleted_items, message.total_items),
                byte_percent: byte_percent(message.freed_bytes, message.total_bytes),
                files: message.files,
                directories: message.directories,
                current_path: message.current_path,
            };
            let _ = app_progress.emit("delete-job-progress", &payload);
        }
    });

    let app_done = app.clone();
    let job_id_done = job_id.clone();
    let use_trash_done = use_trash;
    let started_at = std::time::Instant::now();

    log::info!(
        "delete job {} started: {} path(s), use_trash={}",
        job_id_done,
        paths.len(),
        use_trash_done
    );

    tokio::spawn(async move {
        let job_id_for_scan = job_id_done.clone();
        let work_result = tokio::task::spawn_blocking(move || {
            run_delete_blocking(&job_id_for_scan, paths, use_trash, cancel, phase, progress_tx)
        })
        .await;

        let _ = emit_progress.await;

        let finished = match work_result {
            Ok((outcome, scan)) => {
                build_finished_payload(&job_id_done, use_trash_done, &outcome, &scan)
            }
            Err(join_error) => DeleteJobFinishedPayload {
                job_id: job_id_done.clone(),
                success: false,
                cancelled: false,
                use_trash: use_trash_done,
                error: Some(format!("Delete task failed: {}", join_error)),
                deleted_paths: Vec::new(),
                deleted_items: 0,
                freed_bytes: 0,
                total_items: 0,
                files: 0,
                directories: 0,
                total_bytes: 0,
            },
        };

        let _ = app_done.emit("delete-job-finished", &finished);

        log::info!(
            "delete job {} finished in {:?}: cancelled={} deleted_items={} freed_bytes={} total_items={} files={} directories={} failed={} error={:?}",
            job_id_done,
            started_at.elapsed(),
            finished.cancelled,
            finished.deleted_items,
            finished.freed_bytes,
            finished.total_items,
            finished.files,
            finished.directories,
            finished.error.is_some(),
            finished.error
        );

        if let Ok(mut guard) = DELETE_JOBS.lock() {
            guard.remove(&job_id_done);
        }
    });

    Ok(())
}

fn build_finished_payload(
    job_id: &str,
    use_trash: bool,
    outcome: &DeleteOutcome,
    scan: &ScanOutcome,
) -> DeleteJobFinishedPayload {
    let error = if outcome.cancelled {
        None
    }
    else if outcome.failed_count > 0 {
        match (outcome.failed_count, outcome.last_error.clone()) {
            (count, Some(last)) if count > 1 => {
                Some(format!("{} paths failed. Last error: {}", count, last))
            }
            (_, Some(last)) => Some(last),
            (count, None) if count > 0 => Some(format!("{} paths failed", count)),
            _ => None,
        }
    }
    else {
        None
    };

    DeleteJobFinishedPayload {
        job_id: job_id.to_string(),
        success: !outcome.cancelled && outcome.failed_count == 0,
        cancelled: outcome.cancelled,
        use_trash,
        error,
        deleted_paths: outcome.deleted_paths.clone(),
        deleted_items: outcome.deleted_items,
        freed_bytes: outcome.freed_bytes,
        total_items: scan.total_items(),
        files: scan.total_files(),
        directories: scan.total_directories(),
        total_bytes: scan.total_bytes(),
    }
}

/// 取消请求的结果。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteCancelOutcome {
    /// 这次取消有没有被接受。
    pub accepted: bool,
    /// 作业当前阶段：`scanning` / `deleting`（作业不存在时为空串）。
    pub phase: String,
    /// 没被接受的原因代码。
    ///
    /// - `unknownJob`：作业已经结束，或根本不存在。
    /// - `registryLocked`：作业登记表被别的线程占着。
    /// - `trashMoveInProgress`：正在往回收站搬，搬运中途没有中断点。
    pub reason: Option<String>,
}

/// 取消一个删除作业。
///
/// FORK-MODIFICATION 2026-11-02：不再无脑置标志位。
///
/// 旧实现只要作业还在册就置标志位并返回 `true`，界面于是显示「正在取消…」——
/// 但回收站搬运期根本没有中断点，作业照常把整棵树删完（2026-11-02 真机实测：
/// 点了取消，50,051 项一项没少，后端 `cancelled=false`）。界面在骗人。
///
/// 现在按阶段判定：统计阶段与永久删除阶段接受取消；回收站搬运阶段拒绝，
/// 并把原因带回前端，由前端把「正在取消…」改回进行中。
#[tauri::command]
pub fn cancel_delete_job(job_id: String) -> DeleteCancelOutcome {
    let Ok(guard) = DELETE_JOBS.lock() else {
        return DeleteCancelOutcome {
            accepted: false,
            phase: String::new(),
            reason: Some("registryLocked".to_string()),
        };
    };

    let Some(control) = guard.get(&job_id) else {
        return DeleteCancelOutcome {
            accepted: false,
            phase: String::new(),
            reason: Some("unknownJob".to_string()),
        };
    };

    let phase = control.phase.load(Ordering::Relaxed);

    if phase == JOB_PHASE_DELETING && control.use_trash {
        log::info!(
            "delete job {} cancel rejected: the tree is already being moved to the recycle bin",
            job_id
        );

        return DeleteCancelOutcome {
            accepted: false,
            phase: DELETE_PHASE_DELETING.to_string(),
            reason: Some("trashMoveInProgress".to_string()),
        };
    }

    control.cancel.store(true, Ordering::Relaxed);
    log::info!(
        "delete job {} cancel accepted (phase={})",
        job_id,
        if phase == JOB_PHASE_SCANNING {
            DELETE_PHASE_SCANNING
        } else {
            DELETE_PHASE_DELETING
        }
    );

    DeleteCancelOutcome {
        accepted: true,
        phase: if phase == JOB_PHASE_SCANNING {
            DELETE_PHASE_SCANNING.to_string()
        } else {
            DELETE_PHASE_DELETING.to_string()
        },
        reason: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn register_job(
        job_id: &str,
        phase: u8,
        use_trash: bool,
    ) -> Arc<AtomicBool> {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut guard = DELETE_JOBS.lock().expect("registry lock");
        guard.insert(
            job_id.to_string(),
            DeleteJobControl {
                cancel: cancel.clone(),
                phase: Arc::new(AtomicU8::new(phase)),
                use_trash,
            },
        );
        cancel
    }

    fn unregister_job(job_id: &str) {
        let mut guard = DELETE_JOBS.lock().expect("registry lock");
        guard.remove(job_id);
    }

    #[test]
    fn cancel_is_accepted_while_scanning() {
        let job_id = "unit-cancel-scanning";
        let flag = register_job(job_id, JOB_PHASE_SCANNING, true);

        let outcome = cancel_delete_job(job_id.to_string());

        assert!(outcome.accepted);
        assert_eq!(outcome.phase, DELETE_PHASE_SCANNING);
        assert!(outcome.reason.is_none());
        assert!(flag.load(Ordering::Relaxed));

        unregister_job(job_id);
    }

    #[test]
    fn cancel_is_rejected_while_the_tree_is_moving_to_the_recycle_bin() {
        let job_id = "unit-cancel-trash-move";
        let flag = register_job(job_id, JOB_PHASE_DELETING, true);

        let outcome = cancel_delete_job(job_id.to_string());

        assert!(!outcome.accepted);
        assert_eq!(outcome.phase, DELETE_PHASE_DELETING);
        assert_eq!(outcome.reason.as_deref(), Some("trashMoveInProgress"));
        // 拒绝时**不能**置标志位：置了就是骗作业说「请停下来」。
        assert!(!flag.load(Ordering::Relaxed));

        unregister_job(job_id);
    }

    #[test]
    fn cancel_is_accepted_while_deleting_permanently() {
        let job_id = "unit-cancel-permanent";
        let flag = register_job(job_id, JOB_PHASE_DELETING, false);

        let outcome = cancel_delete_job(job_id.to_string());

        assert!(outcome.accepted);
        assert_eq!(outcome.phase, DELETE_PHASE_DELETING);
        assert!(flag.load(Ordering::Relaxed));

        unregister_job(job_id);
    }

    #[test]
    fn cancel_of_an_unknown_job_is_rejected() {
        let outcome = cancel_delete_job("unit-cancel-unknown-job".to_string());

        assert!(!outcome.accepted);
        assert_eq!(outcome.reason.as_deref(), Some("unknownJob"));
    }

    #[test]
    fn item_percent_uses_denominator() {
        assert_eq!(item_percent(250, Some(1000)), Some(25));
        assert_eq!(item_percent(250, None), None);
        assert_eq!(item_percent(250, Some(0)), None);
    }

    #[test]
    fn item_percent_never_exceeds_hundred() {
        assert_eq!(item_percent(5000, Some(1000)), Some(100));
    }

    #[test]
    fn byte_percent_uses_denominator() {
        assert_eq!(byte_percent(10, Some(100)), Some(10));
        assert_eq!(byte_percent(10, None), None);
        assert_eq!(byte_percent(10, Some(0)), None);
    }
}
