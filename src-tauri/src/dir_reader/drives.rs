// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

use crate::utils::normalize_path;
use std::collections::HashSet;

#[cfg(target_os = "macos")]
use super::drives_platform::{
    append_macos_network_volumes, mount_point_last_component, should_skip_macos_mount,
};
#[cfg(windows)]
use super::drives_platform::{
    append_windows_logical_drives, append_windows_network_drives, append_windows_wsl_drives,
};
#[cfg(target_os = "linux")]
use super::drives_platform::{mount_point_last_component, should_skip_linux_mount};
use super::types::DriveInfo;
use sysinfo::Disks;

pub fn get_system_drives() -> Result<Vec<DriveInfo>, String> {
    let disks = Disks::new_with_refreshed_list();
    let mut drives: Vec<DriveInfo> = Vec::new();
    let mut seen_paths: HashSet<String> = HashSet::new();
    // FORK-MODIFICATION 2026-11-19：sysinfo **实际枚举到**的挂载点。
    //
    // 用来区分「sysinfo 压根没报这个盘」与「sysinfo 报了但我们把它过滤掉了」：
    // 前者才允许走 `GetLogicalDrives` 兜底补进来，后者（固定盘容量读不到）
    // 必须维持原样，否则会改掉固定盘的现有口径。
    let mut sysinfo_paths: HashSet<String> = HashSet::new();

    for disk in disks.iter() {
        let mount_point = disk.mount_point().to_string_lossy().to_string();
        let path = normalize_path(&mount_point);

        sysinfo_paths.insert(path.clone());

        let total_space = disk.total_space();
        let available_space = disk.available_space();

        #[cfg(target_os = "linux")]
        if total_space == 0
            || should_skip_linux_mount(
                &disk.file_system().to_string_lossy(),
                &disk.name().to_string_lossy(),
                &mount_point,
            )
        {
            continue;
        }

        #[cfg(target_os = "macos")]
        if total_space == 0 || should_skip_macos_mount(&mount_point) {
            continue;
        }

        // FORK-MODIFICATION 2026-11-19：容量读不到（0）的**可移动盘**不能丢 ——
        // U 盘 / 读卡器刚插上时容量偶尔读不到，丢了用户就看不到这个盘了。
        // 固定盘的口径不变（仍然丢弃）；可移动盘保留条目，占用显示 0。
        #[cfg(windows)]
        if total_space == 0 && !disk.is_removable() {
            continue;
        }

        if !seen_paths.insert(path.clone()) {
            continue;
        }

        let used_space = total_space.saturating_sub(available_space);
        let percent_used = if total_space > 0 {
            ((used_space as f64 / total_space as f64) * 100.0).round()
        } else {
            0.0
        };

        let file_system_str = disk.file_system().to_string_lossy().to_lowercase();
        let is_network_fs = matches!(
            file_system_str.as_str(),
            "nfs"
                | "nfs4"
                | "cifs"
                | "smbfs"
                | "fuse.sshfs"
                | "fuse.rclone"
                | "fuse.gvfsd-fuse"
                | "afpfs"
        );

        let drive_type = if is_network_fs {
            "Network".to_string()
        } else {
            match disk.kind() {
                sysinfo::DiskKind::HDD => "HDD".to_string(),
                sysinfo::DiskKind::SSD => "SSD".to_string(),
                sysinfo::DiskKind::Unknown(_) => "Unknown".to_string(),
            }
        };

        let display_name = {
            #[cfg(windows)]
            {
                let volume_label = disk.name().to_string_lossy().to_string();
                if volume_label.is_empty() {
                    format!("Local Disk ({})", mount_point.trim_end_matches('\\'))
                } else {
                    format!("{} ({})", volume_label, mount_point.trim_end_matches('\\'))
                }
            }
            #[cfg(target_os = "linux")]
            {
                if mount_point == "/" {
                    "/".to_string()
                } else {
                    mount_point_last_component(&mount_point)
                }
            }
            #[cfg(target_os = "macos")]
            {
                let volume_label = disk.name().to_string_lossy().to_string();
                if volume_label.is_empty() {
                    mount_point_last_component(&mount_point)
                } else {
                    volume_label
                }
            }
        };

        let device_path = disk.name().to_string_lossy().to_string();

        drives.push(DriveInfo {
            name: display_name,
            path,
            mount_point,
            file_system: disk.file_system().to_string_lossy().to_string(),
            drive_type,
            total_space,
            available_space,
            used_space,
            percent_used,
            is_removable: disk.is_removable(),
            is_read_only: disk.is_read_only(),
            is_mounted: true,
            device_path,
        });
    }

    #[cfg(target_os = "macos")]
    append_macos_network_volumes(&mut drives, &mut seen_paths);

    #[cfg(windows)]
    append_windows_network_drives(&mut drives, &mut seen_paths);

    #[cfg(windows)]
    append_windows_wsl_drives(&mut drives, &mut seen_paths);

    // FORK-MODIFICATION 2026-11-19：盘符兜底 —— `GetLogicalDrives` 报告存在、
    // 但 sysinfo 压根没报过的盘（含 `DRIVE_REMOVABLE` 的 U 盘 / 读卡器）。
    // 手机（MTP）没有盘符，这条兜底也救不了它 —— 手机走 shell namespace 管线。
    #[cfg(windows)]
    append_windows_logical_drives(&mut drives, &mut seen_paths, &sysinfo_paths);

    drives.sort_by(|first, second| first.path.cmp(&second.path));

    Ok(drives)
}
