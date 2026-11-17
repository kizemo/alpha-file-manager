# 批次 16 交接卡：focus sync 异常 —— 闸门「假下降沿」根因修复 ＋ 真机部署（2026-11-18）

> **上游入口卡**：`docs/handoff-2026-11-15-quark-pan-batch15.md`（批次 15）。
> **完整台账（本轮唯一真源）**：`docs/handoff-2026-11-11-quark-pan.md` **§20.11**（批次 16 全部证据、工具坑、矩阵都在那里）。
> 本卡只是入口摘要；细节以 §20.11 为准。

## 0. 一句话状态

focus sync 的「写了但没生效」根因已坐实并修复：交接闸门把 **AFM 自己的 pid** 当成了授权 pid，导致 656 次写回被拒；
修复已落地并通过双门禁，**真机 APPDATA 侧车已换成新构建并在运行**；端到端「打开对话框 → 写回成功」的真机交互验收仍**未做**（需真人触发一次对话框）。

## 1. 背景与目标

- **用户诉求**：软件的 focus sync 功能出现异常 ⇒ 查使用日志 → 分析 bug → 修复。
- **focus-sync 是什么**：`extensions/kizemo.focus-sync` 扩展（宿主 `dist/index.js`）＋ Rust 侧车 `sidecar/`（常驻进程，读 UIA、路径写回）。
  宿主把「当前目录」推给侧车；用户在系统文件对话框里选目录时，侧车把该路径写回宿主，实现「两边目录同步」。
- **本轮必须守住的硬约束**（AGENTS §2.8 / §2.10）：
  - 真机 UI 验收只能跑在 `CreateDesktop` 隔离沙箱桌面；注入端不切桌面、不 `SetForegroundWindow`、不发 `SendInput`；枚举用 `EnumDesktopWindows`；
  - 真机结论只认 `npm run tauri:build` 的打包产物（`tauri dev` 不用）；
  - 真实用户数据必须「备份 → 灌夹具 → 验收 → 恢复」，恢复后核对字节数；
  - 对外动作（`git push` / winget PR / GitHub Release）需用户明确授权；
  - 中间产物落文件（`.superpowers/sdd/` 在 gitignore 内），对话里只留指针 ＋ 结论。

## 2. 结论一：根因（**已由 tracing 日志坐实**，不是推测）

- 旧 `foreground_is_afm()` 只认「class == `Tauri Window` 且标题以 `Alpha File Manager` 开头」。
- 装机版 AFM 进程（真机 pid **72772**）另持有**可见**顶层窗口 `hwnd=24842270 class='com.sigma-file-manager.app-sic' title='com.sigma-file-manager.app-siw'` ⇒ 不满足判据。
- 该辅助窗口在前台时 `afm_now` 恒 false ⇒ **假下降沿** ⇒ 交接授权写进了 **AFM 自己的 pid**
  （tracing 原文 11-17 12:15:00.188Z：`handoff recorded: pid=72772`）。
- ⇒ 之后所有真实对话框 pid ≠ 72772 ⇒ 闸门恒拒 ⇒ **656 次 `opened_write_back_skip_no_afm_visit`**，全天只写回成功 1 次。
- 次要污染：扩展把 `sfm://locations`（虚拟位置）当路径推给侧车 ⇒ `current_path` 被写成虚拟路径。

## 3. 结论二：修复（v0.5.21，已落地）

改动文件：`extensions/kizemo.focus-sync/sidecar/src/{state.rs, uia_event.rs, main.rs, http_server.rs}`（**4 files, +365 / −18**）

1. **AFM 识别改为「进程身份」**：`AppState.afm_pid` 记忆 pid；前台 class+title 命中即学；每 500 ms `EnumWindows` 枚举补学（含隐藏窗口）；
   `foreground_is_afm_by(known_afm_pid, class_title_match, hwnd_pid)` ＝ class+title 命中 **或** 前台 pid == 已知 AFM pid。
2. **闸门兜底**：`afm_handoff_allows` 在 `handoff.pid == afm_pid` 时直接拒绝（假下降沿再来也不放行）。
3. **决策可观测**：`afm_gate_cleared` / `afm_gate_armed` / `afm_gate_retry` 三个 JSON 事件；skip 消息带 `dialog_pid`/`handoff_pid`/`afm_pid`。
4. **路径守卫**：`set_current_path` 先做形状校验（盘符 `X:\`、`X:/` 或 UNC，只查形状），非法形状**存空串** ⇒ 虚拟位置期间不写任何路径。
5. `path_update` 事件改报**实际存入**的路径 ＋ 原 pushed 值。
6. **刻意不做**：不改扩展 `dist/index.js`（侧车已防御）；不加 hwnd 强校验（Chromium 导航会重建 hwnd，实测 382 个不同 hwnd）。

## 4. 结论三：门禁 ＋ 真机部署（本轮实测）

- `cargo test` ⇒ **123 passed / 0 failed**（⚠️ 必须用 **1.90.0** toolchain，`stable` 的 std 不可用）。
- `npm run check` ⇒ **EXIT=0**。
- `scripts\release\build-sidecar.cmd` ⇒ `BUILD_OK`（50.11 s）⇒ 新件 sha256 `BFD5DF0C…B4DB` / 2,882,048 B。
- 部署：备份旧件（sha `A8300AAB…52E462`）→ 杀旧侧车 pid 301088 → 覆盖 APPDATA 副本 → AFM 监督器自动重拉起 **pid 277632**，
  **运行中进程映像 sha256 == 新构建**。

## 5. 改动清单（文件级）

| 文件 | 改动 |
| --- | --- |
| `extensions/kizemo.focus-sync/sidecar/src/state.rs` | `afm_pid` 字段 ＋ `learn_afm_pid`/`afm_pid`；`afm_handoff_allows` 增 pid 兜底；`is_local_path_shape()`；`set_current_path` 形状守卫；`v521_afm_identity_tests`（7 个） |
| `extensions/kizemo.focus-sync/sidecar/src/uia_event.rs` | `foreground_is_afm_by()` 纯函数；`find_afm_pid_by_enum()`；`FOUND_AFM_PID`；monitor 改为「先学 pid 再判前台」＋500 ms 枚举补学；闸门事件；`v521_afm_identity_tests`（4 个） |
| `extensions/kizemo.focus-sync/sidecar/src/main.rs` | skip 消息加 `dialog_pid`/`handoff_pid`/`afm_pid` |
| `extensions/kizemo.focus-sync/sidecar/src/http_server.rs` | `path_update` 改报实际存入路径 |
| `docs/handoff-2026-11-11-quark-pan.md` | 新增 §20.11 |
| `docs/handoff-2026-11-18-quark-pan-batch16.md` | 本卡 |

- `extensions/kizemo.focus-sync/bin/focus-sync-sidecar.exe` 与 `installer/focus-sync-sidecar.exe` **未被 git 跟踪**
  （重建后内容已更新，但不入版本库）⇒ 装机机必须自己跑 `build-sidecar.cmd` ＋ 安装器。
- `.superpowers/sdd/focus/`（补丁、取证、部署脚本、日志）全在 gitignore 内。

## 6. 验证矩阵

### 6.1 已验证（实测，有输出）

| 结论 | 证据 |
| --- | --- |
| 根因坐实 | `spike.log.2026-11-17` 中 `handoff recorded: pid=72772`，而 72772 = AFM 自己的 pid；`gate17.txt` 74 条 afm-gate 行 |
| 症状量化 | 11-17：DialogDetected 809 次 / 382 个 hwnd；写回成功 1 次；`skip_no_afm_visit` 656 次（12:31–14:07） |
| Rust 门禁 | 123 passed / 0 failed（`.superpowers\sdd\focus\cargo-test-190.log`） |
| 前端门禁 | `npm run check` EXIT=0（`.superpowers\sdd\focus\npm-check.log`） |
| 真机已是新代码 | 运行映像 sha256 == 新构建；旧/新二进制标记 A/B（旧件 `markers=[]`，新件含 `dialog_pid`/`handoff_pid`/`afm_pid`/`afm_gate_cleared`） |
| 虚拟路径污染已消除（行为级） | 旧：`path_update current_path=sfm://locations`（被存入）；新：`path_update current_path= (pushed='sfm://locations')`（**存入为空**） |
| 新侧车存活 | heartbeat `alive pid=277632` 每 5 s |

### 6.2 推断（有依据，但**未实测**）

- **交接闸门修复在真机上生效**：由 11 条判别性单测 ＋ 进程级确证推断；未做真人对话框交互验收。
- **下次若再复发，日志可自助定位**：`afm_gate_*` 事件 ＋ skip 消息里的三个 pid 足以区分「假下降沿 / 无关应用 / 真的没访问过 AFM」。

### 6.3 未验证（明确没做）

- 端到端「打开对话框 → 写回成功」的真机 UI 交互验收（需真人触发一次系统文件对话框；或把侧车纳入沙箱部署后做 AGENTS §2.8 沙箱 A/B）。
- NSIS 装机链是否把新侧车带给用户（链路线索已查清，但 `bin\*.exe` 不入版本库，装机机须自己 build）。
- 本轮**未**在真机桌面做任何 UI 自动化（违反 §2.8/§2.10 沙箱约束），也**未**动真机用户数据（全程只读）。

## 7. 本轮工具坑（省得下一轮再踩）

1. **`stable-x86_64-pc-windows-msvc`（rustc 1.97.1）的 std 不可用** ⇒ `cargo` 报 `E0463: can't find crate for 'std'`，
   报错落在 `proc-macro2`/`once_cell`/`quote`/`serde_core` 的 build 脚本上 ⇒ **看着像依赖问题，其实是工具链问题**。
   ⇒ 用 `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin\cargo.exe`。
2. **PowerShell `MatchInfo` 没有 `.Trim()`**（要先 `.Line.Trim()`）；`Select-String` 结果直接格式化会刷屏 ⇒ 内容检索用 `grep` 工具。
3. **覆盖正在运行的 exe 会 sharing violation** ⇒ 先杀再复制；AFM 监督器会立刻重拉起旧件 ⇒ 做成 kill→copy 重试循环。
4. **工作区外路径的写操作不能内联** ⇒ 写进 `.ps1` 再 `pwsh -NoProfile -ExecutionPolicy Bypass -File` 执行。
5. **读被占用的日志**用 `File::Open('Open','Read','ReadWrite')` 共享读 ＋ `Seek(-N,'End')` 读尾部；`Get-Content -Raw`/`ReadAllLines` 会抛「正被另一进程使用」。
6. **同文件并行 `edit` 会互相覆盖** ⇒ 源码改动走「补丁文件 ＋ 单次执行」（`apply-patch.cjs`）。
7. `cargo` 不在默认 PATH；**不要与 `npm run check` 并发**；台账追加用 `append-section.cjs`（自动 CRLF 自适应）。
8. 统计日志事件用 `e.kind || e.event`；**不要**拿 `SpikeError` 计数当错误数（heartbeat 也走该类型）。

## 8. 证据路径（全部在仓库内 / 本机日志内）

| 内容 | 路径 |
| --- | --- |
| 补丁执行器 ＋ 19 个 op 的补丁 | `.superpowers\sdd\focus\apply-patch.cjs`、`.superpowers\sdd\focus\patch-v521.txt` |
| 全部 `[afm-gate]` 行（11-10→11-17） | `.superpowers\sdd\focus\gate.txt`、`.superpowers\sdd\focus\gate17.txt` |
| 关键事件序列 / 窗口枚举 | `.superpowers\sdd\focus\seq.txt`、`.superpowers\sdd\focus\windows.txt` |
| 门禁日志 | `.superpowers\sdd\focus\cargo-test-190.log`、`.superpowers\sdd\focus\npm-check.log` |
| 部署脚本 ＋ 报告 ＋ 旧件备份 | `.superpowers\sdd\focus\deploy-sidecar.ps1`、`.superpowers\sdd\focus\deploy-01.txt`、`.superpowers\sdd\focus\installed-sidecar-backup-20261110.exe` |
| 台账正文 | `docs\handoff-2026-11-11-quark-pan.md` **§20.11** |
| 本机日志源 | `%LOCALAPPDATA%\kizemo\focus-sync\logs\spike.log`、`spike.log.2026-11-1*`；`%APPDATA%\com.sigma-file-manager.app\user-data\user-extensions.json` |

## 9. 下一步（按优先级）

1. **端到端真机验收**（最高优先，未做）：在 AFM 里打开一次系统文件对话框 → 侧车应写回成功。
   核验口径：`spike.log` 里出现 `opened_write_back`（而不是 `opened_write_back_skip_no_afm_visit`），
   且 skip 消息若出现则能看出 `dialog_pid`/`handoff_pid`/`afm_pid` 三者关系。
2. **次生缺陷**（§20.11.7 五项，按决策本轮不修）：`__focus_sync_trace` 被 health 轮询刷爆 / `/set_path` 空表阻塞 400 ms /
   `try_mark_committed` 失败也消耗 commit 槽 / 扩展 `normalizePath` 仍推虚拟路径 / 装机不刷新 APPDATA 侧车。
3. **部署链补强**（可选）：让 `bin\focus-sync-sidecar.exe` 与 `installer\focus-sync-sidecar.exe` 进版本库，或在构建流水线里强制 build，
   避免「装机还是旧侧车」。
4. **对外动作**（需用户授权）：`git push origin main`（批次 13/14/15/16 共 5 个提交仍停本地）、winget PR、GitHub Release。
5. **端到端真机验收若要在沙箱做**：需先把侧车二进制纳入沙箱部署位（AGENTS §2.8/§2.10 约束下的真机 UI A/B）。
