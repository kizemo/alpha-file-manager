// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! 全局搜索索引的**递归**文件 watching。
//!
//! 与 `dir_watcher.rs` 的分工（那边是文件浏览器刷新用的）：
//!
//! * `dir_watcher.rs` 是 **NonRecursive**、只监听一层、并且通过
//!   `tauri::AppHandle` 把事件发回前端；
//! * 这里监听的是**盘根**（`C:/`），必须**递归**，事件只进 Rust 侧，
//!   用来驱动索引的增量更新。
//!
//! 因此这里**不碰 tauri** —— `cargo test --lib` 的测试二进制只要链接到
//! `tauri::AppHandle` / `tauri::async_runtime::*` 就会在加载阶段以
//! `STATUS_ENTRYPOINT_NOT_FOUND (0xC0000139)` 崩掉（见 AGENTS.md）。
//!
//! 事件来自 `notify` 的后端线程，回调里**不能阻塞**（阻塞会拖垮整个
//! 通知管线），所以用一个有界通道把事件转交给自己的线程：
//!
//! * 通道满 ⇒ 消费不过来了 ⇒ 丢掉的事件会让索引与磁盘不一致
//!   ⇒ 置 `overflowed` 标志，让上层退回全量重扫；
//! * 事件缓冲区满 ⇒ 同样丢最旧的 ⇒ 同样置 `overflowed`。
//!
//! 「丢了就退回全量」是这里唯一的安全策略：**宁可慢，不能错**。

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;

use notify::event::{ModifyKind, RenameMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use super::incremental::{normalize_index_path, FsEvent};

/// 事件通道容量。满了 ⇒ 判定为「消费不过来」。
const EVENT_CHANNEL_CAPACITY: usize = 8192;

/// 事件缓冲区上限。超出就丢最旧的，并置 `overflowed`。
const EVENT_BUFFER_CAPACITY: usize = 100_000;

struct WatcherState {
    events: Arc<Mutex<Vec<FsEvent>>>,
    /// **成功**监听到的盘根。监听到的才算数 ——
    /// 少监一个盘却当成「都监到了」，那一盘的变化就永远丢了。
    roots: Vec<String>,
    /// 不参与索引、也不产生事件的路径前缀（应用自己的数据目录）。
    excluded_prefixes: Vec<String>,
    generation: u64,
    watcher: Option<RecommendedWatcher>,
    worker: Option<thread::JoinHandle<()>>,
}

/// 当前 watcher 的快照。
#[derive(Debug, Clone, Default)]
pub(super) struct WatcherSnapshot {
    /// watcher 是否在跑。
    pub(super) running: bool,
    /// watcher 的代次。每次重开都会 +1 ——
    /// 代次对不上就说明「上次全量之后 watcher 换过」，中间有缺口，不能只做增量。
    pub(super) generation: u64,
    /// 当前监听的盘根。
    pub(super) roots: Vec<String>,
    /// 事件是否丢过。
    pub(super) overflowed: bool,
}

static WATCHER_STATE: Mutex<Option<WatcherState>> = Mutex::new(None);
static WATCHER_OVERFLOWED: AtomicBool = AtomicBool::new(false);
static WATCHER_GENERATION: AtomicU64 = AtomicU64::new(0);

/// 确保 watcher 正在监听这批盘根，返回当前代次。
///
/// * 盘根没变且 watcher 还在跑 ⇒ 原样返回（不动代次）；
/// * 盘根变了 / watcher 没了 ⇒ 重开，代次 +1。
///
/// 重开意味着「从这一刻起才有事件」，之前的空档只能靠全量重扫补 ——
/// 这件事由上层用**代次**比对来判断（见 `IncrementalGate`）。
pub(super) fn ensure_watcher(
    roots: &[String],
    excluded_prefixes: &[String],
) -> Option<u64> {
    let normalized_roots: Vec<String> = roots
        .iter()
        .map(|root| normalize_index_path(root))
        .collect();
    let normalized_excluded: Vec<String> = excluded_prefixes
        .iter()
        .map(|prefix| normalize_index_path(prefix).trim_end_matches('/').to_string())
        .filter(|prefix| !prefix.is_empty())
        .collect();

    let mut guard = WATCHER_STATE.lock().ok()?;

    if let Some(state) = guard.as_ref() {
        if state.roots == normalized_roots
            && state.excluded_prefixes == normalized_excluded
            && state.watcher.is_some()
        {
            return Some(state.generation);
        }
    }

    stop_locked(&mut guard);

    let (sender, receiver) = sync_channel::<notify::Result<Event>>(EVENT_CHANNEL_CAPACITY);

    let mut watcher = match notify::recommended_watcher(move |result| {
        if let Err(TrySendError::Full(_)) = sender.try_send(result) {
            WATCHER_OVERFLOWED.store(true, Ordering::SeqCst);
        }
    }) {
        Ok(watcher) => watcher,
        Err(error) => {
            log::warn!("[global-search] failed to create index watcher: {error}");
            return None;
        }
    };

    let mut watched_roots: Vec<String> = Vec::new();

    for root in &normalized_roots {
        match watcher.watch(Path::new(root), RecursiveMode::Recursive) {
            Ok(()) => watched_roots.push(root.clone()),
            Err(error) => {
                log::warn!("[global-search] failed to watch {}: {error}", root);
            }
        }
    }

    // 一个盘都没监到 ⇒ 这个 watcher 记不到任何变化 ⇒ 不能拿它做增量。
    if watched_roots.is_empty() {
        log::warn!("[global-search] index watcher could not watch any requested root");
        drop(watcher);
        return None;
    }

    let events: Arc<Mutex<Vec<FsEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let worker_events = Arc::clone(&events);
    let worker_excluded = normalized_excluded.clone();

    let worker = match thread::Builder::new()
        .name("global-search-index-watcher".to_string())
        .spawn(move || {
            while let Ok(result) = receiver.recv() {
                match result {
                    Ok(event) => {
                        let translated: Vec<FsEvent> = translate_event(&event)
                            .into_iter()
                            .filter(|event| !event_is_excluded(event, &worker_excluded))
                            .collect();

                        if translated.is_empty() {
                            continue;
                        }

                        if let Ok(mut buffer) = worker_events.lock() {
                            buffer.extend(translated);

                            if buffer.len() > EVENT_BUFFER_CAPACITY {
                                let overflow = buffer.len() - EVENT_BUFFER_CAPACITY;
                                buffer.drain(0..overflow);
                                WATCHER_OVERFLOWED.store(true, Ordering::SeqCst);
                            }
                        }
                    }
                    Err(error) => {
                        log::warn!("[global-search] index watcher reported an error: {error}");
                        WATCHER_OVERFLOWED.store(true, Ordering::SeqCst);
                    }
                }
            }
        }) {
        Ok(worker) => worker,
        Err(error) => {
            log::warn!("[global-search] failed to spawn index watcher thread: {error}");
            return None;
        }
    };

    let generation = WATCHER_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    *guard = Some(WatcherState {
        events,
        roots: watched_roots,
        excluded_prefixes: normalized_excluded,
        generation,
        watcher: Some(watcher),
        worker: Some(worker),
    });

    Some(generation)
}

/// 当前 watcher 的快照。
pub(super) fn watcher_snapshot() -> WatcherSnapshot {
    let overflowed = WATCHER_OVERFLOWED.load(Ordering::SeqCst);

    let guard = match WATCHER_STATE.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return WatcherSnapshot {
                overflowed,
                ..WatcherSnapshot::default()
            }
        }
    };

    match guard.as_ref() {
        Some(state) => WatcherSnapshot {
            running: state.watcher.is_some(),
            generation: state.generation,
            roots: state.roots.clone(),
            overflowed,
        },
        None => WatcherSnapshot {
            overflowed,
            ..WatcherSnapshot::default()
        },
    }
}

/// 取走当前积压的事件。
pub(super) fn drain_events() -> Vec<FsEvent> {
    let guard = match WATCHER_STATE.lock() {
        Ok(guard) => guard,
        Err(_) => return Vec::new(),
    };

    let state = match guard.as_ref() {
        Some(state) => state,
        None => return Vec::new(),
    };

    let mut buffer = match state.events.lock() {
        Ok(buffer) => buffer,
        Err(_) => return Vec::new(),
    };

    std::mem::take(&mut *buffer)
}

/// 清掉「丢过事件」的标记。
///
/// 只在**一次成功的全量重扫之后**调用 —— 全量重扫之后索引与磁盘重新对齐，
/// 之前的缺口已经被补上了。
pub(super) fn clear_overflowed() {
    WATCHER_OVERFLOWED.store(false, Ordering::SeqCst);
}

/// 关掉 watcher。
pub(super) fn stop_watcher() {
    let mut guard = match WATCHER_STATE.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };

    stop_locked(&mut guard);
}

fn stop_locked(guard: &mut Option<WatcherState>) {
    let state = match guard.take() {
        Some(state) => state,
        None => return,
    };

    // 先放掉 watcher —— 它一析构，notify 后端线程就退出，
    // 回调连同通道的发送端一起被丢掉，工作线程的 `recv()` 随即返回错误。
    let WatcherState {
        watcher,
        worker,
        events: _,
        roots: _,
        excluded_prefixes: _,
        generation: _,
    } = state;

    drop(watcher);

    if let Some(worker) = worker {
        let _ = worker.join();
    }
}

// FORK-MODIFICATION 2026-11-19（B25-2）：可移动盘不进索引根集合。
//
// 上面那个 `watch(root, Recursive)` 会对每个盘根**常驻**持一个卷根句柄。
// 只要句柄还在，Windows 的安全弹出就返回 `CR_REMOVE_VETOED (23)`：
// 用户点「弹出」被拒，只能硬拔 —— 拔了可能损坏 U 盘上的文件系统。
// 递归 watch 的覆盖面是整个卷，代价又远高于「搜不到 U 盘文件」，
// 所以这里在**根集合的构造处**就把可移动盘摘掉（摘在源头，
// watcher 与扫描列表才不会出现两套根集合，见 `scan.rs` 的路由判定）。
//
// 判定用注入形态：真实探针走 `GetDriveTypeW`，单测走表。

/// 盘根集合按「是否可移动盘」分成 (保留, 跳过)。
///
/// 纯函数：`is_removable` 由调用方注入，便于用表断言固定盘 / 网络盘不受影响。
pub(crate) fn partition_removable_drive_roots(
    roots: &[String],
    is_removable: &dyn Fn(&str) -> bool,
) -> (Vec<String>, Vec<String>) {
    let mut kept: Vec<String> = Vec::with_capacity(roots.len());
    let mut skipped: Vec<String> = Vec::new();

    for root in roots {
        if is_removable(root) {
            skipped.push(root.clone());
        }
        else {
            kept.push(root.clone());
        }
    }

    (kept, skipped)
}

/// 这个盘根是不是可移动盘（U 盘 / 读卡器）。非 Windows 恒为 `false`。
pub(crate) fn drive_root_is_removable(root: &str) -> bool {
    #[cfg(windows)]
    {
        windows_drive_root_is_removable(root)
    }
    #[cfg(not(windows))]
    {
        let _ = root;
        false
    }
}

#[cfg(windows)]
fn windows_drive_root_is_removable(root: &str) -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetDriveTypeW;

    /// `WINBASE.h`：`DRIVE_REMOVABLE`。
    const DRIVE_REMOVABLE: u32 = 2;

    let letter = match root.chars().next() {
        Some(letter) => letter,
        None => return false,
    };
    if !letter.is_ascii_alphabetic() {
        return false;
    }

    let root_wide: Vec<u16> = format!("{}:\\", letter.to_ascii_uppercase())
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    // `GetDriveTypeW` 只看盘符，路径不存在 / 无权限时返回 `DRIVE_NO_ROOT_DIR (1)`，
    // 不会失败报错；这里不需要区分「不是可移动盘」和「盘根本不在」。
    unsafe { GetDriveTypeW(PCWSTR::from_raw(root_wide.as_ptr())) == DRIVE_REMOVABLE }
}

/// 把可移动盘从索引根集合里摘掉，并对每个被摘的盘根**无条件**打一条 info 日志。
///
/// 这条日志是装机验收「跑的确实是新代码」的唯一判据，所以：
/// * 级别是 `info`（`tauri-plugin-log` 以 `LevelFilter::Info` 落盘，不需要开 debug）；
/// * 不受任何调试开关 / 采样控制；
/// * 内容含盘符，能一眼对上被摘的那块 U 盘。
pub(crate) fn watchable_drive_roots(roots: &[String]) -> Vec<String> {
    let (kept, skipped) = partition_removable_drive_roots(roots, &drive_root_is_removable);

    for root in &skipped {
        log::info!(
            "[global-search] skipping removable drive root from index watcher: {root}"
        );
    }

    kept
}

/// 释放某个盘根上的常驻 watch（U 盘被拔出 / 正在被弹出时调用）。
///
/// 返回释放掉的根数。同时置 `overflowed`：从这一刻起事件覆盖不全，
/// 下一轮只能退回全量重扫 —— 与「宁可慢，不能错」一致。
pub(crate) fn release_root_watch(root: &str) -> usize {
    let normalized = normalize_index_path(root);

    let mut guard = match WATCHER_STATE.lock() {
        Ok(guard) => guard,
        Err(_) => return 0,
    };

    let Some(state) = guard.as_mut() else {
        return 0;
    };

    let Some(position) = state
        .roots
        .iter()
        .position(|watched| normalize_index_path(watched) == normalized)
    else {
        return 0;
    };

    let removed = state.roots.remove(position);

    if let Some(watcher) = state.watcher.as_mut() {
        if let Err(error) = watcher.unwatch(Path::new(&removed)) {
            log::warn!("[global-search] failed to unwatch {removed}: {error}");
        }
    }

    WATCHER_OVERFLOWED.store(true, Ordering::SeqCst);
    log::info!("[global-search] released index watcher handle for volume {removed}");

    1
}

/// FORK-MODIFICATION 2026-11-19（B25-2）：退出前一次性放掉所有盘根的 watch。
///
/// 按卷释放走 `release_root_watch`（`device_events` 收到 `WM_DEVICECHANGE`
/// 时调用）；这里是最后一道兜底 —— 主窗口「关闭」其实只是 `hide()`
/// （托盘常驻），真正退出走的是 `RunEvent::Exit`，进程若还持着卷根句柄，
/// 用户就会看到「无法停止设备」。
///
/// 返回放掉的根个数（0 表示本来就没在跑）。
pub(crate) fn release_all_root_watches() -> usize {
    let mut guard = match WATCHER_STATE.lock() {
        Ok(guard) => guard,
        Err(_) => return 0,
    };

    let released = guard.as_ref().map_or(0, |state| state.roots.len());

    stop_locked(&mut guard);

    if released > 0 {
        log::info!("[global-search] released {released} index root watch(es) on shutdown");
    }

    released
}

/// 把 `notify` 的事件翻译成这里用的文件系统事件。
fn translate_event(event: &Event) -> Vec<FsEvent> {
    let mut translated = Vec::new();

    match event.kind {
        EventKind::Create(_) => {
            for path in &event.paths {
                translated.push(FsEvent::Created {
                    path: path.to_string_lossy().to_string(),
                });
            }
        }
        EventKind::Remove(_) => {
            for path in &event.paths {
                translated.push(FsEvent::Removed {
                    path: path.to_string_lossy().to_string(),
                });
            }
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            if event.paths.len() >= 2 {
                translated.push(FsEvent::Renamed {
                    from: event.paths[0].to_string_lossy().to_string(),
                    to: event.paths[1].to_string_lossy().to_string(),
                });
            } else {
                for path in &event.paths {
                    translated.push(FsEvent::Removed {
                        path: path.to_string_lossy().to_string(),
                    });
                }
            }
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
            for path in &event.paths {
                translated.push(FsEvent::Removed {
                    path: path.to_string_lossy().to_string(),
                });
            }
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            for path in &event.paths {
                translated.push(FsEvent::Created {
                    path: path.to_string_lossy().to_string(),
                });
            }
        }
        EventKind::Modify(_) => {
            for path in &event.paths {
                translated.push(FsEvent::Modified {
                    path: path.to_string_lossy().to_string(),
                });
            }
        }
        EventKind::Any | EventKind::Other => {
            for path in &event.paths {
                translated.push(FsEvent::Modified {
                    path: path.to_string_lossy().to_string(),
                });
            }
        }
        EventKind::Access(_) => {}
    }

    translated
}

/// 路径是不是落在被排除的前缀里。
fn is_excluded_path(path: &str, excluded_prefixes: &[String]) -> bool {
    let lowered = normalize_index_path(path).to_lowercase();

    excluded_prefixes.iter().any(|prefix| {
        let prefix = prefix.to_lowercase();
        lowered == prefix || lowered.starts_with(&format!("{prefix}/"))
    })
}

/// 这个事件是不是完全落在被排除的范围里。
fn event_is_excluded(event: &FsEvent, excluded_prefixes: &[String]) -> bool {
    match event {
        FsEvent::Created { path } | FsEvent::Modified { path } | FsEvent::Removed { path } => {
            is_excluded_path(path, excluded_prefixes)
        }
        FsEvent::Renamed { from, to } => {
            is_excluded_path(from, excluded_prefixes) || is_excluded_path(to, excluded_prefixes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- B25-2 层 1：可移动盘必须在 roots 构造处被剔除 ---------------------

    /// 注入「哪个盘符是可移动盘」的表，不碰真实 `GetDriveTypeW`。
    ///
    /// 表里的盘符和被问的 root 都先转大写 —— 真实 `GetDriveTypeW` 只认盘符
    /// 不认大小写，替身必须保持同样的语义，否则「大小写不敏感」这件事根本测不到。
    fn table(removable: &[char]) -> impl Fn(&str) -> bool + '_ {
        let removable: Vec<char> = removable
            .iter()
            .map(|letter| letter.to_ascii_uppercase())
            .collect();

        move |root: &str| match root.chars().next() {
            Some(letter) if letter.is_ascii_alphabetic() => {
                removable.contains(&letter.to_ascii_uppercase())
            }
            _ => false,
        }
    }

    #[test]
    fn partition_drops_removable_and_keeps_fixed_and_network_roots() {
        let roots: Vec<String> = [
            "C:/".to_string(),
            "H:/".to_string(),
            "D:/".to_string(),
            "\\\\nas\\share".to_string(),
            "Z:/".to_string(),
        ]
        .to_vec();

        // H: 是 U 盘；Z: 是映射的网络盘 —— 都不该被当成固定盘误删，
        // 但只有 H:（可移动）会被摘掉。
        let (kept, skipped) = partition_removable_drive_roots(&roots, &table(&['H']));

        assert_eq!(skipped, vec!["H:/".to_string()]);
        assert_eq!(
            kept,
            vec![
                "C:/".to_string(),
                "D:/".to_string(),
                "\\\\nas\\share".to_string(),
                "Z:/".to_string()
            ]
        );
    }

    #[test]
    fn partition_keeps_everything_when_no_drive_is_removable() {
        let roots: Vec<String> = ["C:/", "D:/", "E:/", "F:/", "G:/", "I:/"]
            .iter()
            .map(|root| root.to_string())
            .collect();

        let (kept, skipped) = partition_removable_drive_roots(&roots, &table(&[]));

        assert!(skipped.is_empty());
        assert_eq!(kept, roots);
    }

    #[test]
    fn partition_matches_letter_case_insensitively() {
        let roots: Vec<String> = vec!["h:/".to_string(), "H:/".to_string()];

        let (kept, skipped) = partition_removable_drive_roots(&roots, &table(&['h']));

        assert!(kept.is_empty());
        assert_eq!(skipped.len(), 2);

        // 替身之外，真探针自己也必须大小写不敏感。这里只断言「两种写法结果一致」，
        // 不断言一致到 true 还是 false —— 那取决于跑测试这台机器上 H: 到底是什么盘。
        assert_eq!(drive_root_is_removable("h:/"), drive_root_is_removable("H:/"));
    }

    #[test]
    fn removable_probe_rejects_paths_without_a_drive_letter() {
        // 非 Windows 恒 false；Windows 上这些前缀也拿不到盘符，必须走 false 而不是 panic。
        assert!(!drive_root_is_removable(""));
        assert!(!drive_root_is_removable("/mnt/usb"));
        assert!(!drive_root_is_removable("\\\\nas\\share"));
    }

    #[test]
    fn release_root_watch_ignores_roots_that_are_not_watched() {
        // `Q:/` 不是任何测试会去 watch 的根 —— 无论别的测试有没有并发建 watcher，
        // 这里都必须返回 0（并且不能碰 overflowed 标记）。
        assert_eq!(release_root_watch("Q:/"), 0);
    }
    use notify::event::EventAttributes;
    use std::path::PathBuf;

    fn event(kind: EventKind, paths: &[&str]) -> Event {
        Event {
            kind,
            paths: paths
                .iter()
                .map(|path| PathBuf::from(path))
                .collect::<Vec<PathBuf>>(),
            attrs: EventAttributes::default(),
        }
    }

    #[test]
    fn a_created_path_becomes_a_created_event() {
        let translated = translate_event(&event(EventKind::Create(notify::event::CreateKind::File), &["C:/a.txt"]));

        assert_eq!(
            translated,
            vec![FsEvent::Created {
                path: "C:/a.txt".to_string()
            }]
        );
    }

    #[test]
    fn a_removed_path_becomes_a_removed_event() {
        let translated = translate_event(&event(EventKind::Remove(notify::event::RemoveKind::File), &["C:/a.txt"]));

        assert_eq!(
            translated,
            vec![FsEvent::Removed {
                path: "C:/a.txt".to_string()
            }]
        );
    }

    #[test]
    fn a_rename_becomes_a_rename_event() {
        let translated = translate_event(&event(
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            &["C:/old.txt", "C:/new.txt"],
        ));

        assert_eq!(
            translated,
            vec![FsEvent::Renamed {
                from: "C:/old.txt".to_string(),
                to: "C:/new.txt".to_string(),
            }]
        );
    }

    #[test]
    fn a_modification_becomes_a_modified_event() {
        let translated = translate_event(&event(
            EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            &["C:/a.txt"],
        ));

        assert_eq!(
            translated,
            vec![FsEvent::Modified {
                path: "C:/a.txt".to_string()
            }]
        );
    }

    #[test]
    fn events_under_an_excluded_prefix_are_dropped() {
        let excluded = vec![
            "C:/Users/Duanyi/AppData/Roaming/com.sigma-file-manager.app".to_string(),
        ];

        assert!(is_excluded_path(
            "C:/Users/Duanyi/AppData/Roaming/com.sigma-file-manager.app/user-data/index/0",
            &excluded
        ));
        assert!(!is_excluded_path(
            "C:/Users/Duanyi/AppData/Roaming/com.sigma-file-manager.appx/file",
            &excluded
        ));
        assert!(!is_excluded_path("C:/Users/Duanyi/Documents/a.txt", &excluded));
    }

    #[test]
    fn an_access_event_is_ignored() {
        let translated = translate_event(&event(
            EventKind::Access(notify::event::AccessKind::Read),
            &["C:/a.txt"],
        ));

        assert!(translated.is_empty());
    }
}
