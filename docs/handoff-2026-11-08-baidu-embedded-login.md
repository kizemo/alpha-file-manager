# 交接：百度网盘内嵌 WebView2 登录（feat/baidu-embedded-login）

- 生成时间：2026-11-08 深夜
- 仓库：`F:\soft\00selfmade\filemanager\sigma-file-manager`
- 分支：`feat/baidu-embedded-login`（HEAD `5c6e21e3`，未提交的工作区改动即本轮成果）
- 上游入口文档：`docs/handoff-2026-11-08-release-3.13.0-locations-partitions.md`
- 约定入口：`F:\soft\00selfmade\filemanager\AGENTS.md`

> 一句话现状：**代码全部写完、门禁全绿、release 包已打出并静默安装成功、应用已在本机运行**；
> 唯一未完成的是**真机界面验收**与**合并回 main**。界面验收卡在「找不到设置页入口」这一个纯自动化操作问题上，与产品代码无关。

---

## §1 目标与已锁定的决策

用户原始要求（逐字要点）：

1. 在仓库里实现「内嵌 WebView2 登录百度网盘 → Rust 侧 `cookies_for_url()` 读回登录态 → 自动填进连接表单」这套登录设置流程；
2. 建立分支做这件事；
3. **测试成功后合并回主干 `main`**。

已锁定、**不要重新讨论**的决策：

| 项 | 决定 |
|---|---|
| 方案 | 方案 **a**（内嵌 WebView2），b/c/d/f 已否决 |
| 登录窗归属 | Rust 侧 `WebviewWindowBuilder` 创建，label 固定 `afm-baidu-login`，**不改** `tauri.conf.json` capabilities |
| 起始页 | `https://pan.baidu.com/disk/main`（`STOKEN` 只在该页下发） |
| cookie 锚点 | `https://pan.baidu.com/` |
| 事件名 | `afm://baidu-login`（`tauri://` 前缀被 Tauri 保留，不能用） |
| 命令形态 | **四条命令全部同步**（不做 async / `spawn_blocking`），为规避 `cargo test --lib` 的 tao 链接红线 |
| 登录态存放 | 先进模块级 `Mutex<Option<PendingSession>>`；**认领动作放在 `saveConnection` 里**（只有那步才有 connection id 可落盘） |
| 降级路径 | 保留现有「手工粘贴 Cookie」入口，注释改口径为「降级路径」 |
| 昵称 | **本轮不取**（需 async + reqwest），payload 无 `nickname` 字段 |
| CHANGELOG | **本轮不改**。`git tag --list 'v3.13*'` 含 `v3.13.0` ⇒ 该版本已发布；`scripts/verify-changelog.mjs` 只认 `## [x.y.z] - <date>` 且要求数字版本降序，插 `## [Unreleased]` 会让 `Number('Unreleased')` 变 NaN 把排序校验搞红 |

安全约束（硬性）：

- 不得挪用第三方 OAuth 的 `client_id` / `secret`；
- cookie 明文不进日志、不进错误信息、不进前端、不进 URL、不落明文盘、不经本地端口回传；
- 前端只拿「状态 / 摘要 / 过期时间」；
- 探针那套「页面 fetch → `<img>` 回传 127.0.0.1」的诊断通道**不得**搬进产品代码。

---

## §2 已完成的代码改动（逐文件，全部已落盘）

### 2.1 新增 `src-tauri/src/baidu_login.rs`

分层原则：纯逻辑留在 `src-tauri/src/remote/baidu_auth.rs`（已存在，可单测、不碰 `tauri::`）；
本文件是 **tauri 适配层**，参照 `baidu_probe.rs` 的形态 —— 只在 `lib.rs` 的 `mod` 与
`generate_handler!` 被引用，模块内**没有** `#[cfg(test)]`，不进 `remote/` 的声明链。

关键常量与结构：

| 位置 | 内容 |
|---|---|
| `:34` | `LOGIN_WINDOW_LABEL = "afm-baidu-login"` |
| — | `LOGIN_START_URL = "https://pan.baidu.com/disk/main"`、`LOGIN_COOKIE_URL = "https://pan.baidu.com/"`、`LOGIN_EVENT = "afm://baidu-login"`、`STOKEN_NAME = "STOKEN"` |
| — | 轮询 `POLL = 2s` / 放弃 `GIVE_UP = 900s` / 等 STOKEN `STOKEN_WAIT = 20s` |
| `:169` / `:170` | 窗口 `.title("百度网盘登录")`、`.inner_size(1100.0, 820.0)` |
| — | `PendingSession { state, has_stoken }` + `OnceLock<Mutex<...>>`（锁中毒走 `poisoned.into_inner()`）+ `static SETTLED: AtomicBool` |
| `:91` | `BaiduLoginEvent { status: &'static str /* ready\|closed\|timeout\|error */, has_stoken, summary, message }`，`rename_all = "camelCase"` |
| `:132` | `emit` |

四条**同步**命令：

- `remote_baidu_login_start`（`:155`）—— 已有窗口则 `get_webview_window` 判存在后 `set_focus()`，不重复建窗；
- `remote_baidu_login_pending`（`:193`）—— 查待认领状态，供前端进页面时回填；
- `remote_baidu_login_claim(app, connection_id)`（`:211`）—— 认领并落盘；**落盘失败会把 `PendingSession` 放回槽位**（`:231–235`），槽位为空返回 `claimed:false` 属正常空操作；
- `remote_baidu_login_cancel`（`:245`）—— 取消等待。

`spawn_poller`（`:251`）每 2s 一轮：判窗口存活 → 判超时 → `cookies_for_url` → `max_age().whole_seconds()`
换算过期时间 → `scan_cookies`。

`on_window_event` 用 `SETTLED.swap(false, SeqCst)` 判「用户手动关窗」并回 `closed` 事件。

> ⚠️ 关键约束：`cookies_for_url()` 在 Windows 上于**同步 command / 事件回调**里调用会**死锁**，
> 所以必须留在独立线程里轮询 —— 不要「优化」成在命令里直接调。

### 2.2 `src-tauri/src/lib.rs`（LF 行尾，勿转 CRLF）

- 第 10 行 `mod baidu_login;`（放在 `mod baidu_probe;` 之前）；
- `generate_handler!` 中 377–380 行注册上述四条命令。

### 2.3 `src/types/remote.ts`

追加两个类型：

- `BaiduLoginClaimPayload { claimed: boolean; summary: string }`
- `BaiduLoginEventPayload { status: 'ready'|'closed'|'timeout'|'error'; hasStoken: boolean; summary: string | null; message: string | null }`

### 2.4 `src/stores/storage/remote-connections.ts`

- 第 20 行 import 增补 `BaiduLoginClaimPayload`；
- `saveConnection`（`:307`）的百度分支新增 `else`（**341–353 行**）：调
  `invoke<BaiduLoginClaimPayload>('remote_baidu_login_claim', { connectionId: id })`，
  `claimed` 为真时把 `credentialKey` 置为 `id`；
- 新增 `startBaiduLogin()`（`:538`）/ `cancelBaiduLogin()`（`:543`），`:566–567` 导出。

### 2.5 `src/modules/settings/ui/categories/remote/connection-form.vue`

模板实测位于 `:600–654`，脚本部分：

- `import { computed, onMounted, onUnmounted, ref, watch } from 'vue'`；
- `import { listen, type UnlistenFn } from '@tauri-apps/api/event'`；
- `BaiduLoginViewState` 类型、`baiduLoginState` / `baiduLoginSummary` / `isStartingBaiduLogin` 三个 ref；
- `handleStartBaiduLogin()` / `handleCancelBaiduLogin()`；
- `onMounted` 内 `listen<BaiduLoginEventPayload>('afm://baidu-login', ...)`，含 **disposed 晚到兜底**（卸载后才到达的事件不写状态）；
- `onUnmounted` 退订；
- `reset()` 末尾把两个 ref 归零。

模板锚点（验收就靠这些 `data-test`）：

| `data-test` | 行 | 说明 |
|---|---|---|
| `baidu-embedded` | `:608` | 内嵌登录区块容器 |
| `baidu-embedded-start` | `:615` | 按钮，文本 = `settings.remote.baidu.embedded.start` |
| `baidu-embedded-cancel` | `:625` | 仅 `waiting` 态出现 |
| `baidu-embedded-hint` | `:633` | 说明文案 |
| `baidu-embedded-status` | `:641` | 状态行，`:data-status` 暴露状态给测试与样式 |
| `baidu-embedded-summary` | `:650` | 摘要 |
| `baidu-cookie-help` / `-toggle` | `:684` / `:692` | 降级路径：Cookie 帮助 |
| `baidu-open-login` | `:702` | 降级路径：打开浏览器登录 |

### 2.6 i18n（17 个语言包）

- `en.json` 在 `settings.remote.baidu` 内、`cookiePlaceholder` 之后新增
  `embedded { title, start, cancel, hint, status { waiting, ready, closed, timeout, error } }`；
- `node scripts/sync-i18n.js` 同步 17 个包；
- `ch.json` 值已中文化（校验 `U+FFFD` 计数 = 0）：

| key | 中文值 |
|---|---|
| `embedded.title` | 在应用内登录 |
| `embedded.start` | 用内嵌窗口登录 |
| `embedded.cancel` | 停止等待 |
| `embedded.hint` | 在应用内打开登录窗口。登录完成后这张表单会自动填好——你只需给连接起个名字并保存。 |
| `status.waiting` | 正等你完成登录… |
| `status.ready` | 已登录。给这个连接起个名字并保存——会话会在保存时写入。 |
| `status.closed` | 登录还没完成，登录窗口就被关掉了。 |
| `status.timeout` | 登录等待超时了，请重试。 |
| `status.error` | 读不到登录态，请改用 Cookie 输入框。 |

---

## §3 门禁结果（本轮实测）

| 门禁 | 命令 | 结果 |
|---|---|---|
| 类型 | `npx vue-tsc --build` | `TSC_EXIT=0` |
| lint（整仓口径） | `npx eslint .` | **179 errors / 0 warnings**（低于 2026-10-23 记的 188 基线，无回归） |
| lint（改动文件） | `npx eslint <各改动文件>` | `ESLINT_FILE_EXIT=0` |
| CHANGELOG 校验 | `node scripts/verify-changelog.mjs` | `CL_EXIT=0`（版本 3.13.0，49 条目） |
| 定向单测 | 见下 | **4 files / 56 tests passed**（12.55s） |
| 全量单测 | `npx vitest run` | 本轮启动，结果见 `.superpowers/sdd/vitest-full.log` 尾部 `VITEST_EXIT=` |
| Rust 单测可加载性 | `cargo test --lib` | **本轮未跑**，见 §7 未验证项 |

定向单测命令：

```
npx vitest run src/modules/settings/ui/categories/remote/baidu-connection-form.test.ts ^
  src/stores/storage/__tests__/remote-connections-baidu.test.ts ^
  src/stores/storage/__tests__/remote-connections.test.ts ^
  src/types/__tests__/remote-protocol-baidu.test.ts
```

> `npx eslint .` 的口径必须一致：本项目 npm 脚本是 `eslint .`（整仓）。**不要**拿 `eslint src` 的数字对 baseline。
> 另：eslint 不读 `.gitignore`，工作副本里的 `.superpowers/**` 已写进 `eslint.config.ts` 的 `globalIgnores`。

---

## §4 打包与安装（已完成）

构建脚本 `.superpowers/sdd/build-baidu-login.cmd`（**已跑完，勿再跑**）：

```cmd
@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
set PATH=C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;%PATH%
cd /d F:\soft\00selfmade\filemanager\sigma-file-manager
call npm run tauri:build > .superpowers\sdd\build-baidu-login.log 2>&1
echo EXIT=%ERRORLEVEL% >> .superpowers\sdd\build-baidu-login.log
```

启动方式（**不挂 stdout 管道**）：

```powershell
Start-Process -FilePath "cmd.exe" `
  -ArgumentList "/c",".superpowers\sdd\build-baidu-login.cmd" `
  -WorkingDirectory "F:\soft\00selfmade\filemanager\sigma-file-manager" -WindowStyle Hidden
```

结果：

- 日志尾部 `EXIT=0`、`Finished release profile [optimized] target(s) in 4m 44s`；
- 产物 `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.13.0_x64-setup.exe`
  = **45335623 字节，23:42:35**；
- `publish-bundle-to-release.mjs` 因 release 根已有**同版本但不同内容**的已发布副本，本轮包落在
  **`release\local-builds\Alpha File Manager_3.13.0_x64-setup.exe`**。
  ⚠️ release 根那个 45370188 字节 / 21:48 是**旧发布副本，不是本轮产物**。

静默安装 `.superpowers/sdd/install-baidu-e2e.cmd`（`"%SETUP%" /S`）→ `EXIT=0`。

安装位置实测为 **`D:\Program Files\Alpha File Manager`**
（`C:\Program Files\Alpha File Manager` 目录存在但为空，别被它误导）：

- `alpha-file-manager.exe` **81710592 字节 @23:42:02** —— 与本轮二进制一致 ✓
- `uninstall.exe` @23:44:27
- 注册表卸载项 `DisplayVersion=3.13.0`、`InstallLocation=D:\Program Files\Alpha File Manager`

---

## §5 卡点：真机界面验收找不到「设置」入口（纯自动化问题）

### 5.1 已确认的事实

- 应用已启动并前台运行：**PID 962384**，主窗 hwnd **6555342**，窗口 3862x2110，屏幕 3840x2088（DPI 144）。
- 当前停在**「位置」页**。
- UIA 树 dump：116 元素，其中 **35 行有 Name**，已落盘 `.superpowers/sdd/baidu-embedded-accept/uia-tree.txt`。
- 该 dump 里能看到的完整清单（节选关键项）：

| ControlType | Name | 位置 |
|---|---|---|
| Image | 要获取缺失的图片说明，请打开上下文菜单。 | `24,24 31x30` |
| Button | `C` / `D` / `E` / `F` / `G` | `x=21, y=1782/1830/1878/1926/1974`，各 `37x36` |
| Button | 添加网络驱动器 | `21,2025 37x36` |
| Text | 主页 | `88,384 85x57` |
| Text | 用户目录 / 驱动 / 网盘 与 远程连接 | `103,480` / `103,648` / `103,834` |
| Button | 首页 … 音乐（7 个） | `y=534`，各 `444x72` |
| Button | 各盘符 | `y=702`，各 `444x96` |
| Text | 网盘 / 远程连接 | `103,881` / `103,1040` |
| Button | 百度网盘 | `103,920 444x97` |
| Button | 速豹堂 / aiec.fun / jury.xin | `y=1079`，各 `444x97` |

- **左侧 rail 的导航图标（设置、文件浏览器等）在这份 dump 里完全没有出现** ⇒ 它们的 UIA `Name` 是空串。
  盘符按钮有名字，说明 rail 上「有名字的按钮」是能被枚举到的；导航图标属于空 Name 那一类。

### 5.2 下一轮**直接照抄已有解法**，不要再从零试探

仓库里已有实测通过的模板：**`.superpowers/sdd/accept-r22c.ps1`**。它的做法正是本场景需要的：

```powershell
# accept-r22c.ps1 :140-167 的核心思路（已实测可用）
function Get-RailButtons {   # 刷新根后，收集左侧条带内的所有按钮
  Refresh-Root
  ...   # 过滤条件见该文件，按 Top 排序
}
$rails = Get-RailButtons
Say "RAIL_BUTTONS=$($rails.Count)"
$railIndex = -1
for ($i = 0; $i -lt $rails.Count; $i++) {
  [void](Click-Center -e $rails[$i] -What "rail$i")   # 真实鼠标点击（InvokePattern 在 rail 上是静默 no-op）
  Start-Sleep -Milliseconds 1000
  Refresh-Root
  $nav = Find-Elem -Name $nRemote -CtlTypes @('Button','ListItem','Link','TreeItem','Text') -MinW 20
  if ($null -ne $nav) { $railIndex = $i; break }       # 出现「远程」= 已进入设置页
}
Say "SETTINGS_RAIL_INDEX=$railIndex"
```

**要点逐条**：

1. **不要用 `InvokePattern`** 点 rail —— 元素会返回成功但页面不变（`accept-locations.ps1:153` 与 `accept-3130.ps1:153` 都留了这条注释）。
2. 用**真实鼠标点击**（`SetCursorPos` + `mouse_event`），或 `PostMessage(WM_LBUTTONDOWN=0x0201 / UP=0x0202)` 发 `Chrome_RenderWidgetHostHWND`。
3. 判定「到没到设置页」的锚点是 **`远程`** 两字（`$nRemote`），比找齿轮图标可靠。
4. `accept-remote-probe.ps1:73-74` 里的 rail 过滤条件是 `Left ∈ [10,70]`、`Top ∈ [60,500]`；那是**小窗口时期**量的。
   本轮窗口高 2110，rail 导航图标大概率仍在 `Top ∈ [60, ~700]`，但要**从当帧实测**取，别硬编码。
   盘符按钮在 `y=1782..2025`，可当作 rail 下沿的参照。
5. 历史 `accept-locations.ps1:129` 用 `foreach ($railY in @(129, 85, 129))` 逐个试 ——
   那是 2582x1390 窗口下的量值，**本轮 3862x2110 不能照抄**，必须重算或改用上面的「枚举+逐个点」法。

### 5.3 进入设置页之后的验收步骤

1. 设置页 → **远程连接**分节 → **新建连接**；
2. 协议下拉 `data-test="field-protocol"`（`:520`）选 **百度网盘**（`settings.remote.protocol.baidu_pan`；Radix `Select`，选项文本「百度网盘」）；
3. 点 **「用内嵌窗口登录」**（UIA Name = 这个词）→ 断言出现状态行「正等你完成登录…」；
4. 等登录窗：标题 **`百度网盘登录`**、class `Tauri Window`、`1100x820`
   （与主窗并列的另一个 `Tauri Window`，取 rect 最小的那个即可区分）；
5. 期望自动 `ready`（见 §7 的风险项）→ 断言文本片段 **「会话会在保存时写入」**（`status.ready`）；
6. 名称框 `data-test="field-name"`（`:503`）填 `baidu-e2e` → 保存；
7. **断言打到落盘文件**，不要只信界面文字：
   - `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\remote-credentials.json`
     （常量定义在 `src-tauri/src/remote/credentials.rs:22`，路径 `app_data_dir.join(...)` 在 `:49`）
     应出现新键 = 新 connection id；
   - `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-remote-connections.json`
     应出现 `baidu-e2e` 条目；
8. **收尾必须还原**：先确认 `Get-Process alpha-file-manager` 为空 → 再 restore（顺序不能反）。

### 5.4 本轮已有的备份（restore 就用这两份）

`.superpowers/sdd/baidu-e2e-backup/`：

- `remote-credentials.json` — **793 字节**
- `user-remote-connections.json` — **1483 字节**

当前真实数据（4 条连接）：

| 连接 | 协议 | id / credentialKey |
|---|---|---|
| 速豹堂 | sftp | — |
| aiec.fun | sftp | — |
| jury.xin | sftp | — |
| 百度网盘 | `baidu_pan` | `0736c40a51db4fd19f711c0b6934f103` |

`remote-credentials.json` 的 `entries` 下**只有一个键** `0736c40a51db4fd19f711c0b6934f103`，值是 DPAPI 密文。

> 本轮**只做了备份与安装，没有新建任何连接** ⇒ 真实数据目前**未被改坏**。

---

## §6 已验证 / 推断 / 未验证（严格三分）

### 已验证（有命令输出或文件事实支撑）

- 五道门禁全部通过（vue-tsc / eslint 整仓 / eslint 改动文件 / verify-changelog / 定向单测 56 条）。
- `npm run tauri:build` 成功，产出 45335623 字节的 NSIS 安装包。
- 静默安装成功（`EXIT=0`），安装目录为 `D:\Program Files\Alpha File Manager`，二进制字节数与时间戳与本轮构建吻合。
- 应用启动并稳定运行，UIA 能枚举到 116 元素 / 35 个有名字的元素。
- 真机上协议下拉里已存在「百度网盘」，且「位置」页能看到既有 4 条连接（说明本轮安装未破坏用户数据）。

### 推断（合理但**未**在本轮直接观测）

- `EBWebView\Default\Network\Cookies`（`C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.app\EBWebView\Default\Network\Cookies`，**32768 字节**）已存在
  ⇒ 内嵌登录窗打开后可能**已是登录态**，无需人工扫码即可自动 `ready`。
  **这是推断，不是结论** —— 取决于该 cookie 是否仍有效、是否过期。
- 上一轮「按 `settings` / `remote_locations` 搜索得到 0 命中」被我判为 rail 无 Name 的证据；
  现在看更准确的说法是：那两个 needle 是**英文**，而界面是中文，`设置` 就该用码位 `0x8BBE 0x7F6E` 去搜。
  `accept-baidu-t9b.ps1:67` 里已有正确写法 `-join @([char]0x8BBE, [char]0x7F6E)`。
  ⇒ **§5.1 的「rail 无 Name」结论需要用中文 needle 复核一次**，别直接当成定论。

### 未验证（明确缺口）

- **真机端到端**：内嵌登录窗能否弹出、能否读到 cookie、表单能否自动填、保存后能否落盘 —— **一次都没跑过**。
- **`cargo test --lib`**：本轮新增模块靠「全同步命令」规避 tao 链接红线，
  但**测试二进制能否正常加载没有实测**。下一步第一件事就该跑它。
  红线回顾：任何被测试链接到的代码**不得**触碰 `tauri::async_runtime::*`，
  否则会单态化 `GlobalRuntime<Wry>` → 拖进 tao → `comctl32 v6` 在加载期 `STATUS_ENTRYPOINT_NOT_FOUND (0xC0000139)`，
  一个测试都跑不了。
- **全量 `npx vitest run`**：本轮启动，结果待读 `.superpowers/sdd/vitest-full.log` 尾部 `VITEST_EXIT=`
  （typed 意排在打包之后跑，避免 CPU 争用让 `single-instance-guard.test.ts` 撞 30s 超时假红）。

---

## §7 本轮新增的工具坑（下次别再踩）

1. **bash 工具会拒绝「切换工作目录 + 重定向写文件」的组合**，报
   `Local hard safety policy blocked bash: write/delete targets cannot be verified safely after a command working directory change`。
   `cd X; ... > rel.log` 与 `Set-Location X; ... *> abs.log` **两种都挡**（用了绝对路径也挡）。
   ⇒ 正确姿势：把 `cd` 与重定向**都封进纯 ASCII 的 `.cmd`**，外部只 `Start-Process cmd.exe -ArgumentList '/c',<cmd绝对路径>`。
   本轮 `run-vitest-full.cmd` 就是这个形态。
2. **当前模型不能看图片**：`read` 一个 PNG 会返回 `tool image omitted: model does not support images`，
   只剩一行「原尺寸 + 缩放比」。⇒ 不要指望「读截图找齿轮坐标」这条路，UIA / 源码是唯一可靠通道。
3. **`write` 工具拒绝覆盖本会话未读过的已存在文件**（`Refusing to overwrite ... this session has not read the file`）。
   ⇒ 要么先 `read` 一次，要么**换一个新文件名**（后者更省调用）。
4. 前几轮已记、本轮再次生效的坑：`.cmd` / `.ps1` 必须**纯 ASCII**；`.cmd` 的 `echo` 里不能有 `>`；
   `.cmd` 运行期间**绝对不能编辑它**（cmd.exe 按字节偏移逐行读）；后台构建**不挂 stdout 管道**；
   恢复真实数据顺序必须是「先杀进程 → 再 restore」。

---

## §8 证据与产物路径索引

| 类别 | 路径 |
|---|---|
| 构建日志 | `.superpowers/sdd/build-baidu-login.log`（尾部有 `EXIT=`） |
| 安装脚本 / 日志 | `.superpowers/sdd/install-baidu-e2e.cmd` |
| 全量单测脚本 / 日志 | `.superpowers/sdd/run-vitest-full.cmd` / `.superpowers/sdd/vitest-full.log` |
| 本轮 UIA dump | `.superpowers/sdd/baidu-embedded-accept/uia-tree.txt` |
| 本轮截图 | `.superpowers/sdd/baidu-embedded-accept/window.png`、`window-2.png` |
| 真实数据备份 | `.superpowers/sdd/baidu-e2e-backup/remote-credentials.json`（793 B）、`user-remote-connections.json`（1483 B） |
| 本轮安装包 | `release/local-builds/Alpha File Manager_3.13.0_x64-setup.exe` |
| 本轮构建产物 | `src-tauri/target/release/bundle/nsis/Alpha File Manager_3.13.0_x64-setup.exe` |
| 可复用验收模板 | `.superpowers/sdd/accept-r22c.ps1`（rail 枚举+逐个点）、`.superpowers/sdd/accept-locations.ps1`（`SetProcessDPIAware` / `Get-MainWindow` / `Save-Shot` / `Click-Screen` 全模板）、`.superpowers/sdd/accept-baidu-t9b.ps1`（本轮 UIA dump 生成器） |

---

## §9 下阶段操作清单（按顺序照做即可）

1. 确认工作区无新改动：`git -C F:\soft\00selfmade\filemanager\sigma-file-manager status --short`；
2. 读 `.superpowers/sdd/vitest-full.log` 尾部 `VITEST_EXIT=`；红的话先分「真红」与「超时」；
3. 跑 `cargo test --lib`（**必须**注入 MSVC + cargo PATH），确认测试二进制能加载；
4. 关掉正在运行的实例（PID 可能已变），或复用；按 §5.2 的模板写一个新脚本
   （**新文件名**，别覆盖 `accept-r22c.ps1`）进设置页；
5. 按 §5.3 走完端到端验收，证据落到 `.superpowers/sdd/baidu-embedded-accept/`；
6. **先杀进程 → 再 restore 备份**，核对字节数 793 / 1483；
7. 真机通过后 `git checkout main && git merge --no-ff feat/baidu-embedded-login`；
   合并前记得 `git status` 里那个 `extensions/kizemo.focus-sync/sidecar/src/bin/` 是**故意不提交**的；
8. 合并后补一条 CHANGELOG（走 `npm run release:prepare -- --bump none --skip-build` 的正确路径，或等下次发版一起）。

---

## §10 风险与未决

- **登录态可能已过期**：若 WebView2 里的百度会话失效，登录窗会停在登录页，需要**用户扫码**。
  验收脚本必须预留 `NEED_USER_LOGIN` 分支 + 截图取证，不能把它当成产品缺陷。
- **`cookies_for_url` 死锁约束**是硬红线，任何「顺手改成异步命令」的重构都会引入死锁。
- **认领时机**：登录态只在 `saveConnection` 那一步落盘。如果用户点了内嵌登录、`ready` 了、却没保存就离开，
  `PendingSession` 会留在槽位里（最长 900s 后被超时清掉）。这是**设计取舍**，不是 bug，
  但下一轮若要改成「登录即落盘」，必须先解决没有 connection id 可写的问题。
- **合并前最后再确认一次**：`tauri dev` 下所有路由页内容区空白是**已知 dev 特性**，
  绝不能用 `tauri dev` 做界面验收（历史已归因，不是代码缺陷）。