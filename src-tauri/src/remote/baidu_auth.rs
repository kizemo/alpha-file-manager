// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! 百度网盘会话的抽取与持久化。
//!
//! 本文件被 `cargo test --lib` 的测试二进制链接，**不得出现 `tauri::AppHandle`、
//! `tauri::async_runtime::*` 或 `tauri::` 的任何路径** —— 理由见
//! `src-tauri/src/remote/mod.rs` 顶部（一旦碰到，测试二进制会以
//! `STATUS_ENTRYPOINT_NOT_FOUND` 在加载阶段崩掉，一个测试都跑不了）。
//!
//! # 分层
//!
//! - **本文件**：纯逻辑。从一份 cookie 清单里挑出 `BDUSS` / `STOKEN`，把会话
//!   编码成可落盘的 JSON，再交给 `credentials.rs`（DPAPI）写进
//!   `remote-credentials.json`。全程不碰网络、不碰 tauri。
//! - **`remote/commands.rs`**：唯一的 tauri 适配层。负责开网盘登录窗、读 webview
//!   cookie、把 cookie 投影成本文件的 [`BaiduCookieRecord`]。适配层里那件最要紧的
//!   事是**把 cookie 值从内存里尽快搬走**，别打进日志。
//!
//! # 为什么是 cookie 直连，而不是换 token
//!
//! 计划（`docs/plan-2026-11-08-baidu-netdisk-phase1.md` D2）原本定的是
//! 「内嵌 WebView2 登录 + 一次性换 `refresh_token`」。T1 实测**否决**了这条路：
//! `POST https://openapi.baidu.com/oauth/2.0/token` 对 `client_id=250528`
//! 与阴性对照返回**逐字相同**的 `invalid_client` ⇒ 没有任何可用的客户端身份，
//! `GET .../oauth/2.0/authorize` 更是 404。
//!
//! 于是路线改为：**持久化 `BDUSS` + `STOKEN` 两个 cookie，直接当会话用**。
//! 代价是「刷新」这一步不存在了 —— cookie 失效就是失效，只能重新登录。
//! 详见 `docs/handoff-2026-11-08-baidu-netdisk-phase1.md` §11。
//!
//! # 安全等级
//!
//! `BDUSS` **就是百度账号的登录态本身**，拿到它就能冒充用户。落盘走 DPAPI 密文，
//! 日志与错误信息里只出现长度与名字，**绝不出现明文**。

use crate::remote::baidu_api::{
    sanitize_cookie_value, BaiduSession, BAIDU_COOKIE_BDUSS, BAIDU_COOKIE_STOKEN,
};
use crate::remote::credentials;
use crate::remote::error::RemoteError;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// 落盘存档的版本号。读到的版本不认识 ⇒ 按「没有会话」处理 ——
/// 宁可让用户重新登录一次，也不要拿一份自己读不懂的存档去猜。
pub const BAIDU_SESSION_VERSION: u32 = 1;

/// 存档里 `stoken` 距离过期还有多久时算「快过期了」。
///
/// 过期前把这个时间窗留给用户重新登录，而不是等到请求真的 400 了才报「会话失效」。
pub const BAIDU_STOKEN_EXPIRY_WARN_SECONDS: u64 = 24 * 60 * 60;

// ===========================================================================
// cookie 清单的最小投影
// ===========================================================================

/// 一条 cookie 的最小投影。
///
/// 刻意**不复刻** tauri 的 `Cookie` 类型：本文件不能碰 `tauri::`，而 tauri 的
/// cookie 类型还带着 `domain` / `path` / `secure` / `http_only` 等一堆本文件用不上的
/// 字段。适配层负责把两边对上。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaiduCookieRecord {
    pub name: String,
    pub value: String,
}

impl BaiduCookieRecord {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// 扫 cookie 清单的结果：`BDUSS` 有没有、能不能用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaiduCookieOutcome {
    /// 没找到 `BDUSS` —— 用户还没登录，或者登录的是另一个域。
    Missing,
    /// 找到了 `BDUSS`，但值非法（被截断 / 含注入字符）。
    Invalid,
    /// 可用。
    Ready,
}

/// `STOKEN` 的下落。
///
/// `STOKEN` 是**可选** cookie（T1 记录：页面内 fetch 带的是整个 cookie jar，
/// 无法隔离出各端点究竟依赖哪一个）⇒ 它缺失或非法都**不该**让整个会话作废。
/// 但「它在但被丢弃了」必须能被观测到，否则会静悄悄退化成「只发 BDUSS」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaiduStokenState {
    /// 清单里没有 `STOKEN`。
    Absent,
    /// 有 `STOKEN`，但值非法，已丢弃。
    Rejected,
    /// 有 `STOKEN` 且可用。
    Present,
}

/// 一次 cookie 扫描的完整结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaiduCookieScan {
    pub outcome: BaiduCookieOutcome,
    pub stoken: BaiduStokenState,
    /// `outcome` 为 `Ready` 时才有值。
    pub session: Option<BaiduSession>,
}

/// 从 cookie 清单里挑出会话。
///
/// - `BDUSS` 是**必需**的：没有它就什么都做不了。
/// - `STOKEN` 是**可选**的：缺失只记状态，不影响 `BDUSS`。
/// - cookie 名**大小写敏感**（RFC 6265）⇒ `bduss` 不算 `BDUSS`。
/// - 同名出现多次时**取第一个**：WebView2 的清单顺序不保证稳定，
///   「取最后一个」会让同一个会话在不同轮次里变成不同的值。
pub fn scan_cookies(records: &[BaiduCookieRecord]) -> BaiduCookieScan {
    let bduss = records
        .iter()
        .find(|record| record.name == BAIDU_COOKIE_BDUSS)
        .map(|record| record.value.as_str());
    let stoken = records
        .iter()
        .find(|record| record.name == BAIDU_COOKIE_STOKEN)
        .map(|record| record.value.as_str());

    let Some(bduss) = bduss else {
        return BaiduCookieScan {
            outcome: BaiduCookieOutcome::Missing,
            stoken: match stoken {
                Some(_) => BaiduStokenState::Present,
                None => BaiduStokenState::Absent,
            },
            session: None,
        };
    };

    let Some(bduss) = sanitize_cookie_value(bduss) else {
        return BaiduCookieScan {
            outcome: BaiduCookieOutcome::Invalid,
            stoken: match stoken {
                Some(_) => BaiduStokenState::Present,
                None => BaiduStokenState::Absent,
            },
            session: None,
        };
    };

    let (stoken_value, stoken_state) = match stoken {
        None => (None, BaiduStokenState::Absent),
        Some(raw) => match sanitize_cookie_value(raw) {
            Some(value) => (Some(value), BaiduStokenState::Present),
            None => (None, BaiduStokenState::Rejected),
        },
    };

    BaiduCookieScan {
        outcome: BaiduCookieOutcome::Ready,
        stoken: stoken_state,
        session: Some(BaiduSession::new(bduss, stoken_value)),
    }
}

/// 把扫描结果变成一个**可以报给用户**的错误。
///
/// 错误文案里**不含**任何 cookie 内容，只说「去哪儿做什么」。
pub fn scan_error(scan: &BaiduCookieScan) -> Option<RemoteError> {
    match scan.outcome {
        BaiduCookieOutcome::Ready => None,
        BaiduCookieOutcome::Missing => Some(RemoteError::auth(
            "尚未获取到百度网盘登录凭据（BDUSS），请先在网盘登录窗口完成登录",
        )),
        BaiduCookieOutcome::Invalid => Some(RemoteError::auth(
            "百度网盘登录凭据（BDUSS）格式非法，请重新登录获取",
        )),
    }
}

/// 从一段自由格式文本里解析出会话（手工粘贴的降级路径）。
///
/// 接受三种写法：
///
/// 1. `BDUSS=xxx; STOKEN=yyy` —— 浏览器里复制出来的整串 cookie；
/// 2. `BDUSS=xxx` —— 只要 `BDUSS`；
/// 3. 裸值 —— 用户从别的工具里抠出来的 `BDUSS` 本体。
///
/// 另外容忍 DevTools 里常见的 `Set-Cookie: ` 前缀（整行复制出来就是这个形状）。
///
/// 三种写法都过一遍 [`sanitize_cookie_value`]，所以「值被截断 / 含注入字符」
/// 这种输入会被判非法，而不是被原样发出去。
pub fn parse_manual_cookie_input(raw: &str) -> Result<BaiduSession, RemoteError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(RemoteError::auth("请粘贴百度网盘的登录凭据"));
    }

    let body = strip_set_cookie_prefix(trimmed);

    let records = if body.contains(&format!("{BAIDU_COOKIE_BDUSS}=")) {
        parse_cookie_header(body)
    } else {
        // 裸值里出现 `=` 或 `;` ⇒ 这是一段我们没认出来的 cookie 头片段，
        // 而不是 `BDUSS` 本体（`BDUSS` 是 URL 编码的令牌，不含这两种字符）。
        // 按裸值硬吞下去只会得到一个「看起来有凭据、实际 400」的会话。
        if body.contains('=') || body.contains(';') {
            return Err(RemoteError::auth(
                "无法识别的百度网盘凭据格式，请粘贴 BDUSS=... 或整段 Cookie",
            ));
        }
        vec![BaiduCookieRecord::new(BAIDU_COOKIE_BDUSS, body)]
    };

    let scan = scan_cookies(&records);
    if let Some(error) = scan_error(&scan) {
        return Err(error);
    }

    Ok(scan.session.expect("Ready 的扫描结果一定带会话"))
}

/// 去掉 `Set-Cookie: ` 前缀。
///
/// DevTools 的「复制值」给的就是整行 `Set-Cookie: BDUSS=xxx; Path=/; HttpOnly`，
/// 不剥前缀的话会被当成一条名为 `Set-Cookie: BDUSS` 的 cookie ⇒ 认不出 `BDUSS`。
///
/// 用 `to_ascii_lowercase` 做大小写无关的前缀比较：它只改 ASCII 字节，
/// **字节长度不变** ⇒ 后面按 `10` 切片是安全的（非 ASCII 字符不会被它改动，
/// 也就不会让前缀匹配上）。
fn strip_set_cookie_prefix(raw: &str) -> &str {
    const PREFIX: &str = "set-cookie:";
    if raw.to_ascii_lowercase().starts_with(PREFIX) {
        return raw[PREFIX.len()..].trim_start();
    }
    raw
}

/// 把一段 cookie 头解析成清单。
///
/// 分隔符是 `;`，名字与值之间是**第一个** `=`（cookie 值里允许出现 `=`）。
/// 解析不了的片段直接跳过，不报错 —— 头是用户粘来的，脏一点是常态。
pub fn parse_cookie_header(raw: &str) -> Vec<BaiduCookieRecord> {
    raw.split(';')
        .filter_map(|segment| {
            let segment = segment.trim();
            if segment.is_empty() {
                return None;
            }
            let (name, value) = segment.split_once('=')?;
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            Some(BaiduCookieRecord::new(name, value.trim()))
        })
        .collect()
}

// ===========================================================================
// 会话状态
// ===========================================================================

/// 一个已持有的会话 + 它知道的元信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaiduSessionState {
    pub session: BaiduSession,
    /// `STOKEN` 的过期时刻（Unix 秒）。`None` 表示不知道过期时间
    /// （拿不到 `STOKEN`，或 `STOKEN` 是会话 cookie）。
    pub stoken_expires_unix: Option<u64>,
}

impl BaiduSessionState {
    pub fn new(session: BaiduSession, stoken_expires_unix: Option<u64>) -> Self {
        Self {
            session,
            stoken_expires_unix,
        }
    }
}

/// 会话当前的状态。给设置页看的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaiduAuthStatus {
    /// 没存过会话。
    Missing,
    /// 有会话，且 `STOKEN` 没过期（或压根没有 `STOKEN`）。
    Ready,
    /// 有会话，但 `STOKEN` 已经过期。
    ///
    /// `BDUSS` 仍在，`/rest/2.0/xpan/*` 这条主路径**大概率还能用**
    /// —— 所以这不是「未授权」，而是「需要重新登录以补一个新鲜的 `STOKEN`」。
    StokenExpired {
        /// 已经过期了多少秒。
        expired_by: u64,
    },
    /// 有会话，但 `STOKEN` 即将过期。
    StokenExpiringSoon {
        /// 还剩多少秒。
        remaining: u64,
    },
}

/// 当前 Unix 秒。
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// 判断一个过期时刻是否已过。
pub fn is_expired(expires_unix: Option<u64>, now_unix: u64) -> bool {
    matches!(expires_unix, Some(expires) if expires <= now_unix)
}

/// 距离过期还剩多少秒。`None` 表示不知道过期时间。
pub fn remaining_seconds(expires_unix: Option<u64>, now_unix: u64) -> Option<u64> {
    expires_unix.map(|expires| expires.saturating_sub(now_unix))
}

/// 判定会话状态。
///
/// `now_unix` 是**显式参数**而不是内部取当前时间：过期判定必须能在单测里
/// 精确复现，靠真实时钟的话，写出来的测试只是「今天过没过」而已。
pub fn baidu_auth_status(state: Option<&BaiduSessionState>, now_unix: u64) -> BaiduAuthStatus {
    let Some(state) = state else {
        return BaiduAuthStatus::Missing;
    };

    let Some(expires) = state.stoken_expires_unix else {
        // 不知道过期时间 ⇒ 乐观认为还能用。用户真遇到 400 时再重新登录。
        return BaiduAuthStatus::Ready;
    };

    if expires <= now_unix {
        return BaiduAuthStatus::StokenExpired {
            expired_by: now_unix - expires,
        };
    }

    if expires - now_unix <= BAIDU_STOKEN_EXPIRY_WARN_SECONDS {
        return BaiduAuthStatus::StokenExpiringSoon {
            remaining: expires - now_unix,
        };
    }

    BaiduAuthStatus::Ready
}

/// 给日志与界面用的**脱敏**描述。
///
/// 只有 cookie 的名字与长度，没有内容 —— 与 `BaiduSession::describe` 同一个口径。
pub fn describe_state(state: Option<&BaiduSessionState>) -> String {
    let Some(state) = state else {
        return "无会话".to_string();
    };

    let mut text = state.session.describe();
    if let Some(remaining) = remaining_seconds(state.stoken_expires_unix, unix_now()) {
        text.push_str(&format!(" + STOKEN剩余{remaining}秒"));
    }
    text
}

// ===========================================================================
// 落盘
// ===========================================================================

/// `remote-credentials.json` 里那条记录的内部形状。
///
/// **这是密文解密之后的内容**，由 DPAPI 保护；它自己**不是**明文。
#[derive(Debug, Serialize, Deserialize)]
struct StoredBaiduSession {
    version: u32,
    bduss: String,
    #[serde(default)]
    stoken: Option<String>,
    #[serde(default)]
    stoken_expires_unix: Option<u64>,
}

/// 把会话编码成可落盘的字符串。
pub fn encode_session(
    session: &BaiduSession,
    stoken_expires_unix: Option<u64>,
) -> Result<String, RemoteError> {
    let stored = StoredBaiduSession {
        version: BAIDU_SESSION_VERSION,
        bduss: session.bduss().to_string(),
        stoken: session.stoken().map(str::to_string),
        stoken_expires_unix,
    };

    serde_json::to_string(&stored)
        .map_err(|error| RemoteError::unknown(format!("序列化百度网盘会话失败: {error}")))
}

/// 把落盘的字符串解回会话。
///
/// 版本不认识、JSON 坏了、`BDUSS` 缺失或非法 ⇒ 一律报 `Auth`：
/// 这几种情形的用户处置都是同一个动作 —— **重新登录**。
/// 报 `Unknown` 只会让用户看到一句看不懂的话。
pub fn decode_session(raw: &str) -> Result<BaiduSessionState, RemoteError> {
    let stored: StoredBaiduSession = serde_json::from_str(raw)
        .map_err(|_| RemoteError::auth("已保存的百度网盘会话已损坏，请重新登录获取"))?;

    if stored.version != BAIDU_SESSION_VERSION {
        return Err(RemoteError::auth(
            "已保存的百度网盘会话版本不认识，请重新登录获取",
        ));
    }

    let bduss = sanitize_cookie_value(&stored.bduss)
        .ok_or_else(|| RemoteError::auth("已保存的百度网盘会话缺少有效的 BDUSS，请重新登录获取"))?;

    // `STOKEN` 非法只丢 `STOKEN`，不丢整个会话 —— 理由同 `scan_cookies`。
    let stoken = stored.stoken.as_deref().and_then(sanitize_cookie_value);

    Ok(BaiduSessionState::new(
        BaiduSession::new(bduss, stoken),
        stored.stoken_expires_unix,
    ))
}

/// 把会话还原成一段 cookie 头。
///
/// 用来「拿已保存的会话去测一次连接」—— 那条路径上我们手里只有一个
/// 结构化的会话，没有当初用户粘进来的原始文本。
///
/// 形态与 `parse_manual_cookie_input` 吃得下的形态一致（`BDUSS=...; STOKEN=...`），
/// 所以这段文本回到解析器那里能还原出同一个会话。
pub fn session_to_cookie_header(session: &BaiduSession) -> String {
    let mut header = format!("{BAIDU_COOKIE_BDUSS}={}", session.bduss());

    if let Some(stoken) = session.stoken() {
        header.push_str(&format!("; {BAIDU_COOKIE_STOKEN}={stoken}"));
    }

    header
}

/// 加密并保存会话。键就是连接的 `credential_key`。
pub fn save_baidu_session(
    app_data_dir: &Path,
    key: &str,
    session: &BaiduSession,
    stoken_expires_unix: Option<u64>,
) -> Result<(), RemoteError> {
    let encoded = encode_session(session, stoken_expires_unix)?;
    credentials::save_credential(app_data_dir, key, &encoded)
}

/// 读回会话。没有就是 `Ok(None)`。
///
/// **解不开 / 解出来是坏的 ⇒ 报错**，不悄悄当成「没存」。
/// 悄悄吞掉的话，用户会拿着一句「请先登录」反复重登，而真正的病因是存档坏了。
pub fn load_baidu_session(
    app_data_dir: &Path,
    key: &str,
) -> Result<Option<BaiduSessionState>, RemoteError> {
    let Some(encoded) = credentials::load_credential(app_data_dir, key)? else {
        return Ok(None);
    };

    decode_session(&encoded).map(Some)
}

/// 删掉已保存的会话。键不存在是成功的空操作。
pub fn delete_baidu_session(app_data_dir: &Path, key: &str) -> Result<(), RemoteError> {
    credentials::delete_credential(app_data_dir, key)
}

/// 这段存档**是不是**百度网盘会话。
///
/// 用途：拦住「把百度网盘凭据读回前端」这条路。通用的
/// `remote_load_credential` 会把密文解密后原样返回给前端 —— 对 FTP/SFTP 的
/// 密码来说那是既有行为，对 `BDUSS` 来说则是把登录态送回前端。
/// 适配层在调 `remote_load_credential` 之前先问一句这个函数。
pub fn is_baidu_session_blob(raw: &str) -> bool {
    serde_json::from_str::<StoredBaiduSession>(raw)
        .map(|stored| stored.version == BAIDU_SESSION_VERSION)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::error::RemoteErrorKind;
    use tempfile::TempDir;

    fn record(name: &str, value: &str) -> BaiduCookieRecord {
        BaiduCookieRecord::new(name, value)
    }

    fn temp_dir() -> TempDir {
        TempDir::new().expect("create temp dir")
    }

    const BDUSS: &str = "the-bduss-value-0123456789";
    const STOKEN: &str = "the-stoken-value-0123456789";

    // ---- 扫 cookie 清单 -----------------------------------------------------

    #[test]
    fn a_list_without_bduss_reports_missing() {
        let scan = scan_cookies(&[record("STOKEN", STOKEN)]);

        assert_eq!(scan.outcome, BaiduCookieOutcome::Missing);
        assert_eq!(scan.stoken, BaiduStokenState::Present);
        assert!(scan.session.is_none());
    }

    #[test]
    fn an_empty_list_reports_missing() {
        let scan = scan_cookies(&[]);

        assert_eq!(scan.outcome, BaiduCookieOutcome::Missing);
        assert_eq!(scan.stoken, BaiduStokenState::Absent);
    }

    #[test]
    fn a_valid_bduss_produces_a_usable_session() {
        let scan = scan_cookies(&[record("BDUSS", BDUSS)]);

        assert_eq!(scan.outcome, BaiduCookieOutcome::Ready);
        assert_eq!(scan.stoken, BaiduStokenState::Absent);
        let session = scan.session.expect("会话");
        assert_eq!(session.bduss(), BDUSS);
        assert!(session.stoken().is_none());
    }

    #[test]
    fn both_cookies_are_kept_when_both_are_present() {
        let scan = scan_cookies(&[record("BDUSS", BDUSS), record("STOKEN", STOKEN)]);

        assert_eq!(scan.outcome, BaiduCookieOutcome::Ready);
        assert_eq!(scan.stoken, BaiduStokenState::Present);
        let session = scan.session.expect("会话");
        assert_eq!(session.bduss(), BDUSS);
        assert_eq!(session.stoken(), Some(STOKEN));
    }

    /// 阳性对照：`cookie_header` 必须真的带上两个 cookie。
    /// 只断言 `session.stoken()` 的话，忘了拼进请求头这条缺陷就漏过去了。
    #[test]
    fn the_cookie_header_carries_both_cookies() {
        let scan = scan_cookies(&[record("BDUSS", BDUSS), record("STOKEN", STOKEN)]);

        let header = scan
            .session
            .expect("会话")
            .cookie_header()
            .expect("Cookie 头");

        assert!(header.contains(&format!("{BAIDU_COOKIE_BDUSS}={BDUSS}")));
        assert!(header.contains(&format!("{BAIDU_COOKIE_STOKEN}={STOKEN}")));
    }

    #[test]
    fn a_bduss_with_a_semicolon_is_rejected_as_invalid() {
        let scan = scan_cookies(&[record("BDUSS", "abc; injected=1")]);

        assert_eq!(scan.outcome, BaiduCookieOutcome::Invalid);
        assert!(scan.session.is_none());
    }

    #[test]
    fn a_bduss_with_whitespace_is_rejected_as_invalid() {
        let scan = scan_cookies(&[record("BDUSS", "abc def")]);

        assert_eq!(scan.outcome, BaiduCookieOutcome::Invalid);
    }

    #[test]
    fn an_empty_bduss_is_rejected_as_invalid() {
        let scan = scan_cookies(&[record("BDUSS", "")]);

        assert_eq!(scan.outcome, BaiduCookieOutcome::Invalid);
    }

    /// `STOKEN` 坏掉**不能**连累 `BDUSS`：少带一个可选 cookie 好过整个会话作废。
    #[test]
    fn a_rejected_stoken_does_not_take_the_session_down_with_it() {
        let scan = scan_cookies(&[record("BDUSS", BDUSS), record("STOKEN", "bad value")]);

        assert_eq!(scan.outcome, BaiduCookieOutcome::Ready);
        assert_eq!(scan.stoken, BaiduStokenState::Rejected);
        let session = scan.session.expect("会话");
        assert_eq!(session.bduss(), BDUSS);
        assert!(session.stoken().is_none());
    }

    /// cookie 名大小写敏感（RFC 6265）⇒ 小写的 `bduss` 不算数。
    #[test]
    fn cookie_names_are_case_sensitive() {
        let scan = scan_cookies(&[record("bduss", BDUSS)]);

        assert_eq!(scan.outcome, BaiduCookieOutcome::Missing);
    }

    #[test]
    fn unrelated_cookies_are_ignored() {
        let scan = scan_cookies(&[
            record("PANPSC", "whatever"),
            record("BDUSS", BDUSS),
            record("BAIDUID", "whatever"),
        ]);

        assert_eq!(scan.outcome, BaiduCookieOutcome::Ready);
    }

    /// 同名多次取**第一个**：WebView2 的清单顺序不保证稳定。
    #[test]
    fn a_duplicated_bduss_takes_the_first_occurrence() {
        let scan = scan_cookies(&[record("BDUSS", "first"), record("BDUSS", "second")]);

        let session = scan.session.expect("会话");
        assert_eq!(session.bduss(), "first");
    }

    // ---- 错误文案 -----------------------------------------------------------

    #[test]
    fn the_missing_credential_error_is_classified_as_auth() {
        let scan = scan_cookies(&[]);

        let error = scan_error(&scan).expect("应有错误");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    /// 错误文案里**绝不能**出现 cookie 内容。
    #[test]
    fn the_error_message_never_contains_the_credential() {
        let scan = scan_cookies(&[record("BDUSS", "abc; injected=1")]);

        let error = scan_error(&scan).expect("应有错误");

        assert!(!error.message.contains("injected"));
        assert!(!error.message.contains("abc"));
    }

    // ---- 手工粘贴的降级路径 -------------------------------------------------

    #[test]
    fn a_full_cookie_header_pasted_by_hand_is_accepted() {
        let raw = format!("BDUSS={BDUSS}; STOKEN={STOKEN}");

        let session = parse_manual_cookie_input(&raw).expect("应当解析成功");

        assert_eq!(session.bduss(), BDUSS);
        assert_eq!(session.stoken(), Some(STOKEN));
    }

    #[test]
    fn a_bare_bduss_value_pasted_by_hand_is_accepted() {
        let session = parse_manual_cookie_input(BDUSS).expect("应当解析成功");

        assert_eq!(session.bduss(), BDUSS);
        assert!(session.stoken().is_none());
    }

    #[test]
    fn a_bduss_only_header_pasted_by_hand_is_accepted() {
        let raw = format!("BDUSS={BDUSS}");

        let session = parse_manual_cookie_input(&raw).expect("应当解析成功");

        assert_eq!(session.bduss(), BDUSS);
    }

    #[test]
    fn an_empty_paste_is_rejected() {
        let error = parse_manual_cookie_input("   ").expect_err("空输入必须被拒");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    /// 粘了一整条 `Set-Cookie` 头 ⇒ 里面的 `BDUSS` **仍然要能被取出来**，
    /// `Path` / `HttpOnly` 这些属性段被忽略（它们不是我们要发的 cookie）。
    #[test]
    fn a_pasted_set_cookie_header_still_yields_the_bduss() {
        let raw = format!("Set-Cookie: BDUSS={BDUSS}; Path=/; HttpOnly");

        let session = parse_manual_cookie_input(&raw).expect("应当解析成功");

        assert_eq!(session.bduss(), BDUSS);
        assert!(session.stoken().is_none());
    }

    /// 没带 `Set-Cookie:` 前缀、但带了属性段的样子。
    #[test]
    fn a_pasted_header_with_attributes_yields_the_bduss() {
        let raw = format!("BDUSS={BDUSS}; Path=/; HttpOnly");

        let session = parse_manual_cookie_input(&raw).expect("应当解析成功");

        assert_eq!(session.bduss(), BDUSS);
    }

    /// 认不出来的片段必须**明确报错**，不能硬吞成一个「看着有凭据、实际 400」的会话。
    #[test]
    fn an_unrecognizable_fragment_is_rejected_instead_of_guessed() {
        let error = parse_manual_cookie_input("bduss=abc").expect_err("无法识别必须报错");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[test]
    fn the_cookie_header_parser_keeps_equals_signs_inside_values() {
        let records = parse_cookie_header("BDUSS=abc=def==; STOKEN=xyz");

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].value, "abc=def==");
        assert_eq!(records[1].name, "STOKEN");
    }

    #[test]
    fn the_cookie_header_parser_skips_broken_segments() {
        let records = parse_cookie_header(";;; = ; BDUSS=abc; ;");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "BDUSS");
        assert_eq!(records[0].value, "abc");
    }

    // ---- 状态判定 -----------------------------------------------------------

    #[test]
    fn a_missing_session_reads_as_missing() {
        assert_eq!(baidu_auth_status(None, 1_000), BaiduAuthStatus::Missing);
    }

    #[test]
    fn a_session_without_an_expiry_reads_as_ready() {
        let state = BaiduSessionState::new(BaiduSession::new(BDUSS, None), None);

        assert_eq!(
            baidu_auth_status(Some(&state), 1_000),
            BaiduAuthStatus::Ready
        );
    }

    #[test]
    fn a_fresh_stoken_reads_as_ready() {
        let state = BaiduSessionState::new(
            BaiduSession::new(BDUSS, Some(STOKEN.to_string())),
            Some(1_000 + 10 * BAIDU_STOKEN_EXPIRY_WARN_SECONDS),
        );

        assert_eq!(
            baidu_auth_status(Some(&state), 1_000),
            BaiduAuthStatus::Ready
        );
    }

    #[test]
    fn a_stoken_close_to_expiry_reads_as_expiring_soon() {
        let state = BaiduSessionState::new(
            BaiduSession::new(BDUSS, Some(STOKEN.to_string())),
            Some(1_000 + 60),
        );

        assert_eq!(
            baidu_auth_status(Some(&state), 1_000),
            BaiduAuthStatus::StokenExpiringSoon { remaining: 60 }
        );
    }

    #[test]
    fn an_expired_stoken_reads_as_expired() {
        let state = BaiduSessionState::new(
            BaiduSession::new(BDUSS, Some(STOKEN.to_string())),
            Some(900),
        );

        assert_eq!(
            baidu_auth_status(Some(&state), 1_000),
            BaiduAuthStatus::StokenExpired { expired_by: 100 }
        );
    }

    /// 过期时刻**正好等于**当前时刻 ⇒ 已过期。
    /// 差一秒的边界在真实时钟下极难复现，所以这里显式钉死。
    #[test]
    fn a_stoken_expiring_exactly_now_counts_as_expired() {
        let state = BaiduSessionState::new(
            BaiduSession::new(BDUSS, Some(STOKEN.to_string())),
            Some(1_000),
        );

        assert_eq!(
            baidu_auth_status(Some(&state), 1_000),
            BaiduAuthStatus::StokenExpired { expired_by: 0 }
        );
    }

    #[test]
    fn remaining_seconds_saturates_at_zero() {
        assert_eq!(remaining_seconds(Some(900), 1_000), Some(0));
        assert_eq!(remaining_seconds(None, 1_000), None);
        assert_eq!(remaining_seconds(Some(2_000), 1_000), Some(1_000));
    }

    #[test]
    fn the_expiry_helper_agrees_with_the_status_function() {
        assert!(is_expired(Some(900), 1_000));
        assert!(!is_expired(Some(1_100), 1_000));
        assert!(!is_expired(None, 1_000));
    }

    // ---- 脱敏 ---------------------------------------------------------------

    /// 脱敏断言：描述里只有名字与长度，**没有内容**。
    #[test]
    fn the_description_never_leaks_the_credential() {
        let state = BaiduSessionState::new(
            BaiduSession::new(BDUSS, Some(STOKEN.to_string())),
            Some(unix_now() + 3_600),
        );

        let text = describe_state(Some(&state));

        assert!(!text.contains(BDUSS));
        assert!(!text.contains(STOKEN));
        assert!(text.contains("BDUSS"));
        assert!(text.contains("len="));
    }

    #[test]
    fn the_description_of_a_missing_session_is_explicit() {
        assert_eq!(describe_state(None), "无会话");
    }

    #[test]
    fn the_debug_output_never_leaks_the_credential() {
        let session = BaiduSession::new(BDUSS, Some(STOKEN.to_string()));

        let text = format!("{session:?}");

        assert!(!text.contains(BDUSS));
        assert!(!text.contains(STOKEN));
    }

    // ---- 编码 / 解码 -------------------------------------------------------

    #[test]
    fn encoding_and_decoding_round_trip() {
        let session = BaiduSession::new(BDUSS, Some(STOKEN.to_string()));

        let encoded = encode_session(&session, Some(1_800_000_000)).expect("编码");
        let decoded = decode_session(&encoded).expect("解码");

        assert_eq!(decoded.session.bduss(), BDUSS);
        assert_eq!(decoded.session.stoken(), Some(STOKEN));
        assert_eq!(decoded.stoken_expires_unix, Some(1_800_000_000));
    }

    #[test]
    fn decoding_survives_a_session_without_a_stoken() {
        let session = BaiduSession::new(BDUSS, None);

        let encoded = encode_session(&session, None).expect("编码");
        let decoded = decode_session(&encoded).expect("解码");

        assert!(decoded.session.stoken().is_none());
        assert!(decoded.stoken_expires_unix.is_none());
    }

    #[test]
    fn decoding_garbage_is_classified_as_auth() {
        let error = decode_session("not json at all").expect_err("坏存档必须被拒");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[test]
    fn decoding_an_unknown_version_is_rejected() {
        let raw = r#"{"version":999,"bduss":"abc"}"#;

        let error = decode_session(raw).expect_err("版本不认识必须被拒");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[test]
    fn decoding_a_blob_without_bduss_is_rejected() {
        let raw = r#"{"version":1,"stoken":"abc"}"#;

        let error = decode_session(raw).expect_err("缺 BDUSS 必须被拒");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    /// 存档里带一个**非法**的 `BDUSS` ⇒ 整个会话作废（`BDUSS` 是必需的）。
    #[test]
    fn decoding_a_blob_with_an_invalid_bduss_is_rejected() {
        let raw = r#"{"version":1,"bduss":"abc; injected=1"}"#;

        let error = decode_session(raw).expect_err("非法 BDUSS 必须被拒");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    /// 存档里带一个**非法**的 `STOKEN` ⇒ 只丢 `STOKEN`，会话仍在。
    #[test]
    fn decoding_a_blob_with_an_invalid_stoken_keeps_the_session() {
        let raw = r#"{"version":1,"bduss":"abc","stoken":"bad value"}"#;

        let decoded = decode_session(raw).expect("应当解出 BDUSS");

        assert_eq!(decoded.session.bduss(), "abc");
        assert!(decoded.session.stoken().is_none());
    }

    // ---- 落盘往返 -----------------------------------------------------------

    #[test]
    fn saving_and_loading_round_trips_through_the_credential_file() {
        let dir = temp_dir();
        let session = BaiduSession::new(BDUSS, Some(STOKEN.to_string()));

        save_baidu_session(dir.path(), "conn-1", &session, Some(1_800_000_000)).expect("保存");
        let loaded = load_baidu_session(dir.path(), "conn-1")
            .expect("读取")
            .expect("应当有会话");

        assert_eq!(loaded.session.bduss(), BDUSS);
        assert_eq!(loaded.session.stoken(), Some(STOKEN));
        assert_eq!(loaded.stoken_expires_unix, Some(1_800_000_000));
    }

    /// 阳性对照：落盘的**确实是 DPAPI 密文**，明文与它的 base64 都不在文件里。
    #[test]
    fn the_credential_file_never_contains_the_plaintext_session() {
        let dir = temp_dir();
        let session = BaiduSession::new(BDUSS, Some(STOKEN.to_string()));

        save_baidu_session(dir.path(), "conn-1", &session, None).expect("保存");

        let raw = std::fs::read(credentials::credentials_path_in(dir.path())).expect("读原始文件");
        let text = String::from_utf8_lossy(&raw);

        assert!(!text.contains(BDUSS));
        assert!(!text.contains(STOKEN));
    }

    #[test]
    fn loading_an_unknown_key_returns_none() {
        let dir = temp_dir();

        assert!(load_baidu_session(dir.path(), "conn-absent")
            .expect("读取")
            .is_none());
    }

    #[test]
    fn saving_again_replaces_the_previous_session() {
        let dir = temp_dir();

        save_baidu_session(dir.path(), "conn-1", &BaiduSession::new("old", None), None)
            .expect("第一次保存");
        save_baidu_session(dir.path(), "conn-1", &BaiduSession::new("new", None), None)
            .expect("第二次保存");

        let loaded = load_baidu_session(dir.path(), "conn-1")
            .expect("读取")
            .expect("应当有会话");
        assert_eq!(loaded.session.bduss(), "new");
    }

    #[test]
    fn deleting_removes_the_session() {
        let dir = temp_dir();
        save_baidu_session(dir.path(), "conn-1", &BaiduSession::new(BDUSS, None), None)
            .expect("保存");

        delete_baidu_session(dir.path(), "conn-1").expect("删除");

        assert!(load_baidu_session(dir.path(), "conn-1")
            .expect("读取")
            .is_none());
    }

    #[test]
    fn deleting_an_unknown_key_is_a_no_op() {
        let dir = temp_dir();

        delete_baidu_session(dir.path(), "conn-absent").expect("删除不存在的键");
    }

    // ---- 存档识别 -----------------------------------------------------------

    #[test]
    fn a_baidu_blob_is_recognized() {
        let encoded = encode_session(&BaiduSession::new(BDUSS, None), None).expect("编码");

        assert!(is_baidu_session_blob(&encoded));
    }

    /// 阴性对照：普通密码存档**不能**被误认成百度网盘会话 ——
    /// 否则 `remote_load_credential` 的拦截会把所有 FTP/SFTP 密码都拦掉。
    #[test]
    fn an_ordinary_password_is_not_mistaken_for_a_baidu_session() {
        assert!(!is_baidu_session_blob("just a password"));
        assert!(!is_baidu_session_blob(""));
        assert!(!is_baidu_session_blob(r#"{"version":2,"bduss":"x"}"#));
    }
}
