// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! OneDrive（Microsoft Graph）纯函数层 —— 寻址、编码、翻页、错误分类。
//!
//! 红线（与 `remote/mod.rs:8-14` 同一条）：本文件被 `cargo test --lib` 的测试
//! 二进制链接，**不得出现 `tauri::` 的任何路径**。一旦碰到，Tauri 全局运行时
//! 的默认类型参数 `Wry` 被单态化，测试二进制会在**加载阶段**以
//! `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉。
//!
//! ## 官方依据
//!
//! 寻址与编码规则出自 Microsoft Graph 官方文档
//! `https://learn.microsoft.com/en-us/graph/onedrive-addressing-driveitems`：
//!
//! - 路径式寻址：`/me/drive/root:/<路径>:/children` 与
//!   `/me/drive/root:/<路径>:/content`；
//! - **逐段**百分号编码 —— 整条 URL 一次编码会让「哪一段装了什么」变得有歧义；
//! - 官方示例：`Adele's Files` → `Adele's%20Files`、`doc (1).docx` → `doc%20(1).docx`、
//!   `estimate%s.docx` → `estimate%25s.docx`、`Break#Out` → `Break%23Out`、
//!   `saved_game[1].bin` → `saved_game[1].bin`（**方括号不编码**）。
//!
//! ## 两条对官方 `pchar` 集的刻意收紧
//!
//! 1. `+` 编成 `%2B`。官方 `pchar` 允许 `+` 原样出现，但施工书要求编码；
//!    编不编都能解回同一个 `+`，编码只是更保守。
//! 2. `:` 编成 `%3A`。官方 `pchar` 允许 `:`，但 Graph 的路径式寻址拿 `root:`
//!    与 `:/` 当分隔符；文件名里留着裸 `:` 会让「路径」和「分隔符」有歧义。
//!    （OneDrive 本来也禁止文件名含 `:`，所以这是双保险。）

use serde::{Deserialize, Serialize};

use super::error::{RemoteError, RemoteErrorKind};
use super::path::{REMOTE_SCHEME, build_remote_path, join_remote_parent_and_name};
use super::provider::RemoteEntry;

/// Microsoft Graph 的版本化根地址。
pub const ONEDRIVE_GRAPH_BASE_URL: &str = "https://graph.microsoft.com/v1.0";

/// 「当前用户的主盘」地址。`/me/drive` 一次给回盘 id 与账号显示名。
pub const ONEDRIVE_GRAPH_DRIVE_URL: &str = "https://graph.microsoft.com/v1.0/me/drive";

/// Graph 不校验 UA，但发一个像样的总比发 Rust 的默认 UA 好。
pub const ONEDRIVE_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AlphaFileManager/1.0";

/// 单次列目录最多翻多少页。
///
/// Graph 的 `/children` 默认一页只给 200 项，**必须翻页**才拿得全。
/// 上限存在的理由与阿里云盘那道一样：宁可报错，也不让一个出问题的服务端
/// 把本进程变成一个无底洞。
pub const ONEDRIVE_MAX_LIST_PAGES: usize = 200;

/// 单次列目录最多收多少个条目。触顶 ⇒ 报错，不静默截断。
pub const ONEDRIVE_MAX_LIST_ENTRIES: usize = 100_000;

/// 单个文件的最大下载字节数（512 MiB）。
pub const ONEDRIVE_MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;

/// Graph 返回的一个 driveItem。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnedriveGraphItem {
    /// 条目的稳定 id。**必须**填进 `RemoteEntry::entry_id` ——
    /// 同一个目录里允许同名条目，光靠路径认不出是两个。
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub size: Option<u64>,
    /// ISO 8601 时间戳，形如 `2024-01-15T10:30:00Z`。
    #[serde(default)]
    pub last_modified_date_time: Option<String>,
    /// 有这个键就是目录。
    #[serde(default)]
    pub folder: Option<OnedriveGraphFolder>,
    /// 有这个键就是文件。
    #[serde(default)]
    pub file: Option<OnedriveGraphFile>,
    #[serde(default)]
    pub parent_reference: Option<OnedriveParentReference>,
}

/// driveItem 里的 `folder` 对象。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnedriveGraphFolder {
    #[serde(default)]
    pub child_count: Option<u32>,
}

/// driveItem 里的 `file` 对象。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnedriveGraphFile {
    #[serde(default)]
    pub mime_type: Option<String>,
}

/// driveItem 里的 `parentReference` 对象。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnedriveParentReference {
    /// 形如 `/drive/root:/Documents`。取相对路径要**剥掉这个前缀**。
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
}

/// `/children` 接口的一页。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnedriveChildrenPage {
    #[serde(default)]
    pub value: Vec<OnedriveGraphItem>,
    /// 续拉地址。里面带 `$skipToken`。
    #[serde(default, rename = "@odata.nextLink")]
    pub next_link: Option<String>,
}

/// `/me/drive` 的响应。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnedriveDriveInfo {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub drive_type: Option<String>,
    #[serde(default)]
    pub owner: Option<OnedriveOwner>,
}

/// `owner` 对象。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnedriveOwner {
    #[serde(default)]
    pub user: Option<OnedriveUser>,
}

/// `owner.user` 对象。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnedriveUser {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub user_principal_name: Option<String>,
}

impl OnedriveDriveInfo {
    /// 账号显示名。拿不到就 `None` —— **不影响**能否列目录。
    pub fn account_name(&self) -> Option<String> {
        let user = self.owner.as_ref()?.user.as_ref()?;
        user.display_name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .or_else(|| {
                user.user_principal_name
                    .clone()
                    .filter(|name| !name.trim().is_empty())
            })
    }
}

/// 一段字节区间（HTTP `Range` 请求头）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnedriveByteRange {
    pub start: u64,
    /// `None` 表示「到文件末尾」。
    pub end: Option<u64>,
}

impl OnedriveByteRange {
    /// 从某个偏移开始，取到末尾。
    pub fn from_start(start: u64) -> Self {
        Self { start, end: None }
    }

    /// `Range` 请求头的值。
    pub fn to_header_value(self) -> String {
        match self.end {
            Some(end) => format!("bytes={}-{}", self.start, end),
            None => format!("bytes={}-", self.start),
        }
    }
}

// ---------------------------------------------------------------------------
// 路径编码
// ---------------------------------------------------------------------------

/// 路径段里可以原样保留的字符。
///
/// 依据官方文档的 `pchar = unreserved / pct-encoded / sub-delims / ":" / "@"`，
/// 再按文件头的两条说明收紧（去掉 `+`、去掉 `:`）并补上官方示例里的 `[` `]`。
fn is_onedrive_safe_path_char(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(
            character,
            '-' | '.'
                | '_'
                | '~'
                | '!'
                | '$'
                | '&'
                | '\''
                | '('
                | ')'
                | '*'
                | ','
                | ';'
                | '='
                | '['
                | ']'
                | '@'
        )
}

/// 百分号编码**一个**路径段。
///
/// 非 ASCII 按 UTF-8 字节逐个编码（`%E4%B8%AD` 这种）。
pub fn onedrive_encode_path_segment(segment: &str) -> String {
    let mut encoded = String::with_capacity(segment.len());

    for byte in segment.as_bytes() {
        let character = *byte as char;
        if is_onedrive_safe_path_char(character) {
            encoded.push(character);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }

    encoded
}

/// 百分号编码**整条**路径（逐段编码，段间保留 `/`）。
pub fn onedrive_encode_path(path: &str) -> String {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .map(onedrive_encode_path_segment)
        .collect::<Vec<_>>()
        .join("/")
}

/// 剥掉 `parentReference.path` 的 `/drive/root:` 前缀。
///
/// `/drive/root:/Documents` → `/Documents`；`/drive/root:` → `/`。
/// 前缀不在时原样返回（只补齐开头的斜杠）。
pub fn onedrive_strip_drive_root_prefix(path: &str) -> String {
    let trimmed = path.trim();
    let remainder = trimmed.strip_prefix("/drive/root:").unwrap_or(trimmed);

    if remainder.is_empty() {
        return "/".to_string();
    }

    if remainder.starts_with('/') {
        remainder.to_string()
    } else {
        format!("/{remainder}")
    }
}

/// 列子项的地址。
///
/// 根目录走 `/me/drive/root/children`（**不是** `root:/:/children` ——
/// 后者是无效路径）。
pub fn onedrive_build_children_url(remote_path: &str) -> String {
    let encoded = onedrive_encode_path(remote_path);

    if encoded.is_empty() {
        format!("{ONEDRIVE_GRAPH_BASE_URL}/me/drive/root/children")
    } else {
        format!("{ONEDRIVE_GRAPH_BASE_URL}/me/drive/root:/{encoded}:/children")
    }
}

/// 取文件内容的地址。
pub fn onedrive_build_content_url(remote_path: &str) -> String {
    let encoded = onedrive_encode_path(remote_path);
    format!("{ONEDRIVE_GRAPH_BASE_URL}/me/drive/root:/{encoded}:/content")
}

// ---------------------------------------------------------------------------
// 翻页
// ---------------------------------------------------------------------------

/// 从 `@odata.nextLink` 里取 `$skipToken`。
///
/// 参数名大小写不敏感（OData 的 `$skipToken` 在不同接口里大小写不一）。
pub fn onedrive_skip_token_from_next_link(next_link: &str) -> Option<String> {
    let query = next_link.split_once('?')?.1;

    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };

        if !key.trim().eq_ignore_ascii_case("$skiptoken") {
            continue;
        }

        let Ok(decoded) = urlencoding::decode(value) else {
            continue;
        };

        let token = decoded.trim();
        if !token.is_empty() {
            return Some(token.to_string());
        }
    }

    None
}

/// 解析一页 `/children` 响应。
pub fn onedrive_parse_children_page(body: &str) -> Result<OnedriveChildrenPage, RemoteError> {
    serde_json::from_str::<OnedriveChildrenPage>(body).map_err(|error| {
        RemoteError::protocol(format!("OneDrive 返回的目录列表无法解析：{error}"))
    })
}

/// 解析 `/me/drive` 响应。
pub fn onedrive_parse_drive_info(body: &str) -> Result<OnedriveDriveInfo, RemoteError> {
    serde_json::from_str::<OnedriveDriveInfo>(body)
        .map_err(|error| RemoteError::protocol(format!("OneDrive 返回的账号信息无法解析：{error}")))
}

/// 把首页与后续页合并成一份完整列表。
///
/// `fetch` 由调用方提供（网络层或测试夹具）。**翻页的终止条件有三条**：
///
/// 1. 某页没有 `nextLink` ⇒ 收工；
/// 2. 同一个 `$skipToken` 出现两次 ⇒ 服务端异常，**报错而不是死循环**；
/// 3. 页数或条目数触顶 ⇒ 报错，**绝不静默截断**（截断出来的目录看着「少了几个
///    文件」，用户根本不知道）。
pub async fn onedrive_collect_children_pages<F, Fut>(
    first: OnedriveChildrenPage,
    mut fetch: F,
) -> Result<Vec<OnedriveGraphItem>, RemoteError>
where
    F: FnMut(String) -> Fut + Send,
    Fut: std::future::Future<Output = Result<OnedriveChildrenPage, RemoteError>> + Send,
{
    let mut items: Vec<OnedriveGraphItem> = Vec::new();
    let mut page = first;
    let mut seen_tokens: Vec<String> = Vec::new();
    let mut pages = 0usize;

    loop {
        pages += 1;
        if pages > ONEDRIVE_MAX_LIST_PAGES {
            return Err(RemoteError::unknown(format!(
                "OneDrive 目录列表超过 {ONEDRIVE_MAX_LIST_PAGES} 页，已中止翻页。"
            )));
        }

        let next_link = page.next_link.take();
        items.extend(std::mem::take(&mut page.value));

        if items.len() > ONEDRIVE_MAX_LIST_ENTRIES {
            return Err(RemoteError::unknown(format!(
                "OneDrive 目录列表超过 {ONEDRIVE_MAX_LIST_ENTRIES} 项，已中止翻页。"
            )));
        }

        let Some(next_link) = next_link.map(|link| link.trim().to_string()).filter(|link| !link.is_empty())
        else {
            break;
        };

        let Some(token) = onedrive_skip_token_from_next_link(&next_link) else {
            return Err(RemoteError::protocol(
                "OneDrive 的翻页链接里没有 skipToken，无法继续读取目录。",
            ));
        };

        if seen_tokens.iter().any(|seen| seen == &token) {
            return Err(RemoteError::protocol(
                "OneDrive 翻页游标重复，已中止（服务端返回了同一个 skipToken）。",
            ));
        }

        seen_tokens.push(token.clone());
        page = fetch(token).await?;
    }

    Ok(items)
}

// ---------------------------------------------------------------------------
// 错误分类
// ---------------------------------------------------------------------------

/// Graph 的错误码是否是我们**看得懂**的。
///
/// 看得懂 = 落在下面四组里。**只看 `error.code`，不看 `error.message`** ——
/// 消息里可能回显请求内容。
fn onedrive_error_code_is_known(code: &str) -> bool {
    let normalized = code.trim().to_ascii_lowercase();

    matches!(
        normalized.as_str(),
        // 令牌相关
        | "invalidauthtoken"
        | "expiredauthtoken"
        | "invalidauthenticationtoken"
        | "expiredauthenticationtoken"
        | "authenticationfailure"
        | "unauthorized_client"
        | "invalid_grant"
        | "invalid_token"
        // 权限相关
        | "insufficientclaims"
        | "accessdenied"
        | "restrictedaccess"
        | "forbidden"
        // 不存在
        | "itemnotfound"
        | "resourcenotfound"
        | "pathnotfound"
        | "notfound"
        // 限流
        | "quotalimitreached"
        | "activitylimitreached"
        | "toomanyrequests"
    )
}

/// 从错误响应里取**最深**的、看得懂的 `error.code`。
///
/// Graph 的错误可以嵌套：
/// `{"error":{"code":"invalidRequest","innererror":{"code":"itemNotFound"}}}`。
/// 外层的 `invalidRequest` 什么也说明不了，内层的 `itemNotFound` 才有用。
///
/// 「最深」指的是**最深那个看得懂的** —— 最深的那个看不懂时，退回上一层能看懂的。
pub fn onedrive_extract_error_code(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let error = value.get("error")?;

    let mut best: Option<String> = None;
    let mut cursor: Option<&serde_json::Value> = Some(error);

    while let Some(node) = cursor {
        if let Some(code) = node
            .get("code")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|code| !code.is_empty())
        {
            if onedrive_error_code_is_known(code) {
                best = Some(code.to_string());
            }
        }

        cursor = node.get("innererror");
    }

    best
}

/// HTTP 状态码 + `error.code` → [`RemoteErrorKind`]。
///
/// **状态码是主，`error.code` 是修正。**
///
/// ⚠️ 两条不能搞反：
/// - `403 insufficientClaims`（条件访问命中）**必须**归 `Permission`，
///   **绝不能**归 `Auth` —— 归了 `Auth` 就会触发「刷新令牌再试一次」，
///   而刷新解决不了条件访问，白白多打一次网络请求并把用户引向错误的处置。
/// - 只有令牌本身失效（`invalidAuthenticationToken` 之类）才归 `Auth`。
pub fn onedrive_graph_error_kind(status: u16, code: Option<&str>) -> RemoteErrorKind {
    if let Some(code) = code {
        let normalized = code.trim().to_ascii_lowercase();

        if matches!(
            normalized.as_str(),
            "invalidauthtoken"
                | "expiredauthtoken"
                | "invalidauthenticationtoken"
                | "expiredauthenticationtoken"
                | "authenticationfailure"
                | "unauthorized_client"
                | "invalid_grant"
                | "invalid_token"
        ) {
            return RemoteErrorKind::Auth;
        }

        if matches!(
            normalized.as_str(),
            "insufficientclaims" | "accessdenied" | "restrictedaccess" | "forbidden"
        ) {
            return RemoteErrorKind::Permission;
        }

        if matches!(
            normalized.as_str(),
            "itemnotfound" | "resourcenotfound" | "pathnotfound" | "notfound"
        ) {
            return RemoteErrorKind::NotFound;
        }

        if matches!(
            normalized.as_str(),
            "quotalimitreached" | "activitylimitreached" | "toomanyrequests"
        ) {
            // 限流**不是**协议错误，也不是不可达 —— 它需要的是「等一会儿」。
            return RemoteErrorKind::Unknown;
        }
    }

    match status {
        401 => RemoteErrorKind::Auth,
        403 => RemoteErrorKind::Permission,
        404 => RemoteErrorKind::NotFound,
        400 | 405 | 409 | 412 | 416 => RemoteErrorKind::Protocol,
        429 => RemoteErrorKind::Unknown,
        500..=599 => RemoteErrorKind::Unreachable,
        _ => RemoteErrorKind::Unknown,
    }
}

/// 把 Graph 的错误码洗成能安全进用户文案的形状。
///
/// 只留 ASCII 字母数字与 `_` `-`，长度封顶 —— 这样即便上游塞进来一段奇怪的东西，
/// 它也进不了用户可见的文案，更进不了日志。
fn sanitize_graph_code(code: &str) -> Option<String> {
    let cleaned: String = code
        .trim()
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || *character == '_' || *character == '-'
        })
        .take(64)
        .collect();

    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

/// 把一次 Graph 失败翻译成本项目的错误。
///
/// `retry_after` 是 `Retry-After` 请求头的原文（秒数或 HTTP date）。
/// 限流时把它带进文案 —— 官方明确要求「读 `Retry-After`，不要立即重试」。
pub fn onedrive_map_graph_error(
    status: u16,
    body: &str,
    retry_after: Option<&str>,
) -> RemoteError {
    let code = onedrive_extract_error_code(body);
    let kind = onedrive_graph_error_kind(status, code.as_deref());
    let safe_code = code.as_deref().and_then(sanitize_graph_code);

    let message = match kind {
        RemoteErrorKind::Auth => {
            "OneDrive 授权已失效，请重新登录后再试。".to_string()
        }
        RemoteErrorKind::Permission => {
            "OneDrive 拒绝了这次访问：这个账号没有该资源的权限（可能是条件访问策略拦下了）。\
             请换一个账号，或联系管理员开通权限。"
                .to_string()
        }
        RemoteErrorKind::NotFound => "OneDrive 上没有这个文件或目录。".to_string(),
        RemoteErrorKind::Unreachable => {
            "无法连接 OneDrive：网络不可用、DNS 解析失败或微软服务暂时不可用，请稍后重试。"
                .to_string()
        }
        RemoteErrorKind::Unknown if status == 429 => {
            // `Retry-After` 的两种形态语义不同（秒数 = 时长，HTTP date = 绝对时刻），
            // 所以这里统一换算成「还要等多少秒」再进文案。
            let wait = onedrive_parse_retry_after(retry_after, super::onedrive_auth::onedrive_now_unix());
            match wait {
                Some(seconds) => format!(
                    "OneDrive 触发了限流，请等待约 {seconds} 秒后再试（本次请求没有自动重试）。"
                ),
                None => "OneDrive 触发了限流，请稍后再试（本次请求没有自动重试）。".to_string(),
            }
        }
        _ => match safe_code {
            Some(code) => format!("OneDrive 拒绝了这次请求（Graph 错误码：{code}）。"),
            None => format!("OneDrive 拒绝了这次请求（HTTP {status}）。"),
        },
    };

    RemoteError::new(kind, message)
}

/// 解析 `Retry-After` → **还要等多少秒**。
///
/// 两种形态都收：**秒数**（`120`）与 **HTTP date**（`Wed, 21 Oct 2015 07:28:00 GMT`）。
/// 缺失或认不出时返回 `None`。
///
/// 两种形态的**语义不同**：秒数是「还要等多久」，HTTP date 是「到哪个时刻为止」。
/// 把解析结果直接当「等待秒数」用，HTTP date 形态会变成「等待约 1445412480 秒」
/// （48 年）。所以这里统一换算成**剩余秒数**：秒数原样返回，
/// HTTP date 减掉 `now_unix`（已经过去的时间算 0）。
pub fn onedrive_parse_retry_after(value: Option<&str>, now_unix: i64) -> Option<i64> {
    let raw = value.map(str::trim).filter(|value| !value.is_empty())?;

    if let Ok(seconds) = raw.parse::<i64>() {
        return if seconds >= 0 { Some(seconds) } else { None };
    }

    let parsed = httpdate::parse_http_date(raw).ok()?;
    let epoch = parsed
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;

    Some((epoch - now_unix).max(0))
}

// ---------------------------------------------------------------------------
// 时间
// ---------------------------------------------------------------------------

/// 民用日历日期 → 天数（自 1970-01-01 起）。
///
/// Howard Hinnant 的 `days_from_civil`：整数运算，无条件分支，无查表。
fn onedrive_days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let shifted_year = if month <= 2 { year - 1 } else { year };
    let era = if shifted_year >= 0 {
        shifted_year
    } else {
        shifted_year - 399
    } / 400;
    let year_of_era = shifted_year - era * 400;
    let shifted_month = (month + 9) % 12;
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era =
        year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;

    era * 146_097 + day_of_era - 719_468
}

/// ISO 8601 时间戳 → Unix 秒。
///
/// 收这几种形态：`2024-01-15T10:30:00Z`、`2024-01-15T10:30:00.1234567Z`、
/// `2024-01-15T10:30:00+08:00`。**没有时区标记时按 UTC 算。**
pub fn onedrive_iso8601_to_unix_seconds(raw: &str) -> Option<i64> {
    let text = raw.trim();
    let (date_part, rest) = text.split_once('T')?;

    let mut date_fields = date_part.split('-');
    let year: i64 = date_fields.next()?.parse().ok()?;
    let month: i64 = date_fields.next()?.parse().ok()?;
    let day: i64 = date_fields.next()?.parse().ok()?;
    if date_fields.next().is_some() {
        return None;
    }

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let (time_part, offset_seconds) = onedrive_split_utc_offset(rest)?;

    let mut time_fields = time_part.split(':');
    let hour: i64 = time_fields.next()?.parse().ok()?;
    let minute: i64 = time_fields.next()?.parse().ok()?;
    let seconds_field = time_fields.next()?;
    if time_fields.next().is_some() {
        return None;
    }

    let seconds: i64 = match seconds_field.split_once('.') {
        Some((whole, _fraction)) => whole.parse().ok()?,
        None => seconds_field.parse().ok()?,
    };

    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) || !(0..=60).contains(&seconds) {
        return None;
    }

    let days = onedrive_days_from_civil(year, month, day);

    Some(days * 86_400 + hour * 3_600 + minute * 60 + seconds - offset_seconds)
}

/// 拆出时区偏移（秒）。`Z` ⇒ 0；`+08:00` ⇒ `+28800`；没有标记 ⇒ 0。
fn onedrive_split_utc_offset(rest: &str) -> Option<(&str, i64)> {
    if let Some(stripped) = rest.strip_suffix('Z').or_else(|| rest.strip_suffix('z')) {
        return Some((stripped, 0));
    }

    let bytes = rest.as_bytes();
    for index in (0..bytes.len()).rev() {
        let character = bytes[index] as char;
        if character != '+' && character != '-' {
            continue;
        }

        let time_part = &rest[..index];
        let offset = &rest[index + 1..];
        let (hours, minutes) = offset.split_once(':')?;
        let hours: i64 = hours.parse().ok()?;
        let minutes: i64 = minutes.parse().ok()?;
        let total = hours * 3_600 + minutes * 60;

        return Some((time_part, if character == '+' { total } else { -total }));
    }

    Some((rest, 0))
}

// ---------------------------------------------------------------------------
// 写路径寻址（批次 40：OneDrive 从只读升级为读写）
// ---------------------------------------------------------------------------

/// 单次**简单上传**的字节上限（250 MiB）。
///
/// Graph 的 `PUT .../content` 简单上传上限就是 250 MiB；
/// 超过它必须走上传会话（`createUploadSession`）分块 PUT。
pub const ONEDRIVE_MAX_SIMPLE_UPLOAD_BYTES: usize = 250 * 1024 * 1024;

/// 上传会话的分块大小（5 MiB）。
///
/// Graph 要求分块大小是 320 KiB 的整数倍：5 MiB = 16 × 320 KiB ✓。
pub const ONEDRIVE_UPLOAD_CHUNK_BYTES: usize = 5 * 1024 * 1024;

/// 按路径取单个条目的地址（拿它的 `id`）。
///
/// 根目录走 `/me/drive/root`（**不是** `root:/:/` —— 那是无效路径）。
pub fn onedrive_build_item_url(remote_path: &str) -> String {
    let encoded = onedrive_encode_path(remote_path);

    if encoded.is_empty() {
        format!("{ONEDRIVE_GRAPH_BASE_URL}/me/drive/root")
    } else {
        format!("{ONEDRIVE_GRAPH_BASE_URL}/me/drive/root:/{encoded}")
    }
}

/// 上传内容的地址。
pub fn onedrive_build_upload_url(remote_path: &str) -> String {
    let encoded = onedrive_encode_path(remote_path);
    format!("{ONEDRIVE_GRAPH_BASE_URL}/me/drive/root:/{encoded}:/content")
}

/// 创建上传会话的地址（大文件分块上传用）。
pub fn onedrive_build_upload_session_url(remote_path: &str) -> String {
    let encoded = onedrive_encode_path(remote_path);
    format!("{ONEDRIVE_GRAPH_BASE_URL}/me/drive/root:/{encoded}:/createUploadSession")
}

/// 按条目 id 寻址的地址（改名、删除）。
pub fn onedrive_build_item_by_id_url(item_id: &str) -> String {
    format!(
        "{ONEDRIVE_GRAPH_BASE_URL}/me/drive/items/{}",
        urlencoding::encode(item_id.trim())
    )
}

/// 在**根目录**下创建子项的地址。
pub fn onedrive_build_root_children_url() -> String {
    format!("{ONEDRIVE_GRAPH_BASE_URL}/me/drive/root/children")
}

/// 在指定**父目录 id** 下创建子项的地址。
///
/// Graph 的「创建子项」只认父目录的 `id`，不认路径 —— 父目录要先查一次。
pub fn onedrive_build_children_by_id_url(parent_id: &str) -> String {
    format!(
        "{ONEDRIVE_GRAPH_BASE_URL}/me/drive/items/{}/children",
        urlencoding::encode(parent_id.trim())
    )
}

/// 解析单个 driveItem（`GET .../root:/path` 的响应）。
pub fn onedrive_parse_item(body: &str) -> Result<OnedriveGraphItem, RemoteError> {
    serde_json::from_str::<OnedriveGraphItem>(body)
        .map_err(|error| RemoteError::protocol(format!("OneDrive 返回的条目无法解析：{error}")))
}

/// 解析 `createUploadSession` 的响应，取出 `uploadUrl`。
pub fn onedrive_parse_upload_session(body: &str) -> Result<String, RemoteError> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|error| {
        RemoteError::protocol(format!("OneDrive 返回的上传会话无法解析：{error}"))
    })?;

    value
        .get("uploadUrl")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(|url| url.to_string())
        .ok_or_else(|| {
            RemoteError::protocol("OneDrive 没有给出上传会话地址，无法继续上传这个文件。")
        })
}

/// 新建目录的请求体。
///
/// `@microsoft.graph.conflictBehavior: "fail"` —— 同名目录**不覆盖**。
/// 静默覆盖同名目录是最不能出的错（用户以为新建了一个，其实是换掉了原来那个）。
pub fn onedrive_build_create_folder_body(name: &str) -> String {
    // `folder` facet 必须写成**空对象**。
    //
    // 写成 `{ "@odata.type": "#microsoft.graph.driveItem" }` 会被 Graph 判 400
    // （invalidRequest: "The type 'oneDrive.item' was found to be of kind 'Entity'
    // instead of the expected kind 'Complex'"）—— facet 里带 `@odata.type` 时，
    // 服务端要的是 Complex 类型的 facet（`#microsoft.graph.folder`），
    // 而 `driveItem` 是 Entity 类型，两者不兼容 ⇒ **目录根本没建出来**。
    // 实测（2026-11-24，Graph v1.0 /me/drive/root/children）：
    //   空 facet → 201 Created；带 driveItem facet → 400。
    serde_json::json!({
        "name": name,
        "folder": {},
        "@microsoft.graph.conflictBehavior": "fail"
    })
    .to_string()
}

/// 改名的请求体。
pub fn onedrive_build_rename_body(new_name: &str) -> String {
    serde_json::json!({ "name": new_name }).to_string()
}

/// 拆出父目录路径与条目名。
///
/// `/a/b/c` → `("/a/b", "c")`；`/c` → `("/", "c")`；`/` → `None`。
pub fn onedrive_split_parent_and_name(remote_path: &str) -> Option<(String, String)> {
    let trimmed = remote_path.trim();
    let index = trimmed.rfind('/')?;
    let name = trimmed[index + 1..].trim().to_string();

    if name.is_empty() {
        return None;
    }

    let parent = trimmed[..index].trim_end_matches('/');
    let parent = if parent.is_empty() {
        "/".to_string()
    } else {
        parent.to_string()
    };

    Some((parent, name))
}

// ---------------------------------------------------------------------------
// 条目转换
// ---------------------------------------------------------------------------

/// Graph 的 driveItem → 本项目的 `RemoteEntry`。
pub fn onedrive_graph_item_to_remote_entry(
    item: &OnedriveGraphItem,
    connection_id: &str,
    parent_remote_path: &str,
) -> RemoteEntry {
    let name = item.name.clone();
    let remote_path = join_remote_parent_and_name(parent_remote_path, &name);
    let path = build_remote_path(connection_id, &remote_path)
        .unwrap_or_else(|| format!("{REMOTE_SCHEME}/{connection_id}{remote_path}"));

    let modified_time = item
        .last_modified_date_time
        .as_deref()
        .and_then(onedrive_iso8601_to_unix_seconds)
        .filter(|seconds| *seconds >= 0)
        .map(|seconds| (seconds as u64).saturating_mul(1_000))
        .unwrap_or(0);

    RemoteEntry {
        name,
        path,
        is_dir: item.folder.is_some(),
        size: item.size.unwrap_or(0),
        modified_time,
        is_hidden: item.name.starts_with('.'),
        is_symlink: false,
        // 条目 id **必须**带上：同一个目录里允许同名条目。
        entry_id: if item.id.trim().is_empty() {
            None
        } else {
            Some(item.id.clone())
        },
        thumbnail_url: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- 路径编码 -----------------------------------------------------------

    #[test]
    fn graph_path_encoding_matches_the_official_examples() {
        // 官方文档 `onedrive-addressing-driveitems` 的示例表，逐条照抄。
        assert_eq!(
            onedrive_encode_path("Adele's Files"),
            "Adele's%20Files",
            "撇号与空格：撇号不编码，空格编成 %20"
        );
        assert_eq!(
            onedrive_encode_path("Adele's Files/doc (1).docx"),
            "Adele's%20Files/doc%20(1).docx",
            "括号不编码"
        );
        assert_eq!(
            onedrive_encode_path("Adele's Files/estimate%s.docx"),
            "Adele's%20Files/estimate%25s.docx",
            "百分号必须编码"
        );
        assert_eq!(
            onedrive_encode_path("Break#Out"),
            "Break%23Out",
            "井号必须编码"
        );
        assert_eq!(
            onedrive_encode_path("Break#Out/saved_game[1].bin"),
            "Break%23Out/saved_game[1].bin",
            "官方示例里方括号不编码"
        );
    }

    #[test]
    fn graph_path_encoding_covers_the_required_characters() {
        // 施工书点名的四个字符。
        assert_eq!(onedrive_encode_path_segment("a+b"), "a%2Bb", "加号必须编码");
        assert_eq!(onedrive_encode_path_segment("a#b"), "a%23b", "井号必须编码");
        assert_eq!(onedrive_encode_path_segment("a%b"), "a%25b", "百分号必须编码");
        assert_eq!(onedrive_encode_path_segment("a b"), "a%20b", "空格必须编码");
        assert_eq!(
            onedrive_encode_path_segment("saved_game[1].bin"),
            "saved_game[1].bin",
            "方括号不编码"
        );
    }

    #[test]
    fn graph_path_encoding_handles_non_ascii_by_utf8_bytes() {
        assert_eq!(onedrive_encode_path_segment("中"), "%E4%B8%AD");
        assert_eq!(onedrive_encode_path("文档/1.txt"), "%E6%96%87%E6%A1%A3/1.txt");
    }

    #[test]
    fn graph_paths_are_encoded_segment_by_segment() {
        // 分隔符 `/` 必须留在段之间 —— 整条 URL 一次编码会让路径寻址失效。
        assert_eq!(
            onedrive_encode_path("/Documents/My Files/"),
            "Documents/My%20Files",
            "首尾的空段要被丢掉，段间的斜杠保留"
        );
    }

    // ---- 寻址 ---------------------------------------------------------------

    #[test]
    fn children_urls_use_the_path_form_except_for_the_root() {
        assert_eq!(
            onedrive_build_children_url("/"),
            "https://graph.microsoft.com/v1.0/me/drive/root/children"
        );
        assert_eq!(
            onedrive_build_children_url("/Documents"),
            "https://graph.microsoft.com/v1.0/me/drive/root:/Documents:/children"
        );
        assert_eq!(
            onedrive_build_children_url("/Documents/My Files"),
            "https://graph.microsoft.com/v1.0/me/drive/root:/Documents/My%20Files:/children"
        );
    }

    #[test]
    fn content_urls_use_the_path_form() {
        assert_eq!(
            onedrive_build_content_url("/Documents/MyFile.xlsx"),
            "https://graph.microsoft.com/v1.0/me/drive/root:/Documents/MyFile.xlsx:/content"
        );
    }

    #[test]
    fn the_drive_root_prefix_is_stripped_from_parent_references() {
        assert_eq!(onedrive_strip_drive_root_prefix("/drive/root:/Documents"), "/Documents");
        assert_eq!(
            onedrive_strip_drive_root_prefix("/drive/root:/Documents/Sub"),
            "/Documents/Sub"
        );
        assert_eq!(onedrive_strip_drive_root_prefix("/drive/root:"), "/");
        assert_eq!(onedrive_strip_drive_root_prefix("/Documents"), "/Documents");
    }

    // ---- 翻页 ---------------------------------------------------------------

    #[test]
    fn the_skip_token_is_read_out_of_the_next_link() {
        let next_link = "https://graph.microsoft.com/v1.0/me/drive/root/children?$skipToken=abc%2Fdef";

        assert_eq!(
            onedrive_skip_token_from_next_link(next_link).as_deref(),
            Some("abc/def"),
            "skipToken 是百分号编码的，取出来必须解码"
        );
    }

    #[test]
    fn the_skip_token_parameter_name_is_case_insensitive() {
        let next_link = "https://graph.microsoft.com/v1.0/me/drive/root/children?$skiptoken=xyz";

        assert_eq!(
            onedrive_skip_token_from_next_link(next_link).as_deref(),
            Some("xyz")
        );
    }

    #[test]
    fn a_next_link_without_a_skip_token_yields_nothing() {
        assert_eq!(onedrive_skip_token_from_next_link("https://example.com/children"), None);
        assert_eq!(
            onedrive_skip_token_from_next_link("https://example.com/children?$top=5"),
            None
        );
    }

    const PAGE_ONE: &str = r#"{
        "value": [
            {"id": "1", "name": "a.txt", "size": 3, "file": {"mimeType": "text/plain"}},
            {"id": "2", "name": "b", "folder": {"childCount": 0}}
        ],
        "@odata.nextLink": "https://graph.microsoft.com/v1.0/me/drive/root/children?$skipToken=token-1"
    }"#;

    const PAGE_TWO: &str = r#"{
        "value": [
            {"id": "3", "name": "c.txt", "size": 5, "file": {"mimeType": "text/plain"}}
        ]
    }"#;

    #[tokio::test]
    async fn two_pages_are_collected_in_full_and_the_paging_stops() {
        let first = onedrive_parse_children_page(PAGE_ONE).expect("parse page one");
        let second = onedrive_parse_children_page(PAGE_TWO).expect("parse page two");

        let mut calls = 0usize;
        let items = onedrive_collect_children_pages(first, |token| {
            calls += 1;
            assert_eq!(token, "token-1", "续拉必须用上一页给的 skipToken");
            let page = second.clone();
            async move { Ok(page) }
        })
        .await
        .expect("collect");

        assert_eq!(items.len(), 3, "两页的条目必须一个不少");
        assert_eq!(
            items.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(),
            vec!["1", "2", "3"]
        );
        assert_eq!(calls, 1, "最后一页没有 nextLink ⇒ 必须停");
    }

    #[tokio::test]
    async fn a_repeated_skip_token_is_reported_instead_of_looping_forever() {
        let first = onedrive_parse_children_page(PAGE_ONE).expect("parse page one");

        let error = onedrive_collect_children_pages(first, |_token| async {
            Ok(onedrive_parse_children_page(PAGE_ONE).expect("parse page one"))
        })
        .await
        .expect_err("同一个 skipToken 不该被无限续拉");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
    }

    // ---- 错误分类 -----------------------------------------------------------

    #[test]
    fn graph_status_codes_map_to_error_kinds() {
        assert_eq!(onedrive_graph_error_kind(401, None), RemoteErrorKind::Auth);
        assert_eq!(
            onedrive_graph_error_kind(403, None),
            RemoteErrorKind::Permission
        );
        assert_eq!(onedrive_graph_error_kind(404, None), RemoteErrorKind::NotFound);
        assert_eq!(onedrive_graph_error_kind(400, None), RemoteErrorKind::Protocol);
        assert_eq!(onedrive_graph_error_kind(429, None), RemoteErrorKind::Unknown);
        assert_eq!(
            onedrive_graph_error_kind(503, None),
            RemoteErrorKind::Unreachable
        );
    }

    #[test]
    fn insufficient_claims_is_a_permission_problem_not_an_expired_token() {
        // 条件访问命中时 Graph 回 403 `insufficientClaims`。
        // **绝不能**归 Auth：归了就会去刷新令牌重试，而刷新解决不了条件访问。
        let error = onedrive_map_graph_error(
            403,
            r#"{"error":{"code":"insufficientClaims","message":"需要条件访问"}}"#,
            None,
        );

        assert_eq!(error.kind, RemoteErrorKind::Permission);
        assert_ne!(error.kind, RemoteErrorKind::Auth);
    }

    #[test]
    fn an_expired_token_is_reported_as_an_auth_problem() {
        let error = onedrive_map_graph_error(
            401,
            r#"{"error":{"code":"invalidAuthenticationToken"}}"#,
            None,
        );

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[test]
    fn the_error_code_is_read_from_the_error_object_only() {
        // `message` 里带什么都不能影响分类。
        let error = onedrive_map_graph_error(
            404,
            r#"{"error":{"code":"itemNotFound","message":"The resource could not be found"}}"#,
            None,
        );

        assert_eq!(error.kind, RemoteErrorKind::NotFound);
    }

    #[test]
    fn the_deepest_understandable_inner_error_code_wins() {
        let body = r#"{
            "error": {
                "code": "invalidRequest",
                "message": "The request is invalid",
                "innererror": {
                    "code": "itemNotFound",
                    "innererror": { "code": "someOpaqueCode" }
                }
            }
        }"#;

        assert_eq!(
            onedrive_extract_error_code(body).as_deref(),
            Some("itemNotFound"),
            "外层的 invalidRequest 什么也说明不了，内层的 itemNotFound 才有用"
        );
        assert_eq!(
            onedrive_graph_error_kind(400, onedrive_extract_error_code(body).as_deref()),
            RemoteErrorKind::NotFound
        );
    }

    #[test]
    fn an_unreadable_error_body_still_maps_by_status() {
        assert_eq!(onedrive_extract_error_code("not json at all"), None);
        assert_eq!(onedrive_extract_error_code("{}"), None);
        assert_eq!(
            onedrive_graph_error_kind(404, onedrive_extract_error_code("not json").as_deref()),
            RemoteErrorKind::NotFound
        );
    }

    // ---- Retry-After --------------------------------------------------------

    #[test]
    fn retry_after_accepts_seconds_and_http_dates() {
        // 秒数形态：原样就是「还要等多少秒」，与 now 无关。
        assert_eq!(onedrive_parse_retry_after(Some("120"), 0), Some(120));
        assert_eq!(onedrive_parse_retry_after(Some(" 30 "), 0), Some(30));
        // HTTP date 形态：绝对时刻 → 换算成剩余秒数。
        assert_eq!(
            onedrive_parse_retry_after(Some("Wed, 21 Oct 2015 07:28:00 GMT"), 1_445_412_380),
            Some(100),
            "HTTP date 必须换算成「还要等多少秒」，不能把绝对时刻当等待时长"
        );
        // 已经过去的时刻不该算出负的等待时间。
        assert_eq!(
            onedrive_parse_retry_after(Some("Wed, 21 Oct 2015 07:28:00 GMT"), 1_445_412_480),
            Some(0),
            "过去的时间点应该立刻重试（0 秒），不是负数"
        );
    }

    #[test]
    fn a_missing_or_unreadable_retry_after_yields_nothing() {
        assert_eq!(onedrive_parse_retry_after(None, 0), None);
        assert_eq!(onedrive_parse_retry_after(Some(""), 0), None);
        assert_eq!(onedrive_parse_retry_after(Some("   "), 0), None);
        assert_eq!(onedrive_parse_retry_after(Some("soon"), 0), None);
    }

    #[test]
    fn a_throttled_response_carries_the_wait_into_the_message() {
        let error = onedrive_map_graph_error(
            429,
            r#"{"error":{"code":"activityLimitReached"}}"#,
            Some("42"),
        );

        assert_eq!(error.kind, RemoteErrorKind::Unknown);
        assert!(
            error.message.contains("42"),
            "限流文案必须告诉用户要等多久：{}",
            error.message
        );
    }

    // ---- 时间 ---------------------------------------------------------------

    #[test]
    fn iso8601_timestamps_convert_to_unix_seconds() {
        assert_eq!(
            onedrive_iso8601_to_unix_seconds("1970-01-01T00:00:00Z"),
            Some(0)
        );
        assert_eq!(
            onedrive_iso8601_to_unix_seconds("2024-01-15T10:30:00Z"),
            Some(1_705_314_600)
        );
        assert_eq!(
            onedrive_iso8601_to_unix_seconds("2024-01-15T10:30:00.1234567Z"),
            Some(1_705_314_600),
            "小数秒要能被丢掉，不能让整条时间戳解析失败"
        );
        assert_eq!(
            onedrive_iso8601_to_unix_seconds("2024-01-15T10:30:00+08:00"),
            Some(1_705_285_800),
            "带时区偏移的要按偏移换算"
        );
        assert_eq!(onedrive_iso8601_to_unix_seconds("不是时间戳"), None);
    }

    // ---- 条目转换 -----------------------------------------------------------

    #[test]
    fn graph_items_become_remote_entries_with_their_item_id() {
        let item = OnedriveGraphItem {
            id: "item-42".to_string(),
            name: "报告 1.txt".to_string(),
            size: Some(12),
            last_modified_date_time: Some("2024-01-15T10:30:00Z".to_string()),
            file: Some(OnedriveGraphFile::default()),
            ..OnedriveGraphItem::default()
        };

        let entry = onedrive_graph_item_to_remote_entry(&item, "conn-a", "/Documents");

        assert_eq!(entry.name, "报告 1.txt");
        assert_eq!(entry.path, "sfm://remote/conn-a/Documents/报告 1.txt");
        assert!(!entry.is_dir);
        assert_eq!(entry.size, 12);
        assert_eq!(entry.modified_time, 1_705_314_600_000);
        assert_eq!(entry.entry_id.as_deref(), Some("item-42"));
        assert!(!entry.is_hidden);
    }

    #[test]
    fn a_folder_item_is_marked_as_a_directory() {
        let item = OnedriveGraphItem {
            id: "item-7".to_string(),
            name: "子目录".to_string(),
            folder: Some(OnedriveGraphFolder::default()),
            ..OnedriveGraphItem::default()
        };

        let entry = onedrive_graph_item_to_remote_entry(&item, "conn-a", "/");

        assert!(entry.is_dir);
        assert_eq!(entry.path, "sfm://remote/conn-a/子目录");
    }

    #[test]
    fn byte_ranges_render_as_range_headers() {
        assert_eq!(OnedriveByteRange::from_start(0).to_header_value(), "bytes=0-");
        assert_eq!(
            OnedriveByteRange {
                start: 0,
                end: Some(1023)
            }
            .to_header_value(),
            "bytes=0-1023"
        );
    }

    // ---- 账号信息 -----------------------------------------------------------

    #[test]
    fn the_drive_info_carries_the_account_name() {
        let body = r#"{
            "id": "drive-1",
            "driveType": "personal",
            "owner": { "user": { "displayName": "张三" } }
        }"#;

        let info = onedrive_parse_drive_info(body).expect("parse drive info");

        assert_eq!(info.id.as_deref(), Some("drive-1"));
        assert_eq!(info.account_name().as_deref(), Some("张三"));
    }
}

#[cfg(test)]
mod write_path_tests {
    use super::*;

    // ---- 写路径寻址 ---------------------------------------------------------

    #[test]
    fn write_urls_use_the_path_form() {
        assert_eq!(
            onedrive_build_item_url("/Documents/MyFile.xlsx"),
            "https://graph.microsoft.com/v1.0/me/drive/root:/Documents/MyFile.xlsx"
        );
        assert_eq!(
            onedrive_build_item_url("/"),
            "https://graph.microsoft.com/v1.0/me/drive/root"
        );
        assert_eq!(
            onedrive_build_upload_url("/Documents/MyFile.xlsx"),
            "https://graph.microsoft.com/v1.0/me/drive/root:/Documents/MyFile.xlsx:/content"
        );
        assert_eq!(
            onedrive_build_upload_session_url("/Documents/MyFile.bin"),
            "https://graph.microsoft.com/v1.0/me/drive/root:/Documents/MyFile.bin:/createUploadSession"
        );
        assert_eq!(
            onedrive_build_item_by_id_url("item-1"),
            "https://graph.microsoft.com/v1.0/me/drive/items/item-1"
        );
        assert_eq!(
            onedrive_build_root_children_url(),
            "https://graph.microsoft.com/v1.0/me/drive/root/children"
        );
        assert_eq!(
            onedrive_build_children_by_id_url("parent-1"),
            "https://graph.microsoft.com/v1.0/me/drive/items/parent-1/children"
        );
    }

    #[test]
    fn write_paths_are_encoded_segment_by_segment() {
        assert_eq!(
            onedrive_build_upload_url("/My Files/a b.txt"),
            "https://graph.microsoft.com/v1.0/me/drive/root:/My%20Files/a%20b.txt:/content"
        );
    }

    #[test]
    fn parent_and_name_are_split_out_of_a_path() {
        assert_eq!(
            onedrive_split_parent_and_name("/Documents/sub"),
            Some(("/Documents".to_string(), "sub".to_string()))
        );
        assert_eq!(
            onedrive_split_parent_and_name("/sub"),
            Some(("/".to_string(), "sub".to_string()))
        );
        assert_eq!(onedrive_split_parent_and_name("/"), None);
    }

    #[test]
    fn a_new_folder_never_silently_replaces_an_existing_one() {
        let body = onedrive_build_create_folder_body("sub");

        assert!(
            body.contains("conflictBehavior") && body.contains("fail"),
            "同名必须是 fail 而不是 replace：{body}"
        );
        assert!(
            !body.contains("replace"),
            "同名目录绝不能被静默覆盖：{body}"
        );
    }

    #[test]
    fn a_new_folder_body_uses_an_empty_folder_facet() {
        let body = onedrive_build_create_folder_body("sub");

        // 带 `@odata.type` 的 facet 会被 Graph 判 400（Entity vs Complex），
        // 目录压根没建出来。facet 必须是空对象。
        assert!(
            !body.contains("#microsoft.graph.driveItem"),
            "folder facet 不能带 @odata.type=#microsoft.graph.driveItem，Graph 会判 400：{body}"
        );
        assert!(
            body.contains("\"folder\":{}"),
            "folder facet 必须是空对象：{body}"
        );
    }

    #[test]
    fn a_rename_body_carries_only_the_new_name() {
        assert_eq!(onedrive_build_rename_body("b.txt"), "{\"name\":\"b.txt\"}");
    }

    #[test]
    fn an_upload_session_without_a_url_is_an_error() {
        assert!(onedrive_parse_upload_session("{}").is_err());
        assert_eq!(
            onedrive_parse_upload_session("{\"uploadUrl\":\"https://upload.example/s/1\"}")
                .expect("must parse"),
            "https://upload.example/s/1"
        );
    }

    #[test]
    fn a_drive_item_is_parsed_into_its_id_and_kind() {
        let item = onedrive_parse_item(
            "{\"id\":\"item-1\",\"name\":\"sub\",\"folder\":{\"childCount\":0}}",
        )
        .expect("must parse");

        assert_eq!(item.id, "item-1");
        assert_eq!(item.name, "sub");
        assert!(item.folder.is_some());
        assert!(item.file.is_none());
    }
}
