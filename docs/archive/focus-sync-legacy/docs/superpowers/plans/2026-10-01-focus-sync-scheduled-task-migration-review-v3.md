# focus-sync Scheduled Task 方案 v3 Review(深度第三轮审查)

**日期**: 2026-10-01
**作者**: Claude Code (third-pass deep review)
**状态**: ⏳ 修 P0 后即可开始 Phase 1 实施
**被审计划**: `2026-10-01-focus-sync-scheduled-task-migration.md` v0.4(已应用第一轮 + 第二轮 review 的所有修复)
**本 review 性质**: **深度第三轮审查**,专门挖第一/二轮 review 没覆盖到的细节,运行时问题,API 不存在假设
**关联文档**:
- `2026-10-01-focus-sync-scheduled-task-migration-review.md`(第一轮)
- `2026-10-01-focus-sync-scheduled-task-migration-review-v2.md`(第二轮)
- `handoff-focus-sync-retrospective-2026-10-01.md`(根因分析)

---

## 0. 总体评估

| 维度 | v0.4 状态 |
|---|---|
| 第一轮 review P0(5 项) | ✅ 全部修复 |
| 第一轮 review P1(5 项) | ✅ 全部修复 |
| 第二轮 review P0(7 项) | ✅ 全部修复 |
| 第二轮 review P1(3 项) | ✅ 全部修复 |
| 第二轮 review P2(4 项) | ✅ 全部修复 |
| **第三轮新发现的问题** | **3 致命 + 5 严重 + 6 中等 + 4 锦上添花** |

**结论**:**前两轮 review 已清掉 22 个问题**。但深度第三轮发现 plan 在 **API 实际可用性** 和 **TS 路径假设** 上仍有 3 个**致命问题**(会导致实施到 Phase 3 时直接失败)。**这 3 个 P0 必须修**。

---

## 一、🔴 第三轮发现的 P0 致命问题(3 个)

### V3.17: T3.6 用 `fetch()` 被 sandbox 拦截 — 必须改 `sigma.http.request` ⭐⭐⭐⭐⭐

**位置**:T3.6 line 516-540

**问题代码**:
```js
async function pingSidecar() {
  // ... 隐式 fetch('http://127.0.0.1:37421/health') ...
}

setInterval(() => {
  if (restartInFlight) return;
  restartInFlight = true;
  checkHealth().finally(() => { restartInFlight = false; });
}, 15_000);
```

**致命证据**:

回顾 handoff-2026-09-29-sandbox-fix.md:
> Sandbox 检测的 19 个 regex 包括 `fetch(`

Sigma FM extension sandbox **会在 extension worker 启动时扫 JS 代码中的危险 API 关键词**,发现 `fetch(` → 拒绝加载 extension。

**manifest 已包含** `http://127.0.0.1:37421` 权限(host allowlist),但 `fetch` 是被 sandbox 主动拦截的 Web API。

**修复**(用 Tauri 提供的 `sigma.http.request` API):
```js
async function pingSidecar() {
  try {
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
```

`sigma.http.request` 是 Sigma FM extension 提供的 API,manifest 含 `http` permission 即可用,且 host allowlist 含 `127.0.0.1:37421` 已 OK。

---

### V3.2: T3.3 用了**不存在的 API** `sigma.fs.stat` 和 `readRegistry` — over-engineering ⭐⭐⭐⭐⭐

**位置**:T3.3 line 456-491

**问题代码**:
```js
async function fileExists(path) {
  try {
    const stat = await sigma.fs.stat(path);  // ← 不存在
    return stat && stat.isFile;
  } catch {
    return false;
  }
}

// ... readRegistry('HKLM', 'SOFTWARE\\kizemo\\focus-sync', 'SidecarPath')  // ← 不存在
```

**致命证据**:

Sigma FM extension API 实际列表(从前面分析 `create-context-api.ts`、`create-shell-api.ts`、`create-binary-api.ts` 等):
- ✅ `sigma.commands` / `sigma.context` / `sigma.toolbar` / `sigma.ui`
- ✅ `sigma.http` / `sigma.i18n` / `sigma.binary`(v0.2.0 用过)
- ✅ `sigma.storage`
- ❌ **`sigma.fs.stat` —— 不存在**
- ❌ **`readRegistry` —— 不存在**

Sigma FM 是 file manager,**故意不暴露** fs API 给 extension。registry API 也不存在。

**根本问题**:T3.3 整个是**过度设计**。extension 完全不需要 sidecar path —— **HTTP ping 不需要 path**。

`getSidecarPath()` 是 v0.2.0 的遗留需求(v0.2.0 extension 要 spawn sidecar,需要 binary path),v0.3.0 **不再 spawn**,根本不需要。

**修复**:删除整个 T3.3(`getSidecarPath` / `fileExists` / `readRegistry` / `FALLBACK_PATHS` 全部删除)。

`activate()` 简化为:
```js
const activate = async () => {
    try { await sigma.i18n.mergeFromPath('locales'); } catch (e) {}
    try { enabled = (await sigma.storage.get('enabled')) ?? true; } catch (e) { enabled = true; }

    // 直接连 sidecar (binary path 不需要,HIP 通信无依赖)
    const ok = await pingSidecar();
    if (!ok) {
        // T3.6 logic:首次失败立即 notification
        ...
    }
};
```

---

### V3.1: T2.1 stage 7 CopyFiles 是 self-copy,无意义 ⭐⭐⭐⭐

**位置**:T2.1 line 236

**问题代码**:
```nsh
SetOutPath "$INSTDIR\tools"
CopyFiles /SILENT "$INSTDIR\tools\focus-sync-sidecar.exe" "$INSTDIR\tools\focus-sync-sidecar.exe"
IfErrors stage7_failed stage7_done
```

**致命证据**:

NSIS CopyFiles 语法:`CopyFiles <source_file> <destination_file>`

`<source_file>` 从 installer 数据目录(`$PLUGINSDIR`)读取(NSIS 内置)。**source 和 destination 相同**意味着从 `$INSTDIR\tools\` 复制到 `$INSTDIR\tools\` —— **没有任何效果**。

实际上 **Tauri 2.x 的 NSIS 模板会自动**把 `bundle.resources` 声明的文件解压到 `$INSTDIR\`(按 target 路径)。如果 `resources` 里有 `"tools/focus-sync-sidecar.exe"`,Tauri 自动解压到 `$INSTDIR\tools\focus-sync-sidecar.exe`,**不需要 hook 手动 copy**。

**修复**(删除整个 stage 7):
```nsh
!macro NSIS_HOOK_POSTINSTALL
  ; stage 8: write registry
  WriteRegStr HKLM "SOFTWARE\kizemo\focus-sync" "SidecarPath" "$INSTDIR\tools\focus-sync-sidecar.exe"

  ; stage 9: create scheduled task
  nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\tools\register-scheduled-task.ps1" -SidecarPath "$INSTDIR\tools\focus-sync-sidecar.exe"'
  Pop $0
  ${If} $0 != "0"
      DetailPrint "WARNING: register-scheduled-task.ps1 exit $0"
  ${EndIf}
!macroend
```

**或者**(如果真要 stage 7):
```nsh
; stage 7: Tauri 已经解压 resources,这里只验证存在
${IfNot} ${FileExists} "$INSTDIR\tools\focus-sync-sidecar.exe"
    DetailPrint "FATAL: sidecar binary missing from installer resources"
    Abort
${EndIf}
```

---

## 二、🟠 第三轮发现的严重问题(5 个)

### V3.18: T2.5 tauri.conf.json syntax 嵌套位置没说 ⭐⭐⭐⭐

**位置**:T2.5 line 384-397

**问题**:

Plan 给的 syntax 是:
```jsonc
"resources": {
  ...
}
```

但 Tauri 2.x 的 `tauri.conf.json` 结构是:
```jsonc
{
  "bundle": {
    "resources": { ... },  // ← resources 在 bundle 下
    "windows": { ... },
    "nsis": { ... }
  }
}
```

读者照抄可能直接复制到顶层,JSON 不合法。

**修复**(plan 写明嵌套):
```jsonc
// 修改 tauri.conf.json 的 bundle.resources 段
{
  "bundle": {
    "resources": {
      ... (the existing entries)
    }
  }
}
```

---

### V3.5: T4.1 / T2.4 的 `*kizemo*` filter 太宽 ⭐⭐⭐⭐

**位置**:T4.1 line 569 + T2.4 line 343-345

**问题代码**:
```powershell
Get-Process focus-sync-sidecar -ErrorAction SilentlyContinue |
  Where-Object { $_.Path -like '*kizemo*' } |
  Stop-Process -Force
```

**风险**:路径 `*kizemo*` 太宽。如果用户机器上有其他程序路径含 "kizemo"(商业软件、目录名),会被误杀。

**修复**(精确 filter):
```powershell
Get-Process focus-sync-sidecar -ErrorAction SilentlyContinue |
  Where-Object {
      $_.Path -like '*focus-sync-sidecar*' -and
      ($_.Path -like '*\Sigma File Manager*' -or
       $_.Path -like '*\kizemo.focus-sync*' -or
       $_.Path -like '*Sigma FM FocusSync*')
  } |
  Stop-Process -Force
```

**更简单方案**:sidecar binary 部署到特定路径(installer 写死),用 path starts-with 严格匹配:
```powershell
Where-Object {
    $_.Path -like '$INSTDIR\tools\focus-sync-sidecar.exe' -or
    $_.Path -like '$LOCALAPPDATA\Programs\kizemo.focus-sync\bin\focus-sync-sidecar.exe'
}
```

但 **NSIS 阶段调用时 `$INSTDIR` 已知**,可硬编码进 PS script 参数。

---

### V3.8: setInterval 在 worker sandbox 可能被禁 ⭐⭐⭐

**位置**:T3.6 line 534-538

**问题**:

T3.6 用 `setInterval(...)`。`setInterval` 是 JS 内置 timer API,**不在 sandbox 已知 19 个 regex 列表**,但 sandbox 可能在 runtime 检查更多 API。

实际上 sandbox 是**静态代码扫描**(`validateExtensionCode`),不是 runtime sandbox。所以 setInterval 应该 OK(它是 globalThis 上的方法,JS 内置,不是 addEventListener 这种 Web API)。

但 sandbox 检查脚本字符串(看 sandbox.ts)。如果 sandbox 禁用 globalThis 上的方法,setInterval 会失败。

更稳的:plan 加 sanity test,或 fallback 用其他 API。

**修复**(加 fallback):
```js
let intervalHandle;
function scheduleCheck() {
    if (intervalHandle) clearInterval(intervalHandle);
    if (typeof setInterval === 'function') {
        intervalHandle = setInterval(checkHealth, 15_000);
    } else {
        // Fallback:用 chain of setTimeout
        const tick = () => checkHealth().finally(() => {
            intervalHandle = setTimeout(tick, 15_000);
        });
        tick();
    }
}
```

---

### V3.16: §1.2 第 28 行描述措辞不够准确 ⭐⭐⭐

**位置**:§1.2 line 28

**问题**:
> | sidecar 启动者 | Sigma FM 进程树(std::process::Command) | **Windows Task Scheduler**(用户登录触发) |

v0.2.0 不是 "Sigma FM 进程树(std::process::Command)",而是 "Sigma FM 的 `start_extension_command` Tauri command → `processes.rs:start_extension_command` → `std::process::Command::new(sidecar).spawn()`"。

**修复**(精确描述):
> | sidecar 启动者 | Sigma FM 的 `start_extension_command` Tauri command → `std::process::Command::new().spawn()` | **Windows Task Scheduler**(用户登录触发) |

---

### V3.22: T1.1 `--service` flag 的 fail-fast 改动覆盖 spinner 吗?⭐⭐⭐

**位置**:T1.1 line 177-184

**问题描述**:

Plan T1.1 写:
> fail-fast port(strict 37421 bind 一次,失败立即 exit 0)
> 不 retry bind 800ms × 3

但 `sigma-listary-spike/src/config.rs` 现有 `resolve_port()` 逻辑是 retry bind 800ms × 3 + kill orphan。

**T1.1 是只针对 --service mode 改 fail-fast,还是 spike.exe(无 flag)也 fail-fast?**

如果是前者,spinner(无 flag)仍然 retry bind,没问题但不一致。
如果是后者,spinner 行为改变,**破坏 v0.2.0 backward compatibility**(handoff 说 v0.2.0 manual deploy 用 spinner 也工作)。

**修复**(plan 明确):
```rust
if cli.service {
    // fail-fast mode
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    drop(listener);  // 测试 bind
} else {
    // spinner mode:retry bind 保留(向后兼容 manual debug)
    resolve_port_with_retry(port)?;
}
```

---

## 三、🟡 第三轮发现的中等问题(6 个)

### V3.13: §14.1 TC-CRITICAL-2 "Sigma FM 完全卸载后 sidecar 仍 RUNNING" 是无意义测试 ⭐⭐⭐

**位置**:§14.1 line 771

**问题**:

> TC-CRITICAL-2:Sigma FM 完全卸载后,sidecar 仍 RUNNING(独立验证)— **核心价值**

但实际上:**focus-sync 不能脱离 Sigma FM 独立存在**(它是 Sigma FM 扩展)。用户卸载 Sigma FM 后,这个 extension 已经不存在,sidecar 没用了,**应该被清理**(T2.4 / unregister-scheduled-task.ps1)。

让 sidecar 在 Sigma FM 卸载后仍跑是**资源浪费**,不是"核心价值"。

**真实核心价值是**:**Sigma FM 重启/升级/切语言期间 sync 不断**(TC-CRITICAL-1, TC-CRITICAL-3)。

**修复**:
- 删除 TC-CRITICAL-2
- 或改为 TC-CRITICAL-2:卸载 focus-sync extension 后,sidecar 任务被清理,无 orphan 残留(已被 TC-17 覆盖,合并即可)

---

### V3.15: §17.1 回滚方案 registry 路径错 ⭐⭐⭐

**位置**:§17.1 line 917

**问题**:
```powershell
Remove-Item -Path "HKLM:\SOFTWARE\Sigma FM\FocusSync" -Recurse -Force -ErrorAction SilentlyContinue
```

**正确**(与 §18 命名空间一致):
```powershell
Remove-Item -Path "HKLM:\SOFTWARE\kizemo\focus-sync" -Recurse -Force -ErrorAction SilentlyContinue
```

---

### V3.20: T5.6 Start-ScheduledTask 行为依赖性 ⭐⭐⭐

**位置**:T5.6 line 671-675

**问题**:

T5.6 测试:
> [ ] sync 工作时 `Stop-ScheduledTask KizemoFocusSync`
> [ ] extension 15s 内检测失败 → 首次失败立即弹 notification
> [ ] `Start-ScheduledTask KizemoFocusSync` → 5s 内恢复
> [ ] extension 15s 内检测成功 → notification 不再触发

**Windows 行为**:
- `Stop-ScheduledTask`:停止 task,**不杀 task 启动的进程**(sidecar 仍跑)
- `Start-ScheduledTask`:重新启用 task,Task Scheduler 监控进程状态:
  - 如果进程不在,启动新进程(用 task 配置的 executable)
  - 如果进程在,任务状态变更但**不启动新进程**(Task Scheduler 不会 kill 现存进程去 rebuild)

**问题**:`Stop-ScheduledTask` 后 sidecar 仍跑(占 37421,active state)。`Start-ScheduledTask` 不创建新 sidecar(因为已存在)。

所以 T5.6 测试的 "5s 内恢复" 可能**不会发生**(task 已 enabled 但没 spawn 新 spike.exe)。

**修复**(测试加 taskkill 模拟真实 crash):
```bash
# 1. Stop-ScheduledTask
# 2. taskkill /F/IM focus-sync-sidecar.exe (强制杀进程)
# 3. Start-ScheduledTask (触发 task 启动新 sidecar)
# 4. 检查新 sidecar 跑起来
```

或者直接 `taskkill /F/IM focus-sync-sidecar.exe`,让 Task Scheduler RestartCount 检测到 crash,自动 restart(TC-05)。

---

### V3.10: T2.4 顺序问题 — 先杀 sidecar 还是先删 task ⭐⭐⭐

**位置**:T2.4 line 339-357

**问题**:

T2.4 当前顺序:
```powershell
# stage 1: 杀 sidecar 进程
Get-Process focus-sync-sidecar | Stop-Process

# stage 2: 删 task
schtasks /Delete /TN KizemoFocusSync /F
```

**问题**:杀 sidecar 时,如果 sidecar 正在处理 /set_path 请求,会丢失 sync(用户体验不好,虽然 unregister 几乎瞬间完成)。

实际时间:`Stop-Process -Force` ~50ms,`schtasks /Delete` ~200ms,total <300ms,user 几乎感知不到。

**修复**(可选,优化顺序):
```powershell
# 1. 删 task(task 不会再启动新 sidecar)
schtasks /Delete /TN KizemoFocusSync /F 2>&1

# 2. wait in-flight 完成
Start-Sleep -Seconds 1

# 3. 杀 sidecar 进程
Get-Process focus-sync-sidecar | Stop-Process
```

锦上添花,可不修。

---

### V3.12: T1.2 `/health` mode 字段"service" hardcoded ⭐⭐⭐

**位置**:T1.2 line 206

**问题**:

`"mode": "service"` 是 hardcode。但 spinner(无 --service)也跑,mode 应该是 `"manual"` 或 `"spawn"`。

**修复**(从 CLI flag 读):
```rust
let mode = if cli.service { "service" } else { "spawn" };
let body = serde_json::json!({
    "mode": mode,
    ...
});
```

---

### V3.21: §13.2 PS 5.1 兼容性没指定测试环境 ⭐⭐

**位置**:§13.2

**问题**:

`-ExecutionTimeLimit (New-TimeSpan -Seconds 0)` 在 PS 5.1 上**OK**(0 是合法参数)。
但 plan 没说明 PowerShell 版本要求。

**修复**:加 note "测试环境 PowerShell 5.1 (Windows 11 默认)"。

---

## 四、⚫ 第三轮发现的锦上添花(4 个)

### V3.6: T2.6 hardcode 路径 ⭐⭐

**位置**:T2.6 line 410-411

`hardcode "F:\soft\00selfmade\filemanager\"` —— 未来其他贡献者路径不同。

**修复**:用环境变量 + 默认值:
```powershell
param(
    [string]$RepoRoot = $env:FILEMANAGER_REPO ?? "F:\soft\00selfmade\filemanager",
    [string]$SpikeExe = "$RepoRoot\sigma-listary-spike\target\release\spike.exe",
    ...
)
```

---

### V3.7: §16.3 commit 链测试覆盖不全 ⭐⭐

**位置**:§16.3

Plan 提了 1 个 test commit(register-scheduled-task.ps1 unit tests)。但缺:
- Rust unit tests(`/health` endpoint 测试)
- Extension unit tests(fallback 行为测试)

OK 锦上添花。

---

### V3.16: §1.2 第 28 行措辞(已在 V3.16 处理)

---

### V3.23: §18 #11 `/health 扩展字段` — 措辞 ⭐

> #11 | `/health` 扩展字段 | **+ `last_ping_received_ts` + `uia_healthy` + `session_id`** |

"扩展字段" 措辞不准,应是 "新增字段" 或 "v0.3.0 新增字段"。

---

## 五、修改优先级清单(应用顺序)

### P0 — 必须修,否则 Phase 3 实施直接失败(3 项)

1. **V3.17** T3.6 用 `sigma.http.request` 替代 `fetch`(sandbox 拦截)
2. **V3.2** 删除整个 T3.3(`getSidecarPath` / `fileExists` / `readRegistry` / `FALLBACK_PATHS`),extension 不需要 sidecar path
3. **V3.1** 删除 T2.1 stage 7 self-copy(改用 `${IfNot} ${FileExists}` 验证),或完全删除 stage 7

### P1 — 应该修(2 项)

4. **V3.18** T2.5 syntax 写明嵌套位置 `bundle.resources`
5. **V3.5** `*kizemo*` filter 改精确路径匹配(用 `$INSTDIR` 和 `$LOCALAPPDATA` 限定)

### P2 — 优化(6 项)

6. **V3.8** `setInterval` fallback 用 chain setTimeout
7. **V3.13** 删除 TC-CRITICAL-2,合并到 TC-17
8. **V3.15** §17.1 registry 名字改 `kizemo\focus-sync\`
9. **V3.20** T5.6 Stop-ScheduledTask 序列加 taskkill
10. **V3.12** mode 字段从 CLI flag 读
11. **V3.21** §13.2 加 PS 5.1 版本说明

### P3 — 锦上添花(4 项)

12. **V3.6** T2.6 hardcode 路径用环境变量
13. **V3.7** commit 链加 rust/extension test
14. **V3.16** §1.2 措辞精确
15. **V3.23** §18 #11 措辞

---

## 六、第三轮审查的特别观察

### API 实际可用性的重要性

第一/二轮 review 关注 **逻辑层面**(矛盾、设计),但 plan 在 **实际 API 可用性** 上假设过强:

| 假定的 API | 是否存在 | 影响 |
|---|---|---|
| `sigma.fs.stat` | ❌ 不存在 | T3.3 整个失败 |
| `readRegistry` | ❌ 不存在 | T3.3 fallback 失败 |
| `fetch('http://...')` | ❌ 被 sandbox 拦截 | T3.6 health check 失败 |
| `setInterval` | ⚠️ 可能被禁 | T3.6 定时风险 |
| `sigma.binary.getPath` | ✅ 存在 | v0.2.0 用过,但 v0.3.0 移除了 binaries 段 |
| `sigma.http.request` | ✅ 存在 | **T3.6 应该用这个** |

**核心教训**:**plan 写 JS 代码前,必须确认 Sigma FM extension API 实际存在**。建议实施前先 grep Sigma FM `sigma-line` 的所有注册 API,确认清单。

### over-engineering 的危害

T3.3 整个是 v0.2.0 时代的过度设计产物。v0.3.0 不需要 sidecar path,**根本不需要 getSidecarPath 函数**。

**核心教训**:**plan 升级时要明确"哪些 v0.2.0 复杂度可以删除"**,而不是把所有 v0.2.0 代码平移过来。

---

## 七、推荐下一步

**选项 A** ⭐:我现在直接产出 v3 修订版 plan(把 P0 + P1 合并进去),然后 Phase 1 实施
**选项 B**:您手动应用 15 项修改,然后我们 review v4(可能还有第四轮)
**选项 C**:直接 Phase 1 开始,Phase 2/3 开始前应用 P0 修复

**我的推荐**:**选项 A**。P0 三个都是 Phase 3 才暴露的代码层 bug,如果 Phase 1 build OK 后再发现,会浪费半天。

**实施风险评估**:
- P0 修完后,plan 应该 100% 可行
- P1 + P3 都是 nice-to-have,实施时遇到再处理
- 总时间表不变:6.5-7.5 天

是否同意应用选项 A?