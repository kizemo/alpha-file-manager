# focus-sync v0.3.0 focus-16 测试问题分析与修复方案

**日期**: 2026-10-01
**作者**: Mavis (M3)
**输入文档**: `handoff-2026-10-01-focus-16-install-test-results.md`
**代码基线**: `git status -uno` 显示 12 文件 modified / 3 deleted / 2 created,working tree,未 commit
**输出目标**: 为 handoff §2 列出的 3 个问题逐一给出根因、具体代码修复、验证步骤、build 顺序

---

## 1. 通读结论(代码现状盘点)

读完 12 个文件后,确认 Phase 1–4 实施与 v0.5 plan 的差异如下:

| 文件 | 与 v0.5 plan 一致? | 备注 |
|---|---|---|
| `sigma-listary-spike/src/main.rs` | ✅ | `--service` flag / session_id / heartbeat 分支 / startup log 全部实现 |
| `sigma-listary-spike/src/config.rs` | ✅ | spinner vs service fail-fast 模式分支,匹配 V3.22 |
| `sigma-listary-spike/src/state.rs` | ✅ | + 5 health tracking 字段,/health 快照就绪 |
| `sigma-listary-spike/src/http_server.rs` | ✅ | 9 字段 /health,mode=service/spawn,tested |
| `sigma-listary-spike/src/logger.rs` | ⚠️ | **stdout-only**,`#![windows_subsystem = "windows"]` 后会静默丢日志(问题 #1 修复的连带改动点) |
| `sigma-listary-spike/Cargo.toml` | ✅ | + `Win32_System_RemoteDesktop` feature |
| `sigma-file-manager/src-tauri/installer/hooks.nsh` | ✅ | POSTINSTALL stage 0.5-9 + POSTUNINSTALL 清理 |
| `sigma-file-manager/src-tauri/tauri.conf.json` | ✅ | resources 改写完整 |
| `release/extension-installer/installer.nsi` | ✅ | stage 1.5 + 2.5 + 6 |
| `release/extension-installer/register-scheduled-task.ps1` | ✅ | 跨 installer 路径变更检测 + ExecutionTimeLimit 0 |
| `release/extension-installer/unregister-scheduled-task.ps1` | ✅ | 先删 task 再杀 orphan |
| `release/extension-installer/register.ps1` | ❌ | **行 226-233 的 preserve-existing 分支会阻断 v0.2.0 → v0.3.0 升级**(问题 #2) |
| `sigma-file-manager/scripts/build-with-sidecar.ps1` | ✅ | 双目录拷贝,V4.3 修订已应用 |
| `release/extension/package.json` | ✅ | 0.3.0,删除 binaries + shell permission |
| `release/extension/dist/index.js` | ✅ | sigma.http.request, setInterval/setTimeout fallback |

**核心结论**:Phase 1/3/4 实施完整,问题 #1/#2 是**已知小缺陷**(console subsystem / preserve-existing 边界条件),问题 #3 是**新现象需要诊断**。

---

## 2. 问题 #1 🔴 — 命令行弹窗(P0)

### 2.1 现象(用户附图1)

安装 focus-16 后,Windows 桌面显示一个 conhost 黑窗口,标题 `D:\Program Files\Sigma File Manager\tools\focus-sync-sidecar.exe`,内容是 4 行 JSON。

### 2.2 根因(代码级确认)

`sigma-listary-spike/Cargo.toml` 行 12-14:
```toml
[[bin]]
name = "spike"
path = "src/main.rs"
```

没有 `#![windows_subsystem = "windows"]` attribute,Rust 默认走 console subsystem。

`sigma-listary-spike/src/main.rs` 行 1-2:
```rust
//! Spike binary entry point — full integration.
//!
```

没有 subsystem 指令。

`sigma-listary-spike/src/main.rs` 行 168-189(heartbeat / startup log):
```rust
println!("{{\"event\":\"SpikeError\",\"kind\":\"heartbeat\",...}}");   // 直接 stdout
log_event(&Event::SpikeError { kind: "startup".into(), ... });        // 走 logger.rs → stdout
log_event(&Event::SpikeError { kind: "service_mode".into(), ... });   // 同上
```

`sigma-listary-spike/src/logger.rs` 行 49-55:
```rust
pub fn log_event(event: &Event) {
    let _guard = STDOUT_LOCK.lock()...;
    let json = serde_json::to_string(event)...;
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{}", json);
    let _ = stdout.flush();
}
```

**确凿**:3 处 stdout 输出都会触发 conhost 窗口。

### 2.3 Scheduled Task 为什么不自动隐藏 console?

Tauri 默认 spawn 子进程时使用 `CREATE_NO_WINDOW = 0x08000000`,这是 v0.2.0 spawn 模式不显示 console 的原因。

Scheduled Task 启动 binary 时:
- `Start-ScheduledTask` 等价于用户手动双击 .exe → Windows 按 binary subsystem 类型决定窗口行为
- console subsystem binary → 必出 conhost
- windows subsystem binary → 不出窗口
- **Task Scheduler 没有 "Hide Console" flag**(曾有 `Hidden` 属性但 v1903 起 deprecated,schtasks 不支持)

### 2.4 修复方案(实施清单)

**核心改动**:在 `sigma-listary-spike/src/main.rs` 文件顶部加 `#![windows_subsystem = "windows"]`

**连带改动(必须)**:`windows_subsystem = "windows"` 会让 stdout/stderr 不与 console 关联,所有 `println!` 和 `log_event` 写入静默丢失。必须把日志切到文件。

| # | 改动 | 文件 | 内容 |
|---|---|---|---|
| 1 | 加 subsystem attribute | `sigma-listary-spike/src/main.rs` 第 1 行 | `#![windows_subsystem = "windows"]` |
| 2 | 改 tracing 输出到文件 | `sigma-listary-spike/src/main.rs` 行 62-67 | `tracing_subscriber::fmt()` 改用 `tracing_appender::rolling::daily(log_dir, "spike.log")` |
| 3 | 改 logger.rs::log_event 写文件 | `sigma-listary-spike/src/logger.rs` 行 49-55 | `std::io::stdout()` → `OpenOptions::append().open(log_file_path)` |
| 4 | 删 main.rs 行 174-178 heartbeat println | `sigma-listary-spike/src/main.rs` | 在 `--service` 模式下根本没跑,确认 spinner 模式不写 stdout |
| 5 | 加 `tracing-appender` 依赖 | `sigma-listary-spike/Cargo.toml` | `tracing-appender = "0.2"` |
| 6 | log 目录选择 | 新增函数 `log_dir()` | Windows: `%LOCALAPPDATA%\kizemo\focus-sync\logs\`;fallback `%TEMP%\focus-sync-logs\` |
| 7 | uninstall 时清理 logs 目录(可选) | `release/extension-installer/unregister-scheduled-task.ps1` | 删目录 |

### 2.5 文件改写示例(供 Claude Code 实施参考)

**改动 1 — main.rs 第 1 行加 attribute**:
```rust
#![windows_subsystem = "windows"]

//! Spike binary entry point — full integration.
//! ... (其余注释不变)
```

**改动 2 — tracing 切文件输出**:
```rust
// main.rs 第 62-67 替换为:
use tracing_appender::rolling::daily;

let log_dir = spike::logger::log_dir();
let _ = std::fs::create_dir_all(&log_dir);
let file_appender = daily(&log_dir, "spike.log");
let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
let _ = tracing_subscriber::fmt()
    .with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,spike=debug")),
    )
    .with_writer(non_blocking)
    .try_init();
// 注意 _guard 必须保留,否则 non_blocking writer 会在函数结束时 drop → 日志丢尾
// 应通过 std::mem::forget 或 Box::leak 延长生命周期,或传入 main 函数作为 'static 引用
```

**改动 3 — logger.rs::log_event 切文件**:
```rust
// logger.rs 新增 module-level 静态:
use std::sync::OnceLock;
static LOG_FILE: OnceLock<std::fs::File> = OnceLock::new();

fn open_log_file() -> &'static std::fs::File {
    LOG_FILE.get_or_init(|| {
        let dir = log_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("spike.log");   // 单文件 append,tracing-appender 负责 daily rotate
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .expect("open log file")
    })
}

pub fn log_event(event: &Event) {
    let _guard = STDOUT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let json = serde_json::to_string(event).expect("Event serialization never fails");
    let mut f = open_log_file();
    let _ = writeln!(f, "{}", json);
    let _ = f.flush();
}

pub fn log_dir() -> std::path::PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        std::path::PathBuf::from(local).join("kizemo").join("focus-sync").join("logs")
    } else {
        std::env::temp_dir().join("focus-sync-logs")
    }
}
```

**改动 4 — heartbeat println**(在 `if !cli.service` 分支里):
```rust
// main.rs 行 174-178 替换为:
let ts = now_iso8601();
log_event(&Event::SpikeError {
    kind: "heartbeat".into(),
    message: format!("alive pid={}", std::process::id()),
    ts,
});
```

### 2.6 验证步骤

1. **单元测试 PASS**:
   ```bash
   cd sigma-listary-spike && cargo test
   ```
   新增测试:`log_dir_returns_localappdata_when_env_set`(mock LOCALAPPDATA)

2. **Build + 手动启动**:
   ```bash
   cargo build --release
   # 双击 release/spike.exe 应无 console 窗口
   # %LOCALAPPDATA%\kizemo\focus-sync\logs\spike.log.<yyyy-mm-dd> 应有 JSON 行
   curl http://127.0.0.1:37421/health   # 仍返回 200
   ```

3. **Task Scheduler 启动场景**:
   ```powershell
   schtasks /Run /TN KizemoFocusSync
   # 1. 屏幕上不应该出现 console 窗口
   # 2. 日志文件应在 ~1s 内出现 startup JSON
   Get-Content "$env:LOCALAPPDATA\kizemo\focus-sync\logs\spike.log.2026-10-01"
   ```

4. **回归 v0.2.0 spinner 行为**:`cargo run -- --port 37425`(无 `--service`)应仍写 stdout(允许 console),便于本地调试。

### 2.7 风险与回退

- **风险**:log 文件无 size cap → 长期运行无限增长。`tracing-appender::rolling::daily` 自动 rotate 但保留所有历史。建议加 `max_log_files = 7` 或 `Builder::default().max_log_files(7)`。
- **回退**:如出问题,临时方案是在 `register-scheduled-task.ps1` 用 schtasks /Create 加 `Hidden` flag(虽然 deprecated 但 v1903-22H2 仍生效)。
- **回退方案 B(成本最低)**:在 `Sigma-File-Manager-2.2.0-focus-XX-setup.exe` 的 stage 0 加 `taskkill /F /IM focus-sync-sidecar.exe /T` 之前先 `taskkill /F /IM conhost.exe /FI "WINDOWTITLE eq *focus-sync-sidecar*"`(hack,不推荐)

---

## 3. 问题 #2 🟡 — 版本显示 v0.2.0(P1)

### 3.1 现象(用户附图2)

Sigma FM Extensions > Installed tab 显示 "Focus Sync kizemo **v0.2.0**",但用户装的是 focus-16(v0.3.0)。

### 3.2 根因(代码级确认)

`release/extension-installer/register.ps1` 行 226-233(改动 **未** 应用):
```powershell
if ($hasExisting) {
    Write-Host "==> kizemo.focus-sync entry ALREADY EXISTS — preserving as-is (Sigma FM normalized)"
    Write-Host "    version: $($json.installedExtensions.'kizemo.focus-sync'.version)"
    Write-Host "    enabled: $($json.installedExtensions.'kizemo.focus-sync'.enabled)"
} else {
    $json.installedExtensions | Add-Member -NotePropertyName 'kizemo.focus-sync' -NotePropertyValue $kizemoEntry -Force
    Write-Host "==> Added new kizemo.focus-sync entry (v0.3.0 manifest, no binaries segment)"
}
```

注释说这是 round 19c 设计:不要 re-serialize,避免 Sigma FM normalize 后识别为 "never seen"。

**实际副作用**:覆盖安装 v0.2.0 → v0.3.0 时,manifest 完全不动,version 字段保持 0.2.0。Sigma FM UI 读 `installedExtensions['kizemo.focus-sync'].manifest.version` → 显示 0.2.0。

### 3.3 修复方案

**最小改动原则**:仅升级 `version` + `manifest.version` 两个字段,**其他保留**(维持 round 19c 的 normalize 友好行为)。

| # | 改动 | 文件 | 行 |
|---|---|---|---|
| 1 | 加 version mismatch 检测分支 | `release/extension-installer/register.ps1` | 行 226-233 替换为升级逻辑 |

### 3.4 代码改写

```powershell
# register.ps1 行 226-233 替换为:

if ($hasExisting) {
    $existingVersion = $json.installedExtensions.'kizemo.focus-sync'.version
    if ($existingVersion -ne '0.3.0') {
        # v0.3.0 fix: detect v0.2.0 (or older) entry and upgrade ONLY the
        # version fields. Per round 19c, we preserve everything else to avoid
        # Sigma FM re-normalizing the JSON on next save.
        #
        # Why version-only: Sigma FM UI displays version from
        # installedExtensions[id].version (with fallback to manifest.version).
        # Updating only these two string fields keeps byte-format untouched
        # for the rest of the entry.
        $json.installedExtensions.'kizemo.focus-sync'.version = '0.3.0'
        $json.installedExtensions.'kizemo.focus-sync'.manifest.version = '0.3.0'
        Write-Host "==> UPGRADED kizemo.focus-sync entry: v$existingVersion → v0.3.0 (manifest fields preserved per round 19c)"
    } else {
        Write-Host "==> kizemo.focus-sync entry ALREADY EXISTS at v0.3.0 — preserving as-is (Sigma FM normalized)"
        Write-Host "    version: $($json.installedExtensions.'kizemo.focus-sync'.version)"
        Write-Host "    enabled: $($json.installedExtensions.'kizemo.focus-sync'.enabled)"
    }
} else {
    $json.installedExtensions | Add-Member -NotePropertyName 'kizemo.focus-sync' -NotePropertyValue $kizemoEntry -Force
    Write-Host "==> Added new kizemo.focus-sync entry (v0.3.0 manifest, no binaries segment)"
}
```

### 3.5 验证步骤

1. **unit test(可选)**:用 Pester 测 version 升级分支
2. **手动测试**:
   - 安装 focus-15(v0.2.0)→ 装 focus-16(v0.3.0)→ 重启 Sigma FM → Installed tab 应显示 v0.3.0
   - 全新安装 focus-16 → 显示 v0.3.0(走 `else` 分支,无变化)
3. **回归风险**:不动其他字段,round 19c 的 normalize 友好行为完全保留。

### 3.6 注意

`release/extension-installer/register.ps1` 第 219-224 行 `$kizemoEntry.settings` 块覆盖了 `__binaries`,但仅在 `if ($hasExisting) ... else { Add-Member }` 的 **else** 分支用到。这段代码有点冗余(每次 install 都重建 settings),但不影响问题 #2。

`sigma-file-manager/src-tauri/installer/hooks.nsh` 调用的 `register.ps1` 是同一份(`tools/register.ps1` 是 Tauri resources 拷过去的),所以一次修改两边生效。

---

## 4. 问题 #3 🔴 — 下载对话框 sync 失败(P0)

### 4.1 现象(用户附图3)

Chrome/Firefox 下载弹窗:
- **地址栏**:`此电脑 > photos (G:) > video`(未跳)
- **文件名**:`C:`
- **右下角 notification**:"Sidecar not reachable. Open Task Scheduler → KizemoFocusSync → Run."

### 4.2 根因分析(分层)

这是**多因素**问题,需分清"根本没 sync"和"sync 了但写错"。

#### 4.2.1 因素 A — sidecar 实际未运行(最可能)

**证据**:持续显示 "Sidecar not reachable" 通知。`extension/dist/index.js` 行 71-88:
```js
async function pingSidecar() {
    try {
        const r = await sigma.http.request({ url: `${SIDECAR_HTTP}/health`, ... });
        if (r.status !== 200) return false;
        const body = JSON.parse(r.body || '{}');
        return body.status !== 'down' && body.status !== undefined;
    } catch (e) {
        return false;   // ← connection refused / timeout 走这里
    }
}
```

行 99-118:首次 false → 显示通知 + `notifiedSinceLastFailure = true`,**之后失败不再通知**(避免噪音)。

→ 用户看到通知说明 **extension activate 那一刻 sidecar 就已经 unreachable**,而不是 sync 过程中断的。

**可能子原因**:
1. **A1**:Task 注册成功但从未启动(`-RunLevel Limited` + `-LogonType Interactive` 配错、UAC 限制、用户用 fast user switching)
2. **A2**:Task 启动后 sidecar bind 37421 失败 → fail-fast exit 0(config.rs 行 45-47)→ Task Scheduler RestartCount=3 耗尽 → 不再重启
3. **A3**:sidecar 在某次 sync 后崩溃(Rust panic)→ Task Scheduler 重启 3 次都失败 → 不再重启
4. **A4**:sidecar 与 extension 在不同 Windows session(罕见,用户报告 session_id=1 但可能 RDP 多 session)

#### 4.2.2 因素 B — sync 路径被截断为 "C:"

`extension/dist/index.js` 行 29-33:
```js
function normalizePath(p) {
    if (typeof p !== 'string') return p;
    let stripped = p.replace(/^sfm:\/?/i, '');
    return stripped.replace(/\//g, '\\');
}
```

行 219-221(activate):
```js
const initialPath = await sigma.context.getCurrentPath();
if (initialPath) schedulePush(normalizePath(initialPath));
```

如果 `sigma.context.getCurrentPath()` 返回 `"C:"`(驱动器根目录无尾斜杠),会发 `/set_path` body `{"path":"C:"}`。

`sigma-listary-spike/src/http_server.rs` 行 124-228 handle_set_path 把 `C:` 传给 `writer::write_path`:
```rust
let outcome = crate::writer::write_path(d.hwnd as isize, &path);
```

`sigma-listary-spike/src/writer.rs` 行 36:
```rust
crate::uia_inject::inject_folder_path(hwnd_u32, target);
```

uia_inject 内部走 `UIA ValuePattern::SetValue(target)`,把 `C:` 写到 Chromium Save Dialog 的 **filename 输入框** → 用户看到 `C:` 在 filename box。地址栏不动(SetValue 写不到 breadcrumb)。

#### 4.2.3 因素 C — Chromium Save Dialog 不暴露 IFileDialog COM

`handoff` 已说明:Chrome/Firefox/Edge 下载弹窗是 Chromium 自绘,不暴露真实 `IFileDialog` 接口 → COM `SetFolder()` 不可用 → 只能走 UIA `SetValue` → 只能写 filename。

**v0.2.0 round 19 已知限制**:Chromium dialog 的地址栏(Address bar)不在 UIA tree 顶层,无法 SetValue。

#### 4.2.4 综合判断

最可能的因果链:
```
A1/A2/A3 sidecar 没在跑
  → extension pingSidecar() false
    → 通知 "Sidecar not reachable"
      → 用户以为 sync 失败,实际根本没收到 /set_path
        → filename 显示 dialog 默认值 "C:"
          → 用户以为是 sync 写错
```

**真正问题 = sidecar 没在跑**,filename "C:" 只是 dialog 默认状态(不是 sidecar 写入)。

### 4.3 缺失的诊断信息(等用户)

handoff §3 请求了 5 项,**前 2 项是关键**:
1. `curl -v http://127.0.0.1:37421/health` 输出
2. `Get-WinEvent -LogName "Microsoft-Windows-TaskScheduler/Operational" -MaxEvents 20` 输出
3. Sigma FM 当前路径(用户是不是真的切到 "C:" 驱动器了?)
4. 是 Chrome / Firefox / Edge 还是其他 app
5. v0.2.0 时同 dialog 是否工作过

### 4.4 候选修复方案(诊断前可先做)

| # | 假设 | 修复 | 优先级 |
|---|---|---|---|
| **3.4.1** | A1 — Task 没启动 | 改 `register-scheduled-task.ps1` 加 `-RunLevel Highest`(放弃 Limited) | 仅在诊断确认后做 |
| **3.4.2** | A2 — bind 失败 fail-fast 退出 | `config.rs` fail-fast 改为"bind 后 sleep 5s 重试"(会重启)或"log file 留痕" | 仅在 A2 确认后做 |
| **3.4.3** | A3 — 崩溃 | `main.rs` panic hook 已经写 stderr,但 stderr 也丢;**改用 file appender**(同 #1 修复连带) | 必修 |
| **3.4.4** | B — `getCurrentPath()` 返回 "C:" | `normalizePath` 加 driver-root 后缀补全:`if (/^[A-Z]:$/i.test(p)) return p + '\\';` | 必修(防御性) |
| **3.4.5** | C — Chromium dialog 只能写 filename | 接受现状,文档化限制;或加 SendInput fallback 直接打字 | 仅在 3.4.4 修完后仍无效时做 |

### 4.5 必修项 3.4.4(防御性 normalize 修补)

**改动**:`extension/dist/index.js` 行 29-33:
```js
function normalizePath(p) {
    if (typeof p !== 'string') return p;
    let stripped = p.replace(/^sfm:\/?/i, '');
    // v0.3.0 round 19h: bare driver root "C:" → "C:\\"
    // Reason: writer sends via UIA SetValue; if path is "C:" (no trailing
    // backslash), Chromium Save dialog interprets it as filename, not folder.
    // Adding trailing backslash makes it a valid folder path.
    stripped = stripped.replace(/\//g, '\\');
    if (/^[A-Z]:$/i.test(stripped)) {
        stripped = stripped + '\\';
    }
    return stripped;
}
```

**为什么现在可以修**:sidecar 自己也有 address-bar 写入失败的可能(Chromium dialog 不支持 COM SetFolder),但 `C:` → `C:\` 至少让 filename box 显示 `C:\` 而非 `C:`,给用户视觉反馈(告诉他"sidecar 试图写文件夹,但只能写到 filename")。

**配合 #1 修复后的 sidecar 验证**:即便 sidecar 还没起,extension 已发 "C:\" → 即使失败用户也能区分"sidecar 没起来" vs "sidecar 起来但写错"。

### 4.6 待用户补充诊断后再做

不要现在就动 uia_inject.rs / writer.rs。等诊断信息。

---

## 5. 实施顺序(给 Claude Code)

**Phase A — P0 必修(必须 PASS 才算 v0.3.0 ready)**
- [ ] A1 实施 §2.4 改动 1-7(文件:5 处)
- [ ] A2 实施 §4.5(文件:1 处,5 行)
- [ ] A3 重新 `cargo test`(预期 23+ 个 PASS,新加 1-2 个 log_dir 测试)
- [ ] A4 重新 build:`build-with-sidecar.ps1` → `npm run tauri build` → 输出 focus-17-setup.exe
- [ ] A5 安装 focus-17:
  - 验证无 console 窗口
  - 验证 `%LOCALAPPDATA%\kizemo\focus-sync\logs\spike.log.<date>` 有内容
  - 验证 `curl http://127.0.0.1:37421/health` 返回 mode=service / session_id=1
  - 验证 Chromium 下载弹窗 filename 显示 `C:\`(不再是 `C:`)

**Phase B — P1 必修**
- [ ] B1 实施 §3.4 register.ps1 改动
- [ ] B2 重新 build → focus-18-setup.exe
- [ ] B3 安装:
  - 全新装 → Installed tab 显示 v0.3.0
  - 覆盖装(v0.2.0 → v0.3.0)→ Installed tab 显示 v0.3.0(关键)

**Phase C — 待用户补充诊断信息**
- [ ] C1 收集 §4.3 五项数据
- [ ] C2 根据数据落 §4.4 候选修复
- [ ] C3 重新 build → focus-19-setup.exe
- [ ] C4 端到端 7-app 验证

**Phase D — 收尾**
- [ ] D1 commit 13 个 commit(`feat:`, `fix:`, `docs:`, `chore:` 分类)
- [ ] D2 tag `v0.3.0`
- [ ] D3 更新 FORK-KEEP-LIST.md(Scheduled Task 模式)
- [ ] D4 更新 memory `sigma_fm_extension_registration.md` 到 v0.3.0
- [ ] D5 更新 v0.5 plan 文档(把 v3/v4 review 修订合并回去)

---

## 6. 风险评估

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| `windows_subsystem = "windows"` 后无法在 IDE 控制台看 startup log | 中 | 调试不便 | 日志切文件 + 文件路径常量 + 添加 `--log-dir` CLI 覆盖 |
| `tracing-appender` daily rotate 不清理旧文件 | 中 | 磁盘占满 | `Builder::default().max_log_files(7)` |
| register.ps1 改动导致 Sigma FM 拒绝 entry(normalize 拒绝) | 低 | 用户看到 v0.3.0 但扩展不工作 | 只改两个 string 字段,byte 格式不变 |
| normalizePath 加 `\\` 后 user path "C:" 真不是 driver root | 低 | 用户路径被破坏 | 严格正则 `^[A-Z]:$` 才追加 |
| Phase A 改 #1 后,spinner 模式(`--service` 不传)丢日志 | 低 | 调试不便 | spinner 是 dev/debug 模式,丢 stdout 可接受 |

---

## 7. 给用户的清单

**我已做完**:完整代码 review(§1) + 3 个问题的根因分析 + 具体修复方案 + 实施顺序。

**需要你决定**:
1. **Phase A 是否开始?** 如果 yes,告诉 Claude Code 实施 §2.4 + §4.5,预计 30-60 分钟,产出 focus-17。
2. **Phase B 是否并行?** 我建议等 A 完成验证后再做(避免 build 链路反复重跑)。
3. **Phase C(问题 #3)**:请按 handoff §3 提供 5 项诊断数据,**尤其是 #1 curl 输出 + #2 TaskScheduler event**。这两项是判断 A1/A2/A3 的关键。

**我不会做的(避免越权)**:
- 改 `sigma-file-manager/src/` 下任何 fork 源码文件(等用户授权)
- commit / tag / 更新 FORK-KEEP-LIST / 更新 memory(等用户授权)
- 删任何 `mavis-trash` 范围外的文件(等用户授权)
