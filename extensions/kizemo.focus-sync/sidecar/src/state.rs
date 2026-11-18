//! In-memory state: current Sigma path + active file dialog registry + health tracking.
//!
//! Per handoff §6.5: `current_path` is `Mutex<String>` (not `String`),
//! accessed via `set_current_path(&self, p)` + `get_current_path(&self) -> String`.
//!
//! v0.3.0 (focus-sync Scheduled Task migration): added health tracking fields
//! surfaced via `/health` HTTP endpoint for Sigma FM extension monitoring:
//! - `startup_time: Instant`        — uptime calculation
//! - `last_ping_received_ts: AtomicU64` — last `/set_path` (Unix millis, 0 if never)
//! - `last_dialog_event_ts: AtomicU64`  — last `DialogEvent::Opened` (Unix millis)
//! - `uia_healthy: AtomicBool`           — set false if UIA self-check fails 3x
//! - `session_id: u32`                   — set once at startup (ProcessIdToSessionId)

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct DialogInfo {
    pub hwnd: u32,
    pub app: String,
    pub last_known_path: String,
    pub write_strategy: Option<String>,
}

/// v0.6.3 — 「我们最后一次成功写进某个对话框的路径」＋「写入瞬间对话框所在的目录」。
///
/// `path` = 写入成功的目标路径。
/// `observed_dir` = 写入前读到的对话框当前目录（读不到就是 None）。它是
///            「写入之后对话框有没有被挪走」的基线：Chromium 类对话框的
///            SetValue 并不移动对话框，读回来的目录在写入前后一致 ⇒ 靠它
///            区分「我们的写入没让对话框移动」与「用户手动换了个目录」。
#[derive(Debug, Clone)]
pub struct WriteRecord {
    pub path: String,
    pub observed_dir: Option<String>,
}

#[derive(Debug)]
pub struct AppState {
    pub current_path: Mutex<String>,
    pub registry: Mutex<HashMap<u32, DialogInfo>>,

    // ---- v0.6.3: 每个对话框「我们实际写进去过什么」。
    //
    // 与 `registry` 的 `last_known_path` 是两件事：
    //   * registry.last_known_path = 「我们认为对话框现在显示什么」（可能是读
    //     回来的目录，也可能是写失败时保留的旧值）；
    //   * write_records[hwnd]      = 「我们最后一次**成功**写进这个对话框的路径」。
    // 写失败**不得**留下记录（v0.5.12 不变量：写失败必须保持可重试）。
    pub write_records: Mutex<HashMap<u32, WriteRecord>>,

    // ---- v0.6.4: per-dialog write gate (serializes the two write paths).
    //
    // v0.6.3 unified the dedup key for both write paths, but the write record
    // is only persisted AFTER a successful write. Two overlapping writes (the
    // HTTP /set_path immediate write vs. the DialogEvent::Opened write-back)
    // therefore both pass the dedup check before either one has recorded
    // anything => the same path gets written twice.
    //
    // Real machine (batch 20, 2026-11-18): one dialog open produced two Write
    // events for the same target.
    //
    // Fix: both paths lock the SAME per-hwnd gate and re-run their dedup
    // check AFTER acquiring it, so the second writer sees the record left by
    // the first one and skips.
    pub write_gates: Mutex<HashMap<u32, Arc<Mutex<()>>>>,

    // Health tracking (v0.3.0)
    pub startup_time: Instant,
    pub last_ping_received_ts: AtomicU64,
    pub last_dialog_event_ts: AtomicU64,
    pub uia_healthy: AtomicBool,
    pub session_id: u32,

    // focus-18: per-dialog single-shot commit dedup.
    // Semantics: try_mark_committed(hwnd) -> true if first time (may commit),
    //            false if already committed (skip Invoke).
    // clear_committed(hwnd) -> on DialogEvent::Closed, allow next dialog commit.
    pub committed_hwnds: Mutex<HashSet<u32>>,

    // focus-19 (2026-10-02): counter of "dialogs we could not navigate".
    // Incremented by uia_inject::inject_folder_path when the dialog is
    // neither a native IFileDialog (COM SetFolder succeeded) nor a Chromium
    // Save As (41477 address bar). Typical cause: Edge Win11 22H2+ WinUI
    // Save As, which is unsupported by the current detection logic.
    //
    // Surfaced via /health so the extension can poll and notify the user
    // (deep-analysis §4.2 Option B + §8 task 5). Note: count is monotonic
    // for the life of the process; we do NOT rate-limit here. The extension
    // can throttle notifications by tracking last-seen count.
    pub unsupported_dialog_count: AtomicU64,

    // v0.5.5 (Mavis Round 21 B-1 + Round 23 N-1 + Round 25 Q-5): boolean
    // flag tracking whether extension has pushed at least one path. Until
    // this is true, opened_write_back MUST skip — otherwise it'd write the
    // init sentinel "C:\\" to every new dialog (race condition observed in
    // spike.log 06:41:12 etc.). Using a boolean flag instead of string
    // equality (e.g. `current != "C:\\"`) avoids false negatives when user
    // legitimately navigates to "C:\\" root.
    pub first_push_received: AtomicBool,

    // ---- v0.5.7 F-1: single source of truth for "which dialog is foreground"
    //
    // WHY WE DO NOT CALL GetForegroundWindow() INSTEAD:
    //   GetForegroundWindow() answers for the CALLING THREAD's input queue.
    //   `uia_event::start_monitor` runs via spawn_blocking and pumps Windows
    //   messages (GetMessageW/DispatchMessageW), so its answer is correct.
    //   `http_server::run_server` runs on a plain `std::thread::spawn` with NO
    //   message pump; its answers disagreed with the monitor thread's.
    //
    // OBSERVED FAILURE (2026-10-07 16:39): the monitor tracked hwnd=135716 as
    // foreground continuously from 16:39:12.270 to 16:39:25.456, while the
    // HTTP thread's foreground check reported "not foreground" at 16:39:21 and
    // deferred EVERY /set_path. Sync never happened. This also explains the
    // long-standing `current=HWND(0x0)` in the SendInput warn line.
    //
    // Contract: the monitor thread publishes here every tick; everyone else
    // reads this value instead of querying the OS themselves.
    // 0 = no foreground file dialog.
    pub foreground_dialog_hwnd: AtomicI64,

    // ---- v0.5.8: is the extension actually alive right now?
    //
    // The plugin polls `GET /health` every 15s for its own health check. That
    // poll is an independent liveness signal for Sigma FM's extension worker:
    // if /health has not been hit recently, the extension is NOT running, and
    // therefore no fresh path can possibly be arriving.
    //
    // WHY THIS EXISTS (observed twice on 2026-10-08):
    //   Sigma FM started 15:34:11 -> extension activated 15:37:09 (2m58s late)
    //   Sigma FM started 16:31:41 -> still not active 100s later
    // During that window the sidecar held the PRE-restart path and injected it
    // into every new dialog, because nothing marked it as stale. The user saw
    // "old address gets filled in" and no amount of switching in Sigma FM could
    // fix it, since the extension that would push a new path was not up yet.
    //
    // Note this is strictly better than a time-since-last-PUSH threshold: a
    // user who navigates once and then works in a dialog for an hour keeps
    // getting polls, so sync still works. Only a genuinely absent or
    // not-yet-started extension stops it.
    pub last_health_poll_ts: AtomicU64,

    // ---- v0.5.7 G-2: is a write-back currently executing?
    //
    // The monitor polls every 8ms while tracking a dialog; one UIA+SendInput
    // write takes ~300ms. Without this flag the monitor re-fires `Opened`
    // ~30x per write. Combined with G-1 that became an infinite resync loop
    // (observed 2026-10-07 21:37, 1364 resyncs of the SAME path, address bar
    // accumulating the path over and over).
    pub sync_in_progress: AtomicBool,

    // ---- v0.5.19: AFM 交接闸门（handoff gate）
    //
    // v0.5.15 的「访问闩」只上弦、不下弦：只要曾经把窗口切到过 AFM，授权就
    // 永久有效。真机证据（2026-11-09）：11:09:28 在 AFM 里停在百度网盘远程
    // 目录，11:31:18 在另一个程序里打开对话框，路径照样被写进去 —— 中间隔了
    // 22 分钟，用户中间已经切走过好几个程序。
    //
    // v0.5.19 换成「交接」语义：记录「用户从 AFM 切走的那一刻」与「接手前台的
    // 应用 pid」。放行要三条同时成立：
    //   1. 确实发生过 AFM -> X 的交接；
    //   2. 交接距今不超过 TTL（FOCUS_SYNC_AFM_HANDOFF_TTL_MS，默认 120 秒）；
    //   3. 弹窗所属进程 == 接手 AFM 的那个应用。
    // 这正是用户诉求：「如果没有直接从 AFM 切换到弹窗，则不要同步路径」。
    //
    // 交接的观察点仍在 uia_event.rs 的 monitor 循环 —— 它是全进程唯一可靠的
    // 前台观察方（带消息泵；state.rs 的 foreground_dialog 字段注释记录了 HTTP
    // 线程自问 GetForegroundWindow 不可靠的事故）。
    //
    // 逃生阀不变：`FOCUS_SYNC_AFM_GATE=0|false|off` 时闸门恒开（回到 v0.5.14
    // 行为），便于真机 A/B 对照。
    pub afm_handoff: Mutex<Option<AfmHandoff>>,

    // ---- v0.5.21: AFM 进程身份（修「假下降沿」）----
    //
    // 真机根因（2026-11-17）：`is_afm_window` 只认「类名 == `Tauri Window`
    // 且标题以 `Alpha File Manager` 开头」。装机版 AFM 进程另外持有**可见**
    // 的辅助窗口（类名 `com.sigma-file-manager.app-sic` / `-siw`），它不满足
    // 那个判据；它持有前台时用户其实就在 AFM，但 `foreground_is_afm()` 恒为
    // false ⇒ 闸门产生**假下降沿** ⇒ 交接记录把 AFM 自己的 pid 写进授权
    // （真机日志 `spike.log.2026-11-17` 12:15:00 那条
    // `handoff recorded: pid=72772`，72772 正是 AFM 的 pid）⇒ 之后所有真实
    // 对话框的 pid 都匹配不上 ⇒ 656 次 `opened_write_back_skip_no_afm_visit`，
    // 当天只写回成功 1 次。
    //
    // 修法：识别「AFM 在前台」从「窗口特征」升级为「进程身份」——只要前台
    // 窗口属于 AFM 那个进程（主窗口 / 辅助窗口 / 托盘窗），都算「用户在 AFM」。
    //
    // 0 = 还没学到（侧车刚起来、还没枚举到 AFM 窗口）。
    pub afm_pid: AtomicU32,

    // ---- v0.5.22: 闸门路径 B 的时间戳 + 交接链续期所需的状态 ----
    //
    // v0.5.21 把闸门完全建立在「pid 相等」上，真机 2026-11-18 证伪：
    //   * 接收方 = `UGREEN NAS.exe`(15204)，文件对话框属主 = 619924
    //     （该应用派生的对话框宿主进程）⇒ 精确 pid 相等误拒合法写回；
    //   * 用户切回对话框时先点**任务栏** ⇒ 采样落在 `explorer.exe`(13080)
    //     ⇒ 把 shell 记成了「接收方应用」；
    //   * 用户在接收应用里停留两小时 ⇒ 120 秒 TTL 早过期。
    //
    // v0.5.22 用两条**独立**的放行路径覆盖这两种真实流程（见 `afm_gate_allows`）：
    //   A. 交接链身份：对话框属主 == 从 AFM 切进来的那个应用（含 helper 子进程）；
    //   B. 「对话框开着的时候用户去过 AFM」——Listary 原义：
    //      去 AFM 拿路径 → 切回对话框 → 路径自动填上。
    //
    /// v0.5.22 — 用户最近一次在 AFM 前台的时刻（Unix 毫秒，0 = 从没有）。
    pub last_afm_foreground_ts: AtomicU64,
    /// v0.5.22 — 每个对话框**首次被观察到**的时刻（Unix 毫秒）。
    pub dialog_opened_ts: Mutex<HashMap<u32, u64>>,
}

/// v0.5.7 G-2 — RAII guard so a write-back cannot be left stuck "in
/// progress" if it returns early or panics. The monitor refuses to re-fire
/// while this is set, so a leaked flag would freeze re-sync (fail-safe:
/// sync stops rather than loops).
pub struct SyncGuard<'a> {
    state: &'a AppState,
}

impl<'a> Drop for SyncGuard<'a> {
    fn drop(&mut self) {
        self.state.sync_in_progress.store(false, Ordering::SeqCst);
    }
}

impl<'a> std::ops::Deref for SyncGuard<'a> {
    type Target = AppState;
    fn deref(&self) -> &Self::Target {
        self.state
    }
}

/// v0.5.16 — 路径归一化：把「同一个目录」的两种写法变得可比。
///
/// 两侧来源不同：reader 从对话框读回的是 `F:/dir`（正斜杠，shell 返回），
/// AFM 推给侧车的是 `F:\dir`（反斜杠）。字符串直接比较**永远**不相等 ⇒
/// 同一个目录被判成「不同」⇒ 重复写入（v0.5.13 的闪烁回归）。
///
/// 归一化内容：统一分隔符为 `\`、去掉末尾分隔符（根目录 `C:\` 保留一个）、
/// 忽略大小写（Windows 路径本身大小写不敏感）。
pub fn normalize_path(p: &str) -> String {
    let s = p.trim().replace('/', "\\").to_lowercase();
    if s.is_empty() {
        return String::new();
    }
    let mut out = s.trim_end_matches('\\').to_string();
    // `C:\` 归一化成 `c:` 会与裸盘符 `c:` 混为一谈；根目录保留一个反斜杠。
    if out.len() == 2 && out.ends_with(':') {
        out.push('\\');
    }
    out
}

/// v0.5.16 — 两个路径是否指向同一个目录（归一化后比较）。
pub fn paths_equal(a: &str, b: &str) -> bool {
    normalize_path(a) == normalize_path(b)
}

/// v0.5.19 — 这个路径能不能被写进别的程序的对话框？
///
/// 背景（真机 2026-11-09 11:31）：AFM 停在百度网盘远程目录时，扩展把
/// `sfm://remote/<id>/0000temp` 的 scheme 剥掉，得到伪路径
/// `\remote\<id>\0000temp`，侧车把它写进了另一个程序的「打开」对话框 ⇒
/// Windows 弹「找不到 \remote\...\0000temp」错误框；那个错误框又被
/// monitor 当成新对话框 ⇒ 每 1~2 秒重试一次 ⇒ 关闭后仍反复报错。
///
/// 判定两段：
///   1. **形状**：必须是盘符路径 `X:\...` 或 UNC `\\server\share`。
///      根相对路径（`\remote\...`）、`sfm://...`、`\locations` 全部拒绝。
///   2. **存在性**：本地路径必须是**真实存在的目录**。往别人的对话框里写
///      一个不存在的目录，换来的一定是对方的报错框。
///
/// UNC 只查形状不查存在性：网络盘可能离线，探测会阻塞在 IO 上。
/// v0.5.21 — 这个路径**形状**上是不是一个本地路径？
///
/// 与 [`is_writable_local_path`] 的区别：这里**不查存在性**，只查形状。
///
/// 用途：`current_path` 的「存入」侧守卫。扩展把 AFM 的虚拟位置剥掉 scheme
/// 之后推来的是 `\locations`、`\remote\<id>\0000temp` —— 根相对路径，既不
/// 是盘符路径也不是 UNC。原样存入 `current_path` 的后果：
///   * 写回侧虽有 `is_writable_local_path` 拦着（不会写进别人的对话框），
///     但 `current_path` 已被污染，`/health` 报出来的路径是假的；
///   * 去重键 `paths_equal(initial, current)` 全部失配 ⇒ 重复写回。
///
/// 存在性**故意不查**：`current_path` 只是「AFM 此刻指向哪里」的记录，写入侧
/// 已经用 [`is_writable_local_path`] 做过存在性把关。查存在性会把「目录临时
/// 不可用」升级成「路径丢失」，那是过度反应。
pub fn is_local_path_shape(p: &str) -> bool {
    let s = p.trim();
    if s.is_empty() {
        return false;
    }
    let b = s.as_bytes();
    // 盘符路径 `X:\...` / `X:/...`（至少 3 字节；裸盘符 `C:` 不算）
    let is_drive = b.len() >= 3 && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/');
    // UNC `\\server\share`
    let is_unc = s.starts_with("\\\\");
    is_drive || is_unc
}

pub fn is_writable_local_path(p: &str) -> bool {
    let s = p.trim();
    if s.is_empty() {
        return false;
    }
    let b = s.as_bytes();
    let is_drive = b.len() >= 3 && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/');
    let is_unc = s.starts_with("\\\\");
    if !is_drive && !is_unc {
        return false;
    }
    if is_unc {
        return true;
    }
    std::path::Path::new(s).is_dir()
}

/// v0.5.19 — 一次「AFM -> 其它应用」的交接记录。
///
/// `pid` = 接手前台的**应用进程**（不是窗口）：同一个应用里在主窗口和
/// 文件对话框之间来回切换不会改变 pid，所以「用户在 Edge 里点了下载，
/// 弹出另存为」这种同应用内的切换不会被误判成「换了应用」。
#[derive(Debug, Clone, Copy)]
pub struct AfmHandoff {
    pub pid: u32,
    pub ts_ms: u64,
}

/// v0.5.19 — 当前 Unix 毫秒（交接时效判定用）。
pub fn now_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ---- v0.5.22 判别性测试 ----
//
// 覆盖真机 2026-11-18 暴露的三类误拒：
//   ① 对话框属主是接收应用派生的 helper 进程（pid 不等但同一应用）；
//   ② 「对话框开着时用户去过 AFM」（路径 B）应放行；
//   ③ 陈旧的 AFM 访问不得授权后来才弹出的对话框（2026-11-09 事故形态）。
#[cfg(test)]
mod v522_gate_paths_tests {
    use super::{now_unix_millis, AppState};

    #[test]
    fn a_helper_child_of_the_receiving_app_counts_as_the_same_application() {
        // 真机形态：接收方 15204（UGREEN NAS.exe），弹窗属主 619924（它的
        // 对话框宿主子进程）。
        let parents: std::collections::HashMap<u32, u32> =
            [(619_924u32, 152_04u32)].into_iter().collect();
        assert!(
            crate::uia_event::same_application_in(15204, 619_924, |pid| {
                parents.get(&pid).copied()
            }),
            "接收应用的 helper 子进程必须算同一个应用"
        );
        assert!(
            !crate::uia_event::same_application_in(15204, 0x7fff_fffe, |pid| {
                parents.get(&pid).copied()
            }),
            "毫无关系的进程不得被当成同一个应用"
        );
    }

    #[test]
    fn a_visit_to_afm_while_the_dialog_was_open_authorizes_the_sync() {
        // 路径 B：Listary 原义 —— 去 AFM 拿路径 → 切回对话框。
        let state = AppState::new("E:\\Models".into(), 1);
        state.mark_dialog_opened(777);
        state.set_last_afm_foreground_ts(now_unix_millis());
        assert!(
            state.afm_gate_allows(777, 9999),
            "对话框开着的时候用户去过 AFM ⇒ 必须放行"
        );
    }

    #[test]
    fn a_stale_visit_does_not_authorize_a_later_dialog() {
        // 真机 2026-11-09 事故形态：22 分钟前的 AFM 访问不得授权后来弹出的
        // 对话框（这里没有任何交接链 ⇒ 两条路径都不成立）。
        let state = AppState::new("E:\\Models".into(), 1);
        state.mark_dialog_opened(777);
        state.set_last_afm_foreground_ts(now_unix_millis().saturating_sub(600_000));
        assert!(
            !state.afm_gate_allows(777, 9999),
            "陈旧的 AFM 访问不得授权后来弹出的对话框"
        );
    }

    #[test]
    fn the_gate_still_refuses_a_dialog_of_an_unrelated_app() {
        let state = AppState::new("E:\\Models".into(), 1);
        state.learn_afm_pid(true, 72772);
        state.set_afm_handoff(4242);
        assert!(
            state.afm_handoff_allows(4242),
            "接收方自己的对话框必须放行"
        );
        assert!(
            !state.afm_handoff_allows(0x7fff_fffe),
            "无关应用的对话框必须拒绝"
        );
    }

    #[test]
    fn refresh_keeps_a_live_chain_alive_but_never_revives_a_dead_one() {
        let state = AppState::new("E:\\Models".into(), 1);
        state.set_afm_handoff(4242);
        state.refresh_afm_handoff();
        assert!(state.afm_handoff_is_fresh());
        assert!(state.afm_handoff_allows(4242));

        {
            let mut guard = state.afm_handoff.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some(super::AfmHandoff {
                pid: 4242,
                ts_ms: now_unix_millis().saturating_sub(10 * 60 * 1000),
            });
        }
        state.refresh_afm_handoff();
        assert!(
            !state.afm_handoff_is_fresh(),
            "过期的交接链不得被续期复活"
        );
        assert!(!state.afm_handoff_allows(4242));
    }
}

impl AppState {
    pub fn new(initial_path: String, session_id: u32) -> Self {
        Self {
            current_path: Mutex::new(initial_path),
            registry: Mutex::new(HashMap::new()),
            write_records: Mutex::new(HashMap::new()),
            write_gates: Mutex::new(HashMap::new()),
            startup_time: Instant::now(),
            last_ping_received_ts: AtomicU64::new(0),
            last_dialog_event_ts: AtomicU64::new(0),
            uia_healthy: AtomicBool::new(true), // optimistic; self-check may flip
            session_id,
            committed_hwnds: Mutex::new(HashSet::new()),
            unsupported_dialog_count: AtomicU64::new(0),
            first_push_received: AtomicBool::new(false),
            foreground_dialog_hwnd: AtomicI64::new(0),
            last_health_poll_ts: AtomicU64::new(0),
            sync_in_progress: AtomicBool::new(false),
            // v0.5.19: 还没发生过 AFM -> X 的交接，闸门初始关闭。
            afm_handoff: Mutex::new(None),
            // v0.5.21: AFM 进程身份还没学到（monitor 会用 EnumWindows 学）。
            afm_pid: AtomicU32::new(0),
            // v0.5.22: 闸门路径 B 与交接链续期所需的时间戳。
            last_afm_foreground_ts: AtomicU64::new(0),
            dialog_opened_ts: Mutex::new(HashMap::new()),
        }
    }

    /// v0.5.19 — 记录「用户从 AFM 切走」：接手的应用 pid + 发生时刻。
    ///
    /// 由 uia_event.rs 的 monitor 在 **AFM -> 非 AFM** 的下降沿调用。
    ///
    /// v0.5.20 — `pid == 0`（没读到接手应用的进程 id）**不产生授权**。
    /// 理由：授权的可核对性完全建立在 pid 上；一条 pid 未知的交接记录
    /// 没有任何东西可以核对，闸门只能对它「无法核对 ⇒ 放行」，于是它会
    /// 变成一把 TTL 内谁都能用的万能钥匙 —— 正是本闸门要堵的洞。
    /// 取不到 pid 时正确做法是**不记录**，由调用方（monitor）下一 tick 重试。
    pub fn set_afm_handoff(&self, pid: u32) {
        if pid == 0 {
            return;
        }
        let mut guard = self.afm_handoff.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(AfmHandoff {
            pid,
            ts_ms: now_unix_millis(),
        });
    }

    /// v0.5.19 — 用户回到 AFM：作废上一次交接（陈旧授权不许复活）。
    pub fn clear_afm_handoff(&self) {
        let mut guard = self.afm_handoff.lock().unwrap_or_else(|e| e.into_inner());
        *guard = None;
    }

    /// v0.5.22 — 「用户仍停留在接收方应用里」⇒ 给交接链续期。
    ///
    /// 为什么需要续期：真机 2026-11-18 的真实流程是
    /// 「23:58 从 AFM 切进 UGREEN NAS → 02:17 该应用弹出文件对话框」。
    /// 中间两小时用户**一直待在那个应用里**，但交接记录停在 23:58 ⇒
    /// 120 秒 TTL 早过期 ⇒ 合法写回被拒。
    ///
    /// 续期不是放宽闸门：它只延长「同一条 AFM → X 链」的有效期，且
    /// **不会复活已过期的链**（续期前必须仍是新鲜的）⇒
    /// 「用户去了别的应用待够久」这条链仍然会死。
    pub fn refresh_afm_handoff(&self) {
        let mut guard = self.afm_handoff.lock().unwrap_or_else(|e| e.into_inner());
        let Some(handoff) = guard.as_mut() else {
            return;
        };
        let now = now_unix_millis();
        if now.saturating_sub(handoff.ts_ms) > Self::afm_handoff_ttl_ms() {
            // 已经过期的链不许复活。
            return;
        }
        handoff.ts_ms = now;
    }

    /// v0.5.22 — 记录「用户最近一次在 AFM 前台」的时刻。
    pub fn set_last_afm_foreground_ts(&self, ts_ms: u64) {
        self.last_afm_foreground_ts.store(ts_ms, Ordering::Relaxed);
    }

    /// v0.5.22 — 「用户最近一次在 AFM 前台」的时刻（0 = 从没有）。
    pub fn last_afm_foreground_ts(&self) -> u64 {
        self.last_afm_foreground_ts.load(Ordering::Relaxed)
    }

    /// v0.5.22 — 记录对话框**首次被观察到**的时刻（返回 true = 首次）。
    ///
    /// 「首次」很关键：monitor 对同一个对话框会反复发 `Opened`（resync 通道），
    /// 打开时刻必须固定在**第一次**看到它的时候，否则「用户去 AFM 的时刻
    /// 是否晚于对话框打开时刻」这条判据会随 resync 漂移。
    pub fn mark_dialog_opened(&self, hwnd: u32) -> bool {
        if hwnd == 0 {
            return false;
        }
        let mut guard = self.dialog_opened_ts.lock().unwrap_or_else(|e| e.into_inner());
        let existed = guard.contains_key(&hwnd);
        guard.entry(hwnd).or_insert_with(now_unix_millis);
        !existed
    }

    /// v0.5.22 — 对话框首次被观察到的时刻（None = 没记录过）。
    pub fn dialog_opened_ts(&self, hwnd: u32) -> Option<u64> {
        let guard = self.dialog_opened_ts.lock().unwrap_or_else(|e| e.into_inner());
        guard.get(&hwnd).copied()
    }

    /// v0.5.22 — 闸门总入口：**两条独立**的放行路径。
    ///
    /// 路径 B（Listary 原义）：**对话框开着的时候用户去过 AFM** ⇒ 放行。
    ///   「用户去 AFM 拿了个路径 → 切回对话框 → 路径自动填上」是本功能的核心
    ///   场景，它不依赖任何 pid 身份 ⇒ 天然不受 helper 进程 / 任务栏采样影响。
    ///   边界（真机 2026-11-09 事故）：AFM 的访问时刻必须**晚于**对话框的
    ///   打开时刻 —— 否则「22 分钟前访问过 AFM」就能授权后来弹出的对话框。
    ///
    /// 路径 A（交接链身份）：从 AFM 切进来的那个应用（允许 helper 子进程）
    ///   的对话框 ⇒ 放行。真机 2026-11-18：接收方 `UGREEN NAS.exe`(15204)
    ///   派生出的对话框宿主进程(619924)必须被认成同一个应用。
    pub fn afm_gate_allows(&self, dialog_hwnd: u32, dialog_pid: u32) -> bool {
        if !Self::afm_gate_enabled() {
            return true;
        }
        let afm_ts = self.last_afm_foreground_ts();
        if let Some(open_ts) = self.dialog_opened_ts(dialog_hwnd) {
            if afm_ts > 0 && afm_ts >= open_ts {
                return true;
            }
        }
        self.afm_handoff_allows(dialog_pid)
    }

    /// v0.5.21 — 记住 AFM 的进程 id。
    ///
    /// 只有「窗口特征命中」（类名 `Tauri Window` + 标题前缀）时才更新 ——
    /// 这是唯一能把一个窗口**认成** AFM 的证据；其余时候保持原值。
    /// 前台命中即学；AFM 不在前台时由 `uia_event::find_afm_pid_by_enum()`
    /// 枚举顶层窗口补学。
    pub fn learn_afm_pid(&self, class_title_match: bool, hwnd_pid: u32) -> u32 {
        if class_title_match && hwnd_pid != 0 {
            self.afm_pid.store(hwnd_pid, Ordering::Relaxed);
            return hwnd_pid;
        }
        self.afm_pid.load(Ordering::Relaxed)
    }

    /// v0.5.21 — 当前已知的 AFM 进程 id（0 = 还没学到）。
    pub fn afm_pid(&self) -> u32 {
        self.afm_pid.load(Ordering::Relaxed)
    }

    /// v0.5.19 — 当前持有的交接授权（None = 没有）。
    pub fn afm_handoff(&self) -> Option<AfmHandoff> {
        *self.afm_handoff.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// v0.5.19 — 当前是否持有**有效**的交接授权（记录存在且未超 TTL）。
    ///
    /// 与 `afm_handoff_allows` 的 TTL 判定同一口径：`/health` 报
    /// `afm_visit_latched=true` 时，闸门才不会因为超时而拒绝写回。
    /// 否则「记录还在但早过期」会被误读成「授权还活着」。
    pub fn afm_handoff_is_fresh(&self) -> bool {
        let Some(handoff) = self.afm_handoff() else {
            return false;
        };
        now_unix_millis().saturating_sub(handoff.ts_ms) <= Self::afm_handoff_ttl_ms()
    }

    /// v0.5.19 — 闸门：这次弹窗写回能不能放行？
    ///
    /// 三条同时成立才放行：
    ///   1. 发生过 AFM -> X 的交接；
    ///   2. 交接距今不超过 TTL（默认 120 秒）；
    ///   3. 弹窗所属进程 == 接手 AFM 的那个应用。
    ///
    /// v0.5.20 — **pid 未知 ⇒ 不放行**（fail-closed）。
    ///
    /// 旧实现是「`handoff.pid == 0 || dialog_pid == 0` ⇒ 放行」：只要 pid
    /// 取不到，就退化成「TTL 内放行任何弹窗」。这等于把闸门第 3 条整个
    /// 关掉，只剩一个时间窗 —— 一条陈旧授权在 TTL 内就能写回真实路径，
    /// 正是本闸门要堵的洞（2026-11-09 真机事故的同类形态）。
    ///
    /// 现在两个 pid 任何一个读不到都拒绝：
    ///   * `dialog_pid == 0`：弹窗窗口已失效 / 取不到属主，写进去也必然失败；
    ///   * `handoff.pid == 0`：交接时没读到接手应用，无法核对「是不是直接
    ///     从 AFM 切过来的那个应用」。
    /// 两条都是「无法证明 ⇒ 不写」，而不是「无法证明 ⇒ 都写」。
    ///
    /// 逃生阀 `FOCUS_SYNC_AFM_GATE=0|false|off` 命中时恒真（回到 v0.5.14）。
    pub fn afm_handoff_allows(&self, dialog_pid: u32) -> bool {
        if !Self::afm_gate_enabled() {
            return true;
        }
        let Some(handoff) = self.afm_handoff() else {
            return false;
        };
        if now_unix_millis().saturating_sub(handoff.ts_ms) > Self::afm_handoff_ttl_ms() {
            return false;
        }
        // v0.5.20 — pid 未知 ⇒ 不放行（不能只靠 TTL 放行）。
        if handoff.pid == 0 || dialog_pid == 0 {
            return false;
        }
        // v0.5.21 — 交接 pid == AFM 自己的进程 ⇒ 这条交接来自**假下降沿**
        // （用户根本没离开 AFM）⇒ 不放行。
        //
        // 真机证据（2026-11-17）：AFM 的辅助窗口（类名
        // `com.sigma-file-manager.app-sic`）持有前台时旧判据认不出 AFM，于是
        // 记下 `handoff recorded: pid=72772` —— 72772 就是 AFM 的 pid ——
        // 之后所有真实对话框的 pid 都对不上 ⇒ 写回全线被拒。
        //
        // 这是**兜底**：v0.5.21 的进程身份判据已从源头消掉假下降沿；这一条
        // 保证「万一又记错」也不会把 AFM 自己当成「接手应用」。
        let afm_pid = self.afm_pid.load(Ordering::Relaxed);
        if afm_pid != 0 && handoff.pid == afm_pid {
            return false;
        }
        // v0.5.22 — 「同一个应用」判据放宽到进程亲缘关系。
        //
        // 真机 2026-11-18：接收方 = `UGREEN NAS.exe`(pid=15204)，而文件对话框
        // 的属主进程 = 619924 —— 那是该应用派生出来的**对话框宿主进程**。
        // 精确 pid 相等会把这次**合法**写回误拒（`dialog_pid=619924
        // handoff_pid=15204`）。
        //
        // 判据：两者相同，或其中一个是另一个的祖先/后代进程 ⇒ 同一个应用。
        handoff.pid == dialog_pid
            || crate::uia_event::same_application_process(handoff.pid, dialog_pid)
    }

    /// v0.5.19 — 交接授权的有效期（毫秒）。
    ///
    /// 环境变量 `FOCUS_SYNC_AFM_HANDOFF_TTL_MS` 可调，默认 120 秒。
    /// 「直接从 AFM 切换到弹窗」在时间上就该是近的；22 分钟前的访问
    /// （真机 2026-11-09 的事故）必须判为过期。
    pub fn afm_handoff_ttl_ms() -> u64 {
        static TTL: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
        *TTL.get_or_init(|| {
            std::env::var("FOCUS_SYNC_AFM_HANDOFF_TTL_MS")
                .ok()
                .and_then(|v| v.trim().parse::<u64>().ok())
                .unwrap_or(120_000)
        })
    }

    /// v0.5.7 F-1 — publish the current foreground dialog HWND.
    /// Only the monitor thread (which pumps Windows messages) may call this.
    /// `0` means "no foreground file dialog right now".
    pub fn set_foreground_dialog(&self, hwnd: u32) {
        self.foreground_dialog_hwnd
            .store(hwnd as i64, Ordering::Relaxed);
    }

    /// Enter a write-back section; the monitor will not re-fire until the
    /// returned guard is dropped.
    pub fn begin_sync(&self) -> SyncGuard<'_> {
        self.sync_in_progress.store(true, Ordering::SeqCst);
        SyncGuard { state: self }
    }

    pub fn sync_in_progress(&self) -> bool {
        self.sync_in_progress.load(Ordering::SeqCst)
    }

    /// v0.5.7 G-1 — what we last wrote into this dialog (its `last_known_path`).
    ///
    /// This field was documented as the dedup key for re-opened dialogs but was
    /// only ever WRITTEN, never READ, so the round-18 monitor dedup degraded
    /// into "suppress every repeat Opened for a known hwnd". A known dialog
    /// could therefore never re-sync after the user returned to it
    /// (2026-10-07 21:14-21:21 incident).
    ///
    /// Returns None when the dialog is not in the registry.
    pub fn last_written_path(&self, hwnd: u32) -> Option<String> {
        let reg = self.registry.lock().unwrap_or_else(|e| e.into_inner());
        reg.get(&hwnd).map(|d| d.last_known_path.clone())
    }

    /// v0.5.7 G-1 — read the foreground dialog HWND published by the monitor.
    /// Every other component must use THIS rather than calling
    /// `GetForegroundWindow()` itself, which answers per-calling-thread and is
    /// unreliable from the HTTP thread (see the field docs for the incident).
    pub fn foreground_dialog(&self) -> u32 {
        let v = self.foreground_dialog_hwnd.load(Ordering::Relaxed);
        if v <= 0 {
            0
        } else {
            v as u32
        }
    }

    /// focus-18: atomically "mark + report whether first time". Returns true if
    /// `hwnd` has never been committed (this caller MAY commit), returns false
    /// if already committed (skip commit). HashSet::insert returns bool:
    /// `true` = newly inserted, `false` = already present.
    pub fn try_mark_committed(&self, hwnd: u32) -> bool {
        let mut set = self.committed_hwnds.lock().unwrap_or_else(|e| e.into_inner());
        set.insert(hwnd)
    }

    /// focus-18: called from DialogEvent::Closed. Clears the commit mark for
    /// `hwnd`, allowing the next dialog (or re-spawn of the same hwnd) to commit.
    pub fn clear_committed(&self, hwnd: u32) {
        let mut set = self.committed_hwnds.lock().unwrap_or_else(|e| e.into_inner());
        set.remove(&hwnd);
    }

    /// v0.5.23 — 归还「这一次尝试」占用的 commit 槽。
    ///
    /// `try_mark_committed` 是**先占槽、后写入**（写入要花 ~300ms，占着槽
    /// 才能挡住第二个推送再占一次）。但如果这次写入**失败**，这个单次
    /// 防双击槽就被白白烧掉了：同一个 hwnd 之后再也不会拿到
    /// `should_commit == true` ⇒ Auto-Confirm 永远不再触发 —— 闩变成了
    /// 永久闩。失败时必须把槽还回去。
    ///
    /// 与 [`clear_committed`] 的区别：`clear_committed` 的语义是「这个
    /// 对话框关了」（DialogEvent::Closed）⇒ 下一个对话框可以提交；
    /// `unmark_committed` 的语义是「**这次**提交失败了」⇒ 同一次对话框
    /// 还可以再提交一次。两者对集合的作用相同（remove），但调用时机与
    /// 含义不同，分开命名才不会把「失败重试」和「弹窗关闭」混成一谈。
    pub fn unmark_committed(&self, hwnd: u32) {
        let mut set = self.committed_hwnds.lock().unwrap_or_else(|e| e.into_inner());
        set.remove(&hwnd);
    }

    /// v0.5.21 — 存入「AFM 此刻指向的路径」。
    ///
    /// **形状守卫**：不是本地路径形状（`\locations`、`sfm://...`、空串……）
    /// 一律**存空串**，不污染 `current_path`。
    ///
    /// 为什么存空而不是原样存：
    ///   * 空串 ⇒ `should_write_back_when_path_unknown` 直接返回 false
    ///     （current 为空 ⇒ 没有可写的目标）⇒ 虚拟位置期间**不写任何路径**；
    ///   * 空串 ⇒ `is_writable_local_path("")` 为 false ⇒ 写回侧落在
    ///     `skip_non_local_path` 分支 ⇒ 日志里的原因也准。
    /// 扩展下一次推合法路径时 `path_unchanged` 不成立 ⇒ 正常恢复。
    pub fn set_current_path(&self, p: String) {
        let p = if is_local_path_shape(&p) { p } else { String::new() };
        let mut guard = self.current_path.lock().unwrap_or_else(|e| e.into_inner());
        *guard = p;
        // Track that we received a ping — used by /health.
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        self.last_ping_received_ts.store(ts, Ordering::Relaxed);
        // v0.5.5 (Mavis Round 21 B-1 + Round 23 N-1): defense in depth.
        // Always mark first_push_received when path is set, in case future
        // callers skip the explicit mark_first_push_received() call.
        // Idempotent: store(true) on an already-true flag is a no-op.
        self.first_push_received.store(true, Ordering::SeqCst);
    }

    /// v0.5.24 — **显式清空**「AFM 此刻指向的路径」。
    ///
    /// 语义：AFM 此刻**不在任何本地目录**（虚拟位置 / 网盘远程目录）。
    /// 扩展会显式发这个信号（`POST /set_path`，`path` 为空串）。
    ///
    /// 为什么必须显式清（扩展 v0.6.1 改动的直接后果）：
    /// 扩展从 v0.6.1 起**不再把虚拟路径推给侧车**（推了也写不进去 —— 侧车的
    /// 形状闸门只会挡下它，还白费一次 HTTP 往返）。副作用是侧车**收不到任何
    /// 信号** ⇒ `current_path` 一直留着上一次的本地路径 L1 ⇒ 用户在网盘/虚拟
    /// 位置打开「另存为」时，L1 被写进别人的对话框（旧行为是「什么都不写」）。
    ///
    /// 与 [`set_current_path`] 的区别：
    ///   * `set_current_path` 会顺带 `first_push_received = true`（防哨兵）；
    ///   * `clear_current_path` **不动** `first_push_received` —— 「扩展曾经推过
    ///     路径」是历史事实，不该被一次清空抹掉；真正决定「能不能写」的是
    ///     `current_path` 的形状闸门（空串 ⇒ `is_writable_local_path` 为 false
    ///     ⇒ Opened 写回落到 `opened_write_back_skip_non_local_path` 分支）。
    pub fn clear_current_path(&self) {
        let mut guard = self.current_path.lock().unwrap_or_else(|e| e.into_inner());
        *guard = String::new();
    }

    /// v0.5.5: idempotent mark of first push. Called from http_server
    /// set_path early-return branch (path_unchanged) where set_current_path
    /// is not invoked.
    pub fn mark_first_push_received(&self) {
        self.first_push_received.store(true, Ordering::SeqCst);
    }

    /// v0.5.5: query for opened_write_back guard and /health exposure.
    pub fn has_first_push_received(&self) -> bool {
        self.first_push_received.load(Ordering::SeqCst)
    }

    /// v0.5.5 (Mavis Round 25 N-4): single source of truth for "should we
    /// write state.current_path to this new dialog?". Production code in
    /// main.rs:352 and unit tests both call this function, so test
    /// changes if production logic changes.
    pub fn should_opened_write_back(&self, initial_path: &str, current_path: &str) -> bool {
        !initial_path.is_empty()
            && !paths_equal(initial_path, current_path)
            && self.has_first_push_received()
    }

    /// v0.5.16 — 读不到弹窗当前目录时的写回决策。
    ///
    /// 背景（2026-11-06 真机）：Edge 的「另存为」对话框停在本地化特殊文件夹
    /// 「下载」时，地址栏 Name 是 `地址: 下载`（显示名，不是路径形状）⇒
    /// reader 解析失败 ⇒ `initial_path` 为空 ⇒ `should_opened_write_back`
    /// 恒假 ⇒ 写回被**静默跳过**（连一行日志都没有）。用户看到的现象就是
    /// 「从 AFM 切到下载弹窗，路径没有切换」。
    ///
    /// 新语义：**读不到 ≠ 不该写**。读不到时我们无法证明弹窗已经停在目标路
    /// 径上，于是以「这个 hwnd 上一次写入/登记的路径」作为去重键：
    ///   * 键为空（还没写过）⇒ 写；
    ///   * 键 == current（这次访问已经写过了）⇒ 不重复写（v0.5.13 防闪烁不变
    ///     量不变）；
    ///   * 键 != current（AFM 路径变了）⇒ 写。
    pub fn should_write_back_when_path_unknown(&self, hwnd: u32, current_path: &str) -> bool {
        if !self.has_first_push_received() || current_path.trim().is_empty() {
            return false;
        }
        // v0.6.3 — 同一个对话框、同一个目标路径只写一次（v0.5.13 防闪烁不变量）。
        //
        // 写记录是这一支的权威去重键：`/set_path` 即时写成功后由
        // `writer::write_path` 统一留下记录；旧实现只看 registry 的键，而
        // `/set_path` 从不更新它 ⇒ 同一个目标路径会被写两遍。
        if self.already_written(hwnd, current_path, None) {
            return false;
        }
        match self.last_written_path(hwnd) {
            Some(last) if !last.trim().is_empty() => !paths_equal(&last, current_path),
            _ => true,
        }
    }

    /// v0.6.3 — 记录一次**成功**的写入。
    ///
    /// 只有真正写进对话框才允许调用（`writer::write_path` 在成功分支统一调用）。
    /// 写失败绝不能留下记录：那会把 v0.5.7 G-1 的 resync 通道永久关掉
    /// （真机 2026-10-14 07:22：写失败后 52 秒零重试）。
    pub fn record_write(&self, hwnd: u32, path: &str, observed_dir: Option<String>) {
        let mut recs = self.write_records.lock().unwrap_or_else(|e| e.into_inner());
        recs.insert(
            hwnd,
            WriteRecord {
                path: path.to_string(),
                observed_dir,
            },
        );
    }

    /// v0.6.4 - the write gate of one dialog (per-hwnd mutex).
    ///
    /// Both write paths (the HTTP `/set_path` immediate write and the
    /// `DialogEvent::Opened` write-back) must lock the SAME gate and re-run
    /// their dedup check AFTER taking it: the dedup key is only persisted
    /// after a successful write, so without the gate two overlapping writers
    /// both pass the check and the same path is written twice (real machine
    /// 2026-11-18, batch 20).
    ///
    /// The same hwnd always maps to the same gate (shared `Arc`).
    pub fn write_gate(&self, hwnd: u32) -> Arc<Mutex<()>> {
        let mut gates = self.write_gates.lock().unwrap_or_else(|e| e.into_inner());
        gates
            .entry(hwnd)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// v0.6.3 — 取某个对话框最后一次成功写入的记录。
    pub fn write_record(&self, hwnd: u32) -> Option<WriteRecord> {
        let recs = self.write_records.lock().unwrap_or_else(|e| e.into_inner());
        recs.get(&hwnd).cloned()
    }

    /// v0.6.3 — 这个对话框是否已经持有 `path`（归一化比较）。
    ///
    /// 归一化是关键：`/set_path` 收到的路径可能来自扩展的 `normalizePath`
    /// （正斜杠），而写记录里是 AFM 推送的反斜杠形式；旧实现用**逐字节相等**
    /// 比较，同一个目录会被判成两个路径 ⇒ 同一个目标写两遍。
    pub fn dialog_holds_path(&self, hwnd: u32, path: &str) -> bool {
        if path.trim().is_empty() {
            return false;
        }
        if let Some(rec) = self.write_record(hwnd) {
            if paths_equal(&rec.path, path) {
                return true;
            }
        }
        self.last_written_path(hwnd)
            .as_deref()
            .map(|last| paths_equal(last, path))
            .unwrap_or(false)
    }

    /// v0.6.3 — 这个对话框是否**已经写进过** `current_path`，且此后没有被挪走。
    ///
    /// `observed_now` = 这一次读到的对话框当前目录（读不到就是 None）。
    /// 写记录里的 `observed_dir` 是写入瞬间的基线：两者相等 ⇒ 对话框没动过
    /// ⇒ 它仍然是我们写进去的那个路径 ⇒ 不重复写。
    pub fn already_written(&self, hwnd: u32, current_path: &str, observed_now: Option<&str>) -> bool {
        let Some(rec) = self.write_record(hwnd) else {
            return false;
        };
        if !paths_equal(&rec.path, current_path) {
            return false;
        }
        match (rec.observed_dir.as_deref(), observed_now) {
            (Some(written_at), Some(now)) => paths_equal(written_at, now),
            // 读不到基线或读不到当前目录 ⇒ 无法证明对话框被挪走 ⇒ 按「没挪走」处理。
            _ => true,
        }
    }

    /// v0.6.3 — 写回决策的唯一入口（`DialogEvent::Opened` 用）。
    ///
    /// 条件互相独立，缺一个就不写：
    ///   1. 这个对话框没有已经写过同一个目标（v0.6.3 双写去重）；
    ///   2. 路径确实和弹窗当前状态不同（`should_opened_write_back` 的老口径）；
    ///   3. 已经收到过扩展的第一次推送（哨兵）。
    pub fn should_write_back(&self, hwnd: u32, initial_path: &str, current_path: &str) -> bool {
        if !self.has_first_push_received() {
            return false;
        }
        if initial_path.trim().is_empty() {
            return self.should_write_back_when_path_unknown(hwnd, current_path);
        }
        self.should_opened_write_back(initial_path, current_path)
            && !self.already_written(hwnd, current_path, Some(initial_path))
    }

    /// v0.5.15 — 逃生阀：`FOCUS_SYNC_AFM_GATE` 显式关闭 AFM 访问闩。
    ///
    /// 只读一次进程环境（`OnceLock`），因为环境变量在运行期不会变，而
    /// `/health` 与 monitor 每 tick 都要问这个问题。
    /// 认 `0` / `false` / `off`（大小写不敏感）；其他值（含未设置）都是启用。
    pub fn afm_gate_enabled() -> bool {
        static DISABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        !*DISABLED.get_or_init(|| match std::env::var("FOCUS_SYNC_AFM_GATE") {
            Err(_) => false,
            Ok(v) => {
                let v = v.trim().to_ascii_lowercase();
                v == "0" || v == "false" || v == "off"
            }
        })
    }

    /// v0.5.8 — record that the extension just pinged `/ext_alive`.
    /// Called from `http_server::handle_ext_alive`; the ONLY writer.
    ///
    /// Deliberately NOT stamped by `/health`: that endpoint is shared with
    /// diagnostic tooling, so a stamp there would mean "someone read a status
    /// page", not "Sigma FM's extension is running".
    pub fn mark_extension_ping(&self) {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        self.last_health_poll_ts.store(ts, Ordering::Relaxed);
    }

    /// v0.5.8 — raw value of the last extension ping (0 if never seen).
    pub fn last_extension_ping_ts(&self) -> u64 {
        self.last_health_poll_ts.load(Ordering::Relaxed)
    }

    /// v0.5.8 — is a fresh extension ping within `window_ms`?
    ///
    /// Returns false when we have NEVER been pinged (0), which is the state a
    /// freshly started sidecar is in until the extension's first ping lands
    /// (~15s). That is intentional: a sidecar that has never heard from the
    /// extension has no path worth injecting.
    pub fn extension_alive(&self, window_ms: u64) -> bool {
        let last = self.last_health_poll_ts.load(Ordering::Relaxed);
        if last == 0 {
            return false;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        now.saturating_sub(last) <= window_ms
    }

    pub fn get_current_path(&self) -> String {        let guard = self.current_path.lock().unwrap_or_else(|e| e.into_inner());
        guard.clone()
    }

    pub fn register_dialog(&self, info: DialogInfo) {
        let mut reg = self.registry.lock().unwrap_or_else(|e| e.into_inner());
        reg.insert(info.hwnd, info);
        // Track dialog event for /health.
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        self.last_dialog_event_ts.store(ts, Ordering::Relaxed);
    }

    pub fn remove_dialog(&self, hwnd: u32) -> Option<DialogInfo> {
        let mut reg = self.registry.lock().unwrap_or_else(|e| e.into_inner());
        reg.remove(&hwnd)
    }

    pub fn active_dialogs(&self) -> Vec<DialogInfo> {
        let reg = self.registry.lock().unwrap_or_else(|e| e.into_inner());
        reg.values().cloned().collect()
    }

    /// UIA self-check health setter. Called periodically by main loop.
    pub fn set_uia_healthy(&self, healthy: bool) {
        self.uia_healthy.store(healthy, Ordering::Relaxed);
    }

    /// focus-19 (2026-10-02): bump the unsupported-dialog counter. Called by
    /// `uia_inject::inject_folder_path` when the dialog type is neither
    /// IFileDialog (COM) nor Chromium-41477 (UIA). The counter is
    /// monotonic — there is no decrement path. The extension polls this
    /// via `/health` to surface a notification to the user.
    pub fn inc_unsupported_dialog(&self) {
        self.unsupported_dialog_count
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Snapshot for /health endpoint.
    pub fn health_snapshot(&self) -> HealthSnapshot {
        HealthSnapshot {
            uptime_secs: self.startup_time.elapsed().as_secs(),
            last_ping_received_ts: self.last_ping_received_ts.load(Ordering::Relaxed),
            last_dialog_event_ts: self.last_dialog_event_ts.load(Ordering::Relaxed),
            active_dialogs: self.active_dialogs().len(),
            uia_healthy: self.uia_healthy.load(Ordering::Relaxed),
            session_id: self.session_id,
            unsupported_dialog_count: self.unsupported_dialog_count.load(Ordering::Relaxed),
            // v0.5.5 (Mavis Round 21 B-4): expose first_push_received for
            // extension UX (toolbar can show "⚠️ Sigma FM 未推送" if false).
            first_push_received: self.first_push_received.load(Ordering::SeqCst),
            // v0.5.19: 交接授权的状态可直接观测，「被闸门挡住」与「没接闸门」
            // 不再同名。
            afm_visit_latched: self.afm_handoff_is_fresh(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HealthSnapshot {
    pub uptime_secs: u64,
    pub last_ping_received_ts: u64,
    pub last_dialog_event_ts: u64,
    pub active_dialogs: usize,
    pub uia_healthy: bool,
    pub session_id: u32,
    /// focus-19: monotonic counter of dialogs that could not be navigated
    /// (WinUI Save As / unknown types). Surfaced via `/health` for the
    /// extension's user-facing notification.
    pub unsupported_dialog_count: u64,
    /// v0.5.5 (Mavis Round 21 B-4): boolean exposure for extension UX.
    /// Extension reads this to detect "sidecar is waiting for PUSH" and
    /// show user a "Sync Now?" notification.
    pub first_push_received: bool,
    /// v0.5.15: 使用者是否访问过 AFM 窗口且该授权尚未被消费（`FOCUS_SYNC_AFM_GATE`
    /// 启用时才有意义）。
    pub afm_visit_latched: bool,
}

/// v0.5.15 — AFM 访问闩的一次性授权语义。
///
/// 注意：不测 `FOCUS_SYNC_AFM_GATE` 逃生阀。`afm_gate_enabled()` 用
/// `OnceLock` 缓存进程环境，用例之间会互相污染（谁先跑谁定终身）。
/// 逃生阀留给真机 A/B 验收。
/// v0.5.16 — 本次 bug 的判别性用例组。
///
/// 真机症状（2026-11-06）：Edge「另存为」停在本地化特殊文件夹「下载」时，
/// 地址栏是 `地址: 下载`（显示名，不是路径）⇒ reader 读失败 ⇒ 旧实现把
/// 「读不到」当成「不该写」⇒ 写回被静默跳过 ⇒ 用户看到「从 AFM 切到下载
/// 弹窗，路径没有切换」。
#[cfg(test)]
mod v516_write_back_tests {
    use super::AppState;
    use super::DialogInfo;

    const HWND: u32 = 5_001;
    const AFM_PATH: &str = "F:\\soft\\00selfmade\\filemanager\\sigma-file-manager";

    fn state_with_push() -> AppState {
        let s = AppState::new(AFM_PATH.into(), 1);
        s.mark_first_push_received();
        s
    }

    fn register(state: &AppState, last: &str) {
        state.register_dialog(DialogInfo {
            hwnd: HWND,
            app: "#32770".into(),
            last_known_path: last.into(),
            write_strategy: None,
        });
    }

    #[test]
    fn unreadable_dialog_path_still_allows_the_write() {
        let state = state_with_push();
        assert!(
            state.should_write_back_when_path_unknown(HWND, AFM_PATH),
            "读不到目录时必须仍然允许写入，否则用户永远看不到路径切换"
        );
    }

    #[test]
    fn unreadable_dialog_path_does_not_write_twice_for_the_same_target() {
        let state = state_with_push();
        register(&state, AFM_PATH);
        assert!(
            !state.should_write_back_when_path_unknown(HWND, AFM_PATH),
            "已经写过同一个路径 ⇒ 不得重复写（v0.5.13 防闪烁不变量）"
        );
    }

    #[test]
    fn unreadable_dialog_path_writes_again_when_afm_path_changed() {
        let state = state_with_push();
        register(&state, "F:\\a");
        assert!(
            state.should_write_back_when_path_unknown(HWND, "F:\\b"),
            "AFM 路径变了 ⇒ 必须再写一次"
        );
    }

    #[test]
    fn unreadable_dialog_path_respects_the_first_push_sentinel() {
        let state = AppState::new(AFM_PATH.into(), 1);
        assert!(
            !state.should_write_back_when_path_unknown(HWND, AFM_PATH),
            "还没收到第一次推送时不得写入"
        );
    }

    #[test]
    fn slash_direction_does_not_make_the_same_path_look_different() {
        // reader 读回 `F:/dir`（正斜杠，shell 返回），AFM 推 `F:\dir`（反斜杠）。
        assert!(super::paths_equal("F:/dir", "F:\\dir"));
        assert!(super::paths_equal("F:/dir/", "F:\\dir"));
        assert!(super::paths_equal("f:/DIR", "F:\\dir"));
        assert!(!super::paths_equal("F:/dir", "F:\\other"));
    }

    #[test]
    fn opened_write_back_uses_normalized_comparison() {
        let state = state_with_push();
        assert!(
            !state.should_opened_write_back("F:/dir", "F:\\dir"),
            "同一个目录（只是斜杠方向不同）不得触发写回"
        );
        assert!(
            state.should_opened_write_back("F:/other", "F:\\dir"),
            "不同目录必须触发写回"
        );
    }
}

/// v0.6.3 —— 双写去重的判别性测试。
///
/// 背景：侧车内部有**两条**真正写对话框的路径 ——
///   1. `POST /set_path` 的即时写（`http_server.rs`）；
///   2. `DialogEvent::Opened` 的写回（`main.rs`）。
///
/// 旧实现两条路的去重口径不同（前者逐字节相等、后者归一化相等），而且
/// `/set_path` 写完**不留下任何记录** ⇒ 同一个对话框、同一个目标路径会被写
/// 两遍（Chromium ComboBox 的 `SetValue` 是追加语义 ⇒ 路径累加 / 地址栏跳动）。
#[cfg(test)]
mod v063_double_write_tests {
    use super::{paths_equal, AppState};

    const HWND: u32 = 0xA11CE;

    /// 一个「已经收到扩展第一次推送」的侧车状态（否则哨兵会挡住一切）。
    fn state() -> AppState {
        let s = AppState::new("E:\\Models".into(), 1);
        s.mark_first_push_received();
        s
    }

    #[test]
    fn a_second_write_of_the_same_target_is_refused() {
        let s = state();
        s.record_write(HWND, "E:\\Models", Some("C:\\tmp".into()));
        assert!(
            !s.should_write_back(HWND, "C:\\tmp", "E:\\Models"),
            "同一个对话框、同一个目标路径不得写第二遍"
        );
    }

    #[test]
    fn a_different_target_is_still_written() {
        let s = state();
        s.record_write(HWND, "E:\\Models", Some("C:\\tmp".into()));
        assert!(
            s.should_write_back(HWND, "C:\\tmp", "E:\\obsidian"),
            "AFM 换了目标路径 ⇒ 必须再写一次"
        );
    }

    #[test]
    fn a_dialog_the_user_moved_is_written_again() {
        let s = state();
        s.record_write(HWND, "E:\\Models", Some("C:\\tmp".into()));
        assert!(
            s.should_write_back(HWND, "D:\\elsewhere", "E:\\Models"),
            "用户把对话框挪到了别的目录 ⇒ 必须再写一次"
        );
    }

    #[test]
    fn the_set_path_dedup_is_normalized() {
        let s = state();
        s.record_write(HWND, "E:/Models", None);
        assert!(
            s.dialog_holds_path(HWND, "e:\\models"),
            "斜杠方向/大小写不同不得被当成新目标（否则同一个路径写两遍）"
        );
        assert!(
            !s.dialog_holds_path(HWND, "E:\\obsidian"),
            "不同目录不得被当成同一目标"
        );
    }

    #[test]
    fn the_unreadable_branch_dedups_against_the_write_record() {
        let s = state();
        s.record_write(HWND, "E:\\Models", None);
        assert!(
            !s.should_write_back_when_path_unknown(HWND, "E:\\Models"),
            "读不到对话框目录时也必须认「已经写过了」"
        );
        assert!(
            s.should_write_back_when_path_unknown(HWND, "E:\\obsidian"),
            "目标变了 ⇒ 仍然要写"
        );
    }

    #[test]
    fn a_failed_write_leaves_no_record() {
        // 没有记录 ⇒ 去重键为空 ⇒ 允许写（写失败必须保持可重试，v0.5.12 不变量）。
        let s = state();
        assert!(s.write_record(HWND).is_none());
        assert!(s.should_write_back(HWND, "C:\\tmp", "E:\\Models"));
    }

    #[test]
    fn the_sentinel_still_blocks_before_the_first_push() {
        let s = AppState::new("E:\\Models".into(), 1);
        s.record_write(HWND, "E:\\Models", Some("C:\\tmp".into()));
        assert!(
            !s.should_write_back(HWND, "C:\\tmp", "E:\\Models"),
            "还没收到第一次推送时不得写入"
        );
    }

    #[test]
    fn paths_equal_is_the_shared_normalizer() {
        assert!(paths_equal("E:/Models", "e:\\models"));
    }
}

#[cfg(test)]
mod afm_gate_tests {
    use super::{now_unix_millis, AfmHandoff, AppState};

    #[test]
    fn gate_starts_closed_so_no_sync_without_handoff() {
        // v0.5.19 判别性守卫:闸门本身必须处于「启用」态,否则闸门恒真、
        // 本模块其余断言全部变成平凡通过(2026-11-06 就踩过一次取反写反)。
        // 安全前提:本模块没有任何用例改写 FOCUS_SYNC_AFM_GATE。
        assert!(AppState::afm_gate_enabled(), "环境变量未设置时闸门应默认启用");
        let state = AppState::new("E:\\Models".into(), 1);
        assert!(!state.afm_handoff_allows(1234), "新侧车不该有同步授权");
    }

    #[test]
    fn handoff_to_the_dialog_app_authorizes_the_sync() {
        // 用户诉求的正例:从 AFM 切到「有弹窗的那个应用」=> 放行。
        let state = AppState::new("E:\\Models".into(), 1);
        state.set_afm_handoff(4242);
        assert!(
            state.afm_handoff_allows(4242),
            "弹窗属于接手 AFM 的那个应用 => 放行"
        );
    }

    #[test]
    fn dialog_of_another_app_is_refused() {
        // 「非直接从 AFM 切换到弹窗」:AFM 切给了 Edge,后来在 practilog 打开
        // 对话框 => 那个对话框不是接手 AFM 的应用 => 拒绝。
        let state = AppState::new("E:\\Models".into(), 1);
        state.set_afm_handoff(4242);
        assert!(!state.afm_handoff_allows(5555), "别的应用的弹窗必须拒绝");
    }

    #[test]
    fn stale_handoff_expires() {
        // 真机 2026-11-09:11:09 访问过 AFM,11:31 才在另一个程序打开对话框,
        // 中间隔了 22 分钟 —— 旧实现照样授权。交接必须有有效期。
        let state = AppState::new("E:\\Models".into(), 1);
        state.set_afm_handoff(4242);
        {
            let mut guard = state.afm_handoff.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some(AfmHandoff {
                pid: 4242,
                ts_ms: now_unix_millis().saturating_sub(10 * 60 * 1000),
            });
        }
        assert!(!state.afm_handoff_allows(4242), "过期交接必须拒绝");
    }

    #[test]
    fn returning_to_afm_clears_the_handoff() {
        // 用户回到 AFM 这一动作本身让旧交接作废。
        let state = AppState::new("E:\\Models".into(), 1);
        state.set_afm_handoff(4242);
        state.clear_afm_handoff();
        assert!(!state.afm_handoff_allows(4242), "回到 AFM 必须作废旧交接");
    }

    #[test]
    fn health_snapshot_exposes_the_handoff() {
        let state = AppState::new("C:\\".into(), 3);
        assert!(!state.health_snapshot().afm_visit_latched);
        state.set_afm_handoff(4242);
        assert!(state.health_snapshot().afm_visit_latched);
        state.clear_afm_handoff();
        assert!(!state.health_snapshot().afm_visit_latched);
    }

    #[test]
    fn health_snapshot_reports_stale_handoff_as_not_latched() {
        // 真机 2026-11-09：「记录还在」不等于「授权还活着」。
        // /health 必须与闸门同一口径，否则验收会拿过期授权当活授权。
        let state = AppState::new("E:\\Models".into(), 1);
        state.set_afm_handoff(4242);
        {
            let mut guard = state.afm_handoff.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some(AfmHandoff {
                pid: 4242,
                ts_ms: now_unix_millis().saturating_sub(10 * 60 * 1000),
            });
        }
        assert!(!state.health_snapshot().afm_visit_latched);
    }

    #[test]
    fn gate_is_orthogonal_to_the_first_push_sentinel() {
        // 闸门不该改变既有哨兵判定：两者各自独立成立。
        let state = AppState::new("E:\\Models".into(), 1);
        state.set_current_path("E:\\obsidian".into());
        assert!(state.should_opened_write_back("C:\\tmp", "E:\\obsidian"));
        assert!(!state.afm_handoff_allows(1234), "哨兵通过但没有交接授权");
        state.set_afm_handoff(1234);
        assert!(state.should_opened_write_back("C:\\tmp", "E:\\obsidian")
            && state.afm_handoff_allows(1234));
    }

    // ---- v0.5.20：pid 未知 ⇒ 不放行（fail-closed） ----
    //
    // 缺陷（旧实现）：`handoff.pid == 0 || dialog_pid == 0` ⇒ 直接放行。
    // 后果：只要 pid 取不到，闸门第 3 条「弹窗属于接手 AFM 的那个应用」就
    // 整个失效，退化成「TTL 内放行任何弹窗」——陈旧授权在 TTL 内即可写回
    // 真实路径，正是这个闸门要堵的洞。

    #[test]
    fn unknown_dialog_pid_does_not_authorize_the_sync() {
        // 弹窗窗口取不到属主进程（hwnd 已失效 / 读不到 pid）⇒ 不放行。
        // TTL 仍然新鲜也不足以放行。
        let state = AppState::new("E:\\Models".into(), 1);
        state.set_afm_handoff(4242);
        assert!(
            !state.afm_handoff_allows(0),
            "读不到弹窗所属进程时必须拒绝：不能只靠 TTL 放行"
        );
    }

    #[test]
    fn unknown_handoff_pid_does_not_authorize_the_sync() {
        // 交接时没读到接手应用的 pid ⇒ 没有可核对的授权 ⇒ 不放行。
        let state = AppState::new("E:\\Models".into(), 1);
        state.set_afm_handoff(0);
        assert!(
            !state.afm_handoff_allows(4242),
            "交接 pid 未知时必须拒绝：不能因为「有交接记录」就放行"
        );
    }

    #[test]
    fn a_handoff_without_a_known_pid_is_never_recorded() {
        // set_afm_handoff(0) 不产生授权记录 —— 一条 pid 未知的记录没有任何
        // 可核对的东西，只会变成 TTL 内的万能钥匙。
        let state = AppState::new("E:\\Models".into(), 1);
        state.set_afm_handoff(0);
        assert!(
            state.afm_handoff().is_none(),
            "pid 未知的交接不得被记录下来"
        );
        assert!(!state.afm_handoff_allows(4242));
        assert!(!state.health_snapshot().afm_visit_latched);
    }

    #[test]
    fn a_consumed_handoff_cannot_be_reused() {
        // 「消费即失效」：写成功后调用方会下弦（clear_afm_handoff），
        // 同一个授权不得再放行下一次写回。
        let state = AppState::new("E:\\Models".into(), 1);
        state.set_afm_handoff(4242);
        assert!(state.afm_handoff_allows(4242), "首次使用应当放行");
        state.clear_afm_handoff(); // 消费
        assert!(
            !state.afm_handoff_allows(4242),
            "授权被消费一次后必须失效，不能复用"
        );
        assert!(!state.health_snapshot().afm_visit_latched);
    }

    #[test]
    fn tightening_the_pid_rule_does_not_break_the_normal_flow() {
        // 收紧后仍必须放行正常流程：pid 已知且相等 + TTL 未过。
        let state = AppState::new("E:\\Models".into(), 1);
        state.set_afm_handoff(4242);
        assert!(
            state.afm_handoff_allows(4242),
            "正常流程（同一应用、TTL 内）必须照旧放行"
        );
        assert!(state.afm_handoff_is_fresh());
    }
}

/// v0.5.19 — 路径合法性：只有真实存在的本地目录才允许写进别人的对话框。
///
/// 真机事故（2026-11-09 11:31）：AFM 停在百度网盘远程目录时，扩展把
/// `sfm://remote/<id>/0000temp` 剥成伪路径 `\remote\<id>\0000temp`，
/// 侧车把它写进另一个程序的「打开」对话框 ⇒ Windows 弹「找不到」错误框
/// ⇒ 错误框又被当成新对话框 ⇒ 每 1~2 秒重试 ⇒ 关闭后仍反复报错。
/// v0.5.21 — 判别性测试：AFM 进程身份 + 交接闸门 + 路径形状守卫。
///
/// 根因（真机 2026-11-17，`spike.log.2026-11-17` + JSON 事件流 `spike.log`）：
/// 闸门的「下降沿」用「类名 == `Tauri Window` 且标题以 `Alpha File Manager`
/// 开头」识别 AFM，而装机版 AFM 进程另持有可见的辅助窗口
/// （`com.sigma-file-manager.app-sic`）⇒ 假下降沿 ⇒ 交接记录把 AFM 自己的
/// pid（72772）写进授权 ⇒ 真实对话框的 pid 永远匹配不上 ⇒ 全部写回被
/// `opened_write_back_skip_no_afm_visit` 拒绝（当天 656 次，0 次成功）。
#[cfg(test)]
mod v521_afm_identity_tests {
    use super::{is_local_path_shape, AppState};

    // ---- 闸门：交接 pid 不得是 AFM 自己的进程 ----

    #[test]
    fn handoff_recorded_on_the_afm_process_is_not_an_authorization() {
        // 真机那一条就是 `handoff recorded: pid=72772`（AFM 自己）。
        // 这条授权必须被拒：用户根本没离开 AFM。
        let state = AppState::new("E:\\Models".into(), 1);
        state.learn_afm_pid(true, 72772);
        state.set_afm_handoff(72772);
        assert!(
            !state.afm_handoff_allows(72772),
            "交接 pid == AFM 自己的 pid 时必须拒绝（假下降沿）"
        );
    }

    #[test]
    fn a_real_handoff_to_another_app_is_still_allowed() {
        // 收紧不得误伤正常流程：AFM -> 别的应用 -> 该应用的对话框。
        let state = AppState::new("E:\\Models".into(), 1);
        state.learn_afm_pid(true, 72772);
        state.set_afm_handoff(4242);
        assert!(
            state.afm_handoff_allows(4242),
            "正常流程（切到别的应用、该应用的对话框）必须照旧放行"
        );
    }

    #[test]
    fn a_dialog_from_an_unrelated_app_is_still_rejected() {
        let state = AppState::new("E:\\Models".into(), 1);
        state.learn_afm_pid(true, 72772);
        state.set_afm_handoff(4242);
        assert!(
            !state.afm_handoff_allows(9999),
            "没有从 AFM 切过去的那个应用，一律拒绝"
        );
    }

    // ---- AFM 进程身份的学习 ----

    #[test]
    fn afm_pid_is_learned_only_from_a_class_title_match() {
        let state = AppState::new("E:\\Models".into(), 1);
        assert_eq!(state.learn_afm_pid(true, 72772), 72772);
        assert_eq!(state.afm_pid(), 72772);
        // 没有特征命中 ⇒ 不更新（不能被任意窗口的 pid 污染）
        assert_eq!(state.learn_afm_pid(false, 999), 72772);
        assert_eq!(state.afm_pid(), 72772);
    }

    // ---- current_path 的形状守卫 ----

    #[test]
    fn virtual_paths_never_reach_current_path() {
        let state = AppState::new("E:\\Models".into(), 1);
        state.set_current_path("sfm://locations".into());
        assert_eq!(state.get_current_path(), "", "虚拟路径不得进入 current_path");
        state.set_current_path("\\locations".into());
        assert_eq!(state.get_current_path(), "");
        state.set_current_path("\\remote\\abc\\0000temp".into());
        assert_eq!(state.get_current_path(), "");
    }

    #[test]
    fn real_local_paths_are_still_stored() {
        let state = AppState::new("E:\\Models".into(), 1);
        state.set_current_path("D:\\new".into());
        assert_eq!(state.get_current_path(), "D:\\new");
        state.set_current_path("\\\\server\\share".into());
        assert_eq!(state.get_current_path(), "\\\\server\\share");
    }

    #[test]
    fn local_path_shape_accepts_only_drive_and_unc() {
        assert!(is_local_path_shape("D:\\new"));
        assert!(is_local_path_shape("D:/new"));
        assert!(is_local_path_shape("\\\\server\\share"));
        assert!(!is_local_path_shape("C:"));
        assert!(!is_local_path_shape("\\locations"));
        assert!(!is_local_path_shape("sfm://locations"));
        assert!(!is_local_path_shape(""));
    }
}

#[cfg(test)]
mod v519_path_validity_tests {
    use super::is_writable_local_path;

    #[test]
    fn virtual_paths_are_never_writable() {
        assert!(!is_writable_local_path(
            "\\remote\\0f0696babf4f42d1aef22c61aa89b0a9\\0000temp"
        ));
        assert!(!is_writable_local_path(
            "sfm://remote/0f0696babf4f42d1aef22c61aa89b0a9/0000temp"
        ));
        assert!(!is_writable_local_path("\\locations"));
        assert!(!is_writable_local_path("C:"), "裸盘符不是目录");
        assert!(!is_writable_local_path(""), "空路径不可写");
    }

    #[test]
    fn existing_local_dir_is_writable_and_missing_one_is_not() {
        let tmp = std::env::temp_dir();
        assert!(
            is_writable_local_path(tmp.to_str().unwrap()),
            "临时目录必须可写：{}",
            tmp.display()
        );
        let missing = tmp.join("afm-focus-sync-definitely-missing-dir");
        assert!(
            !is_writable_local_path(missing.to_str().unwrap()),
            "不存在的目录不能写进别人的对话框"
        );
    }

    #[test]
    fn unc_shape_is_accepted_without_touching_the_network() {
        // 网络盘可能离线，存在性探测会阻塞在 IO 上 ⇒ UNC 只查形状。
        assert!(is_writable_local_path("\\\\server\\share"));
    }
}

#[cfg(test)]
mod g2_sync_guard_tests {
    use super::{AppState, DialogInfo};

    fn seeded() -> AppState {
        let s = AppState::new("E:\\Models".into(), 1);
        s.register_dialog(DialogInfo {
            hwnd: 722614,
            app: "#32770".into(),
            last_known_path: "11.politics".into(),
            write_strategy: None,
        });
        s
    }

    /// last_written_path must reflect the registry — this is the G-1 dedup
    /// key that was previously written but never read.
    #[test]
    fn last_written_path_reflects_registry() {
        let s = seeded();
        assert_eq!(s.last_written_path(722614).as_deref(), Some("11.politics"));
        assert_eq!(s.last_written_path(999), None);
    }

    /// begin_sync sets the flag; dropping the guard clears it (RAII), so a
    /// write that returns early cannot wedge re-sync permanently.
    #[test]
    fn begin_sync_clears_on_drop() {
        let s = seeded();
        assert!(!s.sync_in_progress());
        {
            let _g = s.begin_sync();
            assert!(s.sync_in_progress());
        }
        assert!(!s.sync_in_progress());
    }

    /// REGRESSION for the 2026-10-07 21:37 infinite loop (1364 resyncs of the
    /// SAME path). While a write-back is in flight the monitor must be held
    /// off; otherwise it re-fires `Opened` every 8ms for the ~300ms the write
    /// takes, and each re-fire re-clobbers the dedup key.
    #[test]
    fn in_progress_write_blocks_monitor_refire() {
        let s = seeded();
        // Before the write: the key still differs from pending -> resync wanted.
        let key_before = s.last_written_path(722614);
        assert_ne!(key_before.as_deref(), Some(s.get_current_path().as_str()));
        // While the write is running the monitor is gated off...
        {
            let _g = s.begin_sync();
            assert!(s.sync_in_progress(), "monitor must be held off during write");
        }
        // ...and after it completes the key is updated so no resync is wanted.
        s.register_dialog(DialogInfo {
            hwnd: 722614,
            app: "#32770".into(),
            last_known_path: s.get_current_path(),
            write_strategy: None,
        });
        let key_after = s.last_written_path(722614);
        assert_eq!(key_after.as_deref(), Some(s.get_current_path().as_str()));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn register_and_remove() {
        let state = AppState::new("C:\\".into(), 1);
        state.register_dialog(DialogInfo {
            hwnd: 1,
            app: "chrome".into(),
            last_known_path: "C:\\Users\\foo".into(),
            write_strategy: None,
        });
        assert_eq!(state.active_dialogs().len(), 1);

        let removed = state.remove_dialog(1).unwrap();
        assert_eq!(removed.app, "chrome");
        assert_eq!(state.active_dialogs().len(), 0);
    }

    #[test]
    fn current_path_updates() {
        let state = AppState::new("C:\\".into(), 1);
        assert_eq!(state.get_current_path(), "C:\\");
        state.set_current_path("D:\\new".into());
        assert_eq!(state.get_current_path(), "D:\\new");
    }

    /// v0.5.24 — 清空语义：只清 `current_path`，**不动** `first_push_received`。
    ///
    /// 为什么不动 flag：「扩展曾经推过路径」是历史事实；一次清空把它抹掉会让
    /// 侧车重启后的补推判据（`first_push_received == false`）被误触发。
    #[test]
    fn clear_current_path_empties_path_but_keeps_first_push_flag() {
        let state = AppState::new("C:\\".into(), 1);
        state.set_current_path("D:\\stale".into());
        assert!(state.has_first_push_received());
        state.clear_current_path();
        assert_eq!(state.get_current_path(), "");
        assert!(
            state.has_first_push_received(),
            "清空不是一次「推送」，历史事实不该被抹掉"
        );
    }

    /// v0.5.24 — 清空之后，形状闸门必须让写回通道**拒绝**写入。
    ///
    /// 这条是 R1 回归的判别点：清空前 `current_path` = 陈旧的本地路径 L1 ⇒
    /// `is_writable_local_path(L1)` 为 true ⇒ L1 会被写进别人的「另存为」。
    ///
    /// 注意：`is_writable_local_path` 要求目录**真实存在**
    /// （`Path::is_dir()`），所以这里用盘符根 `C:\`，不能用编造的 `D:\stale`。
    #[test]
    fn cleared_current_path_is_not_writable() {
        let state = AppState::new("C:\\".into(), 1);
        state.set_current_path("C:\\".into());
        assert!(
            crate::state::is_writable_local_path(&state.get_current_path()),
            "前提：盘符根是真实存在、可写的本地目录"
        );
        state.clear_current_path();
        assert!(
            !crate::state::is_writable_local_path(&state.get_current_path()),
            "清空后必须写不进任何对话框"
        );
    }

    #[test]
    fn health_snapshot_initial() {
        let state = AppState::new("C:\\".into(), 2);
        let snap = state.health_snapshot();
        assert_eq!(snap.session_id, 2);
        assert_eq!(snap.active_dialogs, 0);
        assert_eq!(snap.last_ping_received_ts, 0);
        assert_eq!(snap.last_dialog_event_ts, 0);
        assert!(snap.uia_healthy); // default true
        assert!(snap.uptime_secs < 5);
    }

    #[test]
    fn set_current_path_records_ping_ts() {
        let state = AppState::new("C:\\".into(), 1);
        state.set_current_path("D:\\".into());
        let snap = state.health_snapshot();
        assert!(snap.last_ping_received_ts > 0, "ping ts should be recorded");
    }

    #[test]
    fn register_dialog_records_event_ts() {
        let state = AppState::new("C:\\".into(), 1);
        state.register_dialog(DialogInfo {
            hwnd: 1,
            app: "chrome".into(),
            last_known_path: "".into(),
            write_strategy: None,
        });
        let snap = state.health_snapshot();
        assert!(snap.last_dialog_event_ts > 0);
        assert_eq!(snap.active_dialogs, 1);
    }

    #[test]
    fn set_uia_healthy_toggles() {
        let state = AppState::new("C:\\".into(), 1);
        assert!(state.health_snapshot().uia_healthy);
        state.set_uia_healthy(false);
        assert!(!state.health_snapshot().uia_healthy);
        state.set_uia_healthy(true);
        assert!(state.health_snapshot().uia_healthy);
    }

    #[test]
    fn uptime_advances() {
        let state = AppState::new("C:\\".into(), 1);
        let s1 = state.health_snapshot().uptime_secs;
        std::thread::sleep(Duration::from_millis(50));
        let s2 = state.health_snapshot().uptime_secs;
        assert!(s2 >= s1);
    }

    // focus-18: per-dialog single-shot dedup.
    #[test]
    fn try_mark_committed_returns_true_first_time() {
        let state = AppState::new("C:\\".into(), 1);
        assert!(state.try_mark_committed(12345));
        assert!(state.try_mark_committed(67890)); // different hwnd also first-time
    }

    #[test]
    fn try_mark_committed_returns_false_subsequent_calls() {
        let state = AppState::new("C:\\".into(), 1);
        assert!(state.try_mark_committed(12345));
        assert!(!state.try_mark_committed(12345));
        assert!(!state.try_mark_committed(12345));
    }

    #[test]
    fn clear_committed_resets_state() {
        let state = AppState::new("C:\\".into(), 1);
        assert!(state.try_mark_committed(12345));
        assert!(!state.try_mark_committed(12345));
        state.clear_committed(12345);
        assert!(state.try_mark_committed(12345)); // reset -> can commit again
    }

    /// v0.5.23 — 失败的提交必须把槽还回来。
    ///
    /// `try_mark_committed` 是先占槽、后写入；写入失败后槽若留着，这个
    /// 对话框就再也不能提交了（focus-18 的单次防双击闩变成永久闩 ⇒
    /// Auto-Confirm 永久失效）。`unmark_committed` 就是失败路径上的归还
    /// 动作：归还之后，同一个 hwnd 的下一次提交仍然被允许。
    #[test]
    fn unmark_committed_releases_the_slot_so_a_later_commit_is_allowed() {
        let state = AppState::new("C:\\".into(), 1);
        assert!(state.try_mark_committed(12345));
        assert!(!state.try_mark_committed(12345));
        // 这次提交失败了 ⇒ 把槽还回去。
        state.unmark_committed(12345);
        assert!(
            state.try_mark_committed(12345),
            "失败的提交不得烧掉槽：同一个 hwnd 必须还能再提交一次"
        );
    }

    // focus-19 (2026-10-02): unsupported_dialog_count is monotonic and
    // surfaced via /health for the extension's notification path.
    #[test]
    fn unsupported_count_starts_at_zero() {
        let state = AppState::new("C:\\".into(), 1);
        assert_eq!(state.health_snapshot().unsupported_dialog_count, 0);
    }

    #[test]
    fn inc_unsupported_dialog_is_monotonic() {
        let state = AppState::new("C:\\".into(), 1);
        state.inc_unsupported_dialog();
        state.inc_unsupported_dialog();
        state.inc_unsupported_dialog();
        assert_eq!(state.health_snapshot().unsupported_dialog_count, 3);
        // No decrement path: 100 inc's → count == 100.
        for _ in 0..97 {
            state.inc_unsupported_dialog();
        }
        assert_eq!(state.health_snapshot().unsupported_dialog_count, 100);
    }

    // ---- v0.6.4 write gate tests ------------------------------------------

    #[test]
    fn the_same_dialog_gets_the_same_write_gate() {
        let s = AppState::new("E:/Models".into(), 1);
        let g1 = s.write_gate(1234);
        let g2 = s.write_gate(1234);
        assert!(
            std::sync::Arc::ptr_eq(&g1, &g2),
            "the same hwnd must map to the same write gate"
        );
        let other = s.write_gate(4321);
        assert!(
            !std::sync::Arc::ptr_eq(&g1, &other),
            "different dialogs must not share a write gate"
        );
    }

    #[test]
    fn the_write_gate_makes_the_second_writer_wait() {
        use std::sync::mpsc;
        use std::thread;
        use std::time::Duration;

        let s = AppState::new("E:/Models".into(), 1);
        let g1 = s.write_gate(7777);
        let g2 = s.write_gate(7777);
        let (tx, rx) = mpsc::channel();
        let h1 = thread::spawn(move || {
            let _guard = g1.lock().unwrap_or_else(|e| e.into_inner());
            tx.send("locked").unwrap();
            thread::sleep(Duration::from_millis(250));
        });
        rx.recv().unwrap();
        let h2 = thread::spawn(move || {
            let _guard = g2.lock().unwrap_or_else(|e| e.into_inner());
        });
        thread::sleep(Duration::from_millis(80));
        assert!(
            !h2.is_finished(),
            "the second writer must wait for the first writer to release the gate"
        );
        h1.join().unwrap();
        h2.join().unwrap();
    }

    #[test]
    fn the_dedup_check_inside_the_gate_sees_the_first_writers_record() {
        let s = AppState::new("E:/Models".into(), 1);
        const HWND: u32 = 8888;
        let gate = s.write_gate(HWND);
        {
            let _guard = gate.lock().unwrap_or_else(|e| e.into_inner());
            assert!(
                !s.dialog_holds_path(HWND, "E:/Models"),
                "nothing has been written yet"
            );
            s.record_write(HWND, "E:/Models", None);
        }
        {
            let _guard = gate.lock().unwrap_or_else(|e| e.into_inner());
            assert!(
                s.dialog_holds_path(HWND, "e:/models"),
                "the dedup check re-run inside the gate must see the record left by the first writer"
            );
        }
    }

}