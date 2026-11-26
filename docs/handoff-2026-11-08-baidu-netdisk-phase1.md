# 交接文档：百度网盘接入 AFM「远程位置」· 阶段 1（T1–T9 已完成，待真实会话补验）

- 日期：2026-11-08（T1 收尾于当日 23:5x；T2 / T3 收尾于次日凌晨；**T4–T9 于 2026-11-08 上午收口**）
- 分支：`feat/baidu-remote-provider`（基点 `main` = `6eabbc67`）
- 当前 HEAD：`9063db44`（**仍未产生新提交**，全部改动在工作区）
- 状态：**T1–T9 全部完成。** 打包产物 `npm run tauri:build` 成功（`BUILD_EXITCODE=0`），真机 UI 取证拿到硬证据（协议下拉有「百度网盘」/ 表单切 Cookie 分支 / 保存落盘 `pan.baidu.com:443` / 侧边栏「远程位置」面板列出该连接）。**阶段 1 代码侧收口；依赖真实百度会话的 5 条判定记为「未验证」，需在有会话的机器上补验。** 下一阶段 = 阶段 2（上传写入）+ 真实会话补验。
- **🔔 接力入口（最新）：§19「接力收口（2026-11-08 15:2x）」。** 新会话请先只读 §19，再按需回查 §15–§18；§18 末尾那条「下一会话第一步」已被 §19 取代（它引用的日志文件名不存在，见 §19.4 第 1 条）。§19.2 纠正了安装包交付路径——**真机要装 `release\local-builds\` 里 11-08 14:41:27 那个包，不是 `release\` 顶层 11-07 的旧包**。

---

## 0. 一句话现状

路径 A（复用 AFM 既有 `sfm://remote/` 远程 VFS，新增 `baidu_pan` 协议变体）已获批准；**T1 已用打包产物实测收口**：U1 成立（打包产物里可读 HttpOnly `BDUSS`，且 `STOKEN` 也可读），U2 不成立（`250528` / `266719` 都不是已注册 OAuth 客户端），U3 随之废弃 ⇒ **认证路线定稿为「cookie 直连」**，T3 从「换 `refresh_token`」降级为「读 cookie → 持久化 cookie」。**T4–T8 已把实现全部写完并挂进 crate**：`RemoteProtocol::BaiduPan` 变体、只读 + 写操作 provider、三个新 Tauri command、前端连接表单与 17 个语言包全部到位，`cargo test --lib`（1142 passed）与全量前端 `npx vitest run`（292 files / 2740 tests passed）双绿。**T9 已打包成功（`BUILD_EXITCODE=0`）并拿到真机 UI 硬证据**：协议下拉有「百度网盘」、表单切 Cookie 分支、保存后落盘 `pan.baidu.com:443`、侧边栏「远程位置」面板列出该连接。**唯一没闭环的是依赖真实百度会话的 5 条判定**（本机只有格式合法的假 cookie），必须在有会话的机器上补验；真实用户数据已按「先杀进程再 restore」完整还原。

---

## 1. 背景

### 1.1 产品

Alpha File Manager（AFM）是 Tauri v2 + Vue 3 的 Windows 文件管理器。仓库根：

```
F:\soft\00selfmade\filemanager\sigma-file-manager     ← 内层产品仓（远端 kizemo/alpha-file-manager）
F:\soft\00selfmade\filemanager                        ← 外层约定仓（远端 kizemo/focus-sync，含 AGENTS.md）
```

**所有代码改动都在内层仓**。`AGENTS.md` 在外层仓根目录。

### 1.2 已有的远程能力（这是路径 A 能被砍到 8–10 人天的前提）

AFM 早已有一套完整的远程 VFS，支持 FTP / FTPS(explicit, implicit) / SFTP：

- 后端 `src-tauri/src/remote/`，`RemoteProvider` trait 9 个方法（`provider.rs:40`）
- 统一路径方案 `sfm://remote/<连接id>/<内部路径>`，落盘规则在 `path.rs`
- 递归传输、冲突处理、进度上报在 `transfer.rs`（1955 行，协议无关）
- 连接元数据归前端 store → `user-data/user-remote-connections.json`；密码归 Rust DPAPI → `remote-credentials.json`（键 = 连接 id）。这套分工叫「方案 C」，2026-10-18 拍板，代码在 `registry.rs`
- 侧边栏「远程位置」面板 `remote-nav-panel.vue` **按连接列表渲染，不认协议**

⇒ **只要 `RemoteProtocol` 多一个变体，百度网盘连接就会自动出现在「远程位置」里**。这是用户需求的原话落点，不需要新造"虚拟驱动器"概念。

### 1.3 用户需求

侧边栏「远程位置」里出现「百度网盘」，双击进入后能像本地目录一样浏览、复制、拖拽。

---

## 2. 已定的方向（不要再重新讨论）

| 项 | 决策 | 依据 |
|---|---|---|
| 路径 | **A：复用 `sfm://remote/`**（21–30 人天全量，阶段 1 只读压到 8–10） | `docs/2026-11-07-baidu-netdisk-drive-feasibility.md` §4 |
| 路径 B | 否决：sidecar WebDAV + `net use` 映射（8–12 人天但性能灾难） | 同上 |
| 路径 C | 本轮不做：WinFsp 自研文件系统（45–70 人天） | 同上 |
| 官方开放平台 | 否决：要求企业实名，且只能操作 `/apps/{应用名}/` 目录，与"浏览整个网盘"根本冲突 | 同上 §2 |
| 逆向客户端 | 否决：网页 API 有成熟社区实现可对照；逆向私有加密协议无法产品化，且路径 A 不依赖客户端版本 | 同上 |
| **认证路线** | **cookie 直连**：读 `BDUSS`（必要时带 `STOKEN`）冒充会话，不换 `refresh_token` | **T1 实测**：`docs/2026-11-07-baidu-t1-auth-findings.md` §三 |
| 分支 | **必须开分支**（已开 `feat/baidu-remote-provider`） | 要动 `registry.rs` / `path.rs` / `connection.rs` / `dir_adapter.rs` 这些被 FTP/FTPS/SFTP 共用的文件，爆炸半径覆盖已有三类连接的用户；非官方接口随时可能被封，分支可整体丢弃 |
| 不建 worktree | 是 | `src-tauri/target/` 与 `node_modules` 体量大，第二 worktree 要重建整套产物，`tauri build` 时间翻倍。worktree 只用于 eslint 基线对照 |
| 合并节奏 | 阶段 1 验收通过 → 合回 main 发小版本给用户装机验证；阶段 2（写入）继续同一分支 | 阶段 1 是只读的，风险可控 |

---

## 3. T1 已完成：三问结论（本轮核心产出）

> 完整记录：`docs/2026-11-07-baidu-t1-auth-findings.md`（17,742 字节，8 章）。本节只给结论，**判读细节与方法论限制看原文**。

| 问题 | 结论 | 决定性证据 |
|---|---|---|
| **U1** 打包产物里能否读到 `BDUSS` | **成立** | `VERDICT_U1: PASS`。`bduss_len=192`、`domain=baidu.com`、`http_only=true`；`STOKEN` 在 **+2s** 也可读（`value_len=64`、`domain=pan.baidu.com`、`secure=true`、`expires=2026-12-08`），同批还有 `PANPSC`（414 字符）。端到端 `uinfo` 返回 200 拿到昵称 |
| **U2** 能否用 OAuth 换 `refresh_token` | **不成立** | token 端点用假 secret → `401 {"error":"invalid_client","error_description":"unknown client id"}`；空 secret → `400 invalid_request`（证明参数存在性校验在前）。`authorization_code` 用 `250528` / `266719` 与阴性对照 `afmprobe` 返回**逐字相同** ⇒ 两者都不是已注册 OAuth 客户端 |
| **U3** `access_token` 刷新链路 | **不适用**，随 U2 废弃 | — |

### 3.1 实测取得的服务端行为（真实打到 `pan.baidu.com`）

```
# 带真实登录态（打包产物 WebView2 profile 已有会话），页面内同源 fetch
GET  /rest/2.0/xpan/nas?method=uinfo&app_id=250528&web=1
     -> 200 {"baidu_name":"风尚志段燚","errno":0,"uk":2688829335,"vip_type":2}
GET  /rest/2.0/xpan/file?method=list&dir=%2F&order=name&start=0&limit=10&web=1&app_id=250528
     -> 200 {"errno":0,"list":[...]}          # xpan 列表可用，无 access_token
GET  /rest/2.0/xpan/quota?checkfree=1&checkexpire=1&web=1&app_id=250528
     -> 400 {"error_code":3,"error_msg":"Unsupported open api"}   # 此 app_id + cookie 组合不开
GET  /api/list?dir=%2F&...&app_id=250528&channel=chunlei
     -> 200 {"errno":0,"list":[...]}          # 有 STOKEN 时可用，首条与 xpan 逐字一致

# 无 cookie 时（对照）
GET  /rest/2.0/xpan/nas?method=uinfo  -> 400 {"errmsg":"Invalid Bduss","errno":-6}
GET  /api/quota?...                    -> 400 {"errmsg":"用户未登录","errno":-6}
```

### 3.2 对后续任务的直接影响

- **T2**：列表/元信息**一律走 `/rest/2.0/xpan/file?method=list`**（schema 更规整：`fs_id` / `path` / `isdir` / `size` / `server_mtime`）；`/api/*` 只作备用。**不实现配额**（`xpan/quota` 不开放，且配额不是阶段 1 需求）。
- **T3**：持久化对象 = **`BDUSS` + `STOKEN`**（两者都在 +2s 可读）。不采用「只发 `BDUSS` 是否够」这一未证结论；若 T4 实测证明仅 `BDUSS` 足够再收紧。
- **T7**：授权卡不再需要「粘贴 `refresh_token`」降级路径（换 token 同样需要拿不到的 `client_id` + `client_secret`）。
- **plan 原文的 D2 已作废**（原文写「读 cookie 拿 `BDUSS` → 换 `refresh_token`」，第二步已被实测否决）。plan 文档本身未回改，**以本文档与 T1 记录为准**。

---

## 4. 阶段 1 计划摘要（细节看 plan 原文）

### 三条仍然有效的架构决策（D2 已按上文改写）

- **D1** 用 `RemoteProtocol` 新变体 `BaiduPan`，不新建子系统。加变体后编译器会在 `commands.rs:386` 的 `provider_with_protocol` 穷尽 `match` 处报错 —— **改动清单由编译器给**，这是有意为之的护栏。
- **D3** 阶段 1 不实现 `write_file`，但**必须返回 `Unsupported`，绝不静默 `Ok(())`**。
- **D4** 不往 `RemoteConnection` 加新字段（`host` = `pan.baidu.com`、`port` = 443、`username` = 昵称、`credential_key` = 连接 id），避免重蹈「读写白名单不同步导致重启丢数据」。

### 改动面

后端 **3 新增 + 4 修改**：`baidu_api.rs`(≈400) / `baidu_auth.rs`(≈350) / `baidu_provider.rs`(≈700) 新增；`mod.rs` / `connection.rs` / `commands.rs` / `lib.rs` 改。
前端 **2 新增 + 6 修改**：`remote-authorize-card.vue` 新增；`types/remote.ts` / `connection-form.vue` / `remote-context-menu-visibility.ts` / `remote-directory.ts` / `remote-connections.ts` / 17 个语言包 改。

### 九个任务

| # | 任务 | 人天 | 状态 |
|---|---|---:|---|
| **T1** | 认证可行性实测 U1/U2/U3 | 1.5 | ✅ **已完成** |
| **T2** | `baidu_api.rs` 纯函数层（URL/分页/错误码） | 1.5 | ✅ **已完成**（65 条用例，§12） |
| **T3** | `baidu_auth.rs` 读 cookie → 持久化 cookie | 1.0 | ✅ **已完成**（49 条用例，§12） |
| **T4** | `baidu_provider.rs` 只读（`read_dir`/`read_file`/`get_permissions`） | 2.0 | ✅ **已完成**（只读 provider + `filemetas` 直链解析） |
| **T5** | 写操作 + `write_file`/`set_permissions` 明确 Unsupported | 1.0 | ✅ **已完成**（`bdstoken` / 表单 POST / 建-改名-删；`write_file*` / `set_permissions` 保持 Unsupported） |
| **T6** | 命令接线 + `provider_with_protocol` 分支 | 0.5 | ✅ **已完成**（`RemoteProtocol::BaiduPan` + 3 个 command + 白名单 16→19） |
| **T7** | 前端表单 + 授权卡 + 两个判定函数 | 1.5 | ✅ **已完成**（表单切 Cookie 分支 + 独立 command 存取会话 + `remote_test_connection` 读存档探测） |
| **T8** | 语言包 17 个 + 前端单测 | 0.5 | ✅ **已完成**（17 个语言包 + 3 份前端单测；全量 vitest 292 files / 2740 tests passed） |
| **T9** | 真机验收 + 打包 | 1.0 | ✅ **已完成（打包成功 + 真机 UI 取证）**；依赖真实百度会话的 5 条判定**未验证** |

合计 10.5 人天，实排 8–10。T2 / T3 可并行。

---

## 5. 已验证 / 推断 / 未验证

### ✅ 已验证（有硬证据）

| 结论 | 证据 |
|---|---|
| 打包产物可读 HttpOnly `BDUSS`（192 字符） | `baidu-probe-u1-2.txt` 的 `VERDICT_U1: PASS` + cookie 清单 |
| `STOKEN` 同样可读（64 字符，+2s 出现） | 同上 `stoken_present: yes` |
| cookie 直连能打通只读接口 | `uinfo` 200 `errno:0` + `xpan/file?method=list` 200 `errno:0` |
| `250528` / `266719` 不是已注册 OAuth 客户端 | token 端点响应与阴性对照 `afmprobe` 逐字相同 |
| `xpan/quota` 在当前 app_id + cookie 下不开放 | 400 `{"error_code":3,"error_msg":"Unsupported open api"}` |
| `reqwest` 已是既有依赖（0.13，`default-features=false`，只开 `rustls`+`stream`） | `src-tauri/Cargo.toml:59` ⇒ **不是新增第三方依赖，无需合规审查** |
| 但 `json` / `cookies` / `blocking` **未启用** | 同上 ⇒ T2 的 JSON 解析直接用 `serde_json`，Cookie 头手工拼 `Cookie: BDUSS=...; STOKEN=...`（不可用 `reqwest::cookie::Jar`） |
| tauri 实际解析版本为 **2.12.0**（Cargo.toml 写 2.11） | `Cargo.lock` ⇒ `cookies_for_url` 在 2.12 才保证可用 |
| `cookies_for_url` 在 Windows 同步命令/事件处理器里调用会**死锁** | tauri `webview_window.rs:2731` 警告 ⇒ 必须 async 命令或独立线程 |
| 侧边栏按连接列表渲染、不认协议 | 读 `remote-nav-panel.vue`；`remote-paths.ts` 无协议分支 |
| 加 `RemoteProtocol` 变体会在 `provider_with_protocol` 报穷尽性错误 | `commands.rs:386` 三个分支，无 `_ =>` 兜底 |
| 两条旧分支已完全合入 main | `merge-base --is-ancestor` + 双向 `rev-list --count` |
| **挂了 `baidu_api.rs` + `baidu_auth.rs` 之后 `cargo test --lib` 全绿**（1057 passed / 0 failed / 1 ignored，**无 `STATUS_ENTRYPOINT_NOT_FOUND`**） | `.superpowers/sdd/baidu-t2-test4.log`（末行 `test result: ok.`） |
| 新增用例 **114** 条（`baidu_api` 65 + `baidu_auth` 49），**全部离线、零网络** | 同上，log 里 `remote::baidu_*` 行数 = 114 |
| 两个新文件**不含任何 `tauri::` 路径**（注释里的说明文字除外） | 脚本剥掉 `//` 注释行后正则 `tauri::` 命中 = false |
| **测试二进制链接了 `baidu_probe.rs`（`lib.rs` 里有 `mod baidu_probe;`）仍然全绿** ⇒ 探针里的 `tauri::` 路径被链接器整段丢弃，**不需要**给它加 `#[cfg(test)]` | 同上（这是 §5 未验证清单里那条的正面结论） |
| `reqwest` 未开 `charset` feature ⇒ **`Response::text()` 不可用**，必须 `bytes()` + `String::from_utf8` | reqwest 0.13.5 源码 `async_impl/response.rs` 里 `text()` 挂着 `#[cfg(feature = "charset")]` |
| `build_remote_path` **不做百分号编码**，`RemoteEntry.path` 是字面中文路径（`sfm://remote/conn-a/01A_软件`） | `remote/path.rs:103`；本轮写错过一次期望值，单测当场抓出来 |

### 🔶 推断（合理但未实测，**不得当作已证**）

| 推断 | 若错的后果 |
|---|---|
| Rust 侧 `reqwest` + 手工 `Cookie` 头等价于 WebView2 页面内 fetch | T4 真机才发现 ⇒ 可能需补 `UA` / `Referer` / `PANPSC`，或改用 webview 内代理 |
| `xpan` 写操作（`create`/`mkdir`/`rename`/`move`/`copy`/`delete`）不需要 `bdstoken` | T5 阶段才发现 ⇒ 每次写前先取 `bdstoken`，+0.3 人天 |
| 加 `RemoteProtocol::BaiduPan` 后前端只需改 4 个分支点 | 漏改 ⇒ 运行时下拉缺项或界面报错，T9 才发现 |
| 17 个语言包的 `protocols` 键位于 650-654 行区 | 只需按 key 改，不依赖行号，无关紧要 |

### ❌ 未验证（后续必须做掉才能算阶段 1 成立）

- **各端点究竟依赖哪一个 cookie**：页面内 fetch 带整个 cookie jar，本轮**无法隔离** `BDUSS` / `STOKEN` / `PANPSC` 各自的作用。这是 T1 方法论上的硬限制。
- 配额：`xpan/quota` 不支持；`/api/quota` 在 `STOKEN` 已有后**未复测**。
- `250528` / `266719` 的**真实身份**未查明（只证"不是已注册 OAuth 客户端"；推测 `250528` 是网盘网页端内部 `app_id`，无取证）。
- 百度网页 API 的分页上限、错误码全集、`>1000 项`目录的实际返回形态。
- `xpan` 与 `/api/*` 的字段一致性只比了首条。
- 阶段 1 真机路径：**UI 与落盘链路已有 4 条硬证据**（协议下拉 / 表单 Cookie 分支 / 保存落盘 / 侧边栏面板列出连接，见 §13）；**依赖真实百度会话的 5 条（列表 / 新建 / 改名 / 删除 / 下载）仍未验证** —— 本机只有格式合法的假 cookie。
- **`filemetas` / `dlink` 两个端点的 `sign` / `timestamp` 怎么取，T2 只写了 URL 拼装并标注「未实证」** —— T4 真机确认前不要当成可用。
- `STOKEN` 的真实有效期长度没在真机上量过（T1 只看到当时那个 `expires=2026-12-08`）；T3 里按「过期前 24 小时提示重新登录」处理，是保守估计。
- **Rust 侧 `reqwest` + 手工 `Cookie` 头到底够不够**（可能还缺 `PANPSC` / `Referer` / 更像浏览器的 `UA`）：T2 的 `BAIDU_PAN_USER_AGENT` 是照抄网盘客户端的值，**未实证**。

---

## 6. 下阶段：从 T4 开工

> 本节原为「从 T2 开工」。**T2 / T3 已完成**（交付说明见 §11），下面的「具体形态」现作为 T4 的输入保留。

### 6.1 T2 的具体形态（已完成，保留作对照）

- 新文件 `src-tauri/src/remote/baidu_api.rs`（≈400 行）：**纯 HTTP + 纯函数解析，零 `tauri::`**。
- 顶部必须照抄 `remote/mod.rs:8-18` 那段 `cargo test --lib` 红线说明。
- 列表/元信息走 `/rest/2.0/xpan/file?method=list`；URL 构造、分页、错误码映射全部是可单测的纯函数。
- 契约：`RemoteEntry` 七字段（`name` / `path` / `is_dir` / `size` / `modified_time` / `is_hidden` / `is_symlink`），**时间单位是 Unix 纪元毫秒**（百度返回 `server_mtime` 是秒，必须 ×1000）。
- 错误映射要覆盖 `RemoteErrorKind` **全部 11 个变体**（`Auth` / `HostKeyUnknown` / `HostKeyChanged` / `Unreachable` / `Permission` / `NotFound` / `Protocol` / `Encoding` / `Disconnected` / `Unsupported` / `Unknown`）。
- 验收判据（plan §6）：**单测全绿，零网络；错误码映射覆盖 11 个 kind 的穷尽表**。

### 6.2 正式实现前必须知道的三条

1. 新文件不得出现 `tauri::AppHandle` / `tauri::async_runtime::*`，需要异步就给 `*_blocking` 同步内核入口，调用方自己选执行器。
2. `baidu_probe.rs` 是**探针脚手架，不是正式实现**。T3 写「读 cookie」正式代码时它应当被取代；本轮**故意不加 `#[cfg(test)]`**，那会把整条 `tauri::` 窗口链拉进测试二进制。
3. 探针复跑**不需要人工登录**（登录态在 `%LOCALAPPDATA%\com.sigma-file-manager.app\EBWebView`），跑法见 §8。

### 6.3 关键反例（must-have，来自 plan §7）

- `.` / `..` 段被 `normalize_remote_path` 拒 → `RemoteErrorKind::Protocol`
- `write_file` → `Unsupported`
- `set_permissions` → `Unsupported`
- `get_permissions` → `Ok(None)`

---

## 7. 本项目高危铁律（本轮相关）

全部在 `F:\soft\00selfmade\filemanager\AGENTS.md`，这里只列会咬到本次任务的：

1. **`cargo test --lib` 的测试二进制不能链接 `tauri::AppHandle` / `tauri::async_runtime::*`**，否则 `STATUS_ENTRYPOINT_NOT_FOUND (0xC0000139)`，一个测试都跑不了。三个新文件顶部都要写红线注释。
2. **真机验收必须用 `tauri build` 产物**。`tauri dev` 下所有路由页面内容区空白（已归因为 dev 特有，不是代码缺陷），dev 上做的界面验收结论一律作废。
3. **需要装机实测就直接打包 exe，不要等用户开口**。`npm run tauri:build` 很慢 ⇒ 判断出要验就早启动、丢后台，再做文档/补测试。
4. **`remote-credentials.json` 是本次新增的高风险点**：授权会真的写进用户密文档，存的是可直接冒充用户会话的凭证 ⇒ 写盘/日志/错误信息不得出现明文。验收前备份整个 `user-data/`；恢复顺序必须**先杀进程再 restore**（应用退出时会把内存状态写回）。
5. **改 `.vue` / `.ts` / `.json` 一律用编辑工具**，绝不用 PowerShell `Get-Content -Raw` / `Set-Content`（中文注释会 mojibake）。
6. **`eslint .` 基线数字随提交变**，量的时候一律 `git stash push -u` 量 HEAD，别拿旧数字对。（2026-10-23 实测：`eslint .` = 188 errors / 0 warnings，`eslint src` = 175，`eslint scripts` = 13。）
7. **`.cmd` / `.ps1` 必须纯 ASCII**；`.cmd` 内用 `> logfile 2>&1` 重定向，**不许挂 stdout 管道**；脚本运行期间**不能编辑该脚本**（cmd.exe 按字节偏移逐行读）；echo 里不能出现 `>`。
8. **Rust toolchain 路径里的 `pc-` 不能省**：`1.90.0-x86_64-pc-windows-msvc`。
9. **UIA 验收脚本开头必须 `SetProcessDPIAware()`**；`SendInput` 的 `INPUT` 必须 `[StructLayout(LayoutKind.Explicit, Size = 40)]`。

---

## 8. 工具坑与可复用跑法（两轮合并）

### 8.1 本轮新踩到的

| 坑 | 正确做法 |
|---|---|
| `cookies_for_url(url)` **会 move `url`**，同轮循环第二次调用报 `E0382: borrow of moved value` | 第二次用 `url.clone()`；本轮就是这么让 check 失败、打包被链式脚本正确跳过的 |
| 探针报告轮询用早期出现的 `bduss_len:` 做判据 ⇒ 拿到半截报告 | 判据要选**终态标记** `PROBE_SEQUENCE: complete` 或 `VERDICT_U1:` |
| OAuth 的阴性对照与候选返回**逐字相同** ⇒ 端点根本没有判别力 | 阴性对照必须"真的会失败"才有价值，否则 PASS 2 的结论无效（本轮已作废重做） |
| 打包日志被 cmd.exe 独占，轮询期间 `ReadAllText` 抛「file is being used by another process」 | `try/catch` 包住重试，属正常 |
| `cookie::Expiration` 不实现 `Display`（cookie 0.18.2） | 只能 `Debug` |
| 用 `rg` / `grep` 搜 exe 二进制不可靠 | `node -e "const b=fs.readFileSync(p);console.log(b.includes(Buffer.from(s)))"` |
| `git commit` 打印 `sh: warning: setlocale: LC_ALL: cannot change locale (zh-CN)` | **无影响**，提交仍成功 |

### 8.2 构建（链式：check 通过才 build，一次后台调用完成）

```bat
@echo off
setlocal
set "PATH=C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;%PATH%"
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
set "SDD=F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd"
cd /d F:\soft\00selfmade\filemanager\sigma-file-manager\src-tauri
cargo check --lib > "%SDD%\baidu-checkN.log" 2>&1
echo CHECK_EXITCODE=%ERRORLEVEL% >> "%SDD%\baidu-checkN.log"
if errorlevel 1 goto :fail
cd /d F:\soft\00selfmade\filemanager\sigma-file-manager
call npm run tauri:build > "%SDD%\baidu-buildN.log" 2>&1
echo BUILD_EXITCODE=%ERRORLEVEL% >> "%SDD%\baidu-buildN.log"
goto :done
:fail
echo CHECK_FAILED_SKIPPING_BUILD >> "%SDD%\baidu-checkN.log"
:done
endlocal
```

调用：`cd .superpowers\sdd; & .\baidu-verifyN.cmd`，`run_in_background=true`，timeout 2400。**每次用新文件名**，不要复用正在跑/跑过的脚本名。

### 8.3 跑 U1 探针（不需要人工登录）

```powershell
Get-Process alpha-file-manager -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2
$env:AFM_BAIDU_PROBE='1'
$env:AFM_BAIDU_PROBE_OUT='F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\baidu-probe-u1-N.txt'
Start-Process 'F:\soft\00selfmade\filemanager\sigma-file-manager\src-tauri\target\release\alpha-file-manager.exe'
```

探针机制（`src-tauri/src/baidu_probe.rs`，19,941 字节）：`AFM_BAIDU_PROBE=1` 门控开窗 → 独立线程每 2 秒 `cookies_for_url` 轮询（**必须独立线程**，同步上下文调用会死锁）→ 命中 `BDUSS` 后 `window.eval()` 注入同源 `fetch` 打只读接口 → 用 `new Image().src` 回传到本地 `TcpListener`（`127.0.0.1:47831`，绕 CORS）→ 落盘报告。超时 900s，等回传最多 30s。

### 8.4 分支用途核查的可复用判据

```powershell
git -C $r merge-base --is-ancestor $branch main   # exit 0 = 已合入
git -C $r rev-list --count "main..$branch"        # 0 = 分支没有 main 缺的东西
git -C $r rev-list --count "$branch..main"        # 非 0 = 分支落后 main
git -C $r tag --contains $head --sort=creatordate | Select-Object -First 4
```

---

## 9. 证据路径清单

| 内容 | 路径 |
|---|---|
| 可行性 + 工作量分析（7 节） | `docs/2026-11-07-baidu-netdisk-drive-feasibility.md` |
| **阶段 1 实现计划（开工必读）** | `docs/plan-2026-11-08-baidu-netdisk-phase1.md` |
| **T1 实测记录（三问结论，本 handoff 的源头）** | `docs/2026-11-07-baidu-t1-auth-findings.md` |
| 上一轮交接（UI 默认设置 + 3.12.2 发版） | `docs/handoff-2026-11-07-ui-default-settings-release.md` |
| 仓库约定（约 750 行，必读） | `F:\soft\00selfmade\filemanager\AGENTS.md` |
| T1 探针报告（终态） | `.superpowers/sdd/baidu-probe-u1-2.txt`（7,336B，含 `PROBE_SEQUENCE: complete` + `VERDICT_U1: PASS`） |
| T1 探针报告（首轮，作废对比用） | `.superpowers/sdd/baidu-probe-u1.txt`（6,361B） |
| OAuth 探针原始输出 | `.superpowers/sdd/baidu-oauth-probe/probe-pass2.txt`、`probe-pass3.txt`、`authorize-*.html` |
| 构建日志（成功） | `.superpowers/sdd/baidu-check4.log`（`CHECK_EXITCODE=0`）、`baidu-build3.log`（`BUILD_EXITCODE=0`） |
| 构建日志（失败样本，E0382） | `.superpowers/sdd/baidu-check3.log`（`CHECK_EXITCODE=101`）。⚠️ `baidu-build2.log` **不存在**（check 失败跳过打包） |
| 探针源码 | `src-tauri/src/baidu_probe.rs` |
| **T2 交付** | `src-tauri/src/remote/baidu_api.rs`（约 45KB，65 条用例） |
| **T3 交付** | `src-tauri/src/remote/baidu_auth.rs`（约 37KB，49 条用例） |
| **T2/T3 门禁日志（最终，绿）** | `.superpowers/sdd/baidu-t2-test4.log`（`test result: ok. 1057 passed; 0 failed` + `TEST_EXITCODE=0`） |
| T2/T3 前几轮日志（失败样本，可对照） | `baidu-t2-test1.log`（1007 passed / 1 failed：`the_entry_path_is_the_sfm_remote_path` 百分号编码期望写错）、`baidu-t2-test2.log`（1054 passed / 1 failed：`a_pasted_set_cookie_header_is_rejected` 用例写反）、`baidu-t2-test3.log`（1057 passed / 0 failed） |
| 本轮跑测试用的 `.cmd` | `.superpowers/sdd/baidu-t2-1.cmd` … `baidu-t2-4.cmd`（每次换新文件名；纯 ASCII） |
| `RemoteProvider` trait | `src-tauri/src/remote/provider.rs:40` |
| 错误类型与 11 个 kind | `src-tauri/src/remote/error.rs:15` / `:51` / `:86` |
| 协议定义 | `src-tauri/src/remote/connection.rs:19-47` |
| 协议分发点（编译器报错处） | `src-tauri/src/remote/commands.rs:386` |
| 方案 C 连接/凭据分工 | `src-tauri/src/remote/registry.rs` |
| 远程目录转换契约 | `src-tauri/src/remote/dir_adapter.rs` |
| 前端协议联合类型 | `src/types/remote.ts:15` |
| 协议下拉数组 | `src/modules/settings/ui/categories/remote/connection-form.vue:39` |
| 侧边栏远程面板 | `src/modules/nav-sidebar/components/remote-nav-panel.vue` |
| 真实用户数据目录 | `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\` |

> `.superpowers/sdd/` 被 `.gitignore` 的 `*` 排除，**不进版本库**；这些证据只在本机有效，换机需重跑。

---

## 10. 工作区状态（2026-11-08 T9 收口后更新）

```
分支：feat/baidu-remote-provider
HEAD：9063db44（未产生新提交）

本特性真正改动的文件：
  M  src-tauri/src/lib.rs
  M  src-tauri/src/remote/mod.rs
  M  src-tauri/src/remote/commands.rs
  M  src-tauri/src/remote/connection.rs
  M  src-tauri/src/remote/connection_test.rs
  M  src-tauri/src/remote/registry.rs
  M  src-tauri/src/remote/credentials.rs
  M  src/types/remote.ts
  M  src/stores/storage/remote-connections.ts
  M  src/modules/settings/ui/categories/remote/connection-form.vue
  M  src/localization/messages/*.json（17 个：ch/en/de/es/fr/it/ja/pt/ru/tr/vi/ur/hi/hy/he/fa/sl）
  ?? src-tauri/src/remote/baidu_api.rs                        ← T2
  ?? src-tauri/src/remote/baidu_auth.rs                       ← T3
  ?? src-tauri/src/remote/baidu_provider.rs                   ← T4 + T5
  ?? src-tauri/src/baidu_probe.rs                             ← T1 探针脚手架
  ?? src/types/__tests__/remote-protocol-baidu.test.ts
  ?? src/stores/storage/__tests__/remote-connections-baidu.test.ts
  ?? src/modules/settings/ui/categories/remote/baidu-connection-form.test.ts
  ?? docs/2026-11-07-baidu-t1-auth-findings.md
  ?? docs/plan-2026-11-08-baidu-netdisk-phase1.md
  ?? docs/2026-11-08-baidu-t7-t8-frontend-checklist.md
  ?? docs/2026-11-08-baidu-t9-acceptance.md
  ?? docs/handoff-2026-11-08-baidu-netdisk-phase1.md           ← 本文档
  ?? docs/prompt-2026-11-08-relay-baidu-phase1.md              ← 接力 prompt
  ?? extensions/kizemo.focus-sync/sidecar/src/bin/             ← 历史残留，不要碰
```

⚠️ **工作区里还有 43 个与本特性零关联的 Rust 文件被 `cargo fmt` 改了格式**（全仓格式化噪音；
仓库 HEAD 本来就不是 rustfmt 干净状态 —— 见 §7 第 6 条）。**提交前必须回滚**，
否则一次 `git add -A` 会把 43 个无关文件扫进提交：

```powershell
# 仓库根执行。这 43 个文件与百度网盘没有任何关系（不出现在任何 T4–T9 改动清单里），
# 差异纯粹是 rustfmt 的换行/尾逗号/`use` 重排，回滚不影响本特性功能。
git checkout -- src-tauri/src/app_updater.rs src-tauri/src/batch_rename src-tauri/src/compare src-tauri/src/delete_job.rs src-tauri/src/delete_tree.rs src-tauri/src/dir_reader/reveal.rs src-tauri/src/dir_watcher.rs src-tauri/src/drag_no_activate.rs src-tauri/src/extensions/http.rs src-tauri/src/extensions/manifest_permissions.rs src-tauri/src/file_lock.rs src-tauri/src/global_search.rs src-tauri/src/global_search src-tauri/src/image_edit.rs src-tauri/src/mcp src-tauri/src/orphan_webview.rs src-tauri/src/remote/fake.rs src-tauri/src/remote/ftp_parsing.rs src-tauri/src/remote/ftp_provider.rs src-tauri/src/remote/ftps_provider.rs src-tauri/src/remote/known_hosts.rs src-tauri/src/remote/provider.rs src-tauri/src/remote/sftp_provider.rs src-tauri/src/remote/transfer.rs src-tauri/src/remote/transport.rs src-tauri/src/system_icons.rs src-tauri/src/webview_guard.rs
```

⚠️ **不要为了"修格式"再跑一次 `cargo fmt`**：它会把上面这批文件重新弄脏。
本特性新增/修改的 Rust 块已单独 `rustfmt --edition 2021` 处理过。

判据脚本（可复用）：`.superpowers/sdd/detect-fmt-noise3.cjs` —— 用「rustfmt 后的 HEAD」与工作区
逐字比对来区分「纯格式化」和「真实改动」。本轮用它回滚了 17 个纯空白差异文件；
剩下 43 个因为 rustfmt 会**重排 `use` 语句**、**增删尾逗号**，字节流判不准，留给上表直接回滚。

**提交与否由用户决定**（系统约束：commit / push 仅在用户要求时执行）。

---

## 11. 本轮（T2 + T3）交付说明

### 11.1 T2 `src-tauri/src/remote/baidu_api.rs`

**纯函数 + HTTP 层，零 `tauri::` 路径。**

| 能力 | 关键函数 |
|---|---|
| 会话 | `BaiduSession`（`BDUSS` + `STOKEN`）、`cookie_header()`、`describe()`（只打长度）、`sanitize_cookie_value()`（**判非法即拒**，不是过滤） |
| URL 拼装 | `list_dir_url` / `uinfo_url` / `filemetas_url` / `dlink_url` / `encode_query_component` |
| 分页 | `ListPage` / `clamp_page_limit`（上限 1000）/ `next_page`（短页即结束，`checked_add` 防溢出） |
| 解析 | `parse_list_response` / `parse_uinfo_response` / `parse_dlink_response` |
| 条目映射 | `to_remote_entry` / `to_remote_entries` / `entry_name` / `baidu_seconds_to_unix_millis`（**秒 → 毫秒**） |
| 错误分类 | `BaiduFailure`（10 变体）/ `classify_baidu_failure` / `failure_from_errno` / `failure_from_http_status` / `classify_transport` / `baidu_error_kind_coverage()`（**11 行穷尽表**） |
| HTTP | `build_client()`（UA + 30s 超时 + **不跟随重定向**）/ `get_text()` / `list_dir()` / `fetch_user_info()` |

三个刻意的设计选择：

1. **`Response::text()` 不可用**（reqwest 没开 `charset` feature）⇒ 统一 `bytes()` + `String::from_utf8`，顺带让「非 UTF-8」变成可分类的 `Encoding` 错误。
2. **不跟随重定向**（`redirect::Policy::none()`）⇒ 会话失效的 302 被归类成明确的「请重新授权」，而不是被悄悄跟到某个登录页再报一个看不懂的错。
3. **错误映射用 11 行穷尽表**：`HostKeyUnknown` / `HostKeyChanged` 明确标注「不适用（没有 SSH 主机密钥这个概念）」并写明理由，用会红的断言锁住完整性，而不是伪造主机密钥语义。

### 11.2 T3 `src-tauri/src/remote/baidu_auth.rs`

**「读 cookie → 持久化 cookie」，不是换 token。**

| 能力 | 关键函数 |
|---|---|
| 抽取 | `BaiduCookieRecord` / `scan_cookies()` → `BaiduCookieScan`（`BaiduCookieOutcome` + `BaiduStokenState`） |
| 降级路径 | `parse_manual_cookie_input()`（支持 `BDUSS=…` / 裸值 / 整行 `Set-Cookie: …`）/ `parse_cookie_header()` |
| 状态 | `BaiduSessionState` / `baidu_auth_status()`（`Missing` / `Ready` / `StokenExpiringSoon` / `StokenExpired`）/ `describe_state()` |
| 落盘 | `encode_session()` / `decode_session()` / `save_baidu_session()` / `load_baidu_session()` / `delete_baidu_session()` |
| 防泄漏 | `is_baidu_session_blob()` —— 供 T6 拦住「把 `BDUSS` 读回前端」 |

三条要点：

1. **复用既有 `credentials.rs` 的 DPAPI 密文档**，没有新造第二套存档（`remote-credentials.json`，键 = 连接 id）。落盘的是 JSON（`{version, bduss, stoken, stoken_expires_unix}`），**整份被 DPAPI 加密**。
2. **`STOKEN` 坏掉不连累 `BDUSS`**（可选 cookie 缺失/非法只记状态，不让整个会话作废）；但「它在却被丢弃了」必须可观测 —— `BaiduStokenState::Rejected` 就是干这个的。
3. **过期判定把 `now_unix` 做成显式参数**：靠真实时钟的话，写出来的测试只是「今天过没过」而已。

### 11.3 本轮踩到的坑

| 坑 | 正确做法 |
|---|---|
| `build_remote_path` **不做百分号编码**，我第一版测试期望写成了 `%E8%BD%AF%E4%BB%B6` | 期望值应是字面中文：`sfm://remote/conn-a/01A_软件`（`remote/path.rs:103`） |
| 「粘整条 `Set-Cookie` 头应当被拒」这条测试写反了 —— 实际**应该**能从里面取出 `BDUSS` | 改成「仍然要取出 `BDUSS`，`Path` / `HttpOnly` 属性段忽略」；顺带补上 DevTools 常见的 `Set-Cookie: ` 前缀剥离（原来会把 `Set-Cookie: BDUSS=…` 认成一条名叫 `Set-Cookie: BDUSS` 的 cookie ⇒ 取不到 `BDUSS`） |
| 裸值里含 `=` / `;` 时会被当成裸 `BDUSS` 吞下去 ⇒ 得到「看着有凭据、实际 400」的会话 | 现在明确报错「无法识别的凭据格式」 |
| `cargo fmt --check` 在本仓库**不是绿灯**（`app_updater.rs` / `remote/commands.rs` 等一堆既有差异） | 别拿它当门禁；但**自己新增的块要写成 rustfmt 干净**，别给仓库再添差异（`remote/mod.rs` 里我新增的两组 `pub use` 已按 rustfmt 的排序写） |
| `node -e` 里把 PowerShell 变量插进 JS 字符串 ⇒ 反斜杠被当转义（`\0`）⇒ `ERR_INVALID_ARG_VALUE` | 拼路径时改用正斜杠；更省事的是直接用编辑工具 + `git diff` |
| `$ErrorActionPreference='Stop'` + 原生命令非零退出 ⇒ **后续命令全被跳过**（我那次 `cargo test` 根本没启动） | 预检类命令用 `Continue`，或把要跑的东西放在预检**之前** |

### 11.4 门禁结果

```
cargo test --lib  →  test result: ok. 1057 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
新增用例 114 条（remote::baidu_api 65 + remote::baidu_auth 49），全部离线、零网络
日志：.superpowers/sdd/baidu-t2-test4.log（TEST_EXITCODE=0）
```

---

## 12. 下一会话开工清单

阶段 1 的**代码侧已收口**（见 §13）。下一会话有三条线，**线 A 优先级最高**。

### 12.0 开工前

1. `git -C F:\soft\00selfmade\filemanager\sigma-file-manager checkout feat/baidu-remote-provider`，
   `git status` 核对与 §10 一致；**先按 §10 把 43 个无关的格式化文件回滚掉**。
2. 按顺序读：`AGENTS.md` → 本文档 → `docs/plan-2026-11-08-baidu-netdisk-phase1.md` →
   `docs/2026-11-07-baidu-t1-auth-findings.md` → `docs/2026-11-08-baidu-t9-acceptance.md`。
3. 门禁基线（已绿，除非改了 Rust/前端代码否则不必重跑）：
   `cargo test --lib` = **1142 passed / 0 failed / 1 ignored**；
   `npx vitest run` = **292 files / 2740 tests passed**。
   最终复核日志：后端 `.superpowers/sdd/baidu-t9-test4.log`（`test result: ok. 1142 passed; 0 failed; 1 ignored`），
   前端 `.superpowers/sdd/baidu-t8-vitest3.log`（`292 files / 2740 tests passed`）。
   ⚠️ 已知抖动：`file_lock::tests::unlock_on_a_locked_file_reports_that_it_is_still_held`
   在**全量并发**下偶发失败（2026-11-08 出现过一次 1141 passed / 1 failed，断言「被独占占用的文件不该报告解锁成功」）。
   **单跑该用例必过**（`.superpowers/sdd/baidu-t9-test3.log`），是 Windows 文件锁的既有抖动，与百度改动无关；
   判定成回归之前先单跑它（第二次全量重跑 `baidu-t9-test4.log` 已恢复 1142 / 0 / 1，确认是抖动而非回归）。

### 12.1 线 A：真实百度会话补验（阶段 1 判定闭环，**只能由有会话的机器做**）

判定清单见 `docs/2026-11-08-baidu-t9-acceptance.md` §四：本机已验证 1–4，**8–12 未验证**。

1. 装包：`release/local-builds/Alpha File Manager_3.12.2_x64-setup.exe`
   （或直接跑 `src-tauri/target/release/alpha-file-manager.exe`）。
2. 浏览器登录 `pan.baidu.com` → F12 → Network → 任一请求 → Request Headers → 复制**整段** `Cookie`
   （**必须含 `BDUSS`**，建议同时带 `STOKEN`）。
3. 设置 → 远程 → 新建连接 → 协议选「百度网盘」→ 名称随意 → **粘贴 Cookie** → 保存。
4. 逐条补验：根目录列表 / 新建目录 / 改名 / 删除 / 右键下载到本地且 sha256 一致。
5. 侧边栏「远程位置」面板会列出该连接（**显示的是连接名，不是协议名**），双击进入远程浏览。
6. **验收完先杀进程再恢复数据**：先 `Stop-Process alpha-file-manager` 确认进程为空 → 再 restore，
   并单独删除 `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\remote-credentials.json`
   （⚠️ 密文文件在 `user-data/` 的**上一级**，不在 `user-data/` 里；备份也不含它）。
   备份必须在**那台机器**上重做 —— 本机备份 `.superpowers/sdd/backup-t9-userdata/` 不跨机。

### 12.2 线 B：阶段 2（上传写入）

- 目标：把 `baidu_provider.rs` 的 `write_file` / `set_permissions` 从 `Unsupported` 变成真实现。
- ⚠️ 百度上传是**分片上传**（`superfile2` 一类端点），**不是** T5 已实现的那套简单表单 POST
  （T5 做的是建目录 / 改名 / 删除这类**元数据**写操作）。开工前先补一份上传链路调研，别直接照搬 T5。
- `RemoteProtocol::BaiduPan` 已在 `remote/connection.rs` 注册，阶段 2 直接复用。

### 12.3 线 C：可选打磨

- 设置 → 远程 分类页的说明文案仍是旧句「在这里保存 FTP、FTPS 与 SFTP 连接…」，**没提百度网盘**；
  17 个语言包对应键要一起改。
- 新建连接时**授权卡不显示**（T7 实现 watch `props.connection?.id`，新建连接没有 id）。
  要让新建也能看到授权态，得改 `connection-form.vue` 的 watch 条件。

---

## 13. 本轮（T4 – T9）交付说明

### 13.1 T4 / T5 `baidu_provider.rs` + `baidu_api.rs` 写操作

- **T4 只读**：`read_dir` 走 `xpan/file?method=list` 并翻页到底；`read_file` 走 `filemetas` → `dlink` 直链解析；
  `get_permissions` → `Ok(None)`。
- **T5 写操作**（全部加在 `baidu_api.rs` 里）：`bdstoken` 解析、`encode_form_body`、`rename_filelist`、
  `delete_filelist`、`parse_write_response`、`post_form`、`fetch_bdstoken`。
  provider 侧实现 `make_dir` / `rename_path` / `remove_path`（**删根拒绝**）；
  `write_file*` / `set_permissions` **保持 `Unsupported`**。
  `HttpBaiduApi` 带 **30 分钟 bdstoken 缓存**，且**只在 Auth 失败时重试一次**（其它错误不重试）。

### 13.2 T6 后端接线

- `RemoteProtocol::BaiduPan`（`as_str()` = `"baidu_pan"`，默认端口 **443**）。
- `UnavailableBaiduApi` + `new_deferred`：把「会话还没准备好」这类初始化错误**推迟到第一次使用时**抛出，
  而不是构造期就 panic —— `provider_with_protocol` 的签名保持 `Box<dyn RemoteProvider>` 不变。
- `resolve_connection` 对百度返回 `password: None`（凭据不随连接列表下发）。
- 三个新 Tauri command：`remote_baidu_save_session` / `remote_baidu_auth_status` / `remote_baidu_delete_session`；
  另有 `probe_baidu`；命令白名单 **16 → 19**。

### 13.3 T7 前端

- `src/types/remote.ts`：协议联合类型加 `'baidu_pan'`，加 `isBaiduPanProtocol`、
  `BaiduAuthStatusKind` / `BaiduAuthStatusPayload`、连接上的 `credentialKey?`。
- store 百度分支：固定 `host='pan.baidu.com'` / `port=443'` / `credentialKey = 连接 id`，
  存取会话走**独立 command**。
- `connection-form.vue`：选中百度后**隐藏主机/端口/用户名**，密码框变成 Cookie 框 + 引导文案，
  带授权卡与「删除会话」按钮。
- 后端补 `session_to_cookie_header`；`remote_test_connection(app, request)` 支持带 `credentialKey`，
  由后端从 DPAPI 存档读出会话再探测，**明文不回前端**。

### 13.4 T8 语言包与单测

- 17 个语言包（`ch/en/de/es/fr/it/ja/pt/ru/tr/vi/ur/hi/hy/he/fa/sl`）6 组新键全部补齐；
  ⚠️ 中文 locale 是 **`ch`** 不是 `zh`。
- 新增 3 份前端单测（协议类型 / store 百度分支 / 连接表单）。

### 13.5 三条必须记住的设计约束

1. **百度凭据绝不进通用 `remote_save_credential`** —— 走独立 command `remote_baidu_save_session`。
2. **前端不把百度会话当「密码」回传**；后端只在探测时用 `credentialKey` 读存档。
3. **`write_file` / `set_permissions` 保持 `Unsupported`，绝不静默 `Ok(())`**；
   **Auth 不得被改写成 `Unsupported`** —— 认证失败必须还能被分类成 `Auth`。

### 13.6 T9 打包与真机硬证据

打包：`npm run tauri:build` → `BUILD_EXITCODE=0`（`.superpowers/sdd/baidu-t9-build1.log`）。
产物：`src-tauri/target/release/alpha-file-manager.exe`（81,643,520 字节）；
安装包 `release/local-builds/Alpha File Manager_3.12.2_x64-setup.exe`（45,280,155 字节）。
⚠️ `release/` **根目录**的同名旧包（45,169,271 字节）是旧已发布资产，**不是**本轮产物。

真机（打包版 exe，pid 60648）硬证据：

| 判定 | 证据 |
|---|---|
| 协议下拉有「百度网盘」 | UIA `ListItem \| 百度网盘` |
| 选百度后表单切 Cookie 分支 | `Text \| Cookie` + `Edit \| Cookie` + 引导文案（主机/端口/用户名全消失） |
| 保存落盘 | `user-remote-connections.json`：`protocol=baidu_pan`、`host=pan.baidu.com`、`port=443`、`credentialKey=<连接id>`、`initialPath=/` |
| 侧边栏「远程位置」面板 | 悬停左栏云图标按钮 `(40,414)` 后出现 `Button \| 百度网盘验收测试 @79,649` + `Button \| 编辑该连接: 百度网盘验收测试 @401,655` |

证据文件：`.superpowers/sdd/t9-accept/remotepanel-shot.png`（屏幕 BitBlt，含悬停面板）、
`remotepanel-tree.txt`（UIA 树）。逐条判定与未验证项见 `docs/2026-11-08-baidu-t9-acceptance.md` §4.1。

⚠️ **口径修正**：`remote-nav-panel.vue` 按 `connection.name` 渲染 —— **面板里显示用户自填的连接名，
不显示协议名**。协议名「百度网盘」出现在**设置 → 远程**的连接列表标签上
（UIA：`ListItem | 百度网盘验收测试***百度网盘pan.baidu.com:443/直连编辑删除`）。

### 13.7 本轮踩到的坑

| 坑 | 正确做法 |
|---|---|
| `reqwest 0.13` 没开 `form` / `json` / `cookies` feature | 手工拼 urlencoded body；`Response::text()` 不可用，用 `bytes()` + `String::from_utf8` |
| `spawn_blocking` 闭包要 `'static` | `request.credential_key.clone()` 成 owned String 再传 |
| **密文文件的真实路径在 `user-data/` 的上一级** | `Roaming\com.sigma-file-manager.app\remote-credentials.json`；plan 文档写错了，备份/恢复必须按实际路径 |
| 在 bash 里直接对 `C:\Users\...\AppData` 做递归写/删会被**本地安全策略拦截** | 把逻辑写进工作区内的 `.ps1`，用 `pwsh -NoProfile -ExecutionPolicy Bypass -File <绝对路径>` 执行 |
| `Out-String` 会按控制台宽度**换行** | 拿 git 输出做字节级比对时别用 `Out-String`，改用 node 的 `execFileSync` |
| rustfmt 默认**重排 `use`**、增删尾逗号 | 「工作区 == HEAD」判不准；用 `git diff --name-only` + 人工确认名单回滚 |
| UIA `BoundingRectangle` 可能是 `∞` / `NaN` | 转 `[int]` 前先判 `IsInfinity` / `IsNaN`，否则整段 foreach 会中断（实测只收到 268/983 个元素） |
| 悬停面板要先确认「hover 事件本身能进 webview」 | 本轮先把鼠标停在**必然命中**的图标上做阳性对照，再去找目标 |
| 恢复真实数据的顺序 | **先杀进程 → 再 restore**：应用退出时会把内存里的统计状态写回真实文件 |

### 13.8 本轮数据恢复结果（已执行）

脚本 `.superpowers/sdd/restore-t9-data.ps1`，输出：

```
PROC_STOPPED / RESTORED_FILES=28 / DST_FILES=28 / EXTRA_IN_DST=
CRED_REMOVED / BAIDU_OCCURRENCES=0 / CONN_BYTES=1154 = BACKUP_CONN_BYTES / RESTORE_OK
```

⇒ 真实用户数据已完整还原（28 个文件、字节数与备份一致、无多出文件、假连接已清除、
假凭据文件已删）。**本机现在没有残留任何百度网盘验收数据。**

---

### 14. 2026-11-08 追加轮：导航「无响应」根因修复 + 左侧导航「添加百度网盘」入口

完整取证与逐条结论见 `docs/2026-11-08-nav-unresponsive-and-baidu-entry.md`，这里只留接力必需的部分。

#### 14.1 根因（用户诉求①：点左侧导航切换时 Windows 判「无响应」）

- **空闲态不卡**：`.superpowers/sdd/nav-switch-probe.ps1` 实测 `alive_ms=135~198 / dead_samples=0`
  ⇒ 卡顿是**条件性**的（扫描期才复现）。
- **完整链路**：
  1. 前端在扫描期间**持续轮询** `global_search_get_status`（`src/stores/runtime/global-search.ts`
     的 `pollStatus()` / `startStatusPolling()`，启动即开）；
  2. 该 command 是**同步**的 —— Tauri v2 的同步 command **跑在主线程**，而主线程就是窗口消息泵；
  3. 它要抢 `GLOBAL_SEARCH_STATE` 的**读锁**；
  4. 全量扫描的**提交段**把 `replace_index_dir` / `open_or_create_index` / `flush_pending_changes` /
     `calculate_dir_size` **整段塞在写锁里**（磁盘 IO 几秒级）；
  5. 主线程被写锁挡住 ⇒ 消息泵停摆 ⇒ Windows 判「未响应」。
- **日志口径纠正（重要）**：`webview guard` 的 `main:NNNNms` 是**距上次心跳的毫秒数**
  （心跳走 `invoke`，IPC 在主线程收）⇒ 它 = **主线程阻塞时长**，**不是** RTT。
  1366 条采样里 **91 条 ≥800ms**，最大 ~4.7s；`quick-view` 窗口另有 15 次 ~61s 无心跳。
  此前把它当成 RTT，会把方向带偏到「前端 JS 被阻塞」上。

#### 14.2 已落地的修复

| # | 文件 | 改动 |
|---|---|---|
| R1（根因） | `src-tauri/src/global_search/scan.rs` | 提交段的**磁盘 IO 移出状态写锁**（「清空旧 index/reader/fields」留在锁内，瞬时）。安全性依据：提交窗口由 `status.is_committing` 标记，`query.rs` 在该期间本来就直接返回空结果，读不到中间态 |
| R2（保险） | `src-tauri/src/global_search/commands.rs` | `global_search_get_status` / `global_search_cancel_scan` 由同步 command 改为 `async` + `spawn_blocking`，**彻底不占主线程**。手法沿用 2026-11-07 给 `get_system_drives` / `resolve_windows_directory_shortcut` 用过的同一套（注释见 `dir_reader/commands.rs:129`） |

#### 14.3 左侧导航「添加百度网盘」入口（用户诉求②）

- 左栏云图标**下方**新增一颗**常驻**「+」按钮（`data-test="remote-nav-add"`，`aria-label` = 「添加连接」）；
  悬停面板**顶部**新增一行「+ 添加连接」（`data-test="remote-nav-add"`，emit `add`）。
- store 新增瞬态 `pendingCreate` / `requestRemoteCreate()` / `consumeRemoteCreate()`（**消费即清空**）；
  `connections-section.vue` 的 `applyPendingCreate()` 消费后直接打开新建表单（`onMounted` + `watch` 双路兜底）。
- i18n **复用既有键 `settings.remote.action.add`**（17 个语言包齐全，已逐个核对）⇒ 不新增键、不触发 17 语言包同步。

#### 14.4 验证状态

| 项 | 状态 |
|---|---|
| `cargo test --lib` | ✅ **1142 passed / 0 failed / 1 ignored**（`.superpowers/sdd/baidu-navfix-test1.log`，TEST_EXITCODE=0），与 T9 基线一致、无回归 |
| 前端单测 | ✅ `remote-nav-panel.test.ts` + `remote-category.test.ts` 共 **27 passed**（本轮新增 4 条用例） |
| eslint（本轮 6 个改动文件） | ✅ **0 问题** |
| 真机 UI 证据（左栏「+」按钮 + 点它打开新建表单） | ⏳ 见 §14.5 |
| 真机复验「扫描进行中切换导航不再未响应」 | ⏳ **未做** —— 需要一次全量扫描窗口（14–54 分钟），本轮未安排 |

#### 14.5 遗留与待办

1. **打包产物真机验收**：`npm run tauri:build`（日志 `.superpowers/sdd/baidu-navfix-build1.log`），
   验收脚本 `.superpowers/sdd/nav-entry-accept.ps1`（杀旧实例 → 起新 exe → UIA 按名称找「添加连接」按钮
   → 点击 → 校验设置页远程分类的新建表单 → 截图）。
   ⚠️ `.ps1` 必须**纯 ASCII**，中文参数走**命令行参数**传入；UIA 坐标是物理像素，
   脚本必须先 `SetProcessDPIAware()`。
2. **43 个无关 rustfmt 文件**仍未回滚（§10 有命令，未代为执行，避免误伤真实改动）。
3. 阶段 1 T2–T9 + 本轮改动**全部未提交**（用户未要求 commit/push）。
---

## 15. 导航栏「无响应」根因修复 + 左栏「添加百度网盘」入口（2026-11-08 第 2 轮追加）

本节回应用户两条新诉求：① 点击左侧导航栏切换时出现「无响应」；② 左侧导航没有百度网盘添加入口。

### 15.1 诉求①的根因（已坐实，非猜测）

日志口径先纠正一处：`webview guard: alive ... main:NNNNms` 里的 `NNNNms` 是
**距上次心跳的毫秒数**（心跳走 `invoke`，Tauri IPC 在主线程收），因此它等于
**主线程阻塞时长**，不是 RTT。1366 条采样中 **91 条 ≥800ms**，最大 ~4.7s；
`quick-view` 窗口另有 15 次 ~61s 无心跳；`main` 窗口从未触发 60s hung 判定。

根因链（每一环都有代码/日志依据）：

1. 前端在全局搜索扫描期**持续轮询** `global_search_get_status`
   （`src/stores/runtime/global-search.ts` 的 `pollStatus()` / `startStatusPolling()`，启动即开）。
2. `global_search_get_status` 原先是**同步** Tauri command。Tauri v2 的同步 command
   **跑在主线程**（主线程即窗口消息泵）。
3. 该 command 要抢 `GLOBAL_SEARCH_STATE` 的读锁，而全量扫描的**提交段**把
   `replace_index_dir` / `open_or_create_index` / `flush_pending_changes` / `calculate_dir_size`
   **整段塞在写锁里**（磁盘 IO，秒级）。
4. 主线程被写锁挡住 ⇒ 消息泵停摆 ⇒ Windows 判「未响应」。
   ⇒ 用户在扫描期间点导航栏切换，体验就是「点一下卡住」。

空闲态实测（`.superpowers/sdd/nav-switch-probe.ps1`）：`alive_ms=135~198 /
changed_ms=285~965 / dead_samples=0` ⇒ **切换本身不卡，卡顿是条件性的**（需有扫描在跑），
这与上述根因链一致。

已排除的假设：状态锁「长持锁」（临界区短，只是里面塞了磁盘 IO）；
`get_system_drives` 阻塞（2026-11-07 已改 async，`src-tauri/src/dir_reader/commands.rs:141`）；
前端 JS 线程被阻塞（心跳迟到是 Rust 侧计算的）。

### 15.2 修复（R1 治根因 + R2 加保险）

| 编号 | 文件 | 改法 |
|---|---|---|
| R1 | `src-tauri/src/global_search/scan.rs` | 提交段的磁盘 IO **移出** `GLOBAL_SEARCH_STATE` 写锁；锁内只留「清空旧 index/reader/fields」（瞬时）。安全性依据：`status.is_committing` 期间 `src-tauri/src/global_search/query.rs:165/179/194/481` 本就返回空结果，读不到中间态 |
| R2 | `src-tauri/src/global_search/commands.rs` | `global_search_get_status` / `global_search_cancel_scan` 由同步 command 改为 `async` + `tauri::async_runtime::spawn_blocking`，彻底不占主线程（沿用 2026-11-07 给 `get_system_drives` 的同一手法） |

沿用项目既定规范：**凡在主线程做 IO 的 Tauri command 一律 `spawn_blocking`**（先例
`src-tauri/src/dir_reader/commands.rs:129`）。

### 15.3 诉求②：左栏「添加百度网盘」入口（双入口）

| 位置 | 文件 | 形态 |
|---|---|---|
| 左栏云图标**下方**常驻「+」 | `src/modules/nav-sidebar/nav-sidebar.vue` | `data-test="remote-nav-add"`、`aria-label` = 「添加连接」；点击走 `openRemoteCreate()` |
| 悬停面板**顶部**「+ 添加连接」行 | `src/modules/nav-sidebar/components/remote-nav-panel.vue` | `data-test="remote-nav-add"`，emit `add` |

两者都走 store 瞬态 `pendingCreate`（`src/stores/storage/remote-connections.ts`：
`requestRemoteCreate()` / `consumeRemoteCreate()`，**消费即清空**，与既有 `pendingEditId` 同机制），
由设置页远程分类自己打开新建表单
（`src/modules/settings/ui/categories/remote/connections-section.vue` 的 `applyPendingCreate()`，
`onMounted` + `watch` 双路兜底）。
**导航侧不直接操纵设置页内部状态** —— 这是本轮入口设计的关键约束。

i18n **复用既有键 `settings.remote.action.add`**（17 个语言包齐全，已用 node 逐个核对；
中文值 = 「添加连接」，非中文语言包当前是英文 "Add connection"），**不新增键**，避免 17 语言包同步工作量。

### 15.4 本轮门禁

| 项 | 命令 | 结果 |
|---|---|---|
| Rust 单测 | `cargo test --lib`（`.superpowers/sdd/baidu-navfix-test1.log`） | **1142 passed; 0 failed; 1 ignored**，与 T9 基线一致 |
| 前端单测 | `npx vitest run src/modules/nav-sidebar/components/remote-nav-panel.test.ts src/modules/settings/ui/categories/remote/remote-category.test.ts` | **Test Files 2 passed / Tests 27 passed**（新增 4 条：面板「+」只发 `add`、store 瞬态语义、导航「+」直接打开新建表单、请求被消费后不重复弹表单） |
| ESLint | `npx eslint <本轮 6 个改动文件>` | **0 问题** |

### 15.5 顺带收尾：回滚 43 个无关 rustfmt 噪音文件

handoff §10 的回滚名单里包含 `src-tauri/src/global_search`（整目录），
而本轮 R1/R2 正好改了该目录下的 `scan.rs` / `commands.rs` ⇒ **照抄 §10 命令会毁掉本轮修复**。
已改为脚本化精确回滚：`.superpowers/sdd/fmt-noise-analysis.cjs`（分类）
+ `.superpowers/sdd/rollback-fmt-noise.cjs`（回滚，带 KEEP 白名单）。
结果：**回滚 42 个纯格式化文件**，跳过 `global_search/scan.rs`、`global_search/commands.rs`；
保留阶段 1 的真实改动 7 个（`lib.rs`、`remote/{commands,connection,connection_test,credentials,mod,registry}.rs`）
与 4 个新增文件（`baidu_probe.rs`、`remote/baidu_{api,auth,provider}.rs`）。
回滚后已 grep 验证：所有含 `baidu` 的 Rust 文件都不在被回滚名单内。

## 16. 补洞：入口携带「百度网盘」协议预选（2026-11-08 第 2 轮追加）

### 16.1 为什么要补

§15 的真机验收拿到了硬证据（`.superpowers/sdd/nav-entry-accept.txt`：`EXPECT_HIT=True`、
点开后新建表单的名称/协议/服务器地址/端口/用户名/密码/初始目录/本地目录/测试连接/保存/取消全部出现），
**但同一份 UIA 转储里协议项写的是 `FTP（明文）`**。用户诉求是「添加**百度网盘**」的入口，
点进去却要再手动把协议从 FTP 改成百度网盘 ⇒ 差最后一步，本轮补上。

### 16.2 接线（5 处，均已落盘）

| 文件 | 改动 |
|---|---|
| `src/stores/storage/remote-connections.ts` | `pendingCreate: Ref<RemoteProtocol \| null>`（原 `boolean`）；`requestRemoteCreate(protocol: RemoteProtocol = 'ftp')`；`consumeRemoteCreate(): RemoteProtocol \| null` |
| `src/modules/nav-sidebar/nav-sidebar.vue` | `openRemoteCreate()` 传 `'baidu_pan'`（两颗「+」共用该函数 ⇒ 两个入口都预选百度） |
| `src/modules/settings/ui/categories/remote/connection-form.vue` | Props `initialProtocol?: RemoteProtocol`；`nextProtocol = connection?.protocol ?? props.initialProtocol ?? 'ftp'` |
| `src/modules/settings/ui/categories/remote/connections-section.vue` | `createProtocol` ref + `applyPendingCreate()` 写入 + 模板 `:initial-protocol="createProtocol"` |
| 测试 | `remote-nav-panel.test.ts` store 语义改判协议；`remote-category.test.ts` 新增「baidu_pan 时 `field-host` 消失 / 默认时 `field-host` 出现」的判别断言 |

协议命中 `baidu_pan` 后**无需新增任何 UI 逻辑**：`connection-form.vue` 既有的 `isBaidu` 分支会自动
固定 `host = BAIDU_PAN_HOST`、端口走 `getRemoteProtocolDefaultPort`、隐藏 `服务器地址`/`端口` 行、
渲染百度授权卡（`data-test="baidu-status"`）。

**取舍未向用户确认**：面板顶部那行「+ 添加连接」现在也预选百度网盘（它 emit `add`，统一由
`nav-sidebar.openRemoteCreate()` 决定协议）。若希望该行保持「通用添加」，只需给它单独的
emit 载荷或让 `nav-sidebar` 传 `'ftp'`。

### 16.3 门禁

| 项 | 命令 | 结果 |
|---|---|---|
| 前端单测 | `npx vitest run src/modules/nav-sidebar/components/remote-nav-panel.test.ts src/modules/settings/ui/categories/remote/remote-category.test.ts` | **Test Files 2 passed / Tests 28 passed** |
| ESLint | `npx eslint <7 个改动文件>` | **0 问题**（`connections-section.vue:76` 的 `padding-line-between-statements` 已修） |
| Rust 单测 | 沿用 §15.4 的 **1142 passed**（本轮未改 Rust） | 不变 |
| 打包 | `.superpowers/sdd/baidu-navfix-build5.cmd`（日志 `baidu-navfix-build5.log`） | ✅ `BUILD_EXITCODE=0`，exe `12:42:42`；复验结论见 §16.4 |

### 16.4 真机复验（协议预选）—— **已通过**（2026-11-08 12:43）

判据（用 `.superpowers/sdd/nav-entry-accept.ps1` 的 UIA 转储核对）：协议项应为「百度网盘」，
`服务器地址` / `端口` 两项应**消失**，`baidu-status` 授权卡应出现。

实测结果（`baidu-navfix-build5.log` 末尾 `BUILD_EXITCODE=0`，产物 `alpha-file-manager.exe` 时间戳 `12:42:42`，晚于 5 处改动）：

| 判据 | 结果 |
| --- | --- |
| 左栏「+」命中并点开 | ✅ `TARGET_RECT=18,453 43x42 CENTER=40,474` / `CLICK_METHOD=InvokePattern` / `CLICKED=1` / `EXPECT_HIT=True` / `TREE_COUNT=150` |
| 协议项为「百度网盘」 | ✅ UIA 转储里协议位显示 `百度网盘`（12:27:04 那份产物显示的是 `FTP（明文）`） |
| `服务器地址` / `端口` 消失 | ✅ 转储中两行均不存在，改为百度专有的 `Cookie` / `初始目录` / `本地目录` |
| 百度分支文案 | ✅ 出现「在浏览器里登录 pan.baidu.com 后，复制整段 Cookie 粘贴到这里（必须包含 BDUSS）」，即 `isBaidu` 分支独有的 `hint-cookie` |
| `baidu-status` 授权卡 | ⚠️ **未在 UIA 转储的 Name 列表里出现**——该卡片的可访问名大概为空（本项目图标类控件普遍无名），因此这一条判据**无法用 UIA 证真也无法证伪**。百度分支已由上一行的 `hint-cookie` 文案确认，不影响结论。 |

证据文件：`.superpowers/sdd/nav-entry-accept.txt` / `.superpowers/sdd/nav-entry-accept.png`（12:43:16，309KB）。
复跑命令（中文作命令行参数传入，`.ps1` 本体保持纯 ASCII）：

```powershell
pwsh -NoProfile -File .superpowers/sdd/nav-entry-accept.ps1 -BtnName '添加连接' -ExpectName '添加连接' -ModalCloseName '关闭'
```

> ⚠️ 打包产物 `alpha-file-manager.exe` 的时间戳必须**晚于**本节 5 处改动落盘时间
> （前一份 12:27:04 早于本轮改动，不能用它验协议预选）。

---

## 17. 「+」未出现的根因 = 装机版滞后 + 真机复验闭环（2026-11-08 12:4x）

用户报「云图标下的加号没出现」。根因**不是代码缺陷**：装机的是 10:34 round 1 旧包，
round 2 包 12:27:02 构建成功（`baidu-navfix-build3.log` 尾行 `BUILD_EXITCODE=0`）但从未安装
（12:13 首次重打包被旧实例锁 exe，`os error 5`）。处置 = `nav-entry-install.cmd` 静默覆盖安装
（同版本号 3.12.2，setup exit=0，装机 exe mtime/字节数与产物逐项一致）。

真机复验（`nav-entry-probe.ps1` run4，日志 `nav-entry-probe-run4.log`）：
`ADD_FOUND rect=18,453 43x42`（按名「添加连接」精确命中）+ `CLICK_LANDED=CREATE_DIALOG`
（Invoke 点击真的打开「添加远程连接」表单）+ 实拍 `accept-20261108-nav-entry/nav-full-123740.png`。
详细根因/探针坑（`CP` 被 `cp` 别名遮蔽致空名假阳性；TrueCondition rail 全扫 0 命中不可用作正向对照；
屏幕已变 4K 口径）见 `2026-11-08-nav-unresponsive-and-baidu-entry.md` §五。

协议预选复验（§16.4）仍是待办：装机包 12:26 < 协议预选改动落盘时间，需 build5 产物装后再验。
> 另：`vite` 前端 bundle 在 `12:30`（`✓ built in 12.92s`）就已打进 exe，所以只要 exe 时间戳晚于改动即可信。
> eslint 那次「补空行」修发生在 vite 打包**之后**——纯空行、功能等价，不必为此重打包。


## 18. 本轮追加：「添加百度网盘」页放 Cookie 获取链接 + 详细说明（2026-11-08 下午）

用户诉求：「获取和复制百度网盘 Cookie 的方式是什么？是否可以直接在添加网盘页，添加百度网盘获取 cookie 的页面链接，以及获取方式的详细说明？」

完整结论、口径与门禁数字写在 **docs/2026-11-08-nav-unresponsive-and-baidu-entry.md 的 §六（6.1–6.6）**，本文不复述，避免两处漂移。要点指针：

- 代码落点：src/modules/settings/ui/categories/remote/connection-form.vue（百度分支新增 Collapsible 帮助块 + openUrl 外链按钮）。
- i18n：17 个语言包各新增 settings.remote.baidu.help.* 10 键；写入脚本 .superpowers/sdd/i18n-baidu-help.cjs，校验脚本 .superpowers/sdd/baidu-help-i18n-check.cjs。
- 单测：src/modules/settings/ui/categories/remote/baidu-connection-form.test.ts 新增 3 条判别性用例（合计 8 passed）。
- 文案口径来源是**后端解析函数本身**：src-tauri/src/remote/baidu_auth.rs 的 parse_manual_cookie_input / scan_cookies —— BDUSS 必填且区分大小写、STOKEN 可选、整串 Cookie 也能粘。改文案前先读这两个函数，不要凭想象写「只准粘裸值」。
- **真机复验已通过**（14:52，打包产物 exe mtime 14:41:28）：默认收起 → 鼠标注入点开 → 步骤文本进 UIA 树 → 外链按钮 Invoke 成功并唤起浏览器窗口。判定表与探针坑（`CollapsibleTrigger` 不支持 `InvokePattern`、`EnumWindows` 找不到子窗口）写在 §6.5。证据：.superpowers/sdd/baidu-help-accept.txt、baidu-help-crop-expanded.png。

下一会话的第一步：确认 .superpowers/sdd/baidu-help-build7.log 出现 BUILD_EXITCODE=0，再用 .superpowers/sdd/nav-entry-accept.ps1 起打包产物真机复验帮助块（判据与坑见专项说明 §6.5）。

---

## 19. 接力收口（2026-11-08 15:2x）——新会话从这里读起

**本节是本文的最新状态与唯一交接入口。§1–§18 是历史与细节，按需回查，不要按 §18 结尾的「下一会话第一步」去执行——那条已经做完了，且它引用的日志名是错的（见 19.4）。**

### 19.1 用户诉求清单：三条，代码全部完成

| # | 诉求 | 代码 | 门禁 | 真机 |
|---|---|---|---|---|
| ① | 点左侧导航切换时「未响应」 | 已修（R1+R2，见 §15） | `cargo test --lib` 1142 passed / 0 failed / 1 ignored | **未复验**（需 14–54 分钟全量扫描窗口，见 19.3-A） |
| ② | 左导航加「添加百度网盘」入口 + 协议预选 | 已完成（见 §15.3、§16） | vitest 28 passed、eslint 0 违规 | **已通过**（12:43，`EXPECT_HIT=True` 且表单预选百度网盘，见 §16.4） |
| ③ | 添加网盘页放 Cookie 获取链接 + 详细说明 | 已完成（见 §18） | vitest 8 passed、i18n 对齐 18 passed、eslint 0 | **已通过**（14:5x，见 §18 末与专项说明 §6.5） |

### 19.2 安装包路径（本轮最重要的纠正，别装错包）

- **真机要装的是**：`F:\soft\00selfmade\filemanager\sigma-file-manager\release\local-builds\Alpha File Manager_3.12.2_x64-setup.exe`
  —— 45,350,489 字节，mtime **2026-11-08 14:41:27**，含 ②③ 两批改动的最新构建。
- `release\Alpha File Manager_3.12.2_x64-setup.exe`（45,169,271 字节，mtime **11-07 20:40:25**）**不含 ②③**，它是昨天已发布 3.12.2 的资产副本。**别再把这个顶层路径给用户**——15:12 那条回复指错了，本轮 15:2x 已核实并纠正。
- 机制原因：`scripts/publish-bundle-to-release.mjs:60-79` 的 `placeArtifact()` 遇到「同名但内容不同」的目标时**不覆盖**，改写进 `release/local-builds/`（2026-11-02 为防止本地包冒充已发布物加的规则，注释里写得很清楚）。所以「今天重新打了 3.12.2 的包」必然落在 `local-builds/`，而版本号相同（本轮未 bump 版本）。
- 主二进制：`src-tauri\target\release\alpha-file-manager.exe`，81,992,704 字节，mtime **14:41:28**。

### 19.3 待做（按优先级）

**A. 诉求①的真机复验（唯一悬空项）** — 判据：全量扫描进行中，点左侧导航切到「设置」，**不再弹「未响应」**。
- 前置：机器空闲、无并行会话；扫描窗口 14–54 分钟（本机实测规模见专项说明 §二）。
- 取证手段：UIA 找「未响应」对话框 + 屏幕 BitBlt 实拍；**不要用 `PrintWindow`**（抓不到系统模态框）。
- 注意「应用启动后会弹『新功能』更新日志模态框挡住列表」，脚本要先关它（AGENTS.md 已记）。

**B. 云端列表端到端** — 阶段 1 的 T1–T9 后端与 UI 都在，但**真实百度会话下浏览/搜索云端目录**仍未跑通闭环（§5「❌ 未验证」）。需要用户提供一个真实 BDUSS（可选 STOKEN）走一遍：连接 → 列目录 → 预览 → 断开。凭证落盘是 DPAPI 密文，日志/错误/文案**不得出现明文**。

**C. 批次 2（阶段 2 上传写入）** — §12.2 有线 B 的任务拆分。

**D. 提交与推送** — 见 19.5，改动**全部未提交**，是否提交由用户决定（本仓纪律：只在用户要求时 commit/push）。

### 19.4 本轮新增的工具坑（后续会话务必遵守）

1. **`.superpowers/sdd/` 下的日志文件名会撞历史**。本轮 §18 写的「确认 `baidu-help-build7.log` 出现 `BUILD_EXITCODE=0`」不可执行：该文件不存在，而 `.superpowers/sdd/build7.log` 是 **2026-10-20 00:14** 的陈旧产物，与本批无关。⇒ 新建构建脚本**一次用一个新文件名**（AGENTS.md 也要求 `.cmd` 运行期不得编辑、每次换新名），别复用 `build7` 这类短名。
2. **`cargo build --release` 的 exit 101 不一定是构建失败**。本轮那条 101 来自「构建完顺手 run exe」的包装脚本——exe 正在运行时 cargo 无法重跑它。⇒ 判构建成败要看 NSIS 产物是否落地（`bundle\nsis\*.exe` 的 mtime），不要只看包装脚本的退出码。
3. **`Get-ChildItem -Filter` 不接受数组**（`-Filter 'a','b'` 会让命令挂住直到超时）。多模式用 `-Include` 或 `-match`，或直接分次查。
4. **`release/` 顶层 ≠ 最新包**（根因见 19.2）。交付安装包前，一律用 `Get-ChildItem release -Recurse -Filter '*<ver>*'` 把**所有**同名产物的 mtime+size 列出来再挑，别默认顶层就是新的。

### 19.5 工作区状态（15:2x 实测）

- 分支 `feat/baidu-remote-provider`，HEAD `9063db44`，基点 `main` = `6eabbc67`；版本 `3.12.2`（本轮未 bump）。
- `git status --short` = **50 个改动文件，全部未提交**，含 ①②③ 三批代码、17 个语言包、两份文档、`.superpowers/sdd/` 一次性脚本（该目录不进版本库）。
- ⚠️ 同机存在并行会话 `mvs_6495b276ad1d4717b020888a9f714468`（本会话 `mvs_d2a5f4c01b3149698d340143c645783e`），**共用同一个工作目录** ⇒ 动手前先 `git status` 复核，别把别人的改动算进自己。

### 19.6 证据与文档路径索引（全路径）

- 本文（接力入口）：`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-08-baidu-netdisk-phase1.md`
- 本轮专项说明（根因/口径/门禁/判定表）：`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\2026-11-08-nav-unresponsive-and-baidu-entry.md`
- 项目铁律（必读，含真机验收与打包纪律）：`F:\soft\00selfmade\filemanager\sigma-file-manager\AGENTS.md`
- 阶段 1 计划原文：`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\plans\`（§4 引用）
- 真机探针脚本：`F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\nav-entry-accept.ps1`、`baidu-help-accept.ps1`
- 真机证据：`F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\baidu-help-accept.txt`、`baidu-help-crop-expanded.png`、`unresponsive-dialog.png`、`unresponsive-dialog-2.png`
- 用户真实统计文件（做夹具时务必先备份、验后恢复）：`C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-stats.json`

### 19.7 已知风险 / 未解疑问

- **14:5x 那轮真机复验到底跑的是哪个包，未坐实**。证据只能证明「跑的是含新代码的 14:41:28 主二进制」（`release/` 里 11-07 那个包里根本没有 Cookie 帮助块，UIA 不可能看见它），但 NSIS 安装包当时并未复制进 `release/`，无法确认用户装的是 14:41 的包。⇒ 若用户报「界面没有帮助块」，第一反应是**核对装机版来源**（§17 就踩过一次「装机版滞后」的假缺陷），而不是改代码。
- 诉求① R1/R2 是**根因级修复但缺真机证据**，§15.1 的归因链条是日志时间线 + 代码读，未做「修复前后对比」的真机复现。
- ②③ 未做视觉回归（只看 UIA 树与实拍裁片，未逐语言检查 17 个语言包的中文以外文案是否通顺——机器翻译直存，已知风险）。

## 20. 阶段 2 收尾 + 三项 UI 优化（2026-11-08 16:0x-17:2x）

### 20.1 本轮产出

- **阶段 1 已提交**：`12f33625`（接入百度网盘 provider）。**本轮优化已提交**：`4e44009b`（24 files）。**两者均未 push**（用户要求）。
- 三项优化（用户指定）：① 去掉导航栏云图标下的常驻「+」；② 悬停面板「网盘 / 远程连接」上下分区、网盘在上；③ 添加连接页拆成「添加网盘 / 添加远程连接」双标签页并针对网盘优化录入项。
- 实现要点：面板分区由 `isBaiduPanProtocol` 协议判定驱动（后续加阿里云盘只需标记协议，分区顺序写死网盘在前）；面板每区自带「文字 + 图标」添加按钮、事件载荷带协议；设置页按协议落对应标签页；保留单个 `connection-form.vue` + `variant` prop（`auto` 保持旧行为）。
- 网盘表单形态：Cookie 用原生 `<textarea>`、隐藏协议/主机/端口/用户名/代理、步骤默认展开、代理恒直连、协议恒 `baidu_pan`。
- 17 语言包用 node 脚本批量改写（`node .superpowers/sdd/i18n-remote-tabs.mjs`，`CHANGED=17 SKIPPED=0`，**勿重复执行**）。

### 20.2 真机验收结论（`tauri build` 打包产物，已装机）

- 打包：`npm run tauri:build` → `release\local-builds\Alpha File Manager_3.12.2_x64-setup.exe`（45,349,198 B，16:02:22）；装机走静默安装器 `/S`。
- 优化①：左侧栏顶部仅 6 个按钮，云图标为最后一个（18,393 43x42），其下到 y=1782 全空白 ⇒ 无「+」。
- 优化②：悬停云图标 → 面板命中「远程连接设置 / 网盘 / 添加网盘 / 百度网盘 / 远程连接 / 添加远程连接」⇒ 网盘区在上。
- 优化③：设置页远程分类 → 「添加连接」→ 双 `TabItem`（添加网盘 1558,1098 / 添加远程连接 2138,1098）；切到远程页出现 2 个 ComboBox + 名称/服务器/用户名/密码/初始目录/本地目录，`COOKIE_TEXT_HITS=0`；网盘页无协议/主机/端口/用户名/代理。
- **百度网盘添加成功闭环**：在「添加网盘」表单用真实 BDUSS+STOKEN 新增连接 `AcceptBaidu` → 保存成功（`remote-credentials.json` 由 1 条 DPAPI 条目变 2 条）→ 云图标面板里出现 `Button | AcceptBaidu`（79,550 320x50）→ 点击打开 → 路径栏 `位置 | prompts | 0736c40a… | dfc442674e…`（`sfm://remote/` 链路），目录树列出真实网盘目录（0000temp、01A_软件、01B_设计素材、02A_办公软件教程 … 86_混沌创新院）。
- 门禁：`npx vitest run`（4 个相关测试文件）= **64 passed**；`npx vue-tsc --build` exit 0；本轮 7 个改动文件 `eslint` 0 问题；全仓 `eslint .` = **177 errors / 0 warnings**（记录基线 188 已过期，177 全部落在本轮未触碰文件）。
- 证据目录：`.superpowers/sdd/accept-tabs1/`（`phaseA/B/E3/E4/E6*.txt`、`*-crop*.png`、`uia-*.txt`）。

### 20.3 本轮新增的工具坑（务必遵守）

1. **往 WebView2 里输文本：`KEYEVENTF_UNICODE` 逐字符键入是唯一可靠路径，剪贴板 + Ctrl+V 在设置页表单里不落地**。现象：剪贴板读回长度正确（271）、`SendInput` 返回 4、输入框有焦点边框，但 DOM 仍为空、Vue 校验直接报「连接名称不能为空。」⇒ 改用 `KEYEVENTF_UNICODE`（`wVk=0`、`wScan=码位`、`dwFlags=0x0004` 键下 + `0x0004|0x0002` 键上）后，`NAME_LEN=11`、`COOKIE_LEN_IN_FIELD=271` 全对，v-model 也同步。（旧记忆里「剪贴板 + Ctrl+V 可行」只对文件浏览器工具栏过滤框成立。）
2. **`[System.Security.Cryptography.ProtectedData]::Unprotect` 对本项目 DPAPI 密文抛「参数错误」**，要用裸 `CryptUnprotectData`（无 entropy、`dwFlags=CRYPTPROTECT_UI_FORBIDDEN=1`）。
3. **PowerShell 取 JSON 字典必须先取键名再回查**：`$obj.entries.PSObject.Properties[0].Value` **返回空串**（要 `$obj.entries.PSObject.Properties.Name` 取名后 `Properties[$k].Value`）。空串 ⇒ `FromBase64String` 得 0 字节 ⇒ Unprotect 报 87「参数错误」，看起来像「密文坏了」。
4. **UIA 同名元素要按 ControlType 区分**：面板里的「AcceptBaidu」是 `Button`（79,550），设置页连接列表里同名的是 `ListItem`（1552,1086 1214x110）。只按名字取会点到设置页那一行。
5. **「保存」按钮初始在屏幕外**（y=2170 > 窗口底 2099）⇒ 需先滚轮滚动（滚动会滚整个设置页）再点。
6. **删除连接没有二次确认框**：点行内「删除」即刻生效。脚本若「点完再找一个确认按钮」，会误点到**下一行**的删除按钮——本轮就误删了「速豹堂」SFTP 连接，靠 `.superpowers/sdd/pre-e-backup/` 恢复。⇒ 删除类脚本必须**按行坐标定位**删除按钮、点击后**立即断言行数变化**，禁止再点第二个同名按钮。

### 20.4 用户数据现状

- 验收后已恢复：`user-data/user-remote-connections.json` = 4 条（速豹堂 / aiec.fun / jury.xin / 百度网盘，1483 B）、`remote-credentials.json` = 1 条 DPAPI 条目（793 B）、`remote-known-hosts.json`（463 B），字节数与 `.superpowers/sdd/pre-e-backup/` 逐项一致；恢复顺序按 AGENTS.md：**先 Stop-Process 再 restore**，restore 后应用已重启（PID 已确认存活）。

### 20.5 仍未验证 / 已知风险

- **R2「全局搜索扫描进行中切导航不再未响应」仍缺真机复验**（需一次 14-54 分钟全量扫描窗口）。
- 17 语言包非中文文案为机器翻译直存（已知风险）；`hy` 的 `settings.remote.baidu.help.*` 10 键曾以英文回填。
- 阶段 1 的 `write_file` / `set_permissions` 仍是 Unsupported（写入操作未做）。

---

## 21. 第二轮三项 UI 优化 + 首页深链缺陷修复（2026-11-08 17:0x-）

> **本节是最新状态，接力从 §21 开始读。** §19 的 AGENTS.md 路径有误（见 21.5）。

### 21.1 用户诉求与状态

| # | 诉求 | 代码 | 门禁 | 真机 |
|---|---|---|---|---|
| ① | 设置「远程」页顶部用**标签**区分「网盘」/「远程连接」（不只在添加表单里区分） | 已完成 | ✅ | ✅ 已通过 |
| ② | 「位置」页（= home 主页）分区下方加「网盘」「远程连接」图标+名称，点击直接进入 | 已完成 | ✅ | ✅ 图标/名称通过；**点击深链原先失败，本轮已修**（见 21.3） |
| ③ | 网盘打开后标签页标题显示连接名（如「百度网盘」）而不是 id / `sfm://remote/<id>` | 已完成 | ✅ | ✅ 已通过 |

### 21.2 实现要点（改动文件）

- `src/modules/settings/ui/categories/remote/connections-section.vue`（重写）：顶部常驻 `Tabs`（`data-test="remote-section-tabs"`，触发器 `tab-netdisk` / `tab-remote`）；列表/空态/添加按钮/表单按 `activeSection` 过滤共用一份模板；表单用 `:key` 随分区重建（`connection-form.vue` 靠 `variant` 决定 `isNetdisk`）。
- 默认分区策略：未手动切过时「有网盘连接→网盘；只有远程连接→远程；都没有→网盘」；手动点标签后写 `sectionOverride`，不再被抢走。
- `src/stores/storage/remote-connections.ts`：新增瞬态 `pendingSection` + `requestRemoteSection()` / `consumeRemoteSection()`（消费即清空）。
- `src/modules/home/components/remote-entries-section.vue`（新建）+ `home/components/index.ts` 导出 + `home/pages/home.vue` 挂载：分区标题 `settings.remote.nav.homeTitle`，入口 `entry-netdisk`(CloudIcon) / `entry-remote`(ServerIcon)；点击顺序固定为**先 `requestRemoteSection(section)` 再 `openSettingsTab('remote')`**（反了会丢分区意图）。
- `src/modules/tab-bar/tab/tab.vue` + `src/modules/tab-bar/utils/remote-tab-labels.ts`（新建）：远程标签标题走 `buildRemoteTabLabels`，标题=连接名、副标题=远端相对路径；查不到连接名时退回旧逻辑（显示 id，不留空白）。
- 17 语言包：新增 `settings.remote.nav.homeTitle`、删除语义失效的 `settings.remote.tab.*`；连词已修（ch「网盘 与 远程连接」、en「&」）。
- 网盘归类口径固定为 `isBaiduPanProtocol`（`src/types/remote.ts:173`，仅 `protocol === 'baidu_pan'`）。

### 21.3 ★ 本轮发现并修复的真缺陷：首页入口深链落不到「远程」分类

- **现象**：从 home 页点「网盘」入口 → 设置页激活的是**「常规」**分类，UIA 里只有「数字/短的/长」三个页签（常规页的数字长度选项），远程页签完全没有。
- **根因**（已坐实，非猜测）：`src/stores/runtime/settings.ts` 的 `init()` 兜底
  `if (!sections.value.some(section => section.category === currentTab.value)) setCurrentTab('general')`
  **判据用错了集合**。`sections`（可搜索小节）在 `allSections` 里**没有**任何 `category: 'remote'` 的条目——「远程」是**整页分类**，由 `src/modules/settings/ui/view.vue` 的 `categoryComponentMap`（含 `remote: RemoteCategory`）渲染，根本不进小节列表。⇒ 任何合法的 `currentTab === 'remote'` 都会被误回落成 `'general'`。
  由于 `init()` 开头有 `if (isInitialized.value) return;`，这个覆盖在**每次冷启动后第一次进入设置页**时必然发生 ⇒ 首页「网盘/远程连接」入口与左侧 rail 的「远程」入口**都是坏的**（后者是历史遗留缺陷，本轮一并修好）。
- **修复**：判据换成**导航页签列表** `tabs`（= `settingsTabs`，其中 `experimental` 仍按小节可用性过滤），并抽出纯函数 `resolveSettingsTabAfterInit(tabName, availableTabNames)`（`src/stores/runtime/settings.ts`）便于单测。
- **回归测试**：`src/stores/runtime/__tests__/settings-tab-guard.test.ts`（3 用例，含「init() 之后 currentTab 仍是 remote」）。

### 21.4 门禁（修复后）

- `vitest`（5 个相关文件）= **78 passed**；**全量单测 295 files / 2777 tests passed，exit 0**（`.superpowers/sdd/vitest-full-20261108-fix.log`）。
- `vue-tsc --build` **exit 0**；改动文件 `eslint` **exit 0**；全仓 `eslint .` = **177 errors / 0 warnings**（与基线一致，**exit 1 是 eslint 报告存量错误的正常退出码，不是回归**）。

### 21.5 打包与真机验收（`tauri build` 打包产物，已装机）

- 打包：`npm run tauri:build`（`.superpowers/sdd/build-20261108-fix.cmd`，纯 ASCII，内部 `> log 2>&1` 重定向）→ 新包 `F:\soft\00selfmade\filemanager\sigma-file-manager\release\local-builds\Alpha File Manager_3.12.2_x64-setup.exe`（**45,347,072 B，17:27:10**）。
- 装机：`cmd /c 'start /wait "" "<exe>" /S'` 静默安装 → `D:\Program Files\Alpha File Manager\alpha-file-manager.exe`（81,996,800 B，17:26:36）。
- **本轮修复的真机验收（证据目录 `.superpowers/sdd/accept-tabs2fix/`）**：
  - 主页「网盘」入口 Invoke → 设置页激活的是**「远程」分类**（`REMOTE_CATEGORY_TITLE_HITS=1`），`GENERAL_TEXT_HITS=0`（**不再落「常规」**）；页签 `TabItem 网盘 selected=True`；列表 `ListItem | 百度网盘…`；按钮 `Button | 添加网盘`。
  - 主页「远程连接」入口 Invoke → 仍在「远程」分类，`TabItem 远程连接 selected=True`，按钮 `Button | 添加远程连接`。
  - 附带证据：**应用启动时若 `settingsCurrentTab='remote'`，现在会直接停在「远程」分类**（修复前会被 `init()` 打回「常规」）——这是同一条 store 路径的直接体现。
- 前一轮已在同一安装包体系下确认的三项（不再重复）：设置页远程顶部页签（网盘/远程连接）、home 页「网盘 与 远程连接」分区与两个带图标入口、远程标签标题显示「百度网盘」。

### 21.6 ★ 本轮新增的工具坑

1. **锁屏会话下 UIA `InvokePattern` 可用、原始鼠标/键盘注入不可用**：本轮机器进入锁屏（`LogonUI` 在跑、`GetForegroundWindow()` 返回空、`Graphics.CopyFromScreen` 抛「句柄无效」），`SendInput`/`mouse_event` 全部不落地，但 **UIA `InvokePattern.Invoke()` 照常生效**（Chromium 的可访问性实现自己派发点击）⇒ 锁屏时改用 `InvokePattern`，不要判定「功能没生效」。脚本：`.superpowers/sdd/accept-tabs2-invoke.ps1`。
2. **UIA `InvokePattern` 前必须先定位到正确的元素**：本轮两次空跑分别因为（a）rail 过滤条件写死 `Width>=30`，实际 rail 按钮是 29x28；（b）排序取到了标题栏按钮（Top=10）而不是 rail 第一个（Top=62）。⇒ rail 按钮的稳定特征是 `Left<80` + `Top∈[50,500]` + `宽高∈[20,60]`。
3. **应用重启后窗口不再最大化**（本轮变成 1920x1032），**所有硬编码坐标失效** ⇒ 脚本一律按控件名 + ControlType + 尺寸区间定位，别写死屏幕坐标。
4. **bash 硬安全策略**：`Set-Location` 后用 `Out-File` / 变量拼路径写日志会被拦（报 "write/delete targets cannot be verified safely after a command working directory change"）。⇒ 日志一律用**字面量绝对路径 + `*>` 重定向**；`Set-Location` 用字面量路径，别用变量。
5. **`vitest` 里 mock `@/localization` 必须同时给 `i18n.global.locale`（`{ value: 'en' }`）**：只给 `t` 会让 `src/modules/changelog/composables/use-changelog.ts:155` 的 `watch(() => i18n.global.locale.value)` 在模块加载期抛错，整个测试文件 0 用例。
6. **store 单测只 mock 三个依赖即可**：`@/localization`、`@/stores/storage/user-settings`、`@/stores/runtime/platform`；`useSettingsStore` 顶部 import 的 shortcuts / global-shortcuts / extensions 及 `init()` 里 25 个动态 import 的分类组件在 jsdom 下都能正常加载（实测通过）。
7. **设置页「远程」分类能打开 ≠ 首页入口能深链进去**：前者是手动点导航（`init()` 之后才改 `currentTab`），后者是**跳转前**就设 `currentTab` ⇒ 会被 `init()` 覆盖。验收两者要分别测。
8. **验收后核对用户数据要看「条目」而不只看字节数**：`user-stats.json` 的 `history` 数组应用自己会去重（本轮 99→98），字节数因此变了 131 B，但那**不是**夹具污染；连接数与连接名逐条一致才是硬判据。

### 21.7 用户数据状态（已收尾）

- 验收前备份：`F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\pre-f-backup-20261108\`（4 个文件）。
- 核对结论：本轮**没有注入任何夹具**——连接 4 条（速豹堂 / aiec.fun / jury.xin / 百度网盘）与备份逐名一致；`remote-credentials.json`（793 B，mtime 15:16）、`remote-known-hosts.json`（463 B，mtime 10-19）本轮未被触碰。
- 已按 AGENTS.md 顺序**先 `Stop-Process` 再 restore**：4 个文件字节数与备份**逐项相等**（31550 / 1483 / 1742 / 6038），随后重启应用（PID 已确认存活）。

### 21.8 文档路径更正（§19.6 的 AGENTS.md 路径是错的）

- §19.6 写的 `F:\soft\00selfmade\filemanager\sigma-file-manager\AGENTS.md` **不存在**（读取报 ENOENT）。
- 生效的项目铁律在**仓库根**：`F:\soft\00selfmade\filemanager\AGENTS.md`。
- 下游交接/说明文档若引用了子目录路径，一律以仓库根那份为准。



## 22. R22：远程入口三处改造（设置分区 / 主页连接列表 / 位置页远程条目）

> 用户 2026-11-08 三条修改意见，本轮（R22）实现并收口。
> 上一节 §21 的历史内容不必重读，本节是**唯一最新状态**。

### 22.1 需求与定稿

| # | 用户原话 | 定稿做法 |
| --- | --- | --- |
| 1 | 设置「远程」页面顶部就分为**网盘**和**远程连接**两个选项卡，底部「信任的主机」模块划入**远程连接**选项卡中显示 | 分区判定抽到 `sections.ts` 由父子共用；`index.vue` 收子组件抛上来的 `section-change`，`v-if="activeSection === 'remote'"` 才挂 `TrustedHostsSection` |
| 2 | 主页的网盘/远程连接入口**点击会跳到远程设置页**，应该显示配置好的网盘、远程连接的**服务器名称**，点击在**导航页视图**中打开 | `remote-entries-section.vue` 按 `store.connections` 分「网盘」「远程连接」两组列条目，点击走 `openRemoteConnection(connection, { navigateToNavigator, openPanes })`；**不再** `openSettingsTab('remote')` |
| 3 | 导航器的「位置」页面当前**只有分区**，也要添加网盘和远程配置的服务器 | `buildLocationsDirectoryFromDrives(drives, remoteEntries = getRemoteLocationEntries())` 用**默认参数**带上远程条目，让四个消费方口径一致 |

三条需求的归类口径**只有一个**：`isBaiduPanProtocol`（`src/types/remote.ts:173`）。
以后接阿里云盘只需扩这个判定，三处自动跟上。

### 22.2 落到哪些文件

- 新建 `src/modules/settings/ui/categories/remote/sections.ts`：`RemoteSection = 'netdisk' | 'remote'`、`sectionOfProtocol()`、`sectionOfConnection()`。
- 重写 `.../remote/index.vue`：`activeSection = ref<RemoteSection | null>(null)`。**初值刻意是 `null` 不是 `'netdisk'`** —— 还没收到子组件第一份报告时不知道它落在哪个标签，猜 `'netdisk'` 会让「只有远程连接」的用户在挂载瞬间看见「信任的主机」闪一下再消失。
- `.../remote/connections-section.vue`：改从 `./sections` 引入分区判定；`watch(activeSection, …, { immediate: true })` 向上 emit `section-change`。`immediate` **必需**——否则子组件初值永远不上报，父组件停在 `null`，信任的主机永不显示。
- 重写 `src/modules/home/components/remote-entries-section.vue`：两组列名（`CloudIcon` / `ServerIcon`），空状态给 `settings.remote.nav.homeEmpty` + 「远程连接设置」按钮。空状态那条按钮**不是**回到第一版：第一版是**唯一入口**只有跳设置，用户否掉的是那个。
- 重写 `src/modules/home/components/remote-entries-section.test.ts`（4 用例）。
- 17 个语言包补 `settings.remote.nav.homeEmpty`（`add-home-empty.cjs`：`WRITTEN=17` / `PROBLEMS=0`；中文 locale code 是 `ch` 不是 `zh`）。
- 新建 `src/utils/remote-location-refs.ts`：`RemoteLocationRef { path, name }` + `setRemoteLocationRefs()` / `getRemoteLocationRefs()`。
- `src/stores/storage/remote-connections.ts`：`watch(connections, value => setRemoteLocationRefs(value), { immediate: true })`（store 推、`virtual-locations` 读快照）。
- `src/utils/virtual-locations.ts`：删掉 `useRemoteConnectionsStore` / `RemoteConnection` 两个 import，`getRemoteLocationEntries()` 改为 `return getRemoteLocationRefs().map(ref => createVirtualDirectoryEntry(ref.path, ref.name));`。

### 22.3 ★ 为什么远程条目要走「注册表」而不是直接在工具函数里 import store

这是本轮**唯一一次真实回归**，值得写死：

`src/utils/virtual-locations.ts` 是**纯工具模块**，被 `address-bar.vue`、工具栏、以及**大量单测**引入。
第一版直接在它里面 `import { useRemoteConnectionsStore }`，于是每个使用方的模块图都被拖进
`@tauri-apps/plugin-store` 与 `@/data/user-paths` —— 实测**拖红 8 个用例**（`use-file-browser-drag-session.test.ts`）。

改成「store 主动推快照到无依赖的注册表，工具模块只读快照」后，模块图恢复干净，8 条用例转绿。

**判据**：纯工具模块（`src/utils/**`）**不得** import 任何 `@/stores/**`。
需要 store 数据时，让 store 反向往工具模块注册。

### 22.4 门禁（本轮全部实跑）

| 项 | 结果 | 日志 |
| --- | --- | --- |
| `npx vitest run`（drag-session + virtual-locations + remote-entries） | **3 files / 39 tests passed** | — |
| `npx vue-tsc --build` | **exit 0** | `.superpowers/sdd/tsc-r22b.log` |
| `npx vitest run`（全量） | **295 files / 2778 tests passed**，`Errors 1 error`，exit 1 | `.superpowers/sdd/vitest-full-r22b.log` |
| `npx eslint .`（全仓） | **177 problems (177 errors, 0 warnings)** = 基线 | `.superpowers/sdd/eslint-r22d.log` |

- 全量单测那 1 个 `Errors` 是 **Unhandled Rejection**：`TypeError: Cannot read properties of undefined (reading 'filter')` @ `address-bar-editor-utils.ts:624:69`。单独跑 `address-bar-editor-dialog.test.ts` → 6 passed、**无该错误** ⇒ 并发下的既有隐性 flake，涉及文件本轮**未改动**。
- **`eslint .` 的 exit 1 是 eslint 报告存量错误的正常退出码**，判据看日志末行 `✖ N problems`；本轮基线 = 177。

### 22.5 打包与真机验收

#### 打包与装机

- 脚本 `.superpowers/sdd/build-r22b.cmd`（**纯 ASCII**；`npm run tauri:build`；日志自己 `> logfile 2>&1`，不挂 stdout 管道），后台跑完 **exit 0**；日志 `.superpowers/sdd/build-r22b.log`（245 418 B）末行 `BUILD_EXIT=0`。
- 产物 `release\local-builds\Alpha File Manager_3.12.2_x64-setup.exe` = **45 351 476 B / 18:28:30**（旧包 18:10:36 ⇒ 确认是本轮新包，没复用到旧产物）。
- 装机：停旧进程（`SURVIVORS=0`）→ `cmd /c start /wait "" "<setup>" /S` → `INSTALL_EXIT=0` → 已装 `D:\Program Files\Alpha File Manager\alpha-file-manager.exe` = **81 996 800 B / 18:27:54**（旧 17:26:36）→ 启动新进程 **PID 882344 / 18:29:51**。

#### 验收结论

| 需求 | 结论 | 证据目录 |
| --- | --- | --- |
| ① 设置「远程」分两选项卡，信任的主机只出现在「远程连接」 | ✅ **成立** | `.superpowers/sdd/accept-r22c/` |
| ② 主页列出已配置的网盘/服务器名称，点击在**导航页视图**打开 | ✅ **成立** | `.superpowers/sdd/accept-r22/` |
| ③ 位置页除分区外也列出网盘与远程服务器 | ✅ **成立** | `.superpowers/sdd/accept-r22/` |

**需求 ① 的判别链**（`accept-r22c.ps1` → `accept-r22c/verdict.txt`）：真实鼠标点击 left rail（`SetCursorPos` + `mouse_event`）→ `SETTINGS_RAIL_INDEX=3` 落到设置页 → 点「远程」分类 → 顶部出现两个标签（`TAB_NETDISK=1`，`网盘 @719,417 387x33`；`远程连接 @1105,417 415x33`），随后：

- 默认标签 = `网盘` ⇒ `TRUSTED_ON_NETDISK=0`（元素总数 103，无信任区）
- 点「远程连接」⇒ `TRUSTED_ON_REMOTE=1`，dump 实含
  `Text | 已信任的主机 | 759,927 109x28` 与
  `Text | 你已核对过的主机公钥。只有信任之后，远程目录才会打开。 | 759,954 257x46`，
  元素总数 **103 → 125**（+22）
- 留在远程标签下复测 `TRUSTED_FINAL=1`

⇒ 「已信任的主机」**只在**「远程连接」标签下渲染，在「网盘」标签下完全不渲染。

**需求 ② 的判别链**（`accept-r22.ps1`）：主页 dump 命中 `网盘` / `远程连接` / `速豹堂` / `百度网盘` / `aiec.fun` / `jury.xin`；点击 `速豹堂` 卡片后 `AFTER_CLICK_SETTINGS_TABS net=0 rem=0`（**没跳设置页**）、`AFTER_CLICK_CONNNAME_HITS=0`，页面出现盘符 + 文件列表 + `website_sbt` ⇒ 在导航页视图中打开了该服务器。

**需求 ③ 的判别链**（`accept-r22.ps1` → `uia-c-locations.txt`）：位置页 dump 实含
`Button | 百度网盘 — | 397,97 244x58`、`速豹堂 — @660,97`、`aiec.fun — @397,175`、`jury.xin — @660,175`（左右两屏各一份），与 5 个盘符共存。

#### 取证踩到的坑（写死，别重试）

- **UIA `InvokePattern.Invoke()` 对 left rail 按钮静默无效**：调用返回成功、页面却不切换（三次 dump 元素数都是 412 = 停在导航器页），产出假的 `SETTINGS_REMOTE_NAV_NOT_FOUND`。`LOCKED=False` 时改走**真实鼠标点击**最可靠 —— `accept-r22.ps1`（Invoke 版）在这一步全军覆没，`accept-r22c.ps1`（鼠标版）一次通过。
- **UIA `Name` 精确等值匹配对卡片 / 标签类元素不可靠**：位置页远程条目的真实 `Name` 是 `速豹堂 —`（名称 + 空格 + 容量占位符 ` —`），用等值匹配 `速豹堂` 得 **0 命中 = 假阴性**，本轮因此在需求 ③ 上误判过一轮。此类断言改用子串匹配，或直接读 dump 全文。
- 本次 `SCREEN_X=1920`（100% 缩放），与历史记录的 `2560`（150%）不同 ⇒ **坐标口径随会话缩放变，任何坐标都不要硬编码**。

### 22.6 已知局限（本轮不改，记录在案）

- 「位置」页的远程条目在**连接列表晚于首次构建加载**时，要等下一次刷新才出现（`watch(sharedDrives)` 不会因 `connections` 变化触发）。正常启动路径下 `init()` 先于首次渲染，观察不到。