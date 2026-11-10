# focus-sync v0.3.0 — focus-17 修改建议(基于 focus-16 测试反馈)

**日期**: 2026-10-01
**作者**: Mavis (M3) — 基于 user 提供的两张 console 截图诊断
**输入材料**:
- `handoff-2026-10-01-focus-16-install-test-results.md`(3 问题报告)
- 用户附件 `E:\Users\Duanyi\.minimax\.minimax\v2\assets\2026\10\01\19-32-13-847-asset_20261001-193213-847_0f5ff03c1cc6_f45fe3b8-image.png`(截图1: console 底部日志)
- 用户附件 `E:\Users\Duanyi\.minimax\.minimax\v2\assets\2026\10\01\19-46-36-443-asset_20261001-194636-443_40d4e7a2999c_22235dc6-image.png`(截图2: console 顶部 + UIA inventory)
**接收对象**: Claude Code(用户将本文件转交)
**输出目标**: focus-17-setup.exe(完成所有 3 个问题修复)

---

## 0. TL;DR(给 Claude Code 的执行摘要)

| # | 问题 | 根因 | 修复文件 | 优先级 |
|---|---|---|---|---|
| **1** | 命令行弹窗 | spike.exe 无 `#![windows_subsystem = "windows"]` | `sigma-listary-spike/src/main.rs`(+ logger.rs + Cargo.toml) | P0 |
| **2** | 版本显示 v0.2.0 | `register.ps1` 行 226-233 无脑保留 entry | `release/extension-installer/register.ps1` | P1 |
| **3** | 下载 dialog sync 失败 | `uia_inject.rs` 两个串联 bug(dirname 当 basename + COM 成功后 fall-through) | `sigma-listary-spike/src/uia_inject.rs` | P0 |

**预计改动**:4 个文件 + 1 个新依赖,代码变更 ~50 行。
**预计 build 时间**:15 分钟(spike 重编 + Tauri 打包)。
**预计 install 时间**:5 分钟(用户重启 Sigma FM 即可)。

---

## 1. 截图诊断(关键证据,源自用户两张截图)

### 1.1 截图 1(底部日志)

标题栏:`D:\Program Files\Sigma File Manager\tools\focus-sync-sidecar.exe`
核心证据行:
```
{"event":"DialogDetected","hwnd":7539060,"app":"#32770","ts":"2026-10-01T11:09:01"}
{"event":"Write","hwnd":7539060,"app":"#32770","target":"C:\\DiskGenius_WinPE","strategy":"uia","ts":"2026-10-01T11:09:01"}
BASENAME='C:' to filename edit (was 'C:\DiskGenius_WinPE')
UIA fallback succeeded.
```

### 1.2 截图 2(顶部时序 + UIA inventory)

完整的时序与元素清单。关键证据:
```
{"event":"SpikeError","kind":"startup","message":"spike starting on port 37421 (mode=service)","ts":"2026-10-01T11:05:02"}
{"event":"SpikeError","kind":"startup","message":"spike starting on port 37421 (mode=service)","ts":"2026-10-01T11:06:02"}
[INFO] HTTP server listening port=37421
{"event":"SpikeError","kind":"set_path_dialogs_retry","message":"retried 8x; final dialogs_count=0","ts":"2026-10-01T11:08:43"}
{"event":"SpikeError","kind":"monitor] dialog detected; hwnd=7539060 rect=(847, 174) 1704x927}
{"event":"DialogDetected","hwnd":7539060,"app":"#32770","ts":"2026-10-01T11:08:43"}
{"event":"SpikeError","kind":"opened_write_back","message":"dialog path='BIN大全姐与多兰' current='C:\\downloads'; writing current","ts":"2026-10-01T11:08:43"}
{"event":"Write","hwnd":7539060,"app":"#32770","target":"C:\\downloads","strategy":"uia","ts":"2026-10-01T11:08:43"}
```

UIA inventory 关键行:
```
ctrl=UIA_CONTROLTYPE_ID(50004) automation_id='System.Title' name='应用程序' value='影刀运行及联系开发者'
ctrl=UIA_CONTROLTYPE_ID(50004) automation_id='System.DateTimeModified' name='修改日期' value='2026/ 6/ 5    11:06'
ctrl=UIA_CONTROLTYPE_ID(50004) automation_id='System.Size' name='大小' value='文件夹'
ctrl=UIA_CONTROLTYPE_ID(50003) automation_id='1001' name='文件名' value='C:'                 ← 文件名输入框值
ctrl=UIA_CONTROLTYPE_ID(50003) automation_id='DropDown' name='图片(&B)' value=''
ctrl=UIA_CONTROLTYPE_ID(50000) automation_id='DropDown' name='所有文件' value=''
ctrl=UIA_CONTROLTYPE_ID(50004) automation_id='System.ItemTypeText' name='类型' value='文件夹'
ctrl=UIA_CONTROLTYPE_ID(50004) automation_id='System.ItemNameDisplay' name='名称' value='BIN大全姐与多兰'
ctrl=UIA_CONTROLTYPE_ID(50004) automation_id='HelpButton' name='帮助(&H)' value=''
```

### 1.3 截图合证结论

1. **sidecar 一直在跑**(截图 1 是 11:09:01,截图 2 是 11:08:43,只差 18 秒;sidecar 至少已 alive 4 分钟),"Sidecar not reachable" 通知是 **stale 的**(sidecar 启动前 extension 先 ping 失败,通知机制只显示一次)

2. **sidecar 自动重启了 2 次**(11:05:02 → 11:06:02)— Task Scheduler RestartCount=3 + RestartInterval=1 min 的预期行为,可能是手动 kill 或 crash

3. **dialog class = `#32770`** — 确认是 **Windows Common Dialog**(原生 IFileDialog),不是 Chromium 自绘

4. **Sigma FM 路径 = `C:\downloads`**(截图 2 写事件);截图 1 是 `C:\DiskGenius_WinPE`(另一次测试,不同路径)

5. **dialog 初始 filename 输入框值 = `C:`** — 关键:这是 sidecar 写入后或 dialog 默认值,需要判断

6. **UIA inventory 里没有 `41477` address bar** — Chromium dialog 才会有,所以 `address_bar = None`,代码走 `best_edit` 分支

7. **dialog 是 "影刀运行及联系开发者"**(一个中文 RPA 工具的 Save As dialog)— 测试 app 是 影刀,不是 Chrome 下载

---

## 2. 问题 #1 — 命令行弹窗(P0)

### 2.1 根因(截图证据)

截图 1 的标题 `D:\Program Files\Sigma File Manager\tools\focus-sync-sidecar.exe` 直接证明:
- Sidecar 是 perMachine Installer A 装在 `D:\Program Files\` 下
- Console 窗口可见,说明 spike.exe 是 **console subsystem**

代码确认 — `sigma-listary-spike/src/main.rs` 行 1-2:
```rust
//! Spike binary entry point — full integration.
//!
```
**没有** `#![windows_subsystem = "windows"]` directive,Rust 默认 console subsystem。

### 2.2 修复(必须连带改 logger)

`#![windows_subsystem = "windows"]` 会让 stdout/stderr 不与 console 关联,**所有 `println!` 和 `log_event` 静默丢失**。必须把日志切到文件。

#### 改动 1:加 subsystem attribute

**文件**:`sigma-listary-spike/src/main.rs`
**位置**:第 1 行(在所有 `//!` 注释之前)
**改动**:
```rust
#![windows_subsystem = "windows"]

//! Spike binary entry point — full integration.
//! ...(其余注释完全不变)
```

#### 改动 2:加 tracing-appender 依赖

**文件**:`sigma-listary-spike/Cargo.toml`
**位置**:`[dependencies]` 段(在 `tracing-subscriber` 之后)
**新增**:
```toml
tracing-appender = "0.2"
```

#### 改动 3:tracing 切文件输出

**文件**:`sigma-listary-spike/src/main.rs`
**位置**:行 62-67 的 `tracing_subscriber::fmt().try_init()` 块
**改动前**:
```rust
let _ = tracing_subscriber::fmt()
    .with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,spike=debug")),
    )
    .try_init();
```
**改动后**:
```rust
use tracing_appender::non_blocking;
use tracing_appender::rolling::daily;

let log_dir = spike::logger::log_dir();
let _ = std::fs::create_dir_all(&log_dir);
let file_appender = daily(&log_dir, "spike.log");
let (non_blocking_writer, guard) = non_blocking(file_appender);
// 防 guard drop 导致日志 flush 不全 — leak 到 'static
Box::leak(Box::new(guard));

let _ = tracing_subscriber::fmt()
    .with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,spike=debug")),
    )
    .with_writer(non_blocking_writer)
    .try_init();
```

#### 改动 4:logger.rs::log_event 切文件

**文件**:`sigma-listary-spike/src/logger.rs`
**位置**:行 49-55 的 `log_event` 函数,以及文件顶部 import 段
**改动前**(顶部):
```rust
use crate::events::Event;
use std::io::Write;
use std::sync::Mutex;
use std::time::SystemTime;
```
**改动后**(顶部):
```rust
use crate::events::Event;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;
```
**改动前**(`log_event` 函数):
```rust
pub fn log_event(event: &Event) {
    let _guard = STDOUT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let json = serde_json::to_string(event).expect("Event serialization never fails");
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{}", json);
    let _ = stdout.flush();
}
```
**改动后**:
```rust
/// 返回日志目录(创建如不存在)。Windows 下用 LOCALAPPDATA,
/// fallback 到 TEMP。focus-17 v0.3.0 round-19h: console subsystem 关闭
/// 后 stdout 不可用,所有 log_event 必须切文件。
pub fn log_dir() -> std::path::PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        std::path::PathBuf::from(local).join("kizemo").join("focus-sync").join("logs")
    } else {
        std::env::temp_dir().join("focus-sync-logs")
    }
}

/// 全局共享的 log file appender。OnceLock 保证只打开一次。
fn log_file() -> &'static std::fs::File {
    static FILE: OnceLock<std::fs::File> = OnceLock::new();
    FILE.get_or_init(|| {
        let dir = log_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("spike.log");
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap_or_else(|e| {
                // 兜底:写 TEMP 下的 fallback 路径,避免初始化 panic
                let fallback = std::env::temp_dir().join("focus-sync-fallback.log");
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&fallback)
                    .expect("cannot open fallback log file")
            })
    })
}

pub fn log_event(event: &Event) {
    let _guard = STDOUT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let json = serde_json::to_string(event).expect("Event serialization never fails");
    // v0.3.0 round-19h: stdout 已不可用,改写文件
    let mut f = log_file();
    let _ = writeln!(f, "{}", json);
    let _ = f.flush();
}
```

#### 改动 5:heartbeat println 切 log_event

**文件**:`sigma-listary-spike/src/main.rs`
**位置**:行 168-189 的 `if !cli.service` 分支(heartbeat thread)
**改动前**(行 174-181):
```rust
let ts = now_iso8601();
println!(
    "{{\"event\":\"SpikeError\",\"kind\":\"heartbeat\",\"message\":\"alive pid={}\",\"ts\":\"{}\"}}",
    std::process::id(),
    ts
);
use std::io::Write;
let _ = std::io::stdout().flush();
```
**改动后**:
```rust
let ts = now_iso8601();
log_event(&Event::SpikeError {
    kind: "heartbeat".into(),
    message: format!("alive pid={}", std::process::id()),
    ts,
});
```

注意:`--service` 模式下 heartbeat thread 根本不 spawn(行 168 的 `if !cli.service` 分支),所以只影响 dev 调试场景。

#### 改动 6(可选):uninstall 清理 logs 目录

**文件**:`release/extension-installer/unregister-scheduled-task.ps1`
**位置**:`exit 0` 之前
**新增**:
```powershell
# v0.3.0 round-19h: clean up spike log directory (windows_subsystem="windows"
# redirects all logs here; leaving them behind wastes disk).
$logDir = Join-Path $env:LOCALAPPDATA 'kizemo\focus-sync\logs'
if (Test-Path $logDir) {
    Remove-Item -Recurse -Force $logDir -ErrorAction SilentlyContinue
}
```

注意:这里用 `Remove-Item` 是 PowerShell,不是 mavis-trash 范围(uninstall 是用户主动行为,可恢复删除 OK)。

### 2.3 验证步骤

1. `cd sigma-listary-spike && cargo test` — 预期全部 PASS(logger 新增 log_dir 测试)
2. `cargo build --release` — 产出 `target/release/spike.exe`
3. **手动启动测试**(不带 `--service`):
   - 双击 `spike.exe` → 屏幕**不应**出现 console 窗口
   - `%LOCALAPPDATA%\kizemo\focus-sync\logs\spike.log.<日期>` 应有 JSON 行
   - `curl http://127.0.0.1:37421/health` 仍返回 200
4. **Task Scheduler 启动测试**(带 `--service`):
   - `schtasks /Run /TN KizemoFocusSync` → 不应有 console 窗口
   - log 文件 ~1s 内出现 startup JSON
   - `Get-Content "$env:LOCALAPPDATA\kizemo\focus-sync\logs\spike.log.2026-10-01"` 应看到 startup / session_id / service_mode 三条

### 2.4 风险与缓解

| 风险 | 缓解 |
|---|---|
| log 文件无限增长 | `tracing-appender::daily` 自动按天 rotate;旧文件保留,定期清理脚本(可选) |
| `Box::leak` 内存泄漏 | guard 只占 ~80 字节,进程退出时 OS 回收,长期可忽略 |
| spinner 模式(无 `--service`)dev 调试不便 | spinner 模式本来就是 dev 调试用,丢 stdout 可接受;release 部署都用 `--service` |
| LOCALAPPDATA 不可写(权限问题) | fallback 到 TEMP,不会 panic |

---

## 3. 问题 #2 — 版本显示 v0.2.0(P1)

### 3.1 根因(代码级确认)

`release/extension-installer/register.ps1` 行 226-233:
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

→ 覆盖安装 v0.2.0 → v0.3.0 时,entry 整体保留(manifest 含 version),UI 仍显示 0.2.0。

### 3.2 修复(最小改动原则:只改 version 字段)

**文件**:`release/extension-installer/register.ps1`
**位置**:行 226-233

**改动前**:
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

**改动后**:
```powershell
if ($hasExisting) {
    $existingVersion = $json.installedExtensions.'kizemo.focus-sync'.version
    if ($existingVersion -ne '0.3.0') {
        # v0.3.0 round-19h fix: detect v0.2.0 (or older) entry and upgrade ONLY
        # the version fields. Per round 19c, we preserve everything else to
        # avoid Sigma FM re-normalizing the JSON on next save.
        #
        # Why version-only: Sigma FM UI displays version from
        # installedExtensions[id].version (fallback: manifest.version).
        # Updating only these two string fields keeps byte-format untouched
        # for the rest of the entry.
        $json.installedExtensions.'kizemo.focus-sync'.version = '0.3.0'
        $json.installedExtensions.'kizemo.focus-sync'.manifest.version = '0.3.0'
        Write-Host "==> UPGRADED kizemo.focus-sync entry: v$existingVersion -> v0.3.0 (manifest fields preserved per round 19c)"
    } else {
        Write-Host "==> kizemo.focus-sync entry ALREADY EXISTS at v0.3.0 - preserving as-is (Sigma FM normalized)"
        Write-Host "    version: $($json.installedExtensions.'kizemo.focus-sync'.version)"
        Write-Host "    enabled: $($json.installedExtensions.'kizemo.focus-sync'.enabled)"
    }
} else {
    $json.installedExtensions | Add-Member -NotePropertyName 'kizemo.focus-sync' -NotePropertyValue $kizemoEntry -Force
    Write-Host "==> Added new kizemo.focus-sync entry (v0.3.0 manifest, no binaries segment)"
}
```

### 3.3 验证步骤

1. **场景 A — 全新安装**:
   - 卸载 focus-16(或不安装过)
   - 装 focus-17 → Installed tab 显示 `v0.3.0`(走 `else` 分支,无变化)
2. **场景 B — 覆盖升级(关键)**:
   - 已装 focus-16(v0.2.0 显示)
   - 装 focus-17(v0.3.0)→ 重启 Sigma FM → Installed tab 显示 `v0.3.0`
   - 验证:`Get-Content $env:APPDATA\com.sigma-file-manager.app\user-data\user-extensions.json` 应包含 `"version": "0.3.0"`

### 3.4 风险与缓解

| 风险 | 缓解 |
|---|---|
| Sigma FM normalize 后仍把 v0.3.0 改回 | 只改两个 string 字段,byte 格式不变,round 19c 的 normalize 友好行为完全保留 |
| 用户已经手动改过 version 字段 | 升级逻辑会被下次 install 覆盖(幂等) |

---

## 4. 问题 #3 — 下载 dialog sync 失败(P0,**核心 bug**)

### 4.1 根因(截图 + 代码双重确认)

`sigma-listary-spike/src/uia_inject.rs` 有**两个串联 bug**:

#### Bug A — `dirname` 被当作 `basename` 用(行 332-347)

```rust
// uia_inject.rs 行 332-347
if let Some(edit) = best_edit {
    let basename = dirname(target_path);  // ← 变量名叫 basename,实际调用 dirname()
    if basename != target_path && !basename.is_empty() {
        let value: IUIAutomationValuePattern =
            unsafe { edit.GetCurrentPatternAs(UIA_ValuePatternId)? };
        unsafe { value.SetValue(&BSTR::from(basename.as_str()))? };  // ← 写入了 dirname 结果
        info!(
            "No 41477 address bar found; wrote BASENAME='{}' to filename edit (was '{}').",
            basename,
            target_path
        );
    }
}
```

`dirname()` 函数(行 83-89):
```rust
fn dirname(path: &str) -> String {
    let p = path.trim_end_matches(['\\', '/']);
    match p.rfind(['\\', '/']) {
        Some(idx) => p[..idx].to_string(),  // ← 返回路径的"父目录"部分
        None => p.to_string(),
    }
}
```

**验证对 `C:\downloads` 的执行**:
- `p = "C:\\downloads".trim_end_matches(['\\', '/'])` → `"C:\\downloads"`(不变)
- `p.rfind(['\\', '/'])` → `Some(2)`(`\` 在 index 2)
- `p[..2]` → `"C:"`

→ **dirname("C:\\downloads") = "C:"**(只返回驱动器,丢失所有子路径)

| 输入路径 | dirname 输出 | 真正的 basename |
|---|---|---|
| `C:\downloads` | `C:` | `downloads` |
| `C:\DiskGenius_WinPE` | `C:` | `DiskGenius_WinPE` |
| `D:\photos\video` | `D:\photos` | `video` |

→ 截图 1 中 `BASENAME='C:' to filename edit (was 'C:\DiskGenius_WinPE')` 就是这个 bug 的输出。

#### Bug B — COM SetFolder 成功后 fall-through 到 UIA(行 192-208)

```rust
// uia_inject.rs 行 192-208
let com_ok = try_set_folder_via_com(dialog_hwnd, target_path);
if com_ok {
    info!("Path sync via IFileDialog::SetFolder succeeded.");
    // Don't return — fall through to UIA so Chromium address bar
    // also gets updated (Chromium interprets the SetFolder path as a
    // navigation even though the dialog is COM-driven).
} else {
    warn!("COM SetFolder unavailable; falling back to UIA only.");
}

match inject_via_uia(dialog_hwnd, target_path) {  // ← 总是执行,污染文件名框
    Ok(()) => info!("UIA fallback succeeded."),
    Err(err) => warn!("UIA fallback failed: {err}"),
}
```

**逻辑矛盾**:
- COM SetFolder **成功**意味着 dialog 是 native IFileDialog(Chromium 不支持 COM)
- Native IFileDialog 上 UIA 既不能改地址栏,也不应该污染文件名输入框
- 注释说"Chromium 也会 fall through 让 UIA 处理地址栏",但 Chromium dialog 必然 `com_ok = false`,根本不会进 if 分支

→ **COM 成功后 fall-through 是错逻辑**,且 UIA 分支因为 Bug A 写入了错误的 `C:` 到文件名输入框

### 4.2 修复

#### Bug B 修复:**加 return**(防止 fall-through 污染文件名)

**文件**:`sigma-listary-spike/src/uia_inject.rs`
**位置**:行 192-200

**改动前**:
```rust
let com_ok = try_set_folder_via_com(dialog_hwnd, target_path);
if com_ok {
    info!("Path sync via IFileDialog::SetFolder succeeded.");
    // Don't return — fall through to UIA so Chromium address bar
    // also gets updated (Chromium interprets the SetFolder path as a
    // navigation even though the dialog is COM-driven).
} else {
    warn!("COM SetFolder unavailable; falling back to UIA only.");
}
```

**改动后**:
```rust
let com_ok = try_set_folder_via_com(dialog_hwnd, target_path);
if com_ok {
    info!("Path sync via IFileDialog::SetFolder succeeded.");
    // v0.3.0 round-19h: native IFileDialog 已被 COM 改目录,UIA 既不能改地址栏,
    // 还会污染文件名输入框。直接 return。
    //
    // Chromium dialog 不支持 COM(AccessibleObjectFromWindow 必失败),走 UIA 分支。
    return;
} else {
    warn!("COM SetFolder unavailable; falling back to UIA only.");
}
```

#### Bug A 修复:**改用真正的 basename 提取**

**文件**:`sigma-listary-spike/src/uia_inject.rs`
**位置**:行 332-348

**改动前**:
```rust
// No address bar (this is most native IFileDialogs — SetFolder should
// have already taken care of directory change). Skip the filename box
// entirely to avoid polluting the field with full paths.
if let Some(edit) = best_edit {
    let basename = dirname(target_path);
    if basename != target_path && !basename.is_empty() {
        let value: IUIAutomationValuePattern =
            unsafe { edit.GetCurrentPatternAs(UIA_ValuePatternId)? };
        unsafe { value.SetValue(&BSTR::from(basename.as_str()))? };
        info!(
            "No 41477 address bar found; wrote BASENAME='{}' to filename edit (was '{}').",
            basename,
            target_path
        );
    } else {
        info!(
            "No 41477 address bar; skipping filename write (target is already a basename or empty)."
        );
    }
} else {
    warn!("no Edit element in dialog; nothing written via UIA");
}
```

**改动后**:
```rust
// v0.3.0 round-19h: Chromium Save As dialog 没有 address bar,只能写 filename。
// 但 native IFileDialog 已 return(见上方 Bug B 修复),走到这里只有 Chromium。
//
// 之前用 dirname(target_path) 取 basename 是错的 — 对 "C:\downloads" 返回 "C:"
// (驱动器)。正确的 basename 是路径的最后一个 component。
if let Some(edit) = best_edit {
    // 真正的 basename = 最后一个 \ 或 / 之后的部分
    let basename = match target_path.rfind(['\\', '/']) {
        Some(idx) => &target_path[idx + 1..],
        None => target_path,
    };
    if !basename.is_empty() {
        let value: IUIAutomationValuePattern =
            unsafe { edit.GetCurrentPatternAs(UIA_ValuePatternId)? };
        unsafe { value.SetValue(&BSTR::from(basename))? };
        info!(
            "No 41477 address bar; wrote real BASENAME='{}' (from target '{}') to filename edit.",
            basename, target_path
        );
    } else {
        info!("No 41477 address bar; target has no basename component, skipping.");
    }
} else {
    warn!("no Edit element in dialog; nothing written via UIA");
}
```

#### 附加修复(可选):dirname 函数删除或保留

`dirname` 函数在改完后已无 caller。建议:
- **保留**(`fn dirname(path: &str) -> String`)— 留作 future helper,加 `#[allow(dead_code)]`
- 或**删除** — 简化代码

建议保留(留 `#[allow(dead_code)]`),便于未来可能需要严格 dirname 操作。

### 4.3 关于 "Sidecar not reachable" stale 通知

**判断**:截图证明 sidecar 一直在跑,通知是 stale 的(`extension/dist/index.js` `pingSidecar()` 首次失败时显示通知,后续失败被 `notifiedSinceLastFailure` flag 抑制)。

**短期**:接受现状,不修。

**长期(可选改动)**:让用户在恢复时看到"sidecar 重新可用"的提示。`extension/dist/index.js` 行 99-118:
```js
async function checkHealth() {
    const ok = await pingSidecar();
    if (ok) {
        // v0.3.0 round-19h: clear stale failure flag so user sees recovery
        if (notifiedSinceLastFailure) {
            try {
                sigma.ui.showNotification({
                    title: 'Focus Sync',
                    description: 'Sidecar recovered on 127.0.0.1:37421.',
                    type: 'info',
                    duration: 5000,
                });
            } catch (e) {}
        }
        notifiedSinceLastFailure = false;
        return;
    }
    if (!notifiedSinceLastFailure) {
        try {
            sigma.ui.showNotification({...});
        } catch (e) {}
        notifiedSinceLastFailure = true;
    }
}
```

**优先级**:低,不阻塞 focus-17 验收。

### 4.4 验证步骤(7-app dialog 矩阵)

| App | Dialog 类型 | 期望行为 | 测试方法 |
|---|---|---|---|
| **影刀 RPA Save As** | 原生 IFileDialog (`#32770`) | COM SetFolder 跳地址栏,文件名框保持干净 | 打开影刀 Save dialog → 看 address bar + filename |
| **Chrome 下载** | Chromium Save As | UIA 写 filename(`downloads` 或 `DiskGenius_WinPE`) | Chrome → 设置 → 下载 → 改保存位置 |
| **Firefox 下载** | 类似 Chromium | 同 Chrome | 下载 → 选保存 |
| **Edge Save As** | 原生 IFileDialog | COM 跳地址栏 | Edge → 收藏 → 导出 |
| **VS Code Open Folder** | 原生 IFileDialog | COM 跳地址栏 | VS Code → File → Open Folder |
| **Word Save As** | 原生 IFileDialog | COM 跳地址栏 | Word → File → Save As |
| **Photos app Save** | 原生 IFileDialog | COM 跳地址栏 | Photos → 编辑 → 保存 |

**关键验证**:每个 app 的 filename 输入框都**不应**被填入 `C:` 或类似错误值。

### 4.5 风险与缓解

| 风险 | 缓解 |
|---|---|
| Bug B 修复后某些 dialog 上 COM SetFolder 意外失败,导致地址栏不动 | fallback 到 UIA(已有逻辑) |
| Bug A 修复后 basename 提取对 UNC 路径(`\\server\share\file.txt`)错误 | Rust 的 `rfind` 找到第一个 `\`,UNC 路径上需要特殊处理。建议测试 |
| 影刀 dialog 的 AutomationId 1148 不是 filename edit | `filename_edit_score` 已支持 1148 / 1001 / 名称匹配,优先级正确 |
| 用户期望"完整路径写到 filename"而非 basename | 保持现状(basename 是原生 IFileDialog 的 filename 字段语义,完整路径会被截断) |

---

## 5. 实施顺序(给 Claude Code 的 step-by-step)

### Phase A — 修改源码(预计 20 分钟)

```bash
# 0. 进入 spike 目录
cd F:\soft\00selfmade\filemanager\sigma-listary-spike

# 1. 应用 §2 改动 1-5(main.rs + logger.rs + Cargo.toml)
#    - main.rs 第 1 行加 #![windows_subsystem = "windows"]
#    - main.rs 第 62-67 tracing 切文件
#    - main.rs 第 174-181 heartbeat println 改 log_event
#    - logger.rs 顶部 import + log_event 切文件 + 新增 log_dir() / log_file()
#    - Cargo.toml 加 tracing-appender = "0.2"

# 2. 应用 §4 改动(uia_inject.rs)
#    - 行 192-200 加 return(防 fall-through)
#    - 行 332-348 改 basename 提取逻辑
#    - dirname 函数加 #[allow(dead_code)] 或保留不动

# 3. 应用 §3 改动(register.ps1)
#    - 行 226-233 替换为 version mismatch 检测分支
```

### Phase B — 单元测试(预计 2 分钟)

```bash
cd F:\soft\00selfmade\filemanager\sigma-listary-spike
cargo test
# 预期:全部 PASS(包括新增的 logger::log_dir 测试)
```

### Phase C — 重新编译 + 打包(预计 10 分钟)

```bash
# 1. release build spike.exe
cd F:\soft\00selfmade\filemanager\sigma-listary-spike
cargo build --release

# 2. 同步到 release/extension/bin/ 和 release/extension-installer/
powershell F:\soft\00selfmade\filemanager\sigma-file-manager\scripts\build-with-sidecar.ps1

# 3. Tauri build 输出 focus-17-setup.exe
cd F:\soft\00selfmade\filemanager\sigma-file-manager
npm run tauri build
# 产出:F:\soft\00selfmade\filemanager\release\Sigma-File-Manager-2.2.0-focus-17-setup.exe
```

### Phase D — 手动验证(预计 5 分钟)

```
1. 双击 release\Sigma-File-Manager-2.2.0-focus-17-setup.exe → 安装
2. 不要关闭 conhost(因为没有 conhost 了)
3. 验证窗口不弹出(立即)
4. 验证 Task Scheduler:
   schtasks /Run /TN KizemoFocusSync
   - 不应出现 console 窗口
   - %LOCALAPPDATA%\kizemo\focus-sync\logs\ 应有 spike.log.<日期>
5. 验证健康:
   curl http://127.0.0.1:37421/health
   - status=ok, mode=service, session_id=1
6. 重启 Sigma FM → Installed tab 显示 v0.3.0
7. 测试 7-app dialog 矩阵(§4.4)
```

---

## 6. 不在本次范围(等下一轮)

- `extension/dist/index.js` stale 通知清除(§4.3)— 接受现状
- `unregister-scheduled-task.ps1` 清理 logs 目录(§2.2 改动 6)— 可选
- `dirname` 函数清理(§4.2)— 可选
- `extension/dist/index.js` normalizePath 把 `C:` 补成 `C:\`(原 §4.5)— 取消(sidecar 写 dialog 是写整个 basename,不是 raw path)
- `extension/package.json` description 微调
- `extension/locales/*.json` 翻译 stale 通知文案
- `FORK-KEEP-LIST.md` / memory 更新
- 13 个 commit + tag v0.3.0

---

## 7. 风险汇总(给用户决策)

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| `windows_subsystem` 后 dev 调试不便 | 中 | 低 | log 文件 + `--log-dir` CLI flag(可选) |
| `Box::leak` guard 长期内存 | 低 | 低 | guard ~80 字节,可忽略 |
| Bug B 修复后某些 native dialog COM SetFolder 失败导致地址栏不动 | 中 | 中 | fallback UIA 已存在 |
| Bug A 修复后 UNC 路径 basename 错误 | 低 | 低 | 普通用户场景无 UNC |
| `register.ps1` version 字段改动后 Sigma FM normalize 覆盖 | 低 | 中 | 只改两个 string,byte 格式不变 |
| 编译时间超 10 分钟 | 低 | 低 | Tauri 冷启动 ~5 分钟,rebuild ~30 秒 |

---

## 8. 完成判据(给 Claude Code 的验收清单)

- [ ] `cargo test` 全部 PASS
- [ ] `cargo build --release` 产出 spike.exe(双击无 console 窗口)
- [ ] log 文件路径正确,内容是 JSON 行
- [ ] `npm run tauri build` 产出 focus-17-setup.exe
- [ ] 安装 focus-17 无 console 弹窗
- [ ] Task Scheduler 启动后 %LOCALAPPDATA%\kizemo\focus-sync\logs\ 有 startup log
- [ ] `/health` 返回 mode=service, session_id=1, status=ok
- [ ] 重启 Sigma FM → Installed tab 显示 v0.3.0
- [ ] 覆盖升级场景(focus-16 → focus-17)→ Installed tab 显示 v0.3.0
- [ ] 影刀 Save As dialog → 跳到 `C:\downloads`(地址栏) + 文件名输入框不被填入 `C:`
- [ ] 7-app dialog 矩阵全部通过

---

## 9. 给用户(转交前的注意事项)

1. **本文档是给 Claude Code 执行的实施指引**,不是再 review 一遍。所有修改决策已在 §2-§4 给定。
2. **预计改动**:4 个文件 + 1 个新依赖,代码变更 ~50 行。
3. **预计时间**:从开始到 focus-17-setup.exe 产出 ~40 分钟(含 cargo build + tauri build)。
4. **不要求 Claude Code 提交 git**(用户曾表示"测试通过再 commit")。
5. **如有疑问**:Claude Code 可随时 ping 我(Mavis)确认细节,文档已尽可能自包含。

**用户请确认**:本文档是否需要补充 / 修改?如果 OK,请转交 Claude Code 开始 Phase A。
