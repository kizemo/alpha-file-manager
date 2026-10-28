// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-10-28：目录监听改为「共享 watcher」。
//
// 改前：每个被监听的目录各起 1 个应用线程 + notify 自己再起 1 个事件循环线程，
//       并且事件通道是**无界**的 mpsc。实测 Rust 主进程 77 线程 / 1250 句柄，
//       线程数随同时打开的目录数线性增长（开 N 个标签页 = 2N 个线程）。
//
// 改后：全局**一个** watcher 线程 + **一个** RecommendedWatcher，多路径 watch/unwatch；
//       事件走**有界**队列，满了丢最旧（文件监听只关心「最新状态」，丢事件是安全方向）。
//       线程数从 `2 × 目录数` 变成常量 1。
//
// 对外行为保持不变：`watch_directory` / `unwatch_directory` / `get_watched_directories`
// 的签名与语义不变，`dir-change` 事件名与 payload 字段不变，300 ms 节流语义不变。

use crate::utils::normalize_path;
use notify::{event::ModifyKind, Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// 事件队列容量：满了丢最旧的事件。
const EVENT_QUEUE_CAPACITY: usize = 1024;

/// 共享 watcher 线程的栈大小（std 默认 2 MB，对一个只做事件转发的线程太奢侈）。
const WATCHER_THREAD_STACK_SIZE: usize = 256 * 1024;

/// 同时监听的最大目录数（安全阀，正常使用远达不到）。
const MAX_ACTIVE_WATCHERS: usize = 256;

/// 单次事件节流窗口（毫秒）。
const EVENT_DEBOUNCE_MS: u64 = 300;

/// 事件队列空闲等待间隔（毫秒）。
const EVENT_POLL_INTERVAL_MS: u64 = 100;

/// 已登记的监听目录：归一化路径 → 原始路径（unwatch 时要用原始路径）。
static ACTIVE_WATCHERS: Lazy<Mutex<HashMap<String, PathBuf>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// 全局共享的 watcher 单例（惰性创建，进程内只建一次）。
static SHARED_WATCHER: Lazy<Mutex<Option<Arc<SharedWatcher>>>> = Lazy::new(|| Mutex::new(None));

/// 共享 watcher 的运行时状态。
struct SharedWatcher {
    /// notify watcher 本体。`watch()` / `unwatch()` 接受 `&self`，可跨线程调用。
    watcher: Arc<Mutex<RecommendedWatcher>>,
}

fn active_watcher_count() -> usize {
    ACTIVE_WATCHERS
        .lock()
        .map(|watchers| watchers.len())
        .unwrap_or(0)
}

fn active_watcher_paths() -> Vec<String> {
    ACTIVE_WATCHERS
        .lock()
        .map(|watchers| watchers.keys().cloned().collect())
        .unwrap_or_default()
}

fn is_relevant_event(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(_)
    )
}

fn event_kind_to_string(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::Create(_) => "create",
        EventKind::Remove(_) => "remove",
        EventKind::Modify(ModifyKind::Name(_)) => "rename",
        EventKind::Modify(ModifyKind::Data(_)) => "modify",
        _ => "change",
    }
}

fn dir_watcher_diag_enabled() -> bool {
    std::env::var("SFM_DIR_WATCHER_DIAG")
        .ok()
        .is_some_and(|value| value == "1")
}

macro_rules! dir_watcher_diag {
    ($($arg:tt)*) => {
        if dir_watcher_diag_enabled() {
            log::debug!($($arg)*);
        }
    };
}

// ---------------------------------------------------------------------------
// 有界事件队列
// ---------------------------------------------------------------------------

/// 有界事件队列：容量满时**丢弃最旧**的事件，保留最新状态。
struct EventQueue<T> {
    inner: Mutex<VecDeque<T>>,
    capacity: usize,
    signal: Condvar,
}

impl<T> EventQueue<T> {
    fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(VecDeque::new()),
            capacity: capacity.max(1),
            signal: Condvar::new(),
        }
    }

    fn push(&self, item: T) {
        let mut queue = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        queue.push_back(item);
        while queue.len() > self.capacity {
            queue.pop_front();
        }
        drop(queue);
        self.signal.notify_one();
    }

    /// 等待下一个事件；超时返回 `None`。
    fn pop_timeout(&self, timeout: Duration) -> Option<T> {
        let mut queue = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        if queue.is_empty() {
            let (guard, _wait_result) = self
                .signal
                .wait_timeout(queue, timeout)
                .unwrap_or_else(|error| error.into_inner());
            queue = guard;
        }
        queue.pop_front()
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
    }
}

// ---------------------------------------------------------------------------
// 事件 → 监听目录的映射
// ---------------------------------------------------------------------------

/// `child` 是否等于 `ancestor` 或位于其之下。
///
/// Windows 路径大小写不敏感，因此比较时忽略 ASCII 大小写；
/// 其它平台保持大小写敏感（Linux 上大小写不同的目录是两个目录）。
fn is_same_or_descendant(child: &str, ancestor: &str) -> bool {
    if ancestor == "/" {
        return child.starts_with('/') && child != "/";
    }

    let ancestor = ancestor.trim_end_matches('/');
    if ancestor.is_empty() {
        return false;
    }

    #[cfg(windows)]
    {
        if child.eq_ignore_ascii_case(ancestor) {
            return true;
        }
        let prefix = format!("{ancestor}/");
        match child.get(..prefix.len()) {
            Some(head) => head.eq_ignore_ascii_case(&prefix),
            None => false,
        }
    }

    #[cfg(not(windows))]
    {
        if child == ancestor {
            return true;
        }
        let prefix = format!("{ancestor}/");
        child.starts_with(&prefix)
    }
}

/// 把一条 notify 事件映射到它所属的监听目录（取最长匹配前缀）。
///
/// 共享 watcher 不知道事件来自哪个 `watch()`，只能靠路径反推：
/// 事件路径通常形如 `<被监听目录>/<条目名>`，取最长匹配的已监听目录即可。
fn resolve_watched_path(event_paths: &[PathBuf], watched_paths: &[String]) -> Option<String> {
    let mut best: Option<&str> = None;

    for raw_path in event_paths {
        let candidate = normalize_path(&raw_path.to_string_lossy());

        for watched in watched_paths {
            if !is_same_or_descendant(&candidate, watched) {
                continue;
            }

            if best
                .map(|current| watched.len() > current.len())
                .unwrap_or(true)
            {
                best = Some(watched);
            }
        }
    }

    best.map(|value| value.to_string())
}

// ---------------------------------------------------------------------------
// 节流
// ---------------------------------------------------------------------------

/// 需要发往前端的 `dir-change` 内容。
#[derive(Debug, PartialEq, Eq)]
enum WatcherEmit {
    /// 带具体变更路径与类型的事件。
    Changed { kind: &'static str, changed_path: String },
    /// 节流窗口结束后的兜底刷新事件（路径留空，与旧实现一致）。
    Refresh,
}

/// 每个监听目录一份节流状态。
#[derive(Default)]
struct DebounceEntry {
    last_emit: Option<Instant>,
    pending: bool,
}

/// 节流器：同一目录 300 ms 内只发一次「具体变更」，
/// 窗口结束且队列空闲时补发一次「刷新」。
struct DebounceTracker {
    debounce: Duration,
    entries: HashMap<String, DebounceEntry>,
}

impl DebounceTracker {
    fn new(debounce: Duration) -> Self {
        Self {
            debounce,
            entries: HashMap::new(),
        }
    }

    /// 收到一个事件时调用；返回需要立刻发出的内容。
    fn on_event(
        &mut self,
        watched_path: &str,
        kind: &'static str,
        changed_path: String,
        now: Instant,
    ) -> Option<WatcherEmit> {
        let entry = self.entries.entry(watched_path.to_string()).or_default();

        let should_emit = match entry.last_emit {
            Some(last_time) => now.duration_since(last_time) >= self.debounce,
            None => true,
        };

        if should_emit {
            entry.last_emit = Some(now);
            entry.pending = false;
            return Some(WatcherEmit::Changed { kind, changed_path });
        }

        entry.pending = true;
        None
    }

    /// 空闲时调用：把节流窗口内被压下的事件补发成「刷新」。
    fn on_idle(&mut self, now: Instant) -> Vec<(String, WatcherEmit)> {
        let mut emits = Vec::new();

        for (watched_path, entry) in self.entries.iter_mut() {
            if !entry.pending {
                continue;
            }

            let Some(last_time) = entry.last_emit else {
                continue;
            };

            if now.duration_since(last_time) >= self.debounce {
                entry.last_emit = Some(now);
                entry.pending = false;
                emits.push((watched_path.clone(), WatcherEmit::Refresh));
            }
        }

        emits
    }

    /// 清理已不再监听的目录，避免状态表随浏览时长增长。
    fn retain_watched(&mut self, watched_paths: &HashSet<String>) {
        self.entries
            .retain(|watched_path, _| watched_paths.contains(watched_path));
    }
}

// ---------------------------------------------------------------------------
// 事件消费线程
// ---------------------------------------------------------------------------

fn emit_dir_change(app: &AppHandle, watched_path: &str, emit: &WatcherEmit) {
    let payload = match emit {
        WatcherEmit::Changed {
            kind,
            changed_path,
        } => serde_json::json!({
            "watchedPath": watched_path,
            "changedPath": changed_path,
            "kind": kind,
        }),
        WatcherEmit::Refresh => serde_json::json!({
            "watchedPath": watched_path,
            "changedPath": "",
            "kind": "change",
        }),
    };

    if let Err(err) = app.emit("dir-change", payload) {
        log::error!("Failed to emit dir-change event: {}", err);
    }
}

fn watched_path_snapshot() -> Vec<String> {
    ACTIVE_WATCHERS
        .lock()
        .map(|watchers| watchers.keys().cloned().collect())
        .unwrap_or_default()
}

fn watched_path_set() -> HashSet<String> {
    ACTIVE_WATCHERS
        .lock()
        .map(|watchers| watchers.keys().cloned().collect())
        .unwrap_or_default()
}

/// 启动唯一的 watcher 消费线程。
fn spawn_event_consumer(app: AppHandle, events: Arc<EventQueue<notify::Event>>) {
    let builder = thread::Builder::new()
        .name("afm-dir-watcher".to_string())
        .stack_size(WATCHER_THREAD_STACK_SIZE);

    if let Err(error) = builder.spawn(move || {
        dir_watcher_diag!("[dir-watcher-diag] watcher thread started");

        let mut debounce = DebounceTracker::new(Duration::from_millis(EVENT_DEBOUNCE_MS));

        loop {
            match events.pop_timeout(Duration::from_millis(EVENT_POLL_INTERVAL_MS)) {
                Some(event) => {
                    if !is_relevant_event(&event.kind) {
                        continue;
                    }

                    let kind = event_kind_to_string(&event.kind);
                    let changed_path = event
                        .paths
                        .first()
                        .map(|path| normalize_path(&path.to_string_lossy()))
                        .unwrap_or_default();

                    let watched_paths = watched_path_snapshot();
                    let Some(watched_path) = resolve_watched_path(&event.paths, &watched_paths)
                    else {
                        continue;
                    };

                    if let Some(emit) =
                        debounce.on_event(&watched_path, kind, changed_path, Instant::now())
                    {
                        emit_dir_change(&app, &watched_path, &emit);
                    }
                }
                None => {
                    let now = Instant::now();

                    for (watched_path, emit) in debounce.on_idle(now) {
                        emit_dir_change(&app, &watched_path, &emit);
                    }

                    debounce.retain_watched(&watched_path_set());
                }
            }
        }
    }) {
        log::error!("[dir-watcher-diag] Failed to spawn watcher thread: {}", error);
    }
}

/// 惰性创建全局共享 watcher。
fn ensure_shared_watcher(app: &AppHandle) -> Result<Arc<SharedWatcher>, String> {
    let mut shared = SHARED_WATCHER
        .lock()
        .unwrap_or_else(|error| error.into_inner());

    if let Some(existing) = shared.as_ref() {
        return Ok(Arc::clone(existing));
    }

    let events = Arc::new(EventQueue::new(EVENT_QUEUE_CAPACITY));
    let events_for_callback = Arc::clone(&events);

    let watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                events_for_callback.push(event);
            }
        },
        Config::default().with_poll_interval(Duration::from_secs(1)),
    )
    .map_err(|error| format!("Failed to create directory watcher: {error}"))?;

    dir_watcher_diag!("[dir-watcher-diag] shared RecommendedWatcher created");

    let watcher = Arc::new(Mutex::new(watcher));
    spawn_event_consumer(app.clone(), Arc::clone(&events));

    let instance = Arc::new(SharedWatcher { watcher });
    *shared = Some(Arc::clone(&instance));

    Ok(instance)
}

// ---------------------------------------------------------------------------
// 对外命令
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn watch_directory(app: AppHandle, path: String) -> Result<(), String> {
    let normalized_path = normalize_path(&path);
    let watch_path = PathBuf::from(&path);

    dir_watcher_diag!(
        "[dir-watcher-diag] watch_directory request path={} normalized={} active_count={} active_paths={:?}",
        path,
        normalized_path,
        active_watcher_count(),
        active_watcher_paths(),
    );

    if !watch_path.exists() {
        log::warn!(
            "[dir-watcher-diag] watch_directory rejected missing path={} normalized={}",
            path,
            normalized_path,
        );
        return Err(format!("Path does not exist: {}", path));
    }

    if !watch_path.is_dir() {
        log::warn!(
            "[dir-watcher-diag] watch_directory rejected non-directory path={} normalized={}",
            path,
            normalized_path,
        );
        return Err(format!("Path is not a directory: {}", path));
    }

    {
        let watchers = ACTIVE_WATCHERS
            .lock()
            .map_err(|error| error.to_string())?;

        if watchers.contains_key(&normalized_path) {
            dir_watcher_diag!(
                "[dir-watcher-diag] watch_directory skipped already-active path={} active_count={}",
                normalized_path,
                watchers.len(),
            );
            return Ok(());
        }

        if watchers.len() >= MAX_ACTIVE_WATCHERS {
            log::warn!(
                "[dir-watcher-diag] watch_directory rejected limit reached path={} active_count={} limit={}",
                normalized_path,
                watchers.len(),
                MAX_ACTIVE_WATCHERS,
            );
            return Err(format!(
                "Too many watched directories (limit: {})",
                MAX_ACTIVE_WATCHERS
            ));
        }
    }

    let shared = ensure_shared_watcher(&app)?;

    {
        let mut watcher = shared
            .watcher
            .lock()
            .map_err(|error| error.to_string())?;

        watcher
            .watch(&watch_path, RecursiveMode::NonRecursive)
            .map_err(|error| {
                log::error!(
                    "[dir-watcher-diag] Failed to watch path={} watch_path={} error={}",
                    normalized_path,
                    watch_path.display(),
                    error,
                );
                format!("Failed to watch path: {error}")
            })?;
    }

    dir_watcher_diag!(
        "[dir-watcher-diag] registered watch path={} watch_path={}",
        normalized_path,
        watch_path.display(),
    );

    let mut watchers = ACTIVE_WATCHERS
        .lock()
        .map_err(|error| error.to_string())?;
    watchers.insert(normalized_path.clone(), watch_path.clone());

    dir_watcher_diag!(
        "[dir-watcher-diag] watch_directory registered path={} active_count={} active_paths={:?}",
        normalized_path,
        watchers.len(),
        watchers.keys().cloned().collect::<Vec<_>>(),
    );

    Ok(())
}

#[tauri::command]
pub async fn unwatch_directory(path: String) -> Result<(), String> {
    let normalized_path = normalize_path(&path);

    dir_watcher_diag!(
        "[dir-watcher-diag] unwatch_directory request path={} normalized={} active_count={} active_paths={:?}",
        path,
        normalized_path,
        active_watcher_count(),
        active_watcher_paths(),
    );

    let removed = {
        let mut watchers = ACTIVE_WATCHERS
            .lock()
            .map_err(|error| error.to_string())?;
        watchers.remove(&normalized_path)
    };

    if let Some(raw_path) = removed {
        let shared = SHARED_WATCHER
            .lock()
            .unwrap_or_else(|error| error.into_inner());

        if let Some(instance) = shared.as_ref() {
            let mut watcher = instance
                .watcher
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if let Err(error) = watcher.unwatch(&raw_path) {
                log::warn!(
                    "[dir-watcher-diag] Failed to unwatch path={} error={}",
                    raw_path.display(),
                    error,
                );
            }
        }

        dir_watcher_diag!(
            "[dir-watcher-diag] unwatch_directory removed path={} remaining_active_count={} remaining_active_paths={:?}",
            normalized_path,
            active_watcher_count(),
            active_watcher_paths(),
        );
    } else {
        log::warn!(
            "[dir-watcher-diag] unwatch_directory no active watcher path={} active_count={}",
            normalized_path,
            active_watcher_count(),
        );
    }

    Ok(())
}

#[tauri::command]
pub fn get_watched_directories() -> Result<Vec<String>, String> {
    let watchers = ACTIVE_WATCHERS.lock().map_err(|error| error.to_string())?;
    Ok(watchers.keys().cloned().collect())
}

// ---------------------------------------------------------------------------
// 测试
//
// 这些用例刻意**不触碰** `tauri::AppHandle`：
// 被测试链接到的代码一旦碰到 tauri 运行时，可能把 Wry 窗口链拖进测试二进制，
// 导致 `cargo test --lib` 在加载阶段以 STATUS_ENTRYPOINT_NOT_FOUND 崩掉。
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{
        resolve_watched_path, DebounceTracker, EventQueue, WatcherEmit, EVENT_DEBOUNCE_MS,
    };
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn watched_paths() -> Vec<String> {
        vec!["C:/data".to_string(), "C:/data/inner".to_string()]
    }

    #[test]
    fn event_queue_drops_oldest_when_full() {
        let queue = EventQueue::<String>::new(4);

        for index in 0..6 {
            queue.push(index.to_string());
        }

        assert_eq!(queue.len(), 4);
        assert_eq!(queue.pop_timeout(Duration::from_millis(1)), Some("2".to_string()));
        assert_eq!(queue.pop_timeout(Duration::from_millis(1)), Some("3".to_string()));
    }

    #[test]
    fn event_queue_keeps_latest_state() {
        let queue = EventQueue::<String>::new(2);

        queue.push("first".to_string());
        queue.push("second".to_string());
        queue.push("third".to_string());

        assert_eq!(queue.pop_timeout(Duration::from_millis(1)), Some("second".to_string()));
        assert_eq!(queue.pop_timeout(Duration::from_millis(1)), Some("third".to_string()));
    }

    #[test]
    fn resolve_watched_path_picks_longest_match() {
        let paths = watched_paths();

        assert_eq!(
            resolve_watched_path(&[PathBuf::from(r"C:\data\inner\file.txt")], &paths),
            Some("C:/data/inner".to_string())
        );
        assert_eq!(
            resolve_watched_path(&[PathBuf::from(r"C:\data\other.txt")], &paths),
            Some("C:/data".to_string())
        );
        assert_eq!(
            resolve_watched_path(&[PathBuf::from(r"C:\elsewhere\other.txt")], &paths),
            None
        );
    }

    #[test]
    fn resolve_watched_path_is_case_insensitive_on_windows() {
        let paths = vec!["C:/Data".to_string()];

        assert_eq!(
            resolve_watched_path(&[PathBuf::from(r"c:\data\file.txt")], &paths),
            Some("C:/Data".to_string())
        );
    }

    #[test]
    fn debounce_emits_first_event_immediately() {
        let mut tracker = DebounceTracker::new(Duration::from_millis(EVENT_DEBOUNCE_MS));
        let now = Instant::now();

        assert_eq!(
            tracker.on_event("C:/data", "create", "C:/data/a.txt".to_string(), now),
            Some(WatcherEmit::Changed {
                kind: "create",
                changed_path: "C:/data/a.txt".to_string(),
            })
        );
    }

    #[test]
    fn debounce_suppresses_event_inside_window_then_flushes_on_idle() {
        let mut tracker = DebounceTracker::new(Duration::from_millis(EVENT_DEBOUNCE_MS));
        let start = Instant::now();

        assert!(tracker
            .on_event("C:/data", "create", "C:/data/a.txt".to_string(), start)
            .is_some());

        let inside_window = start + Duration::from_millis(100);
        assert_eq!(
            tracker.on_event(
                "C:/data",
                "create",
                "C:/data/b.txt".to_string(),
                inside_window
            ),
            None
        );

        // 节流窗口还没过，不补发。
        assert!(tracker.on_idle(inside_window).is_empty());

        // 窗口过了，队列空闲 ⇒ 补一次「刷新」。
        let after_window = start + Duration::from_millis(400);
        let emits = tracker.on_idle(after_window);
        assert_eq!(emits.len(), 1);
        assert_eq!(emits[0].0, "C:/data");
        assert_eq!(emits[0].1, WatcherEmit::Refresh);
    }

    #[test]
    fn debounce_keeps_state_per_watched_directory() {
        let mut tracker = DebounceTracker::new(Duration::from_millis(EVENT_DEBOUNCE_MS));
        let start = Instant::now();

        assert!(tracker
            .on_event("C:/data", "create", "C:/data/a.txt".to_string(), start)
            .is_some());

        // 另一个目录不受影响，仍然立即发。
        assert!(tracker
            .on_event("C:/other", "create", "C:/other/a.txt".to_string(), start)
            .is_some());
    }

    #[test]
    fn debounce_retain_watched_drops_unwatched_directories() {
        let mut tracker = DebounceTracker::new(Duration::from_millis(EVENT_DEBOUNCE_MS));
        let start = Instant::now();

        tracker.on_event("C:/data", "create", "C:/data/a.txt".to_string(), start);

        tracker.retain_watched(&std::collections::HashSet::new());
        assert!(tracker.on_idle(start + Duration::from_millis(400)).is_empty());
    }
}
