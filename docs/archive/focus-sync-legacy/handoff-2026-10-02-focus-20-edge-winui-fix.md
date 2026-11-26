# handoff-2026-10-02-focus-20-edge-winui-fix

**接力**: Claude (MiniMax-M3) → 用户 / 下一个 session
**日期**: 2026-10-02
**会话边界**: 当前 session 已 ship focus-19 v0.3.2 + 写 retrospective + 修订 focus-20 plan + 实施 focus-20 v0.3.3
**任务**: 实施 focus-20 v0.3.3 (Edge WinUI Save As 修复,基于 retrospective review 反馈)

---

## 0. TL;DR (30 秒摘要)

1. **focus-20 v0.3.3 已 ship**: 恢复 focus-18 的 filename-write fallback(无 Enter) + 加 Edge legacy flag 一次性通知
2. **解决 focus-19 v0.3.2 的 regression**: 用户从"完全无变化"回到"看到路径"(虽然仍需手动 Enter / Save)
3. **Edge legacy flag 升 primary**: 一次性 notification 教育用户启用 `edge://flags/#edge-legacy-file-picker`,这是真正 work 的方案(经典 IFileDialog)
4. **Option D (Enter commit) 推迟**: 等 user-verify "Edge WinUI Save As 输入 path + Enter 是否真的导航"
5. **git 未 commit**: 等用户授权

---

## 1. 本会话已完成(focus-20 v0.3.3)

### 1.1 Plan + Review 文档

| 文件 | 行数 | 状态 |
|---|---|---|
| `docs/superpowers/plans/2026-10-02-focus-19-retrospective-deep-dive.md` | ~360 | ✅ 已 ship |
| `docs/superpowers/plans/2026-10-02-focus-19-retrospective-review.md` | (Mavis 写) | ✅ 已 ship |
| `docs/superpowers/plans/2026-10-02-focus-20-edge-winui-fix.md` | ~330 | ✅ 新建(实施 plan) |

### 1.2 代码改动(focus-20 v0.3.3)

**改动文件**(4 个,~80 行):
- `sigma-listary-spike/src/uia_inject.rs`:
  - 恢复 focus-18 的 filename-write fallback(写 FULL PATH 到 best_edit,**无** Enter commit)
  - `inject_via_uia` 加 `state: &AppState` 参数,在"无 edit 元素"分支 inc_unsupported_dialog
  - 修复 Mavis §3.2 计数 bug:probe 成功不调 inc_unsupported_dialog
- `sigma-listary-spike/src/http_server.rs`:
  - `SPIKE_VERSION` 0.3.2 → 0.3.3
  - test assertion 同步更新
- `release/extension/dist/index.js`:
  - 加 `maybeShowEdgeLegacyTip()` 一次性 Edge tip 通知
  - 加 v0.3.3 changes header 段
- `release/extension-installer/register.ps1`:
  - 版本号 0.3.2 → 0.3.3(所有 5 处)

### 1.3 Build 产物

- ✅ spike.exe at `sigma-listary-spike/target/release/spike.exe`
- ✅ `release/extension/bin/focus-sync-sidecar.exe`(Tauri bundle.resources)
- ✅ `release/extension-installer/focus-sync-sidecar.exe`(makensis File)
- ⏳ `release/Sigma-File-Manager-2.2.0-focus-20-setup.exe`(NSIS,等 build 完成)

### 1.4 cargo test 状态

47/48 PASS(1 个 pre-existing `config::resolve_port_strict_on_conflict` fail,handoff §5 已记录)。

---

## 2. focus-20 v0.3.3 行为对比

| 场景 | focus-19 v0.3.2 | focus-20 v0.3.3 |
|---|---|---|
| **Edge WinUI Save As**(`#32770` wrapped,无 41477) | 完全无变化(回归) | **filename 框显示 `C:\DrvPath`** + 通知"dialog not navigated" |
| **Edge legacy IFileDialog**(用户启用 flag) | COM SetFolder ✓ | COM SetFolder ✓(无变化) |
| **Chrome Save As**(Chromium 41477) | 41477 + Invoke ✓ | 41477 + Invoke ✓(无变化) |
| **影刀 / Notepad / Word / VS Code Save As**(IFileDialog) | COM SetFolder ✓ | COM SetFolder ✓(无变化) |
| **Edge tip notification** | ❌ 无 | ✅ **激活时一次性弹出**,教用户启用 `edge://flags/#edge-legacy-file-picker` |
| `unsupported_dialog_count` 计数 | 无 edit 元素时 inc | **只在无 edit 元素时 inc**(修复了 Mavis §3.2 计数 bug) |

---

## 3. 用户必做(测试 + 决策)

### 3.1 手动测试 6-app 矩阵(已规划 §6.1-6.3 in focus-20 plan)

按 `docs/superpowers/plans/2026-10-02-focus-20-edge-winui-fix.md` §6 清单:
1. **Edge Modern Save As** → 期望:filename 框显示 `C:\DrvPath`(focus-20 v0.3.3 行为)
2. **Chrome Save As** → 期望:41477 + Invoke 行为不变
3. **影刀 / Notepad / Word / VS Code** → 期望:COM SetFolder 行为不变
4. **Edge tip notification** → 期望:首次激活看到一次性通知

### 3.2 Empirical 验证(决策 focus-21 范围)

按 `docs/superpowers/plans/2026-10-02-focus-20-edge-winui-fix.md` §6.4 + retrospective §2.1:

**测试 A**:Edge WinUI Save As 真实行为
- 在 filename 框**手动输入** `C:\DrvPath\` → 按 Enter
- **观察**:dialog 是否导航到 `C:\DrvPath\`?
  - ✅ 导航成功 → 报告 → focus-21 实施 Option D(path + Enter commit)
  - ❌ 文件名变成字面 `C:\DrvPath\` → 报告 → focus-22+ NavigationView

**测试 B**:Edge legacy flag 实际效果
- `edge://flags/#edge-legacy-file-picker` → Enabled → 重启 Edge
- 触发下载 → Save As
- 用 `inspect.exe` (Windows SDK) 或 UIA Verify 看 dialog class
- **期望**:`#32770` 且 COM 可用(spike 应该走 COM SetFolder 路径,unsupported_dialog_count 不增)

### 3.3 focus-21+ 路径决策

| user-verify 结果 | focus-21 行动 |
|---|---|
| 测试 A ✅ + 测试 B ✅ | focus-21 实施 Option D(path + Enter commit) |
| 测试 A ❌ + 测试 B ✅ | focus-21 只 ship Edge flag 文档化 + README 完整化 |
| 测试 A ✅ + 测试 B ❌ | focus-21 实施 Option D + fallback 到 legacy flag 文档 |
| 测试 A ❌ + 测试 B ❌ | focus-22+ 启动 NavigationView 多步导航(20-40h) |

---

## 4. 当前环境 / 分支 / 基线

| 项 | 值 |
|---|---|
| **分支** | `feat/recovery-2026-09-28`(用户当前开发分支) |
| **基线** | `main` |
| **working tree 状态** | focus-18 + focus-19 + focus-20 改动 M(NO commit per user policy) |
| **rustc** | `1.90.0-x86_64-pc-windows-msvc` |
| **PATH** | `export PATH="/c/Users/Duanyi/.rustup/toolchains/1.90.0-x86_64-pc-windows-msvc/bin:$PATH"` |
| **cargo test fail(已知)** | `config::resolve_port_strict_on_conflict`(pre-existing) |
| **SPIKE_VERSION** | 0.3.3 |

---

## 5. 避坑提示

### 5.1 不要做的

- ❌ **不要 commit git**:focus-20 v0.3.3 改动等用户授权 commit
- ❌ **不要立即实施 Option D**:需 user-verify 后再决定
- ❌ **不要低估 NavigationView 工作量**:Mavis §3.4 评估 20-40h,不是 8-12h
- ❌ **不要相信 v0.3.2 的 deep-analysis**:它误判了 focus-18 的 fallback,导致 regression

### 5.2 要做的

- ✅ 先做 §3.2 empirical 验证,再决定 focus-21 范围
- ✅ 装 focus-20 v0.3.3 后跑 6-app 矩阵
- ✅ Edge tip notification 是 primary 解决方案,README + 文档化是 first-class
- ✅ 看 spike.log.YYYY-MM-DD(tracing 日志),不只是 spike.log(JSON 事件)

---

## 6. 文档链接(给后续会话)

**根因**:`docs/superpowers/plans/2026-10-02-focus-19-retrospective-deep-dive.md`
**review**:`docs/superpowers/plans/2026-10-02-focus-19-retrospective-review.md`
**实施 plan**:`docs/superpowers/plans/2026-10-02-focus-20-edge-winui-fix.md`
**focus-19 handoff**:`handoff-2026-10-02-focus-19-edge-winui-save-as-failure.md`
**focus-18 handoff**:`handoff-2026-10-01-focus-18-path-activation-auto-confirm.md`

---

## 7. 给下一个 session 的"now what"

1. **等 build 完成**:复制 NSIS output → `release/Sigma-File-Manager-2.2.0-focus-20-setup.exe`
2. **报告用户**:本会话所有交付 + 等用户测试 + 决策
3. **用户必做**:
   - 装 focus-20-setup.exe
   - 跑 6-app 矩阵(§3.1)
   - empirical 验证(§3.2)
   - 反馈结果,决定 focus-21 范围
4. **memory 更新**:
   - `memory/focus_sync_v031_focus18.md` → 加 v0.3.3 行为说明(已修 regression)
   - 新建 `memory/focus_sync_v033_focus20.md` 记录 Edge flag tip + 恢复 fallback
5. **git 等授权 commit**

---

## 8. prompt 一句话(给下一个 session)

```
读 handoff-2026-10-02-focus-20-edge-winui-fix.md 全文,然后:
1. 等用户完成 focus-20 v0.3.3 的 6-app 矩阵测试 + empirical 验证(§3)
2. 基于 user-verify 结果决定 focus-21 范围(§3.3 三选项)
3. 不要 commit,不要立即实施 Option D,不要低估 NavigationView 工作量
```
