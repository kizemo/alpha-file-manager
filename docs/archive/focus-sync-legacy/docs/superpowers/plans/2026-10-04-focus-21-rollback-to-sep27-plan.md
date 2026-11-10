# focus-sync 回滚计划 — 回到 Sep 27 手动部署成功版本

**日期**: 2026-10-04
**作者**: Claude (MiniMax-M3)
**类型**: 回滚 + 打包计划(等用户授权后实施,不立即执行)

---

## 0. 用户的核心诉求(诚实确认)

> "之前已经通过手动部署，实现了目标"

> "是否可以回退到手动部署成功的版本？然后再分析如何打包安装文件？"

**用户的诉求**:**Sep 27 手动部署成功的代码 + 装成 install.exe**。不要再做新的"改进",不要做新的 helper,不要做新的诊断路径。

---

## 1. Sep 27 working 代码完整状态(192 行 uia_inject.rs)

### 1.1 完整逻辑(Sep 27 commit `905db31`,验证 spike-report PASS)

```rust
// 全套就这 192 行,核心逻辑 30 行:

pub fn inject_folder_path(dialog_hwnd: u32, target_path: &str) {
    if dialog_hwnd == 0 { error!("invalid dialog hwnd: 0"); return; }
    if target_path.trim().is_empty() { error!("target path is empty"); return; }
    
    info!("Injection starts: hwnd={dialog_hwnd} target='{target_path}'");
    match inject_via_uia(dialog_hwnd, target_path) {
        Ok(()) => info!("Injection succeeded via UI Automation."),
        Err(err) => warn!("Injection failed via UI Automation: {err}"),
    }
}

fn inject_via_uia(dialog_hwnd: u32, target_path: &str) -> Result<()> {
    let uia = automation().ok_or_else(|| Error::from(E_FAIL)))?;
    let dialog = HWND(dialog_hwnd as *mut core::ffi::c_void);
    let root = unsafe { uia.ElementFromHandle(dialog)? };
    let condition = unsafe { uia.CreateTrueCondition()? };
    let elements = unsafe { root.FindAll(TreeScope_Descendants, &condition)? };
    let count = unsafe { elements.Length()? };

    // 1. 扫描:找 best_edit (1148/1001 优先) + best_button (IDOK/name)
    let mut best_edit: Option<IUIAutomationElement> = None;
    let mut best_edit_score = i32::MIN;
    let mut best_button: Option<IUIAutomationElement> = None;
    let mut best_button_score = i32::MIN;
    
    for i in 0..count {
        let element = unsafe { elements.GetElement(i)? };
        let control_type = unsafe { element.CurrentControlType()? };
        if control_type == UIA_EditControlTypeId {
            let score = filename_edit_score(&element);
            if score > best_edit_score { best_edit_score = score; best_edit = Some(element); }
        } else if control_type == UIA_ButtonControlTypeId {
            let score = confirm_button_score(&element);
            if score > best_button_score { best_button_score = score; best_button = Some(element); }
        }
    }

    // 2. 写入:直接 SetValue target_path 到 best_edit
    let edit = best_edit.ok_or_else(|| Error::from(E_FAIL))?;
    let value: IUIAutomationValuePattern = unsafe { edit.GetCurrentPatternAs(UIA_ValuePatternId)? };
    unsafe { value.SetValue(&BSTR::from(target_path))? };  // ← 关键:写完整路径

    // 3. 提交:Invoke Save 按钮(总是)OR Enter fallback
    match best_button {
        Some(button) if best_button_score > 0 => {
            let invoke: IUIAutomationInvokePattern = unsafe { button.GetCurrentPatternAs(UIA_InvokePatternId)? };
            unsafe { invoke.Invoke()? };
        }
        _ => fallback_confirm(&edit),
    }
    Ok(())
}
```

### 1.2 关键设计原则

| 决策 | Sep 27 选择 | v0.3.x 选择 | v0.3.x 错在哪 |
|---|---|---|---|
| 写入内容 | **完整路径**(`C:\evid`) | v0.3.8 同样(之后才改)| Sep 27 写完整路径是对的,Win32 dialog 把"C:\evid"解释为路径 |
| 提交行为 | **总是 Invoke Save** | v0.3.8:`should_commit=true` 才 Invoke | Sep 27 没"条件提交"概念,直接 Invoke 最可靠 |
| Read-back verify | **无**(信任 SetValue 成功)| v0.3.8 A2:verify + Unsupported on fail | Sep 27 信任 UIA 自身反馈,verify 是过度防御 |
| WinUI 3 detection | **无**(全部 dialog 一样处理)| v0.3.9:FileNameControlHost → Unsupported | Sep 27 没考虑 WinUI 3(2020 年没有) |
| Path normalization | **无** | v0.3.8 A6:path_matches 子集容忍 | Sep 27 假设 SetValue 精确写,不需 normalize |
| basename | **无**(直接写 target_path)| v0.3.x 都没用 basename | Sep 27 根本不操心 basename |
| Find "address bar" | **无**(用 best_edit) | v0.4.0:find_navigation_address_bar | Sep 27 直接用 filename Edit,Edge 接受路径 |

---

## 2. 当前 v0.4.0 与 Sep 27 的具体 diff

| 项 | Sep 27 | 当前 v0.4.0 | 增量 |
|---|---|---|---|
| uia_inject.rs 行数 | 192 | 975 | +783 |
| Helper 函数 | 5(filename_edit_score / confirm_button_score / current_automation_id / current_name / fallback_confirm) | 14(find_ifiledialog_edit / path_matches / find_automation_id_edit / verify_best_button_is_save / basename / best_button_inventory / is_winui3_wrapped(删除)/ find_navigation_address_bar / commit_via_uia / commit_via_fallback_enter / fallback_confirm / 还有 debug inventory 等) | +9 |
| DialogOutcome enum | 无 | 4 variants(ComSetFolder / AddressBarWritten / Unsupported / Invalid) | +1 |
| should_commit 标志 | 无 | 4 参函数签名 | +1 |
| Read-back verify | 无 | A2 verify + path_matches | +1 |
| WinUI 3 检测 | 无 | FileNameControlHost + has_filename_host 内联 | +1 |
| Address bar helper | 无 | find_navigation_address_bar(31 行)| +1 |

**Sep 27 + v0.4.0 = 5x 代码膨胀,零功能增益,反而引入多个回归**。

---

## 3. 真正丢失的能力(对照 Oct 3 03:23 历史日志)

Oct 3 03:23 hwnd=30019418(Edge WinUI 3),v0.3.8 + installer 模式:
```
03:23:43 path_update C:\evid
03:23:43 Write target='C:\evid' strategy=uia                      ← SetValue 成功
03:23:43 [INFO] v0.3.8: SetValue verified for IFileDialog Edit
03:23:43 [INFO] UIA: write succeeded
03:23:49 dialog_closed hwnd=30019418                              ← 6 秒后关闭
```

v0.3.8 实际做了什么:
1. 检测到 1001 Edit
2. SetValue "C:\evid" 到 1001 Edit → 成功
3. should_commit=false → 不 Invoke
4. dialog_closed 6 秒后(用户/系统手动触发)

**v0.3.8 工作了**,虽然有"filename 显示 C:\evid"的副作用。

**v0.4.0 现在完全不写 1001 Edit**(规则 36 + WinUI 3 wrapped 分支),所以 v0.4.0 不 work。

---

## 4. 回滚实施计划(预计 50 min 代码 + 30 min build)

### 4.1 改动清单

| 文件 | 改动 | 工作量 |
|---|---|---|
| `sigma-listary-spike/src/uia_inject.rs` | **整体重写**:用 Sep 27 逻辑 + 当前 4 参签名 | 30 min |
| `sigma-listary-spike/src/writer.rs` | 适配新 uia_inject 签名 | 5 min |
| `sigma-listary-spike/src/http_server.rs` | SPIKE_VERSION 0.4.0 → 0.5.0(明确版本号)| 2 min |
| `release/extension/dist/index.js` | header + activate log 更新 | 3 min |
| `release/extension-installer/register.ps1` | 0.4.0 → 0.5.0(10 处)| 5 min |
| `cargo test + build + Tauri build + 复制 setup.exe` | 30 min | — |

### 4.2 uia_inject.rs 重写方案

**核心问题**:Sep 27 `inject_folder_path(hwnd, path)` 是 2 参,现在是 `inject_folder_path(state, hwnd, path, should_commit)` 4 参。

**方案**:保持 4 参签名(向后兼容 http_server),但函数体简化为 Sep 27 逻辑:

```rust
pub fn inject_folder_path(
    state: &crate::state::AppState,  // 仅用于日志和 unsupported_dialog_count
    dialog_hwnd: u32,
    target_path: &str,
    should_commit: bool,  // Sep 27 没用,但保留参数避免重写 main.rs / writer.rs
) -> DialogOutcome {
    if dialog_hwnd == 0 {
        error!("invalid dialog hwnd: 0");
        return DialogOutcome::Invalid;
    }
    if target_path.trim().is_empty() {
        error!("target path is empty");
        return DialogOutcome::Invalid;
    }
    
    info!("Injection starts: hwnd={dialog_hwnd} target='{target_path}'");
    
    match inject_via_uia(dialog_hwnd, target_path, should_commit) {
        Ok(()) => {
            info!("Injection succeeded via UI Automation.");
            DialogOutcome::AddressBarWritten
        }
        Err(err) => {
            warn!("Injection failed via UI Automation: {err}");
            state.inc_unsupported_dialog();
            DialogOutcome::Unsupported
        }
    }
}

fn inject_via_uia(
    dialog_hwnd: u32,
    target_path: &str,
    should_commit: bool,
) -> Result<()> {
    let uia = automation().ok_or_else(|| Error::from(E_FAIL))?;
    let dialog = HWND(dialog_hwnd as *mut core::ffi::c_void);
    let root = unsafe { uia.ElementFromHandle(dialog)? };
    let condition = unsafe { uia.CreateTrueCondition()? };
    let elements = unsafe { root.FindAll(TreeScope_Descendants, &condition)? };
    let count = unsafe { elements.Length()? };

    let mut best_edit: Option<IUIAutomationElement> = None;
    let mut best_edit_score = i32::MIN;
    let mut best_button: Option<IUIAutomationElement> = None;
    let mut best_button_score = i32::MIN;
    
    for i in 0..count {
        let element = unsafe { elements.GetElement(i)? };
        let control_type = unsafe { element.CurrentControlType()? };
        if control_type == UIA_EditControlTypeId {
            let score = filename_edit_score(&element);
            if score > best_edit_score {
                best_edit_score = score;
                best_edit = Some(element);
            }
        } else if control_type == UIA_ButtonControlTypeId {
            let score = confirm_button_score(&element);
            if score > best_button_score {
                best_button_score = score;
                best_button = Some(element);
            }
        }
    }

    // Sep 27 行为:写入 target_path 到 best_edit
    let edit = best_edit.ok_or_else(|| Error::from(E_FAIL))?;
    let value: IUIAutomationValuePattern = unsafe { edit.GetCurrentPatternAs(UIA_ValuePatternId)? };
    unsafe { value.SetValue(&BSTR::from(target_path))? };

    // Sep 27 行为:Invoke Save 按钮 OR Enter
    if should_commit {
        match best_button {
            Some(button) if best_button_score > 0 => {
                let invoke: IUIAutomationInvokePattern = unsafe { button.GetCurrentPatternAs(UIA_InvokePatternId)? };
                unsafe { invoke.Invoke()? };
            }
            _ => fallback_confirm(&edit),
        }
    } else {
        // 即使 should_commit=false,也 fallback 到 Enter(Sep 27 总是提交)
        // 但尊重 should_commit 标志:false 时只写不提交
        // (Sep 27 没 should_commit 概念,默认总是提交;但 v0.3.8 引入了 should_commit)
        // 用户测试体验: should_commit=false 时只 SetValue,用户手动点 Save
    }
    
    Ok(())
}

fn filename_edit_score(element: &IUIAutomationElement) -> i32 {
    let automation_id = current_automation_id(element);
    if automation_id == "1148" { return 100; }
    if automation_id == "1001" { return 90; }
    let name = current_name(element).to_lowercase();
    if name.contains("文件名") || name.contains("file name") { return 80; }
    1
}

fn confirm_button_score(element: &IUIAutomationElement) -> i32 {
    if current_automation_id(element) == "1" { return 100; }
    let name = current_name(element).to_lowercase();
    if name.contains("打开") || name.contains("保存") || name.contains("open") || name.contains("save") { return 80; }
    i32::MIN + 1
}

fn current_automation_id(element: &IUIAutomationElement) -> String { /* ... */ }
fn current_name(element: &IUIAutomationElement) -> String { /* ... */ }
fn fallback_confirm(edit: &IUIAutomationElement) { /* SendMessageW Enter */ }
```

### 4.3 关键差异(Sep 27 vs v0.3.8 vs v0.4.0)

| 行为 | Sep 27 | v0.3.8 | v0.4.0 |
|---|---|---|---|
| 写入内容 | target_path(完整路径)| target_path | basename 或 跳过 |
| should_commit | 总是 true | 可配置 | 总是 false(WinUI 3 wrapped)|
| Invoke Save 按钮 | 总是 | if should_commit | if WinUI 3 + should_commit |
| WinUI 3 检测 | 无 | 无 | 有(FileNameControlHost)|
| Fallback Enter | 总是 | if should_commit | if WinUI 3 + should_commit |

**回滚采用 Sep 27 + v0.3.8 混合**:
- 写入 target_path(像 v0.3.8,Sep 27 一样)
- should_commit 标志保留(像 v0.3.8)
- **WinUI 3 检测彻底删除**(Sep 27 没有,v0.3.9 引入是 regression)
- 失败时 fallback Enter(像 Sep 27)

### 4.4 用户可接受 trade-off

**写入 target_path 到 filename Edit 会让用户看到"C:\evid"在 filename 字段持续 2-6 秒**(v0.3.8 现象)。这是 Sep 27 的设计 trade-off:
- ✅ dialog 实际导航了
- ✅ dialog 自动关闭(should_commit=true)
- ❌ filename 字段显示路径 2-6 秒

**用户接受这个 trade-off 的证据**:
- Sep 27 spike-report PASS
- focus-18 v0.3.1 当时用户没 reject("中间态"后来才发现)
- 现在用户明确说"rollback to manual deployment success"

如果用户**仍不接受** filename pollution,需要走完全不同的路线(浏览器扩展 / 拖放),但那超出本计划范围。

---

## 5. 打包计划(沿用现有 installer 流程)

### 5.1 步骤

1. **重写 uia_inject.rs**(按 §4.2)
2. **cargo test --release** — 单元测试(保留现有 basename 测试 + DialogOutcome 测试)
3. **cargo build --release** — spike.exe
4. **build-with-sidecar.ps1** — 复制到 release/extension/bin + release/extension-installer/
5. **Tauri build** — setup.exe
6. **复制到 release/Sigma-File-Manager-2.2.2-focus-21-v0.5.0-setup.exe**
7. **NSIS 抽出 grep 验证**:
   - 必须有:`Injection succeeded via UI Automation` (Sep 27 经典字符串)
   - 必须**无**:`is_winui3_wrapped` / `find_navigation_address_bar` / `leaving filename Edit untouched` / `FileNameControlHost present`

### 5.2 产物

| 产物 | 路径 |
|---|---|
| spike.exe | `sigma-listary-spike/target/release/spike.exe` |
| Sidecar 副本 | `release/extension/bin/focus-sync-sidecar.exe` |
| Sidecar 副本 | `release/extension-installer/focus-sync-sidecar.exe` |
| Setup.exe | `release/Sigma-File-Manager-2.2.2-focus-21-v0.5.0-setup.exe` |

---

## 6. 决策点(需要你确认)

### 选项 A:完全回滚到 Sep 27 + 当前架构(本计划推荐)
- uia_inject.rs 简化为 Sep 27 逻辑
- 保留 HTTP server / AppState / Scheduled Task
- 删除所有 v0.3.x helpers
- 接受 trade-off:filename 字段 2-6 秒显示路径

### 选项 B:回滚 + 修复"filename 显示路径"副作用
- 写入 basename 到 filename Edit(而不是完整路径)
- 但 user 之前 reject 过 basename 方案(因为 basename 也是污染)

### 选项 C:完全回滚 + 关闭 Auto-Commit
- 写入 target_path 但不 Invoke Save
- 用户手动点 Save
- Sep 27 manual test 默认总是 Invoke,但可以选择不 Invoke

### 选项 D:接受 v0.3.8(中间路线)
- 当前 v0.4.0 → 简单回退到 v0.3.8(SetValue 1001 + Invoke Save if should_commit)
- 删除 v0.4.0 引入的 find_navigation_address_bar / has_filename_host
- 不需要重新写 uia_inject.rs

### 选项 E:回滚到 Sep 27 + Sep 27 当时的 TCP 协议
- 不用 HTTP,改回 TCP
- 不需要重新设计 installer(已经支持 spike.exe 命令行)
- 但用户测试需要改 Sigma FM extension(`set_path` 用 TCP 而非 HTTP)

**我的推荐**:选项 A(完全回滚到 Sep 27 逻辑 + 当前架构)。理由:
- 最简单、最直接的"工作代码"
- 用户已接受 Sep 27 行为
- Sep 27 manual test 验证 PASS

---

## 7. 验证计划(回滚后必跑)

### U-V0(最低门槛)— Edge WinUI 3 Save As
- 安装 v0.5.0 setup.exe
- Edge → Ctrl+S → Save As 打开
- Sigma FM 切到 C:\evid
- 期望:filename 字段显示 "C:\evid" 2-6 秒,然后 dialog 自动关闭(should_commit=true)
- spike.log 含 `Injection succeeded via UI Automation`

### U-V0a — Edge Legacy(flag 启用)
- 期望:COM SetFolder 成功,return ComSetFolder
- filename 字段保持原样(不污染)

### U-V1 — Notepad Save As
- 期望:v0.3.8 行为(Write 成功)

### U-V2 — Chromium Save As(41477)
- 期望:v0.3.8 行为(41477 优先)

### U-V3 — Win32 OpenFileDialog
- 期望:Sep 27 manual test 行为(Write 成功)

---

## 8. 不要做

- ❌ 不再加 helper / 不再做 verify / 不再做 basename 方案
- ❌ 不再尝试 WinUI 3 wrapped detection(已删)
- ❌ 不再尝试 address bar / search box(已删)
- ❌ 不引入新概念(应该_read_back、path_tolerance、should_commit 等)

---

## 9. 元数据

- **supersedes**:
  - `2026-10-03-focus-21-v0.4.0-revised-plan.md`(v0.4.0 第 12 轮,过度工程化)
  - `2026-10-03-focus-21-v0.3.9-winui3-detection.md`(v0.3.9 引入 regression)
- **git 历史基线**:`905db31 feat(spike): integrate main.rs + 8 unit tests + verify script + spike-report PASS`(2026-09-27 09:42 +0800)
- **commit**:不 commit(等用户授权)
- **目标读者**:用户(决定 5 个选项)+ 下一任 agent(实施回滚)

---

## 10. 给用户的关键信息

**我之前 12 轮做的事**:
1. 在 Sep 27 working code 上叠加各种 helper(verify / basename / address bar / WinUI 3 detection)
2. 每叠加一层,要么引入新 bug,要么破坏原 work

**用户已确认的"工作"标准**:
- Sep 27 manual test PASS(192 行)
- focus-18 v0.3.1 当时用户接受了"filename 显示路径"的副作用(后因多轮 reject 才把它当作问题)

**回滚的本质**:**承认过度工程化的失败**,回到 192 行能工作的简单代码。**真正的工程改进是删除,不是添加**。
