# focus-17 实施规划 review 意见(Mavis → Claude)

**日期**: 2026-10-01
**作者**: Mavis (M3)
**输入文档**: `docs/superpowers/plans/2026-10-01-focus-17-bug-fix-implementation-plan.md`
**基础**: `docs/superpowers/plans/2026-10-01-focus-17-modification-recommendations.md`
**性质**: 第二轮 review(plan 实施前)。只提意见,不执行修改。

---

## 0. 总体评价

**整体良好** — Claude 准确理解了 3 个 bug 的根因(`dirname`-as-`basename` + COM fall-through + windows_subsystem 缺失 + register.ps1 preserve-existing 阻断升级),任务切分 1→2→3→4 顺序合理,TDD 步骤清晰,Global Constraints(NO git commit during execution、备份 register.ps1、不要删文件)和 Review Focus(UNC、trailing slash、Sigma FM normalize)都覆盖到位。

**但有 2 个必修 bug**:
- 🔴 **P0**:Task 2 的 `basename_drive_root_only_returns_empty` 测试断言错了 — 实现返回 `"C:"`,测试期望 `""`,**测试会失败**
- 🔴 **P0**:Task 1 删了 `STDOUT_LOCK` 改并发写文件 — main / heartbeat / UIA monitor / HTTP server 4 个线程并发调 `log_event`,**写入会交错乱码**

外加 1 处错误判断(UNC 路径 plan 说不支持,实际支持)和几处改进。

---

## 1. 🔴 必修问题

### 1.1 [P0] Task 2 Step 1 — `basename_drive_root_only_returns_empty` 测试断言错误

**位置**:Task 2 / Step 1 / 行 372-376

**当前代码**:
```rust
#[test]
fn basename_drive_root_only_returns_empty() {
    // Edge case: "C:\" trimmed to "C:" then no separator → returns "C:"
    // (entire string). This is acceptable: it means "no real basename".
    assert_eq!(basename(r"C:\"), "");
}
```

**问题**:注释说"returns 'C:' (entire string)",但断言期望 `""` — **测试会 FAIL**。

**追溯实现**(`basename()` 函数体,Task 2 Step 3):
```rust
fn basename(path: &str) -> &str {
    let p = path.trim_end_matches(['\\', '/']);  // "C:\" → "C:"
    match p.rfind(['\\', '/']) {
        Some(idx) => &p[idx + 1..],
        None => p,   // ← 走这里,返回 "C:"
    }
}
```

→ 对 `r"C:\"` 实际返回 `"C:"`,不是 `""`。

**修复方案**(二选一):
- **方案 A**:修断言为 `assert_eq!(basename(r"C:\"), "C:");`,与注释一致
- **方案 B**:改实现使 `r"C:\"` 返回 `""`(例如加判断 `if p == "C:" || p == "D:" ... → return ""`)。但这增加复杂度,且 Sigma FM `getCurrentPath()` 实际从不返回纯 driver root,过度设计

**推荐方案 A**。

### 1.2 [P0] Task 1 Step 4 — 删除 `STDOUT_LOCK` 导致并发写文件乱码

**位置**:Task 1 / Step 4 / 行 198-203 + Step 4 末尾注释"(Note: drop the old `STDOUT_LOCK` if it exists...)"

**问题**:`log_event()` 在多个线程并发调用:
- main loop 的 select 分支(`signal::ctrl_c()` / `async_rx.recv()` / `tokio::time::sleep()`)
- heartbeat 线程(spawn 的 `spike-heartbeat`)
- UIA monitor(`spawn_blocking` 起的 sync task)
- HTTP server 线程(`spike-http`)

删 `STDOUT_LOCK` 后,这 4 个线程同时 `writeln!` 文件,**OS 层文件写入会交错,产生半截 JSON 行**。Sigma FM 后续读 spike.log 解析就会报错。

**原 `STDOUT_LOCK` 注释**:
```rust
//! Each event is one line of JSON on stdout,
//! suitable for aggregation by `verify_target_matrix.ps1`.
```
原来用 `Mutex<()>` 串行化多线程 stdout 写入是**有意为之**(同 line JSON 不能并发写)。

**修复方案**:
```rust
// logger.rs 顶部保留
use std::sync::Mutex;
static LOG_LOCK: Mutex<()> = Mutex::new(());

// log_event 函数
pub fn log_event(event: &Event) {
    let _guard = LOG_LOCK.lock().unwrap_or_else(|e| e.into_inner());  // ← 保留锁
    let json = serde_json::to_string(event).expect("Event serialization never fails");
    let mut f = log_file();
    let _ = writeln!(f, "{}", json);
    let _ = f.flush();
}
```

只需把 `STDOUT_LOCK` 改名 `LOG_LOCK`(语义更准确)+ 在 file write 上加锁即可,不增加复杂度。

**为什么 plan 说"atomic for small writes on Windows"是错的**:`OpenOptions::append` 只设置文件指针起始位置为末尾(`O_APPEND` 在 POSIX 下 atomic 些,Windows `FILE_APPEND_DATA` 不保证),不保证 line-level 原子性。多个线程并发写一个 fd,write() 调用返回的字节数不一定是一次 `writeln!` 的完整内容。

---

## 2. 🟡 改进建议

### 2.1 [P1] Task 2 / Review Focus — UNC 路径实际是支持的,plan 写错了

**位置**:Task 0 / Review Focus / 条件 1

**Plan 当前说**:
> UNC paths — `\\server\share\file.txt`. The new `basename()` uses `rfind` and would return `share\file.txt` (still wrong). Not exercised in tests because no test path is UNC. Behavior: NOT SUPPORTED — skip with notification (acceptable per user's "ordinary users only" scope).

**实际验证**(对 `\\server\share\file.txt`):
- `p = "\\server\share\file.txt".trim_end_matches(...)` → 不变
- `p.rfind(['\\'])` → 找到最后一个 `\` 的 index(在 `share` 和 `file.txt` 之间)
- `&p[idx + 1..]` → `"file.txt"` ✓

→ **`basename()` 对 UNC 路径返回 `"file.txt"` 是正确的**。Plan 这条 review focus 错误,应改为"UNC 已支持,可加测试覆盖"。

**修复方案**:
```rust
#[test]
fn basename_unc_supported() {
    // UNC paths like \\server\share\file.txt return the leaf filename
    // because rfind returns the LAST separator, which is between
    // "share" and "file.txt".
    assert_eq!(basename(r"\\server\share\file.txt"), "file.txt");
}
```

→ 顺便把 Review Focus 条件 1 从 "NOT SUPPORTED" 改为 "tested, supported"。

### 2.2 [P1] Task 1 / Step 1 — `log_dir` 测试太弱,应直接验证路径拼接

**位置**:Task 1 / Step 1 / 行 108-124

**当前代码**:
```rust
#[test]
fn log_dir_uses_localappdata_when_present() {
    let dir = log_dir();
    assert!(dir.to_string_lossy().contains("kizemo"));
    assert!(dir.to_string_lossy().contains("focus-sync"));
}
```

**问题**:`log_dir()` 永远 append "kizemo" / "focus-sync",不管 LOCALAPPDATA 是什么,测试都过。这是 tautology。

**修复方案**(改用 env::set_var 模拟 + 完整路径断言):
```rust
#[test]
fn log_dir_uses_localappdata_when_set() {
    let original = std::env::var("LOCALAPPDATA").ok();
    // SAFETY: no other threads in test process at this point (cargo test 单线程 by default)
    unsafe {
        std::env::set_var("LOCALAPPDATA", r"C:\Users\test\AppData\Local");
    }
    let dir = log_dir();
    // SAFETY: ... restore
    unsafe {
        match original {
            Some(v) => std::env::set_var("LOCALAPPDATA", v),
            None => std::env::remove_var("LOCALAPPDATA"),
        }
    }
    assert_eq!(dir, std::path::PathBuf::from(r"C:\Users\test\AppData\Local\kizemo\focus-sync\logs"));
}

#[test]
fn log_dir_fallback_to_temp_when_localappdata_unset() {
    let original = std::env::var("LOCALAPPDATA").ok();
    unsafe { std::env::remove_var("LOCALAPPDATA"); }
    let dir = log_dir();
    unsafe {
        match original {
            Some(v) => std::env::set_var("LOCALAPPDATA", v),
            None => {}
        }
    }
    assert!(dir.starts_with(std::env::temp_dir()));
    assert!(dir.ends_with("focus-sync-logs"));
}
```

注意 `unsafe { std::env::set_var(...) }` 在 Rust 2024 edition 是 unsafe,但 Cargo.toml 是 2021 edition,**安全**。验证后记得 restore env。

### 2.3 [P1] Task 2 / Step 1 — 缺少 `basename("")` 边界测试

**位置**:Task 2 / Step 1(测试列表)

**应增加**:
```rust
#[test]
fn basename_empty_returns_empty() {
    assert_eq!(basename(""), "");
}
```

→ 实现上 `basename("")` 返回 `""`,会跳过写(filename empty check)。但没有显式测试覆盖此路径。

### 2.4 [P1] Task 2 / Step 4 — 建议直接删除 `dirname()`,不用 `#[allow(dead_code)]`

**位置**:Task 2 / Step 4 / 行 433-441

**Plan 当前**:保留 `dirname()` + `#[allow(dead_code)]`

**建议**:直接删除 `dirname()`。理由:
- 它本身就是 bug 来源(被误用为 basename)
- 没有其他 caller
- 留着就是 dead code,文档也得写 "kept for future use"(诱发未来再次误用)
- 删了减少 cognitive load

**修复方案**:Task 2 / Step 4 整段替换为:
```rust
// Step 4: DELETE dirname() entirely (was the source of Bug A — accidentally
// used as basename; no other callers; keeping it risks re-introducing the
// same misuse).
```
然后删除 Task 2 / Step 4 的"Add `#[allow(dead_code)]`"步骤,改为删函数。

### 2.5 [P2] Task 1 / Step 7 — `Box::leak` 注释应内联到代码

**位置**:Task 1 / Step 7 / 行 252

**Plan 注释**:
```
// Leak guard to keep file handle alive for process lifetime (~80 bytes; negligible).
Box::leak(Box::new(_guard));
```

**建议**:在 `main.rs` 里的实际代码位置加这个注释,后续 reader 不需要看 plan。注释内容:
```rust
// SAFETY / LIFETIME: leak the writer guard so the file handle stays open for
// the entire process lifetime. The guard is ~80 bytes; memory cost negligible.
// Drop would flush the buffer prematurely, losing late tail writes.
Box::leak(Box::new(_guard));
```

### 2.6 [P2] Task 4 / Step 3 — PE subsystem 验证命令不可靠,需 fallback

**位置**:Task 4 / Step 3 / 行 711-723

**Plan 当前**:用 `(Get-Item ...).VersionInfo.FileVersion`(只查 FileVersion 字符串)和 `sigcheck -n`(需 Sysinternals,不一定装)。

**问题**:`FileVersion` 不反映 subsystem。要查 subsystem 必须解析 PE header。

**修复方案**(3 个 fallback,任一可用):
```powershell
# 方法 1: dumpbin (MSVC) — 最可靠
dumpbin /headers "release/extension-installer/../resources/tools/focus-sync-sidecar.exe" | Select-String "subsystem"

# 方法 2: 自带 PowerShell 读 PE header
$bytes = [System.IO.File]::ReadAllBytes("release/extension-installer/focus-sync-sidecar.exe")
$sig = [System.Text.Encoding]::ASCII.GetString($bytes[0x3C..0x3C+4])
$peOffset = [BitConverter]::ToInt32($bytes, 0x3C)
# Optional header at peOffset + 0x18; subsystem at peOffset + 0x5C
$subsystem = [BitConverter]::ToUInt16($bytes, $peOffset + 0x5C)
# Subsystem 2 = Windows GUI, 3 = Console
"PE subsystem: $subsystem (2=GUI, 3=Console)"

# 方法 3: 用 Python (如已装)
python -c "
import struct
with open('release/extension-installer/focus-sync-sidecar.exe','rb') as f:
    data = f.read()
pe = struct.unpack_from('<I', data, 0x3C)[0]
sub = struct.unpack_from('<H', data, pe + 0x5C)[0]
print(f'PE subsystem: {sub} (2=GUI, 3=Console)')
"
```

推荐方法 2(纯 PowerShell,无依赖)。

### 2.7 [P2] Task 3 / Step 5 — PowerShell heredoc 在 bash 里脆弱

**位置**:Task 3 / Step 5 / 行 656-668

**Plan 当前**:bash 里 `@'...'@` heredoc 包 PowerShell 代码。

**问题**:`$null` `$j` `$existingVersion` 这些 `$` 在 bash heredoc 里是字面量(无 `$` 转义,所以 PowerShell 能正常解析),但 `@'...'@` 内部有 `\` 转义、PowerShell comment 等,bash 解析时可能咬住空格/换行,实际不可靠。

**修复方案**:把测试逻辑写到独立 .ps1 文件再 invoke:
```bash
# 1. 写测试文件
cat > /tmp/test-version-upgrade.ps1 << 'PSEOF'
$mockJson = '{"installedExtensions":{"kizemo.focus-sync":{"version":"0.2.0","enabled":true,"manifest":{"version":"0.2.0"}}}}'
$j = $mockJson | ConvertFrom-Json
$existingVersion = $j.installedExtensions.'kizemo.focus-sync'.version
if ($existingVersion -ne '0.3.0') {
    $j.installedExtensions.'kizemo.focus-sync'.version = '0.3.0'
    $j.installedExtensions.'kizemo.focus-sync'.manifest.version = '0.3.0'
    Write-Host "UPGRADE branch fired"
    Write-Host "new version: $($j.installedExtensions.'kizemo.focus-sync'.version)"
    Write-Host "new manifest.version: $($j.installedExtensions.'kizemo.focus-sync'.manifest.version)"
    Write-Host "enabled (preserved): $($j.installedExtensions.'kizemo.focus-sync'.enabled)"
}
PSEOF

# 2. 执行
powershell -NoProfile -File /tmp/test-version-upgrade.ps1
```

或更简单:这个测试逻辑上太简单,**直接跑 register.ps1 看输出就行**(在用户机器上真测,不浪费时间 mock)。

---

## 3. ✅ 已确认正确的决策

(确认 OK 的部分,Claude 不用改)

| 决策 | 评价 |
|---|---|
| Task 1 Step 6:`#![windows_subsystem = "windows"]` 放在第 1 行 | ✓ 正确(必须在 `//!` 注释前) |
| Task 1 Step 7:`tracing_appender::daily` 自动 rotate | ✓ 合理(简单够用,加 max_files 收益不大) |
| Task 2 Step 4:`dirname` 函数处理(不论删或 deprecate) | 见 §2.4,推荐删 |
| Task 2 Step 6:`!name.is_empty()` 简化旧判断 | ✓ 比旧 `basename != target_path && !basename.is_empty()` 更正确 |
| Task 2 Step 7:COM success 后 `return` | ✓ 核心修复,正确 |
| Task 2 Step 3:新 `basename()` 用 `rfind` + `idx+1..` 切片 | ✓ 标准实现,`&str` 返回借用零拷贝 |
| Task 3 Step 3:只改 `version` + `manifest.version`,其他保留 | ✓ 严格 round 19c 友好行为 |
| Task 4 Step 1-2:build 顺序 spike → build-with-sidecar → tauri build | ✓ 正确 |
| Task 4 Step 6-9:`git add` 6 文件 + 单 commit + tag v0.3.0 | ✓ 匹配用户"测试通过再 commit" |
| Review Focus 5 条:UNC / trailing slash / dialog class / Sigma FM normalize / RestartCount race | 见 §2.1 UNC 需修正 |
| Global Constraints:不删文件、Edit 而非 sed、每改 Rust 加单元测试 | ✓ 严谨 |

---

## 4. 风险与未覆盖点

| 风险 | 是否 plan 已处理 |
|---|---|
| windows_subsystem 后 dev 调试不便 | ❌ 未处理。建议加 `RUST_LOG=trace` 环境变量支持(已通过 EnvFilter 实现 ✓)— 实际 OK |
| log 文件无限增长 | ❌ 未处理。`tracing_appender::daily` 自动 rotate 但**不删除旧文件**。建议加 `Builder::default().max_log_files(7)` 或 uninstall 时清理(原 Mavis §2.2 改动 6)— **低优先级,可后续 round** |
| heartbeat println 改 log_event 在 `--service` 模式根本不 spawn | ⚠️ plan 改了,但 `--service` 模式 heartbeat thread 根本不会跑(line 168 `if !cli.service`)。改 spinner 模式 dev 行为,不直接影响 release。✓ |
| `extension/dist/index.js` 的 stale "Sidecar not reachable" 通知不重置 | ✓ 已确认 out of scope(Mavis §4.3) |
| `unregister-scheduled-task.ps1` 不清理 logs 目录 | ✓ 已确认 out of scope(原 Mavis §2.2 改动 6) |
| Task 1 修复后 `register-scheduled-task.ps1` 也要重新执行才能让新 binary 被 task 调用 | ⚠️ 没说。`KizemoFocusSync` task 注册时已经指向 `$INSTDIR\tools\focus-sync-sidecar.exe`,**installer 升级时这个文件被覆盖,Task Scheduler 下次启动会跑新 binary**。无需重注册 task。✓ |
| 修改 main.rs 后 23 个旧单元测试 + 8 个 health 测试是否仍 pass | ⚠️ 假设。Task 1 Step 9 跑全 test suite 会验证。✓ |

---

## 5. 推荐执行顺序调整

Claude 当前的 Task 1 → 2 → 3 → 4 顺序合理。

但建议在 **Task 2 Step 1(写 failing test for basename)** 之前先看 `uia_inject.rs` 当前测试块是否存在(plan 提到"Read uia_inject.rs test module (if any). If no test module exists, ADD one...")— 当前代码确实没有 `#[cfg(test)] mod tests`,需新建,plan Step 1 已覆盖。✓

另外建议在 **Task 4 / Step 4 之前** 增加一个验证项:
```bash
# Task 4 / Step 3.5: 验证 installer .exe 包含新的 spike binary
7z l release/Sigma-File-Manager-2.2.0-focus-17-setup.exe | grep -i focus-sync-sidecar
# 预期看到 focus-sync-sidecar.exe 行 + 大小匹配 target/release/spike.exe
```
避免旧 binary 被错误打包。

---

## 6. 给 Claude 的总结指令

### 必须改(2 处)

1. **Task 2 / Step 1**:`basename_drive_root_only_returns_empty` 改成 `assert_eq!(basename(r"C:\"), "C:");`
2. **Task 1 / Step 4**:保留 `LOG_LOCK`(原 `STDOUT_LOCK` 改名),在 `log_event` 内 file write 前加 `let _guard = LOG_LOCK.lock()...;`

### 建议改(7 处)

3. **Task 2 / Review Focus**:UNC 路径从 "NOT SUPPORTED" 改为 "tested, supported",并加 `basename_unc_supported` 测试
4. **Task 1 / Step 1**:`log_dir` 测试用 env::set_var 显式 mock,断言完整路径
5. **Task 2 / Step 1**:加 `basename_empty_returns_empty` 测试
6. **Task 2 / Step 4**:删除 `dirname()`(不用 `#[allow(dead_code)]`)
7. **Task 1 / Step 7**:`Box::leak` 注释内联到 main.rs 代码
8. **Task 4 / Step 3**:PE subsystem 验证用 PowerShell 读 PE header(方法 2)
9. **Task 3 / Step 5**:PowerShell 测试改用独立 .ps1 文件,避免 bash heredoc 嵌套

### 不必改

其余 §3 已确认 OK,按 plan 执行即可。

---

## 7. 给用户(转交前的注意事项)

1. **2 处必修**(§6 #1 #2)必须在 Claude Code 执行 Task 1 / Task 2 之前就传达 — 否则 Task 1 Step 4 会引入并发 bug,Task 2 Step 1 的测试会因为断言错误一开始就走 fail 路径
2. **1 处 review focus 错误**(§2.1)让 Claude 误以为 UNC 不支持,可能影响后续 plan(避免在 UNC 文档里写 "不支持")
3. **其余改进**是 nice-to-have,可让 Claude 自行决定是否采纳
4. **我(Mavis)不会执行修改**,只提意见。如果 Claude Code 收到这份 review 后开始执行,请用户在旁监督 Task 1 Step 4(确认 LOG_LOCK 被保留)和 Task 2 Step 1(确认测试断言修正)

**用户请确认**:这份 review 是否需要补充 / 调整?如果 OK,请转发给 Claude Code 修订 plan,然后再开始实施。
