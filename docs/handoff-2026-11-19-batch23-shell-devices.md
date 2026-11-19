# 批次 23 交接卡：shell 设备（手机/U 盘）在「位置」显示 + 双击设备能在 AFM 打开

- 生成时间：2026-11-19
- 计划文档：`docs/plan-2026-11-19-shell-devices-locations.md`
- 上一批交接：`docs/handoff-2026-11-18-quark-pan-batch22.md`
- 状态：**Rust 侧已落地并全绿；前端侧收尾中；未装机**

---

## 一、背景与根因（已坐实，非推断）

用户报告两个严重 bug：

1. 插入手机或 U 盘后，「位置」页显示不出来。
2. 在系统资源管理器里双击手机盘符，因为 AFM 是默认资源管理器，跳到 AFM 后**打不开手机盘符**，只跳到了「位置」界面。

### 根因 1：位置页只枚举 Win32 卷，MTP 手机根本不在结果里

链路：`src/utils/virtual-locations.ts:250-253`（`readLocationsDirectory`）
→ `src-tauri/src/dir_reader/commands.rs:140-145`（`get_system_drives`）
→ `src-tauri/src/dir_reader/drives.rs:19-50`（基于 `sysinfo::Disks`，Windows 走
`FindFirstVolumeW` → `GetVolumePathNamesForVolumeNameW` → `GetDriveTypeW`）。

**MTP 手机不是 Win32 卷**，`sysinfo::Disks` 根本不会报它 ⇒ 架构级缺口，不是过滤条件写错。
另外 `drives.rs:47-50` 有三道过滤会丢 U 盘：容量为 0 的丢弃、无挂载点的丢弃、只保留 FIXED/REMOVABLE。

### 根因 2：shell 命名空间路径被启动链路丢弃

链路：`src-tauri/src/lib.rs:146-175`（`filter_shell_namespace_args`）把 `::{` / `shell:` 参数
**从 `filtered_args` 里删除** → 非 absorbed 的走 `delegate_shell_namespace_paths` → `explorer.exe`。
前端 `src/utils/launch-directories.ts:190-192` 在没有候选时硬造 `LOCATIONS_VIRTUAL_PATH`，
于是用户看到的就是「跳到位置界面」——AFM 已经把 shell 路径丢了，再去要目录当然要空。

真机日志（用户机器 `C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.app\logs\Alpha File Manager.log`）：

```
[reveal] open_in_native_explorer spawned:
path=::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\?\usb#vid_22d9&pid_2765&mi_00#6&337f5e7c&1&0000#{6ac27878-a6fa-4155-ba85-f98f491d4f33}\SID-{10001,,1001925963776}
```

`vid_22d9` = MTP 手机。

---

## 二、Rust 侧（T1–T4，已完成，门禁全绿）

`cd src-tauri; cargo test --lib -- --nocapture`
（cargo 需先加 `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin` 到 PATH）

**实测结果：`1433 passed; 0 failed; 1 ignored`。**

### 改动清单（10 个文件，全在 `src-tauri/`）

| 文件 | 改动 |
|---|---|
| `Cargo.toml` | windows crate 加 2 个 feature：`Win32_UI_Shell_PropertiesSystem`、`Win32_Devices_PortableDevices`（**不新增 crate**） |
| `src/dir_reader/types.rs` | 新增公开类型 `ShellDeviceInfo { name, shell_path, is_folder }`，`#[serde(rename_all = "camelCase")]` |
| `src/dir_reader/shell_namespace.rs` | **新建**（约 700 行）：`is_shell_namespace_path` / `get_shell_devices` / `read_shell_dir` / `stat_shell_entry`；COM 公寓守卫、`SHParseDisplayName` 解析 + 两条退路、`IShellItem` 枚举子项、`IPortableDevice` QI 判据、4 项单测 |
| `src/dir_reader.rs` | 注册 `mod shell_namespace;`；导出 `ShellDeviceInfo` 与 `pub(crate) is_shell_namespace_path` |
| `src/dir_reader/commands.rs` | 新增 `#[tauri::command] get_shell_devices`（走 `spawn_blocking`） |
| `src/dir_reader/read.rs` | `read_dir` / `get_dir_entry` 最前面加 shell 分支；timeout 变体经同两函数自动覆盖 |
| `src/dir_reader/drives.rs` | 容量为 0 的**可移动盘不再丢弃**（固定盘口径不变）；新增 `sysinfo_paths` 集合区分「压根没报过」与「报了但被过滤」 |
| `src/dir_reader/drives_platform.rs` | 新增 `append_windows_logical_drives`：`GetLogicalDrives` + `GetDriveTypeW` 兜底（FIXED/REMOVABLE，容量读不到留 0），只补 sysinfo 未报过的盘 |
| `src/system_icons.rs` | shell 路径改走 shell 图标工厂（`SHCreateItemFromParsingName` → `IShellItemImageFactory::GetImage` → `GetDIBits` → PNG）；`is_shell_icon_candidate_path` **不再拒绝** shell 路径 |
| `src/lib.rs` | T4：删除 `delegate_shell_namespace_paths`、取消自动委派 `explorer.exe`（`open_in_native_explorer` 本体保留 + `#[allow(dead_code)]`）；非 absorbed 的 shell 路径保留进 `filtered_args` 并打 `log::info!("[launch] shell namespace path kept: path={path}")`；`handle_second_instance` 可打开路径判定改为 `filtered_args.len() > 1 \|\| !kept_shell_paths.is_empty()`；注册 `dir_reader::get_shell_devices`；末尾测试模块重写为 4 项 |

### 新增单测（Rust）

```
test dir_reader::shell_namespace::tests::system_drives_have_no_duplicates ... ok
test dir_reader::shell_namespace::tests::shell_paths_round_trip_through_shell_parse_display_name ... ok
test dir_reader::shell_namespace::tests::computer_folder_lists_children_and_real_drives_are_not_devices ... ok
test dir_reader::shell_namespace::tests::shell_directories_can_be_read_and_their_entries_round_trip ... ok
test tests::absorbed_computer_folder_path_is_not_kept ... ok
test tests::kept_shell_namespace_path_is_not_delegated ... ok
test tests::kept_shell_namespace_path_stays_in_launch_args ... ok
test tests::ordinary_filesystem_paths_are_untouched ... ok
```

---

## 三、⭐ 重大实证：本机确实连着手机，已被枚举到并能读目录

**此前判断「本机无手机」是错的**——`Get-Volume` / `sysinfo::Disks` 看不到 MTP 设备，
只有 Shell COM 枚举「此电脑」才看得到。Rust 侧 `--nocapture` 真实输出：

```
[shell-namespace] 设备候选: name=fnNAS path=::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\uuid:4d696e69-444c-164e-9d41-7085c27f4afe 目录=可读，4 个条目
[shell-namespace] 设备候选: name=一加 Ace 3 path=::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\\?\usb#vid_22d9&pid_2765&mi_00#6&337f5e7c&1&0000#{6ac27878-a6fa-4155-ba85-f98f491d4f33} 目录=可读，1 个条目
```

⇒ 手机「一加 Ace 3」的 MTP 目录**真的能读出来**，且解析名与用户日志里的启动参数前缀完全一致。
这是本批次最硬的一条证据：手机分支**已实证**，不是推断。

---

## 四、前端侧（F1–F4，状态：收尾中）

新增文件：
- `src/types/shell-device.ts`（16 行）
- `src/utils/shell-paths.ts`（65 行）
- `src/utils/shell-devices.ts`（59 行）
- `src/utils/open-in-system-explorer.ts`（22 行）
- `src/composables/use-shell-devices.ts`（41 行）

修改文件：
- `src/utils/virtual-locations.ts`（位置页并发拉 drives+devices、`location_group='devices'`）
- `src/utils/normalize-path.ts`（`::{` / `shell:` 原样透传，不规范化、不按 `/` 切分）
- `src/utils/launch-directories.ts`（启动参数优先 shell 路径、打开失败给可见提示）
- `src/composables/use-init.ts`
- `src/types/dir-entry.ts`
- `src/modules/navigator/components/file-browser/address-bar-parts.ts` / `address-bar.vue`
- `file-browser-entry-groups.ts` / `file-browser-grid-view.vue` / `file-browser-list-view.vue` / `file-browser-virtual-rows.ts`
- 17 个 i18n 语言包 `src/localization/messages/*.json`

新增测试文件：
- `src/utils/__tests__/shell-devices.test.ts`
- `src/utils/__tests__/virtual-locations.test.ts`（补充）
- `src/utils/__tests__/normalize-path.test.ts`（补充）
- `src/utils/__tests__/launch-directories.test.ts`（补充）
- `src/composables/__tests__/use-init.test.ts`（补充）
- `src/modules/navigator/components/file-browser/__tests__/address-bar-parts.test.ts`（补充）
- `src/localization/__tests__/shell-device-i18n-keys.test.ts`

---

## 五、前后端接口契约（Rust 已定，前端按此实现）

- 命令 `get_shell_devices` → `ShellDeviceInfo[]`，JSON 字段 `{ "name": string, "shellPath": string, "isFolder": boolean }`
- `get_system_drives` 返回结构不变（`DriveInfo[]`）
- shell 路径原样保留 Windows 的 `SIGDN_DESKTOPABSOLUTEPARSING` 字符串（以 `::{` 或 `shell:` 开头），
  前端必须当**不透明路径**处理（不规范化、不按 `/` 切分）
- `read_dir` / `get_dir_entry` 对 shell 路径透明可用

---

## 六、已知限制与风险（务必如实带给用户）

1. **手机 MTP 已实证**（不再存疑）：本机「一加 Ace 3」被枚举到，目录可读、1 个条目。
2. **U 盘 / 可移动盘兜底未实证**：本机没有可移动盘。兜底代码在
   `src-tauri/src/dir_reader/drives_platform.rs::append_windows_logical_drives`，需插 U 盘复测。
3. **「设备」判据会误报**：兜底判据是「无文件系统路径 + 文件夹」，实测把 NAS（`fnNAS`）也列成了设备。
   判据代码在 `shell_namespace.rs::collect_shell_devices`（`is_portable_device_item` 的 QI 判据 + 兜底判据）。
   前端可按 `name` 过滤，或后续收紧判据。**这是已知待修项。**
4. **`SHParseDisplayName` 不是万能的**：实测 fnNAS 的 `uuid:` 节点解析直接报 `0x80070057`。
   已加两条退路（逐级缩短解析名再匹配子项 / 从「此电脑」按前缀逐级下钻），
   代码在 `shell_namespace.rs::resolve_shell_item`。手机上带 `\SID-{...}` 的两级路径走的就是第二条退路。
5. `LaunchContext.had_delegated_shell_paths` 现在**恒为 false**（字段按约束未删，语义变为「不再委派」）；
   `ShellFilterResult.delegated_paths` 保留但恒空。
6. `open_in_native_explorer` 已无调用点（保留函数体 + `#[allow(dead_code)]`）。
7. shell 图标（`GetDIBits` → PNG）只验证了编译与分流逻辑，**未在真机 UI 上目视确认**图标效果。
8. **注册层本批次不动**：`Classes\Drive` / `Classes\Directory` 未注册，
   所以真实 U 盘盘符双击是否进 AFM 仍未验证；`src-tauri/src/default_file_manager.rs:372-378`
   只注册了 `Folder\shell\open\command`、`Folder\shell\explore\command`、桌面 CLSID `opennewwindow`。
   改注册会动系统行为并牵动 `ROOT_REGISTRY_KEYS` 定长数组与快照/回滚逻辑 ⇒ 列为遗留项。

---

## 七、剩余待办

- [ ] 前端 worker 收尾（改到 `npm run check` 全绿）
- [ ] 清理 worker 临时日志（仓库根 `.check-output.log`、`src-tauri/*.log`）
- [ ] 提交批次 23 代码
- [ ] 版本号 bump + `CHANGELOG.md` + `public/changelog/` 14 个语言包 + README 功能行
- [ ] `npm run check` + `cargo test --lib` 复跑
- [ ] `npm run tauri:build` 出装机 exe，**真机验收**（本批次未装机）
- [ ] 更新台账 `docs/handoff-2026-11-11-quark-pan.md` §20.19
- [ ] 收紧「设备」判据（把 NAS 之类误报挡掉）

## 八、一句话交接

读 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-19-batch23-shell-devices.md`，
接手批次 23 的前端收尾、版本/CHANGELOG、门禁复跑、`tauri build` 装机真机验收与台账 §20.19。
