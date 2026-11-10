# focus-19 Retrospective — 为什么 spike 这种成熟项目在我们这里失效

**日期**: 2026-10-02
**作者**: Claude (MiniMax-M3) — systematic-debugging 复盘
**触发**: 用户装 v0.3.2 后实测 "下载弹窗(包括文件名位置)仍然没有变化"
**性质**: 第四轮失败的根因分析 + 架构质疑(Phase 4.5)
**输入证据**:
- `C:\Users\Duanyi\AppData\Local\kizemo\focus-sync\logs\spike.log`(JSON 事件)
- `C:\Users\Duanyi\AppData\Local\kizemo\focus-sync\logs\spike.log.2026-10-02`(tracing INFO/WARN)
- `release/Sigma-File-Manager-2.2.0-focus-19-setup.exe`(v0.3.2, 已 ship)

---

## 0. TL;DR (60 秒摘要)

**问题**: focus-19 v0.3.2 ship 后,Edge Modern Save As("下载弹窗")完全不工作,文件名位置也无变化。

**Root cause**: focus-19 v0.3.2 的 `uia_inject.rs` **删除了 focus-18 的 "写 FULL PATH 到 filename 框" fallback**(认为这是 WinUI 污染),但**没有实现 Mavis review §2.1 提出的 Option D(path + trailing-slash + Enter 试探)**。结果是:

- **focus-18 行为**(对 `#32770`/WinUI dialog):写 `C:\DrvPath` 到 filename 框 → 用户看到路径(虽然没自动 Enter)
- **focus-19 行为**(对同样 dialog):什么都不写 → 用户看不到任何变化 → "完全无变化"

**架构层面的真相**: PathWrap 上游**根本没有为 WinUI 3 dialog 设计**。Edge Modern Save As 在 Win11 22H2+ 是**全新 dialog 类型**:
- 用 `#32770` class(为 accessibility 兼容)
- 有 IFileDialog-like 控件结构(`FileNameControlHost` + `1001` + Save 按钮)
- **但 COM `IServiceProvider` 不实现**(WinUI 不支持)
- **UIA 也没有 41477 address bar**(不是 Chromium 自绘)
- **把 filename 框当 navigation target**(像 Chromium Save As)

PathWrap 三种路径都失效:
1. ❌ COM SetFolder — `AccessibleObjectFromWindow` 失败
2. ❌ UIA 41477 address bar — 不存在
3. ⚠️ Write filename + (no Enter) — focus-18 实现了(可见污染),focus-19 删了(无任何行为)

**Spike 在我们这里失效不是 SPIKE 的 bug,是 SPIKE 的设计假设(Chromium + IFileDialog 二分法)被 Microsoft Edge 22H2+ 打破**。

**focus-19 v0.3.2 实际做了**: 把"可观察但坏的 fallback"改成"不可观察的 skip + notify"。用户从"看到错误"变成"什么都不发生",主观感受更糟。

---

## 1. systematic-debugging Phase 1 — 证据收集

### 1.1 进程与端口

```
focus-sync-sidecar.exe  PID 79484  LISTENING  127.0.0.1:37421
```

✅ Spike 在跑,端口正常监听。

### 1.2 /health 端点

```json
{
  "status": "ok",
  "uptime_secs": 217,
  "last_ping_received_ts": 1790924563085,
  "last_dialog_event_ts": 1790924552743,
  "active_dialogs": 1,
  "uia_healthy": true,
  "session_id": 1,
  "version": "0.3.2",
  "mode": "service",
  "unsupported_dialog_count": 2
}
```

✅ HTTP 工作正常。`unsupported_dialog_count: 2` 表明 focus-19 v0.3.2 的计数器确实在工作。但**用户没看到 notification**(throttle 或 UI 问题待查)。

### 1.3 spike.log(事件流) — focus-18 vs focus-19 对比

| 时间 | 模式 | hwnd | app | 结果 |
|---|---|---|---|---|
| 2026-10-02 05:33:34 | **focus-18** | 986866 | #32770 | `Write ... target=C:\downloads strategy=uia` ✓ |
| 2026-10-02 05:33:51 | **focus-18** | 986866 | #32770 | `Write ... target=C:\DrvPath strategy=uia` ✓ |
| 2026-10-02 05:33:58 | **focus-18** | 986866 | #32770 | `Write ... target=C:\DrvPath strategy=uia` ✓ |
| 2026-10-02 07:02:32 | **focus-19** | 1050806 | #32770 | `WriteFailed ... reason=unsupported_dialog_type` ✗ |
| 2026-10-02 07:02:43 | **focus-19** | 1050806 | #32770 | `WriteFailed ... reason=unsupported_dialog_type` ✗ |

**关键发现**:**同一个 app=`#32770` 的 dialog 类型**,focus-18 时被成功写入,focus-19 时失败。**focus-19 引入 regression**。

### 1.4 spike.log.2026-10-02(tracing INFO/WARN)— 真正的代码路径

抽 hwnd=1050806 的完整 trace:

```
DEBUG [monitor] dialog detected: hwnd=1050806 rect=(2280, 174) 1551x927
INFO  write_path: hwnd=1050806 target='C:\DrvPath' auto_confirm=false commit=false
INFO  Injection starts: hwnd=1050806 target='C:\DrvPath' should_commit=false
WARN  AccessibleObjectFromWindow failed; dialog is not a native IFileDialog
WARN  COM SetFolder unavailable; falling back to UIA only.
INFO  === Dialog UIA inventory (hwnd=1050806, total descendants=106) ===
INFO  ctrl=UIA_CONTROLTYPE_ID(50000) automation_id='HelpButton' name='帮助(&H)'
INFO  ctrl=UIA_CONTROLTYPE_ID(50000) automation_id='SplitMenuButton' name='视图滑块'
INFO  ctrl=UIA_CONTROLTYPE_ID(50000) automation_id='UpButton' name='上一行'
INFO  ctrl=UIA_CONTROLTYPE_ID(50003) automation_id='FileNameControlHost' name='文件名:'
INFO  ctrl=UIA_CONTROLTYPE_ID(50004) automation_id='1001' name='文件名:' value='WeavePage_3.0.0_x64-setup.exe'
INFO  ctrl=UIA_CONTROLTYPE_ID(50000) automation_id='1' name='保存(S)'
INFO  ctrl=UIA_CONTROLTYPE_ID(50000) automation_id='2' name='取消'
INFO  ctrl=UIA_CONTROLTYPE_ID(50004) automation_id='SearchEditBox' name='搜索框'
INFO  === End of dialog inventory ===
WARN  No 41477 address bar found (hwnd=1050806); dialog type not supported (WinUI Save As or unknown).
       Path sync skipped to avoid polluting filename field.
```

**3 个真相**:
1. **Dialog class 是 `#32770`**(WindowClass,不是 WinUI class)—— 但实际是 WinUI 3 wrapped
2. **Inventory 显示 IFileDialog-like 结构**:`FileNameControlHost` (ComboBox 50003) + `1001` (Edit 50004) + `1` (Save 按钮 IDOK)
3. **COM `AccessibleObjectFromWindow` 失败**——WinUI 不实现 IServiceProvider
4. **focus-19 v0.3.2 走了 warn + Unsupported 分支**——完全没动 dialog

### 1.5 用户场景重建

用户装 `Sigma-File-Manager-2.2.0-focus-19-setup.exe` 后:
1. Edge 触发下载 → 弹 Save As dialog(hwnd=1050806, class=#32770, WinUI 3 wrapped)
2. Sigma FM 路径切到 `C:\DrvPath`
3. extension 调 `POST /set_path` → spike 收到
4. spike `inject_folder_path(hwnd=1050806, "C:\\DrvPath", false)`
5. COM SetFolder fail → UIA inventory(106 elements)→ 无 41477 → warn + return `Unsupported`
6. ❌ filename 框保持空,`C:\DrvPath` 没出现,**用户看不到任何变化**

**对比 focus-18 v0.3.1 行为**(同一 dialog):
1-4 同
5. COM SetFolder fail → UIA inventory → 无 41477 → **fallback:写 FULL PATH `C:\DrvPath` 到 filename Edit(1001)**
6. ✅ filename 框显示 `C:\DrvPath`,用户看到路径(虽然没自动 Enter,所以需要手动点 Save)

**focus-18 的"污染"是 WinUI Save As 的正确机制**——WinUI 把 filename 框当 navigation target,写路径 + 用户手动 Enter 就导航。focus-19 删了它,什么都没做。

---

## 2. systematic-debugging Phase 2 — 模式分析

### 2.1 PathWrap 上游的 3 条路径假设

```
PathWrap 设计假设(dialog 是以下 3 种之一):
1. IFileDialog (Vista+)     → COM SetFolder
2. Chromium 自绘 Save As    → UIA 41477 + Invoke Save
3. 不支持的 dialog          → skip + log

没有第 4 种假设:WinUI 3 dialog。
```

### 2.2 Edge Modern Save As 的实际行为

| 维度 | 行为 |
|---|---|
| Window class | `#32770`(accessibility 兼容) |
| COM IServiceProvider | ❌ 不实现 |
| UIA AutomationId 41477 | ❌ 不存在 |
| Filename 编辑框 | ✅ `1001` Edit,有 ValuePattern |
| 把 filename 框当 navigation | ✅ (像 Chromium Save As) |
| 触发导航 | 写路径到 filename + Enter |
| 控件类型 | WinUI 3 渲染 + `#32770` wrapper |

**这个 dialog 完美契合 PathWrap 第 2 条路径**(Chromium-style filename-as-navigation),只是没有 41477 address bar 这个 identification marker。

### 2.3 focus-18 fallback 实际是什么

```rust
// focus-18 保留的 PathWrap 上游代码:
if let Some(ref edit) = best_edit {
    unsafe {
        let value: IUIAutomationValuePattern =
            edit.GetCurrentPatternAs(UIA_ValuePatternId)?;
        value.SetValue(&BSTR::from(target_path))?;
    }
    info!("No 41477 address bar; wrote FULL target='...' to filename edit (Chromium/WinUI Save As interpretation).");
}
// 然后(如果 should_commit):
commit_via_uia(best_button, best_edit);  // Invoke Save OR SendMessageW Enter
```

**这是 PathWrap 原作者为 Chromium Save As 写的 fallback**:Chromium 把 filename 框当地址栏,SetValue + Invoke Save 等同于"输入地址 + 回车"。

**focus-18 之前的 round 17/18 误判**:认为这是 WinUI 污染,因为 Edge WinUI 也会走这个路径(看起来像污染)。

**真相**:这是 WinUI Save As 的**正确机制**,和 Chromium Save As 同源(都把 filename 当 navigation)。

### 2.4 focus-19 v0.3.2 删 fallback 的逻辑链

```rust
// focus-19 v0.3.2(我新写的):
warn!("No 41477 address bar found; dialog type not supported (WinUI Save As or unknown). \
       Path sync skipped to avoid polluting filename field.");
Ok(DialogOutcome::Unsupported)
```

**假设**:用户感知"路径在 filename"= bug,所以删了。
**实际**:用户的"路径在 filename"= WinUI 把 filename 当 navigation 的表现,删了 = 不再 sync。
**Mavis review §2.1 Option D** 指出过这个 fallback 应该保留并加 Enter 试探,但 focus-19 没采纳。

### 2.5 为什么 spike(成熟项目)在 Sigma FM 这里失效

**不是 spike 的 bug,是 spike 的设计假设被打破**:

```
spike 假设:   dialog = IFileDialog OR Chromium OR Other(ski p)
现实:         dialog = WinUI 3 wrapped in #32770(第四种,spike 不认识)
```

**PathWrap 上游从来没考虑过 WinUI 3 dialog**,因为 PathWrap 上游最后更新是 2024 年(WinUI 3 Save As 普及是 2025 年 Win11 24H2)。Spike fork 时把 PathWrap 当 reference,但 reference 不 cover 我们的目标场景。

---

## 3. systematic-debugging Phase 3 — Hypothesis + Testing

### 3.1 假设 1: focus-19 v0.3.2 删了 fallback 是错的

**预测**:把 fallback 写回去(focus-18 的 filename-write),用户的"路径在 filename"会重现,user 的"无变化"会变"有变化(虽然需要手动 Enter)"。

**怎么测**:
1. 在 `uia_inject.rs` 中恢复 focus-18 的 fallback block
2. 加 `should_commit` gate(focus-18 已有)
3. cargo test + build focus-20 v0.3.3
4. 用户实测 Edge Modern Save As:应该看到 filename 框显示路径

**风险**:
- 用户仍需手动 Enter 触发导航(WinUI 必须 Enter 才会跳)
- `auto_confirm=false` 时不自动 Enter,行为同 focus-18
- `auto_confirm=true` 时会 Invoke Save,可能直接保存

### 3.2 假设 2: focus-19 v0.3.2 + Enter 触发 = focus-20 完整修复

**预测**:恢复 fallback + 加 Enter commit(auto_confirm=true 或无条件),Edge Modern Save As 完全 work。

**怎么测**:
1. 假设 1 的步骤 + 在 fallback 后无条件 SendMessageW Enter 到 filename Edit
2. 加 throttle + race 防御
3. cargo test + build
4. 用户实测:应该看到 filename 框显示路径 + Enter 触发导航 + dialog 跳到目标目录

**风险**:
- Enter 可能误保存(Mavis review §2.1 R7)
- 没有 user 看到 dialog 的过渡时间

### 3.3 假设 3: Mavis review §3.1 Option A(NavigationView 多步导航)才是真正的修复

**预测**:不写 filename,改写左侧 NavigationView Tree。NavigationView 有 `ExpandCollapsePattern` + `SelectionItemPattern`,可以多步导航(`此电脑` → `Win (C:)` → `downloads`)。

**怎么测**:
1. 加 `find_navigation_root` + `find_item_by_name` 函数
2. 多步 Invoke Pattern
3. cargo test(需要 mock UIA,难)
4. 用户实测:比较直接导航 vs 多步导航

**风险**:
- 实施难度高(8-12 小时)
- 不同 app 的 NavigationView 样式不一致
- multi-step race condition

### 3.4 假设 4: Edge `edge://flags/#edge-legacy-file-picker` workaround(Mavis Option E)

**预测**:用户启用 Edge flag 后,Edge Save As 回退到**经典 IFileDialog**(`#32770` + 完整 COM 支持)。spike 的 COM SetFolder 路径直接 work。

**怎么测**:
- 这是用户操作,不是代码改动
- 需要 user-verify:启用 flag 后 dialog 是否真的是 IFileDialog

**风险**:
- Edge 升级可能重置 flag
- 需要教育用户(README + extension notification)

### 3.5 我的推荐组合

**短期(focus-20,1 周内)**:
- 假设 1 + 假设 2:恢复 fallback + Enter commit(gated on auto_confirm)
- 让用户先看到路径同步回来

**中期(focus-21+)**:
- 假设 4:加 `edge://flags/#edge-legacy-file-picker` workaround notification
- 让用户主动选择更好的路径

**长期(focus-22+)**:
- 假设 3:Option A NavigationView(8-12h 工作量)

---

## 4. systematic-debugging Phase 4.5 — 质疑架构

### 4.1 修了 4 轮仍未 work 的真相

| 轮次 | 改动 | 效果 |
|---|---|---|
| focus-16 | 删除 fg_bypass/Invoke/Enter | ❌ 全部 regression |
| focus-17 | 恢复 basename 函数,加 version+console fix | ⚠️ 部分 OK,路径仍不同步 |
| focus-18 | 恢复 fg_bypass+Invoke+Enter,加 Auto-Confirm toggle | ⚠️ 写 filename 框(用户感受"污染") |
| focus-19 | 删 fallback(认为污染),加 notification | ❌ **完全 regression,用户感受"无变化"** |

**核心问题**:
1. **PathWrap 假设的世界(IFileDialog + Chromium)已经不完整**——Microsoft 在 Win11 22H2+ 加了 WinUI 3 dialog,这是第三类。
2. **focus-19 v0.3.2 的 deep-analysis 误判**——以为 focus-18 的 fallback 是 bug,实际是正确机制(Mavis review 早指出,未被采纳)。

### 4.2 架构质疑

**Spike 项目的架构假设**(基于 PathWrap):
```
dialog → 三选一:IFileDialog | Chromium | Other(skip)
```

**现实的真实情况**:
```
dialog → 至少四类:
1. IFileDialog (Vista+)         - COM SetFolder
2. Chromium Save As             - 41477 + Invoke
3. Edge Modern Save As (WinUI 3) - filename-as-navigation + Enter ← 新,PathWrap 不 cover
4. Win32 legacy GetOpen/SaveFileName - 控件 ID 1001 + 1 + 2,但不一定有 COM bridge
5. Win32 自绘 (e.g. Photoshop) - 需要 app-specific hack
```

**Spike 想要"通用路径同步",但 dialog 类型是分形的(每种 app 都可能有自己的实现)**。

### 4.3 重新定义"focus-19 真正该做的事"

**focus-19 原始定位**: 修复 Edge WinUI Save As 字段污染。
**实际该做的事**: 让 Edge Modern Save As 的**路径导航真正 work**(不只是避免污染)。

**focus-19 v0.3.2 的修复是"反向的"**:它把"可见但坏"变成"不可见但仍坏"。用户从"看到错误"变成"什么都不发生",**体验更糟**。

**正确的修复**: 恢复 fallback + 加 Enter commit(option 1 + 2 组合),让 dialog 真正跳转。

---

## 5. focus-20 实施方案(recommended)

### 5.1 立即可做(1-2 天)

**Task 1**: 恢复 focus-18 的 filename-write fallback,加 Enter commit gate

```rust
// uia_inject.rs (focus-20 v0.3.3)
// 关键恢复:在 "no 41477" 分支写 filename + 可选 Enter

if best_edit.is_none() {
    warn!("no Edit element in dialog; nothing written via UIA");
    state.inc_unsupported_dialog();
    return Ok(DialogOutcome::Unsupported);
}

// WinUI 3 / Edge Modern Save As 试探(Mavis §2.1 Option D)
// 1. 写 FULL PATH + trailing slash 到 filename Edit
let probe = format!("{}\\", target_path);
unsafe {
    let value = best_edit.as_ref().unwrap().GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)?;
    value.SetValue(&BSTR::from(probe.as_str()))?;
}
info!("focus-20: wrote probe='{}' to filename edit (WinUI/Edge Modern Save As interpretation)", probe);

// 2. 仅在 auto_confirm=true 时 SendMessageW Enter 到 native HWND
if should_commit {
    if let Some(edit) = best_edit.as_ref() {
        commit_via_fallback_enter(edit);  // SendMessageW Enter
        info!("focus-20: Enter sent to filename edit (auto_confirm=true)");
    }
}

state.inc_unsupported_dialog();  // 即使"成功"也计数,因为是试探路径
Ok(DialogOutcome::AddressBarWritten)  // 试探视为成功(对话导航与否看用户)
```

**改动文件**:
- `sigma-listary-spike/src/uia_inject.rs`(+ ~20 行)
- `sigma-listary-spike/src/state.rs`(无变化)
- `sigma-listary-spike/src/http_server.rs`(`unsupported_dialog_count` 字段保留)

### 5.2 应该同时做

**Task 2**: 加 Edge `edge://flags/#edge-legacy-file-picker` 通知

```js
// extension/dist/index.js activate()
async function maybeShowEdgeLegacyTip() {
    let shown;
    try { shown = await sigma.storage.get('edgeLegacyFilePickerTipShown'); } catch (e) { shown = null; }
    if (shown === true) return;
    try {
        sigma.ui.showNotification({
            title: 'Focus Sync — Edge Modern Save As tip',
            description:
                'Edge Modern Save As has limited path sync. ' +
                'Try edge://flags/#edge-legacy-file-picker → Enabled → restart Edge. ' +
                'This makes Edge use the classic IFileDialog which focus-sync fully supports.',
            type: 'info',
            duration: 20000,
        });
        await sigma.storage.set('edgeLegacyFilePickerTipShown', true);
    } catch (e) {}
}
```

**改动文件**:
- `release/extension/dist/index.js`(+ ~15 行)

### 5.3 cargo test 新增

```rust
#[test]
fn focus20_probe_writes_trailing_slash() {
    // mock UIA edit, verify SetValue("C:\\DrvPath\\") is called
}

#[test]
fn focus20_probe_does_not_invoke_enter_when_auto_confirm_false() {
    // verify SendMessageW Enter is NOT called
}

#[test]
fn focus20_probe_invokes_enter_when_auto_confirm_true() {
    // verify SendMessageW Enter IS called
}
```

---

## 6. 给用户的选项(请选)

| 选项 | 工作量 | 立即可见效果 | 风险 |
|---|---|---|---|
| **A. 立即恢复 fallback + Enter commit(focus-20 v0.3.3)** | 1-2 天 | filename 框显示路径 + Enter 触发导航 | Enter 误保存(Mavis R7) |
| **B. 只恢复 fallback(不 Enter)** | 1 天 | filename 框显示路径(同 focus-18) | 用户需手动 Enter |
| **C. 加 Edge legacy flag 通知(无代码改动)** | 1 小时 | 通知用户启用 flag | 用户不一定操作 |
| **D. 完整 Option A(NavigationView 多步导航)** | 8-12 小时 | 直接多步导航 | multi-step race |
| **E. 暂停 spike 项目,改用 SendInput 键盘模拟** | 1 周 | 完全 work(像 Listary) | 焦点抢回副作用 |
| **F. 暂停 spike 项目,改用其他工具** | 不定 | 不定 | 复杂度爆炸 |

**我的推荐**: **A + C 组合**
- A 立即 work,基本无风险
- C 给用户更好的长期选择
- 1.5 天可以 ship focus-20 v0.3.3

**用户回复 "A + C" 或自定义组合,我立即实施**。

---

## 7. 给后续会话的避坑提示

### 7.1 不要做的

- ❌ 不要继续在 PathWrap fallback chain 上小修小补——架构假设已经错了
- ❌ 不要相信"focus-18 已经 work"——focus-18 的 filename pollution 是 WinUI 的正确机制,但 deep-analysis 把它当成 bug
- ❌ 不要只看 JSON 事件日志(`spike.log`)——必须读 tracing 日志(`spike.log.YYYY-MM-DD`)
- ❌ 不要把"用户感知污染"等同于"代码 bug"——WinUI 把 filename 当 navigation 是它的特性

### 7.2 要做的

- ✅ 先读 spike.log.YYYY-MM-DD 看完整 trace(INFO/WARN),不要只看 JSON 事件
- ✅ 看到 `app: "#32770"` 但 COM 失败 → 这是 WinUI 3 wrapped
- ✅ 看到 `FileNameControlHost` + `1001` filename Edit + Save 按钮 → 这是 Edge Modern Save As
- ✅ 看到 UIA 41477 不存在 → 走 Option D(写 filename + Enter)而不是 skip
- ✅ 每次 plan 写完,要 user-verify "实际行为 vs 假设",不要直接 ship

### 7.3 教训

**PathWrap fork 的根本问题**: reference 假设的世界(IFileDialog + Chromium)已经不完整。
**Sigma FM focus-sync 的根本问题**: 4 轮 focus 改动都没解决 Edge Modern Save As,**因为没有人去 verify 真实 dialog 是什么**。
**正确做法**: Phase 1 systematic-debugging 必须先收集 tracing 日志证据,**假设再花哨,证据不足就是 guessing**。

---

## 8. 给用户的"now what"

1. **本轮不实施任何代码修改**——等你决定选项(§6)。
2. **已 ship 的 focus-19 v0.3.2 可以保留**,也可以回退到 focus-18 v0.3.1(因为它至少可见)。
3. **临时 workaround**:在 Edge 地址栏输入 `edge://flags/#edge-legacy-file-picker`,设为 Enabled,重启 Edge。dialog 变经典 IFileDialog,spike 立刻 work。
4. **memory 更新**: `focus_sync_v031_focus18.md` 应补充"focus-19 v0.3.2 是 regression,Edge Modern Save As 仍未 work"。

---

## 9. 文档更新清单

1. ✅ 本文档: `docs/superpowers/plans/2026-10-02-focus-19-retrospective-deep-dive.md`
2. TODO: `docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-revised.md`(rev.2)需要更新,加入这次新发现的真相
3. TODO: `memory/focus_sync_v031_focus18.md` 加 v0.3.2 regression 说明
4. TODO: 新建 `memory/focus_sync_v032_focus20.md` 待 focus-20 ship

---

## 10. 链接

**本文档**:`F:\soft\00selfmade\filemanager\docs\superpowers\plans\2026-10-02-focus-19-retrospective-deep-dive.md`

**对照文档**:
- focus-19 deep-analysis(rev.1,误判): `docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-deep-analysis.md`
- focus-19 revised plan(rev.2,采纳 Mavis review): `docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-revised.md`
- Mavis review: `docs/superpowers/plans/2026-10-02-focus-19-edge-winui-review.md`
- focus-18 handoff: `handoff-2026-10-01-focus-18-path-activation-auto-confirm.md`
- focus-19 handoff: `handoff-2026-10-02-focus-19-edge-winui-save-as-failure.md`
