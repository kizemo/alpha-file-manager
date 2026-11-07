// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

use super::drives;
use super::mount;
use super::mountable;
use super::network_shares;
use super::path_helpers;
use super::read::{self, DirItemCountOptions, ReadDirOptions};
use super::types::{
    DirContents, DirEntryItemCount, DirEntryLinkMetadata, DriveInfo, MountableDevice,
    NetworkShareParams,
};

#[tauri::command]
pub async fn read_dir(
    path: String,
    options: Option<ReadDirOptions>,
) -> Result<DirContents, String> {
    tauri::async_runtime::spawn_blocking(move || read::read_dir(path, options))
        .await
        .map_err(|join_error| format!("Failed to read directory: {join_error}"))?
}

#[tauri::command]
pub async fn read_dir_with_timeout(
    path: String,
    timeout_ms: Option<u64>,
    options: Option<ReadDirOptions>,
) -> Result<DirContents, String> {
    read::read_dir_with_timeout(path, timeout_ms.unwrap_or(5000), options).await
}

#[tauri::command]
pub async fn get_dir_entry_with_timeout(
    path: String,
    timeout_ms: Option<u64>,
) -> Result<super::types::DirEntry, String> {
    read::get_dir_entry_with_timeout(path, timeout_ms.unwrap_or(2500)).await
}

#[tauri::command]
pub async fn get_link_metadata_batch(
    paths: Vec<String>,
    options: Option<ReadDirOptions>,
) -> Result<Vec<DirEntryLinkMetadata>, String> {
    tauri::async_runtime::spawn_blocking(move || read::get_link_metadata_batch(paths, options))
        .await
        .map_err(|join_error| format!("Failed to read link metadata: {join_error}"))
}

#[tauri::command]
pub async fn get_dir_item_counts_batch(
    paths: Vec<String>,
    options: Option<DirItemCountOptions>,
) -> Result<Vec<DirEntryItemCount>, String> {
    tauri::async_runtime::spawn_blocking(move || read::get_dir_item_counts_batch(paths, options))
        .await
        .map_err(|join_error| format!("Failed to read directory item counts: {join_error}"))
}

// FORK-MODIFICATION (2026-11-07)：「打开文件所在目录」的落点解析。
//
// 一次往返就把快捷方式解析成「要打开的目录 + 要选中的条目」：
//   - 目标**可以是文件**（绝大多数快捷方式指向文件），也接受目录目标；
//   - 断链 / 解析失败 / 超时一律静默回落（`.lnk` 自己的父目录 + `.lnk` 本身）。
//
// 为什么要搬到后端：早先在前端分两步做（`resolve_windows_directory_shortcut`
// + `path_exists`），两个问题都很致命 ——
//   1. 前那个命令**只认目录目标**，指向文件的快捷方式一律解析不出来，
//      于是「打开文件所在目录」回落到「打开 `.lnk` 自己所在的目录」——
//      用户看到的现象就是「点了没反应，没跳到快捷方式指向的文件所在目录」。
//   2. 两个都是**同步** Tauri command，跑在 Tauri 主线程上。COM 解析 /
//      网络盘 stat 一慢就阻塞 tao 事件循环 ⇒ WebView2 心跳断掉 ⇒
//      看门狗判「窗口卡死」⇒ 渲染进程崩溃（用户截图里的「界面渲染进程已崩溃」）。
//
// 这里用 `spawn_blocking` + 超时兜底。命令层是**唯一**允许碰
// `tauri::async_runtime` 的地方（`cargo test --lib` 的红线，见 `dir_reader.rs` 顶部注释）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealEntryTargetPayload {
    /// 要打开的目录。
    pub directory_path: String,
    /// 要选中的条目。
    pub focus_path: String,
    /// 落点是不是从快捷方式解析出来的。
    pub resolved_from_shortcut: bool,
}

#[tauri::command]
pub async fn resolve_reveal_entry_target(
    path: String,
    timeout_ms: Option<u64>,
) -> Result<RevealEntryTargetPayload, String> {
    #[cfg(windows)]
    {
        let timeout_ms = super::reveal::clamp_reveal_timeout_ms(
            timeout_ms.unwrap_or(super::reveal::DEFAULT_REVEAL_TARGET_TIMEOUT_MS),
        );

        let resolved = super::blocking_timeout::with_blocking_timeout(timeout_ms, move || {
            let location = super::reveal::resolve_reveal_entry_location(std::path::Path::new(&path));

            let directory_path = location.directory_path.to_string_lossy().into_owned();
            let focus_path = location.focus_path.to_string_lossy().into_owned();

            RevealEntryTargetPayload {
                directory_path: crate::utils::normalize_path(&directory_path),
                focus_path: crate::utils::normalize_path(&focus_path),
                resolved_from_shortcut: location.resolved_from_shortcut,
            }
        })
        .await;

        return match resolved {
            Ok(payload) => Ok(payload),
            Err(error) => Err(format!("Failed to resolve reveal entry target: {error:?}")),
        };
    }

    #[cfg(not(windows))]
    {
        let _ = (path, timeout_ms);
        Err("resolve_reveal_entry_target is only supported on Windows".to_string())
    }
}

// FORK-MODIFICATION (2026-11-07)：同步 command 跑在 Tauri 主线程上，
// COM 解析一慢就阻塞事件循环 ⇒ 挪到工作线程（语义不变，调用方仍是 Promise）。
#[tauri::command]
pub async fn resolve_windows_directory_shortcut(path: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || read::resolve_windows_directory_shortcut(path))
        .await
        .map_err(|join_error| {
            format!("Failed to resolve Windows directory shortcut: {join_error}")
        })?
}

#[tauri::command]
pub async fn get_system_drives() -> Result<Vec<DriveInfo>, String> {
    tauri::async_runtime::spawn_blocking(drives::get_system_drives)
        .await
        .map_err(|join_error| format!("Failed to enumerate system drives: {join_error}"))?
}

#[tauri::command]
pub fn get_mountable_devices() -> Result<Vec<MountableDevice>, String> {
    mountable::get_mountable_devices()
}

#[tauri::command]
pub fn mount_drive(device_path: String) -> Result<String, String> {
    mount::mount_drive(device_path)
}

#[tauri::command]
pub fn unmount_drive(device_path: String, mount_point: String) -> Result<(), String> {
    mount::unmount_drive(device_path, mount_point)
}

#[tauri::command]
pub fn disconnect_drive(
    device_path: String,
    mount_point: String,
    drive_type: String,
) -> Result<(), String> {
    mount::disconnect_drive(device_path, mount_point, drive_type)
}

#[tauri::command]
pub fn mount_network_share(params: NetworkShareParams) -> Result<String, String> {
    network_shares::mount_network_share(params)
}

#[tauri::command]
pub fn get_parent_dir(path: String) -> Option<String> {
    path_helpers::get_parent_dir(path)
}

// FORK-MODIFICATION (2026-11-07)：搬到工作线程。
//
// `Path::exists()` 在**网络盘**上可能卡上好几秒；同步 command 跑在 Tauri 主线程上，
// 一卡就阻塞 tao 事件循环 ⇒ WebView2 心跳断掉 ⇒ 看门狗判「窗口卡死」。
// 前端调用方式不变（`invoke` 对同步/异步 command 都是 Promise）。
#[tauri::command]
pub async fn path_exists(path: String) -> bool {
    tauri::async_runtime::spawn_blocking(move || path_helpers::path_exists(path))
        .await
        .unwrap_or(false)
}

#[tauri::command]
pub fn path_is_regular_file(path: String) -> bool {
    path_helpers::path_is_regular_file(path)
}

#[tauri::command]
pub async fn path_exists_with_timeout(path: String, timeout_ms: Option<u64>) -> Option<bool> {
    path_helpers::path_exists_with_timeout(path, timeout_ms.unwrap_or(2500)).await
}

#[tauri::command]
pub fn paths_are_directories(paths: Vec<String>) -> Vec<bool> {
    paths
        .into_iter()
        .map(|path| {
            std::path::Path::new(&path)
                .symlink_metadata()
                .map(|meta| meta.is_dir())
                .unwrap_or(false)
        })
        .collect()
}

#[tauri::command]
pub fn path_volume_is_case_sensitive(path: String) -> Result<bool, String> {
    super::path_volume::resolve_path_volume_case_sensitive(&path)
}

#[tauri::command]
pub fn path_comparison_volume_roots() -> Result<Vec<String>, String> {
    super::path_volume::resolve_path_comparison_volume_roots()
}

// FORK-MODIFICATION: 在 Windows Explorer 中选中文件（explorer.exe /select,<path>）。
// 用于目录树右键菜单的「打开文件所在目录」action。非 Windows 平台返回不支持错误。
//
// FORK-MODIFICATION (2026-11-01)：真正的逻辑挪到 `dir_reader::reveal`（纯内核，
// 不碰 tauri runtime —— `cargo test --lib` 的测试二进制一旦被拖进 tao/Wry，
// 加载阶段就会以 0xC0000139 崩掉，详见 `dir_reader.rs` 顶部注释）。
// 那边负责：绝对路径 explorer.exe、路径存在性校验、`.lnk` 目标解析、
// 带 path 的成功/失败日志，以及带 path 的错误串。
//
// FORK-MODIFICATION (2026-11-07)：同步 → async + `spawn_blocking`。
// 之前它跑在 Tauri 主线程上：COM 解析 + 起 explorer.exe 一慢就冻结整个应用
// （看门狗 60 秒后判「窗口卡死」，用户截图里是「渲染进程已崩溃」）。
#[tauri::command]
pub async fn reveal_in_explorer(path: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        return tauri::async_runtime::spawn_blocking(move || {
            super::reveal::reveal_in_explorer_blocking(&path)
        })
        .await
        .map_err(|join_error| format!("Failed to reveal in Windows Explorer: {join_error}"))?;
    }

    #[cfg(not(windows))]
    {
        let _ = path;
        Err("reveal_in_explorer is only supported on Windows".to_string())
    }
}
