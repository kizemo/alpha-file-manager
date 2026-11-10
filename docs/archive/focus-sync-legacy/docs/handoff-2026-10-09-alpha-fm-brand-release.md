# HANDOFF —— 品牌化整合包已发布,下一个任务:修目录树不刷新(2026-10-09 晚)

**交接时间**:2026-10-09 19:35
**上一份**:[`handoff-2026-10-09-plugin-consolidation.md`](handoff-2026-10-09-plugin-consolidation.md)
**本轮性质**:A 阶段合并 → P4 改名 + P5 图标 → 构建 → 装机 → 端到端验证
**状态**:✅ 品牌化整合包可安装可用 · 🔧 **下一个任务:目录树不刷新**

---

## 0. 三十秒速览

| 项 | 值 |
|---|---|
| **品牌仓** | `F:\soft\00selfmade\filemanager\sigma-file-manager`,`main` **`d2be2373`**,⚠️ **已提交、未推送**(领先 `origin/main` = `a3577184` 1 个提交) |
| **主仓** | `F:\soft\00selfmade\filemanager`,`main` **`4d9d294`**,已推送 |
| **产物** | `src-tauri\target\release\bundle\nsis\Alpha File Manager_2.2.0_x64-setup.exe`,**17.12 MB**,sha256 `83925123E1731C88AA9CBC6C69761F43B33D46CDB749531BBC06DDCE3EAB3762` |
| **装机现状** | 已装 `C:\Program Files\Alpha File Manager`,**正在运行**,功能已由用户实测确认 |
| **工作区** | 两仓均 CLEAN |

---

## 1. ⚠️ 先读这条:我上一轮报了一个**错的**结论

我曾断言「sync 插件的 worker 从未执行、端到端同步未达成」。**那是错的** —— 那是**测早了**。

实测(侧车日志 `%LOCALAPPDATA%\kizemo\focus-sync\logs\spike.log.2026-10-09`,**日志是 UTC**,本地 = UTC+8):

| 事件 | 本地时间 |
|---|---|
| 侧车起 / 端口 37421 绑定 | 19:17:04 |
| 应用启动 | 19:17:14 |
| **首次 `EXTENSION ALIVE: /ext_alive ping received`** | **19:22:38** |
| 首次真实路径推送(`C:\downloads` 注入成功) | 19:26:31 |

**应用启动 → 插件首次 ping = 5 分 24 秒。** 上一轮安装那次约 **3.2 分钟**。

⇒ **是冷启动,方向对;但不是 2 分钟,是 3–5.4 分钟且不稳定。**
⇒ 插件功能**已验证正常**,日志里有实打实的注入成功记录。

### 教训(本项目的老毛病,今天又犯一次)

「没测到」被写成了「不存在」。判定依据是**在一个 5 分钟的窗口里取样一次**。
**下个会话如果又要判断插件是否活着,至少等 6 分钟再下结论**,否则会重复我今天这个错误。

---

## 2. 本轮完成的事

### 2.1 A 阶段:feature 分支快进合入品牌仓 `main` ✅

`9e764a16 → a3577184`,1 个提交,两侧零分叉,已推送后被本轮的后续提交超越。
`feat/dialog-focus-sync` **有意不合入** —— 它是文件夹选择器(picker)功能,不是 sync 插件,
最后一个提交字面意思是 `fix(tauri): remove app-level ACL TOML that broke all non-picker commands`,
仍在进行中。sync 插件的 38 个文件当时已完整在 `main`。

### 2.2 P4 改名 + P5 图标 ✅(`d2be2373`,83 文件)

**按 plan-v3 §3.3 的 12 处显示位**,另**补齐了计划没列到的品牌位** —— 装完第一版才发现:

- **17 个语言包共 98 处 `Sigma File Manager` → `Alpha File Manager`**。
  `app.name` 是 Settings/About 实际显示的字符串;只改源码那 12 处,会得到
  **「标题栏叫 Alpha、关于页叫 Sigma」的半吊子品牌**。
  ⚠️ 只改**值**;key `requiresSigmaFileManager` 被 `extension-engine-requirements.vue` 的
  `t()` 查表引用,**不能改**。
- **两处应用内 logo 资源**:`src/assets/icons/logo-1024x1024.png`(About 页)和
  `logo-32x32.png`(左侧导航栏)。它们**不是** Tauri 生成的那套图标,靠 `npx tauri icon` 换不掉。

**禁改项已逐条核对,`git diff` 证实零改动**:

| 文件:行 | 常量 | 为何禁改 |
|---|---|---|
| `src-tauri/src/lan_share/types.rs:15` | `MDNS_INSTANCE_NAME` | mDNS 服务名,改了 LAN 上互相搜不到 |
| `src-tauri/src/windows_installation.rs:30` | `LEGACY_DEFAULT_FILE_MANAGER_DATA_DIR_NAME` | **「legacy」是要迁移走的旧目录名**,改了破坏接管默认文件管理器 |
| `src-tauri/tauri.conf.json:5` | `identifier = com.sigma-file-manager.app` | 决定数据目录、插件安装路径、注册表键 |

### 2.3 门禁修复:堵掉一个**空洞 PASS** ✅

`scripts/verify-deploy-sha.ps1` 的基线还指着架构调整前的 `release\extension\dist\index.js`
(该路径已不存在)。而脚本「取第一个**存在的**路径当基线」⇒ 把**装机文件选成了自己的基线**
⇒ 拿自己比自己,**永远 PASS**。

- 改为指向仓内 `extensions/kizemo.focus-sync/dist/index.js`
- 新增 `BASELINE-STALE` 硬失败,禁止再退化成自比较
- **判别性测试**:修完先跑一次 —— 未安装时 **exit 1 FAIL**,装完才 PASS

---

## 3. 🔧 下一个任务:新建文件夹后目录树不刷新

### 3.1 现象(用户报告)

> 在一个目录中新建文件夹,目录树不会更新,新文件夹不会在目录树中显示

### 3.2 根因(已定位到行)

`src/modules/navigator/composables/use-file-tree.ts` 有**两层互相独立的缓存,且都没有失效路径**:

| 行 | 代码 | 后果 |
|---|---|---|
| **L191** | `if (loadedSet.value.has(path)) return;` | 某路径一旦载入过,`ensureLoaded` **永久早退** |
| **L247** | `if (node.isLoaded \|\| !node.isDirectory) return;` | 连 L205 的「重载父节点」路径也会因已载入而跳过 |

⇒ 在**已展开**的目录里新建文件夹:父节点 `isLoaded` 且其 path 在 `loadedSet` 里,
**没有任何东西会失效这两者** ⇒ 新子节点永远不会出现。

**当前没有任何失效 API**:`use-file-tree.ts:65-78` 的 `UseFileTreeApi` 只导出
`rows` / `ensureLoaded` / `ensureAncestorsLoaded`。
(`folder-tree.ts:148` 的 `reset()` 是**另一个 store** —— 展开/选中状态,不是 children 缓存。)

**也没有任何接线**:新建文件夹走 `file-browser-new-item-dialog.vue`(199 行),
它**完全不通知树**;`file-browser-tree-view.vue:65` 只解构了
`{ rows, ensureAncestorsLoaded }`。

### 3.3 ⚠️ 修复时的陷阱

**`use-file-tree.test.ts:69` 有一条测试 `'ensureLoaded is idempotent on already-loaded paths'`,
它主动锁定了 L191 的早退行为。**

⇒ **不要直接删掉 early-return**,那会破坏这条测试,也会让每次展开都重复读盘。
正确做法是**新增一条独立的失效通道**,而不是改写既有语义。例:
给 `UseFileTreeApi` 加 `invalidate(path)`,同时把该 path 从 `loadedSet` 移除、
并把对应 node 的 `isLoaded` 置回 `false`(注意 L247 那道门也要一起放行)。

### 3.4 建议的验收

- 新建文件夹后,**已展开**的父目录立即出现新节点(不必手动折叠再展开)
- 单元测试覆盖「invalidate 后 ensureLoaded 能重新读到 children」
- `use-file-tree.test.ts` 现有 3 条测试**仍然全绿**
- 顺手回归:删除 / 重命名文件夹后树是否也该刷新(同一条缓存,**很可能同样坏**)

---

## 4. 硬约束(全部继续有效)

§2 的 1–23 条见上一份 handoff,外加本轮新增:

| # | 约束 |
|---|---|
| **24** | **`Select-String` 默认不区分大小写。** 搜 `Sigma` 会命中 200+ 处,其中绝大多数是 `sigma-ui-*` CSS 类名 —— **全局替换等于重写整个设计系统**。真正的品牌文本只有 37 行 / 17 文件(大小写敏感) |
| **25** | **改 `.nsi` / `hooks.nsh` 前后各看一次字节数。** `hooks.nsh` **无 BOM**(首 3 字节 `3B 20 3D`);`.nsi` **有 BOM**(`EF BB BF`)。搞错了 makensis 读不懂 |
| **26** | **图像按颜色抠图前先量色距。** 本轮图标背景 `(248,249,251)` 与字形白 `(255,255,255)` **L1 仅差 17** —— 按颜色抠会把 α 打成空心描边。改用按行取轮廓做 alpha key |
| **27** | **判断「某东西没工作」之前,先确认观察窗口 > 已知冷启动时间。** 见 §1 |
| **28** | **`*.exe` 的名字来自 Cargo 包名,不是 `productName`。** 装机后是 `sigma-file-manager.exe`(不是 `Alpha File Manager.exe`)。要彻底改品牌名得动 `Cargo.toml` 的 `[[bin]]`,属另一次改动。`hooks.nsh` 的 kill 列表本来含它,所以重装正常 |

---

## 5. 状态与待办

### ✅ 已验证通过

| 项 | 结果 |
|---|---|
| 静默安装 / **覆盖重装(升级路径)** | 均 exit 0,复用原目录 |
| 窗口标题 | 实测 `Alpha File Manager` |
| NSIS 警告门禁 | **PASS -- 0 warnings**(含本轮改过的 `hooks.nsh`) |
| 沙箱门禁 | **VALID(0 violations / 19 patterns)** |
| 装机 sha 门禁 | **PASS**(仓库源 vs 装机件,非自比较) |
| 侧车 | `status=ok`、`uia_healthy=true`、计划任务 **Running** |
| sync 插件 | **用户实测正常**;日志有注入成功记录 |

### ❌ 未做 / 未验证

- **`d2be2373` 未推送**(用户明确要求留给下个会话)
- **界面截图未能验证**:屏幕捕获中途失效(`CopyFromScreen: handle is invalid`)。
  目录树「代码编进构建产物」已证实(`dist/assets/folder-tree-*.js` + navigator 含 `showFolderTree`),
  但**没能在界面上确认树真的渲染出来**
- 应用自写的 `user-extensions.json` 有 mojibake:em-dash `—` 被存成 `E9 88 A5 3F`(鈥?)——
  UTF-8 被当 GBK 读。**不影响交付物**(5 个载荷文件与仓库逐字节一致,中文语言包完好,
  7071 个汉字)。属应用侧序列化问题,只影响命令标题显示
- 扩展冷启动**根因未查**(3–5.4 分钟为何这么久)
- **卸载路径从未端到端验证**

### 下一步

| # | 任务 | 状态 |
|---|---|---|
| **1** | **修目录树不刷新**(§3) | 🔧 **从这里开始** |
| 2 | 推送 `d2be2373` | 用户指定留给下个会话 |
| 3 | 查扩展冷启动根因 | 独立 |
| 4 | 验证卸载路径 | 建议在 1 之后 |
| 5 | 修 `user-extensions.json` 的编码损坏 | 低优先级,纯观感 |

---

## 6. 回退节点

| 位置 | 节点 | 说明 |
|---|---|---|
| **品牌仓** | **`d2be2373`** | ★ 当前状态,**未推送** |
| 品牌仓 | `a3577184` | `origin/main`,已推送 |
| 品牌仓 | `9e764a16` | 合并前的 `main`,祖先永久可达 |
| **主仓** | **`4d9d294`** | ★ 已推送 |
| 主仓 | `758f466` | A 阶段合并记录 |
| 磁盘 | `C:\Temp\afm-p1-quarantine-2026-10-09\` | P1 移走的全部东西,可回滚 |

---

## 7. 运维命令

```powershell
$repo = "F:\soft\00selfmade\filemanager\sigma-file-manager"
$env:PATH = "C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;$env:PATH"

# 编侧车
cd "$repo\extensions\kizemo.focus-sync\sidecar"; cargo build --release; cargo test --release

# 部署侧车到两个打包点 + 跑沙箱门禁
cd $repo; powershell -NoProfile -ExecutionPolicy Bypass -File scripts\build-with-sidecar.ps1

# 构建整合包(前端 + Rust + NSIS,约 8 分钟)
npm run tauri build

# NSIS 警告门禁(Tauri 会吞掉 makensis 的警告,必须绕开)
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-nsis-warnings.ps1

# 沙箱门禁(单独跑)
node scripts\scan-sandbox-dynamic.cjs `
  extensions\kizemo.focus-sync\dist\index.js `
  src\modules\extensions\runtime\sandbox.ts

# 装机 sha 验收(退出码 0 才算通过)
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-deploy-sha.ps1

# 侧车健康(**判断插件是否活着,注意冷启动 3-5 分钟**)
(Invoke-RestMethod http://127.0.0.1:37421/health) | Select-Object extension_alive,first_push_received
```

---

## 8. 给新会话的一句话 prompt

```
读 F:\soft\00selfmade\filemanager\docs\handoff-2026-10-09-alpha-fm-brand-release.md(必读)。
只做 §3 的任务;根因已定位到 use-file-tree.ts:191/247 两层缓存,注意 §3.3 那条锁定行为的测试。
不要重新讨论名字/图标,决策已全部锁定。
```