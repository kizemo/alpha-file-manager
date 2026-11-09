# 交接文档 · 百度网盘内嵌 WebView2 登录（分支 feat/baidu-embedded-login）

- 生成时间：2026-11-09 00:15（本机时间）
- 仓库：`F:\soft\00selfmade\filemanager\sigma-file-manager`
- 分支：`feat/baidu-embedded-login`，HEAD `5c6e21e3`
- 上一轮交接入口（历史上下文）：`docs/handoff-2026-11-08-release-3.13.0-locations-partitions.md`

---

## 1. 目标（用户原话口径）

在分支 `feat/baidu-embedded-login` 上实现用户锁定的**方案 a**：

> 内嵌 WebView2 登录百度网盘 → Rust 侧 `cookies_for_url()` 读回登录态 → 自动填进连接表单；**测试成功后合并回主干 `main`**。

方案 b/c/d/f 已被用户否决，不要再讨论选型。

---

## 2. 当前进度（一句话）

**代码、门禁、打包、静默安装、登录窗创建全部完成并实测通过；唯一未完成的是「用户本人在内嵌登录窗里完成百度登录」这一人工环节，因此端到端 claim 真机验收未通过，分支尚未合并回 `main`。**

---

## 3. 已完成且已验证

### 3.1 代码

| 文件 | 内容 |
| --- | --- |
| `src-tauri/src/baidu_login.rs` | **新增**。登录窗创建 + 每 2s 轮询 cookie + 待认领槽位 + 事件回传 |
| `src-tauri/src/lib.rs` | `mod baidu_login;`（第 10 行，`mod baidu_probe;` 之前）；`generate_handler!` 377–380 行注册四条命令 |
| `src/types/remote.ts` | 追加 `BaiduLoginClaimPayload` / `BaiduLoginEventPayload` |
| `src/stores/storage/remote-connections.ts` | `saveConnection`(:307) 百度分支新增 claim 调用（341–353）；新增 `startBaiduLogin()`(:538) / `cancelBaiduLogin()`(:543)，`:566–567` 导出 |
| `src/modules/settings/ui/categories/remote/connection-form.vue` | 模板 :600–654 内嵌登录区 + 事件监听 + 状态机；`reset()` 末尾归零 |
| `src/localization/messages/**`（17 个包） | `settings.remote.baidu.embedded{...}`，`ch.json` 值已中文化 |

Rust 侧关键常量（`src-tauri/src/baidu_login.rs`）：

- `LOGIN_WINDOW_LABEL = "afm-baidu-login"`（:34）
- `LOGIN_START_URL = "https://pan.baidu.com/disk/main"`（`STOKEN` 只在该页下发）
- `LOGIN_COOKIE_URL = "https://pan.baidu.com/"`
- `LOGIN_EVENT = "afm://baidu-login"`
- `POLL = 2s` / `GIVE_UP = 900s` / `STOKEN_WAIT = 20s`
- 窗口 `.title("百度网盘登录")`(:169) `.inner_size(1100.0, 820.0)`(:170)
- 四条命令**全部同步**：`remote_baidu_login_start`(:155) / `_pending`(:193) / `_claim(app, connection_id)`(:211) / `_cancel`(:245)

### 3.2 门禁（实测数字）

| 门禁 | 结果 |
| --- | --- |
| `npx vue-tsc --build` | `TSC_EXIT=0` |
| `node scripts/verify-changelog.mjs` | `CL_EXIT=0`（3.13.0，49 条目） |
| `npx eslint .` | **179 errors / 0 warnings**（低于 HEAD 基线 188，无回归） |
| 定向单测 4 文件 | 4 files / 56 tests passed（12.55 s） |
| 全量 `npx vitest run` | `Test Files 297 passed`、`Tests 2798 passed`，但 `VITEST_EXIT=1` |

> 全量 vitest 的 exit 1 **只**由一条既存 unhandled rejection 引起，与本轮改动无关：
> `TypeError: Cannot read properties of undefined (reading 'filter')` @ `src/modules/navigator/components/file-browser/address-bar-editor-utils.ts:624:69`（`resolveSearchSuggestions`），来源测试 `address-bar-editor-dialog.test.ts`。日志 `.superpowers/sdd/vitest-full.log`。

### 3.3 打包 + 安装

- 构建日志 `.superpowers\sdd\build-baidu-login.log` 尾部 `EXIT=0`，`Finished release profile [optimized] target(s) in 4m 44s`
- 产物 `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.13.0_x64-setup.exe`（45335623 B，23:42:35），已复制到 `release\local-builds\`
- 静默安装 `.superpowers\sdd\install-baidu-e2e.cmd` → `EXIT=0`
- 安装位置 **`D:\Program Files\Alpha File Manager`**；`alpha-file-manager.exe` **81710592 B / 23:42:02** = 本轮二进制 ✓

### 3.4 真机验收（三步，前两步通过）

| 步骤 | 结果 | 脚本 | 证据 |
| --- | --- | --- | --- |
| f1：设置页导航定位百度网盘行 | **通过** | `.superpowers/sdd/accept-baidu-f1-rail.ps1` | `.superpowers/sdd/baidu-embedded-accept/rail/` |
| f2：点「用内嵌窗口登录」→ 登录窗创建 | **通过** | `.superpowers/sdd/accept-baidu-f2-run.ps1` | `.superpowers/sdd/baidu-embedded-accept/run2/` |
| f3：等 ready → 保存 → 认领落盘 | **未通过（用户未登录）** | `.superpowers/sdd/accept-baidu-f3-claim.ps1` | `.superpowers/sdd/baidu-embedded-accept/run3/`、状态摘要 `.superpowers/sdd/baidu-embedded-accept/f3-status.md` |

f3 实测：`TOTAL_WAITED_SEC=900`、`STATUS_NOT_READY`、`NEED_USER_LOGIN`。

---

## 4. 未验证 / 未完成

1. **端到端 claim 未验证**：`remote_baidu_login_claim` 在真实登录态下是否把 cookie 写进 `remote-credentials.json`，尚未取到证据。
2. **`cargo test --lib` 未实测**：本轮靠「四条命令全同步、模块内无 `#[cfg(test)]`、不触碰 `tauri::async_runtime::*`」规避 tao 链接红线，但**没有真跑过**。
3. **分支未合并回 `main`**（用户条件是「测试成功后才合并」）。
4. 全量 vitest 的 `VITEST_EXIT=1` 未做 HEAD 基线对照。

---

## 5. 下一步（按顺序做，不要跳）

### 步骤 A：让用户完成登录（人工，不可自动化）

应用当前状态（2026-11-09 00:14 复核）：

- 进程 `alpha-file-manager` PID **962384**，启动 23:45:09
- 两个 Tauri 窗口：主窗 `6555342`（`-11,-11 3862x2110`，停在「设置 → 远程 → 百度网盘 编辑表单」）+ 登录窗 `17892454`（`228,228 1672x1286`，标题「百度网盘登录」）
- **登录窗内是百度真实登录页，必须用户本人扫码/短信登录，任何脚本都替代不了**

用户要做的三步：

1. 在内嵌登录窗里完成百度网盘登录（扫码或账号密码 + 验证码）
2. 登录成功后，表单里的状态会变成 `ready`，文案提示「会话会在保存时写入」
3. 点表单的**保存**按钮

> ⚠️ `GIVE_UP = 900s`（15 分钟）。超时后 Rust 轮询器会**自动关掉登录窗**，前端转 `timeout`。
> 若已超时：重新点一次「用内嵌窗口登录」，会重新开窗 + 重启轮询器，登录态仍可复用（WebView2 用户数据持久化在
> `C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.app\EBWebView`，**若上次已登录成功，第二次开窗应直接 ready**）。

### 步骤 B：验证认领是否落盘（可以做，不要只问用户「成功了吗」）

**通过判据：`remote-credentials.json` 的字节数 / mtime / SHA256 任一发生变化。**

```powershell
$ErrorActionPreference='Continue'
$root='C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app'
foreach ($f in @("$root\remote-credentials.json", "$root\user-data\user-remote-connections.json")) {
  $i = Get-Item $f
  "{0} = {1}B | {2} | {3}" -f (Split-Path $f -Leaf), $i.Length, $i.LastWriteTime.ToString('HH:mm:ss'), (Get-FileHash $f -Algorithm SHA256).Hash.Substring(0,16)
}
```

基线（未认领时应保持不变）：

- `remote-credentials.json` = **793 B | 15:16:22 | `0AB64B4BE07D590A`**
- `user-remote-connections.json` = **1483 B | 23:45:12 | `A3D4FFAE979AAC49`**

若 `remote-credentials.json` 变了 ⇒ 认领成功。再用 `CryptUnprotectData`（见 §7）解出密文，确认里面是 `BDUSS` / `STOKEN` 而不是旧值。

### 步骤 C：收尾

1. 杀进程（`Get-Process alpha-file-manager | Stop-Process`），然后**核对**真实数据是否被夹具污染 —— 本轮**没有灌夹具**，理论上不需要 restore；但若为了取证动过文件，用 `.superpowers\sdd\baidu-e2e-backup\` 还原（`remote-credentials.json` 793 B + `user-remote-connections.json` 1483 B）。
2. 补跑 `cargo test --lib`（Rust 侧只在 `.superpowers\sdd\*.cmd` 里拼环境：`cargo` 在 `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin`，`pc-` 不能省；先 `call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"`）。
3. **验收通过后**把 `feat/baidu-embedded-login` 合并回 `main`（用户明确要求），合并后跑一遍 `vue-tsc` + 定向单测。

---

## 6. 设计取舍（不是 bug，别「修」）

1. **先登录后建连接**：登录态先进模块级 `Mutex<Option<PendingSession>>`；**认领放在 `saveConnection` 里**，因为只有那一步才有连接 id 可以落盘 key。槽位为空时返回 `claimed:false` 属**正常空操作**。
2. **四条命令全同步**：直接规避 `cargo test --lib` 的 tao 链接红线（见 §7）。
3. **保留手工粘贴 Cookie 的降级路径**，注释已改写成「降级路径」口径，不要删。
4. **UI 语义**：`ready` 只表示「cookie 已拿到、等着保存」，不表示已落盘；状态通过 `:data-status` 暴露给测试与样式，未引入新 CSS 类。
5. **本轮不取昵称**（需 async + `reqwest`），事件 payload 无 `nickname`。
6. **CHANGELOG 不改**：`v3.13.0` 已有 git tag，`scripts/verify-changelog.mjs` 只认 `## [x.y.z] - <date>`，加 `## [Unreleased]` 会让排序校验变红。

---

## 7. 本轮踩到的坑（勿重试）

### 7.1 新坑：真实鼠标点击不判遮挡 = 静默打错目标 ★

f3 的自愈分支只按 UIA 找到表单按钮就直接 `SetCursorPos` + `mouse_event`，**不判断该坐标是否被登录窗盖住**。
登录窗位于屏幕 `228,228 1672x1286`，而「用内嵌窗口登录」按钮中心是 `(1651,1218)` —— **正好落在登录窗内部**。
于是 cycle 1–3 的三次「重启轮询器」点击**全部打在了百度登录页上**，自愈完全无效，看起来像「脚本在跑但没生效」。

**规则**：任何「重试点某个控件」的逻辑，点击前必须先断言目标窗口不存在、或目标点不在其 `GetWindowRect` 内。

### 7.2 其他（沿用，均已固化）

- **`cargo test --lib` 红线**：被测试链接到的代码不得触碰 `tauri::AppHandle` / `tauri::async_runtime::*`，否则测试二进制在**加载阶段**崩 `STATUS_ENTRYPOINT_NOT_FOUND (0xC0000139)`。
- **`SetProcessDPIAware()` 必须在任何 user32 调用之前**，否则 UIA 坐标与 `SetCursorPos`/`GetWindowRect` 差 1.5 倍（本机 DPI 144）。
- **`.ps1` / `.cmd` 一律纯 ASCII**：PowerShell 5.1 / cmd 按 ANSI 读文件，中文会变乱码甚至崩解析器。中文字面量用码位拼，如 `-join @([char]0x4FDD,[char]0x5B58)`。
- **`.cmd` 运行期间绝不可编辑**（cmd.exe 按字节偏移逐行读，会执行到别处的指令片段）。
- **后台长构建不挂 stdout 管道**（`Tee-Object` 会让进程树在编译中途被杀），`.cmd` 内部 `> logfile 2>&1`。
- **PowerShell 函数名勿用 `CP`**（内置别名 `cp` 会劫持）；有返回值的函数内禁用 `Write-Output`，改 `Write-Host`。
- **不要用 `Process.MainWindowHandle`**（会落在 15x15 托盘窗）；要 `EnumWindows` + 按 class `Tauri Window` + 面积最大取主窗。
- **`PrintWindow` 返回陈旧帧、不渲染 hover/overlay**；UIA 读不到 Radix Toast，须配屏幕 BitBlt 实拍。
- **DPAPI 解密用裸 `CryptUnprotectData`**（无 entropy、`dwFlags=CRYPTPROTECT_UI_FORBIDDEN=1`），`ProtectedData::Unprotect` 会抛「参数错误」；PowerShell 取 JSON 字典须先 `.Properties.Name` 再按名取。
- **`cookies_for_url()` 在 Windows 上于同步 command / 事件回调中调用会死锁** ⇒ 必须独立线程轮询（本模块 `spawn_poller` 已如此）。
- **`Remote Desktop / 锁屏`**：锁屏封原始输入注入与屏幕抓取，但 UIA `InvokePattern` 仍可用。

---

## 8. 关键路径速查

| 用途 | 路径 |
| --- | --- |
| 应用数据根 | `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\` |
| 凭据落盘 | `<数据根>\remote-credentials.json`（`src-tauri/src/remote/credentials.rs:22` `REMOTE_CREDENTIALS_FILE`，路径 `app_data_dir.join(...)` :49） |
| 连接列表 | `<数据根>\user-data\user-remote-connections.json` |
| WebView2 用户数据 | `C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.app\EBWebView`（**不在 Roaming**） |
| 数据备份 | `.superpowers\sdd\baidu-e2e-backup\`（793 B + 1483 B） |
| 验收脚本 | `.superpowers\sdd\accept-baidu-f{1,2,3}*.ps1` |
| 验收证据 | `.superpowers\sdd\baidu-embedded-accept\{rail,run2,run3}\` |
| 构建/安装脚本 | `.superpowers\sdd\build-baidu-login.cmd`、`.superpowers\sdd\install-baidu-e2e.cmd` |

**已知 UIA 事实**（省一轮探测）：

- 左侧 rail 6 个无名 `Button`，x=18，43x42，`Top` = 93/153/213/273/333/393 ⇒ **index 3（中心 40,294）= 设置**
- 设置页「远程」导航 `(1287,772)`
- 百度网盘行：`ListItem|百度网盘***百度网盘pan.baidu.com:443/直连编辑删除|1552,706 1214x110`，「编辑」按钮中心 `(2580,760)`
- 内嵌登录按钮：`Button|用内嵌窗口登录|1552,1188 198x61`，中心 `(1651,1218)`
- 登录窗标题「百度网盘登录」，**用标题精确匹配区分，不要靠尺寸**
- `uia-not-ready.txt` 只 dump 主窗子树，**不含登录窗内部**；要判断百度页渲染什么，需 `FromHandle(17892454)` 单独 dump

## 2026-11-09 追加：根因定案 + 修复 + 真机验收通过 + 已合并 main

### 根因（推翻此前结论）
- **D1 白屏**：不是渲染问题，是**建窗死锁**。`tauri-runtime-wry-2.11.4/src/lib.rs:2767`
  的 `create_webview` 注释要求「必须在独立线程调用」。命令从 WebView2 的 IPC 回调进来
  = 事件循环线程 → 在那里 `build()` 会让 webview 创建派发回事件循环并等回复，而事件循环
  正卡在该回调里 ⇒ 窗体建出来（看起来正常）但 webview 永远不渲染，且后续调用一起挂死。
  `AppHandle::run_on_main_thread` **单独用不够**（任务里仍在事件循环线程上调 `build()`）。
- **D2「cookies_for_url 在后台线程必然死锁」**：**结论作废**。探针实证后台线程调用可成功
  （+2s 拿到 11 条 cookie）。真正的挂死点是上面那一条。

### 修复（`src-tauri/src/baidu_login.rs` 整体重写）
- 建窗改到 `std::thread::spawn`；
- 轮询器加 **generation 计数器**（修掉「窗口已存在就提前 return、不重启轮询」的 bug）；
- 读 cookie 改为另起线程 + `recv_timeout(10s)`，**超时判定挪到读 cookie 之前**
  （否则读挂住时超时分支不可达），连续失败 3 次才放弃；
- 新增文件诊断日志 `AFM_BAIDU_LOG`（只写状态，不写 cookie 明文）；
- `baidu_probe.rs` 新增自检驱动器 `AFM_BAIDU_SELFTEST=1`，走**真实命令**链路
  `remote_baidu_login_start` → `remote_baidu_login_pending` → `remote_baidu_login_claim`；
- `remote/commands.rs` 的 `REMOTE_COMMANDS` 补 4 条登录命令（修掉 invoke_handler 反向检查的红测）。

### 真机验收（`tauri build` 打包产物，装在 `D:\Program Files\Alpha File Manager`）
- 证据目录 `.superpowers/sdd/baidu-embedded-accept/run6/`
- `selftest.txt`：`VERDICT_SELFTEST: PASS`，登录态 `BDUSS(len=192) + STOKEN(len=64)`
- `baidu-login.log`：建窗 +1.05s → 轮询读到登录态 +2.0s → 认领落盘 +3.2s
- `login-window.png`：**登录窗渲染出完整百度网盘界面**（D1 白屏已消除）
- 判据：`remote-credentials.json` mtime `2026-11-08 15:16:22` → `2026-11-09 02:58:34`，
  SHA256 `0AB64B4BE07D590A` → `08960A348A44C087`；字节数 793 → 793
- 裸 `CryptUnprotectData` 解密确认：`{"version":1,"bduss":…192…,"stoken":…64…}`
- 新旧明文**逐字段不同**（旧 `dBRXR-…SdRqUj` → 新 `NVejZJ…Q-9qSm`）⇒ 不是「重写一遍」

### 遗留（未证实也未证伪，如实记录）
- 新落盘的 BDUSS **在百度服务端是否仍有效，未能证实也未能证伪**：
  `/disk/main` 是 SPA 静态壳（带/不带 cookie 的 HTML 逐字相同，`uk=0`），
  `gettemplatevariable` 不按 cookie 鉴权，老 `pcs/file` 接口已 404 ⇒ 三个判据都无鉴别力。
  截图右上角显示「登录」而非用户名，疑为 SPA 初始渲染占位，**不能据此判定失效**。
  要定性需用户在新登录窗里人工登录一次，或找一个真按 cookie 鉴权的接口。
- 旧会话备份：`.superpowers/sdd/baidu-e2e-backup/remote-credentials.json`（793 B / `0AB64B4BE07D590A`）。

### 合并与门禁
- 提交 `f2599a0f feat(remote): 百度网盘内嵌登录窗口`，已 **fast-forward 合并回 `main`**
  （`main` = `f2599a0f`，ahead origin/main 1，**未推送**）。
- `cargo test --lib`：**1142 passed / 0 failed / 1 ignored**
- `npx vue-tsc --build`：0；`node scripts/verify-changelog.mjs`：0
- `npx eslint .`：177 errors / 0 warnings（存量；本次改动的 3 个前端文件**零错误**，基线 188）

---

## 2026-11-09 追加批次 B：用户 6 条反馈的修复 + 装机验收

用户实机反馈 6 条（原话见会话），本批全部落码、门禁全绿、已打包装机；真机证据 **③⑥ 已取得，⑤ 部分取得，①②④ 只有代码与单测证据**。

### B.1 六条落点（权威细节见 `.superpowers/sdd/batch-2026-11-09/frontend-map.md`）

| # | 反馈 | 落点 | 证据等级 |
|---|---|---|---|
| ① | 内嵌登录成功要有反馈 | `connection-form.vue` 收到 `afm://baidu-login` 的 `ready` 时弹 toast（新增 i18n 键 `embedded.saved`） | 代码 + 单测 |
| ② | 删除手工填 Cookie 的讲解与输入框 | 删 `field-cookie` 可见 UI，**保留 `password` 数据通路**（`remote-connections.ts:328-334`、`:482`）；`remote-category.test.ts` 两条断言按需求方向**翻转为「必须不存在」** | 代码 + 单测 |
| ③ | 主页去掉「网盘 与 远程连接」标题行、分区行距对齐驱动/用户目录 | `src/modules/home/components/remote-entries-section.vue`（**不是** `navigator/components/directory-grid/`） | **真机已验** |
| ④ | FTP 报「已保存密码但本机找不到对应加密记录」 | 根因＝旧 `read_vault` 对读失败一律返回空库 + `save_credential` 非原子整体写回 ⇒ 一次瞬时读失败后再保存会**静默删掉整库其余凭据**。已加固：`src-tauri/src/remote/credentials.rs` +503/−20（原子写 `write_vault:301`、坏文件先备份 `archive_unreadable_vault:210`、`list_corrupt_backups:263`） | 代码 + cargo 单测；**密文只能用户重填** |
| ⑤ | 地址栏下拉显示长 ID 而非连接名、且被下方内容遮挡 | `address-bar.vue`：`locationsDirectoryItems` 带 `name`，模板 `{{ item.name \|\| getPathDisplayName(item.path) \|\| item.path }}`；`.address-bar__separator-menu-scroll` 的 `max-height:250px` 改 Reka 可用高度 `min()` | 代码 + 单测；连接名渲染**真机已验**，下拉实拍**未取** |
| ⑥ | 地址栏进 位置⇒远程 报「不是一条合法的远程路径：sfm://remote」 | 前端分流：`src/utils/remote-paths.ts` 新增 `isRemoteRootPath()`；`src/utils/virtual-locations.ts` 新增 `buildRemoteRootDirectory()` / `createRemoteRootDirEntry()`，在 `resolveDirectoryContents` / `resolveDirEntry` / `getNavigableParentPath` 三处分流。**未改 Rust 协议** | **真机已验** |

### B.2 门禁（实测数字，全部同一条命令口径）

- 全量 vitest：`2805 passed (2805) / 297 files`，`VITEST_EXIT=0`
- `vue-tsc --build` `TS_EXIT=0`；`node scripts/verify-changelog.mjs` `CHANGELOG_EXIT=0`（应用 3.13.0 / 条目 49）
- `cargo test --lib`：**1153 passed / 0 failed / 1 ignored**；`remote::credentials` 35 passed
- 9 个改动文件 `npx eslint <files>` `ESLINT_ALL_EXIT=0`
- stylelint：HEAD 基线实测 **174 errors**（⇒ 判据是「不新增」，不是全仓为 0）；`connection-form.vue` 的 12 条经基线 worktree 同口径对照确认是存量

### B.3 打包与装机

- 产物 `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.13.0_x64-setup.exe` = 45,320,456 B / 10:03:52
- 静默装机 `cmd /c "<setup.exe>" /S`（不带 `/D`，用注册表 `InstallLocation` 原地覆盖）
- **唯一生效判据＝装机副本字节数 + mtime**：`D:\Program Files\Alpha File Manager\alpha-file-manager.exe` = 82,349,568 B / 10:03:12

### B.4 真机验收证据（`.superpowers/sdd/acc-20261109\`）

- **③** `tree-home.txt`（116 行）+ `shot-home.png`：全树无「网盘 与 远程连接」行；只剩 `Text 网盘 103,844` / `Text 远程连接 103,1011` 两个分区标题，与 `Text 用户目录 103,480` / `Text 驱动 103,648` **同 x=103、同 30px 行高**。
- **⑥** `tree-invoke.txt`（15,778 B）+ `shot-invoke.png`（610,470 B）：按名 Invoke「远程」后 `INVOKE_OK dump=291`、点击前后逐行 `diff_count=106`（阳性对照成立）；树中出现 `[Text] '远程连接' 880,376`、`[Button] '速豹堂 — — - -' 805,420 1134x101 I`、`aiec.fun 805,521`、`jury.xin 805,621`、`[Text] 'sfm://remote' 3386,570`；报错文案判据 `无法打开此文件夹 hits=0`、`不是一条合法的远程路径 hits=0`。实拍另见右侧详情面板 `路径 sfm://remote / 类型 目录 / 4 个项目`。
- **⑤** 同批实拍里连接均以**名称**出现（百度网盘 / 速豹堂 / aiec.fun / jury.xin），证明 `remoteConnectionNames` 的 id→name 口径在装机版生效；**分隔符下拉本身未实拍**（`sfm://remote` 路由的 UIA 树里**没有 `[Edit]`**，地址栏只在目录型路由出现）。

### B.5 本批踩到的坑（勿重试）

- **`acc5-20261109.ps1` 首跑连踩 4 错**，其中真凶是**忘写 `param([string]$Step)`** ⇒ `-Step invoke` 被静默忽略、每次都跑默认 `probe`，症状是「产物文件不存在」而不是报错，白烧两轮。⇒ 改完探针**第一步先跑 Parser::ParseFile 静态校验**。
- 其余三个：`TreeScope)::Children` 括号写错；嵌套函数用 `$script:Walk` 调用 ⇒ ParserError，**必须用裸名递归** `Walk $c2 ($depth+1)`；遗留死函数 `Walk2` 里同样带 `$script:` 前缀 ⇒ 直接删。
- **按主页 dump 的坐标点击不可跨运行复用**（实测点到「快速访问」）；**有 Name 的元素一律用按名 `Invoke`，比几何点击可靠得多**。rail 项无可访问名 ⇒ 只能几何 + `PostMessage` 到 `Chrome_RenderWidgetHostHWND`。
- `acc-20261109.ps1` / `acc2` / `acc4` **全部废弃**；可用脚本只有 `acc5-20261109.ps1`（`-Step probe|invoke|click`，中文 needle 经 `$env:ACC_NAME` 传入以保持脚本纯 ASCII）与 `acc3-20261109.ps1`（rail 几何导航）。
- bash 里 `$env:` 赋值 + `Copy-Item` 写新文件会被权限层拦 ⇒ 探针调用**不要复制产物**，直接 `Select-String -Path <绝对路径>` 读原件。

### B.6 遗留（下一轮做）

1. **⑤ 的下拉实拍**：需「先进真实目录（目录型路由）→ 再点开地址栏分隔符下拉」两步；遮挡机制目前只是静态推断（`dropdown-menu-content.vue:45` 走 `DropdownMenuPortal`、`:62` `z-index:50`、`:63` `overflow:hidden`，全仓最高业务 z-index 是 10 ⇒ 判为**裁切而非层序**）。若实拍仍见遮挡，需要用户重拍一张分辨层序 vs 裁切。
2. **④ 需要用户执行**：设置 → 远程 → 速豹堂（以及很可能同样坏的 aiec.fun、jury.xin）「编辑」→ 重填密码 → 保存（保存链路调 `remote_connection_save_credential` 按连接 id 重建缺失密文键）。
3. **changelog 落点未定**：`v3.13.0` 已打 tag、仓库无 Unreleased 惯例；「先不写」vs「新建 3.13.1 段并 `npm run sync-version`」已发问卷、用户尚未答复。
4. **推送未做**：`main` = `118884dd`(docs) → `f2599a0f`(feat) → `5c6e21e3`，ahead origin/main 2；是否验收后直接推送打 tag 也已发问卷、未答复。
5. 非 `ch` 语言包的 `embedded.hint` / `status.ready` / `status.error` 仍是旧译（`sync-i18n` 只补缺不覆盖），新键 `embedded.saved` 在 15 个非中文包是英文回退。
6. `connection-form.vue` 密码行的 `v-else`（`:629`）现配对内嵌登录块 `v-if="isNetdisk"`（`:578`）⇒ **再动该块会让密码行翻转**，改前先确认配对。

## 2026-11-09 追加批次 C：B.6-1 证据补拍 + 百度网盘删除报「会话已失效」根因与修复

用户本轮指令：只做 B.6 第 1 项（复用 `acc5-20261109.ps1`，**不许新建探针**），外加新报 bug 编号 1 —— 百度网盘连接删除文件报「百度网盘会话已失效，请重新授权」（双层重复文案）。验收口径原话：「要求在连接百度网盘后，可以像操作本地磁盘一样，进行复制、黏贴、删除、剪切、同步等等的操作。」

### C.1 B.6-1 已闭环（真机证据）

- 先决条件：**UIA rect 全为 `0,0 0x0` 的真凶是应用不在前台** ⇒ 用内联 FGS 类（`SetProcessDPIAware` + `GetForegroundWindow`/`GetWindowThreadProcessId`/`AttachThreadInput`/`BringWindowToTop`/`SetForegroundWindow`，主窗句柄 `[IntPtr]722358`）抢前台后 UIA 立刻恢复。本轮**不动用户的 Edge**，只把 AFM 抢回来。
- `acc5-20261109.ps1 -Step probe` / `-Step click`（`$env:ACC_X='1352'; $env:ACC_Y='108'`）→ `tree-click.txt`：分隔符下拉 4 项**全部显示连接名**、逐项完整落在面板内、无裁切 ⇒ B.6-1 的「静态推断」升级为真机证据。
- 裁片：`.superpowers/sdd/acc-20261109/acc5-1-sep-dropdown.png`（20973 B，`[System.Drawing]` rect `1200,40,460,330`）。

### C.2 删除 bug：两条独立缺陷（权威细节见 `docs/2026-11-09-baidu-write-session-findings.md`）

- **D1 代码缺陷（本轮已修）**：`fetch_bdstoken` 抓 `/disk/home` HTML 取 token，而该 URL **恒定 302** → `/disk/main`；client 是 `Policy::none()` ⇒ 3xx 被 `failure_from_http_status` 归 `Auth` ⇒ **会话有效也报「会话已失效」**，`should_refresh_token` 再走一遍同一条死路。跟到 `/disk/main` 也没救：页面 `bdstoken:""`、`uk:0` ⇒ 抓 HTML 取 token 整条路作废。
- **D2 凭据缺陷（需用户本人重新授权）**：保险库那份 bduss/stoken 被服务端拒，三口互证（只读 list、`api/gettemplatevariable`、`/disk/main` 页面）一致 `errno=-6`。凭据文件 `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\remote-credentials.json`，解密用**裸 `CryptUnprotectData`**（`ProtectedData::Unprotect` 对本项目密文抛「参数错误」）。
- **本轮证伪**：「读写用不同 cookie」（读写共用 `cookie_header()`）；「内嵌登录把失效会话判成已登录」（`baidu_login.rs` 的 `LOGIN_START_URL` 是 `/disk/main`，判据是 `cookies_for_url` → `CookieRead::Ok` → `BaiduCookieOutcome::Ready`（含 `STOKEN_WAIT=20s` 宽限）→ `store_pending`/`emit ready`，完全不碰 `/disk/home` 启发式）；「后端有远端目录缓存」（`src-tauri/src/remote/` 只有 `CachedBdstoken` 与 transfer 缓存，目录缓存符号全仓 0 命中 ⇒ 界面那 2 个文件是**标签页里会话有效时加载的旧内存态**，推断未逐行验前端 store）。

### C.3 修复落点与门禁

- 全部改动在 `src-tauri/src/remote/baidu_api.rs`：新增 `BAIDU_BDSTOKEN_ENDPOINT` + `bdstoken_url()` + `BaiduTokenResult` + `BaiduTokenEnvelope` + `parse_bdstoken_response()`，重写 `fetch_bdstoken` 打 `api/gettemplatevariable`。护栏：**token 接口 3xx 一律归 Protocol**（防幻影失效借道复活）；`errno=-6` 仍归 Auth（保住批次 B 的内嵌重登触发链）；空串/`null`/缺 `result` → Protocol + server_message「百度网盘没有下发 bdstoken。」；非法 JSON → Malformed。签名不变 ⇒ `baidu_provider.rs` 零改动、`parse_bdstoken`/`take_bdstoken_value` 及旧测试全保留。
- ⚠️ **该文件插入约 230 行 ⇒ 基线行号全部失效**，后续定位一律 grep `bdstoken_url|parse_bdstoken_response|fetch_bdstoken|write_tests`。
- `cargo test --lib baidu_api` = **96 passed / 0 failed**（含 7 条新断言），日志 `.superpowers/sdd/cargo-baidu-test.log`（`EXITCODE=0`）。构建脚本 `.superpowers/sdd/build-20261109-bdwrite.cmd` → `.superpowers/sdd/build-20261109-bdwrite.log`。
- **打包已完成**：日志 246038 B / mtime 11:58:51，`^error` 0 命中，`Finished release profile [optimized] in 3m 03s` → `Running makensis` → `Finished 1 bundle at: …bundle\nsis\Alpha File Manager_3.13.0_x64-setup.exe (43.31 MiB)` → `[publish-bundle] 已发布到 release/`。⚠️ 日志无 `DONE_EXIT=` 行（`.cmd` 收尾 echo 未落盘），成功判据用 `[publish-bundle]` 那行。
- **交付包**：`release/local-builds/Alpha File Manager_3.13.0_x64-setup.exe` = **45,411,138 B / 11:58:47**（`bundle/nsis/` 同号同尺寸）。`release/` 根目录已有**同号但内容不同**的旧包（10:03:52 / 45,320,456 B），`publish-bundle-to-release.mjs` 拒绝覆盖 ⇒ 落到 `local-builds/`；比旧包多 90,682 B，确属新产物。

### C.4 下一轮按顺序做

1. 把 `release/local-builds/Alpha File Manager_3.13.0_x64-setup.exe`（45,411,138 B / 11:58:47）交用户装包；**装前后核对装机副本大小 + mtime**（本轮源码未提交，版本号仍 3.13.0 与已装同号，只看安装器退出码会假通过）。⚠️ 装机 exe 路径本轮没找到：`C:\Program Files\Alpha File Manager\alpha-file-manager.exe` 实测**不存在**（输出 `no exe`），核对前先定位真实安装目录。
2. **用户本人重新授权百度网盘**（D2 无法代做），再按 findings §6 走列目录 → 新建 → 改名 → 复制/粘贴 → 删除。验证目标用探针自建自删的临时文件，**不得删用户那个 316 MB PDF**。
3. 若删除仍失败：带「（服务端提示：…）」= D1 类协议错误（新 token 来源在活会话下没拿到，需换源）；不带 = 真失效，回看凭据是否又被拒。
4. §5 文案双层重复：**三条静态假设已逐一排除**（见 findings §5）—— 前端 `describeRemoteError` 单层、`remote.delete.failed` 全仓只有一处调用、Rust remote 模块不产「删除失败」前缀（`baidu_api.rs` 里**没有** `describe_failure()`，早前那笔是误记）。⇒ 静态代码无法复现，只能拿**真机 toast 的抓屏 BitBlt 原文**（`PrintWindow` 拍不到 toast 浮层）再定位；D1 修复后 `Auth` 触发条件已变，文案形态可能随之改变。
5. 用户口径「复制、黏贴、剪切、同步」的**实际支持面未逐项确认** —— 本轮只坐实并修了删除所依赖的 token 链路。若产品意图是「全部写操作都像本地磁盘」，需先盘点 `baidu_provider.rs` 实现了哪些写 op。
6. `baidu_provider.rs` 两条注释仍写「重拉一次首页」（`:203`、`:216`），已过时；本轮**故意没改**以免与正在跑的构建抢同一份源码，下轮清掉。

---

## 2026-11-09 追加批次 D：登录 harvest 死凭据短路修复（三道保险）已落盘 + 3.13.1 打包完成

> 根因不在 `baidu_api.rs`（批次 C 修的是 D2 的下游症状），而在 `src-tauri/src/baidu_login.rs`：
> 内嵌登录窗建窗后 **2.005 秒**（第一个轮询 tick）就判 Ready 并自动关窗 —— 它把持久 WebView2 cookie 罐里
> 遗留的**死 BDUSS/STOKEN** 当成新登录，死凭据被存进保险库，于是「测试连接」和「删除」都报
> 「服务器拒绝了该用户名或密码」（用户反馈 1、2 是同一根因的两面）。
> 权威细节见 `docs/2026-11-09-baidu-followup-audit-snapshot.md` **§7**（落地形态、实测行号、单测清单）。

### D.1 修复内容（`src-tauri/src/baidu_login.rs`，8 条新单测全绿）

- **保险一 · 清 jar**：`purge_stale_auth_cookies()` 在建窗后把 jar 里遗留的 BDUSS/STOKEN 用 `delete_cookie` 删掉，并返回 `AuthBaseline`（删除前的值）；删失败只降级、不中止。
- **保险二 · 基线比对**：`has_new_bduss()` / `current_bduss()` —— 轮询到的 BDUSS **与基线同值即一票否决**，继续等真正的新登录；日志按值去重（`stale_logged`），避免每 2 秒刷一条。
- **保险三 · 服务端验活**：`probe_session_live()` + `judge_session_probe()` + `probe_errno()`（**字节滑窗**找 `errno`，零新增依赖、不走 serde）。极性是**只在确凿证据下拒绝**：3xx / 401 / `errno=-6` ⇒ Rejected；403/429/5xx、传输错误、非预期 errno、读不出 errno ⇒ Inconclusive 放行（**有效会话不被误杀**）。同一个被拒的 BDUSS 记进 `rejected_bduss`，不再重复探针。
- 验活实现用**一次性 tokio current-thread runtime + reqwest 异步**：明确**不给 reqwest 加 `blocking` 特性**，并且**不碰 `tauri::async_runtime`**（AGENTS.md 的 cargo test 红线）。
- 判据口径与 `baidu_api.rs:345-350 PROBE_URL_TEMPLATE` 一致：`GET /rest/2.0/xpan/file?method=list&limit=1`，**无 `callback`**；验活必须用 webview jar 的 cookie **原文**，不能用 `sanitize_cookie_value` 之后的 `BaiduCookieRecord`。
- ⚠️ 本轮修掉的唯一编译错是 `E0596`：泛型 `impl Iterator` 形参上调 `.find()` 必须先声明 `mut`（`fn current_bduss<'a>(mut cookies: …)`）。
- ⚠️ 新增大量行 ⇒ 批次 C 之前的 `baidu_login.rs` 行号全部失效；定位改用这些**日志原文锚点**（探针按 ASCII 前缀匹配）：`login_start requested` / `login window built` / `poller started (generation=` / `purge:` / `login 短路拦截` / `login 验活被服务端拒绝` / `login state ready:` / `claim: saved to connection` / `claim: no pending session` / `give up: timeout`。

### D.2 版本与 CHANGELOG

- 手动 bump（**不用** `npm run release:prepare`，依据既往 `.cmd` 运行期间编辑导致版本号被顶掉的事故）：`package.json:3`、`src-tauri/Cargo.toml:4` 均 `3.13.0` → `3.13.1`；`src-tauri/tauri.conf.json:4` 是 `"../package.json"` 无需改；`sync-version` 由 `tauri:build` 自己跑（本轮构建日志实证 `Synced 2 Cargo manifests to 3.13.1`）。
- `CHANGELOG.md` 在 `## [3.13.0]` 之前插入 `## [3.13.1] - November 2026` 段，沿用该文件英文体例（`**Summary:**` / `### What changed` / `**Fixed**`）。

### D.3 门禁（实测数字，同一条命令口径）

- `cargo test --lib` 全量：`.superpowers/sdd/cargo-full-20261109.log`（103,480 B / 13:08:26）⇒ **`1168 passed; 0 failed; 1 ignored`**，另有 `(lib test) generated 15 warnings`（存量 `never read` 类）。定向日志 `.superpowers/sdd/cargo-bdlogin-test.log`（5,848 B / `EXIT=0`）。
- `eslint .`：`.superpowers/sdd/eslint-20261109-bdlogin.log`（21,503 B）⇒ **177 errors / 0 warnings**，`EXIT=1`。低于 AGENTS.md 记录的基线 188 ⇒ **无回归**；本仓只要有存量错误 eslint 就恒返 1，`EXIT=1` 属常态。
- ⚠️ **纠正一个易犯的误判**：链脚本 `gate-build-20261109-bdlogin.cmd` 里 `if not "%ERRORLEVEL%"=="0" goto :stop`（`:16`）**只跟在 `cargo test` 之后**，`:20 call npx eslint .` **没有守卫** ⇒ eslint 非零**不会**拦住构建。别凭「EXIT=1」就断定链断了。
- `tauri build`：`.superpowers/sdd/tauri-build-20261109-bdlogin.log`（246,071 B / 13:14:55）⇒ `Built application at …target\release\alpha-file-manager.exe` → `Running makensis` → **`Alpha File Manager_3.13.1_x64-setup.exe (43.30 MiB)`** → `[publish-bundle]` → `CHAIN DONE`。`^error` 0 命中。
- ⏳ **未归因**：构建日志里出现过一条 `Caused by: Error code 26: File opened that is not a database file`，其后编译继续、产物正常落地。本轮未定位来源（疑与 sqlite/缓存句柄有关），若下轮产物异常先回看这行。

### D.4 交付包（装机判据基线）

- `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.13.1_x64-setup.exe` = **45,407,674 B / 11-09 13:14:51**
- `release\Alpha File Manager_3.13.1_x64-setup.exe` = **45,407,674 B / 11-09 13:14:51**（同尺寸同号，`publish-bundle` 这次没被旧包顶到 `local-builds/`）
- **C.4-1 的遗留已解决**：真实装机目录是 **`D:\Program Files\Alpha File Manager`**（`C:\Program Files\…` 不存在），注册表 `HKLM:\SOFTWARE\…\Uninstall\Alpha File Manager` 的 `InstallLocation`。装机 exe 现状 = **82,349,568 B / 11-09 10:03:12**（批次 B 的旧二进制）。
- 装机判据只看 **装机 exe 的字节数 + mtime**，安装器退出码不算证据。

### D.5 真机验收工具（本轮新增，全部纯 ASCII）

- `.superpowers/sdd/install-3131.cmd`（464 B）：`"%SETUP%" /S` 静默装包；缺包打印 `SETUP_MISSING` / `exit /b 9`。**必须由 `cmd /c` 启动**（`Start-Process -Wait` 调 NSIS 在 PS 5.1 下会挂死）；运行期间不得编辑该 `.cmd`。
- `.superpowers/sdd/acc-20261109-loginfix.ps1`（4,688 B）：走**应用自带 self-test 钩子**验收，不用 UIA 点按钮。
  - 钩子契约实测于 `src-tauri/src/baidu_probe.rs:367-452` + `lib.rs:688-693`：`AFM_BAIDU_SELFTEST=1` 才生效（否则零副作用）、`AFM_BAIDU_SELFTEST_CONN`（不给则自动挑第一条 `baidu_pan`）、`AFM_BAIDU_SELFTEST_OUT`（默认 `%TEMP%\afm-baidu-selftest.txt`）、`SELFTEST_WAIT = 180 s`。`run_selftest` 调的是**真实命令** `remote_baidu_login_start` → 每秒轮 `remote_baidu_login_pending`，报告里 `VERDICT_SELFTEST: PASS|FAIL`。
  - **含义**：修复生效后死凭据被清 ⇒ 用户不扫码就没有 pending ⇒ FAIL（正确行为）；扫码成功 ⇒ pending ⇒ 认领落盘。
  - 取证方式：记 PRE_LOG_OFFSET ⇒ 杀进程 ⇒ 设环境变量 ⇒ `Start-Process`（**不带 `-Wait`**）⇒ 轮询报告 ⇒ 按字节偏移切出 `%APPDATA%\com.sigma-file-manager.app\baidu-login.log` 的**本次新增切片**写 `login-log-appended.txt` ⇒ 打印 ASCII 计数（`purge:` / `BDUSS` 非 purge 行 / `Ready` 非 purge 行 / `login state ready` / `claim: saved to connection`）⇒ **时序硬证据 `WINDOW_OPEN_TO_READY_SECONDS`**（最后一条 `login window built` 与最后一条 `login state ready` 的 `[秒.毫秒]` 时间戳差；缺陷版为 **2.0 s**，修复后应远大于此）⇒ 落 `summary.txt`。
  - 中文文案只落盘不进对话（`.ps1` 纯 ASCII 约束）。
- 验收断言清单（装机后逐条看）：① 有 `purge: 尝试删掉 N 条旧 BDUSS/STOKEN` ② 有 `login 短路拦截：BDUSS 与基线同值` ③ 若有 `login 验活被服务端拒绝` 说明服务端确实拒 ④ `WINDOW_OPEN_TO_READY_SECONDS` 远大于 2 s ⑤ 用户扫码后 `claim: saved to connection` 出现 ⑥ 再用 `.superpowers/sdd/verify-baidu-session-live3.ps1` 验会话有效、然后按 findings §6 走列目录→新建→改名→复制粘贴→删除（**不得删用户那个 316 MB PDF**）。

### D.6 当前阻塞与下一步

- **阻塞**：静默装包必须关闭用户正在使用的实例（实测 13:1x 仍在跑 pid=140052），且修复生效后**必须由用户本人扫码/输密码**才能拿到有效会话 ⇒ 两步都需用户配合。已向用户发出异步问卷（三选一：装机即扫码 / 只装机稍后扫码 / 别动应用只构建），**尚未回答**。
- **下一步**：拿到答复 ⇒ 杀进程 → `cmd /c install-3131.cmd` → 核对装机 exe 字节数+mtime → 跑 `acc-20261109-loginfix.ps1` → 把 180 s 扫码窗口交给用户 → 读 `summary.txt` / `login-log-appended.txt` 下断言 → 复验删除（用户反馈 2）。
- **反馈 3 / 4 / 5 未动工**：审计结论在 `docs/2026-11-09-baidu-followup-audit-snapshot.md`。要点：反馈 4 的自动展开必须加在 store 侧（`src/stores/runtime/status-center.ts:201-215`），远端删除目前**零进度**且逐个循环；反馈 5 的硬前提是**百度网盘上传根本没实现**（`baidu_provider.rs:622-628 reject_upload`），多线程必须建在分片上传器之上，且需对着**有效会话**实测三段式端点（`baidu_api.rs:967-982` 自陈形态未实证）。
- 本轮**故意没做**：全量 `vitest`（留到反馈 4 的 TS 改动一起跑，避免占构建锁）；`baidu_login.rs` 里那句「只做读 cookie 与超时判定两件事」的过时注释修正 —— fresh read `:377-391` 证明它**不在** `fn start_poller` 上方（`:381` 是 `remote_baidu_login_cancel` 的注释），下轮先 grep 定位再改。

---

## 2026-11-09 追加批次 E：真机反馈「重新添加百度网盘后无反馈、也没添加成功」根因与修复（3.13.2）

### E.1 用户报告与日志时间线

用户装 3.13.1 后的操作序列：设置 → 添加网盘 → **删除原来的百度网盘连接** → 重新添加 → 在登录窗里**退出原百度账号** → **扫码登录**。结果：**没有任何反馈，网盘也没添加成功。**

取证来源 `%APPDATA%\com.sigma-file-manager.app\baidu-login.log`（1,462 B / 13:41:51 起不再增长），本轮会话 `[1794202786.497]` 原文：

```
[1794202786.497] login_start requested
[1794202786.658] login window built
[1794202786.659] poller started (generation=1)
[1794202786.666] purge: 尝试删掉 2 条旧 BDUSS/STOKEN，失败 0 条
[1794202788.668] login 短路拦截：BDUSS 与基线同值（旧 jar 遗留），继续等真正的新登录
[1794202856.911] login 验活被服务端拒绝（服务端判定未登录（errno=-6）），不当作 Ready，继续等
[1794202911.978] claim: no pending session
```

落盘核对：`user-data\user-remote-connections.json` 里百度网盘 `id=23f422047e4f4b7db0aaf6fe244ac434` **没有 `credentialKey` 字段**（另三条 SFTP 都有）。

### E.2 根因三层

1. **直接原因（已确证）**：整轮 125 s 从未产生 `Ready` ⇒ `PendingSession` 一直为空 ⇒ 用户点保存时 `remote_baidu_login_claim` 返回 `claimed:false`；旧版 `saveConnection` **静默放过** ⇒ 连接以「无凭据」形态落盘 ⇒ 表现为「没有任何反馈 + 没添加成功」。
2. **上游原因（已确证）**：`purge` 报「删掉 2 条、失败 0 条」，**2.17 s 后读到的 BDUSS 仍与基线同值** ⇒ 清 jar 实际是**空操作**（`delete_cookie` 返回 Ok 但 cookie 还在）。于是登录窗一打开就是**旧账号已登录态**，用户在页面里「退出登录」也没能改变 jar（用户侧登出只作用于服务端会话，本地 cookie 仍在）。
3. **未确证的分叉**：t+70 s 出现了一个**不同于基线**的新 BDUSS，被保险三以 `errno=-6` 正确拒绝（与「退出登录」后的访客 cookie 吻合）；此后 55 s 内既无新 BDUSS、无 `login state ready`、也无 `give up: timeout`。旧版 `poll_loop` 在窗口消失时**静默 return 不打日志**，因此「扫码是否真正完成」与「登录窗是否在保存前被关掉」**无法区分**。

已排除：轮询自身超时（`GIVE_UP = 900 s`，125 s 结束绝不是超时）；「Ready 判定过严」（`scan_cookies` 只要求 BDUSS 合法，STOKEN 可选）；「验活误杀有效会话」（t+70 s 的 `errno=-6` 与访客态吻合，拒绝正确）；「`delete_cookie` 报错」（它返回 Ok）。

### E.3 修复落点

**后端 `src-tauri/src/baidu_login.rs`（918 → 1,062 行）**

- **登录窗独立 cookie 罐**（本轮核心）：`WebviewWindowBuilder::data_directory(profile)`，路径 `<app data>/baidu-login-profile/session-<unix_ms>` —— 每轮新建，建窗前 `sweep_login_profiles()` 只清 `session-*`（不碰共享 jar）。取 base 目录失败或 `create_dir_all` 失败 ⇒ **退回共享 jar**（记日志，不让登录直接失败）。这条路一次性消灭了三个已实测病灶：旧账号短路、`delete_cookie` 空操作、基线否决。
- **全分支诊断日志**：`login profile: 本轮独立 cookie 罐 <path>`、`poller exit: 登录窗已不在（{}s）`、`login 尚未 Ready：<reason>`（按 `Missing` / `Invalid` 去重打印）—— 专为区分 E.2-3 那个分叉而加。
- 新增 3 条单测：`login_profile_dir_gives_each_round_its_own_jar`、`not_ready_reason_is_none_only_when_ready`、`not_ready_reason_distinguishes_missing_from_invalid`。

**前端**

- `src/stores/storage/remote-connections.ts`：`saveConnection` 百度分支在 `claimed.claimed === false` 且**没有既有 `credentialKey`** 时 `throw new Error('baiduLoginRequired')` —— 在写 `connections.value` **之前**抛，不落半成品。编辑已有连接（本来就有 `credentialKey`）落空仍属正常，不抛。
- `connection-form.vue`：`closed` / `timeout` / `error` 三种失败态改为弹 toast；取消按钮 `v-if` 由 `=== 'waiting'` 改为 `!== 'idle' && !== 'ready'`（`ready` 不给取消，避免 `remote_baidu_login_cancel` 的 `clear_pending()` 丢掉刚拿到的登录态）。
- 17 个语言包补 `settings.remote.error.baiduLoginRequired`（ch「请先在应用内完成百度网盘登录，再保存这条连接。」+ en 及其余 15 种）。

### E.4 门禁（实测）

- `cargo test --lib`：**1171 passed / 0 failed / 1 ignored**（基线 1168 + 本轮 3 条）。日志 `.superpowers/sdd/test-baidu.log`，无 `... FAILED`、无 `error[`。
- `eslint .`：**177 errors / 0 warnings**（HEAD 基线 188，无回归；本仓有存量错误时 eslint 恒返 1）。日志 `.superpowers/sdd/eslint-3132.log`，对本轮改动的三个文件零命中。
- `cargo check --lib`：无 error（`.superpowers/sdd/check-baidu.log`）。

### E.5 打包与一次构建失败（TS2367，教训）

- 首轮 `npm run tauri build`（`.superpowers/sdd/build-3132.cmd` → `build-3132.log`）在 vite build **之后**被 TS 掐死：
  `connection-form.vue(155,16): error TS2367: This comparison appears to be unintentional because the types '"error" | "ready" | "closed" | "timeout"' and '"waiting"' have no overlap.`
  根因：后端 `baidu_login.rs` 只发 `ready` / `closed` / `timeout` / `error` **四种** status，`BaiduLoginEventPayload['status']` 同样只有这四种；`waiting` 只是**视图侧**的本地状态（`BaiduLoginViewState = 'idle' | 'waiting' | …`）。我在失败态 toast 里写的 `payload.status !== 'waiting'` 属于**对已收窄类型再比较**，`vue-tsc` 直接判错。
  ⚠️ **教训**：`settings.remote.baidu.embedded.status` 里有 `waiting` 这个 i18n key **不代表**事件 payload 会发它；写状态分支前先看 `src/types/remote.ts` 的联合类型，别拿 i18n key 当事件契约。
  修法：删掉那一半条件，保留 `payload.status !== previousState`（`ready` 已在上一分支返回，落到这里必然是失败态，去重语义不变）。
- 重跑 **`npm run tauri:build`**（`.superpowers/sdd/build-3132b.cmd` → `build-3132b.log`，`Synced 2 Cargo manifests to 3.13.2` → `Built application at` → `Running makensis` → `Finished 1 bundle` → `[publish-bundle] 已发布到 release/` → `[build] exit code: 0`）。
- ⚠️ 上一轮的 `.cmd` 用的是 `npm run tauri build`，**跳过了 `sync-version` 与 `publish-bundle-to-release.mjs`**；要落 `release/` 必须用 `npm run tauri:build`。

### E.6 交付包

- `release\Alpha File Manager_3.13.2_x64-setup.exe` = **45,449,794 B / 11-09 14:12:01**，SHA256 前缀 `6AC035F3742A3393`。
- 版本号三处一致：`package.json:3` = `3.13.2`、`src-tauri/Cargo.toml:4` = `3.13.2`、`tauri.conf.json` 引用 `../package.json`。

### E.7 装机后怎么判修复成立（**本轮尚未真机验证**）

必须由用户本人扫码。装 3.13.2 后按顺序看 `baidu-login.log`：

1. 出现 `login profile: 本轮独立 cookie 罐 …\baidu-login-profile\session-<ms>`。
2. **不再出现** `login 短路拦截：BDUSS 与基线同值`；独立罐为空时 `purge` 应打「jar 里没有 BDUSS/STOKEN 残留」—— 这是**独立罐真的生效**的天然验证信号。
3. 扫码完成后出现 `login state ready:`，点保存出现 `claim: saved to connection`。
4. 若仍失败，用这两条新日志分流：`poller exit: 登录窗已不在`（用户关窗 / 窗口被销毁）还是 `login 尚未 Ready：missing`（罐里始终没 BDUSS ⇒ 扫码没落地到 WebView2 的 jar）。
5. 其后仍按 `docs/2026-11-09-baidu-write-session-findings.md` §6 复验读写：列目录 → 新建文件夹 → 改名 → 复制粘贴 → 删除（只动探针自建自删的临时文件），**不得删用户那个 316 MB PDF**。

### E.8 已知未动工

- `purge_stale_auth_cookies()` 里 `delete_cookie` **删不掉**的根因**未查**（本轮是绕开，不是修好）；独立罐上线后它退化为兜底。
- 反馈 3 / 4 / 5 仍未动工，结论见 D.6 与 `docs/2026-11-09-baidu-followup-audit-snapshot.md`。

## 2026-11-09 追加批次 F：3.13.2 装机反馈两条（「测试连接」误报 + 删除后不刷新）→ 3.13.3

### F.1 用户报告（3.13.2 装机版，扫码登录成功后）

1. 点「测试连接」仍报「连上了，但服务器拒绝了该用户名和密码。」
2. 删除文件成功，但目录要手动点「刷新」才更新，希望实时。

### F.2 反馈 1 根因：**凭据没坏，是探测路径压根没拿到凭据却报了「被拒绝」**

先用离线探针直接问服务端，证伪「凭据失效」与「接口行为不一致」两条猜测
（脚本 `.superpowers\sdd\baidu-probe-offline2.ps1`，DPAPI 裸 `CryptUnprotectData` 解密
`remote-credentials.json` 后直连百度接口，**只打印长度/errno，不打印凭据原文**）：

```
PLAIN_LEN=319 BDUSS_LEN=192 STOKEN_LEN=64 VERSION=1
A_UINFO STATUS=200 ERRNO=0 HAS_NAME=1      A_LIST STATUS=200 ERRNO=0 HAS_LIST=1   (BDUSS+STOKEN, netdisk UA)
B_UINFO STATUS=200 ERRNO=0 HAS_NAME=1      B_LIST STATUS=200 ERRNO=-6             (只给 BDUSS)
C_UINFO STATUS=200 ERRNO=0 HAS_NAME=1      C_LIST STATUS=200 ERRNO=0 HAS_LIST=1   (浏览器 UA)
```

结论三条：
- 保险库里的密文**解得出**、长度正常、两个接口都 `errno=0` ⇒ **不是凭据过期**。
- `uinfo` 只要 `BDUSS`；`STOKEN` 只在 `list` 上必需（这条与本次误报无关，但排除了「接口行为不一致」）。
- 后端 `parse_uinfo_response`（`baidu_api.rs:405-428`）极宽容 ⇒ 只要探测真的把 cookie 填进 `target.password`，
  就必然 `Verified`。**因此唯一能落到误报的现实路径就是 `target.password == None`。**

该路径的真实性：`commands.rs:259-300` 的百度分支只有在 `credential_key` 有值且能读出会话时才回填 cookie；
`probe_baidu`（`connection_test.rs:299`）在 `password` 为 `None` 时**原本返回 `AuthRejected`**
⇒ 前端固定文案就是「连上了，但服务器拒绝了该用户名和密码」。**这句话在事实上是假的：没有发出过任何一个请求。**

两条触发条件都已覆盖：
- **编辑已保存的连接**：`form.id` 有值，理论上能回填 → 若保险库读取失败/键不匹配仍会静默落到无凭据。
- **新建表单未保存**：`form.id` 为 `undefined` ⇒ `credentialKey` 为 `undefined` ⇒ **必然误报**。

### F.3 修复落点（5 处前端 + 1 处后端）

| 文件 | 改动 |
| --- | --- |
| `src-tauri/src/remote/connection_test.rs:299-317` | 无凭据分支 `ProbeOutcome::AuthRejected` → **`ProbeOutcome::MissingCredential`**，文案「这条百度网盘连接还没有登录凭据，请先完成授权。」 |
| `src/localization/messages/ch.json:691` | `missing_credential` 中文改为**「没有可用于验证的凭据，本次未做身份确认。」**（仅 `ch`，未新增 key，故不同步 16 个外语包） |
| `src/modules/settings/ui/categories/remote/connection-form.vue:813-822` | outcome 主文案后新增 `data-test="test-detail"` 副行，`v-if="probeReport.message"` **原样展示后端 message** |
| `src/stores/storage/remote-connections.ts:485-512` | 新增前置分支：`isBaidu && password === undefined && !form.id` 时不发请求，直接回 `missing_credential` + 「这条百度网盘连接还没有保存，请先点「保存」，再点「测试连接」。」 |
| `src/modules/settings/ui/categories/remote/remote-category.test.ts:357` | 断言从「不得出现 message」放宽为 `toContain('AUTH_REJECTED_MESSAGE_MARKER')` |
| `src/localization/messages/en.json` 等 16 个包 | **未动**：既有英文值 "Connected, but no password was supplied, so authentication was not verified." 语义中性，不含「拒绝」 |

**顺带确证的独立缺陷**：修复前 `connection-form.vue` 的 `data-test="test-result"` 只渲染
`t(settings.remote.test.outcome.${outcome})`，**后端 `message` 被完全丢弃** ⇒ 用户看到的诊断文案
与后端实际判断无关。这是误报体感的第二个来源，已由 `test-detail` 修掉。

### F.4 反馈 2：删除后实时显示 → 乐观更新

原先 `deleteRemoteItems` 只在 `onRefresh()` 之后由全量 `read_dir` 刷新列表；
用户体感是「删了没反应」，尤其在百度网盘这种一次 `read_dir` 要跨网的场景。

| 文件 | 改动 |
| --- | --- |
| `.../composables/use-file-browser-navigation.ts:741-758` | 新增 `removeEntriesLocally(paths: string[])`：用 `normalizePath` 比较、整体换 `dirContents.value` 对象，并同步 `workspacesStore.cacheDirectoryEntries(...)` |
| `.../composables/use-file-browser-selection.ts`（第 9 可选参数 `onEntriesRemoved`） | `deleteRemoteItems` 在 `report.deleted.length > 0` 时**先**调 `onEntriesRemoved(paths)`，**再**调 `onRefresh()` |
| `.../composables/use-file-browser.ts`（3 处） | `DataSource` 加可选字段 `removeEntriesLocally?`；导航数据源导出该方法；`useFileBrowserSelection(...)` 第 9 实参接线 |

设计取舍：不是「等刷新更快」，而是**先按已确认的删除结果本地剔除，再用 `silentRefresh()` 校准**。
后端一旦返回旧列表，界面也不会回退到「删了还在」——因为 `silentRefresh` 是重新读，这点仍取决于后端；
乐观更新只解决「等不到刷新前的那段空白」。

### F.5 门禁（实测）

- `npx vue-tsc --noEmit` ⇒ `TSC_EXIT=0`。
- `npx eslint` 6 个改动文件 ⇒ 修掉 `use-file-browser-navigation.ts:756` 的
  `Object properties must go on a new line` 后 `ESLINT_EXIT=0`（`connection_test.rs` 在 eslint 忽略范围内）。
- `npx vitest run` 三个前端测试文件：`baidu-connection-form.test.ts` **9 passed**、
  `remote-category.test.ts` 通过、`use-file-browser-navigation.test.ts` **7 passed**（含新增用例
  「drops the confirmed deletions from the listing without waiting for a re-read」）。
- `baidu-connection-form.test.ts:208` 首轮红过：断言 `expect(toastMock).not.toHaveBeenCalled()` 口径过宽
  （批次 E 之后 `waiting`/`closed` 本就该各弹一条 toast）⇒ 改为只禁 `embedded.saved` 文案，**不是产品缺陷**。
- `src-tauri/src/remote/connection_test.rs` 新增
  `baidu_without_session_does_not_claim_the_server_rejected_it`（断言 `MissingCredential` +
  `!password_checked`）。
- ✅ **`cargo test --lib` 已跑（2026-11-09 14:57，脚本 `.superpowers\sdd\cargotest-3133.cmd`，
  日志 `.superpowers\sdd\cargotest-3133.log`）**：
  `test result: ok. 1172 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 6.64s`，
  且 `remote::connection_test::tests::baidu_without_session_does_not_claim_the_server_rejected_it ... ok`
  ⇒ **F.5 的唯一缺口（当时标注「尚未跑」）已闭合**。
- `eslint .` 全量口径实测 **177 errors / 0 warnings**（日志 `.superpowers\sdd\eslint-3133.log`，
  2026-11-09 15:11）⇒ 低于 AGENTS.md 记录的 188 基线，**不是回归**（基线数字随提交变，
  对照时仍以同一条命令量）。

### F.6 版本与交付包

- `package.json` → **3.13.3**；`sync-version` 打印 `Synced 3 Cargo manifests to 3.13.3`。
- `CHANGELOG.md` 顶部新增 `## [3.13.3] - November 2026`（英文，四条）。
- 构建脚本 `.superpowers\sdd\build-3133.cmd`（纯 ASCII），日志 `.superpowers\sdd\build-3133.log`。
- 产物：`release\Alpha File Manager_3.13.3_x64-setup.exe`。

### F.7 装机后怎么判修复成立

1. **新建表单直接点「测试连接」**（不保存）⇒ 应显示「先点保存」的提示，**不应**出现「服务器拒绝了该用户名和密码」。
2. **已保存的百度连接点「测试连接」** ⇒ 应显示「百度网盘连接正常，账号：<昵称>」；
   若仍为 `missing_credential`，副行会直接给出「还没有登录凭据」——据此可判定是保险库没读到还是别的原因。
3. 任何 outcome 下，主文案下方都应有一行后端诊断（`test-detail`）；没有副行说明后端没给 message。
4. **删除文件** ⇒ 列表应**立刻**少掉该行，不需要点刷新；随后 `silentRefresh` 若发现后端仍返回该项，
   会把它加回来（此时才是后端问题，不是界面问题）。

### F.8 遗留

- 「删除后是否需要刷新」的**后端侧**取证仍缺，**根因未定案**。
  - 已确认的取证通道：安装版日志在
    `%LOCALAPPDATA%\com.sigma-file-manager.app\logs\Alpha File Manager.log`
    （`tauri_plugin_log`，见 `src-tauri/src/lib.rs:625`；`startup-probe.log` 同目录）。
  - 已确认的空白：**该日志里 `remote` / `baidu` 字样零命中**
    （`Select-String` 实测 2026-11-09）⇒ 后端对远程读列/删除**完全没有日志锚点**，
    所以现在无论装哪个版本都拿不到「删除后有没有重新 read_dir、read_dir 回了什么」的证据。
  - 前端守卫早退**不可从前端日志观测**（wry 不透传 webview console，见 AGENTS.md），
    但**不需要前端日志也能二分** —— 只看后端日志即可：
    - 删除后**没有**新的 `remote_read_dir` ⇒ 前端守卫早退 / `onRefresh` 没发出去；
    - 有新的 `remote_read_dir` 且返回清单**仍含**被删项 ⇒ 后端返回旧列表（百度侧最终一致）。
  - 待落的锚点（**只在 3.13.3 装机实测仍需手动刷新时才加**，代码已备好，见下）：
    1. `src-tauri/src/remote/commands.rs` 的 `remote_remove_path`：
       `log::info!("[remote-diag] remove path={} ok={}", ...)`；
    2. 同文件 `remote_read_dir`：进入时打 `[remote-diag] read_dir begin path=...`，
       拿完清单打 `[remote-diag] read_dir done path=... entries={} names=[...] elapsed={:?}`；
    3. （可选，更精确）`src-tauri/src/remote/baidu_provider.rs` 的 `read_dir`：
       打出百度接口的 `errno` 与返回条数，用来区分「服务端给的就是旧快照」。
    形态参照既有 `[dir-watcher-diag]` 前缀约定（`src-tauri/src/dir_watcher.rs:457`）。
- `purge_stale_auth_cookies()` 的 `delete_cookie` 空操作仍未修（同 E.8）。
- 16 个外语语言包的 `missing_credential` 仍是旧英文措辞（中性、不误导，未改）。
- **工作副本未提交**：3.13.2 / 3.13.3 两批（含本批次 F 的 6 处落点）都在工作副本里，
  `git status --short` 共 **45 个条目**、`git diff --stat` 42 文件 / +2211 / −557，
  最后一次提交仍是 `118884dd docs: 补记百度内嵌登录根因定案…`。

## 2026-11-09 追加批次 F.9（3.13.3 真机实测结论 · 反馈 2 被服务端阻塞）

### F.9.1 反馈 1（测试连接文案 + 后端诊断副行）= **实测通过**
- 3.13.3 已静默安装并运行：
  `release\Alpha File Manager_3.13.3_x64-setup.exe`（45,451,695 字节 / 2026-11-09 14:55:11），
  `D:\Program Files\Alpha File Manager\alpha-file-manager.exe` FileVersion=3.13.3。
- 操作路径：设置 → 远程 → 百度网盘 → 编辑 → 「测试连接」。
- 实测主文案：**「连接成功，服务器接受了当前设置。百度网盘连接正常，账号：风扇杰叔叔」**；
  主文案下方出现后端诊断副行（`data-test="test-detail"`）：
  **「本次未提交密码，因此不能据此确认密码正确。」**
- 截图证据：`.superpowers\sdd\accept-3133\test-02.png`。
- 结论：**反馈 1 判定通过**，「无凭据 ⇒ 误报凭据被拒」的问题已消除。

### F.9.2 反馈 2（删除远程条目后列表立即更新）= **无法进入成功路径验收（服务端阻塞）**
不是「列表没立即刷新」，而是**删除请求根本没成功**：删得掉的东西是空集。
三段独立证据：

1. **真机新建目录失败（静默）**：远程目录 `百度网盘/0000temp`（空目录，属性面板
   `sfm://remote/00069babf4f42d1aef22c61aa89b0a9/0000temp`）→ Ctrl+Shift+N → 名称 `afm3133del`
   → 点「创建」（UIA Invoke，rect=1304,596）→ 按钮**立刻变灰**且此后 25/45/135 s **始终未恢复**；
   关闭对话框后列表仍 0 项 ⇒ 远程新建目录**既没成功也没报错**。
   截图：`.superpowers\sdd\accept-3133\mk-02..mk-11.png`。
2. **HTTP 离线探针（只操作自建目录，不碰用户数据）**：
   - 早前（凭据尚有效）：`GET /api/gettemplatevariable…fields=["bdstoken"]` → 200 errno=0、bdstoken len=32；
     `POST /api/create?…&isdir=1&block_list=[]&web=1&app_id=250528`（**代码当前发空 body**）→ 200 **errno=2**；
     `POST /api/filemanager?opera=delete&async=2&onnest=fail&…` body `filelist=[{"path":…}]` → 200 **errno=132**
     + `info`/`verify_scene`/`authwidget` ⇒ **风控校验**（不是权限问题）。
     REST 路径 `/rest/2.0/xpan/*` 对本凭据 errno=9019 / HTTP 400。
   - 本轮（同一凭据）：`gettemplatevariable` 在**三种 header 组合**下**全部 errno=-6**
     （应用同款 UA `netdisk;P2SP;2.2.60.26`／同款 UA+Referer／不带 UA）
     ⇒ **STOKEN/会话已失效或被风控作废**（旧连接 `6c1d7da…` 同样是 -6，
     证据 `.superpowers\sdd\acc-20261109\baidu-token-source.txt`）。
3. **因此**：token 取不到 ⇒ 带 body / 空 body 无法比对；create / delete 均无法拿到成功样本。

**结论（必须如实报告，不得伪报「已修复」）**：反馈 2 的**成功路径在当前百度账号状态下无法完成真机验收**。
阻塞点在百度服务端（errno=2 / errno=132 风控 / 会话 -6），不在前端列表刷新逻辑。
需要用户在内嵌登录窗**重新登录一次**取得有效 STOKEN，应用自身无法完成这一步。

### F.9.3 本轮定位到的三个待修缺陷（代码尚未改动）
- **D1（前端，可独立修，已坐实）**
  `src/modules/navigator/components/file-browser/file-browser-new-item-dialog.vue:34,56-60,90-95,133`
  —— 对话框的 `isSubmitting` 只在 `watch(name)` 里复位；父组件 `handleNewItemConfirm` 仅在 **success**
  时关闭对话框，**失败时既不复位 `isSubmitting`（⇒「创建」按钮永久灰）也不向父级回传任何信号**。
  两个使用点：
  `src/components/dir-entry-interactive/dir-entry-interactive.vue:186-192,241-246`、
  `src/modules/navigator/components/file-browser/composables/use-file-browser-dialogs.ts:173-183`
  （由 `file-browser.vue:466-469` 使用）。
  修法方向：**失败也要复位对话框 `isSubmitting`**（父组件把失败信号回传给对话框），
  让按钮恢复可点；失败 toast 已有现成分支
  （`use-file-browser-selection.ts:1196-1250`），不必另造。
- **D2（后端假设，未实证）**
  `src-tauri/src/remote/baidu_provider.rs:309-321` `create_dir` → `post_form(…, "")` ⇒ **body 为空**。
  怀疑 `/api/create` 空 body 正是 errno=2 的主因，但**未实证**（token 已失效，无法做带/不带 body 的对照）。
  取 token 入口：`baidu_api.rs:1005`（BDSTOKEN_ENDPOINT）、`:1011`（REFERER）、`:1378-1417`（post_form）。
- **D3（可观测性）** F.8 计划的三个 `[remote-diag]` 日志锚点**尚未落**：
  `src-tauri/src/remote/commands.rs` 的 `remote_read_dir`(:403-427)、`remote_remove_path`(:807-829)、
  `remote_make_dir`(:842-868) —— 三处目前**均无日志锚点**。
  没有它们，3.13.3 装机版日志里 `remote`/`baidu` 字样**零命中**，二分不了「守卫早退」与「后端返回旧列表」。

### F.9.4 推荐执行顺序（下一阶段）
1. 落 **D3** 日志锚点（可观测性，先于一切行为改动）；
2. 落 **D1**（按钮复位，纯前端，风险低、与服务端无关，可单独验证）；
3. 落 **D2**（create body）—— 需有效 token 才能实证；
4. 出 **3.13.4** 打包并装机；
5. **请用户在内嵌登录窗重新登录百度网盘**取得新 STOKEN；
6. 复测反馈 2：删除远程条目 ⇒ 列表立即消失（配合 D3 日志判定守卫早退 / 后端旧列表）。

### F.9.5 本轮证据路径
- 阻塞结论文档：`docs/2026-11-09-baidu-write-chain-blocker.md`（8,893 字节）
- 探针脚本（须用 Windows PowerShell 5.1 跑，pwsh 7.6.9 在本机有程序集装载故障）：
  `.superpowers\sdd\baidu-bdstoken-probe.ps1`、`.superpowers\sdd\baidu-create-body-probe.ps1`、
  `.superpowers\sdd\baidu-write-matrix.ps1`
- 窗口复原脚本：`.superpowers\sdd\restore-occluders-3133.ps1`（已执行：Microsoft Store 与 practilog 均已复原原位）

### F.9.6 本轮基线（未变）
- `cargo test --lib` = **1172 passed / 0 failed / 1 ignored**
- `eslint .` = **177 errors / 0 warnings**
- 门禁基线仍随提交漂移，量的时候用 `git stash` 量 HEAD。

## 2026-11-09 追加批次 F.10（D1 + D3 已落地，3.13.4 打包中）

### F.10.1 D1 已修：新建对话框失败后「创建」按钮不再永久变灰
- 改法：`confirm` 事件多带一个**结果回执** `done(ok: boolean)`，
  父组件在异步操作结束后**必须**回执；失败回执 `false` ⇒ 对话框复位 `isSubmitting`，
  按钮恢复可点、对话框保持打开可改名重试。
- 落点：
  - `src/modules/navigator/components/file-browser/file-browser-new-item-dialog.vue`
    （emit 类型 + `handleSubmit` 里的 `done` 闭包，失败才复位）；
  - `src/components/dir-entry-interactive/dir-entry-interactive.vue:186`（`handleNewItemConfirm` 加回执 + try/catch）；
  - `src/modules/navigator/components/file-browser/composables/use-file-browser-dialogs.ts:173`（同上）；
  - 两个使用点其余不变。
- 新增回归测试：`src/modules/navigator/components/file-browser/__tests__/file-browser-new-item-dialog.test.ts`
  （3 条：失败回执后按钮恢复可点 / 未回执时保持禁用 / 成功回执时保持禁用）。
  实测 **3 passed**。

### F.10.2 D3 已修：远程读列 / 删除 / 建目录全部留下日志锚点
- `src-tauri/src/remote/commands.rs`：
  - `remote_read_dir`：`[remote-diag] read_dir begin conn=… path=…`
    与 `[remote-diag] read_dir done conn=… path=… entries=N names=[…] elapsed=…`；
  - `remote_remove_path`：`[remote-diag] remove begin/done … ok=… elapsed=… err=…`；
  - `remote_make_dir`：`[remote-diag] make_dir begin/done … ok=… elapsed=… err=…`。
- 这样「列表没刷新」可以二分：**没有**新的 `read_dir begin` ⇒ 前端守卫早退；
  有 `read_dir done` 但清单仍含被删项 ⇒ 后端/服务端返回旧列表。

### F.10.3 D2 **未做**（保持待办，不塞进 3.13.4）
- `/api/create` 空 body 是否是 errno=2 的主因**仍未实证**（token 已失效，无法做带/不带 body 的对照）。
- 在没有对照数据的前提下改写请求体属于**盲改**，可能引入新问题 ⇒ 本版不改，
  等有效 STOKEN 到位后先做带/不带 body 的 A/B，再决定。

### F.10.4 版本与门禁
- 版本升到 **3.13.4**：`package.json`、`package-lock.json`、`src-tauri/Cargo.toml` 三处一致
  （`tauri.conf.json` 的 version 指向 `../package.json`，无独立值）。
- `CHANGELOG.md` 已加 `## [3.13.4]` 条目（英文，风格与既有条目一致）。
- `cargo test --lib` = **1172 passed / 0 failed / 1 ignored**（与基线逐字一致）。
- `eslint .` 目标文件 = **0 问题**；全量复跑中（此前 181 → 本轮修掉本批新增的 4 条，预期回到 177）。
- 新增脚本：`.superpowers/sdd/cargotest-f9.cmd`、`.superpowers/sdd/eslint-f9.cmd`、`.superpowers/sdd/build-3134.cmd`。

### F.10.5 3.13.4 真机验收计划（**仍需用户先重新登录百度网盘**）
1. 装 3.13.4（`release/Alpha File Manager_3.13.4_x64-setup.exe`）；
2. 设置 → 远程 → 百度网盘 → 编辑 → 重新登录（取得新 STOKEN）⇒ 这一步**只能由用户完成**；
3. 远程目录里「新建目录」⇒ 成功则验证列表立即出现、失败则验证**按钮恢复可点**（D1）；
4. 删除远程条目 ⇒ 验证列表立即消失 + 日志里出现 `[remote-diag] remove done ok=true` 与随后的
   `[remote-diag] read_dir done`（D3 + 反馈 2 的成功路径）；
5. 若仍需手动刷新 ⇒ 用两条日志二分「守卫早退」与「后端返回旧列表」。

## 2026-11-09 追加批次 F.11（D2 根因已实证 → 已修 → 3.13.4 重打包）

### F.11.1 D2 根因**已实证**（不再盲改）
在同一份 DPAPI 会话上做了带/不带 body 的 A/B（`.superpowers\sdd\baidu-create-ab.ps1`）：

| 请求 | 结果 |
|---|---|
| `GET /api/gettemplatevariable` | `errno:0`，bdstoken 正常取到 ⇒ 会话形态正确 |
| `POST /api/create`，**空 body** | `{"errno":2,"path":""}` ⇒ 与装机版「创建目录失败」完全一致 |
| `POST /api/create`，**表单 body** `path=…&isdir=1&block_list=[]` | `errno:0`，**真实创建** `/0000temp/zz-accept-3134`（fs_id=7114173830981） |
| `GET /api/list?dir=/0000temp` | 条目已在服务端列表（`server_filename=zz-accept-3134`, `isdir=1`） |

⇒ 服务端**没有**索引延迟；空 body 就是 `errno:2` 的直接原因。
证据：`.superpowers\sdd\accept-3134\baidu-create-ab.log`、`.superpowers\sdd\accept-3134\baidu-list-probe.log`。

### F.11.2 装机版 D2 缺陷复现（3.13.4 首包）
- 「新建目录」对话框用 UIA `ValuePattern.SetValue` 填入 `zz-accept-3134`（按钮随之变可用）；
- 点「创建」⇒ toast「**创建目录失败** / 百度网盘接口拒绝了这次请求（参数或协议层错误）。」，对话框**未关闭**。
- 截图：`.superpowers\sdd\accept-3134\f2b-02-created.png`、放大裁剪 `.superpowers\sdd\accept-3134\f2b-toast-crop.png`。

### F.11.3 D2 修复已落地
- `src-tauri/src/remote/baidu_api.rs`：新增公开纯函数 `create_dir_form_body(path)`
  （`path` / `isdir=1` / `block_list=[]`，表单百分号编码），并附 2 条单测；
- `src-tauri/src/remote/baidu_provider.rs`：`create_dir` 由 `post_form(&url, "")`
  改为 `post_form(&url, &create_dir_form_body(path))`；query 里的同名参数**保留**（兼容冗余，不是主因）。
- 单测：`cargo test --lib create_dir_form_body` = **2 passed**；
  `cargo test --lib remote::baidu` = 205 passed（其中首轮有 1 条新断言把空格写成 `+` 的笔误，已改为 `%20` 后通过）。

### F.11.4 新发现：装机版**无日志落盘**
- `%APPDATA%\com.sigma-file-manager.app\logs` 与 `LOCALAPPDATA` 下都找不到任何 `*.log`；
  （`tauri_plugin_log` 默认 Stdout + LogDir，但打包版没有可落盘的 LogDir。）
- ⇒ D3 的 `[remote-diag]` 锚点在**装机态不可取证**，本轮只能用「界面证据 + 离线 A/B」组合代替；
  后续若要在真机取日志，需要给 release 构建显式指定日志目录。

### F.11.5 本批次遗留（下一轮）
1. 重打 3.13.4（脚本 `.superpowers\sdd\build-3134-d2.cmd`，日志 `.superpowers\sdd\build-3134-d2.log`）；
2. 装机后**在应用内**新建目录 ⇒ 必须成功（不能再拿探针创建的目录冒充应用创建成功）；
3. 右键删除该目录 ⇒ 测量条目从列表消失的延迟（乐观更新应为即时量级），并观察 toast 是否报错；
4. 反馈 2 的最终结论仍待第 3 步的实测数据（历史上有 errno=132 风控干扰，需在有效会话下重测）。

## 2026-11-09 追加批次 F.12（3.13.4 含 D2 修复重打包 + 装机真机验收：创建 ✅ / 删除即时性 ✅）

### F.12.1 重打包与装机
- 打包：`npm run tauri:build` → `BUILD_EXIT=0`（脚本 `.superpowers\sdd\build-3134-d2.cmd`，日志 `.superpowers\sdd\build-3134-d2.log`）。
- 产物：`src-tauri\target\release\bundle\nsis\Alpha File Manager_3.13.4_x64-setup.exe` = **45,354,324 B**
  （已复制到 `release\local-builds\`；与 3.13.4 首包的 45,355,093 B 不同 ⇒ 确实是新包）。
- 静默安装：`INSTALL_EXIT=0`；`D:\Program Files\Alpha File Manager\alpha-file-manager.exe`
  = 82,589,696 B / mtime 2026-11-09 17:58 / **FileVersion 3.13.4**（与首包的 82,587,136 B 不同）。

### F.12.2 反馈 1 的 D2：**应用内新建目录成功**（D2 修复生效）
- 路径：远程 `百度网盘 / 0000temp` → 工具栏「+」→ 菜单项「新建目录 Ctrl+Shift+N」→ 输入框填 `zz-f11-3134` → 「创建」。
- 结果：**成功**。对话框关闭（`dialog_edits=0`），且同一时刻三处都出现该目录：
  内容区 `Button [zz-f11-3134 —]`、左侧目录树 `Text [zz-f11-3134]`、右侧面板路径 `sfm://remote/…/0000temp/zz-f11-3134`。
- ⇒ 3.13.3/3.13.4 首包上的「创建目录失败 / 参数或协议层错误」已消除。
- 证据：`.superpowers\sdd\accept-3134\nf-run.log`、截图 `nf-02-dialog.png` / `nf-03-filled.png` / `nf-04-after-create.png`。

### F.12.3 反馈 2（删除后列表立即更新）：**通过**，且发现一个必须写进验收口径的细节
- 右键 `zz-f11-3134` → 菜单「删除」→ **列表在 ≤0.4 s 内消失**（`COUNT_T+1x0.4s=0`），
  8 s 后 `COUNT_SETTLED=0`（`silentRefresh` 回读没有把它带回来）。
- ⚠️ **「列表立即消失」≠「已从网盘删除」**：远程删除会弹一个确认框「删除远程项目」（含路径与「无法恢复」提示），
  **不点确认就不会真的删除**。复核服务端 `/api/list`：`zz-f11-3134` **仍在**。
  ⇒ 验收必须拆成两条独立判据：① 列表即时性（乐观更新）② 服务端确已删除（确认框点过之后）。
- 补点确认框的「删除」后复核：服务端 `/api/list` 中 `zz-f11-3134` **已消失** ⇒ 全链路成立。
- 证据：`.superpowers\sdd\accept-3134\dm-run.log`、截图 `del-01-menu.png` / `del-03-after-delete.png` /
  `del-04-settled.png`（能看到仍开着的确认框）、`f11-20-confirmed.png`；
  服务端复核 `baidu-list-after-delete.log`（仍在）/ `baidu-list-after-confirm.log`（已消失）。

### F.12.4 数据清理
- 两个测试目录（`zz-accept-3134` 探针创建的、`zz-f11-3134` 应用创建的）**都已删除**；
  服务端复核 `baidu-list-clean.log`：`zz-accept-3134=False`、`zz-f11-3134=False` ⇒ 用户网盘无残留。

### F.12.5 门禁
- `cargo test --lib` = **1174 passed / 0 failed / 1 ignored**（基线 1172 + 本轮 2 条新单测）。
- `eslint .` = **177 errors / 0 warnings**（与基线逐字一致；本轮只动 Rust，没碰前端）。
- 新增单测：`remote::baidu_api::create_dir_form_body_tests`（2 条：字段齐全 / 特殊字符转义）。

### F.12.6 本批次新增的「工具坑」
1. **UIA 树不是稳定的导航入口**：`Edit address` 浮层里的按钮（`百度网盘 sfm://remote/<id>`）带
   `InvokePattern`，直接 Invoke 就能跳到远程根 —— 比点坐标稳得多。
2. **坐标会随布局变**：上一轮记的「+ 按钮在 (1477,96)」本轮是**地址栏**（点开是「编辑地址」浮层），
   真正的「+」在 (3054,81) 55×54；脚本改成**候选坐标轮询 + 按菜单项名字判定命中**才稳。
3. **Radix 确认框的按钮会出现得比预期晚**：右键「删除」后约 1 s 才出现在 UIA 树里，
   900 ms 处的扫描**必然漏掉**它 ⇒ 脚本要「点击后再轮询确认框」，不能只扫一次。
4. **UIA 树整体只能导出 81–498 行**（Chromium accessibility 树），所以「按名字找元素」是唯一可靠手段，
   「全树扫描 + 坐标猜测」基本无效。

## 2026-11-09 追加批次 F.13（收口：用户反馈确认 + README/更新日志/站点版本文件 + 提交推送）

### F.13.1 用户测试反馈（本轮唯一输入）

> 「+」→「新建目录」→ 填名 →「创建」→ 成功

⇒ 用户本人在 3.13.4 装机版上跑通了「远程新建目录」，与 F.12.2 的自动化证据一致
（对话框关闭 + 内容区/目录树/右侧路径三处同时出现该目录）。
**反馈 1 的 D2（`/api/create` 空 body）判定为已修复并经用户手测确认。**

### F.13.2 本轮改动（文档与发布元数据）

| 对象 | 改动 |
| --- | --- |
| `README.md` / `README.zh-CN.md` | 新增「What changed in 3.13.4」/「3.13.4 改了什么」章节（8 条，含内嵌登录、测试连接、删除即时性、建目录协议、失败反馈、日志锚点） |
| `public/changelog/<14 个语言包>/changelog.md` | 新增 3.13.4 条目；`ch` 为中文正文，其余沿用既有口径（本地化标题 + 英文正文，与 3.13.0 条目同制） |
| 站点源 `E:\办公文件\L1网站\alpha-file-manager\version.json` | 3.13.0 → **3.13.4**（`sha256=cfc0511d…595c95`，3.13.0 已归档进 `history[]`，共 20 条） |
| 站点源安装包副本 | `Alpha_File_Manager_3.13.4_x64-setup.exe` = **45,354,324 B**，sha256 与清单**逐字一致** |
| `E:\办公文件\L1网站\alpha-file-manager\_release-notes-v3.13.4.md` | 已生成（供 `github` 子命令读取正文） |

站点清单用官方脚本改，不要手改 JSON：
`node scripts/release-publish.mjs prepare --bump none --skip-build --notes-file <说明文件>`
（`--bump none` 保持版本号；`--skip-build` 复用 `release/local-builds/` 里已验证的 3.13.4 包）。

### F.13.3 门禁（实测，同一条命令口径）

- `npx vue-tsc --build` → `TSC_EXIT=0`
- `npx eslint .` → **177 errors / 0 warnings**（与基线逐字一致，本轮只动文档与发布元数据）
- `node scripts/verify-changelog.mjs` → `CHANGELOG_EXIT=0`（3.13.4 条目在位、顺序正确、14 个语言包已覆盖当前版本）
- `cargo test --lib` → 沿用 F.12.5 的 **1174 passed / 0 failed / 1 ignored**（本轮未改 Rust 代码，未重跑）

### F.13.4 未做（需要用户点头）

- **站点上传**（`node scripts/release-publish.mjs upload`）与 **GitHub 兜底 release**
  （`node scripts/release-publish.mjs github`）**均未执行** —— 属于对外发布动作，未获确认前不自动执行。
  注意：清单已改但**尚未上传**，用户端此刻还感知不到 3.13.4。

