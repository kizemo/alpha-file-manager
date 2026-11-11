// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// ---------------------------------------------------------------------------
// 夸克网盘远程目录的读取（阶段 3 · T3 · 只读）
//
// 红线（与 `remote/mod.rs:8-14` 同一条）：本文件被 `cargo test --lib` 的测试
// 二进制链接，**不得出现 `tauri::AppHandle`、`tauri::async_runtime::*` 或
// `tauri::` 的任何路径**。一旦碰到，Tauri 全局运行时的默认类型参数 `Wry`
// 被单态化，把 tao 整条窗口链拖进测试二进制；测试二进制没有 application
// manifest 声明 comctl32 v6，`TaskDialogIndirect` 解析不到，进程在加载阶段
// 以 `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉 —— 一个测试都跑不了，
// 且报错与本模块毫无关系。
//
// 阶段 3 的能力边界：**能列目录、能读文件**；**其余一律不支持**。
// 上传（`write_file` / `write_file_with_progress`）、新建目录、删除、重命名、
// 改权限全部返回 `RemoteErrorKind::Unsupported`，绝不返回 `Ok(())` ——
// 静默成功会让用户以为文件已经传上去了，而它根本没有。
//
// 为什么网络访问要包一层 trait：`QuarkApi` 与 `QuarkSessionStore` 两个抽象
// 让本文件可以**零网络**单测 —— 假实现不需要真的连 `drive-pc.quark.cn`，
// 也不需要真的走 DPAPI 密文。
//
// ⚠️ **夸克的条目没有路径，只有 `fid`** —— 服务端不提供「按路径查条目」的
// 能力（百度那边条目自带 `path`）。所以「远端路径 → 目录 fid」只能从根
// （`fid = "0"`）一级一级往下走。详见 `QuarkRemoteProvider::resolve_dir_fid`。
//
// 本文件**不接线**：`remote/mod.rs` 的 `mod` 声明与 `pub use` 由 T5 统一补，
// 接线清单见文件末尾的「TODO(接线)」。
// ---------------------------------------------------------------------------

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;

use super::connection::RemoteConnection;
use super::error::{RemoteError, RemoteErrorKind};
use super::path::{
    normalize_remote_path, reject_path_control_characters, split_remote_parent_and_name,
    validate_remote_entry_name,
};
use super::provider::{RemoteEntry, RemoteProvider};
use super::quark_api::{
    classify_transport, encode_query_component, failure_from_http_status, next_page,
    parse_account_info_response, parse_download_response, parse_list_payload,
    quark_account_info_url, quark_download_request_body, quark_download_url,
    quark_entry_is_directory, quark_entry_name,
    quark_list_dir_url, to_remote_entries, to_remote_error, QuarkDownloadLink, QuarkFileEntry,
    QuarkListPage, QuarkSession, QuarkTransportShape, QuarkUserInfo, QUARK_CONNECT_TIMEOUT,
    QUARK_OPERATION_TIMEOUT, QUARK_USER_AGENT,
};
use super::quark_auth::{load_quark_session, session_to_cookie_header, QuarkSessionState};
use super::registry;

// 这三个常量**直接复用 `quark_api` 里的同一份**，不另写一个 120 秒出来。
// 两处各写一份的话，改一处忘另一处就会出现「单请求超时了但整体没超时」
// 这种查不出来的偏差。重新导出只是为了让 `mod.rs` 一处就能看到 provider
// 用到的全部常量。
pub use super::quark_api::{QUARK_DIRECTORY_TIMEOUT, QUARK_FILE_TIMEOUT, QUARK_MAX_DOWNLOAD_BYTES};

/// 夸克网盘的**根目录 fid**。
///
/// 服务端把根目录的 `pdir_fid` 记成 `"0"`，列根目录就是拿 `"0"` 去列。
pub const QUARK_ROOT_FID: &str = "0";

/// 单次列目录最多翻多少页。
///
/// 200 页 × 100 条 = 2 万个条目，远超任何真实目录。设上限是为了防住
/// 「服务端一直返回满页」造成的无限循环 —— 没有上限的话，一个坏掉的响应
/// 会让界面永远转圈，而界面上看不出任何异常。
pub const QUARK_MAX_LIST_PAGES: u32 = 200;

/// 列表请求的 `device_model` 参数（一个**常量**设备标识）。
///
/// ⚠️ **T0 已实测**（`.superpowers/sdd/quark/t0-device-model.md`）：`device_model`
/// 取**常量**、取**本机主机名**、甚至**整个省略**，列表端点与下载端点都是
/// HTTP 200 / code 0 ⇒ 这个参数**不是必需的**。
///
/// 所以这里**不**取本机主机名 —— 照抄网页端形状会把主机名带出（属敏感项，
/// §6.4 要求不落盘、不进日志），而省掉它没有任何代价。
///
/// `pub` 是给 `quark_login` 的验活探针用的 —— 两处必须用**同一个**取值口径。
pub fn default_device_model() -> String {
    "windows-pc".to_string()
}

/// 列表请求的 `ut` 参数（真实客户端取自 `window.QUARK_WSGUID`）。
///
/// ⚠️ **来源未定**：客户端从渲染进程的 `window.QUARK_WSGUID` 取，我们没有那个
/// 进程。先从环境变量 `QUARK_WSGUID` 读，读不到就发空串。
///
/// ✅ **T0 已核对**（`t0-device-model.md` / `t0-subdir.md`）：**空 `ut` 不会被
/// 服务端拒**（列表与下载端点都是 200/code=0）⇒ 不必在授权阶段把 WSGUID
/// 一并存进会话存档。
///
/// `pub` 是给 `quark_login` 的验活探针用的 —— 两处必须用**同一个**取值口径。
pub fn default_ut() -> String {
    std::env::var("QUARK_WSGUID")
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// 夸克网盘侧的 HTTP 访问。**抽成 trait 是为了零网络单测。**
#[async_trait]
pub trait QuarkApi: Send + Sync {
    /// 列一页目录。`pdir_fid` 是**父目录**的 fid，根目录是 `"0"`。
    ///
    /// `page.page` 从 **1** 开始（夸克用页码，不是百度那种偏移量）。
    async fn list_page(
        &self,
        session: &QuarkSession,
        pdir_fid: &str,
        page: QuarkListPage,
    ) -> Result<Vec<QuarkFileEntry>, RemoteError>;

    /// 取一个文件的下载直链。
    async fn download_link(
        &self,
        session: &QuarkSession,
        fid: &str,
    ) -> Result<QuarkDownloadLink, RemoteError>;

    /// 下载直链指向的内容，最多接受 `max_bytes` 字节。
    async fn download(
        &self,
        session: &QuarkSession,
        link: &QuarkDownloadLink,
        max_bytes: u64,
    ) -> Result<Vec<u8>, RemoteError>;

    /// 取账号昵称（只用于展示「已授权：<昵称>」）。
    async fn user_info(&self, session: &QuarkSession) -> Result<QuarkUserInfo, RemoteError>;
}

/// HTTP 客户端建不起来时的占位实现。
///
/// 存在的理由：`provider_with_protocol` 的签名**不返回 `Result`**（它返回
/// `Box<dyn RemoteProvider>`），而建 HTTP 客户端是会失败的。为了早半秒报错
/// 去改签名、动七八个调用点，收益不抵成本。
///
/// 所以把初始化错误**推迟**到第一次真正用网络的时候再报出来 ——
/// 用户看到的仍是那句具体的错误（而不是一句笼统的「连接失败」），
/// 并且**绝不会**出现「静默成功」。
pub struct UnavailableQuarkApi {
    error: RemoteError,
}

impl UnavailableQuarkApi {
    /// 把一个初始化错误包起来，等第一次真用网络时再报出来。
    pub fn new(error: RemoteError) -> Self {
        Self { error }
    }
}

#[async_trait]
impl QuarkApi for UnavailableQuarkApi {
    async fn list_page(
        &self,
        _session: &QuarkSession,
        _pdir_fid: &str,
        _page: QuarkListPage,
    ) -> Result<Vec<QuarkFileEntry>, RemoteError> {
        Err(self.error.clone())
    }

    async fn download_link(
        &self,
        _session: &QuarkSession,
        _fid: &str,
    ) -> Result<QuarkDownloadLink, RemoteError> {
        Err(self.error.clone())
    }

    async fn download(
        &self,
        _session: &QuarkSession,
        _link: &QuarkDownloadLink,
        _max_bytes: u64,
    ) -> Result<Vec<u8>, RemoteError> {
        Err(self.error.clone())
    }

    async fn user_info(&self, _session: &QuarkSession) -> Result<QuarkUserInfo, RemoteError> {
        Err(self.error.clone())
    }
}

/// 会话存档的读取。**抽成 trait 是为了零 DPAPI 单测。**
pub trait QuarkSessionStore: Send + Sync {
    /// 按键读会话。没有就是 `Ok(None)`；存档坏了要**报错**，不能当成「没存」。
    fn load(&self, key: &str) -> Result<Option<QuarkSessionState>, RemoteError>;
}

/// 走既有 DPAPI 密文（`remote-credentials.json`）的会话存储。
pub struct DpapiQuarkSessionStore {
    app_data_dir: PathBuf,
}

impl DpapiQuarkSessionStore {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            app_data_dir: app_data_dir.to_path_buf(),
        }
    }
}

impl QuarkSessionStore for DpapiQuarkSessionStore {
    fn load(&self, key: &str) -> Result<Option<QuarkSessionState>, RemoteError> {
        load_quark_session(&self.app_data_dir, key)
    }
}

// ===========================================================================
// HTTP
// ===========================================================================

/// 夸克接口的「浏览器化」默认请求头。
///
/// ⚠️ **T0 实测**：探针请求带了 `Accept` / `Accept-Language` / `Origin` /
/// `Referer`，Rust 侧原本只发 `User-Agent` + `Cookie`。`Origin` / `Referer` 不是
/// 装饰 —— 它们是「网页端调用内部接口」的上下文，缺了会被当成非网页来源。
/// 这里让 Rust 侧**逐字对齐实测成功的请求形状**。
fn quark_api_headers() -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/json, text/plain, */*"),
    );
    headers.insert(
        reqwest::header::ACCEPT_LANGUAGE,
        reqwest::header::HeaderValue::from_static("zh-CN,zh;q=0.9"),
    );
    headers.insert(
        reqwest::header::HeaderName::from_static("origin"),
        reqwest::header::HeaderValue::from_static("https://pan.quark.cn"),
    );
    headers.insert(
        reqwest::header::REFERER,
        reqwest::header::HeaderValue::from_static("https://pan.quark.cn/"),
    );
    headers
}

/// 下载直链用的请求头。
///
/// **刻意不带 `Accept`**：直链指向的是 CDN 上的字节流，`Accept: application/json`
/// 可能让 CDN 走内容协商、返回元数据而不是文件本身。只留 `Referer`。
fn quark_download_headers() -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::REFERER,
        reqwest::header::HeaderValue::from_static("https://pan.quark.cn/"),
    );
    headers
}

/// 建一个专供夸克网盘用的 HTTP 客户端。
///
/// **不跟随重定向**：会话失效时夸克会 3xx 到登录页，我们要看见这个 3xx 才能
/// 报「请重新授权」；跟过去只会拿到一个登录页 HTML。
fn build_client() -> Result<reqwest::Client, RemoteError> {
    reqwest::Client::builder()
        .user_agent(QUARK_USER_AGENT)
        .default_headers(quark_api_headers())
        .timeout(QUARK_OPERATION_TIMEOUT)
        .connect_timeout(QUARK_CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| {
            RemoteError::unknown(format!("夸克网盘 HTTP 客户端初始化失败：{error}"))
        })
}

/// 下载用的 HTTP 客户端。
///
/// 与 [`build_client`] 的区别有两条，**两条都是下载直链逼出来的**：
///
/// 1. **跟随重定向**：网盘给出的直链会 3xx 到 CDN 域名，不跟就只拿到一个 3xx。
/// 2. **超时更长**：整份文件要进内存，30 秒的请求超时对大文件必然超时。
///    连接本身仍然只给 30 秒。
fn build_download_client() -> Result<reqwest::Client, RemoteError> {
    reqwest::Client::builder()
        .user_agent(QUARK_USER_AGENT)
        .default_headers(quark_download_headers())
        .timeout(QUARK_FILE_TIMEOUT)
        .connect_timeout(QUARK_CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|error| {
            RemoteError::unknown(format!("夸克网盘下载客户端初始化失败：{error}"))
        })
}

/// 把 `reqwest::Error` 摘成夸克侧的传输层形态。
fn transport_shape_of(error: &reqwest::Error) -> QuarkTransportShape {
    QuarkTransportShape {
        connect: error.is_connect(),
        timeout: error.is_timeout(),
        body: error.is_body(),
        request: error.is_request(),
    }
}

/// 把一次传输层失败翻译成本项目的错误。
fn transport_error(context: &str, error: &reqwest::Error) -> RemoteError {
    let kind = classify_transport(transport_shape_of(error));
    RemoteError::new(kind, format!("{context}：{error}"))
}

/// 发一个 GET 并把响应体读成 UTF-8 文本。
///
/// 响应体**不是**用 `reqwest` 的 `text()` 读的：那个方法被 `charset` feature
/// 门控，而本 crate 的 `reqwest` 是 `default-features = false`，没开这个 feature
/// ⇒ 用了就编不过。这里用 `bytes()` + `String::from_utf8`，顺带把「不是合法
/// UTF-8」变成一个可分类的 `Encoding` 错误。
async fn get_text(
    client: &reqwest::Client,
    url: &str,
    session: &QuarkSession,
) -> Result<String, RemoteError> {
    let cookie = session
        .cookie_header()
        .ok_or_else(|| RemoteError::auth("夸克网盘会话凭据缺失或格式非法，请重新授权。"))?;

    let response = client
        .get(url)
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .map_err(|error| transport_error("访问夸克网盘失败", &error))?;

    let status = response.status();
    if !status.is_success() {
        return Err(to_remote_error(&failure_from_http_status(status.as_u16())));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|error| transport_error("读取夸克网盘响应失败", &error))?;

    String::from_utf8(bytes.to_vec())
        .map_err(|_| RemoteError::encoding("夸克网盘返回的内容编码无法解析。"))
}

/// 发一个 **POST**（JSON 请求体）并把响应体读成 UTF-8 文本。
///
/// 存在的理由：夸克的下载直链接口**只收 POST**（GET → HTTP 405），而且
/// **基础参数必须放 query**、**`fid` 必须走请求体**。见
/// [`quark_download_url`] / [`quark_download_request_body`]。
///
/// 与 [`get_text`] 一样，响应体用 `bytes()` + `String::from_utf8` 读
/// （`text()` 被 `charset` feature 门控，本 crate 的 `reqwest` 没开那个 feature）。
async fn post_json_text(
    client: &reqwest::Client,
    url: &str,
    body: &str,
    session: &QuarkSession,
) -> Result<String, RemoteError> {
    let cookie = session
        .cookie_header()
        .ok_or_else(|| RemoteError::auth("夸克网盘会话凭据缺失或格式非法，请重新授权。"))?;

    let response = client
        .post(url)
        .header(reqwest::header::COOKIE, cookie)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .await
        .map_err(|error| transport_error("访问夸克网盘失败", &error))?;

    let status = response.status();
    if !status.is_success() {
        return Err(to_remote_error(&failure_from_http_status(status.as_u16())));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|error| transport_error("读取夸克网盘响应失败", &error))?;

    String::from_utf8(bytes.to_vec())
        .map_err(|_| RemoteError::encoding("夸克网盘返回的内容编码无法解析。"))
}

/// 拼出真正要请求的下载地址。
///
/// `auth_key` 是服务端单独给的第二个令牌，客户端把它作为 query 追加到
/// `download_url` 后面 —— 但**不能重复追加**（`download_url` 里可能已经带了）。
fn download_url_with_auth_key(link: &QuarkDownloadLink) -> String {
    let Some(auth_key) = link
        .auth_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
    else {
        return link.download_url.clone();
    };

    if link.download_url.contains("auth_key=") {
        return link.download_url.clone();
    }

    let separator = if link.download_url.contains('?') { '&' } else { '?' };
    format!(
        "{}{separator}auth_key={}",
        link.download_url,
        encode_query_component(auth_key)
    )
}

/// 下载一个直链指向的内容，最多接受 `max_bytes` 字节。
///
/// **边下边判**：超过上限立刻停，而不是先把它整份收进内存再发现超了。
async fn download_file(
    client: &reqwest::Client,
    session: &QuarkSession,
    link: &QuarkDownloadLink,
    max_bytes: u64,
) -> Result<Vec<u8>, RemoteError> {
    let cookie = session
        .cookie_header()
        .ok_or_else(|| RemoteError::auth("夸克网盘会话凭据缺失或格式非法，请重新授权。"))?;

    let url = download_url_with_auth_key(link);

    let response = client
        .get(&url)
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .map_err(|error| transport_error("下载夸克网盘文件失败", &error))?;

    let status = response.status();
    if !status.is_success() {
        return Err(to_remote_error(&failure_from_http_status(status.as_u16())));
    }

    let mut collected: Vec<u8> = Vec::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| transport_error("下载夸克网盘文件中断", &error))?;

        if collected.len() as u64 + chunk.len() as u64 > max_bytes {
            return Err(RemoteError::unsupported(format!(
                "文件超过 {} MiB，夸克网盘只读阶段不下载这么大的文件。",
                max_bytes / (1024 * 1024)
            )));
        }

        collected.extend_from_slice(&chunk);
    }

    Ok(collected)
}

/// 走真实 HTTP 的实现。
pub struct HttpQuarkApi {
    client: reqwest::Client,
    download_client: reqwest::Client,
    /// 列表请求的 `device_model` 参数（本机主机名）。
    device_model: String,
    /// 列表请求的 `ut` 参数。
    ut: String,
}

impl HttpQuarkApi {
    /// 生产实现：真 HTTP。
    pub fn new() -> Result<Self, RemoteError> {
        Self::with_client_params(default_device_model(), default_ut())
    }

    /// 显式给出 `device_model` / `ut`。**只给测试与接线用**。
    ///
    /// 这两个参数是列表请求的**必填项**（少了会被服务端拒），所以它们必须能被
    /// 显式注入，而不是只能从环境变量里猜。
    pub fn with_client_params(
        device_model: impl Into<String>,
        ut: impl Into<String>,
    ) -> Result<Self, RemoteError> {
        Ok(Self {
            client: build_client()?,
            download_client: build_download_client()?,
            device_model: device_model.into(),
            ut: ut.into(),
        })
    }
}

#[async_trait]
impl QuarkApi for HttpQuarkApi {
    async fn list_page(
        &self,
        session: &QuarkSession,
        pdir_fid: &str,
        page: QuarkListPage,
    ) -> Result<Vec<QuarkFileEntry>, RemoteError> {
        let url = quark_list_dir_url(
            pdir_fid,
            page.page,
            page.size,
            &self.device_model,
            &self.ut,
        );
        let body = get_text(&self.client, &url, session).await?;
        let payload = parse_list_payload(&body).map_err(|failure| to_remote_error(&failure))?;
        Ok(payload.entries)
    }

    async fn download_link(
        &self,
        session: &QuarkSession,
        fid: &str,
    ) -> Result<QuarkDownloadLink, RemoteError> {
        // ⚠️ 实测：下载直链接口**只收 POST**（GET → HTTP 405）；基础参数在
        // query、`fid` 在请求体。
        let url = quark_download_url(&self.device_model);
        let request_body = quark_download_request_body(fid);
        let body = post_json_text(&self.client, &url, &request_body, session).await?;
        parse_download_response(&body).map_err(|failure| to_remote_error(&failure))
    }

    async fn download(
        &self,
        session: &QuarkSession,
        link: &QuarkDownloadLink,
        max_bytes: u64,
    ) -> Result<Vec<u8>, RemoteError> {
        download_file(&self.download_client, session, link, max_bytes).await
    }

    async fn user_info(&self, session: &QuarkSession) -> Result<QuarkUserInfo, RemoteError> {
        let url = quark_account_info_url();
        let body = get_text(&self.client, &url, session).await?;
        parse_account_info_response(&body).map_err(|failure| to_remote_error(&failure))
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

/// 夸克网盘的远程目录 provider。
///
/// ## 凭据从哪来
///
/// 夸克网盘的连接在连接表里只有元数据（host / port / nickname），
/// **登录态（整罐 cookie，含 `__pus`）整份存在 DPAPI 密文里**，键就是连接的
/// `credential_key`。所以本 provider **不**走 `registry::resolve_connection` ——
/// 那条路会把密文当密码读出来，而夸克网盘要的不是「密码」，是「会话」。
///
/// ## 路径怎么变成 fid
///
/// 夸克的条目**没有路径**，服务端只有 `fid`。所以「远端路径 → 目录」只能从根
/// 一级一级往下走：每往下一级都要列出父目录、按名字找到那一项、拿它的 `fid`。
/// 这是接口形状决定的，不是实现偷懒。
pub struct QuarkRemoteProvider {
    user_data_dir: PathBuf,
    api: Arc<dyn QuarkApi>,
    sessions: Arc<dyn QuarkSessionStore>,
    directory_timeout: Duration,
    file_timeout: Duration,
}

impl QuarkRemoteProvider {
    /// 生产实现：真 HTTP + 真 DPAPI。
    pub fn new(user_data_dir: &Path, app_data_dir: &Path) -> Result<Self, RemoteError> {
        Ok(Self::with_parts(
            user_data_dir,
            Arc::new(HttpQuarkApi::new()?),
            Arc::new(DpapiQuarkSessionStore::new(app_data_dir)),
        ))
    }

    /// **不返回 `Result`** 的构造：HTTP 客户端建不起来时，把错误推迟到第一次调用。
    ///
    /// 给 [`crate::remote::commands::provider_with_protocol`] 用 —— 它的签名是
    /// `Box<dyn RemoteProvider>`，没有地方安放 `Err`。
    pub fn new_deferred(user_data_dir: &Path, app_data_dir: &Path) -> Self {
        let sessions: Arc<dyn QuarkSessionStore> =
            Arc::new(DpapiQuarkSessionStore::new(app_data_dir));

        match HttpQuarkApi::new() {
            Ok(api) => Self::with_parts(user_data_dir, Arc::new(api), sessions),
            Err(error) => Self::with_parts(
                user_data_dir,
                Arc::new(UnavailableQuarkApi::new(error)),
                sessions,
            ),
        }
    }

    /// 注入实现。**只给测试用** —— 生产路径走 [`Self::new`]。
    pub fn with_parts(
        user_data_dir: &Path,
        api: Arc<dyn QuarkApi>,
        sessions: Arc<dyn QuarkSessionStore>,
    ) -> Self {
        Self {
            user_data_dir: user_data_dir.to_path_buf(),
            api,
            sessions,
            directory_timeout: QUARK_DIRECTORY_TIMEOUT,
            file_timeout: QUARK_FILE_TIMEOUT,
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
    /// 让它们先走一遍网络，等于把「用户敲错了一个字符」变成一次 120 秒超时。
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

    /// 入参处理 + 取会话。
    ///
    /// 找不到会话一律报 `Auth` —— 用户的处置只有一个动作：**重新授权**。
    /// 报 `Unknown` 会让用户看到一句看不懂的话。
    fn prepare(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> Result<(QuarkSession, String), RemoteError> {
        let (connection, normalized) = self.prepare_path(connection_id, remote_path)?;

        let Some(key) = connection
            .credential_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
        else {
            return Err(RemoteError::auth(
                "这条夸克网盘连接还没有保存登录凭据，请先完成授权。",
            ));
        };

        let Some(state) = self.sessions.load(key)? else {
            return Err(RemoteError::auth(
                "夸克网盘还没有授权，请先在设置里完成授权。",
            ));
        };

        // 会话存档里是结构化的会话（`__pus` + 若干 cookie），
        // 而 HTTP 层要的是「一罐 cookie」⇒ 这里把两者接上。
        let jar = session_to_cookie_header(&state.session);
        let session = QuarkSession::new(jar);

        // 整罐里有任何一条格式非法 ⇒ 这罐 cookie 不可信，报「请重新授权」
        // 而不是发一个「少了几条」的头过去。
        if session.cookie_header().is_none() {
            return Err(RemoteError::auth(
                "夸克网盘会话凭据缺失或格式非法，请重新授权。",
            ));
        }

        Ok((session, normalized))
    }

    /// 把一条**远端路径**解析成夸克的 `pdir_fid`。
    ///
    /// 根目录恒为 [`QUARK_ROOT_FID`]。往下一级就要列出父目录、按名字找那一项、
    /// 拿它的 `fid` —— 夸克没有「按路径查目录」的能力，只能这样。
    ///
    /// 代价：深度为 N 的路径要 N 次列表请求。
    async fn resolve_dir_fid(
        &self,
        session: &QuarkSession,
        remote_path: &str,
    ) -> Result<String, RemoteError> {
        let mut fid = QUARK_ROOT_FID.to_string();

        for segment in remote_path.split('/').filter(|part| !part.is_empty()) {
            let Some(entry) = self.find_entry(session, &fid, segment).await? else {
                return Err(RemoteError::not_found(format!(
                    "夸克网盘里没有这个目录：{segment}"
                )));
            };

            if !quark_entry_is_directory(&entry) {
                return Err(RemoteError::protocol(format!(
                    "「{segment}」不是一个目录。"
                )));
            }

            fid = entry.fid.clone();
        }

        Ok(fid)
    }

    /// 在一个目录里翻页找同名条目。
    async fn find_entry(
        &self,
        session: &QuarkSession,
        pdir_fid: &str,
        name: &str,
    ) -> Result<Option<QuarkFileEntry>, RemoteError> {
        let mut page = QuarkListPage::first();
        let mut pages = 0u32;

        loop {
            pages += 1;
            if pages > QUARK_MAX_LIST_PAGES {
                return Err(RemoteError::protocol(format!(
                    "夸克网盘这个目录超过了 {QUARK_MAX_LIST_PAGES} 页，已放弃继续翻页。",
                )));
            }

            let raw = self.api.list_page(session, pdir_fid, page).await?;
            let received = raw.len();

            if let Some(found) = raw
                .iter()
                .find(|entry| quark_entry_name(entry).as_deref() == Some(name))
            {
                return Ok(Some(found.clone()));
            }

            let Some(next) = next_page(&page, received) else {
                return Ok(None);
            };
            page = next;
        }
    }

    /// 翻页列一个目录的全部条目。
    async fn list_raw(
        &self,
        session: &QuarkSession,
        pdir_fid: &str,
    ) -> Result<Vec<QuarkFileEntry>, RemoteError> {
        let mut page = QuarkListPage::first();
        let mut entries: Vec<QuarkFileEntry> = Vec::new();
        let mut pages = 0u32;

        loop {
            pages += 1;
            if pages > QUARK_MAX_LIST_PAGES {
                return Err(RemoteError::protocol(format!(
                    "夸克网盘这个目录超过了 {QUARK_MAX_LIST_PAGES} 页，已放弃继续翻页。",
                )));
            }

            let raw = self.api.list_page(session, pdir_fid, page).await?;
            let received = raw.len();

            entries.extend(raw);

            let Some(next) = next_page(&page, received) else {
                break;
            };
            page = next;
        }

        Ok(entries)
    }

    /// 取当前登录账号的昵称。
    ///
    /// 用来在授权卡上回填「已授权：<昵称>」。**失败不影响读目录** ——
    /// 它只是一个锦上添花的展示项。
    pub async fn account_nickname(&self, connection_id: &str) -> Result<String, RemoteError> {
        let (session, _) = self.prepare(connection_id, "/")?;
        let info = self.api.user_info(&session).await?;
        Ok(info.nickname)
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
impl RemoteProvider for QuarkRemoteProvider {
    async fn read_dir(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> Result<Vec<RemoteEntry>, RemoteError> {
        let (session, normalized) = self.prepare(connection_id, remote_path)?;

        let work = async {
            let pdir_fid = self.resolve_dir_fid(&session, &normalized).await?;
            let raw = self.list_raw(&session, &pdir_fid).await?;
            Ok::<Vec<RemoteEntry>, RemoteError>(to_remote_entries(
                connection_id,
                &normalized,
                &raw,
            ))
        };

        with_timeout(self.directory_timeout, "读取夸克网盘目录", work).await
    }

    async fn read_file(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> Result<Vec<u8>, RemoteError> {
        let (session, normalized) = self.prepare(connection_id, remote_path)?;
        let (parent, name) = split_remote_parent_and_name(&normalized)?;

        let work = async {
            let pdir_fid = self.resolve_dir_fid(&session, &parent).await?;

            let Some(entry) = self.find_entry(&session, &pdir_fid, &name).await? else {
                return Err(RemoteError::not_found(format!(
                    "夸克网盘里没有这个文件：{name}"
                )));
            };

            if quark_entry_is_directory(&entry) {
                return Err(RemoteError::protocol(format!(
                    "「{name}」是一个目录，不能当文件读取。"
                )));
            }

            if entry.fid.trim().is_empty() {
                return Err(RemoteError::unsupported(
                    "夸克网盘没有给出这个文件的服务端 id，无法读取。",
                ));
            }

            // 直链拿不到 ⇒ 这条路径在本阶段**读不了**。
            // 但「会话失效」不能被改写成「不支持」：那两件事的用户处置完全不同
            // （重新授权 vs 等后续阶段）。
            let link = match self.api.download_link(&session, &entry.fid).await {
                Ok(link) => link,
                Err(error) => {
                    return Err(match error.kind {
                        RemoteErrorKind::Protocol => RemoteError::unsupported(
                            "夸克网盘没有提供这个文件的下载直链，暂时读不了。",
                        ),
                        _ => error,
                    })
                }
            };

            self.api
                .download(&session, &link, QUARK_MAX_DOWNLOAD_BYTES)
                .await
        };

        with_timeout(self.file_timeout, "读取夸克网盘文件", work).await
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
            "夸克网盘的上传（把本地文件传上去）在只读阶段还不支持。",
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
            "夸克网盘的上传（把本地文件传上去）在只读阶段还不支持。",
        )
    }

    async fn make_dir(&self, connection_id: &str, remote_path: &str) -> Result<(), RemoteError> {
        let (_, normalized) = self.prepare_path(connection_id, remote_path)?;

        // 名字要单独校验：名字里带 `/` 会让一次「新建目录」变成「建到别处去」。
        let (_parent, name) = split_remote_parent_and_name(&normalized)?;
        validate_remote_entry_name(&name)?;

        Err(RemoteError::unsupported(
            "夸克网盘的新建目录在只读阶段还不支持。",
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
            "夸克网盘没有 POSIX 权限位，改权限暂不支持。",
        ))
    }

    async fn get_permissions(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> Result<Option<u32>, RemoteError> {
        // 夸克网盘没有权限位这回事 ⇒ 取不到。
        //
        // **不联网**：走一遍网络只为回答「取不到」是白花的代价。
        self.prepare_path(connection_id, remote_path)?;

        Ok(None)
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
            return Err(RemoteError::protocol("不能删除夸克网盘的根目录。"));
        }

        Err(RemoteError::unsupported(
            "夸克网盘的删除在只读阶段还不支持。",
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
            "夸克网盘的重命名在只读阶段还不支持。",
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
    use crate::remote::error::RemoteErrorKind;
    use crate::remote::quark_api::{failure_from_code, QUARK_LIST_PAGE_SIZE};
    use crate::remote::quark_auth::QuarkAuthSession;
    use crate::remote::registry::{ConnectionsFile, REMOTE_CONNECTIONS_FILE};
    use crate::remote::transport::ProxyConfig;
    use std::collections::{BTreeMap, HashMap};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    const CONNECTION_ID: &str = "conn-quark";
    const CREDENTIAL_KEY: &str = "conn-quark";
    const SECRET_PUS: &str = "fake-pus-value-0123456789";

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    // ------------------------------------------------------------------
    // 假实现
    // ------------------------------------------------------------------

    #[derive(Default)]
    struct FakeApi {
        /// 按 (父目录 fid, 页码) 预先摆好的一页。
        pages: Mutex<Vec<(String, u32, Vec<QuarkFileEntry>)>>,
        /// 记住被请求过的页，便于断言「翻了几页」「请求的是哪个目录」。
        calls: Mutex<Vec<(String, u32, u32)>>,
        /// 列表请求要返回的错误。
        list_error: Mutex<Option<RemoteError>>,
        /// 「服务端永远返回满页」—— 服务端坏掉时的形态，用来验翻页上限。
        runaway: Mutex<bool>,
        /// runaway 时返回的那一页。
        runaway_page: Mutex<Vec<QuarkFileEntry>>,
        /// fid → 下载直链。
        links: Mutex<HashMap<String, QuarkDownloadLink>>,
        /// 取直链要返回的错误。
        link_error: Mutex<Option<RemoteError>>,
        /// 直链 → 内容。
        downloads: Mutex<HashMap<String, Vec<u8>>>,
        /// 下载要返回的错误。
        download_error: Mutex<Option<RemoteError>>,
        /// 下载时收到的上限。
        download_limit: Mutex<u64>,
        /// 账号昵称。
        nickname: Mutex<Option<String>>,
        /// 账号信息要返回的错误。
        user_info_error: Mutex<Option<RemoteError>>,
    }

    impl FakeApi {
        fn set_page(&self, pdir_fid: &str, page: u32, entries: Vec<QuarkFileEntry>) {
            self.pages
                .lock()
                .expect("pages lock")
                .push((pdir_fid.to_string(), page, entries));
        }

        /// 让服务端「永远返回满页」—— 服务端坏掉时的形态。
        fn set_runaway(&self, entries: Vec<QuarkFileEntry>) {
            *self.runaway.lock().expect("runaway lock") = true;
            *self.runaway_page.lock().expect("runaway page lock") = entries;
        }

        /// 让列表请求按夸克的错误码表失败（模拟服务端回了某个 code）。
        fn set_list_failure_code(&self, code: i64) {
            *self.list_error.lock().expect("list error lock") =
                Some(to_remote_error(&failure_from_code(code)));
        }

        fn set_link(&self, fid: &str, url: &str) {
            self.links.lock().expect("links lock").insert(
                fid.to_string(),
                QuarkDownloadLink {
                    download_url: url.to_string(),
                    auth_key: None,
                    expires: None,
                },
            );
        }

        fn set_download(&self, url: &str, bytes: Vec<u8>) {
            self.downloads
                .lock()
                .expect("downloads lock")
                .insert(url.to_string(), bytes);
        }

        fn calls(&self) -> Vec<(String, u32, u32)> {
            self.calls.lock().expect("calls lock").clone()
        }

        fn download_limit(&self) -> u64 {
            *self.download_limit.lock().expect("download limit lock")
        }
    }

    #[async_trait]
    impl QuarkApi for FakeApi {
        async fn list_page(
            &self,
            _session: &QuarkSession,
            pdir_fid: &str,
            page: QuarkListPage,
        ) -> Result<Vec<QuarkFileEntry>, RemoteError> {
            self.calls
                .lock()
                .expect("calls lock")
                .push((pdir_fid.to_string(), page.page, page.size));

            if let Some(error) = self.list_error.lock().expect("list error lock").clone() {
                return Err(error);
            }

            if *self.runaway.lock().expect("runaway lock") {
                return Ok(self.runaway_page.lock().expect("runaway page lock").clone());
            }

            let pages = self.pages.lock().expect("pages lock");
            for (page_dir, page_number, entries) in pages.iter() {
                if page_dir == pdir_fid && *page_number == page.page {
                    return Ok(entries.clone());
                }
            }

            Ok(Vec::new())
        }

        async fn download_link(
            &self,
            _session: &QuarkSession,
            fid: &str,
        ) -> Result<QuarkDownloadLink, RemoteError> {
            if let Some(error) = self.link_error.lock().expect("link error lock").clone() {
                return Err(error);
            }

            self.links
                .lock()
                .expect("links lock")
                .get(fid)
                .cloned()
                .ok_or_else(|| RemoteError::unsupported("这个文件没有下载直链。"))
        }

        async fn download(
            &self,
            _session: &QuarkSession,
            link: &QuarkDownloadLink,
            max_bytes: u64,
        ) -> Result<Vec<u8>, RemoteError> {
            *self.download_limit.lock().expect("download limit lock") = max_bytes;

            if let Some(error) = self
                .download_error
                .lock()
                .expect("download error lock")
                .clone()
            {
                return Err(error);
            }

            self.downloads
                .lock()
                .expect("downloads lock")
                .get(&link.download_url)
                .cloned()
                .ok_or_else(|| RemoteError::not_found("没有这个直链。"))
        }

        async fn user_info(&self, _session: &QuarkSession) -> Result<QuarkUserInfo, RemoteError> {
            if let Some(error) = self
                .user_info_error
                .lock()
                .expect("user info error lock")
                .clone()
            {
                return Err(error);
            }

            Ok(QuarkUserInfo {
                nickname: self
                    .nickname
                    .lock()
                    .expect("nickname lock")
                    .clone()
                    .unwrap_or_else(|| "测试用户".to_string()),
            })
        }
    }

    #[derive(Default)]
    struct FakeSessionStore {
        states: Mutex<HashMap<String, QuarkSessionState>>,
        error: Mutex<Option<RemoteError>>,
    }

    impl FakeSessionStore {
        fn insert(&self, key: &str, state: QuarkSessionState) {
            self.states
                .lock()
                .expect("states lock")
                .insert(key.to_string(), state);
        }
    }

    impl QuarkSessionStore for FakeSessionStore {
        fn load(&self, key: &str) -> Result<Option<QuarkSessionState>, RemoteError> {
            if let Some(error) = self.error.lock().expect("session error lock").clone() {
                return Err(error);
            }

            Ok(self.states.lock().expect("states lock").get(key).cloned())
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

    fn session_state() -> QuarkSessionState {
        QuarkSessionState::new(
            QuarkAuthSession::new(SECRET_PUS, BTreeMap::new()),
            None,
        )
    }

    /// 一条夸克条目。`file_type` 为 `0` 是目录，其余是文件。
    fn file_entry(
        fid: &str,
        pdir_fid: &str,
        name: &str,
        file_type: i64,
        size: u64,
        updated_at: u64,
    ) -> QuarkFileEntry {
        QuarkFileEntry {
            fid: fid.to_string(),
            pdir_fid: pdir_fid.to_string(),
            file_name: name.to_string(),
            name: name.to_string(),
            size,
            file_type,
            format_type: 0,
            updated_at,
            thumbnail: None,
            download_url: None,
            share_fid_token: None,
            name_space: Some("/".to_string()),
        }
    }

    fn dir_entry(fid: &str, pdir_fid: &str, name: &str) -> QuarkFileEntry {
        file_entry(fid, pdir_fid, name, 0, 0, 1_700_000_000)
    }

    fn plain_file(fid: &str, pdir_fid: &str, name: &str, size: u64) -> QuarkFileEntry {
        file_entry(fid, pdir_fid, name, 1, size, 1_700_000_000)
    }

    /// 造满一页（`QUARK_LIST_PAGE_SIZE` 条）的条目。
    fn full_page(pdir_fid: &str, prefix: &str) -> Vec<QuarkFileEntry> {
        (0..QUARK_LIST_PAGE_SIZE)
            .map(|index| {
                plain_file(
                    &format!("{prefix}{index}"),
                    pdir_fid,
                    &format!("{prefix}{index}.txt"),
                    1,
                )
            })
            .collect()
    }

    fn fixture_with_credential(tag: &str, credential_key: Option<&str>) -> Fixture {
        let dir = std::env::temp_dir().join(format!(
            "afm-quark-{tag}-{}",
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");

        // 连接表里这条记录的协议就是 `quark_pan`：provider 按协议分派，
        // 协议写错的话，命令层会把它分派给 SFTP provider。
        let connections = ConnectionsFile {
            connections: vec![RemoteConnection {
                id: CONNECTION_ID.to_string(),
                name: "夸克网盘".to_string(),
                protocol: RemoteProtocol::QuarkPan,
                host: "pan.quark.cn".to_string(),
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
        fixture.sessions.insert(CREDENTIAL_KEY, session_state());
        fixture
    }

    fn provider(fixture: &Fixture) -> QuarkRemoteProvider {
        QuarkRemoteProvider::with_parts(&fixture.dir, fixture.api.clone(), fixture.sessions.clone())
    }

    // ------------------------------------------------------------------
    // 入参与会话
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn a_missing_connection_is_reported_as_not_found() {
        let fixture = fixture("no-connection");
        let provider = provider(&fixture);

        let error = provider
            .read_dir("conn-does-not-exist", "/")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::NotFound);
        assert!(
            fixture.api.calls().is_empty(),
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
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
        assert!(fixture.api.calls().is_empty(), "不该发出任何请求");
    }

    #[tokio::test]
    async fn a_path_with_dot_dot_segments_is_rejected_as_a_protocol_error() {
        let fixture = fixture("dot-dot");
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/pub/../etc")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
        assert!(fixture.api.calls().is_empty(), "不该发出任何请求");
    }

    #[tokio::test]
    async fn a_connection_without_a_credential_key_is_reported_as_auth() {
        let fixture = fixture_with_credential("no-key", None);
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[tokio::test]
    async fn a_connection_whose_session_is_missing_is_reported_as_auth() {
        let fixture = fixture_with_credential("no-session", Some(CREDENTIAL_KEY));
        // 夹具里 key 有，但存档是空的 ⇒ 读回来是 `None`。
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[tokio::test]
    async fn a_broken_session_archive_is_reported_as_the_underlying_error() {
        let fixture = fixture("broken-session");
        *fixture
            .sessions
            .error
            .lock()
            .expect("session error lock") = Some(RemoteError::unknown("会话存档坏了"));
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unknown);
        assert!(error.message.contains("会话存档坏了"));
    }

    #[tokio::test]
    async fn a_session_without_a_usable_cookie_is_reported_as_auth() {
        let fixture = fixture("empty-pus");
        fixture.sessions.insert(
            CREDENTIAL_KEY,
            QuarkSessionState::new(QuarkAuthSession::new("", BTreeMap::new()), None),
        );
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
        assert!(fixture.api.calls().is_empty(), "不该发出任何请求");
    }

    #[tokio::test]
    async fn the_session_store_reads_through_the_injected_implementation() {
        let store = FakeSessionStore::default();
        store.insert(CREDENTIAL_KEY, session_state());

        let state = store
            .load(CREDENTIAL_KEY)
            .expect("load must succeed")
            .expect("the session must be there");

        assert!(state.session.has_credentials());
        assert!(store.load("其它键").expect("load must succeed").is_none());
    }

    // ------------------------------------------------------------------
    // 列目录
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn an_empty_directory_lists_nothing() {
        let fixture = fixture("empty");
        let provider = provider(&fixture);

        let entries = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read root");

        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn lists_the_entries_of_the_root_directory() {
        let fixture = fixture("list");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![
                dir_entry("d1", QUARK_ROOT_FID, "01A_软件"),
                plain_file("f1", QUARK_ROOT_FID, "设计素材.zip", 2048),
            ],
        );
        let provider = provider(&fixture);

        let entries = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read root");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "01A_软件");
        assert!(entries[0].is_dir);
        assert_eq!(entries[1].name, "设计素材.zip");
        assert!(!entries[1].is_dir);
        assert_eq!(entries[1].size, 2048);
    }

    #[tokio::test]
    async fn the_root_directory_is_looked_up_by_its_root_fid() {
        let fixture = fixture("root-fid");
        let provider = provider(&fixture);

        provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read root");

        let calls = fixture.api.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, QUARK_ROOT_FID);
    }

    #[tokio::test]
    async fn an_empty_remote_path_is_normalized_to_the_root() {
        let fixture = fixture("empty-path");
        let provider = provider(&fixture);

        provider
            .read_dir(CONNECTION_ID, "")
            .await
            .expect("read root");

        assert_eq!(fixture.api.calls()[0].0, QUARK_ROOT_FID);
    }

    #[tokio::test]
    async fn the_first_page_is_requested_with_the_default_page_size() {
        let fixture = fixture("page-size");
        let provider = provider(&fixture);

        provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read root");

        let calls = fixture.api.calls();
        assert_eq!(calls[0].1, 1, "页码从 1 开始");
        assert_eq!(calls[0].2, QUARK_LIST_PAGE_SIZE);
    }

    #[tokio::test]
    async fn lists_the_entries_of_a_nested_directory() {
        let fixture = fixture("nested");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![dir_entry("d1", QUARK_ROOT_FID, "pub")],
        );
        fixture.api.set_page(
            "d1",
            1,
            vec![plain_file("f1", "d1", "a.txt", 12)],
        );
        let provider = provider(&fixture);

        let entries = provider
            .read_dir(CONNECTION_ID, "/pub")
            .await
            .expect("read nested");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "a.txt");
        assert_eq!(entries[0].path, "sfm://remote/conn-quark/pub/a.txt");
    }

    #[tokio::test]
    async fn a_nested_directory_is_reached_by_walking_down_one_level_at_a_time() {
        let fixture = fixture("walk");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![dir_entry("d1", QUARK_ROOT_FID, "pub")],
        );
        fixture.api.set_page(
            "d1",
            1,
            vec![dir_entry("d2", "d1", "docs")],
        );
        fixture.api.set_page("d2", 1, vec![]);
        let provider = provider(&fixture);

        provider
            .read_dir(CONNECTION_ID, "/pub/docs")
            .await
            .expect("read nested");

        let calls = fixture.api.calls();
        assert_eq!(
            calls,
            vec![
                (QUARK_ROOT_FID.to_string(), 1, QUARK_LIST_PAGE_SIZE),
                ("d1".to_string(), 1, QUARK_LIST_PAGE_SIZE),
                ("d2".to_string(), 1, QUARK_LIST_PAGE_SIZE),
            ]
        );
    }

    #[tokio::test]
    async fn a_missing_directory_is_reported_as_not_found() {
        let fixture = fixture("missing-dir");
        fixture.api.set_page(QUARK_ROOT_FID, 1, vec![]);
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/ghost")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::NotFound);
    }

    #[tokio::test]
    async fn a_file_where_a_directory_was_expected_is_reported_as_a_protocol_error() {
        let fixture = fixture("file-as-dir");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![plain_file("f1", QUARK_ROOT_FID, "notes.txt", 12)],
        );
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/notes.txt")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
    }

    #[tokio::test]
    async fn builds_sfm_remote_paths_without_percent_encoding() {
        let fixture = fixture("paths");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![dir_entry("d1", QUARK_ROOT_FID, "01A_软件")],
        );
        let provider = provider(&fixture);

        let entries = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read root");

        // 中文**原样**存放：不编码（与本项目本地路径的处理一致）。
        assert_eq!(entries[0].path, "sfm://remote/conn-quark/01A_软件");
        assert!(!entries[0].path.contains('%'));
    }

    #[tokio::test]
    async fn reports_modified_time_in_unix_milliseconds() {
        let fixture = fixture("mtime");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![file_entry(
                "f1",
                QUARK_ROOT_FID,
                "a.txt",
                1,
                0,
                1_700_000_000,
            )],
        );
        let provider = provider(&fixture);

        let entries = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read root");

        assert_eq!(entries[0].modified_time, 1_700_000_000_000);
    }

    #[tokio::test]
    async fn marks_dot_names_as_hidden() {
        let fixture = fixture("hidden");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![plain_file("f1", QUARK_ROOT_FID, ".gitignore", 3)],
        );
        let provider = provider(&fixture);

        let entries = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read root");

        assert!(entries[0].is_hidden);
        assert!(!entries[0].is_symlink);
    }

    #[tokio::test]
    async fn skips_entries_whose_name_contains_a_path_separator() {
        let fixture = fixture("separator");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![
                plain_file("f1", QUARK_ROOT_FID, "a/b.txt", 1),
                plain_file("f2", QUARK_ROOT_FID, "ok.txt", 1),
            ],
        );
        let provider = provider(&fixture);

        let entries = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read root");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "ok.txt");
    }

    #[tokio::test]
    async fn skips_entries_whose_name_cannot_be_read() {
        let fixture = fixture("no-name");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![
                file_entry("f1", QUARK_ROOT_FID, "", 1, 0, 1_700_000_000),
                plain_file("f2", QUARK_ROOT_FID, "ok.txt", 1),
            ],
        );
        let provider = provider(&fixture);

        let entries = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read root");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "ok.txt");
    }

    /// 判别性的一条：翻页必须**不重不漏**。
    ///
    /// 夸克的分页判据是「本页收到的条数 < size ⇒ 取完了」，所以一个正好整页的
    /// 目录会**多发一次**请求（那一页是空的）。这条测试把那一次空请求也算进去。
    #[tokio::test]
    async fn paging_through_three_pages_returns_every_entry_exactly_once() {
        let fixture = fixture("paging");
        fixture
            .api
            .set_page(QUARK_ROOT_FID, 1, full_page(QUARK_ROOT_FID, "a"));
        fixture
            .api
            .set_page(QUARK_ROOT_FID, 2, full_page(QUARK_ROOT_FID, "b"));
        fixture.api.set_page(
            QUARK_ROOT_FID,
            3,
            vec![
                plain_file("z1", QUARK_ROOT_FID, "z1.txt", 1),
                plain_file("z2", QUARK_ROOT_FID, "z2.txt", 1),
            ],
        );
        let provider = provider(&fixture);

        let entries = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read root");

        assert_eq!(entries.len(), 202);

        let mut names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 202, "一个条目都不许重复");
    }

    #[tokio::test]
    async fn a_directory_whose_last_page_is_empty_stops_paging() {
        let fixture = fixture("last-page-empty");
        fixture
            .api
            .set_page(QUARK_ROOT_FID, 1, full_page(QUARK_ROOT_FID, "a"));
        // 第 2 页是空的：整页目录会多发这一次请求。
        fixture.api.set_page(QUARK_ROOT_FID, 2, vec![]);
        let provider = provider(&fixture);

        let entries = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read root");

        assert_eq!(entries.len(), QUARK_LIST_PAGE_SIZE as usize);
        assert_eq!(fixture.api.calls().len(), 2);
    }

    #[tokio::test]
    async fn a_directory_that_never_runs_out_of_pages_is_abandoned() {
        let fixture = fixture("runaway");
        fixture.api.set_runaway(full_page(QUARK_ROOT_FID, "a"));
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
        assert_eq!(
            fixture.api.calls().len(),
            QUARK_MAX_LIST_PAGES as usize,
            "翻页必须在上限处停下来"
        );
    }

    #[tokio::test]
    async fn a_listing_error_is_passed_through_unchanged() {
        let fixture = fixture("list-error");
        *fixture.api.list_error.lock().expect("list error lock") =
            Some(RemoteError::disconnected("连接断了"));
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Disconnected);
    }

    /// 判别性的一条：`code = 31001`（请重新登录）必须落成 `Auth`。
    ///
    /// 「会话失效」被改写成「未知错误」是最糟的一种退化 —— 用户的处置只有
    /// 「重新授权」，报别的就找不着该干什么了。
    #[tokio::test]
    async fn an_expired_session_code_is_reported_as_auth() {
        let fixture = fixture("code-31001");
        fixture.api.set_list_failure_code(31001);
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[tokio::test]
    async fn a_permission_error_code_is_reported_as_permission() {
        let fixture = fixture("code-14014");
        fixture.api.set_list_failure_code(14014);
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Permission);
    }

    #[tokio::test]
    async fn a_not_found_error_code_is_reported_as_not_found() {
        let fixture = fixture("code-14013");
        fixture.api.set_list_failure_code(14013);
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::NotFound);
    }

    #[tokio::test]
    async fn an_unmapped_error_code_is_reported_as_unknown() {
        let fixture = fixture("code-unknown");
        fixture.api.set_list_failure_code(99999);
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unknown);
    }

    // ------------------------------------------------------------------
    // 读文件
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn reads_the_bytes_of_a_file() {
        let fixture = fixture("read");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![plain_file("f1", QUARK_ROOT_FID, "a.txt", 3)],
        );
        fixture.api.set_link("f1", "https://cdn.example/a.txt");
        fixture.api.set_download("https://cdn.example/a.txt", b"abc".to_vec());
        let provider = provider(&fixture);

        let bytes = provider
            .read_file(CONNECTION_ID, "/a.txt")
            .await
            .expect("read file");

        assert_eq!(bytes, b"abc".to_vec());
    }

    #[tokio::test]
    async fn reading_a_missing_file_is_reported_as_not_found() {
        let fixture = fixture("read-missing");
        fixture.api.set_page(QUARK_ROOT_FID, 1, vec![]);
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/ghost.txt")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::NotFound);
    }

    #[tokio::test]
    async fn reading_a_directory_as_a_file_is_reported_as_a_protocol_error() {
        let fixture = fixture("read-dir");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![dir_entry("d1", QUARK_ROOT_FID, "docs")],
        );
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/docs")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
    }

    #[tokio::test]
    async fn reading_a_file_without_a_server_id_is_reported_as_unsupported() {
        let fixture = fixture("read-no-id");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![plain_file("", QUARK_ROOT_FID, "a.txt", 3)],
        );
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/a.txt")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unsupported);
    }

    #[tokio::test]
    async fn a_missing_download_link_is_reported_as_unsupported() {
        let fixture = fixture("read-no-link");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![plain_file("f1", QUARK_ROOT_FID, "a.txt", 3)],
        );
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/a.txt")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unsupported);
    }

    #[tokio::test]
    async fn a_download_link_failure_is_passed_through_unchanged() {
        let fixture = fixture("read-link-error");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![plain_file("f1", QUARK_ROOT_FID, "a.txt", 3)],
        );
        *fixture.api.link_error.lock().expect("link error lock") =
            Some(RemoteError::auth("会话失效了"));
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/a.txt")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[tokio::test]
    async fn a_download_failure_is_passed_through_unchanged() {
        let fixture = fixture("read-download-error");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![plain_file("f1", QUARK_ROOT_FID, "a.txt", 3)],
        );
        fixture.api.set_link("f1", "https://cdn.example/a.txt");
        *fixture.api.download_error.lock().expect("download error lock") =
            Some(RemoteError::unreachable("连不上 CDN"));
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/a.txt")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unreachable);
    }

    #[tokio::test]
    async fn the_download_is_capped_at_the_maximum_file_size() {
        let fixture = fixture("read-limit");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![plain_file("f1", QUARK_ROOT_FID, "a.txt", 3)],
        );
        fixture.api.set_link("f1", "https://cdn.example/a.txt");
        fixture.api.set_download("https://cdn.example/a.txt", b"abc".to_vec());
        let provider = provider(&fixture);

        provider
            .read_file(CONNECTION_ID, "/a.txt")
            .await
            .expect("read file");

        assert_eq!(fixture.api.download_limit(), QUARK_MAX_DOWNLOAD_BYTES);
    }

    #[tokio::test]
    async fn a_download_that_exceeds_the_cap_is_reported_as_unsupported() {
        let fixture = fixture("read-too-big");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![plain_file("f1", QUARK_ROOT_FID, "a.txt", 3)],
        );
        fixture.api.set_link("f1", "https://cdn.example/a.txt");
        *fixture.api.download_error.lock().expect("download error lock") =
            Some(RemoteError::unsupported("文件超过 500 MiB。"));
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/a.txt")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unsupported);
    }

    #[tokio::test]
    async fn reading_a_file_in_a_missing_directory_is_reported_as_not_found() {
        let fixture = fixture("read-missing-parent");
        fixture.api.set_page(QUARK_ROOT_FID, 1, vec![]);
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/ghost/a.txt")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::NotFound);
    }

    #[tokio::test]
    async fn the_parent_directory_is_listed_before_the_file_is_downloaded() {
        let fixture = fixture("read-parent");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![plain_file("f1", QUARK_ROOT_FID, "a.txt", 3)],
        );
        fixture.api.set_link("f1", "https://cdn.example/a.txt");
        fixture.api.set_download("https://cdn.example/a.txt", b"abc".to_vec());
        let provider = provider(&fixture);

        provider
            .read_file(CONNECTION_ID, "/a.txt")
            .await
            .expect("read file");

        let calls = fixture.api.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, QUARK_ROOT_FID);
    }

    // ------------------------------------------------------------------
    // 不支持的操作
    // ------------------------------------------------------------------

    /// 判别性的一条：上传必须**明确报不支持**。
    ///
    /// 静默成功（`Ok(())`）会让用户以为文件已经传上去了，而它根本没有。
    #[tokio::test]
    async fn write_file_is_rejected_as_unsupported_instead_of_silently_succeeding() {
        let fixture = fixture("write");
        let provider = provider(&fixture);

        let error = provider
            .write_file(CONNECTION_ID, "/a.txt", b"content".to_vec())
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unsupported);
        assert!(fixture.api.calls().is_empty(), "不该发出任何请求");
    }

    /// 判别性的一条：带进度的上传同样要报不支持，**且一个字节都不许报进度**。
    #[tokio::test]
    async fn write_file_with_progress_is_rejected_as_unsupported_and_reports_no_progress() {
        let fixture = fixture("write-progress");
        let provider = provider(&fixture);

        let reported = AtomicUsize::new(0);
        let on_progress = |_written: u64| {
            reported.fetch_add(1, Ordering::SeqCst);
        };

        let error = provider
            .write_file_with_progress(CONNECTION_ID, "/a.txt", b"content".to_vec(), &on_progress)
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unsupported);
        assert_eq!(
            reported.load(Ordering::SeqCst),
            0,
            "一个字节都没写，不该有任何进度回调"
        );
        assert!(fixture.api.calls().is_empty(), "不该发出任何请求");
    }

    #[tokio::test]
    async fn make_dir_is_reported_as_unsupported() {
        let fixture = fixture("mkdir");
        let provider = provider(&fixture);

        let error = provider
            .make_dir(CONNECTION_ID, "/new-dir")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unsupported);
        assert!(fixture.api.calls().is_empty(), "不该发出任何请求");
    }

    #[tokio::test]
    async fn remove_path_is_reported_as_unsupported() {
        let fixture = fixture("remove");
        let provider = provider(&fixture);

        let error = provider
            .remove_path(CONNECTION_ID, "/a.txt", false)
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unsupported);
        assert!(fixture.api.calls().is_empty(), "不该发出任何请求");
    }

    #[tokio::test]
    async fn removing_the_root_directory_is_refused_even_before_the_unsupported_answer() {
        let fixture = fixture("remove-root");
        let provider = provider(&fixture);

        let error = provider
            .remove_path(CONNECTION_ID, "/", true)
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
    }

    #[tokio::test]
    async fn rename_path_is_reported_as_unsupported() {
        let fixture = fixture("rename");
        let provider = provider(&fixture);

        let error = provider
            .rename_path(CONNECTION_ID, "/a.txt", "b.txt")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unsupported);
        assert!(fixture.api.calls().is_empty(), "不该发出任何请求");
    }

    #[tokio::test]
    async fn renaming_to_an_invalid_entry_name_is_rejected_as_a_protocol_error() {
        let fixture = fixture("rename-bad-name");
        let provider = provider(&fixture);

        let error = provider
            .rename_path(CONNECTION_ID, "/a.txt", "b/c.txt")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
    }

    #[tokio::test]
    async fn set_permissions_is_reported_as_unsupported() {
        let fixture = fixture("chmod");
        let provider = provider(&fixture);

        let error = provider
            .set_permissions(CONNECTION_ID, "/a.txt", 0o755)
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unsupported);
        assert!(fixture.api.calls().is_empty(), "不该发出任何请求");
    }

    /// 判别性的一条：夸克没有权限位 ⇒ `Ok(None)`，而且**不联网**。
    #[tokio::test]
    async fn get_permissions_answers_none_without_touching_the_network() {
        let fixture = fixture("get-permissions");
        let provider = provider(&fixture);

        let mode = provider
            .get_permissions(CONNECTION_ID, "/a.txt")
            .await
            .expect("get permissions");

        assert_eq!(mode, None);
        assert!(fixture.api.calls().is_empty(), "不该发出任何请求");
    }

    // ------------------------------------------------------------------
    // 账号与构造
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn the_account_nickname_is_read_through_the_api() {
        let fixture = fixture("nickname");
        *fixture.api.nickname.lock().expect("nickname lock") = Some("老张".to_string());
        let provider = provider(&fixture);

        let nickname = provider
            .account_nickname(CONNECTION_ID)
            .await
            .expect("read nickname");

        assert_eq!(nickname, "老张");
    }

    #[tokio::test]
    async fn an_account_info_failure_is_passed_through_unchanged() {
        let fixture = fixture("nickname-error");
        *fixture.api.user_info_error.lock().expect("user info error lock") =
            Some(RemoteError::unreachable("连不上"));
        let provider = provider(&fixture);

        let error = provider
            .account_nickname(CONNECTION_ID)
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unreachable);
    }

    #[tokio::test]
    async fn the_account_nickname_of_an_unauthorized_connection_is_an_auth_error() {
        let fixture = fixture_with_credential("nickname-no-session", Some(CREDENTIAL_KEY));
        let provider = provider(&fixture);

        let error = provider
            .account_nickname(CONNECTION_ID)
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[tokio::test]
    async fn an_unavailable_api_reports_the_initialization_error_instead_of_silently_succeeding() {
        let fixture = fixture("unavailable");
        let provider = QuarkRemoteProvider::with_parts(
            &fixture.dir,
            Arc::new(UnavailableQuarkApi::new(RemoteError::unknown(
                "夸克网盘 HTTP 客户端初始化失败",
            ))),
            fixture.sessions.clone(),
        );

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unknown);
        assert!(error.message.contains("初始化失败"));
    }

    #[tokio::test]
    async fn the_provider_can_be_used_through_a_trait_object() {
        let fixture = fixture("trait-object");
        fixture.api.set_page(
            QUARK_ROOT_FID,
            1,
            vec![plain_file("f1", QUARK_ROOT_FID, "a.txt", 3)],
        );

        let provider: Box<dyn RemoteProvider> = Box::new(provider(&fixture));

        let entries = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read root");

        assert_eq!(entries.len(), 1);
    }

    #[tokio::test]
    async fn the_root_fid_is_zero() {
        assert_eq!(QUARK_ROOT_FID, "0");
    }

    #[tokio::test]
    async fn the_page_limit_is_a_positive_number() {
        assert!(QUARK_MAX_LIST_PAGES > 0);
    }
}

// ---------------------------------------------------------------------------
// 接线清单（由 T5 统一改 `remote/mod.rs`，本文件**不改**它）
//
// 1) `mod` 声明（放在 `mod quark_auth;` 之后）：
//        mod quark_provider;
//
// 2) `pub use`（形如百度侧 67-70 行那段）：
//        pub use quark_provider::{
//            DpapiQuarkSessionStore, HttpQuarkApi, QuarkApi, QuarkRemoteProvider,
//            QuarkSessionStore, UnavailableQuarkApi, QUARK_MAX_LIST_PAGES, QUARK_ROOT_FID,
//        };
//
//    `QUARK_DIRECTORY_TIMEOUT` / `QUARK_FILE_TIMEOUT` / `QUARK_MAX_DOWNLOAD_BYTES`
//    **不要**在这里再导出一遍 —— 它们已经在 `pub use quark_api::{…}` 里了，
//    重复导出会撞 E0252。
//
// 3) 命令层：`commands.rs` 的 `provider_with_protocol` 要认 `RemoteProtocol::QuarkPan`，
//    并调 `QuarkRemoteProvider::new_deferred(user_data_dir, app_data_dir)`。
// ---------------------------------------------------------------------------
