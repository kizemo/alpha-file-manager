---
name: 2026-09-29 handoff (round 11) - Caret range + comment sandbox fix
description: Round 11. Two fixes: 1) engines caret ^ not supported by Sigma FM (must use >=2.2.0 etc.). 2) Comments listing sandbox patterns themselves trigger the validator. Sidecar still not spawning - need new console error.
type: project
---

# Handoff — Engine Caret + Comment Sandbox Fix (2026-09-29 round 11)

**Inherits**: `handoff-2026-09-29-sandbox-fix.md` (round 10 — DevTools showed caret compat error)

**Status**: ⚠️ **TWO FIXES APPLIED** — engines `^X.Y.Z` → `>=X.Y.Z`, sandbox-triggering strings removed from JS comments. **Sidecar still not spawning**. Need user to recheck DevTools console for new error.

---

## 一、Round 10 DevTools error(用户报告)

```
Failed to load extension kizemo.focus-sync: Extension "kizemo.focus-sync" requires 
Sigma File Manager ^2.2.0 (current: 2.2.0) and extension API ^1.0.0 (current: 1.11.0)
```

## 二、Root cause:`parseVersionComparator` 不支持 `^` caret

`sigma-file-manager/src/modules/extensions/runtime/validation.ts:295`:
```typescript
const match = value.trim().match(/^(<=|>=|<|>|=)?\s*(\d+\.\d+\.\d+(?:-[\w.]+)?)$/);
```

Only operators `<=` `>=` `<` `>` `=` are accepted. `^X.Y.Z` (npm semver caret) returns null → `isVersionCompatibleWithRange` returns false.

**Confirmed by subagent**: Sigma FM does NOT support `^` or `~`. Real extensions like `hetima.light-themes` use `">=2.0.0"`. Examples:
- `hetima.light-themes`: `"engines": { "sigmaFileManager": ">=2.0.0" }`
- `kizemo.batch-rename`: `"engines": { "sigmaFileManager": ">=2.2.0" }`

**Fix**: Changed all engines to `>=`:
- `release/extension/package.json`: `^2.2.0` → `>=2.2.0`, `^1.0.0` → `>=1.0.0`
- `release/extension-installer/register.ps1`: same change in embedded manifest
- `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-extensions.json`: patched via `C:\Temp\patch-engines.ps1` (script also simulates compat check, confirms True for both)

## 三、Root cause #2:我的注释本身触发 sandbox

After engines fix, sidecar still didn't spawn. Investigating revealed my own source code comments were triggering sandbox validation:

```
* DO NOT use these patterns (case-insensitive, regex-based):
*   eval, new Function, .constructor('..."..."..."..."), document.,
*   window., localStorage., sessionStorage., indexedDB., fetch(, XMLHttpRequest,
*   WebSocket, self, globalThis, postMessage(, addEventListener(,
*   removeEventListener(, dispatchEvent(, importScripts, navigator
```

The warning I wrote (to document sandbox constraints) listed all 18 forbidden patterns → ALL matched in regex check → `validateExtensionCode` returned 17 errors → extension silently failed.

Also line: `a notification string containing "Sigma navigator" will fail validation` — even the explanation itself matched `navigator` regex.

**Fix**: Removed the entire "IMPORTANT — sandbox constraints" + "Source attribution" + "Naming history" sections from `release/extension/dist/index.js`. Verified with `C:\Temp\scan-source.ps1` — **0 patterns remaining in source**.

Also deploys patched JS to `%APPDATA%\extensions\kizemo.focus-sync\dist\index.js` (6389 bytes, no dangerous patterns).

## 四、Current state(after both fixes)

| Item | Value |
|---|---|
| `manifest.engines.sigmaFileManager` | `>=2.2.0` |
| `manifest.engines.extensionApi` | `>=1.0.0` |
| `isLocal` | `true` |
| `enabled` | `true` |
| `__binaries['focus-sync-sidecar'].path` | correct path |
| `dist/index.js` source | 0 sandbox patterns |
| Sidecar process | **NOT running** |
| Port 37421 | NOT listening |
| Sigma FM | running (PID 40500, started 16:15:46) |

Even with **819-byte minimal test JS** (`sigma.shell.runWithProgress(SIDECAR_PATH, ...)` only), sidecar doesn't spawn. This strongly suggests another gate is blocking.

## 五、User action required

**Critical**: previous DevTools error (`requires Sigma File Manager ^2.2.0...`) was for the OLD engines. After my engines fix, that error is gone. Need to recheck console for **new** error.

1. **Close Sigma FM** completely (right-click taskbar → Quit)
2. **Reopen Sigma FM** (so bootstrap reloads with new engines)
3. **Open DevTools**: Ctrl+Shift+I (or F12)
4. **Console tab** → look for red errors mentioning `kizemo.focus-sync`
5. **Report the new error message** (it'll be different from `^2.2.0`)

If error gone and sidecar still not spawning, the next likely culprits are:
- `read_extension_file` failure (unlikely — file is 6.4KB)
- `validateExtensionCode` failure from a different pattern I haven't identified
- `runWithProgress` Tauri command silently rejecting (e.g., extension not authorized)
- Worker module import failure (e.g., MIME type, sandbox worker)
- Cached bootstrap with stale state (despite file being on disk)

## 六、技术沉淀

### 6.1 Sigma FM version range syntax(confirmed)

**Supported**:
- `=X.Y.Z` (or just `X.Y.Z` — no operator defaults to `=`)
- `>=X.Y.Z`, `<=X.Y.Z`, `>X.Y.Z`, `<X.Y.Z`
- Multiple comparators joined by spaces/commas: `">=2.2.0 <3.0.0"`
- Prerelease forms: `X.Y.Z-tag` (e.g., `2.0.0-beta.1`)

**NOT supported**:
- `^X.Y.Z` (npm caret)
- `~X.Y.Z` (npm tilde)
- Wildcards `*`, `x`
- Range syntax `1.x` or `1.2.x`

`parseVersionComparator` regex: `/^(<=|>=|<|>|=)?\s*(\d+\.\d+\.\d+(?:-[\w.]+)?)$/`

### 6.2 Sandbox validator matches comments and string literals(confirmed)

`validateExtensionCode` uses `pattern.test(code)` where `code` is the raw JS string. Patterns like `navigator\b` match anywhere — code, comments, string literals. A documentation comment listing forbidden patterns itself fails validation.

**Safe documentation alternatives**:
- Don't list forbidden patterns in comments
- Don't put words like "navigator" in user-facing strings (use "folder", "current folder", "Sigma FM")
- Don't mention "WebSocket" or "XMLHttpRequest" in code or strings (use "sidecar", "backend", "API")

### 6.3 Sigma FM extension developer resources(confirmed by subagent)

- **Extension API package**: `aleksey-hoffman/sigma-file-manager/tree/main/packages/api`
- **Manifest schema**: `https://raw.githubusercontent.com/aleksey-hoffman/sigma-file-manager/main/packages/api/manifest.schema.json`
- **Marketplace repo**: `sigma-hub/sfm-marketplace` (wiki may have tutorials)
- **Working examples**:
  - `kizemo/sigma-batch-rename` — full api extension with permissions, commands, contextMenu, build (tsc + esbuild)
  - `hetima/sigma-light-themes` — theme-only api extension (no main)

## 七、本轮改动文件清单

```
MOD  release\extension\package.json                 (engines ^ → >=)
MOD  release\extension-installer\register.ps1      (engines ^ → >=)
MOD  release\extension\dist\index.js              (removed sandbox-pattern comments)
MOD  release\extension-installer\kizemo.focus-sync-0.2.0-setup.exe   (985,986 bytes)
MOD  release\kizemo.focus-sync-0.2.0-setup.exe    (top-level copy)
MOD  C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-extensions.json
     (engines updated to >=2.2.0, preserved through Sigma FM rewrites)
MOD  C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\dist\index.js
     (cleaned JS deployed)
```

## 八、新会话第一句话参考

> 承接 `handoff-2026-09-29-engine-caret-fix.md`(round 11)。修了两个 bug:`engines` 从 `^X.Y.Z` 改为 `>=X.Y.Z`(Sigma FM 不支持 caret);我自己注释里的 sandbox pattern 列表本身触发了 validator。**Sidecar 仍不 spawn,等用户重看 DevTools 拿新错误**。可能还有别的 gate:read_extension_file/validateExtensionCode/runWithProgress/worker import/bootstrap cache。