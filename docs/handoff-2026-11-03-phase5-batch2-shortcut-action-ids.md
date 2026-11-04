# 交接文档 · 2026-11-03 · 阶段五批次 2：快捷键绑定目标改为动作注册表 id

> **本文是阶段五批次 2 的交接入口。** 批次 3 及以后仍以
> `docs/plan-2026-11-02-action-registry-and-incremental-index.md` 为准。
> 面向用户的文字一律简体中文；代码标识符、路径、命令、配置键名保持原样。

---

## §0 必读文档（完整路径，按顺序）

| # | 文档 | 作用 |
|---|---|---|
| 1 | `F:\soft\00selfmade\filemanager\AGENTS.md` | 项目硬约束总纲，**优先级最高**（注入时被截断到 32 KB，需要时直接读源文件） |
| 2 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-03-phase5-batch2-shortcut-action-ids.md` | **本文**，批次 2 的交付内容、语义决策、真机验收结果、本轮新踩的坑 |
| 3 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\plan-2026-11-02-action-registry-and-incremental-index.md` | 阶段五计划：批次 1–7 的顺序、判据、风险、明确不做的事 |
| 4 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\analysis-2026-11-02-files-benchmark.md` | 与 [files-community/Files](https://github.com/files-community/Files) 的对标分析，阶段五的需求来源 |
| 5 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-02-phase4-winget-and-search-scan.md` | 上一阶段交接（winget 3.9.2 + 搜索扫描并行化），含大量历史踩坑 |
| 6（选读） | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-03-phase5-batch1-action-registry.md` | 批次 1 交接原文。批次 1 的语义决策已在本文 §3.2 继承，**不必读**，需要细节时再翻 |

---

## §1 项目坐标

- **仓库根**：`F:\soft\00selfmade\filemanager`（`AGENTS.md` 在此）。**应用仓库是它的子目录** `sigma-file-manager`，
  几乎所有 `git` / `npm` 命令都要带 `-C '<绝对路径>'` 或在子目录里执行。
- **技术栈**：Vue 3 + TypeScript + Pinia + Vite + Tauri 2 + Rust；前端单测 Vitest，组件库自建（`src/components/ui/*`）。
- **版本**：`3.9.2`，三处必须一致 —— `package.json`、`src-tauri\Cargo.toml`、`src-tauri\tauri.conf.json`（`npm run sync-version` 负责）。
- **构建**：`npm run tauri:build`（= `sync-version` → `tauri build` → `publish-bundle-to-release.mjs`；末步只把安装包复制到 `release/`，**不上传**）。
- **真机验收铁律**：必须用 `tauri build` 的打包产物。`tauri dev` 下**所有路由页内容区空白**，dev 上的界面结论一律作废。
- **最近提交**：`ceff7e38 docs(handoff): 新增阶段四交接文档并更新接力 prompt`。

---

## §2 当前状态（事实源，2026-11-03 核验）

### 2.0 ⚠️ 先读这条：批次 2 的前提被推翻了

上游批次 1 交接写的批次 2 任务是「**快捷键设置页现在只读列举，要变成可编辑**」。
**这条前提与代码不符，已被推翻。**

`src/modules/settings/ui/categories/shortcuts/shortcuts.vue` **原本就具备完整编辑能力**：

| 能力 | 代码位置 |
|---|---|
| 录制按键（`isRecording`） | `shortcuts.vue` |
| 保存自定义键位（`saveShortcut`） | `shortcuts.vue` |
| 恢复默认（`resetShortcut`） | `shortcuts.vue` |
| 四类冲突检测 | `shortcuts.vue` |
| 解除绑定（`contextUnassignShortcutBinding`） | `shortcuts.vue` |
| 备用键位（alternate slot） | `shortcuts.vue` |
| 按名称搜索过滤 | `shortcuts.vue` |

⇒ 批次 2 的真实剩余工作被收敛为「**绑注册表 id + 旧格式兼容读取**」，
**本轮没有新增任何界面功能**，用户可见变化只有一处：用户改键后配置里存的是新格式键，同条旧键被删掉。

### 2.1 代码：批次 2 已落盘

新增目录 `src/core/actions/` 下两个新文件（**`src/core/` 整体仍未跟踪**）：

| 文件 | 作用 |
|---|---|
| `src\core\actions\shortcut-actions.ts` | 动作 id 的构造/解析/判别：`SHORTCUT_ACTION_ID_PREFIX='sigma.shortcut.'`、`SHORTCUT_ACTION_SOURCE_ID='sigma.shortcuts'`、`buildShortcutActionId` / `isShortcutActionId` / `parseShortcutActionId` / `buildShortcutActionRegistry` |
| `src\core\actions\shortcut-bindings.ts` | 绑定值形状判别与查表：`isShortcutKeys` / `isShortcutBindingValue` / `pickShortcutBindingSlot` / `lookupShortcutBinding`（原始双查）/ `migrateShortcutBindings` |

新增测试 4 个文件（`src/core/actions/__tests__/shortcut-actions.test.ts`、
`src/core/actions/__tests__/shortcut-bindings.test.ts`、
`src/stores/runtime/__tests__/shortcuts-action-binding.test.ts`、
`src/stores/runtime/__tests__/shortcuts-legacy-fallback.test.ts`）。

改动的既有文件（4 个 + 2 个是用户自己的未提交改动，**别动**）：

- `src\core\actions\index.ts` —— 导出上述两个模块。
- `src\types\user-settings.ts` —— `UserShortcuts` 的键类型由 `ShortcutId` 放宽为 `string`
  （新格式键带命名空间，类型上不再是 `ShortcutId`）。
- `src\stores\runtime\shortcuts.ts` —— 新增 `knownShortcutIds: Set<string>`、`shortcutActionRegistry`、
  `migratedShortcutBindings`、`getShortcutActionId`、`resolveShortcutBindingEntry`、
  `getShortcutBindingFormat`、`persistShortcutBindings`；`setShortcutForAction` /
  `resetShortcutForAction` 改走新格式。
- `src\modules\settings\ui\categories\shortcuts\shortcuts.vue` —— **只做外科式改动**（6 处调用点改为按注册表 id 查绑定），
  不重构这个 2000 行文件。

### 2.2 门禁（本轮实测）

| 门禁 | 命令 | 结果 |
|---|---|---|
| 定向单测 | `npx vitest run src/core/actions src/stores/runtime/__tests__/` | **22 files / 238 tests passed** |
| 类型 | `npm run ts` | **exit 0** |
| 改动文件 eslint | `npx eslint <10 个改动文件绝对路径>` | **0** |
| 全量 eslint 基线对照 | 同一条 `npx eslint .`，当前树 vs `git stash push -u` 后的 HEAD | 当前 **186 errors / 0 warnings**，HEAD **186 errors / 0 warnings** ⇒ **零新增违规** |
| 全量单测 | `npm run test:unit:run` | **274 files：270 passed / 4 failed；2556 tests：2536 passed / 20 failed** |

#### 2.2.1 那 20 条失败 = 既有失败，**不是本轮回归**

在 HEAD（`ceff7e38`）上建独立 worktree 复跑同样 4 个文件，**失败集与失败条数逐条相同**：

| 失败文件 | 条数 | 归属 |
|---|---:|---|
| `src/modules/navigator/components/file-browser/__tests__/file-browser-tree-view.test.ts` | 12 | HEAD 既有 |
| `src/modules/navigator/components/file-browser/composables/__tests__/use-file-browser-navigation.test.ts` | 6 | HEAD 既有 |
| `src/utils/__tests__/module-load-recovery.test.ts` | 1 | HEAD 既有 |
| `src/utils/__tests__/readme-markdown-table.test.ts` | 2 | HEAD 既有（扫 `docs/` 下表格列数，HEAD 版本即不一致） |

⇒ **批次 2 零回归**。

### 2.3 真机界面验收

**结论：通过（12 条断言，0 失败）。** 2026-11-03 09:17 实跑，
日志 `.superpowers\sdd\accept-b2-shortcuts.log`，截图 `.superpowers\sdd\accept-b2-shots\*.png`。

- **产物**：`src-tauri\target\release\alpha-file-manager.exe`（`tauri build` release，09:14:32），
  **不是** `tauri dev`。
- **环境**：**隔离沙箱桌面**（`CreateDesktop` + `EnumDesktopWindows` + `STARTUPINFO.lpDesktop`），
  注入端**不切桌面**，操作者桌面前台窗口全程未被抢（B1 / E2 两条断言）。
- **脚本**：`.superpowers\sdd\accept-b2-shortcuts.ps1`（纯 ASCII）。

| 阶段 | 断言 | 结果 |
|---|---|---|
| A 备份 | A1 真实 `user-settings.json` 备份逐字节一致（5382 B） | PASS |
| A 播种 | A2 夹具落在「快捷键」设置页 | PASS |
| B 读路径 | B1 操作者桌面前台窗口全程未被改变 | PASS |
| B 读路径 | **B2 旧格式绑定被 release 读回**（显示 `Ctrl+Alt+Shift+K`） | PASS |
| B 读路径 | **B3 新格式（动作注册表 id）绑定被 release 读回**（显示 `Ctrl+Alt+Shift+J`） | PASS |
| C 写路径 | C1 既有新格式绑定仍以动作 id 存储 | PASS |
| C 写路径 | **C2 旧格式键被写路径删除** | PASS |
| C 写路径 | **C3 被改动的动作以 `sigma.shortcut.toggleCommandPalette` = `{"key":""}` 落盘** | PASS |
| D 重启 | **D1 重启后旧绑定不复活** | PASS |
| D 重启 | **D2 重启后动作 id 绑定仍在** | PASS |
| E 还原 | E1 真实用户数据逐字节还原（settings / workspaces / stats 三件） | PASS |
| E 还原 | E2 操作者桌面前台窗口自始至终未被抢 | PASS |

**写路径实测**：右键「Open / close command palette」行 → 菜单「取消绑定」→ 落盘的 `shortcuts` 段
从 `{toggleCommandPalette, sigma.shortcut.toggleAddressBar}`
变为 `{sigma.shortcut.toggleAddressBar, sigma.shortcut.toggleCommandPalette: {"key":""}}`。
即 **写新格式键 + 删同条旧格式键**，与设计一致。

**界面证据**：`b2-phaseB-read.png` 同屏可见「Open / close command palette = `Ctrl+Alt+Shift+K`」
（旧格式读回）与「切换到地址栏并打开地址输入框 = `Ctrl+Alt+Shift+J`」（新格式读回）；
`b2-phaseD-restart.png` 可见重启后前者显示 `—`（已解除且未复活）、后者仍是 `Ctrl+Alt+Shift+J`。

**数据安全**：验收后 `user-settings.json`(5382 B) / `user-workspaces.json`(2223 B) / `user-stats.json`(31863 B)
三个文件 sha256 与备份逐字一致；`shortcuts` 段不存在、`settingsCurrentTab=appearance`、`lastRoute=navigator`，
与验收前完全相同；无残留 app 进程、无孤儿 `msedgewebview2`。

> ⚠️ **键盘路径未在沙箱内取证**：沙箱桌面没有键盘输入能力，「录制快捷键 / 编辑弹窗」这条路无法在沙箱里驱动。
> 本轮只取证了**纯鼠标等价**的写路径（右键菜单「取消绑定」）。

### 2.4 构建产物

`npm run tauri:build` 成功（Rust release 5m42s）：

- `src-tauri\target\release\alpha-file-manager.exe`（09:14:32）
- `release\local-builds\Alpha File Manager_3.9.2_x64-setup.exe`（44,870,444 B，09:15:11）
- 前端产物 `dist\assets\*.js` 于 09:08:40 重建，其中已包含新标识 `sigma.shortcut.`
  ⇒ **新 exe 确实含批次 2 代码**（这是必须单独验的一条，见 §5.6）。

### 2.5 工作区

```
 M docs/handoff-2026-11-01-relay-four-tasks.md      ← 用户自己的改动，别动
 M docs/winget-cla-signing-guide.md                 ← 用户自己的改动，别动
 M src/modules/extensions/builtin-commands/index.ts ← 批次 1
 M src/modules/extensions/components/command-palette.vue ← 批次 1
 M src/modules/settings/ui/categories/shortcuts/shortcuts.vue ← 批次 2
 M src/stores/runtime/shortcuts.ts                 ← 批次 2
 M src/types/user-settings.ts                       ← 批次 2
?? docs/analysis-2026-11-02-files-benchmark.md
?? docs/plan-2026-11-02-action-registry-and-incremental-index.md
?? docs/handoff-2026-11-03-phase5-batch1-action-registry.md
?? docs/handoff-2026-11-03-phase5-batch2-shortcut-action-ids.md
?? src/core/
?? src/stores/runtime/__tests__/shortcuts-action-binding.test.ts
?? src/stores/runtime/__tests__/shortcuts-legacy-fallback.test.ts
```

---

## §3 批次 2 的设计与语义（下一批必须遵守）

### 3.1 兼容语义（核心，动了就要重读）

1. **读**：新格式优先；命中新格式时，**整条**快捷键作废旧格式（**不是**按槽位混用）。
2. **写**：落新格式键 `sigma.shortcut.<id>`，并**同步删除同一条**旧格式键。
3. **重置**：同时删新旧两种键。
4. **迁移是内存映射，不整表重写用户文件** —— 只有用户真正改动绑定时才写新格式键，
   `user-settings.json` 不会因为升级被动改写。
5. **回落链路三级**：注册表未命中该动作 → 迁移抛错 → 原始双查（action-id 键 → legacy 键 → 默认键位）。
   （第 2 级由 `shortcuts-legacy-fallback.test.ts` 用 `vi.mock` 让迁移抛错来覆盖。）
6. `shortcutUserAlternateChordSlots`（备用键位）本批次**不迁移**，仍以 `ShortcutId` 为键 —— 遗留项。
7. 命名空间映射而非一刀切：快捷键动作 id = `sigma.shortcut.<ShortcutId>`，
   与命令 id 同处一个注册表命名空间但不冲突。

### 3.2 批次 1 的语义（继续有效，批次 3 不得推翻）

1. **去重规则 = 先注册者赢**。
2. **`showInPalette !== true` 的内置命令不进面板**。
3. **标题来源**：`titleKey` + `t(titleKey)`，无 `titleKey` 才用命令自带 `title`。
   **注册表与适配器都不认识 i18n**（内核不依赖 vue-i18n，这是刻意的）。
4. **只有 `enabled` 的条目出现在面板**。批次 3 做右键菜单需要「显示但置灰」时，
   改 `palette-adapter.ts` 的过滤条件，**不要改 `registry.ts` 的 `resolve` 语义**。
5. **谓词抛异常 → 视为不可用**。
6. **上下文只取一次快照**。
7. **注册表不是全局单例**：每次从响应式快照重建。

### 3.3 三层职责（沿用批次 1）

```
types.ts              纯类型，无依赖
registry.ts           纯数据容器
palette-adapter.ts    适配器（外部命令清单 → ActionDefinition → 面板条目）
shortcut-actions.ts   快捷键动作 id 的构造/解析（批次 2 新增）
shortcut-bindings.ts  绑定值判别与查表（批次 2 新增）
```

内核不依赖 vue-i18n、不依赖 UI 组件；`src/core/actions/index.ts` 是桶文件，所有引用走同一入口。

---

## §4 下阶段任务（批次 3 起，按计划文档）

### 批次 3：工具栏 + 右键菜单 + 扩展命令 API 接注册表

- `src/components/dir-entry-interactive/dir-entry-context-menu.vue`、`src/modules/window-toolbar/`
  各自硬编码命令清单，改为注册表投影。
- **扩展 API 是对外契约**：`createCommands()` 的旧签名必须保留，新增注册路径是**可选**的。
- 单测判据：投影结果与注册表一致；扩展注册后命令面板能搜到。
- **批次 2 给批次 3 的现成抓手**：快捷键侧已经用 `sigma.shortcut.<id>`，
  菜单/工具栏项应复用同一命名空间，不要再造第三套 id 体系。
- ⚠️ **改动范围待用户决策**（见 §6 第 1 条）：先只接文件条目右键菜单，还是连目录树、驱动器卡一起接。

### 批次 4：任务 F 增量索引（Rust，风险最高，独立验收）

- 复用 `src-tauri/src/dir_watcher.rs` 接 tantivy writer，`create` / `update` / `delete` 三类事件增量写；
  每块盘一个 watcher 通道；watcher 不可用时**回落全量扫描**。
- **验收口径不可改**：同一探针 `task3-search-accept-20261102-v2.ps1`、同一埋点
  `src-tauri/src/global_search/scan.rs:346`，**重跑 4 轮全盘扫描**对比基线
  `728,966 / 748,192 / 730,123 / 750,079 ms`；判据 ① 首轮 ≤ 6.3 min ② 稳态轮显著低于 12.2 min
  ③ 增量后查询结果与全量重建一致。
- **红线**：`cargo test --lib` 链接到的代码**不得触碰** `tauri::AppHandle` 或 `tauri::async_runtime::*`
  （会把 tao 拖进测试二进制 → `STATUS_ENTRYPOINT_NOT_FOUND` 0xC0000139，一个测试都跑不了）。
  绕法：给业务模块加 `*_blocking` 同步内核入口。

### 批次 5 / 6 / 7

- 批次 5：自然语言算子 / 自然排序 / 分组与列宽。
- 批次 6：列视图 / 工具栏自定义 / 键位编辑器。
- 批次 7：Git 做成**扩展**（走扩展市场，参照 `extensions/kizemo.focus-sync/`），
  **硬顺序在批次 3 之后** —— 扩展命令必须走注册表，否则又变成第四套入口。

### 明确不做（避免摊薄）

云盘独立卷、格式化 / 批量清空回收站、创建 ADS、引入 WinUI / C#、砍掉全盘索引。

---

## §5 本轮新踩的坑（务必带进下一个会话）

### 5.1 `Add-Type` 的 C# 里漏声明 P/Invoke ⇒ 编译期就挂，验收零输出

- 症状：`Add-Type : ...(183) : 当前上下文中不存在名称"CreateProcess"`，脚本**第一行就死**。
- 真凶：脚本的 C# 辅助类里用了 `CreateProcess` / `CloseHandle`，但**没有写对应的 `[DllImport]`**。
  `Add-Type` 是真编译，不是动态绑定。
- 处置：抄 `sandbox-c1-accept.ps1` 的声明块，不要凭记忆写：
  ```csharp
  [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
  public static extern bool CreateProcess(string app, string cmd, IntPtr pa, IntPtr ta, bool inherit, uint flags, IntPtr env, string cwd, ref STARTUPINFO si, out PROCINFO pi);
  [DllImport("kernel32.dll")]
  public static extern bool CloseHandle(IntPtr h);
  ```
- ⇒ **写完 `.ps1` 先单独跑一次编译**，别直接带真实数据跑。

### 5.2 `$pid` 是 PowerShell **只读自动变量**，不能当函数参数名

- 症状：`System.Management.Automation.SessionStateUnauthorizedAccessException :: Cannot overwrite variable pid
  because it is read-only or constant.`
- 真凶：`function Wait-MainWindow($desktop, [int]$pid, [int]$seconds)` —— 参数绑定试图写只读的 `$pid`。
- 处置：改名 `$procId`。**凡是脚本里出现 `$pid` 当变量用都要改**（`$pidOut` 这种不受影响）。
- 附注：这类「跑起来就死」的异常发生在**播种之前**，真实数据没被动过 ——
  但只有因为脚本的 `finally` 里有「杀进程 → restore」兜底才敢这么试。

### 5.3 应用退出时会**回写 `user-stats.json`** ⇒ 备份/还原清单必须包含它

- AGENTS.md 早就记着这条，但本轮 fixture 一开始只备份 `user-settings.json` + `user-workspaces.json`。
  ⇒ 应用退出瞬间把内存里的统计状态写回真实 `user-stats.json`，**用户的统计数据被验收动作改掉**。
- 处置：`.superpowers\sdd\b2-fixture.cjs` 的 `backup()` / `restore()` 的文件清单已改为
  `[SETTINGS, WORKSPACES, STATS]` 三件。**下轮起任何写真实数据的验收都要先列全这三个文件。**

### 5.4 读「正在被写」的日志用 `Get-Content`，不要用 `[System.IO.File]::ReadAllText`

- 症状：`ReadAllText` 抛「文件正由另一进程使用」。
- 真凶：脚本用 `StreamWriter` + `AutoFlush` 持有日志文件。
- 处置：PowerShell 的 `Get-Content` 默认用 `FileShare.ReadWrite`，能读；
  .NET 的 `File.ReadAllText` 不共享 ⇒ 抛异常。轮询日志一律用 `Get-Content`。

### 5.5 全量单测的失败必须**在 HEAD 基线上复跑**才能定性

- 本轮全量 274 个文件里有 4 个文件 20 条失败，**光看这不能断言「没回归」**。
- 处置：在 HEAD（`ceff7e38`）上 `git worktree add --detach $env:TEMP\afm-b2-baseline HEAD`，
  把主仓 `node_modules` **做 junction 挂进去**（`New-Item -ItemType Junction`），
  在 worktree 里跑同样 4 个文件 ⇒ 失败集逐条相同 ⇒ 零回归。
- ⇒ **比 stash 更稳**：stash 会在构建/测试同时跑时把工作区搅乱，worktree 不碰当前工作区。
  验收完 `git worktree remove --force` 清理。

### 5.6 「新 exe 里到底有没有我这次的改动」要单独验

- 本轮新旧 exe 字节数**完全相同**（80,323,584 B），只看大小无法判断。
- 处置：验 `dist/assets/*.js` 里含不含新标识（本轮 `sigma.shortcut.` 命中 `workspaces-CC6Tn086.js`），
  且 `dist` 的 mtime 必须**晚于**本轮构建启动时间。
- ⇒ `tauri build` 的前端产物在 Rust 编译之前就生成，`dist` 是判断「这版 exe 是不是我要的那版」的硬证据。

### 5.7 沙箱桌面内**没有键盘输入** ⇒ 只能做鼠标等价断言

- `SendInput` 只作用于**输入桌面**，沙箱是 `CreateDesktop` 建的独立桌面，拿不到输入。
- Chromium 的 `Ctrl` / `Shift` 取自 `GetKeyState`，用 `PostMessage` 伪造 `WM_KEYDOWN` **不生效**。
- ⇒ 批次 2 放弃了「录制快捷键 / 编辑弹窗」这条键盘路径，改用**右键菜单「取消绑定」**这条纯鼠标写路径。
- 结论写法要说清：**键盘路径未在沙箱内取证**，不要把鼠标路径的结论扩大成「键盘路径也通过」。

### 5.8 沙箱验收里几个已经跑通的定位手法（沿用，别重写）

- 快捷键显示串是 UIA 的 `ControlType.Text` 元素，锚点用 `Ctrl+Alt+Shift+K` / `Ctrl+Alt+Shift+J`；
  **未分配显示 `—`（U+2014），语言无关**，适合做「键被解除」的判据。
- 右键菜单项用 `Find-MenuItemLike` 按**子串**匹配（中文名「取消绑定」），
  `InvokePattern` 在 Radix 菜单项上可用（**Radix 菜单的 UIA 枚举有竞态，查一次可能 `count=0`，要轮询重试**）。
- 鼠标点击投给 **`Chrome_RenderWidgetHostHWND`**（不是 `Chrome_WidgetWin_0`），
  `lParam` 用该窗口**客户区坐标**（屏幕坐标减去它的原点）。
- 应用启动后会弹「新功能」更新日志模态框挡住内容区 ⇒ 关闭按钮名「关闭」（U+5173 U+95ED）。
- DPI 144（150%）⇒ 脚本开头 `SetProcessDPIAware()`；主窗口不能用 `Process.MainWindowHandle`
  （会落到 15×15 托盘窗），要 `EnumWindows` + `GetWindowThreadProcessId` 过滤，
  取 class 恰为 `Tauri Window`、可见、面积最大者。
- 屏幕 BitBlt / `Graphics.CopyFromScreen` 三档尺寸全失败 ⇒ 用 `PrintWindow(hwnd, hdc, 2)` + UIA。

### 5.9 沿用批次 1 的既有结论（不重复踩）

- `tauri build` 前必须杀干净 `alpha-file-manager`（`BUILD_EXIT=1` 先 grep `failed to remove file`）。
- 构建脚本 `.cmd` 内部 `> logfile 2>&1` 重定向；**运行期间绝对不能编辑**（cmd 按字节偏移逐行读）。
- 后台任务**不能挂 stdout 管道**（`Tee-Object` 会让进程树跟着死）。
- 启动与轮询必须**分成两个进程**；构建进程不能和 background 任务同生共死。
- `.cmd` / `.ps1` 必须**纯 ASCII**（cmd.exe / PowerShell 5.1 按 ANSI 读无 BOM 文件）。
- 绝不用 `Get-Content -Raw` / `Set-Content` 改带中文注释的源文件。

---

## §6 待用户决策 / 需用户本人操作

1. **批次 3 的右键菜单改动范围**：是「先只接文件条目右键菜单」还是「连同目录树、驱动器卡一起接」。
   建议先只接文件条目（`dir-entry-context-menu.vue`），其余按批次推进 —— 待用户确认。
2. **winget PR（任务 E）** 仍等社区版主 `Moderator-Approved`，Agent 代劳不了；用户 CLA 已签。
3. **focus-sync（任务 G）** 需用户确认同步状态，未确认前不动 `user-extensions.json`。
4. **若要补「物理键盘 `Ctrl+Shift+P`」真机证据**：需用户先解锁/恢复交互会话
   （输入桌面 `OpenInputDesktop` 返回 err=5）。解锁后重跑
   `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\accept-palette-v2-20261103.ps1` 即可；
   **这不阻塞批次 3 开工**。

---

## §7 可复用脚本索引（全部在 `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\`）

| 脚本 | 用途 |
|---|---|
| `b5-build4.cmd` | 打包用。`vcvars64.bat` + 注入 rustup toolchain PATH + `npm run tauri:build`，**内部重定向**到 `b5-build4.log`，末尾写 `[build] BUILD_EXIT=`。**纯 ASCII** |
| `accept-b2-shortcuts.ps1` | **批次 2 沙箱验收（当前唯一可用的一版）**：隔离桌面起 release exe → 关更新日志模态框 → UIA 读快捷键显示串（读回旧/新两种格式）→ 右键菜单「取消绑定」写路径 → 落盘核对 → 重启核对 → 还原真实数据。A/E1/E2 三条数据安全断言内建。**纯 ASCII**。日志 `accept-b2-shortcuts.log`，截图 `accept-b2-shots/*.png` |
| `b2-fixture.cjs` | 批次 2 夹具：`backup` / `seed` / `read` / `restore` / `verify`。node + utf8 安全，输出全 ASCII 转义。备份清单 = settings + workspaces + **stats** 三件 |
| `b2-eslint-baseline.ps1` | eslint 全量基线逐字对照：stash 前后计数守卫 + 同一条 `npx eslint .` 跑两次。**纯 ASCII** |
| `sandbox-c1-accept.ps1` | **沙箱桌面模板**：`CreateDesktop` + `EnumDesktopWindows` + `STARTUPINFO.lpDesktop`，含正确的 kernel32 P/Invoke 声明块。写新的沙箱脚本从这里抄 |
| `accept-palette-v2-20261103.ps1` | 批次 1 命令面板真机验收（当前唯一可用的一版）。日志 `accept-palette-v2-20261103.log` |
| `diag-uia-20261103.ps1` | UIA 树体检。**排查「UIA 取不到东西」时先跑它** |
| `diag-input-20261103.ps1` | 输入桌面体检。**判定「按键注入能不能用」的唯一依据** |
| `task3-search-accept-20261102-v2.ps1` | 批次 4 的全盘搜索计时探针（判据 4 轮） |

运行姿势（**不要挂 stdout 管道**，会让后台任务跟着管道一起死）：

```powershell
# 关键：启动和轮询必须分成两个进程。前台调用只负责 Start-Process，十几秒内返回；
# 轮询放到独立的 background 任务里，且那个任务不能是构建进程的父进程。
Start-Process -FilePath 'cmd.exe' -ArgumentList '/c','F:\...\sigma-file-manager\.superpowers\sdd\b5-build4.cmd' -WindowStyle Hidden
```

---

## §8 新会话接力用的一句话 prompt

把下面**整段**（从「请先按顺序」到最后一个句号）复制给新会话，无需再附加任何背景说明：

> 请先按顺序完整阅读这 5 份文档（完整路径如下），据此掌握 Alpha File Manager 项目的背景、目标、已完成进度与下阶段计划，读完先向我简述你理解的当前现状与准备先做哪一步，不要复述文档内容，也不要重做已完成的事：
> 1) `F:\soft\00selfmade\filemanager\AGENTS.md` —— 项目硬约束总纲，优先级最高（注意：它作为项目指令注入时会被截断到 32768 字节，需要完整内容时直接读源文件）；
> 2) `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-03-phase5-batch2-shortcut-action-ids.md` —— 本阶段交接入口，含批次 1–2 的当前状态、语义决策、真机验收结果与本轮新踩的坑；
> 3) `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\plan-2026-11-02-action-registry-and-incremental-index.md` —— 阶段五计划：批次 1–7 的顺序、验收判据、风险与明确不做的事；
> 4) `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\analysis-2026-11-02-files-benchmark.md` —— 与 files-community/Files 的对标分析，是本阶段的需求来源；
> 5) `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-02-phase4-winget-and-search-scan.md` —— 上一阶段交接原文，含 winget 发版与全盘搜索扫描并行化的历史踩坑记录。
> 本阶段从**批次 3**（工具栏 + 右键菜单 + 扩展命令 API 接注册表）开始；开工前先核对交接文档 §2「当前状态」与你实际看到的工作区是否一致，不一致先报告再动手。
