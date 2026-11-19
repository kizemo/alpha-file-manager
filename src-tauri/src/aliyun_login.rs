// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! 内嵌 WebView2 走阿里云盘开放平台的 OAuth 授权（只读）。
//!
//! 与百度/夸克**本质不同**：那两家拿的是 cookie（轮询 `cookies_for_url`），
//! 阿里云盘这里拿的是**一次性授权码**（`code`），cookie 那套完全用不上。
//!
//! ## 流程
//!
//! ```text
//! remote_aliyun_login_start(client_id, client_secret?, redirect_uri?)
//!   → 生成 state / code_verifier → 建授权窗（独立线程）
//!   → 捕获回调里的 code（双保险：on_navigation + eval 轮询）
//!   → 用 code 换 token → 存进「待认领槽位」
//!   → emit("afm://aliyun-login", {status:"ready"}) → 关窗
//!
//! remote_aliyun_login_pending()   → 前端打开表单时兜底查询
//! remote_aliyun_login_claim(id)   → 把待认领会话写进 DPAPI 密文（键 = connection_id）
//! remote_aliyun_login_cancel()    → 清槽位 + 关窗
//! remote_aliyun_auth_status(id)    → 授权状态卡（missing / ready / stale / stale_soon）
//! remote_aliyun_delete_session(id) → 删会话（键不存在是成功的空操作）
//! ```
//!
//! ## 权限边界
//!
//! `code` / `access_token` / `refresh_token` **明文只在本模块与
//! `remote::aliyun_auth` 之间流转**，不出现在事件 payload、命令返回值与日志里。
//! 落盘由 `aliyun_auth::save_aliyun_session` 委托 `credentials.rs`（DPAPI）完成。
//!
//! ⚠️ **不得引用 `tauri::async_runtime::*`**。`cargo test --lib` 会链接整个
//! crate，`GlobalRuntime<Wry>` 一旦被单态化就会把 tao 拖进测试二进制，
//! 进程在**加载阶段**以 `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉。
//! 因此这里的命令全部是同步实现，需要「另起线程」时用 `std::thread`，
//! 需要跑 HTTP 时自己建一个 current-thread tokio runtime。
//!
//! ## 窗口必须在独立线程里建
//!
//! `tauri-runtime-wry` 的 `create_webview` 上有一行原文注释：
//! *this must be called from a separate thread, otherwise the channel will
//! introduce a deadlock*。命令是从 WebView2 的 IPC 回调里进来的 —— 那是
//! **事件循环线程** —— 在那里直接 `WebviewWindowBuilder::build()` 会死锁。
//! 详见 `quark_login.rs` 的模块顶部注释（同一份背景）。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::remote::{
    aliyun_authorize_url, aliyun_code_challenge, aliyun_describe_session, aliyun_extract_code,
    aliyun_generate_code_verifier, aliyun_generate_state, aliyun_now_unix, aliyun_state_matches,
    aliyun_status_name, delete_aliyun_session, load_aliyun_session, save_aliyun_session,
    AliyunAuthSession, RemoteError, ALIYUN_DEFAULT_REDIRECT_URI, ALIYUN_DEFAULT_SCOPE,
};

/// 登录子窗口的固定 label。`start` 靠它判重，轮询线程靠它判存活。
const LOGIN_WINDOW_LABEL: &str = "afm-aliyun-login";

/// 前端订阅的事件名。`tauri://` 前缀由 Tauri 保留，故用自定义协议名。
const LOGIN_EVENT: &str = "afm://aliyun-login";

/// 轮询间隔。
const POLL: Duration = Duration::from_millis(800);

/// 等待授权的总时限。
const GIVE_UP: Duration = Duration::from_secs(600);

/// 回调落地用的本地监听端口（`redirect_uri` 默认指向它）。
const CALLBACK_PORT: u16 = 47832;

/// 本地回调监听的路径前缀。
const CALLBACK_PATH: &str = "/__afm_aliyun_callback";

/// 本轮登录是否已经「程序化收场」。
///
/// 成功 / 超时 / 出错都会先把它置 true，随后关闭窗口；这样
/// `WindowEvent::Destroyed` 就不会再补一条「用户关掉了窗口」。
static SETTLED: AtomicBool = AtomicBool::new(false);

/// 当前轮次代号。每开一轮登录就 +1，轮询线程发现自己不是最新那轮就收工。
static POLLER_GENERATION: AtomicU64 = AtomicU64::new(0);

/// 已捕获、但还没换 token 的授权码。
struct CapturedCode {
    code: String,
    state: Option<String>,
}

/// 待认领的会话（已换好 token，还没写进某条连接）。
struct PendingSession {
    session: AliyunAuthSession,
}

/// 待认领槽位。
type PendingSlot = Mutex<Option<PendingSession>>;

/// 授权码槽位。
type CodeSlot = Mutex<Option<CapturedCode>>;

fn pending_slot() -> &'static PendingSlot {
    static PENDING: OnceLock<PendingSlot> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(None))
}

fn pending_lock() -> std::sync::MutexGuard<'static, Option<PendingSession>> {
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

fn code_slot() -> &'static CodeSlot {
    static CODE: OnceLock<CodeSlot> = OnceLock::new();
    CODE.get_or_init(|| Mutex::new(None))
}

fn code_lock() -> std::sync::MutexGuard<'static, Option<CapturedCode>> {
    code_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 记下一个捕获到的授权码。**只留第一个** —— 后来的重复回调不该覆盖先到的。
fn store_code(code: String, state: Option<String>) {
    let mut guard = code_lock();
    if guard.is_none() {
        *guard = Some(CapturedCode { code, state });
    }
}

/// 登录进度事件。**不携带任何 code / token 明文**。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliyunLoginEvent {
    /// `ready` | `closed` | `timeout` | `error`
    pub status: &'static str,
    /// 脱敏描述。
    pub summary: Option<String>,
    /// 面向用户的短说明。
    pub message: Option<String>,
}

/// 登录窗是否已打开。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliyunLoginStartPayload {
    pub opened: bool,
    /// 实际使用的授权地址（**不含 client_secret**）。
    pub authorize_url: String,
}

/// 有没有待认领的会话。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliyunLoginPendingPayload {
    pub pending: bool,
    /// 脱敏描述（没有内容）。
    pub summary: String,
}

/// 认领结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliyunLoginClaimPayload {
    pub claimed: bool,
    /// 脱敏描述。
    pub summary: String,
}

/// 授权状态卡。
///
/// `status` 只有 4 种：`missing` / `ready` / `stale` / `stale_soon`。
/// **不返回 `loading`** —— 后端没有「正在加载」这种状态可言，
/// 前端要表达加载中得自己管。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliyunAuthStatusPayload {
    /// `missing` | `ready` | `stale` | `stale_soon`
    pub status: String,
    /// 距过期还剩多少秒。`None` = 不知道过期时间。
    pub remaining_seconds: Option<i64>,
    /// 脱敏描述。
    pub summary: String,
    /// 应用 id（**不是密钥**）。
    pub client_id: Option<String>,
    /// 是否配了 `client_secret`。
    pub has_client_secret: bool,
    /// 申请的 scope。
    pub scope: Option<String>,
    /// 过期时刻（Unix 秒）。
    pub expires_at: Option<i64>,
    /// 空间 id。
    pub drive_id: Option<String>,
    /// 账号昵称（还没取到时为空）。
    pub nick_name: Option<String>,
}

/// 应用数据目录。
fn app_data_dir(app: &AppHandle) -> Result<PathBuf, RemoteError> {
    crate::app_dirs::app_data_dir(app)
        .map_err(|err| RemoteError::unknown(format!("无法定位应用数据目录：{err}")))
}

/// 把「空串」与 `None` 一视同仁 —— 前端表单里没填的输入框传过来就是空串。
fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

/// 打开阿里云盘授权窗口。
///
/// `client_secret` 为 `None`/空串 ⇒ PKCE 公开客户端模式；
/// `redirect_uri` 为 `None`/空串 ⇒ 用契约里的默认值。
///
/// 密钥模式与 PKCE 模式**共用同一条 code→token 路径**，唯一差异是有没有
/// `client_secret`（见 `remote::aliyun_api::aliyun_authorization_code_form`）。
#[tauri::command]
pub fn remote_aliyun_login_start(
    app: AppHandle,
    client_id: String,
    client_secret: Option<String>,
    redirect_uri: Option<String>,
) -> Result<AliyunLoginStartPayload, RemoteError> {
    let client_id = client_id.trim().to_string();
    if client_id.is_empty() {
        return Err(RemoteError::protocol(
            "阿里云盘的应用 id 不能为空，请先在表单里填写 client_id。",
        ));
    }

    let client_secret = normalize_optional(client_secret);
    let redirect_uri =
        normalize_optional(redirect_uri).unwrap_or_else(|| ALIYUN_DEFAULT_REDIRECT_URI.to_string());

    // 新一轮登录：上一轮的待认领会话与捕获到的码都作废。
    SETTLED.store(false, Ordering::SeqCst);
    clear_pending();
    *code_lock() = None;

    let state = aliyun_generate_state();
    let code_verifier = aliyun_generate_code_verifier();
    let code_challenge = aliyun_code_challenge(&code_verifier);

    let authorize_url = aliyun_authorize_url(
        &client_id,
        &redirect_uri,
        ALIYUN_DEFAULT_SCOPE,
        &state,
        &code_challenge,
        "S256",
        None,
    );

    // 本轮登录的上下文。轮询线程要用它换 token，所以必须跨线程共享。
    let context = LoginContext {
        client_id: client_id.clone(),
        client_secret: client_secret.clone(),
        redirect_uri: redirect_uri.clone(),
        state: state.clone(),
        code_verifier: code_verifier.clone(),
    };

    if let Some(existing) = app.get_webview_window(LOGIN_WINDOW_LABEL) {
        let _ = existing.set_always_on_top(true);
        let _ = existing.set_focus();
        start_poller(app.clone(), existing, context);
        return Ok(AliyunLoginStartPayload {
            opened: true,
            authorize_url,
        });
    }

    spawn_window_builder(app.clone(), authorize_url.clone(), context);
    Ok(AliyunLoginStartPayload {
        opened: true,
        authorize_url,
    })
}

/// 查询有没有待认领的会话（前端打开表单时用来兜底）。
#[tauri::command]
pub fn remote_aliyun_login_pending() -> AliyunLoginPendingPayload {
    let guard = pending_lock();
    match guard.as_ref() {
        None => AliyunLoginPendingPayload {
            pending: false,
            summary: String::new(),
        },
        Some(pending) => AliyunLoginPendingPayload {
            pending: true,
            summary: pending.session.describe(),
        },
    }
}

/// 把待认领的会话加密落盘到指定连接。
#[tauri::command]
pub fn remote_aliyun_login_claim(
    app: AppHandle,
    connection_id: String,
) -> Result<AliyunLoginClaimPayload, RemoteError> {
    let id = connection_id.trim().to_string();
    let Some(pending) = take_pending() else {
        return Ok(AliyunLoginClaimPayload {
            claimed: false,
            summary: String::new(),
        });
    };

    if id.is_empty() {
        store_pending(pending);
        return Err(RemoteError::unknown(
            "连接尚未建立，无法保存阿里云盘登录态。",
        ));
    }

    let summary = pending.session.describe();
    let dir = app_data_dir(&app)?;
    if let Err(err) = save_aliyun_session(&dir, &id, &pending.session) {
        // 落盘失败就把会话放回去，用户不用重新登录一次。
        store_pending(pending);
        return Err(err);
    }

    Ok(AliyunLoginClaimPayload {
        claimed: true,
        summary,
    })
}

/// 用户主动取消登录。
#[tauri::command]
pub fn remote_aliyun_login_cancel(app: AppHandle) -> Result<(), RemoteError> {
    settle();
    clear_pending();
    *code_lock() = None;
    close_login_window(&app);
    Ok(())
}

/// 授权状态卡。
#[tauri::command]
pub fn remote_aliyun_auth_status(
    app: AppHandle,
    connection_id: String,
) -> Result<AliyunAuthStatusPayload, RemoteError> {
    let id = connection_id.trim().to_string();
    let dir = app_data_dir(&app)?;
    let session = load_aliyun_session(&dir, &id)?;

    let now = aliyun_now_unix();
    let status = crate::remote::aliyun_auth_status(session.as_ref(), now);
    let remaining_seconds = session
        .as_ref()
        .and_then(|session| crate::remote::aliyun_remaining_seconds(session, now));

    Ok(AliyunAuthStatusPayload {
        status: aliyun_status_name(&status).to_string(),
        remaining_seconds,
        summary: aliyun_describe_session(session.as_ref()),
        client_id: session
            .as_ref()
            .map(|session| session.client_id.clone())
            .filter(|value| !value.is_empty()),
        has_client_secret: session
            .as_ref()
            .map(|session| session.client_secret.is_some())
            .unwrap_or(false),
        scope: session.as_ref().and_then(|session| session.scope.clone()),
        expires_at: session.as_ref().and_then(|session| session.expires_at_unix),
        drive_id: session
            .as_ref()
            .and_then(|session| session.drive_id.clone()),
        nick_name: None,
    })
}

/// 删掉已保存的会话。键不存在是成功的空操作。
#[tauri::command]
pub fn remote_aliyun_delete_session(
    app: AppHandle,
    connection_id: String,
) -> Result<(), RemoteError> {
    let id = connection_id.trim().to_string();
    let dir = app_data_dir(&app)?;
    delete_aliyun_session(&dir, &id)
}

/// 本轮登录的上下文。跨线程共享，所以字段都是 `String`（`Send + Sync`）。
struct LoginContext {
    client_id: String,
    client_secret: Option<String>,
    redirect_uri: String,
    state: String,
    code_verifier: String,
}

/// 在独立线程里建授权窗（原因见模块顶部注释）。
fn spawn_window_builder(app: AppHandle, url: String, context: LoginContext) {
    std::thread::spawn(move || {
        let parsed = match tauri::Url::parse(&url) {
            Ok(url) => url,
            Err(err) => {
                settle();
                emit_event(
                    &app,
                    "error",
                    None,
                    Some(&format!("授权地址无效，无法打开授权窗口：{err}")),
                );
                return;
            }
        };

        // 启动本地回调监听：`redirect_uri` 指向本机时，服务端会把浏览器
        // 重定向到我们这儿。没有监听器的话，那个请求会直接失败，
        // 用户看到的是一个「无法访问此网站」的页面 —— 体验极差。
        let callback = start_callback_listener(&app);

        let state = context.state.clone();
        let mut builder =
            WebviewWindowBuilder::new(&app, LOGIN_WINDOW_LABEL, WebviewUrl::External(parsed))
                .title("阿里云盘授权")
                .inner_size(980.0, 760.0)
                .always_on_top(true);

        // 保险一（主路径）：导航拦截。回调 URL 里带 `code=` ⇒
        // 记下它并**阻断这次导航**（不让浏览器去请求一个不存在的回调地址）。
        //
        // ⚠️ 回调跑在 WebView2 事件循环上 ⇒ 这里**绝不做阻塞 IO**：
        // 只「记下 URL」，剩下的活交给轮询线程。
        builder = builder.on_navigation(move |url| {
            if let Some((code, state_value)) = aliyun_extract_code(url.as_str()) {
                store_code(code, state_value);
                return false;
            }
            let _ = &state;
            true
        });

        let window = match builder.build() {
            Ok(window) => window,
            Err(err) => {
                settle();
                emit_event(
                    &app,
                    "error",
                    None,
                    Some(&format!("打开阿里云盘授权窗口失败，请重试：{err}")),
                );
                return;
            }
        };
        let _ = window.set_focus();

        let watcher = app.clone();
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Destroyed)
                && !SETTLED.swap(false, Ordering::SeqCst)
            {
                emit_event(&watcher, "closed", None, Some("授权窗口已关闭。"));
            }
        });

        start_poller(app.clone(), window, context);
        let _ = callback;
    });
}

/// 本地回调监听器。
///
/// 它的唯一职责是**让浏览器重定向到本机时有个东西应答**（并顺手把
/// 回调里的 `code` 记下来，作为 `on_navigation` 之外的额外一重保险）。
/// 真正的主路径仍然是 `on_navigation`。
///
/// 返回一个句柄：把它 drop 掉就意味着「关掉监听器」。
struct CallbackListener {
    shutdown: Arc<AtomicBool>,
}

impl Drop for CallbackListener {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

/// `Arc` 的本地别名（避免在文件头再引一次 `use`）。
type Arc<T> = std::sync::Arc<T>;

/// 启动本地回调监听。
fn start_callback_listener(app: &AppHandle) -> Option<CallbackListener> {
    // 只在 `redirect_uri` 指向本机时才需要监听。第三方回调地址由浏览器自己处理。
    let listener = TcpListener::bind(("127.0.0.1", CALLBACK_PORT)).ok()?;
    let shutdown = Arc::new(AtomicBool::new(false));

    let thread_shutdown = Arc::clone(&shutdown);
    let app = app.clone();
    std::thread::spawn(move || {
        // `TcpListener` 的 `accept` 是阻塞的 ⇒ 用一个短超时的副本来轮询退出条件。
        listener
            .set_nonblocking(true)
            .expect("本地回调监听器设为非阻塞失败");
        while !thread_shutdown.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = [0u8; 4096];
                    let read = stream.read(&mut buffer).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                    let first_line = request.lines().next().unwrap_or_default().to_string();
                    let target = first_line.split_whitespace().nth(1).unwrap_or_default();
                    let path = target.split('?').next().unwrap_or_default();
                    let query = target
                        .split_once('?')
                        .map(|(_, query)| query)
                        .unwrap_or_default();

                    if path.starts_with(CALLBACK_PATH) {
                        if let Some((code, state)) = parse_callback_query(query) {
                            store_code(code, state);
                        }
                        let _ = stream.write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\n\
                              Content-Length: 17\r\nConnection: close\r\n\r\n\
                              authorization ok\n",
                        );
                    } else {
                        let _ = stream.write_all(
                            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        );
                    }
                    let _ = stream.flush();
                    let _ = app;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => break,
            }
        }
    });

    Some(CallbackListener { shutdown })
}

/// 从回调的查询串里抠出 `code` 与 `state`。
fn parse_callback_query(query: &str) -> Option<(String, Option<String>)> {
    let mut code: Option<String> = None;
    let mut state: Option<String> = None;

    for pair in query.split('&') {
        let Some((raw_key, raw_value)) = pair.split_once('=') else {
            continue;
        };
        let key = urlencoding::decode(raw_key).ok()?;
        let value = urlencoding::decode(raw_value).ok()?;
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            _ => {}
        }
    }

    code.map(|value| (value, state))
}

/// 启动轮询线程。
fn start_poller(app: AppHandle, window: tauri::WebviewWindow, context: LoginContext) {
    let generation = POLLER_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || poll_loop(app, window, context, generation));
}

/// 轮询：等授权码 → 换 token → 存进待认领槽位 → 发事件。
fn poll_loop(app: AppHandle, window: tauri::WebviewWindow, context: LoginContext, generation: u64) {
    let started = Instant::now();

    loop {
        std::thread::sleep(POLL);

        // 被新一轮登录取代了 —— 收工。
        if POLLER_GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }

        // 窗口已经没了（用户关闭 / cancel / 程序化收场）就直接收工。
        if app.get_webview_window(LOGIN_WINDOW_LABEL).is_none() {
            return;
        }

        // 超时判定必须排在读码**之前**：读一旦挂住，排后面的超时分支就永远走不到。
        if started.elapsed() >= GIVE_UP {
            settle();
            emit_event(&app, "timeout", None, Some("等待授权超时，请重试。"));
            close_login_window(&app);
            return;
        }

        // 保险二（兜底）：`eval` 读 `location.href`。
        //
        // `on_navigation` 是主路径；这一条只在它没命中时起作用
        // （例如回调是以页面内跳转的形式发生的）。
        //
        // ⚠️ wry 的 `eval` 是同步 API ⇒ 必须放独立线程并带超时预算，
        // 否则一次挂住就把整个轮询线程带走。
        if code_lock().is_none() {
            poll_location_for_code(&window);
        }

        let captured = code_lock().take();
        let Some(captured) = captured else {
            continue;
        };

        // 核对 `state`：不匹配 ⇒ 拒绝（可能是别的会话发来的回调）。
        if !aliyun_state_matches(&context.state, captured.state.as_deref()) {
            settle();
            emit_event(
                &app,
                "error",
                None,
                Some("授权回调校验失败（state 不匹配），请重试。"),
            );
            close_login_window(&app);
            return;
        }

        match exchange_code_for_session(&context, &captured.code) {
            Ok(session) => {
                store_pending(PendingSession { session });
                settle();
                emit_event(&app, "ready", Some(aliyun_describe_session(None)), None);
                close_login_window(&app);
                return;
            }
            Err(error) => {
                settle();
                emit_event(
                    &app,
                    "error",
                    None,
                    Some(&format!("换取访问凭证失败：{}", error.message)),
                );
                close_login_window(&app);
                return;
            }
        }
    }
}

/// 用 `eval` 读 `location.href`，从中抠出授权码。
///
/// 页面里跑的脚本把结果回传给一个本地监听端口 —— `eval` 没有返回值通道
/// （本 crate 未开 tauri 的 `ipc` feature ⇒ `eval_with_callback` 不可用），
/// 所以用「让页面自己发起一个请求」的办法把值带回来。
fn poll_location_for_code(window: &tauri::WebviewWindow) {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let worker_window = window.clone();
    std::thread::spawn(move || {
        let script = format!(
            r#"(function () {{
  try {{
    var href = String(location.href || '');
    if (href.indexOf('code=') < 0) {{ return; }}
    var img = new Image();
    img.src = 'http://127.0.0.1:{port}{path}?code=' +
              encodeURIComponent(href);
  }} catch (e) {{}}
}})();"#,
            port = CALLBACK_PORT,
            path = CALLBACK_PATH
        );
        let _ = worker_window.eval(&script);
        let _ = tx.send(());
    });

    // 预算 5 秒：读一次最多等这么久，绝不让轮询线程被它拖住。
    let _ = rx.recv_timeout(Duration::from_secs(5));
}

/// 用授权码换一份会话。
///
/// 密钥模式与 PKCE 模式走**同一条**代码路径，唯一差异是有没有
/// `client_secret`（PKCE 的 `code_verifier` 两种模式都带）。
fn exchange_code_for_session(
    context: &LoginContext,
    code: &str,
) -> Result<AliyunAuthSession, RemoteError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| RemoteError::unknown("无法创建网络运行时，请重试。"))?;

    let payload = runtime.block_on(async {
        let client = reqwest::Client::builder()
            .timeout(crate::remote::ALIYUN_OPERATION_TIMEOUT)
            // 不跟随重定向：重定向在这里意味着「授权地址不对」。
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| RemoteError::unknown(format!("HTTP 客户端初始化失败：{error}")))?;

        let form = crate::remote::aliyun_authorization_code_form(
            &context.client_id,
            context.client_secret.as_deref(),
            &context.redirect_uri,
            code,
            Some(&context.code_verifier),
        );

        let response = client
            .post(crate::remote::ALIYUN_ACCESS_TOKEN_URL)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form)
            .send()
            .await
            .map_err(|error| {
                RemoteError::unreachable(format!(
                    "无法连接阿里云盘授权服务，请检查网络后重试：{error}"
                ))
            })?;

        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|error| RemoteError::unreachable(format!("读取授权响应失败：{error}")))?;
        let body = String::from_utf8(bytes.to_vec())
            .map_err(|error| RemoteError::encoding(format!("授权响应不是合法的 UTF-8：{error}")))?;

        if !status.is_success() {
            return Err(crate::remote::aliyun_map_failure(status.as_u16(), &body));
        }

        crate::remote::aliyun_parse_token_payload(&body)
    })?;

    Ok(crate::remote::aliyun_session_from_token_payload(
        &payload,
        &context.client_id,
        context.client_secret.clone(),
        Some(context.redirect_uri.clone()),
        Some(ALIYUN_DEFAULT_SCOPE.to_string()),
        aliyun_now_unix(),
        None,
        None,
    ))
}

/// 发事件。
fn emit_event(
    app: &AppHandle,
    status: &'static str,
    summary: Option<String>,
    message: Option<&str>,
) {
    let _ = app.emit(
        LOGIN_EVENT,
        AliyunLoginEvent {
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

/// 当前 Unix 秒。
#[allow(dead_code)]
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
