# Listary-Style Global Focus Sync — Spike Implementation Plan (Port Route)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a standalone Windows binary (`spike.exe`) that validates whether UI Automation can detect / read / write external file dialog paths in 7 target apps. Coverage ≥ 70% (5/7 apps × 3 capabilities) → enter Sigma integration phase. Coverage < 70% → README limitation note (no DLL injection, no commitment to full implementation).

**Architecture:** Single Rust binary in meta-repo subdirectory `sigma-listary-spike/`. **Port route** (this version): 3 files (~705 LoC) lifted from `inaku-Gyan/PathWrap` (MIT) + `QwenLM/qwen-code` cua-driver `fg_bypass.rs` (Apache-2.0), adapted to `windows` 0.62 + tokio runtime. Original self-written approach (14 tasks) was reset on 2026-09-27 after 7-app matrix verification proved infeasible — see `handoff-2026-09-27-focus-activation-uia-port.md` §I for evidence.

**Tech Stack:**
- Rust 1.75+
- `windows` crate **0.62** (PathWrap already uses 0.62 — QwenLM is 0.58, must upgrade signatures)
- `tokio` 1.x (features: `net`, `io-util`, `macros`, `rt-multi-thread`, `sync`, `time`)
- `serde` 1.x + `serde_json` 1.x
- `anyhow` 1.x, `tracing` 0.1 + `tracing-subscriber` 0.3 (json feature)
- Target platform: Windows 11 22H2+

**Spec backing:** `docs/superpowers/specs/2026-09-27-listary-focus-sync-design.md`
**Source research:** `.other/doc/00-research-overview.md` + 4 sub-reports

---

## Route Change Context (2026-09-27)

| Aspect | Original self-written route | **Port route (current)** |
|---|---|---|
| UIA event handling | Self-written `SetWinEventHook` (Task 4) | Port `PathWrap/src/os/monitor.rs` (399 LoC) — adaptive 8/30ms polling + 3-event hook + lost-tick recovery |
| Dialog detection | Self-written multi-condition (Task 5) | Reuse from `monitor.rs::get_dialog_info_if_match` (multi-feature: #32770 + title keywords + ComboBoxEx32/DirectUIHWND/SHELLDLL_DefView/DUIViewWndClassName) |
| Path injection | Self-written UIA + SendInput (Task 7) | Port `PathWrap/src/os/dialog.rs` (176 LoC) — `ValuePattern::SetValue` + `InvokePattern::Invoke` + `SendMessageW(WM_KEYDOWN, VK_RETURN)` fallback (NOT SendInput) |
| Foreground-steal defense | None planned | Port `QwenLM/.../fg_bypass.rs` (129 LoC) — `EnableWindow(FALSE)` RAII guard during UIA Invoke for Chromium hosts |
| Estimated 7-app coverage | <70% (predicted FAIL after 1st verify run) | **70-100%** (PathWrap/QwenLM empirically validated against UWP + Chromium) |
| LoC delivered | 3018 lines self-written (reset) | ~705 lines ported + supporting scaffold |

**Decision rationale:** See `handoff-2026-09-27-focus-activation-uia-port.md` §I. Key finding — `inaku-Gyan/PathWrap` uses **identical `windows` 0.62 stack** + MIT license, lifting its `dialog.rs` + `monitor.rs` is direct. QwenLM `fg_bypass.rs` empirically validated Chromium foreground-steal bypass (7/8 → 0 z-drops).

---

## Global Constraints

- Working directory: `F:\soft\00selfmade\filemanager\` (meta-repo root)
- Spike subdirectory: `sigma-listary-spike\` (new, lives in meta-repo main branch)
- Spike binary does NOT go to upstream — never push to `kizemo/sigma-file-manager`
- Default TCP port: 37421; auto-increment to 37422..37499 on conflict; panic if all taken
- JSON log format: one event per line on stdout (script-aggregatable)
- All errors via `anyhow::Result`; release build suppresses `debug_assert!`
- Exit codes: 0 = normal / coverage PASS, 1 = fatal error, 2 = coverage < 70%
- **Source attribution required** in `sigma-listary-spike/README.md` — link to upstream repos
- License: Apache-2.0 / MIT dual for ported files (per-file header); scaffold: project license
- **DO NOT extend / cherry-pick / release `feat/dialog-focus-sync` branch** (dead code, counter-example only)
- **DO NOT rebase / squash `feat/tree-sidebar-v6-1` branch** (HEAD 8caf14ae, stable, shipped)

---

## File Structure

| File | Source | Responsibility |
|---|---|---|
| `sigma-listary-spike/Cargo.toml` | new | Cargo manifest: windows 0.62, tokio, serde, tracing deps |
| `sigma-listary-spike/Cargo.lock` | auto | Auto-generated, committed for reproducibility |
| `sigma-listary-spike/src/main.rs` | new | Entry: `tokio::select!` over `uia_event` + `tcp_server` + `poll_path_changes` |
| `sigma-listary-spike/src/lib.rs` | new | Re-export modules for unit testing |
| `sigma-listary-spike/src/config.rs` | new | CLI arg parsing + port resolution + app whitelist |
| `sigma-listary-spike/src/logger.rs` | new | JSON event formatting (one-line JSON per event to stdout) |
| `sigma-listary-spike/src/events.rs` | new | Event enum + serde Serialize for JSON log output |
| `sigma-listary-spike/src/state.rs` | new | `AppState` registry: `Mutex<HashMap<HWND, DialogInfo>>` + `current_path: Mutex<String>` |
| `sigma-listary-spike/src/tcp_server.rs` | new | tokio TCP server: line-based JSON protocol |
| `sigma-listary-spike/src/detector.rs` | new | `is_file_dialog(hwnd) -> bool` — wraps `uia_event::get_dialog_info_if_match` |
| `sigma-listary-spike/src/reader.rs` | new | `read_path(hwnd) -> Result<String>` — UIA ValuePattern read |
| `sigma-listary-spike/src/writer.rs` | new | `write_path(hwnd, target) -> WriteOutcome` — wraps `uia_inject::inject_folder_path` + fg_bypass shield |
| **`sigma-listary-spike/src/uia_inject.rs`** | **port from PathWrap `dialog.rs`** | **UIA path injection: score Edit/Button → `ValuePattern::SetValue` + `InvokePattern::Invoke` + `SendMessageW(WM_RETURN)` fallback** |
| **`sigma-listary-spike/src/uia_event.rs`** | **port from PathWrap `monitor.rs`** | **`SetWinEventHook` (3 events) + adaptive polling (8ms/30ms) + lost-tick recovery + dialog detection** — **replace `egui::Context` with `tokio::sync::Notify`** |
| **`sigma-listary-spike/src/fg_bypass.rs`** | **port from QwenLM `fg_bypass.rs`** | **`EnableWindow(FALSE)` RAII guard during UIA Invoke for Chromium hosts; gate on `is_chromium_target_window` only (drop UWP/XAML branch)** |
| `sigma-listary-spike/scripts/verify_target_matrix.ps1` | new (modify original) | 7-app matrix verification; uses PATH fallback wrapper |
| `sigma-listary-spike/scripts/run-verify-with-paths.ps1` | copy from `.other/scripts/` | PATH fallback wrapper (resolves nc.exe, etc.) |
| `sigma-listary-spike/docs/SPIKE_REPORT_TEMPLATE.md` | new | Template for spike report |
| `sigma-listary-spike/README.md` | new | Build + run + verify instructions + source attribution |
| `sigma-listary-spike/.gitignore` | new | `target/`, `*.exe`, `*.pdb`, `*.log` |
| `docs/superpowers/spike-reports/2026-09-27-listary-focus-sync-spike.md` | Task 9 | Final spike report |
| `docs/superpowers/decisions/2026-09-27-listary-focus-sync-decision.md` | Task 9 | go/no-go decision |

---

## Task 0: Update plan + spec docs (route change)

**Files:**
- Overwrite: `docs/superpowers/plans/2026-09-27-listary-focus-sync.md` (this file)
- Append route-change note to: `docs/superpowers/specs/2026-09-27-listary-focus-sync-design.md`

- [ ] **Step 1: Overwrite plan file** with this content (port route, 11 tasks, source attribution)
- [ ] **Step 2: Append §10 "Route Change (2026-09-27)" to spec file** with brief note + link to handoff

Expected: spec §2.2 Component table now shows 3 ported files (`uia_inject.rs`, `uia_event.rs`, `fg_bypass.rs`) with MIT/Apache-2.0 attribution rows.

---

## Task 1: Cargo.toml + spike directory init + lib.rs skeleton

**Files:**
- Create: `sigma-listary-spike/Cargo.toml`
- Create: `sigma-listary-spike/src/lib.rs`
- Create: `sigma-listary-spike/.gitignore`
- Create: `sigma-listary-spike/README.md` (with attribution section)

- [ ] **Step 1: Create directories**
```bash
mkdir -p sigma-listary-spike/src/__tests__
mkdir -p sigma-listary-spike/scripts
mkdir -p sigma-listary-spike/docs
mkdir -p docs/superpowers/spike-reports
mkdir -p docs/superpowers/decisions
```

- [ ] **Step 2: Write `sigma-listary-spike/.gitignore`**
```
target/
*.exe
*.pdb
*.log
Cargo.lock.bak
```

- [ ] **Step 3: Write `sigma-listary-spike/Cargo.toml`**

```toml
[package]
name = "sigma-listary-spike"
version = "0.1.0"
edition = "2021"
publish = false
description = "Spike binary for Listary-style global focus sync. Port route: PathWrap + QwenLM fg_bypass."

[lib]
name = "spike"
path = "src/lib.rs"

[[bin]]
name = "spike"
path = "src/main.rs"

[dependencies]
windows = { version = "0.62", features = [
  "Win32_Foundation",
  "Win32_Graphics_Dwm",
  "Win32_System_Com",
  "Win32_System_LibraryLoader",
  "Win32_System_Threading",
  "Win32_UI_HiDpi",
  "Win32_UI_Accessibility",
  "Win32_UI_WindowsAndMessaging",
  "Win32_UI_Input_KeyboardAndMouse",
] }
tokio = { version = "1", features = ["net", "io-util", "macros", "rt-multi-thread", "sync", "time"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
clap = { version = "4", features = ["derive"] }

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
```

- [ ] **Step 4: Write minimal `sigma-listary-spike/src/lib.rs`**

```rust
//! Spike library — re-exports for unit testing.

pub mod config;
pub mod events;
pub mod logger;
pub mod state;
pub mod uia_inject;
pub mod uia_event;
pub mod fg_bypass;
pub mod detector;
pub mod reader;
pub mod writer;
pub mod tcp_server;
```

- [ ] **Step 5: Write `sigma-listary-spike/src/main.rs` skeleton**

```rust
//! Spike binary entry point.

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "spike", about = "Listary-style global focus sync spike")]
struct Cli {
    #[arg(long, default_value_t = 37421)]
    port: u16,
    #[arg(long, default_value = "C:\\")]
    initial_path: String,
    #[arg(long, value_delimiter = ',')]
    app_whitelist: Vec<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing::info!(port = cli.port, "spike starting (skeleton)");
    Ok(())
}
```

(Real wiring happens in Task 7 — `tokio::select!` over `uia_event` + `tcp_server` + `poll_path_changes`.)

- [ ] **Step 6: Write `sigma-listary-spike/README.md`** with source attribution section listing PathWrap (MIT) + QwenLM (Apache-2.0) + license boilerplate per file

- [ ] **Step 7: Verify skeleton compiles**

```bash
cd /f/soft/00selfmade/filemanager/sigma-listary-spike
cargo build --release 2>&1 | tail -30
```

Expected: BUILD SUCCEEDS with empty placeholder modules (uia_inject/uia_event/fg_bypass contain only `// TODO: port` for now).

- [ ] **Step 8: Commit**

```bash
cd /f/soft/00selfmade/filemanager
git add sigma-listary-spike/
git add docs/superpowers/plans/2026-09-27-listary-focus-sync.md
git add docs/superpowers/specs/2026-09-27-listary-focus-sync-design.md
git commit -m "feat(spike): cargo skeleton + plan/spec docs updated to port route"
```

---

## Task 2: Port PathWrap `dialog.rs` → `uia_inject.rs`

**Source:** `.other/src/PathWrap/src/os/dialog.rs` (176 lines)
**Target:** `sigma-listary-spike/src/uia_inject.rs`
**License header:** MIT (per `inaku-Gyan/PathWrap` LICENSE)

**Adaptations:**
- `dialog_hwnd: isize` (PathWrap) → `dialog_hwnd: u32` (spike convention per old plan)
- HWND construction: `HWND(dialog_hwnd as *mut core::ffi::c_void)`
- BSTR from `target_path`: keep as-is
- Score functions (`filename_edit_score` / `confirm_button_score`): port verbatim
- `fallback_confirm`: keep `SendMessageW(native, WM_KEYDOWN, ...)` — NOT SendInput

- [ ] **Step 1: Copy `dialog.rs` verbatim into `sigma-listary-spike/src/uia_inject.rs`**
- [ ] **Step 2: Add license header** (MIT, copyright `inaku-Gyan`)
- [ ] **Step 3: Adapt HWND type** — change `dialog_hwnd: isize` parameter to `u32`, internally cast
- [ ] **Step 4: Rename `inject_folder_path` → `pub fn inject_path(hwnd: u32, target: &str)`** for spike API consistency
- [ ] **Step 5: Build**

```bash
cd /f/soft/00selfmade/filemanager/sigma-listary-spike
cargo build --release 2>&1 | tail -30
```

Expected: compiles cleanly. If `BSTR::from(target_path)` type errors, add explicit `.to_string()` conversion.

- [ ] **Step 6: Commit**

```bash
git add sigma-listary-spike/src/uia_inject.rs
git commit -m "feat(spike): port PathWrap dialog.rs -> uia_inject.rs (MIT)"
```

---

## Task 3: Port PathWrap `monitor.rs` → `uia_event.rs`

**Source:** `.other/src/PathWrap/src/os/monitor.rs` (399 lines)
**Target:** `sigma-listary-spike/src/uia_event.rs`
**License header:** MIT (per `inaku-Gyan/PathWrap` LICENSE)

**Adaptations (CRITICAL):**
- **Replace `egui::Context` with `tokio::sync::Notify`**: `pub fn start_monitor(sender, ctx: egui::Context)` → `pub async fn start_monitor(sender: Sender<Option<DialogInfo>>, notify: Arc<Notify>)`
- `ctx.request_repaint()` → `notify.notify_one()` (3 occurrences in monitor loop)
- 3-event hook (`SetWinEventHook` for `EVENT_SYSTEM_FOREGROUND | EVENT_OBJECT_FOCUS | EVENT_OBJECT_SHOW`) — **port verbatim**
- Adaptive polling (8ms track / 30ms idle) — **port verbatim**
- Lost-tick recovery (3 ticks confirm) — **port verbatim**
- Multi-feature dialog detection (`get_dialog_info_if_match`) — **port verbatim** (used by `detector.rs`)
- DPI awareness (`DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` + `GetDpiForWindow`) — **port verbatim**

**Caveat:** PathWrap's monitor uses `std::sync::mpsc` (sync). Spike needs async to integrate with tokio::select. Adapt: keep monitor loop synchronous in own thread, send via sync mpsc, **also notify via `tokio::sync::Notify`** to wake select. See handoff §6.2.

- [ ] **Step 1: Copy `monitor.rs` verbatim into `sigma-listary-spike/src/uia_event.rs`**
- [ ] **Step 2: Add license header** (MIT, copyright `inaku-Gyan`)
- [ ] **Step 3: Replace `egui::Context` parameter with `Arc<tokio::sync::Notify>`**
- [ ] **Step 4: Replace all `ctx.request_repaint()` calls with `notify.notify_one()`**
- [ ] **Step 5: Extract `pub fn start_event_monitor(sender, notify) -> JoinHandle<()>` — wrap blocking loop in `tokio::task::spawn_blocking`**
- [ ] **Step 6: Extract `pub fn get_dialog_info_if_match(hwnd) -> Option<DialogInfo>` and `pub fn foreground_hwnd() -> isize` as public API for `detector.rs`**
- [ ] **Step 7: Build**

```bash
cd /f/soft/00selfmade/filemanager/sigma-listary-spike
cargo build --release 2>&1 | tail -30
```

Expected: compiles. **CRITICAL VALIDATION POINT** — confirms `windows` 0.62 compatibility with PathWrap's full feature set (DwmGetWindowAttribute, GetDpiForWindow, SetWinEventHook, GetClassNameW, FindWindowExW, etc.).

- [ ] **Step 8: Commit**

```bash
git add sigma-listary-spike/src/uia_event.rs
git commit -m "feat(spike): port PathWrap monitor.rs -> uia_event.rs (MIT) with tokio::Notify"
```

---

## Task 4: Port QwenLM `fg_bypass.rs` → `fg_bypass.rs`

**Source:** `.other/src/qwen-code/packages/cua-driver/rust/crates/platform-windows/src/uia/fg_bypass.rs` (129 lines)
**Target:** `sigma-listary-spike/src/fg_bypass.rs`
**License header:** Apache-2.0 (per QwenLM LICENSE)

**Adaptations:**
- **API upgrade windows 0.58 → 0.62**:
  - `BOOL` type: now `windows::core::BOOL` (0.62), same `.as_bool()` method
  - `HWND::is_null()` → `HWND::is_invalid()` (if 0.58 source uses is_null, change to is_invalid; 0.62 uses is_invalid)
- **Simplify gate**: drop `crate::input::is_xaml_host_hwnd` reference (spike doesn't have it). Implement minimal `is_chromium_target_window(hwnd) -> bool` = `get_class_name(hwnd).starts_with("Chrome_WidgetWin_")`
- Keep `DisabledHwndGuard` RAII pattern verbatim
- Keep `run_with_uwp_bypass` (rename to `run_with_bypass` since UWP branch dropped)

- [ ] **Step 1: Copy `fg_bypass.rs` verbatim into `sigma-listary-spike/src/fg_bypass.rs`**
- [ ] **Step 2: Add license header** (Apache-2.0, copyright QwenLM contributors)
- [ ] **Step 3: Implement `is_chromium_target_window(hwnd: HWND) -> bool`**
```rust
fn is_chromium_target_window(hwnd: HWND) -> bool {
    let class = get_class_name(hwnd);
    class.starts_with("Chrome_WidgetWin_")
}

fn get_class_name(hwnd: HWND) -> String {
    unsafe {
        let mut buf = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..len as usize])
    }
}
```
- [ ] **Step 4: Replace gate in `make_guard`**: drop `is_xaml_host_hwnd` reference, only `is_chromium_target_window`
- [ ] **Step 5: Rename `run_with_uwp_bypass` → `run_with_bypass`**
- [ ] **Step 6: Build**

```bash
cd /f/soft/00selfmade/filemanager/sigma-listary-spike
cargo build --release 2>&1 | tail -30
```

Expected: compiles. **SECOND VALIDATION POINT** — confirms windows 0.58→0.62 API diff (`HWND::is_null()` vs `is_invalid()`).

- [ ] **Step 7: Commit**

```bash
git add sigma-listary-spike/src/fg_bypass.rs
git commit -m "feat(spike): port QwenLM fg_bypass.rs (Apache-2.0) — chromium foreground shield"
```

---

## Task 5: cargo build --release — full spike validation

**CRITICAL VALIDATION POINT.** Per handoff §I: "Task 5 cargo build --release 是 windows 0.58→0.62 API 兼容性的关键验证点". All 3 ported files + scaffold must compile together. If any windows API signature mismatches between 0.58 (QwenLM) and 0.62 (PathWrap/spike), fix inline.

- [ ] **Step 1: Full build**

```bash
cd /f/soft/00selfmade/filemanager/sigma-listary-spike
cargo build --release 2>&1 | tail -50
```

Expected: build succeeds with all 3 ported files + scaffold. If errors, fix inline per handoff §6.3 (BOOL/HWND::is_invalid() etc.).

- [ ] **Step 2: Verify binary runs**

```bash
cd /f/soft/00selfmade/filemanager/sigma-listary-spike
./target/release/spike.exe --help
```

Expected: clap help text, exit 0.

- [ ] **Step 3: If build fails, document error in handoff and decide: fix inline OR escalate**

- [ ] **Step 4: Commit any fixes**

```bash
git add -u
git commit -m "fix(spike): windows 0.62 API compat fixes for ported files"
```

---

## Task 6: cargo test --lib — 8 unit tests pass

**Tests** (from old plan + spike scaffold):
- `state::tests` — `register_and_remove`, `current_path_updates` (2 tests)
- `config::tests` — `parse_config_defaults`, `resolve_port_preferred_free`, `resolve_port_increments_on_conflict` (3 tests)
- `events::tests` — `now_iso8601_format`, `read_event_serializes`, `log_event_does_not_panic` (3 tests)
Total: **8/8 pass**.

- [ ] **Step 1: Implement `src/state.rs`** (per handoff §6.5: `current_path: Mutex<String>` not `String`)

- [ ] **Step 2: Implement `src/config.rs`** (per old plan Task 2)

- [ ] **Step 3: Implement `src/events.rs`** (per old plan Task 3)

- [ ] **Step 4: Implement `src/logger.rs`** (per old plan Task 3)

- [ ] **Step 5: Add `[[test]]` sections to Cargo.toml** for `src/__tests__/{config,events}.rs`

- [ ] **Step 6: Run tests**

```bash
cd /f/soft/00selfmade/filemanager/sigma-listary-spike
cargo test --lib 2>&1 | tail -15
```

Expected: 8 tests pass.

- [ ] **Step 7: Commit**

```bash
git add sigma-listary-spike/src/{config,events,logger,state}.rs
git add sigma-listary-spike/src/__tests__/
git add sigma-listary-spike/Cargo.toml
git commit -m "feat(spike): state/config/events/logger modules + 8 unit tests"
```

---

## Task 7: Integrate into main.rs — `tokio::select!` over event sources

**Wires:**
- `tcp_server::run_server(state.clone(), cfg.port)` — TCP control
- `uia_event::start_event_monitor(sender, notify)` — SetWinEventHook + polling
- `poll_path_changes(state.clone())` — when `state.current_path` updates, write to all active dialogs

- [ ] **Step 1: Implement `src/tcp_server.rs`** (per old plan Task 9 — line-based JSON, set_path/get_status/quit)

- [ ] **Step 2: Implement `src/detector.rs`** — thin wrapper around `uia_event::get_dialog_info_if_match` returning bool

- [ ] **Step 3: Implement `src/reader.rs`** — thin wrapper around `uia_inject::automation()` + UIA ValuePattern read

- [ ] **Step 4: Implement `src/writer.rs`** — call `uia_inject::inject_path` wrapped in `fg_bypass::run_with_bypass`

- [ ] **Step 5: Rewrite `src/main.rs`** with full `tokio::select!`:

```rust
#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = parse_config(...)?;
    let state = Arc::new(AppState::new(cfg.initial_path.clone()));
    let notify = Arc::new(tokio::sync::Notify::new());

    // Start UIA event monitor in blocking thread
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(64);
    let notify_clone = notify.clone();
    tokio::task::spawn_blocking(move || {
        uia_event::start_event_monitor(event_tx, notify_clone);
    });

    // Start poll_path_changes (when state.current_path updates, write to dialogs)
    let state_for_poll = state.clone();
    tokio::spawn(async move {
        poll_path_changes(state_for_poll).await;
    });

    // Main select
    tokio::select! {
        _ = tcp_server::run_server(state.clone(), cfg.port) => {},
        Some(dialog_info) = event_rx.recv() => {
            // handle dialog detect/read events
        },
    }
    Ok(())
}
```

- [ ] **Step 6: Build**

```bash
cd /f/soft/00selfmade/filemanager/sigma-listary-spike
cargo build --release 2>&1 | tail -30
```

- [ ] **Step 7: Smoke test**

```bash
./target/release/spike.exe &
SP_PID=$!
sleep 2
echo '{"cmd":"get_status"}' | nc 127.0.0.1 37421
echo '{"cmd":"quit"}' | nc 127.0.0.1 37421
wait $SP_PID
```

Expected: get_status returns valid JSON; quit exits cleanly.

- [ ] **Step 8: Commit**

```bash
git add sigma-listary-spike/src/{main,tcp_server,detector,reader,writer}.rs
git commit -m "feat(spike): wire main.rs — tokio::select! over UIA + TCP server"
```

---

## Task 8: Run verify_target_matrix.ps1 — 7-app real coverage

- [ ] **Step 1: Update `scripts/verify_target_matrix.ps1`** with PATH fallback wrapper (template in `.other/scripts/run-verify-with-paths.ps1`)

- [ ] **Step 2: Run script** (manual intervention for each app):

```bash
cd /f/soft/00selfmade/filemanager/sigma-listary-spike
pwsh scripts/verify_target_matrix.ps1 -SpikePath ./target/release/spike.exe
```

Apps: Chrome, Edge, Word, VS Code, 钉钉, 飞书, 微信

- [ ] **Step 3: Capture spike log** to `docs/superpowers/spike-reports/2026-09-27-raw.log`

- [ ] **Step 4: Compute coverage** = (apps passing detect+read+write) / 7

- [ ] **Step 5: Commit script updates**

```bash
git add sigma-listary-spike/scripts/
git commit -m "feat(spike): verify_target_matrix.ps1 with PATH fallback"
```

---

## Task 9: Spike report + decision doc

- [ ] **Step 1: Copy `SPIKE_REPORT_TEMPLATE.md` to `docs/superpowers/spike-reports/2026-09-27-listary-focus-sync-spike.md`**
- [ ] **Step 2: Fill target matrix table with real verify results**
- [ ] **Step 3: Create `docs/superpowers/decisions/2026-09-27-listary-focus-sync-decision.md`** with PASS/FAIL based on coverage
- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/spike-reports/
git add docs/superpowers/decisions/
git commit -m "docs(spike): target matrix results + go/no-go decision"
```

---

## Task 10: Update `sigma-file-manager/README.md`

**Only after Task 9 PASS/FAIL decided.**

- [ ] **Step 1: If PASS**: append "Listary-style focus sync (Experimental)" section with X/7 coverage
- [ ] **Step 2: If FAIL**: append "Listary hook — limitation note" section (per handoff §III)
- [ ] **Step 3: Update both `README.md` and `README.zh-CN.md`**
- [ ] **Step 4: Commit**

```bash
git add sigma-file-manager/README.md sigma-file-manager/README.zh-CN.md
git commit -m "docs(readme): listary hook section (experimental or limitation)"
```

---

## Self-Review

### Spec Coverage

| Spec Section | Covered By |
|---|---|
| §1 Background | Task 0 + plan header "Route Change Context" |
| §2.1 Architecture | Tasks 1, 7 |
| §2.2 Components | Tasks 1-7 (each file mapped) |
| §2.3.1 Read flow | Tasks 3 (`uia_event` detect) + 6 (state) + 7 (`reader` wraps uia_inject) |
| §2.3.2 Write flow | Tasks 2 (`uia_inject` SetValue+Invoke) + 4 (`fg_bypass` shield) + 7 (`writer` orchestrator) |
| §2.3.3 Focus change | Task 3 (`SetWinEventHook` from PathWrap) |
| §2.3.4 TCP protocol | Task 7 (`tcp_server`) |
| §2.4 Error handling | Per-module `Result` types (Tasks 2, 3, 6) |
| §2.5.1 Manual smoke test | Task 7 Step 7 |
| §2.5.2 Target matrix | Task 8 |
| §2.5.3 Coverage decision | Task 9 (decision doc) |
| §3 Key decisions | Route Change table at top of plan |
| §5 Deliverables | Tasks 9-10 |
| §6 Risks | Mitigated: fg_bypass shields Chromium focus-steal (Task 4) |
| §8 References | handoff + .other/doc/00-research-overview.md |

### Identified Risks

- **HWND conversion**: spike uses `u32`, PathWrap uses `isize` — careful cast at boundaries
- **egui → tokio Notify**: PathWrap monitor loop is synchronous; needs `spawn_blocking` + `Notify` to integrate with `tokio::select!`
- **0.58 → 0.62 BOOL/HWND API diff**: likely minor, fix inline at Task 5
- **Source attribution compliance**: MIT/Apache-2.0 headers must be preserved verbatim per file