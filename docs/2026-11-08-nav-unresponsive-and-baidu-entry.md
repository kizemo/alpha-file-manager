# 2026-11-08 左侧导航「无响应」根因分析与修复 + 百度网盘入口

日期：2026-11-08
分支：`feat/baidu-remote-provider`（HEAD 9063db44，阶段 1 T2–T9 改动仍未提交）
范围：用户两条诉求 —— ①点左侧导航切换时出现「无响应」，查日志并修复；②左侧导航里找不到「添加百度网盘」的入口。

---

## 一、诉求 ①：导航切换「无响应」

### 1.1 取证

日志路径（本轮新确认）：`C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.app\logs\Alpha File Manager.log`（UTC 时间）。

| 观察 | 数字 | 含义 |
|---|---|---|
| `webview guard: alive ... main:NNNNms` | 1366 条采样中 **91 条 ≥ 800ms**，最大 ~4.7s | 心跳迟到量 |
| 全局搜索全量扫描 | `elapsed=3249731ms`（54 分钟）/ `865570ms`（14.4 分钟），C/D/E/F/G 五盘，约 **197 万文档** | 扫描期与心跳迟到期高度重合 |
| `quick-view` 窗口 hung | `02:20:47 → 02:35:36` 连续 15 次，每次 ~63s 无心跳 | 同一机制，波及第二个 webview |
| `main` 窗口 hung | **0 次** | 主窗口从未触发 60s 无心跳的判定 |

**关键口径纠正**：`main:NNNNms` 不是往返延迟，而是 **「距上次心跳的毫秒数」**。
心跳走 `invoke('webview_guard_heartbeat')` —— Tauri v2 的 IPC 在**主线程**上收，
主线程一堵，这条心跳就迟到。⇒ **迟到量 = 主线程被阻塞的时长**。

### 1.2 空闲态实测（排除「切换本身卡」）

新写 `.superpowers/sdd/nav-switch-probe.ps1`：逐个点击左栏按钮，用
`SendMessageTimeout(WM_NULL)` 探活 + UIA 树 SHA256 指纹测内容变化。

```
CANDIDATES=7（左栏 5 个页面按钮 + 云图标 + logo）
alive_ms=135~198   changed_ms=285~965   dead_samples=0
```

⇒ **扫描没在跑的时候，切换导航完全不卡**。卡顿是**条件性**的。

### 1.3 根因（证据链闭合）

| 环节 | 证据 |
|---|---|
| ① 前端在扫描期间**持续轮询**状态 | `src/stores/runtime/global-search.ts` → `pollStatus()` → `startStatusPolling()`（启动即开），`POLL_INTERVAL_ACTIVE_MS` 档 |
| ② 该轮询打的是**同步 command** | `src-tauri/src/global_search/commands.rs`：`pub fn global_search_get_status()` —— Tauri v2 同步 command **在主线程执行** |
| ③ 同步 command 要抢**读锁** | `scan.rs`：`GLOBAL_SEARCH_STATE.read()` |
| ④ 写锁被**磁盘 IO 长时间持有** | `scan.rs` 提交段（原 1132–1163）：`replace_index_dir()` + `open_or_create_index()` + `flush_pending_changes()` + `calculate_dir_size()` **整段在写锁内**；`finalize_scan_state()` 里的 `write_meta()` 同理 |
| ⑤ 结论 | 主线程被写锁挡住 → 窗口消息泵停摆 → Windows 判「未响应」；扫描期磁盘被打满又把这几段 IO 放大到数秒 |

排除的假设：
- **状态锁长持锁** —— 早期判断成立（短临界区），后来被 ④ 推翻：临界区**短**，但**里面塞了磁盘 IO**。
- **`get_system_drives` 阻塞** —— 2026-11-07 已改成 async（`dir_reader/commands.rs:141`），不是本次原因。
- **前端 JS 线程被扫描阻塞** —— 心跳迟到是 Rust 侧算的，说明堵点在 Rust 主线程。

### 1.4 修复（三处，均已落地）

**R1｜提交段的磁盘 IO 移出状态写锁**（`src-tauri/src/global_search/scan.rs`，根因修复）
把「清空旧 index/reader/fields」留在锁内（瞬时），把 `replace_index_dir` / `open_or_create_index` /
`flush_pending_changes` / `calculate_dir_size` 全部移到**锁外**。
安全性依据：提交窗口由 `status.is_committing` 标记，查询侧（`query.rs:165/179/194/481`）
在 `is_committing` 期间本来就直接返回空结果，读不到「旧 reader 已清空、新 reader 未装上」的中间态。

**R2｜状态查询 / 取消扫描改成 async**（`src-tauri/src/global_search/commands.rs`，保险）
`global_search_get_status` / `global_search_cancel_scan` 由同步 command 改为
`async` + `tauri::async_runtime::spawn_blocking`，**彻底不占主线程**。
沿用本项目 2026-11-07 给 `get_system_drives` / `resolve_windows_directory_shortcut` 用过的同一手法。

**R3｜文档化的结构约束**：本项目里「同步 command = 主线程 = 窗口消息泵」是硬约束，
后续新增会在主线程做 IO 的 command 必须走 `spawn_blocking`。

### 1.5 验证状态

| 项 | 状态 |
|---|---|
| `cargo test --lib` | ✅ **1142 passed / 0 failed / 1 ignored**（`.superpowers/sdd/baidu-navfix-test1.log`，TEST_EXITCODE=0），与 T9 基线一致、无回归 |
| 真机复验（扫描进行中切换导航不再未响应） | ⏳ **待做** —— 需要打包产物 + 一次全量扫描窗口（14–54 分钟） |

⚠️ 未验证项要说清楚：R1/R2 的**逻辑链**已由日志 + 源码双向坐实，但**「不再未响应」这个现象本身**
尚未在真机上复现验证 —— 真机验收必须用 `tauri build` 产物（`tauri dev` 的路由页内容区是空白，界面结论一律作废）。

---

## 二、诉求 ②：左侧导航加入「添加百度网盘」入口

### 2.1 问题定位

原来的入口链是：左栏云图标 → **悬停**才展开的面板 → 面板里只有「远程连接设置」和已配置连接列表，
新建连接藏在设置页里。悬停面板对第一次用这个功能的人太隐蔽 ⇒ 用户找不到。

### 2.2 改动（双入口）

| 位置 | 改动 |
|---|---|
| `src/modules/nav-sidebar/nav-sidebar.vue` | 云图标**下方**新增一颗**常驻**「+」按钮（`data-test="remote-nav-add"`，`aria-label` = 「添加连接」） |
| `src/modules/nav-sidebar/components/remote-nav-panel.vue` | 面板**顶部**新增一行「+ 添加连接」（`data-test="remote-nav-add"`，emit `add`） |
| `src/stores/storage/remote-connections.ts` | 新增瞬态请求 `pendingCreate` / `requestRemoteCreate()` / `consumeRemoteCreate()`（**消费即清空**，与既有 `pendingEditId` 同机制） |
| `src/modules/settings/ui/categories/remote/connections-section.vue` | `applyPendingCreate()` 消费请求 → 直接打开新建表单；`onMounted` 与 `watch` 双路兜底（组件新挂载 / 已在该分类两种情形） |

**追加：入口带协议预选**（真机验收时发现点开后协议仍是 `FTP（明文）`，与「添加**百度网盘**」的意图差一步）

| 位置 | 改动 |
|---|---|
| `src/stores/storage/remote-connections.ts` | `pendingCreate` 由 `boolean` 升级为 `RemoteProtocol \| null`；`requestRemoteCreate(protocol: RemoteProtocol = 'ftp')`；`consumeRemoteCreate(): RemoteProtocol \| null`（取走即置 `null` 并返回协议） |
| `src/modules/nav-sidebar/nav-sidebar.vue` | `openRemoteCreate()` 传 `'baidu_pan'` —— 两颗「+」（图标轨常驻 + 面板顶部）都走同一个 `openRemoteCreate()`，因此**两个入口都预选百度网盘** |
| `src/modules/settings/ui/categories/remote/connection-form.vue` | Props 新增 `initialProtocol?: RemoteProtocol`；`nextProtocol = connection?.protocol ?? props.initialProtocol ?? 'ftp'`。命中百度分支后既有逻辑自动生效：`host` 固定 `BAIDU_PAN_HOST`、端口走 `getRemoteProtocolDefaultPort`、`服务器地址`/`端口` 输入行隐藏、百度授权卡出现 |
| `src/modules/settings/ui/categories/remote/connections-section.vue` | 新增 `createProtocol` ref，`applyPendingCreate()` 把消费到的协议写进去，模板 `<ConnectionForm :initial-protocol="createProtocol">` |

默认参数仍是 `'ftp'` ⇒ 不传协议的既有调用方行为不变（改动可逆）。

i18n：**复用既有键 `settings.remote.action.add`**（17 个语言包齐全，已逐个核对），
不新增键 ⇒ 不触发 17 语言包同步工作量。`百度网盘` 这个标签用既有键 `settings.remote.protocol.baidu_pan`（`ch.json:655`）。

### 2.3 验证状态

- 前端单测：`remote-nav-panel.test.ts` + `remote-category.test.ts` → **28 passed**（新增/改判 5 条用例：面板「+」只发 `add`、store 瞬态语义含「携带协议 + 不传退回 ftp」、导航「+」打开表单**且 `field-host` 不存在（协议确为百度网盘）**、不带协议时 `field-host` 照常出现、请求被消费后不重复弹表单）。
- eslint：本轮改动的 7 个文件 → **0 问题**。
- 真机 UI 证据：见 `.superpowers/sdd/nav-entry-accept.txt` / `.png`（UIA 按 Name 命中「添加连接」并点击成功）。
- **协议预选已真机复验通过**（产物 `target/release/alpha-file-manager.exe` 时间戳 `12:42:42`，晚于改动；
  前一份 12:27:04 早于改动，其结论只到「入口能点开」为止）。点开左栏「+」后，UIA 转储里协议项显示 **「百度网盘」**
  （旧产物显示 `FTP（明文）`），表单出现百度专有字段 `Cookie` / `初始目录` / `本地目录`，
  `服务器地址` 与 `端口` 两行不再出现 ⇒ 证实 `connection-form.vue` 的 `isBaidu` 分支确实由 `initialProtocol` 驱动。
  本轮证据：`nav-entry-accept.txt` / `nav-entry-accept.png`，`EXPECT_HIT=True`、`TREE_COUNT=150`、`CLICK_METHOD=InvokePattern`。

---

## 三、本轮踩到的坑（续写进 AGENTS.md 的候选）

1. **`webview guard` 的 `main:NNNNms` 是「距上次心跳的毫秒数」，不是往返延迟。**
   心跳走 `invoke`，IPC 在主线程收 ⇒ 这个数字 = **主线程阻塞时长**。此前被当成 RTT，
   于是「前端 JS 被阻塞」这个方向被带偏了很久。
2. **同步 Tauri command 在主线程执行**，而主线程就是窗口消息泵。
   本仓已有先例：2026-11-07 把 `get_system_drives` / `resolve_windows_directory_shortcut` 改成 async，
   注释就写在 `dir_reader/commands.rs:129`。**凡是会在主线程做 IO 的 command，一律 `spawn_blocking`。**
3. **锁的临界区「短」不等于「不阻塞」**：短临界区里塞一次目录替换/索引补写，
   一样能把抢锁的主线程堵几秒。查这类问题要看**临界区里有没有 IO**，而不只是临界区长度。
4. **状态锁外做「换代型」操作要确认读侧是否已按标志位短路**：
   本次靠 `status.is_committing` 让 `query.rs` 在提交窗口直接返回空结果，
   才敢把「新 reader 装回状态」挪到锁外。

---

## 四、待办

- [x] `tauri build` 产物真机验收：左栏「+」按钮可见可点 → 点它落到设置页远程分类 → 新建表单已打开。**已通过**（`.superpowers/sdd/nav-entry-accept.ps1`，`EXPECT_HIT=True`）。
- [x] 协议预选的真机复验：**已通过**（12:43 轮，`12:42:42` 产物）——点开表单协议为「百度网盘」，`服务器地址`/`端口` 两行消失、出现 `Cookie` 粘贴区。见 §2.3。
- [ ] 真机复验「扫描进行中切换导航不再未响应」（需要一次 14–54 分钟的全量扫描窗口，会打断用户使用，本轮未安排）。
- [x] 无关 rustfmt 文件回滚：已**脚本化精确回滚 42 个**（`.superpowers/sdd/fmt-noise-analysis.cjs` + `rollback-fmt-noise.cjs`），KEEP 白名单跳过了 `global_search/scan.rs`、`global_search/commands.rs` —— handoff §10 原命令把 `global_search` **整目录**列进回滚名单，照抄会毁掉诉求 ① 的修复。
- [ ] 阶段 1 T2–T9 改动仍未提交（本轮改动同样未提交；用户未要求 commit/push）。

---

## 五、收尾：用户机「+」未出现 = 装机版滞后（根因 · 处置 · 复验）

**根因（非代码缺陷）**：用户装机运行的是 10:34 的 round 1 旧包，不含「+」代码；
round 2 构建其实已于 12:27:02 **成功**（`.superpowers/sdd/baidu-navfix-build3.log` 尾行 `BUILD_EXITCODE=0`，
产物 `Alpha File Manager_3.12.2_x64-setup.exe` 45,332,362 B），但首次重打包 12:13 因旧实例占用 exe
失败（`os error 5`），之后**从未把新包安装进机器**。

**处置**：`.superpowers/sdd/nav-entry-install.cmd` 静默覆盖安装（taskkill → `setup /S` → dir 核验）。
setup exit=0；装机 exe mtime 变为 12:26、81,980,416 B，与 round 2 产物逐项一致 ⇒ 同版本号 3.12.2
覆盖安装成功，未走卸载。

**真机复验（`.superpowers/sdd/nav-entry-probe.ps1` 修好后 run4，日志 `nav-entry-probe-run4.log`）**：

- `ADD_FOUND rect=18,453 43x42` —— UIA 按真实可访问名「添加连接」命中左栏云图标下方的「+」；
- `CLICK_LANDED=CREATE_DIALOG` —— `InvokePattern.Invoke()` 点击生效，「添加远程连接」表单出现（功能闭环）；
- 整屏实拍 `nav-full-123740.png`（「+」清晰可见）与 `nav-full-124444.png`（点击后表单）。

**探针两个坑（污染并作废了此前所有 UIA 结论，务必别重蹈）**：

1. 函数名 `CP` 被 PowerShell **内置别名 `cp`（Copy-Item）遮蔽**（alias 优先级高于 function），
   码点拼名全部变空 ⇒ UIA 空名匹配到整窗，产出假阳性 `ADD_FOUND 3840x2088`，
   第一轮「旧包复现成功」的阴性结论也因此不可信。凡码点拼名函数改名 `Cps`。
2. `FindAll(Descendants, TrueCondition)` 全扫 rail 在本 Chromium 树里 0 命中 ⇒ rail 正向对照计数不可用；
   可靠判据 = **按名精确 FindFirst + 屏幕实拍**。

**屏幕口径已变**：主窗口 rect `-11,-11 3862x2110`（4K 屏幅，与旧记录 2560x1440@150% 不同）
⇒ 复用任何旧验收脚本里写死的坐标/缩放前，先重测 DPI 与屏幅。

## 六、「添加百度网盘」页里的 Cookie 获取说明（2026-11-08 下午追加）

### 6.1 后端真实解析口径（界面文案必须与它一致）

来源：src-tauri/src/remote/baidu_auth.rs 的 parse_manual_cookie_input（约 :196-224）+ scan_cookies（约 :115-131）。

- BDUSS **必需**，且 cookie 名**区分大小写** —— 只粘小写 bduss 会被判「缺少 BDUSS」。
- STOKEN **可选** —— 没有它读取照常，只是失去上传加速。
- 同名 cookie 取**第一个**。
- 容忍 Set-Cookie: 前缀（自动抓取那条路复用同一个解析函数）。

⇒ 因此**三种粘贴形态都可用**，表单提示按这个口径写：
1. 浏览器复制下来的整串 Cookie（原样粘，后端自己挑 BDUSS / STOKEN）；
2. BDUSS=值; STOKEN=值；
3. 只粘裸 BDUSS 值（不含等号时按裸值处理）。

### 6.2 表单里加了什么

- 只在 protocol === baidu_pan 时出现（模板 v-if="isBaidu"，插在「密钥/浏览器 Cookie」那一行整组之后，不能插在 v-if/v-else-if 兄弟之间，否则打断条件链）。
- 一个**默认收起**的 Collapsible：标题「如何获取并复制 Cookie」；展开后是 5 条步骤 + 3 条注意（顺序照浏览器实际操作顺序写，不写做不到的理想步骤）。
- 「打开百度网盘登录页」按钮调 openUrl(BAIDU_PAN_URL)，交给系统默认浏览器；这是项目里 about.vue 的同款做法（@tauri-apps/plugin-opener），不是新造轮子。
- 测试锚点：data-test 为 baidu-cookie-help / baidu-cookie-help-toggle / baidu-open-login。
- 帮助块默认收起的理由：表单本身已经很长，回来换 Cookie 的人不想先读一遍步骤。

### 6.3 i18n

- 新增 settings.remote.baidu.help.* 共 10 键（toggle / openLogin / step1..step5 / noteRequired / noteExpiry / notePrivacy），17 个语言包全量写入。
- 写入脚本：.superpowers/sdd/i18n-baidu-help.cjs（离散键，不用数组值 —— 对齐门禁会把数组值当违规）。
- 校验脚本：.superpowers/sdd/baidu-help-i18n-check.cjs（核对 17 包齐 10 键、且值里不含 { } 以免被 vue-i18n 当插值）。
- ⚠️ hy（亚美尼亚语）这 10 键暂以**英文**回填，与该包既有 settings.remote.action.add（本就是英文 Add connection）一致，留作后续本地化跟进项。

### 6.4 门禁实测

- npx vitest run src/localization/__tests__/messages-key-alignment.test.ts → Test Files 1 passed / Tests 18 passed。
- npx vitest run src/modules/settings/ui/categories/remote/baidu-connection-form.test.ts → Tests 8 passed（新增 3 条判别性用例：展开后 li 恰为 8、点按钮调用 openUrl("https://pan.baidu.com/")、ftp 协议下帮助块与 hint-cookie 均不存在）。
- npx eslint（connection-form.vue + 该测试文件）→ 0 problems。

### 6.5 真机复验（2026-11-08 14:52，打包产物）

靶子：`src-tauri/target/release/alpha-file-manager.exe`，mtime 2026-11-08 14:41:28，81992704 字节（**不是 dev 构建**）。探针：`.superpowers/sdd/baidu-help-accept.ps1`，判定文件 `baidu-help-accept.txt`，日志 `baidu-help-accept-run3.log`。

| 断言 | 实测 |
| --- | --- |
| 帮助块已渲染 | `HELP_TOGGLE_PRESENT=1` |
| 默认收起 | `COLLAPSED_STATE_LOGIN_VISIBLE=False`（收起态下「打开百度网盘登录页」不在 UIA 树里，符合预期） |
| 点击送达并展开 | `HELP_TOGGLE_MOUSE METHOD=mouse_event CENTER=1642,1346` → `AFTER_MOUSE LOGIN_VISIBLE=True POLL_ITER=1 TREE_COUNT=151→162` |
| 步骤文案进树 | `STEP_PROBE='开发者工具' HITS=1 FIRST=按 F12 打开开发者工具，切到「网络 / Network」面板。` |
| 外链按钮可点 | `OPEN_LOGIN RECT=1571,1389 214x43` / `OPEN_LOGIN METHOD=InvokePattern` |
| 唤起浏览器 | `BROWSER_WINDOWS_BEFORE=0` → `BROWSER_WINDOWS_AFTER=1 NEW=135310/154908`（侧证，非硬证据） |

像素证据：`baidu-help-crop-expanded.png`（从整窗 `baidu-help-after.png` 3862x2110 裁出）肉眼可读——「打开百度网盘登录页」+ 5 步 + 3 条注意全部渲染，中文无 mojibake，下方紧接「初始目录」，说明帮助块插在密码行之后**没有打断 `v-if`/`v-else-if` 链**。

探针踩到的两个坑（后续同类验收直接抄结论）：

- `CollapsibleTrigger` 渲染出的 DOM 元素在 UIA 里**不支持 `InvokePattern`**（`INVOKE_FAIL=…不支持的模式。`），而表单里的普通 `<Button>`（外链那颗）**支持**，同一棵树里两种控件口径不同。
- 旧退路用 `EnumWindows` 找 `Chrome_RenderWidgetHostHWND` 恒为 0 —— 那是**子窗口**，`EnumWindows` 只枚举顶层。可行路径是真实鼠标注入（`AttachThreadInput` + `SetForegroundWindow` + `SetCursorPos` + `mouse_event` 0x0002/0x0004），或 `WindowFromPoint` 命中窗口后按客户区 `lParam` 投 `WM_LBUTTONDOWN/UP`。
- 开关型控件的退路必须**先确认上一次点击没生效**再点第二次，否则第二次会把面板点回收起。

### 6.6 仍未做

- 应用内自动抓 Cookie 的方案 T1（新建 webview 窗口 + `cookies_for_url`）已在 docs/2026-11-07-baidu-t1-auth-findings.md 的 `VERDICT_U1: PASS` 证过可行，**但尚未接线**，是否落地待用户确认。本轮按用户原话只做了「链接 + 详细说明」。
- 「扫描进行中切换导航不再未响应」的真机复验仍未做，需要 14–54 分钟的全量扫描窗口。
- 「扫描进行中切换导航不再未响应」的真机复验仍未做（需要 14–54 分钟全量扫描窗口，会占用用户机器）。
