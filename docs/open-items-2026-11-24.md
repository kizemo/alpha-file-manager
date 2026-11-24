# 未完成事项台账（2026-11-24 起）

> **这份文件是什么**：跨批次的「还没做完 / 还没决定」的唯一账本。
> **怎么用**：每轮收工前进账（状态有变化就改行，不要另建一份）；开工前先读 §0 + §2 + §3。
> **本轮依据**：批次 45 收口（B-1 闭合 + W-7/W-8 落地）→ **批次 45 续篇**（云盘索引配额归因与修复 + O-4 路线 A 启动）→ **批次 45 收尾**（O-4 闭合 + 配额落地 + B-1「另一半」A/B 闭合）。详见 `docs/handoff-2026-11-22-batch38-onedrive-readonly.md` §十六 / §十七 / §十八 / §十九。
> **最近更新**：2026-11-24 深夜（批次 46：§6-1 / §6-2 闭合，§6-3 推进到「等用户扫码」）

---

## 0. 一句话结论

**批次 46** 把 §6 的前两项做完了：**§6-1 闭合** —— 批次 45 的配额改动 + 两份台账已提交 `613e1958`；
**§6-2 判闭合** —— 在**干净包**（无 CDP 旗标）里用应用自带的只读 MCP 做 A/B：百度勾选命中 `count=13`（含 2 条远程），
夸克未勾选 `count=0`，而**盘上索引里夸克条目有 49 条** ⇒ 过滤确实发生在检索期；
唯一没取证的是「用鼠标点一次复选框」这个**动作**，属装置限制（沙箱无键盘），不是产品缺陷。
**§6-3 止步于用户扫码**：登录入口已定位在 `阿里云盘「编辑」→ 打开登录窗口`（**不在远程页上**），
扫码窗 `阿里云盘 登录` 已开在交互桌面（pid 603288），等用户用阿里云盘 App 扫。
仍然开着 **5 项 BLOCKED**：B-2 / B-3 / B-4 / B-5 / B-6 —— 后三项**技术通道已通、登录入口也已通**，
共同前置只剩「用户扫一次码」。
观察项 **11 项**（W-1…W-11，本轮未新增）；已拍板不做 **4 项**。
批次 42 的 9 项已知限制已全部关闭。

---

## 1. 已闭合（留档，防重开）

| ID | 项 | 闭合于 | 靠什么闭合 | 证据 |
|---|---|---|---|---|
| **O-4** | 界面级验证通道（半通 → 闭合） | **批次 45 收尾** | **路线 A**：临时给 `tauri.conf.json` 注入 `additionalBrowserArgs` 打**验收专用包** → Node 内置 `WebSocket` 直连 CDP `45777`（不装 npm 依赖）→ **三档能力全部实测拿到**：① console 65 条 / 0 exception ② `window.__TAURI_INTERNALS__.invoke` 可直调后端命令 ③ 页面内可点勾选框（`aria-checked` true↔false 实测切换）。**收尾义务已履行**：旗标回滚 + 干净包重打（`BUILD_EXIT=0`；干净包 46,078,103 B / SHA256 `3DA2C149…F43CF`；**exe / 安装包 / 装机件三件旗标校验全 `False`**；应用重启后 45777 不再监听） | handoff §十九·2/§十九·6、`.superpowers/sdd/b45/cdp-probe-1.txt`、`check-cdp-flag.ps1`、`b45-build-clean.log` |
| **B-7** | 云盘索引 20k 上限把第二个连接饿死 | **批次 45 收尾** | 配额改为「每连接 20k + 合计随连接数伸缩」→ 真机重建后盘上索引 **21870 条**（百度 20000 + 夸克 1870），**第二个连接的条目真的进了索引** | handoff §十九·4、盘上 `remote-index\meta.json`（`opstamp=21871`） |
| **B-1** | 勾选变更是否**即时**生效（真机，两半口径） | **批次 45 / 45 收尾** | **前半**（用户手工）：取消勾选百度 → 百度独有文件名立刻搜不到（阴性）+ 重勾 → 能搜到（阳性），并由盘上索引非空排除假通过。**后半**（A/B 对照，批次 45 收尾）：索引恒定 21870 条，**只切连接表勾选标记** → 夸克独有文件名 **0 命中（未勾）→ 2 命中（勾上）→ 0 命中（复原）**，同段百度对照命中 `remote:0f0696ba…` 证明索引非空、链路正常 | handoff §十七·1–3、§十九·5，`.superpowers/sdd/b45/b1-q-{A,B,C}.json` |
| O-8 | 剪贴板复制文件被文件占用中断 | 批次 43 | `clipboard.rs` 改「尽力复制」+ `ShCopyOperation` 加 `FOF_SILENT \| FOF_NOCONFIRMATION \| FOF_NOERRORUI`，3 个单测锁位 | `src-tauri/src/clipboard.rs`、handoff §十三·1/§十三·4 |
| O-7 | 添加网盘的「添加」按钮选中不明显 | 批次 43 | 按下态：边框 `--accent` 2px + 底色 `rgba(59,130,246,.14)` + 外发光；聚焦环同步加深。**未走 `scale(.97)`**（与既有 `.afm-primary:active` 冲突，见 `style.css:356-358`） | `src/themes/glass-theme.css:393-417`、handoff §十三·2 |
| O-5 | 批次 42 + 43 提交未推送 origin | 批次 44 | `git -c http.proxy= -c https.proxy= push origin main` → `453f20e8..af1add65`；**全局代理 127.0.0.1:7897 必须临时禁用**，否则 remote-curl 中断 | handoff §十六·6、`git ls-remote origin main` |
| — | OneDrive 已连通账户的旧数据 | 批次 42 随下线一并删 | 用户拍板「彻底删除」；真实 `%APPDATA%\com.sigma-file-manager.app` 清理实测 `existed=False` | handoff §十二·2、`§十二` 表 |

---

## 2. OPEN —— 可推进，缺条件或需用户拍板

**本节暂无「缺条件即可推进」的技术项。** 剩下的都要么等用户操作，要么等用户拍板：

| 待拍板 / 待处置 | 现状 | 下一轮需要谁做什么 |
|---|---|---|
| **云盘索引配额最终口径** | 当前实现 = 选项 **A「每连接 20k + 合计随连接数伸缩」**（推荐项，真机已验证）。问卷（`ask_568ca9f9c963d964b59cdb27`）未答复 | **用户**拍板：沿用 A，还是改成「全局 60k 先到先得」/「分页续扫按需扩索引」 |
| ~~**O-4 收尾：干净交付包**~~ | **已闭合**：旗标回滚 → 干净包 `BUILD_EXIT=0` → exe / 安装包 / 装机件三件旗标校验全 `False` | 见 §1 的 O-4 行 |
| **版本号与 CHANGELOG** | 3.22.0 之后未定 | **用户**定版本号（3.23.0？）、是否本批发版 |
| **本机 `stable` 工具链损坏** | 1.97.1 编译炸数百条假错误；cargo 缓存报 `file is not a database` code 26 → 门禁只能借 1.90.0 | **用户**决定是否允许跑 `rustup` 修复 / 回滚（仓库外系统状态，本会话未动） |

---

## 3. BLOCKED —— 等外部条件

| ID | 项 | 等什么 | 恢复条件 |
|---|---|---|---|
| B-2 | 阿里云盘「登录成功但拿不到 unionId」缺陷（批次 41 登记，未修） | 上游 400 不可复现，需再次复现时抓请求体 | 用户再次遇到 → 提供 `afm-aliyun-login.log`（`src-tauri/src/remote/aliyun_auth.rs:880` 写的旁路日志）。**批次 45 收尾实测阿里云盘仍 HTTP 400（鉴权失效）** |
| B-3 | 登录态过期检测 / 重登提醒 | **O-4 已通**（不再被技术通道阻塞） | 需用户在应用内**重新授权阿里云盘**（当前 400），再做界面级真机验收 |
| B-4 | 「先下结论后被推翻」复发核查（批次 40 19:16 一次） | 过程性，需再观察若干批次 | 连续多轮无同类返工即降级为「已改善」。**批次 45 续篇本身就是一次复发**（曾判「写盘失败」）⇒ 观察期**重新开始** |
| B-5 | 云盘下载落地的杀毒软件超时 | **O-4 已通** | 需一次真实下载动作触发（用户侧） |
| B-6 | 三个网盘真实账户的界面级验收 | **O-4 已通** | 阿里云盘重新授权后，三家各做一轮界面级验收 |

> 批次 45 收尾备注：**O-4 闭合后 B-3 / B-5 / B-6 的「技术阻塞」已解除**，
> 现在只剩「阿里云盘鉴权失效（HTTP 400）」这一个共同前置 —— 需用户在应用内登录一次。
>
> **批次 46 备注（登录入口已定位，扫码页已开到用户屏幕上）**：`设置 → 远程 → 网盘` **页上没有**「打开登录窗口」按钮，
> 入口在**阿里云盘那一行的「编辑」弹窗内**（三个已存连接只有「编辑 / 删除」）。本轮已按此路径把
> `阿里云盘 登录` 窗（`Tauri Window`，`264,0,1949×1468`）开到扫码页（可见 `扫码登录` / `请用 / 阿里云盘 App / 扫码`），
> 并确认编辑弹窗内有 `Text|会话已过期，需要重新登录` —— 这就是 B-3 / B-5 / B-6 的**界面侧根因**。
> 现场复核（22:50）：应用日志末条仍是本地 20:39:32 的 `InvalidParameter.ClientId` 400、`ALIYUN_LINES=4` ⇒ **用户尚未扫码**。
> 扫码后若仍 400（即 B-2），需用户提供 `afm-aliyun-login.log`（`src-tauri/src/remote/aliyun_auth.rs:880` 的旁路日志）。
>
> **收尾复核（22:54）—— 订正上面的现场描述**：`阿里云盘 登录` 窗**已不在顶层窗列表里**了
> （进程 603288 仍在，只剩主窗 `Alpha File Manager` + 两个 24×24 辅助窗），
> 且应用启动后（UTC 13:43:48 起）的 417 行日志里**零 `aliyun` 记录** ⇒ 用户**尚未扫码**，登录窗是被关掉的。
> 重开只要两步、可重复跑（本轮均 `EXIT=0`）：
> `.superpowers\sdd\b46\aliyun-open-edit.ps1` → `.superpowers\sdd\b46\aliyun-open-login.ps1`。

---

## 4. WATCH —— 已知但不阻塞（顺手可修，或已决定暂不动）

| ID | 项 | 现状 | 取舍 |
|---|---|---|---|
| W-1 | `remote/commands.rs` 文件体积 | **实测 8334 行**（批次 44 订正；原记「约 3.0k」是批次 42 的行数，已过期） | 暂不拆 —— 拆分属重构，需单独批次 |
| W-2 | `remote_client.rs` 的 `#[allow(dead_code)]` | 未逐条核实是否仍被需要 | 动 OneDrive 相关代码时顺带评估，不专门开批次 |
| W-3 | CDP 探针脚本 `E:\...\maintenance\cdp\probe.ps1` 报 `SESSION_NOT_FOUND` | **批次 44 已定位根因**：脚本按 `--app=data:text/html` 找 Edge **新标签页**进程，本项目走 WebView2，没有该标签页 → 探针口径与本项目的形态不匹配，不是应用缺陷。**未修**（属共享 maintenance 目录，非本仓库） | 本仓库不受影响；要复用探针得改成按 CDP 端口筛进程 |
| W-4 | 360 杀毒误报安装包 | 用户已添加信任，长期需观察新版本是否复发 | 每次升版装机后留意 |
| W-5 | `tauri dev` 下设置页内容区空白 | 路由页需真实应用上下文，dev 拿不到 | **已在 AGENTS.md §0 明写「dev 不能用于界面验收」**，防再踩 |
| W-6 | 本仓库不是 GitHub Actions 仓库 | 远端只有 `gitee` | CI 状态栏无从显示，属事实而非缺陷 |
| W-7 | **云盘索引重建的「完成」时机在日志里不可判** | **已落地**（批次 45 后半段，handoff §十七·9）：重建命令现在有 入口「N 个连接 + ids」/「收集完成 N 条 collect=Xms」/「提交完成 N 条 commit=Xms 目录」/「重建完成 写入 N 条 总耗时 Xms」四段 INFO，提交失败为 WARN | 「重建完了没有」当场可从日志回答，不必再用段文件 + mtime 反推 |
| W-8 | **检索链路（查询侧）零日志** → 勾选过滤是否生效，无法从应用日志证实 | **已落地**（批次 45 后半段，handoff §十七·9）：索引目录不存在 → 一次性 INFO；索引打不开 → WARN；schema 不符 → 保留 WARN；勾选集为空 → 一次性 INFO。三处返回值语义（`Option` / `Some(vec![])`）**一字未改** | 「搜不到」与「索引是空的」与「没勾任何云盘」三者**在日志里已可区分** |
| **W-9** | **后端 `global_search_index_remote_connections` 没有互斥锁** | 批次 45 收尾坐实：12:41:28 与 12:49:29 两次重建**并发**抢同一索引目录，导致中途采样看到 `segments: []` 的「假空索引」。前端只有模块级串行锁，**CDP 直调 / 任意后端调用会绕过它** | 登记观察项，**本轮不改代码**。要做就是给后端重建加一把 `Mutex`（跨连接共用一把，不是每连接一把） |
| **W-10** | 「云盘索引达到上限」警告**按目录刷屏** | 批次 45 收尾实测：百度触顶后，每跳过一个目录就打一条 WARN，21:12–21:21 那次重建刷了数十条 | 不影响正确性，但会淹没关键日志。建议改成「每个连接只打一次」或降级为 DEBUG |
| **W-11** | CDP 探针回包**延迟约 2 分钟** | 批次 45 收尾实测：`cdp-eval.cjs` 发出 `Runtime.evaluate` 后，页面侧命令**约 120s 后**才真正执行（应用日志的 `query timings` 时间戳晚 120s），且 `--timeout 120000` 一到就回包 | 装置层问题，不是产品缺陷。**注意**：别把这个延迟误判成「后端慢」—— 同一轮实测后端 `query timings: total=15ms`。下一轮做 CDP 交互时**必须给足超时**（≥150s） |

---

## 5. 已拍板不做（含理由，防重开）

| 项 | 拍板 | 理由 |
|---|---|---|
| OneDrive 个人模式 | 不做 | 批次 41 用户决定「仅企业版不支持」。`remote_client.rs:375-389` **本就只发 `scope: 'https://graph.microsoft.com/User.Read offline_access'`，未请求 Files.ReadWrite** → 该项已被代码现状满足，无需改动 |
| 多连接混合检索的排序策略 | 暂不做 | 用户「暂时不管」。现状：全局检索结果**未按 `connection_id` 分组、无跨连接去重**，排序按各连接内部顺序合并。要改需先定产品规则（按相关度？按连接？按路径？），无规则不动 |
| `search_files.rs` 死代码清理 | 延后 | 用户「可以暂时不处理」。当前仍有测试引用，删除需连带改测试，属独立清理批次 |
| 扫描期间冷启首查返回空结果 | 延后 | 用户「可以暂时不处理」（批次 41 实测坐实：扫描未完成时冷启首查返回空，再查一次正常） |

---

## 6. 下批次（批次 47+）建议顺序

> **批次 46 结算**：原第 1 项（提交配额改动 + 两份台账）⇒ **闭合**，提交 `613e1958`（4 files / +485 / −94）；
> 原第 2 项（干净包真机复验勾选链）⇒ **判闭合**，证据见 §0 与 handoff §二十·2；
> 原第 3 项（阿里云盘重新授权）⇒ **推进到扫码边界**，卡在用户扫码，见 §3 的批次 46 备注。
> **本批次未改任何源码**，故门禁用例数与批次 45 续篇逐字一致。

1. **等用户扫码**（原第 3 项）⇒ 扫码成功后立刻做三件事：① 复测 `search_files` 能否命中阿里云盘条目
   ② 若仍 400，收 `afm-aliyun-login.log` 处理 B-2 ③ B-3 / B-5 / B-6 的界面级验收随之解冻。
2. **W-9**（后端重建无互斥锁）作为独立小批次做 —— 一把**跨连接共用**的 `Mutex` + 单测。
   这是剩余项里**唯一纯代码、不依赖用户**的活，应优先。
3. **W-10**（上限警告按目录刷屏）顺手改：每个连接只打一次。
4. **W-4**（360 误报）随新包装机复验。
5. B-2 等用户再次遇到阿里云盘 400；期间不动代码。
6. **待用户处置（仓库外系统状态）**：本机 `rustup` 的 `stable` 已损坏 → 是否允许跑 `rustup` 修复 / 回滚？
   不修则后续 `cargo test --lib` 与 `tauri:build` 都只能借 1.90.0 工具链。
7. **待用户拍板后才动**：版本号（3.23.0？）与 CHANGELOG；云盘索引配额最终口径（§2 前两行）。

---

## 7. 证据路径

> AGENTS.md 的 `§0.3 验证装置地图` 已补上这三类装置的正确用法与「`tauri dev` 不能用于界面验收」警示；本节只留本批次的具体取证指针。

- **批次 46（§6-1 提交 / §6-2 干净包 MCP 复验 / §6-3 登录入口定位）**：
  - `.superpowers/sdd/b46/commit-msg.txt` —— §6-1 提交信息正文（**明写版本号与 CHANGELOG 未动**）
  - `.superpowers/sdd/b46/cargo-test-lib.log` —— `1693 passed / 0 failed / 3 ignored`（借 1.90.0 工具链）
  - `.superpowers/sdd/b46/npm-check.log` —— `npm run check` EXIT=0
  - `.superpowers/sdd/b46/mcp-client.ps1` —— **应用自带只读 MCP 的 HTTP 客户端**（`-Mode list|call`，须 pwsh 7）
  - `.superpowers/sdd/b46/mcp-tools-list.log` —— `mode=readOnly` + 6 个只读工具
  - `.superpowers/sdd/b46/mcp-search-baidu-on.log` —— 百度已勾选 `count=13`，含 2 条 `sfm://remote/0f0696ba…/`
  - `.superpowers/sdd/b46/mcp-search-quark-off.log` —— 夸克未勾选 `count=0`（**判别性反证**：盘上索引里夸克条目 49 条）
  - `.superpowers/sdd/b46/aliyun-dump.ps1` / `aliyun-open-edit.ps1` / `aliyun-open-login.ps1` —— §6-3 三步（页面转储 → 点开「编辑」→ 调「打开登录窗口」）
  - `.superpowers/sdd/b46/aliyun-edit-labels.json` / `aliyun-login-labels.json` —— 元素匹配表（**ASCII 脚本 + UTF-8 中文标签分离**，绕开脚本必须纯 ASCII 的约束）
  - `.superpowers/sdd/b46/accept-aliyun\{dump,before-edit,after-edit,after-edit-hits,login-win-0,login-windows}.txt` —— UIA 原始转储
  - `.superpowers/sdd/b46/state-check.out.log` —— 22:50 现场复核（进程 603288 / 扫码窗在 / 仍 400）
  - `.superpowers/sdd/b46/handoff-s20.md` + `append-s20.cjs` —— handoff §二十 片段与**幂等**追加脚本
  - ⚠️ **废弃脚本，勿复用**：`aliyun-login-go.ps1`（v1/v2）、`aliyun-invoke-login.ps1`、`aliyun-login-drive.ps1 -ClickLabel`
    —— 四条都是在「远程页上有登录按钮」这个**错误模型**上写的，失败原因不是脚本 bug 而是模型错
- **批次 45 收尾（O-4 / 配额 / B-1 后半）**：
  - `.superpowers/sdd/b45/handoff-s19.md` —— 本轮 handoff 章节正文（追加进 `docs/handoff-2026-11-22-batch38-onedrive-readonly.md` §十九）
  - `.superpowers/sdd/b45/b1-ab.ps1` —— **B-1 A/B/C 三段编排**（改勾选集合 → 查询 → 复原）
  - `.superpowers/sdd/b45/b1-rebuild.js`（后端直调重建 + 探针）、`b1-query.js`（纯查询探针）、`b1-toggle.cjs`（改连接表勾选标记并回读校验）
  - `.superpowers/sdd/b45/b1-q-A.json` / `b1-q-B.json` / `b1-q-C.json` —— **A/B/C 三段查询原始回包**
  - `.superpowers/sdd/b45/cdp-probe-1.txt` —— CDP 直连证据（4 targets / console 65 条 / 0 exception）
  - `.superpowers/sdd/b45/cdp-eval.cjs` —— Node 内置 WebSocket 直连 CDP 的求值器（`--port --expr-file --await --timeout`）
  - `.superpowers/sdd/b45/check-cdp-flag.ps1` —— **交付包旗标判据**（对照 `afm-cdp.exe` 含 / `afm-plain.exe` 不含）
  - `.superpowers/sdd/b45/build-clean.cmd` —— 回滚 + 干净包构建（自带 `git checkout` + `findstr` 守卫，残留即 `exit /b 7`）
  - `.superpowers/sdd/b45/b45-build-clean.log` —— 判 **`BUILD_EXIT=`**
- **批次 45 续篇（配额 + O-4 路线 A 启动）**：
  - `.superpowers/sdd/b45/state-batch45.md`、`state-batch45b.md`（本轮状态落盘）
  - `.superpowers/sdd/b45/quota-change.diff`（`remote_index.rs` + `commands.rs` 完整 diff）、`git-diff-stat.txt`、`git-status.txt`
  - `.superpowers/sdd/b45/cargo-test-lib.log`（`cargo test --lib`：**1693 passed / 0 failed / 3 ignored**）
  - `.superpowers/sdd/b45/npm-check.log`（`npm run check`：EXIT=0）
  - `.superpowers/sdd/b45/patch-cdp.cjs`、`build-accept.cmd`、`b45-build.log`（判 **`BUILD_EXIT=`**）
  - `.superpowers/sdd/b45/setup-backup-Alpha File Manager_3.22.0_x64-setup.exe`（构建前的 3.22.0 安装包备份）
  - `.superpowers/sdd/b45/state-now.txt`（12:55 快照，**假空索引**）、`index-now.txt`、`state-b45c.txt`（13:08 快照）、`conn-table.txt`、`tantivy-probe.txt`
- **云盘索引配额根因（原始证据）**：
  - 盘上 `%APPDATA%\com.sigma-file-manager.app\remote-index\meta.json`（顶层键 `segments` / `opstamp`；**21870 条 ⇒ `opstamp=21871`**）
  - 应用日志 `%LOCALAPPDATA%\com.sigma-file-manager.app\logs\Alpha File Manager.log` 里 `global_search::remote_index` 的全量行
    **注意应用日志时间戳是 UTC（本地 = UTC+8）**，过滤用 ASCII 模式（模块名 `global_search::remote_index`），不要用 CJK 字面量匹配。
  - 连接表：`%APPDATA%\com.sigma-file-manager.app\user-data\user-remote-connections.json`；凭据 `remote-credentials.json`（`version|entries`）
- **批次 45 索引取证**：`.superpowers/sdd/b44/rebuild-final.{ps1,txt}`、`.superpowers/sdd/b44/applog-remote-index.{ps1,txt}`
- **批次 44 UIA 证据**：`.superpowers/sdd/b44/uia-report.txt`、`.superpowers/sdd/b44/uia-observe.txt`、`.superpowers/sdd/b44/uia-*.ps1`、
  `.superpowers/sdd/b44/install-status.txt`、`.superpowers/sdd/b44/check3.log`、`.superpowers/sdd/b44-cargo-test.log`、
  `release/Alpha File Manager_3.22.0_x64-setup.exe`（**46,151,697 B / SHA256 `4AB5032B…D470`**）。
  ⚠️ `release/sigma-file-manager_3.22.0_x64-setup.exe` 是**错指针**（该文件不存在），已订正。
- **门禁**：`npm run check` / `npm run test:run` / `npm run verify:all` / `cargo test --lib` / `npm run tauri:build`。
  批次 44 基线：**334 文件 / 3597 用例全绿**、`cargo test --lib` **1691 passed / 0 failed / 3 ignored**、
  `verify-changelog` **67 条目**、`cargo clippy --all-targets -- -D warnings` 见 §十六·3。
  批次 45 续篇：`cargo test --lib` **1693 passed / 0 failed / 3 ignored**（+2 = 本轮新增的两条配额回归单测）。
- **批次 45 改动落点**：`src-tauri/src/global_search/{remote_index,commands}.rs`
  （常量 `REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION`、`total_entry_limit()`、`walk_should_continue()`、
  `collect_remote_entries()`、`walk_connection()`、两条新增单测）。**仍未提交**（工作区改动）。
- **批次 44 改动落点**：`src-tauri/src/global_search/{remote_index,state,index,query,commands}.rs`、
  `src-tauri/permissions/app-commands.toml`、`src/modules/settings/ui/categories/search/search-remote-index-section.vue`（+ 新测试）、
  `src/composables/use-remote-global-search-index.ts`（+ 新测试）。对应提交 `1b0b32e3`。
- **批次 43 改动落点**：`src-tauri/src/clipboard.rs`、`src-tauri/src/commands.rs`（`spawn_detached`）、
  `src-tauri/build.rs`、`src/themes/glass-theme.css`、`src/modules/dialogs/ui/dialogs-connection-panel.vue`、
  `src/modules/dialogs/ui/dialogs-layout.vue`、`src/modules/settings/ui/categories/search/search-remote-index-section.vue`。
  对应提交 `f1d753b5`。
- **批次 42 证据**：`.superpowers/sdd/b42-receipt-*.txt`、`b42-final-*.txt`、`b42-*.log`、
  `E:\Users\Duanyi\Desktop\sigma-file-manager_3.20.0_x64-setup.exe`（批次 42 包）
- **批次 43 证据**：`.superpowers/sdd/b43-receipt-*.txt`、`b43-final-*.txt`、`b43-*.log`、
  `release/Alpha File Manager_3.21.0_x64-setup.exe`（批次 43 包）

---

## 变更记录

| 日期 | 改动 |
|---|---|
| 2026-11-24 | 建档：收口批次 42 的 9 项已知限制 + 批次 41 遗留 + 批次 42 交接里的 3 个待拍板 |
| 2026-11-24 | 批次 43 收口：O-7 / O-8 闭合（commit `f1d753b5`）；O-4 从「全阻塞」改判为「半通」并拆出 B-6；O-5 范围扩为批次 42+43 |
| 2026-11-24 | 批次 44 收口：**O-1 / O-2 / O-3 / O-6 闭合**，O-5 仅剩推送；升版 3.22.0（`1b0b32e3`）、收口 `af1add65`；装机实测 + 单测坐实 O-3 的「首次升级不重建」边界，**CHANGELOG 补了「升级后需打开设置页」的说明**；UIA 改判「可用但只读」；W-1 行数订正、W-3 根因定位；§5 新增 2 项用户拍板；§6 换成批次 45 顺序；handoff 新增 §十六 |
| 2026-11-24 | 批次 44 续：**O-5 闭合**（推送 `453f20e8..af1add65`，commit `8191bf1b`，需临时禁用全局代理 127.0.0.1:7897）。OPEN 归零；§6 重写为批次 45 待办 |
| 2026-11-24 | **批次 45**：**B-1 闭合**（用户手工验收 + 盘上索引反证，详见 handoff §十七）；O-4 补「下一步二选一」待拍板 + 新增第三条取证渠道（应用日志）；**新开 W-7（重建完成时机日志不可判）/ W-8（查询侧零日志，「搜不到」与「索引空」不可区分）**；§6 重排为「W-7+W-8 优先 → O-4 拍板」；§7 增补批次 45 证据路径 |
| 2026-11-24 | **批次 45 后半段**：**W-7 / W-8 落地**（云盘索引重建 + 查询侧取证日志，纯日志补丁不动语义）。前端 check EXIT=0（334 文件 / 3597 用例）、cargo test --lib EXIT=0（1691 passed / 0 failed / 3 ignored，与基线一致）；本机 stable 1.97.1 工具链已损坏，门禁须借 1.90.0（姿势见 handoff §十七·9-6）。详见 handoff §十七·9 |
| 2026-11-24 | **批次 45 续篇**：**新开 B-7（云盘索引 20k 上限是「所有连接合计」的全局配额，百度吃满后阿里云盘/夸克一条都没进索引）**；配额改为**每连接 20k + 合计随连接数伸缩**（`remote_index.rs` / `commands.rs`，+135/−26，**未提交**），两条新增回归单测；**O-4 路线 A 定案并启动**（`tauri.conf.json` 临时注入 `additionalBrowserArgs` → 验收专用包构建中，收尾必须回滚重打包）；门禁：cargo test --lib **1693 passed / 0 failed / 3 ignored**、npm run check EXIT=0；§7 **订正错指针**（`sigma-file-manager_3.22.0_x64-setup.exe` → `Alpha File Manager_3.22.0_x64-setup.exe`，46,151,697 B / SHA256 `4AB5032B…D470`）；§6 重排为批次 46+ 顺序。详见 handoff §十八 |
| 2026-11-24 | **批次 45 收尾**：**O-4 闭合**（路线 A 实测：console 65 条/0 exception + `invoke` 直调后端 + 界面勾选框可点，handoff §十九·2）；**B-7 闭合**（真机重建 21870 条 = 百度 20000 + 夸克 1870，`opstamp=21871`）；**B-1 后半闭合**（同一索引、只切勾选集合的 A/B 对照：夸克独有文件名 **0 → 2 → 0 命中**，百度对照同段命中 `remote:0f0696ba…`）；**新开 W-9（后端重建无互斥锁）/ W-10（上限警告按目录刷屏）/ W-11（CDP 探针回包延迟约 120s）**；B-3 / B-5 / B-6 的技术阻塞解除，只剩「阿里云盘重新授权」一个共同前置；§2 改为「待拍板清单」。详见 handoff §十九 |
| 2026-11-24 | **批次 46**：**§6-1 闭合**（提交 `613e1958`，4 files / **+485 / −94**，含批次 45 的 `global_search/{remote_index,commands}.rs` 配额改动 + 两份台账；**版本号与 CHANGELOG 未动**）；**§6-2 判闭合**（干净包**无 CDP 旗标**下改用应用自带只读 MCP 做 A/B：百度已勾选 `count=13` 含 2 条 `sfm://remote/0f0696ba…/`、夸克未勾选 `count=0`，而盘上索引里夸克条目 **49 条** ⇒ 过滤在**检索期**生效，非假阴）；**§6-3 推进到扫码边界**（**登录入口在「编辑」弹窗内，不在远程页上** —— 这是连续两轮失败的真根因；`阿里云盘 登录` 窗已开到扫码页，等用户扫码；弹窗内 `会话已过期，需要重新登录` = B-3/B-5/B-6 界面侧根因）；§3 增补登录入口说明、§6 重排为批次 47+、§7 增补 b46 证据块；**未改任何源码**，门禁用例数与批次 45 续篇逐字一致。详见 handoff §二十 |
