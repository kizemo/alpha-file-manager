// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! 夸克网盘会话的抽取与持久化。
//!
//! 本文件被 `cargo test --lib` 的测试二进制链接，**不得出现 `tauri::AppHandle`、
//! `tauri::async_runtime::*` 或 `tauri::` 的任何路径** —— 理由见
//! `src-tauri/src/remote/mod.rs` 顶部（一旦碰到，测试二进制会以
//! `STATUS_ENTRYPOINT_NOT_FOUND` 在加载阶段崩掉，一个测试都跑不了）。
//!
//! # 分层
//!
//! - **本文件**：纯逻辑。从一份 cookie 清单里挑出会话 cookie，把会话编码成可落盘的
//!   JSON，再交给 `credentials.rs`（DPAPI）写进 `remote-credentials.json`。
//!   全程不碰网络、不碰 tauri。
//! - **`remote/commands.rs` / `remote/quark_login.rs`**：适配层。负责开网盘登录窗、
//!   读 webview cookie、把 cookie 投影成本文件的 [`QuarkCookieRecord`]。
//!
//! # 为什么是 cookie 直连
//!
//! 方案（`docs/plan-2026-11-11-quark-pan.md`）与百度侧同构：**持久化登录态 cookie，
//! 直接当会话用**。夸克没有公开的 refresh_token 流程（§7-U2），所以「刷新」这一步
//! 同样不存在 —— cookie 失效就是失效，只能重新登录。
//!
//! # 夸克没有 STOKEN ⇒ 状态语义是**重新起草**的，不是照抄
//!
//! 百度的授权状态绑死在 `STOKEN` 的过期时刻上（`stoken_expired` /
//! `stoken_expiring_soon`）。夸克这一侧拿不到本地可读的过期时间，于是本文件用
//! **「距上次验活多久」** 作为等价的代理量：
//!
//! - `last_alive_unix` = **上一次对服务端验活成功**的时刻（登录时的「保险三」
//!   验活、或 provider 一次成功列举之后更新）。
//! - 「过期」= 距上次验活已超过 [`QUARK_SESSION_TRUST_SECONDS`]（信任窗口）。
//! - 「临期」= 距信任窗口结束还不到 [`QUARK_LAST_ALIVE_WARN_SECONDS`]。
//!
//! ⚠️ 这两个阈值是**保守启发式**，不是服务端的真相：夸克什么时候把 `__pus` 作废，
//! 本地无从得知，真正的死亡证据只有服务端返回的 `code=31001`。所以状态为
//! [`QuarkAuthStatus::Stale`] 时只是**建议重新登录**，不是「已经失效」的断言。
//!
//! # 安全等级
//!
//! 登录态 cookie **就是账号的登录态本身**，拿到它就能冒充用户。落盘走 DPAPI 密文，
//! 日志与错误信息里只出现长度与名字，**绝不出现明文** —— 因此本文件里的
//! [`QuarkAuthSession`] 与 [`QuarkCookieRecord`] 都手写了 `Debug`（只打印长度）。
//!
//! # 与 `quark_api.rs` 的重复项（**模块合并时统一**）
//!
//! `quark_api.rs`（同期并行开发）也定义了一套 cookie 常量与清洗函数。本文件
//! **刻意不 import 它** —— 那个模块此刻还不存在，import 会让本文件编译不过。
//! 重复的只有下列三处，合并时以 `quark_api` 的那一份为准：
//!
//! | 本文件 | `quark_api.rs` |
//! |---|---|
//! | [`QUARK_COOKIE_PUS`] | `QUARK_SESSION_COOKIE_PUS` |
//! | [`sanitize_quark_cookie_value`] | `sanitize_cookie_value` |
//! | [`QuarkAuthSession`] | `QuarkSession` |
//!
//! 本文件的 [`QuarkAuthSession`] 之所以不直接叫 `QuarkSession`，是为了避免与
//! `quark_api` 的同名类型在 `remote/mod.rs` 的 `pub use` 里撞名。

use crate::remote::credentials;
use crate::remote::error::RemoteError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 落盘存档的版本号。读到的版本不认识 ⇒ 按「没有凭据」处理 ——
/// 宁可让用户重新登录一次，也不要拿一份自己读不懂的存档去猜。
pub const QUARK_SESSION_VERSION: u32 = 1;

/// 会话 cookie 名：账号登录态，等价于密码。
///
/// ⚠️ **与 `quark_api.rs` 的 `QUARK_SESSION_COOKIE_PUS` 重复**（并行开发期，
/// 本文件不 import 那个尚未存在的模块）⇒ **模块合并时统一到一处**。
///
/// 证据强度：只有开源项目侧说法（方案 §7-U1，B 层），本地 renderer 从不按名字取
/// cookie。真机结论以 T0 为准；在此之前，`__pus` 是**唯一已知**的必需 cookie 名。
pub const QUARK_COOKIE_PUS: &str = "__pus";

/// 下载凭据 cookie 名：与 `__pus` 同域（`.quark.cn`），但**它是下载链的门槛**。
///
/// 真机取证（2026-11-11，读应用自己的 WebView2 cookie 罐）：
/// `__pus`（elen=183）与 `__puus`（elen=311）**都挂在 `.quark.cn`**，
/// `__puus` 比 `__pus` **晚约 1 秒**落地。只带 `__pus` 的会话能列目录，
/// 但下载直链会被 CDN 以 HTTP 412 拒绝 ⇒ 用户看到的就是
/// 「下载到本地失败（参数或协议层错误）」。
///
/// 证据强度：真机 cookie 罐 + 412 实测（A 层）。这条常量就是把那个观测写进代码。
pub const QUARK_COOKIE_PUUS: &str = "__puus";

/// `__pus` 出现后，再等 `__puus` 落地的**宽限时长**（秒）。
///
/// 必须是**有限宽限**而不是硬门槛：夸克不一定给每个账号都发 `__puus`，
/// 把它当必需项会让登录流程死等到超时（`GIVE_UP` 900s），比现在这个 bug 更糟。
/// 到点照样存盘，只是往日志里记一条警告；缺 `__puus` 的后果由
/// `failure_from_http_status(412) => Auth` 兜住 —— 用户会看到「请重新授权」。
pub const QUARK_PUUS_SETTLE_SECONDS: u64 = 20;

/// 一次验活成功后，这份会话被**信任**多久（秒）。
///
/// 夸克不暴露 cookie 的过期时间 ⇒ 只能拿「距上次验活多久」当过期时刻的代理。
/// 默认 7 天：够覆盖「用户几天不登录也不会突然掉线」，又不至于让一份
/// 早就作废的 cookie 一直显示成「已授权」。
///
/// ⚠️ **保守启发式，不是服务端的真相**（见模块头部说明）。
pub const QUARK_SESSION_TRUST_SECONDS: u64 = 7 * 24 * 60 * 60;

/// 距信任窗口结束还剩不到这个时长（秒）时，状态变成「临期」。
///
/// 对应百度的 `BAIDU_STOKEN_EXPIRY_WARN_SECONDS`（方案 §5 阶段 2 里的
/// `QUARK_STOKEN_EXPIRY_WARN_SECONDS`）—— 夸克没有 STOKEN，语义改成
/// 「距上次验活时间」，所以**常量名也跟着改**，不保留 STOKEN 字样。
pub const QUARK_LAST_ALIVE_WARN_SECONDS: u64 = 24 * 60 * 60;

// ===========================================================================
// cookie 清单的最小投影
// ===========================================================================

/// 一条 cookie 的最小投影。
///
/// 刻意**不复刻** tauri 的 `Cookie` 类型：本文件不能碰 `tauri::`，而 tauri 的
/// cookie 类型还带着 `domain` / `path` / `secure` / `http_only` 等一堆本文件用不上的
/// 字段。适配层负责把两边对上。
///
/// `Debug` 是**手写的**：只打印名字与长度，**不打印值** —— 值就是登录态本身。
#[derive(Clone, PartialEq, Eq)]
pub struct QuarkCookieRecord {
    pub name: String,
    pub value: String,
}

impl QuarkCookieRecord {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

impl fmt::Debug for QuarkCookieRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "QuarkCookieRecord({} len={})",
            self.name,
            self.value.len()
        )
    }
}

/// 扫 cookie 清单的结果：`__pus` 有没有、能不能用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarkCookieOutcome {
    /// 没找到 `__pus` —— 用户还没登录，或者登录的是另一个域。
    Missing,
    /// 找到了 `__pus`，但值非法（被截断 / 含注入字符）。
    Invalid,
    /// 可用。
    Ready,
}

/// 一次 cookie 扫描的完整结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarkCookieScan {
    pub outcome: QuarkCookieOutcome,
    /// `outcome` 为 `Ready` 时才有值。
    pub session: Option<QuarkAuthSession>,
}

/// 校验并清洗一个 cookie 的值。
///
/// RFC 6265 的 cookie-value 不允许 CTLs、空白、`"`、`,`、`;`、`\`。这几个字符
/// 出现在会话 cookie 里只有两种可能：**值被截断了**或者**有人想注入头部**
/// —— 两种都不能原样发出去。所以这里不是「过滤」而是**判非法**：含这些字符
/// 就返回 `None`，宁可让调用方报「会话失效」，也不发一个语义不明的头。
///
/// ⚠️ **与 `quark_api.rs` 的 `sanitize_cookie_value` 重复**（并行开发期，
/// 本文件不 import 那个尚未存在的模块）⇒ **模块合并时统一到一处**。
pub fn sanitize_quark_cookie_value(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let forbidden = trimmed.chars().any(|character| {
        character.is_control()
            || character.is_whitespace()
            || matches!(character, '"' | ',' | ';' | '\\')
    });
    if forbidden {
        return None;
    }

    Some(trimmed.to_string())
}

/// 从 cookie 清单里挑出会话。
///
/// - `__pus` 是**必需**的：没有它就什么都做不了。
/// - 其余 cookie 是**可选的**：一并留着（整罐 `.quark.cn` cookie）。
///   夸克到底需要单个 `__pus` 还是整罐 cookie，方案 §7-U11 未定 ⇒
///   **多带几个 cookie 比少带安全**，而 `scan_cookies` 只负责照实收集，
///   裁剪留给 T0 的真机分组对照。
/// - cookie 名**大小写敏感**（RFC 6265）⇒ `__pus` 的大小写变体不算。
/// - 同名出现多次时**取第一个**：WebView2 的清单顺序不保证稳定，
///   「取最后一个」会让同一个会话在不同轮次里变成不同的值。
/// - 值非法的**补充** cookie 直接丢掉（少带一个可选 cookie 好过整个会话作废）；
///   `__pus` 自己非法 ⇒ 整个会话作废（它是必需的，没有它什么都做不了）。
pub fn scan_cookies(records: &[QuarkCookieRecord]) -> QuarkCookieScan {
    let pus = records
        .iter()
        .find(|record| record.name == QUARK_COOKIE_PUS)
        .map(|record| record.value.as_str());

    let Some(pus) = pus else {
        return QuarkCookieScan {
            outcome: QuarkCookieOutcome::Missing,
            session: None,
        };
    };

    let Some(pus) = sanitize_quark_cookie_value(pus) else {
        return QuarkCookieScan {
            outcome: QuarkCookieOutcome::Invalid,
            session: None,
        };
    };

    let mut cookies = BTreeMap::new();
    for record in records {
        // `__pus` 已经有单独的字段了；同名再进 extras 会让 Cookie 头出现两条
        // `__pus=`，服务端怎么解析是它的事，但没必要给自己制造这种歧义。
        if record.name.is_empty() || record.name == QUARK_COOKIE_PUS {
            continue;
        }
        let Some(value) = sanitize_quark_cookie_value(&record.value) else {
            continue;
        };
        // 同名取第一个。
        cookies.entry(record.name.clone()).or_insert(value);
    }

    QuarkCookieScan {
        outcome: QuarkCookieOutcome::Ready,
        session: Some(QuarkAuthSession::new(pus, cookies)),
    }
}

/// 把扫描结果变成一个**可以报给用户**的错误。
///
/// 错误文案里**不含**任何 cookie 内容，只说「去哪儿做什么」。
pub fn scan_error(scan: &QuarkCookieScan) -> Option<RemoteError> {
    match scan.outcome {
        QuarkCookieOutcome::Ready => None,
        QuarkCookieOutcome::Missing => Some(RemoteError::auth(
            "尚未获取到夸克网盘登录凭据（__pus），请先在网盘登录窗口完成登录",
        )),
        QuarkCookieOutcome::Invalid => Some(RemoteError::auth(
            "夸克网盘登录凭据（__pus）格式非法，请重新登录获取",
        )),
    }
}

// ===========================================================================
// 会话状态
// ===========================================================================

/// 夸克网盘的会话凭据。
///
/// **安全等级等同于密码**：`__pus` 就是账号的登录态本身，拿到它就能冒充用户。
/// 落盘（DPAPI 密文）、日志、错误信息里都不得出现明文 —— 因此这里手写了
/// `Debug`（不 derive），只打印长度不打印内容。
///
/// `cookies` 是**其余**的 `.quark.cn` cookie（整罐里除 `__pus` 之外的部分）。
/// 它们是可选的：方案 §7-U11 未定夸克到底需要几个 cookie，多带着比少带安全。
#[derive(Clone, PartialEq, Eq)]
pub struct QuarkAuthSession {
    pus: String,
    cookies: BTreeMap<String, String>,
}

impl QuarkAuthSession {
    /// 新建一个会话。`cookies` 是可选的补充 cookie（`__pus` 本身不在里面）。
    pub fn new(pus: impl Into<String>, cookies: BTreeMap<String, String>) -> Self {
        Self {
            pus: pus.into(),
            cookies,
        }
    }

    /// `__pus` 的原值。**只在拼 Cookie 头时用**，不要拿去打日志。
    pub fn pus(&self) -> &str {
        &self.pus
    }

    /// 其余 cookie 的原值（按名字排序，保证拼出来的头是稳定的）。
    pub fn cookies(&self) -> &BTreeMap<String, String> {
        &self.cookies
    }

    /// 是否至少有一个可用的 cookie。
    pub fn has_credentials(&self) -> bool {
        !self.pus.trim().is_empty()
    }

    /// `__puus` 的原值，没有就返回 `None`。
    ///
    /// 它和其余 cookie 一起存在 `cookies` 里（`scan_cookies` 只把 `__pus` 单拆出来），
    /// 所以这里只是按名字取回；**不要**拿去打日志。
    pub fn puus(&self) -> Option<&str> {
        self.cookies.get(QUARK_COOKIE_PUUS).map(String::as_str)
    }

    /// 是否**齐了下载用的凭据**（`__pus` + `__puus`）。
    ///
    /// 列目录只需要 `__pus`；下载直链需要 `__puus`。登录流程拿这个判断
    /// 「现在存盘会不会存出一个下载不了的会话」。
    pub fn has_download_credentials(&self) -> bool {
        self.has_credentials() && self.puus().is_some()
    }

    /// 给日志用的**脱敏**描述：只有名字与长度，没有内容。
    pub fn describe(&self) -> String {
        let mut parts = vec![format!("{QUARK_COOKIE_PUS}(len={})", self.pus.len())];
        if let Some(puus) = self.puus() {
            parts.push(format!("{QUARK_COOKIE_PUUS}(len={})", puus.len()));
        }
        if !self.cookies.is_empty() {
            parts.push(format!("extras={}", self.cookies.len()));
        }
        parts.join(" + ")
    }
}

/// 登录流程是否应该**再等一会儿**看 `__puus` 会不会落地。
///
/// 两个条件缺一不可：
/// - 凭据不齐（缺 `__puus`）；
/// - `__pus` 出现的时间还没超过 [`QUARK_PUUS_SETTLE_SECONDS`] 的宽限。
///
/// 齐了 ⇒ `false`，立刻存盘；宽限到点还缺 ⇒ 也返回 `false`，
/// 让登录**结束**而不是死等 —— 缺 `__puus` 的会话仍然可以浏览，
/// 下载会拿到 `[auth] 会话已失效，请重新授权`（`412 => Auth`）。
pub fn should_wait_for_puus(session: &QuarkAuthSession, ready_for: Duration) -> bool {
    !session.has_download_credentials() && ready_for.as_secs() < QUARK_PUUS_SETTLE_SECONDS
}

/// 手写的 `Debug`：**只打印长度，绝不打印明文**。
impl fmt::Debug for QuarkAuthSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "QuarkAuthSession({})", self.describe())
    }
}

/// 一个已持有的会话 + 它知道的元信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarkSessionState {
    pub session: QuarkAuthSession,
    /// **上一次验活成功**的时刻（Unix 秒）。`None` 表示从来没验活过
    /// （例如老版本存档，或验活那一步没跑成）。
    pub last_alive_unix: Option<u64>,
}

impl QuarkSessionState {
    pub fn new(session: QuarkAuthSession, last_alive_unix: Option<u64>) -> Self {
        Self {
            session,
            last_alive_unix,
        }
    }
}

/// 会话当前的状态。给设置页看的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarkAuthStatus {
    /// 没存过会话。
    Missing,
    /// 有会话，且「距上次验活」还很新鲜（或压根没验活过）。
    Ready,
    /// 有会话，但距上次验活已超过信任窗口 ⇒ **建议重新登录**。
    ///
    /// ⚠️ 这不是「已经失效」的断言 —— 夸克的 cookie 什么时候作废，本地无从得知。
    /// 真正的死亡证据只有服务端返回的 `code=31001`。
    Stale {
        /// 距上次验活已经过了多少秒。
        elapsed: u64,
    },
    /// 有会话，且信任窗口快到头了。
    StaleSoon {
        /// 还剩多少秒。
        remaining: u64,
    },
}

/// 当前 Unix 秒。
fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// 「该重新验活了」的截止时刻 = 上次验活 + 信任窗口。
///
/// 不知道上次验活时间 ⇒ `None`（乐观认为还能用）。
pub fn trust_deadline_unix(last_alive_unix: Option<u64>) -> Option<u64> {
    last_alive_unix.map(|last_alive| last_alive.saturating_add(QUARK_SESSION_TRUST_SECONDS))
}

/// 判断一个会话是不是已经过了「该重新验活」的窗口。
///
/// 语义**不是**「服务端的 cookie 已作废」，而是「距上次验活太久，状态不可信」。
/// 不知道上次验活时间 ⇒ `false`（乐观）。
pub fn is_expired(last_alive_unix: Option<u64>, now_unix: u64) -> bool {
    matches!(remaining_seconds(last_alive_unix, now_unix), Some(0))
}

/// 距离「该重新验活」还剩多少秒。`None` 表示不知道上次验活时间。
pub fn remaining_seconds(last_alive_unix: Option<u64>, now_unix: u64) -> Option<u64> {
    trust_deadline_unix(last_alive_unix).map(|deadline| deadline.saturating_sub(now_unix))
}

/// 判定会话状态。
///
/// `now_unix` 是**显式参数**而不是内部取当前时间：过期判定必须能在单测里
/// 精确复现，靠真实时钟的话，写出来的测试只是「今天过没过」而已。
pub fn quark_auth_status(state: Option<&QuarkSessionState>, now_unix: u64) -> QuarkAuthStatus {
    let Some(state) = state else {
        return QuarkAuthStatus::Missing;
    };

    let Some(last_alive) = state.last_alive_unix else {
        // 不知道上次验活时间 ⇒ 乐观认为还能用。用户真遇到 31001 时再重新登录。
        return QuarkAuthStatus::Ready;
    };

    let deadline = last_alive.saturating_add(QUARK_SESSION_TRUST_SECONDS);

    if now_unix >= deadline {
        return QuarkAuthStatus::Stale {
            elapsed: now_unix - last_alive,
        };
    }

    let remaining = deadline - now_unix;
    if remaining <= QUARK_LAST_ALIVE_WARN_SECONDS {
        return QuarkAuthStatus::StaleSoon { remaining };
    }

    QuarkAuthStatus::Ready
}

/// 给日志与界面用的**脱敏**描述。
///
/// 只有 cookie 的名字与长度，没有内容 —— 与 [`QuarkAuthSession::describe`]
/// 同一个口径。
pub fn describe_state(state: Option<&QuarkSessionState>) -> String {
    let Some(state) = state else {
        return "无会话".to_string();
    };

    let mut text = state.session.describe();
    if let Some(last_alive) = state.last_alive_unix {
        let elapsed = now_unix().saturating_sub(last_alive);
        text.push_str(&format!(" + 距上次验活{elapsed}秒"));
    }
    text
}

// ===========================================================================
// 落盘
// ===========================================================================

/// `remote-credentials.json` 里那条记录的内部形状。
///
/// **这是密文解密之后的内容**，由 DPAPI 保护；它自己**不是**明文。
///
/// `pus` 是**必需**字段（没有 `#[serde(default)]`）⇒ 一份百度存档
/// （字段是 `bduss`）解不开，会被 [`is_quark_session_blob`] 判成「不是夸克会话」。
#[derive(Serialize, Deserialize)]
struct StoredQuarkSession {
    version: u32,
    pus: String,
    #[serde(default)]
    cookies: BTreeMap<String, String>,
    #[serde(default)]
    last_alive_unix: Option<u64>,
}

/// 手写的 `Debug`：存档里装的就是登录态明文，**只打印长度**。
impl fmt::Debug for StoredQuarkSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "StoredQuarkSession(pus len={}, extras={})",
            self.pus.len(),
            self.cookies.len()
        )
    }
}

/// 把会话编码成可落盘的字符串。
pub fn encode_session(
    session: &QuarkAuthSession,
    last_alive_unix: Option<u64>,
) -> Result<String, RemoteError> {
    let stored = StoredQuarkSession {
        version: QUARK_SESSION_VERSION,
        pus: session.pus().to_string(),
        cookies: session.cookies().clone(),
        last_alive_unix,
    };

    serde_json::to_string(&stored)
        .map_err(|error| RemoteError::unknown(format!("序列化夸克网盘会话失败: {error}")))
}

/// 把落盘的字符串解回会话。
///
/// 版本不认识、JSON 坏了、`__pus` 缺失或非法 ⇒ 一律报 `Auth`：
/// 这几种情形的用户处置都是同一个动作 —— **重新登录**。
/// 报 `Unknown` 只会让用户看到一句看不懂的话。
///
/// ⚠️ **版本不认识 ⇒ 按「没有凭据」处理**（用户侧的处置就是重新登录），
/// **不是** panic：这里返回的是 `Err`，`load_quark_session` 会把它原样传出去，
/// 适配层据此提示「请重新登录」。
pub fn decode_session(raw: &str) -> Result<QuarkSessionState, RemoteError> {
    let stored: StoredQuarkSession = serde_json::from_str(raw)
        .map_err(|_| RemoteError::auth("已保存的夸克网盘会话已损坏，请重新登录获取"))?;

    if stored.version != QUARK_SESSION_VERSION {
        return Err(RemoteError::auth(
            "已保存的夸克网盘会话版本不认识，请重新登录获取",
        ));
    }

    let pus = sanitize_quark_cookie_value(&stored.pus)
        .ok_or_else(|| RemoteError::auth("已保存的夸克网盘会话缺少有效的 __pus，请重新登录获取"))?;

    let mut cookies = BTreeMap::new();
    for (name, value) in &stored.cookies {
        // 存档里混进一条 `__pus` ⇒ Cookie 头会出现两条同名 cookie。
        // 直接丢掉，不给它机会与服务端讨论「哪个才是真的」。
        if name.is_empty() || name.as_str() == QUARK_COOKIE_PUS {
            continue;
        }
        // 非法值只丢这一条，不丢整个会话 —— 少带一个可选 cookie 好过会话作废。
        let Some(value) = sanitize_quark_cookie_value(value) else {
            continue;
        };
        cookies.insert(name.to_string(), value);
    }

    Ok(QuarkSessionState::new(
        QuarkAuthSession::new(pus, cookies),
        stored.last_alive_unix,
    ))
}

/// 把会话还原成一段 cookie 头。
///
/// 用来「拿已保存的会话去测一次连接」—— 那条路径上我们手里只有一个
/// 结构化的会话，没有当初用户粘进来的原始文本。
///
/// 形态与 [`parse_cookie_header`] 吃得下的形态一致（`__pus=...; k=v`），
/// 所以这段文本回到解析器那里能还原出同一个会话。
///
/// ⚠️ 这里**不做二次清洗**：会话里的值在 `scan_cookies` / `decode_session`
/// 两道关口已经过 [`sanitize_quark_cookie_value`]；手工构造出来的会话由调用方负责。
pub fn session_to_cookie_header(session: &QuarkAuthSession) -> String {
    let mut header = format!("{QUARK_COOKIE_PUS}={}", session.pus());

    for (name, value) in session.cookies() {
        header.push_str(&format!("; {name}={value}"));
    }

    header
}

/// 把一段 cookie 头解析成清单。
///
/// 分隔符是 `;`，名字与值之间是**第一个** `=`（cookie 值里允许出现 `=`）。
/// 解析不了的片段直接跳过，不报错 —— 头是用户粘来的，脏一点是常态。
pub fn parse_cookie_header(raw: &str) -> Vec<QuarkCookieRecord> {
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
            Some(QuarkCookieRecord::new(name, value.trim()))
        })
        .collect()
}

/// 加密并保存会话。键就是连接的 `credential_key`。
pub fn save_quark_session(
    app_data_dir: &Path,
    key: &str,
    session: &QuarkAuthSession,
    last_alive_unix: Option<u64>,
) -> Result<(), RemoteError> {
    let encoded = encode_session(session, last_alive_unix)?;
    credentials::save_credential(app_data_dir, key, &encoded)
}

/// 读回会话。没有就是 `Ok(None)`。
///
/// **解不开 / 解出来是坏的 ⇒ 报错**，不悄悄当成「没存」。
/// 悄悄吞掉的话，用户会拿着一句「请先登录」反复重登，而真正的病因是存档坏了。
pub fn load_quark_session(
    app_data_dir: &Path,
    key: &str,
) -> Result<Option<QuarkSessionState>, RemoteError> {
    let Some(encoded) = credentials::load_credential(app_data_dir, key)? else {
        return Ok(None);
    };

    decode_session(&encoded).map(Some)
}

/// 删掉已保存的会话。键不存在是成功的空操作。
pub fn delete_quark_session(app_data_dir: &Path, key: &str) -> Result<(), RemoteError> {
    credentials::delete_credential(app_data_dir, key)
}

/// 这段存档**是不是**夸克网盘会话。
///
/// 用途：拦住「把夸克网盘凭据读回前端」这条路。通用的
/// `remote_load_credential` 会把密文解密后原样返回给前端 —— 对 FTP/SFTP 的
/// 密码来说那是既有行为，对 `__pus` 来说则是把登录态送回前端。
/// 适配层在调 `remote_load_credential` 之前先问一句这个函数。
///
/// 判定靠两件事：`version` 必须认得，且**必须能解出 `pus` 字段**
/// ⇒ 一份百度存档（字段是 `bduss`）解不开，判成「不是夸克会话」。
pub fn is_quark_session_blob(raw: &str) -> bool {
    serde_json::from_str::<StoredQuarkSession>(raw)
        .map(|stored| stored.version == QUARK_SESSION_VERSION)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::error::RemoteErrorKind;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use base64::Engine;
    use tempfile::TempDir;

    const PUS: &str = "the-pus-value-0123456789";
    const EXTRA: &str = "the-extra-value-0123456789";

    /// 一个**像样**的 Unix 时刻（2027-01-15 前后）。
    ///
    /// 状态判定的时间轴必须落在这个量级上：信任窗口是 7 天，用 `1_000` 这种
    /// 小基数去算「距上次验活多久」，减法会下溢、断言会变成在测别的东西。
    const BASE: u64 = 1_800_000_000;

    fn record(name: &str, value: &str) -> QuarkCookieRecord {
        QuarkCookieRecord::new(name, value)
    }

    fn temp_dir() -> TempDir {
        TempDir::new().expect("create temp dir")
    }

    /// 只带 `__pus` 的会话。
    fn plain_session() -> QuarkAuthSession {
        QuarkAuthSession::new(PUS, BTreeMap::new())
    }

    /// `__pus` + 一个补充 cookie 的会话。
    fn session_with_extra() -> QuarkAuthSession {
        let mut cookies = BTreeMap::new();
        cookies.insert("ctoken".to_string(), EXTRA.to_string());
        QuarkAuthSession::new(PUS, cookies)
    }

    const PUUS: &str = "the-puus-value-0123456789";

    /// `__pus` + `__puus` 的会话 —— 也就是**能下载**的完整凭据。
    fn session_with_puus() -> QuarkAuthSession {
        let mut cookies = BTreeMap::new();
        cookies.insert(QUARK_COOKIE_PUUS.to_string(), PUUS.to_string());
        QuarkAuthSession::new(PUS, cookies)
    }

    // ---- `__puus`：下载链的必需 cookie ---------------------------------------
    //
    // 真机缺陷（2026-11-11 用户复测）：新扫码登录后「下载到本地」报
    // `[protocol] 夸克网盘接口拒绝了这次请求（参数或协议层错误）`。
    // 从应用自己的 WebView2 cookie 罐里读实：`__pus`（elen=183）与 `__puus`
    // （elen=311）**同在 `.quark.cn`**，但 `__puus` 比 `__pus` 晚约 1 秒落地。
    // poller 一看见 `__pus` 就验活 + 存盘 ⇒ 存下来的会话**天生缺 `__puus`**，
    // 下载直链被 CDN 以 HTTP 412 拒绝。
    //
    // 旧版把「会话里有没有 `__puus`」当成隐性问题，日志里只留下
    // `__pus(len=120) + extras=16` 这种看不出来的描述。这几条测试把判据固定下来。

    #[test]
    fn pus_is_reported_and_puus_is_read_from_the_extras() {
        assert_eq!(session_with_puus().puus(), Some(PUUS));
        assert_eq!(plain_session().puus(), None);
        assert_eq!(session_with_extra().puus(), None);
    }

    #[test]
    fn download_credentials_need_both_pus_and_puus() {
        assert!(session_with_puus().has_download_credentials());
        assert!(!plain_session().has_download_credentials());
        assert!(!session_with_extra().has_download_credentials());
    }

    #[test]
    fn describe_reports_puus_by_length_only() {
        let described = session_with_puus().describe();
        assert!(described.contains("__puus(len="));
        // 安全铁律：日志里只有长度，绝不出现明文。
        assert!(!described.contains(PUUS));
    }

    #[test]
    fn settle_wait_is_granted_only_inside_the_grace_window() {
        let no_puus = plain_session();
        assert!(should_wait_for_puus(
            &no_puus,
            std::time::Duration::from_secs(0)
        ));
        assert!(should_wait_for_puus(
            &no_puus,
            std::time::Duration::from_secs(QUARK_PUUS_SETTLE_SECONDS - 1)
        ));
        // 宽限到点：到点就存盘，不能无限等（有些账号可能根本不发 `__puus`）。
        assert!(!should_wait_for_puus(
            &no_puus,
            std::time::Duration::from_secs(QUARK_PUUS_SETTLE_SECONDS)
        ));
        // 齐了就不再等。
        assert!(!should_wait_for_puus(
            &session_with_puus(),
            std::time::Duration::from_secs(0)
        ));
    }

    // ---- 扫 cookie 清单 -----------------------------------------------------

    #[test]
    fn an_empty_list_reports_missing() {
        let scan = scan_cookies(&[]);

        assert_eq!(scan.outcome, QuarkCookieOutcome::Missing);
        assert!(scan.session.is_none());
    }

    #[test]
    fn a_list_without_pus_reports_missing() {
        let scan = scan_cookies(&[record("ctoken", EXTRA)]);

        assert_eq!(scan.outcome, QuarkCookieOutcome::Missing);
        assert!(scan.session.is_none());
    }

    #[test]
    fn a_valid_pus_produces_a_usable_session() {
        let scan = scan_cookies(&[record(QUARK_COOKIE_PUS, PUS)]);

        assert_eq!(scan.outcome, QuarkCookieOutcome::Ready);
        let session = scan.session.expect("会话");
        assert_eq!(session.pus(), PUS);
        assert!(session.cookies().is_empty());
        assert!(session.has_credentials());
    }

    /// cookie 名大小写敏感（RFC 6265）⇒ 大小写变体不算 `__pus`。
    #[test]
    fn cookie_names_are_case_sensitive() {
        let scan = scan_cookies(&[record("__PUS", PUS)]);

        assert_eq!(scan.outcome, QuarkCookieOutcome::Missing);
    }

    /// 同名多次取**第一个**：WebView2 的清单顺序不保证稳定。
    #[test]
    fn a_duplicated_pus_takes_the_first_occurrence() {
        let scan = scan_cookies(&[
            record(QUARK_COOKIE_PUS, "first"),
            record(QUARK_COOKIE_PUS, "second"),
        ]);

        let session = scan.session.expect("会话");
        assert_eq!(session.pus(), "first");
    }

    #[test]
    fn a_pus_with_a_semicolon_is_rejected_as_invalid() {
        let scan = scan_cookies(&[record(QUARK_COOKIE_PUS, "abc; injected=1")]);

        assert_eq!(scan.outcome, QuarkCookieOutcome::Invalid);
        assert!(scan.session.is_none());
    }

    #[test]
    fn a_pus_with_whitespace_is_rejected_as_invalid() {
        let scan = scan_cookies(&[record(QUARK_COOKIE_PUS, "abc def")]);

        assert_eq!(scan.outcome, QuarkCookieOutcome::Invalid);
    }

    #[test]
    fn an_empty_pus_is_rejected_as_invalid() {
        let scan = scan_cookies(&[record(QUARK_COOKIE_PUS, "")]);

        assert_eq!(scan.outcome, QuarkCookieOutcome::Invalid);
    }

    /// 补充 cookie 一并留着：夸克到底需要几个 cookie 还没定（§7-U11），
    /// **多带比少带安全**。
    #[test]
    fn the_other_cookies_are_kept_as_extras() {
        let scan = scan_cookies(&[
            record(QUARK_COOKIE_PUS, PUS),
            record("ctoken", EXTRA),
            record("isQuark", "whatever"),
        ]);

        let session = scan.session.expect("会话");
        assert_eq!(session.pus(), PUS);
        assert_eq!(session.cookies().len(), 2);
        assert_eq!(
            session.cookies().get("ctoken").map(String::as_str),
            Some(EXTRA)
        );
    }

    /// 补充 cookie 坏掉**不能**连累 `__pus`：少带一个可选 cookie 好过整个会话作废。
    #[test]
    fn an_invalid_extra_does_not_take_the_session_down_with_it() {
        let scan = scan_cookies(&[
            record(QUARK_COOKIE_PUS, PUS),
            record("ctoken", "bad value"),
        ]);

        assert_eq!(scan.outcome, QuarkCookieOutcome::Ready);
        let session = scan.session.expect("会话");
        assert_eq!(session.pus(), PUS);
        assert!(session.cookies().is_empty());
    }

    #[test]
    fn a_duplicated_extra_takes_the_first_occurrence() {
        let scan = scan_cookies(&[
            record(QUARK_COOKIE_PUS, PUS),
            record("ctoken", "first"),
            record("ctoken", "second"),
        ]);

        let session = scan.session.expect("会话");
        assert_eq!(
            session.cookies().get("ctoken").map(String::as_str),
            Some("first")
        );
    }

    /// `__pus` 不许同时出现在 extras 里 —— 否则 Cookie 头会有两条同名 cookie。
    #[test]
    fn the_pus_cookie_never_appears_in_the_extras() {
        let scan = scan_cookies(&[
            record(QUARK_COOKIE_PUS, PUS),
            record(QUARK_COOKIE_PUS, "another"),
        ]);

        let session = scan.session.expect("会话");
        assert_eq!(session.pus(), PUS);
        assert!(session.cookies().is_empty());
    }

    /// 脱敏断言：cookie 记录的 `Debug` 里只有名字与长度，**没有值**。
    #[test]
    fn the_cookie_record_debug_never_leaks_the_value() {
        let text = format!("{:?}", record(QUARK_COOKIE_PUS, PUS));

        assert!(!text.contains(PUS));
        assert!(text.contains(QUARK_COOKIE_PUS));
        assert!(text.contains("len="));
    }

    // ---- 错误文案 -----------------------------------------------------------

    #[test]
    fn the_missing_credential_error_is_classified_as_auth() {
        let scan = scan_cookies(&[]);

        let error = scan_error(&scan).expect("应有错误");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[test]
    fn the_invalid_credential_error_is_classified_as_auth() {
        let scan = scan_cookies(&[record(QUARK_COOKIE_PUS, "abc; injected=1")]);

        let error = scan_error(&scan).expect("应有错误");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    /// 错误文案里**绝不能**出现 cookie 内容。
    #[test]
    fn the_error_message_never_contains_the_credential() {
        let scan = scan_cookies(&[record(QUARK_COOKIE_PUS, "abc; injected=1")]);

        let error = scan_error(&scan).expect("应有错误");

        assert!(!error.message.contains("injected"));
        assert!(!error.message.contains("abc"));
    }

    #[test]
    fn a_ready_scan_has_no_error() {
        let scan = scan_cookies(&[record(QUARK_COOKIE_PUS, PUS)]);

        assert!(scan_error(&scan).is_none());
    }

    // ---- 脱敏 ---------------------------------------------------------------

    /// 脱敏断言：描述里只有名字与长度，**没有内容**。
    #[test]
    fn the_description_never_leaks_the_credential() {
        let state = QuarkSessionState::new(session_with_extra(), Some(now_unix() + 3_600));

        let text = describe_state(Some(&state));

        assert!(!text.contains(PUS));
        assert!(!text.contains(EXTRA));
        assert!(text.contains(QUARK_COOKIE_PUS));
        assert!(text.contains("len="));
    }

    #[test]
    fn the_description_of_a_missing_session_is_explicit() {
        assert_eq!(describe_state(None), "无会话");
    }

    #[test]
    fn the_debug_output_never_leaks_the_credential() {
        let text = format!("{:?}", session_with_extra());

        assert!(!text.contains(PUS));
        assert!(!text.contains(EXTRA));
    }

    /// 会话状态整体进 `Debug`（日志里会打整个 state）也不能漏明文。
    #[test]
    fn the_state_debug_never_leaks_the_credential() {
        let state = QuarkSessionState::new(session_with_extra(), Some(1_800_000_000));

        let text = format!("{state:?}");

        assert!(!text.contains(PUS));
        assert!(!text.contains(EXTRA));
    }

    // ---- 状态判定 -----------------------------------------------------------

    #[test]
    fn a_missing_session_reads_as_missing() {
        assert_eq!(quark_auth_status(None, BASE), QuarkAuthStatus::Missing);
    }

    #[test]
    fn a_session_without_a_last_alive_reads_as_ready() {
        let state = QuarkSessionState::new(plain_session(), None);

        assert_eq!(quark_auth_status(Some(&state), BASE), QuarkAuthStatus::Ready);
    }

    #[test]
    fn a_freshly_verified_session_reads_as_ready() {
        let state = QuarkSessionState::new(plain_session(), Some(BASE));

        assert_eq!(quark_auth_status(Some(&state), BASE), QuarkAuthStatus::Ready);
    }

    /// 距信任窗口结束只剩 60 秒 ⇒ 「临期」。
    ///
    /// 上次验活的时刻 = `BASE - 信任窗口 + 60`：也就是说**七天前**验过一次活，
    /// 马上就要到头了。
    #[test]
    fn a_session_close_to_the_trust_deadline_reads_as_stale_soon() {
        let state = QuarkSessionState::new(
            plain_session(),
            Some(BASE - QUARK_SESSION_TRUST_SECONDS + 60),
        );

        assert_eq!(
            quark_auth_status(Some(&state), BASE),
            QuarkAuthStatus::StaleSoon { remaining: 60 }
        );
    }

    #[test]
    fn a_session_past_the_trust_deadline_reads_as_stale() {
        let state = QuarkSessionState::new(
            plain_session(),
            Some(BASE - QUARK_SESSION_TRUST_SECONDS),
        );

        assert_eq!(
            quark_auth_status(Some(&state), BASE),
            QuarkAuthStatus::Stale {
                elapsed: QUARK_SESSION_TRUST_SECONDS
            }
        );
    }

    /// 截止时刻**正好等于**当前时刻 ⇒ 已过窗口。
    /// 差一秒的边界在真实时钟下极难复现，所以这里显式钉死。
    #[test]
    fn a_session_expiring_exactly_now_counts_as_stale() {
        let deadline = BASE;
        let state = QuarkSessionState::new(
            plain_session(),
            Some(BASE - QUARK_SESSION_TRUST_SECONDS),
        );

        assert_eq!(
            quark_auth_status(Some(&state), deadline),
            QuarkAuthStatus::Stale {
                elapsed: QUARK_SESSION_TRUST_SECONDS
            }
        );
    }

    #[test]
    fn remaining_seconds_saturates_at_zero() {
        let last_alive = BASE - QUARK_SESSION_TRUST_SECONDS;

        assert_eq!(remaining_seconds(Some(last_alive), BASE), Some(0));
        assert_eq!(remaining_seconds(Some(last_alive), BASE + 10_000), Some(0));
        assert_eq!(remaining_seconds(Some(BASE), BASE), Some(QUARK_SESSION_TRUST_SECONDS));
        assert_eq!(remaining_seconds(None, BASE), None);
    }

    #[test]
    fn the_trust_deadline_is_the_last_alive_time_plus_the_trust_window() {
        assert_eq!(
            trust_deadline_unix(Some(BASE)),
            Some(BASE + QUARK_SESSION_TRUST_SECONDS)
        );
        assert_eq!(trust_deadline_unix(None), None);
    }

    #[test]
    fn the_expiry_helper_agrees_with_the_status_function() {
        let last_alive = BASE - QUARK_SESSION_TRUST_SECONDS;

        assert!(is_expired(Some(last_alive), BASE));
        assert!(!is_expired(Some(last_alive), BASE - 1));
        assert!(!is_expired(None, BASE));
    }

    // ---- 编码 / 解码 -------------------------------------------------------

    #[test]
    fn encoding_and_decoding_round_trip() {
        let encoded = encode_session(&session_with_extra(), Some(1_800_000_000)).expect("编码");
        let decoded = decode_session(&encoded).expect("解码");

        assert_eq!(decoded.session.pus(), PUS);
        assert_eq!(decoded.session.cookies().len(), 1);
        assert_eq!(
            decoded.session.cookies().get("ctoken").map(String::as_str),
            Some(EXTRA)
        );
        assert_eq!(decoded.last_alive_unix, Some(1_800_000_000));
    }

    #[test]
    fn decoding_survives_a_session_without_extras() {
        let encoded = encode_session(&plain_session(), None).expect("编码");
        let decoded = decode_session(&encoded).expect("解码");

        assert_eq!(decoded.session.pus(), PUS);
        assert!(decoded.session.cookies().is_empty());
        assert!(decoded.last_alive_unix.is_none());
    }

    #[test]
    fn decoding_garbage_is_classified_as_auth() {
        let error = decode_session("not json at all").expect_err("坏存档必须被拒");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    /// 版本不认识 ⇒ **按「没有凭据」处理**（用户的处置是重新登录），
    /// **不是 panic**。
    #[test]
    fn decoding_an_unknown_version_is_rejected_without_panicking() {
        let raw = r#"{"version":999,"pus":"abc"}"#;

        let error = decode_session(raw).expect_err("版本不认识必须被拒");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[test]
    fn decoding_a_blob_without_pus_is_rejected() {
        let raw = r#"{"version":1,"cookies":{}}"#;

        let error = decode_session(raw).expect_err("缺 __pus 必须被拒");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    /// 存档里带一个**非法**的 `__pus` ⇒ 整个会话作废（`__pus` 是必需的）。
    #[test]
    fn decoding_a_blob_with_an_invalid_pus_is_rejected() {
        let raw = r#"{"version":1,"pus":"abc; injected=1"}"#;

        let error = decode_session(raw).expect_err("非法 __pus 必须被拒");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    /// 存档里带一个**非法**的补充 cookie ⇒ 只丢那一条，会话仍在。
    #[test]
    fn decoding_a_blob_with_an_invalid_extra_keeps_the_session() {
        let raw = r#"{"version":1,"pus":"abc","cookies":{"ctoken":"bad value"}}"#;

        let decoded = decode_session(raw).expect("应当解出 __pus");

        assert_eq!(decoded.session.pus(), "abc");
        assert!(decoded.session.cookies().is_empty());
    }

    /// 百度存档（字段是 `bduss`）解不开 ⇒ 被拒。
    #[test]
    fn decoding_a_baidu_session_blob_is_rejected() {
        let raw = r#"{"version":1,"bduss":"abc"}"#;

        let error = decode_session(raw).expect_err("百度存档必须被拒");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    /// 端到端的那一条：凭据库里躺着**版本不认识**的存档 ⇒ 读出来是
    /// 「请重新登录」，**不是 panic**，也不是「静默当成没存过」。
    #[test]
    fn loading_a_blob_of_an_unknown_version_asks_for_a_new_login() {
        let dir = temp_dir();
        credentials::save_credential(
            dir.path(),
            "conn-1",
            r#"{"version":999,"pus":"the-pus-value-0123456789"}"#,
        )
        .expect("保存");

        let error = load_quark_session(dir.path(), "conn-1").expect_err("版本不认识必须被拒");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[test]
    fn the_encoded_blob_carries_the_version() {
        let encoded = encode_session(&plain_session(), None).expect("编码");

        assert!(encoded.contains(&format!("\"version\":{QUARK_SESSION_VERSION}")));
    }

    // ---- 落盘往返 -----------------------------------------------------------

    #[test]
    fn saving_and_loading_round_trips_through_the_credential_file() {
        let dir = temp_dir();

        save_quark_session(dir.path(), "conn-1", &session_with_extra(), Some(1_800_000_000))
            .expect("保存");
        let loaded = load_quark_session(dir.path(), "conn-1")
            .expect("读取")
            .expect("应当有会话");

        assert_eq!(loaded.session.pus(), PUS);
        assert_eq!(
            loaded.session.cookies().get("ctoken").map(String::as_str),
            Some(EXTRA)
        );
        assert_eq!(loaded.last_alive_unix, Some(1_800_000_000));
    }

    /// 阳性对照：落盘的**确实是 DPAPI 密文**，明文与它的 base64 都不在文件里。
    ///
    /// 断言扫的是**整个文件**，不是某一个字段。
    #[test]
    fn the_credential_file_never_contains_the_plaintext_session() {
        let dir = temp_dir();

        save_quark_session(dir.path(), "conn-1", &session_with_extra(), None).expect("保存");

        let raw = std::fs::read(credentials::credentials_path_in(dir.path())).expect("读原始文件");
        let text = String::from_utf8_lossy(&raw);

        assert!(!text.contains(PUS), "凭据文件里绝不能出现 cookie 明文");
        assert!(!text.contains(EXTRA), "凭据文件里绝不能出现 cookie 明文");
        assert!(
            !text.contains(&BASE64_STANDARD.encode(PUS)),
            "凭据文件里绝不能出现 cookie 的 base64 —— 那等于明文落盘换个写法"
        );
    }

    #[test]
    fn loading_an_unknown_key_returns_none() {
        let dir = temp_dir();

        assert!(load_quark_session(dir.path(), "conn-absent")
            .expect("读取")
            .is_none());
    }

    #[test]
    fn saving_again_replaces_the_previous_session() {
        let dir = temp_dir();

        save_quark_session(dir.path(), "conn-1", &plain_session(), None).expect("第一次保存");
        save_quark_session(dir.path(), "conn-1", &QuarkAuthSession::new("new", BTreeMap::new()), None)
            .expect("第二次保存");

        let loaded = load_quark_session(dir.path(), "conn-1")
            .expect("读取")
            .expect("应当有会话");
        assert_eq!(loaded.session.pus(), "new");
    }

    #[test]
    fn deleting_removes_the_session() {
        let dir = temp_dir();
        save_quark_session(dir.path(), "conn-1", &plain_session(), None).expect("保存");

        delete_quark_session(dir.path(), "conn-1").expect("删除");

        assert!(load_quark_session(dir.path(), "conn-1")
            .expect("读取")
            .is_none());
    }

    #[test]
    fn deleting_an_unknown_key_is_a_no_op() {
        let dir = temp_dir();

        delete_quark_session(dir.path(), "conn-absent").expect("删除不存在的键");
    }

    /// 夸克会话与既有凭据**共存于同一个凭据库**：键 = 连接 id，
    /// 存一条不能把别的挤掉。
    #[test]
    fn saving_a_quark_session_keeps_the_other_credentials() {
        let dir = temp_dir();
        credentials::save_credential(dir.path(), "conn-baidu", "baidu-secret").expect("保存");

        save_quark_session(dir.path(), "conn-quark", &plain_session(), None).expect("保存");

        assert_eq!(
            credentials::load_credential(dir.path(), "conn-baidu")
                .expect("读取")
                .as_deref(),
            Some("baidu-secret")
        );
        assert!(load_quark_session(dir.path(), "conn-quark")
            .expect("读取")
            .is_some());
    }

    // ---- 存档识别 -----------------------------------------------------------

    #[test]
    fn a_quark_blob_is_recognized() {
        let encoded = encode_session(&plain_session(), None).expect("编码");

        assert!(is_quark_session_blob(&encoded));
    }

    /// 阴性对照：普通密码存档**不能**被误认成夸克网盘会话 ——
    /// 否则 `remote_load_credential` 的拦截会把所有 FTP/SFTP 密码都拦掉。
    #[test]
    fn an_ordinary_password_is_not_mistaken_for_a_quark_session() {
        assert!(!is_quark_session_blob("just a password"));
        assert!(!is_quark_session_blob(""));
        assert!(!is_quark_session_blob(r#"{"version":2,"pus":"x"}"#));
    }

    /// 反向也成立：百度会话不能被夸克的识别函数认走。
    #[test]
    fn a_baidu_session_blob_is_not_mistaken_for_a_quark_session() {
        assert!(!is_quark_session_blob(r#"{"version":1,"bduss":"x"}"#));
    }

    // ---- Cookie 头 ----------------------------------------------------------

    #[test]
    fn the_cookie_header_carries_the_pus_cookie() {
        let header = session_to_cookie_header(&plain_session());

        assert_eq!(header, format!("{QUARK_COOKIE_PUS}={PUS}"));
    }

    #[test]
    fn the_cookie_header_carries_the_extra_cookies() {
        let header = session_to_cookie_header(&session_with_extra());

        assert!(header.contains(&format!("{QUARK_COOKIE_PUS}={PUS}")));
        assert!(header.contains(&format!("ctoken={EXTRA}")));
    }

    /// 拼出来的头要能回到解析器那里还原出同一个会话 ——
    /// 「拿已保存的会话去测一次连接」那条路径就靠这个往返。
    #[test]
    fn the_cookie_header_round_trips_through_the_parser() {
        let original = session_with_extra();

        let header = session_to_cookie_header(&original);
        let records = parse_cookie_header(&header);
        let scan = scan_cookies(&records);

        assert_eq!(scan.outcome, QuarkCookieOutcome::Ready);
        assert_eq!(scan.session, Some(original));
    }
}
