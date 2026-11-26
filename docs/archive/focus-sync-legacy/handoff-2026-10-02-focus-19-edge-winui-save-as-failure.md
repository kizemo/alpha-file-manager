# handoff-2026-10-02-focus-19-edge-winui-save-as-failure

**接力**: Claude (MiniMax-M3) → 下一个 session
**日期**: 2026-10-02
**会话边界**: 当前 session ~活跃 50 分钟,完成 focus-18 v0.3.1 build 后用户实测失败;新写 focus-19 深度分析文档
**任务**: 实现 focus-19 (WinUI BreadcrumbBar 识别 + 跳过不支持 dialog)

---

## 0. TL;DR (30 秒摘要)

1. **focus-18 v0.3.1 已 build** 但用户实测失败 — 路径 `C:\DrvPath` 被填到 Edge Win11 Save As filename 框
2. **根因清晰**: focus-18 把 "WinUI Save As" 等价于 "Chromium Save As",fallback 写 FULL PATH 到 filename 输入框
3. **WinUI ≠ Chromium**: Edge Win11 22H2+ Save As 是 WinUI 3,有 BreadcrumbBar 而非 41477
4. **新诊断文档**: `docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-deep-analysis.md` (10 节,3 选项 + 推荐)
5. **新会话第一件事**: 读新诊断文档 §4 + §5,然后用 executing-plans 技能开始 focus-19 Task 1

---

## 1. focus-18 v0.3.1 状态(本会话已完成)

### 1.1 Build 产物
- ✅ `release/Sigma-File-Manager-2.2.0-focus-18-setup.exe` (17,071,532 B, MD5 `65837b8d...`)
- ✅ spike.exe MD5 `bdb16e86...`(三个位置一致)
- ✅ subsystem = 2 (GUI, 无 console 弹窗)
- ✅ SPIKE_VERSION = 0.3.1
- ✅ cargo test 40/41 (1 pre-existing config fail)
- ✅ 8 unit tests added (fg_bypass 3 + state 3 + writer 2)

### 1.2 改动文件 (10 个,~250 行)
- sigma-listary-spike/src/fg_bypass.rs (新建,从 905db31 恢复 + 3 tests)
- sigma-listary-spike/src/lib.rs (+pub mod fg_bypass)
- sigma-listary-spike/src/writer.rs (新签名 + fg_bypass 包裹 + 2 tests)
- sigma-listary-spike/src/uia_inject.rs (FULL PATH + Invoke + Enter)
- sigma-listary-spike/src/state.rs (+committed_hwnds + 3 tests)
- sigma-listary-spike/src/http_server.rs (SetPathBody.auto_confirm + SPIKE_VERSION 0.3.1)
- sigma-listary-spike/src/main.rs (DialogEvent::Closed → clear_committed)
- release/extension/dist/index.js (autoConfirm state + toolbar)
- release/extension-installer/register.ps1 (focus-sync.toggleAutoConfirm command)
- release/Sigma-File-Manager-2.2.0-focus-18-setup.exe (新)

### 1.3 git commit 状态
**NO commit during execution** — per user authorization policy (handoff §1 + plan §8 Step 5)
- 6 个 sigma-listary-spike/src/*.rs 已 M (tracked changes)
- sigma-listary-spike/src/http_server.rs ?? (untracked — 用户没 commit)

---

## 2. focus-18 v0.3.1 用户实测失败(本会话新发现)

### 2.1 用户反馈
用户装好 focus-18 setup.exe 后实测 Edge Win11 22H2+ Save As dialog:
- 文件名框显示 `C:\DrvPath` (用户的 Sigma FM 路径)
- **未触发导航** — dialog 没跳到 `C:\DrvPath` 目录
- **路径被错误填到文件名处**

### 2.2 用户截图关键观察 (`8735390e-2dcb-4fba-9b82-c210e3230a61/e3264c00-95d6-4255-aed8-bf91ec229812-image.png`)
- 顶部 breadcrumb: `此电脑 > photos (G:) > video` — **WinUI BreadcrumbBar**
- 左侧 navigation tree: Win (C:) / Program (D:) / Office (E:) / 数据 (F:) / photos (G:) / 网络
- 文件名框值: `C:\DrvPath` (focus-18 写入的)
- 文件类型: Application (*.exe)

### 2.3 真正的 dialog 类型
**Windows 11 22H2+ Edge WinUI 3 Save As**:
- COM `AccessibleObjectFromWindow` 失败(WinUI 不实现 IServiceProvider)
- UIA 没有 41477(41477 是 Chromium Save As 的 address bar ID)
- WinUI BreadcrumbBar 不被 focus-18 识别
- focus-18 fallback 把 FULL PATH 写到 filename 输入框(WinUI filename 框不接受路径)

### 2.4 早识别但未被采纳
- `docs/superpowers/plans/2026-10-01-focus-18-path-activation-architecture-regression.md` §3 已经识别 Edge 有 2 个 dialog 实现(Legacy IFileDialog COM / Modern WinUI 3)
- 但 focus-18 plan (path-b-auto-confirm-toggle-plan) 没采纳这个发现,按 PathWrap upstream (Chromium) 路径实现
- 用户的 WinUI Save As 实际不被 focus-18 支持

---

## 3. 必须做的(下一会话)

### 3.1 写 focus-19 plan
**已写**: `docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-deep-analysis.md`
- 10 节 + 3 修复选项 + 推荐(Option A + B 组合)
- 短期 focus-19 = Option B (跳过 + notification)
- 中期 focus-20+ = Option A (识别 WinUI BreadcrumbBar)

### 3.2 等用户跑 spike.log 收集脚本
新会话第一件事:让用户跑:
```powershell
Get-Content "$env:LOCALAPPDATA\kizemo\focus-sync\logs\spike.log.2026-10-02" -Tail 1000 |
  Select-String -Pattern "Dialog UIA inventory|address bar|41477|wrote FULL|AccessibleObjectFromWindow|filename edit|Committed via" |
  Select-Object -Last 50
```
确认 root cause 是 WinUI 路径(没有 41477 + AccessibleObjectFromWindow fail + wrote FULL to filename edit)

### 3.3 用 executing-plans 执行 focus-19
**预计时间**: 2-3 小时增量(主要在 uia_inject.rs 检测逻辑)
**预计改动**: ~150 行,主要是删除"写 FULL PATH 到 filename 框" fallback,加 `detect_winuui_breadcrumb` 桩 + `find_dialog_class` 识别

### 3.4 用户手动测试 (必做)
6-app 矩阵(focus-18 是 7-app):
- Edge WinUI Save As → 跳过 + notification (focus-19 行为)
- Chrome Save As (Chromium) → 41477 + Invoke (focus-18 已 OK)
- 影刀 RPA Save As (native IFileDialog) → COM SetFolder (focus-17 已 OK)
- Notepad Save As (native IFileDialog) → COM SetFolder
- Word Save As (native IFileDialog) → COM SetFolder
- VS Code Save As (native IFileDialog) → COM SetFolder

### 3.5 完成判据 (focus-19)
- cargo test 全部 PASS (40/41 + 新增 ~5 ≈ 45)
- cargo build --release 成功
- Tauri build 产出 `release/Sigma-File-Manager-2.2.0-focus-19-setup.exe`
- 安装 focus-19: 无 console 弹窗
- /health 返回 `unsupported_dialog_count` 字段
- Sigma FM toolbar: 增加 notification 报告不支持的 dialog
- **Edge WinUI Save As**: 不写 filename 框 + 弹 notification
- **Chrome Save As**: 41477 + Invoke(保持 focus-18 行为)
- **Native IFileDialog**: COM SetFolder (保持 focus-17 行为)

---

## 4. 必读顺序 (从最重要到最次要)

新会话第一件事:按顺序读这 5 个文件(每读完一个,API 在哪心里有数):

1. **`docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-deep-analysis.md`** (新)
   - 根因分析 + 3 修复选项 + 推荐
   - 必读 §4 (修复方案) + §5 (用户 spike.log 收集方法)
2. **`docs/superpowers/plans/2026-10-01-focus-18-path-activation-architecture-regression.md`** §3
   - Edge 有 2 个 dialog 实现的早识别(被 focus-18 plan 漏掉)
3. **`docs/superpowers/plans/2026-10-01-focus-18-path-b-auto-confirm-toggle-plan.md`**
   - focus-18 plan 的设计哲学(把 WinUI 当 Chromium 的根源)
4. **`handoff-2026-10-01-focus-18-path-activation-auto-confirm.md`**
   - focus-18 之前的交接状态 + 7-app 矩阵测试
5. **`docs/superpowers/plans/2026-10-01-focus-18-download-dialog-deep-analysis.md`** §2 根因 C
   - Chromium download dialog 架构性不支持(类似 WinUI 但场景不同)

---

## 5. 当前环境 / 分支 / 基线

| 项 | 值 |
|---|---|
| **分支** | `feat/recovery-2026-09-28` (用户当前开发分支) |
| **基线** | `main` (无 HEAD commit) |
| **working tree 状态** | focus-18 改动 M (6 Rust 文件) + focus-16 改动 M (Cargo.lock/toml/README/config/uia_event) + ?? (http_server.rs); NO commit |
| **rustc 版本** | `1.90.0-x86_64-pc-windows-msvc` (用户硬装) |
| **PATH 必须** | `export PATH="/c/Users/Duanyi/.rustup/toolchains/1.90.0-x86_64-pc-windows-msvc/bin:$PATH"` |
| **Cargo.lock 强制降级** | `tracing-appender=0.2.3`, `time=0.3.41`, `thiserror=1.0.69`, `time-core=0.1.4`, `time-macros=0.2.22` |
| **cargo test 已知 fail** | `config::tests::resolve_port_strict_on_conflict` — 1 个,pre-existing |

---

## 6. 避坑提示

### 6.1 工具链
- **rustc 1.97 / stable 坏**: 必须用 rustc 1.90
- **cargo PATH**: bash 默认找不到 cargo。每次 cargo 前 `export PATH=...`(或 alias)
- **npm 跑子进程丢失 PATH**: `npm run tauri build` 需在同条命令前 export PATH

### 6.2 PowerShell heredoc 在 bash 里脆弱
- 如果做 PS 验证,**写 .ps1 文件**用 Write tool,然后 `powershell -File xxx.ps1`

### 6.3 git 操作
- **NO commit during execution** — focus-19 同样要用户授权 commit
- **git add 不要用 `-A` 或 `.`** — 给具体文件路径

### 6.4 focus-18 已修保留
- 保留 focus-17 已修: windows_subsystem / log 文件 / basename 函数
- 保留 focus-18 已修: fg_bypass / Invoke / Enter / FULL PATH / Auto-Confirm toggle / per-dialog dedup
- **focus-19 只改**: 删除"写 FULL PATH 到 filename 框" fallback + 加 WinUI breadcrumb 识别桩

### 6.5 测试断言
- **basename("C:\\") 返回 "C:"** — focus-17 改对,focus-19 不要撤销

---

## 7. 关键证据(给新会话的"why"上下文)

### 7.1 用户场景(实测)
- 用户在 Windows 11 22H2+ 用 Edge 触发 Save As (例如右键图片 → 图片另存为)
- Sigma FM 当前路径 = `C:\DrvPath` (测试路径)
- focus-18 PUSH 路径 → spike uia_inject → COM fail → 无善
- filename 框被污染为 `C:\DrvPath` → 用户感知"路径被错误填到文件名处"

### 7.2 focus-18 uia_inject.rs 关键代码路径
`writer::write_path` (focus-18 new sig) →
`fg_bypass::run_with_bypass` →
`uia_inject::inject_folder_path(hwnd, target, should_commit)`:
1. try_set_folder_via_com → WinUI fail → warn
2. inject_via_uia(hwnd, target, should_commit) → FindAll → no 41477 → best_edit (filename) → SetValue(FULL PATH) → WRONG

### 7.3 Edge Win11 22H2+ Save As 真实结构
- BreadcrumbBar: ControlType.Pane, name 含 "此电脑" / "photos" / "video"
- filename TextBox: 通常 automation_id=104 或 "文件名"
- Save 按钮: automation_id="1" (IDOK)
- COM 不支持 (WinUI 不实现 IServiceProvider)

### 7.4 focus-18 没识别 (3 个原因)
1. plan 把 WinUI 当 Chromium 走 PathWrap upstream 路径
2. `find_any_file_dialog` 只识别 class "Chrome_WidgetWin_*" 和 "#32770",不识别 WinUI
3. fallback "写 FULL PATH to best_edit" 是 Chromium 设计,不适用 WinUI

---

## 8. 已建立的工作树状态(必须 verify 后才能 commit)

修改文件(per focus-19 期望):
- sigma-listary-spike/src/uia_inject.rs (删 fallback + 加 detect_winuui_breadcrumb 桩 + find_dialog_class)
- sigma-listary-spike/src/http_server.rs (/health 加 unsupported_dialog_count)
- release/extension/dist/index.js (polling /health,弹 notification)
- release/extension-installer/register.ps1 (加 focus-sync.reportDialog command)

新文件(可能):
- 无(主要改 uia_inject.rs)

---

## 9. 给下一个 session 的"不要做"清单

- ❌ **不要修改 focus-18 已修的 6 个 Rust 文件**(除非加新功能)— focus-19 主要改 uia_inject.rs (删 fallback) + http_server.rs (新字段)
- ❌ **不要替换 cargo** — rustc 1.90 在 `/c/Users/Duanyi/.rustup/toolchains/1.90.0-x86_64-pc-windows-msvc/bin/`
- ❌ **不要 --force-push** — 用户没说
- ❌ **不要碰 user-extensions.json / Sigma FM 配置** — focus-19 plan 不需要
- ❌ **不要 untrack release/*.exe** — 用户自己管理
- ❌ **不要删除任何 user 文件** — CLAUDE.md 红线
- ❌ **不要改 Cargo.toml dependencies** — 除非新功能需要
- ❌ **不要把 basename("C:\") 改回 dirname()** — focus-17 已修对
- ❌ **不要 commit focus-18 改动**(等用户授权 commit v0.3.1 + v0.3.2)

---

## 10. 完成的最终交付

执行完成后,向用户报告:

1. **focus-19 v0.3.2 计划 5+ 项** ✓ 全部完成
3. **`release/Sigma-File-Manager-2.2.0-focus-19-setup.exe` 已生成**,MD5 与 spike.exe 验证一致
4. **git log 干净**,2 个 commit (focus-18 v0.3.1 + focus-19 v0.3.2)
5. **memory 更新**: focus_sync_v031_focus18.md 应更新为 v0.3.2

---

## 11. prompt 一句话(放在 prompt-focus-19-next-session.md)

```
读 handoff-2026-10-02-focus-19-edge-winui-save-as-failure.md 全文,然后读 docs/superpowers/plans/2026-10-02-focus-19-edge-winui-save-as-deep-analysis.md 按 Option B 短期方案(删除 "写 FULL PATH 到 filename 框" fallback + 加 dialog type 检测 + 通知用户)开始 focus-19 Task 1,完成后 build focus-19-setup.exe 到 release/,不要 commit git(等用户授权)
```