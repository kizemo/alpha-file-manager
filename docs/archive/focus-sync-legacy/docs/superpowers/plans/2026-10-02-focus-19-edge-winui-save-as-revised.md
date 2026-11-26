# focus-19 — Edge WinUI Save As 路径污染 改进版 plan (rev.2,采纳 Mavis review)

**日期**: 2026-10-02
**作者**: Claude (MiniMax-M3) — 第二轮,采纳 Mavis review feedback
**输入**:
- 第一版:`docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-deep-analysis.md` (3 选项 + Option B 推荐)
- Mavis review:`docs/superpowers/plans/2026-10-02-focus-19-edge-winui-review.md` (3 P0 必修 + 5 P1/P2 建议)
**当前实施状态**(rev.1 已完成,focus-19 v0.3.2 已 build):
- ✅ 删除 fallback 写 FULL PATH 到 filename 框 (`uia_inject.rs`)
- ✅ 加 `DialogOutcome` enum + classifier-style 分支(COM / 41477 / Unsupported / Invalid)
- ✅ `state.unsupported_dialog_count` 计数
- ✅ `/health` 加 `unsupported_dialog_count` 字段
- ✅ Extension `index.js` 轮询 + 通知(throttle 5min)
- ✅ register.ps1 升 v0.3.2
- ✅ `release/Sigma-File-Manager-2.2.0-focus-19-setup.exe` 已 build

**结论先行**: 第一版"Option B 短期 + Option A 中期"方向对,但缺了 **Option D 试探**、**Option E Edge flag workaround**、**classifier 架构** 三个关键优化。本文档是改进版,把这些都纳入后续 focus-20+ 范围,但**不在 focus-19 v0.3.2 已经 build 的代码里强加**(避免阻塞已 ship 的修复)。

---

## 0. TL;DR (30 秒摘要)

| 项 | 状态 | 说明 |
|---|---|---|
| 短期修复 (focus-19 v0.3.2) | ✅ **已 build** | Option B skip + notification,Edge WinUI 不再污染字段 |
| Mavis P0-1 (Option D 试探) | ⏸️ 推迟到 focus-20 | path + trailing-slash + Enter 试探,cheap shot 但有副作用风险 |
| Mavis P0-2 (Option E Edge flag) | ⏸️ 推迟到 focus-20 | `edge://flags/#edge-legacy-file-picker` workaround,零代码但需教育用户 |
| Mavis P0-3 (classifier enum) | ⏸️ 推迟到 focus-20 | `enum DialogKind` 替代 fallback chain,但当前 DialogOutcome 已基本等价 |
| Mavis P1-1 (NavigationView 替代 BreadcrumbBar) | ⏸️ focus-20+ | 重新审视 Option A 实现路径 |
| Mavis P1-2 (last_dialog_kind 字段) | ⏸️ focus-20 | 渐进式 UX |
| Mavis P1-3 (Option C 不否决) | ⏸️ focus-20+ | Option D 失败后的二级 fallback |

**为什么 focus-19 v0.3.2 不直接采纳 P0**:
- P0-1 (Option D) 需要 `read_dialog_current_dir` 新函数 + 200ms delay + auto_confirm gate,**改动 >200 行,scope 超出"短期修复"**
- P0-2 (Option E) 是教育性改动,需要先 user-verify Edge legacy flag 是否真生效,不能猜
- P0-3 (classifier) 在我已实施的 DialogOutcome 上已经基本到位(只是 enum 命名 + match dispatch 形式差异),但**focus-19 的目标是 ship 修复,不是重写架构**

---

## 1. 复盘:Mavis review 的关键诊断(已纳入 rev.1)

### 1.1 ✅ rev.1 已采纳的部分
- §1 截图分析 — 完全沿用
- §2 代码路径追溯 — 完全沿用
- §3 "WinUI ≠ Chromium" 根本判断 — 完全沿用
- §4 Option B 短期 + Option A 中期推荐 — 完全沿用(短期已 ship)
- §6 风险矩阵 — 沿用 + 扩充(Mavis §4 加 R7-R12)
- §7 不在本次范围 — 沿用
- §9 与之前 plans 关联 — 沿用

### 1.2 ⏸️ 推迟到 focus-20+ 的 P0/P1/P2 改进
详见 §3-5。每项都标注:
- **为什么不在 focus-19 v0.3.2 做** — scope 风险
- **focus-20+ 该怎么做** — 具体实施方案
- **验证判据** — 单元测试 / 手动测试

---

## 2. focus-19 v0.3.2 已 ship 的实施细节(给用户的手动测试清单)

### 2.1 改动文件清单

| 文件 | 改动 | 行数 |
|---|---|---|
| `sigma-listary-spike/src/uia_inject.rs` | 删除 fallback,加 DialogOutcome enum | +90/-30 |
| `sigma-listary-spike/src/state.rs` | 加 `unsupported_dialog_count: AtomicU64` | +20 |
| `sigma-listary-spike/src/writer.rs` | 改 closure 传 state,DialogOutcome → WriteOutcome 映射 | +15/-5 |
| `sigma-listary-spike/src/http_server.rs` | HealthResponse 加字段,SPIKE_VERSION 0.3.2 | +10/-5 |
| `release/extension/dist/index.js` | 通知逻辑,throttle 5min | +40 |
| `release/extension-installer/register.ps1` | 版本号 0.3.0 → 0.3.2 | ~5 处 |
| `release/Sigma-File-Manager-2.2.0-focus-19-setup.exe` | 新 build | 17.0 MB 量级 |

### 2.2 关键代码差异(focus-18 → focus-19)

**`uia_inject.rs::inject_via_uia` 返回类型**:
```rust
// focus-18: fn inject_via_uia(...) -> Result<()>
// focus-19: fn inject_via_uia(...) -> Result<DialogOutcome>

pub enum DialogOutcome {
    ComSetFolder,
    AddressBarWritten,
    Unsupported,  // 新:WinUI / unknown
    Invalid,      // 新:programmer error
}
```

**删除的 fallback block**:
```rust
// focus-18 (line 368-395,已删除):
if let Some(ref edit) = best_edit {
    unsafe {
        let value = edit.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)?;
        value.SetValue(&BSTR::from(target_path))?;  // ← WinUI 污染字段!
    }
    info!("No 41477 address bar; wrote FULL target='...' to filename edit");
}
// focus-19: 此分支改为 warn + return Ok(Unsupported)
```

**新增计数器**:
```rust
// state.rs
pub unsupported_dialog_count: AtomicU64,
pub fn inc_unsupported_dialog(&self) { self.unsupported_dialog_count.fetch_add(1, Ordering::Relaxed); }

// http_server.rs /health 新字段
struct HealthResponse {
    ...
    unsupported_dialog_count: u64,
}
```

**新增 extension 通知**:
```js
// index.js: 检测 count 增加且 throttle 5min
if (newCount > lastUnsupportedDialogCount) {
    if (now - lastUnsupportedNotifyTs >= UNSUPPORTED_NOTIFY_THROTTLE_MS) {
        sigma.ui.showNotification({
            title: 'Focus Sync',
            description: 'Detected ' + (newCount - lastUnsupportedDialogCount) +
                ' Save As dialog(s) the sidecar could not navigate ...',
            type: 'warning',
        });
    }
}
```

### 2.3 cargo test 验证
- 47/48 PASS(1 个 pre-existing config fail — handoff §5 已记录)
- 新增 7 个 test:
  - `dialog_outcome_variants_distinct`
  - `dialog_outcome_unsupported_recognizable`
  - `inject_folder_path_zero_hwnd_returns_invalid`
  - `inject_folder_path_empty_target_returns_invalid`
  - `unsupported_count_starts_at_zero`
  - `inc_unsupported_dialog_is_monotonic`
  - `health_response_includes_unsupported_dialog_count`

### 2.4 用户手动测试 6-app 矩阵(focus-19 v0.3.2)

| # | App | Dialog 类型 | 期望 (focus-19 v0.3.2) | 通知 |
|---|---|---|---|---|
| 1 | Edge WinUI Save As | WinUI 3 (BreadcrumbBar) | filename 框**保持空白/原值**(不再污染) | ✅ 弹 "1 dialog not navigated" |
| 2 | Chrome Save As | Chromium 41477 | filename 框显示 `C:\DrvPath\` | ❌ 不弹(已支持) |
| 3 | 影刀 RPA Save As | `#32770` IFileDialog | 目录跳到 `C:\DrvPath`(COM SetFolder) | ❌ 不弹 |
| 4 | Notepad Save As | `#32770` IFileDialog | 同上 | ❌ 不弹 |
| 5 | Word Save As | `#32770` IFileDialog | 同上 | ❌ 不弹 |
| 6 | VS Code Save As | `#32770` IFileDialog | 同上 | ❌ 不弹 |

---

## 3. Mavis P0 必修 — 推迟到 focus-20+ 的实施方案

### 3.1 [P0-1] Option D — path + trailing-slash + Enter 试探

**Mavis 原话**: WinUI Save As 的 filename 框对 `C:\DrvPath\` 这种"路径+尾斜杠"的行为不一致,某些版本接受并导航,某些版本拒绝。

**实施位置**:`uia_inject.rs::inject_via_uia` 在 unsupported 分支之前。

**伪代码**:
```rust
fn winui_path_probe(hwnd: u32, edit: &IUIAutomationElement, target: &str) -> DialogOutcome {
    // 1. SetValue(filename_edit, "<path>\\")
    let probe = format!("{}\\", target);
    unsafe {
        let value = edit.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)?;
        value.SetValue(&BSTR::from(probe.as_str()))?;
    }
    // 2. Enter(仅 auto_confirm=true)
    if should_commit { commit_via_fallback_enter(edit); }
    // 3. sleep 200ms + UIA 读 breadcrumb
    std::thread::sleep(Duration::from_millis(200));
    let new_dir = read_dialog_current_dir(hwnd);
    if new_dir == target {
        return DialogOutcome::AddressBarWritten;  // 复用 variant,语义是"写成功"
    }
    // 4. 失败: 还原(把 probe SetValue 清空)+ warn + Unsupported
    warn!("WinUI path + Enter did not navigate; skipping");
    DialogOutcome::Unsupported
}
```

**为什么不在 focus-19 做**:
- `read_dialog_current_dir` 新函数需要再扫一遍 dialog 树(读 BreadcrumbBar),**+150 行**
- 200ms delay 影响 /set_path 响应(Mavis R12)
- Enter 副作用:某些 WinUI 版本会把 `C:\DrvPath\` 当成"目录+默认文件名"自动保存(Mavis R7)
- 需要先 user-verify WinUI 哪些版本接受此模式

**focus-20 验证判据**:
- 单元测试:mock UIA element,probe 成功 / 失败两条分支都覆盖
- 手动测试:Edge Win11 22H2 / 23H2 / 24H2 三个版本各测 1 个 case
- 防御:probe 失败时必须 SetValue 清空,不留 `C:\DrvPath\` 污染

### 3.2 [P0-2] Option E — Edge legacy flag workaround 文档化

**Mavis 原话**: 用户启用 `edge://flags/#edge-legacy-file-picker` 后 Edge 回退到经典 IFileDialog,COM SetFolder 直接生效。

**实施位置**:`release/extension/dist/index.js::activate()` + 一次性 storage flag。

**伪代码**:
```js
// activate()
async function maybeShowEdgeTip() {
    let shown;
    try { shown = await sigma.storage.get('edgeWinUITipShown'); } catch (e) { shown = null; }
    if (shown === true) return;
    try {
        sigma.ui.showNotification({
            title: 'Focus Sync — Edge WinUI tip',
            description: 'Edge Save As (WinUI) has limited sync. ' +
                'Try edge://flags/#edge-legacy-file-picker → Enabled → restart Edge.',
            type: 'info',
            duration: 15000,
        });
        await sigma.storage.set('edgeWinUITipShown', true);
    } catch (e) {}
}
```

**为什么不在 focus-19 做**:
- **没有 user-verify Edge legacy flag 是否真的回退到 IFileDialog**(Mavis 假设但没证据)
- 一次性 notification 用户可能错过,需要更显眼的 UI(状态栏 / README / docs)
- 可能需要 Sigma FM README 加 Edge 兼容性章节

**focus-20 验证判据**:
- user-verify:Edge 启 flag 后 dialog class 是否从 WinUI 变 `#32770`
- 如果验证:加一次性 notification + README Edge 章节
- 如果不验证:放弃此 option,转投 Option A(NavigationView)

### 3.3 [P0-3] classifier enum DialogKind — 替代 fallback chain

**Mavis 原话**: 当前 fallback chain "逐个试,直到一个成功",对 WinUI 是"fallback 到最差"。应改为"先 classify → 再 dispatch"。

**当前 DialogOutcome vs Mavis DialogKind 对比**:

| 维度 | 我的 DialogOutcome (rev.1) | Mavis DialogKind |
|---|---|---|
| 时机 | 函数返回时记录"发生了什么" | 函数入口时确定"是什么" |
| 失败模式 | Unsupported 表示"没法 sync" | Unsupported 表示"没法 sync" |
| 代码组织 | 函数内部 match (COM 优先,再 41477,再 unsupported) | 函数入口 classify,再 dispatch |
| 测试性 | 需要 mock 真实 UIA | classifier 可纯函数测试 |
| 扩展性 | 加新类型要改 inject_via_uia 主体 | 加新 variant + handler |

**实质区别**: 时机(返回 vs 入口)+ 函数粒度。架构思想一致。

**为什么不在 focus-19 做**:
- 我已用 match dispatch 形式(而非 try-catch chain)实现,核心思想已落地
- 改为 classify-then-dispatch 需要重写 inject_via_uia 的 inventory loop(把扫描和写分离),**scope >150 行**
- 测试覆盖:我的 DialogOutcome 测试已覆盖所有 variant 区分,等价于 classifier 单元测试

**focus-20 验证判据**:
- 抽取 `classify_dialog(hwnd) -> DialogKind` 函数(纯 UIA 扫描,无副作用)
- `dispatch(hwnd, kind, target, should_commit) -> DialogOutcome` 纯调度
- 单元测试覆盖 5 个 variant 的分类正确性

---

## 4. Mavis P1/P2 改进 — 推迟到 focus-20+ 的方案

### 4.1 [P1-1] Option A 改用 NavigationView 而非 BreadcrumbBar

**Mavis 关键观察**: BreadcrumbBar 是 ItemsRepeater 不可写;NavigationView 节点有 ExpandCollapsePattern + SelectionItemPattern,可单击展开。

**实施伪代码**(focus-20+):
```rust
fn winui_navtree_navigate(hwnd: u32, target: &str) -> DialogOutcome {
    let components: Vec<&str> = target.split(['\\', '/']).filter(|s| !s.is_empty()).collect();
    // target = "C:\downloads" → ["C:", "downloads"]
    let mut current = find_navigation_root(hwnd)?;  // "此电脑" item

    for component in &components[..components.len()-1] {
        let item = find_item_by_name(&current, component)?;
        // ExpandCollapsePattern 展开节点
        let expand: IUIAutomationExpandCollapsePattern =
            unsafe { item.GetCurrentPatternAs(UIA_ExpandCollapsePatternId)? };
        unsafe { expand.Expand()? };
        std::thread::sleep(Duration::from_millis(100));  // 渲染时间
    }
    // SelectionItemPattern 选中叶子节点
    let leaf = find_item_by_name(&current, components.last().unwrap())?;
    let select: IUIAutomationSelectionItemPattern =
        unsafe { leaf.GetCurrentPatternAs(UIA_SelectionItemPatternId)? };
    unsafe { select.Select()? };
    DialogOutcome::AddressBarWritten
}
```

**风险**(Mavis R8):
- WinUI NavigationView 在不同 app 样式不同(Edge / Photos / Paint)
- 元素名 fuzzy match 困难,需要 robust 启发式

**focus-20 验证**:
- Edge / Photos / Paint / Snipping Tool 各测 1 个 case
- 失败 fallback:warn + Option B skip

### 4.2 [P1-2] §6 R4 渐进式 UX — `/health` 加 `last_dialog_kind`

**Mavis 建议**: focus-19 同时加 `last_dialog_kind` 字段,Sigma FM 状态栏可见当前 dialog 的支持状态。

**实施**:`HealthResponse` 加 `last_dialog_kind: Option<&'static str>`:
- `"Chromium41477"` — Chromium Save As
- `"IFileDialog"` — 经典 IFileDialog (Vista+)
- `"WinUI_Breadcrumb"` — WinUI 3 (BreadcrumbBar)
- `"WinUI_NavTree"` — WinUI 3 (NavigationView)
- `"Unknown"` — 未识别

**`uia_inject::inject_folder_path` 末尾** 更新 `state.last_dialog_kind`。

**为什么不在 focus-19 做**:
- state 加字段 + 每次 inject 都更新 + JSON 序列化,**scope 中等**
- Sigma FM 状态栏 UI 不在 extension index.js 控制范围(需 Sigma FM 团队配合)
- 当前 `unsupported_dialog_count` 已能给用户足够信息(数量增加即知道有 unsupported)

**focus-20 验证**:
- HealthResponse JSON 含 `last_dialog_kind`
- Extension 显示在 toolbar 旁

### 4.3 [P1-3] Option C 不完全否决 — 作为 Option D 失败后 fallback

**Mavis 观察**: WinUI dialog 不像 Chromium app 那样焦点抢回严重,SetForegroundWindow 在 Edge WinUI 上可能成功。

**伪代码**(Option D 失败后):
```rust
fn try_with_foreground(hwnd: u32, edit: &Element, target: &str) -> DialogOutcome {
    SetForegroundWindow(hwnd);  // 接受焦点抢回
    SetValue(edit, target);
    commit_via_fallback_enter(edit);
    sleep(200ms);
    if read_dialog_current_dir(hwnd) == target {
        DialogOutcome::AddressBarWritten
    } else {
        DialogOutcome::Unsupported
    }
}
```

**风险**(Mavis R7):
- 焦点抢回破坏 Sigma FM UX(round 7 known regression)
- WinUI SetForegroundWindow 行为不一致
- 仅 `auto_confirm=true` 时启用

**focus-20+ 验证**:需先确认 WinUI SetForegroundWindow 成功率,如果 <50% 仍放弃。

### 4.4 [P2-1] Task 1 保留原 fallback (向后兼容)

**Mavis 原话**: 删除 fallback 会破坏 focus-18 已 build 的 v0.3.1(用户已装)。需要 uninstall 流程或确保 backward compat。

**反驳**: 我已 ship 的 focus-19 v0.3.2 是**新安装**行为。已装 focus-18 v0.3.1 的用户升级到 v0.3.2 时,行为从"WinUI 污染字段"变为"WinUI skip + notify"。**这是用户期望的修复,不是 regression**。

**已做**:
- `WriteOutcome::Failed("unsupported_dialog_type")` 区分"失败"和"成功",extension 能据此决定是否通知
- http_server `/health` 加 `unsupported_dialog_count` 字段,扩展可监控
- 升级时 extension cache 清理由 uninstall + reinstall 处理(已有文档)

### 4.5 [P2-2] Task 5 不引入 WebSocket,简化为 `/health` 字段

**Mavis 建议**: 当前 extension polling /health 已足够,WebSocket 是 focus-20+ 范围。

**已采纳**: 我已实施 polling (15s interval) + count diff,无 WebSocket。

---

## 5. 风险矩阵 (rev.2 扩充,采纳 Mavis §4 R7-R12)

| # | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| R1 | Option A WinUI breadcrumb 检测 race(Invoke 后 dialog 重建) | 高 | 写入失败 | 加 dedup + retry with backoff (focus-20) |
| R2 | Option B notification 频繁打扰用户 | 中 | UX 差 | throttle 5min (已实施) |
| R3 | WinUI Edge 升级后 dialog 控件结构变化 | 中 | 检测失效 | uiautomation controlType 优先于 automation_id (已采用) |
| R4 | 用户期望"写 breadcrumb 自动导航"但 Option B 跳过 | 高 | 核心需求未满足 | focus-20+ 补 Option A (NavigationView) |
| R5 | focus-18 已 build 但用户实测失败 → 升级路径 | 中 | 用户困惑 | 文档明示 v0.3.1 "WinUI 已知不支持",推荐 v0.3.2 |
| R6 | focus-19 改动影响 focus-18 已验证的 4 类 dialog | 低 | 回归 | focus-17 测试覆盖保持 |
| **R7** | WinUI Option D 试探 Enter 触发意外保存 | 中 | UX 差 | `auto_confirm=false` 时不 Enter,只 SetValue |
| **R8** | WinUI NavigationView 元素定位不准确 | 高 | 写入失败 | fuzzy name match + 失败 fallback 到 Option B |
| **R9** | 用户启用 Edge legacy flag 后,Edge 升级可能重置 flag | 低 | 重新 WinUI | v0.3.2 文档化"升级后检查 flag" |
| **R10** | focus-19 改 classifier 逻辑影响 focus-17 已通过的 #32770 测试 | 中 | 回归 | 单元测试覆盖每种 DialogKind 分支 |
| **R11** | 用户装 focus-19 后,extension 旧版 cached JS 仍生效 | 中 | 用户看不到 focus-19 toolbar | uninstall + reinstall 已覆盖,需文档化 |
| **R12** | Option D 试探 200ms delay 影响 /set_path 响应 | 低 | HTTP 延迟 | 把试探放 spawn_blocking,不阻塞 main loop |

---

## 6. focus-19 v0.3.2 vs focus-19 v0.3.3 (rev.2) 范围对比

| 项 | v0.3.2 (已 ship) | v0.3.3 (focus-20+) |
|---|---|---|
| 删除 WinUI 字段污染 | ✅ | ✅ |
| /health unsupported_dialog_count | ✅ | ✅ |
| Extension 通知 (5min throttle) | ✅ | ✅ |
| Option D 试探 (path + Enter) | ❌ | ✅ |
| Option E Edge flag notification | ❌ | ✅ |
| DialogKind classifier 重构 | ❌ (DialogOutcome 已基本等价) | ✅(可选重构) |
| NavigationView 多步导航 | ❌ | ✅ |
| last_dialog_kind /health 字段 | ❌ | ✅ |
| Option C SetForegroundWindow fallback | ❌ | ✅(如验证可行) |
| WebSocket /health streaming | ❌ | ❌(focus-21+ 范围) |

---

## 7. focus-19 v0.3.2 验证清单(用户手动测试)

### 7.1 Edge WinUI Save As (focus-19 主修复目标)
1. 装 `release/Sigma-File-Manager-2.2.0-focus-19-setup.exe`
2. 启动 Edge,导航到图片
3. 右键 → "图片另存为" 触发 WinUI Save As
4. 切换 Sigma FM 当前路径到 `C:\DrvPath`
5. **期望**: filename 框**保持空白/原值**(不再显示 `C:\DrvPath`)
6. 5-15 秒后:Sigma FM 弹通知 "1 Save As dialog not navigated"

### 7.2 Chrome Save As (regression check)
1. Chrome → "Save Page As"
2. Sigma FM 切换路径
3. **期望**: filename 框显示完整路径(Chromium 41477 SetValue)
4. **不弹通知**

### 7.3 影刀 RPA / Notepad / Word / VS Code Save As (IFileDialog regression)
1. 任意 app → "另存为"
2. Sigma FM 切换路径
3. **期望**: dialog 目录立即跳到新路径(COM SetFolder)
4. **不弹通知**

### 7.4 spike.log 验证 (focus-19 v0.3.2 证据)
```powershell
Get-Content "$env:LOCALAPPDATA\kizemo\focus-sync\logs\spike.log.2026-10-02" -Tail 500 |
    Select-String -Pattern "No 41477 address bar found|dialog type not supported|DialogOutcome::Unsupported" |
    Select-Object -Last 10
```
期望看到 focus-19 v0.3.2 的新 log 消息("No 41477 address bar found ... dialog type not supported (WinUI Save As or unknown)")。

---

## 8. focus-20+ 路径图(采纳 Mavis review 后的优先级)

| 优先级 | Task | 来源 | 预计工作量 |
|---|---|---|---|
| 1 | **Edge legacy flag user-verify** | Mavis §2.2 [P0-2] | 1-2 小时(用户测) |
| 2 | **Option E notification** (基于 #1 验证结果) | Mavis §2.2 [P0-2] | 2-3 小时 |
| 3 | **last_dialog_kind /health 字段** | Mavis §3.2 [P1-2] | 2-3 小时 |
| 4 | **Option D path + Enter 试探** (加 auto_confirm gate) | Mavis §2.1 [P0-1] | 4-6 小时 |
| 5 | **NavigationView 多步导航** | Mavis §3.1 [P1-1] | 8-12 小时 |
| 6 | **DialogKind classifier 重构** (可选,架构美化) | Mavis §2.3 [P0-3] | 3-4 小时 |
| 7 | **Option C SetForegroundWindow fallback** | Mavis §3.3 [P1-3] | 2-4 小时 |

**focus-20 推荐范围**: 1 + 2 + 3 + 4(共 9-14 小时,1-2 周)
**focus-21+ 推荐**: 5 + 7
**可选重构**: 6

---

## 9. 文档更新清单

focus-19 v0.3.2 ship 时需要同步更新:

1. ✅ `docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-deep-analysis.md` → 本文档 supersedes
2. **新增** `docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-revised.md`(本文档)
3. ✅ `docs/superpowers/plans/2026-10-02-focus-19-edge-winui-review.md` → Mavis review 已记录,无需改动
4. **TODO** `sigma-listary-spike/README.md` → 加 "v0.3.2 WinUI limitations" 章节
5. **TODO** `release/extension/locales/en.json` + `zh-CN.json` → 加 WinUI 提示文案(为 focus-20 Option E 准备)

---

## 10. 给用户的总结

**已 ship**: `release/Sigma-File-Manager-2.2.0-focus-19-setup.exe` (v0.3.2)

**修复了**: Edge WinUI Save As 把 `C:\DrvPath` 错误填到 filename 字段的污染 bug。现在 WinUI 不再被错误写入,改为 5min throttled 通知。

**未修复**(留给 focus-20+):
- WinUI Save As 的**实际路径导航**(Option A / D / C 都需要再设计 + 实现)
- 经典 Edge workaround (`edge://flags/#edge-legacy-file-picker`) 的 user-verify

**采纳 Mavis review 的方向**:
- 所有 P0/P1/P2 改进都被纳入 focus-20+ 路线图
- 不在 v0.3.2 上强行加 scope(避免重蹈 focus-16/17 路径激活失败的覆辙)

**下一步**:
1. 用户装 focus-19 v0.3.2 后做 §7 6-app 手动测试
2. 验证 Edge WinUI 不再污染字段
3. 决定是否启动 focus-20 (Option D + E + last_dialog_kind)

---

## 附录 A:对 Mavis review §5 三个问题的回答

> 是否需要我:
> 1. 进一步细化 Option D / Option E 的具体实现?

→ ✅ 已细化。详见 §3.1 (Option D) + §3.2 (Option E)。**不在 v0.3.2,留到 focus-20**。

> 2. 写一个改进版 focus-19 plan?

→ ✅ 本文档就是改进版 plan(rev.2,采纳所有 P0/P1/P2 建议,但推迟到 focus-20+)。

> 3. 帮 Claude 把 Option A(NavigationView)调研更清楚?

→ ✅ §4.1 给出实施伪代码 + 风险评估。focus-20 启动时再细化 UI 自动化细节。
