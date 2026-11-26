# 计划：批次 23 —— 位置页显示手机 / U 盘 + 默认资源管理器打不开设备

> 制定日期：2026-11-19
> 适用仓库：`F:\soft\00selfmade\filemanager\sigma-file-manager`（Alpha File Manager）
> 基线：版本 `3.14.1`，分支 main，HEAD `6e1100d3`
> 插入理由：用户要求**在开发阿里云盘 / OneDrive 之前**先修这个严重 bug。

---

## 〇 结论一览

1. **「位置」页看不到手机，是架构级缺口，不是漏了一个盘符。** 位置页条目 100% 来自 `get_system_drives`（`src-tauri/src/dir_reader/drives.rs:19`），其实现基于 `sysinfo::Disks`，Windows 上走卷枚举（`FindFirstVolumeW` → `GetVolumePathNamesForVolumeNameW` → `GetDriveTypeW`）。**MTP 手机不是卷**：没有 volume GUID、没有挂载点 ⇒ 这些 API 根本看不到它。放开任何过滤条件也没用，它压根没进过枚举结果。
2. **「资源管理器双击手机 → AFM 跳到位置页」有两条独立成因**，且**真机日志已经把手机路径抓下来了**：
   - 用户日志 `…\com.sigma-file-manager.app\logs\Alpha File Manager.log`：
     ```
     [reveal] open_in_native_explorer spawned: path=::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\?\usb#vid_22d9&pid_2765&mi_00#6&337f5e7c&1&0000#{6ac27878-a6fa-4155-ba85-f98f491d4f33}\SID-{10001,,1001925963776}
     ```
     `vid_22d9` = MTP 手机。路径以 `::{` 开头但**不是**「此电脑」精确串 ⇒ 被 `delegate_shell_namespace_paths` 委派给原生 `explorer.exe`，**AFM 自己不打开该设备**。
   - `src-tauri/src/lib.rs:146-175` 把所有 `::{`/`shell:` 开头的 argv **从 `filtered_args` 里删除** ⇒ 前端收不到路径；前端唯一的兜底是「没有路径候选 ⇒ 打开位置页」（`src/utils/launch-directories.ts:190-192`）⇒ **打开盘符变成打开位置**。
3. **AFM 内核从来没有 shell namespace 视图**：`src-tauri/src/dir_reader/read.rs` 用 `fs::metadata` / `fs::read_dir`，读不了 `::{...}`；前端路径层只认真实文件系统路径 + `sfm://locations` 等虚拟路径。⇒ 想让 AFM 打开手机，必须**新建一条 Shell COM 读取管线**。
4. **不需要新增 crate**：`Cargo.toml:153-180` 已启用 `windows = 0.58` 的 `Win32_UI_Shell` / `Win32_UI_Shell_Common` / `Win32_System_Com` ⇒ `IShellItem` / `IShellFolder` / `IEnumIDList` / `SHParseDisplayName` / `SHGetKnownFolderPath` 全部可用（需要 `IPortableDevice` 时只需给 `windows` **加 feature**，不是加依赖）。

---

## 一 目标与范围

| 项 | 内容 |
|---|---|
| 目标 | ① 手机 / U 盘插入后出现在「位置」页（含地址栏下拉，同源）；② 资源管理器双击手机 → AFM 打开该设备，而不是落到位置页；③ 打开失败要有明确提示，不静默 |
| 不做 | 上传/写入手机内容；MTP 私有协议解析（走 Shell COM，不自己实现 MTP 协议） |
| 前提 | 本机**没有手机、也没有可移动盘**（`Get-Volume` 全部 DriveType=Fixed）⇒ 手机 / U 盘分支**无法本机实证**，只能由用户插设备实测 |

---

## 二 根因与改动点

### 2.1 Rust 后端（`src-tauri/`）

| 编号 | 改动 | 位置 |
|---|---|---|
| R1 | 新增 `shell_namespace.rs`：shell 设备枚举 + shell 目录读取 + 单条 stat | `src-tauri/src/dir_reader/shell_namespace.rs`（新） |
| R2 | 新增类型 `ShellDeviceInfo { name, shell_path, is_folder }` | `src-tauri/src/dir_reader/types.rs` |
| R3 | 新增命令 `get_shell_devices`（`spawn_blocking`，与 `get_system_drives` 同款） | `src-tauri/src/dir_reader/commands.rs` + `lib.rs` 注册 |
| R4 | `get_system_drives` 追加 `GetLogicalDrives` + `GetDriveTypeW` 兜底；放宽「`total_space == 0` 直接丢弃」 | `src-tauri/src/dir_reader/drives.rs:47-50` |
| R5 | `read_dir` / `get_dir_entry` 前置分支：路径以 `::{` / `shell:` 开头 ⇒ 走 shell 管线，其余路径**一字不改** | `src-tauri/src/dir_reader/read.rs` |
| R6 | `filter_shell_namespace_args`：**非 absorbed 的 shell 路径保留在 `filtered_args`**；取消自动委派 `explorer.exe`；absorbed（「此电脑」精确串）语义不变 | `src-tauri/src/lib.rs:146-182` |
| R7 | `system_icons.rs` 不再拒绝 `::{` / `shell:` 路径（改走 `SHCreateItemFromParsingName`） | `src-tauri/src/system_icons.rs:315-330` |

### 2.2 前端（`src/`）

| 编号 | 改动 | 位置 |
|---|---|---|
| F1 | 位置页并发拉 `get_system_drives` + `get_shell_devices`，设备条目 `location_group = 'devices'` | `src/utils/virtual-locations.ts:250-253`、`src/types/dir-entry.ts` |
| F2 | `::{` / `shell:` 路径视为**不透明路径**，不规范化、不切分 | `src/utils/normalize-path.ts` |
| F3 | 启动参数：shell 路径候选**优先**，不再被「无候选 ⇒ 位置页」兜底顶掉；打开失败给用户提示 | `src/utils/launch-directories.ts:168-195`、`src/composables/use-init.ts` |
| F4 | i18n 17 个语言包：设备分组标题 / 失败提示 | `src/localization/messages/**` |

---

## 三 门禁与真机验收

```powershell
$env:PATH = 'C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;' + $env:PATH
Set-Location src-tauri; cargo test --lib      # 必须在 src-tauri/ 下跑
npm run check                                  # ts + lint + vitest + verify-changelog + verify:sandbox
npm run tauri:build                            # 真机验收唯一合法产物
```

**真机判据（本机可验，不需要手机）**

| # | 操作 | 期望 |
|---|---|---|
| J1 | 启动 AFM → 位置页 | 盘符列表正常；设备分组不崩（本机无设备，允许为空） |
| J2 | `AlphaFileManager.exe "::{21EC2020-3AEA-1069-A2DD-08002B30309D}"`（控制面板） | AFM **打开该 shell 目录的条目列表**，不是位置页 ⇒ 证明 shell 读取管线生效 |
| J3 | 用真机日志里的**手机路径原串**启动 | 明确错误提示（设备不存在），**不静默**停在位置页 |
| J4 | `AlphaFileManager.exe "C:\Users\Duanyi"` | 正常打开该目录（回归） |

**未判定（本机无法验）**：手机（MTP）真实插入后的「位置页出现设备」与「AFM 内浏览手机内容」⇒ 需用户插设备实测。

---

## 四 风险登记

| # | 风险 | 缓解 |
|---|---|---|
| R1 | Shell namespace 路径的**往返解析**（枚举出来的 `shell_path` 能否再被 `SHParseDisplayName` 解析）是整个方案地基 | Rust 侧写**回环测试**：枚举/读取产出的每个 `shell_path` 必须能被重新解析 |
| R2 | 前端路径层把 `::{...}` 当普通路径规范化会碎 | F2 明确「不透明路径」；vitest 覆盖 |
| R3 | 取消自动委派后，原本被 `explorer.exe` 打开的 shell 路径改由 AFM 承担 | 只对「非 absorbed 的 shell 路径」生效；「此电脑」精确串行为不变 |
| R4 | 手机 / U 盘分支无本机证据 | 明确登记「未判定」，给用户可复现的实测步骤 |
| R5 | `Classes\Drive` / `Classes\Directory` **未注册**（`default_file_manager.rs` 只注册 `Folder` 的 open/explore + 桌面 CLSID） ⇒ 真实 U 盘盘符双击可能仍不走 AFM | 本批次**不改注册层**（改注册会动系统行为与回滚快照）；列为遗留项 |

---

## 五 一句话交接

> 批次 23 = 位置页显示手机/U 盘 + 默认资源管理器打不开设备：后端新建 Shell COM 读取管线（`shell_namespace.rs` + `get_shell_devices`）、`get_system_drives` 加盘符兜底、argv 过滤不再丢弃/委派 shell 路径；前端位置页加「设备」分组、shell 路径当不透明路径、启动参数优先用 shell 路径；地基判据是「枚举出的 shell_path 必须能回环解析」，手机/MTP 分支本机无法实证，留给用户插设备实测。
