# 重要修正:我上一份报告的关键证据是错的

**日期**:2026-10-07 10:50
**修正对象**:`2026-10-07-focus-21-systematic-debugging-phase1-3.md` 的「决定性证据」段落

---

## 0. 修正结论

**我上一份报告说「`activate START` 日志缺失 = `activate()` 未被调用」,这个推论是错的 —— 因为 extension 跑在 Web Worker 里,它的 `console.log` 根本不会出现在你打开的那个 Console。**

**我犯的错误:用「主窗口 Console 里没有日志」推断「Worker 里没有执行」。这是两个不同的执行上下文。**

**Iron Law 状态回退:Phase 3 的假设重新变成「未验证」。**

---

## 1. 证据:extension 跑在独立 Worker

`sigma-file-manager/src/modules/extensions/runtime/worker-runtime.ts:38-43`:

```typescript
function createWorker(): Worker {
  return new Worker(
    new URL('./extension-worker.ts', import.meta.url),
    { type: 'module' },
  );
}
```

`extension-worker.ts:1301-1303`:
```typescript
workerScope.addEventListener('message', (event: MessageEvent<WorkerRuntimeMessage>) => {
  void handleWorkerMessage(event.data as HostToWorkerMessage);
});
```

**架构是:**
```
主窗口 (Vue)
  └─ worker-runtime.ts
       └─ new Worker(extension-worker.ts)     ← 独立 JS 线程
            └─ import index.js → activate() → console.log(...)
                 ↑ 这里的 console 属于 Worker,不是主窗口
```

**Worker 的 console 输出在 DevTools 里有两种查看方式:**
1. Sources 面板 → Threads/Workers 节点 → 选中该 worker
2. Console 面板左上角的**执行上下文下拉框**,切到 worker 的 scope

**如果 Console 顶部有勾选框「Selected context only」(仅显示选定上下文),必须取消勾选,否则 worker 的日志被过滤掉。**

---

## 2. 被推翻的推论

| # | 我上一份报告写的 | 状态 |
|---|---|---|
| 1 | 「`activate()` 第一行是 console.log,日志缺失 = 函数未被调用」 | ❌ **前提错误** —— 日志在 Worker scope,主窗口看不到 |
| 2 | 「决定性证据:H0 已证实」 | ❌ **撤回** |
| 3 | 「已排除 Fix B gate」 | ⚠️ **重新开放** —— 如果 activate 其实跑了,gate 仍可能是真凶 |
| 4 | 「v0.5.5 的 JS 只改版本号 → 回归不在 extension 代码」 | ✅ **仍成立**(文件比对事实) |

---

## 3. 仍然成立的事实(不依赖 console)

| 事实 | 证据 | 是否受本次修正影响 |
|---|---|---|
| sidecar 活着、端口 37421 Listen、`uia_healthy: true` | 实测 | ❌ 不受影响 |
| **`last_ping_received_ts: 0`(108 分钟)** | curl /health | ❌ **不受影响,这是最硬的证据** |
| `first_push_received: false` | curl /health | ❌ 不受影响 |
| extension 已安装/启用/`isLocal: true` | user-extensions.json | ❌ 不受影响 |
| extension 权限含 `http://127.0.0.1:37421` | 注册 manifest | ❌ 不受影响 |
| v0.5.5 的 index.js 只改了版本号字符串 | 文件比对 | ❌ 不受影响 |

**`last_ping_received_ts: 0` 依然是最硬的证据** —— 它证明 **108 分钟内没有任何 HTTP 请求到达 sidecar**,这一点与 console 在哪无关。

**但它无法区分两种情况:**

| 情况 | 说明 |
|---|---|
| (a) | `activate()` 真的没被调用 |
| (b) | `activate()` 被调用了,但 `getCurrentPath()` 抛异常(你在用户截图里看到的 TDZ 错误),导致 `schedulePush` 那行永远走不到 |

**注意 `index.js:431-437` 的顺序:**
```javascript
431: console.log('[focus-sync] v0.5.5 activate START ...');   // ← 无 await
432: try { await sigma.i18n.mergeFromPath('locales'); } catch(e) {}   // ← 有 catch
435: enabled = await loadBool('enabled', true);     // ★ 无 catch,抛错则整个 activate 中断
436: autoConfirm = await loadBool('autoConfirm', false);
437: const tipShown = await loadBool('edgeLegacyFilePickerTipShown', false);
...
502: setTimeout(() => { pingSidecar() }, 5000);     // ← 第一次 HTTP 在这
```

**如果 L435 的 `loadBool` 抛异常(比如 `sigma.storage` 不可用),`activate()` 会在这里中断,`pingSidecar()` 和 `schedulePush()` 都不会执行 —— 表现就是 `last_ping_received_ts: 0`。**

**这条路径完全不需要我之前假设的「init() 中断」,它更简单也更可能。**

---

## 4. 修正后的假设集

| # | 假设 | 可能性 |
|---|---|---|
| **H0-a** | `activate()` 未被调用(在 L431 之前就断了) | 中 —— 需要 worker console 确认 |
| **H0-b** | `activate()` 执行了,但在 L435 `loadBool` 抛异常中断 | **高** —— L435 是第一个无 catch 的 await |
| **H0-c** | `activate()` 完整执行,但 `pingSidecar` / `schedulePush` 的 HTTP 请求被沙箱拦截 | 中 |

**H0-b 是新的头号假设,因为:**
- 它完美解释「零 ping」而不需要假设 `init()` 失败
- L432 有 `catch`,L435/436/437 **没有**
- 用户截图里的 TDZ 错误说明 Sigma FM 的 JS 运行时确实有问题

---

## 5. 下一步:怎么看 Worker 的 console

### 5.1 方法 A(推荐):Console 面板的上下文下拉框

1. 打开 DevTools → **Console** 面板
2. 看 Console 顶部的**执行上下文下拉框**(通常显示 `top`)
3. 展开它,找形如 `blob:...` 或 `extension-worker` 的条目 —— 那是 worker scope
4. 切换过去,看有没有 `[focus-sync]` 日志

### 5.2 方法 B:Sources 面板的 Worker 节点

1. DevTools → **Sources** 面板
2. 左侧展开页面节点,找 **Threads** 或 **Workers** 分组
3. 展开能看到 `extension-worker.js` 之类的条目
4. 选中它,Console 会自动切到该上下文

### 5.3 方法 C:取消「Selected context only」

Console 面板设置齿轮 → 确认 **「Selected context only」没有勾选**。
勾选状态下,worker 的日志会被过滤掉。

### 5.4 粘贴问题(你刚才遇到的)

`allow pasting` 报 `Uncaught SyntaxError: Unexpected identifier 'pasting'`,说明 Chrome 把它当普通 JS 求值了 —— 这在新版 Chrome 里可能已失效。

**替代方案(不需要粘贴):**

| 需求 | 手打命令(够短,不需要粘贴) |
|---|---|
| 测 API | `sigma.context.getCurrentPath()` |
| 测 HTTP | `fetch('http://127.0.0.1:37421/health').then(r=>r.text()).then(console.log)` |
| 看 worker 是否存在 | `performance.getEntriesByType('resource').length` |

**如果一定要粘贴:**
- 手动**一个字符一个字符**敲 `allow pasting`(避免自动补全插入干扰)
- 或在 Sources 面板的 **Snippets**(代码片段)里写 —— Snippets 不受粘贴限制

---

## 6. 仍然不能回答的问题

**这次的 console 修正只能确认「activate 跑没跑」。但要定位「为什么不跑」,需要 H0-b 的证据。**

**最小验证(手打,不用粘贴):**

在 **worker scope** 的 Console 里执行:
```javascript
sigma.storage
```
**结果解读:**

| 结果 | 结论 |
|---|---|
| `undefined` | **`sigma.storage` 不可用 → H0-b 成立** → L435 抛错,activate 中断 |
| `{get: ƒ, set: ƒ, ...}` | storage 正常 → H0-b 排除,查 `loadBool` 内部 |

然后:
```javascript
sigma.storage.get('enabled')
```
**结果解读:**

| 结果 | 结论 |
|---|---|
| `Promise {<pending>}` 或 `true` | storage 调用正常 |
| `Promise {<rejected>}` | **存储层失败** → 这是真正的 root cause |

---

## 7. 沉淀规则

### 规则 70 — 跨执行上下文的日志,不能从单一上下文推断

```markdown
### 规则 70 — 确认「代码没执行」前,必须确认「日志渠道」

**触发**:2026-10-07 修正

**事实**:Sigma FM 的 extension 跑在独立 Web Worker
(worker-runtime.ts:38-43 new Worker)。
extension 的 console.log 只出现在 worker scope,
主窗口 Console 默认看不到。

我曾据此推断「activate() 未被调用」,这是错误推论。

**规则**:
1. 断言「某段代码没执行」之前,必须确认:
   - 它的日志会输出到哪个上下文?(main / worker / iframe / service worker)
   - 你的观察点覆盖那个上下文了吗?
2. Web Worker / iframe / ServiceWorker 里的日志,
   在 DevTools 里需要切换执行上下文或取消「Selected context only」
3. 「在 A 上下文没看到日志」≠「代码没执行」
```

### 规则 71 — 日志缺失时,优先检查「有无 catch」的语句顺序

```markdown
### 规则 71 — 同一函数内,catch 的分布决定中断点

**触发**:index.js:431-437

事实:
  L431 console.log(...)                    无 await
  L432 try { await ... } catch {}          ← 有 catch,不会中断
  L435 await loadBool(...)                 ← 无 catch,抛错则整个 activate 中断
  L502 setTimeout(() => pingSidecar(), 5000)  ← 第一次 HTTP 在这

推论:如果 L435 抛错,表现为「零 ping」,但 L431 的日志已经打出来了。
而 L431 的日志在 worker scope,主窗口看不到 → 误判为「没执行」。

**规则**:
1. 定位「函数在哪一行中断」时,先画出该函数内所有 await,
   标注哪些有 catch、哪些没有
2. 无 catch 的第一个 await = 最可能的静默中断点
3. 静默中断点的表现 = 「后面的代码全没执行」,但前面的日志可能已打
```

---

## 8. 元数据

| 项 | 值 |
|---|---|
| **修正对象** | 上一份报告的「决定性证据:activate 未被调用」 |
| **错误性质** | 忽略了 Web Worker 隔离,跨执行上下文推断 |
| **代码证据** | `worker-runtime.ts:38-43`(new Worker)、`extension-worker.ts:1301`(workerScope) |
| **仍成立** | `last_ping_received_ts: 0`、sidecar 正常、extension 已注册、v0.5.5 只改版本号 |
| **撤回** | 「H0 已证实」「已排除 Fix B gate」 |
| **新头号假设** | **H0-b:L435 `loadBool` 无 catch,`sigma.storage` 不可用导致 activate 中断** |
| **下一步** | 切到 worker scope,手打 `sigma.storage` 和 `sigma.storage.get('enabled')` |

---

## 9. 一句话 prompt(新会话接力)

```
读 docs/superpowers/plans/2026-10-07-focus-21-correction-worker-console.md(必读)。

修正上一份报告的错误推论:
  extension 跑在 Web Worker(worker-runtime.ts:38-43 new Worker)
  → activate() 的 console.log 只在 worker scope,主窗口 Console 看不到
  → 「主窗口没日志」≠「没执行」

撤回:H0 已证实 / 已排除 Fix B gate
保留:last_ping_received_ts: 0(最硬证据,与 console 位置无关)
     sidecar 正常 / extension 已注册 / v0.5.5 只改版本号

新的头号假设 H0-b:
  index.js:435 enabled = await loadBool('enabled', true);  ← 无 catch
  L432 有 catch(不会中断),L435/436/437 无 catch
  若 sigma.storage 不可用 → activate 在 L435 中断
  → L502 的 pingSidecar 和 L567 的 schedulePush 都不执行
  → last_ping_received_ts: 0
  但 L431 的日志已经打了,只是主窗口看不到

用户下一步(切到 worker scope):
  1. Console 顶部执行上下文下拉框 → 切到 worker
     或 Sources → Threads/Workers 展开
     或取消勾选 "Selected context only"
  2. 手打(不需要粘贴):sigma.storage
     → undefined 则 H0-b 成立
  3. 手打:sigma.storage.get('enabled')
     → Promise rejected 则存储层是真正 root cause

粘贴问题:新版 Chrome 的 allow pasting 可能失效。
替代:手打短命令 / Sources → Snippets(不受粘贴限制)。

沉淀 Rule 70(跨上下文不能推断)+ Rule 71(catch 分布定中断点)。
```