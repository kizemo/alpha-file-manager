// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! 阿里云盘开放平台（OpenAPI）的**纯函数层**。
//!
//! 红线（与 `remote/mod.rs:8-14` 同一条）：本文件被 `cargo test --lib` 的测试
//! 二进制链接，**不得出现 `tauri::AppHandle`、`tauri::async_runtime::*` 或
//! `tauri::` 的任何路径**。
//!
//! # 分层
//!
//! - **本文件**：端点常量、请求体构造、响应解析、错误映射、分页、容错反序列化。
//!   **不碰 reqwest，不碰网络**，全部是可单测的纯函数。
//! - **`remote/aliyun_provider.rs`**：HTTP 层。`HttpAliyunApi` 负责真的发请求，
//!   把本文件的纯函数串起来。
//! - **`remote/aliyun_auth.rs`**：会话结构、PKCE、过期判定、DPAPI 落盘。
//!
//! # 容错反序列化的理由
//!
//! 契约 §5.4：**一个字段读不出来，不能让整页列表失败**。
//!
//! 服务端某天多返回一个字段、少返回一个字段、把 `size` 从数字变成字符串 ——
//! 这些都不该让用户的文件列表变成空白页。所以本层不直接 `serde_json::from_str`
//! 到强类型结构体上，而是先落到 `serde_json::Value`，再逐字段**容错提取**：
//! 拿不到就退化成默认值（`None` / `0` / `false`），只有「整体不是 JSON 对象」
//! 才算解析失败。
//!
//! # 错误文案纪律
//!
//! 契约 §6.1 锚点 9：**错误文案绝不含 token / code / refresh_token**。
//!
//! 因此服务端返回的 `message` 字段**不进** `RemoteError.message` —— 它由服务端
//! 控制，指不定里面就夹着某个敏感串。只保留 HTTP 状态码这类中性信息。
//! 服务端的原始错误码与请求编号留在 [`AliyunFailure`] 里，供上层记日志用。

use serde_json::Value;

use crate::remote::error::{RemoteError, RemoteErrorKind};
use crate::remote::path::{build_remote_path, join_remote_parent_and_name};
use crate::remote::provider::RemoteEntry;
use crate::remote::sftp_provider::is_hidden_remote_name;

// ===========================================================================
// 端点与常量（契约 §1.1 / §1.3）
// ===========================================================================

// ---------------------------------------------------------------------------
// 端点分支：web 分支（免密钥，**本轮默认**）与 PDS OpenAPI 分支（要密钥）
// ---------------------------------------------------------------------------
//
// 证据：`.superpowers/sdd/b27-probe.md`（U1–U5 判定）、`b28-bundle-grep.md`
// （bundle 源码里的 `preferPDSOpenApi` 开关）。
//
// - **web 分支**：`api.aliyundrive.com`。网页端自带公开 `client_id`，换 token
//   **不需要** `client_secret`；请求鉴权只有一条 `Authorization: Bearer <token>`。
// - **PDS OpenAPI 分支**：`openapi.alipan.com`。要自备应用与 `client_secret`，
//   端点前缀（`/adrive/v1.0/openFile/…`）与版本都不同。
//
// ⇒ 两套常量成组放在一起，靠 [`ALIYUN_USE_PDS_OPEN_API`] 一个开关切换，
// 结构保留、默认走 web 分支。

/// 端点分支：`true` = PDS OpenAPI 分支（需要 `client_secret`），
/// `false` = web 分支（免密钥，**本轮默认**）。
pub const ALIYUN_USE_PDS_OPEN_API: bool = false;

/// 阿里云盘 **web 分支**的基址（免密钥）。
pub const ALIYUN_API_BASE_URL: &str = "https://api.aliyundrive.com";

/// 网盘连接的 host 恒为这个值。
pub const ALIYUN_PAN_HOST: &str = "api.aliyundrive.com";

/// web 分支的授权页。
pub const ALIYUN_AUTHORIZE_URL: &str = "https://auth.alipan.com/v2/oauth/authorize";

/// web 分支换 token：`POST {sv_endpoint}/token/get`。
///
/// 请求体是 **JSON**（bundle 原文：`post(sv_endpoint+"/token/get", {client_id, code})`），
/// 字段只有 `client_id` + `code`，**没有 `client_secret`**。
pub const ALIYUN_ACCESS_TOKEN_URL: &str = "https://api.aliyundrive.com/token/get";

/// web 分支刷新 token。
///
/// ⚠️ **未坐实**：web 分支的刷新端点没有实测过（批次 27 未关闭项 5）。
/// 刷新失败时明确报「请重新授权」，不影响只读主流程。
pub const ALIYUN_REFRESH_TOKEN_URL: &str = "https://api.aliyundrive.com/token/refresh";

/// web 分支列目录。
pub const ALIYUN_LIST_URL: &str = "https://api.aliyundrive.com/adrive/v3/file/list";

/// web 分支取条目元数据。
pub const ALIYUN_GET_URL: &str = "https://api.aliyundrive.com/adrive/v2/file/get";

/// web 分支取下载直链（**v2**）。
///
/// 坐实（`b28-api-probe.md`）：`POST /adrive/v2/file/get_download_url`，
/// body `{drive_id, file_id}` ⇒ 响应含 `url` / `expiration` / `size` / `crc64_hash`。
pub const ALIYUN_DOWNLOAD_URL_API: &str =
    "https://api.aliyundrive.com/adrive/v2/file/get_download_url";

/// 取当前用户信息（**坐实**）：`POST /v2/user/get`，body `{}`。
///
/// 响应里有 `default_drive_id`（**主盘**）/ `default_sbox_drive_id`（保险箱，
/// 403 `ForbiddenDriveLocked`）/ `user_id` / `nick_name` / `role` / `status`。
pub const ALIYUN_USER_INFO_URL: &str = "https://api.aliyundrive.com/v2/user/get";

/// 取网盘信息（**坐实**）：`POST /v2/drive/get`，body `{drive_id}`。
pub const ALIYUN_DRIVE_INFO_URL: &str = "https://api.aliyundrive.com/v2/drive/get";

/// 扫码登录：取二维码（**坐实**）。
pub const ALIYUN_QR_GENERATE_URL: &str = "https://passport.alipan.com/newlogin/qrcode/generate.do";

/// 扫码登录：轮询状态（**坐实**）。
pub const ALIYUN_QR_QUERY_URL: &str = "https://passport.alipan.com/newlogin/qrcode/query.do";

/// 下载直链回源时**必须**带的 `Referer`。
///
/// 坐实（`b28-api-probe.md`）：少了它 ⇒ HTTP 400 `AuthorizationArgumentError`；
/// 带上它再加 `Range: bytes=0-1023` ⇒ HTTP 206（精确 1024 字节）。
/// 加 `Authorization` **替代不了**它。
pub const ALIYUN_DOWNLOAD_REFERER: &str = "https://www.alipan.com/";

// ---- PDS OpenAPI 分支（保留结构，本轮默认不走） ---------------------------

/// PDS OpenAPI 分支的基址。
pub const ALIYUN_PDS_API_BASE_URL: &str = "https://openapi.alipan.com";

/// PDS 分支的授权页。
pub const ALIYUN_PDS_AUTHORIZE_URL: &str = "https://openapi.alipan.com/oauth/authorize";

/// PDS 分支的换 token 端点。
pub const ALIYUN_PDS_ACCESS_TOKEN_URL: &str = "https://openapi.alipan.com/oauth/access_token";

/// PDS 分支的列目录端点。
pub const ALIYUN_PDS_LIST_URL: &str = "https://openapi.alipan.com/adrive/v1.0/openFile/list";

/// PDS 分支的取元数据端点。
pub const ALIYUN_PDS_GET_URL: &str = "https://openapi.alipan.com/adrive/v1.0/openFile/get";

/// PDS 分支的取下载直链端点。
pub const ALIYUN_PDS_DOWNLOAD_URL_API: &str =
    "https://openapi.alipan.com/adrive/v1.0/openFile/getDownloadUrl";

/// 网页端自带的**公开** `client_id`（web 分支用它换 token，免密钥）。
pub const ALIYUN_WEB_CLIENT_ID: &str = "25dzX3vbYqktVxyX";

/// **没有**「默认主盘」常量。
///
/// 批次 28 真机探针（`.superpowers/sdd/b28-api-probe.md`）推翻了批次 27 的结论：
/// - `954264082` 是 **resource 盘**（根目录只有 2 项），**不是**主盘；
/// - `25998870` 是**保险箱**（`default_sbox_drive_id`），列目录回 403
///   `ForbiddenDriveLocked`；
/// - 真正的主盘是 `POST /v2/user/get` 返回的 `default_drive_id`（根目录 13 项）。
///
/// ⇒ 主盘 id **只**在登录时从 `/v2/user/get` 取，存进会话的 `drive_id`；
/// 取不到就明确报错，**绝不**拿一个常量顶上（那会列出错的目录还不自知）。
///
/// 覆盖默认 `drive_id` 的环境变量名。
pub const ALIYUN_DRIVE_ID_ENV: &str = "ALIYUN_DRIVE_ID";

/// 默认回调地址（web 分支固定为网页端的回调）。
pub const ALIYUN_DEFAULT_REDIRECT_URI: &str = "https://www.alipan.com/sign/callback";

/// 只读所需的 scope 集。**web 分支的 scope 未实测 ⇒ 留空（不传这个参数）。**
pub const ALIYUN_DEFAULT_SCOPE: &str = "";

/// 根目录的 `parent_file_id` 恒为这个字符串。
pub const ALIYUN_ROOT_FILE_ID: &str = "root";

/// 列目录响应里要回哪些字段（`jsonmask`）。
///
/// 逐字来自 bundle 源码（`b28-bundle-grep.md` 命中 `streams_info` 那一段）。
/// 少一个字段就少一份元数据；多一个服务端可能不认 ⇒ **照抄，不增不减**。
pub const ALIYUN_LIST_JSONMASK_FIELDS: &[&str] = &[
    "name",
    "file_id",
    "drive_id",
    "type",
    "size",
    "created_at",
    "updated_at",
    "category",
    "file_extension",
    "parent_file_id",
    "mime_type",
    "starred",
    "thumbnail",
    "url",
    "streams_info",
    "content_hash",
    "user_tags",
    "user_meta",
    "trashed",
    "video_media_metadata",
    "video_preview_metadata",
    "sync_meta",
    "sync_device_flag",
    "sync_flag",
    "punish_flag",
    "from_share_id",
];

/// 单次列目录最多翻多少页（裁定第 5 条：硬上限）。
///
/// 100 页 × 100 条 = 1 万条，远超任何真实目录。设上限是为了防住
/// 「服务端一直返回满页」造成的无限循环 —— 没有上限的话，一个坏掉的响应
/// 会让界面永远转圈，而界面上看不出任何异常。
///
/// **触顶 ⇒ 报错，绝不静默截断。** 静默截断等于告诉用户「这就是全部」，
/// 而实际上目录里还有条目 —— 用户会以为文件丢了。
pub const ALIYUN_MAX_LIST_PAGES: u32 = 100;

/// 下载直链响应里可能出现的字段，**按 bundle 的提升顺序**排列。
///
/// 原文：`WY("/file/get_download_url", e, {encryptedFields:["encrypt_url","encrypt_cdn_url"]})`
/// ⇒ 优先拿明文的 `url`，拿不到再退到加密的 `encrypt_url` / `encrypt_cdn_url`。
/// 中间插了 `streams_url`（分片流直链）。
pub const ALIYUN_DOWNLOAD_URL_FIELDS: &[&str] = &[
    "url",
    "encrypt_url",
    "streams_url",
    "cdn_url",
    "encrypt_cdn_url",
];

/// 单次列目录最多收多少个条目（裁定第 5 条：硬上限）。
///
/// 与页数上限双保险：页数没超但单页返回量异常大时也能收住。
/// 触顶同样 ⇒ 报错，不静默截断。
pub const ALIYUN_MAX_LIST_ENTRIES: usize = 10_000;

/// `limit` 的下界。
pub const ALIYUN_LIST_LIMIT_MIN: u32 = 1;

/// `limit` 的上界。**超范围服务端会静默截断 ⇒ 漏条目且不报错**（契约 §1.3）。
pub const ALIYUN_LIST_LIMIT_MAX: u32 = 100;

/// 默认每页条数。取坐实请求里的值（`b28-final-raw.md` §2）。
pub const ALIYUN_LIST_LIMIT_DEFAULT: u32 = 20;

/// 默认排序字段。坐实请求用的是 `updated_at`。
pub const ALIYUN_LIST_ORDER_BY: &str = "updated_at";

/// 默认排序方向。坐实请求用的是 `DESC`。
pub const ALIYUN_LIST_ORDER_DIRECTION: &str = "DESC";

/// 列表里 `url` 字段的过期秒数（坐实请求：14400）。
pub const ALIYUN_LIST_URL_EXPIRE_SEC: i64 = 14_400;

/// 条目类型：文件。
pub const ALIYUN_ENTRY_TYPE_FILE: &str = "file";

/// 条目类型：目录。
pub const ALIYUN_ENTRY_TYPE_FOLDER: &str = "folder";

/// 官方已知错误码（契约 §1.2）。出现这些 ⇒ 授权/应用配置问题 ⇒ 一律归为 Auth。
pub const ALIYUN_AUTH_ERROR_CODES: &[&str] = &[
    "AppNotExists",
    "InvalidClientSecret",
    "InvalidCode",
    "InvalidClientId",
    "InvalidGrantType",
];

/// 「文件不存在」类的错误码（出现在 HTTP 400 的响应体里）。
pub const ALIYUN_NOT_FOUND_ERROR_CODES: &[&str] = &["NotFound.File", "NotFound"];

/// 「已存在」类的错误码（出现在 HTTP 400 的响应体里）。
pub const ALIYUN_ALREADY_EXIST_ERROR_CODES: &[&str] = &["AlreadyExist.File", "AlreadyExist"];

// ===========================================================================
// 响应结构
// ===========================================================================

/// 错误响应体：`{code, message, requestId}`。
///
/// 三个字段**全部容错**：任意一个读不出来都不影响别的字段。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AliyunFailure {
    /// 服务端错误码，例如 `InvalidCode`。**只给日志看，不进用户可见文案。**
    pub code: Option<String>,
    /// 服务端错误描述。**只给日志看，不进用户可见文案。**
    pub message: Option<String>,
    /// 请求编号，方便用户反馈时定位。
    pub request_id: Option<String>,
}

/// 换 token 的响应体（契约 §1.2）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AliyunTokenPayload {
    /// 访问凭证。
    pub access_token: String,
    /// 刷新凭证。PKCE 公开客户端模式可能没有。
    pub refresh_token: Option<String>,
    /// access_token 有效秒数。
    pub expires_in: Option<i64>,
    /// refresh_token 有效秒数。
    pub refresh_token_expires_in: Option<i64>,
    /// 凭证类型，通常是 `Bearer`。
    pub token_type: Option<String>,
    /// 空间 ID。**不是主盘 id** ⇒ 不拿它当 `drive_id` 用（见
    /// [`aliyun_resolve_drive_id`]）。
    pub drive_id: Option<String>,
    /// 用户 ID。
    pub uid: Option<String>,
    /// 账号昵称。字段名可能是 `user_name` 或 `nick_name` ⇒ 两个都读。
    pub nick_name: Option<String>,
}

/// 列表里的一个条目（契约 §1.3 的 `items[]`）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AliyunFileEntry {
    /// 条目 ID。
    pub file_id: String,
    /// 文件名（不含目录）。
    pub name: String,
    /// `file` / `folder`。
    pub entry_type: String,
    /// 字节数。
    pub size: Option<u64>,
    /// 创建时间（ISO8601 原文）。
    pub created_at: Option<String>,
    /// 修改时间（ISO8601 原文）。
    pub updated_at: Option<String>,
    /// 是否隐藏。
    pub hidden: Option<bool>,
    /// 父目录 ID。
    pub parent_file_id: Option<String>,
    /// 空间 ID。
    pub drive_id: Option<String>,
    /// 是否收藏。
    pub starred: Option<bool>,
    /// 条目状态。
    pub status: Option<String>,
}

/// 列目录的响应体。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AliyunListPayload {
    /// 条目列表。
    pub items: Vec<AliyunFileEntry>,
    /// 游标。非空 ⇒ 还有下一页。
    pub next_marker: Option<String>,
}

/// **一页**列表的结果（[`aliyun_fetch_list_page`] 的返回值）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AliyunListPage {
    /// 这一页的条目。
    pub items: Vec<AliyunFileEntry>,
    /// 下一页的游标。非空 ⇒ 还有下一页。
    pub next_marker: Option<String>,
}

// ===========================================================================
// 容错取值
// ===========================================================================

/// 取字符串字段。数字也接受（转成字符串）。
fn lenient_str(value: &Value, key: &str) -> Option<String> {
    match value.get(key) {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Number(number)) => Some(number.to_string()),
        _ => None,
    }
}

/// 取整数字段。字符串数字也接受。
fn lenient_i64(value: &Value, key: &str) -> Option<i64> {
    match value.get(key) {
        Some(Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|v| v as i64)),
        Some(Value::String(text)) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// 取无符号整数字段。负数按「读不出来」处理。
fn lenient_u64(value: &Value, key: &str) -> Option<u64> {
    lenient_i64(value, key).and_then(|number| u64::try_from(number).ok())
}

/// 取布尔字段。`0` / `1` 也接受。
fn lenient_bool(value: &Value, key: &str) -> Option<bool> {
    match value.get(key) {
        Some(Value::Bool(flag)) => Some(*flag),
        Some(Value::Number(number)) => number.as_i64().map(|value| value != 0),
        Some(Value::String(text)) => match text.trim() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// 把一个 JSON 值容错地转成 [`AliyunFileEntry`]。
///
/// 任何字段缺失或类型不合预期，都退化成默认值，**不返回错误**。
pub fn aliyun_entry_from_value(value: &Value) -> AliyunFileEntry {
    AliyunFileEntry {
        file_id: lenient_str(value, "file_id").unwrap_or_default(),
        name: lenient_str(value, "name").unwrap_or_default(),
        entry_type: lenient_str(value, "type").unwrap_or_default(),
        size: lenient_u64(value, "size"),
        created_at: lenient_str(value, "created_at"),
        updated_at: lenient_str(value, "updated_at"),
        hidden: lenient_bool(value, "hidden"),
        parent_file_id: lenient_str(value, "parent_file_id"),
        drive_id: lenient_str(value, "drive_id"),
        starred: lenient_bool(value, "starred"),
        status: lenient_str(value, "status"),
    }
}

/// 解析错误响应体。解析不出来就返回空的 [`AliyunFailure`]（不报错）。
pub fn aliyun_parse_failure(body: &str) -> AliyunFailure {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return AliyunFailure::default();
    };

    if !value.is_object() {
        return AliyunFailure::default();
    }

    AliyunFailure {
        code: lenient_str(&value, "code"),
        message: lenient_str(&value, "message"),
        request_id: lenient_str(&value, "request_id").or_else(|| lenient_str(&value, "requestId")),
    }
}

// ===========================================================================
// 错误映射（契约 §1.6）
// ===========================================================================

/// HTTP 状态 + 响应体错误码 ⇒ [`RemoteErrorKind`]。
///
/// | 状态 | 归类 |
/// |---|---|
/// | 401 / 403 | Auth |
/// | 400 | 看 body 的 `code`：`NotFound.File` → NotFound；`AlreadyExist.File` → 已存在；其它 → 协议错误 |
/// | 404 | NotFound |
/// | 409 | 已存在 |
/// | 429 | 暂时性不可达 |
/// | 5xx | 不可达 |
/// | 3xx | Auth（客户端刻意不跟随重定向） |
pub fn aliyun_failure_kind(status: u16, code: Option<&str>) -> RemoteErrorKind {
    // 官方已知的应用/授权类错误码，一律按 Auth 处理 —— 它们与 HTTP 状态无关
    // （换 token 端点经常用 400 回 `InvalidCode`）。
    if let Some(code) = code {
        if ALIYUN_AUTH_ERROR_CODES.contains(&code) {
            return RemoteErrorKind::Auth;
        }
    }

    match status {
        401 | 403 => RemoteErrorKind::Auth,
        400 => {
            if let Some(code) = code {
                if ALIYUN_NOT_FOUND_ERROR_CODES.contains(&code) {
                    return RemoteErrorKind::NotFound;
                }
                if ALIYUN_ALREADY_EXIST_ERROR_CODES.contains(&code) {
                    return RemoteErrorKind::Protocol;
                }
            }
            RemoteErrorKind::Protocol
        }
        404 => RemoteErrorKind::NotFound,
        409 => RemoteErrorKind::Protocol,
        429 => RemoteErrorKind::Unreachable,
        300..=399 => RemoteErrorKind::Auth,
        500..=599 => RemoteErrorKind::Unreachable,
        _ => RemoteErrorKind::Protocol,
    }
}

/// 用户可见的错误文案。
///
/// **只含中性信息**：不含服务端原文、不含错误码、不含任何凭证片段。
/// 这样锚点 9「错误文案绝不含 token / code / refresh_token」是结构性成立的，
/// 而不是靠「服务端这次没回敏感串」的运气。
fn aliyun_failure_message(kind: RemoteErrorKind, status: u16) -> String {
    let prefix = format!("阿里云盘返回了 HTTP {status}");
    match kind {
        RemoteErrorKind::Auth => format!("{prefix}，拒绝了这次请求，请重新授权。"),
        RemoteErrorKind::NotFound => format!("{prefix}，没有找到对应的条目。"),
        RemoteErrorKind::Unreachable => format!("{prefix}，服务暂时不可达，请稍后再试。"),
        _ => format!("{prefix}，响应内容无法识别。"),
    }
}

/// 把一次失败的 HTTP 响应映射成 [`RemoteError`]。
///
/// `body` 会被解析成 [`AliyunFailure`] 供上层记日志，但**不进**错误文案。
pub fn aliyun_map_failure(status: u16, body: &str) -> RemoteError {
    let failure = aliyun_parse_failure(body);
    let kind = aliyun_failure_kind(status, failure.code.as_deref());
    RemoteError::new(kind, aliyun_failure_message(kind, status))
}

// ===========================================================================
// 请求体构造
// ===========================================================================

/// `limit` 夹进 `[1, 100]`。
///
/// 服务端对超范围的 `limit` 是**静默截断**（契约 §1.3）—— 不报错、少返回条目。
/// 用户会以为网盘里就这几个文件，却怎么翻都翻不出更多。
pub fn aliyun_clamp_limit(limit: i64) -> u32 {
    if limit < i64::from(ALIYUN_LIST_LIMIT_MIN) {
        return ALIYUN_LIST_LIMIT_MIN;
    }
    if limit > i64::from(ALIYUN_LIST_LIMIT_MAX) {
        return ALIYUN_LIST_LIMIT_MAX;
    }
    limit as u32
}

/// 组装 `application/x-www-form-urlencoded` 请求体。
pub fn aliyun_form_body(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| {
            format!(
                "{}={}",
                urlencoding::encode(key),
                urlencoding::encode(value)
            )
        })
        .collect::<Vec<String>>()
        .join("&")
}

// ---------------------------------------------------------------------------
// 端点选择（web 分支 / PDS OpenAPI 分支）
// ---------------------------------------------------------------------------
//
// 两套端点成组存在，靠 [`ALIYUN_USE_PDS_OPEN_API`] 一个开关切换。web 分支是
// 本轮默认；PDS 分支的常量**保留**，以便随时切回去 —— 但默认不走到它。

/// 换 token 的端点。
pub fn aliyun_access_token_url() -> &'static str {
    if ALIYUN_USE_PDS_OPEN_API {
        ALIYUN_PDS_ACCESS_TOKEN_URL
    } else {
        ALIYUN_ACCESS_TOKEN_URL
    }
}

/// 刷新 token 的端点。
pub fn aliyun_refresh_token_url() -> &'static str {
    if ALIYUN_USE_PDS_OPEN_API {
        ALIYUN_PDS_ACCESS_TOKEN_URL
    } else {
        ALIYUN_REFRESH_TOKEN_URL
    }
}

/// 取下载直链的端点。
pub fn aliyun_download_url_api() -> &'static str {
    if ALIYUN_USE_PDS_OPEN_API {
        ALIYUN_PDS_DOWNLOAD_URL_API
    } else {
        ALIYUN_DOWNLOAD_URL_API
    }
}

/// 取条目元数据的端点。
pub fn aliyun_get_url() -> &'static str {
    if ALIYUN_USE_PDS_OPEN_API {
        ALIYUN_PDS_GET_URL
    } else {
        ALIYUN_GET_URL
    }
}

/// 列目录的端点。
///
/// **web 分支要带 `jsonmask`**：它决定了服务端回哪些字段。
/// 少了 `jsonmask` 服务端可能只回最小集 ⇒ 列表里就没有大小、修改时间了。
/// 字段清单逐字来自 bundle（见 [`ALIYUN_LIST_JSONMASK_FIELDS`]），
/// **不增不减** —— 多一个服务端可能不认。
pub fn aliyun_list_url() -> String {
    if ALIYUN_USE_PDS_OPEN_API {
        return ALIYUN_PDS_LIST_URL.to_string();
    }

    let fields = ALIYUN_LIST_JSONMASK_FIELDS.join(",");
    format!("{ALIYUN_LIST_URL}?jsonmask=next_marker,items({fields})")
}

// ---------------------------------------------------------------------------
// 请求体构造
// ---------------------------------------------------------------------------

/// 用授权码换 token 的请求体（**web 分支**）。
///
/// **JSON**，字段只有 `client_id` + `code` —— bundle 原文如此：
/// `post(sv_endpoint+"/token/get", {client_id, code})`。
///
/// **没有 `client_secret`**（web 分支免密钥），也**没有** `code_verifier`
/// ⇒ 授权页那边就不能带 `code_challenge`（带了却拿不出来换 token）。
pub fn aliyun_authorization_code_body(client_id: &str, code: &str) -> Value {
    serde_json::json!({
        "client_id": client_id,
        "code": code,
    })
}

/// 用刷新凭证换新 token 的请求体（**web 分支**）。
///
/// ⚠️ 刷新端点**未实测**（批次 27 未关闭项 5）⇒ 刷新失败时明确报
/// 「请重新授权」，不影响只读主流程。
pub fn aliyun_refresh_token_body(client_id: &str, refresh_token: &str) -> Value {
    serde_json::json!({
        "client_id": client_id,
        "refresh_token": refresh_token,
    })
}

/// 列目录的请求体。
///
/// 字段坐实请求（`b28-final-raw.md` §2 + b29 真机复测）：
/// `drive_id` / `parent_file_id` / `limit` / `url_expire_sec` /
/// 三个 `*_process` / `fields` / `order_by` / `order_direction`。
///
/// `marker` 为 `None` 或空串 ⇒ **不放**这个字段（第一页没有游标）。
///
/// ⚠️ **不放 `all` 字段**。真机实测（`b29-body-probe2.log`）：带 `all=false`
/// 时服务端只回 **13** 项、根目录少了「手机备份」这一项；不带 `all` 时回
/// **14** 项。少掉的那一项是**静默丢失**，必须去掉这个字段。
///
/// ⚠️ 三个 `*_process` 字段必须是**字符串**（`"image/resize,w_256/format,avif"`
/// 这种「指令串」），**不能**写成 `{"type":..,"size":..}` 对象 —— 那是
/// 另一个云盘接口的形态。实测写成对象时服务端回
/// **HTTP 400 `InvalidRequestJSONFormat`**（b29 真机复现：对象形态 400，
/// 字符串形态 200，证据 `b29-body-probe.log`）。
pub fn aliyun_list_request_body(
    drive_id: &str,
    parent_file_id: &str,
    limit: i64,
    marker: Option<&str>,
    order_by: &str,
    order_direction: &str,
) -> Value {
    let mut body = serde_json::Map::new();
    body.insert("drive_id".to_string(), Value::String(drive_id.to_string()));
    body.insert(
        "parent_file_id".to_string(),
        Value::String(parent_file_id.to_string()),
    );
    body.insert(
        "limit".to_string(),
        Value::Number(aliyun_clamp_limit(limit).into()),
    );
    body.insert(
        "url_expire_sec".to_string(),
        Value::Number(ALIYUN_LIST_URL_EXPIRE_SEC.into()),
    );
    body.insert(
        "image_thumbnail_process".to_string(),
        Value::String("image/resize,w_256/format,avif".to_string()),
    );
    body.insert(
        "image_url_process".to_string(),
        Value::String("image/resize,w_1920/format,avif".to_string()),
    );
    body.insert(
        "video_thumbnail_process".to_string(),
        Value::String(
            "video/snapshot,t_120000,f_jpg,m_lfit,w_256,ar_auto,m_fast".to_string(),
        ),
    );
    body.insert("fields".to_string(), Value::String("*".to_string()));
    body.insert("order_by".to_string(), Value::String(order_by.to_string()));
    body.insert(
        "order_direction".to_string(),
        Value::String(order_direction.to_string()),
    );

    if let Some(marker) = marker {
        let marker = marker.trim();
        if !marker.is_empty() {
            body.insert("marker".to_string(), Value::String(marker.to_string()));
        }
    }

    Value::Object(body)
}

/// 取条目元数据的请求体。
pub fn aliyun_get_request_body(drive_id: &str, file_id: &str) -> Value {
    serde_json::json!({
        "drive_id": drive_id,
        "file_id": file_id,
    })
}

/// 取下载直链的请求体。
pub fn aliyun_download_url_request_body(drive_id: &str, file_id: &str) -> Value {
    serde_json::json!({
        "drive_id": drive_id,
        "file_id": file_id,
    })
}

/// 取当前用户信息的请求体。**实测是空对象**。
pub fn aliyun_user_info_request_body() -> Value {
    serde_json::json!({})
}

/// 取网盘信息的请求体。
pub fn aliyun_drive_info_request_body(drive_id: &str) -> Value {
    serde_json::json!({
        "drive_id": drive_id,
    })
}

/// 拼授权页地址。
///
/// `scope` 为空串 ⇒ **不放**这个参数（web 分支的 scope 未实测，宁可不发）。
/// `code_challenge` 为 `None` ⇒ 不带 PKCE 参数。
///
/// ⚠️ **web 分支的授权页参数未坐实**（`.superpowers/sdd` 里没有 `oauth/authorize`
/// 的原文证据）⇒ 这里按 bundle 推断拼装，参数名以 `b28-code-plan.md` 为准。
/// 唯一**结构性**的约束是：带了 `code_challenge` 就必须在换 token 时带
/// `code_verifier`，而 web 分支的 `token/get` 只收 `{client_id, code}`
/// ⇒ **web 分支不传 challenge**。
pub fn aliyun_authorize_url(
    client_id: &str,
    redirect_uri: &str,
    scope: &str,
    state: &str,
    code_challenge: Option<&str>,
    code_challenge_method: Option<&str>,
    drive: Option<&str>,
) -> String {
    let mut pairs: Vec<(&str, &str)> = vec![
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
        ("response_type", "code"),
        ("state", state),
        ("auto_login", "true"),
        ("style", "folder"),
    ];

    let scope = scope.trim();
    if !scope.is_empty() {
        pairs.push(("scope", scope));
    }

    if let Some(challenge) = code_challenge {
        let challenge = challenge.trim();
        if !challenge.is_empty() {
            pairs.push(("code_challenge", challenge));
            if let Some(method) = code_challenge_method {
                let method = method.trim();
                if !method.is_empty() {
                    pairs.push(("code_challenge_method", method));
                }
            }
        }
    }

    if let Some(drive) = drive {
        let drive = drive.trim();
        if !drive.is_empty() {
            pairs.push(("drive", drive));
        }
    }

    let base = if ALIYUN_USE_PDS_OPEN_API {
        ALIYUN_PDS_AUTHORIZE_URL
    } else {
        ALIYUN_AUTHORIZE_URL
    };

    format!("{base}?{}", aliyun_form_body(&pairs))
}

/// 从回调 URL 里抠出 `code` 与 `state`。
///
/// 只认查询串里的 `code` 参数；`state` 有就一并带出来。
/// 扣不出来返回 `None` —— 调用方据此判定「还没拿到授权码」。
pub fn aliyun_extract_code(url: &str) -> Option<(String, Option<String>)> {
    let query = url.split_once('?').map(|(_, rest)| rest)?;
    let query = query.split('#').next().unwrap_or(query);

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

/// 回调回来的 `state` 跟预期的是否一致。
///
/// 服务端没回 `state` ⇒ **不一致**（必须拒绝）：宁可让用户重登一次，
/// 也不能放行一个来路不明的回调。
pub fn aliyun_state_matches(expected: &str, actual: Option<&str>) -> bool {
    match actual {
        Some(actual) if !actual.is_empty() => actual == expected,
        _ => false,
    }
}

// ===========================================================================
// 响应解析
// ===========================================================================

/// 解析列目录的响应。
pub fn aliyun_parse_list_payload(body: &str) -> Result<AliyunListPayload, RemoteError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| RemoteError::protocol("阿里云盘返回的目录内容无法解析。"))?;

    if !value.is_object() {
        return Err(RemoteError::protocol("阿里云盘返回的目录内容无法解析。"));
    }

    let items = value
        .get("items")
        .and_then(Value::as_array)
        .map(|array| array.iter().map(aliyun_entry_from_value).collect())
        .unwrap_or_default();

    let next_marker = lenient_str(&value, "next_marker").and_then(|marker| {
        let marker = marker.trim().to_string();
        if marker.is_empty() {
            None
        } else {
            Some(marker)
        }
    });

    Ok(AliyunListPayload { items, next_marker })
}

/// 这一页之后**还有没有**下一页。
///
/// `next_marker` 为空/缺失 ⇒ 取完了（契约 §1.3：cursor 制，不是页码）。
pub fn aliyun_next_marker(payload: &AliyunListPayload) -> Option<String> {
    payload
        .next_marker
        .as_deref()
        .map(str::trim)
        .filter(|marker| !marker.is_empty())
        .map(str::to_string)
}

// ===========================================================================
// 分页：单页拉取 / 拉全（裁定第 5 条）
// ===========================================================================
//
// 裁定第 5 条：`read_dir` 必须**跟着 `next_marker` 一直翻**，直到游标为空；
// 并且要有硬上限，**触顶要报错，不许静默截断**（静默截断 = 用户以为目录里
// 就那几个文件，实际后面还有几百个，是会让人误删数据的那种坑）。
//
// 这两个函数刻意留在**纯函数层**：它们只认「一个能按游标取页的回调」，
// 不认 HTTP。所以测试可以用假回调喂假 JSON，**零网络**验证多页拼接。

/// 翻页太多时报的错（页数上限）。
pub fn aliyun_list_page_limit_error() -> RemoteError {
    RemoteError::protocol(format!(
        "阿里云盘这个目录超过了 {ALIYUN_MAX_LIST_PAGES} 页，已放弃继续翻页。"
    ))
}

/// 翻页太多时报的错（条目数上限）。
pub fn aliyun_list_entry_limit_error() -> RemoteError {
    RemoteError::protocol(format!(
        "阿里云盘这个目录的条目超过了 {ALIYUN_MAX_LIST_ENTRIES} 条，已放弃继续翻页。"
    ))
}

/// **单页**拉取：按给定游标取一页，并整理成 [`AliyunListPage`]。
///
/// `fetch` 拿到的就是服务端返回的**原始响应体**（本层不做网络），
/// 由调用方（provider）决定怎么发请求、怎么处理「授权失效先刷新再重试」。
pub fn aliyun_fetch_list_page<F>(
    mut fetch: F,
    marker: Option<String>,
) -> Result<AliyunListPage, RemoteError>
where
    F: FnMut(Option<String>) -> Result<String, RemoteError>,
{
    let body = fetch(marker)?;
    let payload = aliyun_parse_list_payload(&body)?;
    let next_marker = aliyun_next_marker(&payload);
    Ok(AliyunListPage {
        items: payload.items,
        next_marker,
    })
}

/// **拉全**：跟着 `next_marker` 一直翻到游标为空，把所有页的条目拼起来。
///
/// 触顶（页数或条目数）⇒ **报错**，绝不返回一份「看起来正常但被截断」的列表。
pub fn aliyun_fetch_all_list_pages<F>(mut fetch: F) -> Result<Vec<AliyunFileEntry>, RemoteError>
where
    F: FnMut(Option<String>) -> Result<String, RemoteError>,
{
    let mut entries: Vec<AliyunFileEntry> = Vec::new();
    let mut marker: Option<String> = None;
    let mut pages = 0u32;

    loop {
        pages += 1;
        if pages > ALIYUN_MAX_LIST_PAGES {
            return Err(aliyun_list_page_limit_error());
        }

        let page = aliyun_fetch_list_page(&mut fetch, marker.clone())?;
        entries.extend(page.items);

        if entries.len() > ALIYUN_MAX_LIST_ENTRIES {
            return Err(aliyun_list_entry_limit_error());
        }

        match page.next_marker {
            Some(next) => marker = Some(next),
            None => break,
        }
    }

    Ok(entries)
}

/// 这个条目**是不是目录**。
pub fn aliyun_entry_is_directory(entry: &AliyunFileEntry) -> bool {
    entry
        .entry_type
        .trim()
        .eq_ignore_ascii_case(ALIYUN_ENTRY_TYPE_FOLDER)
}

/// 这个条目的名字（去掉可能带的路径分隔痕迹）。
pub fn aliyun_entry_name(entry: &AliyunFileEntry) -> &str {
    entry.name.as_str()
}

/// 解析换 token 的响应。
///
/// 没有 `access_token` ⇒ 报 Auth「请重新授权」。
pub fn aliyun_parse_token_payload(body: &str) -> Result<AliyunTokenPayload, RemoteError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| RemoteError::auth("阿里云盘的授权响应无法解析，请重新授权。"))?;

    if !value.is_object() {
        return Err(RemoteError::auth(
            "阿里云盘的授权响应无法解析，请重新授权。",
        ));
    }

    let access_token = lenient_str(&value, "access_token").unwrap_or_default();
    if access_token.trim().is_empty() {
        return Err(RemoteError::auth("未能取到访问凭证，请重新授权。"));
    }

    Ok(AliyunTokenPayload {
        access_token,
        refresh_token: lenient_str(&value, "refresh_token")
            .filter(|value| !value.trim().is_empty()),
        expires_in: lenient_i64(&value, "expires_in"),
        refresh_token_expires_in: lenient_i64(&value, "refresh_token_expires_in"),
        token_type: lenient_str(&value, "token_type"),
        drive_id: lenient_str(&value, "drive_id").filter(|value| !value.trim().is_empty()),
        uid: lenient_str(&value, "uid"),
        nick_name: lenient_str(&value, "user_name")
            .or_else(|| lenient_str(&value, "nick_name"))
            .filter(|value| !value.trim().is_empty()),
    })
}

/// 解析下载直链。
///
/// **按 bundle 的提升顺序**依次读 [`ALIYUN_DOWNLOAD_URL_FIELDS`] 里的字段，
/// 第一个非空的就是它。`streams_url` 有时是个对象（`{"size": "<url>"}`）
/// ⇒ 对象形态再往里取一层 `url`。
pub fn aliyun_parse_download_url(body: &str) -> Result<String, RemoteError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| RemoteError::protocol("阿里云盘没有返回下载直链。"))?;

    if !value.is_object() {
        return Err(RemoteError::protocol("阿里云盘没有返回下载直链。"));
    }

    for field in ALIYUN_DOWNLOAD_URL_FIELDS {
        let Some(candidate) = value.get(*field) else {
            continue;
        };

        let url = match candidate {
            Value::String(text) => Some(text.clone()),
            Value::Object(_) => lenient_str(candidate, "url").or_else(|| {
                // `streams_url` 的**对象**形态实测未坐实（契约 §B 只坐实了顶层
                // `url`）⇒ `{"size": "<url>"}` 与 `{"size": {"url": "<url>"}}`
                // 两种都认，取不到就继续往下找下一个字段。
                match candidate.get("size") {
                    Some(Value::String(text)) => Some(text.clone()),
                    Some(inner) if inner.is_object() => lenient_str(inner, "url"),
                    _ => None,
                }
            }),
            _ => None,
        };

        if let Some(url) = url {
            let url = url.trim();
            if !url.is_empty() {
                return Ok(url.to_string());
            }
        }
    }

    Err(RemoteError::protocol("阿里云盘没有返回下载直链。"))
}

/// 解析当前登录账号应当使用的 `drive_id`。
///
/// 优先级（从高到低）：
/// 1. 环境变量 [`ALIYUN_DRIVE_ID_ENV`] —— 运维/调试用的显式覆盖；
/// 2. 会话里显式存下来的 `drive_id`（登录时从 `/v2/user/get` 的
///    `default_drive_id` 取到并存进会话）。
///
/// **没有第三档** —— 批次 28 推翻了批次 27 的「默认主盘常量」：
/// `954264082` 是 resource 盘、`25998870` 是保险箱（403）。
/// 取不到 ⇒ `None` ⇒ 上层明确报错，让用户重新授权，**不猜**。
pub fn aliyun_resolve_drive_id(explicit: Option<&str>) -> Option<String> {
    if let Ok(value) = std::env::var(ALIYUN_DRIVE_ID_ENV) {
        let value = value.trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }

    explicit
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// 解析 `openFile/get` 的响应（取条目元数据）。
///
/// 裁定第 8 条：**元数据以 `openFile/get` 为准** —— 列表接口只当索引用。
/// 字段名未实测 ⇒ 容错：顶层对象就是条目；服务端若裹了一层 `file` / `item`，
/// 就取内层那个。
pub fn aliyun_parse_entry_body(body: &str) -> Result<AliyunFileEntry, RemoteError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| RemoteError::protocol("阿里云盘返回的文件信息无法解析。"))?;

    if !value.is_object() {
        return Err(RemoteError::protocol("阿里云盘返回的文件信息无法解析。"));
    }

    let inner = ["file", "item"]
        .iter()
        .find_map(|key| value.get(*key).filter(|inner| inner.is_object()))
        .unwrap_or(&value);

    Ok(aliyun_entry_from_value(inner))
}

// ===========================================================================
// 时间戳
// ===========================================================================

/// 闰年无关的「民用日期 → 天数」转换（Howard Hinnant 算法）。
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_position = (month + 9) % 12;
    let day_of_year = (153 * month_position + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// 解析一段全 ASCII 数字。
fn parse_ascii_digits(text: &str) -> Option<i64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<i64>().ok()
}

/// 解析时区偏移（秒）。
///
/// `""`（没有时区）按 UTC 处理；`Z`/`z` 是 0；`+08:00` 是 28800。
fn aliyun_parse_utc_offset(text: &str) -> Option<i64> {
    let text = text.trim();
    if text.is_empty() {
        return Some(0);
    }
    if text.eq_ignore_ascii_case("z") {
        return Some(0);
    }

    let (sign, rest) = match text.as_bytes().first()? {
        b'+' => (1i64, &text[1..]),
        b'-' => (-1i64, &text[1..]),
        _ => return None,
    };

    let (hours, minutes) = if rest.contains(':') {
        let (hours, minutes) = rest.split_once(':')?;
        (hours, minutes)
    } else if rest.len() == 4 {
        (&rest[0..2], &rest[2..4])
    } else {
        return None;
    };

    let hours = parse_ascii_digits(hours)?;
    let minutes = parse_ascii_digits(minutes)?;
    Some(sign * (hours * 3600 + minutes * 60))
}

/// ISO8601 时间戳 → Unix **毫秒**。
///
/// 支持 `2024-01-02T03:04:05Z`、`2024-01-02T03:04:05.123Z`、
/// `2024-01-02T03:04:05+08:00`、以及没有时区后缀（按 UTC）这几种写法。
///
/// 解析不出来返回 `None` —— 宁可不显示修改时间，也不要显示一个错的时间。
pub fn aliyun_iso8601_to_unix_millis(text: &str) -> Option<u64> {
    let trimmed = text.trim();
    if trimmed.len() < 19 {
        return None;
    }

    let year = parse_ascii_digits(trimmed.get(0..4)?)?;
    if trimmed.as_bytes().get(4)? != &b'-' {
        return None;
    }
    let month = parse_ascii_digits(trimmed.get(5..7)?)?;
    if trimmed.as_bytes().get(7)? != &b'-' {
        return None;
    }
    let day = parse_ascii_digits(trimmed.get(8..10)?)?;

    let separator = trimmed.as_bytes().get(10)?;
    if separator != &b'T' && separator != &b' ' {
        return None;
    }

    let hour = parse_ascii_digits(trimmed.get(11..13)?)?;
    if trimmed.as_bytes().get(13)? != &b':' {
        return None;
    }
    let minute = parse_ascii_digits(trimmed.get(14..16)?)?;
    if trimmed.as_bytes().get(16)? != &b':' {
        return None;
    }
    let second = parse_ascii_digits(trimmed.get(17..19)?)?;

    let mut rest = trimmed.get(19..)?;
    let mut millis = 0i64;
    if let Some(stripped) = rest.strip_prefix('.') {
        let digits: String = stripped
            .chars()
            .take_while(char::is_ascii_digit)
            .take(3)
            .collect();
        rest = stripped.get(digits.len()..)?;
        let mut scaled = digits;
        while scaled.len() < 3 {
            scaled.push('0');
        }
        millis = parse_ascii_digits(&scaled)?;
    }

    let offset = aliyun_parse_utc_offset(rest)?;

    let days = days_from_civil(year, month, day);
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second - offset;
    let total = seconds * 1000 + millis;

    u64::try_from(total).ok()
}

// ===========================================================================
// 条目 → RemoteEntry
// ===========================================================================

/// 把一个阿里云盘条目转成 [`RemoteEntry`]。
///
/// `path` 是**完整**的 `sfm://remote/<连接 id>/...`（契约 §6.1 锚点 13）。
pub fn aliyun_to_remote_entry(
    entry: &AliyunFileEntry,
    connection_id: &str,
    parent_path: &str,
) -> RemoteEntry {
    let child_path = join_remote_parent_and_name(parent_path, &entry.name);
    let path = build_remote_path(connection_id, &child_path).unwrap_or(child_path);

    let hidden = entry.hidden.unwrap_or(false) || is_hidden_remote_name(&entry.name);

    RemoteEntry {
        name: entry.name.clone(),
        path,
        is_dir: aliyun_entry_is_directory(entry),
        size: entry.size.unwrap_or(0),
        modified_time: entry
            .updated_at
            .as_deref()
            .and_then(aliyun_iso8601_to_unix_millis)
            .unwrap_or(0),
        is_hidden: hidden,
        is_symlink: false,
    }
}

/// 把一批阿里云盘条目转成 [`RemoteEntry`]。
pub fn aliyun_to_remote_entries(
    entries: &[AliyunFileEntry],
    connection_id: &str,
    parent_path: &str,
) -> Vec<RemoteEntry> {
    entries
        .iter()
        .map(|entry| aliyun_to_remote_entry(entry, connection_id, parent_path))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(entry_type: &str) -> AliyunFileEntry {
        AliyunFileEntry {
            file_id: "file-1".to_string(),
            name: "示例.txt".to_string(),
            entry_type: entry_type.to_string(),
            size: Some(128),
            updated_at: Some("2024-01-02T03:04:05Z".to_string()),
            ..AliyunFileEntry::default()
        }
    }

    // ---- 分页 ---------------------------------------------------------------

    #[test]
    fn the_next_marker_decides_whether_another_page_is_needed() {
        let empty = AliyunListPayload {
            items: Vec::new(),
            next_marker: None,
        };
        assert_eq!(aliyun_next_marker(&empty), None);

        let blank = AliyunListPayload {
            items: Vec::new(),
            next_marker: Some("   ".to_string()),
        };
        assert_eq!(aliyun_next_marker(&blank), None);

        let more = AliyunListPayload {
            items: Vec::new(),
            next_marker: Some("marker-2".to_string()),
        };
        assert_eq!(aliyun_next_marker(&more), Some("marker-2".to_string()));
    }

    #[test]
    fn the_list_payload_is_parsed_from_a_cursor_page() {
        let body = r#"{"items":[{"file_id":"f1","name":"a.txt","type":"file","size":12},
                                 {"file_id":"f2","name":"dir","type":"folder"}],
                        "next_marker":"m2"}"#;

        let payload = aliyun_parse_list_payload(body).expect("解析");

        assert_eq!(payload.items.len(), 2);
        assert_eq!(payload.items[0].name, "a.txt");
        assert_eq!(payload.items[0].size, Some(12));
        assert!(!aliyun_entry_is_directory(&payload.items[0]));
        assert!(aliyun_entry_is_directory(&payload.items[1]));
        assert_eq!(aliyun_next_marker(&payload), Some("m2".to_string()));
    }

    #[test]
    fn a_missing_next_marker_means_the_listing_is_complete() {
        let body = r#"{"items":[{"file_id":"f1","name":"a.txt","type":"file"}]}"#;

        let payload = aliyun_parse_list_payload(body).expect("解析");

        assert_eq!(aliyun_next_marker(&payload), None);
    }

    #[test]
    fn one_broken_item_does_not_sink_the_whole_page() {
        let body = r#"{"items":[{"file_id":"f1","name":"a.txt","type":"file","size":"不是数字"},
                                 {"name":"b.txt"},
                                 "完全不是对象"],"next_marker":""}"#;

        let payload = aliyun_parse_list_payload(body).expect("解析");

        assert_eq!(payload.items.len(), 3);
        assert_eq!(payload.items[0].size, None);
        assert_eq!(payload.items[1].file_id, "");
        assert_eq!(aliyun_next_marker(&payload), None);
    }

    #[test]
    fn a_body_that_is_not_an_object_is_rejected() {
        let error = aliyun_parse_list_payload("[1,2,3]").expect_err("数组不是合法响应");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
    }

    // ---- limit 夹取 ---------------------------------------------------------

    #[test]
    fn the_page_limit_is_clamped_into_the_supported_range() {
        assert_eq!(aliyun_clamp_limit(0), 1);
        assert_eq!(aliyun_clamp_limit(-5), 1);
        assert_eq!(aliyun_clamp_limit(1), 1);
        assert_eq!(aliyun_clamp_limit(50), 50);
        assert_eq!(aliyun_clamp_limit(100), 100);
        assert_eq!(aliyun_clamp_limit(101), 100);
        assert_eq!(aliyun_clamp_limit(9999), 100);
    }

    #[test]
    fn the_list_body_carries_the_clamped_limit_and_omits_an_empty_marker() {
        let body = aliyun_list_request_body(
            "drive-1",
            ALIYUN_ROOT_FILE_ID,
            500,
            None,
            ALIYUN_LIST_ORDER_BY,
            ALIYUN_LIST_ORDER_DIRECTION,
        );

        assert_eq!(body["limit"], serde_json::json!(100));
        assert_eq!(body["drive_id"], serde_json::json!("drive-1"));
        assert_eq!(body["parent_file_id"], serde_json::json!("root"));
        assert!(body.get("marker").is_none());

        // 坐实请求里的那几个字段，一个都不能少。
        assert_eq!(body["url_expire_sec"], serde_json::json!(14400));
        assert_eq!(body["fields"], serde_json::json!("*"));
        // 三个 `*_process` 必须是**字符串**（图片/视频处理指令串）。
        // 写成 `{"type":..,"size":..}` 对象 ⇒ 服务端回 HTTP 400 InvalidRequestJSONFormat。
        assert_eq!(
            body["image_thumbnail_process"],
            serde_json::json!("image/resize,w_256/format,avif")
        );
        assert_eq!(
            body["image_url_process"],
            serde_json::json!("image/resize,w_1920/format,avif")
        );
        assert_eq!(
            body["video_thumbnail_process"],
            serde_json::json!("video/snapshot,t_120000,f_jpg,m_lfit,w_256,ar_auto,m_fast")
        );
        // ⚠️ 请求体里**不能**带 `all`。真机实测（`b29-body-probe2.log`）：
        // 带 `all=false` 时服务端只回 13 项、根目录少了「手机备份」；
        // 不带 `all` 才回全 14 项。少的那项是静默丢失。
        assert!(body.get("all").is_none());

        let with_marker = aliyun_list_request_body(
            "drive-1",
            ALIYUN_ROOT_FILE_ID,
            20,
            Some("m2"),
            ALIYUN_LIST_ORDER_BY,
            ALIYUN_LIST_ORDER_DIRECTION,
        );
        assert_eq!(with_marker["limit"], serde_json::json!(20));
        assert_eq!(with_marker["marker"], serde_json::json!("m2"));
    }

    #[test]
    fn the_list_url_carries_the_json_mask_of_the_bundled_fields() {
        let url = aliyun_list_url();

        assert!(url.starts_with(ALIYUN_LIST_URL));
        assert!(url.contains("jsonmask=next_marker,items("));
        assert!(url.contains("file_id"));
        assert!(url.contains("updated_at"));
        assert!(url.contains("from_share_id"));
    }

    #[test]
    fn the_entry_metadata_endpoint_is_the_v2_one_that_exists() {
        // `/adrive/v3/file/get` **不存在**：服务端回 404
        // `{"code":"NotFound","message":"uri(/v3/file/get) not exist"}`；
        // 真机实测 `/adrive/v2/file/get` 回 200（证据 `b29-list-probe4.log`）。
        assert_eq!(
            ALIYUN_GET_URL,
            "https://api.aliyundrive.com/adrive/v2/file/get"
        );
        assert!(aliyun_get_url().ends_with("/file/get"));
    }

    // ---- 条目类型 -----------------------------------------------------------

    #[test]
    fn the_entry_type_decides_whether_the_entry_is_a_directory() {
        assert!(aliyun_entry_is_directory(&entry("folder")));
        assert!(aliyun_entry_is_directory(&entry("FOLDER")));
        assert!(!aliyun_entry_is_directory(&entry("file")));
        assert!(!aliyun_entry_is_directory(&entry("")));
    }

    // ---- 时间戳 -------------------------------------------------------------

    #[test]
    fn the_iso8601_timestamp_becomes_unix_millis() {
        assert_eq!(
            aliyun_iso8601_to_unix_millis("2024-01-02T03:04:05Z"),
            Some(1_704_164_645_000)
        );
        assert_eq!(
            aliyun_iso8601_to_unix_millis("2024-01-02T03:04:05.123Z"),
            Some(1_704_164_645_123)
        );
        // 东八区 ⇒ 减 8 小时。
        assert_eq!(
            aliyun_iso8601_to_unix_millis("2024-01-02T11:04:05+08:00"),
            Some(1_704_164_645_000)
        );
        assert_eq!(aliyun_iso8601_to_unix_millis("不是时间"), None);
        assert_eq!(aliyun_iso8601_to_unix_millis(""), None);
    }

    #[test]
    fn the_epoch_itself_is_parsed_correctly() {
        assert_eq!(
            aliyun_iso8601_to_unix_millis("1970-01-01T00:00:00Z"),
            Some(0)
        );
    }

    // ---- 错误映射 -----------------------------------------------------------

    #[test]
    fn the_error_body_maps_onto_the_right_error_kind() {
        assert_eq!(aliyun_failure_kind(401, None), RemoteErrorKind::Auth);
        assert_eq!(aliyun_failure_kind(403, None), RemoteErrorKind::Auth);
        assert_eq!(aliyun_failure_kind(404, None), RemoteErrorKind::NotFound);
        assert_eq!(aliyun_failure_kind(409, None), RemoteErrorKind::Protocol);
        assert_eq!(aliyun_failure_kind(429, None), RemoteErrorKind::Unreachable);
        assert_eq!(aliyun_failure_kind(500, None), RemoteErrorKind::Unreachable);
        assert_eq!(aliyun_failure_kind(503, None), RemoteErrorKind::Unreachable);
        assert_eq!(aliyun_failure_kind(302, None), RemoteErrorKind::Auth);

        // 400 要看响应体里的错误码。
        assert_eq!(
            aliyun_failure_kind(400, Some("NotFound.File")),
            RemoteErrorKind::NotFound
        );
        assert_eq!(
            aliyun_failure_kind(400, Some("AlreadyExist.File")),
            RemoteErrorKind::Protocol
        );
        assert_eq!(
            aliyun_failure_kind(400, Some("InvalidCode")),
            RemoteErrorKind::Auth
        );
        assert_eq!(
            aliyun_failure_kind(400, Some("随便什么")),
            RemoteErrorKind::Protocol
        );
    }

    #[test]
    fn the_failure_body_is_parsed_leniently() {
        let failure = aliyun_parse_failure(
            r#"{"code":"InvalidCode","message":"授权码无效","requestId":"req-1"}"#,
        );

        assert_eq!(failure.code.as_deref(), Some("InvalidCode"));
        assert_eq!(failure.message.as_deref(), Some("授权码无效"));
        assert_eq!(failure.request_id.as_deref(), Some("req-1"));

        let empty = aliyun_parse_failure("not json");
        assert_eq!(empty, AliyunFailure::default());
    }

    #[test]
    fn the_error_text_never_leaks_the_secrets() {
        // 服务端原文里夹着敏感串 ⇒ 也不许进用户可见文案。
        let body = r#"{"code":"InvalidCode","message":"token=abcdef refresh_token=zzz code=qqq"}"#;
        let error = aliyun_map_failure(400, body);

        let text = error.message.to_lowercase();
        assert!(!text.contains("token"), "错误文案里出现了凭证：{text}");
        assert!(!text.contains("code"), "错误文案里出现了授权码：{text}");
        assert!(!text.contains("abcdef"));
        assert!(!text.contains("zzz"));
        assert!(!text.contains("qqq"));
    }

    // ---- 换 token -----------------------------------------------------------

    #[test]
    fn the_token_payload_is_parsed_leniently() {
        let body = r#"{"access_token":"at-1","refresh_token":"rt-1","expires_in":7200,
                       "refresh_token_expires_in":2592000,"token_type":"Bearer","drive_id":"d-1"}"#;

        let payload = aliyun_parse_token_payload(body).expect("解析");

        assert_eq!(payload.access_token, "at-1");
        assert_eq!(payload.refresh_token.as_deref(), Some("rt-1"));
        assert_eq!(payload.expires_in, Some(7200));
        assert_eq!(payload.refresh_token_expires_in, Some(2_592_000));
        assert_eq!(payload.drive_id.as_deref(), Some("d-1"));
    }

    #[test]
    fn a_token_response_without_an_access_token_is_an_auth_problem() {
        let error = aliyun_parse_token_payload(r#"{"expires_in":7200}"#).expect_err("必须失败");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[test]
    fn the_authorization_code_body_is_json_without_a_client_secret() {
        let body = aliyun_authorization_code_body("client-1", "the-code");

        assert_eq!(body["client_id"], serde_json::json!("client-1"));
        assert_eq!(body["code"], serde_json::json!("the-code"));
        // web 分支免密钥：请求体里**不许**出现 client_secret / code_verifier。
        assert!(body.get("client_secret").is_none());
        assert!(body.get("code_verifier").is_none());
        assert!(body.get("grant_type").is_none());
    }

    #[test]
    fn the_refresh_body_carries_the_refresh_token() {
        let body = aliyun_refresh_token_body("client-1", "the-refresh");

        assert_eq!(body["client_id"], serde_json::json!("client-1"));
        assert_eq!(body["refresh_token"], serde_json::json!("the-refresh"));
        assert!(body.get("client_secret").is_none());
    }

    // ---- 授权页 -------------------------------------------------------------

    #[test]
    fn the_authorize_url_carries_every_required_parameter() {
        let url = aliyun_authorize_url(
            "client-1",
            ALIYUN_DEFAULT_REDIRECT_URI,
            "user:base file:all:read",
            "state-1",
            Some("challenge-1"),
            Some("S256"),
            None,
        );

        assert!(url.starts_with(ALIYUN_AUTHORIZE_URL));
        assert!(url.contains("client_id=client-1"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("state=state-1"));
        assert!(url.contains("code_challenge=challenge-1"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("scope=user%3Abase%20file%3Aall%3Aread"));
        assert!(url.contains("redirect_uri=https%3A%2F%2Fwww.alipan.com%2Fsign%2Fcallback"));
    }

    #[test]
    fn the_web_branch_authorize_url_carries_no_pkce_and_no_scope() {
        // web 分支的 `token/get` 只收 `{client_id, code}` ⇒ 授权页**不能**带
        // `code_challenge`（带了拿 `code_verifier` 换不到 token）。
        let url = aliyun_authorize_url(
            ALIYUN_WEB_CLIENT_ID,
            ALIYUN_DEFAULT_REDIRECT_URI,
            ALIYUN_DEFAULT_SCOPE,
            "state-1",
            None,
            None,
            None,
        );

        assert!(url.starts_with(ALIYUN_AUTHORIZE_URL));
        assert!(url.contains("client_id=25dzX3vbYqktVxyX"));
        assert!(url.contains("state=state-1"));
        assert!(!url.contains("code_challenge"));
        assert!(!url.contains("scope="));
    }

    #[test]
    fn the_code_is_extracted_from_the_callback_url() {
        let (code, state) =
            aliyun_extract_code("http://127.0.0.1:47832/callback?code=the-code&state=the-state")
                .expect("应该拿到授权码");

        assert_eq!(code, "the-code");
        assert_eq!(state.as_deref(), Some("the-state"));
        assert!(aliyun_extract_code("http://127.0.0.1:47832/callback").is_none());
    }

    #[test]
    fn the_callback_state_must_match() {
        assert!(aliyun_state_matches("s1", Some("s1")));
        assert!(!aliyun_state_matches("s1", Some("s2")));
        assert!(!aliyun_state_matches("s1", None));
        assert!(!aliyun_state_matches("s1", Some("")));
    }

    // ---- 直链 ----------------------------------------------------------------

    #[test]
    fn the_download_url_follows_the_bundled_field_order() {
        assert_eq!(
            aliyun_parse_download_url(r#"{"url":"https://x/1"}"#).expect("解析"),
            "https://x/1"
        );
        // `url` 优先于 `encrypt_url`。
        assert_eq!(
            aliyun_parse_download_url(r#"{"url":"https://x/2","encrypt_url":"https://x/enc"}"#)
                .expect("解析"),
            "https://x/2"
        );
        // `url` 空 ⇒ 退到 `encrypt_url`。
        assert_eq!(
            aliyun_parse_download_url(r#"{"url":"","encrypt_url":"https://x/3"}"#).expect("解析"),
            "https://x/3"
        );
        // `streams_url` 是对象时，往里取一层 `url`。
        assert_eq!(
            aliyun_parse_download_url(r#"{"streams_url":{"size":"https://x/4"}}"#).expect("解析"),
            "https://x/4"
        );
        assert!(aliyun_parse_download_url(r#"{}"#).is_err());
    }

    #[test]
    fn the_drive_id_comes_from_the_environment_or_the_session() {
        // 显式值优先。
        assert_eq!(
            aliyun_resolve_drive_id(Some("drive-x")),
            Some("drive-x".to_string())
        );
        // 空串当作没有。
        assert_eq!(aliyun_resolve_drive_id(Some("   ")), None);
        // 批次 28：**没有**默认主盘常量 ⇒ 取不到就是 `None`
        // （`954264082` 是 resource 盘、`25998870` 是保险箱）。
        if std::env::var(ALIYUN_DRIVE_ID_ENV).is_err() {
            assert_eq!(aliyun_resolve_drive_id(None), None);
        }
    }

    #[test]
    fn the_file_metadata_is_read_from_the_get_endpoint() {
        let entry =
            aliyun_parse_entry_body(r#"{"file_id":"f-1","name":"甲.txt","type":"file","size":12}"#)
                .expect("解析");
        assert_eq!(entry.file_id, "f-1");
        assert_eq!(entry.name, "甲.txt");
        assert_eq!(entry.entry_type, "file");
        assert_eq!(entry.size, Some(12));

        let wrapped =
            aliyun_parse_entry_body(r#"{"file":{"file_id":"f-2","name":"乙.txt"}}"#).expect("解析");
        assert_eq!(wrapped.file_id, "f-2");

        assert!(aliyun_parse_entry_body("garbage").is_err());
    }

    // ---- RemoteEntry --------------------------------------------------------

    #[test]
    fn the_remote_entry_carries_the_full_remote_path() {
        let converted = aliyun_to_remote_entry(&entry("file"), "conn-1", "/");

        assert_eq!(converted.name, "示例.txt");
        assert_eq!(converted.path, "sfm://remote/conn-1/示例.txt");
        assert!(!converted.is_dir);
        assert_eq!(converted.size, 128);
        assert_eq!(converted.modified_time, 1_704_164_645_000);
        assert!(!converted.is_hidden);
        assert!(!converted.is_symlink);
    }

    #[test]
    fn a_dot_prefixed_entry_is_marked_as_hidden() {
        let mut value = entry("file");
        value.name = ".隐藏".to_string();

        let converted = aliyun_to_remote_entry(&value, "conn-1", "/");

        assert!(converted.is_hidden);
    }

    #[test]
    fn a_nested_entry_keeps_the_parent_folder_in_its_path() {
        let converted = aliyun_to_remote_entry(&entry("file"), "conn-1", "/资料");

        assert_eq!(converted.path, "sfm://remote/conn-1/资料/示例.txt");
    }
}

// ===========================================================================
// 分页（裁定第 5 条）：单页 / 拉全 / 触顶报错
// ===========================================================================

#[cfg(test)]
mod paging_tests {
    use super::{
        aliyun_fetch_all_list_pages, aliyun_fetch_list_page, RemoteError, RemoteErrorKind,
    };

    /// 造一页假的列目录响应体。
    fn page_body(names: &[&str], next_marker: Option<&str>) -> String {
        let items = names
            .iter()
            .map(|name| {
                format!("{{\"file_id\":\"id-{name}\",\"name\":\"{name}\",\"type\":\"file\"}}")
            })
            .collect::<Vec<_>>()
            .join(",");
        match next_marker {
            Some(marker) => format!("{{\"items\":[{items}],\"next_marker\":\"{marker}\"}}"),
            None => format!("{{\"items\":[{items}]}}"),
        }
    }

    #[test]
    fn a_single_page_reports_whether_another_page_follows() {
        let page = aliyun_fetch_list_page(
            |_marker| Ok::<String, RemoteError>(page_body(&["甲"], Some("m1"))),
            None,
        )
        .expect("单页拉取应该成功");

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].name, "甲");
        assert_eq!(page.next_marker.as_deref(), Some("m1"));
    }

    #[test]
    fn a_listing_follows_the_cursor_until_the_marker_runs_out() {
        let mut requested: Vec<Option<String>> = Vec::new();
        let entries = aliyun_fetch_all_list_pages(|marker| {
            requested.push(marker.clone());
            Ok::<String, RemoteError>(match marker.as_deref() {
                None => page_body(&["甲", "乙"], Some("m1")),
                Some("m1") => page_body(&["丙"], None),
                Some(_) => page_body(&[], None),
            })
        })
        .expect("拉全应该成功");

        let names: Vec<String> = entries.iter().map(|entry| entry.name.clone()).collect();
        assert_eq!(names, vec!["甲", "乙", "丙"]);

        assert_eq!(requested.len(), 2, "游标必须逐页往下传");
        assert_eq!(requested[0], None);
        assert_eq!(requested[1].as_deref(), Some("m1"));
    }

    #[test]
    fn a_listing_that_never_ends_is_reported_instead_of_being_truncated() {
        let result = aliyun_fetch_all_list_pages(|_marker| {
            Ok::<String, RemoteError>(page_body(&["甲"], Some("same")))
        });

        let error = result.expect_err("永远翻不完必须报错，不能静默截断");
        assert_eq!(error.kind, RemoteErrorKind::Protocol);
        assert!(error.message.contains("页"));
    }
}
