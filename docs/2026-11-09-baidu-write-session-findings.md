# 2026-11-09 百度网盘写操作「会话已失效」归因报告

> 证据目录：`.superpowers/sdd/acc-20261109/`（`baidu-token-source.txt`、`baidu-session-probe.txt`、`baidu-bdstoken-probe.txt`）。
> 探针：`.superpowers/sdd/baidu-session-probe.ps1`、`baidu-bdstoken-probe.ps1`、`baidu-token-source-probe.ps1`（全部只读，不打印 cookie/token 明文）。

## 1. 用户报的现场

连接百度网盘后删除 `sfm://remote/6c1d7da6e8e84df6834c19ef3fcb64e1/0000temp/孙宇晨《我的女友景甜》.pdf` 报错：

```text
删除失败:  删除失败: sfm://…/孙宇晨《我的女友景甜》.pdf:百度网盘会话已失效，请重新授权。百度网盘会话已失效，请重新授权。
```

## 2. 两条独立缺陷

### D1（代码缺陷，本轮已修）：写操作的 bdstoken 来源已死

- 旧实现 `fetch_bdstoken` 抓 `https://pan.baidu.com/disk/home` 的 HTML 再正则取 token。
- 实测（带形态正确的 cookie）：`/disk/home` **恒定 302** → `/disk/main?from=homeFlow`（旧首页产品级下线，重定向目标**不是登录页**）。
- 本模块 client 用 `redirect::Policy::none()`，3xx 被 `failure_from_http_status` 归成 `BaiduFailure::Auth` ⇒ **会话明明有效也报「会话已失效」**；`should_refresh_token` 又认为 Auth 值得重试 ⇒ 同一条死路走两遍。
- 顺重定向拿 `/disk/main` 也没用：页面里 `"bdstoken":""` 是空串、`uk:0`、`username:""`，`yundata/yunData/initContext/baiduInfo/hasLogin` 命中数全为 0 ⇒ **抓 HTML 取 token 整条路作废**。
- 读链路不受影响的解释已证伪「读写用不同 cookie」：读写共用同一份 `cookie_header()`。读能用是因为读走 `rest/2.0/xpan/file` 系接口，不经 `/disk/home`。

**修复（已落盘）**：`fetch_bdstoken` 改打 JSON 接口 `api/gettemplatevariable`（`bdstoken_url()` + `parse_bdstoken_response()`），并加护栏：**token 接口收到 3xx 一律归 Protocol**（防止「幻影会话失效」借道复活）；`errno=-6` 仍归 Auth（保住批次 B 的内嵌重新登录触发链）；空串/`null`/缺 `result` 归 Protocol。`baidu_provider.rs` 的 `refresh_bdstoken` / `should_refresh_token` 签名不变、零改动。

**未坐实项**：`gettemplatevariable` 在**有效会话**下返回 hex32 尚未观测到（本轮只拿到 `errno=-6`）。失败信号明确、形态与社区实现一致，属「推断可用」。真机重授权后一测即知。

### D2（凭据缺陷，需用户重新授权）：保险库里的会话确实失效

三口互证同一份 bduss/stoken 被服务端拒：

| 探针 | 结果 |
|---|---|
| 只读 `rest/2.0/xpan/file?method=list` | HTTP 200，`errno=-6` |
| `api/gettemplatevariable` | HTTP 200，`errno=-6` |
| `/disk/main` 页面 | `uk:0`、`username:""`、`bdstoken:""` |

⇒ 即使 D1 修好，这份凭据也做不了任何需要登录态的事。**用户必须在新包里亲自重新授权百度网盘**。

## 3. 「界面还能列目录」与「只读接口已 errno=-6」的矛盾（本轮已解释）

界面当时仍列出 `/0000temp` 的 2 个文件，而同一 cookie 打只读 list 已 `errno=-6`。

本轮排查（`grep` 实测）：`src-tauri/src/remote/` 下带 cache 的只有两处，且都不是目录缓存 —— `baidu_provider.rs` 的 `CachedBdstoken`（只缓存 bdstoken，TTL 见 `BAIDU_BDSTOKEN_TTL`）与 `transfer.rs` 的传输态缓存；`cached_entries|dir_cache|listings_cache|listing_cache|entries_cache` 在 `src-tauri/src/**.rs` 全仓 **0 命中**。

⇒ 后端不存在「远端目录列表缓存」，所以界面上那 2 个文件不是新拉的，而是**该标签页此前会话有效时已加载、尚未刷新的内存态**（推断，未逐行验证前端 store）。

**结论不变且更重要**：不能拿「能列目录」当「会话有效」的证据 —— 会话是否有效只能以服务端应答（`errno`）为准。

## 4. 「内嵌登录把失效会话判成已登录」——本轮已证伪

原疑点：若 `baidu_login.rs` 的成功判据仍依赖 `/disk/home` 重定向启发式，带陈旧 BDUSS 时该 URL 不再跳登录页，可能把失效会话判成「已登录」并保存，从而解释「授权看似成功、凭据实际被拒」。

实测（grep `src-tauri/src/baidu_login.rs`）：**登录判据完全不碰那条启发式**。`LOGIN_START_URL` 是 `https://pan.baidu.com/disk/main`（不是 `/disk/home`），判定链是纯 cookie jar 口径：

`cookies_for_url()`（抓 WebView2 的 jar）→ `CookieRead::Ok` → `scan_cookies()` 必须返回 `BaiduCookieOutcome::Ready`，否则继续轮询 → STOKEN 缺席时有 `STOKEN_WAIT = 20 s` 宽限 → `store_pending` + `settle` + `emit_event("ready")` + 关窗；落盘失败时 `claim` 把 pending 放回、不谎报成功。

⇒ 「幻影已登录」不成立。残余风险只剩一条边缘情形：WebView2 罐里残留旧 BDUSS 且站点未强制重登时，抓到的确实是那份旧凭据 —— 判据本身没错，但**「能抓到 BDUSS+STOKEN」不等于「服务端接受它」**。D2 观测到的正是这种被拒形态，因此用户重新授权时若站点直接给了旧会话，仍可能重演；真机验收要按 §6 用服务端应答确认。

## 5. 报错文案双层重复（次要，静态排查已穷尽，需真机原文）

用户贴的形态是「删除失败: 」「`sfm://…`」「同一句话两遍」。本轮把三条候选假设**逐一实测排除**：

1. **前端不拼接**：`describeRemoteError`（`src/utils/remote-directory.ts:99-109`）只返回
   `caught.message` / `Error.message` / `String(caught)` 三者之一，单层，不会把同一句复制两遍。
2. **不是两条 toast 各写一次**：`remote.delete.failed` 全仓**只有一处**调用
   （`use-file-browser-selection.ts:1722`），toast 结构是 `title = 删除失败` +
   `description = failures.map(f => \`${f.path}: ${f.message}\`).join('\n')` ⇒ 只能解释用户抄到的
   **一层**「删除失败」和一条 `路径: 消息`。
3. **Rust 侧 remote 模块不产这个前缀**：`删除失败` 只出现在 `delete_tree.rs` 的注释与
   `file_lock_commands.rs:72`（全角冒号「彻底删除失败：」，属本地删除路径）；
   `baidu_provider.rs:2102` 那句短文案「会话已失效，请重新授权。」位于
   `#[tokio::test] passes_through_an_auth_failure_during_a_write` 的**测试夹具**里，不参与运行时。

⚠️ **修正本文早前一笔误记**：`describe_failure()` 在 `baidu_api.rs` 里 **0 命中**（当前源码没有这个函数）。
Auth 分支的 `server_message` 取自**服务端 JSON 的 `msg` 字段**（如「需要验证验证码，或者账号异常」），而
`BaiduWriteError::to_remote_error`（`baidu_api.rs:1293-1303`）的追加形态是
`{base.message}（服务端提示：{server_message}）` —— **带括号**，与用户抄录的形态不一致，
所以「服务端提示拼接导致重复」这条假设也不成立（除非用户抄录时漏了括号）。

⇒ 结论：**静态代码里无法复现这个重复，必须拿真机 toast 的屏幕实拍原文**（BitBlt 抓屏，`PrintWindow` 不渲染 toast 浮层）。
不阻塞主修复；且 D1 修复后 `Auth` 的触发条件本身变了（token 能从 JSON 接口取到），文案形态可能随之改变。

## 6. 验收判据（真机）

1. 重新打包安装后，在设置页对百度网盘**重新授权**（内嵌登录，用户本人输密码）。
2. 列目录 → 新建文件夹 → 改名 → 复制/粘贴 → 删除（用探针自建自删的临时文件，**不得删用户那个 316 MB PDF**）。
3. 全程不应出现「会话已失效」；若出现，看是否带「（服务端提示：…）」以区分 D1 类协议错误与真失效。
4. 装包前核对新 bundle 的 exe **大小 + mtime**（本轮源码改动未提交，装机版本号仍是 3.13.0 ⇒ 与已装版本同号，必须用文件大小/mtime 证明组件真被替换，不能只看安装器退出码）。

## 7. 本轮落盘与门禁状态

- 代码改动全部在 `src-tauri/src/remote/baidu_api.rs`（新增 `bdstoken_url()` / `BaiduTokenResult` / `BaiduTokenEnvelope` / `parse_bdstoken_response()`，重写 `fetch_bdstoken`，新增常量 `BAIDU_BDSTOKEN_ENDPOINT`；保留 `parse_bdstoken` / `take_bdstoken_value` 与其旧测试 ⇒ 向后兼容）。插入约 230 行 ⇒ **该文件基线行号全部失效，定位一律 grep**。
- 单元测试：`cargo test --lib baidu_api` → **96 passed / 0 failed / 0 ignored**（1065 filtered out），`.superpowers/sdd/cargo-baidu-test.log`（`EXITCODE=0`）。含 7 条针对新 token 来源的断言（URL 不含 `disk/home`、`disk/main`；`errno=0` hex32 → Ok；`errno=-6` → Auth；空串/`null`/缺 `result` → Protocol；非 JSON → Malformed）。
- 打包：**已完成**。`.superpowers/sdd/build-20261109-bdwrite.cmd` → 日志 `.superpowers/sdd/build-20261109-bdwrite.log`（246038 B / mtime 11:58:51；`rustc=0 cargo=0`、`^error` 0 命中；关键行 `Finished \`release\` profile [optimized] target(s) in 3m 03s` → `Running makensis` → `Finished 1 bundle at: …bundle\nsis\Alpha File Manager_3.13.0_x64-setup.exe (43.31 MiB)` → `[publish-bundle] 已发布到 release/`）。⚠️ 日志里**没有** `DONE_EXIT=` 行（`.cmd` 收尾 echo 未落盘），成功判据用 `[publish-bundle] 已发布到 release/`。
- **交付包**：`release/local-builds/Alpha File Manager_3.13.0_x64-setup.exe`，**45,411,138 B / 2026-11-09 11:58:47**（`src-tauri/target/release/bundle/nsis/` 里同号同尺寸）。因 `release/` 根目录已有**同号但内容不同**的旧包（10:03:52 / 45,320,456 B），`publish-bundle-to-release.mjs` 逐字拒绝覆盖、改写入 `release/local-builds/` ⇒ 比旧包多 90,682 B，确属新产物。
- 遗留待办：① 用户装新包 + 真机重新授权 + §6 全流程验收；② §5 文案双层重复未定位（需真机 toast 抓屏原文）；③ `baidu_provider.rs` 里两条注释（`:203`、`:216`）仍写「重拉一次首页」，已过时（本轮未改，避免与构建抢同一份源码），下轮随注释级 PR 清掉；④ 用户验收口径「复制/粘贴/剪切/同步」的实际支持面未逐项确认；⑤ 新 token 来源 `api/gettemplatevariable` 在**有效会话**下返回 hex32 仍属推断（本轮只观测到失效会话的 `errno=-6`），真机重授权后即知；⑥ 本轮代码与文档改动**均未提交**（`main` = `5c6e21e3`，ahead origin 2）；⑦ 装机位置 exe 路径待定位——本轮实测 `C:\Program Files\Alpha File Manager\alpha-file-manager.exe` **不存在**（输出 `no exe`），§6 第 4 条的「大小 + mtime」核对要先找到真实安装目录。
