# focus-sync Scheduled Task 方案 Review(代码审计 + 风险评估)

**日期**: 2026-10-01
**作者**: Claude Code (systematic review)
**状态**: ⏳ 待应用修改到 plan,然后再实施
**被审计划**: `2026-10-01-focus-sync-scheduled-task-migration.md`
**关联文档**:
- `handoff-focus-sync-retrospective-2026-10-01.md`(根因分析)
- `2026-10-01-focus-sync-windows-service-migration.md`(已否决的 Service 方案,作为 Session 0 隔离的对比参考)

---

## 0. 总体评分

| 维度 | 评分 | 说明 |
|---|---|---|
| 架构方向 | ⭐⭐⭐⭐⭐ | Scheduled Task > Service > Registry HKCU Run,Session 1+ 跑,UIA/COM 全可用 |
| 致命 bug | **3 个,必须修复** | T3.4 自相矛盾 + -RL HIGHEST admin-only + 多用户 port 冲突 |
| 高风险 | 6 个,应该修 | notification 延迟、--service flag 依赖顺序、health 端点缺失信号、ExecutionTimeLimit、race 幂等、strict port retry |
| 中低风险 | 7 个,优化 | path fallback、phase gating、resources syntax 等 |
| 整体可行性 | 中 | 修致命 + 高风险后,可行 |

**结论**:**先修 3 个致命 + 6 个高风险,再开始实施**。直接批改会导致实施到一半再回头改 source。

---

## 二、🔴 致命问题(4 个,必须修)

### F1.1 ⭐⭐⭐⭐⭐ T3.4 lazy-create vs T3.2 删除 `shell` 权限**自相矛盾**

**位置**:`2026-10-01-focus-sync-scheduled-task-migration.md` T3.2 (line 267-274) + T3.4 (line 292-306)

**问题**:
- T3.2 显式删除 `permissions` 中的 `"shell"` —— 因为不再 spawn child
- T3.4 要求 extension 在 `activate()` 中调 `createScheduledTask()`,该函数通过 spawn PowerShell 执行 `schtasks /Create`

**矛盾证据**:
- 删除 `shell` 权限后,extension 无法调用 `sigma.shell.run*` 系列 API
- Plan T3.4 引用了 `readBundledFile` API —— 这是 plan 假想的 API,Sigma FM 实际没有,extension 读 bundled PS 脚本需要 fs.read 权限,且 spawn PS 需要 shell 权限
- 即使绕过 API 直接调 PS,sandbox 检测会扫 JS 中的危险 API 名称

**推荐修改**:
| 方案 | 改动 |
|---|---|
| **A** ⭐⭐⭐⭐⭐ | **由 installer 创建,删除 T3.4 全节内容** |
| B | 保留 `shell` 权限,放弃 T3.2 改动 |
| C | extension 写 `RunButk` registry 调 PS(同样需要 shell 权限,失败) |

**推荐 A 的具体改动**:
1. 删除 T3.1 中的 `createScheduledTask()` / `startScheduledTask()` 调用
2. 删除 T3.4 整节
3. 删除 T3.5 (PS script 在 extension)整节
4. 在 T2.1 (installer A POSTINSTALL)新增 stage 9:调 `register-scheduled-task.ps1`(走 admin context,无 sandbox 限制)
5. 在 T2.2 (installer B)新增 stage:调 `register-scheduled-task.ps1`(走 user-level schtasks,无需 admin)
6. 在 T2.3 (uninstall)调 `unregister-scheduled-task.ps1`

---

### F1.2 ⭐⭐⭐⭐⭐ T3.5 用 `-RL HIGHEST`,Standard User 装失败

**位置**:T3.5 line 313-322

**问题代码**:
```powershell
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries ... 
Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Settings $settings -Force
```
虽未显式写 `-RunLevel`,但默认是 `Highest` 等价行为需进一步确认(见 M1.4)。

更严重的版本:如果 plan T3.5 显式加 `-RunLevel Highest`,**Standard User 创建 task 失败**,因为:
- `RunLevel Highest` 需要 admin token(任务以 admin 权限跑)
- 标准用户在自己 token 下创建时不能要求 admin 跑,schtasks /Create 会返回 `ACCESS_DENIED (0x80070005)`

**修复**:
```powershell
# 方案 A: 明确 -RL LIMITED
Register-ScheduledTask ... -RunLevel Limited ...

# 方案 B: 不指定 RunLevel(默认是 Limited)
Register-ScheduledTask ...  # 不传 -RunLevel
```

**推荐 A**,理由:UIA/COM 不需要 admin,Limited 足够。

---

### F1.3 ⭐⭐⭐⭐ **多用户 port 37421 冲突**

**位置**:T1.1 line 159-160 + §13.2 line 514-515

**问题**:
> 保留 strict port(37421)+ retry bind(800ms × 3)

但 plan §13.2 仅识别风险未给修复:
> 理论只 1 个 37421,其他用户可改 ENV

**实际情况**:计划没有任何 ENV port 切换代码。"其他用户可改 ENV"是空话。

**失败场景**:
- 多人共用一台机器(RDP / Family PC / 公用 PC)
- User A 登录 → Task Scheduler 启动 User A 的 sidecar → bind 37421 ✓
- User B 也登录 → Task Scheduler 启动 User B 的 sidecar → bind **37421 fails (EADDRINUSE)**
- User B 的 sidecar **exit 1** → Task Scheduler RestartCount 启动循环
- 实际上 User A 的 sidecar 处理了所有用户的 dialog(可能因为 loopback 是 user-agnostic)
- 但 Task Scheduler 反复 restart User B 的 sidecar,产生 Windows event log spam

**推荐修复**:
1. T1.1 strict port 失败后**不 retry bind**,直接 exit 0(clean exit,Task Scheduler 不算 crash,不重启)
3. 在 `/health` 端点 + extension notification 中,当 sidecar 无法 bind 时给用户清晰提示
4. Plan §13.2 改为明确写"多用户场景仅第一个登录用户的 sidecar 工作,其余用户 sync 通过该用户的 sidecar" 或 "每个用户用单独 task,只接受 SIGUSR1"

---

### F1.4 ⭐⭐⭐⭐ T4.1 `taskkill /F /IM` 误杀风险

**位置**:T4.1 line 366-370

**问题代码**:
```nsh
nsExec::ExecToLog 'taskkill /F /IM focus-sync-sidecar.exe /T'
Pop $0
; 不管 exit code,继续
```

**误杀风险**:如果用户机器上有其他程序也叫 `focus-sync-sidecar.exe`(理论上不常见但存在),被误杀。

**修复**(选 1):

| 选项 | 实现 |
|---|---|
| **A** ⭐ | 用 PowerShell 验证进程 exe 路径后再杀:```powershell
Get-Process focus-sync-sidecar -ErrorAction SilentlyContinue \| Where-Object { $_.Path -like '*kizemo*' } \| Stop-Process -Force
``` |
| B | 加 spike instance ID(随机生成,写 registry),taskkill 时用 pid + path 双重确认 |
| C | 把 spike.exe 改名(例如 `kizemo-focus-sync-sidecar.exe`),降低误杀概率 |

**推荐 A**,简单可靠。

---

## 三、🟠 高风险问题(6 个,应该修)

### H1.1 ⭐⭐⭐⭐ T3.6 notification 触发延迟 90s 太长

**位置**:T3.6 line 327-340

**当前设计**:
> `/get_status` 连续 3 次失败(每 30s 一次 = 90s 没响应)

**问题**:
- 用户切路径后 sync 不工作,**90s 后**才看到 notification
- 用户会以为 sync 没生效,反复切换路径,体验差

**修复**:
```js
let notifiedSinceLastFailure = false;

async function checkHealth() {
  const ok = await pingSidecar();
  if (!ok && !notifiedSinceLastFailure) {
    sigma.ui.showNotification({
      title: 'Focus Sync',
      description: 'Sidecar not reachable. Open Task Scheduler → KizemoFocusSync → Run.',
      type: 'error',
      duration: 15000,
    });
    notifiedSinceLastFailure = true;
  } else if (ok) {
    notifiedSinceLastFailure = false;
  }
}

setInterval(checkHealth, 15_000);  // 改为 15s 检测频率
```

**附加**:
- 检测频率从 30s 改为 15s(平衡延迟 vs 资源)
- 首次失败立即 notification,直到下次成功才重置
- notification 文案给 actionable instructions

---

### H1.2 ⭐⭐⭐⭐ `--service` flag 没在 Plan 中明确传给 spike

**位置**:T1.1 line 153-166 + T3.5 line 318

**问题**:
- spike 现有 CLI parser(clap derive) **没有** `--service` flag
- Plan T3.5 调用 `-Argument '--service'`
- Phase 1 必须先 build,否则 PS script 调 `--service` spike 会拒绝("unexpected argument")
- 依赖关系没说

**修复**:
1. 在 Plan §5 Phase 划分加显式 phase gating:
   ```
   Phase 1 (sidecar --service + /health): MUST complete before Phase 2/3
   ```
2. T1.1 spike `--service` flag 加详细的 clap declaration:
   ```rust
   #[arg(long, hide = true)]
   service: bool,  // 启用时:关 heartbeat / 关 retry bind / 保留其他
   ```

---

### H1.3 ⭐⭐⭐ T1.2 `/health` 端点 `last_dialog_event_ts` 不反映真实健康

**位置**:T1.2 line 170-182

**当前设计**:
```json
{
  "status": "ok",
  "uptime_secs": 12345,
  "last_dialog_event_ts": 1234567890,    // 只在 DialogEvent::Opened 更新
  "version": "0.3.0",
  "mode": "service"
}
```

**问题**:
- sidecar 长时间没 dialog 时,**last_dialog_event_ts 是旧值**,`status=ok` 但 sidecar 可能 hang 或 UIA hook 失效
- 用户看 `/health` 觉得 OK,实际不可用

**推荐 /health**:
```json
{
  "status": "ok",                              // "ok" | "degraded" | "down"
  "uptime_secs": 12345,
  "last_ping_received_ts": 1234567890,         // ★ 关键:上次收到 /set_path 请求
  "last_dialog_event_ts": 1234567890,
  "active_dialogs": 0,
  "uia_healthy": true,                         // 自检 GetRootElement 成功
  "session_id": 1,                             // 当前 session ID,验证 Session 1+
  "version": "0.3.0",
  "mode": "service"
}
```

**实现要求**:
- `last_ping_received_ts`:每次 `/set_path` handler 调用时更新(AtomicU64)
- `uia_healthy`:每 30s 调一次 `IUIAutomation::GetRootElement()`,失败 3 次 → `false`
- `session_id`:调 `ProcessIdToSessionId(GetCurrentProcessId())` 一次性记录

---

### H1.4 ⭐⭐⭐ T3.5 PS script 没设 `ExecutionTimeLimit`

**位置**:T3.5 line 313-322

**问题**:
- Task Scheduler 默认 `ExecutionTimeLimit = 3 days`
- 3 天后 task **自动停止**,用户根本不会知道
- 直到下次 sync 失败才发现

**修复**:
```powershell
$settings = New-ScheduledTaskSettingsSet `
  -AllowStartIfOnBatteries `
  -DontStopIfGoingOnBatteries `
  -RestartCount 3 `
  -RestartInterval (New-TimeSpan -Minutes 1) `
  -ExecutionTimeLimit (New-TimeSpan -Seconds 0)   # ★ 0 = 永不过期

Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Settings $settings -Force | Out-Null
```

**附加建议**:在 README / handoff 里加 **"如果 sync 突然不工作,检查 Task Scheduler → KizemoFocusSync → Last Run Time"** 作为第一步 troubleshooting。

---

### H1.5 ⭐⭐⭐ T3.5 PS script `Register-ScheduledTask -Force` race

**位置**:T3.5 line 313-322

**问题**:
- `Register-ScheduledTask -Force` 已存在则**覆盖**
- 多个进程(多 Sigma FM 实例 / 重启 race)同时跑 create,可能:
  - 一个覆盖另一个 → task 配置丢失
  - 多个 Start-ScheduledTask 调用 → task 多次 start

**修复**(用 idempotent + 分离 create 与 start):
```powershell
$taskExists = Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
if (-not $taskExists) {
    $action = New-ScheduledTaskAction -Execute $SidecarPath -Argument '--service'
    $trigger = New-ScheduledTaskTrigger -AtLogOn
    $settings = New-ScheduledTaskSettingsSet `
      -AllowStartIfOnBatteries `
      -DontStopIfOnBatteries `
      -RestartCount 3 `
      -RestartInterval (New-TimeSpan -Minutes 1) `
      -ExecutionTimeLimit (New-TimeSpan -Seconds 0)
    Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Settings $settings -Description 'Kizemo Focus Sync sidecar' | Out-Null
}
# 不管新创建还是已存在,都确保启动
Start-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
```

**关键**:`-Force` 移除,改为"if exists skip create"。新版本发布只需 uninstall + reinstall,in-place upgrade 不能改 task config(可加 path 检查)。

---

### H1.6 ⭐⭐⭐ T1.1 strict port retry bind 半存活状态

**位置**:T1.1 line 161

**当前设计**:
> 保留 strict port(37421)+ retry bind(800ms × 3)

**问题**:
- v0.3.0 下,strict port 是好的(避免 silent fallback)
- 但 retry bind 800ms × 3 后仍失败则 exit → Task Scheduler retry 1 分钟后重启
- **首次启动若 37421 被占**(残留 v0.2.0 spawn、或 F1.3 多用户),task 一直失败重启

**修复**:
1. T1.1 **关掉 retry bind**,直接 fail-fast:
   ```rust
   let port = resolve_port_strict(cfg.port)?;  // 一次 bind,失败 exit
   ```
2. fail 状态返回 exit code 让 Task Scheduler 检测到失败 → RestartCount 调度
3. v0.3.0 不应承担 retry 责任,retry 是 Task Scheduler 的事

---

## 四、🟡 中低风险(7 个,优化项)

### M1.1 T3.3 path 发现 fallback 探测路径列表不完整

**位置**:T3.3 line 277-290

**当前设计**:
```js
// 1. HKLM\SOFTWARE\Sigma FM\FocusSync\SidecarPath (perMachine install)
// 2. HKCU\SOFTWARE\kizemo\focus-sync\SidecarPath (perUser install)
// 3. 探测常见路径(probing fallback)
//    - C:\Program Files\Sigma File Manager\tools\focus-sync-sidecar.exe
//    - $LOCALAPPDATA\Programs\kizemo.focus-sync\bin\focus-sync-sidecar.exe
// 4. 找不到则 throw,弹 notification
```

**问题**:fallback 只 2 个 hardcoded 路径,不全。

**修复**:补全路径数组 + 显式版本:
```js
const FALLBACK_PATHS = [
  // Installer A 路径
  `${process.env.ProgramFiles}\\Sigma File Manager\\tools\\focus-sync-sidecar.exe`,
  `${process.env.ProgramFiles}\\Sigma File Manager\\Sigma File Manager\\tools\\focus-sync-sidecar.exe`,  // 双层
  `${process.env.ProgramW6432}\\Sigma File Manager\\tools\\focus-sync-sidecar.exe`,  // 32-bit on 64-bit
  // Installer B 路径
  `${process.env.LOCALAPPDATA}\\Programs\\kizemo.focus-sync\\bin\\focus-sync-sidecar.exe`,
  // Side-by-side 用户路径(测试 / 调试)
  `${process.env.USERPROFILE}\\focus-sync-sidecar.exe`,
];
```

**附加**:notification 文案直接给 troubleshooting URL,用户能自己排查。

---

### M1.2 T3.1 activate 中 `pingSidecar` race 多次 start

**位置**:T3.1 line 256-263

**问题**:30s 检测一次,失败时重试 startScheduledTask。如果检测本身失败 + startScheduledTask 也慢,可能 race。

**修复**:single-flight pattern:
```js
let restartInFlight = false;
async function checkAndRestart() {
  const ok = await pingSidecar();
  if (ok) return;
  if (restartInFlight) return;  // 防止并发 restart
  restartInFlight = true;
  try {
    // 注: 这部分在 v0.3.1+ 可能移除,因 T1.1 fix 让 task 完全自治
    await startScheduledTask();
  } finally {
    restartInFlight = false;
  }
}
```

**附加考虑**(若 plan 改为 installer 创建 task):**完全删除 startScheduledTask**,因为 task 由 Task Scheduler 管理,不需要 extension 干预。

---

### M1.3 T1.3 build toolchain 选择

**位置**:T1.3 line 187-190

**当前设计**:
```
rustup run 1.85.0 cargo build --release
```

**问题**:
- spike 独立 crate,无 sigma-spec 依赖,不需要 1.85.0
- 但 plan 沿用 sigma-file-manager 的 1.85.0,可能有 feature 兼容问题

**修复**:明确 spike 的 MSRV:
- 检查 `sigma-listary-spike/Cargo.toml` 的 `rust-version` 字段
- 用 `cargo +stable build --release` 而不是 `rustup run 1.85.0`
- 验证 spike 不依赖 1.85+ 才有的 features

---

### M1.4 T3.5 PS script `New-ScheduledTaskAction` 默认 RunLevel 不明确

**位置**:T3.5 line 313-322

**问题**:plan 没显式指定 `-RunLevel`。PowerShell 文档:
- `New-ScheduledTaskAction` 默认行为是 **继承当前 token**(`Limited` for Standard User)
- 但 `Register-ScheduledTask` 接受 `-RunLevel`,默认也是 `Highest`

**修复**:显式指定 `-RunLevel Limited`,防止未来 PowerShell 版本变化:
```powershell
Register-ScheduledTask ... -RunLevel Limited ... | Out-Null
```

(这是 F1.2 的延伸)

---

### M1.5 T2.5 tauri.conf.json resources 改动 syntax 不明确

**位置**:T2.5 line 241-247

**当前描述**:
> 删除 `bin/focus-sync-sidecar/focus-sync-sidecar.exe` 的 APPDATA 路径声明
> 新增 `$INSTDIR\tools\focus-sync-sidecar.exe` 的资源声明

**问题**:没贴具体 syntax 改动前后对比。

**修复**:plan 加 syntax 对比:
```jsonc
// 改动前
"resources": {
  "extensions/kizemo.focus-sync/dist/index.js": "extensions/kizemo.focus-sync/dist/index.js",
  "bin/focus-sync-sidecar/0.2.0/focus-sync-sidecar.exe": "bin/focus-sync-sidecar/0.2.0/focus-sync-sidecar.exe"
}

// 改动后
"resources": {
  "extensions/kizemo.focus-sync/dist/index.js": "extensions/kizemo.focus-sync/dist/index.js",
  "tools/focus-sync-sidecar.exe": "tools/focus-sync-sidecar.exe",   // 新
  "tools/register-scheduled-task.ps1": "tools/register-scheduled-task.ps1",  // 新(installer 调用)
}
```

---

### M1.6 T5.6 验证不测 taskkill crash 场景

**位置**:T5.6 line 440-446

**当前设计**:
> [ ] `taskkill /F /IM focus-sync-sidecar.exe` → Task Scheduler 在 60s 内自动重启 → sync 恢复

但前面只测了"Stop-ScheduledTask + Start-ScheduledTask",**没测实际 crash 触发 RestartCount**。

**修复**:加测试 case:
- `[ ] taskkill /F /IM focus-sync-sidecar.exe`(模拟 crash)→ Task Scheduler 监控检测到进程 exit → RestartCount 触发 → 60s 内自动重启 → sync 恢复
- 验证 RestartCount = 3 / RestartInterval = 1 min 配置生效

---

### M1.7 T3.5 PS script cleanup bug

**位置**:T2.3 line 225-228

**当前 PS script**:
```powershell
schtasks /Delete /TN "KizemoFocusSync" /F 2>&1
# exit 0:成功 / exit 1:task 不存在(幂等 OK)
```

**问题**:
- 没捕获 status code
- `/F` 在不存在时也可能返回非零
- NSIS 调用 `Pop $0`,但 plan 没说明如何根据 `$0` 判断成功

**修复**:
```powershell
$output = schtasks /Delete /TN "KizemoFocusSync" /F 2>&1
$exitCode = $LASTEXITCODE
# exit 0: 成功删除
# exit 1: task 不存在(幂等 OK)
# 其他: 真错误
if ($exitCode -ne 0 -and $exitCode -ne 1) {
    Write-Host "WARNING: schtasks /Delete exit $exitCode"
    Write-Host $output
}
exit 0  # NSIS 不需要知道精确状态,只关心没崩
```

---

## 五、对 Plan §18 四个待确认问题的推荐

### 问题 1:sidecar binary 路径 `$INSTDIR\tools\` vs `$INSTDIR\bin\`?

**推荐**:**`tools\`**(当前选择 OK),理由:
- 与 sigma-file-manager 自带 tools 同级(已有的 `resolve_user_appdata.ps1` 等)
- 语义清晰(工具目录)
- 不与 installer B 的 perUser `bin\` 路径混淆

**确认**:保持 `$INSTDIR\tools\focus-sync-sidecar.exe`。

---

### 问题 2:Registry 命名空间 `HKLM\SOFTWARE\Sigma FM\FocusSync\` vs `HKLM\SOFTWARE\kizemo\focus-sync\`?

**推荐**:**`HKLM\SOFTWARE\kizemo\focus-sync\`(perMachine) + `HKCU\SOFTWARE\kizemo\focus-sync\`(perUser)**,理由:
- 跨 installer 命名空间一致(perMachine + perUser 都用 `kizemo\focus-sync\`)
- **不依赖 Sigma FM 命名空间**,独立产品
- perUser 路径天然是 HKCU,避免权限问题

**修正 Plan**:
- T2.1 (Installer A) 写 `HKLM\SOFTWARE\kizemo\focus-sync\SidecarPath`
- T2.2 (Installer B) 写 `HKCU\SOFTWARE\kizemo\focus-sync\SidecarPath`(perUser)
- T3.3 探测顺序:HKLM → HKCU → path fallback

⚠️ **当前 plan 不一致**:T2.1 写 `HKLM\SOFTWARE\Sigma FM\FocusSync\`,T2.2 写 `HKCU\SOFTWARE\kizemo\focus-sync\`,**需要修正**。

---

### 问题 3:Scheduled Task name?

**推荐**:**`KizemoFocusSync`**(当前选择 OK),理由:
- 独立产品,与 Sigma FM 解耦
- 未来可扩展(子任务如 `KizemoFocusSyncUpdater` / `KizemoFocusSyncHealthCheck`)

**确认**:保持 `KizemoFocusSync`。

---

### 问题 4:Health check 频率?

**推荐**:**15s**(折中,反对 30s):
- 30s + 失败 3 次 = 90s 才有 notification,用户体验差(H1.5 修复改为首次立即通知,但检测频率仍应 < 30s)
- 5-10s 太频繁(每分钟 6-12 次 HTTP request,虽然 127.0.0.1 free)
- **15s = 最多 15s 检测到 sidecar 死亡 + 立即 notification → 用户感知 < 30s**

**修改**:
- 检测频率:30s → 15s
- 失败触发:3 次累积 → 首次立即 notification(H1.1)

---

## 六、整体结构性建议

### 建议 1:删除 lazy-create,改为 installer 创建

**核心矛盾**:Plan 当前让 extension 做 installer 的事(创建 Scheduled Task)。这导致:
- 与"extension 不再 spawn"的设计矛盾
- shell permission 矛盾
- extension 不能 spawn PS
- 用户没启动 Sigma FM 时 task 不创建

**修复**:**Plan 加新 Phase 2 子任务**:installer POSTINSTALL stage 9 调 `register-scheduled-task.ps1`(走 admin / user context,无 sandbox 限制)。

**删除**:
- T3.4 (lazy-create)
- T3.5 (PS script 在 extension)
- T3.1 中的 createScheduledTask / startScheduledTask 调用
- T2.3 保留(在 installer uninstall)

---

### 建议 2:Phase 划分加显式 gating

**当前问题**:Plan §5 列了 phase 顺序,但没说"必须完成才能开始"。例如 Phase 2 / 3 同时开始,但 T3.4 lazy-create 假设 T1.1 `--service` flag 已 build。

**修复**:
```
Phase 1 (sidecar --service + /health): MUST complete + build PASS before Phase 2 / 3 start
Phase 2 (installer 改造): 可与 Phase 3 并行,但 installer A build 需 Phase 1 binary
Phase 3 (extension 改造): 可与 Phase 2 并行,但 release 也需 Phase 1 binary
```

---

### 建议 3:删除 task definition 节中关于"通过 extension 调 spawn"的描述

**位置**:Plan §3 提及 reload 触发器已证明 extension 不能 spawn PS,与 T3.4 lazy-create 矛盾。

**修复**:Plan §3 末尾加一段:
> **T3.4 lazy-create 取消。Scheduled Task 创建由 installer POSTINSTALL 阶段执行,详见 T2.6 新增任务。**

---

### 建议 4:测试矩阵分必测/应测

**当前问题**:§11 测试矩阵把"清理流程"和"7-app matrix"混在一起,没区分必测/应测。

**修复**:重排:
  - **§11.1 必测**(P0,无这些不算完成):core scenario + crash recovery
  - **§11.2 应测**(P1,争取全过):7-app matrix + 多用户
  - **§11.3 不测**(P2,占位):edge cases

---

## 七、修改优先级清单(实施顺序)

### P0(必须,实施前)

1. **F1.1 修复矛盾**(T3.4 移到 installer)
2. **F1.2 -RL Limited**(Standard User 不阻塞)
3. **F1.3 strict port fail-fast**(多用户兼容)
4. **F1.4 taskkill path filter**(防误杀)
5. **H1.2 --service flag + phase gating**(依赖关系)

### P1(应该,实施中)

6. **H1.1 notification 首次立即 + 15s 检测**
7. **H1.3 /health 端点补 `last_ping_received_ts`**
8. **H1.4 ExecutionTimeLimit = 0**
10. **H1.5 idempotent create PS script**

### P2(锦上添花)

11. **H1.6 strict port fail-fast 重构**(归在 F1.3 里)
12. **M1.1 - M1.7 优化项**

---

## 八、综述

当前 Plan 方向**正确**(Scheduled Task 优于 Service / Registry HKCU Run),但有 3 个**实施会失败**的 bug 和 6 个**会降低体验**的细节。

**建议下一步**:
1. 应用 P0(5 项)+ P1(4 项)修改到 plan,产 v0.2 版
2. 把修改后的 plan 发回给我 review 一次
3. 然后开始 Phase 1 实施

需要我现在直接产出 v0.2 修订版 plan,把所有修改合并进去吗?