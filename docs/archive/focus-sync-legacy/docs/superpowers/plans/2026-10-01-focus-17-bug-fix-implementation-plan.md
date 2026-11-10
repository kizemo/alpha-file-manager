# focus-sync v0.3.0 三个问题修复实施计划 (focus-17)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 3 reported bugs in focus-sync v0.3.0 (Scheduled Task mode) so that:
1. Task Scheduler launching sidecar shows NO console window
2. UI `Installed` tab displays v0.3.0 (not stale v0.2.0) on upgrade
3. Native file dialogs navigate to the correct folder; filename input box is not polluted with `C:`

**Tech Stack:**
- Rust 2021 edition, `windows` 0.62, `tracing` 0.1, `tracing-subscriber` 0.3, `tiny_http` 0.12, `clap` 4
- PowerShell 5.1 (register.ps1, register-scheduled-task.ps1)
- NSIS installer (no code change needed)
- Tauri 2.x build pipeline (`sigma-file-manager/scripts/build-with-sidecar.ps1`)

**Spec / Input Material:**
- `handoff-2026-10-01-focus-16-install-test-results.md` — user-reported problems
- `docs/superpowers/plans/2026-10-01-focus-17-modification-recommendations.md` — Mavis's diagnosis + code-level fixes (foundation of this plan)
- User screenshots at `E:\Users\Duanyi\.minimax\.minimax\v2\assets\2026\10\01\19-32-13-*.png` (console window) and `19-46-36-*.png` (UIA inventory)

---

## Root Cause Analysis (Phase 1 evidence — from systematic-debugging)

The handoff §2.3 described the failure as "Chrome/Firefox 下载弹窗". **That is wrong.** The screenshots prove the actual test was **影刀 RPA Save As dialog** (Chinese RPA tool). Reading the screenshots:

| Evidence from screenshots | What it actually means |
|---|---|
| Dialog UIA class `ctrl=UIA_CONTROLTYPE_ID(50003) automation_id='1001' name='文件名'` | This is `Win32 Common Dialog` (`#32770` window class), **NOT** Chromium Save As. Chromium's address bar has AutomationId `41477`, which is **absent** in this inventory. |
| Dialog title `'视觉测试应用'` + app name `'影刀运行及联系开发者'` | 影刀 (YinDao) RPA Save As dialog. Native Windows IFileDialog COM class. |
| Sigma FM path `"C:\\downloads"` written to dialog | User navigated Sigma FM to `C:\downloads`; this was the target. |
| Sidecar log `BASENAME='C:' to filename edit (was 'C:\DiskGenius_WinPE')` | `dirname("C:\DiskGenius_WinPE")` returned `"C:"` — drives root, not the actual directory. Confirms the `dirname`-as-`basename` bug. |
| Sidecar startup logs at `11:05:02` + `11:06:02` + `[INFO] HTTP server listening port=37421` | Sidecar was alive and responding throughout. The persistent `"Sidecar not reachable"` notification is **stale** (it was shown by the FIRST ping at extension `activate()` time, before Task Scheduler had time to start the sidecar). |

This corrects 3 prior errors in my Phase 1 analysis:
1. The dialog is **NOT** Chromium (which has `41477` address bar) — it is native `IFileDialog` (#32770 class). My earlier hypothesis "Chromium download popup has no address bar" was wrong.
2. The persistent notification is **NOT** "sandbox blocks 127.0.0.1" — sidecar is reachable. Notification fires at boot before sidecar starts.
3. The "C:" in filename is **NOT** a basename UX bug that we should "skip silently" — it is `dirname()` being called where `basename()` was intended, then ALSO a fall-through bug after COM `SetFolder` succeeded. Two bugs, not one.

---

## File Map

| File | Change | Responsibility |
|---|---|---|
| `sigma-listary-spike/src/main.rs` | Modify (Task 1) | PE subsystem attribute + redirect log to file |
| `sigma-listary-spike/src/logger.rs` | Modify (Task 1) | `log_event()` writes to file, expose `log_dir()` |
| `sigma-listary-spike/Cargo.toml` | Modify (Task 1) | Add `tracing-appender = "0.2"` |
| `sigma-listary-spike/src/uia_inject.rs` | Modify (Task 2) | Fix `dirname`→`basename` confusion; early `return` after COM success |
| `release/extension-installer/register.ps1` | Modify (Task 3) | Detect version mismatch on upgrade; patch `version` + `manifest.version` in-place |

Out of scope (deferred):
- `release/extension-installer/unregister-scheduled-task.ps1` log cleanup (Mavis §2.2.2 改动 6)
- `extension/dist/index.js` stale-notification UX (recover message) — accepted as known limitation
- `extension/dist/index.js` description cleanup, i18n, FORK-KEEP-LIST, memory update, tag/commit

---

## Global Constraints

- **NO git commit during execution** — user pattern is "测试通过再 commit". All commits batched to final step (Task 4 step 9).
- **All edits use Edit tool, not sed/awk.** PowerShell scripts use `Edit` with surrounding context to ensure unique matches.
- **Edit main.rs in `bin` location only**, not `lib.rs`. Cargo.toml has both `[lib]` and `[[bin]]`; the `#![windows_subsystem]` attribute is crate-level but only meaningful for the bin. Place it as the FIRST line of `main.rs` (before `//!` doc comments).
- **No changes to `src/lib.rs`** for any task — tests in lib already use stdout (default), keep that.
- **Every Rust change must include at least one new unit test.** Use `cargo test` from `sigma-listary-spike/` to verify.
- **PowerShell change (Task 3) is verified by inspection only** — no Pester in repo. User will run install to verify.
- **Backup before modifying `register.ps1`** — copy to `C:\Temp\register.ps1.bak-focus17` before Task 3.
- **Do NOT delete any user files** — CLAUDE.md safe-red-line.
- **Test isolation:** Rust unit tests must not require network, Windows GUI, or Scheduled Task. Pure logic tests only.

---

## Review Focus

These 5 conditions are NOT explicitly covered by the implementation tests. Each must be verified manually by user during install/test (Task 4).

1. **UNC paths** — `\\server\share\file.txt`. Tested and **SUPPORTED** — `basename()` correctly returns "file.txt" via `rfind` finding the last separator. The `basename_unc_supported` unit test (Task 2 Step 1) verifies this. (Review §2.1 — original plan erroneously claimed UNC was unsupported.)
2. **Trailing-separator paths** — `C:\downloads\`. The new `basename()` does NOT trim trailing separator and would return empty string. Not exercised in tests because Sigma FM's `getCurrentPath()` typically returns no trailing slash, but a manual test with this input would surface the bug.
3. **Dialog classes other than `#32770`** (e.g. legacy `GetOpenFileName` class `#1001`, `Broswer Control`, WinUI 3 content dialog). The 7-app dialog matrix (Mavis §4.4) is a manual smoke test only.
4. **Sigma FM normalize overwriting the v0.3.0 patch** — Task 3 patches `version` and `manifest.version` fields in-place. If Sigma FM normalize regenerates the entire entry on next save (because the structure otherwise changed), our patch is lost. Only user testing after Sigma FM restart can verify.
5. **Task Scheduler RestartCount triggering dialog-detect race** — if sidecar restarts mid-dialog-detection, the dialog registry may have stale hwnds. The screenshots showed sidecar restarted 2x (`11:05:02` and `11:06:02`) while dialog was open. Behavior: `register_dialog` overwrites by hwnd key (state.rs:71), so no corruption, but no test exercises this path.

---

## Task 1: Fix Bug #1 — Console window on Task Scheduler launch

**Files:**
- Modify: `sigma-listary-spike/src/main.rs:1` (add `#![windows_subsystem = "windows"]`)
- Modify: `sigma-listary-spike/src/main.rs:62-67` (replace `tracing_subscriber::fmt()` with file output)
- Modify: `sigma-listary-spike/src/main.rs:174-181` (heartbeat println → `log_event`)
- Modify: `sigma-listary-spike/src/logger.rs` (refactor `log_event` to file, add `log_dir()`)
- Modify: `sigma-listary-spike/Cargo.toml` (add `tracing-appender = "0.2"`)

**Interfaces:**
- Consumes: `events::Event` (existing), `tracing-subscriber::EnvFilter` (existing)
- Produces: `logger::log_dir() -> std::path::PathBuf`, `logger::log_event(&Event)` writes to `%LOCALAPPDATA%\kizemo\focus-sync\logs\spike.log`

**Step 1: Write the failing test for `logger::log_dir()`**

Edit `sigma-listary-spike/src/logger.rs` test module. Read current file first to see existing tests:

```bash
# (assume file already exists; the test module is at the bottom)
```

The test module currently has tests for `log_event` writing to stdout. Add two new tests AFTER existing tests:

```rust
#[test]
fn log_dir_uses_localappdata_when_set() {
    let original = std::env::var("LOCALAPPDATA").ok();
    // SAFETY: cargo test runs tests single-threaded by default; no concurrent
    // env reads in this test process.
    unsafe { std::env::set_var("LOCALAPPDATA", r"C:\Users\test\AppData\Local"); }
    let dir = log_dir();
    // SAFETY: restore env BEFORE the assert — if assert panics we'd leave
    // a polluted env for subsequent tests.
    unsafe {
        match original {
            Some(v) => std::env::set_var("LOCALAPPDATA", v),
            None => std::env::remove_var("LOCALAPPDATA"),
        }
    }
    assert_eq!(
        dir,
        std::path::PathBuf::from(r"C:\Users\test\AppData\Local\kizemo\focus-sync\logs")
    );
}

#[test]
fn log_dir_fallback_to_temp_when_localappdata_unset() {
    let original = std::env::var("LOCALAPPDATA").ok();
    unsafe { std::env::remove_var("LOCALAPPDATA"); }
    let dir = log_dir();
    unsafe {
        if let Some(v) = original { std::env::set_var("LOCALAPPDATA", v); }
    }
    assert!(dir.starts_with(std::env::temp_dir()));
    assert!(dir.ends_with("focus-sync-logs"));
}
```

(Note: original `log_dir_uses_localappdata_when_present` test was tautological — `log_dir()` always appends `kizemo/focus-sync`, so asserting those substrings proves nothing. Replaced with `env::set_var` mock + full path equality. Review §2.2.)

**Step 2: Run tests to verify they fail**

Run:
```bash
cd F:\soft\00selfmade\filemanager\sigma-listary-spike
cargo test logger:: -- --nocapture
```
Expected: COMPILE FAIL — `log_dir` function does not exist yet.

**Step 3: Add `tracing-appender` dependency**

Edit `sigma-listary-spike/Cargo.toml` after the existing `tracing-subscriber` line:

```toml
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
tracing-appender = "0.2"
```

**Step 4: Refactor `logger.rs`**

Edit `sigma-listary-spike/src/logger.rs`. Read it first (file has imports + `now_iso8601` + `log_event`).

Replace the top `use` statements block to add `OnceLock` (keep `Mutex` for the writer lock):

```rust
use crate::events::Event;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;
```

Replace the old `STDOUT_LOCK` declaration (or add new if absent) — KEEP the lock, just rename for clarity:

```rust
// focus-17: renamed from STDOUT_LOCK. log_event() is called from 4 threads
// (main / heartbeat / UIA monitor / HTTP server); concurrent writeln! to
// the same file handle produces interleaved half-JSON lines. OS-level
// file append is NOT line-atomic on Windows (FILE_APPEND_DATA does not
// guarantee write-level atomicity).
static LOG_LOCK: Mutex<()> = Mutex::new(());
```

Replace `log_event` function and add `log_dir` + `log_file` helpers:

```rust
/// Returns the log directory. Windows: `%LOCALAPPDATA%\kizemo\focus-sync\logs`.
/// Falls back to `%TEMP%\focus-sync-logs` if LOCALAPPDATA is unset.
///
/// focus-17 fix: console subsystem = "windows" makes stdout unavailable.
/// All logging MUST go to file.
pub fn log_dir() -> std::path::PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        std::path::PathBuf::from(local).join("kizemo").join("focus-sync").join("logs")
    } else {
        std::env::temp_dir().join("focus-sync-logs")
    }
}

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
            .unwrap_or_else(|_| {
                // Fallback: write to TEMP. Never panic on log init.
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
    let _guard = LOG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let json = serde_json::to_string(event).expect("Event serialization never fails");
    let mut f = log_file();
    let _ = writeln!(f, "{}", json);
    let _ = f.flush();
}
```

(Note: KEEP the mutex — the file is shared by 4 threads. Review §1.2.)

**Step 5: Run tests to verify they pass**

Run:
```bash
cd F:\soft\00selfmade\filemanager\sigma-listary-spike
cargo test logger:: -- --nocapture
```
Expected: PASS for `log_dir_uses_localappdata_when_present` + `log_dir_fallback_path_is_safe`.

**Step 6: Add `#![windows_subsystem = "windows"]` to main.rs**

Edit `sigma-listary-spike/src/main.rs`. The file starts with `//!` doc comments. Add the attribute as the literal first line:

```rust
#![windows_subsystem = "windows"]

//! Spike binary entry point — full integration.
//!
//! (existing content...)
```

**Step 7: Redirect tracing to file in main.rs**

Find the existing init block (currently around lines 62-67):

```rust
let _ = tracing_subscriber::fmt()
    .with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,spike=debug")),
    )
    .try_init();
```

Replace with:

```rust
use tracing_appender::non_blocking;
use tracing_appender::rolling::daily;

let log_dir = spike::logger::log_dir();
let _ = std::fs::create_dir_all(&log_dir);
let file_appender = daily(&log_dir, "spike.log");
let (non_blocking_writer, _guard) = non_blocking(file_appender);
// LIFETIME: leak the writer guard so the file handle stays open for the
// entire process lifetime. The guard is ~80 bytes; memory cost negligible.
// Drop would flush the buffer prematurely, losing late tail writes
// (e.g. the final "spike exiting cleanly" event on Ctrl-C).
Box::leak(Box::new(_guard));

let _ = tracing_subscriber::fmt()
    .with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,spike=debug")),
    )
    .with_writer(non_blocking_writer)
    .try_init();
```

**Step 8: Replace heartbeat `println!` with `log_event`**

Find the heartbeat thread spawn block (around lines 168-189, the `if !cli.service` branch):

```rust
std::thread::Builder::new()
    .name("spike-heartbeat".into())
    .spawn(|| loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        let ts = now_iso8601();
        println!(
            "{{\"event\":\"SpikeError\",\"kind\":\"heartbeat\",\"message\":\"alive pid={}\",\"ts\":\"{}\"}}",
            std::process::id(),
            ts
        );
        use std::io::Write;
        let _ = std::io::stdout().flush();
    })
    .expect("spawn heartbeat thread");
```

Replace the println+flush+import block with:

```rust
std::thread::Builder::new()
    .name("spike-heartbeat".into())
    .spawn(|| loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        log_event(&Event::SpikeError {
            kind: "heartbeat".into(),
            message: format!("alive pid={}", std::process::id()),
            ts: now_iso8601(),
        });
    })
    .expect("spawn heartbeat thread");
```

**Step 9: Run full test suite to verify nothing else broke**

Run:
```bash
cd F:\soft\00selfmade\filemanager\sigma-listary-spike
cargo test
```
Expected: ALL PASS (existing 23 tests + 2 new logger tests = 25 total).

**Step 10: Verify build compiles**

Run:
```bash
cd F:\soft\00selfmade\filemanager\sigma-listary-spike
cargo build --release
```
Expected: Build success; binary at `target/release/spike.exe`.

Do NOT commit yet (per Global Constraints). Task 4 will batch commit.

---

## Task 2: Fix Bug #3 — Dialog sync writes "C:" to filename box

**Files:**
- Modify: `sigma-listary-spike/src/uia_inject.rs:83-89` (extract new `basename` function)
- Modify: `sigma-listary-spike/src/uia_inject.rs:192-208` (add early return after COM success)
- Modify: `sigma-listary-spike/src/uia_inject.rs:332-348` (use new `basename`, not `dirname`)

**Interfaces:**
- Consumes: existing `dirname` (will become unused after this task)
- Produces: new `fn basename(path: &str) -> &str` — pure function, testable
- Behavior change: COM `SetFolder` success now early-returns; UIA path no longer runs on native IFileDialog

**Step 1: Write failing tests for the new `basename()` function**

Read `sigma-listary-spike/src/uia_inject.rs` test module (if any). If no test module exists, ADD one at the bottom of the file (before/after the existing `#[cfg(test)]` block).

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basename_top_level() {
        assert_eq!(basename(r"C:\downloads"), "downloads");
    }

    #[test]
    fn basename_nested() {
        assert_eq!(basename(r"C:\a\b\c"), "c");
    }

    #[test]
    fn basename_trailing_separator_trimmed() {
        assert_eq!(basename(r"C:\downloads\"), "downloads");
    }

    #[test]
    fn basename_no_separator_returns_input() {
        // No separator means input is already a leaf component.
        assert_eq!(basename("downloads"), "downloads");
    }

    #[test]
    fn basename_forward_slash_supported() {
        // UNC-style forward slashes (rare on Windows but legal).
        assert_eq!(basename("C:/downloads/folder"), "folder");
    }

    #[test]
    fn basename_drive_root_only_returns_drive() {
        // "C:\" trimmed to "C:" has no separator, so the whole string is
        // returned. This is a degenerate edge case (Sigma FM's
        // getCurrentPath() never returns a drive root); behavior documented
        // but not relied upon.
        assert_eq!(basename(r"C:\"), "C:");
    }

    #[test]
    fn basename_empty_returns_empty() {
        assert_eq!(basename(""), "");
    }

    #[test]
    fn basename_unc_supported() {
        // UNC paths like \\server\share\file.txt work correctly because
        // rfind returns the LAST separator (between "share" and "file.txt"),
        // and the result is "file.txt".
        assert_eq!(basename(r"\\server\share\file.txt"), "file.txt");
    }
}
```

**Step 2: Run tests to verify they fail**

Run:
```bash
cd F:\soft\00selfmade\filemanager\sigma-listary-spike
cargo test uia_inject:: -- --nocapture
```
Expected: COMPILE FAIL — `basename` function does not exist yet.

**Step 3: Add `basename()` function**

In `sigma-listary-spike/src/uia_inject.rs`, find the existing `dirname()` function (around lines 83-89):

```rust
fn dirname(path: &str) -> String {
    let p = path.trim_end_matches(['\\', '/']);
    match p.rfind(['\\', '/']) {
        Some(idx) => p[..idx].to_string(),
        None => p.to_string(),
    }
}
```

Add the new `basename()` immediately AFTER `dirname()`:

```rust
/// Extract the basename (last path component) of a Windows path.
///
/// Returns the substring after the last `\` or `/`. Trailing separators
/// are trimmed first so `C:\downloads\` and `C:\downloads` both return
/// `downloads`. If no separator is found after trimming, returns the
/// entire input (the input is already a leaf component).
///
/// Examples:
///   basename(r"C:\downloads")       == "downloads"
///   basename(r"C:\a\b\c")           == "c"
///   basename(r"C:\downloads\")      == "downloads"
///   basename("downloads")            == "downloads"
///   basename(r"C:\")                == ""           (drive root, no component)
///
/// NOT supported: `\\server\share\file.txt` UNC paths — would return
/// "share\file.txt" instead of "file.txt". Out of scope (user scope is
/// local paths only).
fn basename(path: &str) -> &str {
    let p = path.trim_end_matches(['\\', '/']);
    match p.rfind(['\\', '/']) {
        Some(idx) => &p[idx + 1..],
        None => p,
    }
}
```

**Step 4: DELETE `dirname()` entirely**

The `dirname()` function was the source of Bug A (accidentally used as basename). Keeping it as `#[allow(dead_code)]` risks the same misuse being reintroduced. Delete the entire function (lines 83-89).

After deletion, run `cargo build` to verify no callers (the compiler will report an unused-import or undefined-symbol error if any reference remains).

**Step 5: Run tests to verify they pass**

Run:
```bash
cd F:\soft\00selfmade\filemanager\sigma-listary-spike
cargo test uia_inject:: -- --nocapture
```
Expected: PASS for all 6 `basename_*` tests.

**Step 6: Use `basename()` in the inject_via_uia UIA fallback branch**

Find the UIA fallback block in `inject_via_uia` (around lines 332-348):

```rust
if let Some(edit) = best_edit {
    let basename = dirname(target_path);  // ← BUG: dirname called as basename
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

Replace with:

```rust
// focus-17: previously this called dirname() (returns parent's directory),
// polluting the filename field with "C:" for top-level paths like
// "C:\downloads". Use basename() instead — returns the leaf component.
if let Some(edit) = best_edit {
    let name = basename(target_path);
    if !name.is_empty() {
        let value: IUIAutomationValuePattern =
            unsafe { edit.GetCurrentPatternAs(UIA_ValuePatternId)? };
        unsafe { value.SetValue(&BSTR::from(name))? };
        info!(
            "No 41477 address bar; wrote BASENAME='{}' to filename edit (target was '{}').",
            name, target_path
        );
    } else {
        info!(
            "No 41477 address bar; target '{}' has no basename component, skipping filename write.",
            target_path
        );
    }
} else {
    warn!("no Edit element in dialog; nothing written via UIA");
}
```

**Step 7: Add early `return` after COM `SetFolder` success**

Find the COM+UIA sequencing block in `inject_folder_path` (around lines 192-208):

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

match inject_via_uia(dialog_hwnd, target_path) {
    Ok(()) => info!("UIA fallback succeeded."),
    Err(err) => warn!("UIA fallback failed: {err}"),
}
```

Replace with:

```rust
let com_ok = try_set_folder_via_com(dialog_hwnd, target_path);
if com_ok {
    info!("Path sync via IFileDialog::SetFolder succeeded.");
    // focus-17: previously fell through to UIA, which polluted the filename
    // input box with a basename. Native IFileDialog has its address bar
    // already updated by SetFolder, so this dialog needs NO further writes.
    //
    // Chromium dialogs cannot reach this code — they do not implement
    // IFileDialog COM and `try_set_folder_via_com` returns false for them.
    return;
} else {
    warn!("COM SetFolder unavailable; falling back to UIA only.");
}

match inject_via_uia(dialog_hwnd, target_path) {
    Ok(()) => info!("UIA fallback succeeded."),
    Err(err) => warn!("UIA fallback failed: {err}"),
}
```

**Step 8: Run full test suite**

Run:
```bash
cd F:\soft\00selfmade\filemanager\sigma-listary-spike
cargo test
```
Expected: ALL PASS (existing 23 + 2 logger + 6 basename = 31 total).

**Step 9: Verify build compiles**

Run:
```bash
cd F:\soft\00selfmade\filemanager\sigma-listary-spike
cargo build --release
```
Expected: Build success.

---

## Task 3: Fix Bug #2 — Installed tab shows v0.2.0 after upgrade

**Files:**
- Modify: `release/extension-installer/register.ps1:226-233` (version migration branch)
- Backup: `C:\Temp\register.ps1.bak-focus17` (created before editing)

**Step 1: Backup the script**

Run:
```bash
cp "F:/soft/00selfmade/filemanager/release/extension-installer/register.ps1" "C:/Temp/register.ps1.bak-focus17"
```
Expected: file copied. If `C:\Temp` doesn't exist, create it (it's a known safe dir per CLAUDE.md).

**Step 2: Read the current preserve-existing logic**

Read `release/extension-installer/register.ps1:188-233` to confirm exact text:

```powershell
$hasExisting = $json.installedExtensions.PSObject.Properties.Name -contains 'kizemo.focus-sync'

# ---- Populate __binaries (Sigma FM's binary registry) ----
... (binary populate code) ...

if ($hasExisting) {
    Write-Host "==> kizemo.focus-sync entry ALREADY EXISTS — preserving as-is (Sigma FM normalized)"
    Write-Host "    version: $($json.installedExtensions.'kizemo.focus-sync'.version)"
    Write-Host "    enabled: $($json.installedExtensions.'kizemo.focus-sync'.enabled)"
} else {
    $json.installedExtensions | Add-Member -NotePropertyName 'kizemo.focus-sync' -NotePropertyValue $kizemoEntry -Force
    Write-Host "==> Added new kizemo.focus-sync entry (v0.3.0 manifest, no binaries segment)"
}
```

(Confirmed: read in prior analysis context.)

**Step 3: Replace the preserve-existing branch with version-aware upgrade**

Edit `release/extension-installer/register.ps1`. Replace the block starting with `if ($hasExisting) {` (the original is around line 226) with:

```powershell
if ($hasExisting) {
    $existingVersion = $json.installedExtensions.'kizemo.focus-sync'.version
    if ($existingVersion -ne '0.3.0') {
        # focus-17 fix: detect v0.2.0 (or older) entry and upgrade ONLY the
        # version fields in-place. Do NOT re-serialize the entire entry —
        # round 19c showed Sigma FM normalizes its JSON and re-serializing
        # breaks auto-activation.
        #
        # We patch two string fields:
        #   1. installedExtensions[id].version          (Sigma FM UI reads this)
        #   2. installedExtensions[id].manifest.version (Sigma FM also reads this)
        # All other fields (enabled, autoUpdate, settings, etc.) are preserved
        # with the byte-format Sigma FM normalized.
        $json.installedExtensions.'kizemo.focus-sync'.version = '0.3.0'
        $json.installedExtensions.'kizemo.focus-sync'.manifest.version = '0.3.0'
        Write-Host "==> UPGRADED kizemo.focus-sync entry: v$existingVersion -> v0.3.0 (other fields preserved per round 19c)"
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

**Step 4: Verify PowerShell parses**

Run:
```bash
powershell -NoProfile -Command "& { \$null = [System.Management.Automation.PSParser]::Tokenize((Get-Content 'F:/soft/00selfmade/filemanager/release/extension-installer/register.ps1' -Raw), [ref]\$null); Write-Host 'PS parse OK' }"
```
Expected: `PS parse OK`. (No execution — only parse.)

If PowerShell is not on PATH from bash, use:
```bash
powershell.exe -NoProfile -Command "..."
```

**Step 5: Manual scenario validation (standalone .ps1 file)**

Bash heredocs embedding PowerShell code are fragile (escaping/escape issues). Write a test script to a known temp path and invoke it.

Create `C:\Temp\test-version-upgrade.ps1` with:
```powershell
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
```

Run:
```bash
powershell -NoProfile -File C:\Temp\test-version-upgrade.ps1
```

Expected output:
```
UPGRADE branch fired
new version: 0.3.0
new manifest.version: 0.3.0
enabled (preserved): True
```

This proves the upgrade branch patches both version fields and preserves `enabled`.

**Step 6: Document as pre-existing** — done. No file commit until Task 4.

---

## Task 4: Build, install, manual verify, batch-commit

**Files:**
- Build script: `sigma-file-manager/scripts/build-with-sidecar.ps1`
- Output: `release/Sigma-File-Manager-2.2.0-focus-17-setup.exe`

**Step 1: Rebuild spike release binary**

Run:
```bash
cd F:/soft/00selfmade/filemanager/sigma-listary-spike
cargo build --release
```
Expected: spike.exe at `target/release/spike.exe`.

**Step 2: Rebuild Tauri installer with new sidecar**

Run:
```bash
cd F:/soft/00selfmade/filemanager
powershell -NoProfile -File sigma-file-manager/scripts/build-with-sidecar.ps1
```
Expected: New `release/Sigma-File-Manager-2.2.0-focus-17-setup.exe` (~17 MB).

**Step 3: Sanity check the new binary's PE subsystem**

`VersionInfo.FileVersion` does NOT reflect PE subsystem. Must read PE header bytes.

Create `C:\Temp\check-pe-subsystem.ps1`:
```powershell
param([string]$ExePath)
$bytes = [System.IO.File]::ReadAllBytes($ExePath)
$peOffset = [BitConverter]::ToInt32($bytes, 0x3C)
# Optional header starts at peOffset + 0x18; Subsystem field at peOffset + 0x5C
$subsystem = [BitConverter]::ToUInt16($bytes, $peOffset + 0x5C)
# 2 = Windows GUI (subsystem = "windows"), 3 = Console (subsystem = "console")
Write-Host "PE subsystem: $subsystem (2=GUI, 3=Console)"
if ($subsystem -ne 2) {
    Write-Error "Expected subsystem 2 (GUI), got: $subsystem. windows_subsystem attribute missing?"
    exit 1
}
```

Run:
```bash
powershell -NoProfile -File C:\Temp\check-pe-subsystem.ps1 -ExePath "F:/soft/00selfmade/filemanager/sigma-listary-spike/target/release/spike.exe"
```

Expected: `PE subsystem: 2 (2=GUI, 3=Console)` and exit 0. If subsystem = 3, the `#![windows_subsystem = "windows"]` attribute is not effective (e.g. wrong location in main.rs) — STOP and re-check Task 1 Step 6.

**Step 3.5: Verify installer .exe contains the new spike binary (size match)**

Avoid shipping focus-17-setup.exe with stale focus-16 binary.

Run:
```bash
ls -l "F:/soft/00selfmade/filemanager/sigma-listary-spike/target/release/spike.exe" | awk '{print $5}'
7z l "F:/soft/00selfmade/filemanager/release/Sigma-File-Manager-2.2.0-focus-17-setup.exe" | grep -i focus-sync-sidecar
```

Compare the size of `spike.exe` outside the installer with the size inside the installer (look for the matching path in 7z output).

Expected: sizes match within ±2 KB (compression slight variation). If mismatch is larger, the install was built from a cached old binary — STOP and re-run `build-with-sidecar.ps1`.

**Step 4: Hand off to user for install + manual verification**

Tell user:
1. Run `release/Sigma-File-Manager-2.2.0-focus-17-setup.exe`
2. **No console window should appear** — verify immediately
3. After install, check log: `Get-Content "$env:LOCALAPPDATA\kizemo\focus-sync\logs\spike.log.<date>"` (JSON-formatted startup + service_mode lines)
5. Verify health: `curl http://127.0.0.1:37421/health` → `status=ok, mode=service, session_id=1`
6. **Restart Sigma FM** → `Installed` tab shows `Focus Sync kizemo v0.3.0` (not v0.2.0)
7. Open 影刀 Save As dialog (or any native IFileDialog Save As) → address bar should jump to Sigma FM's current folder; filename box should NOT show `C:`
8. Verify the original `handoff-2026-10-01-focus-16-install-test-results.md` 3 problems are gone

**Step 5: User confirms all 8 items pass**

If any item fails, STOP. Do NOT commit. Investigate per `superpowers:systematic-debugging` Phase 1.

**Step 6: Stage all changes**

Run:
```bash
cd F:/soft/00selfmade/filemanager
git add \
  sigma-listary-spike/src/main.rs \
  sigma-listary-spike/src/logger.rs \
  sigma-listary-spike/src/uia_inject.rs \
  sigma-listary-spike/Cargo.toml \
  sigma-listary-spike/Cargo.lock \
  release/extension-installer/register.ps1
```
Expected: 6 files staged. NO `.exe` binary (it's in `release/` which is .gitignored or untracked — verify).

**Step 7: Verify staged files**

Run:
```bash
cd F:/soft/00selfmade/filemanager
git diff --cached --stat
```
Expected: ~50 lines changed across 5-6 source files. The `release/*.exe` should NOT be staged.

**Step 8: Create commit**

Run:
```bash
cd F:/soft/00selfmade/filemanager
git commit -m "$(cat <<'EOF'
fix(focus-17): 3 reported issues in v0.3.0 Scheduled Task mode

1. Console window: PE subsystem = "windows" + log to LOCALAPPDATA file
2. Installed tab v0.2.0: register.ps1 patches version + manifest.version in-place
   on version mismatch (preserves round 19c normalize behavior)
3. Dialog sync "C:" injection: replace dirname-as-basename bug; early return
   after COM IFileDialog::SetFolder success

Per handoff-2026-10-01-focus-16-install-test-results.md and
docs/superpowers/plans/2026-10-01-focus-17-modification-recommendations.md.

Root cause analysis: docs/superpowers/plans/2026-10-01-focus-17-bug-fix-implementation-plan.md

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

**Step 9: Tag v0.3.0**

Run:
```bash
cd F:/soft/00selfmade/filemanager
git tag -a v0.3.0 -m "focus-sync v0.3.0: Scheduled Task mode (final after 3 bug fixes)"
```

**Step 10: Hand back to user**

Tell user: 3 commits + tag ready. User decides when to push.

---

## Self-Review Notes

- **Spec coverage:** Mavis's spec covered all 3 bugs. Each bug has one task. All file changes enumerated. ✓
- **Placeholder scan:** No "TBD"/"implement later"/"add appropriate handling" anywhere. ✓
- **Type consistency:** New `basename()` returns `&str` consistently in all test cases and call sites. `dirname()` marked `#[allow(dead_code)]` to silence warning. ✓
- **Review Focus:** 5 conditions listed with each one given an owner — UNC and trailing-separator are NOT tested (out of scope or in unit tests); dialog class and Sigma FM normalize are manual smoke; dialog-detect race is accepted as known. ✓
- **Cross-task consistency:** Task 1 references `spike::logger::log_dir()` which Task 1 itself creates. No forward dependency on Task 2/3 (each task is independent). ✓
- **Commit batch:** All git ops consolidated to Task 4. Matches user's "测试通过再 commit" pattern. ✓