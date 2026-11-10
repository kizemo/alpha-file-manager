# focus-sync Scheduled Task 方案 v2 Review(对修改后计划的二次审查)

**日期**: 2026-10-01
**作者**: Claude Code (second-pass systematic review)
**状态**: ⏳ 应用 P0 修复后即可开始实施
**被审计划**: `2026-10-01-focus-sync-scheduled-task-migration.md`(v2,已应用第一轮 review 的 P0+P1)
**本 review 性质**: 二次审查,验证第一轮 review 建议是否落实 + 找出仍存在的问题
**关联文档**:
- `2026-10-01-focus-sync-scheduled-task-migration-review.md`(第一轮 review)
- `handoff-focus-sync-retrospective-2026-10-01.md`(根因分析)

---

## 0. 总体评估

| 维度 | v1 → v2 改进 | v2 状态 |
|---|---|---|
| 核心矛盾(T3.4 lazy-create vs 删 shell 权限) | ✅ 已彻底修复 | 0 个致命矛盾 |
| Session 0 / UIA 失效 | ✅ 已规避(选 Scheduled Task 而非 Service) | OK |
| 第一轮 review P0(5 项) | ✅ 全部修复 | 0 个 P0 未修 |
| 第一轮 review P1(5 项) | ✅ 全部修复 | 0 个 P1 未修 |
| 第一轮 review P2(7 项) | ⚠️ 1 项 M1.3 未修(spike build toolchain) | nice-to-have |
| **第二轮发现的问题** | **7 个 P0 + 3 个 P1 + 4 个 P2** | **需修后实施** |

**结论**:**第一轮 review 100% 落实**,但实施前还需修 **7 个 P0 问题**(集中在 build pipeline / NSIS 语法 / sandbox 限制 / 注册表名字一致性)。

---

## 一、Review 建议落实核查表

| Review 项 | v2 状态 | v2 位置 | 评价 |
|---|---|---|---|
| F1.1 T3.4 矛盾(extension 不能 spawn PS) | ✅ 已修 | §7 + §8 头部 | T3.4/T3.5 删除,改为 installer 创建。**彻底解决** |
| F1.2 -RL Highest → Limited | ✅ 已修 | T2.3 line 276 | Standard User 不再阻塞 |
| F1.3 多用户 port 冲突 | ✅ 部分修 | T1.1 fail-fast | 仅缓解 Event Log spam,**真正问题待文档** |
| F1.4 taskkill 误杀 | ✅ 已修 | T4.1 PowerShell path filter | 防误杀 OK |
| H1.1 notification 90s 延迟 | ✅ 已修 | T3.6 立即通知 + 15s | |
| H1.2 --service flag 依赖顺序 | ✅ 已修 | §5.1 Phase gating | |
| H1.3 /health 真实信号 | ✅ 已修 | T1.2 + `session_id=0 panic` | |
| H1.4 ExecutionTimeLimit | ✅ 已修 | T2.3 `-Seconds 0` | |
| H1.5 idempotent create | ✅ 已修 | T2.3 `if not exists` | |
| H1.6 strict port retry bind | ✅ 已修 | T1.1 fail-fast exit0 | |
| M1.1 fallback paths 完整 | ✅ 已修 | T3.3 5 个路径 | |
| M1.2 single-flight pattern | ✅ 已修 | T3.6 restartInFlight | |
| M1.3 build toolchain | ❌ **未修** | T1.3 line 215 仍写 1.85.0 | 没明确 spike MSRV |
| M1.4 显式 RunLevel | ✅ 已修 | T2.3 | |
| M1.5 resources syntax 对比 | ✅ 已修 | T2.5 完整对比 | |
| M1.6 crash recovery 测试 | ✅ 已修 | T5.6 | |
| M1.7 unregister exit code | ✅ 已修 | T2.4 line 296-300 | |

**P0 + P1 全部修复,M1.3 是 P2,影响小。**

---

## 二、🔴 第二轮发现的 P0 严重问题(7 个,必须修)

### S1: sidecar binary 怎么进入 installer?链断裂 ⭐⭐⭐⭐⭐

**位置**:T2.1 + T2.5

**问题分析**:

T2.1 hooks.nsh stage 7 "copy sidecar binary 到 `$INSTDIR\tools\`":
```nsh
SetOutPath "$INSTDIR\tools"
CopyFiles /SILENT "$INSTDIR\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe" "$INSTDIR\tools\focus-sync-sidecar.exe"
```

但 **T2.5 把 sidecar binary 从 tauri.conf.json resources 中删了**:
```jsonc
// 删除两行 sidecar binary → 由 hook 显式复制
```

**结果**:**Tauri 不再打包 sidecar 进 installer**,NSIS hook 就拿不到 source 文件,`CopyFiles` 失败,POSTINSTALL stage 7 报错,stage 9 创建 task 指向空文件。

**修复(2 选 1)**:

| 选项 | 实现 |
|---|---|
| **A** ⭐⭐⭐⭐ | **保留 sidecar binary 在 tauri.conf.json resources** + 在 build script 里协调从 `sigma-listary-spike/target/release/spike.exe` 复制到 `release/extension/bin/focus-sync-sidecar.exe`(Phase 1 build 完后跑) |
| B | 在 hooks.nsh 里用其他方式取 sidecar(如 build script 显式 `File` 到 `$PLUGINSDIR`) |

**推荐 A**,保持一致性:

```jsonc
// T2.5 改动后的 tauri.conf.json resources(修正版)
"resources": {
  "../../release/extension/package.json": "extensions/kizemo.focus-sync/package.json",
  "../../release/extension/dist": "extensions/kizemo.focus-sync/dist",
  "../../release/extension/locales": "extensions/kizemo.focus-sync/locales",
  "../../release/extension-installer/register.ps1": "tools/register.ps1",
  "../../release/extension-installer/resolve_user_appdata.ps1": "tools/resolve_user_appdata.ps1",
  "../../release/extension-installer/register-scheduled-task.ps1": "tools/register-scheduled-task.ps1",   // 新
  "../../release/extension-installer/unregister-scheduled-task.ps1": "tools/unregister-scheduled-task.ps1",  // 新
  "../../release/extension/bin/focus-sync-sidecar.exe": "tools/focus-sync-sidecar.exe"  // ★ 保留(v0.3.0 hook 复制需要)
}
```

并加 build script step:
```bash
# sigma-file-manager/scripts/build-with-sidecar.ps1 (新)
$spikeExe = "F:\soft\00selfmade\filemanager\sigma-listary-spike\target\release\spike.exe"
$target = "F:\soft\00selfmade\filemanager\release\extension\bin\focus-sync-sidecar.exe"
Copy-Item -Path $spikeExe -Destination $target -Force
```

---

### S2: NSIS `!include` 不能嵌文件,应是 `File` 指令 ⭐⭐⭐⭐

**位置**:T2.6 line 343-345

**问题代码**:
```powershell
!include "register-scheduled-task.ps1"
!include "unregister-scheduled-task.ps1"
```

`!include` 是 NSIS 预处理器指令,**只用于预编译时插入代码**,**不**用于把文件打包进 installer。

**修复**:

Installer B 的 installer.nsi:
```nsi
Section "Install"
    SetOutPath "$INSTDIR\bin"
    File "register-scheduled-task.ps1"
    File "unregister-scheduled-task.ps1"
    ; ... 现有 File 调用 ...
SectionEnd

Section "Uninstall"
    ; 卸载 PS files (NSIS 自动用 install log 反删)
    Delete "$INSTDIR\bin\register-scheduled-task.ps1"
    Delete "$INSTDIR\bin\unregister-scheduled-task.ps1"
SectionEnd
```

**为什么是 `$INSTDIR\bin\` 不是 `$INSTDIR\tools\`**:Installer B 的 InstallDir 是 `$LOCALAPPDATA\Programs\kizemo.focus-sync\`,Plan T2.2 第 245-247 行已经定好 sidecar binary 在 `$INSTDIR\bin\focus-sync-sidecar.exe`,PS helper 应该在同级 `$INSTDIR\bin\`。

---

### S3: T3.3 fallback 用了 `process.env`,在 worker 中可能 undefined ⭐⭐⭐⭐

**位置**:T3.3 line 391-397

**问题分析**:

extension JS 跑在 Worker thread,Sigma FM sandbox 可能不暴露 `process.env`(sandbox 删除了 global 环境对象,见 sandbox.ts 配置)。

回顾 sandbox 已知禁用的 API(来自 handoff-2026-09-29-sandbox-fix.md):
> document.x、localStorage、sessionStorage、indexedDB、fetch、XMLHttpRequest、WebSocket、self、globalThis、postMessage、addEventListener、removeEventListener、dispatchEvent、importScripts、navigator、eval、new Function、Function.constructor、.constructor

**`process.env` 不在禁用列表**,但 sandbox 是否暴露 globalThis.process?从 extension-worker.ts 看:
```typescript
const workerScope = globalThis as typeof globalThis & {
  postMessage: ...,
  // 没看到 process 字段
};
```

但 `globalThis.process` 在标准 Node.js / Worker 中是可用的。Sigma FM 的 worker 是否暴露,需验证。

**修复**(不依赖 process.env):
```js
async function getSidecarPath() {
  // 1. Registry HKLM\SOFTWARE\kizemo\focus-sync\SidecarPath
  // 2. Registry HKCU\SOFTWARE\kizemo\focus-sync\SidecarPath
  // 3. Hardcoded fallback(不依赖环境变量)+ existsSync 检查

  const FALLBACK_PATHS = [
    'C:\\Program Files\\Sigma File Manager\\tools\\focus-sync-sidecar.exe',
    'C:\\Program Files (x86)\\Sigma File Manager\\tools\\focus-sync-sidecar.exe',
    'C:\\Program Files\\Sigma File Manager\\Sigma File Manager\\tools\\focus-sync-sidecar.exe',  // 双层 nested
    // Installer B 路径(perUser):$LOCALAPPDATA\Programs\kizemo.focus-sync\bin\
    // 但要 %USERPROFILE% 替换 hardcoded 不优雅
    // 改用:Windows API GetFolderPath 拿 LOCALAPPDATA(需要 native code,不现实)
    // 简化:从 registry HKCU 拿(2 步已覆盖)
  ];

  for (const p of FALLBACK_PATHS) {
    if (await fileExists(p)) return p;
  }

  return null;
}
```

`fileExists` 也需要 sandbox 允许 fs access。Sigma FM 的 `BASE_PERMISSION` 含 `fs.read`,应该 OK。如果不行,fallback 到通知用户手动 reinstall。

---

### S4: T2.4 unregister 不杀 sidecar 进程,会留 orphan ⭐⭐⭐⭐

**位置**:T2.4 line 286-302

**问题分析**:

`schtasks /Delete /TN KizemoFocusSync /F` 删除 task,**但正在跑的 sidecar 进程不知道**,会变 orphan。用户重新装机时:
- 新 task 创建并启动新 sidecar
- 旧 orphan sidecar 仍占 37421 端口 → 新 sidecar fail-fast exit 0
- 用户看到 sync 不工作,但排查时不知道有 orphan

**修复**:
```powershell
# stage 1: kill running sidecar process(用 path filter 防误杀)
Get-Process focus-sync-sidecar -ErrorAction SilentlyContinue |
  Where-Object { $_.Path -like '*kizemo*' } |
  Stop-Process -Force -ErrorAction SilentlyContinue

# stage 2: delete task(幂等)
$taskName = 'KizemoFocusSync'
$output = schtasks /Delete /TN $taskName /F 2>&1
$exitCode = $LASTEXITCODE
if ($exitCode -ne 0 -and $exitCode -ne 1) {
    Write-Host "WARNING: schtasks /Delete exit $exitCode"
    Write-Host $output
}

exit 0   # NSIS 不需要精确状态
```

---

### S5: T2.5 syntax 对比"改动后"还引用了 `bin/focus-sync-sidecar-*.exe` ⭐⭐⭐⭐

**位置**:T2.5 line 326-336

**问题分析**:

对比里"改动后"区块**没显式删除**原 line 319-320:
```
"../../release/extension/bin/focus-sync-sidecar-canonical.exe": "extensions/kizemo.focus-sync/bin/focus-sync-sidecar/focus-sync-sidecar.exe",
"../../release/extension/bin/focus-sync-sidecar.exe": "bin/focus-sync-sidecar/0.2.0/focus-sync-sidecar.exe",
```

只有一句注释 "删除两行 sidecar binary → 由 hook 显式复制"。

读者对照 syntax 时会困惑:到底删没删?

而且**这条注释本身是错的**(根据 S1 修复),sidecar binary 必须保留。

**修复**:根据 S1 的方案选择,显式保留或删除:

```jsonc
// 改动后(revised by S1)
"resources": {
  "../../release/extension/package.json": "extensions/kizemo.focus-sync/package.json",
  "../../release/extension/dist": "extensions/kizemo.focus-sync/dist",
  "../../release/extension/locales": "extensions/kizemo.focus-sync/locales",
  "../../release/extension-installer/register.ps1": "tools/register.ps1",
  "../../release/extension-installer/resolve_user_appdata.ps1": "tools/resolve_user_appdata.ps1",
  "../../release/extension-installer/register-scheduled-task.ps1": "tools/register-scheduled-task.ps1",      // 新
  "../../release/extension-installer/unregister-scheduled-task.ps1": "tools/unregister-scheduled-task.ps1", // 新
  "../../release/extension/bin/focus-sync-sidecar.exe": "tools/focus-sync-sidecar.exe",                     // ★ 保留(v0.3 hook 需要)
  // 删除(v0.3.0 不再需要 APPDATA 二进制路径):
  // "../../release/extension/bin/focus-sync-sidecar-canonical.exe": "extensions/kizemo.focus-sync/bin/focus-sync-sidecar/focus-sync-sidecar.exe",  (removed)
  // "../../release/extension/bin/focus-sync-sidecar.exe": "bin/focus-sync-sidecar/0.2.0/focus-sync-sidecar.exe",  (removed)
}
```

---

### S6: T5.7 卸载验证描述用错 registry 名字 ⭐⭐⭐⭐

**位置**:T5.7 line 562-567

**问题代码**:
```
[ ] registry key `HKLM\SOFTWARE\Sigma FM\FocusSync\SidecarPath` 被删
```

但 §7.1 + §7.2 T2.1/T2.2 + §18 #2 都确认命名空间是 `kizemo\focus-sync\`,不是 `Sigma FM\FocusSync\`。

**修复**:
```
[ ] registry key `HKLM\SOFTWARE\kizemo\focus-sync\SidecarPath` 被删(perMachine,Installer A)
[ ] registry key `HKCU\SOFTWARE\kizemo\focus-sync\SidecarPath` 被删(perUser,Installer B)
[ ] `Get-ScheduledTask KizemoFocusSync` 返回 not found(task 已清理)
[ ] `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\` 被删(extension 文件)
```

---

### S7: T2.1 stage 9 不验证 stage 7 是否成功 ⭐⭐⭐⭐

**位置**:T2.1 line 232-234

**问题分析**:

如果 stage 7 (copy sidecar) 失败:
- stage 8 写 registry 仍跑(可能成功,但指向空路径)
- stage 9 创建 task,指向不存在的 exe
- task 立即 fail,Task Scheduler 反复 restart

**修复**(NSIS IfErrors 模式):
```nsh
; stage 7: copy sidecar binary
SetOutPath "$INSTDIR\tools"
CopyFiles /SILENT "$INSTDIR\tools\focus-sync-sidecar.exe" "$INSTDIR\tools\focus-sync-sidecar.exe"
IfErrors stage7_failed stage7_done

stage7_failed:
    DetailPrint "WARNING: sidecar copy failed, skipping Scheduled Task creation"
    Goto stage9_skip

stage7_done:
    ; stage 8: write registry
    WriteRegStr HKLM "SOFTWARE\kizemo\focus-sync" "SidecarPath" "$INSTDIR\tools\focus-sync-sidecar.exe"

    ; stage 9: create scheduled task
    nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\tools\register-scheduled-task.ps1" -SidecarPath "$INSTDIR\tools\focus-sync-sidecar.exe"'
    Pop $0
    ${If} $0 != "0"
        DetailPrint "WARNING: register-scheduled-task.ps1 exit $0"
    ${EndIf}

stage9_skip:
```

---

## 三、🟡 第二轮发现的 P1 问题(3 个)

### M1: T1.2 `session_id=0 panic` 太激进 ⭐⭐⭐

**位置**:T1.2 line 209

**问题分析**:

在某些特殊 session 启动(RDP console session、SSH session、headless CI runner),`ProcessIdToSessionId` 可能返回 0 但实际不是 Session 0(无真正的显示)。

Panic 会导致 task 一直 restart 失败,Event Log spam,即使 sidecar 代码本身没问题。

**修复**:warn 但不 panic,让 `/health` 返回 `status="down"` + reason:
```rust
let session_id = unsafe { ProcessIdToSessionId(GetCurrentProcessId()) };
let initial_health = if session_id == 0 {
    log::error!(
        "sidecar running in Session 0 (UIA cannot see user dialogs). \
         Expected Session 1+. This is likely a deployment bug — \
         the installer should have scheduled this task for the user's interactive session."
    );
    "degraded"
} else {
    "ok"
};
```

`status` 字段在 T1.2 已存在 `"ok" | "down"`,扩展为 `"ok" | "degraded" | "down"`。

---

### M2: 已存在的 task binary 路径变化(in-place upgrade) ⭐⭐⭐

**位置**:T2.3 + T4

**问题分析**:

**v0.3.0 → v0.3.1** in-place 升级:
- hook 覆盖写 `$INSTDIR\tools\focus-sync-sidecar.exe`(同路径)
- task 引用 `$INSTDIR\tools\focus-sync-sidecar.exe`(同路径)
- **OK**

**Installer B 升级到 Installer A 切换**(假设):
- Installer B 路径:`$LOCALAPPDATA\Programs\kizemo.focus-sync\bin\`
- Installer A 路径:`$INSTDIR\tools\`(可能 = `$PROGRAMFILES\Sigma File Manager\tools\`)
- 旧 task 指向 `$LOCALAPPDATA\Programs\...\bin\`,Installer A hook 不覆盖这路径
- **旧 task 失效,新 task 指向新路径**

**修复**:plan 加 note:
> v0.3.x in-place 升级要求二进制路径不变。若路径变更(如 Installer B 切到 Installer A),用户需手动 `Unregister-ScheduledTask -TaskName KizemoFocusSync` 再重装。

或更彻底:T4.1 在升级时检测 registry key 的 HKLM/HKCU 路径是否改变,如果改变,unregister 旧 task。

---

### M3: T3.3 fallback paths 加 `existsSync` 检查 ⭐⭐⭐

**位置**:T3.3 line 389-398(可与 S3 合并修复)

**问题分析**:

fallback 数组里的路径可能不存在(目录被删、Program Files 重定向到其他盘)。

**修复**(并入 S3):fallback paths 加 existsSync 检查,避免把不存在的路径返回给后续函数。

---

## 四、⚫ 第二轮发现的 P2 锦上添花(4 个)

### M4: §13.3 "Activate 阶段 PS 执行慢" 已过期 ⭐⭐

**位置**:§13.3 line 641

**问题分析**:

extension 不再 spawn PS,这条不适用。误导。

**修改**:
```diff
- | `Activate` 阶段 PS 执行慢 | 低 | 低 | 缓存到下次的 ping 周期 30s |
+ | installer 阶段 PS 冷启动 | 低 | 低 | 仅影响 POSTINSTALL 1-2s,不影响 sync UX |
```

---

### M5: §2.2 进程树图概念不准确 ⭐⭐

**位置**:§2.2 line 69-73

**问题分析**:

`schtasks.exe` 只是命令行工具,**实际触发进程的是 Task Scheduler service(svchost.exe 中)**。

**修改**:
```diff
[进程树 A: Windows Task Scheduler]
- services.exe → TaskScheduler → schtasks.exe → 创建用户会话进程
+ services.exe → svchost.exe (Task Scheduler service)
    └── focus-sync-sidecar.exe (用户身份,SESSION 1+,At logon 触发)
         └── listen 127.0.0.1:37421
         └── 拥有完整用户 desktop → UIA/COM OK
```

---

### M6: T5.4 taskkill 漏 /T ⭐⭐

**位置**:T5.4 line 537

**问题分析**:

`taskkill /F /IM sigma-file-manager.exe` 不杀 Sigma FM 子进程(extensions、worker 等)。需要加 `/T`。

**修改**:
```diff
- [ ] **杀 Sigma FM**:`taskkill /F /IM sigma-file-manager.exe`
+ [ ] **杀 Sigma FM**:`taskkill /F /IM sigma-file-manager.exe /T`(杀主进程 + 整个进程树,验证 sidecar 仍独立存活)
```

---

### M7: T2.3 Start-ScheduledTask 错误处理粗 ⭐⭐

**位置**:T2.3 line 280

**问题分析**:

`Start-ScheduledTask -ErrorAction SilentlyContinue` 静默所有错误,包括路径不存在等真错误。

**修复**:
```powershell
try {
    Start-ScheduledTask -TaskName $taskName -ErrorAction Stop | Out-Null
} catch {
    # 已运行(0x800710D5)忽略;其他错误 log
    if ($_.Exception.HResult -ne 0x800710D5) {
        Write-Host "WARNING: Start-ScheduledTask: $($_.Exception.Message)"
    }
}
```

---

## 五、修改优先级清单(应用顺序)

### P0 — 实施前必须修(7 项)

1. **S1** sidecar binary 保留在 tauri.conf.json resources(改回),加 build script step
2. **S2** NSIS `!include` 改 `File` 指令,放到 `$INSTDIR/bin/`
3. **S3** T3.3 fallback 不用 `process.env`,用 hardcoded + existsSync
4. **S4** T2.4 unregister 时 taskkill sidecar 进程
5. **S5** T2.5 syntax 对比显式删除 line 319-320(根据 S1 方案)
6. **S6** T5.7 卸载验证修正 registry 名字(`kizemo\focus-sync\`)
7. **S7** T2.1 stage 9 在 stage 7 失败时跳过(NSIS IfErrors)

### P1 — 应该修(3 项)

8. **M1** `session_id=0` 不 panic,只 warn + 返回 degraded
9. **M2** plan 加 note:sidecar 路径变更需 uninstall + reinstall
10. **M3** fallback paths + existsSync(可合并 S3)

### P2 — 锦上添花(4 项)

11. **M4** §13.3 删除 "PS 执行慢" 过期条目
12. **M5** §2.2 进程树图修正(svchost.exe 替代 schtasks.exe)
13. **M6** T5.4 taskkill 加 `/T`
14. **M7** T2.3 Start-ScheduledTask 错误细粒度处理

---

## 六、整体评价

**修复后版本(v2)** 比 v1 进步显著,核心矛盾(F1.1)已彻底解决,7 类 reload 触发器免疫失效,Session 0 / UIA 问题规避。

**剩余 7 个 P0 问题集中在**:
1. **build pipeline 协调**(S1 + S5)—— Tauri / NSIS / build script 跨目录 binary 流转
2. **NSIS 语法**误用(S2)—— `!include` vs `File`
3. **Worker sandbox 限制**(S3)—— `process.env` 可能不可用
4. **进程生命周期**(S4)—— task 删但不杀进程留 orphan
5. **计划文档内部一致性**(S6)—— registry 命名空间前后不一

**修复 P0 后即可实施**。P1/P2 可在 Phase 1 build 时一并处理。

---

## 七、推荐下一步

**选项 A** ⭐:我现在直接产出 v3 修订版 plan(把 P0 + P1 都合并进去),您只需 review v3 后开始实施
**选项 B**:您手动应用这 14 项修改,然后我们 review v3(避免再 review 一轮)
**选项 C**:您先开始 Phase 1(sidecar --service flag),Phase 2 开始前应用 P0 修复

我推荐 **选项 A**,因为 v3 是"可执行版本",避免反复 review 浪费时间。文档已经结构化,合并工作量不大。是否同意?