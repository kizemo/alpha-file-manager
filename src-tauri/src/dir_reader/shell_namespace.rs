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

use super::types::{DirContents, DirEntry, ShellDeviceInfo, ShellPathCrumb};

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
    use super::super::types::{
        DirContents, DirEntry, OpenedDirectoryTimes, ShellDeviceInfo, ShellPathCrumb,
    };
    use std::ffi::c_void;
    use std::sync::OnceLock;
    use windows::core::{GUID, HSTRING, PCWSTR, Interface};
    use windows::Win32::Foundation::{FILETIME, HANDLE, HWND, SIZE};
    use windows::Win32::Graphics::Gdi::{
        BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDIBits, GetDC,
        GetObjectW, HBITMAP, HGDIOBJ, ReleaseDC,
    };
    use windows::Win32::System::Com::StructuredStorage::PropVariantToUInt64;
    use windows::Win32::System::Com::{CoInitialize, CoTaskMemFree, CoUninitialize, IBindCtx};
    use windows::Win32::System::SystemServices::{SFGAO_FLAGS, SFGAO_FOLDER};
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::Shell::PropertiesSystem::{
        PROPERTYKEY, PSGetPropertyKeyFromName,
    };
    use windows::Win32::UI::Shell::{
        IEnumShellItems, IShellItem, IShellItem2, IShellItemImageFactory, KF_FLAG_DEFAULT,
        SIGDN, SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_FILESYSPATH, SIGDN_NORMALDISPLAY,
        SIIGBF_THUMBNAILONLY, BHID_EnumItems, FOLDERID_ComputerFolder, SHCreateItemFromIDList,
        SHGetKnownFolderItem, SHParseDisplayName,
    };

    /// 「此电脑」的**解析名**（CLSID_MyComputer）。
    ///
    /// 注意别和 `FOLDERID_ComputerFolder` 搞混：后者是 known folder ID
    /// （{0AC0837C-BBF8-452A-850D-79D08E667CA7}），前者是 shell 命名空间的 CLSID
    /// （{20D04FE0-3AEA-1069-A2D8-08002B30309D}）—— 设备节点的解析名就是
    /// `::{20D04FE0-...}` 加上设备路径后缀，所以系统节点排除表里用的也是它。
    pub(super) const COMPUTER_FOLDER_PARSING_NAME: &str =
        "::{20D04FE0-3AEA-1069-A2D8-08002B30309D}";

    /// `System.Size` 的**兜底**属性键（取文件大小）。
    ///
    /// 批次 25 根因：原先这个 fmtid 被手写成
    /// `{9B174B33-40FF-11D2-A27E-00C04FC30871}` —— 那是「名称/序列号」一族的
    /// fmtid，**不是** `System.Size` 的。`IShellItem2::GetProperty` 对不认识的键
    /// 不报错，而是回 `S_OK` + 空值，`PropVariantToUInt64` 再把「读不到」静默转成 0
    /// ⇒ 手机（MTP/WPD）里所有文件的大小都显示 `0 B`。
    /// 真机取证见 `.superpowers\sdd\b25-thumb-probe.txt`：同一个条目用
    /// `{B725F130-47EF-101A-A5F1-02608C9EEBAC}` pid 12（`System.Size` 真正的
    /// fmtid，与下面 `PKEY_DATE_MODIFIED` 同族）读到 2285028 / 476981428 /
    /// 862016 等真实字节数，而用写错的键一律读到 0。
    ///
    /// 因此正式修复**不再以手写 GUID 为准**：优先让 propsys 自己解析
    /// `"System.Size"`（见 `system_size_key`），只有解析失败才退回这个常量。
    const PKEY_SIZE_FALLBACK: PROPERTYKEY = PROPERTYKEY {
        fmtid: GUID::from_u128(0xb725f130_47ef_101a_a5f1_02608c9eebac),
        pid: 12,
    };

    /// 解析出的 `System.Size` 属性键（进程内解析一次即可，propsys 给的是稳定常量）。
    static SYSTEM_SIZE_KEY: OnceLock<PROPERTYKEY> = OnceLock::new();

    /// 取 `System.Size` 的属性键：优先用 propsys 解析 `"System.Size"`。
    ///
    /// 让系统自己给出键，就不再有人有机会把 GUID 抄错。
    fn system_size_key() -> PROPERTYKEY {
        if let Some(key) = SYSTEM_SIZE_KEY.get() {
            return *key;
        }

        let mut key = PKEY_SIZE_FALLBACK;
        let name = HSTRING::from("System.Size");
        let mut resolved = PROPERTYKEY {
            fmtid: GUID::default(),
            pid: 0,
        };

        match unsafe { PSGetPropertyKeyFromName(&name, &mut resolved) } {
            Ok(()) => key = resolved,
            Err(error) => {
                log::warn!("[shell] 解析 System.Size 属性键失败，使用兜底键: {error}");
            }
        }

        let _ = SYSTEM_SIZE_KEY.set(key);
        key
    }

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
    /// **本机真机状态（2026-11-19 批次 24 复核）**：这台机器现在连着 MTP 手机
    /// 「一加 Ace 3」，`get_shell_devices()` 能枚举到它、`read_shell_dir()` 能读出它
    /// 下面的存储节点（`\SID-{...}`）⇒ 早先「这台机器上没有手机、此路未验证」的说法作废。
    /// 但实测这台手机 `portable=false`：它是靠**判据三**（无文件系统路径 + 文件夹形态）
    /// 入选的，并没有命中本函数的 `IPortableDevice` 接口 ⇒ 本判定在本机上仍属
    /// 「未被任何设备命中」的路径，所以判定逻辑一行没动。
    /// 测试 `shell_device_paths_resolve_to_navigable_chains` 会逐台打印 `portable=`，
    /// 便于分辨某台设备是靠判据二（WPD 接口）还是判据三（无盘符的文件夹形态）入选的。
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

    /// 祖先链的最大层数 —— 防环 / 防失控。
    ///
    /// shell 命名空间真实深度远小于这个数（设备 → 存储 → 目录），命中上限
    /// 只可能是 `GetParent()` 在某个命名空间扩展里绕圈，那时截断比死循环好。
    const MAX_SHELL_CHAIN_DEPTH: usize = 32;

    /// 这个解析路径是不是「此电脑」节点**本身**。
    ///
    /// 祖先链靠它决定在哪一层截断：面包屑的最外层是设备根，不含「此电脑」。
    /// 判定不能只认 `::{GUID}` 一种拼法 —— 同一个节点也可能被写成
    /// `shell:::{GUID}` / `shell:MyComputerFolder`（见 `lib.rs` 的启动参数过滤）。
    pub(super) fn is_computer_folder_parsing_name(parsing_name: &str) -> bool {
        let lower = parsing_name.trim().to_ascii_lowercase();

        let canonical = COMPUTER_FOLDER_PARSING_NAME.to_ascii_lowercase();

        lower == canonical
            || lower == format!("shell:{canonical}")
            || lower == "shell:mycomputerfolder"
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

    /// 读 shell 条目的文件大小（字节）。
    ///
    /// 「读不到」和「0 字节」是两件事：以前一律静默返回 0，界面上就成了
    /// 「这个文件是空的」，用户完全看不出错。现在失败会写日志，
    /// 0 只在该条目确实没有大小属性时出现。
    fn item_size(item: &IShellItem) -> u64 {
        let Ok(item_with_properties) = item.cast::<IShellItem2>() else {
            log::debug!("[shell] 条目不支持 IShellItem2，大小按未知处理");
            return 0;
        };

        let key = system_size_key();

        let Ok(property) = (unsafe { item_with_properties.GetProperty(&key) }) else {
            log::debug!("[shell] 条目没有 System.Size 属性（目录或未索引），大小按未知处理");
            return 0;
        };

        if property.is_empty() {
            log::debug!("[shell] System.Size 返回空值，大小按未知处理");
            return 0;
        }

        match unsafe { PropVariantToUInt64(&property) } {
            Ok(value) => value,
            Err(error) => {
                log::warn!("[shell] System.Size 转成整数失败: {error}");
                0
            }
        }
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
            entry_id: None,
            thumbnail_url: None,
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
    /// 从 shell 条目取缩略图并转成 RGBA 像素（批次 25：手机里的图片要能出图）。
    ///
    /// 手机（MTP/WPD）条目**不是文件系统路径** ⇒ `fs::metadata` 和 `image` 解码对它
    /// 必然失败，唯一能出图的通道是 shell 自己的 `IShellItemImageFactory::GetImage`。
    ///
    /// 真机取证见 `.superpowers\sdd\b25-thumb-probe.txt`：手机**图片**在
    /// `SIIGBF_THUMBNAILONLY` 下 64/128/256 三档全部成功；手机**视频**同一 flag 下
    /// 一律回 `0x8004B200`（没有缩略图提供程序），去掉该 flag 虽然能拿到位图，
    /// 但那很可能只是通用图标 ⇒ 宁可不出图，也不用假封面污染「每视频一份」的缓存。
    pub(super) fn shell_item_thumbnail_rgba(
        shell_path: &str,
        max_dimension: u32,
    ) -> Result<(u32, u32, Vec<u8>), String> {
        let item = resolve_shell_item(shell_path)?;
        let factory = item
            .cast::<IShellItemImageFactory>()
            .map_err(|error| format!("Shell 条目没有缩略图接口: {error}"))?;

        let side = i32::try_from(max_dimension.max(16))
            .map_err(|error| format!("缩略图尺寸超限: {error}"))?;

        let bitmap = unsafe { factory.GetImage(SIZE { cx: side, cy: side }, SIIGBF_THUMBNAILONLY) }
            .map_err(|error| format!("Shell 缩略图生成失败: {:08X}", error.code().0 as u32))?;

        let pixels = bitmap_to_rgba(bitmap);

        // GetImage 交回来的是 GDI 位图句柄，用完必须删，否则 GDI 对象泄漏。
        unsafe {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
        }

        pixels
    }

    /// `HBITMAP` →（宽, 高, RGBA8 像素）。
    ///
    /// 用 `GetDIBits` 以 32bpp、`biHeight` 取负（自上而下）一次取出整幅位图，
    /// 再把 GDI 的 BGRX 翻成 `image` 要的 RGBA。GDI 一律把 alpha 填 0，
    /// 照原样用会把缩略图洗成全透明 ⇒ 强制补成 255。
    fn bitmap_to_rgba(bitmap: HBITMAP) -> Result<(u32, u32, Vec<u8>), String> {
        let mut info = BITMAP::default();
        let info_read = unsafe {
            GetObjectW(
                HGDIOBJ(bitmap.0),
                i32::try_from(std::mem::size_of::<BITMAP>())
                    .map_err(|error| format!("位图信息长度异常: {error}"))?,
                Some(&mut info as *mut BITMAP as *mut c_void),
            )
        };
        if info_read == 0 {
            return Err("读取 Shell 缩略图位图信息失败".to_string());
        }

        let width = u32::try_from(info.bmWidth).map_err(|_| "位图宽度异常".to_string())?;
        let height = u32::try_from(info.bmHeight).map_err(|_| "位图高度异常".to_string())?;
        if width == 0 || height == 0 {
            return Err("Shell 缩略图位图为空".to_string());
        }

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let hdc = unsafe { GetDC(HWND::default()) };
        let scanlines = unsafe {
            GetDIBits(
                hdc,
                bitmap,
                0,
                height,
                Some(pixels.as_mut_ptr() as *mut c_void),
                &mut bmi,
                DIB_RGB_COLORS,
            )
        };
        unsafe {
            let _ = ReleaseDC(HWND::default(), hdc);
        }
        if scanlines == 0 {
            return Err("取出 Shell 缩略图像素失败".to_string());
        }

        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);

            if pixel[3] == 0 {
                pixel[3] = 255;
            }
        }

        Ok((width, height, pixels))
    }

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

    /// 祖先链：从**最外层（设备根）**一路到 `shell_path` 自身。
    ///
    /// 前端地址栏要把它拼成可逐级点击的面包屑。这件事前端做不了：
    /// 解析路径的段名不等于显示名（`SID-{10001,,1001925963776}` 显示成
    /// 「内部共享存储空间」），而且从「双击手机启动」那条路进来时，
    /// 前端手上只有末级一条路径，祖先一概不知 ⇒ 只能由后端沿
    /// `IShellItem::GetParent()` 往上走。
    pub(super) fn resolve_shell_path_chain(
        shell_path: &str,
    ) -> Result<Vec<ShellPathCrumb>, String> {
        let _com = ComGuard::new()?;

        // 复用对外那套定位逻辑（含前缀匹配退路）：能列目录的路径就必须能拉出链。
        let item = resolve_shell_item(shell_path)?;

        let mut chain: Vec<ShellPathCrumb> = Vec::new();
        let mut current = item;

        for depth in 0..MAX_SHELL_CHAIN_DEPTH {
            // 拿不到解析路径的层级**不进链**：面包屑每一级都要能被
            // `resolve_shell_item` / `read_shell_dir` 重新导航。
            let Some(parsing_name) = display_name(&current, SIGDN_DESKTOPABSOLUTEPARSING) else {
                break;
            };

            // 「此电脑」与桌面根不进链 —— 链在最外层那个设备根处终止。
            if is_computer_folder_parsing_name(&parsing_name) {
                break;
            }

            // 显示名为空时退回本级解析路径：绝不返回空 name。
            let name = display_name(&current, SIGDN_NORMALDISPLAY)
                .unwrap_or_else(|| parsing_name.clone());

            // 末级（= 入参这一层）原样回填**入参字符串**：它刚被 `resolve_shell_item`
            // 成功定位过，往返性已由本次调用保证；而 shell 有可能把同一节点写成
            // 另一种大小写/转义，前端拿到的链尾若与自己的当前路径不等，
            // 面包屑最后一级就点不回当前页。
            let crumb_path = if depth == 0 {
                if !parsing_name.eq_ignore_ascii_case(shell_path) {
                    log::debug!(
                        "[shell-namespace] 入参与 shell 回给的解析名不一致: input={shell_path} shell={parsing_name}"
                    );
                }

                shell_path.to_string()
            } else {
                parsing_name
            };

            // 环保护：`GetParent()` 给回同一层就地终止。
            let repeats_last = chain
                .last()
                .map(|last| last.path.eq_ignore_ascii_case(&crumb_path))
                .unwrap_or(false);

            if repeats_last {
                log::warn!("[shell-namespace] 祖先链出现重复层级，提前终止: {crumb_path}");
                break;
            }

            chain.push(ShellPathCrumb {
                path: crumb_path,
                name,
            });

            match unsafe { current.GetParent() } {
                Ok(parent) => current = parent,
                // 没有父级 = 已经顶到桌面根，链在此结束。
                Err(error) => {
                    log::debug!("[shell-namespace] 祖先链到顶: {error}");
                    break;
                }
            }
        }

        if chain.len() >= MAX_SHELL_CHAIN_DEPTH {
            log::warn!(
                "[shell-namespace] 祖先链触到深度上限 {MAX_SHELL_CHAIN_DEPTH}，已截断: {shell_path}"
            );
        }

        // 收集顺序是「由内向外」，面包屑要「由外向内」。
        chain.reverse();

        Ok(chain)
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

/// 取 shell namespace 条目（手机里的图片/视频）的缩略图 RGBA 像素。
///
/// 手机条目不是文件系统路径，`image` crate 解不了 ⇒ 必须由 shell 出图，
/// 判据与调用方见 `image_thumbnails::generate_image_thumbnail_file`。
/// 非 Windows 平台没有 shell namespace ⇒ 明确失败，绝不静默返回空图。
pub fn shell_item_thumbnail_rgba(
    shell_path: &str,
    max_dimension: u32,
) -> Result<(u32, u32, Vec<u8>), String> {
    #[cfg(windows)]
    {
        platform::shell_item_thumbnail_rgba(shell_path, max_dimension)
    }

    #[cfg(not(windows))]
    {
        let _ = (shell_path, max_dimension);
        Err("shell namespace 路径只支持 Windows".to_string())
    }
}

/// 把一个 shell namespace 路径拆成**从最外层到它自己**的祖先链，供面包屑逐级点击。
///
/// 为什么要单独一个接口：shell 路径（`::{…}\SID-{…}`）在人眼里是有层级的，
/// 但对 `Path` 来说只是一整串，双击启动时也只有**末级**路径有意义 ⇒
/// 前端光靠字符串切不出来，必须问 shell 本身。
///
/// 非 Windows 平台没有 shell namespace；非 shell 路径直接拒绝，
/// 别让它悄悄走到 COM 里再报一个看不懂的错。
pub fn resolve_shell_path_chain(shell_path: &str) -> Result<Vec<ShellPathCrumb>, String> {
    if !is_shell_namespace_path(shell_path) {
        return Err(format!("不是 shell namespace 路径: {shell_path}"));
    }

    #[cfg(windows)]
    {
        platform::resolve_shell_path_chain(shell_path)
    }

    #[cfg(not(windows))]
    {
        Err("shell namespace 路径只支持 Windows".to_string())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::platform::{
        computer_folder_item, display_name, enum_child_items, is_computer_folder_parsing_name,
        resolve_shell_item, COMPUTER_FOLDER_PARSING_NAME,
    };
    use super::{get_shell_devices, read_shell_dir, resolve_shell_path_chain};
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

    /// 非 shell 路径必须在进 COM 之前就被拒掉，并且错误里要带上原始路径，
    /// 否则前端只看到一个没有上下文的字符串，无法定位是哪一次点击坏了。
    #[test]
    fn non_shell_paths_are_rejected_with_the_original_path_in_the_error() {
        for path in ["C:\\Windows", "E:\\", "::20D04FE0-3AEA-1069-A2D8-08002B30309D", "/mnt/nas"] {
            let error = resolve_shell_path_chain(path)
                .expect_err("普通文件系统路径不该解析出 shell 祖先链");

            assert!(
                error.contains("不是 shell namespace 路径"),
                "错误应当说明原因: {error}"
            );
            assert!(
                error.contains(path),
                "错误应当带上原始路径: {error}"
            );
        }
    }

    /// 「此电脑」本身可解析，但按规范**不进链**（面包屑的最外层是设备根）⇒ 空链、不报错。
    /// 这条同时证明链的终止条件真的挂在计算机文件夹上，而不是一路摸到桌面根。
    #[test]
    fn computer_folder_itself_resolves_to_an_empty_chain() {
        let chain = resolve_shell_path_chain(COMPUTER_FOLDER_PARSING_NAME)
            .expect("「此电脑」是合法 shell 路径，不该报错");

        assert!(
            chain.is_empty(),
            "「此电脑」的祖先链应当为空，实际 {} 项: {:?}",
            chain.len(),
            chain
        );
    }

    /// 真机判别性测试：对每台枚举到的设备，从设备根往下钻两层，
    /// 每一层都要能拉出一条**可逐级导航**的祖先链，且链首正好是设备根。
    #[test]
    fn shell_device_paths_resolve_to_navigable_chains() {
        let devices = get_shell_devices().expect("设备枚举应该成功");

        println!(
            "[shell-namespace] 祖先链真机测试：本次枚举到 {} 台设备",
            devices.len()
        );

        if devices.is_empty() {
            println!("[shell-namespace] 本机没有枚举到 shell 设备，祖先链真机断言未判定");
            return;
        }

        for device in &devices {
            let item = resolve_shell_item(&device.shell_path).expect("设备根应该能定位");

            // 顺带打出「这台设备是靠哪条判据进设备列表的」：
            // portable=true 走 WPD 接口，false 走「无文件系统路径 + 文件夹形态」兜底。
            use windows::core::Interface as _;
            use windows::Win32::Devices::PortableDevices::IPortableDevice;

            let portable = item.cast::<IPortableDevice>().is_ok();

            println!(
                "[shell-namespace] 设备: name={} portable={portable} path={}",
                device.name, device.shell_path
            );

            let mut probes = vec![device.shell_path.clone()];

            // 沿「首个子目录」往下钻两级（手机：设备根 → 存储容器 → 一级相册目录）。
            for _ in 0..2 {
                let Ok(contents) = read_shell_dir(probes.last().expect("探针列表非空")) else {
                    break;
                };

                let Some(child) = contents.entries.iter().find(|entry| entry.is_dir) else {
                    break;
                };

                probes.push(child.path.clone());
            }

            for probe in &probes {
                assert_chain(probe, &device.shell_path, &device.name);
            }
        }
    }

    /// 对一条祖先链做四项断言：链首是设备根、末项与入参逐字相等、
    /// 每项都能被重新导航、链里不出现「此电脑」；并把链原文打印出来供人工核对。
    fn assert_chain(shell_path: &str, expected_root_path: &str, expected_root_name: &str) {
        let chain = resolve_shell_path_chain(shell_path)
            .unwrap_or_else(|error| panic!("祖先链解析失败: {shell_path} ({error})"));

        assert!(!chain.is_empty(), "祖先链不该为空: {shell_path}");

        println!("[shell-namespace] 祖先链入口: {shell_path}");

        for crumb in &chain {
            println!(
                "[shell-namespace]     name={} path={}",
                crumb.name, crumb.path
            );
        }

        // ① 链首 = 设备根，且显示名与 `get_shell_devices()` 给的那台设备一致。
        let root = chain.first().expect("非空链必有链首");

        assert!(
            root.path.eq_ignore_ascii_case(expected_root_path),
            "链首应当是设备根: 期望 {expected_root_path} 实际 {}",
            root.path
        );
        assert_eq!(
            root.name, expected_root_name,
            "链首显示名应当与设备列表给出的名字一致"
        );

        // ② 末项路径与入参**逐字**相等（前端靠它点回当前页）。
        let last = chain.last().expect("非空链必有末项");
        assert_eq!(last.path, shell_path, "链末必须原样回填入参路径");

        for crumb in &chain {
            // ③ 每一项都得能被重新导航（面包屑每一级都要能点）。
            resolve_shell_item(&crumb.path)
                .unwrap_or_else(|error| panic!("链中该项无法被重新导航: {} ({error})", crumb.path));

            // ④ 链里不出现「此电脑」，也不出现空显示名。
            assert!(
                !is_computer_folder_parsing_name(&crumb.path),
                "链里不该出现「此电脑」: {}",
                crumb.path
            );
            assert!(
                !crumb.name.is_empty(),
                "链里不该有空显示名: {}",
                crumb.path
            );
        }
    }

    /// 判别测试（批次 25）：shell 命名空间读出来的文件大小必须等于真实字节数。
    ///
    /// 需求本身是「手机（MTP/WPD）里的图片/视频大小不能显示成 `0 B`」。
    /// 这条判据不需要接手机：`read_shell_dir` / `stat_shell_entry` 对普通文件系统
    /// 路径同样可用，走的是**同一条 `item_size()`**。历史上属性键的 fmtid 写错，
    /// `GetProperty` 对这个未知键回 `S_OK` + `VT_EMPTY`，`PropVariantToUInt64`
    /// 把「读不到」静默转成 0 ⇒ 所有 shell 条目大小恒为 0 ⇒ 本测试改前必红。
    #[test]
    fn shell_namespace_reads_real_file_size() {
        let dir = tempfile::tempdir().expect("临时目录");
        let expected_size: u64 = 123_456;
        let sample = dir.path().join("b25-size-sample.bin");
        std::fs::write(&sample, vec![7u8; expected_size as usize]).expect("写入样本文件");

        let dir_path = dir.path().to_string_lossy().to_string();
        let contents = read_shell_dir(&dir_path).expect("shell 命名空间应能枚举普通目录");
        let entry = contents
            .entries
            .iter()
            .find(|item| item.name == "b25-size-sample.bin")
            .expect("目录里应能找到样本文件");
        assert_eq!(
            entry.size, expected_size,
            "read_shell_dir 读出的 size 必须等于真实字节数"
        );

        let sample_path = sample.to_string_lossy().to_string();
        let statted = super::stat_shell_entry(&sample_path).expect("stat_shell_entry 应能读取普通文件");
        assert_eq!(
            statted.size, expected_size,
            "stat_shell_entry 读出的 size 必须等于真实字节数"
        );
    }

    /// 真值来源（批次 25）：`System.Size` 的权威属性键必须由 propsys 自己解析，
    /// 而不是手写 GUID。
    ///
    /// 这条是**判据锚点**而非判别测试：它用 `PSGetPropertyKeyFromName("System.Size")`
    /// 拿到系统认可的键，再断言该键能读出真实字节数。产品代码必须与它同源。
    #[test]
    fn propsys_resolved_system_size_key_reads_real_file_size() {
        use windows::core::{GUID, HSTRING, Interface};
        use windows::Win32::System::Com::StructuredStorage::PropVariantToUInt64;
        use windows::Win32::UI::Shell::IShellItem2;
        use windows::Win32::UI::Shell::PropertiesSystem::{
            PSGetPropertyKeyFromName, PROPERTYKEY,
        };

        let dir = tempfile::tempdir().expect("临时目录");
        let expected_size: u64 = 4_224;
        let sample = dir.path().join("b25-key-sample.bin");
        std::fs::write(&sample, vec![0u8; expected_size as usize]).expect("写入样本文件");

        let path = sample.to_string_lossy().to_string();
        let item = resolve_shell_item(&path).expect("shell 应能解析普通文件路径");
        let item_with_properties: IShellItem2 = item.cast().expect("shell 条目应支持 IShellItem2");

        let mut key = PROPERTYKEY {
            fmtid: GUID::default(),
            pid: 0,
        };
        let name = HSTRING::from("System.Size");
        unsafe { PSGetPropertyKeyFromName(&name, &mut key) }
            .expect("propsys 应能解析 System.Size 属性键");

        let variant = unsafe { item_with_properties.GetProperty(&key) }
            .expect("propsys 解析出的 System.Size 键必须能读到属性");
        let value = unsafe { PropVariantToUInt64(&variant) }.expect("System.Size 应能转成 u64");
        assert_eq!(
            value, expected_size,
            "propsys 解析出的 System.Size 键必须读出真实字节数（产品代码必须与这条同源）"
        );
    }
}
