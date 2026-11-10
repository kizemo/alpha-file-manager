# handoff-2026-10-01-focus-18-path-activation-auto-confirm

**接力**: Claude (MiniMax-M3) → 下一个 session
**日期**: 2026-10-01
**会话边界**: 当前 session 活跃 ~95 分钟,Bash 调用 ~55,达到 CLAUDE.md "≥90 min 主动写交接" 触发点
**任务**: 执行 focus-18 plan (PathWrap 路径激活恢复 + Auto-Confirm toggle)

---

## 0. TL;DR(给接手会话的 30 秒摘要)

1. **focus-17 v0.3.0 已 build 并打包到 `release/Sigma-File-Manager-2.2.0-focus-17-setup.exe`**
2. **focus-17 修复 2/3 bugs**: console window ✓ / version v0.3.0 ✓ / **path activation 失败**(用户测试 Edge Save As, PUSH 200 但 dialog 不跳)
3. **根因已诊断**: spike.log 证据 + 代码对比。focus-16 删除 3 个关键部分,focus-17 加的 basename 写错方向
5. **Mavis 已写好 plan**: `docs/superpowers/plans/2026-10-01-focus-18-path-b-auto-confirm-toggle-plan.md` — 这是**最终执行 plan**
6. **新会话第一件事**: 读 §3 必读顺序,然后用 executing-plans 技能执行

---

## 1. 已完成的工作(本会话)

### 1.1 Bug 分析 + 修复(focus-17)
- **Bug #1** (console window): `#![windows_subsystem = "windows"]` + tracing 切文件 ✓ 修
- **Bug #2** (version v0.2.0): register.ps1 加 in-place version patch ✓ 修
- **Bug #3** (path activation): 修了 basename 方向错误 + COM SetFolder 早返回 — **未完整修**,用户实测失败

### 1.2 深度诊断(focus-18 分析)
- 读 spike.log 抓到 4 个 PUSH 200 全失败
- 抓 UIA inventory: Edge Save As 是 WinUI ItemsView,无 41477,Save 按钮 `automation_id='1'`
- git history 对比 commit 905db31 vs 当前: 发现 focus-16 删除 `fg_bypass` + `InvokePattern::Invoke` + `SendMessageW Enter`
- 写了 `docs/superpowers/plans/2026-10-01-focus-18-path-activation-architecture-regression.md` (我原始诊断,部分错误)

### 1.3 Mavis 给的最终 plan
- **位置**: `docs/superpowers/plans/2026-10-01-focus-18-path-b-auto-confirm-toggle-plan.md`
- **9 项任务**,~250 行代码改动,10 个文件(1 新建 + 9 修改)
- **核心改进**(超过我的诊断):
  1. 写 **FULL PATH** 到 filename 框(不是 basename — Edge WinUI 接受完整路径导航)
  2. dedup 改为 **per-dialog single-shot**(用 `state.try_mark_committed`),不是 per-path
  3. **Auto-Confirm toggle 默认 OFF**(用户主动 opt-in 才 Invoke Save)
  4. **fg_bypass 包裹恢复** + Invoke + Enter commit 步骤恢复
  5. extension toolbar 加菜单项,state 用 `sigma.storage`

### 1.4 已打包产物
- `release/Sigma-File-Manager-2.2.0-focus-17-setup.exe` (17,065,209 bytes, MD5 `a2ee9c787277545b7ecedacf7af41dd6`)
- spike.exe (focus-17) at `sigma-listary-spike/target/release/spike.exe`
- 用户已安装 + 验证:Bug #1 + #2 已修,Bug #3 未修

---

## 2. 必须做的(下一会话)

### 2.1 执行 Mavis 的 plan
- **计划文件**: `docs/superpowers/plans/2026-10-01-focus-18-path-b-auto-confirm-toggle-plan.md`
- **执行方式**: 用 `superpowers:executing-plans` 技能(Task-by-task in same session)
- **预计时间**: 60-90 分钟(增量 vs focus-17 是恢复代码 + 测试 + 用户 7-app 矩阵手动测试)

### 2.2 完成判据(Mavis §7)
11 项必须 PASS:
- `cargo test` 全部 PASS(原 23 + 新增 ~13 ≈ 36)
- `cargo build --release` 成功
- Tauri build 产出 `release/Sigma-File-Manager-2.2.0-focus-18-setup.exe`
- 安装 focus-18 无 console 弹窗
- `/health` 返回 mode=service / session_id=1
- Sigma FM 重启后 toolbar 有 "Auto-Confirm" 菜单项
- **Auto-Confirm OFF**:Edge Save As filename 框显示 `C:\downloads\`(不 Invoke)
- **Auto-Confirm ON**:Edge Save As filename 框 + 自动 Invoke Save
- **per-dialog dedup**:开 ON 后 Sigma FM 浏览 A→B→C,只第一次 push 触发 Save
- 原生 IFileDialog(影刀/Word/Notepad/VS Code):SetFolder 跳地址栏
- Chromium dialog: 41477 + Invoke 触发导航

### 2.3 用户手动测试(必做)
Auto-Confirm 行为验证、7-app 矩阵必须用户做,Agent 只能 build + 自测。完成 build 后请用户跑测试。

---

## 4. 必读顺序(从最重要到最次要)

**新会话第一件事**: 按顺序读这 5 个文件(每读完一个,API 在哪心里有数):

1. **`docs/superpowers/plans/2026-10-01-focus-18-path-b-auto-confirm-toggle-plan.md`** (977 行)
   - Mavis 的最终执行 plan。**这是唯一权威执行文件**
   - 9 个 Task step-by-step;代码预计 ~250 行
   - 验收清单 §7 必须全过

2. **`docs/superpowers/plans/2026-10-01-focus-18-path-activation-architecture-regression.md`** (我写的根因诊断)
   - 看 spike.log 实测证据 §11 + 完整 UIA 结构
   - 解释**为什么**focus-16 改动是错的
   - **我原计划的 basename 部分是错的**,Mavis 的 FULL PATH 才是对的

3. **`docs/superpowers/plans/2026-10-01-focus-17-bug-fix-implementation-plan.md`** (focus-17 plan)
   - 看"已完成"的 focus-17 改动了什么
   - **重要**: focus-18 plan 的代码修改要保留 focus-17 的修复(console、version、basename 函数);不要撤销

4. **`handoff-2026-10-01-focus-16-install-test-results.md`** (focus-16 用户报告)
   - 用户报的 3 个问题原话 + 截图描述
   - focus-18 任务就是修 #3

5. **`docs/superpowers/plans/2026-10-01-focus-17-modification-recommendations.md`** (Mavis 之前的 focus-17 修复方案)
   - 看 §4 Bug #3 双 bug 诊断 + 修复(diff blocks)
   - 验证我和他理解一致

---

## 5. 当前环境 / 分支 / 基线

| 项 | 值 |
|---|---|
| **分支** | `feat/recovery-2026-09-28` (用户当前开发分支) |
| **基线** | `main` (无 HEAD commit, working tree 状态) |
| **working tree 状态** | M (focus-17 改动 + focus-16 改动):Cargo.lock/Cargo.toml/logger.rs/main.rs/uia_inject.rs 等; ?? release/ (新目录); src/http_server.rs (新文件) |
| **rustc 版本** | `1.90.0-x86_64-pc-windows-msvc` (用户硬装,broken;stable/1.97 broken) |
| **PATH 必须** | `export PATH="/c/Users/Duanyi/.rustup/toolchains/1.90.0-x86_64-pc-windows-msvc/bin:$PATH"` |
| **Cargo.lock 强制降级** | `tracing-appender=0.2.3`、`time=0.3.41`、`thiserror=1.0.69`、`time-core=0.1.4`、`time-macros=0.2.22` |
| **cargo test 已知 fail** | `config::tests::resolve_port_strict_on_conflict` — 1 个,pre-existing,非本次修复 |

---

## 6. 避坑提示

### 6.1 工具链
- **rustc 1.97 / stable 坏**:`only metadata stub found for rustc_std_workspace_core` 错误。必须用 rustc 1.90
- **cargo PATH**:bash 默认找不到 cargo。每次命令前 `export PATH=...`(或 alias)
- **非 Test harness parallel flakiness**:`cargo test --lib` 默认并行; env-var mock 测试可能 race。保留 focus-17 的"用 contains 而非 starts_with"断言模式

### 6.2 Bash 嵌套 PowerShell
- **PowerShell heredoc 在 bash 里脆弱**(Mavis review §2.7)。如果做 PS 验证,**写 .ps1 文件**用 Write tool,然后 `powershell -File xxx.ps1`

### 6.3 git 操作
- **NO commit during execution** — 用户说"测试通过再 commit"。focus-18 Task 8 才 batch commit
- **git add 不要用 `-A` 或 `.`** — 给具体文件路径
- **register.ps1 备份**:Task 7 step 2 之前先 `cp register.ps1 C:\Temp\register.ps1.bak-focus18`

### 6.4 fg_bypass 恢复
- Task 1 step 1: `git show 905db31:sigma-listary-spike/src/fg_bypass.rs > sigma-listary-spike/src/fg_bypass.rs`
- **保留原 Apache-2.0 license header** + 在末尾加 focus-18 commit ref (R9 风险)

### 6.5 测试断言
- **basename("C:\\") 返回 "C:"**(不是 "")— 详见 Mavis review §1.1。我原测试期望错。新测试用 "C:"
- **fallback env-var test relaxed**:contains("focus-sync") 而非 starts_with(temp_dir)— 详见我的 Ruling

---

## 7. 关键证据(给新会话的"why"上下文)

### 7.1 用户场景
- 用户在 Sigma FM 浏览器浏览文件(切 A → B → C)
- 切换路径时,任何应用的"另存为"对话框应自动:
  - 默认:filename 框显示完整路径(用户手动 Save)
  - Opt-in:自动 Invoke Save 一次
- 用户当前的测试场景:**Edge 另存为**(`automation_id='1'` Save button, 无 41477 address bar)

### 7.2 spike.log 关键证据(2026-10-01T14:30:43)
```
T14:30:43.176683Z  WARN  AccessibleObjectFromWindow failed; dialog is not a native IFileDialog
T14:30:43.475205Z  INFO  === Dialog UIA inventory (hwnd=3671744, total descendants=106) ===
T14:30:43.555513Z  INFO  No 41477 address bar; wrote BASENAME='downloads' to filename edit
T14:30:43.556081Z  INFO  UIA fallback succeeded.
                       ★★★ 没有 Invoke / 没有 commit ★★★
```

### 7.3 Edge Save As UIA 结构
- `automation_id='1001' name='文件名'` = filename Edit
- `automation_id='1' name='保存(S)'` = Save 按钮 (IDOK, Invoke 完全可行)
- `automation_id='2'` = 取消按钮 (IDCANCEL)
- 无 41477,无 breadcrumb 控件,无 IFileDialog COM

### 7.4 focus-16 + focus-17 删了 3 个关键(必须恢复)
1. **fg_bypass::run_with_bypass 包裹** — Chromium 焦点抢回防护
2. **InvokePattern::Invoke commit** — Save 按钮触发 dialog navigation
3. **SendMessageW Enter fallback** — 无 Save button 时兜底

---

## 8. 已建立的工作树状态(必须 verify 后才能 commit)

修改文件(per Mavis Task 8 git add list):
- sigma-listary-spike/src/fg_bypass.rs(新建,从 905db31 恢复)
- sigma-listary-spike/src/writer.rs(加 state + auto_confirm + fg_bypass 包裹)
- sigma-listary-spike/src/uia_inject.rs(加 should_commit + Invoke + Enter + FULL PATH)
- sigma-listary-spike/src/state.rs(+ committed_hwnds 字段 + try_mark_committed / clear_committed)
- sigma-listary-spike/src/http_server.rs(body 加 auto_confirm)
- sigma-listary-spike/src/lib.rs(+ pub mod fg_bypass)
- sigma-listary-spike/src/main.rs(DialogEvent::Closed → clear_committed)
- sigma-listary-spike/Cargo.lock(可能因依赖变更自动)
- release/extension/dist/index.js(+ autoConfirm state + toolbar item + body field)
- release/extension-installer/register.ps1(+ focus-sync.toggleAutoConfirm command)

---

## 9. 给下一个 session 的"不要做"清单

- ❌ **不要修改 focus-17 已修的 3 个 bug** (#1 console / #2 version / #3 basename 函数)— focus-18 在它们基础上叠加,不是撤销
- ❌ **不要替换 cargo** — rustup 默认是 stable(损坏)。rustc 1.90 在 `/c/Users/Duanyi/.rustup/toolchains/1.90.0-x86_64-pc-windows-msvc/bin/`
- ❌ **不要 --force-push** — 用户没说
- ❌ **不要碰 user-extensions.json / Sigma FM 配置** — focus-18 plan 不需要
- ❌ **不要 untrack release/*.exe** — 用户自己管理
- ❌ **不要删除任何 user 文件** — CLAUDE.md 红线
- ❌ **不要改 Cargo.toml dependencies** — 除非 fg_bypass 恢复后编译失败需要新增 dep(Q3 验证)
- ❌ **不要把 basename('千分之一') 改回 dirname()** — Mavis 已弃用,但 basename 函数保留

---

## 10. 完成的最终交付

执行完成后,向用户报告:

1. **所有 Mavis §7 的 11 项完成判据全过**(cargo test + PE subsystem + Sigma FM toolbar + Auto-Confirm 双状态测试 + per-dialog dedup + IFileDialog + Chromium 测试)
2. **`release/Sigma-File-Manager-2.2.0-focus-18-setup.exe` 已生成**,MD5 与 spike.exe 验证一致
3. **git log 干净,1 个 commit + tag v0.3.1**
4. **memory 提醒**: 用户 memory 中 `sigma_fm_extension_registration.md` 还没更新 v0.3.1 的 Auto-Confirm 行为 — 下个 session 应该补

---

## 11. prompt 一句话(放在 prompt-focus-18-auto-confirm-next.md)

```
读 handoff-2026-10-01-focus-18-path-activation-auto-confirm.md 全文,然后读 docs/superpowers/plans/2026-10-01-focus-18-path-b-auto-confirm-toggle-plan.md 按 9 个 Task 顺序执行,最后 build focus-18-setup.exe 到 release/,不要 commit git(等用户授权)
```