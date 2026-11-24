# 未完成事项台账（2026-11-24 起）

> **这份文件是什么**：跨批次的「还没做完 / 还没决定」的唯一账本。
> **怎么用**：每轮收工前进账（状态有变化就改行，不要另建一份）；开工前先读 §0 + §2 + §3。
> **本轮依据**：批次 44 收口（O-1 / O-2 / O-3 / O-6 全部闭合，升版 3.22.0 已装机）→ **批次 45** 复核并闭合 B-1（真机验收 + 盘上索引取证）。详见 `docs/handoff-2026-11-22-batch38-onedrive-readonly.md` §十六 / §十七。
> **最近更新**：2026-11-24（批次 45：B-1 闭合 + O-4 路线待拍板 + 2 条新观察项 W-7/W-8）

---

## 0. 一句话结论

仍然开着 **1 项**：界面级取证的**半通**路线（O-4）；
**批次 45 已闭合 1 项**：B-1（勾选变更即时生效，真机验收 + 索引侧证据双确认）。
阻塞 **5 项**（B-2…B-6，全部等外部条件或等 O-4）；
观察项 **8 项**（W-1…W-8，不阻塞发布）；
已拍板不做 **4 项**。批次 42 的 9 项已知限制已全部关闭：O-1 / O-2 / O-3 / O-6 于批次 44 闭合，O-4 半通，O-5 已推送闭合，O-7 / O-8 于批次 43 闭合。

---

## 1. 已闭合（留档，防重开）

| ID | 项 | 闭合于 | 靠什么闭合 | 证据 |
|---|---|---|---|---|
| O-8 | 剪贴板复制文件被文件占用中断 | 批次 43 | `clipboard.rs` 改「尽力复制」+ `ShCopyOperation` 加 `FOF_SILENT \| FOF_NOCONFIRMATION \| FOF_NOERRORUI`，3 个单测锁位 | `src-tauri/src/clipboard.rs`、handoff §十三·1/§十三·4 |
| O-7 | 添加网盘的「添加」按钮选中不明显 | 批次 43 | 按下态：边框 `--accent` 2px + 底色 `rgba(59,130,246,.14)` + 外发光；聚焦环同步加深。**未走 `scale(.97)`**（与既有 `.afm-primary:active` 冲突，见 `style.css:356-358`） | `src/themes/glass-theme.css:393-417`、handoff §十三·2 |
| O-5 | 批次 42 + 43 提交未推送 origin | 批次 44 | `git -c http.proxy= -c https.proxy= push origin main` → `453f20e8..af1add65`；**全局代理 127.0.0.1:7897 必须临时禁用**，否则 remote-curl 中断 | handoff §十六·6、`git ls-remote origin main` |
| **B-1** | 勾选变更是否**即时**生效（真机） | **批次 45** | **用户手工验收双向通过**：阴性（17:03 取消勾选百度 → 百度独有文件名立刻搜不到）+ **阳性（17:3x 重勾百度 → 同一文件名能搜到）**；再由索引侧证据排除「空索引导致的假通过」（验收时刻盘上索引非空且含百度条目，16:39:35 提交，见 §十七·1） | handoff §十七·1/§十七·2（含第 7 条阳性对照）/§十七·3、`remote-index\meta.json` |
| — | OneDrive 已连通账户的旧数据 | 批次 42 随下线一并删 | 用户拍板「彻底删除」；真实 `%APPDATA%\com.sigma-file-manager.app` 清理实测 `existed=False` | handoff §十二·2、`§十二` 表 |

---

## 2. OPEN —— 可推进，缺条件或需用户拍板

### O-4 · 界面级验证通道（半通，**不再全阻塞**）

- **批次 41 原判定**：`WebView2AdditionalArguments` 在 `tauri.conf.json:40-41` 已写，但 `verify-runtime-cfg.js:27-28` 只核 3 个键 → 不保证运行时生效；实测 CDP `9222` 不通。**批次 43 / 44 复测：仍不通。**
- **批次 42 实测（已修正）**：UIA 路线可用，见下。
- **批次 44 装机实测（订正批次 42 的悲观结论）**：UIA 路线**可用** ——
  `-WindowScope SingleWindow` + 遍历后代控件树可读到 `设置` 导航项 / `云盘检索` / 三条连接行
  （`已添加账户 · 百度网盘 · 未勾选` / `阿里云盘 · 已勾选 · 停用` / `Quark …`）；
  搜索页 `主页 / 全部 / 本地 / 云盘 / 类型 / 筛选` 等标签全部可见。
  批次 42 之所以只拿到 `Text "Alpha File Manager"`，是**控件树遍历深度不够**，不是 UIA 拿不到内容。
  装置：`npm run uia:check`（只读，不启停应用）；脚本 `.superpowers/sdd/b44/uia-*.ps1`（纯 ASCII）。
- **结论**：**界面级取证不再全阻塞** —— UIA 能证明「渲染出了什么」，但点不了开关、验不了「点下去有没有效」。
  CDP 仍不通（批次 44 复测）→ **交互级仍需 O-4**。
- **下一步（二选一，需用户拍板）**：
  - **路线 A（推荐）**：打**验收专用包** —— 把 `additional_browser_arguments()`（`src-tauri/src/tauri_capability.rs:225-239`）
    改成追加 `--remote-allow-origins=*`，并给 `verify-runtime-cfg` 加校验；**不入库**（或走独立 feature flag）。
    拿到 webview console 后，界面交互与 console 报错都能直接取。
  - **路线 B**：继续用 UIA 观察 + 手工验收（批次 44 / 45 已证明够用：批次 44 装机界面证据 + 批次 45 的 B-1 闭合都走这条）。
    成本是拿不到 console，只能读控件树。
  - **批次 45 补充的第三条证据渠道（不算路线，但好用）**：**应用日志**。
    `%LOCALAPPDATA%\com.sigma-file-manager.app\logs\Alpha File Manager.log` 能坐实「重建跑了几次 / 哪个连接收集了多少条」，
    比抓屏更可核。缺口见 **W-7**（查询链路零日志，所以日志**证明不了**过滤生效，只能证明扫描侧）。
- **阻塞什么**：不阻塞批次 44 / 45。只影响「点开关 / 看 console 报错」这类交互级取证。

---

## 3. BLOCKED —— 等外部条件

| ID | 项 | 等什么 | 恢复条件 |
|---|---|---|---|
| B-2 | 阿里云盘「登录成功但拿不到 unionId」缺陷（批次 41 登记，未修） | 上游 400 不可复现，需再次复现时抓请求体 | 用户再次遇到 → 提供 `afm-aliyun-login.log`（`src-tauri/src/remote/aliyun_auth.rs:880` 写的旁路日志） |
| B-3 | 登录态过期检测 / 重登提醒 | **O-4** | 界面自动化通道打通 |
| B-4 | 「先下结论后被推翻」复发核查（批次 40 19:16 一次） | 过程性，需再观察若干批次 | 连续多轮无同类返工即降级为「已改善」 |
| B-5 | 云盘下载落地的杀毒软件超时 | **O-4** | 同上 |
| B-6 | 三个网盘真实账户的界面级验收 | **O-4** | 同上 |

> B-3 / B-5 / B-6 都卡在 O-4：批次 44 已确认 **UIA 只够「读」不够「操作」** —— 能确认渲染结果，
> 无法代用户点开关、看 console 报错。若要走界面自动化，必须先修 O-4 的 CDP。

> ~~B-1 的口径更新（批次 44）：不再必须等 O-4。~~
> **批次 45 已闭合**：用户手工验收「取消勾选百度 → 百度独有文件名立刻搜不到」通过，
> 且**索引侧证据排除了「空索引导致的假通过」**（验收时刻盘上索引含 20000 条百度条目）——
> 完整证据链与残余风险见 handoff §十七·1 / §十七·2。
> **收口时补做阳性对照**：重新勾选百度 → 同一文件名**能搜到**（17:3x）⇒ 双向证据齐，
> §十七·3 该条残余风险清零；仍**只留**「另一半口径」（只勾百度搜非百度文件应搜不到）未做。

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
| W-7 | **云盘索引重建的「完成」时机在日志里不可判** | 只有逐连接 `INFO 云盘索引连接 <id> 收集到 N 条`（`remote_index.rs:200-205`），**没有写盘提交 / 收尾 / 耗时**日志。批次 45 取证时因此无法回答「17:03 用户搜索那一刻重建完了没有」，只能改用**盘上段文件 + mtime** 反推 | 修法很小：在 `write_remote_index` 提交后补一条 `INFO`（文档数 + 耗时）。建议与 **W-8** 同批做 |
| W-8 | **检索链路（查询侧）零日志** → 勾选过滤是否生效，无法从应用日志证实 | `query.rs` 全程 `Ok/Err` 静默：`search_remote_index` 返回 `None`（目录不存在 / 打不开 / schema 不符）时**静默降级成「只搜本地」**，`build_connection_filter` 为空集时**静默返回空云盘结果**（`query.rs:303-307`），三者对用户与日志都是同一种「没结果」 | 修法很小：降级 / 空集各补一条 `WARN`。**没有它，「搜不到」永远无法与「索引空」区分开**（批次 45 踩的就是这个） |

---

## 5. 已拍板不做（含理由，防重开）

| 项 | 拍板 | 理由 |
|---|---|---|
| OneDrive 个人模式 | 不做 | 批次 41 用户决定「仅企业版不支持」。`remote_client.rs:375-389` **本就只发 `scope: 'https://graph.microsoft.com/User.Read offline_access'`，未请求 Files.ReadWrite** → 该项已被代码现状满足，无需改动 |
| 多连接混合检索的排序策略 | 暂不做 | 用户「暂时不管」。现状：全局检索结果**未按 `connection_id` 分组、无跨连接去重**，排序按各连接内部顺序合并。要改需先定产品规则（按相关度？按连接？按路径？），无规则不动 |
| `search_files.rs` 死代码清理 | 延后 | 用户「可以暂时不处理」。当前仍有测试引用，删除需连带改测试，属独立清理批次 |
| 扫描期间冷启首查返回空结果 | 延后 | 用户「可以暂时不处理」（批次 41 实测坐实：扫描未完成时冷启首查返回空，再查一次正常） |

---

## 6. 下批次（批次 45+）建议顺序

1. ~~**B-1**：勾选变更即时生效的真机验收~~ —— **批次 45 已闭合**。
2. **W-7 + W-8 一起做**（很小的改动，一条 `INFO` + 两条 `WARN`）：把「重建完成」与「远端结果被过滤/降级」变成日志里可读的事实。
   收益是**以后任何一次「搜不到」的报障都能当场定性**，不必再靠 mtime / 段文件反推。
3. **O-4 路线拍板**（§2）→ 决定后 B-3 / B-5 / B-6 才有解；不拍板则这三项继续挂着。
4. 顺手项：批次 44 遗留的 `src-tauri/src/global_search/scan.rs:387` `unused variable: reader`（既有噪音，非回归）—— 想清噪音就单独一个小 commit，别混进功能批次。
5. B-2 等用户再次遇到阿里云盘 400；期间不动代码。
6. 云盘索引条目上限：`REMOTE_INDEX_MAX_ENTRIES = 20_000`（`remote_index.rs:48`）已在批次 44 装机时**被实测触发**（百度目录超上限，日志刷 `未继续下钻`）→ 是否提额 / 改分页续扫，**待用户决定**。

---

## 7. 证据路径

> AGENTS.md 的 `§0.3 验证装置地图` 已补上这三类装置的正确用法与「`tauri dev` 不能用于界面验收」警示；本节只留本批次的具体取证指针。

- **批次 45 索引取证**：`.superpowers/sdd/b44/rebuild-final.{ps1,txt}`（`meta.json` 原文 + mtime）、
  `.superpowers/sdd/b44/applog-remote-index.{ps1,txt}`（应用日志 remote_index 行全量抽取，293 命中）、
  盘上实况 `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\remote-index\`（`meta.json` 顶层 `segments` + 6 个段数据文件，`max_doc: 20000`）。
  **注意应用日志时间戳是 UTC（本地 = UTC+8）**。
- **批次 44 UIA 证据**：`.superpowers/sdd/b44/uia-report.txt`（门禁）、
  `.superpowers/sdd/b44/uia-observe.txt`（装机界面）、`.superpowers/sdd/b44/uia-*.ps1`（纯 ASCII 装置）、
  `.superpowers/sdd/b44/install-status.txt`（装机状态）、`.superpowers/sdd/b44/check3.log`（npm run check）、
  `.superpowers/sdd/b44-cargo-test.log`（cargo test --lib）、
  `release/sigma-file-manager_3.22.0_x64-setup.exe`（42,981,492 B / SHA256 `11594C69…` 尾 `F6D2`）。
- **门禁**：`npm run check` / `npm run test:run` / `npm run verify:all` / `cargo test --lib` / `npm run tauri:build`。
  批次 44 基线：**334 文件 / 3597 用例全绿**、`cargo test --lib` **1691 passed / 0 failed / 3 ignored**、
  `verify-changelog` **67 条目**、`cargo clippy --all-targets -- -D warnings` 见 §十六·3。
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
  `release/sigma-file-manager_3.21.0_x64-setup.exe`（批次 43 包）

---

## 变更记录

| 日期 | 改动 |
|---|---|
| 2026-11-24 | 建档：收口批次 42 的 9 项已知限制 + 批次 41 遗留 + 批次 42 交接里的 3 个待拍板 |
| 2026-11-24 | 批次 43 收口：O-7 / O-8 闭合（commit `f1d753b5`）；O-4 从「全阻塞」改判为「半通」并拆出 B-6；O-5 范围扩为批次 42+43 |
| 2026-11-24 | 批次 44 收口：**O-1 / O-2 / O-3 / O-6 闭合**，O-5 仅剩推送；升版 3.22.0（`1b0b32e3`）、收口 `af1add65`；装机实测 + 单测坐实 O-3 的「首次升级不重建」边界，**CHANGELOG 补了「升级后需打开设置页」的说明**；UIA 改判「可用但只读」；W-1 行数订正、W-3 根因定位；§5 新增 2 项用户拍板；§6 换成批次 45 顺序；handoff 新增 §十六 |
| 2026-11-24 | 批次 44 续：**O-5 闭合**（推送 `453f20e8..af1add65`，commit `8191bf1b`，需临时禁用全局代理 127.0.0.1:7897）。OPEN 归零；§6 重写为批次 45 待办 |
| 2026-11-24 | **批次 45**：**B-1 闭合**（用户手工验收 + 盘上索引反证，详见 handoff §十七）；O-4 补「下一步二选一」待拍板 + 新增第三条取证渠道（应用日志）；**新开 W-7（重建完成时机日志不可判）/ W-8（查询侧零日志，「搜不到」与「索引空」不可区分）**；§6 重排为「W-7+W-8 优先 → O-4 拍板」；§7 增补批次 45 证据路径 |
