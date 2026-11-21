// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! 远程文件缩略图的**代理**命令：把「云端像素取回来并缩好」这件事整个留在 Rust 里。
//!
//! ## 为什么需要这个模块
//!
//! 网盘的下载直链带签名、还会过期，前端拿到之后**跨域取不到像素**（浏览器 CORS
//! 与签名头都不通），所以直链方案在架构上不成立。这里改成：前端只交一个远程路径，
//! 由 provider 用它自己的会话把字节代取回来，缩成 PNG 落进缓存，最后回一个**本地
//! 缓存文件路径**给 `convertFileSrc`。签名 URL 从头到尾不离开 provider 那一层。
//!
//! ## 本模块**不实现任何 HTTP**
//!
//! 取字节是 [`RemoteProvider::thumbnail_bytes`] 的职责——每个 provider 已经有
//! 自己带超护栏网的下载器；在这里另起一个 reqwest 等于把那些护栏复制一遍，
//! 还会让签名 URL 跨模块流动。本模块只做四件事：
//! 解析路径 → 找到 provider → 要字节 → 交给
//! [`crate::image_thumbnails::write_remote_thumbnail_bytes`]。
//!
//! ## 分层红线（改这个文件前务必读）
//!
//! 见 `remote/mod.rs` 顶部与 `remote/commands.rs` 顶部那两段注释：`remote` 模块
//! 本体被 `cargo test --lib` 链接，**模块本体**里出现 `tauri::AppHandle`，会把它
//! 单态化出来的 `GlobalRuntime<Wry>` 连同样板窗口链一起拖进测试二进制；测试二进制
//! 缺 comctl32 v6 manifest ⇒ 加载期 `STATUS_ENTRYPOINT_NOT_FOUND`，整套测试崩掉
//! 且看不出是谁干的。
//!
//! 本文件之所以**可以**用 `tauri::command` / `tauri::AppHandle`
//! / `tauri::async_runtime::spawn_blocking`，靠的是 `commands.rs` 顶部立下的同一
//! 条件：**没有任何单测调用这些 command**。所以本文件的测试只许测自由函数，
//! **绝不允许新增「跑一下 command」的用例**——那等于当场拆掉这层保护。
//! 需要可测的逻辑，就往下抽成收 `PathBuf`/`&str` 的自由函数。

use std::path::PathBuf;

// 一律走 `crate::remote::…` 全路径：本模块是新加的兄弟模块，哪些名字经 `mod.rs`
// 再导出、哪些只是 `commands.rs` 的私有 import，从这里看不出来。写全路径一次到位，
// 省得跟着 `pub use` 清单的改动来回修 import。
use crate::image_thumbnails;
use crate::remote::error::RemoteError;
use crate::remote::path::parse_remote_path;
use crate::remote::provider_with_protocol;
use crate::remote::registry;

/// 一次缩略图代取**最多**允许吃下多少字节。
///
/// 首选链取的是云端**已经缩好**的 256px，正常也就几十 KB；但这个值必须是硬上限，
/// 因为候选链最后一级会退到**原图**直链（云端还没生成缩略图时）。32 MB 刻意压在
/// 本地缩略图源文件的 128 MB 限额之下，同时明显高于缩略图应有的规模：真撞上
/// 几百 MB 的 RAW，宁可直接超限降级成占位图，也不要为了一个 384px 的格子
/// 把内存和带宽吃掉。
const REMOTE_THUMBNAIL_MAX_DOWNLOAD_BYTES: u64 = 32 * 1024 * 1024;

/// 兜底上限：前端传来的尺寸不能变成「让后端去解一张无上限的图」。
const REMOTE_THUMBNAIL_MAX_DIMENSION: u32 = 1024;

/// 形状与 `commands.rs` 里的同名 helper 一致：用户数据目录挂在应用数据目录下面。
fn user_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, RemoteError> {
    let app_data_dir = crate::app_dirs::app_data_dir(app)
        .map_err(|error| RemoteError::unknown(format!("解析应用数据目录失败: {error}")))?;
    Ok(app_data_dir.join(
        &crate::user_storage_files_config::user_storage_files_config().user_data_dir_name,
    ))
}

/// 读注册表是**同步文件 IO**，必须挪出异步运行时——照抄 `commands.rs` 里那个
/// 私有 helper 的做法（它没有 `pub`，跨模块拿不到，只能各留一份）。
async fn resolve_connection_blocking(
    user_data_dir: PathBuf,
    app_data_dir: PathBuf,
    connection_id: String,
) -> Result<registry::ResolvedConnection, RemoteError> {
    tauri::async_runtime::spawn_blocking(move || {
        registry::resolve_connection(&user_data_dir, &app_data_dir, &connection_id)
    })
    .await
    .map_err(|error| -> RemoteError { RemoteError::unknown(format!("解析远程连接失败: {error}")) })?
}

/// 把前端的 `max_dimension` 夹到合法区间。
///
/// 0 会让 [`image_thumbnails::write_remote_thumbnail_bytes`] 去生成一张 0×0 的
/// 缩略图——那必然在管线很深的位置解码失败并写一条看不懂的错误日志。在门口就把它
/// 抬回 1，比让后面替它擦屁股便宜。
fn clamp_remote_thumbnail_dimension(max_dimension: u32) -> u32 {
    max_dimension.clamp(1, REMOTE_THUMBNAIL_MAX_DIMENSION)
}

/// 生成（或复用）一个远程图片文件的本地缩略图缓存。
///
/// 返回 `Ok(None)` 表示「这张图现在拿不到像素」，前端据此显示占位图，**不是错误**：
/// 不支持代取的 provider（ftp/sftp/百度网盘/夸克都落在 trait 默认实现上）、
/// 云端没生成缩略图、直链过期、体积超限、像素解码失败，全部归到这一类。
/// 只有连接本身出了问题（注册表里查不到、目录都建不出来）才走 `Err`。
#[tauri::command]
pub async fn remote_generate_thumbnail(
    app: tauri::AppHandle,
    path: String,
    modified_time: u64,
    max_dimension: u32,
) -> Result<Option<String>, String> {
    let dimension = clamp_remote_thumbnail_dimension(max_dimension);

    // 不是远程路径（本地盘、或者前端拼错了）在这里安静地判「没有缩略图」，
    // 不去查注册表，也不给前端报一条红色的错。
    let Some(parsed) = parse_remote_path(&path) else {
        return Ok(None);
    };
    let connection_id = parsed.connection_id;
    let remote_path = parsed.remote_path;

    let user_data_dir = user_data_dir(&app).map_err(|error| error.to_string())?;
    let app_data_dir =
        crate::app_dirs::app_data_dir(&app).map_err(|error| error.to_string())?;

    // `RemoteError` 没有 `Into<String>`（本 crate 刻意不给它 blanket 转换，
    // 避免哪条命令忘了脱敏就把内部错误直出给前端），所以这里显式转。
    let resolved = resolve_connection_blocking(
        user_data_dir.clone(),
        app_data_dir.clone(),
        connection_id.clone(),
    )
    .await
    .map_err(|error| error.to_string())?;
    // 注意 `provider_with_protocol` **不返回 Result**（构造失败被推迟到第一次调用），
    // 所以这里没有 `?`。
    let provider = provider_with_protocol(resolved.connection.protocol, &user_data_dir, &app_data_dir).await;

    let bytes = match provider
        .thumbnail_bytes(&connection_id, &remote_path, REMOTE_THUMBNAIL_MAX_DOWNLOAD_BYTES)
        .await
    {
        Ok(Some(bytes)) => bytes,
        // 拿不到像素是**常态**而非异常：trait 的默认实现就是这么约定的。
        Ok(None) => return Ok(None),
        // 错误文本由 provider 负责脱敏（签名 URL 不得出现在里面）。
        Err(error) => {
            log::debug!("[remote-diag] remote thumbnail fetch failed: {error}");
            return Ok(None);
        }
    };

    let cache_key =
        image_thumbnails::remote_thumbnail_cache_key(&connection_id, &remote_path, modified_time, dimension);
    let cache_dir = image_thumbnails::remote_thumbnail_cache_dir(&app)?;

    // 解码 + 缩放 + 落盘是 CPU 与磁盘活，不能在异步运行时的工作线程上原地做
    // （和本地 `generate_image_thumbnail` 同一个理由）。
    let staged = tauri::async_runtime::spawn_blocking(move || {
        image_thumbnails::write_remote_thumbnail_bytes(
            cache_dir,
            &remote_path,
            cache_key,
            bytes,
            dimension,
        )
    })
    .await
    .map_err(|error| format!("生成远程缩略图的任务被取消: {error}"))??;

    Ok(Some(staged))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 三个注册点里唯一有逻辑的一环：把云端来的路径切成「连接 + 云端路径」。
    /// 切错了会连到**别人的账号**上去，所以这条必须钉住。
    ///
    /// 前缀只能是 `sfm://remote/`（`remote/path.rs` 里的 `REMOTE_SCHEME`），**不是** `remote://`。
    /// 直觉写法 `remote://conn/x.jpg` 会被 `strip_prefix` 直接判 `None`，
    /// 所以这条用例同时是「防止有人按直觉把前缀改回去」的哨兵。
    #[test]
    fn splits_a_remote_path_into_connection_and_path() {
        let parsed = parse_remote_path("sfm://remote/conn-42/album/IMG_0001.jpg").unwrap();
        assert_eq!(parsed.connection_id, "conn-42");
        assert!(
            parsed.remote_path.starts_with('/'),
            "云端路径要归一成绝对形式，got {:?}",
            parsed.remote_path
        );
        assert!(
            parsed.remote_path.ends_with("IMG_0001.jpg"),
            "云端路径不能把文件名吃掉，got {:?}",
            parsed.remote_path
        );
    }

    /// 非远程路径必须解析失败 ⇒ 命令在门口就返回 `Ok(None)`。
    /// `sfm://remote`、`sfm://remote/`、`sfm://remote//pub` 这三条看着像远程路径，
    /// 但连接 id 为空或首段是空 id，放行就会拿空 id 去查注册表；
    /// 它们与 `remote://...` 一样都是 `remote/path.rs` 测试里钉死的 `None` 形状。
    #[test]
    fn non_remote_path_is_not_parsed() {
        assert!(parse_remote_path("C:\\Users\\me\\a.png").is_none());
        assert!(parse_remote_path("/home/me/a.png").is_none());
        assert!(parse_remote_path("sfm://remote").is_none());
        assert!(parse_remote_path("sfm://remote/").is_none());
        assert!(parse_remote_path("sfm://remote//pub").is_none());
        assert!(parse_remote_path("remote://conn-42/album/IMG_0001.jpg").is_none());
        assert!(parse_remote_path("").is_none());
    }

    #[test]
    fn clamps_the_frontend_dimension() {
        assert_eq!(clamp_remote_thumbnail_dimension(0), 1);
        assert_eq!(clamp_remote_thumbnail_dimension(384), 384);
        assert_eq!(
            clamp_remote_thumbnail_dimension(u32::MAX),
            REMOTE_THUMBNAIL_MAX_DIMENSION
        );
    }

    /// 代取的字节上限必须**低于**本地缩略图源文件的 128 MB 限额：
    /// 两条管线共用同一个解码器，如果远程上限反而更高，就会出现「本地一定拒、
    /// 远程偏要解」的裂缝，那条裂缝只在网盘上才踩得到，测试环境里永远看不见。
    #[test]
    fn download_ceiling_stays_below_the_local_source_limit() {
        assert!(REMOTE_THUMBNAIL_MAX_DOWNLOAD_BYTES < 128 * 1024 * 1024);
        assert!(REMOTE_THUMBNAIL_MAX_DOWNLOAD_BYTES >= 8 * 1024 * 1024);
    }

    /// 这条是**分层红线自身的回归哨兵**，不是业务测试，别删：
    /// [`RemoteProvider`](super::super::RemoteProvider) 必须保持 dyn 兼容。
    /// 一旦有人把 `provider_with_protocol` 改成分销泛型或给 trait 加了关联类型，
    /// 这个命令的分派会崩成一堆看不懂的报错，这里提前一步报警。
    ///
    /// 注意这里只写了**一个永远不会被调用的类型标注**，不实例化任何 provider，
    /// 更不调用 command（原因见文件头）。
    #[test]
    fn provider_stays_object_safe_for_the_proxy_command() {
        fn assert_object_safe(_provider: &dyn super::super::RemoteProvider) {}
        let _ = assert_object_safe;
    }
}
