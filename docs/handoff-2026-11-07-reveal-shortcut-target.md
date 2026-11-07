# 交接卡 · 2026-11-07 「打开文件所在目录」对快捷方式失效 + 界面卡死 / 渲染进程崩溃

> 新会话请先读本文件，再读 `docs/work-2026-11-07-reveal-shortcut-target.md`（完整排查记录，含 §10 根因与 §11 真机验收），
> 以及 `docs/handoff-2026-11-07-reveal-shortcut-icon-and-native-separator.md`（上一轮交接，背景）。

---

## 1. 背景与目标

内层仓 `F:\soft\00selfmade\filemanager\sigma-file-manager`（Alpha File Manager / Sigma File Manager），
基线 **3.12.1** 已发版。外层仓 `F:\soft\00selfmade\filemanager` 只放 `docs/`、`AGENTS.md`、`.superpowers/`。

**用户诉求**（两条，均已闭环，真机验收结论见 §4.5 与 §4.6）：

1. 右键快捷方式 → 「打开文件所在目录」应跳到**快捷方式指向的目标文件所在目录**并选中目标；
   实际是「点了没反应」+「弹『已中止操作』」+「右键子菜单加载不出来」+「界面渲染进程已崩溃（窗口 main）」。
2. 消除三张截图里的弹窗：「已中止操作」「渲染进程已崩溃」「Windows 找不到 '\\' 文件」。

**本卡已更新到 2026-11-07 18:2x 第三轮收尾**：原生 Explorer 分支已真机覆盖，两个分支都通过。

## 2. 根因（已坐实，证据在 §5）

- **根因 A（功能缺陷）**：解析快捷方式目标时只认「目录目标」，
  指向**文件**的快捷方式（绝大多数）解析失败 ⇒ 前端回落成「打开 `.lnk` 自己所在目录」⇒「点了没反应」。
- **根因 B（卡死/崩溃）**：`.lnk` 相关的解析 / `path_exists` / `reveal_in_explorer` / `get_system_icon`
  都是**同步 Tauri command**（跑在 Tauri 主线程）。2026-11-06 起 `get_system_icon`
  对 `.lnk` 要「解析目标再取目标图标」，前端 `system-icon-cache.ts` 又按「条目 × 尺寸」逐个 invoke
  ⇒ 渲染开始菜单目录时主线程被堵死 **61 秒** ⇒ 心跳断 ⇒ 看门狗 reload 失败 ⇒ 渲染进程崩溃。
- **根因 C（第二轮，用户实际遇到的分支）**：
  ① `IShellLinkW::Resolve(HWND(NULL), 0)` 在目标失效时弹 Windows 自己的模态框「快捷方式存在问题」且**调用方永久阻塞**
     （测试二进制当场卡死 60 s+）；② 目标以**环境变量块 + 8.3 短名**记录（`PROGRA~2\BAIDUN~1\BAIDUN~1.EXE`）时
     `SLGP_RAWPATH` 返回的不是真实路径 ⇒ `exists()` 必失败 ⇒ 回落成「`.lnk` 所在目录」；
  ③ 解析失败时把 `.lnk` **自己**交给 `explorer.exe /select,…` ⇒ Explorer 失败 ⇒ 弹「**已中止操作**」。
- **三个弹窗**：「渲染进程已崩溃」= 根因 B 的后果；「已中止操作」「Windows 找不到 '\\' 文件」= Windows Shell 对失效快捷方式的响应。

## 3. 已完成的改动（代码已落地，门禁见 §4）

Rust：

| 文件 | 改动 |
|---|---|
| `src-tauri/src/dir_reader/read.rs` | `resolve_windows_shortcut_target` 改为**两级**取：`GetPath(flags=0)`（shell 自展开）→ `GetPath(SLGP_RAWPATH)` + 手工 `expand_environment_variables()`；**删掉 `IShellLink::Resolve` 这一级**（会弹模态框并永久阻塞） |
| `src-tauri/src/dir_reader/reveal.rs` | 新增 `RevealMode { Select, OpenDirectory }`；`RevealTarget` / `PreparedReveal` 增 `mode`；新增 `build_open_argument()`；解析不出 / 目标失效 ⇒ `OpenDirectory`（打开 `.lnk` 所在目录，**绝不把坏 `.lnk` 交给 shell**）并 `log::warn!("[reveal] shortcut target …")`；`prepare_reveal` 失败分支补 `[reveal] reveal_in_explorer rejected` 日志 |
| `src-tauri/src/dir_reader/commands.rs` | 新增 `resolve_reveal_entry_target`（async + `spawn_blocking` + 超时 + `normalize_path`）与 `RevealEntryTargetPayload`（camelCase）；`resolve_windows_directory_shortcut` / `path_exists` / `reveal_in_explorer` 同步 → async + `spawn_blocking` |
| `src-tauri/src/system_icons.rs` | `get_system_icon` 拆 async command + 纯函数 `get_system_icon_blocking` |
| `src-tauri/src/clipboard_source.rs` | 同步上下文改调 `get_system_icon_blocking` |
| `src-tauri/src/dir_reader/blocking_timeout.rs` | `BlockingTimeoutError` 加 `#[derive(Debug)]` |
| `src-tauri/src/lib.rs` | `generate_handler!` 注册新命令 |

前端：

| 文件 | 改动 |
|---|---|
| `src/utils/resolve-reveal-entry-target.ts` | 改走后端命令 `resolve_reveal_entry_target`；payload 透出 `resolvedFromShortcut` |
| `src/utils/reveal-containing-directory.ts` | **原生分支**改为「先解析落点再交给 Explorer」：解析成功传 `focusPath`（目标）；`.lnk` 且 `resolvedFromShortcut===false` 时传 `directoryPath`（所在目录） |
| `src/utils/__tests__/resolve-reveal-entry-target.test.ts` | 6 个用例补 `resolvedFromShortcut` |
| `src/utils/__tests__/reveal-containing-directory.test.ts` | 新增 mock + 2 个用例 |

`reveal.rs` 保持**纯内核**（不碰 `tauri::AppHandle` / `tauri::async_runtime::*`），`spawn_blocking` 只放 command 层。

## 4. 门禁现状

| 项 | 结果 |
|---|---|
| `npx vitest run`（2 个受影响测试文件） | ✅ 2 files / 12 tests 全过 |
| ESLint（4 个改动文件单检） | ✅ 0 问题（`eslint .` 全量基线见 §6.1，**未重跑**） |
| `cargo check --lib` | ✅ 0 错误 |
| `cargo test --lib` | ✅ **943 passed / 0 failed / 1 ignored**（6.05 s，删探针后复跑），未触发 tao 红线 |
| `tauri build`（release 打包） | ✅ `src-tauri/target/release/alpha-file-manager.exe`（81 500 160 B）+ NSIS `Alpha File Manager_3.12.1_x64-setup.exe`（45 149 314 B，sha256 `2918147330C51D4444585BB4C4D8A756875A786449BA57448F5707DFE2EDDABE`） |

## 4.5 真机验收结论 · 应用内分支（2026-11-07 17:05–17:25）

产物：`src-tauri/target/release/alpha-file-manager.exe`；静默安装到用户实际路径
`D:\Program Files\Alpha File Manager\alpha-file-manager.exe`（mtime 2026-11-07 18:00:12）。

| 场景 | 实测结果 | 判定 |
|---|---|---|
| 真实快捷方式（文件目标） | 右键 `…\02文件管理\FileZilla.lnk` → 落到 `D:\Program Files\FileZilla FTP Client`，选中 `filezilla.exe`，活工作区 `T[9][0].path` 同步 | ✅ |
| 夹具·文件目标 | 落到 `C:/Users/Duanyi/afm-reveal-fixture/targets`，选中 `target-file.txt`（27 B） | ✅ |
| 夹具·断链 | 静默回落：停留在 `links` 目录、选中 `link-broken.lnk`、**无任何弹窗** | ✅ |
| 卡死 / 崩溃 | 验收时段 `window-hung` / `recovery failed` **零新增** | ✅ |

## 4.6 真机验收结论 · 原生 Explorer 分支（2026-11-07 18:1x–18:2x）——**用户实际遇到的分支**

判定机制与手法见排查记录 §11.2 / §11.3：把 `HKCU\Software\Classes\Folder\shell\open|explore` 两个 command
临时改回 `explorer.exe` ⇒ `is_default_file_manager` = false ⇒ 前端走原生分支；验收后逐项还原（`RESTORED-OK`）。

| 检查项 | 实测 | 判定 |
|---|---|---|
| 右键目标 | `…\02文件管理\百度网盘.lnk`（目标以**环境变量块 + 8.3 短名**记录，就是用户点的那一个） | — |
| 落点 | Explorer 窗口 `D:\Program Files\BaiduNetdisk`（**目标所在目录**，不是 `.lnk` 所在目录） | ✅ |
| 选中 | `focus` = `selected` = `D:\Program Files\BaiduNetdisk\BaiduNetdisk.exe` | ✅ |
| 系统弹框 | 无任何可见 `#32770` ⇒ **无「已中止操作」** | ✅ |
| 分支确认 | 应用内标签页**未跳转**（仍是快捷方式所在目录）⇒ 确实走原生分支 | ✅ |
| 应用日志 | `argument=/select,"D:\Program Files\BaiduNetdisk\BaiduNetdisk.exe" … mode=Select`，全程无 `.lnk` | ✅ |

> 验收脚本 `afm-accept-reveal3.ps1` 有一处**判据瑕疵**（非产品缺陷）：它取**最后一条** `[reveal]` 日志做断言，
> 而最后一条是 `… spawned:` 行（不含 `mode=`）⇒ 误报 `FAIL … mode=Select`；实际**第一条**就带 `mode=Select`
> （`accept-reveal4/app-log-reveal.txt`）。判据应改成「**任一条**含 `mode=Select` 且**不含** `.lnk`」。

**用户数据已还原**：`user-workspaces.json` 3637 B → 3637 B；两个注册表 command 值与原值逐字相同（`RESTORED-OK`）。

## 5. 证据路径

- 应用日志：`C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.app\logs\Alpha File Manager.log`（**UTC** 时间戳）
- 原生分支证据：外层仓 `.superpowers/sdd/accept-reveal4\`（`01-before.png` / `02-menu.png` / `03-after.png` / `uia-dump.txt` / `menu-dump.txt` / `explorer-windows.txt` / `dialogs.txt` / `app-log-reveal.txt` / `summary.txt`）
- 编排脚本：`.superpowers/sdd/run-accept-reveal4.ps1`（注册表翻转 + 验收 + 还原，纯 ASCII）；`afm-accept-reveal3.ps1`（UIA 验收主体）；`accept-reveal3-prep.cjs`（工作区 patch/restore）
- 门禁日志：`.superpowers/sdd/cargo-test-reveal5.log`（943 passed）、`tauri-build-reveal4.log`、`gate3-reveal.stdout.log`
- 注册表快照（应用自己写的接管前后状态）：`C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.default-file-manager\registry-snapshot.json`
- 验收夹具：`C:\Users\Duanyi\afm-reveal-fixture\{links,targets}\`（**links 与 targets 必须分属不同目录**）
- 应用内分支证据：`.superpowers/sdd/accept-reveal3\`

## 6. 下一步（按顺序）

1. **提交前跑一次 `eslint .` 全量对照基线**（**用 `git stash push -u` 量 HEAD**，基线数字随提交变；
   已知 2026-10-23 的基线是 188 errors / 0 warnings，`.superpowers/` 与 `graphify-out/`
   已在 `eslint.config.ts` 的 `globalIgnores` 里）。
2. 决定发版节奏：本次改动含 `src-tauri` 的 command 签名变更（同步 → async + `spawn_blocking`），
   建议按 **3.12.2 补丁版**发；发版前把 `package.json` / `src-tauri/Cargo.toml` / `tauri.conf.json`
   三处版本号对齐（历史上踩过 `.cmd` 里 `>` 把版本顶掉的坑，见 `AGENTS.md`）。
3. 两个分支均已验收通过，**不需要**再做分支覆盖类验收。
4. 夹具目录 `C:\Users\Duanyi\afm-reveal-fixture` 留在用户盘上（工作区外删除被安全策略拦截），下轮复用即可。

## 7. 本轮踩的坑（务必先读）

1. **`IShellLinkW::Resolve(HWND(NULL), 0)` 会弹 Windows 自己的模态框并让调用方永久阻塞**：
   目标失效时弹「快捷方式存在问题」，owner pid = 调用方（测试二进制）⇒ `cargo test --lib` 卡 60 s+ 一个用例都跑不完。
   ⇒ 解析快捷方式**只用** `GetPath`（两级），**不要**调 `Resolve`；同理别在列表渲染路径上解 `.lnk`。
2. **`SLGP_RAWPATH` 对「环境变量块 + 8.3 短名」的目标返回的不是真实路径** ⇒ `exists()` 必失败。
   ⇒ 顺序必须是 `GetPath(0)`（shell 自展开）→ `RAWPATH` + 手工 `expand_environment_variables()`；
   验收样本必须用这种 `.lnk`（如 `百度网盘.lnk`），用 `FileZilla.lnk` 会得到假阳性。
3. **`cmd /c` 单行链不能用于 vcvars**：`%PATH%` 会在 `call vcvars64.bat` 之前展开，
   覆盖掉 MSVC 路径 ⇒ 误用 Git 的 `link.exe`。⇒ Rust 构建一律走**多行 `.cmd`**。
4. **`cargo test --lib` 红线**：被测试链到的代码不能碰 `tauri::AppHandle` / `tauri::async_runtime::*`
   ⇒ 拖进 tao/Wry ⇒ 测试二进制加载阶段 0xC0000139 崩掉。
   ⇒ `spawn_blocking` 只放 command 层；`reveal.rs` 的落点单测必须用**可注入的假解析器**，不碰 COM。
5. **分支判定要看注册表，不要猜**：`is_default_file_manager` 比对的是
   `HKCU\Software\Classes\Folder\shell\open|explore` 与 `HKCU\Software\Classes\CLSID\{52205fd8-…}\shell\opennewwindow`
   三个 command 值；改任一个即可把它翻成 false（验收完必须还原）。
6. **改用户系统状态（注册表）的规矩**：先读原值 → 临时改 → 验收 → **先杀应用** → 逐项还原并打印还原结果。
7. **后台任务输出要用重定向到文件**，不要挂 stdout 管道（`Tee-Object` / `| Select-String`）：管道一断进程树跟着死。
8. **`vitest --reporter=basic` 在本仓库不存在**，跑全量用默认 reporter，再按 `Test Files` / `Tests` 两行抓结果。
9. **验收脚本里的断言要看全量再判**：「取最后一条日志」这类写法会误报 FAIL，判据要改成「任一条满足」。
10. **PowerShell 里 `Get-Content -Raw` / `Set-Content` 改带中文的文件会把中文变 mojibake** ⇒ 用编辑工具或 node `fs.*Sync(...,'utf8')`。

## 8. 接力提示

- 一句话 prompt（给新会话）：
  「读 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-07-reveal-shortcut-target.md`
  与同目录 `docs\work-2026-11-07-reveal-shortcut-target.md`，接手『打开文件所在目录』快捷方式 bug 的**发版收尾**：
  代码、门禁、打包、两个分支的真机验收都已完成且通过，剩余只有 eslint 全量基线对照与发版决定。」
- 状态外置：所有中间产物（脚本、日志、证据）落外层仓 `F:\soft\00selfmade\filemanager\.superpowers\sdd\`；
  对话里只留指针 + 结论，不贴长输出。
- 当前工作副本**未提交**（HEAD 仍是 `b49fbd31`）：改动文件 =
  `src-tauri/src/dir_reader/{read.rs,reveal.rs,commands.rs}`、`src-tauri/src/{lib.rs,system_icons.rs,clipboard_source.rs}`、
  `src-tauri/src/dir_reader/blocking_timeout.rs`、`src/utils/{resolve-reveal-entry-target.ts,reveal-containing-directory.ts}`、
  `src/utils/__tests__/{resolve-reveal-entry-target,reveal-containing-directory}.test.ts`。
  临时探针 `probe_real_shortcut_target` **已删除**。
