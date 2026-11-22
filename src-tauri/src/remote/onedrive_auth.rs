// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! OneDrive（Microsoft Graph）会话层：结构、编解码、PKCE 与令牌响应解析。
//!
//! 本文件被 `cargo test --lib` 的测试二进制链接，**不得出现 `tauri::` 的任何路径** ——
//! 理由见 `src-tauri/src/remote/mod.rs` 顶部。
//!
//! ## 权限边界
//!
//! `access_token` / `refresh_token` **明文只在本模块、`remote::onedrive_provider`
//! 与 `onedrive_oauth` 之间流转**，不出现在命令返回值、事件 payload 与日志里。
//! 落盘委托 [`crate::remote::credentials`]（DPAPI）。
//!
//! ## 三条不能改的契约
//!
//! 1. 版本固定为 [`ONEDRIVE_SESSION_VERSION`]（`1`）。读到别的版本按「请重新登录」
//!    处理，**不要**猜着读。
//! 2. 字段名逐字固定：`version` / `client_id` / `access_token` / `refresh_token` /
//!    `expires_at` / `account` / `drive_id`。改名会让已经存好的会话读不出来。
//! 3. **每次刷新都必须把新的 `refresh_token` 覆盖旧的。** 微软侧的 refresh token
//!    有 90 天滚动窗口，不覆盖会让用户在第 91 天突然被迫重新登录，而且没有任何
//!    本地迹象可查。

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::remote::credentials;
use crate::remote::error::RemoteError;

/// 会话存档格式版本。
pub const ONEDRIVE_SESSION_VERSION: u32 = 1;

/// 离过期还有多久就该提示用户。access token 只有约 1 小时，5 分钟够用户反应。
pub const ONEDRIVE_EXPIRY_WARN_SECONDS: i64 = 300;

/// 微软身份平台（v2.0 授权码 + PKCE）。租户固定 `common`：
/// 个人账号与工作/学校账号都能登，企业租户要限定范围时再改这里。
pub const ONEDRIVE_TENANT: &str = "common";

/// 授权端点。
pub const ONEDRIVE_AUTHORIZE_URL: &str =
    "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";

/// 令牌端点（授权码兑换与刷新共用）。
pub const ONEDRIVE_TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";

/// 申请的权限。
///
/// **必须显式带 `offline_access`** —— 少了它微软不会发 refresh token，
/// 用户每小时都要重新登录一次，而报错信息只会说「没有 refresh_token」。
/// `Files.Read` 是只读批次的全部权限，不申请任何写权限。
pub const ONEDRIVE_SCOPE: &str = "Files.Read offline_access";

/// PKCE `code_verifier` 的长度下限（RFC 7636 §4.1）。
pub const ONEDRIVE_CODE_VERIFIER_MIN_LEN: usize = 43;

/// PKCE `code_verifier` 的长度上限（RFC 7636 §4.1）。
pub const ONEDRIVE_CODE_VERIFIER_MAX_LEN: usize = 128;

/// 一条已登录的 OneDrive 会话。
///
/// **这是密文解密之后的内容**，由 DPAPI 保护；它自己**不是**明文。
/// 字段全为必需（除两个可选元信息）⇒ 一份百度存档（`bduss`）或夸克存档（`pus`）
/// 解不开，会被 [`is_onedrive_session_blob`] 判成「不是 OneDrive 会话」。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnedriveAuthSession {
    /// 固定 [`ONEDRIVE_SESSION_VERSION`]。
    pub version: u32,
    /// 申请授权的应用（客户端）ID。刷新令牌时要原样带回去。
    ///
    /// **`client_id` 只存在这份会话里，不进 `RemoteConnection`** ——
    /// 它是登录态的一部分，不是连接配置。
    pub client_id: String,
    /// Bearer token，约 1 小时有效。
    pub access_token: String,
    /// 刷新令牌，90 天滚动有效。可能为空（极少见：租户策略不给）。
    pub refresh_token: String,
    /// access token 的绝对过期时间（Unix 秒）。
    pub expires_at: i64,
    /// 账号显示名/邮箱。拿不到就是 `None`，**不影响**能否列目录。
    #[serde(default)]
    pub account: Option<String>,
    /// Graph 的 drive id。`None` 表示走 `/me/drive`。
    #[serde(default)]
    pub drive_id: Option<String>,
}

impl OnedriveAuthSession {
    pub fn has_refresh_token(&self) -> bool {
        !self.refresh_token.trim().is_empty()
    }

    pub fn has_access_token(&self) -> bool {
        !self.access_token.trim().is_empty()
    }

    /// 能不能拿它去发请求：有 access token，且（还没过期 或 还能刷新）。
    pub fn is_usable(&self, now_unix: i64) -> bool {
        self.has_access_token() && (!onedrive_is_expired(self.expires_at, now_unix) || self.has_refresh_token())
    }

    /// 日志/报错用的描述。**只打长度与布尔，不打 token 本身。**
    pub fn describe(&self) -> String {
        format!(
            "OneDrive 会话（access_token {} 字节，refresh_token {}，过期时间 {}）",
            self.access_token.len(),
            if self.has_refresh_token() {
                "有"
            } else {
                "无"
            },
            self.expires_at
        )
    }
}

/// 手写 `Debug`：默认派生会把 access token 原样打进任何一条 `{:?}` 日志。
impl fmt::Debug for OnedriveAuthSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OnedriveAuthSession")
            .field("version", &self.version)
            .field("client_id_len", &self.client_id.len())
            .field("access_token_len", &self.access_token.len())
            .field("refresh_token_len", &self.refresh_token.len())
            .field("expires_at", &self.expires_at)
            .field("account", &self.account)
            .field("drive_id", &self.drive_id)
            .finish()
    }
}

/// 登录态是否可用。前端只据此显示「已登录 / 未登录 / 需要重新登录」。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnedriveAuthStatusPayload {
    pub authorized: bool,
    /// access token 的绝对过期时间（Unix 秒）。
    pub expires_at: Option<i64>,
    pub account: Option<String>,
}

pub fn onedrive_now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

/// `expires_at == 0` 表示「不知道过期时间」，**不当成已过期** ——
/// 把它当已过期会让每一次调用都先去刷新，而刷新本身也可能失败。
pub fn onedrive_is_expired(expires_at: i64, now_unix: i64) -> bool {
    expires_at > 0 && now_unix >= expires_at
}

pub fn onedrive_is_expiring_soon(expires_at: i64, now_unix: i64) -> bool {
    expires_at > 0 && now_unix + ONEDRIVE_EXPIRY_WARN_SECONDS >= expires_at
}

/// 会话 → 前端状态。`None` 会话（没登录）也走这一条，避免两处各写一遍。
pub fn onedrive_auth_status_payload(
    session: Option<&OnedriveAuthSession>,
    now_unix: i64,
) -> OnedriveAuthStatusPayload {
    match session {
        Some(session) if session.is_usable(now_unix) => OnedriveAuthStatusPayload {
            authorized: true,
            expires_at: Some(session.expires_at),
            account: session.account.clone(),
        },
        Some(session) => OnedriveAuthStatusPayload {
            authorized: false,
            expires_at: Some(session.expires_at),
            account: session.account.clone(),
        },
        None => OnedriveAuthStatusPayload {
            authorized: false,
            expires_at: None,
            account: None,
        },
    }
}

// ---------------------------------------------------------------------------
// 随机值与 PKCE
// ---------------------------------------------------------------------------

/// 十六进制随机串。本仓没有 `rand` 依赖，用 uuid v4 的 122 位熵拼出来
/// （同 `aliyun_auth.rs` 的既有做法），够 `state` 与 `code_verifier` 用。
fn random_hex(len: usize) -> String {
    let mut out = String::with_capacity(len + 32);
    while out.len() < len {
        out.push_str(&uuid::Uuid::new_v4().simple().to_string());
    }
    out.truncate(len);
    out
}

/// CSRF 防护用的 `state`。
pub fn onedrive_generate_state() -> String {
    random_hex(32)
}

/// PKCE `code_verifier`：64 个十六进制字符，落在 RFC 7636 的 43..=128 区间内，
/// 且只含 `[A-Za-z0-9]` —— 不需要再百分号编码。
pub fn onedrive_generate_code_verifier() -> String {
    random_hex(64)
}

/// `code_challenge = base64url_no_pad(sha256(verifier))`，43 个字符，无 `=` 填充。
pub fn pkce_code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

/// 拼授权页地址。参数顺序稳定，测试可直接断言。
pub fn build_authorize_url(
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
) -> String {
    format!(
        "{ONEDRIVE_AUTHORIZE_URL}?client_id={}&response_type=code&redirect_uri={}\
         &response_mode=query&scope={}&state={}&code_challenge={}\
         &code_challenge_method=S256",
        urlencoding::encode(client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(ONEDRIVE_SCOPE),
        urlencoding::encode(state),
        urlencoding::encode(code_challenge),
    )
}

/// 表单体。`application/x-www-form-urlencoded` —— 空格编成 `%20`，
/// 微软两种写法都收。
pub fn onedrive_form_body(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| {
            format!(
                "{}={}",
                urlencoding::encode(key),
                urlencoding::encode(value)
            )
        })
        .collect::<Vec<_>>()
        .join("&")
}

// ---------------------------------------------------------------------------
// 令牌响应
// ---------------------------------------------------------------------------

/// 令牌端点的响应。成功与失败共用一套字段（微软的失败响应也是 JSON）。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OnedriveTokenResponse {
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// 秒。缺省按 3600（微软的默认值）算，宁可早一点刷新。
    #[serde(default)]
    pub expires_in: Option<i64>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
}

/// 解析令牌响应。
///
/// `previous_refresh_token` 只在**刷新**时给：微软有时只回新的 access token，
/// 这时必须沿用旧 refresh token，否则下一次刷新直接失去授权。
pub fn onedrive_parse_token_response(
    client_id: &str,
    body: &str,
    now_unix: i64,
    previous_refresh_token: Option<&str>,
) -> Result<OnedriveAuthSession, RemoteError> {
    let parsed: OnedriveTokenResponse = serde_json::from_str(body)
        .map_err(|error| RemoteError::auth(format!("OneDrive 登录响应不是合法 JSON：{error}")))?;

    if let Some(code) = parsed
        .error
        .as_deref()
        .map(str::trim)
        .filter(|code| !code.is_empty())
    {
        // 只带 code，**不带 description** —— 后者可能回显请求里的 token。
        return Err(RemoteError::auth(format!(
            "OneDrive 授权失败（{code}），请重新登录。"
        )));
    }

    let access_token = parsed
        .access_token
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            RemoteError::auth("OneDrive 登录响应里没有 access_token，请重新登录。")
        })?;

    let refresh_token = parsed
        .refresh_token
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
        .or_else(|| {
            previous_refresh_token
                .map(str::trim)
                .filter(|token| !token.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_default();

    let expires_in = parsed
        .expires_in
        .filter(|seconds| *seconds > 0)
        .unwrap_or(3_600);

    Ok(OnedriveAuthSession {
        version: ONEDRIVE_SESSION_VERSION,
        client_id: client_id.to_string(),
        access_token,
        refresh_token,
        expires_at: now_unix.saturating_add(expires_in),
        account: None,
        drive_id: None,
    })
}

/// 刷新后把旧会话里的元信息补回新会话。`None` 才补，已有值不覆盖。
pub fn onedrive_apply_refresh(
    previous: &OnedriveAuthSession,
    refreshed: &mut OnedriveAuthSession,
) {
    if refreshed.client_id.trim().is_empty() {
        refreshed.client_id = previous.client_id.clone();
    }
    if refreshed.account.is_none() {
        refreshed.account = previous.account.clone();
    }
    if refreshed.drive_id.is_none() {
        refreshed.drive_id = previous.drive_id.clone();
    }
}

// ---------------------------------------------------------------------------
// 落盘
// ---------------------------------------------------------------------------

/// 序列化成 `remote-credentials.json` 里的那段文本。
///
/// 与会话结构本身同形：`version` 是结构体字段，不再套一层外壳。
pub fn onedrive_encode_session(session: &OnedriveAuthSession) -> Result<String, RemoteError> {
    serde_json::to_string(session)
        .map_err(|error| RemoteError::unknown(format!("序列化 OneDrive 会话失败：{error}")))
}

/// 反序列化。版本不符 / 解析失败 / 没有 access token 一律报 `Auth`
/// （不静默当成「没存」—— 否则用户会看到「还没登录」而查不出真正原因）。
pub fn onedrive_decode_session(raw: &str) -> Result<OnedriveAuthSession, RemoteError> {
    let session: OnedriveAuthSession = serde_json::from_str(raw).map_err(|error| {
        RemoteError::auth(format!("已保存的 OneDrive 会话读不出来，请重新登录：{error}"))
    })?;

    if session.version != ONEDRIVE_SESSION_VERSION {
        return Err(RemoteError::auth(format!(
            "已保存的 OneDrive 会话格式版本是 {}，本版本只认 {ONEDRIVE_SESSION_VERSION}，\
             请重新登录 OneDrive。",
            session.version
        )));
    }

    if !session.has_access_token() {
        return Err(RemoteError::auth(
            "已保存的 OneDrive 会话里没有访问凭证，请重新登录 OneDrive。",
        ));
    }

    Ok(session)
}

pub fn save_onedrive_session(
    app_data_dir: &Path,
    key: &str,
    session: &OnedriveAuthSession,
) -> Result<(), RemoteError> {
    let encoded = onedrive_encode_session(session)?;
    credentials::save_credential(app_data_dir, key, &encoded)
}

pub fn load_onedrive_session(
    app_data_dir: &Path,
    key: &str,
) -> Result<Option<OnedriveAuthSession>, RemoteError> {
    let Some(raw) = credentials::load_credential(app_data_dir, key)? else {
        return Ok(None);
    };

    onedrive_decode_session(&raw).map(Some)
}

pub fn delete_onedrive_session(app_data_dir: &Path, key: &str) -> Result<(), RemoteError> {
    credentials::delete_credential(app_data_dir, key)
}

/// 这段存档**是不是** OneDrive 会话。
///
/// 用途：拦住「把 OneDrive 凭据读回前端」这条路 —— 通用的 `remote_load_credential`
/// 会把密文解密后原样返回给前端。适配层在调用它之前先问一句这个函数。
///
/// 判定靠三件事：能解析、`version` 认得、`access_token` 非空。
/// ⇒ 一份百度存档（`bduss`）或夸克存档（`pus`）解不开，判成「不是 OneDrive 会话」。
pub fn is_onedrive_session_blob(raw: &str) -> bool {
    serde_json::from_str::<OnedriveAuthSession>(raw)
        .map(|session| {
            session.version == ONEDRIVE_SESSION_VERSION && !session.access_token.trim().is_empty()
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(now: i64) -> OnedriveAuthSession {
        OnedriveAuthSession {
            version: ONEDRIVE_SESSION_VERSION,
            client_id: "app-client-id".to_string(),
            access_token: "access-token-value".to_string(),
            refresh_token: "refresh-token-value".to_string(),
            expires_at: now + 3_600,
            account: Some("someone@example.com".to_string()),
            drive_id: Some("drive-1".to_string()),
        }
    }

    // ---- 授权页地址 ---------------------------------------------------------

    #[test]
    fn the_authorize_url_carries_every_pkce_parameter() {
        let verifier = onedrive_generate_code_verifier();
        let challenge = pkce_code_challenge(&verifier);
        let state = onedrive_generate_state();

        let url = build_authorize_url(
            "app-client-id",
            "http://localhost:51234",
            &state,
            &challenge,
        );

        assert!(url.starts_with(ONEDRIVE_AUTHORIZE_URL));
        assert!(url.contains("client_id=app-client-id"));
        assert!(url.contains("response_type=code"));
        assert!(
            url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A51234"),
            "redirect_uri 必须被百分号编码：{url}"
        );
        assert!(
            url.contains("offline_access"),
            "缺 offline_access 就拿不到 refresh token：{url}"
        );
        assert!(url.contains("code_challenge_method=S256"));
        assert!(!state.is_empty(), "state 不能为空");
        assert!(
            url.contains(&format!("state={state}")),
            "state 必须原样进 URL：{url}"
        );
        assert_eq!(challenge.len(), 43, "S256 challenge 恒为 43 个字符");
    }

    // ---- PKCE ---------------------------------------------------------------

    /// RFC 7636 附录 B 的官方向量。这是唯一能证明「我们算的是 S256」的判据 ——
    /// 自己算一遍再自己比一遍是同义反复。
    #[test]
    fn pkce_challenge_matches_the_rfc_7636_vector() {
        assert_eq!(
            pkce_code_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn pkce_challenges_are_unpadded_base64url_of_sha256() {
        let verifier = onedrive_generate_code_verifier();
        let challenge = pkce_code_challenge(&verifier);

        assert_eq!(challenge.len(), 43);
        assert!(!challenge.contains('='), "只能是 no-pad 形式");
        assert!(!challenge.contains('+') && !challenge.contains('/'), "只能是 url-safe 字符集");
        assert!(
            verifier.len() >= ONEDRIVE_CODE_VERIFIER_MIN_LEN
                && verifier.len() <= ONEDRIVE_CODE_VERIFIER_MAX_LEN,
            "verifier 长度必须在 RFC 7636 的区间内：{}",
            verifier.len()
        );
        assert!(verifier.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' || c == '~'));

        // 独立性判据：换一个 verifier，challenge 必须跟着变。
        assert_ne!(challenge, pkce_code_challenge("another-verifier-0123456789-abcdefghijkl"));
    }

    #[test]
    fn state_and_verifier_are_not_repeated() {
        assert_ne!(onedrive_generate_state(), onedrive_generate_state());
        assert_ne!(
            onedrive_generate_code_verifier(),
            onedrive_generate_code_verifier()
        );
        assert_eq!(onedrive_generate_state().len(), 32);
        assert_eq!(onedrive_generate_code_verifier().len(), 64);
    }

    // ---- 令牌响应 -----------------------------------------------------------

    #[test]
    fn a_token_response_is_parsed_into_a_session() {
        let parsed = onedrive_parse_token_response(
            "app-client-id",
            r#"{"access_token":"at","refresh_token":"rt","expires_in":3600}"#,
            1_000,
            None,
        )
        .expect("解析");

        assert_eq!(parsed.version, ONEDRIVE_SESSION_VERSION);
        assert_eq!(parsed.client_id, "app-client-id");
        assert_eq!(parsed.access_token, "at");
        assert_eq!(parsed.refresh_token, "rt");
        assert_eq!(parsed.expires_at, 4_600);
    }

    /// 微软在刷新时有时不回新的 refresh token ⇒ 必须沿用旧的，
    /// 否则下一次刷新直接失去授权（用户看到的是「登录态莫名失效」）。
    #[test]
    fn a_refresh_without_a_new_refresh_token_keeps_the_old_one() {
        let parsed = onedrive_parse_token_response(
            "app-client-id",
            r#"{"access_token":"at2","expires_in":1800}"#,
            2_000,
            Some("old-refresh"),
        )
        .expect("解析");

        assert_eq!(parsed.refresh_token, "old-refresh");
    }

    #[test]
    fn an_oauth_error_response_is_reported_as_an_auth_error() {
        let error = onedrive_parse_token_response(
            "app-client-id",
            r#"{"error":"invalid_grant","error_description":"AADSTS70008: expired"}"#,
            0,
            None,
        )
        .expect_err("必须报错");

        assert_eq!(error.kind, crate::remote::error::RemoteErrorKind::Auth);
        assert!(error.message.contains("invalid_grant"));
        assert!(
            !error.message.contains("AADSTS70008"),
            "不要把 error_description 回显出来：{}",
            error.message
        );
    }

    // ---- 存档识别 -----------------------------------------------------------

    #[test]
    fn a_serialized_session_is_recognised_as_a_onedrive_blob() {
        let encoded = onedrive_encode_session(&session(1_000)).expect("编码");

        assert!(is_onedrive_session_blob(&encoded));
        assert_eq!(
            onedrive_decode_session(&encoded).expect("解码"),
            session(1_000)
        );
    }

    /// 阴性对照：别的网盘存档与普通密码**不能**被误认成 OneDrive 会话 ——
    /// 否则适配层的拦截会把所有 FTP/SFTP 密码都拦掉。
    #[test]
    fn other_blobs_and_plain_passwords_are_not_mistaken_for_a_onedrive_session() {
        assert!(!is_onedrive_session_blob("just a password"));
        assert!(!is_onedrive_session_blob(""));
        assert!(!is_onedrive_session_blob(r#"{"version":1,"bduss":"x"}"#));
        assert!(!is_onedrive_session_blob(r#"{"version":1,"pus":"x"}"#));
        assert!(!is_onedrive_session_blob(
            r#"{"version":2,"client_id":"a","access_token":"t","refresh_token":"r","expires_at":0}"#
        ));
    }

    #[test]
    fn an_unknown_session_version_is_an_auth_error_not_a_silent_miss() {
        let error = onedrive_decode_session(
            r#"{"version":9,"client_id":"a","access_token":"t","refresh_token":"r","expires_at":0}"#,
        )
        .expect_err("必须报错");

        assert_eq!(error.kind, crate::remote::error::RemoteErrorKind::Auth);
    }

    // ---- 状态 -----------------------------------------------------------------

    #[test]
    fn the_auth_status_is_false_without_a_session() {
        let payload = onedrive_auth_status_payload(None, 1_000);

        assert!(!payload.authorized);
        assert_eq!(payload.expires_at, None);
        assert_eq!(payload.account, None);
    }

    #[test]
    fn an_expired_token_without_refresh_token_is_not_authorized() {
        let mut expired = session(0);
        expired.refresh_token = String::new();

        let payload = onedrive_auth_status_payload(Some(&expired), 10_000);

        assert!(!payload.authorized, "既过期又刷不了，必须报未授权");
        assert_eq!(payload.account.as_deref(), Some("someone@example.com"));
    }

    #[test]
    fn an_expired_token_with_a_refresh_token_is_still_authorized() {
        let mut expired = session(0);
        expired.expires_at = 10;

        let payload = onedrive_auth_status_payload(Some(&expired), 10_000);

        assert!(payload.authorized, "还能刷新 ⇒ 用户不必重新登录");
    }

    #[test]
    fn refreshing_keeps_the_account_and_drive_id() {
        let previous = session(1_000);
        let mut refreshed = OnedriveAuthSession {
            version: ONEDRIVE_SESSION_VERSION,
            client_id: String::new(),
            access_token: "new-at".to_string(),
            refresh_token: "new-rt".to_string(),
            expires_at: 9_999,
            account: None,
            drive_id: None,
        };

        onedrive_apply_refresh(&previous, &mut refreshed);

        assert_eq!(refreshed.client_id, "app-client-id");
        assert_eq!(refreshed.account, previous.account);
        assert_eq!(refreshed.drive_id, previous.drive_id);
        assert_eq!(refreshed.access_token, "new-at");
    }

    /// `Debug` 绝不能把令牌打出来 —— 任何一条 `{:?}` 日志都会把它写进日志文件。
    #[test]
    fn the_debug_rendering_does_not_leak_tokens() {
        let rendered = format!("{:?}", session(1_000));

        assert!(!rendered.contains("access-token-value"));
        assert!(!rendered.contains("refresh-token-value"));
    }
}