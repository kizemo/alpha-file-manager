# focus-20 Retrospective #3 (rev.3) — 架构复盘 + 源码 Bug 整合 + focus-21 v0.3.4 plan

**日期**: 2026-10-02
**作者**: Claude (MiniMax-M3)
**输入**:
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-architecture-failure-rev2.md`(retrospective rev.2)
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-rev2-review.md`(Mavis 第 6 轮 review,4 源码 Bug + 4 P3 改进)
- 实际源码:`extension/dist/index.js` / `sigma-listary-spike/src/uia_inject.rs` / `main.rs` / `state.rs`
- 用户最新截图(`C:\downloads\` 在 filename 框)
**supersedes**: retrospective rev.2(`focus-20-retrospective-architecture-failure-rev2.md`)
**性质**: 第 6 轮 review 后,文档 final + focus-21 v0.3.4 实施 plan

---

## 0. TL;DR (60 秒摘要)

**Mavis 第 6 轮 review 的核心发现**:**5 轮 review 全在文档层面,没人查实际代码**。结果暴露 4 个源码 Bug:

| # | Bug | 严重度 | 状态 |
|---|---|---|---|
| A | **HTTP server 线程未 CoInitializeEx**(focus-17 埋的雷) | 🔴 HIGH | **必修** |
| B | Edge tip flag 永久性,无重置机制 | 🟡 MEDIUM | **必修** |
| C | 初次激活双 notification UX 混乱 | 🟡 MEDIUM | **必修** |
| D | `?? true/false` 语义模糊 | 🟢 LOW | 建议重构 |

**focus-21 v0.3.4 工作量**:**4-5 h + 30 min user-verify**

**focus-21 v0.3.4 改动 = Bug A/B/C 修复 + README Edge 章节**(用户决策 + 路径 A 通知升级 + Bug D 暂不实施)+ 4 P3 改进(累加重构、log 降级等)。

**架构层结论不变**(rev.2 已确立):PathWrap 不适合 WinUI 3,5 轮焦点改动错位。

---

## 1. 5 轮修复失败的真相(沿用 rev.2)

每轮 focus 都在写 Filename 控件(green arrow),用户要的是更新 BreadcrumbBar/NavigationView(red arrow)。dialog 模型根错位。

---

## 2. systematic-debugging Phase 4.5 触发(沿用 rev.2)

5 轮修复失败 → 质疑 PathWrap 架构。已确立:停止 v0.3.x 小修。

---

## 3. 假设部分(沿用 rev.2 CLAIM 标记)

§3.1 重写为"测试方法 + 待 user-verify + 结论待填",**禁止写预期**。

---

## 4. Phase 4.5 架构质疑(沿用 rev.2)

PathWrap 假设不适合 WinUI 3。**新发现(Mavis 第 6 轮)**:不只是架构假设错,**5 轮 focus 改动还埋下了源码层 Bug**。

---

## 5. 5 个新架构路径(沿用 rev.2)

路径 A (Edge legacy flag) / B (SendInput) / C (NavigationView) / D (暂停) / **E (接受现状 + 文档化,推荐 default fallback)**。

---

## 6. focus-20 v0.3.3 回滚策略(沿用 rev.2)

**当前状态**: 已 ship,17,071,214 bytes,MD5 `882005bb...`,用户已装。

5 个回滚选项(推荐不主动回滚,继续 focus-21)。

---

## 7. Edge legacy flag 通知策略升级(沿用 rev.2)

4 个 UX 改动方案(toolbar button + settings UI + 启动检查 + README 章节)。

---

## 8. focus-21 决策树(沿用 rev.2,user-verify 后分支)

**但决策树之外,本节(Mavis 第 6 轮新增)**:**源码 Bug 必须先修,无论 user-verify 结论**。

---

## 9. 🔴 源码 Bug 检查 — Mavis 第 6 轮 review 新增

> **核心洞察**:5 轮 review 全在文档/计划层,**没人查实际代码**。这次暴露的 Bug A 表明系统性盲点。

### 9.1 Bug A [HIGH] — HTTP server 线程未 CoInitializeEx

**位置**:`sigma-listary-spike/src/main.rs` 第 240-247 行(close 到 thread::Builder::new().spawn() 的位置)

**当前代码**(per review):
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

**问题诊断**:
- 主线程(`async fn main()`)line 89-94 调用 `CoInitializeEx(COINIT_APARTMENTTHREADED)`
- HTTP server 在**新 std::thread** 运行,**没有自己的 CoInitializeEx**
- HTTP server → `handle_set_path` → `write_path` → `inject_folder_path` → `inject_via_uia` → `automation()` (thread_local) → `uia.ElementFromHandle(dialog)` — **UIA 调用依赖 COM**

**为什么目前看起来 work**:
- `automation()` 的 thread_local `IUIAutomation` 由 `CoCreateInstance` 创建
- 某些 Windows 版本 `CoCreateInstance` **隐式初始化**调用线程的 COM(STA 模式)
- 测试期间用户 Windows 配置支持这种隐式初始化

**风险**:
- 某些 Windows 版本 / 系统配置(`RPC_E_CHANGED_MODE` / `CO_E_NOTINITIALIZED`)可能失败
- 如果用户线程已在 MTA 模式,后续 STA 初始化会失败
- 失败模式:UIA 调用静默失败或返回错误,**导致 dialog 检测/导航全部失效**(用户感知:又"完全无变化")

**影响范围**:影响所有 dialog(IFileDialog / Chromium / WinUI 3),不仅 Edge Modern Save As。

**修复方案**(修复代码草稿,focus-21 v0.3.4 实施):
```rust
// main.rs 第 240-247 行 替换为:
let _http_handle = std::thread::Builder::new()
    .name("spike-http".into())
    .spawn(move || {
        // COM init as STA — required for UIA. Each thread that uses COM
        // must call CoInitializeEx independently. focus-21: per Mavis
        // review §2.1 — pre-existing bug from focus-17.
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

**验证判据**:
- 在多种 Windows 版本测试(22H2 / 23H2 / 24H2)
- 确认 UIA 调用不再依赖隐式 COM 初始化
- `cargo test` 全 PASS

**优先级**:**🔴 HIGH,focus-21 v0.3.4 必修**

---

### 9.2 Bug B [MEDIUM] — Edge tip flag 永久性,无重置机制

**位置**:`release/extension/dist/index.js` 行 298-318(maybeShowEdgeLegacyTip 函数)

**当前代码**(per review):
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

**问题诊断**:
- 用户首次装 focus-20 → notification 弹 → flag 设为 true
- 用户错过(睡着 / 切窗口 / duration 太长被忽略)
- 升级 / 重装 → flag 仍为 true → notification 不再弹
- **用户永远看不到这条重要提示**(导致 Edge legacy flag 教育失败)

**修复方案**(focus-21 v0.3.4 实施 toolbar 按钮 + settings 重置):

**Toolbar 永久按钮**(`activate()` 内 toolbar registerDropdown):
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

// 在 registerDropdown items 数组加:
{ id: 'resetEdgeTip', title: '🔁 Re-show Edge tip', commandId: 'focus-sync.resetEdgeTip' }

// 在 command 注册处加:
sigma.commands.registerCommand({ id: 'focus-sync.resetEdgeTip', title: 'Re-show Edge tip' }, resetEdgeTip);

// 在 registerDropdown 第 2 参数 callbacks 加:
{ syncNow, toggle, toggleAutoConfirm, resetEdgeTip }
```

**验证判据**:
- 用户点 toolbar "🔁 Re-show Edge tip" → notification "will show on next activation"
- 重启 Sigma FM → Edge tip notification 再次显示

**优先级**:**🟡 MEDIUM,focus-21 v0.3.4 必修**

---

### 9.3 Bug C [MEDIUM] — 初次激活双 notification UX 混乱

**位置**:`release/extension/dist/index.js` 行 278-340(`activate()` 函数)

**当前代码**(per review):
```js
const activate = async () => {
    // ...
    
    // Edge legacy tip(行 298-318)— 触发 1
    sigma.ui.showNotification({...});
    
    // Initial ping(行 326-339)— 触发 2
    void pingSidecar().then((ok) => {
        if (!ok && !notifiedSinceLastFailure) {
            sigma.ui.showNotification({...});  // 'error' 类型
        }
    });
    startHealthCheck();
}
```

**问题诊断**:
- 用户首次激活 focus-20 → 看到 2 条 notification:
  1. "Edge Save As tip"(info, 25s)
  2. "Sidecar not reachable"(error, 15s)— 如果 sidecar 还没启动
- 2 条同时显示,用户不知道先看哪个
- 如果 sidecar 启动延迟(15s 内),用户先看到 "Sidecar not reachable"(红色 error),**恐慌**

**修复方案**(focus-21 v0.3.4 实施延迟 initial ping):

```js
// activate() 函数内,Edge tip 显示后,initial ping 延迟 5s
sigma.ui.showNotification({...Edge tip...});

// 延迟 initial ping 5s — 给 sidecar 启动时间(Scheduled Task 模式)
setTimeout(() => {
    void pingSidecar().then((ok) => {
        if (!ok && !notifiedSinceLastFailure) {
            sigma.ui.showNotification({
                title: 'Focus Sync',
                description: 'Sidecar not reachable...',
                type: 'error',
                duration: 15000,
            });
        }
    });
}, 5000);

startHealthCheck();  // 保留原 timing
```

**验证判据**:
- 用户首次激活 → 先看到 Edge tip → 5s 后才检查 sidecar(给 Scheduled Task 启动时间)
- 不再出现"Sidecar not reachable" + Edge tip 同时弹

**优先级**:**🟡 MEDIUM,focus-21 v0.3.4 必修**

---

### 9.4 Bug D [LOW] — `?? true/false` 重构为 `loadBool` helper

**位置**:`release/extension/dist/index.js` 行 281-282

**当前代码**(per review):
```js
try { enabled = (await sigma.storage.get('enabled')) ?? true; } catch (e) { enabled = true; }
try { autoConfirm = (await sigma.storage.get('autoConfirm')) ?? false; } catch (e) { autoConfirm = false; }
```

**诊断**(per Mavis):逻辑正确,只是可读性。

**修复方案**(可选清理,focus-22+ 或低优先级):
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

**优先级**:**🟢 LOW,focus-22+ 实施或本轮可选**

---

## 10. P3 改进(Mavis 第 6 轮 review §3)

### 10.1 §3.1 [P3] `unsupported_dialog_count` 累加语义集中化

**位置**:`uia_inject.rs::inject_via_uia` 行 486-490 + `inject_folder_path` 行 308-315

**当前逻辑**(per review):
- `inject_via_uia` 内的 `else`(no Edit element)调用 `inc_unsupported_dialog()` → 返回 `Ok(Unsupported)`
- `inject_folder_path` 内的 Err 分支也调用 `inc_unsupported_dialog()` → 返回 `Unsupported`
- 但 `inject_via_uia` 内部的所有 `Ok(Unsupported)` 都已累加,`inject_folder_path` 不再累加
- **风险**:未来 `inject_via_uia` 新增其他返回 `Ok(Unsupported)` 的路径,会**漏累加**或**双累加**

**改进方案**(focus-21 v0.3.4 实施,降低未来 bug 风险):

`inject_via_uia` 改为**纯函数**(不修改 state),累加统一在 `inject_folder_path`:

```rust
// uia_inject.rs::inject_via_uia — 移除 state 参数 + 所有 inc_unsupported_dialog
fn inject_via_uia(dialog_hwnd: u32, target_path: &str, should_commit: bool) -> Result<DialogOutcome> {
    // ... 不再写 state.inc_unsupported_dialog() ...
    if let Some(ref edit) = best_edit {
        // ... write ...
        Ok(DialogOutcome::AddressBarWritten)
    } else {
        // 不累加,只是返回 Unsupported
        Ok(DialogOutcome::Unsupported)
    }
}

// uia_inject.rs::inject_folder_path — 集中累加
match inject_via_uia(dialog_hwnd, target_path, should_commit) {
    Ok(DialogOutcome::AddressBarWritten) => DialogOutcome::AddressBarWritten,
    Ok(DialogOutcome::Unsupported) => {
        state.inc_unsupported_dialog();  // ← 集中累加
        DialogOutcome::Unsupported
    }
    Ok(DialogOutcome::ComSetFolder) => DialogOutcome::ComSetFolder,
    Ok(DialogOutcome::Invalid) => DialogOutcome::Invalid,
    Err(err) => {
        warn!("UIA fallback failed: {err}; treating as unsupported");
        state.inc_unsupported_dialog();  // ← 集中累加
        DialogOutcome::Unsupported
    }
}
```

**验证判据**:
- 单元测试覆盖:`inject_via_uia` 不再写 state
- `inject_folder_path` match 分支全覆盖
- cargo test 全 PASS

**优先级**:🟢 P3,降低未来 bug 风险(focus-21 v0.3.4 可选)

---

### 10.2 §3.2 [P3] Edge tip notification duration 太长

**位置**:`extension/dist/index.js` 行 315(可能因后续编辑而不同)

**当前代码**(per review):
```js
sigma.ui.showNotification({...duration: 25000});
```

**问题**:25 秒太长,多数 notification 平台用 5-10 秒。

**改进方案**:
```js
sigma.ui.showNotification({...duration: 8000});  // 改 8 秒
```

**理由**:用户错过 → toolbar 按钮(Bug B 修复)兜底,不用 25s 占屏。

**优先级**:🟢 P3,UX 微改进(focus-21 v0.3.4 顺手改)

---

### 10.3 §3.3 [P3] inventory log 改 debug-level

**位置**:`uia_inject.rs::inject_via_uia` 行 359 + 行 392-395

**当前代码**(per review):
```rust
info!("=== Dialog UIA inventory (hwnd={dialog_hwnd}, total descendants={count}) ===");
for i in 0..count {
    info!("  ctrl={:?} automation_id='{}' name='{}' value='{}'", ...);
}
info!("=== End of dialog inventory ===");
```

**问题**:每次 dialog 触发都打 100+ 行 inventory log。Edge WinUI Save As 106 个 descendants → log 文件暴涨。

**改进方案**:
```rust
// 改为 debug-level。Production 用 `info,spike=debug` 时不显示。
debug!("=== Dialog UIA inventory (hwnd={dialog_hwnd}, total descendants={count}) ===");
for i in 0..count {
    debug!("  ctrl={:?} automation_id='{}' name='{}' value='{}'", ...);
}
debug!("=== End of dialog inventory ===");
```

**理由**:`info` 是默认 level,用户日常看 log 会看到 inventory 噪音。`debug` 让默认配置安静,需要时 `RUST_LOG=debug` 启用。

**优先级**:🟢 P3,log 文件大小优化(focus-21 v0.3.4 顺手改)

---

### 10.4 §3.4 [INFO] 预存限制(focus-22+ 清理)

| # | 限制 | 来源 |
|---|---|---|
| B-1 | UIA elements 线程亲和性(主线程 vs HTTP thread) | 已知 — `thread_local` COM,Bug A 修复会缓解 |
| B-2 | `committed_hwnds` hwnd 复用冲突 | 已知 — hwnd 可能跨 dialog session 复用 |
| B-3 | Unicode / RTL path 处理 | 已知 — BSTR 内部处理,但未 user-verify |
| B-4 | Sigma FM normalize 覆盖 v0.3.0 patch | 已知 — round 19c 风险 |

**focus-21 v0.3.4 不处理**,focus-22+ 一并清理。

---

## 11. 风险矩阵(rev.3 扩充,采纳 Mavis §4)

| # | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| R1-R25 | (前几轮已识别) | — | — | (已记录) |
| **R26** | HTTP server 线程 COM 未初始化(预存) | 中 | UIA 调用失效 | **Bug A 修复** |
| **R27** | 用户错过 Edge tip,flag 永久性 | 中 | Edge flag 教育失败 | **Bug B 修复(toolbar 按钮)** |
| **R28** | 初次激活双 notification UX 混乱 | 中 | 首次体验差 | **Bug C 修复(延迟 initial ping)** |
| **R29** | `unsupported_dialog_count` 累加路径未来误改 | 低 | 计数不准 | 重构累加逻辑(§10.1) |
| **R30** | inventory log 文件增长 | 中 | disk 占满 | 改 debug-level(§10.3) |
| **R31** | COM 隐式初始化跨 Windows 版本不一致 | 中 | 部分用户 UIA 失效 | **Bug A 修复是彻底方案** |

---

## 12. focus-21 v0.3.4 完整实施 plan(Mavis §6 整合)

### 12.1 短期必修(2-3 h)

| # | 任务 | 文件 | 工作量 |
|---|---|---|---|
| **A** | **修 Bug A — HTTP server 线程 CoInitializeEx** | `sigma-listary-spike/src/main.rs` | 30 min |
| **B** | **修 Bug B — Edge tip toolbar 按钮 + resetEdgeTip 函数** | `release/extension/dist/index.js` | 1 h |
| **C** | **修 Bug C — 延迟 initial ping 5s** | `release/extension/dist/index.js` | 15 min |
| D | Edge tip duration 25s → 8s | `release/extension/dist/index.js` | 5 min |
| E | inventory log 改 debug-level | `sigma-listary-spike/src/uia_inject.rs` | 15 min |

**总计**:**2-2.5 h**

### 12.2 短期建议(1-2 h)

| # | 任务 | 文件 | 工作量 |
|---|---|---|---|
| F | 累加重构(`inject_via_uia` 改纯函数) | `sigma-listary-spike/src/uia_inject.rs` | 30 min |
| G | README 加 Edge Modern Save As 章节 | `sigma-listary-spike/README.md`(新建) | 1 h |

**总计**:**1.5 h**

### 12.3 集成测试(1 h)

| # | 任务 | 工作量 |
|---|---|---|
| H | cargo test 全 PASS(含 Bug A/B 回归测试) | 30 min |
| I | cargo build --release + Tauri NSIS build | 30 min |

### 12.4 User-verify(必做,30 min)

| # | 任务 |
|---|---|
| **J** | **Edge WinUI filename + Enter 行为测试**(3 种格式:`C:\DrvPath\` / `C:\DrvPath` / `C:\DrvPath\file.exe`) |
| K | Edge legacy flag 启用 + 重启 + 验证 Save As dialog class |

### 12.5 总计

| 类型 | 工作量 |
|---|---|
| focus-21 v0.3.4 代码改动(A-G) | **3.5-4 h** |
| 集成测试(H + I) | 1 h |
| User-verify(J + K) | **30 min(用户)** |
| **总计** | **4.5-5 h + 30 min user** |

### 12.6 focus-21 v0.3.4 ship 产物

- `sigma-listary-spike/target/release/spike.exe`
- `release/extension/bin/focus-sync-sidecar.exe`
- `release/extension-installer/focus-sync-sidecar.exe`
- `release/Sigma-File-Manager-2.2.0-focus-21-setup.exe`
- `sigma-listary-spike/README.md`(新增 Edge Modern Save As 章节)
- SPIKE_VERSION 0.3.3 → **0.3.4**
- Extension 版本号 v0.3.3 → **v0.3.4**

### 12.7 focus-21 v0.3.4 决策树(基于 user-verify)

```
User verifies Edge WinUI filename + Enter behavior (J + K)
├── 如果 "navigation works"(J + K 确认)
│   ├── focus-22 实施 Option D(写 path + Enter commit)
│   └── 用户仍需启用 Edge legacy flag 才能 100% work
│
└── 如果 "navigation 不 work"(J + K 否定,预期大概率)
    ├── 选项 A: Edge legacy flag + focus-21 已 ship 通知升级
    │   └── focus-22 实施 README + Edge flag 完整支持
    │
    └── 选项 E: 接受现状 + 文档化
        └── focus-22 写完整 README(focus-21 v0.3.4 已加章节)
        └── 推荐作为 default fallback
```

---

## 13. focus-21 决策清单(沿用 rev.2 §10)

**3 件事必做**:

### 13.1 User-verify(J + K,30 min,必做)

- [ ] Edge WinUI Save As filename 框手动输入 `C:\DrvPath\` → Enter,看是否导航
- [ ] 测试 3 种格式:`C:\DrvPath\` / `C:\DrvPath` / `C:\DrvPath\file.exe`
- [ ] 启用 Edge legacy flag,重启 Edge,看 Save As dialog class

### 13.2 Bug 修复优先级(无需用户决策,全部必修)

- [ ] **🔴 HIGH Bug A — HTTP server CoInitializeEx**(必修)
- [ ] **🟡 MEDIUM Bug B — Edge tip toolbar 按钮**(必修)
- [ ] **🟡 MEDIUM Bug C — 延迟 initial ping 5s**(必修)

### 13.3 P3 改进(可选)

- [ ] inventory log 改 debug-level(顺手改)
- [ ] Edge tip duration 25s → 8s(顺手改)
- [ ] 累加重构(可选)
- [ ] README 加 Edge 章节(必修)

---

## 14. 教训(沿用 rev.2 + 新增源码层教训)

### 14.1 我自己的认知错误(沿用 rev.2)

1. 假设性推理代替 user-verify
2. 看 user message 但不解读诉求
3. 每次 fix 在同一条思路下微调
4. 没有质疑上游 PathWrap 的假设
5. retrospective #1 §3.1 又犯"假设预期"错

### 14.2 源码层新错误(Mavis 第 6 轮指出)

6. **5 轮 review 全在文档/计划层,没人查实际代码**
7. **Bug A (HTTP thread CoInitializeEx) 是 focus-17 引入的预存 bug**,5 轮 review 全没发现
8. **Bug B (Edge tip flag 永久性) 是我设计的通知策略缺陷**,用户错过永远看不到
9. **Bug C (双 notification UX) 是 activate() 函数并发问题**,Mavis review 才暴露

### 14.3 给后续会话的硬规则(rev.2 §11.2 + 新增)

**规则 1-5**: 沿用 rev.2

**规则 6**(rev.3 新增):**源码 bug 检查应在每轮 review 中**。5 轮 review 都只看 retrospective 文档,**没人检查实际代码**导致 Bug A 延迟 5 轮才发现。**review checklist 必须包含"打开 main.rs / index.js 看实际代码"**。

**规则 7**(rev.3 新增):**threading + COM 是 Rust Windows 编程的雷区**。任何后续涉及 threading + COM 的 Rust 项目都应警惕线程独立的 COM 初始化。

**规则 8**(rev.3 新增):**一次性 toast notification 不能依赖用户记忆**。重要提示必须有永久入口(toolbar button / settings UI / README)。

---

## 15. 文档链接(完整链路)

**本文档(retrospective #3 rev.3,final)**:
`F:\soft\00selfmade\filemanager\docs\superpowers\plans\2026-10-02-focus-20-retrospective-architecture-failure-rev3.md`

**supersedes**:
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-architecture-failure-rev2.md`(retrospective #2)
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-architecture-failure.md`(retrospective #1)

**输入文档**:
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-rev2-review.md`(Mavis 第 6 轮 review,4 源码 Bug)
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-review.md`(Mavis 第 5 轮 review)
- `docs/superpowers/plans/2026-10-02-focus-19-retrospective-deep-dive.md`(retrospective #0)

**对照 plans**:
- `docs/superpowers/plans/2026-10-02-focus-20-edge-winui-fix.md`(focus-20 实施 plan)
- `docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-revised.md`(focus-19 rev.2)

**5 轮 handoffs**:
- focus-17/18/19/20 handoffs in `handoff-2026-10-*.md`

**截图证据**:
- 用户最新截图:`E:\Users\Duanyi\.claude\uploads\7b65d5b1-...-image.png`(已纳入 §1)
