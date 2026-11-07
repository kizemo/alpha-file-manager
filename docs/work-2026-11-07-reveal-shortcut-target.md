# 2026-11-07 「打开文件所在目录」对快捷方式失效 + 界面卡死 / 渲染进程崩溃 —— 排查记录

- 记录人：MCode 会话 `mvs_56ce963d7af14735a27fa98d60529f3b`
- 基线版本：3.12.1（内层仓 `F:\soft\00selfmade\filemanager\sigma-file-manager`）
- 相关交接卡：`docs/handoff-2026-11-07-shortcut-icon-and-native-separator.md`

---

## 1. 用户报告的现象

在导航器里右键一个**快捷方式**（`.lnk`）→ 菜单里选「打开文件所在目录」：

1. **点了没反应** —— 没有跳到快捷方式指向的目标所在目录；
2. **右键菜单的子菜单加载不出来**，界面像是卡死；
3. 随后弹出「**Alpha File Manager 的界面渲染进程已崩溃（窗口 main）**」。

用户另外贴了三张报错弹窗截图：
「已中止操作」、「渲染进程已崩溃」、「Windows 找不到 '\\' 文件」。

---

## 2. 证据

### 2.1 应用日志（全量扫描）

日志文件：`C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.app\logs\Alpha File Manager.log`（时间戳是 **UTC**）。

扫描脚本与产物落外层仓 `.superpowers/sdd/log-reveal-scan.txt`（12,292 行全量扫描，结果文件 23 KB）：

| 检索项 | 命中 |
|---|---|
| `[reveal]` / `reveal_in_explorer` / `resolve_windows*` / `path_exists` / `get_system_icon` | 全日志**仅 1 条** `[reveal]` |
| 那唯一 1 条的时间 | **2026-11-05**，路径是虚拟路径 `::{645FF040-…}`，走的是 `open_in_native_explorer` |
| 2026-11-07 事故时段（UTC 08:15–08:29） | **零** `[reveal]` 行 |

⇒ **事故走的是「应用内定位」分支**（AFM 是默认资源管理器），原生 Explorer 分支根本没触发。
所以「已中止操作」「Windows 找不到 '\\' 文件」两个弹窗**不可能是本应用主动 shell out 造成的**
（那条路有 `[reveal]` 日志，且一条都没有）。

### 2.2 看门狗日志

同一份日志里的看门狗记录（UTC 08:21:51 = 本地 16:21:51，与用户截图时间吻合）：

```
[2026-11-07][08:21:51] window-hung detected for window main (no heartbeat for 61592 ms), reloading it
[2026-11-07][…] recovery failed ... prompting the user
[2026-11-07][08:24:12] window-hung detected for window main (no heartbeat for 122014 ms), reloading it
```

⇒ 主线程 / 渲染侧被**连续阻塞 61 秒以上**（第二次 122 秒），看门狗 reload 失败 ⇒ 渲染进程崩溃。

### 2.3 排除项

- JS 侧死循环：`use-file-browser-selection.ts` 的 `requestFocusEntryAfterRefresh` / `pendingFocusRequest`
  只写 ref，无定时重试 ⇒ 排除。
- 用户截图里的 `02文件管理绿联云.lnk` 现已不存在（现存 `绿联云.lnk` 指向 `D:\Program Files\UGREEN NAS\UGREEN NAS.exe`），
  ⇒ 真机验收改用自建夹具（见 §7）。

---

## 3. 根因

### 根因 A：落点解析**只认目录目标**（功能缺陷 → 「点了没反应」）

`read::resolve_windows_directory_shortcut` 要求快捷方式的目标 `is_dir()` 才返回目标路径。
而**绝大多数快捷方式指向的是文件**，于是解析一律返回 `None` ⇒ 前端静默回落成
「打开 `.lnk` 自己所在的目录」⇒ 用户看到的就是「点了没反应，没跳到目标所在目录」。

对应代码位置：`src-tauri/src/dir_reader/read.rs`（`resolve_windows_directory_shortcut`）、
`src/utils/resolve-reveal-entry-target.ts`（2026-11-03 的 L1 版本分两步走：
`resolve_windows_directory_shortcut` → `path_exists`）。

### 根因 B：阻塞 I/O 跑在 Tauri 主线程上（→ 卡死 + 渲染进程崩溃）

四个**同步** Tauri command 都在 Tauri 主线程执行：

| 命令 | 阻塞点 |
|---|---|
| `resolve_windows_directory_shortcut` | COM 解析 `.lnk` |
| `path_exists` | `Path::exists()`；网络盘上能卡好几秒 |
| `reveal_in_explorer` | COM 解析 + 起 `explorer.exe` |
| `get_system_icon` | 2026-11-06 改成「`.lnk` 先解析目标再取目标图标」⇒ COM 解析 + 目标 stat + 图标提取 + PNG 编码 |

前端 `src/composables/system-icon-cache.ts:184-234`（`prefetchSystemIconsForEntries`）按
**条目 × 尺寸**逐个 `invoke('get_system_icon')`。渲染「开始菜单快捷方式」目录时，
每个 `.lnk` 都要走一遍 COM 解析 + 目标 stat ⇒ 主线程被堵死 **61 秒** ⇒
WebView2 心跳断掉 ⇒ 看门狗判「窗口卡死」⇒ reload 失败 ⇒ 渲染进程崩溃，
右键菜单的子菜单加载不出来也是同一个冻结的表现。

---

## 4. 三类弹窗的归属

| 弹窗 | 归属 | 依据 |
|---|---|---|
| 「界面渲染进程已崩溃（窗口 main）」 | 根因 B 的直接后果 | 看门狗日志（§2.2） |
| 「已中止操作」 | **Windows Shell** 对失效快捷方式的响应 | 事故时段应用零 `[reveal]`、零 shell 调用；应用内定位分支从不 shell out |
| 「Windows 找不到 '\\' 文件」 | 同上（Shell 对断链 `.lnk` 的响应） | 同上 |

后两个弹窗的**潜在来源**仍在 `src/utils/resolve-navigable-item-target.ts`：
「打开」动作走 `opensAsFile: true`，会把 `.lnk` **本身**交给系统去执行 ——
系统再去解这个 `.lnk`，断链时就弹自己的框。本轮**未改**这条路径（改动会牵连「打开」语义），
列入真机复验项。

---

## 5. 修复

修复方向：**后端一次往返解析落点（文件/目录目标都支持）+ 所有阻塞 I/O 移出主线程 + 超时兜底**。
原生 Explorer 路径保持「选目标即显示其父目录」的既有正确语义，不动。

### 5.1 Rust

**`src-tauri/src/dir_reader/reveal.rs`**（纯内核，不碰 `tauri::async_runtime`，守 `cargo test --lib` 的 tao 红线）

- 新增 `RevealEntryLocation { directory_path, focus_path, resolved_from_shortcut }`；
- 新增 `resolve_reveal_entry_location(path)`：
  - 不是快捷方式 → 自己的父目录 + 自己；
  - 是快捷方式、目标能解析且**还在** → **目标的父目录** + 目标（**不再要求目标是目录**）；
  - 断链 / 解析失败 / 目标刚好被删 → 静默回落 `.lnk` 自己的父目录 + `.lnk`；
- 新增可注入版 `resolve_reveal_entry_location_with`（单测用假解析器，不碰 COM）；
- 新增 `DEFAULT_REVEAL_TARGET_TIMEOUT_MS = 2500`、`MIN = 200`、`MAX = 10000`、`clamp_reveal_timeout_ms`；
- `reveal_in_explorer_blocking` 的 `prepare_reveal` 失败分支**补日志**
  （以前这条分支一条日志都不打，事后完全看不出走了哪条路）：
  `[reveal] reveal_in_explorer rejected: path=… error=…`；
- 新增 5 个单测（文件目标快捷方式 / 目录目标快捷方式 / 断链回落 / 普通文件保持自身父目录 / 超时 clamp）。

**`src-tauri/src/dir_reader/commands.rs`**（命令层是唯一允许碰 `tauri::async_runtime` 的地方）

- 新增 `RevealEntryTargetPayload`（`#[serde(rename_all = "camelCase")]`：`directoryPath` / `focusPath` / `resolvedFromShortcut`）；
- 新增 `async fn resolve_reveal_entry_target(path, timeout_ms)`：`spawn_blocking` + `with_blocking_timeout`
  （默认 2500ms，clamp 到 200..10000）+ `crate::utils::normalize_path` 归一正斜杠；
- `resolve_windows_directory_shortcut` / `path_exists` / `reveal_in_explorer` 由**同步**改 **async + `spawn_blocking`**
  （语义不变；前端 `invoke` 对同步/异步 command 都是 Promise，调用方无需改）。

**`src-tauri/src/system_icons.rs`**

- `get_system_icon` 拆成 async command（`spawn_blocking`）+ 纯函数 `get_system_icon_blocking`（`pub(crate)`，可被单测链接）。

**`src-tauri/src/clipboard_source.rs`**

- 同步上下文（剪贴板监听线程）改调 `get_system_icon_blocking`（`get_system_icon` 变 async 后的必要跟改）。

**`src-tauri/src/dir_reader/blocking_timeout.rs`**

- `BlockingTimeoutError` 加 `#[derive(Debug)]`（新代码要用 `{:?}` 打印错误）。

**`src-tauri/src/lib.rs`**

- `generate_handler!` 注册 `dir_reader::resolve_reveal_entry_target`。

### 5.2 前端

**`src/utils/resolve-reveal-entry-target.ts`**（重写）

- deps 从 `{resolveShortcutTarget, pathExists}` 改为 `{resolveTarget}`；
- 默认实现走新命令 `invoke('resolve_reveal_entry_target', { path })`；
- 后端返回 null / 抛错 → **静默回落** `getParentDirectory(entryPath)` + `normalizePath(entryPath)`；
- 删掉已无人引用的 `isWindowsShortcutPath` / `resolveWindowsShortcutTarget` / `pathExists` 导出。

**`src/utils/__tests__/resolve-reveal-entry-target.test.ts`**（重写）：6 个用例
（指向文件的快捷方式 / 指向目录的快捷方式 / 断链回落 / 后端报错回落 / 默认走后端命令 / 后端返回空回落）。

### 5.3 接线（未改动，已核对）

`src/utils/reveal-containing-directory.ts` → `openNavigatorRevealEntry`
（`src/utils/open-navigator-directory.ts:227-278`）→ `resolveRevealEntryTarget(entryPath)`
→ `openPathInCurrentTab(directoryPath)` + `setPendingLaunchReveal(directoryPath, focusPath)`。
落点解析结果一路传到导航器并选中目标条目。

---

## 6. 门禁

| 项 | 命令 | 结果 |
|---|---|---|
| Rust 编译 | `cargo check --lib` | 见 §7（本文档落盘时仍在跑） |
| Rust 单测 | `cargo test --lib` | 见 §7 |
| 前端类型 | `npx vue-tsc --build` | **0 错误**（无输出） |
| 前端全量单测 | `npm run test:unit:run` | **Test Files 289 passed / Tests 2706 passed**（与基线完全一致） |
| ESLint（改动文件单检） | `npx eslint src/utils/resolve-reveal-entry-target.ts src/utils/__tests__/resolve-reveal-entry-target.test.ts` | **0 问题**（EXIT=0） |

产物落点：外层仓 `.superpowers/sdd/`：`cargo-test-lib.log`、`cargo-check-lib.log`、
`vue-tsc.log`、`vitest-full.log`、`cargo-errors.txt`、`cargo-error-blocks.txt`。

---

## 7. 未验证项 / 真机验收清单（**打包产物**）

打包命令：`npm run tauri:build`（脚本：`.superpowers/sdd/tauri-build-reveal.cmd`，日志
`.superpowers/sdd/tauri-build-reveal.log`）。**必须用 `tauri build` 的打包产物验收**，
`tauri dev` 的界面一律作废（路由内容区空白，dev 构建特有）。

验收夹具（已建好）：

```
C:\Users\Duanyi\afm-reveal-fixture\
  target-file.txt                 ← 普通文件
  subdir\inner-file.txt           ← 目录目标
  link-to-file.lnk  -> C:\Users\Duanyi\afm-reveal-fixture\target-file.txt
  link-to-dir.lnk   -> C:\Users\Duanyi\afm-reveal-fixture\subdir
  link-broken.lnk   -> C:\afm-no-such-dir\missing.exe   （断链）
```

验收点：

1. 右键 `link-to-file.lnk` → 「打开文件所在目录」⇒ 应跳到 `afm-reveal-fixture` 目录并**选中 `target-file.txt`**；
2. 右键 `link-to-dir.lnk` → 应跳到 `afm-reveal-fixture` 并**选中 `subdir`**；
3. 右键 `link-broken.lnk` → 静默回落（打开 `.lnk` 自己所在目录 + 选中 `.lnk`），**不弹错误、不冻结界面**；
4. 打开含 `.lnk` 的目录（如开始菜单快捷方式目录）⇒ 列表渲染**不卡死**，无「渲染进程已崩溃」弹窗；
5. 三个弹窗（「已中止操作」「渲染进程已崩溃」「Windows 找不到 '\\' 文件」）是否都不再出现；
6. 应用日志里应能看到 `[reveal]` 相关记录（原生分支）或至少不再出现看门狗 `window-hung` 记录。

---

## 8. 本轮踩的坑

1. **`cmd /c` 单行链不能用于 vcvars 环境**：`%PATH%` 会在 `call vcvars64.bat` **之前**展开，
   把 MSVC 路径覆盖掉 ⇒ 误用 Git 的 `C:\Program Files\Git\usr\bin\link.exe`
   （报 `/usr/bin/link: missing operand after '\377\376'`）。一切 Rust 构建走**多行 `.cmd`**。
2. **`cargo test --lib` 的隐形红线**：被测试链接到的代码不能碰 `tauri::AppHandle` /
   `tauri::async_runtime::*`（会拖进 tao，测试二进制加载阶段 0xC0000139 崩掉）。
   ⇒ `spawn_blocking` 只放 command 层；落点解析单测放 `reveal.rs`，用可注入的假解析器，不碰 COM。
3. **`BlockingTimeoutError` 既没有 `Display` 也没有 `Debug`**：只能用 `{:?}`，得先补 `#[derive(Debug)]`。
4. **「重写整个文件」最容易踩的坑**：本轮把 `commands.rs` 整体重写时，
   把 `read::get_link_metadata_batch` 当成了返回 `Result`（实际直接返回 `Vec`），
   多加了一个 `?` ⇒ E0308；又把 `mount_drive` 的签名从 1 个参数改成 2 个 ⇒ E0061。
   ⇒ 改这种文件要**逐个函数对照 HEAD 版**，而不是凭印象重写。
   本轮的对照产物：`.superpowers/sdd/commands-head.rs`（HEAD 版）与 `commands-diff.txt`。
5. **命令改成 async 后，同步调用方要一起改**：`get_system_icon` 改 async ⇒
   `clipboard_source.rs` 的 `.ok().flatten()` 编译不过 ⇒ 改调纯函数 `get_system_icon_blocking`。

---

## 9. 真机验收结果（2026-11-07，`tauri build` 打包产物）

产物：`src-tauri/target/release/alpha-file-manager.exe`（17:01:35 构建，81.5 MB）
+ NSIS 安装包 `src-tauri/target/release/bundle/nsis/Alpha File Manager_3.12.1_x64-setup.exe`（45 MB）。

| 场景 | 实测 | 判定 |
|---|---|---|
| 真实快捷方式（`02文件管理\FileZilla.lnk` → 目标 `D:\Program Files\FileZilla FTP Client\filezilla.exe`） | 右键 → 「打开文件所在目录」⇒ 落到目标所在目录、**选中 `filezilla.exe`**；活工作区文件 `T[9][0].path` 同步为 `D:/Program Files/FileZilla FTP Client` | ✅ |
| 夹具·文件目标（`links\link-to-file.lnk` → `targets\target-file.txt`） | 落到 **`afm-reveal-fixture/targets`**（不是 links 目录），**选中 `target-file.txt`**（27 B），状态栏「已选择 1 个项目」 | ✅ |
| 夹具·断链（`links\link-broken.lnk` → `C:\afm-no-such-dir\missing.exe`） | 静默回落：留在 `links` 目录、选中该 `.lnk`、详情面板「链接状态：已断开」、**无弹窗**、日志零错误 | ✅ |
| 卡死 / 崩溃 | 验收时段看门狗**零** `window-hung` / `recovery failed`（日志里仅存事故当日 08:21:51 / 08:22:51 / 08:24:12 UTC 三条）；应用全程可响应 | ✅ |

**三类弹窗**：验收全程「已中止操作」「渲染进程已崩溃」「Windows 找不到 '\\' 文件」一个都没出现。

证据：外层仓 `.superpowers/sdd/accept-reveal-shots\`（真实快捷方式）、
`accept-reveal-shots-file\`（文件目标）、`accept-reveal-shots-broken\`（断链），
每目录含 `01-before.png` / `02-menu.png` / `03-after.png` + `uia-dump.txt` + `menu-dump.txt` + `summary.txt`。

**用户数据已还原**：验收期间改过活工作区文件 `user-data\user-workspaces.json` 的当前标签路径，
验收后还原为原值并深比对确认（除该字段外逐字段一致，11 个标签页完好），验收进程已退出。

### 9.1 验收过程中新踩的坑

1. **活的工作区文件是 `user-data\user-workspaces.json`**，不是根目录那个同名文件（陈旧副本）。
   本轮先改错了根目录那份，验收时应用仍回到旧目录，白跑一轮；改活文件后一次命中。
2. **改活工作区文件必须先杀进程再改**：应用退出/导航时会把内存状态写回该文件，
   不先杀进程的话，夹具路径会被应用自己的状态覆盖。
3. **「恢复用户数据」顺序固定为「先杀进程 → 再 restore」**，否则应用退出时会把内存态再写回去一次。
4. 右键菜单项在 UIA 里**稳定可枚举**（Radix 暴露成 `ControlType.MenuItem`，13 项），
   本轮一次就命中，不需要轮询重试。

---

## 10. 第二轮（2026-11-07 17:32 用户复报）：「仍然没跳转 + 弹「已中止操作」」

上一轮的真机验收（§9）**结论作废**：验的是「应用内定位」口径，而用户实际走的是**原生 Explorer 分支**，
且用户跑的还是**修复前的安装包**。本轮重新定位，三条硬证据 + 一个当场抓回的自造雷。

### 10.1 证据一：用户运行的是**旧安装版**

`Get-Process alpha-file-manager` → `D:\Program Files\Alpha File Manager\alpha-file-manager.exe`，
mtime `2026/11/6 19:53:52`，FileVersion 3.12.1。
ASCII 符号探针（把 exe 当字节流读出来找 ASCII 串）：

| 二进制 | 含 `resolve_reveal_entry_target` |
|---|---|
| 用户已安装版 `D:\Program Files\Alpha File Manager\…exe` | **否** |
| 新构建 `src-tauri\target\release\alpha-file-manager.exe`（2026-11-07 17:02） | 是 |

⇒ 用户复报时跑的还是**上一轮修复之前**的包。

### 10.2 证据二：AFM **不是**默认资源管理器 ⇒ 走原生 Explorer 分支

`HKCU\Software\Classes\Directory\shell`、`HKCR\Directory\shell`、`HKCR\Directory\OpenShellHandler`
三个位置**全空** ⇒ Rust 的 `is_default_file_manager` 返回 false ⇒
`revealEntriesContainingDirectory` 走 `revealInExplorer()` → `reveal_in_explorer` → `explorer.exe /select,…`。

- 上一轮**根本没改这条分支**（只改了「应用内定位」那条）；
- 上一轮的真机验收是**把 AFM 设成默认文件管理器**之后验的 ⇒ 口径不对；
- 用户截图里的「已中止操作」也正是这条分支把 `.lnk` 交给 shell 之后，Windows 弹的框。

### 10.3 证据三（真凶）：`SLGP_RAWPATH` 返回的是**8.3 短名 + 环境变量块**，不是可用路径

`百度网盘.lnk`（866 B）的二进制里能读到的字符串（ASCII 扫描）：

```
"/D:\\"   "PROGRA~2"   "BAIDUN~1"   "BAIDUN~1.EXE"
"Program"   "D:\\Program Files\\BaiduNetdisk\\BaiduNetdisk.exe"
```

⇒ 这个快捷方式的目标是**用「环境变量块」记录的**：环境变量名 `Program`（= `ProgramFiles`）+
相对路径 `PROGRA~2\BAIDUN~1\BAIDUN~1.EXE`（**8.3 短名**）。
长路径 `D:\Program Files\BaiduNetdisk\BaiduNetdisk.exe` 确实也写在文件里，但
`IShellLink::GetPath(SLGP_RAWPATH)` **不返回它**（RAWPATH = 不做环境变量展开、不做短名还原）。

后果链（与用户现象一一对应）：

1. RAWPATH 解析出的 `...\PROGRA~2\BAIDUN~1\BAIDUN~1.EXE` → `exists()` 必然失败
   ⇒ 落点回落成「`.lnk` 自己所在的目录」⇒ **用户看到「没有跳转」**；
2. 旧逻辑在解析失败时把 `.lnk` **本身**交给 `explorer.exe` ⇒ Explorer 去解析/启动它并失败
   ⇒ 弹 Windows 自己的模态框**「已中止操作」**（框内路径正是 `.lnk` 自身，与用户截图一致）。

**对照物（说明上一轮为什么是假阳性）**：`FileZilla.lnk`（917 B）把目标存成**字面长路径**
`D:\Program Files\FileZilla FTP Client\FileZilla.exe` ⇒ RAWPATH 照样能解析 ⇒ 上一轮拿它验收当然「通过」。
⇒ **验收样本必须挑目标存成环境变量块的快捷方式**，否则测的是另一条路径。

第三方 COM（`WScript.Shell`）读同一个 `.lnk` → `D:\Program Files\BaiduNetdisk\BaiduNetdisk.exe`（Exists=True）
⇒ 目标本身是好的，坏的只是**我们的解析方式**。

### 10.4 本轮自造、当场抓回的雷：`IShellLinkW::Resolve()` 会弹模态框并**永久阻塞**

第一版修法是三级取：`GetPath(flags=0)` → `Resolve(HWND(NULL), 0)` → `GetPath(SLGP_RAWPATH)` + 手工展开。
跑 `cargo test --lib` 时，用例 `system_icons::tests::shortcut_icon_target_resolves_lnk_target`
**卡了 60 秒以上不结束**（它用一个指向不存在文件的断链 `.lnk`）。
枚举顶层窗口抓到一个**可见**的 `#32770`，标题「快捷方式存在问题」，
而它的 **owner pid 就是测试二进制自己**（`sigma_file_manager_lib-*.exe`，pid 581536）。

⇒ `IShellLink::Resolve` 在目标失效时会弹 **Windows 自己的模态框**；
`hwnd = NULL` 时那个框没有父窗口 ⇒ 调用方**永久阻塞**。
更要命的是 `resolve_windows_shortcut_target` 还被**目录列表**调用（`dir_reader.rs:49`）
⇒ 列表里只要出现一个断链快捷方式，整个应用就会卡死（正是用户看到的「渲染进程已崩溃」形态）。

**修法**：删掉 `Resolve` 那一级，改成两级
（`GetPath(flags=0)` 让 shell 自己展开 → `GetPath(SLGP_RAWPATH)` + 我们手工展开 `%VAR%`），
取到第一个 `exists()` 的路径就返回。

### 10.5 本轮改动

| 文件 | 改动 |
|---|---|
| `src-tauri/src/dir_reader/read.rs` | `resolve_windows_shortcut_target`：两级取（删掉会弹框的 `Resolve`）；新增 `expand_environment_variables` |
| `src-tauri/src/dir_reader/reveal.rs` | 新增 `RevealMode { Select, OpenDirectory }`；目标解析不出/已失效 ⇒ 打开 `.lnk` **所在目录**，**绝不把坏 `.lnk` 交给 shell**；新增 `build_open_argument`；日志加 `mode={:?}` |
| `src/utils/resolve-reveal-entry-target.ts` | payload 透出 `resolvedFromShortcut`（前端判断「解析成功与否」的判据） |
| `src/utils/reveal-containing-directory.ts` | 原生分支改成「先解析落点、再交给 Explorer」：解析成功传目标路径，解析不出只传所在目录 |
| `src/utils/__tests__/resolve-reveal-entry-target.test.ts` | 6 个用例补 `resolvedFromShortcut` |
| `src/utils/__tests__/reveal-containing-directory.test.ts` | 新增 mock + 2 条用例（快捷方式→传目标；解析不出→只传所在目录） |

门禁：`cargo test --lib` → **944 passed / 0 failed / 1 ignored**（8.62 s，断链用例不再卡死）；
探针 `probe_real_shortcut_target` → `PROBE_TARGET=Some("D:\\Program Files\\BaiduNetdisk\\BaiduNetdisk.exe")`；
`npm run tauri:build` → `TAURI_BUILD_EXIT=0`，
NSIS 包 `Alpha File Manager_3.12.1_x64-setup.exe`（45,149,314 B，2026-11-07 18:00:54，
sha256 `2918147330C51D4444585BB4C4D8A756875A786449BA57448F5707DFE2EDDABE`）。



## 11. 真机验收 · 原生 Explorer 分支（2026-11-07 18:1x–18:2x，release 打包产物）

### 11.1 为什么必须单独覆盖这一分支

用户 2026-11-07 17:32 复报的「**没有跳转 + 弹『已中止操作』**」发生在 **原生 Explorer 分支**：
旧逻辑把 `.lnk` **自己**交给 `explorer.exe /select,…`，Explorer 解不出目标 ⇒ 弹系统模态框「已中止操作」（框内路径正是 `.lnk` 自身）。

而上一轮验收（§4.5）走的是**应用内分支**：当时 `is_default_file_manager` 返回 true，
前端 `reveal-containing-directory.ts` 走 `openNavigatorRevealEntry` ⇒ **根本没调用** `reveal_in_explorer`
⇒ 上一轮的「✅」对用户实际遇到的分支**不成立**（假阳性）。

### 11.2 分支判定机制（本轮查清）

`src-tauri/src/default_file_manager.rs`：

- `detect_default_file_manager()`（:269）→ `launcher_registry_owner()`（:108）
  → `registry_is_owned_by_launcher()`（:96）逐条比对**三个** command key 的值是否等于本机 launcher 路径：
  - `HKCU\Software\Classes\Folder\shell\open\command`
  - `HKCU\Software\Classes\Folder\shell\explore\command`
  - `HKCU\Software\Classes\CLSID\{52205fd8-5dfb-447d-801a-d0b52f2e83e1}\shell\opennewwindow\command`
- 三个全中且 `DelegateExecute` 为空 ⇒ 命中 `LauncherRegistryOwner::Current`
  ⇒ 再看 `launcher::deployment_is_current()`；命中 `Legacy` ⇒ 直接 true。
- **一个都不中**（owner = None）⇒ 回落到「当前注册表快照 == `registry-snapshot.json` 的 `owned_keys`」比对。

本机实况（`reg query` 转储）：AFM 已接管上述三个键（值指向
`C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.default-file-manager\sigma-file-manager-launcher.exe`），
`registry-snapshot.json`（`%LOCALAPPDATA%\com.sigma-file-manager.default-file-manager\`，18 151 B）的
`original_keys` 三个根全为 `existed=false`、`owned_keys` 全为 `existed=true` ⇒ AFM 是默认资源管理器 ⇒ 走应用内分支。

### 11.3 手法（临时翻到原生分支，事后精确还原）

`registry-snapshot.json` 是应用自己写的接管前后快照，`ROOT_REGISTRY_KEYS` 等常量来自 crate
`sfm_default_file_manager_common`（`windows_installation.rs:23-27` 的 `pub use`）。

把 `open` / `explore` 两个 command 的默认值临时改回 `"C:\WINDOWS\explorer.exe" "%1"`
⇒ owner = None ⇒ 快照比对也不等 ⇒ `is_default_file_manager` = **false** ⇒ 前端走原生分支。
验收脚本 `.superpowers/sdd/run-accept-reveal4.ps1`（纯 ASCII）在 `finally` 里用
`RegistryKey.SetValue` 写回**原值**并逐项比对（`RESTORED-OK`）。

> 注意：改注册表属于改用户系统状态，**必须**先读原值、事后逐项还原并打印还原结果；
> 顺序固定为「先杀应用 → 再还原注册表与工作区」。

### 11.4 验收结果（全部通过）

| 检查项 | 实测 |
|---|---|
| 右键目标 | 开始菜单 `02文件管理` 目录里的 `百度网盘.lnk`（UIA 命中 `ROW 百度网盘 at 571,300`），右键菜单 13 项，Invoke「打开文件所在目录」 |
| **落点** | Explorer 窗口 `file:///D:/Program%20Files/BaiduNetdisk`（解码后 `D:\Program Files\BaiduNetdisk`）——**目标所在目录**，不是 `.lnk` 所在目录 |
| **选中** | `focus` 与 `selected` 均为 `D:\Program Files\BaiduNetdisk\BaiduNetdisk.exe` —— **选中的就是快捷方式指向的文件** |
| 系统弹框 | **无任何可见 `#32770`** ⇒ 没有「已中止操作」，也没有「快捷方式存在问题」 |
| 分支确认 | 应用内当前标签页仍为 `C:/ProgramData/Microsoft/Windows/Start Menu/Programs/02文件管理`（**未跳转**）⇒ 证明本轮走的是原生分支，不是应用内分支 |
| 应用日志 | `[reveal] reveal_in_explorer: path=D:/Program Files/BaiduNetdisk/BaiduNetdisk.exe selected=… argument=/select,"D:\Program Files\BaiduNetdisk\BaiduNetdisk.exe" from_shortcut=false mode=Select` ⇒ 交给 Explorer 的是**目标**，全程无 `.lnk` |

证据：`.superpowers/sdd/accept-reveal4/`（`01-before.png` / `02-menu.png` / `03-after.png` /
`uia-dump.txt` / `menu-dump.txt` / `explorer-windows.txt` / `dialogs.txt` / `app-log-reveal.txt` / `summary.txt`）。

### 11.5 本轮结论与遗留

- **两个分支现在都正确**：应用内分支（§4.5）与原生 Explorer 分支（本节）都落到目标目录并选中目标文件，
  都不再弹「已中止操作」，也都不再把 `.lnk` 交给 shell。
- 临时探针 `probe_real_shortcut_target`（`// TEMP-PROBE-REMOVE`）**已删除**；
  删后复跑 `cargo test --lib` ⇒ **943 passed / 0 failed / 1 ignored**（6.05 s），无新增告警，未触发 tao 红线。
- **验收脚本自身的一处判据瑕疵**（不是产品缺陷）：`afm-accept-reveal3.ps1` 取**最后一条** `[reveal]` 日志做断言，
  而最后一条是 `reveal_in_explorer spawned:`（不含 `mode=`），于是报了 `FAIL app log does not show the TARGET path with mode=Select`；
  实际**第一条**日志就带 `mode=Select`（见 `app-log-reveal.txt`）。判据应改成「**任一条**含 `mode=Select` 且**不含** `.lnk`」。
- **推断（未单独取证）**：用户 17:32 那次之所以走原生分支，很可能是当时装的是 2026-11-06 19:53 的旧包
  （ASCII 符号探针证明旧包**不含** `resolve_reveal_entry_target`），而 11-07 重新安装后 launcher 部署被刷新
  ⇒ `deployment_is_current` 转为 true ⇒ 应用内分支。**两条分支本轮都已实测通过**，故不再单独复现该推断。
- 用户数据已还原：`user-workspaces.json` 3637 B → 3637 B；两个注册表 command 值 `RESTORED-OK`（与原值逐字相同）。
