# kizemo.focus-sync Standalone Installer

This directory contains a one-click NSIS installer that registers the Focus Sync
extension with Sigma File Manager. It bundles the `register.ps1` and
`unregister.ps1` scripts that edit `%APPDATA%\com.sigma-file-manager.app\user-data\user-extensions.json`.

## Files

| File | Purpose |
|---|---|
| `installer.nsi` | NSIS source. Compiles to `kizemo.focus-sync-0.2.0-setup.exe`. |
| `register.ps1` | Adds kizemo.focus-sync entry to user-extensions.json. |
| `unregister.ps1` | Removes kizemo.focus-sync entry from user-extensions.json. |
| `README.md` | This file. |
| `build.ps1` | One-shot build script (calls makensis). |

## Build

```powershell
powershell -ExecutionPolicy Bypass -File build.ps1
```

Output: `kizemo.focus-sync-0.2.0-setup.exe` (typically ~3 MB).

## Usage

### Install

> **IMPORTANT — Sigma File Manager MUST be closed before running this installer.**
>
> The installer **HARD-BLOCKS** (refuses to install) if `sigma-file-manager.exe`
> is running. Reason: Sigma FM holds an in-memory copy of `user-extensions.json`
> and silently overwrites the on-disk file on next save (typically within 2 minutes
> or on graceful close). If we let install complete while Sigma FM is running,
> the registration edit is wiped and Focus Sync will not appear in the Installed
> tab. This class of failure was the 2026-09-30 incident — fix is HARD-BLOCK.

1. **Right-click the Sigma File Manager taskbar icon → Quit.** Make sure
   no `sigma-file-manager.exe` is in Task Manager.
2. Double-click `kizemo.focus-sync-0.2.0-setup.exe`.
3. Follow the install wizard. The installer copies extension files to
   `%APPDATA%\com.sigma-file-manager.app\` and edits user-extensions.json.
4. Open Sigma File Manager.
5. Press **Ctrl+Shift+X** to open the Extensions panel.
6. The **Installed** tab should now list **Focus Sync** (author kizemo, 96.x KB).
7. Toggle it on. The toolbar should show **Sync Now** / **Enable / Disable** items.
8. Test: open any Windows app's "Save As" dialog (Notepad, Chrome, Word, etc.)
   and navigate folders in Sigma FM. The dialog should auto-jump to the
   current Sigma folder.

### Uninstall

1. Close Sigma File Manager.
2. Run `appwiz.cpl` (Add or Remove Programs).
3. Find "Focus Sync Extension" and click Uninstall.

Or run `kizemo.focus-sync-0.2.0-setup.exe` again and choose Uninstall from
the welcome page (NSIS auto-detects if the installer has been run before).

## How it works

Sigma File Manager's "Installed" tab reads from
`%APPDATA%\com.sigma-file-manager.app\user-data\user-extensions.json`'s
`installedExtensions` field — **not** from a disk scan of the `extensions/`
folder. So merely copying files there is insufficient; the entry must also
be registered in user-extensions.json.

`register.ps1` reads user-extensions.json, parses it as JSON, adds a
`kizemo.focus-sync` entry (modeled after the existing `hetima.light-themes`
entry), and writes it back with 2-space indent (matching Sigma FM's format).
On the next Sigma FM startup, the new entry is loaded into the in-memory
extension store and the Installed tab lists Focus Sync.

## Why CLOSE Sigma FM first?

Sigma FM uses Tauri LazyStore with a one-shot startup bootstrap. Once
running, its in-memory state is decoupled from the on-disk file. If you
edit user-extensions.json while Sigma FM is running, your edit is preserved
on disk but Sigma FM still has the old state — and the next save
(any UI interaction, periodic flush) overwrites your edit with the stale
in-memory state, undoing the registration.

## What if Focus Sync still doesn't appear after install?

1. Verify the file was edited:
   ```powershell
   Get-Content "$env:APPDATA\com.sigma-file-manager.app\user-data\user-extensions.json" |
       ConvertFrom-Json |
       Select-Object -ExpandProperty installedExtensions |
       Select-Object *, @{N='kizemo';E={$_.PSObject.Properties.Name -contains 'kizemo.focus-sync'}}
   ```
   The `kizemo` row should show `True`.

2. Confirm the extension files are on disk:
   ```powershell
   Test-Path "$env:APPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\package.json"
   Test-Path "$env:APPDATA\com.sigma-file-manager.app\binaries\focus-sync-sidecar\0.2.0\focus-sync-sidecar.exe"
   ```
   Both should return `True`.

3. Re-run `register.ps1` manually after closing Sigma FM.

4. If still no luck, check Sigma FM's console (DevTools) for schema
   validation errors on the manifest.