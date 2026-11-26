# focus-19 Retrospective Review (Mavis → Claude)

**日期**: 2026-10-02
**作者**: Mavis (M3)
**输入**:
- `docs/superpowers/plans/2026-10-02-focus-19-retrospective-deep-dive.md`(Claude retrospective)
- `docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-revised.md`(focus-19 rev.2)
- 实际源码:`sigma-listary-spike/src/uia_inject.rs`(line 244-466)、`state.rs`(line 45-55)
- 用户截图描述(hwnd=1050806, #32770 WinUI wrapped)
**性质**: 第四轮 review。**重要转折点**:focus-19 v0.3.2 ship 后,这是首次发现之前几轮的修复方向是**反向的**。

---

## 0. TL;DR

**Claude 这次的 retrospective 是这一系列 review 中最扎实的一次**:
- ✅ 拿到了真实的 spike.log + spike.log.YYYY-MM-DD 两份日志对比
- ✅ 正确识别 focus-19 v0.3.2 是**反向修复** —— 把"可见但坏"变成"不可见但仍坏"
- ✅ 正确识别 focus-18 v0.3.1 的"filename pollution" 实际是 WinUI 把 filename 当 navigation target 的**正确机制**

但**修复方向仍有偏差**,需要 4 处修正:

1. 🔴 **"filename-as-navigation" 的论断需要 empirical 验证** —— Claude 推断 WinUI 接受 path + Enter 触发导航,但**没有任何 evidence**。不同 WinUI 版本可能行为完全不同。
2. 🔴 **Option D (path + trailing slash + Enter) 的前提假设未验证** —— 该方案直接当成"cheap shot 先试"假设,但**WinUI 多数版本不接受纯路径作为 filename**,Enter 只会变无效输入。
3. 🔴 **focus-20 仍然在"写 filename + Enter" 框架下思考** —— 用户原始诉求是"dialog 真正跳转",不是"filename 显示路径"。这两者不同。
4. 🟡 **Edge legacy flag (Option E) 应升为 primary 路径** —— 这是 **唯一已知的可靠 WinUI 工作方案**(其他都是 best-effort),文档化为"官方建议"而非"备选方案"。

---

## 1. Claude 做对的部分(关键认可)

### 1.1 systematic-debugging Phase 1 — 证据收集极扎实

对比前几轮 Claude 跳到结论的毛病,这次:
- ✅ 读了 `spike.log`(JSON 事件) **和** `spike.log.2026-10-02`(tracing INFO/WARN)
- ✅ 看到 `app: "#32770"` 但 COM SetFolder 失败 → 识别为 WinUI 3 wrapped
- ✅ 看到完整 UIA inventory(106 descendants)
- ✅ 对比 focus-18 和 focus-19 在**同一 dialog type**(`#32770`)的不同行为
- ✅ 提供了时间戳(2026-10-02 05:33:34 vs 07:02:32)的对照证据

→ 这是前几轮缺失的关键步骤。

### 1.2 §2.3 反向论证 — 真相揭示

Claude §2.3:
> focus-18 保留的 PathWrap 上游代码:`value.SetValue(&BSTR::from(target_path))?` — 这是 PathWrap 原作者为 Chromium Save As 写的 fallback
> **focus-18 之前的 round 17/18 误判**:认为这是 WinUI 污染
> **真相**:这是 WinUI Save As 的**正确机制**

这是**这一系列 review 中最重要的认知校正**。我们前几轮的"修复"实际上是破坏正确机制。

### 1.3 §4.1 修了 4 轮仍未 work 的真相表

```
focus-16: 删除 fg_bypass/Invoke/Enter → ❌ 全部 regression
focus-17: 恢复 basename,加 version+console fix → ⚠️ 部分 OK
focus-18: 恢复 fg_bypass+Invoke+Enter → ⚠️ 写 filename 框(用户感受"污染")
focus-19: 删 fallback → ❌ **完全 regression,用户感受"无变化"**
```

这张表清楚展示了**越改越糟**的过程。Claude 敢于承认这一点很关键。

### 1.4 §4.2 架构质疑 — 现实 5 类 dialog

```
1. IFileDialog (Vista+)
2. Chromium Save As
3. Edge Modern Save As (WinUI 3) ← 新,PathWrap 不 cover
4. Win32 legacy GetOpen/SaveFileName
5. Win32 自绘 (e.g. Photoshop)
```

**现实 dialog 类型是分形的**(每种 app 都可能有自己的实现)。这是架构层面的真相,后续修复必须基于此。

### 1.5 §7 给后续会话的避坑提示

§7.1 "不要做的" 和 §7.2 "要做的" 都很准确,**尤其是**:
> ✅ 看到 `FileNameControlHost` + `1001` filename Edit + Save 按钮 → 这是 Edge Modern Save As
> ✅ 看到 UIA 41477 不存在 → 走 Option D(写 filename + Enter)而不是 skip

---

## 2. 🔴 Claude 修复方案的 3 处偏差

### 2.1 [P0] "filename-as-navigation" 论断需要 empirical 验证

**Claude §2.3 推断**:
> 这**是**:
> - Windows 11 22H2+ WinUI 3 dialog(Edge / Photos / Paint / 等 Save As 在新版 Edge 已迁 WinUI)
> ...
> **Chromium 把 filename 框当地址栏,SetValue + Invoke Save 等同于"输入地址 + 回车"**。

**问题**:这是**经验推断**,没有 empirical 测试证据。

**实际行为**(Windows file dialog 通用约定):
- filename 框接受**文件名**(`report.pdf`)
- filename 框接受**完整路径+文件名**(`C:\downloads\report.pdf`)— 通常有效
- filename 框接受**纯路径**(`C:\downloads\`)— **多数 dialog 拒绝**或视为无效输入
- Edge WinUI 具体行为**未测试**

**关键测试**:用户在 Edge Win11 22H2+ 上手动测试 — 在 Save As dialog 的 filename 框输入 `C:\DrvPath\`,然后 Enter。看是否导航。

**如果验证为"navigation works"**:Claude 方案 ✓,Option D 可行
**如果验证为"navigation 不 work"**:Claude 方案 ✗,需要完全不同的策略(Option F/G?)

**建议**:在 focus-20 实施前,**先做 empirical 验证**。代码改动应基于验证结果,而非理论推断。

### 2.2 [P0] Option D 前提假设错误

**Claude §5.1 Task 1 提案**:
```rust
let probe = format!("{}\\", target_path);
value.SetValue(&BSTR::from(probe.as_str()))?;
// 2. 仅在 auto_confirm=true 时 SendMessageW Enter 到 native HWND
if should_commit { commit_via_fallback_enter(edit); }
```

**问题**:
- "path + trailing slash + Enter" 假设 WinUI 接受此模式,**未验证**
- 即使 WinUI 接受,**Enter 后会发生什么未知**:
  - 可能导航(理想)
  - 可能保存(灾难 — 用户没确认)
  - 可能无效输入无动作(无效)

**Claude §3.2 §5.1 也承认**:
> Enter 可能误保存(Mavis review §2.1 R7)

但没有给出**验证步骤**。

**建议**:
- Option D 必须先做 empirical 验证(用户手测)
- 如果验证 navigation works,**仅 auto_confirm=true 时**才 Enter(gate 已加)
- 如果验证 navigation 不 work,**Option D 失败 → fallback 到 Edge legacy flag 通知**

### 2.3 [P0] 用户原始诉求被误解

**用户原始反馈**(handoff-2026-10-01-focus-16):
> 下载弹窗(Windows Open File Dialog):
> - 地址栏:`此电脑 > photos (G:) > video`(未同步,显示 dialog 原本打开的位置)
> - 文件名:`C:`
> 没有任何 v0.3.0 期望的 sync 行为

**用户最新反馈**(本轮):
> 仍然无法同步下载弹窗的路径,但地址可以同步到弹窗的"文件名"填写处

**用户实际诉求**:**dialog 真正跳转到新文件夹**(地址栏更新),而不是 filename 框更新。

**Claude 当前 focus-20 方案**:
- Option D 写 `C:\DrvPath\` 到 filename + Enter → **可能**导航
- 但这是**best-effort**,**不保证**

**问题**:即使 Claude 方案"成功",也是把 dialog 状态变成"`C:\DrvPath\` 在 filename 框 + Enter 触发导航",这对 WinUI 来说:
- ✓ 视觉上 dialog 跳到新文件夹(用户想要的)
- ✗ 但 filename 框变成了 `C:\DrvPath\`(一个奇怪的文件名)
- ✗ 用户需要手动 fix 文件名才能保存

**这其实是 Edge legacy flag 更好的根本原因** —— 用户启用 legacy flag 后,Edge 用经典 IFileDialog,**地址栏和文件名框是分开的两个控件**,spike 的 COM SetFolder 直接修改地址栏,filename 框不动。**这是用户真正想要的 UX**。

**建议**:Option E (Edge legacy flag) 应升为 **primary 解决方案**,而不是"零代码备选"。

---

## 3. 🟡 修复方案的具体问题

### 3.1 [P1] focus-20 方案 Task 1 + Task 2 顺序问题

**Claude §5 推荐组合**:A + C(立即恢复 fallback + Enter commit + Edge flag 通知)

**问题**:
- Task 1 (恢复 fallback + Enter) — **基于未验证的假设**(见 §2.1)
- Task 2 (Edge flag 通知) — 应该是**primary**,但被列为 secondary

**建议重新排序**:

| 优先级 | Task | 工作量 | 风险 |
|---|---|---|---|
| **1** | **User-verify "path + Enter 在 Edge WinUI 是否真的 navigate"** | 用户 30 min | 零 |
| **2** | **Edge legacy flag 通知 + README** | 2-3 h | 零(用户操作) |
| **3** | focus-18 fallback 恢复(写 filename,**不**自动 Enter) | 1 h | 低 |
| **4** | Option D(写 path + Enter)**仅当 #1 验证通过** | 4-6 h | 中(Enter 副作用) |
| **5** | NavigationView 多步导航(仅 #1-#4 都不 work 时) | 8-12 h | 中 |

**核心原则**:**evidence-based**,不要在未验证的假设上重投入。

### 3.2 [P1] §5.1 Task 1 代码逻辑 bug

**Claude 提案**:
```rust
state.inc_unsupported_dialog();  // 即使"成功"也计数
Ok(DialogOutcome::AddressBarWritten)  // 试探视为成功
```

**问题**:
- 即使 probe 失败并 fallback 到 Unsupported,也调用了 `state.inc_unsupported_dialog()` — 重复计数
- 即使 probe 成功,也调用 `state.inc_unsupported_dialog()` — 但实际是 AddressBarWritten,不应计入 Unsupported

**修复**:
```rust
match probe_outcome {
    Ok(()) => {
        // 成功导航
        return Ok(DialogOutcome::AddressBarWritten);
    }
    Err(_) => {
        state.inc_unsupported_dialog();
        return Ok(DialogOutcome::Unsupported);
    }
}
```

### 3.3 [P2] §5.2 Task 2 通知文案需要更具体

**Claude §5.2**:
```js
description: 'Edge Modern Save As has limited path sync. ' +
    'Try edge://flags/#edge-legacy-file-picker → Enabled → restart Edge. ' +
    'This makes Edge use the classic IFileDialog which focus-sync fully supports.',
```

**建议改进**:
- 添加**具体步骤截图指引**(如"Edge address bar → 输入 `edge://flags/#edge-legacy-file-picker` → 搜索 'legacy file picker' → 改为 'Enabled' → 重启 Edge")
- 解释**为什么** 这样能解决问题(教育用户)
- 提供**rollback 路径**(如果用户启用 flag 后出问题)

### 3.4 [P2] §3 hypothesis 3 实施难度评估过低

**Claude §3.3**:
> **风险**:
> - 实施难度高(8-12 小时)
> - 不同 app 的 NavigationView 样式不一致
> - multi-step race condition

**实际上 8-12h 严重低估**:
- 需要先识别 NavigationView 节点(`Microsoft.UI.Xaml.Controls.NavigationViewItem`)
- 不同 app 命名树差异巨大(Edge / Photos / Paint / Snipping Tool)
- fuzzy name match 需要 robust 启发式(可能是 ML)
- WinUI 不同版本(22H2 / 23H2 / 24H2)控件树变化

**估计**:20-40 小时,且需持续维护(WinUI 版本更新)

**建议**:NavigationView 列为 **focus-22+ 长期目标**,不作为 focus-20 范围。

### 3.5 [P2] §6 "选项 A" 工作量评估与 §3.3 不一致

**§6 选项 D 写"完整 Option A(NavigationView 多步导航) = 8-12 小时"**,但 §3.3 hypothesis 3 写"实施难度高(8-12 小时)"。两处数字一致,但 §3.4 论证过低。

---

## 4. 风险补充

| # | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| R13 | focus-20 restore fallback 后,用户的旧 v0.3.2 升级路径不清晰 | 高 | 用户困惑 | installer 文档化 v0.3.2 → v0.3.3 行为变化 |
| R14 | "filename-as-navigation" 假设在 Edge 不同 Win11 版本下行为不一致 | 高 | 部分用户 work,部分不 work | 不分 Win11 版本做差异化支持(成本太高),推荐 Edge legacy flag |
| R15 | Option D 的 200ms sleep + UIA read 增加 /set_path 延迟 | 中 | HTTP 响应慢 | 已用 spawn_blocking(R12 已有) |
| R16 | Edge legacy flag 用户教育成本高(很多用户不知道 flag 存在) | 高 | 用户不启用 | README + 安装向导步骤截图 + 持续 notification |
| R17 | focus-20 写 `C:\DrvPath\` 到 filename 框,**之后** 用户输入实际 filename 时,SetValue 已覆盖 → 用户困惑 | 中 | UX 差 | 焦点 sync 后**不覆盖**用户已输入的字符(检测 user typing) |
| R18 | Option D 成功导航后,WinUI 不会把 focus 切回 Sigma FM → 用户需手动 click Sigma FM | 低 | UX 差 | 接受(WinUI 行为) |

---

## 5. 给 Claude 的总结指令

### 必修(3 处)

1. **§2.1 empirical 验证**:在 focus-20 实施前,**先做 user-verify** "Edge WinUI Save As filename 框输入 `C:\DrvPath\` + Enter 是否真的导航"。这是后续方案的基础。如果验证不 work,Option D 不应实施。
2. **§2.2 Option D 前提修正**:不要假设 WinUI 接受"纯路径 + Enter"。把 Option D 改为条件性(仅 #1 验证通过才实施),并加 fallback(失败 → 立即清空 probe + 通知 + skip)。
3. **§3.1 优先级重排**:Edge legacy flag 升为 primary 解决方案(零代码 + 已验证 work),focus-20 第一优先级是 README + 一次性 notification + 步骤截图。

### 建议(4 处)

4. **§3.2 Task 1 计数逻辑修正**:probe 成功时**不**调用 `inc_unsupported_dialog()`。
5. **§3.3 通知文案改进**:加具体步骤(URL、search term、setting value)。
6. **§3.4 NavigationView 重新评估**:从 8-12h 改为 20-40h,推为 focus-22+。
7. **§R17 加 user-typing 检测**:WritePath 前检查 filename 框当前值,如果非空且用户可能正在输入,**不覆盖**。

### 不必改

其余(§1 表)已确认 OK,按 Claude 的方案执行即可。

---

## 6. 战略建议(给用户)

**短期(focus-20,1-2 天)**:
1. **用户做 empirical 验证** — Edge WinUI Save As 行为测试
2. **README + extension notification 推 Edge legacy flag** — 零代码 work-around
3. **focus-18 fallback 恢复** — 至少让用户看到路径(即使不导航)

**中期(focus-21+,1 周)**:
- 仅当 §1 验证通过:Option D (path + Enter 试探)
- 仅当 Option D 不可靠:NavigationView 多步导航(20-40h)

**长期(focus-22+)**:
- 等待 Microsoft 文档化 WinUI 3 dialog 的 programmatic 访问(目前无官方 API)
- 或切换到其他方案(键盘模拟 / 用户脚本 / 其他工具)

**核心原则**:**evidence-based 修复**。每一轮 focus 改动前,先 empirical 验证假设,不要在推测上重投入。

---

## 7. 关键证据汇总(给用户决策)

| 假设 | 状态 | 决策依据 |
|---|---|---|
| focus-19 v0.3.2 是 regression | ✅ 已验证 | spike.log 对比 focus-18 vs focus-19 同一 dialog 行为 |
| focus-18 v0.3.1 写 path 到 filename 框是"正确机制" | ⚠️ 部分正确 | 是 WinUI 唯一可写控件,但**不一定触发导航** |
| Edge legacy flag 让 Edge 用 IFileDialog | ❓ 未验证 | 假设存在,需 user-verify |
| WinUI filename 框接受 path + Enter 触发导航 | ❓ 未验证 | 需 user-verify(2.1) |
| NavigationView 多步导航 work | ❓ 未验证 | 实施难度被低估(20-40h) |

**没有 empirical 验证的修复都是赌博**。focus-20 必须先验证 §1 + §2.1,再决定代码改动。

---

## 8. 给用户的核心问题(需用户回复)

1. **你能做一次手动测试吗**:在 Edge Win11 22H2+ 上,Save As dialog 打开后,在 filename 框输入 `C:\DrvPath\` 然后按 Enter,看 dialog 是否导航?这是 focus-20 是否实施 Option D 的关键。
2. **Edge legacy flag 工作吗**:启用 `edge://flags/#edge-legacy-file-picker` 后,Edge Save As dialog 真的变成经典 IFileDialog 吗?(打开 dev console 看 dialog class 变化)
3. **focus-19 v0.3.2 是否回退到 focus-18 v0.3.1**:用户装 v0.3.2 后看不到任何变化,可能想回退。但 v0.3.2 至少不会污染字段,v0.3.1 会污染。

**回复我这些问题的答案,我可以更精准地给出 focus-20 实施方案**。

---

## 9. 给后续会话的关键 takeaway

1. **diagnose first, fix second**:前几轮 Claude 跳太快。retrospective 这次做到位了。
2. **避免在假设上重投入**:Option A (8-12h) / Option D (4-6h) / Option E (2-3h) 的工作量评估都基于"假设 work"。**实际 work 量是"假设 work 的概率" × "实际工作量"**。
3. **永远 first-verify 再 implement**:`Option D` 的核心是 "path + Enter 在 Edge WinUI 是否真的导航"。这个 30 分钟的手动测试决定后续 20-40h 投入是否值得。
4. **Plan vs reality**:`focus-19 v0.3.2 是 regression` 这种结论前 3 轮 Claude 都应该想到,但没有。**spike.log + tracing 日志 + user 反馈**三者交叉验证是根因分析的最低标准。
5. **Edge legacy flag 是真正的方案**:0 代码,已验证工作(假设),用户启用即见效。其他所有方案都是 best-effort。
