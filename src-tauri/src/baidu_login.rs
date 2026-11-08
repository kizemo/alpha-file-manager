// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! 内嵌 WebView2 登录百度网盘。
//!
//! 流程：`remote_baidu_login_start` 打开一个指向 `pan.baidu.com` 的子窗口，
//! 后台线程每 2 秒用 `cookies_for_url` 读一次 cookie；读到 `BDUSS` 就存进
//! 进程内的待认领槽位并发事件通知前端。前端拿到事件后调用
//! `remote_baidu_login_claim`，把登录态加密落盘到当前连接。
//!
//! 权限边界：cookie 明文只在本模块与 `remote::baidu_auth` 之间流转，
//! 不出现在事件 payload、命令返回值与日志里。
//!
//! 注意：本模块**不得**引用 `tauri::async_runtime::*`。`cargo test --lib`
//! 会链接整个 crate，`GlobalRuntime<Wry>` 一旦被单态化就会把 tao 拖进来，
//! 测试二进制加载期直接 `STATUS_ENTRYPOINT_NOT_FOUND (0xC0000139)`。
//! 因此这里的命令全部是同步实现，需要「另起线程」时用 `std::thread`。
//!
//! ## 窗口必须在独立线程里建（2026-11-09 定案）
//!
//! `tauri-runtime-wry` 的 `create_webview` 上有一行原文注释：
//! *this must be called from a separate thread, otherwise the channel will
//! introduce a deadlock*。
//!
//! 命令是从 WebView2 的 IPC 回调里进来的 —— 那就是**事件循环线程**。
//! 在那里直接 `WebviewWindowBuilder::build()` 会：
//!
//! 1. 窗口体（HWND）当场建出来，看起来一切正常；
//! 2. 但 webview 的创建被**派发回事件循环**并等它的回复；
//! 3. 事件循环此刻正卡在 IPC 回调里（回调不返回，回复就不会来）——死锁。
//!
//! 于是登录窗永远是一块白板，`cookies_for_url` 也永远等不到结果，
//! 超时分支同样走不到，窗口会一直挂着。
//!
//! 所以这里把建窗放进 `std::thread::spawn`：命令立刻返回，事件循环空出来，
//! 派发出去的建窗请求才排得上队。

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::remote::{
    describe_state, save_baidu_session, scan_cookies, unix_now, BaiduCookieOutcome,
    BaiduCookieRecord, BaiduSessionState, BaiduStokenState, RemoteError,
};

/// 登录子窗口的固定 label。`start` 靠它判重，轮询线程靠它判存活。
const LOGIN_WINDOW_LABEL: &str = "afm-baidu-login";
/// 登录页。`STOKEN` 只在这个路径下发。
const LOGIN_START_URL: &str = "https://pan.baidu.com/disk/main";
/// cookie 取值锚点，覆盖 `.pan.baidu.com` 与 `.baidu.com` 两个域。
const LOGIN_COOKIE_URL: &str = "https://pan.baidu.com/";
/// 前端订阅的事件名。`tauri://` 前缀由 Tauri 保留，故用自定义协议名。
const LOGIN_EVENT: &str = "afm://baidu-login";
/// `STOKEN` 的 cookie 名。
const STOKEN_NAME: &str = "STOKEN";

const POLL: Duration = Duration::from_secs(2);
const GIVE_UP: Duration = Duration::from_secs(900);
const STOKEN_WAIT: Duration = Duration::from_secs(20);

/// 单次读 cookie 的时间上限。
///
/// `cookies_for_url` 在 Windows 上走 wry 的 `wait_with_pump`，它会阻塞在
/// `GetMessageA` 上等 WebView2 的回调。万一回调没送达，这一次读就永远挂着，
/// 于是「超时」分支永远走不到、登录窗也永远关不掉 —— 这一轮实测踩过。
/// 所以读 cookie 也要另起线程并带超时。
const COOKIE_READ_BUDGET: Duration = Duration::from_secs(10);
/// 连续读失败多少次就放弃这一轮登录。
const MAX_READ_FAILURES: u32 = 3;

/// 诊断日志的环境变量；不给就用应用数据目录下的默认文件名。
const LOG_ENV: &str = "AFM_BAIDU_LOG";
const DEFAULT_LOG_NAME: &str = "baidu-login.log";

/// 已读到、但还没被任何连接认领的登录态。
///
/// `BaiduSession` 本身没有 `stoken()` 访问器，所以 `STOKEN` 是否存在
/// 要在这里显式记下来。
struct PendingSession {
    state: BaiduSessionState,
    has_stoken: bool,
}

type PendingSlot = Mutex<Option<PendingSession>>;

fn pending_slot() -> &'static PendingSlot {
    static PENDING: OnceLock<PendingSlot> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(None))
}

fn pending_lock() -> std::sync::MutexGuard<'static, Option<PendingSession>> {
    // 锁中毒时直接取回内部值：这里没有「写一半」的中间态需要保护。
    pending_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn take_pending() -> Option<PendingSession> {
    pending_lock().take()
}

fn store_pending(pending: PendingSession) {
    *pending_lock() = Some(pending);
}

fn clear_pending() {
    *pending_lock() = None;
}

/// 本轮登录是否已经「程序化收场」。
///
/// 成功 / 超时 / 出错都会先把它置 true，随后关闭窗口；这样
/// `WindowEvent::Destroyed` 就不会再补一条「用户关掉了窗口」。
static SETTLED: AtomicBool = AtomicBool::new(false);

/// 当前轮次代号。每开一轮登录就 +1，轮询线程发现自己不是最新那轮就收工。
static POLLER_GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BaiduLoginEvent {
    /// `ready` | `closed` | `timeout` | `error`
    status: &'static str,
    has_stoken: bool,
    summary: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaiduLoginStartPayload {
    pub opened: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaiduLoginPendingPayload {
    pub pending: bool,
    pub has_stoken: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaiduLoginClaimPayload {
    pub claimed: bool,
    pub summary: String,
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, RemoteError> {
    crate::app_dirs::app_data_dir(app)
        .map_err(|err| RemoteError::unknown(format!("无法定位应用数据目录：{err}")))
}

fn emit_event(
    app: &AppHandle,
    status: &'static str,
    has_stoken: bool,
    summary: Option<String>,
    message: Option<&str>,
) {
    let _ = app.emit(
        LOGIN_EVENT,
        BaiduLoginEvent {
            status,
            has_stoken,
            summary,
            message: message.map(str::to_string),
        },
    );
}

fn settle() {
    SETTLED.store(true, Ordering::SeqCst);
}

fn close_login_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LOGIN_WINDOW_LABEL) {
        let _ = window.close();
    }
}

/// 诊断日志的文件路径。**只写状态，不写 cookie 明文。**
fn log_path(app: &AppHandle) -> Option<PathBuf> {
    static LOG_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    LOG_PATH
        .get_or_init(|| {
            if let Ok(value) = std::env::var(LOG_ENV) {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    return Some(PathBuf::from(trimmed));
                }
            }
            crate::app_dirs::app_data_dir(app)
                .ok()
                .map(|dir| dir.join(DEFAULT_LOG_NAME))
        })
        .clone()
}

fn log_line(app: &AppHandle, message: &str) {
    let Some(path) = log_path(app) else {
        return;
    };
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| format!("{}.{:03}", value.as_secs(), value.subsec_millis()))
        .unwrap_or_else(|_| "?".to_string());
    let line = format!("[{stamp}] {message}\n");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = file.write_all(line.as_bytes());
        let _ = file.flush();
    }
}

/// 打开（或聚焦）登录窗口。
#[tauri::command]
pub fn remote_baidu_login_start(app: AppHandle) -> Result<BaiduLoginStartPayload, RemoteError> {
    // 新一轮登录：上一轮的待认领会话与收场标记都作废。
    SETTLED.store(false, Ordering::SeqCst);
    clear_pending();
    log_line(&app, "login_start requested");

    if let Some(existing) = app.get_webview_window(LOGIN_WINDOW_LABEL) {
        let _ = existing.set_focus();
        // 窗口还在，但上一轮的轮询线程可能已经收工了 ——
        // 重新点一次必须能重新开始等，否则永远等不到登录态。
        start_poller(app.clone(), existing);
        return Ok(BaiduLoginStartPayload { opened: true });
    }

    spawn_window_builder(app.clone());
    Ok(BaiduLoginStartPayload { opened: true })
}

/// 在独立线程里建登录窗（原因见模块顶部注释）。
fn spawn_window_builder(app: AppHandle) {
    std::thread::spawn(move || {
        let url = match tauri::Url::parse(LOGIN_START_URL) {
            Ok(url) => url,
            Err(err) => {
                log_line(&app, &format!("login url invalid: {err}"));
                settle();
                emit_event(
                    &app,
                    "error",
                    false,
                    None,
                    Some("登录地址无效，无法打开登录窗口。"),
                );
                return;
            }
        };

        let window = match WebviewWindowBuilder::new(&app, LOGIN_WINDOW_LABEL, WebviewUrl::External(url))
            .title("百度网盘登录")
            .inner_size(1100.0, 820.0)
            .build()
        {
            Ok(window) => window,
            Err(err) => {
                log_line(&app, &format!("login window build failed: {err}"));
                settle();
                emit_event(
                    &app,
                    "error",
                    false,
                    None,
                    Some("打开百度网盘登录窗口失败，请重试。"),
                );
                return;
            }
        };
        log_line(&app, "login window built");

        let watcher = app.clone();
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Destroyed) && !SETTLED.swap(false, Ordering::SeqCst) {
                emit_event(
                    &watcher,
                    "closed",
                    false,
                    None,
                    Some("登录窗口已关闭。"),
                );
            }
        });

        start_poller(app.clone(), window);
    });
}

/// 查询是否有待认领的登录态（前端打开表单时用来兜底）。
#[tauri::command]
pub fn remote_baidu_login_pending() -> BaiduLoginPendingPayload {
    let guard = pending_lock();
    match guard.as_ref() {
        None => BaiduLoginPendingPayload {
            pending: false,
            has_stoken: false,
            summary: String::new(),
        },
        Some(pending) => BaiduLoginPendingPayload {
            pending: true,
            has_stoken: pending.has_stoken,
            summary: describe_state(Some(&pending.state)),
        },
    }
}

/// 把待认领的登录态加密落盘到指定连接。
#[tauri::command]
pub fn remote_baidu_login_claim(
    app: AppHandle,
    connection_id: String,
) -> Result<BaiduLoginClaimPayload, RemoteError> {
    let id = connection_id.trim().to_string();
    let Some(pending) = take_pending() else {
        log_line(&app, "claim: no pending session");
        return Ok(BaiduLoginClaimPayload {
            claimed: false,
            summary: String::new(),
        });
    };

    if id.is_empty() {
        store_pending(pending);
        return Err(RemoteError::unknown("连接尚未建立，无法保存百度网盘登录态。"));
    }

    let PendingSession { state, has_stoken } = pending;
    let summary = describe_state(Some(&state));
    let dir = app_data_dir(&app)?;
    if let Err(err) = save_baidu_session(&dir, &id, &state.session, state.stoken_expires_unix) {
        // 落盘失败就把会话放回去，用户不用重新登录一次。
        store_pending(PendingSession { state, has_stoken });
        log_line(&app, &format!("claim: save failed: {err}"));
        return Err(err);
    }

    log_line(&app, &format!("claim: saved to connection {id}"));
    Ok(BaiduLoginClaimPayload {
        claimed: true,
        summary,
    })
}

/// 用户主动取消登录。
#[tauri::command]
pub fn remote_baidu_login_cancel(app: AppHandle) {
    settle();
    clear_pending();
    close_login_window(&app);
}

fn start_poller(app: AppHandle, window: tauri::WebviewWindow) {
    let generation = POLLER_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    log_line(&app, &format!("poller started (generation={generation})"));
    std::thread::spawn(move || poll_loop(app, window, generation));
}

/// 一次读 cookie 的结果。`Timeout` 与 `Failed` 分开，是因为
/// 「读挂住了」和「读报错了」要分开记 —— 前者是死锁的味道。
enum CookieRead {
    Ok(Vec<tauri::webview::Cookie<'static>>),
    Failed(String),
    Timeout,
}

/// 读 cookie：另起线程 + 超时，绝不让主轮询线程被挂死。
fn read_cookies(
    window: &tauri::WebviewWindow,
    url: tauri::Url,
    budget: Duration,
) -> CookieRead {
    let (tx, rx) = std::sync::mpsc::channel();
    let worker_window = window.clone();
    let worker_url = url.clone();
    std::thread::spawn(move || {
        let outcome = worker_window
            .cookies_for_url(worker_url)
            .map_err(|err| err.to_string());
        let _ = tx.send(outcome);
    });

    match rx.recv_timeout(budget) {
        Ok(Ok(cookies)) => CookieRead::Ok(cookies),
        Ok(Err(err)) => CookieRead::Failed(err),
        Err(_) => CookieRead::Timeout,
    }
}

fn poll_loop(app: AppHandle, window: tauri::WebviewWindow, generation: u64) {
    let started = Instant::now();
    let url = match tauri::Url::parse(LOGIN_COOKIE_URL) {
        Ok(url) => url,
        Err(_) => {
            settle();
            emit_event(
                &app,
                "error",
                false,
                None,
                Some("登录地址无效，无法读取登录状态。"),
            );
            close_login_window(&app);
            return;
        }
    };

    let mut stoken_deadline: Option<Instant> = None;
    let mut read_failures: u32 = 0;

    loop {
        std::thread::sleep(POLL);

        // 被新一轮登录取代了 —— 收工。
        if POLLER_GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }

        // 窗口已经没了（用户关闭 / cancel / 程序化收场）就直接收工，
        // 否则线程会空转到 900 秒。
        if app.get_webview_window(LOGIN_WINDOW_LABEL).is_none() {
            return;
        }

        // 超时判定必须排在读 cookie **之前**：读一旦挂住，
        // 排在后面的超时分支就永远走不到。
        if started.elapsed() >= GIVE_UP {
            log_line(&app, "give up: timeout");
            settle();
            emit_event(&app, "timeout", false, None, Some("登录等待超时，请重试。"));
            close_login_window(&app);
            return;
        }

        match read_cookies(&window, url.clone(), COOKIE_READ_BUDGET) {
            CookieRead::Timeout => {
                read_failures += 1;
                log_line(&app, &format!("cookies read timed out ({read_failures})"));
                if read_failures >= MAX_READ_FAILURES {
                    log_line(&app, "give up: cookies unreadable");
                    settle();
                    emit_event(
                        &app,
                        "error",
                        false,
                        None,
                        Some("读取登录状态失败，请重试。"),
                    );
                    close_login_window(&app);
                    return;
                }
            }
            CookieRead::Failed(err) => {
                read_failures += 1;
                log_line(&app, &format!("cookies read failed ({read_failures}): {err}"));
            }
            CookieRead::Ok(cookies) => {
                read_failures = 0;

                let mut records = Vec::with_capacity(cookies.len());
                let mut stoken_expires_unix: Option<u64> = None;
                for cookie in &cookies {
                    if cookie.name() == STOKEN_NAME {
                        // `cookie 0.18` 把 `max_age()` 从 `std::time::Duration` 换成了
                        // `time::Duration`（`SignedDuration`），没有 `as_secs()`，
                        // 只有 `whole_seconds()`（i64）。负值按「已过期」夹到 0。
                        stoken_expires_unix = cookie.max_age().map(|age| {
                            unix_now().saturating_add(age.whole_seconds().max(0) as u64)
                        });
                    }
                    records.push(BaiduCookieRecord::new(cookie.name(), cookie.value()));
                }

                let scan = scan_cookies(&records);
                if !matches!(scan.outcome, BaiduCookieOutcome::Ready) {
                    continue;
                }

                let has_stoken = !matches!(scan.stoken, BaiduStokenState::Absent);
                let Some(session) = scan.session else {
                    continue;
                };

                // BDUSS 已经到手，但 STOKEN 还没下发：再等一会儿，
                // 刚跳到磁盘页那几秒 STOKEN 可能还没写进 cookie 罐。
                if !has_stoken {
                    match stoken_deadline {
                        None => {
                            stoken_deadline = Some(Instant::now());
                            continue;
                        }
                        Some(deadline) if deadline.elapsed() < STOKEN_WAIT => continue,
                        _ => {}
                    }
                }

                let state = BaiduSessionState::new(session, stoken_expires_unix);
                let summary = describe_state(Some(&state));
                log_line(&app, &format!("login state ready: {summary}"));
                store_pending(PendingSession { state, has_stoken });
                settle();
                emit_event(&app, "ready", has_stoken, Some(summary), None);
                close_login_window(&app);
                return;
            }
        }
    }
}
