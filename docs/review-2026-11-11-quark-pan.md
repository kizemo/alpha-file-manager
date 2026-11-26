# 夸克网盘接入 AFM「远程位置」技术方案 · 独立评审报告

> 本报告是 `docs/plan-2026-11-11-quark-pan.md` 的独立评审；方案修订已按本报告逐条执行。

- **评审对象**：`docs/plan-2026-11-11-quark-pan.md`（1266 行，8 章）
- **评审日期**：2026-11-11
- **评审性质**：独立复核。全部结论基于对仓库与 A 层产物的**只读**核验；本轮未修改仓库任何源码 / 配置；本报告已提升为 `docs/review-2026-11-11-quark-pan.md`。**未运行** `cargo build` / `cargo test --lib` / `npm run check` / `tauri build`，**未触碰** `%APPDATA%\com.sigma-file-manager.app\user-data\`。
- **证据约定**：每条结论给出 `文件:行号`；无法用只读手段确认的，明确标注「推断」或「未核验」。**不把「方案里这么写」当作证据**。
- 被评审方案自身的行号统一记作 `plan:行号`。

## 结论

**需修改后放行。**

方案的调研质量在同类文档里属上乘：A 层（本地 quark-cloud-drive 客户端 bundle）实证扎实，`file/sort` 端点、query 参数、响应形状、错误码表、下载 URL 过期语义等核心主张经复核**基本全部成立**；§2.2 给出的仓库锚点绝大多数准确，可直接当施工图用。

但有 **3 个必须先改的阻塞项**，都在执行层：

1. **UI 集成模型与仓库现状直接矛盾**（B1）：方案 §4.4/§4.5 的「协议下拉加项 → 选夸克网盘」建立在错误前提上——仓库里远程连接表单的协议下拉**刻意不含网盘协议**，网盘走独立分区 + 硬编码分支。按现方案实施会做出一个「选了却填不了对应字段」的死路。
2. **T0 没有 cookie 获取路径**（B2）：T0 是「头号风险，必须先做」的阶段，却没有规定如何合法取得 cookie，实操上跑不起来、也无法被第二个人复现。
3. **T0 没有量化通过判据**（B3）：只写了「二选一，不许模糊」，没写「什么算通过 / 什么算失败 / 什么算证据不足」，实测结论会因人而异，且极易把「参数没抄全」误判成「路线不成立」。

这三点都不是措辞问题，而是**前置条件缺失或与现状冲突**。修掉之后，方案的技术主体可以放行；其余 Major/Minor 均可在实施中同步修正。

## 阻塞项（Blocker）

### B1 — 「协议下拉加项」的 UI 集成模型与仓库现状直接矛盾

**触发条件**：任何人按 §4.4（plan:690）/§4.5（plan:712）去实现前端接线。

**核验到的事实**：

- `src/modules/settings/ui/categories/remote/connection-form.vue:63-69` 有一段明确注释：「**不含百度网盘**：网盘有自己的标签页与专属的录入项，把它塞进这个下拉等于给用户一条『选了却填不了对应字段』的死路」，紧接着 `const PROTOCOLS: RemoteProtocol[] = ["ftp", "ftps_explicit", "ftps_implicit", "sftp"];` —— 下拉里**没有网盘协议**。
- 网盘协议是在 `connection-form.vue:230-232` 由 `props.variant === "netdisk" ? "baidu_pan" : ...` **硬编码**的。也就是说，这个表单自己不是协议选择器，协议由上游决定。
- 上游决定点在 `src/modules/settings/ui/categories/remote/connections-section.vue`：`:43-44` 用 `isBaiduPanProtocol()` 把连接分成「网盘 / 远程」两个分区；`:87` 定义 `activeVariant`；`:93-97` 的注释写着「带着 baidu_pan 切到远程连接标签，表单会选出一个下拉里根本不存在的值」，因此有回落逻辑。
- 同文件 `:32` 的注释明说：「判定口径只有一个 —— `isBaiduPanProtocol`。以后接入阿里云盘之类的网盘……」 ⇒ **网盘分区本来就是为多网盘协议预留的扩展位**。
- `src/modules/nav-sidebar/components/remote-nav-panel.vue:57-74` 的 `sections` 数组里，网盘分区同样只有**一个** `protocol: "baidu_pan"` 字段。

**影响**：方案描述的「协议下拉加项」在现仓库里**没有落点**。要么把网盘协议塞进 `PROTOCOLS`（直接违反 `connection-form.vue:66-67` 的设计注释，且夸克网盘没有 host/port/密码字段可填），要么另造一个协议选择器（与仓库结构重复）。

**证据**：`connection-form.vue:63-69`、`:230-232`；`connections-section.vue:32`、`:43-44`、`:87`、`:93-97`；`remote-nav-panel.vue:57-74`；方案 `plan:690`、`plan:712`。

**建议改法**（方案里需写死其一）：

- **(a) 扩成网盘 provider 选择器**：`connections-section.vue` 的网盘分区增加「百度网盘 / 夸克网盘」二选一；`remote-nav-panel.vue` 的 `sections` 由「每分区一个 protocol 字段」改成「每分区一个 provider 字段」；`sections.ts` 的 `sectionOfProtocol` 相应改判定。改动面比现方案大，但与仓库既有结构一致。
- **(b) 两个并列入口**：网盘分区与远程分区一样各一个「添加」按钮，夸克走独立表单组件（像百度一样仍在 `connection-form.vue` 内部分支，但用 `variant === "quark"` 区分）。
- 无论选哪个，都必须**补齐 §4.4 前端文件清单**：`connections-section.vue`（现方案完全没提）与 `src/modules/settings/ui/categories/remote/index.vue` 才是真正的分区决策点。

### B2 — T0 没有规定 cookie 的获取路径，阶段不可执行

**触发条件**：执行 T0（plan:859-884）。

**核验到的事实**：T0 只写「用真实登录拿到的 cookie 直接打 file/sort」（plan:861），列出 8 个待答问题与凭据处理约束（plan:883），但**没有任何一处说明 cookie 从哪来**。而 A 层实证：本地客户端「从**不按名字**取 cookie」（plan:567 U1），`__pus` 只有 B 层开源项目侧证据（plan:100、plan:752）。

**影响**：T0 是全案头号风险闸门。缺 cookie 来源就既跑不起来、也无法被第二个人复现；一旦有人用「过期/残缺 cookie」测出 401，会被误读为 R1 成立、整条路线作废。

**证据**：`plan:861`、`plan:883`、`plan:567`（U1）、`plan:752`。

**建议改法**：在 T0 里写死一条可复现路径（当前只读环境下唯一不依赖改本仓库的办法）：

> 用 **Edge** 登录 `https://pan.quark.cn/` → DevTools → Network → 任一 `drive-pc.quark.cn` 请求 → 复制整条 `Cookie` 请求头 → 存入 `.superpowers/sdd/quark/t0-cookie.txt`（**不入 git**，阶段结束销毁）。

并写明：① 采集整罐 `.quark.cn` cookie（U11 待验，故先采整罐、再在 T0 内分组裁剪）；② 记录采集时刻；③ 不做任何写操作（与 plan:883 一致）。

### B3 — T0 缺少量化通过判据，易把「参数没抄全」误判成「路线不成立」

**触发条件**：T0 实测得出 401 或 `code != 0` 时。

**核验到的事实**：plan:876-879 只写了「一份实测记录」「明确写明主路线成立或必须走退路（二选一，不许模糊）」，没有「什么算通过」的可判定条件。同时 §4.8 R1（plan:836）把概率定为「中高」，但 §4.6 的列表请求规范（plan:757-765）**漏了真实客户端会带的 `device_model` 与 `ut`**（详见 M1）—— 用缺参请求去测，必然容易被拒。

**影响**：T0 极可能产出「非浏览器来源被拒 → 走退路」的错误结论，而真实原因只是少抄了两个 query 参数。

**证据**：`plan:836`、`plan:757-765`、`plan:876-879`；缺参事实见 M1。

**建议改法**：把 T0 闸门改成三态 + 量化判据：

| 结果 | 判据 | 动作 |
|---|---|---|
| **主路线成立** | HTTP 200 **且** `code == 0` **且** `data.list` 与 `metadata` 自洽（条数 ≤ `_size`，`metadata.total` ≥ list 条数）**且** 连续 3 次请求都成功 **且** 用列表里任一子目录 `pdir_fid` 再列举一次也成功 | 进入 T1 |
| **必须走退路** | 带**合法且完整** cookie、**且已按 §4.6 补齐全部 query 参数**后，仍返回 HTTP 401/403 **或** `code == 31001 / 31004` | 转 §7.1 退路，并同时给出退路最小可运行证据（plan:879） |
| **证据不足** | 5xx / 超时 / 429 / 弹验证码 / cookie 采集不全 / 参数未补齐 | **不得**判「路线不成立」，先补齐再测；本态必须写进记录 |


## 重要问题（Major）

### M1 — §4.6 的列表请求规范漏了真实客户端实际带的 `device_model` 与 `ut`（与 §3.2.3 自相矛盾）

- **事实 1**：方案 §3.2.3 的 `fileRequest` 行（plan:471）**记录了** request 拦截器注入 `uc_param_str / device_model / getUcParams() / ve`；但同一节的 `driveRequest` 行（plan:456-462）**只列了实例级 params、timeout、withCredentials，没有拦截器一行**。
- **事实 2**：`file/sort` 走的正是 `driveRequest`（模块 781615，offset ≈5313953）。复核该实例：**存在** request 拦截器 `m.interceptors.request.use(e => { const t = getUcParams(); return e.params = { uc_param_str: UC_PARAMS_STR, device_model: getDeviceNameSync(), ...e.params, ...t }, e })` ⇒ 真实客户端在 `file/sort` 上**也**带 `device_model`（来自 `getDeviceNameSync()`，即本机主机名）与 `ut`（来自 `window.QUARK_WSGUID`，模块 514962）。
- **事实 3**：§4.6 的请求形态（plan:757-765）里 `device_model`、`ut` **都没有**。
- **影响**：按 §4.6 实现并按 §4.6 去 T0 实测 ⇒ 少两个参数 ⇒ R1 被误触发（与 B3 同源）。`device_model` 还会把本机主机名带出（见 N8）。
- **证据**：`plan:456-462`、`plan:471`、`plan:757-765`；产物 `asar/dist/renderer/index.js` offset ≈5313953（模块 781615）、模块 514962。
- **建议**：§4.6 的请求形态补 `device_model` 与 `ut`，并注明「`device_model` 取主机名，属敏感项，见 §6.4」。

### M2 — R1 概率「中高」被高估，且漏记了「出口 IP 被拦截」这一独立风险

- **事实 1**：B 层来源 `https://docs.hunhepan.com/reman/pantools.html` 原文明确写：「实际上，只需要名为 `__pus` 的 cookie 即可」，且该工具是**原生 Windows 程序（Go，主程序 main.exe / rm-task.exe）用单 cookie 直连**，**不是浏览器来源** ⇒ 「非浏览器来源就被拒」至少存在一个可工作的反例。
- **事实 2**：同一页面原文：「在 2024/07/24 晚起，貌似国外服务器访问夸克 API 时会被拦截，所以暂时开发了本工具用以在本地执行」 ⇒ 存在一个**与请求来源无关、只与出口 IP 有关**的独立失败模式。
- **影响**：R1 定「中高」⇒ 会被当成大概率事件；但真实画像更接近「来源限制小、IP/风控限制大」。方案 §4.8（plan:832-851）没有「出口 IP 被拦截」这一行，R2 也只泛指风控/验证码。
- **证据**：`plan:836`（R1）、`plan:837`（R2）；B 层 `https://docs.hunhepan.com/reman/pantools.html`。
- **建议**：R1 概率下调为「低～中」；新增 **R17「用户出口 IP 被风控/拦截」**（概率中、影响高、缓解：T0 记录实测出口 IP 与响应特征；命中则整体走退路，且 UI 需给出明确文案而非笼统的「登录失败」）。

### M3 — T0 的工具形态与闸门方式需要收紧

- **事实**：plan:861 允许「写一个只读探针（`src-tauri/src/quark_probe.rs`，`AFM_QUARK_PROBE=1` 才生效）**或**用 curl」；plan:678 又把 `quark_probe.rs` 列为**新增**后端文件 ≈300 行。而 T0 的闸门是「`cargo test --lib` 仍通过」（plan:881）。
- **影响**：T0 阶段就往产品代码树塞探针模块，一旦 T0 结论是「走退路」，这段代码要么白写、要么成为长期负担；同时 `cargo test --lib` 闸门把 T0 变成「必须先能编译」的任务，拖慢头号风险验证。
- **证据**：`plan:861`、`plan:678`、`plan:881`。
- **建议**：T0 **只允许** curl / `Invoke-WebRequest` 之类的**外部只读探针**（不碰产品代码树）；`quark_probe.rs` 移到 T4 之后，或在 T0 结论为「主路线成立」后再决定是否需要（一个 env-gated 常驻探针对最终用户没有价值）。T0 闸门改为「§8 的 8 问逐条有结论 + 主路线/退路/证据不足三选一 + 临时凭据文件已销毁」。

### M4 — i18n 工作量被显著低估

- **事实**：方案 §2.2.8（plan:287-296）说每个语言包「**4 处** baidu 命中」，并要求夸克「在 **17 个**语言包各加同样 4 处」。
- **事实**：`src/localization/messages/ch.json` 里 `protocols.baidu_pan`（`:655`）与 `baiduLoginRequired`（`:681`）是**平铺键**；而 `baidu`（`:723`）是一个**嵌套对象**，叶子串分布在 `ch.json:723-748`，约 **15 个**键，且含**百度专属措辞**（如 `stoken_expired` / `stoken_expiring_soon`）。
- **影响**：夸克没有 STOKEN，对应的「失效/临期」文案要**重新起草语义**，不是把 stoken 换成 pus 就能抄；17 语言 × 约 15 键 ≈ **250+ 条**新串（含 16 种非中文语言的人工翻译），而不是 17 × 4 = 68 条。
- **证据**：`ch.json:655`、`:681`、`:723`、`:746`、`:723-748`；`plan:287-296`、`plan:696`。
- **建议**：§4.4 的 i18n 行改为「各加 **1 个约 15 键的嵌套对象** + 2 个平铺键」；T7 工时 0.5 → 1.0 人天（见「工作量复核」）。

### M5 — 工期与产物规模不匹配

见「工作量复核」章节。摘要：方案 8.0 人天偏乐观 **40–55%**，复核为 **11.5–12.5 人天**（若 T0 判定走退路，再 +1.5～2.0）。依据见该章节的模板体量实测表。

### M6 — 多处行号/文件清单与仓库现状不符（含两处会误导实施的关键点）

| 方案表述 | 实际 | 证据 |
|---|---|---|
| `remote/mod.rs` 的 `41-68` 是「三段 `pub use baidu_*`」（plan:83） | 实为 **4 段**（`:41` / `:53` / `:58` / `:65`） | `src-tauri/src/remote/mod.rs:41,53,58,65` |
| `connection-form.vue`（**927 行**，plan:255） | 实为 **1032 行** | `src/modules/settings/ui/categories/remote/connection-form.vue`（1032 行） |
| `connection-form.vue:26` 是「协议选项来源」（plan:261） | `:26` 是一行 **import**；协议选项在 **`:69`** | `connection-form.vue:26`、`:69` |
| `remote-tab-labels.ts`「1 处命中，按协议取名」，要加夸克（plan:279） | 该文件**完全不含** baidu / protocol / netdisk 字样，是**协议无关**的（只吃 `connectionName`）⇒ **不需要改** | `src/modules/tab-bar/utils/remote-tab-labels.ts:1-60`（全文 60 行） |
| `baidu_login.rs`（**996 行**，plan:182） | 实为 **1092 行** | `src-tauri/src/baidu_login.rs`（1092 行） |
| `trait BaiduApi` 在 `:75`（plan:134） | 实为 **`:77`** | `src-tauri/src/remote/baidu_provider.rs:77` |
| `trait BaiduSessionStore` 在 `:178`（plan:136） | 实为 **`:179`** | `src-tauri/src/remote/baidu_provider.rs:179` |
| `failure_from_http_status()` 在 `:644`（plan:113） | 实为 **`:647`** | `src-tauri/src/remote/baidu_api.rs:647` |
| `connection_test.rs` 命中 **14**（plan:302） | 实含 **24** 个 test 函数 | `src-tauri/src/remote/connection_test.rs` |
| `commands.rs:358/:379/:586` 是「`provider_with_protocol` 的三个 BaiduPan 分支」（plan:179） | 三处 BaiduPan 属实，但**分属三个不同函数**；`provider_with_protocol` 函数体在 **`:573`** | `src-tauri/src/remote/commands.rs:358,379,586,573` |

### M7 — §4.7 错误映射表不穷尽

- **事实 1**：方案 §4.7（plan:810-829）自述「**穷尽**」。但 A 层错误码表在 `index.js` 里有**两份副本**（模块 887411 @6041787，另一份 @6030862），第二份额外含 **`23018` / `25008` / `38001`**。
- **事实 2**：模块 781615 的错误处理是 `if ([31001, 31004].includes(r.code) && ...user.logout())` ⇒ **`31004` 与 31001 同等对待（触发登出）**，而 §4.7 只映射了 `31001`，`31004` 会落到兜底 `Unknown`。
- **事实 3**：`24005` 的 A 层原文是「文件/文件夹状态已更新，请返回重试」；方案 §4.7 写「文件状态已更新 → Protocol」方向对，但方案 §3.2.4 的转述与「隐私空间锁定」混淆（`24005` 原文无「隐私空间」字样）。
- **影响**：未列出的码走 `Unknown` ⇒ 用户看到笼统错误；`31004` 尤其糟（会话已失效却提示未知错误）。
- **证据**：`plan:810-829`；产物 `index.js` 模块 887411 @6041787 / @6030862、模块 781615。
- **建议**：§4.7 补 `31004 → Auth`、`23018 / 25008 / 38001` 三行（语义按 A 层文案），把 `24005` 的描述改成原文；表脚注注明「`Unknown` 兜底必须携带原始 code 便于排障」。

### M8 — 「波及 4 个既有 provider」的数量不准，且 `read_file_stream` 是假想符号

- **事实**：plan:37 的「流式下载（`read_file_stream`）…… 要改 `RemoteProvider` trait，**波及 4 个既有 provider**」。`read_file_stream` **不是仓库里存在的符号**——`provider.rs` 的 trait 只有 9 个方法（`:40 / :46 / :58 / :69 / :97 / :106 / :118 / :131 / :147 / :177`），且 `:55` 的注释明确「刻意不提供流接口」；全仓库 `impl RemoteProvider for` 共 **5** 处。
- **影响**：数量写错会让「改 trait 的代价」被低估；用不存在的符号做决策锚点会误导后续检索。
- **证据**：`src-tauri/src/remote/provider.rs:40-177`（含 `:55` 注释）；`plan:37`。


## 次要问题（Minor）

- **N1** plan:113 的 `failure_from_http_status` 行号应为 `:647`（同 M6 表）。
- **N2** plan:83 的「三段 `pub use baidu_*`」应为 4 段（同 M6 表）；且 §4.3 plan:679 写「三个 `mod` + 两段 `pub use`」，与 §2.2.1 的自述不一致（夸克侧确为 3 mod + 2 pub use，但**参照系**写错）。
- **N3** plan:255 的 `connection-form.vue` 行数 927 → 1032；plan:261 的 `:26` → `:69`（同 M6 表）。
- **N4** plan:200 把 `poll_loop` 标为 `:773-956`，但 `baidu_login.rs` 共 1092 行，区间终点需复核（本轮未逐行核验）。
- **N5** §2.2.9（plan:302-305）的「命中数」口径是**行命中**（314 / 162 / 155），而 §6.1（plan:1110）用「**条**」数设单测门槛（T1 ≥45 / T2 ≥20 / T3 ≥25）；实测模板的 **test 函数数**为 api 98 / auth 49 / provider 59（合计 206），按「条」设门槛过松，建议改为按 test 函数数对齐（api ≥60 / auth ≥25 / provider ≥40）。
- **N6** §6.2 门禁（plan:1121）漏了 `verify:sandbox`：`package.json` 的 `scripts.check` 实为 `run-p ts lint:check test:unit:run verify-changelog verify:sandbox`。
- **N7** §6.4（plan:1149-1155）称 `remote-credentials.json` 是「**新增**风险点」，但该文件**已存在**（`src-tauri/src/remote/credentials.rs:25` `REMOTE_CREDENTIALS_FILE = "remote-credentials.json"`），现存的是 FTP 类凭据。措辞应改为「既有文件**新增承载**一类账号级 cookie 的凭据」，并把「验收后确认 `remote-credentials.json` 里没有夸克 cookie 明文」补进验收清单（现清单只查了 `user-remote-connections.json`，plan:1154）。
- **N8** §6.4 未提及 `device_model` 会带出本机主机名（M1），建议补一行「请求参数不落盘、不进日志」。（**推断**：§6.4 现有 4 条里没有此项；本轮只读到 plan:1149-1155。）
- **N9** §4.5（plan:712-719）把登录起始页写死为 `https://pan.quark.cn/`，但 §7-U3（plan:1183）自述「`scanLoginPage` 是**扫码页**、`pan.quark.cn` 是网盘首页；客户端真正走哪个入口**未定**」。§4.5 应改为「以 U3 的 T0 结论为准」，避免把未定项写死成实现细节。
- **N10** 方案在别处把「另一处同款**列表**调用」当作列表语义证据；复核发现该处（offset ≈5339978）实际 endpoint 是 `/1/clouddrive/share/sharepage/detail`（分享页详情），**不是**根目录列表。表述应为「另一处**同款响应形状**（`{list, metadata}`）的调用」。
- **N11** §3.2.3 结尾（plan:484）「没有 `sign` / `x-*` / `sec-*` 签名参数」表述过宽：`index.js` @6031405 处**确有** `X-U-Content-Encoding: wg` + body 加密，只是**不在** `file/sort` 路径上。建议改成「**`file/sort` 路径上**没有签名参数」。
- **N12** §4.4 前端清单（plan:685-700）漏了 `connections-section.vue` 与 `remote/index.vue`（与 B1 同源，但清单本身也要补）。
- **N13** §2.2.7 还提到 `remote-entries-section.vue`（4 处命中）、`tab/tab.vue`、`nav-sidebar.vue`（各 1-2 处）—— 本轮**未核验**这三处的命中数与是否需要改；它们不在 §4.4 的改动清单里，若实际需要改则清单不完整。
- **N14** §3.2.4（plan:486-499）对错误码表的收录范围小于 A 层实际内容（第二份副本多出 3 个码），详见 M7。

## T0 实测方案复核

### 方案原文的可取之处

- 把认证可行性放在 T0、要求「先做」（plan:859）——方向正确。
- 约束「只读、不分享、不删除、凭据临时文件阶段末销毁」（plan:883）——正确且必要。
- 闸门写明「探针模块不得引入 `tauri::` 到被测链路」（plan:881）——正确，但工具形态建议改（M3）。
- 8 个待答问题（plan:865-874）覆盖了来源/参数/fr/cookie 集合/单位/上限/风控/昵称接口，与 U1–U14 基本对应——**问题清单本身是合格的**。

### 复核结论：T0 需要重写，不能照现文执行

| 维度 | 方案现状 | 复核意见 |
|---|---|---|
| 凭据来源 | 未写（plan:861） | **不可执行** → 写死 Edge DevTools 取 Cookie 头（B2） |
| 通过判据 | 只有「二选一，不许模糊」（plan:878） | **不可判定** → 量化：HTTP 200 + `code == 0` + list 自洽 + 连续 3 次 + 子目录二次列举（B3） |
| 请求完整性 | §4.6 规范缺 `device_model` / `ut`（plan:757-765） | **会把缺参误判成被拒** → 先补规范再测（M1） |
| 失败归因 | 只区分「接受 / 拒绝」（plan:867） | **缺第三态** → 5xx / 超时 / 429 / 验证码 / cookie 不全 归「证据不足」，不得判路线不成立（B3） |
| IP 维度 | 无 | **漏记** → 新增 R17（M2） |
| 工具形态 | 允许写 `quark_probe.rs` 进产品树（plan:861、plan:678） | **不建议** → T0 用外部 curl / `Invoke-WebRequest`（M3） |
| 闸门 | `cargo test --lib` 通过（plan:881） | **错配** → 改成「8 问逐条有结论 + 三态之一 + 临时凭据已销毁」（M3） |

### 本机已取得的无凭据基线（可作 T0 对照锚点，均为只读）

| 探针 | 结果 | 含义 |
|---|---|---|
| `GET https://drive-pc.quark.cn/1/clouddrive/file/sort?pdir_fid=0&_page=1&_size=1&_fetch_total=1&pr=ucpro&fr=win&sys=win32&ve=7.4.5.1027&uc_param_str=dsdnfrpfbivesscpgimibtbmnijblauputogpintnwktprchmt`（无 Cookie） | **HTTP 401** | 请求**到达了业务层**（不是 WAF/边缘拦截）；说明该端点与参数组合本身是通的 |
| `GET https://pan.quark.cn/account/info?fr=pc&platform=pc`（无 Cookie） | **HTTP 200** | 路由存在、端点可用（U12 的候选端点成立） |

> 上述基线是本轮**无凭据**实测，只证明「请求到达服务端并被拒」与「路由存在」，**不构成**对 U4 / U12 的结论。

### 建议的 T0 执行清单（可直接抄进方案）

1. Edge 登录 `https://pan.quark.cn/`；DevTools 复制 `drive-pc.quark.cn` 请求的完整 `Cookie` 头 → 存入 `.superpowers/sdd/quark/t0-cookie.txt`（**不入 git**）。
2. 请求 A（最小集）：只带 §4.6 现有参数 + Cookie ⇒ 记录 HTTP 状态与 `code`。
3. 请求 B（补 `device_model` + `ut`）⇒ 同上。
4. 请求 C（`fr=pc`）与请求 D（`fr=win`）⇒ 同上。
5. 请求 E（`_size=1000`）⇒ 记录实际返回条数（U9）。
6. 请求 F（`/account/info` 带 Cookie）⇒ 记录昵称字段（U12）。
7. 用列表中任一子目录 `pdir_fid` 再列举一次（自洽性）。
8. 连续重复步骤 2 / 3 的成功请求 3 次。
9. 记录：出口 IP、是否弹验证码、响应头中的风控标记。
10. 按 B3 的三态表下结论；**销毁** `t0-cookie.txt`，并在记录里写明已销毁。


## U1–U14 未定项处置

| 编号 | 主题 | 必须现在拍板？ | 建议默认值 | 依据 |
|---|---|---|---|---|
| U1 | 会话 cookie 名是否为 `__pus` | 否（T0 验） | **不写死**为唯一必需；实现上按「整罐 cookie 存、按名索引」设计 | A 层本地 renderer 从不按名字取 cookie（plan:567）；B 层 hunhepan 原文称 `__pus` 单个即可 |
| U2 | 服务端是否存在 `refresh_token` | 否 | v1 不实现，仅在 §7 留档 | 14 个 bundle 全量搜索 `refresh_token` / `refreshToken` **0 命中**（产物）；`plan:1184` |
| U3 | 登录页精确 URL | **是（低成本）** | T0 一并抓包确定；在此之前 §4.5 写「以 T0 结论为准」，**不写死** `pan.quark.cn/` | `plan:1183`；配置块里 `scanLoginPage = https://su.quark.cn/4_eMHBJ`（产物 `out-config-block.txt`） |
| U4 | 服务端是否接受非浏览器来源 Cookie 请求 | **是** | 这是 R1，必须 T0 先答；**不得**因缺 `device_model` / `ut` 就判否 | `plan:1177`；M1（真实客户端确实带这两个参数） |
| U5 | `fr=win` vs `fr=pc` | 否（T0 A/B） | 默认 `fr=win`（与真实客户端一致），保 `fr=pc` 作为对照 | `plan:1179`；模块 780721 `ucparams.fr` |
| U6 | `ut` / `QUARK_WSGUID` 是否必需 | 否（T0） | T0 必测；**同时**把 `device_model` 一起测（别只测 `ut`） | `plan:1178`；M1（两者由同一拦截器注入） |
| U7 | 是否有风控 / 验证码 / 频率限制 | 否（T0 + T8） | 不预判；错误映射里保留「风控 / 协议变更」可辨识文案 | `plan:1185`；R2（plan:837） |
| U8 | `size` / `updated_at` 单位 | 否（T0 核对） | `size` 暂按字节、`updated_at` 暂按秒，但**必须**在代码里保留显式换算函数（照抄 `baidu_seconds_to_unix_millis`） | `plan:1181`；`baidu_api.rs` 同款函数存在 |
| U9 | `_size` 服务端上限 | 否（T0） | 常量取 100（与客户端一致），并**必须**用 `clamp_page_limit` 夹住 | `plan:1182`；客户端 `_size = 100`（产物 @5312702） |
| U10 | 下载是否需额外 header | 否（T0 / T8） | 默认只带 Cookie；补 `Referer: https://pan.quark.cn/` 作为兜底 | `plan:1186`；客户端只显式注入 Cookie |
| U11 | cookie 是否须覆盖整个 `.quark.cn` | 否（T0 分组） | 先存整罐（简单且安全），再在 T0 里裁剪到最小集 | `plan:1187`；B 层称 `__pus` 单个就够 |
| U12 | `pan.quark.cn/account/info` 登录态下是否稳定返回昵称 | 否（T0） | **端点已证实存在**（无凭据探针 200），但昵称字段形状仍待验；UI 需有「昵称未知」的降级路径 | `plan:1188`；本机只读探针 HTTP 200 |
| U13 | `_sort` 参数格式的确切取值 | 否（T0） | 默认 `file_name:asc,updated_at:desc`（方案已给），但**排序字段取值需以实测为准**，不得当成既定契约 | `plan:1189`；B 层给出三种示例 |
| U14 | 是否有「根目录以外的并列入口」（分享 / 我的资源） | 否（T8） | v1 **不伪装成目录**（与百度一致），保持「网盘」单根语义 | `plan:1190` |

> 结论：**14 项里只有 U4 必须现在拍板**（它决定整条路线是否成立），且 U4 的答案只能由 T0 给出；U3 可低成本在 T0 一并定。方案把 U1–U14 全部推给 T0 的安排是合理的，缺的是「拍板时用什么判据」（B3）。

## 事实性核验表

### A. 后端 · 数据模型与协议注册

| 方案中的主张 | 仓库中的真实情况 | 文件:行号 | 结论 |
|---|---|---|---|
| `connection.rs:19-33` `enum RemoteProtocol` 含 BaiduPan | 一致 | `src-tauri/src/remote/connection.rs:19-33` | 成立 |
| `connection.rs:37-44` `default_port()`，BaiduPan → 443 在 `:42` | 一致 | `remote/connection.rs:37-44`、`:42` | 成立 |
| `connection.rs:46-54` `as_str()`，BaiduPan → `"baidu_pan"` 在 `:52` | 一致 | `remote/connection.rs:46-54`、`:52` | 成立 |
| `connection.rs:57-60` `struct RemoteConnection`（camelCase） | 一致 | `remote/connection.rs:57-60` | 成立 |
| `mod.rs:20-39` `mod` 声明块（baidu 三模块在 `:20-22`） | 一致 | `remote/mod.rs:20-39` | 成立 |
| `mod.rs:41-68` 是「三段 `pub use baidu_*`」 | 实为 **4 段**（`:41` / `:53` / `:58` / `:65`） | `remote/mod.rs:41,53,58,65` | **不成立** |
| `mod.rs:8-14` `cargo test --lib` 红线注释 | 一致 | `remote/mod.rs:8-14` | 成立 |
| `registry.rs:119-127` BaiduPan 分支（`password: None`） | 一致（`:119-128`） | `remote/registry.rs:119-128` | 成立 |
| `error.rs:17-46` `RemoteErrorKind` 共 **11** 个 | 一致 | `remote/error.rs:17-46` | 成立 |

### B. 后端 · 纯逻辑层模板

| 方案中的主张 | 仓库中的真实情况 | 文件:行号 | 结论 |
|---|---|---|---|
| `baidu_api.rs` 2424 行 | 一致 | `remote/baidu_api.rs`（2424 行） | 成立 |
| `baidu_api.rs:47` `BAIDU_PAN_HOST` | 一致 | `remote/baidu_api.rs:47` | 成立 |
| `baidu_api.rs:50` `BAIDU_PAN_BASE_URL` | 一致 | `remote/baidu_api.rs:50` | 成立 |
| `baidu_api.rs:58` `BAIDU_PAN_APP_ID`（注释说它不是 OAuth client_id） | 一致 | `remote/baidu_api.rs:58` | 成立 |
| `baidu_api.rs:65` `BAIDU_PAN_USER_AGENT` | 一致 | `remote/baidu_api.rs:65` | 成立 |
| `baidu_api.rs:70` `BAIDU_LIST_PAGE_LIMIT = 1000` | 一致 | `remote/baidu_api.rs:70` | 成立 |
| `baidu_api.rs:73 / 78 / 84` 三级超时 | 一致 | `remote/baidu_api.rs:73,78,84` | 成立 |
| `baidu_api.rs:87 / 90` 两个 cookie 名常量 | 一致 | `remote/baidu_api.rs:87,90` | 成立 |
| `baidu_api.rs:106` `struct BaiduSession`（手写 Debug 只打印长度） | 一致 | `remote/baidu_api.rs:106` | 成立 |
| `baidu_api.rs:174` `sanitize_cookie_value()`（判非法而非过滤） | 一致 | `remote/baidu_api.rs:174` | 成立 |
| `baidu_api.rs:208 / 222 / 233` 三个 URL 构造函数 | 一致 | `remote/baidu_api.rs:208,222,233` | 成立 |
| `baidu_api.rs:283` `clamp_page_limit()`（服务端静默截断） | 一致 | `remote/baidu_api.rs:283` | 成立 |
| `baidu_api.rs:297` `next_page()` | 一致 | `remote/baidu_api.rs:297` | 成立 |
| `baidu_api.rs:318` `struct BaiduFileEntry` | 一致 | `remote/baidu_api.rs:318` | 成立 |
| `baidu_api.rs:400 / 413 / 431` 三个解析器 | 一致 | `remote/baidu_api.rs:400,413,431` | 成立 |
| `baidu_api.rs:562` `enum BaiduFailure` | 一致 | `remote/baidu_api.rs:562` | 成立 |
| `baidu_api.rs:589` `classify_baidu_failure()` | 一致 | `remote/baidu_api.rs:589` | 成立 |
| `baidu_api.rs:617` `failure_from_errno()` | 一致 | `remote/baidu_api.rs:617` | 成立 |
| `baidu_api.rs:644` `failure_from_http_status()` | 实为 **`:647`** | `remote/baidu_api.rs:647` | **不成立** |
| `baidu_api.rs:611` 被引用为映射表依据 | 该行内容与「穷尽映射表」不完全对应 | `remote/baidu_api.rs:611` | 部分成立 |
| `baidu_auth.rs` `BAIDU_SESSION_VERSION = 1` @ `:49` | 一致 | `remote/baidu_auth.rs:49` | 成立 |
| `baidu_auth.rs` 过期告警秒 @ `:54` | 一致 | `remote/baidu_auth.rs:54` | 成立 |
| `baidu_auth.rs:122-167` `scan_cookies()` | 一致 | `remote/baidu_auth.rs:122-167` | 成立 |
| `baidu_auth.rs` 还有 `session_to_cookie_header` | **方案未提及**该函数，而 `commands.rs:282` 正在用它 | `remote/baidu_auth.rs:436`、`remote/commands.rs:282` | **不成立（漏项）** |
| `baidu_auth.rs` 文件 1013 行、模块头注释 `21-31` | 行数一致；`21-31` 区间内容本轮未逐字核验 | `remote/baidu_auth.rs`（1013 行） | 部分成立 |

### C. 后端 · provider 层

| 方案中的主张 | 仓库中的真实情况 | 文件:行号 | 结论 |
|---|---|---|---|
| `baidu_provider.rs` 2306 行 | 一致 | `remote/baidu_provider.rs`（2306 行） | 成立 |
| `:54` `BAIDU_DIRECTORY_TIMEOUT = 120s` | 一致 | `remote/baidu_provider.rs:54` | 成立 |
| `:60` `BAIDU_FILE_TIMEOUT = 900s` | 一致 | `remote/baidu_provider.rs:60` | 成立 |
| `:67` `BAIDU_MAX_LIST_PAGES = 200` | 一致 | `remote/baidu_provider.rs:67` | 成立 |
| `:73` `BAIDU_MAX_DOWNLOAD_BYTES = 500 MiB` | 一致 | `remote/baidu_provider.rs:73` | 成立 |
| `:75` `trait BaiduApi` | 实为 **`:77`** | `remote/baidu_provider.rs:77` | **不成立** |
| `:124` `struct UnavailableBaiduApi` | 一致 | `remote/baidu_provider.rs:124` | 成立 |
| `:178` `trait BaiduSessionStore` | 实为 **`:179`** | `remote/baidu_provider.rs:179` | **不成立** |
| `:200` `struct HttpBaiduApi` | 一致 | `remote/baidu_provider.rs:200` | 成立 |
| `:369` `struct DpapiBaiduSessionStore` | 一致 | `remote/baidu_provider.rs:369` | 成立 |
| `:410` `struct BaiduRemoteProvider` | 一致 | `remote/baidu_provider.rs:410` | 成立 |
| `:637` `impl RemoteProvider for BaiduRemoteProvider` | 一致 | `remote/baidu_provider.rs:637` | 成立 |
| `provider.rs` trait 共 **9** 个方法，行号 `40/46/58/69/97/106/118/131/147/177` | 一致 | `remote/provider.rs:40,46,58,69,97,106,118,131,147,177` | 成立 |
| 存在 `read_file_stream`（plan:37 引用为 trait 方法） | **不存在**；`:55` 注释明说刻意不提供流接口 | `remote/provider.rs:55` | **不成立** |
| 「波及 **4** 个既有 provider」 | 全仓库 `impl RemoteProvider for` 共 **5** 处 | `src-tauri/src/`（grep 计数） | **不成立** |


### D. 后端 · 凭据与命令层

| 方案中的主张 | 仓库中的真实情况 | 文件:行号 | 结论 |
|---|---|---|---|
| `credentials.rs` 的锚点 `54 / 343 / 390 / 461 / 486 / 506` | 一致 | `remote/credentials.rs:54,343,390,461,486,506` | 成立 |
| `credentials.rs:25` `REMOTE_CREDENTIALS_FILE = "remote-credentials.json"`（方案未提） | 一致 | `remote/credentials.rs:25` | 成立（方案漏项，见 N7） |
| `commands.rs:37-38 / 149 / 170 / 206 / 270-285 / 991-1004` | 一致 | `remote/commands.rs:37-38,149,170,206,270-285,991-1004` | 成立 |
| `commands.rs:358 / 379 / 586` 是「`provider_with_protocol` 的三个 BaiduPan 分支」 | 三处分支属实，但**分属三个不同函数**；`provider_with_protocol` 本体在 `:573` | `remote/commands.rs:358,379,586,573` | **部分成立** |
| `baidu_login.rs`（**996 行**，plan:182） | 实为 **1092 行** | `src-tauri/src/baidu_login.rs`（1092 行） | **不成立** |
| `baidu_login.rs:54 / 56 / 58 / 60 / 62 / 64 / 66 / 67 / 68 / 76 / 78 / 82` 一组常量 | 一致 | `remote/baidu_login.rs:54-82` | 成立 |
| `baidu_login.rs:84-95` 三道保险 | 一致 | `remote/baidu_login.rs:84-95` | 成立 |
| `baidu_login.rs:108-137` pending-session slot | 一致 | `remote/baidu_login.rs:108-137` | 成立 |
| `baidu_login.rs:347 / 430 / 448 / 485` 四个 login 命令 | 一致 | `remote/baidu_login.rs:347,430,448,485` | 成立 |
| `baidu_login.rs:536` `PROBE_LIVE_URL` | 一致 | `remote/baidu_login.rs:536` | 成立 |
| `baidu_login.rs:773-956` `poll_loop` | 起点成立，终点未核 | `remote/baidu_login.rs:773` | 部分成立 |
| `baidu_login.rs:185` `LOGIN_PROFILE_ROOT = "baidu-login-profile"` | 一致 | `remote/baidu_login.rs:185` | 成立 |
| `baidu_login.rs:196-197` `session-<stamp_ms>` | 一致 | `remote/baidu_login.rs:196-197` | 成立 |
| `lib.rs:11-12` `mod baidu_login;` / `mod baidu_probe;` | 一致 | `src-tauri/src/lib.rs:11-12` | 成立 |
| `lib.rs:377-383` `generate_handler!` 含 7 个 baidu 命令 | 一致 | `src-tauri/src/lib.rs:377-383` | 成立 |
| `lib.rs:698-703` probe install / selftest | 一致（`:699-703`） | `src-tauri/src/lib.rs:699-703` | 成立 |
| `capabilities/default.json` 已具备所需 capability，夸克不需改 | `allow-create-webview-window` / `allow-get-all-webviews` / `allow-close` / `allow-destroy` 均在；`windows` = `["main","quick-view","print-view"]` | `src-tauri/capabilities/default.json:5-9,14-15,27,30` | 成立（「不需要改」属**推断**） |

### E. 前端

| 方案中的主张 | 仓库中的真实情况 | 文件:行号 | 结论 |
|---|---|---|---|
| `types/remote.ts:14-15` `RemoteProtocol` 五个变体 | 一致 | `src/types/remote.ts:14-15` | 成立 |
| `types/remote.ts:173-174` `isBaiduPanProtocol()` | 一致（`:173`） | `src/types/remote.ts:173` | 成立 |
| `types/remote.ts:190 / 286 / 302` 三个载荷类型 | 一致 | `src/types/remote.ts:190,286,302` | 成立 |
| `utils/virtual-locations.ts:31 / 137-139` | 一致 | `src/utils/virtual-locations.ts:31,137,139` | 成立 |
| `utils/remote-location-refs.ts:12` | 一致 | `src/utils/remote-location-refs.ts:12` | 成立 |
| `types/dir-entry.ts:40` `location_group` | 一致（`LocationGroup` 在 `:46`） | `src/types/dir-entry.ts:40,46` | 成立 |
| `remote-connections.ts` 的 9 组锚点 `48-49/323-346/358/380-386/488-519/533/542/558/563` | 一致 | `src/stores/storage/remote-connections.ts`（同上） | 成立 |
| `sections.ts:8-20` 用 `isBaiduPanProtocol` 分流到 netdisk 分区 | 一致 | `src/modules/settings/ui/categories/remote/sections.ts:8-20` | 成立 |
| `remote-nav-panel.vue:11 / 34 / 38 / 63-64 / 72` 分区与过滤 | 一致 | `src/modules/nav-sidebar/components/remote-nav-panel.vue:11,34,38,63,64,72` | 成立 |
| `connection-form.vue`（**927 行**，plan:255） | 实为 **1032 行** | `connection-form.vue`（1032 行） | **不成立** |
| `connection-form.vue:26` 是「协议选项来源」（plan:261） | `:26` 是 import；协议选项在 **`:69`**，且**不含网盘** | `connection-form.vue:26,69` | **不成立** |
| 「协议下拉加项 / 下拉出现夸克网盘」（plan:690、plan:712） | `PROTOCOLS` 刻意不含网盘协议（注释 `:66-67` 明确反对）；网盘协议由 `variant === "netdisk"` 硬编码 | `connection-form.vue:63-69,230-232` | **不成立（与仓库现状矛盾）** |
| `connection-form.vue:79` `BAIDU_PAN_HOST` | 一致 | `connection-form.vue:79` | 成立 |
| `connection-form.vue:89-134` 登录状态与 start / cancel（`:95` / `:114`） | 一致 | `connection-form.vue:89-134` | 成立 |
| `connection-form.vue:139` 监听 `afm://baidu-login` | 一致 | `connection-form.vue:139` | 成立 |
| `connection-form.vue:220-255` 网盘分支隐藏 host / port / 密码 | 区间略有出入：实际硬编码在 `:216-245` | `connection-form.vue:216-245` | 部分成立 |
| `connection-form.vue:376-413` 授权状态卡 | 一致 | `connection-form.vue:376-413` | 成立 |
| `remote-tab-labels.ts`「1 处命中，按协议取名」，要加夸克（plan:279） | 全文 60 行，**不含** baidu / protocol / netdisk，只吃 `connectionName` ⇒ **不需要改** | `src/modules/tab-bar/utils/remote-tab-labels.ts:1-60` | **不成立** |
| §4.4 前端清单完整 | 漏 `connections-section.vue`（真正的网盘 / 远程分区决策点）与 `remote/index.vue` | `connections-section.vue:43-44,87,93-97`；`remote/index.vue` | **不成立** |

### F. i18n

| 方案中的主张 | 仓库中的真实情况 | 文件:行号 | 结论 |
|---|---|---|---|
| 共 **17** 个语言包 | 一致 | `src/localization/messages/`（17 个 `*.json`） | 成立 |
| 中文 locale code 是 `ch` 不是 `zh` | 一致（无 `zh.json`） | `src/localization/messages/ch.json` | 成立 |
| `ch.json:655` `protocols.baidu_pan` | 一致 | `ch.json:655` | 成立 |
| `ch.json:681` `baiduLoginRequired` | 一致 | `ch.json:681` | 成立 |
| `ch.json:723` `baidu`（对象） | 一致 | `ch.json:723` | 成立 |
| `ch.json:746` `baidu.saved` | 一致 | `ch.json:746` | 成立 |
| 每个语言包「4 处」，夸克照加 4 处 | 4 处里 1 处是**嵌套对象**，实际约 **15** 个叶子串（含 `stoken_expired` / `stoken_expiring_soon` 等百度专属措辞） | `ch.json:723-748` | **部分成立** |

### G. A 层实证（产物 `asar/dist/renderer/index.js`，按字符偏移复核）

| 方案中的主张 | 产物中的真实情况 | 位置 | 结论 |
|---|---|---|---|
| `GET {cloudDriveUrl}/1/clouddrive/file/sort`，参数 `pdir_fid/_page/_size/_fetch_total/_fetch_sub_dirs/fetch_dir_file_num/_sort` | 一致，默认 `_size = 100`，`_sort: s.join(",")` | offset ≈5312702 | 成立 |
| 列表响应形状 `{list, metadata}` | 一致 | offset ≈5339978 | 成立 |
| 「另一处同款**列表**调用」 | 该处 endpoint 实为 `/1/clouddrive/share/sharepage/detail`（分享页详情） | offset ≈5339978 | **部分成立** |
| `driveRequest` 实例级 params `{pr,fr,sys,ve,uc_param_str}`、`timeout 1e4`、`withCredentials` | 一致 | offset ≈5313953（模块 781615） | 成立 |
| `driveRequest` 无 request 拦截器（plan:456-462 未列） | 该实例**确有** request 拦截器，注入 `uc_param_str` / `device_model` / `getUcParams()` | offset ≈5313953 | **不成立（漏记）** |
| `fileRequest` 实例级 params `{...driveBaseParams}` + request 拦截器 | 一致 | offset ≈6047616 | 成立 |
| `fileRequest` response 拦截器 `code !== 0 ⇒ reject` | 一致 | offset ≈6047616 | 成立 |
| `ucparams.fr = darwin ? "mac" : "win"`、`pr = "ucpro"` | 一致 | 模块 780721 | 成立 |
| `UC_PARAMS_STR` 固定串 | 一致 | 模块 860782 | 成立 |
| `getUcParams()` → `ut` ← `QUARK_WSGUID`、`ve` ← `QUARK_VERSION` | 一致 | 模块 514962 | 成立 |
| `getQuarkPCVersion()` 从 UA 匹配 `QuarkPC/(\d+\.\d+\.\d+\.\d+)` | 一致 | 模块 407716 | 成立 |
| 「没有 `sign` / `x-*` / `sec-*` 签名参数」（plan:484） | `file/sort` 路径上成立；但 `index.js` @6031405 另有 `X-U-Content-Encoding: wg` + body 加密（**不在**该路径上） | @6031405 | 部分成立（表述过宽） |
| 错误码表（模块 887411 @6041787） | 逐条成立；另有第二份副本 @6030862 **多出** `23018 / 25008 / 38001` | 模块 887411、@6041787、@6030862 | 部分成立 |
| `31001` 是会话失效码 | 成立；且客户端 `if ([31001, 31004].includes(r.code) && ...user.logout())` ⇒ **`31004` 同为会话失效码** | 模块 781615 | 部分成立（漏 `31004`） |
| `24005` = 文件 / 文件夹状态已更新 | 原文「文件/文件夹状态已更新，请返回重试」；**不**含「隐私空间锁定」 | @6041787、@6030862 | 部分成立 |
| 下载链接过期判定 `Date.now() >= 1000 * (exp - 1800)` | 逐字成立（`auth_key=` / `Expires=`） | offset ≈235989 | 成立 |
| 本地 bundle 无 `refresh_token` | 14 个 bundle 全量搜索 `refresh_token` / `refreshToken` **0 命中** | 产物全量 grep | 成立 |
| 本地 bundle 无 `__pus` / `__puus` | **0 命中**（`out-pus.txt` 为 0 字节） | 产物全量 grep | 成立 |
| 配置块：`cookieHost:"quark.cn"`、`cloudDriveUrl:"https://drive-pc.quark.cn"`、`client_id:"533"`、`secureNo:14100`、`scanLoginPage:"https://su.quark.cn/4_eMHBJ"`、`uopAccessSalt`、`utoken`、`openApiUrl` | 全部一致 | `out-config-block.txt` | 成立 |

### H. B 层与其它

| 方案中的主张 | 真实情况 | 证据 | 结论 |
|---|---|---|---|
| B 层称「只需 `__pus`」 | 原文成立 | `https://docs.hunhepan.com/reman/pantools.html` | 成立 |
| 失败响应 `{"status":401,"code":31001,"message":"require login [guest]","req_id":"971212","timestamp":1728791773}` | 原文成立 | 同上 | 成立 |
| B 层工具是浏览器来源 ⇒ 非浏览器直连风险「中高」 | 该工具是**原生 Windows（Go）程序**单 cookie 直连 ⇒ 构成反例 | 同上 | **不成立（R1 概率应下调）** |
| 国外服务器访问夸克 API 会被拦截 | 原文「2024/07/24 晚起貌似国外服务器访问夸克 API 时会被拦截」⇒ 存在 IP 维度独立风险，方案未记 | 同上 | 成立（方案漏项） |
| §2.2.10 命中清单 96 文件 / 2189 行 | 扫描器自报 `scanned 1937 files; 96 files; 2189 matching lines` | `.superpowers/sdd/quark/baidu-touchpoints.txt:3` | 成立 |
| §2.2.9 列出的测试文件均存在 | 全部存在 | `src-tauri/src/remote/connection_test.rs` 等 | 成立 |
| `connection_test.rs` 命中 14 | 实含 **24** 个 test 函数 | `src-tauri/src/remote/connection_test.rs` | 部分成立 |
| §6.2 门禁 = `npm run check` | `package.json` 的 `scripts.check` 实为 `run-p ts lint:check test:unit:run verify-changelog verify:sandbox` ⇒ **漏 `verify:sandbox`** | `package.json`（`scripts.check`） | **不成立** |
| `AGENTS.md:24/29/68/116/117/119/120` 的引用与方案一致 | 一致 | `AGENTS.md:24,29,68,116-120` | 成立 |
| §6.3「`tauri dev` 的界面验收一律作废」 | 与仓库铁律一致 | `AGENTS.md:116-120` | 成立 |

## 工作量复核

### 方案估算 vs 复核

| 阶段 | 方案(人天) | 复核(人天) | 差异依据 |
|---|---:|---:|---|
| T0 实测 | 1.0 | **1.0–1.5** | 需补 cookie 获取（B2）、量化判据（B3）、参数补全（M1）、IP 维度记录（M2） |
| T1 API 层 | 1.5 | **2.0–2.5** | 模板 `baidu_api.rs` 生产 1044 行 + 98 个 test fn；方案要 ≥45 条单测且需覆盖 11 种 kind 的穷尽映射（M7 还要再补 4 个码） |
| T2 会话层 | 0.5 | **1.0–1.5** | 模板 `baidu_auth.rs` 生产 488 行 + 49 个 test fn；方案漏了 `session_to_cookie_header`（`baidu_auth.rs:436`）这类必备的会话 → cookie 头转换 |
| T3 provider | 1.0 | **1.5–2.0** | 模板 `baidu_provider.rs` 生产 790 行 + 59 个 test fn；且需处理夸克特有的分页 / `_fetch_total` 交叉校验（R4） |
| T4 登录窗 | 1.0 | **1.0–1.5** | 模板 `baidu_login.rs` 957 生产行；夸克入口 URL 本身是 U3 未定项，可能要返工 |
| T5 命令接线 | 0.5 | **0.5–1.0** | `commands.rs` 现有 1065 行要加 3 + 4 命令 + 3 处分支 + 注册自检；方案称 `+60` 行属乐观 |
| T6 前端 | 1.0 | **2.0–2.5** | 因 B1 要改的不止一个文件：网盘分区决策点 `connections-section.vue` + `remote/index.vue` + `sections.ts` + `remote-nav-panel.vue` 的 `sections` 结构 + 表单分支；若走 (a) 方案还要改 provider 选择器 |
| T7 i18n | 0.5 | **1.0–1.5** | 17 语言 × 约 15 键 ≈ 250+ 条串，含 16 种非中文语言的人工翻译（M4） |
| T8 真机 | 1.0 | **1.0–1.5** | `tauri build` + 装机 + 多账号 / 多网盘分区 / 失效重授权等场景 |
| **合计** | **8.0** | **11.5–12.5** | 偏乐观 **40–55%** |

**若 T0 判定走退路（§7.1）**：再加 **1.5–2.0 人天**（退路是「在 WebView2 里跑 fetch、把 JSON 回传 Rust」，需要新的数据通道与前端脚本），合计 **13.0–14.5 人天**。

### 复核依据：模板体量实测

| 模板文件 | 总行数 | 生产代码 | 测试代码 | test 函数数 |
|---|---:|---:|---:|---:|
| `remote/baidu_api.rs` | 2424 | 1044 | 1380 | **98** |
| `remote/baidu_auth.rs` | 1013 | 488 | 525 | **49** |
| `remote/baidu_provider.rs` | 2306 | 790 | 1516 | **59** |
| `baidu_login.rs` | 1092 | 957 | 135 | 11 |
| **合计** | **6835** | **3279** | **3556** | **217** |

> 说明：方案 §4.3 给夸克新文件的规模是 `quark_api.rs` ≈600 / `quark_auth.rs` ≈350 / `quark_provider.rs` ≈700 / `quark_login.rs` ≈700 行；对照模板的**生产代码**行数（1044 / 488 / 790 / 957），方案给的是纯生产口径，与模板本身不冲突，但**没有把测试体量算进去**——按模板「测试 : 生产 ≈ 1 : 1」（3556 : 3279），方案的总工期必须把测试算进去，否则 T1–T3 的「零网络单测」根本写不完。

### 门禁与验收的复核

- §6.1（plan:1110）「`cargo test --lib` 新增 ≥ 90 条」：按 **test 函数**口径，模板 api / auth / provider 合计 **206** 个 ⇒ 建议改为「api ≥60 / auth ≥25 / provider ≥40，合计 ≥125」（见 N5）。
- §6.2（plan:1121）漏 `verify:sandbox`（见 N6）。
- §6.4（plan:1149-1155）建议补两条：`remote-credentials.json` 内无夸克 cookie 明文；`device_model` 不落盘 / 不进日志（N7、N8）。

## 评审边界与未验证项

**本轮做过（全部只读）**：通读方案全文（4 段，1266 行）；逐条核对 §2.2 的仓库锚点（Rust 后端约 60 处行号、TS / Vue 前端约 40 处行号）；按字符偏移复核 A 层 bundle 的端点、参数、拦截器、错误码表、过期判定；复核 B 层页面原文；2 次无凭据只读网络探针（401 / 200）。

**本轮未做（因此下列结论未验证）**：

1. **夸克服务端在登录态下的真实行为**——U1–U14 的答案只能由 T0 给出；本轮**未持有任何凭据**，未做任何带凭据请求。
2. **`cargo test --lib` / `npm run check` / `npm run test:unit:run` / `tauri build` 均未执行**（本机另有会话在做真机验收），故「现有测试是否全绿」未复核。
3. **B 层 `夸克网盘.py` 源码未读到**（本次 fetch 超时），§3.3 中涉及该文件的具体行号未逐一核对；§3.3 的结论仅基于已读到的 `pantools.html` 原文。
4. **`baidu_login.rs:773-956`（`poll_loop`）的区间终点**未逐行核验。
5. **`capabilities/default.json`「不需要改」**属**推断**（依据是百度同构路径 + capability 已存在），未做真机建窗验证。
6. **§2.2.7 中 `remote-entries-section.vue`（4 处）、`tab/tab.vue`、`nav-sidebar.vue`（各 1-2 处）的命中数与是否需改**，以及 §2.2.9 的「命中数」口径（行命中 vs test 函数）——部分未核。
7. **§6.3 / §6.5 的真机验收条目**只做了文本层面的一致性比对（对照 `AGENTS.md:116-120`），未做实机验证。
8. **`quark_probe.rs` 的必要性**（M3）属设计判断，未做实现级评估。

**边界声明**：本轮**未修改**仓库任何源码 / 配置 / 文档；仅新建本报告文件（`.superpowers/sdd/quark/plan-review-2026-11-11.md`，该目录已 gitignore）；**未触碰** `%APPDATA%\com.sigma-file-manager.app\user-data\`；**未执行**任何重型构建命令；网络访问仅限 2 次无凭据只读 GET 与 1 次公开文档 GET。
