# 根因确认:extension 被 Sigma FM sandbox 校验拒绝,从未加载

**日期**:2026-10-07 12:00
**Phase 4 状态**:✅ **Phase 1-3 完成,根因已证实**

---

## 0. 根因(一行)

**`release/extension/dist/index.js` 第 7 行的注释里有一个词 `window.`(句号紧跟),命中 Sigma FM 的 `validateExtensionCode` 正则 `/\bwindow\s*\./g`,导致 extension 加载在 `loader.ts:359-361` 被 `throw` 拒绝 —— Worker 从未被创建,`activate()` 从未执行,sidecar 从未收到任何请求。**

---

## 1. 硬证据链

### 1.1 你提供的截图(决定性)

| 截图 | 观察 | 推论 |
|---|---|---|
| **图 1** | Console 上下文下拉只有 `top` / `tauri.localhost` / `DevTools Performance Metrics`,**无 worker scope** | **Worker 从未被创建** |
| **图 2** | Sources → 页面 → `tauri.localhost` → 只有 `assets` / `(索引)` / `startup-recovery.js`,**无 extension-worker.js** | **同上,交叉确认** |
| **图 4** | 设置 → Console → 「仅限选定的上下文」**未勾选** | 排除「日志被过滤」的可能 |

**两张图互相独立地证明了同一件事:`new Worker()` 没有执行过。**

### 1.2 代码路径定位

`loader.ts:316-405` — `loadApiExtension()`:

```typescript
346: runtime.workerHost = createWorkerHost(api);        // 只创建 host 对象,不 new Worker
349: const fileContent = await invokeAsExtension('read_extension_file', ...);
354: const decoder = new TextDecoder();
355: const code = decoder.decode(new Uint8Array(fileContent));
356: const validationResult = validateExtensionCode(code);   // ★ 校验
357: const moduleEntry = await createBlobModuleEntry(extensionId, mainFile);
358:
359: if (!validationResult.valid) {
360:   throw new Error(`Extension code validation failed: ${...}`);   // ★★ 在这里 throw
361: }
...
365: await runtime.workerHost.initialize(moduleEntry.entryUrl);   // ← new Worker 在这,永远到不了
```

**`throw` 在 `:360`,而 `new Worker()` 在 `:365` 之后 —— 顺序上决定了 Worker 根本不会被创建。**

**这完美解释了你的所有观察:**

| 观察 | 解释 |
|---|---|
| Sources 无 worker | `:360` throw 了,`:365` 没执行 |
| Console 无 `[focus-sync]` 日志 | `activate()` 在 worker 里,worker 不存在 |
| `last_ping_received_ts: 0` | extension 死了,从未发 HTTP |
| 「之前某些阶段出现过」 | 那时 index.js 还没有这个 `window.` |

### 1.3 实测:违规 pattern 的精确位置

我对**实际部署到 `%APPDATA%` 的文件**做了扫描:

```
=== sandbox pattern scan on DEPLOYED index.js ===
  HIT  window\.               x1

=== window. 出现位置 ===
  line 7: *   - H2 works without foreground; UIA SetValue does not require foreground window.
```

**`line 7` 是一行注释:`...does not require foreground window.`**

**注意结尾:`window.` 的句号紧跟在 `window` 后面。** 正则 `/\bwindow\s*\./g` 匹配 `window` + 零个空白 + `.` → **命中**。

### 1.4 违规规则原文(`sandbox.ts:207-241`)

```typescript
const dangerousPatterns = [
  { pattern: /\beval\s*\(/g,          message: 'eval() is not allowed' },
  { pattern: /\bnew\s+Function\s*\(/g, message: 'new Function() is not allowed' },
  { pattern: /\.constructor\s*\(\s*['"`]/g, ... },
  { pattern: /\bdocument\s*\./g,      ... },
  { pattern: /\bwindow\s*\./g,        message: 'Direct window access is not allowed' },  // ← 225-227
  { pattern: /\blocalStorage\s*\./g,  ... },
  { pattern: /\bsessionStorage\s*\./g, ... },
  { pattern: /\bindexedDB\s*\./g,     ... },
  { pattern: /\bfetch\s*\(/g,          ... },
  // ...
];
```

**`pattern.test(code)` 作用在原始 JS 字符串上 —— 注释和字符串字面量里的内容同样会被匹配。**

---

## 2. 这不是新问题 —— 它发生过两次

`docs/handoff-2026-09-29-engine-caret-fix.md` 记录了**完全同类的失败**:

> **Root cause #2:我的注释本身触发 sandbox**
>
> After engines fix, sidecar still didn't spawn. Investigating revealed my own source code comments were triggering sandbox validation
>
> The warning I wrote (to document sandbox constraints) listed all 18 forbidden patterns → ALL matched in regex check → `validateExtensionCode` returned 17 errors → extension silently failed
>
> **Fix**: Removed the entire "IMPORTANT — sandbox constraints" sections from `release/extension/dist/index.js`

**当时的修法是:把注释里列出的禁用词清单删掉。**

**这次的 `window.` 是同一类问题换了个词 —— 上次删了「清单列举」,这次是自然语言句子里的 `window.`。**

### 2.1 为什么上次修了还会再犯

| # | 原因 |
|---|---|
| 1 | 修的是「具体那个词」,不是「机制」 |
| 2 | 没人写检查脚本防止新注释再次命中 |
| 3 | 文件经过 20+ 轮注释扩充,每次都有人自然地写「foreground window」这种措辞 |
| 4 | v0.5.5 的 `index.js` 前 10 行是版本 changelog 注释,专门描述 SendInput / foreground / H2 —— **高危措辞密集区** |

---

## 3. 这也解释了为什么 v0.5.5 的 12 个 Fix 全部无效

**v0.5.5 改了 sidecar 的 12 处(41477 排除、first_push_received gate、default_value、日志、版本号),但 extension 从来没有加载过。**

```
sidecar 改动 ✅ 12 处全部 ship
        ↓
但 extension 死在 loader.ts:360  ← 从未进入 sidecar 的代码路径
        ↓
sidecar 的 12 处改动一次都没被触发
```

**gate、sentinel、41477 排除 —— 全部是「extension 活着」的前提下才有意义的东西。**

---

## 4. 修复(Phase 4)

### 4.1 最小修复:改一行注释

`release/extension/dist/index.js:7`:

```javascript
// BEFORE  ← 命中 /\bwindow\s*\./g
 *   - H2 works without foreground; UIA SetValue does not require foreground window.

// AFTER   ← 避开 window. 这个模式
 *   - H2 works without foreground; UIA SetValue does not require foreground focus.
```

**同时建议检查整个文件头部注释区(1-40 行),把所有可能命中 sandbox regex 的措辞换掉:**

| 禁用 | 安全替代 |
|---|---|
| `window.` | `focus` / `foreground focus` |
| `document.` | `DOM` / `page` |
| `localStorage.` | `local store` |
| `fetch(` | `sidecar` / `backend` / `API` |
| `navigator` | `folder` / `current folder` |
| `WebSocket` | `sidecar` |
| `XMLHttpRequest` | `sidecar` / `backend` |
| `self` | `scope` / `context` |
| `addEventListener(` | `register handler` |
| `postMessage(` | `send` |

### 4.2 必须同时做的防护(否则会第三次复发)

**加一个 CI 级别的扫描脚本**,在 build 时拦截:

```powershell
# scripts/check-extension-sandbox.ps1
$index = "release/extension/dist/index.js"
$code = Get-Content $index -Raw
$forbidden = @(
  '\beval\s*\(', '\bnew\s+Function\s*\(', '\.constructor\s*\(\s*[''"`]',
  '\bdocument\s*\.', '\bwindow\s*\.', '\blocalStorage\s*\.',
  '\bsessionStorage\s*\.', '\bindexedDB\s*\.', '\bfetch\s*\(',
  '\bXMLHttpRequest\b', '\bWebSocket\b', '\bself\b', '\bglobalThis\b',
  '\bpostMessage\s*\(', '\baddEventListener\s*\(', '\bnavigator\b'
)
$hits = @()
foreach ($p in $forbidden) {
  $c = ([regex]::Matches($code, $p, 'IgnoreCase')).Count
  if ($c -gt 0) { $hits += "  $p x$c" }
}
if ($hits.Count -gt 0) {
  Write-Host "SANDBOX PATTERN VIOLATION in $index" -ForegroundColor Red
  $hits | ForEach-Object { Write-Host $_ -ForegroundColor Red }
  exit 1
}
Write-Host "OK: no sandbox-forbidden patterns"
```

**注意 `\bself\b` 会误报**(英文单词 "self" 常见)。我实测当前部署文件 `self` 命中 1 次但**定位不到**(在某个被我的正则边界处理漏掉的位置),需要用脚本精确复核。

### 4.3 修复后必须验证(不能只看日志)

```powershell
# 1. build + 扫描
pwsh scripts/check-extension-sandbox.ps1    # 必须 OK

# 2. 部署
Copy-Item release\extension\dist\index.js `
  "$env:APPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\dist\index.js" -Force

# 3. ★ 关键:Sigma FM 必须完全退出再重启
#    (MEMORY.md: "close Sigma FM before editing")
#    否则 runtime 用的是内存里的旧代码

# 4. 验证 Worker 出现了(这才是真正的验收标准)
#    DevTools → Sources → 页面 → tauri.localhost → 应该能看到 worker 节点
#    或者 Console 上下文下拉里出现 worker scope

# 5. 验证 PUSH 到了
curl http://127.0.0.1:37421/health
#    last_ping_received_ts 应该 > 0(不再是 0)
```

---

## 5. 关于你的三个截图的答复

| 问题 | 答复 |
|---|---|
| Console 下拉无 worker scope | ✅ **正常且正确** —— Worker 从未被创建,不是找不到 |
| Sources 无 extension-worker.js | ✅ **同上,这是决定性证据** |
| 无「selected context only」选项 | ✅ 设置里叫「**仅限选定的上下文**」(图 4 已看到,未勾选)。**不是问题所在** |
| 粘贴失败 | 已绕过 —— 现在不需要粘贴,直接看 Sources 面板有没有 worker 节点即可 |

---

## 6. 沉淀规则

### 规则 72 — Sigma FM extension 源码必须做 sandbox 正则扫描

```markdown
### 规则 72 — extension 源码(含注释)必须过 sandbox 正则

**触发**:2026-10-07 根因确认,同一问题第二次发生

**事实**:
  sandbox.ts:207-241 的 dangerousPatterns 用 pattern.test(code) 匹配
  【原始 JS 字符串】,注释和字符串字面量同样被匹配。
  第一次:handoff-2026-09-29「注释里列了 18 个禁用词」→ 17 errors
  第二次:2026-10-07 index.js:7「...foreground window.」→ 命中 /\bwindow\s*\./g

**规则**:
1. 任何改动 extension dist/index.js 之后,必须跑 sandbox 扫描脚本
2. 注释和 changelog 是高危区 —— 恰恰是最爱写「foreground window」这类措辞的地方
3. 禁用词清单(全部 case-insensitive):
   eval( / new Function( / .constructor(' / document. / window. /
   localStorage. / sessionStorage. / indexedDB. / fetch( /
   XMLHttpRequest / WebSocket / self / globalThis /
   postMessage( / addEventListener( / removeEventListener( /
   dispatchEvent( / importScripts / navigator
4. 安全替代词见 §4.1 表格
5. 改完必须【完全退出 Sigma FM 再启动】,否则内存里的旧代码还在
```

### 规则 73 — 「Worker 节点是否存在」是 extension 是否加载的硬指标

```markdown
### 规则 73 — extension 加载成功的验收标准是 Worker 出现,不是日志

**依据**:loader.ts:365 workerHost.initialize() 内部才 new Worker
      若 :360 throw,Worker 不存在

**规则**:
1. extension「是否加载成功」的判定:DevTools → Sources → 页面 → 目标域名下
   是否出现 worker 节点。没有 = 加载失败。
2. 不要用「主窗口 Console 里有没有 extension 的 console.log」来判断 ——
   extension 在 worker scope,主窗口永远看不到(2026-10-07 踩过)
3. sidecar 侧收到请求 = 更下游的间接证据,不如 Worker 节点直接
```

---

## 7. 元数据

| 项 | 值 |
|---|---|
| **根因** | `dist/index.js:7` 注释含 `window.` → `sandbox.ts:225` 正则命中 → `loader.ts:360` throw |
| **历史** | 同一类问题第二次(`handoff-2026-09-29-engine-caret-fix.md` 记录第一次) |
| **对 v0.5.5 的影响** | sidecar 的 12 个 Fix 全部未触发,一次都没执行过 |
| **修复量** | 1 行注释 + 1 个防护脚本 |
| **验收标准** | Sources 面板出现 worker 节点 + `last_ping_received_ts > 0` |
| **新规则** | Rule 72(sandbox 扫描)/ Rule 73(Worker 节点是硬指标) |

---

## 8. 一句话 prompt(新会话接力)

```
读 docs/superpowers/plans/2026-10-07-focus-21-root-cause-confirmed-sandbox.md(必读)。

根因确认(非推测):
  release/extension/dist/index.js:7 注释含 "foreground window."
  → 命中 sandbox.ts:225 的 /\bwindow\s*\./g
  → loader.ts:359-361 throw "Extension code validation failed"
  → loader.ts:365 workerHost.initialize() 未执行 → new Worker 从未创建
  → activate() 从未运行 → sidecar 108 分钟零请求

用户的截图是决定性证据:
  Sources 面板无 worker 节点 + Console 上下文下拉无 worker scope
  → Worker 从未创建(不是找不到,是从未存在)

这是同一问题的第二次发生:
  handoff-2026-09-29-engine-caret-fix.md 记录第一次
  (注释里列了 18 个禁用词 → 17 errors → extension silently failed)

修复(最小):
  1. index.js:7 「foreground window.」→「foreground focus.」
  2. 全文件扫一遍注释区,换掉所有 sandbox 禁用词
  3. 加 scripts/check-extension-sandbox.ps1 防护(build 时拦截)
  4. 部署后【完全退出 Sigma FM 再重启】(MEMORY.md 明确要求)
  5. 验收:Sources 面板出现 worker 节点 + curl /health 的 last_ping_received_ts > 0

沉淀 Rule 72(sandbox 正则扫描)+ Rule 73(Worker 节点是加载硬指标)。
```

---

## 9. 对 100+ 轮的最终归因

| 阶段 | 真实状态 |
|---|---|
| focus-16 ~ focus-20 | extension 被 sandbox 拒绝(sidecar 一直收到 0 请求) |
| v0.3.0 ~ v0.3.8 | 同类问题修了又犯 |
| v0.5.1 ~ v0.5.5 | 同一问题换个词复发,12 个 sidecar Fix 全部未触发 |
| 100+ 轮 review | 全部在读 sidecar 源码,没有人检查过 extension 是否加载 |

**这解释了一个之前无法理解的现象:为什么改了几十个版本的 sidecar,`last_ping_received_ts` 一直是 0。**

**因为 sidecar 的代码从来没有被 extension 的请求触发过。**

---

## 10. 需要你确认的

**修复前我需要知道:是否授权我实施?**

修复内容:
1. 改 `release/extension/dist/index.js` 的注释(1 行起步,可能需要扫全文件)
2. 新增 `scripts/check-extension-sandbox.ps1` 防护脚本
3. 部署到 `%APPDATA%`
4. **你需要完全退出并重启 Sigma FM**(这一步我无法代做)

**在你确认之前,我不动任何文件。**
