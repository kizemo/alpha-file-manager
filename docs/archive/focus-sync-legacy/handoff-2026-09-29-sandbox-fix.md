---
name: 2026-09-29 handoff (round 10) - Sandbox 'navigator' string fix + __binaries injection
description: Round 10. Fixed two more bugs: 1) __binaries field was empty (sigma.binary.getPath returned null). 2) Extension code contained "navigator" string which tripped validateExtensionCode sandbox. Still need user DevTools check.
type: project
---

# Handoff — Sandbox Fix + __binaries (2026-09-29 round 10)

**Inherits**: `handoff-2026-09-29-sidecar-path-fix.md` (round 9 — sidecar at wrong path)

**Status**: ⚠️ **PARTIAL FIX** — `__binaries` populated + sandbox `navigator` string removed from dist/index.js. **Sidecar still not spawning** despite all fixes. Need DevTools console inspection to identify the actual error.

---

## 一、Root cause #1:`sigma.binary.getPath` 返回 null

`create-binary-api.ts:47-51`:
```typescript
getPath: async (id: string): Promise<string | null> => {
    ensureBinaryPermission();
    const info = await resolveInstalledBinaryInfo(id);
    return info?.path ?? null;  // <-- returns null if __binaries[id] is empty
}
```

`resolveInstalledBinaryInfo` reads from `settings.customSettings.__binaries[id]`. For local installs (where the user copies the binary directly without going through Sigma FM's binary download flow), this is **empty by default**. So `getPath` returns null → `activate()` bails with "Sidecar binary not installed" notification.

**Fix**: Manually populated `__binaries['focus-sync-sidecar'].path` in `user-extensions.json`. PowerShell script at `C:\Temp\fix-binary-storage.ps1`. Backup at `C:\Temp\user-extensions.json.bak-kizemo-20260929-152212`.

After this:
- `__binaries['focus-sync-sidecar']` has `{id, path, version, installedAt, source: 'local'}`
- `getPath` should now return the path
- Sidecar SHOULD have spawned, but didn't — see Root cause #2

## 二、Root cause #2:extension code sandbox 校验失败

`sandbox.ts:201-289` `validateExtensionCode` regex-checks extension JS for dangerous patterns:
- `eval(`, `new Function(`, `.constructor('..."`
- `document.`, `window.`, `localStorage.`, `sessionStorage.`, `indexedDB.`, `fetch(`
- `XMLHttpRequest`, `WebSocket`, `self`, `globalThis`
- `postMessage(`, `addEventListener(`, `removeEventListener(`, `dispatchEvent(`
- `importScripts`, **`navigator`**

The regex `(?<![\w$.])navigator\b` matches the word "navigator" not preceded by word/$. characters. **It matches inside string literals too**, not just code identifiers.

Original `dist/index.js:46` had:
```javascript
sigma.ui.showNotification({ title: 'Focus Sync', description: 'No current path in Sigma navigator.', type: 'warning' });
```

→ Validation fails: `"Direct navigator access is not allowed"`
→ `loadApiExtension` throws `Error: Extension code validation failed: Direct navigator access is not allowed`
→ Error caught in `init()` at line 2085 with `console.error`
→ Extension never runs `activate()`, sidecar never spawns

**Fix**: Replaced `'No current path in Sigma navigator.'` with `'No current folder in Sigma FM.'`. Source map regenerated.

## 三、Sidecar still doesn't spawn after both fixes

After both fixes:
- ✅ Sidecar at correct path: `extensions/kizemo.focus-sync/bin/focus-sync-sidecar/focus-sync-sidecar.exe`
- ✅ `__binaries` populated with valid path
- ✅ Sandbox `navigator` regex no longer matches
- ❌ **Sidecar process still not running after 20s wait**

Even an ultra-minimal test JS (921 bytes, just calls `sigma.shell.runWithProgress(SIDECAR_PATH, ['--ipc', 'http', '--port', '37421'], () => {})`) **does not spawn sidecar**.

This strongly suggests something else is wrong **upstream of activate()**:
- Either `loadExtension` errors before `activate()` is called
- Or the worker module fails to import for some other reason
- Or there's another sandbox pattern I haven't identified

**Cannot determine without DevTools console access.** `console.error` output goes to Sigma FM's webview DevTools (Ctrl+Shift+I in Sigma FM), not to stdout/file.

## 四、User action required

User must open Sigma FM's DevTools console to see the actual error:

1. **Open Sigma FM**
2. Press **Ctrl+Shift+I** (DevTools opens; verified via process tree, PID 41820 "DevTools - tauri.localhost/")
3. Click **Console** tab
4. Look for red error messages about kizemo.focus-sync:
   - `"Extension code validation failed: ..."`
   - `"Failed to load extension kizemo.focus-sync: ..."`
   - Anything else red
5. **Report the error message** so we can fix the next layer

## 五、What I tried that should have worked

| Approach | Result |
|---|---|
| Hardcoded SIDECAR_PATH (bypass `sigma.binary.getPath`) | Sidecar still not spawned |
| Minimal JS (921 bytes, no dangerous patterns) | Sidecar still not spawned |
| Populated `__binaries` field | File accepted, but no spawn |
| Removed `'navigator'` string | File accepted, but no spawn |
| Multiple Sigma FM restarts | Each starts, writes file, no spawn |

The pattern of "all fixes applied, file accepted, but no spawn" suggests an issue **outside the extension JS itself**.

## 六、沉淀(给未来会话)

### 6.1 Sigma FM extension sandbox patterns(confirmed)

All 18 regex patterns in `validateExtensionCode` check code **AND string literals**:
1. `eval(`
2. `new Function(`
3. `.constructor(['"\`])` (single quote + open paren)
4. `document.`
5. `window.`
6. `localStorage.`
7. `sessionStorage.`
8. `indexedDB.`
9. `fetch(`
10. `XMLHttpRequest`
11. `WebSocket`
12. `(?<![\w$.])self\b` (no word/dot before)
13. `(?<![\w$.])globalThis\b`
14. `(?<![\w$.])postMessage\(`
16. `(?<![\w$.])addEventListener\(`
17. `(?<![\w$.])removeEventListener\(`
18. `(?<![\w$.])dispatchEvent\(`
19. `importScripts`
20. `(?<![\w$.])navigator\b`

**Safe word alternatives**:
- `navigator` → `folder`, `current folder`, `Sigma FM`
- `window` → `context`
- `self` → `me`, `this`
- `globalThis` → `context`
- `fetch` → `sigma.http.request`
- `localStorage` → `sigma.storage`

### 6.2 Sigma FM binary registration model(updated)

For local installs, **TWO** registrations required:

1. **Disk file**: `extensions/<id>/bin/<binary_id>/<executable>` (from round 9)
2. **`__binaries` storage**: `settings.customSettings.__binaries[binary_id] = {id, path, version, installedAt}` (from round 10)

`sigma.binary.getPath(id)` reads ONLY from storage, not disk. So local installs must populate storage manually or via UI install flow.

### 6.3 Sigma FM UI vs actual extension code

UI display state (toggle on/off, enabled field) is **decoupled** from extension code load state:
- UI reads from `storageStore.extensionsData.installedExtensions[id].enabled`
- Code load is triggered by `init()` → `loadExtension(extension.id, 'onStartup')`
- A failed load (sandbox, engine compat, etc.) does NOT change the `enabled` field
- User sees "extension enabled" but no actual functionality

## 七、本轮改动文件清单

```
MOD  release\extension\dist\index.js                 (removed 'navigator' string)
MOD  release\extension\dist\index.js.map             (regenerated)
MOD  C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-extensions.json
     (added __binaries['focus-sync-sidecar'].path entry)
```

无代码改动在 git repo 内(extension 源码在 release/extension/ 是构建产物)。

---

## 八、新会话第一句话参考

> 承接 `handoff-2026-09-29-sandbox-fix.md`(round 10)。修了两个 bug:`__binaries` 注入 + 'navigator' 字符串(sandbox 已禁)。但 sidecar 仍不 spawn。**最关键**:用户需要在 Sigma FM DevTools(Ctrl+Shift+I 已开,PID 41820)看 Console 红字错误,告诉我错误内容。可能还有未发现的 sandbox 模式,或 loadExtension 链上游的另一个 gate。