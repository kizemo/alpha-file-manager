# focus-20 Retrospective #2 (rev.2) — 5 轮失败架构复盘 + focus-21 决策框架

**日期**: 2026-10-02
**作者**: Claude (MiniMax-M3)
**输入**:
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-architecture-failure.md`(retrospective #1)
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-review.md`(Mavis 第 5 轮 review,5 处偏差)
- 用户最新截图(`C:\downloads` 在 filename 框,此电脑 > photos (G:) > video 不动)
- spike.log.2026-10-02 (focus-20 v0.3.3 实际 trace)
**supersedes**: retrospective #1(`focus-20-retrospective-architecture-failure.md`)— 因 5 处偏差被 Mavis 标记
**性质**: 第 5 轮 review 后,文档 final + focus-21 决策框架

---

## 0. TL;DR (60 秒摘要)

**Mavis review 给的 5 处偏差,全部采纳**:

| # | 偏差 | 采纳 |
|---|---|---|
| 🔴 P0-1 | focus-20 v0.3.3 已 ship,需要回滚策略 | ✅ §6 新增回滚决策表 |
| 🔴 P0-2 | "WinUI 3 filename + Enter 不导航" 是 CLAIM 不是 evidence | ✅ §3.1 重写为"待 user-verify" |
| 🔴 P0-3 | SendInput 工作量 5-10d 严重低估 → 10-15d | ✅ §5 路径 B 工作量 + 子任务 |
| 🟡 P1-1 | Edge flag 一次性通知不够强 | ✅ §7 加强通知策略(4 个 UX 改动) |
| 🟡 P1-2 | §6 缺"接受现状"路径 E | ✅ §5 路径 E(0 天,文档化限制) |
| 🟡 P1-3 | focus-21 应按 user-verify 分支决策 | ✅ §8 focus-21 决策树 |

**focus-21 不是预设方案,是按 user-verify 结果分支的决策树**。

---

## 1. 5 轮修复失败的真相(沿用 retrospective #1)

| 轮次 | spike.log 行为 | 触碰的控件 | 用户实际诉求 |
|---|---|---|---|
| focus-16 | 删了 fg_bypass/Invoke/Enter | (无) | (无法测试) |
| focus-17 | 写 BASENAME 到 filename | Filename | BreadcrumbBar |
| focus-18 | 写 FULL PATH 到 filename | Filename | BreadcrumbBar |
| focus-19 | 删 fallback(完全无变化) | (无) | BreadcrumbBar |
| **focus-20 v0.3.3** | **写 FULL PATH 到 filename** | **Filename** | **BreadcrumbBar** |

**核心真相**:每轮 focus 都在写 **Filename 控件**(green arrow),但用户要的是更新 **BreadcrumbBar / NavigationView**(red arrow)。dialog 模型根错位。

---

## 2. systematic-debugging Phase 4.5 触发(沿用 retrospective #1)

5 轮修复失败 → 触发 Phase 4.5 → 质疑 PathWrap 架构。

**PathWrap 假设** vs **WinUI 3 真实模型**:
- PathWrap: dialog ∈ {IFileDialog, Chromium, Other},有"地址栏"可写
- WinUI 3: BreadcrumbBar(不可写) + NavigationView(Tree,可展开) + Filename(literal only)

---

## 3. 假设部分 — CLAIM 标记为待 verify(retrospective #1 修正 P0-2)

### 3.1 ~~"WinUI 3 filename + Enter 不导航"~~ → 改为待 user-verify

**retrospective #1 的错误**:
> §3.1 假设 A:
> **测试方法**:用户手测 — 在 WinUI 3 Save As 的 filename 框手动输入 `C:\DrvPath\`,Enter,看是否导航。
> **预期**:不导航(只是把 `C:\DrvPath\` 当字面文件名,Save 时报错)。

**Mavis review §2.2 指出**:
> Claude 在 §2.4 自我批评"假设性推理代替 user-verify",但 §3.1 又给出"未经验证的预期"。
> 这违反了 systematic-debugging 的 evidence-first 原则。

**修订后**:
> **§3.1 待 verify 的 CLAIM**:
> 测试方法已知(用户手测),**预期待定**,需 user-verify 后填入结论。
>
> 测试方案:
> 1. Edge WinUI Save As 打开
> 2. 在 filename 框**手动输入**以下三种格式之一:
>    - 格式 A: `C:\DrvPath\`(带 trailing slash,纯路径)
>    - 格式 B: `C:\DrvPath`(无 trailing slash,纯路径)
>    - 格式 C: `C:\DrvPath\file.exe`(路径 + 默认文件名)
> 3. 按 Enter
> 4. 观察 dialog 是否导航到 `C:\DrvPath\`
> 5. 测试完成后,记录每种格式的真实行为,作为 focus-21 Option D 的决策依据

### 3.2 假设 B: BreadcrumbBar / NavigationView 是 navigation 的正确入口

(沿用 retrospective #1,已用 spike.log inventory 验证)

### 3.3 假设 C: SendInput Ctrl+L 可绕过 UI 复杂度

(沿用 retrospective #1,标注为高风险,见 §5 路径 B)

---

## 4. Phase 4.5 架构质疑(沿用 retrospective #1,无变化)

**核心结论**:PathWrap 架构假设(IFileDialog + Chromium + Other)不适合 WinUI 3。**停止再做 v0.3.x 改 filename 框的小修**。

---

## 5. 5 个新架构路径(retrospective #1 4 个 + 新增路径 E)

### 路径 A: Edge legacy flag(已 ship 通知)

**机制**:用户启用 `edge://flags/#edge-legacy-file-picker` → Edge Save As 回退到经典 IFileDialog → spike COM SetFolder work。

**成本**:
- 工作量:1-2 小时(focus-20 v0.3.3 已 ship notification)
- 用户行动:启用 flag + 重启 Edge
- 副作用:Edge 升级可能重置 flag(R9 / R22)

**成功率**:80%+

### 路径 B: SendInput 键盘模拟(retrospective #1 5-10d → rev.2 10-15d)

**机制**:`Ctrl+L`(Win11 通用地址栏快捷键)→ SendInput 输入 path → Enter。

**retrospective #1 的低估**:5-10 天
**rev.2 修正**:**10-15 天**,因漏算:

| 子任务 | 工作量 | 风险 |
|---|---|---|
| SendInput 基础实现 | 1-2 d | 中 |
| **焦点抢回防御**(fg_bypass 复辟,被 round 18 删除) | 2-3 d | **高** |
| **Win11 多版本 Ctrl+L 适配**(22H2 / 23H2 / 24H2 / 25H2) | 2-3 d | 中 |
| **auto_confirm gate 整合**(与现有 path B 联动) | 1 d | 低 |
| **用户输入检测**(不能覆盖已输入字符,Mavis §R17) | 1 d | 中 |
| **SendInput 失败回退** + 多 dialog 并发 | 1-2 d | 中 |
| 持续维护负担 | 不定 | 高 |

**额外风险**:
- `SetForegroundWindow` 在 Win10+ 受 `SPI_GETFOREGROUNDLOCKTIMEOUT` 限制(默认 200ms)
- `IsHungAppWindow` 可能标记 SendInput 为可疑输入
- SendInput 在某些 dialog 上被 Windows 拦截

**成功率**:60-70%

**推荐**:作为"plan B",仅在 A 失败时启动。

### 路径 C: NavigationView 多步导航(沿用 retrospective #1)

**机制**:识别 NavigationView tree → 逐级 Expand + Select。

**成本**:20-40 小时(per Mavis §3.4 评估)
**成功率**:50%
**推荐**:长期,推 focus-22+,只在 A/B/E 都失败时启动。

### 路径 D: 暂停 spike 项目(沿用 retrospective #1)

**机制**:接受 spike 在 WinUI 3 上不能 work,推荐用户用 Listary / PowerToys 替代。
**成本**:0
**成功率**:100%

### 路径 E(新增):接受现状 + 文档化(推荐 default fallback)

**Mavis review §2.5**:
> 很多用户场景:用户不需要 Edge WinUI sync(只用 Chrome / Word / Notepad / VS Code)
> focus-sync 已 work,只是 Edge Modern Save As 不 work
> "暂停 spike 项目"可能太激进

**机制**:
- 接受 focus-sync 不支持 Edge WinUI Modern Save As
- README 加章节说明
- 一次性通知文案明确限制
- 提供 3 个替代方案(任选):
  - 启用 Edge legacy flag(路径 A)
  - 使用其他 dialog(Chrome / Word / Notepad / VS Code / 影刀)
  - 使用 Listary / PowerToys 替代
- **不再投入开发时间在 Edge WinUI 上**(除非用户主动要求)

**成本**:0-1 小时(只 README + 通知文案)
**成功率**:100%
**UX**:用户明确知道限制,可选择替代方案

**推荐**:**作为 default fallback**,不投入新架构开发。

---

## 6. focus-20 v0.3.3 回滚策略(Mavis §2.1 新增)

### 6.1 当前状态

| 项 | 值 |
|---|---|
| focus-20 v0.3.3 build artifact | `release/Sigma-File-Manager-2.2.0-focus-20-setup.exe`(17,071,214 bytes, MD5 `882005bb...`) |
| SPIKE_VERSION | 0.3.3 |
| 用户机器状态 | 已装,filename 框被污染 |
| 我之前自评 | "focus-20 v0.3.3 不应该 ship" |

**承认错误**:retrospective #1 写"不应该 ship"是回顾性批评,但**当前状态是已 ship**。必须给回滚路径。

### 6.2 回滚选项决策表(Mavis §2.1 表 + 风险补充)

| 选项 | 实施 | 行为 | 用户截图感受 | 推荐 |
|---|---|---|---|---|
| 装 focus-19 v0.3.2 | 重装 `Sigma-File-Manager-2.2.0-focus-19-setup.exe` | 完全无变化 | "什么都没发生" | ❌ 用户已抱怨 |
| 装 focus-18 v0.3.1 | 找历史 setup.exe | filename 框写 FULL PATH | 同 focus-20 v0.3.3,仅版本号不同 | ❌ |
| 装 focus-17 v0.3.0 | 找历史 setup.exe | filename 框写 BASENAME("downloads") | 略好(短文件名),但仍不是用户要的 | ⚠️ |
| **不换版本,继续 focus-21** | 保持 focus-20 v0.3.3 | (同上) | 接受现状,等 focus-21 | ✅ 推荐 |
| 卸载 focus-sync | Sigma FM → Extensions → 卸载 | 无任何 sync 行为 | "彻底放弃" | ⚠️ 路径 D 备选 |

### 6.3 推荐回滚决策

**不主动回滚**。让用户选择:
- 如果用户接受 focus-20 v0.3.3 → focus-21 进入新架构决策树(§8)
- 如果用户不能接受 → 提供历史版本 setup.exe(需要 git tag / archive)

**风险 R21**(Mavis §4):回滚带来再次"重装"成本,用户体验差。

---

## 7. Edge legacy flag 通知策略升级(Mavis §2.4 P1)

### 7.1 当前状态(focus-20 v0.3.3 已 ship)

**已实施**:
- `extension/dist/index.js` line 294-317 加 `maybeShowEdgeLegacyTip()`
- 用 `sigma.storage.get('edgeLegacyFilePickerTipShown')` flag,**只显示 1 次**
- toast notification,duration 25s,标题 "Focus Sync — Edge Save As tip"

**Mavis 指出问题**:用户可能错过 → notification 不再显示。

### 7.2 4 个加强方案(未实施,作为 focus-21 候选)

| # | 方案 | 工作量 | 效果 |
|---|---|---|---|
| 1 | **toolbar 永久按钮** "Focus Sync — Edge WinUI help" | 1 h | 用户随时可点开看 |
| 2 | **settings UI** 重置 `edgeLegacyFilePickerTipShown` flag | 1 h | 用户可重看通知 |
| 3 | **每次 Sigma FM 启动检查** `unsupported_dialog_count > 0` 弹通知 | 1 h | 强制提醒 |
| 4 | **README 加 Edge Modern Save As 章节**(完整步骤) | 1 h | 用户主动查 |

**推荐**:4 个全部实施(focus-21 短期任务),总计 4 小时。

### 7.3 实施细节

**toolbar button**(方案 1):
```js
// extension/dist/index.js activate()
sigma.toolbar.registerDropdown({
    id: 'focus-sync.toolbar',
    title: 'Focus Sync',
    icon: 'mdi-target',
    items: [
        { id: 'syncNow', title: 'Sync Now', commandId: 'focus-sync.syncNow' },
        { id: 'separator', title: '', separator: true },
        { id: 'toggle', title: 'Enable / Disable', commandId: 'focus-sync.toggle' },
        { id: 'separator2', title: '', separator: true },
        { id: 'toggleAutoConfirm', title: 'Auto-Confirm (currently: ' + (autoConfirm ? 'ON' : 'OFF') + ')', commandId: 'focus-sync.toggleAutoConfirm' },
        // focus-21: 永久可见的 Edge WinUI help 按钮
        { id: 'separator3', title: '', separator: true },
        { id: 'edgeWinUIHelp', title: '⚠️ Edge Modern Save As 不支持?', commandId: 'focus-sync.edgeWinUIHelp' }
    ]
}, { syncNow, toggle, toggleAutoConfirm, edgeWinUIHelp });

async function edgeWinUIHelp() {
    sigma.ui.showNotification({
        title: 'Edge Modern Save As workaround',
        description:
            'Edge "Modern Save As" (Win11 22H2+) 不能用 focus-sync 自动同步路径。\n\n' +
            '解决方案:\n' +
            '1. Edge 地址栏输入 edge://flags/#edge-legacy-file-picker\n' +
            '2. "Legacy File Picker" 改为 "Enabled"\n' +
            '3. 重启 Edge\n\n' +
            '之后 Edge 用经典 IFileDialog,focus-sync 完整支持。',
        type: 'info',
        duration: 30000,
    });
}
```

---

## 8. focus-21 决策树(基于 Mavis §3)

**核心原则**:**按 user-verify 结果分支,不是预设方案**。

```
User verifies WinUI 3 filename + Enter behavior
├── 如果"navigation works"(用户手测确认)
│   ├── 实施 focus-21 Option D(写 path + trailing slash + Enter)
│   │   - 加 auto_confirm gate
│   │   - 加失败清空 probe + Unsupported
│   │   - 工作量:4-6 h
│   │   - 成功率:依赖 user-verify 真实性
│   └── 用户仍需启用 Edge legacy flag 才能 100% work
│
└── 如果"navigation 不 work"(预期大概率)
    ├── 选项 A: Edge legacy flag + 通知升级 → focus-21 完整实施
    │   └── 工作量:4 h(7.2 表 4 个方案全部做)
    │
    ├── 选项 B: SendInput Ctrl+L → 调研 prototype
    │   └── 工作量:2-3 d(调研) + 10-15 d(实施,仅当 A 失败时启动)
    │
    └── 选项 E: 接受现状 + 文档化 → DEFAULT fallback
        └── 工作量:0-1 h(README + 一次性通知)
        └── 推荐作为 default
```

### 8.1 focus-21 短期任务(立即做,所有路径都需要)

| # | 任务 | 工作量 | 备注 |
|---|---|---|---|
| 1 | **User-verify Edge WinUI filename + Enter 行为**(3 种格式) | 用户 30 min | **必做** — 决定后续方向 |
| 2 | **User-verify Edge legacy flag 是否 work** | 用户 15 min | **必做** — 决定路径 A 是否可行 |
| 3 | README 加 Edge Modern Save As 章节 | 1 h | 立即做(所有路径都需要) |
| 4 | Extension toolbar 加 "Edge WinUI help" 按钮 | 1 h | 用户永久可见 |
| 5 | Settings UI 重置 `edgeLegacyFilePickerTipShown` flag | 1 h | 用户可重看通知 |
| 6 | 启动检查 `unsupported_dialog_count > 0` 弹通知 | 1 h | 强制提醒 |
| 7 | focus-21 build + ship v0.3.4 | 1 h | 立即做 |

**总计**:4-5 h(用户验证外)+ 用户 45 min 验证

### 8.2 focus-21 中期任务(等 user-verify 后分支)

| # | 任务 | 工作量 | 触发条件 |
|---|---|---|---|
| A | **Edge legacy flag 完整支持** | 2 h | user-verify flag work |
| B | Option D(path + Enter) | 4-6 h | **仅当** user-verify navigation works |
| C | SendInput Ctrl+L prototype | 2-3 d | **仅当** A 失败 |

### 8.3 focus-22+ 长期(避免)

| # | 任务 | 工作量 | 触发条件 |
|---|---|---|---|
| NavigationView 多步导航 | 20-40 h | 仅当 A、B 全失败 |
| 其他 WinUI API 调研 | 不定 | 等 Microsoft 文档化 |

---

## 9. 风险矩阵(rev.2 扩充,采纳 Mavis §4)

| # | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| R1-R20 | (前几轮已识别) | — | — | (已记录) |
| **R21** | focus-20 v0.3.3 已 ship,回滚带来再次重装成本 | 中 | 用户体验差 | 提供清晰的回滚路径文档(§6) |
| **R22** | Edge legacy flag 用户启用后,Edge 升级重置 flag | 中 | 重新 WinUI | README 加"升级后检查 flag"提示 |
| **R23** | user-verify 结论"Enter navigation works",但实际是 Save 触发(误判) | 中 | Option D 误实施 | 同时测试 Enter + Save 两个分支 |
| **R24** | 通知"用户没看到"持续发生(用户 focus 模式) | 高 | Edge flag 不启用 | 升级到 toolbar button + settings UI + 启动检查(§7) |
| **R25** | user-verify 不做,Claude 又一次"假设性推理" | 高 | focus-21 重复 focus-20 错误 | systematic-debugging Phase 0-1 不允许跳过(本文件 §3.1 修订后已声明) |

---

## 10. 给用户决策清单(Mavis §6 整合)

**你已做了 5 轮 review,我做了 5 轮 review,Claude 终于承认架构错。这是这一系列 review 的终点。**

**建议做 3 件事**(决定 focus-21):

### 10.1 User-verify(15-30 分钟,必做)

- [ ] 在 Edge WinUI Save As filename 框手动输入 `C:\DrvPath\` 然后 Enter,看是否导航
- [ ] 测试三种格式:`C:\DrvPath\` / `C:\DrvPath` / `C:\DrvPath\file.exe`
- [ ] 启用 Edge legacy flag(`edge://flags/#edge-legacy-file-picker`),重启 Edge
- [ ] 触发下载 Save As,观察 dialog class 是否变成经典 IFileDialog

### 10.2 回滚决策(看截图偏好)

- [ ] 接受 focus-20 v0.3.3(继续 focus-21)
- [ ] 回滚到 focus-19 v0.3.2(完全无变化)
- [ ] 回滚到 focus-18 v0.3.1(filename 框污染)
- [ ] 回滚到 focus-17 v0.3.0(filename 框写 BASENAME)
- [ ] 卸载 focus-sync,用 Listary 替代

### 10.3 focus-21 路径决策

- [ ] **A + E 组合** = Edge legacy flag + 文档化限制(4-5 h,**推荐 default**)
- [ ] A + B = Edge flag + SendInput prototype(2-3 d 调研)
- [ ] A + C = Edge flag + NavigationView(20-40 h)
- [ ] **E 单独** = 接受现状 + 文档化(0-1 h,**最简单**)
- [ ] D = 暂停 spike(0)

**回复 10.1 + 10.2 + 10.3 的决定,我可以基于你的实证数据给出 focus-21 v0.3.4 实施方案**。

---

## 11. 教训(沿用 retrospective #1 + Mavis §7)

### 11.1 我自己的认知错误

1. **假设性推理代替 user-verify** — 每轮 fix 都是"基于 Chromium Save As 模型推测 WinUI 3 行为",从不 user-verify(retrospective #1 §2.4)
2. **看 user message 但不解读诉求** — 用户说"filename 污染",我以为"filename 错",实际是"导航根本没工作"
3. **每次 fix 在同一条思路下微调** — 从 v0.3.0 到 v0.3.3 都是"写 filename 框",只是写法和细节变
4. **没有质疑上游 PathWrap 的假设** — 它假设的世界和 WinUI 3 真实世界不匹配
5. **retrospective #1 §3.1 又犯"假设预期"错**(Mavis §2.2 指出) — 自我批评后又犯

### 11.2 给后续会话的硬规则(retrospective #1 §9.3 + rev.2 强化)

**规则 1**:新 dialog 类型的支持,**必须先 user-verify dialog 的真实控件结构**(inspect.exe / UIA Verify),再决定 fix 方向。

**规则 2**:fix 之后,**必须 user-verify "path 在正确字段显示"**(用户截图 + 标记箭头)。

**规则 3**:3+ fix 失败时,**质疑上游/架构假设**,不质疑实现细节。

**规则 4**:**不发"已知错的方案"**(focus-20 v0.3.3 应该 user-verify 后再决定 ship / 不 ship,不应直接 ship)。

**规则 5**(rev.2 新增):**retrospective / plan 中禁止写"预期结果"**(写"测试方法 + 待 user-verify + 结论待填")。**写"预期"就是"假设性推理"的变种,违反 evidence-first**。

---

## 12. 文档链接(完整链路)

**本文档(retrospective #2 rev.2,final)**:
`F:\soft\00selfmade\filemanager\docs\superpowers\plans\2026-10-02-focus-20-retrospective-architecture-failure-rev2.md`

**supersedes**:
`docs/superpowers/plans/2026-10-02-focus-20-retrospective-architecture-failure.md`(retrospective #1,5 处偏差已修)

**输入文档**:
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-review.md`(Mavis 第 5 轮 review)
- `docs/superpowers/plans/2026-10-02-focus-19-retrospective-deep-dive.md`(retrospective #0)
- `docs/superpowers/plans/2026-10-02-focus-19-retrospective-review.md`(Mavis review of retrospective #0)

**对照 plans**:
- `docs/superpowers/plans/2026-10-02-focus-20-edge-winui-fix.md`(focus-20 实施 plan,基于错的假设)
- `docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-revised.md`(focus-19 rev.2)

**5 轮 handoffs**:
- focus-17/18/19/20 handoffs in `handoff-2026-10-*.md`

**截图证据**:
- 用户最新截图:`E:\Users\Duanyi\.claude\uploads\7b65d5b1-...-image.png`(已纳入 §1.1)
