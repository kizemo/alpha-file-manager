# Sigma File Manager 扩展开发指南

> **文档目标**：把「为 Sigma File Manager 开发扩展」拆成可执行的四块——**要求**（必须满足什么）、
> **方法**（怎么实现）、**流程**（按什么顺序做）、**合格标准**（做到什么程度算做完）。
>
> **资料基础**：
> - 官方开发文档 [sigma-hub/sfm-marketplace wiki](https://github.com/sigma-hub/sfm-marketplace/wiki)（2026-10-16 拉取，9 个页面全量通读）
> - 官方模板 [sigma-hub/sfm-extension-template](https://github.com/sigma-hub/sfm-extension-template)
> - 宿主一手实现（本工作区 `sigma-file-manager/`，宿主版本 **3.1.3**）：`src/modules/extensions/**`
> - 真实扩展实例：`extensions/kizemo.focus-sync`（本工作区内置）
>
> 官方 wiki 没有写沙箱的具体规则，而沙箱恰恰是扩展开发最高频的阻塞点。
> 本文所有沙箱、加载、生命周期相关的结论都直接来自宿主源码，并标注了 `文件:行号`。

---

## 目录

- [0. 心智模型：扩展到底是什么](#0-心智模型扩展到底是什么)
- [1. 要求](#1-要求)
- [2. 方法](#2-方法)
- [3. 流程](#3-流程)
- [4. 合格标准](#4-合格标准)
- [5. 故障速查](#5-故障速查)
- [附录 A：最小扩展清单模板](#附录-a最小扩展清单模板)
- [附录 B：发布前自检清单](#附录-b发布前自检清单)
- [附录 C：Alpha File Manager 分支的差异](#附录-calpha-file-manager-分支的差异)
- [附录 D：参考资料](#附录-d参考资料)

---

## 0. 心智模型：扩展到底是什么

### 0.1 三层结构

写扩展前必须先分清这三层，后面所有约束都是这三层各自的规矩。

```
┌─────────────────────────────────────────────────────────────┐
│ ① package.json 清单层（声明式）                               │
│    id / permissions / engines / contributes / binaries       │
│    → 宿主据此决定：能不能装、装完显示什么、哪些 API 可用      │
├─────────────────────────────────────────────────────────────┤
│ ② dist/index.js 代码层（行为式）                             │
│    export function activate(context) / deactivate()          │
│    → 宿主在 Web Worker 里加载，注册命令/菜单/UI/监听          │
├─────────────────────────────────────────────────────────────┤
│ ③ 外部资源层（可选）                                          │
│    本地二进制 / 远程 API / sidecar 进程 / 本地数据目录        │
│    → 由 ① 里的 binaries / http 权限声明，由宿主代管          │
└─────────────────────────────────────────────────────────────┘
```

**最容易踩的认知错误**：把扩展当成「一个网页」。它不是。它没有 DOM，运行在 Worker 里，
只能通过 `sigma.*` 这一个全局对象访问宿主能力。**凡是需要浏览器能力的地方，先问「有没有对应的 `sigma` 命名空间」**。

### 0.2 双源架构：Marketplace 与 Package 各管一半

扩展的元数据来自两个地方，职责分离得很干净（wiki：Extension Architecture）：

| 来源 | 位置 | 谁控制 | 管什么 |
|------|------|--------|--------|
| **Marketplace** | `sfm-marketplace` 的 `registry.json` | SFM 维护者 | 发布者、描述、分类——**影响用户信任的字段** |
| **Package** | 扩展仓库的 `package.json` + 构建产物 | 扩展作者 | 权限、入口、版本——**技术/运行时字段** |

版本号由宿主**自动从 GitHub release tag（`v*`）发现**，不需要在注册表里登记。

### 0.3 三种扩展类型

| 类型 | 说明 | 入口 `main` | 典型场景 |
|------|------|------------|---------|
| `api` | ESM 模块，拥有完整 `sigma` API；也可纯清单静态贡献 | 运行代码的通常写 `dist/index.js`；**纯主题 / 纯图标主题可以省略** | 绝大多数扩展 |
| `iframe` | 沙箱化 iframe 集成 | `index.html` | 需要完整自定义页面的场景 |
| `webview` | 独立的 Tauri 窗口 | `index.html` | 需要独立窗口、多窗口协作 |

**默认选 `api`。** 本文档以 `api` 类型为主线。

### 0.4 运行时模型

宿主源码给出的确切事实：

- 加载入口 `src/modules/extensions/runtime/loader.ts:391` 调 `createWorkerHost(...)`，
  **API 扩展跑在一个独立创建的 Web Worker 里**。
- 在此之前，`loader.ts:366` 先跑 `validateExtensionCode(code)` 做**静态准入校验**，不通过直接抛错、扩展不激活。
- `sandbox.ts` 里还有一个 `createSandbox()`（`with(globalContext)` + `new Function` 的老式执行器），
  **本版本中没有被加载路径调用**，不要按它的 `ALLOWED_GLOBALS` / `BLOCKED_GLOBALS` 列表来推断运行时约束。
  真正的约束来自下面 §1.4 的静态校验 + Worker 本身没有 DOM 这两件事。

---

## 1. 要求

### 1.1 准入要求（想进官方商店必须满足）

官方 wiki 在 Getting Started 开头用 `WARNING` 块明确写了三条硬门槛：

1. **质量**——设计、行为、可用性必须是有意为之：功能要真的好用、界面要真的好看、
   要给用户一个**明确的收益**。不是「能跑就行」。
2. **用途**——必须与**文件管理、效率提升，或用户使用 Sigma File Manager 的方式本身**有实质关联。
   与应用领域无关的扩展不会被收录，除非它好到能成为特例。

> 这两条是**评审标准，不是技术检查**。技术全绿但「跟文件管理没关系」，照样被拒。
> 反过来，一个只有右键菜单 + 一个通知、但把一个高频痛点解决得很好的扩展，是能过的。

另外，提交评审时还会按 [Submitting to the Marketplace](https://github.com/sigma-hub/sfm-marketplace/wiki/Submitting-to-the-Marketplace) 核查：
无恶意代码与未授权数据收集、README 有使用说明、用了网络/远程 API/超出用户预期的数据采集**必须在 README 里说明**、
`permissions` 与实际行为一致、`engines` 真实反映兼容性、许可证与 SPDX 标识一致且第三方资源有署名、
仓库保持可维护。详见 §4.4。

### 1.2 清单必填字段

`package.json` 根目录必须有：

| 字段 | 要求 |
|------|------|
| `id` | `publisher.extension-name`，全局唯一 |
| `name` | 显示名 |
| `version` | 语义化版本 |
| `repository` | **公开的** GitHub 仓库 URL |
| `license` | SPDX 标识符 |
| `extensionType` | `api` / `iframe` / `webview` |
| `main` | 运行时入口模块。`iframe`/`webview` 必填；`api` 中**运行代码的必填**，纯主题/纯图标主题可省略 |
| `permissions` | 所需权限（见 §1.3） |
| `engines` | 兼容范围（见下） |

> ⚠️ **运行代码的 `api` 扩展必须写 `"type": "module"`。**
> 这是标准 npm 字段，**清单 schema 不校验它**，漏写不会报错，但宿主无法把入口当 ESM 导入，
> 扩展在运行时失败且**没有清晰报错**（wiki Troubleshooting 专章警告过这一点）。

**`engines` 两个子字段**：

| 字段 | 说明 |
|------|------|
| `sigmaFileManager` | 必填，宿主版本范围，如 `">=2.0.0"` |
| `extensionApi` | 可选，`@sigma-file-manager/api` 版本范围。**用了特定版本新增的 API 就必须声明** |

`extensionApi` 不满足时：已安装的扩展仍然可见，但在应用升级前**无法启用**；依赖更高版本的商店条目对用户隐藏。

已知门槛值：`sigma.http` → `>=1.10.0`；`sigma.view` → `>=1.11.0`；
list-detail 弹窗、剪贴板读 API 同属需要声明的较新能力。

**常用可选字段**：`$schema`（本地 schema 路径，给编辑器校验/补全）、`publisher`、`icon`、`banner`、
`media`（详情页预览图/视频）、`categories`、`tags`、`platforms`（非跨平台时必填）、
`activationEvents`、`contributes`、`binaries`、`previousName`（改名兼容）。

> ⚠️ **`files` 字段对 tag 安装无效。** 标准 npm 发布白名单在 Git 归档安装路径上不起作用，
> 要控制 tag 树里有哪些文件，请用 `.gitignore`。

### 1.3 权限模型与最小权限原则

| 权限 | 解锁能力 |
|------|---------|
| `contextMenu` | `sigma.contextMenu` |
| `commands` | `sigma.commands` |
| `sidebar` | 侧边栏贡献 |
| `toolbar` | 工具栏贡献 |
| `notifications` | `sigma.ui.showNotification()` |
| `dialogs` | `sigma.ui.showDialog()` 与 `sigma.dialog` |
| `fs.read` | 只读文件系统操作 |
| `fs.write` | 写入与下载 |
| `shell` | `sigma.shell` 与只读 `sigma.binary` |
| `clipboard` | `sigma.ui` 下的剪贴板 API |
| `openUrl` | `sigma.context.openUrl()` |
| `http` | `sigma.http.request()`，**必须带 host 白名单对象** |
| `view` | `sigma.view` 布局与排序控制 |

**三条硬规矩**：

1. **只申请真正用到的权限。** 权限范围越小，用户信任越高，评审风险越低——这同时是评审维度。
2. **每个命名空间都需要对应权限**，调用被拒时错误是 `permissionDenied: <namespace>`。
   加完权限**必须重新安装扩展**才生效。
3. **HTTP 权限必须用对象形式**，写裸字符串 `"http"` 会被直接拒绝：

```json
"permissions": [
  "commands",
  "notifications",
  {
    "name": "http",
    "hosts": [
      "https://api.example.com",
      "http://localhost:*",
      "http://127.0.0.1:8080"
    ]
  }
]
```

host 白名单的模式规则：scheme 必须 `http`/`https` 且与请求一致；主机名大小写不敏感；
不写端口 = 该主机任意端口；`:*` 后缀显式表示任意端口。
宿主**每次请求前**校验 URL，并对重定向目标二次校验（最多 5 跳）；
请求体与响应体各限 10 MB；`Cookie` / `Host` / `Content-Length` 三个请求头被屏蔽。
**用户会在安装时和扩展权限页看到这份白名单**，所以只写你真要用的源。

### 1.4 沙箱约束（官方文档没写，但这是最高频阻塞点）

宿主在 `loader.ts:366` 对扩展入口代码执行 `validateExtensionCode()`。
命中以下 **19 条正则**中的任意一条，扩展**不会激活**：

| # | 正则 | 报错文案 | 替代方案 |
|---|------|---------|---------|
| 1 | `\beval\s*\(` | `eval() is not allowed` | 无 |
| 2 | `\bnew\s+Function\s*\(` | `new Function() is not allowed` | 无 |
| 3 | `\.constructor\s*\(\s*['"\`]` | `Dynamic code construction via .constructor() is not allowed` | 无 |
| 4 | `\bdocument\s*\.` | `Direct document access is not allowed` | `sigma.ui.*` |
| 5 | `\bwindow\s*\.` | `Direct window access is not allowed` | `sigma.*` |
| 6 | `\blocalStorage\s*\.` | `localStorage is not allowed` | `sigma.storage` |
| 7 | `\bsessionStorage\s*\.` | `sessionStorage is not allowed` | `sigma.storage` |
| 8 | `\bindexedDB\s*\.` | `indexedDB is not allowed` | `sigma.storage` / `sigma.fs.storage` |
| 9 | `\bfetch\s*\(` | `fetch() is not allowed - use sigma.fs instead` | `sigma.http.request()` 或 `sigma.fs` |
| 10 | `\bXMLHttpRequest\b` | `XMLHttpRequest is not allowed` | `sigma.http.request()` |
| 11 | `\bWebSocket\b` | `WebSocket is not allowed` | 宿主未提供 |
| 12 | `(?<![\w$.])self\b` | `Direct self access is not allowed` | 无 |
| 13 | `(?<![\w$.])globalThis\b` | `Direct globalThis access is not allowed` | 无 |
| 14 | `(?<![\w$.])postMessage\s*\(` | `Direct postMessage() is not allowed` | 无 |
| 15 | `(?<![\w$.])addEventListener\s*\(` | `Direct addEventListener() is not allowed` | `sigma.context.onPathChange` 等 |
| 16 | `(?<![\w$.])removeEventListener\s*\(` | `Direct removeEventListener() is not allowed` | `dispose()` |
| 17 | `(?<![\w$.])dispatchEvent\s*\(` | `Direct dispatchEvent() is not allowed` | 无 |
| 18 | `\bimportScripts\b` | `importScripts is not allowed` | ESM `import` |
| 19 | `(?<![\w$.])navigator\b` | `Direct navigator access is not allowed` | `sigma.platform.*` |

来源：`sigma-file-manager/src/modules/extensions/runtime/sandbox.ts:208-285`。

**三条必须知道的扫描语义**：

1. **只扫可执行代码，注释和字符串字面量先被剥离。**
   `sandbox.ts:290` 调 `stripCommentsAndLiterals(code)` 再跑正则，
   代码注释里写「请勿使用 `fetch()`」或英文散文里的 `self` **不会**被拒。
   宿主为此专门写了回归测试（`runtime/__tests__/sandbox-validation.test.ts`），并在测试里点名
   focus-sync 的 `dist/index.js` 曾因散文里的 `self` 一词被误拒。

   > **这条修正了一条流传甚广的旧说法。** 早期版本确实扫描注释，曾导致注释里一句
   > `foreground window.` 让整个扩展被静默拒绝、108 分钟零请求。
   > 在当前宿主（3.1.3）上，注释已不再是雷区。

2. **模板字符串的插值部分仍会被扫描。**
   测试用例 `` const url = `https://x/${fetch("y")}` `` 明确被拒。
   也就是说字面量里的静态文本安全，**动态拼接出来的表达式不安全**。

3. **失败的表现是「什么都没发生」。**
   `loader.ts:385` 直接抛错 → `catch` 块调 `clearExtensionRegistrations(extensionId)` 清空全部注册。
   同时 `loader.ts:368-375` 会把 `code validation failed: <具体错误>` 以 `error` 级别
   写进 worker diagnostics。**用户视角的现象是「扩展装了、但什么都不发生」**——
   必须去诊断里看才有具体原因。

> ⚠️ **自己写沙箱扫描脚本时，不要硬编码正则快照。**
> `sandbox.ts` 在 git 历史里变更过（`ba3e5a3f` 新增了第 3 条 `.constructor()`），
> 硬编码会静默失效。宿主自带 `sigma-file-manager/scripts/scan-sandbox-dynamic.cjs`，
> 它**从 `sandbox.ts` 实时解析 pattern**。优先用它，或直接指向宿主那份。

> ⚠️ **扫描器比运行时闸门更严格 —— 这是故意的。**
> 上面的「剥离注释」只发生在运行时的 `validateExtensionCode()` 里。
> **两个扫描器（宿主的 `scan-sandbox-dynamic.cjs` 与本仓库的 `scripts/scan-sandbox.cjs`）
> 都直接对原始代码跑正则，不剥离注释。**
> 实测：一份只有 `// TODO: do not call fetch() here; window. is also blocked` 的文件，
> 两个扫描器都报 `INVALID (2 violations)`，而运行时闸门会放行。
>
> 这是 **fail-closed** 设计，可以这样理解：
> **扫描器通过 ⇒ 运行时一定通过；扫描器报错 ⇒ 未必真有问题（可能只是注释里提到了违禁词）。**
> 所以扫描器报红时，先看它标注的 `[in comment]` / `[IN CODE]`——
> `[IN CODE]` 才是真违规；只有 `[in comment]` 命中可以放行，但仍建议顺手改掉措辞，
> 否则 CI 上的自检会一直红。

### 1.5 打包与发行要求

这是「本地能跑、装到别人机器上就废」的头号来源，必须当成硬要求：

| 要求 | 原因 |
|------|------|
| **`main` 指向的文件必须提交进 git tag** | tag 安装走 **Git 归档** |
| **宿主不会为用户跑 `npm install` / `npm run build`** | 用户只拿到归档里已有的东西 |
| **不能发布 `node_modules`** | 体积；且 `@sigma-file-manager/api` 是纯 devDependency，运行时由宿主提供 |
| **所有运行时资源都要提交** | CSS、字体、图标、`locales/`、`media` 预览图、图标主题引用的图标 |
| **`media` 资源要瘦** | 这些文件随每次安装和更新一起分发，务必压缩 |

完整清单：本地 `npm run build` → 提交 `dist/` → `node_modules` 留在 `.gitignore` → 打 `v1.0.0` tag →
**检查 tag 树里确实包含 `dist/`**。

### 1.6 身份与命名红线

- `id` 用 `publisher.extension-name`，`publisher` 通常是自己的 GitHub 用户或组织。
- **社区扩展禁止使用 `sigma`、`sigma-file-manager`、`sfm`、`sigma-hub` 等前缀**，
  禁止使用 Sigma File Manager 的名称、logo、徽章、会让用户以为「官方」的措辞，除非有明确授权。
- 市场 `registry.json` 的 `id` 必须与扩展 `package.json` 的 `id` **完全一致**。
- `featured: true` 是维护者保留字段，**不要自己设**。
- 市场 `name` ≤ 50 字符，`description` 10–500 字符。
- 避免同一扩展在市场出现重复条目，除非有明确理由（例如换主的维护型 fork）。

---

## 2. 方法

### 2.1 选模板起步

官方提供 8 个模板，**从模板起步，不要从空目录起步**：

| 模板 | 适用 |
|------|------|
| `templates/minimal-command` | 一个简单命令 + 通知 |
| `templates/context-menu-notification` | 文件右键菜单动作 |
| `templates/modal-and-dialogs` | 命令 + 对话框或弹窗 |
| `templates/progress-and-context` | 进度 UI、当前上下文、设置 |
| `templates/http-api` | HTTP 请求 + form / list-detail 弹窗 |
| `templates/binary-deno` | 托管外部二进制 |
| `templates/powershell-runtime` | 仅 Windows 的 PowerShell 集成 |
| `templates/advanced-shell-fallback` | 跨平台 shell 与二进制工作流 |

把选中的目录复制到新仓库，该目录即扩展根目录。

**用模板后必须做的替换**（官方 Getting Started 第 2 步）：

1. 替换 `package.json` 里的所有占位：`id` / `name` / `publisher` / `repository` / `license` /
   `categories` / `tags` / `engines` / 非跨平台时的 `platforms` / `permissions`。
2. 替换命令 ID 与右键菜单 ID（`package.json` 与 `src/index.ts` 两处）。
3. 更新 `activationEvents`，**尤其是 `onCommand:...` 的值**。
4. 重命名 `locales/en.json` 里的可见文案。
5. **删掉所有你不出货的 demo 命令、菜单项、弹窗、通知。**

### 2.2 ID 命名规则（最容易出错的一处）

**短 ID 与全 ID 的分界线**：

| 位置 | 用哪种 | 例子 |
|------|--------|------|
| `contributes.commands[].id` | **短 ID** | `download-video` |
| `contributes.contextMenu[].id` | **短 ID** | `convert` |
| `sigma.commands.registerCommand()` | **短 ID** | `{ id: 'download-video' }` |
| `sigma.contextMenu.registerItem()` | **短 ID** | `{ id: 'convert' }` |
| `activationEvents` | **全 ID** | `onCommand:sigma.video-downloader.download-video` |
| `contributes.keybindings[].command` | **不带扩展前缀的命令 ID** | `download-video` |

宿主执行命令面板时用的是 `{extensionId}.{commandId}` 拼出来的全 ID。
两边不一致 → **静默不激活**，没有任何报错（wiki Troubleshooting 单列一节）。

### 2.3 UI 能力选型

不要凭感觉选 API，按这张表对：

| 需求 | 用什么 |
|------|--------|
| 短反馈 | `sigma.ui.showNotification()` |
| 确认或单值输入 | `sigma.ui.showDialog()` |
| 长耗时任务 | `sigma.ui.withProgress()` |
| 收集表单值 | `sigma.ui.showModal()` |
| 多字段 + 实时联动校验 | `sigma.ui.createModal({ layout: 'form' })` |
| 可搜索列表 + 详情面板 | `sigma.ui.createModal({ layout: 'listDetail' })` |
| 弹窗里拉远程 API 数据 | `sigma.http.request()` + form / list-detail |
| 响应系统剪贴板变化 | `sigma.ui.onClipboardChange()` |

**form 布局钩子**：`onSubmit(values, buttonId)`（返回 `false` 可阻止关闭）、
`onClose`、`onValueChange`、`updateElement`、`setContent`、`setButtons`、`getValues`、`close`。

**list-detail 布局钩子**：`onSelectionChange`、`onSearchChange`、`onFilterChange`、
`setListDetail`（合并部分更新）、`getListDetail`。

> ⚠️ **list-detail 有个反直觉行为**：宿主**不会**在你的 `onSelectionChange` 回调之前预更新选中项。
> 必须在回调里显式调 `setListDetail()`，把选中项、预览、元数据、按钮文案**一起**应用。
>
> ⚠️ `setContent()` 默认**按元素 ID 保留已填值**。整表重建、想要全新默认值时，
> 传 `{ preserveValues: false }`。

### 2.4 文件访问：四选一

`sigma.fs` 下有**四套不同的访问模型**，选错会白折腾：

| 模型 | 用途 | 关键限制 |
|------|------|---------|
| `sigma.fs`（核心） | 用户已导航到 / 已选中的绝对路径 | **宿主按会话内的用户交互范围收窄访问**；范围外的任意绝对路径会被拒 |
| `sigma.fs.private` | 扩展目录内的文件 | 路径相对扩展根，**不能含 `.` 或 `..` 段** |
| `sigma.fs.storage` | **扩展自有持久文件**（缓存、生成物、导入的用户数据） | 需要 `deleteFile` / `importFile` |
| `sigma.fs.scoped` | 扩展自己的存储目录**之外**、由用户授权的目录 | 先 `requestDirectoryAccess()` 弹窗授权 |

**默认选 `sigma.fs.storage`。** 核心 `sigma.fs` 的会话范围限制是最容易在
「我本地跑得好好的，用户那边报 Access denied」这类问题上翻车的选择。

要访问用户自有存储目录之外的位置，走 `requestDirectoryAccess({ permission, title, defaultPath })`，
其中 `permission` 为 `read` / `write` / `readWrite`，需要 `dialogs` + `fs.read`/`fs.write` 权限。

### 2.5 网络与外部二进制

**网络**：优先 `sigma.http.request()`，不要 shell 出去跑 `curl`，也不要自己打包 HTTP 客户端。
宿主负责强制白名单、屏蔽敏感头、限制 10 MB、校验重定向。

```ts
const response = await sigma.http.request({
  url: 'https://api.example.com/items',
  method: 'GET',
  query: { page: 1 },
  headers: { Accept: 'application/json' },
  timeoutMs: 10_000,          // 默认 30000，最大 120000
});

if (!response.ok) {
  throw new Error(`Request failed with status ${response.status}`);
}

const payload = JSON.parse(new TextDecoder().decode(response.body));
```

> ⚠️ **JSON 解析失败的最常见原因是解析了被截断的响应体。**
> `response.body` 是 `Uint8Array`，必须**完整**解码后再 parse，不要先截断。

**外部二进制**（`ffmpeg`、`yt-dlp`、`deno` 等）：声明在 `package.json > binaries`，
运行时用 `sigma.binary.getPath(id)` / `getInfo(id)` 解析。**宿主在安装和更新时负责下载、校验、缓存、清理。**

资产字段：`downloadUrl`（**精确 URL**）、`integrity`（`sha256:<64位十六进制>`，必填）、
`archive`（是否需解压）、`executable`、`platform`、`arch`。

四条纪律：

1. **锁定精确版本**，用带版本的资产 URL。
2. **能给出显式 SHA-256 就给。**
3. 只有上游本来就在资产旁边发布了可靠校验文件时，才依赖校验文件托管。
4. **已发布的扩展绝不依赖可变的 `latest` 下载 URL。**

**开发/生产不维护两套二进制定义**——同一份清单既用于从文件夹加载，也用于市场安装。
完整性校验发生在**两个不同层**：

| 层 | 校验什么 | 在哪配置 |
|----|---------|---------|
| 扩展包 | GitHub release ZIP | 市场注册项的 `releaseMetadata.<version>.integrity` |
| 外部二进制 | 从清单资产下载的归档/可执行文件 | `binaries[].assets[].integrity` |

> `registry.json` 里的任何字段都**不能**替代二进制元数据；二进制声明永远在扩展清单里。

**`onInstall` 与取消传播**：用户取消二进制下载时，宿主会中止安装并自动回滚。
如果你的 `onInstall` 在二进制装完后还有后续工作，**不要用 try/catch 把取消吞掉**——
当 `sigma.runtime.isExtensionInstallCancelledError(error)` 为真时，让它抛出去或重新抛出。

### 2.6 国际化

不需要在扩展里引 i18n 库，扩展的翻译会**并入宿主 i18n**。

推荐模式：

```ts
const t = sigma.i18n.extensionT;

export async function activate() {
  await sigma.i18n.mergeFromPath('locales');   // 读取 locales/{locale}.json
}
```

之后用 `t('myKey', { name: 'value' })` 取用户可见文案。
`extensionT` 在缺键时返回 `fallback`，再缺则返回 key 本身。
各 `locales/*.json` 之间 **key 必须一致**。

宿主自身的 key 用 `sigma.i18n.t()`，如 `extensions.loadingExtension`。

### 2.7 平台与路径

- `sigma.platform`：`os` / `arch` / `pathSeparator` / `isWindows` / `isMacos` / `isLinux` / `joinPath(...)`。
  **按平台分支行为、按平台规则拼路径**，不要自己判断反斜杠。
- `sigma.path`：纯字符串工具 `dirname` / `basename` / `extname`，不触平台、跨平台一致。
- 只支持部分平台的扩展，**必须在清单里用 `platforms` 显式声明**。

### 2.8 生命周期与资源清理

导出约定：

```ts
export async function activate(context) {
  // context.extensionId / extensionPath / storagePath / activationEvent
}

export function deactivate() {
  // 释放未自动托管的定时器、订阅等
}
```

`activationEvents` 可用值：
`onStartup` / `onInstall` / `onUninstall` / `onEnable` / `onDisable` / `onUpdate` / `onLocaleChange` /
`onCommand:<全命令ID>`。

大多数注册方法返回 `Disposable`，**扩展禁用或卸载时会自动清理**。
只有**不由运行时自动管理**的长期资源（自己 `setTimeout` 起的轮询、自己持有的句柄）才需要在
`deactivate()` 里显式释放。

> 关于 `setInterval`：宿主沙箱的 `ALLOWED_GLOBALS` 里有它，静态校验也不拦它，
> 但实践中有场景会被拦。**需要周期任务时，防御性地用 `setTimeout` 链**更稳
> （这是 focus-sync 扩展在生产中总结出的做法）。

---

## 3. 流程

### 3.1 端到端流程

```
 ① 立项自检          ② 选模板            ③ 改清单           ④ 装依赖
 用途相关？          8 选 1              换掉所有占位        npm install
 质量过硬？          复制为扩展根         对齐 permissions     （api 包是 devDep）
     │                   │                   │                   │
     ▼                   ▼                   ▼                   ▼
 ⑤ 写代码            ⑥ 构建              ⑦ 沙箱自检          ⑧ 本地安装
 activate/deactivate  npm run build        scan-sandbox-        扩展页 →
 + 替换 demo ID        确认 main 存在      dynamic.cjs         右上菜单 →
 + 重写文案                                VALID (0 violations) 从文件夹安装
     │                   │                   │                   │
     ▼                   ▼                   ▼                   ▼
 ⑨ 真机验收          ⑩ 发版              ⑪ 提交商店          ⑫ 后续更新
 在打包版里跑       提交 dist/            fork sfm-marketplace  bump version
 完整用户路径       打 v1.0.0 tag          registry.json 加条目   重建+提交
                   推 tag                 开 PR                 新 tag
                                                          + releaseMetadata
```

### 3.2 每一步的出口条件

每一步都有「做到了什么才算做完」，缺一项就别往下走。

| 步骤 | 出口条件（gate） |
|------|-----------------|
| ① 立项自检 | 能用一句话说清「解决谁的什么高频痛点」；与文件管理/效率有实质关联 |
| ② 选模板 | 选定 8 个模板之一，并已复制为独立仓库根 |
| ③ 改清单 | 搜不到任何 `your-name` / `my-extension` / `My Extension` 占位；命令 ID 短/全 ID 一致；`permissions` 与代码实际调用逐条对齐 |
| ④ 装依赖 | `npm install` 成功；`engines.extensionApi` 与用到的 API 匹配 |
| ⑤ 写代码 | 所有 demo 命令/菜单/弹窗/通知已删除；用户可见文案全部走 `extensionT`；`deactivate()` 已处理自定义长期资源 |
| ⑥ 构建 | `npm run build` 通过；`main` 指向的文件在磁盘上真实存在 |
| ⑦ 沙箱自检 | 扫描器输出 `VALID (0 violations / N patterns)`，且规则数 N 与宿主 `sandbox.ts` 一致 |
| ⑧ 本地安装 | 从文件夹安装成功；**等待冷启动完成后**扩展出现在列表且能启用 |
| ⑨ 真机验收 | 见 §4.3，必须有**产物级证据**，不能只凭「调用没报错」 |
| ⑩ 发版 | `git ls-tree v1.0.0 dist/` 能列出构建产物；`node_modules` 不在 tag 树内 |
| ⑪ 提交商店 | PR 已开；`registry.json` 的 `id` 与清单一致；`releaseMetadata` 的 `integrity` 是**该版本 release ZIP 的真实 SHA-256** |
| ⑫ 更新 | 新 tag 推送后，宿主自动发现版本（`v*` 模式）；若用了 `releaseMetadata` 则同步更新该字段 |

### 3.3 商店提交三步

1. Fork [sfm-marketplace](https://github.com/sigma-hub/sfm-marketplace)。
2. 在 `registry.json` 加一条：

```json
{
  "id": "your-username.extension-name",
  "name": "Your Extension",
  "description": "Clear description of what it does",
  "publisher": {
    "name": "Your Name",
    "url": "https://github.com/your-username"
  },
  "repository": "https://github.com/your-username/your-extension",
  "featured": false,
  "categories": ["Productivity"],
  "tags": ["keyword1", "keyword2"],
  "releaseMetadata": {
    "1.0.0": { "integrity": "sha256:<release-zip-sha256-hex>" }
  }
}
```

3. 对 main 分支开 PR。

`releaseMetadata` 可选：写了就校验 release ZIP 完整性；不写，应用仍能从 tag 发现版本，只是跳过完整性校验。
键名用**不带 `v` 前缀**的语义版本（`1.0.0` 对应 tag `v1.0.0`）。

**可用分类**：`File Management` / `Media` / `Security` / `Backup & Sync` / `Productivity` /
`Search & Filter` / `Appearance` / `Integration` / `Developer Tools` / `Example` / `Other`。

**徽章**由维护方决定：`Official`（`sigma-hub` 组织出品）/ `Community`（第三方）/ `Featured`（精选）。

### 3.4 发版更新流程

已上架扩展发布新版本：

1. `package.json` 里 bump `version`。
2. **重建**并提交新的 `dist/`。
3. 打与版本号对应的 tag（如 `v1.1.0`）。
4. 推 tag。
5. 若用了 `releaseMetadata`，加新版本的 SHA-256。
6. 对市场仓库开 PR。

宿主会**自动从 GitHub release tag（`v*`）发现新版本**。
不用 `releaseMetadata` 的话，日常更新只需推 tag，不必再开市场 PR。

---

## 4. 合格标准

分四级。**逐级向上**，不要跳级——A 级不通过时，B/C/D 级的结论全部作废。

### 4.1 A 级：静态合规（机器可判定，必须 100%）

| # | 检查项 | 判定 |
|---|--------|------|
| A1 | `package.json` 全部必填字段齐全 | 逐项对照 §1.2 |
| A2 | 运行代码的 `api` 扩展含 `"type": "module"` | `node -e` 读 JSON 断言 |
| A3 | `main` 指向的文件在仓库中**已提交且存在** | `git ls-tree <tag> <main>` |
| A4 | 沙箱校验 0 违规，且规则条数与宿主一致 | 动态扫描器 `VALID (0 violations / 19 patterns)` |
| A5 | `node_modules` 不在 tag 树内 | `git ls-tree -r <tag> --name-only \| grep node_modules` 为空 |
| A6 | `permissions` 中每一项都能在代码里找到对应调用 | 反向核对，无「多申请」也无「漏申请」 |
| A7 | `http` 权限是对象形式且 hosts 非空 | 清单解析断言 |
| A8 | 搜不到任何模板占位符 | `grep -rn 'your-name\|my-extension\|My Extension'` |
| A9 | 短 ID / 全 ID 用法符合 §2.2 | `contributes` 与 `registerCommand` 的 id 逐条对齐 |
| A10 | 所有 `media`/图标资源在 tag 树内 | 清单引用路径逐条 `git cat-file -e` |

### 4.2 B 级：功能验收（必须用真实打包版）

| # | 检查项 | 判定 |
|---|--------|------|
| B1 | 扩展在**打包版宿主**中安装成功并能启用 | 非 dev 构建 |
| B2 | `activate()` 真的被调用 | 有宿主侧可见证据（日志/注册项/通知），非「没报错」 |
| B3 | 命令面板能搜到命令，执行后有预期效果 | 逐条命令过一遍 |
| B4 | 右键菜单项在 `when` 条件命中/不命中时**表现都正确** | 命中显示、不命中隐藏 |
| B5 | 所有声明的设置项可读写且持久 | 改设置 → 重启应用 → 值仍在 |
| B6 | 剪贴板/网络/文件系统权限被真实触发过一次 | 至少一次成功路径 |
| B7 | 错误路径有可操作的报错，不是静默失败 | 断网/文件不存在/权限不足各测一次 |
| B8 | 长时间任务有进度反馈且可取消 | `withProgress` 场景 |
| B9 | 扩展禁用 / 卸载后无残留（进程、文件、注册项） | 看进程管理器 + 存储目录 |
| B10 | 目标平台的路径构造正确（Windows/macOS/Linux 或声明的子集） | 至少覆盖声明的每个平台 |

### 4.3 C 级：工程质量

| # | 检查项 |
|---|---------|
| C1 | 网络请求、文件操作、外部进程的失败**都被显式捕获**并给出可操作提示 |
| C2 | 执行文件系统 / shell / 网络动作**之前**先校验来自命令参数、prompt、自定义弹窗的输入 |
| C3 | 所有用户可见文案走 `sigma.i18n`，`locales/*.json` 各语言 key 一致 |
| C4 | `deactivate()` 释放了不由运行时托管的长期资源 |
| C5 | 用户可见字符串在 17 个语言包中都有对应值（宿主有 17 种语言） |
| C6 | 扩展详情页有 `media` 预览，且资源体积已优化 |
| C7 | README 含安装、配置、使用、权限说明；**有网络或超预期数据采集时明确说明** |

### 4.4 D 级：商店就绪

| # | 检查项 |
|---|---------|
| D1 | 仓库公开，README 有使用说明，至少一个 release tag |
| D2 | `id` = `publisher.extension-name`，市场与清单完全一致 |
| D3 | **未使用** `sigma` / `sfm` / `sigma-hub` 等官方前缀、logo、徽章或「官方」措辞 |
| D4 | `publisher.name` / `publisher.url` 指向真实维护者 |
| D5 | `name` ≤ 50 字符，`description` 10–500 字符，准确不误导 |
| D6 | `featured` 未被设为 `true`（除非维护者已同意） |
| D7 | 无恶意代码、无未授权数据采集 |
| D8 | 许可证文件与 SPDX 标识一致；第三方资源已署名/授权 |
| D9 | `engines.sigmaFileManager` 真实反映兼容性；用了新 API 则 `engines.extensionApi` 已声明 |
| D10 | 依赖外部工具的，全部声明在 `binaries` 且每个资产可校验 |
| D11 | `releaseMetadata`（若用）的 `integrity` 是**该版本 release ZIP 的真实 SHA-256**，已重新计算 |
| D12 | 无重复市场条目 |

### 4.5 验收纪律（这条比上面所有条目加起来都重要）

**「调用型 API 返回成功」不等于「状态改变了」。**

- 对**禁用**的按钮调 UIA `Invoke()` 会**静默空操作**，脚本照样打印 `invoked=yes`。
- `SetValue` 的 read-back 只证明值进了输入框，**不证明框架状态变了**。
- 「不报错」和「没测到」在输出里长得极像。

因此，验收断言只认两类证据：

1. **持久化后的文件**（配置、落盘数据实际变了）
2. **可见的像素 / 状态变化**（截图对比、UI 状态确实变了）

同理，写探针时必须带**阳性对照**：先操作一个必然有可见后果的元素，确认观测手段本身有效，
再去测目标。否则「没反应」分不清是探针失败还是功能没反应。

**判红时信息量已经足够；判绿时必须先问「探针真的跑到被测对象了吗」。**
任何注入式验证至少要有一个**必红的阳性对照**：先随便破坏一处、确认转红，再开始逐条测。

---

## 5. 故障速查

按「症状 → 排查顺序」组织。官方 Troubleshooting 的全部条目 + 宿主源码实证补充。

### 5.1 扩展完全不工作

| 症状 | 排查 |
|------|------|
| 装上了但什么都不发生 | ① 看 worker diagnostics 里的 `code validation failed:` → 沙箱违规；② 确认 `dist/` 已提交在 tag 内 |
| 命令面板搜不到 | 命令 ID 用了全 ID 而不是短 ID |
| 命令面板有但执行报 `Command not found` | ① `registerCommand()` 的 id 与 `contributes.commands` 的短 ID 不一致；② 改了 `dist/` 但没重建重装；③ 面板执行的是 `{extensionId}.{commandId}` |
| 命令点了没反应（静默） | `activationEvents` 的 `onCommand:` 用了短 ID —— **必须全 ID** |
| 装完报错但没细节 | 运行代码的 `api` 扩展漏了 `"type": "module"` |

### 5.2 沙箱相关

| 症状 | 说明 |
|------|------|
| `fetch() is not allowed` | 用 `sigma.http.request()`（需 `http` 权限 + hosts 白名单）或 `sigma.fs` |
| `localStorage` / `indexedDB` is not allowed | 用 `sigma.storage`（结构化值）/ `sigma.fs.storage`（文件） |
| `Direct navigator access` | 用 `sigma.platform.*` |
| `Direct document/window access` | 用 `sigma.ui.*` / `sigma.dialog` |
| `Direct addEventListener()` | 用 `sigma.context.onPathChange` / `onSelectionChange` / `onClipboardChange` / `sigma.settings.onChange` |
| `Direct self access`（**代码里**） | 注意：注释里的 `self` 已不再触发（宿主剥离注释后再扫描）；检查模板字符串插值 |

### 5.3 权限相关

| 报错 | 处理 |
|------|------|
| `permissionDenied: http` | 清单里缺 `http` 权限对象 |
| `permissionDenied: view` | 加 `"view"` 到 `permissions`，**然后重装** |
| `Access denied: target host is not allowed` | ① 把**精确 origin** 加进 `hosts`；② scheme 与端口要匹配；③ **重装** |
| 裸 `"http"` 权限被拒 | 改用 `{ "name": "http", "hosts": [...] }` |
| `Request header "Cookie" is not allowed` | `Cookie` / `Host` / `Content-Length` 被屏蔽，移除 |
| 请求成功但 JSON 解析失败 | 解析了截断的 body。用完整 `response.body` |
| 任意命名空间 `permissionDenied` | 加对应权限并**重装** |
| `sigma.binary.getPath()` 返回 `null` | ① `binaries[].id` 与 `getPath(id)` 参数一致；② 当前平台/架构有对应 asset；③ `integrity` 是合法 `sha256:<64 hex>` 且与实际文件匹配 |

### 5.4 行为相关

| 症状 | 处理 |
|------|------|
| `setLayout/setSorting` 不生效 | 确认加了 `view` 权限并**重装**；注意它改的是**全应用**导航器设置，不是当前目录 |
| schema 对 sidebar `url` 报 `format: uri` | 升级 `@sigma-file-manager/api`。旧 schema 错把相对路径要求成完整 URI |
| 主题 / 图标主题扩展要求 `main` | 不需要。只贡献主题的 `api` 扩展可省略 `main`，也不需要空 `index.js` |
| `install` 大小异常 | `node_modules` 进了 tag 树 |
| 用了 CDN 依赖导致离线不可用 | 自带本地资源 |
| 二进制下载 URL 指向 `latest` | 已发布扩展必须锁精确版本 + 显式 SHA-256 |

---

## 附录 A：最小扩展清单模板

```json
{
  "$schema": "./node_modules/@sigma-file-manager/api/manifest.schema.json",
  "id": "your-name.my-extension",
  "name": "My Extension",
  "version": "1.0.0",
  "type": "module",
  "extensionType": "api",
  "main": "dist/index.js",
  "repository": "https://github.com/your-name/my-extension",
  "license": "MIT",
  "publisher": {
    "name": "Your Name",
    "url": "https://github.com/your-name"
  },
  "categories": ["Productivity"],
  "tags": ["keyword1", "keyword2"],
  "engines": {
    "sigmaFileManager": ">=2.0.0",
    "extensionApi": ">=1.11.0"
  },
  "activationEvents": ["onStartup"],
  "contributes": {
    "commands": [
      { "id": "run", "title": "Run", "description": "…", "icon": "play" }
    ],
    "contextMenu": [
      { "id": "run", "title": "Run", "group": "sigma", "when": { "entryType": "file" } }
    ]
  },
  "permissions": ["commands", "contextMenu", "notifications"],
  "scripts": {
    "build": "tsc",
    "prepare": "npm run build"
  }
}
```

对应的最小 `src/index.ts`：

```ts
const t = sigma.i18n.extensionT;

export async function activate(context) {
  await sigma.i18n.mergeFromPath('locales');

  sigma.commands.registerCommand(
    { id: 'run', title: t('run.title') },
    async () => {
      const entries = sigma.context.getSelectedEntries();
      if (entries.length === 0) {
        sigma.ui.showNotification({ title: t('noSelection'), type: 'warning' });
        return;
      }
      try {
        await doWork(entries);
        sigma.ui.showNotification({ title: t('done'), type: 'success' });
      }
      catch (error) {
        sigma.ui.showNotification({ title: t('failed'), type: 'error', description: String(error) });
      }
    },
  );

  sigma.contextMenu.registerItem(
    { id: 'run', title: t('run.title') },
    async (ctx) => {
      // ctx.selectedEntries: { path, name, isDirectory, size?, extension? }[]
    },
  );
}

export function deactivate() {
  // 释放不由运行时托管的长期资源
}
```

对应的 `package.json > activationEvents` 提醒：
如果命令在启动时不激活、只在被调用时激活，写
`"onCommand:your-name.my-extension.run"`（**全 ID**）。

---

## 附录 B：发布前自检清单

```bash
# 1. 沙箱自检（用宿主的动态扫描器，不要硬编码规则）
node sigma-file-manager/scripts/scan-sandbox-dynamic.cjs dist/index.js
# 期望：VALID (0 violations / 19 patterns)

# 2. main 指向的文件存在且已提交
node -e "const p=require('./package.json');console.log(p.main)"
git ls-files --error-unmatch "$(node -p "require('./package.json').main")"

# 3. tag 树里没有 node_modules
git ls-tree -r v1.0.0 --name-only | grep node_modules   # 期望无输出

# 4. 没有模板占位符
grep -rn 'your-name\|my-extension\|My Extension' --include='*.json' --include='*.ts' .   # 期望无输出

# 5. 权限与代码调用逐条对齐（人工过一遍 permissions 数组）
```

---

## 附录 C：Alpha File Manager 分支的差异

本工作区内的宿主是 **Alpha File Manager**（Sigma File Manager 分支），宿主版本 **3.1.3**，
Tauri `identifier` 为 `com.sigma-file-manager.app`。相对于上游，扩展开发需要注意：

| 项 | 差异 |
|----|------|
| **沙箱注释扫描** | 已修复。`sandbox.ts:290` 剥离注释与字面量后再匹配，散文里的 `self`/`window.` 不再导致误拒。上游文档与本仓库早期 `docs/extension-changelog.md` 中「沙箱会扫描注释」的说法已过时。 |
| **两个扫描器都比运行时闸门严格** | 运行时闸门 `validateExtensionCode()` 先剥离注释与字面量再匹配；**但两个扫描器（宿主 `scan-sandbox-dynamic.cjs` 与本仓库 `scripts/scan-sandbox.cjs`）都直接对原始代码跑正则**，因此注释里出现 `fetch(` / `window.` 会被**报为违规**。这是**故意的 fail-closed**：扫描器通过 ⇒ 运行时一定通过；反之不成立。 |
| **扫描器的真正价值是规则不会过期** | 选宿主的 `scan-sandbox-dynamic.cjs`，因为它从 `sandbox.ts` **实时解析 pattern**；本仓库的 `scan-sandbox.cjs` 是硬编码快照，`sandbox.ts` 变更后会静默失效。 |
| **扩展冷启动** | 需要 **3–5 分钟**。判定「扩展没生效」之前必须等够这个窗口。 |
| **真机验收形态** | **必须用 `tauri build` 的打包产物**。`tauri dev` 下应用外壳正常但**所有路由页面内容区空白**（`RouterView` 挂载却不渲染子组件），在 dev 构建上做的界面验收结论一律作废。 |
| **调试手段** | 拿不到 webview console 是常态（wry 不透传 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`，CDP 连不上）。可用取证：`PrintWindow(hwnd, hdc, 2)` 截图、UIA 树枚举、页面内 Vue DevTools 组件搜索框（UIA `ValuePattern` 可写入）。 |
| **DPI** | 本机 DPI 144（150% 缩放）。验收量到的是物理像素，换算 CSS 逻辑像素要除以 1.5。 |

---

## 附录 D：参考资料

| 资源 | 地址 |
|------|------|
| 官方开发文档（wiki） | https://github.com/sigma-hub/sfm-marketplace/wiki |
| 官方模板 | https://github.com/sigma-hub/sfm-extension-template |
| 官方市场 | https://github.com/sigma-hub/sfm-marketplace |
| 扩展 API 包 | https://www.npmjs.com/package/@sigma-file-manager/api |
| 上游仓库 | https://github.com/aleksey-hoffman/sigma-file-manager |
| Lucide 图标（UI 用） | https://lucide.dev/icons/ |
| 宿主沙箱实现 | 本工作区 `sigma-file-manager/src/modules/extensions/runtime/sandbox.ts` |
| 宿主加载器 | 本工作区 `sigma-file-manager/src/modules/extensions/runtime/loader.ts` |
| 沙箱动态扫描器 | 本工作区 `sigma-file-manager/scripts/scan-sandbox-dynamic.cjs` |
| 真实扩展实例 | 本工作区 `sigma-file-manager/extensions/kizemo.focus-sync/` |
| 扩展变更历史与实战教训 | 本仓库 `docs/extension-changelog.md` |

---

*本文档事实核对于 2026-10-16。官方 wiki 会持续更新；涉及沙箱规则、清单字段、API 版本号的结论，
请以宿主源码 `src/modules/extensions/**` 为准，wiki 为辅。*
