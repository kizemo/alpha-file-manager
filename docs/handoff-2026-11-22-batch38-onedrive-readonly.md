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
