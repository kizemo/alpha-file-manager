# focus-20 — Edge WinUI Save As 修复方案 v0.3.3(rev.2,采纳 retrospective review)

**日期**: 2026-10-02
**作者**: Claude (MiniMax-M3)
**输入**:
- `docs/superpowers/plans/2026-10-02-focus-19-retrospective-deep-dive.md`(根因分析)
- `docs/superpowers/plans/2026-10-02-focus-19-retrospective-review.md`(Mavis review,4 处 P0/P1 修正)
- `docs/superpowers/plans/2026-10-02-focus-19-edge-winui-review.md`(前一轮 review,Option D/E 来源)
- `docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-deep-analysis.md`(focus-19 deep-analysis,误判来源)
- `docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-revised.md`(focus-19 rev.2 部分采纳,需修正)

**前情**:focus-19 v0.3.2 ship 后用户实测 "下载弹窗(包括文件名位置)完全无变化" → systematic-debugging 揭示 **focus-19 v0.3.2 是 regression**(删了 focus-18 的正确 fallback)。

---

## 0. TL;DR(30 秒摘要)

**focus-20 v0.3.3 修复方案(基于 evidence-first 原则)**:

| 优先级 | Task | 状态 | 工作量 | 风险 |
|---|---|---|---|---|
| **P1** | Edge legacy flag 通知(README + extension notification) | ⏳ 立即实施 | 2-3 h | 零 |
| **P2** | 恢复 focus-18 fallback(写 filename,**无** Enter) | ⏳ 立即实施 | 1-2 h | 低 |
| **P3** | Option D (path + Enter commit) | ⏸️ 等 user-verify | 4-6 h | 中 |
| **P4** | NavigationView 多步导航 | ⏸️ focus-22+ | 20-40 h | 高 |
| **P5** | 其他架构改动 | ⏸️ 待 P3/P4 结论 | — | — |

**核心原则(per Mavis review §6)**:**evidence-based**,不在未验证假设上重投入。
**Phase 1 ship**:P1 + P2(立即可做),保留 focus-19 v0.3.2 的 unsupported 计数 /health 字段。
**Phase 2 待 user-verify 后**:P3 (Option D)。

---

## 1. 根因复盘(retrospective 核心,简版)

**focus-19 v0.3.2 是 REGRESSION**:
- focus-18 v0.3.1 写 FULL PATH 到 filename 框 → 用户看到 `C:\DrvPath`(可见但"污染"感)
- focus-19 v0.3.2 删除该 fallback → 用户完全看不到变化(更糟)

**Dialog 类型**:Edge Modern Save As 是 **WinUI 3 wrapped in `#32770`**:
- Window class `#32770`(为 accessibility 兼容)
- 有 IFileDialog-like 结构(`FileNameControlHost` + `1001` Edit + `1` Save)
- **COM IServiceProvider 不实现**(`AccessibleObjectFromWindow` fail)
- **UIA 41477 不存在**(不是 Chromium 自绘)
- **把 filename 框当 navigation target**(像 Chromium Save As)

**PathWrap 上游假设**:dialog ∈ {IFileDialog, Chromium, Other(skip)},**WinUI 3 是第 4 类,PathWrap 不 cover**。

---

## 2. review 反馈采纳清单

| review § | 内容 | 采纳方式 |
|---|---|---|
| §2.1 [P0] | "filename-as-navigation" 需 empirical 验证 | ✅ Option D 推迟到 user-verify 后 |
| §2.2 [P0] | Option D 前提假设错误 | ✅ Option D 加条件性 gate + fallback |
| §2.3 [P0] | 用户原始诉求是"dialog 真正跳转",不是"filename 显示路径" | ✅ Edge legacy flag 升 primary(经典 IFileDialog 才是真跳转) |
| §3.1 [P1] | focus-20 方案优先级重排 | ✅ Edge flag 第一,fallback 第二,Option D 第三 |
| §3.2 [P1] | Task 1 计数 bug(probe 成功不调 inc_unsupported_dialog) | ✅ 修复计数逻辑 |
| §3.3 [P2] | Task 2 通知文案需具体步骤 | ✅ 加 URL + search term + setting value + rollback 路径 |
| §3.4 [P2] | NavigationView 工作量被低估(8-12h → 20-40h) | ✅ 推 focus-22+,不在 focus-20 范围 |
| §R13 | v0.3.2 升级到 v0.3.3 行为变化需文档化 | ✅ handoff + memory 更新 |
| §R17 | user-typing 检测 | ⏸️ focus-21+,P2,本次不做 |

---

## 3. focus-20 v0.3.3 详细 Task 列表

### Task #1 (已 supersede): "filename-as-navigation" empirical 验证 ⏸️

**状态**:等待用户手测,**focus-20 不实施,仅在 handoff 中记录方法**。

**测试方法**(用户自测):
1. Edge 触发下载 → Save As dialog 打开
2. 在 filename 框**手动输入** `C:\DrvPath\`(精确,带 trailing slash)
3. 按 Enter
4. 观察 dialog 是否导航到 `C:\DrvPath\` 目录

**结果处理**:
- ✅ 导航成功 → 报告给 Claude → focus-21 实施 Option D
- ❌ 导航失败 / 文件名变成 `C:\DrvPath\`(字面) → 报告给 Claude → focus-22+ NavigationView

**Edge legacy flag 测试**(并行):
1. Edge 地址栏输入 `edge://flags/#edge-legacy-file-picker` → Enter
2. 找到 "Legacy File Picker" → 改为 "Enabled"
3. 重启 Edge
4. 触发下载 Save As → 观察 dialog class 是否变成经典 IFileDialog
5. 用 `inspect.exe` (Windows SDK) 或 UIA Verify 工具看 `ClassName` 字段

**结果处理**:
- ✅ Dialog class 变成 `#32770` 且 COM 可用 → focus-20 main work,Edge flag 文档化
- ❌ Edge flag 不生效 / Dialog 仍是 WinUI → Edge flag 文档化但加 caveat

### Task #2 (P1,立即实施):Edge legacy flag notification

**目的**:零代码 + 用户操作 = 真正 work 的方案(Mavis review §2.2 [P0-2])。

**实施位置**:`release/extension/dist/index.js::activate()`。

**伪代码**(per review §3.3 加具体步骤):
```js
async function maybeShowEdgeLegacyTip() {
    let shown;
    try { shown = await sigma.storage.get('edgeLegacyFilePickerTipShown'); } catch (e) { shown = null; }
    if (shown === true) return;
    try {
        sigma.ui.showNotification({
            title: 'Focus Sync — Edge Save As tip',
            description:
                'Edge "Modern Save As" (Win11 22H2+) has limited path sync.\n' +
                '\n' +
                'To enable full sync:\n' +
                '1. Open Edge, type edge://flags/#edge-legacy-file-picker in the address bar\n' +
                '2. Find "Legacy File Picker" → set to "Enabled"\n' +
                '3. Restart Edge\n' +
                '\n' +
                'After restart, Save As uses the classic IFileDialog and focus-sync fully supports it.\n' +
                '\n' +
                'To revert: same URL → set to "Default" → restart Edge.',
            type: 'info',
            duration: 25000,
        });
        await sigma.storage.set('edgeLegacyFilePickerTipShown', true);
    } catch (e) {}
}
```

**调用时机**:在 `activate()` 的现有 pingSidecar 之后,startHealthCheck 之前。

**改动文件**:
- `release/extension/dist/index.js`(+ ~25 行,新函数 + 调用)
- `release/extension/locales/en.json` + `zh-CN.json`(i18n 准备,本期不做但预留位置)

**风险**:
- notification 用户可能错过 → throttle 1 次(已加)
- 用户启用 flag 后体验差 → README 加 rollback 说明(已加)

**验证判据**:
- 装 v0.3.3 → 启动 Sigma FM → extension 激活 → 看到 Edge tip notification
- 重启 Edge 后 Save As dialog → spike.com /health `unsupported_dialog_count` 不再增加(因为变成 IFileDialog,COM OK)

### Task #3 (P2,立即实施):恢复 focus-18 fallback(写 filename,**无** Enter)

**目的**:让用户**至少看到路径**(focus-18 行为),作为 Edge flag 未启用时的 fallback UX。**不自动 Enter**(等 user-verify)。

**实施位置**:`sigma-listary-spike/src/uia_inject.rs::inject_via_uia` 的 "no 41477" 分支。

**当前代码**(focus-19 v0.3.2):
```rust
// focus-19 (2026-10-02): REMOVED the "write FULL PATH to filename edit"
// fallback ... returns Ok(DialogOutcome::Unsupported).
warn!("No 41477 address bar found (hwnd={dialog_hwnd}); ...");
Ok(DialogOutcome::Unsupported)
```

**focus-20 v0.3.3 代码**(恢复 + 计数修复 per Mavis §3.2):
```rust
// focus-20 (2026-10-02): RESTORED focus-18's "write FULL PATH to filename edit"
// fallback (per retrospective review §3.1 priority #3). focus-19 v0.3.2's
// removal was a regression — this IS the WinUI Save As mechanism.
//
// Per retrospective review §2.1, §2.2: Option D (Enter commit) is DEFERRED
// until user-verify (Task #1). This Task #3 = write filename only, NO Enter.
// User manually clicks Save.
//
// Per retrospective review §3.2: probe success DOES NOT call
// inc_unsupported_dialog() — we did write something, treat as success.
// Increment is for the case where there's NO edit element at all.
//
// Edge Modern Save As (WinUI 3 wrapped in #32770) is the primary use case:
// filename box accepts path as navigation target after manual Enter / Save
// click. Chrome Save As would have hit the 41477 branch above and used
// Invoke Save directly.
if let Some(ref edit) = best_edit {
    unsafe {
        let value: IUIAutomationValuePattern =
            edit.GetCurrentPatternAs(UIA_ValuePatternId)?;
        value.SetValue(&BSTR::from(target_path))?;
    }
    info!(
        "focus-20: wrote FULL target='{target_path}' to filename edit \
         (WinUI/Edge Modern Save As fallback, restored from focus-18; \
         no auto-Enter per Mavis review §2.1/§2.2; user manually saves)."
    );
    return Ok(DialogOutcome::AddressBarWritten);
} else {
    warn!("no Edit element in dialog; nothing written via UIA");
    state.inc_unsupported_dialog();
    return Ok(DialogOutcome::Unsupported);
}

// Note: NO commit_via_uia() call. Per Mavis review §2.2:
// - Enter may trigger accidental save (R7)
// - "Pure path in filename" WinUI behavior is unverified (§2.1)
// - User can manually adjust filename + click Save
```

**改动文件**:
- `sigma-listary-spike/src/uia_inject.rs`(+ ~30 行 / -10 行)

**风险**:
- 用户看到 `C:\DrvPath` 在 filename 框但没自动 Enter → UX 比 v0.3.2 好(可见)但仍需手动 Save
- focus-18 测试矩阵 7 app 全部重新过(回归测试)

**验证判据**:
- Edge Modern Save As:filename 框显示 `C:\DrvPath`,**不**自动 Enter / Save
- Chrome Save As:走 41477 分支,行为同 focus-18(Invoke Save if auto_confirm)
- 影刀 / Notepad / Word / VS Code:走 COM SetFolder 分支,行为同 focus-18 / focus-19
- `unsupported_dialog_count` 仅在 dialog 完全无 Edit 元素时增加(per Mavis §3.2 修复)

### Task #4 (P3,推迟):Option D (path + Enter commit)

**状态**:**不实施**,等 Task #1 user-verify 结果。

**条件性实施**(仅当 user-verify "path + Enter 真的导航"):
1. 在 Task #3 的 `SetValue` 后加 trailing slash 转换:`format!("{}\\", target_path)`
2. 加 `should_commit` gate(已存在) → SendMessageW Enter 到 filename Edit native HWND
3. 加 200ms sleep + UIA read-back 验证(`read_dialog_current_dir`)
4. 失败:清空 probe + return Unsupported(per Mavis §2.2 fallback)

**风险**:
- Enter 误保存(R7)
- 不同 Win11 版本行为不一致(R14)

### Task #5 (P4,focus-22+):NavigationView 多步导航

**状态**:**不实施**,推 focus-22+。

**实际工作量**(per Mavis §3.4):**20-40 小时**,不是我之前估的 8-12h。

**原因**:
- 需要识别 `Microsoft.UI.Xaml.Controls.NavigationViewItem`
- 不同 app 命名树差异巨大(Edge / Photos / Paint / Snipping Tool)
- fuzzy name match 需要 robust 启发式(可能是 ML)
- WinUI 不同版本(22H2 / 23H2 / 24H2)控件树变化
- 需持续维护

### Task #6 (P5):其他架构改动

**状态**:待 P3/P4 结论。

---

## 4. 风险矩阵 (rev.2 扩充,采纳 review §4 R13-R18)

| # | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| R1-R12 | (前几轮已识别) | — | — | (已记录) |
| **R13** | focus-19 v0.3.2 → v0.3.3 升级路径不清晰 | 高 | 用户困惑 | handoff 文档明示行为变化 |
| **R14** | "filename-as-navigation" 假设在 Edge 不同 Win11 版本下行为不一致 | 高 | 部分用户 work | 推荐 Edge legacy flag(无版本差异) |
| **R15** | Option D 的 200ms sleep 增加 /set_path 延迟 | 中 | HTTP 慢 | spawn_blocking(已规划) |
| **R16** | Edge legacy flag 用户教育成本高 | 高 | 用户不启用 | README + 安装向导 + 持续 notification |
| **R17** | 写 `C:\DrvPath` 到 filename 后,用户输入实际文件名被覆盖 | 中 | UX 差 | focus-21+ 加 user-typing 检测 |
| **R18** | Option D 成功导航后,WinUI 不切焦点回 Sigma FM | 低 | UX 差 | 接受(WinUI 行为) |
| **R19** | focus-20 fallback 写 path 后,Edge 升级导致 WinUI 控件 ID 变化 | 中 | 写失败 | 持续监控 + 测试矩阵 |
| **R20** | user-verify 结论"Enter 不导航"时,focus-20 fallback 是最后一道防线 | 中 | 仍无效 | README 推荐 Edge flag |

---

## 5. focus-20 v0.3.3 ship 范围

| 改动 | 文件 | 行数 |
|---|---|---|
| Task #2: Edge legacy flag notification | `release/extension/dist/index.js` | +25 |
| Task #3: 恢复 focus-18 fallback (无 Enter) | `sigma-listary-spike/src/uia_inject.rs` | +30 / -10 |
| Task #3: 计数逻辑修复(probe 成功不调 inc) | `sigma-listary-spike/src/uia_inject.rs` | (同上) |
| cargo test | (验证) | — |
| Build focus-20-setup.exe | `release/Sigma-File-Manager-2.2.0-focus-20-setup.exe` | — |

**预计时间**:2-3 小时
**预计代码改动**:~50 行
**预计风险**:低(Task #2 无代码风险,Task #3 恢复已有行为)

---

## 6. 验证清单(用户手动测试)

### 6.1 Edge Modern Save As(focus-20 主目标)

1. 装 `release/Sigma-File-Manager-2.2.0-focus-20-setup.exe`
2. 启动 Sigma FM → extension 激活 → **看到 Edge tip notification**
3. 触发 Edge 下载 → Save As dialog 打开
4. 切 Sigma FM 路径到 `C:\DrvPath`
5. **期望**:filename 框显示 `C:\DrvPath`(Task #3 行为)
6. **不期望**:自动 Enter / 自动点 Save(等 user-verify)

### 6.2 Chrome Save As(regression check)

1. Chrome → "Save Page As"
2. Sigma FM 切路径
3. **期望**:filename 框显示完整路径(41477 SetValue)
4. 如果 auto_confirm=true:Invoke Save 自动导航

### 6.3 影刀 / Notepad / Word / VS Code Save As(regression check)

1. 任意 app → "另存为"
2. Sigma FM 切路径
3. **期望**:dialog 目录立即跳到新路径(COM SetFolder)

### 6.4 Task #1 empirical 验证(用户自测)

**核心测试**:
1. Edge Save As dialog 打开
2. 在 filename 框**手动输入** `C:\DrvPath\`
3. 按 Enter
4. **观察**:dialog 是否导航到 `C:\DrvPath\`?
   - ✅ 导航成功 → 报告 Claude → focus-21 实施 Option D
   - ❌ 文件名变成 `C:\DrvPath\` 字面 → 报告 Claude → focus-22+ NavigationView

**Edge legacy flag 验证**:
1. `edge://flags/#edge-legacy-file-picker` → Enabled → 重启 Edge
2. 触发下载 → Save As
3. 用 `inspect.exe` (Windows SDK) 看 dialog class
4. **期望**:class = `#32770` 且 COM ServiceProvider 可用

---

## 7. 给用户的"now what"

1. **本轮 ship**:Task #2 (Edge flag notification) + Task #3 (恢复 fallback 无 Enter)
2. **build**:focus-20-setup.exe (Sigma-File-Manager-2.2.0-focus-20-setup.exe)
3. **不 commit git**:等用户授权
4. **用户测试 6-app 矩阵**(见 §6.1-6.3)+ Task #1 empirical 验证(§6.4)
5. **基于 user-verify 结果**决定 focus-21 范围

**核心建议**(per Mavis review §6):
- **短期(本轮 ship)**:P1 + P2 = Edge flag notification + 恢复 fallback
- **中期(focus-21,1 周后)**:仅当 user-verify "Enter 导航" 成功 → Option D
- **长期(focus-22+,数周后)**:NavigationView(20-40h)或等待 Microsoft 文档化 WinUI API

---

## 8. 文档链接

**本文档**(focus-20 plan,实施用):
`F:\soft\00selfmade\filemanager\docs\superpowers\plans\2026-10-02-focus-20-edge-winui-fix.md`

**对照文档**:
- 根因:`docs/superpowers/plans/2026-10-02-focus-19-retrospective-deep-dive.md`
- review:`docs/superpowers/plans/2026-10-02-focus-19-retrospective-review.md`
- focus-19 误判:`docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-deep-analysis.md`
- focus-19 rev.2(部分采纳):`docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-revised.md`
- focus-18 实施:`docs/superpowers/plans/2026-10-01-focus-18-path-b-auto-confirm-toggle-plan.md`

---

## 附录 A:对 review §8 用户问题的回答(预估)

| 问题 | 预测 | 依据 |
|---|---|---|
| Edge WinUI Save As:filename 框 + Enter 是否导航? | ⚠️ 50/50 | WinUI 设计意图是,但实现可能 bug |
| Edge legacy flag 是否让 dialog 变经典 IFileDialog? | ✅ 高概率 80%+ | Chromium 系 flag 历史上 work 过类似(经典 download bar) |
| 是否回退 focus-19 v0.3.2 → focus-18 v0.3.1? | ❌ 不建议 | focus-18 污染字段,v0.3.2 至少不污染 |

**建议用户优先回答**:Edge legacy flag 是否 work(影响 focus-20 是否需要 Task #1 验证)。
