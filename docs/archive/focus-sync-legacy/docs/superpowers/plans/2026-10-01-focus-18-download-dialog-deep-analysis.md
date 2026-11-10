# focus-17 安装后下载弹窗同步失败 · 深度分析 (focus-18)

**日期**: 2026-10-01
**作者**: Claude Code
**输入**: 用户反馈 focus-17 v0.3.0 安装后,版本号显示正确(v0.3.0 ✓),但下载弹窗路径同步仍然失败
**控制台日志关键证据**:
```
[focus-sync] v0.3.0 activate START (Scheduled Task mode)
[focus-sync] onPathChange raw="C:/DiskGenius_WinPE"
[focus-sync] PUSH raw="C:/DiskGenius_WinPE" normalized="C:\\DiskGenius_WinPE"
[focus-sync] PUSH response status=200     ← sidecar 接收 OK
[focus-sync] onPathChange raw="C:/downloads"
[focus-sync] PUSH raw="C:/downloads" normalized="C:\\downloads"
[focus-sync] PUSH response status=200
[focus-sync] onPathChange raw="C:/DrvPath"
[focus-sync] PUSH raw="C:/DrvPath" normalized="C:\\DrvPath"
[focus-sync] PUSH response status=200
[focus-sync] onPathChange raw="C:/downloads"
[focus-sync] PUSH raw="C:/downloads" normalized="C:\\downloads"
[focus-sync] PUSH response status=200
```
**结论先行**: 4 个 PUSH 全部 200 OK,但 dialog 上路径未变化 → **sidecar 的 `set_path` handler 收到请求并返回成功响应,但写入 dialog 失败或写到了错的 dialog / 写到了对的 dialog 但 dialog 不响应**。HTTP 200 不保证写入真完成 — `WriteFailed` 事件存在但不影响 HTTP status。

**结论 2**: 用户测的是"下载弹窗"(不是"另存为"弹窗),这两者的架构完全不同(详见根因 C)。

---

## 1. 诊断路径(用户需要提供)

**先看 spike.log**:`%LOCALAPPDATA%\kizemo\focus-sync\logs\spike.log.<日期>`

```powershell
# 看最近 200 行,过滤关键事件
Get-Content "$env:LOCALAPPDATA\kizemo\focus-sync\logs\spike.log.2026-10-01" -Tail 500 |
  Select-String -Pattern "DialogDetected|set_path_dialogs|set_path_iter|opened_write_back|^.*Write\"|^.*WriteFailed\"" |
  Select-Object -Last 80
```

按下面 3 个**互斥**的证据模式,确定根因:

### 证据模式 A:`active_dialogs` 为 0

如果看到:
```json
{"event":"SpikeError","kind":"set_path_dialogs","message":"dialogs_count=0"}
{"event":"SpikeError","kind":"set_path_dialogs_retry","message":"retried 8x; final dialogs_count=0"}
```
且**没有** `set_path_iter` / `Write` / `WriteFailed` 事件。

**含义**: monitor **从未检测到用户的下载弹窗**。`active_dialogs` HashMap 始终为空,所以 `set_path` handler 拿到空列表,空循环,200 OK 但什么都不写。

### 证据模式 B:`WriteFailed` 事件存在

如果看到:
```json
{"event":"WriteFailed","hwnd":7539060,"app":"#32770","target":"C:\\downloads","reason":"..."}
```
**含义**: monitor 检测到 dialog,但 UIA / COM 写入失败。`reason` 字段是关键。

### 证据模式 C:只有 `Write` 事件,没有 `WriteFailed`

如果看到:
```json
{"event":"Write","hwnd":7539060,"app":"#32770","target":"C:\\downloads","strategy":"uia"}
```
**含义**: 写入"成功"(UIA / COM 没有返回 error)。但 dialog 不响应 — dialog 自身不支持 path-only SetValue。

---

## 2. 三个互斥根因 + 各自修复方向

### 根因 A:**Monitor 没检测到下载弹窗**(`active_dialogs = 0`)

#### 根因

`sigma-listary-spike/src/uia_event.rs::get_active_file_dialog` (行 267-278):
```rust
pub fn get_active_file_dialog() -> Option<DialogInfo> {
    let hwnd = unsafe { GetForegroundWindow() };
    if !hwnd.is_invalid() {
        if let Some(info) = get_dialog_info_if_match(hwnd) {
            return Some(info);
        }
    }
    // Intentionally no global scan here: new detection must come from foreground
    // to reduce false positives from generic #32770 system dialogs
    None
}
```

**只检查 `GetForegroundWindow()` 前景窗口**。如果用户在 Sigma FM 切路径(浏览器在后台),前景是 Sigma FM 而不是 dialog。monitor 检测不到 dialog,`register_dialog` 不调用。

#### 触发场景

**A1**: 用户在 Chrome 切到前台 → 点下载 → Chrome 弹出"另存为"dialog → **dialog 出现但用户没在前台**(用户还在 Chrome 的其他标签) → monitor 检测失败 → `active_dialogs` 始终空。
**A2**: 用户用 dialog 的 `Alt+Tab` 切回 Sigma FM 选路径 → 此时前景是 Sigma FM, dialog 在后台 → monitor 检测 Sigma FM(不是 file dialog class,直接 return None) → `active_dialogs` 不更新。
**A3**: Chrome 的下载弹窗不是标准 `#32770` class(实际是 Chromium 自绘的 `Chrome_RenderWidgetHostHWND`),`get_dialog_info_if_match` 直接 reject。

#### 修复方向(待诊断数据确认后选一个)

| 方案 | 实现位置 | 优 | 劣 |
|---|---|---|---|
| **A-i** `EnumWindows` 全局扫描,扩展 class 白名单 | `uia_event.rs::get_active_file_dialog` 增加 `EnumWindows` 枚举 | 检测到后台 dialog | 性能开销 + 误报风险(generic `#32770` 警告框也被检测) |
| **A-ii** `SetWinEventHook` 加 `EVENT_OBJECT_CREATE` + class 扩展 | uia_event.rs 监听窗口创建事件 | 主动检测 dialog 出现 | 误报风险相同 |
| **A-iii** `GetForegroundWindow` 失败时 fallback `GetLastInputFocus` 或 `GetGUIThreadInfo` | uia_event.rs 增加 fallback | 找到最近一次聚焦的 dialog | A3(Chromium 自绘)仍检测不到 |
| **A-iv** 诊断模式 — `dev/test/dump-dialog-tree.ps1` 跑全列出可见 hwnd + 名字 | 临时脚本 | 验证 class 白名单 | 仅诊断,非修复 |

**推荐 A-i**(最彻底,但需要 title/class 强白名单避免误报)。**当前限制**: `find_any_file_dialog` (行 342-362) 已存在但**只在 Opened dedup 失败时调用**(行 213)。需要重新设计为 primary detection。

---

### 根因 B:**写入失败**(`WriteFailed` 事件)

#### 根因

`uia_inject.rs::inject_folder_path` 的 fallback chain:
1. **COM `IFileDialog::SetFolder`** (行 192-208)— 适用 Vista+ IFileDialog,但**下载弹窗不是 IFileDialog** → 失败
2. **UIA AutomationId 41477 address bar** (行 321-327)— 适用 Chromium Save As,**下载弹窗没有 41477** → 跳过
3. **UIA best_edit basename 写入** (行 332-348)— 适用有 filename box 的 dialog → **理论可行,但 plan 删了 Enter 自动提交 + dialog 不响应 path-only SetValue**

#### 关键问题

**B1**: `write_path` 内部只调用 `inject_folder_path`,**没有把 dialog 切到前台**。UIA SetValue 在 dialog 不在前台时**可能静默失败**(Windows UIA 要求 element 在前台或聚焦才能操作)。

**B2**: 写入 `SetValue` 后 dialog 不响应,因为:
- Chrome 下载弹窗:filename 框只接受**纯文件名**,不接受目录路径
- Edge Save As (WinUI):SetValue 后必须按 Enter / 点 Save 才会跳目录
- 影刀 RPA Save As:可能是另一个 story

**B3**: `inject_via_uia` 找不到 41477 时写 basename 到 filename box,但**没有提交动作**(plan 刻意删了 auto-click Save / SendInput Enter)。

#### 修复方向

| 方案 | 实现 | 优 | 劣 |
|---|---|---|---|
| **B-i** SetValue 前调 `SetForegroundWindow(dialog_hwnd)` | `uia_inject.rs::inject_folder_path` 增加 SetForegroundWindow | 解决 B1 | Windows 限制 foreground stealing,可能被拒 |
| **B-ii** SetValue 后发 Enter / SendInput fallback | writer.rs 加 submit 步骤 | 解决 B2/B3 | round 17 删过 SendInput(foreground 抢回问题),会回归 |
| **B-iii** 检测 download dialog 类,弹 notification 跳过 | uia_event.rs 加 download dialog 识别 | 不污染 dialog | 不解决用户核心需求 |
| **B-iv** 用 `SendMessageW(WM_SETTEXT)` 而非 UIA SetValue | uia_inject.rs 切换路径 | 老 Win32 API,某些 dialog 仍响应 | 不通用(已被 Win32 标记 deprecated) |

**推荐 B-i**(最小改动,治本)。**注意**: `SetForegroundWindow` 在 Vista+ 受 `SPI_GETFOREGROUNDLOCKTIMEOUT` 限制(默认 200ms)。需调用 `AllowSetForegroundWindow` 或自己持有 foreground。

---

### 根因 C:**架构性不可能** — Chrome / Edge / Firefox 下载弹窗不支持 sync

#### 真相

**"下载弹窗" ≠ "另存为弹窗"**。这两种 dialog 架构完全不同:

| 特性 | 另存为 (Save As) | 下载确认 (Download) |
|---|---|---|
| 父进程 | 应用进程内 | 浏览器内核 (Chromium) |
| UI 框架 | Vista+ IFileDialog COM | Chromium 自绘 popup |
| 自动化接口 | IFileDialog::SetFolder / UIA 完整 | **几乎没有** |
| 有无目录导航 | ✓ (地址栏) | ✗ (只有 filename box) |
| 有无 filename box | ✓ | ✓ |
| SetValue 后能否改变目录 | ✓ | ✗ |

**Chrome / Edge / Firefox 的"下载确认"弹窗是 Chromium 内核的自绘 dialog,不暴露任何 Windows 标准的 IFileDialog / UIA address bar 接口**。你能 SetValue 的只有 filename box(填什么就是什么文件名),**无法改变下载目录**。下载目录由 Chrome 内核通过 `--default-download-directory` 命令行参数 / Chromium DevTools Protocol `Page.setDownloadBehavior` 决定。

**结论**: 当前架构(`uia_inject` 写 filename box)**对 Chrome / Edge / Firefox 下载弹窗根本无效**,无论怎么修。

#### 用户可能测试的真实场景

- Chrome "Save As"(右键图片 → 图片另存为):**可能有效**(Chrome 标准 SaveAs dialog,有 41477)
- Chrome "下载" (点超链 → 弹出下载确认):**无效**
- Edge "Save As"(文件菜单 → 另存为):**可能有效**(Edge 用 WinUI,SetFolder 有效)
- Edge "下载":**无效**
- Firefox "下载":**无效**
- 影刀 RPA "Save As":**已验证有效**(focus-16 测试)

#### 修复方向(架构性)

| 方案 | 实现 | 优 | 劣 |
|---|---|---|---|
| **C-i** 检测 Chromium 自绘 dialog,显式跳过 + 弹 notification | uia_event.rs 加 class 黑名单 | 诚实告知用户限制 | 不解决用户需求 |
| **C-ii** 用 Chrome DevTools Protocol / 浏览器扩展 改默认下载目录 | 新建浏览器扩展,IPC 到 sidecar | 真正解决问题 | 跨浏览器 × 3 大工作 |
| **C-iii** 通过注册表 / 默认下载目录修改 Edge 默认下载目录 | 修改 Windows registry | 部分有效(只能改 Edge) | Chromium 内核覆盖 |
| **C-iv** 集成 `--with-context-menu` 第三方工具(类似 Listary) | 引入 Listary 类似的产品 | 完整 | 重做产品 |

**推荐 C-i + C-ii 并行**: C-i 立刻(半天), C-ii 长期(独立项目)。

---

## 3. 综合判定矩阵

| 证据模式 | 根因 | 短期修复 | 长期方案 |
|---|---|---|---|
| A (`active_dialogs=0`) | monitor 没检测到 dialog | 加 `EnumWindows` fallback + class 白名单 | 同左 |
| B (`WriteFailed`) | UIA / COM 写入失败 | `SetForegroundWindow` + retry + diagnostics |
| C (`Write OK but no effect`) | dialog 不支持 sync(浏览器下载) | 检测 Chromium 自绘 dialog,跳过 + notification | 浏览器扩展 CDP 方案 |

**最大可能组合**: A + C(用户大概率在测 Chrome 下载,monitor 检测不到 + 即使检测到也无法 sync)。

---

## 4. 用户需要的数据(优先级排序)

1. **spike.log 的关键事件**(§1 三种证据模式之一)— 决定走 A/B/C 哪条路
2. **下载弹窗截图 + 标题**— 确认是哪种 app / dialog type
3. **测试用的 app**:Chrome 下载? Edge 下载? Firefox? Word Save As? 影刀 Save As?— 决定是 A vs C

---

## 5. 不在本次范围

- Bug #1 (console window):已修,verified
- Bug #2 (version 显示):已修,verified
- Bug #3 当前("下载弹窗 sync 失败"):本次**新增**深度分析,**修复方案待用户诊断数据后定**

---

## 6. 建议下一步

1. **用户跑 §1 的 Get-Content 命令**,看 spike.log 三种模式之一
2. **用户截一张下载弹窗的图**,我判断 dialog class
3. **我根据数据**给修复方案 + 测试用例
4. **优先级**:先 A(最常见,半天修),再 B(需 foreground steal 工作),最后 C(架构性,可选)

**当前进度**:
- focus-17 v0.3.0: 2/3 bugs 修复(Bug #1 ✓ / Bug #2 ✓ / Bug #3 部分修复但 Chrome 下载场景失败)
- 待 focus-18 修复后重新打包,tag v0.3.1(而非 v0.3.0,因为 v0.3.0 实际未解决问题)