# focus-sync v0.3.0 安装后 3 问题修复 Handoff

**日期**: 2026-10-01
**作者**: Claude Code (上一个会话)
**承接**: 用户在安装 `release/Sigma-File-Manager-2.2.0-focus-16-setup.exe` 后报告 3 个问题

---

## ⚡ 一句话 Prompt(给下一个 session)

> 继续修复 focus-sync v0.3.0 安装后的 3 个用户报告问题(`#1` 命令行弹窗、`#2` 版本显示 v0.2.0、`#3` 对话框 sync 失败 — Chrome 下载弹窗写入 "C:" 到 filename 但地址栏不变);所有背景、代码位置、根因分析在 `handoff-2026-10-01-focus-16-install-test-results.md`(同目录),从那里读起。

---

## 📁 必读文件清单(按重要性)

| 顺序 | 文件路径 | 用途 |
|---|---|---|
| 1 | `handoff-2026-10-01-focus-16-install-test-results.md` | **执行记录 + 3 问题根因分析**(本会话的核心交付) |
| 2 | `docs/superpowers/plans/2026-10-01-focus-sync-scheduled-task-migration.md` | v0.5 plan,4 轮 review 后所有修复 |
| 3 | `docs/superpowers/plans/2026-10-01-focus-sync-scheduled-task-migration-review-v4.md` | 第 4 轮 review,详情修复策略 |
| 4 | `sigma-listary-spike/src/main.rs` | spike 启动逻辑(任务 #1 加 `#![windows_subsystem]`) |
| 5 | `release/extension-installer/register.ps1` | manifest 注册逻辑(任务 #2 修改 `if ($hasExisting)`) |
| 6 | `sigma-listary-spike/src/uia_inject.rs` | sidecar writer(任务 #3 核心改动点) |
| 7 | `release/extension/dist/index.js` | extension,可能需要排查 path 来源 |

---

## 🎯 接手时的状态

### ✅ 已完成
- Phase 1(spike `--service` + `/health`)
- Phase 2(installer hooks.nsh + installer.nsi + tauri.conf.json + 2 个新 PS scripts)
- Phase 3(extension package.json + index.js 重写)
- Phase 4(升级路径 taskkill + 跨 installer 路径检测)
- Installer 构建完成并执行(`release/Sigma-File-Manager-2.2.0-focus-16-setup.exe`,17 MB, MD5 `8561cb8725817441fb370f86ed6a8dc6`)

### ❌ 待修复(用户测试发现)
| # | 问题 | 严重度 | 修复成本 |
|---|---|---|---|
| **#1** | sidecar 启动时显示 conhost 窗口(console app 没设 windows subsystem) | P0 UX | 低(加 `#![windows_subsystem = "windows"]`) |
| **#2** | Extensions tab 显示 v0.2.0 不是 v0.3.0 | P1 困惑 | 低(改 `register.ps1` 的 `if ($hasExisting)` 分支) |
| **#3** | Chrome 下载弹窗不 sync,filename 写 "C:" | P0 功能 | 中-高(需要诊断数据) |

### 📦 未 commit
所有代码改动在 working tree,**未 commit**(用户指令"测试通过再 commit")。

---

## 🔧 修复 #1 快速方案(预计 5 分钟)

**位置**:`sigma-listary-spike/src/main.rs` line 1

**当前**(无 subsystem 声明):
```rust
//! Spike binary entry point — full integration.
...
use anyhow::Result;
use clap::Parser;
```

**修复**:在 `//!` 注释块之后,**第一行 Rust 代码之前**加:
```rust
#![windows_subsystem = "windows"]
```

**为什么这是最佳方案**:
- 一劳永逸:Task Scheduler、任何启动场景都不显示 console
- 失去 stdout:但 v0.3.0 已经用 `tracing` 写 Event Log,可以保留 stderr 写到 file 或 Windows Event Log
- 不需要修改 Task Scheduler XML

**调试副作用**:debug 时看不到 stdout 了,但:
- tracing 仍写 stderr 到 console(测试时用 stderr)
- 失败信息可通过 Event Viewer 查看
- 必要时可加 tracing-appender 写 file

**然后重新 build**:
```bash
cd sigma-listary-spike
cargo build --release
powershell -File sigma-file-manager/scripts/build-with-sidecar.ps1
cd sigma-file-manager
npm run tauri:build
```

---

## 🔧 修复 #2 快速方案(预计 10 分钟)

**位置**:`release/extension-installer/register.ps1` 行 226-233 附近

**问题**:`if ($hasExisting) { preserve }` 阻止了 v0.3.0 manifest 覆盖 v0.2.0 entry

**修复策略**(部分更新,只改 version + manifest,保留其他):
```powershell
if ($hasExisting) {
    # v0.3.0 fix: 在保留其他字段的同时,显式更新 version 和 manifest 字段
    # 让 Sigma FM Extensions tab 显示正确版本
    $json.installedExtensions.'kizemo.focus-sync'.version = '0.3.0'
    $json.installedExtensions.'kizemo.focus-sync'.manifest = $kizemoManifest
    Write-Host "==> kizemo.focus-sync entry EXISTS — updated version+manifest to v0.3.0 (other fields preserved)"
} else {
    # ... existing new-install branch
}
```

**风险**:Sigma FM 在 normalize 时可能 strip `manifest` 字段。但 `version` 是 scalar field,应该保留。

**验证**:重 build 后装,看 Extensions tab 是否变成 v0.3.0。

---

## 🔧 修复 #3 深度诊断(预计 30+ 分钟)

**位置**:`sigma-listary-spike/src/uia_inject.rs` + `src/writer.rs`

**现象**:
- Chrome 下载弹窗地址栏**不变**(原本 G:\video,期望变到 Sigma FM 当前路径)
- filename box 写入了 "C:"(不是完整路径)
- Sigma FM notification 持续显示 "Sidecar not reachable"

**可能原因**(按可能性排序):

### 原因 A: sidecar 实际已死,console 是陈旧窗口
**验证**:
```bash
netstat -ano | grep 37421  # 37421 端口是否被监听?
curl http://127.0.0.1:37421/health  # sidecar 还活着吗?
Get-ScheduledTask KizemoFocusSync | Select State, LastTaskResult
Get-WinEvent -LogName "Microsoft-Windows-TaskScheduler/Operational" -MaxEvents 10
```

### 原因 B: extension 发 "C:" 路径
附图3 写 "C:" 在 filename。Sigma FM tree 切到驱动器根时,`getCurrentPath()` 可能返回 "C:"(无尾斜杠)。
**验证**:在 Sigma FM 中切到 "C:" 驱动器 → 看 extension 推送什么路径。可以加 `console.log('[focus-sync] PUSH raw=' + JSON.stringify(path))`。

### 原因 C: writer 把 path 写到 filename 而不是 address bar
v0.2.0 writer 在 Chromium Save As dialog 上:
- COM `IFileDialog::SetFolder` **失败**(Chromium 不暴露真实 IFileDialog COM)
- UIA fallback 写 filename box(可能是错误字段)
**修复方向**:uia_inject.rs 需要识别 Chromium 的地址栏控件,直接 UIA SetValue。难度大。

### 原因 D: sidecar 和 extension 在不同 session
Task Scheduler task `LogonType=Interactive` 但如果用户在多个 session(RDP),task 可能跑在不同 session。
**验证**:session_id 显示 1,但 Sigma FM 的 session_id 可能不同。

---

## 🧪 测试场景(修复后必须跑)

| TC | 场景 | 预期 |
|---|---|---|
| **#1 verify** | 重 build + 安装 | **conhost 窗口不再出现** |
| **#2 verify** | 打开 Extensions tab | **显示 v0.3.0**(不是 v0.2.0) |
| **#3 verify** | Sigma FM 切路径,开 Chrome 下载 | **filename/address bar 自动填 Sigma FM 路径** |
| **#3 alt** | Chrome 地址栏是否更新(不只是 filename) | 看实现 |
| **i18n** | Sigma FM 切换语言 zhs↔en↔zh-tw | Focus Sync 不消失, sync 持续 |

---

## 🔄 重新 build 命令(标准流程)

```bash
# 1. 改 spike source 后:
cd sigma-listary-spike && cargo build --release

# 2. 同步到 release 目录:
powershell -File sigma-file-manager/scripts/build-with-sidecar.ps1

# 3. 重新 build Sigma FM installer:
cd sigma-file-manager && npm run tauri:build

# 4. Copy 新 installer 到 release/:
cp "sigma-file-manager/src-tauri/target/release/bundle/nsis/Sigma File Manager_2.2.0_x64-setup.exe" \
   "release/Sigma-File-Manager-2.2.0-focus-16-setup.exe"
```

---

## 📋 Commit 计划(测试通过后)

按 plan §16.3 的 12 个 commit 顺序:
1. `feat(spike): add --service flag with fail-fast port (spinner mode keeps retry)` (T1.1)
2. `feat(spike): add /health endpoint with mode flag and session warn` (T1.2)
3. `chore(spike): rebuild sidecar v0.3.0`(包括 #1 的 subsystem 修复)
4. `chore(build): script to sync spike.exe → extension/bin/ (env var configurable)` (T2.6)
5. `feat(installer): register-scheduled-task.ps1 with idempotent create + path change detection` (T2.3)
6. `feat(installer): unregister-scheduled-task.ps1 with precise path filter + orphan kill` (T2.4)
7. `feat(installer-a): perMachine validate sidecar + registry path + Scheduled Task` (T2.1+T2.5)
8. `feat(installer-b): perUser deploy sidecar + File bundling + registry path + Scheduled Task` (T2.2)
9. `fix(installer): cleanup legacy spawn-mode sidecar with precise PowerShell path filter` (T4.1)
10. `feat(extension): drop binaries + shell permission + sidecar path logic, only HTTP health-check via sigma.http.request` (T3.1+T3.2+T3.6)
11. `fix(installer): register.ps1 updates existing manifest version to v0.3.0 (over-install)`(#2 修复)
12. `docs(fork): update FORK-KEEP-LIST.md with Scheduled Task migration notes` (T6.2)

---

## 📊 时间估算

| 任务 | 时间 |
|---|---|
| 修复 #1 (windows_subsystem) | 5 分钟 |
| 修复 #2 (register.ps1 partial update) | 10 分钟 |
| 诊断 #3 根因 | 30 分钟 |
| 修复 #3(根据诊断) | 30-120 分钟 |
| 重新 build + install + test | 20 分钟 |
| Commit(12 个) | 15 分钟 |
| **总计** | **~2-3 小时** |

---

## ⚠️ 重要提醒

1. **不要重新生成 plan 文档**:plan v0.5 已足够,4 轮 review 修完了。
2. **不要重启 session 后重新读所有文件**:working tree 已修改,直接用 Read 看现状即可。
3. **先修 #1**:最低风险,build option 即可,可以快速验证 build 链路。
4. **#3 可能要回到 round 19 历史**:handoff `handoff-focus-sync-retrospective-2026-10-01.md` 有 v0.2.0 在 Chrome 上的具体行为,对比 v0.3.0 看是 regression 还是从未工作过。
5. **用户对 "C:" 在 filename 很敏感**:这是 regression,需要修。

---

## ✅ 完成标志

- [ ] conhost 窗口不再出现
- [ ] Extensions tab 显示 v0.3.0
- [ ] Chrome 下载弹窗 sync 工作(地址栏 OR filename 至少一个更新到 Sigma FM 路径)
- [ ] notification "Sidecar not reachable" 不再持续显示(或者真的 reachable 时不显示)
- [ ] 12 个 commit 通过
- [ ] Tag v0.3.0
- [ ] FORK-KEEP-LIST.md 更新
- [ ] memory file 更新

