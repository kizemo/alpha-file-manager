---
name: 2026-09-29 handoff (round 9) - Sidecar path fix
description: Round 9. Fixed: sidecar must live at extensions/<id>/bin/<binary_id>/ not binaries/<binary_id>/<version>/. Sigma FM's get_extension_binary_path looks at the former.
type: project
---

# Handoff — Sidecar Path Fix (2026-09-29 round 9)

**Inherits**: `handoff-2026-09-29-isLocal-fix.md` (round 8 — extension shows as enabled but sidecar never spawned)

**Status**: ✅ **FIX APPLIED + EXE REBUILT** — sidecar copied to `extensions\kizemo.focus-sync\bin\focus-sync-sidecar\`. NSIS installer updated to use this path. Exe rebuilt (986,032 bytes, 15:33:14). Awaiting user verification of focus sync functionality.

---

## 一、本轮问题

User confirmed extension is visible and "已激活" (toggled on), but focus sync doesn't work:
> 通过 edge 下载文件，弹出下载弹窗，在选择保存路径界面，切换回 sigma 窗口，再切回下载弹窗界面，下载路径没有进行切换。

### Diagnostic

```
==> Sidecar process:   NOT RUNNING
==> Port 37421:        NOT listening
==> Extension JS:      loaded (dist/index.js at correct path)
==> Manifest:          correct (activationEvents: onStartup, http permission, binaries declared)
==> Sidecar exe:       2,927,616 bytes, sha256 matches installer staging
```

**Extension UI is "on" but sidecar process doesn't exist.** activate() either errored early or never ran.

---

## 二、真正根因

`extension/dist/index.js:105-114`:
```javascript
const sidecarPath = await sigma.binary.getPath(SIDECAR_BINARY_ID);
if (!sidecarPath) {
    sigma.ui.showNotification({
        title: 'Focus Sync',
        description: 'Sidecar binary not installed. Open Extensions → Focus Sync → Install Binary.',
        type: 'error',
        duration: 8000,
    });
    return;        // <-- returns early, no path-change subscription, no sidecar
}
```

If `sigma.binary.getPath()` returns null, the extension silently bails out (after showing an error toast that the user might miss).

### Why `getPath` returned null

`sigma-file-manager/src-tauri/src/extensions/binaries.rs:118-134`:
```rust
pub async fn get_extension_binary_path(...) {
    let binary_dir = get_extension_binary_dir(&app_handle, &extension_id, &binary_id)?;
    let binary_path = resolve_binary_file_path(&binary_dir, &executable_name)?;
    if binary_path.exists() {
        Ok(Some(binary_path.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}
```

And `binaries.rs:40-48`:
```rust
fn get_extension_binary_dir(...) -> Result<PathBuf, String> {
    let extension_dir = get_extension_dir(app_handle, extension_id)?;
    Ok(extension_dir.join("bin").join(binary_id))   // <-- expected path
}
```

So Sigma FM looks at:
**`%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe`**

But round 7's installer put it at:
**`%APPDATA%\com.sigma-file-manager.app\binaries\focus-sync-sidecar\0.2.0\focus-sync-sidecar.exe`** ❌

→ `binary_path.exists()` returns false → `Ok(None)` → extension bails.

---

## 三、修复

### 3.1 Run-time fix (immediate)

Copied sidecar to expected location:
```
%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe  ✅
```

Smoke test: sidecar runs at new location, port 37421 LISTENING, HTTP 404 on `/` (expected — only `/set_path` and `/quit` endpoints defined), 5-sec self-test passed.

### 3.2 Installer fix (forward-going)

`release/extension-installer/installer.nsi` updated:
- Stage 4 `SetOutPath` target changed from `binaries\${PRODUCT_SIDECAR}\${PRODUCT_SIDECAR_VER}` to `extensions\${PRODUCT_EXTDIR}\bin\${PRODUCT_SIDECAR}`
- Uninstaller `RMDir /r binaries\${PRODUCT_SIDECAR}` retained for legacy cleanup
- Status log line updated to show new sidecar path

Rebuilt:
```
release\extension-installer\kizemo.focus-sync-0.2.0-setup.exe   986,032 bytes   15:33:14
release\kizemo.focus-sync-0.2.0-setup.exe                        (top-level copy)
```

### 3.3 Orphan cleanup (optional)

`%APPDATA%\com.sigma-file-manager.app\binaries\focus-sync-sidecar\0.2.0\` is still on disk from round 7 install. It's NOT used by Sigma FM (since `get_extension_binary_path` doesn't look there). Safe to manually delete via:
```powershell
Remove-Item -Recurse -Force "$env:APPDATA\com.sigma-file-manager.app\binaries\focus-sync-sidecar"
```

---

## 四、用户验证

Sigma FM is currently NOT running. To complete the fix:

1. **Start Sigma FM** (double-click `D:\Program Files\Sigma File Manager\sigma-file-manager.exe`)
3. Wait ~5 seconds for `activate()` to spawn sidecar
4. Open Edge, trigger a download → Save As dialog
5. Switch to Sigma FM, navigate to any folder
6. Switch back to Edge's Save As dialog — path should auto-update

If still doesn't work, run diagnostics:
```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File C:\Temp\diag-focus-sync-runtime.ps1
```

If sidecar process still not running, check Sigma FM DevTools console for the "Sidecar binary not installed" error toast.

---

## 五、技术沉淀(给未来会话)

### 5.1 Sigma FM binary location rule(关键)

When an extension declares `manifest.binaries`, the binary file MUST live at:

```
%APPDATA%\com.sigma-file-manager.app\extensions\<extension_id>\bin\<binary_id>\<executable_name>
```

This is hardcoded in `sigma-file-manager/src-tauri/src/extensions/binaries.rs:40-48`. There is no fallback to `binaries/` or other paths.

Version is NOT part of the path (despite the manifest declaring `version`).

### 5.2 What `sigma.binary.getPath(id)` actually does

Tauri command `get_extension_binary_path(extension_id, binary_id, executable_name)`:
1. Resolves binary dir: `extensions/<extension_id>/bin/<binary_id>`
2. Resolves file path: `<binary_dir>/<executable_name>`
3. Returns `Some(path)` if file exists, else `None`

### 5.3 Why round 7's installer put sidecar in the wrong place

Round 7 modeled the installer pattern on `D:\Program Files\Sigma File Manager\bin\focus-sync-sidecar\0.2.0\` (where Tauri bundles binaries in **production install**), but that path is for the Sigma FM main app binaries. **Extension binaries follow a different convention** (extensions/<id>/bin/<id>/).

This was a real bug in round 7's installer that round 9 fixes. Anyone re-running round 7's installer (the 985,825-byte exe from 15:22) would still hit this issue. Use the round 9 exe (986,032 bytes from 15:33:14) instead.

### 5.4 Extension activation failure silent path

`activate()` in extension JS is allowed to bail early without throwing. If `sigma.binary.getPath()` returns null, the function:
- Shows an error toast ("Sidecar binary not installed")
- Returns early
- The extension is **still considered activated** by Sigma FM (UI shows enabled)
- No `sigma.context.onPathChange()` subscription is registered
- No commands are registered
- No toolbar items appear

User sees: "Focus Sync listed and toggle is on, but nothing happens". This matches the bug report.

If the user reports a similar bug in the future, FIRST check `sigma.binary.getPath()` returns non-null. If null, check the binary file exists at the expected path.

---

## 六、本轮改动文件清单

```
MOD  release\extension-installer\installer.nsi              (Stage 4 path + status log)
MOD  release\extension-installer\kizemo.focus-sync-0.2.0-setup.exe  (986,032 bytes)
MOD  release\kizemo.focus-sync-0.2.0-setup.exe               (top-level copy)
NEW  handoff-2026-09-29-sidecar-path-fix.md                 (this file)
NEW  %APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe
     (sidecar at correct path, 2,927,616 bytes)
```

无代码改动在 git repo 内。

---

## 八、新会话第一句话参考

> 承接 `handoff-2026-09-29-sidecar-path-fix.md`(round 9,继承 round 8 isLocal fix)。Root cause:Sigma FM 的 `get_extension_binary_path` 在 `extensions/<id>/bin/<binary_id>/` 找二进制,不是 `binaries/`。Round 7 installer 把 sidecar 装错地方 → activate() 找不到 → 静默 bail → UI 显示 enabled 但实际不工作。修复:sidecar 已复制到正确路径 + installer.nsi Stage 4 改路径 + exe 重建。**下一步:用户启动 Sigma FM,试 Edge 下载弹窗路径自动切换**。