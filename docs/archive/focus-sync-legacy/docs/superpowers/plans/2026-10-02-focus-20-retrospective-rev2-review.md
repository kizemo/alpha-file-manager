# focus-20 Retrospective rev.2 Review — Mavis 意见 + 源码 Bug 检查

**日期**: 2026-10-02
**作者**: Mavis (M3)
**输入文档**:
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-architecture-failure-rev2.md`(retrospective rev.2,final)
- 当前源码:`extension/dist/index.js` / `sigma-listary-spike/src/uia_inject.rs` / `state.rs` / `http_server.rs` / `writer.rs` / `fg_bypass.rs` / `main.rs` / `logger.rs`
- 用户最新截图(`C:\downloads\` 在 filename 框)
**性质**: 第 6 轮 review(retrospective rev.2 + 实际源码 bug 检查)

---

## 0. TL;DR

**retrospective rev.2 极好**:采纳了我之前 5 处偏差的全部,文档结构清晰,focus-21 决策树合理。

**但程序代码里有 4 处 bug 待修**:

1. 🔴 **HIGH**:`main.rs` HTTP server 线程**没有 CoInitializeEx**,依赖 CoCreateInstance 隐式初始化(预存 bug,focus-17 引入)
2. 🟡 **MEDIUM**:`extension/dist/index.js` Edge legacy tip 的 `edgeLegacyFilePickerTipShown` flag 永久性 — 用户错过就再也看不到,**没有重置机制**
3. 🟡 **MEDIUM**:`extension/dist/index.js` 初次激活时,Edge tip 和 Sidecar not reachable 两个通知可能同时弹,UX 混乱
4. 🟢 **LOW**:`extension/dist/index.js` `?? true` / `?? false` 处理不区分 `null` 和 `undefined`(语义模糊,但实际行为正确)

---

## 1. Claude 做对的部分(retrospective rev.2)

### 1.1 §0 TL;DR — 5 处偏差全部采纳

| # | Mavis review 偏差 | Claude rev.2 采纳 |
|---|---|---|
| P0-1 | focus-20 v0.3.3 已 ship,需要回滚策略 | ✅ §6 新增回滚决策表 |
| P0-2 | "WinUI 3 filename + Enter 不导航" 是 CLAIM 不是 evidence | ✅ §3.1 重写为"待 verify" |
| P0-3 | SendInput 工作量 5-10d 严重低估 → 10-15d | ✅ §5 路径 B 子任务拆分 |
| P1-1 | Edge flag 一次性通知不够强 | ✅ §7 加强通知策略 4 方案 |
| P1-2 | §6 缺"接受现状"路径 E | ✅ §5 路径 E 新增 |
| P1-3 | focus-21 应按 user-verify 分支决策 | ✅ §8 决策树 |

— **100% 采纳**,这是少见的"review 反馈全部接受"。

### 1.2 §5 路径 B 子任务拆分

```
SendInput 基础实现          1-2 d
焦点抢回防御               2-3 d    ← 显式列出"fg_bypass 复辟"
Win11 多版本 Ctrl+L 适配    2-3 d    ← 显式列出"22H2/23H2/24H2/25H2"
auto_confirm gate 整合     1 d
用户输入检测                1 d
SendInput 失败回退          1-2 d
```

— **精确**列出被低估的部分,工作量估算 10-15 天比 rev.1 的 5-10 天准确。

### 1.3 §5 路径 E(新增)"接受现状 + 文档化"

— 务实推荐,与我在 review §2.5 建议一致。

### 1.4 §8 focus-21 决策树

```
User verifies WinUI 3 filename + Enter behavior
├── 如果"navigation works"  →  Option D(条件性)
└── 如果"navigation 不 work" →  A (Edge flag) / B (SendInput) / E (默认推荐)
```

— **核心原则**:"按 user-verify 结果分支,不是预设方案"。✓

### 1.5 §11.2 规则 5(新增)

> **规则 5**(rev.2 新增):**retrospective / plan 中禁止写"预期结果"**(写"测试方法 + 待 user-verify + 结论待填")。**写"预期"就是"假设性推理"的变种,违反 evidence-first**。

— 这条规则很好,应该作为后续 systematic-debugging 的硬约束。

---

## 2. 🔴 源码 Bug 检查(关键发现)

### Bug A [HIGH] — HTTP server 线程未 CoInitializeEx(预存)

**位置**:`sigma-listary-spike/src/main.rs` 行 240-247

**当前代码**:
```rust
let _http_handle = std::thread::Builder::new()
    .name("spike-http".into())
    .spawn(move || {
        if let Err(e) = http_server::run_server(state_for_http, port, mode) {
            tracing::warn!(error = %e, "http server exited");
        }
    })
    .expect("spawn http server thread");
```

**问题**:
- `main` 函数在主线程(line 89-94)调用 `CoInitializeEx(COINIT_APARTMENTTHREADED)`
- 但 HTTP server 在**新 std::thread** 上运行,**没有 CoInitializeEx**
- HTTP server → `handle_set_path` → `write_path` → `inject_folder_path` → `inject_via_uia` → `automation()` (thread_local) → `uia.ElementFromHandle(dialog)` — **UIA 调用依赖 COM**

**为什么目前看起来工作**:
- `automation()` 的 thread_local `IUIAutomation` 由 `CoCreateInstance` 创建
- 某些 Windows 版本,`CoCreateInstance` 会**隐式初始化**调用线程的 COM(STA 模式)
- 测试期间用户的 Windows 配置支持这种隐式初始化,所以 UIA 调用成功

**风险**:
- 某些 Windows 版本 / 系统配置(`RPC_E_CHANGED_MODE` / `CO_E_NOTINITIALIZED`)可能失败
- 如果用户线程已经在 MTA 模式,后续 STA 初始化会失败
- 失败模式:UIA 调用静默失败或返回错误,导致 dialog 检测/导航全部失效

**修复**:
```rust
std::thread::Builder::new()
    .name("spike-http".into())
    .spawn(move || {
        // COM init as STA — required for UIA. Each thread that uses COM
        // must call CoInitializeEx independently.
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            );
        }
        if let Err(e) = http_server::run_server(state_for_http, port, mode) {
            tracing::warn!(error = %e, "http server exited");
        }
    })
    .expect("spawn http server thread");
```

**优先级**:HIGH(预存 bug,Windows 配置变了就崩)

---

### Bug B [MEDIUM] — Edge legacy tip flag 永久性,无重置机制

**位置**:`release/extension/dist/index.js` 行 298-318

**当前代码**:
```js
try {
    const tipShown = await sigma.storage.get('edgeLegacyFilePickerTipShown');
    if (tipShown !== true) {
        sigma.ui.showNotification({
            title: 'Focus Sync — Edge Save As tip',
            description: '...',
            type: 'info',
            duration: 25000,
        });
        await sigma.storage.set('edgeLegacyFilePickerTipShown', true);
    }
} catch (e) {
    // ...
}
```

**问题**:
- 用户首次装 focus-20 → notification 弹 → flag 设为 true
- 用户错过(睡着了 / 切窗口 / notification duration 太长被忽略)
- 用户升级 / 重装 → flag 仍为 true → notification 不再弹
- **用户永远看不到这条重要提示**

**retrospective §7.2 提到的解决方案**:
- toolbar 永久按钮(方案 1)
- settings UI 重置 flag(方案 2)
- 启动检查 unsupported_dialog_count > 0(方案 3)

— **已识别**,但**未实施**。retrospective §8.1 focus-21 短期任务才有。

**修复**(focus-21 短期,先做 toolbar 按钮):
```js
async function resetEdgeTip() {
    try {
        await sigma.storage.set('edgeLegacyFilePickerTipShown', false);
        sigma.ui.showNotification({
            title: 'Focus Sync',
            description: 'Edge tip will show on next activation.',
            type: 'info',
            duration: 3000,
        });
    } catch (e) {}
}

// 在 toolbar dropdown 加:
{ id: 'resetEdgeTip', title: '🔁 Re-show Edge tip', commandId: 'focus-sync.resetEdgeTip' }
```

**优先级**:MEDIUM(用户已错过,影响 Edge flag 教育)

---

### Bug C [MEDIUM] — 初次激活双通知并发 UX 混乱

**位置**:`release/extension/dist/index.js` 行 278-340(activate 函数)

**当前代码**:
```js
const activate = async () => {
    console.log('[focus-sync] v0.3.3 activate START ...');
    // ...
    
    // Edge legacy tip(行 298-318)
    sigma.ui.showNotification({...});  // ← 触发 1
    
    // Initial ping(行 326-339)
    void pingSidecar().then((ok) => {
        if (!ok && !notifiedSinceLastFailure) {
            sigma.ui.showNotification({
                title: 'Focus Sync',
                description: 'Sidecar not reachable...',
                type: 'error',
                duration: 15000,
            });  // ← 触发 2
        }
    });
    startHealthCheck();  // 之后还有
}
```

**问题**:
- 用户首次激活 focus-20 → 看到 2 条 notification:
  1. "Edge Save As tip"(info, 25s)
  2. "Sidecar not reachable"(error, 15s)— 如果 sidecar 还没启动
- 2 条通知可能同时显示,用户不知道先看哪个
- 如果 sidecar 启动延迟(15s 内),用户先看到 "Sidecar not reachable"(红色 error),恐慌

**修复**:
1. **延迟 initial ping 5s**(给 sidecar 启动时间)
2. **Edge tip 优先级高**,先显示,initial ping 后显示
3. **合并到一个 notification**(不优雅但实用)

**推荐方案 1**:
```js
// Edge tip(立即显示)
sigma.ui.showNotification({...Edge tip...});

// 延迟 initial ping 5s — 给 sidecar 启动时间
setTimeout(() => {
    void pingSidecar().then((ok) => {
        if (!ok && !notifiedSinceLastFailure) {
            sigma.ui.showNotification({...});
        }
    });
}, 5000);

startHealthCheck();
```

**优先级**:MEDIUM(首次用户体验差)

---

### Bug D [LOW] — `?? true` / `?? false` 语义模糊

**位置**:`release/extension/dist/index.js` 行 281-282

**当前代码**:
```js
try { enabled = (await sigma.storage.get('enabled')) ?? true; } catch (e) { enabled = true; }
try { autoConfirm = (await sigma.storage.get('autoConfirm')) ?? false; } catch (e) { autoConfirm = false; }
```

**问题**:
- `??` 仅在 `null` / `undefined` 时使用默认值
- 如果 `sigma.storage.get()` 返回 `false` 显式(用户上次设置 enabled=false),`false ?? true` = `false` ✓
- 如果返回 `null` / `undefined`(用户首次装),`null ?? true` = `true` ✓

— 实际上逻辑是正确的。`??` 比 `||` 更精确,`||` 会把 `false` 也视为 falsy。

**但问题**:如果 `sigma.storage.get` 抛异常(被 try/catch 捕获),统一用 catch 分支的 hardcode 值:
- enabled → true(默认启用)✓
- autoConfirm → false(默认关闭)✓

— 双重 fallback 防御,但代码读起来重复。

**建议**(可选清理):
```js
async function loadBool(key, defaultValue) {
    try {
        const v = await sigma.storage.get(key);
        return v ?? defaultValue;
    } catch (e) {
        return defaultValue;
    }
}
const enabled = await loadBool('enabled', true);
const autoConfirm = await loadBool('autoConfirm', false);
```

— **优先级**:LOW(逻辑正确,只是可读性)

---

## 3. 🟢 其他次要问题

### 3.1 [P3] `unsupported_dialog_count` 累加语义

**位置**:`uia_inject.rs::inject_via_uia` 行 486-490 + `inject_folder_path` 行 308-315

**逻辑**:
- `inject_via_uia` 内的 `else` 分支(no Edit element)调用 `inc_unsupported_dialog()` → 返回 `Ok(Unsupported)`
- `inject_folder_path` 内的 Err 分支也调用 `inc_unsupported_dialog()` → 返回 `Unsupported`
- 但 `inject_via_uia` 内部的所有 `Ok(Unsupported)` 都已累加,`inject_folder_path` 不再累加

**风险**:如果未来 `inject_via_uia` 新增其他返回 `Ok(Unsupported)` 的路径,会**漏累加**或**双累加**。

**建议**:把累加逻辑统一到 `inject_folder_path`(在 match 分支后):
```rust
match inject_via_uia(...) {
    Ok(DialogOutcome::Unsupported) => {
        state.inc_unsupported_dialog();  // 集中累加
        DialogOutcome::Unsupported
    }
    Err(err) => {
        state.inc_unsupported_dialog();  // 集中累加
        warn!(...);
        DialogOutcome::Unsupported
    }
    ...
}
```

并从 `inject_via_uia` 内部**移除** `state.inc_unsupported_dialog()`,改为 `inject_via_uia` 是纯函数(不修改 state),`inject_folder_path` 负责累加。

— **优先级**:P3(逻辑当前正确,但重构可降低未来 bug 风险)

### 3.2 [P3] Edge tip notification duration 太长

**位置**:`extension/dist/index.js` 行 315

**当前代码**:
```js
sigma.ui.showNotification({...duration: 25000});
```

**问题**:25 秒太长了。多数 notification 平台用 5-10 秒。25 秒会:
- 占用屏幕顶部中央位置很久
- 让其他通知排队显示

**建议**:duration 改 8000-10000ms。如果用户错过,有 Bug B 的 toolbar 按钮兜底。

— **优先级**:P3(UX 小问题)

### 3.3 [P3] `chatty` log warning

**位置**:`uia_inject.rs::inject_via_uia` 行 359 + 行 392-395

**当前代码**:
```rust
info!("=== Dialog UIA inventory (hwnd={dialog_hwnd}, total descendants={count}) ===");
for i in 0..count {
    // ...
    info!("  ctrl={:?} automation_id='{}' name='{}' value='{}'", ...);
}
info!("=== End of dialog inventory ===");
```

**问题**:每次 dialog 触发都打 100+ 行 inventory log。Edge WinUI Save As dialog 有 106 个 descendants → log 文件暴涨。

**建议**:debug-level 而非 info-level。Production 用 `info,spike=debug` 但 inventory 应该用 `debug!`。

— **优先级**:P3(log 文件大小,但属于 diagnostic feature)

### 3.4 [INFO] 预存但已识别的限制

| # | 限制 | 来源 |
|---|---|---|
| B-1 | UIA elements 线程亲和性(主线程 vs HTTP thread) | 已知 — `thread_local` COM |
| B-2 | `committed_hwnds` hwnd 复用冲突 | 已知 — hwnd 可能跨 dialog session 复用 |
| B-3 | Unicode / RTL path 处理 | 已知 — BSTR 内部处理,但未 user-verify |
| B-4 | Sigma FM normalize 覆盖 v0.3.0 patch | 已知 — round 19c 风险 |

— 这些都是历史 bug,**focus-20 没引入新版本**,但仍存在。

---

## 4. 风险矩阵(补充)

| # | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| R26 | HTTP server 线程 COM 未初始化(预存) | 中 | UIA 调用失效 | **Bug A 修复** |
| R27 | 用户错过 Edge tip,flag 永久性 | 中 | Edge flag 教育失败 | **Bug B 修复(toolbar 按钮)** |
| R28 | 初次激活双 notification UX 混乱 | 中 | 首次体验差 | **Bug C 修复(延迟 initial ping)** |
| R29 | `unsupported_dialog_count` 累加路径未来误改 | 低 | 计数不准 | 重构累加逻辑(§3.1) |
| R30 | inventory log 文件增长 | 中 | disk 占满 | 改 debug-level(§3.3) |
| R31 | COM 隐式初始化跨 Windows 版本不一致 | 中 | 部分用户 UIA 失效 | Bug A 修复是彻底方案 |

---

## 5. 给 Claude 的总结指令

### 必修(2 处)

1. **Bug A (HIGH) — HTTP server 线程 CoInitializeEx**:在 `main.rs` 行 240 的 `spawn` 闭包内,主线程的 `CoInitializeEx` 之前调用,新增对 HTTP server 线程的 CoInitializeEx 调用。这是**预存 bug,focus-17 引入,5 轮 review 都没发现**。
2. **Bug B (MEDIUM) — Edge tip flag 重置机制**:focus-21 短期任务加 toolbar 按钮 "🔁 Re-show Edge tip"(已在 rev.2 §8.1 列出,但需实施)。

### 建议(2 处)

3. **Bug C (MEDIUM) — 双 notification UX**:初次激活时 initial ping 延迟 5s(给 sidecar 启动时间),避免与 Edge tip 同时显示。
4. **Bug D (LOW) — 重构 `loadBool` helper**:可选,提升可读性。

### 不必改(预存)

§3 列出 4 处预存限制(B-1 ~ B-4),属于历史 risk,**focus-20 没引入新版本**。可在 focus-22+ 一并清理。

---

## 6. 给用户的核心建议

**retrospective rev.2 极好 — Claude 这次接受了所有偏差,文档结构清晰**。focus-21 决策树是合理的下一步。

**但源码里仍有问题**:
1. **Bug A(HIGH)** 预存 5 轮的 COM 初始化 bug — 跨 Windows 版本可能崩
2. **Bug B(MEDIUM)** Edge tip 用户错过永久看不到 — 需要 toolbar 按钮兜底

**这两处应在 focus-21 v0.3.4 一起修**,不要等 focus-22+。

### 行动清单

| 优先级 | 任务 | 来源 |
|---|---|---|
| 🔴 HIGH | **修 Bug A(HTTP server CoInitializeEx)** | 本 review §2.1 |
| 🟡 MEDIUM | **修 Bug B(Edge tip toolbar 按钮)** | 本 review §2.2 + retrospective §8.1 |
| 🟡 MEDIUM | **修 Bug C(延迟 initial ping 5s)** | 本 review §2.3 |
| 🟡 MEDIUM | README 加 Edge Modern Save As 章节 | retrospective §8.1 task #3 |
| 🟢 LOW | 修 Bug D(可选 `loadBool` 重构) | 本 review §2.4 |
| 🟢 LOW | inventory log 改 debug-level | 本 review §3.3 |
| 🔴 HIGH | **User-verify Edge WinUI filename + Enter 行为** | retrospective §3.1 |

**预计 focus-21 v0.3.4 工作量**:
- Bug 修复:2-3 h
- README:1 h
- User-verify:30 min(用户)
- Build + 集成测试:1 h

**总计**:4-5 h + 30 min user 验证。

---

## 7. 给后续会话的关键 takeaway

1. **retrospective 框架好用**:Claude 这次 system-debugging + Phase 4.5 触发 + 用户决策点,**方法论已被验证**。
2. **源码 bug 检查应在 review 里加项**:5 轮 review 都只看 retrospective 文档,**没人检查实际代码**。这次发现了 Bug A(HTTP thread COM 未初始化)是 focus-17 引入但**5 轮 review 全没发现**。
3. **Claude 接受偏差的态度好**:rev.2 100% 采纳了我之前 review 的 5 处偏差,**没有辩护**。这是协作应有的态度。
4. **focus-21 决策树是好的产品决策**:不再"我做我做我",而是"用户验证,我执行"。
5. **Bug A 是系统性盲点**:threaded COM 初始化是 Windows 编程常被忽略的细节,任何后续涉及 threading + COM 的 Rust 项目都应警惕。
