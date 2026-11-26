// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! B25-2 层 2：`WM_DEVICECHANGE` —— 弹出流程里把本进程持有的卷句柄放掉。
//!
//! ## 为什么要这一层
//!
//! 用户插着 U 盘点「安全弹出」，Windows 返回 `CR_REMOVE_VETOED (23)`。
//! 根因（见 `.superpowers/sdd/b25-usb-diagnosis.md`）是本进程对每个索引盘根
//! 持有一个**常驻**的递归 watch 句柄（`global_search/index_watcher.rs`），
//! 文件浏览器开着 U 盘目录时 `dir_watcher.rs` 也持一份。卷根句柄还在，
//! 系统就不允许卸载 —— 只能硬拔。
//!
//! 层 1（`watchable_drive_roots`）已经把可移动盘整体摘出索引根集合；
//! 这一层管的是**已经在跑**的进程遇到弹出请求时的动作：
//!
//! ```text
//! WM_DEVICECHANGE
//!   ├─ DBT_DEVICEQUERYREMOVE      弹出前先问一句 ⇒ 现在就放句柄，别等 veto
//!   ├─ DBT_DEVICEREMOVEPENDING    马上就要拔了 ⇒ 再放一次（幂等）+ 通知前端
//!   └─ DBT_DEVICEREMOVECOMPLETE   已经拔了 ⇒ 清掉残留登记 + 通知前端关标签页
//! ```
//!
//! 三个移除事件一律**先释放、不否决**。返回 `BROADCAST_QUERY_DENY` 就是在否决
//! 弹出，我们没有任何理由否决一个「用户主动要拔盘」的请求。
//!
//! ## 为什么这里不出现 `tauri::*`
//!
//! 本模块的纯逻辑要在 `cargo test --lib` 里跑。AGENTS.md 记录的那条坑是：
//! 只要测试二进制里被单态化出 `tauri::AppHandle`（默认 `R = Wry`），
//! tao 的整条窗口链就跟着进来，而测试二进制没有声明 comctl32 v6 的 manifest，
//! 进程在**加载阶段**以 `STATUS_ENTRYPOINT_NOT_FOUND (0xC0000139)` 崩掉。
//! 所以发送事件的动作由调用方（`lib.rs`，本来就满是 tauri）以闭包注入，
//! 这里只认识 `Fn(Vec<VolumeEventPayload>)`。
//!
//! 同理，`DEV_BROADCAST_*` 的指针解读留在 `platform` 里：
//! 纯的分派/判定层只吃已经解析好的 `DbtBroadcast`，可以用注入的记录器单测。

use std::sync::Mutex;

/// 发给前端的事件名。前端在 `use-drives.ts` 里监听。
pub(crate) const VOLUME_EVENT: &str = "volume-change";

/// `dbcv_unitmask` 的有效位区间：bit0 = `A:` … bit25 = `Z:`。
const DRIVE_LETTER_BITS: u32 = 26;

/// 广播的**事件类型**（`wParam`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DbtEventKind {
    /// 设备到位（插入）。
    Arrival,
    /// 弹出前的询问：这是最后一次能干净放句柄的机会。
    QueryRemove,
    /// 上一次询问被别的应用否决了，弹出取消 ⇒ 不需要放句柄，但要告诉前端「没拔掉」。
    QueryRemoveFailed,
    /// 即将移除。
    RemovePending,
    /// 移除完成。
    RemoveComplete,
    /// 类型相关事件（卷标签变化等走这条）。
    TypeSpecific,
    /// 设备节点树变了（`DBT_DEVNODES_CHANGED`）—— 只打日志，不动句柄。
    DevNodesChanged,
    /// 不认识的 `wParam`：必须原样留着，方便装机日志里对上新事件的码值。
    Other(u32),
}

/// `wParam` → 事件类型。分派表是封闭的，未知码不 panic。
pub(crate) fn classify_dbt_event(wparam: u32) -> DbtEventKind {
    match wparam {
        DBT_DEVICEARRIVAL => DbtEventKind::Arrival,
        DBT_DEVICEQUERYREMOVE => DbtEventKind::QueryRemove,
        DBT_DEVICEQUERYREMOVEFAILED => DbtEventKind::QueryRemoveFailed,
        DBT_DEVICEREMOVEPENDING => DbtEventKind::RemovePending,
        DBT_DEVICEREMOVECOMPLETE => DbtEventKind::RemoveComplete,
        DBT_DEVICETYPESPECIFIC => DbtEventKind::TypeSpecific,
        DBT_DEVNODES_CHANGED => DbtEventKind::DevNodesChanged,
        other => DbtEventKind::Other(other),
    }
}

// `windows` crate 里这些码是裸 `u32` 常量；这里自己声明一份，
// 好让纯逻辑层（以及非 Windows 编译）不依赖 feature 开关。
const DBT_DEVICEARRIVAL: u32 = 0x0000_8000;
const DBT_DEVICEQUERYREMOVE: u32 = 0x0000_8001;
const DBT_DEVICEQUERYREMOVEFAILED: u32 = 0x0000_8002;
const DBT_DEVICEREMOVEPENDING: u32 = 0x0000_8003;
const DBT_DEVICEREMOVECOMPLETE: u32 = 0x0000_8004;
const DBT_DEVICETYPESPECIFIC: u32 = 0x0000_8005;
const DBT_DEVNODES_CHANGED: u32 = 0x0007;

/// 这个事件是不是「要放句柄」的移除流程。
///
/// `QueryRemove` 也算：它在弹出**之前**，是唯一一次还能从容收尾的机会。
/// `QueryRemoveFailed` / `Arrival` / `TypeSpecific` / `DevNodesChanged` 都不放。
pub(crate) fn is_removal_event(kind: DbtEventKind) -> bool {
    matches!(
        kind,
        DbtEventKind::QueryRemove | DbtEventKind::RemovePending | DbtEventKind::RemoveComplete
    )
}

/// `dbcv_unitmask` → 盘符列表（bit0 = `A:`）。
///
/// 高位的 bit26（`0x0400_0000`）不是盘符，是「卷标签变化 / 无盘符变更」的标记位，
/// 必须忽略，否则会凭空造出一个 `AA:` 这样的鬼盘符。
pub(crate) fn drive_letters_from_unit_mask(unit_mask: u32) -> Vec<char> {
    (0..DRIVE_LETTER_BITS)
        .filter(|bit| unit_mask & (1 << bit) != 0)
        .map(|bit| char::from(b'A' + bit as u8))
        .collect()
}

/// 解读过的一帧广播（纯数据，便于单测）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DbtBroadcast {
    /// 卷广播：`dbcv_unitmask` 已展开成盘符（可能一次多盘）。
    Volume {
        kind: DbtEventKind,
        letters: Vec<char>,
    },
    /// 句柄广播：`DBT_DEVTYP_HANDLE`。**不能**被当成卷来解读 ——
    /// 它的 `dbch_handle` 与 `dbcv_unitmask` 在同一偏移上，误读会造出假盘符。
    Handle { kind: DbtEventKind },
    /// 设备节点树变化。
    DevNodesChanged { kind: DbtEventKind },
    /// 设备接口广播：`DBT_DEVTYP_DEVICEINTERFACE`。注册 `GUID_DEVINTERFACE_VOLUME`
    /// 之后系统投的就是这个类型；它带的是 `\\?\Volume{…}` 接口名，**没有盘符**
    /// ⇒ 不动句柄、不发事件。这是**已知类型**，不能和畸形包混为一谈。
    DeviceInterface { kind: DbtEventKind },
    /// 结构体尺寸不合法 / 类型不认识 —— 什么都不做，只留日志线索。
    Malformed { devicetype: u32 },
}

/// 一条广播的处理结果：要发给前端的东西，以及**要不要否决弹出**。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BroadcastOutcome {
    pub(crate) payloads: Vec<VolumeEventPayload>,
    /// `true` 表示要回 `BROADCAST_QUERY_DENY`。本模块任何路径都不该置真。
    pub(crate) veto: bool,
}

/// 发给前端的一帧。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VolumeEventPayload {
    /// `"arrival"` / `"query-remove"` / `"remove-pending"` / `"remove-complete"` / …
    pub(crate) kind: &'static str,
    /// 大写盘符（`"H"`）；句柄广播或未知类型为 `None`。
    pub(crate) drive_letter: Option<String>,
    /// 放掉了多少条 `dir_watcher` 登记（同一事件里重复释放会体现在计数上）。
    pub(crate) released_dir_watches: usize,
    /// 放掉了多少个索引 watcher 的盘根句柄。
    pub(crate) released_index_roots: usize,
}

impl DbtEventKind {
    pub(crate) fn code(self) -> &'static str {
        match self {
            DbtEventKind::Arrival => "arrival",
            DbtEventKind::QueryRemove => "query-remove",
            DbtEventKind::QueryRemoveFailed => "query-remove-failed",
            DbtEventKind::RemovePending => "remove-pending",
            DbtEventKind::RemoveComplete => "remove-complete",
            DbtEventKind::TypeSpecific => "type-specific",
            DbtEventKind::DevNodesChanged => "dev-nodes-changed",
            DbtEventKind::Other(_) => "other",
        }
    }
}

/// 句柄释放动作的注入面：单测里用记录器断言「释放了、而且目标卷是对的」。
pub(crate) trait Releaser {
    /// 释放 `dir_watcher` 在该卷上的登记，返回条数。`volume_root` 形如 `H:/`。
    fn release_dir_watches(&mut self, volume_root: &str) -> usize;
    /// 释放索引 watcher 对该盘根的 watch，返回个数。
    fn release_index_root(&mut self, volume_root: &str) -> usize;
}

/// 生产用的释放器：真的去叫那两个模块。
pub(crate) struct ProductionReleaser;

impl Releaser for ProductionReleaser {
    fn release_dir_watches(&mut self, volume_root: &str) -> usize {
        crate::dir_watcher::release_volume_watches(volume_root)
    }

    fn release_index_root(&mut self, volume_root: &str) -> usize {
        crate::global_search::release_root_watch(volume_root)
    }
}

/// 把盘符拼成卷根路径。`H` → `H:/`（索引侧与 dir_watcher 侧都用正斜杠归一化形式）。
fn volume_root_of(letter: char) -> String {
    format!("{}:/", letter.to_ascii_uppercase())
}

/// 处理一帧已解析的广播：释放句柄 → 组装要发的事件 → 决定是否否决。
///
/// 纯函数（副作用只经由注入的 `releaser`），所以三个移除事件的释放行为
/// 与释放目标都能用记录器断言。
pub(crate) fn handle_broadcast(
    broadcast: &DbtBroadcast,
    releaser: &mut dyn Releaser,
) -> BroadcastOutcome {
    match broadcast {
        DbtBroadcast::Volume { kind, letters } => {
            let mut payloads = Vec::with_capacity(letters.len());

            for letter in letters {
                let volume_root = volume_root_of(*letter);
                let drive_letter = Some(volume_root[..1].to_string());

                if is_removal_event(*kind) {
                    let released_dir_watches = releaser.release_dir_watches(&volume_root);
                    let released_index_roots = releaser.release_index_root(&volume_root);

                    log::info!(
                        "[device-events] {:?} on {}: released {} dir watch(es), {} index root(s)",
                        kind,
                        volume_root,
                        released_dir_watches,
                        released_index_roots
                    );

                    payloads.push(VolumeEventPayload {
                        kind: kind.code(),
                        drive_letter,
                        released_dir_watches,
                        released_index_roots,
                    });
                }
                else {
                    payloads.push(VolumeEventPayload {
                        kind: kind.code(),
                        drive_letter,
                        released_dir_watches: 0,
                        released_index_roots: 0,
                    });
                }
            }

            BroadcastOutcome { payloads, veto: false }
        }
        DbtBroadcast::Handle { kind } => {
            // 卷过滤器根本注册不上（见 `platform::registration_filter` 的注释），
            // 卷广播走的是系统对**顶层窗口**的自动投递。所以正常不会收到句柄广播；
            // 真收到了也只能说明「某个被通知的句柄所在的设备有动作」，
            // 拿不到盘符 ⇒ 不释放任何东西，只让前端刷新一次位置列表。
            // 否决弹出同样是禁止的。
            BroadcastOutcome {
                payloads: if matches!(kind, DbtEventKind::Arrival) || is_removal_event(*kind) {
                    vec![VolumeEventPayload {
                        kind: kind.code(),
                        drive_letter: None,
                        released_dir_watches: 0,
                        released_index_roots: 0,
                    }]
                }
                else {
                    Vec::new()
                },
                veto: false,
            }
        }
        DbtBroadcast::DeviceInterface { kind } => {
            // 注册卷接口 GUID 后收到的就是这一类。它只有 `\\?\Volume{…}`，
            // 既没有盘符也没有可安全解读的定长字段 ⇒ **不**动句柄、**不**发事件；
            // 盘符级的释放一律由上面的 Volume 分支负责。
            // 这里只留一条 debug 线索，绝不按 warn 打（那会把装机日志刷满）。
            log::debug!("[device-events] device-interface broadcast, kind={kind:?}");

            BroadcastOutcome { payloads: Vec::new(), veto: false }
        }
        DbtBroadcast::DevNodesChanged { .. } => {
            // 只打日志：整棵设备树的变化没有卷信息，动句柄反而危险。
            BroadcastOutcome { payloads: Vec::new(), veto: false }
        }
        DbtBroadcast::Malformed { devicetype } => {
            log::warn!("[device-events] unknown or truncated broadcast, devicetype={devicetype:#010x}");
            BroadcastOutcome { payloads: Vec::new(), veto: false }
        }
    }
}

type EmitterFn = Box<dyn Fn(Vec<VolumeEventPayload>) + Send + Sync + 'static>;

/// 发送器由 `lib.rs` 注入（这里不能出现 `tauri::AppHandle`，见模块头注释）。
static EMITTER: Mutex<Option<EmitterFn>> = Mutex::new(None);

fn emit(payloads: Vec<VolumeEventPayload>) {
    if payloads.is_empty() {
        return;
    }

    let emitter = match EMITTER.lock() {
        Ok(guard) => guard,
        Err(error) => return log::warn!("[device-events] emitter lock failed: {error}"),
    };

    if let Some(emitter) = emitter.as_ref() {
        emitter(payloads);
    }
}

/// 装好发送器并（Windows 上）起广播接收线程。重复调用只生效一次。
pub(crate) fn init<F>(emitter: F)
where
    F: Fn(Vec<VolumeEventPayload>) + Send + Sync + 'static,
{
    {
        let mut guard = match EMITTER.lock() {
            Ok(guard) => guard,
            Err(error) => {
                log::warn!("[device-events] failed to install emitter: {error}");
                return;
            }
        };

        if guard.is_some() {
            return;
        }
        *guard = Some(Box::new(emitter));
    }

    start();
}

#[cfg(windows)]
fn start() {
    if let Err(error) = std::thread::Builder::new()
        .name("device-events".to_string())
        .spawn(platform::run)
    {
        log::warn!("[device-events] failed to start device broadcast thread: {error}");
    }
}

#[cfg(not(windows))]
fn start() {
    // macOS / Linux 的弹出走各自的卷通知（Darwin 的 `unmountAndEjectDevice` 等），
    // 那套接线不在 B25-2 范围内；这里只保证 Windows 上的 veto 路径被堵住。
    log::debug!("[device-events] device broadcasts are wired for Windows only");
}

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;

    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostQuitMessage,
        RegisterClassExW, RegisterDeviceNotificationW, TranslateMessage, WNDCLASSEXW,
        DEV_BROADCAST_DEVICEINTERFACE_W, DEV_BROADCAST_HDR, DEV_BROADCAST_VOLUME,
        DBT_DEVTYP_DEVICEINTERFACE, DBT_DEVTYP_HANDLE, DBT_DEVTYP_VOLUME,
        DEVICE_NOTIFY_WINDOW_HANDLE, MSG, WM_DEVICECHANGE, WM_DESTROY, WM_QUIT, WS_EX_TOOLWINDOW,
        WS_POPUP,
    };

    use super::{
        classify_dbt_event, drive_letters_from_unit_mask, handle_broadcast, DbtBroadcast,
        DbtEventKind, ProductionReleaser, VOLUME_EVENT,
    };

    /// 窗口类名。窗口不显示，类名只要在本进程内唯一即可。
    const CLASS_NAME: &str = "AlphaFileManagerDeviceEvents";
    /// 窗口标题，同上，仅用于调试时辨认。
    const WINDOW_NAME: &str = "DeviceEventsWindow";

    /// `GUID_DEVINTERFACE_VOLUME` = `{53F5630D-B6BF-11D0-94F2-00A0C91EFB8B}`。
    ///
    /// windows 0.58 把它挂在 `Win32::System::Ioctl` 这个 feature 上，本 crate 没开
    /// （开它要把整个 `windows` crate 重编一遍）。值原样取自生成绑定
    /// `GUID_DEVINTERFACE_VOLUME = GUID::from_u128(0x53f5630d_b6bf_11d0_94f2_00a0c91efb8b)`；
    /// 判别探针用的也是这个字面量，注册返回 OK。字段由单测
    /// `registration_filter_targets_the_volume_interface_class` 钉死。
    const GUID_DEVINTERFACE_VOLUME: windows::core::GUID =
        windows::core::GUID::from_u128(0x53f5630d_b6bf_11d0_94f2_00a0c91efb8b);

    /// 喂给 `RegisterDeviceNotificationW` 的过滤器。
    ///
    /// 单独成函数只为一件事：让**字段值**能被纯单测钉住。上一版的缺陷就埋在这里
    /// —— `DBT_DEVTYP_VOLUME(2)` 的过滤器系统根本不接受，而它写在 `run()` 里，
    /// 单测一条都够不着，于是 11 条测试全绿、真机上功能等于没有。
    ///
    /// 判别证据：`.superpowers/sdd/b25d-probe-registration-v2.txt` 的 44 格矩阵
    /// （22 种 filter × message-only / 隐藏顶层窗两种窗口形态）。
    /// 凡 `dbch_devicetype = 2` 的组合——unitmask 全 1、只填 flags、只留 12 字节头、
    /// 把 size 字段改成 12/20——**全部** `HRESULT 0x8007000D (ERROR_INVALID_DATA)`；
    /// 只把 devicetype 换成 5（`DEV_BROADCAST_DEVICEINTERFACE_W` +
    /// `GUID_DEVINTERFACE_VOLUME`）就成功。⇒ 被拒的是设备类型本身。
    ///
    /// 注意：这**不**代表卷广播收不到了。带 unitMask 的卷广播由系统向顶层窗口
    /// 自动投递，无需（也无法）注册，解读路径见 `parse_broadcast` 的 Volume 分支。
    pub(super) fn registration_filter() -> DEV_BROADCAST_DEVICEINTERFACE_W {
        DEV_BROADCAST_DEVICEINTERFACE_W {
            dbcc_size: std::mem::size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>() as u32,
            dbcc_devicetype: DBT_DEVTYP_DEVICEINTERFACE.0,
            dbcc_reserved: 0,
            dbcc_classguid: GUID_DEVINTERFACE_VOLUME,
            dbcc_name: [0],
        }
    }

    /// `LPARAM` → 纯数据。只有这里碰原始指针；偏移/尺寸都按 `dbch_size` 校验过才读。
    ///
    /// # Safety
    /// 只在 `lparam` 指向系统刚投递的广播结构时调用（即 wndproc 内）。
    unsafe fn parse_broadcast(wparam: usize, lparam: isize) -> DbtBroadcast {
        let kind = classify_dbt_event(wparam as u32);

        // `DBT_DEVNODES_CHANGED` 不带结构体（`lParam` 为 0），只需要知道"发生了"。
        if matches!(kind, DbtEventKind::DevNodesChanged) {
            return DbtBroadcast::DevNodesChanged { kind };
        }

        if lparam == 0 {
            return DbtBroadcast::Malformed { devicetype: 0 };
        }

        // 安全检查：先把头部读出来，尺寸不够就绝不往前读。
        let header = &*(lparam as *const DEV_BROADCAST_HDR);
        let header_size = std::mem::size_of::<DEV_BROADCAST_HDR>() as u32;

        if header.dbch_size < header_size {
            return DbtBroadcast::Malformed {
                devicetype: header.dbch_devicetype.0,
            };
        }

        if header.dbch_devicetype == DBT_DEVTYP_VOLUME {
            if header.dbch_size < std::mem::size_of::<DEV_BROADCAST_VOLUME>() as u32 {
                return DbtBroadcast::Malformed {
                    devicetype: header.dbch_devicetype.0,
                };
            }

            let volume = &*(lparam as *const DEV_BROADCAST_VOLUME);

            return DbtBroadcast::Volume {
                kind,
                letters: drive_letters_from_unit_mask(volume.dbcv_unitmask),
            };
        }

        if header.dbch_devicetype == DBT_DEVTYP_HANDLE {
            // 只确认它是句柄广播，绝不按卷的偏移去读 `dbch_handle`。
            return DbtBroadcast::Handle { kind };
        }

        if header.dbch_devicetype == DBT_DEVTYP_DEVICEINTERFACE {
            // 我们注册的正是这一类（卷接口 GUID）。`dbcc_name` 是变长的
            // `\\?\Volume{…}`，不含盘符 ⇒ 只认类型，绝不去读名字。
            return DbtBroadcast::DeviceInterface { kind };
        }

        DbtBroadcast::Malformed {
            devicetype: header.dbch_devicetype.0,
        }
    }

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_DEVICECHANGE {
            let kind = classify_dbt_event(wparam.0 as u32);
            let parsed = parse_broadcast(wparam.0, lparam.0);

            if matches!(kind, DbtEventKind::DevNodesChanged) {
                // 设备树变化：只留日志线索（装机时用来核对「新代码在跑」）。
                match &parsed {
                    DbtBroadcast::DevNodesChanged { .. } => {
                        log::info!("[device-events] DBT_DEVNODES_CHANGED (log only)")
                    }
                    other => log::info!("[device-events] DBT_DEVNODES_CHANGED, parsed={other:?}"),
                }
                return LRESULT(0);
            }

            let outcome = handle_broadcast(&parsed, &mut ProductionReleaser);

            // 否决 = 用户在弹出时看到「无法停止设备」。这里永远不否决。
            if outcome.veto {
                log::warn!("[device-events] veto requested for {parsed:?}, refusing to deny removal");
            }

            super::emit(outcome.payloads);

            return LRESULT(0);
        }

        if msg == WM_DESTROY {
            PostQuitMessage(0);
            return LRESULT(0);
        }

        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    /// 线程主体：建隐藏顶层窗口 → 尽量注册卷接口广播 → 只跑消息循环。
    pub(super) fn run() {
        // `PCWSTR` 只是借用指针，所以这两个 Vec 必须活到 `run()` 结束。
        let class_wide: Vec<u16> = CLASS_NAME.encode_utf16().chain(std::iter::once(0)).collect();
        let window_wide: Vec<u16> = WINDOW_NAME.encode_utf16().chain(std::iter::once(0)).collect();

        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            lpszClassName: windows::core::PCWSTR::from_raw(class_wide.as_ptr()),
            ..Default::default()
        };

        let atom = unsafe { RegisterClassExW(&class) };
        if atom == 0 {
            log::warn!("[device-events] failed to register device window class");
            return;
        }

        // 隐藏**顶层**窗口：`WS_POPUP` + `WS_EX_TOOLWINDOW`，从不 `ShowWindow`
        // ⇒ 不进任务栏、不进 Alt-Tab，但它仍然是**顶层**窗口（父句柄为 0）。
        //
        // 这里不能再用 message-only（父窗口 `HWND_MESSAGE`）：带 unitMask 的卷广播
        // 是系统向顶层窗口**自动广播**的，message-only 窗口不在这条广播链上；
        // 而唯一能补上这段订阅的 `RegisterDeviceNotificationW` 又不接受卷过滤器
        // （见 `registration_filter`）。两件事合起来就是旧代码「日志报错 + 线程退出，
        // 功能等于没有」的完整成因。
        let hwnd = match unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                class.lpszClassName,
                windows::core::PCWSTR::from_raw(window_wide.as_ptr()),
                WS_POPUP,
                0,
                0,
                0,
                0,
                HWND(std::ptr::null_mut()),
                None,
                None,
                None,
            )
        } {
            Ok(hwnd) => hwnd,
            Err(error) => {
                log::warn!("[device-events] failed to create hidden top-level window: {error}");
                return;
            }
        };

        // 注册失败**不致命，绝不 return**。真正要收的卷广播靠顶层窗口自动投递，
        // 与这条注册无关；这里只是额外订阅卷接口（devicetype=5）作为线索。
        // 旧代码在这个位置 `return`，于是一次 API 拒绝就让整条监听线程退出 ——
        // 装机日志里的 `failed to register volume notifications: 数据无效。(0x8007000D)`
        // 加上成功行 `[device-events] listening for` 全量零命中，就是这么来的。
        let filter = registration_filter();

        match unsafe {
            RegisterDeviceNotificationW(
                hwnd,
                std::ptr::from_ref(&filter).cast::<c_void>(),
                DEVICE_NOTIFY_WINDOW_HANDLE,
            )
        } {
            Ok(_notification) => {
                log::info!("[device-events] volume device-interface notifications registered")
            }
            Err(error) => log::warn!(
                "[device-events] device-interface registration failed ({error}); relying on the \
                 automatic top-level volume broadcast"
            ),
        }

        log::info!("[device-events] listening for {VOLUME_EVENT} broadcasts on Windows");

        let mut message = MSG::default();

        loop {
            match unsafe { GetMessageW(&mut message, None, 0, 0) }.0 {
                0 | -1 => break,
                _ => {}
            }

            if message.message == WM_QUIT {
                break;
            }

            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- 必测 1：unitMask 位 → 盘符 --------------------------------------

    #[test]
    fn unit_mask_bits_expand_to_drive_letters() {
        // bit0 = A, bit7 = H。
        assert_eq!(drive_letters_from_unit_mask(1 << 7), vec!['H']);
        assert_eq!(drive_letters_from_unit_mask(1 | (1 << 7)), vec!['A', 'H']);
        // 多盘一次广播要按位序全部展开。
        assert_eq!(
            drive_letters_from_unit_mask((1 << 0) | (1 << 2) | (1 << 25)),
            vec!['A', 'C', 'Z']
        );
    }

    #[test]
    fn unit_mask_ignores_the_non_letter_high_bits() {
        assert!(drive_letters_from_unit_mask(0).is_empty());
        // bit26（0x0400_0000）是「卷标签变化」标记位，不是盘符。
        assert!(drive_letters_from_unit_mask(1 << 26).is_empty());
        assert!(drive_letters_from_unit_mask(0xFFFF_FFFF).len() == 26);
        assert_eq!(
            drive_letters_from_unit_mask(0xFFFF_FFFF).last(),
            Some(&'Z'),
            "位序必须从 A 排到 Z，不能凭空多出 AA:"
        );
    }

    // --- 必测 2：DBT 分派表 ----------------------------------------------

    #[test]
    fn dbt_wparam_dispatch_table_is_closed() {
        assert_eq!(classify_dbt_event(0x8000), DbtEventKind::Arrival);
        assert_eq!(classify_dbt_event(0x8001), DbtEventKind::QueryRemove);
        assert_eq!(
            classify_dbt_event(0x8002),
            DbtEventKind::QueryRemoveFailed
        );
        assert_eq!(classify_dbt_event(0x8003), DbtEventKind::RemovePending);
        assert_eq!(classify_dbt_event(0x8004), DbtEventKind::RemoveComplete);
        assert_eq!(classify_dbt_event(0x8005), DbtEventKind::TypeSpecific);
        assert_eq!(classify_dbt_event(0x0007), DbtEventKind::DevNodesChanged);
        // 未知码不得 panic，也不能被误判成移除事件。
        assert_eq!(classify_dbt_event(0xDEAD), DbtEventKind::Other(0xDEAD));
        assert_eq!(classify_dbt_event(0), DbtEventKind::Other(0));

        for kind in [
            DbtEventKind::QueryRemove,
            DbtEventKind::RemovePending,
            DbtEventKind::RemoveComplete,
        ] {
            assert!(is_removal_event(kind), "{kind:?} 必须触发释放");
        }
        for kind in [
            DbtEventKind::Arrival,
            DbtEventKind::QueryRemoveFailed,
            DbtEventKind::TypeSpecific,
            DbtEventKind::DevNodesChanged,
            DbtEventKind::Other(1234),
        ] {
            assert!(!is_removal_event(kind), "{kind:?} 不该触发释放");
        }
    }

    // --- 必测 3：三种移除事件都释放、且目标卷正确 -------------------------

    /// 记录器：只记下「谁被叫了、参数是什么」。
    struct RecordingReleaser {
        dir: Vec<String>,
        index: Vec<String>,
    }

    impl RecordingReleaser {
        fn new() -> Self {
            Self { dir: Vec::new(), index: Vec::new() }
        }
    }

    impl Releaser for RecordingReleaser {
        fn release_dir_watches(&mut self, volume_root: &str) -> usize {
            self.dir.push(volume_root.to_string());
            2
        }

        fn release_index_root(&mut self, volume_root: &str) -> usize {
            self.index.push(volume_root.to_string());
            1
        }
    }

    fn volume(kind: DbtEventKind, letters: &[char]) -> DbtBroadcast {
        DbtBroadcast::Volume { kind, letters: letters.to_vec() }
    }

    #[test]
    fn every_removal_event_releases_both_watchers_on_the_right_volume() {
        for kind in [
            DbtEventKind::QueryRemove,
            DbtEventKind::RemovePending,
            DbtEventKind::RemoveComplete,
        ] {
            let mut releaser = RecordingReleaser::new();
            let outcome = handle_broadcast(&volume(kind, &['H']), &mut releaser);

            assert_eq!(releaser.dir, vec!["H:/".to_string()], "{kind:?} 目标卷不对");
            assert_eq!(releaser.index, vec!["H:/".to_string()], "{kind:?} 目标卷不对");
            assert!(!outcome.veto, "{kind:?} 绝不许否决弹出");
            assert_eq!(outcome.payloads.len(), 1);
            assert_eq!(outcome.payloads[0].drive_letter.as_deref(), Some("H"));
            assert_eq!(outcome.payloads[0].kind, kind.code());
            assert_eq!(outcome.payloads[0].released_dir_watches, 2);
            assert_eq!(outcome.payloads[0].released_index_roots, 1);
        }
    }

    #[test]
    fn arrival_and_query_remove_failed_release_nothing() {
        for kind in [DbtEventKind::Arrival, DbtEventKind::QueryRemoveFailed] {
            let mut releaser = RecordingReleaser::new();
            let outcome = handle_broadcast(&volume(kind, &['H']), &mut releaser);

            assert!(releaser.dir.is_empty(), "{kind:?} 不该释放句柄");
            assert!(releaser.index.is_empty(), "{kind:?} 不该释放句柄");
            assert!(!outcome.veto);
            assert_eq!(outcome.payloads.len(), 1, "{kind:?} 仍要通知前端刷新");
            assert_eq!(outcome.payloads[0].released_dir_watches, 0);
        }
    }

    #[test]
    fn multi_volume_broadcast_releases_each_letter() {
        let mut releaser = RecordingReleaser::new();
        let outcome = handle_broadcast(&volume(DbtEventKind::RemovePending, &['H', 'J']), &mut releaser);

        assert_eq!(releaser.dir, vec!["H:/".to_string(), "J:/".to_string()]);
        assert_eq!(releaser.index, vec!["H:/".to_string(), "J:/".to_string()]);
        assert_eq!(outcome.payloads.len(), 2);
        assert!(!outcome.veto);
    }

    #[test]
    fn handle_broadcast_is_never_misparsed_as_a_volume() {
        // 句柄广播的 dbch_handle 与卷的 dbcv_unitmask 偏移不同：
        // 误按卷解读会凭空冒出一堆盘符并释放错卷。
        let mut releaser = RecordingReleaser::new();
        let outcome = handle_broadcast(
            &DbtBroadcast::Handle { kind: DbtEventKind::RemoveComplete },
            &mut releaser,
        );

        assert!(releaser.dir.is_empty());
        assert!(releaser.index.is_empty());
        assert!(!outcome.veto);
        assert_eq!(outcome.payloads.len(), 1);
        assert_eq!(outcome.payloads[0].drive_letter, None);
    }

    #[test]
    fn devnodes_changed_only_logs() {
        let mut releaser = RecordingReleaser::new();
        let outcome = handle_broadcast(
            &DbtBroadcast::DevNodesChanged { kind: DbtEventKind::DevNodesChanged },
            &mut releaser,
        );

        assert!(outcome.payloads.is_empty(), "设备树变化不往前端发卷事件");
        assert!(releaser.dir.is_empty());
        assert!(releaser.index.is_empty());
        assert!(!outcome.veto);
    }

    #[test]
    fn malformed_broadcast_is_dropped_without_vetoing() {
        let mut releaser = RecordingReleaser::new();
        let outcome = handle_broadcast(
            &DbtBroadcast::Malformed { devicetype: 0x0000_0009 },
            &mut releaser,
        );

        assert!(outcome.payloads.is_empty());
        assert!(releaser.dir.is_empty());
        assert!(!outcome.veto);
    }

    #[test]
    fn removal_event_code_strings_are_stable_for_the_frontend() {
        assert_eq!(DbtEventKind::QueryRemove.code(), "query-remove");
        assert_eq!(DbtEventKind::RemovePending.code(), "remove-pending");
        assert_eq!(DbtEventKind::RemoveComplete.code(), "remove-complete");
        assert_eq!(DbtEventKind::Arrival.code(), "arrival");
    }

    #[test]
    fn volume_root_of_is_upper_case_with_trailing_slash() {
        assert_eq!(volume_root_of('h'), "H:/");
        assert_eq!(volume_root_of('Z'), "Z:/");
    }

    // --- 必测 4：注册过滤器的字段值 ---------------------------------------
    //
    // B25-2 装机失败的根因就是这个结构体的 `dbcc_devicetype`：
    // `DBT_DEVTYP_VOLUME(2)` 的过滤器 `RegisterDeviceNotificationW` 直接回
    // `0x8007000D`（判别探针 `.superpowers/sdd/b25d-probe-registration-v2.txt`，
    // 22 种 filter × 2 种窗口形态共 44 格全部实测）。
    // 原来这段构造埋在 `platform::run()` 里，单测一条都够不着 ⇒ 11 条测试全绿
    // 而真机功能为零。所以把构造抽成 `platform::registration_filter()` 后钉住它。
    //
    // 注意钉的是**注册用的 filter**，不是收到的广播：卷广播本身仍是 devicetype=2
    // 的 `DEV_BROADCAST_VOLUME`（由系统向顶层窗口自动投递，见 parse 侧的 Volume 分支）。

    #[cfg(windows)]
    #[test]
    fn registration_filter_targets_the_volume_interface_class() {
        use windows::Win32::UI::WindowsAndMessaging::{
            DBT_DEVTYP_DEVICEINTERFACE, DBT_DEVTYP_VOLUME, DEV_BROADCAST_DEVICEINTERFACE_W,
        };

        let filter = super::platform::registration_filter();

        assert_eq!(filter.dbcc_devicetype, DBT_DEVTYP_DEVICEINTERFACE.0);
        assert_eq!(
            filter.dbcc_devicetype, 5,
            "devicetype 必须是 DBT_DEVTYP_DEVICEINTERFACE 的字面值 5"
        );
        assert_ne!(
            filter.dbcc_devicetype,
            DBT_DEVTYP_VOLUME.0,
            "回归不变量：DBT_DEVTYP_VOLUME(2) 的过滤器系统不接受，绝不能再拿它去注册"
        );
        assert_eq!(
            filter.dbcc_classguid,
            windows::core::GUID::from_u128(0x53f5630d_b6bf_11d0_94f2_00a0c91efb8b),
            "必须是 GUID_DEVINTERFACE_VOLUME {{53F5630D-B6BF-11D0-94F2-00A0C91EFB8B}}"
        );
        assert_eq!(
            filter.dbcc_size as usize,
            std::mem::size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>(),
            "dbcc_size 要等于结构体实际大小（探针 ABI 头实测 32）"
        );
        assert_eq!(filter.dbcc_size, 32, "x64 下 DEV_BROADCAST_DEVICEINTERFACE_W 是 32 字节");
        assert_eq!(filter.dbcc_reserved, 0);
        assert_eq!(filter.dbcc_name, [0], "注册用的 filter 不带接口名");
    }

    #[test]
    fn device_interface_broadcast_is_known_and_never_vetoes() {
        let mut releaser = RecordingReleaser::new();
        let outcome = handle_broadcast(
            &DbtBroadcast::DeviceInterface { kind: DbtEventKind::RemoveComplete },
            &mut releaser,
        );

        // 接口广播只有 `\\?\Volume{…}`，拿不到盘符 ⇒ 不释放、不发事件，但也不许否决。
        assert!(outcome.payloads.is_empty());
        assert!(releaser.dir.is_empty());
        assert!(releaser.index.is_empty());
        assert!(!outcome.veto);
    }
}
