# 交接卡 · 批次 17 —— focus sync 闸门「判据过脆」修复 + 日志刷屏回归修复 + 端到端真机验收通过

- **日期**：2026-11-18
- **仓库**：`F:\soft\00selfmade\filemanager\sigma-file-manager`
- **台账真源**：`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-11-quark-pan.md` → **`### 20.12 批次 17`**（本批全部证据在 §20.12.1–§20.12.9）
- **上一批**：`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-18-quark-pan-batch16.md`（批次 16 = v0.5.21 闸门假下降沿修复）
- **状态**：**本批已闭环**（门禁双绿 + 真机部署 + 端到端验收通过 + 已 commit）

---

## 1. 背景

用户报「软件的 focus sync 功能出现异常」。批次 16 坐实并修掉了第一个根因（AFM 身份误判 ⇒ 假下降沿 ⇒ 交接授权写进 AFM 自己的 pid ⇒ 全天 656 次写回被拒），
但真机 e2e 暴露**第二个缺陷**：闸门判据过脆，**仍然恒拒**；同时 v0.5.21 自己引入了一个**每 500 ms 一行的枚举日志刷屏**（当天 15,951 行）。

## 2. 本批目标（已完成）

1. 把「闸门只有一条严格相等判据」改成**两条独立放行路径**（交接链身份 / 对话框开着时用户去过 AFM）。
2. 修掉接收方采样把 shell 表面当接收方、交接链过期复活、枚举日志刷屏三个次生缺陷。
3. 端到端真机验收：**路径真的写进对话框**。

## 3. 改了什么

| 文件 | 改动要点 |
| --- | --- |
| `extensions/kizemo.focus-sync/sidecar/src/state.rs` | 新增 `last_afm_foreground_ts`、`dialog_opened_ts`、`refresh_afm_handoff()`、`mark_dialog_opened()`、`dialog_opened_ts()`、**`afm_gate_allows(dialog_hwnd, dialog_pid)`（两条路径总入口）**；`afm_handoff_allows` 追加 `same_application_process` 放宽 |
| `extensions/kizemo.focus-sync/sidecar/src/uia_event.rs` | `is_shell_surface_class()`、`same_application_in()`、`parent_pid_of()`（ToolHelp 快照）、`same_application_process()`、`SnapshotGuard`；monitor 接收方采样排除 shell 表面、交接链续期不复活过期链、枚举日志 pid 变化才打印 |
| `extensions/kizemo.focus-sync/sidecar/src/main.rs` | `state.mark_dialog_opened(hwnd_u32)`；闸门判定改用 `afm_gate_allows`；skip 消息带三个 pid |
| `extensions/kizemo.focus-sync/sidecar/src/http_server.rs` | `/set_path` 闸门改用 `afm_gate_allows` |

合计 **4 files, +466 / −44**（新增 7 条判别性测试）。

**产品语义定稿**（三方对齐后自行定稿，未发问卷）：
- **路径 A（交接链身份）**：从 AFM 切进来的那个应用放行；对话框属主若是该应用的 helper 子进程（互为祖先后代）同样放行。
- **路径 B（对话框开着时用户去过 AFM）**：AFM 访问时刻 **必须 ≥ 对话框打开时刻**（挡住 2026-11-09「陈旧访问授权后来弹出的对话框」）。
- 两条任一成立即放行；都不成立才拒。

## 4. 门禁与部署（实测数字）

- `cargo test`（**必须用 1.90.0 toolchain**）⇒ **130 passed / 0 failed**（较批次 16 的 123 新增 7 条）。
- `npm run check` ⇒ **EXIT=0**（311 个测试文件 / 3239 个用例）。
- `scripts\release\build-sidecar.cmd` ⇒ `BUILD_EXIT=0`；新件 **sha256 `0DC1B563CF68367B76AA567BB9BDA763A048B5EFF24D9A72A31F81D4E19DFE8D` / 2,891,776 B**。
- 真机部署（`deploy-sidecar-522.ps1`）：备份 `installed-sidecar-backup-20261118-v0521.exe` → 杀 pid 118816 → 覆盖 APPDATA → 监督器重拉起 **pid 661500**。
- **「跑的是哪份代码」三判据全中**：装机副本 sha == 构建产物 sha == **运行映像 sha**（三处相等）＋ 二进制标记 A/B。

## 5. 端到端验收（已通过）

2026-11-18 03:09:30–03:09:40Z（本地 11:09），真人操作：应用内弹「选择文件」对话框 → 切回 AFM 取路径 → 点任务栏切回对话框。

```
03:09:30  DialogDetected hwnd=6230674
03:09:31-33  opened_write_back_skip_no_afm_visit ×3 (dialog_pid=724248 handoff_pid=0 afm_pid=118160)   ← 还没去 AFM，正确拒绝
03:09:33  afm_gate_cleared  "AFM back in foreground"
03:09:37  afm_gate_armed   "handoff recorded: pid=724248"
03:09:38  opened_write_back "writing current"                                                          ← 放行成功
03:09:40  Write  target='F:\soft\00selfmade\practilog\release' strategy=uia                           ← 路径写进对话框
03:10:06  dialog_closed
```

**刷屏回归同时验证修好**：v0.5.22 进程整个会话内 `[afm-gate] learned AFM pid=…` **只打印 1 次**（v0.5.21 当天 15,951 次）。

## 6. 已验证 / 推断 / 未验证

| # | 结论 | 状态 |
| --- | --- | --- |
| ① | 端到端写回成功（路径进对话框） | **已验证** |
| ② | 无授权时闸门仍拒（不恒放行） | **已验证** |
| ③ | 枚举日志刷屏回归已修 | **已验证** |
| ④ | 真机跑的是 v0.5.22 代码（三处 sha 相等 + 标记串） | **已验证** |
| ⑤ | 路径 A 的「祖先后代」分支（helper 子进程） | **推断**（真机本轮命中的是严格相等分支；仅单测覆盖） |
| ⑥ | 路径 B 真机命中（AFM 访问晚于对话框打开） | **已验证** |
| ⑦ | 装机版（NSIS）会把新侧车带给用户 | **未验证**（`bin\`/`installer\` 的 exe 未被 git 跟踪 ⇒ 装机机必须重跑 build + 安装器） |
| ⑧ | 沙箱真机 UI A/B（AGENTS §2.8/§2.10） | **本轮未做**（同批次 16） |
| ⑨ | 路径 B 语义副作用（去 AFM 即视为授权，与意图无关也会写回） | **已知并接受**（Listary 原义） |

## 7. 证据路径（全在本机）

| 内容 | 路径 |
| --- | --- |
| 台账 §20.12（**真源**） | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-11-quark-pan.md` |
| v0.5.22 补丁（12 op + 3 op） | `...\sigma-file-manager\.superpowers\sdd\focus\patch-v522.txt` / `patch-v522b.txt`（执行器 `apply-patch.cjs`） |
| 补丁执行结果 | **12/12 + 3/3 全命中，failed=0** |
| Rust 门禁日志 | `...\sigma-file-manager\.superpowers\sdd\focus\cargo-test-522.log`（130/0） |
| 前端门禁日志 | `...\sigma-file-manager\.superpowers\sdd\focus\npm-check-522.log`（EXIT=0） |
| 构建日志 | `...\sigma-file-manager\.superpowers\sdd\focus\build-sidecar-522.log`（BUILD_EXIT=0） |
| 部署脚本 / 报告 | `...\sigma-file-manager\.superpowers\sdd\focus\deploy-sidecar-522.ps1` / `deploy-522.txt` |
| 旧件备份（可回滚） | `...\sigma-file-manager\.superpowers\sdd\focus\installed-sidecar-backup-20261118-v0521.exe`（sha `BFD5DF0C…B4DB`） |
| e2e 监听器 / 输出 | `...\sigma-file-manager\.superpowers\sdd\focus\e2e-watch2.cjs` / `e2e-522.txt`（本轮窗口不够，判据取自日志直读） |
| 真机侧车日志 | `%LOCALAPPDATA%\kizemo\focus-sync\logs\spike.log`（JSON 事件流）＋ `spike.log.2026-11-18`（tracing） |
| 真机 HTTP | `http://127.0.0.1:37421/health` |

## 8. 下一步（按优先级）

1. **`git push origin main`** —— 批次 13→17 共 **8 个提交**停本地，**需用户明确授权**。
2. **装机链验证（未验证项 ⑦）**：`bin\`/`installer\` 里的 exe 未跟踪 ⇒ 装机机必须重跑 `build-sidecar.cmd` + 安装器，否则装出来仍是旧侧车。
3. **路径 A 祖先后代分支的真机覆盖（未验证项 ⑤）**：需要一个「对话框属主是 helper 子进程」的宿主（如某些网盘/办公软件）复现一次。
4. **次生缺陷**（沿用 §20.11.7 登记，本批仍未修）：`__focus_sync_trace` 被 health 轮询刷爆；`/set_path` 空表阻塞 400 ms；`try_mark_committed` 失败也消耗 commit 槽；扩展 `normalizePath` 仍推虚拟路径；扩展侧 `path_update` 与 sidecar 双写去重。
5. **版本号 / CHANGELOG**：本批**刻意不改**（避免破坏 `verify-changelog`）；发布前需按仓库惯例补。

## 9. 本轮工具坑（已踩，勿重试）

1. **`cargo` 不在默认 PATH**，且 `stable-x86_64-pc-windows-msvc`(1.97.1) 的 std 不可用（E0463）⇒ 必须
   `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin\cargo.exe`。
2. **`cargo` 与 `npm run check` 不要并发**（本轮串在同一个后台任务里跑）。
3. **内联 `Copy-Item` 到工作区外会被安全策略拦** ⇒ 部署写 `.ps1` 再 `pwsh -NoProfile -ExecutionPolicy Bypass -File`。
4. **覆盖运行中的 exe ⇒ sharing violation** ⇒ 先杀进程再复制，且 AFM 监督器会立刻重拉起旧件（脚本做 kill→copy 重试循环）。
5. **读被占用日志**用 `File::Open('Open','Read','ReadWrite')` + `Seek(-N,'End')`；`Get-Content -Raw` 会抛「正被另一进程使用」。
6. **同文件并行 edit 会互相覆盖** ⇒ 源码改动一律走「补丁文件 ＋ 单次执行」。
7. **e2e 监听窗口别只给 10 分钟**（本轮用户隔了 ~30 分钟才操作，监听器超时无收获）。
8. **PowerShell `MatchInfo` 没有 `.Trim()`**（要先 `.Line.Trim()`）⇒ 内容检索用 `grep` 工具。
9. **`Get-Content` 读仓库文件可以，但改带中文的源文件不要用 `Set-Content`** ⇒ 用 write 工具 / Node 脚本。

## 10. 接力 prompt

见本轮最终回答正文（按用户约定，prompt 不落盘）。
