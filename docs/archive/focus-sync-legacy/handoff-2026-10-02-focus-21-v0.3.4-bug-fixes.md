# handoff-2026-10-02-focus-21-v0.3.4-bug-fixes

**接力**: Claude (MiniMax-M3) → 新会话
**日期**: 2026-10-02
**会话边界**: 当前会话已完成 5 轮 focus 改动 + 3 份 retrospective 文档,即将进入 focus-21 v0.3.4 实施阶段。**会话活跃 ~3h+ 且上下文已重**,建议新开会话接力。
**任务**: 实施 focus-21 v0.3.4(修 4 源码 Bug + README Edge 章节 + build)

---

## 0. TL;DR (60 秒摘要)

**5 轮 focus 改动全部失败根因**:PathWrap 架构假设(IFileDialog + Chromium + Other)不适合 WinUI 3 Save As dialog。所有 5 轮都在写 Filename 控件,用户要的是更新 BreadcrumbBar / NavigationView。

**focus-21 v0.3.4 必修 4 项**:
1. 🔴 **Bug A [HIGH]** HTTP server 线程未 CoInitializeEx(focus-17 引入的预存 bug)
2. 🟡 **Bug B [MEDIUM]** Edge tip flag 永久性,无重置机制(用户错过永远看不到)
3. 🟡 **Bug C [MEDIUM]** 初次激活双 notification UX 混乱(Edge tip + Sidecar not reachable 同时弹)
4. 🟡 README 加 Edge Modern Save As 章节

**预期工作量**: 4-5 h + 30 min user-verify(用户手测 WinUI 3 filename + Enter 行为)

---

## 1. 当前环境 / 分支 / 基线

| 项 | 值 |
|---|---|
| **分支** | `feat/recovery-2026-09-28`(用户当前开发分支) |
| **基线** | `main` |
| **working tree 状态** | focus-18 + focus-19 + focus-20 改动 M(NO commit per user policy) |
| **rustc** | `1.90.0-x86_64-pc-windows-msvc` |
| **PATH 必须** | `export PATH="/c/Users/Duanyi/.rustup/toolchains/1.90.0-x86_64-pc-windows-msvc/bin:$PATH"` |
| **cargo test fail(已知)** | `config::resolve_port_strict_on_conflict`(pre-existing,handoff §5 已记录) |
| **SPIKE_VERSION 当前** | 0.3.3 → 目标 **0.3.4** |
| **Extension 版本号 当前** | v0.3.3 → 目标 **v0.3.4** |

---

## 2. 必读顺序(从最重要到最次要)

按 superpowers 规则,新会话第一件事按顺序读这 5 个文件(每读完一个,API 在哪心里有数):

### 2.1 🔴 必读第一(根因 + 决策框架)

**`docs/superpowers/plans/2026-10-02-focus-20-retrospective-architecture-failure-rev3.md`**(新)
- 整个 5 轮修复失败的真相
- Phase 4.5 架构质疑(PathWrap vs WinUI 3)
- 4 源码 Bug 详细诊断 + 修复代码草稿
- focus-21 v0.3.4 完整 plan(§12)
- 风险矩阵 R1-R31
- 8 条硬规则

### 2.2 🟡 必读第二(focus-20 handoff 状态)

**`handoff-2026-10-02-focus-20-edge-winui-fix.md`**
- focus-20 v0.3.3 build 状态
- 6-app 矩阵测试清单

### 2.3 🟡 必读第三(focus-19 原始问题)

**`handoff-2026-10-02-focus-19-edge-winui-save-as-failure.md`**
- 用户首次反馈(filename pollution)
- focus-19 v0.3.2 regression 证据
- spike.log 关键 evidence

### 2.4 🟢 选读(focus-18 背景)

**`handoff-2026-10-01-focus-18-path-activation-auto-confirm.md`**
- focus-18 v0.3.1 实现细节
- 7-app 测试矩阵
- spike.log 中 focus-18 成功行为(对比 focus-19/20)

### 2.5 🟢 选读(retrospective #1 + #2)

- `docs/superpowers/plans/2026-10-02-focus-19-retrospective-deep-dive.md`(retrospective #0)
- `docs/superpowers/plans/2026-10-02-focus-19-retrospective-review.md`(Mavis 第 5 轮 review)

---

## 3. focus-21 v0.3.4 详细任务清单

按 `retrospective rev.3` §12 完整 plan。**每项都附修复代码草稿**,直接可实施。

### 3.1 必修(2-3 h)

#### Task A: 🔴 修 Bug A — HTTP server 线程 CoInitializeEx

**文件**: `sigma-listary-spike/src/main.rs` 第 240-247 行

**修复代码**(rev.3 §9.1):
```rust
let _http_handle = std::thread::Builder::new()
    .name("spike-http".into())
    .spawn(move || {
        // COM init as STA — required for UIA. Each thread that uses COM
        // must call CoInitializeEx independently. focus-21: per Mavis
        // review §2.1 — pre-existing bug from focus-17.
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            );
        }
        if let Err(e) = http_server::run_server(state_for_http, port, mode) {
            tracing::warn!(error = %e, "http server exited");
        }
    })
    .expect("spawn http server thread");
```

**验证判据**: cargo build + 在多种 Windows 版本测试(用户多版本测试)

#### Task B: 🟡 修 Bug B — Edge tip toolbar 按钮

**文件**: `release/extension/dist/index.js` line 298-318

**修复代码**(rev.3 §9.2):

1. 添加 `resetEdgeTip` 函数
2. 在 `registerDropdown` items 加 `{ id: 'resetEdgeTip', title: '🔁 Re-show Edge tip', commandId: 'focus-sync.resetEdgeTip' }`
3. 在 command 注册处加 `sigma.commands.registerCommand({ id: 'focus-sync.resetEdgeTip', ... }, resetEdgeTip)`
4. 在 `registerDropdown` 第 2 参数 callbacks 加 `resetEdgeTip`

```js
async function resetEdgeTip() {
    try {
        await sigma.storage.set('edgeLegacyFilePickerTipShown', false);
        sigma.ui.showNotification({
            title: 'Focus Sync',
            description: 'Edge tip will show on next activation.',
            type: 'info',
            duration: 3000,
        });
    } catch (e) {}
}
```

#### Task C: 🟡 修 Bug C — 延迟 initial ping 5s

**文件**: `release/extension/dist/index.js` line 326-339 (`activate()`)

**修复代码**(rev.3 §9.3):
```js
// Edge tip 显示后,initial ping 延迟 5s
sigma.ui.showNotification({...Edge tip...});

setTimeout(() => {
    void pingSidecar().then((ok) => {
        if (!ok && !notifiedSinceLastFailure) {
            sigma.ui.showNotification({
                title: 'Focus Sync',
                description: 'Sidecar not reachable...',
                type: 'error',
                duration: 15000,
            });
        }
    });
}, 5000);

startHealthCheck();
```

#### Task D: Edge tip duration 25s → 8s(顺手改)

#### Task E: inventory log 改 debug-level(顺手改)

**文件**: `sigma-listary-spike/src/uia_inject.rs` line 359 + 392-395

**修复**: `info!` → `debug!`

### 3.2 建议(1-2 h)

#### Task F: 累加重构

**文件**: `sigma-listary-spike/src/uia_inject.rs`

**修复**(rev.3 §10.1):
- `inject_via_uia` 改纯函数(移除 state 参数 + 移除内部 inc_unsupported_dialog)
- `inject_folder_path` 集中累加(只在 Ok(Unsupported) 和 Err 分支)

#### Task G: README 加 Edge Modern Save As 章节

**新建文件**: `sigma-listary-spike/README.md`(或 sigma-file-manager 的 docs/)

**内容**: Edge legacy flag 启用步骤 + 当前限制 + 替代方案

### 3.3 集成 + User-verify(1.5 h)

#### Task H: cargo test 全 PASS(含新回归测试)

#### Task I: cargo build + Tauri NSIS build

**步骤**:
1. `cargo test --release`(应 47/48 PASS + 新加的 regression test)
2. `cargo build --release`(spike.exe)
3. `powershell -File sigma-file-manager/scripts/build-with-sidecar.ps1`
4. `cd sigma-file-manager && npm run tauri:build`(NSIS setup.exe)
5. 复制到 `release/Sigma-File-Manager-2.2.0-focus-21-setup.exe`

#### Task J: 用户手测 WinUI 3 filename + Enter 行为

**3 种格式**:
- `C:\DrvPath\`(带 trailing slash)
- `C:\DrvPath`(无 trailing slash)
- `C:\DrvPath\file.exe`(路径 + 默认文件名)

**结果记录**:作为 focus-22 决策依据

#### Task K: 用户启用 Edge legacy flag 验证

**步骤**:
1. `edge://flags/#edge-legacy-file-picker` → Enabled → 重启 Edge
2. 触发下载 Save As
3. 观察 dialog class 是否变成经典 IFileDialog
4. 验证 spike 的 COM SetFolder 路径 work(unsupported_dialog_count 不增)

---

## 4. 改动文件清单(预计)

| 文件 | 改动 |
|---|---|
| `sigma-listary-spike/src/main.rs` | Task A: HTTP thread CoInitializeEx |
| `release/extension/dist/index.js` | Task B + C + D: toolbar button + 延迟 ping + duration |
| `sigma-listary-spike/src/uia_inject.rs` | Task E + F: log debug + 累加重构 |
| `sigma-listary-spike/src/http_server.rs` | SPIKE_VERSION 0.3.3 → 0.3.4 + test assertion 同步 |
| `release/extension-installer/register.ps1` | 版本号 0.3.3 → 0.3.4 |
| `sigma-listary-spike/README.md`(新建) | Edge Modern Save As 章节 |

预计改动 ~150 行 + 新建 README ~50 行 = 总 ~200 行。

---

## 5. 避坑提示

### 5.1 不要做的(CLAUDE.md 红线 + retrospective 规则)

- ❌ **不要 commit git** — 等用户授权 commit(沿用 focus-18/19/20 政策)
- ❌ **不要回退 focus-20 v0.3.3** — 用户已装,继续 focus-21 升级
- ❌ **不要 "再试一次 v0.3.x"** — PathWrap 架构已确认不适合 WinUI 3,必须换路径
- ❌ **不要写 "预期结果"**(违反规则 5) — 测试方法 + 待 user-verify
- ❌ **不要单条命令触碰多个用户文件**(CLAUDE.md 红线)
- ❌ **不要修改 focus-18/19/20 已修的 6 个 Rust 文件**(除非新功能需要)
- ❌ **不要 `--force-push`** — 用户没说
- ❌ **不要碰 user-extensions.json** — focus-21 v0.3.4 不需要

### 5.2 要做的(per retrospective rev.3 规则)

- ✅ **先读 retrospective rev.3**(`/docs/superpowers/plans/2026-10-02-focus-20-retrospective-architecture-failure-rev3.md`)
- ✅ **每个 Bug 修复后跑 cargo test**(确保 regression)
- ✅ **cargo build 后跑 build-with-sidecar.ps1 + npm run tauri:build**(沿用 focus-20 流程)
- ✅ **测试 focus-21 v0.3.4 时跑 6-app 矩阵**(handoff §6.1-6.3)
- ✅ **保留 focus-17 已修的 3 个 bug**(console / version / basename)— focus-21 在它们基础上叠加
- ✅ **保留 focus-18 已修**(fg_bypass / Invoke / Enter / Auto-Confirm)— focus-21 不撤销
- ✅ **保留 focus-19 已修**(unsupported_dialog_count / health 字段 / extension polling)— focus-21 在它们基础上叠加

---

## 6. focus-21 v0.3.4 完成后,用户必做

### 6.1 6-app 矩阵测试(focus-20 v0.3.3 已有清单,沿用)

按 `handoff-2026-10-02-focus-20-edge-winui-fix.md` §6:

| # | App | 期望 |
|---|---|---|
| 1 | Edge Modern Save As(WinUI 3) | filename 框显示路径(Task B + C 修复后应更清晰)+ 弹 Edge tip 通知 |
| 2 | Chrome Save As | 41477 + Invoke 不变 |
| 3 | 影刀 / Notepad / Word / VS Code | COM SetFolder 不变 |
| 4 | Edge tip notification | **首次激活看到** + toolbar "🔁 Re-show Edge tip" 按钮工作 |
| 5 | Sidecar not reachable notification | **首次激活 5s 后才出现**(Bug C 修复) |

### 6.2 WinUI 3 filename + Enter 验证(决策 focus-22)

按 `retrospective rev.3` §3.1 测试方法,3 种格式手测,记录结果。

### 6.3 Edge legacy flag 验证

按 `retrospective rev.3` §13.1 K 测试方法,启用 flag + 重启 + 观察 dialog class。

---

## 7. 当前环境状态(精确)

### 7.1 working tree M 文件

```
M sigma-listary-spike/Cargo.lock
M sigma-listary-spike/Cargo.toml
M sigma-listary-spike/README.md
M sigma-listary-spike/src/config.rs
M sigma-listary-spike/src/fg_bypass.rs
M sigma-listary-spike/src/lib.rs
M sigma-listary-spike/src/logger.rs
M sigma-listary-spike/src/main.rs
M sigma-listary-spike/src/state.rs
M sigma-listary-spike/src/uia_event.rs
M sigma-listary-spike/src/uia_inject.rs
M sigma-listary-spike/src/writer.rs
M release/extension/dist/index.js  ← focus-19/20 改动
?? sigma-listary-spike/src/http_server.rs  ← untracked
?? docs/superpowers/plans/2026-10-02-*.md  ← 多个新文档
?? release/Sigma-File-Manager-2.2.0-focus-19-setup.exe
?? release/Sigma-File-Manager-2.2.0-focus-20-setup.exe  ← focus-20 v0.3.3 (17,071,214 bytes)
?? handoff-2026-10-02-focus-19-edge-winui-save-as-failure.md
?? handoff-2026-10-02-focus-20-edge-winui-fix.md
?? handoff-2026-10-02-focus-21-v0.3.4-bug-fixes.md  ← 本文档
?? task.md
```

**git status**: working tree 有 ~13 个 M 文件 + 多个 untracked,全部未 commit(沿用 user authorization policy)

### 7.2 spike.exe 当前运行状态

```
focus-sync-sidecar.exe  PID 79484  LISTENING  127.0.0.1:37421  v0.3.3
```

注意:**Task A 修复后需要重启 spike.exe**(`taskkill /F /T /IM focus-sync-sidecar.exe`,然后 Scheduled Task 重新启动)。

### 7.3 /health 当前快照

```json
{
  "status": "ok",
  "version": "0.3.3",
  "mode": "service",
  "unsupported_dialog_count": 2  // Edge WinUI 累积
}
```

**Task A 修复后**,用户重启 spike → version 0.3.4 + UIA 正常工作(消除 COM 隐式初始化风险)。

---

## 8. 8 条硬规则(retrospective rev.3 新增/强化)

**规则 1**:新 dialog 类型的支持,必须先 user-verify dialog 的真实控件结构。

**规则 2**:fix 之后,必须 user-verify "path 在正确字段显示"。

**规则 3**:3+ fix 失败时,质疑上游/架构假设,不质疑实现细节。

**规则 4**:不发"已知错的方案"。

**规则 5**:retrospective / plan 中**禁止写"预期结果"**(写"测试方法 + 待 user-verify + 结论待填")。

**规则 6**(rev.3 新增):**源码 bug 检查应在每轮 review 中**。5 轮 review 都只看文档,没人查实际代码导致 Bug A 延迟 5 轮才发现。**review checklist 必须包含"打开 main.rs / index.js 看实际代码"**。

**规则 7**(rev.3 新增):threading + COM 是 Rust Windows 编程的雷区。每个用 COM 的线程独立 CoInitializeEx。

**规则 8**(rev.3 新增):一次性 toast notification 不能依赖用户记忆。重要提示必须有永久入口(toolbar button / settings UI / README)。

---

## 9. 文档链接(给后续会话)

**核心 5 文档**:
1. `docs/superpowers/plans/2026-10-02-focus-20-retrospective-architecture-failure-rev3.md` ← **必读第一**
2. `handoff-2026-10-02-focus-20-edge-winui-fix.md` ← focus-20 状态
3. `handoff-2026-10-02-focus-19-edge-winui-save-as-failure.md` ← focus-19 原始问题
4. `handoff-2026-10-01-focus-18-path-activation-auto-confirm.md` ← focus-18 背景
5. `docs/superpowers/plans/2026-10-02-focus-19-retrospective-deep-dive.md` ← retrospective #0

**Plan + Review**:
- `docs/superpowers/plans/2026-10-02-focus-20-edge-winui-fix.md`(focus-20 plan,基于错的假设)
- `docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-revised.md`(focus-19 rev.2)
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-architecture-failure-rev2.md`(retrospective #2,已被 #3 替代)
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-rev2-review.md`(Mavis 第 6 轮 review)
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-architecture-failure.md`(retrospective #1)

**spike.log 证据**:
- `C:\Users\Duanyi\AppData\Local\kizemo\focus-sync\logs\spike.log`(JSON 事件)
- `C:\Users\Duanyi\AppData\Local\kizemo\focus-sync\logs\spike.log.2026-10-02`(tracing INFO/WARN)

**Build 产物**:
- `release/Sigma-File-Manager-2.2.0-focus-19-setup.exe`(17,070,746 bytes)
- `release/Sigma-File-Manager-2.2.0-focus-20-setup.exe`(17,071,214 bytes, MD5 `882005bb...`)
- 即将 build: `release/Sigma-File-Manager-2.2.0-focus-21-setup.exe`

---

## 10. prompt 一句话(给下一个 session)

```
读 handoff-2026-10-02-focus-21-v0.3.4-bug-fixes.md 全文(必读),
然后读 docs/superpowers/plans/2026-10-02-focus-20-retrospective-architecture-failure-rev3.md 的 §9 + §10 + §12,
按 focus-21 v0.3.4 plan 实施 4 必修 Bug 修复(A/B/C/D) + 集成测试 + build,
完成后不要 commit git(等用户授权),把 focus-21-setup.exe 路径报告给用户。
```
