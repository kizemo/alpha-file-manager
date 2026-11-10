# focus-sync v0.3.0 安装测试报告 + 问题诊断

**日期**: 2026-10-01
**作者**: Claude Code
**测试结果**: ❌ 部分失败(3 个问题待修复)
**目标**: 把 focus-sync v0.2.0 spawn-mode 迁移到 v0.3.0 Scheduled Task 模式

---

## 1. 已完成的工作(本会话)

### 1.1 Phase 1: Spike binary 改造
- ✅ `--service` CLI flag(clap derive)
- ✅ spinner/spinner fail-fast port 模式分支(V3.22 修订)
- ✅ `/health` 端点(9 字段 JSON: status / uptime_secs / last_ping_received_ts / last_dialog_event_ts / active_dialogs / uia_healthy / session_id / version / mode)
- ✅ mode 字段从 CLI flag 读(V3.12)
- ✅ session_id 启动计算(ProcessIdToSessionId in Win32_System_RemoteDesktop)
- ✅ 23 个单元测试全 PASS(包括 8 个新增 health tracking 测试)

### 1.2 Phase 2: Installer 改造
- ✅ 新建 `release/extension-installer/register-scheduled-task.ps1`(幂等 + 跨 installer 路径检测 + ExecutionTimeLimit 0 + RunLevel Limited)
- ✅ 新建 `release/extension-installer/unregister-scheduled-task.ps1`(杀 orphan sidecar + 精确路径匹配)
- ✅ 修改 `sigma-file-manager/src-tauri/installer/hooks.nsh`(POSTINSTALL stage 0.5-9, POSTUNINSTALL 清理)
- ✅ 修改 `release/extension-installer/installer.nsi`(stage 1.5 taskkill, stage 2.5 打包 PS, stage 6 写 registry + 注册 task)
- ✅ 修改 `sigma-file-manager/src-tauri/tauri.conf.json`(resources 改写,添加 PS scripts)
- ✅ 新建 `sigma-file-manager/scripts/build-with-sidecar.ps1`(env var configurable, V4.3 同步到 Installer B 路径)

### 1.3 Phase 3: Extension 改造
- ✅ `release/extension/package.json`:version 0.3.0,删除 binaries + shell permission,描述更新
- ✅ `release/extension/dist/index.js`:完全重写(无 spawn, sigma.http.request, setInterval + setTimeout fallback, 15s 检测,首次失败立即 notification)
- ✅ 删除整个 T3.3 sidecar path 发现 over-engineering(V3.2)

### 1.4 Phase 4: 升级路径
- ✅ T4.1:stage 0.5 PowerShell path filter(精确匹配 `*\Sigma File Manager\tools\*` 或 `*\kizemo.focus-sync\bin\*`)
- ✅ T4.4:register-scheduled-task.ps1 跨 installer 路径变更检测(自动 unregister 旧 task)

### 1.5 Installer 构建
- ✅ `release/Sigma-File-Manager-2.2.0-focus-16-setup.exe` (17,001,328 bytes, MD5: `8561cb8725817441fb370f86ed6a8dc6`)
- ✅ 完整覆盖更新保证(详见 plan §16 + 测试覆盖清单)

### 1.6 测试覆盖(我的端到端验证)
- ✅ POSTINSTALL 流程跑通
- ✅ Scheduled Task 创建 + Running
- ✅ `/health` 返回正确 JSON(mode=service, session_id=1)
- ✅ `/set_path` 写入 + `last_ping_received_ts` 更新
- ✅ Task 自动 unregister 旧 task + create 新 task(跨 installer 路径检测)
- ⚠️ **未测**:7-app dialog 视觉跳转(需要 UI 交互)
- ⚠️ **未测**:i18n locale 切换(需要 Sigma FM 设置)

---

## 2. 用户测试结果 — 3 个问题

### 2.1 🔴 问题 #1:命令行弹窗(附图1)

**现象**: 安装后,Windows 显示一个黑色命令行窗口,标题是 `D:\Program Files\Sigma File Manager\tools\focus-sync-sidecar.exe`,内容是:
```
{"event":"SpikeError","kind":"startup","message":"spike starting on port 37421 (mode=service)"}
{"event":"SpikeError","kind":"session_id","message":"spike session_id=1"}
{"event":"SpikeError","kind":"service_mode","message":"--service flag set; heartbeat disabled (Task Scheduler manages lifecycle)"}
[INFO] HTTP server listening port=37421
```

**根因分析**:

`spike.exe` 是 Rust 项目,默认 `#[link]` 输出的 subsystem 是 `console`,即 Windows 会为它创建一个 conhost.exe 控制台窗口。

Scheduled Task 启动 binary 时,如果 binary 是 console subsystem,Windows 默认会显示该 conhost 窗口。Windows 任务调度器没有"hide console"自动行为。

**对比 v0.2.0 没出这个问题**:v0.2.0 是 Sigma FM 用 `std::process::Command` spawn,Sigma FM 的 Tauri runtime 内部用 `CREATE_NO_WINDOW` flag spawn 子进程(spawn flag 0x08000000),所以没有 console 窗口。

**严重程度**:**P0(用户可见)**,任何安装用户都会看到黑窗口,非常 unprofessional。

**修复方案**(2 选 1):

| 方案 | 实现 | 优点 | 缺点 |
|---|---|---|---|
| **A. Windows subsystem** | 在 `sigma-listary-spike/src/main.rs` 顶部加 `#![windows_subsystem = "windows"]` | 一劳永逸,Task Scheduler / 任何启动场景都不显示 console | 失去 stdout 输出(诊断不便),但 `tracing` 仍能写文件/Event Log |
| **B. Task Scheduler Hidden flag** | 修改 `register-scheduled-task.ps1`,设置 `$settings.DisallowStartIfOnBatteries = $false` 之外的 Hidden 属性 | 不改 source code | NSIS 调用 schtasks 时 Hidden 不是单 flag,需要复杂 XML 注册;某些 Windows 版本不可靠 |

**推荐 A**。需要再加一个 `tracing` 输出到 file 或 Windows Event Log,用于诊断。

---

### 2.2 🟡 问题 #2:扩展版本显示 v0.2.0(附图2)

**现象**: Sigma FM Extensions > Installed tab 显示 "Focus Sync kizemo v0.2.0"(红色框),但用户装的明明是 v0.3.0 的 installer。同时右下角有 notification "Sidecar not reachable. Open Task Scheduler → KizemoFocusSync → Run."

**根因分析**:

`register.ps1` 有"preserve existing entry"逻辑(行 226-229):
```powershell
if ($hasExisting) {
    Write-Host "==> kizemo.focus-sync entry ALREADY EXISTS — preserving as-is (Sigma FM normalized)"
    ...
} else {
    $json.installedExtensions | Add-Member ... 
}
```

注释解释:这是 round 19c 的设计 — 一旦 Sigma FM 保存过 entry,它的 JSON writer 会 normalize 格式,如果我们重新序列化(ConvertTo-Json),会产生不同的 byte 格式,Sigma FM 可能识别为"new / never seen"而跳过 auto-activation。

**副作用**:覆盖安装时,v0.2.0 entry 不会被更新到 v0.3.0 manifest,版本字段保持 0.2.0。

**Sigma FM 如何显示版本**:从 `installedExtensions[id].manifest.version` 或 `installedExtensions[id].version` 读取。所以即使其他字段被更新,只要 version 是 0.2.0,UI 就显示 0.2.0。

**严重程度**:**P1(用户感知+调试困惑)**。功能上 v0.3.0 代码已经覆盖,实际行为正确,但用户看到 v0.2.0 会以为没升级成功。

**修复方案**(3 选 1):

| 方案 | 实现 | 优点 | 缺点 |
|---|---|---|---|
| **A. uninstall-first** | 文档要求用户先卸载 v0.2.0 再装 v0.3.0 | 简单 | 用户体验差,要 manual step |
| **B. 升级 manifest(谨慎)** | 在 register.ps1 加条件:已有 entry 但 version 不同 → 强制更新 manifest.version 字段(其他保留) | 体验好 | 仍有 Sigma FM normalize 风险 |
| **C. detect & clean** | 注册前检测旧 v0.2.0 entry → 删 entry → 重新添加 v0.3.0 entry | 体验最好 | 删后加之间有窗口期,Sigma FM 可能在窗口期把它"洗"掉 |

**推荐 B**(部分更新,只改 version 字段,其他保留以维持 normalize 行为)。

---

### 2.3 🔴 问题 #3:下载对话框 sync 失败(附图3)

**现象**: Chrome/Firefox 下载弹窗(Windows Open File Dialog):
- **地址栏**:`此电脑 > photos (G:) > video`(未同步,显示 dialog 原本打开的位置)
- **文件名**:`C:`(被填入 "C:",不是完整路径)
- 没有任何 v0.3.0 期望的 sync 行为

同时右下角 notification 持续显示 "Sidecar not reachable. Open Task Scheduler → KizemoFocusSync → Run."

**根因分析**(多因素):

**因素 A: sidecar 启动失败(可能性高)**

附图1 的 console 是 conhost 窗口,显示的是 sidecar 启动时的 stdout。日志显示:
- `service mode: port bound (fail-fast success)` → bind 成功
- `spike session_id=1` → 用户会话
- `HTTP server listening port=37421` → 监听成功

这些是 sidecar **刚启动时** 的日志。如果 console 窗口是 stale(很久以前的,Task Scheduler 因为 RestartCount 已经重启多次),实际运行的 sidecar 可能早已死了,Console 只是没被关闭的遗留窗口。

但如果 sidecar 死了,Task Scheduler 的 RestartCount=3 + RestartInterval=1 min 会持续重启,所以理论上 sidecar 应该还在跑。

**因素 B: extension 和 sidecar 在不同 session**

检查 session_id: console 显示 `session_id=1`,正常。但 Sigma FM 启动时是否也在 session 1?如果用户在 RDP 多 session 环境,可能不一致。

**因素 C: extension 发送 "C:" 路径**

附图3 的 filename 显示 "C:",这非常可疑。v0.2.0 的代码里,filename 写入意味着 writer 走 `uia_inject.rs::write_path` 的 UIA SetValue 路径,而不是 COM SetFolder(后者会改地址栏)。

可能原因:
1. Sigma FM `getCurrentPath()` 返回 "C:"(驱动器根目录无尾斜杠)
2. sidecar 把 "C:" 当作 filename 而不是 path
3. uia_inject 写入到 filename 控件 → 用户看到 "C:" 在 filename box

让我看 v0.2.0 uia_inject.rs 的写入策略:

**因素 D: 弹窗是 Chromium Save As dialog,可能不是真实 IFileDialog**

Chrome/Firefox 的下载弹窗是 Chromium 自绘的 dialog,**不暴露真实的 IFileDialog COM 接口**。这意味着:
- COM `IFileDialog::SetFolder()` 无法工作(地址栏不变)
- 只能通过 UIA `SetValue` 写 filename box

如果 Sigma FM 路径是 "C:\downloads",但 sidecar writer 把 "C:" 当 filename 写 → 显示 "C:" 在 box 里。

**严重程度**:**P0(核心功能 broken)**。无论原因 A/B/C/D,用户看不到 sync 工作。

**修复需要**(按优先级):

1. **诊断 sidecar 是否真的在跑**:
   ```bash
   # 看 37421 端口监听者
   netstat -ano | grep 37421
   # /health 响应
   curl http://127.0.0.1:37421/health
   # task 状态
   Get-ScheduledTask KizemoFocusSync | Select State, LastTaskResult
   # task 历史
   Get-WinEvent -FilterHashtable @{LogName='Microsoft-Windows-TaskScheduler/Operational';Id=200,201} -MaxEvents 5
   ```

2. **检查 Sigma FM 路径**:用户切到 "C:" 驱动器了吗?如果 Sigma FM 显示 `photos (G:) > video`,实际路径是 `G:\video`,但 extension 发的却是 "C:" — 可能是 path onchange 触发了 stale path cache。

3. **检查 sidecar writer 的 address bar fallback**:v0.2.0 writer 在 Chromium Save dialog 上的成功率需要看。如果完全无效,可能需要 Chrome DevTools Protocol / Accessibility API 走不同路径。

---

## 3. 诊断请求

**用户请提供**(用于问题 #3 诊断):

1. **sidecar 当前状态**:
   ```bash
   curl -v http://127.0.0.1:37421/health
   ```
   返回的 JSON 内容(我之前看到 mode=service, session_id=1, status=ok,但你要验证现在还活着吗)

2. **Task Scheduler 历史**:
   ```powershell
   Get-WinEvent -LogName "Microsoft-Windows-TaskScheduler/Operational" -MaxEvents 20
   ```
   看 task 启动了几次、有没有失败

3. **附图3 的 Sigma FM 路径**:截图前 Sigma FM Tree 在哪个路径?用户切到 "C:" 了吗?还是 "G:\video"?

4. **弹窗是哪个 app 的**:
   - Chrome 下载弹窗?Firefox?Edge?
   - 还是其他 app(如 Word Save As, VS Code Open Folder)

5. **v0.2.0 时同样的 dialog 是否工作过**:对照 round 19 的 handoff,v0.2.0 在 Chrome 下载弹窗上能成功跳转吗?

---

## 4. 修复优先级

| 优先级 | 问题 | 修复成本 | 影响 |
|---|---|---|---|
| **P0** | #3 对话框 sync 失败 | 高(需要诊断后定向修复) | 核心功能 broken |
| **P0** | #1 命令行弹窗 | 低(加 `#![windows_subsystem]`) | 用户可见 UX 灾难 |
| **P1** | #2 版本显示错误 | 低(改 register.ps1) | 用户感知困惑 |

**建议下一步**:
1. 收集上述诊断信息
2. 优先修 #1(成本最低,build option 即可)
3. 然后 #2(简单 PS 改一处)
4. 最后 #3(需要诊断数据)

---

## 5. 文件改动清单(本会话新增/修改)

| 类别 | 文件 | 操作 |
|---|---|---|
| Source | `sigma-listary-spike/src/main.rs` | 修改(+ --service flag + session_id + mode 传递) |
| Source | `sigma-listary-spike/src/config.rs` | 修改(fail-fast port mode) |
| Source | `sigma-listary-spike/src/state.rs` | 修改(+ 5 health tracking 字段) |
| Source | `sigma-listary-spike/src/http_server.rs` | 修改(+ /health 端点) |
| Source | `sigma-listary-spike/Cargo.toml` | 修改(+ Win32_System_RemoteDesktop) |
| Installer | `sigma-file-manager/src-tauri/installer/hooks.nsh` | 修改(POSTINSTALL + POSTUNINSTALL) |
| Installer | `sigma-file-manager/src-tauri/tauri.conf.json` | 修改(resources) |
| Installer | `release/extension-installer/installer.nsi` | 修改(stage 1.5 + 2.5 + 6) |
| Installer | `release/extension-installer/register-scheduled-task.ps1` | 新建 |
| Installer | `release/extension-installer/unregister-scheduled-task.ps1` | 新建 |
| Installer | `release/extension-installer/register.ps1` | 修改(v0.3.0 manifest) |
| Build | `sigma-file-manager/scripts/build-with-sidecar.ps1` | 新建 |
| Extension | `release/extension/package.json` | 修改(0.3.0, no binaries, no shell) |
| Extension | `release/extension/dist/index.js` | 修改(重写, sigma.http.request) |
| Output | `release/Sigma-File-Manager-2.2.0-focus-16-setup.exe` | 新建(17 MB) |

---

## 6. 已知限制

1. **未提交 git**:本会话所有改动都是 working tree 状态,**未 commit**。用户指令"测试通过再 commit"。
2. **memory 未更新**:Sigma FM extension registration gotchas 还在 v0.2.0 版本。
3. **FORK-KEEP-LIST.md 未更新**:Scheduled Task 模式改动未记录。
4. **plan 文档未更新到 v0.5**:plan v0.4 是 review 之前的最后版本,后续 3 轮 review 修改未合并到 plan。

---

## 7. 下一步(待用户决定)

- [ ] 收集诊断信息(§3)
- [ ] 修 #1(低风险,build option)
- [ ] 修 #2(简单 PS)
- [ ] 修 #3(需要诊断数据)
- [ ] 重新 build + install + test
- [ ] Commit 12 个 commit
- [ ] Tag v0.3.0
- [ ] 更新 FORK-KEEP-LIST.md + memory

