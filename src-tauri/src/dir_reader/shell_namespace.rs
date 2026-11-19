// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! Windows shell namespace（`::{...}` / `shell:...`）的设备枚举与目录读取。
//!
//! 背景：「位置」页的盘符 100% 来自 `get_system_drives`，而它基于 `sysinfo::Disks`
//! （Windows 上走 Win32 卷枚举）⇒ **手机（MTP）/ 便携设备不是卷**，没有 volume GUID、
//! 没有盘符挂载点 ⇒ 永远进不了位置页。这是架构级缺口，不是 bug。
//!
//! 这里补一条独立的 shell 通道：枚举「此电脑」（`FOLDERID_ComputerFolder`）的子节点，
//! 把「不是真实盘」的那些（手机、U 盘、读卡器…）连同它们的
//! `SIGDN_DESKTOPABSOLUTEPARSING` 路径交给前端。
//!
//! 路径模型（整个方案的地基）：shell 路径**原样保留 Windows 给的字符串**
//! —— 不规范化、不把 `\` 换成 `/`、不按分隔符切分。前端把它当**不透明路径**，
//! 后端用 `SHParseDisplayName` 把它还原回 `IShellItem`。

use super::types::{DirContents, DirEntry, ShellDeviceInfo};

/// 判断一个路径是不是 shell namespace 路径（`::{...}` 或 `shell:...`）。
///
/// 纯字符串判断，跨平台可用：`read.rs` 的对外入口靠它分流到 shell 管线。
pub fn is_shell_namespace_path(path: &str) -> bool {
    let trimmed = path.trim();
    let lower = trimmed.to_ascii_lowercase();

    trimmed.starts_with("::{") || lower.starts_with("shell:")
}

// ---------------------------------------------------------------------------
// Windows 实现
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod platform {
    use super::extension_from_name;
    use super::super::types::{DirContents, DirEntry, OpenedDirectoryTimes, ShellDeviceInfo};
    use std::ffi::c_void;
    use windows::core::{GUID, PCWSTR, Interface};
    use windows::Win32::Foundation::{FILETIME, HANDLE};
    use windows::Win32::System::Com::StructuredStorage::PropVariantToUInt64;
    use windows::Win32::System::Com::{CoInitialize, CoTaskMemFree, CoUninitialize, IBindCtx};
    use windows::Win32::System::SystemServices::{SFGAO_FLAGS, SFGAO_FOLDER};
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY;
    use windows::Win32::UI::Shell::{
        IEnumShellItems, IShellItem, IShellItem2, KF_FLAG_DEFAULT, SIGDN,
        SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_FILESYSPATH, SIGDN_NORMALDISPLAY, BHID_EnumItems,
        FOLDERID_ComputerFolder, SHCreateItemFromIDList, SHGetKnownFolderItem, SHParseDisplayName,
    };

    /// 「此电脑」的**解析名**（CLSID_MyComputer）。
    ///
    /// 注意别和 `FOLDERID_ComputerFolder` 搞混：后者是 known folder ID
    /// （{0AC0837C-BBF8-452A-850D-79D08E667CA7}），前者是 shell 命名空间的 CLSID
    /// （{20D04FE0-3AEA-1069-A2D8-08002B30309D}）—— 设备节点的解析名就是
    /// `::{20D04FE0-...}` 加上设备路径后缀，所以系统节点排除表里用的也是它。
    pub(super) const COMPUTER_FOLDER_PARSING_NAME: &str =
        "::{20D04FE0-3AEA-1069-A2D8-08002B30309D}";

    /// `System.Size` 属性键（取文件大小）。
    const PKEY_SIZE: PROPERTYKEY = PROPERTYKEY {
        fmtid: GUID::from_u128(0x9b174b33_40ff_11d2_a27e_00c04fc30871),
        pid: 12,
    };

    /// `System.DateModified` 属性键（取修改时间）。
    const PKEY_DATE_MODIFIED: PROPERTYKEY = PROPERTYKEY {
        fmtid: GUID::from_u128(0xb725f130_47ef_101a_a5f1_02608c9eebac),
        pid: 14,
    };

    /// 已知系统节点的 GUID（**整个路径就是 `::{GUID}`** 时才算命中）。
    ///
    /// 这些节点在「此电脑」下可能出现，但它们不是设备，绝不能塞给前端。
    ///
    /// 注意：设备的解析名形如
    /// `::{20D04FE0-...}\\?\usb#vid_22d9&pid_2765...` —— **以「此电脑」的 GUID 开头**，
    /// 所以判定必须是「整体等于 `::{GUID}`」而不是「含有这个 GUID」，
    /// 否则会把手机误判成「此电脑」而丢掉。
    const KNOWN_SYSTEM_SHELL_GUIDS: &[&str] = &[
        "20d04fe0-3aea-1069-a2d8-08002b30309d", // 此电脑
        "21ec2020-3aea-1069-a2dd-08002b30309d", // 控制面板
        "21ec2020-3aea-1069-a2d8-08002b30309d", // 控制面板（另一个变体）
        "645ff040-5081-101b-9f08-00aa002f954e", // 回收站
        "d20ea4e3-3957-11d2-a40b-0c5020524153", // 管理工具
        "f42ee2d3-909f-4907-8871-4c22fc0bf756", // 网络连接
        "0bb4f1f3-9b33-4f31-ab2a-39d5b7a1b0ce", // 3D 对象
    ];

    /// COM 生命周期守卫。
    ///
    /// 这些函数跑在 `spawn_blocking` 派出来的工作线程上，那条线程**没有** COM 公寓，
    /// 不初始化就直接调 shell API 会拿到乱七八糟的失败码。
    pub(super) struct ComGuard;

    impl ComGuard {
        pub(super) fn new() -> Result<Self, String> {
            // `CoInitialize` 返回 HRESULT：S_OK / S_FALSE 都算成功，
            // 只有 RPC_E_CHANGED_MODE（线程已有别的公寓）才算失败。
            unsafe { CoInitialize(None) }
                .ok()
                .map_err(|error| format!("COM 初始化失败: {error:?}"))?;

            Ok(Self)
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }

    fn to_wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// 取 shell 项的某种显示名（`SIGDN_*`）。
    ///
    /// 空串按「没有」处理（`SIGDN_FILESYSPATH` 对非文件系统节点返回空串）。
    pub(super) fn display_name(item: &IShellItem, sigdn: SIGDN) -> Option<String> {
        unsafe {
            let pwstr = item.GetDisplayName(sigdn).ok()?;
            let text = if pwstr.is_null() {
                String::new()
            } else {
                pwstr.to_string().unwrap_or_default()
            };

            if !pwstr.is_null() {
                CoTaskMemFree(Some(pwstr.0 as *const c_void));
            }

            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
    }

    /// 解析名 → `IShellItem`。
    ///
    /// 走 `SHParseDisplayName` 拿 PIDL，再用 `SHCreateItemFromIDList` 换成 `IShellItem`
    /// —— 这条链路对 `::{GUID}`、`shell:...` 以及设备那种带 `\\?\usb#...` 后缀的
    /// 解析名都成立（**这是整个方案的地基：路径必须能往返**）。
    pub(super) fn shell_item_from_parsing_name(parsing_name: &str) -> Result<IShellItem, String> {
        let wide = to_wide(parsing_name);
        let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();

        unsafe {
            SHParseDisplayName(
                PCWSTR::from_raw(wide.as_ptr()),
                None::<&IBindCtx>,
                &mut pidl,
                0,
                None,
            )
            .map_err(|error| format!("无法解析 shell 路径 {parsing_name}: {error}"))?;
        }

        let item = unsafe { SHCreateItemFromIDList::<IShellItem>(pidl) };

        if !pidl.is_null() {
            unsafe { CoTaskMemFree(Some(pidl as *const c_void)) };
        }

        item.map_err(|error| format!("无法从 shell 路径创建条目 {parsing_name}: {error}"))
    }

    /// 「此电脑」的 shell 项。
    pub(super) fn computer_folder_item() -> Result<IShellItem, String> {
        let known_folder_item = unsafe {
            SHGetKnownFolderItem::<HANDLE, IShellItem>(
                &FOLDERID_ComputerFolder,
                KF_FLAG_DEFAULT,
                HANDLE(std::ptr::null_mut()),
            )
        };

        match known_folder_item {
            Ok(item) => Ok(item),
            Err(error) => {
                log::warn!("[shell-namespace] SHGetKnownFolderItem 失败，改用 GUID 解析: {error}");

                shell_item_from_parsing_name(COMPUTER_FOLDER_PARSING_NAME)
            }
        }
    }

    /// 枚举 shell 节点的子项。
    ///
    /// 用的是 `IShellItem::BindToHandler(BHID_EnumItems)`（现代 API，等价于旧接口
    /// `IShellFolder::EnumItems`）—— 它直接给出**绝对路径**的 `IShellItem`，
    /// 省掉「相对 PIDL 拼绝对 PIDL」这一步（拼错了就是一堆莫名其妙的解析失败）。
    pub(super) fn enum_child_items(item: &IShellItem) -> Result<Vec<IShellItem>, String> {
        let enumerator: IEnumShellItems =
            unsafe { item.BindToHandler::<Option<&IBindCtx>, IEnumShellItems>(None, &BHID_EnumItems) }
                .map_err(|error| format!("无法枚举 shell 节点子项: {error}"))?;

        let mut children = Vec::new();

        loop {
            let mut buffer: [Option<IShellItem>; 1] = [None];
            let mut fetched = 0u32;

            let result = unsafe {
                enumerator.Next(&mut buffer[..], Some(&mut fetched as *mut u32))
            };

            if fetched == 0 {
                break;
            }

            if let Some(child) = buffer[0].clone() {
                children.push(child);
            }

            if result.is_err() {
                break;
            }
        }

        Ok(children)
    }

    /// 解析 shell 路径 → `IShellItem`（**带退路**）。
    ///
    /// 主路是 `SHParseDisplayName`；它解析不了时走退路：从「此电脑」逐层枚举子项，
    /// 用「子项解析名是目标解析名的前缀」来匹配。
    ///
    /// 为什么要退路：设备节点（手机）的解析名形如
    /// `::{20D04FE0-...}\?\usb#vid_22d9&pid_2765...`，本机实测**有节点的解析名
    /// `SHParseDisplayName` 直接报 0x80070057（参数错误）** —— 只靠主路会
    /// 「枚举得到但打不开」。退路只依赖 shell 自己的父子结构，不依赖解析名可解析。
    pub(super) fn resolve_shell_item(shell_path: &str) -> Result<IShellItem, String> {
        // 主路：直接解析。
        if let Ok(item) = shell_item_from_parsing_name(shell_path) {
            return Ok(item);
        }

        log::debug!("[shell-namespace] SHParseDisplayName 解析失败，走退路: path={shell_path}");

        // 退路 1：逐级缩短解析名（每次砍掉最后一段 `\`），每缩短一级就再试一次
        // `SHParseDisplayName`；解析成功后在该节点的子项里按前缀找目标。
        // —— 「控制面板\字体」「设备\SID-{...}」这种两级路径靠这条走通。
        let mut prefix = shell_path;

        while let Some(cut_index) = prefix.rfind('\\') {
            prefix = &prefix[..cut_index];

            if prefix.is_empty() {
                break;
            }

            let Ok(parent) = shell_item_from_parsing_name(prefix) else {
                continue;
            };

            if let Ok(matched) = resolve_shell_item_by_prefix(&parent, shell_path) {
                return Ok(matched);
            }
        }

        // 退路 2：从「此电脑」按前缀逐级下钻（设备节点的解析名可能整段都解析不了）。
        let computer_folder = computer_folder_item()?;
        resolve_shell_item_by_prefix(&computer_folder, shell_path)
    }

    /// 逐级下钻：在 `parent` 的子项里找「解析名等于目标」或「解析名是目标前缀」的项。
    ///
    /// 前缀匹配是为了支持「设备 → 设备内部子目录」这种两级路径
    /// （`...\usb#...` 后面还跟着 `\SID-{...}`）。
    fn resolve_shell_item_by_prefix(
        parent: &IShellItem,
        target: &str,
    ) -> Result<IShellItem, String> {
        let children = enum_child_items(parent)?;

        for child in &children {
            let Some(child_path) = display_name(child, SIGDN_DESKTOPABSOLUTEPARSING) else {
                continue;
            };

            if child_path.eq_ignore_ascii_case(target) {
                return Ok(child.clone());
            }

            // 注意用 `get` 而不是切片 —— 避免非字符边界 panic。
            let is_prefix = target
                .get(..child_path.len())
                .map(|prefix| prefix.eq_ignore_ascii_case(&child_path))
                .unwrap_or(false);

            if is_prefix {
                if let Ok(matched) = resolve_shell_item_by_prefix(child, target) {
                    return Ok(matched);
                }
            }
        }

        Err(format!("无法定位 shell 路径: {target}"))
    }

    /// 是不是 WPD 设备（手机 / 便携设备）。
    ///
    /// 判据：`QueryInterface(IPortableDevice)` 成功 —— 这是 Windows Portable Devices
    /// 的标准接口，MTP 手机与部分读卡器都实现了它。
    ///
    /// **本机无法实证**：这台机器上没有手机，`QueryInterface` 这一支没被真机验证过，
    /// 需要用户插上设备后复测（见报告的「已知限制」）。
    fn is_portable_device_item(item: &IShellItem) -> bool {
        use windows::Win32::Devices::PortableDevices::IPortableDevice;

        item.cast::<IPortableDevice>().is_ok()
    }

    /// 是不是已知系统节点（控制面板 / 3D 对象 / 此电脑本身 / 回收站…）。
    ///
    /// 只在**整个路径就是 `::{GUID}`** 时命中；设备那种带后缀的解析名不受影响。
    fn is_known_system_shell_node(parsing_name: &str) -> bool {
        let lower = parsing_name.trim().to_ascii_lowercase();

        if lower.starts_with("shell:") {
            return true;
        }

        let Some(inner) = lower.strip_prefix("::{") else {
            return false;
        };

        let Some(closing_index) = inner.find('}') else {
            return false;
        };

        // `}` 后面还有内容 ⇒ 这是某个节点的**子路径**，不是节点本身。
        if closing_index + 1 != inner.len() {
            return false;
        }

        KNOWN_SYSTEM_SHELL_GUIDS.contains(&&inner[..closing_index])
    }

    fn item_is_folder(item: &IShellItem) -> bool {
        let attributes: SFGAO_FLAGS = match unsafe { item.GetAttributes(SFGAO_FOLDER) } {
            Ok(attributes) => attributes,
            Err(_) => {
                // 兜底：能枚举子项 ⇒ 是文件夹。
                return unsafe {
                    item.BindToHandler::<Option<&IBindCtx>, IEnumShellItems>(None, &BHID_EnumItems)
                        .is_ok()
                };
            }
        };

        // `SFGAO_FLAGS` 是位掩码新类型，直接按位判断（不依赖 `Flags` trait）。
        attributes.0 & SFGAO_FOLDER.0 != 0
    }

    fn item_size(item: &IShellItem) -> u64 {
        let Ok(item_with_properties) = item.cast::<IShellItem2>() else {
            return 0;
        };

        let Ok(property) = (unsafe { item_with_properties.GetProperty(&PKEY_SIZE) }) else {
            return 0;
        };

        unsafe { PropVariantToUInt64(&property).unwrap_or(0) }
    }

    fn item_modified_time(item: &IShellItem) -> u64 {
        let Ok(item_with_properties) = item.cast::<IShellItem2>() else {
            return 0;
        };

        let Ok(file_time) = (unsafe { item_with_properties.GetFileTime(&PKEY_DATE_MODIFIED) }) else {
            return 0;
        };

        filetime_to_unix_ms(file_time)
    }

    /// FILETIME（1601-01-01 起的 100ns 计数）→ Unix 毫秒。
    fn filetime_to_unix_ms(file_time: FILETIME) -> u64 {
        const TICKS_PER_MILLISECOND: u64 = 10_000;
        const UNIX_EPOCH_OFFSET_MILLISECONDS: u64 = 11_644_473_600_000;

        let ticks = ((file_time.dwHighDateTime as u64) << 32) | file_time.dwLowDateTime as u64;

        ticks
            .checked_div(TICKS_PER_MILLISECOND)
            .unwrap_or(0)
            .saturating_sub(UNIX_EPOCH_OFFSET_MILLISECONDS)
    }

    /// shell 项 → `DirEntry`。
    ///
    /// `path` 用 `SIGDN_DESKTOPABSOLUTEPARSING` 的**原样字符串**（不规范化）。
    pub(super) fn shell_item_to_dir_entry(item: &IShellItem) -> Option<DirEntry> {
        let name = display_name(item, SIGDN_NORMALDISPLAY)?;
        let path = display_name(item, SIGDN_DESKTOPABSOLUTEPARSING)?;

        let is_dir = item_is_folder(item);
        let size = if is_dir { 0 } else { item_size(item) };
        let modified_time = item_modified_time(item);

        Some(DirEntry {
            ext: extension_from_name(&name),
            name: name.clone(),
            path,
            size,
            item_count: None,
            modified_time,
            accessed_time: 0,
            created_time: 0,
            mime: None,
            is_file: !is_dir,
            is_dir,
            is_symlink: false,
            is_hidden: false,
            link_type: None,
            link_target: None,
            link_status: None,
            hard_link_count: None,
        })
    }

    /// 枚举「此电脑」下的**设备**节点。
    pub(super) fn collect_shell_devices() -> Result<Vec<ShellDeviceInfo>, String> {
        let _com = ComGuard::new()?;

        let computer_folder = computer_folder_item()?;
        let children = enum_child_items(&computer_folder)?;
        let mut devices = Vec::new();

        for child in &children {
            // 判据一：真实盘（`SIGDN_FILESYSPATH` 非空）由 `get_system_drives` 负责，跳过。
            let filesys_path = display_name(child, SIGDN_FILESYSPATH);
            if filesys_path.is_some() {
                continue;
            }

            let Some(shell_path) = display_name(child, SIGDN_DESKTOPABSOLUTEPARSING) else {
                continue;
            };

            // 判据二：WPD 设备（`QueryInterface(IPortableDevice)`）。
            let is_portable_device = is_portable_device_item(child);
            // 判据三（兜底）：没有文件系统路径、又是文件夹形态的节点 ——
            // shell 里的设备节点（手机 / U 盘）都长这样。
            let is_folder = item_is_folder(child);

            if !is_portable_device && !is_folder {
                continue;
            }

            // 已知系统节点（控制面板 / 3D 对象 / 回收站…）不是设备。
            if is_known_system_shell_node(&shell_path) {
                continue;
            }

            let name = display_name(child, SIGDN_NORMALDISPLAY).unwrap_or_else(|| shell_path.clone());

            log::debug!(
                "[shell-namespace] 设备节点: name={name} portable={is_portable_device} path={shell_path}"
            );

            devices.push(ShellDeviceInfo {
                name,
                shell_path,
                is_folder,
            });
        }

        Ok(devices)
    }

    /// 读一个 shell 节点的子项。
    pub(super) fn read_shell_dir(shell_path: &str) -> Result<DirContents, String> {
        let _com = ComGuard::new()?;

        let item = resolve_shell_item(shell_path)?;
        let children = enum_child_items(&item)?;

        let mut entries: Vec<DirEntry> = children
            .iter()
            .filter_map(|child| shell_item_to_dir_entry(child))
            .collect();

        entries.sort_by(|first, second| {
            (!first.is_dir, first.name.to_lowercase())
                .cmp(&(!second.is_dir, second.name.to_lowercase()))
        });

        let dir_count = entries.iter().filter(|entry| entry.is_dir).count();
        let file_count = entries.iter().filter(|entry| entry.is_file).count();

        Ok(DirContents {
            // shell 路径**原样保留**：规范化会把它改坏（`\` → `/`）。
            path: shell_path.to_string(),
            entries,
            total_count: dir_count + file_count,
            dir_count,
            file_count,
            opened_directory_times: OpenedDirectoryTimes {
                modified_time: 0,
                accessed_time: 0,
                created_time: 0,
            },
        })
    }

    /// 单个 shell 条目的 stat。
    pub(super) fn stat_shell_entry(shell_path: &str) -> Result<DirEntry, String> {
        let _com = ComGuard::new()?;

        let item = resolve_shell_item(shell_path)?;

        shell_item_to_dir_entry(&item)
            .ok_or_else(|| format!("无法读取 shell 条目: {shell_path}"))
    }
}

/// 从显示名里取扩展名（小写，没有则 `None`）。
///
/// shell 条目没有真实文件系统路径，只能从显示名推断扩展名。
fn extension_from_name(name: &str) -> Option<String> {
    let dot_index = name.rfind('.')?;

    if dot_index == 0 || dot_index + 1 >= name.len() {
        return None;
    }

    let extension = name[dot_index + 1..].to_ascii_lowercase();

    if extension.is_empty() {
        None
    } else {
        Some(extension)
    }
}

/// 枚举「此电脑」下的设备节点（手机 / U 盘 / 读卡器…）。
///
/// 非 Windows 平台返回空列表：那里没有 shell namespace 这回事。
pub fn get_shell_devices() -> Result<Vec<ShellDeviceInfo>, String> {
    #[cfg(windows)]
    {
        platform::collect_shell_devices()
    }

    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
}

/// 读一个 shell namespace 目录的子项。
pub fn read_shell_dir(shell_path: &str) -> Result<DirContents, String> {
    #[cfg(windows)]
    {
        platform::read_shell_dir(shell_path)
    }

    #[cfg(not(windows))]
    {
        let _ = shell_path;
        Err("shell namespace 路径只支持 Windows".to_string())
    }
}

/// 取单个 shell namespace 条目的信息。
pub fn stat_shell_entry(shell_path: &str) -> Result<DirEntry, String> {
    #[cfg(windows)]
    {
        platform::stat_shell_entry(shell_path)
    }

    #[cfg(not(windows))]
    {
        let _ = shell_path;
        Err("shell namespace 路径只支持 Windows".to_string())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::platform::{
        computer_folder_item, display_name, enum_child_items, resolve_shell_item,
        COMPUTER_FOLDER_PARSING_NAME,
    };
    use super::{get_shell_devices, read_shell_dir};
    use windows::Win32::UI::Shell::{SIGDN_FILESYSPATH, SIGDN_NORMALDISPLAY};

    /// 回环测试（方案的地基）：凡是本模块吐出来的 shell 路径，
    /// 都必须能被 `SHParseDisplayName` 重新解析回 `IShellItem`。
    #[test]
    fn shell_paths_round_trip_through_shell_parse_display_name() {
        let devices = get_shell_devices().expect("设备枚举应该成功");

        for device in &devices {
            assert!(
                device.shell_path.starts_with("::{") || device.shell_path.to_lowercase().starts_with("shell:"),
                "设备路径应当是 shell namespace 路径: {}",
                device.shell_path
            );

            resolve_shell_item(&device.shell_path).unwrap_or_else(|error| {
                panic!("设备路径无法被重新定位: {} ({error})", device.shell_path)
            });
        }

        let contents = read_shell_dir(COMPUTER_FOLDER_PARSING_NAME).expect("「此电脑」应该可枚举");

        for entry in &contents.entries {
            resolve_shell_item(&entry.path).unwrap_or_else(|error| {
                panic!("条目路径无法被重新定位: {} ({error})", entry.path)
            });
        }
    }

    /// 「此电脑」枚举测试：子项数量 ≥ 1，且真实盘符不会被当成设备返回。
    #[test]
    fn computer_folder_lists_children_and_real_drives_are_not_devices() {
        let _com = super::platform::ComGuard::new().expect("COM 初始化应该成功");

        let computer_folder = computer_folder_item().expect("应该能拿到「此电脑」");
        let children = enum_child_items(&computer_folder).expect("「此电脑」应该可枚举");

        assert!(
            !children.is_empty(),
            "「此电脑」下至少应该有 1 个子项（盘符也算）"
        );

        let devices = get_shell_devices().expect("设备枚举应该成功");

        for device in &devices {
            // 打印出来便于人工核对「这一台机器上到底枚举到了什么设备」，
            // 以及这些设备的目录能不能真的读出来。
            let readable = match super::read_shell_dir(&device.shell_path) {
                Ok(contents) => format!("可读，{} 个条目", contents.entries.len()),
                Err(error) => format!("不可读: {error}"),
            };

            println!(
                "[shell-namespace] 设备候选: name={} path={} 目录={readable}",
                device.name, device.shell_path
            );

            let item = resolve_shell_item(&device.shell_path).expect("设备路径应该能定位到 shell 条目");

            let filesys_path = display_name(&item, SIGDN_FILESYSPATH);

            assert!(
                filesys_path.is_none(),
                "真实盘不该被当成设备返回: {}（文件系统路径 {}）",
                device.name,
                filesys_path.unwrap_or_default()
            );

            let display = display_name(&item, SIGDN_NORMALDISPLAY).unwrap_or_default();
            assert!(!display.is_empty(), "设备应当有显示名");
        }
    }

    /// shell 目录读取测试：「此电脑」是阳性对照（必须能读到条目），
    /// 「控制面板」是可能不可枚举的节点（失败要能区分开）。
    #[test]
    fn shell_directories_can_be_read_and_their_entries_round_trip() {
        let contents = read_shell_dir(COMPUTER_FOLDER_PARSING_NAME)
            .expect("「此电脑」应该能读出条目（阳性对照）");

        assert!(
            !contents.entries.is_empty(),
            "「此电脑」下应该至少有盘符条目"
        );

        // shell 路径必须**原样保留**（不规范化、不把 `\` 换成 `/`）。
        assert_eq!(contents.path, COMPUTER_FOLDER_PARSING_NAME);

        for entry in &contents.entries {
            assert!(!entry.name.is_empty(), "条目必须有显示名");

            // 条目路径既可能是 shell 解析名（设备节点），也可能是普通盘符路径
            // （「此电脑」下的真实盘给的是 `E:\`）—— 两种都必须能被后端重新定位到。
            resolve_shell_item(&entry.path)
                .unwrap_or_else(|error| panic!("条目路径无法被重新定位: {} ({error})", entry.path));
        }

        // 「控制面板」不一定能枚举（不同 Windows 版本不一样）：
        // 失败只说明**这个节点**不可枚举，不代表管线坏了 —— 阳性对照已经过了。
        let control_panel = "::{21EC2020-3AEA-1069-A2DD-08002B30309D}";

        match read_shell_dir(control_panel) {
            Ok(control_panel_contents) => {
                for entry in &control_panel_contents.entries {
                    resolve_shell_item(&entry.path).unwrap_or_else(|error| {
                        panic!("控制面板条目路径无法被重新定位: {} ({error})", entry.path)
                    });
                }
            }
            Err(error) => {
                println!("[shell-namespace] 「控制面板」不可枚举（不影响结论）: {error}");
            }
        }
    }

    /// `get_system_drives` 兜底测试：结果里不能有重复项（同一挂载点只留一条）。
    #[test]
    fn system_drives_have_no_duplicates() {
        let drives = super::super::drives::get_system_drives().expect("盘符枚举应该成功");

        let mut paths = std::collections::HashSet::new();

        for drive in &drives {
            assert!(
                paths.insert(drive.path.clone()),
                "盘符重复: {}（{}）",
                drive.path,
                drive.name
            );

            let letter_count = drive
                .path
                .chars()
                .filter(|character| character.is_ascii_alphabetic())
                .count();

            assert!(
                letter_count <= 1,
                "盘符路径应当只含一个盘符字母: {}",
                drive.path
            );
        }
    }
}
