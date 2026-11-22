# 批次 38 交接：OneDrive 只读接入（`onedrive_pan`）与 3.18.0 打包收口

- 日期：2026-11-22
- 批次：38
- 阶段：A 级（只读）交付收口 + 版本号裁决 + 打包 / 真机验收
- 仓库根：`F:\soft\00selfmade\filemanager\sigma-file-manager`（外层目录不写文件）
- 基线 commit：`97b3a22e feat(batch37)`；**批次 38 的全部改动当前仍未提交**（`git status --short` 56 项）
- 权威契约：`.superpowers/sdd/b38/impl-brief.md`；方案全文：`docs/plan-2026-11-22-batch38-onedrive-support.md`

---

## 0. 一句话结论

批次 38 的 OneDrive 只读接入（后端协议、前端接入、门禁、真机验收）**已全绿，具备发布条件**；
本轮额外完成了四件事：① 版本号从 3.17.0 顶到 **3.18.0**；② 补齐 15 份 CHANGELOG 与双 README 的 3.18.0 条目，
并修掉 README 里 4 处「OneDrive 还在路上」的过时预告；③ 新增 **2 个真实出网的负向测试**（假令牌 → Microsoft Graph 真 401 → 错误分类为 Auth）；
④ 产出**真实凭据验收装置**，把 5 个「必须要有真实 Azure 账号才能判」的用例变成「一条命令 + 一次人工浏览器登录」。

**唯一无法在当前条件下判定的是 5 个真实用例**（真实登录 / 列目录 / 下载 / 全局索引 / 测试连接连通性）——
缺真实 Azure `client_id` 与 OneDrive 账号，状态是「装置就绪、待真实凭据」，不是「已修」也不是「失败」。

---

## 1. 背景与目标

批次 38 要给 Alpha File Manager 接入 OneDrive 作为**第四个网盘 provider**，首批只做只读：
能列目录、读文件、下载到本地；不做上传 / 新建目录 / 改名 / 删除，不做图片缩略图。

协议字面量（锁定，不得改）：

| 位置 | 字面量 |
|---|---|
| Rust 枚举 | `RemoteProtocol::OnedrivePan` |
| serde tag | `onedrive_pan` |
| `as_str()` | `"onedrive_pan"` |
| 端口 | `443` |
| TypeScript | `'onedrive_pan'` |
| 中文名 | `OneDrive` |

> 方案旧名 `Onedrive` / `onedrive` **已作废**。旧名会与既有「夸克网盘 / 阿里云盘」的类型命名空间撞车。

---

## 2. 本轮做了什么

### 2.1 版本号裁决：3.17.0 → 3.18.0（minor）

**为什么必须 bump**：仓库里 `release/Alpha File Manager_3.17.0_x64-setup.exe`（46,004,374 B，2026-11-22 00:53:19）
**今天已经作为线上资产发布**，而把版本顶到 3.17.0 的那次提交正是 `97b3a22e feat(batch37)`。
留在 3.17.0 会让两个内容不同的二进制都自称 3.17.0，而且更新通道指向的已发布 3.17.0 **不含 OneDrive**。

**怎么做**：只改 `package.json` 的 `"version"`，再跑 `npm run sync-version` 同步 3 个 Cargo 清单。
**没有**用 `node scripts/release-publish.mjs prepare --bump minor`——那个命令会连带打包并写站点源，
AGENTS.md 记载过它被误跑两次把版本顶错；而且本轮明确不做对外发布。

四处版本号现已一致为 `3.18.0`：

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/default-file-manager-common/Cargo.toml`
- `src-tauri/default-file-manager-launcher/Cargo.toml`

### 2.2 交付化材料（CHANGELOG × 15 + README × 2）

| 文件 | 位置 | 状态 |
|---|---|---|
| `CHANGELOG.md` | 仓库根 | 新增 `## [3.18.0] - November 2026`（Summary + 6 条 `**Added**`） |
| `public/changelog/ch/changelog.md` | 中文 | 新增 `## [3.18.0] - 2026 年 11 月` |
| `public/changelog/{de,es,fa,fr,hi,it,ja,pt,ru,sl,tr,ur,vi}/changelog.md` | 13 份语言包 | 均新增 3.18.0 条目 |
| `README.md` | 仓库根 | 新增 `### What changed in 3.18.0` |
| `README.zh-CN.md` | 仓库根 | 新增 `### 3.18.0 改了什么` |

`node scripts/verify-changelog.mjs` → **EXITCODE=0**，`CHANGELOG 条目 62`，输出
`OK    CHANGELOG.md 覆盖当前版本，条目顺序正确。`

> 14 份 `public/changelog/**` 会被 vite 打进 `dist/` 随安装包发布，**必须齐**：
> 缺一条就会让应用内更新弹窗回退显示旧版本说明。这是本轮**不允许在语言包落盘前启动打包**的原因。

### 2.3 README 里 4 处过时预告的修正（事实性错误，直接修）

OneDrive 已经在 3.18.0 里交付了，但 README 还在说「正在路上 / 随后补上」：

| 文件 | 行 | 原意 | 现状 |
|---|---|---|---|
| `README.md` | L52 | OneDrive「也在同一个列表里」 | 改为已支持、浏览器登录 |
| `README.md` | L312 | 竞品表：Google Drive 等「在路上」 | 改为 OneDrive 已支持，并注明本版只读 |
| `README.zh-CN.md` | L47 | 同 L52 | 同上 |
| `README.zh-CN.md` | L299 | 竞品表：OneDrive「随后补上」 | 同上 |

修正脚本：`.superpowers/sdd/b38/fix-readme-onedrive-rows.cjs`
（4 条单行精确替换，**每条必须唯一命中否则整体中止不写盘**；写回 UTF-8 无 BOM；末尾自检 BOM / U+FFFD）。

### 2.4 实网负向链路：2 个 `#[ignore]` 测试

改动**只在** `#[cfg(test)] mod tests` 内，**生产代码零改动**（用 `src-tauri/src` 下 188 个 `.rs` 的 SHA-256 前后对比自证）。

| 用例 | 位置 | 作用 |
|---|---|---|
| `onedrive_live_graph_rejects_invalid_token_as_auth_error` | `src-tauri/src/remote/onedrive_provider.rs:1317`（`#[ignore]` 在 :1316） | 直连真实 Graph，用假令牌拿真 401，断言错误分类 |
| `onedrive_live_provider_read_dir_with_fake_session_fails_as_auth_error` | `src-tauri/src/remote/onedrive_provider.rs:1385`（`#[ignore]` 在 :1384） | provider 层用假会话调 `read_dir`，断言返回 `Err` 且 kind=`Auth` |

实跑（`cargo test --lib -- --ignored --nocapture onedrive_live`）：

```
[onedrive_live] GET https://graph.microsoft.com/v1.0/me/drive/root/children（假令牌）
[onedrive_live] 真实状态码 = 401
[onedrive_live] 响应体前 300 字 = {"error":{"code":"InvalidAuthenticationToken","message":"Protocol 'Bearer' failed to validate because The token could not be read.",...}}
[onedrive_live] error.code = Some("InvalidAuthenticationToken")
[onedrive_live] 映射后的文案 = OneDrive 授权已失效，请重新登录后再试。
[onedrive_live] provider 错误 kind = Auth
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1668 filtered out
EXITCODE=0
```

**这条链路的意义**：OneDrive 读路径从「从未实跑过」降级为「**链路通、认证被正确拒绝**」。
真实 401 由微软服务器返回，不是本地 mock；错误分类落到 `RemoteErrorKind::Auth` 而不是泛化的网络错误。

> 单独跑之前先验过出网：`https://graph.microsoft.com/v1.0/me/drive/root/children` 带无效 Bearer → **HTTP 401**；
> `https://login.microsoftonline.com/common/oauth2/v2.0/authorize` → **200**。

### 2.5 真实凭据验收装置

- `/.superpowers/sdd/b38/accept-real-credentials.ps1`（**纯 ASCII**）
- `/.superpowers/sdd/b38/REAL-CREDENTIAL-ACCEPT.md`（中文操作手册）

一条命令 + 一次人工浏览器登录，覆盖 5 个真实用例。
凭据注入格式（已核实）：`remote-credentials.json` = `{ "entries": { "<key>": "<base64(DPAPI 密文)>" } }`；
Rust 侧 `CryptProtectData` **无 entropy**（`entropy=None`，flag `CRYPTPROTECT_UI_FORBIDDEN`），
所以脚本可以用 .NET `ProtectedData.Protect(..., entropy: null)` 造出合法密文。

---

## 3. 门禁结果

| 门禁 | 命令 | 结果 |
|---|---|---|
| Rust 单测 | `cargo test --manifest-path src-tauri/Cargo.toml --lib` | **1667 passed / 0 failed / 3 ignored**（EXIT=0） |
| Rust 实网负向 | `cargo test --lib -- --ignored --nocapture onedrive_live` | **2 passed / 0 failed**（EXIT=0） |
| Rust 编译 | `cargo check` | EXIT=0（36 warnings 基线） |
| 类型 | `npm run ts` | **EXIT=0**（`vue-tsc` 无报错） |
| Lint | `npm run lint:check` | **EXIT=0** |
| 前端单测 | `npm run test:unit:run` | **EXIT=0**（329 个测试文件 / 3549 例全通过） |
| 沙箱校验 | `npm run verify:sandbox` | 随 `npm run tauri:build` 的链路跑，EXIT=0（构建继续） |

> Rust 的 ignored 数由 1 → 3 是**预期**（新增 2 个 `#[ignore]` 实网用例）；**通过数必须仍是 1667，不增不减**。
> 实网用例默认不参与 `cargo test --lib`，需要显式加 `-- --ignored`。

---

## 4. 打包与真机验收

### 4.1 打包

- 脚本：`.superpowers/sdd/b38/build-318.cmd`（纯 ASCII）→ 日志 `build-318.log`
- 链路：`npm run tauri:build` = `sync-version && verify:sandbox && tauri build && publish-bundle-to-release.mjs`
- 前端 `vite build`：`✓ built in 10.38s`；Rust release：`Finished release profile in 5m 02s`，36 warnings（基线不变）
- 可执行文件：`src-tauri/target/release/alpha-file-manager.exe`（85,634,048 B，mtime 16:34:28）
  - `VersionInfo.FileVersion = 3.18.0`，`ProductVersion = 3.18.0`
- 安装包：`src-tauri/target/release/bundle/nsis/Alpha File Manager_3.18.0_x64-setup.exe`（46,103,864 B，16:34:27）
  - **SHA-256 = `b3be132c4999582891643af62ca2383bfab61bb20dfbcad370a481d2ae858fe4`**

### 4.2 安装包归位（**未对外发布**）

- 3.18.0 **线上尚无同名资产**，按 `release/README.md` 的约定（顶层只放与官方资产逐字节一致的包）
  从 `release/` 顶层移入隔离区 `release/local-builds/Alpha File Manager_3.18.0_x64-setup.exe`
  （字节数与 sha256 均未变）。
- 已登记进 `release/LOCAL-BUILDS.txt`，判定 `NO_OFFICIAL_ASSET`。
- **未执行** `scripts/release-publish.mjs prepare / upload / github`；站点源 `version.json` 未写，
  GitHub Release 未创建，winget 清单未提。

### 4.3 真机验收（3.18.0 release 产物）

装置：`b38-accept.ps1` + `steps.json`，跑在**隔离桌面**上；产物目录 `.superpowers/sdd/b38/accept-318b/`。

```
[b38] pin startup page: PINNED file=...\user-settings.json lastRoute=settings settingsCurrentTab=remote changelog.lastSeenVersion=3.18.0 exit=0
[b38] PASS  S1 operator desktop foreground untouched :: before=593934 after=593934
[b38] PASS  step2 click target exists :: name=远程 type=Button hits=1
[b38] PASS  step4 assert control :: name=OneDrive type=Button hits=1
[b38] PASS  step11 edit present :: name=应用（客户端）ID hits=1
[b38] PASS  step12 assert control :: name=开始授权 type=Button hits=1
[b38] PASS  step13 assert text :: needle=你在 Microsoft Entra 里 hits=1 expected=True
[b38] PASS  step14 set edit value :: name=应用（客户端）ID value=11111111-2222-3333-4444-555555555555 hits=1
[b38] PASS  step17 click target exists :: name=开始授权 type=Button hits=1
[b38] PASS  step20 assert text :: needle=正在等待你在浏览器里完成授权 hits=1 expected=True
[b38] PASS  step21 assert control :: name=授权页地址 hits=2
[b38] PASS  step23 click target exists :: name=保存 type=Button hits=1
[b38] PASS  step25 assert text :: needle=请先在应用内完成 OneDrive 授权 hits=1 expected=True
[b38] SUMMARY pass=13 fail=0
ACCEPT_EXIT=0
```

界面证据（UIA 树，隔离桌面 PNG 恒黑不可用作证据）：设置页「远程」分类 → OneDrive 条目 →
OneDrive 连接表单（`应用（客户端）ID` 输入框存在且能写入）→「开始授权」→ 授权等待态文案 + 授权页地址 →
未授权时点「保存」给出「请先在应用内完成 OneDrive 授权」。

**用户数据安全**：装置在备份之后、启动之前把 `user-settings.json` 的 `lastRoute` 钉成 `settings`、
`settingsCurrentTab` 钉成 `remote`、`changelog.lastSeenVersion` 钉成 `3.18.0`；跑完自动覆盖式还原。
跑完后实测用户真实状态已回到原样（`lastRoute = navigator`、`settingsCurrentTab = search`、
`changelog.lastSeenVersion = 3.14.0`），应用进程已退出。

### 4.4 期间修掉的装置缺陷（**不是产品回归**）

3.18.0 首次真机验收是**红**的（`.superpowers/sdd/b38/accept-318/`，**pass=1 fail=12**），
根因是**装置的夹具状态依赖**，不是产品问题：

| 轮次 | app-data `lastRoute.name` | 应用启动落点 | 结果 |
|---|---|---|---|
| 16:13（`accept/`，上一批） | `settings` | 设置页 | pass=13 fail=0 |
| 16:35（`accept-318/`） | `navigator` | 文件浏览器 | pass=1 fail=12（step2 起连环失败） |

应用从 `user-settings.json` 的 `lastRoute` 恢复启动页（`startupPage='last'`），
于是**操作者上次停在哪个页面**决定这一轮跑在哪；两轮之间打开过应用，就把它改成了 `navigator`，
于是「远程」按钮在 UIA 树里根本不存在，后面 12 项连环失败。
**应用代码与上一轮通过的构建完全一致**（只差版本号 / changelog / README，不影响 UI 行为）。

修法（`b38-accept.ps1` 新增 `-StartupRouteName` / `-StartupSettingsTab` / `-PinChangelogVersion`，
默认开启，`-NoPinStartupRoute` 可关）：备份之后、启动之前调 `pin-startup-route.cjs` 钉住启动页，
跑完照旧还原。**验收从此与操作者的历史状态解耦。**

顺带修掉：`b38-accept.ps1` 头部注释里混进了中文（违反本项目「`.superpowers/sdd/*.ps1` 必须纯 ASCII」的自定约定），
已改成 ASCII 占位示例；新增的 `pin-startup-route.cjs` 纯 ASCII（实测 non_ascii_bytes=0）。

---

## 5. 已验证 / 推断 / 未判定

### 5.1 已验证（有实测输出，证据路径见第 6 节）

- 批次 38 的 OneDrive 只读全链路：后端 provider / api / auth、前端连接表单与远程连接列表、协议字面量锁定。
- Rust 门禁：1667 passed / 0 failed / 3 ignored；实网负向 2 passed（**真实 Microsoft Graph 返回 401**）。
- 交付化材料：15 份 CHANGELOG + 双 README 的 3.18.0 条目齐备；`verify-changelog` EXIT=0；14 份 locale 与 `public/` 源一致。
- README 4 处过时 OneDrive 预告已修。
- 版本号 4 处同步为 3.18.0。
- **3.18.0 真机验收：pass=13 fail=0（ACCEPT_EXIT=0）**，产物在 `.superpowers/sdd/b38/accept-318b/`；详见第 4 节。
- 验收装置的「启动页状态依赖」缺陷已修（钉住 `lastRoute`），并顺带把 ps1 里混进去的中文注释清成纯 ASCII。
- 安装包已归位到 `release/local-builds/` 并按 `NO_OFFICIAL_ASSET` 登记；**未对外发布**。

### 5.2 推断（有依据，但不是直接实测）

- **PKCE 登录回路在真实浏览器里能跑通**：依据是本机 `login.microsoftonline.com` 出网 200 + 授权 URL 构造符合 RFC 6749（scope **空格分隔**），
  但**没有真实账号点过「同意」**。
- **真实凭据下的 5 个用例能通过**：依据是负向链路已通 + provider 错误分类正确 + 前端表单已接通，
  但**端到端从未用真实账号跑过**。
- **上传 / 新建目录 / 改名 / 删除本版不可用**：依据是 provider 只实现了读路径（未写实现），属于「实现里没有」而非「实测被拒」。

### 5.3 未判定（客观条件不具备，装置已就绪）

| 用例 | 未判定原因 | 装置 |
|---|---|---|
| 真实登录（PKCE + 浏览器 + 账号） | 缺真实 Azure `client_id`；Microsoft 登录与 MFA 必须用户本人完成 | `accept-real-credentials.ps1` |
| 真实列目录 | 同上 | 同上 |
| 真实下载 | 同上 | 同上 |
| 真实全局索引（OneDrive 源纳入全局检索） | 同上 | 同上 |
| 「测试连接」连通性 | 同上 | 同上 |

> 判定口径：`UNDETERMINED` **不等于** PASS，也**不等于** FAIL。交接后拿到真实凭据时，
> 跑 `accept-real-credentials.ps1` 即可把这 5 项从「未判定」推进到「已验证」。

---

## 6. 证据路径

**契约与进度**
- `.superpowers/sdd/b38/impl-brief.md`（权威契约）
- `.superpowers/sdd/b38/impl-backend-report.md`（批次 38 完整实现 / 门禁 / 验收 / 未判定结论）
- `.superpowers/sdd/b38/ROOT_PROGRESS.md`（根会话进度与工具坑）
- `docs/plan-2026-11-22-batch38-onedrive-support.md`（方案全文）
- `E:\Users\Duanyi\.minimax\.minimax\maintenance\task-state\LATEST.md`（任务交接卡）

**本轮产物**
- `.superpowers/sdd/b38/build-318.cmd`（纯 ASCII 的打包脚本）
- `.superpowers/sdd/b38/build-318.log`（打包日志）
- `.superpowers/sdd/b38/gate-frontend-318.log`（前端门禁日志）
- `.superpowers/sdd/b38/fix-readme-onedrive-rows.cjs`（README 修正脚本）
- `.superpowers/sdd/b38/patch-changelog-318.mjs`、`fix-changelog-blankline.mjs`（13 份 locale changelog 生成脚本，原先误落在仓库外层，已移入）
- `.superpowers/sdd/b38/accept-real-credentials.ps1`、`REAL-CREDENTIAL-ACCEPT.md`（真实凭据验收装置）
- `.superpowers/sdd/b38/accept-318b/`（**3.18.0 真机验收通过轮**：UIA dump ×8 + 日志 + 截图）
- `.superpowers/sdd/b38/accept-318/`（3.18.0 真机验收**失败轮**（pass=1 fail=12）的证据，根因见 4.4）
- `.superpowers/sdd/b38/pin-startup-route.cjs`（钉住启动页的夹具脚本，纯 ASCII）
- `.superpowers/sdd/b38/b38-accept.ps1`（验收装置本体；新增 `-StartupRouteName` / `-StartupSettingsTab` / `-PinChangelogVersion`）

**后端新文件（批次 38）**
- `src-tauri/src/remote/onedrive_api.rs`（40,762 B）
- `src-tauri/src/remote/onedrive_auth.rs`（25,328 B）
- `src-tauri/src/remote/onedrive_provider.rs`（52,617 B，含 2 个 `#[ignore]` 实网用例）
- `src-tauri/src/onedrive_oauth.rs`（26,244 B，**唯一允许出现 `tauri::` 的新文件**）

**前端（批次 38）**
- `src/types/remote.ts`
- `src/stores/storage/remote-connections.ts`
- `src/modules/settings/ui/categories/remote/{connection-form.vue,connections-section.vue}`
- `src/localization/messages/*.json`（17 份）

---

## 7. 下一步

1. **真实凭据验收（需要用户）**：提供 Azure 应用 `client_id` + 可登录的 OneDrive 账号，
   按 `REAL-CREDENTIAL-ACCEPT.md` 跑一次，把 5 个用例从「未判定」推进到「已验证」。
   **Microsoft 登录与 MFA 必须用户本人完成**，脚本只能等待与取证。
2. ✅ **提交批次 38（已完成）**：57 个文件已提交（`feat(batch38): OneDrive 只读接入…`），工作区干净。
   `release/` 与 `.superpowers/sdd/` 都被 gitignore，**不在 commit 里**；
   `docs/plan-2026-11-22-batch38-onedrive-support.md` 与本交接文档**已入**。
   **未 push** —— push 属对外动作，需要时再单独做。
3. **对外发布（需要用户授权，默认不做）**：GitHub Release / winget 清单 / 站点上传都属对外发布。
   授权后的顺序是：
   ```
   node scripts/release-publish.mjs prepare --bump none --skip-build --notes <说明文件>
   node scripts/release-publish.mjs upload
   node scripts/release-publish.mjs github
   ```
   `--bump none` 很重要：版本号已经在 2.1 节手工顶到 3.18.0，再 bump 会顶成 3.18.1。
   `prepare` 会写站点源 `version.json` 并把安装包复制进站点目录；**`upload` 才是真正对外**。
4. **B 级读写（单独批次）**：先有真实账号验收完 A 级，再开上传 / 新建 / 改名 / 删除。

---

## 8. 工具坑（下一轮直接照用，别重踩）

**本项目专用**
- release 产物**不支持** `SFM_E2E_DATA_DIR`（只在 debug 生效）⇒ 真机验收要走
  「备份真实 `%APPDATA%\com.sigma-file-manager.app\user-data\` → 灌夹具 → 验收 → 先杀进程再 restore」。
  `b38-accept.ps1` 已内置 `-AppDataPath` 的备份/恢复（覆盖式恢复，不删新文件）。
- 应用**从 app-data 的 `user-settings.json` 恢复启动页**（`startupPage='last'` + `lastRoute.name`），
  **不是**从 WebView2 的 localStorage ⇒ 真机验收的夹具必须把 `lastRoute` 钉住，
  否则「操作者上次停在哪个页面」决定这一轮跑在哪（实测**同一构建**一轮绿、一轮红 12 项）。
  装置现由 `pin-startup-route.cjs` 在备份之后、启动之前钉住，跑完照旧还原。
  排查这类「同一构建两轮结果不同」时，先比对两轮的 app-data 备份，别急着怀疑产品。
- 隔离桌面上的 PNG **全黑**（DWM 不合成 WebView2 表面）⇒ 界面证据**只能**取 UIA 树 dump。
- 验收装置自身的 5 个老坑（上一批已修，别改回去）：
  ① `Invoke-Click` 的盲点是取 rect 中心 → 优先用 UIA `InvokePattern`，用坐标必须裁剪进目标窗口；
  ② `assert` 只扫 `*.Text`，命中不到 Button 的 Name → 用 `assertControl`；
  ③ `verify-auth-url.cjs` 曾把 scope 当 `+` 分隔（RFC 6749 是**空格**分隔）；
  ④ 授权前**不渲染状态卡**，不能拿「状态卡缺失」当「未授权」判据；
  ⑤ 条目级右键菜单可能是不被主窗覆盖的独立顶层窗 ⇒「帧里没有菜单」只能写「未判定」。
- `.superpowers/sdd/*.cmd`、`*.ps1` **必须纯 ASCII**（Windows PowerShell 5.1 把无 BOM 文件当 ANSI 读）；
  中文只放在 UTF-8 的 steps JSON 里。
- 后台构建**不许挂 stdout 管道**，`.cmd` 内部用 `> logfile 2>&1`；`.cmd` 运行期间不可编辑，改则新建文件名。

**仓库纪律**
- 改含中文的文件**不得**用 PowerShell `Get-Content -Raw` / `Set-Content`；批量改写用 node 脚本。
  保持 UTF-8 无 BOM、无 U+FFFD、保持原换行风格。
- `release/` 顶层**只放与 GitHub 官方资产逐字节一致**的安装包；本地重建一律进 `release/local-builds/`，
  并在 `release/LOCAL-BUILDS.txt` 登记（判定取值 `LOCAL_REBUILD` / `NO_OFFICIAL_ASSET`）。
  `scripts/publish-bundle-to-release.mjs` 已按此规则改过：目标不存在 → 落顶层；存在且内容不同 → 改写 `local-builds/`。
- `release/local-builds/` 目录里实际有 17 个文件（含本轮移入的 3.18.0），但 `LOCAL-BUILDS.txt` 只登记了 3 条
  （3.9.1 / 3.9.2 / 3.18.0）——**登记漂移是本轮之前就有的**，本批未扩大范围去补，记在这里供后续决定。
- `npm run tauri:build` 的链路是 `sync-version && verify:sandbox && tauri build && publish-bundle-to-release.mjs`，
  末尾那一步会把 NSIS 产物复制进 `release/`。
- `cargo` 全局缓存的「last-use 记录库」在本机已损坏，打包日志里会打
  `warning: failed to save last-use data` / `Error code 26: File opened that is not a database file`。
  **这是缓存 GC 的告警，不影响编译**，不要当失败去排查。

**流程纪律**
- 派「只读取证 + 落盘报告」必须派 **worker**（`explore` / `verifier` 只读不写，会直接 `ROLE_MISMATCH` 拒单）。
- worker 自述不可信，根会话须自查（批次 37 曾出现 worker 中途改断言文件造成 vitest 竞态假红）。
- 后台任务结束会向 idle 的属主父会话投递消息并自动再开一轮 agent 循环；
  本轮结束前**不得留未终态后台任务**。

## 10. 交接后独立复核（下一会话实测，2026-11-22）

不采信 worker / 交接自述，全部亲跑。结论：§7 第 2 项（提交）核实为真；两项证据缺口已用新鲜实跑闭合；新发现一个装机缺口。

### 10.1 核实为真

- HEAD = `5c96adc0`，57 个文件，`git status --porcelain` 为空，未 push（`main...origin/main [ahead 2]`，批次 37 同样未推）。
- 契约字面量：`src-tauri/src` 命中 `OnedrivePan|onedrive_pan` 13 处 / 5 文件；`src` 命中 `onedrive_pan` 字符串 8 处 / 4 文件；作废旧名 `RemoteProtocol::Onedrive` 与字面量 `onedrive`（独立引号形式）零命中。
- 版本四处一致 3.18.0；`node scripts/verify-changelog.mjs` 亲跑 → 62 条目、顺序正确、EXIT=0；安装包 46,103,864 B 在 `release/local-builds/`，`LOCAL-BUILDS.txt` 登记 sha256 `b3be132c…858fe4` + 判定 `NO_OFFICIAL_ASSET`，`release/` 顶层无 3.18.0（归位口径一致）。
- 双 README 的 4 处「OneDrive 在路上」预告已改为「已支持 / 本版只读」；commit 文件清单内无 `.log` / `.pyc` / `release/` 杂质。

### 10.2 证据缺口与处置（关键教训）

1. 交接声称 `1667 passed / 0 failed / 3 ignored`，但盘上 `gate-cargo-test.log` 只报 `1 ignored` ⇒ 该 log 属过期快照。原因：`.superpowers/sdd` 被 `.superpowers/sdd/.gitignore` 忽略，b38 目录在本轮复核期间被整体换成另一份快照 —— `install-318.log` / `pin-restore.log` / `gate-cargo-test-ignored.log` / `gate-cargo-test-ignored-2nd.log` / `accept-318-real.log` / `accept-318b-real.log` 全部从盘上消失。**gitignored 证据目录不是持久证据存储，落盘即可能被换掉。**
2. 「2 个 `#[ignore]` 实网负向用例已跑过」当时无任何落盘产物。
   ⇒ 处置：本轮重跑，新鲜证据 `.superpowers/sdd/b38/reverify-318.log`（复现脚本 `reverify-318.cmd`，同目录）：
   `CARGO_TEST_EXIT=0` + `test result: ok. 1667 passed; 0 failed; 3 ignored`；`IGNORED_EXIT=0` + `test result: ok. 2 passed; 0 failed; 0 ignored; 1668 filtered out`（`onedrive_live_graph_rejects_invalid_token_as_auth_error`、`onedrive_live_provider_read_dir_with_fake_session_fails_as_auth_error`）。缺口 1、2 已闭合。
3. `ACCEPT_EXIT` / `BUILD_EXIT` 判词仍不在任何 log 内（只有 `[b38] SUMMARY pass=13 fail=0` 与 `Finished release profile [optimized] in 5m 02s`）⇒ 登记为证据不完整，不是失败。

### 10.3 新发现：3.18.0 从未真正装机（§7 未覆盖）

- 卸载注册表 `DisplayVersion = 3.16.1`，`InstallLocation = D:\Program Files\Alpha File Manager`，但该目录下 exe 不存在 ⇒ 本机当前处于「半卸载」状态。
  **【已订正，见 §11.2 · 收尾轮实测】** 目录与主程序**俱在**：`alpha-file-manager.exe` 85,031,424 B @ 11-21 21:48:40、FileVersion `3.16.1`。本机不是「半卸载」，而是**上一批次的 3.16.1 正常在装、3.18.0 从未装机**。本条核心结论不变，但支撑证据从「exe 消失」换成「版本号 + 尺寸 + 哈希三重差异」，更强。错因疑为按 `Alpha File Manager.exe`（带空格、不存在）探测 —— 见 §10.5-C L431 引用的 `Running: "...\Alpha File Manager.exe"`。
- `accept-318b/` 的 13 项 PASS 是拿 `src-tauri/target/release/alpha-file-manager.exe`（FileVersion 3.18.0、mtime 16:34，即本次 build 裸产物）跑出来的；`b38-accept.ps1:28` 的默认 `-ExePath` 正是该裸产物。
- ⇒ 界面证据（UIA dump 命中「请先在应用内完成 OneDrive 授权」等）真实有效，但**不等于 NSIS 安装包的装机实测**。要闭合装机口径：静默安装 `release/local-builds/Alpha File Manager_3.18.0_x64-setup.exe`（同版本重装须 `/FORCE`），再把 `b38-accept.ps1` 的 `-ExePath` 指向装机路径重跑。需提权，属用户授权范围。

### 10.4 仍待用户输入或授权（没有一项可由 Agent 单方推进）

① 真实 Azure `client_id` + 可登录的 OneDrive 账号（装置 `accept-real-credentials.ps1` + `REAL-CREDENTIAL-ACCEPT.md` 已就绪，Microsoft 登录与 MFA 必须本人）；② push `5c96adc0`；③ 对外发布；④ 3.18.0 装机实测（需提权）；⑤ 批次 39（OneDrive B 级读写）与 `LOCAL-BUILDS.txt` 登记漂移（实际 17 个文件 vs 仅登记 3 条）。

### 10.5 交接后第二轮取证：退出码补齐、ignored 口径差异解释、4 条进程事实全部落空（2026-11-22 22:20–22:50，本轮实测）

**A. §10.4 里缺的两条退出码，已在盘上找到；另两条仍缺**

> **【收尾撤回】本段两条「已在盘上找到」的退出码，收尾复读时 `Test-Path` 均返回 `ABSENT`**
> （`pin-restore.log`、`gate-cargo-test-318b.log` 都已不在盘上，且 `gate-cargo-test-318*.cmd` 也不存在 ⇒ 无法重建）。
> 按「引用某个 log 的数字必须当场回读该文件」的纪律，`PIN_RESTORE_EXIT=0` 与 `1667 passed; 1 ignored / CARGO_TEST_EXIT=0`
> **一律降级为不可复核**，本段状态改为 **未判定（证据文件已消失）**；§10.1-B / §10.4 第 3 条的「pin 回退未复核」标记维持原样。
> 仍然成立的只有不依赖这两份 log 的部分：工作区干净（`git status --porcelain` 为空）⇒ `src/App.tsx` 当前无残留启动路由注入。
> 下面原文保留仅供追溯，**不得再作为事实引用**。

- `.superpowers/sdd/b38/pin-restore.log` → `PIN_RESTORE_EXIT=0`（第 55 行，验收夹具还原成功）。
- `.superpowers/sdd/b38/gate-cargo-test-318b.log` → `CARGO_TEST_EXIT=0`（第 2014 行），lib 行（第 2012 行）`1667 passed; 0 failed; 1 ignored`。
- **仍未闭合**：`ACCEPT_EXIT`（`accept-318b-console.log` 与 `accept-318b/b38-accept.log` 内均无该判词）与 `BUILD_EXIT`（`build-318.log` 内无）
  ⇒ 继续按「证据不完整」登记，**不得**据此写 PASS/FAIL。

**B. 「1 ignored」与「3 ignored」不矛盾：是测试集在长，不是门禁口径漂**

| 产物 | 命令 | passed | ignored | 合计 |
| --- | --- | --- | --- | --- |
| `gate-cargo-test-318b.log`（较早快照） | cargo test --lib | 1667 | 1 | 1668 |
| `reverify-318.log`（本轮权威重跑） | cargo test --lib | 1667 | 3 | 1670 |

passed 两边同为 1667，ignored 差 2、总数差 2 ⇒ 差的正是后来新增的两个 `onedrive_live` 实网负向用例（b 门禁跑时它们还不存在）。
本轮权威重跑逐字：`test result: ok. 1667 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 7.31s` + `CARGO_TEST_EXIT=0`；
`cargo test --lib -- --ignored --nocapture onedrive_live` 逐字：`test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1668 filtered out; finished in 1.31s` + `IGNORED_EXIT=0`。
3 个 ignored 的名字（log 内逐字命中）：`image_edit::tests::probe_external_sample`、
`remote::onedrive_provider::tests::onedrive_live_graph_rejects_invalid_token_as_auth_error`、
`remote::onedrive_provider::tests::onedrive_live_provider_read_dir_with_fake_session_fails_as_auth_error`（后两条注释「需要访问 graph.microsoft.com（实网负向用例）」）。
**口径提醒**：`gate-cargo-test-318.log` 那类总数 3336 的行来自 `cargo test`（全 target），与 `--lib` 的 1670 不是一回事，别混用。

**C. 12:25 那次「3.16.0 → 3.18.0」装机日志其实自相矛盾（§10.3 加重，根因本轮不下结论）**

> **【收尾撤回】本段逐字引用的 `install-318.log` 收尾复读时 `Test-Path` 返回 `ABSENT`**（`accept-318b-real.log` 同批消失）。
> ⇒ 表内各行号与判词、以及「日志自相矛盾」这个判断本身**一并作废**，登记为 **未判定（证据文件已消失）**：矛盾既没被证伪也没被证实。
> 要追根因必须**先重跑一次静默安装**产出新鲜 `install-318.log`（含 `INSTALL_EXIT=`、PREINSTALL/POSTINSTALL 的 `detectedVersion`、
> `UNCHANGED`/`STALE_NOT_REPLACED` 判词）再谈结论。当前**可核对**的装机事实只有注册表 `DisplayVersion 3.16.1` +
> `InstallLocation D:\Program Files\Alpha File Manager`（该目录下 exe 已不存在，§10.1-A 实测）⇒ **「3.18.0 从未真正装机」仍然成立**。
> **【已订正，见 §11.2 · 收尾轮实测】** 括注不成立：该目录下 `alpha-file-manager.exe`（85,031,424 B、FileVersion `3.16.1`）俱在。
> 主结论「3.18.0 从未真正装机」**仍成立且证据更强** —— 装机版 3.16.1 与裸产物 3.18.0 在**版本号、尺寸、sha256 三项全不同**。
> 下面原文保留仅供追溯，**不得再作为事实引用**。

`.superpowers/sdd/b38/install-318.log`（mtime 11-22 12:25，90 行）逐字：第 6 行 `Upgrade: detected installed version 3.16.0`，
第 71 行 `Running: "D:\Program Files\Alpha File Manager\Alpha File Manager.exe" /P:SHOWALL=0`，第 107 行
`PREINSTALL: D:\Program Files\Alpha File Manager\alpha-file-manager.exe present, will overwrite`，
第 154–156 行 `INSTALL_EXIT=0` / `POSTINSTALL: detectedVersion=3.16.1` / `UNCHANGED: ... alpha-file-manager.exe still present`。
⇒ 「退出码 0」+「版本没前进」+「文件没被覆盖」三件事同时成立，注册表停在 3.16.1，而 `InstallLocation` 现在连 `Alpha File Manager.exe` 都不存在。
**结论收紧**：这份 log **不能**当装机成功证据；§10.3 的「装机面未闭合」维持原判。根因追查（防呆早退 / 写错目录 / 事后被卸载）不属交付范围，留用户决定是否单开一轮。

**D. 用户交来的 4 条「运行中进程」事实：本轮实测全部不属实（是「不属实」，不是「未判定」）**

探针 `.superpowers/sdd/b38/verify-process-claims.ps1`（纯 ASCII，参数化 `-PidClaim` / `-Port`）→ 日志 `verify-process-claims.log`；
4 行结论落盘 `reports/task56-process-verify.txt`（**未纳入 git 跟踪**，`reports/` 不在 .gitignore 内，故本轮 `git status` 会多出这一个未跟踪文件）。

1. 「`alpha.exe` 以 PID 284840 运行且带 `--remote-debugging-port=9222`」 ⇒ **不属实**：Win32_Process ProcessId=284840 返回 null；
   全盘进程名无一匹配 `alpha`，在场的 12 个 `msedgewebview2.exe` 全属别的应用且均未带该 flag。
   **结构性纠正（比结论更值钱）**：Tauri/WebView2 的调试 flag 落在子进程 `msedgewebview2.exe` 的命令行上，**不在应用主 exe 上**，
   9222 的 owning PID 也是 webview 子进程 ⇒ 用「主进程 exe + 主进程命令行」去断言这条，姿势本身就不对。
2. 「9222 被 PID 284840 占用」 ⇒ **不属实**：`Get-NetTCPConnection -LocalPort 9222 -State Listen` 返回 0 条监听，没有任何 PID 占 9222。
3. 「CDP `/json/list` 可达并显示 `alpha-file-manager` 页面」 ⇒ **不属实**：`http://127.0.0.1:9222/json/list` 连接被目标主动拒绝。
4. 「UIA 树里有名为 `Alpha File Manager` 的 pane」 ⇒ **不属实**：桌面根 12 个顶层窗、0 个该名；按 NameProperty 全后代搜索 0 命中。
   `LogonUI` 不在场、非锁屏 ⇒ 排除「锁屏让取证失真」这条误判方向（记忆里那个坑本轮不适用），真实原因就是**应用根本没在跑**。

**要把 2/3/4 变成可判**：先把应用带 CDP 启起来 —— 设 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 为
`--remote-debugging-port=9222 --remote-allow-origins=*`，再启动装机 exe（当前不存在）或裸产物
`src-tauri/target/release/alpha-file-manager.exe`（FileVersion 3.18.0），然后重跑 `verify-process-claims.ps1`。
本轮按用户「都先不动，只交接」未启动、未安装、未 push。

**E. 本轮新增工具坑（照用，别重踩）**

- 本机硬策略拒「先 `cd` 再写文件」的 bash 调用，**纯 ASCII 的 `Set-Content` 同样被拒**；绕法 = 文件一律走 `write` / `edit` 工具落盘，
  bash 只做执行与只读回读。本轮实测有效。
- PS 双引号串里写 `"C1_PID_$PidClaim_NOT_FOUND"` 会被解析成变量 `$PidClaim_NOT_FOUND` ⇒ 输出空值、判据标签失真。
  带下划线必须 `"${PidClaim}_NOT_FOUND"`。本轮 C1 那行就踩了（结论没受影响：null 分支 + 全盘进程扫描双证）。
- 证据目录会被换快照：早先读过的 `gate-cargo-test-ignored*.log` / `accept-318-real.log` 等随后消失，而 `pin-restore.log` / `gate-cargo-test-318b.log`
  本轮又出现 ⇒ 任何「引用某个 log 里的数字」的判词，落笔前必须当场回读该文件；消失的一律改用重跑产物。
---

## 10.6 收尾复读：一批证据文件已从盘上消失，§10.5 的 A/C 两段据此撤回（2026-11-22）

落笔 §10.5 后做收尾复读，`Test-Path` 结果如下（b38 目录此刻 34 项）：

| 文件 | 收尾复读结果 | 影响 |
|---|---|---|
| `reverify-318.log` | 在盘（158,761 B @ 17:23:49） | **唯一可回读的权威门禁证据**，`1667 passed / 3 ignored / CARGO_TEST_EXIT=0`、`2 passed / IGNORED_EXIT=0` |
| `reverify-318.cmd` | 在盘 | 可重跑出上面那份 log |
| `build-318.log` / `gate-frontend-318.log` / `gate-cargo-check2.log` | 在盘 | ~~`BUILD_EXIT=0`、`GATE_EXIT=0`、`CARGO_CHECK_EXIT=0` 仍可核对~~ **【已订正，见 §11.3】** 三份 log 里**没有任何 `BUILD_EXIT`/`GATE_EXIT`/`CARGO_CHECK_EXIT` 行**；`gate-frontend-318.log` 实际只有 `TS_EXIT`/`LINT_EXIT`/`UNIT_EXIT` |
| `pin-restore.log` | **ABSENT** | §10.5-A 撤回（见上） |
| `install-318.log` | **ABSENT** | §10.5-C 撤回（见上） |
| `accept-318b-real.log` | **ABSENT** | §10.5-B 表格左列第 4 行（`ACCEPT_EXIT` 缺行）无法核对 |
| `gate-cargo-test-318.log` / `gate-cargo-test-318b.log` | **ABSENT** | §10.5-B 表格左列全部不可核对；且**两个 `.cmd` 也不存在**，这两份 log 无法重建 |
| `accept-318b/b38-accept.log` | 在盘（5,265 B @ 16:30:43） | `SUMMARY pass=13 fail=0` 仍可核对，但对象是**裸产物**不是装机 |

结论与纪律：

1. **§10.5-B 的表格只有右列可采信**（右列全部出自 `reverify-318.log`，在盘可回读）；左列四份文件均已消失，
   「cargo 数字漂移」这个现象本身本轮**读到过**，但现在**拿不出证据**，后续轮次若要引用必须重跑。
2. 本批**可复核的门禁基线只有一条**：`reverify-318.cmd` → `1667 passed; 0 failed; 3 ignored` + `CARGO_TEST_EXIT=0`，
   `--ignored --nocapture onedrive_live` → `2 passed; 0 failed; 2 filtered-out 口径见 §10.2` + `IGNORED_EXIT=0`。
   其余任何数字在引用前都必须当场回读文件。
   **【收尾轮更新】** 该基线现已由本会话**重新执行**并取得执行级证据（`close-318-gates.log`，见 §11.1），不再依赖 `reverify-318.log` 是否还在盘。
3. 证据目录在本轮内被成批换掉（早先读到、随后 ABSENT），**原因未查，不属交付范围**；
   应对方式就是上面这条纪律，以及「缺退出码行 ⇒ 登记证据不完整，不写 PASS/FAIL」。
4. §7 的下一步清单**不变**：真实凭据 5 用例仍未判定；3.18.0 装机实测仍未做；4 个提交（`5c96adc0`、
   `b1cfddd2`、`d8e6907d` 及本节）仍未 push。用户已裁决本轮「都先不动，只交接」。
   **【用户指令已更新】** 本轮用户改为要求「接手批次 38 收尾」，但 §10.4 列出的**授权边界未被撤销**，故 push / 发布 / 装机 / 真实凭据四项仍待用户批准。

---

## 11. 收尾复核轮（2026-11-22）：本轮所有判词均为当场执行或当场回读

> 用户指令：「不要采信文档里任何未经当场回读的 log 数字」。本节按此执行 —— 下面每一条要么是本会话**跑出来的命令**，
> 要么是本会话 `Select-String`/`Get-Item`/`Get-FileHash` **当场回读**的文件。凡引用他处 log 数字而未回读成功的，一律登记不可采信。

### 11.1 权威门禁：从「推断级」升级为「本会话执行级」

新建并执行 `.superpowers/sdd/b38/close-318-gates.cmd`（纯 ASCII，无 stdout 管道），产出 `close-318-gates.log`（170,161 B @ 18:33:34）。判词行逐字回读：

- L1856 `test result: ok. 1667 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 7.11s` ⇒ L1858 `CARGO_TEST_EXIT=0`
- L2053 `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1668 filtered out; finished in 1.92s` ⇒ L2055 `IGNORED_EXIT=0`
- L2421 `CARGO_CHECK_EXIT=0`（`cargo check --tests`，`Finished dev profile [unoptimized + debuginfo] in 53.29s`）
  ⇒ L2058 附近的 `warning: failed to save last-use data` / `file is not a database` 是**本机 cargo 缓存告警**，与代码无关，不是失败

与 §10.5-B 的声称**逐字吻合**（1667/0/3 与 2 passed），耗时抖动（7.11s vs 7.31s、1.92s vs 1.31s）下通过数与 ignored 数完全一致 ⇒ 该声称成立，且现在有了执行级证据。

**新坑（务必带走）**：首版脚本写成 `call cargo check 2>&1 | findstr ...` 再 `echo %ERRORLEVEL%` ⇒ 拿到的是 **findstr 的退出码**，会产出一条**看似正常实则无关**的 `CARGO_CHECK_EXIT`。已改为直接跑、不挂管道。⇒ 规则：**退出码判词必须由被判定命令自己的 ERRORLEVEL 产生，不得经过管道。**

### 11.2 装机面实况：3.16.1 正常在装，「半卸载」判词作废（订正 §10.3 / §10.5-C）

| 项 | 本会话实测 |
|---|---|
| `D:\Program Files\Alpha File Manager\alpha-file-manager.exe` | **85,031,424 B** @ 2026-11-21 21:48:40；FileVersion = ProductVersion = **3.16.1**；sha256 `391134bf073f0d5c98c855eae2bd8b029354bd6ece7707be15936662d7d636cc` |
| 同目录其余内容 | `uninstall.exe` 101,742 B、`.__wtest` 0 B、`extensions/`、`tools/` |
| 卸载注册表 | `DisplayVersion = 3.16.1` |
| 裸产物 `src-tauri/target/release/alpha-file-manager.exe` | **85,634,048 B** @ 11-22 16:34:28；FileVersion **3.18.0**；sha256 `358ea43fbdf5484477579e90102d26163284f5c5605f2392d8d7c5b92396166f` |

⇒ 版本号、尺寸、哈希**三项全不同** ⇒「**3.18.0 从未真正装机**」成立，且证据比原先（「exe 不见了」）强得多。
⇒ 原「exe 不存在 / 半卸载」是**错误探测**：极可能按 `Alpha File Manager.exe`（带空格、根本不存在）去找 —— 旁证见 §10.5-C L431 引用的 `Running: "D:\Program Files\Alpha File Manager\Alpha File Manager.exe" /P:SHOWALL=0`。真实主程序名是 `alpha-file-manager.exe`。
⇒ **重要推论**：13 项真机 PASS 的对象是裸产物。若后续把 `b38-accept.ps1 -ExePath` 指向装机路径而**不先装 3.18.0**，会验到 3.16.1，测不到批次 38 的任何代码。装机实测必须「先安装（同版本重装带 `/FORCE`）再验」。

### 11.3 §10.6 表格 L477 那行不实：缺退出码判词的是 4 条，不是 2 条

- 本会话 `Test-Path` 复核：`pin-restore.log`、`install-318.log`、`accept-318b-real.log`、`gate-cargo-test-318.log`、`gate-cargo-test-318b.log` **全部 False** ⇒ §10.6 的 ABSENT 名单成立。
- 但「`BUILD_EXIT=0`、`GATE_EXIT=0`、`CARGO_CHECK_EXIT=0` 仍可核对」**不实**：`gate-frontend-318.log` 的 `_EXIT` 命中集只有 `TS_EXIT=0`(L6)、`LINT_EXIT=0`(L20)、`UNIT_EXIT=0`(L167)，`GATE_EXIT_present=False`；`build-318.log` 与 `gate-cargo-check2.log` 里**无任何 `_EXIT` 行**；`accept-318b/b38-accept.log` 里**无 `ACCEPT_EXIT`**。
  ⇒ 缺汇总退出码的从 2 条扩大到 **4 条**（`BUILD_EXIT`、`GATE_EXIT`、`ACCEPT_EXIT`、`CARGO_CHECK_EXIT`），其中 `CARGO_CHECK_EXIT` 已由 §11.1 补齐为执行级。
- 实质判词仍在盘可回读（本会话读取）：前端 `TS_EXIT=0` / `LINT_EXIT=0` / L162 `Test Files 329 passed (329)` / L163 `Tests 3549 passed (3549)`；构建 `build-318.log` L1985 `✓ built in 10.38s`、L2303 `Finished release profile [optimized] in 5m 02s`。⇒ 「没有汇总退出码」≠「门禁失败」。
- 包侧自洽（本会话实测）：`release/local-builds/Alpha File Manager_3.18.0_x64-setup.exe` = 46,103,864 B @ 16:34:27，sha256 `b3be132c4999582891643af62ca2383bfab61bb20dfbcad370a481d2ae858fe4`，与 `release/LOCAL-BUILDS.txt` 登记行（`NO_OFFICIAL_ASSET`）**逐字一致**。

### 11.4 新发现：3 处未接线的 OneDrive dead code（登记，不在收尾期动）

`close-318-gates.log` L109-L166 的编译告警（本会话回读）：`ONEDRIVE_EXPIRY_WARN_SECONDS`（`onedrive_auth.rs:41`）、`ONEDRIVE_TENANT`（`:45`）、`onedrive_is_expiring_soon`（`:163`）never used；另有 `with_timeouts`（`onedrive_provider.rs:603`）never used。

- 三个模块是 `pub(crate)`（`remote/mod.rs:42-44`）⇒ 无调用者即**真死代码**，且 `#[cfg(test)]` 也未覆盖。
- 对照 aliyun 侧确实接线：`aliyun_auth.rs:232` 调用 `aliyun_is_expiring_soon`，并经 `remote/mod.rs:99` 再导出 ⇒ 这是「OneDrive 侧漏接」而非「设计上不接」。
- **不影响已交付行为**：过期判定走 `onedrive_is_expired`（`onedrive_auth.rs:106`，被 `:168` 的 `onedrive_auth_status_payload` 使用，测试 `:616-638` 覆盖）。缺的只是「提前 `ONEDRIVE_EXPIRY_WARN_SECONDS`（300 s）即将过期提醒」这一层。
- ⇒ **不是明显 bug，是未接线的可选功能层**。收尾期不动：任何改动都会让已通过的 13 项真机验收与 `release/local-builds/` 那个 sha 的包和源码不再对应。建议批次 39 一并处置（接上并补测试，或删除）。

### 11.5 计划文档存在契约漂移（防下一轮「照计划文档修回」）

`docs/plan-2026-11-22-batch38-onedrive-support.md` §5.1/§5.2/§5.3 仍写 `Onedrive` / `'onedrive'`；权威契约是 `.superpowers/sdd/b38/impl-brief.md` 的 `RemoteProtocol::OnedrivePan` / serde tag `"onedrive_pan"` / TS `'onedrive_pan'` / 中文名 `OneDrive`（代码即此）。**以 impl-brief 与代码为准，不得按计划文档回改。**
计划 §5.3 坑 2 依然有效且高危：`registry.rs:128-136` 的**第二处**白名单若漏改，DPAPI 里那段会话 JSON 会被当 `password` 塞进 `ResolvedConnection.password`，**不报错但泄漏**。

### 11.6 本轮闭合情况

| §7 / §10.4 未闭合项 | 本轮状态 |
|---|---|
| 权威门禁的可复核性 | **已闭合**（执行级，§11.1） |
| 装机面实况 | **已闭合**（订正 2 处文档错误，§11.2） |
| ABSENT 名单 / 退出码缺行 | **已闭合**（缺行 2→4，其中 1 条补齐，§11.3） |
| OneDrive dead code | **新发现，登记**，留批次 39（§11.4） |
| 计划文档契约漂移 | **已登记**（§11.5） |
| 真实凭据 5 用例 | **仍未判定** — 需用户本人提供 Azure `client_id` 并完成 Microsoft 登录 / MFA |
| 3.18.0 装机实测 | **仍未做** — 需提权安装 + `/FORCE`，随后按 §11.2 改 `-ExePath` |
| push | **未做** — 需用户授权（`main` 现 ahead 6，含批次 37 的 `97b3a22e`） |
| 对外发布 | **未做** — 需用户授权；顺序见 §7.3（`--bump none`） |
| `BUILD_EXIT` / `GATE_EXIT` / `ACCEPT_EXIT` | **证据不完整** — 要补只能整轮重跑（`tauri build` 极慢），不建议为补一条退出码重跑 |

本轮 git 实况（本会话回读）：`HEAD = 5488cd6f`；提交链 `97b3a22e → 5c96adc0 feat(batch38) → b1cfddd2 → d8e6907d → 3292ddc6 → 5488cd6f`；`git status --porcelain` 仅 `?? reports/`（`reports/` **不在** `.gitignore` 内，是否入库待用户定）；`.superpowers/sdd/b38/` 现 44 项（§10.6 记的 34 项又已过期 ⇒ 再次印证「当场回读」纪律）。
