# focus-21 Phase 4 — Sandbox Gate Implementation & Verification Record

**日期**:2026-10-07
**执行**:handoff `2026-10-07-focus-21-handoff-phase4.md` §4 的 P0 三项
**根因**:`release/extension/dist/index.js` 头部注释含 `foreground window.` → 命中 Sigma FM
`sandbox.ts` 的 `/\bwindow\s*\./g` → `loader.ts:360` throw → `loader.ts:365` 的 `new Worker()`
从未执行 → extension 从未加载 → sidecar 0 请求(`last_ping_received_ts: 0`)。

**遵守的硬约束**:未 commit git / 未安装 pre-commit / 未改 sidecar / 一次只改一处 /
Step 5 完全退出 Sigma FM 交由用户操作。

---

## 1. 基线(改动前实测)

```
$ node scripts\scan-sandbox-dynamic.cjs release\extension\dist\index.js
sandbox source : sigma-file-manager\src\modules\extensions\runtime\sandbox.ts
patterns found : 19 (从源码动态提取,非硬编码)

[VIOLATION] /\bwindow\s*\./g  (1)
    line 7: "window."  [in comment]  *   - H2 works without foreground; UIA SetValue does not require foreground window.

RESULT: INVALID (1 violations)
EXITCODE=1
```

基线与 handoff §3 的记录一致,根因逐字节复现。

> **偏差**:handoff §4 Step 1 写的是「第 1-90 行」,实测注释块实际是**第 1-109 行**
> (109 行处 `*/` 收尾)。按实际范围替换。

---

## 2. Step 1 — 压缩头部注释

`release/extension/dist/index.js` 第 1-109 行 → 6 行:

```javascript
/**
 * Focus Sync — Sigma FM extension entry point.
 *
 * Sends the active folder to a local sidecar process over HTTP.
 * Sandbox constraints: see scripts/scan-sandbox-dynamic.cjs
 */
```

**改动范围证明**(`git diff --numstat`):

```
3       106     release/extension/dist/index.js
```

新增 3 行 / 删除 106 行,净减 103 行。`/**`、` *`、` */` 三行与原文相同故计入上下文。
**零代码行被触碰**。ESM 语法检查 `node --check` 退出码 0。

**⚠️ 说明**:原注释块包含 v0.3.0 → v0.5.5 的完整变更历史(约 105 行)。按 handoff §4
要求整体压缩,历史记录未保留在 `index.js` 内。相关内容仍在
`docs/superpowers/plans/` 的历史文档里。如需保留 changelog,建议改放
`docs/extension-changelog.md`,而不是放回会被 sandbox 正则扫描的源文件。

---

## 3. Step 2 — 动态扫描验证

```
$ node scripts\scan-sandbox-dynamic.cjs release\extension\dist\index.js
patterns found : 19 (从源码动态提取,非硬编码)
RESULT: VALID (0 violations / 19 patterns)
EXITCODE=0
```

---

## 4. Step 3 — 部署到 %APPDATA%

```
repo     SHA256 = 0CEA389F98E9D7F57AF606A80165A9AD10B0DF07ED1E07A222A3F50586F58473
deployed SHA256 = 0CEA389F98E9D7F57AF606A80165A9AD10B0DF07ED1E07A222A3F50586F58473
IDENTICAL = True
deployed size = 20814
```

部署前旧文件 SHA256 = `A78E96D3268FDA3A9CBF028D5BF02B26038261FB018D62C36E53BE7519541389`,
与 handoff §3 记录一致 → 确认部署的确实是含违规注释的那一版。

**部署后独立复扫**(不信任仓库里的副本,直接扫 %APPDATA% 里的文件):

```
$ node scripts\scan-sandbox-dynamic.cjs "C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\dist\index.js"
RESULT: VALID (0 violations / 19 patterns)
EXITCODE=0
```

---

## 5. Step 4 — Hook 防护(4 个)

所有 hook 一律调用 **`scan-sandbox-dynamic.cjs`**(动态读取 `sandbox.ts`),
**不使用** `scan-sandbox.cjs`(硬编码 19 条快照,已在 `ba3e5a3f` 过期)。

### ⚠️ 实施中发现的真实缺陷(已修)

`scan-sandbox-dynamic.cjs:27` 的 `sandbox.ts` 默认路径是**相对 CWD** 的:

```javascript
const sandboxFile = process.argv[3]
  || path.join('sigma-file-manager', 'src', 'modules', 'extensions', 'runtime', 'sandbox.ts');
```

若 hook 从非仓库根目录运行,扫描器会以 `exit 2 (FATAL)` 失败 —— gate 会**误报**。
实测对比(在 `C:\Users\Duanyi\AppData\Local\Temp` 下运行):

```
A) 不传显式 sandbox.ts  → [FATAL] sandbox.ts not found … EXITCODE=2   ← 误报
B) 传显式 sandbox.ts    → RESULT: VALID (0 violations / 19 patterns) EXITCODE=0
```

**修法**:三个 hook 全部改为**显式传入 `sandbox.ts` 绝对路径**,不再依赖 CWD。

### Hook 1 — `sigma-file-manager/scripts/build-with-sidecar.ps1`

位置:`$ErrorActionPreference = 'Stop'` 之后、`$spikeExe` 之前。扫描失败 → `exit 1`。
若 `release/extension/dist/index.js` 不存在则整段跳过(该脚本也用于只构建 sidecar 的场景)。

**端到端实测**(从 `%TEMP%` 运行,验证 CWD 无关性):

```
==> Validating extension against Sigma FM sandbox (dynamic)
RESULT: VALID (0 violations / 19 patterns)
==> Extension sandbox validation PASSED
Copied spike.exe → release\extension\bin\focus-sync-sidecar.exe
Copied spike.exe → release\extension-installer\focus-sync-sidecar.exe
SCRIPT_EXITCODE = 0
BEFORE/AFTER 两个二进制 SHA256 均 4E8CEAC7…20A4  unchanged=True
```

**失败分支实测**(用故意违规的 fixture,证明 gate 不是永远放行):

```
[VIOLATION] /\bwindow\s*\./g  (1)
    line 5: "window."  [in comment]  * UIA SetValue does not require foreground window.
RESULT: INVALID (1 violations)
>>> GATE FIRED: would Write-Error + exit 1 (scanner exit 1)
```

### Hook 2 — `release/extension-installer/manual-install.ps1`

位置:`# Top-level files` 之前,**先验证后拷贝**,验证失败 `exit 1`,一个字节都不落地。

### Hook 2b — `release/extension-installer/register.ps1`

**与 handoff 规格的偏差**:handoff 说「在复制 index.js 到 %APPDATA% 之前插入」,但
`register.ps1` **根本不拷贝 index.js**(是 `installer.nsi` 拷的,register 由它在拷贝后调用)。
照抄会插到一个不存在的位置上。

**实际做法**:改为验证 **已部署的那份**
(`$appDataDir\extensions\kizemo.focus-sync\dist\index.js`)—— 也就是 Sigma FM 启动时
真正会读的文件,并放在写 `user-extensions.json` 之前(改任何东西之前先失败)。

**fail-soft 设计**:`register.ps1` 由 NSIS 安装器在**用户机器**上运行,那里没有仓库、
也可能没有 node。所以 `sandboxScanner` / `sandboxTs` / `deployedIndexJs` 任一缺失时
**跳过 gate 并打印提示**,而不是让安装器崩掉;一旦三者齐全而扫描失败,则**硬失败**。

### Hook 3 — pre-commit:**已获用户授权并安装**

见 §6b。安装在 `.git/hooks/pre-commit`,两个分支均实测通过。**未执行任何 commit。**

### Hook 4 — 手动全量体检

```powershell
# 显式传 sandbox.ts,避免 CWD 依赖导致 exit 2 误报
$sandboxTs = Join-Path $PWD 'sigma-file-manager\src\modules\extensions\runtime\sandbox.ts'
Get-ChildItem -Recurse -Filter "index.js" -Path release -ErrorAction SilentlyContinue |
  ForEach-Object {
    Write-Host "---- $($_.FullName) ($($_.Length) B)"
    & node scripts\scan-sandbox-dynamic.cjs $_.FullName $sandboxTs
  }
```

实测结果:

```
---- F:\soft\00selfmade\filemanager\release\extension\dist\index.js (20814 B)
RESULT: VALID (0 violations / 19 patterns)

TOTAL index.js under release\ = 1
INVALID files = 0
```

---

## 6. Step 5/6 — 重启与验证(实测结果)

用户于 13:51 完全退出并重启 Sigma FM(旧 PID 35832 → 新 PID **33188**,StartTime 13:51:26)。

### Step 6a — Sources 面板 worker 节点:✅ **出现**

> **这是 H0 的决定性证据。** 修复前 Sources 面板**没有任何 worker 节点**;
> 修复后出现 ⇒ `loader.ts:360` 的 throw 消失了,`loader.ts:365` 的 `new Worker()` 真正执行。

### Step 6d — `last_ping_received_ts`:仍为 0,但**这不是故障信号**(见下)

```
uptime_secs           : 21074      (sidecar 仍在跑,未重启 → 计数延续)
last_ping_received_ts : 0
first_push_received   : false
```

### ⚠️ 重要更正:Step 6d 的判据本身是错的

handoff §4 Step 6d 要求「`last_ping_received_ts` 必须 > 0(不再是 0)」才判定根因确认。
**这个判据把「health 探活」和「路径推送」搞混了。** 逐字节核对 sidecar 源码:

`sigma-listary-spike/src/state.rs:9`(模块文档自己就写明了):

```rust
//! - `last_ping_received_ts: AtomicU64` — last `/set_path` (Unix millis, 0 if never)
```

写入点**唯一** —— `state.rs:99-113` 的 `set_current_path()`:

```rust
pub fn set_current_path(&self, p: String) {
    *guard = p;
    let ts = ...;
    self.last_ping_received_ts.store(ts, Ordering::Relaxed);   // ← 唯一写点
    self.first_push_received.store(true, Ordering::SeqCst);
}
```

而 `/health` 的处理函数(`http_server.rs:321-335`)**只读快照、从不调用** `set_current_path`。

extension 的两条 HTTP 路径因此完全不对称:

| extension 侧 | HTTP | 会不会写 `last_ping_received_ts` |
|---|---|---|
| `pingSidecar()` / `checkHealth()`(`index.js:85` / `118`) | `GET /health` | **不会** |
| `pushNow()`(`index.js:57`,由 `onPathChange` 触发) | `POST /set_path` | **会** |

**结论**:`last_ping_received_ts: 0` 只说明**用户还没在 Sigma FM 里切换过目录**,
与 extension 是否加载无关。`activate()` 里 t=5s 的首探和每 15s 的 `checkHealth()`
本来就不会写这个字段 —— 即使一切完全正常,它也会一直是 0。

⇒ **不能用它判定根因**。正确的判定信号是 `first_push_received`,
且必须先在 Sigma FM 里导航目录触发一次 `POST /set_path`。这实际就是 Step 7。

### 同时更正:handoff §5 的「第二阻断点」假设不成立

§5 推测第二阻断点是「`index.js:435` 的 `loadBool`(无 catch)」。实测该函数
**有完整 catch**(`index.js:243-252`,catch 分支 `return defaultValue`)。
且 `activate()` 从 L331 到 L421(L421 `startHealthCheck()`)之间**没有任何未捕获的 await**。
⇒ 无需再按该线索排查。

### 真正的下一步 = Step 7(功能验证)

在 Sigma FM 里**导航到另一个目录**(这是触发 `onPathChange` → `POST /set_path` 的唯一方式),
然后复查 `/health`,`first_push_received` 应翻为 `true`、`last_ping_received_ts > 0`。

### Step 7 — 实测结果:✅ **根因确认,全链路打通**

用户导航目录后复查 `/health`:

```
first_push_received      : true                                    ← 翻转为 true
last_ping_received_ts    : 1791352568886  (= 2026-10-07 13:56:08) ← 正是用户导航的那一刻
uptime_secs              : 21283        (sidecar 未重启,计数连续)
active_dialogs           : 0
unsupported_dialog_count : 0
status                   : ok   version: 0.5.5   session_id: 1
```

### 最终因果链验证 —— 每一环都有直接观测

```
index.js 头部注释去除 "foreground window."
    ↓
scan-sandbox-dynamic.cjs → VALID (0 violations / 19 patterns)
    ↓
validateExtensionCode() 不再 throw  ← 【直接观测】DevTools 出现 worker 节点
    ↓
new Worker() 真正执行 → activate()
    ↓
onPathChange(用户导航目录)
    ↓
POST /set_path  → sidecar 收到
    ↓
first_push_received = true, last_ping_received_ts = 13:56:08  ← 【直接观测】
```

handoff §5 分支判定表命中第三行:**有 worker 节点 + ping>0 → ✅ 根因确认,功能恢复**。

**证据强度**:从 handoff 原先标注的【强推断】(缺 `loader.ts:403` 的 `console.error` 直接输出)
升级为**直接观测** —— 修复前无 worker 节点 / 修复后有 worker 节点,且 `/health` 的
`last_ping_received_ts` 与用户操作时刻精确吻合。

### 仍未验证的部分(如实记录)

`extension → sidecar` 这一段已完全验证。`sidecar → 真实 Save As 对话框` 这一段
**本次未测** —— 需要用户实际打开一个 Save As 对话框观察。
若对话框是 WinUI 3(Edge 现代版),sidecar 会返回 `Unsupported` 并累加
`unsupported_dialog_count`(当前为 0);这是 focus-19/20 已知的、被用户接受的
trade-off,与本次 sandbox 修复无关。


---

## 6b. Step 4 Hook 3 — pre-commit(已获用户授权并安装)

安装在 `F:\soft\00selfmade\filemanager\.git\hooks\pre-commit`(安装前确认无同名文件,
嵌套仓库 `sigma-file-manager` 内无 extension `index.js`,故只装父仓库一处)。

**fail-OPEN 设计**:仅当 node 缺失 / 扫描器缺失时才跳过 gate 并大声警告,
避免因为运行时缺失而把用户锁死在 git 之外;Hook 1/2 仍然硬失败。

**两个分支均实测**(通过 `C:\Program Files\Git\bin\bash.exe` 执行):

```
分支 1 通过(干净树,无 staged)      → HOOK_EXIT=0
分支 2 拦截(staged 违规 fixture)   → HOOK_EXIT=1  + "pre-commit: BLOCKED"
unstage 后复跑                      → HOOK_EXIT=0
```

分支 2 走的是真实的 `git diff --cached` 路径,证明 hook 不是永远放行。
**staging 后立即 `git reset`,未执行任何 commit**;`git diff --cached` 已确认为空,
`release/extension/dist/index.js` 的 SHA256 保持 `0CEA389F…8473` 未变。


---

## 7. 本次改动的文件清单

| 文件 | 仓库 | 改动 |
|---|---|---|
| `release/extension/dist/index.js` | filemanager | -103 行(仅头部注释) |
| `sigma-file-manager/scripts/build-with-sidecar.ps1` | sigma-file-manager(嵌套仓库) | +Hook 1 |
| `release/extension-installer/manual-install.ps1` | filemanager | +Hook 2 |
| `release/extension-installer/register.ps1` | filemanager | +Hook 2b |
| `.git/hooks/pre-commit` | filemanager | +Hook 3(已授权) |
| `%APPDATA%\…\kizemo.focus-sync\dist\index.js` | 部署 | 已更新 |

**未改动**:sidecar 二进制及源码、`sandbox.ts`、`loader.ts`、`index.js` 的任何代码行。

**未执行任何 `git commit`。** 仅 stage 过一个 fixture 并已 `git reset` 还原,
`git diff --cached` 已确认为空。

**遗留物**:`.gate-fixture/` 下的两个 fixture 文件(`bad-index.js`、`dist/index.js`),
是本次用来证明 gate / hook 拦截分支真的会触发的故意违规样本,从未提交、从未部署。
本环境无可用的 `mavis-trash`,且策略禁止改用永久删除命令,因此保留未清理 —— 可手动删除整个 `.gate-fixture` 目录。

