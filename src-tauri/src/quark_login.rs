// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! 内嵌 WebView2 登录夸克网盘。
//!
//! 流程与 `baidu_login.rs` 同构：`remote_quark_login_start` 打开一个指向
//! `pan.quark.cn` 的子窗口，后台线程每 2 秒读一次 cookie；读到 `__pus` 就把
//! 会话放进进程内的待认领槽位并发事件通知前端。前端拿到事件后调用
//! `remote_quark_login_claim`，把登录态加密落盘到当前连接。
//!
//! 权限边界：cookie 明文只在本模块与 `remote::quark_auth` 之间流转，
//! 不出现在事件 payload、命令返回值与日志里。
//!
//! ⚠️ **不得引用 `tauri::async_runtime::*`**。`cargo test --lib` 会链接整个
//! crate，`GlobalRuntime<Wry>` 一旦被单态化就会把 tao 拖进测试二进制，
//! 进程在**加载阶段**以 `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉。
//! 因此这里的命令全部是同步实现，需要「另起线程」时用 `std::thread`。
//!
//! ## 窗口必须在独立线程里建
//!
//! `tauri-runtime-wry` 的 `create_webview` 上有一行原文注释：
//! *this must be called from a separate thread, otherwise the channel will
//! introduce a deadlock*。命令是从 WebView2 的 IPC 回调里进来的 —— 那是
//! **事件循环线程** —— 在那里直接 `WebviewWindowBuilder::build()` 会：
//!
//! 1. 窗口体（HWND）当场建出来，看起来一切正常；
//! 2. 但 webview 的创建被**派发回事件循环**并等它的回复；
//! 3. 事件循环此刻正卡在 IPC 回调里（回调不返回，回复就不会来）—— 死锁。
//!
//! 于是登录窗永远是一块白板，`cookies_for_url` 也永远等不到结果。
//! 所以这里把建窗放进 `std::thread::spawn`：命令立刻返回，事件循环空出来，
//! 派发出去的建窗请求才排得上队。
//!
//! ## 与百度侧的差异（每一条都有证据，不是随手改的）
//!
//! 1. **锚点是 `__pus`，没有 STOKEN** —— 夸克只有这一个会话 cookie
//!    （`quark_auth::QUARK_COOKIE_PUS`）。
//! 2. **一次读多个域** —— `__pus` 挂在 `.quark.cn` 还是只在
//!    `passport.quark.cn` 上**未实证**（方案 §7-U1）。只读一个域，漏了就是
//!    「用户明明登录成功了，界面却一直说没登录」。
//! 3. **不做「基线同值 ⇒ 一票否决」** —— 百度那条保险针对的是 jar 里遗留的
//!    **死** BDUSS。夸克这边每轮用**独立 cookie 罐**（罐子天生是空的），
//!    遗留凭据的问题不存在；而一个**仍然有效**的旧 `__pus` 恰好是我们要的。
//!    所以这里只保留「保险一」（清罐），不保留「保险二」。
//! 4. **验活探针只记日志，不当否决** —— T0 实测：**游客态**打列表接口也是
//!    `401 + code=31001`。也就是说 401 **不能**证明 cookie 死了，只证明
//!    「这次请求没被认」。用它否决 ⇒ 一旦参数不全就把真登录挡在门外。
//!    探针真正的用途是给 T0 的 U4 一个**真实**答案（拿到 `200 + code=0`
//!    ⇒ 主路线成立），以及决定要不要记 `last_alive_unix`。

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::remote::{
    quark_default_device_model, quark_default_ut, quark_describe_state, quark_list_dir_url,
    quark_scan_cookies, quark_session_to_cookie_header, save_quark_session, QuarkAuthSession,
    QuarkCookieOutcome, QuarkCookieRecord, QuarkSessionState, RemoteError, QUARK_COOKIE_PUS,
};

/// 登录子窗口的固定 label。`start` 靠它判重，轮询线程靠它判存活。
const LOGIN_WINDOW_LABEL: &str = "afm-quark-login";

/// 登录页入口。
///
/// T0 已实证：`https://pan.quark.cn/` 会渲染**二维码登录页**（游客态页面正文
/// 含「二维码过期，请刷新」——那是没扫码时的正常形态）。
/// 方案 §7-U3 留了两个候选（`pan.quark.cn/` 与 `su.quark.cn/4_eMHBJ`），
/// 这里先用前者；U3 的结论出来后**只改这一个常量**。
const LOGIN_START_URL: &str = "https://pan.quark.cn/";

/// 前端订阅的事件名。`tauri://` 前缀由 Tauri 保留，故用自定义协议名。
const LOGIN_EVENT: &str = "afm://quark-login";

/// 独立 cookie 罐的根目录（应用数据目录下的子目录）。
const LOGIN_PROFILE_ROOT: &str = "quark-login-profile";

/// 轮询间隔。
const POLL: Duration = Duration::from_secs(2);
/// 等待登录的总时限。
const GIVE_UP: Duration = Duration::from_secs(900);
/// 单次读 cookie 的时间上限。
///
/// `cookies_for_url` 在 Windows 上走 wry 的 `wait_with_pump`，它会阻塞在
/// `GetMessageA` 上等 WebView2 的回调。万一回调没送达，这一次读就永远挂着，
/// 于是「超时」分支永远走不到、登录窗也永远关不掉。所以读 cookie 也要另起
/// 线程并带超时。
const COOKIE_READ_BUDGET: Duration = Duration::from_secs(12);
/// 连续读失败多少次就放弃这一轮登录。
const MAX_READ_FAILURES: u32 = 3;
/// 验活探针的时间上限。
const VERIFY_BUDGET: Duration = Duration::from_secs(8);

/// 读 cookie 的候选 URL。
///
/// 顺序有意义：**同名取第一条**（见 `dedupe_cookie_records`），所以把登录页
/// 所在的 `pan.quark.cn` 排在最前面。
const COOKIE_URLS: [&str; 3] = [
    "https://pan.quark.cn/",
    "https://www.quark.cn/",
    "https://passport.quark.cn/",
];

/// 诊断日志的环境变量；不给就用应用数据目录下的默认文件名。
const LOG_ENV: &str = "AFM_QUARK_LOG";
const DEFAULT_LOG_NAME: &str = "quark-login.log";

/// 已读到、但还没被任何连接认领的登录态。
struct PendingSession {
    state: QuarkSessionState,
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

/// 登录进度事件。**不携带任何 cookie 明文**。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuarkLoginEvent {
    /// `ready` | `closed` | `timeout` | `error`
    status: &'static str,
    /// 脱敏描述（只有 cookie 的名字与长度）。
    summary: Option<String>,
    /// 面向用户的短说明。
    message: Option<String>,
}

/// 登录窗是否已打开。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarkLoginStartPayload {
    pub opened: bool,
}

/// 有没有待认领的登录态。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarkLoginPendingPayload {
    pub pending: bool,
    /// 脱敏描述（只有 cookie 的名字与长度，没有内容）。
    pub summary: String,
}

/// 认领结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarkLoginClaimPayload {
    pub claimed: bool,
    /// 脱敏描述。
    pub summary: String,
}

/// 当前 Unix 秒。
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, RemoteError> {
    crate::app_dirs::app_data_dir(app)
        .map_err(|err| RemoteError::unknown(format!("无法定位应用数据目录：{err}")))
}

/// 本轮登录专用的 webview 数据目录。
///
/// 登录窗若与主窗口共用 data-dir，打开时必然继承罐里**上一轮**留下的夸克
/// cookie —— 用户看到的是「已登录的旧账号」。既然实测证明「建窗后主动删
/// cookie」不可靠（百度侧 2026-11-09 真机日志），就改成**每轮一支空罐**：
/// 罐里本来就没有 `__pus`，「保险二」自然失效。
fn login_profile_dir(base: &Path, stamp_ms: u64) -> PathBuf {
    base.join(LOGIN_PROFILE_ROOT)
        .join(format!("session-{stamp_ms}"))
}

/// 清扫上一轮遗留的 profile 子目录。
///
/// 只在「确定没有在用登录窗」时调用（`remote_quark_login_start` 已保证此时
/// 窗口不存在），所以整棵 `session-*` 都是死数据。删除失败只记日志。
fn sweep_login_profiles(app: &AppHandle, base: &Path) {
    let root = base.join(LOGIN_PROFILE_ROOT);
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };
    let mut removed = 0usize;
    let mut failed = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let is_session = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with("session-"))
            .unwrap_or(false);
        if !is_session {
            continue;
        }
        match std::fs::remove_dir_all(&path) {
            Ok(()) => removed += 1,
            Err(_) => failed += 1,
        }
    }
    if removed > 0 || failed > 0 {
        log_line(
            app,
            &format!("login profile 清扫：删掉 {removed} 个旧 session 目录，失败 {failed} 个"),
        );
    }
}

/// 取一支本轮专用的、已经建好的空 cookie 罐。
///
/// 失败一律返回 `None`，调用方退回共享 data-dir（并写日志说明）——
/// 宁可沿用旧行为，也不要让「换账号登录」因为建目录失败而彻底不可用。
fn prepare_login_profile(app: &AppHandle) -> Option<PathBuf> {
    let base = match app_data_dir(app) {
        Ok(dir) => dir,
        Err(err) => {
            log_line(
                app,
                &format!("login profile: 拿不到应用数据目录，退回共享 jar: {err}"),
            );
            return None;
        }
    };
    sweep_login_profiles(app, &base);
    let stamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0);
    let dir = login_profile_dir(&base, stamp_ms);
    match std::fs::create_dir_all(&dir) {
        Ok(()) => {
            log_line(
                app,
                &format!("login profile: 本轮独立 cookie 罐 {}", dir.display()),
            );
            Some(dir)
        }
        Err(err) => {
            log_line(
                app,
                &format!(
                    "login profile: 建目录失败，退回共享 jar（{}）: {err}",
                    dir.display()
                ),
            );
            None
        }
    }
}

fn emit_event(
    app: &AppHandle,
    status: &'static str,
    summary: Option<String>,
    message: Option<&str>,
) {
    let _ = app.emit(
        LOGIN_EVENT,
        QuarkLoginEvent {
            status,
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

/// 打开（或聚焦）夸克网盘登录窗口。
#[tauri::command]
pub fn remote_quark_login_start(app: AppHandle) -> Result<QuarkLoginStartPayload, RemoteError> {
    // 新一轮登录：上一轮的待认领会话与收场标记都作废。
    SETTLED.store(false, Ordering::SeqCst);
    clear_pending();
    log_line(&app, "login_start requested");

    if let Some(existing) = app.get_webview_window(LOGIN_WINDOW_LABEL) {
        let _ = existing.set_focus();
        // 窗口还在，但上一轮的轮询线程可能已经收工了 ——
        // 重新点一次必须能重新开始等，否则永远等不到登录态。
        start_poller(app.clone(), existing);
        return Ok(QuarkLoginStartPayload { opened: true });
    }

    spawn_window_builder(app.clone());
    Ok(QuarkLoginStartPayload { opened: true })
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
                    None,
                    Some("登录地址无效，无法打开登录窗口。"),
                );
                return;
            }
        };

        // 每轮登录换一支全新的 webview 数据目录：罐子天生是空的，
        // 一打开就是未登录态，用户直接扫码即可。
        let mut builder =
            WebviewWindowBuilder::new(&app, LOGIN_WINDOW_LABEL, WebviewUrl::External(url))
                .title("夸克网盘登录")
                .inner_size(1100.0, 820.0);
        if let Some(profile) = prepare_login_profile(&app) {
            builder = builder.data_directory(profile);
        }
        let window = match builder.build() {
            Ok(window) => window,
            Err(err) => {
                log_line(&app, &format!("login window build failed: {err}"));
                settle();
                emit_event(
                    &app,
                    "error",
                    None,
                    Some("打开夸克网盘登录窗口失败，请重试。"),
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
pub fn remote_quark_login_pending() -> QuarkLoginPendingPayload {
    let guard = pending_lock();
    match guard.as_ref() {
        None => QuarkLoginPendingPayload {
            pending: false,
            summary: String::new(),
        },
        Some(pending) => QuarkLoginPendingPayload {
            pending: true,
            summary: quark_describe_state(Some(&pending.state)),
        },
    }
}

/// 把待认领的登录态加密落盘到指定连接。
#[tauri::command]
pub fn remote_quark_login_claim(
    app: AppHandle,
    connection_id: String,
) -> Result<QuarkLoginClaimPayload, RemoteError> {
    let id = connection_id.trim().to_string();
    let Some(pending) = take_pending() else {
        log_line(&app, "claim: no pending session");
        return Ok(QuarkLoginClaimPayload {
            claimed: false,
            summary: String::new(),
        });
    };

    if id.is_empty() {
        store_pending(pending);
        return Err(RemoteError::unknown("连接尚未建立，无法保存夸克网盘登录态。"));
    }

    let PendingSession { state } = pending;
    let summary = quark_describe_state(Some(&state));
    let dir = app_data_dir(&app)?;
    if let Err(err) = save_quark_session(&dir, &id, &state.session, state.last_alive_unix) {
        // 落盘失败就把会话放回去，用户不用重新登录一次。
        store_pending(PendingSession { state });
        log_line(&app, &format!("claim: save failed: {err}"));
        return Err(err);
    }

    log_line(&app, &format!("claim: saved to connection {id}"));
    Ok(QuarkLoginClaimPayload {
        claimed: true,
        summary,
    })
}

/// 用户主动取消登录。
#[tauri::command]
pub fn remote_quark_login_cancel(app: AppHandle) -> Result<(), RemoteError> {
    settle();
    clear_pending();
    close_login_window(&app);
    Ok(())
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
///
/// 一次读**多个域**（`COOKIE_URLS`）：`__pus` 到底挂在哪个域上未实证，
/// 漏读一个域的后果是「用户明明登录成功了，界面却一直说没登录」。
fn read_cookies(window: &tauri::WebviewWindow) -> CookieRead {
    let (tx, rx) = std::sync::mpsc::channel();
    let worker_window = window.clone();
    std::thread::spawn(move || {
        let mut merged: Vec<tauri::webview::Cookie<'static>> = Vec::new();
        let mut failures: Vec<String> = Vec::new();
        for raw in COOKIE_URLS {
            let Ok(url) = tauri::Url::parse(raw) else {
                continue;
            };
            match worker_window.cookies_for_url(url) {
                Ok(cookies) => merged.extend(cookies),
                Err(err) => failures.push(format!("{raw}: {err}")),
            }
        }
        if merged.is_empty() && !failures.is_empty() {
            let _ = tx.send(CookieRead::Failed(failures.join("; ")));
            return;
        }
        let _ = tx.send(CookieRead::Ok(merged));
    });

    match rx.recv_timeout(COOKIE_READ_BUDGET) {
        Ok(CookieRead::Ok(cookies)) => CookieRead::Ok(cookies),
        Ok(CookieRead::Failed(err)) => CookieRead::Failed(err),
        Ok(CookieRead::Timeout) => CookieRead::Timeout,
        Err(_) => CookieRead::Timeout,
    }
}

/// 合并 cookie 清单：**同名取第一条**。
///
/// 口径与 `quark_auth::scan_cookies`（「同名取第一个」）一致 ——
/// WebView2 的清单顺序不保证稳定，「取最后一个」会让同一个会话在不同轮次里
/// 变成不同的值。
fn dedupe_cookie_records(records: Vec<QuarkCookieRecord>) -> Vec<QuarkCookieRecord> {
    let mut merged: Vec<QuarkCookieRecord> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for record in records {
        if seen.insert(record.name.clone()) {
            merged.push(record);
        }
    }
    merged
}

/// 保险一：建窗后第一件事 —— 读一次 cookie 罐，把可能残留的 `__pus` 删掉。
///
/// 用**独立 cookie 罐**时这一步天然是空操作（罐子本来是空的），
/// 正好可以当作「独立罐确实生效」的信号；退回共享 data-dir 时它才是有效兜底。
fn purge_stale_pus(app: &AppHandle, window: &tauri::WebviewWindow) {
    let cookies = match read_cookies(window) {
        CookieRead::Ok(cookies) => cookies,
        CookieRead::Timeout => {
            log_line(app, "purge: 读 cookie 罐超时，跳过清 jar");
            return;
        }
        CookieRead::Failed(err) => {
            log_line(app, &format!("purge: 读 cookie 罐失败，跳过清 jar: {err}"));
            return;
        }
    };
    let victims: Vec<_> = cookies
        .into_iter()
        .filter(|cookie| cookie.name() == QUARK_COOKIE_PUS)
        .collect();
    let total = victims.len();
    if total == 0 {
        log_line(app, "purge: jar 里没有 __pus 残留");
        return;
    }
    // 和 read_cookies 一样走一次性线程 + 带超时接收：`delete_cookie` 同样是
    // 会同步等事件循环的 API，不能让它把轮询线程钉死。
    let (tx, rx) = std::sync::mpsc::channel();
    let worker_window = window.clone();
    std::thread::spawn(move || {
        let mut failed = 0usize;
        for cookie in victims {
            if worker_window.delete_cookie(cookie).is_err() {
                failed += 1;
            }
        }
        let _ = tx.send(failed);
    });
    match rx.recv_timeout(COOKIE_READ_BUDGET) {
        Ok(failed) => log_line(
            app,
            &format!("purge: 尝试删掉 {total} 条旧 __pus，失败 {failed} 条"),
        ),
        Err(_) => log_line(app, "purge: 删除线程超时"),
    }
}

/// `QuarkCookieOutcome` 里「为什么还没 Ready」的可读原因。
///
/// 只用于诊断日志：轮询每 2 秒跑一次，把原因写清楚才能事后区分
/// 「用户还没扫码」（Missing）与「扫到了但值不合法」（Invalid）。
fn not_ready_reason(outcome: &QuarkCookieOutcome) -> Option<&'static str> {
    match outcome {
        QuarkCookieOutcome::Ready => None,
        QuarkCookieOutcome::Missing => Some("__pus 还没出现在 cookie 罐里"),
        QuarkCookieOutcome::Invalid => Some("__pus 值不合法"),
    }
}

/// 会话是否可用的判定结果。
///
/// ⚠️ **刻意没有 `Rejected` 这个变体**：T0 实测游客态也是 `401 + code=31001`，
/// 拿 401 当「会话已死」的证据会把真登录挡在门外（见模块顶部注释 4）。
#[derive(Debug, PartialEq, Eq)]
enum SessionVerdict {
    /// 服务端确认这个会话能用。
    Live,
    /// 判不出来。调用方按「放行」处理：探针自身失败不该打掉用户真的登录成功。
    Inconclusive(&'static str),
}

/// 从返回体里抠出 `code` 字段的值。
///
/// 手法与 `baidu_login::probe_errno` 一致（字节级，不做 JSON 反序列化）：
/// 夸克的 `code` 两种形态都出现过 —— 数字 `0` 与字符串 `"OK"`。
fn extract_code(body: &str) -> Option<String> {
    let tail = body.split_once("\"code\"")?.1;
    let tail = tail.trim_start();
    let tail = tail.strip_prefix(':')?.trim_start();
    if let Some(inner) = tail.strip_prefix('"') {
        let end = inner.find('"')?;
        return Some(inner[..end].to_string());
    }
    let end = tail
        .find(|character: char| !character.is_ascii_digit() && character != '-')
        .unwrap_or(tail.len());
    if end == 0 {
        return None;
    }
    Some(tail[..end].to_string())
}

/// 纯函数：按探针的 HTTP 状态码 + 返回体判定会话。
///
/// **只有确凿的成功证据（`200` + `code` 为 `0` 或 `"OK"`）才判 Live**；
/// 其余一律 Inconclusive。特别地，`401`/`403` **不**判失败 ——
/// T0 实测游客态也是 401，那只说明「这次请求没被认」。
fn judge_session_probe(status: u16, body: &str) -> SessionVerdict {
    if status != 200 {
        return SessionVerdict::Inconclusive("探针返回非 200，判不出会话状态");
    }
    match extract_code(body).as_deref() {
        Some("0") | Some("OK") => SessionVerdict::Live,
        Some(_) => SessionVerdict::Inconclusive("服务端返回了非成功的 code"),
        None => SessionVerdict::Inconclusive("读不出返回体里的 code"),
    }
}

/// 拿**即将**存进保险库的那份会话，直接问一次服务端「你认不认它」。
///
/// 执行器用一次性 `current-thread` tokio runtime + reqwest 异步 client：
/// 本仓 reqwest 没开 `blocking` 特性，而这里绝不能用 `tauri::async_runtime`
/// （模块头的 cargo-test 红线：被测试链接到的代码不能碰它）。
///
/// **结论只写日志，不否决登录** —— 见 [`SessionVerdict`]。
fn probe_session_live(session: &QuarkAuthSession) -> SessionVerdict {
    let header = quark_session_to_cookie_header(session);
    let url = quark_list_dir_url(
        "0",
        1,
        1,
        &quark_default_device_model(),
        &quark_default_ut(),
    );
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return SessionVerdict::Inconclusive("无法创建验活运行时"),
    };
    let outcome = runtime.block_on(async move {
        let client = reqwest::Client::builder()
            .timeout(VERIFY_BUDGET)
            // 不跟随重定向：重定向是风控或登录态不认的表现，跟过去就只剩 200 了。
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let response = client
            .get(&url)
            .header(reqwest::header::COOKIE, header)
            .send()
            .await?;
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        Ok::<_, reqwest::Error>((status, body))
    });
    match outcome {
        Ok((status, body)) => judge_session_probe(status, &body),
        Err(_) => SessionVerdict::Inconclusive("探针请求失败（网络不通或超时）"),
    }
}

/// 连续读不到 cookie ⇒ 收场。
///
/// 读超时与读失败对用户是同一件事（这一轮永远等不到 Ready），所以收场逻辑共用。
fn give_up_unreadable(app: &AppHandle) {
    log_line(app, "give up: cookies unreadable");
    settle();
    emit_event(app, "error", None, Some("读取登录状态失败，请重试。"));
    close_login_window(app);
}

fn poll_loop(app: AppHandle, window: tauri::WebviewWindow, generation: u64) {
    let started = Instant::now();

    // 保险一：清掉罐里可能残留的 `__pus`。
    // 用独立 cookie 罐时这一步天然是空操作（日志会报「jar 里没有 __pus 残留」），
    // 正好可以当作「独立罐确实生效」的信号；退回共享 jar 时它仍是有效兜底。
    purge_stale_pus(&app, &window);

    // 「为什么还没 Ready」按原因去重 —— 否则每 2 秒一条，日志很快就没法看了。
    let mut not_ready_logged: Option<&'static str> = None;
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
            log_line(
                &app,
                &format!(
                    "poller exit: 登录窗已不在（{}s）",
                    started.elapsed().as_secs()
                ),
            );
            return;
        }

        // 超时判定必须排在读 cookie **之前**：读一旦挂住，
        // 排在后面的超时分支就永远走不到。
        if started.elapsed() >= GIVE_UP {
            log_line(&app, "give up: timeout");
            settle();
            emit_event(&app, "timeout", None, Some("登录等待超时，请重试。"));
            close_login_window(&app);
            return;
        }

        match read_cookies(&window) {
            CookieRead::Timeout => {
                read_failures += 1;
                log_line(&app, &format!("cookies read timed out ({read_failures})"));
                if read_failures >= MAX_READ_FAILURES {
                    give_up_unreadable(&app);
                    return;
                }
            }
            CookieRead::Failed(err) => {
                read_failures += 1;
                log_line(&app, &format!("cookies read failed ({read_failures}): {err}"));
                if read_failures >= MAX_READ_FAILURES {
                    give_up_unreadable(&app);
                    return;
                }
            }
            CookieRead::Ok(cookies) => {
                read_failures = 0;

                let records: Vec<QuarkCookieRecord> = cookies
                    .iter()
                    .map(|cookie| QuarkCookieRecord::new(cookie.name(), cookie.value()))
                    .collect();
                let records = dedupe_cookie_records(records);
                let scan = quark_scan_cookies(&records);

                if let Some(reason) = not_ready_reason(&scan.outcome) {
                    if not_ready_logged != Some(reason) {
                        not_ready_logged = Some(reason);
                        log_line(&app, &format!("login 尚未 Ready：{reason}，继续等"));
                    }
                    continue;
                }

                let Some(session) = scan.session else {
                    continue;
                };

                // 验活探针：**只记日志，不否决**。
                // 它真正的用途是给 T0 的 U4 一个真实答案 ——
                // 拿到 200 + code=0 ⇒ 主路线（Rust 直连）成立。
                let last_alive_unix = match probe_session_live(&session) {
                    SessionVerdict::Live => {
                        log_line(&app, "login 验活通过：服务端认这份会话（主路线可用）");
                        Some(unix_now())
                    }
                    SessionVerdict::Inconclusive(reason) => {
                        log_line(&app, &format!("login 验活判不出来（不否决）: {reason}"));
                        None
                    }
                };

                let state = QuarkSessionState::new(session, last_alive_unix);
                let summary = quark_describe_state(Some(&state));
                log_line(&app, &format!("login state ready: {summary}"));
                store_pending(PendingSession { state });
                settle();
                emit_event(&app, "ready", Some(summary), None);
                close_login_window(&app);
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- extract_code ----------

    #[test]
    fn extract_code_reads_numeric_and_string_forms() {
        assert_eq!(extract_code(r#"{"code":0,"data":{}}"#), Some("0".to_string()));
        assert_eq!(extract_code(r#"{"code":"OK","data":{}}"#), Some("OK".to_string()));
        assert_eq!(extract_code(r#"{ "code" : 0 }"#), Some("0".to_string()));
    }

    #[test]
    fn extract_code_is_none_when_the_field_is_absent() {
        assert_eq!(extract_code(r#"{"status":"ok"}"#), None);
        assert_eq!(extract_code(r#"{"xcode":0}"#), None);
        assert_eq!(extract_code(""), None);
    }

    #[test]
    fn extract_code_tolerates_a_non_numeric_body() {
        // 抠不到数字就返回 None，不 panic。
        assert_eq!(extract_code(r#"{"code":null}"#), None);
    }

    // ---------- judge_session_probe ----------

    #[test]
    fn judge_accepts_only_conclusive_success_as_live() {
        assert_eq!(
            judge_session_probe(200, r#"{"code":0,"data":{"list":[]}}"#),
            SessionVerdict::Live
        );
        assert_eq!(
            judge_session_probe(200, r#"{"code":"OK","data":{}}"#),
            SessionVerdict::Live
        );
    }

    #[test]
    fn judge_never_rejects_on_401() {
        // T0 实测：游客态也是 401 + code=31001 ⇒ 401 **不能**证明 cookie 死了。
        assert_eq!(
            judge_session_probe(401, r#"{"code":31001}"#),
            SessionVerdict::Inconclusive("探针返回非 200，判不出会话状态")
        );
    }

    #[test]
    fn judge_treats_ambiguous_evidence_as_inconclusive() {
        assert_eq!(
            judge_session_probe(200, r#"{"code":31001}"#),
            SessionVerdict::Inconclusive("服务端返回了非成功的 code")
        );
        assert_eq!(
            judge_session_probe(200, r#"{"status":"ok"}"#),
            SessionVerdict::Inconclusive("读不出返回体里的 code")
        );
        assert_eq!(
            judge_session_probe(500, ""),
            SessionVerdict::Inconclusive("探针返回非 200，判不出会话状态")
        );
    }

    // ---------- dedupe_cookie_records ----------

    #[test]
    fn dedupe_keeps_the_first_occurrence_of_a_name() {
        let merged = dedupe_cookie_records(vec![
            QuarkCookieRecord::new("__pus", "first"),
            QuarkCookieRecord::new("ctoken", "x"),
            QuarkCookieRecord::new("__pus", "second"),
        ]);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].value, "first");
        assert_eq!(merged[1].name, "ctoken");
    }

    // ---------- not_ready_reason ----------

    #[test]
    fn not_ready_reason_distinguishes_missing_from_invalid() {
        assert_eq!(not_ready_reason(&QuarkCookieOutcome::Ready), None);
        assert!(not_ready_reason(&QuarkCookieOutcome::Missing).is_some());
        assert!(not_ready_reason(&QuarkCookieOutcome::Invalid).is_some());
        assert_ne!(
            not_ready_reason(&QuarkCookieOutcome::Missing),
            not_ready_reason(&QuarkCookieOutcome::Invalid)
        );
    }

    // ---------- 常量 ----------

    #[test]
    fn the_login_window_constants_are_stable() {
        assert_eq!(LOGIN_WINDOW_LABEL, "afm-quark-login");
        assert_eq!(LOGIN_EVENT, "afm://quark-login");
        assert_eq!(LOGIN_START_URL, "https://pan.quark.cn/");
        assert_eq!(LOGIN_PROFILE_ROOT, "quark-login-profile");
        assert_eq!(QUARK_COOKIE_PUS, "__pus");
    }

    #[test]
    fn the_cookie_urls_start_with_the_login_page_origin() {
        // 同名取第一条 ⇒ 登录页所在的域必须排在最前面。
        assert_eq!(COOKIE_URLS[0], "https://pan.quark.cn/");
        assert!(COOKIE_URLS.len() >= 2);
    }
}
