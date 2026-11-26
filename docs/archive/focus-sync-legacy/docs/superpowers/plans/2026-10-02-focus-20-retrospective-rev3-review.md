# focus-20 Retrospective rev.3 — 第 7 轮 review(源码深度审查 + 用户验证阻断 + 实施风险)

**日期**: 2026-10-02
**作者**: Mavis (MiniMax-M3)
**输入**:
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-architecture-failure-rev3.md`(rev.3 final)
- 实际源码(本次深入逐行审查):
  - `sigma-listary-spike/src/main.rs`(462 行,已读全)
  - `sigma-listary-spike/src/uia_inject.rs`(695 行,已读全)
  - `sigma-listary-spike/src/state.rs`(297 行,已读全)
  - `sigma-listary-spike/src/writer.rs`(225 行,已读全)
  - `sigma-listary-spike/src/http_server.rs`(394 行,已读全)
  - `sigma-listary-spike/src/uia_event.rs`(526 行,已读全)
  - `sigma-listary-spike/src/fg_bypass.rs`(168 行,已读全)
  - `sigma-listary-spike/src/reader.rs`(60 行,已读全)
  - `sigma-listary-spike/src/lib.rs`(32 行)
  - `sigma-listary-spike/src/config.rs`(159 行)
  - `sigma-listary-spike/src/events.rs`(70 行)
  - `release/extension/dist/index.js`(362 行,已读全)
  - `sigma-listary-spike/Cargo.toml`
**性质**: 第 7 轮 review。本次**全文逐行审查源码**,重点检查 rev.3 final 是否还有遗漏的源码层 Bug / UX 缺陷 / 实施风险,以避免"源码修改后仍然无法达到目标"。
**supersedes**: 无(retrospective review 类,与前 6 轮并列。

---

## 0. TL;DR(60 秒摘要)

rev.3 是**迄今为止最完整的 retrospective**,整合了 5 轮架构失败 + 4 源码 Bug + focus-21 v0.3.4 plan。但**本文档(第 7 轮)在 rev.3 基础上进一步发现 9 处遗漏,其中 3 处 HIGH 风险可能导致 focus-21 实施后仍然无法达到目标**:

| # | 新发现 | 严重度 | 风险 |
|---|---|---|---|
| **N1** | **41477 分支 Invoke Save,WinUI fallback 分支不 Invoke** — 同 `should_commit` 下行为不一致 | 🔴 HIGH | 用户体验割裂:auto-Confirm 在 Chromium work,在 Edge WinUI 不 work,误以为"auto-Confirm 坏了" |
| **N2** | **Bug C 修复不完整** — 仅延迟 initial ping,未延迟 health check + tip + 整体启动 grace period | 🟡 MEDIUM | 启动 30s 内仍可能弹 2-3 条 notification,UX 仍然混乱 |
| **N3** | **`commit_via_uia` 41477 分支 + 后续 user-verify 直接相关,无 fallback 路径** — 假设 Edge legacy flag work | 🟡 MEDIUM | 若 user-verify 否定 + Edge legacy flag 无效,focus-21 实施后用户仍被困 |
| N4 | Bug A 修复无 RAII CoUninitialize | 🟢 LOW | 线程退出时 COM 句柄不释放 |
| N5 | Bug B `resetEdgeTip` 通知写"next activation"但需要重启 Sigma FM | 🟢 LOW | 误导用户以为立即生效 |
| N6 | Bug B flag 立即置 true,用户错过永远看不到 | 🟢 LOW | 提议改为延时置 true 或周期性重新显示 |
| N7 | Bug D `loadBool` 应该本轮一起做 | 🟢 LOW | <30 行,低风险,改善可读性 |
| N8 | P3 #1 累加重构有引入新 bug 风险 | 🟡 MEDIUM | 当前累加语义正确,refactor 可能破坏正确性 |
| N9 | 实施 plan 缺少 **源码再读 checkpoint** + **集成测试 for COM** | 🟡 MEDIUM | 5 轮 review 全在文档,实施时可能重蹈覆辙 |

**核心建议**(避免"修改后仍无法达到目标"):

1. **强制 user-verify 为 focus-21 实施的 BLOCKING gate**(不是只列在 §13.3,要在 §12.1 之前)
2. **修复 commit_via_uia 对称性**(N1)— 41477 / WinUI fallback 应一致
3. **添加 30 秒 boot grace period**(N2)— 一次性解决启动 notification 雪崩
4. **推迟 P3 #1 累加重构到 focus-22+**(N8)— 风险 > 收益
5. **在实施 plan 中插入"代码再读 checkpoint"**(N9)— 每完成一组 bug 修复,必须重读已修改文件

---

## 1. rev.3 final 文档的复核结论

rev.3 在以下方面做得**很好**(无需修改):

✅ **架构结论正确**:PathWrap 假设不适合 WinUI 3,5 轮焦点错位根因诊断(反复确认 lookup → green arrow vs red arrow)
✅ **Bug A 修复方向正确**:HTTP server 线程需要 `CoInitializeEx`(`thread_local` UIA instance 强制每线程 COM init)
✅ **Bug B 修复方向正确**:Edge tip flag 永久性问题 → toolbar 按钮重置
✅ **Bug C 修复方向正确**:初次激活 double notification → 延迟 initial ping
✅ **focus-21 决策树合理**:按 user-verify 结果分支,推荐 A+E 组合作为 default fallback
✅ **4 P3 改进方向正确**(累加重构 / duration / log / 预存限制)
✅ **文档结构清晰**(4 P3 + 4 Bug + 风险矩阵 + 实施 plan + 决策清单)

但 rev.3 在**实施细节**和**风险预防**方面仍有 9 处遗漏(见 §2-§10)。

---

## 2. 🔴 N1 [HIGH] — `commit_via_uia` 行为不一致(41477 vs WinUI fallback)

### 2.1 问题

**位置**:`sigma-listary-spike/src/uia_inject.rs`

第 433-447 行(41477 分支):
```rust
if let Some(bar) = address_bar {
    let value: IUIAutomationValuePattern =
        unsafe { bar.GetCurrentPatternAs(UIA_ValuePatternId)? };
    unsafe { value.SetValue(&BSTR::from(target_path))? };
    info!("Wrote FULL target to address bar (41477): '{target_path}'");

    // 41477 commit: SetValue then Invoke Save triggers Chromium dialog
    // navigation. Bypass wrapper is in writer.rs (Chromium foreground
    // steal protection).
    if should_commit {
        if let Err(e) = commit_via_uia(best_button.as_ref(), best_edit.as_ref()) {
            warn!("commit after 41477 SetValue failed: {e}");
        }
    }
    return Ok(DialogOutcome::AddressBarWritten);
}
```

第 472-490 行(WinUI fallback 分支):
```rust
if let Some(ref edit) = best_edit {
    unsafe {
        let value: IUIAutomationValuePattern =
            edit.GetCurrentPatternAs(UIA_ValuePatternId)?;
        value.SetValue(&BSTR::from(target_path))?;
    }
    info!(
        "focus-20: wrote FULL target='{target_path}' to filename edit \
         (WinUI/Edge Modern Save As fallback, restored from focus-18; \
         no auto-Enter per retrospective review §2.1/§2.2; user manually saves)."
    );
    // Note: NO commit_via_uia() — Enter/Invoke deferred to Option D
    // (Task #1 user-verify pending). User manually clicks Save.
    return Ok(DialogOutcome::AddressBarWritten);
}
```

**问题诊断**:
- 同一个 `should_commit` 变量下,41477 分支会 `commit_via_uia` (Invoke Save button),WinUI fallback 分支**不会**
- 用户开启 Auto-Confirm(扩展 toolbar 菜单切换 ON):
  - 打开 Chromium Save As → 自动 Invoke Save → 自动保存
  - 打开 Edge WinUI Save As → 仅写文件名 → 用户必须再点 Save
- **用户体验割裂**:用户以为"Auto-Confirm 坏了"或在某些 app 不工作,实际是设计如此

### 2.2 风险

🔴 **HIGH** — 这是用户最可能首先遇到的"看起来坏掉了"的场景:
1. 用户在 Chromium 测试 Auto-Confirm ON,成功
2. 用户在 Edge 测试 Auto-Confirm ON,**失败**(没有自动 Invoke Save)
3. 用户报告"Auto-Confirm 在 Edge 不工作"
5. Claude 重写 WinUI fallback 的 commit 逻辑(focus-23 又一轮修复)

如果不修,用户报告的失败模式就是"Auto-Confirm 时有时坏"。

### 2.3 修复方案(三种选一)

#### 方案 A:两个分支都不 Invoke,统一行为(focus-21 v0.3.4 实施)

```rust
// uia_inject.rs 41477 分支 (line 433-447): 移除 commit_via_uia 调用
if let Some(bar) = address_bar {
    let value: IUIAutomationValuePattern =
        unsafe { bar.GetCurrentPatternAs(UIA_ValuePatternId)? };
    unsafe { value.SetValue(&BSTR::from(target_path))? };
    info!("Wrote FULL target to address bar (41477): '{target_path}'");
    // focus-21: NO commit_via_uia here — symmetric with WinUI fallback
    // (per Mavis review N1 [HIGH]). User must click Save (or use toolbar's
    // new "Commit" command for explicit Invoke — focus-22+).
    return Ok(DialogOutcome::AddressBarWritten);
}
```

**优点**:行为对称,user-verify 不需要验证 Invoke 是否 work
**缺点**:Chromium 用户失去 Auto-Confirm 功能(部分)

#### 方案 B:两个分支都 Invoke,统一行为(focus-21 v0.3.4 实施)

```rust
// uia_inject.rs WinUI fallback 分支 (line 472-490): 添加 commit_via_uia 调用
if let Some(ref edit) = best_edit {
    unsafe {
        let value: IUIAutomationValuePattern =
            edit.GetCurrentPatternAs(UIA_ValuePatternId)?;
        value.SetValue(&BSTR::from(target_path))?;
    }
    info!("focus-21: wrote FULL target='{target_path}' to filename edit");
    // focus-21: also commit (symmetric with 41477 branch) when should_commit
    if should_commit {
        if let Err(e) = commit_via_uia(best_button.as_ref(), best_edit.as_ref()) {
            warn!("commit after fallback write failed: {e}");
        }
    }
    return Ok(DialogOutcome::AddressBarWritten);
}
```

**优点**:对称行为,Auto-Confirm 在所有支持的 dialog 都 work
**缺点**:需要 user-verify 确认 WinUI 在 Invoke 后会 commit Save / 导航

#### 方案 C:把 Auto-Confirm 设为显式双击(focus-22+)

UI 改造,toolbar 增加两个按钮:
- "Sync Now (write path only)" — 默认行为
- "Sync + Save (write + Invoke Save)" — 等价于 Auto-Confirm ON
- Auto-Confirm toggle 保留,但描述改为"deprecated, use Sync + Save"

**优点**:行为显式,不需要对称性
**缺点**:UI 复杂度增加,需要 user 重新适应

### 2.4 推荐

**方案 B + 强制 user-verify WinUI Invoke 行为**:

1. focus-21 v0.3.4 实施方案 B(两个分支都 Invoke)
2. user-verify 必做项 **新增**:
   - [ ] Edge WinUI Save As,Auto-Confirm ON,filename 框输入 `C:\DrvPath\`,验证 dialog 自动 commit Save / 导航
   - [ ] 如果失败,回退到方案 A(两个分支都不 Invoke)
3. 文档明确说明:`Auto-Confirm ON` 表示"sidecar 会在写入路径后自动调用 Invoke Save 按钮",无论 dialog 类型

### 2.5 用户验证阻断

**focus-21 v0.3.4 §12.4 user-verify 任务清单应增加**:

- [ ] **N1-verify-1**: Edge WinUI filename + Enter 行为测试(沿用 J) — 3 种格式:`C:\DrvPath\` / `C:\DrvPath` / `C:\DrvPath\file.exe`
- [ ] **N1-verify-2 (新增)**: Edge WinUI + Auto-Confirm ON → filename 框输入 `C:\DrvPath\` → 验证 Invoke Save 是否自动 commit / 导航
- [ ] **N1-verify-3 (新增)**: 如果 N1-verify-2 失败,改用方案 A(两个分支都不 Invoke),重新测试

---

## 3. 🟡 N2 [MEDIUM] — Bug C 修复不完整,启动 notification 仍可能雪崩

### 3.1 问题

**位置**:`release/extension/dist/index.js` 第 278-340 行(`activate()` 函数)

**当前 activate() 流程**:
```
t=0:    Edge tip shown
t=0:    initial pingSidecar() (Bug C 修复: delay 5s)
t=0:    startHealthCheck() → setInterval(tick, 15s) → first tick at t=15s
t=0:    registerDropdown / context / initialPath push
```

**rev.3 §9.3 Bug C 修复建议**(只改 initial ping):
```js
// 延迟 initial ping 5s — 给 sidecar 启动时间
setTimeout(() => { void pingSidecar()...; }, 5000);
```

**问题**:
- 启动后 30 秒内仍可能弹 2-3 条 notification:
  - **Edge tip**(t=0)
  - **Sidecar not reachable**(t=5s,如果 sidecar 还没 bind 37421)
  - **Unsupported dialog count changed**(t=15s 或 t=30s,如果 sidecar 期间累积了 unsupported dialog)
- 用户体验仍混乱

### 3.2 进一步诊断

**Sidecar 启动时序**(基于 v0.3.0 Scheduled Task mode + focus-20):
- 用户登录 Windows → Task Scheduler 触发 KizemoFocusSync(AtLogOn)
- Task Scheduler 启动 → `spike.exe --service` → 大约 1-3 秒后 `port 37421` bind OK
- 用户启动 Sigma FM → extension 激活 → activate()
- **如果用户在 <1 秒内从登录切换到 Sigma FM 启动,sidecar 可能还在 bind 阶段**

更糟的情况:
- 用户登录 Windows,关掉电脑,过 1 小时重启 Sigma FM(无重启 PC) → Task Scheduler 不重新触发 KizemoFocusSync → 但之前的 sidecar 还在跑
- 但是用户可能从休眠恢复 → sidecar 也还在跑
- 所以 sidecar-not-reachable 通常只在首次登录或重启后出现

### 3.3 修复方案:30 秒 boot grace period

```js
// index.js 第 278 行 activate() 开头
const activate = async () => {
    // ... 现有初始化 ...
    
    // focus-21 (Mavis review N2 [MEDIUM]): boot grace period 30s
    // During this period, all error/warning notifications are suppressed
    // except the Edge legacy file picker tip (which is critical info).
    // Reason: sidecar may take 5-15s to bind 37421 after Sigma FM launch,
    // and user may trigger dialog events before health check is baseline-stable.
    const BOOT_GRACE_MS = 30000;
    const bootStartedAt = performance.now();
    const inGracePeriod = () => performance.now() - bootStartedAt < BOOT_GRACE_MS;
    
    // Edge legacy tip — ALWAYS shown (critical, not gated by grace period)
    try {
        const tipShown = await sigma.storage.get('edgeLegacyFilePickerTipShown');
        if (tipShown !== true) {
            sigma.ui.showNotification({...Edge tip...});
            await sigma.storage.set('edgeLegacyFilePickerTipShown', true);
        }
    } catch (e) {}
    
    // Initial ping — delayed 5s, BUT notification gated by grace period
    setTimeout(() => {
        void pingSidecar().then((ok) => {
            if (!ok && !inGracePeriod() && !notifiedSinceLastFailure) {
                // notify
            }
        });
    }, 5000);
    
    startHealthCheck();
    
    // In checkHealth, gate unsupported_dialog_count notification:
    //   if (newCount > lastUnsupportedDialogCount && !inGracePeriod())
    // (Existing 5-min throttle already gates)
};
```

**优点**:统一处理,启动 noise 降到 1 条(仅 Edge tip)
**缺点**:如果服务真的挂了,30 秒内不会立刻看到提示(用户主动点 "Sync Now" 仍会看到)

### 3.4 推荐

**focus-21 v0.3.4 §12.1 任务 C 扩展**:

- [ ] **Bug C 修复 (扩展)**: 添加 30 秒 boot grace period
- 预计工作量:15 分钟(在现有 Bug C 修复上加 `inGracePeriod()` 检查)

---

## 4. 🟡 N3 [MEDIUM] — `commit_via_uia` 41477 路径假设 Edge legacy flag work,无 fallback

### 4.1 问题

**位置**:`sigma-listary-spike/src/uia_inject.rs` 第 433-447 行(41477 分支)+ `commit_via_uia()` 第 496-514 行

**41477 分支**:
```rust
if should_commit {
    if let Err(e) = commit_via_uia(best_button.as_ref(), best_edit.as_ref()) {
        warn!("commit after 41477 SetValue failed: {e}");
    }
}
```

**commit_via_uia**:
```rust
fn commit_via_uia(
    best_button: Option<&IUIAutomationElement>,
    best_edit: Option<&IUIAutomationElement>,
) -> Result<()> {
    if let Some(button) = best_button {
        let invoke: IUIAutomationInvokePattern = unsafe {
            button.GetCurrentPatternAs(UIA_InvokePatternId)?
        };
        unsafe { invoke.Invoke()? };
        info!("Committed via Invoke(Save button).");
        return Ok(());
    }
    if let Some(edit) = best_edit {
        commit_via_fallback_enter(edit);
        return Ok(());
    }
    warn!("commit_via_uia: no button and no edit; commit skipped");
    Ok(())
}
```

**问题诊断**:
- `commit_via_uia` 用 `Invoke(Save button)` 或 `SendMessageW Enter` to filename edit
- **Invoke Save button** 在 Chromium 41477 dialog 中 work(Chromium Save As 的默认按钮是 IDOK = "Save")
- **Invoke Save button** 在 Edge WinUI dialog 中**可能不 work**(WinUI Save button 可能不响应 UIA Invoke)
- **SendMessageW Enter** 在 Chromium 41477 dialog 中**可能不 work**(41477 不是标准 Windows edit control,SendMessageW 可能被 Chromium 屏蔽)
- **SendMessageW Enter** 在 Edge WinUI dialog 中**可能 work**(WinUI filename box 可能接受 Enter)

**风险**:
- user-verify 之前,不知道哪个组合 work
- focus-21 v0.3.4 实施后,Invoke Save 可能 success log 但 dialog 没有 commit(user 看到的"成功"是假的)

### 4.2 进一步诊断

**问题延伸**:即使 Invoke 返回 success,WinUI dialog 内部状态可能没刷新。`commit_via_uia` 没有 verification step。

```rust
unsafe { invoke.Invoke()? };  // 返回 Ok 不代表 dialog 已 commit
info!("Committed via Invoke(Save button).");  // 乐观日志
```

**应改**:
```rust
unsafe { invoke.Invoke()? };
// 短暂 sleep,让 UI 处理事件
std::thread::sleep(std::time::Duration::from_millis(50));
// 验证:再次读 filename box,看是否已 commit
// (但这增加了 UIA 调用次数,加 focus-50 path 的 read_path)
// 或者:用 IsWindow 检查 dialog 是否还存在
let still_open = unsafe {
    use windows::Win32::UI::WindowsAndMessaging::IsWindow;
    IsWindow(dialog_hwnd).as_bool()
};
if !still_open {
    info!("Dialog closed — Invoke committed.");
} else {
    warn!("Dialog still open after Invoke — commit may have failed");
}
```

但这是 focus-22+ 的优化,不在 focus-21 范围。

### 4.3 修复方案

**focus-21 v0.3.4 §12.1 任务 B 扩展**(在方案 B 的对称性修复基础上):

- 41477 分支 + WinUI fallback 分支都 Invoke(`commit_via_uia`)
- `commit_via_uia` 内部:
  - 优先 Invoke Save button
  - fallback to SendMessageW Enter
  - **每次 Invoke 后增加 verification sleep(100ms)+ IsWindow 检查**
- user-verify 增加 N1-verify-2(N1 §2.4)
- 如果 N1-verify-2 失败,**回退到方案 A**(两个分支都不 Invoke)

### 4.4 推荐

**决策树扩展**(在 rev.3 §12.7 基础上):

```
User verifies Edge WinUI filename + Enter behavior (J + K + N1-verify-2)
├── 如果 "J + K + N1-verify-2 全 positive"
│   └── focus-22 实施 option D (path + Enter commit, 41477 Invoke Save WinUI fallback Invoke)
│
└── 如果 "J 或 K 或 N1-verify-2 任意 negative"
    ├── 选项 A: 方案 A(都不 Invoke,仅 Write path) + Edge legacy flag 已 ship 通知升级
    │   └── focus-22 实施 README + Edge flag 完整支持
    │
    └── 选项 E: 接受现状 + 文档化
        └── focus-22 写完整 README(focus-21 v0.3.4 已加章节)
        └── 推荐作为 default fallback
```

---

## 5. 🟢 N4 [LOW] — Bug A 修复无 RAII CoUninitialize

### 5.1 问题

**位置**:`sigma-listary-spike/src/main.rs` 第 240-247 行 + rev.3 §9.1 修复代码

**rev.3 修复代码**:
```rust
let _http_handle = std::thread::Builder::new()
    .name("spike-http".into())
    .spawn(move || {
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

**问题**:
- `CoInitializeEx` 调用后没有 `CoUninitialize`
- HTTP server 线程常驻(`/quit` 触发 `std::process::exit(0)`),理论上不会正常退出
- 但如果 HTTP server panic / 异常,COM 句柄不释放
- 实际影响:线程常驻 → COM 不释放 → 正常流程下问题不大

### 5.2 修复方案

```rust
let _http_handle = std::thread::Builder::new()
    .name("spike-http".into())
    .spawn(move || {
        // RAII guard: CoInitialize on construction, CoUninitialize on Drop
        struct ComGuard;
        impl Drop for ComGuard {
            fn drop(&mut self) {
                unsafe {
                    let _ = windows::Win32::System::Com::CoUninitialize();
                }
            }
        }
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            );
        }
        let _guard = ComGuard;  // dropped when closure returns
        
        if let Err(e) = http_server::run_server(state_for_http, port, mode) {
            tracing::warn!(error = %e, "http server exited");
        }
    })
    .expect("spawn http server thread");
```

**优先级**:🟢 LOW,实际不常触发
**focus-21 v0.3.4 是否做**:可选(顺手改 <30 行)

---

## 6. 🟢 N5-N7 [LOW] — 通知 / Bug D 改进

### 6.1 N5: Bug B `resetEdgeTip` 通知文案误导

**位置**:`release/extension/dist/index.js` + rev.3 §9.2 修复代码

**rev.3 修复代码**:
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
```

**问题**:用户点 "🔁 Re-show Edge tip" → 看到 "next activation" → 但 `activate()` 只在 Sigma FM 重启 / 扩展重载时运行,不是每次都运行

**修复**:
```js
async function resetEdgeTip() {
    try {
        await sigma.storage.set('edgeLegacyFilePickerTipShown', false);
        // IMMEDIATELY re-show the tip (don't wait for next activation)
        sigma.ui.showNotification({
            title: 'Focus Sync — Edge Save As tip',
            description: '...full tip content...',
            type: 'info',
            duration: 12000,  // 12s for re-show (slightly longer than default 8s)
        });
        // Re-set flag (so next activate() doesn't double-show)
        await sigma.storage.set('edgeLegacyFilePickerTipShown', true);
    } catch (e) {}
}
```

**优点**:用户立即看到 tip,不需要重启 Sigma FM

### 6.2 N6: Bug B flag 设置时机不合理

**位置**:`release/extension/dist/index.js` 第 298-318 行

**当前代码**:
```js
if (tipShown !== true) {
    sigma.ui.showNotification({...Edge tip...});  // 弹通知
    await sigma.storage.set('edgeLegacyFilePickerTipShown', true);  // 立刻置 true
}
```

**问题**:如果用户在 t=0 弹通知时不在屏幕前),过了 25 秒后通知自动消失,flag 已经是 true → 永远不再弹

**修复方案 A**:延迟置 flag
```js
if (tipShown !== true) {
    sigma.ui.showNotification({...Edge tip...});
    // Delay setting flag to 30s — give user time to read
    setTimeout(async () => {
        try {
            await sigma.storage.set('edgeLegacyFilePickerTipShown', true);
        } catch (e) {}
    }, 30000);
}
```

**修复方案 B**:周期性重新提示(更激进)
```js
// 在 startHealthCheck 之外,新增一个 setTimeout(24h)循环
// 如果 flag 仍为 true 且用户没启用 Edge legacy flag(无法检测),重新弹 tip
// 但这增加了复杂度,且无法验证 Edge legacy flag 状态
```

**推荐**:方案 A(30s 延迟置 flag)+ Bug B toolbar 按钮(N5 修复)。组合后,即使 Bug A + Bug B 都没触发,toolbar 按钮是终极兜底。

### 6.3 N7: Bug D `loadBool` 应该本轮一起做

**位置**:`release/extension/dist/index.js` 第 281-282 行 + rev.3 §9.4

**当前代码**:
```js
try { enabled = (await sigma.storage.get('enabled')) ?? true; } catch (e) { enabled = true; }
try { autoConfirm = (await sigma.storage.get('autoConfirm')) ?? false; } catch (e) { autoConfirm = false; }
```

**问题**:
- 2 行重复 try/catch + default
- Bug B 修复后还需要再加一行 `edgeLegacyFilePickerTipShown`(增加第 3 个)
- 第 4 个 `inGracePeriod` 不需要 storage(纯运行时)
- 5 个 boolean 配置 + try/catch,可读性差

**修复**:
```js
async function loadBool(key, defaultValue) {
    try {
        return (await sigma.storage.get(key)) ?? defaultValue;
    } catch (e) {
        return defaultValue;
    }
}

// Usage:
const enabled = await loadBool('enabled', true);
const autoConfirm = await loadBool('autoConfirm', false);
const tipShown = await loadBool('edgeLegacyFilePickerTipShown', false);
```

**优点**:Bug B 修复需要加新 storage flag,顺便统一所有 storage 读取模式
**工作量**:5 分钟

**推荐**:focus-21 v0.3.4 必修(<5 分钟,可读性 + Bug B 修复需要)

---

## 7. 🟡 N8 [MEDIUM] — P3 #1 累加重构有引入新 bug 风险

### 7.1 问题

**位置**:`sigma-listary-spike/src/uia_inject.rs` + rev.3 §10.1

**rev.3 §10.1 重构方案**:
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

**当前代码**(已在源码中,正确):
```rust
// inject_via_uia 内部确实 inc_unsupported_dialog (line 488):
} else {
    warn!("no Edit element in dialog; nothing written via UIA");
    state.inc_unsupported_dialog();
    return Ok(DialogOutcome::Unsupported);
}
```

**风险分析**:
- 当前代码是正确的:**只 inc 一次**(内部 "no edit element" 分支)
- `inject_folder_path` 的 `Ok(Unsupported)` 分支**不 inc**(避免双计数)
- `inject_folder_path` 的 `Err(err)` 分支**才 inc**(处理 UIA 异常,不能糊弄)

**rev.3 §10.1 重构后**:
- `inject_via_uia` 不再 `state.inc_unsupported_dialog()`,改为纯函数
- `inject_folder_path` 集中 inc 在 `Ok(Unsupported)` + `Err(err)` 分支

**问题**:
- 重构前后**累加语义不变**(都 inc 一次)
- 重构后**代码更易理解**(集中在一处)
- **但是**:重构可能引入新 bug 的途径:
  - 把 `state.inc_unsupported_dialog()` 移到 `inject_folder_path` 时,如果忘记 `Err(err)` 分支也累加,会漏计
  - 如果未来加新的 `Ok(DialogOutcome)` variant,可能忘记在 `inject_folder_path` 累加
  - 测试覆盖如果不全,会逃过 cargo test

### 7.2 修复方案

**推荐**:focus-21 v0.3.4 **不做** P3 #1 重构

**理由**:
- 当前代码工作正常(cargo test 通过)
- 风险 > 收益(代码易读性 +10%,bug 风险 ×2)
- 推迟到 focus-22+,先做低风险改动(Bug A/B/D)

**如果你坚持重构**,必须满足:
1. 单元测试覆盖 `inject_via_uia` 各种返回路径(`AddressBarWritten` / `Unsupported` / `Err`)
2. 单元测试覆盖 `inject_folder_path` 所有 match 分支
3. integration test 验证 `unsupported_dialog_count` 在不同 dialog 下的累加

---

## 8. 🟡 N9 [MEDIUM] — 实施 plan 缺少源码再读 checkpoint + 集成测试 for COM

### 8.1 问题

**位置**:`rev.3 §12.1-§12.5`(focus-21 v0.3.4 实施 plan)

**当前 plan 结构**:
- §12.1 短期必修(2-3 h):A/B/C/D/E 5 个任务
- §12.2 短期建议(1-2 h):F/G 2 个任务
- §12.3 集成测试(1 h):H/I 2 个任务(H=cargo test, I=cargo build + Tauri NSIS build)
- §12.4 User-verify(30 min):J/K 2 个任务

**问题**:
1. **H=cargo test 全 PASS** 不能验证 Bug A 修复(单元测试不调用实际 COM)
2. **没有"代码再读"checkpoint** — 5 轮 review 全在文档层的教训没被吸收
3. **没有"重新分析 inventory log"的 checkpoint** — Bug A 修复后,需要重新看 inventory log 确认 UIA 仍然 enumerate 所有 descendants
4. **没有"多 Windows 版本测试"** — rev.3 §9.1 验证判据说"在多种 Windows 版本测试(22H2/23H2/24H2)",但 plan 没强制

### 8.2 修复方案:添加 checkpoint + integration test

**focus-21 v0.3.4 §12.1 修改**:

```markdown
### 12.1 短期必修(2.5-3.5 h)

| # | 任务 | 文件 | 工作量 | **CHECKPOINT(必须)** |
|---|---|---|---|---|
| **A** | 修 Bug A — HTTP server 线程 CoInitializeEx (+ RAII ComGuard,N4) | `src/main.rs` | 30 min | 重新读 `main.rs` 第 230-260 行,确认所有 spawn 的 std::thread 都有正确的 COM init |
| **A-2** | 修 N1/N3 — `commit_via_uia` 对称性(方案 B) | `src/uia_inject.rs` | 30 min | 重新读 `inject_via_uia` + `commit_via_uia`,确认 41477 / WinUI fallback 行为一致 |
| **B** | 修 Bug B — Edge tip toolbar 按钮 + resetEdgeTip(N5 修复立即重显) | `extension/dist/index.js` | 1 h | 重新读 `activate()` + `registerDropdown` + `registerCommand`,确认 4 个 command 都注册 |
| **B-2** | 修 N7 — `loadBool` helper | `extension/dist/index.js` | 5 min | grep `sigma.storage.get` 确认全部用 loadBool |
| **C** | 修 Bug C — 延迟 initial ping 5s + N2 boot grace period 30s | `extension/dist/index.js` | 30 min | 重新读 `activate()` + `checkHealth`,确认 grace period 在所有路径上 |
| **D** | Edge tip duration 25s → 8s | `extension/dist/index.js` | 5 min | 重新读 `maybeShowEdgeLegacyTip` |
| **E** | inventory log 改 debug-level | `src/uia_inject.rs` | 15 min | 重新读 `inject_via_uia` 第 359-395 行 |

**总计**:**2.5-3 h**
```

**新增 §12.2.5 Integration test for COM**:

```markdown
| **I-COM-1** | 实际运行 sidecar,Edge WinUI Save As,观察 /health 端点返回 ok(degraded or not) | 10 min |
| **I-COM-2** | 实际运行 sidecar,Edge Legacy Save As (COM SetFolder 路径),观察日志看到 `SetFolder via COM succeeded` | 10 min |
| **I-COM-3** | 实际运行 sidecar,Chromium Save As (41477 路径),观察日志看到 `Wrote FULL target to address bar (41477)` + `Committed via Invoke(Save button)` | 10 min |
| **I-COM-4** | 故意 kill sidecar (taskkill /F /IM spike.exe),重启,观察 `port bound` 日志 | 5 min |
```

**新增 §12.5.5 Multi-Windows-version test**(可选,需要 VM):

```markdown
| **M-WIN-1** | Windows 22H2 VM 上运行 sidecar,所有 dialog 路径测试 | 1 h(可选) |
| **M-WIN-2** | Windows 23H2 VM 上运行 sidecar | 1 h(可选) |
| **M-WIN-3** | Windows 24H2 VM 上运行 sidecar | 1 h(可选) |
```

### 8.3 推荐

- **focus-21 v0.3.4 必修**:每个 task 后加 "重新读代码" checkpoint
- **focus-21 v0.3.4 必修**:I-COM-1/2/3 integration test(实际运行 sidecar + 观察日志)
- **focus-21 v0.3.4 可选**:M-WIN-1/2/3 multi-version test(需要 VM,非必需)

---

## 9. 📋 第 7 轮发现的全部 9 项汇总

| # | 类别 | 风险 | focus-21 v0.3.4 是否必修 | 工作量 |
|---|---|---|---|---|
| N1 | `commit_via_uia` 不一致 | 🔴 HIGH | **必修**(方案 B + user-verify) | 30 min |
| N2 | Bug C 不完整) | 🟡 MEDIUM | **必修**(30s boot grace period) | 15 min |
| N3 | `commit_via_uia` 验证 + 决策树 | 🟡 MEDIUM | **必修**(扩展 §12.7 决策树) | 0(决策树更新) |
| N4 | Bug A 无 RAII | 🟢 LOW | 可选(顺手改) | 5 min |
| N5 | Bug B 通知文案 | 🟢 LOW | **必修**(重显 tip) | 5 min |
| N6 | Bug B flag 设置时机 | 🟢 LOW | 可选(30s 延迟) | 5 min |
| N7 | Bug D `loadBool` | 🟢 LOW | **必修**(与 Bug B 一起) | 5 min |
| N8 | **P3 #1 重构风险** | 🟡 MEDIUM | **不做**(推迟 focus-22+) | 0 |
| N9 | 实施 plan 缺 checkpoint + I-COM | 🟡 MEDIUM | **必修**(添加 checkpoint + test) | 30 min plan 更新 |

**focus-21 v0.3.4 必修工作量**:**3.5-4 h 代码 + 1 h integration test + 30 min user-verify**
**总工作量**(vs rev.3 §12.5 的 4.5-5 h):**5-5.5 h + 30 min user-verify**

---

## 10. 修订后的 focus-21 v0.3.4 完整实施 plan

### 10.1 短期必修(3-4 h)

| # | 任务 | 文件 | 工作量 |
|---|---|---|---|
| **A** | 修 Bug A — HTTP server 线程 CoInitializeEx + RAII ComGuard(N4) | `src/main.rs` | 30 min |
| **A-2** | 修 N1/N3 — `commit_via_uia` 对称性(方案 B,41477 + WinUI 都 Invoke) | `src/uia_inject.rs` | 30 min |
| **B** | 修 Bug B + N5 — Edge tip toolbar button + resetEdgeTip 立即重显 | `extension/dist/index.js` | 1 h |
| **B-2** | 修 N7 — `loadBool` helper + Bug D 整合 | `extension/dist/index.js` | 10 min |
| **C** | 修 Bug C + N2 — 延迟 initial ping 5s + 30s boot grace period | `extension/dist/index.js` | 30 min |
| **D** | Edge tip duration 25s → 8s | `extension/dist/index.js` | 5 min |
| **E** | inventory log 改 debug-level | `src/uia_inject.rs` | 15 min |

**总计**:**3-3.5 h**

### 10.2 短期建议(0.5 h)

| # | 任务 | 文件 | 工作量 |
|---|---|---|---|
| F | README 加 Edge Modern Save As 章节 | `sigma-listary-spike/README.md`(新建) | 30 min |
| G | N6: Bug B flag 30s 延迟置 true | `extension/dist/index.js` | 5 min |

**总计**:**0.5 h**

### 10.3 集成测试(1 h)

| # | 任务 | 工作量 |
|---|---|---|
| H | cargo test 全 PASS + **重新读 `main.rs` / `uia_inject.rs` / `index.js`** | 30 min |
| I-COM | I-COM-1/2/3 实际运行 sidecar 测试(Edge WinUI / Edge Legacy / Chromium) | 30 min |
| I | cargo build --release + Tauri NSIS build | 30 min |

### 10.4 User-verify(必做,扩展,30-45 min)

| # | 任务 |
|---|---|
| **J** | Edge WinUI filename + Enter 行为测试(3 种格式:`C:\DrvPath\` / `C:\DrvPath` / `C:\DrvPath\file.exe`) |
| **K** | Edge legacy flag 启用 + 重启 + 验证 Save As dialog class |
| **N1-verify-2** | Edge WinUI + Auto-Confirm ON → filename 框输入 `C:\DrvPath\` → 验证 Invoke Save 是否自动 commit / 导航 |
| **N1-verify-3** | 如果 N1-verify-2 失败,改用方案 A(两个分支都不 Invoke),重新测试 |

### 10.5 总计

| 类型 | 工作量 |
|---|---|
| focus-21 v0.3.4 代码改动(A-G) | **3.5-4 h** |
| 集成测试(H + I-COM + I) | **1.5 h** |
| User-verify(J + K + N1-verify) | **30-45 min(用户)** |
| **总计** | **5-5.5 h + 30-45 min user** |

### 10.6 ship 产物(沿用 rev.3 §12.6)

无变化

### 10.7 focus-21 v0.3.4 决策树(扩展 rev.3 §12.7)

```
User verifies Edge WinUI filename + Enter behavior (J + K + N1-verify-2)
├── 如果 "J + K + N1-verify-2 全 positive"
│   ├── focus-22 实施 A+E 组合(已 ship 通知升级 + 文档化)
│   └── 用户可继续使用 focus-21 v0.3.4
│
└── 如果 "J 或 K 或 N1-verify-2 任意 negative"
    ├── 选项 A: 实施 N1 方案 A(两个分支都不 Invoke,仅 Write path)
    │   └── 修复 `commit_via_uia` 不一致性,聚焦"写入即导航"
    │
    └── 选项 E: 接受现状 + 文档化(default)
        └── focus-22 写完整 README(focus-21 v0.3.4 已加章节)
        └── 用户需启用 Edge legacy flag(K)才能 100% work
```

---

## 11. 风险矩阵(扩展 rev.3 §11)

| # | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| R1-R31 | (前几轮已识别) | — | — | (已记录) |
| **R32** | **commit_via_uia 行为不一致(41477 vs WinUI)** | 高 | Auto-Confirm 看起来时好时坏 | **N1 修复(方案 B)** + user-verify |
| **R33** | **启动 30s 内 notification 雪崩** | 中 | 用户首次体验混乱 | **N2 修复(30s grace)** |
| **R34** | **Invoke Save 返回 Ok 但 dialog 未 commit** | 中 | user-verify 不准 | N3 验证 IsWindow(可选) |
| **R35** | **Cargo test 不验证 COM 行为** | 高 | Bug A 修复无效 | **I-COM integration test** |
| **R36** | **5 轮 review 不查源码的盲点** | 高 | 漏掉其他源码 bug | **N9 强制 code-read checkpoint** |
| R37 | COM 句柄不释放(线程退出时) | 低 | 资源泄漏 | N4 RAII(可选) |
| R38 | Bug B flag 永久 | Timed | 用户错过 tip | N5/N6 修复 |

---

## 12. 决策清单(扩展 rev.3 §13)

**4 件事必做**:

### 12.1 User-verify(J + K + N1-verify,45 min,必做,BLOCKING)

- [ ] Edge WinUI Save As filename 框手动输入 `C:\DrvPath\` → Enter,看是否导航
- [ ] 测试 3 种格式:`C:\DrvPath\` / `C:\DrvPath` / `C:\DrvPath\file.exe`
- [ ] 启用 Edge legacy flag,重启 Edge,看 Save As dialog class
- [ ] **N1-verify-2**: Edge WinUI + Auto-Confirm ON → 验证 Invoke Save 自动 commit
- [ ] **N1-verify-3**: 如果 N1-verify-2 失败,改方案 A 重测

### 12.2 Bug 修复优先级(无需用户决策,全部必修)

- [ ] **🔴 HIGH Bug A — HTTP server CoInitializeEx + RAII(N4)**
- [ ] **🔴 HIGH N1 — commit_via_uia 对称性(方案 B)**
- [ ] **🟡 MEDIUM Bug B + N5 — Edge tip toolbar + resetEdgeTip 立即重显**
- [ ] **🟡 MEDIUM Bug C + N2 — 延迟 initial ping + 30s boot grace**
- [ ] **🟢 LOW N6 — Bug B flag 30s 延迟置 true(可选)**
- [ ] **🟢 LOW N7 — loadBool helper**

### 12.3 P3 改进(可选)

- [ ] inventory log 改 debug-level(必修,顺手)
- [ ] Edge tip duration 25s → 8s(必修,顺手)
- [ ] ~~累加重构(N8 推迟 focus-22+)~~
- [ ] README 加 Edge 章节(必修)

### 12.4 实施 plan 加固(N9)

- [ ] 每个 task 后加"重新读代码"checkpoint
- [ ] 添加 I-COM-1/2/3 integration test
- [ ] 添加 user-verify N1-verify 任务

---

## 13. 教训(扩展 rev.3 §14)

### 13.1 沿用 rev.3

1. 假设性推理代替 user-verify
2. 看 user message 但不解读诉求
3. 每次 fix 在同一条思路下微调
4. 没有质疑上游 PathWrap 的假设
5. retrospective #1 §3.1 又犯"假设预期"错

### 13.2 第 7 轮新增(Mavis)

10. **commit_via_uia 不一致是典型的"分支增加而忘记对齐"** — 41477 是 focus-18 加的,WinUI fallback 是 focus-20 加的,两个分支独立实现,没有 review 对称性
11. **Bug C 修复不完整是典型的"只修表面不复查其他触发点"** — 修了 initial ping,没复查 health check + 整体启动 grace period
12. **Cargo test 通过 ≠ 实施成功** — 单元测试不调用实际 COM,Bug A 修复后必须实际运行 sidecar 验证
13. **5 轮 review 不查源码的盲点要靠"代码再读 checkpoint"强制** — 写文档容易,看代码难,但代码才是真相

### 13.3 给后续会话的硬规则(扩展)

- **规则 1-8**: 沿用 rev.3
- **规则 9**(第 7 轮新增):**对称性 review** — 任何 if-else 分支新增逻辑,必须 review 对称性(同 input → 同 output)
- **规则 10**(第 7 轮新增):**修复必须复查所有触发点** — 修一个触发点时,必须 grep 同类函数的所有调用,逐个修
- **规则 11**(第 7 轮新增):**cargo test 通过不等于实施成功** — 必须有 integration test 验证实际行为
- **规则 12**(第 7 轮新增):**实施后必须再读代码** — 5 轮 review 教训:写文档容易看代码难,实施 plan 必须包含代码再读 checkpoint

---

## 14. 与 rev.3 final 的差异

| # | 方面 | rev.3 final | 第 7 轮修订 |
|---|---|---|---|
| Bug A 修复 | `CoInitializeEx` | ✅ 已记录 | ✅ + N4 RAII ComGuard(顺手) |
| Bug B 修复 | toolbar 按钮 reset | ✅ 已记录 | ✅ + N5 立即重显 + N6 flag 30s 延迟(可选) |
| Bug C 修复 | 延迟 initial ping 5s | ✅ 已记录 | ✅ + N2 30s boot grace period(整体) |
| Bug D 修复 | loadBool helper | ✅ 建议 focus-22+ | ✅ 本轮一起做(N7) |
| P3 #1 累加重构 | 重构 inject_via_uia | ✅ focus-21 可选 | ❌ 推迟 focus-22+(N8 风险) |
| P3 #2 duration 25s → 8s | 改 duration | ✅ focus-21 顺手 | ✅ 保留 |
| P3 #3 inventory log debug | 改 debug-level | ✅ focus-21 顺手 | ✅ 保留 |
| P3 #4 预存限制 | focus-22+ | ✅ focus-22+ | ✅ 保留 |
| **N1 commit_via_uia 对称性** | 未处理 | (未提) | ✅ **方案 B + user-verify 必修** |
| **N3 commit_via_uia 验证 + 决策树** | 未处理 | (未提) | ✅ **扩展 §12.7 决策树** |
| **N9 实施 plan 加固** | 未处理 | (未提) | ✅ **添加 checkpoint + I-COM test** |
| **user-verify 任务扩展** | J + K | ✅ | ✅ + **N1-verify-2/3 必修** |
| **user-verify 阻断** | 30 min task | ✅ | ✅ + **§10.7 BLOCKING gate 标记** |
| 总工作量 | 4.5-5 h + 30 min user | ✅ | **5-5.5 h + 30-45 min user**(必修增加) |

---

## 15. 文档链接

**本文档(第 7 轮 review)**:
`F:\soft\00selfmade\filemanager\docs\superpowers\plans\2026-10-02-focus-20-retrospective-rev3-review.md`

**对照文档**:
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-architecture-failure-rev3.md`(rev.3 final,本文档输入)
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-rev2-review.md`(Mavis 第 6 轮,4 源码 Bug)
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-review.md` (Mavis 第 5 轮)

**5 轮 focus 实施产物**(均失败):
- focus-17 v0.3.0 / focus-18 v0.3.1 / focus-19 v0.3.2 / focus-20 v0.3.3

**用户已装版本**:
- `release/Sigma-File-Manager-2.2.0-focus-20-setup.exe`(17 MB, MD5 `882005bb...`)

---

## 16. 总结:为什么这些改进能让"修改后达到目标"

**核心论证**:

1. **N1 解决 Auto-Confirm 看起来"时好时坏"的问题** — 用户报告"Auto-Confirm 在 Edge 不 work",实际是 41477 / WinUI 分支行为不一致。修复后,Auto-Confirm 在 Chromium 和 Edge WinUI 表现一致(都 Invoke / 都不 Invoke)。即使 user-verify 否定 N1-verify-2,有 plan B(方案 A 回退)。

2. **N2 解决启动 notification 雪崩** — 启动 30 秒内最多 1 条 notification(Edge tip),其他全在 grace period 内抑制。比 rev.3 的"仅延迟 initial ping"更彻底。

3. **N3 + 决策树更新 + user-verify N1-verify 任务** 让 user-verify 不再是"开放检查项",而是 BLOCKING。focus-22 不能在 user-verify 完成前开始实施。

4. **N9 实施 plan 加固** 强制代码再读 + integration test,避免 5 轮 review 全在文档层的教训重演。

5. **N8 推迟 P3 #1 重构** 避免引入新 bug 的风险。

6. **N4-N7 LOW 改进** 让 UX 更精细(立即重显 + flag 延迟 + loadBool helper)。

**用户验证的关键路径**:
1. focus-21 v0.3.4 实施(代码改动 + integration test)
2. user-verify(J + K + N1-verify-2/3)— 用户在 Edge WinUI / Edge Legacy / Chromium 上各测一次
3. 如果 J + K + N1-verify-2 否定 → 选方案 A(回退 N1 方案 A,只 Write 不 Invoke) + 选项 E(文档化)
4. 如果 J + K + N1-verify-2 肯定 → ship focus-21 v0.3.4,继续 focus-22

**绝对不能避免的目标失败模式**:
- "源码修改后仍然无法达到目标" — 这意味着 user-verify 通过 + 实施后用户仍然看到 dialog 不导航
- **N1 修复(方案 B)对此直接负责**:如果方案 B 实施后 WinUI Invoke 真的 work,用户的目标达成
- **N3 user-verify 是必要前置**:只有 user-verify 通过,才说明 user 真的看到了导航
- **N8 推迟重构**:不在用户验证路径上增加新风险
- **N9 加固 plan**:避免实施过程中的新引入 bug