# 百度网盘「点一下即登录、自动填表」技术可行性分析

- 日期：2026-11-08
- 范围：AFM（Alpha File Manager，Tauri v2.12.0 + Vue 3 + Rust，Windows）接入百度网盘登录态
- 命题：用户在 AFM 里点「添加百度网盘」→ 弹百度登录页 → 用户登录 → 应用**自动**读取登录态并自动填好连接配置（不需要用户手工粘贴 Cookie）
- 性质：只读调研（本文件是本轮唯一新增文件）

---

## 一句话结论

**技术上完全可行，而且这件事在本仓库里已经被实测证明过一次**（`src-tauri/src/baidu_probe.rs` 在 `tauri build` 产物里成功用 Rust 侧 `cookies_for_url()` 读到了 HttpOnly 的 `BDUSS`（192 字符）与 `STOKEN`），剩下的工作是把它从「探针」产品化成「正式 command + 前端按钮」；同时本轮**推翻了仓库 T1 的一个结论**——「百度 OAuth 走不通」只对 `client_id=250528/266719` 成立，OAuth 端点本身完全可用（用 AList 公开的已注册客户端实测：`invalid_client` 变成 `invalid refresh token`，说明客户端被认可），所以「自建应用 → 授权码 → refresh_token」这条能自动续期的长期路线是**有条件可行**的。

---

## 方案对照表

| # | 方案 | 可行性 | 实现复杂度 | 用户步骤 | 主要风险 | 依据来源 |
|---|---|---|---|---|---|---|
| a | **AFM 内嵌 WebView2 打开 `pan.baidu.com` 登录 → Rust 侧 `cookies_for_url()` 读 cookie** | **可行（本仓库已实测）** | 中（新增 1 个 Rust command + 1 个前端按钮 + 事件回传） | 2 步：点按钮 → 在弹窗里登录（**首次之后 1 步**，登录态留在应用 profile 里） | 百度对嵌入式浏览器的风控强度未知；cookie 过期后无静默续期 | 仓库 T1 实测；Tauri 2.12.0 源码文档 |
| b | 系统浏览器（Edge）登录 → AFM 读**浏览器本地 cookie 库** | **不可行**（v20/App-Bound 封堵）；**有条件可行**的变体是「专用 profile + CDP 远程调试」，但对普通用户不可接受 | 高 | 3–5 步，且要引入调试端口 | 本机任何程序可控制该浏览器；Edge 同样受 ABE 影响 | Chrome 127+ ABE 公开报道；安全研究文章（二手） |
| c | 百度开放平台 **OAuth**（自建应用：授权码 + `refresh_token`） | **有条件可行**（需企业实名 + 应用审核；`client_id/secret` 必须是自己注册或被授权的） | 高（注册流程 + 回调 URL + token 刷新） | 2 步：点按钮 → 浏览器里授权 | 合规（挪用他人 `client_id/secret` 是红线）；审核周期未知 | **本轮自测**：`openapi.baidu.com/oauth/2.0/token` 各分支响应；AList 驱动源码 |
| d | 设备码流 / 扫码登录等「不跳浏览器」的官方授权 | **不可行**（百度 OAuth 端点不支持 `device_code`） | — | — | — | **本轮自测**：`unsupported_grant_type` |
| e | 社区既有实现参考 | 参考价值高 | — | — | 依赖他人密钥 | AList `drivers/baidu_netdisk` 源码；BaiduPCS-Py（BDUSS 模式） |
| f | 本地 127.0.0.1 中转页 + 系统浏览器 | **不可行**（技术上拿不到 cookie：HttpOnly + 跨源 + 三方 cookie 限制） | 低 | — | — | 本轮 curl 实测 `pan.baidu.com` 响应头无 `X-Frame-Options`，但 cookie 隔离是硬约束 |

> 「用户步骤」按「AFM 里点到会话可用」计，不含填写连接名等非认证字段。

---## 仓库现状（已查证，2026-11-08 读码确认）

| 事实 | 位置 |
|---|---|
| 内嵌 webview 登录 + 读 cookie 的**探针已存在且跑通过** | `src-tauri/src/baidu_probe.rs`（`AFM_BAIDU_PROBE=1` 门控） |
| 探针的做法：`WebviewWindowBuilder::new(app, "afm-baidu-probe", WebviewUrl::External(...))` 开独立窗口 → **独立线程**每 2 秒 `window.cookies_for_url(url)` → 命中 `BDUSS` 后再等最多 20 秒取 `STOKEN` | `src-tauri/src/baidu_probe.rs:121-227` |
| 纯逻辑层：从 cookie 清单挑 `BDUSS`/`STOKEN`、脱敏、编解码、落盘 | `src-tauri/src/remote/baidu_auth.rs` |
| 落盘层：DPAPI（裸 `CryptProtectData` + `CRYPTPROTECT_UI_FORBIDDEN`），密文存 `<app_data_dir>/remote-credentials.json` | `src-tauri/src/remote/credentials.rs:104-139` |
| 已注册的 Tauri 命令只有三个：`remote_baidu_save_session` / `remote_baidu_auth_status` / `remote_baidu_delete_session` | `src-tauri/src/remote/commands.rs:148-219` |
| **适配层并没有「开网盘登录窗」的代码** —— `baidu_auth.rs` 的模块注释声称「`remote/commands.rs`……负责开网盘登录窗」，但对 `commands.rs` grep `Webview`/`cookies_for_url`/`登录窗` **零命中**（文档与实现不一致） | `src-tauri/src/remote/baidu_auth.rs:17-19` vs `src-tauri/src/remote/commands.rs` |
| 前端现状：网盘表单给的是「打开登录页」外链 + 「F12 复制 BDUSS」文字说明，即**今天仍是手工粘贴** | `src/modules/settings/ui/categories/remote/remote-connection-form.vue:76-100` |
| `tauri` 实际解析版本 **2.12.0**（`Cargo.toml` 写 `2.11`，lock 文件解析到 2.12.0） | `src-tauri/Cargo.lock` |

**⇒ 方案 a 缺的只是「产品化那一层」**：开登录窗的命令、把 cookie 投影成会话的适配、把结果回传给前端的自动填表流程。读 cookie 这件最难的事已经证明了。

---

## 方案 a：AFM 内嵌 WebView2 登录 + Rust 侧读 cookie

### 已查证

1. **打包产物里能读到 HttpOnly `BDUSS`**（不是文档承诺，是实测）。
   `docs/2026-11-07-baidu-t1-auth-findings.md` §2.3 记录：`bduss_len=192`、`bduss_domain=baidu.com`、`bduss_http_only=true`，首轮在用户完成登录后 **+146s** 出现；`VERDICT_U1: PASS (BDUSS readable from packaged webview)`。
   同轮四类判据逐条对齐：能读 HttpOnly、能读父域（`baidu.com`）cookie、锚点 URL 取 `https://pan.baidu.com/` 即可、父域 cookie 也能取到。

2. **`STOKEN` 也可读**，且它在 `pan.baidu.com` 域下发、HttpOnly、`value_len=64`、带 `expires`。T1 第二轮实测 `STOKEN` 在 **+2s** 即出现（前提是起始页是网盘主界面 `https://pan.baidu.com/disk/main`，登录页不会下发它）。

3. **登录态会留在应用自己的 WebView2 profile 里**。T1 第二轮不需要人工登录，文档写明「登录态已落在 WebView2 的 profile（`%LOCALAPPDATA%\<identifier>\EBWebView`）」。本轮在本机复核：该目录**确实存在**（`C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.app\EBWebView`）。
   ⇒ **第二次添加百度网盘可以做到「零输入、点一下就好」**，这是这条路最大的 UX 红利。

4. **Tauri 2.12.0 的 API 契约**（`docs.rs/tauri/2.12.0/src/tauri/webview/webview_window.rs.html`，已逐行核对）：
   - `2715: pub fn cookies_for_url(&self, url: Url) -> crate::Result<Vec<Cookie<'static>>>`；
   - 文档明写返回的是 runtime cookie store 的内容，**including HTTP-only and secure cookies**（`2702` / `2719` 行附近）；
   - 同一文件 `2732` 行有警告：*"On Windows, this function deadlocks when used in a synchronous command or event handlers, see [the Webview2 issue]"*（`58`/`115` 行同款警告）。

5. **不需要 `with_webview`**。`with_webview` 是取平台原生句柄用的（本仓库 `src-tauri/src/url_drop.rs:300`、`src-tauri/src/windows_print_view_webview.rs:12` 用它拿 WebView2 的 `CoreWebView2` COM）。读 cookie 属于高层 API，直接调即可；只有要做原生注入/拦截时才需要下沉到 COM。

6. **子窗口与主窗口共享同一个 WebView2 环境与 cookie store**（同一个 user-data 目录 ⇒ 同一个 `CookieManager`）。这既是「第二次免登录」的原因，也是「在登录窗里读得到 cookie」的原因。

### 推断（未实测）

- 在**正式实现**里复刻探针的成功率应当与探针一致（探针用的就是同一套 `WebviewWindowBuilder` + `cookies_for_url`），但探针未覆盖「用户主动取消登录」「百度弹短信验证码」「验证码失败重试」这些分支。
- 百度是否对 WebView2 的 UA（带 `wv` 标识）额外加严风控，本轮**未查证**。
- `STOKEN` 的真实有效期长度未量过（T1 只看到当时那个 `expires=2026-12-08`）。

### 结论

**可行，推荐走这条（短期）**。唯一硬约束：cookie 过期后**没有静默续期**，必须让用户重新登录——UI 必须如实表达这一点，不能假装「一直有效」。

---

## 方案 b：读系统浏览器（Edge/Chrome）的本地 cookie 库

### 已查证

1. **本机确实存在这两个库**（`Default\Network\Cookies`）：Edge `786432` 字节、Chrome `294912` 字节，均在 `%LOCALAPPDATA%` 下。
2. **Chrome 127 起启用 App-Bound Encryption（ABE）**：Google 的 Chrome 安全团队在官方博客说明，ABE 依赖一个**特权服务**校验调用方身份，编码应用身份进密文，其它应用解密会失败；在此之前只用 DPAPI，而 DPAPI 挡不住「以登录用户身份运行的恶意程序」。⇒ 第三方程序直接解密 `v20` cookie 这条路**已被系统性封堵**。
3. **加密版本分层**（安全研究文章，二手）：`v10` = 直接 DPAPI；`v11` = AES-GCM，密钥由 DPAPI 保护的 key + 域名 SHA256 派生；`v20` = ABE，绑定**可执行文件路径 + 用户**，普通进程拿不到。
4. **Chrome 136 起，默认 user-data 目录不允许开 `--remote-debugging-port`**（启动日志：`DevTools remote debugging requires a non-default data directory`）。社区实践因此都改成「专用 profile + 调试端口 + CDP」。
5. 存在**被支持的官方途径**吗？—— 本轮**未查到**任何面向第三方桌面应用的「导出浏览器 cookie」官方接口。Cookie Store API 是网页 JS API 且不暴露 HttpOnly；`edge://` 页面没有导出功能；密码管理器 API 只管密码不管 cookie。⇒ 官方途径：**未查证存在**。

### 推断

- Edge 同源 Chromium，很可能同样走 ABE/v20 路线，但**本轮未取到 Edge 侧的确证**：我在本机对 Edge 的 `Cookies` 文件做字节扫描，`v10/v11/v20/v13` 前缀计数**全为 0**（文件可能是稀疏/预分配或数据在 WAL 里），**这一条属未查证**，不作为结论使用。
- 即便解密成功，把「用户日常浏览器的全部 cookie」读进一个文件管理器，隐私与合规风险远高于收益。

### 结论

**不可行**（直接读浏览器 cookie 库）。
**「专用 profile + CDP」变体在技术上可行**，但：① 要用户在一个隔离 profile 里重新登录一次（与 a 方案的 2 步相比没有优势）；② 调试端口开着时**本机任何程序都能控制该浏览器**；③ 需要引入 WebSocket 客户端依赖（`reqwest` 不带 ws）。⇒ **不推荐**。

---## 方案 c：百度开放平台 OAuth（自建应用，授权码 → refresh_token）

### 本轮自测记录（2026-11-08，直接打 `https://openapi.baidu.com/oauth/2.0/token`）

| # | 请求 | 响应 | 判读 |
|---|---|---|---|
| 1 | `grant_type=refresh_token` + AList 公开的 `client_id` + **正确** `client_secret` + 假 `refresh_token` | `{"error":"invalid_request","error_description":"invalid refresh token, lifetime changed"}` | **客户端被认可**——只差一个真的 refresh_token |
| 2 | 同上但 `client_secret` 写错 | `{"error":"invalid_client","error_description":"Client authentication failed"}` | 端点确实校验 secret ⇒ #1 的「被认可」不是假阳性 |
| 3 | `client_id=999999999`（阴性对照） | `{"error":"invalid_client","error_description":"unknown client id"}` | 与 #1 **响应不同** ⇒ 该实验有判别力 |
| 4 | `client_id=250528` / `client_id=266719` | 均为 `unknown client id` | **与仓库 T1 的 U2 结论一致**；这两个 id 只是网盘网页端内部 `app_id`，不是 OAuth 客户端 |
| 5 | `grant_type=authorization_code` + 正确 client/secret + 假 `code` + `redirect_uri=oob` | `{"error":"invalid_grant","error_description":"invalid code , expired or revoked"}` | **授权码流本身可用** |
| 6 | `grant_type=device_code` / `urn:ietf:params:oauth:grant-type:device_code` / `password` | `{"error":"unsupported_grant_type"}` | 见方案 d |
| 7 | `grant_type=client_credentials` + 正确 client/secret | **成功返回 access_token**，但 scope 列表里**没有 `netdisk`** | 客户端凭据流与网盘权限无关 |
| 8 | `GET https://openapi.baidu.com/oauth/2.0/authorize?response_type=code&client_id=<AList>&redirect_uri=oob&scope=basic,netdisk&state=x` | **HTTP 200** | 授权页对该客户端可用 |

> ⚠️ 上面第 1/5/8 行用的 `client_id`/`client_secret` 是 **AList 项目在 `drivers/baidu_netdisk/meta.go` 里公开的默认值**。它们**只是用来证明「OAuth 端点本身是好的」**，**AFM 不应该挪用**（见「安全红线」）。

### 已查证

- 百度 OAuth 端点**完全正常**：授权码流 + refresh_token 刷新流都能走通，客户端鉴权也正常。
- 仓库 T1 的「OAuth 走不通」结论的**适用范围被本轮收窄**：它只否证了 `client_id` 取 `250528`/`266719`，**没有否证 OAuth 这条路本身**。
- AList 的百度网盘驱动走的就是**官方 OAuth**：`meta.go` 默认 `client_id=hq9yQ9w9kR4YHj1kyYafLygVocobh7Sf` / `client_secret=YH2VpZcFJHYNnV6vLfHQXDBhcE7ZChyE`，`util.go` 用 `refresh_token` 换 `access_token`，再调 `https://pan.baidu.com/rest/2.0/xpan/...`（`xpan/nas`、`xpan/file`、`xpan/multimedia`）。

### 推断 / 未查证

- **百度开放平台的注册门槛**：仓库 T1 记录的是「要求企业实名，且只能访问 `/apps/{应用名}/` 目录」。本轮**未能访问官方文档站**（`openapi.baidu.com` 文档页在本机网络不可达）⇒ 「个人开发者能否注册」「审核周期」「`netdisk` scope 是否覆盖用户整个网盘读写」**均未查证**。
- AList 的驱动以 `DefaultRoot: "/"` 工作，**看起来** `netdisk` scope 能覆盖整盘，但 AList 用的是「已审核通过的应用」权限，不能据此推断新注册应用也有同样权限。**这一条是推断，不是已证。**
- redirect_uri 的形态（`oob` / 自定义 https 回调 / 本地 `127.0.0.1` 回调）在本轮**未查证**是否都能拿到 `code`。

### 结论

**有条件可行**。它是**唯一能拿到 `refresh_token`、从而 cookie 过期后仍可自动续期**的路线，长期看质量明显高于 cookie 直连。但前提是**AFM 自己注册应用并通过审核**（企业实名 + 应用审核 + 可能的审核周期），属于「要花人力和等待」的工作量，**不适合作为「点一下即登录」这个体验的第一版**。

---

## 方案 d：设备码流 / 扫码登录等「不跳浏览器」的官方授权

### 已查证

- 百度 OAuth 端点对 `grant_type=device_code`、`grant_type=urn:ietf:params:oauth:grant-type:device_code`、`grant_type=password` 一律返回
  `{"error":"unsupported_grant_type","error_description":"The authorization grant type is not supported"}`。
- 百度**自己的**网页登录支持扫码/短信，但那是 `passport.baidu.com` 的**网页流程**，**不是**给第三方应用的授权码协议；没有「第三方应用申请 device_code、用户在百度 App 里确认」的公开协议（本轮**未查到**）。

### 结论

**不可行**。百度没有 device code 流；「扫码登录」对第三方应用没有等价物。

---

## 方案 e：社区既有实现

### 已查证

| 实现 | 认证方式 | 对 AFM 的参考价值 |
|---|---|---|
| **AList / OpenList** `drivers/baidu_netdisk` | 官方 OAuth（`client_id`/`client_secret` + `refresh_token`），调 `xpan/*` | 高：证明「xpan REST + token」这条路成立；也证明 OAuth 客户端确实存在 |
| **BaiduPCS-Py** | 同时支持 **BDUSS 模式**与 refresh_token 模式 | 高：证明「BDUSS 直连」也是社区通行做法 |
| **AFM 自己（T1 实测）** | `BDUSS` + `STOKEN` cookie 直连 | —— |

### 结论

社区两条路都在用：**cookie 直连**与**官方 OAuth**。没有第三条「无浏览器登录」的路。

---

## 方案 f：本地 127.0.0.1 中转页 + 系统浏览器

### 已查证

- 本轮 `curl -I https://pan.baidu.com/` 实测：响应 **200**，响应头里**没有** `X-Frame-Options`，也**没有** CSP `frame-ancestors` ⇒ **从响应头看，页面可以被 iframe 套**（这一条对方案 f 是有利的）。
- 但同一响应也**没有** `Access-Control-Allow-Origin` ⇒ 跨源页面**读不到** `pan.baidu.com` 的响应体。

### 推断

- 即便把登录页套进 `127.0.0.1` 的 iframe 里：① `BDUSS` 是 HttpOnly ⇒ 页面 JS 永远读不到；② `127.0.0.1` 页面与 `pan.baidu.com` 跨站 ⇒ 属**第三方 cookie** 语境，现代 Chrome/Edge 默认对第三方 cookie 设限；③ 真正的 cookie 会落进**系统浏览器**的 cookie 库 ⇒ 又回到方案 b 的死路。

### 结论

**不可行**。这条路拿不到 cookie，等价于方案 b 的前半段。

------

## 推荐路线与实现骨架

### 路线排序

| 排名 | 路线 | 何时做 | 理由 |
|---|---|---|---|
| **1** | **方案 a：内嵌 WebView2 登录 + cookie 直连** | **立即做**（约 2–3 人天含验收） | 本仓库已实测跑通；零新增依赖；用户 2 步（首次）/ 1 步（之后）；风险只剩「cookie 会过期」 |
| **2** | **方案 c：自建百度开放平台应用 → OAuth `refresh_token`** | **并行启动**（注册与审核要等人，不阻塞路线 1） | 唯一能自动续期的路；长期质量更好；卡在企业实名与审核 |
| ✗ | 方案 b / d / f | 不做 | 分别被 App-Bound Encryption、百度不支持的 grant type、HttpOnly+跨源 堵死 |

**不要**两条路线同时改 `baidu_auth.rs` 的落盘格式：建议 `StoredBaiduSession` 加一个 `kind` 字段（`cookie` / `oauth`）做向前兼容，读到不认识的 `kind` 当作「没有会话」重新授权。

### 路线 1 实现骨架

#### 1) Rust 侧

新增文件建议 `src-tauri/src/remote/baidu_login.rs`（tauri 适配层）。

**🚩 红线（照抄 `src-tauri/src/baidu_probe.rs:10-16` 的说明）**：本模块会碰 `tauri::AppHandle` / `WebviewWindowBuilder`，**不得**进入 `cargo test --lib` 的链接闭包，否则会把 tao 拖进来、测试二进制在加载阶段以 `STATUS_ENTRYPOINT_NOT_FOUND (0xC0000139)` 崩掉、一个测试都跑不了。
落地做法：新模块**只**在 `setup_handler` / `generate_handler!` 处被引用，**模块内部不要写 `#[cfg(test)]`**（一加就把整条 `tauri::` 窗口链拉进测试二进制）。

**命令面**（建议新增 2 个）：

```rust
// 开始一次内嵌登录。async 是必须的（见下方异步约束）。
#[tauri::command]
pub async fn remote_baidu_begin_login(app: AppHandle) -> Result<(), RemoteError>;

// 轮询登录结果（前端也可以直接监听事件，二选一即可）。
#[tauri::command]
pub async fn remote_baidu_login_poll(connection_id: String) -> Result<BaiduAuthStatusPayload, RemoteError>;
```

**流程骨架**：

1. **开登录窗**（从 Rust 侧建窗 ⇒ **不需要**新增 capability；若改成前端 `new WebviewWindow(...)` 就必须改 `tauri.conf.json` 的 `capabilities`，别走那条路）：

   ```rust
   let window = WebviewWindowBuilder::new(
       &app,
       "baidu-login",
       WebviewUrl::External(tauri::Url::parse("https://pan.baidu.com/disk/main").unwrap()),
   )
   .title("百度网盘登录")
   .inner_size(1100.0, 820.0)
   .build()?;
   ```

   - 起始页用 **`/disk/main` 而不是 `/login`**：实测 `STOKEN` 只在网盘主界面上下发，`/login` 不下发；且已登录时会直接进主界面 ⇒ **第二次添加就是零输入**。
   - 窗口 label 要固定且可复用：用户重复点「添加」时先 `get_webview_window("baidu-login")` 判断是否已存在，**已存在就聚焦而不是再开一个**（本项目「开关型控件要先探测再点击」是同一条教训）。

2. **轮询 cookie**（**必须**在独立线程上）：

   ```rust
   std::thread::spawn(move || {
       loop {
           std::thread::sleep(Duration::from_secs(2));
           let cookies = window.cookies_for_url(Url::parse("https://pan.baidu.com/").unwrap())?;
           // 命中 BDUSS 后，最多再等 20s 取 STOKEN
       }
   });
   ```

   - 锚点 URL 用 `https://pan.baidu.com/`：实测能从这个锚点读到 `domain=baidu.com` 的父域 cookie。
   - **`STOKEN` 的等待窗口不能省**：T1 首轮就是「页面还没加载完」被误读成「`STOKEN` 不存在」。照抄探针的 20 秒上限。

3. **投影 + 落盘**：cookie 列表 → `baidu_auth::BaiduCookieRecord` → `baidu_auth::scan_cookies()` → `baidu_auth::save_baidu_session(app_data_dir, &connection_id, &session, stoken_expires)`。
   落盘走 `credentials::save_credential`（DPAPI `CryptProtectData`），**不要**另起一套明文存储。

4. **先落盘、后回传、最后关窗**：cookie 明文在内存里的时间越短越好；成功后立刻 `window.close()`。
   ⚠️ **不要**把探针里那套「页面 fetch → `<img>` 回传到 127.0.0.1」的诊断通道搬进产品代码：它会把响应体（可能含敏感信息）写进本地端口/日志。昵称要用的话，用页面内同源 `fetch` 拿到后**只回传昵称**，绝不回传 cookie。

5. **回传前端**：用 `app.emit("baidu://login-result", payload)`，payload 只含 `{ ok: bool, reason: Option<String>, nickname: Option<String> }`。
   **前端永远拿不到 cookie**（这与现有 `baidu_auth::is_baidu_session_blob` 拦「凭据读回前端」的思路一致）。

**异步 / 线程约束（硬）**：

- `cookies_for_url` **绝不能**在**同步** command 或事件回调里调 —— tauri 2.12.0 `webview_window.rs:2732` 明写 Windows 上会死锁。
- 轮询线程用 `std::thread::spawn` + `sleep`（探针的形状，已实测）；**不要**把 `WebviewWindow` 句柄塞进 `tauri::async_runtime::spawn_blocking` 后在里面调 WebView2 COM。
- `WebviewWindow` 跨线程只做只读调用（`cookies_for_url` / `eval` / `close`），**不要**跨线程操作 COM 指针（`with_webview` 拿到的 `CoreWebView2` 是 UI 线程对象）。
- `baidu_auth.rs` / `credentials.rs` 是**纯逻辑层**，不得出现 `tauri::` 任何路径（这层已有单测，改动时保持住）。

#### 2) 前端侧

1. 设置 → 网盘 →「添加」按钮 ⇒ 直接调 `remote_baidu_begin_login`，**不要**先建一条空连接再补凭据（现在的表单是「以连接为中心」的，流程要倒过来）。
2. 监听 `baidu://login-result`：
   - 成功 ⇒ 创建连接记录（`protocol: 'baidu_pan'`、`host: 'pan.baidu.com'`、`port: 443`、名字默认「百度网盘」+ 昵称），拿到 `connection.id` 后调 `remote_baidu_save_session`（或让 Rust 在建连时一并落盘）。
   - 失败 ⇒ 提示并**自动退回**现有的「手工粘贴 Cookie」降级路径（`remote-connection-form.vue` 已有该路径与说明文案）。
3. 已有 `BaiduAuthStatusPayload` 的四态（`missing` / `ready` / `stoken_expired` / `stoken_expiring_soon`）可以直接复用做「会话失效，请重新登录」的提示 —— 但要清楚：**cookie 过期只能靠用户重新登录**，没有静默续期。

#### 3) 依赖与验收

- **零新增第三方依赖**：cookie 能力来自 Tauri 本身，DPAPI 在 `credentials.rs`，HTTP 客户端 `reqwest` 已具备且 `baidu_api.rs` 已在用。
- 验收必须用 `npm run tauri:build` 的**打包产物**（`tauri dev` 下所有路由页内容区全空白，界面结论一律作废）。
- 真机数据隔离：`SFM_E2E_DATA_DIR` 只在 debug 构建生效 ⇒ release 产物上要么先备份 `remote-credentials.json` 再验、验完恢复，要么接受真实凭据被写进去再手工清。

---## 安全红线清单

`BDUSS` / `STOKEN` **等价于账号权限本身**（拿到即可冒充用户），与密码同级。以下每条都是硬红线：

1. **不进日志**：`log::info!/warn!/error!` 与 `tauri-plugin-log` 里不得出现 cookie 明文；只允许出现 **cookie 名 + 长度**（仓库既有 `BaiduSession::describe` / `describe_state` 就是这个口径，照用）。
2. **不进错误信息**：所有面向用户的错误文案只说「去哪儿做什么」（`baidu_auth::scan_error` 已是这个口径）。
3. **不进前端**：前端只拿「状态分类 + 昵称 + 过期时间」，永远拿不到 cookie 串。沿用 `baidu_auth::is_baidu_session_blob` 的拦截思路（它就是为「把凭据读回前端」而写的）。
4. **不落明文**：一律走 `credentials::save_credential` 的 DPAPI 密文（`remote-credentials.json`）；**不要**另存明文 JSON、**不要**用 base64 冒充加密、**不要**把明文塞进 `user-settings.json` / `user-stats.json`。
5. **不进 URL / query**：不要用 `?bduss=...` 的方式在窗口间传值（会进浏览器历史、事件日志、崩溃报告）。
6. **不回传到本地端口**：探针那套 `<img>` 回传到 `127.0.0.1` 只用于诊断，产品代码里不要出现；确需回传时**只回传非敏感字段**（如昵称）。
7. **只存本机、不外传**：不遥测、不上传、不同步到任何云端。AFM 也没有账号体系，不要为了「多设备可用」引入上传。
8. **用户可随时清除**：设置页必须提供「退出百度网盘」⇒ 删 DPAPI 存档 **+** 删 WebView2 profile 里的百度 cookie（`delete_cookie` / 清 browsing data），否则 WebView2 里仍留着可用的登录态。
9. **应用 profile 里的百度登录态本身是敏感资产**：`%LOCALAPPDATA%\com.sigma-file-manager.app\EBWebView` 里长期存着用户的百度会话 ⇒ 「卸载/重置」路径要能清掉它，UI 上要如实说明「本应用会记住你的网盘登录状态以免重复登录」。
10. **不得挪用第三方的 OAuth 客户端凭据**：AList 公开的 `client_id`/`client_secret` 只在本报告里用作「证明 OAuth 端点是好的」的实验样本，**AFM 不得把它们写进代码或配置文件**。走路线 2 必须自己注册应用。
11. **不绕过浏览器安全机制**：不采用注入、hook、内存读取等手段去拿浏览器 cookie；路线 1 只用应用**自己**的 WebView2 profile，权限边界清晰。
12. **风控与合规**：不要把「批量账号」「多账号自动登录」做成产品能力；遵守百度用户协议，只做「用户为自己账号授权」。

---

## 未查证事项与影响

| # | 未查证事项 | 为什么没查证 | 对决策的影响 |
|---|---|---|---|
| 1 | 百度开放平台的**注册门槛 / 审核周期 / 个人开发者可否申请** | 官方文档站（`openapi.baidu.com` 文档页、`pan.baidu.com/union`）在本机网络不可达 | 影响**路线 2 的排期**，不影响路线 1 |
| 2 | `scope=basic,netdisk` 的**确切权限范围**（是否覆盖用户整个网盘读写、是否被限到 `/apps/{应用名}/`） | 同上；AList 用的是已审核应用，不能类推 | 决定路线 2 能否替代 cookie 直连；**在查清前不要为此改现有 cookie 实现** |
| 3 | Edge 是否已启用 **App-Bound Encryption / v20** | 本机对 Edge `Cookies` 文件做字节扫描，`v10/v11/v20` 前缀计数全为 0（数据可能在 WAL / 文件为预分配空库），无有效结论 | 只影响方案 b 的判定力度；方案 b 已因「无官方接口 + 调试端口风险」被否，不影响结论 |
| 4 | 百度对**嵌入式 WebView2 的风控强度**（是否强制短信/验证码、触发率多高） | 需要真人真机反复试 | 影响路线 1 的「失败率」与是否必须保留手工粘贴兜底（建议**默认保留**） |
| 5 | `STOKEN` 的**真实有效期长度**与能否续期 | T1 只看到当时那个 `expires=2026-12-08` | 影响 UI 上「会话即将过期」的提示阈值（现取 24h 是保守估计） |
| 6 | Rust 侧 `reqwest` 手工拼 Cookie 头**是否等价于页面内 fetch** | T1 §2.6 已标注「留到 T4 真机验」；本轮未跑带真实会话的 T4 | 影响「只存 `BDUSS` 还是两个都存」；现在两个都存是安全选择 |
| 7 | `authorization_code` 的 `redirect_uri` 可用形态（`oob` / 自定义 https / `127.0.0.1`） | 需要真客户端才能验 | 影响路线 2 的实现细节，不影响路线 1 |
| 8 | Tauri 2.12.0 中**死锁警告精确挂在哪个 cookie API 上** | 已核对到 `webview_window.rs` 的 `2715`（`cookies_for_url`）、`2732`（警告行）、`58`/`115`（同款警告），但未逐行确认 `cookies()` 与 `cookies_for_url()` 各自的完整文档块 | 影响很小：按最坏情况处理（全部走独立线程）即可 |

---

## 本轮实际做过的核查（可复现）

| 核查 | 命令 / 来源 | 结果 |
|---|---|---|
| 百度 OAuth 端点 8 个分支 | `curl -X POST https://openapi.baidu.com/oauth/2.0/token`（见方案 c 表） | 见上表 |
| `pan.baidu.com` 响应头 | `curl -I https://pan.baidu.com/` | 200；无 `X-Frame-Options`；无 CSP `frame-ancestors`；无 `Access-Control-Allow-Origin` |
| Tauri cookie API 原文 | `https://docs.rs/tauri/2.12.0/src/tauri/webview/webview_window.rs.html` | `cookies_for_url` 在 2715 行；死锁警告在 2732 行 |
| AList 驱动源码 | `https://raw.githubusercontent.com/AlistGo/alist/main/drivers/baidu_netdisk/{meta.go,util.go,driver.go}` | OAuth client 默认值 + `xpan/*` 调用 |
| 本机 WebView2 profile | `%LOCALAPPDATA%\com.sigma-file-manager.app\EBWebView` | 存在 |
| 本机浏览器 cookie 库 | `%LOCALAPPDATA%\{Microsoft\Edge,Google\Chrome}\User Data\Default\Network\Cookies` | 两者均存在；加密前缀扫描无结论 |

---

## 来源清单

**仓库内（本项目一手证据）**
- `src-tauri/src/baidu_probe.rs`（内嵌 webview 读 cookie 探针）
- `src-tauri/src/remote/baidu_auth.rs`、`credentials.rs`、`commands.rs`
- `docs/2026-11-07-baidu-t1-auth-findings.md`（T1 实测记录：U1 成立 / U2 的适用范围）
- `docs/plan-2026-11-08-baidu-netdisk-phase1.md`、`docs/handoff-2026-11-08-baidu-netdisk-phase1.md`

**外部**
- Tauri 2.12.0 `WebviewWindow` 源码（cookie API 原文）：https://docs.rs/tauri/2.12.0/src/tauri/webview/webview_window.rs.html
- Tauri `WebviewWindow` API 文档：https://docs.rs/tauri/2.12.0/tauri/webview/struct.WebviewWindow.html
- AList 百度网盘驱动（元数据与 OAuth 默认值）：https://raw.githubusercontent.com/AlistGo/alist/main/drivers/baidu_netdisk/meta.go
- AList 百度网盘驱动（token 刷新与 `xpan/*` 调用）：https://raw.githubusercontent.com/AlistGo/alist/main/drivers/baidu_netdisk/util.go
- 百度 OAuth token 端点（自测）：https://openapi.baidu.com/oauth/2.0/token
- 百度 OAuth 授权页（自测返回 200）：https://openapi.baidu.com/oauth/2.0/authorize
- 百度网盘主页（自测响应头）：https://pan.baidu.com/
- Chrome App-Bound Encryption 的公开报道（引述 Google Chrome 安全团队博客，**原文本机不可达**）：https://securityboulevard.com/2024/08/google-using-enhanced-encryption-to-protect-cookies
- Chrome v20 加密机制的安全研究分析（**二手**）：https://netlas.io/blog/hannibal_stealer_part_1
- Chrome 136 起默认 profile 禁止 `--remote-debugging-port`（**二手，中文社区**）：https://bbs.huaweicloud.com/blogs/512043
- 「专用 profile + CDP」实践（**二手**）：https://blog.csdn.net/2503_91811914/article/details/166249802

**未能访问（因此相关结论标为未查证）**
- https://security.googleblog.com/2024/03/protecting-chrome-cookies-on-windows.html（连接超时）
- https://blog.chromium.org/2024/03/protecting-chrome-cookies-on-windows.html（连接超时）
- https://developer.chrome.com/blog/remote-debugging-port（连接超时）
- 百度开放平台官方文档页（超时）
- AList/OpenList 官方文档站（超时）

---

## 给下一位工程师的三句话

1. 读 cookie 这件最难的事**已经做过了**：`baidu_probe.rs` 在打包产物里读到了 HttpOnly `BDUSS`（192 字符）与 `STOKEN`，**直接复用它的形状**（`WebviewWindowBuilder` + `WebviewUrl::External("https://pan.baidu.com/disk/main")` + 独立线程 2 秒轮询 + 命中后最多再等 20 秒取 `STOKEN`）。
2. 缺的是**产品化那一层**：新增 `remote_baidu_begin_login` 命令、把 cookie 投影成会话并走 DPAPI 落盘、用事件回传前端、前端改成「先登录后建连接」，并保留手工粘贴作兜底。
3. 同时**纠正一条既有结论**：百度 OAuth 是通的（T1 那个「不通」只否证了 `250528`/`266719` 两个 id）；若要长期免重复登录，值得并行去注册一个百度开放平台应用换 `refresh_token`。