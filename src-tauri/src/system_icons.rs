// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use file_icon_provider::{get_file_icon, Error as FileIconError};
use image::codecs::png::PngEncoder;
use image::ImageEncoder;
use lru::LruCache;
use once_cell::sync::Lazy;
use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// 图标 data URL 缓存的条数上限。
///
/// 2026-10-28 从 512 降到 128：单条 base64 PNG 在 256px 时可达百 KB 级，
/// 512 条理论上能到 60 MB 常驻。key 里 `ext:<扩展名>:<尺寸>` 那一族是全目录共享的
/// （几十个扩展名 × 3 档尺寸），真正把槽位撑满的是 `path:<完整路径>:<尺寸>`
/// 那一族 —— 而它本来就是「每个 exe 各占一条」，被淘汰的代价最小。
const ICON_DATA_URL_CACHE_CAPACITY: usize = 128;

/// 「取不到图标」的路径标记的条数上限。
///
/// 2026-10-28 之前这个集合只增不减，是 Rust 侧唯一严格无界的容器。
/// 超出上限时按最旧优先淘汰：这些路径只是「取不到图标」的标记，
/// 命中与否只影响图标是否 fallback，淘汰掉最多让下次多试一次系统调用。
const MAX_UNSUPPORTED_SHELL_ICON_PATHS: usize = 512;

static ICON_DATA_URL_CACHE: Lazy<Mutex<LruCache<String, String>>> = Lazy::new(|| {
    Mutex::new(LruCache::new(
        NonZeroUsize::new(ICON_DATA_URL_CACHE_CAPACITY)
            .unwrap_or_else(|| NonZeroUsize::new(ICON_DATA_URL_CACHE_CAPACITY).unwrap()),
    ))
});

static ICON_PROBE_DIR: Lazy<Result<PathBuf, String>> = Lazy::new(|| {
    let dir = crate::app_dirs::local_data_dir_without_app("com.sigma-file-manager.app")
        .ok_or_else(|| "Could not resolve local data directory".to_string())?
        .join("icon-probes");
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir)
});

static UNSUPPORTED_SHELL_ICON_PATHS: Lazy<Mutex<LruCache<String, ()>>> = Lazy::new(|| {
    Mutex::new(LruCache::new(
        NonZeroUsize::new(MAX_UNSUPPORTED_SHELL_ICON_PATHS)
            .unwrap_or_else(|| NonZeroUsize::new(MAX_UNSUPPORTED_SHELL_ICON_PATHS).unwrap()),
    ))
});

fn normalize_path_for_os(path: &str) -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(path.replace('/', "\\"))
    }
    #[cfg(not(windows))]
    {
        PathBuf::from(path)
    }
}

fn has_unique_icon(extension: &Option<String>) -> bool {
    #[cfg(windows)]
    let unique_icon_extensions = [
        "exe", "dll", "ico", "lnk", "scr", "cpl", "msi", "appx", "msix",
    ];

    #[cfg(target_os = "linux")]
    let unique_icon_extensions = ["desktop", "appimage"];

    #[cfg(target_os = "macos")]
    let unique_icon_extensions = ["icns"];

    extension
        .as_ref()
        .map(|ext| unique_icon_extensions.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn file_icon_cache_key(path: &str, is_dir: bool, extension: &Option<String>, size: u16) -> String {
    if is_dir {
        return format!("dir:{}:{size}", path.to_lowercase());
    }

    let normalized_extension = extension
        .as_ref()
        .map(|ext| ext.to_lowercase())
        .unwrap_or_default();

    if has_unique_icon(extension) {
        format!("path:{}:{size}", path.to_lowercase())
    } else {
        format!("ext:{normalized_extension}:{size}")
    }
}

fn normalize_extension_for_probe(extension: &Option<String>) -> Option<String> {
    extension
        .as_ref()
        .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 16
                && value.chars().all(|character| {
                    character.is_ascii_alphanumeric() || character == '-' || character == '_'
                })
        })
}

fn icon_probe_path(extension: &Option<String>) -> Result<PathBuf, String> {
    let probe_dir = match ICON_PROBE_DIR.as_ref() {
        Ok(path) => path,
        Err(error) => return Err(error.clone()),
    };

    let file_name = match normalize_extension_for_probe(extension) {
        Some(extension_value) => format!("probe.{extension_value}"),
        None => "probe.txt".to_string(),
    };

    let probe_path = probe_dir.join(file_name);
    if !probe_path.exists() {
        fs::write(&probe_path, []).map_err(|error| error.to_string())?;
    }

    Ok(probe_path)
}

fn is_shell_icon_candidate_path(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed.contains("://") {
        return false;
    }

    #[cfg(windows)]
    {
        let lower = trimmed.to_ascii_lowercase();
        if trimmed.starts_with("::{") || lower.starts_with("shell:") {
            return false;
        }
    }

    true
}

pub(crate) fn is_windows_apps_alias_path(path: &str) -> bool {
    path.replace('/', "\\")
        .to_ascii_lowercase()
        .contains("\\microsoft\\windowsapps\\")
}

fn normalize_shell_icon_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(path.to_string_lossy().replace('/', "\\"))
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

#[cfg(windows)]
pub(crate) fn strip_shell_icon_index(path: &str) -> String {
    if let Some(comma_index) = path.rfind(',') {
        let suffix = path[comma_index + 1..].trim();
        if !suffix.is_empty()
            && suffix
                .chars()
                .all(|character| character.is_ascii_digit() || character == '-')
        {
            return path[..comma_index].to_string();
        }
    }

    path.to_string()
}

#[cfg(windows)]
pub(crate) fn has_shell_executable_icon_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "exe" | "dll" | "lnk" | "scr" | "cpl" | "msi" | "com" | "cmd" | "bat"
            )
        })
        .unwrap_or(false)
}

fn is_usable_shell_icon_target(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }

    if !path.exists() {
        return false;
    }

    if is_windows_apps_alias_path(&path.to_string_lossy()) {
        return false;
    }

    is_shell_icon_candidate_path(&path.to_string_lossy())
}

#[cfg(windows)]
fn is_shell_parsable_path(path: &Path) -> bool {
    use windows::core::HSTRING;
    use windows::Win32::System::Com::{CoInitialize, CoUninitialize};
    use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName};

    unsafe {
        if CoInitialize(None).is_err() {
            return false;
        }

        let path_string = HSTRING::from(path.as_os_str());
        let parsable =
            SHCreateItemFromParsingName::<_, _, IShellItemImageFactory>(&path_string, None).is_ok();
        CoUninitialize();
        parsable
    }
}

#[cfg(not(windows))]
fn is_shell_parsable_path(_path: &Path) -> bool {
    true
}

/// 记一条「取不到图标」的路径，返回 true 表示这是新记的。
///
/// 容量超限时 LRU 自动淘汰最旧的一条。语义不变：这些路径只是
/// 「取不到图标」的标记，命中与否只影响图标是否 fallback。
fn remember_unsupported_shell_icon_key(
    unsupported_paths: &mut LruCache<String, ()>,
    path_key: String,
) -> bool {
    unsupported_paths.put(path_key, ()).is_none()
}

fn remember_unsupported_shell_icon_path(path: &Path) -> bool {
    let path_key = path.to_string_lossy().to_ascii_lowercase();
    let Ok(mut unsupported_paths) = UNSUPPORTED_SHELL_ICON_PATHS.lock() else {
        return false;
    };

    remember_unsupported_shell_icon_key(&mut unsupported_paths, path_key)
}

fn is_remembered_unsupported_shell_icon_path(path: &Path) -> bool {
    let path_key = path.to_string_lossy().to_ascii_lowercase();
    UNSUPPORTED_SHELL_ICON_PATHS
        .lock()
        .ok()
        .is_some_and(|mut unsupported_paths| unsupported_paths.get(&path_key).is_some())
}

fn absolute_existing_path(path: &Path) -> Option<PathBuf> {
    if !path.exists() {
        return None;
    }

    if path.is_absolute() {
        return dunce::canonicalize(path)
            .ok()
            .filter(|resolved_path| is_usable_shell_icon_target(resolved_path))
            .or_else(|| {
                path.is_absolute()
                    .then(|| path.to_path_buf())
                    .filter(|resolved_path| is_usable_shell_icon_target(resolved_path))
            });
    }

    std::env::current_dir()
        .ok()
        .and_then(|cwd| dunce::canonicalize(cwd.join(path)).ok())
        .filter(|resolved_path| is_usable_shell_icon_target(resolved_path))
}

/// FORK-MODIFICATION 2026-11-06：`.lnk` 快捷方式的目标路径（取图标用）。
///
/// 解析不出来、目标不存在、目标是 WindowsApps 别名 —— 一律返回 `None`，
/// 由调用方回退到快捷方式自身的图标。
#[cfg(windows)]
fn resolve_shortcut_icon_target(path: &Path) -> Option<PathBuf> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();

    if extension != "lnk" {
        return None;
    }

    let target_path = PathBuf::from(crate::dir_reader::resolve_shortcut_target_path(path)?);

    if !target_path.is_absolute() || !target_path.exists() {
        return None;
    }

    if is_windows_apps_alias_path(&target_path.to_string_lossy()) {
        return None;
    }

    Some(target_path)
}

#[cfg(not(windows))]
fn resolve_shortcut_icon_target(_path: &Path) -> Option<PathBuf> {
    None
}

fn resolve_shell_icon_path(
    path: &str,
    is_dir: bool,
    extension: &Option<String>,
) -> Option<PathBuf> {
    if !is_shell_icon_candidate_path(path) {
        return None;
    }

    let normalized_path = normalize_path_for_os(path);

    if is_dir {
        return normalized_path
            .is_dir()
            .then(|| absolute_existing_path(&normalized_path))
            .flatten();
    }

    if normalized_path.is_file() {
        // FORK-MODIFICATION 2026-11-06：`.lnk` 显示**目标**的图标。
        //
        // 系统给快捷方式文件本身的是通用「快捷方式」图标；把链接解析成目标再取图标，
        // 才能显示 exe / 文档自己的图标。断链或解析失败时回退到 `.lnk` 自身。
        if let Some(shortcut_target) = resolve_shortcut_icon_target(&normalized_path) {
            if let Some(resolved_target) = absolute_existing_path(&shortcut_target) {
                return Some(resolved_target);
            }
        }

        return absolute_existing_path(&normalized_path);
    }

    absolute_existing_path(&icon_probe_path(extension).ok()?)
}

fn encode_icon_to_png_data_url(width: u32, height: u32, pixels: Vec<u8>) -> Result<String, String> {
    if width == 0 || height == 0 {
        return Err("Invalid icon dimensions".to_string());
    }

    let expected_len = width
        .checked_mul(height)
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| "Icon dimensions overflow".to_string())? as usize;

    if pixels.len() != expected_len {
        return Err("Unexpected icon pixel buffer length".to_string());
    }

    let mut png_bytes: Vec<u8> = Vec::new();
    let png_encoder = PngEncoder::new(&mut png_bytes);
    png_encoder
        .write_image(&pixels, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|error| error.to_string())?;

    let base64_png = BASE64_STANDARD.encode(png_bytes);
    Ok(format!("data:image/png;base64,{base64_png}"))
}

fn get_icon_data_url_uncached(path: &Path, size: u16) -> Result<String, String> {
    let shell_path = normalize_shell_icon_path(path);

    if !is_usable_shell_icon_target(&shell_path)
        || is_remembered_unsupported_shell_icon_path(&shell_path)
    {
        return Err("Unsupported shell icon path".to_string());
    }

    if !is_shell_parsable_path(&shell_path) {
        remember_unsupported_shell_icon_path(&shell_path);
        log::debug!(
            "[shell-icon] path is not shell-parsable path={}",
            shell_path.display(),
        );
        return Err("Unsupported shell icon path".to_string());
    }

    let icon = match get_file_icon(&shell_path, size) {
        Ok(icon) => icon,
        Err(FileIconError::PathDoesNotExist) => {
            return Err("Path does not exist".to_string());
        }
        Err(FileIconError::NullIconSize) => {
            return Err("Invalid icon size".to_string());
        }
        Err(FileIconError::Failed) => {
            remember_unsupported_shell_icon_path(&shell_path);
            log::debug!(
                "[shell-icon] get_file_icon failed path={} size={}",
                shell_path.display(),
                size,
            );
            return Err("Failed to get icon".to_string());
        }
    };

    encode_icon_to_png_data_url(icon.width, icon.height, icon.pixels)
}

pub(crate) fn get_path_icon_data_url(path: &Path, size: u16) -> Option<String> {
    get_icon_data_url_uncached(path, size).ok()
}

// FORK-MODIFICATION (2026-11-07)：搬到工作线程。
//
// `get_system_icon` 要做 COM 取图标 + PNG 编码，单次就不便宜；同步 command 跑在
// Tauri 主线程上，一个列表页几十个条目就能把事件循环堵死 ⇒ WebView2 心跳断掉
// ⇒ 看门狗判「窗口卡死」⇒ 渲染进程崩溃（2026-11-07 用户实测）。
// 纯逻辑挪到 `get_system_icon_blocking`（不碰 tauri runtime，可被单测链接）。
#[tauri::command]
pub async fn get_system_icon(
    path: String,
    is_dir: bool,
    extension: Option<String>,
    size: Option<u16>,
) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        get_system_icon_blocking(path, is_dir, extension, size)
    })
    .await
    .map_err(|join_error| format!("Failed to get system icon: {join_error}"))?
}

pub(crate) fn get_system_icon_blocking(
    path: String,
    is_dir: bool,
    extension: Option<String>,
    size: Option<u16>,
) -> Result<Option<String>, String> {
    let icon_size = size.unwrap_or(32).clamp(8, 256);
    let cache_key = file_icon_cache_key(&path, is_dir, &extension, icon_size);

    if let Ok(mut cache) = ICON_DATA_URL_CACHE.lock() {
        if let Some(cached_value) = cache.get(&cache_key) {
            return Ok(Some(cached_value.to_string()));
        }
    }

    let Some(icon_path) = resolve_shell_icon_path(&path, is_dir, &extension) else {
        return Ok(None);
    };

    let data_url_result = get_icon_data_url_uncached(&icon_path, icon_size);

    match data_url_result {
        Ok(data_url) => {
            if let Ok(mut cache) = ICON_DATA_URL_CACHE.lock() {
                cache.put(cache_key, data_url.clone());
            }
            Ok(Some(data_url))
        }
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_extension_for_probe_rejects_invalid_values() {
        assert_eq!(
            normalize_extension_for_probe(&Some("../evil".to_string())),
            None
        );
        assert_eq!(
            normalize_extension_for_probe(&Some("pdf".to_string())),
            Some("pdf".to_string())
        );
    }

    #[test]
    fn resolve_shell_icon_path_rejects_virtual_paths() {
        assert!(resolve_shell_icon_path("sfm://locations", true, &None).is_none());
    }

    #[test]
    fn resolve_shell_icon_path_rejects_shell_namespace_paths() {
        assert!(
            resolve_shell_icon_path("::{20D04FE0-3EA5-1069-A2D8-08002B30309D}", true, &None)
                .is_none()
        );
        assert!(resolve_shell_icon_path("shell:MyComputerFolder", true, &None).is_none());
    }

    #[test]
    fn icon_probe_path_is_absolute() {
        let probe_path = icon_probe_path(&Some("txt".to_string())).unwrap();
        assert!(probe_path.is_absolute());
        assert!(probe_path.exists());
    }

    #[test]
    fn windows_apps_alias_paths_are_rejected() {
        assert!(is_windows_apps_alias_path(
            r"C:\Users\example\AppData\Local\Microsoft\WindowsApps\wt.exe"
        ));
        assert!(!is_windows_apps_alias_path(
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"
        ));
    }

    #[test]
    #[cfg(windows)]
    fn windows_apps_alias_does_not_call_shell_icon_provider() {
        let windows_apps_wt = dirs::data_local_dir()
            .map(|dir| dir.join("Microsoft").join("WindowsApps").join("wt.exe"));

        let Some(path) = windows_apps_wt.filter(|candidate| candidate.exists()) else {
            return;
        };

        assert!(get_path_icon_data_url(&path, 32).is_none());
    }

    #[test]
    #[cfg(windows)]
    fn extension_probe_resolves_shell_icon() {
        for extension in ["txt", "pdf", "exe"] {
            let probe_path = icon_probe_path(&Some(extension.to_string())).unwrap();
            assert!(
                get_icon_data_url_uncached(&probe_path, 32).is_ok(),
                "shell icon lookup failed for extension {extension}"
            );
        }
    }

    #[test]
    #[cfg(windows)]
    fn strip_shell_icon_index_removes_numeric_suffix() {
        assert_eq!(
            strip_shell_icon_index(r"C:\Windows\System32\cmd.exe,0"),
            r"C:\Windows\System32\cmd.exe"
        );
    }

    #[test]
    #[cfg(windows)]
    fn forward_slash_paths_are_normalized_for_shell_lookup() {
        let forward_slash_path = PathBuf::from(r"C:/Windows/System32/cmd.exe");
        assert!(get_path_icon_data_url(&forward_slash_path, 32).is_some());
    }

    /// 「取不到图标」的路径标记必须有上限：它是 Rust 侧唯一严格无界的容器，
    /// 2026-10-28 之前只增不减。
    #[test]
    fn unsupported_shell_icon_paths_are_capped() {
        let mut unsupported_paths = LruCache::new(
            NonZeroUsize::new(MAX_UNSUPPORTED_SHELL_ICON_PATHS)
                .unwrap_or_else(|| NonZeroUsize::new(MAX_UNSUPPORTED_SHELL_ICON_PATHS).unwrap()),
        );

        for path_index in 0..(MAX_UNSUPPORTED_SHELL_ICON_PATHS + 64) {
            remember_unsupported_shell_icon_key(
                &mut unsupported_paths,
                format!("c:/fake/path-{path_index}.exe"),
            );
        }

        assert!(
            unsupported_paths.len() <= MAX_UNSUPPORTED_SHELL_ICON_PATHS,
            "路径标记条数 {} 超过了上限 {MAX_UNSUPPORTED_SHELL_ICON_PATHS}",
            unsupported_paths.len(),
        );
        assert_eq!(unsupported_paths.len(), MAX_UNSUPPORTED_SHELL_ICON_PATHS);
        assert!(
            !unsupported_paths.contains("c:/fake/path-0.exe"),
            "最早的路径应被淘汰"
        );
        assert!(
            unsupported_paths.contains(&format!(
                "c:/fake/path-{}.exe",
                MAX_UNSUPPORTED_SHELL_ICON_PATHS + 63
            )),
            "最新的路径应被保留"
        );
    }

    /// FORK-MODIFICATION 2026-11-06：`.lnk` 要能解析出目标，才能取到**目标的**图标。
    ///
    /// 这条用例同时覆盖正反两面：
    /// - 指向真实文件的快捷方式 ⇒ 解析出目标；
    /// - 指向已删除文件的断链快捷方式 ⇒ 解析不出目标（回退到快捷方式自身的图标）。
    #[test]
    #[cfg(windows)]
    fn shortcut_icon_target_resolves_lnk_target() {
        let temp_dir = std::env::temp_dir().join("afm-shortcut-icon-target-test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");

        let source_path = temp_dir.join("target.exe");
        std::fs::write(&source_path, b"afm-shortcut-icon-test").expect("write source file");
        let shortcut_path = temp_dir.join("target.lnk");
        crate::link_operations::create_shortcut(&source_path, &shortcut_path)
            .expect("create shortcut");
        assert!(shortcut_path.exists(), "shortcut file should exist");

        let resolved = crate::dir_reader::resolve_shortcut_target_path(&shortcut_path)
            .expect("shortcut target should resolve");
        assert!(
            resolved.to_ascii_lowercase().ends_with("target.exe"),
            "resolved target was {resolved}"
        );

        let icon_target =
            resolve_shortcut_icon_target(&shortcut_path).expect("icon target should resolve");
        assert_eq!(
            icon_target
                .file_name()
                .map(|name| name.to_string_lossy().to_string()),
            Some("target.exe".to_string())
        );

        let broken_shortcut_path = temp_dir.join("broken.lnk");
        crate::link_operations::create_shortcut(&temp_dir.join("missing.exe"), &broken_shortcut_path)
            .expect("create broken shortcut");

        assert!(
            resolve_shortcut_icon_target(&broken_shortcut_path).is_none(),
            "broken shortcut must not resolve to an icon target"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// 图标 data URL 缓存的条数上限被收紧到 128（2026-10-28 之前是 512）。
    #[test]
    fn icon_data_url_cache_capacity_stays_tightened() {
        assert_eq!(ICON_DATA_URL_CACHE_CAPACITY, 128);

        let cache_capacity = ICON_DATA_URL_CACHE
            .lock()
            .map(|cache| cache.cap().get())
            .unwrap_or_default();

        assert_eq!(cache_capacity, ICON_DATA_URL_CACHE_CAPACITY);
    }

        }
