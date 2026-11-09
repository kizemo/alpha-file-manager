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
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct DialogInfo {
    pub hwnd: u32,
    pub app: String,
    pub last_known_path: String,
    pub write_strategy: Option<String>,
}

#[derive(Debug)]
pub struct AppState {
    pub current_path: Mutex<String>,
    pub registry: Mutex<HashMap<u32, DialogInfo>>,

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

impl AppState {
    pub fn new(initial_path: String, session_id: u32) -> Self {
        Self {
            current_path: Mutex::new(initial_path),
            registry: Mutex::new(HashMap::new()),
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
        }
    }

    /// v0.5.19 — 记录「用户从 AFM 切走」：接手的应用 pid + 发生时刻。
    ///
    /// 由 uia_event.rs 的 monitor 在 **AFM -> 非 AFM** 的下降沿调用。
    pub fn set_afm_handoff(&self, pid: u32) {
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

    /// v0.5.19 — 当前持有的交接授权（None = 没有）。
    pub fn afm_handoff(&self) -> Option<AfmHandoff> {
        *self.afm_handoff.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// v0.5.19 — 闸门：这次弹窗写回能不能放行？
    ///
    /// 三条同时成立才放行：
    ///   1. 发生过 AFM -> X 的交接；
    ///   2. 交接距今不超过 TTL（默认 120 秒）；
    ///   3. 弹窗所属进程 == 接手 AFM 的那个应用。
    ///      认不出 pid 时（0）只查前两条，不做应用级限制。
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
        if handoff.pid == 0 || dialog_pid == 0 {
            return true;
        }
        handoff.pid == dialog_pid
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

    pub fn set_current_path(&self, p: String) {
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
        match self.last_written_path(hwnd) {
            Some(last) if !last.trim().is_empty() => !paths_equal(&last, current_path),
            _ => true,
        }
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
            afm_visit_latched: self.afm_handoff().is_some(),
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
    const AFM_PATH: &str = "F:\\soft\\00selfmade\\filemanager";

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
}

/// v0.5.19 — 路径合法性：只有真实存在的本地目录才允许写进别人的对话框。
///
/// 真机事故（2026-11-09 11:31）：AFM 停在百度网盘远程目录时，扩展把
/// `sfm://remote/<id>/0000temp` 剥成伪路径 `\remote\<id>\0000temp`，
/// 侧车把它写进另一个程序的「打开」对话框 ⇒ Windows 弹「找不到」错误框
/// ⇒ 错误框又被当成新对话框 ⇒ 每 1~2 秒重试 ⇒ 关闭后仍反复报错。
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
}