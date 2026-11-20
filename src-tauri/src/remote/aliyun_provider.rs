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
// 能力边界：**能列目录、能读文件**；**其余一律不支持**。
// 上传（`write_file` / `write_file_with_progress`）、新建目录、改权限
// （`set_permissions` / `get_permissions`）、删除、重命名全部返回
// `RemoteErrorKind::Unsupported`，绝不返回 `Ok(())` —— 静默成功会让用户以为
// 文件已经传上去了，而它根本没有。
//
// 为什么网络访问要包一层 trait：`AliyunApi` 与 `AliyunSessionStore` 两个抽象
// 让本文件可以**零网络**单测 —— 假实现不需要真的连 `openapi.alipan.com`，
// 也不需要真的走 DPAPI 密文。
//
// ⚠️ **阿里云盘的条目没有路径，只有 `file_id`** —— 服务端不提供「按路径查条目」
// 的能力。所以「远端路径 → 目录 file_id」只能从根（`file_id = "root"`）一级
// 一级往下走。详见 `AliyunRemoteProvider::resolve_dir_file_id`。
//
// ⚠️ **写类操作一条都不许返回 `Ok(())`**（契约 §〇）。
// ---------------------------------------------------------------------------

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;

use super::aliyun_api::{
    aliyun_access_token_url, aliyun_authorization_code_body, aliyun_download_url_api,
    aliyun_get_url, aliyun_list_entry_limit_error, aliyun_list_page_limit_error,
    aliyun_list_url, aliyun_map_failure, aliyun_next_marker, aliyun_parse_download_url,
    aliyun_parse_entry_body, aliyun_parse_list_payload, aliyun_parse_token_payload,
    aliyun_refresh_token_body, aliyun_refresh_token_url, aliyun_resolve_drive_id,
    aliyun_to_remote_entries, AliyunFileEntry, AliyunListPayload, AliyunTokenPayload,
    ALIYUN_LIST_LIMIT_DEFAULT, ALIYUN_LIST_ORDER_BY, ALIYUN_LIST_ORDER_DIRECTION,
    ALIYUN_ROOT_FILE_ID,
};
use super::aliyun_auth::{
    aliyun_is_expired, aliyun_now_unix, load_aliyun_session, save_aliyun_session, AliyunAuthSession,
};
use super::connection::RemoteConnection;
use super::error::{RemoteError, RemoteErrorKind};
use super::path::{
    normalize_remote_path, reject_path_control_characters, split_remote_parent_and_name,
    validate_remote_entry_name,
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

    if !status.is_success() {
        // 只记状态码与错误码，**不记响应原文** —— 原文由服务端控制，
        // 里面可能夹着不该进日志的东西。
        let failure = super::aliyun_api::aliyun_parse_failure(&text);
        log::warn!(
            "Aliyun API failure url={url} status={} code={:?}",
            status.as_u16(),
            failure.code
        );
        return Err(aliyun_map_failure(status.as_u16(), &text));
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
    let response = client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("x-requested-with", "XMLHttpRequest")
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

    if !status.is_success() {
        // 只记状态码与错误码，**不记响应原文** —— 原文由服务端控制，
        // 里面可能夹着不该进日志的东西。
        let failure = super::aliyun_api::aliyun_parse_failure(&text);
        log::warn!(
            "Aliyun token failure url={url} status={} code={:?}",
            status.as_u16(),
            failure.code
        );
        return Err(aliyun_map_failure(status.as_u16(), &text));
    }

    Ok(text)
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

/// 走真实 HTTP 的实现。
pub struct HttpAliyunApi {
    client: reqwest::Client,
    download_client: reqwest::Client,
}

impl HttpAliyunApi {
    /// 生产实现：真 HTTP。
    pub fn new() -> Result<Self, RemoteError> {
        Ok(Self {
            client: build_client()?,
            download_client: build_download_client()?,
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

        let body = aliyun_refresh_token_body(&session.client_id, refresh_token);
        let text = post_json_text_unauthenticated(
            &self.client,
            aliyun_refresh_token_url(),
            &body.to_string(),
        )
        .await?;
        let payload = aliyun_parse_token_payload(&text)?;

        Ok(aliyun_session_from_token_payload(
            &payload,
            &session.client_id,
            session.redirect_uri.clone(),
            session.scope.clone(),
            aliyun_now_unix(),
            session.drive_id.clone(),
            session.refresh_token.clone(),
            session.nick_name.clone(),
        ))
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

    /// 所有「改内容」的操作的统一出口：一律明确报「不支持」。
    ///
    /// **绝不返回 `Ok(())`。** 上传（把本地文件传上云盘）不在只读阶段的范围里，
    /// 静默成功会让用户以为文件已经传上去了，而它根本没有。
    fn reject_write(
        &self,
        connection_id: &str,
        remote_path: &str,
        message: &str,
    ) -> Result<(), RemoteError> {
        self.prepare_path(connection_id, remote_path)?;

        Err(RemoteError::unsupported(message))
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

    async fn write_file(
        &self,
        connection_id: &str,
        remote_path: &str,
        _content: Vec<u8>,
    ) -> Result<(), RemoteError> {
        self.reject_write(
            connection_id,
            remote_path,
            "阿里云盘的上传（把本地文件传上去）在只读阶段还不支持。",
        )
    }

    async fn write_file_with_progress(
        &self,
        connection_id: &str,
        remote_path: &str,
        _content: Vec<u8>,
        _on_progress: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<(), RemoteError> {
        // 一个字节都没写 ⇒ **绝不能**触发任何进度回调。
        self.reject_write(
            connection_id,
            remote_path,
            "阿里云盘的上传（把本地文件传上去）在只读阶段还不支持。",
        )
    }

    async fn make_dir(&self, connection_id: &str, remote_path: &str) -> Result<(), RemoteError> {
        let (_, normalized) = self.prepare_path(connection_id, remote_path)?;

        // 名字要单独校验：名字里带 `/` 会让一次「新建目录」变成「建到别处去」。
        let (_parent, name) = split_remote_parent_and_name(&normalized)?;
        validate_remote_entry_name(&name)?;

        Err(RemoteError::unsupported(
            "阿里云盘的新建目录在只读阶段还不支持。",
        ))
    }

    async fn set_permissions(
        &self,
        connection_id: &str,
        remote_path: &str,
        _mode: u32,
    ) -> Result<(), RemoteError> {
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
        // 契约 §〇：改权限（含**查**权限）在只读阶段一律不支持。
        // 这里**不联网** —— 走一遍网络只为回答「不支持」是白花的代价。
        self.prepare_path(connection_id, remote_path)?;

        Err(RemoteError::unsupported(
            "阿里云盘没有 POSIX 权限位，查询权限暂不支持。",
        ))
    }

    async fn remove_path(
        &self,
        connection_id: &str,
        remote_path: &str,
        _is_dir: bool,
    ) -> Result<(), RemoteError> {
        let (_, normalized) = self.prepare_path(connection_id, remote_path)?;

        // 根目录没有条目名，删它就是删整个网盘 —— 这条要在开网络之前挡掉。
        if normalized == "/" {
            return Err(RemoteError::protocol("不能删除阿里云盘的根目录。"));
        }

        Err(RemoteError::unsupported(
            "阿里云盘的删除在只读阶段还不支持。",
        ))
    }

    async fn rename_path(
        &self,
        connection_id: &str,
        from_path: &str,
        new_name: &str,
    ) -> Result<(), RemoteError> {
        // 新名字先校验：**改名只能是改名，不能顺手变成移动**。
        validate_remote_entry_name(new_name)?;

        self.prepare_path(connection_id, from_path)?;

        Err(RemoteError::unsupported(
            "阿里云盘的重命名在只读阶段还不支持。",
        ))
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
    // 写类操作：一律不支持（契约 §〇 / §6.1 锚点 12）
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn every_write_operation_reports_that_it_is_unsupported() {
        let fixture = fixture("write-ops");
        let provider = provider(&fixture);

        let upload = provider
            .write_file(CONNECTION_ID, "/a.txt", b"hello".to_vec())
            .await;
        assert_eq!(
            upload.expect_err("上传必须失败").kind,
            RemoteErrorKind::Unsupported
        );

        let upload_with_progress = provider
            .write_file_with_progress(CONNECTION_ID, "/a.txt", b"hello".to_vec(), &|_| {})
            .await;
        assert_eq!(
            upload_with_progress.expect_err("带进度上传必须失败").kind,
            RemoteErrorKind::Unsupported
        );

        let make_dir = provider.make_dir(CONNECTION_ID, "/新建").await;
        assert_eq!(
            make_dir.expect_err("新建目录必须失败").kind,
            RemoteErrorKind::Unsupported
        );

        let set_permissions = provider
            .set_permissions(CONNECTION_ID, "/a.txt", 0o644)
            .await;
        assert_eq!(
            set_permissions.expect_err("改权限必须失败").kind,
            RemoteErrorKind::Unsupported
        );

        let get_permissions = provider.get_permissions(CONNECTION_ID, "/a.txt").await;
        assert_eq!(
            get_permissions.expect_err("查权限必须失败").kind,
            RemoteErrorKind::Unsupported
        );

        let remove = provider.remove_path(CONNECTION_ID, "/a.txt", false).await;
        assert_eq!(
            remove.expect_err("删除必须失败").kind,
            RemoteErrorKind::Unsupported
        );

        let rename = provider.rename_path(CONNECTION_ID, "/a.txt", "b.txt").await;
        assert_eq!(
            rename.expect_err("重命名必须失败").kind,
            RemoteErrorKind::Unsupported
        );
    }

    #[tokio::test]
    async fn a_write_operation_never_reports_success() {
        let fixture = fixture("write-never-ok");
        let provider = provider(&fixture);

        assert!(provider
            .write_file(CONNECTION_ID, "/a.txt", b"hello".to_vec())
            .await
            .is_err());
        assert!(provider.make_dir(CONNECTION_ID, "/新建").await.is_err());
        assert!(provider
            .remove_path(CONNECTION_ID, "/a.txt", false)
            .await
            .is_err());
        assert!(provider
            .rename_path(CONNECTION_ID, "/a.txt", "b.txt")
            .await
            .is_err());
    }

    #[tokio::test]
    async fn a_failed_upload_never_triggers_a_progress_callback() {
        use std::sync::atomic::AtomicU64;

        let fixture = fixture("write-progress");
        let provider = provider(&fixture);
        let counter = AtomicU64::new(0);

        let result = provider
            .write_file_with_progress(CONNECTION_ID, "/a.txt", b"hello".to_vec(), &|value| {
                counter.fetch_add(value, Ordering::SeqCst);
            })
            .await;

        assert!(result.is_err());
        assert_eq!(
            counter.load(Ordering::SeqCst),
            0,
            "一个字节都没写，不该有进度回调"
        );
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
