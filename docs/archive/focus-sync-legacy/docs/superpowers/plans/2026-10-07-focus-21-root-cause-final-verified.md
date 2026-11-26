# v0.5.5 根因最终确认报告(逐字节验证版)

**日期**:2026-10-07 12:10
**方法**:逐字节复刻 Sigma FM 的 19 条 sandbox 正则,实测部署文件 + 全链路 gate 扫描
**结论**:**根因确认,且是唯一阻断点。但修复后仍需验证 3 个次级风险。**

---

## 0. 根因(已逐字节证实,非推测)

```
sigma-listary-spike 改了 12 处
        ↓
但 release/extension/dist/index.js 第 7 行注释含 "foreground window."
        ↓
sandbox.ts:225 的 /\bwindow\s*\./g 命中
        ↓
loader.ts:359-361  throw "Extension code validation failed"
        ↓
loader.ts:365  new Worker() 从未执行
        ↓
activate() 从未运行 → sidecar 108 分钟 0 请求
```

---

## 1. 逐字节验证(不是靠 pattern 猜)

### 1.1 我读了 Sigma FM 的真实源码

`sandbox.ts:207-284` — **19 条正则,含 4 条负向前瞻**(`(?<![\w$.])`):

```typescript
207: const dangerousPatterns = [
209:   /\beval\s*\(/g
213:   /\bnew\s+Function\s*\(/g
217:   /\.constructor\s*\(\s*['"`]/g
221:   /\bdocument\s*\./g
225:   /\bwindow\s*\./g                                        ← 命中点
229:   /\blocalStorage\s*\./g
233:   /\bsessionStorage\s*\./g
237:   /\bindexedDB\s*\./g
241:   /\bfetch\s*\(/g
245:   /\bXMLHttpRequest\b/g
249:   /\bWebSocket\b/g
253:   /(?<![\w$.])self\b/g                                   ← 有负向前瞻
257:   /(?<![\w$.])globalThis\b/g
261:   /(?<![\w$.])postMessage\s*\(/g
265:   /(?<![\w$.])addEventListener\s*\(/g
269:   /(?<![\w$.])removeEventListener\s*\(/g
273:   /(?<![\w$.])dispatchEvent\s*\(/g
277:   /\bimportScripts\b/g
281:   /(?<![\w$.])navigator\b/g
284: ];
286: for (const { pattern, message } of dangerousPatterns) {
287:   if (pattern.test(code)) errors.push(message);      ← 对【原始 JS 字符串】匹配
290: }
```

**我第一份报告里的 pattern 清单是凭 handoff 文档重构的,漏了 4 条负向前瞻。这次直接读源码,一字不差。**

### 1.2 我写了逐字节扫描器实测

`scripts/scan-sandbox.cjs` — 完整复刻 19 条正则(JS 正则,与 TS 同语义),逐条报告命中位置并标注是否在注释里。

**对部署文件实测:**

```
FILE: C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\dist\index.js
SIZE: 26550 bytes / 582 lines

[VIOLATION] Direct window access is not allowed  (1 hit)
    line 7: "window."  [in comment]
    *   - H2 works without foreground; UIA SetValue does not require foreground window.

RESULT: INVALID (1 violations)
```

### 1.3 repo 与 deployed 完全一致

```
repo     : A78E96D3268FDA3A9CBF028D5BF02B26038261FB018D62C36E53BE7519541389  (26851 B)
deployed : A78E96D3268FDA3A9CBF028D5BF02B26038261FB018D62C36E53BE7519541389  (26851 B)
IDENTICAL: True
```

**改 repo 就等于改 deployed,不需要两步同步。**

### 1.4 你的截图交叉印证

| 截图 | 观察 | 推论 |
|---|---|---|
| 图 1 | Console 上下文只有 `top` / `tauri.localhost` / `DevTools Performance Metrics` | 无 worker scope |
| 图 2 | Sources → 页面 → `tauri.localhost` → 只有 `assets` / `(索引)` / `startup-recovery.js` | **无 worker 节点** |
| 图 4 | 「仅限选定的上下文」未勾选 | 不是日志过滤问题 |

**Sigma FM 进程实测在运行**:PID 35832,`D:\Program Files\Sigma File Manager\sigma-file-manager.exe`,启动于 2026/10/7 9:36:34。

**所以这不是「环境没跑」,而是「跑了但 extension 被拒绝」。截图是在运行中的实例上抓的,证据有效。**

---

## 2. 全链路 gate 扫描:只有 1 个失败

`scripts/check-extension-gates.cjs` — 复刻 `loader.ts:316-365` + `validation.ts` 的所有前置检查。

```
PASS  manifest.id                                    kizemo.focus-sync
PASS  manifest.name                                   Focus Sync
PASS  manifest.version                                0.5.5
PASS  manifest.extensionType=api                      api
PASS  manifest.main                                   dist/index.js
PASS  manifest.activationEvents includes onStartup    ["onStartup"]
PASS  main file exists on disk                        ✅
PASS  main file non-empty                             26851 bytes
PASS  engines.sigmaFileManager no caret               >=2.2.0
PASS  engines.extensionApi no caret                   >=1.0.0
PASS  registered enabled=true                         true
PASS  installPendingDependencies=false                false
PASS  isLocal=true                                    true
PASS  localSourcePath matches                         ✅
PASS  http permission is object form                  {"name":"http","hosts":["http://127.0.0.1:37421"]}
PASS  http host allowlist has 127.0.0.1:37421         ✅
PASS  no bare "http" string permission                ✅
PASS  extension SIDECAR port                          127.0.0.1:37421
PASS  port matches sidecar 37421                      37421
PASS  locales/en.json exists                          ✅
PASS  locales/zh-CN.json exists                       ✅
PASS  index.js.map exists (optional)                  ✅
====================================================
ALL GATES PASS (sandbox 校验之外)
```

**结论:manifest / 注册 / 权限 / 路径 / 依赖 全部健康。唯一失败是 sandbox 代码校验。**

---

## 3. 修复方案(精确到行)

### 3.1 主修复:1 行

`release/extension/dist/index.js:7`:

```javascript
// BEFORE(命中 sandbox.ts:225 /\bwindow\s*\./g)
 7:  *   - H2 works without foreground; UIA SetValue does not require foreground window.

// AFTER
 7:  *   - H2 works without foreground; UIA SetValue does not require foreground focus.
```

`window.` → `focus.` 。同义、无歧义、不命中任何正则。

### 3.2 但整段 changelog 注释都是雷区

我逐行读了 1-40 行,发现**这个文件头部是高危措辞密集区**:

| 行 | 内容 | 风险 |
|---|---|---|
| 2 | `Focus Sync v0.5.5 — Sigma FM extension entry point.` | 干净 |
| 5 | `v0.5.4 GetForegroundWindow check was warn + continue` | 干净 |
| 7 | `...require foreground window.` | ❌ **命中** |
| 8 | `50ms flicker between SetValue target_path and SetValue original_filename.` | 干净 |
| 11 | `Ctrl+A 不 work in WinUI 3 wrapped Save As` | 干净 |
| 13 | `SendInput uses Ctrl+L + Home + Shift+End` | 干净 |

**只有第 7 行命中。** 但我不能只看前 40 行 —— 扫描器已经验证了全文件 582 行只有这 1 处。

**建议同时把整个 v0.5.5 changelog 块精简掉** —— 它描述的是 sidecar 内部实现,对 extension 用户零价值,却是纯粹的雷区。

### 3.3 防护脚本(必须加,否则第三次复发)

**`scripts/scan-sandbox.cjs`(已创建,可用)**
- 逐字节复刻 19 条正则
- 报告命中位置 + 是否在注释中
- `exit 1` 让 CI 拦截

**集成到构建流程**(`scripts/build-extension.ps1` 或 Tauri build 钩子):
```powershell
node scripts/scan-sandbox.cjs release/extension/dist/index.js
if ($LASTEXITCODE -ne 0) { throw "sandbox violation - refusing to package" }
```

---

## 4. 修复后的 3 个次级风险(必须一并验证)

**这些不会阻止 extension 加载,但会影响「加载后能否正常工作」。**

### 4.1 风险 A:`activate()` 内 L435 无 catch,可能静默中断

`index.js:431-437`:
```javascript
431: console.log('[focus-sync] v0.5.5 activate START ...');   // 无 await ✅
432: try { await sigma.i18n.mergeFromPath('locales'); } catch (e) {}   // 有 catch ✅
435: enabled = await loadBool('enabled', true);      // ★ 无 catch
436: autoConfirm = await loadBool('autoConfirm', false);
437: const tipShown = await loadBool('edgeLegacyFilePickerTipShown', false);
```

**若 `sigma.storage` 不可用,L435 抛错 → `activate()` 中断 → L502 的 `pingSidecar()` 和 L567 的 `schedulePush()` 都不执行 → 依然是零 ping。**

**验证方法**(修复 sandbox 后立刻做):
```
Sources 面板 → 页面 → tauri.localhost → 出现 worker 节点 ✅
Console → 切到 worker scope → 看有没有 [focus-sync] 日志
curl /health → last_ping_received_ts 是否 > 0
```

**如果 worker 出现了但 `last_ping_received_ts` 还是 0** → 风险 A 成立,需要在 L435 加 try/catch。

### 4.2 风险 B:`http://` 明文在 CSP 下可能被拦

manifest 声明的是 `http://127.0.0.1:37421`(明文 HTTP)。

**实测确认环境因素**(不是问题,但要知道):
```
ProxyEnable   : 0                              ← 系统代理关闭
ProxyServer   : 127.0.0.1:7897
NO_PROXY      : localhost,127.0.0.1,::1        ← 已覆盖
```
**代理不是问题。** 但 `sigma.http.request` 走的是 Rust 侧 reqwest,是否读 `NO_PROXY` 取决于实现 —— 我没有验证。**如果修复后 ping 失败,这里是第一嫌疑。**

### 4.3 风险 C:Sigma FM 未完全退出导致代码不刷新

`MEMORY.md` 明确记录:
> `sigma_fm_extension_registration.md` — **close Sigma FM before editing**

**你必须完全退出 Sigma FM(任务栏右键 → 退出,不是关窗口)再重启**,否则内存里还是旧代码,会看到「改了但没变」的假象。

---

## 5. 完整实施与验证序列

```
Step 1  改 index.js:7  window. → focus.
Step 2  node scripts/scan-sandbox.cjs release/extension/dist/index.js
        → 必须输出 "RESULT: VALID (0 violations)"
Step 3  ★ 完全退出 Sigma FM(任务栏 → 退出)
Step 4  启动 Sigma FM
Step 5  DevTools → Sources → 页面 → tauri.localhost
        → 应该出现 worker 节点(这是 extension 加载成功的硬指标)
Step 6  Console → 上下文下拉 → 切到 worker scope
        → 应该看到 "[focus-sync] v0.5.5 activate START"
Step 7  curl http://127.0.0.1:37421/health
        → last_ping_received_ts 必须 > 0(不再是 0)
Step 8  在 Sigma FM 导航到任意目录
Step 9  观察 extension console 是否有 "[focus-sync] PUSH raw=..."
Step 10 打开 Edge Save As,观察地址栏/目录
```

**Step 5 是分水岭:没有 worker 节点 = sandbox 还没修好;有 worker 节点 = 进入 Step 6-7 验证运行时。**

---

## 6. 顺带清理:那个 changelog 块建议删

`index.js:1-~90` 是 v0.5.1~v0.5.5 的详细 changelog 注释,描述 SendInput / foreground / H2 / SetValue 等 sidecar 内部实现。

**问题**:
- 对 extension 用户零价值(他们只关心「能不能同步」)
- 是 sandbox 雷区(已经因此挂过一次)
- 每次 bump 版本都要维护,是重复故障源

**建议**:压缩成 3 行
```javascript
/**
 * Focus Sync v0.5.5 — Sigma FM extension entry point.
 *
 * Sends the active folder to a local sidecar process over HTTP.
 * See docs/superpowers/plans/ for implementation notes.
 */
```

**但这不是修复的必要条件** —— 扫描器已确认只有 1 处命中。可以单独做。

---

## 7. 沉淀规则(合并此前所有教训)

### 规则 72(升级版)— extension 源码必须过 sandbox 正则扫描

```markdown
### 规则 72 — extension dist/index.js 必须过 19 条 sandbox 正则扫描

**触发**:同一问题第二次发生
  第一次 handoff-2026-09-29:注释里列了 18 个禁用词 → 17 errors
  第二次 2026-10-07:index.js:7「foreground window.」→ 1 error

**权威来源**:`sigma-file-manager/src/modules/extensions/runtime/sandbox.ts:207-284`
不要凭记忆重构 pattern 列表 —— 4 条带负向前瞻 (?<![\w$.])。

**19 条正则**:
  eval( / new Function( / .constructor(' / document. / window. /
  localStorage. / sessionStorage. / indexedDB. / fetch( /
  XMLHttpRequest / WebSocket / (?<![\w$.])self / (?<![\w$.])globalThis /
  (?<![\w$.])postMessage( / (?<![\w$.])addEventListener( /
  (?<![\w$.])removeEventListener( / (?<![\w$.])dispatchEvent( /
  importScripts / (?<![\w$.])navigator

**规则**:
1. 改 dist/index.js 后必须跑 `node scripts/scan-sandbox.cjs <file>`
2. 匹配作用于【原始 JS 字符串】—— 注释和字符串字面量同样被匹配
3. changelog 注释是高危区,别在 extension 源码里描述 sidecar 内部实现
4. 禁用词替代表:window.→focus / document.→DOM / fetch(→sidecar /
   navigator→folder / WebSocket→sidecar / self→scope
```

### 规则 74 — 「Worker 节点存在」是 extension 加载的硬指标

```markdown
### 规则 74 — extension 是否加载成功,看 Worker 节点而不是日志

**依据**:loader.ts:356 validateExtensionCode → :360 throw → :365 new Worker
      顺序上,校验失败则 Worker 不存在

**规则**:
1. 验收标准:DevTools → Sources → 页面 → 目标域名下有 worker 节点
2. 不用「主窗口 Console 有没有 extension 的 console.log」判断 ——
   extension 在 worker scope,主窗口永远看不到(2026-10-07 踩过)
3. 不用「sidecar 有没有收到请求」判断 —— 那是更下游的间接证据
```

### 规则 75 — 声称根因前必须逐字节复刻对方的校验逻辑

```markdown
### 规则 75 — 诊断别人的系统时,读源码而非读文档

**触发**:2026-10-07

**错误**:我第一份报告根据 handoff 文档重构了 sandbox pattern 清单,
     漏了 4 条负向前瞻 (?<![\w$.]),导致扫描结果可能失真。

**规则**:
1. 判定「某文件会不会被 X 拒绝」时,
   必须直接读 X 的实现源码,逐条复刻其正则
2. 写一个扫描脚本实测,而不是用 PowerShell -match 近似
   (PS 正则与 JS 正则在 lookbehind 支持上不同)
3. 扫描器输出必须含:命中位置 + 原文 + 是否在注释中
4. 第三方文档(handoff/memory)只能作为线索,不能作为依据
```

---

## 8. 回答你之前的问题:「每次诊断都有遗漏」

**你说得对。我梳理了一下历次遗漏的模式:**

| 轮次 | 我的遗漏 | 为什么会漏 |
|---|---|---|
| R22-R26 | 从未验证 extension 是否加载 | 只读 sidecar 源码 |
| 自我审计 | 编造「45% 部署 bug」 | 用印象代替统计 |
| Phase 1 报告 | 用「主窗口无日志」推断「未执行」 | 忽略 Worker 隔离 |
| 根因报告 | 凭 handoff 重构 pattern 列表 | 读文档而非源码 |
| **本轮** | 补齐了前 4 项 | 逐字节复刻 + 实测 |

**共同的根因:我在用「推理」代替「测量」。**

**这一轮之所以终于对,是因为我做了三件之前没做的事:**
1. 读了 Sigma FM 的**真实源码**(`sandbox.ts:207-284`),而不是 handoff 里的二手描述
2. 写了**逐字节扫描器**,用 JS 正则(与 TS 同语义)实测部署文件
3. **实测了 Sigma FM 进程**存在(PID 35832),排除了「环境没跑」这个我上一轮的错误假设

---

## 9. 元数据

| 项 | 值 |
|---|---|
| **根因** | `index.js:7` 注释含 `window.` → `sandbox.ts:225` 命中 → `loader.ts:360` throw |
| **验证方式** | 逐字节复刻 19 条正则 + 实测部署文件 + 全链路 22 项 gate 扫描 |
| **唯一失败项** | sandbox 代码校验(其余 21 项全 PASS) |
| **repo == deployed** | SHA256 完全一致 |
| **Sigma FM 进程** | PID 35832,运行中,启动 9:36:34 |
| **历史复发** | 第 2 次(首次 handoff-2026-09-29) |
| **新增工具** | `scripts/scan-sandbox.cjs`、`scripts/check-extension-gates.cjs` |
| **次级风险** | A: L435 无 catch / B: http 明文 / C: 未完全退出 |
| **新规则** | Rule 72(升级)/ 74(Worker 硬指标)/ 75(读源码不读文档) |

---

## 10. 一句话 prompt(新会话接力)

```
读 docs/superpowers/plans/2026-10-07-focus-21-root-cause-final-verified.md(必读)。

根因逐字节证实(非推测):
  release/extension/dist/index.js:7 注释 "foreground window."
  → sandbox.ts:225 /\bwindow\s*\./g 命中
  → loader.ts:359-361 throw
  → :365 new Worker() 从未执行
  → activate() 从未运行 → sidecar 108 分钟 0 请求

验证工具(已创建):
  node scripts/scan-sandbox.cjs release/extension/dist/index.js
    → 逐字节复刻 sandbox.ts:207-284 的 19 条正则
    → 当前输出:RESULT: INVALID (1 violations), line 7 [in comment]
  node scripts/check-extension-gates.cjs
    → 22 项全链路 gate,除 sandbox 外全部 PASS
  repo 与 deployed SHA256 完全一致(A78E96D3...)

Sigma FM 进程实测在运行(PID 35832, 9:36:34 启动),
所以用户截图是在活实例上抓的,证据有效。

修复(1 行):
  index.js:7  "foreground window." → "foreground focus."

实施序列:
  1. 改 line 7
  2. scan-sandbox.cjs 必须输出 VALID
  3. ★ 完全退出 Sigma FM(任务栏右键退出,不是关窗口)
  4. 重启 → Sources 面板看 worker 节点是否出现(硬指标)
  5. Console 切 worker scope → 看 [focus-sync] activate START
  6. curl /health → last_ping_received_ts > 0

次级风险(修复后验证):
  A. index.js:435 loadBool 无 catch,若 sigma.storage 不可用会静默中断
  B. http:// 明文(代理已排除:ProxyEnable=0 + NO_PROXY 含 127.0.0.1)
  C. 未完全退出 Sigma FM 会看到「改了没变」假象

沉淀 Rule 72(升级)/ 74(Worker 硬指标)/ 75(读源码不读文档)。
```

---

## 11. 当前状态与下一步

**未实施任何修改。** 已创建两个只读诊断脚本(不改产品代码):
- `scripts/scan-sandbox.cjs`
- `scripts/check-extension-gates.cjs`

**等你授权后才改 `index.js:7`。**

授权后我会:
1. 改 line 7
2. 跑扫描确认 VALID
3. 部署到 %APPDATA%
4. **你完全退出并重启 Sigma FM**
5. 一起验收 Worker 节点 + `last_ping_received_ts > 0`
