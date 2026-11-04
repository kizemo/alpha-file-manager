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
