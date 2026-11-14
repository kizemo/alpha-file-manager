# 批次 15 交接卡：百度侧同形缺陷复验 ＋ `timeout` 终态 ＋ 装机复验（2026-11-14）

> 上游真台账：`docs/handoff-2026-11-11-quark-pan.md`（本轮技术细节记在它的 **§20.10**）。
> 本文件是**给下一轮会话的入口卡**：背景、目标、进度、下一步，以及「哪些是实测的、哪些是推断的、哪些还没验」。
> 品牌名固定 **Alpha File Manager**；`afm://quark-login` / `afm://baidu-login` 频道名、legacy 数据目录名、Tauri `identifier` 是受保护常量。

---

## 0. 一句话状态

批次 14 遗留的三项**本轮全部做完并实测通过**：① **百度侧**同形缺陷沙箱 A/B（基线弹 2 次 / 修复后弹 1 次，PASS）；
② **`timeout` 终态**（等满 900 秒）真机验证 PASS —— 到点弹**一条带字**的 timeout toast，且**没有**第二条 `closed` 叠加；
③ **装机复验**（NSIS 静默安装 → 跑装机产物）PASS —— toast「一条带字」。
本轮**没有改动任何 `src/` / `src-tauri/` 源码**，改动只有台账 §20.10 与本文件；**已 commit、未 push**；
未提 winget PR、未发 GitHub Release、未做 upstream 移植。仓库门禁 `npm run check` 全绿（3238 用例）。

---

## 1. 背景与目标

- **上游卡**：`docs/handoff-2026-11-14-quark-pan-batch14.md`（批次 14 入口卡）＋ 其 §5.3「未验证」三条。
- **本轮选定目标**（全部是**验收**，不是改码）：
  1. **百度侧**同形缺陷（`connection-form.vue` 与 `netdisk-add-dialog.vue` 同听全局频道 `afm://baidu-login`）的沙箱真机 A/B；
  2. **`timeout` 终态**（Rust `GIVE_UP = 900` 秒）的真机验证；
  3. **装机复验**：NSIS 静默安装 → 沙箱跑**装机产物** → 确认 toast 是「一条带字」。
- **未纳入本轮**：upstream 移植、R4（17 语言包母语复核）、R5（`Ctrl+Shift+P` 换机器验证）、winget 分发、对外发布动作。

### 本轮必须守住的硬约束

| 约束 | 出处 | 本轮怎么守的 |
| --- | --- | --- |
| 验收脚本不许在用户交互桌面抢焦点，必须跑 `CreateDesktop` 隔离沙箱 | AGENTS §2.8 | 全部轮次跑在独立沙箱桌面、窗口固定 1280×900；脚本自身不切桌面、不 `SetForegroundWindow`、不发 `SendInput`；沙箱内只做鼠标等价断言（PostMessage 鼠标 ＋ UIA ＋ `PrintWindow`）；枚举用 `EnumDesktopWindows` |
| 真机验收只用 `npm run tauri:build` 的打包产物，`tauri dev` 结论作废 | AGENTS §2.10 | 基线用批次 13 留档二进制；修复后 / 装机都用 `target\release` 与 NSIS 装机副本；`tauri dev` 全程没用 |
| push / winget PR / GitHub Release 属对外动作，需用户授权 | §19.5-1 | 一律**没做** |
| 恢复真实数据顺序：先杀进程 → 再 restore | AGENTS / 上轮教训 | 按此执行，restore 后核对字节数 6326 ＝ 备份字节数 |
| 明显 bug 根因坐实后直接按推荐方案修，不发问卷 | 用户偏好 | 探针日志句柄的共享/降级兜底、`poll-toast` 计数表达式 bug 直接改（只影响日志与计数，既有步骤语义零改动） |

---

## 2. 结论一：百度侧同形缺陷 A/B **PASS**

| 轮次 | 二进制 | burst 帧 0 相对帧 39 多出的 UIA 节点 | 判定 |
| --- | --- | --- | --- |
| 基线 `t15baidu-base` | 批次 13 留档基线（`t15-baseline\alpha-file-manager.exe`，82,767,360 B，2026-11-12 01:01:57，sha256 `D2EC859D…29D9`） | **2 个 `ListItem`**：`700,804,534x50` ＋ `700,742,534x50`，`name` 为空（空壳卡片） | 基线弹 **2** 次 |
| 修复后 `t15baidu-fix` | `src-tauri\target\release\alpha-file-manager.exe`（82,756,096 B，2026-11-14 01:38:52，v3.14.0，含 toast 空壳修复） | **1 个 `ListItem`**：`name=[登录还没完成，登录窗口就被关掉了。登录窗口已关闭。] rect=700,737,534x117` ＋ `Text name=[登录窗口已关闭。] rect=725,800,168x27` | 修复后弹 **1** 次，**文字可见** |

- **链路（13 步）**：`远程连接` tab → `添加远程连接` → `网盘` tab → `百度网盘` → 等登录窗（标题 **「百度网盘登录」**）
  → `dump-windows` → `WM_CLOSE` 关窗 → burst 40 帧 × 150 ms。
- **节点数**：基线帧 0–3 = 143、帧 4–39 = 141；修复后帧 0 = 141（`COUNT_closed=1`）、帧 1–39 = 138。
  两轮 step-10 / step-12 基准帧都是 **138 节点** ⇒ 起点无残留。
- **计数口径**：数 UIA `ListItem` 实例数；**不能用 toast 文案计数**（修复前卡片是空壳，文案口径恒 0）。
- **两侧同构**：百度/夸克共用同一套 i18n 文案（`closed`/`timeout`/`error` 逐字相同，只有品牌名不同）⇒ 计数 needle 无需区分两侧。
- 差分命令：`node .superpowers/sdd/quark/t14-uia-diff.cjs .superpowers/sdd/quark/t15-shots <tag> 13 0 39`。

---

## 3. 结论二：`timeout` 终态 **PASS**

- **轮次**：`t15installed-timeout`（跑**装机副本**；计划 `t15-plan-timeout.json` = 前 13 步同链路 ＋ `wait 875s` ＋ `poll-toast sec=180 interval=250`）。
- **链路**：远程连接 tab → 添加远程连接 → 网盘 tab → **百度网盘** → 登录窗升起（标题「百度网盘登录」，1672×1106）
  → `dump-windows` → **不关登录窗**、静等 875 秒 → 以 250 ms 间隔轮询 180 秒。
  ⚠️ 这一轮**绝不能**用 `close-login` 步骤：它会先发 `WM_CLOSE`，抢先触发 `closed` 终态，测的就不是 timeout 了 ——
  所以另加了 `poll-toast` 步骤（只轮询、不碰登录窗）。
- **实测结果**（地面真相取自逐轮 UIA dump，**不是**探针的计数，见 §7 工具坑 ⑦）：
  - 轮询 **23–29** 轮：节点数 **141**（基线 138 ＋ 3）、`ListItem` **3 个**（基线 2 个 ＋ **1 个 toast**），差出来的就是终态卡片：
    - `ListItem name=[登录等待超时了，请重试。登录等待超时，请重试。] rect=700,737,534x117`
    - `Text    name=[登录等待超时，请重试。] rect=725,800,231x27`
    ⇒ **标题 = i18n 文案「登录等待超时了，请重试。」，副标题 = Rust 侧 summary「登录等待超时，请重试。」**，
    卡片 **117px**（两行带字）⇒ **timeout 终态真实到达，且 toast 带字**。
  - 轮询 **30–403** 轮：节点数回到 **138**、`ListItem` 回到 **2** ⇒ toast 退场后**没有第二条 toast**。
  - **基线那 2 个 `ListItem` 是远程连接列表里的两条已存连接**
    （`百度网盘***…pan.baidu.com:443`、`夸克网盘***…pan.quark.cn:443`），**不是 toast** ⇒ 计数口径仍是「基线 ＋ 1」。
- **同时验证了缺陷 B 修复在 `timeout` 路径上的关键性质**：Rust 发完 `timeout` 就会 `close_login_window` 关掉登录窗，
  若 `settle()` 没吞掉随后的 `Destroyed`，就会**再弹一条 `closed` toast** ⇒ 实测**只有一条** ⇒ `settle()` 生效。
- **时间**：toast 出现在轮询窗口开始后约 10 秒 ⇒ 从点击「百度网盘」算约 **890 秒**
  （探针日志无时间戳，精度到十几秒），与 `GIVE_UP = 900s`（Rust 侧从登录流程启动起算 ＋ 2 秒轮询节拍）相符。
- 证据：`t15installed-timeout-run.log`、`t15installed-timeout-poll-14-23-uia.json` … `-29-uia.json`、
  `t15installed-timeout-poll-14-23.png`（toast 在场的那一帧）、`t15installed-timeout-state.json`、
  扫描输出 `.superpowers\sdd\quark\t15-poll-scan.txt`（扫描脚本 `t15-poll-scan.cjs`）。
- ⚠️ **`ready` 终态仍未验证**：需要真人扫码。本轮扫描里 `ready` 针脚会命中一句**静态提示**
  「在应用内打开登录窗口。登录成功后窗口会自动关闭，Cookie 随即取到——你只需保存这条连接。」
  （`581,728 565x52`，每一轮都在），**那是设置面板的说明文字、不是 toast**，不得据此声称 `ready` 已验证。

---

## 4. 结论三：装机复验 **PASS**

- **静默安装**：`Alpha File Manager_3.14.0_x64-setup.exe`（45,452,538 B）以 `/S /FORCE` 安装 ⇒ `INSTALL_EXIT=0`；
  守卫日志 `2026-11-14 07:29:32 [early-guard] OUTCOME=proceed reason=forced`
  （同版本同构建必须带 `/FORCE`，否则守卫静默 `Quit reason=same-build`）。
- ⚠️ **实际安装目录是 `D:\Program Files\Alpha File Manager`，不是 `C:\Program Files\…`**：
  注册表 `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Alpha File Manager` 的 `InstallLocation` 指向 D 盘；
  `C:\Program Files\Alpha File Manager` 目录**存在但为空**（无 exe）⇒ 写死 C 盘路径会把「装好了」误判成「没装上」。
- **判据不能用裸 sha256**：装机 `C55BF795…891F3` vs 构建 `DFCCCDD1…6AC41` 不等 ——
  NSIS 会在二进制里打 29 B 的 bundle 标记（`__TAURI_BUNDLE_TYPE_VAR_NSS__` vs `…_UNK__`，等长 ⇒ 尺寸不变但 sha 变）。
  **归一化 sha256**（把 `__TAURI_BUNDLE_TYPE_VAR_[A-Z0-9]+__` 抹成同一个等长串）后两者**完全相等**：
  `FA0F7D4C75CF6850F08ED389013CBCA5E727D2D580161BB919004E5BE3848A23` ⇒ **装机产物 == 本次构建**。
- **沙箱轮跑的是装机副本的字节级副本**（`.superpowers\sdd\quark\t15-installed\alpha-file-manager.exe`，sha256 `C55BF795…891F3`，
  与 `D:\Program Files\…` 里的装机文件逐字节相同）—— 原因见 §7 工具坑 ⑤，**台账如实登记**。
- **判定**：burst 帧 0 = 141 节点、帧 39 = 138 节点，差集 = **1 个 `ListItem`**
  （`name=[登录还没完成，登录窗口就被关掉了。登录窗口已关闭。] rect=700,737,534x117`）
  ＋ `Text name=[登录窗口已关闭。] rect=725,800,168x27`
  ⇒ 卡片 **117px**（两行带字），不是 50px 空壳 ⇒ **「一条带字」PASS**。

---

## 5. 改动清单（文件级）

| 文件 | 改动 |
| --- | --- |
| `docs/handoff-2026-11-11-quark-pan.md` | 新增 **§20.10**（本轮方法 / A/B 数据 / 装机与归一化 sha / `timeout` / 工具坑 / 收尾状态） |
| `docs/handoff-2026-11-15-quark-pan-batch15.md` | 本文件 |
| `.superpowers/sdd/quark/t14-quark-toast-sandbox.ps1` | **仅**追加 `poll-toast` 步骤类型 ＋ 日志句柄加 `FileShare.ReadWrite` 兜底 ＋ 修 `poll-toast` 的 `[string]$x.Method()` 优先级 bug（gitignore 内，不进版本库） |

> 本轮**没有改动** `src/` 与 `src-tauri/` 下任何文件；受保护常量（品牌名、频道名、legacy 目录名、Tauri `identifier`）全部未动。

---

## 6. 验证矩阵

### 6.1 已验证（实测，有输出）

| 判据 | 结果 |
| --- | --- |
| 百度侧基线轮 13 步沙箱链路 | 全通（`t15baidu-base-run.log`，`WRAP_OK`） |
| 百度侧基线弹 2 次 | burst 帧 0 多出 **2 个 `ListItem`**（143 节点 vs 基准 138） |
| 百度侧修复后弹 1 次 | burst 帧 0 多出 **1 个 `ListItem`**（141 节点 vs 基准 138） |
| 修复后 toast 文字真的渲染出来了 | UIA `ListItem name=[登录还没完成，登录窗口就被关掉了。登录窗口已关闭。]` ＋ `Text name=[登录窗口已关闭。]` |
| NSIS 静默安装 | `INSTALL_EXIT=0`，守卫 `OUTCOME=proceed reason=forced` |
| 装机产物 == 本次构建 | 归一化 sha256 两侧**完全相等**（`FA0F7D4C…8A23`） |
| 装机轮 toast「一条带字」 | 差集 **1 个 `ListItem`** ＋ `Text`，卡片 117px |
| `timeout` 终态真实到达 | 轮询 23–29 轮出现 **1 条** timeout toast（141 节点 / 3 `ListItem`），标题「登录等待超时了，请重试。」 |
| `timeout` 路径**不叠加** `closed` | 轮询 30–403 轮全程 138 节点 / 2 `ListItem` ⇒ 只有一条 toast |
| 仓库门禁 | `npm run check` ⇒ `CHECK_EXIT=0`，**311 文件 / 3238 用例全绿** |
| 真实设置已恢复 | `t14-seed.cjs restore` ⇒ `restored bytes=6326`（= 备份字节数），恢复前已结束 `alpha-file-manager` |

### 6.2 推断（有依据，但**未实测**）

1. 「百度侧与夸克侧文案同构 ⇒ 计数 needle 可共用」⇒ 依据是 `ch.json` 两个 key 逐字相同 **且** 两侧登录窗都实测升起过；**没有**为两侧各写一套针脚。
2. 「装机产物与构建产物的**唯一**差异是 29 B 的 bundle 标记」⇒ 依据是归一化后 sha 整体相等；**没有**逐字节列出差异偏移（批次 9 曾用 64 KiB 分块比对，本轮没重做）。
3. 「toast 出现在点击后约 890 秒」⇒ 探针日志行**无时间戳**，只有「轮询窗口内第 23 轮」这个序数；
   换算依赖「404 轮 / 180 秒」的单轮耗时假设（每轮含一次全树 UIA dump ＋ 截图，约 0.45 s）⇒ 精度只到十几秒。

### 6.3 未验证（明确没做）

1. **`ready` 终态**（真实扫码登录）：需要真人扫码，本轮**没有**自动化。
2. **`error` 终态**（读不到登录态）：未单独构造。
3. **夸克侧的 `timeout`**：本轮 `timeout` 走的是**百度侧**（一轮同时覆盖「未验证的百度侧」与「未验证的 timeout」）；夸克侧 `timeout` 未单跑。
4. **非中文 locale** 下的 toast 文案：本轮全程 `locale=ch`。
5. `git push`、winget PR、GitHub Release、upstream 移植（均属对外/大工程动作）。

---

## 7. 本轮工具坑（省得下一轮再踩）

1. **`Tee-Object` 与探针自写的同名 `run.log` 互相锁文件 ⇒ 轮次被中途打断（作废重跑）**：
   探针自己写 `$ShotDir\<tag>-run.log`；再叠一层 `*>&1 | Tee-Object -FilePath <同名>` ⇒ 探针开日志时撞上 Tee 的句柄，
   `FileStream` 抛 `The process cannot access the file … because it is being used by another process`
   ⇒ 装机轮只到 step-11、timeout 轮只到 step-03，两轮证据全部作废。
   ⇒ **探针轮次不要用 Tee 抢同名文件**；要留控制台痕迹就写**不同名**（`-console.log`）。
2. **被强杀的探针会留下 `powershell.exe` 残留，并一直占着 `run.log` 句柄** ⇒ 下一轮开日志即 sharing violation
   （现象是「脚本在第一步就炸」，不是探针逻辑错）。⇒ 每轮开跑前清三样：
   `powershell.exe`（命令行含 `t15-wrap` / `t14-quark-toast`）、`alpha-file-manager`、
   `msedgewebview2.exe`（命令行含 `alpha-file-manager` / `com.sigma-file-manager`）。
3. **单实例插件会把新启动转交给旧实例** ⇒ 残留实例存在时，探针里的 UIA 树是 **0 节点**
   （症状：`shot ok=False` 连着出现 ＋ `NOT FOUND in uia ( nodes)` ＋ 窗口尺寸读成 0）——
   看起来像「界面没渲染」，实际是**拿不到自己的窗口**。⇒ 见到这组症状先查残留实例，别改探针。
4. **本机安全策略会杀 `New-Item -ItemType Directory`**（连同整条命令）⇒ 目录一律用写文件工具预建（写一个 `.keep`）。
5. **工作区外的 exe 路径不能直接作为探针启动路径** ⇒ 装机副本先拷进工作区再跑（台账登记「跑的是字节级副本」）；
   且**命令行里带 `%TEMP%` 下的 exe 路径**启动子进程 ⇒ **整个进程树被秒杀**（exit -1、零输出，连 wrapper 首行标记都写不出来）。
6. **探针日志句柄加共享/降级兜底**（本轮已改 `t14-quark-toast-sandbox.ps1`）：
   `New-Object System.IO.StreamWriter($path,$false,$enc)` 以独占语义开文件，遇残留句柄直接抛；
   改成 `FileStream(…, FileShare.ReadWrite)`，被占用时自动降级到 `<tag>-run-HHmmss.log`。
   ⇒ **既有步骤语义零改动**，批次 14 的复现性不受影响。
7. **`poll-toast` 步骤里的表达式优先级 bug ⇒ 它的 `COUNT_*` 全是废数（本轮踩到）**：
   写成 `if ([string]$n.name.Contains($needle))` 时，强制转换套在**整个布尔表达式**上
   ⇒ 布尔被转成字符串 `"False"` / `"True"` ⇒ `if ("False")` 恒为真
   ⇒ **四个 `COUNT_*` 同时恒等于「有名字的节点数」**（实测 67），且每轮都误判命中、狂写截图。
   ⇒ **判词一律取自逐轮 UIA dump 的节点名 + rect**（本轮用 `t15-poll-scan.cjs` 扫 dump 得到）；
   本轮已改成 `if (([string]$n.name).Contains([string]$needle))`（与 burst 段写法一致），下一轮起 `COUNT_*` 可用。
   ⚠️ 同形通病：**任何 `[string]$x.Method(...)` 都会把布尔转成字符串**，PowerShell 里判布尔别这么写。
8. `Add-Type` 的 C# 段在 pwsh 7 下编译不过 ⇒ UIA/像素探针一律用 `powershell.exe` 跑（沿用批次 14 结论）。
9. 打包必须走 `.superpowers\sdd\quark\batch13-build.cmd`（`cargo` 不在默认 PATH）；本轮**没有重新打包**（沿用 2026-11-14 01:38 的产物）。
10. `node` 读 PS 5.1 写的 JSON 会撞 **BOM**（`Set-Content -Encoding UTF8` 会带 BOM）⇒ 解析前先 `.replace(/^\uFEFF/, '')`。

---

## 8. 证据路径（全部在仓库内）

| 内容 | 路径 |
| --- | --- |
| 真台账（§20.10 本轮详情） | `docs/handoff-2026-11-11-quark-pan.md` |
| 沙箱探针脚本（13 步链路 ＋ `poll-toast`，纯 ASCII） | `.superpowers\sdd\quark\t14-quark-toast-sandbox.ps1` |
| 探针 wrapper（捕获真异常） | `.superpowers\sdd\quark\t15-wrap.ps1` |
| 计划 / 标签 / 文案模式 | `.superpowers\sdd\quark\t15-plan-baidu.json`、`t15-plan-timeout.json`、`t14-labels.json`、`t14-toast-patterns.json` |
| UIA 逐帧差分脚本 | `.superpowers\sdd\quark\t14-uia-diff.cjs` |
| `timeout` 轮逐帧扫描脚本 / 输出 | `.superpowers\sdd\quark\t15-poll-scan.cjs`、`t15-poll-scan.txt` |
| 差分输出 | `.superpowers\sdd\quark\t15-uia-ab-installed.txt`（另见批次 14 的 `t14-uia-ab.txt`） |
| 安装 / 校验脚本 | `.superpowers\sdd\quark\t15-install.ps1`、`t15-installed-verify.ps1`、`t15-copy-baseline.ps1` |
| 三轮日志 / 逐帧 UIA 树 / 截图 | `.superpowers\sdd\quark\t15-shots\t15baidu-base-*`、`t15baidu-fix-*`、`t15installed-*`、`t15installed-timeout-*` |
| `timeout` 终态那一帧 | `.superpowers\sdd\quark\t15-shots\t15installed-timeout-poll-14-23.png` ＋ `…-23-uia.json` |
| 基线二进制 / 装机副本暂存 | `.superpowers\sdd\quark\t15-baseline\alpha-file-manager.exe`、`.superpowers\sdd\quark\t15-installed\alpha-file-manager.exe` |
| 设置夹具与备份 | `.superpowers\sdd\quark\t14-seed.cjs`、`.superpowers\sdd\quark\t14-backup\user-settings.json` |
| 仓库门禁输出 | `.superpowers\sdd\quark\t15-check.log` |

---

## 9. 下一步（按优先级）

1. **`ready` 终态**：需要真人扫码（本机或手机上完成一次百度/夸克扫码），
   自动化只能负责「代码层 + 数据层 + 像素/几何层」的证据，最后一跳往往必须由用户手工点。
2. **夸克侧的 `timeout` / `error` 终态**：换 `btn-quark` 复用本轮脚本再跑一轮（`timeout` 一轮 ≈ 19 分钟）。
3. **非中文 locale 的 toast 文案**：`locale=en` 灌夹具再跑一遍，确认 `status.*` 键在 17 个语言包里都有值。
4. 对外动作（`git push` / winget PR / GitHub Release）**等用户明确授权**再动。
