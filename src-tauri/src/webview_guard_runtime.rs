// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! WebView2 子进程崩溃兜底 —— **依赖 tauri 的接线部分**。
//!
//! 判定逻辑在 `webview_guard.rs`（纯逻辑，不引用 tauri）；这里只做三件事：
//!
//! 1. **采集**：前端窗口每 5 秒调一次 `webview_guard_heartbeat`；
//! 2. **判定**：看门狗线程每 2 秒扫一次 WebView2 子进程 + 读心跳，交给纯逻辑模块判定；
//! 3. **恢复**：判定为崩溃 ⇒ 先自动 `reload()` 该窗口；reload 之后心跳没回来 ⇒
//!    升级成原生提示框（此时界面可能已经是死的，网页里的提示框发不出来）。
//!
//! ## 为什么要「原生提示框」
//!
//! 2026-10-28 的取证结论（见 handoff §18.11 D3）：三次 WebView2 崩溃都发生在
//! `msedgewebview2.exe` **自己的子进程**里，而 **AFM 主进程存活**。
//! 也就是说，出事的那一方不会自己报错 —— 只能靠主进程侧探测。
//! 而如果崩的是承载主窗口的渲染进程，**网页里的 UI 也没了**，
//! 「用 toast 提示用户」这条路在那个场景下根本不存在。
//! ⇒ 兜底提示必须有一条**不依赖 webview** 的通道：Win32 `MessageBoxW`。

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

use crate::webview_guard::{
    browser_process_replaced,
    decide_health, scan_webview_processes, WebviewGuardVerdict, WindowHeartbeat,
    BROWSER_GONE_CONFIRM_TICKS, HEARTBEAT_INTERVAL_MS, HEARTBEAT_TIMEOUT_MS,
    WATCHDOG_POLL_INTERVAL_MS,
};

/// 看门狗把状态变化推给前端的事件名。
pub const WEBVIEW_GUARD_EVENT: &str = "webview-guard://status";

/// reload 之后等待心跳恢复的时限（毫秒）。超过就升级成原生提示。
const RECOVERY_DEADLINE_MS: u64 = 15_000;

/// 「有 WebView2 子进程消失过」这个旁证的保鲜时长（毫秒）。
///
/// 进程消失只会在它发生的那一次采样里被看见，而心跳要 20 秒才超时 ——
/// 两个时间尺度对不上，所以旁证要保留一段时间，
/// 否则「崩溃」这个结论只在 2 秒的窗口里可见，很容易被采样时机错过。
const PROCESS_EVIDENCE_WINDOW_MS: u64 = 30_000;

/// 单调时钟起点。看门狗与心跳共用同一个时间基准。
static GUARD_START: OnceLock<Instant> = OnceLock::new();

/// 距离看门狗启动过去了多少毫秒。
pub fn now_ms() -> u64 {
    GUARD_START
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis() as u64
}

/// 推给前端的通知。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewGuardNotice {
    /// `renderer-crashed` / `window-hung` / `browser-gone` / `recovered` / `recovery-failed`。
    pub kind: String,
    /// 出问题的窗口标签。
    pub label: String,
    /// 补充说明。
    pub detail: String,
    /// 是否还能自动恢复。
    pub recoverable: bool,
}

/// 单个窗口的状态（给调试界面 / 状态查询用）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewGuardWindowStatus {
    pub label: String,
    pub missed_ms: u64,
}

/// 看门狗的完整状态。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewGuardStatus {
    pub windows: Vec<WebviewGuardWindowStatus>,
    pub browser_alive: bool,
    pub browser_seen: bool,
    pub recovery: String,
}

/// 恢复流程的状态机。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum RecoveryState {
    /// 空闲。
    #[default]
    Idle,
    /// 已经对某个窗口发起过一次 reload，正在等它的心跳回来。
    ReloadAttempted { label: String, started_ms: u64 },
}

/// 看门狗的可变状态。
#[derive(Debug, Default)]
struct GuardInner {
    /// 窗口标签 → 最近一次心跳时刻。
    heartbeats: HashMap<String, u64>,
    /// 已经见过的 WebView2 子进程 pid。
    known_pids: HashSet<u32>,
    /// 恢复流程状态机。
    recovery: RecoveryState,
    /// 见过浏览器进程没有。
    browser_seen: bool,
    /// 浏览器进程当前是否还在。
    browser_alive: bool,
    /// 「浏览器进程消失」连续命中了几次。
    browser_gone_ticks: u32,
    /// 最近一次「有 WebView2 子进程消失」的时刻。
    ///
    /// 用 `Option` 而不是 `u64`：初值 0 会被「距今多久」的比较当成「刚刚发生过」
    /// （看门狗启动后的头 30 秒内），于是那段时间里任何心跳超时都会被误判成
    /// 「确实有进程死过」⇒ 必须真的发生过一次才置 `Some`。
    last_vanish_ms: Option<u64>,
    /// 已经见过的 WebView2 浏览器进程 pid（用来识别「浏览器进程被换掉」）。
    known_browser_pids: HashSet<u32>,
    /// 原生提示框是否已经弹过。
    ///
    /// 同一场故障里提示框只能弹一次：用户关掉之后，看门狗下一轮又会重新
    /// 判定为「浏览器进程没了」，如果没有这道闩锁，提示框会每 2 秒弹一次，
    /// 变成无法关闭的刷屏。应用恢复正常（判定回到 Healthy）时重新解锁。
    native_prompt_shown: bool,
    /// 看门狗已经轮询了多少轮（用于周期存活日志）。
    tick_count: u64,
}

/// 由 Tauri 托管的看门狗状态。
#[derive(Default)]
pub struct WebviewGuardState {
    inner: Mutex<GuardInner>,
    shutting_down: AtomicBool,
    /// 主窗口的原生句柄（HWND，0 表示还没抓到）。
    ///
    /// 为什么不用「Tauri 窗口表里还有没有 main」来判断主窗口死活：
    /// WebView2 浏览器进程死亡时，wry 有可能把 webview 窗口对象一并摘掉
    /// ⇒ 那种情况下 get_webview_window("main") 返回 None，
    /// 看门狗会在每轮开头直接返回，「浏览器进程没了 ⇒ 提示用户重启」这条路
    /// 永远走不到。而 HWND 是宿主窗口（tao 建的），浏览器进程死掉不会销毁它
    /// ⇒ 用 IsWindow(hwnd) 判活才是可靠口径。
    main_hwnd: AtomicIsize,
}

fn lock_inner(state: &WebviewGuardState) -> MutexGuard<'_, GuardInner> {
    state.inner.lock().unwrap_or_else(|error| error.into_inner())
}

/// 前端心跳。前端的 `setInterval` 每 5 秒调一次。
#[tauri::command]
pub fn webview_guard_heartbeat(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, WebviewGuardState>,
) {
    let label = window.label().to_string();
    let now = now_ms();
    let mut inner = lock_inner(&state);
    // 第一次收到心跳时留一条日志：它是「前端 ↔ 后端心跳通道确实通了」的
    // 阳性对照，出问题时能一眼分清「通道没通」与「通道通了但没检测到」。
    let is_first = inner.heartbeats.insert(label.clone(), now).is_none();
    // 前端又发心跳了 ⇒ 界面真的活着 ⇒ 解开「原生提示框只弹一次」的闩锁。
    //
    // 为什么不能用「判定回到 Healthy」来解锁：Healthy 只代表「暂时还没判成故障」。
    // 渲染进程死掉之后、心跳超时还没到 60s 卡死门槛的那段时间里，判定同样是 Healthy
    // ⇒ 闩锁会在故障还没结束的时候被解开，下一轮又弹一次框
    // （沙箱实测：12:55:58 判卡死、12:56:14 又弹了一次提示框）。
    // 前端心跳才是「界面真的活着」的阳性信号。
    let latch_released = if inner.native_prompt_shown {
        inner.native_prompt_shown = false;
        true
    } else {
        false
    };
    drop(inner);

    if is_first {
        log::info!("webview guard: heartbeat established for window {label}");
    }

    if latch_released {
        log::info!(
            "webview guard: the frontend is alive again; the native prompt latch is released for window {label}"
        );
    }
}

/// 查询看门狗状态（诊断用）。
#[tauri::command]
pub fn webview_guard_status(state: tauri::State<'_, WebviewGuardState>) -> WebviewGuardStatus {
    let now = now_ms();
    let inner = lock_inner(&state);

    let windows = inner
        .heartbeats
        .iter()
        .map(|(label, &last_ping_ms)| WebviewGuardWindowStatus {
            label: label.clone(),
            missed_ms: now.saturating_sub(last_ping_ms),
        })
        .collect();

    WebviewGuardStatus {
        windows,
        browser_alive: inner.browser_alive,
        browser_seen: inner.browser_seen,
        recovery: match &inner.recovery {
            RecoveryState::Idle => "idle".to_string(),
            RecoveryState::ReloadAttempted { label, .. } => format!("reload-attempted:{label}"),
        },
    }
}

/// 应用正在退出 ⇒ 看门狗不要再弹提示。
pub fn mark_shutting_down(app: &AppHandle) {
    app.state::<WebviewGuardState>()
        .shutting_down
        .store(true, Ordering::Relaxed);
}

/// 启动看门狗线程。
pub fn start_webview_guard(app: &AppHandle) {
    // 先记住主窗口的原生句柄：判定「主窗口还在不在」要靠 HWND，
    // 不能靠 Tauri 的窗口表（浏览器进程死亡时那个表可能被清空）。
    capture_main_hwnd(app);

    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("afm-webview-guard".to_string())
        .spawn(move || watchdog_loop(app));

    if let Err(error) = spawned {
        log::error!("webview guard: failed to spawn the watchdog thread: {error}");
    }
}

/// 记住主窗口的原生句柄。窗口还没建出来时返回 false。
fn capture_main_hwnd(app: &AppHandle) -> bool {
    let Some(window) = app.get_webview_window("main") else {
        return false;
    };

    match window.hwnd() {
        Ok(hwnd) => {
            // 取原始指针，避开 Tauri 那一版 windows crate 的 HWND 类型。
            let raw = hwnd.0 as isize;
            if raw == 0 {
                return false;
            }
            app.state::<WebviewGuardState>()
                .main_hwnd
                .store(raw, Ordering::Relaxed);
            log::info!("webview guard: main window handle captured");
            true
        }
        Err(error) => {
            log::warn!("webview guard: failed to get the main window handle: {error}");
            false
        }
    }
}

/// 句柄还没抓到时补抓一次（窗口可能比看门狗线程晚建出来）。
fn ensure_main_hwnd(app: &AppHandle) {
    let state = app.state::<WebviewGuardState>();
    if state.main_hwnd.load(Ordering::Relaxed) != 0 {
        return;
    }
    capture_main_hwnd(app);
}

fn is_shutting_down(app: &AppHandle) -> bool {
    app.state::<WebviewGuardState>()
        .shutting_down
        .load(Ordering::Relaxed)
}

fn watchdog_loop(app: AppHandle) {
    // 等 webview 真正起来再开始判定，避免把「启动初期」当成异常。
    std::thread::sleep(Duration::from_millis(WATCHDOG_POLL_INTERVAL_MS * 2));

    while !is_shutting_down(&app) {
        tick(&app);
        std::thread::sleep(Duration::from_millis(WATCHDOG_POLL_INTERVAL_MS));
    }
}

fn tick(app: &AppHandle) {
    // 应用正在退出 ⇒ 不做任何判定。
    // 退出过程里 WebView2 的子进程会先死，没有这道闸就会被当成「崩溃」。
    if is_shutting_down(app) {
        return;
    }

    // 窗口可能比看门狗晚建出来 ⇒ 句柄还没抓到时补抓一次。
    ensure_main_hwnd(app);

    // 主窗口真的没了（窗口已被销毁）⇒ 同样不判定。
    if !main_window_alive(app) {
        return;
    }

    let now = now_ms();
    let snapshot = scan_webview_processes();
    let state = app.state::<WebviewGuardState>();

    // 1) 进程 bookkeeping：找出消失的 WebView2 子进程。
    let mut inner = lock_inner(&state);
    let current: HashSet<u32> = snapshot.all_pids.iter().copied().collect();
    let vanished: Vec<u32> = inner.known_pids.difference(&current).copied().collect();
    inner.known_pids.retain(|pid| current.contains(pid));
    inner.known_pids.extend(current);

    if !vanished.is_empty() {
        inner.last_vanish_ms = Some(now);
        log::warn!(
            "webview guard: WebView2 child process(es) vanished: {:?}",
            vanished
        );
    }

    // 浏览器进程被换掉，也算「确实死过 WebView2 子进程」的旁证：
    // 杀浏览器进程时 WebView2 会自己把浏览器进程拉起来（新的 pid），
    // 被杀的那个在进程表里可能还挂着 ⇒ 「pid 集合少了东西」这条旁证不成立，
    // 只能靠「浏览器进程集合变了」来认。
    let known_browser: Vec<u32> = inner.known_browser_pids.iter().copied().collect();
    if browser_process_replaced(&known_browser, &snapshot.browser_pids) {
        inner.last_vanish_ms = Some(now);
        log::warn!(
            "webview guard: the WebView2 browser process was replaced: old={:?} new={:?}",
            known_browser,
            snapshot.browser_pids
        );
    }
    inner.known_browser_pids = snapshot.browser_pids.iter().copied().collect();

    inner.browser_alive = snapshot.has_browser();
    if snapshot.has_browser() {
        inner.browser_seen = true;
    }

    // 2) 摘掉已经关掉的窗口的心跳（否则会误判成「窗口崩了」）。
    let alive_labels: HashSet<String> = app.webview_windows().keys().cloned().collect();
    inner.heartbeats.retain(|label, _| alive_labels.contains(label));

    let heartbeat_entries: Vec<(String, u64)> = inner
        .heartbeats
        .iter()
        .map(|(label, &last_ping_ms)| (label.clone(), last_ping_ms))
        .collect();

    let process_evidence = inner
        .last_vanish_ms
        .map(|at| now.saturating_sub(at) <= PROCESS_EVIDENCE_WINDOW_MS)
        .unwrap_or(false);
    let expect_browser = inner.browser_seen;
    let recovery = inner.recovery.clone();
    drop(inner);

    // 窗口可见性要在释放锁之后再查：查询会走 Tauri 的 manager 锁，
    // 不该在看门狗自己的锁里面再套一层。
    let heartbeats: Vec<WindowHeartbeat> = heartbeat_entries
        .into_iter()
        .map(|(label, last_ping_ms)| WindowHeartbeat {
            visible: is_window_observable(app, &label),
            label,
            last_ping_ms,
        })
        .collect();

    // 周期性「我还活着」日志：这是看门狗线程自身的存活证明，
    // 顺带把每个窗口的心跳延迟打出来，排查时不用再猜。
    let tick_count = {
        let state = app.state::<WebviewGuardState>();
        let mut inner = lock_inner(&state);
        inner.tick_count = inner.tick_count.saturating_add(1);
        inner.tick_count
    };
    if tick_count % 30 == 0 {
        let detail = heartbeats
            .iter()
            .map(|heartbeat| {
                format!(
                    "{}:{}ms",
                    heartbeat.label,
                    now.saturating_sub(heartbeat.last_ping_ms)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        log::info!(
            "webview guard: alive, tick={tick_count}, heartbeat_interval_ms={HEARTBEAT_INTERVAL_MS}, browser_alive={}, windows=[{detail}]",
            snapshot.has_browser()
        );
    }

    // 3) 恢复流程的进展。
    if let RecoveryState::ReloadAttempted { label, started_ms } = recovery {
        let resumed = heartbeats
            .iter()
            .find(|heartbeat| heartbeat.label == label)
            .map(|heartbeat| now.saturating_sub(heartbeat.last_ping_ms) < HEARTBEAT_TIMEOUT_MS)
            .unwrap_or(true);

        if resumed {
            log::info!("webview guard: window {label} is alive again after the reload");
            emit_notice(app, "recovered", &label, "", true);
            reset_recovery(app);
        } else if now.saturating_sub(started_ms) >= RECOVERY_DEADLINE_MS {
            escalate_to_prompt(app, &label);
            reset_recovery(app);
        } else {
            // 还在等：这一轮不要再发起第二次 reload。
            return;
        }
    }

    // 4) 判定。
    let verdict = decide_health(
        now,
        &heartbeats,
        &snapshot,
        expect_browser,
        process_evidence,
    );

    match verdict {
        WebviewGuardVerdict::Healthy => {
            let state = app.state::<WebviewGuardState>();
            let mut inner = lock_inner(&state);
            inner.browser_gone_ticks = 0;
            // 注意：这里**不能**解锁提示框闩锁。Healthy 只代表「暂时还没判成故障」，
            // 渲染进程死掉之后、还没到 60s 卡死门槛的那段时间里判定也是 Healthy
            // ⇒ 在故障还没结束的时候解锁，下一轮就会再弹一次框。
            // 解锁的唯一依据是「前端又发心跳了」（见 webview_guard_heartbeat）。
        }
        WebviewGuardVerdict::BrowserProcessGone => {
            let state = app.state::<WebviewGuardState>();
            let mut inner = lock_inner(&state);
            inner.browser_gone_ticks = inner.browser_gone_ticks.saturating_add(1);
            let ticks = inner.browser_gone_ticks;
            drop(inner);

            if ticks >= BROWSER_GONE_CONFIRM_TICKS {
                handle_browser_gone(app);
            }
        }
        WebviewGuardVerdict::RendererCrashed { label, missed_ms } => {
            attempt_recovery(app, &label, missed_ms, "renderer-crashed");
        }
        WebviewGuardVerdict::WindowHung { label, missed_ms } => {
            attempt_recovery(app, &label, missed_ms, "window-hung");
        }
    }
}

fn reset_recovery(app: &AppHandle) {
    let state = app.state::<WebviewGuardState>();
    let mut inner = lock_inner(&state);
    inner.recovery = RecoveryState::Idle;
}

/// 自动恢复：先 reload 窗口，等它的心跳回来。
fn attempt_recovery(app: &AppHandle, label: &str, missed_ms: u64, kind: &str) {
    let now = now_ms();

    {
        let state = app.state::<WebviewGuardState>();
        let mut inner = lock_inner(&state);
        if inner.recovery != RecoveryState::Idle {
            return;
        }
        inner.recovery = RecoveryState::ReloadAttempted {
            label: label.to_string(),
            started_ms: now,
        };
    }

    log::error!(
        "webview guard: {kind} detected for window {label} (no heartbeat for {missed_ms} ms), reloading it"
    );
    emit_notice(app, kind, label, "", true);

    let Some(window) = app.get_webview_window(label) else {
        // 窗口已经不在了 ⇒ 应用正在退出，不打扰用户。
        log::warn!("webview guard: window {label} is gone; the app is probably shutting down");
        reset_recovery(app);
        return;
    };

    if window.reload().is_err() {
        log::error!("webview guard: failed to reload window {label}");
        reset_recovery(app);
        escalate_to_prompt(app, label);
    }
}

/// 恢复失败 ⇒ 升级成不依赖 webview 的原生提示。
fn escalate_to_prompt(app: &AppHandle, label: &str) {
    if is_shutting_down(app) || !main_window_alive(app) {
        log::warn!("webview guard: skip the native prompt for {label}; the app is shutting down");
        return;
    }

    if !claim_native_prompt(app) {
        log::warn!("webview guard: the native prompt for {label} was already shown; skip");
        return;
    }

    log::error!("webview guard: recovery failed for window {label}; prompting the user");
    emit_notice(app, "recovery-failed", label, "", false);
    prompt_user(app, label);
}

/// 抢「弹原生提示框」的名额。同一场故障只允许弹一次。
fn claim_native_prompt(app: &AppHandle) -> bool {
    let state = app.state::<WebviewGuardState>();
    let mut inner = lock_inner(&state);
    if inner.native_prompt_shown {
        return false;
    }
    inner.native_prompt_shown = true;
    true
}

/// WebView2 浏览器进程没了 ⇒ 所有 webview 一起死，reload 也没用。
fn handle_browser_gone(app: &AppHandle) {
    if is_shutting_down(app) || !main_window_alive(app) {
        log::warn!("webview guard: the browser process is gone but the app is shutting down; ignore it");
        return;
    }

    if !claim_native_prompt(app) {
        log::warn!("webview guard: the native prompt was already shown; skip");
        return;
    }

    log::error!(
        "webview guard: the WebView2 browser process is gone; every webview of this app is dead"
    );
    emit_notice(app, "browser-gone", "main", "", false);
    prompt_user(app, "main");
}

/// 主窗口还在不在。
///
/// 口径 = 原生 HWND 是否仍是有效窗口。
///
/// 不能用 get_webview_window("main").is_some()：浏览器进程死亡时 wry 可能把
/// webview 窗口对象一起摘掉，那样看门狗会在每轮开头就返回，
/// 「浏览器进程没了 ⇒ 提示用户重启」这条路根本走不到。
fn main_window_alive(app: &AppHandle) -> bool {
    let raw = app.state::<WebviewGuardState>().main_hwnd.load(Ordering::Relaxed);

    if raw == 0 {
        // 还没抓到句柄 ⇒ 退回到 Tauri 的窗口表。
        return app.get_webview_window("main").is_some();
    }

    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::IsWindow;

        let hwnd = HWND(raw as *mut core::ffi::c_void);
        return unsafe { IsWindow(hwnd) }.as_bool();
    }

    #[cfg(not(windows))]
    {
        app.get_webview_window("main").is_some()
    }
}

/// 窗口是不是「看得见的」：可见且没有最小化。
///
/// 隐藏 / 最小化的窗口，Chromium 会限流它的定时器，心跳间隔变长是限流的产物，
/// 不是界面卡住 ⇒ 这类窗口不参与「卡死」判定。
fn is_window_observable(app: &AppHandle, label: &str) -> bool {
    app.get_webview_window(label)
        .map(|window| {
            let visible = window.is_visible().unwrap_or(false);
            let minimized = window.is_minimized().unwrap_or(false);
            visible && !minimized
        })
        .unwrap_or(false)
}

fn emit_notice(app: &AppHandle, kind: &str, label: &str, detail: &str, recoverable: bool) {
    let notice = WebviewGuardNotice {
        kind: kind.to_string(),
        label: label.to_string(),
        detail: detail.to_string(),
        recoverable,
    };

    if let Err(error) = app.emit(WEBVIEW_GUARD_EVENT, notice) {
        log::warn!("webview guard: failed to emit the notice: {error}");
    }
}

/// 原生提示框。
///
/// 崩的是主窗口渲染进程时，网页里的 UI 也没了 ⇒ toast 这条路不存在，
/// 只能用系统对话框。按钮只有「是 / 否」两个，用系统本地化的文字，
/// 语义写在正文里。
#[cfg(windows)]
fn prompt_user(app: &AppHandle, label: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_ICONERROR, MB_TOPMOST, MB_YESNO,
    };

    // 自己拼 UTF-16 缓冲：MessageBoxW 要的是以 NUL 结尾的宽字符串，
    // 直接拼比绕 HSTRING 的所有权更直白。
    let text = wide_string(&format!(
        "Alpha File Manager 的界面渲染进程已崩溃（窗口 {label}），无法自动恢复。\n\n是否重启应用？"
    ));
    let caption = wide_string("Alpha File Manager");

    let answer = unsafe {
        MessageBoxW(
            HWND(std::ptr::null_mut()),
            PCWSTR(text.as_ptr()),
            PCWSTR(caption.as_ptr()),
            MB_YESNO | MB_ICONERROR | MB_TOPMOST,
        )
    };

    if answer.0 == IDYES.0 {
        restart_app(app);
    }
}

/// 拼一个以 NUL 结尾的 UTF-16 宽字符串。
#[cfg(windows)]
fn wide_string(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(not(windows))]
fn prompt_user(_app: &AppHandle, label: &str) {
    log::error!(
        "webview guard: the webview of window {label} is broken; a native prompt is only implemented on Windows"
    );
}

/// 重启应用。
fn restart_app(app: &AppHandle) {
    if let Ok(exe) = std::env::current_exe() {
        spawn_delayed_restart(&exe);
    }

    app.exit(0);
}

/// 延迟重启。
///
/// 直接 `spawn` 新实例会被单实例插件截胡（新实例把启动参数转发给老实例就退出了），
/// 而老实例此刻正是坏的那个 ⇒ 必须等老进程真的退出之后再拉起新的。
#[cfg(windows)]
fn spawn_delayed_restart(exe: &std::path::Path) {
    use std::os::windows::process::CommandExt;

    // CREATE_NO_WINDOW
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let script = format!(
        "ping -n 3 127.0.0.1 >nul & start \"\" \"{}\"",
        exe.display()
    );

    let _ = std::process::Command::new("cmd")
        .args(["/C", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

#[cfg(not(windows))]
fn spawn_delayed_restart(_exe: &std::path::Path) {}
