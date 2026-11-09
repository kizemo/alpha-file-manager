// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// ---------------------------------------------------------------------------
// 百度网盘 Web API 的纯函数层
//
// 红线（与 `remote/mod.rs:8-18` 同一条）：本文件被 `cargo test --lib` 的测试
// 二进制链接，**不得出现 `tauri::AppHandle`、`tauri::async_runtime::*` 或
// `tauri::` 的任何路径**。一旦碰到，Tauri 全局运行时的默认类型参数 `Wry`
// 被单态化，把 tao 整条窗口链拖进测试二进制；测试二进制没有 application
// manifest 声明 comctl32 v6，`TaskDialogIndirect` 解析不到，进程在加载阶段
// 以 `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉 —— 一个测试都跑不了，
// 且报错与本模块毫无关系。
//
// 本文件里没有任何 UI 逻辑，也没有凭据落盘逻辑：
//   - URL 拼装、分页计算、响应解析、错误分类全部是纯函数，可离线单测；
//   - HTTP 走既有的 `reqwest`（0.13，`default-features=false`，只开 `rustls` +
//     `stream`）：**不开 `json` / `cookies` / `charset` feature** —— JSON 用
//     `serde_json` 解，Cookie 头手工拼，响应体用 `bytes()` + `String::from_utf8`
//     解（顺带把「响应不是合法 UTF-8」变成一个可分类的 `Encoding` 错误）。
//
// 接口来源说明（**不是官方开放平台**）：官方开放平台要求企业实名，且第三方应用
// 只能访问 `/apps/{应用名}/` 目录，与「浏览用户整个网盘」根本冲突，已在
// `docs/2026-11-07-baidu-netdisk-drive-feasibility.md` §1 坐实并否决。本文件走
// 的是网页端 REST 接口，形态与社区通行实现一致；**实证依据见
// `docs/2026-11-07-baidu-t1-auth-findings.md`**：
//   - `/rest/2.0/xpan/nas?method=uinfo` → 200 `errno:0`（拿到昵称）
//   - `/rest/2.0/xpan/file?method=list` → 200 `errno:0`（字段 `fs_id` / `path` /
//     `isdir` / `size` / `server_mtime`）
//   - `/rest/2.0/xpan/quota` → 400 `Unsupported open api` ⇒ **不实现配额**
//   - `/api/*` 需要 `pan.baidu.com` 域下发的 `STOKEN`，本轮**不作主路径**
//
// 未实证的部分在本文件里逐处标注「未实证」；调用方不得把它们当成已证结论。
// ---------------------------------------------------------------------------

use crate::remote::error::{RemoteError, RemoteErrorKind};
use crate::remote::path::build_remote_path;
use crate::remote::provider::RemoteEntry;
use crate::remote::sftp_provider::is_hidden_remote_name;
use futures_util::StreamExt;
use serde::Deserialize;
use std::fmt;
use std::time::Duration;

/// 百度网盘 Web 端的主机名。
pub const BAIDU_PAN_HOST: &str = "pan.baidu.com";

/// 百度网盘 Web 端的根地址（全部接口都是 HTTPS）。
pub const BAIDU_PAN_BASE_URL: &str = "https://pan.baidu.com";

/// 网页端内部的 `app_id`。
///
/// ⚠️ **它不是 OAuth `client_id`**：T1 实测拿 `250528` / `266719` 去换
/// `refresh_token`，服务端回 `unknown client id`，与阴性对照逐字相同
/// （`docs/2026-11-07-baidu-t1-auth-findings.md` §3.3）。
/// 它只是网页端接口的一个 query 参数，换 `access_token` 那条路已被整条废弃。
pub const BAIDU_PAN_APP_ID: &str = "250528";

/// 请求 `User-Agent`。
///
/// 百度网盘网页端自己用的是 `netdisk;P2SP;...` 这一族标识；下载直链对 UA
/// 有要求，社区通行做法是带上它。**未实证**：Rust 侧手工拼的 UA 与页面内
/// fetch 是否等价，留给 T4 真机验。
pub const BAIDU_PAN_USER_AGENT: &str = "netdisk;P2SP;2.2.60.26";

/// 列表分页的默认页大小，同时是服务端的**上限**（超过会被截断）。
///
/// 超过 1000 项的目录必须翻页，这是阶段 1 必须处理的情况（plan §7 真机第 5 条）。
pub const BAIDU_LIST_PAGE_LIMIT: u32 = 1000;

/// 单次 HTTP 请求的超时。
pub const BAIDU_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

/// 建立下载连接的超时。
///
/// 与请求超时分开：连不上要**立刻**放弃，而下载慢是正常的。
pub const BAIDU_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// 下载一个文件的整体超时。
///
/// 比 [`BAIDU_OPERATION_TIMEOUT`] 宽得多：整份文件要进内存，30 秒的请求
/// 超时对几百 MiB 的文件是必然超时的。
pub const BAIDU_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(900);

/// 会话 cookie 名：账号登录态，等价于密码。
pub const BAIDU_COOKIE_BDUSS: &str = "BDUSS";

/// 会话 cookie 名：网盘网页端下发的短时令牌，`/api/*` 端点依赖它。
pub const BAIDU_COOKIE_STOKEN: &str = "STOKEN";

// ===========================================================================
// 会话
// ===========================================================================

/// 百度网盘的会话凭据。
///
/// **安全等级等同于密码**：`BDUSS` 就是百度账号的登录态本身，拿到它就能冒充
/// 用户。写盘（走 DPAPI 密文档）、日志、错误信息里都不得出现明文 —— 因此
/// 这里手写了 `Debug`（不 derive），只打印长度不打印内容。
///
/// 持久化对象是 `BDUSS` + `STOKEN` 两个，理由见 T1 记录 §5.3：页面内 `fetch`
/// 带的是整个 cookie jar，**无法隔离出各端点究竟依赖哪一个 cookie**，
/// 「只发 `BDUSS` 够不够」属未证结论 ⇒ 两者都留着。
#[derive(Clone, PartialEq, Eq)]
pub struct BaiduSession {
    bduss: String,
    stoken: Option<String>,
}

impl BaiduSession {
    /// 新建一个会话。`stoken` 为 `None` 表示只拿到 `BDUSS`。
    pub fn new(bduss: impl Into<String>, stoken: Option<String>) -> Self {
        Self {
            bduss: bduss.into(),
            stoken,
        }
    }

    /// `BDUSS` 的原值。**只在拼 Cookie 头时用**，不要拿去打日志。
    pub fn bduss(&self) -> &str {
        &self.bduss
    }

    /// `STOKEN` 的原值（可能没有）。
    pub fn stoken(&self) -> Option<&str> {
        self.stoken.as_deref()
    }

    /// 是否至少有一个可用的 cookie。
    pub fn has_credentials(&self) -> bool {
        !self.bduss.trim().is_empty()
    }

    /// 拼出 `Cookie:` 头的值。
    ///
    /// `BDUSS` 缺失或格式非法 ⇒ `None`（调用方据此报「会话失效/未授权」）。
    /// `STOKEN` 非法时**只丢弃 STOKEN**，不影响 `BDUSS` —— 少带一个可选 cookie
    /// 好过整个请求失败。
    pub fn cookie_header(&self) -> Option<String> {
        let bduss = sanitize_cookie_value(&self.bduss)?;

        let mut header = format!("{BAIDU_COOKIE_BDUSS}={bduss}");
        if let Some(stoken) = self.stoken.as_deref().and_then(sanitize_cookie_value) {
            header.push_str(&format!("; {BAIDU_COOKIE_STOKEN}={stoken}"));
        }

        Some(header)
    }

    /// 给日志用的**脱敏**描述：只有名字与长度，没有内容。
    pub fn describe(&self) -> String {
        let mut parts = vec![format!("{BAIDU_COOKIE_BDUSS}(len={})", self.bduss.len())];
        if let Some(stoken) = self.stoken.as_ref() {
            parts.push(format!("{BAIDU_COOKIE_STOKEN}(len={})", stoken.len()));
        }
        parts.join(" + ")
    }
}

/// 手写的 `Debug`：**只打印长度，绝不打印明文**。
impl fmt::Debug for BaiduSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "BaiduSession({})", self.describe())
    }
}

/// 校验并清洗一个 cookie 的值。
///
/// RFC 6265 的 cookie-value 不允许 CTLs、空白、`"`、`,`、`;`、`\`。这几个字符
/// 出现在会话 cookie 里只有两种可能：**值被截断了**或者**有人想注入头部**
/// —— 两种都不能原样发出去。所以这里不是「过滤」而是**判非法**：含这些字符
/// 就返回 `None`，宁可让调用方报「会话失效」，也不发一个语义不明的头。
pub fn sanitize_cookie_value(raw: &str) -> Option<String> {
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

// ===========================================================================
// URL 拼装
// ===========================================================================

/// 百分号编码一个 query 组件。
///
/// 只保留 `A-Z` / `a-z` / `0-9` / `-` / `_` / `.` / `~`；**斜杠也会被编码**
/// （`/` → `%2F`），因为网盘路径是 query 值的一部分而不是 URL 路径。
pub fn encode_query_component(raw: &str) -> String {
    urlencoding::encode(raw).into_owned()
}

/// 列目录：`/rest/2.0/xpan/file?method=list`。
///
/// 参数顺序与 T1 探针实测通过的那条请求**逐字一致**（`docs/2026-11-07-baidu-t1-auth-findings.md`
/// §5.1），避免「参数顺序不同服务端就换一套行为」这种玄学问题。
pub fn list_dir_url(dir: &str, start: u32, limit: u32) -> String {
    format!(
        "{}/rest/2.0/xpan/file?method=list&dir={}&order=name&start={}&limit={}&web=1&app_id={}",
        BAIDU_PAN_BASE_URL,
        encode_query_component(dir),
        start,
        clamp_page_limit(limit),
        BAIDU_PAN_APP_ID
    )
}

/// 取账号信息：`/rest/2.0/xpan/nas?method=uinfo`。
///
/// T1 实测 200 `errno:0`，返回 `baidu_name` / `uk` / `vip_type` / `avatar_url`。
pub fn uinfo_url() -> String {
    format!(
        "{}/rest/2.0/xpan/nas?method=uinfo&app_id={}&web=1",
        BAIDU_PAN_BASE_URL, BAIDU_PAN_APP_ID
    )
}

/// 取文件元信息（含下载直链）：`/rest/2.0/xpan/multimedia?method=filemetas`。
///
/// ⚠️ **未实证**：T1 两轮只验了只读接口，没打过这条。端点形态与社区通行实现
/// 一致，T4 真机必须验。`fs_ids` 是一个 JSON 数组，因此整体要编码。
pub fn filemetas_url(fs_ids: &[u64]) -> String {
    let ids = fs_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "{}/rest/2.0/xpan/multimedia?method=filemetas&fsids={}&dlink=1&web=1&app_id={}",
        BAIDU_PAN_BASE_URL,
        encode_query_component(&format!("[{ids}]")),
        BAIDU_PAN_APP_ID
    )
}

/// 取下载直链：`/api/download`。
///
/// ⚠️ **未实证**：`sign` / `timestamp` 来自网盘网页端页面里的临时签名，
/// 获取方式待 T4 真机确认（社区通行做法是从页面配置里取）。
pub fn dlink_url(fs_id: u64, sign: &str, timestamp: u64) -> String {
    format!(
        "{}/api/download?sign={}&timestamp={}&fs_id={}&dlink=1&web=1&app_id={}&channel=chunlei&clienttype=0",
        BAIDU_PAN_BASE_URL,
        encode_query_component(sign),
        timestamp,
        fs_id,
        BAIDU_PAN_APP_ID
    )
}

// ===========================================================================
// 分页
// ===========================================================================

/// 一页列表的取数范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListPage {
    /// 起始偏移（从 0 开始）。
    pub start: u32,
    /// 本页最多取多少条。
    pub limit: u32,
}

/// 把请求的页大小夹到服务端允许的范围。
///
/// 0 ⇒ 用默认页大小（同时也是上限 1000）；超过 1000 ⇒ 截到 1000。
///
/// 为什么要夹：服务端会**静默截断**超限的 `limit`，返回 1000 条却给出
/// `errno:0`。不夹的话分页会漏条目 —— 目录里第 1001 条之后的内容永远看不到，
/// 而且**不会报错**。
pub fn clamp_page_limit(limit: u32) -> u32 {
    if limit == 0 {
        return BAIDU_LIST_PAGE_LIMIT;
    }
    limit.min(BAIDU_LIST_PAGE_LIMIT)
}

/// 算下一页。返回 `None` 表示**已经取完**。
///
/// 判据是「本页收到的条数 < limit」⇒ 服务端没有更多了。
///
/// ⚠️ 边界：恰好收满 `limit` 条时会**多发一次**请求（那一页会是空的）。
/// 这是刻意的 —— 接口没有 `has_more` 字段，少一条的目录与正好整页的目录
/// 无法从返回值上区分，硬猜会漏条目。
pub fn next_page(page: &ListPage, received: usize) -> Option<ListPage> {
    if received < page.limit as usize {
        return None;
    }
    if received == 0 {
        return None;
    }

    let next_start = page.start.checked_add(page.limit)?;
    Some(ListPage {
        start: next_start,
        limit: page.limit,
    })
}

// ===========================================================================
// 响应结构
// ===========================================================================

/// 列表里的一个条目（xpan 形态）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BaiduFileEntry {
    /// 文件 id。后续取直链要它。
    #[serde(default)]
    pub fs_id: u64,
    /// 网盘里的完整路径，形如 `/01A_软件`。
    #[serde(default)]
    pub path: String,
    /// 文件名（不含目录）。
    #[serde(default)]
    pub server_filename: String,
    /// 1 = 目录，0 = 文件。
    #[serde(default)]
    pub isdir: i64,
    /// 字节数。
    #[serde(default)]
    pub size: u64,
    /// 服务器修改时间，**单位是秒**。
    #[serde(default)]
    pub server_mtime: u64,
    /// 本地（客户端）修改时间，**单位是秒**。
    #[serde(default)]
    pub local_mtime: u64,
}

/// 账号信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaiduUserInfo {
    pub baidu_name: String,
    pub uk: u64,
    pub vip_type: i64,
    pub avatar_url: Option<String>,
}

/// 列表响应外壳。
#[derive(Debug, Clone, Deserialize)]
struct BaiduListEnvelope {
    #[serde(default)]
    errno: Option<i64>,
    #[serde(default)]
    errmsg: Option<String>,
    #[serde(default)]
    list: Vec<BaiduFileEntry>,
}

/// 账号信息响应外壳。
#[derive(Debug, Clone, Deserialize)]
struct BaiduUinfoEnvelope {
    #[serde(default)]
    errno: Option<i64>,
    #[serde(default)]
    errmsg: Option<String>,
    #[serde(default)]
    baidu_name: Option<String>,
    #[serde(default)]
    uk: Option<u64>,
    #[serde(default)]
    vip_type: Option<i64>,
    #[serde(default)]
    avatar_url: Option<String>,
}

/// 下载直链响应外壳。
#[derive(Debug, Clone, Deserialize)]
struct BaiduDlinkEnvelope {
    #[serde(default)]
    errno: Option<i64>,
    #[serde(default)]
    errmsg: Option<String>,
    #[serde(default)]
    dlink: Option<String>,
    #[serde(default)]
    fs_id: Option<u64>,
}

// ===========================================================================
// 响应解析（纯函数）
// ===========================================================================

/// 解析列表响应。
///
/// `errno != 0` ⇒ 走 [`failure_from_errno`]；响应不是合法 JSON 或缺 `errno`
/// ⇒ [`BaiduFailure::Malformed`]。
pub fn parse_list_response(body: &str) -> Result<Vec<BaiduFileEntry>, BaiduFailure> {
    let envelope: BaiduListEnvelope =
        serde_json::from_str(body).map_err(|_| BaiduFailure::Malformed)?;

    let errno = envelope.errno.ok_or(BaiduFailure::Malformed)?;
    if errno != 0 {
        return Err(failure_from_errno(errno, envelope.errmsg.as_deref()));
    }

    Ok(envelope.list)
}

/// 解析账号信息响应。
pub fn parse_uinfo_response(body: &str) -> Result<BaiduUserInfo, BaiduFailure> {
    let envelope: BaiduUinfoEnvelope =
        serde_json::from_str(body).map_err(|_| BaiduFailure::Malformed)?;

    let errno = envelope.errno.ok_or(BaiduFailure::Malformed)?;
    if errno != 0 {
        return Err(failure_from_errno(errno, envelope.errmsg.as_deref()));
    }

    Ok(BaiduUserInfo {
        baidu_name: envelope.baidu_name.unwrap_or_default(),
        uk: envelope.uk.unwrap_or_default(),
        vip_type: envelope.vip_type.unwrap_or_default(),
        avatar_url: envelope.avatar_url,
    })
}

/// 解析下载直链响应。
pub fn parse_dlink_response(body: &str) -> Result<String, BaiduFailure> {
    let envelope: BaiduDlinkEnvelope =
        serde_json::from_str(body).map_err(|_| BaiduFailure::Malformed)?;

    let errno = envelope.errno.ok_or(BaiduFailure::Malformed)?;
    if errno != 0 {
        return Err(failure_from_errno(errno, envelope.errmsg.as_deref()));
    }

    envelope
        .dlink
        .filter(|link| !link.is_empty())
        .ok_or(BaiduFailure::Malformed)
}

/// `filemetas` 响应里的一个下载直链。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BaiduDlink {
    /// 文件 id。某些响应里会缺，省略时是 0。
    #[serde(default)]
    pub fs_id: u64,
    /// 直链本身。空串表示「这条没有直链」。
    #[serde(default)]
    pub dlink: String,
}

/// `filemetas` 响应外壳。
#[derive(Debug, Clone, Deserialize)]
struct BaiduFilemetasEnvelope {
    #[serde(default)]
    errno: Option<i64>,
    #[serde(default)]
    errmsg: Option<String>,
    #[serde(default)]
    list: Vec<BaiduDlink>,
}

/// 解析 `filemetas` 响应（取下载直链）。
///
/// ⚠️ **未实证**：T1 两轮只验了只读接口，没打过这条（见
/// `docs/2026-11-07-baidu-t1-auth-findings.md`）。形状按社区通行实现写，
/// T4 真机必须验。
pub fn parse_filemetas_response(body: &str) -> Result<Vec<BaiduDlink>, BaiduFailure> {
    let envelope: BaiduFilemetasEnvelope =
        serde_json::from_str(body).map_err(|_| BaiduFailure::Malformed)?;

    let errno = envelope.errno.ok_or(BaiduFailure::Malformed)?;
    if errno != 0 {
        return Err(failure_from_errno(errno, envelope.errmsg.as_deref()));
    }

    Ok(envelope.list)
}

// ===========================================================================
// 条目 → RemoteEntry
// ===========================================================================

/// 秒 → Unix 纪元毫秒。
///
/// 百度返回的 `server_mtime` 是**秒**，而 `RemoteEntry.modified_time` 的约定是
/// **Unix 纪元毫秒**（见 `provider.rs` 的字段注释）。忘了 ×1000 的后果是
/// 「按修改时间排序」在远程目录下变成一片随机顺序 —— 而且**不会报错**。
///
/// 与 `sftp_provider::sftp_seconds_to_unix_millis` 同一件事，只是来源不同
/// （那边是 SFTP 的 `mtime`，这边是 xpan 的 `server_mtime`）。
pub fn baidu_seconds_to_unix_millis(seconds: u64) -> u64 {
    seconds.saturating_mul(1_000)
}

/// 条目的名字。`server_filename` 为空时退回路径的末段。
pub fn entry_name(entry: &BaiduFileEntry) -> Option<String> {
    if !entry.server_filename.trim().is_empty() {
        return Some(entry.server_filename.clone());
    }

    let leaf = entry
        .path
        .rsplit('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or_default();

    if leaf.is_empty() {
        None
    } else {
        Some(leaf.to_string())
    }
}

/// 把一个 xpan 条目映射成本项目的 `RemoteEntry`。
///
/// 返回 `None` 表示这条数据没法用（名字取不到、或路径构造不出来）——
/// **跳过它，而不是造一条半截数据**。
pub fn to_remote_entry(connection_id: &str, entry: &BaiduFileEntry) -> Option<RemoteEntry> {
    let name = entry_name(entry)?;
    let path = build_remote_path(connection_id, &entry.path)?;

    let modified_seconds = if entry.server_mtime > 0 {
        entry.server_mtime
    } else {
        entry.local_mtime
    };

    Some(RemoteEntry {
        is_hidden: is_hidden_remote_name(&name),
        is_symlink: false,
        is_dir: entry.isdir == 1,
        size: entry.size,
        modified_time: baidu_seconds_to_unix_millis(modified_seconds),
        name,
        path,
    })
}

/// 映射一整页。映射不出来的条目被跳过。
pub fn to_remote_entries(connection_id: &str, entries: &[BaiduFileEntry]) -> Vec<RemoteEntry> {
    entries
        .iter()
        .filter_map(|entry| to_remote_entry(connection_id, entry))
        .collect()
}

// ===========================================================================
// 错误分类
// ===========================================================================

/// 百度网盘侧的失败原因。
///
/// 这是**协议无关的语义分类**，不是 `RemoteErrorKind` 的副本 ——
/// 两者之间由 [`classify_baidu_failure`] 一张穷尽表连接。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaiduFailure {
    /// 会话失效 / 未登录（`errno = -6`）。
    Auth,
    /// 已登录但没有权限。
    Permission,
    /// 目标不存在。
    NotFound,
    /// 参数或协议层被拒。
    Protocol,
    /// 网络不可达：DNS 失败、连接被拒、超时、被限流。
    Unreachable,
    /// 连上了但中途断开。
    Disconnected,
    /// 响应不是合法 JSON，或结构与预期不符。
    Malformed,
    /// 响应体不是合法 UTF-8（文件名编码解不开）。
    Encoding,
    /// 本阶段不支持的能力（写操作、配额等）。
    Unsupported,
    /// 兜底。
    Unknown,
}

/// 把百度侧的失败原因翻译成本项目的错误分类。
///
/// 这是一张**穷尽**映射：每个 [`BaiduFailure`] 变体都必须落到一个
/// `RemoteErrorKind`，漏一个就会让某类失败退化成「笼统错误」。
pub fn classify_baidu_failure(failure: &BaiduFailure) -> RemoteErrorKind {
    match failure {
        BaiduFailure::Auth => RemoteErrorKind::Auth,
        BaiduFailure::Permission => RemoteErrorKind::Permission,
        BaiduFailure::NotFound => RemoteErrorKind::NotFound,
        BaiduFailure::Protocol => RemoteErrorKind::Protocol,
        BaiduFailure::Unreachable => RemoteErrorKind::Unreachable,
        BaiduFailure::Disconnected => RemoteErrorKind::Disconnected,
        // 「响应无法解析」在 `RemoteErrorKind::Protocol` 的定义里就是协议层错误。
        BaiduFailure::Malformed => RemoteErrorKind::Protocol,
        BaiduFailure::Encoding => RemoteErrorKind::Encoding,
        BaiduFailure::Unsupported => RemoteErrorKind::Unsupported,
        BaiduFailure::Unknown => RemoteErrorKind::Unknown,
    }
}

/// 把百度的 `errno` 翻成失败原因。
///
/// 逐条标注了证据强度，**不要把「社区约定」当成「实测」**：
///
/// | errno | 含义 | 证据 |
/// |---|---|---|
/// | `-6` | 用户未登录 / 会话失效 | **实测**（T1 探针：`{"errmsg":"用户未登录","errno":-6}`） |
/// | `-9` | 文件不存在 | 社区通行约定，**未实测** |
/// | `2` | 参数错误 | 社区通行约定，**未实测** |
///
/// 其余 errno 一律先看 `errmsg` 的中文提示（百度在 `errmsg` 里写得很直白），
/// 再兜底成 `Unknown`。
pub fn failure_from_errno(errno: i64, errmsg: Option<&str>) -> BaiduFailure {
    match errno {
        -6 => return BaiduFailure::Auth,
        -9 => return BaiduFailure::NotFound,
        2 => return BaiduFailure::Protocol,
        _ => {}
    }

    let message = errmsg.unwrap_or_default();
    if message.contains("未登录") || message.contains("登录已失效") {
        return BaiduFailure::Auth;
    }
    if message.contains("不存在") {
        return BaiduFailure::NotFound;
    }
    if message.contains("权限") {
        return BaiduFailure::Permission;
    }
    if message.contains("参数") {
        return BaiduFailure::Protocol;
    }

    BaiduFailure::Unknown
}

/// 把 HTTP 状态码翻成失败原因。
///
/// 3xx 归到 `Auth`：会话失效时百度会把请求**重定向到登录页**，
/// 而我们**刻意不跟随重定向**（`redirect::Policy::none()`），就是为了把这件事
/// 变成一条可分类的「请重新授权」，而不是拿到一个登录页 HTML 再报「响应无法解析」。
pub fn failure_from_http_status(status: u16) -> BaiduFailure {
    match status {
        200..=299 => BaiduFailure::Unknown,
        301 | 302 | 303 | 307 | 308 => BaiduFailure::Auth,
        401 | 403 => BaiduFailure::Auth,
        404 => BaiduFailure::NotFound,
        408 | 429 => BaiduFailure::Unreachable,
        500..=599 => BaiduFailure::Unreachable,
        _ => BaiduFailure::Protocol,
    }
}

/// 传输层失败的形态（把 `reqwest::Error` 的判断结果摘出来，便于离线单测）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TransportErrorShape {
    /// 连接阶段失败（DNS、握手、拒绝连接）。
    pub connect: bool,
    /// 超时。
    pub timeout: bool,
    /// 响应体读了一半断了。
    pub body: bool,
    /// 请求构造/发送阶段失败。
    pub request: bool,
}

/// 把 `reqwest::Error` 摘成 [`TransportErrorShape`]。
pub fn transport_shape_of(error: &reqwest::Error) -> TransportErrorShape {
    TransportErrorShape {
        connect: error.is_connect(),
        timeout: error.is_timeout(),
        body: error.is_body(),
        request: error.is_request(),
    }
}

/// 把传输层失败分类。
///
/// 纯函数形态：单测不碰网络。
pub fn classify_transport(shape: TransportErrorShape) -> RemoteErrorKind {
    if shape.connect || shape.timeout {
        return RemoteErrorKind::Unreachable;
    }
    if shape.body {
        return RemoteErrorKind::Disconnected;
    }
    if shape.request {
        return RemoteErrorKind::Protocol;
    }
    RemoteErrorKind::Unknown
}

/// 一行「这个错误分类在百度网盘上能不能出现」的说明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaiduErrorKindCoverage {
    pub kind: RemoteErrorKind,
    /// 本协议**能否**产出这个分类。
    pub producible: bool,
    /// 不能的话，为什么。**必须非空** —— 「不适用」也要说清楚为什么不适用。
    pub note: &'static str,
}

/// `RemoteErrorKind` 全部 11 个变体逐个表态的**穷尽表**。
///
/// 为什么要显式写出来：`RemoteErrorKind` 有 11 个变体，而百度网盘只涉及其中的
/// 9 个。剩下两个（`HostKeyUnknown` / `HostKeyChanged`）是 **SSH 主机密钥**
/// 概念 —— 百度网盘走 HTTPS + 系统信任库，没有「这台主机你信不信」这回事，
/// 所以**永远不会被本协议产出**。
///
/// 把这件事写成一张表而不是靠注释，是为了让「漏了哪个分类」变成一条会红的断言
/// —— 将来 `RemoteErrorKind` 加了新变体，这张表会立刻不完整。
pub fn baidu_error_kind_coverage() -> Vec<BaiduErrorKindCoverage> {
    vec![
        BaiduErrorKindCoverage {
            kind: RemoteErrorKind::Auth,
            producible: true,
            note: "会话失效：errno -6，或 3xx 重定向到登录页，或 HTTP 401/403。",
        },
        BaiduErrorKindCoverage {
            kind: RemoteErrorKind::HostKeyUnknown,
            producible: false,
            note:
                "不适用：这是 SSH 主机密钥概念；百度网盘走 HTTPS + 系统信任库，没有主机指纹可确认。",
        },
        BaiduErrorKindCoverage {
            kind: RemoteErrorKind::HostKeyChanged,
            producible: false,
            note:
                "不适用：同上；TLS 证书由系统信任库校验，证书链异常表现为连接失败（Unreachable）。",
        },
        BaiduErrorKindCoverage {
            kind: RemoteErrorKind::Unreachable,
            producible: true,
            note: "网络不可达：连接失败、超时、被限流（429）、服务端 5xx。",
        },
        BaiduErrorKindCoverage {
            kind: RemoteErrorKind::Permission,
            producible: true,
            note: "已登录但没有权限：errno 提示里含「权限」。",
        },
        BaiduErrorKindCoverage {
            kind: RemoteErrorKind::NotFound,
            producible: true,
            note: "目标不存在：errno -9，或提示里含「不存在」，或 HTTP 404。",
        },
        BaiduErrorKindCoverage {
            kind: RemoteErrorKind::Protocol,
            producible: true,
            note: "协议层错误：errno 2（参数错误）、响应无法解析（Malformed）、HTTP 4xx。",
        },
        BaiduErrorKindCoverage {
            kind: RemoteErrorKind::Encoding,
            producible: true,
            note: "响应体不是合法 UTF-8（文件名编码解不开）。",
        },
        BaiduErrorKindCoverage {
            kind: RemoteErrorKind::Disconnected,
            producible: true,
            note: "连上了但响应体读了一半断了。",
        },
        BaiduErrorKindCoverage {
            kind: RemoteErrorKind::Unsupported,
            producible: true,
            note: "本阶段不支持的能力：写操作（上传）、配额接口等。",
        },
        BaiduErrorKindCoverage {
            kind: RemoteErrorKind::Unknown,
            producible: true,
            note: "兜底：无法归类的失败。",
        },
    ]
}

/// 把失败原因翻成给用户看的错误。
///
/// 文案一律简体中文，且**不含任何凭据内容**。
pub fn to_remote_error(failure: &BaiduFailure) -> RemoteError {
    let message = match failure {
        BaiduFailure::Auth => "百度网盘会话已失效，请重新授权。",
        BaiduFailure::Permission => "百度网盘拒绝了这次操作，权限不足。",
        BaiduFailure::NotFound => "百度网盘上没有这个路径。",
        BaiduFailure::Protocol => "百度网盘接口拒绝了这次请求（参数或协议层错误）。",
        BaiduFailure::Unreachable => "连不上百度网盘服务器。",
        BaiduFailure::Disconnected => "与百度网盘的连接中途断开了。",
        BaiduFailure::Malformed => "百度网盘返回了无法解析的响应。",
        BaiduFailure::Encoding => "百度网盘返回的内容编码无法解析。",
        BaiduFailure::Unsupported => "这个操作在百度网盘上暂不支持。",
        BaiduFailure::Unknown => "百度网盘操作失败。",
    };

    RemoteError::new(classify_baidu_failure(failure), message)
}

// ===========================================================================
// HTTP
// ===========================================================================

/// 建一个专供百度网盘用的 HTTP 客户端。
///
/// **不跟随重定向**：会话失效时百度会 302 到登录页，我们要看见这个 302 才能
/// 报「请重新授权」；跟过去只会拿到一个登录页 HTML。
pub fn build_client() -> Result<reqwest::Client, RemoteError> {
    reqwest::Client::builder()
        .user_agent(BAIDU_PAN_USER_AGENT)
        .timeout(BAIDU_OPERATION_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| RemoteError::unknown(format!("百度网盘 HTTP 客户端初始化失败：{error}")))
}

/// 发一个 GET 并把响应体读成 UTF-8 文本。
///
/// 响应体**不是**用 `reqwest` 的 `text()` 读的：那个方法被 `charset` feature
/// 门控，而本 crate 的 `reqwest` 是 `default-features = false`，没开这个 feature
/// ⇒ 用了就编不过。这里用 `bytes()` + `String::from_utf8`，顺带把「不是合法
/// UTF-8」变成一个可分类的 `Encoding` 错误。
pub async fn get_text(
    client: &reqwest::Client,
    url: &str,
    session: &BaiduSession,
) -> Result<String, RemoteError> {
    let cookie = session
        .cookie_header()
        .ok_or_else(|| RemoteError::auth("百度网盘会话凭据缺失或格式非法，请重新授权。"))?;

    let response = client
        .get(url)
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .map_err(|error| {
            let kind = classify_transport(transport_shape_of(&error));
            RemoteError::new(kind, format!("访问百度网盘失败：{error}"))
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(to_remote_error(&failure_from_http_status(status.as_u16())));
    }

    let body = response.bytes().await.map_err(|error| {
        let kind = classify_transport(transport_shape_of(&error));
        RemoteError::new(kind, format!("读取百度网盘响应失败：{error}"))
    })?;

    String::from_utf8(body.to_vec()).map_err(|error| {
        RemoteError::encoding(format!("百度网盘响应不是合法的 UTF-8 文本：{error}"))
    })
}

/// 列一页目录。
pub async fn list_dir(
    client: &reqwest::Client,
    session: &BaiduSession,
    dir: &str,
    start: u32,
    limit: u32,
) -> Result<Vec<BaiduFileEntry>, RemoteError> {
    let url = list_dir_url(dir, start, limit);
    let body = get_text(client, &url, session).await?;
    parse_list_response(&body).map_err(|failure| to_remote_error(&failure))
}

/// 取账号信息。
pub async fn fetch_user_info(
    client: &reqwest::Client,
    session: &BaiduSession,
) -> Result<BaiduUserInfo, RemoteError> {
    let url = uinfo_url();
    let body = get_text(client, &url, session).await?;
    parse_uinfo_response(&body).map_err(|failure| to_remote_error(&failure))
}

/// 下载用的 HTTP 客户端。
///
/// 与 [`build_client`] 的区别有两条，**两条都是下载直链逼出来的**：
///
/// 1. **跟随重定向**：网盘给出的直链会 302 到 CDN 域名，不跟就只拿到一个 302。
///    `reqwest` 在跨主机重定向时会自动摘掉 `Cookie` 头 —— 正好是想要的：
///    凭据只发给 `pan.baidu.com`，不给 CDN。
/// 2. **超时更长**：整份文件要进内存，30 秒的请求超时对大文件必然超时。
///    连接本身仍然只给 30 秒。
pub fn build_download_client() -> Result<reqwest::Client, RemoteError> {
    reqwest::Client::builder()
        .user_agent(BAIDU_PAN_USER_AGENT)
        .timeout(BAIDU_DOWNLOAD_TIMEOUT)
        .connect_timeout(BAIDU_CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|error| RemoteError::unknown(format!("百度网盘下载客户端初始化失败：{error}")))
}

/// 取一个文件的下载直链。
///
/// 走 `filemetas`（`dlink=1`）。⚠️ **未实证**，见 [`parse_filemetas_response`]。
pub async fn fetch_dlink(
    client: &reqwest::Client,
    session: &BaiduSession,
    fs_id: u64,
) -> Result<String, RemoteError> {
    let url = filemetas_url(&[fs_id]);
    let body = get_text(client, &url, session).await?;
    let list = parse_filemetas_response(&body).map_err(|failure| to_remote_error(&failure))?;

    list.into_iter()
        .find(|item| item.fs_id == fs_id || item.fs_id == 0)
        .map(|item| item.dlink)
        .filter(|link| !link.is_empty())
        .ok_or_else(|| RemoteError::unsupported("百度网盘没有给出这个文件的下载直链。"))
}

/// 下载一个直链指向的内容，最多接受 `max_bytes` 字节。
///
/// **边下边判**：超过上限立刻停，而不是先把它整份收进内存再发现超了。
pub async fn download_file(
    client: &reqwest::Client,
    session: &BaiduSession,
    url: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, RemoteError> {
    let cookie = session
        .cookie_header()
        .ok_or_else(|| RemoteError::auth("百度网盘会话凭据缺失或格式非法，请重新授权。"))?;

    let response = client
        .get(url)
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .map_err(|error| {
            let kind = classify_transport(transport_shape_of(&error));
            RemoteError::new(kind, format!("下载百度网盘文件失败：{error}"))
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(to_remote_error(&failure_from_http_status(status.as_u16())));
    }

    let mut collected: Vec<u8> = Vec::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| {
            let kind = classify_transport(transport_shape_of(&error));
            RemoteError::new(kind, format!("下载百度网盘文件中断：{error}"))
        })?;

        if collected.len() as u64 + chunk.len() as u64 > max_bytes {
            return Err(RemoteError::unsupported(format!(
                "文件超过 {} MiB，阶段 1 不下载这么大的文件。",
                max_bytes / (1024 * 1024)
            )));
        }

        collected.extend_from_slice(&chunk);
    }

    Ok(collected)
}

// ===========================================================================
// 写操作：bdstoken / 新建目录 / 改名 / 删除
// ===========================================================================
//
// ⚠️ **端点形态未实证**。T5 动手时这台机器上还没有
// `remote-credentials.json`（没有任何可用的百度会话），打不出真实请求，
// 因此本节的形态依据是**社区通行实现**：
//
//   - `bypy`（`pip install bypy`）：`api/filemanager?opera=rename|delete`，
//     表单体 `filelist=[{"path":"…","newname":"…}]` / `filelist=["…"]`，
//     `bdstoken` 从首页 HTML 里正则取出；
//   - 百度官方 `cpanbd` Python SDK 的 `filemanager(opera, filelist)` 文档
//     给出的 `filelist` 示例与上面**逐字同形**。
//
// 形态一致 ≠ 服务端一定认。判据是 T9 真机清单第 9/10/11 条
// （新建出现在网页端、改名同步、删除进回收站）。**不要把本节当成已证结论。**

/// 网页端**旧**首页。
///
/// **写操作不再从这里取 `bdstoken`**（原因见 [`BAIDU_BDSTOKEN_ENDPOINT`]），
/// 目前它只剩内嵌登录流程当「是否已登录」的探针地址。
pub const BAIDU_PAN_HOME_URL: &str = "https://pan.baidu.com/disk/home";

/// 写操作要用的 `bdstoken` 的真正来源：JSON 接口 `api/gettemplatevariable`。
///
/// 2026-11-09 真机实测（证据 `.superpowers/sdd/acc-20261109/baidu-token-source.txt`）：
///
/// 1. `GET /disk/home` 即使带着形态正确的 cookie 也**恒定 302**，Location 指向
///    `/disk/main?from=homeFlow` —— 也就是**不是登录页**，旧首页已下线；
/// 2. 跟到 `/disk/main` 拿到的是 SPA 外壳，里面 `"bdstoken":""` 是**空串**，
///    `uk:0`、`username:""`，而且 `yundata` / `yunData` / `initContext` /
///    `baiduInfo` / `hasLogin` 的命中数**全为 0**。
///
/// ⇒ 「抓首页 HTML 再正则取 token」这条路整体作废。更糟的是本模块的 client 刻意用
/// `redirect::Policy::none()`，那个 302 会被 [`failure_from_http_status`] 归成
/// [`BaiduFailure::Auth`]，于是**会话明明有效也会报「百度网盘会话已失效」**，
/// 而 [`should_refresh_token`] 又认为 Auth 值得重试，同一条死路走两遍。
/// 这就是用户看到的删除报错的成因。
pub const BAIDU_BDSTOKEN_ENDPOINT: &str = "https://pan.baidu.com/api/gettemplatevariable";

/// 写操作请求的 `Referer`。
///
/// 网页端的写接口本来就是在 `disk/home` 页面里点的，社区实现都带这个头。
/// 带错的后果不是被拒，而是「参数被静默忽略」——所以宁可带上。
pub const BAIDU_PAN_REFERER: &str = "https://pan.baidu.com/disk/home";

/// `bdstoken` 的缓存时长（30 分钟）。
///
/// `bdstoken` 与登录态同生共死，会话失效时它也会跟着失效。缓存只是为了
/// 不必每做一次写操作就重新拉一遍首页（首页有几百 KB）。
pub const BAIDU_BDSTOKEN_TTL: Duration = Duration::from_secs(1800);

/// 新建目录：`POST /api/create`。
///
/// `path` 是**目标目录的完整路径**（含最后那一段目录名），`isdir=1` 表示建目录。
pub fn create_dir_url(bdstoken: &str, path: &str) -> String {
    format!(
        "{BAIDU_PAN_BASE_URL}/api/create?a=commit&channel=chunlei&bdstoken={}&path={}&isdir=1&block_list=[]&web=1&app_id={}",
        encode_query_component(bdstoken),
        encode_query_component(path),
        BAIDU_PAN_APP_ID
    )
}

/// 新建目录的**表单请求体**。
///
/// 2026-11-09 真机 A/B（证据 `.superpowers/sdd/accept-3134/baidu-create-ab.log`）：
/// `POST /api/create` 只把参数放在 query、body 传空串时服务端恒回 `{"errno":2,"path":""}`；
/// 同样的参数改成表单 body 则 `errno:0` 并真的建出目录。
/// ⇒ body 必须带 `path` / `isdir=1` / `block_list=[]`，query 里的同名参数只是兼容冗余。
pub fn create_dir_form_body(path: &str) -> String {
    encode_form_body(&[
        ("path", path),
        ("isdir", "1"),
        ("block_list", "[]"),
    ])
}

#[cfg(test)]
mod create_dir_form_body_tests {
    use super::create_dir_form_body;

    #[test]
    fn form_body_carries_path_isdir_and_block_list() {
        let body = create_dir_form_body("/0000temp/zz-accept-3134");
        assert_eq!(
            body,
            "path=%2F0000temp%2Fzz-accept-3134&isdir=1&block_list=%5B%5D"
        );
    }

    #[test]
    fn form_body_escapes_special_characters() {
        let body = create_dir_form_body("/a b&c=d");
        assert!(body.contains("path=%2Fa%20b%26c%3Dd"));
        assert!(!body.contains("path=/a b&c=d"));
    }
}

/// 改名 / 删除：`POST /api/filemanager`。
///
/// `opera` 取 `"rename"` 或 `"delete"`。`async=2` 表示同步返回结果，
/// `onnest=fail` 表示遇到失败就整批失败。
pub fn filemanager_url(bdstoken: &str, opera: &str) -> String {
    format!(
        "{BAIDU_PAN_BASE_URL}/api/filemanager?opera={}&async=2&onnest=fail&channel=chunlei&bdstoken={}&web=1&app_id={}",
        encode_query_component(opera),
        encode_query_component(bdstoken),
        BAIDU_PAN_APP_ID
    )
}

/// 从首页 HTML 里取 `bdstoken`。
///
/// 页面里这个值有几种写法，全部要认：
///
/// | 写法 | 出处 |
/// |---|---|
/// | `"bdstoken":"xxxx"` | `locals.mset({...})` / `yunData.setData({...})` |
/// | `\"bdstoken\":\"xxxx\"` | 嵌在 JS 字符串字面量里（键和值前面各多一个反斜杠） |
/// | `'bdstoken','xxxx'` | 老版本 `'bdstoken','xxxx'` 一族 |
/// | `initPrefetch('xxxx', …)` | 更老的一代页面，token 就是 `initPrefetch` 的第一个参数 |
///
/// 值本身是十六进制形态，所以只取 `[0-9A-Za-z_-]`。
///
/// `"null"` / `"undefined"` 一律当作**没拿到** —— 拿到这两个字面量当 token
/// 发出去，服务端会回一个语焉不详的参数错误，比直接报错更难查。
pub fn parse_bdstoken(page: &str) -> Option<String> {
    // 显式写法优先；转义形态单独列出，因为转义页面里键和值前面各多一个反斜杠。
    const MARKERS: [&str; 7] = [
        "\"bdstoken\":\"",
        "\"bdstoken\": \"",
        "bdstoken\":\"",
        "\"bdstoken\\\":\\\"",
        "bdstoken\\\":\\\"",
        "'bdstoken','",
        "'bdstoken': '",
    ];

    for marker in MARKERS {
        let Some(index) = page.find(marker) else {
            continue;
        };

        if let Some(token) = take_bdstoken_value(&page[index + marker.len()..]) {
            return Some(token);
        }
    }

    // 兜底：老页面把 token 放在 `initPrefetch` 的第一个参数里（bypy 老实现的取法）。
    // 只有上面所有显式写法都没命中时才用它 —— 万一页面里两种都有，
    // 显式的那个才是权威值，不能被 `initPrefetch` 抢走。
    if let Some(index) = page.find("initPrefetch(") {
        let rest = &page[index + "initPrefetch(".len()..].trim_start();
        let quote = rest.chars().next()?;
        if quote == '\'' || quote == '"' {
            if let Some(token) = take_bdstoken_value(&rest[quote.len_utf8()..]) {
                return Some(token);
            }
        }
    }

    None
}

/// 从 token 值的起点往后取一段 `[0-9A-Za-z_-]`，取不到或取到 `null` / `undefined`
/// 都算没拿到。
fn take_bdstoken_value(text: &str) -> Option<String> {
    let end = text
        .find(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '_' || character == '-')
        })
        .unwrap_or(text.len());

    if end == 0 {
        return None;
    }

    let token = &text[..end];
    if token == "null" || token == "undefined" {
        return None;
    }

    Some(token.to_string())
}

/// `bdstoken` 接口的完整 URL。
///
/// 参数形态照社区实现（bypy / python-baidupcs）：`fields` 是一个 JSON 数组字符串，
/// 整值做一次百分号编码。我们只要 `bdstoken` 一项 —— 多要字段不增加信息，
/// 反而把 `token`/`auth` 这类无关值带进日志风险里。
pub fn bdstoken_url() -> String {
    format!(
        "{BAIDU_BDSTOKEN_ENDPOINT}?clienttype=0&app_id={BAIDU_PAN_APP_ID}&web=1&fields={}",
        encode_query_component(r#"["bdstoken"]"#)
    )
}

/// `gettemplatevariable` 响应里的 `result` 对象。
#[derive(Debug, Clone, Deserialize)]
struct BaiduTokenResult {
    #[serde(default)]
    bdstoken: Option<String>,
}

/// `gettemplatevariable` 响应外壳。
///
/// 实测形状（`.superpowers/sdd/acc-20261109/baidu-token-source.txt` C 段）：
///
/// ```text
/// 未登录: {"errno":-6,"request_id":…}
/// 已登录: {"errno":0,"result":{"bdstoken":"<hex32>", …},"request_id":…}
/// ```
#[derive(Debug, Clone, Deserialize)]
struct BaiduTokenEnvelope {
    #[serde(default)]
    errno: Option<i64>,
    #[serde(default)]
    errmsg: Option<String>,
    #[serde(default)]
    error_code: Option<i64>,
    #[serde(default)]
    error_msg: Option<String>,
    #[serde(default)]
    result: Option<BaiduTokenResult>,
}

/// 解析 `gettemplatevariable` 的 JSON 响应，取出 `bdstoken`。
///
/// 三条判定，全部有实测依据：
///
/// 1. `errno` / `error_code` 非 0 ⇒ 走 [`classify_write_failure`]，与会话/参数错误
///    同口径（`-6` 归 Auth ⇒ 触发批次 B 的内嵌重新登录）；
/// 2. `result.bdstoken` 缺失、空串、`"null"`、`"undefined"` ⇒ [`BaiduFailure::Protocol`]。
///    `/disk/main` 页面里 `"bdstoken":""` 就是这个形状（D 段实测），
///    把它当「会话失效」报会误导用户；
/// 3. 响应不是合法 JSON ⇒ [`BaiduFailure::Malformed`]（接口改版的第一信号）。
pub fn parse_bdstoken_response(body: &str) -> Result<String, BaiduWriteError> {
    let envelope: BaiduTokenEnvelope =
        serde_json::from_str(body).map_err(|_| BaiduWriteError {
            failure: BaiduFailure::Malformed,
            server_message: None,
        })?;

    if let Some(code) = envelope.error_code {
        if code != 0 {
            let message = envelope.error_msg.clone();
            return Err(BaiduWriteError {
                failure: classify_write_failure(code, message.as_deref()),
                server_message: message,
            });
        }
    }

    let errno = envelope.errno.unwrap_or(0);
    if errno != 0 {
        let message = envelope.errmsg.clone();
        return Err(BaiduWriteError {
            failure: classify_write_failure(errno, message.as_deref()),
            server_message: message,
        });
    }

    let raw = envelope
        .result
        .and_then(|result| result.bdstoken)
        .unwrap_or_default();
    let token = raw.trim();

    if token.is_empty() || token == "null" || token == "undefined" {
        return Err(BaiduWriteError {
            failure: BaiduFailure::Protocol,
            server_message: Some("百度网盘没有下发 bdstoken。".to_string()),
        });
    }

    Ok(token.to_string())
}

/// 拼一个 `application/x-www-form-urlencoded` 的请求体。
pub fn encode_form_body(pairs: &[(&str, &str)]) -> String {
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

/// 改名用的 `filelist` 表单值。
///
/// 值本身是一个 JSON 数组，**再整体**做一次百分号编码（表单值的规矩）。
/// 路径里的引号 / 反斜杠由 `serde_json` 负责转义 —— 网盘路径里出现 `"` 是
/// 允许的，手拼 JSON 会把它变成「参数注入」。
pub fn rename_filelist(path: &str, new_name: &str) -> String {
    let path_json = serde_json::to_string(path).unwrap_or_default();
    let name_json = serde_json::to_string(new_name).unwrap_or_default();
    format!("[{{\"path\":{path_json},\"newname\":{name_json}}}]")
}

/// 删除用的 `filelist` 表单值（一个路径的 JSON 数组）。
pub fn delete_filelist(path: &str) -> String {
    let path_json = serde_json::to_string(path).unwrap_or_default();
    format!("[{path_json}]")
}

/// 写操作响应里的一条结果。
#[derive(Debug, Clone, Deserialize)]
struct BaiduWriteInfo {
    #[serde(default)]
    errno: Option<i64>,
    #[serde(default)]
    errno_msg: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

/// 写操作响应外壳。
///
/// `/api/create` 与 `/api/filemanager` 两种形状都吃：
///
/// ```text
/// 成功: {"errno":0,"path":"/x","isdir":1}
/// 成功: {"errno":0,"info":[{"errno":0,"path":"/x"}]}
/// 失败: {"errno":2,"errmsg":"…"}
/// 失败: {"errno":0,"info":[{"errno":31066,"errno_msg":"文件已存在","path":"/x"}]}
/// 失败: {"error_code":3,"error_msg":"…"}
/// ```
#[derive(Debug, Clone, Deserialize)]
struct BaiduWriteEnvelope {
    #[serde(default)]
    errno: Option<i64>,
    #[serde(default)]
    errmsg: Option<String>,
    #[serde(default)]
    errno_msg: Option<String>,
    #[serde(default)]
    error_code: Option<i64>,
    #[serde(default)]
    error_msg: Option<String>,
    #[serde(default)]
    info: Vec<BaiduWriteInfo>,
}

/// 写操作的失败：分类 + 服务端原话。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaiduWriteError {
    /// 归类后的失败原因。
    pub failure: BaiduFailure,
    /// 服务端给的中文提示（如果有）。**不含凭据。**
    pub server_message: Option<String>,
}

impl BaiduWriteError {
    /// 翻成给用户看的错误。服务端的中文提示会附在后面 ——
    /// 「目标已存在」这种提示只有服务端说得清。
    pub fn to_remote_error(&self) -> RemoteError {
        let base = to_remote_error(&self.failure);

        match self.server_message.as_deref().map(str::trim) {
            Some(message) if !message.is_empty() => RemoteError::new(
                base.kind,
                format!("{}（服务端提示：{message}）", base.message),
            ),
            _ => base,
        }
    }
}

/// 把写操作返回的 `errno` 归类。
///
/// 与 [`failure_from_errno`] 的区别只有一处：服务端用「文件已存在」这类
/// **中文提示**表达「重名」，而 `errno` 本身是个没见过的数字。把它归到
/// `Protocol`（参数层被拒）比归到 `Unknown` 对用户有用得多。
fn classify_write_failure(errno: i64, message: Option<&str>) -> BaiduFailure {
    let failure = failure_from_errno(errno, message);

    if failure == BaiduFailure::Unknown {
        let text = message.unwrap_or_default();
        if text.contains("已存在") || text.contains("已重名") {
            return BaiduFailure::Protocol;
        }
    }

    failure
}

/// 解析写操作的响应。
///
/// **顶层 `errno` 为 0 不代表成功** —— `filemanager` 把逐条结果放在 `info`
/// 数组里，顶层仍是 0，而里面那条可能是 `errno:31066`（已存在）。
/// 只看顶层就会把「没改名成」当成「改名成功了」—— 这是最坏的一类假成功。
pub fn parse_write_response(body: &str) -> Result<(), BaiduWriteError> {
    let envelope: BaiduWriteEnvelope = serde_json::from_str(body).map_err(|_| BaiduWriteError {
        failure: BaiduFailure::Malformed,
        server_message: None,
    })?;

    if let Some(code) = envelope.error_code {
        if code != 0 {
            let message = envelope.error_msg.clone();
            return Err(BaiduWriteError {
                failure: classify_write_failure(code, message.as_deref()),
                server_message: message,
            });
        }
    }

    let top_errno = envelope.errno.unwrap_or(0);
    if top_errno != 0 {
        let message = envelope
            .errmsg
            .clone()
            .or_else(|| envelope.errno_msg.clone());
        return Err(BaiduWriteError {
            failure: classify_write_failure(top_errno, message.as_deref()),
            server_message: message,
        });
    }

    for item in &envelope.info {
        let item_errno = item.errno.unwrap_or(0);
        if item_errno != 0 {
            let message = item.errno_msg.clone();
            return Err(BaiduWriteError {
                failure: classify_write_failure(item_errno, message.as_deref()),
                server_message: message,
            });
        }
    }

    Ok(())
}

/// 发一个 `POST`（表单编码）并把响应体读成 UTF-8 文本。
///
/// 与 [`get_text`] 的差别只有三处：方法、带 `Referer`、带
/// `Content-Type: application/x-www-form-urlencoded`。
///
/// 响应体仍然用 `bytes()` + `String::from_utf8` 读（`text()` 被 `charset`
/// feature 门控，本 crate 没开这个 feature）。
pub async fn post_form(
    client: &reqwest::Client,
    session: &BaiduSession,
    url: &str,
    body: &str,
) -> Result<String, RemoteError> {
    let cookie = session
        .cookie_header()
        .ok_or_else(|| RemoteError::auth("百度网盘会话凭据缺失或格式非法，请重新授权。"))?;

    let response = client
        .post(url)
        .header(reqwest::header::COOKIE, cookie)
        .header(reqwest::header::REFERER, BAIDU_PAN_REFERER)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body.to_string())
        .send()
        .await
        .map_err(|error| {
            let kind = classify_transport(transport_shape_of(&error));
            RemoteError::new(kind, format!("访问百度网盘失败：{error}"))
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(to_remote_error(&failure_from_http_status(status.as_u16())));
    }

    let bytes = response.bytes().await.map_err(|error| {
        let kind = classify_transport(transport_shape_of(&error));
        RemoteError::new(kind, format!("读取百度网盘响应失败：{error}"))
    })?;

    String::from_utf8(bytes.to_vec()).map_err(|error| {
        RemoteError::encoding(format!("百度网盘响应不是合法的 UTF-8 文本：{error}"))
    })
}

/// 取写操作要用的 `bdstoken`。
///
/// 来源是 [`BAIDU_BDSTOKEN_ENDPOINT`]（JSON 接口），**不再是 `/disk/home` 的 HTML**。
/// 原因（2026-11-09 实测，见常量文档）：旧首页恒定 302，本模块 client 的
/// `Policy::none()` 把 3xx 归成 [`BaiduFailure::Auth`]，导致会话明明有效也报
/// 「百度网盘会话已失效，请重新授权」——即用户报的删除 bug 的成因 D1。
pub async fn fetch_bdstoken(
    client: &reqwest::Client,
    session: &BaiduSession,
) -> Result<String, RemoteError> {
    let cookie = session
        .cookie_header()
        .ok_or_else(|| RemoteError::auth("百度网盘会话凭据缺失或格式非法，请重新授权。"))?;

    let response = client
        .get(bdstoken_url())
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .map_err(|error| {
            let kind = classify_transport(transport_shape_of(&error));
            RemoteError::new(kind, format!("访问百度网盘失败：{error}"))
        })?;

    let status = response.status();

    // 护栏：token 接口按约定只会回 JSON。**收到 3xx 一律报 Protocol，不归 Auth** ——
    // 「幻影会话失效」（会话有效却报已失效）就是这么来的，不许它借道复活。
    if status.is_redirection() {
        return Err(RemoteError::protocol(format!(
            "百度网盘 bdstoken 接口返回了重定向（HTTP {}），接口形态可能已变更。",
            status.as_u16()
        )));
    }

    if !status.is_success() {
        return Err(to_remote_error(&failure_from_http_status(status.as_u16())));
    }

    let bytes = response.bytes().await.map_err(|error| {
        let kind = classify_transport(transport_shape_of(&error));
        RemoteError::new(kind, format!("读取百度网盘响应失败：{error}"))
    })?;

    let body = String::from_utf8(bytes.to_vec()).map_err(|error| {
        RemoteError::encoding(format!("百度网盘响应不是合法的 UTF-8 文本：{error}"))
    })?;

    parse_bdstoken_response(&body).map_err(|error| error.to_remote_error())
}

// ===========================================================================
// 写操作的单测（零网络）
// ===========================================================================

#[cfg(test)]
mod write_tests {
    use super::*;

    // ------------------------------------------------------------------
    // bdstoken
    // ------------------------------------------------------------------

    #[test]
    fn the_bdstoken_is_read_from_the_home_page() {
        let page = r#"locals.mset({"bdstoken":"742aa0d6886423a5503bbc67afdb2a7d","other":1});"#;

        assert_eq!(
            parse_bdstoken(page),
            Some("742aa0d6886423a5503bbc67afdb2a7d".to_string())
        );
    }

    #[test]
    fn the_bdstoken_is_read_from_a_page_with_a_space_after_the_colon() {
        let page = r#"yunData.setData({ "bdstoken": "abc123" });"#;

        assert_eq!(parse_bdstoken(page), Some("abc123".to_string()));
    }

    #[test]
    fn the_bdstoken_is_read_from_a_javascript_escaped_page() {
        let page = r#"var s = "{\"bdstoken\":\"deadbeef\"}";"#;

        assert_eq!(parse_bdstoken(page), Some("deadbeef".to_string()));
    }

    #[test]
    fn the_bdstoken_is_read_from_the_legacy_single_quote_form() {
        let page = "initPrefetch('9f8e7d6c5b4a', {'a':1});";

        assert_eq!(parse_bdstoken(page), Some("9f8e7d6c5b4a".to_string()));
    }

    #[test]
    fn the_explicit_bdstoken_wins_over_the_legacy_init_prefetch_form() {
        let page = "initPrefetch('9f8e7d6c5b4a', {\"bdstoken\":\"deadbeef\"});";

        assert_eq!(parse_bdstoken(page), Some("deadbeef".to_string()));
    }

    #[test]
    fn a_page_without_a_bdstoken_has_none() {
        assert_eq!(parse_bdstoken("<html><body>login</body></html>"), None);
    }

    #[test]
    fn a_null_bdstoken_is_rejected() {
        assert_eq!(parse_bdstoken(r#"{"bdstoken":"null"}"#), None);
    }

    #[test]
    fn an_undefined_bdstoken_is_rejected() {
        assert_eq!(parse_bdstoken(r#"{"bdstoken":"undefined"}"#), None);
    }

    #[test]
    fn an_empty_bdstoken_is_rejected() {
        assert_eq!(parse_bdstoken(r#"{"bdstoken":""}"#), None);
    }

    // ------------------------------------------------------------------
    // bdstoken：JSON 接口来源（2026-11-09 D1 修复）
    // ------------------------------------------------------------------

    #[test]
    fn the_bdstoken_url_points_at_the_json_endpoint_not_the_home_page() {
        let url = bdstoken_url();

        assert!(url.starts_with("https://pan.baidu.com/api/gettemplatevariable?"));
        assert!(url.contains(&format!("app_id={BAIDU_PAN_APP_ID}")));
        assert!(url.contains("fields=%5B%22bdstoken%22%5D"));
        assert!(!url.contains("disk/home"));
        assert!(!url.contains("disk/main"));
    }

    #[test]
    fn the_bdstoken_is_read_from_the_json_envelope() {
        let body = r#"{"errno":0,"request_id":123,"result":{"bdstoken":"742aa0d6886423a5503bbc67afdb2a7d","token":"","uk":"123456"}}"#;

        assert_eq!(
            parse_bdstoken_response(body).as_deref(),
            Ok("742aa0d6886423a5503bbc67afdb2a7d")
        );
    }

    #[test]
    fn a_negative_errno_in_the_token_envelope_keeps_the_auth_classification() {
        // 实测：无效会话打 gettemplatevariable 回 {"errno":-6,…}（D 段 C 项）。
        // -6 必须继续归 Auth，才能触发批次 B 的内嵌重新登录。
        let error = parse_bdstoken_response(r#"{"errno":-6,"request_id":1}"#).unwrap_err();

        assert_eq!(error.failure, BaiduFailure::Auth);
    }

    #[test]
    fn an_empty_bdstoken_in_the_json_envelope_is_a_protocol_failure() {
        // /disk/main 的真实形状：errno=0 而 bdstoken 为空串。
        // 归 Protocol 而非 Auth —— 「会话已失效」在此是误导。
        let error =
            parse_bdstoken_response(r#"{"errno":0,"result":{"bdstoken":"","uk":"0"}}"#).unwrap_err();

        assert_eq!(error.failure, BaiduFailure::Protocol);
    }

    #[test]
    fn a_literal_null_bdstoken_in_the_json_envelope_is_rejected() {
        let error = parse_bdstoken_response(r#"{"errno":0,"result":{"bdstoken":"null"}}"#)
            .unwrap_err();

        assert_eq!(error.failure, BaiduFailure::Protocol);
    }

    #[test]
    fn a_missing_result_object_in_the_json_envelope_is_rejected() {
        let error = parse_bdstoken_response(r#"{"errno":0,"request_id":7}"#).unwrap_err();

        assert_eq!(error.failure, BaiduFailure::Protocol);
    }

    #[test]
    fn a_non_json_token_response_is_malformed() {
        // 接口改版（比如被换成 HTML）的第一信号就是这里。
        let error = parse_bdstoken_response("<html><body>gateway</body></html>").unwrap_err();

        assert_eq!(error.failure, BaiduFailure::Malformed);
    }

    // ------------------------------------------------------------------
    // URL 与表单
    // ------------------------------------------------------------------

    #[test]
    fn the_create_url_carries_the_token_and_the_full_path() {
        let url = create_dir_url("tok", "/01A_软件/新建目录");

        assert!(url.starts_with("https://pan.baidu.com/api/create?"));
        assert!(url.contains("a=commit"));
        assert!(url.contains("isdir=1"));
        assert!(url.contains("bdstoken=tok"));
        // 路径整体编码，斜杠不能裸露。
        assert!(
            url.contains("path=%2F01A_%E8%BD%AF%E4%BB%B6%2F%E6%96%B0%E5%BB%BA%E7%9B%AE%E5%BD%95")
        );
    }

    #[test]
    fn the_filemanager_url_carries_the_opera_and_the_token() {
        let url = filemanager_url("tok", "rename");

        assert!(url.starts_with("https://pan.baidu.com/api/filemanager?"));
        assert!(url.contains("opera=rename"));
        assert!(url.contains("async=2"));
        assert!(url.contains("onnest=fail"));
        assert!(url.contains("bdstoken=tok"));
    }

    #[test]
    fn the_form_body_is_percent_encoded() {
        let body = encode_form_body(&[("filelist", "[\"/a b\"]")]);

        assert_eq!(body, "filelist=%5B%22%2Fa%20b%22%5D");
    }

    #[test]
    fn the_rename_filelist_is_a_json_array() {
        let value = rename_filelist("/pub/a.txt", "b.txt");

        assert_eq!(value, r#"[{"path":"/pub/a.txt","newname":"b.txt"}]"#);
    }

    #[test]
    fn the_delete_filelist_is_a_json_array_of_one_path() {
        let value = delete_filelist("/pub/a.txt");

        assert_eq!(value, r#"["/pub/a.txt"]"#);
    }

    #[test]
    fn a_quote_in_the_path_is_escaped_by_the_json_encoder() {
        let value = rename_filelist("/pub/a\"b.txt", "c.txt");

        // 手拼 JSON 会在这里破掉，整个 filelist 变成可注入的参数。
        assert_eq!(value, r#"[{"path":"/pub/a\"b.txt","newname":"c.txt"}]"#);
    }

    // ------------------------------------------------------------------
    // 响应
    // ------------------------------------------------------------------

    #[test]
    fn a_successful_create_response_is_accepted() {
        assert!(parse_write_response(r#"{"errno":0,"path":"/pub/sub","isdir":1}"#).is_ok());
    }

    #[test]
    fn a_successful_filemanager_response_is_accepted() {
        assert!(
            parse_write_response(r#"{"errno":0,"info":[{"errno":0,"path":"/pub/a.txt"}]}"#).is_ok()
        );
    }

    #[test]
    fn a_top_level_failure_is_reported() {
        let error =
            parse_write_response(r#"{"errno":2,"errmsg":"参数错误"}"#).expect_err("must fail");

        assert_eq!(error.failure, BaiduFailure::Protocol);
        assert_eq!(error.server_message.as_deref(), Some("参数错误"));
    }

    #[test]
    fn a_per_item_failure_is_reported_even_when_the_top_level_errno_is_zero() {
        let error = parse_write_response(
            r#"{"errno":0,"info":[{"errno":31066,"errno_msg":"文件已存在","path":"/pub/sub"}]}"#,
        )
        .expect_err("must fail");

        assert_eq!(error.failure, BaiduFailure::Protocol);
        assert_eq!(error.server_message.as_deref(), Some("文件已存在"));
    }

    #[test]
    fn an_auth_failure_is_reported_as_auth() {
        let error =
            parse_write_response(r#"{"errno":-6,"errmsg":"用户未登录"}"#).expect_err("must fail");

        assert_eq!(error.failure, BaiduFailure::Auth);
    }

    #[test]
    fn the_xpan_error_shape_is_reported() {
        let error = parse_write_response(r#"{"error_code":3,"error_msg":"Unsupported open api"}"#)
            .expect_err("must fail");

        assert_eq!(error.failure, BaiduFailure::Unknown);
        assert_eq!(
            error.server_message.as_deref(),
            Some("Unsupported open api")
        );
    }

    #[test]
    fn a_non_json_body_is_reported_as_malformed() {
        let error = parse_write_response("<html>login</html>").expect_err("must fail");

        assert_eq!(error.failure, BaiduFailure::Malformed);
    }

    #[test]
    fn the_error_message_carries_the_server_hint() {
        let error = parse_write_response(
            r#"{"errno":0,"info":[{"errno":31066,"errno_msg":"文件已存在"}]}"#,
        )
        .expect_err("must fail");

        let remote = error.to_remote_error();

        assert_eq!(remote.kind, RemoteErrorKind::Protocol);
        assert!(remote.message.contains("文件已存在"));
    }

    #[test]
    fn the_error_message_never_leaks_the_cookie() {
        let error =
            parse_write_response(r#"{"errno":-6,"errmsg":"用户未登录"}"#).expect_err("must fail");

        assert!(!error.to_remote_error().message.contains("BDUSS"));
    }
}

// ===========================================================================
// 单测（零网络）
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// T1 探针实测过的响应形状（原样照抄，不是构造出来的）。
    const OBSERVED_LIST_BODY: &str = r#"{"errno":0,"list":[
        {"fs_id":101,"path":"/01A_软件","server_filename":"01A_软件","isdir":1,"size":0,"server_mtime":1700000000},
        {"fs_id":102,"path":"/01A_软件/设计素材.zip","server_filename":"设计素材.zip","isdir":0,"size":2048,"server_mtime":1700000060}
    ]}"#;

    // ---- URL 拼装 ------------------------------------------------------

    #[test]
    fn the_list_url_matches_the_url_the_probe_used() {
        // T1 探针实测 200 的那条请求，逐字对照。
        assert_eq!(
            list_dir_url("/", 0, 10),
            "https://pan.baidu.com/rest/2.0/xpan/file?method=list&dir=%2F&order=name&start=0&limit=10&web=1&app_id=250528"
        );
    }

    #[test]
    fn the_list_url_encodes_the_whole_path_including_the_slash() {
        let url = list_dir_url("/01A_软件", 1000, 500);

        assert!(url.contains("dir=%2F01A_%E8%BD%AF%E4%BB%B6"), "{url}");
        assert!(url.contains("start=1000"), "{url}");
        assert!(url.contains("limit=500"), "{url}");
    }

    #[test]
    fn the_list_url_clamps_an_oversized_limit() {
        assert!(list_dir_url("/", 0, 5000).contains("limit=1000"));
    }

    #[test]
    fn the_uinfo_url_carries_the_app_id() {
        let url = uinfo_url();

        assert!(url.starts_with("https://pan.baidu.com/rest/2.0/xpan/nas?method=uinfo"));
        assert!(url.contains("app_id=250528"), "{url}");
        assert!(url.contains("web=1"), "{url}");
    }

    #[test]
    fn the_filemetas_url_encodes_the_fs_id_array() {
        let url = filemetas_url(&[1, 2, 3]);

        assert!(url.contains("fsids=%5B1%2C2%2C3%5D"), "{url}");
        assert!(url.contains("dlink=1"), "{url}");
    }

    #[test]
    fn the_filemetas_url_handles_an_empty_id_list() {
        assert!(filemetas_url(&[]).contains("fsids=%5B%5D"));
    }

    #[test]
    fn the_dlink_url_carries_the_sign_and_timestamp() {
        let url = dlink_url(42, "a+b/c", 1700000000);

        assert!(url.contains("fs_id=42"), "{url}");
        assert!(url.contains("sign=a%2Bb%2Fc"), "{url}");
        assert!(url.contains("timestamp=1700000000"), "{url}");
        assert!(url.contains("dlink=1"), "{url}");
    }

    #[test]
    fn query_components_keep_only_unreserved_characters() {
        assert_eq!(encode_query_component("aZ0-_.~"), "aZ0-_.~");
        assert_eq!(encode_query_component("/"), "%2F");
        assert_eq!(encode_query_component("a b"), "a%20b");
        assert_eq!(encode_query_component("中文"), "%E4%B8%AD%E6%96%87");
    }

    // ---- 分页 ----------------------------------------------------------

    #[test]
    fn the_page_limit_is_clamped_to_the_server_maximum() {
        assert_eq!(clamp_page_limit(10), 10);
        assert_eq!(clamp_page_limit(1000), 1000);
        assert_eq!(clamp_page_limit(100_000), 1000);
    }

    #[test]
    fn a_zero_limit_falls_back_to_the_default_page_size() {
        assert_eq!(clamp_page_limit(0), BAIDU_LIST_PAGE_LIMIT);
    }

    #[test]
    fn a_short_page_means_the_listing_is_complete() {
        let page = ListPage {
            start: 0,
            limit: 1000,
        };

        assert_eq!(next_page(&page, 3), None);
    }

    #[test]
    fn a_full_page_advances_the_offset() {
        let page = ListPage {
            start: 0,
            limit: 1000,
        };

        assert_eq!(
            next_page(&page, 1000),
            Some(ListPage {
                start: 1000,
                limit: 1000
            })
        );
    }

    #[test]
    fn an_empty_page_ends_the_listing() {
        let page = ListPage {
            start: 0,
            limit: 1000,
        };

        assert_eq!(next_page(&page, 0), None);
    }

    #[test]
    fn the_offset_never_overflows() {
        let page = ListPage {
            start: u32::MAX - 10,
            limit: 1000,
        };

        assert_eq!(next_page(&page, 1000), None);
    }

    // ---- 响应解析 ------------------------------------------------------

    #[test]
    fn the_observed_list_payload_is_parsed() {
        let entries =
            parse_list_response(OBSERVED_LIST_BODY).expect("the probe payload must parse");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].fs_id, 101);
        assert_eq!(entries[0].path, "/01A_软件");
        assert_eq!(entries[0].isdir, 1);
        assert_eq!(entries[1].server_filename, "设计素材.zip");
        assert_eq!(entries[1].size, 2048);
        assert_eq!(entries[1].server_mtime, 1700000060);
    }

    #[test]
    fn an_empty_list_is_a_valid_answer() {
        let entries = parse_list_response(r#"{"errno":0,"list":[]}"#).expect("empty list");

        assert!(entries.is_empty());
    }

    #[test]
    fn a_missing_list_key_is_an_empty_list() {
        let entries = parse_list_response(r#"{"errno":0}"#).expect("missing list");

        assert!(entries.is_empty());
    }

    #[test]
    fn a_non_json_body_is_reported_as_malformed() {
        let failure = parse_list_response("<html>login</html>").expect_err("not json");

        assert_eq!(failure, BaiduFailure::Malformed);
    }

    #[test]
    fn a_body_without_errno_is_reported_as_malformed() {
        let failure = parse_list_response(r#"{"list":[]}"#).expect_err("no errno");

        assert_eq!(failure, BaiduFailure::Malformed);
    }

    #[test]
    fn an_api_errno_becomes_a_failure() {
        let failure = parse_list_response(r#"{"errno":-6,"errmsg":"用户未登录"}"#)
            .expect_err("errno must fail");

        assert_eq!(failure, BaiduFailure::Auth);
    }

    #[test]
    fn the_observed_uinfo_payload_is_parsed() {
        let info = parse_uinfo_response(
            r#"{"baidu_name":"风尚志段燚","errno":0,"uk":2688829335,"vip_type":2,"avatar_url":"https://example.invalid/a.png"}"#,
        )
        .expect("the probe payload must parse");

        assert_eq!(info.baidu_name, "风尚志段燚");
        assert_eq!(info.uk, 2688829335);
        assert_eq!(info.vip_type, 2);
        assert_eq!(
            info.avatar_url.as_deref(),
            Some("https://example.invalid/a.png")
        );
    }

    #[test]
    fn the_uinfo_payload_without_an_avatar_is_still_parsed() {
        let info = parse_uinfo_response(r#"{"baidu_name":"n","errno":0,"uk":1,"vip_type":0}"#)
            .expect("must parse");

        assert_eq!(info.avatar_url, None);
        assert_eq!(info.uk, 1);
    }

    #[test]
    fn a_uinfo_errno_becomes_a_failure() {
        let failure = parse_uinfo_response(r#"{"errno":-6,"errmsg":"用户未登录"}"#)
            .expect_err("errno must fail");

        assert_eq!(failure, BaiduFailure::Auth);
    }

    #[test]
    fn the_dlink_payload_is_parsed() {
        let link = parse_dlink_response(
            r#"{"errno":0,"dlink":"https://d.pcs.baidu.com/file/x","fs_id":7}"#,
        )
        .expect("must parse");

        assert_eq!(link, "https://d.pcs.baidu.com/file/x");
    }

    #[test]
    fn a_dlink_payload_without_a_link_is_malformed() {
        let failure = parse_dlink_response(r#"{"errno":0}"#).expect_err("no dlink");

        assert_eq!(failure, BaiduFailure::Malformed);
    }

    // ---- 条目映射 ------------------------------------------------------

    fn sample_entry() -> BaiduFileEntry {
        BaiduFileEntry {
            fs_id: 101,
            path: "/01A_软件".to_string(),
            server_filename: "01A_软件".to_string(),
            isdir: 1,
            size: 4096,
            server_mtime: 1_700_000_000,
            local_mtime: 0,
        }
    }

    #[test]
    fn the_modified_time_is_converted_from_seconds_to_millis() {
        let entry = to_remote_entry("conn-a", &sample_entry()).expect("must map");

        assert_eq!(entry.modified_time, 1_700_000_000_000);
    }

    #[test]
    fn a_directory_is_marked_as_a_directory() {
        let entry = to_remote_entry("conn-a", &sample_entry()).expect("must map");

        assert!(entry.is_dir);
    }

    #[test]
    fn the_entry_path_is_the_sfm_remote_path() {
        let entry = to_remote_entry("conn-a", &sample_entry()).expect("must map");

        assert_eq!(entry.path, "sfm://remote/conn-a/01A_软件");
    }

    #[test]
    fn the_name_falls_back_to_the_path_leaf() {
        let mut entry = sample_entry();
        entry.server_filename = String::new();

        let mapped = to_remote_entry("conn-a", &entry).expect("must map");
        assert_eq!(mapped.name, "01A_软件");
    }

    #[test]
    fn an_entry_without_any_name_is_skipped() {
        let mut entry = sample_entry();
        entry.server_filename = String::new();
        entry.path = "/".to_string();

        assert_eq!(to_remote_entry("conn-a", &entry), None);
    }

    #[test]
    fn a_dot_prefixed_name_is_hidden() {
        let mut entry = sample_entry();
        entry.server_filename = ".gitignore".to_string();

        let mapped = to_remote_entry("conn-a", &entry).expect("must map");
        assert!(mapped.is_hidden);
    }

    #[test]
    fn a_baidu_entry_is_never_a_symlink() {
        let entry = to_remote_entry("conn-a", &sample_entry()).expect("must map");

        assert!(!entry.is_symlink);
    }

    #[test]
    fn the_local_mtime_is_used_when_the_server_mtime_is_missing() {
        let mut entry = sample_entry();
        entry.server_mtime = 0;
        entry.local_mtime = 1_700_000_500;

        let mapped = to_remote_entry("conn-a", &entry).expect("must map");
        assert_eq!(mapped.modified_time, 1_700_000_500_000);
    }

    #[test]
    fn a_whole_page_is_mapped() {
        let entries = vec![sample_entry(), sample_entry()];
        let mapped = to_remote_entries("conn-a", &entries);

        assert_eq!(mapped.len(), 2);
        assert!(mapped
            .iter()
            .all(|entry| entry.path.starts_with("sfm://remote/conn-a/")));
    }

    #[test]
    fn the_seconds_conversion_saturates_instead_of_overflowing() {
        assert_eq!(baidu_seconds_to_unix_millis(u64::MAX), u64::MAX);
    }

    // ---- 错误分类 ------------------------------------------------------

    #[test]
    fn errno_minus_six_is_an_auth_failure() {
        assert_eq!(
            failure_from_errno(-6, Some("用户未登录")),
            BaiduFailure::Auth
        );
    }

    #[test]
    fn errno_minus_nine_is_a_not_found_failure() {
        assert_eq!(failure_from_errno(-9, None), BaiduFailure::NotFound);
    }

    #[test]
    fn errno_two_is_a_protocol_failure() {
        assert_eq!(failure_from_errno(2, None), BaiduFailure::Protocol);
    }

    #[test]
    fn an_unknown_errno_falls_back_to_the_message() {
        assert_eq!(
            failure_from_errno(-1, Some("目录不存在")),
            BaiduFailure::NotFound
        );
        assert_eq!(
            failure_from_errno(-1, Some("没有操作权限")),
            BaiduFailure::Permission
        );
        assert_eq!(
            failure_from_errno(-1, Some("参数错误")),
            BaiduFailure::Protocol
        );
    }

    #[test]
    fn an_unknown_errno_without_a_hint_is_unknown() {
        assert_eq!(failure_from_errno(-777, None), BaiduFailure::Unknown);
        assert_eq!(
            failure_from_errno(-777, Some("说不清")),
            BaiduFailure::Unknown
        );
    }

    #[test]
    fn http_401_and_403_are_auth_failures() {
        assert_eq!(failure_from_http_status(401), BaiduFailure::Auth);
        assert_eq!(failure_from_http_status(403), BaiduFailure::Auth);
    }

    #[test]
    fn a_redirect_to_the_login_page_is_an_auth_failure() {
        assert_eq!(failure_from_http_status(302), BaiduFailure::Auth);
        assert_eq!(failure_from_http_status(301), BaiduFailure::Auth);
    }

    #[test]
    fn http_404_is_a_not_found_failure() {
        assert_eq!(failure_from_http_status(404), BaiduFailure::NotFound);
    }

    #[test]
    fn rate_limiting_and_server_errors_are_unreachable() {
        assert_eq!(failure_from_http_status(429), BaiduFailure::Unreachable);
        assert_eq!(failure_from_http_status(503), BaiduFailure::Unreachable);
    }

    #[test]
    fn other_client_errors_are_protocol_failures() {
        assert_eq!(failure_from_http_status(400), BaiduFailure::Protocol);
    }

    #[test]
    fn a_connect_failure_is_unreachable() {
        let kind = classify_transport(TransportErrorShape {
            connect: true,
            ..Default::default()
        });

        assert_eq!(kind, RemoteErrorKind::Unreachable);
    }

    #[test]
    fn a_timeout_is_unreachable() {
        let kind = classify_transport(TransportErrorShape {
            timeout: true,
            ..Default::default()
        });

        assert_eq!(kind, RemoteErrorKind::Unreachable);
    }

    #[test]
    fn a_body_that_stops_mid_way_is_disconnected() {
        let kind = classify_transport(TransportErrorShape {
            body: true,
            ..Default::default()
        });

        assert_eq!(kind, RemoteErrorKind::Disconnected);
    }

    #[test]
    fn a_request_failure_is_a_protocol_failure() {
        let kind = classify_transport(TransportErrorShape {
            request: true,
            ..Default::default()
        });

        assert_eq!(kind, RemoteErrorKind::Protocol);
    }

    #[test]
    fn an_unrecognised_transport_failure_is_unknown() {
        let kind = classify_transport(TransportErrorShape::default());

        assert_eq!(kind, RemoteErrorKind::Unknown);
    }

    #[test]
    fn every_failure_maps_to_a_kind() {
        let pairs = [
            (BaiduFailure::Auth, RemoteErrorKind::Auth),
            (BaiduFailure::Permission, RemoteErrorKind::Permission),
            (BaiduFailure::NotFound, RemoteErrorKind::NotFound),
            (BaiduFailure::Protocol, RemoteErrorKind::Protocol),
            (BaiduFailure::Unreachable, RemoteErrorKind::Unreachable),
            (BaiduFailure::Disconnected, RemoteErrorKind::Disconnected),
            (BaiduFailure::Malformed, RemoteErrorKind::Protocol),
            (BaiduFailure::Encoding, RemoteErrorKind::Encoding),
            (BaiduFailure::Unsupported, RemoteErrorKind::Unsupported),
            (BaiduFailure::Unknown, RemoteErrorKind::Unknown),
        ];

        for (failure, expected) in pairs {
            assert_eq!(classify_baidu_failure(&failure), expected);
        }
    }

    #[test]
    fn the_coverage_table_covers_every_kind_exactly_once() {
        let coverage = baidu_error_kind_coverage();

        assert_eq!(coverage.len(), 11);

        let mut kinds: Vec<RemoteErrorKind> = coverage.iter().map(|row| row.kind).collect();
        kinds.sort_by_key(|kind| kind.as_str());

        let mut expected = vec![
            RemoteErrorKind::Auth,
            RemoteErrorKind::HostKeyUnknown,
            RemoteErrorKind::HostKeyChanged,
            RemoteErrorKind::Unreachable,
            RemoteErrorKind::Permission,
            RemoteErrorKind::NotFound,
            RemoteErrorKind::Protocol,
            RemoteErrorKind::Encoding,
            RemoteErrorKind::Disconnected,
            RemoteErrorKind::Unsupported,
            RemoteErrorKind::Unknown,
        ];
        expected.sort_by_key(|kind| kind.as_str());

        assert_eq!(kinds, expected);
    }

    #[test]
    fn every_kind_marked_producible_is_reachable_from_a_failure() {
        let mut reachable: Vec<RemoteErrorKind> = vec![
            BaiduFailure::Auth,
            BaiduFailure::Permission,
            BaiduFailure::NotFound,
            BaiduFailure::Protocol,
            BaiduFailure::Unreachable,
            BaiduFailure::Disconnected,
            BaiduFailure::Malformed,
            BaiduFailure::Encoding,
            BaiduFailure::Unsupported,
            BaiduFailure::Unknown,
        ]
        .iter()
        .map(classify_baidu_failure)
        .collect();
        reachable.sort_by_key(|kind| kind.as_str());
        reachable.dedup();

        let mut promised: Vec<RemoteErrorKind> = baidu_error_kind_coverage()
            .iter()
            .filter(|row| row.producible)
            .map(|row| row.kind)
            .collect();
        promised.sort_by_key(|kind| kind.as_str());

        assert_eq!(promised, reachable);
    }

    #[test]
    fn the_host_key_kinds_are_the_only_ones_marked_unproducible() {
        let unproducible: Vec<RemoteErrorKind> = baidu_error_kind_coverage()
            .iter()
            .filter(|row| !row.producible)
            .map(|row| row.kind)
            .collect();

        assert_eq!(
            unproducible,
            vec![
                RemoteErrorKind::HostKeyUnknown,
                RemoteErrorKind::HostKeyChanged
            ]
        );
    }

    #[test]
    fn every_coverage_row_explains_itself() {
        for row in baidu_error_kind_coverage() {
            assert!(!row.note.trim().is_empty(), "{:?} 缺一条说明", row.kind);
        }
    }

    #[test]
    fn the_error_messages_never_leak_the_cookie() {
        let session = BaiduSession::new("SECRET_BDUSS_VALUE".to_string(), None);
        let error = to_remote_error(&BaiduFailure::Auth);

        assert!(!error.message.contains("SECRET_BDUSS_VALUE"));
        assert!(!error.message.contains(&session.describe()));
        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    // ---- 会话 / Cookie -------------------------------------------------

    #[test]
    fn the_cookie_header_carries_the_bduss() {
        let session = BaiduSession::new("abc123".to_string(), None);

        assert_eq!(session.cookie_header().as_deref(), Some("BDUSS=abc123"));
    }

    #[test]
    fn the_cookie_header_includes_the_stoken_when_there_is_one() {
        let session = BaiduSession::new("abc123".to_string(), Some("token456".to_string()));

        assert_eq!(
            session.cookie_header().as_deref(),
            Some("BDUSS=abc123; STOKEN=token456")
        );
    }

    #[test]
    fn an_invalid_stoken_is_dropped_instead_of_failing_the_request() {
        let session = BaiduSession::new("abc123".to_string(), Some("bad value".to_string()));

        assert_eq!(session.cookie_header().as_deref(), Some("BDUSS=abc123"));
    }

    #[test]
    fn a_session_without_a_usable_bduss_has_no_cookie_header() {
        assert_eq!(BaiduSession::new("", None).cookie_header(), None);
        assert_eq!(BaiduSession::new("has space", None).cookie_header(), None);
    }

    #[test]
    fn cookie_values_are_trimmed_and_accepted_when_ordinary() {
        assert_eq!(
            sanitize_cookie_value("  abc123  "),
            Some("abc123".to_string())
        );
        assert_eq!(sanitize_cookie_value("a-b_c.d~e"), Some("a-b_c.d~e".into()));
    }

    #[test]
    fn cookie_values_with_separators_or_controls_are_rejected() {
        assert_eq!(sanitize_cookie_value("a;b"), None);
        assert_eq!(sanitize_cookie_value("a,b"), None);
        assert_eq!(sanitize_cookie_value("a\"b"), None);
        assert_eq!(sanitize_cookie_value("a\\b"), None);
        assert_eq!(sanitize_cookie_value("a\r\nb"), None);
        assert_eq!(sanitize_cookie_value("   "), None);
    }

    #[test]
    fn the_debug_output_never_prints_the_secret() {
        let session = BaiduSession::new("SECRET_BDUSS_VALUE".to_string(), Some("stoken".into()));

        let rendered = format!("{session:?}");
        assert!(!rendered.contains("SECRET_BDUSS_VALUE"), "{rendered}");
        assert!(!rendered.contains("stoken"), "{rendered}");
    }

    #[test]
    fn the_description_reports_lengths_only() {
        let session = BaiduSession::new("123456".to_string(), Some("7890".to_string()));

        assert_eq!(session.describe(), "BDUSS(len=6) + STOKEN(len=4)");
    }

    #[test]
    fn a_session_reports_whether_it_has_credentials() {
        assert!(BaiduSession::new("abc", None).has_credentials());
        assert!(!BaiduSession::new("", None).has_credentials());
    }
}
