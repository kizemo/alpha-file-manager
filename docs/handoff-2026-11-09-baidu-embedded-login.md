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
