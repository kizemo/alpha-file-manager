// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! 阿里云盘会话的生成、过期判定与加密落盘。
//!
//! 红线（与 `remote/mod.rs:8-14` 同一条）：本文件被 `cargo test --lib` 的测试
//! 二进制链接，**不得出现 `tauri::AppHandle`、`tauri::async_runtime::*` 或
//! `tauri::` 的任何路径**。一旦碰到，Tauri 全局运行时的默认类型参数 `Wry`
//! 被单态化，把 tao 整条窗口链拖进测试二进制；测试二进制没有 application
//! manifest 声明 comctl32 v6，`TaskDialogIndirect` 解析不到，进程在加载阶段
//! 以 `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉 —— 一个测试都跑不了，
//! 且报错与本模块毫无关系。
//!
//! # 分层
//!
//! - **本文件**：纯逻辑。PKCE 生成、过期判定、会话编码，交给 `credentials.rs`
//!   （DPAPI）写进 `remote-credentials.json`。全程不碰网络、不碰 tauri。
//! - **`remote/aliyun_provider.rs`**：HTTP 层。真正去阿里云盘换 token、列目录。
//! - **`src-tauri/src/aliyun_login.rs`**：tauri 适配层。授权窗、捕获授权码。
//!
//! # 安全等级
//!
//! `access_token` / `refresh_token` **就是账号的访问凭证本身**，拿到它就能冒充
//! 用户。落盘走 DPAPI 密文，日志与错误信息里只出现长度，**绝不出现明文** ——
//! 因此 [`AliyunAuthSession`] 手写了 `Debug`（只打印长度）。
//!
//! # 有效期一律以服务端返回为准
//!
//! 契约 §1.2：access_token 约 2 小时（密钥模式），PKCE 公开客户端模式可能长到
//! 30 天。**本文件不写死任何有效期常量** —— 只把响应里的 `expires_in` 记成
//! `expires_at_unix`，过期判定完全由它驱动。

use std::fmt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::remote::credentials;
use crate::remote::error::RemoteError;

/// 落盘存档的版本号。读到的版本不认识 ⇒ 按「没有凭据」处理 ——
/// 宁可让用户重新授权一次，也不要拿一份自己读不懂的存档去猜。
pub const ALIYUN_SESSION_VERSION: u32 = 1;

/// 「即将过期」的窗口（秒）。
///
/// 5 分钟足够用户在界面上看到提示并点「重新授权」，又不至于让一份刚换到的
/// token 立刻显示成「临期」。
pub const ALIYUN_EXPIRY_WARN_SECONDS: i64 = 300;

/// PKCE `code_verifier` 的合法长度区间（RFC 7636）。
pub const ALIYUN_CODE_VERIFIER_MIN_LEN: usize = 43;
pub const ALIYUN_CODE_VERIFIER_MAX_LEN: usize = 128;

/// 授权会话。落盘进 DPAPI 密文，键 = 连接的 `credential_key`（= 连接 id）。
///
/// 字段名**逐字**照契约 §2.2：这是持久化契约，改名会让已存下来的会话读不出来。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AliyunAuthSession {
    /// 访问凭证。**绝不进日志、绝不进前端载荷。**
    pub access_token: String,
    /// 刷新凭证。PKCE 公开客户端模式可能没有。
    pub refresh_token: Option<String>,
    /// access_token 的过期时刻（Unix **秒**）。`None` = 服务端没给有效期。
    pub expires_at_unix: Option<i64>,
    /// refresh_token 的过期时刻（Unix 秒）。
    pub refresh_expires_at_unix: Option<i64>,
    /// 空间 id。列表接口的 `drive_id` 就是它。
    pub drive_id: Option<String>,
    /// 开放平台应用 ID。换 token / 刷新时都要带。
    pub client_id: String,
    /// 开放平台应用密钥。`None` = PKCE 公开客户端模式。
    pub client_secret: Option<String>,
    /// 授权时用的回调地址。刷新时必须**原样**带回。
    pub redirect_uri: Option<String>,
    /// 授权时申请的 scope。
    pub scope: Option<String>,
    /// `Bearer`。服务端没给时按 `Bearer` 处理。
    pub token_type: String,
}

impl AliyunAuthSession {
    /// 该不该带 `Authorization` 头。空 token 一律不带。
    pub fn authorization_header_value(&self) -> Option<String> {
        let token = self.access_token.trim();
        if token.is_empty() {
            return None;
        }

        let scheme = self.token_type.trim();
        if scheme.is_empty() {
            Some(format!("Bearer {token}"))
        } else {
            Some(format!("{scheme} {token}"))
        }
    }

    /// 有没有可用于刷新的刷新凭证。
    pub fn has_refresh_token(&self) -> bool {
        self.refresh_token
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
    }

    /// 刷新凭证本身是不是也已经过期了。
    ///
    /// `None` = 服务端没给刷新凭证的过期时间 ⇒ 乐观认为还能用。
    pub fn is_refresh_expired(&self, now_unix: i64) -> bool {
        match self.refresh_expires_at_unix {
            Some(expires_at) => now_unix >= expires_at,
            None => false,
        }
    }

    /// 给日志用的**脱敏**描述：只有长度与状态，没有内容。
    pub fn describe(&self) -> String {
        let mut parts = vec![format!("access_token(len={})", self.access_token.len())];
        if let Some(refresh) = self.refresh_token.as_deref() {
            parts.push(format!("refresh_token(len={})", refresh.len()));
        }
        if let Some(drive_id) = self.drive_id.as_deref() {
            parts.push(format!("drive={drive_id}"));
        }
        if self.client_secret.is_some() {
            parts.push("secret=有".to_string());
        } else {
            parts.push("secret=无".to_string());
        }
        parts.join(" + ")
    }
}

/// 手写的 `Debug`：**只打印长度，绝不打印明文**。
impl fmt::Debug for AliyunAuthSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "AliyunAuthSession({})", self.describe())
    }
}

/// 会话当前的状态。给设置页看的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AliyunAuthStatus {
    /// 没存过会话。
    Missing,
    /// 有会话，且还没到过期时刻（也不在临期窗口里）。
    Ready,
    /// 有会话，但已经过了过期时刻 ⇒ **必须重新授权**。
    Stale {
        /// 已经过期了多少秒。
        elapsed: i64,
    },
    /// 有会话，且快到过期时刻了。
    StaleSoon {
        /// 还剩多少秒。
        remaining: i64,
    },
}

/// 当前 Unix 秒。
pub fn aliyun_now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// 这个会话**是不是已经过期**。
///
/// `expires_at_unix` 为 `None`（服务端没给有效期）⇒ **不认为过期**：
/// 宁可多试一次请求（服务端会明确回 401），也不要凭猜测把用户踢出登录态。
pub fn aliyun_is_expired(session: &AliyunAuthSession, now_unix: i64) -> bool {
    match session.expires_at_unix {
        Some(expires_at) => now_unix >= expires_at,
        None => false,
    }
}

/// 这个会话**是不是快要过期**（在临期窗口内）。
pub fn aliyun_is_expiring_soon(session: &AliyunAuthSession, now_unix: i64) -> bool {
    match session.expires_at_unix {
        Some(expires_at) => {
            let remaining = expires_at - now_unix;
            remaining > 0 && remaining <= ALIYUN_EXPIRY_WARN_SECONDS
        }
        None => false,
    }
}

/// 距过期还剩多少秒。`None` = 不知道过期时间。
pub fn aliyun_remaining_seconds(session: &AliyunAuthSession, now_unix: i64) -> Option<i64> {
    session
        .expires_at_unix
        .map(|expires_at| (expires_at - now_unix).max(0))
}

/// 判定会话状态。
///
/// `now_unix` 是**显式参数**而不是内部取当前时间：过期判定必须能在单测里
/// 精确复现，靠真实时钟的话，写出来的测试只是「今天过没过」而已。
pub fn aliyun_auth_status(session: Option<&AliyunAuthSession>, now_unix: i64) -> AliyunAuthStatus {
    let Some(session) = session else {
        return AliyunAuthStatus::Missing;
    };

    if aliyun_is_expired(session, now_unix) {
        let elapsed = now_unix - session.expires_at_unix.unwrap_or(now_unix);
        return AliyunAuthStatus::Stale { elapsed };
    }

    if aliyun_is_expiring_soon(session, now_unix) {
        return AliyunAuthStatus::StaleSoon {
            remaining: aliyun_remaining_seconds(session, now_unix).unwrap_or(0),
        };
    }

    AliyunAuthStatus::Ready
}

/// 状态的中文名 / 机器名。
///
/// **只有 4 种**：`missing` / `ready` / `stale` / `stale_soon`。
/// **不返回 `loading`** —— 后端没有「正在加载」这种状态可言，
/// 前端要表达加载中得自己管。
pub fn aliyun_status_name(status: &AliyunAuthStatus) -> &'static str {
    match status {
        AliyunAuthStatus::Missing => "missing",
        AliyunAuthStatus::Ready => "ready",
        AliyunAuthStatus::Stale { .. } => "stale",
        AliyunAuthStatus::StaleSoon { .. } => "stale_soon",
    }
}

/// 给日志与界面用的**脱敏**描述。
pub fn aliyun_describe_session(session: Option<&AliyunAuthSession>) -> String {
    let Some(session) = session else {
        return "无会话".to_string();
    };

    let mut text = session.describe();
    if let Some(expires_at) = session.expires_at_unix {
        let remaining = (expires_at - aliyun_now_unix()).max(0);
        text.push_str(&format!(" + 剩余{remaining}秒"));
    }
    text
}

// ===========================================================================
// PKCE
// ===========================================================================

/// 随机十六进制串。`len` 个字符（每个字符 4 bit）。
///
/// 用两个 uuid v4 拼出 32 位十六进制（契约 §3.4）。`uuid` 是既有依赖，不新增。
fn random_hex(len: usize) -> String {
    let mut out = String::with_capacity(len);
    while out.len() < len {
        out.push_str(&format!("{:x}", uuid::Uuid::new_v4().simple()));
    }
    out.truncate(len);
    out
}

/// 生成 PKCE 的 `code_verifier`：64 位随机十六进制。
///
/// 64 个字符落在 RFC 7636 要求的 `43..=128` 区间内。
pub fn aliyun_generate_code_verifier() -> String {
    random_hex(64)
}

/// 生成 OAuth 的 `state`：32 位随机十六进制。
pub fn aliyun_generate_state() -> String {
    random_hex(32)
}

/// 由 `code_verifier` 算出 S256 的 `code_challenge`：
/// `BASE64URL_NO_PAD(SHA256(code_verifier))`。
pub fn aliyun_code_challenge(code_verifier: &str) -> String {
    let digest = Sha256::digest(code_verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

// ===========================================================================
// 落盘
// ===========================================================================

/// 落盘的存档外壳。带版本号，读到不认识的版本按「没有凭据」处理。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredAliyunSession {
    version: u32,
    session: AliyunAuthSession,
}

/// 编码一份会话。**不加密** —— 加密由 `credentials.rs`（DPAPI）做。
pub fn aliyun_encode_session(session: &AliyunAuthSession) -> Result<String, RemoteError> {
    let stored = StoredAliyunSession {
        version: ALIYUN_SESSION_VERSION,
        session: session.clone(),
    };

    serde_json::to_string(&stored)
        .map_err(|error| RemoteError::unknown(format!("阿里云盘会话编码失败：{error}")))
}

/// 解码一份会话。
///
/// **解不开 / 版本不认识 ⇒ 报错**，不悄悄当成「没存」。
/// 悄悄吞掉的话，用户会拿着一句「请先授权」反复重登，而真正的病因是存档坏了。
pub fn aliyun_decode_session(raw: &str) -> Result<AliyunAuthSession, RemoteError> {
    let stored: StoredAliyunSession = serde_json::from_str(raw)
        .map_err(|_| RemoteError::auth("阿里云盘会话存档无法解析，请重新授权。"))?;

    if stored.version != ALIYUN_SESSION_VERSION {
        return Err(RemoteError::auth(
            "阿里云盘会话存档版本不认识，请重新授权。",
        ));
    }

    Ok(stored.session)
}

/// 加密并保存会话。键就是连接的 `credential_key`。
pub fn save_aliyun_session(
    app_data_dir: &Path,
    key: &str,
    session: &AliyunAuthSession,
) -> Result<(), RemoteError> {
    let encoded = aliyun_encode_session(session)?;
    credentials::save_credential(app_data_dir, key, &encoded)
}

/// 读回会话。没有就是 `Ok(None)`。
pub fn load_aliyun_session(
    app_data_dir: &Path,
    key: &str,
) -> Result<Option<AliyunAuthSession>, RemoteError> {
    let Some(encoded) = credentials::load_credential(app_data_dir, key)? else {
        return Ok(None);
    };

    aliyun_decode_session(&encoded).map(Some)
}

/// 删掉已保存的会话。键不存在是成功的空操作。
pub fn delete_aliyun_session(app_data_dir: &Path, key: &str) -> Result<(), RemoteError> {
    credentials::delete_credential(app_data_dir, key)
}

/// 这段存档**是不是**阿里云盘会话。
///
/// 用途：拦住「把阿里云盘凭据读回前端」这条路 —— 通用的 `remote_load_credential`
/// 会把密文解密后原样返回给前端。
pub fn is_aliyun_session_blob(raw: &str) -> bool {
    serde_json::from_str::<StoredAliyunSession>(raw)
        .map(|stored| stored.version == ALIYUN_SESSION_VERSION)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::error::RemoteErrorKind;

    const BASE: i64 = 1_800_000_000;

    fn session(expires_at: Option<i64>) -> AliyunAuthSession {
        AliyunAuthSession {
            access_token: "the-access-token-value-0123456789".to_string(),
            refresh_token: Some("the-refresh-token-value-0123456789".to_string()),
            expires_at_unix: expires_at,
            refresh_expires_at_unix: expires_at.map(|value| value + 86_400),
            drive_id: Some("drive-123".to_string()),
            client_id: "client-abc".to_string(),
            client_secret: Some("secret-xyz".to_string()),
            redirect_uri: Some("http://127.0.0.1:47832/callback".to_string()),
            scope: Some("user:base file:all:read".to_string()),
            token_type: "Bearer".to_string(),
        }
    }

    // ---- 过期判定 -----------------------------------------------------------

    #[test]
    fn a_session_without_an_expiry_is_never_expired() {
        assert!(!aliyun_is_expired(&session(None), BASE));
    }

    #[test]
    fn a_session_is_expired_once_the_expiry_moment_arrives() {
        assert!(!aliyun_is_expired(&session(Some(BASE + 1)), BASE));
        assert!(aliyun_is_expired(&session(Some(BASE)), BASE));
        assert!(aliyun_is_expired(&session(Some(BASE - 1)), BASE));
    }

    #[test]
    fn the_expiry_window_marks_a_session_as_expiring_soon() {
        // 还在窗口之外 ⇒ 不是临期。
        assert!(!aliyun_is_expiring_soon(
            &session(Some(BASE + ALIYUN_EXPIRY_WARN_SECONDS + 1)),
            BASE
        ));
        // 正好落在窗口边界上 ⇒ 临期。
        assert!(aliyun_is_expiring_soon(
            &session(Some(BASE + ALIYUN_EXPIRY_WARN_SECONDS)),
            BASE
        ));
        // 已经过期 ⇒ 不算「临期」（那是「过期」）。
        assert!(!aliyun_is_expiring_soon(&session(Some(BASE)), BASE));
        // 没有过期时间 ⇒ 不是临期。
        assert!(!aliyun_is_expiring_soon(&session(None), BASE));
    }

    #[test]
    fn the_status_follows_the_expiry_and_the_warning_window() {
        assert_eq!(aliyun_auth_status(None, BASE), AliyunAuthStatus::Missing);
        assert_eq!(
            aliyun_auth_status(Some(&session(Some(BASE + 3_600))), BASE),
            AliyunAuthStatus::Ready
        );
        assert_eq!(
            aliyun_auth_status(Some(&session(Some(BASE + 60))), BASE),
            AliyunAuthStatus::StaleSoon { remaining: 60 }
        );
        assert_eq!(
            aliyun_auth_status(Some(&session(Some(BASE))), BASE),
            AliyunAuthStatus::Stale { elapsed: 0 }
        );
        assert_eq!(
            aliyun_auth_status(Some(&session(Some(BASE - 120))), BASE),
            AliyunAuthStatus::Stale { elapsed: 120 }
        );
    }

    #[test]
    fn the_remaining_seconds_saturate_at_zero() {
        assert_eq!(
            aliyun_remaining_seconds(&session(Some(BASE + 100)), BASE),
            Some(100)
        );
        assert_eq!(
            aliyun_remaining_seconds(&session(Some(BASE - 100)), BASE),
            Some(0)
        );
        assert_eq!(aliyun_remaining_seconds(&session(None), BASE), None);
    }

    // ---- PKCE ---------------------------------------------------------------

    #[test]
    fn the_code_verifier_length_is_inside_the_rfc_range() {
        let verifier = aliyun_generate_code_verifier();

        assert!(
            verifier.len() >= ALIYUN_CODE_VERIFIER_MIN_LEN
                && verifier.len() <= ALIYUN_CODE_VERIFIER_MAX_LEN,
            "code_verifier 的长度必须在 43..=128 之间，实际 {}",
            verifier.len()
        );
        assert!(verifier.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn the_code_challenge_is_the_s256_base64url_of_the_verifier() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = aliyun_code_challenge(verifier);

        // RFC 7636 附录 B 的官方测试向量。
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        assert!(!challenge.contains('='), "BASE64URL_NO_PAD 不带填充");
        assert!(!challenge.contains('+') && !challenge.contains('/'));
    }

    #[test]
    fn the_code_challenge_is_stable_and_verifier_dependent() {
        let verifier = aliyun_generate_code_verifier();

        assert_eq!(
            aliyun_code_challenge(&verifier),
            aliyun_code_challenge(&verifier)
        );
        assert_ne!(
            aliyun_code_challenge(&verifier),
            aliyun_code_challenge(&aliyun_generate_code_verifier())
        );
    }

    #[test]
    fn the_state_is_thirty_two_hex_characters_and_unpredictable() {
        let state = aliyun_generate_state();

        assert_eq!(state.len(), 32);
        assert!(state.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(state, aliyun_generate_state());
    }

    // ---- 脱敏 ---------------------------------------------------------------

    #[test]
    fn the_debug_output_never_leaks_the_tokens() {
        let text = format!("{:?}", session(Some(BASE)));

        assert!(!text.contains("the-access-token-value-0123456789"));
        assert!(!text.contains("the-refresh-token-value-0123456789"));
        assert!(!text.contains("secret-xyz"));
        assert!(text.contains("access_token(len="));
    }

    #[test]
    fn the_description_never_leaks_the_tokens() {
        let text = aliyun_describe_session(Some(&session(Some(BASE))));

        assert!(!text.contains("the-access-token-value-0123456789"));
        assert!(!text.contains("the-refresh-token-value-0123456789"));
        assert!(!text.contains("secret-xyz"));
    }

    // ---- 编码 / 解码 -------------------------------------------------------

    #[test]
    fn encoding_and_decoding_round_trip() {
        let encoded = aliyun_encode_session(&session(Some(BASE))).expect("编码");
        let decoded = aliyun_decode_session(&encoded).expect("解码");

        assert_eq!(decoded, session(Some(BASE)));
    }

    #[test]
    fn decoding_garbage_is_classified_as_auth() {
        let error = aliyun_decode_session("not json at all").expect_err("坏存档必须被拒");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[test]
    fn decoding_an_unknown_version_is_rejected_without_panicking() {
        let raw = r#"{"version":999,"session":{"access_token":"a"}}"#;

        let error = aliyun_decode_session(raw).expect_err("版本不认识必须被拒");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[test]
    fn a_stored_blob_is_recognised_as_an_aliyun_session() {
        let encoded = aliyun_encode_session(&session(Some(BASE))).expect("编码");

        assert!(is_aliyun_session_blob(&encoded));
        assert!(!is_aliyun_session_blob(r#"{"version":1,"pus":"abc"}"#));
        assert!(!is_aliyun_session_blob("not json"));
    }

    // ---- 授权头 -------------------------------------------------------------

    #[test]
    fn the_authorization_header_uses_the_recorded_token_type() {
        let mut value = session(None);
        assert_eq!(
            value.authorization_header_value().as_deref(),
            Some("Bearer the-access-token-value-0123456789")
        );

        value.token_type = String::new();
        assert_eq!(
            value.authorization_header_value().as_deref(),
            Some("Bearer the-access-token-value-0123456789")
        );

        value.access_token = String::new();
        assert_eq!(value.authorization_header_value(), None);
    }

    #[test]
    fn a_refresh_token_is_only_considered_present_when_it_is_not_blank() {
        let mut value = session(None);
        assert!(value.has_refresh_token());

        value.refresh_token = Some("   ".to_string());
        assert!(!value.has_refresh_token());

        value.refresh_token = None;
        assert!(!value.has_refresh_token());
    }
}
