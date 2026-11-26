# 批次 23 交接卡：shell 设备（手机/U 盘）在「位置」显示 + 双击设备能在 AFM 打开

- 生成时间：2026-11-19
- 计划文档：`docs/plan-2026-11-19-shell-devices-locations.md`
- 上一批交接：`docs/handoff-2026-11-18-quark-pan-batch22.md`
- 状态：**代码全绿、3.15.0 已打包并静默装机成功、日志确认运行 `version=3.15.0`；真机验收 J1–J3 取证中**
- 取证产物目录：`.superpowers/sdd/b23-live-*`（截图 / UIA dump / 原始路径字节 / cargo 日志）

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
   **这是已知待修项**，收紧方案已定（见 §十一）。

   ⭐ **权威字节形态**（当场从 cargo 原始日志回读，非转述；见 `.superpowers/sdd/b23-live-shellpath-raw.txt`）：

   | 设备 | 长度 | 反斜杠总数 | 连续段 | 原文 |
   |---|---|---|---|---|
   | fnNAS | 82 | 1 | `[1]` | <code>::{20D04FE0-…}\uuid:4d696e69-…</code> |
   | 一加 Ace 3 | 129 | 4 | `[3,1]` | <code>::{20D04FE0-…}\\\?\usb#vid_22d9&pid_2765&mi_00#6&337f5e7c&1&0000#{6ac27878-…}</code> |

   ⇒ 手机是 **`\\\?\usb#`**（分隔符 `\` + 设备接口前缀 `\?\`），fnNAS 是 **`\uuid:`**。
   因此「解析名必须含 `\?\`」即可挡掉 fnNAS、保留手机/读卡器（后者恒为 `\?\usb#` / `\?\storage#`）。
   另注：枚举出的路径**不含** `\SID-{…}` 后缀；用户 argv 里那个后缀是下一级子项，属 J3 的判定对象。
4. **`SHParseDisplayName` 不是万能的**：实测 fnNAS 的 `uuid:` 节点解析直接报 `0x80070057`。
   已加两条退路（逐级缩短解析名再匹配子项 / 从「此电脑」按前缀逐级下钻），
   代码在 `shell_namespace.rs::resolve_shell_item`。手机上带 `\SID-{...}` 的两级路径走的就是第二条退路。
5. `LaunchContext.had_delegated_shell_paths` 现在**恒为 false**（字段按约束未删，语义变为「不再委派」）；
   `ShellFilterResult.delegated_paths` 保留但恒空。
6. `open_in_native_explorer` 已无调用点（保留函数体 + `#[allow(dead_code)]`）。
7. shell 图标（`GetDIBits` → PNG）只验证了编译与分流逻辑，**未在真机 UI 上目视确认**图标效果。
8. **注册层本批次不动——且实测证明本批次不需要动**（原判据担心「盘符双击可能进不了 AFM」，已排除）：
   - `HKCR\Folder\shell\open\command` 与 `Folder\shell\explore\command`（由
     `HKCU\SOFTWARE\Classes\Folder\…` 写入）= `"…\sigma-file-manager-launcher.exe" "%1"` ⇒ AFM 的
     「默认资源管理器」在本机**处于生效状态**，真实双击入口是 **launcher 而非主 exe**。
   - `HKLM\SOFTWARE\Classes\Folder\shell\open\command` 仍是 `C:\WINDOWS\Explorer.exe`，被 HKCU 覆盖。
   - `(HKCR\Drive\shell).GetSubKeyNames()` 只有 `change-passphrase/change-pin/cmd/Cursor/find/manage-bde/
     pintohome/Powershell/resume-bde*/unlock-bde/WSL` ⇒ **没有 open/explore** ⇒ 带盘符的设备（U 盘、
     手机若分配了盘符）双击**兜底到 `Folder\shell\open\command`**，argv 是 `E:\` 这类普通文件系统路径，
     既有 drives 分支就能接住 ⇒ **不需要新增 `Classes\Drive` 注册**。
   - `HKCU\SOFTWARE\Classes\CLSID\{20D04FE0-…}` 与 `HKCU\Software\Classes\Directory\Background\shell` 均 MISS。
   - launcher 侧参数链路已静态审完（`default-file-manager-launcher/src/launch_args.rs:114-118` 对非
     `/select` 参数**原样透传**，`main.rs:406-410` 用完整 `passthrough` 启动主程序）⇒ launcher **不会**
     过滤 `::{…}` 参数。缺一条 shell 路径 argv 的单测，列入 §十一。

---

## 七、版本面与提交面

| 项 | 值 |
| --- | --- |
| 版本 | `3.14.1` → **`3.15.0`** |
| 同步 | `package.json` = `src-tauri/Cargo.toml` = `default-file-manager-common/Cargo.toml` = `default-file-manager-launcher/Cargo.toml`（`npm run sync-version` 已跑）；`tauri.conf.json` 的 version 指向 `../package.json` |
| `CHANGELOG.md` | 新增 `## [3.15.0] - November 2026` 条目（4 条：修复 / 修复 / 新增 / 修复） |
| `public/changelog/` | 14 个 locale 包（ch/de/es/fa/fr/hi/it/ja/pt/ru/sl/tr/ur/vi）全部补齐 3.15.0 条目 |
| `README.md` / `README.zh-CN.md` | 功能表新增「手机与 U 盘 / Phones and USB drives」一行；新增「3.15.0 改了什么 / What changed in 3.15.0」小节 |

提交：

| commit | 内容 |
| --- | --- |
| `3b0ca277` | docs(plan)：批次 23 落地方案 |
| `e9a51abc` | feat(devices)：双端实现 + 新增单测 |
| `f92c455e` | release(3.15.0)：版本 bump + CHANGELOG + README + 台账 §20.19 |

台账：`docs/handoff-2026-11-11-quark-pan.md` **§20.19**（根因、后端 10 文件表、前端 F1–F4、门禁输出、重大实证、已知限制、版本面、下一阶段起点）。

## 八、剩余待办

- [x] 前端 worker 收尾（`npm run check` 全绿）
- [x] 清理 worker 临时日志（仓库根 `.check-output.log`、`src-tauri/*.log`）
- [x] 提交批次 23 代码
- [x] 版本号 bump + `CHANGELOG.md` + `public/changelog/` 14 个语言包 + README 功能行
- [x] `npm run check`（退出码 0）+ `cargo test --lib`（1433 passed / 0 failed）复跑
- [x] 更新台账 `docs/handoff-2026-11-11-quark-pan.md` §20.19
- [ ] **`npm run tauri:build` 装机 exe + 真机验收**（构建已启动，结果见 §九）
- [ ] **收紧「设备」判据**（把 `fnNAS` 之类误报挡掉）
- [ ] 回到主线：阿里云盘 + OneDrive（动手前先修 `src/utils/remote-directory.ts:137-147` 硬编码「夸克网盘」的串台文案）

## 九、真机验收（待补）

构建脚本：`.superpowers/sdd/b23-build.cmd`（纯 ASCII，内部 `> log 2>&1`，末尾只复制到 `release/`，不上传）。
日志：`.superpowers/sdd/b23-build.log`。

装机后的三条判据（本机有「一加 Ace 3」，**手机分支可在本机实证**）：

| # | 判据 | 做法 | 期望 |
| --- | --- | --- | --- |
| J1 | 位置页出现设备 | 启动装好的 exe，参数 `sfm://locations`，截帧 | 出现「设备」分区，含「一加 Ace 3」 |
| J2 | 双击手机能打开 | 启动 exe，参数为 MTP 的 shell 路径（`::{20D04FE0-…}\\?\usb#vid_22d9…`） | 窗口落在手机目录内，**不是**位置页 |
| J3 | 用户日志里的完整路径 | 启动 exe，参数含 `\SID-{10001,,1001925963776}` 后缀 | 落在手机里对应子目录 |

U 盘（本机无可移动盘）：**未判定**，需用户插 U 盘复测。

## 十一、取证结束后要做的两件事（判据已定，别在取证期间动源码）

1. ~~**收紧「设备」判据**（挡掉 fnNAS 误报）~~ **【已改判：保留 fnNAS，不挡】**
   - 改判依据（2026-11-19 实证）：fnNAS 走 `\uuid:` 形态，是资源管理器「此电脑」里同样出现的 WPDev 设备，
     且实测可读（`read_dir` 返回 4 个条目）。挡掉它等于削功能，与「和真实资源管理器一致」的设计目标相反。
   - 若将来要挡，必须有用户明确反馈，且判别性测试要两向自证（手机留下、目标设备掉出去、不引进固定盘/网络盘）。
   - **仍然要改的是注释**：`is_portable_device_item` 上方写着「这台机器上没有手机」，已被实证推翻（本机有一加 Ace 3）。
   - 位置：`src-tauri/src/dir_reader/shell_namespace.rs::collect_shell_devices`（`:431-474`）兜底分支
     与 `is_portable_device_item`（`:307-318`）。
2. **补 launcher 的 shell 路径 argv 单测**
   - 位置：`src-tauri/default-file-manager-launcher/src/launch_args.rs`（现有 6 个测试，含
     `keeps_plain_path_arguments_untouched`，但没有 `::{…}` 形态的用例）。
   - 期望：`::{20D04FE0-…}\\\?\usb#…` 作为普通参数**原样进 `passthrough`**，不被当成 `/select` 改写。

## 十二、真机取证姿势（本轮踩过坑，照抄别再试）

读运行中程序的日志（文件被独占，`ReadAllLines` / `Get-Content -Tail` 全失败，只有这个姿势能用）：

```powershell
$log="$env:LOCALAPPDATA\com.sigma-file-manager.app\logs\Alpha File Manager.log"
$fs=[System.IO.File]::Open($log,[System.IO.FileMode]::Open,[System.IO.FileAccess]::Read,[System.IO.FileShare]::ReadWrite)
$sr=New-Object System.IO.StreamReader($fs,[System.Text.Encoding]::UTF8); $txt=$sr.ReadToEnd(); $sr.Close(); $fs.Close()
$lines=$txt -split "`r?`n"   # 时间戳是 UTC，本地 = UTC+8
```

- release 日志只有 INFO/WARN ⇒ **缺某条 `log::debug!` 行不能当 FAIL**。
- 判据要先自证可见：先在网络/路径正常状态下让同一判据命中一次，再上故障态。
- `[launch]` 相关日志行当前为 **0 条**（还没带 shell argv 启动过 3.15.0）。
- PowerShell `*>` 重定向 + `Set-Location` 会被安全策略拦 ⇒ cargo/构建输出一律走纯 ASCII `.cmd` 内部
  `cd /d` + `> "log" 2>&1`。

## 十三、批次 23 装机真机验收结论（worker `bg_706079d4`，3.15.0 装机版）

证据目录 `.superpowers\sdd\`，主报告 `b23-live-j2-report.txt`；UIA dump `b23-live-uia-*.txt`；截帧 `b23-live-shot-*.png`。

| 判据 | 结论 | 关键证据 |
| --- | --- | --- |
| J1 位置页出现「设备」分组（手机 + fnNAS） | **PASS** | `b23-live-shot-02-locations.png` / `b23-live-uia-20-locations.txt` |
| J2 launcher 带 shell argv 启动 → AFM 新标签直接打开手机存储 | **PASS** | 日志新增 `[launch] shell namespace path kept`；地址栏末段=「一加 Ace 3」，内容列出「内部共享存储空间」（`b23-live-j2-report.txt:5-28`、`b23-live-uia-40-L-base.txt`、`b23-live-shot-04-L-base.png`） |
| J3 带 `\SID-{…}` 后缀的 argv 原样保留并读出目录 | **PASS** | 读出 73 文件夹 / 91 项目（`b23-live-j2-report.txt:50-72`、`b23-live-uia-42-L-sid.txt`、`b23-live-shot-06-L-sid.png`） |
| 附加：位置页点手机卡片直接进入手机存储（不再退回位置页） | **PASS** | UIA 中心点 939,491 |
| 对照组：主 exe（非 launcher）直接传参 | **UNDETERMINED** | 同样记 kept，但新标签打开的是 `H:\verysync-windows-amd64-v2.21.3`；同期「小兵U盘 (H:)」刚插入，无法归因 |
| 疑点：U 盘双击会进 AFM | **不是缺陷** | `HKCR\Drive\shell` 无 open/explore 子键 ⇒ 继承 `HKCR\Folder\shell\open\command`（= launcher `"%1"`），属预期的默认资源管理器行为 |
| 疑点：桌面 CLSID `{20D04FE0-…}` 未注册 | **不计缺陷** | `HKCU\…\Explorer\CLSID\{20D04FE0-…}` = MISS，按父层口径处理；未改注册表 |

**D1（真机暴露的真实缺陷，批次 24 处理）**：设备名在 SID 层丢失 ⇒ 面包屑/标签退化成通用「设备」。
- 对比证据：`b23-live-uia-40-L-base.txt` 末段是 `一加 Ace 3 | address-bar__part--last`，
  而 `b23-live-uia-42-L-sid.txt` / `b23-live-uia-30-phone-enter.txt` 末段是 `设备 | address-bar__part--last`。
- 根因：`src/modules/navigator/components/file-browser/address-bar-parts.ts:89-90` 用
  `context.deviceNames[path]` 做**整串精确匹配**，查不到即回落 `fallbackLabel`（`:64`，i18n `devices`=「设备」）。
  而位置页点卡片**正是落到 SID 路径**，所以主路径用户看到的标签就是「设备」。

## 十四、批次 24（两条用户新报缺陷 + 三条并行修复）

用户 14:19 报：① 目录树显示不出手机；② 进入手机盘后地址栏只能显示到「设备」这一级、不能逐级向下。

| 线 | 根因（已直读源码坐实） | 派单 | 汇报 |
| --- | --- | --- | --- |
| 目录树 | `src/modules/navigator/utils/remote-tree-roots.ts` 的 `resolveTreeRoots` 在非远程焦点时 `return { rootPaths: [...input.drivePaths] }` ⇒ 树根只有盘符，shell 设备从不进树 | worker `bg_b9ed59e5` | `.superpowers/sdd/b24-tree-devices-report.md` |
| 地址栏逐级 + D1 | `address-bar-parts.ts:84-109 buildShellAddressBarParts` 把整条 shell 路径当一级，只产出「位置 > 设备名」 | Rust worker `bg_b50822c6` 新增 `resolve_shell_path_chain`；前端接线由根会话做 | `.superpowers/sdd/b24-shell-chain-report.md` |
| launcher 回归护栏 | 原 6 条测试无 `::{…}` 形态用例 | worker `bg_e23b033f`（只加测试，未改产品代码） | `b24-launcher-argv-test-report.md`：**8 条新用例 / `cargo test -p sigma-file-manager-launcher` 14 passed 0 failed**，未发现真实缺陷 |

**为什么祖先链必须由后端给（设计约束，不是 bug）**：① 每级的解析路径与显示名**都要问 Shell**（`\SID-{10001,,1001925963776}` 的显示名是「内部共享存储空间」，与字面完全不同）；② 双击直接启动时前端只拿到末级一条路径，祖先完全未知；③ 设备根 `::{GUID}` 内部本身就含 `\` ⇒ **禁止按反斜杠 split**。

launcher worker 顺带登记的两条观察（未改，留给上层）：
1. 启动器对非 select 参数**不剥引号**；真机不受影响（注册串 `"<launcher>" "%1"`，外层引号被 Windows 解析吃掉），仅「双重引号」才可能传到主程序，届时 `lib.rs:103-106` 的 `starts_with("::{")` 判不出 shell 路径 —— **潜在健壮性缺口，无真机证据**。
2. `src-tauri/src/lib.rs:810` 测试常量 `PHONE_SHELL_PATH` 用的是**单反斜杠** `9D}\?\usb#…`，与真机取证原文 `9D}\\\?\usb#…` 不一致（少一个 `\`）。结论不受影响（判据只看 `::{` 前缀），但作为取证样例不准 ⇒ 待改成 §六.3 的权威字节形态。

## 十五、U 盘问题的实证结论（回答用户 14:19 第 1 问）

**原来的 SFM 本来就支持带盘符 U 盘显示**，批次 23 不是「新增 U 盘支持」，而是两处加固：
- 旧 `src-tauri/src/dir_reader/drives.rs`（`e9a51abc^`）已用 `sysinfo::Disks::new_with_refreshed_list()` 枚举**所有卷**，U 盘本就在内；
  Windows 分支唯一会丢可移动盘的判据是 `#[cfg(windows)] if total_space == 0 { continue; }`。
- 加固一：判据收紧为 `if total_space == 0 && !disk.is_removable() { continue; }` ⇒ **容量读不到的可移动盘不再丢**（读卡器、刚插未就绪的盘常见）。
- 加固二：`drives_platform.rs` 新增 163 行 `append_windows_logical_drives`（`GetLogicalDrives` + `GetDriveTypeW`），
  **只补 sysinfo 压根没报过的盘**（固定盘/网络盘口径完全不变）；新增 `sysinfo_paths: HashSet<String>` 区分「没报」与「报了被过滤」。
- diffstat：`2 files changed, 184 insertions(+), 2 deletions(-)`。
- 不带盘符的 **MTP 手机**才是完全新增的能力（shell namespace 链路）。用户已回报「U盘测试显示正常」⇒ 位置页 U 盘分支实测通过。

## 十六、批次 24 两条 worker 收口实况（14:48 实测回读，含一条**未闭合门禁红**）

### 16.1 Rust 线 `bg_b50822c6` —— 三门禁全绿

证据 `.superpowers\sdd\b24-shell-chain-report.md` + `b24-gate2.done.txt`：
`CLIPPY_EXIT=0 / CHAIN_EXIT=0 / TEST_EXIT=0 / ALL_DONE`，`b24-testall.log` 末行
**`1436 passed; 0 failed; 1 ignored`**（基线 1433 ⇒ 新增 3 例，零回归）。

对外契约（面包屑接线以此为准，worker 声明签名不再动）：
- 命令 `resolve_shell_path_chain(path: String) -> Vec<ShellPathCrumb>`，**JSON 键 = `path` / `name`**；
- 顺序 `[0]` = 设备根 … 末项逐字等于入参；**不含**「此电脑」/桌面根；入参即「此电脑」⇒ `Ok([])`；
- `name` 取 `SIGDN_NORMALDISPLAY`，取不到退回该级解析路径 ⇒ **绝不空**；
- 非 shell 路径 ⇒ `Err("不是 shell namespace 路径: <原样>")`；解析失败 ⇒ `Err("无法定位 shell 路径: <原样>")`；
- 防失控：重复层级（环）/ 深度 32 / `GetParent()` 报错 ⇒ 就地返回已收集链，**不报错**。

真机判别性证据（一加 Ace 3，`b24-cargo-gate.log` SECTION 1）：三层链 `一加 Ace 3 → 内部共享存储空间 → .7934039a`，
**SID 层显示名正确、不再退化「设备」⇒ D1 由这条链一并解决**；`assert_eq!` 校末项逐字等于入参。

两条如实登记的实测坑（重要）：
1. **`is_portable_device_item` 在本机对一加 Ace 3 实测 `portable=false`** —— 手机是靠判据三
   （无 `SIGDN_FILESYSPATH` + 文件夹形态）进的设备列表，**不是** WPD 接口。以后别假设「手机一定命中 WPD」。
2. **设备枚举结果不稳定**：同机器两次测试，一次 2 台（fnNAS + 手机）一次 1 台（fnNAS 缺席）
   ⇒ fnNAS 的祖先链**仍未取到真机样本（未判定）**，别把「覆盖了手机」当全覆盖。
3. `src-tauri/src/lib.rs:810` 常量少一个反斜杠：**本批未修**（改动清单里没有 lib.rs 常量行），仍待办。

### 16.2 前端目录树线 `bg_b9ed59e5` —— 单测绿但 **`npm run check` 的 ts 段红 3 条**

`b24-tree-devices-report.md` §二~§三：改动 4 个源文件（`remote-tree-roots.ts` 新增可选 `deviceRoots`、
`folder-tree.ts` 的 `parentDirectoryPath`/`computeAncestorPaths`/`markTreeStale` 加 `isShellPath` 护栏、
`use-file-tree.ts` 五处、`navigator.vue` 喂 `shellDevices`）+ 3 个测试文件新增 22 例（14 例先红后绿）。
三文件复跑 `b24-green.txt` = **108 passed (108)**。

**但完整门禁 `b24-check.txt` 是红的（`ERROR: "ts" exited with 2`）**，vue-tsc 三条：
- `src/modules/navigator/composables/__tests__/use-file-tree.test.ts(71,15)` 与 `(95,15)` TS2322：
  `readDir` 的 mock 返回对象只给了 `path/name/is_dir/size/modifiedAt`，**缺 `DirEntry` 的 9 个必填字段**
  （`ext, item_count, modified_time, accessed_time, …`）⇒ 修法 = mock 返回类型改成 `DirEntry[]` 并补全字段
  （或用仓库既有的 DirEntry 测试工厂），**不是**放宽类型断言。
- `src/modules/navigator/utils/__tests__/remote-tree-roots.test.ts(320,12)` TS7053：
  用 `PHONE_ROOT` 常量索引字面量类型 `{ 'C:/': string; 'D/': string }` ⇒ 给该夹具标 `Record<string, string>`。
- worker 汇报里 §四「本轮：待回填」就是这条红 ⇒ **谁接手谁修，`npm run check` 退出码非 0 不得提交**。

### 16.3 该线自己标出的最高风险（未判定，装机后第一优先验）

**shell 子节点分隔符口径**：设备根 `shellPath` 是 `\` 形态，而 `folder-tree.ts` 注释声称 Rust 的
`DirEntry.path` 已被 `src-tauri/src/utils.rs::normalize_path` 归一成 `/`。若展开手机后子节点 key 用 `/`、
根 key 用 `\` ⇒ 前缀截断结果与根 key 不相等 → `ensureLoaded` 走 `addRoot`，
**把设备子目录立成并列第二根**（v6.3 2026-10-19 修过的同形缺陷）。
- 判据：装机展开手机，看 `read_dir` 回来的子节点 `path` 里是 `\` 还是 `/`。
- 若为 `/`：正确修法是让 Rust 侧对 shell 路径**跳过 normalize**，**不要**在前端补两种口径。

### 16.4 面包屑前端接线（尚未开工，输入已齐）

`address-bar-parts.ts:60-70` 的 `ShellBreadcrumbContext` 增可选 `chain?: readonly {path,name}[]`；
`buildShellAddressBarParts:84-109` 改为「位置 > 链各级」，**链缺失/未命中末项时回落现有两段式**（纯函数不同步拿不到后端）。
`address-bar.vue:114-134` 侧：`watch(() => props.currentPath)` 里对 `isShellPath` 的 path 异步 `invoke('resolve_shell_path_chain')`
+ Map 缓存 + 仅当 path 仍是当前才写回（避免竞态）；invoke 风格照 `src/utils/shell-devices.ts:61`。
测试补 `__tests__/address-bar-parts.test.ts`（现有 shell 用例在 `:203/:226`，改两段式断言会连带受影响）。

## 十、一句话交接

读 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-19-batch23-shell-devices.md`，
批次 23 已完成并装机 3.15.0（J1/J2/J3 全 PASS，见 §十三）；接手批次 24：
§十四 的三线修复（目录树显示设备 / 地址栏逐级下钻 + D1 设备名退化 / launcher 护栏测试），
三条汇报文件收口后跑门禁 + 提交 + 回填台账 §20.19，再回阿里云盘 + OneDrive 主线。

