# 批次 29 交接：阿里云盘只读接入（应用内 webview 扫码登录）

- 日期：2026-11-20
- 分支：`main`
- 版本：3.15.0（`6a60eb26`）→ 本批次随 **3.16.0** 一同发布
- 状态：代码、门禁、真机验收全部完成；本地 commit 已落地，**push 需单独确认**

---

## 1. 目标与红线

把**阿里云盘**接进网盘连接列表，走通三条链路：

1. **扫码登录** —— 登录窗口在应用内弹出，手机扫码即完成授权。
2. **列主盘根目录** —— 显示真实文件名。
3. **读一个文件** —— 拿到真实字节数与哈希。

**只读红线（本批次不可越过）**

- 只做「扫码登录 / 列目录 / 读文件」。
- **不实现、不调用**上传、删除、改名、分享、移动等写接口。
- 保险箱盘（vault）只识别，不解锁。

**凭据纪律**

- `accessToken` / `refreshToken` / `bizExt` / cookie / 签名直链的**明文**一律不进日志、事件 payload、命令返回值与文件。
- 对外只报**长度** + `short_sha16`。

---

## 2. 根因与修复（本批次最有价值的一条）

### 现象

扫码登录成功、连接能保存，但**列目录必挂**：等 20 秒左右后报「与阿里云盘的连接中断」。

### 根因

阿里云盘前面的网关（istio-envoy）对**不声明 `Accept-Encoding: gzip`** 的请求：

1. 只回**响应头**；
2. 把**响应体挂住 20.1~20.4 秒**；
3. 然后**掐断连接**。

reqwest 未开 `gzip` feature ⇒ 收到的响应体是坏的 ⇒ 报 `Kind::Decode` ⇒ `classify_transport()` 把它误映射成 `RemoteErrorKind::Disconnected` ⇒ 文案显示成「与阿里云盘的连接中断」。

### 判别性 A/B 证据（决定性）

`.superpowers/sdd/b29-diag-05.txt`，curl + .NET 双通道：

| 通道 | 是否带 `Accept-Encoding: gzip` | 结果 |
|---|---|---|
| curl | 否 | `exit=18` / **20.1s** / **0 字节** |
| curl | 是 | `exit=0` / **0.18~0.19s** / 2579~2580 字节 gzip |
| .NET | 否 | 20.1s 后 `Error while copying content to a stream.` |
| .NET | 是 | 0.16s / 2578 字节 |

已逐项排除：proxy、WinHTTP Direct、IE `ProxyEnable=0`、防火墙、凭据、端点契约、HTTP/2。

### 修复

- `src-tauri/Cargo.toml`：给 reqwest 加 `gzip` feature（附 5 行中文注释说明原因）。
- `src-tauri/Cargo.lock`：随之新增 `async-compression 0.4.52` / `compression-core 0.4.34` / `compression-codecs 0.4.47`
  —— **必须与 `Cargo.toml` 同批提交**。
- `src-tauri/src/remote/aliyun_provider.rs`：新增判别性单测 `the_http_client_negotiates_gzip_with_aliyun`
  —— 本地起一个恒回 gzip 的 TCP 服务，断言请求头含 `accept-encoding` 且响应体是明文。

**为什么不用手写 gunzip、也不只给阿里云盘手加头**：网关要求的是**协商**，不是**特例**。crate 级 `gzip` feature 才是正解。

---

## 3. 本批次代码变更（41 项）

### 后端（`src-tauri/`）

- `Cargo.toml` / `Cargo.lock`：reqwest `gzip` feature 及其依赖。
- `build.rs`：新增构建期配置。
- `capabilities/default.json` + `capabilities/aliyun-login.json`：webview 登录窗口所需能力。
- `permissions/app-commands.toml`、`permissions/default.toml`：新增权限定义目录。
- `src/aliyun_login.rs`：应用内登录窗口（webview）流程。
- `src/remote/aliyun_qr.rs`（新增）：二维码生成与刷新。
- `src/remote/aliyun_api.rs`：列目录 / 取元数据 / 取下载地址。
- `src/remote/aliyun_auth.rs`：QR bizExt 换取并托管会话。
- `src/remote/aliyun_provider.rs`：provider 实现 + gzip 判别性单测。
- `src/remote/{commands,connection_test,mod}.rs`：命令注册与连接测试。
- `src/lib.rs`：注册命令与插件。

### 前端（`src/`）

- `modules/settings/ui/categories/remote/connection-form.vue`：网盘连接表单接阿里云盘。
- `modules/settings/ui/categories/remote/aliyun-connection-form.test.ts`（新增）。
- `modules/remote/utils/__tests__/aliyun-command-registration.test.ts`。
- `stores/storage/remote-connections.ts` + 对应测试。
- `types/remote.ts` + 对应测试。
- `localization/messages/*.json`（17 个语言包）+ `localization/__tests__/aliyun-i18n-keys.test.ts`。
- `docs/2026-11-19-aliyun-open-platform-application-guide.md`（新增）。

### 端点与参数（踩过的坑，写下来免得再踩）

- 列表体带 `all=false` 会**静默少一项**。
- `*_process` 必须是**字符串**（传对象 → 400）。
- 元数据端点用 `/adrive/v2/file/get`（**v3 不存在**）。
- 下载端点 `/adrive/v2/file/get_download_url`，回源要带 `Referer: https://www.alipan.com/`。
- **OAuth code 路线（`/token/get`）不可用**；只有 QR bizExt 换出的 token 能直接当 Bearer 用。

---

## 4. 验收结果（三链全绿）

`ACCEPT_RESULT=PASS`，汇总日志：`.superpowers/sdd/b29-accept-gzip-run.log`。

| 链路 | 判据 | 实测 |
|---|---|---|
| 一 · 扫码登录 | `new → scanned → confirmed` | bizExt len **2852** / sha16 `b0bbea660c94b0e9`；`credential stored` → `emit kind=ready`；DPAPI 落盘，重启后授权仍在 |
| 二 · 列根目录 | 真实中文文件名 | **14 项**：软件、16设计、视频备份、00电商运营、13读书栏目、12创意 写作、11家庭生活教育、08艺术、06心理学、03管理营销、02经济学、09历史、手机备份、Midjourney表情包提示词生成器.xlsm；DOM↔API **零差异**（`only_in_dom: []` / `only_in_backend: []`）；`read_dir` 169ms~1.2s |
| 三 · 读文件 | 字节数 + 哈希 | `Midjourney表情包提示词生成器.xlsm`（file_id `6594a4a7716d54828fb84e179575896fa82c7c29`）→ **94512 字节**，sha256 `62C1289FFE15A117A1C1ED74C0B67F678592950423A487E3E9675D58DE9F459C`，首字节 `504b0304`，与后端声明 size 一致 |

像素双证：`.superpowers/sdd/b29-main-window.png`（3840×2088，`PNG_UNIFORM=False`），Default 桌面（`IS_DEFAULT_DESKTOP=True`）。

### 门禁

| 门禁 | 结果 | 证据 |
|---|---|---|
| `cargo test --lib` | ✅ 1554 passed / 0 failed / 1 ignored | `b29-gate-cargo-gzip.log` |
| `ts`（vue-tsc --build） | ✅ EXIT=0 | `b29-gate-ts.log` |
| `lint:check` | ✅ EXIT=0 | `b29-gate-lint2.log` |
| `test:unit:run`（vitest） | ✅ EXIT=0，322 files / 3465 tests | `b29-gate-vitest2.log` |
| `verify-changelog` / `verify:sandbox` | ✅ EXIT=0 | `b29-gate-changelog.log` / `b29-gate-sandbox.log` |
| `npm run check`（全量） | 见收口台账 | `b29-gate-check.log` |

### 收尾

- CDP 旗标已回滚：`src-tauri/tauri.conf.json` 的 `additionalBrowserArgs` 已删，`git diff` 无差异。
- 正式 exe 已**无旗标重建**：`target/release/alpha-file-manager.exe`（84879360 B / 17:21:10），`Select-String remote-debugging-port` 命中 False。
- 正式 exe 启动冒烟通过（主窗标题「Alpha File Manager」，stderr 空），进程已清理。

---

## 5. 未验证项（如实登记，不算 PASS）

1. 二维码尺寸 180×180 与刷新按钮**未目视确认**。
2. token / refresh 的**真实续期**未坐实（只坐实了登录后授权仍在）。
3. 15 个非中英文语言包的 `embedded.*` 文案仍是旧 OAuth 措辞（本次只补了功能键）。
4. Rust **12 条 warnings** 未逐条查看。
5. 站点源 `version.json` 仍是 3.14.0 ⇒ **3.15.0 与 3.16.0 都还没上站**；`release/` 里只有 3.15.0 的安装包。

---

## 6. 本轮工具坑（别再走）

- **任何直连阿里云盘的探针都必须带 `Accept-Encoding: gzip`**，否则把「服务端挂 body」误判成「网络不通」。
- 判别性测试**不能用 `RequestBuilder::build()`** 查 `Accept-Encoding`（reqwest 0.13 发送时才注入），必须「本地恒回 gzip 的服务 + 断言明文 + 断言请求头」。
- `b29-cdp-build.cmd` 是**假绿陷阱**（npx 提前返回 + copy 撞锁仍 exit 0）——用 `b29-build-final.ps1`。
- `cmd /c "npx tauri build"` 需要 cargo 在 PATH，否则 `cargo metadata` 报 `program not found`。
- release 构建约 6 分钟，期间勿动 target；`cargo test --lib` 与 `cargo build --release` 必须**串行**。
- `b29-build-final.ps1` 会用旧件 `afm-plain.exe` 覆盖 `alpha-file-manager.exe` ⇒ 正式件必须在旗标回滚后**另行重建**。
- 链路二跑太早会 `NO_TARGET_MATCH: navigator` —— 先确认 `/json/list` 主窗 url 已是 `http://tauri.localhost/navigator`。
- 链路三 Tauri 参数必须 **camelCase**（`remotePaths/localDir/overwritePaths/jobId`）；snake_case 会失败。
- **grep 工具会跳过 `.superpowers` 目录** ⇒ 查 b27/b28/b29 取证文件用 read 或 `Select-String`。
- 改 README 会触发两个 vitest 门禁（markdown 表格列数、相对 URL），改完必须复跑 vitest。
- 改 CHANGELOG / locale 文件必须保持 **LF、无 BOM**；PowerShell `Set-Content` 会写 BOM ⇒ 用 Node 脚本或 edit 工具写文件。
- 抓屏守卫：`GetThreadDesktop` + `GetUserObjectInformationW` 确认 Default 桌面；优先 `PrintWindow(hwnd,hdc,2)`；窗口按 **PID + 面积降序**取最大。
- 现存 32 个 `msedge.exe` 是用户自己的常规 Edge（无 9222、无 b27-edge-profile）⇒ **勿杀**。

---

## 7. 权威台账位置

- `.superpowers/sdd/b29-status-2026-11-20-1730-final.md` —— 最终收口台账
- `.superpowers/sdd/b29-handoff.md` §10 —— 批次 29 收口章节（含授权范围：**commit/bump 本地做，push 需单独确认**）
- `.superpowers/sdd/b29-webview-impl.md` §6.1/§6.2/§7/§8 —— 实现说明
- `.superpowers/sdd/b29-webview-acceptance.md` §5 —— 验收记录
- `.superpowers/sdd/b29-diag-05.txt` —— gzip 根因 A/B 决定性证据

> `.superpowers/sdd/` 整目录被 `.superpowers/sdd/.gitignore` 忽略，**不进 git**；本文件是仓库内那份可追溯的交接。
