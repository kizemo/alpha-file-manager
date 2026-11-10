# focus-21 v0.5.5 修复实施计划(三轮审阅收敛版)

**日期**:2026-10-07 13:50
**输入**:
- `2026-10-07-focus-21-final-synthesis-with-claude-review.md`(我方终版)
- Claude 二轮审阅 `E:/办公文件/G1销售管理/00data_analysis/doc-review-focus-21-final-synthesis-2026-10-07.md`
**状态**:Phase 1-3 已完成,Phase 4 待授权

---

## 0. 本轮采纳与新增

### 0.1 采纳 Claude 二轮的 3 条(全部验证后采纳)

| # | Claude 建议 | 验证 | 采纳 |
|---|---|---|---|
| **C-1** | §4.2 压缩注释「治标不治本」 | ✅ **成立**(见 §1) | ✅ 已修:引入**动态扫描器** |
| **C-2** | §7.3 AGENTS.md 是文档不是强制 | ✅ **成立** | ✅ 改为 hook + 规则措辞修正 |
| **C-3** | §5.1/§6.3 责任不清 | ✅ **成立** | ✅ §6 明确「暂不做 P2」+ 触发条件 |

### 0.2 采纳 4 条 polish

| # | 建议 | 采纳 |
|---|---|---|
| C-4 | §4.2 措辞补「Sigma 升级后必须重跑」 | ✅ |
| C-5 | §4.4 Step 顺序调整(先压缩再验证) | ✅ |
| C-6 | §7.2 A-2 规则改「同一症状复发 ≥2 次」 | ✅ |
| C-7 | §4.4 补 A-4 严重度指标 | ✅ |

### 0.3 本轮新增的实测发现

**Claude 的 C-1 指向了一个我上一版方案的根本缺陷,而我实测确认了它:**

```
$ git log --oneline -- src/modules/extensions/runtime/sandbox.ts

9ee96cfc  Dev: lint
ba3e5a3f  ... Add .constructor() with string args to static code validator patterns
891fb146  Dev: large extension system refactor - Move to ESM - Use sandbox to fully isolate extensions
```

**`sandbox.ts` 在 git 历史中被改过,而且是往「增加 pattern」的方向改的。**

**我上一版的 `scan-sandbox.cjs` 把 19 条正则硬编码在脚本里 —— 那是 2026-10-07 的快照。**

**后果:Sigma FM 一旦升级改了 sandbox,我的 4 个 hook 全部静默失效,extension 再次被静默拒绝。**

**这正是「同一个坑复发两次」的机制性原因 —— 我修的是症状(1 行 `window.`),连防线(4 hook)本身都是版本绑定的。**

---

## 1. C-1 的完整验证与修正

### 1.1 Claude 的论证

> 「注释从 90 行压缩到 4 行,风险降低了但没消除。sandbox 是 `pattern.test(code)` 对原始字符串匹配,任何自然语言措辞都可能中招。**未来 Sigma 升级正则时**,新加入的词中任一中招。」
> 「只是验证当前不违规。下次 Sigma 升级,这次 4 行注释又可能违规,还得再压缩。」

### 1.2 我的实测确认

**`ba3e5a3f` 那次 commit 的实际 diff**:
```diff
+++ b/src/modules/extensions/runtime/sandbox.ts
+    {
+      pattern: /\.constructor\s*\(\s*['"`]/g,
+      message: 'Dynamic code construction via .constructor() is not allowed',
+    },
```

**这就是活生生的证据** —— 上游加了一条新 pattern `.constructor('`。如果当时有 extension 的注释里写了 `.constructor(`,它就会被静默拒绝。

**当前 sandbox.ts 状态**:
```
SHA256 : F2337F5A0922D85478CD6394A25F391E1C1D3073B9CD52E2604F8671AF815CE5
size   : 7274 B
mtime  : 09/25/2026
Sigma FM installed version : 2.2.0
```

### 1.3 修正:动态扫描器(已实现并自测)

**新建** `scripts/scan-sandbox-dynamic.cjs`:

```javascript
// 直接 parse sandbox.ts,提取所有 pattern: /regex/flags 字面量
const arrayMatch = sandboxSrc.match(/const\s+dangerousPatterns\s*=\s*\[/);
const scope = arrayMatch ? sandboxSrc.slice(arrayMatch.index) : sandboxSrc;
const entryRe = /pattern:\s*(\/(?:\\.|[^/\n\\])+\/[gimsuy]*)\s*,\s*message:\s*(['"`]).../g;
```

**三个关键设计**:

| 设计 | 为什么 |
|---|---|
| **从源码动态提取** | Sigma 改了 pattern,扫描器自动跟随 |
| **提取失败 → exit 2**(不是 0) | 防止「结构变了提不出 pattern → 误报 VALID」 |
| **打印 pattern 数量** | 输出 `patterns found: 19`,数量突变会立刻可见 |

**自测结果**:

```
# 当前 extension(应复现 1 violation)
patterns found : 19 (从源码动态提取,非硬编码)
[VIOLATION] /\bwindow\s*\./g  (1)
    line 7: "window."  [in comment]
RESULT: INVALID (1 violations)                          ✅ 与硬编码版一致

# 干净文件(应 VALID)
RESULT: VALID (0 violations / 19 patterns)             ✅

# 路径错误(应 FATAL,退出码 2)
[FATAL] target not found: nonexistent.js              ✅ fail-safe
```

**取代关系**:`scan-sandbox-dynamic.cjs` **取代** `scan-sandbox.cjs`,后者保留作为「动态提取失败时的降级参照」。

---

## 2. Phase 4 实施计划(最终)

### 2.1 P0-1:主修复(1 行)

`release/extension/dist/index.js:7`:
```javascript
- *   - H2 works without foreground; UIA SetValue does not require foreground window.
+ *   - H2 works without foreground; UIA SetValue does not require foreground focus.
```

### 2.2 P0-2:压缩头部注释(Claude C-5 采纳:先做这个)

`release/extension/dist/index.js:1-90` → 4 行:

```javascript
/**
 * Focus Sync — Sigma FM extension entry point.
 *
 * Sends the active folder to a local sidecar process over HTTP.
 * Sandbox constraints: see scripts/scan-sandbox-dynamic.cjs
 */
```

**⚠️ 措辞说明(C-4 采纳)**:
> 这 4 行在 **Sigma FM 2.2.0 + 当前 sandbox.ts** 下 0 violations。
> **每次 Sigma FM 升级后必须重跑 `scan-sandbox-dynamic.cjs`** —— sandbox pattern 可能新增
> (历史实证:`ba3e5a3f` 就新增了 `.constructor()`)。

**为什么仍然保留 4 行而不是 0 行**:完全无注释会让后续维护者不知道为什么这个文件如此脆弱。4 行里已包含「去哪查」的指引。

### 2.3 P0-3:四 hook 防御(全部改用动态扫描器)

#### Hook 1 — local build:`sigma-file-manager/scripts/build-with-sidecar.ps1`

```powershell
# 插入位置:脚本开头,复制 spike.exe 之前
$extensionJs = Join-Path $RepoRoot "release\extension\dist\index.js"
if (Test-Path $extensionJs) {
    Write-Host "==> Validating extension against Sigma FM sandbox (dynamic)"
    & node (Join-Path $RepoRoot "scripts\scan-sandbox-dynamic.cjs") $extensionJs
    if ($LASTEXITCODE -ne 0) {
        Write-Error @"
SANDBOX VALIDATION FAILED (exit $LASTEXITCODE).
Sigma FM validateExtensionCode will REJECT this extension and it will
load SILENTLY-NEVER — no UI error, no notification, user sees "nothing happens".
Patterns are read LIVE from sandbox.ts, so this is never stale.
"@
        exit 1
    }
    Write-Host "==> Extension sandbox validation PASSED"
}
```

#### Hook 2 — deploy:`release/extension-installer/register.ps1` + `manual-install.ps1`

```powershell
# 在复制 index.js 到 %APPDATA% 之前
$src = Join-Path $PSScriptRoot "..\extension\dist\index.js"
& node (Join-Path $PSScriptRoot "..\..\scripts\scan-sandbox-dynamic.cjs") $src
if ($LASTEXITCODE -ne 0) { throw "Refusing to deploy invalid extension" }
```

**理由**:build 之后文件可能被人手改,deploy 时再验一次。

#### Hook 3 — pre-commit(**需单独授权**,你此前要求「不要 commit git」)

`.git/hooks/pre-commit`:
```bash
#!/bin/sh
cd "$(git rev-parse --show-toplevel)"
node scripts/scan-sandbox-dynamic.cjs release/extension/dist/index.js || {
  echo "extension sandbox validation failed — refusing commit"
  exit 1
}
```

#### Hook 4 — 手动全量体检

```powershell
Get-ChildItem -Recurse -Filter "index.js" -Path release -ErrorAction SilentlyContinue |
  ForEach-Object { node scripts\scan-sandbox-dynamic.cjs $_.FullName }
```

**4 个 hook 任何一个失效,剩下 3 个仍能 catch。**

### 2.4 实施序列(C-5 采纳:先压缩再验证)

```
Step 1  压缩 index.js:1-90 头部注释 → 4 行
Step 2  node scripts/scan-sandbox-dynamic.cjs release/extension/dist/index.js
        → 必须 VALID (0 violations / 19 patterns)
Step 3  改 index.js:7  window. → focus.
        (注:压缩后 line 编号会变,此时第 7 行是 "Sends the active folder..."
         → window. 那行已随注释被删掉,Step 3 可能变成 no-op)
Step 4  再跑一次 scan-sandbox-dynamic.cjs → 确认 VALID
Step 5  修改 build-with-sidecar.ps1(Hook 1)
Step 6  修改 register.ps1 / manual-install.ps1(Hook 2)
Step 7  (需授权)安装 pre-commit hook
Step 8  ★ 完全退出 Sigma FM(任务栏右键退出,不是关窗口)
Step 9  启动 Sigma FM
Step 10 DevTools → Sources → 页面 → tauri.localhost → 看 worker 节点是否出现
Step 11 Console 筛选 "Failed to load extension" → 应无输出
Step 12 Console 切 worker scope → 应见 [focus-sync] activate START
Step 13 curl /health → last_ping_received_ts > 0
Step 14 Sigma FM 导航 → worker console 见 PUSH raw=
Step 15 打开 Save As → 观察地址栏/目录
```

### 2.5 分支判定

| Step 10 | 结论 |
|---|---|
| 无 worker 节点 | H0 证伪 → 回 Phase 1 查其他 gate |
| 有 worker + Step 13 = 0 | H0 成立但有第二阻断点 → 查 `index.js` 的 `loadBool` 无 catch |
| 有 worker + Step 13 > 0 | ✅ 根因确认,功能恢复 |

---

## 3. C-2 的修正:AGENTS.md 是文档不是强制

### 3.1 Claude 的论证

> 「AGENTS.md 是**文档,Claude 不会自动强制读它**。即使写了规则,Claude 在每次会话可能忽略 —— 除非有 system hook。」
> 「触发条件『准备提出任何修复方案之前』—— Claude 通常在用户报『有 bug』后**自动开始修复**,不会停下来加载 skill。这条规则只在 Claude 自我审计时有效。」

**判定:成立。** 文档是「建议」,hook 才是「强制」。

### 3.2 修正:三层强制

#### 层 1 — `AGENTS.md`(软约束,给人和 AI 读)

```markdown
## Bug 诊断强制规则

**触发条件**(任一满足):
- 用户报告功能不正常
- **同一症状复发 ≥2 次**(可观测,不是主观判定)
- 准备提出任何「修复方案」之前

**必须动作**:
1. 加载 `systematic-debugging (from superpowers)` skill
2. 完成 Phase 1-3 后才允许提 Phase 4
3. 诊断结论必须标注证据强度(直接观测 / 强推断 / 假设)
```

**C-6 采纳**:「修复失败」(主观)→「**同一症状复发 ≥2 次**」(可观测,用户可判断)。

#### 层 2 — `pre-commit` hook(已在 §2.3 Hook 3)

**真正会触发的强制点** —— commit 时校验,不由 AI 自觉决定。

#### 层 3 — CI / `package.json` script(需用户确认是否有 CI)

`sigma-file-manager/package.json`:
```json
{
  "scripts": {
    "validate:extension": "node ../scripts/scan-sandbox-dynamic.cjs ../release/extension/dist/index.js",
    "prebuild": "npm run validate:extension"
  }
}
```

**`prebuild` 会在 `npm run tauri build` 前自动触发** —— 这比 pre-commit 更靠前,能在 build 阶段就拦住。

### 3.3 关于 Claude 建议的 `PreToolUse` hook

Claude 建议:
```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Write|Edit|Bash",
      "hooks": [{
        "type": "command",
        "command": "echo '触发 systematic-debugging skill' >> ~/.claude/reminder.log"
      }]
    }]
  }
}
```

**我保留意见,未采纳**:

| # | 理由 |
|---|---|
| 1 | `matcher: "Write\|Edit\|Bash"` 会匹配**所有**写操作(包括写文档),噪音过大 |
| 2 | 写日志到 `~/.claude/reminder.log` 是**旁路提示**,不阻断 —— 约束力 ≈ 0 |
| 3 | 该配置属于 Claude Code 运行时配置,不在本 repo 内,**不在本项目可控范围** |
| 4 | 更有效的做法是 `package.json` 的 `prebuild` —— 它在**构建必经路径**上,AI 无法绕过 |

**这是一个「不采纳但记录理由」的案例,不是遗漏。**

---

## 4. C-3 的修正:休眠缺陷的排期与责任

### 4.1 Claude 的论证

> 「『必须先完成架构决策』—— 但谁负责?什么时间窗口?是否在 sprint backlog?§6.3 提了问题但**没回答**。」
> 「不能悬而未决。」

**判定:成立。** 我上一版确实把架构决策悬置了。

### 4.2 修正:明确决策

| 项 | 决策 | 理由 |
|---|---|---|
| **P2-1 HTTP → stdio** | **暂不做** | 风险中等,收益中等(省 340 行);当前功能未验证通过前不做架构变更 |
| **P2-2 53 KB PowerShell → Sigma 原生 binary** | **暂不做** | 需先修 N-2(`__binaries.path = null`),而 N-2 修不修取决于 P2-1 是否做 —— 依赖链 |

**决策原则**(本次确立,后续遵循):
> **在 Phase 4 的功能恢复验证通过之前,不做任何架构变更。**
> 理由:架构变更会让「功能是否恢复」这个判断变得更难 —— 混着改就又回到了 100+ 轮的困局。

### 4.3 休眠缺陷的触发条件(明确化)

#### N-2:`__binaries["focus-sync-sidecar"].path = null`

```markdown
**激活条件**:任何转向「由 extension 启动 sidecar」的架构变更
            (sigma.shell.runWithProgress / sigma.binary.getPath 路线)
**激活前提**:必须先做 P2-1 或 P2-2 的决策
**检测方法**:
  $j = Get-Content "$env:APPDATA\...\user-extensions.json" -Raw | ConvertFrom-Json
  $j.installedExtensions.'kizemo.focus-sync'.settings.customSettings.__binaries |
    ConvertTo-Json -Depth 5
  # 检查每个 entry 的 path 是否为 null
**当前状态**:休眠 —— extension grep `sigma.binary` 零匹配
```

#### N-3:`build_http_client()` 无 `.no_proxy()`

```markdown
**激活条件**:用户开启系统代理(实测 ProxyServer=127.0.0.1:7897 已配置,仅 ProxyEnable=0)
**检测方法**:
  Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings" |
    Select-Object ProxyEnable, ProxyServer
  # ProxyEnable 0 → 1 即激活
**验证方式**:开启代理后 curl /health + 观察 extension console
**若确认受影响**:向 Sigma FM 提 issue 要求 build_http_client 加 .no_proxy()
**当前状态**:休眠 —— ProxyEnable=0
```

---

## 5. C-7 采纳:严重度指标(A-4)

### 5.1 Claude 的论证

> 「缺一条 A-4 = 如何度量『问题严重度』。根因的修复价值 = (消除的潜在损失时间) / (修复+复发总成本)。这条决定 bug 优先级排序。」

### 5.2 建立指标

**定义**:
```
严重度 = 复发频率 × 单次排查成本 / 修复成本
```

**本项目实测数据**:

| Bug 类型 | 复发次数 | 单次排查 | 修复成本 | 严重度 | 优先级 |
|---|---|---|---|---|---|
| **sandbox 拒绝** | **2 次** | ~4 小时 | **1 行** | **极高** | **P0** |
| 部署/路径不一致 | ~3 次 | ~1.5 小时 | ~10 行 | 中 | P1 |
| UIA 元素识别 | ~5 次 | ~1 小时 | ~5 行 | 中 | P1 |
| sidecar gate/sentinel | ~3 次 | ~1 小时 | ~5 行 | 中 | P1 |
| extension PUSH 不来 | 1 次 | ~2 小时 | 未知 | 低(未复发) | P2 |

**结论:sandbox 类的严重度比其它高一个数量级** —— 复发 2 次 × 4 小时排查 ÷ 1 行修复 = **8 : 1 的投入产出比**。

**这解释了为什么它值得投入 4 hook 防护(30 行)。**

### 5.3 对未来的约束

> **新 bug 进来时,先算这个比值。**
> 修复成本 > 排查成本 / 3 的 bug,说明它可能不该「修」而该「重设计」。
> 反之,像 sandbox 这样的(1 行修复 vs 4 小时排查),**必须立刻修 + 加防线**。

---

## 6. 对用户三问的最终回答

### 6.1 「是否偏移过多?」

| 层 | 判定 | 依据 |
|---|---|---|
| **功能层**(894 行 UIA/COM) | **不偏移** | extension API 无 Win32 能力(实测 33 模块 + `input_simulation.rs` 全文) |
| **通信层**(418 行 HTTP) | 可裁(P2) | 改 stdio 省 340 行 |
| **部署层**(53 KB) | 可裁(P2) | Sigma 原生 binary 可省 40 KB,但省不掉 register.ps1 |
| **文档层**(90 行注释) | **有害,已裁** | 本次故障直接原因 |
| **防线层**(构建校验) | **缺失,已补** | 复发 2 次的机制性原因 |

**判定:偏移集中在集成层,功能层是技术必然。**

### 6.2 「任务量过大?」

| 成本 | 量 | 可压缩? |
|---|---|---|
| 根因修复 | 1 行 | — |
| 压缩注释 | ~15 行 | — |
| 4 hook 防护 | ~30 行 | — |
| 动态扫描器 | ~150 行 | 已写好,复用 |
| **单次排查** | **~4 小时** | ❌ **不可压缩** |
| **复发** | **2 次 × 4 小时 = 8 小时** | ✅ **靠 4 hook 归零** |

**准确表述:单次成本合理,复发成本极高。防护投资回报率 8:1,值得投。**

### 6.3 「总是有 bug?」

**不是「总是」,是「有一类缺防线」。**

按 §5.2 的严重度表,复发 2 次以上的只有 sandbox、部署、UIA 识别、gate/sentinel 四类。**每一类复发的原因都是同一个:没有自动化防线。**

**本次补的 4 hook 只覆盖 sandbox 类。** 另三类(部署/路径/UIA/gate)仍无防线 —— 但它们的修复成本都在 5-10 行,复发 1-3 次,优先级 P1。

---

## 7. 沉淀规则(本轮新增 2 条)

### 规则 79 — 校验器不能硬编码被校验方的规则

```markdown
### 规则 79 — 扫描器必须动态读取被扫描方的规则,不能硬编码快照

**触发**:2026-10-07,Claude 二轮审阅指出
  scan-sandbox.cjs 把 19 条 sandbox 正则硬编码,那是 2026-10-07 的快照

**实证**:sandbox.ts 在 git 历史中被改过,且是往「增加 pattern」方向:
  ba3e5a3f  Add .constructor() with string args to static code validator patterns
  891fb146  Move to ESM - Use sandbox to fully isolate extensions

**后果**:Sigma FM 升级改 pattern → 4 个 hook 全部静默失效
        → extension 再次被静默拒绝 → 第三次复发

**规则**:
1. 任何「扫描/校验」工具,规则必须从被校验方源码【动态提取】
2. 提取失败时必须 exit 2(非 0),不能误报「通过」
3. 输出必须包含「规则条数」,数量突变要立刻可见
4. 硬编码快照只能作为降级参照,不能作为主路径
```

### 规则 80 — 休眠缺陷必须写清触发条件与依赖链

```markdown
### 规则 80 — 「暂不做」也要写清依赖链,不能悬而未决

**触发**:2026-10-07,Claude 指出我写「留待架构决策」但没说谁做、何时做

**规则**:
1. 任何 P2 项必须明确标注:
   - 状态:暂不做 / 排队中 / 已排期
   - 依赖:哪些前置项完成后才能动
   - 激活条件:什么情况/事件会激活它
   - 检测方法:如何知道它变成了现实
2. 「等架构决策」不是合法状态 —— 必须写成「等 P2-1 的决策,而 P2-1 依赖 X」
3. 本项目原则:功能恢复验证通过之前,不做任何架构变更
```

---

## 8. 元数据

| 项 | 值 |
|---|---|
| **输入** | Claude 二轮审阅 `doc-review-focus-21-final-synthesis-2026-10-07.md` |
| **采纳** | C-1(动态扫描器)/ C-2(三层强制)/ C-3(排期明确)/ C-4(措辞)/ C-5(顺序)/ C-6(规则措辞)/ C-7(严重度指标) |
| **不采纳** | Claude 建议的 `.claude/settings.json` PreToolUse hook —— 记录理由(§3.3) |
| **本轮实测** | sandbox.ts git 历史变更 + `.constructor()` diff + 动态扫描器实现 + 3 项自测 |
| **新增工具** | `scripts/scan-sandbox-dynamic.cjs`(取代 `scan-sandbox.cjs`) |
| **新规则** | 79(动态提取规则)/ 80(休眠缺陷依赖链) |
| **P0 工作量** | 1 行 + 15 行 + 30 行 = 46 行 |
| **P0 总耗时** | ~40 分钟(含 Sigma FM 重启) |

---

## 9. 一句话 prompt(新会话接力)

```
读 docs/superpowers/plans/2026-10-07-focus-21-phase4-implementation.md(必读)。

采纳 Claude 二轮 7 条建议,核心修正是:
  C-1 「硬编码 19 条正则」是根本缺陷 → 已实现 scan-sandbox-dynamic.cjs
     从 sandbox.ts 动态提取 pattern,提取失败 exit 2
     实证:sandbox.ts 在 git 历史中改过
       ba3e5a3f  Add .constructor() to static code validator patterns
     ⇒ Sigma 升级改 pattern 后,旧扫描器会静默失效导致第三次复发

Phase 4 P0 三项(46 行,~40 分钟):
  1. 压缩 index.js:1-90 头部注释 → 4 行
  2. node scripts/scan-sandbox-dynamic.cjs → 必须 VALID
     (注:压缩后含 window. 的行已删,原「改 line 7」可能变成 no-op)
  3. 4 hook 防护,全部改用动态扫描器
     Hook1 build-with-sidecar.ps1 / Hook2 register.ps1+manual-install.ps1
     Hook3 pre-commit(需单独授权)/ Hook4 手动全量
  4. ★ 完全退出 Sigma FM → 重启 → Sources 面板看 worker 节点
  5. curl /health → last_ping_received_ts > 0

P2 明确「暂不做」:HTTP→stdio / 部署简化
  理由:功能恢复验证通过前不做任何架构变更
  依赖链:P2-2 依赖 N-2(__binaries.path=null)修复,而 N-2 依赖 P2-1 决策

不采纳:.claude/settings.json PreToolUse hook(不在 repo 可控范围 + 约束力≈0)
  替代:package.json 的 prebuild script(在构建必经路径上,AI 无法绕过)

沉淀 Rule 79(动态提取规则)/ 80(休眠缺陷依赖链)。
```

---

## 10. 当前状态

**未修改任何产品代码。**

已创建并自测的工具:
- `scripts/scan-sandbox-dynamic.cjs` — **动态提取 + fail-safe**(推荐用这个)
- `scripts/scan-sandbox.cjs` — 硬编码快照(降级参照)
- `scripts/check-extension-gates.cjs` — 22 项全链路 gate

**等你授权后执行 Phase 4 的 P0 三项。**
其中 Hook 3(pre-commit)需单独授权 —— 你此前明确要求「不要 commit git」。
