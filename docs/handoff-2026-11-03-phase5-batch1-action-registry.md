# 交接文档 · 2026-11-03 · 阶段五批次 1：动作注册表内核 + 命令面板改读注册表

> **本文是阶段五批次 1 的交接入口。** 批次 2 及以后仍以
> `docs/plan-2026-11-02-action-registry-and-incremental-index.md` 为准。
> 面向用户的文字一律简体中文；代码标识符、路径、命令、配置键名保持原样。
>
> ⚠️ **批次 1 与批次 2 均已完成。** 新会话请改读
> `docs/handoff-2026-11-03-phase5-batch2-shortcut-action-ids.md`（当前交接入口），
> 从**批次 3** 开始；**本文 §8 的接力 prompt 已过期**。

---

## §0 必读文档（完整路径，按顺序）

| # | 文档 | 作用 |
|---|---|---|
| 1 | `F:\soft\00selfmade\filemanager\AGENTS.md` | 项目硬约束总纲，**优先级最高**（注入时被截断到 32 KB，需要时直接读源文件） |
| 2 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\plan-2026-11-02-action-registry-and-incremental-index.md` | 阶段五计划：批次 1–7 的顺序、判据、风险、明确不做的事 |
| 3 | 本文 `docs\handoff-2026-11-03-phase5-batch1-action-registry.md` | 批次 1 的交付内容、语义决策、真机验收结果 |
| 4 | `...\docs\analysis-2026-11-02-files-benchmark.md` | 与 [files-community/Files](https://github.com/files-community/Files) 的对标分析，阶段五的需求来源 |
| 5 | `...\docs\handoff-2026-11-02-phase4-winget-and-search-scan.md` | 上一阶段交接（winget 3.9.2 + 搜索扫描并行化），含大量历史踩坑 |

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

### 2.1 代码：批次 1 已落盘

新增目录 `src/core/`（**未跟踪**，全部为新文件）：

| 文件 | 作用 |
|---|---|
| `src\core\actions\types.ts` | `ActionContext` / `ActionWhen` / `ActionAvailability` / `ActionDefinition` / `ResolvedAction` |
| `src\core\actions\registry.ts` | `createActionRegistry()`：注册、查、去重、按上下文求 `availability` |
| `src\core\actions\predicates.ts` | `getActionContext()` + 现成谓词：`alwaysAvailable` / `whenNoDialogOpen` / `whenNotEditingText` / `whenRouteIs` / `combineWhen` |
| `src\core\actions\palette-adapter.ts` | 第一个适配器：把扩展命令 + 内置命令投影成命令面板条目 |
| `src\core\actions\index.ts` | 桶文件（只做 re-export） |
| `src\core\actions\__tests__\*.test.ts` | 3 个测试文件，31 条用例 |

改动的既有文件（2 个功能文件 + 2 个文档是用户自己的未提交改动，**别动**）：

- `src\modules\extensions\components\command-palette.vue`
  —— `allCommands` 改为走注册表；内置标题表 `builtinCommandTitleKeys` **移出**本组件；
  历史上 `command-palette.vue` 里的调试打点已全部清除（13,841 B，`FORK-TEMP` / `B1DBG` 命中数 0）。
- `src\modules\extensions\builtin-commands\index.ts`
  —— 新增导出 `BUILTIN_COMMAND_TITLE_KEYS`（三个内置命令的 i18n 标题键）。

### 2.2 单测门禁

```
npx vitest run src/core/actions
→ Test Files  3 passed (3)
  Tests      31 passed (31)
  exit 0
```

覆盖点：注册 / 重复 id 返回 `false` / 空 id 抛错 / 谓词求值 / `when` 抛异常降级 / 适配器的去重与
`showInPalette` 过滤 / `titleKey` 透传 / `availability !== 'enabled'` 被过滤。

### 2.3 真机界面验收

**结论：通过。命令面板在 3.9.2 打包产物上真实可用，命令列表已由批次 1 的新注册表供给。**
2026-11-03 01:0x 实跑，完整日志：`.superpowers\sdd\accept-palette-v2-20261103.log`。

- **触发方式**：点击窗口工具栏里的命令面板按钮
  （`src\modules\extensions\components\command-palette-toolbar-button.vue`，UIA 里
  `ControlType.Button`、空名、rect `85,15 43x42`、`InvokePattern` 可用）。
  它的 `@click` 调 `shortcutsStore.executeShortcut('toggleCommandPalette')`，
  与键盘快捷键**走同一条 handler 链路**。
- **面板确实渲染**（UIA 树，`FromHandle` 入口）：
  `ControlType.Window | '命令面板'`（rect `1536,811 768x466`）、
  `ControlType.Text | '命令面板'`、
  `ControlType.Text | '输入文字筛选，然后从列表中选择命令。'`、
  `ControlType.ComboBox | '输入命令或搜索...'`、`ControlType.List`。
- **命令列表有 8 条 `ListItem`**，扩展命令与内置命令都在：

  ```
  Focus Sync: Sync Now
  Focus Sync: Enable / Disable
  Focus Sync: Toggle Auto-Confirm
  Focus Sync: Re-show Edge tip
  Clipboard History: Clipboard History Manager Alt+Shift+V
  Sigma: 打开设置
  Sigma: 打开扩展
  Sigma: 重新加载窗口
  ```

  ⇒ **批次 1 的 `buildPaletteActionRegistry()` + `titleKey` 透排在真机上成立**：
  扩展命令在前、内置 `Sigma:` 命令在后，与「先扩展后内置、先注册者赢」一致。
- **像素侧佐证**：`PrintWindow` 前后两帧 `DIFF=7,977,542`（模态遮罩改写全窗），
  B 帧 PNG 5,001,471 B vs A 帧 4,325,039 B。
- **旁证**：面板打开后 UIA 节点数从 406 掉到 60 —— Radix Dialog 把背景设为 `aria-hidden`，
  这是模态正常行为。

**唯一没验成的一项（环境阻塞，不是代码缺陷）**：物理键盘 `Ctrl+Shift+P` 的真机触发。
本机会话的**输入桌面不可用**：`OpenInputDesktop()` 返回 `NULL` + `ERROR_ACCESS_DENIED(5)`，
`SendInput` 返回 0 + err=5，`GetForegroundWindow()` 返回 0 ⇒ 本会话内任何进程都注入不了按键
（与本次改动无关；`EnumWindows` / `PrintWindow` / UIA 此时全部正常，所以很容易误判成「产品坏了」）。
解锁或恢复交互会话后重跑 `.superpowers\sdd\accept-palette-v2-20261103.ps1` 即可补齐。
代码路径侧证据：`src\stores\runtime\__tests__\shortcuts.test.ts` **30 passed (30)**。

### 2.4 构建产物

`npm run tauri:build` 成功后，产物为：

- `src-tauri\target\release\alpha-file-manager.exe`
- `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.9.2_x64-setup.exe`
- 末步复制到 `release\`（若 `release\` 里旧包被别的进程锁住，会被复制步骤 exit 1 中断 —— **构建本身已成功**，
  新包用 `node -e "fs.copyFileSync(a,b)"` 单独补进去即可）。

### 2.5 工作区

```
 M docs/handoff-2026-11-01-relay-four-tasks.md      ← 用户自己的改动，别动
 M docs/winget-cla-signing-guide.md                 ← 用户自己的改动，别动
 M src/modules/extensions/builtin-commands/index.ts
 M src/modules/extensions/components/command-palette.vue
?? docs/analysis-2026-11-02-files-benchmark.md
?? docs/plan-2026-11-02-action-registry-and-incremental-index.md
?? docs/handoff-2026-11-03-phase5-batch1-action-registry.md
?? src/core/
```

`git diff --stat`（不含未跟踪文件）：4 files changed, 72 insertions(+), 51 deletions(-)。

---

## §3 批次 1 的设计与语义（下一批必须遵守）

### 3.1 三层职责

```
types.ts          纯类型，无依赖
registry.ts       纯数据容器：Map<id, ActionDefinition> + resolve(context)
palette-adapter.ts 把「外部命令清单」翻译成 ActionDefinition，再投影成面板条目
```

注册表**不是全局单例**：`buildPaletteActionRegistry()` 每次从 Pinia 的响应式快照重建一份。
这样避免了「全局注册表 + 响应式快照」两份状态互相打架，也让单测不需要 mock 任何 store。

### 3.2 与旧实现逐条对齐的语义（改动前请对照）

1. **去重规则 = 先注册者赢**。扩展/运行时命令先注册，内置命令后注册且 `register` 对重复 id 返回 `false`
   ⇒ 效果等价于旧代码里的 `existingIds` 集合。
2. **`showInPalette !== true` 的内置命令不进面板**（旧代码是 `if (!builtin.showInPalette) continue`）。
3. **标题来源**：`titleKey` 由 `BUILTIN_COMMAND_TITLE_KEYS` 提供，组件里用 `t(titleKey)`；
   没有 `titleKey` 就用命令自带 `title`。旧实现的 `builtinCommandTitleKeys` 是组件内的局部常量，
   现在移到 `builtin-commands/index.ts` 导出，**注册表与适配器都不认识 i18n**（这是刻意的：内核不依赖 vue-i18n）。
4. **只有 `enabled` 的条目出现在面板**。`disabled` 与 `hidden` 目前都被过滤掉 ——
   批次 3 做右键菜单时会需要「显示但置灰」，届时改 `palette-adapter.ts` 的过滤条件即可，
   **不要改 `registry.ts` 的 `resolve` 语义**。
5. **谓词抛异常 → 视为不可用**（`catch { isAvailable = false }`），且由 `hideWhenUnavailable` 决定
   `disabled` 还是 `hidden`。这一条是为了「某个扩展的 `when` 有 bug 时，面板整体不崩」。
6. **上下文只取一次快照**：`getActionContext(activeRouteName)` 读的是
   `src/utils/dom-interaction-state.ts` 的 `isInputFieldActive()` / `isDialogOpened()`。
   面板里通过 `resolveAllCommands()` 函数（而非内联到 `computed`）调用，保证每次求值拿到当帧状态。

### 3.3 为什么 `actions/index.ts` 是桶文件

`command-palette.vue` 一行 import 就能拿到类型 + 函数；后续批次（快捷键 store、工具栏、右键菜单）
也走同一个入口，避免出现 `@/core/actions/registry` 与 `@/core/actions/palette-adapter` 两套引用风格。

---

## §4 下阶段任务（批次 2 起，按计划文档）

### 批次 2：快捷键 store 绑 id + 快捷键设置页可编辑

- `src/stores/runtime/shortcuts.ts` 现在把「快捷键」绑在动作名字符串上（如 `toggleCommandPalette`），
  要改成绑注册表 id；`src/modules/settings/ui/categories/shortcuts/` 现在**只读列举**，要变成可编辑。
- **硬要求**：必须**保留旧格式兼容读取**，迁移失败时回落旧逻辑（用户既有绑定不能失效）。
- 单测判据：绑定解析、冲突检测、改键后持久化往返。

### 批次 3：工具栏 + 右键菜单 + 扩展命令 API 接注册表

- `src/components/dir-entry-interactive/dir-entry-context-menu.vue`、`src/modules/window-toolbar/` 各自硬编码命令清单，
  改为注册表投影。
- **扩展 API 是对外契约**：`createCommands()` 的旧签名必须保留，新增注册路径是**可选**的。
- 单测判据：投影结果与注册表一致；扩展注册后命令面板能搜到。

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

### 5.1 `tauri build` 会「静默失败」：旧 exe 被运行中的进程占用 + `.cmd` 无内部重定向

- 症状：后台跑了一刻钟，`target\release\alpha-file-manager.exe` 时间戳**纹丝不动**，日志文件不存在。
- 真凶两步：① 上一版应用正从 `src-tauri\target\release\alpha-file-manager.exe` 运行，
  **链接器覆盖不了正在执行的 exe**；② `.cmd` 没有内部重定向，`cargo` / npm 的 stderr 被 Hidden 窗口丢弃。
- 处置：**重建前先 `Stop-Process` 掉 `alpha-file-manager` 并确认进程数为 0**；
  构建脚本一律内部 `> "%LOG%" 2>&1`（见 §7 的 `b5-build3.cmd`）。
- 判据只有一条：exe 时间戳离开旧值 + 日志尾部出现 `[build] BUILD_EXIT=0`。
  **不要用「进程消失了」当成功判据**。
- **补充（2026-11-03 00:25 实测，第二条新坑）**：杀掉进程并确认 `PROCS=0` 之后启动构建，跑到链接阶段时
  又出现了一个**新的** `alpha-file-manager` 进程（PID 42948，`START=00:25:58`，路径正是
  `src-tauri\target\release\alpha-file-manager.exe`），于是 `tauri build` 以
  `error: failed to remove file '...\alpha-file-manager.exe'` / `Caused by: 拒绝访问。 (os error 5)` 失败。
  ⇒ **`BUILD_EXIT=1` 时先 `grep` 日志里有没有 `failed to remove file`**：这是「exe 被占用」而非编译错误，
  此时 cargo 产物已经全部生成，只需**杀进程 + 原地重跑**（增量重跑只需重新链接 + 打包，几分钟）。
  ⇒ 重跑必须用**新的 `.cmd` 文件名**（`b5-build3.cmd`），不要改正在跑的那一个（§5.5 有 `.cmd` 逐行读取的坑）。
  ⇒ 构建期间**不要启动应用**；验收要等 `BUILD_EXIT=0` 之后再起。

### 5.2 命令面板的可见性取证：**UIA 入口选错会得出「面板打不开」的假结论（2026-11-03 已翻案）**

- **旧结论作废**：先前记的「面板文案从来不在 UIA 树里」「`PrintWindow` 不渲染 overlay 所以拍不到面板」，
  **两条都是错的**，真实原因是 **UIA 入口选错了**。本轮已用 UIA 直接读到面板标题与全部 8 条命令。
- **错误入口**：`RootElement.FindAll(TreeScope::Children, ProcessIdProperty == pid)` 在本机
  **一条都匹配不到**（哪怕 app 进程确实有可见的 `Tauri Window` 顶层窗口），
  于是 `Descendants` 恒为 0，看起来像「webview 根本没暴露可访问性」。
- **正确入口**：`AutomationElement.FromHandle($主窗口句柄).FindAll(TreeScope::Descendants, TrueCondition)`
  ⇒ 一次拿到 **406 个节点**，工具栏按钮、目录树、面包屑、面板列表全部可见。
  ⇒ **凡是从 UIA 取不到东西，先怀疑入口，不是先怀疑产品。**
- `RootElement.FindAll(Descendants, ControlType==Button)` 这种「从根搜后代」**能用**
  （本轮就是靠它关掉更新日志模态框），但慢且容易误伤别的进程
  ⇒ 关按钮必须加 `Current.ProcessId -eq $目标pid`。
- `PrintWindow(hwnd, hdc, 2)` **能拍到 Radix 面板**（面板在 webview 的 DOM 里，不是 overlay 层）：
  面板打开时整窗被模态遮罩改写，`DIFF` 近 800 万像素。
- 屏幕 BitBlt / `CopyFromScreen` 仍然整条不可用（见 §5.9），**别再在这条路上试**。
- 验收脚本仍要带**阳性对照**：先发一个裸 `Tab`，用 `AutomationElement.FocusedElement`
  比对焦点描述是否变化（`POSCTRL_FOCUS before=... after=... moved=True/False`）。
  阳性对照不通过 ⇒ 结论只能是「注入链路没进 webview」，不能写成「面板坏了」。

### 5.3 像素 diff 必须下沉到 C#

3 个 6M 像素的双层 PowerShell 循环会跑几分钟。本轮在 `Add-Type` 的 C# 里写静态方法
`PW.Diff(byte[], byte[], stride, w, h, inset)`，PowerShell 只传 `byte[]`。
**注意**：C# 类型定义里不能直接用 `System.Drawing.Bitmap`（报 `CS1069`），
`Add-Type -AssemblyName System.Drawing` 只让 PowerShell 侧可用。

### 5.4 截图区域要用「窗口 ∩ 屏幕」

`SetWindowPos` 到 TOPMOST 后窗口可能仍有负原点（本机窗口 rect 起点 `-11,-11`）。
直接用 `GetWindowRect` 的宽高建位图会越界，必须取交集：
`capX = max(0, left)`、`capW = min(screenW, right) - capX`。

### 5.5 其它（沿用既有结论，不重复踩）

- 本机 DPI 144（150% 缩放），**验收脚本开头必须 `SetProcessDPIAware()`**，
  生效判据是 `GetSystemMetrics(0)` 从 `1707` 变成 `2560`。
- 主窗口不能用 `Process.MainWindowHandle`（会落到 15×15 托盘窗），
  要 `EnumWindows` + `GetWindowThreadProcessId` 过滤，取 class 恰为 `Tauri Window`、可见、面积最大者。
- 应用启动后可能弹「新功能」更新日志模态框挡住文件列表，关闭按钮名「关闭」（U+5173 U+95ED）。

### 5.6 后台任务会连带杀掉构建进程树（2026-11-03 00:31 实测，第二次踩同一个坑）

- 现象：`.cmd` 内部重定向的日志停在某个时间点不再增长，**既没有 `[build] BUILD_EXIT=` 也没有任何
  `error:` 行**，看起来像「编译卡死」。实际上进程已经被从外部杀死。
- 真凶：把「`Start-Process` 起构建」和「长轮询日志」放在**同一个 background 任务**里，
  该 background 任务被 runtime 判定 lost / 终止时，它创建的 `cmd.exe` 子进程树一起消失。
  ⇒ **构建进程不能和任何 background 任务同生共死。**
- 正确姿势（两条一起做）：
  1. 前台调用只做 `Start-Process -WindowStyle Hidden`，**十几秒内就返回**，不要在同一调用里 sleep 轮询；
  2. 轮询放到**另一个** background 任务里，且那个任务只读文件、不持有构建进程。
- 判据仍然只有一条：日志尾部出现 `[build] BUILD_EXIT=0`。
- 附带结论：`Get-Process -Name cargo,rustc` 在**编译阶段**会显示 `CARGO=2`，
  但**链接阶段**可能已经是 0（cargo 父进程在等 `rustc`/`link.exe`），
  所以「CARGO=0」**不能**当构建结束或失败的判据，只能看日志。

### 5.7 命令面板的快捷键不是系统级热键（2026-11-03 核对源码）

- `src/stores/runtime/shortcuts.ts` 里 `scope` 的三个取值是 **`'global' | 'navigator' | 'settings'`**，
  指的是**前端路由 scope**（任何路由 / 仅导航器 / 仅设置页），**不是** Windows 系统级全局热键。
  ⇒ 命令面板由**前端 keydown 处理**，把按键注入到 webview 理论上就能触发。
- `toggleCommandPalette` 的默认键位在 `shortcuts.ts:157` 附近：`ctrl + shift + p`，
  `conditions: {}`（无 `dialogIsOpened` 限制，任何时候都能开）。
- 用户真实设置 `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-settings.json`
  里的 `shortcuts` 段**为空**（`SHORTCUT_KEYS=0`）⇒ 走默认键位，
  **验收直接用 `Ctrl+Shift+P` 即可**，不必改用户设置。
- 组件侧 `command-palette.vue` 通过 `shortcutsStore.registerHandler('toggleCommandPalette', ...)`
  注册回调，并 `defineExpose({ open, close, toggle, isOpen })` ⇒ 组件级可测点很干净
  （但仓库里 `*.spec.ts` 数量为 0，没有组件挂载测试基础设施，临时补要动依赖，**不建议**）。

### 5.8 验收脚本本轮的加固（`accept-palette-clean-20261103.ps1`）

1. **点击取焦点**：`SetForegroundWindow` 只保证**窗口**是前台，不保证**页面**有键盘焦点；
   注入的按键因此可能根本进不了 webview。新增 `Focus-Webview`：置顶后用
   `SetCursorPos` + `mouse_event(0x0002/0x0004)` 在客户区中心点一下，把焦点交给页面。
2. **焦点进程号**：`AutomationElement.FocusedElement.Current.ProcessId` 与目标 pid 比对，
   输出 `inApp=True/False`。这比「Tab 之后焦点描述有没有变」更直接 ——
   后者分不清「焦点在宿主里」和「焦点在应用里」。
3. **B 帧连拍**：面板有打开动画，连拍两帧取后一帧，避免拍到未绘制的中间态。
4. `GetWindowRect` 的 `out` 参数在 PowerShell 里要写成
   `$r = New-Object PW+RECT` + `[PW]::GetWindowRect($h, [ref]$r)`。

### 5.9 输入桌面被锁 ⇒ 按键注入整条失效（但界面取证照旧能做）

- **症状**：`SendInput` 返回 0 + `GetLastError()=5`（`ERROR_ACCESS_DENIED`），
  `GetForegroundWindow()` 返回 **0**，`SetForegroundWindow` 永远失败，
  `AutomationElement.FocusedElement` 停在别的进程（宿主 MiniMax Code）。
- **判定方法（别用「按键没反应」倒推）**：`OpenInputDesktop(0, FALSE, DESKTOP_READOBJECTS)`
  返回 `NULL` + err=5 ⇒ **本会话的输入桌面不是我们所在的桌面**（会话锁定 / 断开 / 输入桌面被切走）。
  本轮实测：`station=WinSta0`、`threadDesktop=Default`、`inputDesktop=<NULL err=5>`。
- **同时正常的**：`EnumWindows`、`PrintWindow`、UIA 全部照常工作 ⇒
  **界面取证与注入失效可以并存**，前者能做完，后者只能等会话恢复。
- **屏幕捕获这条路本机彻底不可用**：`Graphics.CopyFromScreen` 三档尺寸全报「句柄无效」，
  `BitBlt` 三档全 `BITBLT_FAILED`（`GetDC` 有值但拿不到可绘制 DC）。
  ⇒ **改用 `PrintWindow` + UIA，不要再在 BitBlt/CopyFromScreen 上试。**
- **`PrintWindow` 的正确用法**：`Graphics.GetHdc()` 拿内存位图的 DC（不碰屏幕 DC），
  `[PW]::PrintWindow($hwnd, $hdc, 2)` → `ReleaseHdc` → `Save(png)`。
  本轮 `accept-palette-v2` 靠它稳定产出 4.3 MB / 5.0 MB 的 PNG。

### 5.10 命令面板有**非键盘**触发点：注入被禁时也能验收

- `src\modules\extensions\components\command-palette-toolbar-button.vue` 是窗口工具栏里的 ⌘ 按钮，
  `@click` 调 `shortcutsStore.executeShortcut('toggleCommandPalette')` ——
  **和键盘快捷键同一条 handler 链路**，用它能证明「面板本体可用」。
- 它是 `window-toolbar.vue` 动作层的**第一个**元素 ⇒ UIA 里取 **x 最小**的那个工具栏行按钮
  （本机 `rect=85,15 43x42`；28×28 CSS px × 1.5 缩放 = 42 物理 px，尺寸也对得上）。
- 该按钮**没有 aria-label / title**（只有图标），UIA `Name` 为空
  ⇒ 取元素时**不能只筛有名字的**；但 `InvokePattern` **可用**，`Invoke()` 能真正触发 Vue 的 `@click`。
  ⚠️ `Invoke` 返回成功 ≠ 面板真的开了，**必须用 UIA 里出现「命令面板」来判定效果**。
- 兜底（未用上）：给 `Chrome_RenderWidgetHostHWND` 投 `WM_LBUTTONDOWN(0x0201)` /
  `WM_LBUTTONUP(0x0202)`，`lParam` 用该窗口客户区坐标（`ScreenToClient` 换算）。

---

## §6 待用户决策 / 需用户本人操作

1. **批次 3 的右键菜单改动范围**：是「先只接文件条目右键菜单」还是「连同目录树、驱动器卡一起接」。
   建议先只接文件条目（`dir-entry-context-menu.vue`），其余按批次推进 —— 待用户确认。
2. **winget PR（任务 E）** 仍等外部审核员，Agent 代劳不了，保持等待。
3. **focus-sync（任务 G）** 需用户确认同步状态，未确认前不动 `user-extensions.json`。
4. **若还要补「物理键盘 `Ctrl+Shift+P`」这一项真机证据**：需要用户先解锁/恢复交互会话
   （当前会话的输入桌面 `OpenInputDesktop` 返回 err=5，注入不了按键）。
   解锁后重跑 `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\accept-palette-v2-20261103.ps1`
   即可；**面板本体已验收通过，这只是补一项旁证**，不阻塞批次 2 开工。

---

## §7 可复用脚本索引（全部在 `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\`）

| 脚本 | 用途 |
|---|---|
| `b5-build4.cmd` | 打包用。`vcvars64.bat` + 注入 rustup toolchain PATH + `npm run tauri:build`，**内部重定向**到 `b5-build4.log`，末尾写 `[build] BUILD_EXIT=`。**纯 ASCII** |
| `accept-palette-v2-20261103.ps1` | **命令面板真机验收（当前唯一可用的一版）**：起新 exe → 找主窗 → 关更新日志模态框 → UIA 快照（`FromHandle` 入口）→ 找工具栏里 x 最小的按钮（命令面板 ⌘ 按钮）→ `InvokePattern.Invoke()` → 再取 UIA 快照 + `PrintWindow` 前后两帧像素 diff → 输出面板标题、描述、占位符、**全部命令 ListItem**。自带起进程与收尾杀进程。**纯 ASCII**。日志：`accept-palette-v2-20261103.log` |
| `diag-uia-20261103.ps1` | UIA 树体检：打印 app 进程树、窗口子 HWND（含 `Chrome_RenderWidgetHostHWND`）、`RootElement` 顶层窗口、`FromHandle` 全部节点、`RawViewWalker` 遍历、坐标采样。**排查「UIA 取不到东西」时先跑它** |
| `diag-input-20261103.ps1` | 输入桌面体检：`station` / `threadDesktop` / `inputDesktop` / `OpenInputDesktop` 错误码 / `GetForegroundWindow` / `SendInput` 返回值与错误码。**判定「按键注入能不能用」的唯一依据** |
| `accept-palette-clean-20261103.ps1` | 旧版（键盘注入路线）。本轮因输入桌面被锁而**跑不出结论**，保留作输入恢复后的备用。截图已改成 `PrintWindow`，UIA 已改成 `FromHandle` |
| `accept-palette-full-20261103.ps1` | 旧版一键包装（起进程 → 调上面那条 → 杀进程）。新流程用 `accept-palette-v2` 自带起进程，不必再用它 |
| `task3-search-accept-20261102-v2.ps1` | 批次 4 的全盘搜索计时探针（判据 4 轮） |

运行姿势（**不要挂 stdout 管道**，会让后台任务跟着管道一起死）：

```powershell
# 关键：启动和轮询必须分成两个进程。前台调用只负责 Start-Process，10 秒内返回；
# 轮询放到独立的 background 任务里，且那个任务不能是构建进程的父进程。
Start-Process -FilePath 'cmd.exe' -ArgumentList '/c','F:\...\sigma-file-manager\.superpowers\sdd\b5-build4.cmd' -WindowStyle Hidden
```

---

## §8 新会话接力用的一句话 prompt

把下面**整段**（从「请先按顺序」到最后一个句号）复制给新会话，无需再附加任何背景说明：

> 请先按顺序完整阅读这 5 份文档（完整路径如下），据此掌握 Alpha File Manager 项目的背景、目标、已完成进度与下阶段计划，读完先向我简述你理解的当前现状与准备先做哪一步，不要复述文档内容，也不要重做已完成的事：
> 1) `F:\soft\00selfmade\filemanager\AGENTS.md` —— 项目硬约束总纲，优先级最高（注意：它作为项目指令注入时会被截断到 32768 字节，需要完整内容时直接读源文件）；
> 2) `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-03-phase5-batch1-action-registry.md` —— 本阶段交接入口，含批次 1 的当前状态与下阶段任务；
> 3) `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\plan-2026-11-02-action-registry-and-incremental-index.md` —— 阶段五计划：批次 1–7 的顺序、验收判据、风险与明确不做的事；
> 4) `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\analysis-2026-11-02-files-benchmark.md` —— 与 files-community/Files 的对标分析，是本阶段的需求来源；
> 5) `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-02-phase4-winget-and-search-scan.md` —— 上一阶段交接原文，含 winget 发版与全盘搜索扫描并行化的历史踩坑记录。
> 本阶段从**批次 2**（快捷键 store 绑 id + 快捷键设置页可编辑）开始；开工前先核对交接文档 §2「当前状态」与你实际看到的工作区是否一致，不一致先报告再动手。