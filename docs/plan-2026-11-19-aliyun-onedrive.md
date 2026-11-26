# 计划：阿里云盘与 OneDrive 接入

> 制定日期：2026-11-19
> 适用仓库：`F:\soft\00selfmade\filemanager\sigma-file-manager`（Alpha File Manager）
> 基线：`package.json` version = `3.14.1`；分支 main，HEAD `18f54134`
> 依据：只读架构调研（未运行构建/测试）+ 仓库既有实现（夸克/百度网盘已落地并发布 3.14.0）

---

## 〇 结论一览

1. **provider 抽象层已经就绪，不需要重构。** `RemoteProvider` trait（`src-tauri/src/remote/provider.rs:39-183`）已把夸克与百度统一到同一组 9 个方法签名；`transfer.rs`、上传/下载/删除/改名的命令层、目录对比命令全部只吃 `&dyn RemoteProvider`。**新增网盘不需要改这些文件。**
2. **两个新网盘应落在核心 `src-tauri`，不应放进 `extensions/`。** 扩展沙箱是"第三方插件 + 权限 + 签名"模型，接入网盘要重写 trait、DPAPI 凭据、协议枚举、登录窗四条主线，成本远高于收益。
3. **两者的共同点是 OAuth，而不是"网盘"。** 现有夸克/百度的会话是 cookie blob；OneDrive 与阿里云盘是 `(access_token, refresh_token, expires_at)` 三元组。建议新开一条**窄 trait**（`OAuthCloudApi`）只服务这两个新 provider，**不要**回头改造夸克/百度。
4. **先做阿里云盘。** 它的结构最接近百度（两次请求拿直链、直链自带签名、不需要带 cookie 回源），不需要 PKCE、不需要本地回环 HTTP 服务、依赖增量最小。
5. **OneDrive 有一个前置未知项必须先做 0.5 天可行性验证**：内嵌 WebView2 能否截获 OAuth 重定向并拿到 authorization code。这条结论决定 OneDrive 是"新增 3-4 个依赖 + 本地监听器"还是"复用现有 `*_login.rs` 骨架"。
6. **动手前先修一个既有串台 bug**：`src/utils/remote-directory.ts:137-147` 的兜底文案硬编码了"夸克网盘"，百度会话失效也会显示"夸克网盘会话已失效"。新增第三个 provider 会放大它。

---

## 一 目标与范围

### 1.1 目标

在现有网盘体系（夸克 + 百度）之上，新增 **阿里云盘** 与 **OneDrive** 两个 provider，使它们具备与夸克相同的最小可用闭环：**登录 → 添加连接 → 浏览目录 → 下载文件 → 复制到本地 → 重启后仍在**。

### 1.2 本阶段范围（第一版只做只读）

与夸克现状保持一致（`quark_provider.rs:876` `reject_write`）：第一版**只实现 `read_dir` + `read_file`**，上传、建目录、删除、改名一律返回 `RemoteErrorKind::Unsupported`。

理由：写操作对 OAuth 系网盘要额外处理"上传会话 / 分片上传 / 冲突重命名"，与核心价值（能浏览、能下载）正交；且夸克已经以只读形态发布，用户侧已有先例。

### 1.3 明确不做

- 不做扩展化（见 §三 3.1）。
- 不做网盘之间的统一 trait 合并（见 §三 3.2）。
- 不在本计划内做批量同步/自动备份（属目录对比子系统，另立计划）。

---

## 二 现状盘点（已验证）

### 2.1 provider 抽象（已统一，不要动）

| 项 | 位置 |
|---|---|
| `RemoteProvider` trait（9 方法，`#[async_trait]` dyn 兼容） | `src-tauri/src/remote/provider.rs:39-183` |
| 刻意**不给**默认实现（占位实现会"编译通过、点了没反应、报告还显示成功"） | `provider.rs:114-117, 144-146, 174-176` |
| trait 对象可用性回归测试 | `provider.rs:261-275` |
| 夸克实现 | `quark_provider.rs:889` |
| 百度实现 | `baidu_provider.rs:637` |
| 错误类型 `RemoteError` / `RemoteErrorKind`（snake_case 稳定） | `src-tauri/src/remote/error.rs:51-59` |
| 传输编排（协议无关） | `src-tauri/src/remote/transfer.rs` |
| 目录对比复用 provider 工厂 | `src-tauri/src/compare/commands.rs:554` |

**能力边界差异（设计基线，新 provider 需在文档中写明）**：
- 夸克**完全只读**：`quark_provider.rs:876` 把上传/建目录/删除/改名全部挡为 `Unsupported`（见文件头 `:16-19`）。
- 百度**不能上传但能写元数据**：`baidu_provider.rs:627` `reject_upload` 只挡上传；`make_dir` / `rename_path` / `remove_path` 是真实现。

### 2.2 凭据存储（可 100% 复用）

| 项 | 位置 |
|---|---|
| 凭据文件 `remote-credentials.json`（app-data 目录） | `credentials.rs:25, 54` |
| DPAPI 加密写 + 原子写 + 坏文件备份 | `credentials.rs:486, 342, 319-328, 184` |
| 读（解不开就报错，不静默当"没存"） | `credentials.rs:461` |
| 键校验（只查非空，无命名格式约束） | `credentials.rs:331` |

**关键约定：`credential_key` == 连接 id（uuid v4 simple）**。前端网盘分支 `credentialKey = id`（`src/stores/storage/remote-connections.ts:382, 411, 431, 449, 465`），测试连接时同样是 `credentialKey: isNetdisk ? form.id : undefined`（`:615`）。夸克/百度**不走** `registry::resolve_connection`，provider 自己按 `credential_key` 读存档（`quark_provider.rs:631-733`、`baidu_provider.rs:408, 501`）。

⇒ 新 provider **沿用同一套约定**，前端 `credentialKey = id` 不变。

### 2.3 会话结构与过期判定（差异最大的地方）

| | 夸克 | 百度 | OneDrive / 阿里云盘（推断） |
|---|---|---|---|
| 会话载体 | cookie blob（`__pus` + `__puus`） | cookie 对（`BDUSS` + `STOKEN`）+ 独立 `bdstoken` | `(access_token, refresh_token, expires_at_unix, scope)` |
| 结构定义 | `quark_auth.rs:484-492` | `baidu_auth.rs:376-384` | 新建（建议 `StoredOAuthSession`） |
| 过期判定 | **启发式**：上次验活 + 7 天信任窗口（`quark_auth.rs:109, 410, 418`） | **真过期**：`stoken_expires_unix` | **真过期**：服务端 `expires_at` + refresh token |
| 鉴权头 | `Cookie` | `Cookie` | `Authorization: Bearer` |
| 直链 | 带 cookie 回源 + `auth_key`（`quark_provider.rs:462-482`） | `filemetas` dlink，**标注"未实证"**（`baidu_api.rs:900-915`） | 自带签名，短时效，**不带 cookie** |

⇒ 夸克那套"启发式过期 + 等 `__puus` 沉降"的 hack（`quark_auth.rs:348` `should_wait_for_puus`）在 OAuth 系**整类消失**。

### 2.4 标识模型（必须先定，否则接口会拧巴）

| provider | 条目标识 | 路径可直查？ |
|---|---|---|
| 夸克 | 只有 `fid`，**无路径** | 否，须从根 `fid="0"` 逐级走 `resolve_dir_fid`（`quark_provider.rs:769`） |
| 百度 | **自带 path** | 是，`find_entry`（`baidu_provider.rs:557`） |
| OneDrive | item id（Graph driveItem id） | 否，需逐级解析 |
| 阿里云盘 | file_id / drive_id | 否，需逐级解析 |

分页语义也不同：夸克是**页码从 1 起**，百度是**偏移量**（`quark_provider.rs:114` vs `baidu_api::next_page`）。新 provider 需在 `*_api.rs` 内自洽，**不要**去统一。

### 2.5 登录形态

两个现有 provider 都是**内嵌 WebView2 + cookie 轮询**，差异只在锚点 cookie、域、探针策略、jar 清理：

| | 夸克 `quark_login.rs` | 百度 `baidu_login.rs` |
|---|---|---|
| 窗口 label | `afm-quark-login`（`:69`） | `afm-baidu-login`（`:54`） |
| 入口 URL | `https://pan.quark.cn/`（`:73-75`） | `https://pan.baidu.com/disk/main`（`:56`） |
| 建窗方式 | **独立 `std::thread`**（`:20-33` 注释：从 IPC 回调里建会死锁） | 同 |
| 锚点 cookie | `__pus`（`quark_auth.rs:81`） | `BDUSS`（`baidu_login.rs:64`） |
| 轮询 | 2s | 2s |
| 额外等待 | `__puus` 晚约 1s，宽限 20s（`quark_auth.rs:100`） | `STOKEN` 晚到，等 20s（`baidu_login.rs:66, 68`） |
| 探针策略 | 只记日志，不当否决（`:46-50`） | 基线同值 ⇒ 一票否决（`:42-45`） |
| 事件频道 | `afm://quark-login` | `afm://baidu-login` |
| 认领 | `remote_quark_login_claim` | `remote_baidu_login_claim` |

**新 provider 若走 OAuth，这套"轮询 cookie"不能照抄**——OAuth 拿到的是一次性 code，不是 cookie。

### 2.6 直链回源

夸克/百度都是"拿直链 → 另一次 GET 直链"，但**鉴权方式不同**：
- 夸克：直链 URL 拼 `auth_key`（`quark_provider.rs:462-482`），GET 时**必须带 cookie**（缺 `__puus` 被 CDN 回 412，`quark_provider.rs:493-502`）。
- 百度：直链是裸 `String`，GET 时带 cookie。
- OneDrive/阿里云盘：**直链自身即授权**，多带 cookie 是多余的。

⇒ 「带 cookie 拉直链」模式对两个新 provider **不适用**。

客户端策略差异（必须对齐，否则 3xx 会被误判成协议错误）：
- API 客户端 `.redirect(Policy::none())` —— **刻意不跟随重定向**，让 3xx 变成可分类的"请重新授权"（`quark_provider.rs:267-278`）。
- 下载客户端 `.redirect(Policy::limited(5))` + 文件超时（`quark_provider.rs:287-298`）。

### 2.7 仓库内零 Microsoft / 阿里代码（已 grep 确认）

| 关键词 | 命中 |
|---|---|
| `graph.microsoft.com` / `login.microsoftonline` / `msgraph` / `pkce` | **0** |
| `aliyundrive` / `openapi.aliyun` / `drive.aliyuncs` | **0** |
| `aliyun` | 仅 README 路线图宣示（`README.md:106`、`README.zh-CN.md:266`），**零代码** |
| `onedrive` | 仅无关命中（Windows 本地 `onedrivetemp` 隐藏目录、`dir_reader` 路径判断、竞品分析文档） |

依赖现状（`src-tauri/Cargo.toml`）：已有 `reqwest 0.13`（rustls + stream）、`serde`/`serde_json`、`base64 0.22`、`tokio 1`、`async-trait 0.1`、`futures-util 0.3`、`tempfile 3`、`windows 0.58`、`dirs 6`。
**缺**：`oauth2` 系列、`openidconnect`、`msal`、设备码流库、`pkce`、`url`（URL 解析）、`sha2`（PKCE 必需）、`hyper`/`axum`（本地回环服务）。

---

## 三 关键设计决策

### 3.1 落核心 `src-tauri`，不落 `extensions/`

扩展沙箱提供的是"扩展能干什么"的能力面（装扩展、跑二进制、HTTP 白名单、文件读写），**不是 provider 抽象**——里面没有 `RemoteProvider`、没有存储协议概念。接入网盘要重写四条主线：

1. `RemoteProvider` trait 实现 → 核心在 `src-tauri/src/remote/provider.rs`；
2. DPAPI 凭据读写 → 核心 `credentials.rs`；
3. `RemoteProtocol` 枚举 + `generate_handler!` → 核心 `lib.rs`；
4. 内嵌 WebView2 登录窗 → 核心 `*_login.rs`。

### 3.2 不动 `RemoteProvider`，也不强行统一 `QuarkApi` / `BaiduApi`

`QuarkApi`（`quark_provider.rs:111`）与 `BaiduApi`（`baidu_provider.rs:77`）**不该合并**：能力集不可统一（百度有 3 个写操作 + `bdstoken`，夸克全无且明确只读）；标识模型不可统一（fid 树 vs path 树，OneDrive item id，阿里云盘 file_id）；直链类型不可统一（结构体 vs 裸串 vs 签名 URL）。

合并的代价：夸克要实现 3 个永远返回 `Unsupported` 的方法——**这正是 `provider.rs` 三处注释反复警告的模式**。

### 3.3 新开窄 trait `OAuthCloudApi`（建议形状，非仓库现状）

OneDrive 与阿里云盘**彼此之间**的共性远大于它们与夸克/百度的共性：

```rust
#[async_trait]
pub trait OAuthCloudApi: Send + Sync {
    async fn list_page(&self, session: &OAuthSession, dir: &str, cursor: Option<&str>)
        -> Result<Vec<CloudEntry>, RemoteError>;
    async fn download_link(&self, session: &OAuthSession, item_id: &str)
        -> Result<CloudDownloadLink, RemoteError>;
    async fn download(&self, session: &OAuthSession, link: &CloudDownloadLink, max_bytes: u64)
        -> Result<Vec<u8>, RemoteError>;
    async fn user_info(&self, session: &OAuthSession) -> Result<CloudUserInfo, RemoteError>;
}
```

`OAuthSession` = `{ access_token, refresh_token, expires_at_unix, scope }`，存进**同一个** `remote-credentials.json`、**同一个** `credentials::save_credential`、**同一个键 = 连接 id**——这三条是真能 100% 复用的资产。

**顺序建议**：先落阿里云盘，用它的实现去反推 trait 的最终形状；OneDrive 作为该 trait 的**第二个**实现接入。这样抽象是被第二个实现"逼"出来的，而不是第一个实现"顺手定"出来的。

### 3.4 先修前端错误文案硬编码（前置清理）

`src/utils/remote-directory.ts:137-147` 按 `kind` 分支返回中文，其中 `auth` / `not_found` / `unsupported` / `unreachable` 四个分支**硬编码"夸克网盘"**。新增第三个 provider 后，百度与新 provider 的会话失效都会显示"夸克网盘会话已失效"。

⇒ Phase 0 先把它参数化（按 provider 传显示名），这是**既有 bug**，不是本次引入，但必须先修。

### 3.5 导出名冲突（E0252）已知坑

`remote/mod.rs:111-140` 记录：夸克/百度有**同名裸函数**（`clamp_page_limit` / `next_page` / `to_remote_error` / `failure_from_http_status` / `sanitize_cookie_value` / `classify_transport` / `encode_query_component` / `parse_list_response`），裸 `pub use` 会撞 **E0252**，必须加前缀别名。新增 provider 若再引入同名函数会再撞一次。

现成的接线清单模板：`quark_provider.rs:2403-2421` 文件末尾「接线清单（由 T5 统一改 `remote/mod.rs`）」——3 步（`mod` 声明 / `pub use` / 命令层 arm）。

---

## 四 分阶段计划

### Phase 0：前置清理与决策就位（0.5 天）

| 编号 | 任务 | 交付 | 验收 |
|---|---|---|---|
| P0-1 | 参数化 `src/utils/remote-directory.ts:137-147` 的 provider 显示名 | 前端改动 + vitest | 百度会话失效不再显示"夸克网盘"；`npm run check` 绿 |
| P0-2 | OneDrive 授权形态可行性验证 | 一份结论备忘（进本文件 §八） | 明确回答"WebView2 能否截获 OAuth redirect 拿到 code" |
| P0-3 | 阿里云盘 OpenAPI 端点与授权流程核实 | 端点清单 + 鉴权字段表 | 有可复现的最小请求（列根目录 + 取直链） |

P0-2 / P0-3 **必须先做**，否则 Phase 3 的架构选型是空中楼阁。P0-2 建议做法：最小 Tauri 窗口 → 打开授权页 → 在 `on_navigation` 里观察 URL 变化。

### Phase 1：阿里云盘（3–5 天）

| 编号 | 任务 | 参考实现 |
|---|---|---|
| P1-1 | `remote/aliyun_api.rs`：端点构造、响应解析、`failure_from_http_status` / `failure_from_code` / `to_remote_error` / `classify_transport` | `baidu_api.rs:642-657` 或 `quark_api.rs:1119-1152, 1269-1279` |
| P1-2 | `remote/aliyun_auth.rs`：`StoredOAuthSession`、save/load/delete（委托 `credentials.rs`）、`is_aliyun_session_blob`、过期判定 | `quark_auth.rs:484-492, 606-649` |
| P1-3 | `remote/aliyun_provider.rs`：`AliyunApi` + `AliyunSessionStore` + `impl RemoteProvider`（9 方法，不支持的返 `Unsupported`） | `quark_provider.rs:111-217, 641-1100` |
| P1-4 | `remote/mod.rs`：`mod` + `pub use`（**前缀别名避 E0252**） | `remote/mod.rs:44-72, 118-158` |
| P1-5 | `remote/commands.rs`：`provider_with_protocol` / `list_with_protocol` / `stat_with_protocol` 三个 arm + 3 条会话命令 | `commands.rs:151, 172, 208`（百度）/ `:256, 284, 320`（夸克） |
| P1-6 | `remote/connection_test.rs`：`probe_inner` arm + `probe_aliyun`；**无凭据必须报 `MissingCredential` 而非 `AuthRejected`** | `connection_test.rs:248-263, 1224-1240` |
| P1-7 | `aliyun_login.rs`：授权窗（**独立 `std::thread`** 建窗）+ 事件推送 + 待认领槽位 + `claim` / `cancel` / `pending` | `quark_login.rs`（42251 B）/ `baidu_login.rs`（43514 B） |
| P1-8 | `lib.rs`：`mod aliyun_login;` + `generate_handler!` 注册 7 条命令 | `lib.rs:11-12, 45, 378-391` |
| P1-9 | 前端 8 个文件（见 §五） | — |
| P1-10 | 测试 6 处（见 §五） | — |

### Phase 2：抽象收敛（0.5 天）

把 P1-3 里阿里云盘的具体形状抽象成 `OAuthCloudApi` + `OAuthSession`（§三 3.3），让 OneDrive 能直接套用。**此步不做也不阻塞 OneDrive**（OneDrive 可以先照抄阿里云盘的三文件结构），但做了能省掉第二次返工。

### Phase 3：OneDrive 可行性验证 → 实现（4–6 天）

先做 P0-2。若验证结论是"WebView2 能拿到 code"：

| 编号 | 任务 | 备注 |
|---|---|---|
| P3-1 | `remote/onedrive_api.rs` | Graph driveItem 列表 + `@microsoft.graph.downloadUrl` 取直链 |
| P3-2 | `remote/onedrive_auth.rs` | `StoredOAuthSession`（access/refresh/expires/scope） |
| P3-3 | `remote/onedrive_provider.rs` | `impl RemoteProvider`（只读） |
| P3-4 | `onedrive_login.rs` | 授权窗 + code 捕获 + refresh |
| P3-5 | 命令/枚举/注册（照 P1-4 ~ P1-8） | — |
| P3-6 | 前端 8 个文件 + i18n 17 个语言包 | — |

若结论是"必须 system browser + PKCE"，则需要**新增依赖**：`sha2`（PKCE）、`url`（URL 解析）、本地回环 HTTP 监听器（`hyper`/`axum` 或 `tiny_http`——后者仓库在 focus-sync 侧车里已有，但**在核心之外**，不能直接复用）、以及 system browser 启动（`open`/`ShellExecute`）。这一分支工作量 +1.5 天，且要新引入一条"无浏览器可用则放弃"的失败路径。

### Phase 4：门禁、真机验收与发布（1–2 天）

1. `cd src-tauri && cargo test --lib`（**必须在 `src-tauri/` 下跑**，仓库根无 `Cargo.toml`；`cargo` 不在 PATH，需 `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin`，`pc-` 不能省）。
2. `npm run check`（= `ts` + `lint:check` + `test:unit:run` + `verify-changelog`）。
3. `npm run tauri:build` → 装机实测（**真机验收唯一合法产物**）。
4. README / README.zh-CN 路线图行更新（`README.md:106`、`README.zh-CN.md:266`）。
5. CHANGELOG + 17 个语言包。

---

## 五 最小改动清单

### 5.1 后端（Rust）

| 序 | 文件 | 动作 |
|---|---|---|
| 1 | `src-tauri/src/remote/connection.rs` | `RemoteProtocol` 加变体；`default_port()`（`:43-51`，网盘 = 443）+ `as_str()`（`:53-62`，snake_case，如 `aliyun_pan` / `onedrive`） |
| 2 | `src-tauri/src/remote/<p>_api.rs`（新） | URL 构造、请求/响应解析、错误映射 |
| 3 | `src-tauri/src/remote/<p>_auth.rs`（新） | 会话结构、save/load/delete（委托 `credentials.rs`）、`is_<p>_session_blob`、过期判定 |
| 4 | `src-tauri/src/remote/<p>_provider.rs`（新） | `<P>Api` trait + `<P>SessionStore` trait + `impl RemoteProvider` |
| 5 | `src-tauri/src/remote/mod.rs` | `mod` 声明（`:20-42`）+ `pub use` 块（`:44-175`），**前缀别名避 E0252** |
| 6 | `src-tauri/src/remote/commands.rs` | ① `provider_with_protocol`（`:742-762`）② `list_with_protocol`（`:470-503`）③ `stat_with_protocol`（`:505-530`）④ 3 条会话命令 |
| 7 | `src-tauri/src/remote/connection_test.rs` | `probe_inner`（`:248-263`）+ `probe_<p>()`；无凭据报 `MissingCredential` |
| 8 | `src-tauri/src/<p>_login.rs`（新） | 内嵌 WebView2 授权窗（独立 `std::thread`！）+ 事件 + 认领槽位 |
| 9 | `src-tauri/src/lib.rs` | `mod <p>_login;`（`:11-12, 45` 旁）+ `generate_handler!` 注册（`:378-391`） |

**自动继承、不必改**：`remote/transfer.rs`、`compare/commands.rs:554`、`remote/registry.rs:135-143`。

### 5.2 前端（TS/Vue）

| 序 | 文件 | 动作 |
|---|---|---|
| 1 | `src/types/remote.ts` | `:15` 枚举加值 + **`:14` 注释"六个变体"要改**；`:148-166` 端口；`:176-199` 加 `is<P>Protocol`、更新 `isNetdiskProtocol`；`:310-314` / `:377+` 登录 payload 镜像 |
| 2 | `src/stores/storage/remote-connections.ts` | `:66-99` host/port 常量 + `fixedNetdiskHost` / `fixedNetdiskPort`；`:339-500` 保存分支（**先 DPAPI 密文后 storage**，`:342-350` 硬约束）；`:587` `isNetdisk` 判定；`:615` `credentialKey`；加 `start<P>Login` / `cancel<P>Login`（`:656, 693`） |
| 3 | `src/modules/settings/ui/categories/remote/netdisk-add-dialog.vue` | `:44` `NetdiskProtocol` 联合类型；`:74` `i18nPrefix`；`:84` `cancelLogin`；`:123-128` 启动；`:250` host；`:292/309` 订阅 `afm://<p>-login`；⚠️ `:14-16`「本组件必须常驻挂载」 |
| 4 | `src/modules/settings/ui/categories/remote/connections-section.vue` | `:181` `startAdd` 分派；`:411-419` 增加按钮（`data-test="add-netdisk-<p>"`） |
| 5 | `src/modules/settings/ui/categories/remote/connection-form.vue` | `:59` `netdiskProtocol` prop；`:406/409` 分支；`:1187-1188` 协议排除列表 |
| 6 | `src/modules/nav-sidebar/components/remote-nav-panel.vue` | `:70-75` `extraProviders` 加条目（`:64-69` 注释：网盘协议不在下拉里） |
| 7 | `src/localization/messages/*.json` | **17 个语言包**：`en, ch, de, es, fa, fr, he, hi, hy, it, ja, pt, ru, sl, tr, ur, vi`。参照 `en.json:690-692`（协议名）、`:718-719`（登录提示）、`:763-819`（provider 文案）；用 `npm run sync-i18n` 同步 |
| 8 | `src/utils/remote-directory.ts` | `:137-147` 兜底文案按 provider 参数化（Phase 0 先修） |

### 5.3 测试

| 序 | 位置 | 内容 |
|---|---|---|
| 1 | `src-tauri/src/remote/<p>_api.rs` / `_auth.rs` / `_provider.rs` 内联 `#[cfg(test)] mod tests` | 零网络 fake 实现 trait（照 `quark_provider.rs:1100+`、`baidu_provider.rs:830+`）。**仓库无 `src-tauri/tests/` 目录** |
| 2 | `src/modules/remote/utils/__tests__/<p>-command-registration.test.ts`（新） | **照抄** `quark-command-registration.test.ts`（读 Rust 源码，断言"定义 / 注册进 `generate_handler!` / 前端有调用点 / 无僵尸命令"）。动机：漏注册 → 真机 "command not found" 但单测全绿 |
| 3 | `src/types/__tests__/remote-protocol-<p>.test.ts` | 照 `remote-protocol-quark.test.ts` |
| 4 | `src/stores/storage/__tests__/remote-connections-<p>.test.ts` | 照 `remote-connections-quark.test.ts` |
| 5 | `src/localization/__tests__/<p>-i18n-keys.test.ts` | 照 `quark-i18n-keys.test.ts` |
| 6 | `src/modules/settings/ui/categories/remote/<p>-connection-form.test.ts` | 照 `quark-connection-form.test.ts` |

### 5.4 文档

| 文件 | 动作 |
|---|---|
| `README.md:106` / `README.zh-CN.md:266` | 「阿里云盘正在路上」路线图行更新 |
| `CHANGELOG.md` | `npm run verify-changelog` 是 `npm run check` 的一环，不改会红 |
| `docs/plan-2026-11-19-aliyun-onedrive.md` | 本文件；Phase 0/2 结论回填 §八 |

---

## 六 风险登记

| # | 风险 | 影响 | 缓解 |
|---|---|---|---|
| R1 | OneDrive 授权形态未知（WebView2 拦截 vs system browser + PKCE） | 决定是否新增 3-4 个依赖 + 本地回环服务 | **P0-2 前置验证**，不验证不排期 |
| R2 | 阿里云盘 OpenAPI 端点/授权流程未核实 | 可能整个 Phase 1 架构需改 | **P0-3 前置核实** |
| R3 | `cargo test --lib` 的 tauri 红线：新 `*_login.rs` 若引用 `tauri::async_runtime::*`，整个测试二进制在加载期以 `STATUS_ENTRYPOINT_NOT_FOUND (0xC0000139)` 崩 | 门禁全红 | 现有两个 `*_login.rs` 都用 `std::thread` 规避（`quark_login.rs:18`、`baidu_login.rs:18`），照抄 |
| R4 | `remote/mod.rs` E0252 导出冲突 | 编译失败 | 前缀别名，照 `remote/mod.rs:111-140` 记录 |
| R5 | `cargo` 不在 PATH | 门禁跑不起来 | 用完整 toolchain 路径，`pc-` 不能省（`AGENTS.md:170`） |
| R6 | vitest 必须排除 `.superpowers/**` 与 `graphify-out/**` | 误报失败 | `AGENTS.md:707-710` |
| R7 | 前端错误文案串台 | 用户看到错误 provider 名字 | Phase 0 先修（§三 3.4） |
| R8 | 微软/阿里可能要求应用注册、审核、回调域名 | 交付时间不可控 | Phase 0 只做技术验证；应用注册与审核作为**独立外部依赖**在计划里显式列出 |
| R9 | 未运行任何构建/测试（本计划基于只读调研） | 清单行号可能与本地未提交改动不一致 | 动手前先 `git status` 核对工作树，逐条复核行号 |

---

## 七 门禁（收口时按序跑）

```powershell
# Rust（必须在 src-tauri/ 下；cargo 不在 PATH）
$env:PATH = 'C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;' + $env:PATH
Set-Location src-tauri; cargo test --lib

# 前端（仓库默认提交前闸门）
npm run check          # = ts + lint:check + test:unit:run + verify-changelog

# 真机验收唯一合法产物
npm run tauri:build
```

补充约束：
- 新增的 `*_login.rs` **不得**出现 `tauri::AppHandle` / `tauri::async_runtime::*`。
- `npm run check` 必须**逐字** `No issues found!`；多包仓库要**逐包**跑 analyze，info 级 lint 也是真缺陷。
- 版本/身份认定以 **sha** 为准；不得扫 exe ASCII 字符串判版本（`AGENTS.md` §2.10）。

---

## 八 待验证问题（本文件内为"未判定"，动手前必须回填）

| # | 问题 | 当前状态 | 回填时机 |
|---|---|---|---|
| Q1 | 内嵌 WebView2 能否在授权页重定向时截获 code / token？ | **未判定** | P0-2 |
| Q2 | 阿里云盘 OpenAPI 的当前授权方式（设备码 / 扫码）与端点清单？ | **未判定** | P0-3 |
| Q3 | 阿里云盘与 OneDrive 的直链是否都自带签名、不需要回源带 cookie？ | **推断**（基于与百度 filemetas 的结构相似性） | P1-1 实测 |
| Q4 | OneDrive 是否需要 `offline_access` scope 才能拿到 refresh_token？ | **未判定** | P3-4 |
| Q5 | 两个新 provider 的「上传」是否可以推迟到第二版？ | **本计划决策：推迟** | Phase 4 复盘 |
| Q6 | 阿里云盘 / OneDrive 是否需要独立的"根目录"概念（阿里云盘有 drive_id 根）？ | **未判定** | P1-1 |

---

## 九 一句话交接

> 阿里云盘先行（结构贴近百度、无需 PKCE），OneDrive 随后（先做 0.5 天 WebView2 授权可行性验证再定架构），两者共用一条新开的窄 OAuth trait 与既有 DPAPI 凭据/键约定，第一版只做只读；落地前先修前端错误文案的"夸克网盘"硬编码串台。
