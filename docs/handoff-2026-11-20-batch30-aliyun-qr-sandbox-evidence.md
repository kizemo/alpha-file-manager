# Handoff · 批次 30 装机沙箱二维码图证（2026-11-20）

## 背景

批次 30 修的是「阿里云盘登录二维码被撑成 809 CSS px」的布局缺陷：
`src\modules\settings\ui\categories\remote\connection-form.vue:1824-1839` 给
`.remote-connection-form__qr` 加了 `width/max-width/height/max-height: 180px; margin: 0 auto`。
HEAD = `ce4cf6af`，tag `v3.16.0` 已建，装机产物即验证对象（不重构建、不改 src）。

本会话的任务只有一个：在**隔离桌面（沙箱）**里驱动**已装机**的
`D:\Program Files\Alpha File Manager\alpha-file-manager.exe`
（FileVersion 3.16.0，84,895,232 B，sha256 前缀 `FE2A6376B56E068D`），
走到「设置 → 远程 → 阿里云盘 → 显示二维码」，截出二维码图证交回父会话，
同时供用户真机扫码登录。

沙箱取证手段受 `AGENTS.md:647-676`（§2.8）硬约束，违反即证据作废。

## 结果：已完成

二维码图证已产出，判据全绿：

```
MEASURE device=152x152 css=152x152 expectedDevice=180 size_pass=True ink=0.537 panel=0.236
captured_at = 2026-11-20T21:59:59   desktop = b30u15019   dpr = 1
missing_targets = 空
```

- 主证据：`F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\b30\b30-qr.png`
  （152×152 px，7,011 B，完整可扫，三个定位图案齐全）
- 上下文图证：`.superpowers\sdd\b30\b30-qr-panel.png`（ink 0.236）
- 全窗过程图：同目录 `b30-00-maximized.png` / `b30-02-aliyun-form.png` /
  `b30-03-fallback-open.png` / `b30-04-after-request.png` / `b30-poll-1.png`
- 结构化 trace：`.superpowers\sdd\b30\b30-uia-trace.json`（coord / clicks / missing_targets /
  measure / captured_at）
- 运行日志：`.superpowers\sdd\b30\b30-uia.log`、`b30-uia-run.out.txt`
- 元素树 dump：`b30-uia-dump.txt`、`b30-dump-decoded.txt`（\uXXXX 已解码）

152/180 = 0.844，与 QR version 7（45 模块 + 每边 4 静区 = 53，45/53 = 0.849）吻合，
说明 `<img>` 渲染盒正是 180 CSS px，缺陷已消除（修复前是 809 CSS px，墨迹会到 ~750 px）。

## 关键发现：UIA 看不到二维码，必须用像素判据

`b30-uia-dump.txt` 的最后一次 dump（nodes=152 named=80）里**没有任何二维码节点**，
只有 `要获取缺失的图片说明，请打开上下文菜单。 | Image | 16,16,21,20` 这个无关项。
原因：二维码 `<img>`（`data-test="aliyun-qr-image"`，`connection-form.vue:1447-1453`）
没有可访问名称，Chromium 把它从无障碍树里剔除了。

⇒ 之前几轮「POLL images= squares= 全空」不是点击失败、不是尺寸回归没修好，
而是**检测手段本身不可能命中**。本轮把出码判据从 UIA 节点改为位图墨迹定位
（`B30::QrInkBox`：先取暗像素行投影的最长连续段，再在该行段内取列投影最宽连续段，
要求两边都 ≥ minSide 且非对称度 ≤ 0.25），第一次轮询即命中。

## 本轮修掉的脚本缺陷（全部落盘生效）

1. **决定性的一条**：`STARTUPINFO` 的 `lpDesktop` 是 `public string` 而结构体默认
   `CharSet.Ansi`，调用方却是 `CreateProcessW` ⇒ 子进程拿到被 ANSI 字节污染的桌面名，
   进程创建成功但初始化失败，**16 s 内以 `0xC0000142 (STATUS_DLL_INIT_FAILED)` 退出，
   桌面上零窗口**。改为
   `[StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] public struct STARTUPINFO`。
   改完立刻 `WINDOW_FOUND`。定位方法：与已知可用模板
   `.superpowers\sdd\quark\t14-quark-toast-sandbox.ps1`（`55` / `278-279` 用 `IntPtr lpDesktop`）
   逐字段对比 launch 参数，而不是猜「偶发/环境」。
2. `POINT` 类型缺失 ⇒ `B30` 内新增 `public struct POINT`，PS 侧 `New-Object B30+POINT`。
3. 校准块在 `Bitmap.Dispose()` 后仍引用 ⇒ 先取 `$capW/$capH` 再 Dispose。
4. 内嵌分支裁图原点错用主窗 frame ⇒ 引入 `$cropWi`，measure 段按 `$cropWi` 是否为空选原点。
5. 新增 `GetExitCodeProcess`，PS 侧须 `$code = [uint32]0` 才能正确 marshal `out uint`。
6. 窗口发现改为轮询（`$WindowWaitSec=60`，每 5 s 判进程是否已退并打印
   `APP_EXITED code=0x...` / `WAIT_WINDOW`）；启动改为 `$LaunchAttempts=3` 次重试，
   每次 attempt 新建桌面、attempt>1 先杀残留实例、成功后打印 `DESKTOP_WINDOWS` 全窗口列表。
7. 出码检测改像素墨迹定位（见上）；尺寸判据改为「墨迹框 ⊂ 180 px img 盒」：
   `min_qr_px = 180*dpr*0.50 = 90`，`max_qr_px = 180*dpr*1.10 = 198`，
   `size_pass = 90 ≤ w,h ≤ 180`。

## 已验证 / 推断 / 未验证

**已验证（沙箱实跑，run bg_464b8fc4）**
- `CreateDesktopW(0x1FF)` + `si.lpDesktop = 'WinSta0\' + $deskName`（Unicode marshal）能起沙箱实例。
- 沙箱内 `dpr = 1`（`GetDpiForWindow` = 96），**与本机 Default 桌面的 1.5 不同**。
- 坐标模型：UIA BoundingRectangle 为屏幕物理像素，root rect == 客户区，
  `位图 px = 屏幕 px - frame origin`（frame origin = -8,-8）；脚本自证
  `COORD_CHECK ... => CONFIRMED`、`CAPTURE_PX 2576x1408 == FRAME_PX`。
- `PostMessageW` 投给 `Chrome_RenderWidgetHostHWND` 客户区坐标的鼠标等价点击**能驱动页面**：
  依次命中并点开 `阿里云盘`(Button 1231,697,91,41) → `备用：手动二维码登录`(1035,903,809,25)
  → `显示二维码`(1035,927,105,41)，`<details>` 确实展开、二维码确实渲染。
- 二维码墨迹 152×152 device px、ink 0.537、裁图可解码为完整 QR。

**推断（合理但未单独取证）**
- 152 px 墨迹对应 180 CSS px img 盒（由 QR version 7 模块占比反推，未直接读 DOM）。

**未验证 / 未取证**
- 键盘路径**未在沙箱内取证**（§2.8：沙箱内没有键盘输入，只允许鼠标等价断言）。
- 真机扫码 → 授权回传 → 连接保存这条闭环未测（需用户手机扫码）。
- 二维码有效期未实测：`captured_at = 2026-11-20T21:59:59`，阿里云 device_code 流通常
  分钟级过期，过期后前端会显示「二维码已过期」；要扫就尽快，失效就重跑脚本。
- CDP 路线在装机产物上不可用（exe 不监听 9333，直连 ECONNREFUSED）；
  `.superpowers\sdd\b30\b30-sandbox-qr.ps1` / `b30-cdp.mjs` 留盘但**不是本轮路径**。

## 复现命令

```powershell
# 一次性取证（会先杀残留 alpha-file-manager，再建沙箱桌面）
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File `
  'F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\b30\b30-uia.ps1' 2>&1 |
  Tee-Object -FilePath 'F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\b30\b30-uia-run.out.txt'
```
必须用 `powershell.exe`（5.1），脚本必须**纯 ASCII**（中文文案由码点构造）。
门禁自检：`NON_ASCII_COUNT=0`、heredoc `@'` 与 `'@` 数量相等、`PS51_PARSE_ERRORS=0`。
注意 bash 工具进程是 pwsh 7，其 Parser 结论不能代替 5.1。

## 工具坑（勿重踩）

- `0xC0000142` 秒退 + 桌面零窗口 ⇒ 先查结构体 CharSet / lpDesktop marshal，别归因环境。
- here-string 结束符写成 `'''` 而非 `'@`：pwsh 7 不报错，5.1 会把后段吞进字符串。
- 表达式上下文里静态方法必须用 `[B30]::FindMainWindow($hDesk, [ref]$all)` 写法。
- PS 5.1 无三元运算符；`int + 'x'` 会走 Int32 转换报错 ⇒ 一律 `('{0}x{1}' -f $a,$b)`。
- `New-Object Drawing.Bitmap($path)` 不稳 ⇒ `[Drawing.Bitmap]::FromFile($path)`，
  **先取 Width/Height 再 Dispose**，否则文件锁挡住后续 `Crop`。
- PowerShell 别名 `trace` → `Set-TraceSource` 优先级高于同名 function ⇒ 用
  `$Script:Clicks/$Script:Missing` + `$trace` hashtable。
- 长跑取证只能经 bash 工具 `run_in_background` 起（`Start-Process -WindowStyle Hidden` 吞 stderr），
  且严禁 `| Select-Object -First N` 截断。
- 同名节点必须按 ControlType 排序再比名称长度、面积：boot 时 `阿里云盘` 同时命中描述段落
  `Text`(1035,446,618,19) 与真 `Button`(1231,697,91,41)，`BestBox` 靠 `Rank()` 选对了 Button。

## 下阶段建议

1. 用户手机扫 `b30-qr.png` 完成授权 → 验证连接落库、列表出现「阿里云盘」条目；
   若已过期，用上文复现命令重跑取新码。
2. 沙箱内补「内嵌官方页登录」路径图证（`aliyun-embedded`，`connection-form.vue:1363-1502`），
   本轮只覆盖了备用手动二维码分支。
3. 二维码尺寸判据若要更硬，可加一条 DOM 级断言（需 dev 构建开 CDP，或前端补
   `aria-label` 让 `<img>` 进入无障碍树 —— 后者是 1 行改动，能让以后 UIA 直接命中）。
4. 本会话未 commit、未改 src、未动 `src-tauri\target`；`release/` 被 gitignore。

---

## 批次 30 续：建议项 2 / 3 已完成（2026-11-20 22:55）

接手本文件的下一个会话把「下阶段建议」的四条全部推进了一轮。

### 建议项 2：内嵌官方页登录路径 —— 沙箱取证 PASS

取证脚本：`.superpowers\sdd\b30\b30-embedded.ps1`（50116 B，**新建**；`b30-uia.ps1` 未动），
自写 JSON writer（原因见「工具坑」）。被测对象仍是已装机的 3.16.0
（`sha256_16=FE2A6376B56E068D`，`len=84895232`）。

| 判据 | 结果 | 证据行 |
|---|---|---|
| 点「打开登录窗口」出现第二个顶层窗口 | **PASS** | `WINDOWS_AFTER [… 690490 cls=Tauri Window title=阿里云盘 登录 vis=True rect=38,0,1710,1106]`；`LOGIN_WINDOW hwnd=0x690490 titleMatch=True` |
| Rust 侧建窗 | **PASS** | `afm-aliyun-login.log` 新增 20 行：`login_start called` → `emit kind=opening` → `login window data_directory=…session-1795186428127` → `login window built` → 周期性 `observed status=new` / `emit kind=page_status` |
| 表单状态跟随 | **PASS** | `STATUS_PROBE_WAITING len=59 …正等你扫码…`；`ERROR_PROBE len=0`（无错误文案） |
| 官方页真的渲染出来了 | **PASS**（截图） | `b30-embedded-login-window.png`（-0s/-4s/-10s 三连拍）：窗口标题「阿里云盘 登录」，页面是官方扫码登录页（扫码登录/账号登录 标签 + 官方二维码 + 「请用阿里云盘 App 扫码」） |
| 「关闭窗口」路径 | **PASS** | `LOG_CLOSE 138: login_close called` / `139: emit kind=cancelled`；关窗后枚举只剩主窗 |
| 新码尺寸 | **PASS** | `MEASURE device=230x230 css=153.3x153.3 expectedDevice=180 size_pass=True ink=0.542 panel=0.308`；`captured_at = 2026-11-20T22:54:10` |

**未判定（不洗成 PASS）**：登录窗内 **web 正文文字**没进 UIA 树（只能拿到窗口标题与文档名
`登录` / `登录 - Web 内容`）；页面渲染正确性只由 `PrintWindow` 截图证明。

**尺寸口径互证**：本轮沙箱 dpr = **1.5**（上一轮是 1.0），墨迹 230 设备 px = 153.3 CSS px；
153.3 ÷ 180 = **0.852**，上一轮 152 ÷ 180 = 0.844，都等于 QR version 7 的 45/53 = 0.849
⇒ 两轮独立互证 `<img>` 渲染盒就是 180 CSS px，批次 30 的布局缺陷确实消除。

### 建议项 3：二维码 `<img>` 补可访问名（已改源码，**未 commit**）

`src\modules\settings\ui\categories\remote\connection-form.vue:1447-1469`：
`alt=""` → `:alt="t('settings.remote.aliyun.embedded.title')"`（复用现有 key「扫码登录」，
**不新增 i18n key** —— 新增要同步 17 个语言包，`aliyun-i18n-keys.test.ts` 会卡键不一致）。
配套回归测试加在 `aliyun-connection-form.test.ts`（断言 alt 非空且等于该 key）。
验证：`vitest` **14 passed / 14**；`eslint` 两个文件 **exit 0**。
⚠️ 这条要**下一次构建**才进 exe；当前装机产物仍是无障碍树盲区。

### 建议项 1：扫码闭环 —— 仍未完成，等用户真机扫码

- `user-remote-connections.json` 仍是 5 条（3×SFTP + 百度网盘 + 夸克网盘），**没有阿里云盘条目**；
  appdata 根目录下**没有任何阿里云盘会话 blob** ⇒ 扫码→认领→落库这条链一次都没走通过。
- 机制（源码实证）：`remote_aliyun_qr_poll` 返回 `confirmed` 只让界面变「已登录」，
  会话先进**待认领槽位**；中文文案写明「已登录。**保存这条连接即可**——会话会在保存时写入」
  ⇒ 扫完码**还必须在界面点一次「保存」**，会话才真正落盘、列表才出现「阿里云盘」。
- 交接时的活口：沙箱里留了一个**活着轮询**的实例（Run 3 收尾 `APP_PID 258340 alive=True
  instance_left_running=True`，启动 22:53:22），最新码 `b30-qr-live3.png`
  （`captured_at=2026-11-20T22:54:10`，sha256 `FABF42A92AE8D90BB74D27E389B77F3799A1E59845B6C89A4F83A62DD6374074`）。
  ⇒ 用户扫完 → 手机点「确认登录」→ **在沙箱里用鼠标探针驱动一次「保存」** → 核验
  会话 blob + `aliyun_pan` 条目。
- 本轮全程未在沙箱里做键盘断言（§2.8：沙箱内无键盘输入）；键盘路径依旧未取证。

### 本轮新增的工具坑

1. **PS 5.1 的 `ConvertTo-Json` 在深/宽结构上会指数级展开** —— 本轮两次把脚本撑到
   **3.4 GB / 1.4 GB 工作集**且不返回，看起来像「脚本卡死」。⇒ trace JSON **必须自写 writer**
   （深度上限 6、数组上限 60），或改写扁平 txt。症状是进程活着、CPU 满、输出停住。
2. **登录窗截图要等页面渲染**：点开瞬间 webview 还是 `about:blank`（Run 1/2 的截图是空页），
   等 4–10 s 才出官方页 ⇒ 截图判据必须带「等渲染再拍」，否则会误判成「窗口是空的」。
3. **沙箱 dpr 不固定**：同一台机同一套脚本，两轮分别拿到 dpr=1.0 与 dpr=1.5
   ⇒ 尺寸判据必须从 CSS 期望值 × dpr 推导，不能把 180 当设备像素硬编码。

### 产物清单（全部在 `.superpowers\sdd\b30\`）

`b30-embedded.ps1`、`b30-embedded.log`、`b30-embedded-run.out.txt`、
`b30-embedded-trace.json`（11695 B，38 键，`ConvertFrom-Json` 解析通过）、
`b30-embedded-trace.txt`、`b30-embedded-login-window.png` + `-0s/-4s/-10s`、
`b30-embedded-00-maximized.png` / `-02-aliyun-form.png` / `-03-after-window-start.png` /
`-04-fallback-open.png` / `-05-after-qr-request.png` / `-poll-1.png`、
`b30-embedded-dump-main.txt` / `-dump-login.txt`、
`b30-embedded-log-before.txt` / `-after.txt` / `-closed.txt`、
`b30-qr-live2.png` / `-live2-panel.png`（22:54:10 那轮）、`b30-qr-live3.png` / `-live3-panel.png`（同名副本，防客户端缓存）。

### 当前状态

- 源码改动（`connection-form.vue` + `aliyun-connection-form.test.ts`）**未 commit**；
  HEAD 仍 `ce4cf6af`，tag `v3.16.0` 未动，未重构建、未改 `src-tauri\target`。
- 批次 30 的**唯一未闭合项 = 建议项 1 的真机扫码闭环**（需要用户的手机）。

## 批次 30 续：建议项 1 扫码闭环已完成（2026-11-20 23:07）

状态：**PASS（闭环达成）**。用户手机扫码并在手机上确认登录后，沙箱内已成功驱动「保存这条连接」，阿里云盘会话落盘核验通过。

### 事实链（全部来自沙箱实测；脚本 `.superpowers\sdd\b30\b30-save.ps1`）

| 环节 | 证据 |
| --- | --- |
| 扫码确认 | 沙箱主窗 `0xA40450` 内 UIA 命中「已登录。保存这条连接即可——会话会在保存时写入。」×2 处（592,386 与 592,812）；scanned / waiting / timeout / error 四个否定判据全部 `no` |
| 填名 | 名称框 `Edit#1`（accessible Name=名称，592,181,1806,242）初值为空；鼠标点击聚焦（screen 1199,211）后用 WM_CHAR 注入码点 0x963F/0x91CC/0x4E91/0x76D8 ⇒ 回读值 = 阿里云盘。**这是键盘路径**，脚本内显式标注 `typed_via`，不冒充鼠标证据 |
| 保存 | 「保存」共 2 个：`592,997,685,1058`（禁用）、`738,1318,829,1379`（启用但在可视区外）；Wheel 下滚 3 格后按钮移到 `738,874,829,935`，点击 (784,904) |
| 连接落库 | `user-remote-connections.json` 5 条 → **6 条**，mtime 23:06:36；新增 `protocol=aliyun_pan`、`name=阿里云盘`、`host=api.aliyundrive.com`、`port=443`、`credentialKey=4082003cccea41cd86673872155be846` |
| 凭据落库 | `remote-credentials.json` `entries` 2 → **3**，新增 `key=4082003cccea41cd86673872155be846`（base64 DPAPI 密文，长度 1716），mtime 23:06:35；未产生 `remote-credentials.corrupt-*.bak` |
| 列表 | 保存后界面回读：连接列表出现「阿里云盘 / api.aliyundrive.com:443 / 直连」（UIA `LIST_HAS_ALIYUN=yes`；截图 `b30-save-03-after-save.png`） |

### 关键路径事实（避免下次重走弯路）

- 阿里云盘会话**只有一个落盘位置**：`<appdata>\remote-credentials.json`（DPAPI 密文，键 = 连接 credentialKey）。**不存在**「独立会话 blob 文件」，按 blob 找是错判据。
- 闭环顺序固定为：扫码 → `remote_aliyun_qr_poll` 返回 confirmed → Rust 解凭据 + 探活 + 存入**内存待认领槽位** → 点「保存」→ 前端 `saveConnection` → Rust `remote_aliyun_login_claim` → 落盘。**等用户扫码本身不会产生任何落盘产物**，必须驱动保存才闭环。
- 新建阿里云盘连接时名称框为空（`connections-section.vue` 只置协议不填名），而 `validateForm` 与 Rust `connection.rs:112` 都要求名称非空 ⇒ 保存前必须先填名。

### 本轮脚本层踩的坑（已修）

1. `$host` 是 PowerShell **只读自动变量** ⇒ 取 render host 必须换名（`$rhost`）。
2. PS 5.1 里 `$cands = @()` 是**固定长度数组**，`.Add()` 抛 `Collection was of a fixed size.` ⇒ 改用 `New-Object Collections.ArrayList`。
3. 取证脚本**必须幂等可重跑**：首轮在「点保存」前崩掉后名称框已含「阿里云盘」，直接重跑会叠成「阿里云盘阿里云盘」。已加守卫：当前值已等于期望则跳过录入（`typed_via=preexisting_wm_char`）。
4. `$found` 未命中时是 `$null`，而 `$null.Length -eq 0` 在 5.1 下为 **False**（会误判成「已填名」而直奔保存）⇒ 改用 `[string]::IsNullOrEmpty()`。
5. 可视区外的按钮要先 `Wheel` 下滚并**每次重新查询**按钮坐标（本例 1318 → 874），不能只按初始坐标点。

### 产物（`.superpowers\sdd\b30\`）

`b30-save.ps1`（脚本本体）、`b30-save.log`、`b30-save-run.out.txt`（观察模式）、`b30-save-act.out.txt`（首轮动作，崩在按钮收集）、`b30-save-act2.out.txt`（闭环成功）、`b30-save-trace.json`、`b30-save-dump.txt`、`b30-save-00-attach.png`、`b30-save-01-after-focus.png`、`b30-save-02-named.png`、`b30-save-03-after-save.png`。

### 收尾

沙箱实例（PID 258340，桌面 `b30u82795`）在取证完成后**已关闭**，以释放 release 单实例锁；用户可在自己的桌面正常启动 AFM，连接与凭据均已落盘、不受影响。

### 截图口径补记（2026-11-20 23:10）

`b30-save.ps1` 每次运行都会**覆盖** `b30-save-00/01/02` 三张截图，所以磁盘上现存的这三张是**最后一次成功运行（act#2）的同一帧**（sha256 前缀同为 `3CE7C80E1667F87E`，画面中名称框已含「阿里云盘」），不代表三个不同时刻。

「名称框初值为空」的证据在 UIA 回读里：`b30-save-run.out.txt`（观察模式）与 `b30-save-act.out.txt`（act#1）中的 `EDITS #1|592,181,1806,242|en=True|name=名称|val=` 为空串；`b30-save-03-after-save.png`（哈希 `9CF628E569524C07`）是保存后的列表帧。

**取证纪律**：固定文件名会被后一轮覆盖 ⇒ 结论必须写明「该文件是第几轮留下的」，否则会出现「同一哈希的三张图 = 三个时刻」的误读。

### 未取证项（如实标注）

- **实际连通性未验**：本轮只闭合了「凭据落库 + 连接条目 + 列表出现」三条判据；用该连接真正列出网盘文件（走 `api.aliyundrive.com`）**未在沙箱内取证**。
- **键盘路径未在沙箱内取证**：名称录入走 WM_CHAR（脚本内标注 `typed_via`），沙箱无键盘输入，所有**快捷键**路径均未取证。
