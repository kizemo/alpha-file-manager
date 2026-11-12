# 项目工作约定

本文件是本仓库的 Agent 约定。后续会话在改动本项目代码前必须先读本文件。

## 0. 项目宪章（必读：文件名机制 / 铁律 / 验证装置地图 / 目录与工具索引）

### 0.1 上下文文件怎么被加载（先看这条，别重复造文档）

- MiniMax Code 载入项目约定时，在一个目录里按 `["AGENTS.md", "AGENTS.MD", "CLAUDE.md", "CLAUDE.MD"]` 顺序查找，**命中第一个就立刻返回**（`loadContextFileFromDir` 读到即 `return`，不会继续找）。
- ⇒ 本仓库**只有 `AGENTS.md` 这一份**，且**不要新建 `CLAUDE.md`**：它排在 `AGENTS.md` 之后，永远不会被读取；多出来的文件只会变成没人维护的第二份「铁律」，日后必然与本文件冲突。**`Agent.md` 根本不在候选列表里**，建了也不会有任何产品读它。
- 加载范围是「**当前工作目录及其上游祖先目录**」，**不向下进入子目录**。⇒ 会话的工作目录必须是 `...\sigma-file-manager` 本身。把上一层 `...\filemanager` 当 workspace 时，本文件**不会被注入**，Agent 只能靠 prompt 显式指定路径去读。
- ⇒ 本文件是**项目宪章 + 坑位库的唯一权威入口**。新约定一律追加进本文件；确实需要长正文时放进 `docs/`，并在此登记路径。

### 0.2 铁律（违反即返工）

#### 产品与品牌

- 面向用户的输出一律**简体中文**（细则见 §1）。
- 品牌名固定 **Alpha File Manager**，命名与图标已全部锁定，不再讨论。
- **受保护常量不得改动**：mDNS 服务名、legacy 数据目录名、Tauri `identifier`。

#### 验收与构建

- **真机验收只用 `npm run tauri:build` 的打包产物**。`tauri dev` 下所有路由页内容区空白是已知特性（`RouterView` 已挂载但不渲染子组件），**不是缺陷**；在 dev 构建上做的界面验收结论一律作废。
- 判断「结论必须真机才能成立」就**自己打包，不等用户开口**。判定条件：新功能缺界面证据、缺陷只有截图能确认、改了安装器/打包脚本（NSIS 钩子、内嵌资源、单实例守卫）。`tauri build` 很慢（Rust release + 打包）⇒ **早启动、丢后台**，同时做不占构建锁的活。
- 后台构建**不许挂 stdout 管道**（`Tee-Object` / `| Select-String`）：管道一断，进程树跟着死，编译到一半被标 `Command aborted`、产物没落地。一律让 `.cmd` 内部 `> logfile 2>&1` 重定向到文件，再单独读文件看进度。
- `.superpowers/sdd/*.cmd`、`*.ps1` **必须纯 ASCII**：`cmd.exe` 按系统 ANSI 代码页解析，中文会变乱码并被当成命令执行。
- **`SFM_E2E_DATA_DIR` 只在 debug 构建生效**（`src-tauri/src/app_dirs.rs` 的覆盖是 `#[cfg(debug_assertions)]`，release 把它编译掉）。release 产物上做真机验收又不想动用户真实数据，只能「**备份真实文件 → 灌夹具 → 验收 → 恢复**」。真实数据在 `%APPDATA%\com.sigma-file-manager.app\user-data\`。
- **恢复真实数据的顺序是「先杀进程 → 再 restore」**：应用退出时会把内存里的状态写回 `user-stats.json`，进程还在就会把真实文件再覆盖成夹具。restore 前必须确认 `Get-Process alpha-file-manager` 为空，恢复后核对字节数与条目数。
- 观察窗口要足够长：本项目的扩展冷启动需要 **3–5 分钟**，启动初期的探针失败不等于功能坏了。

#### 工具与环境

- 不得用 PowerShell `Get-Content -Raw` / `Set-Content` 改带中文注释的源文件：无 BOM 的 UTF-8 会被按 ANSI 读坏。批量改 JSON 用 node 写脚本。
- 拿不到 webview console 是本项目常态（wry 不透传 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`，CDP 连不上）。可用取证手段：`PrintWindow(hwnd, hdc, 2)` 截图、UIA 树枚举（Tauri 窗口 class 为 `Tauri Window`）、页面内 Vue DevTools 组件搜索（UIA `ValuePattern` 写入）。

#### Agent 工作方式

- 项目所有文件**只落在 `sigma-file-manager\` 内**。外层 `F:\soft\00selfmade\filemanager` 只保留迁移源与 `_trash-2026-11-10`，不往里写新文件、不在外层另起项目。
- 中间产物（长日志、截图、探针脚本、状态快照）一律落文件，对话里只留「路径 + 结论」，不要把长输出回灌。
- 单轮工具调用控制在 10 次以内；超过 20 次立刻停止逐步试探，改为一次取齐信息。
- 根因已坐实、修复可逆的明显 bug，**直接按推荐方案修完汇报**，不把工程判断做成选择题。只有「删/覆盖用户数据、付费或对外发布、大量耗时或改变架构方向」才必须先问。

### 0.3 验证装置地图

按「改了什么 → 跑什么 → 看什么」使用；除真机项外均可在仓库根执行。

| 装置 | 覆盖 | 通过判据 | 留证 |
| --- | --- | --- | --- |
| `npm run ts`（`vue-tsc --build`） | 前端 TS + Vue SFC 类型 | 无 error | 终端 |
| `npm run lint:eslint:check`（`eslint . --max-warnings 0`） | JS / TS / Vue 规范 | 0 warning；**error 数基线随提交漂移**，要比基线必须在 HEAD 上量（`git stash` → 跑 → `git stash pop`） | 终端 |
| `npm run lint:stylelint:check` | `src/**/*.{css,vue}` | 0 warning | 终端 |
| `npm run test:unit:run`（`node ./scripts/vitest-run.mjs run`） | Vitest 单元测试 | 全绿 | 终端 |
| `npm run check` | `ts` + `lint:check` + `test:unit:run` + `verify-changelog` | 全绿（本仓库默认提交前闸门） | 终端 |
| `npm run check:win` | `check` 的真机 E2E 版（多跑 `test:e2e:webdriver`） | 全绿；首次需 `npm run test:e2e:webdriver:install-edge-driver` | 终端 |
| `npm run verify-changelog` | CHANGELOG 与版本一致性 | 无差异 | 终端 |
| `npm run check:winget` | winget manifest 合法性 | 无报错 | 终端 |
| `npm run tauri:build` | **真机验收的唯一合法产物** | 构建成功，安装包落到 `release/`（末尾脚本只复制、**不上传**） | `release/` |
| `npm run release:verify` / `npm run verify:publish-quarantine` | 发布物完整性 / 隔离校验 | 无异常 | 终端 |
| `npm run sync-i18n` | 17 个语言包同步 | 生成物无意外 diff | `git diff` |

**真机验收专用装置**（在 `.superpowers/sdd/`，不受 eslint 门禁）

| 装置 | 用途 | 关键约束 |
| --- | --- | --- |
| `build-fb4.cmd` | 注入 MSVC / Rust 环境后打包 | 纯 ASCII；内部 `> log` 重定向，不挂管道 |
| `app-key.ps1` | `SendInput` 键鼠注入 | `INPUT` 必须 `[StructLayout(LayoutKind.Explicit, Size = 40)]`，否则 `SendInput` 静默返回 0；文本一律 `KEYEVENTF_UNICODE` 逐字符键入 |
| `dash-accept-data.cjs backup\|seed\|restore` | release 验收的用户数据备份 / 灌入 / 恢复 | `backup` 语义是「文件不存在才建」，**不覆盖旧备份** ⇒ 必须看它打印的字节数 |
| `afm-capture-only.ps1` | 只截主窗口 | 先 `SetProcessDPIAware()`；主窗口用 `EnumWindows` 取 class=`Tauri Window` 中可见且面积最大者，**不要**用 `Process.MainWindowHandle` |

**eslint 不覆盖的目录**（`eslint.config.ts` 的 `globalIgnores`）：`**/dist/**`、`**/dist-ssr/**`、`**/coverage/**`、`**/e2e-webdriver/**`、`src-tauri/**`、`_acceptance/**`、`.superpowers/**`、`graphify-out/**`、`extensions/**/src/generated-*.js`、`scripts/archive-focus-sync/**`。⇒ 这些地方的问题 lint 抓不到，别把「lint 绿」当成「全都对」。

### 0.4 目录与工具索引

| 路径 | 是什么 |
| --- | --- |
| `src/` | Vue 3 前端：视图、Pinia store、路由、17 个语言包 `src/localization/messages/**`（中文的 locale code 是 `ch`，不是 `zh`） |
| `src-shared/` | 前后端共享的 TypeScript 代码 |
| `src-tauri/` | Tauri 2 / Rust 主进程；`src-tauri/src/app_dirs.rs` 决定数据目录解析 |
| `e2e-webdriver/` | WebDriver E2E，独立 `package.json`（用 `--prefix e2e-webdriver` 调用） |
| `packages/`、`extensions/` | 本地包与扩展；`extensions/kizemo.focus-sync/` 是 Focus Sync 侧车 |
| `scripts/` | 构建、发布、校验脚本：`sync-version.js`、`verify-changelog.mjs`、`vitest-run.mjs`、`publish-bundle-to-release.mjs`、`release-publish.mjs` |
| `docs/` | 文档库：`handoff-*.md`（交接）、`plan-*.md`（计划）、`prompt-*.md`（派发）、`design-*`、`work-*`；子目录 `archive/ blog/ brand/ marketing/ operations/ screenshots/ superpowers/ winget/` |
| `graphify-out/` | 代码导航产物（用法见 §3） |
| `dist/`、`release/` | 前端构建产物、安装包落点（均被 gitignore） |
| `_acceptance/` | 验收留证（被 gitignore） |
| `.superpowers/sdd/` | 探针脚本与日志的工作区（eslint 忽略） |
| `flatpak/`、`snap/` | 非 Windows 打包配置 |

**接手一个任务时的阅读顺序**：本文件 §0 → 最近一份 `docs/handoff-*.md` → 对应的 `docs/plan-*.md` → 再动代码。历史坑位见 §2 与附录 A1–A7。

## 1. 输出语言

**面向用户的输出一律使用简体中文。**

具体包括：

- 对话回复、进度说明、结论与汇报
- 交接文档（`docs/handoff-*.md`）、计划与评审文档
- 应用界面文案与 17 个语言包（`src/localization/messages/**`，中文的 locale code 是 `ch` 而非 `zh`）中的中文值
- 面向使用者的说明性文字（README、操作指引、提示信息）

例外，以下内容保持原样、不得翻译或改写：

- 代码标识符：变量名、函数名、文件路径、命令行、配置键名
- 技术专有名词本身（Pinia、Vitest、NSIS、sha256 等），在句子中可保留英文
- 代码注释中的标识符引用

判断标准：如果这句话是给人看的，就用简体中文；如果是机器要读的，就保持原样。

## 2. 本项目的其他硬约束

- 品牌名统一为 **Alpha File Manager**，相关决策已全部锁定，不要重新讨论命名或图标。
- 受保护常量不得改动：mDNS 服务名、legacy 数据目录名、Tauri `identifier`。
- 验证结论前先确认观察窗口足够长（本项目扩展冷启动需 3–5 分钟）。
- **真机验收必须用 `tauri build` 的打包产物，不能用 `tauri dev`**。`tauri dev` 构建下应用外壳正常（左侧导航、盘符、状态栏都在），但**所有路由页面的内容区都是空白**——`RouterView` 已挂载却不渲染任何子组件（Vue DevTools 里搜 `navigator` / `settings` 只命中 Pinia store，搜 `sidebar` 能命中组件，可据此判别）。2026-10-12 已归因：该现象把 `src/` 全部回退到改动前仍然复现，而同机已安装的打包版完全正常，因此是 dev 构建特有，**不是代码缺陷**。在 dev 构建上做的界面验收结论一律作废。
- **需要装机实测就直接打包 exe，不要等用户开口**（用户 2026-10-19 明确要求）。判定条件是「**结论需要真机才能成立**」：新功能缺界面证据、缺陷修复只有截图能确认、改了安装器/打包脚本（NSIS 钩子、内嵌资源、单实例守卫）——这类不要只交代码 + 门禁数字就把「请用户装包」写成下一步。命令用 `npm run tauri:build`（末尾的 `publish-bundle-to-release.mjs` **只把安装包复制到仓库 `release/`，不上传**，可以放心跑）。`tauri build` 很慢（Rust release + 打包），所以**一旦判断出要真机验收就早启动、丢后台跑**，再做文档、补测试等不占构建锁的活。构建脚本放 `.superpowers/sdd/*.cmd` 且**必须纯 ASCII**（cmd.exe 按系统 ANSI 代码页读 `.cmd`，中文注释会变乱码并被当成命令执行）。**后台跑的时候不要挂 stdout 管道**（`Tee-Object` / `| Select-String`）：管道一断，进程树跟着死 —— 2026-10-23 就因为 `Tee-Object` 让后台任务在 Rust 主 crate 编译到一半时被标成 `Command aborted`，产物没落地、十分钟白跑。**长编译一律让 `.cmd` 内部用 `> logfile 2>&1` 重定向到文件**，再单独读文件看进度。
- **`SFM_E2E_DATA_DIR` 只在 debug 构建生效**（2026-10-23 实测）：`src-tauri/src/app_dirs.rs` 的隔离覆盖是 `#[cfg(debug_assertions)]`，**release 构建把 override 编译掉**，永远走 Tauri 路径解析、读写真实 user-data。⇒ 在 release 产物上做真机验收且不想动用户真实数据，只能「**备份真实文件 → 灌夹具 → 验收 → 恢复**」（本轮用 `.superpowers/sdd/dash-accept-data.cjs backup|seed|restore` 走 `user-stats.json`；真实文件在 `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\`）。**验收后必须恢复并核对字节数与条目数**，别把夹具留给用户。
- **恢复真实数据的顺序必须是「先杀进程 → 再 restore」**（2026-10-23 真机踩到，代价是真实数据被夹具顶掉一次）：应用退出时会把**内存里的统计状态写回**真实 `user-stats.json` ⇒ 夹具灌进去之后，只要应用还在跑（哪怕是被 `Stop-Process` 杀掉），磁盘上的真实文件就变成夹具残留。**restore 之前务必确认 `Get-Process alpha-file-manager` 为空**，否则 restore 完还会被再写回去一次。
  ⚠️ 连带一条：`dash-accept-data.cjs backup` 的语义是「**备份文件不存在才建**」⇒ 它**不会**覆盖旧备份。上一轮留下的 `.dashaccept-backup` 可能才是**真正的真实数据**，而磁盘上的 `user-stats.json` 已经是夹具残留（本轮就撞上了：`backup` 报「already exists, keeping the older one」，而它报的 5191 字节正是夹具大小）。⇒ **跑 backup 后一定看它打印的字节数**，若明显小于真实值，先 `inspect` 备份文件本身再决定信谁。
- 拿不到 webview console 是本项目的常态：wry 不透传 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`，CDP 连不上。可用的取证手段是 `PrintWindow(hwnd, hdc, 2)` 截图、UIA 树枚举（`InvokePattern` 可点击，Tauri 窗口 class 为 `Tauri Window`），以及页面内 Vue DevTools 的组件搜索框（UIA `ValuePattern` 可写入）。UIA 判别时要带对照项，别只搜一个名字。
- **`SendInput` 其实能进 WebView2 —— 此前「SendInput 对 WebView2 无效」的结论是错的，已作废**（2026-10-15 21:5x 复核）：`.superpowers/sdd/app-key.ps1` 的 40 字节 `INPUT` 结构、`SetForegroundWindow`、`Marshal.SizeOf=40` 都是对的，`SendInput` 每次都返回 1，Ctrl+V 真的把剪贴板内容粘进了工具栏过滤框并触发了过滤。真凶是**那个脚本的字面量解析**：非组合键 token 会走 `[Convert]::ToUInt32(char.ToUpper(), 16)`，即把字母当十六进制解析，于是 `V` / `W` / `I` 这些非 A–F 的字母直接抛 `FormatException`，脚本中途退出 —— 看起来就像「键打不进去」。已把 A–Z 的 VK 补进 `$named` 表。
  ⇒ 由此得到输入文字的**首选路径已改为 `KEYEVENTF_UNICODE` 逐字符键入**（2026-11-08 真机实测）：`wVk=0`、`wScan=字符码位`、键下 `dwFlags=0x0004`、键上 `0x0004|0x0002`，24 字符一批、批间 sleep 30 ms；该路径 DOM 与 Vue `v-model` 都同步（实测名称框 `NAME_LEN=11`、Cookie 文本框 `COOKIE_LEN_IN_FIELD=271`）。**`Set-Clipboard` + `CTRL+V` 只在部分场景可靠**（文件浏览器工具栏过滤框可以），**设置页表单里不落地**：剪贴板读回长度正确、`SendInput` 返回 4、输入框有焦点边框，但 DOM 仍为空、Vue 校验直接报「连接名称不能为空。」⇒ 往 WebView2 输文本一律走 `KEYEVENTF_UNICODE`。**失败的替代方案，别再试**：往 `Chrome_RenderWidgetHostHWND` 投 `WM_CHAR`（`0x0102`）会被静默丢弃；剪贴板 + 中键粘贴（`WM_MBUTTONDOWN/UP`）也不会触发粘贴。
- **自己写注入脚本时，`INPUT` 结构必须显式 `[StructLayout(LayoutKind.Explicit, Size = 40)]`**（2026-10-23 踩到）：官方 `INPUT` 的 union 同时容纳 `MOUSEINPUT`（32 字节），总长 40。若只声明 `KEYBDINPUT`，`Marshal.SizeOf` 算出来会小于 40，`SendInput` **静默失败**（返回 0、不报错、不抛异常），表现是「粘贴没反应 / 键打不进去」—— 本轮就看到「Popover 打开了、输入框有焦点光标，但 `Ctrl+V` 的文字始终没进去」。照抄 `.superpowers/sdd/app-key.ps1` 的结构定义即可。
- **注入探针要「阳性对照排在前面」**（2026-10-23）：先跑「必然命中」的那一项，再跑「必然不命中」的那一项。这样出现空结果时能立刻分辨是「注入没进去」还是「功能真坏了」；反过来先跑阴性对照的话，空结果有两种解释，白白多花一轮。
- **本轮（2026-11-08）新增四个坑**：① **DPAPI 解密**：`[System.Security.Cryptography.ProtectedData]::Unprotect` 对本项目密文抛「参数错误」，要用**裸 `CryptUnprotectData`**（无 entropy、`dwFlags=CRYPTPROTECT_UI_FORBIDDEN=1`）；`ProtectedData::Protect` 往返测试却是正常的 ⇒ 别据此判定「密文坏了」。② **PowerShell 取 JSON 字典**：`$obj.entries.PSObject.Properties[0].Value` **返回空串**（须先 `.Properties.Name` 取键名再 `Properties[$k].Value`）；空串 ⇒ `FromBase64String` 得 0 字节 ⇒ Unprotect 报 87「参数错误」，看着像密文损坏。③ **删除远程连接没有二次确认框**：点行内「删除」即刻生效；脚本若「点完再找一个同名确认按钮」，会误点到**下一行**的删除按钮（本轮误删了「速豹堂」SFTP，靠 `.superpowers/sdd/pre-e-backup/` 恢复）⇒ 删除类脚本必须**按行坐标定位**按钮、点击后**立即断言行数变化**。④ **UIA 同名元素按 ControlType 区分**：云图标悬停面板里的连接名是 `Button`（79,550），设置页连接列表里同名的是 `ListItem`（1552,1086）⇒ 只按名字取会点到设置页那一行。
- **UIA 探针两个新坑**（2026-11-08 踩到）：① PowerShell 函数名不要用 **`CP`** —— 内置别名 `cp`（= `Copy-Item`）优先级**高于**函数，调用会被静默劫持成复制命令，函数根本不执行，曾因此产出「rail 命中 43x42 按钮」的**假阳性**（`FindFirst` 空 Name 会匹配整窗）。改名 `Cps` 之类。② **`TreeScope::Descendants` + `Condition.TrueCondition` 全扫在本应用 Chromium 树不可用**：对 43x42 按钮和云图标全部 0 命中（`RAIL_BUTTONS=0`），不能拿它当正向对照 —— 可见性断言改用「**按名精确 FindFirst + 屏幕 BitBlt 实拍**」双口径。另：点击闭环比计数更硬（Invoke「+」后轮询到「添加远程连接」表单 = `CLICK_LANDED`，功能成立与否看这个）。
- **UIA 读不到 Radix Toast：toast 出现 ≠ UIA 查得到**（2026-11-02 踩到）：断链快捷方式验收时，toast 确实渲染在屏幕上（屏幕实拍 `accept-20261102-reveal/case2-toast-3.png` 可见「无法打开文件所在目录」+ description），但整棵 `AutomationElement` 子树里按名字搜**一个都搜不到** ⇒ 「UIA 命中数 = 0」**不能**判定 toast 没弹，必须配**屏幕 BitBlt 实拍**取证。
- **`WM_NULL` 的 LRESULT 本来就是 0**（2026-11-02 踩到）：用「`SendMessageTimeout` 的结果值 != 0」判应用是否响应，会对**健康窗口**得出 `HUNG`（假阴性，一度被当成应用卡死）。判响应要用 `SendMessageTimeout` 的 **BOOL 返回值**。
- **快捷方式目标失效时后端不打日志是预期行为**（2026-11-02）：`reveal.rs` 的 `prepare_reveal` 直接返回 Err ⇒ 断链场景日志里**没有** `[reveal]` 行属正常，别当成漏埋点。
- **Radix 菜单的 UIA 枚举有竞态**（2026-11-02）：菜单已可见时第一次枚举仍可能 `count=0`，紧接着第二次就命中 ⇒ 枚举菜单项必须**轮询重试**，查一次就判「菜单项不存在」会得出假阴性。
- **`Save-AfmShot` 拍的是整窗 ⇒ 屏幕坐标 = PNG 坐标 − 窗口原点**（2026-11-02）：`Capture()` 用 `GetWindowRect` 拍整窗（含边框），本机窗口 rect 起点为 `(-11,-11)` ⇒ `screen = png - 11`；反过来靠 PNG 坐标点右键会整体偏 11 px。
- **本应用的工具栏按钮不暴露 UIA `InvokePattern`**（2026-10-15 实测）：文件浏览器工具栏的过滤按钮探出来是 `[Button] 55x54 pats=`，空 pattern。**可用的第二条路**：`PostMessage(WM_LBUTTONDOWN=0x0201 / WM_LBUTTONUP=0x0202)` 发给 **`Chrome_RenderWidgetHostHWND`**（不是 `Chrome_WidgetWin_0`），`lParam` 用**该窗口客户区坐标**（屏幕坐标减去它的原点）。单击会被应用收到（grid 卡片会选中、过滤框会打开）。键盘则直接走上面的 `SendInput`。
  **过滤框本身在 UIA 树里是个空壳**：打开后只多出一个 `[Window] name="" 510x56 @(664,82)`（Chromium 给可编辑区建的原生 IME 窗口），下面挂的是 `-∞x-∞` 的 `[Group]`，**没有 `[Edit]`、没有 `ValuePattern`**。所以「用 UIA SetValue 写过滤词」这条路在本项目不存在，只能靠键盘注入。
  **写注入探针时必须带阳性对照**：先点一个必然有可见后果的元素（文件夹卡片 → 出现选中态），确认内容哈希变了，再去点目标按钮。否则「没反应」分不清是注入失败还是功能本身没反应。
  全局搜索的放大镜（标题栏 `(1450,38)`）与右键菜单项（Chromium accessibility 的 `MenuItem` Name）**有** `InvokePattern`，可以直接调。
- **验收夹具不要建在 `%TEMP%` 下**（2026-10-15 实测）：`src-tauri/src/global_search/ignore.rs` 的 `builtin_ignored_paths()` 含 `/AppData/Local/Temp`，且 `matches_plain_ignored_path` 对 `/` 开头的模式做 `path.contains(...)`。夹具放 `%TEMP%` 时整棵子树被剪掉，**目录内递归搜索的命中恒为 0** —— 看起来像产品缺陷，其实是被忽略规则挡死了。放到 `C:\Users\<user>\` 下的普通目录即可。
- **开关型控件要先探测再点击**（2026-10-15 实测）：过滤按钮是 toggle，上一轮脚本点了之后输入框还开着，下一轮再点就把它关了，于是得到一条假的「找不到输入框」。脚本里先查目标元素是否存在，不存在才点。
- **UIA 里从点往上找祖先会一路走到窗口级元素**（2026-10-15 实测）：`TAURI_DRAG_RESIZE_WINDOW` 覆盖整个客户区，取它的中心点右键 = 点在「搜索面板与导航器面板的缝」上，弹出的是导航器的菜单而不是搜索结果的菜单。必须给祖先加硬边界（宽度上限 + 落在目标区域内），不能只看「有 Name」。
- Chromium 把列表行暴露成什么 `ControlType` **不稳定**（2026-10-15 实测）：全局搜索结果行**不是** `ListItem`，`AutomationElement.FromPoint` 也没给出可用祖先。稳定的是行内那个 **28×28 的图标**（面板其它图标是 19×19 / 25×25），按尺寸挑第一个即可定位整行。
- **不要用 `Process.MainWindowHandle` 取本项目的主窗口**（2026-10-15 实测）：它会落在 15×15 的托盘窗 `com.sigma-file-manager.app-sic` 上；进程里还有隐藏的 `Alpha File Manager | Quick View` 与 `tray_icon_app` / `global_hotkey_app`。要自己 `EnumWindows` + `GetWindowThreadProcessId` 过滤，再取 **class 为 `Tauri Window`、可见、面积最大**的那个。
- **改 `user-settings.json` 的 `navigator.defaultDirectory` 不等于应用会打开那个目录**（2026-10-15 实测）：`user-workspaces.json` 里保存的当前标签页优先，会把启动目录钉死在原处。验收要量指定目录时，必须同时改掉 `workspaces[].tabGroups[][]` 中 `type === 'directory'` 的 `path`，否则量到的是另一个目录。
- 验收量到的是**物理像素**：本机 DPI 144（150% 缩放），换算 CSS 逻辑像素要除以 1.5。2026-10-15 那一轮就是因为忘了除，把目录卡的 `78` 读成「小档应为 52，不符」而差点误判成产品 bug。
- **验收脚本必须一开头就 `SetProcessDPIAware()`，否则 UIA 坐标与 `SetCursorPos` / `WindowFromPoint` / `GetWindowRect` 差 1.5 倍**（2026-11-02 钉死，这是「点按钮毫无反应」的真凶）：UIA 的 `BoundingRectangle` 返回**物理像素**，而 **DPI-unaware 的 PowerShell 进程**调 `GetWindowRect` / `SetCursorPos` / `WindowFromPoint` 用的是**逻辑像素**。同一窗口实测：unaware 时 `GetWindowRect=-7,-7 1721x927`、`SM_CXSCREEN=1707x960`；调用 `SetProcessDPIAware()` 后 `GetWindowRect=-11,-11 2582x1390`、`SM_CXSCREEN=2560x1440`，且 **`UIA_RECT=-11,-11 2582x1390` 与之逐字一致**。
  ⇒ 把 UIA 报的 `(2027,36)` 直接喂给 unaware 的 `SetCursorPos`，系统按逻辑坐标解释，实际落到物理 `(3040,54)` —— 已经出了屏幕，`WindowFromPoint` 因此命中别的窗口（本轮命中钉钉浮层、全屏 Windows 设置），遮挡守卫忠实地报 `PID_MATCH=False`，看着像「被遮挡」。**守卫没错，是坐标口径错了。**
  ⇒ 正确姿势：脚本在**任何 user32 调用之前** `[DllImport("user32.dll")] static extern bool SetProcessDPIAware();` 并立即调用，之后三套坐标统一到物理像素可直接互用。生效判据：`GetSystemMetrics(0)` 从 `1707` 变成 `2560`。
- `PrintWindow(hwnd, hdc, 2)` 在大列表**平滑滚动动画未结束时**会抓到未绘制的黑块。遇到疑似渲染缺陷，先延长等待并连拍两次取后一帧再下结论——别把抓屏时机问题当成产品缺陷上报。
- `PrintWindow` **不渲染 hover / overlay 层**（悬停才出现的快捷面板、右键菜单都不在里面）。验这类交互只能走**屏幕 DC BitBlt**，即拍屏幕合成结果。
- 屏幕 BitBlt 只拍屏幕合成结果：**别的窗口盖在上面就会拍到别的窗口**（此前 Edge 盖住应用导致拍到 GitHub 页面，产出两次假的「点击没反应」）。用 `WindowFromPoint` + `GetAncestor(h, 2)` 校验实际绘制者。
  - **但白名单必须包含应用自己的 `msedgewebview2`**（2026-10-15 实测）：webview2 的 `Chrome_WidgetWin_0` 顶层窗口就是光标落在页面区域时下方的那个窗口，`owner pid` 会是 `msedgewebview2.exe` 而不是主进程。只认主进程 pid 会把**每一张干净截图**都判成「被遮挡」而中止。正确做法是放行主进程 + 父进程链能回到主进程的全部 `msedgewebview2`。
- **悬停面板里的坐标不能硬编码**（2026-10-15 实测）：面板是 hover 驱动的，滚动位置在多次运行之间会漂（点树的滚轮落点与面板重叠时尤其明显），写死的表头 Y 会落到列表项上而不报错。要**在点击前从当帧实测字形位置**，按序号选中第 N 个，而不是按写死的 Y。
  - 面板 x 范围约 `70..480`，目录树 x 范围约 `80..340`，**两者重叠**。在导航器路由上想把面板滚到顶，滚轮落点必须避开目录树（用 `x≈455`），否则滚的是树。
- 验收脚本里 **PowerShell 函数只要有返回值，就不能在里面混用 `Write-Output`**：诊断输出会进管道并成为返回值的一部分，实测让点击坐标变成字符串、脚本在第一条断言前静默退出。函数内改用 `Write-Host`。
- **门禁基线必须用同一条命令量，混口径会误判成回归**（2026-10-15 实测）：项目 npm 脚本是 `eslint .`（整仓），基线数字 `186 errors / 121 warnings` 就是这个口径；改成 `eslint src` 在同一个基线提交（`0f73f159`）上只有 `179 / 121`——差的 7 个是 `src` 之外的存量。拿 `eslint src` 的数字去对 186，会凭空多出 7 个「回归」。
  稳妥做法是在基线建一个 worktree 跑**同一条命令**做逐字对照：
  `git worktree add --detach $env:TEMP\afm-baseline-eslint 0f73f159`（该目录已有 `node_modules`）。
- **`eslint .` 的数字必须是「干净检出」那个数**（2026-10-21 实测）：eslint **不读 `.gitignore`**，
  所以工作副本里的 `.superpowers/`（150 多个一次性 `.cjs` 脚本，`.superpowers/sdd/.gitignore` 是 `*`，
  整个目录不进版本库）会被一起 lint —— 同一个提交，工作副本里量到 **689 errors**，干净检出里只有 **177**。
  已把 `.superpowers/**` 与 `graphify-out/**` 写进 `eslint.config.ts` 的 `globalIgnores`。
  **2026-10-23 实测更新基线**（用 `git stash push -u` 量 HEAD、同一台机器同一条命令）：
  `eslint .` = **188 errors / 0 warnings**，其中 `eslint src` = **175**、`eslint scripts` = 13。
  ⚠️ 之前记的「177 / 170」**已过期** —— 3.5.0 那轮新增的
  `compare-sync-dialog.test.ts` 带进来 5 个存量违规，`eslint src` 变成 175。
  **基线数字会随提交变**，量的时候一律用 `git stash` 量 HEAD，别拿旧数字对。
  **看到几百个错误先按目录分组看**，别急着当成回归 —— 先分清「仓库里真有的」和「只有工作副本才有的」。
- **单文件 `eslint --fix` 通过 ≠ 回到基线**（2026-10-15 实测）：`--fix` 会把该文件里**基线本来就有的**违规一起修掉，单文件复验会显示 0 个问题，但总数可能已经低于或高于基线。必须再跑一次全量对照才算数。
- `$ErrorActionPreference = 'Stop'` 会把**原生命令的 stderr**变成终止错误（`NativeCommandError`），门禁脚本在调 `npm` / `npx` 前后必须显式切回 `Continue`，否则第一条 warning 输出的 stderr 就让脚本带 exit 1 退出，看起来像门禁挂了。
- `vitest --reporter=basic` 在本仓库这版不存在，会 `Failed to load custom Reporter from basic` 直接中止。跑全量单测用默认 reporter，再按 `Test Files` / `Tests` 两行抓结果。
- **打包的 MSVC/Rust 注入**（每次 bash 调用都要重来）：`cargo` 不在 PATH，在 `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin`。⚠️ toolchain 目录名里的 `pc-` 不能省（`1.90.0-x86_64-pc-windows-msvc`）；写成 `1.90.0-x86_64-windows-msvc` 时 `cargo metadata` 会报 `program not found`，`tauri build` 在 3 秒内失败退出（exit 1）——复用旧 `.cmd` 前先核对这一段；MSVC 环境靠 `call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"`（该路径存在，MSVC 工具版本 `14.44.35207`）。可直接复用 `.superpowers/sdd/build-fb4.cmd`。
- **绝不要用 PowerShell 的 `Get-Content -Raw` / `Set-Content` 改带中文注释的源文件**（2026-10-15 踩到，代价是一个文件的中文全变 mojibake）：PowerShell 5.1 的 `Get-Content` 默认按 **ANSI** 读无 BOM 的 UTF-8，中文注释读进来就是乱码，再写回去就把文件毁了。`Set-Content -Encoding UTF8` 还会额外写 BOM。
  要改仓库里的 `.vue` / `.ts` / `.json`，一律用**编辑工具**，或 `git checkout -- <file>` 恢复后重做；批量改 JSON 用 `node` 的 `fs.readFileSync/writeFileSync(..., 'utf8')`。
  改完立刻用 node 验一遍：`s.includes('某个中文注释')` 且 `s.match(/\uFFFD/g)` 为空，再往下走。
- **给 `user-stats` 加顶层字段，「读白名单」和「写列表」要一起改**（2026-10-15 真机踩到）：读侧 `USER_STATS_TOP_LEVEL_KEYS = Object.keys(DEFAULT_USER_STATS)`，把字段加进 `DEFAULT_USER_STATS` 就自动放行；写侧 `saveStats()` 却是**逐键显式 `set()`** 的固定五项。漏掉写侧的后果是：新建收藏分类 → 界面出现 → 下拉里也有 → **重启即丢**，`user-stats.json` 里该键是 `undefined`，全程不报错。已修（`saveStats` 补 `favoriteCategories`）。
  **只断言内存状态的单测抓不到这个**——`store.favoriteCategories` 一直是对的。要断言 `storedValues.get('favoriteCategories')` 这种真正走一遍 mock store `entries()` 的往返。
- **`InvokePattern.Invoke()` 返回成功 ≠ 点击生效**（2026-10-15 实测）：对**禁用**的按钮调 Invoke 是静默空操作，脚本照样打印 `invoked=yes`。「按钮有没有真的生效」只能靠**落盘文件**或**像素可见变化**判定。
  同理 UIA `ValuePattern.SetValue` 的 read-back 只说明值进了 DOM 输入框；**Vue 的 `v-model` 是否响应要单独验**（本项目实测会触发 `input`，可用「填一个不存在的路径，看校验提示是否出现」当阳性对照）。
- **`user-stats.json` 是验收反馈 4 的事实来源**：仪表盘的分类/收藏状态都能从这里读到，比看界面更硬。路径 `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-stats.json`。
- **`cargo test --lib` 的测试二进制里有一条隐形红线：任何被测试链接到的代码都不能碰到 `tauri::AppHandle` 或 `tauri::async_runtime::*`**（2026-10-16 踩到，代价是一整轮无法定位的排查）。
  现象：测试二进制在**加载阶段**以 `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉，一个测试都跑不了，报错里完全看不出跟你的模块有关。
  根因三步：① 本仓 `lib.rs` 的 `.run()` 持有全部 command，但测试不调 `run()`，所以基线上 **tao 一条导入都没有**，GUI 代码整段被链接器丢掉；② 一旦你的代码被测试链接到并碰到 `tauri::async_runtime::spawn_blocking`，它的 `static RUNTIME: OnceLock<GlobalRuntime<Wry>>` **带默认类型参数 `Wry`**，一处调用就单态化出 `<Wry as Runtime>`，把 tao 整条窗口链拖进来（实测新增 152 个导入，user32 占 104 个）；③ tao 里有 `comctl32.dll!TaskDialogIndirect`，它属于 **comctl32 v6**，只有 application manifest 声明了才解析得到，测试二进制没有 manifest → 加载器找不到导出。
  定位手段：`dumpbin /DEPENDENTS` 看新增 DLL → 二分 `mod` 声明缩小范围 → `dumpbin /IMPORTS` 与基线二进制**逐项 diff 导入表**。⚠️ 基线必须是 `git stash` 后**现重建**的那份，`target/debug/deps` 里的历史产物会给出「新增 152 个 tao 导入」这种假结论。
  绕法：给业务模块加 `*_blocking` 同步内核入口（见 `src-tauri/src/dir_reader.rs`、`src-tauri/src/global_search.rs`），调用方自己选执行器。
- **`#[tool_router]` 不等于接线完成**（2026-10-16，rmcp）：只写 `#[tool_router] impl X {…}` 加自写的 `impl ServerHandler for X { fn get_info }`，编译通过、单测通过，**只有 `tools/list` 返回 `{"tools":[]}`**，一个工具都调不到。必须给 `impl ServerHandler` 加 `#[tool_handler]`。rustc 对 `tool_router` 字段报的 `never read` 警告在这里**不是误报**，是线索。
- **验收时会有置顶的 Electron 窗口盖住应用**（2026-10-15）：遮挡守卫会正确地 `ABORT`，但 `ShowWindow(SW_MINIMIZE)` 对它无效（会自己弹回）。要把它 `MoveWindow` 到 `(-4000,-4000)`，并把原 rect 记进 `occluders.json` 以便收尾复原（`.superpowers/sdd/shift-occluders.ps1`）。
- **`net use` 的盘符参数必须带冒号**（2026-10-22 真机踩到，代价是绕了半轮）：`net use Y \\host\share`（不带冒号）会被 `net.exe` 当成「连接名叫 Y 的远程资源」，报 **System error 67 —— 找不到网络名**；而 **67 恰好也是「共享根本不存在」的错误码**，两者一模一样，UNC 路径明明完全正确却一直像在「路径拼错」。`net use Y: \\host\share` 才是映射盘符。**凡是拼 `net use` 命令行，盘符一律带冒号。**
- **`net.exe` 的输出是控制台代码页，不是 UTF-8**（2026-10-22）：中文 Windows 上是 cp936，按 UTF-8 解得到的是一串替换字符（用户看到「net use failed: ████████67██」，完全读不出原因）。要用 `MultiByteToWideChar` + `GetConsoleOutputCP` 按代码页解（`kernel32` 直接 `extern` 声明即可，零新增依赖）。
- **`PrintWindow` 会返回陈旧帧**（2026-10-22 实测）：进度事件把 WebView2 渲染线程刷爆时，`PrintWindow(hwnd, hdc, 2)` 拿到的还是上一帧 —— 实测两次**不同**的运行都画出「file-00354.dat」。**判后台任务的运行状态要看文件系统（文件数是否停止增长），不要看屏幕。**
- **门禁红了先分清「真红」还是「超时」**（2026-10-22）：`single-instance-guard.test.ts` 的两条用例红了，但报错是 `Test timed out in 30000ms` 而不是断言失败 —— 根因是守卫脚本要**枚举全机进程**（WMI），机器上有 431 个进程时一次 `Get-CimInstance Win32_Process` 要 6.5 秒，用例跑完 42.5 秒 > vitest 的 30 秒默认超时，脚本本身其实全部通过。**断言失败才有内容可看；超时是另一回事。**
- **`.ps1` 里也不能有中文**（2026-10-22，和 `.cmd` 同理）：PowerShell 5.1 按 ANSI 读无 BOM 文件，`.ps1` 里的中文正则会把解析器搞崩（报 `Unexpected token` + 字符串终止符丢失）。中文一律**作命令行参数**传入（命令行能正确带 Unicode）。`.ps1` 同理必须纯 ASCII。
- **`.cmd` 的 `echo` 里不能出现 `>`**（2026-10-24 实测）：cmd.exe 把 `>` 当**重定向符**，连注释和 echo 文本里的都不放过。写 `echo [release] bump minor -> 3.6.0` 的结果是：① echo 的输出被截断在 `-` 处；② **仓库根目录多出一个名为 `3.6.0)` 的垃圾文件**（`> 3.6.0)` 的重定向目标）。⇒ 描述性文字里改用 `to` / `=>`，或者把整句用引号括起来。症状是「构建日志少了一行 + git status 里冒出一个看不懂的文件」。
- **`.cmd` 运行期间绝对不能编辑它**（2026-10-24 实测，代价是一次发版返工）：cmd.exe 不是一次性把脚本读进内存，而是**按字节偏移逐行读**。在脚本跑到一半时改文件，后面几行的偏移全部错位 ⇒ cmd 会执行**别处的指令片段**。本轮的实际后果：改了一行注释后，cmd 接着执行了**重复的 `release:prepare --bump minor`**，把版本从 3.6.0 顶到 **3.7.0**，第二次的 `>` 还把日志截断，十来分钟的 Rust 构建产物全丢。⇒ ① 要改就**先停任务**（`task_stop`）再改，再**新建一个文件名**重跑，不要复用同名文件；② 脚本开头把这条写进注释（但注释不能是中文，cmd 按 ANSI 读）；③ 改动前先 `git diff --stat` 确认版本号没被顶掉，`package.json` 与 `src-tauri/Cargo.toml` 三处必须一致。
- **`SendInput` 的 Ctrl+V 要用正确的 VK 码**（2026-10-22）：VK_LCONTROL=0x11、**VK_V=0x56**。写成 `0x10`（那是 VK_SHIFT）发出去的是 Ctrl+Shift，字段一直空着，看起来像产品 bug。粘贴前还要先 **Ctrl+A 全选**，否则是**追加**而不是替换。
- **注入的 Shift 到不了 webview 的 `shiftKey`（2026-10-22，Edge 隔离复现，与 Tauri 无关）**：
  用 Edge 打开一个本地按键日志页（`.superpowers/sdd/probe-shift-in-edge.ps1` + `key-probe-server.cjs`）实测：
  `Shift+Delete` / `Shift+ArrowDown` 到达页面时 `shiftKey=false`，而 `Ctrl+Shift+F` 正常（`shift=true ctrl=true`）。
  形态是**修饰键的 keyup 抢在主键 keydown 之前到达**。
  后果：应用里 `Shift+Delete` 变成**「移至垃圾箱」**而不是「从驱动器中删除」—— 看着像产品 bug，其实是注入缺陷。
  ⚠️ **以下四条都试过，全都不解决问题，别再试**：① `VK_SHIFT 0x10` → `VK_LSHIFT 0xA0`；② 改用扫描码（`KEYEVENTF_SCANCODE`，shift=0x2A、del=0x53）；③ 把整个组合塞进**一次** `SendInput` 调用；④ 每次一个事件 + `Start-Sleep 60ms`。
  **验收永久删除不要依赖键盘**，改走右键菜单顶部图标条里的垃圾桶（见下条）。字母键的 `Ctrl+Shift` 组合是可靠的，只有「单修饰键 + 非字母主键」会丢修饰键。
- **永久删除的入口不在工具栏里，在文件条目右键菜单顶部的图标条里**（2026-10-22 实测，**上一轮写的「工具栏的垃圾桶下拉」是错的**）：
  `file-browser-actions-menu.vue` 全仓只被 `dir-entry-context-menu.vue` 引用 —— 也就是**右键菜单**，没有第二处。
  它的 quick-actions 区（`display:flex`）是菜单**顶部**的一排**纯图标按钮**：实测 5 个**可访问名为空**的 `Button`
  （屏幕坐标 x=2124/2178/2232/2286/2340, y=261, 各 48×42），依次是 铅笔(重命名) / 复制 / 粘贴 / 链接 / **垃圾桶**。
  垃圾桶点开是 Radix `DropdownMenu`：两项「移至垃圾箱 Del」与「**从驱动器中删除 Shift+Del**」。
  ⚠️ **三个坑**：① 图标按钮**没有可访问名**，UIA 取元素时**不能只取有名字的**（否则永远找不到它）；
  ② 下拉项的**可访问名带快捷键**（`从驱动器中删除 Shift+Del`），要按**子串**匹配，按等值匹配找不到；
  ③ 永久删除确认框的按钮名也是「**删除**」（`dialogs.deleteIrreversibly.delete`），与下拉项文字**同名** —— 必须靠 ControlType 区分（`MenuItem` vs `Button`）。
- **右键菜单作用于「光标下的那一个条目」**（2026-10-22）：`dir-entry-interactive.vue` 里是 `:entries="[dirEntry]"`。
  ⇒ 想一次删掉一整棵树，靶子必须**挂在一个顶层条目下面**（`…\target\d000..d019\…`），
  右键那个 `target` 才是入口。**没有多选入口**（`Ctrl+A` 选完再右键，右键菜单里也只有那一个）。
  ⇒ 条目总数口径随之变：`directories` = 子目录数 **+1**（顶层目录自己），所以 `20 × 500` 的靶子是 **10,021** 项，不是 10,020。
- **量进程线程数当并行证据时，基线必须在触发动作之前取**（2026-10-22）：触发用的辅助脚本自己会 sleep 2 秒，
  事后取基线会把**已经开始干活的 worker 线程**算进基线里，于是「峰值 == 基线」看起来像「没有并行」。
  正确量法：删除前 71 → 删除中峰值 75（正好 +4，对上 `MAX_PARALLEL_WORKERS = 4`）。
- **PowerShell 里 `[System.IO.Directory]::EnumerateFiles(...).Count` 不会真的枚举**（2026-10-22）：
  惰性 IEnumerable 上的 `.Count` 拿到的是**扩展方法**而不是元素数，要包一层 `@(...)`。
  不包的后果是计数恒为 -1，验收脚本会一路等到超时才退出。
- **应用启动后会弹「新功能」更新日志模态框，把整个文件列表挡住**（2026-10-22）：UIA 树里此时只有更新日志的内容，**一个文件卡片都找不到**。验收脚本必须先关掉它（关闭按钮名「关闭」= U+5173 U+95ED），否则「找不到卡片」会被误判成页面没加载。
- **应用日志文件被应用独占锁着**（2026-10-22）：应用运行时 `[System.IO.File]::ReadAllText($appLog)` 抛「文件正由另一进程使用」。必须用
  `File.Open($p, 'Open', 'Read', 'FileShare.ReadWrite')` + `StreamReader` 读。日志时间戳是 **UTC**（比本地少 8 小时）。
- **`publish-bundle-to-release.mjs` 会遍历 `bundle/nsis/` 下所有历史安装包**，逐个 `rmSync` + 复制到 `release/`。只要 `release/` 里**任何一个**旧包被别的进程锁住（实测 3.4.0 被锁），整步就 exit 1 —— 而 `tauri build` 本身早已成功。
  ⚠️ 打包后要**分清「构建失败」和「复制失败」**：新包用 `node -e "fs.copyFileSync(a,b)"` 单独补进 `release/` 即可，不必重跑整个打包。
- **删除的条目总数要把「顶层路径本身」算进去**（2026-10-22）：`scan_paths` 对目录路径会 `entry.directories = 1`（把它自己算进去），再递归数子项。
  所以「20 个目录 × 每目录 500 个文件」的靶子，条目总数是 **20 × 501 = 10,020**（不是 10,000）；终值 `deleted_items` 必须与 `total_items` **逐个相等**，不等就是有 bug。
- **watcher 生命周期不得阻塞目录渲染（2026-11-03 复发修复，第三次复发就到这条）**：`use-file-browser-navigation.ts` 的 `readDir` 曾把 `Promise.all([loadDirectoryContents(...), stopWatching()])` 绑在一起，只要 `stopWatching()` 不 settle，就会出现「进入目录长时间不出列表 + 刷新按键一直转圈 + 之后每次导航全挂」，而**日志里一条 error 都没有**（静默挂起）。基因是 `watchedPath` 未清空 ⇒ 后续每次导航都去 await 同一个挂起的 promise；`enqueueWatcherOperation` 的 `watcherOperation = result.catch(() => {})` 又把它变成**全局串行队列永久毒化**。
  ⇒ 硬约束：① 渲染路径**只 await 读目录本身**，停监听一律 fire-and-forget（`void stopWatching()`）；② 任何 watcher invoke 必须走带超时的包装（`withWatcherTimeout`，5 s 放行、永不 reject）；③ `watchedPath = null` 要在 `await` **之前**；
  ⇒ 守门测试：`composables/__tests__/use-file-browser-navigation.test.ts` 的 `watcher teardown isolation` 用例让 `unwatch_directory` 返回永不 settle 的 promise。**谁把 `Promise.all` 写回去，它立刻红**。
- **诊断埋点永远不许有能力打断主流程（2026-11-03 实测）**：`dirWatcherDiagEnabled()` 无保护读 `localStorage`，在 vitest（`localStorage is not available because --localstorage-file was not provided`）抛 `TypeError: Cannot read properties of undefined (reading 'getItem')`，直接把 `readDir` 打断，表现为 5 条「存量红」。`log*Diag` / `*DiagEnabled` 一律 try/catch + 先判 `typeof x === 'undefined' || x === null`，永不抛。

### 2.1 「比较并同步」的验收与清理（2026-10-22 新增）

- **比较太快时抓不到进度指示**（2026-10-22）：默认准则（时间 + 大小）下 10 万条目/侧整轮只要 **354 ms**，
  UIA 轮询根本来不及看到进度条。要取证**必须打开「按内容校验和」准则**，耗时才会拉到 **18 s**。
  ⇒ 「没抓到进度条」这时**不能**判成「进度条没实现」。
- **Radix 对话框在 UIA 里是 `[Window]`**，可访问名＝标题（「比较并同步」）。
  **必须把元素搜索限定在这个子树内**：不限定时位置启发式会点到对话框标题栏的**关闭按钮**，
  点完对话框消失，后续找不到「比较」按钮，症状是「脚本说找不到比较按钮」。
- **比较对话框的两个路径输入框可访问名＝label 文本**（「左侧目录」/「右侧目录」），有 ValuePattern 可读回。
  ⚠️ **粘贴前必须先 Ctrl+A**（输入框有初始值，否则是**追加**而不是替换）；粘贴后按 `END` 抑制内联补全；
  **读回值必须等于期望**才算填对。
- **「按内容校验和」这个 Radix Switch 没有可访问名**，靠「同一行右侧的小控件」定位（实测点击位置约 `(1886,694)`）。
- **compare 模块此前一行日志都没有**（整份应用日志只有 `drag_no_activate` 的噪音）。
  ⇒ 「查日志没查到错误」当时只能证明「后端没埋点」，**不能**证明「没出错」。
  要拿错误文本只能**造出现场复现**。本轮已补埋点（`compare started/collected/finished` + 跳过 WARN）。
- **验收靶子的清理**（本项目在用户家目录会堆几十万文件）：
  - `rm --` 会被安全层拦（提示 `requires the targets-only trash marker`），
    改用运行时的可恢复删除器：`<activeDataDir>\bin\mavis-trash.cmd -- "<路径>"`（接受 `-r` / `-f` 等 rm 风格参数）。
  - ⚠️ **20 万文件的大树用这个 wrapper 会报 `failed to trash`**，但**直接调底层 API 是成功的** ——
    真实错误被 CLIXML 吞掉，wrapper 的报错**不可信**。兜底写法：
    ```powershell
    Add-Type -AssemblyName Microsoft.VisualBasic
    [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory($p,'OnlyErrorDialogs','SendToRecycleBin')
    ```
  - **`icacls /deny` 造出来的拒绝访问目录，删除前必须先 `icacls <dir> /remove:d Duanyi`**，否则删不掉。
  - 清理完**核对「家目录里剩 0 个 + 回收站里 7 个都在」**，才算真清干净。

### 2.2 与 upstream 的同步（2026-10-23 实测）

- **AFM 与 upstream 已深度分叉**：merge base 是 `fed49cee`（2026-08-27）。AFM 领先 **251** 个 commit，upstream 领先 **23** 个。
- **别用 GitHub compare 页的「148 个文件」判断差距** —— upstream 改的 148 个文件里 **89 个 AFM 也改过**（真冲突面），只有 59 个是 AFM 干净的。
- **AFM 历史上做过两轮上游移植**（`86cf58ee` A 类「扩展安全加固」、`9e880451` B 类「连选/回收站确认/窗口状态/e2e 隔离」）。
  ⇒ **是重写移植、不是 cherry-pick**，所以 `git cherry` 会把 23 个 upstream commit **全判成「未移植」**。
  **在本项目用 `git cherry` 判「是否已合并 upstream」是错的**，必须用逐行命中率：
  ```powershell
  git -c http.proxy= fetch upstream main
  node scripts/audit-upstream-port.mjs        # 取每个 upstream commit 补丁的新增行，在 AFM HEAD 逐行命中
  ```
  判据：命中率 ≥ 90% ≈ 已落地（差的是注释/license 头/格式化）；< 60% ≈ 真落后。
- **`git -c http.proxy=` 的用法**：本机代理 `127.0.0.1:7897` 的 TLS 握手间歇性坏，
  直连 github.com 正常。**所有 git 远程操作都建议带这个参数**，避免误判成仓库出问题。
- ⚠️ **2026-10-28 实测推翻了上面这条「所有远程操作都带这个参数」**：`git -c http.proxy= push origin main`
  报 **`Recv failure: Connection was reset`**（直连被重置），而**走默认代理（`git push origin main`）一次就成功**。
  ⇒ **push 的正确姿势是「先带默认代理，失败再退回 `-c http.proxy=` 重试」**，不要固定用其中一个；
  fetch 与 push 可能走不通的方向相反，别把结论写死。
- **Node 的 `fetch` 不读 git 的代理配置**：Node 24 要加 `NODE_USE_ENV_PROXY=1` +
  `HTTPS_PROXY=http://127.0.0.1:7897`（或 `HTTP_PROXY`）才走代理；否则直连 Cloudflare 会
  `UND_ERR_CONNECT_TIMEOUT`（10 s 超时）。⚠️ 症状是**发版脚本最后一步报错、但上传其实已经成功** ——
  看报错位置，别一看到 exit 1 就重跑上传（会重复传 20 MB）。
  `gh` CLI 不受此影响（`gh api` 直连可用）。
- **upstream 的 23 个 commit 里，10 个已无合并价值**（命中率 92%–100%：
  SSRF 修复、扩展隔离加固、扩展主流程测试、Shift+方向键连选、双击标题栏最大化、
  全屏后窗口状态恢复、SMB 移入回收站前确认、e2e 全新 Windows profile、
  e2e 临时数据目录隔离、隔离目录删除重试）。
- **真正还没落地的是**：CI 的 `e2e-windows` job + e2e 残留数据检查（AFM 已有全套 e2e 脚本，
  但 `ci.yml` 里**完全没有 e2e job**）、locale JSON 末尾换行（AFM 的 `scripts/sync-i18n.js` 会把换行写掉）。
- **仪表盘重设计是产品决策、不是合并决策**：upstream 把合并基线上那个 609 行 / 18,853 字符的
  `dashboard.vue` 拆成 545 行 + 8 个新组件（标签侧栏/按类型筛选/缩略图预览/拖拽排序…）+ 2 个新 composable，
  但它**删掉了** `entry-card-tags.vue` 与 `dashboard-tag-section-header.vue`（AFM 正在用），
  且需要 `removeTagFromAllItems` / `removeTaggedItem` 两个存储函数（AFM 的 `user-stats.ts` 里没有），
  而 AFM 自己有 upstream 没有的**收藏分类**（AFM 的仪表盘 938 行 / 30,362 字符，比 upstream 旧版大 60%）。
  **建议按能力逐项移植，不要整体替换页面骨架。**
  已经被 upstream 证明「可低风险单独搬」的三块，本仓已落地：
  `src/utils/tagged-sections.ts`（分组逻辑）、`src/utils/page-drop-targets.ts`（拖放目标）、
  `src/utils/tag-colors.ts`（色板去重）。
  ⇒ 「只搬纯函数」这条建议是对的，但**要挑 AFM 已经内联了同一套逻辑的那几个**（搬过去等于去重 + 补测试），
  其余的搬进来就是**死代码**（没有 UI 入口就没人调）。

### 2.3 GitHub 仓库门面与公开素材（2026-10-24 新增）

- **对外公开的截图不能带用户真实工作区**（2026-10-24 踩到）：用户的目录里有
  `G1销售管理`、`订单数据` 这类业务目录名，一旦入镜就是公开泄露。
  且**目录树一旦展开某个盘根，该盘根的全部一级目录都会列出来**（C 盘根有 `办公文件`/`文件位置.xlsx`，
  D 盘根有 `gbrain-latest`/`paddleocr-*`/`ComfyUI`…），**都不可用**。
  ⇒ 正确做法：`subst X: <演示目录父目录>` + 标签页路径写成 `X:\AlphaFM-Demo`。
  应用解析不出 `X:` 的物理位置 ⇒ **真实盘全部保持折叠**，只有中性演示内容可见；
  用完 `subst X: /d` 解绑。截图脚本见 `.superpowers/sdd/afm-capture-only.ps1`（只抓不重启）
  与 `.superpowers/sdd/hero-shot2.cjs`（建演示目录 / 写单标签 workspace / 恢复）。
- **`LICENSE.md` 必须是许可证的规范原文**（2026-10-24）：GitHub 侧栏的许可证探测（licensee）
  要求正文是原文；开头加一段自写的「TL;DR 摘要」就会被打断 ⇒ 侧栏显示 **NOASSERTION**，
  第一眼像「没写许可证」。自写摘要要单独放 `NOTICE.md`，README 的 License 段同时指向两个文件。
- **本仓在 GitHub 上是 upstream 的 fork**（`fork: true`，parent = `sigma-file-manager`）：
  ① 部分 awesome 榜单**明文不收 fork**；② `0PandaDEV/awesome-windows` 的 README 第 5 行
  **明文禁止 AI 生成的 PR** ⇒ 该仓库**主动跳过**，不要尝试。③ 保留 fork 还能蹭到上游 6.7k star
  的社会证明，所以暂不 Duplicate 成独立仓库。
- **这个 fork 上的 GitHub Actions 从来没跑过（2026-10-24 实测）**：
  `GET /repos/kizemo/alpha-file-manager/actions/runs` 返回 `total_count = 0`，
  5 个 workflow（ci / msix / release / sign-windows / winget）全是 `active` 但**一次都没触发过**；
  `GET .../actions/permissions` 却返回 `{"enabled":true,"allowed_actions":"all"}` ——
  **「enabled」不等于「会跑」**，public fork 的 Actions 默认不启用，`check-runs` 也是 0。
  ⇒ 后果：① `.github/workflows/winget.yml`（`on: release: [released]`）**永远不会触发**，
  所以**每一次 winget 提清单都必须手工开 PR**，不要指望发版自动提交；
  ② 该 workflow 自己还缺 `WINGET_TOKEN` secret（仓库 secret 列表为空），
  就算 Actions 通了也是第一步就失败；
  ③ `ci.yml` 也从没跑过 ⇒ **GitHub 上没有任何 CI 信号**，门禁只能靠本机跑。
  ⚠️ 判断「发版有没有自动进 winget」时，不要去看 workflow 文件存不存在，
  要看 `actions/runs` 的 `total_count`。

- **提交 awesome 榜单前必须先读它的 `contributing.md`**（2026-10-24）：
  `Axorax/awesome-free-apps` 只让改 `README.md`/`MOBILE.md`、**不许动 `filter/`**（自动生成）、
  要加在**分类末尾**、commit message 用 `Add: 名称`；
  `chrysler-stone/awesome-tauri-apps` 要**分类内按字母序**。
  平台图标（🪟 等）**从它自己的图例行里复制**——手写代理对会变成别的字符，
  而 `🟢` 是**带链接**的（`[🟢](仓库地址)`），不是裸字符。
- **`gh` CLI 在 PowerShell 5.1 下的三个坑**（2026-10-24）：
  ① `--jq` 表达式里带双引号会被 PS 吃掉 ⇒ 一律用 `node` 解析 JSON；
  ② `--input -` 走管道会破坏 JSON ⇒ 把载荷写成 UTF-8 文件再 `--input <file>`；
  ③ PR 的 API 路径是 `repos/{owner}/{repo}/pulls/{n}`，不是 `repos/{owner}/{repo}/{n}`。
- **冷启动要 3 分钟**：`afm-nav-accept.ps1` 里 90 秒的等窗口上限不够，
  表现为「AFM main window never appeared」，但应用其实好好地起来了
  ⇒ 用 `afm-capture-only.ps1` 只抓不重启。

### 2.4 winget 发布与 GitHub 素材（2026-10-24 新增）

- **winget 清单不需要带 `winget` scope 的令牌**（实测）：那个 scope 只给
  「Submit new package」工具用，**直接开 PR 就行**（上游 CONTRIBUTING 明写
  「routine manifest submissions do not need a separate issue」）。
  流程 = fork `microsoft/winget-pkgs` → 从 `master` 拉分支 → 提交清单 → 向 `master` 开 PR；
  上游的 GitHub App 会自动跑校验流水线（下载安装包核对哈希、跑 `winget validate`、沙箱试装）。
- **目录结构是 `manifests/<发布者首字母小写>/<发布者>/<应用名>/<版本>/`**，一个版本一个目录，
  里面四个文件：`<包 ID>.yaml`（version）、`<包 ID>.installer.yaml`（installer）、
  `<包 ID>.locale.en-US.yaml`（**defaultLocale**）与 `<包 ID>.locale.zh-CN.yaml`（**locale**）。
  ⚠️ **同一个版本目录里，每种 `ManifestType` 只允许一份文件**（2026-10-24 实测踩中）：
  两份 locale 文件都写 `defaultLocale` ⇒ 上游判 `DuplicateMultiFileManifestType`
  （"The multi file manifest should contain only one file with the particular ManifestType"），
  校验在第 02 步就停掉。规则出处：`microsoft/winget-cli` 的
  `src/AppInstallerCommonCore/Manifest/YamlParser.cpp` —— `Version` / `Installer` / `DefaultLocale`
  各有「已找到」标志位，重复即报错；`Locale` 分支只查 `PackageLocale` 重复
  （错误码 `DuplicateMultiFileManifestLocale`），**允许多份**。
  另外 `version` 文件的 `DefaultLocale` 必须等于 `defaultLocale` 文件的 `PackageLocale`，
  否则判 `InconsistentMultiFileManifestDefaultLocale`。
  ⚠️ **两个 locale schema 的字段表不一样**（`aka.ms/winget-manifest.*.1.12.0.schema.json`）：
  `defaultLocale` 必填 `Publisher` / `PackageName` / `License` / `ShortDescription`，
  `locale` 只必填 `PackageIdentifier` / `PackageVersion` / `PackageLocale` / `ManifestType` / `ManifestVersion`；
  而且 **`Moniker` 只出现在 `defaultLocale` 的字段表里**，`locale` 的字段表里没有
  ⇒ 完整元数据（`Publisher`/`PackageName`/`License`/`Moniker`…）一律留在 `defaultLocale` 文件，
  `locale` 文件只放翻译过的 `ShortDescription` / `Description` / `Tags`。
  对照上游真实多语言包：`manifests/m/Microsoft/AKSdesktop/0.43.0/`。
  ⚠️ **中间那层是「包 ID 按点拆开后的发布者段」，不是包名**：
  `kizemo.AlphaFileManager` ⇒ 发布者层 `kizemo`、应用层 `AlphaFileManager`；
  包名 `Alpha File Manager`（带空格）**不能**当目录名。
  对照组（同期已被受理的 PR）：`manifests/j/Justsenger/ExHyperV/1.5.2/Justsenger.ExHyperV.yaml`、
  `manifests/o/OpenAI/ChatGPT/26.1021.3705.0/OpenAI.ChatGPT.yaml`。
  ⚠️ **上游的发布者目录名保留原始大小写**（`manifests/m/Microsoft/...`，不是 `manifests/m/microsoft/...`），
  拿上游目录当模板时别自己小写化，否则 contents API 直接 404。
  ⚠️ **踩过的坑（本轮 PR 被机器人打回 "missing the manifests folder"）**：
  用 contents API 提交时，若相对路径是**以 `.../winget/manifests` 为基准**算出来的，
  `manifests/` 这层根目录会被吃掉，文件落到仓库根下的 `k/...` —— 机器人直接判缺目录。
  ⇒ **相对路径必须以包含 `manifests/` 的那一层为基准**。
  schema 用 **1.12.0**（PR 模板指定的版本；仓库里虽然已有 1.28.0，模板仍指向 1.12）；
  PR 标题格式 `New package: <包 ID> version <版本>`。本仓的原文存在 `docs/winget/`。
- **清单里的 `Publisher` 必须等于安装器写进 Windows ARP 的发布者**，否则包管理器
  不认得你机器上装的是清单里那个程序，升级也就不会接管。来源链：
  `tauri.conf.json` 的 `bundle.publisher` → NSIS 模板 `!define MANUFACTURER` →
  `WriteRegStr ... "Publisher"`。
  ⚠️ **Tauri v2 的 `bundle.publisher` 默认取 identifier 的第二段**
  （本仓 identifier `com.sigma-file-manager.app` ⇒ 默认 `sigma-file-manager`），
  所以想写 `kizemo` 必须**显式**设 `bundle.publisher`；
  一改就**必须重新打包 + 重新发版**（哈希变、ARP 发布者变），再提交清单。
- **本机没有 `winget` 命令行**：App Installer 装了（1.29.379.0），但 `winget.exe` 的
  别名没注册，且从 `C:\Program Files\WindowsApps\...` 直接执行报 **Access is denied**
  ⇒ 本地 `winget validate` / `winget install --manifest` 跑不了，只能交给上游流水线。
  **安装包也没有代码签名**（`Get-AuthenticodeSignature` ⇒ `NotSigned`），
  上游评审通常会要求签名 ⇒ 需要发布者自备证书。
- **winget 的 CLA 是在 PR 里回一行命令，不是在网页上签**（2026-10-24 实测通过）：
  机器人 `microsoft-github-policy-service` 会在 PR 里留言，要求回复
  `@microsoft-github-policy-service agree`（公司形式：`agree company="<公司名>"`）。
  回复后 `license/cla` 这个 check 变 success、摘要 "All CLA requirements met."。
  ⚠️ **两种形式是法律声明，含义不同**（个人独立拥有知识产权 vs 在为雇主工作过程中提交），
  **只能由账号本人决定**，agent 不得替用户选；代发用
  `gh pr comment <n> --repo microsoft/winget-pkgs --body '@microsoft-github-policy-service agree'`
  即可（运行时令牌的身份就是 PR 作者）。**同一个 GitHub 账号对 microsoft 各仓库只需签一次**。
- **winget PR 的状态看标签，不看 check runs**（2026-10-24 实测）：
  校验 App 在一次 operation 结束后会**把自己的 10 条 check run 删掉** ⇒
  「没有 check run」往往意味着**已经跑完了**，不是没跑。真正的状态在 PR 标签上：
  `Validation-Completed`（通过，常同时有 `Azure-Pipeline-Passed`）、`Validation-Executable-Error`
  （校验脚本自身出错）、`Validation-Installation-Error`（沙箱试装失败）、
  `Validation-Defender-Error`（安装包被 Defender 拦）、`Validation-Guide`（给了你指引）、
  `Needs-Author-Feedback` / `Needs-Attention`（要你回复）。
  **重跑校验只有两条路**：① 版主在 PR 里发 `@wingetbot run`（**非版主发会被拒**：
  机器人回 "Commenter does not have sufficient privileges"）；
  ② **推一个新的 commit**（`synchronize` 事件）——但**不能是空提交**：
  空提交会被策略机器人判成 `noContent`（"This pull request does not update any files"）。
  ⇒ 想重跑就往清单里塞一行**真实的注释**再推。
  ⚠️ 校验服务本身也会抽风：2026-10-24 我们的 operation 报
  `Internal-Error-PR` / `WinGetSvcError`（03–09 步全 skipped、10 步 neutral），
  发生在分支正在连续删/加文件的半路上 —— **operation 是在 diff 还没稳的时候算的**。
  ⇒ **一次推到位再让它跑**（别一边改一边让它验），确实报错了就用上面的 ② 重推。
- **上游现在的校验是「Agentic Workflow」**（2026-10-24 实测）：
  `.github/workflows/*.lock.yml` 由**打标签**触发（`pull_request_target: types: [labeled]`），
  真正的清单校验由 `wingetvalidator-prod[bot]` 这个 App 跑；新包会先被自动打上
  `Project-File` 标签，校验随后才开始（要下载安装包核对哈希 + 沙箱试装，十几分钟起步）。
  ⇒ **PR 提交后没有立刻出现校验结果属正常**，别急着重提。
- **第 08 步「沙箱试装」可能跑 35 分钟以上，别当卡死**（2026-10-24 实测）：
  本项目 3.5.1 那轮从 07:10 跑到 07:45，而同期其它包样本约 10 分钟。
  根因：`tauri.conf.json` **没配 `webviewInstallMode`**，Tauri v2 默认
  `downloadBootstrapper` ⇒ 沙箱机器没装 WebView2 运行时时，安装过程要联网下载
  引导程序再装运行时（~130MB）。**是安装耗时，不是挂起。**
  ⚠️ 取舍：`offlineInstaller` ⇒ 安装包多 ~130MB；`skip` ⇒ 目标机没 WebView2 就跑不起来。
- **校验全绿之后，剩下的是社区版主人工评审**（2026-10-24 实测）：
  机器人会留言 "The check-in policies require a moderator to approve PRs from the
  community… please be patient"。已合并的上游 PR 标签是
  `Validation-Completed` + `Azure-Pipeline-Passed` + `Moderator-Approved` + `New-Package`
  —— **`Moderator-Approved` 是版主加的，我们加不了也不该加**；`@wingetbot run` 同理。
  合并前要做的准备：**先签 CLA**（账号级，签一次对 microsoft 各仓库通用），
  否则会一直挂着 `Needs-CLA` 标签。
- **孤立的 CR 字节是一颗地雷**（2026-10-24 实测，代价是一轮打包白跑）：
  `src-tauri/src/extensions/http.rs` 有一行注释中间夹了个裸 `0x0D`，
  rustc 把 CR 当空白 ⇒ 整行仍是注释、能编译；
  但**任何规范化行尾的写入路径**（编辑器、带 eol=crlf 的 `git checkout`、文本编辑 API）
  都会把它变成真换行 ⇒ 注释后半句变成裸代码 ⇒ **整个 crate 编译失败**
  （本轮 10 个错误，`extensions/github.rs` / `extensions/filesystem.rs` 的 E0277 全是级联报错）。
  ⇒ **改任何 `.rs` 之前先扫孤立 CR**（`git show HEAD:<file>` 取原始字节比对），
  修复用 node 按字节改，**别用会规范化行尾的写入路径**。
  全仓扫描结果：只有这一个文件（1369 个受版本控制的文本文件）。
- **写工具会把一个汉字写成 3 个 U+FFFD**（2026-10-24）：症状是文档里出现「双」后面跟三个 U+FFFD 替换符
  （应为「双语」）—— 一个 3 字节 UTF-8 字符被拆成三个无效字节。
  ⇒ 凡是写了中文的文本文件，**写完必须验** `(s.match(/\uFFFD/g)||[]).length === 0`；
  但**有些文件里的 U+FFFD 是故意的**（`src-tauri/src/dir_reader/network_shares.rs`
  与 `docs/superpowers/plans/2026-10-15-handoff-batch4-restart.md` 就在正文里举例乱码），
  扫描时先排除它们再判定。
- **`tauri build` 报 `failed to remove file ... os error 5`**（2026-10-24）：
  是**上一轮残留的应用进程**占着 `src-tauri/target/release/alpha-file-manager.exe`。
  release 构建下 `SFM_E2E_DATA_DIR` 被编译掉 ⇒ 那个进程一退就会把内存里的统计状态
  写回真实 `user-stats.json` ⇒ **打包前先确认 `Get-Process alpha-file-manager` 为空**。
  ⚠️ **这个失败点出现在整个编译的最后一步**（Rust 全部编完、准备把可执行文件替换掉时），
  所以代价是**十几分钟的 Rust release 编译全部白跑**，2026-10-24 发 3.6.0 时就白跑了一轮。
  ⇒ **把清理做成构建脚本的前置守卫**，别靠人工记得：
  ```cmd
  powershell -NoProfile -Command "Get-Process alpha-file-manager -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep -Seconds 2; if (Get-Process alpha-file-manager -ErrorAction SilentlyContinue) { exit 1 } else { exit 0 }"
  ```
  ⚠️ 另有一条：**别把「进程检查 + 目录列举 + 进程列举」塞进同一次 bash 调用**。
  本轮那次调用**整段输出是空的**（连本该列出 39 个安装包的 `Get-ChildItem` 都没输出），
  于是「没查到 alpha-file-manager」被当成了「没有残留进程」——实际**有一个 explorer 双击启动的实例在跑**。
  ⇒ 进程检查**单独跑一次、单独看输出**，并在脚本里再做一次前置守卫兜底。

- **HelloGitHub 的提交表单没有独立路由**：导航里的「提交项目」是**客户端弹出的模态框**；
  `/submit` 一类路径全是 404，真实存在的只有 `/report/contribution`（那是**贡献排行榜**，不是表单）。
  登录入口是侧栏的「立即登录」与 `/user/login`（会再跳回首页）。
  ⚠️ **自动化点击超时 ≠ 点不动**：本轮 `click` 在页面上连续 275 秒超时、且点击根本没派发，
  当时我记的结论是「这个站点点不了」——**这是错的**。真因是**页面还停在骨架屏状态**：
  元素一直在变，工具的「元素是否稳定」前置检查就永远过不去。
  等页面稳定后（判据：`inspect` 只要 ~100ms 就返回，而超时前它是 275 秒），
  同一个 `click` 只用 **600 毫秒**就成功，模态框正常弹出、提交也成功（POST 返回 200）。
  ⇒ **判别方法**：先 `inspect` 一次，`durationMs` 上百毫秒以内就说明页面已稳定，可以点；
  超时则说明页面还在抖，先让页面加载完（或改用 `query` 读内容）再点。
  ⇒ **读页面用 `query`（text/semantic，100–200ms）**，比 `inspect` 稳，也比它便宜。
- **Markdown 表格只有两类是真缺陷，判据照 GFM 规范**（2026-10-24 实测）：
  规范原文（`github/gfm` §4.10 Tables）：
  > The header row must match the delimiter row in the number of cells. If not, a table will not be recognized.
  >
  > The remainder of the table's rows may vary in the number of cells. If there are a number of cells fewer than the number of cells in the header row, empty cells are inserted. If there are greater, the excess is ignored.

  ⇒ ① **分隔行 ≠ 表头** ⇒ 整块**不被识别成表格**，塌成一段话（用户第一次报的那个 README 缺陷）。
  ⇒ ② **数据行比表头多列** ⇒ 多出来的列被**静默丢弃**（表还在，内容没了）。
  ⇒ **数据行比表头少列不是缺陷**（GFM 会补空单元格）—— 早先的门禁把它算成错误，那是误报。
  ⚠️ 单元格正文里的 `|` 必须写成 `\|`，否则那一行会被拆成多列，多余列直接被丢。
  已有机器门禁：`src/utils/__tests__/readme-markdown-table.test.ts`
  （覆盖根 README + `docs/**/*.md`，判据与 GFM 对齐，77 条用例，含负向用例）。
- **winget 上游实况（2026-11-12 现场复查，覆盖此前全部结论）**：
  ⚠️ 此前写的「当前 PR 是 `#460744`、状态 `OPEN`、只差版主点合并」**已过期** ——
  `#460744`（3.5.1）**由我们自己于 2026-11-01 关闭**（留言「Superseded by the 3.9.0 submission」），
  **从未合并**（`mergedAt = null`）；真正在审的是 **`#467381`（3.9.1），至今 `OPEN`**。
  上游 `manifests/k/kizemo/AlphaFileManager` 目录 **404 ⇒ 这个包从未入库 winget**。
  ⇒ **当前唯一阻塞点是 `#467381`，不是 `#460744`**。
  ⚠️ **查阻塞点只能用** `gh pr list --repo microsoft/winget-pkgs --author kizemo --state all --limit 20`；
  **用「上游 `manifests/` 目录 404」反推「没提过 PR」是错的** —— 404 只说明**没合并入库**，不说明没提交过。
- **CLA 签没签要看 check-run `license/cla`，不能只看机器人留言**（2026-10-26 实测）：
  策略机器人留言 "This PR cannot be merged until you sign the CLA" 之后，
  回一句 `@microsoft-github-policy-service agree` 就会让它变 success；
  但**那条留言会一直留在 PR 里**，只读评论会误判成「还没签」。
  查法：`gh api repos/microsoft/winget-pkgs/commits/<head sha>/check-runs` 里找 `license/cla`。
  ⚠️ PowerShell 里 `gh api ... --jq '... | ...'` 会被拆成多个参数，报
  「accepts 1 arg(s), received 5」⇒ 改用 `| ConvertFrom-Json` 在 PowerShell 侧取字段。
- **winget 提 PR 的排队规则不变：等前一个 PR 合并后再提下一版**（2026-10-26 决定，2026-11-12 复核仍适用）：
  同一个包 ID 同时只允许一个 PR ⇒ 在 `#467381`（3.9.1）合并之前，3.14.0 的 PR 提上去大概率被要求「先关旧 PR」。
  ⇒ **要提的是 3.14.0**：清单四件套已入库 `docs/winget/manifests/k/kizemo/AlphaFileManager/3.14.0/`
  （`InstallerSha256` 指向**已发布资产** `0D046209…5A3E6` / 45451606 B；`ReleaseDate: 2026-11-11`，按 UTC 写）。
  旧的 `3.6.0/`（提交 `3a87a0f1`）与 `3.6.1/` 清单留在仓库**备查不删**，但都已作废（3.6.x 从未提交上游）。
  逐步流程见 `docs/winget/README.md`。
  ⚠️ **上游不校验 PR 标题**（`doc/Authoring.md` / `doc/Policies.md` 都没有标题规范，
  机器人只管清单内容）⇒ 版本更新用 `Update kizemo.AlphaFileManager to 3.14.0` 即可。
- **winget 清单的三个硬约束**（2026-10-25 实测，不是猜的）：
  ① **`ManifestVersion` 旧版本照收不误** —— 上游当前基线已是 `1.28.0`，
  但抽样最近合并的 6 份 PR 清单，**仍有 2 份用 1.12.0、1 份用 1.6.0 在合并**
  ⇒ 沿用已被 3.5.1 验证过的 `1.12.0` 风险最低，不必追新。
  ② **`Tags` 上限 `maxItems: 16`** —— 而本项目 3.5.1 的标签**正好已经 16 个**
  ⇒ 想加新功能的标签必须先删一个（删的是与 `file-manager`/`file-explorer`
  语义重复的 `file-management`，换上 `file-unlock`）。
  ③ **`ShortDescription` ≤256 字符 / `Description` ≤10000 / Tag ≤40 字符**，
  限值取自 `https://aka.ms/winget-manifest.defaultLocale.1.12.0.schema.json`。
- **本机没装 `winget` CLI**（`Get-Command winget` 为空），所以「`winget validate` +
  沙箱试装」这两项只能交给上游流水线（一轮十几分钟起步）。
  为此写了**零依赖**的本地替身：`docs/winget/check-manifest.mjs`，
  跑 `npm run check:winget`。
  它核对四种 `ManifestType` 各一份、`DefaultLocale` 与 `PackageLocale` 一致、
  locale 文件不重复声明 defaultLocale、`Moniker` 不外泄、长度上限、
  以及 `InstallerSha256` 与**本地安装包实际字节**一致。
  ⚠️ 这个脚本本身要验过才有意义：2026-10-25 用**阴性对照**验过 ——
  故意改坏 7 处（两种 `ManifestType` 写反、locale 里塞 `Moniker`、标签超限、
  哈希改错等）会被逐条抓出，其中两种 `ManifestType` 写反正是 3.5.1 踩过的
  `DuplicateMultiFileManifestType`。**写完检查脚本一定要造一份坏数据试它**。

### 2.5 「解除文件锁定 / 彻底删除」的验收坑（2026-10-24 实测）

功能本体见 `docs/design-2026-10-24-file-unlock-and-force-delete.md`。
下面是**验收脚本**踩到的坑，复用于后续任何右键菜单验收：

- **同一个文件名在 UIA 树里会出现三处**：左侧目录树（纯 `Text`，常常滚到屏幕外）、
  右侧信息面板的标题（纯 `Text`，名字就是文件名）、文件列表行（**`Button`**，
  可访问名是「文件名 — 大小 相对时间 日期」）。
  ⇒ 认文件列表行要同时满足三条：**ControlType 是 `Button`** + **名字比裸文件名长** +
  **矩形落在窗口内**。只按名字找会点到目录树里的同名项（它的坐标常常在屏幕外，
  点下去什么也不会发生），或点到信息面板标题（坐标在更靠右的位置）。
- **Radix 右键菜单是 toggle 的**：菜单已经开着时再右键会**把它关掉**，
  不是「重新打开一个」。
  ⇒ 脚本必须「右键 → 检查目标菜单项在不在 → 不在就再右键一次」，
  而且**右键与点击菜单项必须在同一次脚本运行里**（换进程会带走焦点、关掉菜单）。
- **`InvokePattern.Invoke()` 对 Radix 菜单项不一定生效，对对话框里的按钮生效**。
  菜单项优先走「按元素中心 PostMessage 真实点击」，
  对话框按钮可以直接 Invoke。
- **「占用者」类夹具的持锁进程会自己到期**（sleep 结束就退出）。
  到期后应用走的是**另一条分支**（没有占用者 ⇒ 直接解锁、不弹对话框），
  症状看起来像「功能坏了」或「注入没进去」。
  ⇒ 持锁时长要**远长于**验收脚本的运行时长（本次用 7200 秒，
  第一次用 900 秒就踩到了）。
- **release 构建下 `SFM_E2E_DATA_DIR` 不生效** ⇒ 真机验收必须走
  「备份 → 灌夹具 → 验收 → 恢复」，且**先杀进程再 restore**
  （应用退出时会把内存里的统计写回真实 `user-stats.json`）。
  恢复后要**核对 SHA256**，不是只看字节数。
- **本功能的第三层兜底会往机器的 `PendingFileRenameOperations` 里写一条**：
  `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager`。
  这是**机器级**的值，不属于本项目 ⇒ 验收结束必须把**自己那几条**摘掉，
  否则用户下次重启会静默删掉夹具文件（连 Rust 单测造的临时文件也会进这个表）。
  清理时按「源路径 + 紧随其后的空目标」成对删除，别碰别人的条目。

### 2.6 出站拖拽（拖到微信/资源管理器）的触发条件与验收（2026-10-25 实测）

用户报的现象：AFM **最大化** + 聊天窗口前置时，长按拖文件到聊天窗口，
AFM 确实不抢前台（聊天窗口还在前面），但**文件拖不进聊天窗口发送**。

- **根因不是「回退」**：拖拽相关代码自 2026-10-21 起**一字未改**。
  是**从未被验收覆盖的设计缺口** —— 10-21 的验收脚本
  （`accept-drag-out.ps1` / `drag-lab.ps1`）一律把 AFM 缩成 1500x760 /
  1500x1400，**从未测过最大化**。
  ⇒ **验收矩阵里必须显式列出窗口尺寸**，「缩小窗口测过了」≠「测过了」。
- **出站 OLE 拖拽原先只有一个触发条件：「指针移出了 AFM 的视口」**
  （`use-file-browser-drag-session.ts` 的 `isCursorOutsideViewport`）。
  ⇒ **AFM 一最大化这条就永远不成立**：视口铺满全屏，指针停在盖在上面的
  聊天窗口上时坐标仍在视口内 ⇒ 不切出站拖拽 ⇒ 抬手时 `targetPath` 为空 ⇒
  `cancelDrag()` ⇒ 文件拖不进去。
- **修法**：后端加探针命令「指针当前是不是停在外部窗口上」
  （`GetCursorPos` → `WindowFromPoint` → `GetAncestor(GA_ROOT)`），
  前端在内建拖拽会话里**定时**问它（50ms 一次），**连续命中 2 次**才切出站。
  用定时器而不是 `mousemove`：指针压在别的程序上时 **webview 收不到 mousemove**。
- **判定「自家窗口」必须用 `IsChild(主窗口, 命中窗口)`，不能只看 pid**：
  AFM 的 webview 渲染窗口（`Chrome_RenderWidgetHostHWND`）跑在
  **`msedgewebview2.exe`** 进程里，但它们**是主窗口的后代**
  （`WRY_WEBVIEW` → `Chrome_WidgetWin_0` → `Chrome_WidgetWin_1` → 渲染窗口，
  实测主窗口底下挂着 30 个这样的后代）。
  ⇒ 只按 pid 判会把自家页面当成外部窗口 ⇒ 在 AFM **内部**拖文件时误起
  系统级拖拽，**内建拖放直接坏掉**。这条判错比原故障更严重。
- **判错的两个方向都要想到**：把外部窗口当成自家 ⇒ 出站拖拽起不来（原故障）；
  把自家窗口当成外部 ⇒ 内部拖放被系统级拖拽抢走。
- **长按路径的 6 CSS px 容差不要动**：改容差会改变「按住不动」的语义，
  容易引发误触发。修复以「指针在外部窗口上」为准。
- **内建→出站的切换路径原先漏了两步**：`captureOutboundDragTarget()` 与
  `beginOutboundDragActivationWatch()` 只在长按路径里调过 ⇒ 切过去时
  目标窗口不会被提到前面、前台也不归还。
  ⇒ 出站拖拽的完整生命周期已收进 `startOutboundDragForEntries()`，两条路径共用。
- **验收判据看应用日志**（发布版没有 devtools、wry 不透传 CDP）：
  `pointer probe external=true` / `outbound-drag-started` /
  `captured outbound drag target` / `activated drop target`。
  探针只在**答案变化时**打日志，否则 50ms 一次会把日志刷爆。
- 验收脚本：`.superpowers/sdd/accept-drag-out-maximized.ps1`
  （`-Mode Layout|External|Internal`；`Internal` 是阴性对照：
  在自家窗口内拖到文件夹上**不得**起系统级拖拽）。
- **探针类逻辑必须做变异验证**：本轮故意断开触发（前端）、故意漏掉
  「后代」判定（Rust），确认对应用例会红 —— 否则「全绿」只是没接线的检查器。

### 2.7 发版：tag 必须落在「发版提交」上，且发版后要核对（2026-10-25 实测）

3.6.1 发版后复核时发现的：**GitHub release v3.6.1 的 tag 指向 `3a87a0f1`**，
而那份提交的 `package.json` 是 **3.6.0**，并且**不含本次拖拽修复**（修复在 `3da37f90`，
是 tag 目标的后一个提交）。

- **根因**：tag 在会话早期（约 10:02）就建好了，**早于修复代码落地**；
  而 `gh release create` 看到 tag 已存在就**直接复用**，不会检查它指向哪。
  ⇒ 发版脚本（`scripts/release-publish.mjs`）里**没有任何 `git tag` 动作**，
  tag 全靠人手工建 ⇒ 顺序错了没人拦。
- **后果范围**：安装包本身**完全正确**（资产 sha256 与本地构建、站点 `version.json`、
  winget 清单四方一致），错的只是 **GitHub release 页的「Source code (zip/tar.gz)」**
  ⇒ 从 tag 拉源码拿到的是「没有这次修复的 3.6.0」。
  更新通道（站点 `version.json` / release 资产 / winget `InstallerUrl`）都按资产走，**不受影响**。
- **正确约定**：**tag = 发版提交**（版本号已 bump、CHANGELOG 已写、代码已全在的那一个）。
  ⇒ 建 tag 必须在**发版提交之后**，不能在动手改代码之前就占坑。
- **发版后必查三条**（`git rev-list -n1 vX.Y.Z`）：
  ① tag 目标提交的 `package.json` 版本 == 发布版本；
  ② tag 目标包含本次修复提交（`git merge-base --is-ancestor <修复提交> vX.Y.Z`）；
  ③ `git show vX.Y.Z:<修复文件>` 里能 grep 到本次新增的标识符。
  不满足就 `git tag -f vX.Y.Z <发版提交> && git push origin vX.Y.Z --force`
  （**release 资产不受影响**，资产挂在 release 上，不挂在 tag 的提交上）。
- **历史两个 tag 是「另一种」错，别照抄也别急着动**：`v3.5.1` / `v3.6.0`
  都指向「发版提交的前一个」，功能代码都在，只差版本号与 CHANGELOG 条目。
  ⇒ 判据是「tag 里有没有这次发布的**功能**」，不是「版本号对不对」。
- **发布完不等于发版结束**：3.14.0 的 winget 清单四件套已备好，但**必须等前一个 PR 合并后才能提**
  （同一个包 ID 同时只允许一个 PR，并行提交会被要求「等前一版合了再提」）。
  当前在审的是 **3.9.1 的 PR `#467381`（`OPEN`）**，卡在**上游版主人工评审**（社区志愿者，无 SLA）
  ⇒ **不在我们控制范围内，只能等**；要提的是 **3.14.0**（见上文 winget 小节）。
  ⚠️ `#460744`（3.5.1）已于 2026-11-01 由我们自行关闭、**从未合并**，**别再把它当阻塞点**。

### 2.8 AFM 真机验收必须跑在隔离桌面（沙箱）里（2026-10-28 新增）

**用户原话**：「希望对AFM的测试验收，可以放在沙箱内，避免提升AFM焦点时对电脑使用造成影响，也避免电脑正在使用，影响到调试。」

**这是硬约束，不是建议。** 只要脚本会启动 AFM 本体（`alpha-file-manager.exe`）做验收，就不许在用户的交互桌面上抢焦点。

- **模板**：`.superpowers\sdd\sandbox-c1-accept.ps1`（纯 ASCII，跑 `powershell.exe` 5.1）。
  诊断用：`.superpowers\sdd\diag-desktop.ps1`。
- **沙箱怎么建**：注入端**不要切换自己的桌面**（切了会破坏用户侧的输入状态，也可能被 UIPI 拦住）。
  用 `CreateDesktop` 建独立桌面 → `CreateProcess` 时把 `STARTUPINFO.lpDesktop` 指向它 →
  注入端留在用户桌面，只用 `EnumDesktopWindows(hDesktop, ...)` 枚举沙箱窗口。
  ⚠️ **`EnumWindows` 枚举的是「调用线程所在桌面」的窗口**，不切桌面就必须用 `EnumDesktopWindows`。
- **`CreateDesktop` 的访问掩码必须给 `DESKTOP_ALL (0x1FF)`**：只给 `DESKTOP_SWITCHDESKTOP (0x100)`
  会失败并返回 **win32=8（ERROR_INVALID_HANDLE）**，症状看着像「这台机器不支持沙箱」（2026-10-28 实测）。
- **沙箱验收前必须先结束正在运行的 `alpha-file-manager`**：release 构建里 `app_dirs::is_isolated()` **恒为 false**
  （`SFM_E2E_DATA_DIR` 覆盖被 `#[cfg(debug_assertions)]` 编译掉）⇒ `tauri_plugin_single_instance` 一定会装。
  沙箱实例会秒退并把启动转交给旧实例（症状：日志说「已启动」，但沙箱桌面里没有窗口）。
- **沙箱里没有键盘输入，别在沙箱里假装验过键盘**：
  `SendInput` 只能注入到**输入桌面**（用户正在看的那个）；`SetKeyboardState` 只能改**调用线程自己**的状态；
  Chromium 的 Ctrl/Shift 取自 `GetKeyState`，PostMessage 伪造的按下状态不生效。
  ⇒ **沙箱内只能做鼠标等价断言**（PostMessage 鼠标 + UIA + `PrintWindow` 截图）；
  键盘快捷键路径必须另找证据（读代码分派 + 找状态可观测量），并在结论里写明「键盘路径未在沙箱内取证」。
- **鼠标注入**：投给 **`Chrome_RenderWidgetHostHWND`**（不是 `Chrome_WidgetWin_0`），
  坐标是**该窗口的客户区坐标**（`screen - hostRect.Left/Top`）。
  切换型控件（toggle）先探测元素是否存在再点，否则会把面板点关。
- **截图取证**：`PrintWindow(hwnd, hdc, 2)` 在沙箱桌面照样能出图（离屏渲染，不依赖屏幕合成）。
- **UIA 跨桌面可用**：`AutomationElement.FromHandle` 能读沙箱窗口的元素树（2026-10-28 实测可用），
  因此「渲染了多少行」「统计文案」「左栏选择徽标有没有消失」都能在沙箱里断言。
- **验收脚本只认「阳性对照 + 事实来源」**：先点一个必然有可见后果的元素确认注入生效，再去点目标；
  复制结果要看**剪贴板 CF_HDROP 条数**（`Get-Clipboard -Raw` 只读 CF_UNICODETEXT，读不到文件拖放格式）。

### 2.9 NSIS 安装器守卫与「静默退出」的三个硬事实（2026-10-29 实测）

- **NSIS 的 `StrCmp` 不支持 `*` 通配**：实测 8 种形态一次都没匹配上（`$CMDLINE` 内容
  确实是 `"C:\...\setup.exe" /FORCE`）。⇒ **不要在 NSIS 里用字符串通配匹配命令行参数**。
- **安装器进程里 `$1..$9` 是空的**：NSIS 只在**卸载器**里把命令行参数放进 `$1..$9`。
  ⇒ 安装器要拿自己的命令行，只有 `$CMDLINE` 这一个来源，且它**带双引号**，
  直接插进 `nsExec` 的命令行会打破引号结构 ⇒ 走**文件**交接：
  `FileOpen $9 "$PLUGINSDIR\xxx.txt" w` + `FileWrite $9 "$CMDLINE"`（**原样写出，无换行污染**）。
  判定逻辑（正则等）放在 **PowerShell 侧**——那一侧有单测覆盖，坏了会被发现。
- **32 位进程读注册表要用 `OpenBaseKey(hive, view)` + 单参 `OpenSubKey`**：
  安装器是 32 位（注册表被重定向到 `Wow6432Node`），`OpenRemoteBaseKey` + 3 参
  `OpenSubKey` **在 32 位进程里基键已被重定向、无法再切视图**，永远读不到值。
  ⇒ 遍历 HKLM/HKCU × Registry64/Registry32 两个视图。
  ⚠️ 剥离 `Software\` 前缀必须用 `-replace '^Software\\', ''`，**不能用 `Substring(8)`**
  （`Software\` 长度是 9，会拼出双反斜杠路径）。
- **PowerShell 函数参数不能命名 `$args`**：`$args` 是**自动变量**，函数里声明同名参数
  **不会绑定**（实测 `bound args=[]`）⇒ 传参静默丢失，而且脚本照样「成功」。
  ⇒ 改名 `$setupArgs` 之类；**验收脚本必须显式断言「参数真的传进去了」**
  （返回值里回带 `ArgsUsed` 并 assert），否则会拿到假阳性 PASS。
- **「重复运行安装包」的无害化**：会话界面里的 `<media type="file" src="…setup.exe" />`
  附件标签，宿主每次重新渲染都会 `setLastPreviewedFile` + `autoOpen`；
  `.exe` 无内置预览器 ⇒ `shell.openPath` 直接执行 ⇒ NSIS 向导弹窗。
  ⇒ 修复只能落在**安装包侧**：`.onInit`（向导出现前唯一能拿到控制权的位置）做版本守卫，
  **同版本 / 更旧 → 静默 `Quit`（零界面）**；判不出来 → **失败开放**（守卫坏了不能让人装不上）；
  `/FORCE` → 放行。**绝不弹 MessageBox**——用户诉求是「不再弹窗」，模态框违背。
  守卫判定脚本要**编译期内嵌、运行期解包到 `$PLUGINSDIR`**，靠**判定文件**回话
  （`nsExec::ExecToLog` 不回传退出码，`ExecToStack` 的压栈顺序不值得赌）。
  ⚠️ `${VERSION}` / `${UNINSTKEY}` 定义在 `!include "{{installer_hooks}}"` **之后**，
  hooks.nsh 内拿不到编译期常量 ⇒ 由模板 `StrCpy` 写进 hooks 声明的 `Var`。
- **vitest 必须排除 `.superpowers/**` 与 `graphify-out/**`**（与 `eslint.config.ts` 的
  `globalIgnores` 对齐）：这两个目录在工作副本里留下成百上千个 `*.test.cjs`，
  目录名被拍平成 `node_modules__xxx`，匹配不到默认的 `**/node_modules/**` ⇒
  外来用例被当成项目用例跑并集体假红。**门禁必须量的是仓库里真正存在的那部分。**
- **发布脚本不能无条件覆盖 `release/` 下的同名文件**（2026-11-02）：`release/` 同时是
  **GitHub Releases 的本地镜像**和**本地构建产物的落地目录**，`rmSync + cpSync` 的无条件覆盖
  会让「给已发布版本重新构建一次」把本地构建写进已发布版本的文件名里 ⇒ **本地包冒充官方发布物**
  （实测 49 个本地安装包里 **12 个**如此，含 3.8.0 / 3.9.0 / 3.9.1）。
  现在的规则：目标不存在 → 照常落到顶层；内容相同 → 不动；**内容不同 → 改写 `release/local-builds/` 并告警**。
  ⚠️ 改这个脚本必须连带看 `scripts/release-publish.mjs` 的 `latestInstaller()`：它要**一起扫 `local-builds/`**，
  否则 `prepare` 会挑到「旧的官方副本」，把上一版的 sha256 写进 `version.json` ⇒ 线上包与清单对不上，
  应用下载后校验失败**直接删文件**。
- **GitHub Release 资产的官方 SHA256 不用下载就能拿**：releases API 每个 asset 带 `digest: "sha256:..."`
  （`https://api.github.com/repos/<owner>/<repo>/releases?per_page=100`）。
  ⚠️ 本机到 GitHub 发行 CDN 的下载被限速到 **0.01 MB/s**（`curl` 20 秒下了 163 KB，`exit 28` 超时），
  44 MB 的安装包要 40 分钟以上 ⇒ **靠下载比对哈希在这台机器上不可行**，改用 API 的 `digest` 字段。
- **`release/` 整个目录被 gitignore**（`.gitignore:43`）⇒ 改 `release/` 里的文件**不进仓库**、
  `git status` 里也看不到 ⇒ 别以为没生效。
- **`release/README.md` 这类「本地镜像」文档会长期腐化**：它曾还是 Sigma File Manager 时代的旧文
  （讲 `v2.2.0-focus.1`、仓库地址指向旧的 `kizemo/sigma-file-manager`）⇒ 改品牌 / 改仓库时要一并核对。

### 2.10 夸克网盘（批次 9–10）的四个硬事实

真源台账：`docs/handoff-2026-11-11-quark-pan.md`（§16–§17）；逐轮记账 `.superpowers/sdd/quark/t9-fix-log.md`。
**下面四条都是实测结论，引用时直接用，不要再自己推一遍**：

- **下载的 412 出现在「CDN 回源」那一跳，不在「下载接口」那一跳**（2026-11-11 22:2x 实测）：
  同一份当前会话的三个变体（完整罐 / 去掉 `__puus` / 只留 `__pus`）调
  `POST /1/clouddrive/file/download` **一律 200/code=0 并返回 `download_url`**；
  拿该 URL 再带 Cookie 打 `dl-pc-sz.drive.quark.cn` 才分出 **206（有 `__puus`） vs 412（无 `__puus`）**。
  ⇒ `__puus` 是**直链回源门槛 cookie**；`quark_provider.rs::download_file` 把 CDN 非 2xx 走
  `failure_from_http_status`，`412 => Auth` ⇒ 用户看到「[auth] 夸克网盘会话已失效，请重新授权。」
- **存盘时机比「域覆盖」更关键**：登录时 `__puus` 比 `__pus` 晚约 1 秒落地 ⇒ 早期版本存下的会话永远缺 `__puus`
  ⇒ 下载必 412。修复是「存盘前有限宽限等待」（`QUARK_PUUS_SETTLE_SECONDS`），**不是**加 cookie 域名。
- **判「装机版有没有修复」别扫 exe 里的 ASCII 字符串**：release 二进制搜不到 `__puus(len=` 这类 format 片段
  （无判别力）；PE `TimeDateStamp` 恒为 `0x00100000`（确定性构建），装机副本与 `target/release` **同尺寸不同 sha**。
  ⇒ 只认「运行时新增日志行」或「只有新版才有的行为差异」。
- **别假设「连接 id == 凭据条目键」**：`remote-credentials.json` 的键是 `credential_key`，查不到就当「没授权」。
  改凭据前**先杀实例**，否则应用退出时会把内存里的会话回写盖掉你刚放好的文件。
  只读体检工具：`.superpowers/sdd/quark/t10-real-download-check.ps1`（不打印 cookie 值 / `pus` / `auth_key`）。

## 3. 代码导航：graphify 知识图谱
本仓库已建好本地 AST 知识图谱（2026-10-21 部署）：**零 LLM、零 API 成本、纯本地**，产物在 `graphify-out/`（已加进 `.gitignore`，不进版本库）。

- **重建**：`graphify extract . --code-only`（1243 个代码文件 / 21 万行，**约 42 秒**）；改过代码就 `graphify update .`。
- **当前规模**：16548 节点 / 34722 边 / 723 社区，96% 边为 `EXTRACTED`、4% `INFERRED`。
- **查问题**：`graphify query "..."`（返回相关子图）、`graphify path "A" "B"`（最短路径）、`graphify explain "X"`（单个符号的全部连接）、`graphify affected "模块名" --depth 2`（**改这里会波及谁**）、`graphify god-nodes --top 10`（核心抽象）。
  MCP 侧工具同名（`query_graph` / `shortest_path` / `get_node` / `god_nodes` / `graph_stats`），MiniMax Code 与 cc-haha 都已注册 `graphify` 这个 MCP server；这些工具都吃可选的 `project_path`（指向含 `graphify-out/graph.json` 的目录），所以**一个注册服务所有项目**。
- `graphify affected "drag-no-activate.ts"` 实测能一次列全该文件的调用方、导入方与 3 个相关测试文件——「改这里影响谁」这种问题 grep 给不出完整答案，这是图最实用的场景。

**已知盲区（别据此下结论，每条都是实测）**：

- **Rust 侧不抓 `mod` 声明**。抽取器抓 `use` 语句（→`imports_from`）和函数定义（→`contains`），但不抓 `mod foo;`。实测 `src-tauri/src/lib.rs` 的文件节点**出边为 0**，而 `lib.rs:20` 有 `mod drag_no_activate;`、`lib.rs:332-341` 注册了 9 个 command。所以对 Rust 模块跑 `graphify affected` 会返回「No affected nodes found」，**这不代表没人依赖它**，只代表图没建这条边。
- **Tauri IPC 边界是断的**。TS 侧 `invoke('set_drag_no_activate')` 的字符串实参**不产生边**；Rust 侧 `pub fn set_drag_no_activate()` 虽然作为符号在图里，但两边互不相连。跨语言追踪必须手工搭桥：以 **command 名字**为锚点，先在 Rust 侧 `graphify query "<command>"` 找定义，再在 TS 侧搜 `invoke('<command>')`。
- **AST 缓存命中率只有约 25%**（1243 个文件只缓存了 309 个），所以 `update` 与全量 `extract` 耗时接近（32–54 秒），**别指望 `update` 是秒级**。
- **节点/社区数在两次运行间会变**（12814 → 16548，因为图里混入了文档概念节点）。**不要把绝对数当稳定指标，更不要拿它做回归门禁。**
- **同名节点会匹配错**。实测 `graphify explain "lib.rs"` 命中的是 TS 测试文件里的 Rust 源码字符串 `LIB_RS`，不是 `src-tauri/src/lib.rs`。名字有歧义时改用 `graphify query` 看候选，或用 MCP `get_node` 传精确 node id。
- **`INFERRED` 边要人工复核**。图自己会在报告的 "Suggested Questions" 里追问「这些推断出来的关系对不对」；实测已出现把 ESLint 配置注释误判成引用 `DirEntry` 的 INFERRED 边。

### 2.3 孤儿 WebView2 进程会让下一次启动直接 panic（exit 101）

- **症状**：应用启动即退，退出码 **101**，应用日志只到 `run: builder built, entering .run()`；
  崩溃原文在 stderr：`tauri-2.x/src/app.rs: Failed to setup app: failed to create webview:
  WebView2 error: WindowsError(HRESULT(0x800700AA), "请求的资源在使用中。")`。
- **根因**：前几轮验收用 SIGKILL 强杀 app，遗留的**孤儿 `msedgewebview2.exe`**
  子进程仍持有 `%LOCALAPPDATA%\com.sigma-file-manager.app\EBWebView` 配置目录锁，
  ⇒ 后续所有启动都在创建 WebView2 时失败。**与本次改动无关**，是历史遗留进程。
- **清理**：只杀「命令行含 `com.sigma-file-manager.app`」的 `msedgewebview2.exe`
  即可立刻恢复。**不要**误杀别的应用的 webview2。
- **根治（2026-10-30 已落码）**：新增 `src-tauri/src/orphan_webview.rs`，
  在 `run()` 的 `prepare_isolated_environment()` 之后、Builder 之前调用
  `cleanup_orphan_webview_processes()`。判定口径是
  「引用我们自己的 WebView2 数据目录」且「祖先链 32 层内没有活着的应用实例」⇒ 才杀，
  避免第二个实例启动时误杀第一个实例的 webview2。模块**不依赖 tauri**（可进 `cargo test --lib`）。
- **验收脚本**：`.superpowers/sdd/acc-r10-orphan.ps1` 跑 4 阶段 A/B ——
  旧版+无孤儿必须成功（对照）、新版+无孤儿必须成功（造孤儿）、
  旧版+有孤儿必须失败（复现）、新版+有孤儿必须成功（修复生效），
  且旧孤儿 pid 必须全部消失 ⇒ `VERDICT=PASS`。

### 2.4 图标母版：改图标要改 SVG 母版，不要手绘 PNG（2026-10-30）

- **图标母版不在版本库里**：`src/assets/icons/logo-1024x1024.png` 是 **Edge headless 渲染内联 SVG** 的产物，
  母版脚本在 `.superpowers/sdd/e1-icon.cjs` / `gen-assets.cjs`，而 `.superpowers/sdd/.gitignore` 是 `*`
  ⇒ **换机器或新会话可能找不到母版**；此时用 `git show HEAD:src/assets/icons/logo-1024x1024.png` 取旧稿，
  再按旧稿实测轮廓重描（轮廓用 alpha 逐列 top/bottom 剖面量，不要目测）。
- **母版几何（24×24 viewBox）**：文件夹路径
  `M2.6 7.2a1.8 1.8 0 0 1 1.8-1.8h4.1l2 2.4h7.9a1.8 1.8 0 0 1 1.8 1.8v7.6a1.8 1.8 0 0 1-1.8 1.8H4.4a1.8 1.8 0 0 1-1.8-1.8z`，
  α 是 `<text x="11.4" y="16.4" text-anchor="middle" font-family="Georgia, 'Times New Roman', serif" font-size="10.4" font-weight="700">`。
  1024 画布下旧稿文件夹 ink bbox = `x[110..861] y[230..810]`。
- **放大的正确手法**：对**整组图形**（文件夹 + α）施加**同一个** `transform="translate(tx ty) scale(k)"`，
  这样形状与比例自动保持。实测 `k=1.21315` → ink bbox 变 `x[56..966] y[160..863]`，
  ink 像素占比 36.7% → 53.9%。
- **逐像素重画会画错**（本轮作废过一版：翻盖左侧斜边、左上/左下圆角全错，还多出一块碎片）。
  ⇒ 放大/重画后必须用**新旧 alpha 轮廓 IoU** 量化保真度（实测 0.99698 才算过），
  IoU 没过就回母版重来，**不要靠肉眼看图定稿**。
- **Edge headless 窗口 < 1024 会渲染出空白图**（实测 `--window-size=32,32` → 117 B 空 PNG、`128,128` → 699 B 空 PNG；
  `1024,1024` 正常）⇒ **一律 1024 渲染再降采样**，降采样用 `skia-canvas` 的 `drawImage`（`toBufferSync('png')`，
  `toBuffer('png')` 是 Promise，CommonJS 顶层 await 会报错）。
- **`src-tauri/icons/` 只是产物**：唯一输入是 1024 PNG。重生成用 `npm run tauri:gen-icons`；
  **不要直接跑 `tauri icon <path>`，会 panic**。
- **`logo-32x32.png` 其实是 64×64 画布**（文件名骗人，CSS 再缩到 32 显示）⇒ 替换时**必须保持画布尺寸一致**，
  否则显示尺寸会缩一半（与「图标变小」的诉求正好相反）。
- **图标复验口径**：新 exe `ExtractIconEx` 取 32×32，与 `src-tauri/icons/icon.ico` 的**原始 PNG 条目**逐点比对；
  再拿**已安装的旧版 exe** 做对照（必须不一致，否则说明 exe 没重新嵌入图标）。
  `Icon.ToBitmap()` 有像素失真，只能当粗判。

## 4. 本轮（批次 4 增量索引 / 批次 7 Git 扩展）新增的坑位

- **打包前必须先杀应用进程**：应用还在跑时 `cargo` 覆盖 `src-tauri/target/release/alpha-file-manager.exe`
  会报 `error: failed to remove file ...` + `Caused by: 拒绝访问。 (os error 5)` ⇒ **构建在最后一步失败**，
  前面的 Rust 编译全部白跑（实测浪费一整轮）。先 `taskkill /F /T /IM alpha-file-manager.exe`，
  并用 `File.Open(exe,'Open','ReadWrite','None')` 确认锁已释放，再启动打包。
  ⚠️ 判失败**只看日志里的 `BUILD_EXIT=`**，`.cmd` 固定 `exit /b 0` ⇒ 任务 succeeded ≠ 构建成功。
- **同一段收尾逻辑不要留两份实现**：全量扫描路径当时是一份 `finalize_scan_state` 的**复制粘贴副本**，
  副本漏掉了新增的 `scan round timings` 埋点 ⇒ 真机验收脚本等满 25 分钟超时才发现。
  ⇒ 同一语义的状态收尾**必须收敛到同一个函数**；发现「同一段逻辑出现两次」就先合并再改。
- **验收脚本等的是日志标记行**：标记行没打出来时，脚本表现为「等满超时 → 报 FATAL」，
  看起来像功能坏了。**先确认标记行确实会被打印，再解释超时**；
  定位手法 = 比对「同一批新增代码的其它埋点在不在日志里」⇒ 在 ⇒ 是**路径没走到**，不在 ⇒ 是**代码没编进去**。
- **`cargo test --lib` 必须在 `src-tauri/` 目录下跑**：仓库根没有 `Cargo.toml`，
  在根目录跑会报 `could not find Cargo.toml ... or any parent directory` 且退出码 101。
- **WebView2 调试端口只有一条路能走通**：`HKCU\Software\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments`
  与环境变量 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` **对本应用都无效**（webview2 进程命令行里没有
  `--remote-debugging-port`）；只有 Tauri v2 窗口配置的 `additionalBrowserArgs` 生效
  （`src-tauri/tauri.conf.json` → `app.windows[0].additionalBrowserArgs`）。
  ⚠️ 该字段是**验收用临时改动，收尾必须回滚并重新打包**。
  连上之后用 Node 内置 `WebSocket`（Node 22+）直连 CDP，无需任何 npm 依赖。
- **应用界面语言不是固定的**：同一台机器上，验收脚本曾因「按中文正则找按钮」而全盘落空（当时界面是英文），
  后来同一脚本又发现界面是中文 ⇒ **正则必须中英双语**（`rescan|重新扫描` 这种写法），
  且**不要**用「界面是中文/英文」当固定前提写进结论。
- **验收脚本匹配文案时，候选集要从「被测对象自己的 locale 文件」推导，不要硬编码单一语言**（2026-11-04 实测）：
  批次 7 首轮验收 `b23-ext-accept.cjs` 把命令标题硬编码成英文（`Repository status` / `Git - Open in IDE`），
  而扩展的运行时标题走**自己的** `locales/zh-CN.json`（`仓库状态` / `Git — 在 IDE 中打开`），
  宿主界面是中文 ⇒ 脚本得到「命令面板 0/12、右键菜单 0/2」的**假阴性**，
  实际扩展已 `activate.ok`（worker 诊断可证）。⇒ 正确写法：`readFileSync(扩展 locales/zh-CN.json)` +
  `en.json` + 原始 key 三路并列为候选，任一命中即算存在，并把命中的那一种语言打进报告。
  **「一个都没找到」先怀疑自己的匹配词，而不是先怀疑功能。**
- **扩展「装进宿主」必须走宿主自己的 store，不能裸调后端命令**（2026-11-04 实测）：
  后端 `install_local_extension` 只把目录拷进 `extensions/` 目录；「已安装 + 已启用」的账本在
  `user-data/user-extensions.json` 里，而 `reconcileInstalledExtensions()` **只把存储里没有的标成 broken，
  不会把磁盘独有的补回存储** ⇒ 裸调后端的结果是**永远不激活**。
  可驱动路径 = CDP 里 `document.querySelector('#app').__vue_app__` →
  `config.globalProperties.$pinia._s.get('extensions').installLocalExtension(sourcePath)`
  （等价于扩展页选完文件夹后的真实路径）。
  ⚠️ 读存储态别读错层：路径是 `pinia.state.value.extensionsStorage.extensionsData.installedExtensions`
  （storage store 里面还有一层 `extensionsData`），不是 `.installedExtensions`。
- **打包可以「等验收跑完再自动开始」**：把 `.cmd` 写成「先轮询 `tasklist | find` 判断应用已退出
  **且** 验收报告文件已存在 → 再 `taskkill` → 再构建」，就能把 10 分钟的 Rust 编译
  和验收收尾**串起来跑**，而不是干等。⚠️ 轮询条件必须同时含「进程没了」和「报告已落盘」，
  只看「进程没了」会在验收中途的 kill/重启间隙里提前开跑。

- **站点「版本说明」写在 `version.json` 的 `notes` 字段，不在 `_release-notes-*.md`**（2026-11-06 实测）：站点 `index.html` 用 `m.notes` 渲染更新说明；而 `_upload_alpha_files.py` 的 `SITE_FILES` 白名单只有 `index.html` / `.htaccess` / `version.json` / `download.php` —— `_release-notes-v*.md` **根本不上传**，它只被 `release-publish.mjs github` 读作 GitHub Release 正文。⇒ 改文案要同时改 `version.json` 的 `notes` **和** `history[0].notes`（应用内更新通道也读这个），外加 `_release-notes-v<版本>.md`。
- **SFTP 全量上传会中途截断，半截文件在公网上仍是 200**（2026-11-06 实测，代价是 3.8.0 安装包被写坏）：`_upload_alpha_files.py` 一次传 35 个文件 / 791 MB，传到 `Alpha_File_Manager_3.8.0_x64-setup.exe` 时连接中断、脚本以**退出码 1** 结束 —— 但已写上去的半个文件留在远端（远端 5,603,328 B / 本地 44,704,300 B），公网 HEAD 照样返回 200。⇒ 只验「当前版本能不能下」会漏掉旧版本被写坏。发布后必须**逐文件比对远端 md5 + size**（SSH 跑 `md5sum` + `stat -c %s`）与本地一致。修单个文件用 paramiko 单独 `put` + 远端 `md5sum` 校验即可，不必重跑 791 MB 全量。

- **`IShellLinkW::Resolve` 会弹 Windows 自己的模态框并让调用方永久阻塞**（2026-11-07 实测）：目标失效时弹「快捷方式存在问题」，owner pid = 调用方进程；传 `HWND(NULL)` 时没有父窗口可挂 ⇒ 调用方**永久卡住**（`cargo test --lib` 当场卡 60 s+ 一个用例都跑不完，列表渲染路径上更是直接卡死整个应用）。⇒ 解析 `.lnk` **只用 `GetPath`**：`GetPath(flags=0)`（shell 自展开）→ `GetPath(SLGP_RAWPATH)` + 手工 `expand_environment_variables()`，**绝不调 `Resolve`**。
- **`SLGP_RAWPATH` 对「环境变量块 + 8.3 短名」记录的目标返回的不是真实路径**（2026-11-07 实测）：`百度网盘.lnk` 的 RAWPATH 是 `PROGRA~2\BAIDUN~1\BAIDUN~1.EXE` ⇒ `exists()` 必失败 ⇒ 回落成「`.lnk` 所在目录」（用户看到「没跳转」）。⇒ 验收样本必须用这种 `.lnk`，用存字面长路径的 `FileZilla.lnk` 会得到**假阳性**。另：把 `.lnk` 自己交给 `explorer.exe /select,…` ⇒ Explorer 弹「**已中止操作**」⇒ 解析失败时只打开 `.lnk` 所在目录，**绝不把 `.lnk` 交给 shell**。
- **「我是不是默认资源管理器」看的是三个注册表 command 值**（2026-11-07 查清）：`HKCU\Software\Classes\Folder\shell\open\command`、`…\explore\command`、`HKCU\Software\Classes\CLSID\{52205fd8-5dfb-447d-801a-d0b52f2e83e1}\shell\opennewwindow\command`；三个全等于本机 launcher 路径才算默认（`default_file_manager.rs` 的 `registry_is_owned_by_launcher`）。⇒ **改任一个即可把「打开文件所在目录」翻到原生 Explorer 分支**做验收；改用户注册表必须**先读原值 → 临时改 → 先杀应用 → 逐项还原并打印还原结果**。
- **验收断言别只取「最后一条」日志**（2026-11-07 实测）：`[reveal]` 一次操作会打两行（`reveal_in_explorer: … mode=Select` + `reveal_in_explorer spawned:`），断言取最后一行 ⇒ 误报 `FAIL mode=Select`。⇒ 判据写成「**任一条**满足条件」。
- **`tauri build` 会先跑一次前端构建 ⇒ 发版元数据必须「先写完再打包」**（2026-11-07 实测，代价是整包重打）：`tauri build` 在 Rust 编译**之前**先跑 `beforeBuildCommand`（vite build），**它跑完之后才落盘的元数据不会进包**。本轮 3.12.2 的 `CHANGELOG.md` 与 14 个 locale 更新日志在 20:28 才写完，而 vite 构建在 20:24~20:27 就结束了 ⇒ 装出来的包里「新功能」对话框**停在 3.12.1**（UIA 实测：版本列表顶部没有 3.12.2）。⇒ 发版顺序固定为「**先写完全部元数据 → 再打包**」；已经打早了只能重打（Rust 编译有缓存时约 5 分钟）。判定包是否含新元数据：用 UIA 枚举更新日志弹窗的版本列表项，看顶部是不是当前版本。
- **UIA 定位：图标按钮没有无障碍名，且窗口 handle 出现 ≠ 元素可枚举**（2026-11-07 实测）：① 侧边栏导航与设置页里的图标按钮**没有 UIA `Name`**（只有 `ClassName` 里的 lucide 图标 class，如 `lucide-settings-icon`、`lucide-rotate-ccw-icon`）⇒ **按名字找「设置」永远找不到**，必须按图标 class 定位元素、再用 `TreeWalker.ControlViewWalker` 往上走到 Button 祖先；② 拿到窗口 handle 后 webview 还没渲染完，整棵 UIA 树只有 12 个 Pane，**必须轮询等树长起来**再枚举/点击；③ 更新日志模态框也是渲染后才出现的，关闭优先用「关闭」按钮的 `InvokePattern`（实测可用），退路才是 `Esc ×2`。
- **`release-publish.mjs prepare` 有两种互斥模式，别混用**（2026-11-07 实测）：`prepare --write-notes <文件>` 是**独立分支，只生成** `_release-notes-v<版本>.md`（带大小/sha256），**不动 `version.json`、不复制安装包**；真正改清单的完整流程是 `prepare --bump none --skip-build --notes-file <文件>`。⚠️ **`--bump` 默认是 `patch`** ⇒ 版本号已 bump 好的发版轮次**必须显式 `--bump none`**，否则 3.12.2 会被顶成 3.12.3。站点说明文字直接取仓库的 `_notes-<版本>.md`（已核对 3.12.1：站点 `version.json` 的 `notes` 与 `_notes-3.12.1.md` 逐字相同）。
- **站点上传顺序：先传安装包、最后翻 version.json**（2026-11-07 实测）：`_upload_alpha_files.py` 默认会把源目录里**全部 35 个 .exe / 791 MB** 重传一遍（本轮只要 1 个新包），且 AGENTS 已有「全量上传中途截断、远端半截文件仍是 200」的记录。⇒ 只传新增的那一个包用 paramiko 单文件 `put` + 远端 `md5sum`/`stat` 校验（脚本 `.superpowers/sdd/upload-one.py`），页面与清单用 `--files-only`；**且安装包必须先传完再传 `version.json`**，否则用户会点到还没传完的包。发布后必查 `release-publish.mjs verify`（版本号/大小/sha256 三项，源站与 Cloudflare 边缘各一次）。


## 附录：2026-11-09 百度内嵌登录（tauri build 真机验收）

### A1 Tauri 里创建 webview 必须在独立线程，否则窗体建出来但内容永远空白

`tauri-runtime-wry` 的 `create_webview` 上有原文注释：*this must be called from a
separate thread, otherwise the channel will introduce a deadlock*。

命令是从 WebView2 的 IPC 回调进来的 —— 那就是**事件循环线程**。在那里直接
`WebviewWindowBuilder::build()` 的表现极具迷惑性：

1. 窗口体（HWND）当场建出来，枚举窗口、找标题、截图全都正常；
2. 但 webview 的创建被**派发回事件循环**并等它的回复；
3. 事件循环此刻正卡在那个 IPC 回调里（回调不返回，回复就不会来）—— 死锁。

于是窗内是一块白板，**后续任何依赖 webview 完成的调用都会一起挂住**
（本例是 `cookies_for_url` 永远不返回，连带「超时」分支也永远走不到）。

⇒ 修法：建窗放进 `std::thread::spawn`，命令立刻返回，事件循环空出来。
**`AppHandle::run_on_main_thread` 单独用不够** —— 它只是把任务异步投递回事件循环，
任务里仍然在事件循环线程上调 `build()`，webview 派发照样死锁。

⇒ 判据别只看「窗口在不在」：**要验 webview 真的渲染了**（截图看内容 + 后续调用有结果）。

### A2 DPAPI 密文变了 ≠ 明文变了

DPAPI 每次加密都用随机 IV，**同样的明文每次密文都不同**。
判「凭据是否真的换了」不能只看 `remote-credentials.json` 的 SHA256 —— 那只证明
「被重新加密过」，不证明「内容变了」。

⇒ 必须解密前后对比：裸 `CryptUnprotectData`（`ProtectedData::Unprotect` 在本项目抛
「参数错误」）解出明文，逐字段比。本轮就靠这一步分清「真换了会话」与「只是重写一遍」。

### A3 「带/不带 cookie 响应没差异」不等于「cookie 失效」

想验证登录态有效性时，先问一句：**这个判据本来能不能看见目标**。

本轮踩到：百度网盘 `/disk/main` 是 SPA 静态壳，服务端对带/不带 BDUSS 返回**逐字相同**的
HTML（`uk=0`）；`api/gettemplatevariable` 不用 cookie 鉴权；老 `pcs/file` 接口已 404。
三个判据全都没有鉴别力 ⇒ 「没差异」推不出「已失效」。

⇒ 要么找一个**真的按 cookie 鉴权**且会随登录态改变响应的接口，要么就把结论写成
「未能证实也未能证伪」，不要拿无鉴别力的判据下结论。

### A4 对外上传：安装包必须排在 version.json 之前

站点发布是 SFTP 上传到 `/var/www/wordpress/alpha-file-manager`，`version.json` 里写着
安装包的 URL + sha256，应用下载后会本地比对 sha256，**不匹配就删文件**。

⇒ **顺序错了会出两个故障**：
① `_upload_alpha_files.py` 的 `SITE_FILES`（含 `version.json`）**先于**所有 `.exe` 上传
   —— 全量上传期间清单已指向新包，但包还没到服务器 ⇒ 用户点下载拿到 404；
② 若只传了 `version.json` 而没传新包 ⇒ sha256 与线上包对不上 ⇒ 应用直接删下载文件。

⇒ 正确做法（2026-11-09 发布 3.13.4 实测）：
**先把新安装包传上去并校验，再传 `version.json` 等页面文件**。定向上传用
`.superpowers/sdd/upload-alpha-one.py`（复用站点脚本的 paramiko 链路，只传白名单文件），
传完用站点自带脚本的 `--files-only` 模式传页面文件，最后跑
`node scripts/release-publish.mjs verify` 做公网回读。

⇒ 顺带：全量上传会把站点目录里**全部** `.exe`（约 1.5 GB）重传一遍，其中绝大部分与本次
发布无关 —— 除了排顺序，别无必要。

### A5 `gh release view --json` 的三个坑

① **没有 `isLatest` 字段**，写了直接报 `Unknown JSON field` 退出 1 ⇒ 判「是不是 Latest」
   要看 `gh release list` 的输出；
② 本机 git/gh 走 sh 时会先吐一行 `/usr/bin/sh: warning: setlocale: LC_ALL: cannot change
   locale (zh-CN)`，**这行会污染 stdout** ⇒ 别在 PowerShell 里对 `gh --json` 的输出做
   `ConvertFrom-Json`（会炸 `Unexpected character encountered while parsing value: U`），
   直接用 `gh ... --json ... --jq '...'` 让 gh 自己过滤；
③ `release-publish.mjs verify` 会自动挑 `release/` 下 **mtime 最新**的 `.exe`，
   而顶层目录可能躺着**同版本号的另一个包** ⇒ 判定「发布的到底是哪个包」必须看
   **字节数 + sha256** 三方对照，不能只看文件名。

### A6 「授权闩」必须同时有 TTL 和「消费即失效」（2026-11-09 focus-sync 实测）

focus-sync v0.5.15 的 `afm_visit_latch` 是一个裸 `AtomicBool`：AFM 前台上升沿上弦，
**没有任何失效路径**。真机日志实证 ⇒ 22 分钟前的一次「AFM → practilog」切换，
依然构成有效授权，把当时那个百度远程伪路径写进了 practilog 的「打开」对话框。

⇒ **判据**（凡是「某个闸门决定要不要对外部做不可逆动作」的设计）：

1. 授权必须带**时效**（TTL）——「上一次切换」不是永久通行证；
2. 授权必须**用掉即失效**——写成功一次就消费掉；
3. 授权必须绑定**具体对象**——本例用「接手前台的进程 pid」，
   于是「AFM → A → B」的多跳切换不再同步（用户从 AFM 直切到弹窗才同步）；
4. 闸门要**在每一个写站点**都存在，不能只保护一条路径 ——
   本例 `/set_path`（侧车重启后的陈旧补推）原先**故意不加闸**，
   结果它成了绕过闸门的旁路。补上后合闸才真正闭合。

⇒ **验收**：闸门类修复必须有「正例放行 + 反例拒绝 + 过期拒绝」三条**同一条判据**下的用例；
只测「拒绝」会把「功能整体失灵」误判成「修好了」。

⇒ **可观测字段必须与闸门同一口径**（2026-11-09 21:09 补记）：`/health` 里的
`afm_visit_latched` 原先只判「记录是否存在」不判 TTL ⇒ 侧车跑了 23 分钟仍报
`true`，而实际闸门早已拒绝写回 ⇒ **验收会被自己的观测数据误导**
（把过期授权当成活授权）。凡是「健康检查/调试字段」要回答「现在还能不能放行」，
就必须复用闸门本体的那套判定，而不是另写一份更宽松的。

⇒ **顺带一条**（写入前的路径合法性）：对外发路径前必须判「**形状 + 存在性**」——
`X:\` 或 `\\server\share` 才算本地目录，且本地目录还要 `is_dir()` 为真。
scheme 虚拟路径（`sfm://remote/<id>/<dir>`）**不能剥前缀后当路径用**：
剥掉 `sfm:/` 得到的 `\remote\<id>\0000temp` 是「看起来像相对路径、实际不存在」的伪路径，
Windows 会弹「找不到」错误框，而那个错误框又会被当成新对话框 → 每 1~2 秒重试一次。

### A7 PowerShell 的 P/Invoke 回调里直接输出字符串**不会进管道**（2026-11-09 实测）

在 `EnumWindows` 这类回调委托里直接 `Write-Output "..."`，
枚举结束后的管道里**一条都收不到** —— 表现为「枚举结果为空」，
于是误判成「AFM 没有主窗口」，实际 hwnd 一直存在。

⇒ 正确做法：回调里把结果塞进 `ArrayList` / `List<T>`，
枚举结束后再统一打印（和 AGENTS.md 里「函数里有返回值就不能混用 Write-Output」是同一类坑）。

⇒ 同源的三条工具坑（本轮一起踩到）：

- 跨进程 `SetForegroundWindow` 被**前台锁**挡住时，
  用 `AttachThreadInput(我的线程, 目标线程, true)` → `SetForegroundWindow` → `false` 才稳；
- `schtasks /End /TN X /F` 的 `/F` **不被接受**（报 Invalid argument/option），
  用 PowerShell 的 `Stop-ScheduledTask` / `Start-ScheduledTask`；
- `.cmd` 里的 `timeout` 在非交互环境报「Input redirection is not supported」并**立即返回**，
  等待要改用 `powershell -Command "Start-Sleep -Seconds N"`。

⇒ 另：`node --check` 对含 `export` 的 `.js` 可以**直接跑**（Node 会自动按 ESM 解析），
不必先复制成 `.mjs` —— 为复制文件被安全策略拦两次是白折腾。

### A8 站点上传：改一个文件用 `--only`，别为了 version.json 重传 1.09 GB（2026-11-12 实测）

- `_upload_alpha_files.py`（站点源目录 `E:\办公文件\L1网站\alpha-file-manager`）原先只有两种粒度：
  **全量**（42 文件 / 1.09 GB / 15–20 分钟）与 `--files-only`（页面 + version.json）。
  本轮给它加了 **`--only <文件名>`**（可重复），只改 `version.json` 时单传一个文件只要几秒。
  ⚠️ **全量上传不要随手跑**：线上目录与本地源目录**不是字节镜像** ——
  线上 `index.html` 的 `<head>` 被 WordPress 注入了 `<link rel="EditURI" … jetpack-rpc?rsd>`，
  所以公网与本地 sha **必然不等**，别把这当成上传失败；
  公网 `download.php` 会 301 + 再跳到安装包本体（`fetch` 跟随后看到的是 45 MB 的 exe，不是脚本）。
- **改线上 `version.json` 的安全流程**：本地改 → 带**格式守卫**回写
  （重新序列化后必须与原文件逐字节一致才允许写入，否则中止）→ `--only version.json` 上传
  （脚本自带远端 md5+size 校验）→ **带 cache-busting 的公网回读**（`?_=<时间戳>`），
  比对公网 sha256 与本地 sha256。
