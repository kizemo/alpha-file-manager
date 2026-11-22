// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! OneDrive 内嵌登录（Microsoft 身份平台 v2.0 授权码 + PKCE）的命令层。
//!
//! ## 为什么这个文件在 `src-tauri/src/` 顶层
//!
//! `src-tauri/src/remote/` 是**无 tauri 的内核层**（该目录 `mod.rs` 顶部有说明）。
//! 这里的 7 条命令都需要 `AppHandle` —— app-data 目录是读写 DPAPI 密文的唯一入口，
//! 而 `app_dirs::app_data_dir` 必须有 `Manager` ⇒ 只能放顶层，
//! 与 `aliyun_login.rs` / `quark_login.rs` 同一处境。
//!
//! ## 登录流程
//!
//! 1. `remote_onedrive_login_start`：在 `127.0.0.1:0` 上绑一个回环监听器，
//!    生成 `state` 与 PKCE `code_verifier`，拼出授权页地址，并（缺省）用系统
//!    浏览器打开它。
//! 2. 用户在微软页面上登录并同意授权 ⇒ 浏览器跳回 `http://localhost:{port}`。
//! 3. 回环监听器在后台线程里收下这一跳，校验 `state`，用授权码换令牌，
//!    再顺手打一次 `/me/drive` 把账号名与 drive id 补进会话（失败不阻断登录）。
//! 4. 前端轮询 `remote_onedrive_login_pending` 看阶段。
//! 5. `remote_onedrive_login_claim(connectionId)`：把会话按 `connectionId`
//!    写进 DPAPI 密文，并回一份登录状态。
//!
//! ## 令牌只在三处流转
//!
//! 本模块、`remote::onedrive_provider`、DPAPI 密文。**不进命令返回值、
//! 不进事件载荷、不进日志。**

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

// 新模块之间**一律走直连模块路径**，不依赖 `remote/mod.rs` 里的 `pub use`
// 再导出（那部分由根会话另派 worker 补）。
use crate::remote::onedrive_api::ONEDRIVE_USER_AGENT;
use crate::remote::onedrive_auth::{
    build_authorize_url as onedrive_build_authorize_url, delete_onedrive_session,
    load_onedrive_session, onedrive_auth_status_payload, onedrive_decode_session,
    onedrive_form_body, onedrive_generate_code_verifier, onedrive_generate_state,
    onedrive_now_unix, onedrive_parse_token_response,
    pkce_code_challenge as onedrive_pkce_code_challenge, save_onedrive_session,
    OnedriveAuthSession, OnedriveAuthStatusPayload, ONEDRIVE_SCOPE, ONEDRIVE_TOKEN_URL,
};
use crate::remote::onedrive_provider::onedrive_fetch_drive_info;
// `RemoteError` 走 `remote/mod.rs` 里**本来就有**的再导出 —— `error` 模块本身
// 是私有的，从 crate 根够不到它。
use crate::remote::RemoteError;

/// 回环监听等待授权回调的上限。5 分钟足够走完一次 MFA。
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

/// 轮询回环监听的间隔。50ms 足够跟手，也不至于把 CPU 烧掉。
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// 读取回调请求头的字节上限。
const MAX_REQUEST_BYTES: usize = 8 * 1024;

/// HTTP 客户端的总超时。
const HTTP_TIMEOUT: Duration = Duration::from_secs(60);

/// 登录流程的阶段。**这四个值是契约**，前端按字符串分派。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OnedriveLoginState {
    Idle,
    Pending,
    Completed,
    Failed,
}

impl OnedriveLoginState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

/// `remote_onedrive_login_start` 的返回值。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnedriveLoginStartPayload {
    /// 授权页地址。前端在「不开浏览器」时把它显示出来让用户自己复制。
    pub auth_url: String,
    /// 回环回调地址。**必须**与授权请求里的 `redirect_uri` 逐字一致。
    pub redirect_uri: String,
}

/// `remote_onedrive_login_pending` 的返回值。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnedriveLoginPendingPayload {
    pub state: OnedriveLoginState,
    /// 失败原因。没有失败时是 `null`。
    ///
    /// **始终序列化**（缺 `skip_serializing_if`）—— 前端的类型是
    /// `error?: string | null`，字段整个消失会让「有没有出错」变成
    /// 「字段在不在」，两件事混成一件。
    pub error: Option<String>,
    /// 当前是否已经拿到可用的会话。
    pub authorized: bool,
}

/// 全局槽位里的一份登录状态。
struct LoginSlot {
    state: OnedriveLoginState,
    error: Option<String>,
    session: Option<OnedriveAuthSession>,
}

/// 登录轮次计数器。
///
/// 每开一轮登录就 +1。还在等回调的后台线程在写回状态前会核对轮次 ——
/// **上一轮的线程不许覆盖新一轮的状态**，否则用户第二次登录会被第一次的
/// 迟到回调打成「失败」。
static LOGIN_GENERATION: AtomicU64 = AtomicU64::new(0);

static LOGIN_SLOT: OnceLock<Mutex<Option<LoginSlot>>> = OnceLock::new();

fn login_slot() -> MutexGuard<'static, Option<LoginSlot>> {
    LOGIN_SLOT
        .get_or_init(|| Mutex::new(None))
        .lock()
        // 毒化了的锁不代表数据坏了 —— 拿回数据继续用，别把登录功能整个废掉。
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 应用数据目录。
fn app_data_dir(app: &AppHandle) -> Result<PathBuf, RemoteError> {
    crate::app_dirs::app_data_dir(app)
        .map_err(|error| RemoteError::unknown(format!("无法定位应用数据目录：{error}")))
}

/// 一次性 current-thread tokio runtime。
///
/// 本仓 reqwest 没开 `blocking` 特性，而后台回调线程是**普通 OS 线程**
/// （不是 Tauri 的异步运行时）⇒ 自己建一个用完即弃的 runtime。
fn run_blocking<F: std::future::Future>(future: F) -> Result<F::Output, RemoteError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| RemoteError::unknown(format!("无法初始化网络运行时：{error}")))?;

    Ok(runtime.block_on(future))
}

/// 建一个带 UA 的 HTTP 客户端。不跟随重定向：令牌端点不会重定向，
/// 真发生重定向说明中间人插进来了。
fn build_client() -> Result<reqwest::Client, RemoteError> {
    reqwest::Client::builder()
        .user_agent(ONEDRIVE_USER_AGENT)
        .timeout(HTTP_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| RemoteError::unknown(format!("OneDrive HTTP 客户端初始化失败：{error}")))
}

// ---------------------------------------------------------------------------
// 回环监听
// ---------------------------------------------------------------------------

/// 回环监听器。
struct LoopbackListener {
    v4: TcpListener,
    v6: Option<TcpListener>,
    port: u16,
    generation: u64,
}

/// 回环回调地址。**必须**是 `http://localhost:{port}` —— 微软那边登记的
/// 重定向 URI 就是这个形状。
fn redirect_uri_for_port(port: u16) -> String {
    format!("http://localhost:{port}")
}

impl LoopbackListener {
    fn bind(generation: u64) -> Result<Self, RemoteError> {
        let v4 = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|error| {
            RemoteError::unreachable(format!("无法在 127.0.0.1 上监听 OneDrive 登录回调：{error}"))
        })?;

        let port = v4
            .local_addr()
            .map(|address| address.port())
            .unwrap_or_default();

        // `localhost` 在 Windows 上会先解析到 `::1`。同一个端口再尽力绑一次 IPv6，
        // 绑不上也不影响 —— 浏览器连 `::1` 失败后会退回到 `127.0.0.1`。
        let v6 = TcpListener::bind((Ipv6Addr::LOCALHOST, port)).ok();

        let _ = v4.set_nonblocking(true);
        if let Some(listener) = &v6 {
            let _ = listener.set_nonblocking(true);
        }

        Ok(Self {
            v4,
            v6,
            port,
            generation,
        })
    }

    fn redirect_uri(&self) -> String {
        redirect_uri_for_port(self.port)
    }

    fn wait_for_callback(&self, timeout: Duration) -> CallbackOutcome {
        let deadline = Instant::now() + timeout;

        loop {
            if LOGIN_GENERATION.load(Ordering::SeqCst) != self.generation {
                return CallbackOutcome::Cancelled;
            }
            if Instant::now() >= deadline {
                return CallbackOutcome::Timeout;
            }

            if let Some((stream, _)) = accept_nonblocking(&self.v4) {
                return self.handle(stream);
            }
            if let Some(listener) = &self.v6 {
                if let Some((stream, _)) = accept_nonblocking(listener) {
                    return self.handle(stream);
                }
            }

            std::thread::sleep(POLL_INTERVAL);
        }
    }

    /// 收下这一跳，回一句人话（免得浏览器那一页顶着一个空白）。
    fn handle(&self, mut stream: TcpStream) -> CallbackOutcome {
        let mut buffer = [0u8; MAX_REQUEST_BYTES];
        let read = stream.read(&mut buffer).unwrap_or(0);
        let request = String::from_utf8_lossy(&buffer[..read]).to_string();

        let query = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|target| target.split_once('?'))
            .map(|(_, query)| query.to_string())
            .unwrap_or_default();

        let params = parse_query(&query);

        let code = params.get("code").cloned();
        let state = params.get("state").cloned();
        let error = params.get("error").cloned();

        let body = if error.is_some() {
            "OneDrive 授权未完成，可以关掉这个标签页了。"
        } else {
            "OneDrive 授权完成，可以关掉这个标签页了。"
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();

        CallbackOutcome::Redirect {
            code,
            state,
            error,
        }
    }
}

fn accept_nonblocking(listener: &TcpListener) -> Option<(TcpStream, SocketAddr)> {
    match listener.accept() {
        Ok(pair) => Some(pair),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
        Err(_) => None,
    }
}

/// 后台线程等到的回调结果。
enum CallbackOutcome {
    /// 轮次已经变了（用户又开了一轮，或点了取消）。
    Cancelled,
    /// 等太久没人来。
    Timeout,
    /// 收到了这一跳。
    Redirect {
        code: Option<String>,
        state: Option<String>,
        error: Option<String>,
    },
}

/// 解析 query string。键与值都做百分号解码。
fn parse_query(query: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();

    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (raw_key, raw_value) = pair.split_once('=').unwrap_or((pair, ""));
        let key = urlencoding::decode(raw_key)
            .map(|value| value.into_owned())
            .unwrap_or_else(|_| raw_key.to_string());
        let value = urlencoding::decode(raw_value)
            .map(|value| value.into_owned())
            .unwrap_or_else(|_| raw_value.to_string());
        params.insert(key, value);
    }

    params
}

// ---------------------------------------------------------------------------
// 换令牌
// ---------------------------------------------------------------------------

/// 用授权码换令牌。
///
/// **不带 `client_secret`** —— 桌面应用是「公开客户端」，靠 PKCE 证明自己；
/// 带了反而会被微软拒（`AADSTS7000218`）。
async fn exchange_code(
    client_id: &str,
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> Result<OnedriveAuthSession, RemoteError> {
    let body = onedrive_form_body(&[
        ("client_id", client_id),
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("code_verifier", code_verifier),
        ("scope", ONEDRIVE_SCOPE),
    ]);

    let client = build_client()?;

    let response = client
        .post(ONEDRIVE_TOKEN_URL)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .map_err(|error| {
            RemoteError::unreachable(format!("连接微软登录服务器失败，请检查网络后重试：{error}"))
        })?;

    let status = response.status();
    let bytes = response.bytes().await.map_err(|error| {
        RemoteError::disconnected(format!("读取微软登录服务器的响应失败：{error}"))
    })?;
    let text = String::from_utf8(bytes.to_vec())
        .map_err(|_| RemoteError::encoding("微软登录服务器的响应不是合法的 UTF-8 文本。"))?;

    if !status.is_success() {
        // **不回显响应原文** —— 里面有授权码。
        return Err(RemoteError::auth(format!(
            "OneDrive 授权码兑换失败（HTTP {}），请重新登录。",
            status.as_u16()
        )));
    }

    onedrive_parse_token_response(client_id, &text, onedrive_now_unix(), None)
}

/// 顺手补上账号名与 drive id。
///
/// **失败不阻断登录** —— 这两项只是显示用的元信息，拿不到照样能用。
async fn enrich_session(session: &mut OnedriveAuthSession) {
    let Ok(info) = onedrive_fetch_drive_info(&session.access_token).await else {
        return;
    };

    if session.account.is_none() {
        session.account = info.account_name();
    }
    if session.drive_id.is_none() {
        session.drive_id = info.id.clone();
    }
}

// ---------------------------------------------------------------------------
// 槽位写入
// ---------------------------------------------------------------------------

fn finish_completed(generation: u64, session: OnedriveAuthSession) {
    if LOGIN_GENERATION.load(Ordering::SeqCst) != generation {
        return;
    }

    let mut guard = login_slot();
    *guard = Some(LoginSlot {
        state: OnedriveLoginState::Completed,
        error: None,
        session: Some(session),
    });
}

fn finish_failed(generation: u64, message: &str) {
    if LOGIN_GENERATION.load(Ordering::SeqCst) != generation {
        return;
    }

    let mut guard = login_slot();
    *guard = Some(LoginSlot {
        state: OnedriveLoginState::Failed,
        error: Some(message.to_string()),
        session: None,
    });
}

/// 起后台线程等回调。**线程失败不影响命令本身** —— 授权页地址已经给出去了，
/// 用户可以自己复制到浏览器里完成授权。
fn spawn_callback_worker(
    listener: LoopbackListener,
    client_id: String,
    code_verifier: String,
    redirect_uri: String,
    state: String,
    generation: u64,
) {
    let _ = std::thread::Builder::new()
        .name("onedrive-oauth-callback".to_string())
        .spawn(move || {
            let outcome = listener.wait_for_callback(CALLBACK_TIMEOUT);

            let CallbackOutcome::Redirect {
                code,
                state: got_state,
                error,
            } = outcome
            else {
                match outcome {
                    CallbackOutcome::Timeout => {
                        finish_failed(generation, "等待 OneDrive 授权回调超时，请重新发起登录。")
                    }
                    CallbackOutcome::Cancelled => return,
                    CallbackOutcome::Redirect { .. } => unreachable!(),
                }
                return;
            };

            if let Some(code) = error {
                finish_failed(generation, &format!("OneDrive 授权未通过（{code}）。"));
                return;
            }

            let Some(code) = code else {
                finish_failed(generation, "OneDrive 回调里没有授权码，请重新登录。");
                return;
            };

            if got_state.as_deref() != Some(state.as_str()) {
                finish_failed(generation, "OneDrive 回调的 state 对不上，已拒绝这次授权。");
                return;
            }

            let exchanged = run_blocking(async {
                let mut session =
                    exchange_code(&client_id, &code, &code_verifier, &redirect_uri).await?;
                enrich_session(&mut session).await;
                Ok::<OnedriveAuthSession, RemoteError>(session)
            })
            .and_then(|inner| inner);

            match exchanged {
                Ok(session) => finish_completed(generation, session),
                Err(error) => finish_failed(generation, &error.message),
            }
        });
}

// ---------------------------------------------------------------------------
// 命令
// ---------------------------------------------------------------------------

/// 开始一轮 OneDrive 登录。
///
/// `open_browser` 缺省 = 打开系统浏览器；`Some(false)` 时只把 `authUrl`
/// 交给前端（用户自己复制到别处去授权）。
#[tauri::command]
pub async fn remote_onedrive_login_start(
    app: AppHandle,
    client_id: String,
    open_browser: Option<bool>,
) -> Result<OnedriveLoginStartPayload, RemoteError> {
    let client_id = client_id.trim().to_string();
    if client_id.is_empty() {
        return Err(RemoteError::protocol(
            "OneDrive 登录需要填写应用（客户端）ID。",
        ));
    }

    // 上一轮登录还悬着 ⇒ 作废它。轮次前进之后，
    // 上一轮那个还在等回调的线程就写不回状态了。
    let generation = LOGIN_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    let listener = LoopbackListener::bind(generation)?;
    let redirect_uri = listener.redirect_uri();

    let state = onedrive_generate_state();
    let code_verifier = onedrive_generate_code_verifier();
    let challenge = onedrive_pkce_code_challenge(&code_verifier);
    let auth_url = onedrive_build_authorize_url(&client_id, &redirect_uri, &state, &challenge);

    {
        let mut guard = login_slot();
        *guard = Some(LoginSlot {
            state: OnedriveLoginState::Pending,
            error: None,
            session: None,
        });
    }

    spawn_callback_worker(
        listener,
        client_id,
        code_verifier,
        redirect_uri.clone(),
        state,
        generation,
    );

    if open_browser.unwrap_or(true) {
        app.opener()
            .open_url(auth_url.clone(), None::<&str>)
            .map_err(|error| RemoteError::unknown(format!("打开 OneDrive 登录页失败：{error}")))?;
    }

    Ok(OnedriveLoginStartPayload {
        auth_url,
        redirect_uri,
    })
}

/// 这一轮登录进行到哪儿了。前端轮询它。
#[tauri::command]
pub fn remote_onedrive_login_pending() -> OnedriveLoginPendingPayload {
    let guard = login_slot();

    match guard.as_ref() {
        None => OnedriveLoginPendingPayload {
            state: OnedriveLoginState::Idle,
            error: None,
            authorized: false,
        },
        Some(slot) => OnedriveLoginPendingPayload {
            state: slot.state,
            error: slot.error.clone(),
            authorized: slot
                .session
                .as_ref()
                .map(|session| session.is_usable(onedrive_now_unix()))
                .unwrap_or(false),
        },
    }
}

/// 把这一轮登录的结果存进 DPAPI 密文（键 = 连接 id），并回一份登录状态。
#[tauri::command]
pub fn remote_onedrive_login_claim(
    app: AppHandle,
    connection_id: String,
) -> Result<OnedriveAuthStatusPayload, RemoteError> {
    let key = connection_id.trim().to_string();
    if key.is_empty() {
        return Err(RemoteError::protocol("领取 OneDrive 登录结果需要一条连接 id。"));
    }

    let session = {
        let mut guard = login_slot();
        let slot = guard.as_mut().ok_or_else(|| {
            RemoteError::auth("还没有发起过 OneDrive 登录，请先点「登录 OneDrive」。")
        })?;

        if slot.state != OnedriveLoginState::Completed {
            return Err(RemoteError::auth(match slot.state {
                OnedriveLoginState::Failed => slot
                    .error
                    .clone()
                    .unwrap_or_else(|| "OneDrive 登录失败，请重新登录。".to_string()),
                OnedriveLoginState::Pending => "OneDrive 登录还在进行中，请稍候。".to_string(),
                _ => "OneDrive 登录还没有完成。".to_string(),
            }));
        }

        slot.session
            .clone()
            .ok_or_else(|| RemoteError::auth("OneDrive 登录结果里没有会话，请重新登录。"))?
    };

    let dir = app_data_dir(&app)?;
    save_onedrive_session(&dir, &key, &session)?;

    Ok(onedrive_auth_status_payload(Some(&session), onedrive_now_unix()))
}

/// 取消这一轮登录。
#[tauri::command]
pub fn remote_onedrive_login_cancel() -> Result<(), RemoteError> {
    // 轮次前进 ⇒ 还在等回调的线程会自己退出。
    LOGIN_GENERATION.fetch_add(1, Ordering::SeqCst);

    let mut guard = login_slot();
    *guard = None;

    Ok(())
}

/// 查一条连接当前有没有可用的 OneDrive 会话。
///
/// **不返回 `Err`** —— 前端拿它决定「显示登录按钮还是显示账号名」，
/// 返回错误会让每一次打开连接设置页都变成一次未捕获异常。
#[tauri::command]
pub fn remote_onedrive_auth_status(
    app: AppHandle,
    connection_id: String,
) -> OnedriveAuthStatusPayload {
    let key = connection_id.trim().to_string();
    if key.is_empty() {
        return onedrive_auth_status_payload(None, onedrive_now_unix());
    }

    let dir = match app_data_dir(&app) {
        Ok(dir) => dir,
        Err(error) => {
            log::warn!("读取 OneDrive 登录状态失败（定位不到应用数据目录）：{error}");
            return onedrive_auth_status_payload(None, onedrive_now_unix());
        }
    };

    match load_onedrive_session(&dir, &key) {
        Ok(Some(session)) => onedrive_auth_status_payload(Some(&session), onedrive_now_unix()),
        Ok(None) => onedrive_auth_status_payload(None, onedrive_now_unix()),
        Err(error) => {
            log::warn!("读取 OneDrive 登录状态失败：{error}");
            onedrive_auth_status_payload(None, onedrive_now_unix())
        }
    }
}

/// 前端自己换到令牌后交给后端落盘。
#[tauri::command]
pub fn remote_onedrive_save_session(
    app: AppHandle,
    connection_id: String,
    client_id: String,
    session: String,
) -> Result<(), RemoteError> {
    let key = connection_id.trim().to_string();
    if key.is_empty() {
        return Err(RemoteError::protocol("保存 OneDrive 会话需要一条连接 id。"));
    }

    let mut decoded = onedrive_decode_session(&session)?;

    // 前端传的 `client_id` 非空时以它为准 —— 刷新令牌要原样带回去。
    let trimmed = client_id.trim();
    if !trimmed.is_empty() {
        decoded.client_id = trimmed.to_string();
    }

    let dir = app_data_dir(&app)?;
    save_onedrive_session(&dir, &key, &decoded)
}

/// 删掉一条连接的 OneDrive 会话（登出）。
#[tauri::command]
pub fn remote_onedrive_delete_session(
    app: AppHandle,
    connection_id: String,
) -> Result<(), RemoteError> {
    let key = connection_id.trim().to_string();
    if key.is_empty() {
        return Err(RemoteError::protocol("删除 OneDrive 会话需要一条连接 id。"));
    }

    let dir = app_data_dir(&app)?;
    delete_onedrive_session(&dir, &key)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 阶段名是契约：前端按字符串分派，改名会让「登录中」显示不出来。
    #[test]
    fn the_login_state_names_are_locked_to_the_contract() {
        assert_eq!(OnedriveLoginState::Idle.as_str(), "idle");
        assert_eq!(OnedriveLoginState::Pending.as_str(), "pending");
        assert_eq!(OnedriveLoginState::Completed.as_str(), "completed");
        assert_eq!(OnedriveLoginState::Failed.as_str(), "failed");

        assert_eq!(
            serde_json::to_string(&OnedriveLoginState::Completed).expect("serialize"),
            "\"completed\""
        );
    }

    /// `error` 字段必须**始终**在场 —— 少���时「有没有出错」会变成
    /// 「字段在不在」。
    #[test]
    fn the_pending_payload_always_carries_the_error_field() {
        let payload = OnedriveLoginPendingPayload {
            state: OnedriveLoginState::Idle,
            error: None,
            authorized: false,
        };

        let json = serde_json::to_value(&payload).expect("serialize");

        assert!(json.get("error").is_some(), "error 字段必须始终在场：{json}");
        assert!(json["error"].is_null());
        assert_eq!(json["state"], "idle");
        assert_eq!(json["authorized"], false);
    }

    #[test]
    fn a_query_string_is_decoded_into_its_parts() {
        let params = parse_query("code=abc%2Fdef%3D&state=xyz&error=access_denied");

        assert_eq!(params.get("code").map(String::as_str), Some("abc/def="));
        assert_eq!(params.get("state").map(String::as_str), Some("xyz"));
        assert_eq!(params.get("error").map(String::as_str), Some("access_denied"));
    }

    #[test]
    fn an_empty_query_string_yields_no_parameters() {
        assert!(parse_query("").is_empty());
    }

    /// 回环地址的形状是微软那边登记的重定向 URI，改了就换不到令牌。
    #[test]
    fn the_redirect_uri_is_built_from_the_loopback_port() {
        assert_eq!(
            redirect_uri_for_port(51234),
            "http://localhost:51234"
        );
    }
}
