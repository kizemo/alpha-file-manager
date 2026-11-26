# 批次 38 · AFM 云盘接入 OneDrive：研究与实施方案

- 日期：2026-11-22
- 状态：**研究阶段完成，尚未写任何产品代码**（工作区基线 `27110478`，批次 37 收口 commit）
- 证据目录：`.superpowers/sdd/b38/recon-code.md`（代码落点，383 行）、`.superpowers/sdd/b38/recon-graph-api.md`（Graph 硬约束，275 行，58 处 `learn.microsoft.com` 引用）
- 本文所有 `file:line` 为侦察时快照；实施前按符号名 grep 再核一次（符号名在本文与两份侦察报告中都已写全）

---

## 0. 一句话结论

**技术路线上没有不可解的硬障碍，但 OneDrive 和 AFM 现有三家网盘不是同一种架构**：现有三家（百度 Cookie、夸克 Cookie、阿里云内嵌登录窗 + 扫码）都是「在应用内取一个会话 blob」，而 OneDrive 的标准原生路径是「**开系统浏览器 + 本地回环收授权码 + PKCE**」。因此本批次真实的成本重心是：

1. **一套 AFM 目前完全没有的机制**：`tokio` 本地回环 HTTP 监听 + PKCE + state 校验 + 回调解析（现有代码里零先例）；
2. **一个只能由人提供的凭据**：Azure 应用注册的 `client_id`（我不能代你在门户里注册）。

Provider 层本身反而最轻——`credentials.rs` 一行不用改，`transfer` / `thumbnails` / `remote_index` / `dir_adapter` 全都不用动。

---

## 1. 现有三家网盘的接入形态（模板盘点）

| 维度 | 百度网盘 | 夸克网盘 | 阿里云盘 |
|---|---|---|---|
| 认证载体 | Cookie（`BDUSS` 必需 + `STOKEN`） | Cookie（`__pus` 必需 + `__puus` 下载必需） | OAuth 令牌包 |
| 获取方式 | 内嵌窗读 cookie | 内嵌窗读 cookie | 内嵌登录窗 + 扫码 + 粘贴 token |
| 会话结构 | `remote/baidu_auth.rs:49`（v1） | `remote/quark_auth.rs:81/:92` | `remote/aliyun_auth.rs:66-94`（v3） |
| 登录窗模块 | `src-tauri/src/baidu_login.rs` | `src-tauri/src/quark_login.rs` | `src-tauri/src/aliyun_login.rs` |
| 独立能力文件 | 无（只读 cookie） | 无（只读 cookie） | **有** `capabilities/aliyun-login.json` |
| 前端入口 | `netdisk-add-dialog.vue`（快捷弹窗） | 同左 | `connections-section.vue:366` `startAddAliyunNetdisk`（走完整表单） |
| 授权卡 | 弹窗内 | 弹窗内 | `connection-form.vue:246-545` 整块 |

**推论：OneDrive 属于「阿里云那一类」**——OAuth、需要独立授权卡、需要走完整表单而不是快捷弹窗。所以：

- ✅ 照抄 `aliyun_auth.rs` 的**会话结构与存取范式**（几乎是 1:1 对应：`access_token / refresh_token / expires_at`）；
- ✅ 照抄 `connection-form.vue:246-545` 的**授权卡形态**；
- ❌ **不能**照抄 `aliyun_login.rs` 的**机制**（见 §3）；
- ❌ `netdisk-add-dialog.vue` 那套（`baidu_pan | quark_pan` 局部硬编码、主机三元式、两条事件订阅）与本批次**无关**，不用去改它的漏点。

---

## 2. 为什么 OneDrive 的落点仍然很轻

`RemoteProvider` 是唯一扩展点，且调用方全部按 `&dyn` 编排：

- trait：`src-tauri/src/remote/provider.rs:50`，9 个方法**无默认实现**（`:56/:68/:79/:107/:116/:128/:141/:157/:187`），唯一有默认实现的是 `thumbnail_bytes`（`:213-221` → `Ok(None)`）。
- 错误模型 `RemoteErrorKind` 11 类（`remote/error.rs:15-47`），网盘不用 `HostKey*`。
- **不需要动的文件**：`provider.rs`、`error.rs`、`dir_adapter.rs`、`credentials.rs`、`transfer.rs`、`thumbnails.rs`、`global_search/remote_index.rs`、`path.rs`、`fake.rs`、`src-shared/remote-connection.sample.json`、`settings/.../remote/sections.ts`。
- **缩略图可以不做**：全仓 `async fn thumbnail_bytes` 只命中 `remote/aliyun_provider.rs:1769` 与 trait 本体，百度/夸克都没实现 ⇒ 降级占位图不算功能缺口。
- **权限位本来就不支持**：三家网盘 `set_permissions` 一律 `Unsupported`（`baidu_provider.rs:732` / `quark_provider.rs:1002` / `aliyun_provider.rs:1949`），现成 helper `transfer::unsupported_set_permissions`（导出 `remote/mod.rs:231`）。

⚠️ **「最小 provider」不等于「少写方法」**：9 个方法在编译期就逼你全表态。最小实现的做法是把暂不支持的能力**显式返回 `RemoteErrorKind::Unsupported`**——写 `Ok(())` 是数据事故，写 `todo!()` 是运行期 panic。

---

## 3. 认证：三方案与推荐（本批次最大的架构决策）

### 3.1 决策表（摘自 `recon-graph-api.md` §D）

| 维度 | ① 设备码流 | ② 系统浏览器 + 授权码 + PKCE + `http://localhost` | ③ 内嵌 WebView |
|---|---|---|---|
| 需注册 redirect URI | **不需要** | 需 `http://localhost`（端口被忽略） | 需遗留 `.../common/oauth2/nativeclient` |
| 需开「允许公共客户端流」 | **是**（默认关闭） | 否 | 否 |
| MSA（个人账户）体验 | **要再登录一次**（官方明文） | 一次登录 | 一次登录 |
| MFA / 条件访问 | 真浏览器，隔离干净 | 真浏览器，隔离干净 | 可能被 CA / cookie 干扰 |
| 时效 | 码 **15 分钟** | 码 **约 1 分钟** | 约 1 分钟 |
| 实现复杂度 | 低（无监听、无回调解析） | 中（本地监听 + state + PKCE + 回调） | 中高（拦 URL，登录页会变） |
| 官方定位 | 公开客户端合法路径 | **原生/桌面推荐路径** | 仅「embedded browsers 遗留值」 |
| 结论 | **回退方案** | **首选** | 不推荐 |

### 3.2 推荐：② 为主，① 作为备用分支

理由（每条都有官方依据，见 `recon-graph-api.md`）：

1. 官方给原生应用的推荐就是「系统浏览器 + `http://localhost`」，PKCE「now recommended for all application types」。
2. `localhost` 的**端口在匹配时被忽略**（RFC 8252 §7.3），所以可以随绑定随机端口，无需在 Azure 门户登记具体端口；且**不要**注册多个仅端口不同的 URI（服务器会任选，可能选中你没监听的那个）。
3. **设备码流在个人账户上是体验硬伤**：MSA 走 `/common` 或 `/consumers` 会被要求「再登录一次」（设备侧无 cookie），且**没有 `verification_uri_complete`**（官方明文 "not included or supported at this time"）⇒ 用户必须手抄 `user_code`，码 15 分钟有效。OneDrive 用户绝大多数是个人账户，直接把最差路径给主力用户。
4. **不要用内嵌 WebView**：`nativeclient` 重定向是给 embedded browsers 的遗留值，登录页可能随时变；而系统浏览器方案让 MFA/CA 都在真浏览器完成，隔离干净。

### 3.3 本仓库可行性已核实

| 需要的能力 | 现状 | 结论 |
|---|---|---|
| 打开系统浏览器 | `tauri-plugin-opener = "=2.5.4"`（`src-tauri/Cargo.toml:34`） | ✅ 现成 |
| 本地 TCP 监听 | `tokio` 已开 `net` feature（`Cargo.toml:54`） | ✅ 现成 |
| HTTP 客户端 | `reqwest 0.13` + rustls（`Cargo.toml:64`） | ✅ 现成 |
| SHA-256（PKCE） | 需确认是否已有依赖（`sha2` 未在依赖段命中） | ⚠️ 若无则加一个纯 Rust 小依赖 |

### 3.4 与现有实现的关键差异（别照抄）

`aliyun_login.rs` 是 **WebviewWindow 内嵌登录**路线（`remote_aliyun_login_start` `:1160` / `..._observed` `:1190` / `..._close` `:1295`，窗口 label `afm-aliyun-login` `:669`，事件频道 `afm://aliyun-login` `:672`），靠前端轮询 + 页面回调 `invoke`。

OneDrive 走回环则是**纯 Rust 侧**：建 `TcpListener` → 拼授权 URL → `opener` 开系统浏览器 → 在同一进程里 accept 回调 → 校验 `state` → 换 token。**因此不需要 `capabilities/onedrive-login.json`**（那个文件存在的前提是「登录页要 `invoke` 应用命令」），也就不需要新的 window label 与新的事件频道——**这是 OneDrive 比阿里云少一块成本的地方**。

### 3.5 token 生命周期硬约束（必须在实现里体现）

- `refresh_token` 默认 **90 天**；`access_token` 约 **1 小时**（`expires_in: 3599`）。
- **每次刷新返回新 RT，旧 RT 不被吊销** ⇒ 并发刷新不会互废，但**取到新 RT 必须立即持久化覆盖旧的**。
- **必须显式带 `offline_access`**，否则拿不到 refresh token。
- **原生应用绝不能用 client secret**：公开客户端在兑换授权码时 "must not use secrets or certificates"。

---

## 4. 路径与数据模型：ID 为主键，别用路径

- **ID 式**：`/me/drive/items/{item-id}`，item id 在**重命名、移动后不变** → 必须作为持久主键（写进 `RemoteEntry.entry_id`，`provider.rs:16-35` 已留该字段，`dir_adapter.rs:64-65` 会透传）。
- **路径式**：`/me/drive/root:/{path}:/children` 依赖名字，改名即失效 → 只用于「用户手输初始路径」与展示。
- **路径编码按 RFC 3986，只编码 path 段**：`pchar` 含 `+`；`#`、空格等必须百分号编码；官方示例 `saved_game[1].bin` 的 `[1]` **不编码**。商业版额外把 `#`、`%` 列为保留字符。
- **`parentReference.path` 形如 `/drive/root:/Documents`** → 取相对路径必须剥掉 `/drive/root:` 前缀。
- **移到根不能用 `"id":"root"`**，必须给真实根文件夹 ID（先 `GET /drives/{drive-id}/root`）。
- **列目录默认 200 项**，超出返 `@odata.nextLink`，分页参数 `$skipToken` ⇒ `read_dir` 必须实现翻页，不能假设一次取全。

---

## 5. 落地点清单

### 5.1 后端（A 级必改）

| # | 文件 | 改什么 |
|---|---|---|
| 1 | `src-tauri/src/remote/connection.rs:19-46` | 加 `Onedrive` 变体（**命名见 §5.3 的坑**） |
| 2 | 同上 `:50-59` `default_port()` | 加 443 |
| 3 | 同上 `:61-71` `as_str()` | 返回 `"onedrive"` |
| 4 | 同上 `:232-246` `connection_error_hint()` | 加分支（穷尽 match） |
| 5 | 同上 `:285` 附近 | 补一条协议名锁定测试 |
| 6 | `src-tauri/src/remote/commands.rs:802` | `provider_with_protocol` 的 `match` 加分支 |
| 7 | `src-tauri/src/remote/registry.rs:128-136` | `matches!` 加协议 → 拿 `password: None` |
| 8 | `src-tauri/src/remote/connection_test.rs:249-296` | probe 分支 + `probe_onedrive` |
| 9 | `src-tauri/src/remote/mod.rs:20-47` + `pub use` 段 | 声明并导出新模块 |
| 10 | `src-tauri/src/remote/onedrive_auth.rs` 🆕 | 会话结构 + 版本号 + 存取四函数 + `is_onedrive_session_blob` 守卫 |
| 11 | `src-tauri/src/remote/onedrive_api.rs` 🆕 | Graph HTTP 封装 + 纯函数层（URL 编码、分页、错误码映射可单测） |
| 12 | `src-tauri/src/remote/onedrive_provider.rs` 🆕 | `impl RemoteProvider` |
| 13 | `src-tauri/src/onedrive_oauth.rs` 🆕 | 回环监听 + PKCE + state + 换 token + 刷新 |

### 5.2 前端（A 级必改）

| # | 文件 | 改什么 |
|---|---|---|
| 14 | `src/types/remote.ts:15-22` | `RemoteProtocol` 联合类型加 `'onedrive'` |
| 15 | 同上 `:30` | `NETDISK_PROTOCOLS` 加值（**权威单点**） |
| 16 | 同上 `:183-204` | `getRemoteProtocolDefaultPort` 加分支 |
| 17 | 同上 `:260-264` | `NETDISK_PROVIDER_NAME_KEYS` 补键（**不补就编译报错**，是护栏） |
| 18 | 同上（授权状态区） | 🆕 `OnedriveAuthStatusPayload` 等镜像类型 |
| 19 | `src/stores/storage/remote-connections.ts:95/:112` | `fixedNetdiskHost` / `fixedNetdiskPort` 加分支 |
| 20 | 同上 `:154-158` | `NETDISK_PROTOCOL_LABELS` 加中文标签 |
| 21 | 同上 `:431` 起的 `saveConnection` | 加第五路分派（照 Aliyun `:534`） |
| 22 | `settings/.../remote/connections-section.vue:553-580` | 加第四个「添加 OneDrive」按钮（照 Aliyun `:577`） |
| 23 | 同上 `:301-308` / `:366-371` | 加 `startAddOnedriveNetdisk`（走完整表单，不走快捷弹窗） |
| 24 | `settings/.../remote/connection-form.vue` | 照 `:246-545` 加 OneDrive 授权卡（状态 / 开始登录 / 取消 / 重新登录） |
| 25 | `src/localization/messages/*.json` × 17 | 协议名 + provider 块（`embedded.status` 那套 key 名不能改名） |

**明确不需要动**：`settings/.../remote/netdisk-add-dialog.vue`（它的 `:45` 局部硬编码与 `:263` 主机三元式是百度/夸克专用路径，OneDrive 不走它）、`sections.ts`（只要 `isNetdiskProtocol` 认了新协议就自动跟上，注释 `:11-12` 明说）、`capabilities/`（回环方案不需要独立能力文件，见 §3.4）。

### 5.3 两个必须点名的坑

**坑 1：serde 与 `as_str()` 会分叉。**
`connection.rs:18` 有 `#[serde(rename_all = "snake_case")]`。若变体写成 `OneDrive`，serde 得 `one_drive`，而 `as_str()` 手写返回 `"onedrive"` ⇒ **磁盘序列化值与 `as_str()` 返回值不一致**，是埋雷（前端 `NETDISK_PROTOCOLS` 只认其中一个）。
✅ 解法：变体写成 **`Onedrive`**（serde 得 `onedrive`，天然一致），或给变体加显式 `#[serde(rename = "onedrive")]`。**并补一条测试同时断言 serde 值与 `as_str()`**。

**坑 2：漏改后端第二处白名单 = 静默数据事故。**
`registry.rs:128-136` 决定「不读口令」。OAuth 协议漏改这里，`resolve_connection` 会把 DPAPI 里那段会话 JSON 当作 `password` 塞进 `ResolvedConnection.password`（结构 `:54-58`）——不报错，但会在别处泄漏/误用。

---

## 6. 两级工作量

### A 级「最小可用」= 浏览 + 下载 + 全局索引 + 连接测试
- 后端：改 9 个既有文件 + 新增 4 个文件（§5.1）
- 前端：改 4 个既有文件 + 17 份语言包（§5.2）
- 写操作全部显式 `Unsupported`（合法，与百度/夸克现状一致）
- 不含缩略图、不含上传/改名/删除/新建目录

### B 级「完整」= A 级 + 读写
在 A 级之上追加：
- `write_file` / `write_file_with_progress`（必须真报进度，`provider.rs:86-113`，分块常量 `REMOTE_WRITE_CHUNK_BYTES = 256 KiB` `:39`）
- `make_dir` / `remove_path` / `rename_path`
- 上传会话（§7 第 6 条）+ 异步复制（§7 第 8 条）
- 专有命令三件套（`save_session` / `auth_status` / `delete_session`）+ `lib.rs:408-449` 注册 + `app-commands.toml:163-204` 白名单
- 前端 store 分派细化 + 授权卡完整状态机

**量级判断：B 级的主要成本不在 provider，而在 OAuth 授权链路。** provider 读路径是「4 个端点 + 一个翻页」，而 OAuth 是「监听 + PKCE + state + 兑换 + 刷新轮转 + 错误分类」。

---

## 7. 实现必须遵守的硬约束（逐条有官方依据）

1. **`PUT .../content` 默认 `conflictBehavior = replace`，会静默覆盖同名文件**；要「不覆盖」必须显式 `fail`/`rename`，且该注解**放在 URL 里而非 body 里**。
2. **简单上传上限 250 MB**；更大的必须用上传会话。
3. **上传分片必须是 320 KiB（327,680 B）的整数倍**，单请求 < 60 MiB，**必须顺序上传**；推荐 5–10 MiB；官方建议 >10 MiB 才用会话。
4. **PUT 到 `uploadUrl` 时不要带 `Authorization`**（带了 401）。
5. **重复发送已收分片 → 416**；`nextExpectedRanges` 只表示「尚未收到的范围」，**不是上传模板**。
6. **上传会话提交方式按 drive 类型分支**：Business/SharePoint 用空 POST 提交（`deferCommit`），Personal 用最终 PUT 提交。
7. **`GET .../content` 返回 302** 到预认证 URL；**Range 必须加在预认证 URL 上，加在 `/content` 上无效**；预认证 URL 约 **1 小时失效且不可缓存** ⇒ 长下载必须能重新换新 URL。
8. **复制是异步的**：202 + `Location` 监控 URL，需轮询到 `completed|failed`；进程重启丢失该 URL 就无法得知结果。
9. **删除是进回收站，不是永久删除**（`DELETE` → 204）⇒ UI 文案不能写「永久删除」。
10. **错误分支只吃 `error.code`**，不吃 `error.message`（不本地化、不得直接展示）；`innererror` 可递归，取**最深一层能理解的 code**。
11. **条件访问命中返回 `403 insufficient_claims` 而不是 401** ⇒ 不能一律当「token 过期」去刷新重试。
12. **限流一律读 `Retry-After`**；无该头才指数退避；**不要立即重试**；写操作比读更容易被限；批量里子请求 429 不会自动重试。
13. **个人账户 vs 商业版有实质差异**：保留字符（商业版多 `#`、`%`）、`children?expand=thumbnails` 在商业版不生效、`originalOrientation` 仅个人版、复制 `conflictBehavior` 个人版不支持、上传提交方式不同。
14. **`read_dir` 必须翻页**（默认 200 项）。
15. **无凭据必须报 `ProbeOutcome::MissingCredential`，不能报 `AuthRejected`**（`connection_test.rs:308-312` 等三处已有先例）。
16. **`is_onedrive_session_blob` 守卫必须写**（照 `aliyun_auth.rs:374`）：阻止通用 `remote_load_credential` 把整段会话回传给前端。
17. **新命令必须双处登记**：`lib.rs:392` 起的 `generate_handler!`（remote 段 `:408-449`）+ `permissions/app-commands.toml` remote 段 `:163-204`。漏白名单是**运行期权限拒绝**，不是编译错。

---

## 8. Azure 门户必须人工完成的事（我做不了）

1. 注册应用（Entra ID → 应用注册），Supported account types 按定位选（要支持个人账户就选「任何组织目录中的账户和个人 Microsoft 账户」）。
2. 平台选**桌面/移动应用**，加重定向 URI `http://localhost`。**不要加「单页应用」平台**（会引入 SPA 型 24 小时 refresh token 约束）。
3. API 权限：Microsoft Graph → **委托权限** `Files.ReadWrite`（只读则 `Files.Read`）→ **不要加应用程序权限**（`/me/drive` 不支持 Application 权限）。
4. **不要生成 client secret / 证书**。
5. 抄下 Application (client) ID。
6. 只走回环方案时**不需要**开「允许公共客户端流」；若要做设备码流备用分支则必须开。

---

## 9. 验收方案（沿用批次 37 装置）

- **沙箱约束**：`CreateDesktop` + `STARTUPINFO.lpDesktop`，访问掩码 `DESKTOP_ALL (0x1FF)`，`EnumDesktopWindows` 枚举；断言一律走 UIA dump（`assertNamePresent` 全控件 `Name` 通配扫描），坐标从 UIA 树动态定位，先 `waitReady`。**沙箱 PNG 不作为证据。**
- **构建**：只认 `npm run tauri:build` 产物；`tauri dev` 界面结论作废。
- **数据目录**：release 验收须「备份真实 `%APPDATA%\com.sigma-file-manager.app\user-data\` → 灌夹具 → 验收 → 先杀进程再 restore」。
- **本批次特有的验收难点（必须提前设计，否则会假红）**：
  1. **OAuth 回环无法在沙箱内完成真实登录**（需要在真浏览器里输账号密码 + MFA）。⇒ 验收必须把「认证」与「已认证后的行为」切开：认证段只验「点击开始登录 → 系统浏览器被拉起 / 授权 URL 参数正确（client_id、redirect_uri、scope 含 `offline_access`、`code_challenge` 与 `code_challenge_method=S256`、`state` 非空）」；数据段用**注入的令牌包**（照批次 37 灌夹具的方式写进 DPAPI 仓）验浏览/下载/索引。
  2. **没有真实 OneDrive 凭据时，`read_dir` 类断言一律登记「未判定」，不得登记 PASS/FAIL**（沿用批次 37 §10.4 的口径）。
  3. **纯函数层可完全单测**：URL 编码（含 `+`、`#`、`%`、空格、`[1]` 不编码）、`@odata.nextLink` 翻页、状态码 → `RemoteErrorKind` 映射、`parentReference.path` 剥前缀、上传分片对齐 320 KiB、`Retry-After` 解析。这些应在 A 级就全绿。

---

## 10. 待业主（用户）拍板的两件事

### 决策一：`client_id` 归属 —— **这是唯一的硬阻塞**

没有有效的 Azure 应用注册，A 级连一次真实调用都跑不通。

| 选项 | 代价 | 备注 |
|---|---|---|
| a. 让用户在连接表单里**自填** client_id | 零前置；普通用户门槛极高 | **本仓库已有先例**：阿里云 `startAddAliyunNetdisk` + 表单填 `client_id`（`connections-section.vue:301-308`） |
| b. 项目方注册自有应用并**内置** client_id | 需有人在 Azure 门户做 §8；开源仓库硬编码 client_id 合法（官方明确 client_id 是公开值） | 长期 UX 最佳 |
| c. 内置默认 + 高级用户可覆盖 | b 的成本 + a 的灵活 | 最终形态 |

**工程建议**：**先按 a 打通整条链路**（不阻塞开发、不动 Azure），把 c 作为上线前的形态。这样本批次可以立刻进入实现。

### 决策二：A 级范围是否包含「只读 vs 读写」

- 只读（`Files.Read`）：能满足浏览 + 下载 + 索引，风险面最小。
- 读写（`Files.ReadWrite`）：要多做上传会话 / 移动 / 删除 / 新建目录，成本显著上升（§6 B 级）。

**工程建议**：A 级按**只读**交付并验收，B 级单独一批。理由：只读已经能验证完 OAuth 链路 + 寻址模型 + 翻页 + 错误分类这四件真正有风险的事；写操作的风险（覆盖、分片、异步复制）各自独立，混在一起会让验收判据互相污染。

---

## 11. 本轮做了什么 / 没做什么

**做了**：读批次 37 交接文档与 `AGENTS.md`；核仓库基线（`27110478`、干净、3.17.0）；派两个只读侦察子会话产出 `.superpowers/sdd/b38/recon-code.md` 与 `.superpowers/sdd/b38/recon-graph-api.md`；核实 `tauri-plugin-opener` / `tokio net` 可用性；合成本文。

**没做**：未写任何产品代码；未改任何既有文件；未跑构建/测试；**未跑 `CI=true npm run check`**（本轮只有文档新增，无产品代码变更，门禁留到实现批次）。

**未验证（明确登记）**：`sha2` 依赖是否存在；OneDrive 真实凭据下的端到端行为；Graph 的 `$top` 上限、限流阈值数字、缩略图 URL 有效期秒数、`driveType` 枚举完整值、租户禁用用户同意的后果——这些在 `recon-graph-api.md` §G 已逐条登记为「未找到官方依据」，不得据印象补全。