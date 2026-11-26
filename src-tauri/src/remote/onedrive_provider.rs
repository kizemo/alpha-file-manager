// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! OneDrive（Microsoft Graph）**读写** provider。
//!
//! 红线（与 `remote/mod.rs:8-14` 同一条）：本文件被 `cargo test --lib` 的测试
//! 二进制链接，**不得出现 `tauri::` 的任何路径**。一旦碰到，Tauri 全局运行时
//! 的默认类型参数 `Wry` 被单态化，测试二进制会在**加载阶段**以
//! `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉。
//!
//! ## 权限边界
//!
//! `access_token` / `refresh_token` 明文只在本模块、`remote::onedrive_auth` 与
//! 顶层 `onedrive_oauth` 之间流转，**不出现在命令返回值、事件 payload 与日志里**。
//!
//! ## 读写
//!
//! 批次 38 是「只读接入」；批次 40 按用户反馈把它升级为**完整读写**：
//! 上传、新建目录、改名、删除都真的落到 Microsoft Graph 上。
//!
//! **唯一还明说不支持的是「改权限」** —— Graph 没有 POSIX 权限位，
//! 假装支持比明说不支持更糟。
//!
//! 写路径的三条安全边界：
//!
//! - 新建目录用 `@microsoft.graph.conflictBehavior: "fail"` ——
//!   同名目录**绝不**被静默覆盖（用户以为新建了一个，其实是换掉了原来那个）；
//! - 改名只带**新名字**，不带路径（Graph 的 PATCH 只认 name）；
//! - 删除与改名都先把路径解析成条目 id 再打，根目录一律拒绝。

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use super::connection_test::{ConnectionTestReport, ProbeOutcome, ProbeTarget};
use super::error::{RemoteError, RemoteErrorKind};
use super::onedrive_api::{
    onedrive_build_children_by_id_url, onedrive_build_children_url, onedrive_build_content_url,
    onedrive_build_create_folder_body, onedrive_build_item_by_id_url, onedrive_build_item_url,
    onedrive_build_rename_body, onedrive_build_root_children_url,
    onedrive_build_upload_session_url, onedrive_build_upload_url, onedrive_collect_children_pages,
    onedrive_graph_item_to_remote_entry, onedrive_map_graph_error, onedrive_parse_children_page,
    onedrive_parse_drive_info, onedrive_parse_item, onedrive_parse_upload_session,
    onedrive_split_parent_and_name, OnedriveByteRange, OnedriveChildrenPage, OnedriveDriveInfo,
    OnedriveGraphItem, ONEDRIVE_GRAPH_DRIVE_URL, ONEDRIVE_MAX_DOWNLOAD_BYTES,
    ONEDRIVE_MAX_SIMPLE_UPLOAD_BYTES, ONEDRIVE_UPLOAD_CHUNK_BYTES, ONEDRIVE_USER_AGENT,
};
use super::onedrive_auth::{
    delete_onedrive_session, load_onedrive_session, onedrive_apply_refresh,
    onedrive_is_expired, onedrive_now_unix, onedrive_parse_token_response,
    onedrive_refresh_request_body, save_onedrive_session, OnedriveAuthSession,
    ONEDRIVE_TOKEN_URL,
};
use super::path::{normalize_remote_path, reject_path_control_characters};
use super::provider::{RemoteEntry, RemoteProvider};
use super::registry;
use super::transfer::unsupported_set_permissions;

/// 单次 Graph 请求的超时。
pub const ONEDRIVE_OPERATION_TIMEOUT: Duration = Duration::from_secs(120);

/// 连接超时。
pub const ONEDRIVE_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// 列目录的总时限。
pub const ONEDRIVE_DIRECTORY_TIMEOUT: Duration = Duration::from_secs(180);

/// 读一个文件（含下载）的总时限。文件要整个进内存 ⇒ 比列表宽松。
pub const ONEDRIVE_FILE_TIMEOUT: Duration = Duration::from_secs(600);

/// 一个文件内容的取法。
///
/// Graph 的 `/content` 端点**通常**回 302 + `Location`（预认证直链），
/// 但少数情形会直接把内容放在响应体里。两种形状都收下 ——
/// 硬假设「一定是 302」会在第二种情形下变成一次莫名其妙的 401。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnedriveContentTarget {
    /// 预认证直链。**不带 `Authorization`**、只带 `Range` 去下载。
    PreauthenticatedUrl(String),
    /// 服务端直接把内容放在响应体里。
    Inline(Vec<u8>),
}

/// Graph 接口。抽成 trait 是为了**零 DPAPI、零网络的单测**。
#[async_trait]
pub trait OnedriveApi: Send + Sync {
    /// 列一页子项。`skip_token` 是上一页 `@odata.nextLink` 里的 `$skipToken`。
    async fn list_children_page(
        &self,
        session: &OnedriveAuthSession,
        remote_path: &str,
        skip_token: Option<&str>,
    ) -> Result<OnedriveChildrenPage, RemoteError>;

    /// 取文件内容：要么给预认证直链，要么直接给内容。
    async fn content_location(
        &self,
        session: &OnedriveAuthSession,
        remote_path: &str,
    ) -> Result<OnedriveContentTarget, RemoteError>;

    /// 在**预认证直链**上下载。**不得**带 `Authorization` ——
    /// 那条直链自身即授权，带上 Bearer 反而会让微软拒掉它。
    async fn download_preauthenticated(
        &self,
        url: &str,
        range: Option<OnedriveByteRange>,
    ) -> Result<Vec<u8>, RemoteError>;

    /// 用 refresh token 换一份新会话。**必须**带回新的 `refresh_token`。
    async fn refresh_session(
        &self,
        session: &OnedriveAuthSession,
    ) -> Result<OnedriveAuthSession, RemoteError>;

    /// 取账号与主盘信息。
    async fn drive_info(
        &self,
        session: &OnedriveAuthSession,
    ) -> Result<OnedriveDriveInfo, RemoteError>;

    // ---- 写路径（批次 40）--------------------------------------------------

    /// 按路径取单个条目（拿它的 id）。
    ///
    /// Graph 的「改名」「删除」只认条目 id，不认路径 ⇒ 先查一次。
    async fn item_by_path(
        &self,
        session: &OnedriveAuthSession,
        remote_path: &str,
    ) -> Result<OnedriveGraphItem, RemoteError>;

    /// 上传文件内容（覆盖语义）。
    ///
    /// on_progress 收的是**累计字节数**。
    async fn upload_content(
        &self,
        session: &OnedriveAuthSession,
        remote_path: &str,
        content: &[u8],
        on_progress: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<(), RemoteError>;

    /// 新建目录。传**完整目标路径**（父目录必须已存在）。
    async fn create_folder(
        &self,
        session: &OnedriveAuthSession,
        remote_path: &str,
    ) -> Result<(), RemoteError>;

    /// 改名。new_name 只是**名字**，不含路径。
    async fn rename_item(
        &self,
        session: &OnedriveAuthSession,
        remote_path: &str,
        new_name: &str,
    ) -> Result<(), RemoteError>;

    /// 删除（文件或空目录）。
    async fn delete_item(
        &self,
        session: &OnedriveAuthSession,
        remote_path: &str,
    ) -> Result<(), RemoteError>;
}

/// 「Graph 客户端建不起来」时的替身：每次调用都报同一个错。
///
/// 给 [`OnedriveRemoteProvider::new_deferred`] 用 —— 它的签名里没有 `Result`。
#[derive(Debug)]
pub struct UnavailableOnedriveApi {
    error: RemoteError,
}

impl UnavailableOnedriveApi {
    pub fn new(error: RemoteError) -> Self {
        Self { error }
    }
}

#[async_trait]
impl OnedriveApi for UnavailableOnedriveApi {
    async fn list_children_page(
        &self,
        _session: &OnedriveAuthSession,
        _remote_path: &str,
        _skip_token: Option<&str>,
    ) -> Result<OnedriveChildrenPage, RemoteError> {
        Err(self.error.clone())
    }

    async fn content_location(
        &self,
        _session: &OnedriveAuthSession,
        _remote_path: &str,
    ) -> Result<OnedriveContentTarget, RemoteError> {
        Err(self.error.clone())
    }

    async fn download_preauthenticated(
        &self,
        _url: &str,
        _range: Option<OnedriveByteRange>,
    ) -> Result<Vec<u8>, RemoteError> {
        Err(self.error.clone())
    }

    async fn refresh_session(
        &self,
        _session: &OnedriveAuthSession,
    ) -> Result<OnedriveAuthSession, RemoteError> {
        Err(self.error.clone())
    }

    async fn drive_info(
        &self,
        _session: &OnedriveAuthSession,
    ) -> Result<OnedriveDriveInfo, RemoteError> {
        Err(self.error.clone())
    }

    async fn item_by_path(
        &self,
        _session: &OnedriveAuthSession,
        _remote_path: &str,
    ) -> Result<OnedriveGraphItem, RemoteError> {
        Err(self.error.clone())
    }

    async fn upload_content(
        &self,
        _session: &OnedriveAuthSession,
        _remote_path: &str,
        _content: &[u8],
        _on_progress: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<(), RemoteError> {
        Err(self.error.clone())
    }

    async fn create_folder(
        &self,
        _session: &OnedriveAuthSession,
        _remote_path: &str,
    ) -> Result<(), RemoteError> {
        Err(self.error.clone())
    }

    async fn rename_item(
        &self,
        _session: &OnedriveAuthSession,
        _remote_path: &str,
        _new_name: &str,
    ) -> Result<(), RemoteError> {
        Err(self.error.clone())
    }

    async fn delete_item(
        &self,
        _session: &OnedriveAuthSession,
        _remote_path: &str,
    ) -> Result<(), RemoteError> {
        Err(self.error.clone())
    }
}

/// 会话存档的读写。**抽成 trait 是为了零 DPAPI 单测。**
pub trait OnedriveSessionStore: Send + Sync {
    /// 按键读会话。没有就是 `Ok(None)`；存档坏了要**报错**，不能当成「没存」。
    fn load(&self, key: &str) -> Result<Option<OnedriveAuthSession>, RemoteError>;

    /// 写回会话。**刷新后必须调用** ——
    /// 微软的 refresh token 有 90 天滚动窗口，不覆盖会让用户在第 91 天突然被迫重新登录。
    fn save(&self, key: &str, session: &OnedriveAuthSession) -> Result<(), RemoteError>;

    /// 删掉会话。键不存在是成功的空操作。
    fn delete(&self, key: &str) -> Result<(), RemoteError>;
}

/// 走既有 DPAPI 密文（`remote-credentials.json`）的会话存储。
pub struct DpapiOnedriveSessionStore {
    app_data_dir: PathBuf,
}

impl DpapiOnedriveSessionStore {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            app_data_dir: app_data_dir.to_path_buf(),
        }
    }
}

impl OnedriveSessionStore for DpapiOnedriveSessionStore {
    fn load(&self, key: &str) -> Result<Option<OnedriveAuthSession>, RemoteError> {
        load_onedrive_session(&self.app_data_dir, key)
    }

    fn save(&self, key: &str, session: &OnedriveAuthSession) -> Result<(), RemoteError> {
        save_onedrive_session(&self.app_data_dir, key, session)
    }

    fn delete(&self, key: &str) -> Result<(), RemoteError> {
        delete_onedrive_session(&self.app_data_dir, key)
    }
}

/// 走真实 HTTP 的实现。
pub struct HttpOnedriveApi {
    /// Graph 接口专用：**不跟随重定向** ——
    /// `/content` 的 302 我们要自己读 `Location`，跟过去就看不到那根直链了。
    client: reqwest::Client,
    /// 下载专用：**跟随重定向**（直链本身可能再跳一次）。
    download_client: reqwest::Client,
}

/// 建一个带 UA 的 Graph 客户端（不跟随重定向）。
fn build_graph_client() -> Result<reqwest::Client, RemoteError> {
    reqwest::Client::builder()
        .user_agent(ONEDRIVE_USER_AGENT)
        .timeout(ONEDRIVE_OPERATION_TIMEOUT)
        .connect_timeout(ONEDRIVE_CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| RemoteError::unknown(format!("OneDrive HTTP 客户端初始化失败：{error}")))
}

/// 建一个下载专用客户端（跟随重定向）。
fn build_download_client() -> Result<reqwest::Client, RemoteError> {
    reqwest::Client::builder()
        .user_agent(ONEDRIVE_USER_AGENT)
        .timeout(ONEDRIVE_FILE_TIMEOUT)
        .connect_timeout(ONEDRIVE_CONNECT_TIMEOUT)
        .build()
        .map_err(|error| RemoteError::unknown(format!("OneDrive 下载客户端初始化失败：{error}")))
}

/// 把响应体读成 UTF-8 文本。
///
/// ⚠️ **不能用 `reqwest` 的 `text()`**：那个方法被 `charset` feature 门控，
/// 而本 crate 的 `reqwest` 是 `default-features = false` ⇒ 用了就编不过。
async fn read_body(response: reqwest::Response) -> Result<String, RemoteError> {
    let bytes = response
        .bytes()
        .await
        .map_err(|error| RemoteError::disconnected(format!("读取 OneDrive 响应失败：{error}")))?;

    String::from_utf8(bytes.to_vec())
        .map_err(|_| RemoteError::encoding("OneDrive 返回的内容编码无法解析。"))
}

/// 取出 `Retry-After` 头（秒数或 HTTP date 都收，解析在 `onedrive_api` 里）。
fn retry_after_header(response: &reqwest::Response) -> Option<String> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

impl HttpOnedriveApi {
    /// 生产实现：真 HTTP。
    pub fn new() -> Result<Self, RemoteError> {
        Ok(Self {
            client: build_graph_client()?,
            download_client: build_download_client()?,
        })
    }

    /// 发一个带 Bearer 的 GET，把失败翻译成本项目的错误。
    async fn graph_get(&self, url: &str, token: &str) -> Result<String, RemoteError> {
        let response = self
            .client
            .get(url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", token.trim()),
            )
            .send()
            .await
            .map_err(|error| {
                RemoteError::unreachable(format!(
                    "无法连接 OneDrive：网络不可用、DNS 解析失败或微软服务暂时不可用，请稍后重试。（{error}）"
                ))
            })?;

        let status = response.status();
        let retry_after = retry_after_header(&response);
        let body = read_body(response).await?;

        if !status.is_success() {
            return Err(onedrive_map_graph_error(
                status.as_u16(),
                &body,
                retry_after.as_deref(),
            ));
        }

        Ok(body)
    }

    /// 发一个带 Bearer 的请求（POST / PUT / PATCH / DELETE），
    /// 把失败翻译成本项目的错误。
    ///
    /// body 为 None 时不发请求体。
    async fn graph_send(
        &self,
        method: reqwest::Method,
        url: &str,
        token: &str,
        content_type: Option<&str>,
        body: Option<Vec<u8>>,
    ) -> Result<String, RemoteError> {
        let mut request = self
            .client
            .request(method, url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", token.trim()),
            );

        if let Some(content_type) = content_type {
            request = request.header(reqwest::header::CONTENT_TYPE, content_type);
        }

        if let Some(body) = body {
            request = request.body(body);
        }

        let response = request.send().await.map_err(|error| {
            RemoteError::unreachable(format!(
                "无法连接 OneDrive：网络不可用、DNS 解析失败或微软服务暂时不可用，请稍后重试。（{error}）"
            ))
        })?;

        let status = response.status();
        let retry_after = retry_after_header(&response);
        let text = read_body(response).await?;

        if !status.is_success() {
            return Err(onedrive_map_graph_error(
                status.as_u16(),
                &text,
                retry_after.as_deref(),
            ));
        }

        Ok(text)
    }

    /// 上传会话的分块 PUT。**不带 Authorization** ——
    /// 上传会话 URL 自身即授权，带上 Bearer 反而可能被拒。
    async fn upload_chunk(
        &self,
        upload_url: &str,
        chunk: &[u8],
        offset: u64,
        total: u64,
    ) -> Result<(), RemoteError> {
        let response = self
            .client
            .put(upload_url)
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .header(
                reqwest::header::CONTENT_RANGE,
                format!(
                    "bytes {}-{}/{}",
                    offset,
                    offset + chunk.len() as u64 - 1,
                    total
                ),
            )
            .body(chunk.to_vec())
            .send()
            .await
            .map_err(|error| {
                RemoteError::unreachable(format!(
                    "无法连接 OneDrive：上传分块时网络中断，请稍后重试。（{error}）"
                ))
            })?;

        let status = response.status();
        if !status.is_success() {
            let retry_after = retry_after_header(&response);
            let text = read_body(response).await?;

            return Err(onedrive_map_graph_error(
                status.as_u16(),
                &text,
                retry_after.as_deref(),
            ));
        }

        Ok(())
    }
}

#[async_trait]
impl OnedriveApi for HttpOnedriveApi {
    async fn list_children_page(
        &self,
        session: &OnedriveAuthSession,
        remote_path: &str,
        skip_token: Option<&str>,
    ) -> Result<OnedriveChildrenPage, RemoteError> {
        let mut url = onedrive_build_children_url(remote_path);

        if let Some(token) = skip_token.map(str::trim).filter(|token| !token.is_empty()) {
            url.push_str(&format!("?$skipToken={}", urlencoding::encode(token)));
        }

        let body = self.graph_get(&url, &session.access_token).await?;
        onedrive_parse_children_page(&body)
    }

    async fn content_location(
        &self,
        session: &OnedriveAuthSession,
        remote_path: &str,
    ) -> Result<OnedriveContentTarget, RemoteError> {
        let url = onedrive_build_content_url(remote_path);

        let response = self
            .client
            .get(&url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", session.access_token.trim()),
            )
            .send()
            .await
            .map_err(|error| {
                RemoteError::unreachable(format!(
                    "无法连接 OneDrive：网络不可用、DNS 解析失败或微软服务暂时不可用，请稍后重试。（{error}）"
                ))
            })?;

        let status = response.status();

        if status.is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());

            return match location {
                Some(location) => Ok(OnedriveContentTarget::PreauthenticatedUrl(location)),
                None => Err(RemoteError::protocol(
                    "OneDrive 给了重定向但没有给出下载地址，无法读取这个文件。",
                )),
            };
        }

        if status.is_success() {
            // 少数情形：服务端直接把内容放在响应体里。
            let bytes = response
                .bytes()
                .await
                .map_err(|error| {
                    RemoteError::disconnected(format!("读取 OneDrive 响应失败：{error}"))
                })?;

            if bytes.len() as u64 > ONEDRIVE_MAX_DOWNLOAD_BYTES {
                return Err(RemoteError::unsupported(
                    "这个 OneDrive 文件太大了，已中止下载。",
                ));
            }

            return Ok(OnedriveContentTarget::Inline(bytes.to_vec()));
        }

        let retry_after = retry_after_header(&response);
        let body = read_body(response).await?;

        Err(onedrive_map_graph_error(
            status.as_u16(),
            &body,
            retry_after.as_deref(),
        ))
    }

    async fn download_preauthenticated(
        &self,
        url: &str,
        range: Option<OnedriveByteRange>,
    ) -> Result<Vec<u8>, RemoteError> {
        let mut request = self.download_client.get(url);

        if let Some(range) = range {
            request = request.header(reqwest::header::RANGE, range.to_header_value());
        }

        // ⚠️ **不带 `Authorization`**：预认证直链自身即授权。
        let response = request.send().await.map_err(|error| {
            RemoteError::unreachable(format!(
                "无法连接 OneDrive：网络不可用、DNS 解析失败或微软服务暂时不可用，请稍后重试。（{error}）"
            ))
        })?;

        let status = response.status();
        if !status.is_success() {
            return Err(RemoteError::unreachable(format!(
                "下载 OneDrive 文件失败（HTTP {}）。",
                status.as_u16()
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|error| RemoteError::disconnected(format!("下载 OneDrive 文件失败：{error}")))?;

        if bytes.len() as u64 > ONEDRIVE_MAX_DOWNLOAD_BYTES {
            return Err(RemoteError::unsupported(
                "这个 OneDrive 文件太大了，已中止下载。",
            ));
        }

        Ok(bytes.to_vec())
    }

    async fn refresh_session(
        &self,
        session: &OnedriveAuthSession,
    ) -> Result<OnedriveAuthSession, RemoteError> {
        let client_id = session.client_id.trim();
        if client_id.is_empty() {
            return Err(RemoteError::auth(
                "OneDrive 会话里没有 client_id，无法自动续期，请重新登录。",
            ));
        }

        // **没有 client_secret**：这是公共客户端（PKCE）流程。
        //
        // ⚠️ **刻意不带 `scope`**：刷新时**不传**就沿用当初授权的范围；
        // 传一个与当初不同的范围，微软回 `invalid_scope`（AADSTS70011），
        // 把用户**已经登录好的会话**直接废掉 —— 升级 scope（Files.Read → Files.ReadWrite）
        // 之后，升级前登录过的用户一次都续不了期，只能被逼着重新登录。
        let body = onedrive_refresh_request_body(client_id, session.refresh_token.trim());

        let response = self
            .client
            .post(ONEDRIVE_TOKEN_URL)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(body)
            .send()
            .await
            .map_err(|error| {
                RemoteError::unreachable(format!(
                    "无法连接微软登录服务：网络不可用、DNS 解析失败或服务暂时不可用。（{error}）"
                ))
            })?;

        let status = response.status();
        let text = read_body(response).await?;

        if !status.is_success() {
            // 续期失败只有一种用户处置：**重新登录**。
            return Err(RemoteError::auth(
                "OneDrive 登录已过期且自动续期失败，请重新登录。",
            ));
        }

        let mut refreshed = onedrive_parse_token_response(
            client_id,
            &text,
            onedrive_now_unix(),
            Some(session.refresh_token.trim()),
        )?;

        onedrive_apply_refresh(session, &mut refreshed);

        Ok(refreshed)
    }

    async fn drive_info(
        &self,
        session: &OnedriveAuthSession,
    ) -> Result<OnedriveDriveInfo, RemoteError> {
        let body = self
            .graph_get(ONEDRIVE_GRAPH_DRIVE_URL, &session.access_token)
            .await?;

        onedrive_parse_drive_info(&body)
    }

    async fn item_by_path(
        &self,
        session: &OnedriveAuthSession,
        remote_path: &str,
    ) -> Result<OnedriveGraphItem, RemoteError> {
        let url = onedrive_build_item_url(remote_path);
        let body = self.graph_get(&url, &session.access_token).await?;

        onedrive_parse_item(&body)
    }

    async fn upload_content(
        &self,
        session: &OnedriveAuthSession,
        remote_path: &str,
        content: &[u8],
        on_progress: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<(), RemoteError> {
        let token = session.access_token.trim();

        // 小文件：一次 PUT 完事（Graph 的简单上传上限 250 MiB）。
        if content.len() <= ONEDRIVE_MAX_SIMPLE_UPLOAD_BYTES {
            let url = onedrive_build_upload_url(remote_path);

            self.graph_send(
                reqwest::Method::PUT,
                &url,
                token,
                Some("application/octet-stream"),
                Some(content.to_vec()),
            )
            .await?;

            on_progress(content.len() as u64);

            return Ok(());
        }

        // 大文件：先开上传会话，再分块 PUT（5 MiB = 16 x 320 KiB，Graph 要求）。
        let session_url = onedrive_build_upload_session_url(remote_path);
        let body = self
            .graph_send(
                reqwest::Method::POST,
                &session_url,
                token,
                Some("application/json"),
                Some(b"{}".to_vec()),
            )
            .await?;
        let upload_url = onedrive_parse_upload_session(&body)?;

        let total = content.len() as u64;
        let mut sent = 0u64;

        while sent < total {
            let end = (sent + ONEDRIVE_UPLOAD_CHUNK_BYTES as u64).min(total);
            let chunk = &content[sent as usize..end as usize];

            self.upload_chunk(&upload_url, chunk, sent, total).await?;

            sent = end;
            on_progress(sent);
        }

        Ok(())
    }

    async fn create_folder(
        &self,
        session: &OnedriveAuthSession,
        remote_path: &str,
    ) -> Result<(), RemoteError> {
        let Some((parent, name)) = onedrive_split_parent_and_name(remote_path) else {
            return Err(RemoteError::protocol(
                "新建目录需要一个具体的目录名，不能是根目录。",
            ));
        };

        let token = session.access_token.trim();

        // Graph 的「创建子项」只认**父目录 id**。父目录在根时走 root/children，
        // 否则先查一次父目录的 id。
        let url = if parent == "/" {
            onedrive_build_root_children_url()
        } else {
            let parent_item = self.item_by_path(session, &parent).await?;
            let parent_id = parent_item.id.trim();

            if parent_id.is_empty() {
                return Err(RemoteError::protocol(
                    "OneDrive 没有给出父目录的 id，无法在这个目录里新建子项。",
                ));
            }

            onedrive_build_children_by_id_url(parent_id)
        };

        self.graph_send(
            reqwest::Method::POST,
            &url,
            token,
            Some("application/json"),
            Some(onedrive_build_create_folder_body(&name).into_bytes()),
        )
        .await?;

        Ok(())
    }

    async fn rename_item(
        &self,
        session: &OnedriveAuthSession,
        remote_path: &str,
        new_name: &str,
    ) -> Result<(), RemoteError> {
        let item = self.item_by_path(session, remote_path).await?;
        let item_id = item.id.trim();

        if item_id.is_empty() {
            return Err(RemoteError::protocol(
                "OneDrive 没有给出这个条目的 id，无法重命名。",
            ));
        }

        self.graph_send(
            reqwest::Method::PATCH,
            &onedrive_build_item_by_id_url(item_id),
            session.access_token.trim(),
            Some("application/json"),
            Some(onedrive_build_rename_body(new_name).into_bytes()),
        )
        .await?;

        Ok(())
    }

    async fn delete_item(
        &self,
        session: &OnedriveAuthSession,
        remote_path: &str,
    ) -> Result<(), RemoteError> {
        let item = self.item_by_path(session, remote_path).await?;
        let item_id = item.id.trim();

        if item_id.is_empty() {
            return Err(RemoteError::protocol(
                "OneDrive 没有给出这个条目的 id，无法删除。",
            ));
        }

        self.graph_send(
            reqwest::Method::DELETE,
            &onedrive_build_item_by_id_url(item_id),
            session.access_token.trim(),
            None,
            None,
        )
        .await?;

        Ok(())
    }
}

/// 拿一个 access token 探一次 `/me/drive`，顺带取回账号显示名与盘 id。
///
/// **登录成功后用它补齐 `account` / `drive_id`** —— 取不到**不阻断登录**：
/// 账号名只是界面上的一行字，缺了不该让用户重新登录一次。
pub async fn onedrive_fetch_drive_info(access_token: &str) -> Result<OnedriveDriveInfo, RemoteError> {
    let client = build_graph_client()?;

    let response = client
        .get(ONEDRIVE_GRAPH_DRIVE_URL)
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", access_token.trim()),
        )
        .send()
        .await
        .map_err(|error| {
            RemoteError::unreachable(format!(
                "无法连接 OneDrive：网络不可用、DNS 解析失败或微软服务暂时不可用，请稍后重试。（{error}）"
            ))
        })?;

    let status = response.status();
    let retry_after = retry_after_header(&response);
    let body = read_body(response).await?;

    if !status.is_success() {
        return Err(onedrive_map_graph_error(
            status.as_u16(),
            &body,
            retry_after.as_deref(),
        ));
    }

    onedrive_parse_drive_info(&body)
}

/// OneDrive 的只读 provider。
pub struct OnedriveRemoteProvider {
    user_data_dir: PathBuf,
    api: Arc<dyn OnedriveApi>,
    sessions: Arc<dyn OnedriveSessionStore>,
    directory_timeout: Duration,
    file_timeout: Duration,
}

/// 不报告进度的空回调（上传进度是可选的）。
fn no_progress(_bytes: u64) {}

/// 带时限的包装。超时 ⇒ `Unreachable`（网络不可达那一类，不是协议错）。
async fn with_timeout<T>(
    limit: Duration,
    what: &'static str,
    work: impl Future<Output = Result<T, RemoteError>>,
) -> Result<T, RemoteError> {
    match tokio::time::timeout(limit, work).await {
        Ok(result) => result,
        Err(_) => Err(RemoteError::unreachable(format!(
            "{what}超时（{limit:?}），请稍后重试。"
        ))),
    }
}

impl OnedriveRemoteProvider {
    /// 生产实现：真 HTTP + 真 DPAPI。
    pub fn new(user_data_dir: &Path, app_data_dir: &Path) -> Result<Self, RemoteError> {
        Ok(Self::with_parts(
            user_data_dir,
            Arc::new(HttpOnedriveApi::new()?),
            Arc::new(DpapiOnedriveSessionStore::new(app_data_dir)),
        ))
    }

    /// **不返回 `Result`** 的构造：HTTP 客户端建不起来时，把错误推迟到第一次调用。
    ///
    /// 给 `commands.rs` 的 `provider_with_protocol` 用 —— 它的签名是
    /// `Box<dyn RemoteProvider>`，没有地方安放 `Err`。
    pub fn new_deferred(user_data_dir: &Path, app_data_dir: &Path) -> Self {
        let sessions: Arc<dyn OnedriveSessionStore> =
            Arc::new(DpapiOnedriveSessionStore::new(app_data_dir));

        match HttpOnedriveApi::new() {
            Ok(api) => Self::with_parts(user_data_dir, Arc::new(api), sessions),
            Err(error) => Self::with_parts(
                user_data_dir,
                Arc::new(UnavailableOnedriveApi::new(error)),
                sessions,
            ),
        }
    }

    /// 注入实现。**只给测试用** —— 生产路径走 [`Self::new`]。
    pub fn with_parts(
        user_data_dir: &Path,
        api: Arc<dyn OnedriveApi>,
        sessions: Arc<dyn OnedriveSessionStore>,
    ) -> Self {
        Self {
            user_data_dir: user_data_dir.to_path_buf(),
            api,
            sessions,
            directory_timeout: ONEDRIVE_DIRECTORY_TIMEOUT,
            file_timeout: ONEDRIVE_FILE_TIMEOUT,
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
    ) -> Result<(super::connection::RemoteConnection, String), RemoteError> {
        reject_path_control_characters(remote_path)?;

        let normalized = normalize_remote_path(remote_path).ok_or_else(|| {
            RemoteError::protocol(format!("远端路径含有不合法的段: {remote_path}"))
        })?;

        let connection = registry::find_connection(&self.user_data_dir, connection_id)?;

        Ok((connection, normalized))
    }

    /// 入参处理 + 取会话（必要时**先刷新**）。
    ///
    /// 找不到会话一律报 `Auth` —— 用户的处置只有一个动作：**重新登录**。
    async fn prepare(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> Result<(String, OnedriveAuthSession, String), RemoteError> {
        let (connection, normalized) = self.prepare_path(connection_id, remote_path)?;

        let Some(key) = connection
            .credential_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
        else {
            return Err(RemoteError::auth(
                "这条 OneDrive 连接还没有保存登录凭据，请先完成授权。",
            ));
        };

        let Some(mut session) = self.sessions.load(key)? else {
            return Err(RemoteError::auth(
                "OneDrive 还没有授权，请先在设置里完成授权。",
            ));
        };

        // **主动**发现过期：省掉一次注定 401 的请求。
        if onedrive_is_expired(session.expires_at, onedrive_now_unix()) {
            session = self.refresh_session(key, &session).await?;
        }

        Ok((key.to_string(), session, normalized))
    }

    /// 用刷新凭证换一份新会话，并**立即写回存档**。
    ///
    /// 写回是必须的：微软的 refresh token 有 90 天**滚动**窗口，
    /// 不覆盖旧的会让用户在第 91 天突然被迫重新登录，而且没有任何本地迹象可查。
    async fn refresh_session(
        &self,
        key: &str,
        session: &OnedriveAuthSession,
    ) -> Result<OnedriveAuthSession, RemoteError> {
        if !session.has_refresh_token() {
            return Err(RemoteError::auth(
                "OneDrive 登录已过期，且没有可用于续期的刷新凭证，请重新登录。",
            ));
        }

        let refreshed = match self.api.refresh_session(session).await {
            Ok(refreshed) => refreshed,
            Err(error) => {
                // 续期失败只有一种用户处置：**重新登录**。
                // 但网络问题不该被说成「请重新登录」—— 那会把用户引向错误的动作。
                return match error.kind {
                    RemoteErrorKind::Auth => Err(RemoteError::auth(
                        "OneDrive 登录已过期且自动续期失败，请重新登录。",
                    )),
                    _ => Err(error),
                };
            }
        };

        self.sessions.save(key, &refreshed)?;

        Ok(refreshed)
    }

    /// 列目录（带翻页）。
    async fn list_children(
        &self,
        session: &OnedriveAuthSession,
        remote_path: &str,
    ) -> Result<Vec<OnedriveGraphItem>, RemoteError> {
        let first = self
            .api
            .list_children_page(session, remote_path, None)
            .await?;

        // 闭包自己克隆 session 与路径 ⇒ 不借用任何局部变量，
        // `Send` 与生命周期都不会出问题。
        let api = Arc::clone(&self.api);
        let session_for_pages = session.clone();
        let path_for_pages = remote_path.to_string();

        onedrive_collect_children_pages(first, move |token: String| {
            let api = Arc::clone(&api);
            let session = session_for_pages.clone();
            let path = path_for_pages.clone();

            async move {
                api.list_children_page(&session, &path, Some(&token))
                    .await
            }
        })
        .await
    }

    /// 取一份**可用的** access token（批次 39：测试连接复用已保存的授权会话）。
    ///
    /// 三态，缺一不可：
    ///
    /// - `Ok(None)` —— 存档里**根本没有**会话（从未授权）。
    ///   调用方据此报 `MissingCredential`（「请先完成授权」），
    ///   **不得**报 `AuthRejected`（那是「服务器拒了这组凭据」，两回事）。
    /// - `Ok(Some(token))` —— 有可用令牌。
    /// - `Err(error)` —— 续期失败。调用方据此报 `AuthRejected` / `Unreachable`，
    ///   **不能**退化成「没有凭据」—— 那会把用户引向错误的动作。
    ///
    /// 写回规则：**只在真的刷新过之后**才写回存档。
    /// 没过期时一次都不写 —— 探针是纯读操作，不该改变下次测试的输入。
    pub(crate) async fn ensure_session_token(
        &self,
        key: &str,
    ) -> Result<Option<String>, RemoteError> {
        let trimmed_key = key.trim();

        if trimmed_key.is_empty() {
            return Ok(None);
        }

        let Some(session) = self.sessions.load(trimmed_key)? else {
            return Ok(None);
        };

        let session = if onedrive_is_expired(session.expires_at, onedrive_now_unix()) {
            // 续期失败 ⇒ 报错（RemoteErrorKind::Auth ⇒ 探针回 AuthRejected）。
            self.refresh_session(trimmed_key, &session).await?
        } else {
            session
        };

        if !session.has_access_token() {
            return Err(RemoteError::auth(
                "OneDrive 的会话存档里没有访问令牌，请重新登录。",
            ));
        }

        Ok(Some(session.access_token.clone()))
    }
}

#[async_trait]
impl RemoteProvider for OnedriveRemoteProvider {
    async fn read_dir(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> Result<Vec<RemoteEntry>, RemoteError> {
        let (_key, session, normalized) = self.prepare(connection_id, remote_path).await?;

        let work = async {
            let items = self.list_children(&session, &normalized).await?;

            Ok::<Vec<RemoteEntry>, RemoteError>(
                items
                    .iter()
                    .map(|item| {
                        onedrive_graph_item_to_remote_entry(item, connection_id, &normalized)
                    })
                    .collect(),
            )
        };

        with_timeout(self.directory_timeout, "读取 OneDrive 目录", work).await
    }

    /// 两步走：先打 Graph 的 `/content` 拿预认证直链，
    /// 再在**最终 URL** 上带 `Range: bytes=0-` 重新请求。
    ///
    /// 为什么不在 Graph 那一跳就带 `Range`：Graph 的 `/content` 会 302，
    /// `reqwest` 的重定向实现**默认会丢掉 `Range`** ⇒ 整份文件都会被拉下来。
    async fn read_file(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> Result<Vec<u8>, RemoteError> {
        let (_key, session, normalized) = self.prepare(connection_id, remote_path).await?;

        if normalized == "/" {
            return Err(RemoteError::not_found("远端空间根不是一个文件。"));
        }

        let work = async {
            match self.api.content_location(&session, &normalized).await? {
                OnedriveContentTarget::Inline(bytes) => Ok(bytes),
                OnedriveContentTarget::PreauthenticatedUrl(url) => {
                    self.api
                        .download_preauthenticated(&url, Some(OnedriveByteRange::from_start(0)))
                        .await
                }
            }
        };

        with_timeout(self.file_timeout, "读取 OneDrive 文件", work).await
    }

    // ---- 写路径（批次 40：OneDrive 从只读升级为读写）-------------------------

    async fn write_file(
        &self,
        connection_id: &str,
        remote_path: &str,
        content: Vec<u8>,
    ) -> Result<(), RemoteError> {
        self.write_file_with_progress(connection_id, remote_path, content, &no_progress)
            .await
    }

    async fn write_file_with_progress(
        &self,
        connection_id: &str,
        remote_path: &str,
        content: Vec<u8>,
        on_progress: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<(), RemoteError> {
        let (_key, session, normalized) = self.prepare(connection_id, remote_path).await?;

        if normalized == "/" {
            return Err(RemoteError::protocol(
                "远端空间根不是一个文件，不能往上面写。",
            ));
        }

        let work = async {
            self.api
                .upload_content(&session, &normalized, &content, on_progress)
                .await
        };

        with_timeout(self.file_timeout, "上传 OneDrive 文件", work).await
    }

    async fn make_dir(&self, connection_id: &str, remote_path: &str) -> Result<(), RemoteError> {
        let (_key, session, normalized) = self.prepare(connection_id, remote_path).await?;

        // 根目录已经存在 ⇒ 视为成功（trait 约定：已存在即成功）。
        if normalized == "/" {
            return Ok(());
        }

        let work = async {
            self.api.create_folder(&session, &normalized).await
        };

        with_timeout(self.directory_timeout, "新建 OneDrive 目录", work).await
    }

    async fn set_permissions(
        &self,
        _connection_id: &str,
        _remote_path: &str,
        _mode: u32,
    ) -> Result<(), RemoteError> {
        Err(unsupported_set_permissions("OneDrive"))
    }

    /// OneDrive 没有可靠的 POSIX 权限位 ⇒ `Ok(None)`（「取不到」），
    /// **不是** `Err` —— 前端用它判断「支不支持改权限」，
    /// 返回错误会让每一次打开对话框都变成一次未捕获异常。
    async fn get_permissions(
        &self,
        _connection_id: &str,
        _remote_path: &str,
    ) -> Result<Option<u32>, RemoteError> {
        Ok(None)
    }

    async fn remove_path(
        &self,
        connection_id: &str,
        remote_path: &str,
        _is_dir: bool,
    ) -> Result<(), RemoteError> {
        let (_key, session, normalized) = self.prepare(connection_id, remote_path).await?;

        if normalized == "/" {
            return Err(RemoteError::protocol("不能删除远端空间的根目录。"));
        }

        let work = async {
            self.api.delete_item(&session, &normalized).await
        };

        with_timeout(self.directory_timeout, "删除 OneDrive 文件或目录", work).await
    }

    async fn rename_path(
        &self,
        connection_id: &str,
        from_path: &str,
        new_name: &str,
    ) -> Result<(), RemoteError> {
        let name = new_name.trim();

        if name.is_empty() {
            return Err(RemoteError::protocol("新名字不能为空。"));
        }

        if name.contains('/') || name.contains('\\') {
            return Err(RemoteError::protocol(
                "新名字只能是文件名，不能包含路径分隔符。",
            ));
        }

        let (_key, session, normalized) = self.prepare(connection_id, from_path).await?;

        if normalized == "/" {
            return Err(RemoteError::protocol("不能重命名远端空间的根目录。"));
        }

        let work = async {
            self.api.rename_item(&session, &normalized, name).await
        };

        with_timeout(self.directory_timeout, "重命名 OneDrive 文件或目录", work).await
    }
}

/// OneDrive 的「测试连接」探针。
///
/// **本函数住在 provider 这一侧**（`remote/connection_test.rs` 归别的 worker），
/// 由 `connection_test::probe_inner` 调用：
/// `RemoteProtocol::OnedrivePan => super::onedrive_provider::probe_onedrive(target).await`。
///
/// 凭据是**一份 OAuth 会话**里的访问凭证（`access_token`），不是口令 ——
/// 设置页在 OneDrive 协议下那个输入框装的就是访问凭证；
/// 「从存档还原会话」那条路（`commands.rs` 的 `remote_test_connection`）
/// 交过来的也是同一段文本。
///
/// 没有凭证时报「还没授权」而不是「连接失败」：两者的处置完全不同 ——
/// ⚠️ 这里**不能**报 `AuthRejected`（与百度 / 夸克 / 阿里云同一条理由：
/// 「没拿到凭据」与「服务器拒了这组凭据」是两件事）。
pub async fn probe_onedrive(target: &ProbeTarget) -> Result<ConnectionTestReport, RemoteError> {
    let Some(raw) = target.password.as_deref().map(str::trim) else {
        return Ok(ConnectionTestReport {
            outcome: ProbeOutcome::MissingCredential,
            server_identity: None,
            password_checked: false,
            tls_verified: false,
            message: "这条 OneDrive 连接还没有登录凭据，请先完成授权。".to_string(),
        });
    };

    if raw.is_empty() {
        return Ok(ConnectionTestReport {
            outcome: ProbeOutcome::MissingCredential,
            server_identity: None,
            password_checked: false,
            tls_verified: false,
            message: "这条 OneDrive 连接还没有登录凭据，请先完成授权。".to_string(),
        });
    }

    match onedrive_fetch_drive_info(raw).await {
        Ok(info) => {
            let account = info
                .account_name()
                .unwrap_or_else(|| "账号未知".to_string());

            Ok(ConnectionTestReport {
                outcome: ProbeOutcome::Verified,
                server_identity: None,
                password_checked: true,
                tls_verified: false,
                message: format!("OneDrive 连接正常，账号：{account}"),
            })
        }
        Err(error) => Ok(ConnectionTestReport {
            outcome: onedrive_probe_outcome(error.kind),
            server_identity: None,
            password_checked: true,
            tls_verified: false,
            message: error.message,
        }),
    }
}

/// 错误类别 → 探针结论。
///
/// `connection_test.rs` 里那份映射是**模块私有**的，这里自己写一份：
/// 与其为了复用一行映射反向依赖到别人的私有 API，不如在这里明写这几行。
/// 语义与那边完全一致。
fn onedrive_probe_outcome(kind: RemoteErrorKind) -> ProbeOutcome {
    match kind {
        RemoteErrorKind::Auth => ProbeOutcome::AuthRejected,
        RemoteErrorKind::Unreachable | RemoteErrorKind::Disconnected => ProbeOutcome::Unreachable,
        RemoteErrorKind::Protocol => ProbeOutcome::ProtocolError,
        RemoteErrorKind::Permission => ProbeOutcome::AuthRejected,
        _ => ProbeOutcome::ProtocolError,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    const CONNECTION_ID: &str = "conn-onedrive";

    fn session_with(access_token: &str, refresh_token: &str, expires_at: i64) -> OnedriveAuthSession {
        OnedriveAuthSession {
            version: super::super::onedrive_auth::ONEDRIVE_SESSION_VERSION,
            client_id: "app-client-id".to_string(),
            access_token: access_token.to_string(),
            refresh_token: refresh_token.to_string(),
            expires_at,
            account: Some("someone@example.com".to_string()),
            drive_id: Some("drive-1".to_string()),
        }
    }

    /// 假会话存储：记录每一次 `save` 的键。
    #[derive(Default)]
    struct FakeSessionStore {
        sessions: Mutex<HashMap<String, OnedriveAuthSession>>,
        saved_keys: Mutex<Vec<String>>,
    }

    impl FakeSessionStore {
        fn with(key: &str, session: OnedriveAuthSession) -> Self {
            let mut sessions = HashMap::new();
            sessions.insert(key.to_string(), session);

            Self {
                sessions: Mutex::new(sessions),
                saved_keys: Mutex::new(Vec::new()),
            }
        }

        fn saved_keys(&self) -> Vec<String> {
            self.saved_keys.lock().expect("lock").clone()
        }
    }

    impl OnedriveSessionStore for FakeSessionStore {
        fn load(&self, key: &str) -> Result<Option<OnedriveAuthSession>, RemoteError> {
            Ok(self.sessions.lock().expect("lock").get(key).cloned())
        }

        fn save(&self, key: &str, session: &OnedriveAuthSession) -> Result<(), RemoteError> {
            self.sessions
                .lock()
                .expect("lock")
                .insert(key.to_string(), session.clone());
            self.saved_keys.lock().expect("lock").push(key.to_string());
            Ok(())
        }

        fn delete(&self, key: &str) -> Result<(), RemoteError> {
            self.sessions.lock().expect("lock").remove(key);
            Ok(())
        }
    }

    /// 假 Graph 接口：记录被请求过的直链与 `Range`。
    struct FakeApi {
        first_page: OnedriveChildrenPage,
        content_url: String,
        downloaded_url: Mutex<Option<String>>,
        downloaded_range: Mutex<Option<String>>,
        refreshed: Mutex<Option<OnedriveAuthSession>>,
        refresh_calls: Mutex<usize>,
        uploaded_items: Mutex<Vec<(String, Vec<u8>)>>,
        created_folders: Mutex<Vec<String>>,
        renamed_items: Mutex<Vec<(String, String)>>,
        deleted_items: Mutex<Vec<String>>,
    }

    impl FakeApi {
        fn new() -> Self {
            Self {
                first_page: OnedriveChildrenPage {
                    value: vec![OnedriveGraphItem {
                        id: "item-1".to_string(),
                        name: "readme.txt".to_string(),
                        size: Some(12),
                        last_modified_date_time: Some("2024-01-15T10:30:00Z".to_string()),
                        folder: None,
                        file: Some(super::super::onedrive_api::OnedriveGraphFile {
                            mime_type: Some("text/plain".to_string()),
                        }),
                        parent_reference: None,
                    }],
                    next_link: None,
                },
                content_url: "https://download.example/presigned?sig=abc".to_string(),
                downloaded_url: Mutex::new(None),
                downloaded_range: Mutex::new(None),
                refreshed: Mutex::new(None),
                refresh_calls: Mutex::new(0),
                uploaded_items: Mutex::new(Vec::new()),
                created_folders: Mutex::new(Vec::new()),
                renamed_items: Mutex::new(Vec::new()),
                deleted_items: Mutex::new(Vec::new()),
            }
        }

        fn with_refresh(mut self, refreshed: OnedriveAuthSession) -> Self {
            self.refreshed = Mutex::new(Some(refreshed));
            self
        }

        /// 上传过的文件（路径 + 内容）。
        fn uploaded(&self) -> Vec<(String, Vec<u8>)> {
            self.uploaded_items.lock().expect("lock").clone()
        }

        /// 新建过的目录。
        fn created_folders(&self) -> Vec<String> {
            self.created_folders.lock().expect("lock").clone()
        }

        /// 改过的名（路径 + 新名字）。
        fn renamed_items(&self) -> Vec<(String, String)> {
            self.renamed_items.lock().expect("lock").clone()
        }

        /// 删掉过的路径。
        fn deleted_items(&self) -> Vec<String> {
            self.deleted_items.lock().expect("lock").clone()
        }
    }

    #[async_trait]
    impl OnedriveApi for FakeApi {
        async fn list_children_page(
            &self,
            _session: &OnedriveAuthSession,
            _remote_path: &str,
            _skip_token: Option<&str>,
        ) -> Result<OnedriveChildrenPage, RemoteError> {
            Ok(self.first_page.clone())
        }

        async fn content_location(
            &self,
            _session: &OnedriveAuthSession,
            _remote_path: &str,
        ) -> Result<OnedriveContentTarget, RemoteError> {
            Ok(OnedriveContentTarget::PreauthenticatedUrl(
                self.content_url.clone(),
            ))
        }

        async fn download_preauthenticated(
            &self,
            url: &str,
            range: Option<OnedriveByteRange>,
        ) -> Result<Vec<u8>, RemoteError> {
            *self.downloaded_url.lock().expect("lock") = Some(url.to_string());
            *self.downloaded_range.lock().expect("lock") =
                range.map(|range| range.to_header_value());

            Ok(b"hello onedrive".to_vec())
        }

        async fn refresh_session(
            &self,
            _session: &OnedriveAuthSession,
        ) -> Result<OnedriveAuthSession, RemoteError> {
            *self.refresh_calls.lock().expect("lock") += 1;

            self.refreshed
                .lock()
                .expect("lock")
                .clone()
                .ok_or_else(|| RemoteError::auth("测试夹具里没有配置刷新结果"))
        }

        async fn drive_info(
            &self,
            _session: &OnedriveAuthSession,
        ) -> Result<OnedriveDriveInfo, RemoteError> {
            Ok(OnedriveDriveInfo::default())
        }

        async fn item_by_path(
            &self,
            _session: &OnedriveAuthSession,
            remote_path: &str,
        ) -> Result<OnedriveGraphItem, RemoteError> {
            let name = remote_path
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or(remote_path)
                .to_string();

            Ok(OnedriveGraphItem {
                id: format!("id::{remote_path}"),
                name,
                ..Default::default()
            })
        }

        async fn upload_content(
            &self,
            _session: &OnedriveAuthSession,
            remote_path: &str,
            content: &[u8],
            on_progress: &(dyn Fn(u64) + Send + Sync),
        ) -> Result<(), RemoteError> {
            self.uploaded_items
                .lock()
                .expect("lock")
                .push((remote_path.to_string(), content.to_vec()));

            on_progress(content.len() as u64);

            Ok(())
        }

        async fn create_folder(
            &self,
            _session: &OnedriveAuthSession,
            remote_path: &str,
        ) -> Result<(), RemoteError> {
            self.created_folders
                .lock()
                .expect("lock")
                .push(remote_path.to_string());

            Ok(())
        }

        async fn rename_item(
            &self,
            _session: &OnedriveAuthSession,
            remote_path: &str,
            new_name: &str,
        ) -> Result<(), RemoteError> {
            self.renamed_items
                .lock()
                .expect("lock")
                .push((remote_path.to_string(), new_name.to_string()));

            Ok(())
        }

        async fn delete_item(
            &self,
            _session: &OnedriveAuthSession,
            remote_path: &str,
        ) -> Result<(), RemoteError> {
            self.deleted_items
                .lock()
                .expect("lock")
                .push(remote_path.to_string());

            Ok(())
        }
    }

    /// 建一个「连接表 + 会话存档」的夹具。
    fn fixture(session: OnedriveAuthSession) -> (tempfile::TempDir, PathBuf, FakeSessionStore) {
        let dir = tempfile::tempdir().expect("temp dir");
        let user_data_dir = dir.path().join("user-data");
        std::fs::create_dir_all(&user_data_dir).expect("mkdir");

        let connection = super::super::connection::RemoteConnection {
            id: CONNECTION_ID.to_string(),
            name: "OneDrive".to_string(),
            protocol: super::super::connection::RemoteProtocol::OnedrivePan,
            host: "graph.microsoft.com".to_string(),
            port: 443,
            username: String::new(),
            initial_path: "/".to_string(),
            local_path: None,
            proxy: super::super::transport::ProxyConfig::Direct,
            credential_key: Some(CONNECTION_ID.to_string()),
            include_in_global_search: false,
        };

        let file = registry::ConnectionsFile {
            connections: vec![connection],
        };

        let path = registry::connections_path_in(&user_data_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(&path, serde_json::to_string(&file).expect("serialize")).expect("write");

        let store = FakeSessionStore::with(CONNECTION_ID, session);

        (dir, user_data_dir, store)
    }

    fn provider(
        user_data_dir: &Path,
        api: FakeApi,
        store: &Arc<FakeSessionStore>,
    ) -> OnedriveRemoteProvider {
        OnedriveRemoteProvider::with_parts(
            user_data_dir,
            Arc::new(api),
            Arc::new(FakeSessionStore {
                sessions: Mutex::new(store.sessions.lock().expect("lock").clone()),
                saved_keys: Mutex::new(Vec::new()),
            }),
        )
    }

    /// 读文件时，**最终下载 URL 必须是 `/content` 给的那根直链**，
    /// 且 `Range` 落在**那根直链**上（不是 Graph 那一跳）。
    #[tokio::test]
    async fn reading_a_file_downloads_from_the_preauthenticated_url_with_a_range() {
        let session = session_with("access-token", "refresh-token", onedrive_now_unix() + 3_600);
        let (_dir, user_data_dir, store) = fixture(session);

        let api = Arc::new(FakeApi::new());
        let store = Arc::new(store);
        let provider = OnedriveRemoteProvider::with_parts(
            &user_data_dir,
            api.clone(),
            store.clone(),
        );

        let bytes = provider
            .read_file(CONNECTION_ID, "/readme.txt")
            .await
            .expect("read must succeed");

        assert_eq!(bytes, b"hello onedrive".to_vec());
        assert_eq!(
            api.downloaded_url.lock().expect("lock").as_deref(),
            Some("https://download.example/presigned?sig=abc"),
            "下载必须打在 `/content` 给出的最终直链上"
        );
        assert_eq!(
            api.downloaded_range.lock().expect("lock").as_deref(),
            Some("bytes=0-"),
            "`Range` 必须落在最终直链上"
        );
    }

    /// 过期会话 ⇒ 先刷新一次，**并且立刻把新会话写回存档**。
    ///
    /// 不写回的话，微软的滚动 refresh token 会在 90 天后突然失效。
    #[tokio::test]
    async fn an_expired_session_is_refreshed_and_written_back_immediately() {
        let expired = session_with("stale-token", "refresh-token", onedrive_now_unix() - 10);
        let (_dir, user_data_dir, store) = fixture(expired);

        let refreshed = session_with("fresh-token", "new-refresh-token", onedrive_now_unix() + 3_600);
        let api = Arc::new(FakeApi::new().with_refresh(refreshed.clone()));
        let store = Arc::new(store);
        let provider = OnedriveRemoteProvider::with_parts(
            &user_data_dir,
            api.clone(),
            store.clone(),
        );

        let entries = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("list must succeed");

        assert_eq!(entries.len(), 1);
        assert_eq!(
            api.refresh_calls.lock().expect("lock").clone(),
            1,
            "过期会话必须先刷新一次"
        );
        assert_eq!(
            store.saved_keys(),
            vec![CONNECTION_ID.to_string()],
            "刷新后的会话必须立刻写回存档"
        );

        let stored = store
            .load(CONNECTION_ID)
            .expect("load")
            .expect("session must exist");
        assert_eq!(stored.access_token, "fresh-token");
        assert_eq!(stored.refresh_token, "new-refresh-token");
    }

    /// 写路径现在**真的能写**（批次 40：OneDrive 从只读升级为读写）。
    /// 只有「改权限」仍是 Unsupported —— Graph 没有 POSIX 权限位，
    /// 假装支持比明说不支持更糟。
    #[tokio::test]
    async fn file_operations_work_but_permissions_stay_unsupported() {
        let session = session_with("access-token", "refresh-token", onedrive_now_unix() + 3_600);
        let (_dir, user_data_dir, store) = fixture(session);
        let api = Arc::new(FakeApi::new());
        let store = Arc::new(store);
        let provider =
            OnedriveRemoteProvider::with_parts(&user_data_dir, api.clone(), store.clone());

        provider
            .write_file(CONNECTION_ID, "/docs/report.bin", b"hello onedrive".to_vec())
            .await
            .expect("upload must succeed");
        provider
            .make_dir(CONNECTION_ID, "/docs/sub")
            .await
            .expect("mkdir must succeed");
        provider
            .rename_path(CONNECTION_ID, "/docs/report.bin", "renamed.bin")
            .await
            .expect("rename must succeed");
        provider
            .remove_path(CONNECTION_ID, "/docs/renamed.bin", false)
            .await
            .expect("remove must succeed");

        assert_eq!(
            api.uploaded(),
            vec![("/docs/report.bin".to_string(), b"hello onedrive".to_vec())],
            "上传的内容必须逐字节送到目标路径"
        );
        assert_eq!(
            api.created_folders(),
            vec!["/docs/sub".to_string()],
            "新建目录必须落在目标路径"
        );
        assert_eq!(
            api.renamed_items(),
            vec![("/docs/report.bin".to_string(), "renamed.bin".to_string())],
            "改名只带新名字，不带路径"
        );
        assert_eq!(
            api.deleted_items(),
            vec!["/docs/renamed.bin".to_string()],
            "删除必须打在目标路径上"
        );

        let chmod = provider
            .set_permissions(CONNECTION_ID, "/docs/report.bin", 0o644)
            .await
            .expect_err("Graph 没有 POSIX 权限位，改权限必须明确说不支持");
        assert_eq!(chmod.kind, RemoteErrorKind::Protocol);
    }

    /// 没有会话 ⇒ `Auth`（用户的处置只有「重新登录」一个动作）。
    #[tokio::test]
    async fn a_connection_without_a_session_reports_an_auth_error() {
        let session = session_with("access-token", "refresh-token", onedrive_now_unix() + 3_600);
        let (_dir, user_data_dir, _store) = fixture(session);

        // 存档里没有这份会话。
        let empty = Arc::new(FakeSessionStore::default());
        let provider = OnedriveRemoteProvider::with_parts(
            &user_data_dir,
            Arc::new(FakeApi::new()),
            empty,
        );

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("list must fail");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    /// 列目录要把 Graph 的 driveItem 变成 `RemoteEntry`：
    /// 名字、路径、`entry_id` 与修改时间（Unix 毫秒）都要对。
    #[tokio::test]
    async fn listing_a_directory_produces_entries_with_ids_and_millisecond_times() {
        let session = session_with("access-token", "refresh-token", onedrive_now_unix() + 3_600);
        let (_dir, user_data_dir, store) = fixture(session);

        let provider = provider(&user_data_dir, FakeApi::new(), &Arc::new(store));

        let entries = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("list must succeed");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "readme.txt");
        assert_eq!(
            entries[0].path,
            "sfm://remote/conn-onedrive/readme.txt"
        );
        assert_eq!(entries[0].entry_id.as_deref(), Some("item-1"));
        assert!(!entries[0].is_dir);
        assert_eq!(entries[0].size, 12);
        assert_eq!(
            entries[0].modified_time, 1_705_314_600_000,
            "修改时间必须是 Unix 毫秒"
        );
    }

    // ---- 实网负向用例（默认门禁不跑） -------------------------------------

    /// 实网负向用例（api 层）：带一个**假** Bearer 令牌去打 Microsoft Graph，
    /// 证明「读路径」真的走到了 Graph，并且失败被正确分类成 `Auth`。
    ///
    /// **不需要真实 OneDrive 账号** —— 假令牌必然被 Graph 拒掉，
    /// 而「被拒掉的形态」正是要验的东西：HTTP 401 + `error.code`。
    ///
    /// 默认门禁不跑（`#[ignore]`）：它要访问 `graph.microsoft.com`。
    /// 显式跑法：
    /// `cargo test --lib -- --ignored --nocapture onedrive_live`
    #[tokio::test]
    #[ignore = "需要访问 graph.microsoft.com（实网负向用例）"]
    async fn onedrive_live_graph_rejects_invalid_token_as_auth_error() {
        let url = onedrive_build_children_url("/");
        let token = "invalid-access-token-for-negative-probe";

        println!("[onedrive_live] GET {url}（假令牌）");

        // 客户端配置**照抄**生产路径（`build_graph_client`），不自造。
        let client = super::build_graph_client().expect("graph client");

        let response = client
            .get(&url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {token}"),
            )
            .send()
            .await
            .expect("请求必须真的打到 graph.microsoft.com");

        let status = response.status();
        let body = super::read_body(response).await.expect("response body");

        // 真实状态码与响应体**先打印再断言**：判据不满足时，
        // 光看「401 != 实际值」是查不出原因的。
        println!("[onedrive_live] 真实状态码 = {}", status.as_u16());
        println!(
            "[onedrive_live] 响应体前 300 字 = {}",
            body.chars().take(300).collect::<String>()
        );

        assert_eq!(
            status.as_u16(),
            401,
            "假令牌必须被 Graph 以 401 拒掉；真实状态码与响应体已打印在上面"
        );

        // **只认 `error.code`** —— 不从 `error.message` 里猜。
        let code = super::super::onedrive_api::onedrive_extract_error_code(&body);
        println!("[onedrive_live] error.code = {code:?}");

        let kind = super::super::onedrive_api::onedrive_graph_error_kind(
            status.as_u16(),
            code.as_deref(),
        );
        assert_eq!(
            kind,
            RemoteErrorKind::Auth,
            "401 + error.code 必须被分类成 Auth（实际 {kind:?}）"
        );

        // 分类对了还不够：**给用户看的文案**也得有内容。
        let mapped = onedrive_map_graph_error(status.as_u16(), &body, None);
        println!("[onedrive_live] 映射后的文案 = {}", mapped.message);
        assert!(
            !mapped.message.trim().is_empty(),
            "给用户看的文案不能为空"
        );
    }

    /// 实网负向用例（provider 层，端到端）：把一份**假会话**写进真 DPAPI 存档，
    /// 让 provider 去列目录，断言失败被分类成 `Auth`。
    ///
    /// 验的是「读路径整条走通到 Graph」：会话能读出来、令牌没过期（**不触发刷新**）、
    /// 请求真的发出去、401 被翻译成 `Auth`。
    ///
    /// 绝不能出现两种「假绿」：返回 `Ok` 空列表，或 panic。
    #[tokio::test]
    #[ignore = "需要访问 graph.microsoft.com（实网负向用例）"]
    async fn onedrive_live_provider_read_dir_with_fake_session_fails_as_auth_error() {
        let fake_session = OnedriveAuthSession {
            version: super::super::onedrive_auth::ONEDRIVE_SESSION_VERSION,
            client_id: "11111111-2222-3333-4444-555555555555".to_string(),
            access_token: "invalid-access-token-for-negative-probe".to_string(),
            refresh_token: String::new(),
            // 还没过期 ⇒ **不触发刷新**：本次要验的是读路径，不是续期路径。
            expires_at: onedrive_now_unix() + 3_600,
            account: None,
            drive_id: None,
        };

        // 连接表（`credential_key` = CONNECTION_ID）由夹具写好。
        let (_dir, user_data_dir, _store) = fixture(fake_session.clone());

        // app_data_dir：真 DPAPI 会话存档。
        // `tempfile::tempdir()` 在 `std::env::temp_dir()` 下建一个唯一子目录，
        // 测试结束（TempDir 析构）自动删掉。
        let app_data_dir = tempfile::tempdir().expect("temp dir");

        let encoded = super::super::onedrive_auth::onedrive_encode_session(&fake_session)
            .expect("encode session");
        super::super::credentials::save_credential(
            app_data_dir.path(),
            CONNECTION_ID,
            &encoded,
        )
        .expect("save credential");

        let provider =
            OnedriveRemoteProvider::new(&user_data_dir, app_data_dir.path()).expect("provider");

        let error = match provider.read_dir(CONNECTION_ID, "/").await {
            Ok(entries) => panic!(
                "假令牌不可能列目录成功：却拿到了 {} 条（{:?}）",
                entries.len(),
                entries
                    .iter()
                    .map(|entry| entry.name.clone())
                    .collect::<Vec<_>>()
            ),
            Err(error) => error,
        };

        println!("[onedrive_live] provider 错误 kind = {:?}", error.kind);
        println!("[onedrive_live] provider 错误消息 = {}", error.message);

        assert_eq!(
            error.kind,
            RemoteErrorKind::Auth,
            "假令牌的读失败必须归 Auth（实际 {:?}，消息：{}）",
            error.kind,
            error.message
        );
    }

    // ---- 批次 39：测试连接复用已保存的授权会话 -------------------------------

    /// scope 必须含 offline_access（少了它微软不发 refresh token），
    /// **且**必须是 Files.ReadWrite —— 只读的 Files.Read 不再算数。
    #[test]
    fn the_requested_scope_carries_read_write_and_offline_access() {
        let scope = super::super::onedrive_auth::ONEDRIVE_SCOPE;

        assert!(
            scope.contains("Files.ReadWrite"),
            "scope 必须是读写权限，实际：{scope}"
        );
        assert!(
            scope.contains("offline_access"),
            "scope 必须含 offline_access，否则没有 refresh token，实际：{scope}"
        );
        assert!(
            !scope.contains("Files.Read "),
            "只读的 Files.Read 不能再出现在 scope 里，实际：{scope}"
        );
    }

    /// 刷新请求体**不得**带 scope：带了与当初不同的范围，微软回
    /// invalid_scope（AADSTS70011），把用户**已经登录好的会话**直接废掉。
    #[test]
    fn the_refresh_request_never_carries_a_scope() {
        let body =
            super::super::onedrive_auth::onedrive_refresh_request_body("client-id", "refresh-token");

        assert!(!body.contains("scope"), "刷新请求不能带 scope，实际：{body}");
        assert!(
            body.contains("grant_type=refresh_token"),
            "实际：{body}"
        );
        assert!(body.contains("client_id=client-id"), "实际：{body}");
    }

    /// 存档里有会话且**未过期** ⇒ 拿到那条 access token，
    /// 且**不写回存档**（探针是纯读操作，不该改变下次测试的输入）。
    #[tokio::test]
    async fn ensure_session_token_returns_the_stored_token_without_writing_back() {
        let session = session_with("stored-token", "refresh-token", onedrive_now_unix() + 3_600);
        let (_dir, user_data_dir, store) = fixture(session);
        let store = Arc::new(store);
        let provider = OnedriveRemoteProvider::with_parts(
            &user_data_dir,
            Arc::new(FakeApi::new()),
            store.clone(),
        );

        let token = provider
            .ensure_session_token(CONNECTION_ID)
            .await
            .expect("存档里有会话，必须拿得到令牌");

        assert_eq!(token, Some("stored-token".to_string()));
        assert_eq!(
            store.saved_keys(),
            Vec::<String>::new(),
            "探针不得改写已保存的会话"
        );
    }

    /// 存档里的会话**已过期** ⇒ 先续期再返回新令牌，**并写回**
    /// （微软的 refresh token 是 90 天滚动窗口，不写回第 91 天会突然失效）。
    #[tokio::test]
    async fn ensure_session_token_refreshes_an_expired_session_and_writes_it_back() {
        let expired = session_with("stale-token", "refresh-token", onedrive_now_unix() - 10);
        let (_dir, user_data_dir, store) = fixture(expired);

        let refreshed =
            session_with("fresh-token", "new-refresh-token", onedrive_now_unix() + 3_600);
        let api = Arc::new(FakeApi::new().with_refresh(refreshed));
        let store = Arc::new(store);
        let provider =
            OnedriveRemoteProvider::with_parts(&user_data_dir, api.clone(), store.clone());

        let token = provider
            .ensure_session_token(CONNECTION_ID)
            .await
            .expect("过期会话必须先续期再给令牌");

        assert_eq!(token, Some("fresh-token".to_string()));
        assert_eq!(
            api.refresh_calls.lock().expect("lock").clone(),
            1,
            "过期会话必须先刷新一次"
        );
        assert_eq!(
            store.saved_keys(),
            vec![CONNECTION_ID.to_string()],
            "刷新后的会话必须立刻写回存档"
        );
    }

    /// 存档里**根本没有**会话（从未授权）⇒ Ok(None)：
    /// 调用方据此报 MissingCredential，**不得**报 AuthRejected。
    #[tokio::test]
    async fn ensure_session_token_reports_a_missing_session_as_none() {
        let session = session_with("stored-token", "refresh-token", onedrive_now_unix() + 3_600);
        let (_dir, user_data_dir, _store) = fixture(session);

        let empty = Arc::new(FakeSessionStore::default());
        let provider = OnedriveRemoteProvider::with_parts(
            &user_data_dir,
            Arc::new(FakeApi::new()),
            empty,
        );

        let token = provider
            .ensure_session_token("never-authorized")
            .await
            .expect("没有会话不是错误，是「没授权」");

        assert_eq!(token, None);
    }

    /// 续期失败 ⇒ 报错（调用方据此报 AuthRejected），
    /// **不能**退化成「没有凭据」—— 那会把用户引向错误的动作。
    #[tokio::test]
    async fn ensure_session_token_reports_a_failed_refresh_as_an_error() {
        let expired = session_with("stale-token", "refresh-token", onedrive_now_unix() - 10);
        let (_dir, user_data_dir, store) = fixture(expired);

        // FakeApi 没配置刷新结果 ⇒ 刷新回 Auth。
        let provider = OnedriveRemoteProvider::with_parts(
            &user_data_dir,
            Arc::new(FakeApi::new()),
            Arc::new(store),
        );

        let error = provider
            .ensure_session_token(CONNECTION_ID)
            .await
            .expect_err("续期失败必须报错，不能装作没有凭据");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
        assert!(
            error.message.contains("重新登录"),
            "文案必须把用户引向「重新登录」，实际：{}",
            error.message
        );
    }
}
