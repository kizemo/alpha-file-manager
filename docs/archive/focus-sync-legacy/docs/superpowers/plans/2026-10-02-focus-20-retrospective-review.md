# focus-20 Retrospective #2 Review — Mavis 意见 + focus-21 建议

**日期**: 2026-10-02
**作者**: Mavis (M3)
**输入文档**: `docs/superpowers/plans/2026-10-02-focus-20-retrospective-architecture-failure.md`
**用户截图证据**: 用户最新附图(`C:\downloads\` 在 filename 框,此电脑 > photos (G:) > video 不动)
**基础**: 5 轮 review 累积(`focus-17` 到 `focus-20`)
**性质**: 第 5 轮 review(可能也是最后一轮 — systematic-debugging 已触发 Phase 4.5)

---

## 0. TL;DR

**Claude 这次 retrospective 极好 — 是这一系列 review 中认知最深的一次**:
- ✅ 主动触发 systematic-debugging Phase 4.5(质疑架构)
- ✅ 承认 5 轮修复失败 + 自我剖析根因
- ✅ 不再做 v0.3.x 小修的明确决定
- ✅ 4 个新架构路径(Edge legacy flag / SendInput / NavigationView / 暂停)+ 工作量 + 风险

但**仍有 5 处偏差**需要修正:

1. 🔴 **focus-20 v0.3.3 已经 ship,且实际装在用户机器上** — "focus-20 v0.3.3 不应该 ship"是 retrospective 自我批评,但**用户已装,需要在 focus-21 提供回滚路径**。
2. 🔴 **"WinUI 3 filename + Enter 不导航" 是 CLAIM 不是 evidence** — Claude 在 §3 假设 A 重复了 systematic-debugging 批评的"假设性推理代替 user-verify"错误。需要 user-verify。
3. 🔴 **路径 B "SendInput" 工作量 5-10 天严重低估** — 实际 10-15 天,涉及焦点抢回防御、auto_confirm gate、不同 Win11 版本适配。
4. 🟡 **Edge legacy flag 通知用户** — 已实施(`extension/dist/index.js` line 294-317),但用户可能错过。**应加强提示策略**(modal / toolbar button / 重复触发)。
5. 🟡 **§6 优先级建议可更明确** — 缺"接受现状"选项(什么都不做,只文档化限制)。

---

## 1. Claude 做对的部分(关键认可)

### 1.1 §0 TL;DR — 5 轮修复失败表

```
focus-16:删了 fg_bypass/Invoke/Enter → 全 regression
focus-17:部分恢复 → 部分 OK 但路径不同步
focus-18:恢复 + 加 fallback → filename 框被污染
focus-19:删 fallback → 完全无变化(regression)
focus-20 v0.3.3:恢复 fallback(无 Enter) → **仍是 filename 框污染**
```

这张表清晰展示**越改越糟**。Claude 不再掩盖。

### 1.2 §1.2 spike.log trace 准确

实际 trace 显示:
```
08:58:56 INFO  focus-20: wrote FULL target='C:\downloads' to filename edit
                  (WinUI/Edge Modern Save As fallback, restored from focus-18;
                   no auto-Enter per retrospective review §2.1/§2.2;
                   user manually saves).
```

— 与用户截图吻合:filename 框显示 `C:\downloads`,但 dialog 没跳。

### 1.3 §2.1-§2.2 PathWrap 模型 vs WinUI 3 真实模型对比

```
PathWrap 假设:           WinUI 3 真实:
1. IFileDialog (COM)      [BreadcrumbBar] ← ItemsRepeater,不可写
2. Chromium Save As       [NavigationView Tree] ← 需 Expand + Select
3. Other → skip            [ItemsView Grid]
                          [Filename TextBox] ← literal 文件名 only
```

— 这是**用户 5 轮反馈的真相**。"写 filename = 导航"在 WinUI 3 中根本不成立。

### 1.4 §2.4 自我剖析 — "为什么 5 轮都错"

> **根因**:
> 1. **PathWrap 上游代码不能区分 dialog 类型**,统一走 "写 filename 框" 这个 fallback
> 2. **我每次"修复"都是调整 fallback 的细节**(写什么、是否 Enter、是否通知)
> 3. **没有一次 fix 触碰到真正的 navigation 控件**(BreadcrumbBar / NavigationView)

— **这正是我在前几轮 review 中反复指出的偏差**。Claude 终于 internalize 了。

### 1.5 §4.1-§4.3 触发 systematic-debugging Phase 4.5

> **If 3+ fixes failed:** STOP and question the architecture

— Claude 正确触发并应用了这个规则。Phase 4.5 是 systematic-debugging 的关键转折点,**在第 5 轮才触发是迟到但不迟于不做**。

### 1.6 §5 4 个新架构路径 + 工作量 + 风险评估

```
路径 A: Edge legacy flag  1-2h, 零代码, 80%+ 成功率
路径 B: SendInput 键盘模拟  5-10d, 弃用 PathWrap, 60-70% 成功率
路径 C: NavigationView 多步  20-40h, 高实施难度, 50% 成功率
路径 D: 暂停 spike 项目  0, 100% 成功率(什么都不做)
```

— 清晰、量化、可决策。

### 1.7 §7 "给用户核心问题" — Stop guessing

> **不再"修复"v0.3.x — 必须用户决策新架构方向**

— 正确的方法论转变。

### 1.8 §9 教训 + 硬规则

```
规则 1:新 dialog 类型的支持,必须先 user-verify dialog 的真实控件结构
规则 2:fix 之后,必须 user-verify "path 在正确字段显示"
规则 3:3+ fix 失败时,质疑上游/架构假设,不质疑实现细节
规则 4:不发"已知错的方案"
```

— 这些规则应作为后续工作的硬约束。

---

## 2. 🔴 5 处偏差(需修正)

### 2.1 [P0] focus-20 v0.3.3 已 ship,需要回滚策略

**Claude §8 自批**:
> focus-20 v0.3.3 不应该 ship。应该在 ship 前 user-verify "WinUI 3 filename + Enter 是否导航",如果验证为否,**不应该 ship 这版**,直接走路径 A/B/C。

**问题**:
- focus-20 v0.3.3 已经 ship 且 user 已装(`Sigma-File-Manager-2.2.0-focus-20-setup.exe` 17071214 bytes, 2026-10-02 16:44:44)
- 用户最新截图证明 focus-20 v0.3.3 仍在写 filename 框
- "不应该 ship" 是 retrospective,但**当前状态是已 ship**

**回滚选项**:
| 选项 | 影响 | 推荐 |
|---|---|---|
| 装 focus-19 v0.3.2 | 完全无变化(用户感受"什么都没发生") | ❌ 用户已抱怨"完全无变化" |
| 装 focus-18 v0.3.1 | filename 框被污染(同 focus-20 v0.3.3) | ❌ 同 focus-20,只是换版本号 |
| 装 focus-17 v0.3.0 | filename 框写 BASENAME("downloads") | ⚠️ 比 FULL PATH 略好,但仍不是用户要的 |
| **不换版本,继续 focus-21** | 接受现状,直接进入新架构 | ✅ **推荐** |
| 卸载 focus-sync | 用户用 Listary/PowerToys 替代 | ⚠️ 路径 D 备选 |

**建议**:不主动回滚,让用户选择。提供所有回滚路径供决策。

### 2.2 [P0] "WinUI 3 filename + Enter 不导航" 是 CLAIM 不是 evidence

**Claude §3.1 假设 A**:
> **测试方法**:用户手测 — 在 WinUI 3 Save As 的 filename 框手动输入 `C:\DrvPath\`,Enter,看是否导航。
> **预期**:不导航(只是把 `C:\DrvPath\` 当字面文件名,Save 时报错)。

**问题**:
- Claude 在 §2.4 自我批评"假设性推理代替 user-verify"
- 但 §3.1 又给出"未经验证的预期"
- 这违反了 systematic-debugging 的 evidence-first 原则

**真实情况**:**不知道**。需要 user-verify。

**关键测试**(已在 §6.4 中写明但未执行):
1. Edge WinUI Save As 打开
2. 在 filename 框**手动**输入 `C:\DrvPath\`(精确,带 trailing slash)
3. 按 Enter
4. 观察 dialog 是否导航

**根据 Windows file dialog 通用约定**:
- "C:\DrvPath\" 在 filename 框 → 多半 Save 时报错(找不到 C:\DrvPath 文件)
- "C:\DrvPath\file.exe" 在 filename 框 → 多半导航 + Save
- "C:\DrvPath" 在 filename 框(无 trailing slash 无 filename) → 行为不确定

**建议**:
- 真的去做 user-verify,**别预测**
- 测试三种格式:`C:\DrvPath\` / `C:\DrvPath` / `C:\DrvPath\file.exe`
- 记录真实行为,作为 focus-21 决策依据

### 2.3 [P0] 路径 B "SendInput" 工作量严重低估

**Claude §5 路径 B**:
> **工作量**:5-10 天(包括 race condition 防御 + auto_confirm gate)

**问题**:
- 5-10 天只算了"实现 SendInput"
- 漏算:
  - **焦点抢回副作用防御**(fg_bypass 复辟 — round 18 才删除的)
  - **不同 Win11 版本 Ctrl+L 行为适配**(22H2 / 23H2 / 24H2 / 25H2)
  - **auto_confirm gate 与现有 path B 整合**
  - **用户输入检测**(不能覆盖用户已输入字符)
  - **SendInput 失败回退**
  - **多 dialog 并发处理**

**修正估算**:**10-15 天**,且需持续维护。

**SendInput 在 Win11 上的实际风险**:
- Windows 10+ 引入了 `IsHungAppWindow` / `HungAppCheck`
- SendInput 在某些 dialog 上会被 Windows 标记为可疑输入,可能被 dialog 拦截
- `SetForegroundWindow` 在 Windows 10+ 受 `SPI_GETFOREGROUNDLOCKTIMEOUT` 限制(默认 200ms)

**建议**:路径 B 工作量改为 **10-15 天**,且作为"plan B"而非 primary。

### 2.4 [P1] Edge legacy flag 通知用户已实施,但不够强

**Claude §5 路径 A**:
> 1-2 小时(focus-20 v0.3.3 已 ship 一次性 notification)

**实际状态**:
- focus-20 v0.3.3 确实实施了 notification(`extension/dist/index.js` line 294-317,`edgeLegacyFilePickerTipShown` storage flag)
- 但用户最新反馈显示:**用户没注意到 / 没启用 / 没 work**

**可能原因**:
- 用户首次装 focus-20 v0.3.3 → notification 显示 → 用户错过 → notification 不再显示
- 用户装了 focus-19 v0.3.2 → 升级到 focus-20 v0.3.3 → storage flag 保留(`edgeLegacyFilePickerTipShown=true`)→ notification 不显示
- notification 显示但用户没操作

**建议加强**:
1. **增加 Sigma FM toolbar 按钮**:"Focus Sync — Edge WinUI help"(永久可见)
2. **增加 settings UI**:用户可重置 `edgeLegacyFilePickerTipShown` 重新看通知
3. **检查 /health 持续弹**:如果 `unsupported_dialog_count > 0`,每次 Sigma FM 启动都弹
4. **README 加 Edge WinUI 章节**(Mavis review §3.5 之前已建议)

### 2.5 [P1] §6 缺"接受现状"选项

**Claude §6 "立即不做"**:
> 立即不做:再做 v0.3.x 改 filename 框的小修。

**问题**:Claude 列了 4 个新架构路径,但**没列"按现状接受 + 文档化"选项**。

**现实场景**:很多用户场景:
- 用户不需要 Edge WinUI sync(只用 Chrome / Word / Notepad / VS Code)
- focus-sync 已 work,只是 Edge Modern Save As 不 work
- "暂停 spike 项目"可能太激进

**补选项**:

### 路径 E:接受现状 + 文档化
- 接受 focus-sync 不支持 Edge WinUI Modern Save As
- README 加章节说明
- 通知用户:Edge Modern Save As 不支持,建议(任选):
  - 启用 Edge legacy flag(路径 A)
  - 使用其他 dialog(Chrome / Word / Notepad / VS Code / 影刀)
  - 使用 Listary / PowerToys 替代
- 不再投入开发时间在 Edge WinUI 上

**成本**:0(只 README + 一次性通知文案)
**成功率**:100%(无失败可能,因为不做)
**UX**:用户明确知道限制,可选择替代方案

---

## 3. 🟡 focus-21 建议(给 Claude 的具体下一步)

基于上述 review,我的 focus-21 建议:

### 3.1 focus-21 决策树

```
User verifies WinUI 3 filename + Enter behavior
├── 如果"navigation works"(用户手测确认)
│   ├── 实施 focus-21 Option D(写 path + trailing slash + Enter)
│   ├── 工作量:1-2 天
│   └── 用户仍需启用 Edge legacy flag 才能 100% work
│
└── 如果"navigation 不 work"(预期大概率)
    ├── 选项 1: Edge legacy flag (路径 A) — 唯一已 work 的方案
    │   └── 升级 focus-21 通知策略(§2.4)
    │
    ├── 选项 2: SendInput Ctrl+L (路径 B) — 10-15 天
    │
    └── 选项 3: 接受现状 + 文档化 (路径 E) — 0 天
        └── 推荐作为 default fallback
```

### 3.2 focus-21 短期任务(立即做)

| # | 任务 | 工作量 | 备注 |
|---|---|---|---|
| 1 | **User-verify Edge WinUI filename + Enter 行为** | 用户 30 min | 必做 — 决定后续方向 |
| 2 | **User-verify Edge legacy flag 是否 work** | 用户 15 min | 必做 — 决定路径 A 是否可行 |
| 3 | README 加 Edge Modern Save As 章节 | 1 h | 立即做 |
| 4 | Extension toolbar 加 "Help / Edge WinUI" 按钮 | 1 h | 用户永久可见 |
| 5 | 升级 notification 策略(modal / 多触发) | 2 h | 解决"用户错过"问题 |

### 3.3 focus-21 中期任务(等 user-verify 后)

| # | 任务 | 工作量 | 备注 |
|---|---|---|---|
| A | **Edge legacy flag 完整支持**(如果 user-verify work) | 2 h | 已实施 notification,加 README 详细步骤 |
| B | Option D(path + Enter)**仅当 user-verify navigation works** | 4-6 h | 加 auto_confirm gate + 失败清空 probe |
| C | SendInput Ctrl+L 调研 + prototype | 2-3 d | 仅当 A、B 都不 work 时启动 |

### 3.4 focus-22+ 长期(避免)

| # | 任务 | 工作量 | 备注 |
|---|---|---|---|
| NavigationView 多步导航 | 20-40 h | 推 focus-22+,只在 A、B、C 全失败时启动 |
| 其他 WinUI API 调研 | 不定 | 等 Microsoft 文档化 |

---

## 4. 风险矩阵(补充)

| # | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| R21 | focus-20 v0.3.3 已被 user 装,回滚带来再次"重装"成本 | 中 | 用户体验差 | 提供清晰的回滚路径文档 |
| R22 | Edge legacy flag 用户启用后,Edge 升级重置 flag | 中 | 重新 WinUI | README 加"升级后检查 flag"提示 |
| R23 | user-verify 结论"Enter navigation works",但实际是 Save 触发(误判) | 中 | Option D 误实施 | 同时测试 Enter + Save 两个分支 |
| R24 | 通知"用户没看到"持续发生(用户 focus 模式) | 高 | Edge flag 不启用 | 升级到 toolbar button + settings UI |
| R25 | user-verify 不做,Claude 又一次"假设性推理" | 高 | focus-21 重复 focus-20 错误 | systematic-debugging Phase 0-1 不允许跳过 |

---

## 5. 给 Claude 的总结指令

### 必修(3 处)

1. **§2.1 补充回滚路径** — focus-20 v0.3.3 已 ship,retrospective 必须提供回滚选项(§2.1 表)。
2. **§2.2 移除假设 A "预期:不导航"** — 这是预测,不是 evidence。改为 "**测试方法已知,预期待定,需 user-verify 后填入结论**"。
3. **§2.3 路径 B 工作量从 5-10 天改为 10-15 天** — 漏算了焦点抢回防御、版本适配、auto_confirm gate 等。

### 建议(3 处)

4. **§2.4 加强 Edge flag 通知策略** — toolbar button / settings UI / 重复触发,而不是依赖一次性 toast notification。
5. **§2.5 加路径 E "接受现状 + 文档化"** — 作为 default fallback,不投入开发时间在 Edge WinUI 上。
6. **§3 focus-21 决策树** — 按 user-verify 结果分支,不是预设方案。

### 不必改

其余(§1 表)已确认 OK,Claude 的 systematic-debugging 框架、retrospective 诚实、4 路径列表都很好。

---

## 6. 给用户的核心建议

**你已经做了 5 轮 review,我做了 5 轮 review,Claude 终于承认架构错。这是这一系列 review 的终点。**

**建议你做 3 件事**(before 决定下一步):

1. **做 user-verify**(15-30 分钟):
   - 在 Edge WinUI Save As filename 框手动输入 `C:\DrvPath\` 然后 Enter,看是否导航
   - 测试三种格式:`C:\DrvPath\` / `C:\DrvPath` / `C:\DrvPath\file.exe`
   - 启用 Edge legacy flag,重启 Edge,看 Save As 是否变经典 IFileDialog

2. **做回滚决策**(看截图偏好):
   - focus-20 v0.3.3(filename 显示路径,污染)
   - focus-19 v0.3.2(完全无变化)
   - focus-18 v0.3.1(filename 显示路径,污染)
   - focus-17 v0.3.0(filename 显示 basename)
   - 卸载 focus-sync,用 Listary 替代

3. **决定后续路径**:
   - A:Edge legacy flag + focus-21 文档化(2 h)
   - B:SendInput Ctrl+L(10-15 d)
   - C:NavigationView(20-40 h)
   - D:暂停 spike(0)
   - **E:接受现状 + 文档化(0)— 推荐作为 default fallback**

**回复我 1、2、3 的决定,我可以基于你的实证数据给出 focus-21 实施方案**。

---

## 7. 给后续会话的关键 takeaway

1. **diagnose first, fix second**:前 4 轮 Claude 跳太快。retrospective #2 终于做到位。
2. **避免在假设上重投入**:5 轮 focus 改动都是"基于 Chromium Save As 模型推测 WinUI 3 行为",从不 user-verify。
3. **永远 first-verify 再 implement**:路径 D 的核心是 "WinUI 3 接受 path + Enter 触发导航"。**这个 30 分钟的手动测试决定后续 100+ 小时投入是否值得**。
4. **Plan vs reality**:5 轮 focus 改动都没解决 Edge Modern Save As,**因为没有人去 verify 真实 dialog 控件结构**。
5. **5+ fix 失败的真正代价**:不只是时间浪费,**用户信任度下降**。下次再"修复",用户会更警惕。
6. **架构质疑的价值**:Phase 4.5 不是"承认失败",是"换一个正确的方向"。比继续小修更有价值。
