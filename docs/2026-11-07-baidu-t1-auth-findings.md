# 百度网盘接入 · T1 认证可行性实测记录

- 日期：2026-11-07
- 分支：`feat/baidu-remote-provider`
- 产物：`tauri build` 的 `alpha-file-manager.exe`（3.12.2，`target/release`，`BUILD_EXITCODE=0`）
- 探针源码：`src-tauri/src/baidu_probe.rs`（`AFM_BAIDU_PROBE=1` 门控，不属于正式实现）

本文件是 plan §6 中 **T1（认证可行性实测 U1/U2/U3）** 的实测记录。
没有实测支撑的推断一律标注为「推断」，不与实测结论混写。

---

## 一、结论速览

| 编号 | 问题 | 结论 | 关键证据 |
|---|---|---|---|
| **U1** | 打包产物里能否读到 `BDUSS` | **成立** | `bduss_len=192`、`http_only=true`，同日端到端 `uinfo` 返回 `errno:0` |
| **U2** | 能否用 OAuth 换取 `refresh_token` | **不成立** | 400/401 `unknown client id`；`250528` 与阴性对照返回逐字相同 |
| **U3** | `access_token` 刷新链路 | **不再适用** | 依赖 U2；整条 token 路线已废弃 |

**U1 成立、U2 不成立** ⇒ 认证路线定为 **cookie 直连**，
而不是 plan 里原先假设的「登录 → 换 `refresh_token` → 用 token 调 API」。
`BDUSS` 是**已实证可读**的那一个凭证；`STOKEN` 同样可读（+2s），
但「仅 `BDUSS` 是否足够」本轮**未能隔离验证**（见 5.2），故 T3 倾向两者都持久化。

---

## 二、U1：打包产物里能否读到 `BDUSS`

### 2.1 为什么这条不能靠读文档下结论

tauri 2.12.0 的 `WebviewWindow::cookies_for_url`（`webview_window.rs:2715`）文档写明会返回
"including HTTP-only and secure cookies"，但本项目在 `tauri dev` 下界面不可用（见 AGENTS.md），
且 cookie 读取依赖 WebView2 运行时行为，只能在打包产物里实证。

### 2.2 做法

`baidu_probe.rs` 在 `setup_handler` 里挂载，`AFM_BAIDU_PROBE=1` 时才开一个
`WebviewWindowBuilder` 窗口到百度页面，独立线程每 2 秒调一次 `cookies_for_url`。

两个刻意的设计：

- **独立线程轮询**：tauri `webview_window.rs:2731` 明确警告 Windows 上在同步命令 /
  事件处理器里调 cookie API 会死锁，必须 async 或独立线程。
- **回传走本地 `TcpListener` + `new Image().src`**：页面内同源 `fetch` 能带齐 cookie，
  但跨域 `POST` 会被 CORS 拦；用图片请求把结果送到 `127.0.0.1:47831` 绕开。
  这样 **`BDUSS` 全程不落到磁盘上**，报告里只写长度与前 6 个字符。

### 2.3 实测输出（首轮，报告原文节选）

```
--- cookies at +2s (count=4) ---
  name=BAIDUID domain=baidu.com path=/ secure=false http_only=false ... value_len=37
  name=BAIDUID_BFESS domain=baidu.com path=/ secure=true http_only=false ... value_len=37
  name=csrfToken domain=pan.baidu.com path=/ secure=true http_only=false ... value_len=24
  name=newlogin domain=baidu.com path=/ secure=true http_only=true ... value_len=1

--- cookies at +146s (count=11) ---
  name=BDUSS domain=baidu.com path=/ secure=false http_only=true expires=2027-12-12 ... value_len=192
  name=BDUSS_BFESS domain=baidu.com path=/ secure=true http_only=true ... value_len=192
  name=XFI domain=pan.baidu.com ... value_len=36
  name=XFT domain=pan.baidu.com ... value_len=44
  name=csrfToken domain=pan.baidu.com ... value_len=24
  name=ppfuid domain=baidu.com ... value_len=32

BDUSS_FOUND: yes
bduss_len: 192
bduss_domain: baidu.com
bduss_path: /
bduss_secure: false
bduss_http_only: true
stoken_present: no
VERDICT_U1: PASS (BDUSS readable from packaged webview)
```

### 2.4 判据逐条对齐

| 计划里的疑虑 | 实测 |
|---|---|
| 打包产物能否读到 `BDUSS` | ✅ 读到，192 字符，domain `baidu.com`，path `/` |
| 能否读到 **HttpOnly** cookie | ✅ `BDUSS` / `BDUSS_BFESS` / `newlogin` 三条都是 `http_only=true` 且被读出 |
| 能否读到父域 cookie（不在 `pan.baidu.com`） | ✅ 锚点 `https://pan.baidu.com/` 读到了 `domain=baidu.com` 的 cookie |
| 页面内同源请求是否带登录态 | ✅ `uinfo` 返回 `errno:0` + 真实昵称 + `uk` + `vip_type:2`（超级会员） |

**U1 判定：成立。** 且不需要「粘贴 BDUSS」的降级输入框，也不需要企业实名开放平台。

### 2.5 首轮暴露的分叉：`xpan REST` 认 cookie，`/api/*` 不认

同一轮里三条接口用的是**同样的带 cookie 页面内 fetch**，结果却分成两派：

| 接口 | 首轮结果 |
|---|---|
| `/rest/2.0/xpan/nas?method=uinfo` | **200** `{"errno":0, "baidu_name":…, "vip_type":2}` |
| `/api/quota?checkfree=1&checkexpire=1…` | **400** `{"errmsg":"用户未登录","errno":-6}` |
| `/api/list?dir=%2F…` | **200** `{"errno":-6}` |

这不是「cookie 无效」——同一次运行里 `uinfo` 已经拿到昵称。合理解释是两者读的不是同一份凭证：
`/api/*` 是网盘网页端的历史接口，依赖 `pan.baidu.com` 域下的 `STOKEN`，而首轮
`stoken_present: no`。

这条直接决定 T2 里 `baidu_api.rs` 的接口形态，因此追加第二轮探针（见 §五）。

### 2.6 必须记住的边界

`uinfo` 成功是**页面内 `fetch`** 的结果，带的是 WebView2 自己的 cookie jar。
**这不等于 Rust 侧 `reqwest` 手工拼 `Cookie` 头也能成功。** 服务端可能还看
`User-Agent` / `Referer` / `STOKEN`。Rust 侧的等价性要到 T4 真机验，不能引用本节当已证。

---

## 三、U2：能否用 OAuth 换取 `refresh_token`

### 3.1 结论

**不成立。** `250528` 与 `266719` 都不是已注册的 OAuth 客户端。

### 3.2 过程中的一次自我纠正（重要）

第一轮探针只比了「候选 `client_id` 能不能拿到授权页」，看到 200 就准备判成立。
第二轮补的**阴性对照**（`client_id=999999999` 与 `afmprobe`）同样返回 200 的「与百度连接」页 ⇒
**authorize 端点对任意 `client_id` 都不校验**，第一轮结论无判别力，作废。

随后做了一次本地零成本比对：4 个授权页 HTML 逐字节比对，唯一差异只有一行

```js
var access_client_id = '<cid>' || 'init';
```

只是回显。⇒ authorize 端点彻底没有判别力，不能用来验证 `client_id` 合法性。

> **方法论沉淀**：判定「某端点能证明 X」之前，必须先确认它对「X 不成立」的输入有不同表现。
> 阴性对照如果和被验证对象返回一致，整个实验就是零信息。

### 3.3 决定性实验（PASS 3，打真实 token 端点）

`POST https://openapi.baidu.com/oauth/2.0/token`：

| 输入 | 返回 |
|---|---|
| `grant_type=refresh_token` + 候选 `client_id` + **有** `client_secret` | **401** `{"error":"invalid_client","error_description":"unknown client id"}` |
| 同上但 `client_secret` 留空 | **400** `{"error":"invalid_request","error_description":"The request is missing a required parameter"}` |
| `grant_type=authorization_code` + `client_id=250528` | 401 `unknown client id` |
| `grant_type=authorization_code` + `client_id=afmprobe`（**阴性对照**） | 401 `unknown client id`，**与 `250528` 逐字相同** |

两条独立的硬证据：

1. **参数存在性校验在前**：空 `secret` 报 `invalid_request`，有 `secret` 才进到 `invalid_client`
   ⇒ 说明服务端确实按 OAuth 规范读这些字段，不是「随便什么都拒」。
2. **候选与阴性对照返回完全一致** ⇒ `250528` 与 `266719` 都不是注册客户端。

### 3.4 顺带查清的事

- `250528` 是**网盘网页端内部 `app_id`**（`/api/list` 的 query 里就叫 `app_id`），
  **不是** OAuth `client_id`。二者在百度体系里是两套东西。推断（未取证内部文档）。
- `GET https://pan.baidu.com/oauth/2.0/authorize` → **404**，该路径不存在，别再试。
- 正式开放平台确实存在（`/rest/2.0/xpan/*` + `access_token`），但要求企业实名，
  且只能访问 `/apps/{应用名}/` 目录 —— 与本产品「浏览用户自己的网盘」需求不符，已否决。

### 3.5 对原计划降级路线的影响

plan 里写的降级是「让用户粘贴 `refresh_token`」。这条也**不采用**：
`refresh_token` 换 `access_token` 同样需要 `client_id` + `client_secret`，而这两个值拿不到。
改用**cookie 直连**降级，成本更低且已被 U1 实证可行。

---

## 四、U3：`access_token` 刷新链路

**不再适用。** U2 不成立 ⇒ 没有 `access_token`，也就没有刷新问题。
本项记为「随 U2 一并废弃」，不产出实测。

---

## 五、第二轮补充验证（已完成）

目的：确认阶段 1 能否**全部走 `xpan REST`**，以及 `/api/*` 是否需要 `STOKEN`。

> **一次被链条挡住的失败**：第二轮第一次打包其实编译失败 ——
> `error[E0382]: borrow of moved value: url`（`src-tauri/src/baidu_probe.rs:211`），
> `cookies_for_url(url)` 先 move 了 `url`，`cookies_for_url(url.clone())` 才轮到。
> `CHECK_EXITCODE=101`，脚本按设计打出 `CHECK_FAILED_SKIPPING_BUILD` 且**没有**进打包
> （日志 `baidu-check3.log`）。改成 `url.clone()` 后第三次通过
> （`CHECK_EXITCODE=0` / `BUILD_EXITCODE=0`，产物 `81694208` 字节，mtime `23:43:53`）。
> 记这一笔是为了说明「check 不过就不打包」这条链确实拦住了错误产物 ——
> 当时 `release/` 里那份旧 exe 仍在，若直接跑会得到**上一版探针**的结果。

改动（均在 `baidu_probe.rs`）：

1. `START_URL` 由 `https://pan.baidu.com/` 改为 `https://pan.baidu.com/disk/main`
   —— 登录态已存在时直接进主界面，触发 `pan.baidu.com` 下发 `STOKEN`。
2. 命中 `BDUSS` 后，若 `STOKEN` 尚未出现，**最多再等 20 秒**并记录
   `STOKEN_APPEARED_AFTER`。不这么做会把「页面还没加载完」误读成「`/api/*` 不可用」。
3. `PROBE_APIS` 扩到 4 条：
   - `uinfo`（阳性对照，已知 200）
   - `xpan_list` → `/rest/2.0/xpan/file?method=list&dir=%2F&limit=10&web=1&app_id=250528`
   - `xpan_quota` → `/rest/2.0/xpan/quota?checkfree=1&checkexpire=1&web=1&app_id=250528`
   - `api_list` → `/api/list?dir=%2F&limit=10&web=1&clienttype=0&app_id=250528&channel=chunlei`

第二轮不需要人工登录：登录态已落在 WebView2 的 profile（`%LOCALAPPDATA%\<identifier>\EBWebView`）。

### 5.1 实测结果

报告：`.superpowers/sdd/baidu-probe-u1-2.txt`（7336 字节，含 `PROBE_SEQUENCE: complete` 与
`VERDICT_U1: PASS`）。构建：`baidu-check4.log` `CHECK_EXITCODE=0` / `baidu-build3.log`
`BUILD_EXITCODE=0`，产物 `alpha-file-manager.exe` 81694208 字节。

**cookie 清单**

| cookie | 是否出现 | 关键属性 |
|---|---|---|
| `BDUSS` | 是（首轮 +146s，登录后可读） | `domain=baidu.com`、`http_only=true`、`bduss_len=192` |
| `STOKEN` | 是，**+2s 即出现** | `domain=pan.baidu.com`、`secure=true`、`http_only=true`、`value_len=64`、`expires=2026-12-08` |
| `PANPSC` | 是 | 414 字符 |

**四条接口**（页面内同源 `fetch`，请求自动带整个 cookie jar）

| 端点 | 状态 | 结果 |
|---|---|---|
| `xpan/nas?method=uinfo` | **200** | `errno:0`、`baidu_name:"风尚志段燚"`、`uk:2688829335`、`vip_type:2`、`avatar_url` 存在 |
| `xpan/file?method=list`（`dir=%2F&order=name&start=0&limit=10`） | **200** | `errno:0`，条目 `0000temp` / `01A_软件` / `01B_设计素材` / `02A_办公软件教程` / `02B_编程软件教程`；字段 `fs_id` / `path` / `isdir` / `size` / `server_mtime` / `category` |
| `xpan/quota`（`checkfree=1&checkexpire=1`） | **400** | `{"error_code":3,"error_msg":"Unsupported open api"}` |
| `/api/list`（`dir=%2F&clienttype=0&channel=chunlei`） | **200** | `errno:0`，首条与 `xpan/file?method=list` 返回**逐字一致** |

### 5.2 判读

落在判读表**第一行**（`xpan_list` 200），但有两条例外必须写清：

1. **`xpan/quota` 端点在这个 `app_id` + cookie 组合下不开放**（`Unsupported open api`）。
   配额取不到，但**不阻塞 T2** —— 阶段 1 不显示配额。
2. **首轮 `api_list` 的 `errno:-6` 已被解释**：首轮 `STOKEN` 缺失，本轮 `STOKEN` 存在时同一端点 200
   ⇒ 那个 `-6` 不是端点不可用，是缺 `STOKEN`。

同时确认：`xpan/file?method=list` **不需要 `access_token`**（U2 已证没有任何 OAuth 客户端，
不存在 access_token，接口照样 200）。

> **方法论限制（重要，防止把结论用过头）**：页面内 `fetch` 会自动携带**整个 cookie jar**，
> 因此**无法隔离出每个端点究竟依赖哪一个 cookie**。
> 「`xpan` 不需要 `access_token`」成立；
> 「`xpan` 只需要 `BDUSS`」**未证** —— 服务端可能还看 `STOKEN` / `PANPSC` / `UA` / `Referer`。
> 这条留给 T4 真机验（Rust 侧手工拼 Cookie 头），不得在 T2/T3 里当成已证前提。

### 5.3 T2 落地决定

- **列表与元信息一律走 `/rest/2.0/xpan/file?method=list`**，`baidu_api.rs` 按 xpan 规范实现。
  理由：已实证 200 + `errno:0`，字段名更规整（`fs_id` / `path` / `isdir` / `size` / `server_mtime`）。
- **`/api/*` 不作主路径**，只作为备用记录 —— 它需要 `STOKEN`，而 `STOKEN` 只在
  `pan.baidu.com` 域下发，未来若 `BDUSS` 单独可用就不必持久化它。
- **`baidu_auth.rs` 倾向同时持久化 `BDUSS` 与 `STOKEN`**（两者都在 +2s 可读），
  避免押注「只发 `BDUSS` 是否够」这一未证结论；若 T4 实测证明仅 `BDUSS` 足够再收紧。
  二者同为会话等价凭证，安全等级相同。
- **不实现配额接口**，`xpan/quota` 不可用不视为缺陷。

---

## 六、对后续任务的影响

| 任务 | 原计划 | 按实测修正后 |
|---|---|---|
| **T2** `baidu_api.rs` | 混合 `/api/*` 与 `xpan` | **纯 `xpan REST`**（`xpan/file?method=list` 已实证 200）；`/api/*` 仅作备用，不做主路径；不实现配额 |
| **T3** `baidu_auth.rs` | 内嵌 WebView2 登录 → 读 cookie → **换 `refresh_token`** | **只做「读 cookie → 持久化 cookie」**，不再有 token 交换步骤；代码量显著下降 |
| **T7** 前端授权卡 | 扫码 → 显示昵称 | 不变（`uinfo` 已证明能拿到昵称）；但 STATUS 只能是「cookie 有效 / cookie 失效」两态 |
| **验收第 14 条** | 「重新授权」提示 | 改为「**cookie 失效**提示重新授权」 |

`baidu_auth.rs` 的持久化对象从「`refresh_token` + 到期时间」变成「`BDUSS` + `STOKEN`」
（两者均已在 U1 第二轮实证可读，见 5.3）。
`remote-credentials.json` 里存的是**可直接冒充用户会话的凭证**，安全等级等同密码 ——
写盘、日志、错误信息里都不得出现明文。

---

## 七、未验证项（不得当作已证）

1. **Rust 侧 `reqwest` + 手工 Cookie 头是否等价于页面内 fetch** —— 要到 T4 真机验。
2. `xpan REST` 的写操作（`create` / `mkdir` / `rename` / `move` / `copy` / `delete`）是否需要
   `bdstoken` 或其它额外参数 —— 两轮都只验了只读。
3. 分页上限、>1000 项目录的返回形态、错误码全集；`xpan` 与 `/api/*` 的字段一致性也只比了首条。
4. **各端点究竟依赖哪一个 cookie**（`BDUSS` 单独是否够、`STOKEN` / `PANPSC` 是否必需）——
   见 5.2 的方法论限制，页面内 fetch 带整个 cookie jar，无法隔离。留给 T4。
5. 配额取不到：`xpan/quota` 返回 `Unsupported open api`；`/api/quota` 需 `STOKEN`，
   首轮 400 是 `STOKEN` 缺失所致，本轮**未复测**（`STOKEN` 已有）。阶段 1 不显示配额，不阻塞。
6. `cargo test --lib` 是否会因 `baidu_probe.rs` 链接进测试二进制而触发
   `STATUS_ENTRYPOINT_NOT_FOUND` —— 理论分析应被链接器丢弃（只由 `setup_handler` 调用，
   测试不执行 `setup_handler`），**未实证**。这也是本轮不在 `baidu_probe.rs` 里加 `#[cfg(test)]`
   的原因：一加就会把整条 `tauri::` 窗口链拉进测试二进制。
7. `250528` / `266719` 的真实身份仍未查明 —— 只证明「不是已注册 OAuth 客户端」。
   推测 `250528` 是网盘网页端内部 `app_id`（`/api/list` 的 query 里就叫 `app_id`），未取证。

---

## 八、证据路径

全部在 `.superpowers/sdd/`（该目录被 `.gitignore` 排除，不进版本库）：

| 文件 | 内容 |
|---|---|
| `baidu-probe-u1.txt` | **U1 首轮报告**（cookie 清单 + 三条接口回传 + `VERDICT_U1: PASS`） |
| `baidu-probe-u1-2.txt` | **U1 第二轮报告**（7336 字节，`STOKEN` + 四条接口 + `PROBE_SEQUENCE: complete` + `VERDICT_U1: PASS`）—— **第五节结论的依据** |
| `baidu-oauth-probe-result.txt` | U2 第一轮（已作废） |
| `baidu-oauth-probe/probe-pass2.txt` | U2 第二轮（54 行，阴性对照暴露问题） |
| `baidu-oauth-probe/probe-pass3.txt` | U2 第三轮（67 行，决定性实验） |
| `baidu-oauth-probe/authorize-*.html` | 4 个授权页，用于本地逐字节比对 |
| `baidu-check.log` / `baidu-check2.log` | 首轮 check（`CHECK_EXITCODE=0`） |
| `baidu-check3.log` | 第二轮 check —— **`101`，`E0382`**（`CHECK_FAILED_SKIPPING_BUILD`），本轮唯一一次失败 |
| `baidu-check4.log` | 第三轮 check（`CHECK_EXITCODE=0`），第二节所述产物的编译依据 |
| `baidu-build.log` | 首轮打包日志（`BUILD_EXITCODE=0`） |
| `baidu-build3.log` | 第三轮打包日志（`BUILD_EXITCODE=0`），产出 81694208 字节的 exe |

⚠️ `baidu-build2.log` **不存在** —— 第二轮 check 失败后脚本按设计跳过了打包。
若在文件列表里看到它，说明记错了轮次。

构建脚本：`baidu-check.cmd` / `baidu-build.cmd` / `baidu-verify.cmd` / `baidu-verify2.cmd` / `baidu-verify3.cmd`
（每轮用新文件名，绝不复用正在运行的 `.cmd`）。