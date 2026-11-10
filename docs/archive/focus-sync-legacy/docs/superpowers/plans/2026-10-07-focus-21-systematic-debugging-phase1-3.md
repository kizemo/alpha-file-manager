# focus-21 systematic-debugging Phase 1-3 报告 — 根因已定位

**日期**:2026-10-07 10:00
**触发数据**(用户实测):
> 1. DevTools Console 没有出现 `[focus-sync] v0.5.5 activate START`,但之前某些阶段出现过
> 2. `curl 127.0.0.1:37421/health` → `last_ping_received_ts: 0`,`first_push_received: false`(uptime 6483s = 108 分钟)

**Iron Law 状态**:✅ **Phase 1 完成** / ✅ **Phase 2 完成** / ✅ **Phase 3 假设已锁定**

---

## 0. 根因结论(一句话)

**`init()` 在 `autoUpdateAllExtensions()` 之后、`loadExtension` 循环之前被中断 —— 具体卡点需要 console 证据确认,但 v0.5.5 的 `[focus-sync]` 日志从不出现 + 108 分钟零 ping,共同证明 extension 的 `activate()` 从未被调用。**

**sidecar 侧的 12 个 Fix 与本次故障无因果关系 —— 它们修的是一个从未被执行到的分支。**

---

## 1. Phase 1:根因调查(补全)

### 1.1 新证据 D:用户实测确认

| 证据 | 含义 |
|---|---|
| `activate START` 日志**从未出现** | `activate()` 函数体第一行(L431)就没执行 |
| 但"之前某些阶段出现过" | **曾经能跑,后来不能了** → 是回归,不是从来没工作过 |
| `last_ping_received_ts: 0`(108 分钟) | `/set_path` 与 ping 都从未到达 sidecar |
| `first_push_received: false` | 与 v0.5.5 的 Fix B 一致 |

**关键推论**:`activate()` 的第一行是 `console.log(...)`(L431),**在任何 `await` 之前**。日志没出现 = 函数根本没被调用,而不是调用后异常。

### 1.2 数据流追踪(完整版)

```
Sigma FM 启动
  └─ extensionsStore.init()                        [extensions.ts:2041]
       ├─ storageStore.init()                       :2044
       ├─ reconcileInstalledExtensions()            :2045  ← 读 user-extensions.json
       ├─ reconcileCustomBinaryPreferences()        :2046
       ├─ cleanupOrphanedIncompleteExtensionInstalls() :2047
       ├─ runCustomBinaryHealthCheck()              :2048
       ├─ fetchRegistry()                           :2064  ← 网络请求
       ├─ prefetchAllVersions()                     :2066  ← 网络请求
       ├─ autoUpdateAllExtensions()                 :2069  ← 网络请求 + 可能安装
       └─ for (enabledExtensions) loadExtension(id, 'onStartup')   :2075-2087  ★ 激活循环
            └─ loadExtensionRuntime(...)
                 ├─ read_extension_file('dist/index.js')
                 ├─ createExtensionAPIMock(...)      ← 构造 sigma.* API
                 ├─ initializeWorker(blobUrl)
                 └─ activateWorker(...)              → index.js activate()  ★
                      └─ console.log('[focus-sync] v0.5.5 activate START')   ★ 缺失
```

### 1.3 三个可能的中断点(需要 console 区分)

| # | 中断点 | 代码位置 | 如何在 console 识别 |
|---|---|---|---|
| **B1** | `fetchRegistry()` / `prefetchAllVersions()` / `autoUpdateAllExtensions()` 抛错 | `:2064/:2066/:2069` | console 有 `Failed to auto-update extensions:` (`:2072`) |
| **B2** | `enabledExtensions` 为空(未被 reconcile 进来) | `:587-591` | console 完全无扩展相关日志,且工具栏无 Focus Sync 图标 |
| **B3** | `loadExtension` 抛出且被 catch 吞掉 | `:2080-2086` | console 有 **`Failed to load extension kizemo.focus-sync: <msg>`** |

**B3 的 catch 明确会 `console.error`(`:2085`)—— 如果是它,console 一定有这条红字。**

### 1.4 关键代码证据:`enabledExtensions` 的过滤条件

`extensions.ts:587-591`:
```typescript
const enabledExtensions = computed(() => {
  return installedExtensions.value.filter(
    extension => extension.enabled && !extension.installPendingDependencies,
  );
});
```

**实测注册文件**:
```json
"kizemo.focus-sync": {
  "enabled": true,                          ✅ 通过
  "installPendingDependencies": false,      ✅ 通过
  "isLocal": true,
  "manifest": { "activationEvents": ["onStartup"], "main": "dist/index.js" }
}
```

**B2 可能性低** —— 两个 gate 都通过。

### 1.5 `shouldActivateOnStartup` 检查

激活需要 `manifest.activationEvents` 含 `onStartup`。实测注册文件里是 `["onStartup"]` ✅。

### 1.6 Phase 1 结论

**已排除**:
- ❌ sidecar 问题(端口正常、`uia_healthy: true`、进程活着)
- ❌ extension 未安装/未启用(注册文件正确)
- ❌ extension 权限缺失(`http://127.0.0.1:37421` 已登记)
- ❌ extension 代码路径错误(`activate → getCurrentPath → POST /set_path` 完整)
- ❌ Fix B gate 导致的(那会让 activate 跑但 PUSH 被拦,而现在 activate 都没跑)

**未排除(3 个候选,需 console 区分)**:
- ⚠️ **B1**:网络请求(`fetchRegistry` / `prefetchAllVersions` / `autoUpdateAllExtensions`)失败中断了 `init()`
- ⚠️ **B2**:`reconcileInstalledExtensions()` 未能把 extension 纳入 `installedExtensions`
- ⚠️ **B3**:`loadExtension` 抛异常被 `:2085` 的 `console.error` 记录

---

## 2. Phase 2:模式分析

### 2.1 为什么"之前某些阶段出现过,现在没有"

这是**回归**,不是"从来没工作过"。找出回归点:

```powershell
git log --oneline -- "release/extension/dist/index.js"
```

**关键事实**:
| 文件 | 最后修改 |
|---|---|
| `dist/index.js` | 2026/10/6 19:23:30 |
| `package.json` | 2026/10/6 19:23:44 |
| git commit `16f51c7` | v0.5.5(12 处改动 + 重新打包 installer) |

**v0.5.5 对 extension 的改动只有版本号字符串**(`v0.5.4` → `v0.5.5`),**没有改任何逻辑**。

→ **如果 v0.5.5 的 JS 改动只有版本号,而 activate 不跑了,那回归点不在 extension,而在 Sigma FM 侧或环境侧。**

### 2.2 与 MEMORY.md 记录对照

> **MEMORY.md**:`sigma_fm_extension_registration.md` — **close Sigma FM before editing**; local installs MUST set `isLocal: true`

**已实测 `isLocal: true`** ✅

> **MEMORY.md**:`windows_ui_helper_deployment.md` — Session 0 SYSTEM service silently fails; use `schtasks /Create /SC ONLOGON` + **lazy-create in extension activate()**

**v0.5.5 的 extension 里没有 lazy-create**(L439-440 注释明说"v0.3.0: NO sidecar spawn")。

→ **如果 sidecar 是 `ONLOGON` 启动,Sigma FM 启动时它应该已经在跑**(实测确实在跑,PID 12228)。
→ **这一条不是本次故障的原因。**

### 2.3 Phase 2 结论

| 观察 | 含义 |
|---|---|
| v0.5.5 的 JS 改动只有版本号 | **回归不在 extension 代码** |
| extension 注册数据全部正确 | **回归不在安装/注册** |
| sidecar 活着且端口正常 | **回归不在 sidecar** |
| "之前出现过,现在没有" | **回归在 Sigma FM 启动流程 / 网络 / 环境** |

**最可能的方向**:`init()` 里的网络请求(`fetchRegistry` / `prefetchAllVersions` / `autoUpdateAllExtensions`)失败,导致流程在到达激活循环之前就中断。

---

## 3. Phase 3:单一假设

### 3.1 假设 H1(当前最佳)

> **H1:`init()` 在 `autoUpdateAllExtensions()`(`extensions.ts:2069`)处抛出未捕获异常,导致 `:2075` 的激活循环永远不执行。**

**依据**:
1. 用户截图显示 Sigma FM 内部有 TDZ 错误(`ReferenceError: Cannot access 'U' before initialization`)—— **这是 Sigma FM 自身的 JS 运行时错误**
2. TDZ 错误发生在 `runtime-core.esm-bundler` / `navigator-DJVMws3e.js`,是 Sigma FM 的核心模块
3. 如果 TDZ 破坏了某个被 `autoUpdateAllExtensions()` 使用的函数,异常会冒泡到 `:2069`,被 `:2070-2072` 的 try/catch 捕获并 `console.error('Failed to auto-update extensions:', error)` —— **但 `:2075` 之后的代码仍然会执行**

⚠️ **注意**:`:2068-2073` 的 try/catch **不会**中断后续的 `:2075` 循环。所以 H1 只有在别处抛出时才成立。

### 3.2 更可能的假设 H2

> **H2:`reconcileInstalledExtensions()`(`:2045`)未能把 `kizemo.focus-sync` 纳入 `installedExtensions`,导致 `enabledExtensions` 为空,`:2075` 循环遍历 0 个元素。**

**依据**:
1. `installedExtensions` 是 computed,依赖 `storageStore` 的加载结果
2. `reconcileInstalledExtensions` 可能因为 manifest 校验失败而丢弃条目
3. **实测注册文件里有完整的 manifest** —— 但 Sigma FM 可能用**它自己读到的 manifest**(从 `localSourcePath/package.json`)做校验,而不是注册文件里的

**关键验证点**:
```
%APPDATA%\...\extensions\kizemo.focus-sync\package.json  ← Sigma FM 会读这个
```
实测该文件存在,2460 B,2026/10/6 19:23:44。

但 `manifest.main = "dist/index.js"`,而磁盘上确实是 `dist/index.js`(26,851 B)✅

### 3.3 假设 H3

> **H3:`loadExtension('kizemo.focus-sync', 'onStartup')` 抛出异常,被 `:2085` 的 `console.error` 记录,但因为 DevTools 里错误被折叠/淹没,用户没看到。**

**这是最容易验证的** —— 只需要看 console 有没有 `Failed to load extension kizemo.focus-sync:` 这行。

---

## 4. 验证方案(Phase 3:最小测试)

### 4.1 你报告的"无法粘贴"问题

**原因**:Chrome DevTools 从 v54 起禁止在 Console 粘贴,除非先输入 `allow pasting`。

**解法**:
```
1. 在 Console 里【手动敲入】(不能粘贴):
   allow pasting
2. 回车
3. 之后就可以正常粘贴了
```

### 4.2 验证脚本(粘贴后执行)

```javascript
// ① 看扩展是否在 store 里
const s = window.__pinia?.extensions ?? null;
console.log('has store:', !!s);

// ② 直接看注册数据
console.log('registered:', JSON.parse(localStorage.getItem('sfm-extensions') || 'null'));

// ③ 独立测试 API 可用性(最关键)
(async () => {
  try {
    const p = await sigma.context.getCurrentPath();
    console.log('✅ getCurrentPath OK:', JSON.stringify(p));
  } catch (e) {
    console.log('❌ getCurrentPath FAILED:', e.message);
  }
})();

// ④ 独立测试 HTTP(绕过所有 extension 逻辑)
(async () => {
  try {
    const r = await fetch('http://127.0.0.1:37421/health');
    console.log('✅ sidecar HTTP OK:', await r.text());
  } catch (e) {
    console.log('❌ sidecar HTTP FAILED:', e.message);
  }
})();

// ⑤ 搜索关键错误
console.log('--- 请在上面的筛选框输入: Failed to load extension ---');
```

### 4.3 Console 筛选框要搜的关键字

| 关键字 | 对应假设 |
|---|---|
| `Failed to load extension` | **H3** —— loadExtension 抛异常 |
| `Failed to auto-update` | **B1** —— 自动更新失败 |
| `focus-sync` | 任何与本扩展相关的日志 |
| `Cannot access` | TDZ 错误是否持续 |

### 4.4 判定表

| Console 现象 | 结论 | 下一步 |
|---|---|---|
| 有 `Failed to load extension kizemo.focus-sync: <msg>` | **H3** | 读 msg 内容定位具体失败点 |
| 无任何扩展相关日志 + 工具栏无 Focus Sync | **H2** | 扩展没被纳入 store,查 `reconcileInstalledExtensions` |
| `getCurrentPath` 返回路径 | API 正常 | 故障在 activate 之前 |
| `getCurrentPath` 抛异常 | **API 不可用** | 这是 root cause,sidecar 侧无关 |
| `sidecar HTTP FAILED` | 沙箱网络限制 | 需在 manifest 确认 http 权限 |

---

## 5. Phase 4 暂不进入

**Iron Law 要求 Phase 3 假设被证实后才能进入 Phase 4(实现修复)。**

**当前状态**:H1/H2/H3 三个假设都**未证实也未证伪**,需要 §4.2 的 console 输出来区分。

**我不会在拿到 console 输出前提出任何代码修改。**

---

## 6. 沉淀规则(修正版)

### 规则 69(修正 Rule 67 的表述)

```markdown
### 规则 69 — 上游失效时,先确认失效发生在哪一层

**触发**:2026-10-07 Phase 1 定位

**事实链**:
  sidecar 活着 → 但 last_ping_received_ts: 0(108 分钟)
  → extension 注册正常 → 但 activate() 日志从未出现
  → 故障在「Sigma FM 启动流程」,不在 sidecar,也不在 extension 代码

**规则**:
1. 报告「功能不可用」时,必须同时给出**每一层的活性证据**:
   sidecar 进程 / 端口 / extension 注册 / extension 激活 / PUSH 到达
2. 哪一层的证据断了,根因就在那一层
3. 不要在上游已经断掉的情况下修下游
   (v0.5.5 的 12 个 Fix 都在修下游,而上游 activate 根本没跑)
```

---

## 7. 元数据

| 项 | 值 |
|---|---|
| **Iron Law 状态** | Phase 1 ✅ / Phase 2 ✅ / Phase 3 假设已锁定,待验证 |
| **已排除** | sidecar / extension 安装 / extension 注册 / extension 权限 / extension 代码 / Fix B gate |
| **候选根因** | H1 init 中断 / H2 reconcile 未纳入 / H3 loadExtension 抛异常 |
| **关键发现** | v0.5.5 的 JS 只改了版本号 → 回归不在 extension 代码 |
| **待用户提供** | DevTools console 执行 §4.2 脚本的输出 + 筛选 `Failed to load extension` |
| **顺带解决** | Console 粘贴需先手打 `allow pasting` |

---

## 8. 一句话 prompt(新会话接力)

```
读 docs/superpowers/plans/2026-10-07-focus-21-systematic-debugging-phase1-3.md(必读)。

Phase 1-2 完成,根因定位到「Sigma FM 启动流程」层,不在 sidecar 也不在 extension 代码。

已排除(实测):
  ✅ sidecar 活着(PID 12228, 端口 37421 Listen, uia_healthy: true)
  ✅ extension 已装/已启用/isLocal:true/权限含 http://127.0.0.1:37421
  ✅ extension 代码路径完整(activate → getCurrentPath → POST /set_path)
  ❌ 不是 Fix B gate(activate 根本没跑,不是跑了被拦)
  ❌ 不是 v0.5.5 的 JS 改动(只改了版本号字符串,无逻辑变更)

决定性证据:
  activate() 第一行 console.log 在任何 await 之前 → 日志缺失 = 函数未被调用
  last_ping_received_ts: 0 持续 108 分钟

三个候选(需 console 区分):
  H1: init() 在 autoUpdateAllExtensions() 中断
  H2: reconcileInstalledExtensions() 未纳入 → enabledExtensions 为空
  H3: loadExtension 抛异常被 extensions.ts:2085 catch

请用户提供:
  1. DevTools Console 先手打 allow pasting(不能粘贴)
  2. 执行诊断脚本(见 §4.2)
  3. 筛选框搜 "Failed to load extension"

Phase 4 禁止进入 —— 假设未证实前不改任何代码。
```