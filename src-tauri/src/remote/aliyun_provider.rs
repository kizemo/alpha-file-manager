// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// ---------------------------------------------------------------------------
// 阿里云盘远程目录的读取（批次 26 · 只读）
//
// 红线（与 `remote/mod.rs:8-14` 同一条）：本文件被 `cargo test --lib` 的测试
// 二进制链接，**不得出现 `tauri::AppHandle`、`tauri::async_runtime::*` 或
// `tauri::` 的任何路径**。一旦碰到，Tauri 全局运行时的默认类型参数 `Wry`
// 被单态化，把 tao 整条窗口链拖进测试二进制；测试二进制没有 application
// manifest 声明 comctl32 v6，`TaskDialogIndirect` 解析不到，进程在加载阶段
// 以 `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉 —— 一个测试都跑不了，
// 且报错与本模块毫无关系。
//
// 能力边界：**能列目录、能读文件**；**写操作已落地**（批次 31）——上传
// （`write_file` / `write_file_with_progress`）、新建目录、删除、重命名都真的
// 落到服务端。**只有改权限（`set_permissions` / `get_permissions`）仍是
// `RemoteErrorKind::Unsupported`** —— 阿里云盘没有 POSIX 权限位，无从设置。
//
// 为什么网络访问要包一层 trait：`AliyunApi` 与 `AliyunSessionStore` 两个抽象
// 让本文件可以**零网络**单测 —— 假实现不需要真的连 `openapi.alipan.com`，
// 也不需要真的走 DPAPI 密文。
//
// ⚠️ **阿里云盘的条目没有路径，只有 `file_id`** —— 服务端不提供「按路径查条目」
// 的能力。所以「远端路径 → 目录 file_id」只能从根（`file_id = "root"`）一级
// 一级往下走。详见 `AliyunRemoteProvider::resolve_dir_file_id`。
//
// ⚠️ **写类操作失败时一条都不许返回 `Ok(())`**（契约 §〇）—— 静默成功会让
// 用户以为文件已经传上去了，而它根本没有。
// ---------------------------------------------------------------------------

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;

use super::aliyun_api::{
    aliyun_access_token_url, aliyun_authorization_code_body, aliyun_complete_upload_body,
    aliyun_create_file_body, aliyun_create_folder_body, aliyun_delete_body, aliyun_download_url_api,
    aliyun_entry_type_name, aliyun_failure_is_already_exist, aliyun_get_url,
    aliyun_list_entry_limit_error, aliyun_list_page_limit_error, aliyun_list_url, aliyun_map_failure,
    aliyun_map_write_failure, aliyun_move_body, aliyun_next_marker, aliyun_parse_create_payload,
    aliyun_parse_download_url, aliyun_parse_entry_body, aliyun_parse_list_payload,
    aliyun_parse_token_payload, aliyun_part_ranges, aliyun_refresh_client_id_candidates,
    aliyun_refresh_token_body_with_client_id, aliyun_refresh_token_url, aliyun_rename_body,
    aliyun_resolve_drive_id,
    aliyun_to_remote_entries, AliyunCreateResult, AliyunFileEntry, AliyunListPayload,
    AliyunPartRange, AliyunTokenPayload, AliyunUploadPart, ALIYUN_COMPLETE_URL, ALIYUN_CREATE_URL,
    ALIYUN_DELETE_URL, ALIYUN_LIST_LIMIT_DEFAULT, ALIYUN_LIST_ORDER_BY,
    ALIYUN_LIST_ORDER_DIRECTION, ALIYUN_MOVE_URL, ALIYUN_ROOT_FILE_ID, ALIYUN_UPDATE_URL,
    ALIYUN_UPLOAD_CHUNK_BYTES,
};
use super::aliyun_auth::{
    aliyun_is_expired, aliyun_now_unix, load_aliyun_session, save_aliyun_session, AliyunAuthSession,
};
use super::connection::RemoteConnection;
use super::error::{RemoteError, RemoteErrorKind};
use super::path::{
    join_remote_parent_and_name, normalize_remote_path, reject_path_control_characters,
    split_remote_parent_and_name, validate_remote_entry_name,
};
use super::provider::{RemoteEntry, RemoteProvider};
use super::registry;

/// 单次普通请求的超时。
pub const ALIYUN_OPERATION_TIMEOUT: Duration = Duration::from_secs(120);

/// 连接超时。
pub const ALIYUN_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// 列目录的总时限。
pub const ALIYUN_DIRECTORY_TIMEOUT: Duration = Duration::from_secs(180);

/// 读一个文件（含下载）的总时限。文件要整个进内存 ⇒ 比列表宽松。
pub const ALIYUN_FILE_TIMEOUT: Duration = Duration::from_secs(600);

/// 单个文件的最大下载字节数（512 MiB）。
pub const ALIYUN_MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;

/// 单次列目录最多翻多少页。
///
/// 常量本体在 `aliyun_api`（纯函数层）—— 裁定第 5 条要求「单页拉取」与
/// 「拉全」两个函数都在 API 层，且上限只有一个出处。
pub use super::aliyun_api::ALIYUN_MAX_LIST_PAGES;

/// 单次列目录最多收多少个条目。触顶 ⇒ 报错，不静默截断。
pub use super::aliyun_api::ALIYUN_MAX_LIST_ENTRIES;

/// 阿里云盘开放平台不校验 UA，但发一个像样的总比发 Rust 的默认 UA 好。
pub const ALIYUN_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AlphaFileManager/1.0";

/// 阿里云盘侧的 HTTP 访问。**抽成 trait 是为了零网络单测。**
#[async_trait]
pub trait AliyunApi: Send + Sync {
    /// 列一页目录。`parent_file_id` 是**父目录**的 id，根目录是 `"root"`。
    ///
    /// `marker` 是**游标**（不是页码、不是偏移量）：`None` ⇒ 第一页；
    /// 上一页的 `next_marker` ⇒ 续拉（契约 §1.3）。
    async fn list_page(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        parent_file_id: &str,
        marker: Option<&str>,
    ) -> Result<AliyunListPayload, RemoteError>;

    /// 取一个条目的**元数据**（`openFile/get`）。
    ///
    /// 裁定第 8 条：元数据以 `openFile/get` 为准 —— 列表接口只当索引用。
    async fn file_info(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        file_id: &str,
    ) -> Result<AliyunFileEntry, RemoteError>;

    /// 取一个文件的下载直链。
    async fn download_url(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        file_id: &str,
    ) -> Result<String, RemoteError>;

    /// 下载直链指向的内容，最多接受 `max_bytes` 字节。
    ///
    /// 直链**自身即授权**（契约 §1.5）⇒ 不带 `Authorization`、不带 cookie。
    async fn download(
        &self,
        session: &AliyunAuthSession,
        url: &str,
        max_bytes: u64,
    ) -> Result<Vec<u8>, RemoteError>;

    /// 用刷新凭证换一份新会话。
    ///
    /// 成功时返回的会话**必须**带新的 `refresh_token`（刷新凭证单次有效）。
    async fn refresh_session(
        &self,
        session: &AliyunAuthSession,
    ) -> Result<AliyunAuthSession, RemoteError>;

    // ------------------------------------------------------------------
    // 写操作（批次 31）
    // ------------------------------------------------------------------
    //
    // 写操作**不做「授权失效 ⇒ 刷新后重试」**：重放一次「新建文件」可能
    // 在网盘里留下两个同名条目。失败就**停下来把控制权交回用户**
    // （契约 §6.2），由用户自己决定要不要重试。
    //
    // 取会话时的**主动**过期刷新仍然保留 —— 那是发请求之前就发现过期，
    // 不是重放一个已经执行过的写操作。

    /// 新建目录。返回 `Ok(true)` 表示「这个目录**已经存在**」（按成功处理）。
    async fn create_folder(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        parent_file_id: &str,
        name: &str,
    ) -> Result<bool, RemoteError>;

    /// 新建文件，返回上传会话（`file_id` + 分片直链 + `upload_id`/`revision_id`）。
    async fn create_file(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        parent_file_id: &str,
        name: &str,
    ) -> Result<AliyunCreateResult, RemoteError>;

    /// 把 `content` 按 `ranges` 切块，`PUT` 到各片自己的直链上。
    ///
    /// `on_progress` 收到的是**累计已发送字节数**（与
    /// `RemoteProvider::write_file_with_progress` 同一套语义）。
    async fn upload_parts(
        &self,
        session: &AliyunAuthSession,
        ranges: &[AliyunPartRange],
        content: Arc<Vec<u8>>,
        on_progress: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<(), RemoteError>;

    /// 上传收尾（`file/complete`）。
    async fn complete_upload(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        file_id: &str,
        upload_id: &str,
        revision_id: &str,
    ) -> Result<(), RemoteError>;

    /// 重命名（`file/update`，字段是 `name`）。
    async fn rename_entry(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        file_id: &str,
        name: &str,
    ) -> Result<(), RemoteError>;

    /// 删除（`file/delete`）。`entry_type` 是 `"file"` 或 `"folder"` ——
    /// **服务端要求带上**。
    async fn delete_entry(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        file_id: &str,
        entry_type: &str,
    ) -> Result<(), RemoteError>;

    /// 移动到另一个父目录（`file/move`）。
    async fn move_entry(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        file_id: &str,
        to_parent_file_id: &str,
    ) -> Result<(), RemoteError>;
}

/// HTTP 客户端建不起来时的占位实现。
///
/// 存在的理由：`provider_with_protocol` 的签名**不返回 `Result`**（它返回
/// `Box<dyn RemoteProvider>`），而建 HTTP 客户端是会失败的。所以把初始化错误
/// **推迟**到第一次真正用网络的时候再报出来 —— 用户看到的仍是那句具体的错误，
/// 并且**绝不会**出现「静默成功」。
pub struct UnavailableAliyunApi {
    error: RemoteError,
}

impl UnavailableAliyunApi {
    pub fn new(error: RemoteError) -> Self {
        Self { error }
    }
}

#[async_trait]
impl AliyunApi for UnavailableAliyunApi {
    async fn list_page(
        &self,
        _session: &AliyunAuthSession,
        _drive_id: &str,
        _parent_file_id: &str,
        _marker: Option<&str>,
    ) -> Result<AliyunListPayload, RemoteError> {
        Err(self.error.clone())
    }

    async fn file_info(
        &self,
        _session: &AliyunAuthSession,
        _drive_id: &str,
        _file_id: &str,
    ) -> Result<AliyunFileEntry, RemoteError> {
        Err(self.error.clone())
    }

    async fn download_url(
        &self,
        _session: &AliyunAuthSession,
        _drive_id: &str,
        _file_id: &str,
    ) -> Result<String, RemoteError> {
        Err(self.error.clone())
    }

    async fn download(
        &self,
        _session: &AliyunAuthSession,
        _url: &str,
        _max_bytes: u64,
    ) -> Result<Vec<u8>, RemoteError> {
        Err(self.error.clone())
    }

    async fn refresh_session(
        &self,
        _session: &AliyunAuthSession,
    ) -> Result<AliyunAuthSession, RemoteError> {
        Err(self.error.clone())
    }

    async fn create_folder(
        &self,
        _session: &AliyunAuthSession,
        _drive_id: &str,
        _parent_file_id: &str,
        _name: &str,
    ) -> Result<bool, RemoteError> {
        Err(self.error.clone())
    }

    async fn create_file(
        &self,
        _session: &AliyunAuthSession,
        _drive_id: &str,
        _parent_file_id: &str,
        _name: &str,
    ) -> Result<AliyunCreateResult, RemoteError> {
        Err(self.error.clone())
    }

    async fn upload_parts(
        &self,
        _session: &AliyunAuthSession,
        _ranges: &[AliyunPartRange],
        _content: Arc<Vec<u8>>,
        _on_progress: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<(), RemoteError> {
        Err(self.error.clone())
    }

    async fn complete_upload(
        &self,
        _session: &AliyunAuthSession,
        _drive_id: &str,
        _file_id: &str,
        _upload_id: &str,
        _revision_id: &str,
    ) -> Result<(), RemoteError> {
        Err(self.error.clone())
    }

    async fn rename_entry(
        &self,
        _session: &AliyunAuthSession,
        _drive_id: &str,
        _file_id: &str,
        _name: &str,
    ) -> Result<(), RemoteError> {
        Err(self.error.clone())
    }

    async fn delete_entry(
        &self,
        _session: &AliyunAuthSession,
        _drive_id: &str,
        _file_id: &str,
        _entry_type: &str,
    ) -> Result<(), RemoteError> {
        Err(self.error.clone())
    }

    async fn move_entry(
        &self,
        _session: &AliyunAuthSession,
        _drive_id: &str,
        _file_id: &str,
        _to_parent_file_id: &str,
    ) -> Result<(), RemoteError> {
        Err(self.error.clone())
    }
}

/// 会话存档的读写。**抽成 trait 是为了零 DPAPI 单测。**
pub trait AliyunSessionStore: Send + Sync {
    /// 按键读会话。没有就是 `Ok(None)`；存档坏了要**报错**，不能当成「没存」。
    fn load(&self, key: &str) -> Result<Option<AliyunAuthSession>, RemoteError>;

    /// 写回会话。刷新成功后**必须**调用 —— 刷新凭证单次有效，
    /// 不写回去的话下一次就用不了新的那一份。
    fn save(&self, key: &str, session: &AliyunAuthSession) -> Result<(), RemoteError>;
}

/// 走既有 DPAPI 密文（`remote-credentials.json`）的会话存储。
pub struct DpapiAliyunSessionStore {
    app_data_dir: PathBuf,
}

impl DpapiAliyunSessionStore {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            app_data_dir: app_data_dir.to_path_buf(),
        }
    }
}

impl AliyunSessionStore for DpapiAliyunSessionStore {
    fn load(&self, key: &str) -> Result<Option<AliyunAuthSession>, RemoteError> {
        load_aliyun_session(&self.app_data_dir, key)
    }

    fn save(&self, key: &str, session: &AliyunAuthSession) -> Result<(), RemoteError> {
        save_aliyun_session(&self.app_data_dir, key, session)
    }
}

// ===========================================================================
// HTTP
// ===========================================================================

/// 开放平台接口的默认请求头。
fn aliyun_api_headers() -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    headers
}

/// 建一个专供阿里云盘用的 HTTP 客户端。
///
/// **不跟随重定向**：会话失效时服务端会 3xx，我们要看见这个 3xx 才能报
/// 「请重新授权」；跟过去只会拿到一个登录页 HTML（契约 §1.6 的 3xx ⇒ Auth）。
fn build_client() -> Result<reqwest::Client, RemoteError> {
    reqwest::Client::builder()
        .user_agent(ALIYUN_USER_AGENT)
        .default_headers(aliyun_api_headers())
        .timeout(ALIYUN_OPERATION_TIMEOUT)
        .connect_timeout(ALIYUN_CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| RemoteError::unknown(format!("阿里云盘 HTTP 客户端初始化失败：{error}")))
}

/// 下载用的 HTTP 客户端。
///
/// 与 [`build_client`] 的区别有两条，**两条都是下载直链逼出来的**：
///
/// 1. **跟随重定向**：网盘给出的直链会 3xx 到 CDN 域名，不跟就只拿到一个 3xx。
/// 2. **超时更长**：整份文件要进内存，120 秒的请求超时对大文件必然超时。
fn build_download_client() -> Result<reqwest::Client, RemoteError> {
    reqwest::Client::builder()
        .user_agent(ALIYUN_USER_AGENT)
        .timeout(ALIYUN_FILE_TIMEOUT)
        .connect_timeout(ALIYUN_CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|error| RemoteError::unknown(format!("阿里云盘下载客户端初始化失败：{error}")))
}

/// 上传用的 HTTP 客户端。
///
/// 与 [`build_client`] 的区别只有一条，但这条是**硬约束**：
/// **不带任何默认头**（不设 `Content-Type`、不设 `Accept`）。
///
/// OSS 的预签名直链把 `Content-Type` 算进签名 —— 多带一个
/// `Content-Type: application/oct-stream` 就是 403 `SignatureDoesNotMatch`
/// （实测）。所以这个客户端**刻意**什么都不设。
fn build_upload_client() -> Result<reqwest::Client, RemoteError> {
    reqwest::Client::builder()
        .user_agent(ALIYUN_USER_AGENT)
        .timeout(ALIYUN_FILE_TIMEOUT)
        .connect_timeout(ALIYUN_CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| RemoteError::unknown(format!("阿里云盘上传客户端初始化失败：{error}")))
}

/// 把传输层失败分类成 [`RemoteErrorKind`]。
fn classify_transport(error: &reqwest::Error) -> RemoteErrorKind {
    if error.is_timeout() || error.is_connect() {
        RemoteErrorKind::Unreachable
    } else if error.is_body() || error.is_decode() {
        RemoteErrorKind::Disconnected
    } else {
        RemoteErrorKind::Unknown
    }
}

/// 分类文案：每条都落到「用户下一步动作」，不含英文原始错误。
///
/// ⚠️ 文案里**不许**出现 `code` / `token` 这类字样（契约 §6.1 锚点 9）。
fn transport_message_for(kind: RemoteErrorKind) -> &'static str {
    match kind {
        RemoteErrorKind::Unreachable => {
            "无法连接阿里云盘：网络不可用、DNS 解析失败或服务器超时，请检查网络连接后重试。"
        }
        RemoteErrorKind::Disconnected => "与阿里云盘的连接中断：请检查网络后重试。",
        RemoteErrorKind::Protocol => {
            "阿里云盘返回了无法识别的响应：请稍后重试，若持续失败请重新授权。"
        }
        _ => "访问阿里云盘失败：网络或网盘服务暂时不可用，请稍后重试。",
    }
}

/// 把一次传输层失败翻译成本项目的错误。
fn transport_error(context: &str, error: &reqwest::Error) -> RemoteError {
    let kind = classify_transport(error);
    log::warn!("Aliyun transport failure[{context}] kind={kind:?} raw={error}");
    RemoteError::new(kind, transport_message_for(kind))
}

/// 发一个带鉴权头的 JSON POST，返回**原始的**状态码与响应体。
///
/// 状态码与响应体都交回调用方 —— 写操作要靠它区分「已经存在」
/// （按成功处理）与「真失败」。
async fn send_authorized_json(
    client: &reqwest::Client,
    url: &str,
    body: &str,
    session: &AliyunAuthSession,
) -> Result<(u16, String), RemoteError> {
    let authorization = session
        .authorization_header_value()
        .ok_or_else(|| RemoteError::auth("阿里云盘会话凭据缺失，请重新授权。"))?;

    let response = client
        .post(url)
        .header(reqwest::header::AUTHORIZATION, authorization)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .await
        .map_err(|error| transport_error("访问阿里云盘失败", &error))?;

    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| transport_error("读取阿里云盘响应失败", &error))?;

    let text = String::from_utf8(bytes.to_vec())
        .map_err(|_| RemoteError::encoding("阿里云盘返回的内容编码无法解析。"))?;

    Ok((status.as_u16(), text))
}

/// 发一个 POST（JSON 请求体）并把响应体读成 UTF-8 文本。
///
/// 响应体**不是**用 `reqwest` 的 `text()` 读的：那个方法被 `charset` feature
/// 门控，而本 crate 的 `reqwest` 是 `default-features = false`，没开这个 feature
/// ⇒ 用了就编不过。这里用 `bytes()` + `String::from_utf8`。
async fn post_json_text(
    client: &reqwest::Client,
    url: &str,
    body: &str,
    session: &AliyunAuthSession,
) -> Result<String, RemoteError> {
    let (status, text) = send_authorized_json(client, url, body, session).await?;

    if !(200..=299).contains(&status) {
        // 只记状态码与错误码，**不记响应原文** —— 原文由服务端控制，
        // 里面可能夹着不该进日志的东西。
        let failure = super::aliyun_api::aliyun_parse_failure(&text);
        log::warn!(
            "Aliyun API failure url={url} status={status} code={:?}",
            failure.code
        );
        return Err(aliyun_map_failure(status, &text));
    }

    Ok(text)
}

/// 发一个**写操作**的 POST（JSON 请求体），失败时用写操作专属的文案。
///
/// 与 [`post_json_text`] 只有两处不同：
///
/// 1. 文案里带上**是哪个操作**失败了 —— 用户看到的是「新建目录时…」，
///    而不是笼统的「响应内容无法识别」。
/// 2. 服务端原文**仍然不进**用户可见文案（契约 §6.1 锚点 9）。
async fn post_write_json_text(
    client: &reqwest::Client,
    url: &str,
    body: &str,
    session: &AliyunAuthSession,
    operation: &'static str,
) -> Result<String, RemoteError> {
    let (status, text) = send_authorized_json(client, url, body, session).await?;

    if !(200..=299).contains(&status) {
        let failure = super::aliyun_api::aliyun_parse_failure(&text);
        log::warn!(
            "Aliyun write failure op={operation} url={url} status={status} code={:?}",
            failure.code
        );
        return Err(aliyun_map_write_failure(operation, status, &text));
    }

    Ok(text)
}

/// 发一个 POST（JSON 请求体，**不带** `Authorization`）并把响应体读成 UTF-8 文本。
///
/// 换 token / 刷新 token 走这条路：鉴权靠请求体里的 `code` / `refresh_token`，
/// **不带** `Authorization`、不带 cookie。
///
/// `x-requested-with: XMLHttpRequest` 是 bundle 原文带的，照抄。
async fn post_json_text_unauthenticated(
    client: &reqwest::Client,
    url: &str,
    body: &str,
) -> Result<String, RemoteError> {
    let (status, text) = post_json_text_unauthenticated_raw(client, url, body).await?;

    if !(200..=299).contains(&status) {
        // 只记状态码与错误码，**不记响应原文** —— 原文由服务端控制，
        // 里面可能夹着不该进日志的东西。
        let failure = super::aliyun_api::aliyun_parse_failure(&text);
        log::warn!(
            "Aliyun token failure url={url} status={status} code={:?}",
            failure.code
        );
        return Err(aliyun_map_failure(status, &text));
    }

    Ok(text)
}

/// 发同样的 POST，但把 `(HTTP 状态码, 响应体文本)` **原样**交回：
/// 不映射成错误、不打日志。
///
/// 为什么需要这一层：续期候选链要按服务端错误码决定「换下一级 client_id」
/// 还是「直接失败」，而 [`aliyun_map_failure`] 会把错误码吃掉（只剩
/// 「返回了 HTTP 400」）⇒ 必须拿到未加工的状态码与响应体。
async fn post_json_text_unauthenticated_raw(
    client: &reqwest::Client,
    url: &str,
    body: &str,
) -> Result<(u16, String), RemoteError> {
    let response = client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("x-requested-with", "XMLHttpRequest")
        .body(body.to_string())
        .send()
        .await
        .map_err(|error| transport_error("访问阿里云盘失败", &error))?;

    let status = response.status().as_u16();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| transport_error("读取阿里云盘响应失败", &error))?;

    let text = String::from_utf8(bytes.to_vec())
        .map_err(|_| RemoteError::encoding("阿里云盘返回的内容编码无法解析。"))?;

    Ok((status, text))
}

/// 下载直链回源时**必带**的请求头。
///
/// 坐实（`.superpowers/sdd/b28-api-probe.md`）：
/// - 只带 `Authorization` ⇒ HTTP 400 `AuthorizationArgumentError`；
/// - 带上 `Referer: https://www.alipan.com/` ⇒ 200；
/// - 再加 `Range: bytes=0-1023` ⇒ 206（精确 1024 字节）。
///
/// ⇒ **不再逐个候选头组合试探**：批次 28 坐实了 `Referer` 才是关键那个头，
/// 加 `Authorization` **替代不了**它。
///
/// 返回 `Vec<(String, String)>` 而不是 reqwest 的类型 ⇒ 这层保持纯函数、可单测。
pub fn aliyun_download_header_pairs(session: &AliyunAuthSession) -> Vec<(String, String)> {
    let mut headers: Vec<(String, String)> = Vec::new();

    headers.push((
        "Referer".to_string(),
        super::aliyun_api::ALIYUN_DOWNLOAD_REFERER.to_string(),
    ));
    headers.push(("User-Agent".to_string(), ALIYUN_USER_AGENT.to_string()));

    if let Some(authorization) = session.authorization_header_value() {
        headers.push(("Authorization".to_string(), authorization));
    }

    headers
}

/// 把响应体收进内存，边收边判上限。
async fn collect_body(
    response: reqwest::Response,
    max_bytes: u64,
) -> Result<Vec<u8>, RemoteError> {
    let mut collected: Vec<u8> = Vec::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| transport_error("下载阿里云盘文件中断", &error))?;

        if collected.len() as u64 + chunk.len() as u64 > max_bytes {
            return Err(RemoteError::unsupported(format!(
                "文件超过 {} MiB，阿里云盘只读阶段不下载这么大的文件。",
                max_bytes / (1024 * 1024)
            )));
        }

        collected.extend_from_slice(&chunk);
    }

    Ok(collected)
}

/// 下载一个直链指向的内容，最多接受 `max_bytes` 字节。
///
/// **必带 `Referer`**（见 [`aliyun_download_header_pairs`]）：少了它服务端回
/// 400 `AuthorizationArgumentError`。被拒 ⇒ 明确报错，带上状态码
/// （**不含**响应原文）。
async fn download_file(
    client: &reqwest::Client,
    session: &AliyunAuthSession,
    url: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, RemoteError> {
    let mut request = client.get(url);
    for (name, value) in aliyun_download_header_pairs(session) {
        request = request.header(name, value);
    }

    let response = request
        .send()
        .await
        .map_err(|error| transport_error("下载阿里云盘文件失败", &error))?;

    let status = response.status();
    if status.is_success() {
        return collect_body(response, max_bytes).await;
    }

    log::warn!("Aliyun download rejected status={}", status.as_u16());

    match status.as_u16() {
        401 | 403 => Err(RemoteError::auth(
            "阿里云盘拒绝了这次下载，请重新授权后再试。",
        )),
        status => Err(RemoteError::protocol(format!(
            "阿里云盘的下载直链被拒绝了（HTTP {status}），请重新授权后再试。"
        ))),
    }
}

/// 上传用的分块流。
///
/// 每次产出一块**拷贝**（块很小，内存峰值只多一块），并把这一块的长度
/// 报进进度通道。
///
/// 之所以要拷贝：reqwest 的流式 body 要求 `Stream` 是 `'static`，而文件
/// 内容是**借来**的。拷贝总量 = 文件大小，但**峰值内存只多一块**
/// （`ALIYUN_UPLOAD_CHUNK_BYTES`），不会翻倍。
fn aliyun_upload_chunk_stream(
    content: Arc<Vec<u8>>,
    range: AliyunPartRange,
    progress: tokio::sync::mpsc::UnboundedSender<u64>,
) -> impl futures_util::Stream<Item = Result<Vec<u8>, std::io::Error>> + Send + 'static {
    let chunk_size = ALIYUN_UPLOAD_CHUNK_BYTES;
    let state = (content, range.start, range.end, progress);

    futures_util::stream::unfold(
        state,
        move |(content, cursor, end, progress)| async move {
            if cursor >= end {
                return None;
            }

            let take = std::cmp::min(chunk_size as u64, end - cursor) as usize;
            let start = cursor as usize;
            let chunk = content[start..start + take].to_vec();

            let _ = progress.send(take as u64);

            Some((Ok(chunk), (content, cursor + take as u64, end, progress)))
        },
    )
}

/// 把进度通道里的增量转成 `on_progress` 回调。
///
/// 通道关闭（= 流已经结束）就**返回**，不去等一个不会再来的增量。
async fn report_upload_progress(
    mut progress_rx: tokio::sync::mpsc::UnboundedReceiver<u64>,
    on_progress: &(dyn Fn(u64) + Send + Sync),
    written: &mut u64,
) {
    while let Some(delta) = progress_rx.recv().await {
        *written = written.saturating_add(delta);
        on_progress(*written);
    }
}

/// 上传一个分片，并在传输过程中**如实**上报已发送字节数。
///
/// 进度是「已经交给 socket 的字节数」，不是「服务端已收下的字节数」——
/// 这是所有 HTTP 客户端的常态，但它**绝不撒谎**：回调里给的值永远是真正
/// 已经写出去的字节数。
async fn upload_one_part(
    client: &reqwest::Client,
    upload_url: &str,
    content: Arc<Vec<u8>>,
    range: AliyunPartRange,
    on_progress: &(dyn Fn(u64) + Send + Sync),
    written: &mut u64,
) -> Result<(), RemoteError> {
    let length = range.end.saturating_sub(range.start);

    let (progress_tx, progress_rx) = tokio::sync::mpsc::unbounded_channel::<u64>();
    let stream = aliyun_upload_chunk_stream(content, range, progress_tx);
    let body = reqwest::Body::wrap_stream(stream);

    let request = client
        .put(upload_url)
        // OSS 的签名不含 `Content-Length`，但它**要求**请求有确定的正文长度
        // （`Transfer-Encoding: chunked` 会被拒）⇒ 这里手工写死长度。
        // hyper 会尊重调用方自己设的 `Content-Length`（`hyper` 的
        // `proto/h1/role.rs`：用户设的头优先于 body 自己的长度），
        // 所以线上看到的既不是 chunked 也不是没有长度。
        .header(reqwest::header::CONTENT_LENGTH, length)
        // ⚠️ **刻意不设 Content-Type**：OSS 的签名把它算进去了，
        // 多一个 `application/oct-stream` 就是 403 `SignatureDoesNotMatch`。
        .body(body);

    // 上传与进度汇报**并行**跑：进度通道一断就说明流已经结束。
    //
    // 用 `join` 而不是 `tokio::select!` —— 后者要 `tokio` 的 `macros`
    // feature，而本 crate 的 `tokio` 依赖里**没开**（只开了
    // `rt` / `sync` / `io-util` / `net` / `time`），生产构建里用不了。
    let send = request.send();
    let report = report_upload_progress(progress_rx, on_progress, written);
    let (response, ()) = futures_util::future::join(send, report).await;

    let response = response.map_err(|error| transport_error("上传到阿里云盘失败", &error))?;

    let status = response.status();
    if !status.is_success() {
        log::warn!("Aliyun upload rejected status={}", status.as_u16());
        return Err(RemoteError::protocol(format!(
            "阿里云盘的上传被拒绝了（HTTP {}），请重试。",
            status.as_u16()
        )));
    }

    Ok(())
}

/// 走真实 HTTP 的实现。
pub struct HttpAliyunApi {
    client: reqwest::Client,
    download_client: reqwest::Client,
    /// 上传用的客户端：**不带默认头**（见 [`build_upload_client`]）。
    upload_client: reqwest::Client,
}

impl HttpAliyunApi {
    /// 生产实现：真 HTTP。
    pub fn new() -> Result<Self, RemoteError> {
        Ok(Self {
            client: build_client()?,
            download_client: build_download_client()?,
            upload_client: build_upload_client()?,
        })
    }

    /// 用授权码换一份 token（**web 分支**：JSON 体，免密钥）。
    ///
    /// 请求体只有 `{client_id, code}` —— bundle 原文如此，**没有** `client_secret`。
    pub async fn exchange_code(
        &self,
        client_id: &str,
        code: &str,
    ) -> Result<AliyunTokenPayload, RemoteError> {
        let body = aliyun_authorization_code_body(client_id, code);
        let text =
            post_json_text_unauthenticated(&self.client, aliyun_access_token_url(), &body.to_string())
                .await?;
        aliyun_parse_token_payload(&text)
    }
}

#[async_trait]
impl AliyunApi for HttpAliyunApi {
    async fn list_page(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        parent_file_id: &str,
        marker: Option<&str>,
    ) -> Result<AliyunListPayload, RemoteError> {
        let body = super::aliyun_api::aliyun_list_request_body(
            drive_id,
            parent_file_id,
            i64::from(ALIYUN_LIST_LIMIT_DEFAULT),
            marker,
            ALIYUN_LIST_ORDER_BY,
            ALIYUN_LIST_ORDER_DIRECTION,
        );

        let text = post_json_text(
            &self.client,
            &aliyun_list_url(),
            &body.to_string(),
            session,
        )
        .await?;

        aliyun_parse_list_payload(&text)
    }

    async fn file_info(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        file_id: &str,
    ) -> Result<AliyunFileEntry, RemoteError> {
        let body = super::aliyun_api::aliyun_get_request_body(drive_id, file_id);
        let text = post_json_text(
            &self.client,
            aliyun_get_url(),
            &body.to_string(),
            session,
        )
        .await?;

        aliyun_parse_entry_body(&text)
    }

    async fn download_url(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        file_id: &str,
    ) -> Result<String, RemoteError> {
        let body = super::aliyun_api::aliyun_download_url_request_body(drive_id, file_id);
        let text = post_json_text(
            &self.client,
            aliyun_download_url_api(),
            &body.to_string(),
            session,
        )
        .await?;

        aliyun_parse_download_url(&text)
    }

    async fn download(
        &self,
        session: &AliyunAuthSession,
        url: &str,
        max_bytes: u64,
    ) -> Result<Vec<u8>, RemoteError> {
        download_file(&self.download_client, session, url, max_bytes).await
    }

    async fn refresh_session(
        &self,
        session: &AliyunAuthSession,
    ) -> Result<AliyunAuthSession, RemoteError> {
        let Some(refresh_token) = session
            .refresh_token
            .as_deref()
            .map(str::trim)
            .filter(|token| !token.is_empty())
        else {
            return Err(RemoteError::auth(
                "阿里云盘登录已过期，且没有可用于续期的刷新凭证，请重新授权。",
            ));
        };

        // 批次 34 真机实测订正：只把空串补成网页端公开 id **不够**。
        // 服务端先认 `refresh_token` 再校验 `client_id` ⇒ 传错 client 会回
        // 400 `InvalidParameter.ClientId`（假 token 矩阵：假 token 配任何
        // client_id 都只回 `InvalidParameter.RefreshToken`，说明这条校验
        // 只在真 token 上触发）。所以按候选链逐级降级，只有 ClientId 这一种
        // 失败才继续往下试；别的失败一律立刻停（别把凭证问题当 client 问题）。
        let candidates = aliyun_refresh_client_id_candidates(&session.client_id);
        let stored_client_id = aliyun_refresh_client_id(&session.client_id);
        let mut last_error: Option<RemoteError> = None;

        for (index, candidate) in candidates.iter().enumerate() {
            let body =
                aliyun_refresh_token_body_with_client_id(candidate.as_deref(), refresh_token);
            let (status, text) = post_json_text_unauthenticated_raw(
                &self.client,
                aliyun_refresh_token_url(),
                &body.to_string(),
            )
            .await?;

            // 无条件打一行：存档是加密的，外部读不到 `client_id` 实际取值，
            // 这行日志是唯一能坐实「到底在拿哪个 client 换 token」的证据。
            log::warn!(
                "Aliyun refresh attempt index={index} client_id={:?} status={status} code={:?}",
                candidate.as_deref(),
                super::aliyun_api::aliyun_parse_failure(&text).code
            );

            if (200..=299).contains(&status) {
                let payload = aliyun_parse_token_payload(&text)?;

                return Ok(aliyun_session_from_token_payload(
                    &payload,
                    stored_client_id,
                    session.redirect_uri.clone(),
                    session.scope.clone(),
                    aliyun_now_unix(),
                    session.drive_id.clone(),
                    session.refresh_token.clone(),
                    session.nick_name.clone(),
                ));
            }

            let retry_without_this_client_id =
                status == 400 && super::aliyun_api::aliyun_failure_rejects_client_id(&text);
            last_error = Some(aliyun_map_failure(status, &text));

            if !retry_without_this_client_id {
                break;
            }
        }

        Err(last_error.unwrap_or_else(|| {
            RemoteError::auth("阿里云盘登录已过期，且没有可用的续期 client，请重新授权。")
        }))
    }

    async fn create_folder(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        parent_file_id: &str,
        name: &str,
    ) -> Result<bool, RemoteError> {
        let body = aliyun_create_folder_body(drive_id, parent_file_id, name);
        let (status, text) =
            send_authorized_json(&self.client, ALIYUN_CREATE_URL, &body.to_string(), session)
                .await?;

        if (200..=299).contains(&status) {
            return Ok(false);
        }

        // 「已经存在」按成功处理（`make_dir` 的契约：已存在视为成功）。
        if aliyun_failure_is_already_exist(&text) {
            return Ok(true);
        }

        Err(aliyun_map_write_failure("新建目录", status, &text))
    }

    async fn create_file(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        parent_file_id: &str,
        name: &str,
    ) -> Result<AliyunCreateResult, RemoteError> {
        let body = aliyun_create_file_body(drive_id, parent_file_id, name);
        let text = post_write_json_text(
            &self.client,
            ALIYUN_CREATE_URL,
            &body.to_string(),
            session,
            "新建文件",
        )
        .await?;

        aliyun_parse_create_payload(&text)
    }

    async fn upload_parts(
        &self,
        _session: &AliyunAuthSession,
        ranges: &[AliyunPartRange],
        content: Arc<Vec<u8>>,
        on_progress: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<(), RemoteError> {
        // ⚠️ **刻意不带 `Authorization`**：OSS 的预签名直链**自己就是授权**
        // （签名在 URL 的 query 里）。再塞一个 `Authorization` 头，OSS 会改
        // 按 header 里的签名去校验 ⇒ 403。
        let mut written: u64 = 0;

        for range in ranges {
            if range.upload_url.trim().is_empty() {
                return Err(RemoteError::protocol(
                    "阿里云盘没有给出这一片的上传地址，无法上传这个文件。",
                ));
            }

            upload_one_part(
                &self.upload_client,
                &range.upload_url,
                Arc::clone(&content),
                range.clone(),
                on_progress,
                &mut written,
            )
            .await?;
        }

        Ok(())
    }

    async fn complete_upload(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        file_id: &str,
        upload_id: &str,
        revision_id: &str,
    ) -> Result<(), RemoteError> {
        let body = aliyun_complete_upload_body(drive_id, file_id, upload_id, revision_id);
        post_write_json_text(
            &self.client,
            ALIYUN_COMPLETE_URL,
            &body.to_string(),
            session,
            "完成上传",
        )
        .await?;
        Ok(())
    }

    async fn rename_entry(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        file_id: &str,
        name: &str,
    ) -> Result<(), RemoteError> {
        let body = aliyun_rename_body(drive_id, file_id, name);
        post_write_json_text(
            &self.client,
            ALIYUN_UPDATE_URL,
            &body.to_string(),
            session,
            "重命名",
        )
        .await?;
        Ok(())
    }

    async fn delete_entry(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        file_id: &str,
        entry_type: &str,
    ) -> Result<(), RemoteError> {
        let body = aliyun_delete_body(drive_id, file_id, entry_type);
        post_write_json_text(
            &self.client,
            ALIYUN_DELETE_URL,
            &body.to_string(),
            session,
            "删除",
        )
        .await?;
        Ok(())
    }

    async fn move_entry(
        &self,
        session: &AliyunAuthSession,
        drive_id: &str,
        file_id: &str,
        to_parent_file_id: &str,
    ) -> Result<(), RemoteError> {
        let body = aliyun_move_body(drive_id, file_id, to_parent_file_id);
        post_write_json_text(
            &self.client,
            ALIYUN_MOVE_URL,
            &body.to_string(),
            session,
            "移动",
        )
        .await?;
        Ok(())
    }
}

/// 续期请求用的 `client_id`。
///
/// 会话里存了就用存的；**空值回退到网页端公开 id**。批次 34 真机实测：
/// 批次 29 的扫码登录把 `client_id` 写成了空串，access_token 一过期，
/// `POST /token/refresh` 收到的请求体就是 `{"client_id":""}`，服务端回
/// `400 InvalidParameter.ClientId` ⇒ 会话永远不能自续，用户每次都得重扫。
fn aliyun_refresh_client_id(stored: &str) -> &str {
    let stored = stored.trim();
    if stored.is_empty() {
        super::aliyun_api::ALIYUN_WEB_CLIENT_ID
    } else {
        stored
    }
}

/// 用一次换 token 的响应拼出一份会话。
///
/// - **有效期一律以响应里的 `expires_in` / `refresh_token_expires_in` 为准**
///   （契约 §1.2），本函数不写死任何有效期。
/// - 响应里没带 `refresh_token` ⇒ **沿用旧的那一份**（有些模式不返回新的）。
///   服务端说它已经失效时，下一次续期会明确失败 ⇒ 用户重新授权，不会静默错。
/// - 响应里没带 `drive_id` ⇒ 沿用旧的那一份。
/// - **web 分支没有客户端密钥** ⇒ 这份会话里**没有** `client_secret`。
#[allow(clippy::too_many_arguments)]
pub fn aliyun_session_from_token_payload(
    payload: &AliyunTokenPayload,
    client_id: &str,
    redirect_uri: Option<String>,
    scope: Option<String>,
    now_unix: i64,
    fallback_drive_id: Option<String>,
    fallback_refresh_token: Option<String>,
    fallback_nick_name: Option<String>,
) -> AliyunAuthSession {
    AliyunAuthSession {
        access_token: payload.access_token.clone(),
        refresh_token: payload.refresh_token.clone().or(fallback_refresh_token),
        expires_at_unix: payload.expires_in.map(|seconds| now_unix + seconds),
        refresh_expires_at_unix: payload
            .refresh_token_expires_in
            .map(|seconds| now_unix + seconds),
        drive_id: payload.drive_id.clone().or(fallback_drive_id),
        client_id: client_id.to_string(),
        redirect_uri,
        scope,
        token_type: payload
            .token_type
            .clone()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "Bearer".to_string()),
        nick_name: payload.nick_name.clone().or(fallback_nick_name),
        device_id: None,
    }
}

/// 给一类操作套上总时限。
async fn with_timeout<T>(
    timeout: Duration,
    what: &'static str,
    work: impl Future<Output = Result<T, RemoteError>>,
) -> Result<T, RemoteError> {
    match tokio::time::timeout(timeout, work).await {
        Ok(result) => result,
        Err(_) => Err(RemoteError::disconnected(format!(
            "{what}超过 {} 秒仍未完成，已放弃这次操作",
            timeout.as_secs()
        ))),
    }
}

/// 阿里云盘的远程目录 provider。
///
/// ## 凭据从哪来
///
/// 阿里云盘的连接在连接表里只有元数据（host / port / nickname），
/// **整份 OAuth 会话存在 DPAPI 密文里**，键就是连接的 `credential_key`。
/// 所以本 provider **不**走 `registry::resolve_connection` —— 那条路会把密文
/// 当密码读出来，而阿里云盘要的不是「密码」，是「会话」。
///
/// ## 路径怎么变成 file_id
///
/// 阿里云盘的条目**没有路径**，服务端只有 `file_id`。所以「远端路径 → 目录」
/// 只能从根一级一级往下走：每往下一级都要列出父目录、按名字找到那一项、
/// 拿它的 `file_id`。这是接口形状决定的，不是实现偷懒。
pub struct AliyunRemoteProvider {
    user_data_dir: PathBuf,
    api: Arc<dyn AliyunApi>,
    sessions: Arc<dyn AliyunSessionStore>,
    directory_timeout: Duration,
    file_timeout: Duration,
}

impl AliyunRemoteProvider {
    /// 生产实现：真 HTTP + 真 DPAPI。
    pub fn new(user_data_dir: &Path, app_data_dir: &Path) -> Result<Self, RemoteError> {
        Ok(Self::with_parts(
            user_data_dir,
            Arc::new(HttpAliyunApi::new()?),
            Arc::new(DpapiAliyunSessionStore::new(app_data_dir)),
        ))
    }

    /// **不返回 `Result`** 的构造：HTTP 客户端建不起来时，把错误推迟到第一次调用。
    ///
    /// 给 [`crate::remote::commands::provider_with_protocol`] 用 —— 它的签名是
    /// `Box<dyn RemoteProvider>`，没有地方安放 `Err`。
    pub fn new_deferred(user_data_dir: &Path, app_data_dir: &Path) -> Self {
        let sessions: Arc<dyn AliyunSessionStore> =
            Arc::new(DpapiAliyunSessionStore::new(app_data_dir));

        match HttpAliyunApi::new() {
            Ok(api) => Self::with_parts(user_data_dir, Arc::new(api), sessions),
            Err(error) => Self::with_parts(
                user_data_dir,
                Arc::new(UnavailableAliyunApi::new(error)),
                sessions,
            ),
        }
    }

    /// 注入实现。**只给测试用** —— 生产路径走 [`Self::new`]。
    pub fn with_parts(
        user_data_dir: &Path,
        api: Arc<dyn AliyunApi>,
        sessions: Arc<dyn AliyunSessionStore>,
    ) -> Self {
        Self {
            user_data_dir: user_data_dir.to_path_buf(),
            api,
            sessions,
            directory_timeout: ALIYUN_DIRECTORY_TIMEOUT,
            file_timeout: ALIYUN_FILE_TIMEOUT,
        }
    }

    /// 覆盖时限。**只给测试用**（用来验超时分支）。
    pub fn with_timeouts(mut self, directory_timeout: Duration, file_timeout: Duration) -> Self {
        self.directory_timeout = directory_timeout;
        self.file_timeout = file_timeout;
        self
    }

    /// 入参处理：挡控制字符 → 规范化路径 → 找连接。
    ///
    /// **全部在开网络之前完成**：控制字符是本地判定，路径不合法也是。
    fn prepare_path(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> Result<(RemoteConnection, String), RemoteError> {
        reject_path_control_characters(remote_path)?;

        let normalized = normalize_remote_path(remote_path).ok_or_else(|| {
            RemoteError::protocol(format!("远端路径含有不合法的段: {remote_path}"))
        })?;

        let connection = registry::find_connection(&self.user_data_dir, connection_id)?;

        Ok((connection, normalized))
    }

    /// 入参处理 + 取会话（必要时**先刷新**）。
    ///
    /// 找不到会话一律报 `Auth` —— 用户的处置只有一个动作：**重新授权**。
    async fn prepare(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> Result<(String, AliyunAuthSession, String), RemoteError> {
        let (connection, normalized) = self.prepare_path(connection_id, remote_path)?;

        let Some(key) = connection
            .credential_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
        else {
            return Err(RemoteError::auth(
                "这条阿里云盘连接还没有保存登录凭据，请先完成授权。",
            ));
        };

        let Some(mut session) = self.sessions.load(key)? else {
            return Err(RemoteError::auth(
                "阿里云盘还没有授权，请先在设置里完成授权。",
            ));
        };

        // **主动**发现过期：省掉一次注定 401 的请求。
        // 刷新失败 ⇒ 明确告诉用户「请重新授权」，而不是把一次 401 报成网络问题。
        if aliyun_is_expired(&session, aliyun_now_unix()) {
            let refreshed = self.refresh_session(key, &session).await?;
            session = refreshed;
        }

        Ok((key.to_string(), session, normalized))
    }

    /// 用刷新凭证换一份新会话，并**写回存档**。
    ///
    /// 写回是必须的：刷新凭证**单次有效**（契约 §1.2），不写回去下一次就用不了。
    async fn refresh_session(
        &self,
        key: &str,
        session: &AliyunAuthSession,
    ) -> Result<AliyunAuthSession, RemoteError> {
        if !session.has_refresh_token() {
            return Err(RemoteError::auth(
                "阿里云盘登录已过期，且没有可用于续期的刷新凭证，请重新授权。",
            ));
        }

        let refreshed = match self.api.refresh_session(session).await {
            Ok(refreshed) => refreshed,
            Err(error) => {
                // 续期失败只有一种用户处置：**重新授权**。
                // 但网络问题不该被说成「请重新授权」—— 那会把用户引向错误的动作。
                return match error.kind {
                    RemoteErrorKind::Auth => Err(RemoteError::auth(
                        "阿里云盘登录已过期且自动续期失败，请重新授权。",
                    )),
                    _ => Err(error),
                };
            }
        };

        self.sessions.save(key, &refreshed)?;

        Ok(refreshed)
    }

    /// 列一页；遇到「授权失效」就**先刷新再重试一次**。
    async fn api_list_page(
        &self,
        key: &str,
        session: &mut AliyunAuthSession,
        drive_id: &str,
        parent_file_id: &str,
        marker: Option<&str>,
    ) -> Result<AliyunListPayload, RemoteError> {
        let attempt = {
            let snapshot = session.clone();
            self.api
                .list_page(&snapshot, drive_id, parent_file_id, marker)
                .await
        };

        match attempt {
            Ok(payload) => Ok(payload),
            Err(error) if error.kind == RemoteErrorKind::Auth => {
                let refreshed = self.refresh_session(key, session).await?;
                *session = refreshed;
                let snapshot = session.clone();
                self.api
                    .list_page(&snapshot, drive_id, parent_file_id, marker)
                    .await
            }
            Err(error) => Err(error),
        }
    }

    /// 取元数据（`openFile/get`）；遇到「授权失效」就**先刷新再重试一次**。
    async fn api_file_info(
        &self,
        key: &str,
        session: &mut AliyunAuthSession,
        drive_id: &str,
        file_id: &str,
    ) -> Result<AliyunFileEntry, RemoteError> {
        let attempt = {
            let snapshot = session.clone();
            self.api.file_info(&snapshot, drive_id, file_id).await
        };

        match attempt {
            Ok(entry) => Ok(entry),
            Err(error) if error.kind == RemoteErrorKind::Auth => {
                let refreshed = self.refresh_session(key, session).await?;
                *session = refreshed;
                let snapshot = session.clone();
                self.api.file_info(&snapshot, drive_id, file_id).await
            }
            Err(error) => Err(error),
        }
    }

    /// 取直链；遇到「授权失效」就**先刷新再重试一次**。
    async fn api_download_url(
        &self,
        key: &str,
        session: &mut AliyunAuthSession,
        drive_id: &str,
        file_id: &str,
    ) -> Result<String, RemoteError> {
        let attempt = {
            let snapshot = session.clone();
            self.api.download_url(&snapshot, drive_id, file_id).await
        };

        match attempt {
            Ok(url) => Ok(url),
            Err(error) if error.kind == RemoteErrorKind::Auth => {
                let refreshed = self.refresh_session(key, session).await?;
                *session = refreshed;
                let snapshot = session.clone();
                self.api.download_url(&snapshot, drive_id, file_id).await
            }
            Err(error) => Err(error),
        }
    }

    /// 取当前登录账号应当使用的 `drive_id`。
    ///
    /// 顺序：环境变量 `ALIYUN_DRIVE_ID` → 会话里的 `drive_id`
    /// （登录时从 `/v2/user/get` 的 `default_drive_id` 取到并存进会话）。
    ///
    /// **取不到就明确报错** —— 批次 28 推翻了批次 27 的「默认主盘常量」：
    /// `954264082` 是 resource 盘、`25998870` 是保险箱（403），
    /// 拿它们顶上只会列出错的目录。
    fn resolve_drive_id(&self, session: &AliyunAuthSession) -> Result<String, RemoteError> {
        aliyun_resolve_drive_id(session.drive_id.as_deref())
            .ok_or_else(|| RemoteError::auth("阿里云盘的主盘信息还没拿到，请重新扫码登录一次。"))
    }

    /// 把一条**远端路径**解析成阿里云盘的目录 `file_id`。
    ///
    /// 根目录恒为 [`ALIYUN_ROOT_FILE_ID`]。往下一级就要列出父目录、按名字找
    /// 那一项、拿它的 `file_id` —— 阿里云盘没有「按路径查目录」的能力。
    ///
    /// 代价：深度为 N 的路径要 N 次列表请求。
    async fn resolve_dir_file_id(
        &self,
        key: &str,
        session: &mut AliyunAuthSession,
        drive_id: &str,
        remote_path: &str,
    ) -> Result<String, RemoteError> {
        let mut file_id = ALIYUN_ROOT_FILE_ID.to_string();

        for segment in remote_path.split('/').filter(|part| !part.is_empty()) {
            let Some(entry) = self
                .find_entry(key, session, drive_id, &file_id, segment)
                .await?
            else {
                return Err(RemoteError::not_found(format!(
                    "阿里云盘里没有这个目录：{segment}"
                )));
            };

            if !super::aliyun_api::aliyun_entry_is_directory(&entry) {
                return Err(RemoteError::protocol(format!(
                    "「{segment}」不是一个目录。"
                )));
            }

            file_id = entry.file_id.clone();
        }

        Ok(file_id)
    }

    /// 在一个目录里**跟着游标**翻页找同名条目。
    async fn find_entry(
        &self,
        key: &str,
        session: &mut AliyunAuthSession,
        drive_id: &str,
        parent_file_id: &str,
        name: &str,
    ) -> Result<Option<AliyunFileEntry>, RemoteError> {
        let mut marker: Option<String> = None;
        let mut pages = 0u32;

        loop {
            pages += 1;
            if pages > ALIYUN_MAX_LIST_PAGES {
                return Err(aliyun_list_page_limit_error());
            }

            let payload = self
                .api_list_page(key, session, drive_id, parent_file_id, marker.as_deref())
                .await?;

            if let Some(found) = payload.items.iter().find(|entry| entry.name == name) {
                return Ok(Some(found.clone()));
            }

            match aliyun_next_marker(&payload) {
                Some(next) => marker = Some(next),
                None => return Ok(None),
            }
        }
    }

    /// 跟着游标列一个目录的全部条目。
    async fn list_raw(
        &self,
        key: &str,
        session: &mut AliyunAuthSession,
        drive_id: &str,
        parent_file_id: &str,
    ) -> Result<Vec<AliyunFileEntry>, RemoteError> {
        let mut entries: Vec<AliyunFileEntry> = Vec::new();
        let mut marker: Option<String> = None;
        let mut pages = 0u32;

        loop {
            pages += 1;
            if pages > ALIYUN_MAX_LIST_PAGES {
                return Err(aliyun_list_page_limit_error());
            }

            let payload = self
                .api_list_page(key, session, drive_id, parent_file_id, marker.as_deref())
                .await?;

            let next = aliyun_next_marker(&payload);
            entries.extend(payload.items);

            if entries.len() > ALIYUN_MAX_LIST_ENTRIES {
                return Err(aliyun_list_entry_limit_error());
            }

            match next {
                Some(next) => marker = Some(next),
                None => break,
            }
        }

        Ok(entries)
    }

    /// 取当前登录账号的昵称（只用于展示「已授权：<昵称>」）。
    ///
    /// **不联网** —— 昵称在换 token 时就拿到了，存在会话里。
    pub async fn account_nickname(&self, connection_id: &str) -> Result<String, RemoteError> {
        let (_, session, _) = self.prepare(connection_id, "/").await?;
        Ok(session.nick_name.unwrap_or_default())
    }

    /// 把一个条目**移动**到另一个目录下（`file/move`）。
    ///
    /// `to_path` 是**目标目录**，条目名字**不变** —— 这是「移动」而不是「改名」
    /// （改名走 [`RemoteProvider::rename_path`]，它只能在原目录内改名）。
    ///
    /// 返回移动之后条目所在的**完整远端路径**。
    ///
    /// ⚠️ [`RemoteProvider`] 这个 trait 里**没有** move 方法（加它会牵连
    /// 百度/夸克/SFTP/FTP/FTPS/假实现六个 provider）⇒ 这里只作为
    /// `AliyunRemoteProvider` 的**公开方法**提供，命令层尚未接线。
    pub async fn move_path(
        &self,
        connection_id: &str,
        from_path: &str,
        to_path: &str,
    ) -> Result<String, RemoteError> {
        let (key, mut session, from_normalized) = self.prepare(connection_id, from_path).await?;

        // 目标只当**目录**用：名字由源条目决定，移动不该顺手改名字。
        reject_path_control_characters(to_path)?;
        let Some(to_normalized) = normalize_remote_path(to_path) else {
            return Err(RemoteError::protocol(
                "目标路径含有不合法的段，请检查后重试。",
            ));
        };

        let (from_parent, name) = split_remote_parent_and_name(&from_normalized)?;
        validate_remote_entry_name(&name)?;

        let work = async {
            let drive_id = self.resolve_drive_id(&session)?;
            let from_parent_file_id = self
                .resolve_dir_file_id(&key, &mut session, &drive_id, &from_parent)
                .await?;
            let to_parent_file_id = self
                .resolve_dir_file_id(&key, &mut session, &drive_id, &to_normalized)
                .await?;

            let Some(entry) = self
                .find_entry(&key, &mut session, &drive_id, &from_parent_file_id, &name)
                .await?
            else {
                return Err(RemoteError::not_found(format!(
                    "阿里云盘里没有这个条目：{name}"
                )));
            };

            if entry.file_id.trim().is_empty() {
                return Err(RemoteError::unsupported(
                    "阿里云盘没有给出这个条目的服务端 id，无法移动。",
                ));
            }

            self.api
                .move_entry(&session, &drive_id, &entry.file_id, &to_parent_file_id)
                .await?;

            Ok::<String, RemoteError>(join_remote_parent_and_name(&to_normalized, &name))
        };

        with_timeout(self.directory_timeout, "移动阿里云盘条目", work).await
    }
}

#[async_trait]
impl RemoteProvider for AliyunRemoteProvider {
    async fn read_dir(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> Result<Vec<RemoteEntry>, RemoteError> {
        let (key, mut session, normalized) = self.prepare(connection_id, remote_path).await?;

        let work = async {
            let drive_id = self.resolve_drive_id(&session)?;
            let parent_file_id = self
                .resolve_dir_file_id(&key, &mut session, &drive_id, &normalized)
                .await?;
            let raw = self
                .list_raw(&key, &mut session, &drive_id, &parent_file_id)
                .await?;

            Ok::<Vec<RemoteEntry>, RemoteError>(aliyun_to_remote_entries(
                &raw,
                connection_id,
                &normalized,
            ))
        };

        with_timeout(self.directory_timeout, "读取阿里云盘目录", work).await
    }

    async fn read_file(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> Result<Vec<u8>, RemoteError> {
        let (key, mut session, normalized) = self.prepare(connection_id, remote_path).await?;
        let (parent, name) = split_remote_parent_and_name(&normalized)?;

        let work = async {
            let drive_id = self.resolve_drive_id(&session)?;
            let parent_file_id = self
                .resolve_dir_file_id(&key, &mut session, &drive_id, &parent)
                .await?;

            let Some(entry) = self
                .find_entry(&key, &mut session, &drive_id, &parent_file_id, &name)
                .await?
            else {
                return Err(RemoteError::not_found(format!(
                    "阿里云盘里没有这个文件：{name}"
                )));
            };

            if super::aliyun_api::aliyun_entry_is_directory(&entry) {
                return Err(RemoteError::protocol(format!(
                    "「{name}」是一个目录，不能当文件读取。"
                )));
            }

            if entry.file_id.trim().is_empty() {
                return Err(RemoteError::unsupported(
                    "阿里云盘没有给出这个文件的服务端 id，无法读取。",
                ));
            }

            // 元数据以 `openFile/get` 为准（裁定第 8 条）：列表条目只当索引用。
            // 服务端没给 id 时退回列表里那个 —— 宁可少一个字段，也不能读不了。
            let meta = self
                .api_file_info(&key, &mut session, &drive_id, &entry.file_id)
                .await?;
            let file_id = if meta.file_id.trim().is_empty() {
                entry.file_id.clone()
            } else {
                meta.file_id.clone()
            };

            // 直链拿不到 ⇒ 这条路径在本阶段**读不了**。
            // 但「会话失效」不能被改写成「不支持」：那两件事的用户处置完全不同。
            let url = self
                .api_download_url(&key, &mut session, &drive_id, &file_id)
                .await?;

            self.api
                .download(&session, &url, ALIYUN_MAX_DOWNLOAD_BYTES)
                .await
        };

        with_timeout(self.file_timeout, "读取阿里云盘文件", work).await
    }

    // 走的是 `read_file` 同一条解析链（`prepare` → 定位目录 → 找条目 → 取详情 →
    // 下载），三处差别都是刻意的：
    //   1. 条目不存在 / 是目录 / `file_id` 为空 ⇒ `Ok(None)`，不像 `read_file`
    //      那样冒 `Err`——「这张图没有缩略图」是合法结果，不该污染错误日志。
    //   2. 候选直链按「越小越省」排：列表接口已经带回的 256px 缩略图 →
    //      详情接口的缩略图 → 原图直链。前两条取不到才退原图，全取不到就 `None`。
    //   3. 下载上限用调用方给的 `max_bytes`，不是 `ALIYUN_MAX_DOWNLOAD_BYTES`
    //      （512 MB）——缩略图不该把整部电影拉进内存。
    // 签名 URL 只在本函数里存在，失败只记一句不含 URL 的 debug 日志。
    async fn thumbnail_bytes(
        &self,
        connection_id: &str,
        remote_path: &str,
        max_bytes: u64,
    ) -> Result<Option<Vec<u8>>, RemoteError> {
        let (key, mut session, normalized) = self.prepare(connection_id, remote_path).await?;
        let (parent, name) = split_remote_parent_and_name(&normalized)?;

        let work = async {
            let drive_id = self.resolve_drive_id(&session)?;
            let parent_file_id = self
                .resolve_dir_file_id(&key, &mut session, &drive_id, &parent)
                .await?;

            let Some(entry) = self
                .find_entry(&key, &mut session, &drive_id, &parent_file_id, &name)
                .await?
            else {
                return Ok::<Option<Vec<u8>>, RemoteError>(None);
            };

            if super::aliyun_api::aliyun_entry_is_directory(&entry) {
                return Ok::<Option<Vec<u8>>, RemoteError>(None);
            }
            if entry.file_id.trim().is_empty() {
                return Ok::<Option<Vec<u8>>, RemoteError>(None);
            }

            // 候选直链，按「越小越省」排。列表里那份是服务端已经缩好的 256px jpeg，
            // 最省流量；原图直链只在前两条都不成时才当兜底用。
            let mut candidates: Vec<String> = Vec::with_capacity(3);
            if let Some(thumbnail) = entry.thumbnail.as_deref() {
                if !thumbnail.trim().is_empty() {
                    candidates.push(thumbnail.to_string());
                }
            }

            // 详情接口失败（条目刚被删、权限变了）就当没有缩略图，降级而不冒错。
            match self
                .api_file_info(&key, &mut session, &drive_id, &entry.file_id)
                .await
            {
                Ok(meta) => {
                    if let Some(thumbnail) = meta.thumbnail.as_deref() {
                        if !thumbnail.trim().is_empty() {
                            candidates.push(thumbnail.to_string());
                        }
                    }
                    let file_id = if meta.file_id.trim().is_empty() {
                        entry.file_id.clone()
                    } else {
                        meta.file_id.clone()
                    };
                    if let Ok(url) = self
                        .api_download_url(&key, &mut session, &drive_id, &file_id)
                        .await
                    {
                        candidates.push(url);
                    }
                }
                Err(error) => {
                    log::debug!("[remote-diag] aliyun thumbnail stat unavailable: {error}");
                }
            }

            for url in candidates {
                // 缩略图直链是短期签名，过期与被限流都是常态：一条不通就试下一条。
                match self.api.download(&session, &url, max_bytes).await {
                    Ok(bytes) if !bytes.is_empty() => {
                        return Ok::<Option<Vec<u8>>, RemoteError>(Some(bytes));
                    }
                    Ok(_) => {}
                    Err(error) => {
                        log::debug!("[remote-diag] aliyun thumbnail candidate rejected: {error}");
                    }
                }
            }
            Ok::<Option<Vec<u8>>, RemoteError>(None)
        };

        with_timeout(self.file_timeout, "取阿里云盘缩略图", work).await
    }

    async fn write_file(
        &self,
        connection_id: &str,
        remote_path: &str,
        content: Vec<u8>,
    ) -> Result<(), RemoteError> {
        self.write_file_with_progress(connection_id, remote_path, content, &|_| {})
            .await
    }

    async fn write_file_with_progress(
        &self,
        connection_id: &str,
        remote_path: &str,
        content: Vec<u8>,
        on_progress: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<(), RemoteError> {
        let (key, mut session, normalized) = self.prepare(connection_id, remote_path).await?;

        // 名字要单独校验：名字里带 `/` 会让一次「上传」变成「传到别处去」。
        let (parent, name) = split_remote_parent_and_name(&normalized)?;
        validate_remote_entry_name(&name)?;

        let work = async {
            let drive_id = self.resolve_drive_id(&session)?;
            let parent_file_id = self
                .resolve_dir_file_id(&key, &mut session, &drive_id, &parent)
                .await?;

            // 第一阶段：建条目，拿上传直链。
            let created = self
                .api
                .create_file(&session, &drive_id, &parent_file_id, &name)
                .await?;

            let total_len = content.len() as u64;
            let ranges = aliyun_part_ranges(total_len, &created.parts)?;

            // 第二阶段：把内容 `PUT` 到直链上（进度如实上报）。
            if total_len > 0 {
                self.api
                    .upload_parts(&session, &ranges, Arc::new(content), on_progress)
                    .await?;
            }

            // 第三阶段：收尾。缺 `upload_id` / `revision_id` 就没法收尾。
            let (Some(upload_id), Some(revision_id)) = (
                created.upload_id.as_deref(),
                created.revision_id.as_deref(),
            ) else {
                return Err(RemoteError::protocol(
                    "阿里云盘没有给出上传会话信息，无法完成这次上传。",
                ));
            };

            self.api
                .complete_upload(
                    &session,
                    &drive_id,
                    &created.file_id,
                    upload_id,
                    revision_id,
                )
                .await?;

            Ok(())
        };

        with_timeout(self.file_timeout, "上传文件到阿里云盘", work).await
    }

    async fn make_dir(&self, connection_id: &str, remote_path: &str) -> Result<(), RemoteError> {
        let (key, mut session, normalized) = self.prepare(connection_id, remote_path).await?;

        // 名字要单独校验：名字里带 `/` 会让一次「新建目录」变成「建到别处去」。
        let (parent, name) = split_remote_parent_and_name(&normalized)?;
        validate_remote_entry_name(&name)?;

        let work = async {
            let drive_id = self.resolve_drive_id(&session)?;
            let parent_file_id = self
                .resolve_dir_file_id(&key, &mut session, &drive_id, &parent)
                .await?;

            // 返回 `true` = 目录**已经存在** —— 按成功处理（`make_dir` 的契约）。
            let _already_existed = self
                .api
                .create_folder(&session, &drive_id, &parent_file_id, &name)
                .await?;

            Ok(())
        };

        with_timeout(self.directory_timeout, "在阿里云盘新建目录", work).await
    }

    async fn set_permissions(
        &self,
        connection_id: &str,
        remote_path: &str,
        _mode: u32,
    ) -> Result<(), RemoteError> {
        // 阿里云盘**没有 POSIX 权限位**这一套能力（没有 owner/group/other
        // 的 rwx 三元组），服务端也没有对应的接口 ⇒ 无从设置。
        // 这里**不联网** —— 走一遍网络只为回答「不支持」是白花的代价。
        self.prepare_path(connection_id, remote_path)?;

        Err(RemoteError::unsupported(
            "阿里云盘没有 POSIX 权限位，改权限暂不支持。",
        ))
    }

    async fn get_permissions(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> Result<Option<u32>, RemoteError> {
        // 同 [`Self::set_permissions`]：阿里云盘没有权限位可查。
        self.prepare_path(connection_id, remote_path)?;

        Err(RemoteError::unsupported(
            "阿里云盘没有 POSIX 权限位，查询权限暂不支持。",
        ))
    }

    async fn remove_path(
        &self,
        connection_id: &str,
        remote_path: &str,
        is_dir: bool,
    ) -> Result<(), RemoteError> {
        let (key, mut session, normalized) = self.prepare(connection_id, remote_path).await?;

        // 根目录没有条目名，删它就是删整个网盘 —— 这条要在开网络之前挡掉。
        if normalized == "/" {
            return Err(RemoteError::protocol("不能删除阿里云盘的根目录。"));
        }

        let (parent, name) = split_remote_parent_and_name(&normalized)?;

        let work = async {
            let drive_id = self.resolve_drive_id(&session)?;
            let parent_file_id = self
                .resolve_dir_file_id(&key, &mut session, &drive_id, &parent)
                .await?;

            let Some(entry) = self
                .find_entry(&key, &mut session, &drive_id, &parent_file_id, &name)
                .await?
            else {
                return Err(RemoteError::not_found(format!(
                    "阿里云盘里没有这个条目：{name}"
                )));
            };

            if entry.file_id.trim().is_empty() {
                return Err(RemoteError::unsupported(
                    "阿里云盘没有给出这个条目的服务端 id，无法删除。",
                ));
            }

            // `type` 必填 —— 服务端要知道删的是文件还是目录。
            // 优先用服务端给的**真实类型**；它缺失时才用调用方传下来的判断。
            let entry_type = if entry.entry_type.trim().is_empty() {
                aliyun_entry_type_name(is_dir)
            } else {
                entry.entry_type.as_str()
            };

            self.api
                .delete_entry(&session, &drive_id, &entry.file_id, entry_type)
                .await?;

            Ok(())
        };

        with_timeout(self.directory_timeout, "删除阿里云盘条目", work).await
    }

    async fn rename_path(
        &self,
        connection_id: &str,
        from_path: &str,
        new_name: &str,
    ) -> Result<(), RemoteError> {
        // 新名字先校验：**改名只能是改名，不能顺手变成移动**。
        validate_remote_entry_name(new_name)?;

        let (key, mut session, normalized) = self.prepare(connection_id, from_path).await?;
        let (parent, name) = split_remote_parent_and_name(&normalized)?;

        let work = async {
            let drive_id = self.resolve_drive_id(&session)?;
            let parent_file_id = self
                .resolve_dir_file_id(&key, &mut session, &drive_id, &parent)
                .await?;

            let Some(entry) = self
                .find_entry(&key, &mut session, &drive_id, &parent_file_id, &name)
                .await?
            else {
                return Err(RemoteError::not_found(format!(
                    "阿里云盘里没有这个条目：{name}"
                )));
            };

            if entry.file_id.trim().is_empty() {
                return Err(RemoteError::unsupported(
                    "阿里云盘没有给出这个条目的服务端 id，无法重命名。",
                ));
            }

            self.api
                .rename_entry(&session, &drive_id, &entry.file_id, new_name)
                .await?;

            Ok(())
        };

        with_timeout(self.directory_timeout, "重命名阿里云盘条目", work).await
    }
}

// ===========================================================================
// 测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::connection::RemoteProtocol;
    use crate::remote::registry::{ConnectionsFile, REMOTE_CONNECTIONS_FILE};
    use crate::remote::transport::ProxyConfig;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    const CONNECTION_ID: &str = "conn-aliyun";
    const CREDENTIAL_KEY: &str = "conn-aliyun";
    const FRESH_TOKEN: &str = "fresh-access-token-value";
    const STALE_TOKEN: &str = "stale-access-token-value";

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    // ------------------------------------------------------------------
    // 假实现
    // ------------------------------------------------------------------

    /// 一页列表：`parent_file_id` + `marker` ⇒ 条目 + 下一页游标。
    type Page = (String, Option<String>, Vec<AliyunFileEntry>, Option<String>);

    #[derive(Default)]
    struct FakeApi {
        pages: Mutex<Vec<Page>>,
        list_error: Mutex<Option<RemoteError>>,
        /// 用这个 token 发请求就报「授权失效」—— 用来验「先刷新再重试」。
        reject_token: Mutex<Option<String>>,
        refresh_result: Mutex<Option<Result<AliyunAuthSession, RemoteError>>>,
        refresh_calls: AtomicUsize,
        links: Mutex<HashMap<String, String>>,
        downloads: Mutex<HashMap<String, Vec<u8>>>,
        list_drive_ids: Mutex<Vec<String>>,
        /// 记住被请求过的 (父目录, 游标)，便于断言「按游标续拉了」。
        list_calls: Mutex<Vec<(String, Option<String>)>>,
        /// 写操作记录：新建目录 (drive_id, parent_file_id, name)。
        created_folders: Mutex<Vec<(String, String, String)>>,
        /// 写操作记录：新建文件 (drive_id, parent_file_id, name)。
        created_files: Mutex<Vec<(String, String, String)>>,
        /// 写操作记录：上传的分片 (part_number, 字节数)。
        uploaded: Mutex<Vec<(u32, u64)>>,
        /// 写操作记录：上传收尾 (file_id, upload_id, revision_id)。
        completed: Mutex<Vec<(String, String, String)>>,
        /// 写操作记录：重命名 (file_id, 新名字)。
        renamed: Mutex<Vec<(String, String)>>,
        /// 写操作记录：删除 (file_id, type)。
        deleted: Mutex<Vec<(String, String)>>,
        /// 写操作记录：移动 (file_id, 目标父目录 id)。
        moved: Mutex<Vec<(String, String)>>,
        /// 让写操作失败（验「失败即停，不重放」）。
        write_error: Mutex<Option<RemoteError>>,
        /// 只让**上传**失败（前面几步照常走完）。
        upload_error: Mutex<Option<RemoteError>>,
        /// `create_folder` 返回「这个目录已经存在」。
        folder_already_exists: Mutex<bool>,
    }

    impl FakeApi {
        fn set_page(
            &self,
            parent_file_id: &str,
            marker: Option<&str>,
            entries: Vec<AliyunFileEntry>,
            next_marker: Option<&str>,
        ) {
            self.pages.lock().expect("pages lock").push((
                parent_file_id.to_string(),
                marker.map(str::to_string),
                entries,
                next_marker.map(str::to_string),
            ));
        }

        fn set_refresh(&self, result: Result<AliyunAuthSession, RemoteError>) {
            *self.refresh_result.lock().expect("refresh lock") = Some(result);
        }

        fn refresh_calls(&self) -> usize {
            self.refresh_calls.load(Ordering::SeqCst)
        }

        fn list_calls(&self) -> Vec<(String, Option<String>)> {
            self.list_calls.lock().expect("list calls lock").clone()
        }

        /// 列目录时用过的 `drive_id`（按顺序）。用来断言「主盘取自登录时存下的值」。
        fn list_drive_ids(&self) -> Vec<String> {
            self.list_drive_ids.lock().expect("list drive ids lock").clone()
        }

        /// 新建目录的调用记录。
        fn created_folders(&self) -> Vec<(String, String, String)> {
            self.created_folders
                .lock()
                .expect("created folders lock")
                .clone()
        }

        /// 新建文件的调用记录。
        fn created_files(&self) -> Vec<(String, String, String)> {
            self.created_files.lock().expect("created files lock").clone()
        }

        /// 上传分片的调用记录。
        fn uploaded(&self) -> Vec<(u32, u64)> {
            self.uploaded.lock().expect("uploaded lock").clone()
        }

        /// 上传收尾的调用记录。
        fn completed(&self) -> Vec<(String, String, String)> {
            self.completed.lock().expect("completed lock").clone()
        }

        /// 重命名的调用记录。
        fn renamed(&self) -> Vec<(String, String)> {
            self.renamed.lock().expect("renamed lock").clone()
        }

        /// 删除的调用记录。
        fn deleted(&self) -> Vec<(String, String)> {
            self.deleted.lock().expect("deleted lock").clone()
        }

        /// 移动的调用记录。
        fn moved(&self) -> Vec<(String, String)> {
            self.moved.lock().expect("moved lock").clone()
        }
    }

    #[async_trait]
    impl AliyunApi for FakeApi {
        async fn list_page(
            &self,
            session: &AliyunAuthSession,
            drive_id: &str,
            parent_file_id: &str,
            marker: Option<&str>,
        ) -> Result<AliyunListPayload, RemoteError> {
            self.list_calls
                .lock()
                .expect("list calls lock")
                .push((parent_file_id.to_string(), marker.map(str::to_string)));
            self.list_drive_ids
                .lock()
                .expect("list drive ids lock")
                .push(drive_id.to_string());

            if let Some(error) = self.list_error.lock().expect("list error lock").clone() {
                return Err(error);
            }

            if let Some(rejected) = self.reject_token.lock().expect("reject token lock").clone() {
                if session.access_token == rejected {
                    return Err(RemoteError::auth("访问凭证已失效。"));
                }
            }

            let pages = self.pages.lock().expect("pages lock");
            for (page_parent, page_marker, entries, next_marker) in pages.iter() {
                // **不**按 `drive_id` 过滤：批次 28 起 `drive_id` 有多个合法来源
                // （环境变量 / 会话），具体传了哪个由 `list_drive_ids()` 断言。
                if page_parent == parent_file_id && page_marker.as_deref() == marker {
                    return Ok(AliyunListPayload {
                        items: entries.clone(),
                        next_marker: next_marker.clone(),
                    });
                }
            }

            Ok(AliyunListPayload::default())
        }

        async fn file_info(
            &self,
            _session: &AliyunAuthSession,
            _drive_id: &str,
            file_id: &str,
        ) -> Result<AliyunFileEntry, RemoteError> {
            Ok(AliyunFileEntry {
                file_id: file_id.to_string(),
                ..AliyunFileEntry::default()
            })
        }

        async fn download_url(
            &self,
            _session: &AliyunAuthSession,
            _drive_id: &str,
            file_id: &str,
        ) -> Result<String, RemoteError> {
            self.links
                .lock()
                .expect("links lock")
                .get(file_id)
                .cloned()
                .ok_or_else(|| RemoteError::unsupported("这个文件没有下载直链。"))
        }

        async fn download(
            &self,
            _session: &AliyunAuthSession,
            url: &str,
            max_bytes: u64,
        ) -> Result<Vec<u8>, RemoteError> {
            let bytes = self
                .downloads
                .lock()
                .expect("downloads lock")
                .get(url)
                .cloned()
                .ok_or_else(|| RemoteError::not_found("没有这个直链。"))?;

            if bytes.len() as u64 > max_bytes {
                return Err(RemoteError::unsupported("文件太大了。"));
            }

            Ok(bytes)
        }


        async fn create_folder(
            &self,
            _session: &AliyunAuthSession,
            drive_id: &str,
            parent_file_id: &str,
            name: &str,
        ) -> Result<bool, RemoteError> {
            if let Some(error) = self.write_error.lock().expect("write error lock").clone() {
                return Err(error);
            }

            self.created_folders
                .lock()
                .expect("created folders lock")
                .push((
                    drive_id.to_string(),
                    parent_file_id.to_string(),
                    name.to_string(),
                ));

            Ok(*self
                .folder_already_exists
                .lock()
                .expect("folder already exists lock"))
        }

        async fn create_file(
            &self,
            _session: &AliyunAuthSession,
            drive_id: &str,
            parent_file_id: &str,
            name: &str,
        ) -> Result<AliyunCreateResult, RemoteError> {
            if let Some(error) = self.write_error.lock().expect("write error lock").clone() {
                return Err(error);
            }

            self.created_files
                .lock()
                .expect("created files lock")
                .push((
                    drive_id.to_string(),
                    parent_file_id.to_string(),
                    name.to_string(),
                ));

            Ok(AliyunCreateResult {
                file_id: "f-new".to_string(),
                upload_id: Some("upload-1".to_string()),
                revision_id: Some("revision-1".to_string()),
                parts: vec![AliyunUploadPart {
                    part_number: 1,
                    upload_url: "https://upload.invalid/part-1".to_string(),
                    // 单片路径不需要 `size`（见 `aliyun_part_ranges`）。
                    size: None,
                }],
            })
        }

        async fn upload_parts(
            &self,
            _session: &AliyunAuthSession,
            ranges: &[AliyunPartRange],
            content: Arc<Vec<u8>>,
            on_progress: &(dyn Fn(u64) + Send + Sync),
        ) -> Result<(), RemoteError> {
            if let Some(error) = self.upload_error.lock().expect("upload error lock").clone() {
                return Err(error);
            }

            let mut written = 0u64;
            for range in ranges {
                let start = range.start as usize;
                let end = range.end as usize;
                let chunk = content[start..end].to_vec();
                written += chunk.len() as u64;

                self.uploaded
                    .lock()
                    .expect("uploaded lock")
                    .push((range.part_number, chunk.len() as u64));

                on_progress(written);
            }

            Ok(())
        }

        async fn complete_upload(
            &self,
            _session: &AliyunAuthSession,
            _drive_id: &str,
            file_id: &str,
            upload_id: &str,
            revision_id: &str,
        ) -> Result<(), RemoteError> {
            self.completed
                .lock()
                .expect("completed lock")
                .push((
                    file_id.to_string(),
                    upload_id.to_string(),
                    revision_id.to_string(),
                ));
            Ok(())
        }

        async fn rename_entry(
            &self,
            _session: &AliyunAuthSession,
            _drive_id: &str,
            file_id: &str,
            name: &str,
        ) -> Result<(), RemoteError> {
            self.renamed
                .lock()
                .expect("renamed lock")
                .push((file_id.to_string(), name.to_string()));
            Ok(())
        }

        async fn delete_entry(
            &self,
            _session: &AliyunAuthSession,
            _drive_id: &str,
            file_id: &str,
            entry_type: &str,
        ) -> Result<(), RemoteError> {
            self.deleted
                .lock()
                .expect("deleted lock")
                .push((file_id.to_string(), entry_type.to_string()));
            Ok(())
        }

        async fn move_entry(
            &self,
            _session: &AliyunAuthSession,
            _drive_id: &str,
            file_id: &str,
            to_parent_file_id: &str,
        ) -> Result<(), RemoteError> {
            self.moved
                .lock()
                .expect("moved lock")
                .push((file_id.to_string(), to_parent_file_id.to_string()));
            Ok(())
        }

        async fn refresh_session(
            &self,
            _session: &AliyunAuthSession,
        ) -> Result<AliyunAuthSession, RemoteError> {
            self.refresh_calls.fetch_add(1, Ordering::SeqCst);

            match self.refresh_result.lock().expect("refresh lock").clone() {
                Some(result) => result,
                None => Ok(session_with(FRESH_TOKEN, Some("new-refresh-token"))),
            }
        }
    }

    #[derive(Default)]
    struct FakeSessionStore {
        sessions: Mutex<HashMap<String, AliyunAuthSession>>,
        error: Mutex<Option<RemoteError>>,
        saved: Mutex<Vec<String>>,
    }

    impl FakeSessionStore {
        fn insert(&self, key: &str, session: AliyunAuthSession) {
            self.sessions
                .lock()
                .expect("sessions lock")
                .insert(key.to_string(), session);
        }

        fn saved_keys(&self) -> Vec<String> {
            self.saved.lock().expect("saved lock").clone()
        }
    }

    impl AliyunSessionStore for FakeSessionStore {
        fn load(&self, key: &str) -> Result<Option<AliyunAuthSession>, RemoteError> {
            if let Some(error) = self.error.lock().expect("session error lock").clone() {
                return Err(error);
            }

            Ok(self
                .sessions
                .lock()
                .expect("sessions lock")
                .get(key)
                .cloned())
        }

        fn save(&self, key: &str, session: &AliyunAuthSession) -> Result<(), RemoteError> {
            self.saved.lock().expect("saved lock").push(key.to_string());
            self.insert(key, session.clone());
            Ok(())
        }
    }

    // ------------------------------------------------------------------
    // 夹具
    // ------------------------------------------------------------------

    struct Fixture {
        dir: PathBuf,
        api: Arc<FakeApi>,
        sessions: Arc<FakeSessionStore>,
    }

    fn session_with(access_token: &str, refresh_token: Option<&str>) -> AliyunAuthSession {
        AliyunAuthSession {
            access_token: access_token.to_string(),
            refresh_token: refresh_token.map(str::to_string),
            expires_at_unix: Some(aliyun_now_unix() + 3_600),
            refresh_expires_at_unix: Some(aliyun_now_unix() + 86_400),
            drive_id: Some("drive-1".to_string()),
            client_id: "client-abc".to_string(),
            redirect_uri: Some(crate::remote::ALIYUN_DEFAULT_REDIRECT_URI.to_string()),
            scope: Some("user:base file:all:read".to_string()),
            token_type: "Bearer".to_string(),
            nick_name: Some("测试用户".to_string()),
            device_id: None,
        }
    }

    fn expired_session() -> AliyunAuthSession {
        let mut session = session_with(STALE_TOKEN, Some("old-refresh-token"));
        session.expires_at_unix = Some(aliyun_now_unix() - 10);
        session
    }

    #[test]
    fn refresh_client_id_falls_back_to_web_client_id() {
        let web_id = crate::remote::ALIYUN_WEB_CLIENT_ID;
        let resolve = crate::remote::aliyun_provider::aliyun_refresh_client_id;

        // 批次 29 的旧会话 `/ 粘贴 token` 会话：空串必须回退，否则续期 400。
        assert_eq!(resolve(""), web_id);
        assert_eq!(resolve("   "), web_id);
        // 存了值就原样用（不覆写用户/未来版本的取值）。
        assert_eq!(resolve("client-abc"), "client-abc");
        assert_eq!(resolve(" client-abc "), "client-abc");
    }

    fn dir_entry(file_id: &str, name: &str) -> AliyunFileEntry {
        AliyunFileEntry {
            file_id: file_id.to_string(),
            name: name.to_string(),
            entry_type: "folder".to_string(),
            size: Some(0),
            updated_at: Some("2024-01-02T03:04:05Z".to_string()),
            ..AliyunFileEntry::default()
        }
    }

    fn plain_file(file_id: &str, name: &str, size: u64) -> AliyunFileEntry {
        AliyunFileEntry {
            file_id: file_id.to_string(),
            name: name.to_string(),
            entry_type: "file".to_string(),
            size: Some(size),
            updated_at: Some("2024-01-02T03:04:05Z".to_string()),
            ..AliyunFileEntry::default()
        }
    }

    fn fixture_with_credential(tag: &str, credential_key: Option<&str>) -> Fixture {
        let dir = std::env::temp_dir().join(format!(
            "afm-aliyun-{tag}-{}",
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");

        // 连接表里这条记录的协议就是 `aliyun_pan`：provider 按协议分派，
        // 协议写错的话，命令层会把它分派给别的 provider。
        let connections = ConnectionsFile {
            connections: vec![RemoteConnection {
                id: CONNECTION_ID.to_string(),
                name: "阿里云盘".to_string(),
                protocol: RemoteProtocol::AliyunPan,
                host: "openapi.alipan.com".to_string(),
                port: 443,
                username: "测试用户".to_string(),
                initial_path: "/".to_string(),
                local_path: None,
                proxy: ProxyConfig::Direct,
                credential_key: credential_key.map(str::to_string),
            }],
        };
        std::fs::write(
            dir.join(REMOTE_CONNECTIONS_FILE),
            serde_json::to_string_pretty(&connections).expect("serialize connections"),
        )
        .expect("write connections file");

        let api = Arc::new(FakeApi::default());
        let sessions = Arc::new(FakeSessionStore::default());

        Fixture { dir, api, sessions }
    }

    /// 连接表里有 `credential_key`，**存档里也有**一份可用的会话。
    fn fixture(tag: &str) -> Fixture {
        let fixture = fixture_with_credential(tag, Some(CREDENTIAL_KEY));
        fixture.sessions.insert(
            CREDENTIAL_KEY,
            session_with(FRESH_TOKEN, Some("refresh-token")),
        );
        fixture
    }

    fn provider(fixture: &Fixture) -> AliyunRemoteProvider {
        AliyunRemoteProvider::with_parts(
            &fixture.dir,
            fixture.api.clone(),
            fixture.sessions.clone(),
        )
    }

    // ------------------------------------------------------------------
    // 写类操作：真的写（契约 §〇 / §6.1 锚点 12）
    // ------------------------------------------------------------------

    /// 在根目录里放一个 `a.txt`，供删除/重命名/移动这些用例用。
    fn seed_root_file(fixture: &Fixture) {
        fixture.api.set_page(
            ALIYUN_ROOT_FILE_ID,
            None,
            vec![plain_file("f-1", "a.txt", 5)],
            None,
        );
    }

    #[tokio::test]
    async fn every_write_operation_now_reaches_the_server() {
        let fixture = fixture("write-ops");
        seed_root_file(&fixture);
        let provider = provider(&fixture);

        provider
            .write_file(CONNECTION_ID, "/a.txt", b"hello".to_vec())
            .await
            .expect("上传必须成功");
        provider
            .make_dir(CONNECTION_ID, "/新建")
            .await
            .expect("新建目录必须成功");
        provider
            .remove_path(CONNECTION_ID, "/a.txt", false)
            .await
            .expect("删除必须成功");
        provider
            .rename_path(CONNECTION_ID, "/a.txt", "b.txt")
            .await
            .expect("重命名必须成功");

        let files = fixture.api.created_files();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].1, ALIYUN_ROOT_FILE_ID, "文件要建在父目录里");
        assert_eq!(files[0].2, "a.txt");

        let folders = fixture.api.created_folders();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].1, ALIYUN_ROOT_FILE_ID, "目录要建在父目录里");
        assert_eq!(folders[0].2, "新建");

        assert_eq!(fixture.api.uploaded(), vec![(1u32, 5u64)], "5 个字节一片");
        assert_eq!(fixture.api.completed().len(), 1, "上传完必须收尾");
        assert_eq!(
            fixture.api.deleted(),
            vec![("f-1".to_string(), "file".to_string())]
        );
        assert_eq!(
            fixture.api.renamed(),
            vec![("f-1".to_string(), "b.txt".to_string())]
        );
    }

    #[tokio::test]
    async fn the_permission_operations_stay_unsupported() {
        let fixture = fixture("permissions");
        let provider = provider(&fixture);

        // 阿里云盘没有 POSIX 权限位 ⇒ 这两个操作仍然是「不支持」。
        assert_eq!(
            provider
                .set_permissions(CONNECTION_ID, "/a.txt", 0o644)
                .await
                .expect_err("改权限必须失败")
                .kind,
            RemoteErrorKind::Unsupported
        );
        assert_eq!(
            provider
                .get_permissions(CONNECTION_ID, "/a.txt")
                .await
                .expect_err("查权限必须失败")
                .kind,
            RemoteErrorKind::Unsupported
        );
    }

    #[tokio::test]
    async fn the_root_directory_cannot_be_deleted() {
        let fixture = fixture("root-delete");
        let provider = provider(&fixture);

        let error = provider
            .remove_path(CONNECTION_ID, "/", false)
            .await
            .expect_err("根目录不能删");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
        assert!(
            fixture.api.deleted().is_empty(),
            "一个删除请求都不该发出去"
        );
    }

    #[tokio::test]
    async fn the_delete_carries_the_real_entry_type() {
        let fixture = fixture("delete-type");
        fixture.api.set_page(
            ALIYUN_ROOT_FILE_ID,
            None,
            vec![dir_entry("d-1", "资料")],
            None,
        );
        let provider = provider(&fixture);

        // 调用方说它是文件（`false`），服务端说它是目录 ⇒ **以服务端为准**：
        // 服务端删错类型会把目录当文件删掉。
        provider
            .remove_path(CONNECTION_ID, "/资料", false)
            .await
            .expect("删除目录必须成功");

        assert_eq!(
            fixture.api.deleted(),
            vec![("d-1".to_string(), "folder".to_string())]
        );
    }

    #[tokio::test]
    async fn the_rename_uses_the_entry_id() {
        let fixture = fixture("rename-id");
        seed_root_file(&fixture);
        let provider = provider(&fixture);

        provider
            .rename_path(CONNECTION_ID, "/a.txt", "b.txt")
            .await
            .expect("重命名必须成功");

        assert_eq!(
            fixture.api.renamed(),
            vec![("f-1".to_string(), "b.txt".to_string())]
        );
    }

    #[tokio::test]
    async fn the_move_call_carries_the_target_parent_id() {
        let fixture = fixture("move");
        fixture.api.set_page(
            ALIYUN_ROOT_FILE_ID,
            None,
            vec![plain_file("f-1", "a.txt", 5), dir_entry("d-1", "资料")],
            None,
        );
        let provider = provider(&fixture);

        let new_path = provider
            .move_path(CONNECTION_ID, "/a.txt", "/资料")
            .await
            .expect("移动必须成功");

        assert_eq!(new_path, "/资料/a.txt");
        assert_eq!(
            fixture.api.moved(),
            vec![("f-1".to_string(), "d-1".to_string())]
        );
    }

    #[tokio::test]
    async fn the_upload_reports_the_real_byte_count() {
        use std::sync::atomic::AtomicU64;

        let fixture = fixture("upload-progress");
        let provider = provider(&fixture);
        let counter = AtomicU64::new(0);

        provider
            .write_file_with_progress(CONNECTION_ID, "/a.txt", b"hello".to_vec(), &|value| {
                counter.fetch_add(value, Ordering::SeqCst);
            })
            .await
            .expect("上传必须成功");

        assert_eq!(counter.load(Ordering::SeqCst), 5, "进度必须是真实字节数");
    }

    #[tokio::test]
    async fn a_failed_upload_never_reports_success() {
        use std::sync::atomic::AtomicU64;

        let fixture = fixture("upload-failure");
        *fixture.api.upload_error.lock().expect("upload error lock") =
            Some(RemoteError::protocol("模拟上传失败。"));
        let provider = provider(&fixture);
        let counter = AtomicU64::new(0);

        let result = provider
            .write_file_with_progress(CONNECTION_ID, "/a.txt", b"hello".to_vec(), &|value| {
                counter.fetch_add(value, Ordering::SeqCst);
            })
            .await;

        assert!(result.is_err(), "上传失败必须报错，不能当成功");
        assert_eq!(counter.load(Ordering::SeqCst), 0, "失败的上传不谎报进度");
        assert!(
            fixture.api.completed().is_empty(),
            "上传失败就不能收尾"
        );
    }

    #[tokio::test]
    async fn the_upload_request_has_a_content_length_and_no_content_type() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        /// 读满一个 HTTP 请求（头部 + `Content-Length` 指定的正文）。
        async fn read_request(socket: &mut tokio::net::TcpStream) -> Vec<u8> {
            let mut buf: Vec<u8> = Vec::new();
            let mut tmp = [0u8; 1024];

            loop {
                let head_end = buf
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n".as_slice())
                    .map(|position| position + 4);

                if let Some(head_end) = head_end {
                    let head = String::from_utf8_lossy(&buf[..head_end]).to_ascii_lowercase();
                    let length = head
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length:")
                                .and_then(|value| value.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);

                    if buf.len() >= head_end + length {
                        return buf;
                    }
                }

                let read = socket.read(&mut tmp).await.expect("read request");
                if read == 0 {
                    return buf;
                }
                buf.extend_from_slice(&tmp[..read]);
            }
        }

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let request = read_request(&mut socket).await;
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await
                .expect("write response");
            request
        });

        let content = b"hello".to_vec();
        let api = HttpAliyunApi::new().expect("http api");
        let range = AliyunPartRange {
            part_number: 1,
            upload_url: format!("http://{addr}/part-1"),
            start: 0,
            end: content.len() as u64,
        };

        api.upload_parts(
            &session_with(FRESH_TOKEN, None),
            &[range],
            Arc::new(content.clone()),
            &|_| {},
        )
        .await
        .expect("上传必须成功");

        let request = server.await.expect("server task");
        let head_end = request
            .windows(4)
            .position(|window| window == b"\r\n\r\n".as_slice())
            .expect("请求头应当完整")
            + 4;
        let head = String::from_utf8_lossy(&request[..head_end]).to_ascii_lowercase();

        assert!(head.starts_with("put "), "上传必须是 PUT：{head}");
        assert!(
            head.contains("content-length: 5"),
            "必须带 Content-Length（chunked 会被 OSS 拒）：{head}"
        );
        assert!(
            !head.contains("content-type:"),
            "上传请求绝不能带 Content-Type（会破坏 OSS 签名）：{head}"
        );
        assert!(
            !head.contains("transfer-encoding: chunked"),
            "上传请求不能是 chunked：{head}"
        );
        assert!(
            !head.contains("authorization:"),
            "OSS 预签名直链自带授权，不能再加 Authorization：{head}"
        );
        assert_eq!(&request[head_end..], &content[..], "正文必须原样送达");
    }

    // ------------------------------------------------------------------
    // 读目录（契约 §6.1 锚点 13 / 15）
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn the_listing_entries_carry_the_full_remote_path() {
        let fixture = fixture("read-dir");
        fixture.api.set_page(
            ALIYUN_ROOT_FILE_ID,
            None,
            vec![plain_file("f-1", "说明.txt", 12)],
            None,
        );
        let provider = provider(&fixture);

        let entries = provider.read_dir(CONNECTION_ID, "/").await.expect("列目录");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "说明.txt");
        assert_eq!(entries[0].path, "sfm://remote/conn-aliyun/说明.txt");
        assert!(!entries[0].is_dir);
        assert_eq!(entries[0].size, 12);
    }

    #[tokio::test]
    async fn the_listing_follows_the_cursor_until_the_marker_runs_out() {
        let fixture = fixture("cursor");
        fixture.api.set_page(
            ALIYUN_ROOT_FILE_ID,
            None,
            vec![plain_file("f-1", "第一页.txt", 1)],
            Some("m-2"),
        );
        fixture.api.set_page(
            ALIYUN_ROOT_FILE_ID,
            Some("m-2"),
            vec![plain_file("f-2", "第二页.txt", 2)],
            None,
        );
        let provider = provider(&fixture);

        let entries = provider.read_dir(CONNECTION_ID, "/").await.expect("列目录");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "第一页.txt");
        assert_eq!(entries[1].name, "第二页.txt");

        let calls = fixture.api.list_calls();
        assert_eq!(calls.len(), 2, "应该按游标续拉一次");
        assert_eq!(calls[0].1, None);
        assert_eq!(calls[1].1.as_deref(), Some("m-2"));
    }

    #[tokio::test]
    async fn a_nested_directory_is_resolved_by_walking_down_from_the_root() {
        let fixture = fixture("nested");
        fixture.api.set_page(
            ALIYUN_ROOT_FILE_ID,
            None,
            vec![dir_entry("d-1", "资料")],
            None,
        );
        fixture
            .api
            .set_page("d-1", None, vec![plain_file("f-1", "笔记.txt", 3)], None);
        let provider = provider(&fixture);

        let entries = provider
            .read_dir(CONNECTION_ID, "/资料")
            .await
            .expect("列目录");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "sfm://remote/conn-aliyun/资料/笔记.txt");
    }

    #[tokio::test]
    async fn a_missing_connection_is_reported_as_not_found() {
        let fixture = fixture("no-connection");
        let provider = provider(&fixture);

        let error = provider
            .read_dir("conn-does-not-exist", "/")
            .await
            .expect_err("必须失败");

        assert_eq!(error.kind, RemoteErrorKind::NotFound);
        assert!(
            fixture.api.list_calls().is_empty(),
            "连接都不存在，不该发出任何请求"
        );
    }

    #[tokio::test]
    async fn a_path_with_control_characters_is_rejected_before_any_request() {
        let fixture = fixture("control-char");
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/pub\r\nQUIT")
            .await
            .expect_err("必须失败");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
        assert!(fixture.api.list_calls().is_empty(), "不该发出任何请求");
    }

    #[tokio::test]
    async fn a_connection_without_a_session_asks_the_user_to_authorize() {
        let fixture = fixture_with_credential("no-session", Some(CREDENTIAL_KEY));
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("必须失败");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[tokio::test]
    async fn a_connection_without_a_credential_key_asks_the_user_to_authorize() {
        let fixture = fixture_with_credential("no-key", None);
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("必须失败");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    // ------------------------------------------------------------------
    // 过期处理（契约 §6.1 锚点 14）
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn an_expired_session_is_refreshed_before_the_request_is_retried() {
        let fixture = fixture("expired");
        fixture.sessions.insert(CREDENTIAL_KEY, expired_session());
        fixture
            .api
            .set_refresh(Ok(session_with(FRESH_TOKEN, Some("new-refresh-token"))));
        fixture.api.set_page(
            ALIYUN_ROOT_FILE_ID,
            None,
            vec![plain_file("f-1", "说明.txt", 12)],
            None,
        );
        let provider = provider(&fixture);

        let entries = provider.read_dir(CONNECTION_ID, "/").await.expect("列目录");

        assert_eq!(entries.len(), 1);
        assert_eq!(fixture.api.refresh_calls(), 1, "过期后应该先刷新一次");
        assert_eq!(
            fixture.sessions.saved_keys(),
            vec![CREDENTIAL_KEY.to_string()],
            "刷新后的会话必须写回存档（刷新凭证单次有效）"
        );
    }

    #[tokio::test]
    async fn a_server_side_auth_failure_triggers_one_refresh_and_one_retry() {
        let fixture = fixture("auth-retry");
        fixture
            .api
            .set_refresh(Ok(session_with(FRESH_TOKEN, Some("new-refresh-token"))));
        // 服务端对旧凭证一律回「授权失效」。
        *fixture.api.reject_token.lock().expect("reject token lock") =
            Some(STALE_TOKEN.to_string());
        fixture.api.set_page(
            ALIYUN_ROOT_FILE_ID,
            None,
            vec![plain_file("f-1", "说明.txt", 12)],
            None,
        );

        // 存档里的会话用的是**旧**凭证，但没过期 ⇒ 第一条请求会吃到 Auth。
        fixture.sessions.insert(
            CREDENTIAL_KEY,
            session_with(STALE_TOKEN, Some("refresh-token")),
        );

        let provider = provider(&fixture);
        let entries = provider.read_dir(CONNECTION_ID, "/").await.expect("列目录");

        assert_eq!(entries.len(), 1);
        assert_eq!(fixture.api.refresh_calls(), 1, "只应刷新一次");
        assert_eq!(fixture.api.list_calls().len(), 2, "第一次失败后应重试一次");
    }

    #[tokio::test]
    async fn a_failed_refresh_asks_the_user_to_authorize_again() {
        let fixture = fixture("refresh-fails");
        fixture.sessions.insert(CREDENTIAL_KEY, expired_session());
        fixture
            .api
            .set_refresh(Err(RemoteError::auth("刷新凭证无效。")));
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("必须失败");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
        assert!(
            error.message.contains("重新授权"),
            "文案要告诉用户去重新授权：{}",
            error.message
        );
    }

    #[tokio::test]
    async fn an_expired_session_without_a_refresh_token_asks_the_user_to_authorize() {
        let fixture = fixture("expired-no-refresh");
        let mut session = expired_session();
        session.refresh_token = None;
        fixture.sessions.insert(CREDENTIAL_KEY, session);
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("必须失败");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
        assert_eq!(fixture.api.refresh_calls(), 0, "没有刷新凭证就不该尝试刷新");
    }

    // ------------------------------------------------------------------
    // 读文件
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn a_file_is_downloaded_through_its_direct_link() {
        let fixture = fixture("read-file");
        fixture.api.set_page(
            ALIYUN_ROOT_FILE_ID,
            None,
            vec![plain_file("f-1", "说明.txt", 5)],
            None,
        );
        fixture
            .api
            .links
            .lock()
            .expect("links lock")
            .insert("f-1".to_string(), "https://cdn.example/1".to_string());
        fixture
            .api
            .downloads
            .lock()
            .expect("downloads lock")
            .insert("https://cdn.example/1".to_string(), b"hello".to_vec());
        let provider = provider(&fixture);

        let bytes = provider
            .read_file(CONNECTION_ID, "/说明.txt")
            .await
            .expect("读文件");

        assert_eq!(bytes, b"hello".to_vec());
    }

    #[tokio::test]
    async fn a_directory_can_not_be_read_as_a_file() {
        let fixture = fixture("read-dir-as-file");
        fixture.api.set_page(
            ALIYUN_ROOT_FILE_ID,
            None,
            vec![dir_entry("d-1", "资料")],
            None,
        );
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/资料")
            .await
            .expect_err("必须失败");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
    }

    // ------------------------------------------------------------------
    // 空间 id
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn the_drive_id_saved_at_login_time_is_the_one_used_for_listing() {
        // 登录时从 `/v2/user/get` 取到的主盘 id 存进了会话 ⇒ 列目录就用它。
        let fixture = fixture("drive-id");
        let mut session = session_with(FRESH_TOKEN, Some("refresh-token"));
        session.drive_id = Some("drive-from-login".to_string());
        fixture.sessions.insert(CREDENTIAL_KEY, session);
        fixture.api.set_page(
            ALIYUN_ROOT_FILE_ID,
            None,
            vec![plain_file("f-1", "说明.txt", 12)],
            None,
        );
        let provider = provider(&fixture);

        let entries = provider.read_dir(CONNECTION_ID, "/").await.expect("列目录");

        assert_eq!(entries.len(), 1);
        assert_eq!(
            fixture.api.list_drive_ids(),
            vec!["drive-from-login".to_string()]
        );
    }

    #[tokio::test]
    async fn a_drive_id_that_cannot_be_found_asks_the_user_to_authorize() {
        let fixture = fixture("drive-id-missing");
        let mut session = session_with(FRESH_TOKEN, Some("refresh-token"));
        session.drive_id = None;
        fixture.sessions.insert(CREDENTIAL_KEY, session);
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("必须失败");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    // ------------------------------------------------------------------
    // 传输层：gzip 协商
    // ------------------------------------------------------------------

    /// 阿里云盘服务端只理「声明了 gzip」的请求。
    ///
    /// 实测（2026-11-20，证据 `.superpowers/sdd/b29-diag-05.txt`）：同一个
    /// curl，只差一个头 ——
    ///
    /// - 不带 `Accept-Encoding` ⇒ 服务端（istio-envoy）只回响应头、承诺
    ///   `Content-Length`，然后把响应体挂住 20 秒再掐断连接
    ///   （curl exit 18 / 0 字节；reqwest 报 `error decoding response body`）；
    /// - 带 `Accept-Encoding: gzip` ⇒ 0.19 秒 / 2580 字节，正常返回。
    ///
    /// reqwest 的 `gzip` 是**编译期** feature（`Cargo.toml` 里
    /// `features = ["rustls", "stream", "gzip"]`）：不开它，请求就不会带
    /// `Accept-Encoding` 头 ⇒ 正好命中上面那个 20 秒挂起。
    ///
    /// 这条测试起一个**只会说 gzip** 的本地服务，把「客户端必须能协商并
    /// 解压 gzip」这条契约钉死：feature 关掉时拿到的是原始压缩字节，
    /// 断言立刻变红。
    #[tokio::test]
    async fn the_http_client_negotiates_gzip_with_aliyun() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");

            // 读到请求头结束。
            let mut head = Vec::new();
            let mut chunk = [0u8; 1024];
            while !head.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = tokio::io::AsyncReadExt::read(&mut socket, &mut chunk)
                    .await
                    .expect("read request");
                if read == 0 {
                    break;
                }
                head.extend_from_slice(&chunk[..read]);
            }

            // 只说 gzip。
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            std::io::Write::write_all(&mut encoder, b"{\"items\":[]}")
                .expect("gzip payload");
            let compressed = encoder.finish().expect("finish gzip");

            let mut response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                compressed.len()
            )
            .into_bytes();
            response.extend_from_slice(&compressed);
            tokio::io::AsyncWriteExt::write_all(&mut socket, &response)
                .await
                .expect("write response");
            tokio::io::AsyncWriteExt::flush(&mut socket).await.ok();

            String::from_utf8_lossy(&head).to_string()
        });

        let client = build_client().expect("建 HTTP 客户端");
        let response = client
            .post(format!("http://{addr}/adrive/v3/file/list"))
            .body("{}")
            .send()
            .await
            .expect("请求必须成功");
        let bytes = response.bytes().await.expect("读响应体");
        let request_head = server.await.expect("服务端任务");

        assert!(
            request_head.to_ascii_lowercase().contains("accept-encoding"),
            "请求必须声明 Accept-Encoding，否则阿里云盘服务端会挂住响应体：\n{request_head}"
        );
        assert_eq!(
            String::from_utf8(bytes.to_vec()).expect("响应体必须是 UTF-8 明文"),
            "{\"items\":[]}",
            "gzip 响应体必须被自动解压，否则拿到的是原始压缩字节"
        );
    }
}
