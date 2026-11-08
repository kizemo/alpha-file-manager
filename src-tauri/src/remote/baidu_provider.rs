// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// ---------------------------------------------------------------------------
// 百度网盘远程目录的读取（阶段 1 只读）
//
// 红线（与 `remote/mod.rs:8-18` 同一条）：本文件被 `cargo test --lib` 的测试
// 二进制链接，**不得出现 `tauri::AppHandle`、`tauri::async_runtime::*` 或
// `tauri::` 的任何路径**。一旦碰到，Tauri 全局运行时的默认类型参数 `Wry`
// 被单态化，把 tao 整条窗口链拖进测试二进制；测试二进制没有 application
// manifest 声明 comctl32 v6，`TaskDialogIndirect` 解析不到，进程在加载阶段
// 以 `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉 —— 一个测试都跑不了，
// 且报错与本模块毫无关系。
//
// 阶段 1 的能力边界：**能读、能新建目录、能改名、能删除**；**不能上传**。
// 上传（`write_file` / `write_file_with_progress`）与「改权限」一律返回
// `RemoteErrorKind::Unsupported`，绝不返回 `Ok(())` —— 静默成功会让用户
// 以为文件已经传上去了，而它根本没有。
//
// 为什么网络访问要包一层 trait：`BaiduApi` 与 `BaiduSessionStore` 两个抽象
// 让本文件可以**零网络**单测 —— 假实现不需要真的连 `pan.baidu.com`，
// 也不需要真的走 DPAPI 密文。
// ---------------------------------------------------------------------------

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;

use super::baidu_api::{
    build_client, build_download_client, create_dir_url, delete_filelist, download_file,
    encode_form_body, entry_name, fetch_bdstoken, fetch_dlink, filemanager_url, list_dir,
    next_page, parse_write_response, post_form, rename_filelist, to_remote_entries,
    to_remote_entry, BaiduFileEntry, BaiduSession, ListPage, BAIDU_BDSTOKEN_TTL,
    BAIDU_LIST_PAGE_LIMIT,
};
use super::baidu_auth::{load_baidu_session, BaiduSessionState};
use super::connection::RemoteConnection;
use super::error::{RemoteError, RemoteErrorKind};
use super::path::{
    join_remote_parent_and_name, normalize_remote_path, reject_path_control_characters,
    split_remote_parent_and_name, validate_remote_entry_name,
};
use super::provider::{RemoteEntry, RemoteProvider};
use super::registry;

/// 列目录的整体超时。
///
/// 比单次 HTTP 超时（30 秒）宽：一个大目录要翻很多页，每页各是一次请求。
/// 这一层的意义是「别把界面卡死」，不是「一次请求别太久」。
pub const BAIDU_DIRECTORY_TIMEOUT: Duration = Duration::from_secs(120);

/// 读一个文件的整体超时。
///
/// 文件要整份进内存（`RemoteProvider::read_file` 没有流式版本），
/// 500 MiB 走 30 秒的请求超时必然超时 —— 所以这一层要给得宽。
pub const BAIDU_FILE_TIMEOUT: Duration = Duration::from_secs(900);

/// 单次列目录最多翻多少页。
///
/// 200 页 × 1000 条 = 20 万个条目，远超任何真实目录。设上限是为了防住
/// 「服务端一直返回满页」造成的无限循环 —— 没有上限的话，一个坏掉的响应
/// 会让界面永远转圈，而界面上看不出任何异常。
pub const BAIDU_MAX_LIST_PAGES: u32 = 200;

/// 单个文件最大下载多少字节（500 MiB）。
///
/// 阶段 1 的 `read_file` 没有流式版本，整份文件要进内存。500 MiB 是一道
/// 人为的闸：超过就明确报「不支持」，而不是把机器的内存吃光。
pub const BAIDU_MAX_DOWNLOAD_BYTES: u64 = 500 * 1024 * 1024;

/// 百度网盘侧的 HTTP 访问。**抽成 trait 是为了零网络单测。**
#[async_trait]
pub trait BaiduApi: Send + Sync {
    /// 列一页目录。
    async fn list_page(
        &self,
        session: &BaiduSession,
        dir: &str,
        page: ListPage,
    ) -> Result<Vec<BaiduFileEntry>, RemoteError>;

    /// 取一个文件的下载直链。
    async fn file_dlink(&self, session: &BaiduSession, fs_id: u64) -> Result<String, RemoteError>;

    /// 下载直链指向的内容，最多接受 `max_bytes` 字节。
    async fn download(
        &self,
        session: &BaiduSession,
        url: &str,
        max_bytes: u64,
    ) -> Result<Vec<u8>, RemoteError>;

    /// 取写操作要用的 `bdstoken`。
    async fn bdstoken(&self, session: &BaiduSession) -> Result<String, RemoteError>;

    /// 在网盘里新建一个目录。`path` 是**目标目录的完整路径**（含目录名）。
    async fn create_dir(&self, session: &BaiduSession, path: &str) -> Result<(), RemoteError>;

    /// 改一个条目的名字。`path` 是条目的完整路径，`new_name` 只是名字。
    async fn rename_entry(
        &self,
        session: &BaiduSession,
        path: &str,
        new_name: &str,
    ) -> Result<(), RemoteError>;

    /// 删除一个条目。百度网盘的删除是「进回收站」，不是「立刻消失」。
    async fn remove_entry(&self, session: &BaiduSession, path: &str) -> Result<(), RemoteError>;
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
struct UnavailableBaiduApi {
    error: RemoteError,
}

#[async_trait]
impl BaiduApi for UnavailableBaiduApi {
    async fn list_page(
        &self,
        _session: &BaiduSession,
        _dir: &str,
        _page: ListPage,
    ) -> Result<Vec<BaiduFileEntry>, RemoteError> {
        Err(self.error.clone())
    }

    async fn file_dlink(
        &self,
        _session: &BaiduSession,
        _fs_id: u64,
    ) -> Result<String, RemoteError> {
        Err(self.error.clone())
    }

    async fn download(
        &self,
        _session: &BaiduSession,
        _url: &str,
        _max_bytes: u64,
    ) -> Result<Vec<u8>, RemoteError> {
        Err(self.error.clone())
    }

    async fn bdstoken(&self, _session: &BaiduSession) -> Result<String, RemoteError> {
        Err(self.error.clone())
    }

    async fn create_dir(&self, _session: &BaiduSession, _path: &str) -> Result<(), RemoteError> {
        Err(self.error.clone())
    }

    async fn rename_entry(
        &self,
        _session: &BaiduSession,
        _path: &str,
        _new_name: &str,
    ) -> Result<(), RemoteError> {
        Err(self.error.clone())
    }

    async fn remove_entry(&self, _session: &BaiduSession, _path: &str) -> Result<(), RemoteError> {
        Err(self.error.clone())
    }
}

/// 会话存档的读取。**抽成 trait 是为了零 DPAPI 单测。**
pub trait BaiduSessionStore: Send + Sync {
    /// 按键读会话。没有就是 `Ok(None)`；存档坏了要**报错**，不能当成「没存」。
    fn load(&self, key: &str) -> Result<Option<BaiduSessionState>, RemoteError>;
}

/// 缓存下来的 `bdstoken`。
#[derive(Debug, Clone)]
struct CachedBdstoken {
    token: String,
    fetched_at: Instant,
}

/// 写操作失败后要不要换个新 token 再试一次。
///
/// 只有「会话类失败」才值得重试：`bdstoken` 会过期，但参数错误不会因为
/// 换个 token 就变对 —— 那种情况重试只是把同一个错误再报一遍。
fn should_refresh_token(error: &RemoteError) -> bool {
    matches!(error.kind, RemoteErrorKind::Auth)
}

/// 走真实 HTTP 的实现。
pub struct HttpBaiduApi {
    client: reqwest::Client,
    download_client: reqwest::Client,
    /// 写操作要用的 `bdstoken`（缓存，避免每次写操作都重拉一次首页）。
    bdstoken: tokio::sync::Mutex<Option<CachedBdstoken>>,
}

impl HttpBaiduApi {
    pub fn new() -> Result<Self, RemoteError> {
        Ok(Self {
            client: build_client()?,
            download_client: build_download_client()?,
            bdstoken: tokio::sync::Mutex::new(None),
        })
    }

    /// 拿一个可用的 `bdstoken`：先看缓存，过期了才去首页取。
    async fn cached_bdstoken(&self, session: &BaiduSession) -> Result<String, RemoteError> {
        {
            let guard = self.bdstoken.lock().await;
            if let Some(cached) = guard.as_ref() {
                if cached.fetched_at.elapsed() < BAIDU_BDSTOKEN_TTL {
                    return Ok(cached.token.clone());
                }
            }
        }

        self.refresh_bdstoken(session).await
    }

    /// 强制重新取一个 `bdstoken`，并刷新缓存。
    async fn refresh_bdstoken(&self, session: &BaiduSession) -> Result<String, RemoteError> {
        let token = fetch_bdstoken(&self.client, session).await?;

        let mut guard = self.bdstoken.lock().await;
        *guard = Some(CachedBdstoken {
            token: token.clone(),
            fetched_at: Instant::now(),
        });

        Ok(token)
    }

    /// 丢掉缓存的 `bdstoken`（判定它过期之后）。
    async fn forget_bdstoken(&self) {
        let mut guard = self.bdstoken.lock().await;
        *guard = None;
    }

    /// 跑一次写操作：取 token → 执行 → 失败且像是 token 过期时换一个新 token 再试一次。
    ///
    /// **只重试一次**：会话真的失效时再试也是白试，那种情况要交给用户
    /// 「重新授权」，而不是在这里默默重试到超时。
    async fn run_write<F, Fut>(&self, session: &BaiduSession, op: F) -> Result<(), RemoteError>
    where
        F: Fn(String) -> Fut,
        Fut: Future<Output = Result<(), RemoteError>>,
    {
        for attempt in 0..2u32 {
            let token = if attempt == 0 {
                self.cached_bdstoken(session).await?
            } else {
                self.refresh_bdstoken(session).await?
            };

            match op(token).await {
                Ok(()) => return Ok(()),
                Err(error) => {
                    if attempt == 0 && should_refresh_token(&error) {
                        self.forget_bdstoken().await;
                        continue;
                    }
                    return Err(error);
                }
            }
        }

        Err(RemoteError::unknown("百度网盘写操作失败。"))
    }
}

#[async_trait]
impl BaiduApi for HttpBaiduApi {
    async fn list_page(
        &self,
        session: &BaiduSession,
        dir: &str,
        page: ListPage,
    ) -> Result<Vec<BaiduFileEntry>, RemoteError> {
        list_dir(&self.client, session, dir, page.start, page.limit).await
    }

    async fn file_dlink(&self, session: &BaiduSession, fs_id: u64) -> Result<String, RemoteError> {
        fetch_dlink(&self.client, session, fs_id).await
    }

    async fn download(
        &self,
        session: &BaiduSession,
        url: &str,
        max_bytes: u64,
    ) -> Result<Vec<u8>, RemoteError> {
        download_file(&self.download_client, session, url, max_bytes).await
    }

    async fn bdstoken(&self, session: &BaiduSession) -> Result<String, RemoteError> {
        self.cached_bdstoken(session).await
    }

    async fn create_dir(&self, session: &BaiduSession, path: &str) -> Result<(), RemoteError> {
        let path = Arc::new(path.to_string());

        self.run_write(session, move |token| {
            let path = Arc::clone(&path);
            async move {
                let url = create_dir_url(&token, path.as_str());
                let body = post_form(&self.client, session, &url, "").await?;
                parse_write_response(&body).map_err(|error| error.to_remote_error())
            }
        })
        .await
    }

    async fn rename_entry(
        &self,
        session: &BaiduSession,
        path: &str,
        new_name: &str,
    ) -> Result<(), RemoteError> {
        let path = Arc::new(path.to_string());
        let new_name = Arc::new(new_name.to_string());

        self.run_write(session, move |token| {
            let path = Arc::clone(&path);
            let new_name = Arc::clone(&new_name);
            async move {
                let url = filemanager_url(&token, "rename");
                let filelist = rename_filelist(path.as_str(), new_name.as_str());
                let body = encode_form_body(&[("filelist", filelist.as_str())]);
                let body = post_form(&self.client, session, &url, &body).await?;
                parse_write_response(&body).map_err(|error| error.to_remote_error())
            }
        })
        .await
    }

    async fn remove_entry(&self, session: &BaiduSession, path: &str) -> Result<(), RemoteError> {
        let path = Arc::new(path.to_string());

        self.run_write(session, move |token| {
            let path = Arc::clone(&path);
            async move {
                let url = filemanager_url(&token, "delete");
                let filelist = delete_filelist(path.as_str());
                let body = encode_form_body(&[("filelist", filelist.as_str())]);
                let body = post_form(&self.client, session, &url, &body).await?;
                parse_write_response(&body).map_err(|error| error.to_remote_error())
            }
        })
        .await
    }
}

/// 走既有 DPAPI 密文（`remote-credentials.json`）的会话存储。
pub struct DpapiBaiduSessionStore {
    app_data_dir: PathBuf,
}

impl DpapiBaiduSessionStore {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            app_data_dir: app_data_dir.to_path_buf(),
        }
    }
}

impl BaiduSessionStore for DpapiBaiduSessionStore {
    fn load(&self, key: &str) -> Result<Option<BaiduSessionState>, RemoteError> {
        load_baidu_session(&self.app_data_dir, key)
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

/// 百度网盘的远程目录 provider。
///
/// ## 凭据从哪来
///
/// 百度网盘的连接在连接表里只有元数据（host / port / nickname），
/// **登录态（`BDUSS` + `STOKEN`）整份存在 DPAPI 密文里**，键就是连接的
/// `credential_key`。所以本 provider **不**走 `registry::resolve_connection` ——
/// 那条路会把密文当密码读出来，而百度网盘要的不是「密码」，是「会话」。
pub struct BaiduRemoteProvider {
    user_data_dir: PathBuf,
    api: Arc<dyn BaiduApi>,
    sessions: Arc<dyn BaiduSessionStore>,
    directory_timeout: Duration,
    file_timeout: Duration,
}

impl BaiduRemoteProvider {
    /// 生产实现：真 HTTP + 真 DPAPI。
    pub fn new(user_data_dir: &Path, app_data_dir: &Path) -> Result<Self, RemoteError> {
        Ok(Self::with_parts(
            user_data_dir,
            Arc::new(HttpBaiduApi::new()?),
            Arc::new(DpapiBaiduSessionStore::new(app_data_dir)),
        ))
    }

    /// **不返回 `Result`** 的构造：HTTP 客户端建不起来时，把错误推迟到第一次调用。
    ///
    /// 给 [`crate::remote::commands::provider_with_protocol`] 用 —— 它的签名是
    /// `Box<dyn RemoteProvider>`，没有地方安放 `Err`。推迟报错之后，
    /// 用户看到的仍然是那句具体的错误，而不是笼统的「连接失败」。
    pub fn new_deferred(user_data_dir: &Path, app_data_dir: &Path) -> Self {
        let sessions: Arc<dyn BaiduSessionStore> =
            Arc::new(DpapiBaiduSessionStore::new(app_data_dir));

        match HttpBaiduApi::new() {
            Ok(api) => Self::with_parts(user_data_dir, Arc::new(api), sessions),
            Err(error) => Self::with_parts(
                user_data_dir,
                Arc::new(UnavailableBaiduApi { error }),
                sessions,
            ),
        }
    }

    /// 注入实现。**只给测试用** —— 生产路径走 [`Self::new`]。
    pub fn with_parts(
        user_data_dir: &Path,
        api: Arc<dyn BaiduApi>,
        sessions: Arc<dyn BaiduSessionStore>,
    ) -> Self {
        Self {
            user_data_dir: user_data_dir.to_path_buf(),
            api,
            sessions,
            directory_timeout: BAIDU_DIRECTORY_TIMEOUT,
            file_timeout: BAIDU_FILE_TIMEOUT,
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
    ) -> Result<(BaiduSession, String), RemoteError> {
        let (connection, normalized) = self.prepare_path(connection_id, remote_path)?;

        let Some(key) = connection
            .credential_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
        else {
            return Err(RemoteError::auth(
                "这条百度网盘连接还没有保存登录凭据，请先完成授权。",
            ));
        };

        let Some(state) = self.sessions.load(key)? else {
            return Err(RemoteError::auth(
                "百度网盘还没有授权，请先在设置里完成授权。",
            ));
        };

        Ok((state.session, normalized))
    }

    /// 翻页列目录。
    async fn list_all(
        &self,
        session: &BaiduSession,
        connection_id: &str,
        dir: &str,
    ) -> Result<Vec<RemoteEntry>, RemoteError> {
        let mut page = ListPage {
            start: 0,
            limit: BAIDU_LIST_PAGE_LIMIT,
        };
        let mut entries: Vec<RemoteEntry> = Vec::new();
        let mut pages = 0u32;

        loop {
            pages += 1;
            if pages > BAIDU_MAX_LIST_PAGES {
                return Err(RemoteError::protocol(format!(
                    "百度网盘这个目录超过了 {BAIDU_MAX_LIST_PAGES} 页，已放弃继续翻页。",
                )));
            }

            let raw = self.api.list_page(session, dir, page).await?;
            let received = raw.len();

            entries.extend(to_remote_entries(connection_id, &raw));

            let Some(next) = next_page(&page, received) else {
                break;
            };
            page = next;
        }

        Ok(entries)
    }

    /// 在父目录里翻页找同名条目。
    async fn find_entry(
        &self,
        session: &BaiduSession,
        parent: &str,
        name: &str,
    ) -> Result<Option<BaiduFileEntry>, RemoteError> {
        let mut page = ListPage {
            start: 0,
            limit: BAIDU_LIST_PAGE_LIMIT,
        };
        let mut pages = 0u32;

        loop {
            pages += 1;
            if pages > BAIDU_MAX_LIST_PAGES {
                return Err(RemoteError::protocol(format!(
                    "百度网盘这个目录超过了 {BAIDU_MAX_LIST_PAGES} 页，已放弃继续翻页。",
                )));
            }

            let raw = self.api.list_page(session, parent, page).await?;
            let received = raw.len();

            if let Some(found) = raw
                .iter()
                .find(|entry| entry_name(entry).as_deref() == Some(name))
            {
                return Ok(Some(found.clone()));
            }

            let Some(next) = next_page(&page, received) else {
                return Ok(None);
            };
            page = next;
        }
    }

    /// 查一个条目的元信息（名字、类型、大小、修改时间）。
    ///
    /// 与 FTP / SFTP 的 `stat` 同语义：**列父目录再按名字找** —— 百度网盘的
    /// 开放接口没有「按路径查条目」这种能力。
    pub async fn stat(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> Result<RemoteEntry, RemoteError> {
        let (session, normalized) = self.prepare(connection_id, remote_path)?;
        let (parent, name) = split_remote_parent_and_name(&normalized)?;

        let work = async {
            let Some(entry) = self.find_entry(&session, &parent, &name).await? else {
                return Err(RemoteError::not_found(format!(
                    "百度网盘里没有这个条目：{name}"
                )));
            };

            to_remote_entry(connection_id, &entry).ok_or_else(|| {
                RemoteError::protocol(format!(
                    "百度网盘返回的条目「{name}」缺少必要字段，无法显示。"
                ))
            })
        };

        with_timeout(self.directory_timeout, "读取百度网盘条目", work).await
    }

    /// 上传统一出口：一律明确报「不支持」。
    ///
    /// **绝不返回 `Ok(())`。** 上传（把本地文件传上云盘）不在阶段 1 的范围里，
    /// 静默成功会让用户以为文件已经传上去了，而它根本没有。
    fn reject_upload(&self, connection_id: &str, remote_path: &str) -> Result<(), RemoteError> {
        self.prepare_path(connection_id, remote_path)?;

        Err(RemoteError::unsupported(
            "百度网盘的上传（把本地文件传上去）在阶段 1 还不支持。",
        ))
    }
}

#[async_trait]
impl RemoteProvider for BaiduRemoteProvider {
    async fn read_dir(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> Result<Vec<RemoteEntry>, RemoteError> {
        let (session, normalized) = self.prepare(connection_id, remote_path)?;

        let work = async { self.list_all(&session, connection_id, &normalized).await };

        with_timeout(self.directory_timeout, "读取百度网盘目录", work).await
    }

    async fn read_file(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> Result<Vec<u8>, RemoteError> {
        let (session, normalized) = self.prepare(connection_id, remote_path)?;
        let (parent, name) = split_remote_parent_and_name(&normalized)?;

        let work = async {
            let Some(entry) = self.find_entry(&session, &parent, &name).await? else {
                return Err(RemoteError::not_found(format!(
                    "百度网盘里没有这个文件：{name}"
                )));
            };

            if entry.isdir == 1 {
                return Err(RemoteError::protocol(format!(
                    "「{name}」是一个目录，不能当文件读取。"
                )));
            }

            if entry.fs_id == 0 {
                return Err(RemoteError::unsupported(
                    "百度网盘没有给出这个文件的服务端 id，无法读取。",
                ));
            }

            // 直链拿不到 ⇒ 这条路径在本阶段**读不了**。
            // 但「会话失效」不能被改写成「不支持」：那两件事的用户处置完全不同
            // （重新授权 vs 等阶段 2）。
            let url = match self.api.file_dlink(&session, entry.fs_id).await {
                Ok(url) => url,
                Err(error) => {
                    return Err(match error.kind {
                        RemoteErrorKind::Protocol => RemoteError::unsupported(
                            "百度网盘没有提供这个文件的下载直链，暂时读不了。",
                        ),
                        _ => error,
                    })
                }
            };

            self.api
                .download(&session, &url, BAIDU_MAX_DOWNLOAD_BYTES)
                .await
        };

        with_timeout(self.file_timeout, "读取百度网盘文件", work).await
    }

    async fn write_file(
        &self,
        connection_id: &str,
        remote_path: &str,
        _content: Vec<u8>,
    ) -> Result<(), RemoteError> {
        self.reject_upload(connection_id, remote_path)
    }

    async fn write_file_with_progress(
        &self,
        connection_id: &str,
        remote_path: &str,
        _content: Vec<u8>,
        _on_progress: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<(), RemoteError> {
        self.reject_upload(connection_id, remote_path)
    }

    async fn make_dir(&self, connection_id: &str, remote_path: &str) -> Result<(), RemoteError> {
        let (session, normalized) = self.prepare(connection_id, remote_path)?;

        // 名字要单独校验：服务端收的是「父目录 + 名字」，而名字里带 `/`
        // 会让一次「新建目录」变成「建到别处去」。
        let (parent, name) = split_remote_parent_and_name(&normalized)?;
        validate_remote_entry_name(&name)?;
        let target = join_remote_parent_and_name(&parent, &name);

        let work = async { self.api.create_dir(&session, &target).await };
        with_timeout(self.directory_timeout, "在百度网盘新建目录", work).await
    }

    async fn set_permissions(
        &self,
        connection_id: &str,
        remote_path: &str,
        _mode: u32,
    ) -> Result<(), RemoteError> {
        self.prepare_path(connection_id, remote_path)?;

        Err(RemoteError::unsupported(
            "百度网盘没有 POSIX 权限位，改权限暂不支持。",
        ))
    }

    async fn get_permissions(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> Result<Option<u32>, RemoteError> {
        // 百度网盘没有权限位这回事 ⇒ 取不到。
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
        let (session, normalized) = self.prepare(connection_id, remote_path)?;

        // 根目录没有条目名，删它就是删整个网盘 —— 这条要在开网络之前挡掉。
        if normalized == "/" {
            return Err(RemoteError::protocol("不能删除百度网盘的根目录。"));
        }

        let work = async { self.api.remove_entry(&session, &normalized).await };
        with_timeout(self.directory_timeout, "在百度网盘删除", work).await
    }

    async fn rename_path(
        &self,
        connection_id: &str,
        from_path: &str,
        new_name: &str,
    ) -> Result<(), RemoteError> {
        // 新名字先校验：**改名只能是改名，不能顺手变成移动**。
        validate_remote_entry_name(new_name)?;

        let (session, normalized) = self.prepare(connection_id, from_path)?;

        let work = async { self.api.rename_entry(&session, &normalized, new_name).await };
        with_timeout(self.directory_timeout, "在百度网盘重命名", work).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::baidu_api::{BaiduSession, BAIDU_LIST_PAGE_LIMIT};
    use crate::remote::connection::RemoteProtocol;
    use crate::remote::error::RemoteErrorKind;
    use crate::remote::registry::{ConnectionsFile, REMOTE_CONNECTIONS_FILE};
    use crate::remote::transport::ProxyConfig;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    const CONNECTION_ID: &str = "conn-baidu";
    const CREDENTIAL_KEY: &str = "conn-baidu";
    const SECRET_BDUSS: &str = "SECRET_BDUSS_0123456789abcdef";

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    // ------------------------------------------------------------------
    // 假实现
    // ------------------------------------------------------------------

    #[derive(Default)]
    struct FakeApi {
        /// 按 (目录, 起始偏移) 预先摆好的一页。
        pages: Mutex<Vec<(String, u32, Vec<BaiduFileEntry>)>>,
        /// 记住被请求过的页，便于断言「翻了几页」「请求的是哪个目录」。
        calls: Mutex<Vec<(String, u32)>>,
        dlinks: Mutex<HashMap<u64, String>>,
        dlink_error: Mutex<Option<RemoteError>>,
        downloads: Mutex<HashMap<String, Vec<u8>>>,
        download_error: Mutex<Option<RemoteError>>,
        /// 下载时收到的上限。
        download_limit: Mutex<u64>,
        /// 「服务端永远返回满页」—— 服务端坏掉时的形态，用来验翻页上限。
        runaway: Mutex<bool>,
        /// runaway 时返回的那一页。
        runaway_page: Mutex<Vec<BaiduFileEntry>>,
        /// 记下被创建的目录。
        created: Mutex<Vec<String>>,
        /// 记下被改名的条目：(原路径, 新名)。
        renamed: Mutex<Vec<(String, String)>>,
        /// 记下被删除的条目。
        removed: Mutex<Vec<String>>,
        /// 写操作要返回的错误。
        write_error: Mutex<Option<RemoteError>>,
    }

    impl FakeApi {
        fn set_page(&self, dir: &str, start: u32, entries: Vec<BaiduFileEntry>) {
            self.pages
                .lock()
                .expect("pages lock")
                .push((dir.to_string(), start, entries));
        }

        /// 让服务端「永远返回满页」—— 服务端坏掉时的形态。
        fn set_runaway(&self, entries: Vec<BaiduFileEntry>) {
            *self.runaway.lock().expect("runaway lock") = true;
            *self.runaway_page.lock().expect("runaway page lock") = entries;
        }

        fn calls(&self) -> Vec<(String, u32)> {
            self.calls.lock().expect("calls lock").clone()
        }

        /// 被创建过的目录。
        fn created(&self) -> Vec<String> {
            self.created.lock().expect("created lock").clone()
        }

        /// 被改过名的条目。
        fn renamed(&self) -> Vec<(String, String)> {
            self.renamed.lock().expect("renamed lock").clone()
        }

        /// 被删除过的条目。
        fn removed(&self) -> Vec<String> {
            self.removed.lock().expect("removed lock").clone()
        }

        /// 让下一次写操作返回这个错误。
        fn set_write_error(&self, error: RemoteError) {
            *self.write_error.lock().expect("write error lock") = Some(error);
        }
    }

    #[async_trait]
    impl BaiduApi for FakeApi {
        async fn list_page(
            &self,
            _session: &BaiduSession,
            dir: &str,
            page: ListPage,
        ) -> Result<Vec<BaiduFileEntry>, RemoteError> {
            self.calls
                .lock()
                .expect("calls lock")
                .push((dir.to_string(), page.start));

            if *self.runaway.lock().expect("runaway lock") {
                return Ok(self.runaway_page.lock().expect("runaway page lock").clone());
            }

            let pages = self.pages.lock().expect("pages lock");
            for (page_dir, start, entries) in pages.iter() {
                if page_dir == dir && *start == page.start {
                    return Ok(entries.clone());
                }
            }

            Ok(Vec::new())
        }

        async fn file_dlink(
            &self,
            _session: &BaiduSession,
            fs_id: u64,
        ) -> Result<String, RemoteError> {
            if let Some(error) = self.dlink_error.lock().expect("dlink error lock").clone() {
                return Err(error);
            }

            self.dlinks
                .lock()
                .expect("dlinks lock")
                .get(&fs_id)
                .cloned()
                .ok_or_else(|| RemoteError::unsupported("这个文件没有下载直链。"))
        }

        async fn download(
            &self,
            _session: &BaiduSession,
            url: &str,
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
                .get(url)
                .cloned()
                .ok_or_else(|| RemoteError::not_found("没有这个直链。"))
        }

        async fn bdstoken(&self, _session: &BaiduSession) -> Result<String, RemoteError> {
            Ok("fake-bdstoken".to_string())
        }

        async fn create_dir(&self, _session: &BaiduSession, path: &str) -> Result<(), RemoteError> {
            if let Some(error) = self.write_error.lock().expect("write error lock").clone() {
                return Err(error);
            }

            self.created
                .lock()
                .expect("created lock")
                .push(path.to_string());

            Ok(())
        }

        async fn rename_entry(
            &self,
            _session: &BaiduSession,
            path: &str,
            new_name: &str,
        ) -> Result<(), RemoteError> {
            if let Some(error) = self.write_error.lock().expect("write error lock").clone() {
                return Err(error);
            }

            self.renamed
                .lock()
                .expect("renamed lock")
                .push((path.to_string(), new_name.to_string()));

            Ok(())
        }

        async fn remove_entry(
            &self,
            _session: &BaiduSession,
            path: &str,
        ) -> Result<(), RemoteError> {
            if let Some(error) = self.write_error.lock().expect("write error lock").clone() {
                return Err(error);
            }

            self.removed
                .lock()
                .expect("removed lock")
                .push(path.to_string());

            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeSessionStore {
        states: Mutex<HashMap<String, BaiduSessionState>>,
        error: Mutex<Option<RemoteError>>,
    }

    impl FakeSessionStore {
        fn insert(&self, key: &str, state: BaiduSessionState) {
            self.states
                .lock()
                .expect("states lock")
                .insert(key.to_string(), state);
        }
    }

    impl BaiduSessionStore for FakeSessionStore {
        fn load(&self, key: &str) -> Result<Option<BaiduSessionState>, RemoteError> {
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

    fn session_state() -> BaiduSessionState {
        BaiduSessionState::new(
            BaiduSession::new(SECRET_BDUSS, Some("STOKEN_VALUE".to_string())),
            None,
        )
    }

    fn file_entry(
        fs_id: u64,
        path: &str,
        name: &str,
        isdir: i64,
        size: u64,
        mtime: u64,
    ) -> BaiduFileEntry {
        BaiduFileEntry {
            fs_id,
            path: path.to_string(),
            server_filename: name.to_string(),
            isdir,
            size,
            server_mtime: mtime,
            local_mtime: mtime,
        }
    }

    fn fixture(tag: &str) -> Fixture {
        let dir = std::env::temp_dir().join(format!(
            "afm-baidu-{tag}-{}",
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");

        // 连接表里这条记录的协议就是 `baidu_pan`：provider 按协议分派，
        // 协议写错的话，命令层会把它分派给 SFTP provider。
        let connections = ConnectionsFile {
            connections: vec![RemoteConnection {
                id: CONNECTION_ID.to_string(),
                name: "百度网盘".to_string(),
                protocol: RemoteProtocol::BaiduPan,
                host: "pan.baidu.com".to_string(),
                port: 443,
                username: "测试用户".to_string(),
                initial_path: "/".to_string(),
                local_path: None,
                proxy: ProxyConfig::Direct,
                credential_key: Some(CREDENTIAL_KEY.to_string()),
            }],
        };
        std::fs::write(
            dir.join(REMOTE_CONNECTIONS_FILE),
            serde_json::to_string_pretty(&connections).expect("serialize connections"),
        )
        .expect("write connections file");

        let api = Arc::new(FakeApi::default());
        let sessions = Arc::new(FakeSessionStore::default());
        sessions.insert(CREDENTIAL_KEY, session_state());

        Fixture { dir, api, sessions }
    }

    fn provider(fixture: &Fixture) -> BaiduRemoteProvider {
        BaiduRemoteProvider::with_parts(&fixture.dir, fixture.api.clone(), fixture.sessions.clone())
    }

    // ------------------------------------------------------------------
    // 单个条目
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn stat_finds_an_entry_by_listing_its_parent_directory() {
        let fixture = fixture("stat-hit");
        let provider = provider(&fixture);
        fixture.api.set_page(
            "/",
            0,
            vec![
                file_entry(11, "/notes.txt", "notes.txt", 0, 12, 1_700_000_000),
                file_entry(12, "/docs", "docs", 1, 0, 1_700_000_001),
            ],
        );

        let entry = provider
            .stat(CONNECTION_ID, "/notes.txt")
            .await
            .expect("stat must find the entry");

        assert_eq!(entry.name, "notes.txt");
        assert!(!entry.is_dir, "notes.txt 是文件");
        assert_eq!(entry.size, 12);
    }

    /// 判别性的一条：条目不在列表里时必须报「没有」，
    /// 而不是返回一个空的 `RemoteEntry` 让上层以为它存在。
    #[tokio::test]
    async fn stat_of_a_missing_entry_is_reported_as_not_found() {
        let fixture = fixture("stat-miss");
        let provider = provider(&fixture);
        fixture.api.set_page(
            "/",
            0,
            vec![file_entry(
                11,
                "/notes.txt",
                "notes.txt",
                0,
                12,
                1_700_000_000,
            )],
        );

        let error = provider
            .stat(CONNECTION_ID, "/ghost.txt")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::NotFound);
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
    async fn lists_the_entries_of_a_directory() {
        let fixture = fixture("list");
        fixture.api.set_page(
            "/",
            0,
            vec![
                file_entry(101, "/01A_软件", "01A_软件", 1, 0, 1_700_000_000),
                file_entry(102, "/设计素材.zip", "设计素材.zip", 0, 2048, 1_700_000_060),
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
    async fn builds_sfm_remote_paths_without_percent_encoding() {
        let fixture = fixture("paths");
        fixture.api.set_page(
            "/",
            0,
            vec![file_entry(
                101,
                "/01A_软件",
                "01A_软件",
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

        // 中文**原样**存放：不编码（与本项目本地路径的处理一致）。
        assert_eq!(entries[0].path, "sfm://remote/conn-baidu/01A_软件");
        assert!(!entries[0].path.contains('%'));
    }

    #[tokio::test]
    async fn reports_modified_time_in_unix_milliseconds() {
        let fixture = fixture("mtime");
        fixture.api.set_page(
            "/",
            0,
            vec![file_entry(101, "/a.txt", "a.txt", 0, 1, 1_700_000_000)],
        );
        let provider = provider(&fixture);

        let entries = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read root");

        assert_eq!(entries[0].modified_time, 1_700_000_000_000);
    }

    #[tokio::test]
    async fn marks_dot_prefixed_entries_as_hidden() {
        let fixture = fixture("hidden");
        fixture
            .api
            .set_page("/", 0, vec![file_entry(101, "/.隐藏", ".隐藏", 0, 1, 1)]);
        let provider = provider(&fixture);

        let entries = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read root");

        assert!(entries[0].is_hidden);
    }

    #[tokio::test]
    async fn never_marks_an_entry_as_a_symlink() {
        let fixture = fixture("symlink");
        fixture
            .api
            .set_page("/", 0, vec![file_entry(101, "/a.txt", "a.txt", 0, 1, 1)]);
        let provider = provider(&fixture);

        let entries = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read root");

        assert!(!entries[0].is_symlink);
    }

    #[tokio::test]
    async fn follows_pagination_until_the_last_partial_page() {
        let fixture = fixture("pages");
        let full: Vec<BaiduFileEntry> = (0..BAIDU_LIST_PAGE_LIMIT)
            .map(|index| {
                file_entry(
                    index as u64 + 1,
                    &format!("/f{index}"),
                    &format!("f{index}"),
                    0,
                    1,
                    1,
                )
            })
            .collect();
        fixture.api.set_page("/", 0, full);
        fixture.api.set_page(
            "/",
            BAIDU_LIST_PAGE_LIMIT,
            vec![file_entry(9999, "/last.txt", "last.txt", 0, 1, 1)],
        );
        let provider = provider(&fixture);

        let entries = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read root");

        assert_eq!(entries.len(), BAIDU_LIST_PAGE_LIMIT as usize + 1);
        assert_eq!(entries.last().expect("last entry").name, "last.txt");

        let calls = fixture.api.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].1, 0);
        assert_eq!(calls[1].1, BAIDU_LIST_PAGE_LIMIT);
    }

    #[tokio::test]
    async fn stops_paging_when_the_server_stops_returning_full_pages() {
        let fixture = fixture("pages-short");
        fixture
            .api
            .set_page("/", 0, vec![file_entry(1, "/a.txt", "a.txt", 0, 1, 1)]);
        let provider = provider(&fixture);

        provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read root");

        assert_eq!(fixture.api.calls().len(), 1);
    }

    #[tokio::test]
    async fn refuses_to_paginate_forever() {
        let fixture = fixture("pages-runaway");
        // 每一页都返回满页 —— 服务端坏掉时的形态。
        let full: Vec<BaiduFileEntry> = (0..BAIDU_LIST_PAGE_LIMIT)
            .map(|index| {
                file_entry(
                    index as u64 + 1,
                    &format!("/f{index}"),
                    &format!("f{index}"),
                    0,
                    1,
                    1,
                )
            })
            .collect();
        fixture.api.set_runaway(full);
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("must stop paginating");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
        // 翻页必须被上限截断，而不是无限请求下去。
        assert!(fixture.api.calls().len() <= BAIDU_MAX_LIST_PAGES as usize + 1);
    }

    #[tokio::test]
    async fn skips_entries_that_cannot_be_mapped() {
        let fixture = fixture("unmappable");
        // `path` 为空 ⇒ 拼不出 `sfm://remote/...` ⇒ 这条要被跳过，而不是造半截数据。
        fixture.api.set_page(
            "/",
            0,
            vec![
                file_entry(1, "", "", 0, 1, 1),
                file_entry(2, "/ok.txt", "ok.txt", 0, 1, 1),
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

    // ------------------------------------------------------------------
    // 列目录：入参与凭据
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn rejects_dot_segments_before_touching_the_network() {
        let fixture = fixture("dot");
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/pub/../etc")
            .await
            .expect_err("dot-dot must be rejected");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
        assert!(fixture.api.calls().is_empty());
    }

    #[tokio::test]
    async fn rejects_single_dot_segments() {
        let fixture = fixture("dot-single");
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/pub/./docs")
            .await
            .expect_err("single dot must be rejected");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
    }

    #[tokio::test]
    async fn rejects_control_characters_before_touching_the_network() {
        let fixture = fixture("control");
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/pub\r\nQUIT")
            .await
            .expect_err("control characters must be rejected");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
        assert!(fixture.api.calls().is_empty());
    }

    #[tokio::test]
    async fn reports_an_unknown_connection_as_not_found() {
        let fixture = fixture("unknown-connection");
        let provider = provider(&fixture);

        let error = provider
            .read_dir("conn-missing", "/")
            .await
            .expect_err("unknown connection must fail");

        assert_eq!(error.kind, RemoteErrorKind::NotFound);
    }

    #[tokio::test]
    async fn reports_a_connection_without_credentials_as_an_auth_problem() {
        let fixture = fixture("no-credential-key");
        let provider = provider(&fixture);

        // 把连接表里的凭据键去掉。
        let mut connections: ConnectionsFile = serde_json::from_str(
            &std::fs::read_to_string(fixture.dir.join(REMOTE_CONNECTIONS_FILE)).expect("read"),
        )
        .expect("parse");
        connections.connections[0].credential_key = None;
        std::fs::write(
            fixture.dir.join(REMOTE_CONNECTIONS_FILE),
            serde_json::to_string_pretty(&connections).expect("serialize"),
        )
        .expect("write");

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("missing credential key must fail");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[tokio::test]
    async fn reports_a_missing_session_as_an_auth_problem() {
        let fixture = fixture("no-session");
        fixture.sessions.states.lock().expect("states lock").clear();
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("missing session must fail");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
        assert!(fixture.api.calls().is_empty());
    }

    #[tokio::test]
    async fn reports_a_broken_session_archive_as_an_auth_problem() {
        let fixture = fixture("broken-session");
        *fixture.sessions.error.lock().expect("session error lock") = Some(RemoteError::auth(
            "已保存的百度网盘会话已损坏，请重新登录获取",
        ));
        let provider = provider(&fixture);

        let error = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("broken archive must fail");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[tokio::test]
    async fn passes_through_an_auth_failure_from_the_service() {
        let fixture = fixture("api-auth");
        fixture.api.set_page("/", 0, Vec::new());

        // 假 API 直接把服务端的失败透传给 provider。
        let failing = BaiduRemoteProvider::with_parts(
            &fixture.dir,
            Arc::new(FailingApi {
                error: RemoteError::auth("用户未登录"),
            }),
            fixture.sessions.clone(),
        );

        let error = failing
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("auth failure must surface");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[tokio::test]
    async fn passes_through_a_permission_failure_from_the_service() {
        let fixture = fixture("api-permission");
        let failing = BaiduRemoteProvider::with_parts(
            &fixture.dir,
            Arc::new(FailingApi {
                error: RemoteError::permission("没有权限"),
            }),
            fixture.sessions.clone(),
        );

        let error = failing
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("permission failure must surface");

        assert_eq!(error.kind, RemoteErrorKind::Permission);
    }

    #[tokio::test]
    async fn passes_through_a_not_found_failure_from_the_service() {
        let fixture = fixture("api-not-found");
        let failing = BaiduRemoteProvider::with_parts(
            &fixture.dir,
            Arc::new(FailingApi {
                error: RemoteError::not_found("目录不存在"),
            }),
            fixture.sessions.clone(),
        );

        let error = failing
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("not found must surface");

        assert_eq!(error.kind, RemoteErrorKind::NotFound);
    }

    #[tokio::test]
    async fn passes_through_an_unreachable_failure_from_the_service() {
        let fixture = fixture("api-unreachable");
        let failing = BaiduRemoteProvider::with_parts(
            &fixture.dir,
            Arc::new(FailingApi {
                error: RemoteError::unreachable("网络不可达"),
            }),
            fixture.sessions.clone(),
        );

        let error = failing
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("unreachable must surface");

        assert_eq!(error.kind, RemoteErrorKind::Unreachable);
    }

    #[tokio::test]
    async fn normalizes_the_remote_path_before_listing() {
        let fixture = fixture("normalize");
        fixture.api.set_page(
            "/pub",
            0,
            vec![file_entry(1, "/pub/a.txt", "a.txt", 0, 1, 1)],
        );
        let provider = provider(&fixture);

        let with_slash = provider
            .read_dir(CONNECTION_ID, "/pub/")
            .await
            .expect("trailing slash");
        let without_slash = provider
            .read_dir(CONNECTION_ID, "/pub")
            .await
            .expect("plain path");
        let doubled = provider
            .read_dir(CONNECTION_ID, "/pub//")
            .await
            .expect("repeated slash");

        assert_eq!(with_slash, without_slash);
        assert_eq!(doubled, without_slash);
    }

    #[tokio::test]
    async fn never_leaks_the_bduss_into_an_error_message() {
        let fixture = fixture("secret");
        let failing = BaiduRemoteProvider::with_parts(
            &fixture.dir,
            Arc::new(FailingApi {
                error: RemoteError::unknown("服务返回了一个奇怪的结果"),
            }),
            fixture.sessions.clone(),
        );

        let error = failing
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("must fail");

        assert!(!error.message.contains(SECRET_BDUSS));

        // 未授权路径上的文案同样不能带凭据。
        fixture.sessions.states.lock().expect("states lock").clear();
        let missing = provider(&fixture)
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect_err("must fail");

        assert!(!missing.message.contains(SECRET_BDUSS));
    }

    // ------------------------------------------------------------------
    // 读文件
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn reads_a_file_through_its_download_link() {
        let fixture = fixture("read");
        fixture.api.set_page(
            "/pub",
            0,
            vec![file_entry(7, "/pub/a.txt", "a.txt", 0, 5, 1)],
        );
        fixture
            .api
            .dlinks
            .lock()
            .expect("dlinks lock")
            .insert(7, "https://d.pcs.baidu.com/file/7".to_string());
        fixture
            .api
            .downloads
            .lock()
            .expect("downloads lock")
            .insert(
                "https://d.pcs.baidu.com/file/7".to_string(),
                b"hello".to_vec(),
            );
        let provider = provider(&fixture);

        let content = provider
            .read_file(CONNECTION_ID, "/pub/a.txt")
            .await
            .expect("read file");

        assert_eq!(content, b"hello".to_vec());
    }

    #[tokio::test]
    async fn caps_the_download_size() {
        let fixture = fixture("read-cap");
        fixture.api.set_page(
            "/pub",
            0,
            vec![file_entry(7, "/pub/a.txt", "a.txt", 0, 5, 1)],
        );
        fixture
            .api
            .dlinks
            .lock()
            .expect("dlinks lock")
            .insert(7, "https://example.invalid/7".to_string());
        fixture
            .api
            .downloads
            .lock()
            .expect("downloads lock")
            .insert("https://example.invalid/7".to_string(), b"hello".to_vec());
        let provider = provider(&fixture);

        provider
            .read_file(CONNECTION_ID, "/pub/a.txt")
            .await
            .expect("read file");

        assert_eq!(
            *fixture
                .api
                .download_limit
                .lock()
                .expect("download limit lock"),
            BAIDU_MAX_DOWNLOAD_BYTES
        );
    }

    #[tokio::test]
    async fn finds_a_file_that_lives_on_a_later_page() {
        let fixture = fixture("read-page");
        let full: Vec<BaiduFileEntry> = (0..BAIDU_LIST_PAGE_LIMIT)
            .map(|index| {
                file_entry(
                    index as u64 + 1,
                    &format!("/pub/f{index}"),
                    &format!("f{index}"),
                    0,
                    1,
                    1,
                )
            })
            .collect();
        fixture.api.set_page("/pub", 0, full);
        fixture.api.set_page(
            "/pub",
            BAIDU_LIST_PAGE_LIMIT,
            vec![file_entry(4242, "/pub/target.txt", "target.txt", 0, 3, 1)],
        );
        fixture
            .api
            .dlinks
            .lock()
            .expect("dlinks lock")
            .insert(4242, "https://example.invalid/4242".to_string());
        fixture
            .api
            .downloads
            .lock()
            .expect("downloads lock")
            .insert("https://example.invalid/4242".to_string(), b"xyz".to_vec());
        let provider = provider(&fixture);

        let content = provider
            .read_file(CONNECTION_ID, "/pub/target.txt")
            .await
            .expect("read file");

        assert_eq!(content, b"xyz".to_vec());
    }

    #[tokio::test]
    async fn reports_a_missing_file_as_not_found() {
        let fixture = fixture("read-missing");
        fixture.api.set_page(
            "/pub",
            0,
            vec![file_entry(1, "/pub/other.txt", "other.txt", 0, 1, 1)],
        );
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/pub/nope.txt")
            .await
            .expect_err("missing file must fail");

        assert_eq!(error.kind, RemoteErrorKind::NotFound);
    }

    #[tokio::test]
    async fn refuses_to_read_a_directory_as_a_file() {
        let fixture = fixture("read-dir");
        fixture
            .api
            .set_page("/pub", 0, vec![file_entry(1, "/pub/sub", "sub", 1, 0, 1)]);
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/pub/sub")
            .await
            .expect_err("directory must fail");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
    }

    #[tokio::test]
    async fn refuses_a_file_without_a_server_side_id() {
        let fixture = fixture("read-no-fsid");
        fixture.api.set_page(
            "/pub",
            0,
            vec![file_entry(0, "/pub/a.txt", "a.txt", 0, 1, 1)],
        );
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/pub/a.txt")
            .await
            .expect_err("no fs_id must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unsupported);
    }

    #[tokio::test]
    async fn reports_a_missing_download_link_as_unsupported() {
        let fixture = fixture("read-no-dlink");
        fixture.api.set_page(
            "/pub",
            0,
            vec![file_entry(7, "/pub/a.txt", "a.txt", 0, 1, 1)],
        );
        // 没有登记直链 ⇒ 假 API 报 `Unsupported`。
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/pub/a.txt")
            .await
            .expect_err("missing dlink must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unsupported);
    }

    #[tokio::test]
    async fn turns_a_protocol_failure_while_fetching_the_link_into_unsupported() {
        let fixture = fixture("read-dlink-protocol");
        fixture.api.set_page(
            "/pub",
            0,
            vec![file_entry(7, "/pub/a.txt", "a.txt", 0, 1, 1)],
        );
        *fixture.api.dlink_error.lock().expect("dlink error lock") =
            Some(RemoteError::protocol("响应无法解析"));
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/pub/a.txt")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unsupported);
    }

    #[tokio::test]
    async fn does_not_turn_an_auth_failure_while_fetching_the_link_into_unsupported() {
        let fixture = fixture("read-dlink-auth");
        fixture.api.set_page(
            "/pub",
            0,
            vec![file_entry(7, "/pub/a.txt", "a.txt", 0, 1, 1)],
        );
        *fixture.api.dlink_error.lock().expect("dlink error lock") =
            Some(RemoteError::auth("用户未登录"));
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/pub/a.txt")
            .await
            .expect_err("must fail");

        // 「重新授权」与「不支持」的用户处置完全不同，不能被合并。
        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[tokio::test]
    async fn passes_through_a_download_failure() {
        let fixture = fixture("read-download-error");
        fixture.api.set_page(
            "/pub",
            0,
            vec![file_entry(7, "/pub/a.txt", "a.txt", 0, 1, 1)],
        );
        fixture
            .api
            .dlinks
            .lock()
            .expect("dlinks lock")
            .insert(7, "https://example.invalid/7".to_string());
        *fixture
            .api
            .download_error
            .lock()
            .expect("download error lock") = Some(RemoteError::disconnected("下载中断"));
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/pub/a.txt")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Disconnected);
    }

    #[tokio::test]
    async fn refuses_to_read_the_remote_root_as_a_file() {
        let fixture = fixture("read-root");
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/")
            .await
            .expect_err("root is not a file");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
    }

    #[tokio::test]
    async fn rejects_control_characters_in_a_file_path() {
        let fixture = fixture("read-control");
        let provider = provider(&fixture);

        let error = provider
            .read_file(CONNECTION_ID, "/pub/a\r\n.txt")
            .await
            .expect_err("control characters must be rejected");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
        assert!(fixture.api.calls().is_empty());
    }

    #[tokio::test]
    async fn reports_an_unknown_connection_while_reading_a_file() {
        let fixture = fixture("read-unknown-connection");
        let provider = provider(&fixture);

        let error = provider
            .read_file("conn-missing", "/pub/a.txt")
            .await
            .expect_err("unknown connection must fail");

        assert_eq!(error.kind, RemoteErrorKind::NotFound);
    }

    // ------------------------------------------------------------------
    // 权限
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn reports_no_permissions_without_touching_the_network() {
        let fixture = fixture("perm");
        let provider = provider(&fixture);

        let mode = provider
            .get_permissions(CONNECTION_ID, "/pub/a.txt")
            .await
            .expect("get permissions");

        assert_eq!(mode, None);
        assert!(fixture.api.calls().is_empty());
    }

    #[tokio::test]
    async fn reports_an_unknown_connection_while_reading_permissions() {
        let fixture = fixture("perm-unknown");
        let provider = provider(&fixture);

        let error = provider
            .get_permissions("conn-missing", "/pub/a.txt")
            .await
            .expect_err("unknown connection must fail");

        assert_eq!(error.kind, RemoteErrorKind::NotFound);
    }

    // ------------------------------------------------------------------
    // 写操作：新建目录 / 改名 / 删除
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn creates_a_directory_under_the_given_parent() {
        let fixture = fixture("write-mkdir");
        let provider = provider(&fixture);

        provider
            .make_dir(CONNECTION_ID, "/pub/sub")
            .await
            .expect("create directory");

        assert_eq!(fixture.api.created(), vec!["/pub/sub".to_string()]);
    }

    #[tokio::test]
    async fn creates_a_directory_in_the_remote_root() {
        let fixture = fixture("write-mkdir-root");
        let provider = provider(&fixture);

        provider
            .make_dir(CONNECTION_ID, "/sub")
            .await
            .expect("create directory");

        assert_eq!(fixture.api.created(), vec!["/sub".to_string()]);
    }

    #[tokio::test]
    async fn refuses_to_create_a_directory_at_the_remote_root() {
        let fixture = fixture("write-mkdir-root-bad");
        let provider = provider(&fixture);

        let error = provider
            .make_dir(CONNECTION_ID, "/")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
        assert!(fixture.api.created().is_empty());
    }

    #[tokio::test]
    async fn removes_a_file() {
        let fixture = fixture("write-remove");
        let provider = provider(&fixture);

        provider
            .remove_path(CONNECTION_ID, "/pub/a.txt", false)
            .await
            .expect("remove file");

        assert_eq!(fixture.api.removed(), vec!["/pub/a.txt".to_string()]);
    }

    #[tokio::test]
    async fn removes_a_directory() {
        let fixture = fixture("write-remove-dir");
        let provider = provider(&fixture);

        provider
            .remove_path(CONNECTION_ID, "/pub/sub", true)
            .await
            .expect("remove directory");

        assert_eq!(fixture.api.removed(), vec!["/pub/sub".to_string()]);
    }

    #[tokio::test]
    async fn refuses_to_delete_the_remote_root() {
        let fixture = fixture("write-remove-root");
        let provider = provider(&fixture);

        let error = provider
            .remove_path(CONNECTION_ID, "/", true)
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
        assert!(fixture.api.removed().is_empty());
    }

    #[tokio::test]
    async fn renames_an_entry_in_place() {
        let fixture = fixture("write-rename");
        let provider = provider(&fixture);

        provider
            .rename_path(CONNECTION_ID, "/pub/a.txt", "b.txt")
            .await
            .expect("rename entry");

        assert_eq!(
            fixture.api.renamed(),
            vec![("/pub/a.txt".to_string(), "b.txt".to_string())]
        );
    }

    #[tokio::test]
    async fn rejects_an_illegal_new_name_before_calling_the_service() {
        let fixture = fixture("write-rename-bad");
        let provider = provider(&fixture);

        let error = provider
            .rename_path(CONNECTION_ID, "/pub/a.txt", "b/c.txt")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
        assert!(fixture.api.renamed().is_empty());
    }

    #[tokio::test]
    async fn rejects_a_dot_new_name_before_calling_the_service() {
        let fixture = fixture("write-rename-dot");
        let provider = provider(&fixture);

        let error = provider
            .rename_path(CONNECTION_ID, "/pub/a.txt", "..")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
        assert!(fixture.api.renamed().is_empty());
    }

    #[tokio::test]
    async fn passes_through_a_write_failure() {
        let fixture = fixture("write-failure");
        fixture
            .api
            .set_write_error(RemoteError::permission("网盘拒绝了这个操作。"));
        let provider = provider(&fixture);

        let error = provider
            .make_dir(CONNECTION_ID, "/pub/sub")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Permission);
    }

    #[tokio::test]
    async fn passes_through_an_auth_failure_during_a_write() {
        let fixture = fixture("write-auth");
        fixture
            .api
            .set_write_error(RemoteError::auth("会话已失效，请重新授权。"));
        let provider = provider(&fixture);

        let error = provider
            .remove_path(CONNECTION_ID, "/pub/a.txt", false)
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[tokio::test]
    async fn refuses_to_write_a_file() {
        let fixture = fixture("write-file");
        let provider = provider(&fixture);

        let error = provider
            .write_file(CONNECTION_ID, "/pub/a.txt", b"body".to_vec())
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unsupported);
    }

    #[tokio::test]
    async fn refuses_to_write_a_file_with_progress_and_never_reports_progress() {
        let fixture = fixture("write-progress");
        let provider = provider(&fixture);
        let reported = AtomicUsize::new(0);

        let error = provider
            .write_file_with_progress(CONNECTION_ID, "/pub/a.txt", b"body".to_vec(), &|bytes| {
                reported.fetch_add(bytes as usize, Ordering::SeqCst);
            })
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unsupported);
        assert_eq!(reported.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn refuses_to_change_permissions() {
        let fixture = fixture("write-chmod");
        let provider = provider(&fixture);

        let error = provider
            .set_permissions(CONNECTION_ID, "/pub/a.txt", 0o755)
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Unsupported);
    }

    #[tokio::test]
    async fn reports_an_illegal_path_before_calling_the_service() {
        let fixture = fixture("write-bad-path");
        let provider = provider(&fixture);

        let error = provider
            .make_dir(CONNECTION_ID, "/pub/../etc")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
        assert!(fixture.api.created().is_empty());
    }

    #[tokio::test]
    async fn reports_an_unknown_connection_before_calling_the_service() {
        let fixture = fixture("write-unknown");
        let provider = provider(&fixture);

        let error = provider
            .make_dir("conn-missing", "/pub/sub")
            .await
            .expect_err("must fail");

        assert_eq!(error.kind, RemoteErrorKind::NotFound);
        assert!(fixture.api.created().is_empty());
    }

    #[tokio::test]
    async fn never_touches_the_network_for_an_upload() {
        let fixture = fixture("write-no-network");
        let provider = provider(&fixture);

        let _ = provider
            .write_file(CONNECTION_ID, "/pub/a.txt", vec![])
            .await;
        let _ = provider
            .write_file_with_progress(CONNECTION_ID, "/pub/a.txt", vec![], &|_| {})
            .await;

        assert!(fixture.api.calls().is_empty());
        assert!(fixture.api.created().is_empty());
        assert!(fixture.api.renamed().is_empty());
        assert!(fixture.api.removed().is_empty());
    }

    // ------------------------------------------------------------------
    // bdstoken 过期后的重试
    // ------------------------------------------------------------------

    #[test]
    fn only_an_auth_failure_is_worth_retrying_with_a_new_token() {
        assert!(should_refresh_token(&RemoteError::auth("会话失效。")));
        assert!(!should_refresh_token(&RemoteError::protocol("参数错误。")));
        assert!(!should_refresh_token(&RemoteError::permission(
            "权限不足。"
        )));
        assert!(!should_refresh_token(&RemoteError::not_found(
            "没有这个路径。"
        )));
        assert!(!should_refresh_token(&RemoteError::unreachable("连不上。")));
    }

    // ------------------------------------------------------------------
    // 与其它 provider 的一致性
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn is_usable_through_a_trait_object() {
        let fixture = fixture("dyn");
        fixture
            .api
            .set_page("/", 0, vec![file_entry(1, "/a.txt", "a.txt", 0, 1, 1)]);
        let provider: Box<dyn RemoteProvider> = Box::new(provider(&fixture));

        let entries = provider
            .read_dir(CONNECTION_ID, "/")
            .await
            .expect("read through the trait object");

        assert_eq!(entries.len(), 1);
    }

    /// 假 API：所有方法都返回同一个错误。
    struct FailingApi {
        error: RemoteError,
    }

    #[async_trait]
    impl BaiduApi for FailingApi {
        async fn list_page(
            &self,
            _session: &BaiduSession,
            _dir: &str,
            _page: ListPage,
        ) -> Result<Vec<BaiduFileEntry>, RemoteError> {
            Err(self.error.clone())
        }

        async fn file_dlink(
            &self,
            _session: &BaiduSession,
            _fs_id: u64,
        ) -> Result<String, RemoteError> {
            Err(self.error.clone())
        }

        async fn download(
            &self,
            _session: &BaiduSession,
            _url: &str,
            _max_bytes: u64,
        ) -> Result<Vec<u8>, RemoteError> {
            Err(self.error.clone())
        }

        async fn bdstoken(&self, _session: &BaiduSession) -> Result<String, RemoteError> {
            Err(self.error.clone())
        }

        async fn create_dir(
            &self,
            _session: &BaiduSession,
            _path: &str,
        ) -> Result<(), RemoteError> {
            Err(self.error.clone())
        }

        async fn rename_entry(
            &self,
            _session: &BaiduSession,
            _path: &str,
            _new_name: &str,
        ) -> Result<(), RemoteError> {
            Err(self.error.clone())
        }

        async fn remove_entry(
            &self,
            _session: &BaiduSession,
            _path: &str,
        ) -> Result<(), RemoteError> {
            Err(self.error.clone())
        }
    }
}
