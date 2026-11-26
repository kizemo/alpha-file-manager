# focus-sync v0.3.1 — 路径 B 实施方案(Auto-Confirm Toggle)

**日期**: 2026-10-01
**作者**: Mavis (M3)
**基础**: 
- `docs/superpowers/plans/2026-10-01-focus-18-path-activation-architecture-regression.md`(Claude 诊断)
- `docs/superpowers/plans/2026-10-01-focus-18-architecture-regression-review.md`(Mavis review)
- `git show 905db31`(原 PathWrap 实现)
**接收对象**: Claude Code(用户将转交)
**Tag 目标**: v0.3.1(v0.3.0 实际未完全解决 path activation)

---

## 0. TL;DR

**核心改动**:恢复原 PathWrap 的 `fg_bypass` + Invoke + Enter 完整链路,但通过 **Auto-Confirm toggle(默认 OFF)** 门控 commit 步骤,避免重复自动保存。

**用户行为**:
- **默认(OFF)**:`extension/dist/index.js` 把 Sigma FM 路径写到 dialog filename 框(完整路径,如 `C:\downloads\`),**不自动 Invoke Save**。用户手动点 Save。这是经典 Listary 行为。
- **ON(opt-in)**:Toolbar 菜单点 "Auto-Confirm: ON" → 下次 PUSH 自动 Invoke Save,**每个 dialog session 触发一次**(dedup)。

**为什么这个方案比 Claude 原方案好**:
1. **保留 FULL PATH 写入**(不是 basename)— 原 PathWrap 成功的核心机制
2. **dedup 改为 per-dialog single-shot**(不是 path-change)— 避免用户 Sigma FM 浏览 A→B→C 触发 3 次自动保存
3. **默认安全** — Auto-Confirm OFF 是 path-only,不会误保存
4. **fg_bypass 恢复** — 解决 Chromium dialog UIA 焦点抢回问题

---

## 1. Goal & Non-Goals

### 1.1 Goals

| # | 目标 | 验收 |
|---|---|---|
| G1 | 用户在 Sigma FM 浏览时,Edge / Chrome / 影刀 Save As dialog filename 框自动填充 Sigma FM 路径(完整路径) | 视觉测试 |
| G2 | Auto-Confirm ON 时,首次 push 自动 Invoke Save(每个 dialog 仅一次) | 7-app 矩阵 |
| G3 | Auto-Confirm OFF 时,只写 path 不 Invoke(默认安全) | 7-app 矩阵 |
| G4 | Chromium dialog(`Chrome_WidgetWin_*`)UIA 调用不被宿主抢回焦点 | fg_bypass 单元测试 |
| G5 | per-dialog single-shot dedup — 同一 dialog 多次 push 只 commit 一次 | state 单元测试 |
| G6 | UI 新增 "Auto-Confirm" toolbar 菜单项,默认 OFF,可切换 | 手动测试 |

### 1.2 Non-Goals

- `extension/dist/index.js` stale 通知清除(原 Mavis §4.3)— 接受现状
- `dirname` 函数清理 — 建议删,但非阻塞
- `extension/dist/index.js` description / i18n / FORK-KEEP-LIST / memory 更新 — 不在本次范围
- Sigma FM extension normalize 兼容性 — 假设下次重启仍 OK(risk)
- 多 Sigma FM 实例同步 — 不支持
- 远程 sidecar 调用 — 假设 127.0.0.1

---

## 2. 架构总览

### 2.1 数据流(用户启用 Auto-Confirm 并触发 sync)

```
[Sigma FM] 路径变化
    ↓ onPathChange
[extension/dist/index.js]
    ├─ 读取 sigma.storage('autoConfirm') → true
    ├─ normalizePath('C:/downloads') → 'C:\\downloads'
    ├─ schedulePush(path)
    │     ↓ 100ms debounce
    └─ sigma.http.request({
         url: 'http://127.0.0.1:37421/set_path',
         body: { path: 'C:\\downloads', auto_confirm: true }
       })
    ↓
[spike http_server::handle_set_path]
    ├─ 解析 body: { path, auto_confirm }
    ├─ state.set_current_path(path)
    ├─ state.active_dialogs() → [DialogInfo{ hwnd: 7539060, ... }]
    └─ 对每个 dialog:
         writer::write_path(state, hwnd, path, auto_confirm)
    ↓
[writer::write_path]
    ├─ state.try_mark_committed(hwnd) → 首次返回 true(后续返回 false)
    ├─ should_commit = auto_confirm && first_time
    └─ fg_bypass::run_with_bypass(hwnd, || {
         uia_inject::inject_folder_path(hwnd, path, should_commit)
       })
    ↓
[uia_inject::inject_folder_path]
    ├─ try_set_folder_via_com(hwnd, path)
    │     └─ 成功 → return (native IFileDialog,address bar 已跳)
    ├─ inject_via_uia(hwnd, path, should_commit)
    │     ├─ 找到 41477 address bar → SetValue(path) + if should_commit: Invoke Save
    │     ├─ 找到 1001/1148 filename edit → SetValue(path) + if should_commit: Invoke Save
    │     └─ 否则: warn, no-op
    └─ return
```

### 2.2 数据流(用户禁用 Auto-Confirm,默认)

```
[Sigma FM] 路径变化
    ↓
[extension] body: { path, auto_confirm: false }
    ↓
[writer::write_path]
    ├─ state.try_mark_committed(hwnd) → true (but should_commit=false)
    └─ should_commit = false
    └─ uia_inject::inject_folder_path(hwnd, path, false)
    ↓
[uia_inject] 只 SetValue, 不 Invoke
    → dialog 显示 filename = 'C:\\downloads\\'
    → 用户手动点击 Save
```

### 2.3 dedup 行为

| 场景 | 状态 |
|---|---|
| Dialog 第一次 push,Auto-Confirm ON | `try_mark_committed` 返回 true → Invoke |
| Dialog 第二次 push,Auto-Confirm ON | `try_mark_committed` 返回 false → skip Invoke,仍 SetValue |
| Dialog 任何 push,Auto-Confirm OFF | skip Invoke,只 SetValue |
| Dialog Closed event | `state.clear_committed(hwnd)` → 下次 dialog 可重新 commit |

---

## 3. 文件改动清单

| # | 文件 | 操作 | 任务 |
|---|---|---|---|
| 1 | `sigma-listary-spike/src/fg_bypass.rs` | **新建**(从 git history 905db31 恢复) | Task 1 |
| 2 | `sigma-listary-spike/src/writer.rs` | 修改(加 fg_bypass 包裹 + state 引用 + auto_confirm 传递) | Task 2 |
| 3 | `sigma-listary-spike/src/uia_inject.rs` | 修改(恢复 FULL PATH 写 + Invoke + Enter + auto_confirm 参数) | Task 3 |
| 4 | `sigma-listary-spike/src/state.rs` | 修改(+ committed_hwnds: Mutex<HashSet<u32>> + try_mark_committed/clear_committed 方法) | Task 4 |
| 5 | `sigma-listary-spike/src/http_server.rs` | 修改(解析 body.auto_confirm + 调 writer::write_path 传 4 参数) | Task 5 |
| 6 | `sigma-listary-spike/src/lib.rs` | 修改(声明 `pub mod fg_bypass;`) | Task 1 |
| 7 | `sigma-listary-spike/src/main.rs` | 修改(DialogEvent::Closed → clear_committed) | Task 4 |
| 8 | `release/extension/dist/index.js` | 修改(+ autoConfirm state + toolbar item + body field) | Task 6 |
| 9 | `release/extension/package.json` | 无需改(commands 列表已用 contributes.commands 在 register.ps1 维护) | — |
| 10 | `release/extension-installer/register.ps1` | 修改(+ focus-sync.autoConfirm toggle command) | Task 7 |

**总改动**:10 个文件(其中 1 个新建、9 个修改),预计代码量 ~250 行(含测试)。

---

## 4. Task-by-Task 实施

### Phase 0: 全局约束(沿用 focus-17 plan §Global Constraints)

- NO git commit during execution — Task 8 批量 commit
- 每改一处 Rust 代码必须新增至少 1 个单元测试
- Edit 而非 sed/awk
- 不删任何用户文件
- 备份 register.ps1 到 `C:\Temp\register.ps1.bak-focus18`

### Task 1: 恢复 `fg_bypass.rs`

**目的**: 解决 Chromium dialog(`Chrome_WidgetWin_*`)UIA 调用被宿主自我夺回焦点的问题。原 commit 8016f3d 引入,被 focus-16 删除。

**Step 1**: 从 git history 恢复 `fg_bypass.rs`

```bash
git show 905db31:sigma-listary-spike/src/fg_bypass.rs > sigma-listary-spike/src/fg_bypass.rs
```

**Step 2**: 在 `lib.rs` 加 `pub mod fg_bypass;`

**文件**:`sigma-listary-spike/src/lib.rs` 行 17-26
```rust
pub mod config;
pub mod detector;
pub mod events;
pub mod fg_bypass;       // ← 新增
pub mod http_server;
pub mod logger;
pub mod reader;
pub mod state;
pub mod uia_event;
pub mod uia_inject;
pub mod writer;
```

**Step 3**: 添加单元测试(检查 `is_chromium_target_window` 行为)

**文件**:`sigma-listary-spike/src/fg_bypass.rs`(恢复后,文件末尾追加)

注:实际测试需要 Windows HWND mock,这里只验证函数对已知 Chromium class name 返回 true,非 Chromium 返回 false。如果原文件已含此类测试则跳过。

**验收**:`cargo test fg_bypass:: -- --nocapture` PASS。

### Task 2: 修改 `writer.rs` — 加 state + auto_confirm + fg_bypass

**文件**:`sigma-listary-spike/src/writer.rs`

**改动**: 重写整个 `write_path` 函数签名,加 2 个新参数

**改动前**(行 29-39):
```rust
pub fn write_path(hwnd_isize: isize, target: &str) -> WriteOutcome {
    if hwnd_isize == 0 {
        return WriteOutcome::Failed("hwnd is 0".into());
    }
    let hwnd_u32 = hwnd_isize as u32;
    info!("write_path: hwnd={hwnd_isize} target='{target}'");
    crate::uia_inject::inject_folder_path(hwnd_u32, target);
    WriteOutcome::UiaSetValue
}
```

**改动后**:
```rust
use crate::state::AppState;
use tracing::{info, warn};

/// 写 path 到 dialog,可选地 commit (Invoke Save / Enter)。
/// `auto_confirm` 来自 extension PUSH body,默认 false。
/// per-dialog 单次 commit (state.try_mark_committed 实现)。
pub fn write_path(
    state: &AppState,
    hwnd_isize: isize,
    target: &str,
    auto_confirm: bool,
) -> WriteOutcome {
    if hwnd_isize == 0 {
        return WriteOutcome::Failed("hwnd is 0".into());
    }
    let hwnd_u32 = hwnd_isize as u32;

    // focus-18: per-dialog single-shot commit dedup.
    // 第一次 push 到新 hwnd → try_mark_committed 返回 true → 可 commit。
    // 后续 push → 返回 false → 跳过 commit(只写 path)。
    let should_commit = if auto_confirm {
        state.try_mark_committed(hwnd_u32)
    } else {
        false
    };

    info!(
        "write_path: hwnd={hwnd_isize} target='{target}' auto_confirm={auto_confirm} commit={should_commit}"
    );

    // focus-18: 恢复 fg_bypass 包裹(Chromium 焦点抢回防护)。
    // 对 native IFileDialog / WinUI:is_chromium_target_window 返回 false → no-op。
    crate::fg_bypass::run_with_bypass(hwnd_u32, || {
        crate::uia_inject::inject_folder_path(hwnd_u32, target, should_commit);
    });

    WriteOutcome::UiaSetValue
}
```

**Step 4**: 添加单元测试(假 state)

**文件**:`sigma-listary-spike/src/writer.rs` 文件末尾追加

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    #[test]
    fn write_path_hwnd_zero_returns_failed() {
        let state = AppState::new("C:\\".into(), 1);
        let outcome = write_path(&state, 0, "C:\\downloads", false);
        assert!(matches!(outcome, WriteOutcome::Failed(_)));
    }

    #[test]
    fn write_path_auto_confirm_false_never_marks_committed() {
        let state = AppState::new("C:\\".into(), 1);
        // hwnd=0 → early return,不调 try_mark_committed
        let _ = write_path(&state, 0, "C:\\downloads", false);
        assert!(state.committed_hwnds.lock().unwrap().is_empty());
    }
}
```

**验收**:`cargo test writer::` PASS。

### Task 3: 修改 `uia_inject.rs` — 恢复 FULL PATH + Invoke + Enter

**文件**:`sigma-listary-spike/src/uia_inject.rs`

**改动 3 处**:

#### 改动 A:`inject_folder_path` 加 `should_commit` 参数

**改动前**(行 174):
```rust
pub fn inject_folder_path(dialog_hwnd: u32, target_path: &str) {
```

**改动后**:
```rust
pub fn inject_folder_path(dialog_hwnd: u32, target_path: &str, should_commit: bool) {
```

并在函数内部,COM SetFolder 成功后保留早 return(Bug B 修复);UIA fallback 分支的 commit 由 `should_commit` 门控。

#### 改动 B:UIA fallback 恢复 FULL PATH 写 + Invoke / Enter

**改动位置**:`inject_via_uia` 函数尾部。

**改动前**(行 321-348):
```rust
if let Some(bar) = address_bar {
    let value: IUIAutomationValuePattern =
        unsafe { bar.GetCurrentPatternAs(UIA_ValuePatternId)? };
    unsafe { value.SetValue(&BSTR::from(target_path))? };
    info!("Wrote target to address bar (41477).");
    return Ok(());
}

if let Some(edit) = best_edit {
    let name = basename(target_path);
    if !name.is_empty() {
        let value: IUIAutomationValuePattern =
            unsafe { edit.GetCurrentPatternAs(UIA_ValuePatternId)? };
        unsafe { value.SetValue(&BSTR::from(name))? };
        info!(
            "No 41477 address bar; wrote real BASENAME='{}' (from target '{}') to filename edit.",
            name, target_path
        );
    } else {
        info!("No 41477 address bar; target has no basename component, skipping.");
    }
} else {
    warn!("no Edit element in dialog; nothing written via UIA");
}
```

**改动后**:
```rust
// focus-18: 恢复 FULL PATH 写(不是 basename)。Chromium / Edge Save As 的
// filename 框接受完整路径,SetValue("C:\downloads\") 后 Invoke Save 会
// 导航到该路径并保存(basename-only 写入则只改 filename 不改目录,失去 sync 意义)。
if let Some(bar) = address_bar {
    let value: IUIAutomationValuePattern =
        unsafe { bar.GetCurrentPatternAs(UIA_ValuePatternId)? };
    unsafe { value.SetValue(&BSTR::from(target_path))? };
    info!("Wrote FULL target to address bar (41477): '{}'", target_path);
    // 41477 路径 commit:Chromium Save As SetValue 后 + Invoke 触发导航
    if should_commit {
        commit_via_uia(&bar)?;  // 用 best_button 即可
    }
    return Ok(());
}

if let Some(edit) = best_edit {
    // focus-18: 写 FULL PATH(不是 basename)。用户可手动覆盖。
    unsafe {
        let value: IUIAutomationValuePattern =
            edit.GetCurrentPatternAs(UIA_ValuePatternId)?;
        value.SetValue(&BSTR::from(target_path))?;
    }
    info!(
        "No 41477 address bar; wrote FULL target='{}' to filename edit (Chromium/WinUI Save As interpretation).",
        target_path
    );
} else {
    warn!("no Edit element in dialog; nothing written via UIA");
}

// focus-18: commit 步骤。SetValue 后 Invoke Save button 触发 dialog navigation。
// 仅当 should_commit=true(per-dialog 单次,writer::write_path 已 dedup)。
if should_commit {
    commit_via_uia(/* dialog root or best_button */)?;
}
```

**注**:`commit_via_uia` 是新 helper,见改动 C。

#### 改动 C:新增 `commit_via_uia` helper + 恢复 `fallback_confirm`

**改动前**(行 414-426 `fallback_confirm` 函数已存在但 dead code)

**改动后**:把 `fallback_confirm` 改名为 `commit_via_fallback_enter`(语义清晰),加 `commit_via_uia` 顶层 helper:

```rust
/// focus-18: 在已经 SetValue 之后,Invoke Save button(commit 步骤)。
/// 优先 Invoke best_button(IDOK / 保存 / open / save 名);
/// 找不到时 fallback 到 SendMessageW Enter 到 filename edit。
fn commit_via_uia(dialog_root: &IUIAutomationElement) -> Result<()> {
    // 重新遍历 dialog 找 best_button(我们已经遍历过一遍但 best_button 是 inject_via_uia 的局部变量)
    // 简化:重新扫描一次。或把 best_button 提升为函数返回值。
    //
    // 实际: 直接复用 inject_via_uia 里的 best_button 变量最简单,但需要重构函数签名。
    // 这里采用简单方案:重新扫描 best_button。
    let uia = automation().ok_or_else(|| Error::from(E_FAIL))?;
    let condition = unsafe { uia.CreateTrueCondition()? };
    let elements = unsafe { dialog_root.FindAll(TreeScope_Descendants, &condition)? };
    let count = unsafe { elements.Length()? };

    let mut best_button: Option<IUIAutomationElement> = None;
    let mut best_button_score = i32::MIN;

    for i in 0..count {
        let element = unsafe { elements.GetElement(i)? };
        let control_type = unsafe { element.CurrentControlType()? };
        if control_type == UIA_ButtonControlTypeId {
            let score = confirm_button_score(&element);
            if score > best_button_score {
                best_button_score = score;
                best_button = Some(element);
            }
        }
    }

    match best_button {
        Some(button) if best_button_score > 0 => {
            let invoke: IUIAutomationInvokePattern =
                unsafe { button.GetCurrentPatternAs(UIA_InvokePatternId)? };
            unsafe { invoke.Invoke()? };
            info!("Committed via Invoke(Save button).");
            Ok(())
        }
        Some(edit_fallback) => {
            // 没找到合适的 button,但找到了 edit(应不会发生,函数 require dialog_root)
            warn!("no Save button found; commit skipped");
            Ok(())
        }
        None => {
            warn!("no Save button in dialog; commit skipped");
            Ok(())
        }
    }
}

/// focus-18: fallback confirm — SendMessageW Enter 到 filename box native HWND。
/// 不触发 SendInput(避免焦点抢回)。
fn commit_via_fallback_enter(edit: &IUIAutomationElement) {
    let native = unsafe { edit.CurrentNativeWindowHandle() }.unwrap_or_default();
    if native.is_invalid() {
        warn!("fallback confirm skipped: filename edit has no native window handle");
        return;
    }
    unsafe {
        let _ = SendMessageW(native, WM_KEYDOWN, Some(WPARAM(VK_RETURN)), Some(LPARAM(0)));
        let _ = SendMessageW(native, WM_KEYUP, Some(WPARAM(VK_RETURN)), Some(LPARAM(0)));
    }
    info!("Committed via SendMessageW(WM_KEYDOWN, Enter).");
}
```

**注**: 简化方案 — `commit_via_uia` 不要求传 dialog_root,而是把 best_button 通过返回值传出。详见下方 **方案简化**(可在 Step 1 用更优雅的 refactor)。

#### 改动 D:加 `IUIAutomationInvokePattern` import

**改动**:`uia_inject.rs` 行 50-55 import 段

```rust
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement,
    IUIAutomationInvokePattern, IUIAutomationValuePattern,
    TreeScope_Descendants, UIA_ButtonControlTypeId,
    UIA_ComboBoxControlTypeId, UIA_EditControlTypeId,
    UIA_InvokePatternId, UIA_ValuePatternId,
};
```

(原本已 import InvokePattern,确认即可)

#### 改动 E:添加单元测试

**文件**:`uia_inject.rs` 文件末尾追加(已有的 `#[cfg(test)] mod tests` 块)

```rust
// 已有测试:
// basename_top_level
// basename_nested
// basename_trailing_separator_trimmed
// basename_no_separator_returns_input
// basename_forward_slash_supported
// basename_drive_root_only_returns_empty  ← Mavis review §1.1 指出断言错,改 assert_eq!(basename(r"C:\"), "C:");

// 新增:
// 不需要 mock UIA,因为 commit_via_uia 需要真实 dialog。
// 单元测试只覆盖 basename() 纯函数 + 未来加 UIA integration test 跳过。
```

**验收**:`cargo test uia_inject::` PASS(Mavis §1.1 必修:测试断言修正)。

### Task 4: 修改 `state.rs` — 加 committed_hwnds 字段

**文件**:`sigma-listary-spike/src/state.rs`

**改动前**(行 14-50):
```rust
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct AppState {
    pub current_path: Mutex<String>,
    pub registry: Mutex<HashMap<u32, DialogInfo>>,
    pub startup_time: Instant,
    pub last_ping_received_ts: AtomicU64,
    pub last_dialog_event_ts: AtomicU64,
    pub uia_healthy: AtomicBool,
    pub session_id: u32,
}

impl AppState {
    pub fn new(initial_path: String, session_id: u32) -> Self {
        Self {
            current_path: Mutex::new(initial_path),
            registry: Mutex::new(HashMap::new()),
            startup_time: Instant::now(),
            last_ping_received_ts: AtomicU64::new(0),
            last_dialog_event_ts: AtomicU64::new(0),
            uia_healthy: AtomicBool::new(true),
            session_id,
        }
    }
    // ... 其他方法 ...
}
```

**改动后**:
```rust
use std::collections::{HashMap, HashSet};
// ... 其他 import ...

#[derive(Debug)]
pub struct AppState {
    pub current_path: Mutex<String>,
    pub registry: Mutex<HashMap<u32, DialogInfo>>,
    pub startup_time: Instant,
    pub last_ping_received_ts: AtomicU64,
    pub last_dialog_event_ts: AtomicU64,
    pub uia_healthy: AtomicBool,
    pub session_id: u32,
    // focus-18: per-dialog single-shot commit dedup.
    // 写入语义:try_mark_committed(hwnd) → true 表示首次,false 表示已 commit 过。
    // clear_committed(hwnd) → DialogEvent::Closed 时调用,允许下次 dialog commit。
    pub committed_hwnds: Mutex<HashSet<u32>>,
}

impl AppState {
    pub fn new(initial_path: String, session_id: u32) -> Self {
        Self {
            // ... 原有字段 ...
            committed_hwnds: Mutex::new(HashSet::new()),
        }
    }

    // ... 其他方法 ...

    /// focus-18: 原子地"标记 + 报告是否首次"。返回 true 表示 hwnd 未曾 commit 过
    /// (本次可 commit),返回 false 表示已 commit 过(本次跳过 commit)。
    pub fn try_mark_committed(&self, hwnd: u32) -> bool {
        let mut set = self.committed_hwnds.lock().unwrap_or_else(|e| e.into_inner());
        set.insert(hwnd)  // HashSet::insert 返回 bool:newly inserted = true
    }

    /// focus-18: 在 DialogEvent::Closed 时调用,清除 hwnd 的 commit 标记,
    /// 下次相同 hwnd (或新 dialog 用相同 hwnd)可重新 commit。
    pub fn clear_committed(&self, hwnd: u32) {
        let mut set = self.committed_hwnds.lock().unwrap_or_else(|e| e.into_inner());
        set.remove(&hwnd);
    }
}
```

**新增单元测试**(在 `state.rs` 已有 `mod tests` 块末尾追加):
```rust
#[test]
fn try_mark_committed_returns_true_first_time() {
    let state = AppState::new("C:\\".into(), 1);
    assert!(state.try_mark_committed(12345));
    assert!(state.try_mark_committed(67890));
}

#[test]
fn try_mark_committed_returns_false_subsequent_calls() {
    let state = AppState::new("C:\\".into(), 1);
    assert!(state.try_mark_committed(12345));
    assert!(!state.try_mark_committed(12345));
    assert!(!state.try_mark_committed(12345));
}

#[test]
fn clear_committed_resets_state() {
    let state = AppState::new("C:\\".into(), 1);
    assert!(state.try_mark_committed(12345));
    assert!(!state.try_mark_committed(12345));
    state.clear_committed(12345);
    assert!(state.try_mark_committed(12345));  // reset 后又可 commit
}
```

**验收**:`cargo test state::` PASS(原有测试 + 3 个新测试)。

### Task 5: 修改 `http_server.rs` — 解析 body.auto_confirm + 调新 writer 签名

**文件**:`sigma-listary-spike/src/http_server.rs`

**改动 A**:`SetPathBody` 加 `auto_confirm` 字段

**改动前**(行 32-35):
```rust
#[derive(Debug, Deserialize)]
struct SetPathBody {
    path: String,
}
```

**改动后**:
```rust
#[derive(Debug, Deserialize)]
struct SetPathBody {
    path: String,
    /// focus-18: 来自 extension PUSH body。默认 false (serde default)。
    /// true 表示 extension 用户启用了 Auto-Confirm,sidecar 可 Invoke Save。
    #[serde(default)]
    auto_confirm: bool,
}
```

**改动 B**:`handle_set_path` 调 writer 时传 `auto_confirm`

**改动位置**:行 188-194 范围

**改动前**:
```rust
for d in dialogs {
    log_event(&Event::SpikeError {
        kind: "set_path_iter".into(),
        message: format!("hwnd={} app={}", d.hwnd, d.app),
        ts: now_iso8601(),
    });
    let outcome = crate::writer::write_path(d.hwnd as isize, &path);
    // ...
}
```

**改动后**:
```rust
for d in dialogs {
    log_event(&Event::SpikeError {
        kind: "set_path_iter".into(),
        message: format!("hwnd={} app={} auto_confirm={}", d.hwnd, d.app, body.auto_confirm),
        ts: now_iso8601(),
    });
    let outcome = crate::writer::write_path(&state, d.hwnd as isize, &path, body.auto_confirm);
    // ...
}
```

**验收**:`cargo test http_server::` 已有测试 + 现有 23 个测试仍 PASS。

### Task 6: 修改 `extension/dist/index.js` — 加 Auto-Confirm toggle

**文件**:`release/extension/dist/index.js`

**改动 A**:加 `autoConfirm` 全局 state

**改动前**(行 24):
```js
let enabled = true;
let pushTimer = null;
let healthCheckTimer = null;
let notifiedSinceLastFailure = false;
```

**改动后**:
```js
let enabled = true;
let autoConfirm = false;  // focus-18: default OFF (safe)
let pushTimer = null;
let healthCheckTimer = null;
let notifiedSinceLastFailure = false;
```

**改动 B**:activate 中读 storage

**改动前**(行 171):
```js
try { enabled = (await sigma.storage.get('enabled')) ?? true; } catch (e) { enabled = true; }
```

**改动后**:
```js
try { enabled = (await sigma.storage.get('enabled')) ?? true; } catch (e) { enabled = true; }
try { autoConfirm = (await sigma.storage.get('autoConfirm')) ?? false; } catch (e) { autoConfirm = false; }
```

**改动 C**:`pushNow` body 加 `auto_confirm` 字段

**改动前**(行 54-59):
```js
const r = await sigma.http.request({
    url: `${SIDECAR_HTTP}/set_path`,
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ path: normalized }),
});
```

**改动后**:
```js
const r = await sigma.http.request({
    url: `${SIDECAR_HTTP}/set_path`,
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ path: normalized, auto_confirm: autoConfirm }),
});
```

**改动 D**:加 `toggleAutoConfirm` 函数

**改动位置**:在 `toggle` 函数后追加

```js
async function toggleAutoConfirm() {
    try {
        autoConfirm = !autoConfirm;
        await sigma.storage.set('autoConfirm', autoConfirm);
        try {
            sigma.ui.showNotification({
                title: 'Focus Sync',
                description: 'Auto-Confirm: ' + (autoConfirm ? 'ON (next push will auto-save)' : 'OFF (you click Save manually)'),
                type: autoConfirm ? 'warning' : 'info',
                duration: 5000,
            });
        } catch (e) {}
    } catch (e) {}
}
```

**改动 E**:registerCommand + registerDropdown

**改动前**(行 194-209):
```js
sigma.commands.registerCommand({ id: 'focus-sync.syncNow', title: 'Sync Now' }, syncNow);
sigma.commands.registerCommand({ id: 'focus-sync.toggle', title: 'Enable / Disable' }, toggle);
// ...
sigma.toolbar.registerDropdown({
    id: 'focus-sync.toolbar',
    title: 'Focus Sync',
    icon: 'mdi-target',
    items: [
        { id: 'syncNow', title: 'Sync Now', commandId: 'focus-sync.syncNow' },
        { id: 'separator', title: '', separator: true },
        { id: 'toggle', title: 'Enable / Disable', commandId: 'focus-sync.toggle' }
    ]
}, { syncNow, toggle });
```

**改动后**:
```js
sigma.commands.registerCommand({ id: 'focus-sync.syncNow', title: 'Sync Now' }, syncNow);
sigma.commands.registerCommand({ id: 'focus-sync.toggle', title: 'Enable / Disable' }, toggle);
sigma.commands.registerCommand({ id: 'focus-sync.toggleAutoConfirm', title: 'Toggle Auto-Confirm' }, toggleAutoConfirm);
// ...
sigma.toolbar.registerDropdown({
    id: 'focus-sync.toolbar',
    title: 'Focus Sync',
    icon: 'mdi-target',
    items: [
        { id: 'syncNow', title: 'Sync Now', commandId: 'focus-sync.syncNow' },
        { id: 'separator', title: '', separator: true },
        { id: 'toggle', title: 'Enable / Disable', commandId: 'focus-sync.toggle' },
        { id: 'separator', title: '', separator: true },
        { id: 'toggleAutoConfirm', title: 'Auto-Confirm (currently: ' + (autoConfirm ? 'ON' : 'OFF') + ')', commandId: 'focus-sync.toggleAutoConfirm' }
    ]
}, { syncNow, toggle, toggleAutoConfirm });
```

**注**:toolbar title 不能动态更新(Sigma FM 在 registerDropdown 时冻结)。用户需要重新打开 toolbar dropdown 看新状态。如果 Sigma FM API 支持 update,可加 update 调用(待验证)。

**验收**:Sigma FM 重启后 toolbar 有 "Auto-Confirm" 菜单项,点击切换有 notification。

### Task 7: 修改 `register.ps1` — 加 toggleAutoConfirm command

**文件**:`release/extension-installer/register.ps1`

**改动 A**:加 command

**改动位置**:行 117-131 范围($kizemoManifest contributes.commands)

**改动前**:
```powershell
contributes  = [ordered]@{
    commands = @(
        [ordered]@{
            id          = 'focus-sync.syncNow'
            title       = 'Focus Sync — Sync current path to all open dialogs'
            description = "Push Sigma's current folder to all detected file dialogs (Chrome / Word / VS Code / etc.)"
            icon        = 'mdi-target'
        },
        [ordered]@{
            id          = 'focus-sync.toggle'
            title       = 'Focus Sync — Enable / Disable'
            description = 'Toggle focus sync on/off (sidecar stays running but stops pushing paths when disabled)'
            icon        = 'mdi-pause-circle'
        }
    )
```

**改动后**:
```powershell
contributes  = [ordered]@{
    commands = @(
        [ordered]@{
            id          = 'focus-sync.syncNow'
            title       = 'Focus Sync — Sync current path to all open dialogs'
            description = "Push Sigma's current folder to all detected file dialogs (Chrome / Word / VS Code / etc.)"
            icon        = 'mdi-target'
        },
        [ordered]@{
            id          = 'focus-sync.toggle'
            title       = 'Focus Sync — Enable / Disable'
            description = 'Toggle focus sync on/off (sidecar stays running but stops pushing paths when disabled)'
            icon        = 'mdi-pause-circle'
        },
        [ordered]@{
            id          = 'focus-sync.toggleAutoConfirm'
            title       = 'Focus Sync — Toggle Auto-Confirm'
            description = "Toggle auto-confirm mode (default OFF). When ON, the sidecar auto-clicks Save once per dialog session — useful when you want Sigma FM path changes to immediately save."
            icon        = 'mdi-flash-auto'
        }
    )
```

**Step 2**: 备份 register.ps1

```powershell
cp "F:/soft/00selfmade/filemanager/release/extension-installer/register.ps1" "C:/Temp/register.ps1.bak-focus18"
```

**验收**:PowerShell 解析 OK(用 Mavis review §2.7 的 PowerShell heredoc 测试方法)。

### Task 8: Build + 集成测试 + Batch Commit

**Step 1**:重编 spike

```bash
cd F:/soft/00selfmade/filemanager/sigma-listary-spike
cargo test        # 全部 PASS(原 23 + 新 ~10)
cargo build --release
```

**Step 2**:build installer

```bash
cd F:/soft/00selfmade/filemanager
powershell -NoProfile -File sigma-file-manager/scripts/build-with-sidecar.ps1
cd sigma-file-manager
npm run tauri build
```

产出:`release/Sigma-File-Manager-2.2.0-focus-18-setup.exe`

**Step 3**:用户安装 + 集成测试

| # | App | Dialog 类型 | 期望(Auto-Confirm OFF) | 期望(Auto-Confirm ON) |
|---|---|---|---|---|
| 1 | Edge Save As (WinUI) | ItemsView / Save 按钮 id='1' | filename 框显示 `C:\downloads\` (full path),不 Invoke | filename 框显示 `C:\downloads\` + 自动点 Save |
| 2 | Chrome Save As | Chromium `Chrome_WidgetWin_*` | filename 框显示 full path(通过 41477 优先),不 Invoke | + Invoke Save |
| 3 | 影刀 RPA Save As | 原生 `#32770` IFileDialog | COM SetFolder 跳到 `C:\downloads`,无 filename 写入 | 同 OFF(已 commit by SetFolder) |
| 4 | Notepad Save As | 原生 `#32770` IFileDialog | COM SetFolder 跳 | 同 OFF |
| 5 | Word Save As | 原生 `#32770` IFileDialog | COM SetFolder 跳 | 同 OFF |
| 6 | VS Code Save As | 原生 `#32770` IFileDialog | COM SetFolder 跳 | 同 OFF |
| 7 | Firefox Save As | Mozilla 自绘 | (需 spike.log 调查,暂列) | (需 spike.log 调查,暂列) |

**Step 4**:9-app 矩阵(简化为 4 类)
- 原生 IFileDialog(影刀 / Notepad / Word / VS Code)— 全部 OK
- Chromium / Edge WinUI Save As — 重点测试 Auto-Confirm 行为
- 自动化验证:只需 1-2 个手动 case,其余 trust 7-app matrix 走覆盖

**Step 5**:batch commit

```bash
cd F:/soft/00selfmade/filemanager
git add \
  sigma-listary-spike/src/fg_bypass.rs \
  sigma-listary-spike/src/writer.rs \
  sigma-listary-spike/src/uia_inject.rs \
  sigma-listary-spike/src/state.rs \
  sigma-listary-spike/src/http_server.rs \
  sigma-listary-spike/src/lib.rs \
  sigma-listary-spike/src/main.rs \
  sigma-listary-spike/Cargo.lock \
  release/extension/dist/index.js \
  release/extension-installer/register.ps1
git diff --cached --stat
git commit -m "$(cat <<'EOF'
fix(focus-18): restore PathWrap path activation with Auto-Confirm toggle

Regressions from focus-16 + focus-17:
- Removed fg_bypass::run_with_bypass wrapper (Chromium focus steal)
- Removed InvokePattern::Invoke commit step
- Removed SendMessageW Enter fallback
- Wrote basename only (instead of FULL path) — broke Chromium/WinUI
  Save As dialog navigation

Fix:
- Restore fg_bypass from git history (commit 905db31)
- Restore Invoke + Enter commit steps in uia_inject.rs
- Write FULL path to filename box (Chromium/WinUI Save As interprets
  as save-to-path navigation)
- Add per-dialog single-shot commit dedup (state.try_mark_committed)
- Add Auto-Confirm toolbar toggle (extension + register.ps1):
  - Default OFF: write path only (safe)
  - Opt-in ON: path + Invoke Save (per-dialog single-shot)
- Extend SetPathBody to carry auto_confirm flag

Per docs/superpowers/plans/2026-10-01-focus-18-path-b-auto-confirm-toggle-plan.md
and 2026-10-01-focus-18-architecture-regression-review.md.

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
git tag -a v0.3.1 -m "focus-sync v0.3.1: PathWrap activation restored with Auto-Confirm toggle"
```

---

## 5. 风险汇总

| # | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| R1 | Auto-Confirm ON 时,user 改 filename 后又被 Invoke Save 覆盖 | 中 | UX 差 | per-dialog single-shot,首次后不再 Invoke |
| R2 | Auto-Confirm ON 时,Chromium 41477 + Invoke 触发导航失败 | 低 | dialog 不动 | fallback 到 filename edit + Invoke |
| R3 | fg_bypass 对 WinUI dialog 副作用 | 低 | 焦点错乱 | `is_chromium_target_window` 已 gated |
| R4 | Sigma FM normalize 在重启后覆盖 register.ps1 的 `toggleAutoConfirm` command | 中 | toolbar 缺菜单项 | 该 command 不需新 settings 字段,risk 低;实测验证 |
| R5 | body 缺 `auto_confirm` 字段时 serde default 是否正确 | 低 | body 解析失败 | `#[serde(default)]` + `bool` default `false`,但**单元测试**验证 |
| R6 | toolbar title 不能动态更新,用户看不到 Auto-Confirm 当前状态 | 中 | UX 差 | 用 notification + 用户记住自己设的状态 |
| R7 | extension 修改后,Sigma FM 已加载的 extension cache 不刷新 | 中 | 用户看不到新 toolbar | uninstall + reinstall(已在 installer 流程) |
| R8 | per-dialog single-shot dedup 在 Chromium dialog flicker 时丢失 commit 状态 | 低 | 二次 commit 误触发 | 主路径 monitor emit Closed 不可靠(round 19c),接受 |
| R9 | fg_bypass 从 git history 恢复后,license header 与当前不匹配 | 中 | legal 不合规 | 恢复时显式保留原 header + 加 focus-18 commit ref |
| R10 | multiple dialogs 同时打开,dedup 状态跨 dialog 串扰 | 低 | 误 commit | `committed_hwnds` 是 hwnd-keyed,无串扰 |

---

## 6. 不在本次范围

- `extension/dist/index.js` stale 通知清除(Mavis 原 §4.3)— 接受
- `dirname` 函数清理 — 建议删,但非阻塞
- `extension/locales/*.json` 翻译
- `FORK-KEEP-LIST.md` / memory `sigma_fm_extension_registration.md` 更新
- 注册表 `focus-sync.autoConfirm` 持久化(用 sigma.storage 即可,无需注册表)
- v0.3.1 升级路径(覆盖安装)— 默认 Auto-Confirm OFF,Sigma FM 需重启,无副作用
- Chromium 41477 address bar 的 Enter 触发(目前 SetValue 不 Enter)— 需 spike.log 二次验证

---

## 7. 完成判据(Verification Checklist)

**必须满足**(Claude Code 自验 + 用户验收):

- [ ] `cargo test` 全部 PASS(原 23 + 新增 logger 2 + basename 6 + state 3 + writer 2 ≈ 36 个)
- [ ] `cargo build --release` 产出 spike.exe,无 compile warning
- [ ] Tauri build 产出 `release/Sigma-File-Manager-2.2.0-focus-18-setup.exe`
- [ ] 安装 focus-18:无 console 弹窗(沿用 focus-17 #1 修复)
- [ ] `/health` 返回 mode=service / session_id=1 / status=ok
- [ ] 装后 Sigma FM 重启,toolbar 有 "Auto-Confirm" 菜单项(默认 OFF)
- [ ] **Auto-Confirm OFF 测试**:Edge Save As → filename 框显示 `C:\downloads\`,dialog 不动,用户手动 Save → 保存到 `C:\downloads\`
- [ ] **Auto-Confirm ON 测试**:Edge Save As → filename 框显示 `C:\downloads\` + 自动 Invoke Save → 文件自动保存
- [ ] **per-dialog dedup**:开 Auto-Confirm 后,Sigma FM 浏览 A→B→C,只第一次 push 触发自动保存(A),B/C 写 path 但不 Invoke
- [ ] **原生 IFileDialog 测试**(影刀 / Word / Notepad / VS Code):SetFolder 跳地址栏,与 Auto-Confirm 状态无关
- [ ] **Chromium 测试**(Chrome Save As):41477 路径 + Invoke,Auto-Confirm ON 时触发导航
- [ ] git log 干净,只 1 个 commit + tag v0.3.1

---

## 8. 给用户(转交前的检查清单)

1. **本文档是给 Claude Code 执行的最终 plan**。Claude Code 拿到后应能 step-by-step 执行,无需再问澄清问题。
2. **预计改动**:10 个文件(1 新建、9 修改)+ 1 新依赖不变,代码量 ~250 行(含测试)。
3. **预计时间**:从开始到 focus-18 installer 产出 ~60 分钟(增量 vs focus-17 主要在恢复代码 + 测试)。
4. **关键风险已标注**(§5 R1-R10),Claude Code 在每个 Task 完成后应交叉验证。
5. **如有疑问**:Claude Code 可随时 ping 我(Mavis)确认细节,文档已尽可能自包含。
6. **不要求 Claude Code 提交 git**(用户曾说"测试通过再 commit")— Task 8 才 batch commit。
7. **验证清单(§7)**11 项必须全 PASS 才算 v0.3.1 准备就绪。

**用户请确认**:本文档是否需要补充 / 修改?如果 OK,请转交 Claude Code 开始 Phase 1(Task 1 恢复 fg_bypass)。
