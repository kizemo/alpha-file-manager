# focus-19 — Edge WinUI Save As 路径污染 (focus-18 v0.3.1 实测失败) 深度分析

**日期**: 2026-10-02
**作者**: Claude (MiniMax-M3)
**输入**: 用户在 Windows 11 装了 `release/Sigma-File-Manager-2.2.0-focus-18-setup.exe` 后实测
**截图证据**: 用户上传 `8735390e-2dcb-4fba-9b82-c210e3230a61/e3264c00-95d6-4255-aed8-bf91ec229812-image.png`
**结论先行**: **focus-18 v0.3.1 把"WinUI Save As"误判成"Chromium Save As",fallback 把完整路径写到 filename 输入框,污染字段。** 真正的路径导航目标(WinUI BreadcrumbBar)focus-18 未识别。

---

## 1. 用户截图 — dialog 架构特征提取

| 视觉元素 | 实现推测 | focus-18 是否识别 |
|---|---|---|
| 标题 "另存为" | WinUI dialog (Edge / Photos / 等) | — |
| 顶部 `此电脑 > photos (G:) > video` | **Microsoft.UI.Xaml.Controls.BreadcrumbBar** (ItemsRepeater + Hyperlink items) | ❌ 未识别 |
| 左侧 navigation tree (此电脑 / Win (C:) / Program (D:) / Office (E:) / 数据 (F:) / photos (G:) / 网络) | WinUI NavigationView | ❌ 未识别 |
| 右侧 file grid (名称 / 修改日期 / 类型 / 大小) | WinUI ItemsView | ❌ 未识别 |
| 底部 文件名输入框 + 保存类型 + 保存/取消 | WinUI TextBox + ComboBox + Button | ✓ filename 框被识别 (但被错用) |
| 文件名框值 `C:\DrvPath` | focus-18 写入的 Sigma FM 当前路径 | ❌ **这是 bug** |
| 文件类型 Application (*.exe) | ComboBox filter | — |

**关键判断**: 这**不是**:
- Chromium Save As (无 AutomationId 41477)
- 旧版 native IFileDialog (COM AccessibleObjectFromWindow 失败 — WinUI 不实现 IServiceProvider)

这**是**:
- **Windows 11 22H2+ WinUI 3 dialog**(Edge / Photos / Paint / 等 Save As 在新版 Edge 已迁 WinUI)

---

## 2. focus-18 实际走的代码路径

`writer::write_path(state, hwnd_isize, target, auto_confirm)` (focus-18 新签名) →
`fg_bypass::run_with_bypass(hwnd, ||)` → `uia_inject::inject_folder_path(hwnd, target, should_commit)`

`inject_folder_path` 内部 fallback chain:
1. **`try_set_folder_via_com(hwnd, target)`** (line 120)
   - 调 `AccessibleObjectFromWindow(hwnd, OBJID_NATIVEOM, IID_IAccessible)` 
   - WinUI dialog 不实现 IServiceProvider → **returns false**
   - 用户 spike.log 应有 `WARN: AccessibleObjectFromWindow failed; dialog is not a native IFileDialog`
2. **`inject_via_uia(hwnd, target, should_commit)`** (line 188)
   - FindAll(TreeScope_Descendants)
   - 遍历 descendants → 没有 41477 (Chromium-only ID)
   - 找到 best_edit (filename TextBox automation_id=104)
   - **SetValue(BSTR::from("C:\DrvPath")) → 写入 filename 输入框**
   - 用户 spike.log 应有 `INFO: No 41477 address bar; wrote FULL target='C:\DrvPath' to filename edit (Chromium/WinUI Save As interpretation).`

**focus-18 的"WinUI Save As interpretation"注释是错的** — WinUI filename 框**不接受完整路径**(只接受文件名)。Chromium Save As 才接受路径在 filename 框 + Invoke Save 触发导航;WinUI 不接受。

---

## 3. 真正的根因 (deep root cause)

### 3.1 架构级错误: focus-18 误把 WinUI 当 Chromium

focus-18 plan (`docs/superpowers/plans/2026-10-01-focus-18-path-b-auto-confirm-toggle-plan.md` §0 TL;DR) 写:
> "默认(OFF):extension/dist/index.js 把 Sigma FM 路径写到 dialog filename 框(完整路径,如 C:\downloads\),不自动 Invoke Save。这是经典 Listary 行为。"

这个设计哲学来自 **PathWrap upstream** (905db31),而 PathWrap 设计对象是 **Chromium 派生应用**(Listary 在 2010s 主要目标是 Chrome / Electron apps)。

**Microsoft 自家的 Edge Win11 Save As 已经是 WinUI 3 渲染**,**不是 Chromium 自绘 dialog**(虽然 Edge 浏览器整体是 Chromium 内核,但 Save As dialog 在 Win11 22H2+ 是独立 WinUI 进程)。

复盘证据: `docs/superpowers/plans/2026-10-01-focus-18-path-activation-architecture-regression.md` §3 已经识别 "Edge 实际有两个文件 dialog 实现" — Legacy (IFileDialog COM) / Modern (WinUI 3)。**但 focus-18 plan 没采纳这个发现**,反而按 PathWrap 上游 (Chromium) 路径实现了。

### 3.2 失败链条 (4 个失败点)

1. **检测失败**: focus-18 假设"WinUI Save As"等价于"Chromium Save As",实际 WinUI breadcrumb 不是 41477
2. **写入位置错**: fallback 把完整路径写到 filename 输入框 — WinUI filename 框**不接受完整路径**,只接受文件名
3. **无 fallback**: 没有"WinUI BreadcrumbBar 识别"分支,WinUI breadcrumb 完全被忽略
4. **COM 失败是 silent**: WinUI dialog COM `AccessibleObjectFromWindow` 失败,只 warn 不 throw,后续 UIA fallback 接管污染路径

### 3.3 用户感知层面

用户看到文件名框写着 `C:\DrvPath`,不是文件名 — 这看起来"路径被错填到文件名处"。**这不是用户理解错**,**而是 focus-18 的动作对 WinUI dialog 来说是错的**。

---

## 4. 修复方案(3 选项 + 推荐)

### 4.1 Option A: 识别 WinUI BreadcrumbBar + 优先写它

**实现**: 在 `uia_inject.rs::inject_via_uia` 增加 WinUI breadcrumb 检测:

```rust
// Pseudocode
let breadcrumb = find_winuui_breadbar(root);  // ControlType=Pane, name="..." 
                                              // OR automation_id="BreadcrumbBar"
                                              // OR child element is BreadcrumbBar type
if let Some(b) = breadcrumb {
    // Try Invoke first item ("此电脑") to expand nav menu, then send text
    // OR if has ValuePattern (rare), SetValue
    // OR drop into target folder via navigation menu
    return Ok(());
}
```

**风险**:
- WinUI BreadcrumbBar 通常**不支持 ValuePattern**(是 ItemsView / ItemsRepeater,不是 Edit)
- Invoke 单个 item 只能"激活"(展开 popup),不能 SetValue
- 需要点击 → 弹出 navigation menu → 再 SetValue 到 menu Edit 控件 — **多步交互,易 race condition**

**实施难度**: 高(需要多步 UI 交互 + race 防御)

### 4.2 Option B: 退回 native dialog: 不支持的 dialog 显式跳过 + notification (推荐)

**实现**: 识别 dialog 类型,只对**已知支持 path 注入**的 dialog 操作,其他跳过:

```rust
// Pseudocode
if com_setfolder_ok {
    info!("SetFolder via COM succeeded");
    return Ok(());  // Native IFileDialog → ✓
}
if has_41477_address_bar {
    setvalue_41477(target);
    if should_commit { invoke_save_button(); }
    return Ok(());  // Chromium → ✓
}
// WinUI / 其他: 不要 fallback 到 filename 框
warn!("dialog type not supported: no COM SetFolder, no 41477 address bar");
notify_user("Current dialog does not support auto-path. Sync skipped.");
return Ok(());  // WinUI → skip silently + warn
```

**风险**:
- 用户不会看到路径被填到 filename 框(避免"污染字段"误解)
- 但 WinUI Save As 也不被同步了 — **核心需求未满足**

**实施难度**: 低 — 但需要重新设计"WinUI 跳过"通知 UX

### 4.3 Option C: SetForegroundWindow + Enter 触发 (基线方案)

**实现**: 对所有 dialog 类型,SetForegroundWindow → SetValue to filename/breadcrumb → SendMessageW Enter:

```rust
SetForegroundWindow(hwnd);  // 用户可能感知焦点抢回
SetValue(filename_or_breadcrumb, target);
SendMessageW(native, WM_KEYDOWN, VK_RETURN, 0);  // 触发导航
SendMessageW(native, WM_KEYUP, VK_RETURN, 0);
```

**风险**:
- `SetForegroundWindow` 在 Windows 10+ 受 `SPI_GETFOREGROUNDLOCKTIMEOUT` 限制(默认 200ms),经常失败
- 焦点抢回会破坏 Sigma FM 浏览 UX(round 7 known regression)
- WinUI filename 框 SetValue 路径 + Enter 会**保存到 C:\DrvPath\ 文件名**(把 `C:\DrvPath` 当目录,**Enter 后创建该目录/默认文件名**)— 可能导致意外文件

**实施难度**: 中 — 但有副作用

### 4.4 推荐: Option A + Option B 组合

**短期 (focus-19)**:
- Option B: 显式跳过 WinUI dialog,弹 notification 告知用户"dialog 不支持"
- 保留 Option A 的代码 hook(写 `find_winuui_breadbar` 桩),留待后续

**中期 (focus-20+)**:
- Option A 完整实现: 识别 WinUI BreadcrumbBar + Invoke + 菜单交互 + race 防御

**为什么不选 C**: 副作用太大,违反"不抢焦点"原则(round 7 教训)。

---

## 5. 用户需要的数据(spike.log 证据收集)

修复方案需要先验证根因。请用户跑:

```powershell
# 看最近 500 行,过滤关键事件
Get-Content "$env:LOCALAPPDATA\kizemo\focus-sync\logs\spike.log.2026-10-02" -Tail 1000 |
  Select-String -Pattern "Dialog UIA inventory|address bar|41477|wrote FULL|AccessibleObjectFromWindow|filename edit|Committed via" |
  Select-Object -Last 50
```

期望看到的 evidence 模式 (验证 focus-18 错走 WinUI path):

```json
{"event":"SpikeError","kind":"opened_write_back","message":"dialog path='...' current='C:\\DrvPath'; writing current"}
{"event":"SpikeError","kind":"set_path_iter","message":"hwnd=... app=Chrome_WidgetWin_1 auto_confirm=false"}  ← 如果是 Chromium
// OR
{"event":"SpikeError","kind":"set_path_iter","message":"hwnd=... app=#32770 auto_confirm=false"}  ← 如果是 IFileDialog
// OR  (最可能的 WinUI scenario)
{"event":"SpikeError","kind":"set_path_iter","message":"hwnd=... app=... auto_confirm=false"}  ← 其他 class
{"event":"WriteFailed" 或 "Write", hwnd=..., reason 或 strategy="uia"}
```

加上 dialog inventory output(在 spike.log 应有 `=== Dialog UIA inventory (hwnd=..., total descendants=...) ===` 段):
- 如果看到 `automation_id="5004"` 之类**没有** `41477` 出现 → 确认是 WinUI dialog
- 如果看到 `name="BreadcrumbBar"` 或 `name="此电脑"` 之类 → 确认 WinUI breadcrumb 可识别

---

## 6. 风险矩阵 (per focus-18 plan §5 风格)

| # | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| R1 | Option A WinUI breadcrumb 检测 race(Invoke 后 dialog 重建) | 高 | 写入失败 | 加 dedup + retry with backoff |
| R2 | Option B notification 频繁打扰用户 | 中 | UX 差 | throttle notification 5min 内只发 1 次 |
| R3 | WinUI Edge 升级后 dialog 控件结构变化 | 中 | 检测失效 | uiautomation 的 controlType 优先于 automation_id(name 不可靠) |
| R4 | 用户期望"写 breadcrumb 自动导航"但 Option B 跳过 | 高 | 核心需求未满足 | focus-20+ 补 Option A |
| R5 | focus-18 已 build 但用户实测失败 → 升级路径 | 中 | 用户困惑 | 文档明示 v0.3.1 "WinUI 已知不支持",推荐 v0.3.2 |
| R6 | focus-19 改动影响 focus-18 已验证的 4 类 dialog (Edge WinUI / Chrome / 影刀 / Word) | 低 | 回归 | focus-17 测试覆盖保持 |

---

## 7. 不在本次范围

- 重写 uia_event.rs monitor (focus-19 已能检测到 dialog,hwnd 已知)
- 修改 extension/dist/index.js(协议层 OK,PUSH body 已支持 auto_confirm)
- 修改 register.ps1 (commands 列表已含 toggleAutoConfirm)
- 修改 http_server.rs (SetPathBody.auto_confirm 已 OK)

---

## 8. focus-19 任务建议 (后续 plan 待写)

7 个建议 Task 顺序:
1. **`uia_inject.rs`**: 删除"写 FULL PATH 到 filename 框" 的 fallback(改为 warn + skip)
2. **`uia_inject.rs`**: 加 `detect_winuui_breadcrumb` 函数(ControlType.Pane + name heuristic)— 留接口桩
3. **`uia_inject.rs`**: 加 `find_dialog_class(hwnd)` 识别 dialog type (Chromium / IFileDialog / WinUI / Unknown)
4. **`http_server.rs`**: /health 加 `unsupported_dialog_count` 字段,通知用户
5. **`extension/dist/index.js`**: 加 WebSocket / polling `/health`,unsupported dialog 弹 notification
6. **`register.ps1`**: 加 `focus-sync.reportDialog` command
7. **Task 8 (build + test)**: 重新 build focus-19 installer,验证 Edge WinUI / Chrome / 影刀 / Word / Notepad / VS Code 6 app 矩阵

预计代码改动: ~150 行(主要在 uia_inject.rs 检测逻辑),预计时间 2-3 小时(增量 vs focus-18 是 8 hours)。

---

## 9. 与之前 plans 的关联

- `docs/superpowers/plans/2026-10-01-focus-18-path-activation-architecture-regression.md` §3 — 早识别 Edge 有 2 个 dialog 实现,但 focus-18 plan 未采纳
- `docs/superpowers/plans/2026-10-01-focus-18-download-dialog-deep-analysis.md` §2 根因 C — 早识别 Chromium download dialog 架构性不支持(但用户这次不是 download,是 Save As)
- `docs/superpowers/plans/2026-10-01-focus-18-path-b-auto-confirm-toggle-plan.md` §0 — plan 假设 "WinUI Save As" 等价于 "Chromium Save As",实际不等价

---

## 10. 给接手会话的 TL;DR

**focus-18 v0.3.1 不支持 WinUI Save As dialog**。用户装了 focus-18 后实测 Edge Win11 Save As → 路径被错误填到 filename 输入框。修复需要走 Option B(短期跳过 + notification) + Option A 长期(识别 WinUI BreadcrumbBar)。

不要:
- ❌ 不要重做 PathWrap upstream 路径(它针对 Chromium,不等价于 WinUI)
- ❌ 不要 commit focus-18 的 git(等用户授权 commit v0.3.1 + 后续 v0.3.2 plan)

要做:
- ✅ 读 docs/superpowers/plans/2026-10-01-focus-18-path-activation-architecture-regression.md §3 (Edge 两个 dialog 实现 — 早识别但未被采纳)
- ✅ 读本文档 §4 (3 个修复选项 + 推荐)
- ✅ 等用户跑 §5 spike.log 收集脚本后开始 focus-19 Task 1