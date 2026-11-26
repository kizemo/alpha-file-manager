# focus-21 终版分析与优化(整合 Claude 审阅 + 实测反驳)

**日期**:2026-10-07 13:00
**输入**:Claude 审阅 `E:/办公文件/G1销售管理/00data_analysis/doc-review-focus-21-systematic-debugging-2026-10-07.md`
**方法**:逐条验证 Claude 的 5 条优化建议 + 对「砍掉 spike」建议做能力边界实测

---

## 0. 结论摘要

### 0.1 接受 Claude 的 4 条(已实施到本计划)

| # | Claude 建议 | 本计划采纳 |
|---|---|---|
| 优化 2 | N-1 修法单薄,改 **4 hook 防御** | ✅ §5.3 |
| 优化 3 | N-2/N-3 需排期 + 监控 | ✅ §6 |
| 优化 4 | 用户问题需拆解回答 | ✅ §1 |
| 优化 5 | 「为什么会跳过 skill」未治本 | ✅ §7.3 |

### 0.2 部分接受优化 1(但 Claude 的推论错了)

Claude 说:
> 「如果 spike 不再提供价值,应砍掉 spike,直接用 Sigma FM extension 单边架构」

**我实测了,这个建议技术上不可行。** 详见 §2。

### 0.3 补 Claude 漏掉的关键事实

| # | Claude 漏了什么 |
|---|---|
| **M-1** | 没验证 Sigma FM extension API 到底有没有 Win32 能力 —— 我验证了:**没有** |
| **M-2** | 把「extension 平台引入 sandbox 约束」说成复杂度飙升 —— 实际是**sigma 的必要约束**,不是我们的选择 |
| **M-3** | 没量化「根因 vs 排查成本」的真实比例(它说"1 行换 86 篇"是病态,但没算排查的**不可压缩**部分) |
| **M-4** | 建议里出现了「Rachel」「composite alpha」「.huashu-product-watchlist.md」等**与本项目无关的内容**(疑似模板残留) |

---

## 1. 直接回答用户的问题(Claude 优化 4)

### 1.1 「是否偏移过多?」

**偏移是真的,但不是 Claude 说的那种偏移。**

| 维度 | spike 原始 | 当前 | 判定 |
|---|---|---|---|
| 对话框控制(UIA/COM) | 176 行 PathWrap | 894 行 | **必要** —— 这是功能本体 |
| 进程模型 | 单 exe | sidecar + extension Worker + Sigma 主进程 | **必要** —— 见 §2 |
| HTTP 通信 | 无 | 418 行 | **可裁** —— 改 stdio 可省 ~340 行 |
| 部署 | 无 | 53 KB PowerShell + NSIS | **可裁** —— Sigma 原生 binary 系统可替代 |
| 头部注释 | 无 | 90 行 changelog | **有害** —— 本次故障直接由它引起 |
| 构建校验 | 不需要 | **完全缺失** | **有害** —— 复发两次的根因 |

**判定:偏移集中在「集成层」而非「功能层」。功能层(894 行)是该功能必需的;集成层(HTTP + 部署 + 注释管理)是本项目的自加负担,且大部分可裁。**

### 1.2 「任务量过大?」—— 算真实比例

| 成本项 | 工作量 | 可压缩? |
|---|---|---|
| **根因修复** | **1 行**(`window.` → `focus.`) | — |
| 防护(4 hook) | ~30 行 | — |
| 扫描器 | ~80 行(已写) | 一次性 |
| **压缩 90 行注释** | ~15 行 | 一次性 |
| **删除可裁的 HTTP/部署层** | ~800 行 | 一次性,但需重构 |
| ──── | ──── | ──── |
| **复发一次的排查成本** | **~4 小时 / 5 份文档** | ❌ **不可压缩** |

**关键认知:根因 1 行 ≠ 排查 1 行。**

**排查成本不可压缩**,因为它包括:发现症状 → 定位调用链 → 读 Sigma FM 源码 → 逐字节复刻校验逻辑 → 验证工具 → 交叉印证。这是 systematic-debugging 的固有成本。

**能压缩的是「复发」** —— 靠 4 hook 防护。

**所以「任务量过大」的准确表述是:单次成本合理(1 行),但复发成本极高(4 小时/次),已复发 2 次。防护投资回报率极高。**

### 1.3 「总是有 bug?」

**不是「总是有 bug」,是「有一类 bug 缺少防线」。**

| 类别 | 次数 | 防线 |
|---|---|---|
| sandbox 拒绝 | **2 次** | ❌ 无 → 本次补 4 hook |
| 部署/路径 | 多次 | ⚠️ 有脚本但无校验 |
| UIA 元素识别 | 多次 | ⚠️ 有测试但无实测 |
| extension PUSH | 1 次 | ⚠️ 无 |
| sidecar 逻辑(gate/sentinel) | 多次 | ✅ 有测试 |

**唯一缺防线的是 sandbox 类**,而它恰好是最隐蔽的(静默失败,无 UI 提示,用户只看到「没反应」)。

---

## 2. 反驳 Claude 的「砍掉 spike」建议(实测证据)

### 2.1 Claude 的推论

> 「如果 spike 不再提供价值,应砍掉 spike,直接用 Sigma FM extension 单边架构,N-1/N-2 这类系统性缺陷将大幅减少」

### 2.2 实测:Sigma FM extension API 没有任何 Win32 能力

**实测 1** — 扫描 Sigma FM 主项目全部 `.rs` 和 `.ts`,搜索对话框控制相关 API:

```powershell
Get-ChildItem -Recurse -Include "*.rs","*.ts" `
  -Path sigma-file-manager\src, sigma-file-manager\src-tauri |
  Select-String "UIAutomation|IFileDialog|AccessibleObjectFromWindow|SendInput|EnumWindows|SetForegroundWindow"
```

**结果**:
```
input_simulation.rs:8      ← 只有 SendInput
input_simulation.rs:34
```

**读全文后确认**(`input_simulation.rs`,1831 B):
```rust
6:  fn simulate_paste_shortcut_sync() -> Result<(), String> {
7:      use windows::Win32::UI::Input::KeyboardAndMouse::{
8:          SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
9:          VIRTUAL_KEY, VK_CONTROL, VK_V,        // ← 只有 Ctrl+V
10:     };
...
34:     let sent_count = SendInput(&inputs, ...);   // 模拟粘贴
```

**Sigma FM 主项目唯一的 Win32 能力是「模拟 Ctrl+V 粘贴」。没有 UIA,没有 IFileDialog,没有窗口枚举。**

**实测 2** — 列出 Sigma FM extension API 的全部 33 个模块:
```
binary-download-counts / binary-install-activity / clipboard-* / configuration /
create-binary-api / create-commands-api / create-context-api / create-context-menu-api /
create-dialog-api / create-extension-api / create-fs-api / create-http-api /
create-i18n-api / create-path-api / create-platform-api / create-settings-api /
create-shell-api / create-sidebar-api / create-storage-api / create-toolbar-api /
create-ui-api / create-view-api / dialog-state / extension-context / keybindings /
modal-state / path-api-core / platform / registrations / web-clipboard-linux
```

**没有任何一个暴露 UIA / IFileDialog / 窗口查找。**

**实测 3** — 我们的 extension 实际使用的 API:
```
sigma.ui         x10
sigma.storage     x6
sigma.http        x6
sigma.commands    x4
sigma.context     x3
sigma.fs          x1
sigma.toolbar     x1
sigma.registry    x1
```

**结论:sidecar 的能力无法被 extension 替代 —— 因为 extension 根本没有那个能力。**

### 2.3 因此 Claude 的建议不可行

| Claude 设想 | 技术现实 |
|---|---|
| 「砍掉 spike,用 Sigma FM extension 单边」 | ❌ extension 无法控制 Windows 对话框 |
| 「让 Sigma 直接调用 Synapse」 | ❌ Synapse 不存在于本项目 |
| 「N-1/N-2 会大幅减少」 | ✅ 这部分对,但 N-1 是构建管线问题,与 spike 存在无关 |

**N-1(构建管线无校验)与 spike 无关** —— 如果没有 sidecar、没有 extension,就没有 `release/extension/dist/index.js`,也就不需要校验它。**Claude 把「extension 平台带来的新职责」误判为「spike 带来的负担」。**

### 2.4 真正可裁的部分(修正版建议)

| 可裁项 | 节省 | 风险 | 优先级 |
|---|---|---|---|
| extension 头部 90 行 changelog | ~15 行注释 | 零 | **P0**(本次故障直接原因) |
| `http_server.rs` → stdio | ~340 行 | 中(需改 extension 通信) | P2 |
| 53 KB PowerShell → Sigma 原生 binary | ~40 KB | 中 | P2 |
| 4 hook 防护 | +30 行 | 零 | **P0** |
| ~~砍掉 spike~~ | — | **不可行** | ❌ |

**修订后的架构建议**:
> **保留 spike**(它的 Win32 能力不可替代),**裁剪集成层**(注释瘦身 + 后续 HTTP→stdio + 部署简化),
> **补齐防线**(4 hook),**不重构核心功能层**。

---

## 3. Claude 遗漏的关键事实(实测补充)

### M-1:sidecar 的能力边界(Claude 完全没查)

Claude 花了大量篇幅论证「spike 是负担」,**但没查 Sigma FM extension API 到底有什么**。

**实测结论:extension API 完全没有 Win32 能力。** 这是决定性的 —— 它使「砍掉 spike」的建议不可行。

**教训**:评估「某个组件是否可替代」时,**必须先测量替代方的能力边界**,而不是假设。

### M-2:sandbox 不是我们引入的复杂度

Claude 说:
> 「spike 不需要 sandbox → 二次开发引入了 sandbox 概念 = 复杂度飙升」

**事实核对**:
- sandbox 是 **Sigma FM 主项目**的机制(`sandbox.ts` 在 `sigma-file-manager/src/` 下)
- 我们没用 UIA 就不会触发它,但我们**用 extension 就必然受它约束**
- 这跟「spike 需不需要 sandbox」无关 —— **任何** Sigma FM extension 都受此约束

**判定:M-2 的归因错误。** sandbox 约束是使用 Sigma FM extension 平台的**固有成本**,不是 spike 带来的。

### M-3:根因 vs 排查成本的真实比例

Claude 说:
> 「1 行修复需 86 篇 handoff + 2 个扫描器 + 1 份 final report —— 这是病态」

**修正**:
- 86 篇文档**不是这一次排查产生的**,是过去 86 轮的累积
- 本次排查的实际产出:1 份报告 + 2 个工具,约 2 小时
- **不可压缩的部分**:读 Sigma FM 源码、逐字节复刻校验、验证工具、交叉印证用户截图
- **可压缩的部分**:复发(靠防护)

**所以「病态」的判断方向对,但归因错了** —— 不是「1 行修复要 86 篇」,而是「**缺少防线导致同一 1 行问题复发 2 次,每次排查 2-4 小时**」。

### M-4:审阅文档中的模板残留

Claude 审阅里出现了与本项目无关的内容:

| 出现位置 | 内容 | 问题 |
|---|---|---|
| §2.3 | 「如果 spike 不再提供净价值,**Rachel** 是过度工程」 | 「Rachel」与本项目无关,疑似模板残留 |
| §2.3 | 「本项目 spike 是 **composite alpha**?」 | 疑似其他项目的术语 |
| 优化 3 | 「写一个 `.huashu-product-watchlist.md`」 | 疑似其他项目的约定 |

**这些不影响核心判断,但说明审阅文档可能是模板套用,未完全针对本项目。** 本报告不采纳这些引用。

---

## 4. Phase 4 实施计划(修订版)

### 4.1 主修复(不变,P0)

`release/extension/dist/index.js:7`:
```javascript
- *   - H2 works without foreground; UIA SetValue does not require foreground window.
+ *   - H2 works without foreground; UIA SetValue does not require foreground focus.
```

### 4.2 压缩头部注释(新增,P0)

**理由**:90 行 changelog 是 sandbox 雷区 + 每次 bump 都要维护 + 对用户零价值。

```javascript
/**
 * Focus Sync v0.5.5 — Sigma FM extension entry point.
 *
 * Sends the active folder to a local sidecar process over HTTP.
 * Implementation notes: docs/superpowers/plans/
 */
```

**⚠️ 实施时必须用 `scan-sandbox.cjs` 验证压缩后的版本 0 violations。**

### 4.3 四 hook 防御(采纳 Claude 优化 2,P0)

#### Hook 1:`build-with-sidecar.ps1`(local build)

```powershell
# 插入位置:脚本开头,复制 spike.exe 之前
$extensionJs = Join-Path $RepoRoot "release\extension\dist\index.js"
if (Test-Path $extensionJs) {
    Write-Host "==> Validating extension against Sigma FM sandbox patterns"
    & node (Join-Path $RepoRoot "scripts\scan-sandbox.cjs") $extensionJs
    if ($LASTEXITCODE -ne 0) {
        Write-Error @"
SANDBOX VALIDATION FAILED.
Sigma FM validateExtensionCode (sandbox.ts:207-284) will REJECT this
extension and it will load SILENTLY-NEVER — no UI error, no notification.
Fix: remove sandbox-forbidden patterns (see scan-sandbox.cjs output).
"@
        exit 1
    }
    Write-Host "==> Extension sandbox validation PASSED"
}
```

#### Hook 2:deploy 脚本(`register.ps1` / `manual-install.ps1`)

```powershell
# 在复制 index.js 到 %APPDATA% 之前
$src = "release\extension\dist\index.js"
& node "scripts\scan-sandbox.cjs" $src
if ($LASTEXITCODE -ne 0) { throw "Refusing to deploy invalid extension" }
```

**理由**:build 后文件可能被手改,deploy 时再验一次。

#### Hook 3:pre-commit hook(需用户授权)

`.git/hooks/pre-commit`:
```bash
#!/bin/sh
node scripts/scan-sandbox.cjs release/extension/dist/index.js || {
  echo "extension sandbox validation failed"
  exit 1
}
```

**注意:用户此前明确要求「不要 commit git」,此 hook 需单独授权才启用。**

#### Hook 4:一次性全量体检(手动)

```powershell
# 扫仓库内所有可能的 extension JS
Get-ChildItem -Recurse -Filter "index.js" -Path release,*.other -ErrorAction SilentlyContinue |
  ForEach-Object { node scripts\scan-sandbox.cjs $_.FullName }
```

**4 个 hook 任何一个失效,剩下 3 个仍能 catch。**

### 4.4 实施序列(修订)

```
Step 1  改 index.js:7  window. → focus.
Step 2  压缩 index.js:1-90 头部注释
Step 3  node scripts/scan-sandbox.cjs release/extension/dist/index.js
        → 必须 RESULT: VALID (0 violations)
Step 4  修改 build-with-sidecar.ps1(加 Hook 1)
Step 5  修改 register.ps1 / manual-install.ps1(加 Hook 2)
Step 6  (需授权)安装 pre-commit hook
Step 7  ★ 完全退出 Sigma FM(任务栏右键退出)
Step 8  启动 Sigma FM
Step 9  DevTools → Sources → tauri.localhost → 看 worker 节点是否出现
Step 10 Console 筛选 "Failed to load extension" → 应无输出
Step 11 Console 切 worker scope → 应见 [focus-sync] activate START
Step 12 curl /health → last_ping_received_ts > 0
Step 13 Sigma FM 导航 → worker console 见 PUSH raw=
Step 14 打开 Save As → 观察地址栏/目录
```

### 4.5 分支判定(不变)

| Step 9 | 结论 |
|---|---|
| 无 worker 节点 | H0 证伪 → 回 Phase 1 查其他 gate |
| 有 worker + Step 12 = 0 | H0 成立但有第二阻断点 → 查 `index.js:435` `loadBool` 无 catch |
| 有 worker + Step 12 > 0 | ✅ 根因确认 |

---

## 5. 休眠缺陷排期(采纳 Claude 优化 3)

### 5.1 N-2:`__binaries["focus-sync-sidecar"].path = null`

```markdown
### N-2 休眠缺陷档案

**现象**:user-extensions.json 里
  installedExtensions["kizemo.focus-sync"].settings.customSettings
    .__binaries["focus-sync-sidecar"].path = null

**影响链**:create-binary-api.ts:33-35 → getPath() 返回 null
          → 若 extension 调用 sigma.binary.getPath() 则拿到 null

**当前状态**:休眠
  证据:grep 部署的 dist/index.js,`sigma.binary` / `getPath` 零匹配
  原因:v0.3.0(2026-09-30)起 sidecar 改用 Scheduled Task 启动,
       extension 只通过 sigma.http.request 通信,不碰 sigma.binary

**激活条件**:任何转向「由 extension 启动 sidecar」的架构变更
          (例如 sigma.shell.runWithProgress / sigma.binary.getPath 路线)

**检测方法**:
  $j = Get-Content "$env:APPDATA\com.sigma-file-manager.app\user-data\user-extensions.json" -Raw | ConvertFrom-Json
  $j.installedExtensions.'kizemo.focus-sync'.settings.customSettings.__binaries |
    ConvertTo-Json -Depth 5
  # 检查每个 entry 的 path 是否为 null

**修复前置**:必须先完成架构决策(见 §6.3),不能顺手改
```

### 5.2 N-3:`build_http_client()` 无 `.no_proxy()`

```markdown
### N-3 潜在风险档案

**现象**:src-tauri/src/extensions/http.rs:31-40
        reqwest::Client::builder() 无 .no_proxy()
        → reqwest 默认读系统代理设置

**影响链**:系统代理开启 → sigma.http.request('http://127.0.0.1:37421/health')
          → 可能被代理(127.0.0.1:7897)拦截 → extension 收不到响应

**当前状态**:休眠
  实测:HKCU\...\Internet Settings
        ProxyEnable = 0        ← 代理关闭
        ProxyServer = 127.0.0.1:7897   ← 但已配置
        NO_PROXY = localhost,127.0.0.1,::1

**激活条件**:用户开启系统代理(Clash / V2Ray / 公司代理)

**检测方法**:
  Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings" |
    Select-Object ProxyEnable, ProxyServer
  # ProxyEnable 从 0 变为 1 即激活

**缓解方案(不需改 Sigma FM)**:
  extension 已在 activate 日志里提到 .no_proxy()(历史踩过坑)
  验证方式:开启代理后 curl /health + 观察 extension console
  若确认受影响 → 向 Sigma FM 提 issue 要求 build_http_client 加 .no_proxy()
```

---

## 6. 架构建议(修正版,取代 Claude 的「砍掉 spike」)

### 6.1 保留 spike 的理由(实测)

```
Sigma FM extension API 能力清单(实测 33 个模块):
  ui / storage / http / commands / context / fs / toolbar / registry / i18n
  binary / shell / dialog / view / settings / platform / path
  clipboard / sidebar / contextMenu / keybindings / configuration
  ─────────────────────────────────────────────────────
  Win32 能力:仅 simulate_paste_shortcut_sync(模拟 Ctrl+V)
  UIA / IFileDialog / 窗口枚举:❌ 全部不存在
```

**结论:sidecar 的「控制 Windows 对话框」能力,extension 无法替代。保留 spike 是技术必然。**

### 6.2 该裁的是集成层,不是功能层

| 层 | 内容 | 处置 |
|---|---|---|
| **功能层** | `uia_inject.rs` 894 行 UIA/COM 对话框控制 | **保留** —— 不可替代 |
| **通信层** | `http_server.rs` 418 行 | **可裁** —— 改 stdio 省 ~340 行(P2) |
| **部署层** | 53 KB PowerShell + NSIS | **可裁** —— Sigma 原生 binary 系统(P2) |
| **文档层** | extension 90 行 changelog | **裁** —— P0,本次故障直接原因 |
| **防线层** | 构建校验 | **补** —— P0,4 hook |

### 6.3 待决策的架构问题(不急,但要记录)

**Q:是否值得把 53 KB PowerShell 换成 Sigma 原生 binary 机制?**

**已实测的前提条件**(见 `2026-10-07-focus-21-spike-to-sigma-native-binary-verification.md`):
- ✅ `sigma.binary.*` 存在(`create-binary-api.ts`),但只有 3 个**只读**方法
- ✅ `sigma.shell.runWithProgress` 存在,需 `shell` 权限(`risk: 'highest'`)
- ⚠️ `register.ps1` 的 `__binaries` 登记**无法删除**(N-2 休眠缺陷)
- ❌ manifest `binaries` 需要 HTTP `downloadUrl` + `integrity`,本地 exe 走不通

**结论:能省 40 KB(53→13),但省不掉 register.ps1。价值中等,风险中等,建议 P2。**

---

## 7. 「为什么会跳过 skill」的治本(采纳 Claude 优化 5)

### 7.1 Claude 的观察

> 「文档第 9 节已经识别出根因:『我一直跳过了 systematic-debugging 明确要求的阶段,自己发明流程』。**但只诊断自己的遗漏,没诊断为什么会跳过**。」

### 7.2 我的回答:三个真实原因

| # | 原因 | 证据 | 对策 |
|---|---|---|---|
| **A-1** | **skill 需要主动召唤** | 连续 20+ 轮我在提方案,没调 skill | 写进 `AGENTS.md` 强制规则 |
| **A-2** | **前期症状看起来「简单」** | 每次都是「改一行就好」,看起来不需要流程 | 规则:**≥2 次修复失败 = 强制走 skill** |
| **A-3** | **没有可观测的目标状态** | `Write strategy="uia"` 看起来像成功,实际是写文本 | 规则:**修复前先定义「什么算成功」,且必须可观测** |

### 7.3 治本方案

**A-1 的对策** — 在项目根 `AGENTS.md` 加:
```markdown
## Bug 诊断强制规则

**触发条件**(任一满足):
- 用户报告功能不正常
- 同一问题已修复 ≥ 2 次仍未解决
- 准备提出任何「修复方案」之前

**必须动作**:
1. 加载 `systematic-debugging (from superpowers)` skill
2. 完成 Phase 1-3 后才允许提 Phase 4
3. 诊断结论必须标注证据强度(直接观测 / 强推断 / 假设)

**反面教材**:
- focus-21 的 100+ 轮:每轮都直接提修复,从未跑完 Phase 1-3
- 代价:同一 sandbox 拒绝复发 2 次,每次排查 2-4 小时
```

**A-2 的对策** — 「≥2 次失败 = 强制走 skill」写进上述规则。

**A-3 的对策** — 每份修复计划必须有:
```markdown
## 成功判据(必须可观测)
- 什么日志/界面变化 = 修复成功?
- 什么日志/界面变化 = 修复失败?
- 如何区分「写入成功」和「功能生效」?
```

**本项目的具体实例**:
| 判据 | 可观测吗 |
|---|---|
| `Write strategy="uia"` 出现 | ❌ 只证明写入,不证明导航 |
| dialog 当前目录变化 | ✅ 但需要用户目视 |
| `last_ping_received_ts > 0` | ✅ 完全可观测 |

---

## 8. 元数据

| 项 | 值 |
|---|---|
| **输入** | Claude 审阅 `doc-review-focus-21-systematic-debugging-2026-10-07.md` |
| **采纳** | 优化 2(四 hook)/ 3(排期+监控)/ 4(拆解回答)/ 5(治本) |
| **部分采纳** | 优化 1 —— 「评估 ROI」采纳,「砍掉 spike」**实测否决** |
| **实测反驳** | Sigma FM extension API 无任何 Win32 能力(33 模块 + `input_simulation.rs` 全文) |
| **纠正 Claude** | M-1(未查能力边界)/ M-2(sandbox 归因错)/ M-3(成本归因)/ M-4(模板残留) |
| **Phase 4 变更** | 新增注释压缩 + 4 hook(原 +8 行 → 4 层) |
| **新增** | N-2/N-3 休眠档案 + 触发条件 + 检测方法 + 3 条 AGENTS.md 规则 |

---

## 9. 一句话 prompt(新会话接力)

```
读 docs/superpowers/plans/2026-10-07-focus-21-final-synthesis-with-claude-review.md(必读)。

整合了 Claude 审阅,采纳 4 条优化,实测否决 1 条:

采纳:
  优化 2: N-1 修法从 +8 行升级为 4 hook 防御(build / deploy / pre-commit / 全量体检)
  优化 3: N-2 / N-3 建休眠档案(现象/影响链/当前状态/激活条件/检测方法/前置)
  优化 4: 直接回答用户三问(偏移/任务量/总出 bug),给出量化
  优化 5: 分析「为什么会跳过 skill」三个原因 + AGENTS.md 强制规则

否决(实测):
  Claude 说「砍掉 spike,用 Sigma FM extension 单边架构」
  实测证据:
    - Sigma FM 主项目全部 .rs/.ts 搜索 UIAutomation|IFileDialog|SendInput|EnumWindows
      → 唯一命中 input_simulation.rs,且全文 1831 B 只做「模拟 Ctrl+V」
    - extension API 33 个模块逐个列出,无一暴露 UIA / IFileDialog / 窗口枚举
  ⇒ sidecar 的「控制 Windows 对话框」能力 extension 无法替代
  ⇒ 保留 spike 是技术必然,应裁的是集成层(HTTP 418 行 / 部署 53 KB)不是功能层

纠正 Claude 的 4 处:
  M-1: 未查 Sigma FM extension API 的实际能力边界
  M-2: sandbox 是 Sigma FM 固有约束,不是 spike 带来的复杂度
  M-3: 不是「1 行修复要 86 篇」,是「缺防线导致同一 1 行问题复发 2 次」
  M-4: 审阅含模板残留(Rachel / composite alpha / .huashu-product-watchlist.md)

Phase 4 最终方案:
  P0-1 index.js:7 window. → focus.
  P0-2 压缩 index.js:1-90 头部注释(降 sandbox 雷区)
  P0-3 4 hook 防护
  P2   http_server.rs → stdio(省 340 行)
  P2   53 KB PowerShell → Sigma 原生 binary(省 40 KB)
```

---

## 10. 当前状态

**未修改任何产品代码。**

已创建/验证的工具:
- `scripts/scan-sandbox.cjs` — 逐字节复刻 19 条 sandbox 正则,已自测复现历史故障
- `scripts/check-extension-gates.cjs` — 22 项全链路 gate,全 PASS

**等你授权后执行 Phase 4 的 P0 三项。** 其中 Hook 3(pre-commit)需要单独授权 —— 你此前明确要求「不要 commit git」。
