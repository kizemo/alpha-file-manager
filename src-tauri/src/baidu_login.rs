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
use std::path::{Path, PathBuf};
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
/// `BDUSS` 的 cookie 名。会话身份全靠它，基线比对只认这一个。
const BDUSS_NAME: &str = "BDUSS";

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

/// 服务端验活的时间上限。探测接口是轻量 list，8 秒足够；
/// 超时就按「证据不足」放行，绝不把窗口无限期挂着。
const VERIFY_BUDGET: Duration = Duration::from_secs(8);

/// 登录 harvest 短路缺陷的三道保险（2026-11-09 实测坐实）：
///
/// 旧 jar 里遗留的死 BDUSS/STOKEN 会被当成「刚登录成功」，建窗后 2.005 秒
/// （第一个轮询 tick）就判 Ready 并自动关窗，用户根本没机会扫码；这对
/// 死凭据随后被存进保险库，服务端连读接口都回 `errno=-6`。
///
/// 保险一：轮询开始前把 jar 里的 `BDUSS`/`STOKEN` 删掉（清不掉也有保险二）。
/// 保险二：Ready 必须「当前 BDUSS ≠ 基线 BDUSS」，同值一票否决。
/// 保险三：关窗之前对着服务端验活，只在拿到**确凿死亡证据**时才拒绝。
///
/// 代价说明：若清 jar 失败且百度把**同一个** BDUSS 原样续回（真「重新登录」
/// 罕见），窗口会留到用户退出再登录或 900 秒超时 —— 宁可多等，不可误判 Ready。

/// 诊断日志的环境变量；不给就用应用数据目录下的默认文件名。
const LOG_ENV: &str = "AFM_BAIDU_LOG";
const DEFAULT_LOG_NAME: &str = "baidu-login.log";

/// 已读到、但还没被任何连接认领的登录态。
///
/// `BaiduSession` 本身没有 `stoken()` 访问器，所以 `STOKEN` 是否存在
/// 要在这里显式记下来。
///
/// 注意：这里**不保存 BDUSS/STOKEN 明文之外的额外副本**——BDUSS 只在轮询线程的
/// 局部作用域里参与基线比对与验活，claim 之后交给 `save_baidu_session` 落盘。
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

/// 独立 cookie 罐的根目录（应用数据目录下的子目录）。
const LOGIN_PROFILE_ROOT: &str = "baidu-login-profile";

/// 本轮登录专用的 webview 数据目录。
///
/// 登录窗原先与主窗口共用 data-dir，打开时必然继承主窗口（以及上一轮登录）
/// 留在罐里的百度 cookie —— 用户看到的是「已登录的旧账号」，必须先手点退出
/// 才能换号。而「建窗后主动删 cookie」这条路实测无效：`delete_cookie` 返回 Ok，
/// 日志也如实报「尝试删掉 2 条、失败 0 条」，但 2 秒后轮询读到的 BDUSS 还是
/// 原值（2026-11-09 真机日志坐实）。既然清不掉，就换成每轮一支空罐：
/// 罐里本来没有 BDUSS，`purge` 只作兜底，基线否决自然失效。
fn login_profile_dir(base: &Path, stamp_ms: u64) -> PathBuf {
    base.join(LOGIN_PROFILE_ROOT)
        .join(format!("session-{stamp_ms}"))
}

/// 清扫上一轮遗留的 profile 子目录。
///
/// 只在「确定没有在用登录窗」时调用（`remote_baidu_login_start` 已保证此时
/// 窗口不存在），所以整棵 `session-*` 都是死数据。删除失败只记日志：
/// 真删不掉（被占用）也不该让这一轮登录开不起来。
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

        // 每轮登录换一支全新的 webview 数据目录：罐子天生是空的，
        // 一打开就是未登录态，不再需要用户先手点「退出登录」再扫码。
        // 建目录失败时退回共享 jar（`prepare_login_profile` 内已记日志）。
        let mut builder =
            WebviewWindowBuilder::new(&app, LOGIN_WINDOW_LABEL, WebviewUrl::External(url))
                .title("百度网盘登录")
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

/// `BAIDUID` 的 cookie 名。验活探针要把它一起带上（百度的风控会看这个）。
const BAIDUID_NAME: &str = "BAIDUID";

/// 验活探针端点：与读接口同一口径的目录列表，`limit=1` 把返回体压到最小。
///
/// 依据：`docs/2026-11-09-baidu-write-session-findings.md` §4-B 实测失效会话在这里
/// 回 `status=200 errno=-6`；`baidu_api.rs` 的 errno 表同样写着
/// 「`-6` 用户未登录/会话失效，实测 T1 探针」。两处口径一致。
const PROBE_LIVE_URL: &str = "https://pan.baidu.com/rest/2.0/xpan/file?method=list&limit=1&order=name&asc=0&web=1&app_id=250528&clienttype=0&_input_charset=utf-8";

/// 会话是否可用的判定结果。
#[derive(Debug, PartialEq, Eq)]
enum SessionVerdict {
    /// 服务端确认这个会话能用。
    Live,
    /// 服务端**确凿地**把请求当成未登录 —— 放行的话就会把死凭据存进保险库。
    Rejected(&'static str),
    /// 判不出来（网络不通、返回体读不懂、预期外的状态码）。
    /// 调用方按「放行」处理：探针自身失败不该打掉用户真的登录成功。
    Inconclusive(&'static str),
}

/// 打开登录窗口**之前** cookie 罐里已有的会话身份（保险二的比对基准）。
struct AuthBaseline {
    bduss: Option<String>,
}

/// 从返回体里抠出 `errno` 的整数值。
///
/// 手法与 `baidu_api::errno_code` 一致（字节级滑窗，不做 JSON 反序列化）：
/// 找到 `errno` → 跳引号/空白 → 必须紧跟 `:` → 再跳引号/空白 → 允许前导负号 → 取数字。
/// `None` = 返回体里读不出 errno。
fn probe_errno(body: &str) -> Option<i64> {
    const KEY: &[u8] = b"errno";
    let bytes = body.as_bytes();
    let skip_noise = |mut i: usize| -> usize {
        while i < bytes.len()
            && (bytes[i] == b'"' || bytes[i] == b'\'' || bytes[i].is_ascii_whitespace())
        {
            i += 1;
        }
        i
    };
    let mut i = 0usize;
    while i + KEY.len() <= bytes.len() {
        if &bytes[i..i + KEY.len()] == KEY {
            let colon = skip_noise(i + KEY.len());
            if colon < bytes.len() && bytes[colon] == b':' {
                let start = skip_noise(colon + 1);
                let mut end = start;
                if end < bytes.len() && bytes[end] == b'-' {
                    end += 1;
                }
                let digits = end;
                while end < bytes.len() && bytes[end].is_ascii_digit() {
                    end += 1;
                }
                if end > digits {
                    return std::str::from_utf8(&bytes[start..end]).ok()?.parse().ok();
                }
            }
            i += KEY.len();
            continue;
        }
        i += 1;
    }
    None
}

/// 纯函数：按探针的 HTTP 状态码 + 返回体判定会话。
///
/// 判据口径与 `baidu_api::classify_write_response` 一致（`-6` = 未登录/会话失效、
/// 重定向 = 未登录），差别只在探针是 GET、不需要 bdstoken。
/// **只有确凿证据才判 Rejected**：`403`/`429`/`5xx` 这类更像风控或自身故障，
/// 一律 Inconclusive，不要把有效会话误杀。
fn judge_session_probe(status: u16, body: &str) -> SessionVerdict {
    if matches!(status, 301 | 302 | 303 | 307 | 308) {
        return SessionVerdict::Rejected("被当成未登录重定向（3xx）");
    }
    if status == 401 {
        return SessionVerdict::Rejected("服务端返回 401 未授权");
    }
    if status != 200 {
        return SessionVerdict::Inconclusive("探针返回非 200，判不出会话状态");
    }
    match probe_errno(body) {
        Some(0) => SessionVerdict::Live,
        Some(-6) => SessionVerdict::Rejected("服务端判定未登录（errno=-6）"),
        Some(_) => SessionVerdict::Inconclusive("探针返回预期外的 errno"),
        None => SessionVerdict::Inconclusive("读不出探针返回体里的 errno"),
    }
}

/// 取 cookie 罐里的 BDUSS 值。同名多条时取第一条 —— 与
/// `baidu_auth::scan_cookies`「同名取第一个」的口径保持一致。
fn current_bduss<'a>(
    mut cookies: impl Iterator<Item = &'a tauri::webview::Cookie<'static>>,
) -> Option<&'a str> {
    cookies
        .find(|cookie| cookie.name() == BDUSS_NAME)
        .map(|cookie| cookie.value())
}

/// 纯函数：当前 BDUSS 是不是「与基线不同的新值」。
///
/// - 有值且 ≠ 基线 ⇒ 这次登录真的换发了新会话；
/// - 有值且 = 基线 ⇒ 就是旧 jar 里那一条，一票否决；
/// - 无值 ⇒ 也否决（调用点在 `Ready` 之后，属兜底）。
fn has_new_bduss(baseline: &AuthBaseline, current: Option<&str>) -> bool {
    match current {
        Some(current) => baseline.bduss.as_deref() != Some(current),
        None => false,
    }
}

/// 保险三：拿**即将**存进保险库的那份会话，直接问一次百度「你认不认它」。
///
/// 执行器用一次性 `current-thread` tokio runtime + reqwest 异步 client：
/// 本仓 reqwest 没开 `blocking` 特性，而这里绝不能用 `tauri::async_runtime`
/// （模块头的 cargo-test 红线：被测试链接到的代码不能碰它）。
fn probe_session_live(cookies: &[tauri::webview::Cookie<'static>]) -> SessionVerdict {
    let mut jar = String::new();
    for cookie in cookies {
        if matches!(
            cookie.name(),
            BDUSS_NAME | STOKEN_NAME | BAIDUID_NAME
        ) {
            if !jar.is_empty() {
                jar.push_str("; ");
            }
            jar.push_str(cookie.name());
            jar.push('=');
            jar.push_str(cookie.value());
        }
    }
    if !jar.contains(BDUSS_NAME) {
        return SessionVerdict::Inconclusive("cookie 串里没有 BDUSS");
    }
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
            // 不跟随重定向：302 是「未登录」的确凿证据，跟过去就只剩 200 了。
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let response = client
            .get(PROBE_LIVE_URL)
            .header(reqwest::header::COOKIE, jar)
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

/// 保险一：建窗后第一件事 —— 读一次 cookie 罐，把旧的 `BDUSS`/`STOKEN` 删掉，
/// 并把**删除之前**看到的 BDUSS 记成基线（清不掉也还有保险二兜着）。
fn purge_stale_auth_cookies(
    app: &AppHandle,
    window: &tauri::WebviewWindow,
    url: &tauri::Url,
) -> AuthBaseline {
    let mut baseline = AuthBaseline { bduss: None };
    let cookies = match read_cookies(window, url.clone(), COOKIE_READ_BUDGET) {
        CookieRead::Ok(cookies) => cookies,
        CookieRead::Timeout => {
            log_line(app, "purge: 读 cookie 罐超时，跳过清 jar（保险二/三仍在）");
            return baseline;
        }
        CookieRead::Failed(err) => {
            log_line(
                app,
                &format!("purge: 读 cookie 罐失败，跳过清 jar（保险二/三仍在）: {err}"),
            );
            return baseline;
        }
    };
    baseline.bduss = current_bduss(cookies.iter()).map(str::to_owned);
    let victims: Vec<_> = cookies
        .into_iter()
        .filter(|cookie| cookie.name() == BDUSS_NAME || cookie.name() == STOKEN_NAME)
        .collect();
    let total = victims.len();
    if total == 0 {
        log_line(app, "purge: jar 里没有 BDUSS/STOKEN 残留");
        return baseline;
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
        Ok(failed) => {
            log_line(
                app,
                &format!("purge: 尝试删掉 {total} 条旧 BDUSS/STOKEN，失败 {failed} 条"),
            );
        }
        Err(_) => log_line(app, "purge: 删除线程超时（保险二/三仍在）"),
    }
    baseline
}

/// 连续读不到 cookie ⇒ 收场。
///
/// 读超时与读失败对用户是同一件事（这一轮永远等不到 Ready），所以收场逻辑共用。
fn give_up_unreadable(app: &AppHandle) {
    log_line(app, "give up: cookies unreadable");
    settle();
    emit_event(app, "error", false, None, Some("读取登录状态失败，请重试。"));
    close_login_window(app);
}

/// `BaiduCookieOutcome` 里「为什么还没 Ready」的可读原因。
///
/// 只用于诊断日志：轮询每 2 秒跑一次，把原因写清楚才能事后区分
/// 「用户还没扫码」（Missing）与「扫到了但值不合法」（Invalid）。
fn not_ready_reason(outcome: &BaiduCookieOutcome) -> Option<&'static str> {
    match outcome {
        BaiduCookieOutcome::Ready => None,
        BaiduCookieOutcome::Missing => Some("BDUSS 还没出现在 cookie 罐里"),
        BaiduCookieOutcome::Invalid => Some("BDUSS 值不合法"),
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

    // 保险一：清掉 jar 里遗留的 BDUSS/STOKEN，并记下「建窗时还在的 BDUSS」作为基线。
    // 用独立 cookie 罐时这一步天然是空操作（日志会报「jar 里没有 BDUSS/STOKEN 残留」），
    // 正好可以当作「独立罐确实生效」的信号；退回共享 jar 时它仍是有效兜底。
    let baseline = purge_stale_auth_cookies(&app, &window, &url);
    // 已被服务端确证「死」的 BDUSS：同一个值不再重复探针，免得每 2 秒轰炸服务端。
    let mut rejected_bduss: Option<String> = None;
    // 「同值旧凭据」的提示日志按值去重，避免轮询把日志刷爆。
    let mut stale_logged: Option<String> = None;
    // 「为什么还没 Ready」按原因去重 —— 否则每 2 秒一条，日志很快就没法看了。
    let mut not_ready_logged: Option<&'static str> = None;
    // 「BDUSS 合法但会话字段缺失」只记一次。
    let mut session_missing_logged = false;

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
            // 补一条诊断：此前这里是静默 return，事后完全分不清
            // 「消息还没到点就被关窗」与「轮询自己崩了」。
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
            emit_event(&app, "timeout", false, None, Some("登录等待超时，请重试。"));
            close_login_window(&app);
            return;
        }

        match read_cookies(&window, url.clone(), COOKIE_READ_BUDGET) {
            CookieRead::Timeout => {
                read_failures += 1;
                log_line(&app, &format!("cookies read timed out ({read_failures})"));
                if read_failures >= MAX_READ_FAILURES {
                    give_up_unreadable(&app);
                    return;
                }
            }
            CookieRead::Failed(err) => {
                // 读失败和读超时一样会让这一轮永远等不到 Ready。
                // 旧行为只记日志不计数 ⇒ 会一路空转到 900 秒才收场。
                read_failures += 1;
                log_line(&app, &format!("cookies read failed ({read_failures}): {err}"));
                if read_failures >= MAX_READ_FAILURES {
                    give_up_unreadable(&app);
                    return;
                }
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
                if let Some(reason) = not_ready_reason(&scan.outcome) {
                    // 每 2 秒一条会把日志刷爆，所以只在原因变化时记一条。
                    if not_ready_logged != Some(reason) {
                        not_ready_logged = Some(reason);
                        log_line(&app, &format!("login 尚未 Ready：{reason}，继续等"));
                    }
                    continue;
                }

                // 保险二：BDUSS 与「建窗时的基线」同值 ⇒ 这是旧 jar 里的遗留凭据，
                // 不是用户这次新扫出来的。万一清 jar 没清掉，这里照样一票否决。
                let Some(current_bduss) = current_bduss(cookies.iter()) else {
                    continue;
                };
                if !has_new_bduss(&baseline, Some(current_bduss)) {
                    if stale_logged.as_deref() != Some(current_bduss) {
                        stale_logged = Some(current_bduss.to_owned());
                        log_line(
                            &app,
                            "login 短路拦截：BDUSS 与基线同值（旧 jar 遗留），继续等真正的新登录",
                        );
                    }
                    continue;
                }
                // 已经探出「死」的同一个 BDUSS 不再重复探针，等它的值发生变化。
                if rejected_bduss.as_deref() == Some(current_bduss) {
                    continue;
                }

                let has_stoken = !matches!(scan.stoken, BaiduStokenState::Absent);
                let Some(session) = scan.session else {
                    if !session_missing_logged {
                        session_missing_logged = true;
                        log_line(&app, "login 尚未 Ready：BDUSS 合法但会话字段缺失，继续等");
                    }
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

                // 保险三：交给服务端判决。只有确凿证据（3xx / 401 / errno=-6）才拒绝，
                // 判不出来一律放行 —— 宁可多等一轮，也不误杀一个有效会话。
                match probe_session_live(&cookies) {
                    SessionVerdict::Live => {}
                    SessionVerdict::Rejected(reason) => {
                        rejected_bduss = Some(current_bduss.to_owned());
                        log_line(
                            &app,
                            &format!("login 验活被服务端拒绝（{reason}），不当作 Ready，继续等")
                        );
                        continue;
                    }
                    SessionVerdict::Inconclusive(reason) => {
                        log_line(&app, &format!("login 验活判不出来，按放行处理: {reason}"));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cookie(name: &'static str, value: &'static str) -> tauri::webview::Cookie<'static> {
        tauri::webview::Cookie::new(name, value)
    }

    // ---------- probe_errno ----------

    #[test]
    fn probe_errno_reads_positive_and_negative() {
        assert_eq!(probe_errno(r#"{"errno":0,"list":[]}"#), Some(0));
        assert_eq!(probe_errno(r#"{"request_id":"1","errno":-6}"#), Some(-6));
        assert_eq!(probe_errno(r#"{ "errno" : 2 , "x": 3 }"#), Some(2));
    }

    #[test]
    fn probe_errno_is_a_byte_scan_not_a_json_parse() {
        // 容忍字符串形态的 errno（服务端两种都出现过）
        assert_eq!(probe_errno(r#"{"errno":"-6"}"#), Some(-6));
        // 数字后面还有字符时取前面那段 —— 这是字节滑窗的取舍，不追求 JSON 严格性
        assert_eq!(probe_errno(r#"{"errno":12ab}"#), Some(12));
        // 缺冒号
        assert_eq!(probe_errno(r#"{"errno"0}"#), None);
        // 压根没有这个键
        assert_eq!(probe_errno(r#"{"list":[1,2,3]}"#), None);
        // 只有键名的一部分
        assert_eq!(probe_errno(r#"{"errnox":0}"#), None);
    }

    // ---------- judge_session_probe ----------

    #[test]
    fn judge_accepts_only_errno_zero_as_live() {
        assert_eq!(
            judge_session_probe(200, r#"{"errno":0,"list":[]}"#),
            SessionVerdict::Live
        );
    }

    #[test]
    fn judge_treats_ambiguous_evidence_as_inconclusive() {
        // 没有 STOKEN 时服务端可能回别的 errno（findings §7-②），判不出来就放行
        assert_eq!(
            judge_session_probe(200, r#"{"errno":2,"list":[]}"#),
            SessionVerdict::Inconclusive("探针返回预期外的 errno")
        );
        assert_eq!(
            judge_session_probe(200, r#"{"no_errno":true}"#),
            SessionVerdict::Inconclusive("读不出探针返回体里的 errno")
        );
    }

    #[test]
    fn judge_rejects_on_conclusive_evidence() {
        assert_eq!(
            judge_session_probe(200, r#"{"errno":-6,"list":[]}"#),
            SessionVerdict::Rejected("服务端判定未登录（errno=-6）")
        );
        assert_eq!(
            judge_session_probe(302, ""),
            SessionVerdict::Rejected("被当成未登录重定向（3xx）")
        );
        assert_eq!(
            judge_session_probe(401, ""),
            SessionVerdict::Rejected("服务端返回 401 未授权")
        );
    }

    #[test]
    fn judge_keeps_throttling_and_server_errors_inconclusive() {
        // 这四条是「有效会话不被误杀」的关键：风控与自身故障都不是拒绝证据
        for status in [403u16, 429, 500, 503] {
            assert_eq!(
                judge_session_probe(status, r#"{"errno":-6}"#),
                SessionVerdict::Inconclusive("探针返回非 200，判不出会话状态"),
                "status={status} 不该被判成拒绝"
            );
        }
    }

    // ---------- current_bduss / has_new_bduss ----------

    #[test]
    fn current_bduss_takes_first_name_match() {
        let cookies = vec![cookie("BAIDUID", "b"), cookie("BDUSS", "first"), cookie("BDUSS", "second")];
        assert_eq!(current_bduss(cookies.iter()), Some("first"));

        let without = vec![cookie("BAIDUID", "b")];
        assert_eq!(current_bduss(without.iter()), None);
    }

    #[test]
    fn has_new_bduss_vetoes_same_value_only() {
        let baseline = AuthBaseline { bduss: Some("stale".to_owned()) };
        // 同值 = 旧 jar 遗留 ⇒ 一票否决
        assert!(!has_new_bduss(&baseline, Some("stale")));
        // 换了值 = 用户真的重新授权了 ⇒ 放行
        assert!(has_new_bduss(&baseline, Some("fresh")));
        // jar 里本来没有 BDUSS ⇒ 不构成否决
        assert!(!has_new_bduss(&baseline, None));
        assert!(has_new_bduss(&AuthBaseline { bduss: None }, Some("any")));
    }

    // ---------- login_profile_dir ----------

    #[test]
    fn login_profile_dir_gives_each_round_its_own_jar() {
        let base = Path::new("C:\\data");
        let a = login_profile_dir(base, 1_794_202_786_497);
        let b = login_profile_dir(base, 1_794_202_786_498);
        // 相差 1 ms 也必须是两个目录 —— 每轮登录都要拿到一支空罐，
        // 否则上一轮的 BDUSS 会让保险二（基线否决）把整轮登录卡死。
        assert_ne!(a, b, "两轮登录不能共用同一个 cookie 罐");
        assert!(a.starts_with(base.join(LOGIN_PROFILE_ROOT)));
        assert_eq!(a.file_name().and_then(|n| n.to_str()), Some("session-1794202786497"));
    }

    // ---------- not_ready_reason ----------

    #[test]
    fn not_ready_reason_is_none_only_when_ready() {
        assert_eq!(not_ready_reason(&BaiduCookieOutcome::Ready), None);
    }

    #[test]
    fn not_ready_reason_distinguishes_missing_from_invalid() {
        let missing = not_ready_reason(&BaiduCookieOutcome::Missing).expect("Missing 必须给出原因");
        let invalid = not_ready_reason(&BaiduCookieOutcome::Invalid).expect("Invalid 必须给出原因");
        // 日志按原因字符串去重：两条原因若相同，「BDUSS 没出现」和
        // 「BDUSS 非法」就会在日志里合并成一条，排查时分不清是哪一种。
        assert_ne!(missing, invalid);
    }
}
