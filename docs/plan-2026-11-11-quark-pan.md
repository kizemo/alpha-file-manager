# 夸克网盘（Quark Netdisk）接入 AFM「远程位置」——调研结论与实施计划

- 文档时间：2026-11-11
- 文档性质：**只读调研结论 + 实施计划**。本轮**不含任何功能代码**，仓库除本文件与 `docs/review-2026-11-11-quark-pan.md`（评审报告）外未做任何改动。
- 前置文档：`docs/plan-2026-11-08-baidu-netdisk-phase1.md`（百度阶段 1 计划，本文的模板）
- 调研对象：
  - 仓库侧：既有百度网盘实现（只读）
  - 客户端侧：本机安装的夸克网盘 PC 客户端 `D:\Program Files\Quark\`（**只读**，未启动、未登录、未安装任何东西）
- 预估工作量：**11.5–12.5 人天**（v1 只读 + 登录 + 浏览）。原估 8.0 人天**偏乐观 40–55%**；复核值与依据见 §5.2。若 T0 判定走退路，再 **+1.5–2.0 人天**（合计 13.0–14.5）。

---

## 1. 背景与目标

### 1.1 背景

Alpha File Manager 目前只支持 `ftp` / `ftps_explicit` / `ftps_implicit` / `sftp` 四种远程协议，以及第五种「网盘类」协议 `baidu_pan`。用户希望把**夸克网盘**也作为远程位置接进来，行为与百度网盘完全一致：在「添加网盘 / 添加远程连接」界面里多一个夸克网盘选项，点「添加」弹出内嵌登录网页（扫码或手机号/密码），登录成功后应用拿到会话，随后即可把夸克网盘当远程目录浏览。

### 1.2 目标

**做（v1）**

1. **应用内授权**：内嵌 WebView2 打开夸克网盘登录页，用户扫码 / 账号密码登录，应用自己取 cookie 并加密落盘。
2. **列目录**：`read_dir`，含分页（夸克一页 100 条，`_size` 上限待验）。
3. **下载**：单文件下载，复用既有 `transfer::download` 的进度事件。
4. **连接设置表单**：夸克协议专属的授权卡，取代 host / port / 密码输入。
5. **错误分类**：所有失败必须落到既有 11 个 `RemoteErrorKind` 之一，**不新增 kind**。

**不做（v1）**

| 项 | 归属 | 理由 |
|---|---|---|
| 上传 / 拖拽入远程 | 永不做 | 夸克上传需要分片 + 秒传 + `upload/pre` + `upload/auth` + `upload/finish` 三段式 |
| 写操作（建目录 / 改名 / 删除） | v1.1 可选 | endpoint 已实证存在（见 §3.2），但需额外处理 `24005` / `23008` 冲突码与 `31001` 会话失效分流 |
| `set_permissions` / `get_permissions` | 永不 | 夸克没有 POSIX 权限位，必须返回 `Unsupported` / `Ok(None)` |
| 分享 / 转存 / 离线下载 / 分类 | 永不 | 与文件浏览无关 |
| 流式下载 | 永不做 | `RemoteProvider` trait **刻意不提供流接口**（`provider.rs:55` 注释：不做 `read_to` / `write_from` 之类的流接口），要做就得改 trait，并波及全仓库 **5** 个 `impl RemoteProvider for` 实现 |
| 多账号同时在线 | 不做 | 多账号 = 建多条连接，cookie 各存各的 |

### 1.3 与百度网盘的关系

夸克网盘**完全照抄百度网盘的形态**：内嵌 WebView2 登录 → 拿 cookie → cookie 直连会话 → DPAPI 密文落盘 → `RemoteProtocol` 新变体。

**不引入 `refresh_token` / OAuth 流程。** 两条独立理由：

- 百度侧这条路**已经被实测否决**：计划 D2 定的「内嵌登录 + 换 refresh_token」在 T1 实测中被推翻（`openapi.baidu.com/oauth/2.0/token` 对 `client_id=250528` 与阴性对照返回逐字相同的 `invalid_client`，`authorize` 直接 404），路线改为持久化 `BDUSS` + `STOKEN` 两个 cookie 当会话用，**「刷新」这一步不存在，cookie 失效即重新登录**（`src-tauri/src/remote/baidu_auth.rs:21-31`）。
- 夸克本地客户端**本身就是 cookie 模型**：`withCredentials: true` + `cookieHost: "quark.cn"` + 固定 query 参数，**没有** `refresh_token` 的任何迹象（本地 JS 全量搜索无命中）。

---

## 2. 现有百度网盘实现架构

### 2.1 分层总览

```
[设置页前端]  connection-form.vue  ──invoke──▶  [Tauri command 层]  commands.rs
                                                      │
                                                      ├─▶ remote_quark_* 命令族
                                                      │      ├─▶ quark_login.rs   （内嵌 WebView2 登录窗）
                                                      │      ├─▶ quark_auth.rs    （会话编解码 / 状态）
                                                      │      └─▶ quark_api.rs     （纯 HTTP + 纯解析，零 tauri::）
                                                      │
                                                      └─▶ provider_with_protocol() ──▶ quark_provider.rs
                                                                                        │
                                                                                        └─▶ remote/credentials.rs（DPAPI 密文）
```

**关键分层约束**：`quark_api.rs` / `quark_auth.rs` / `quark_provider.rs` 三个文件**不得出现 `tauri::` 的任何路径**（`tauri::AppHandle` / `tauri::async_runtime::*`），否则 `cargo test --lib` 链接期会把 `tao` 整条窗口链拖进测试二进制，进程在加载阶段以 `STATUS_ENTRYPOINT_NOT_FOUND (0xC0000139)` 崩掉，一个测试都跑不了（`src-tauri/src/remote/mod.rs:8-14`）。

### 2.2 Touch-point 完整清单

以下每一行都是**必须改动**或**必须确认**的位置。行号对应当前工作区状态。

#### 2.2.1 后端 · 数据模型与协议注册

| 文件 | 行号 | 现状 | 夸克要做什么 |
|---|---|---|---|
| `src-tauri/src/remote/connection.rs` | `19-33` | `enum RemoteProtocol { Ftp, FtpsExplicit, FtpsImplicit, Sftp, BaiduPan }` | 新增 `QuarkPan` 变体 |
| `src-tauri/src/remote/connection.rs` | `37-44` | `default_port()` 匹配，`Self::BaiduPan => 443`（`:42`） | 加 `Self::QuarkPan => 443` |
| `src-tauri/src/remote/connection.rs` | `46-54` | `as_str()` 匹配，`Self::BaiduPan => "baidu_pan"`（`:52`） | 加 `Self::QuarkPan => "quark_pan"` |
| `src-tauri/src/remote/connection.rs` | `57-60` | `struct RemoteConnection`（`#[serde(rename_all = "camelCase")]`） | **不改**（不新增字段） |
| `src-tauri/src/remote/mod.rs` | `20-39` | `mod` 声明块（`baidu_api` / `baidu_auth` / `baidu_provider` 在 `:20-22`） | 加 `mod quark_api;` / `mod quark_auth;` / `mod quark_provider;` |
| `src-tauri/src/remote/mod.rs` | `41-68` | **四段** `pub use baidu_*`（`:41` / `:53` / `:58` / `:65`） | 加两段 `pub use quark_*` |
| `src-tauri/src/remote/mod.rs` | `8-14` | `cargo test --lib` 红线注释 | 夸克三个新文件顶部照抄同一段说明 |
| `src-tauri/src/remote/registry.rs` | `119-127` | `BaiduPan` 分支：**不走**通用 password 路径（`password: None`） | 加 `QuarkPan` 同款分支 |
| `src-tauri/src/remote/error.rs` | `17-46` | `RemoteErrorKind` 共 **11** 个 | **不改** |

#### 2.2.2 后端 · 纯逻辑层（模板：`baidu_api.rs` / `baidu_auth.rs`）

`src-tauri/src/remote/baidu_api.rs`（2424 行）关键锚点：

| 行号 | 符号 | 夸克对应物 |
|---|---|---|
| `47` | `BAIDU_PAN_HOST = "pan.baidu.com"` | `QUARK_PAN_HOST = "pan.quark.cn"` |
| `50` | `BAIDU_PAN_BASE_URL` | `QUARK_DRIVE_BASE_URL = "https://drive-pc.quark.cn"` |
| `58` | `BAIDU_PAN_APP_ID`（注释写明它**不是** OAuth client_id） | `QUARK_CLIENT_ID = "533"`（本地实证的 query 参数） |
| `65` | `BAIDU_PAN_USER_AGENT` | `QUARK_USER_AGENT`（需带 `QuarkPC/x.y.z.w` 家族标识，见 §7） |
| `70` | `BAIDU_LIST_PAGE_LIMIT = 1000` | `QUARK_LIST_PAGE_SIZE = 100` |
| `73` / `78` / `84` | 请求 / 连接 / 下载三级超时 | 三级超时（夸克下载 URL 自带 30 分钟提前过期，见 §3.2） |
| `87` / `90` | `BAIDU_COOKIE_BDUSS` / `BAIDU_COOKIE_STOKEN` | `QUARK_COOKIE_PUS = "__pus"`（**仅开源项目侧证据**，见 §7） |
| `106` | `struct BaiduSession`（手写 `Debug`，只打印长度不打印明文） | `struct QuarkSession`（同款） |
| `174` | `sanitize_cookie_value()`（**判非法**而非过滤） | 同款照抄 |
| `208` | `list_dir_url()` | `quark_list_dir_url()` |
| `222` | `uinfo_url()` | `quark_account_info_url()` |
| `233` | `filemetas_url()`（⚠️ 未实证） | `quark_download_url()` |
| `283` | `clamp_page_limit()`（服务端**静默截断**超限 limit 且不报错） | 同款——夸克 `_size` 上限待验，但**必须夹** |
| `297` | `next_page()` | 同款（夸克有 `_fetch_total`，可更精确） |
| `318` | `struct BaiduFileEntry` | `struct QuarkFileEntry` |
| `400` / `413` / `431` | `parse_list_response` / `parse_uinfo_response` / `parse_dlink_response` | 同款三份解析器 |
| `562` | `enum BaiduFailure`（协议无关的语义分类） | `enum QuarkFailure` |
| `589` | `classify_baidu_failure()`（**穷尽**映射表） | `classify_quark_failure()`（**穷尽**） |
| `617` | `failure_from_errno()`（逐条标注证据强度） | `failure_from_code()`（夸克是 `code` 不是 `errno`） |
| `647` | `failure_from_http_status()` | 同款 |

`src-tauri/src/remote/baidu_auth.rs`（155 处命中）关键锚点：

| 行号 | 符号 | 说明 |
|---|---|---|
| `21-31` | 模块头注释 | **记录了百度为何放弃 refresh_token**，夸克照抄这个结论 |
| `49` | `BAIDU_SESSION_VERSION = 1` | 夸克用 `QUARK_SESSION_VERSION = 1` |
| `54` | `BAIDU_STOKEN_EXPIRY_WARN_SECONDS = 86400` | 夸克没有 STOKEN，改为「距上次验活时间」语义 |
| `122-167` | `scan_cookies()` | 大小写敏感、同名取第一个、只投影必要字段 |
| — | `encode_session` / `decode_session` / `is_baidu_session_blob` / `load_baidu_session` / `save_baidu_session` / `delete_baidu_session` | 会话 blob 的编解码与存取 |
| — | `describe_state` / `baidu_auth_status` / `is_expired` / `remaining_seconds` | 授权状态查询（前端授权卡的数据源） |
| `436` | `session_to_cookie_header()`（会话 → `Cookie` 请求头字符串） | 夸克同款 —— `commands.rs:282` 正在用它，**不能漏** |

#### 2.2.3 后端 · provider 层（模板：`baidu_provider.rs`，2306 行）

| 行号 | 符号 | 夸克对应物 |
|---|---|---|
| `54` | `BAIDU_DIRECTORY_TIMEOUT = 120s` | `QUARK_DIRECTORY_TIMEOUT` |
| `60` | `BAIDU_FILE_TIMEOUT = 900s` | `QUARK_FILE_TIMEOUT` |
| `67` | `BAIDU_MAX_LIST_PAGES = 200` | `QUARK_MAX_LIST_PAGES` |
| `73` | `BAIDU_MAX_DOWNLOAD_BYTES = 500 MiB` | `QUARK_MAX_DOWNLOAD_BYTES`（夸克下载 URL 30 分钟过期，阈值要更保守） |
| `77` | `trait BaiduApi`（**抽 trait 是为了零网络单测**） | `trait QuarkApi` |
| `124` | `struct UnavailableBaiduApi`（占位实现，把初始化错误推迟到第一次真用网络） | `UnavailableQuarkApi` |
| `179` | `trait BaiduSessionStore`（**抽 trait 是为了零 DPAPI 单测**） | `trait QuarkSessionStore` |
| `200` | `struct HttpBaiduApi` | `HttpQuarkApi` |
| `369` | `struct DpapiBaiduSessionStore` | `DpapiQuarkSessionStore` |
| `410` | `struct BaiduRemoteProvider`（**不走** `registry::resolve_connection`，那条路会把密文当密码读） | `QuarkRemoteProvider` |
| `637` | `impl RemoteProvider for BaiduRemoteProvider` | 同款 |

`src-tauri/src/remote/provider.rs` 的 trait 契约（**不改**，夸克实现全部 9 个方法）：

| 行号 | 方法 | 夸克 v1 行为 |
|---|---|---|
| `40` | `pub trait RemoteProvider: Send + Sync` | — |
| `46` | `read_dir` | **实现**：调 `file/sort` + 分页 |
| `58` | `read_file` | **实现**：取 `download_url` 后下载 |
| `69` | `write_file` | `RemoteError::unsupported(...)` |
| `97` | `write_file_with_progress` | `RemoteError::unsupported(...)` |
| `106` | `make_dir` | v1 `unsupported`；v1.1 可实现（endpoint 已实证） |
| `118` | `set_permissions` | `RemoteError::unsupported(...)` |
| `131` | `get_permissions` | `Ok(None)`（协议无权限位） |
| `147` | `remove_path` | v1 `unsupported`；v1.1 可实现 |
| `177` | `rename_path` | v1 `unsupported`；v1.1 可实现 |

`src-tauri/src/remote/credentials.rs`（DPAPI 凭据链路，**不改**）：

| 行号 | 符号 |
|---|---|
| `54` | `credentials_path_in()` |
| `343` | `protect()`（**必须**带 `CRYPTPROTECT_UI_FORBIDDEN`） |
| `390` | `unprotect()`（解不开归 `Auth` 而不是 `Unknown`） |
| `461` | `load_credential()` |
| `486` | `save_credential()`（走 `vault_for_write`，坏文件先备份） |
| `506` | `delete_credential()` |

#### 2.2.4 后端 · 命令层与登录窗

`src-tauri/src/remote/commands.rs`：

| 行号 | 符号 | 夸克要做什么 |
|---|---|---|
| `37-38` | `use` 导入 | 加夸克模块导入 |
| `149` | `remote_baidu_save_session` | 加 `remote_quark_save_session` |
| `170` | `remote_baidu_auth_status` | 加 `remote_quark_auth_status` |
| `206` | `remote_baidu_delete_session` | 加 `remote_quark_delete_session` |
| `270-285` | 「测试连接」从存档还原会话作 credential | 加夸克分支 |
| `358` / `379` / `586` | 三个 `BaiduPan` 分支（**分属三个不同函数**；`provider_with_protocol` 函数体在 `:573`） | 各加一个 `QuarkPan` 分支（**穷尽匹配，编译器会强制你改全**） |
| `991-1004` | 命令注册表**反向自检**清单（含 7 个 baidu 命令） | 加夸克命令，否则自检失败 |

`src-tauri/src/baidu_login.rs`（1092 行）关键锚点：

| 行号 | 符号 | 说明 |
|---|---|---|
| `5-38` | 模块头注释 | **三条硬约束**：cookie 明文只在模块与 `baidu_auth` 间流转 / 不得引用 `tauri::async_runtime::*` / **登录窗必须在独立 `std::thread` 里建** |
| `54` | `LOGIN_WINDOW_LABEL = "afm-baidu-login"` | `afm-quark-login` |
| `56` | `LOGIN_START_URL = "https://pan.baidu.com/disk/main"` | `https://pan.quark.cn/` |
| `58` | `LOGIN_COOKIE_URL = "https://pan.baidu.com/"` | `https://pan.quark.cn/`（cookie 锚点） |
| `60` | `LOGIN_EVENT = "afm://baidu-login"` | `afm://quark-login` |
| `62` / `64` | `STOKEN_NAME` / `BDUSS_NAME` | `__pus`（待验，见 §7） |
| `66` / `67` | `POLL = 2s` / `GIVE_UP = 900s` | 同款 |
| `68` | `STOKEN_WAIT = 20s` | 夸克改成「等 `__pus` 出现」 |
| `76` / `78` | `COOKIE_READ_BUDGET = 10s` / `MAX_READ_FAILURES = 3` | 同款 |
| `82` | `VERIFY_BUDGET = 8s` | 同款 |
| `84-95` | **三道保险**：① 建窗后清 jar 旧 cookie ② Ready 必须「当前 cookie ≠ 基线 cookie」 ③ 关窗前对服务端验活，只在拿到确凿死亡证据时拒绝 | **照抄三道保险**（但换成夸克的 cookie 名与服务端验活接口） |
| `108-137` | `PendingSession` / `PendingSlot` / `take_pending` / `store_pending` | **pending-session slot + claim-on-save** 两段式 |
| `347` / `430` / `448` / `485` | `remote_baidu_login_start` / `_pending` / `_claim` / `_cancel` | 夸克同款四个命令 |
| `536` | `PROBE_LIVE_URL` | 夸克的验活 URL |
| `773`（起点已核）/ `956`（**区间终点未核验**） | `poll_loop`（轮询 cookie → 判定 Ready → 关窗 → 验活） | 同款 |

`src-tauri/src/lib.rs`：

| 行号 | 符号 | 夸克要做什么 |
|---|---|---|
| `11-12` | `mod baidu_login;` / `mod baidu_probe;` | 加 `mod quark_login;` |
| `377-383` | `generate_handler!` 里 7 个 baidu command | 加 4 个夸克 login 命令 + 3 个夸克会话命令 |
| `698-703` | `baidu_probe::install` / `install_selftest`（仅 `AFM_BAIDU_PROBE=1` / `AFM_BAIDU_SELFTEST=1` 生效） | 可选加 `AFM_QUARK_PROBE=1` |

`src-tauri/src/baidu_probe.rs`（31 处命中）：百度专用探针模块。夸克可照抄一个 `quark_probe.rs`。

`src-tauri/capabilities/default.json`：已具备 `core:webview:allow-create-webview-window`、`core:webview:allow-get-all-webviews`、`core:window:allow-close/destroy/...`；`windows` 数组是 `["main","quick-view","print-view"]`（**登录窗由 Rust 侧建，前端只 invoke**）。→ **夸克不需要改 capabilities**。

#### 2.2.5 前端 · 类型与判定

`src/types/remote.ts`：

| 行号 | 符号 | 夸克要做什么 |
|---|---|---|
| `14-15` | `export type RemoteProtocol = 'ftp' \| 'ftps_explicit' \| 'ftps_implicit' \| 'sftp' \| 'baidu_pan'`（注释写「五个变体」） | 加 `'quark_pan'`，注释改「六个变体」 |
| `173-174` | `isBaiduPanProtocol()` —— **唯一判定口径** | 加 `isQuarkPanProtocol()`；**并新增一个统一的 `isNetdiskProtocol()`** 收口两种网盘协议（否则前端会出现第二套并行判定口径） |
| `190` | `BaiduAuthStatusPayload`（与后端逐条镜像） | 加 `QuarkAuthStatusPayload` |
| `286` | `BaiduLoginClaimPayload` | 加 `QuarkLoginClaimPayload` |
| `302` | `BaiduLoginEventPayload` | 加 `QuarkLoginEventPayload` |

`src/utils/virtual-locations.ts`：

| 行号 | 内容 | 夸克要做什么 |
|---|---|---|
| `31` | 远程位置构造入口 | 加夸克分支 |
| `137-139` | 用 `isBaiduPanProtocol` 设 `location_group` | 改成用统一判定 |

`src/utils/remote-location-refs.ts:12`：同口径。

`src/types/dir-entry.ts:40`：目录条目的 `location_group` 类型定义（网盘归属组）。

#### 2.2.6 前端 · 存储

`src/stores/storage/remote-connections.ts`：

| 行号 | 内容 | 夸克要做什么 |
|---|---|---|
| `48-49` | 固定 host / port（百度固定为 `pan.baidu.com` / 443） | 加夸克固定值 |
| `323-346` | 保存时百度分支调 `remote_baidu_save_session` / `remote_baidu_delete_session`，并调 `remote_baidu_login_claim` | 加夸克分支 |
| `358` | `baiduLoginRequired` 错误 | 加 `quarkLoginRequired` |
| `380-386` | 保存时**按协议强制** host / port | 加夸克分支 |
| `488-519` | 「测试连接」特殊处理（百度从存档还原会话） | 加夸克分支 |
| `533` | `getBaiduAuthStatus` | 加 `getQuarkAuthStatus` |
| `542` | `deleteBaiduSession` | 加 `deleteQuarkSession` |
| `558` | `startBaiduLogin` | 加 `startQuarkLogin` |
| `563` | `cancelBaiduLogin` | 加 `cancelQuarkLogin` |

#### 2.2.7 前端 · UI

`src/modules/settings/ui/categories/remote/connection-form.vue`（1032 行）：

> ⚠️ **不存在** `baidu-connection-form.vue`。目录下只有同名 **`.test.ts`**，百度分支**在 `connection-form.vue` 内部**。夸克照抄这个位置，不要去找一个不存在的文件。

| 行号 | 内容 | 夸克要做什么 |
|---|---|---|
| `63-69` | `PROTOCOLS = ["ftp", "ftps_explicit", "ftps_implicit", "sftp"]` —— **刻意不含网盘协议**（`:66-67` 注释：「把它塞进这个下拉等于给用户一条『选了却填不了对应字段』的死路」） | **不改** —— 网盘协议**不进**这个下拉，改用 provider 选择器（见 §4.4） |
| `69` | 协议选项的实际位置（`:26` 只是一行 `import`，不是选项来源） | 不改（协议下拉只留远程连接协议） |
| `230-232` | 网盘协议**硬编码**：`props.variant === "netdisk" ? "baidu_pan" : ...` | 改成按 provider 分流：百度 → `baidu_pan`，夸克 → `quark_pan` |
| `79` | `BAIDU_PAN_HOST = 'pan.baidu.com'` 常量 | 加夸克常量 |
| `89-134` | `baiduLoginState` ref / `handleStartBaiduLogin`（`:95`）/ `handleCancelBaiduLogin`（`:114`） | 加夸克等价物 |
| `139` | 监听 `afm://baidu-login` 事件 | 加 `afm://quark-login` |
| `216-245` | 网盘分支隐藏 host / port / 密码框（协议恒为 `baidu_pan`） | 加夸克分支（协议恒为 `quark_pan`） |
| `376-413` | 授权状态卡（显示昵称 / 失效重授权） | 加夸克版本 |

`src/modules/settings/ui/categories/remote/sections.ts:8-20`：用 `isBaiduPanProtocol` 把连接分流到 `'netdisk'` 分区 → 改成统一判定 `isNetdiskProtocol`。

`src/modules/settings/ui/categories/remote/connections-section.vue`（**网盘 / 远程分区的真正决策点**，原方案漏列）：

| 行号 | 内容 | 夸克要做什么 |
|---|---|---|
| `32` | 注释：「判定口径只有一个 —— `isBaiduPanProtocol`。以后接入阿里云盘之类的网盘，只要把判定扩进去，这一页的分区自动跟上」 | 判定扩成 `isNetdiskProtocol` |
| `43-44` | 用 `isBaiduPanProtocol` 把连接分成「网盘 / 远程」两个分区 | 改成 `isNetdiskProtocol` |
| `87` | `activeVariant`（表单形态跟标签走） | 网盘分支要能携带 provider（百度 / 夸克） |
| `93-97` | 回落逻辑：「带着 `baidu_pan` 切到远程连接标签，表单会选出一个下拉里根本不存在的值」 | 同样要挡住 `quark_pan` |

`src/modules/settings/ui/categories/remote/index.vue`：父级分类，靠分区结果决定「信任的主机」是否显示 → 同口径改判定。

`src/modules/nav-sidebar/components/remote-nav-panel.vue`：

| 行号 | 内容 | 夸克要做什么 |
|---|---|---|
| `11` | 分区定义 | 「网盘」分区 |
| `34` / `38` | 过滤逻辑 | 「网盘」分区过滤网盘协议、「远程」分区取反 |
| `63-64` | 新建请求预置协议 `'baidu_pan'` | 加夸克预置 |
| `72` | 「远程」分区取反 | 改成统一判定 |

`src/modules/tab-bar/utils/remote-tab-labels.ts`：**不需要改**。全文 60 行，**不含** `baidu` / `protocol` / `netdisk` 字样，只吃 `connectionName` ⇒ 与协议无关。

`src/modules/home/components/remote-entries-section.vue`：首页远程入口（4 处命中）—— **命中数未核验**。

`src/modules/tab-bar/tab/tab.vue`、`src/modules/nav-sidebar/nav-sidebar.vue`：各 1-2 处命中，协议名展示 —— **命中数未核验**。

> ⚠️ 上面这三处的命中数与「是否需要改」**本轮未核验**（评审 N13）。它们**不在** §4.4 的改动清单里；若实施时发现需要改，§4.4 清单要同步补齐。

#### 2.2.8 i18n

`src/localization/messages/*.json` —— 共 **17** 个语言包（**中文 locale code 是 `ch`，不是 `zh`**）。百度的实际落点是「**1 个嵌套对象 + 2 个平铺键**」：

| 位置 | 形态 | 内容 |
|---|---|---|
| `protocols.baidu_pan` | **平铺键** | `ch.json:655` → `"百度网盘"` |
| `baiduLoginRequired` | **平铺键** | `ch.json:681` |
| `baidu`（**嵌套对象**） | **对象**：叶子串分布在 `ch.json:723-748`，约 **15 个键** | `ch.json:723`（对象起点）、`ch.json:746`（`baidu.saved`）等；含 `stoken_expired` / `stoken_expiring_soon` 等**百度专属措辞** |

夸克需要在 **17 个**语言包各加「**1 个约 15 键的嵌套对象** + **2 个平铺键**」（`protocols.quark_pan` / `quarkLoginRequired` / `quark` 对象）⇒ 17 × 约 15 ≈ **250+ 条**新串，含 16 种非中文语言的人工翻译。同步命令：`npm run sync-i18n`。

> ⚠️ **夸克没有 STOKEN**：百度的「失效 / 临期」文案是 `stoken_expired` / `stoken_expiring_soon`，语义绑死在 STOKEN 上。夸克的对应文案要**重新起草语义**（如「距上次验活时间」），**不是**把 stoken 换成 pus 抄一遍。

#### 2.2.9 测试与探针

| 文件 | 命中数（**行命中**口径） | 夸克要做什么 |
|---|---:|---|
| `src-tauri/src/remote/connection_test.rs` | 14（行命中）/ **24 个 test 函数** | 协议解析测试加夸克 |
| `src-tauri/src/remote/baidu_api.rs` 内联 `mod tests` | 314 | 新建 `quark_api.rs` 的等价单测 |
| `src-tauri/src/remote/baidu_provider.rs` 内联 `mod tests` | 162 | 新建 `quark_provider.rs` 的等价单测 |
| `src-tauri/src/remote/baidu_auth.rs` 内联 `mod tests` | 155 | 新建 `quark_auth.rs` 的等价单测 |
| `src/modules/settings/ui/categories/remote/baidu-connection-form.test.ts` | 34 | 加夸克等价测试文件 |
| `src/stores/storage/__tests__/remote-connections-baidu.test.ts` | 25 | 加夸克等价测试文件 |
| `src/utils/__tests__/virtual-locations.test.ts` | 14 | 加夸克断言 |
| `src/modules/navigator/components/file-browser/__tests__/file-browser-location-groups.test.ts` | 8 | 加夸克断言 |
| `src/types/__tests__/remote-protocol-baidu.test.ts` | 9 | 加夸克断言 |
| `src/modules/nav-sidebar/components/remote-nav-panel.test.ts` | 9 | 加夸克断言 |
| `src/modules/tab-bar/utils/__tests__/remote-tab-labels.test.ts` | 7 | 加夸克断言 |
| `src/modules/settings/ui/categories/remote/remote-category.test.ts` | 6 | 加夸克断言 |
| `src/utils/__tests__/reveal-containing-directory.test.ts` | 6 | 加夸克断言 |
| `src/modules/home/components/remote-entries-section.test.ts` | 1 | 加夸克断言 |
| `src-tauri/src/dir_reader/read.rs` | 2 | 确认无需改（协议无关） |

> **口径说明**：本表的「命中数」是**行命中**口径（`baidu-touchpoints.txt` 的行数），**不是** test 函数数。Rust 三个模板的 **test 函数数**实测为 api **98** / auth **49** / provider **59**（合计 206），`connection_test.rs` 实含 **24** 个 test 函数。§6.1 的单测门槛按 **test 函数数**口径设（评审 N5）。

#### 2.2.10 完整命中清单（96 个文件 / 2189 行命中）

`.superpowers/sdd/quark/baidu-touchpoints.txt` 是本清单的**机器生成全文**（每文件精确行号 + 该行原文），按文件路径排序，共 96 个文件：

```
AGENTS.md(4) CHANGELOG.md(12) README.md(7) README.zh-CN.md(7)
docs/2026-11-07-baidu-netdisk-drive-feasibility.md(33)
docs/2026-11-07-baidu-t1-auth-findings.md(56)
docs/2026-11-08-baidu-login-auto-fill-feasibility.md(81)
docs/2026-11-08-baidu-t7-t8-frontend-checklist.md(43)
docs/2026-11-08-baidu-t9-acceptance.md(37)
docs/2026-11-08-nav-unresponsive-and-baidu-entry.md(28)
docs/2026-11-09-baidu-followup-audit-snapshot.md(35)
docs/2026-11-09-baidu-write-chain-blocker.md(24)
docs/2026-11-09-baidu-write-session-findings.md(21)
docs/handoff-2026-11-08-baidu-embedded-login.md(64)
docs/handoff-2026-11-08-baidu-netdisk-phase1.md(185)
docs/handoff-2026-11-09-baidu-embedded-login.md(130)
docs/plan-2026-11-08-baidu-netdisk-phase1.md(43)
docs/archive/focus-sync-legacy/*(9) docs/handoff-2026-11-01-relay-four-tasks.md(1)
docs/handoff-2026-11-07-reveal-shortcut-target.md(6)
docs/handoff-2026-11-08-release-3.13.0-locations-partitions.md(11)
docs/handoff-2026-11-09-focus-sync-remote-path-fix.md(3)
docs/prompt-2026-11-08-relay-baidu-phase1.md(3) docs/prompt-2026-11-08-relay-nav-fix.md(4)
docs/work-2026-11-07-reveal-shortcut-target.md(12)
extensions/kizemo.focus-sync/sidecar/src/{http_server.rs(2),main.rs(3),state.rs(3)}
public/changelog/{ch,de,es,fa,fr,hi,it,ja,pt,ru,sl,tr,ur,vi,zh}/changelog.md(各 5)
src/localization/messages/{ch,de,en,es,fa,fr,he,hi,hy,it,ja,pt,ru,sl,tr,ur}.json(各 4)
src/modules/home/components/remote-entries-section{.vue(4),.test.ts(1)}
src/modules/nav-sidebar/{components/remote-nav-panel.vue(6),components/remote-nav-panel.test.ts(9),nav-sidebar.vue(2)}
src/modules/navigator/components/file-browser/__tests__/file-browser-location-groups.test.ts(8)
src/modules/settings/ui/categories/remote/{baidu-connection-form.test.ts(34),connection-form.vue(119),connections-section.vue(7),remote-category.test.ts(6),sections.ts(4)}
src/modules/tab-bar/{tab/tab.vue(1),utils/remote-tab-labels.ts(1),utils/__tests__/remote-tab-labels.test.ts(7)}
src/stores/storage/remote-connections.ts(48) src/stores/storage/__tests__/remote-connections-baidu.test.ts(25)
src/types/dir-entry.ts(1) src/types/remote.ts(18) src/types/__tests__/remote-protocol-baidu.test.ts(9)
src/utils/remote-location-refs.ts(1) src/utils/virtual-locations.ts(3)
src/utils/__tests__/{reveal-containing-directory.test.ts(6),virtual-locations.test.ts(14)}
src-tauri/src/baidu_login.rs(63) src-tauri/src/baidu_probe.rs(31)
src-tauri/src/dir_reader/read.rs(2) src-tauri/src/lib.rs(14)
src-tauri/src/remote/{baidu_api.rs(314),baidu_auth.rs(155),baidu_provider.rs(162),commands.rs(51),connection.rs(10),connection_test.rs(14),mod.rs(22),registry.rs(14)}
```

### 2.3 百度链路里**不可外推**的耦合（这是不能直接泛化 `baidu_login.rs` 的原因）

| 百度特有的东西 | 位置 | 泛化的代价 |
|---|---|---|
| cookie 名 `BDUSS` / `STOKEN` | `baidu_login.rs:62,64`、`baidu_api.rs:87,90` | 换 cookie 名即换语义 |
| `STOKEN` 的三态（Absent / Rejected / Present）与 86400 秒临期告警 | `baidu_auth.rs`（`BaiduStokenState` / `BAIDU_STOKEN_EXPIRY_WARN_SECONDS`） | 夸克无 STOKEN 概念 |
| 百度特有错误码 `errno:-6`（未登录） | `baidu_api.rs:611` | 夸克是 `code:31001` |
| 百度特有的 `bdstoken` 机制（`/api/gettemplatevariable`，写操作必需） | `baidu_api.rs:1005-1017` | 夸克写操作无此概念 |
| 百度特有的 `Referer: /disk/home` 依赖 | `baidu_api.rs:1011` | 夸克不需要 |
| 每轮登录换**独立 webview data-dir**（`LOGIN_PROFILE_ROOT = "baidu-login-profile"`，`session-<stamp_ms>`） | `baidu_login.rs` | 因为共享 jar 时 `delete_cookie` 实测无效——**这个教训夸克必须继承** |

**结论**：新建 `src-tauri/src/quark_login.rs` 作为**兄弟文件**，不泛化 `baidu_login.rs`。共享部分（窗口创建 / 轮询骨架）可在两次都上线后再抽，本次不抽——泛化会污染已上线的百度链路、放大回归面，并把百度的实测注释与 `cargo test --lib` 红线注释稀释掉。

---

## 3. 夸克网盘接口调研结论

### 3.1 调研方法与工具

**结论先行**：夸克客户端是 **Electron 应用**，API 逻辑全部在 `app.asar` 的 JS bundle 里，**不在原生二进制里**。因此本轮的主证据是**直接读取 JS bundle**，比反编译更高信度。

#### 3.1.1 本地安装目录

`D:\Program Files\Quark\`：`quark.exe`(15,021,104 B)、`quark_proxy.exe`、`build_info.json`、`debug.log`、`results.txt`、`unins000.*`、版本目录 `7.4.5.1027\` 与 `7.4.7.1036\`、`components\`、`Dictionaries\`、`PS\`、`Temp\`。版本目录内是完整 Electron 分发（`quark.dll`、`resources.dll`、`app_asar.dll`、`Resources\app.asar`、`Resources\quantum_app.asar`、`v8_context_snapshot.*` 等）。递归共 4011 个文件。

#### 3.1.2 asar 解析

自写只读解析器 `.superpowers/sdd/quark/asar-read.cjs`（解析 pickle 头，只读不写）：

- `app.asar` 共 **4509** 个条目，`dataStart = 1191808`
- 提取：`dist/renderer/index.js`（8,834,654 B，主 bundle，含网盘 API）、`dist/main/main.js`（5,884,882 B）、`dist/renderer/library-70d2b1.entry.js`（2,262,220 B）、`dist/renderer/common-e33bd8.entry.js`（3,821,197 B）、`dist/preloads/*.js`（10 个）、`package.json`

#### 3.1.3 REA MCP 的成功与失败（**精确记录**）

| # | 工具调用 | 结果 |
|---|---|---|
| 1 | `mcp__rea__binary_session` | `{"open":false,"analysis_provider_binding":null,"target_support":{"status":"unknown","reason":"No target is open."}}`；Ghidra provider `availability_code=not_configured`，reason「Set GHIDRA_INSTALL_DIR to an extracted Ghidra 12.1.x release directory.」；java 存在但 `version_unresolved`；`native_decompiler` check 失败「Matching native decompiler for win32/x64 was not found」 |
| 2 | `mcp__rea__open_binary` path=`D:\Program Files\Quark\7.4.5.1027\quark.exe` | ❌ **失败**：被本地硬安全策略拦截 ——「Local hard safety policy blocked mcp__rea__open_binary: protected Windows path "D:\Program Files\" cannot be written or deleted.」 |
| 3 | `mcp__rea__open_binary` path=`.superpowers\sdd\quark\quark-launcher.exe`（从 `D:\Program Files\Quark\quark.exe` 复制的副本） | ✅ **成功绑定**：`{"path":"...quark-launcher.exe","format":"pe","kind":"executable","sha256":"a461005270b34655c21fb763ec16fe77fac985f5a3a33eb6e9275ed590b3c0b4","architecture":"x86_64"}` |
| 4 | `mcp__rea__binary_overview` | ❌ **失败** `provider_unavailable`。四个候选 provider 全部不可用：`ghidra`=`not_configured`；`hopper`=`unsupported_host`（Windows 不支持）；`ida`=`not_configured`（需 `REA_IDA_MCP_CONFIG`）；`kuna`=`not_configured` **且** `target_format_unsupported`（kuna 只收 ELF 与 thin Mach-O，**明确不收 PE**） |
| 5 | `mcp__rea__open_binary` path=`.superpowers\sdd\quark\asar\dist\main\main.js` | ✅ **成功绑定**：`{"format":"javascript","sha256":"522f6fecea47d915511ee4a78596e6a171a564fce1d8932375e8dff3f8a6065f"}` |
| 6 | `mcp__rea__list_procedures`（对上面这个 JS） | ❌ **失败** `capability_unavailable` / `target_unsupported`：ghidra「Ghidra v1 imports executable targets, not artifact targets.」、hopper「cannot directly analyze artifact targets」、ida「admits executable inputs, not analysis databases」、kuna「decompiles executable images, not databases or archives」 |

**REA 总结**：**绑定成功，但没有任何 provider 能产出函数级分析**。PE 被 kuna 明确拒绝，JS artifact 被全部 provider 拒绝。因此本轮**没有任何反编译结论**——全部结论改用 asar JS 直读 + 字符串/上下文提取得出。原始记录见 `.superpowers/sdd/quark/rea-session-notes.txt`。

（补充：`quark.exe` 本身也只是 Electron 启动器壳，API 逻辑本来就不在原生码里，所以 REA 即使可用也提供不了比 JS bundle 更多的信息。）

### 3.2 A 层：本地客户端实证（最高信度——来自 `dist/renderer/index.js`）

#### 3.2.1 服务配置块（prod，offset ~4859974，模块 `873133`）

| 键 | 值 |
|---|---|
| `cookieHost` | `"quark.cn"` |
| `cloudPanUrl` | `"https://pan.quark.cn"` |
| `cloudDriveUrl` | `"https://drive-pc.quark.cn"` |
| `coralSignInUrl` | `"https://coral3.quark.cn"` |
| `cloudDriveVipUrl` | `"https://drive.quark.cn"` |
| `cloudAlbumUrl` | `"https://api-album.quark.cn"` |
| `scanLoginPage` | `"https://su.quark.cn/4_eMHBJ"` |
| `uopAccessUrl` | `"https://access-open.quark.cn"` |
| `uopAccessSalt` | `"cti30uk26JyjgDQI"` |
| `utoken` | `"https://utoken2.uc.cn"` |
| `openApiUrl` | `"https://open-api-drive.quark.cn"` |
| `client_id` | `"533"` |
| `secureNo` | `14100` |

#### 3.2.2 列表 endpoint（`index.js:5312702` 附近，模块 `sortableList`）

```
GET https://drive-pc.quark.cn/1/clouddrive/file/sort
```

| 参数 | 值 / 说明 |
|---|---|
| `pdir_fid` | 父目录 fid，**根目录为 `0`** |
| `_page` | 页码，默认 1 |
| `_size` | 页大小，默认 100 |
| `_fetch_total` | 是否取总数 |
| `_fetch_sub_dirs` | 是否取子目录数 |
| `fetch_dir_file_num` | 目录文件数 |
| `_sort` | 逗号拼接的排序串（开源项目侧示例：`size:desc,file_type:asc` / `file_name:asc,updated_at:desc` / `updated_at:desc,file_type:asc`） |

响应：`.data`，即 `{ list: [...], metadata: {...} }`（`index.js:5339978` 处确认 `t?.data?.list.length` 与 `metadata: t.metadata`）。

**另一处同款响应形状（`{list, metadata}`）的调用**（`index.js:5339978`）—— 其 endpoint 实为 `/1/clouddrive/share/sharepage/detail`（分享页详情），**不是**根目录列表：

```
params: { pwd_id, stoken, pdir_fid: "0", force: 0, _page: 1, _size: 1,
          _fetch_banner: 1, _fetch_share: 1, _fetch_total: 1,
          fetch_update_flag: 1, support_visit_limit_private_share: true }
headers: { "Content-Type": "application/json" }
```

#### 3.2.3 请求层（**决定我们能不能脱离浏览器直接请求**）

**`driveRequest`**（`index.js:5313953`，`t.request =`）—— axios 实例：

| 项 | 值 |
|---|---|
| 实例级 `params` | `pr: ucparams.pr`, `fr: ucparams.fr`, `sys: process.platform`, `ve: getQuarkPCVersion()`, `uc_param_str: "dsdnfrpfbivesscpgimibtbmnijblauputogpintnwktprchmt"` |
| `timeout` | `1e4`（10 秒） |
| `withCredentials` | `true` |
| **request 拦截器** | **该实例确有**：`e.params = { uc_param_str: UC_PARAMS_STR, device_model: getDeviceNameSync(), ...e.params, ...getUcParams(), ve }` ⇒ `file/sort` **也**带 `device_model`（来自 `getDeviceNameSync()`，即本机主机名）与 `ut`（来自 `window.QUARK_WSGUID`，模块 `514962`） |

**`fileRequest`**（`index.js:6047616`，`t.fileRequest =`）—— axios 实例：

| 项 | 值 |
|---|---|
| 实例级 `params` | `{ ...driveBaseParams }`（`driveBaseParams = { ...baseParams }`，模块 `731675`） |
| `timeout` | `1e4` |
| `withCredentials` | `true` |
| request 拦截器 | 注入 `params = { uc_param_str: UC_PARAMS_STR, device_model: getDeviceNameSync(), ...e.params, ...getUcParams(), ve }` |
| response 拦截器 | **`e.data.code !== 0` ⇒ reject** |

**常量与来源**：

| 符号 | 值 | 位置 |
|---|---|---|
| `ucparams.fr` | `darwin ? "mac" : "win"` → **Windows 客户端 ⇒ `fr=win`** | 模块 `780721` |
| `ucparams.pr` | `"ucpro"` | 模块 `780721` |
| `UC_PARAMS_STR` | `"dsdnfrpfbivesscpgimibtbmnijblauputogpintnwktprchmt"`（固定串） | 模块 `860782` |
| `getUcParams()` | `window.QUARK_WSGUID` → `ut`；`window.QUARK_VERSION` → `ve` | 模块 `514962` |
| `getQuarkPCVersion()` | 从 UA 匹配 `QuarkPC/(\d+\.\d+\.\d+\.\d+)`；`appVersion` 从 UA 匹配 `quark-cloud-drive/([\d.]+)` | 模块 `407716` |

> **⭐ 关键实证：`file/sort` 路径上没有 `sign` / `x-*` / `sec-*` 签名参数。** 本地客户端就是 `withCredentials` + 固定 query 参数 + Cookie，**不需要自造签名**。这是夸克相对百度最大的实施优势。
>
> ⚠️ 表述边界：`index.js` @6031405 处**确有** `X-U-Content-Encoding: wg` + body 加密，但**不在 `file/sort` 路径上**。引用本条结论时**必须**带上「`file/sort` 路径上」这个限定。

#### 3.2.4 错误码表（模块 `887411` `ERROR_TIPS`，`index.js:6047616` 处重复一份）

| 码 | 含义 | 码 | 含义 |
|---|---|---|---|
| `14001`-`14010` | 网络 / 数据异常 | `23010` | 上传失败，请稍后重试 |
| `14012` | 文件名非法 | `24005` | **文件/文件夹状态已更新，请返回重试** |
| `14013` | 文件不存在 | `25004` | 文件总大小超限 |
| `14014` | 违规 | `25005` | 解析失败 |
| `21001` | 云端已不存在 | **`31001`** | **「请重新登录」← 会话失效信号** |
| `22001` | 含违规信息 | `32003` | 空间不足 |
| `23001` | 上传失败，请稍后重试 | `32004` | 文件大小超限 |
| `23005` | 下载失败，请稍后重试 | `32005` | 离线下载次数超限 |
| `23008` | 重命名失败，存在同名文件 | `43004` | 本地时间差异较大 |
| `23018` | 文件不支持下载 | `25008` | 文件已失效 |
| `38001` | 文件不支持下载 | — | — |

> **收录范围**：A 层的 `ERROR_TIPS` 在 `index.js` 里有**两份副本**（模块 `887411` @6041787，另一份 @6030862）。上表按**两份副本的并集**收录 —— 第二份多出 `23018` / `25008` / `38001` 三个码（原表漏记）。`24005` 的 A 层原文是「**文件/文件夹状态已更新，请返回重试**」，**不含**「隐私空间锁定」字样。
>
> **会话失效码**：`31001` 与 **`31004`** 同等对待 —— 客户端的响应处理是 `if ([31001, 31004].includes(r.code) && ...user.logout())` ⇒ **`31004` 也是会话失效码**，会触发登出。

#### 3.2.5 下载语义

- `download_url` 由 `/1/clouddrive/file/download`（`index.js:5303154`）及 task 体系取得。
- 下载 URL 带 `auth_key=<unix秒>` 或 `Expires=<unix秒>`；`checkDownloadUrlExpired` 判定条件是 `Date.now() >= 1000 * (exp - 1800)` —— **提前 30 分钟**判过期。
- 下载通道**显式注入 Cookie** header（`index.js:6557148` 附近：`T.headers?.set("Cookie", D)`，`D = await getCookies()`，走 IPC `sys.cookies`）。
- 下载 CDN 域名：`dl-c-sz-u.drive.quark.cn`、`dl-c-zb-u.drive.quark.cn`。

#### 3.2.6 文件条目字段（`index.js:235411` 附近）

`fid`、`pdir_fid`、`name` / `file_name`、`size`（另有 `file_size` 出现在 `index.js:238295`）、`format_type`、`file_type`、`file_source`、`download_url`、`thumbnail`、`name_space`、`share_fid_token`、`update_flag`、`dir_init_lock`、`updated_at`。

#### 3.2.7 Cookie 处理

- renderer **从未按名字**引用 `__pus` / `__puus`（uniq 搜索 `__pu[a-z]*` 只命中 `__pulse` / `__purple` 两个无关标识符）→ 客户端是「整罐 cookie + `withCredentials`」模型，**不按名字挑**。
- `cookieHost: "quark.cn"` ⇒ cookie 下发在 `.quark.cn` 域上。
- `getCookies` 走主进程 IPC `sys.cookies`（`main.js`：`cookies.get(`@2421704`、`cookies.set(`@2643660、`session.defaultSession`@2913850）。

#### 3.2.8 已实证的写操作 endpoint（v1.1 可用）

| 操作 | Method + Path | Body / Params |
|---|---|---|
| 建文件夹 | `POST /1/clouddrive/file`（`index.js:5312358`） | `{ pdir_fid, file_name, dir_path, dir_init_lock }` |
| 重命名 | `POST /1/clouddrive/file/rename`（`index.js:5310993`） | — |
| 删除 | `POST /1/clouddrive/file/delete`（`index.js:5374934`）/ `POST /1/clouddrive/series/delete`（`index.js:7079643`） | — |
| 回收站 | `GET/POST /1/clouddrive/file/recycle/list\|recover\|remove` | — |
| 上传 | `POST /1/clouddrive/file/upload/pre\|auth\|finish`、`/1/clouddrive/file/formupload/pre`、`/1/clouddrive/file/update/hash` | — |
| 分享 | `/1/clouddrive/share/sharepage/detail\|save\|token`、`/1/clouddrive/share/sharepage/dir`、`/1/clouddrive/share` | — |
| 分类 | `GET /1/clouddrive/file/category` | — |
| 成员 | `GET /1/clouddrive/member`（`index.js:7498041`） | — |
| 其它 | `GET /1/clouddrive/auth/pc/flush`、`GET /1/clouddrive/file/info`、`GET /1/clouddrive/file/info/list`、`GET /1/clouddrive/folder/list`、`GET /1/clouddrive/folder/update`、`POST /2/clouddrive/file/advanced_sort` | — |

完整 163 条 endpoint 清单见 `.superpowers/sdd/quark/out-clouddrive-paths.txt`。

#### 3.2.9 本地**没有**的东西

- **没有 `refresh_token` 的任何迹象**：本地 JS 全量搜索 `refresh_token` 无命中。
- **没有自造签名**：无 `sign` / `x-*` / `sec-*` 参数。
- **没有按名字取 cookie**：renderer 侧从不用 `__pus` 字面量。

### 3.3 B 层：开源项目佐证（外部来源，与 A 层互为交叉验证）

| 来源 | 结论 |
|---|---|
| `https://github.com/lala123963/lingk5/blob/main/夸克网盘.py`、`https://github.com/SingTsang/Auto_Check_In/blob/main/checkIn_Quark.py` | 抓包流程明确写：「浏览器访问 `https://pan.quark.cn/` 并登录 → F12 Network 找以 `sort` 开头的请求，即 `url=https://drive-pc.quark.cn/1/clouddrive/file/sort` → 复制全部 cookie」；query 用 `{"pr":"ucpro","fr":"pc","uc_param_str":""}`；账号信息用 `GET https://pan.quark.cn/account/info?fr=pc&platform=pc` → `data.nickname` |
| `https://docs.hunhepan.com/reman/pantools.html` | 「浏览器打开 `https://pan.quark.cn/`，登录后 F12 刷新，Network 找 `https://drive-pc.quark.cn` 开头的任意链接，复制 Request Headers 里的 cookie。**实际上，只需要名为 `__pus` 的 cookie 即可**」 |
| `https://greasyfork.org/ug/scripts/571335-夸克网盘空间占用分析自用版`、`/zh-TW/scripts/504333-夸克网盘批量工具集` | `@match https://pan.quark.cn/*`；`SORT_PARAMS = {size:'size:desc,file_type:asc', name:'file_name:asc,updated_at:desc', time:'updated_at:desc,file_type:asc'}`；`PAGE_SIZE=100`；条目用 `item['fid']` / `item['file_name']` |
| `https://developer.cloud.tencent.com/article/2643818` | 夸克网盘 API 封装（列表/搜索/重命名/上传/分享/转存），依赖有效 cookie |
| `https://www.hqwc.cn/a/1360320.html` | quark-auto-save 使用指南：cookie 失效需重新登录 `pan.quark.cn` |

**B 层与 A 层的交叉验证结果**：

| 事项 | A 层（本地） | B 层（开源） | 判定 |
|---|---|---|---|
| 列表 endpoint | ✅ `/1/clouddrive/file/sort` | ✅ 同 | **双侧一致，高信度** |
| 列表 host | ✅ `drive-pc.quark.cn` | ✅ 同 | **双侧一致** |
| `pr` | ✅ `ucpro` | ✅ 同 | **双侧一致** |
| `fr` | ✅ `win`（本地客户端） | `pc`（脚本） | **冲突 → 见 §7-U5** |
| `uc_param_str` | ✅ 固定长串 | 空串也可用 | **A 层更全，以 A 层为准** |
| 登录入口 | `pan.quark.cn` / `scanLoginPage` | `pan.quark.cn` | **双侧一致** |
| 账号昵称 | — | ✅ `GET /pan.quark.cn/account/info` → `data.nickname` | 仅 B 层 |
| `__pus` | ❌ 本地未按名引用 | ✅ 社区称「只需 `__pus`」 | **仅 B 层 → §7-U1** |
| 排序串 | 支持 `_sort` | 给出具体格式 | 双侧一致 |

### 3.4 C 层：假设待验（**不得当作已知结论使用**）

| # | 假设 | 为什么没坐实 | 验证方式 |
|---|---|---|---|
| U1 | 会话 cookie 名是 `__pus` | 本地 renderer 从不按名字取 cookie；只有开源项目侧说法 | T0 真机实测 |
| U2 | 夸克有 / 没有 `refresh_token` | 本地 JS 全量搜索无命中，只能说「客户端不用」，不能说「服务端没有」 | T0 顺带确认 |
| U3 | 登录页精确 URL | `scanLoginPage` 是**扫码页**、`pan.quark.cn` 是网盘首页；客户端真正走哪个入口未定 | T0 真机 |
| U4 | 服务端是否接受**非浏览器来源**的 Cookie 请求（缺 `device_model` / `ut` / `QUARK_WSGUID`） | 本地客户端有 `device_model` 注入，但**无法证明它是否必需** | **T0 头号风险** |
| U5 | `fr=win` vs `fr=pc` | 本地客户端用 `win`，开源脚本用 `pc`，**两者都被广泛使用** | T0 A/B 对照 |
| U6 | `ut` / `QUARK_WSGUID` 是否必需 | 同 U4 | T0 |
| U7 | 是否有风控 / 验证码 / 频率限制 | 客户端里有 `coralSignInUrl` / `secureNo` 等风控相关配置，但没找到触发条件 | T0 + 真机 |
| U8 | `size` 字段单位（字节 / KB） | 本地 bundle 只做展示，未见单位换算的权威定义 | T0 用已知大小文件核对 |
| U9 | `_size` 的服务端上限（客户端用 100） | 未见截断告警 | T0 请求 `_size=1000` 看返回条数 |
| U10 | 下载是否需要额外 header（`Referer` / `Origin`） | 客户端只显式注入了 Cookie | T0 真机下载 |
| U11 | cookie 是否必须覆盖整个 `.quark.cn` 域 | 客户端用整罐；社区说 `__pus` 单个就够 | T0 分组对照 |

### 3.5 证据强度总表

| 事项 | 强度 | 来源 |
|---|---|---|
| 列表 endpoint + 响应形状 | **确定** | A + B 双侧一致 |
| 请求参数集合（`pr`/`fr`/`sys`/`ve`/`uc_param_str`） | **确定** | A 层实证 |
| 无自造签名 | **确定** | A 层实证（response 拦截器只判 `code`） |
| 会话失效信号 `code:31001` | **确定** | A 层错误码表 |
| 下载 URL 过期语义（提前 30 分钟） | **确定** | A 层 `checkDownloadUrlExpired` |
| 写操作 endpoint | **确定（存在性）** / 未知（语义与冲突码处理） | A 层实证 |
| 账号昵称接口 | **高** | B 层 |
| cookie 名 `__pus` | **中** | 仅 B 层 |
| 服务端接受非浏览器请求 | **未知** | C 层 U4 —— **头号风险** |
| `fr` 取值 | **未知** | C 层 U5 |

---

## 4. 方案设计

### 4.1 架构决策

**D1 —— 用 `RemoteProtocol` 新变体 `QuarkPan`，不新建子系统。**

`sfm://remote/` 这条链（目录树根、地址栏、面包屑、双屏、右键菜单、传输引擎、比较引擎）已经全部接线并经过真机验收。只要多认一种协议，它就自然出现在侧边栏「远程位置」。`provider_with_protocol`（`commands.rs`）的 `match` 是穷尽的，加变体后**编译器会在所有需要改的地方报错**——改动清单由编译器给，不靠人工找。

**D2 —— 认证走「内嵌 WebView2 登录 + cookie 直连会话」，不手抄 cookie，不引入 refresh_token。**

理由见 §1.3。用户不需要开 F12 复制 cookie（那是 `docs.hunhepan.com` 这类工具的做法），体验与百度网盘一致。

**D3 —— 新建 `src-tauri/src/quark_login.rs` 兄弟文件，不泛化 `baidu_login.rs`。**

理由见 §2.3。

**D4 —— 登录窗必须在独立 `std::thread` 里建。**

`tauri-runtime-wry` 的 `create_webview` 原文注释：**must be called from a separate thread, otherwise the channel will introduce a deadlock**。命令从 WebView2 IPC 回调进来 = 事件循环线程，直接建窗会死锁 ⇒ **登录窗永远白板**。这是百度踩过的坑，夸克必须照抄 `std::thread::spawn` 建窗。

**D5 —— 照抄百度的三道保险。**

① 建窗后清 jar 里的旧 cookie（`delete_cookie`）；② Ready 必须满足「当前 cookie ≠ 基线 cookie」；③ 关窗前对服务端验活，只在拿到**确凿死亡证据**时拒绝。每轮登录换**独立 webview data-dir**（因为共享 jar 时 `delete_cookie` 实测无效）。夸克的「当前 cookie」判定从百度的 `BDUSS` 换成 `__pus`（若 U1 不成立则退化为「整罐 cookie 的指纹变化」）。

**D6 —— v1 只做只读；`write_file` / `set_permissions` 明确返回 `RemoteError::unsupported`，绝不静默成功。**

`provider.rs` 顶部已写死这条规矩。静默成功会让用户的文件凭空消失且没有任何提示。`get_permissions` 返回 `Ok(None)`（协议无权限位），不是 `Err`。

**D7 —— 不往 `RemoteConnection` 加新字段。**

- `host` 复用为固定值 `drive-pc.quark.cn`（不可编辑）
- `port` 复用为 `443`
- `username` 复用为授权后回填的账号昵称（只读展示）
- `credential_key` = 连接 id（指向 DPAPI 密文）
- 其余全在密文档里

理由：加字段要同时改读白名单与写列表，AGENTS.md 已记着「新建收藏分类 → 重启即丢」那次事故。

**D8 —— 不新增 `RemoteErrorKind`。**

全部失败映射到既有 11 个 kind（`error.rs:17-46`）。

**D9 —— 凭据绝不明文落盘。**

会话 blob（cookie 明文）只存在于内存与 DPAPI 密文里，**绝不**进 `user-remote-connections.json`，**绝不**进日志，**绝不**进错误信息。`QuarkSession` 照抄 `BaiduSession` 的手写 `Debug`（只打印长度不打印明文）。

### 4.2 数据模型改动

**后端** `src-tauri/src/remote/connection.rs`：

```rust
pub enum RemoteProtocol {
    Ftp,
    FtpsExplicit,
    FtpsImplicit,
    Sftp,
    BaiduPan,
    // 新增
    QuarkPan,
}
```

- `as_str()` → `"quark_pan"`
- `default_port()` → `443`

**前端** `src/types/remote.ts`：

```ts
export type RemoteProtocol =
  | 'ftp' | 'ftps_explicit' | 'ftps_implicit' | 'sftp' | 'baidu_pan' | 'quark_pan';
```

新增 `isQuarkPanProtocol(p)` 与**统一的** `isNetdiskProtocol(p)`（= 百度 || 夸克）。所有「网盘类」分流（`sections.ts` / `virtual-locations.ts` / `remote-location-refs.ts` / `remote-nav-panel.vue`）一律改用 `isNetdiskProtocol`，**不新增第二套并行判定口径**。

### 4.3 后端文件清单

| 文件 | 动作 | 规模 | 要点 |
|---|---|---|---|
| `src-tauri/src/remote/quark_api.rs` | 新增 | ≈600 行 | 纯 HTTP + 纯函数解析层，**零 `tauri::` 路径** |
| `src-tauri/src/remote/quark_auth.rs` | 新增 | ≈350 行 | 会话编解码 / 状态 / DPAPI 存取，**零 `tauri::` 路径** |
| `src-tauri/src/remote/quark_provider.rs` | 新增 | ≈700 行 | `impl RemoteProvider for QuarkRemoteProvider` |
| `src-tauri/src/quark_login.rs` | 新增 | ≈700 行 | 内嵌 WebView2 登录窗，**独立 `std::thread` 建窗** |
| `src-tauri/src/remote/mod.rs` | 改 | +8 行 | 夸克侧 3 个 `mod` + 2 段 `pub use`（**参照系**：百度侧是 **4 段** `pub use`，不是 3 段） |
| `src-tauri/src/remote/connection.rs` | 改 | +3 行 | 变体、`as_str`、`default_port` |
| `src-tauri/src/remote/registry.rs` | 改 | +4 行 | `QuarkPan` 不走 password 路径 |
| `src-tauri/src/remote/commands.rs` | 改 | +60 行 | 3 个会话命令 + 4 个 login 命令 + `provider_with_protocol` 三处分支 + 命令注册自检清单 |
| `src-tauri/src/lib.rs` | 改 | +8 行 | `mod quark_login;` + `generate_handler!` 注册 7 个命令 |

> **`quark_probe.rs` 不在本清单里**：它是 **T0 之后**才评估的可选项 —— 仅在主路线成立、且确有常驻探针需求时才决定要不要新增。一个 env-gated 常驻探针对最终用户没有价值；而 T0 阶段就往产品代码树塞模块，一旦 T0 结论是「走退路」，这段代码要么白写、要么成为长期负担。T0 一律用**外部只读探针**（`curl` / `Invoke-WebRequest`），见 §5 阶段 0。

### 4.4 前端文件清单

**集成模型：网盘 provider 选择器**（本轮拍板的实现方向；理由见下方「为什么选 provider 选择器」）。

| 文件 | 动作 | 要点 |
|---|---|---|
| `src/types/remote.ts` | 改 | 协议联合类型 + `isQuarkPanProtocol` + `isNetdiskProtocol` + 3 份夸克载荷镜像类型 |
| `src/modules/settings/ui/categories/remote/connections-section.vue` | 改 | **网盘分区的 provider 选择器**（「添加」时二选一：百度网盘 / 夸克网盘）；分区判定改用 `isNetdiskProtocol`；`remoteInitialProtocol` 的回落要同时挡住 `quark_pan` |
| `src/modules/settings/ui/categories/remote/index.vue` | 改 | 父级分类靠分区结果决定「信任的主机」是否显示 → 同口径改判定 |
| `src/modules/settings/ui/categories/remote/connection-form.vue` | 改 | **网盘分支按 provider 分流**（百度走现有 `baidu_pan` 分支，夸克走 `quark_pan` 分支）；夸克分支**隐藏** host/port/密码框，改显示授权卡。**协议下拉不动**（`:63-69` 刻意不含网盘协议） |
| `src/stores/storage/remote-connections.ts` | 改 | 固定 host/port、保存分派、测试连接、授权状态、登录 start/cancel |
| `src/modules/settings/ui/categories/remote/sections.ts` | 改 | `isBaiduPanProtocol()` → `isNetdiskProtocol()` |
| `src/modules/nav-sidebar/components/remote-nav-panel.vue` | 改 | 「网盘」/「远程」分区过滤改用统一判定；`sections` 里网盘分区原先单一的 `protocol: "baidu_pan"` 字段改为**可容纳多 provider 的结构**；新建预置协议 |
| `src/utils/virtual-locations.ts` | 改 | `location_group` 改用统一判定 |
| `src/utils/remote-location-refs.ts` | 改 | 同口径 |
| `src/localization/messages/*.json`（**17 个**） | 改 | 各加 **1 个约 15 键的嵌套对象** + **2 个平铺键**（`protocols.quark_pan` / `quarkLoginRequired` / `quark` 对象）；中文 locale 是 `ch` |
| `src/modules/settings/ui/categories/remote/quark-connection-form.test.ts` | 新增 | 夸克表单测试（对应百度的 `baidu-connection-form.test.ts`） |
| `src/stores/storage/__tests__/remote-connections-quark.test.ts` | 新增 | 夸克存储测试 |
| `src/types/__tests__/remote-protocol-quark.test.ts` | 新增 | 协议判定测试 |

#### 为什么选 provider 选择器，而不是协议下拉

1. **协议下拉里没有网盘协议，而且这是刻意的**：`connection-form.vue:63-69` 的注释原文写着「**不含百度网盘**：网盘有自己的标签页与专属的录入项，把它塞进这个下拉等于给用户一条『选了却填不了对应字段』的死路」，`PROTOCOLS` 只有 `ftp` / `ftps_explicit` / `ftps_implicit` / `sftp`。夸克网盘同样没有 host / port / 密码可填 ⇒ 塞进去就正是那条死路。
2. **网盘协议是硬编码的，不由表单决定**：`connection-form.vue:230-232` 的 `props.variant === "netdisk" ? "baidu_pan" : ...` 说明「选哪个网盘协议」这件事发生在**上游分区层**，不在表单里。
3. **网盘分区本来就是为多网盘协议预留的扩展位**：`connections-section.vue:32` 的原话是「判定口径只有一个 —— `isBaiduPanProtocol`。以后接入阿里云盘之类的网盘，只要把判定扩进去，这一页的分区自动跟上」⇒ 「选哪个网盘」天然属于分区层的职责，做成 provider 选择器与仓库既有结构一致。

⇒ 改动面收敛在五个文件内：`connections-section.vue` + `remote/index.vue` + `sections.ts` + `remote-nav-panel.vue` + `connection-form.vue`。

#### 4.4.1 已否决的方案：把网盘协议加进「协议下拉」

> **已否决**（评审 B1）。原方案写的是「协议下拉加项 → 下拉选夸克网盘」，**不得**按此实现：仓库里 `PROTOCOLS` **刻意不含**网盘协议（`connection-form.vue:66-67` 的注释明确反对），网盘协议由 `connection-form.vue:230-232` 的 `variant === "netdisk"` **硬编码**。照原方案实施会做出一个「选了却填不了对应字段」的死路。本轮改用上面的 **provider 选择器**。

**不需要改**（明确写出来，防下一轮白跑）：

- `src-tauri/src/remote/{provider.rs, path.rs, dir_adapter.rs, credentials.rs, error.rs, transfer.rs}` —— 协议无关
- `src-tauri/capabilities/default.json` —— 登录窗由 Rust 侧建，所需 capability 已具备
- `src-tauri/src/remote/connection_test.rs` 的结构 —— 夸克不握手，不复用 `ProbeOutcome`
- `src/modules/tab-bar/utils/remote-tab-labels.ts` —— 全文 60 行，**不含** `baidu` / `protocol` / `netdisk` 字样，只吃 `connectionName` ⇒ **协议无关，不需要改**
- `src-shared/remote-connection.sample.json` —— 字段没变
- `user-remote-connections.json` 的顶层键与读/写白名单 —— 字段没变

### 4.5 认证链路时序

```
[设置页] 网盘分区选 provider「夸克网盘」→ 点「添加」
   ↓
[前端] invoke remote_quark_login_start
   ↓
[Rust] std::thread::spawn → 在独立线程建 WebviewWindow("afm-quark-login")
   │  ├─ 保险一：建窗后清 jar 里的旧 __pus（清不掉就换独立 data-dir）
   │  ├─ 记录基线 cookie 指纹
   │  └─ 加载 <登录起始页>（**以 U3 的 T0 结论为准**；候选：`https://pan.quark.cn/`、配置块里的 `scanLoginPage = https://su.quark.cn/4_eMHBJ`）
   ↓
[用户] 在内嵌窗口扫码 / 手机号+密码登录
   ↓
[窗口] 导航到登录页之后，cookie jar 出现 __pus
   │  └─ 保险二：Ready 条件 = 当前 cookie 指纹 ≠ 基线 cookie 指纹
   ↓
[Rust] 读 cookie → 关窗 → 保险三：对服务端验活（GET /1/clouddrive/file/sort?pdir_fid=0&_size=1）
   │  └─ 只有拿到确凿死亡证据（code=31001 / 明确鉴权失败）才拒绝
   ↓
[前端] 轮询 remote_quark_login_pending → 拿到 pending session
   ↓
[前端] 保存连接 → invoke remote_quark_login_claim（claim-on-save）
   ↓
[Rust] quark_auth::encode_session → credentials::save_credential（DPAPI）
   │  └─ 键 = credential_key = 连接 id
   ↓
[Rust] GET https://pan.quark.cn/account/info?fr=pc&platform=pc → data.nickname
   │  └─ 回填 RemoteConnection.username（只读展示）
   ↓
[前端] 授权卡显示「已授权：<昵称>」
```

### 4.6 API 层契约（`quark_api.rs`）

```rust
pub const QUARK_PAN_HOST: &str = "pan.quark.cn";
pub const QUARK_DRIVE_BASE_URL: &str = "https://drive-pc.quark.cn";
pub const QUARK_CLIENT_ID: &str = "533";
pub const QUARK_PR: &str = "ucpro";
pub const QUARK_UC_PARAM_STR: &str =
    "dsdnfrpfbivesscpgimibtbmnijblauputogpintnwktprchmt";
pub const QUARK_LIST_PAGE_SIZE: u32 = 100;
pub const QUARK_SESSION_COOKIE_PUS: &str = "__pus"; // ⚠️ 仅开源项目侧证据
```

**列表请求**（形态已实证）：

```
GET https://drive-pc.quark.cn/1/clouddrive/file/sort
    ?pdir_fid=0
    &_page=1&_size=100
    &_fetch_total=1&_fetch_sub_dirs=0
    &_sort=file_name:asc,updated_at:desc
    &pr=ucpro&fr=win&sys=win32&ve={UA 版本}&uc_param_str={固定串}
    &device_model={本机主机名}&ut={QUARK_WSGUID}
Cookie: <.quark.cn 域整罐>
```

> **`device_model` 与 `ut` 不能省**（评审 M1）：真实客户端的 `driveRequest` **确有** request 拦截器，注入 `device_model`（来自 `getDeviceNameSync()`，即**本机主机名**）与 `ut`（来自 `window.QUARK_WSGUID`）。若按旧规范少这两个参数去 T0 实测，**被拒会被误读成「路线不成立」**。
>
> ⚠️ `device_model` 会把**本机主机名**带出 ⇒ 属敏感项，落盘与日志策略见 §6.4。

**响应**（形态已实证）：

```jsonc
{
  "code": 0,                       // code !== 0 ⇒ 失败
  "data": {
    "list": [
      {
        "fid": "…", "pdir_fid": "0",
        "file_name": "…", "name": "…",
        "size": 12345,             // ⚠️ 单位待验 (U8)
        "file_type": 0, "format_type": 0,
        "updated_at": 1700000000,  // ⚠️ 秒 or 毫秒待验
        "thumbnail": "…", "download_url": "…",
        "share_fid_token": "…", "name_space": "…"
      }
    ],
    "metadata": { /* … */ }
  }
}
```

**下载**（形态已实证）：

```
GET https://drive-pc.quark.cn/1/clouddrive/file/download?fid={fid}
  → 返回带 auth_key / Expires 的 download_url
  → 直接 GET 该 URL 下载（客户端显式带 Cookie header）
  → 过期判定：Date.now() >= 1000 * (exp - 1800)  ⇒ 提前 30 分钟刷新
```

**分页**：夸克有 `_fetch_total` / `metadata`，可拿到总数；判据仍按百度同款「本页收到的条数 < `_size` ⇒ 取完」，并保留「恰好收满时多发一次空请求」的刻意行为（宁可多一次请求，不可漏条目）。

**超时**（三级，照抄百度）：

| 常量 | 值 | 理由 |
|---|---|---|
| `QUARK_OPERATION_TIMEOUT` | 30 s | 单次 HTTP 请求（本地客户端用 10 s，但它有长连接；Rust 侧保守放宽） |
| `QUARK_CONNECT_TIMEOUT` | 30 s | 建立下载连接 |
| `QUARK_FILE_TIMEOUT` | 900 s | 整份文件进内存 |
| `QUARK_DIRECTORY_TIMEOUT` | 120 s | 整个目录翻页 |
| `QUARK_MAX_DOWNLOAD_BYTES` | 500 MiB | 人为闸；夸克下载 URL 30 分钟过期，超大文件风险更高，阈值应**不高于**百度 |

### 4.7 错误映射表（**穷尽**，落到既有 11 个 kind）

| 夸克 `code` / 现象 | 语义 | `RemoteErrorKind` | 依据 |
|---|---|---|---|
| `31001` | 请重新登录 | **`Auth`** | A 层错误码表 |
| `31004` | 会话失效（与 `31001` 同等，触发登出） | **`Auth`** | A 层响应处理 `if ([31001, 31004].includes(r.code) && ...user.logout())` |
| HTTP 401 / 403 | 鉴权失败 | `Auth` | 通用 |
| HTTP 3xx（跳登录页） | 会话失效 | `Auth` | 通用 |
| `14013` / `21001` | 文件不存在 / 云端已不存在 | `NotFound` | A 层 |
| `14014` / `22001` | 违规 / 含违规信息 | `Permission` | A 层 |
| `14012` | 文件名非法 | `Protocol` | A 层 |
| `23008` | 重命名同名冲突 | `Protocol` | A 层（同百度的「已存在」归 Protocol） |
| `24005` | **文件/文件夹状态已更新，请返回重试** | `Protocol` | A 层原文（**不含**「隐私空间锁定」） |
| `23018` | 文件不支持下载 | `Unsupported` | A 层第二份副本 |
| `25008` | 文件已失效 | `NotFound` | A 层第二份副本 |
| `38001` | 文件不支持下载 | `Unsupported` | A 层第二份副本 |
| `43004` | 本地时间差异大 | `Protocol` | A 层 |
| `25004` / `32004` / `32003` / `32005` | 体积/空间/次数超限 | `Unsupported` | A 层 |
| `14001`-`14010` | 网络 / 数据异常 | `Unreachable` | A 层 |
| `25005` | 解析失败 | `Protocol` | A 层 |
| 响应非合法 JSON / 缺 `code` | 协议改版 | `Protocol` | 通用 |
| 其余未列出的 `code` | 兜底 | `Unknown` | — |
| 连接超时 / DNS 失败 | 网络不可达 | `Unreachable` | 通用 |

> **映射表必须穷尽**：漏一个变体就会让某类失败退化成「笼统错误」。照抄 `baidu_api.rs:589` 的 `classify_baidu_failure` 写法，每条都标注证据强度。
>
> **`Unknown` 兜底必须携带原始 code 便于排障**：落到 `Unknown` 的那条错误信息里要带上夸克原始 `code` —— 否则用户只看到「未知错误」，开发侧也无从定位是哪个码。

### 4.8 风险表

| # | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| **R1** | **服务端拒绝非浏览器来源的 Cookie 请求**（缺 `device_model` / `ut` / `QUARK_WSGUID`） | **低～中** | **整条路线作废** | **T0 必须先做**：用真机登录拿到的 cookie 直接打 `file/sort`，逐个补齐 `device_model` / `ut` / `ve` 看哪个必需。**若全部必需**：退化为「登录窗内执行 `fetch` 并把 JSON 结果回传 Rust」（即在 WebView2 里跑请求，Rust 只做解析与落盘）——这条退路仍可用，因为 `file/sort` 是公开的 GET 接口。**概率已下调为「低～中」**（原估偏高）：B 层 `https://docs.hunhepan.com/reman/pantools.html` 的工具是**原生 Windows（Go）程序**用**单个 cookie 直连**，构成「非浏览器来源也能工作」的可工作反例 |
| R2 | 风控 / 验证码 / 频率限制 | 中 | 频繁 401 或验证码页 | 退路同上；错误映射里单列「协议已变更 / 风控」文案 |
| R3 | `fr=win` 与 `fr=pc` 都不被接受 | 低中 | 列表请求全失败 | T0 做 A/B 对照，取被接受的那个；两个都失败则按 R1 退路处理 |
| R4 | `_size` 服务端静默截断 | 中 | **目录漏条目且不报错** | 照抄百度 `clamp_page_limit`：**必须夹** `_size`；并用 `_fetch_total` 交叉校验总数 |
| R5 | `size` / `updated_at` 单位猜错 | 中 | 大小显示错误 / 排序错乱 | T0 用已知大小文件核对；时间单位错了会让「按修改时间排序」变成随机顺序且**不会报错**——照抄 `baidu_seconds_to_unix_millis` 的显式换算函数 |
| R6 | 下载 URL 30 分钟过期 | 中 | 下载中途 403 | 下载前取新 URL；`read_file` 一次性拿链接即用；超过 `QUARK_FILE_TIMEOUT` 明确报错 |
| R7 | 下载需要额外 header（`Referer` / `Origin`） | 中 | 下载 403 | T0 真机下载验证；必要时带 `Referer: https://pan.quark.cn/` |
| R8 | `__pus` 不是唯一必需 cookie | 中 | 登录成功但列表 401 | 保险三的验活用**真实列表请求**而不是「只看 cookie 在不在」——这样能在关窗前就发现「cookie 不够」 |
| R9 | 夸克接口改版 | 中 | 功能失效 | 解析层对缺字段 / 非 JSON 明确报 `Protocol`；整体分支可丢弃 |
| R10 | `cargo test --lib` 链接期崩 | 中 | 全部测试跑不了 | 三个新文件顶部照抄红线说明；`cargo test --lib` 在 T1 结束时立刻跑一次 |
| R11 | 登录窗白板（死锁） | 中 | 登录流程不可用 | **必须**在独立 `std::thread` 里建窗（D4） |
| R12 | 登录窗读到旧 cookie（假 Ready） | 中 | 用户以为登好了，其实没登 | 照抄三道保险：清 jar / 基线比对 / 服务端验活（D5） |
| R13 | 夸克 cookie 是账号最高权限凭据 | **确定** | 泄漏 = 账号沦陷 | 明文只在内存 + DPAPI 密文；手写 `Debug` 只打印长度；不进日志 / 错误信息 / `user-remote-connections.json` |
| R14 | 下载限速 | 高 | 体验差 | **不做任何绕过**；UI 显示真实速率与已下载量 |
| R15 | 大文件内存峰值 | 中 | 吃满内存 | `QUARK_MAX_DOWNLOAD_BYTES` 人为闸，超限返回 `Unsupported` |
| R16 | 误以为有 `refresh_token` | 低 | 走弯路 | A 层已否定；§7-U2 留作待验，但 v1 不实现 |
| **R17** | **用户出口 IP 被风控 / 拦截** | 中 | **高** | **T0 记录实测出口 IP 与响应特征**；命中则整体走退路（§7.1）。UI 要给**明确文案**（如「当前网络环境无法访问夸克网盘，请更换网络后重试」），**不能**笼统报「登录失败」。依据：B 层原文「2024/07/24 晚起，貌似国外服务器访问夸克 API 时会被拦截」⇒ 存在与请求来源无关、**只与出口 IP 有关**的独立失败模式 |

---

## 5. 分阶段实施计划

> 每阶段都有**确切闸门命令**。任一闸门红则该阶段不算完成。

### 阶段 0（T0）——认证可行性实测（**头号风险，必须先做**）

**工具形态（评审 M3）**：**只允许外部只读探针** —— `curl` / `Invoke-WebRequest` 之类，**不碰产品代码树**。**不写** `src-tauri/src/quark_probe.rs`：T0 阶段往产品树塞模块，一旦结论是「走退路」这段代码就白写；而一个 env-gated 常驻探针对最终用户没有价值。该文件已从 §4.3 的新增清单移除，只在 **T4 之后**、且主路线成立确有需要时才评估。

**凭据来源（评审 B2 —— 写死的可复现路径）**：

**已建好的自动化链路**（本轮已落地并实测跑通，全部在 `.superpowers/sdd/quark/`）：

| 文件 | 作用 |
|---|---|
| `t0-run.ps1` | 一键编排：起 Edge（CDP）→ 等登录 → 抓 cookie → 取 `ut` → 跑探针 → 写记录 |
| `t0-edge.ps1` | 用**一次性 profile** 起带 `--remote-debugging-port` 的 Edge |
| `t0-cookie.cjs` | 经 CDP 取 `.quark.cn` **整罐** cookie；找不到 `quark.cn` 页面即中止（**绝不读无关应用的 cookie**） |
| `t0-meta.cjs` | 从已登录页面直取 `ut`（`window.QUARK_WSGUID`）与 UA ⇒ **不靠猜** |
| `t0-probe.cjs` | 按 8 问发只读请求 + 三态判据；**只输出 cookie 名称与长度，绝不输出值** |
| `t0-kill-edge.ps1` | 只杀一次性 profile 的 Edge，不碰用户日常窗口 |

执行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .superpowers/sdd/quark/t0-run.ps1 -Port 9333 -WaitMinutes 12
```

> ⚠️ **端口必须先查占用**：本机 9222 已被另一个应用占用，CDP 会连到**别人的浏览器**并读到**无关应用的 cookie**。`t0-run.ps1` 会自动换端口，两个 CDP 脚本也会在找不到 `quark.cn` 页面时直接中止。
>
> ⚠️ **等待条件必须是登录态 cookie**：匿名访问 `pan.quark.cn` 也会下发 12 个 `.quark.cn` cookie（含 `ctoken` / `isQuark` 等），此时打 `file/sort` 只会得到 `HTTP 401 + code=31001`。**只有出现 `__pus` 才算登录**，否则「游客 cookie 的 401」会被误读成「路线不成立」。

**手工兜底**（自动化不可用时）：

1. 用 **Edge** 登录 `https://pan.quark.cn/`。
2. DevTools → Network → 任一 `drive-pc.quark.cn` 请求 → 复制**整条 `Cookie` 请求头**。
3. 存入 `.superpowers/sdd/quark/t0-cookie.txt`（**不入 git**，阶段末销毁）。

采集要求：

- 采集**整罐** `.quark.cn` cookie（U11 待验 ⇒ **先采整罐、再在 T0 内分组裁剪**，不要一上来就只挑 `__pus`）；
- 记录**采集时刻**（用于判断过期）；
- 全程**只读** —— 不做任何写操作、不分享、不删除。

**必须回答的问题（8 问）**：

| # | 问题 | 关联 | 判定口径 |
|---|---|---|---|
| 1 | 非浏览器来源的 Cookie 直连是否被接受？ | U4 / R1 | 三态：主路线成立 / 必须走退路 / 证据不足 |
| 2 | 缺 `device_model` / `ut` / `ve` / `uc_param_str` 分别会不会被拒？ | U4 / U6 | 得出必需参数清单；**必须在已按 §4.6 补齐全部 query 参数的前提下测** |
| 3 | `fr=win` 与 `fr=pc` 哪个被接受？ | U5 / R3 | 记录被接受的值（两个都失败 ⇒ 先查 R17 的 IP 维度，不直接判路线不成立） |
| 4 | `__pus` 单个是否够？还是要整罐？ | U1 / U11 | 记录必需 cookie 清单 |
| 5 | `size` 与 `updated_at` 的单位 | U8 / R5 | 用已知大小文件核对 |
| 6 | `_size` 的服务端上限 | U9 / R4 | 请求 `_size=1000` 看返回条数 |
| 7 | 是否有风控 / 验证码；**出口 IP 是否被拦** | U7 / R2 / **R17** | 记录现象 + 实测出口 IP + 响应特征 |
| 8 | 账号昵称接口 `GET /pan.quark.cn/account/info` 是否可用 | U12 / B 层 | 记录响应（端点已证实存在，昵称字段形状待验） |

**闸门：三态判定（评审 B3 —— 量化判据）**：

| 结果 | 判据 | 动作 |
|---|---|---|
| **主路线成立** | HTTP 200 **且** `code == 0` **且** `data.list` 与 `metadata` 自洽（条数 ≤ `_size`，`metadata.total` ≥ list 条数）**且** 连续 **3 次**请求都成功 **且** 用列表里任一子目录 `pdir_fid` 再列举一次也成功 | 进入 T1 |
| **必须走退路** | 带**合法且完整** cookie、**且已按 §4.6 补齐全部 query 参数**后，仍返回 HTTP **401/403** **或** `code == 31001 / 31004` | 转 §7.1 退路，并同时给出退路方案的**最小可运行**证据 |
| **证据不足** | 5xx / 超时 / 429 / 弹验证码 / cookie 采集不全 / 参数未补齐 | **不得**判「路线不成立」—— 先补齐再测；本态**必须**写进记录 |

**验收判据**：

- 一份实测记录（上述 8 问逐条给结论 + 原始响应片段），落到 `.superpowers/sdd/quark/t0-record.md`
- 记录里明确写下**三态之一**（主路线成立 / 必须走退路 / 证据不足），并写明**出口 IP**、**采集时刻**、是否弹验证码
- 若判「必须走退路」：同时给出退路方案的**最小可运行**证据
- 阶段末**销毁** `.superpowers/sdd/quark/t0-cookie.txt`，并在记录里写明已销毁

**闸门**（**不再**用 `cargo test --lib` —— 那会把 T0 变成「必须先能编译」的任务，拖慢头号风险验证）：

- §5 阶段 0 的 **8 问逐条有结论**
- **三态之一**已写下
- **临时凭据文件已销毁**

> **约束**：T0 允许**用用户自己的夸克账号**做登录与只读列举，**不得**做任何写操作、不得分享、不得删除。凭据只落 `.superpowers/sdd/quark/` 下的临时文件并在阶段结束时销毁。

### 阶段 1（T1）——`quark_api.rs` 纯逻辑层

**做什么**（全部纯函数，零网络）：

- 常量：host / base_url / client_id / pr / fr / uc_param_str / page size / 超时 / cookie 名
- `quark_list_dir_url()` / `quark_account_info_url()` / `quark_download_url()`
- `clamp_page_limit()` / `next_page()`
- `parse_list_response()` / `parse_account_info_response()` / `parse_download_response()`
- `enum QuarkFailure` + `classify_quark_failure()`（**穷尽**）+ `failure_from_code()` + `failure_from_http_status()`
- `struct QuarkFileEntry` + `to_remote_entry()` / `to_remote_entries()`
- `quark_seconds_to_unix_millis()`（**显式**单位换算，防 R5）
- `sanitize_cookie_value()`（照抄百度：**判非法**而非过滤）
- `struct QuarkSession` + 手写 `Debug`（只打印长度）

**验收判据**：
- `cargo test --lib` 新增 **api ≥ 60 个 test 函数**，**零 `tauri::`**、**零网络**
- 关键反例（缺一条就算没做完）：
  - `code != 0` ⇒ 走失败分类，**不得**解析出空列表冒充成功
  - `code == 31001` ⇒ `Auth`
  - 非 JSON / 缺 `code` ⇒ `Protocol`
  - `size` 单位换算有单测锁定
  - `clamp_page_limit` 超限被截断
  - `sanitize_cookie_value` 对含 `;` / `"` / 空白 / 控制字符的值返回 `None`
- 错误码映射覆盖**全部** 11 个 `RemoteErrorKind` 的穷尽表（照抄 `baidu_error_kind_coverage` 的做法）

**闸门**：
```
cargo test --lib
npm run check
```

### 阶段 2（T2）——`quark_auth.rs` 会话层

**做什么**（照抄 `baidu_auth.rs`）：

- `QUARK_SESSION_VERSION = 1`
- `encode_session()` / `decode_session()` / `is_quark_session_blob()`
- `scan_cookies()`（大小写敏感、同名取第一个、只投影必要字段）
- `load_quark_session()` / `save_quark_session()` / `delete_quark_session()`（走 `credentials.rs` 的 DPAPI 链路，键 = 连接 id）
- `quark_auth_status()` / `describe_state()` / `is_expired()` / `remaining_seconds()`
- `session_to_cookie_header()`（会话 → `Cookie` 请求头字符串；模板在 `baidu_auth.rs:436`，`commands.rs:282` 正在用它，**不能漏**）
- `QUARK_STOKEN_EXPIRY_WARN_SECONDS` → 改为「距上次验活时间」的等价语义（夸克无 STOKEN）

**验收判据**：
- `cargo test --lib` 新增 **auth ≥ 25 个 test 函数**
- 关键反例：
  - 密文里**不含**明文 cookie（扫全文断言）
  - `Debug` 输出**不含** cookie 明文（只含长度）
  - `decode_session` 对版本不认识的 blob ⇒ 按「没有凭据」处理，**不是** panic
  - `save` / `load` 往返后 cookie 完整

**闸门**：
```
cargo test --lib
npm run check
```

### 阶段 3（T3）——`quark_provider.rs` 只读 provider

**做什么**（照抄 `baidu_provider.rs` 的三个 trait 拆分）：

- `trait QuarkApi`（**抽 trait 是为了零网络单测**）/ `HttpQuarkApi` / `UnavailableQuarkApi`（占位实现，把初始化错误推迟到第一次真用网络）
- `trait QuarkSessionStore`（**抽 trait 是为了零 DPAPI 单测**）/ `DpapiQuarkSessionStore`
- `struct QuarkRemoteProvider` + `impl RemoteProvider`
- 实现 `read_dir` / `read_file` / `get_permissions`
- `write_file` / `write_file_with_progress` / `set_permissions` / `make_dir` / `remove_path` / `rename_path` ⇒ **明确 `RemoteError::unsupported`**

**验收判据**：
- `cargo test --lib` 新增 **provider ≥ 40 个 test 函数**
- 关键反例（缺一条就算没做完）：
  - `write_file` 必须返回 `Unsupported`，**不能返回 `Ok(())`**
  - `set_permissions` 必须返回 `Unsupported`，**不能返回 `Ok(())`**
  - `make_dir` / `remove_path` / `rename_path` 在 v1 同样返回 `Unsupported`
  - `get_permissions` 返回 `Ok(None)`，**不是** `Err`
  - `read_dir` 翻页：构造 3 页数据，断言条目**不重不漏**
  - `read_dir` 遇 `code=31001` ⇒ `Auth`，不是 `Unknown`
- `RemoteProvider` 的 9 个方法**全部**有实现（编译期保证）

**闸门**：
```
cargo test --lib
npm run check
```

### 阶段 4（T4）——`quark_login.rs` 内嵌登录窗

**做什么**（照抄 `baidu_login.rs`）：

- `LOGIN_WINDOW_LABEL = "afm-quark-login"`
- `LOGIN_START_URL` = **以 U3 的 T0 结论为准**（候选：`https://pan.quark.cn/`、配置块里的 `scanLoginPage = https://su.quark.cn/4_eMHBJ`）—— **不写死**
- `LOGIN_COOKIE_URL` = 同上（cookie 锚点）
- `LOGIN_EVENT = "afm://quark-login"`
- 每轮登录**独立 webview data-dir**（`quark-login-profile/session-<stamp_ms>`）
- **在独立 `std::thread` 里建窗**（D4，红线）
- 三道保险（D5）：清 jar / 基线比对 / 服务端验活
- `PendingSession` / `PendingSlot` / `take_pending` / `store_pending` 两段式
- 四个命令：`remote_quark_login_start` / `_pending` / `_claim` / `_cancel`
- 轮询 `POLL=2s` / `GIVE_UP=900s` / cookie 读取预算 10s / 最大失败 3 次 / 验活预算 8s

**验收判据**：
- `cargo test --lib` 不崩（**这个文件里的逻辑必须能抽出纯函数测试**；`tauri::` 只允许出现在非测试路径）
- 真机（打包产物）里：点「添加夸克网盘」→ **窗口正常显示**（不是白板）→ 扫码登录 → 授权卡显示昵称
- 二次点击「添加」⇒ 弹窗正常，**不**误判为已登录
- 关窗后**没有**残留窗口（`allow-close` / `allow-destroy` 已具备）

**闸门**：
```
cargo test --lib
npm run check
npm run tauri:build
```

### 阶段 5（T5）——命令接线与协议注册

**做什么**：

- `connection.rs`：加 `QuarkPan` 变体 / `as_str` / `default_port`
- `mod.rs`：三个 `mod` + 两段 `pub use`
- `registry.rs`：`QuarkPan` **不走**通用 password 路径（`password: None`）
- `commands.rs`：
  - `remote_quark_save_session` / `remote_quark_auth_status` / `remote_quark_delete_session`
  - `provider_with_protocol` 的**三处** `QuarkPan` 分支
  - 「测试连接」从存档还原会话作 credential
  - 命令注册表反向自检清单加夸克命令
- `lib.rs`：`mod quark_login;` + `generate_handler!` 注册 7 个夸克命令

**验收判据**：
- `cargo test --lib` 通过
- **命令注册自检通过**（`commands.rs:991-1004` 那一套反向自检会抓漏注册）
- `provider_with_protocol` 的 `match` **穷尽**（编译期保证）
- 协议枚举的 `as_str` / `default_port` 有单测

**闸门**：
```
cargo test --lib
npm run check
```

### 阶段 6（T6）——前端接线

**做什么**：

- `src/types/remote.ts`：协议类型 + `isQuarkPanProtocol` + `isNetdiskProtocol` + 3 份载荷镜像类型
- `connection-form.vue`：**网盘分支按 provider 分流**（百度走现有 `baidu_pan` 分支，夸克走 `quark_pan` 分支）；夸克分支隐藏 host/port/密码框，显示授权卡（登录按钮 / 状态 / 昵称 / 重授权 / 取消）。**协议下拉不动**
- `remote-connections.ts`：固定 host/port、保存分派、测试连接、授权状态、登录 start/cancel、`quarkLoginRequired`
- `sections.ts` / `virtual-locations.ts` / `remote-location-refs.ts` / `remote-nav-panel.vue`：**全部改用 `isNetdiskProtocol`**（消除并行判定口径）
- `remote-nav-panel.vue`：新建请求可预置 `'quark_pan'`

**验收判据**：
- `npm run test:unit:run` 新增 ≥ 15 条
- 关键反例：
  - 在网盘分区选 provider「夸克网盘」后，host/port/密码框**不可见**
  - 未授权时保存连接 ⇒ 报「需要登录夸克网盘」分类文案
  - `isNetdiskProtocol('baidu_pan')` 与 `isNetdiskProtocol('quark_pan')` 均为 `true`，`isNetdiskProtocol('sftp')` 为 `false`
  - 侧边栏「网盘」分区**同时**出现百度与夸克连接；「远程」分区**不含**它们
- 真机（打包产物）截图：网盘分区的 provider 选择器里有「百度网盘 / 夸克网盘」；选夸克后表单切成授权卡

**闸门**：
```
npm run check
npm run tauri:build
```

### 阶段 7（T7）——i18n 与收尾

**做什么**：

- **17 个**语言包各加 **1 个约 15 键的嵌套对象** + **2 个平铺键**（`protocols.quark_pan` / `quarkLoginRequired` / `quark` 对象）；中文值走 `ch.json`。夸克**没有 STOKEN**，「失效 / 临期」文案要**重新起草语义**，不是把 stoken 换成 pus 抄一遍
- 跑 `npm run sync-i18n` 同步
- 改完验 `\uFFFD` 为空（编码没坏）
- `CHANGELOG.md` 按仓库既有格式补一条（`npm run check` 里的 `verify-changelog` 会验）

**验收判据**：
- 17 个语言包**全部**含夸克的「1 个约 15 键嵌套对象 + 2 个平铺键」，**无遗漏**
- `npm run sync-i18n` 后无 diff
- `npm run check` 全绿（含 `verify-changelog`）

**闸门**：
```
npm run sync-i18n
npm run check
```

### 阶段 8（T8）——真机验收与打包

**做什么**：按 §6 的真机清单逐条实拍。

**闸门**：
```
npm run tauri:build
```

> **打包很慢** ⇒ 判断出要验就**早启动**、丢后台跑，再补文档/测试。构建脚本放 `.superpowers/sdd/*.cmd`、**纯 ASCII**。后台跑不要挂 stdout 管道。`tauri dev` 下的界面验收**一律作废**（AGENTS.md:24 / :116 / :117）。

### 5.1 阶段依赖与并行

```
T0 ──► T1 ──► T2 ──► T3 ──► T4 ──► T5 ──► T6 ──► T7 ──► T8
 │
 └──►（若 T0 判定走退路：T1 的 HTTP 层改为「在 WebView2 内 fetch + Rust 解析」，
      T1/T3 形态变化，但阶段划分与闸门不变）
```

T1 与 T2 可并行（不同文件、无依赖）。T6 与 T7 可并行（不同文件）。

### 5.2 工时预估（评审 M5 复核值）

| 阶段 | 原估（人天） | **复核（人天）** | 差异依据 |
|---|---:|---:|---|
| T0 实测 | 1.0 | **1.0–1.5** | 要补 cookie 获取路径（B2）、三态判据（B3）、参数补全（M1）、IP 维度记录（M2 / R17） |
| T1 API 层 | 1.5 | **2.0–2.5** | 模板 `baidu_api.rs` 生产 1044 行 + 98 个 test fn；错误码映射还要再补 4 个码（M7） |
| T2 会话层 | 0.5 | **1.0–1.5** | 模板 `baidu_auth.rs` 生产 488 行 + 49 个 test fn；还漏了 `session_to_cookie_header` 这类必备的会话 → cookie 头转换 |
| T3 provider | 1.0 | **1.5–2.0** | 模板 `baidu_provider.rs` 生产 790 行 + 59 个 test fn；且需处理夸克特有的分页 / `_fetch_total` 交叉校验（R4） |
| T4 登录窗 | 1.0 | **1.0–1.5** | 模板 `baidu_login.rs` 957 生产行；夸克入口 URL 本身是 U3 未定项，可能要返工 |
| T5 命令接线 | 0.5 | **0.5–1.0** | `commands.rs` 现有 1065 行，要加 3 + 4 命令 + 3 处分支 + 注册自检；原估 `+60` 行属乐观 |
| T6 前端 | 1.0 | **2.0–2.5** | 因 B1 要改的不止一个文件：`connections-section.vue` + `remote/index.vue` + `sections.ts` + `remote-nav-panel.vue` 的 `sections` 结构 + 表单分支 + provider 选择器 |
| T7 i18n | 0.5 | **1.0–1.5** | 17 语言 × 约 15 键 ≈ 250+ 条串，含 16 种非中文语言的人工翻译（M4） |
| T8 真机 | 1.0 | **1.0–1.5** | `tauri build` + 装机 + 多账号 / 多网盘分区 / 失效重授权等场景 |
| **合计** | **8.0** | **11.5–12.5** | 原 8.0 人天**偏乐观 40–55%** |

**若 T0 判定走退路（§7.1）**：再加 **1.5–2.0 人天**（退路是「在 WebView2 里跑 fetch、把 JSON 回传 Rust」，需要新的数据通道与前端脚本），合计 **13.0–14.5 人天**。

> **模板体量实测**（工时复核依据）：`baidu_api.rs` 2424 行 / 生产 1044 / test fn **98**；`baidu_auth.rs` 1013 行 / 生产 488 / test fn **49**；`baidu_provider.rs` 2306 行 / 生产 790 / test fn **59**；`baidu_login.rs` 1092 行 / 生产 957 / test fn 11。
>
> §4.3 给夸克新文件的规模是**纯生产**口径，**没把测试体量算进去** —— 按模板「测试 : 生产 ≈ 1 : 1」，T1–T3 的「零网络单测」根本写不完。

---

## 6. 验收判据

### 6.1 单测

- `cargo test --lib` 新增 **≥ 125 个 test 函数**（**api ≥ 60 + auth ≥ 25 + provider ≥ 40**，按 **test 函数数**口径，不是行命中；模板实测 api **98** / auth **49** / provider **59**，合计 206），且**不出现** `STATUS_ENTRYPOINT_NOT_FOUND`
- `npm run test:unit:run` 新增 **≥ 15 条**
- 关键反例（**必须有，缺一条就算没做完**）：
  - `.` / `..` 段必须被 `normalize_remote_path` 拒 → `RemoteErrorKind::Protocol`
  - `write_file` / `write_file_with_progress` 必须返回 `Unsupported`，**不能返回 `Ok(())`**
  - `set_permissions` 必须返回 `Unsupported`，**不能返回 `Ok(())`**
  - `make_dir` / `remove_path` / `rename_path` 在 v1 必须返回 `Unsupported`
  - `get_permissions` 返回 `Ok(None)`（协议无权限位），不是 `Err`
  - `code != 0` 不得解析出「成功的空目录」
  - `code == 31001` ⇒ `Auth`

### 6.2 门禁

```
cargo test --lib
npm run check        # = ts + lint:check + test:unit:run + verify-changelog + verify:sandbox
npm run tauri:build
```

- `npm run check` 的 `scripts.check` 实为 `run-p ts lint:check test:unit:run verify-changelog verify:sandbox` ⇒ **`verify:sandbox` 也在门禁里**，别漏（评审 N6）

- `eslint .` 的基线数字随提交变，别拿旧数字对；要对照就先量 HEAD 真实基线
- `npm run tauri:build` 成功，产物落到 `release/`

### 6.3 真机（**必须打包产物，`tauri dev` 的界面验收一律作废**）

1. 设置 → 远程 → **网盘**分区 → 点「添加」→ **provider 选择器出现「百度网盘 / 夸克网盘」二选一**
2. 选「夸克网盘」→ host/port/密码框**消失**，出现「添加夸克网盘」授权按钮
3. 点添加 → 内嵌窗口**正常显示**（不是白板）→ 扫码 / 手机号密码登录 → 关窗后卡片显示账号昵称
4. 二次点「添加」⇒ 弹窗正常，**不**误判为已登录
5. 保存 → **侧边栏「网盘」分区出现该连接**（用户需求的原话落点）
6. 双击 → 列出根目录，条目名 / 大小 / 时间正确（**大小单位对得上网页端**）
7. 进两层子目录 → 地址栏与面包屑正确
8. 右键一个文件 → 下载到本地 → **文件落地且 sha256 与原文件一致**
9. 复制一个远端文件 → 粘贴到本地目录 → 落地一致
10. 右键菜单里**没有**「权限」类项
11. 断网 → 提示是**分类文案**（如「网络不可达」），不是「未知错误」
12. 让会话失效（在网页端退出登录）→ 列表操作提示「请重新登录」，**不是**静默失败
13. 夸克连接与百度连接**同时存在**，两条链路互不影响（回归检查）
14. 重启应用 → 夸克连接**仍在**且仍可浏览（凭据持久化验证）

### 6.4 数据安全（验收前必做）

- `remote-credentials.json` 是**既有文件**（`credentials.rs:25` `REMOTE_CREDENTIALS_FILE = "remote-credentials.json"`，现存的是 FTP 类凭据），本轮**新增承载**一类**账号级 cookie** 的凭据 —— 属新增风险点：授权会真的写进用户的密文档。验收前备份整个 `user-data/` 目录
- 恢复顺序**必须先杀进程再 restore**（AGENTS.md：应用退出时会把内存状态写回，先 restore 会被覆盖）
- `dash-accept-data.cjs backup` 的语义是「备份不存在才建」，**跑完必须看它打印的字节数**
- 验收后确认 `user-remote-connections.json` 里**没有**任何 cookie 明文（只有 `credential_key`）
- 验收后确认 `remote-credentials.json` 里**没有**夸克 cookie 明文（只有 DPAPI 密文）
- 请求参数（含 `device_model` 带出的**本机主机名**）**不落盘、不进日志**（评审 M1 / N8）

### 6.5 与既有铁律的对照

| 铁律 | 本计划的落实 |
|---|---|
| `cargo test --lib` 不得链接 `tauri::` | 三个新模块全部零 `tauri::`；`AppHandle` 只出现在 `commands.rs` 与 `quark_login.rs`（后者不被测试链路链接） |
| 真机验收用打包产物 | T8 走 `npm run tauri:build` |
| 需要装机实测就直接打包 | T4/T6/T8 判定要验就早启动后台构建，不等用户开口 |
| 改 `.vue` / `.ts` / `.json` 用编辑工具 | 语言包 17 个用 `node` 脚本按 UTF-8 读写，改完验 `\uFFFD` 为空 |
| 面向用户文案简体中文 | 语言包中文值走 `ch.json`（locale code 是 `ch`，不是 `zh`）；代码标识符 / API 路径 / header / cookie 名保持英文原样 |
| 不新增 `RemoteConnection` 字段 | host / port / username / `credential_key` 全部复用（D7） |
| 不新增 `RemoteErrorKind` | 全部失败落到既有 11 个 kind（§4.7） |
| 受保护常量不动 | 不碰 mDNS 服务名、legacy 数据目录名、Tauri `identifier` |

---

## 7. 未知与待验项

> **这一节是本计划里最重要的部分。** 下面每一条都**不得**在实现时被当作已知结论使用。

| # | 未知 | 影响面 | 关联风险 | 验证阶段 | 当前证据强度 |
|---|---|---|---|---|---|
| **U4** | **服务端是否接受非浏览器来源的 Cookie 请求**（缺 `device_model` / `ut` / `QUARK_WSGUID` 是否被拒） | **决定整条路线是否成立** | **R1** | **T0** | **无**（C 层） |
| U6 | `ut` / `QUARK_WSGUID` 是否必需 | 请求头清单 | R1 | T0 | 无 |
| U5 | `fr=win`（本地客户端）vs `fr=pc`（开源脚本）哪个被接受 | 请求参数 | R3 | T0 | **冲突** |
| U1 | 会话 cookie 名是不是 `__pus`、是否单 cookie 就够 | 登录窗判定 + 凭据格式 | R8 | T0 | 仅 B 层 |
| U8 | `size` 的单位（字节 / KB）、`updated_at` 的单位（秒 / 毫秒） | 显示与排序 | R5 | T0 | 无 |
| U9 | `_size` 的服务端上限 | 分页正确性 | R4 | T0 | 无（客户端用 100） |
| U3 | 夸克登录页的精确 URL（`pan.quark.cn` 首页 / `su.quark.cn` 扫码页（配置块里的 `scanLoginPage = https://su.quark.cn/4_eMHBJ`）/ `coral3.quark.cn`） | 登录窗能否登进去 | — | T0 | 三选一未定（§4.5 **不写死**，以 T0 结论为准） |
| U2 | 夸克服务端是否存在 `refresh_token`（本地客户端**不用** ≠ 服务端**没有**） | v1 不实现，仅记录 | R16 | T0 | 本地无命中 |
| U7 | 是否有风控 / 验证码 / 频率限制 | 稳定性 | R2 | T0 + T8 | 有相关配置，无触发条件 |
| U10 | 下载是否需要额外 header（`Referer` / `Origin` / `Range`） | 下载是否 403 | R7 | T0 / T8 | 客户端只显式注入了 Cookie |
| U11 | cookie 是否必须覆盖整个 `.quark.cn` 域 | 凭据采集范围 | R8 | T0 | 社区称 `__pus` 单个够 |
| U12 | 账号昵称接口 `GET https://pan.quark.cn/account/info?fr=pc&platform=pc` 在登录态下是否稳定返回 | 授权卡显示昵称 | — | T0 | 仅 B 层 |
| U13 | `file/sort` 的 `_sort` 参数格式在本客户端版本下的确切取值 | 排序表现 | — | T0 | B 层给出三种示例 |
| U14 | 夸克是否有「根目录以外的并列入口」（如「分享」「我的资源」），要不要像百度那样**不伪装成目录** | 导航语义 | — | T8 | 未查 |

### 7.1 若 T0 判定主路线不成立（降级路径，按优先级）

| 顺序 | 降级方案 | 代价 | 备注 |
|---|---|---|---|
| 1 | **在 WebView2 内执行 `fetch`**，Rust 只做解析与落盘 | +1.5 人天 | `file/sort` 是公开 GET 接口，页面内 fetch 天然带全部 cookie 与浏览器指纹。**这条路径几乎必然可行**，因为它就是夸克网页端自己在做的事 |
| 2 | 让用户手动粘贴 `__pus`（设置页给输入框） | +0.5 人天 | 与百度「粘贴 BDUSS」的降级思路同构 |
| 3 | 只做下载直链（`download_url`）不做目录列举 | 功能残缺 | 只在用户明确要求时考虑 |

> **工作量口径**：若 T0 判「必须走退路」，整体再加 **1.5–2.0 人天**（按上表**方案 1** 估算；方案 2 是它的替代路径，**不是**叠加）。合计见 §5.2。

### 7.2 明确**没有**答案的问题（不猜）

- 夸克是否有官方开放平台的稳定授权流程（`open-api-drive.quark.cn` + `client_id:533` + `uopAccessSalt` 的完整语义）—— **本轮未查证**，v1 不走这条路。
- 夸克客户端的 `secureNo: 14100` 的用途 —— **本轮未查证**，只记录存在。
- `coralSignInUrl` / `utoken2.uc.cn` 的用途 —— **本轮未查证**。

---

## 8. 证据路径

### 8.1 本轮产出的中间产物（全部在 `.superpowers/sdd/quark/`）

| 路径 | 内容 |
|---|---|
| `.superpowers/sdd/quark/asar-read.cjs` | 只读 asar 头解析器（解析 pickle 头；`[0..4) payloadSize \| [4..8) strLen \| [8..) JSON`） |
| `.superpowers/sdd/quark/hunt.cjs` | 压缩 JS 的上下文 grep |
| `.superpowers/sdd/quark/uniq.cjs` | 唯一 token 提取 |
| `.superpowers/sdd/quark/win.cjs` | 偏移窗口 dump |
| `.superpowers/sdd/quark/scan-baidu-touchpoints.cjs` | 仓库 baidu touch-point 扫描器 |
| `.superpowers/sdd/quark/baidu-touchpoints.txt` | **完整 touch-point 清单**：96 个文件 / 2189 行命中，每文件精确行号 + 该行原文（**§2.2 的直接数据源**） |
| `.superpowers/sdd/quark/rea-session-notes.txt` | **REA 会话状态记录**（§3.1.3 六步的原文摘录，含成功与失败） |
| `.superpowers/sdd/quark/quark-launcher.exe` | 从 `D:\Program Files\Quark\quark.exe` 复制的副本（仅供 REA 绑定）；sha256 `a461005270b34655c21fb763ec16fe77fac985f5a3a33eb6e9275ed590b3c0b4` |
| `.superpowers/sdd/quark/asar/dist/renderer/index.js` | **主证据**：夸克 renderer bundle（8,834,654 B），含全部网盘 API |
| `.superpowers/sdd/quark/asar/dist/main/main.js` | 主进程 bundle（5,884,882 B），sha256 `522f6fecea47d915511ee4a78596e6a171a564fce1d8932375e8dff3f8a6065f` |
| `.superpowers/sdd/quark/out-config-block.txt` | §3.2.1 服务配置块原文 |
| `.superpowers/sdd/quark/out-req2.txt` / `out-req3.txt` | §3.2.2 列表调用与参数原文 |
| `.superpowers/sdd/quark/out-req-layer.txt` | §3.2.3 两个 axios 请求层原文 |
| `.superpowers/sdd/quark/out-mods-dump.txt` | §3.2.3 常量模块（`ucparams` / `UC_PARAMS_STR` / `getUcParams` / `getQuarkPCVersion`）原文 |
| `.superpowers/sdd/quark/out-listparams.txt` | 列表参数集合 |
| `.superpowers/sdd/quark/out-final-loc.txt` | 关键位置定位 |
| `.superpowers/sdd/quark/out-clouddrive-paths.txt` | **163 条 endpoint 完整清单** |
| `.superpowers/sdd/quark/out-hosts.txt` / `out-quarkhosts-all.txt` | 全部 host |
| `.superpowers/sdd/quark/out-headers.txt` / `out-xheaders.txt` | header 集合（**证明无自造签名**） |
| `.superpowers/sdd/quark/out-cookies.txt` / `out-pus.txt` / `out-pus-names.txt` | cookie 处理（`out-pus.txt` 为空 = renderer 从不按名引用 `__pus`） |
| `.superpowers/sdd/quark/out-pr.txt` / `out-prfr.txt` | `pr` / `fr` 取值 |
| `.superpowers/sdd/quark/out-httpclient.txt` | HTTP 客户端构造 |
| `.superpowers/sdd/quark/out-drivepc.txt` | `drive-pc.quark.cn` 相关 |
| `.superpowers/sdd/quark/out-regions.txt` | 区域/节点配置 |
| `.superpowers/sdd/quark/t0-run.ps1` | **T0 一键编排**：起 Edge（CDP）→ 等登录 → 抓 cookie → 取 `ut` → 跑探针 → 写记录 |
| `.superpowers/sdd/quark/t0-edge.ps1` | 用**一次性 profile** 起带 `--remote-debugging-port` 的 Edge |
| `.superpowers/sdd/quark/t0-cookie.cjs` | 经 CDP 取 `.quark.cn` **整罐** cookie；找不到 `quark.cn` 页面即中止（**绝不读无关应用的 cookie**） |
| `.superpowers/sdd/quark/t0-meta.cjs` | 从已登录页面直取 `ut`（`window.QUARK_WSGUID`）与 UA ⇒ **不靠猜** |
| `.superpowers/sdd/quark/t0-probe.cjs` | 8 问只读请求 + 三态判据；**只输出 cookie 名称与长度，绝不输出值** |
| `.superpowers/sdd/quark/t0-kill-edge.ps1` | 只杀一次性 profile 的 Edge，不碰用户日常窗口 |
| `.superpowers/sdd/quark/t0-record.md` | **T0 实测记录**（8 问逐条 + 原始响应片段 + 三态结论） |
| `.superpowers/sdd/quark/t0-cookie.txt` | **临时凭据**（不入 git，阶段末销毁） |

### 8.2 仓库侧证据（只读引用）

| 路径 | 用途 |
|---|---|
| `docs/plan-2026-11-08-baidu-netdisk-phase1.md` | 模板；D1/D2/D3/D4 的原始决策与「后来被推翻」的记录 |
| `docs/2026-11-07-baidu-t1-auth-findings.md` | 百度 T1 实测（refresh_token 路线为何被否决） |
| `docs/2026-11-09-baidu-write-chain-blocker.md` / `docs/2026-11-09-baidu-write-session-findings.md` | 百度写操作链路的踩坑（夸克 v1.1 直接复用教训） |
| `docs/handoff-2026-11-08-baidu-embedded-login.md` / `docs/handoff-2026-11-09-baidu-embedded-login.md` | 百度内嵌登录的交接记录（独立线程建窗的教训来源） |
| `src-tauri/src/baidu_login.rs` | 登录窗模板（三道保险 / pending slot / 独立线程） |
| `src-tauri/src/remote/baidu_api.rs` / `baidu_auth.rs` / `baidu_provider.rs` | API / 会话 / provider 三层模板 |
| `src-tauri/src/remote/mod.rs:5-18` | `cargo test --lib` 红线原文（`STATUS_ENTRYPOINT_NOT_FOUND` 成因） |
| `AGENTS.md` | 闸门命令、真机验收铁律、备份恢复顺序、`.cmd` 纯 ASCII 要求 |

### 8.3 外部来源（B 层）

- `https://github.com/lala123963/lingk5/blob/main/夸克网盘.py`
- `https://github.com/SingTsang/Auto_Check_In/blob/main/checkIn_Quark.py`
- `https://docs.hunhepan.com/reman/pantools.html`
- `https://greasyfork.org/ug/scripts/571335-夸克网盘空间占用分析自用版`
- `https://greasyfork.org/zh-TW/scripts/504333-夸克网盘批量工具集`
- `https://developer.cloud.tencent.com/article/2643818`
- `https://www.hqwc.cn/a/1360320.html`

### 8.4 本轮的工作边界声明

- 本轮**只读**了 `D:\Program Files\Quark\`，**未启动**夸克客户端、**未安装**任何东西、**未登录**任何账号。
- 本轮**未修改**仓库中除 `docs/plan-2026-11-11-quark-pan.md` 与 `docs/review-2026-11-11-quark-pan.md`（评审报告）以外的任何文件；`.superpowers/sdd/quark/` 下为新建的中间产物。
- 本轮介入时仓库工作区是**干净的**：P5b 发版那批改动（`CHANGELOG.md`、`package.json`、`src/localization/messages/*.json`、`src-tauri/Cargo.toml` / `Cargo.lock` 等）已在 P5b 的三条 commit（`d1236562` / `8e5c01b9` / `5e10ff46`）里入库，**不再是未提交状态**。

---

## 9. 评审与修订记录

- **评审报告**：`docs/review-2026-11-11-quark-pan.md`（原始位置 `.superpowers/sdd/quark/plan-review-2026-11-11.md`，本轮已提升为正式文档）。
- **评审结论**：**需修改后放行** —— 方案的调研质量与 §2.2 的仓库锚点经复核基本全部成立，但有 **3 个 Blocker**（执行层的前置条件缺失或与现状冲突）必须先改。
- **本轮修订**：已按评审报告**逐条执行**，下列编号与评审编号一一对应。

| 评审编号 | 问题摘要 | 改在方案哪一节 |
|---|---|---|
| **B1** | UI 集成模型与仓库现状矛盾（被否决的旧写法见 §4.4.1） | §4.4（改用**网盘 provider 选择器**，含「为什么选 provider 选择器」与 §4.4.1「已否决的方案」）、§2.2.7、§5 阶段 6、§6.3 |
| **B2** | T0 没有规定 cookie 获取路径 | §5 阶段 0「凭据来源」：Edge DevTools 取整条 `Cookie` 头 → `.superpowers/sdd/quark/t0-cookie.txt`（不入 git、阶段末销毁） |
| **B3** | T0 没有量化通过判据 | §5 阶段 0「闸门：三态判定」（主路线成立 / 必须走退路 / 证据不足）+ 8 问判定口径对齐 |
| **M1** | §4.6 漏 `device_model` / `ut` | §4.6 请求形态 + 注、§3.2.3 `driveRequest` 拦截器行 |
| **M2** | R1 概率高估、漏出口 IP 风险 | §4.8（R1 改「低～中」；新增 **R17**） |
| **M3** | T0 工具形态与闸门方式 | §4.3（`quark_probe.rs` 移出新增清单 + 移后说明）、§5 阶段 0（只用外部只读探针；闸门改为「8 问 + 三态 + 凭据已销毁」） |
| **M4** | i18n 工作量被低估 | §2.2.8、§4.4 i18n 行、§5 阶段 7、§5.2 T7 |
| **M5** | 工期与产物规模不匹配 | §5.2（**11.5–12.5 人天**）、文档头部「预估工作量」 |
| **M6** | 多处行号 / 文件清单与仓库现状不符 | §2.2.1 / §2.2.2 / §2.2.3 / §2.2.4 / §2.2.7 / §2.2.9（逐条订正；`remote-tab-labels.ts` 改为「不需要改」） |
| **M7** | §4.7 错误映射表不穷尽 | §4.7（补 `31004` / `23018` / `25008` / `38001`，改回 `24005` 原文，脚注注明 `Unknown` 兜底须带原始 code）、§3.2.4 |
| **M8** | 「流式下载」是假想符号、trait 变更的波及面数量不准 | §1.2「不做」表（改为 trait **刻意不提供流接口** + 全仓库 **5** 个 impl） |
| **N1** | `failure_from_http_status` 行号 | §2.2.2（`:647`） |
| **N2** | `pub use` 段数与参照系 | §2.2.1（**4 段**）、§4.3（参照系写错） |
| **N3** | `connection-form.vue` 行数与选项行号 | §2.2.7（**1032 行** / 选项在 `:69`） |
| **N4** | `poll_loop` 区间终点未核验 | §2.2.4（终点标注**未核验**） |
| **N5** | 单测门槛口径 | §2.2.9 口径说明、§5 阶段 1 / 2 / 3、§6.1（按 test 函数数 ≥125） |
| **N6** | 门禁漏 `verify:sandbox` | §6.2 |
| **N7** | `remote-credentials.json` 是既有文件 | §6.4（改为「既有文件新增承载」）+ 验收清单补一条 |
| **N8** | 请求参数不落盘 / 不进日志 | §6.4（补 `device_model` 带出的本机主机名） |
| **N9** | 登录起始页写死 | §4.5、§5 阶段 4、§7 U3（**以 T0 结论为准**；`scanLoginPage = https://su.quark.cn/4_eMHBJ` 记为候选） |
| **N10** | 「同款列表调用」实为分享页详情 | §3.2.2（改为「同款响应形状」） |
| **N11** | 「没有签名参数」表述过宽 | §3.2.3 结尾（限定在 `file/sort` 路径上） |
| **N12** | §4.4 前端清单漏文件 | §4.4（补 `connections-section.vue` 与 `remote/index.vue`） |
| **N13** | §2.2.7 命中数未核验 | §2.2.7（标注「未核验」） |
| **N14** | §3.2.4 错误码收录范围不足 | §3.2.4（并入第二份副本的 3 个码） |

### 9.1 剩余未决项

| 项 | 状态 |
|---|---|
| **U4**（服务端是否接受非浏览器来源的 Cookie 请求） | **必须由 T0 拍板**，且**不得**因缺 `device_model` / `ut` 就判否。它决定整条路线是否成立 |
| **U3**（登录页精确 URL） | 可在 **T0 一并定**（低成本，抓包即可） |
| U1 / U2 / U5 / U6 / U7 / U8 / U9 / U10 / U11 / U12 / U13 | **延后到实现期**：全部由 T0 的实测记录与 T4–T8 的真机验收回答；方案已给默认值与判定口径 |
| **U14**（是否有根目录以外的并列入口） | 延后到 T8；v1 **不伪装成目录**（与百度一致） |

### 9.2 一句话状态

> **T0 仍是头号闸门，修订完成不等于路线成立。** 本轮只是把方案改到「可以按图施工」的状态 —— B1 / B2 / B3 修掉之后，**能否走通仍要看 T0 的三态结论**；若 T0 判「必须走退路」，范围、工期与形态都要重排（§7.1）。
