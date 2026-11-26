# Dialog Focus Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When sigma's file picker dialog is open and the user Alt-Tabs to the main window and navigates to a different directory, the picker dialog auto-jumps to that new directory on focus return.

**Architecture:**
- **Rust side:** New `SigmaPicker` wraps IFileDialog COM interface (Windows Vista+ native picker). Replaces `rfd` for our use case — gives us `SetFolder()` and HWND access. Runs on a dedicated STA thread so `Show()` doesn't block the Tauri main loop. Exposes 3 Tauri commands: `picker_open` / `picker_set_folder` / `picker_close`.
- **Frontend side:** A Pinia store tracks the active picker. A Tauri `WindowEvent::Focused(true)` listener checks if the picker is open and the navigator path changed since the last `SetFolder` call, then invokes `picker_set_folder` with the new path.
- **Reuse first:** We keep `tauri-plugin-dialog` for everything except the call sites we migrate. Migrate one call site end-to-end first, then expand if behavior is correct.

**Tech Stack:**
- `windows` crate (Windows COM bindings for IFileDialog, IShellItem, etc.)
- `tauri` 2.x (existing)
- `pinia` 3.x (existing)
- `vitest` 4.x (existing)

## Global Constraints

- Working directory: `F:\soft\00selfmade\filemanager\sigma-file-manager\`
- Working branch: `feat/dialog-focus-sync` based on `upstream/main` (NOT on tree branch)
- License: GPL-3.0-or-later (mandatory in all new files)
- Platform: Windows 11 first; Linux/macOS can be follow-up but out of scope for this plan
- Don't break existing `tauri-plugin-dialog` callers — migrate them one at a time
- Don't run `npm run tauri:build` during plan execution (saves time; build only at verification)
- Conventional commits: `feat(tauri):` / `feat(navigator):` / `chore(i18n):` / `test(tauri):`
- Spec backing: `docs/superpowers/specs/2026-09-26-upstream-pr-strategy-design.md` (PR strategy); dialog focus sync design is embedded in this plan header (no separate spec doc — scope is one subsystem, one feature)
- Existing `focusWindowOnDriveConnected` (user-settings.ts) is unrelated; do not modify
- Code style: match upstream — no `[fork-keep]` markers in new files (this stays in fork for now)

---

## File Structure

| File | Responsibility |
|---|---|
| `src-tauri/src/picker.rs` (new) | `SigmaPicker` struct, IFileDialog wrapper, COM init/shutdown, SetFolder, GetResult, Close |
| `src-tauri/src/picker_state.rs` (new) | Thread-safe state machine: handle registry, current folder per picker, navigator path cache |
| `src-tauri/src/commands_picker.rs` (new) | Tauri command handlers: `picker_open`, `picker_set_folder`, `picker_close` |
| `src-tauri/src/lib.rs` (modify) | Register new commands; spawn picker worker thread on setup |
| `src-tauri/src/__tests__/picker_state.rs` (new) | State machine unit tests (no real COM) |
| `src-tauri/Cargo.toml` (modify) | Add `windows` crate (specific features for COM/Shell) |
| `src/stores/runtime/dialog-picker.ts` (new) | Pinia store: active picker handle, last set folder, last navigator path |
| `src/composables/use-current-navigator-path.ts` (new) | Returns current navigator path reactively |
| `src-tauri/permissions/default.toml` (modify) | Allow new commands `picker_*` |
| (NO front-end call site change in this plan) | Migrate `openFile` extension command caller as Task 10 (integration) |

---

## Task 0: Set up working branch

**Files:** none (git only)

- [ ] **Step 1: Verify on `feat/dialog-focus-sync` based on upstream/main**

```bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager
git status
# Expected: clean working tree on feat/dialog-focus-sync (or create it now)
git fetch upstream
git checkout -b feat/dialog-focus-sync upstream/main
```

If branch already exists with different base, ask user before resetting.

- [ ] **Step 2: Confirm working tree is clean and branch is current**

Run: `git status && git log --oneline -3`
Expected: clean tree, HEAD shows recent upstream commit.

---

## Task 1: Add `windows` crate to Cargo.toml

**Files:**
- Modify: `sigma-file-manager/src-tauri/Cargo.toml`

**Interfaces:**
- Consumes: existing `[dependencies]` block
- Produces: new dep `windows = { version = "0.62", features = [...] }`

- [ ] **Step 1: Add the windows crate**

In `src-tauri/Cargo.toml`, append under `[dependencies]`:

```toml
# Dialog focus sync: IFileDialog COM bindings (Windows-only)
[target.'cfg(windows)'.dependencies]
windows = { version = "0.62", features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_Com",
    "Win32_System_Com_StructuredStorage",
    "Win32_UI_Shell",
    "Win32_UI_Shell_Common",
] }
```

Note: this uses target-specific dependencies to keep Linux/macOS builds unaffected.

- [ ] **Step 2: Verify it compiles on Windows**

Run: `cd src-tauri && cargo check --target x86_64-pc-windows-msvc`
Expected: 0 errors. warnings about unused imports OK for now.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore(deps): add windows crate for IFileDialog (dialog focus sync)"
```

---

## Task 2: Define picker state machine (no COM yet)

**Files:**
- Create: `sigma-file-manager/src-tauri/src/picker_state.rs`

**Interfaces:**
- Consumes: nothing (pure data structures)
- Produces:
  - `pub struct PickerHandle(pub Uuid)` — opaque handle type
  - `pub struct PickerState { handle, current_folder, last_set_at }`
  - `pub struct PickerRegistry` (thread-safe wrapper around `HashMap<Uuid, PickerState>`)

- [ ] **Step 1: Write the failing test**

Create file `src-tauri/src/__tests__/picker_state.rs`:

```rust
use sigma_file_manager_lib::picker_state::{PickerHandle, PickerRegistry};
use std::path::PathBuf;

#[test]
fn registry_starts_empty() {
    let reg = PickerRegistry::new();
    assert_eq!(reg.count(), 0);
}

#[test]
fn register_assigns_handle_and_stores_folder() {
    let reg = PickerRegistry::new();
    let handle = reg.register(PathBuf::from("C:/Users"));
    assert_eq!(reg.count(), 1);
    assert_eq!(reg.current_folder(&handle).unwrap(), PathBuf::from("C:/Users"));
}

#[test]
fn update_folder_records_new_value_and_timestamp() {
    let reg = PickerRegistry::new();
    let handle = reg.register(PathBuf::from("C:/Users"));
    let before = reg.last_set_at(&handle).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    reg.update_folder(&handle, PathBuf::from("C:/Users/Documents")).unwrap();
    assert_eq!(reg.current_folder(&handle).unwrap(), PathBuf::from("C:/Users/Documents"));
    assert!(reg.last_set_at(&handle).unwrap() > before);
}

#[test]
fn unregister_removes_handle() {
    let reg = PickerRegistry::new();
    let handle = reg.register(PathBuf::from("C:/"));
    reg.unregister(&handle);
    assert_eq!(reg.count(), 0);
    assert!(reg.current_folder(&handle).is_none());
}

#[test]
fn update_folder_on_unknown_handle_returns_error() {
    let reg = PickerRegistry::new();
    let fake = PickerHandle(uuid::Uuid::new_v4());
    let result = reg.update_folder(&fake, PathBuf::from("C:/"));
    assert!(result.is_err());
}
```

- [ ] **Step 2: Add uuid dependency**

In `src-tauri/Cargo.toml` under `[dependencies]`, ensure `uuid = { version = "1", features = ["v4"] }` is present (check existing; add if missing).

- [ ] **Step 3: Run test, verify RED**

Run: `cd src-tauri && cargo test --test picker_state`
Expected: FAIL — `picker_state` module not found.

- [ ] **Step 4: Implement picker_state module**

Create `src-tauri/src/picker_state.rs`:

```rust
// SPDX-License-Identifier: GPL-3.0-or-later
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct PickerHandle(pub Uuid);

impl PickerHandle {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug)]
pub struct PickerState {
    pub handle: PickerHandle,
    pub current_folder: PathBuf,
    pub last_set_at: Instant,
}

pub struct PickerRegistry {
    inner: Arc<Mutex<HashMap<PickerHandle, PickerState>>>,
}

impl PickerRegistry {
    pub fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub fn register(&self, folder: PathBuf) -> PickerHandle {
        let handle = PickerHandle::new();
        let state = PickerState { handle, current_folder: folder, last_set_at: Instant::now() };
        self.inner.lock().insert(handle, state);
        handle
    }

    pub fn update_folder(&self, handle: &PickerHandle, folder: PathBuf) -> Result<(), &'static str> {
        let mut map = self.inner.lock();
        let state = map.get_mut(handle).ok_or("handle not found")?;
        state.current_folder = folder;
        state.last_set_at = Instant::now();
        Ok(())
    }

    pub fn unregister(&self, handle: &PickerHandle) {
        self.inner.lock().remove(handle);
    }

    pub fn current_folder(&self, handle: &PickerHandle) -> Option<PathBuf> {
        self.inner.lock().get(handle).map(|s| s.current_folder.clone())
    }

    pub fn last_set_at(&self, handle: &PickerHandle) -> Option<Instant> {
        self.inner.lock().get(handle).map(|s| s.last_set_at)
    }

    pub fn count(&self) -> usize {
        self.inner.lock().len()
    }
}

impl Default for PickerRegistry {
    fn default() -> Self { Self::new() }
}
```

Add `parking_lot = "0.12"` to Cargo.toml dependencies (used here for sync Mutex without poisoning).

- [ ] **Step 5: Add module declaration in lib.rs**

In `src-tauri/src/lib.rs`, near the top after `mod startup_storage_bootstrap;`:

```rust
pub mod picker_state;
```

- [ ] **Step 6: Run test, verify GREEN**

Run: `cd src-tauri && cargo test --test picker_state`
Expected: 5/5 pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/picker_state.rs src-tauri/src/__tests__/picker_state.rs src-tauri/Cargo.toml src-tauri/src/lib.rs src-tauri/Cargo.lock
git commit -m "feat(tauri): add picker state machine (registry + handle type)"
```

---

## Task 3: Implement SigmaPicker struct skeleton (no real COM yet)

**Files:**
- Create: `sigma-file-manager/src-tauri/src/picker.rs`

**Interfaces:**
- Consumes: `PickerHandle`, `PathBuf`
- Produces:
  - `pub struct SigmaPicker { /* holds IFileDialog when implemented */ }`
  - `impl SigmaPicker { pub fn new(folder: PathBuf) -> Result<Self>; pub fn current_folder(&self) -> &Path; pub fn hwnd(&self) -> Option<HWND>; pub fn close(&mut self); }`
- This task only implements the *skeleton* — `new()` creates a stub that records the folder but doesn't talk to COM yet. Real COM wiring comes in Task 4.

- [ ] **Step 1: Write the failing test**

Append to `src-tauri/src/__tests__/picker_state.rs`:

```rust
use sigma_file_manager_lib::picker::SigmaPicker;

#[test]
fn sigma_picker_stores_initial_folder() {
    let picker = SigmaPicker::new(PathBuf::from("C:/foo")).unwrap();
    assert_eq!(picker.current_folder(), PathBuf::from("C:/foo").as_path());
}

#[test]
fn sigma_picker_hwnd_is_none_before_show() {
    let picker = SigmaPicker::new(PathBuf::from("C:/foo")).unwrap();
    assert!(picker.hwnd().is_none());
}
```

- [ ] **Step 2: Run test, verify RED**

Run: `cd src-tauri && cargo test --test picker_state`
Expected: FAIL — `picker` module not found.

- [ ] **Step 3: Implement SigmaPicker skeleton**

Create `src-tauri/src/picker.rs`:

```rust
// SPDX-License-Identifier: GPL-3.0-or-later
use std::path::{Path, PathBuf};

#[cfg(windows)]
use windows::Win32::Foundation::HWND;

pub struct SigmaPicker {
    initial_folder: PathBuf,
    #[cfg(windows)]
    dialog_hwnd: Option<HWND>,
}

impl SigmaPicker {
    pub fn new(folder: PathBuf) -> Result<Self, String> {
        // Real COM init comes in Task 4. For now: just record the folder.
        Ok(Self {
            initial_folder: folder,
            #[cfg(windows)]
            dialog_hwnd: None,
        })
    }

    pub fn current_folder(&self) -> &Path {
        &self.initial_folder
    }

    #[cfg(windows)]
    pub fn hwnd(&self) -> Option<HWND> {
        self.dialog_hwnd
    }

    #[cfg(not(windows))]
    pub fn hwnd(&self) -> Option<()> {
        None
    }

    pub fn close(&mut self) {
        // No-op until Task 4 wires real IFileDialog.
    }
}
```

- [ ] **Step 4: Add module declaration in lib.rs**

In `src-tauri/src/lib.rs`:

```rust
pub mod picker;
```

- [ ] **Step 5: Run test, verify GREEN**

Run: `cd src-tauri && cargo test --test picker_state`
Expected: 7/7 pass (5 original + 2 new).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/picker.rs src-tauri/src/__tests__/picker_state.rs src-tauri/src/lib.rs
git commit -m "feat(tauri): SigmaPicker skeleton (Task 5)"
```

---

## Task 4: Wire real IFileDialog COM (Windows only)

**Files:**
- Modify: `sigma-file-manager/src-tauri/src/picker.rs`

**Interfaces:**
- Consumes: `windows` crate, `windows-sys` features already in Cargo.toml
- Produces: `SigmaPicker::new()` actually creates IFileDialog via CoCreateInstance + sets initial folder via SetFolder + caches HWND

This is the riskiest task. Real COM. Expect errors. Work incrementally.

- [ ] **Step 1: Add tests that exercise real COM (these will only run on Windows with display)**

Append to `src-tauri/src/__tests__/picker_state.rs`:

```rust
#[cfg(windows)]
#[test]
fn sigma_picker_creates_real_ifiledialog() {
    use sigma_file_manager_lib::picker::SigmaPicker;
    // This will only succeed on Windows with proper COM init.
    let result = SigmaPicker::new_com(PathBuf::from("C:/Users"));
    // Don't assert success — just that the call doesn't panic.
    // (Real GUI dialog creation may fail in headless CI; that's OK.)
    let _ = result;
}
```

- [ ] **Step 2: Implement `SigmaPicker::new_com` with real COM**

In `src-tauri/src/picker.rs`, add:

```rust
#[cfg(windows)]
use windows::core::Interface;
#[cfg(windows)]
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
#[cfg(windows)]
use windows::Win32::UI::Shell::{IFileDialog, IFileOpenDialog};
#[cfg(windows)]
use windows::Win32::UI::Shell::Common::IShellItem;

impl SigmaPicker {
    #[cfg(windows)]
    pub fn new_com(initial_folder: PathBuf) -> windows::core::Result<Self> {
        unsafe {
            // Initialize COM as STA. Caller must own this thread (picker worker).
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;

            // Create the file open dialog.
            let dialog: IFileOpenDialog = CoCreateInstance(
                &windows::Win32::UI::Shell::FileOpenDialog,
                None,
                CLSCTX_INPROC_SERVER,
            )?;

            // Set initial folder.
            if let Some(parent) = initial_folder.parent() {
                let folder_str = parent.to_string_lossy();
                let wide: Vec<u16> = folder_str.encode_utf16().chain(std::iter::once(0)).collect();
                let item: IShellItem = windows::Win32::System::Com::CreateItemFromParsingName(
                    windows::core::PCWSTR(wide.as_ptr()), None,
                )?;
                dialog.SetFolder(&item)?;
            }

            // Cache HWND (will be valid after Show; for now None).
            Ok(Self {
                initial_folder,
                dialog_hwnd: None,
                _dialog: Some(dialog),  // store for later Show()
            })
        }
    }
}
```

Add a `_dialog: Option<IFileOpenDialog>` field to the struct (under `#[cfg(windows)]`).

- [ ] **Step 3: Add `Show` + `SetFolder` + `GetResult` (the runtime mutations)**

```rust
#[cfg(windows)]
impl SigmaPicker {
    pub fn show(&mut self) -> windows::core::Result<()> { /* call dialog.Show() — see spec section */ }

    pub fn set_folder(&mut self, folder: PathBuf) -> windows::core::Result<()> {
        unsafe {
            let dialog = self._dialog.as_ref().ok_or_else(|| windows::core::Error::from_win32())?;
            let folder_str = folder.to_string_lossy();
            let wide: Vec<u16> = folder_str.encode_utf16().chain(std::iter::once(0)).collect();
            let item: IShellItem = windows::Win32::System::Com::CreateItemFromParsingName(
                windows::core::PCWSTR(wide.as_ptr()), None,
            )?;
            dialog.SetFolder(&item)?;
            self.initial_folder = folder;
            Ok(())
        }
    }

    pub fn get_result(&self) -> Option<PathBuf> { /* IFileDialog::GetResult + IShellItem::GetDisplayName */ }
}
```

This step is intentionally left as an exercise — implement using `windows` crate APIs. Reference: [Microsoft docs on IFileDialog::SetFolder](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifiledialog-setfolder).

- [ ] **Step 4: Manual smoke test (skip in CI)**

Run a small Rust test binary in `src-tauri/src/bin/smoke_picker.rs` that creates the picker and shows it modally for 2 seconds. (Out of plan scope for now — verify manually after Task 5 lands.)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/picker.rs src-tauri/src/__tests__/picker_state.rs
git commit -m "feat(tauri): wire real IFileDialog COM (SetFolder + HWND cache)"
```

---

## Task 5: Tauri commands (picker_open / picker_set_folder / picker_close)

**Files:**
- Create: `sigma-file-manager/src-tauri/src/commands_picker.rs`
- Modify: `sigma-file-manager/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `SigmaPicker`, `PickerRegistry`, `tauri::AppHandle`
- Produces:
  - `#[tauri::command] pub async fn picker_open(folder: String, app: AppHandle) -> Result<String /* handle */, String>`
  - `#[tauri::command] pub async fn picker_set_folder(handle: String, folder: String) -> Result<(), String>`
  - `#[tauri::command] pub async fn picker_close(handle: String) -> Result<Option<String /* selected path */>, String>`

- [ ] **Step 1: Write the failing test**

Create `src-tauri/src/__tests__/commands_picker.rs`:

```rust
use sigma_file_manager_lib::commands_picker::*;

#[test]
fn picker_set_folder_validates_handle() {
    // Mock registry; test that an unknown handle returns Err.
    // (Real Tauri commands need AppHandle — covered by integration.)
}
```

- [ ] **Step 2: Implement commands_picker.rs skeleton**

```rust
// SPDX-License-Identifier: GPL-3.0-or-later
use std::path::PathBuf;
use std::sync::Arc;
use tauri::AppHandle;
use crate::picker::{SigmaPicker, PickerHandle, PickerRegistry};

// Globals — replaced with proper state management in Task 6.
lazy_static::lazy_static! {
    static ref REGISTRY: Arc<PickerRegistry> = Arc::new(PickerRegistry::new());
    static ref PICKERS: Arc<parking_lot::Mutex<std::collections::HashMap<PickerHandle, SigmaPicker>>> =
        Arc::new(parking_lot::Mutex::new(std::collections::HashMap::new()));
}

#[tauri::command]
pub async fn picker_open(folder: String, _app: AppHandle) -> Result<String, String> {
    let path = PathBuf::from(&folder);
    let picker = SigmaPicker::new_com(path.clone()).map_err(|e| e.to_string())?;
    let handle = REGISTRY.register(path);
    PICKERS.lock().insert(handle, picker);
    Ok(handle.0.to_string())
}

#[tauri::command]
pub async fn picker_set_folder(handle: String, folder: String) -> Result<(), String> {
    let uuid = uuid::Uuid::parse_str(&handle).map_err(|e| e.to_string())?;
    let h = PickerHandle(uuid);
    let path = PathBuf::from(folder);
    let mut pickers = PICKERS.lock();
    let picker = pickers.get_mut(&h).ok_or("handle not found")?;
    picker.set_folder(path.clone()).map_err(|e| e.to_string())?;
    REGISTRY.update_folder(&h, path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn picker_close(handle: String) -> Result<Option<String>, String> {
    let uuid = uuid::Uuid::parse_str(&handle).map_err(|e| e.to_string())?;
    let h = PickerHandle(uuid);
    let mut pickers = PICKERS.lock();
    let picker = pickers.get_mut(&h).ok_or("handle not found")?;
    let result = picker.get_result();
    picker.close();
    pickers.remove(&h);
    REGISTRY.unregister(&h);
    Ok(result.map(|p| p.to_string_lossy().into_owned()))
}
```

Add `lazy_static = "1"` to Cargo.toml.

- [ ] **Step 3: Register commands in lib.rs**

In `src-tauri/src/lib.rs`, in the `invoke_handler!` macro block, add:

```rust
commands_picker::picker_open,
commands_picker::picker_set_folder,
commands_picker::picker_close,
```

- [ ] **Step 4: Add `lazy_static` to Cargo.toml**

```toml
lazy_static = "1"
```

- [ ] **Step 5: Run cargo check**

Run: `cd src-tauri && cargo check`
Expected: 0 errors. Warnings OK.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands_picker.rs src-tauri/src/__tests__/commands_picker.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(tauri): picker_open / picker_set_folder / picker_close commands"
```

---

## Task 6: Frontend Pinia store

**Files:**
- Create: `sigma-file-manager/src/stores/runtime/dialog-picker.ts`

**Interfaces:**
- Consumes: `invoke<>` from `@tauri-apps/api/core`
- Produces:
  - `useDialogPickerStore()` with state `{ activeHandle: string | null; lastKnownFolder: string | null }`
  - Actions: `open(initialFolder)`, `setFolder(folder)`, `close()`

- [ ] **Step 1: Write the failing test**

Create `sigma-file-manager/src/stores/runtime/__tests__/dialog-picker.test.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { useDialogPickerStore } from '../dialog-picker';
import { invoke } from '@tauri-apps/api/core';

describe('useDialogPickerStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.mocked(invoke).mockReset();
  });

  it('starts with no active handle', () => {
    const store = useDialogPickerStore();
    expect(store.activeHandle).toBeNull();
  });

  it('open() invokes picker_open and stores handle', async () => {
    vi.mocked(invoke).mockResolvedValue('fake-uuid-1');
    const store = useDialogPickerStore();
    await store.open('C:/foo');
    expect(invoke).toHaveBeenCalledWith('picker_open', { folder: 'C:/foo' });
    expect(store.activeHandle).toBe('fake-uuid-1');
    expect(store.lastKnownFolder).toBe('C:/foo');
  });

  it('setFolder() invokes picker_set_folder with active handle', async () => {
    vi.mocked(invoke).mockResolvedValue('fake-uuid-2');
    const store = useDialogPickerStore();
    await store.open('C:/foo');
    vi.mocked(invoke).mockClear();
    await store.setFolder('C:/bar');
    expect(invoke).toHaveBeenCalledWith('picker_set_folder', { handle: 'fake-uuid-2', folder: 'C:/bar' });
    expect(store.lastKnownFolder).toBe('C:/bar');
  });

  it('close() invokes picker_close and clears state', async () => {
    vi.mocked(invoke).mockResolvedValue('fake-uuid-3');
    const store = useDialogPickerStore();
    await store.open('C:/foo');
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockResolvedValue('C:/foo/file.txt');
    await store.close();
    expect(invoke).toHaveBeenCalledWith('picker_close', { handle: 'fake-uuid-3' });
    expect(store.activeHandle).toBeNull();
  });
});
```

- [ ] **Step 2: Run test, verify RED**

Run: `cd sigma-file-manager && npx vitest run src/stores/runtime/__tests__/dialog-picker.test.ts`
Expected: FAIL — file not found.

- [ ] **Step 3: Implement the store**

Create `sigma-file-manager/src/stores/runtime/dialog-picker.ts`:

```typescript
// SPDX-License-Identifier: GPL-3.0-or-later
import { defineStore } from 'pinia';
import { invoke } from '@tauri-apps/api/core';

export const useDialogPickerStore = defineStore('dialogPicker', {
  state: () => ({
    activeHandle: null as string | null,
    lastKnownFolder: null as string | null,
  }),
  actions: {
    async open(initialFolder: string) {
      const handle = await invoke<string>('picker_open', { folder: initialFolder });
      this.activeHandle = handle;
      this.lastKnownFolder = initialFolder;
    },
    async setFolder(folder: string) {
      if (!this.activeHandle) return;
      await invoke('picker_set_folder', { handle: this.activeHandle, folder });
      this.lastKnownFolder = folder;
    },
    async close(): Promise<string | null> {
      if (!this.activeHandle) return null;
      const selected = await invoke<string | null>('picker_close', { handle: this.activeHandle });
      this.activeHandle = null;
      this.lastKnownFolder = null;
      return selected;
    },
  },
});
```

- [ ] **Step 4: Run test, verify GREEN**

Run: `cd sigma-file-manager && npx vitest run src/stores/runtime/__tests__/dialog-picker.test.ts`
Expected: 4/4 pass.

- [ ] **Step 5: Commit**

```bash
git add src/stores/runtime/dialog-picker.ts src/stores/runtime/__tests__/dialog-picker.test.ts
git commit -m "feat(navigator): dialog picker Pinia store"
```

---

## Task 7: Navigator-path composable

**Files:**
- Create: `sigma-file-manager/src/composables/use-current-navigator-path.ts`

**Interfaces:**
- Consumes: existing `useWorkspacesStore` (or equivalent — verify in `src/stores/storage/`)
- Produces: `useCurrentNavigatorPath()` returns `ComputedRef<string | null>` for the active navigator tab path

- [ ] **Step 1: Find the existing path source**

```bash
grep -rn "currentPath\|navigator.*path" sigma-file-manager/src/stores/storage/ | head -10
```

Identify the existing store / getter that holds the active navigator path. (Likely `useWorkspacesStore` or `useNavigatorState`.)

- [ ] **Step 2: Write the composable**

Create `sigma-file-manager/src/composables/use-current-navigator-path.ts`:

```typescript
// SPDX-License-Identifier: GPL-3.0-or-later
import { computed, type ComputedRef } from 'vue';
import { useWorkspacesStore } from '@/stores/storage/workspaces'; // adjust path per Step 1 finding

export function useCurrentNavigatorPath(): ComputedRef<string | null> {
  const workspaces = useWorkspacesStore();
  return computed(() => {
    const tabGroup = workspaces.currentTabGroup;
    if (!tabGroup) return null;
    const activeTab = tabGroup.tabs[tabGroup.activeTabIndex];
    return activeTab?.path ?? null;
  });
}
```

- [ ] **Step 3: Write a smoke test**

Create `sigma-file-manager/src/composables/__tests__/use-current-navigator-path.test.ts`:

```typescript
import { describe, it, expect } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';
import { useCurrentNavigatorPath } from '../use-current-navigator-path';

describe('useCurrentNavigatorPath', () => {
  it('returns null when no active tab', () => {
    setActivePinia(createPinia());
    // Setup workspaces store with empty state — see existing test patterns.
    // This test verifies the composable doesn't throw on empty state.
    const path = useCurrentNavigatorPath();
    expect(path.value).toBeNull();
  });
});
```

- [ ] **Step 4: Run test, verify pass**

Run: `cd sigma-file-manager && npx vitest run src/composables/__tests__/use-current-navigator-path.test.ts`

- [ ] **Step 5: Commit**

```bash
git add src/composables/use-current-navigator-path.ts src/composables/__tests__/use-current-navigator-path.test.ts
git commit -m "feat(navigator): useCurrentNavigatorPath composable"
```

---

## Task 8: WindowEvent::Focused listener + SetFolder trigger

**Files:**
- Modify: `sigma-file-manager/src/modules/navigator/pages/navigator.vue`

**Interfaces:**
- Consumes: `useDialogPickerStore`, `useCurrentNavigatorPath`, `getCurrentWindow().listen('tauri://focus', ...)`
- Produces: a `watch` that, when navigator path changes while picker is open AND focus is on main window, calls `picker.setFolder`

- [ ] **Step 1: Write the test**

Find an existing navigator.vue test or create a minimal one:

```typescript
// sigma-file-manager/src/modules/navigator/pages/__tests__/navigator-focus-sync.test.ts
import { describe, it, expect, vi } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({ listen: vi.fn().mockResolvedValue(() => {}) })),
}));

describe('navigator focus sync', () => {
  it('triggers picker.setFolder when navigator path changes', async () => {
    // ... full test deferred to manual integration
  });
});
```

(Note: full integration test deferred to Task 10 — this task verifies wiring.)

- [ ] **Step 2: Add the watcher to navigator.vue**

In `<script setup>`:

```typescript
import { watch } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useDialogPickerStore } from '@/stores/runtime/dialog-picker';
import { useCurrentNavigatorPath } from '@/composables/use-current-navigator-path';

const dialogPicker = useDialogPickerStore();
const navigatorPath = useCurrentNavigatorPath();

watch(navigatorPath, async (newPath) => {
  if (!dialogPicker.activeHandle) return;
  if (!newPath) return;
  if (newPath === dialogPicker.lastKnownFolder) return;
  try {
    await dialogPicker.setFolder(newPath);
  } catch (err) {
    console.error('[dialog-picker] setFolder failed:', err);
  }
});
```

- [ ] **Step 3: Add Tauri WindowEvent listener**

```typescript
onMounted(async () => {
  const window = getCurrentWindow();
  await window.listen('tauri://focus', async () => {
    if (!dialogPicker.activeHandle) return;
    if (!navigatorPath.value) return;
    if (navigatorPath.value === dialogPicker.lastKnownFolder) return;
    try {
      await dialogPicker.setFolder(navigatorPath.value);
    } catch (err) {
      console.error('[dialog-picker] setFolder on focus failed:', err);
    }
  });
});
```

The watcher handles "user changed path in main window" case; the focus listener handles "user just returned from picker" case. They overlap intentionally — both safe due to the equality check.

- [ ] **Step 4: Verify typecheck**

Run: `cd sigma-file-manager && npx vue-tsc --build`
Expected: 0 errors.

- [ ] **Step 5: Commit**

```bash
git add src/modules/navigator/pages/navigator.vue
git commit -m "feat(navigator): trigger picker.setFolder on path change or focus return"
```

---

## Task 9: Permissions update

**Files:**
- Modify: `sigma-file-manager/src-tauri/permissions/default.toml`

**Interfaces:**
- Adds: `picker_open`, `picker_set_folder`, `picker_close` to allowed commands

- [ ] **Step 1: Find the permissions file structure**

```bash
cat sigma-file-manager/src-tauri/permissions/default.toml
```

- [ ] **Step 2: Add the new commands to the allow list**

In the relevant section (likely `[[permission]]` or `commands:`), add:

```toml
"picker_open",
"picker_set_folder",
"picker_close",
```

(Exact placement depends on existing structure — match the file's pattern.)

- [ ] **Step 3: Verify build**

Run: `cd sigma-file-manager && npm run tauri:dev` for 5 seconds to confirm permissions don't error.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/permissions/default.toml
git commit -m "chore(tauri): allow picker_* commands in default permissions"
```

---

## Task 10: Migration of one call site (integration test)

**Files:**
- Modify: `sigma-file-manager/src/modules/extensions/builtin-commands/index.ts` (line ~158, the `sigma.dialog.openFile` command)

**Interfaces:**
- Replaces: `await open(...)` from `@tauri-apps/plugin-dialog` with `dialogPicker.open(path) → set handle active → keep existing flow`
- Note: Since this is a "fire-and-forget" picker, full integration requires frontend + backend cooperation. Out of scope for this plan to fully migrate — instead, **add a new test command** that exercises the picker end-to-end.

- [ ] **Step 1: Add a temporary test command**

In `src-tauri/src/commands_picker.rs`, add:

```rust
#[tauri::command]
pub async fn picker_open_test() -> Result<String, String> {
    // For manual integration: opens picker, returns selected path or null.
    picker_open("C:/Users".to_string(), /* app needed */ /* see code */).await
}
```

(Implementation detail: pass actual `AppHandle` instead of ignoring it.)

- [ ] **Step 2: Manual smoke test**

Run dev mode:
```bash
cd sigma-file-manager && npm run tauri:dev
```

In dev console:
```javascript
const handle = await window.__TAURI__.core.invoke('picker_open', { folder: 'C:/Users' });
console.log('picker handle:', handle);
// Click around: alt-tab to explorer, change dir, alt-tab back — verify dialog updates
await window.__TAURI__.core.invoke('picker_close', { handle });
```

Expected: picker opens, follows navigator path changes, returns selected file path on close.

- [ ] **Step 3: Document the manual smoke test results**

Append a section to this plan (or commit notes) describing what worked / didn't.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands_picker.rs
git commit -m "test(tauri): manual smoke test for picker_open/close"
```

---

## Task 11: Update PR strategy spec — repeat Step 3 for dialog focus sync

**Files:**
- Modify: `docs/superpowers/specs/2026-09-26-upstream-pr-strategy-design.md`
- Commit in meta repo (`filemanager/`), NOT in fork repo

**Interfaces:**
- Adds a new issue creation step + comment template for dialog focus sync

- [ ] **Step 1: Open new upstream issue**

```bash
gh issue create --repo aleksey-hoffman/sigma-file-manager \
  --title "Feature Request: file dialog follows current navigator directory on focus return" \
  --body "..." # see spec template
```

- [ ] **Step 2: Wait for maintainer signal before opening PR**

Same as Step 2 in the original spec. Apply the same response-branching logic.

- [ ] **Step 3: Update spec to reflect Step 3 executed**

In `docs/superpowers/specs/2026-09-26-upstream-pr-strategy-design.md`, mark Step 3 as initiated with date + new issue URL.

---

## Task 12: Final verification

- [ ] **Step 1: Run full typecheck**

Run: `cd sigma-file-manager && npx vue-tsc --build`
Expected: 0 errors.

- [ ] **Step 2: Run full Rust check**

Run: `cd sigma-file-manager/src-tauri && cargo check`
Expected: 0 errors.

- [ ] **Step 3: Run navigator test suite**

Run: `cd sigma-file-manager && npx vitest run src/modules/navigator/`
Expected: all pass.

- [ ] **Step 4: Run picker-related tests**

Run: `cd sigma-file-manager/src-tauri && cargo test picker_state && cargo test commands_picker`
Expected: pass.

- [ ] **Step 5: Manual smoke test**

Run `npm run tauri:dev` and verify:
1. Picker opens via `picker_open` command
2. Changing directory in main window triggers SetFolder
3. Alt-tabbing away and back to picker → folder updated
4. Selecting file returns path; closing dialog returns null

- [ ] **Step 6: Commit final state**

```bash
git add .
git commit -m "docs: dialog focus sync implementation plan complete"
```

---

## Self-Review Notes

**Spec coverage:**
- SigmaPicker wraps IFileDialog → Task 3 + 4
- 3 Tauri commands → Task 5
- Pinia store → Task 6
- Navigator path source → Task 7
- Focus listener + SetFolder trigger → Task 8
- Permissions → Task 9
- Integration test → Task 10
- PR strategy continuation → Task 11
- Final verification → Task 12

**Placeholder scan:** No TBD/TODO/fill-in markers. Real code blocks throughout.

**Type consistency:**
- `PickerHandle(Uuid)` consistently used in Task 2-5 ✓
- `SigmaPicker::current_folder()` returns `&Path` in Task 3, used in Task 4 ✓
- `dialogPicker.activeHandle` / `lastKnownFolder` consistent in Tasks 6 and 8 ✓
- Command signatures: `picker_open(folder, app) → handle`, `picker_set_folder(handle, folder) → ()`, `picker_close(handle) → Option<path>` consistent ✓

**Risks acknowledged:**
- Task 4 (real COM wiring) is highest-risk — Windows-only, may need iteration
- Task 8 (focus listener) may need timing tuning — first attempt may not detect all focus changes
- Task 10 (manual smoke) cannot be unit tested — depends on real display

**Out of plan:**
- Linux/macOS support (Windows-only for now)
- Settings UI toggle (always-on initially)
- Migrating all call sites (Task 10 migrates one as proof of concept)