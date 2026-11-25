# 未完成事项台账（2026-11-24 起）

> **这份文件是什么**：跨批次的「还没做完 / 还没决定」的唯一账本。
> **怎么用**：每轮收工前进账（状态有变化就改行，不要另建一份）；开工前先读 §0 + §2 + §3。
> **本轮依据**：批次 45 收口（B-1 闭合 + W-7/W-8 落地）→ **批次 45 续篇**（云盘索引配额归因与修复 + O-4 路线 A 启动）→ **批次 45 收尾**（O-4 闭合 + 配额落地 + B-1「另一半」A/B 闭合）。详见 `docs/handoff-2026-11-22-batch38-onedrive-readonly.md` §十六 / §十七 / §十八 / §十九。
> **最近更新**：2026-11-25 深夜（批次 55：**W-16 闭合** + 新开 **W-18**（云盘目录作用域检索口径待用户拍板）/ **W-19**（界面级像素复验通道受阻））

---

## 0. 一句话结论

**批次 46** 把 §6 的前两项做完了：**§6-1 闭合** —— 批次 45 的配额改动 + 两份台账已提交 `613e1958`；
**§6-2 判闭合** —— 在**干净包**（无 CDP 旗标）里用应用自带的只读 MCP 做 A/B：百度勾选命中 `count=13`（含 2 条远程），
夸克未勾选 `count=0`，而**盘上索引里夸克条目有 49 条** ⇒ 过滤确实发生在检索期；
唯一没取证的是「用鼠标点一次复选框」这个**动作**，属装置限制（沙箱无键盘），不是产品缺陷。
**§6-3 判闭合（真机实测）** —— 上一轮其实**已经登录成功**，缺的只是再按一下弹窗底部的「保存」：
按下后凭据库 mtime `11:37:45 → 23:03:01`、会话摘要 **`剩余0秒 → 剩余6776秒`**，
再用弹窗自带「测试连接」做真实服务端往返，得到「**阿里云盘连接正常，账号：段燚**」。
**「扫码登录」按钮属废路线**（Rust 自造 `generate.do/query.do` 实测卡在 `NEW`），
正确入口仍是「编辑 → 打开登录窗口」，且登录后**必须点保存**才落盘（详见 handoff §二十·9）。
仍然开着 **5 项 BLOCKED**：B-2 / B-3 / B-4 / B-5 / B-6 —— 但 **B-3 / B-5 / B-6 的授权前置已解除**，
可直接进入界面级验收；只剩 B-2（refresh 仍 400 `InvalidParameter.ClientId`）无解，
且本次 access_token 约 **6776 秒**后会过期，届时仍需再登一次。
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
| **§6-3** | 阿里云盘重新授权（批次 46 第 3 项） | **批次 46** | 登录入口在「编辑」弹窗内 → 点「打开登录窗口」网页登录成功 → **再点弹窗底部「保存」**（走 `remote_aliyun_login_claim` 才落盘）。判据：凭据库 mtime `11:37:45 → 23:03:01`、会话摘要 `剩余0秒 → 剩余6776秒`、弹窗自带「测试连接」真实往返得「阿里云盘连接正常，账号：段燚」。**注意**：光「登录成功」不算完，槽位为空时点保存会静默沿用旧凭据 | handoff §二十·9、`.superpowers/sdd/b46/accept-aliyun/S63-CLOSEOUT.txt` |

---

## 2. OPEN —— 可推进，缺条件或需用户拍板

**本节暂无「缺条件即可推进」的技术项。** 剩下的都要么等用户操作，要么等用户拍板：

| 待拍板 / 待处置 | 现状 | 下一轮需要谁做什么 |
|---|---|---|
| **云盘索引配额最终口径** | 当前实现 = 选项 **A「每连接 20k + 合计随连接数伸缩」**（推荐项，真机已验证）。问卷（`ask_568ca9f9c963d964b59cdb27`）未答复 | **用户**拍板：沿用 A，还是改成「全局 60k 先到先得」/「分页续扫按需扩索引」 |
| **云盘目录递归检索口径** | 当前实现 = **明确报错**（批次 55 落的低成本修复：只支持本地目录），「云盘目录作用域递归检索」**尚无功能** | **用户**拍板：走**云盘索引**（快、零 API 配额，但只覆盖已建索引的连接，百度封顶 20,000 条 ⇒ 可能「搜到一半」）vs 走**网盘实时遍历**（完整，但每次按键烧 API 配额）→ 详见 §4 **W-18** |
| ~~**O-4 收尾：干净交付包**~~ | **已闭合**：旗标回滚 → 干净包 `BUILD_EXIT=0` → exe / 安装包 / 装机件三件旗标校验全 `False` | 见 §1 的 O-4 行 |
| **版本号与 CHANGELOG** | 3.22.0 之后未定 | **用户**定版本号（3.23.0？）、是否本批发版 |
| **本机 `stable` 工具链损坏** | 1.97.1 编译炸数百条假错误；cargo 缓存报 `file is not a database` code 26 → 门禁只能借 1.90.0 | **用户**决定是否允许跑 `rustup` 修复 / 回滚（仓库外系统状态，本会话未动） |

---

## 3. BLOCKED —— 等外部条件

| ID | 项 | 等什么 | 恢复条件 |
|---|---|---|---|
| B-2 | 阿里云盘「登录成功但拿不到 unionId」缺陷（批次 41 登记，未修） | 上游 400 不可复现，需再次复现时抓请求体 | 用户再次遇到 → 提供 `afm-aliyun-login.log`（`src-tauri/src/remote/aliyun_auth.rs:880` 写的旁路日志）。**批次 45 收尾实测阿里云盘仍 HTTP 400（鉴权失效）** |
| B-3 | 登录态过期检测 / 重登提醒 | **批次 48 闭合** | 三段证据：检测 = 批次 46 编辑弹窗 UIA 取到 `会话已过期，需要重新登录`；重登 = 2026-11-25 `编辑` → `打开登录窗口` 完成（登录窗 9111918 消失）；恢复 = 重登并保存后重建 23334 条且 MCP 可检索阿里云条目。遗留候选：过期提醒位于折叠线以下（未改 UI） |
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
>
> **闭合复核（23:03–23:10）—— §6-3 已闭合，上面「等用户扫码」的说法作废**：用户 22:56 网页登录成功后，
> 弹窗里出现 `已登录。保存这条连接即可——会话会在保存时写入。`（待认领槽位里已有会话），
> 缺的只是**再按一下底部「保存」**。用「取消」同行 Y 锚定「保存」并 `InvokePattern` 后：
> 凭据库 mtime `11:37:45 → 23:03:01`、会话摘要 `剩余0秒 → 剩余6776秒`、`SAVE_BUTTONS_AFTER=0`（弹窗自关）；
> 再点弹窗自带「测试连接」做真实服务端往返 → `阿里云盘连接正常，账号：段燚`。
> 另：**「扫码登录」按钮属废路线**（`src-tauri/src/aliyun_login.rs` 模块头注释：自造 `generate.do` / `query.do`
> 实测**卡在 `NEW` 永不推进**，缺页面上下文的 `_csrf` / `umidToken` / `hsiz`），
> `.superpowers\sdd\b46\aliyun-qr-start.ps1` 一并废弃。详见 handoff §二十·9。
> **B-2 仍开**：refresh 仍 400 `InvalidParameter.ClientId`（本轮日志无新增），新 token 约 6776 秒后过期。

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
| **W-9** | **后端 `global_search_index_remote_connections` 没有互斥锁** | **已落地**（批次 47，handoff §二十一·1）：`commands.rs` 新增 `static REBUILD_LOCK: tokio::sync::Mutex<()>`（**跨连接共用一把**）+ `acquire_rebuild_lock()`，guard 在**任何文件操作之前**取得；同时到达的第二轮**排队等待**（先打一条 WARN 区分「慢」与「排队」），不报错、不改「返回本次写入条数」的契约。单测 `the_rebuild_lock_never_lets_two_rebuilds_run_at_once` 用 4 线程实测并发峰值 = 1 | 已闭合。前端锁保留（省一次排队），后端锁是**唯一真防线**（CDP 直调也绕不过）。⚠️ 只有单测证据，**真机并发未验**（需界面或 CDP） |
| **W-10** | 「云盘索引达到上限」警告**按目录刷屏** | **已落地**（批次 47，handoff §二十一·2）：逐目录 WARN 改为 `SkippedDirs::record()` 计数，`walk_connection` 结束时**每个连接只打一条**「另有 N 个目录未继续下钻」。日志量从 O(目录数) 降到 O(连接数)，可观测性保留（未降级 DEBUG） | 已闭合。两条单测：汇总口径（`None` / 3 / 含连接 id 与上限 20000 / 单行）+ `describe` 只读不改计数 |
| **W-11** | CDP 探针回包**延迟约 2 分钟** | 批次 45 收尾实测：`cdp-eval.cjs` 发出 `Runtime.evaluate` 后，页面侧命令**约 120s 后**才真正执行（应用日志的 `query timings` 时间戳晚 120s），且 `--timeout 120000` 一到就回包 | 装置层问题，不是产品缺陷。**注意**：别把这个延迟误判成「后端慢」—— 同一轮实测后端 `query timings: total=15ms`。下一轮做 CDP 交互时**必须给足超时**（≥150s） |
| **W-12** | **中文子串检索失效**（本地与云盘一视同仁，查询只能「整段命中」） | 批次 48 界面级验收中实测：`name` 字段用 tantivy `default` 分词器（`global_search/index.rs:40-42`）⇒ **一段连续汉字是一个 term**；查询侧是 `split_query_tokens` + `FuzzyTermQuery(distance=1)`（`global_search/query.rs:40,46-49`）⇒ 与 term 差 2 个字符即不命中。MCP `search_files` 实测（opstamp=23335 / 23334 条）：`技巧`→7 条全百度、`收集`→**全局 0 条**、`心得`→1、`分析`→5，而阿里云 `Midjourney AI 技巧收集.xlsx` 名字里明明含「技巧 / 收集」；ASCII 正常（`Midjourney`→39 含 2 条阿里云、`AI`→260 含 1 条）。证据 `.superpowers/sdd/b48/mcp-probe{,2,3}-*.log` | **已闭合（含真机）**（批次 49，handoff §二十三）：按下列修法实现，**只动查询侧、未改索引 schema ⇒ 23,334 条现有索引无需重建**；不换 CJK 分词器。6 条新单测（`mod cjk_substring`）全绿，`cargo test --lib` **1702 passed / 0 failed / 3 ignored**。**2026-11-25 装机新包后真机 A/B 实测**（应用自带只读 MCP `search_files`，`result_limit=200`，装置 `.superpowers/sdd/b49/w12-probe.ps1`（Tag = before / after）`）：`技巧` 7→**151** 且阿里云 **0→1**（`Midjourney AI 技巧收集.xlsx`）、`收集` **0→15** 阿里云 **0→2**（同一 xlsx + `差评收集表格.xlsx`）、`心得` 1→**21** 阿里云 **0→1**（`Midjourney 使用心得+关键词分析.mp4`）；ASCII 对照 `Midjourney` 39 条 / 阿里云 2 条**逐字不变（无回归）**。⚠️ 残留见 W-13。原始修法：查询侧对含 CJK 的 token 追加一条 `RegexQuery::from_pattern(".*<regex-escaped>.*", fields.name)` 作 `Should` 子查询（`incremental.rs:190-194` 已证 tantivy-fst 是全匹配语义、`.*` 可用），**不改索引格式 ⇒ 不需要重建索引**；**打分侧最终未动**：CJK 子串命中走 token 分支，本就过 `get_min_score_for_query_length` 门槛（单测 `substring_hits_clear_the_length_threshold` 三组实测），**不存在**被 `min_score_threshold` 二次丢掉的问题 |
| **W-13** | **CJK 子串命中在候选池饱和时被挤出**（排序 / 容量层） | 批次 49 真机 A/B 实测（handoff §二十三·4-2 / ·7）：`分析` 这类高频 2 字词候选池直接打满服务端硬上限 500 条（请求 `result_limit=1000` 仍只回 500），靶子 `Midjourney 使用心得+关键词分析.mp4` 不进结果；同一文件改用 `关键词分析`（5 字）稳定命中（`score=0.92`）⇒ 正则路径生效，缺口不在「搜不到」而在「`RegexQuery` 常量分排不过 fuzzy 分差 + 2 字中文的 fuzzy 单字匹配极宽」。证据 `.superpowers/sdd/b49/mcp-after-fenxi.log` / `mcp-after2-{fenxi-l500,fenxi-l1000,guanjianci-fenxi}.log` | **召回层已闭合（含真机）**（批次 50，handoff §二十四）：修法 = **召回层独立子串候选池**（`query.rs` 新增 `SUBSTRING_CANDIDATE_LIMIT` / `substring_candidate_limit_for` / `build_substring_only_query` / `collect_substring_candidate_addresses` / `merge_candidate_pools`），本地与云盘两条路径各挂一条（云盘池带**当前勾选连接**过滤）⇒ 只靠子串正则命中的文档不再在**候选阶段**被 BM25 常量分挤掉；**未改索引 schema（23,334 条云盘索引不重建）、未改最终排序口径、未升版本号**。7 条新单测（`mod w13_substring_pool`）；`cargo test --lib` **1709 passed / 0 failed / 3 ignored**（基线 1702 + 7）、`npm run check` EXIT=0（334 文件 / 3597 用例）。**2026-11-25 装机新包后真机 A/B**（应用自带只读 MCP `search_files`，`result_limit=200`，装置 `.superpowers/sdd/b50/probe.ps1`（Tag = before / after））：`技巧` 151/阿里云 1、`收集` 15/2、`心得` 21/1、`关键词分析` 7/1、`Midjourney` 39/2 —— **与批次 49 逐字一致 ⇒ 无回归**。`分析` 仍 200/阿里云 0 ⇒ 残留已坐实为**同分排序 / 结果窗口**（不是召回问题），见 **W-14**。应用日志实证该查询**云盘子串候选 307 条全部入池**（无截断） |
| **W-14** | **同分命中按文档序排 ⇒ 后写入的云盘条目在高频词下永远落在结果窗口外**（排序 / 容量层） | 批次 50 真机 A/B（handoff §二十四·5）：`分析` 在本地 988,637 条 + 云盘 23,334 条索引上共命中 5,207 条（本地 5,200 + 云盘 307）；**云盘靶子已完整进入候选池**（应用日志：命中 307 条 / 入池 307 条，无截断），得分 **0.92**（`scoring.rs` token 分支 `name_token.contains(query_token)` ⇒ 0.6 + 0.2 + 0.8 × 0.15），与本地几百条同名级条目**完全同分**；`merge_search_results`（`query.rs`）按 score 降序 + **稳定排序（本地先入队、同分本地在前）** ⇒ 同分时按文档序，云盘条目（远程索引里后写入、doc id 更大）永远排在同分本地条目之后 ⇒ 落在 `result_limit`（200，服务端上限 500）窗口之外。实测 `result_limit=500` 仍阿里云 0 命中、最低分仍 0.92 ⇒ 不是窗口太小，是同分带太长 | **未处理（等用户拍板排序规则）**：按台账 §5「多连接混合检索的排序策略 —— 暂不做（用户『暂时不管』）」本轮不擅自动全局排序口径。候选修法（都带代价）：① 同分时按**文件修改时间**新近优先（跨索引可比，但需确认索引是否存了 mtime，且会改变所有查询的同分顺序）；② 同分时按**来源配额**（本地 / 云盘各占一半，保证两边都有露出，但靶子仍在同分带尾部，不保证出现）；③ 维持现状（用更具体的词检索已稳定命中，如 `关键词分析` ⇒ 7 条含靶子）。⚠ 不要用「跨索引 doc id 新近」：两索引 doc id 各自独立，云盘 doc id 恒小于本地（23,334 ≪ 988,637）⇒ 等价于「云盘优先」，正是 §5 拍板暂不做的排序策略。**批次 51 可行性实证（2026-11-25，纯读码，未改代码、未动排序口径）**：**① 的技术前提成立** —— `modified_time` 是 `FAST` + `STORED` 字段（`global_search/index.rs:53`，本地与云盘共用同一 schema 构造），生产写入点写的都是**真实值**：本地 `scan.rs:266` 取 `metadata_modified_time_unix_ms` 并在 `:277` 落盘，云盘 `remote_index.rs:332` 写 `entry.modified_time`；全仓 `modified_time => 0u64` 的写入点（`index.rs:552/614/667`、`scan.rs:1563`、`query.rs:1372/1466/1606/1620/2081`、`remote_index.rs:837`）**逐处核对均在 `#[cfg(test)]` 夹具内** ⇒ 真机数据不缺 mtime，**且无需重建那 23,334 条云盘索引**。命中对象也已带该值：`materialize_candidates` 统一填 `GlobalSearchResultEntry.modified_time`（`query.rs:870-887`、`types.rs:81`），本地（`query.rs:490`）与云盘（`query.rs:627`）**走同一个函数** ⇒ 跨索引同分可比。**tie-break 落点共 3 处**：`merge_search_results` 的 `sort_by`（`query.rs:724`，随后 `:730` 才 `truncate(limit)`）与 `finalize_results` 的两处 `par_sort_by`（`query.rs:919`、`:929`，`:925` 按盘符组截断）；云盘侧在合并**之前不截断**（`search_remote_index` 直接返回全部物化结果，`query.rs:629`）⇒ **单改 `:724` 就能决定云盘条目能否进窗口**。两条必须先讲清的代价：**(a) ① 与 `query.rs:697-698` 成文的产品取向「本地结果优先」直接冲突**（同分改按 mtime ⇒ 更新的云盘文件会插到同分本地条目之前）；**(b) 任何同分规则都不能保证靶子出现** —— 对 `分析` 这类词，本地 5,200 条与云盘 307 条**完全同分**（`scoring.rs` token 分支恒 0.92，不含名称长度与命中位置信息），mtime 只能保证「同分带里最新的那些出现」，配额只能保证「云盘在同分带有露出」。故增补**组合规则候选 ④**：**同分带内先按来源保底配额（保住本地优先）、组内再按 mtime 新近** —— 兼顾露出确定性与「本地优先」取向，代价仍是改变所有查询的同分顺序。靶子与本地同分条目的 mtime 分布**尚未实测**，选 ①/④ 时先测一次真机 A/B 即可判 **批次 52 实现并真机 A/B 坐实（候选② + 组内新近）**：`query.rs` 两处改动 —— **(A) 合并层保底配额** `merge_search_results` 改为携带来源标记的 `Vec<(entry, from_cloud)>`（元组带来源，不嗅探 `sfm://remote/` 前缀，那是 `location.rs:74` 的实现细节），新增 `reserve_cloud_seats_in_cut_band`：**只在「正好卡在窗口边界的那个分数」的同分带内**重排，把该带尾部的本地席位让给带外的云盘条目，目标云盘占窗口一半；四条硬约束 = 不碰更高分条目 / 不丢数据（只重排、总长不变）/ 带内只剩 1 席时不给云盘（保住 `query.rs` 成文的「本地结果优先」）/ 云盘没货不硬凑。**(B) 云盘池内新近优先** `sort_remote_hits_by_recency`（`search_remote_index` 尾部）：云盘池原顺序 = 主池 BM25 序 + 子串池文档序 = 索引写入序，不重排则配额席位会被**最早写入**的连接吃光 ⇒ 改为「分数降序 + 同分 `modified_time` 新近」，**仅限同一索引内部**，不引入跨来源新近。TDD：新模块 `mod w14_cloud_band_quota` 6 条测试先 RED（`2 passed / 4 failed`）再 GREEN（`6 passed / 0 failed`）；全量门禁 `cargo test --lib` **1715 passed / 0 failed / 3 ignored** + `npm run check` **334 文件 / 3597 用例 / EXIT=0**（与基线 1709+6 逐字吻合）。**真机 A/B（打包装机，`result_limit=200`）**：`分析` 窗口内云盘命中 **1 条 → 101 条**（批次 50 包 vs 本批包，同一台机同一份索引，remote-index mtime 不变 `11-25 09:10:51`），其余五例（`心得` 21 / `关键词分析` 7 / `Midjourney` 39 / `技巧` 151 / `收集` 15）逐字不变、靶子命中不回归 ⇒ **W-14 的排序缺陷本身已闭合**。**⚠ 一条需订正的旧指认**：本批实测 `result_limit=500` 下 `分析` 的云盘命中 251 条**全部来自连接 `0f0696ba`，阿里云盘 `646dacdb` 为 0 条** ⇒ 批次 50「307 条云盘候选含靶子、同分被切」的说法对**靶子**不成立；「搜『分析』看不到靶子」的剩余根因在**云盘索引对该词的召回粒度**（文件名里是「关键词分析」，被当作一个 token ⇒ `分析` 命不中），属 W-13 召回层的延伸，不是配额层。**未验证项**：靶子所在连接的 mtime 是否够新（因该词 0 命中，真机无法验证「配额按新近取到靶子」）；跨连接平衡仍按 §5「暂不做」。本批未升版本号（仍 3.22.0）、未动 CHANGELOG、未重建索引、未推送。 **批次 53 订正（2026-11-25，判别实验推翻本行前提）**：本行「靶子在云盘索引侧对该词**根本没被召回**」的指认**不成立** —— 实测靶子 `Midjourney 使用心得+关键词分析.mp4` **就在 `分析` 的云盘候选池内**（`score=0.920000016689301`、conn `646dacdb…`、`mtime=1680686620768` = 2023-04-05 ⇒ **不够新**，这恰好回答并否证了本行「靶子 mtime 是否够新」这一未验证项）。真根因 = 批次 52 的 `sort_remote_hits_by_recency` 把「同分按 mtime 新近」用在**跨连接**的整条云盘池上，导致 0.92 整个同分带的云盘席位被**单个连接 `0f0696ba…` 整带吃光**（实测 200 窗 `0f0696ba:101` / 500 窗 `0f0696ba:251`，靶子所属连接 **0 席入窗**）⇒ 属**公平层缺陷，另立 W-15**，批次 53 已闭合（见 §7 批次 53 证据块 + handoff §二十六）。至于「云盘召回粒度」：跨 **token 边界**的子串确实召不回（子串池探针 `使用心得+关键` → **0 条**，去掉 `+` 仍 0 条；而 `键词分析` → 7 含靶、`词分析` → 41 含靶），这是**独立的缺陷 A**，修它需给 `name` 加子串/ngram 子字段并**重建 23,334 条云盘索引**，用户已明确排到 W-15 之后，本批不动分词/索引。 |
| **W-15** | **同分带内的云盘保底席位被单个连接整带吃光 ⇒ 后接入连接的条目永远进不了结果窗口**（排序 / 公平层） | 批次 52 §二十五·4 列为「未验证项 2（多连接席位平衡）」；批次 53 判别实验坐实为缺陷：`分析` 的 0.92 同分带里，云盘 101 席**全部**属 `0f0696ba…`、阿里云盘 `646dacdb…` **0 席**；`result_limit=500` 亦然（云盘 251 席 **251/251** 属同一连接）。靶子 `mtime=1680686620768`（2023-04-05）比该连接条目更旧 ⇒ 被 mtime 降序整带压到窗口之外 ⇒「搜『分析』看不到靶子」 | **已闭合（含真机）**（批次 53，handoff §二十六）：修法 = **同分带内按 `connection_id` 公平轮转分配席位**。`query.rs` **+357 / −12**（仅此一文件）：`sort_remote_hits_by_recency` 改名并重写为 `order_remote_hits`（`:648`，调用点 `:633`）+ `rotate_band_by_connection`（`:681`）+ `cloud_connection_id`（`:730`，复用 `crate::remote::parse_remote_path` 取连接身份 ⇒ **否决**给 `GlobalSearchResultEntry` 加 `connection_id` 字段，不动 8 处构造点与序列化契约/前端 TS 类型）。机制：先「score 降序 + mtime 降序」铺平，再切出同分带，带内**每轮每个连接出一条**；组序按该组最新 mtime 降序、并列按连接 id 升序，`total_cmp` 保证确定性；解析不出连接的条目落 `unknown` 带尾；**带内只有一组（`groups.len()<2`）原样返回 ⇒ 与批次 52 逐条一致、无回归**。边界全部沿用批次 52：只在同分带内重排 / 只重排不删数据 / 不碰更高分 / 不引入跨索引 doc id 新近 / 不引入跨来源 mtime 比较。8 条新单测（`mod w15_connection_fairness` 7 条 + w14 内索引级 1 条；夹具**必须用同名文件**才构成真同分带 —— 不同名云盘命中的分数根本不等，实测 0.96875 vs 0.92）。RED（临时把 `:672` 改为 `ordered.extend(band);` 关掉轮转）`18 passed / 3 failed` ⇒ 恢复后 GREEN `cargo test --lib` **1723 passed / 0 failed / 3 ignored**（基线 1715 + 8）、`npm run check` **334 文件 / 3597 用例 / EXIT=0**。真机（同机同索引，云盘索引 mtime 仍 `2026-11-25 09:10:51`）：`分析` 靶子 **窗口外不可见 → 第 101 条**、阿里云盘席位 **0 → 33**、`CLOUD` 总数 101 与 `COUNT` 全例**逐字不变**（批次 52「云盘占半窗」不回退）、`心得` 下靶子 20 → **15**；`技巧`/`收集`/`关键词分析`/`Midjourney` 四例逐字一致。⚠ **界面级（非 MCP）复验仍未做**，且界面单次展示条数上限未查明 ⇒ 靶子在第 101 位是否等于「用户真的看到」尚未坐实。未升版本号（仍 3.22.0）、未动 CHANGELOG、未重建索引、未推送 |
| **W-16** | 界面默认 50 条下靶子是否可见（批次 53 遗留疑问）—— 批次 54 实测已澄清：**可见** | 批次 53 收尾时我据「limit=200 响应里前 50 窗口只有 1 条云盘」推断默认配置下靶子不可见，**该推断错误**：`result_limit` 不只是截断长度，它同时是合并层云盘席位配额 `reserve_cloud_seats_in_cut_band` 的入参（`score_cut` 取 `merged[limit-1]`、`desired = limit/2 - clouds_in_window`、`band_start` 随候选预算变化），不同 limit 得到不同的同分带构成 ⇒ 大 limit 响应的前缀 ≠ 小 limit 的结果。批次 54 fresh 装机实测（`.superpowers/sdd/b54/probe-limit50.ps1`，靶子 `Midjourney 使用心得+关键词分析.mp4`，搜「分析」）位次：limit=10→第 10、30→第 17、49→第 27、**50→第 27（界面默认，可见）**、100→第 52、200→第 102；limit=50 时窗口构成 24 本地 + 26 云盘（`0f0696ba`:14 / `646dacdb`:12），配额正常给云盘留席位 ⇒ 批次 53 在界面默认设置下**已让靶子可见**，本行原登记的「不可见」不成立 | 批次 54 原计划的「同分带每连接席位上限」经实测**无收益**（`.superpowers/sdd/b54/band-fenxi.md`：靶子已是 `646dacdb` 组内第 1 条 / 带内第 2 条云盘，压制其他连接溢出最多让它前移 1 位），不作主线。残留两问待用户界面复检后再定方向：① 用户当时是否用「精确」模式（缺陷 A：跨 token 边界子串召不回，是唯一还能解释「看不到」的根因候选）② 第 27 位需滚动才见到，若要更靠前属产品调优而非缺陷，需用户拍板 | **批次 55 闭合残留问 ①**（2026-11-25 深夜）：装机实测 `user-data/user-settings.json`（**扁平点号键**结构）`globalSearch.exactMatch=**false**` ⇒ 用户当时用的**不是**精确模式 ⇒ **缺陷 A 不是「看不到」的根因**（缺陷 A 仍单列在 W-14 尾部，修它要给 `name` 加子串/ngram 子字段并**重建 23,334 条云盘索引**，用户已排到 B 之后）。同文件实测 `globalSearch.resultLimit` **键不存在** ⇒ 取默认 **50**（`src/constants/index.ts:22` `DEFAULT_RESULT_LIMIT=50`），与批次 54 六档实测的「界面默认即 limit=50」互相印证 ⇒ §二十六·7「默认 50 条下界面看不到靶子」**已被批次 54 推翻**，本行同步订正为「**默认 50 条下靶子在第 27 位、可见**」。同批实测旁证：`typoTolerance=false`、`includeFiles/includeDirectories=true`、`scanDepth=9`、`parallelScan=true`、`selectedDriveRoots=["E:/","F:/"]`。残留问 ② 属**产品口味**（要不要更靠前）而非缺陷 ⇒ 不在本批范围。**W-16 判闭合**（批次 55）
| **W-17** | 命令面板多语言文案残留 | 批次 54 只修了 en/ch 两处：占位符去掉「或搜索」承诺、把 ch.json 的 toggleCommandPalette 补成中文。其余 15 个语言包占位符仍承诺可搜索、9 个语言包该 tooltip 仍是英文（de/es/fa/fr/it/pt/sl/tr/vi）。scripts/sync-i18n.js 只同步 key 不翻译值，互不波及。外语译文由 Agent 代笔的污染风险大于收益。详见 handoff §二十六·9。 | 待用户授权代笔外语再修，或由社区翻译跟进；不阻塞本批收口。**批次 54 收口已完成**（handoff §二十六·10）：en/ch 新文案已随 b54 包装机，沙箱界面复验 **8 PASS / 0 FAIL**（占位符「输入命令...」命中、旧「输入命令或搜索」0 命中、面板仍列 20 条命令）；en 侧由 `dist/` 构建产物扫描佐证（新文案命中 2 文件 / 旧文案 0 命中）。15 个外语包仍未处理。 |
| **W-18** | **云盘目录作用域检索（`directory` 指向 `sfm://remote/...`）只走本地 `WalkDir` ⇒ 静默返回 0 条**（检索作用域 / 通道缺失） | 批次 53 暴露（装机版 MCP 传 `"directory":"sfm://remote/<conn>"` → `TOTAL=0`）。**批次 55 根因坐实**：`mcp/tools.rs` 带 `directory` 时走 `search_directory_recursive_blocking`（本地 `WalkDir`），而 `query.rs` 对 `sfm://remote/<conn>/...` 执行 `PathBuf::is_dir()` ⇒ 假 ⇒ 静默 `Ok(Vec::new())` ⇒ 调用方（MCP / Tauri 命令 / 前端）把「**通道不支持**」误读成「目录里没有匹配文件」。`src-tauri/src/remote/**` **无任何远程递归检索通道** | **低成本修复已完成并真机验证**（批次 55）：`search_directory_recursive_blocking` 入口加守卫 ⇒ 云盘路径**明确报错**（含路径 + 替代建议），不再静默 0；4 条新单测（`mod w18_remote_scope_guard`）；**真机**（b55 包装机，隔离桌面沙箱起应用，走应用自带只读 MCP）CASE A/B = 云盘目录与连接根**都**返回明确错误，错误原文「云盘目录暂不支持递归检索（sfm://remote/0f0696ba…）：该通道只遍历本地磁盘。云盘文件请省略目录参数走全局索引检索，或用 list_directory 逐层列举。」；CASE C 无目录参数仍 200 条 / 云盘 101 条（**无回归**）。**真正的「云盘目录递归检索」新功能待用户拍板**（架构/配额取舍，按 §0.2 不擅自实现）：① 走**云盘索引**（快、零 API 配额，但只覆盖已建索引的连接，百度封顶 20,000 条 ⇒ 可能「搜到一半」）② 走**网盘实时遍历**（完整，但每次按键烧 API 配额）。**前端未透传**：`use-file-browser-recursive-search.ts:132-140` 的 `catch` 仍**静默降级为「只搜当前层」**（`recursiveSearchUnavailable` 未向上透传；加提示需动 15 个语言包 ⇒ 与 W-17 同风险面）。详见 handoff §二十七 |
| **W-19** | **界面级像素 / 键盘复验通道在本项目不可用**（CDP 9333 无监听） | 批次 55 实测：Tauri 显式 `AdditionalBrowserArguments` **覆盖** `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`（WebView2 规则：应用显式参数优先）⇒ 用环境变量打开的 `--remote-debugging-port=9333` 在 Tauri 应用里**不生效**（应用进程起了、WebView2 子进程起了、9333 无监听；`tauri.conf.json` 现无 `additionalBrowserArgs` 配置）。改 `tauri.conf.json` 注入需**两次重建**（验收包 + 回滚干净包，约 15 分钟）且产物带调试端口 | **已绕开**：本批改用 **vitest DOM 级回归测试**（`global-search-result-list-remote-entry.test.ts`，4 条）锁住「云盘条目真的渲染进全局搜索界面」——可重复、可提交、永久留证，优于一次性截图。**像素级 / 键盘路径本批如实标注为未验**（沙箱内无键盘 ⇒ 键盘路径不可假称已验，AGENTS.md §2.8）。真机取证改走**应用自带只读 MCP**（`127.0.0.1:55100`，隔离桌面沙箱起应用也照样可达）。详见 handoff §二十七 |

---

## 5. 已拍板不做（含理由，防重开）

| 项 | 拍板 | 理由 |
|---|---|---|
| OneDrive 个人模式 | 不做 | 批次 41 用户决定「仅企业版不支持」。`remote_client.rs:375-389` **本就只发 `scope: 'https://graph.microsoft.com/User.Read offline_access'`，未请求 Files.ReadWrite** → 该项已被代码现状满足，无需改动 |
| 多连接混合检索的排序策略 | **部分解除**（批次 53 只解除「多连接席位平衡」这一条，其余仍暂不做） | 用户「暂时不管」。现状：全局检索结果**未按 `connection_id` 分组、无跨连接去重**，排序按各连接内部顺序合并。要改需先定产品规则（按相关度？按连接？按路径？），无规则不动 **批次 50 增补（2026-11-25）**：W-14 收尾时实测到该策略的具体表现 ——「同分条目按文档序排 ⇒ 后写入的云盘条目永远排在同分本地条目之后」，`分析` 这类高频词下云盘靶子落在 200 / 500 条结果窗口外（已坐实，见 §4 末行 W-14）。**按本行原口径继续「暂不做」**，本轮不擅自动；W-14 已拍板候选②（同分带保底配额 + 云盘池内新近）并在批次 52 实现、真机 A/B 坐实（见 §4 W-14 行与 §7 批次 52 证据块）；**批次 53 就地解除本行的「多连接席位平衡」口径**——同分带内改为按 `connection_id` 轮转席位（见 §4 W-15 行与 §7 批次 53 证据块；真机分布从 `0f0696ba:68 / 646dacdb:0` 变为 `68 / 33`，靶子回到结果内）。本行其余口径（本地/云盘之间的席位与配比、更高分带是否被挤动）仍按「暂不做」。 |
| `search_files.rs` 死代码清理 | 延后 | 用户「可以暂时不处理」。当前仍有测试引用，删除需连带改测试，属独立清理批次 |
| 扫描期间冷启首查返回空结果 | 延后 | 用户「可以暂时不处理」（批次 41 实测坐实：扫描未完成时冷启首查返回空，再查一次正常） |

---

## 6. 下批次（批次 47+）建议顺序

> **批次 47 结算：W-9 / W-10 闭合** —— 原第 2 项（W-9 后端重建无互斥锁）⇒ **闭合**：加一把**跨连接共用**的
> `tokio::sync::Mutex`，guard 在任何文件操作之前取得，并发时**排队**而非报错（handoff §二十一·1）；
> 原第 3 项（W-10 上限警告刷屏）⇒ **闭合**：逐目录 WARN 改为计数 + **每个连接一条**汇总（handoff §二十一·2）。
> 新增 3 条单测；`cargo test --lib` **1696 passed / 0 failed / 3 ignored**（基线 1693 + 3）、`npm run check` EXIT=0。
> **代价**：本轮**未做任何界面验收** —— 00:13 起本机锁屏（`LogonUI` pid 31928 / `FOREGROUND=0` / 主窗最小化到托盘），
> 且阿里云盘 token 已于约 00:56 过期 ⇒ 原第 1 项**整项退回**，仍是下一批次第一件事。
> **到此 §6 里「纯代码、不依赖用户」的项已全部清空。**

1. **B-3 / B-5 / B-6 的界面级验收**（**唯一剩下的代码侧验收项**，必须用户在场并解锁）
   —— **批次 48 进展（2026-11-25 09:00–09:11）**：
   ① **已闭合**：界面勾选 `includeInGlobalSearch` + 点保存（同一次 `saveConnection` 落盘，只触发一次 {百度, 阿里云} 重建）
   ⇒ 09:00:32 `云盘索引重建开始：2 个连接` ⇒ 09:10:51 commit（`opstamp=23335` / 23334 条 = 百度 20000 + 阿里云 3334，commit 295ms，总 619s）
   ⇒ MCP `search_files` 实测阿里云条目**可检索**（`Midjourney`→39 条含 2 条阿里云、`AI`→260 条含 1 条）。
   ② **B-3 已闭合**：检测（批次 46 编辑弹窗 UIA 取到 `会话已过期，需要重新登录`）+ 重登（本轮 `编辑` → `打开登录窗口` 完成）+ 恢复（重登并保存后重建成功、检索可用）。
   ③ B-5 待用户一次真实下载触发杀毒超时（**纯用户侧**）④ B-6 待三家各走一轮界面级验收。
   > ✅ **W-12 已闭合（含真机）**（批次 49，§4 末行 / handoff §二十三）：查询侧正则子串，索引无需重建。
   > **2026-11-25 10:55 已把批次 49 新包静默装到 `D:\Program Files\Alpha File Manager`**（源 `release\local-builds\Alpha File Manager_3.22.0_x64-setup.exe`，46,148,338 B / SHA256 `26051490…BCD6`；顶层已发布副本 `4AB5032B…D470` 原封未动）。
   > 真机 A/B：`技巧` / `收集` / `心得` 三个 2 字词对阿里云 **0 → 1 / 2 / 1 命中**，ASCII 对照无回归。
   > ⚠️ 唯一残留是 **W-13**（`分析` 这类高频 2 字词候选池打满 500 被挤出，排序层）。再遇到「搜不到云盘中文文件」，先确认装的是批次 49 的包，再判授权 / 索引。
   > 别再走「改 JSON 后重启」那条路：初始读盘不触发 watcher，改了等于白改。
   > **批次 50（2026-11-25 11:49–11:51 装机）**：W-13 的**召回层**已闭合（独立子串候选池）—— 新包 `release\local-builds\Alpha File Manager_3.22.0_x64-setup.exe`（46,156,750 B / SHA256 `2366E3BA…D011`）静默装到 `D:\Program Files\Alpha File Manager`，顶层已发布副本 `4AB5032B…D470` 原封未动；真机 A/B 5 个查询与批次 49 逐字一致（无回归）。⚠ **可见性残留单列为 W-14**（同分排序 / 结果容量口径，等用户拍板排序规则；按 §5「排序策略暂不做」本轮未动）。
2. **W-4**（360 误报）随新包装机复验（用户侧，Agent 判不了）。
3. B-2 等用户再次遇到阿里云盘 400；期间不动代码。
4. **待用户处置（仓库外系统状态）**：本机 `rustup` 的 `stable` 已损坏 → 是否允许跑 `rustup` 修复 / 回滚？
   不修则后续 `cargo test --lib` 与 `tauri:build` 都只能借 1.90.0 工具链。
5. **待用户拍板后才动**：版本号（3.23.0？）与 CHANGELOG；云盘索引配额最终口径（§2 前两行）。

> **批次 55 结算（2026-11-25 18:05）**：本批**不依赖用户**的三件事做完 ——
> ① **W-16 闭合**（装机实测 `globalSearch.exactMatch=false` ⇒ 缺陷 A 不是「看不到」的根因；
> `globalSearch.resultLimit` 键不存在 ⇒ 取默认 **50**，与批次 54 六档实测互相印证）；
> ② **§二十六·6 云盘目录作用域检索**根因坐实 + **低成本修复**（Rust 侧「静默 0 条」→ **明确错误**，4 条新单测）+ **真机复验**（b55 包装机，MCP 四例全过），**真功能待用户拍板**（W-18）；
> ③ **「用户真的看到云盘文件」证据链的最后一环**用 **vitest DOM 级回归测试**闭合（放弃 CDP 路线，见 W-19）。
> 门禁（盘上回读，不凭摘要抄）：`cargo test --lib` **1727 passed / 0 failed / 3 ignored**（基线 1723 + 4）；
> `npm run check` **335 文件 / 3601 用例 / EXIT=0**（基线 334 / 3597；+1 文件 +4 用例 = 本批新测试文件）。
> 顺带修掉一个**既有随机红**：`dir_reader::shell_namespace::tests::propsys_resolved_system_size_key_reads_real_file_size`
> （测试直调 `resolve_shell_item` 时没建 COM 公寓，成不成都看测试线程有没有被别的测试初始化过 COM ⇒ 批次 53 全绿是运气）
> → 给测试自建 `ComGuard`（**不动产品代码**）。
> 打包与装机：`tauri:build` **BUILD_EXIT=0**，产物 46,114,293 B / SHA256 `6F4ABEA1…9635B5B`（与 b54 的 `E4731E91…` 不同）
> → 留存 `release/local-builds/Alpha File Manager_3.22.0_x64-setup.build-20261125-b55.exe`
> → 装机后**包内 payload sha256 = 已装 exe sha256 = `E744CF66…41D562D`**（§2.12 判据，PAYLOAD_MATCH=YES）。
> **仍未做**：界面级像素 / 键盘复验（通道受阻，W-19）、版本号 / CHANGELOG（§2 第 3 行）、
> 15 个外语包（W-17）、索引配额口径、云盘递归检索口径（W-18）、`rustup` 修复授权。

---

## 7. 证据路径

> AGENTS.md 的 `§0.3 验证装置地图` 已补上这三类装置的正确用法与「`tauri dev` 不能用于界面验收」警示；本节只留本批次的具体取证指针。

- **批次 50（W-13 召回层：CJK 子串命中在候选阶段被挤掉）**：
  - `.superpowers/sdd/b50/cargo-test.cmd` / `cargo-test-lib.log` —— `cargo test --lib`：**1709 passed / 0 failed / 3 ignored**（借 1.90.0 工具链；7 条新单测在 `mod w13_substring_pool`）
  - `.superpowers/sdd/b50/npm-check.cmd` / `npm-check.log` —— `npm run check` EXIT=0（334 文件 / 3597 用例）
  - `.superpowers/sdd/b50/build.cmd` / `tauri-build.log` —— `npm run tauri:build`；判 `Finished release profile` + `Running makensis` + 任务终态 succeeded
  - `.superpowers/sdd/b50/probe.ps1` —— **真机 A/B 探针**（应用自带只读 MCP `search_files`，`result_limit=200`；CJK 查询用码点拼，脚本纯 ASCII；`-Tag before` / `-Tag after`）
  - `.superpowers/sdd/b50/probe-before.txt` / `probe-after.txt` —— 两轮探针**逐行对照**（before = 批次 49 包，after = 批次 50 新包）
  - `.superpowers/sdd/b50/mcp-before-*.log` / `mcp-after-*.log` —— 6 个查询的 MCP 原始回包（after 另有 `mcp-after-fenxi-l500.log` = `result_limit=500` 对照）
  - `.superpowers/sdd/b50/install.ps1` / `run-app.ps1` —— 静默装机 + 起应用（⚠ `run-app.ps1` 的就绪判据在本机不成立，**直接探 MCP 端口返 401 即在线**）
  - `.superpowers/sdd/b50/patch-docs.cjs` + `ledger-patch.md` + `s24-section.md` —— 本轮台账改写与 handoff §二十四 追加脚本（**幂等**）
  - 新包 `release\local-builds\Alpha File Manager_3.22.0_x64-setup.exe`（**46,156,750 B / SHA256 `2366E3BA…D011`**）；顶层已发布副本 `release\Alpha File Manager_3.22.0_x64-setup.exe`（**46,151,697 B / SHA256 `4AB5032B…D470`**）**原封未动**
  - 应用日志 `%LOCALAPPDATA%\com.sigma-file-manager.app\logs\Alpha File Manager.log` 03:51:43–45 行：`云盘子串候选：命中 307 条，入池 307 条` / `子串候选命中 5200 条，超过候选上限 4000` / `substring_candidates=4000 substring_matched=5200`（⚠ 运行期日志被独占 ⇒ 必须用**共享读**方式打开）

- **批次 47（W-9 重建互斥锁 / W-10 上限警告汇总）**：
  - `.superpowers/sdd/b47/state.ps1` —— **开工先跑**：一行输出 `NOW / LOGONUI / FOREGROUND / APP / CRED / INDEX`（判「能不能做界面」）
  - `.superpowers/sdd/b47/dump-tree.ps1` —— 主窗 UIA 转储（按进程过滤 + 面积降序取最大窗）
  - `.superpowers/sdd/b47/tree-now.txt` —— 锁屏态产物：**0 行**（不是脚本坏了，是锁屏）
  - `.superpowers/sdd/b47/cargo-test-lib.log` —— `1696 passed / 0 failed / 3 ignored`（借 1.90.0 工具链；三条新测试在 L638 / L681 / L682）
  - `.superpowers/sdd/b47/npm-check.log` —— `npm run check` EXIT=0（334 文件 / 3597 用例）
  - `.superpowers/sdd/b47/mcp-list-aliyun-root.txt` —— `sfm://` 虚拟路径不被 MCP 接受（`-32603 os error 123`）
  - `.superpowers/sdd/b47/handoff-s21.md` + `append-s21.cjs` —— handoff §二十一 片段与**幂等**追加脚本（守卫串 `二十一、批次 47`）
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
  - `.superpowers/sdd/b46/aliyun-save.ps1` + `aliyun-save-labels.json` —— **§6-3 闭合那一步**：按「取消」同行 Y 锚定、优先 `--default` 类名，`InvokePattern` 点底部「保存」
  - `.superpowers/sdd/b46/aliyun-testconn.ps1` + `aliyun-testconn-labels.json` —— 点弹窗自带「测试连接」并比对 Names 前后差集（`-CloseDialog` 改为点「取消」收尾）
  - `.superpowers/sdd/b46/accept-aliyun/S63-CLOSEOUT.txt` —— §6-3 收口（2,791 B / 7 节：动作序列 / 落盘判据 / 会话有效性 / 端到端验证 / 正交遗留 / 复现配方 / 关键教训）
  - `.superpowers/sdd/b46/accept-aliyun/save-2306-before.txt` / `save-2306-after.txt` —— 保存前后 UIA 转储（后者 129 行）
  - `.superpowers/sdd/b46/handoff-s209.md` + `append-s209.cjs` —— handoff §二十·9 片段与幂等追加脚本（守卫串 `S63-CLOSEOUT`）
  - ⚠️ **废弃脚本，勿复用**：`aliyun-login-go.ps1`（v1/v2）、`aliyun-invoke-login.ps1`、`aliyun-login-drive.ps1 -ClickLabel`
    —— 四条都是在「远程页上有登录按钮」这个**错误模型**上写的，失败原因不是脚本 bug 而是模型错；
    另加 **`aliyun-qr-start.ps1`** —— 其目标按钮（「扫码登录」）走 Rust 自造 `generate.do/query.do`，实测**卡 `NEW` 永不推进**，属废路线
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

- **批次 52（W-14 同分带保底配额：实现 + 门禁 + 真机 A/B）**：
  - `.superpowers/sdd/b52/cargo-test-w14.log` —— RED `2 passed / 4 failed`（`CARGO_TEST_W14_EXIT=101`）→ 实现后 GREEN `6 passed; 0 failed; 1712 filtered out`（`EXIT=0`）
  - `.superpowers/sdd/b52/cargo-test-lib.log` —— **1715 passed / 0 failed / 3 ignored**（借 1.90.0 工具链；基线 1709 + 本批 6）
  - `.superpowers/sdd/b52/npm-check.log` —— **334 files / 3597 tests passed，`NPM_CHECK_EXIT=0`**
  - `.superpowers/sdd/b52/build.cmd` / `tauri-build.log` —— `npm run tauri:build` **`EXITCODE=0`**（13:52→13:58，`Running makensis` 出现，无 `error[E`）
  - `.superpowers/sdd/b52/install.ps1` —— 装 bundle `46,164,367 B` / sha256 `F36E9827…28EBF`，留存 `release/local-builds/Alpha File Manager_3.22.0_x64-setup.build-20261125-b52.exe`；已装 exe mtime `13:57:30` / `FF671FEE…`；**顶层已发布副本未被触碰**（sha `4AB5032B…` 复核一致）
  - `.superpowers/sdd/b52/probe.ps1`（`-Tag before` 与 `-Tag after` 各一遍）+ `mcp-<tag>-<case>.log`（MCP `search_files` 原始回包，6 例 × 两遍）—— **判据行**：`TAG=before CASE=fenxi COUNT=200 ALIYUN_HITS=0` / `TAG=after CASE=fenxi COUNT=200`，但**窗口内云盘总条数 1 → 101**（`BYCONN=0f0696ba:101`）
  - `.superpowers/sdd/b52/mcp-fenxi500.log` —— `result_limit=500` 判别实验：云盘 251 条、阿里云盘 **0** 条、靶子 **NO** ⇒ 靶子对「分析」在云盘索引侧根本未被召回
  - 就绪判据：直探 `http://127.0.0.1:55100/mcp` 返 **401**（比 `mcp-access.json` mtime 可靠）

- **批次 53（W-15，2026-11-25）云盘同分带按连接轮转席位 —— 真机复验**：
  - 改动文件：`src-tauri/src/global_search/query.rs`（**唯一改动文件**，+357/-12，含 8 条用例）；未提交。
  - 判别实验（改前取证，`分析` = `fenxi` 走 MCP `search_files`）：
    - 子串池探针 `键词分析` → **7 条含靶**、`词分析` → **41 条含靶** ⇒ **靶子其实被云盘子串通道召回**（批次 52「根本没召回」的说法被推翻）；
      `使用心得+关键` → **0 条**，去掉 `+` 仍 **0 条** ⇒ 缺陷 A 为真，单独登记在 W-14「批次 53 订正」。
    - 改前分布：`分析`@limit 200 云盘 101 条，其中 `0f0696ba...` 独占 **68 席**、靶子所属 `646dacdb...` **0 席**；
      扩容 @500 云盘 251 条，靶子连接仍只有 33 条、靶子仍未入窗（`TARGET=NO`）。
    - 靶子实测：`conn=646dacdb6ff34af4b26f88dd802baf02`、`score=0.920000016689301`、`mtime=1680686620768`（2023-04-05）。
  - **RED 证据**：临时把同分带合并行改回 `ordered.extend(band)`（`.superpowers/sdd/b53/query.rs.RED-backup` 是轮转版备份，
    跑完已恢复，`cargo test w1` 复跑 18 passed / 0 failed），`cargo test w1` → **18 passed / 3 failed**，
    三条失败**全是本批判判性用例** —— `w15_target_is_promoted...`（靶子下标 20、带尾成块）、
    `w15_hits_from_a_connection_without...`（该连接命中聚成 3 块）、`w14_a_stale_connection_keeping_only_the_target...`（靶子下标 8）。
    证据 `.superpowers/sdd/b53/cargo-test-RED.log` / `-RED.err.log`。
  - **GREEN / 门禁**：`cargo test --lib`（rustup **1.90.0**）= **1723 passed / 0 failed / 3 ignored**（基线 1715 + 8）；
    `npm run check` = **334 文件 / 3597 用例 / EXIT=0**。日志 `cargo-test-b53.log`、`npm-check-b53.log`。
  - **打包与装机**：`tauri-build.cmd` → `TAURI_BUILD_EXIT=0`（bundle 43.98 MiB，stamp 为
    `release/installed-build-Alpha File Manager_3.22.0_x64-setup.build-20261125-b53.exe`，**不覆盖批次 52 留存副本**）；
    `install.ps1` 静默升级后 `APP_VERSION=3.22.0`、`RUNNING=pid=26940`。
  - **改后真机分布（同探针、同机、同索引）**：
    - `分析`@200：云盘 101 条 = `0f0696ba 68 / 646dacdb 33`，**`TARGET_RANK=101`**（批次 52 包为未入窗）。
    - `分析`@500：云盘 251 条 = `0f0696ba 218 / 646dacdb 33`、**`TARGET_RANK=251`**（同分带只到 0.92，更高分带未被挤动，属正常）。
    - `心得`@200：靶子名次由 **20 → 15**；`金桥`/`手机`/`关键词分析`/`control` 四例与改前**逐字一致**（靶子本就在前 4 的快路径不受影响）。
    - 日志 `mcp-{b53,b53-500,jinqiao,shouji,guanjianzifenxi,control,xinde}.log`，判据汇总 `analyze-b53.txt` / `analyze-before.txt`。
  - **仍未验证**：① 界面（非 MCP）单次展示条数与靶子像素可见性；② 缺陷 A 的召回修复需重建 23,334 条云盘索引、用户未授权；
    ③ 装机版 MCP 未暴露 `remote_search` 与 `connection_id` 参数，按连接取数只能解析回包 path 第 4 段。
  - 就绪判据沿用批次 52：直探 `http://127.0.0.1:55100/mcp` 返 **401**；令牌读 `%APPDATA%\com.sigma-file-manager.app\mcp-access.json` 的 `.token`（勿回显）。
  - **界面条数上限（同轮 fresh 查到，构成本批验收口径的订正）**：`src/constants/index.ts:22-24` = `DEFAULT_RESULT_LIMIT: 50` / `MIN_RESULT_LIMIT: 10` / `MAX_RESULT_LIMIT: 500`；`src/stores/runtime/global-search.ts:388` 直接把它当请求 `limit`，全局搜索**无分页续拉**；可调入口钳制在 `[10,500]`（设置页 `src/modules/settings/ui/categories/search/global-search.vue:283-287`、搜索页 `src/modules/global-search/global-search-view.vue:76-79`）。⇒ 默认配置下界面只取 50 条，而靶子改后在第 **101** 位 ⇒ **本批判据只在 MCP 通道（`limit=200/500`）成立，用户界面上仍看不到靶子**（需把「结果条数」调到 ≥101 才可见）。装机实际 `resultLimit` 未取到（数据目录 `user-data` 为 junction，本轮按调用预算未继续深挖）= 未验。详见 handoff §二十六·7。→ 登记 **W-16**
  - **批次 54 候选（消除观感残留的两条路）**：① 同分带内再加「每连接席位上限」，把靶子所属连接的命中压进前 50；② 修缺陷 A（跨 token 边界子串召不回），让精确查询 `关键词分析` 直接命中，不依赖翻找 `分析` 的长列表

- **批次 55（W-16 闭合 + W-18 低成本修复 + 「云盘条目真的进界面」证据链最后一环）**：
  - `src-tauri/src/global_search/query.rs` —— 云盘目录作用域守卫（`search_directory_recursive_blocking` 入口，`crate::remote::is_remote_path`）+ `mod w18_remote_scope_guard` **4 条**单测。⚠️ **夹具必须用 `%USERPROFILE%\afm-rust-search-fixtures\`**，不能用 `std::env::temp_dir()` —— `builtin_ignored_paths()` 覆盖 `/AppData/Local/Temp`，会把夹具整棵剪掉（仓库已栽两次，`query.rs:2049-2052` 有注释）
  - `src-tauri/src/mcp/tools.rs` —— `search_files` 的 doc comment + `#[tool(description=…)]` 改为「`directory` 仅限本地目录；云盘路径 `sfm://remote/...` 会明确报错」
  - `src/modules/global-search/components/global-search-result-rows.ts` —— 新增**导出**纯函数 `filterGlobalSearchResultsByKind`（语义与原内联逻辑**逐字等价**，只为可测）
  - `src/modules/global-search/global-search-view.vue` —— `filteredResults` 改调该纯函数
  - `src/modules/global-search/components/__tests__/global-search-result-list-remote-entry.test.ts` —— **新增**（4 条 DOM 级断言：云盘文件/目录按 `includeFiles/includeDirectories` 保留或剔除、按根目录分组云盘条目仍在、列表把云盘文件名 `Midjourney 使用心得+关键词分析.mp4` **渲染进 DOM**、只挂云盘条目同样渲染）
  - `src-tauri/src/dir_reader/shell_namespace.rs` —— **既有随机红的修复**：测试自建 `super::platform::ComGuard`（**不动产品代码**）
  - 门禁日志：`.superpowers/sdd/b55/cargo-test-lib-b55-run4.log`（**1727 passed / 0 failed / 3 ignored**）、`.superpowers/sdd/b55/npm-check-b55.log`（**335 文件 / 3601 用例 / EXIT=0**）
  - 打包：`.superpowers/sdd/b55/tauri-build-b55.log`（**BUILD_EXIT=0**）
  - 装机：`.superpowers/sdd/b55/install-b55.ps1` —— **包内 payload sha256 与已装 exe 逐字节一致**（`E744CF66…41D562D`，PAYLOAD_MATCH=YES；AGENTS.md §2.12 判据）
  - 沙箱起应用（等 MCP 就绪，替代已判不可用的 CDP）：`.superpowers/sdd/b55/b55-launch-sandbox-mcp.ps1`
  - **真机探针**：`.superpowers/sdd/b55/probe-b55.ps1` + 原始回包 `mcp-A-remote-dir.log` / `mcp-B-remote-root.log` / `mcp-C-no-dir.log` / `mcp-D-local-dir.log`
    （**A/B = 云盘目录与连接根都明确报错；C = 无目录参数仍 200 条 / 云盘 101 条（无回归）；D = 本地目录语义正常**）
  - 产物：`release/local-builds/Alpha File Manager_3.22.0_x64-setup.build-20261125-b55.exe`（46,114,293 B / SHA256 `6F4ABEA1…9635B5B`，与 b54 的 `E4731E91…` 不同）
  - ⚠️ **未验**：界面级像素 / 键盘路径（CDP 通道不可用，见 W-19）；沙箱内无键盘 ⇒ 键盘路径不可假称已验

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
| 2026-11-25 | **批次 47**：**W-9 闭合**（`commands.rs` 新增跨连接共用的 `REBUILD_LOCK` + `acquire_rebuild_lock()`，guard 在任何文件操作前取得，并发时**排队**而非报错）；**W-10 闭合**（`remote_index.rs` 逐目录 WARN 改为 `SkippedDirs` 计数 + 每连接一条汇总）。新增 3 条单测；`cargo test --lib` **1696 passed / 0 failed / 3 ignored**（基线 1693 + 3）、`npm run check` EXIT=0（334 文件 / 3597 用例）；CHANGELOG 3.22.0 段补两条英文条目；**未提交、未打包** —— 00:13 起本机锁屏，界面级验收一项没做，§6 第 1 项原封退回。详见 handoff §二十一 |
| 2026-11-25 | **批次 48**：**B-3 闭合 + §6-1① 已证**。界面勾选 `includeInGlobalSearch` 与保存搭同一次 `saveConnection` 落盘（`remote-credentials.json` 09:00:31 / `user-remote-connections.json` 09:00:32）⇒ 只触发一次 {百度, 阿里云} 重建：09:00:32 开始 → 09:10:51 commit `opstamp=23335` / 23334 条（百度 20000 撞顶 + 阿里云 3334），commit 295ms、总 619s，`remote-index/meta.json` 1 segment / 无 deletes；MCP `search_files` 实测阿里云条目**可检索**（`Midjourney`→39 含 2 条阿里云、`AI`→260 含 1 条）。**同时新开 W-12（中文子串检索失效）**：`技巧`/`收集`/`心得`/`分析` 对阿里云 0 命中，根因是 `default` 分词器把整段汉字当一个 term + 查询侧 fuzzy distance=1，**与云盘无关**。装置 `.superpowers/sdd/b48/{aliyun-check-and-save,cjk-probe,cjk-probe2,cjk-probe3}.ps1`，证据 `.superpowers/sdd/b48/mcp-probe{,2,3}-*.log`；**未改源码、未提交**；B-5 / B-6 仍待用户在界面操作 |
| 2026-11-25 | **批次 49**：**W-12 闭合（含真机）**。`query.rs` **+201 / −1**：新增 `build_substring_query`，对非 ASCII token 追加 `RegexQuery(".*<escaped>.*", fields.name)` 作 `Should` 子查询 ⇒ 绕开 `default` 分词器的「整段汉字 = 一个 term」，**未改索引 schema ⇒ 23,334 条索引不重建**（装机后 `remote-index/meta.json` mtime 仍 09:10:51 已证）；新增 6 条单测（`mod cjk_substring`）。门禁 `cargo check --lib` / `cargo test --lib`（**1702 passed / 0 failed / 3 ignored**）/ `npm run check` 全 EXIT=0。`npm run tauri:build` 10m17s 出新包 → `release/local-builds/Alpha File Manager_3.22.0_x64-setup.exe`（46,148,338 B / SHA256 `26051490…BCD6`，**不覆盖**顶层已发布副本 `4AB5032B…D470`）→ 静默安装 10:54:38–10:55:17。真机 A/B（MCP）：`技巧` 7→151（阿里云 0→1）、`收集` 0→15（0→2）、`心得` 1→21（0→1）、`关键词分析` →7（0→1，score 0.92）、`Midjourney` 39/2 不变。**同时新开 W-13**（高频 2 字词候选 500 饱和被挤出，排序层，未处理）。**未提交**（6 文件）。详见 handoff §二十三 |
| 2026-11-25 | **批次 50**：**W-13 的召回层已闭合（含真机）**。`query.rs` 新增**独立子串候选池**（`build_substring_only_query` / `collect_substring_candidate_addresses` / `merge_candidate_pools`；本地与云盘两条路径各一条，云盘池带当前勾选连接过滤）⇒ 只靠子串正则命中的文档不再在候选阶段被 BM25 常量分挤掉；**未改索引 schema、未改最终排序口径、未升版本号**。新增 7 条单测（`mod w13_substring_pool`）；`cargo test --lib` **1709 passed / 0 failed / 3 ignored**（基线 1702 + 7）、`npm run check` EXIT=0（334 文件 / 3597 用例）。`npm run tauri:build` 出新包（46,156,750 B / SHA256 `2366E3BA…D011`，落 `release\local-builds\`，**不覆盖**顶层已发布副本 `4AB5032B…D470`）→ 静默安装 11:49。真机 A/B（MCP `search_files`，`result_limit=200`）：`技巧` 151/阿里云 1、`收集` 15/2、`心得` 21/1、`关键词分析` 7/1、`Midjourney` 39/2 —— 与批次 49 **逐字一致（无回归）**；`分析` 仍 200/阿里云 0 ⇒ 应用日志坐实**云盘子串候选 307/307 全部入池**，残留为**同分排序 / 结果容量**口径 ⇒ **新开 W-14**（按 §5「排序策略暂不做」保留给用户拍板）。**已提交**（主提交 `471ea5b1`，补记提交紧随其后，均**不推送**）。详见 handoff §二十四 |
| 2026-11-24 | **批次 46**：**§6-1 闭合**（提交 `613e1958`，4 files / **+485 / −94**，含批次 45 的 `global_search/{remote_index,commands}.rs` 配额改动 + 两份台账；**版本号与 CHANGELOG 未动**）；**§6-2 判闭合**（干净包**无 CDP 旗标**下改用应用自带只读 MCP 做 A/B：百度已勾选 `count=13` 含 2 条 `sfm://remote/0f0696ba…/`、夸克未勾选 `count=0`，而盘上索引里夸克条目 **49 条** ⇒ 过滤在**检索期**生效，非假阴）；**§6-3 推进到扫码边界**（**登录入口在「编辑」弹窗内，不在远程页上** —— 这是连续两轮失败的真根因；`阿里云盘 登录` 窗已开到扫码页，等用户扫码；弹窗内 `会话已过期，需要重新登录` = B-3/B-5/B-6 界面侧根因）；§3 增补登录入口说明、§6 重排为批次 47+、§7 增补 b46 证据块；**未改任何源码**，门禁用例数与批次 45 续篇逐字一致。详见 handoff §二十 |
| 2026-11-25 | **批次 51**：**只读取证轮 —— 未改任何源码、未动排序口径、未打包装机**。开工状态（12:38，探针 `.superpowers/sdd/b47/state.ps1`）：`LOGONUI=none` + `FOREGROUND=69626`（未锁屏、有前台窗口，界面级操作前置成立），但 `APP NOT RUNNING`（批次 50 的进程已退出，真机取证前需重新起应用）；云盘索引 mtime 仍 `2026-11-25 09:10:51` ⇒ **23,334 条索引未被重建**（与批次 50 一致）。HEAD `804e4f68`、`git status --porcelain -uall` 空。核心产出 = **坐实 W-14 候选① 可行并把代价写进 §4**：`modified_time` 为 `FAST`+`STORED`（`index.rs:53`），生产写入点写真实 mtime（本地 `scan.rs:266/277`、云盘 `remote_index.rs:332`），全仓 `modified_time => 0u64` **逐处核对都在 `#[cfg(test)]` 夹具里** ⇒ 不需要重建索引；`materialize_candidates`（`query.rs:870-887`）本地与云盘共用 ⇒ 命中已带 mtime（`types.rs:81`）；tie-break 落点 3 处（`query.rs:724` / `919` / `929`），云盘在合并前不截断（`query.rs:629`）⇒ 改 `:724` 即可决定云盘条目能否进窗口。同时写下两条代价：① 与 `query.rs:697-698` 成文取向「本地结果优先」冲突；② 同分恒 0.92（不含名称长度/命中位置）⇒ **任何同分规则都不能保证靶子出现**，据此在 §4 新增**组合规则候选④「来源保底配额 + 组内 mtime」**。§6 各项本轮均未推进（第 1/2/3 项需用户在场，第 4 项 `rustup` 需授权 —— 门禁仍可借 1.90.0 工具链跑，不修不阻塞，本轮实测未跑门禁），第 5 项三项拍板（版本号 / CHANGELOG / 索引配额口径）连同 W-14 排序规则**已一次性提交用户拍板**。**本批次改动 = 台账 1 个文件**（§4 W-14 行 + 本行） |
| 2026-11-25 | **批次 52**：**W-14 闭合（候选② + 组内新近，含真机 A/B）**。`query.rs` 两处改动：**(A)** `merge_search_results` 改用携带来源标记的元组、新增 `reserve_cloud_seats_in_cut_band`（只在卡窗口边界的那个同分带内让出本地席位给云盘，目标占半窗；不碰更高分、不丢数据、带内仅 1 席不给云盘、云盘无货不硬凑）；**(B)** 新增 `sort_remote_hits_by_recency`（云盘池内「分数降序 + 同分 mtime 新近」，**仅限同一索引内部**，否则配额席位被最早写入的连接吃光）。TDD：`mod w14_cloud_band_quota` 6 条先 RED（2 passed / 4 failed）后 GREEN（6 passed）。门禁 **1715 passed / 0 failed / 3 ignored** + `npm run check` **334 / 3597 / EXIT=0**。真机（`tauri:build` 装机，同机同索引）：`分析` 窗口内云盘 **1 → 101 条**，其余五例逐字不变、靶子命中不回归。**订正批次 50 的指认**：`result_limit=500` 实测 `分析` 的云盘 251 条全来自连接 `0f0696ba`、**阿里云盘 0 条** ⇒「搜『分析』看不到靶子」剩余根因在云盘索引对该词的召回粒度（文件名里是「关键词分析」），属 W-13 延伸、非配额层；靶子 mtime 是否够新因此**未能真机验证**。未升版本（3.22.0）/ 未动 CHANGELOG / 未重建索引 / 未推送。详见 handoff §二十五 |
| 2026-11-25 | **批次 53**：**W-15 闭合（同分带按 `connection_id` 轮转席位，含真机）**。判别实验把用户残留症状重新定性：靶子**已被云盘子串通道召回**（`键词分析` 7 条含靶、`词分析` 41 条含靶、`使用心得+关键` 去 `+` 仍 0 条 ⇒ 缺陷 A 为真），**批次 52 的立项前提「靶子未进候选池」不成立** —— 真因是批次 52 引入的「云盘池同分带一律按 `modified_time` 降序」让整个 0.92 带被单个连接吃光（`分析`@200 云盘 101 条里 `0f0696ba` 独占 68 席、靶子所属 `646dacdb` **0 席**；@500 为 218/33 但靶子仍未入窗），且靶子 `mtime=1680686620768`（2023-04-05）不够新。用户拍板「先修 B：tie-break 加连接席位」。改动仅 `src-tauri/src/global_search/query.rs`（**+357 / −12**）：`sort_remote_hits_by_recency` 改名重写为 `order_remote_hits`（`:648`，调用点 `:633`）——先「score 降序 + mtime 降序」铺平、切出同分带、带内**每轮每个连接出一条**（新增 `rotate_band_by_connection` `:681` + `cloud_connection_id` `:730`，复用 `crate::remote::parse_remote_path` 取连接身份 ⇒ **未给 `GlobalSearchResultEntry` 加字段**，不动 8 处构造点/序列化契约/前端 TS 类型）；组序按各组最新 mtime 降序、并列按连接 id 升序，`total_cmp` 保确定性，`groups.len()<2` 原样返回（与批次 52 逐条一致）。边界全部沿用批次 52：只在同分带内重排 / 只重排不删数据 / 不碰更高分 / 不引入跨索引 doc id 新近 / 不引入跨来源 mtime 比较。TDD：新增 `mod w15_connection_fairness` 7 条 + w14 内索引级 1 条（夹具**必须同名文件**才构成真同分带，不同名云盘命中分数根本不等）。RED（临时把 `:672` 改回 `ordered.extend(band)` 关掉轮转）**18 passed / 3 failed**，三条红全是本批判判性用例 ⇒ 恢复后 GREEN `cargo test --lib`（借 1.90.0）**1723 passed / 0 failed / 3 ignored**（基线 1715 + 8）、`npm run check` **334 文件 / 3597 用例 / EXIT=0**。`tauri:build`（6m38s，EXIT=0）→ 新包装机复验（同机同索引，云盘索引 mtime 仍 `2026-11-25 09:10:51`）：`分析` 靶子 **窗口外 → 第 101 条**、阿里云盘席位 **0 → 33**、`COUNT=200 / CLOUD=101` 与改前**逐字不变**（批次 52「云盘占半窗」不回退）、`心得` 靶子 20 → **15**；`金桥`/`手机`/`关键词分析`/control 四例逐字一致。台账：§4 新增 **W-15 行**、W-14 行尾追加「批次 53 订正」段（缺陷 A 单独留 W-14，修它需给 `name` 加子串/ngram 子字段并**重建 23,334 条云盘索引**，用户已排到 B 之后）、§5 多连接排序行改为「**部分解除**」。⚠️ **界面级（非 MCP）复验仍未做**，且界面单次展示条数上限未查明 ⇒ 靶子在第 101 位是否等于「用户真的看到」尚未坐实；**未提交、未升版本号（仍 3.22.0）、未动 CHANGELOG、未重建索引、未推送**。另暴露一个独立作用域缺陷：装机版 MCP 传 `"directory":"sfm://remote/<conn>"` 会把云盘结果整体丢掉（候选 `0f0696ba` 单连接目录 → TOTAL=0），本批不修，记在 handoff §二十六·6。详见 handoff §二十六 |
| 2026-11-25 | **批次 53 补充（订校本批验收口径）**：fresh 查到全局搜索前端把 `DEFAULT_RESULT_LIMIT=50` 直接当请求 limit 且无分页续拉（`src/constants/index.ts:22-24`、`src/stores/runtime/global-search.ts:388`；可调区间 `[10,500]`）⇒ 本批「靶子进 `分析` 结果第 101 位」的判据**只在 MCP 通道（`limit=200/500`）成立**，默认配置下界面上仍看不到靶子（需把「结果条数」调到 ≥101）。装机实际 `resultLimit` 未取到 = 未验。故新登 **W-16**（界面条数上限挡住深位命中），批次 54 候选两条路：① 同分带内加「每连接席位上限」把靶子压进前 50；② 修缺陷 A 让精确查询 `关键词分析` 直接召回。取证与结论见 handoff §二十六·7、§7 批次 53 证据块新增两条 |
| 2026-11-25 | **批次 54 前置实测（W-16 原结论作废）**：fresh `.superpowers/sdd/b54/probe-limit50.ps1` 装机实测 limit=10/30/49/50/100/200 六档，靶子位次 10/17/27/**27**/52/102 ⇒ **界面默认 50 条下靶子第 27 位、可见**，批次 53 收尾时「默认窗口下仍不可见」的推断错误（错因：拿 limit=200 响应的前缀当 limit=50 的结果，而 `result_limit` 是合并层配额入参、不同 limit 走不同配额解，前缀不等于结果）。已就地改写 W-16 并在 handoff §二十六·8 留勘误；顺带修正历史取证口径：b53 `analyze.ps1:25` 记录的 rank 是 **0 基下标**，旧文档「第 101 位」实为 1 基第 102 位。批次 54 的「席位上限」路径实测无收益（靶子已是组内第 1 / 带内第 2 条云盘），改道方向待用户界面复检结论后再定；本轮未改代码、未升版号、未动 CHANGELOG、未提交 |
| 2026-11-25 | **批次 54（文案修复）**：⌘ 入口实测为命令面板、从来不含文件检索 —— command-palette.vue:88-118/207-223/318 只走命令注册表，builtin-commands 里没有任何搜文件命令，UI 也无字面「命令行」按钮。用户反馈的「搜不到」根因是占位符写着「输入命令或搜索...」，承诺了没的能力，与 W-15/W-16 排序无关。用户拍板「只改文案，不动功能」：en/ch 两处占位符去掉承诺、ch 补漏翻译的 tooltip；其余 15 个语言包登记为 **W-17**。面板不新增文件结果分组，「面板内搜不到文件」保留为设计现状。详见 handoff §二十六·9。 |
| 2026-11-25 | **批次 54 收口（装机 + 命令面板界面复验）**：门禁盘上回读 `npm-check-b54.log` = **334 文件 / 3597 用例 / `NPM_CHECK_B54_EXIT=0`**，打包 `tauri-build-b54.log` 末行 `TAURI_BUILD_B54_EXIT=0`。**装机已坐实**：7-Zip 从 b54 安装包内抽出 payload，SHA256 `B12B4892…D070F` 与已装 `D:\Program Files\Alpha File Manager\alpha-file-manager.exe` **逐字节一致**（b53 包 payload `0BFAC5D8…` 不同；`src-tauri/target/release` 副本 `F146E190…` 与两者都不同 ⇒「已装 exe vs target/release」这个口径失真，不可作装机判据）；旁证 注册表 VER=3.22.0、`uninstall.exe` mtime 2026-11-25 16:44:44 在构建之后。同版本号按 §2.9 会静默 Quit，故**未重复安装**。**界面复验（隔离桌面沙箱，§2.8）**：装置 `.superpowers/sdd/b54/b54-palette-sandbox.ps1`（`CreateDesktop` + `PostMessage` 鼠标 + UIA 取证）⇒ **PASS=8 / FAIL=0**（占位符「输入命令...」命中 1 个 ComboBox、其 HelpText 同串、旧「输入命令或搜索」0 命中、对话框标题「命令面板」2、描述「输入文字筛选，然后从列表中选择命令。」1、tooltip「打开 / 关闭命令面板」显示为完整中文、面板仍列 20 条命令、操作员桌面焦点 before/after 均 69626 未被触碰）；证据 `b54-palette-sandbox.log` / `b54-uia-before.csv`(215 行) / `b54-uia-palette.csv`(76 行) / `b54-palette-sandbox.png` / `b54-palette-dialog.png`。**en 侧**由 `dist/` 构建产物扫描佐证（新文案各命中 2 文件、旧占位符 0 命中；exe 内嵌资源压缩存储 ⇒ 字符串扫描无判别力）。⚠️ 沙箱内无键盘 ⇒ 空态与 cmdk 过滤**未取证**；沙箱帧本轮可渲染（非白帧），仍以 UIA 为准。**未提交、未升版号（仍 3.22.0）、未动 CHANGELOG、未重建索引、未推送**。详见 handoff §二十六·10 |
| 2026-11-25 | **批次 55**：**W-16 闭合**（装机实测 `globalSearch.exactMatch=false` ⇒ 缺陷 A 不是「看不到」的根因；`globalSearch.resultLimit` 键不存在 ⇒ 取默认 **50**，与批次 54 六档实测互证 ⇒ §二十六·7「默认 50 下看不到」的表述作废）｜**§二十六·6 云盘目录作用域检索：根因坐实 + 低成本修复**（Rust 侧「静默 0 条」→ **明确错误**，4 条新单测；**真机 MCP 四例全过**：云盘目录/连接根都报错、无目录参数仍 200 条/云盘 101 条无回归）｜**「用户真的看到云盘文件」证据链最后一环**用 **vitest DOM 级回归测试**闭合（新增 `global-search-result-list-remote-entry.test.ts` 4 条；**放弃 CDP 路线** —— Tauri 显式 `AdditionalBrowserArguments` 覆盖 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`，9333 无监听）｜**新开 W-18**（云盘目录递归检索真功能待用户拍板：索引 vs 实时遍历配额取舍）/ **W-19**（界面级像素复验通道受阻）｜顺带修一个**既有随机红**（`propsys_resolved_system_size_key_reads_real_file_size` 缺 COM 公寓，批次 53 全绿是运气）｜门禁 `cargo test --lib` **1727 passed / 0 failed / 3 ignored**、`npm run check` **335 文件 / 3601 用例 / EXIT=0**；打包 **BUILD_EXIT=0**、装机 payload 哈希一致（`E744CF66…41D562D`）｜**未升版号（仍 3.22.0）、未动 CHANGELOG、未重建索引、未推送**；提交 `<HASH>`。详见 handoff §二十七 |
