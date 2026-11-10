# focus-sync v0.3.0: 迁移到 Scheduled Task 模式(详细执行计划)

**日期**: 2026-10-01
**作者**: Claude Code (systematic-debugging)
**状态**: ⏳ 待用户确认 + 执行
**承接**: `handoff-focus-sync-retrospective-2026-10-01.md` + `2026-10-01-focus-sync-windows-service-migration.md`(已否决)
**目标**: 让 `focus-sync-sidecar.exe` 通过 Windows Scheduled Task("At logon")从 Sigma FM 进程树**彻底脱离**,在用户会话中独立运行,从根上消除 taskkill 死亡问题
**版本**:kizemo.focus-sync **0.3.0**

---

## 1. 架构定性(回答核心疑问)

### 1.1 这还是一个 Sigma FM 扩展吗?

**是的**。kizemo.focus-sync v0.3.0 仍然是:
- ✅ 一个 Sigma FM 扩展(`extensionType: "api"`,id `kizemo.focus-sync`,依赖 `sigmaFileManager >= 2.2.0`)
- ✅ kizemo/focus-sync 仓库下的一个产品
- ✅ 由 Sigma FM 扩展协议授权(permissions + activationEvents)
- ✅ 使用同一个 installer 双形态(Bundled Installer A + Standalone Installer B)
- ✅ 在 GitHub Releases 同一个仓库下分发

### 1.2 跟 Sigma FM 的关系变化

| 维度 | focus-sync v0.2.0(现状) | focus-sync v0.3.0(目标) |
|---|---|---|
| 组件数 | 2(extension + sidecar) | **不变**:还是 2 个组件 |
| sidecar 启动者 | Sigma FM 的 `start_extension_command` Tauri command → `processes.rs:start_extension_command` → `std::process::Command::new(sidecar).spawn()`(v3 V3.16 精确描述) | **Windows Task Scheduler**(用户登录触发) |
| sidecar 运行时身份 | Sigma FM 子进程 | **用户会话独立进程**(Task Scheduler 创建) |
| sidecar 跟 Sigma FM 的关系 | child process | **进程无关**(无父子关系) |
| 通信方式 | HTTP 127.0.0.1:37421 | **不变**(还是 HTTP) |
| 产品形态 | Sigma FM 扩展 | **仍是 Sigma FM 扩展** |

### 1.3 类比(成熟的同类产品)

| 产品 | 前端组件 | 后台 helper | 谁启 helper | 杀掉会怎样 |
|---|---|---|---|---|
| Docker Desktop | UI | dockerd daemon | 系统启动 | 容器不可用 |
| VS Code | Editor | Code Helper / Language Server | 用户登录 | LSP 失效 |
| 1Password | GUI | 1Password CLI | 用户登录 | CLI 失效 |
| **focus-sync v0.3.0** | Sigma FM 内的 JS | focus-sync-sidecar.exe | **用户登录(Scheduled Task)** | sync 不工作 |

---

## 2. Before/After 架构对比

### 2.1 Before(现状 — focus-15,失败)

```
Sigma FM process tree(用户会话)
└── sigma-file-manager.exe
    └── focus-sync-sidecar.exe (spawn via std::process::Command)
        └── listen 127.0.0.1:37421

触发 reload(7 类触发器,见 §3):
  worker-runtime.ts:481 disposeActivationResources
    → worker-runtime.ts:415 shellTask.cancel()
      → cancel_extension_command (processes.rs:498)
        → terminate_process_tree (processes.rs:527)
          → taskkill /F /T (processes.rs:44-65)
            → ❌ sidecar 死亡
              → ❌ PUSH connection refused
                → ❌ 4 次 retry 全失败(150/300/450ms)
```

> **v3 review V3.16 修订**:v0.2.0 sidecar 启动路径完整描述是:
> `start_extension_command` Tauri command → `processes.rs:start_extension_command` → `std::process::Command::new(sidecar).spawn()`,不是简单的 "Sigma FM 进程树"。这是第一/二轮 review 没挖到的细节。

### 2.2 After(目标 — focus-16,成功)(2026-10-01 review v2 M5 修订)

```
[进程树 A: Windows Task Scheduler](2026-10-01 review v2 M5 修订)
services.exe
  └── svchost.exe (Task Scheduler service)
       └── focus-sync-sidecar.exe (用户身份,SESSION 1+,At logon 触发)
            └── listen 127.0.0.1:37421
            └── 拥有完整用户 desktop → UIA/COM OK

[进程树 B: Sigma FM](用户会话,与 sidecar 无父子关系)
sigma-file-manager.exe
  └── extension runtime (kizemo.focus-sync extension)
       └── 启动时:仅 HTTP health check ping 127.0.0.1:37421(不 spawn,不动 Scheduled Task)
       └── Task 由 installer POSTINSTALL 阶段创建(详见 §7)

[进程树 C: 触发 reload](如切语言、卸载扩展)
sigma-file-manager.exe
  └── taskkill /F /T → 只杀 Sigma FM 子进程
       └── sidecar 在 svchost.exe 子进程树 → ✅ 不受影响
```

> **M5 修订说明**:`schtasks.exe` 只是命令行工具,**实际触发进程的是 Task Scheduler service**(跑在 svchost.exe 中)。所以 v0.3.0 进程树是 `services.exe → svchost.exe → focus-sync-sidecar.exe`。

### 2.3 关键 API 替代矩阵

| v0.2.0(现状) | v0.3.0(目标) |
|---|---|
| `sigma.shell.runWithProgress(spikePath, [...])` | **不调用** |
| `install` 时 spawn child | `install` 时只 copy binary 到 `$INSTDIR\tools\` |
| `uninstall` 时 child 自然死 | `uninstall` 时 `schtasks /Delete /TN KizemoFocusSync /F` |
| extension 启 spawn,dispose 时 cancel | **extension 不启 spawn,不动 Scheduled Task**(task 由 installer 创建) |
| `package.json.binaries[]` 段 | **移除整段**(不再 download/run) |
| `package.json.permissions` 含 `shell` | **移除 `shell`**,保留 `http`、`commands`、`toolbar`、`notifications` |

---

## 3. 7 类 reload 触发器(确认根因)

> 详见 `2026-10-01-focus-sync-windows-service-migration.md:50-66`(已验证代码)

| # | 触发器 | 文件:行 | 修复后是否仍杀 sidecar |
|---|---|---|---|
| 1 | extension disable/uninstall | `loader.ts:91-127 unloadExtensionRuntime` | ❌ 不再杀(独立进程树) |
| 2 | **i18n locale 变化** ⭐ | `extensions.ts:2091-2093 watch(i18n.global.locale)` | ❌ 不再杀 |
| 3 | binary 下载/编辑 | `extensions.vue:185-203 reloadExtensionsAfterBinaryEdit` | ❌ 不再杀 |
| 4 | install/update/refresh/delete | `extensions.ts:844, 972, 1039, 1136, 1185`(5 处) | ❌ 不再杀 |
| 5 | Sigma FM 重启 | 隐式触发 #2 + #4 | ❌ 不再杀 |
| 6 | 启动时 locale 初始化 | `extensions.ts:1593-1612 reloadExtensionsLocale` | ❌ 不再杀 |
| 7 | 主页 mount / reload window | `builtin-commands/index.ts:300-303` | ❌ 不再杀 |

**修复后全部失效,因为 sidecar 不在 Sigma FM 子进程树中**。这是 Scheduled Task 方案相对 Service 方案的核心优势——后者也免疫以上所有,但额外引入 Session 0 / UIA 失效问题。

---

## 4. 为什么是 Scheduled Task 而不是 Windows Service

> 详见用户上一轮回答(2026-10-01 会话上下文)

| 维度 | Windows Service(SYSTEM/Session 0) | Scheduled Task("At logon") |
|---|---|---|
| UIA 可用性 | ❌ Session 0 看不到用户窗口 | ✅ 用户 Session |
| IFileDialog COM | ❌ 找不到 | ✅ OK |
| SetForegroundWindow | ❌ 无 foreground session | ✅ OK |
| 多用户隔离 | 单一 instance 跨用户 | 每用户独立 task |
| 卸载语义 | `sc.exe delete` | `Unregister-ScheduledTask` |
| 失败恢复 | SCM 三次失败后停 | Task "Repeat task every 1 min if fails" |
| 复杂度 | 中(需 service wrapper) | **极低**(直接 schtasks 命令) |
| 密码存储 | 无 | 无 |

**结论**:对 use case (per-user UIA helper),Scheduled Task 语义更准确,实现更简单。

---

## 5. Phase 划分

| Phase | 内容 | 工期 | 阻塞关系 |
|---|---|---|---|
| Phase 1 | Sidecar 加 `--service` 模式 + `/health` 端点 + **fail-fast port** | 0.5 天 | —(必须先完成并 build PASS) |
| Phase 2 | 两 installer 改造(copy binary + 写 registry + **创建 Scheduled Task**) | 1.5 天 | **依赖 Phase 1 build 通过**(否则 installer 调用 `--service` flag 不存在) |
| Phase 3 | Extension 改造(移除 spawn + 健康检查 + notification;**不创建 task**) | 1.5 天 | **依赖 Phase 1 build 通过**(manifest 引用 binary) |
| Phase 4 | 升级路径(从 v0.2.0 升级到 v0.3.0) | 0.5 天 | 依赖 Phase 2 |
| Phase 5 | 验证(用户机器手动测试) | 2-3 天 | 依赖 Phase 4 |
| Phase 6 | Release & 文档 | 0.5 天 | 依赖 Phase 5 |
| **合计** | | **6.5-7.5 天** | — |

### 5.1 显式 Phase Gating(2026-10-01 review 修订)

```
Phase 1 (sidecar --service + /health): MUST complete + build PASS before Phase 2 / 3 start
Phase 2 (installer 改造): 可与 Phase 3 并行,但 installer A build 需 Phase 1 binary(否则调 --service flag 失败)
Phase 3 (extension 改造): 可与 Phase 2 并行,但 release 也需 Phase 1 binary
Phase 4 (升级路径): 必须 Phase 2 完成后才能测(因为升级时 installer 阶段 task 创建会先跑)
```

**为什么 Phase 2/3 不能先于 Phase 1**:
- T2.6 register-scheduled-task.ps1 调用 `spike.exe --service` — spike 没有 `--service` flag 会 exit 1,task 创建后立即失败
- Phase 3 extension 改完如果用户装,manifest 引用 v0.3.0 binary 但 installer 还没就绪,会出现"装了 extension 但 task 不存在"

---

## 6. Phase 1: Sidecar 改造(0.5 天)

### Task 1.1:加 `--service` flag(2026-10-01 review F1.2/H1.2/H1.6 + v3 review V3.22 修订)

- **File**: `sigma-listary-spike/src/main.rs`
- **V3.22 关键修复**:**明确 fail-fast 仅针对 `--service` 模式**,spinner(无 flag)保留 retry bind 800ms × 3(v0.2.0 backward compatibility for manual debug)
- **改动**: 接收 `--service` CLI flag(clap derive)
  ```rust
  #[arg(long, hide = true)]
  service: bool,  // 启用时:关 heartbeat / 关 auto-kill orphan / fail-fast port
  ```
- **行为变化**(V3.22 明确分支):
  ```rust
  if cli.service {
      // V3.22:fail-fast mode(Scheduled Task 部署)
      let listener = TcpListener::bind(("127.0.0.1", port))?;
      drop(listener);  // 测试 bind
      log::info!("sidecar started in service mode (fail-fast port)");
  } else {
      // V3.22:spinner mode 保留 retry bind(v0.2.0 manual debug 向后兼容)
      resolve_port_with_retry(port)?;  // 800ms × 3 retry + kill orphan
      log::info!("sidecar started in spinner mode (retry bind)");
  }
  ```
- **行为变化**:
  - 关掉 heartbeat 线程(`--service` 时,Task Scheduler RestartCount 代替)
  - 关掉 auto-kill orphan(`--service` 时,无人值守,自杀会进入 Task Scheduler 重启循环)
  - **fail-fast port**(`--service` 时,F1.3/H1.6 修订):strict 37421 bind 一次,失败立即 `exit 0` 干净退出
    - 失败时记日志:`FATAL: bind 127.0.0.1:37421 failed (EADDRINUSE); another user's sidecar is active, exiting cleanly`
  - 保留 HTTP API(`/set_path`、`/get_status`、`/health`)
  - 保留所有 UIA/COM 注入逻辑
- **验收**:
  - `spike.exe --service` 立即 listen 37421
  - `spike.exe --service` 在 37421 被占时 exit 0(不 exit 1,避免 task restart loop)
  - `spike.exe`(不带 flag,spinner 模式)仍 retry bind 800ms × 3(v0.2.0 兼容)
- **commit**: `feat(spike): add --service flag with fail-fast port (spinner mode keeps retry bind)`

### Task 1.2:加 `/health` 端点(2026-10-01 review H1.3 + v2 review M1 + v3 review V3.12 修订)

- **File**: `sigma-listary-spike/src/http_server.rs`
- **V3.12 关键修复**:`mode` 字段从 CLI flag 读取(不是 hardcode "service"),区分 `--service` vs 默认 spinner 模式
- **新增端点**:
  ```json
  GET /health → 200 OK
  {
    "status": "ok",                              // "ok" | "degraded" | "down"
    "uptime_secs": 12345,
    "last_ping_received_ts": 1700000000,         // 上次收到 /set_path 请求(AtomicU64)
    "last_dialog_event_ts": 1700000000,          // 上次 DialogEvent::Opened
    "active_dialogs": 0,
    "uia_healthy": true,                         // 每 30s 自检 GetRootElement,失败 3 次 → false
    "session_id": 1,                             // ProcessIdToSessionId
    "version": "0.3.0",
    "mode": "service"                            // V3.12 修复:"service" if cli.service else "spawn"
  }
  ```
- **实现要求**:
  - `last_ping_received_ts`:每次 `/set_path` handler 调用时更新(AtomicU64)
  - `uia_healthy`:每 30s 调一次 `IUIAutomation::GetRootElement()`,失败 3 次 → false
  - `session_id`:启动时一次性 `ProcessIdToSessionId(GetCurrentProcessId())`,**若 = 0 不 panic**(M1 修订),只 `log::error!` + `/health.status = "degraded"`
  - **V3.12**:`mode = if cli.service { "service" } else { "spawn" }`(从 CLI flag 读,spinner 调试时 mode = "spawn")
- **作用**:extension 健康检查 + 用户手动诊断 + 反映 sidecar 当前会话状态
- **commit**: `feat(spike): add /health endpoint with mode flag and session warn`

### Task 1.3:重新 build sidecar

- **命令**: `cd sigma-listary-spike && rustup run 1.85.0 cargo build --release`
- **产物**: `sigma-listary-spike/target/release/spike.exe`
- **验收**:2.5 MB PE32+ x86-64,`spike.exe --service` 立即 listen 37421,`/health` 返回 200 且 `session_id >= 1`(FAIL if session_id=0)

---

## 7. Phase 2: Installer 改造(1.5 天)

> **2026-10-01 review F1.1 重大修订**:Scheduled Task **由 installer 创建**(POSTINSTALL),不再由 extension lazy-create。extension 完全不 spawn PS,不需要 `shell` permission。

### Task 2.1:Installer A (bundled) - hooks.nsh 改动(2026-10-01 review §V-Q2 + v2 review S7 + v3 review V3.1 重大修订)

- **File**: `sigma-file-manager/src-tauri/installer/hooks.nsh`
- **V3.1 关键修订**:**删除 stage 7 self-copy**。原 stage 7 的 `CopyFiles /SILENT "$INSTDIR\tools\focus-sync-sidecar.exe" "$INSTDIR\tools\focus-sync-sidecar.exe"` 是 self-copy(NSIS CopyFiles source 从 `$PLUGINSDIR` 读,不是 `$INSTDIR`),**无效**。Tauri 2.x NSIS 模板**自动解压** `bundle.resources` 到 `$INSTDIR\`(按 target 路径),所以 `resources` 里的 `"tools/focus-sync-sidecar.exe"` 已经在 `$INSTDIR\tools\focus-sync-sidecar.exe`,**不需要 hook 手动 copy**。
- **POSTINSTALL 改动**(完整 NSIS 代码,v3 V3.1 简化版):
  ```nsh
  ; stage 7: 验证 Tauri 解压的 sidecar binary 存在(V3.1 修订:从 self-copy 改为 exists check)
  ${IfNot} ${FileExists} "$INSTDIR\tools\focus-sync-sidecar.exe"
      DetailPrint "FATAL: sidecar binary missing from installer resources"
      Abort
  ${EndIf}

  ; stage 8: write registry (统一命名空间 kizemo\focus-sync\)
  WriteRegStr HKLM "SOFTWARE\kizemo\focus-sync" "SidecarPath" "$INSTDIR\tools\focus-sync-sidecar.exe"

  ; stage 9: create scheduled task
  nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\tools\register-scheduled-task.ps1" -SidecarPath "$INSTDIR\tools\focus-sync-sidecar.exe"'
  Pop $0
  ${If} $0 != "0"
      DetailPrint "WARNING: register-scheduled-task.ps1 exit $0"
  ${EndIf}
  ```
- **关键修复**:
  - **V3.1**:stage 7 改为 `${IfNot} ${FileExists}` 验证,**不手动 copy**(Tauri 已自动解压)
  - **S7**(v2):验证失败 → `Abort`(NSIS 直接退出 installer),比原 IfErrors + Goto 更简洁
  - 命名空间统一为 `kizemo\focus-sync\`(2026-10-01 review §V-Q2)
  - stage 4/5 删除(APPDATA 路径不再需要)
- **POSTUNINSTALL 改动**:
  - 保留删 `%APPDATA%\extensions\kizemo.focus-sync\`(extension 文件)
  - 新增:删 registry key `HKLM\SOFTWARE\kizemo\focus-sync\SidecarPath`(注意命名空间)
  - 新增:调 `unregister-scheduled-task.ps1`(清理任务 + 杀 orphan sidecar 进程,详见 T2.4)
- **验收**:POSTINSTALL 日志显示 stage 7-9 成功,`Get-ScheduledTask KizemoFocusSync` 返回,registry writer 写入正确
- **commit**: `feat(installer-a): perMachine validate sidecar + registry path + create Scheduled Task`

### Task 2.2:Installer B (standalone) - installer.nsi 改动(2026-10-01 review §V-Q2 + v2 review S2/S7 修订)

- **File**: `release/extension-installer/installer.nsi`
- **改动**:
  - Stage 4:把 sidecar binary 从 `%APPDATA%\extensions\kizemo.focus-sync\bin\` 改为 `$INSTDIR\bin\focus-sync-sidecar.exe`(perUser)
  - 新增 stage:写 registry key `HKCU\SOFTWARE\kizemo\focus-sync\SidecarPath = $INSTDIR\bin\focus-sync-sidecar.exe`(命名空间统一)
  - 新增 stage:**调 `register-scheduled-task.ps1 -SidecarPath "$INSTDIR\bin\focus-sync-sidecar.exe"`**(user-level schtasks,无需 admin)
  - **S2 修复**:用 NSIS `File` 指令打包 PS scripts 到 `$INSTDIR\bin\`(不是 `!include`):
    ```nsi
    Section "Install"
        SetOutPath "$INSTDIR\bin"
        File "register-scheduled-task.ps1"
        File "unregister-scheduled-task.ps1"
        ; ... 现有 File 调用(sidecar binary 等)...
    SectionEnd

    Section "Uninstall"
        ; 卸载 PS files (NSIS 自动用 install log 反删)
        Delete "$INSTDIR\bin\register-scheduled-task.ps1"
        Delete "$INSTDIR\bin\unregister-scheduled-task.ps1"
    SectionEnd
    ```
- **S7 修复**:POSTINSTALL 顺序加 IfErrors 检查(若 sidecar binary 复制失败,跳过 task 创建)
- **Uninstaller 改动**:
  - 调 `unregister-scheduled-task.ps1`(清理当前用户的 task + 杀 orphan sidecar 进程,详见 T2.4)
  - 删 registry key
- **验收**:install 后 `reg query HKCU\SOFTWARE\kizemo\focus-sync\SidecarPath` 返回正确路径,`Get-ScheduledTask KizemoFocusSync` 返回,`$INSTDIR\bin\` 下 PS scripts 存在
- **commit**: `feat(installer-b): perUser deploy sidecar + registry path + create Scheduled Task`

### Task 2.3:新增 `register-scheduled-task.ps1`(2026-10-01 review F1.1/H1.4/H1.5 + v2 review M7 修订)

- **File**: `release/extension-installer/register-scheduled-task.ps1`(新文件)
- **功能**(幂等 + 显式 RunLevel + 无超时 + 细粒度错误处理):
  ```powershell
  param([Parameter(Mandatory)][string]$SidecarPath)
  $taskName = 'KizemoFocusSync'
  $taskExists = Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
  if (-not $taskExists) {
      $action   = New-ScheduledTaskAction -Execute $SidecarPath -Argument '--service'
      $trigger  = New-ScheduledTaskTrigger -AtLogOn
      $settings = New-ScheduledTaskSettingsSet `
          -AllowStartIfOnBatteries `
          -DontStopIfGoingOnBatteries `
          -RestartCount 3 `
          -RestartInterval (New-TimeSpan -Minutes 1) `
          -ExecutionTimeLimit (New-TimeSpan -Seconds 0)   # ★ 0 = 永不过期(H1.4 修复)
      Register-ScheduledTask `
          -TaskName $taskName `
          -Action $action `
          -Trigger $trigger `
          -Settings $settings `
          -RunLevel Limited `                            # ★ 显式 Limited(F1.2 修复,Standard User 不阻塞)
          -Description 'Kizemo Focus Sync sidecar helper' | Out-Null
  }
  # 不管新创建还是已存在,都确保启动(幂等)
  try {
      Start-ScheduledTask -TaskName $taskName -ErrorAction Stop | Out-Null
  } catch {
      # M7 修复:细化错误处理 - 0x800710D5 = "task already running"(忽略)
      if ($_.Exception.HResult -ne 0x800710D5) {
          Write-Host "WARNING: Start-ScheduledTask: $($_.Exception.Message)"
      }
  }
  ```
- **幂等**:`if exists skip create`(H1.5 修复);`Start-ScheduledTask` 已 Running 不报错(M7 细化)
- **错误处理**:任何 step 失败时 Write-Host 但 exit 0(NSIS 不需要精确状态,只关心没崩)
- **commit**: `feat(installer): register-scheduled-task.ps1 with idempotent create + unlimited execution + fine error handling`

### Task 2.4:新增 `unregister-scheduled-task.ps1`(2026-10-01 review M1.7 + v2 review S4 + v3 review V3.5/V3.10 修订)

- **File**: `release/extension-installer/unregister-scheduled-task.ps1`(新文件)
- **V3.5 关键修复**:`*kizemo*` filter 太宽。改用**精确路径匹配**(installer 部署的特定路径)
- **V3.10 优化**:顺序问题 — 先删 task 再杀 sidecar,避免杀进程时丢失 in-flight 请求
- **功能**(幂等 + 显式 exit code + **杀 orphan sidecar 进程**,S4 + V3.5 + V3.10 修订):
  ```powershell
  $taskName = 'KizemoFocusSync'

  # V3.10 优化:先删 task(task 不会再启动新 sidecar)
  $output = schtasks /Delete /TN $taskName /F 2>&1
  $exitCode = $LASTEXITCODE
  # exit 0 = 成功删除
  # exit 1 = task 不存在(幂等 OK)
  if ($exitCode -ne 0 -and $exitCode -ne 1) {
      Write-Host "WARNING: schtasks /Delete exit $exitCode"
      Write-Host $output
  }

  # wait in-flight 完成
  Start-Sleep -Seconds 1

  # S4 + V3.5 修复:kill orphan sidecar(精确路径匹配,*kizemo* 太宽改精确路径)
  Get-Process focus-sync-sidecar -ErrorAction SilentlyContinue |
    Where-Object {
        $_.Path -like '*\Sigma File Manager\tools\focus-sync-sidecar.exe' -or
        $_.Path -like '*\kizemo.focus-sync\bin\focus-sync-sidecar.exe'
    } |
    Stop-Process -Force -ErrorAction SilentlyContinue

  exit 0   # NSIS 不需要精确状态,只关心没崩
  ```
- **关键修复**:
  - **S4**:**杀 orphan sidecar** 防重装时 sync 静默失效
  - **V3.5**:精确路径(`*\Sigma File Manager\tools\*` 或 `*\kizemo.focus-sync\bin\*`),不误杀其他 kizemo-named 程序
  - **V3.10**:`Delete` → `Sleep 1` → `Stop-Process`,减少 in-flight 请求丢失
- **commit**: `feat(installer): unregister-scheduled-task.ps1 with precise path filter + orphan kill`

### Task 2.4:升级路径(Phase 4 详细,此处先占位)

- POSTINSTALL 新增 stage 9:检测并 `taskkill /F /IM focus-sync-sidecar.exe`(旧 spawn 模式残留)
- 详见 §10 Phase 4

### Task 2.5:tauri.conf.json 资源声明更新(2026-10-01 review M1.5 + v2 review S1/S5 + v3 review V3.18 重大修订)

- **File**: `sigma-file-manager/src-tauri/tauri.conf.json`
- **V3.18 关键修复**:**显式写出嵌套位置** `bundle.resources`,避免读者复制到顶层(JSON 不合法)
- **改动 syntax 对比**(S1 关键决策:sidecar binary **保留在 resources**!):
  ```jsonc
  // 修改 tauri.conf.json 的 bundle.resources 段(V3.18 修订:显式嵌套)
  {
    "bundle": {
      "resources": {
        // 改动前(已有):
        // "../../release/extension/package.json": "extensions/kizemo.focus-sync/package.json",
        // "../../release/extension/dist": "extensions/kizemo.focus-sync/dist",
        // "../../release/extension/locales": "extensions/kizemo.focus-sync/locales",
        // "../../release/extension-installer/register.ps1": "tools/register.ps1",
        // "../../release/extension-installer/resolve_user_appdata.ps1": "tools/resolve_user_appdata.ps1",

        // 改动后(v3 V3.1 修订:Tauri 自动解压 resources,不再需要 hook 手动 copy):
        "../../release/extension-installer/register-scheduled-task.ps1": "tools/register-scheduled-task.ps1",   // ★ 新
        "../../release/extension-installer/unregister-scheduled-task.ps1": "tools/unregister-scheduled-task.ps1",  // ★ 新
        "../../release/extension/bin/focus-sync-sidecar.exe": "tools/focus-sync-sidecar.exe"                      // ★ 保留(S1 修复:Tauri 自动解压到 $INSTDIR\tools\,hook stage 7 只验证存在)
        // 显式删除(v0.3.0 不再需要 APPDATA 二进制路径):
        // "../../release/extension/bin/focus-sync-sidecar-canonical.exe": "extensions/kizemo.focus-sync/bin/focus-sync-sidecar/focus-sync-sidecar.exe",  (removed)
        // "../../release/extension/bin/focus-sync-sidecar.exe": "bin/focus-sync-sidecar/0.2.0/focus-sync-sidecar.exe",  (removed — 注意:旧路径是 binary path,新路径覆盖源文件)
      },
      "windows": { ... },
      "nsis": { ... }
    }
  }
  ```
- **关键修复**(S1):**保留** `release/extension/bin/focus-sync-sidecar.exe` 作为 resources。Tauri 2.x NSIS 模板**自动解压** `bundle.resources` 到 `$INSTDIR\`(按 target 路径),所以 `tools/focus-sync-sidecar.exe` 资源项会自动落到 `$INSTDIR\tools\focus-sync-sidecar.exe`。**v3 V3.1 修订**:hook stage 7 只需 `${IfNot} ${FileExists}` 验证存在,**不需要 CopyFiles**
- **commit**: `chore(installer-a): add PS scripts + keep sidecar binary in resources (Tauri auto-extracts)`

### Task 2.6:build script 同步更新 — sidecar binary 必须从 build 产物同步(2026-10-01 review + v2 review S1 + v3 review V3.6 修订)

- **File**: `sigma-file-manager/scripts/build-with-sidecar.ps1`(新文件)
- **V3.6 关键修复**:用**环境变量** + 默认值,不 hardcode 路径(其他贡献者路径可能不同)
- **目的**:Phase 1 build 出 `sigma-listary-spike/target/release/spike.exe` 后,**复制到** `release/extension/bin/focus-sync-sidecar.exe`,让 Tauri 打包能找到
- **代码**:
  ```powershell
  # sigma-file-manager/scripts/build-with-sidecar.ps1
  param(
      [string]$RepoRoot = $env:FILEMANAGER_REPO ?? "F:\soft\00selfmade\filemanager",  # V3.6 修订:env var + default
      [string]$SpikeExe = "$RepoRoot\sigma-listary-spike\target\release\spike.exe",
      [string]$Target   = "$RepoRoot\release\extension\bin\focus-sync-sidecar.exe"
  )
  if (-not (Test-Path $SpikeExe)) {
      Write-Error "spike.exe not found at $SpikeExe. Run 'cargo build --release' first."
      exit 1
  }
  Copy-Item -Path $SpikeExe -Destination $Target -Force
  Write-Host "Copied spike.exe → $Target"
  ```
- **集成**:在 sigma-file-manager 的 build script(`npm run tauri build` 之前)调用
  - **贡献者用法**:设置 `$env:FILEMANAGER_REPO = "<your path>"`,然后跑 `pwsh scripts/build-with-sidecar.ps1`
- **commit**: `chore(build): script to sync spike.exe → extension/bin/ (env var configurable)`

---

## 8. Phase 3: Extension 改造(1.5 天 · 关键)

> **2026-10-01 review F1.1 重大修订**:extension **不再创建 Scheduled Task**。task 由 installer POSTINSTALL 阶段创建。extension 只负责:路径发现 + 健康检查 + notification。完全无 spawn 依赖。

### Task 3.1:extension `index.js` 移除 spawn 逻辑(2026-10-01 review F1.1 + v3 review V3.2 简化)

- **File**: `release/extension/dist/index.js`
- **改动**(v3 review V3.2 重大简化 — **不只移除 spawn,还删 sidecar path 相关所有代码**):
  - 删除 `activate()` 中所有 `sigma.shell.runWithProgress` 调用
  - 删除 stdout pipe、heartbeat 检查、retry spawn 逻辑
  - 删除 createScheduledTask / startScheduledTask 函数(已移到 installer)
  - **v3 简化**:删除 `getSidecarPath()` 函数(over-engineering,v0.3.0 不需要 sidecar path)
  - **v3 简化**:删除 `fileExists` / `readRegistry` / `FALLBACK_PATHS` / `sigma.fs.stat` 调用(API 不存在)
  - 改为 `activate()`:
    1. i18n 初始化(`sigma.i18n.mergeFromPath`)
    2. 读 storage enabled 状态(`sigma.storage.get`)
    3. `pingSidecar()` 健康检查 — `sigma.http.request`(T3.6,**不是 fetch**,V3.17 修订)
    4. `scheduleCheck()` 定时循环(T3.6,**setInterval + setTimeout fallback**,V3.8 修订)
- **关键**:extension **不持有 sidecar lifetime**,**不需要 sidecar path**(HTTP ping 无 path 依赖),完全无状态,只读 HTTP
- **commit**: `feat(extension): remove spawn + sidecar path logic, only HTTP health-check`

### Task 3.2:extension `package.json` 移除 binaries 段 + shell permission

- **File**: `release/extension/package.json`
- **改动**:
  - 删除整个 `"binaries": [...]` 数组(Sigma FM 不再 download/run sidecar)
  - `permissions` 段删除 `"shell"`(不再需要)
  - 保留 `http://127.0.0.1:37421` host
  - 保留 `commands` + `toolbar` + `notifications`
- **commit**: `feat(extension): drop binaries manifest + shell permission (Scheduled Task moved to installer)`

### ~~Task 3.3:sidecar 路径发现~~ — **已删除**(v3 review V3.2 重大修订)

> **v3 review V3.2 关键洞察**:**T3.3 整个是 v0.2.0 时代的过度设计产物**。
>
> - `sigma.fs.stat` API **不存在**(Sigma FM 故意不暴露 fs API 给 extension)
> - `readRegistry` API **不存在**
> - `getSidecarPath()` 是 v0.2.0 的遗留需求(v0.2.0 extension 要 spawn sidecar,需要 binary path)
> - **v0.3.0 不再 spawn,HTTP ping 不需要 sidecar path**
>
> 删除 `getSidecarPath` / `fileExists` / `readRegistry` / `FALLBACK_PATHS` / `Sigma FM fs` API 调用全部。
>
> **实施教训**:plan 升级时必须明确"哪些 v0.2.0 复杂度可以删除",而不是把所有 v0.2.0 代码平移过来。

### ~~Task 3.4:lazy-create Scheduled Task~~ — **已删除**(2026-10-01 review F1.1)

> **原因**:删除 `shell` 权限后,extension 无法 spawn PowerShell。Scheduled Task 创建由 installer POSTINSTALL 阶段执行,详见 T2.1/T2.2。

### ~~Task 3.5:新增 `create-scheduled-task.ps1`(extension bundled)~~ — **已删除**(2026-10-01 review F1.1)

> **原因**:PS script 改为 installer bundled(由 installer 调用,在 admin/user context 跑),不是 extension bundled。详见 T2.3。

### Task 3.6:`activate()` 健康检查 + 通知(2026-10-01 review H1.1/M1.2 + v3 review V3.17/V3.8 重大修订)

- **File**: `release/extension/dist/index.js`
- **V3.17 关键修复**:**不能用 `fetch()`**。Sandbox 静态扫 regex `fetch(` 直接拒绝加载 extension。**改用 `sigma.http.request`**(Sigma FM extension API,manifest 含 `http` permission 即可用)
- **V3.8 关键修复**:`setInterval` 可能被 sandbox 禁用。**fallback 用 chain of setTimeout**
- **检测频率**:**15s**(原 30s,review 修订)
- **通知触发**:**首次失败立即 notification**(原"连续 3 次"改为"首次"),直到下次成功才重置
- **代码**(V3.17/V3.8 修订版):
  ```js
  async function pingSidecar() {
      try {
          // V3.17 修复:用 sigma.http.request(不是 fetch,sandbox 拦截)
          const r = await sigma.http.request({
              url: 'http://127.0.0.1:37421/health',
              method: 'GET',
              timeout: 5000,
          });
          return r.status === 200 && r.body && JSON.parse(r.body).status !== 'down';
      } catch (e) {
          return false;
      }
  }

  let notifiedSinceLastFailure = false;
  let restartInFlight = false;

  async function checkHealth() {
      const ok = await pingSidecar();
      if (ok) {
          notifiedSinceLastFailure = false;
          return;
      }
      if (!notifiedSinceLastFailure) {
          sigma.ui.showNotification({
              title: 'Focus Sync',
              description: 'Sidecar not reachable. Open Task Scheduler → KizemoFocusSync → Run. See https://github.com/kizemo/focus-sync#troubleshooting',
              type: 'error',
              duration: 15000,
          });
          notifiedSinceLastFailure = true;
      }
  }

  // V3.8 修复:setInterval fallback → chain of setTimeout
  let intervalHandle;
  function scheduleCheck() {
      if (intervalHandle) clearInterval(intervalHandle);
      if (typeof setInterval === 'function') {
          intervalHandle = setInterval(() => {
              if (restartInFlight) return;
              restartInFlight = true;
              checkHealth().finally(() => { restartInFlight = false; });
          }, 15_000);
      } else {
          // Fallback:chain of setTimeout
          const tick = () => {
              if (restartInFlight) { intervalHandle = setTimeout(tick, 15_000); return; }
              restartInFlight = true;
              checkHealth().finally(() => {
                  restartInFlight = false;
                  intervalHandle = setTimeout(tick, 15_000);
              });
          };
          tick();
      }
  }
  scheduleCheck();
  ```
- **troubleshooting URL**:`https://github.com/kizemo/focus-sync#troubleshooting`
- **API 实际可用性清单**(v3 review 沉淀):
  - ✅ `sigma.http.request`(T3.6 使用)
  - ✅ `sigma.ui.showNotification`(T3.6 使用)
  - ✅ `sigma.storage`(T3.1 使用)
  - ✅ `sigma.i18n`(T3.1 使用)
  - ❌ `sigma.fs.stat`(**不存在** — T3.3 删除原因)
  - ❌ `fetch()`(**被 sandbox 拦截** — T3.6 修复)
- **commit**: `feat(extension): 15s health check via sigma.http.request (no fetch, sandbox safe)`

---

## 9. Phase 3 子任务依赖图(2026-10-01 review + v3 review 简化)

```
T3.1 (移除 spawn,只剩 health check)
  ├── T3.2 (manifest 清理) [并行可做]
  └── T3.6 (notification 15s + sigma.http.request + setTimeout fallback) [并行可做]

T3.3 已删除(v3 review V3.2 over-engineering 移除)
T3.4 / T3.5 已删除(2026-10-01 review F1.1)
  - task 创建由 installer 阶段执行(T2.1 / T2.2 / T2.3)
```

---

## 10. Phase 4: 升级路径(0.5 天)

### Task 4.1:升级时清理旧 spawn sidecar(2026-10-01 review F1.4 + v3 review V3.5 修订)

- **File**: `sigma-file-manager/src-tauri/installer/hooks.nsh` (Installer A) + `release/extension-installer/installer.nsi` (Installer B)
- **触发**:任何 POSTINSTALL 阶段(stage 0 后立即执行,stage 9 register task 之前)
- **V3.5 关键修复**:改用**精确路径匹配**,不依赖宽 filter `*kizemo*`
- **逻辑**:
  ```nsh
  ; stage 0.5: cleanup old spawn-mode sidecar from v0.2.0
  ; V3.5 修复:精确路径匹配(Sigma FM perMachine 或 kizemo.focus-sync perUser),避免误杀同名进程
  nsExec::ExecToLog 'powershell -NoProfile -Command "Get-Process focus-sync-sidecar -ErrorAction SilentlyContinue | Where-Object { $_.Path -like ''*\Sigma File Manager\tools\focus-sync-sidecar.exe'' -or $_.Path -like ''*\kizemo.focus-sync\bin\focus-sync-sidecar.exe'' } | Stop-Process -Force"'
  Pop $0
  ; 不管 exit code,继续(幂等)
  ```
- **理由**:v0.2.0 模式下 sidecar 由 Sigma FM spawn,卸载 v0.2.0 后用户重装 v0.3.0,旧 sidecar 可能仍在跑(占 37421 端口)。精确路径避免误杀其他 kizemo-named 程序
- **commit**: `fix(installer): cleanup legacy spawn-mode sidecar with precise PowerShell path filter`

### Task 4.2:升级时保留用户配置

- `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\dist\index.js` 会被新版覆盖
- `%APPDATA%\com.sigma-file-manager.app\user-extensions.json` 不动(Sigma FM 自身管理)
- 用户在 v0.2.0 中设置的 toggle state(enabled/disabled)状态:`user-extensions.json` 里有 entry,新版 extension 读取 OK
- **不需要迁移脚本**

### Task 4.3:升级后 installer 创建 Scheduled Task(2026-10-01 review 修订)

- v0.2.0 用户升级到 v0.3.0:
  1. installer POSTINSTALL stage 0.5:PowerShell path filter taskkill 旧 spawn-mode sidecar
  2. installer POSTINSTALL stage 7:复制 sidecar binary 到 `$INSTDIR\tools\`
  3. installer POSTINSTALL stage 8:写 registry key(`HKLM\SOFTWARE\kizemo\focus-sync\SidecarPath`)
  4. **installer POSTINSTALL stage 9:调 `register-scheduled-task.ps1` → 创建 task → `Start-ScheduledTask` 立即跑**
  5. 用户启动 Sigma FM → extension activate → 仅 HTTP health check → sync 已工作(无需重启 PC)
- **用户感知**:装完 v0.3.0 后,**无需重启 PC,无需手动启动 task**,sync 立即工作
- **commit**: (跟随 T2.1)

> **与原 plan 区别**:v0.2.0 → v0.3.0 升级时,**installer 阶段就创建并启动 task**,extension 无需 lazy-create。**这避免了 review F1.1 矛盾**(extension 不需要 spawn PS)。

### Task 4.4:跨 installer 升级路径变更(2026-10-01 review v2 M2 修订)

- **场景**:用户从 Installer B 升级到 Installer A(或反之),sidecar binary 路径变化
  - Installer B 路径:`$LOCALAPPDATA\Programs\kizemo.focus-sync\bin\focus-sync-sidecar.exe`
  - Installer A 路径:`$PROGRAMFILES\Sigma File Manager\tools\focus-sync-sidecar.exe`
- **问题**:现有 Scheduled Task 指向**旧路径**,installer 只覆盖新路径的 binary,旧 task 仍引用旧位置
- **修复**:T4.1 的 PowerShell path filter taskkill 已经处理了旧 binary 的进程。**但旧 task 不会自动删除**,需要 installer 检测后 unregister
- **检测逻辑**(T4.1 增强):
  ```powershell
  # 检测是否有 task 指向不同 sidecar 路径(路径变更时)
  $existingTask = Get-ScheduledTask -TaskName 'KizemoFocusSync' -ErrorAction SilentlyContinue
  if ($existingTask) {
      $existingAction = $existingTask.Actions[0].Execute
      if ($existingAction -ne $SidecarPath) {
          Write-Host "INFO: existing task points to $existingAction, but new path is $SidecarPath. Unregistering old task."
          schtasks /Delete /TN 'KizemoFocusSync' /F | Out-Null
      }
  }
  ```
- **加到 register-scheduled-task.ps1 顶部**(在 create task 之前)
- **commit**: (跟随 T2.3)

---

## 11. Phase 5: 验证(2-3 天 · 用户机器手动)

### Task 5.1:全新装机测试(Installer A)

- [ ] 卸载 focus-15(如有)
- [ ] 装 focus-16 installer A
- [ ] 检查 `$INSTDIR\tools\focus-sync-sidecar.exe` 存在
- [ ] 检查 `HKLM\SOFTWARE\kizemo\focus-sync\SidecarPath` 正确
- [ ] **installer POSTINSTALL 调 `register-scheduled-task.ps1` 后,`Get-ScheduledTask KizemoFocusSync` 已存在**(无需 Sigma FM 启动)
- [ ] 启动 Sigma FM → extension 仅 HTTP health check → sync 工作
- [ ] Edge Save As → Sigma FM 切路径 → dialog 跳转 ✓
- [ ] **关键 TC-1**:切 Sigma FM 语言 zhs → en → zh-tw,看 sync 是否仍工作 ✓
- [ ] 重启 Sigma FM 5 次,每次切路径,看 sync 是否仍工作 ✓

### Task 5.2:全新装机测试(Installer B)

- [ ] 在已有 Sigma FM 的机器装 Installer B
- [ ] 检查 `%LOCALAPPDATA%\Programs\kizemo.focus-sync\bin\focus-sync-sidecar.exe` 存在
- [ ] 检查 `HKCU\SOFTWARE\kizemo\focus-sync\SidecarPath` 正确
- [ ] 同 Task 5.1 验证流程

### Task 5.3:升级路径测试(从 focus-15)

- [ ] 装 focus-15(模拟老用户)
- [ ] 启动 Sigma FM,确认 sync 工作
- [ ] 装 focus-16 installer A(覆盖升级)
- [ ] 检查:旧 sidecar 进程被 taskkill(PowerShell path filter,F1.4)✓
- [ ] installer POSTINSTALL 调 register-scheduled-task.ps1 → `Get-ScheduledTask KizemoFocusSync` 已存在 ✓
- [ ] 启动 Sigma FM → sync 立即工作(无需重启 PC)✓

### Task 5.4:核心验证 — Sigma FM 死亡不影响 sidecar(2026-10-01 review v2 M6 修订)

- [ ] Sigma FM 在跑,sidecar service 在跑,`Get-ScheduledTask KizemoFocusSync` 显示 Running
- [ ] **杀 Sigma FM**:`taskkill /F /IM sigma-file-manager.exe /T`(M6 修订:加 `/T` 杀进程树)
- [ ] `Get-ScheduledTask KizemoFocusSync` 仍 Running ✓
- [ ] 重启 Sigma FM → extension re-activate → pingSidecar 成功 → sync 工作 ✓

### Task 5.5:7-app dialog 矩阵

| ID | App | 场景 | 预期 |
|---|---|---|---|
| TC-01 | Edge | Save As | ✓ dialog 跳转 |
| TC-02 | Chrome | Download | ✓ dialog 跳转 |
| TC-03 | Word | 另存为 | ✓ dialog 跳转 |
| TC-04 | VS Code | Open Folder | ✓ dialog 跳转 |
| TC-05 | 钉钉 | 发送文件 | ✓ dialog 跳转 |
| TC-06 | 飞书 | 上传 | ✓ dialog 跳转 |
| TC-07 | 微信 | 发送文件 | ✓ dialog 跳转 |

### Task 5.6:sidecar 失败恢复验证(2026-10-01 review M1.6 + v3 review V3.20 修订)

- [ ] sync 工作时 `Stop-ScheduledTask KizemoFocusSync`
- [ ] extension 15s 内检测失败 → 首次失败立即弹 notification(不累积 3 次)
- [ ] **V3.20 修订**:`Stop-ScheduledTask` 不杀 sidecar 进程,`Start-ScheduledTask` 也不会新建(task Scheduler 不 kill 现存进程)。所以测试需要加 taskkill:
  ```bash
  # 1. Stop-ScheduledTask KizemoFocusSync(停止 task)
  # 2. taskkill /F /IM focus-sync-sidecar.exe(强制杀进程,模拟真实 crash)
  # 3. Start-ScheduledTask KizemoFocusSync(触发 task 启动新 sidecar)
  # 4. 检查新 sidecar 跑起来(sigma.http.request /health 返回 200)
  ```
- [ ] extension 15s 内检测成功 → notification 不再触发
- [ ] **★ crash recovery**:`taskkill /F /IM focus-sync-sidecar.exe`(M1.6 补充)→ Task Scheduler 监控检测到进程 exit → RestartCount=3 / RestartInterval=1min 触发 → 60s 内自动重启 → extension 检测成功 → sync 恢复

### Task 5.7:卸载流程验证(2026-10-01 review v2 S6 修订)

- [ ] 卸载 Installer A(通过 Add/Remove Programs)
- [ ] registry key `HKLM\SOFTWARE\kizemo\focus-sync\SidecarPath` 被删(S6 修订:命名空间统一)
- [ ] `Get-ScheduledTask KizemoFocusSync` 返回 not found(extension 已清理)
- [ ] **★ 验证 orphan sidecar 已被 taskkill**(S4 修复):卸载前先 `Get-Process focus-sync-sidecar` 应有 PID,卸载后再 `Get-Process focus-sync-sidecar` 应为空
- [ ] `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\` 被删
- [ ] 卸载 Installer B 同样验证(`HKCU\SOFTWARE\kizemo\focus-sync\SidecarPath` 被删)

### Task 5.8:多用户场景(高级)

- [ ] User A 装 + 启动 → User A 的 task 创建 + sidecar 跑
- [ ] 切换到 User B(无 focus-sync 安装)→ User B 看不到 task(预期)
- [ ] 切回 User A → User A 的 task 仍 Running(预期)

---

## 12. Phase 6: Release & 文档(0.5 天)

### Task 6.1:打 tag v0.3.0

- 在 `kizemo/focus-sync` repo 打 v0.3.0
- release body 列出:
  - 架构变更(Scheduled Task 模式)
  - 升级路径
  - 装机说明
  - 验证证据
- assets:
  - 新的 sidecar binary(spike.exe v0.3.0)
  - 完整 installer 包

### Task 6.2:更新 `docs/FORK-KEEP-LIST.md`

- 新增章节"Scheduled Task 化"
- 记录:
  - installer 改动(POSTINSTALL stage 7-8)
  - extension 入口改造
  - 移除 manifest binaries 段

### Task 6.3:更新 handoff

- 写 `handoff-2026-10-XX-scheduled-task-migration.md`
- 列出 commit 链 + SHA + 测试证据 + 用户验收步骤

### Task 6.4:更新 memory

- 更新 `E:\Users\Duanyi\.claude\projects\F--soft-00selfmade-filemanager\memory\sigma_fm_extension_registration.md`:
  - 新增 gotcha:sidecar 路径走 registry(perMachine + perUser 双形态,统一 `kizemo\focus-sync\`)
  - 新增 gotcha:Scheduled Task **由 installer 创建**(不是 extension lazy-create,2026-10-01 review 修订)
  - 新增 gotcha:PS script 必须显式 `-RunLevel Limited`(默认 Highest 会阻塞 Standard User)
  - 新增 gotcha:`-ExecutionTimeLimit (New-TimeSpan -Seconds 0)` 必须显式设,否则 3 天后 task 自停
  - 新增 gotcha:升级时 `taskkill` 必须 PowerShell path filter,避免误杀同名进程

---

## 13. 风险与缓解

### 13.1 高风险(2026-10-01 review 修订)

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| ~~extension 在 activate 时无权限调 `Register-ScheduledTask`~~(F1.1 已删除) | — | — | **已删除**:extension 不再创建 task,由 installer 阶段调 |
| Antivirus 把 spike.exe 标记 | 中 | 高 | 用户加白名单,README 写清(spike 不签名) |
| Scheduled Task 在用户重启后不自动跑 | 低 | 高 | 测试:"At logon" trigger 默认行为就是登入后跑 |
| Sigma FM 启动时任务延迟,sidecar 还没 listen | 中 | 中 | installer POSTINSTALL 调 `Start-ScheduledTask` 立即跑(T2.3),不等登录 |

### 13.2 中风险

| 风险 | 概率 | 影响 | 缓解(2026-10-01 review + v3 review 修订) |
|---|---|---|---|
| **多用户机器 port 37421 冲突** ⭐(F1.3) | 高 | 中 | **T1.1 fail-fast port**(仅 `--service` 模式,spinner 保留 retry bind,V3.22 修订):bind 37421 失败立即 `exit 0`,不触发 Task Scheduler RestartCount。后果:**只第一个登录用户的 sidecar 工作,后续用户无 sidecar 但 127.0.0.1 仍可达(User A 的 sidecar 服务所有用户)**,无 Event Log spam。文档说明限制 |
| **PowerShell 5.1 兼容性** ⭐(v3 V3.21 修订,加版本说明) | 低 | 中 | **测试环境 PowerShell 5.1**(Windows 11 默认自带)。`Register-ScheduledTask`、`Get-ScheduledTask`、`New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Seconds 0)` 在 5.1 上 OK(0 是合法参数)。T2.3/T2.4 PS scripts 不依赖 PS 6+ 特性 |
| 升级时旧 sidecar 残留占端口 | 中 | 中 | T4.1 taskkill 旧 sidecar(用 PowerShell **精确** path filter,V3.5 修订) |
| Task Scheduler 被企业管控禁用 | 低 | 高 | 用户用 Admin 启用;或 fallback 到 spawn 模式(plan 没实现,需要时再加) |
| **Standard User 装 Installer B 时 -RL 默认 Highest 失败** ⭐(F1.2) | 高 | 高 | **T2.3 显式 `-RunLevel Limited`**:Standard User 在自己 token 下创建 task 不需要 admin,`Limited` 足够(UIA/COM 走 user session 仍可访问用户窗口) |

### 13.3 低风险(2026-10-01 review v2 M4 修订)

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| 注册表被安全软件清理 | 极低 | 低 | 重新装机即可 |
| ~~`Activate` 阶段 PS 执行慢~~(M4 删除:已过期) | — | — | extension 不再 spawn PS,该风险不存在 |
| installer 阶段 PS 冷启动 | 低 | 低 | 仅影响 POSTINSTALL 1-2s,不影响 sync UX |
| bundle resources 路径写错(perMachine) | 低 | 中 | installer A 的 tauri.conf.json 写 `$INSTDIR\tools\` 路径正确 |
| 跨 installer 切换导致 task 路径错位(M2) | 低 | 中 | T4.4 检测到现有 task 指向不同路径时自动 unregister |
| sidecar binary 在 build pipeline 同步失败(S1) | 低 | 高 | T2.6 build script 显式 Copy-Item,缺文件立即 fail |
| Worker sandbox 不暴露 `process.env`(S3) | 中 | 中 | T3.3 已移除 `process.env`,改 hardcoded + existsSync |

---

## 14. 验收标准(2026-10-01 review 分层修订)

### 14.1 必须满足(P0,无这些不算完成)(v3 review V3.13 修订)

- [ ] **TC-CRITICAL-1**:Sigma FM 重启 5 次,sync 持续工作
- [ ] ~~**TC-CRITICAL-2**:Sigma FM 完全卸载后,sidecar 仍 RUNNING~~ — **v3 review V3.13 删除**:这是无意义测试。focus-sync 不能脱离 Sigma FM 独立存在,卸载后 sidecar 应该被清理(被 TC-17 覆盖)
- [ ] **TC-CRITICAL-3**:Sigma FM 切语言(zhs ↔ en ↔ zh-tw)多次,sync 仍工作(原 reload 杀手场景)— **修复 7 类 reload 触发器** — **真实核心价值**
- [ ] **TC-CRITICAL-4**:sidecar crash 后(taskkill /F),Task Scheduler 在 60s 内自动重启(RestartCount 验证)— **M1.6 + V3.20 修订**
- [ ] **TC-CRITICAL-5**:升级路径(focus-15 → focus-16)无残留 sidecar 进程(用 path filter 验证)
- [ ] **TC-CRITICAL-6**:7-app dialog 矩阵 ≥ 5 个视觉跳转成功
- [ ] **TC-CRITICAL-7**:卸载 Installer 后,Scheduled Task 被清理(POSTUNINSTALL 调 unregister-scheduled-task.ps1)
- [ ] **TC-CRITICAL-8**(F1.2):Standard User(非 admin)装 Installer B,Scheduled Task 创建成功(`-RunLevel Limited`)

### 14.2 应满足(P1,争取全过)

- [ ] sidecar 启动延迟 < 3 秒(Task Scheduler At logon)
- [ ] sidecar 内存占用 < 30 MB
- [ ] extension 启动时间 < 500ms
- [ ] network profile 切换不影响 127.0.0.1 binding
- [ ] TC-12 修订:**sidecar 健康检查每 15s 一次,首次失败立即 notification**(不累积)
- [ ] TC-13:extension 启动后 1s 内 ping 37421 成功

### 14.3 不测(P2,占位 edge cases)

- [ ] 多用户同时登录场景下,非首个登录用户的 sidecar fail-fast exit 0(不污染 Event Log)— **F1.3 限制**:只有首个用户的 sidecar 工作
- [ ] antivirus 标记 spike.exe(不签名,见 §13.1)— 用户加白名单

### 14.4 测试矩阵汇总

| ID | 场景 | 预期 | 必/应 | 备注 |
|---|---|---|---|---|
| TC-01 | clean install → 启动 Sigma FM → sync 工作 | ✓ | 必 | |
| TC-02 | sync 工作 → 切语言 zhs→en → sync 仍工作 | ✓ | 必 | reload 杀手场景 |
| TC-03 | sync 工作 → 重启 Sigma FM 5 次 → 每次 sync 工作 | ✓ | 必 | |
| TC-04 | sync 工作 → 杀 Sigma FM → 重启 → sync 工作 | ✓ | 必 | |
| TC-05 | sync 工作 → 杀 spike.exe (taskkill /F) → 60s 内自动重启 → sync 工作 | ✓ | 必 | M1.6 验证 RestartCount |
| TC-06 | 卸载 Installer → Scheduled Task 不存在 | ✓ | 必 | |
| TC-07 | Edge Save As → Sigma FM tree 切 → dialog 跳转 | ✓ | 必 | |
| TC-08 | Chrome download → Sigma FM tree 切 → dialog 跳转 | ✓ | 必 | |
| TC-09 | Word 另存为 → Sigma FM tree 切 → dialog 跳转 | ✓ | 必 | |
| TC-10 | VS Code Open Folder → Sigma FM tree 切 → dialog 跳转 | ✓ | 必 | |
| TC-11 | 7-app matrix ≥ 5 个 dialog 跳转 | ✓ | 必 | |
| TC-12 | sidecar 健康检查每 15s 一次,**首次失败立即** notification | ✓ | 应 | H1.1 修订 |
| TC-13 | extension 启动后 1s 内 ping 37421 成功 | ✓ | 应 | |
| TC-14 | 升级路径无端口冲突 | ✓ | 必 | |
| TC-15 | Standard User 装 Installer B → task 创建成功 | ✓ | 必 | F1.2 验证 RL Limited |
| TC-16 | `/health` 返回 `session_id >= 1`(非 Session 0) | ✓ | 必 | H1.3 验证 |
| TC-17 | v2 S4:卸载后无 orphan sidecar 残留 | ✓ | 必 | S4 验证 `Get-Process focus-sync-sidecar` 为空 |
| TC-18 | v2 S2:Installer B 安装后 `$INSTDIR\bin\register-scheduled-task.ps1` 存在 | ✓ | 必 | S2 验证 NSIS `File` 打包 |
| TC-19 | v2 S7:sidecar copy 失败时,register task 跳过(无失败 task) | ✓ | 应 | S7 验证 IfErrors 守卫 |
| TC-20 | v2 M2:跨 installer 切换(Installer B → A)后,旧 task 自动 unregister | ✓ | 应 | M2 验证路径变更检测 |

---

## 15. 时间线

| 阶段 | 工作量 | 累计 | 备注 |
|---|---|---|---|
| Phase 1: sidecar --service + /health | 0.5 天 | 0.5 天 | 可立即开始 |
| Phase 2: installer 改造(A+B) | 1.5 天 | 2 天 | 依赖 Phase 1 |
| Phase 3: extension 改造(health-check only,task 在 installer 创建) | 1.0 天 | 3 天 | 依赖 Phase 1 |
| Phase 4: 升级路径 | 0.5 天 | 4 天 | 依赖 Phase 2 |
| Phase 5: 用户机器手动验证 | 2-3 天 | 6-7 天 | **不可压缩** |
| Phase 6: Release & 文档 | 0.5 天 | 6.5-7.5 天 | |
| **合计** | | **6.5-7.5 天** | |

实际可能更长,**因为用户机器手动验证无法压缩**。

---

## 16. 文件改动清单(2026-10-01 review + v2 review 修订)

### 16.1 修改文件

```
F:\soft\00selfmade\filemanager\
├── sigma-listary-spike\
│   ├── src\
│   │   ├── main.rs                        [Phase 1.1: --service flag + fail-fast port]
│   │   └── http_server.rs                 [Phase 1.2: /health 端点扩展字段,session_id=0 warn-only]
│   └── target\release\spike.exe           [Phase 1.3: build 产物]
├── sigma-file-manager\
│   ├── scripts\build-with-sidecar.ps1     [Phase 2.6: NEW - sync spike.exe → extension/bin/]
│   └── src-tauri\
│       ├── installer\hooks.nsh            [Phase 2.1: perMachine deploy + IfErrors 守卫 + task 注册]
│       └── tauri.conf.json                [Phase 2.5: resources 加 PS scripts,保留 sidecar binary]
└── release\
    ├── extension\
    │   ├── package.json                   [Phase 3.2: 移除 binaries + shell permission]
    │   └── dist\index.js                  [Phase 3.1/3.3/3.6: 移除 spawn + health check + hardcoded fallback]
    └── extension-installer\
        ├── installer.nsi                  [Phase 2.2: perUser deploy + File 打包 PS + task 注册]
        └── build.ps1                      [Phase 2.6: makensis 引用新 PS 文件]
```

### 16.2 新增文件

```
F:\soft\00selfmade\filemanager\
├── sigma-file-manager\scripts\
│   └── build-with-sidecar.ps1             [Phase 2.6: 同步 spike.exe 到 Tauri 打包路径]
└── release\extension-installer\
    ├── register-scheduled-task.ps1        [Phase 2.3: 创建 task + 跨 installer 路径检测]
    └── unregister-scheduled-task.ps1      [Phase 2.4: 杀 orphan sidecar + 删 task]
```

### 16.3 Commit 链(预计 13 个 commit,v3 review 修订)

1. `feat(spike): add --service flag with fail-fast port (spinner mode keeps retry)` (T1.1 + V3.22)
2. `feat(spike): add /health endpoint with mode flag and session warn` (T1.2 + V3.12)
3. `test(spike): unit tests for /health endpoint (mock session_id, mode, uia_healthy)` (V3.7 修订)
4. `chore(spike): rebuild sidecar v0.3.0` (T1.3)
5. `chore(build): script to sync spike.exe → extension/bin/ (env var configurable)` (T2.6 + V3.6)
6. `feat(installer): register-scheduled-task.ps1 with idempotent create + path change detection` (T2.3 + M2 + M7)
7. `feat(installer): unregister-scheduled-task.ps1 with precise path filter + orphan kill` (T2.4 + S4 + V3.5 + V3.10)
8. `feat(installer-a): perMachine validate sidecar + registry path + Scheduled Task` (T2.1+T2.5+V3.1+S7)
9. `feat(installer-b): perUser deploy sidecar + File bundling + registry path + Scheduled Task` (T2.2+S2)
10. `fix(installer): cleanup legacy spawn-mode sidecar with precise PowerShell path filter` (T4.1 + V3.5)
11. `feat(extension): drop binaries + shell permission + sidecar path logic, only HTTP health-check via sigma.http.request` (T3.1+T3.2+V3.2+T3.6+V3.17+V3.8)
12. `test(extension): unit tests for pingSidecar fallback + setInterval/setTimeout fallback` (V3.7 修订)
13. `docs(fork): update FORK-KEEP-LIST.md with Scheduled Task migration notes` (T6.2)

### 16.4 必读的前置文档

- `handoff-focus-sync-retrospective-2026-10-01.md`(本次根因分析)
- `docs/superpowers/plans/2026-10-01-focus-sync-windows-service-migration.md`(已否决方案,作为对比)
- `docs/superpowers/plans/2026-09-24-sigma-fork-and-plugin.md`(fork 工作流总览)
- `docs/superpowers/specs/2026-09-27-listary-focus-sync-design.md`(spec 上下文)
- `docs/superpowers/decisions/2026-09-27-listary-focus-sync-decision.md`(PASS 决策)
- `docs/FORK-KEEP-LIST.md`(同步策略)
- `docs/superpowers/plans/2026-10-01-focus-sync-scheduled-task-migration-review.md`(第一轮 review)
- `docs/superpowers/plans/2026-10-01-focus-sync-scheduled-task-migration-review-v2.md`(第二轮 review)

### 16.5 必须保留不变的

- `sigma-listary-spike/src/uia_inject.rs` 版权头(MIT, PathWrap)
- `sigma-listary-spike/src/uia_event.rs` 版权头(MIT, PathWrap)
- 任何已 commit 的 spike 移植文件版权头

---

## 17. 回滚方案

### 17.1 如果 Scheduled Task 模式失败(v3 review V3.15 修订)

- Installer POSTINSTALL 失败 → sidecar 没装 → 用户回退到 focus-15 installer
- 用户操作:
  ```powershell
  # 手动清理(V3.15 修订:命名空间统一为 kizemo\focus-sync\)
  Unregister-ScheduledTask -TaskName KizemoFocusSync -Confirm:$false -ErrorAction SilentlyContinue
  Get-Process focus-sync-sidecar -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -like '*\Sigma File Manager\tools\focus-sync-sidecar.exe' -or
                   $_.Path -like '*\kizemo.focus-sync\bin\focus-sync-sidecar.exe' } |
    Stop-Process -Force -ErrorAction SilentlyContinue
  Remove-Item -Path "HKLM:\SOFTWARE\kizemo\focus-sync" -Recurse -Force -ErrorAction SilentlyContinue  # V3.15 修订
  Remove-Item -Path "HKCU:\SOFTWARE\kizemo\focus-sync" -Recurse -Force -ErrorAction SilentlyContinue  # perUser 兜底
  # 重装 focus-15
  ```

### 17.2 如果 sync 工作不稳定

- 回退到 focus-15 installer
- Scheduled Task 不影响旧 installer 的 spawn 路径(完全独立)

### 17.3 如果 commit 链有问题

- 整个 PR 没合,只针对本地
- 用户继续用 focus-15 + kizemo/focus-sync v0.2.0 release

---

## 18. 待确认问题(2026-10-01 review 已更新答案)

| # | 问题 | 推荐答案 | 状态 |
|---|---|---|---|
| 1 | sidecar binary perMachine 路径 | **`$INSTDIR\tools\focus-sync-sidecar.exe`** | ✅ 确认 |
| 2 | registry key 命名空间(perMachine + perUser 统一) | **`HKLM\SOFTWARE\kizemo\focus-sync\`** + **`HKCU\SOFTWARE\kizemo\focus-sync\`**(2026-10-01 review §V-Q2 修正) | ✅ 确认 |
| 3 | Scheduled Task name | **`KizemoFocusSync`** | ✅ 确认 |
| 4 | extension health check 频率 | **15s**(2026-10-01 review §V-Q4 修订,原 30s) | ✅ 确认 |
| 5 | sidecar RunLevel | **`Limited`**(2026-10-01 review F1.2 修订) | ✅ 确认 |
| 6 | port 37421 多用户冲突策略 | **fail-fast exit 0**(2026-10-01 review F1.3 修订,文档说明限制) | ✅ 确认 |
| 7 | task kill path filter | **精确路径匹配**(2026-10-01 review F1.4 + v3 review V3.5 修订,不用宽 `*kizemo*`) | ✅ 确认 |
| 8 | task 创建位置 | **installer POSTINSTALL**(2026-10-01 review F1.1 重大修订,extension 不 spawn) | ✅ 确认 |
| 9 | ExecutionTimeLimit | **`New-TimeSpan -Seconds 0`** = 永不过期(2026-10-01 review H1.4 修订) | ✅ 确认 |
| 10 | PS script `Register-ScheduledTask` 幂等 | **`if exists skip create`**(2026-10-01 review H1.5 修订,不用 `-Force`) | ✅ 确认 |
| 11 | `/health` v0.3.0 新增字段 | **`last_ping_received_ts` + `uia_healthy` + `session_id` + `mode`**(2026-10-01 review H1.3 + v3 review V3.12 修订) | ✅ 确认 |
| 12 | notification 触发延迟 | **首次失败立即,15s 检测周期**(2026-10-01 review H1.1 + v3 review V3.8 setInterval fallback 修订) | ✅ 确认 |
| 13 | extension HTTP API | **`sigma.http.request`**(v3 review V3.17 修订,**不用 `fetch`** 被 sandbox 拦截) | ✅ 确认 |
| 14 | extension sidecar path | **不需要**(v3 review V3.2 重大修订,删除整个 T3.3 over-engineering) | ✅ 确认 |
| 15 | PS 测试环境 | **PowerShell 5.1**(v3 review V3.21 修订,Windows 11 默认) | ✅ 确认 |
| 16 | /health mode 字段来源 | **从 CLI flag 读**(v3 review V3.12 修订,`--service` → "service",spinner → "spawn") | ✅ 确认 |
| 17 | stage 7 self-copy | **改为 exists check**(v3 review V3.1 修订,Tauri 2.x NSIS 自动解压 resources) | ✅ 确认 |
| 18 | `--service` fail-fast 范围 | **仅 `--service` 模式,spinner 保留 retry bind**(v3 review V3.22 修订,保持 v0.2.0 backward compat) | ✅ 确认 |

**所有问题已确认,可以开始 Phase 1 实施**。

---

## 19. 修改历史

| 日期 | 版本 | 修改人 | 摘要 |
|---|---|---|---|
| 2026-10-01 | v0.1 | Claude Code | 初版,基于 Windows Service 方案 |
| 2026-10-01 | v0.2 | Claude Code | **重大重构**:改用 Scheduled Task 方案(避免 Session 0 UIA 失效) |
| 2026-10-01 | v0.3 | Claude Code | **第一轮 review 应用**:修复 3 致命 + 6 高风险 + 7 中低风险 |
| 2026-10-01 | v0.4 | Claude Code | **第二轮 review 应用**:修复 7 P0 + 3 P1 + 4 P2(build pipeline / NSIS 语法 / sandbox 限制 / 一致性) |
| 2026-10-01 | **v0.5** | Claude Code | **第三轮 review 应用 — API 真实性深度审查**:修复 3 P0 + 2 P1 + 6 P2 + 4 P3:<br>**P0** V3.17 `fetch` 改 `sigma.http.request`(sandbox 拦截)/ V3.2 **删除整个 T3.3 over-engineering**(`sigma.fs.stat`/`readRegistry` 不存在,sidecar path 不需要)/ V3.1 stage 7 self-copy 改为 `${IfNot} ${FileExists}`(Tauri 自动解压)<br>**P1** V3.18 T2.5 syntax 显式 `bundle.resources` 嵌套 / V3.5 `*kizemo*` filter 改精确路径匹配<br>**P2** V3.8 setInterval fallback → chain setTimeout / V3.13 删除 TC-CRITICAL-2(无意义测试) / V3.15 §17.1 rollback 路径修正 / V3.20 T5.6 加 taskkill 模拟 crash / V3.12 mode 字段从 CLI flag 读 / V3.21 §13.2 加 PS 5.1 环境说明<br>**P3** V3.6 T2.6 hardcode 路径用 env var / V3.7 commit 链测试覆盖 / V3.16 §1.2 措辞精确 / V3.23 §18 #11 措辞<br>**关键洞察**:plan 升级时必须明确"哪些 v0.2.0 复杂度可以删除",**over-engineering 是头号风险** |

---

**结束。下次 session 接手时,从 Phase 1 Task 1.1 开始。**
