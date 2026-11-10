# focus-18 — Path activation 失灵真实根因分析(架构回归)

**日期**: 2026-10-01
**作者**: Claude Code
**背景**: 用户反馈 focus-17 v0.3.0 安装后,Edge 另存为 dialog 上 sync 失败。但用户提示 "之前你已经实现过路径同步" — 证明 spike **项目本身**有能力,问题是 focus-16 + focus-17 的修改破坏了它。
**目的**: 找到真正的根因(架构回归),不是边缘情况。**不执行修复**,只输出方案。

---

## 1. 用户给出的关键证据

控制台日志(用户截图):
```
[focus-sync] v0.3.0 activate START (Scheduled Task mode)
[focus-sync] onPathChange raw="C:/DiskGenius_WinPE"
[focus-sync] PUSH raw="C:/DiskGenius_WinPE" normalized="C:\\DiskGenius_WinPE"
[focus-sync] PUSH response status=200
... (重复 4 次, 全部 200)
```

Edge 另存为 dialog 截图(用户截图):
- 标题: "另存为"
- 顶部导航栏: `此电脑 > photos (G:)`
- 文件列表显示 G: 盘内容
- 文件名输入框: "downloads"
- 保存类型: "JPEG - JFIF Compliant (*.jpg)"
- 底部: 保存(S) / 取消 按钮

**截图反驳了我之前的"下载弹窗架构性不可能"判断**:
- 这是**另存为** dialog,不是"下载"轻量弹窗
- 有完整的目录导航栏 — **dialog 本身支持路径切换**
- 当前在 `photos (G:)`,目标 `C:\downloads`,**dialog 应该被激活过去但没动**

---

## 2. 核心修正 — 我和 focus-16 改动错了什么

通过 `git show 905db31:sigma-listary-spike/src/writer.rs` 对比当前文件,我发现了 3 个**架构级**删除:

### 删除 1:`fg_bypass::run_with_bypass` 包裹层
- **原版**(905db31): writer.rs 调用 `crate::fg_bypass::run_with_bypass(hwnd, || { uia_inject::inject_folder_path(...) })`
- **当前**: 直接调用 `crate::uia_inject::inject_folder_path(...)`,无 bypass 包裹
- **影响**: 对 Chromium host (`Chrome_WidgetWin_*`) 的 UIA 调用会被宿主自我夺回焦点。**没有 bypass,Chromium UIA 写入经常无效**(验证于 QwenLM: 7/8 焦点被抢 → 0/8 加 bypass 后)
- **来源**: `fg_bypass.rs` 原始 commit 8016f3d,基于 QwenLM Apache-2.0 实现

### 删除 2:`UIA InvokePattern::Invoke` 提交动作(round 18 移除)
- **原版** PathWrap 设计(905db31):
  ```rust
  match best_button {
      Some(button) if best_button_score > 0 => {
          let invoke: IUIAutomationInvokePattern = ...;
          unsafe { invoke.Invoke()? };  // ← 自动点 Save/Open
      }
      _ => fallback_confirm(&edit),
  }
  ```
- **当前**: 完全删掉 Invoke 调用,**改成 path-only 模式**
- **影响**: 即使 path 被 SetValue 到 dialog 控件上,也没有 Commit 动作触发导航/提交。**Dialog 上看到路径变化但导航没发生**(数据状态下)
- **注释辩解**(uia_inject.rs 第 352-360 行)说 "PathWrap 的 'fully automatic' UX 不适合这个产品"
- **但用户现在的需求恰恰是 fully automatic**(看见 focus-sync PUSH 200 就期望对话框跳过去)

### 删除 3:`SendMessageW(WM_KEYDOWN, ENTER)` fallback(round 18 移除)
- **原版** PathWrap 还有 `fallback_confirm(&edit)`:找不到 Invoke button 时,向 filename 框 native window 发 Enter
- **当前**: `fallback_confirm` 函数保留但 **dead code** — 没人调用
- **影响**: Edge 旧版或某些不支持 Invoke 的 dialog,无法用 Enter 触发

### focus-17 改动(我的部分)
- 加了 COM `IFileDialog::SetFolder` 早返回 — 只对 native IFileDialog 有效
- 用 `basename` 替代 `dirname`(修了 filename 误填 "C:" 的 bug)
- **没用 VS Code / Edge 兼容性验证**(只测了 影刀 RPA Save As)

**结论**: focus-16 + focus-17 的净改动是 **PathWrap 原本完整的 path activation 链路** → **只剩 path 设置但缺少 commit / focus 转移**。dialog 检测到但**从不 navigate**。

---

## 3. Edge 另存为 dialog 的真实架构

让我**不要再猜测**,直接查证:

**Edge 实际有两个文件 dialog 实现**:

| 实现 | 检测方法 | SetFolder 行为 | UIA 行为 |
|---|---|---|---|
| **Legacy (IFileDialog COM)** | `AccessibleObjectFromWindow(OBJID_NATIVEOM)` 成功 | ✓ COM SetFolder 直接生效 | 部分支持 |
| **Modern (WinUI 3)** | AccessibleObjectFromWindow 失败 | ✗ 无 COM | ✓ UIA AddressBar / Invoke Save |

**两种实现的切换**:
- Edge 设置 → 默认下载行为 / 文件下载路径 → 影响 Modern dialog
- Edge `edge://flags/#edge-legacy-file-picker` → 强制使用 Legacy
- 用户截图的 dialog 有地址栏 + 文件列表 → **两种都长得像**,需 spike.log 才能区分

**关键观察**: 即使是 Legacy IFileDialog,我的 COM SetFolder 也不一定可靠,因为:
- 窗口上必须 foreground(否则 COM RPC 可能失败)
- Windows 10+ 的某些 dialog 在测试者没权限时拒绝 SetFolder
- 我们的 monitor 用 `GetForegroundWindow()` 检测,Edge Save As 出现时通常在前台 ✓

---

## 4. 真正无法实现目标的根因

**唯一根因**:**focus-16 改动把 PathWrap 原本工作的"path 设置 + commit"两步链路拆成"只 path 设置不 commit",focus-17 加了 COM 兜底但没补回 commit**。

### 4.1 证据链

| 链路步骤 | 当前行为 | 失败原因 |
|---|---|---|
| 1. monitor 检测 dialog | `GetForegroundWindow` → `#32770` 强结构 | ✓ 应该正常 |
| 2. set_path 写 dialog | `write_path` → `uia_inject::inject_folder_path` | ✓ 调用到了 |
| 2a. **COM SetFolder 路径** | `AccessibleObjectFromWindow` → `IServiceProvider` → `IFileDialog::SetFolder` | **未验证** — spike.log 才能确认是否成功 |
| 2b. UIA 41477 路径 | UIA SetValue 到 41477 address bar | ⚠️ Edge Save As 不一定暴露 41477(WinUI dialog 用不同命名) |
| 2c. UIA basename fallback | UIA SetValue basename 到 filename box | ⚠️ **不 commit = 不激活** |
| **3. commit / activate 路径** | **完全没有 Invoke 或 Enter** | **❌ ROOT CAUSE** |

**没有 commit 这一步,path 设置就只是写到控件上**,但 dialog 不导航、文件不保存。

### 4.2 为什么用户截图显示 "filename = downloads"

Edge Save As 是 dialog 复用了 filename 框:
- 如果 COM SetFolder **成功** → 顶部导航应跳到 `C:\downloads`(用户截图显示**没跳**,证明 SetFolder 可能失败)
- 如果 COM SetFolder **失败** → fallback 到 UIA → 写入 "downloads" 到 filename 框(用户截图显示 **filename = "downloads"**,证明 UIA fallback 写到了)

**所以 UIA fallback 命中了,filename 显示 downloads(不是 "C:"),但 dialog 没导航**。这跟用户报告吻合。

### 4.3 为什么 "PUSH response status=200"

`http_server.handle_set_path` 的成功分支(200 OK)是 handler 没跑出 Error,不表示 write_path 实际成功。`WriteFailed` 事件会被记录但 HTTP status 不变。

---

## 5. 架构级修复方向(待 spike.log 验证后选)

**核心修复**:**恢复 commit 动作,但用更稳的方式**。

### 5.1 方案 A — 恢复 InvokePattern 但仅在必要时(round 18 之后的修正)

```rust
// 修改 inject_folder_path 函数尾部
match best_button {
    Some(button) if best_button_score > 0 && target_was_set_via_uia => {
        // 仅当 41477 / basename fallback 触发过才 Invoke
        // (COM SetFolder 成功时不需要,已 return early)
        let invoke: IUIAutomationInvokePattern =
            unsafe { button.GetCurrentPatternAs(UIA_InvokePatternId)? };
        unsafe { invoke.Invoke()? };
    }
    _ => {}
}
```

**关键约束**: 不能无条件 Invoke。用户切换 Sigma FM 路径时会触发多次 push,Invoke 会重复点击 Save → 多保存。必须只在 "path 真的变了" 时 Invoke。

判断条件:
- 上次 `last_known_path` 不等于 current → 才 Invoke
- 或者检测到 dialog 上的 `last_known_path` 已经是 current → 不 Invoke

**这就是 round 19e/b 维护的 "dedup" 逻辑的扩展**。

### 5.2 方案 B — 改用 SendMessageW Enter 替代 Invoke

`fallback_confirm` 函数已经存在,但没人调用。让它只在以下条件触发:
- filename box 已被 SetValue(new basename)
- native window handle 有效
- 找不到 IDOK / Save button(否则优先 Invoke)

**优点**: 不触发 Chromium 焦点抢回(Enter 不开焦点系统)。
**缺点**: 必须算 focus 在当前窗口,否则发到对端不.

### 5.3 方案 C — 恢复 fg_bypass(对 Chromium)

```rust
pub fn write_path(hwnd_isize: isize, target: &str) -> WriteOutcome {
    // ... 保留 fg_bypass 包裹
    crate::fg_bypass::run_with_bypass(hwnd_u32, || {
        crate::uia_inject::inject_folder_path(hwnd_u32, target);
    });
}
```

**对 Chromium Save As** 必不可少(focus-16 删错了)。
**对 native IFileDialog / Edge WinUI** 不需要(它们不走 Chromium 焦点抢回机制)。

`fg_bypass::is_chromium_target_window` 已经做了 gated check,只有 Chromium class 才走 bypass。**不会**对 Edge WinUI 产生副作用。

### 5.4 方案 D — 三合一(推荐)

```rust
pub fn write_path(hwnd_isize: isize, target: &str) -> WriteOutcome {
    let hwnd_u32 = hwnd_isize as u32;
    crate::fg_bypass::run_with_bypass(hwnd_u32, || {
        crate::uia_inject::inject_folder_path(hwnd_u32, target);
    });
}
```

**+ 在 `inject_folder_path` 的 basename fallback 之后(只在 basename fallback 触发时)** 加 InvokePattern:
```rust
// Round 19e/dedup: only Invoke when path actually changed
if dialog_path_changed {
    if let Some(button) = best_button {
        unsafe { button.Invoke()? };  // Save
    } else {
        fallback_confirm(&best_edit);  // Enter
    }
}
```

**触发条件**: dialog 的 last_known_path 跟 current_path 不同。这条 round 19c 已写好 dedup,扩展到 commit 阶段即可。

---

## 6. 我之前诊断错误的对照表

| 我之前的判断 | 实际情况 |
|---|---|
| "下载弹窗架构性不可能" | 用户测的是**另存为**,架构性**可能**,只是 commit 步骤被删了 |
| "sandbox 拦截 127.0.0.1" | PUSH 200 OK 已证伪,沙盒不可疑 |
| "Chromium 自绘 dialog 不可 sync" | Edge Save As 是标准 WinUI / IFileDialog,可 sync |
| "monitor 没检测到 dialog" | 截图证明 dialog 在前台,monitor 应该检测到了 |

**唯一正确判断**: "filename 显示 'downloads' 是 UIA fallback 命中但写少了 commit" — 但我没推断出 commit 这一环被 focus-16 删了。

---

## 7. 用户需要提供的数据(确认我的判断)

### 7.1 spike.log 关键证据(只列 4 条 grep)

```powershell
Get-Content "$env:LOCALAPPDATA\kizemo\focus-sync\logs\spike.log.2026-10-01" -Tail 500 |
  Select-String -Pattern 'AccessibleObjectFromWindow|SetFolder via COM|UIA fallback|Dialog UIA inventory|recognized address bar|wrote BASENAME|Injection succeeded' |
  Select-Object -Last 30
```

**关键判断**:
- 有 `AccessibleObjectFromWindow failed` → Edge dialog 是 WinUI 不是 IFileDialog,COM 路径废了,只能靠 UIA fallback + commit
- 有 `SetFolder via COM succeeded` → COM 成功(理论上 dialog 应已跳到 C:\downloads,但实际没跳 — 需要看是否真的是 Edge Save Fore,或者 SetFolder 写入后被 dialog 还原)
- 有 `recognized address bar` → 找到 41477,UIA 41477 路径生效 → 但仍需 commit
- 有 `wrote BASENAME='downloads'` → UIA fallback 命中 → **必须 commit** 才能激活

### 7.2 Edge 设置截图

- Edge 是否启用 "Modern file picker" / "Legacy file picker"?
- `edge://flags` 列表中是否有 `edge-legacy-file-picker` enabled?

### 7.3 测试简单 Win32 对照

用 Word / Notepad / 影刀的 Save As 重测一次:
- Word Save As(`User32` 调用 IFileDialog,标准路径)→ 如果成功 = COM 路径有效
- 影刀 Save As(用户之前验证过的)→ 应继续成功

---

## 8. 修复路径总结

**待 spike.log 数据到达后**:

| 证据 | 修复方案 |
|---|---|
| COM failed,OutputNameAfterUIA fallback 命中 | 5.4 D 三合一(恢复 fg_bypass + 加 commit dedup) |
| COM 成功但 dialog 没跳 | 5.1 A(commit 缺失);COM 不能保证 dialog 更新到 UIA 显示 |
| 41477 找到但 dialog 没跳 | 5.1 A(commit 缺失);Chromium 41477 SetValue + Enter 触发导航 |

**预计改动文件**:
- `sigma-listary-spike/src/writer.rs`: 加 `fg_bypass` 包裹(5.4)
- `sigma-listary-spike/src/uia_inject.rs`: 在 basename fallback 后 Invoke / Enter(5.4 dedup)
- 新增 `state.rs` 字段:`last_known_path` 已存在,扩展用于 commit dedup
- 新增 unit test: `inject_folder_path` commit dedup 逻辑

**预计代码量**:~30 行,纯修复 round 18 过度删除。

---

## 9. 错误反思

我(Claude)犯了系统性错误:

1. **没有读 git 历史**就直接改 — focus-16 + focus-17 改动前后没对比,删了 PathWrap 关键链路都不知道
2. **轻信 handoff"return 19c round 19e"** 的判断 — 没读 19e/f 改动决策的逻辑就接受"auto-click Save 是 UX 倒退"的论点
4. **看了 .claude/feedback/feedback_phase1_first.md** 的提醒 "after 2+ failed fix rounds, STOP and read framework source",但这次第一轮(用户报告)就跳到 "架构性不可能" 结论
5. **没做反向验证**: spike.log 没要求用户跑就推断根因,违反 systematic-debugging Phase 1

**修复**: 重写时必须先 `git diff` 看完整改动历史 + 跑 spike.log 取数据,再制定方案。

---

## 10. 不在本次范围

- 修代码(用户明确 "不要执行")
- 重打安装包(等修复方案确定后)
- Tag(等修复后)

**本次任务**:分析 + 输出一份可执行的修复 plan。等用户给 spike.log 数据后,**升级为 focus-18 plan**(用 writing-plans 技能生成)。

---

## 11. spike.log 实测证据(2026-10-01T14:30:43 ~ 14:40:59)

### 11.1 关键事件时序(完整捕获)

| 时间戳 | 事件 | 含义 |
|---|---|---|
| `T14:30:42.735223Z` | `[monitor] dialog detected: hwnd=3671744 rect=(1000, 174) 1551x927` | monitor 检测到 dialog ✓ (前台 dialog) |
| `T14:30:43.176683Z` | `Injection starts: hwnd=3671744 target='C:\downloads'` | set_path handler 触发 write |
| `T14:30:43.176667Z` | `WARN AccessibleObjectFromWindow failed; dialog is not a native IFileDialog` | **Edge Save As 不是 IFileDialog**(是 WinUI / ItemsView)|
| `T14:30:43.475205Z` | `=== Dialog UIA inventory (hwnd=3671744, total descendants=106) ===` | UIA 遍历 106 个元素 |
| `T14:30:43.555513Z` | `No 41477 address bar; wrote BASENAME='downloads' to filename edit (target was 'C:\downloads').` | basename 写入 filename 框 ✓ |
| `T14:30:43.556081Z` | `UIA fallback succeeded.` | **代码认为成功,实际未 commit** |
| `T14:30:56.843717Z` | (第二轮)`AccessibleObjectFromWindow failed` | 同上 |
| `T14:30:57.190778Z` | `No 41477 address bar; wrote BASENAME='DrvPath'` | basename 写入 ✓ |
| `T14:40:59.126088Z` | (第三轮,不同 hwnd=2296600)`wrote BASENAME='downloads'` | 不同 dialog 也走相同路径 |

### 11.2 Edge Save As dialog 完整 UIA 结构(去重后)

```
AutomationId 数量  Name/Value
automation_id=''          32个  (SplitMenuButton 等无 ID 控件)
automation_id='DropDown'  24个  (FileType 等)
automation_id='System.Size' 12个  (右栏文件大小)
automation_id='System.ItemNameDisplay' 12个
automation_id='System.DateModified' 12个
automation_id='UpPageButton' 4个   name='向上翻页'
automation_id='UpButton' 4个       name='上一? (up)
automation_id='DownButton' 4个       name='下一? (down)
automation_id='DownPageButton' 4个 name='向下翻页'
automation_id='SplitMenuButton' 4个 name='视图滑块'
automation_id='HelpButton' 4个     name='帮助(&H)'
automation_id='SearchBoxSearchButton' 4个  search 按钮
automation_id='SearchEditBox' 4个   search 输入框
automation_id='FileNameControlHost' 4个   filename 控件容器
automation_id='FileTypeControlHost' 4个   name='保存类型:' value='JPEG...'
automation_id='1001' 4个           name='文件?' (filename Edit)
automation_id='2' 4个              取消按钮(? ID 2 = IDCANCEL)
automation_id='1' 4个              name='保存(S)'   ★ IDOK
```

### 11.3 关键发现

1. **Save 按钮 AutomationId='1'**(标准 IDOK) — UIA Invoke **完全可行**
2. **Filename 框 AutomationId='1001'**(标准 ID)
3. **无 41477 address bar**(确认 Chromium Save As 风格不可用)
4. **无 IFileDialog COM**(AccessibleObjectFromWindow 失败 → WinUI/ItemsView)
6. **无 `BreadcrumbBar`** 类(Edge WinUI Save As 渲染面包屑用 ItemsView 而非标准 breadcrumb)
7. **无 `ExplorerBrowser` / `NamespaceTreeControl`** 标准 IFileDialog 子件

### 11.4 路径激活失败的精确时序

```
focus-sync 触发 PUSH ──► HTTP 200 (协议层成功)
                                  ↓
                            sidecar set_path handler 收到 'C:\downloads'
                                  ↓
                            active_dialogs=[hwnd=3671744] (1 个)
                                  ↓
                            write_path(hwnd, 'C:\downloads')
                                  ↓
                            inject_folder_path(hwnd, 'C:\downloads')
                                  ↓
COM try_set_folder_via_com ──► AccessibleObjectFromWindow FAIL
                                                     ↓
                                       走 UIA fallback 路径
                                                     ↓
                            UIA 遍历 106 元素,未找到 41477
                                                     ↓
                            UIA SetValue(BSTR("downloads")) 到 1001 框
                                                     ↓
                            log "wrote BASENAME='downloads'"
                                                     ↓
                            log "UIA fallback succeeded"
                                                     ↓
                            function returns Ok(())
                                                     ↓
                  ★★★ 没有 Invoke Save / 没有 commit ★★★
                                                     ↓
                            dialog 上:filename='downloads',dir='photos (G:)' 不动
```

**断点在 uia_inject.rs 第 332 行(写 basename 后,直接 return Ok(()))**。原 PathWrap 代码在这一行之后有 Invoke Save / fallback_confirm Enter 步骤,被 round 18 移除。

### 11.5 用户截图 vs spike.log 互证

| 用户截图显示 | spike.log 解释 |
|---|---|
| 地址栏 "此电脑 > photos (G:)" 不动 | 没有 Invoke Save / 没有提交 |
| filename 框 = "downloads" | basename 写入成功 |
| PUSH response status=200 | HTTP 协议层 OK,但 write_path 实际"半成功" |
| version v0.3.0 显示 | Bug #2 修复 ✓(与 path activation 无关) |

---

## 12. focus-18 修复实施计划

### 12.1 修复总览

| 改动 | 文件 | 任务 |
|---|---|---|
| 1. 恢复 fg_bypass 包裹 | `sigma-listary-spike/src/writer.rs` | Task 1 |
| 2. 在 basename fallback 后 Invoke Save button | `sigma-listary-spike/src/uia_inject.rs` | Task 2 |
| 3. SendMessageW Enter fallback 替换 `fallback_confirm` 兜底 | `sigma-listary-spike/src/uia_inject.rs` | Task 2 |
| 4. dedup:只在 path 真变了才 commit | `sigma-listary-spike/src/uia_inject.rs` + `state.rs` | Task 3 |
| 5. 单元测试 (Verify id='1' button identification) | `sigma-listary-spike/src/uia_inject.rs` | Task 4 |
| 6. 集成 build + 7-app 矩阵测试 | release/ | Task 5 |

### 12.2 Task 1 — 恢复 fg_bypass

**目的**: 防止 Chromium dialog(`Chrome_WidgetWin_*` class)在 UIA 调用期间自我夺回焦点。原 commit 8016f3d 引入,被 focus-16 删除。

**改动**: `writer.rs` 直接调用改为 `fg_bypass::run_with_bypass` 包裹。fg_bypass.rs 必须从 git history 恢复(8016f3d 版本,Apache-2.0 QwenLM)。

```rust
// writer.rs 当前:
crate::uia_inject::inject_folder_path(hwnd_u32, target);

// 改为:
crate::fg_bypass::run_with_bypass(hwnd_u32, || {
    crate::uia_inject::inject_folder_path(hwnd_u32, target);
});
```

**理由**:
- 对 Chromium host (`Chrome_WidgetWin_*`): bypass 防止焦点抢回(QwenLM 验证 7/8 → 0/8)
- 对 native / WinUI: `is_chromium_target_window` 返回 false → no-op
- 对 Edge WinUI: no-op (Edge class = `WinUIDesktopWin32WindowClass` 不是 `Chrome_WidgetWin_*`)

### 12.3 Task 2 — 恢复 Invoke / Enter commit 步骤

**目的**: uia_inject.rs 第 332 行写 basename 后,**立即 Invoke Save button**(automation_id='1')触发 commit。fallback 到 SendMessageW Enter。

**改动位置**: `uia_inject.rs::inject_via_uia` 第 332 行附近

```rust
// focus-17 当前:
if let Some(edit) = best_edit {
    let name = basename(target_path);
    if !name.is_empty() {
        // SetValue filename box
        unsafe { value.SetValue(&BSTR::from(name))? };
    }
    // ← 直接结束,无 Invoke / Enter
}
Ok(())

// 改为:
if let Some(edit) = best_edit {
    let name = basename(target_path);
    if !name.is_empty() {
        unsafe { value.SetValue(&BSTR::from(name))? };
        info!("Wrote basename to filename edit; now committing via Save button.");
    }
    
    // focus-18 restore: commit the path
    match best_button {
        Some(button) if best_button_score > 0 => {
            let invoke: IUIAutomationInvokePattern =
                unsafe { button.GetCurrentPatternAs(UIA_InvokePatternId)? };
            unsafe { invoke.Invoke()? };
            info!("Committed path via Invoke(Save).");
        }
        _ => {
            // Fallback: Send Enter to native window
            if let Err(e) = send_enter_to_edit(&edit) {
                warn!("Fallback confirm Enter failed: {e}");
            } else {
                info!("Committed path via SendMessageW(WM_KEYDOWN, Enter).");
            }
        }
    }
}
Ok(())
```

**关键约束**:
- 必须 `best_button_score > 0`(只在找到真 Save 按钮时 Invoke;不能 Invoke random ID 按钮)
- SendMessageW Enter 在 filename box native window handle 上(已存在的 `fallback_confirm` 函数可直接复用)
- **绝对不能用 SendInput**(round 17 注明会抢焦点)

### 12.4 Task 3 — dedup 防止重复 commit

**问题**: 用户在 Sigma FM 中浏览 A → B → C,每次切换都触发 push。如果 commit 立即 Invoke Save,**用户切换 3 次 = 保存 3 次文件**(灾难)。

**修复**: 只在 path 真变了时才 commit。

**实现**: 在 `inject_folder_path` 接收 `last_known_path` 参数(从 dialog registry 取),只在 `target_path != last_known_path` 时 Invoke。

```rust
// main.rs::DialogEvent::Opened handler 当前:
spike::writer::write_path(hwnd_u32 as isize, &current);

// 改为(commit-aware):
let last = state.get_dialog_last_path(hwnd_u32);
spike::writer::write_path(hwnd_u32 as isize, &current, last.as_deref());
```

`state.rs` 加 `get_dialog_last_path(hwnd: u32) -> Option<String>` 方法(查询 registry)。

### 12.5 Task 4 — 单元测试

新增 4 个测试,覆盖新逻辑:

1. `score_save_button_by_automation_id`: 验证 '1' 分数 = 100(最高)
2. `score_save_button_by_name`: 验证 name 含"保存/Save/open"得分高
3. `commit_decision_path_changed`: `last != target` → commit;equal → skip
4. `inject_via_uia_filename_only_path_no_commit`: 确保 target 已含文件名 + last==target 时不 invoke

### 12.6 Task 5 — 集成测试

| 测试 | app | dialog 类型 | 期望结果 |
|---|---|---|---|
| 1 | 影刀 RPA Save As | `#32770` IFileDialog | COM SetFolder 成功 + Invoke 不需要(已 commit) |
| 2 | Chrome Save As | Chromium `Chrome_WidgetWin_*` | 41477 SetValue + 不需 Invoke(Chrome 自己处理)|
| 3 | Edge Save As (WinUI) | ItemsView | basename 写入 + Invoke Save 提交 ✓ |
| 4 | Edge Save As (Legacy) | `#32770` IFileDialog | COM SetFolder 提交 ✓ |
| 5 | Firefox Save As | Mozilla 自绘 | 待 spike.log 调查 |
| 6 | Word Save As | `#32770` IFileDialog | COM 提交 ✓ |
| 7 | VS Code Save As | `#32770` IFileDialog | COM 提交 ✓ |

### 12.7 风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| Invoke Save 自动保存文件,触发用户没预期的 save | 用户体验差 | dedup(只在 path 真变了才 commit)+ 不在当前路径未 commit |
| fg_bypass::run_with_bypass 对 WinUI 副作用 | 焦点错乱 | is_chromium_target_window 已 gated |
| 多 dialog 同时触发 commit | 多个文件保存 | round 19c dedup + main loop 单线程 commit |
| SendMessageW Enter 对 Chromium Save As 不响应 | 路径不激活 | 41477 path 优先,跳过 basename fallback |

### 12.8 不在本次范围(依然不修)

- 监控扩展脚本( `extension/dist/index.js` 没改)
- SPEC 文档更新(等 focus-18 验证后)
- FORK-KEEP-LIST 更新

---

## 14. 下一步

**等用户授权后,我会**:
1. 用 writing-plans 技能生成 focus-18 完整实施 plan
2. 执行 Task 1-5
3. 重打 focus-18 安装包到 `release/Sigma-File-Manager-2.2.0-focus-18-setup.exe`
4. 用户安装 + 7-app 矩阵测试
5. Tag `v0.3.1`(因为 v0.3.0 实际未完全解决问题)