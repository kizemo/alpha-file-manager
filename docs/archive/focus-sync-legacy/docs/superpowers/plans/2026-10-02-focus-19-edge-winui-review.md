# focus-19 — Edge WinUI Save As 实测失败分析(Mavis review)

**日期**: 2026-10-02
**作者**: Mavis (M3)
**输入**: `docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-deep-analysis.md`
**基础**: 上一轮 Path B 实施(`docs/superpowers/plans/2026-10-01-focus-18-path-b-auto-confirm-toggle-plan.md`)
**性质**: 第三轮 review。给 Claude 第二轮回合,只提意见,不执行修改。

---

## 0. TL;DR

**Claude 这次方向对了**: focus-18 把 WinUI Save As 当作 Chromium Save As 处理,错把完整路径写到 filename 输入框,污染字段。**3 个修复选项大方向合理,推荐 Option B 短期 + Option A 中期,但有 4 处可改进**:

1. 🔴 **新增 Option D** — "写路径 + trailing slash + Enter" 先尝试,失败再 skip。WinUI 部分版本接受此模式。
2. 🔴 **新增 Option E** — Edge 文档化 workaround:`edge://flags/#edge-legacy-file-picker` 启用后 dialog 变 legacy IFileDialog,焦点 sync 直接 work。
3. 🟡 **Option A 目标选错** — BreadcrumbBar 是 ItemsRepeater 不可写,**应改用左侧 NavigationView**(tree-view 风格)多步导航。比 BreadcrumbBar 可靠。
4. 🟡 **检测逻辑缺分类器** — 当前是 "try-and-fail",应是 "先 classify → 再 route"。代码结构应改成 `enum DialogKind { Chromium41477, IFileDialog, WinUIBreadcrumb, WinUINavTree, Unknown }`,而不是 fallback chain。

---

## 1. Claude 做对的部分

| 决策 | 评价 |
|---|---|
| §1 截图分析(WinUI dialog 特征提取) | ✓ 准确,识别了 BreadcrumbBar / NavigationView / ItemsView / TextBox 等组件 |
| §2 实际代码路径追溯 | ✓ 一字不差还原了 focus-18 的执行流 |
| §3 "WinUI ≠ Chromium" 的根本判断 | ✓ **核心修复方向正确**。PathWrap 上游针对 Chromium,不能盲套到 WinUI |
| §3.2 4 个失败点列表 | ✓ 完整 |
| §4 Option B + Option A 组合推荐 | ✓ 短期跳过 + 中期实现,平衡务实 |
| §6 R1-R6 风险矩阵 | ✓ 全面 |
| §7 "不在本次范围" 划清 | ✓ 正确 |
| §9 与之前 plans 的关联追溯 | ✓ 诚实地标注"早识别但未采纳" |

---

## 2. 🔴 必修改进(3 处)

### 2.1 [P0] 新增 Option D — "path + trailing slash + Enter" 试探

**问题**: WinUI Save As 的 filename 输入框对**只包含路径的字符串**(如 `C:\DrvPath\`)行为不一致:
- 某些版本 WinUI:接受路径,Enter 后导航到该目录
- 某些版本 WinUI:拒绝非法文件名,Enter 无效

→ 这是值得**先尝试再放弃**的 cheap shot。

**实现位置**:`uia_inject.rs::inject_via_uia` 在 WinUI 检测分支中,**先**尝试:
```rust
// focus-19 Option D: WinUI 试探
if is_winuui_dialog {
    // 1. SetValue(filename_edit, "<path>\\")
    value.SetValue(&BSTR::from(format!("{}\\", target_path)))?;
    // 2. SendMessageW Enter 到 filename edit native HWND
    commit_via_fallback_enter(&edit);
    // 3. 等 200ms,看 dialog 是否导航(用 UIA 读 breadcrumb)
    std::thread::sleep(Duration::from_millis(200));
    let new_dir = read_dialog_current_dir(hwnd);  // 新增函数
    if new_dir == target_path {
        info!("WinUI: path + trailing-slash + Enter triggered navigation ✓");
        return Ok(());
    }
    // 4. 失败: warn + 通知(走 Option B 路径)
    warn!("WinUI: path + Enter did not navigate; skipping");
    notify_unsupported_dialog("WinUI Save As");
    return Ok(());
}
```

**关键**:**不要 fallback 到 raw path 写到 filename**(focus-18 犯的错),用户看到 `C:\DrvPath` 在 filename 框会困惑。

**风险**:Enter 可能触发**意外保存**(某些 WinUI 版本 Enter on `C:\DrvPath\` 会自动用默认文件名保存)。缓解:`auto_confirm` gate — 默认 OFF 时不 Enter,只 SetValue。

### 2.2 [P0] 新增 Option E — Edge legacy flag workaround 文档化

**问题**: Edge Win11 22H2+ 默认用 WinUI Save As。如果用户启用 `edge://flags/#edge-legacy-file-picker`,Edge 会**回退到经典 IFileDialog**(`#32770`),COM SetFolder 直接生效。

**这是**:
- **零代码改动** 的解决方案
- 适合不想等 focus-20+ 的用户
- 已验证对其他 Chromium fork 工作

**实现**:
- `release/extension/dist/index.js` 的 `activate()` 加一次性 notification:
  ```js
  try {
      sigma.ui.showNotification({
          title: 'Focus Sync — Edge WinUI tip',
          description: 'Edge Save As (WinUI) has limited sync. Try edge://flags/#edge-legacy-file-picker → Enabled → restart Edge.',
          type: 'info',
          duration: 15000,
      });
  } catch (e) {}
  ```
- 或者:`release/extension/locales/*.json` 加 i18n(可选)
- `FORK-KEEP-LIST.md` 加 Edge flag 注意事项

**只显示一次**:用 `sigma.storage.get('edgeWinUITipShown')` flag,已显示过就跳过。

### 2.3 [P0] 检测逻辑改为分类器而非 fallback chain

**问题**: focus-18 的 fallback chain 是"逐个试,直到一个成功":
1. try COM SetFolder → 失败
2. try 41477 address bar → 没找到
3. try filename edit SetValue → **写错了位置**

这种结构对 WinUI dialog 的失败模式是"fallback 到最差选项",而不是"识别类型后选最佳方案"。

**实现**:加 `enum DialogKind`,首次扫描 dialog 时确定类型,然后**直接 dispatch** 到对应 handler:

```rust
enum DialogKind {
    NativeIFileDialog,     // COM SetFolder works → return
    Chromium41477,          // 41477 + Invoke works
    WinUINavigationView,    // 左 tree 可导航 → Option A long-term
    WinUIBreadcrumb,        // 顶部 breadcrumb 不可写 → Option D 试探
    Unsupported,            // 跳过 + 通知
}

fn classify_dialog(hwnd: u32, root: &IUIAutomationElement) -> DialogKind {
    // 1. try COM AccessibleObjectFromWindow
    if try_set_folder_via_com(hwnd, "").is_ok_with_com_probe() {
        return DialogKind::NativeIFileDialog;
    }
    // 2. scan for 41477 (Chromium)
    let elements = root.FindAll(TreeScope_Descendants, &condition)?;
    for el in elements {
        if current_automation_id(&el) == "41477" {
            return DialogKind::Chromium41477;
        }
    }
    // 3. WinUI heuristic: NavigationView (control_type=Tree/Tab + automation_id matches pattern)
    if has_winuui_navtree(&elements) {
        return DialogKind::WinUINavigationView;
    }
    if has_winuui_breadcrumb(&elements) {
        return DialogKind::WinUIBreadcrumb;
    }
    DialogKind::Unsupported
}
```

然后:
```rust
match classify_dialog(hwnd, &root) {
    DialogKind::NativeIFileDialog => try_set_folder_via_com(hwnd, target)?,
    DialogKind::Chromium41477 => write_41477_and_invoke(hwnd, target, should_commit)?,
    DialogKind::WinUINavigationView => winui_navtree_navigate(hwnd, target)?,  // Option A long-term
    DialogKind::WinUIBreadcrumb => winui_breadcrumb_probe(hwnd, target, edit)?,  // Option D
    DialogKind::Unsupported => notify_and_skip(hwnd)?,
}
```

**好处**:
- 每种 dialog 走最佳路径,不会 fallback 到错误位置
- 新增 dialog 类型只需加 enum 变体 + handler
- 单元测试可覆盖分类器(无需真实 dialog)

---

## 3. 🟡 改进建议(5 处)

### 3.1 [P1] Option A 目标选错 — 用 NavigationView 而非 BreadcrumbBar

**Claude §4.1 说**:
> WinUI BreadcrumbBar 通常**不支持 ValuePattern**(是 ItemsView / ItemsRepeater,不是 Edit)
> Invoke 单个 item 只能"激活"(展开 popup),不能 SetValue
> 需要点击 → 弹出 navigation menu → 再 SetValue 到 menu Edit 控件 — **多步交互,易 race condition**

**Claude 自己已认识到 BreadcrumbBar 不可写,但仍把它列为 Option A 目标**。

**实际更好的目标**:**左侧 NavigationView**(TreeView 风格)。

从用户截图描述:
- 左侧:`此电脑 / Win (C:) / Program (D:) / Office (E:) / 数据 (F:) / photos (G:) / 网络`

这是 NavigationView / TreeView,**每个 item 可点击展开**,层级结构清晰。导航流程:
1. Find "Win (C:)" item → Invoke / click
2. → 右侧 grid 显示 C:\ 内容
3. Find "downloads" item → Invoke / click
4. → 进入 C:\downloads

**优势 vs BreadcrumbBar**:
- NavigationView 节点有 **ExpandCollapsePattern**(可展开/折叠),**SelectionItemPattern**(可选中)
- 不需要"先激活 popup 再 SetValue"的多步交互
- 多步但每步都是单 Invoke,不涉及 race condition 重的"popup 出现 + 找 Edit"

**实施步骤**:
```rust
fn winui_navtree_navigate(hwnd: u32, target_path: &str) -> Result<()> {
    // target_path = "C:\downloads"
    let components: Vec<&str> = target_path.split(['\\', '/']).filter(|s| !s.is_empty()).collect();
    // components = ["C:", "downloads"]
    let mut current_root = find_navigation_root(hwnd)?;  // "此电脑" item
    for component in &components[..components.len()-1] {
        // Navigate to drive letter (skip "C:" — expand tree root)
        // Or find item by name == component
        let item = find_item_by_name(&current_root, component)?;
        let expand: IUIAutomationExpandCollapsePattern =
            unsafe { item.GetCurrentPatternAs(UIA_ExpandCollapsePatternId)? };
        unsafe { expand.Expand()? };
        std::thread::sleep(Duration::from_millis(100));  // 给 tree 渲染时间
    }
    // Click the last component (leaf folder)
    let leaf = find_item_by_name(&current_root, components.last().unwrap())?;
    let select: IUIAutomationSelectionItemPattern =
        unsafe { leaf.GetCurrentPatternAs(UIA_SelectionItemPatternId)? };
    unsafe { select.Select()? };
    Ok(())
}
```

**风险**:WinUI NavigationView 在不同 app 里样式不同(Edge / Photos / Paint 各自实现),元素名字不一致。需要 robust 的 fuzzy match 或直接基于控件结构遍历。

### 3.2 [P1] §6 R4 "核心需求未满足"需要具体应对策略

**Claude §6 R4 说**:
> R4 | 用户期望"写 breadcrumb 自动导航"但 Option B 跳过 | 高 | 核心需求未满足 | focus-20+ 补 Option A |

**这是诚实的承认,但没有给出具体过渡策略**。

**建议**:
- focus-19 (短期):Option B skip + notification,用户**明确知道**WinUI 不被支持
- focus-19 同时:**在 Sigma FM 状态栏显示当前 dialog 的支持状态**(`/health` 加 `last_dialog_kind: "Chromium41477" | "IFileDialog" | "WinUI (unsupported)" | "Unknown"`)
- 用户看到状态后,可以决定是否启用 Edge legacy flag 或换用其他 dialog
- focus-20+ 实现 Option A(NavigationView)补救

**这是 "渐进式 UX"**:不假装支持,但告诉用户真实情况。

### 3.3 [P1] Option C 不应完全否决

**Claude §4.3 反对 Option C 的理由**:
> SetForegroundWindow 在 Windows 10+ 受 SPI_GETFOREGROUNDLOCKTIMEOUT 限制(默认 200ms),经常失败
> 焦点抢回会破坏 Sigma FM 浏览 UX(round 7 known regression)

**但 Claude 漏了一个细节**:WinUI dialog 不像 Chromium app 那样焦点抢回严重。WinUI 是 Microsoft 官方组件,`SetForegroundWindow` 在 Edge WinUI 上通常成功(因为 Edge 主进程把 dialog 作为自己的窗口,不涉及 chromium focus stealing)。

**建议**:
- Option C 不应直接否决,而是**作为 Option D 失败后的 fallback**
- 即:先尝试 Option D(path + Enter),失败再尝试 Option C(SetForegroundWindow + path + Enter),最后 Option B(skip + notify)
- Option C 加 `auto_confirm=true` gate(避免误保存)

### 3.4 [P2] §8 Task 6 / Task 5 顺序问题

**Claude §8 列出 7 个 Task**:
1. uia_inject.rs 删 fallback 写 filename
2. uia_inject.rs 加 detect_winuui_breadcrumb
3. uia_inject.rs 加 find_dialog_class
4. http_server.rs /health 加 unsupported_dialog_count
5. extension/dist/index.js WebSocket polling
6. register.ps1 focus-sync.reportDialog
7. Build + test

**问题**:
- Task 1 删 fallback 会破坏 focus-18 已 build 的 v0.3.1(用户已装)。需要 uninstall 流程或确保 backward compat
- Task 5 WebSocket polling 是个**重大架构改动**(sidecar 加 WS server),focus-19 不应承担这个范围
- Task 6 加 command 但当前 toolbar 已有 `reportDialog` 不存在,实际是 `report_unsupported_dialog` — 命名/语义需明确

**建议**:
- Task 1 改为"在 fallback chain 末尾加 WinUI skip 分支",不删原 fallback(向后兼容)
- Task 5 (WebSocket) 改为"简单 `/health` 字段 + extension 启动时读一次"。WS 是 focus-20+ 范围
- Task 6 改为可选 toolbar 菜单项,默认隐藏

### 3.5 [P2] §5 spike.log 收集脚本可以更具体

**Claude §5 让用户跑**:
```powershell
Get-Content "$env:LOCALAPPDATA\kizemo\focus-sync\logs\spike.log.2026-10-02" -Tail 1000 |
  Select-String -Pattern "Dialog UIA inventory|address bar|41477|wrote FULL|AccessibleObjectFromWindow|filename edit|Committed via" |
  Select-Object -Last 50
```

**建议补充**:
```powershell
# 同时看 app class (确定是 WinUI 还是别的)
Select-String -Pattern "DialogDetected|app=" | Select-Object -Last 10

# 看完整 inventory 段(验证 WinUI 控件结构)
Select-String -Pattern "Dialog UIA inventory|End of dialog inventory" -Context 0,50
```

---

## 4. 风险补充(Claude 未列)

| # | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| R7 | WinUI Option D 试探 Enter 触发意外保存 | 中 | UX 差 | `auto_confirm=false` 时不 Enter,只 SetValue |
| R8 | WinUI NavigationView 元素定位不准确(不同 app 控件树不同) | 高 | 写入失败 | 加 fuzzy name match + 失败 fallback 到 Option B |
| R9 | 用户启用 Edge legacy flag 后,Edge 升级可能重置 flag | 低 | 重新 WinUI | v0.3.2 文档化"升级后检查 flag" |
| R10 | focus-19 改 classifier 逻辑影响 focus-17 已通过的 #32770 测试 | 中 | 回归 | 单元测试覆盖每种 DialogKind 分支 |
| R11 | 用户装 focus-19 后,从 focus-18 升级时 extension 旧版 cached JS 仍生效 | 中 | 用户看不到 focus-19 toolbar | uninstall + reinstall 已覆盖,但需文档化 |
| R12 | §2.1 Option D 试探需要 delay 200ms,影响 /set_path 响应时间 | 低 | HTTP 延迟 | 把试探放 spawn_blocking,不阻塞 main loop |

---

## 5. 给 Claude 的总结指令

### 必修(3 处)

1. **加 Option D** — path + trailing-slash + Enter 试探,失败再 skip
2. **加 Option E** — Edge `edge://flags/#edge-legacy-file-picker` workaround 文档化 + 一次性 notification
3. **改 fallback chain 为 DialogKind 分类器** — `enum DialogKind` + match dispatch,不试-and-fail

### 建议(5 处)

4. **Option A 改用 NavigationView 而非 BreadcrumbBar** — 节点可展开/选中,实施更稳
5. **§6 R4 渐进式 UX** — focus-19 同时加 `/health` 字段 `last_dialog_kind`,Sigma FM 状态栏可见
6. **Option C 不完全否决** — 作为 Option D 失败后的二线 fallback
7. **Task 1 不删原 fallback,加 WinUI skip 末尾分支**(向后兼容 focus-18 v0.3.1)
8. **Task 5 不引入 WebSocket,简化为 /health 字段 + extension 启动读一次**

### 不必改

其余(§1 表)已确认 OK,按 Claude 的方案执行即可。

---

## 6. 给用户的核心建议

Claude 这次诊断方向正确(WinUI ≠ Chromium),3 个修复选项也合理。但:

**短期**:Option B (skip + notify) 是**务实但 unsatisfying** 的方案 — Edge 用户装了 focus-19 后,Save As dialog 仍然不会 sync,只是不再被污染。

**中期**:Option A (NavigationView 多步导航) 是**真正能 work 的方案**,但需要 focus-20+ 单独 sprint。

**立刻可用 workaround**:Edge legacy file picker flag(`edge://flags/#edge-legacy-file-picker`),**零代码改动**就能让 focus-19 支持 Edge Save As。

**建议优先顺序**:
1. **加 Option E**(零代码 + 立刻见效)→ focus-19 包含
2. **加 Option D**(winui path + Enter 试探,cheap shot)→ focus-19 包含
3. **改分类器**(enum DialogKind)→ focus-19 包含
4. **实现 Option A**(NavigationView 多步导航)→ focus-20+
5. **§6 R5**:v0.3.1 升 v0.3.2 时文档明示 WinUI 限制 + 推荐 Edge legacy flag

是否需要我:
1. 进一步细化 Option D / Option E 的具体实现?
2. 写一个改进版 focus-19 plan?
3. 帮 Claude 把 Option A(NavigationView)调研更清楚?
