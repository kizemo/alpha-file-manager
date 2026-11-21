// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! 阿里云盘「扫码登录」的命令层（只读，批次 29）。
//!
//! ## 为什么是扫码，不是开放平台的 OAuth 授权码
//!
//! 证据：`.superpowers/sdd/b28-api-probe.md`。
//! - **QR 直出**（`query.do` → `CONFIRMED` → `bizExt` → `pds_login_result.accessToken`）
//!   拿到的 token 实测能直接当 Bearer 用（`/v2/user/get` 200、`file/list` 200）。
//! - **OAuth code 换 token**（`POST /token/get`）拿到的 41 字符 token
//!   **不能**当 Bearer（HTTP 400 `InvalidHeader`）⇒ 这条路**不可用**。
//!
//! ⇒ 本模块只走 QR。开放平台那套 OAuth 的常量与函数留在 `remote::aliyun_api`
//! 里**不接线**，并在注释里写明「实测不可用」—— 不假装它是可用能力。
//!
//! ## 流程
//!
//! ```text
//! remote_aliyun_qr_start()            → 取二维码票据（code_content / ck / t）
//! remote_aliyun_qr_poll(ck, t)        → 轮询状态；CONFIRMED 时解 bizExt、
//!                                      打一次 /v2/user/get 探活、存进「待认领槽位」
//! remote_aliyun_save_session(id, tok) → 「粘贴 access_token」兜底：探活后直接落盘
//! remote_aliyun_login_pending()       → 前端打开表单时兜底查询
//! remote_aliyun_login_claim(id)       → 把待认领会话写进 DPAPI 密文（键 = connection_id）
//! remote_aliyun_login_cancel()        → 清槽位
//! remote_aliyun_auth_status(id)       → 授权状态卡（missing / ready / stale / stale_soon）
//! remote_aliyun_delete_session(id)    → 删会话（键不存在是成功的空操作）
//! ```
//!
//! ## 权限边界
//!
//! `access_token` / `refresh_token` **明文只在本模块与 `remote::aliyun_auth`
//! 之间流转**，不出现在命令返回值、事件 payload 与日志里。
//! 落盘由 `aliyun_auth::save_aliyun_session` 委托 `credentials.rs`（DPAPI）完成。
//!
//! ⚠️ **不得引用 `tauri::async_runtime::*`**。`cargo test --lib` 会链接整个
//! crate，`GlobalRuntime<Wry>` 一旦被单态化就会把 tao 拖进测试二进制，
//! 进程在**加载阶段**以 `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉。
//! 需要跑 HTTP 时自己建一个 current-thread tokio runtime（同 `quark_login.rs`）。

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use serde::Serialize;
use tauri::AppHandle;

use crate::remote::{
    aliyun_decode_biz_ext, aliyun_describe_session, aliyun_expire_time_to_unix, aliyun_now_unix,
    aliyun_parse_qr_poll, aliyun_parse_qr_ticket, aliyun_parse_user_profile,
    aliyun_qr_generate_query, aliyun_qr_query_form, aliyun_qr_query_query, aliyun_status_name,
    aliyun_user_info_request_body, delete_aliyun_session, load_aliyun_session,
    save_aliyun_session, AliyunAuthSession, AliyunQrClientContext, AliyunQrCredentials,
    AliyunQrStatus, AliyunUserProfile, RemoteError, ALIYUN_QR_GENERATE_URL,
    ALIYUN_QR_QUERY_URL, ALIYUN_USER_AGENT, ALIYUN_USER_INFO_URL, ALIYUN_WEB_CLIENT_ID,
};

/// 单次请求的超时。
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

/// 待认领的会话（已拿到 token，还没写进某条连接）。
struct PendingSession {
    session: AliyunAuthSession,
}

/// 待认领槽位。
type PendingSlot = Mutex<Option<PendingSession>>;

fn pending_slot() -> &'static PendingSlot {
    static PENDING: OnceLock<PendingSlot> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(None))
}

fn pending_lock() -> MutexGuard<'static, Option<PendingSession>> {
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

/// 当前扫码会话的客户端上下文（按 `ck` 存）。
///
/// 一次只有一张码在扫 ⇒ 一个槽位就够。
/// 存不下（进程重启 / 换了 ck）就退化成一份新上下文 ——
/// 实测服务端**按 `ck` 查会话**（`b29-accept-run/key-probe-01.json`），
/// 所以退化只丢「同一组客户端标识」，不会让会话查不到。
type QrContextSlot = Mutex<Option<(String, AliyunQrClientContext)>>;

fn qr_context_slot() -> &'static QrContextSlot {
    static SLOT: OnceLock<QrContextSlot> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn qr_context_lock() -> MutexGuard<'static, Option<(String, AliyunQrClientContext)>> {
    qr_context_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 记下「这张码属于哪份上下文」。
fn store_qr_context(ck: &str, context: &AliyunQrClientContext) {
    *qr_context_lock() = Some((ck.to_string(), context.clone()));
}

/// 取回当初出码时用的那份上下文；取不到就临时生成一份。
fn qr_context_for(ck: &str) -> AliyunQrClientContext {
    let guard = qr_context_lock();

    match guard.as_ref() {
        Some((stored_ck, context)) if stored_ck == ck => context.clone(),
        _ => AliyunQrClientContext::random(),
    }
}

/// 二维码票据。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliyunQrStartPayload {
    /// 二维码内容（前端用 `qrcode` 库渲染成图片）。
    pub code_content: String,
    /// 轮询用的会话标识。
    pub ck: String,
    /// 轮询用的时间戳。
    pub t: Option<i64>,
}

/// 扫码轮询的结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliyunQrPollPayload {
    /// `new` | `scanned` | `confirmed` | `expired` | `canceled` | `error`
    pub status: String,
    /// 面向用户的短说明（**不含任何凭据**）。
    pub message: Option<String>,
    /// 是不是终态（前端据此停止轮询）。
    pub final_status: bool,
}

/// 有没有待认领的会话。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliyunLoginPendingPayload {
    pub pending: bool,
    /// 脱敏描述。
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
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliyunAuthStatusPayload {
    /// `missing` | `ready` | `stale` | `stale_soon`
    pub status: String,
    /// 距过期还剩多少秒。`None` = 不知道过期时间。
    pub remaining_seconds: Option<i64>,
    /// 脱敏描述。
    pub summary: String,
    /// 应用 id（**不是密钥**）。扫码登录没有开放平台应用 ⇒ 一般是空。
    pub client_id: Option<String>,
    /// 是否配了 `client_secret`。扫码登录**没有**密钥 ⇒ 恒为 `false`。
    pub has_client_secret: bool,
    /// 申请的 scope。
    pub scope: Option<String>,
    /// 过期时刻（Unix 秒）。
    pub expires_at: Option<i64>,
    /// 空间 id。
    pub drive_id: Option<String>,
    /// 账号昵称。
    pub nick_name: Option<String>,
}

/// 应用数据目录。
fn app_data_dir(app: &AppHandle) -> Result<PathBuf, RemoteError> {
    crate::app_dirs::app_data_dir(app)
        .map_err(|err| RemoteError::unknown(format!("无法定位应用数据目录：{err}")))
}

/// 一次性 current-thread tokio runtime。
///
/// 本仓 reqwest 没开 `blocking` 特性，而这里**绝不能**用
/// `tauri::async_runtime`（见模块顶部注释）⇒ 自己建一个用完即弃的 runtime。
fn run_blocking<F: std::future::Future>(future: F) -> Result<F::Output, RemoteError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| RemoteError::unknown(format!("无法初始化网络运行时：{error}")))?;

    Ok(runtime.block_on(future))
}

/// 建一个带 UA 的 HTTP 客户端。
fn build_client() -> Result<reqwest::Client, RemoteError> {
    reqwest::Client::builder()
        .user_agent(ALIYUN_USER_AGENT)
        .timeout(HTTP_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| RemoteError::unknown(format!("阿里云盘 HTTP 客户端初始化失败：{error}")))
}

/// 把响应体读成 UTF-8 文本；非 2xx ⇒ 明确报错（**不回显响应原文**）。
async fn read_text(
    response: reqwest::Response,
    what: &'static str,
) -> Result<String, RemoteError> {
    let status = response.status();

    if !status.is_success() {
        return Err(RemoteError::protocol(format!(
            "{what}被阿里云盘拒绝了（HTTP {}）。",
            status.as_u16()
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|_| RemoteError::disconnected("与阿里云盘的连接中断，请检查网络后重试。"))?;

    String::from_utf8(bytes.to_vec())
        .map_err(|_| RemoteError::encoding("阿里云盘返回的内容编码无法解析。"))
}

/// `GET` 一个地址并读回 UTF-8 文本。
async fn http_get_text(url: &str) -> Result<String, RemoteError> {
    let client = build_client()?;

    let response = client.get(url).send().await.map_err(|error| {
        RemoteError::unreachable(format!(
            "无法连接阿里云盘：网络不可用、DNS 解析失败或服务器超时，请检查网络连接后重试。（{error}）"
        ))
    })?;

    read_text(response, "阿里云盘的接口").await
}

/// `POST` 一个表单并读回 UTF-8 文本。
async fn http_post_form_text(url: &str, body: &str) -> Result<String, RemoteError> {
    let client = build_client()?;

    let response = client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body.to_string())
        .send()
        .await
        .map_err(|error| {
            RemoteError::unreachable(format!(
                "无法连接阿里云盘：网络不可用、DNS 解析失败或服务器超时，请检查网络连接后重试。（{error}）"
            ))
        })?;

    read_text(response, "阿里云盘的接口").await
}

/// `POST` 一个 JSON（带 `Authorization: Bearer`）并读回 UTF-8 文本。
async fn http_post_json_bearer(
    url: &str,
    access_token: &str,
    body: &str,
) -> Result<String, RemoteError> {
    let client = build_client()?;

    let response = client
        .post(url)
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", access_token.trim()),
        )
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .await
        .map_err(|error| {
            RemoteError::unreachable(format!(
                "无法连接阿里云盘：网络不可用、DNS 解析失败或服务器超时，请检查网络连接后重试。（{error}）"
            ))
        })?;

    read_text(response, "阿里云盘的接口").await
}

/// 拿 token 打一次 `/v2/user/get` 探活，顺带取回主盘 id 与昵称。
///
/// **这一步不能省**：批次 28 实测 `/token/get` 给的 41 字符 token 不能当
/// Bearer 用 ⇒ 探活失败就是「这个 token 没用」，必须当场报错，
/// 绝不能把一个坏凭据存进存档。
fn probe_user_profile(access_token: &str) -> Result<AliyunUserProfile, RemoteError> {
    let body = aliyun_user_info_request_body().to_string();
    let text = run_blocking(http_post_json_bearer(
        ALIYUN_USER_INFO_URL,
        access_token,
        &body,
    ))??;

    aliyun_parse_user_profile(&text)
}

/// 用扫码拿到的凭据拼一份会话。
fn session_from_credentials(
    credentials: &AliyunQrCredentials,
    profile: &AliyunUserProfile,
) -> AliyunAuthSession {
    let now = aliyun_now_unix();
    let expires_at_unix = credentials
        .expire_time
        .as_deref()
        .and_then(aliyun_expire_time_to_unix)
        .or_else(|| credentials.expires_in.map(|seconds| now + seconds));

    AliyunAuthSession {
        access_token: credentials.access_token.clone(),
        refresh_token: credentials.refresh_token.clone(),
        expires_at_unix,
        refresh_expires_at_unix: None,
        drive_id: profile
            .default_drive_id
            .clone()
            .or_else(|| credentials.default_drive_id.clone()),
        // 扫码拿到的 token 属于网页端 ⇒ 续期也只能用网页端公开 id。
        // 批次 34 前这里存的是空串，导致 access_token 过期后永远续不上。
        client_id: ALIYUN_WEB_CLIENT_ID.to_string(),
        redirect_uri: None,
        scope: None,
        token_type: "Bearer".to_string(),
        nick_name: profile.nick_name.clone().or_else(|| credentials.nick_name.clone()),
        device_id: None,
    }
}

/// 用「粘贴进来的 access_token」拼一份会话。
fn session_from_token(access_token: &str, profile: &AliyunUserProfile) -> AliyunAuthSession {
    AliyunAuthSession {
        access_token: access_token.to_string(),
        refresh_token: None,
        expires_at_unix: None,
        refresh_expires_at_unix: None,
        drive_id: profile.default_drive_id.clone(),
        client_id: String::new(),
        redirect_uri: None,
        scope: None,
        token_type: "Bearer".to_string(),
        nick_name: profile.nick_name.clone(),
        device_id: None,
    }
}

/// 取一张新的登录二维码。
#[tauri::command]
pub fn remote_aliyun_qr_start() -> Result<AliyunQrStartPayload, RemoteError> {
    // 新一轮扫码：上一轮待认领的会话作废。
    clear_pending();

    // 一次扫码会话只有一份客户端上下文；generate 与 query 共用它。
    let context = AliyunQrClientContext::random();
    let url = format!("{ALIYUN_QR_GENERATE_URL}?{}", aliyun_qr_generate_query(&context));
    let text = run_blocking(http_get_text(&url))??;
    let ticket = aliyun_parse_qr_ticket(&text)?;

    // 记下「这张码属于哪份上下文」，轮询时按 ck 取回来。
    store_qr_context(&ticket.ck, &context);

    // 诊断：只打长度，不打码内容。
    eprintln!(
        "[remote-diag] aliyun qr generate ck_len={} t_present={}",
        ticket.ck.len(),
        ticket.t.is_some()
    );

    Ok(AliyunQrStartPayload {
        code_content: ticket.code_content,
        ck: ticket.ck,
        t: ticket.t,
    })
}

/// 轮询扫码状态。
///
/// `CONFIRMED` 时立刻完成「解凭据 → 探活 → 存进待认领槽位」，
/// 前端只要看到 `confirmed` 就可以提示用户「可以保存这条连接了」。
#[tauri::command]
pub fn remote_aliyun_qr_poll(ck: String, t: Option<i64>) -> Result<AliyunQrPollPayload, RemoteError> {
    let ck = ck.trim().to_string();

    if ck.is_empty() {
        return Err(RemoteError::protocol(
            "缺少二维码票据，请重新点一次「生成二维码」。",
        ));
    }

    // 取回当初出码时用的那份上下文（按 ck 认）。
    let context = qr_context_for(&ck);
    let url = format!("{ALIYUN_QR_QUERY_URL}?{}", aliyun_qr_query_query());
    let body = aliyun_qr_query_form(&ck, t, &context, ALIYUN_USER_AGENT);
    let text = match run_blocking(http_post_form_text(&url, &body)) {
        Ok(Ok(text)) => text,
        Ok(Err(error)) => {
            eprintln!("[remote-diag] aliyun qr poll request failed: {error}");
            return Err(error);
        }
        Err(error) => {
            eprintln!("[remote-diag] aliyun qr poll runtime failed: {error}");
            return Err(error);
        }
    };
    let poll = aliyun_parse_qr_poll(&text)?;

    // 诊断：只打状态与长度 ——
    // `bizExt` 里带 accessToken ⇒ **绝不打响应正文**。
    eprintln!(
        "[remote-diag] aliyun qr poll status={} body_len={} has_biz_ext={}",
        poll.status.name(),
        text.len(),
        poll.biz_ext.is_some()
    );

    if poll.status != AliyunQrStatus::Confirmed {
        return Ok(AliyunQrPollPayload {
            status: poll.status.name().to_string(),
            message: None,
            final_status: poll.status.is_final(),
        });
    }

    let Some(biz_ext) = poll.biz_ext.as_deref() else {
        return Err(RemoteError::auth("扫码结果里没有凭据，请重新扫码。"));
    };

    let credentials = aliyun_decode_biz_ext(biz_ext)?;
    let profile = probe_user_profile(&credentials.access_token)?;
    let session = session_from_credentials(&credentials, &profile);
    store_pending(PendingSession { session });

    Ok(AliyunQrPollPayload {
        status: AliyunQrStatus::Confirmed.name().to_string(),
        message: Some("扫码成功，可以保存这条连接了。".to_string()),
        final_status: true,
    })
}

/// 「粘贴 access_token」兜底：探活成功后直接写进这条连接的存档。
#[tauri::command]
pub fn remote_aliyun_save_session(
    app: AppHandle,
    connection_id: String,
    access_token: String,
) -> Result<AliyunAuthStatusPayload, RemoteError> {
    let id = connection_id.trim().to_string();
    let token = access_token.trim().to_string();

    if token.is_empty() {
        return Err(RemoteError::protocol("请先粘贴阿里云盘的 access_token。"));
    }

    let profile = probe_user_profile(&token)?;
    let session = session_from_token(&token, &profile);

    // 新连接还没有 id ⇒ 存进「待认领槽位」，等连接落库后再由 claim 认领。
    if id.is_empty() {
        let payload = status_payload_from_session(Some(&session));
        store_pending(PendingSession { session });
        return Ok(payload);
    }

    let dir = app_data_dir(&app)?;
    save_aliyun_session(&dir, &id, &session)?;

    auth_status_payload(&dir, &id)
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

/// 用户主动取消登录（清掉待认领槽位）。
#[tauri::command]
pub fn remote_aliyun_login_cancel() -> Result<(), RemoteError> {
    clear_pending();
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

    auth_status_payload(&dir, &id)
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

/// 授权状态卡的内容（`auth_status` 与 `save_session` 共用）。
fn auth_status_payload(
    dir: &Path,
    id: &str,
) -> Result<AliyunAuthStatusPayload, RemoteError> {
    let session = load_aliyun_session(dir, id)?;

    Ok(status_payload_from_session(session.as_ref()))
}

/// 从会话算状态卡（读盘与「粘贴 token」两条路径共用同一套字段）。
fn status_payload_from_session(session: Option<&AliyunAuthSession>) -> AliyunAuthStatusPayload {
    let now = aliyun_now_unix();
    let status = crate::remote::aliyun_auth_status(session, now);
    let remaining_seconds =
        session.and_then(|session| crate::remote::aliyun_remaining_seconds(session, now));

    AliyunAuthStatusPayload {
        status: aliyun_status_name(&status).to_string(),
        remaining_seconds,
        summary: aliyun_describe_session(session),
        client_id: session
            .map(|session| session.client_id.clone())
            .filter(|value| !value.is_empty()),
        // 扫码登录**没有**客户端密钥 ⇒ 恒为 `false`。
        has_client_secret: false,
        scope: session.and_then(|session| session.scope.clone()),
        expires_at: session.and_then(|session| session.expires_at_unix),
        drive_id: session.and_then(|session| session.drive_id.clone()),
        nick_name: session.and_then(|session| session.nick_name.clone()),
    }
}

// ══════════════════════════════════════════════════════════════════════════
// 阶段 B：内嵌真实登录页（批次 29）
// ══════════════════════════════════════════════════════════════════════════
//
// ## 为什么要把登录换成内嵌 webview
//
// 上面那套「Rust 自己拼 `generate.do` / `query.do`」的路线，实测**卡在 `NEW`
// 永远不动**（证据：`.superpowers/sdd/b29-accept-run/`、`b29-key-probe*.json`）：
// 手机扫码后 App 提示登录成功，服务端的会话状态却始终没被推进。
// 根因是 `query.do` 需要**真实页面上下文**（`_csrf` / `umidToken` / `hsiz` /
// 一堆由页面自己种的 cookie），客户端凭空造不出来。
//
// ⇒ 改法和百度网盘 / 夸克对齐：**打开官方登录页，让页面自己驱动出码与轮询**，
// 我们只在页面里挂一个响应钩子，把它自己拿到的 `content.data.bizExt` 交回来。
// 凭据的获取路径完全没变（`bizExt` → `aliyun_decode_biz_ext` → 探活 → 落盘），
// 变的只是「`bizExt` 从谁来」：从自造请求变成真实页面。
//
// ## 数据流
//
// ```text
// remote_aliyun_login_start()
//   → 新建一个独立 data 目录（保证这一轮一定出二维码，不继承已登录态）
//   → std::thread::spawn 建窗（**不能**在事件循环线程 build，见模块顶部注释）
//   → initialization_script 注入 ALIYUN_LOGIN_HOOK_JS
//         页面自身 fetch/XHR 打到 qrcode/query.do
//         → 钩子读**响应正文**（不碰 DOM）取 qrCodeStatus / bizExt
//         → __TAURI_INTERNALS__.invoke('remote_aliyun_login_observed')
// remote_aliyun_login_observed()
//   → 状态去重后转发给前端（progress 可见）
//   → 拿到 bizExt：抢终态名额 ⇒ 另起线程 decode → probe → store_pending
//     ⇒ emit `ready` ⇒ 关窗
// remote_aliyun_login_claim(id)   ← 复用上面既有的认领命令，语义不变
// remote_aliyun_login_close()     ← 用户取消：抢名额 + emit `cancelled` + 关窗
// ```
//
// ## 事件频道
//
// `afm://aliyun-login`，`kind` ∈ `opening` / `page_status` / `ready` /
// `error` / `cancelled` / `closed` / `give_up`。与
// `baidu_login.rs` 的 `afm://baidu-login` 同一套口径（自定义协议名，
// 不用 `tauri://` 保留前缀）。payload 里**只有脱敏摘要**，没有 token。
//
// ## 凭据纪律
//
// `bizExt` / `accessToken` 明文不进日志、不进事件 payload：
// 日志只打长度 + `short_sha16`，payload 只带 `describe()` 级别的摘要。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use serde_json::{json, Value};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::remote::short_sha16;

/// 登录子窗的 label。崩溃兜底只跟踪**发过心跳**的窗口，这个窗口不发心跳，
/// 因此不会被看门狗判定成 hung 去 `reload()`（`webview_guard_runtime.rs` 的
/// 判定循环遍历的是 `heartbeats`，不是 `webview_windows()`）。
pub const ALIYUN_LOGIN_WINDOW_LABEL: &str = "afm-aliyun-login";

/// 前端订阅的登录事件频道。
pub const ALIYUN_LOGIN_EVENT: &str = "afm://aliyun-login";

/// 官方登录页。⚠️ 参数**一个都不能少**：裸地址会被拒成「非法请求 [appNameError]」
/// （实测见 `b29-embedded-webview-plan.md` §2）。
const ALIYUN_LOGIN_URL: &str = "https://passport.alipan.com/mini_login.htm?appName=aliyun_drive&fromSite=52&appEntrance=web_default&bizParams=taobaoBizLoginFrom%3Dweb_default&mainPage=false&isMobile=false&lang=zh_CN&returnUrl=&umidTag=SERVER";

/// 每轮登录一个全新的 webview 数据目录（与 `baidu_login.rs` 同一口径）。
///
/// 不复用主目录：否则机器上已登录过阿里云盘时，登录页直接判定「已登录」、
/// 根本不出二维码，用户扫不到东西。
const ALIYUN_LOGIN_PROFILE_ROOT: &str = "aliyun-login-profile";

/// 诊断日志默认落点（`%APPDATA%/<identifier>/` 下）。
const ALIYUN_LOGIN_DEFAULT_LOG_NAME: &str = "afm-aliyun-login.log";

/// 环境变量：覆盖诊断日志路径，真机验收时用。
const ALIYUN_LOGIN_LOG_ENV: &str = "AFM_ALIYUN_LOGIN_LOG";

/// 硬上限：15 分钟没结果就放弃（跟手扫一次码 + 输密码的合理用时）。
const ALIYUN_LOGIN_GIVE_UP_MS: u128 = 900 * 1000;

/// 这一轮登录有没有被**终结**（成功 / 失败 / 取消 / 超时 / 用户关窗）。
///
/// 谁把它从 `false` 翻成 `true` 谁就拿到「唯一终态」名额 ——
/// 注入钩子有两条投递路径（iframe 直投 + 主帧转发），必然重复，靠它去重。
static ALIYUN_LOGIN_CLAIM: AtomicBool = AtomicBool::new(false);

/// 看门狗代号：开新一轮就 +1，旧看门狗发现自己过期即退出。
static ALIYUN_LOGIN_WATCHDOG_GEN: AtomicU64 = AtomicU64::new(0);

/// 上一次转发给前端的页面状态，用来去重（避免同一状态每秒刷一条事件）。
fn aliyun_last_status_slot() -> &'static Mutex<Option<String>> {
    static LAST: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    LAST.get_or_init(|| Mutex::new(None))
}

/// 墙钟毫秒。本模块不引 `chrono`，直接用 `SystemTime`。
fn aliyun_epoch_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default()
}

/// 诊断日志路径：`AFM_ALIYUN_LOGIN_LOG` 优先，否则应用数据目录下的默认文件。
fn aliyun_login_log_path(app: &AppHandle) -> PathBuf {
    if let Ok(from_env) = std::env::var(ALIYUN_LOGIN_LOG_ENV) {
        let trimmed = from_env.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    match app_data_dir(app) {
        Ok(dir) => dir.join(ALIYUN_LOGIN_DEFAULT_LOG_NAME),
        Err(_) => PathBuf::from(ALIYUN_LOGIN_DEFAULT_LOG_NAME),
    }
}

/// 追一行诊断日志。日志是**可选能力**：写失败绝不影响登录流程。
fn aliyun_login_log(app: &AppHandle, message: &str) {
    use std::io::Write;

    let path = aliyun_login_log_path(app);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut file) => {
            let _ = writeln!(file, "[{}] {message}", aliyun_epoch_ms());
        }
        Err(error) => {
            eprintln!("[remote-diag] aliyun login log open failed: {error}");
        }
    }
}

/// 抢「本轮登录的唯一终态」名额。`true` = 归我收尾。
fn claim_aliyun_login() -> bool {
    !ALIYUN_LOGIN_CLAIM.swap(true, Ordering::SeqCst)
}

/// 往登录事件频道发一条消息。
fn emit_aliyun_login(app: &AppHandle, kind: &str, mut payload: Value) {
    if let Value::Object(map) = &mut payload {
        map.insert("kind".to_string(), json!(kind));
        map.insert("at".to_string(), json!(aliyun_epoch_ms()));
    }

    aliyun_login_log(app, &format!("emit kind={kind}"));

    if let Err(error) = app.emit(ALIYUN_LOGIN_EVENT, &payload) {
        aliyun_login_log(app, &format!("emit kind={kind} failed: {error}"));
    }
}

/// 关掉登录子窗。
///
/// 延后 200ms 再关：`remote_aliyun_login_observed` 是**这个 webview 自己**发来的
/// 调用，在它的命令处理里同步 `close()` 会把正在返回的 IPC 掐掉。
fn close_aliyun_login_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(ALIYUN_LOGIN_WINDOW_LABEL) else {
        return;
    };

    let handle = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        let _ = handle.close();
    });
}

/// 收尾本轮登录：发终态事件 + 关窗。调用方必须已经 `claim_aliyun_login()`。
fn finalize_aliyun_login(app: &AppHandle, kind: &str, payload: Value) {
    emit_aliyun_login(app, kind, payload);
    close_aliyun_login_window(app);
}

/// 页面 `qrCodeStatus` → 前端 `AliyunQrStatusKind`。
///
/// ⚠️ 淘系把「已扫码」拼成 `SCANED`（少一个 N），实测见
/// `b29-embedded-webview-plan.md` §2 ⇒ 两种拼法都收。
fn aliyun_normalize_page_status(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => String::new(),
        "new" | "init" => "new".to_string(),
        "scaned" | "scanned" => "scanned".to_string(),
        "confirmed" | "success" => "confirmed".to_string(),
        "expired" => "expired".to_string(),
        "canceled" | "cancelled" => "canceled".to_string(),
        other => other.to_string(),
    }
}

/// 回收过期的登录数据目录（`session-<建罐时刻毫秒>`）。
///
/// 正在用的窗口（`CLAIM == false`）留 15 分钟余量，其余留 30 分钟，
/// 免得误删另一个窗格正在扫的那一轮。
fn sweep_aliyun_login_profiles(app: &AppHandle, now_ms: u128) {
    let root = match app_data_dir(app) {
        Ok(dir) => dir.join(ALIYUN_LOGIN_PROFILE_ROOT),
        Err(_) => return,
    };
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };

    let keep_ms = if ALIYUN_LOGIN_CLAIM.load(Ordering::SeqCst) {
        30 * 60 * 1000
    } else {
        15 * 60 * 1000
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(created_ms) = name
            .strip_prefix("session-")
            .and_then(|suffix| suffix.parse::<u128>().ok())
        else {
            continue;
        };
        if now_ms.saturating_sub(created_ms) <= keep_ms {
            continue;
        }

        let target = entry.path();
        let removed = std::fs::remove_dir(&target).is_ok()
            || std::fs::remove_dir_all(&target).is_ok();
        aliyun_login_log(
            app,
            &format!(
                "sweep {} removed={removed}",
                target.display()
            ),
        );
    }
}

/// 给这一轮登录建一个**空的**数据目录；失败就退回共享 cookie jar。
fn prepare_aliyun_login_profile(app: &AppHandle) -> Option<PathBuf> {
    let now_ms = aliyun_epoch_ms();
    sweep_aliyun_login_profiles(app, now_ms);

    let root = match app_data_dir(app) {
        Ok(dir) => dir.join(ALIYUN_LOGIN_PROFILE_ROOT),
        Err(_) => return None,
    };
    if std::fs::create_dir_all(&root).is_err() {
        return None;
    }

    let profile = root.join(format!("session-{now_ms}"));
    if std::fs::create_dir_all(&profile).is_ok() {
        Some(profile)
    } else {
        None
    }
}

/// 注入到登录页的钩子脚本。
///
/// 要点：
/// 1. 只挂 **`query.do` 的响应正文**，不碰 DOM、不改页面行为；
/// 2. `initialization_script` 在 WebView2 里会进**所有帧**，而登录二维码实际在
///    `passport.alipan.com` 的 iframe 里 ⇒ 帧内优先直投 IPC；
/// 3. IPC 不可用时退回 `window.parent.postMessage`，由主帧的钩子转发
///    （跨帧 `postMessage` 不受同源限制，`chrome.webview` 在跨源 iframe
///    里是否可见本机未坐实，所以两条路都留）；
/// 4. Rust 侧按 `CLAIM` 去重 ⇒ 双投递无害；
/// 5. 全程 `try/catch` 静默 —— 钩子坏了也不能让登录页白屏。
const ALIYUN_LOGIN_HOOK_JS: &str = r#"
(function () {
  "use strict";
  if (window.__afmAliyunLoginHook) return;
  window.__afmAliyunLoginHook = true;

  var MARK = "__afmAliyunLogin";

  function sendToNative(status, bizExt) {
    var ok = false;
    try {
      var internals = window.__TAURI_INTERNALS__;
      if (internals && typeof internals.invoke === "function") {
        internals.invoke("remote_aliyun_login_observed", {
          status: status || "",
          bizExt: bizExt || ""
        });
        ok = true;
      }
    } catch (error) {}
    return ok;
  }

  function relayToParent(status, bizExt) {
    try {
      if (window.parent && window.parent !== window) {
        window.parent.postMessage(
          { __afmAliyunLogin: 1, status: status || "", bizExt: bizExt || "" },
          "*"
        );
      }
    } catch (error) {}
  }

  // IPC 可能比钩子晚就绪：先排队，另开定时器冲刷。
  var queue = [];
  function flush() {
    if (!window.__TAURI_INTERNALS__) return;
    while (queue.length) {
      var item = queue.shift();
      if (!sendToNative(item[0], item[1])) {
        queue.unshift(item);
        break;
      }
    }
  }
  var timer = setInterval(function () {
    flush();
    if (window.__TAURI_INTERNALS__ && queue.length === 0) clearInterval(timer);
  }, 200);

  function report(status, bizExt) {
    if (!status && !bizExt) return;
    queue.push([status || "", bizExt || ""]);
    flush();
    relayToParent(status || "", bizExt || "");
  }

  function sliceJson(text) {
    if (typeof text !== "string" || !text.length) return "";
    var start = text.indexOf("{");
    var end = text.lastIndexOf("}");
    return start >= 0 && end > start ? text.slice(start, end + 1) : "";
  }

  function handleBody(text) {
    try {
      var jsonText = sliceJson(text);
      if (!jsonText) return;
      var obj = JSON.parse(jsonText);
      var content = obj && obj.content;
      var data = content && content.data;
      if (!data) return;
      var status = typeof data.qrCodeStatus === "string" ? data.qrCodeStatus : "";
      var bizExt = "";
      if (data.bizExt) {
        bizExt =
          typeof data.bizExt === "string"
            ? data.bizExt
            : JSON.stringify(data.bizExt);
      }
      report(status, bizExt);
    } catch (error) {}
  }

  function isQueryUrl(url) {
    return typeof url === "string" && url.indexOf("qrcode/query.do") >= 0;
  }

  // ---- fetch ----
  var nativeFetch = window.fetch;
  if (typeof nativeFetch === "function") {
    window.fetch = function (input) {
      var url = "";
      try {
        url = typeof input === "string" ? input : input && input.url ? input.url : "";
      } catch (error) {}
      var result = nativeFetch.apply(this, arguments);
      if (!isQueryUrl(url)) return result;
      try {
        return result.then(function (response) {
          try {
            response.clone().text().then(handleBody, function () {});
          } catch (error) {}
          return response;
        });
      } catch (error) {
        return result;
      }
    };
  }

  // ---- XMLHttpRequest ----
  try {
    var nativeOpen = XMLHttpRequest.prototype.open;
    var nativeSend = XMLHttpRequest.prototype.send;

    XMLHttpRequest.prototype.open = function (method, url) {
      try {
        this.__afmUrl = typeof url === "string" ? url : "";
      } catch (error) {}
      return nativeOpen.apply(this, arguments);
    };

    XMLHttpRequest.prototype.send = function () {
      var xhr = this;
      if (isQueryUrl(xhr.__afmUrl)) {
        try {
          xhr.addEventListener("readystatechange", function () {
            if (xhr.readyState !== 4) return;
            try {
              var type = xhr.responseType;
              if (type === "" || type === "text") handleBody(xhr.responseText);
            } catch (error) {}
          });
        } catch (error) {}
      }
      return nativeSend.apply(this, arguments);
    };
  } catch (error) {}

  // ---- 主帧收 iframe 的转投递（只有主帧这条路会真的落地）----
  try {
    window.addEventListener("message", function (event) {
      var data = event && event.data;
      if (!data || !data.__afmAliyunLogin) return;
      sendToNative(data.status, data.bizExt) ||
        queue.push([data.status || "", data.bizExt || ""]);
    });
  } catch (error) {}
})();
"#;

/// 看门狗：`ALIYUN_LOGIN_GIVE_UP_MS` 内没有终态就放弃。
///
/// 用独立线程 + `sleep` 分片，**不碰** `tauri::async_runtime`（模块顶部红线）。
fn start_aliyun_login_watchdog(app: &AppHandle) {
    let generation = ALIYUN_LOGIN_WATCHDOG_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let handle = app.clone();

    std::thread::spawn(move || {
        let started = aliyun_epoch_ms();
        let step = Duration::from_millis(1000);

        while aliyun_epoch_ms().saturating_sub(started) < ALIYUN_LOGIN_GIVE_UP_MS {
            if ALIYUN_LOGIN_CLAIM.load(Ordering::SeqCst) {
                return;
            }
            if ALIYUN_LOGIN_WATCHDOG_GEN.load(Ordering::SeqCst) != generation {
                return;
            }
            std::thread::sleep(step);
        }

        if !claim_aliyun_login() {
            return;
        }
        if ALIYUN_LOGIN_WATCHDOG_GEN.load(Ordering::SeqCst) != generation {
            return;
        }

        aliyun_login_log(&handle, "watchdog: gave up after 900s");
        finalize_aliyun_login(
            &handle,
            "give_up",
            json!({ "message": "登录超时了，请重新打开登录窗口再试一次。" }),
        );
    });
}

/// 建登录子窗。
///
/// ⚠️ 必须 `std::thread::spawn`：在事件循环线程上直接 `build()` 会死锁
/// （白板 + `cookies_for_url` 永挂），`baidu_login.rs` 顶部已写明这条实测结论。
fn spawn_aliyun_login_builder(app: AppHandle) {
    std::thread::spawn(move || {
        if ALIYUN_LOGIN_CLAIM.load(Ordering::SeqCst) {
            aliyun_login_log(&app, "builder skipped: login already settled");
            return;
        }

        let url = match tauri::Url::parse(ALIYUN_LOGIN_URL) {
            Ok(url) => url,
            Err(error) => {
                aliyun_login_log(&app, &format!("bad login url: {error}"));
                if claim_aliyun_login() {
                    finalize_aliyun_login(
                        &app,
                        "error",
                        json!({ "message": format!("登录地址无法解析：{error}") }),
                    );
                }
                return;
            }
        };

        let mut builder = WebviewWindowBuilder::new(
            &app,
            ALIYUN_LOGIN_WINDOW_LABEL,
            WebviewUrl::External(url),
        )
        .title("阿里云盘 登录")
        .inner_size(1100.0, 820.0)
        .initialization_script(ALIYUN_LOGIN_HOOK_JS);

        match prepare_aliyun_login_profile(&app) {
            Some(profile) => {
                aliyun_login_log(
                    &app,
                    &format!("login window data_directory={}", profile.display()),
                );
                builder = builder.data_directory(profile);
            }
            None => {
                // 建空罐失败 ⇒ 退回共享 jar。真机上这会导致「已登录不出码」，
                // 所以必须留一行日志，别静默降级。
                aliyun_login_log(
                    &app,
                    "empty login profile unavailable; falling back to the shared cookie jar",
                );
            }
        }

        let window = match builder.build() {
            Ok(window) => window,
            Err(error) => {
                aliyun_login_log(&app, &format!("build login window failed: {error}"));
                if claim_aliyun_login() {
                    finalize_aliyun_login(
                        &app,
                        "error",
                        json!({ "message": format!("打不开登录窗口：{error}") }),
                    );
                }
                return;
            }
        };

        aliyun_login_log(&app, "login window built");

        let watch = app.clone();
        window.on_window_event(move |event| {
            if !matches!(event, tauri::WindowEvent::Destroyed) {
                return;
            }
            // 我们主动关窗时已经抢过名额，这里不会重复发。
            if !claim_aliyun_login() {
                return;
            }
            aliyun_login_log(&watch, "login window destroyed before login completed");
            emit_aliyun_login(&watch, "closed", json!({}));
        });
    });
}

/// 打开阿里云盘官方登录页（与百度网盘 / 夸克一致的内嵌浏览器登录）。
#[tauri::command]
pub fn remote_aliyun_login_start(app: AppHandle) -> Result<Value, RemoteError> {
    aliyun_login_log(&app, "login_start called");

    // 新一轮：清掉上一轮的终态标记与待认领会话。
    ALIYUN_LOGIN_CLAIM.store(false, Ordering::SeqCst);
    ALIYUN_LOGIN_WATCHDOG_GEN.fetch_add(1, Ordering::SeqCst);
    *aliyun_last_status_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    clear_pending();

    if let Some(window) = app.get_webview_window(ALIYUN_LOGIN_WINDOW_LABEL) {
        aliyun_login_log(&app, "login_start: window already open, focusing");
        emit_aliyun_login(&app, "opening", json!({ "reused": true }));
        let _ = window.set_focus();
        return Ok(json!({ "started": true, "reused": true }));
    }

    emit_aliyun_login(&app, "opening", json!({ "reused": false }));
    start_aliyun_login_watchdog(&app);
    spawn_aliyun_login_builder(app.clone());

    Ok(json!({ "started": true, "reused": false }))
}

/// 注入钩子把登录页自己的 `query.do` 结果交回这里。
///
/// 凭据只在这一层与 `remote::aliyun_auth` 之间流转 ⇒ 返回值与事件里
/// **没有任何 token**。
#[tauri::command]
pub fn remote_aliyun_login_observed(
    app: AppHandle,
    status: Option<String>,
    biz_ext: Option<String>,
) -> Result<Value, RemoteError> {
    let raw_status = status.unwrap_or_default();
    let page_status = aliyun_normalize_page_status(&raw_status);
    let biz_ext = biz_ext.unwrap_or_default();

    // 诊断：只打长度与指纹前 16 位，绝不打 `bizExt` 正文。
    aliyun_login_log(
        &app,
        &format!(
            "observed status={} biz_ext_len={} biz_ext_sha16={}",
            if page_status.is_empty() { "-" } else { &page_status },
            biz_ext.len(),
            if biz_ext.is_empty() {
                "-".to_string()
            } else {
                short_sha16(&biz_ext)
            }
        ),
    );

    if ALIYUN_LOGIN_CLAIM.load(Ordering::SeqCst) {
        return Ok(json!({ "ignored": "settled" }));
    }

    // 状态转发给前端（去重），让用户看得见「已扫码 / 已过期」。
    if !page_status.is_empty() {
        let mut last = aliyun_last_status_slot()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if last.as_deref() != Some(page_status.as_str()) {
            *last = Some(page_status.clone());
            drop(last);
            emit_aliyun_login(&app, "page_status", json!({ "status": page_status }));
        }
    }

    let has_ticket = !biz_ext.trim().is_empty();
    if !has_ticket && page_status != "confirmed" {
        return Ok(json!({ "ok": true }));
    }

    if !claim_aliyun_login() {
        return Ok(json!({ "ignored": "claimed" }));
    }

    if !has_ticket {
        // 页面报了 CONFIRMED 却没带凭据 ⇒ 契约变了，必须当场说清，
        // 绝不能让用户对着一个「看起来成功了」的窗口等下去。
        aliyun_login_log(&app, "page reported confirmed without bizExt");
        finalize_aliyun_login(
            &app,
            "error",
            json!({ "message": "登录页回报已确认，但没有交出凭据，请重新登录一次。" }),
        );
        return Err(RemoteError::auth(
            "阿里云盘登录页没有返回凭据，请重新登录。",
        ));
    }

    // 解凭据要联网探活（最长 30s），而命令处理跑在主线程上 ⇒ 另起线程做，
    // 结果一律走事件回前端。
    let worker = app.clone();
    let payload = biz_ext.trim().to_string();
    std::thread::spawn(move || match complete_aliyun_login(&worker, &payload) {
        Ok(summary) => {
            aliyun_login_log(&worker, "login completed: credential stored");
            finalize_aliyun_login(&worker, "ready", json!({ "summary": summary }));
        }
        Err(error) => {
            let message = error.to_string();
            aliyun_login_log(&worker, &format!("login failed: {message}"));
            // 名额已经在我们手上，直接收尾（不再 claim）。
            emit_aliyun_login(&worker, "error", json!({ "message": message }));
            close_aliyun_login_window(&worker);
        }
    });

    Ok(json!({ "accepted": true }))
}

/// 把页面交回的 `bizExt` 走完「解凭据 → 探活 → 存待认领槽位」。
///
/// 这四行与 `remote_aliyun_qr_poll` 的 `CONFIRMED` 分支**逐字一致**：
/// 换的只是凭据来源，落盘路径一个字都没改。
fn complete_aliyun_login(app: &AppHandle, biz_ext: &str) -> Result<String, RemoteError> {
    let _ = app;

    let credentials = aliyun_decode_biz_ext(biz_ext)?;
    let profile = probe_user_profile(&credentials.access_token)?;
    let session = session_from_credentials(&credentials, &profile);
    let summary = session.describe();
    store_pending(PendingSession { session });

    Ok(summary)
}

/// 用户主动关掉登录窗 / 点「取消登录」。
///
/// 与 `remote_aliyun_login_cancel`（只清槽位，走 QR 那条路）分开：
/// 这个还要抢终态名额并真正关窗。
#[tauri::command]
pub fn remote_aliyun_login_close(app: AppHandle) -> Result<Value, RemoteError> {
    aliyun_login_log(&app, "login_close called");
    clear_pending();

    if claim_aliyun_login() {
        finalize_aliyun_login(&app, "cancelled", json!({}));
    } else {
        close_aliyun_login_window(&app);
    }

    Ok(json!({ "closed": true }))
}
