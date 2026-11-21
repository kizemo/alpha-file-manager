// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirEntryLinkType {
    Symlink,
    Shortcut,
    Junction,
    Hardlink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirEntryLinkStatus {
    Valid,
    Broken,
    Unknown,
    Unsupported,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub ext: Option<String>,
    pub path: String,
    pub size: u64,
    pub item_count: Option<u32>,
    pub modified_time: u64,
    pub accessed_time: u64,
    pub created_time: u64,
    pub mime: Option<String>,
    pub is_file: bool,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub is_hidden: bool,
    pub link_type: Option<DirEntryLinkType>,
    pub link_target: Option<String>,
    pub link_status: Option<DirEntryLinkStatus>,
    pub hard_link_count: Option<u64>,
    /// 服务端给的稳定条目 ID（远程网盘用），本地条目恒为 `None`。
    ///
    /// 远程条目的 `path` 由「父目录路径 + 条目名」拼成，而同一个目录里**允许存在同名条目**
    /// ——阿里云盘真机实测（2026-11-21）在「手机备份/照片备份」下找到约 90 组同名文件。
    /// 两个不同条目的 `path` 因此逐字相同，选中判定若按 `path` 比较，
    /// 点一个条目就会同时高亮两个（用户 2026-11-21 报的 bug）。
    pub entry_id: Option<String>,
    /// 服务端给的缩略图直链（远程网盘用），本地条目恒为 `None`。
    ///
    /// 这是**签名 URL**：可以直接给 `<img src>` 用，但**不得**写进日志或用户可见文案。
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirEntryLinkMetadata {
    pub path: String,
    pub link_type: Option<DirEntryLinkType>,
    pub link_target: Option<String>,
    pub link_status: Option<DirEntryLinkStatus>,
    pub hard_link_count: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirEntryItemCount {
    pub path: String,
    pub item_count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenedDirectoryTimes {
    pub modified_time: u64,
    pub accessed_time: u64,
    pub created_time: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirContents {
    pub path: String,
    pub entries: Vec<DirEntry>,
    pub total_count: usize,
    pub dir_count: usize,
    pub file_count: usize,
    pub opened_directory_times: OpenedDirectoryTimes,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DriveInfo {
    pub name: String,
    pub path: String,
    pub mount_point: String,
    pub file_system: String,
    pub drive_type: String,
    pub total_space: u64,
    pub available_space: u64,
    pub used_space: u64,
    pub percent_used: f64,
    pub is_removable: bool,
    pub is_read_only: bool,
    pub is_mounted: bool,
    pub device_path: String,
}

/// FORK-MODIFICATION 2026-11-19：「此电脑」下的**设备**节点（手机 / U 盘 / 读卡器…）。
///
/// 这类节点不是 Win32 卷 ⇒ `get_system_drives`（基于 `sysinfo::Disks`）永远看不到它们。
/// `shell_path` 是 Windows 的 `SIGDN_DESKTOPABSOLUTEPARSING` 原样字符串
/// （以 `::{` 或 `shell:` 开头），前端必须当**不透明路径**处理
/// （不规范化、不按 `/` 切分）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellDeviceInfo {
    /// 显示名（例如 "iPhone"）。
    pub name: String,
    /// `SIGDN_DESKTOPABSOLUTEPARSING`，可被 `SHParseDisplayName` 重新解析。
    pub shell_path: String,
    pub is_folder: bool,
}

/// FORK-MODIFICATION 2026-11-19（批次 24）：shell 路径的**面包屑一级**。
///
/// shell 设备（手机 / MTP）的解析路径是不透明字符串：段名 ≠ 显示名
/// （`...\SID-{10001,,1001925963776}` 的显示名是「内部共享存储空间」），
/// 而且直接双击手机启动时前端只拿得到末级那一条路径 ⇒ 前端自己拆不出祖先链，
/// 只能由后端沿 `IShellItem::GetParent()` 往上走再返回。
///
/// 序列化沿用 `ShellDeviceInfo` 的 `camelCase`（两个字段都是单词，
/// 因此实际 JSON 键名固定为 `path` / `name`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellPathCrumb {
    /// `SIGDN_DESKTOPABSOLUTEPARSING` 原样字符串，可再次喂给 `resolve_shell_item`。
    pub path: String,
    /// `SIGDN_NORMALDISPLAY`；取不到时退回同级的 `path`，**绝不为空串**。
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MountableDevice {
    pub name: String,
    pub device_path: String,
    pub file_system: String,
    pub size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkShareParams {
    pub protocol: String,
    pub host: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub remote_path: String,
    pub mount_name: String,
    /// Windows 上要映射到的盘符（`"Z"` 或 `"Z:"`）。
    ///
    /// `None` 表示让 `net use *` 自动分配一个盘符（Windows 的惯例是从 Z 往下排）。
    /// 资源管理器的「映射网络驱动器」允许手选盘符，所以这里留了口子。
    ///
    /// **只在 Windows 上有意义**：其它平台的挂载点是目录，不是盘符。
    #[serde(default)]
    pub drive_letter: Option<String>,
}
