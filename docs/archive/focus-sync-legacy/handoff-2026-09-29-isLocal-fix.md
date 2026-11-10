---
name: 2026-09-29 handoff (round 8) - isLocal fix for Focus Sync startup auto-cleanup
description: Round 8. Discovered removeUnapprovedInstalledExtensions auto-deletes extensions not in marketplace registry. Fix: add isLocal: true + localSourcePath to entry.
type: project
---

# Handoff — isLocal Fix (2026-09-29 round 8)

**Inherits**: `handoff-2026-09-29-extension-installer.md` (round 7 — NSIS installer + register.ps1 written, JSON registered but Sigma FM startup silently re-deleted entry)

**Status**: ✅ **FIX APPLIED + REBUILT + DEPLOYED** — register.ps1 now writes `isLocal: true` and `localSourcePath`. NSIS installer rebuilt. user-extensions.json already updated with fix (47498 bytes, kizemo has `isLocal: True`). Awaiting user verification after Sigma FM restart.

---

## 一、真正根因(本轮确认)

`sigma-file-manager\src\stores\runtime\extensions.ts:1884`:

```typescript
async function removeUnapprovedInstalledExtensions(nextRegistry: ExtensionRegistry) {
    const approvedExtensionIds = new Set(nextRegistry.extensions.map(ext => ext.id));
    const removableIds = Object.entries(installedExtensions)
        .filter(([id, data]) => {
            if (data.isLocal) return false;       // <-- 关键判断
            if (approvedExtensionIds.has(id)) return false;
            ...
        })
        .map(...);

    for (const id of removableIds) {
        await removeInstalledExtension(id, { runUninstallHook: false });  // <-- 自动删除
        showExtensionNoLongerAvailableToast(id, name);
    }
}
```

启动时遍历 `installedExtensions`,凡**不在 marketplace registry** 且 **`isLocal` 不是 true** 的 entry → 自动删除并写回。

Round 7 register.ps1 漏了 `isLocal: true` → 启动时被这个函数当作"未批准"删掉 → 用户看到只有 hetima。

---

## 二、修复(register.ps1)

`release\extension-installer\register.ps1` 增加两个字段:

```powershell
$kizemoEntry = [ordered]@{
    ...
    version = '0.2.0'
    isLocal = $true                                                       # <-- 新增
    localSourcePath = "$env:APPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync"  # <-- 新增
}
```

`isLocal: true` 让 `removeUnapprovedInstalledExtensions` 跳过这个 entry。`localSourcePath` 记录来源路径(可选,用于本地 update 流程)。

---

## 三、本轮验证(已通过)

### 3.1 NSIS 重建

```
==> register.ps1 has isLocal fix: OK
==> Compiling NSIS installer...
[成功]
==> Built: F:\soft\00selfmade\filemanager\release\kizemo.focus-sync-0.2.0-setup.exe
    Size: 985,825 bytes (15:22:25)
```

`release\` 顶层副本同步更新到 985,825 bytes。

### 3.2 文件部署

```
==> Sigma FM is NOT running. Proceeding with redeploy + register.
==> Extension files copied to ...\extensions\kizemo.focus-sync\
==> Sidecar copied
```

5 个文件就位 + sidecar (2,927,616 bytes)。

### 3.3 注册成功

```
==> Backup created: C:\Temp\user-extensions.json.bak-register-20260929-152212
==> Wrote ...\user-extensions.json (47498 bytes)

=== Final verify ===
installedExtensions count: 2
kizemo.focus-sync: PRESENT
  version:         0.2.0
  enabled:         True
  isLocal:         True   <-- KEY FIELD
  localSourcePath: C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync
  manifest.id:     kizemo.focus-sync
  manifest.engines.sigmaFileManager: ^2.2.0
```

---

## 四、用户接下来要做

1. **打开 Sigma FM**(双击 `D:\Program Files\Sigma File Manager\sigma-file-manager.exe`)
2. `Ctrl+Shift+X` → **已安装** tab → 应该看到 **Focus Sync**(作者 kizemo, v0.2.0)
3. 启用 toggle,验证 toolbar 出现 **Sync Now** / **Enable / Disable**
4. 实测 Save As 对话框焦点同步

如果还是看不到,可能 startup auto-cleanup 还有别的路径。下一会话深挖 `removeUnapprovedInstalledExtensions` 的调用链。

---

## 五、技术沉淀(给未来会话)

### 5.1 Sigma FM 注册 extension 必须字段

除了 round 7 §5.2 列出的字段,**还必须**有:

- `isLocal: true` ← **关键,漏掉会被 startup auto-cleanup 删除**
- `localSourcePath: <absolute path to extension dir>` ← 本地 update 流程需要

这两个字段是 `storage/extensions.ts:193` `addInstalledExtension` 函数的标准输入(`options?.isLocal`, `options?.localSourcePath`)。本地 install 必须传 `true`,marketplace install 也建议传。

### 5.2 Startup auto-cleanup 全景

`runtime/extensions.ts` 启动时:
1. `init()` 调用 `storageStore.init()` → 加载 user-extensions.json 到 in-memory
2. `reconcileInstalledExtensions()` → 检测 broken(storagedisk 缺失)
3. **`removeUnapprovedInstalledExtensions()` → 删除非 marketplace 非 local 的 entry**
4. UI 显示 broken entries(在 Installed tab,带警告图标)

第 3 步是 round 7/8 发现的隐藏删除路径。所有 local install 都要过这一关。

### 5.3 NSIS + LZMA 压缩包内嵌验证

`SetCompressor /FINAL /SOLID lzma` 把 `File` 嵌入的文件用 LZMA 压缩。所以 `grep isLocal` 在 exe 二进制里查不到。验证方式:
- 重建前后 size 对比(应略变)
- 用 7-zip 解压 exe,看 `$PLUGINSDIR` 内的文件
- 或用 `7z e kizemo.focus-sync-0.2.0-setup.exe -oC:\Temp\extract` 解压

### 5.4 Local install 的完整字段集(更新版)

```json
{
  "autoUpdate": true,
  "enabled": true,
  "installPendingDependencies": false,
  "installedAt": <unix-ms>,
  "manifest": { ... },
  "settings": {
    "customSettings": { "__binaries": {} },
    "scopedDirectories": []
  },
  "version": "0.2.0",
  "isLocal": true,                                    ← 新增
  "localSourcePath": "$APPDATA\...\extensions\id"     ← 新增
}
```

---

## 六、本轮改动文件清单(都未 commit)

```
MOD  release\extension-installer\register.ps1             (+ isLocal = $true + localSourcePath)
MOD  release\extension-installer\installer.nsi            (no change, but rebuilt)
MOD  release\extension-installer\kizemo.focus-sync-0.2.0-setup.exe  (985,825 bytes, 15:22:25)
MOD  release\kizemo.focus-sync-0.2.0-setup.exe            (top-level copy, 15:22:25)
NEW  handoff-2026-09-29-isLocal-fix.md                    (this file)
MOD  C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-extensions.json
     (47498 bytes, with kizemo + isLocal: True)
```

无代码改动在 git repo 内。

---

## 七、新会话第一句话参考

> 承接 `handoff-2026-09-29-isLocal-fix.md`(round 8,继承 round 7 extension-installer)。真正根因找到:Sigma FM 启动时 `removeUnapprovedInstalledExtensions` 会删除任何**不在 marketplace registry** 且 **`isLocal !== true`** 的 entry。Round 7 register.ps1 漏了 `isLocal: true`。修复已加,exe 已重建 (985,825 bytes),user-extensions.json 已写入 (47498 bytes, kizemo 有 `isLocal: True`)。**下一步:用户启动 Sigma FM → Ctrl+Shift+X → 已安装 tab 应该看到 Focus Sync**。如果还是看不到,需要查 `removeUnapprovedInstalledExtensions` 调用链是否还有其他触发路径。