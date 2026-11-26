# 修复与验证报告：扩展不激活的真因与修复

**日期**：2026-10-14
**结论**：根因是**扩展沙箱校验器误报**，与 sidecar 版本完全无关。已修复并真机验证扩展恢复激活。

---

## 1. 根因

`src/modules/extensions/runtime/sandbox.ts` 的危险模式规则

```ts
{ pattern: /(?<![\w$.])self\b/g, message: 'Direct self access is not allowed' }
```

直接对**原始源码**执行。扩展 `dist/index.js` 中一句英文注释

```js
// your PC." — a self-inflicted false alarm caused by our own installer.
```

里的 `self-inflicted`（`self` 后接 `-`，前接空格）命中规则，扩展因此在
**创建 Worker 之前**的代码校验阶段就被拒绝，从未激活。

---

## 2. 证据

### 2.1 新增诊断通道（`__hostWorkerDiagnostics`）

修复前：

```
[15:41:12.592Z] module.validate  error  code validation failed: Direct self access is not allowed
[15:41:12.618Z] load.fail        error  Extension code validation failed: ...
```

**无任何 `initialize.start` / `activate.start` 记录** —— 失败早于 Worker 创建。

### 2.2 对照实验（真实 `validateExtensionCode`）

| 对照 | 结果 |
|---|---|
| A 线上部署版（38,550 bytes） | **FAIL** — `Direct self access is not allowed`，1 处命中 |
| B 同一文件，仅改注释里的 `self` 一词 | **PASS** |
| C git 已提交版（37,144 bytes） | **PASS** — 0 处命中 |

A 与 B 是同一份文件，唯一差异是那个词 ⇒ 排除文件损坏/编码/版本不匹配等所有其他解释。

### 2.3 引入者

```
git diff --numstat -- extensions/kizemo.focus-sync/dist/index.js
36	16	extensions/kizemo.focus-sync/dist/index.js
```

36 行未提交改动中唯一命中的一行就是那句注释，其余 35 行无害。

---

## 3. 修复

### 3.1 修复① —— 改注释（保留其余 35 行）

```diff
-// your PC." — a self-inflicted false alarm caused by our own installer.
+// your PC." — a false alarm we caused ourselves with our own installer.
```

### 3.2 修复② —— 修校验器（根治）

新增 `src/modules/extensions/runtime/strip-non-code.ts`：
在正则扫描前把**注释、字符串、模板串、正则字面量按等长置空**（保留偏移与换行），
`validateExtensionCode` 只对这份可执行代码扫描。

注释与字面量是惰性的、不会执行，因此只去除误报，不削弱对真实代码的检查。

同时修复了开发过程中发现的实现缺陷：模板串最初只置空开头的反引号、正文未置空，
`` `window.${a}` `` 里的 `window.` 仍会误判。修复后 `${...}` 插值仍保持可扫描，
藏在插值里的 `fetch()` 照样能被抓到。

### 3.3 非平凡性验证（注入缺陷）

把校验器注入回扫原始源码：

- ✗ 4 条误报用例转红（含 `self` 注释、`fetch()`/`window.`/`self.` 注释、字符串内含违规词、正则字面量中的 `self`）
- ✓ **14 条「真实违规仍被拦」用例全部保持绿色**

⇒ 修复确实在起作用，且没有削弱安全检查。

---

## 4. 真机验证（`tauri build` 产物）

用 **AppData 里仍带 `self-inflicted` 的旧文件**（sha `9D2B602E`）运行，
验证修复②能否独立救回：

```
[16:01:20.022Z] module.validate    info  code validation passed
[16:01:20.039Z] initialize.start   info  posting initialize for blob:...
[16:01:20.090Z] initialize.ok      info  worker initialized
[16:01:20.090Z] activate.start     info  posting activate
[16:01:21.628Z] activate.ok        info  extension activated
```

侧车 `/health`：

```json
{ "extension_alive": true, "first_push_received": true,
  "last_health_poll_age_ms": 4575, "version": "0.5.8" }
```

侧车日志收到路径推送：

```json
{"kind":"path_update","message":"current_path=F:\\soft\\00selfmade"}
{"kind":"set_path_dialogs","message":"dialogs_count=0"}
```

扩展自身 trace 恢复正常心跳：

```
N03.health.req → N04.health.resp status=200
N01.initialPing
N03.health.resp status=200（每 15 秒）
```

**结论：应用 → 扩展 → 侧车 三段链路已完全恢复**（这是弹窗注入的机制前提）。

---

## 5. 门禁

| 门禁 | 结果 |
|---|---|
| `eslint` | 0 |
| `vue-tsc` | 0 |
| 单测（全量 `src/modules` + `src/stores`） | **857/857 通过**（118 文件） |

新增测试 29 条（沙箱校验）+ 5 条（诊断持久化）。

### 5.1 一处需要说明的既有失败

`home-banner-click-opens-background-manager.test.ts > renders the clickable media container`
在全量并行下报 `Error: Test timed out in 5000ms`（挂点 `mountBanner()`）：

- 单独跑 3/3 通过，单独跑该用例耗时 2.8 秒（默认超时 5 秒）
- `--testTimeout=30000` 后全量 **857/857 通过**
- `home-banner.vue` 的 import 图中**没有** `@/modules/extensions` 或 `stores/runtime/extensions`

⇒ 本机并行负载下的既有慢用例超时，与本次改动无依赖关系。**未**在纯净树上验证过，
但这是排除法结论而非直接证据。

---

## 6. 未完成项

**「弹窗打开后路径确实被注入」尚未做 GUI 实跑验证。**

本环境限制：UIA 树只能走到 `RootWebArea → Group`，webview 内控件未暴露；
`SendInput` 不可用。因此无法自动触发「另存为」弹窗。

需要人工一步：在运行中的应用里触发一次下载/另存为，然后观察
`/health` 的 `active_dialogs` 是否 > 0，以及侧车日志是否出现 `Write`。

---

## 7. 本轮全部代码改动

| 文件 | 改动 |
|---|---|
| `src/modules/extensions/runtime/strip-non-code.ts` | **新增**：等长剥离注释/字面量 |
| `src/modules/extensions/runtime/sandbox.ts` | `validateExtensionCode` 改扫剥离后的代码 |
| `src/modules/extensions/runtime/worker-diagnostics.ts` | **新增**：Worker 生命周期诊断环 + sink |
| `src/modules/extensions/utils/worker-diagnostic-persistence.ts` | **新增**：防抖持久化 sink |
| `src/modules/extensions/utils/__tests__/worker-diagnostic-persistence.test.ts` | **新增**：5 条回归测试 |
| `src/modules/extensions/runtime/__tests__/sandbox-validation.test.ts` | **新增**：29 条测试 |
| `src/modules/extensions/runtime/worker-runtime.ts` | 补 `onerror`/`onmessageerror`；新增 `rejectAllPending()`（修「Promise 永久挂起」）；阶段诊断 |
| `src/modules/extensions/runtime/extension-worker.ts` | `await import` 加 try/catch，错误经既有 `bridge-response` 带回宿主 |
| `src/modules/extensions/runtime/loader.ts` | 传 `extensionId`；记录 `module.read`/`module.validate`/`module.entry`/`load.fail` |
| `src/stores/runtime/extensions.ts` | 注册持久化 sink |
| `extensions/kizemo.focus-sync/dist/index.js` | 注释改词 1 行（其余 35 行保留） |

打包产物：`release\Alpha File Manager_3.0.3_x64-setup.exe`
2026-10-14 00:00:55，18,105,689 bytes，sha256 `373618F9FD7F8832…`

运行时：sidecar **0.5.8**（sha `7C7EB7C3EB100E71…`），
`/health` 自报 `version: "0.5.8"`。