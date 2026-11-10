---
name: 2026-09-29 handoff (round 7) - Standalone extension installer + JSON registration
description: Round 7. Built standalone NSIS installer kizemo.focus-sync-0.2.0-setup.exe (962 KB). register.ps1 written, executed, JSON validated. Awaiting user verification after Sigma FM restart.
type: project
---

# Handoff — Standalone Extension Installer (2026-09-29 round 7)

**Inherits**: `handoff-2026-09-29-focus-sync-json-patch.md` (round 6 — manual JSON patch was overwritten by Sigma FM's in-memory state at 13:39:14, even though Sigma FM was running)
**Inherits chain**: round 6 patch → round 5 root cause → round 4 POSTINSTALL hook → round 3 build → round 2 smart upgrade → round 1 base

**Status**: ✅ **INSTALLER READY + REGISTRATION DONE** — `kizemo.focus-sync-0.2.0-setup.exe` (962 KB) at `release/`. `register.ps1` already executed (Sigma FM was closed). JSON validated. Awaiting user verification after Sigma FM restart.

---

## 一、用户上轮反馈

> "仍然没有出现插件。"

Round 6 的手动 JSON patch 在用户启动 Sigma FM 后被覆盖(13:39:14 文件被 Sigma FM 改回只有 hetima)。用户要求改用独立 exe 安装方案。

---

## 二、本轮交付

### 2.1 Standalone NSIS Installer

| 项 | 值 |
|---|---|
| 文件 | `release\kizemo.focus-sync-0.2.0-setup.exe` |
| 大小 | 985,654 bytes (962 KB) |
| 路径 | `release\` 顶层(易访问) |
| 副本 | `release\extension-installer\` |
| 源 | `release\extension-installer\installer.nsi` |
| 构建 | `release\extension-installer\build.ps1` |
| NSIS | `C:\Program Files (x86)\NSIS\makensis.exe` |

### 2.2 Source structure (`release\extension-installer\`)

```
installer.nsi     - NSIS installer source (3 stages: file copy + PowerShell register + WriteUninstaller)
register.ps1      - Add kizemo.focus-sync to user-extensions.json
unregister.ps1    - Remove kizemo.focus-sync from user-extensions.json
build.ps1         - Wrapper around makensis (also copies to release/ top-level)
README.md         - Usage docs (Chinese + install/uninstall/troubleshooting)
```

### 2.3 Installer flow (NSIS)

1. **Stage 1**: Detect running Sigma FM (PowerShell `Get-Process`, exit code 0/1). Warn if running.
2. **Stage 2**: Create `$LOCALAPPDATA\Programs\kizemo.focus-sync\` for installer metadata.
3. **Stage 3**: Copy extension files to `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\`.
4. **Stage 4**: Copy sidecar to `%APPDATA%\com.sigma-file-manager.app\binaries\focus-sync-sidecar\0.2.0\`.
5. **Stage 5**: Run `register.ps1` via PowerShell to write user-extensions.json entry.
6. **Stage 6**: `WriteUninstaller` to `$INSTDIR\uninst.exe`.
7. **Finish page**: Instructions to open Sigma FM and verify.

### 2.4 Register.ps1 logic

1. Backup `user-extensions.json` to `C:\Temp\user-extensions.json.bak-register-<ts>`.
2. Read JSON, parse via `ConvertFrom-Json`.
3. Build kizemo entry with all manifest fields (id, name, version, description, publisher, repository, license, icon, categories, tags, extensionType, main, type, engines, permissions, activationEvents, binaries, contributes).
4. If existing entry → preserve enabled/autoUpdate, refresh manifest.
5. Else → add new entry.
6. Serialize via `ConvertTo-Json -Depth 100` (PowerShell default 4-space; Sigma FM re-normalizes on next save).
7. Write back with UTF-8 no BOM.

---

## 三、本轮验证(已通过)

Sigma FM 在执行 register.ps1 前**已关闭**,所以 JSON 写入不会立刻被覆盖。

```
==> Backup created: C:\Temp\user-extensions.json.bak-register-20260929-134742
==> Reading C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-extensions.json
==> Added new kizemo.focus-sync entry
==> Wrote ...\user-extensions.json (47244 bytes)
```

JSON parse OK. `installedExtensions` 现在有 2 个 entry:`hetima.light-themes` + `kizemo.focus-sync`。所有 manifest 字段验证完整:
- `manifest.id = kizemo.focus-sync`
- `manifest.name = Focus Sync`
- `manifest.version = 0.2.0`
- `manifest.engines.sigmaFileManager = ^2.2.0`
- `manifest.extensionType = api`
- `manifest.main = dist/index.js`
- `manifest.binaries[0].id = focus-sync-sidecar`
- `manifest.contributes.commands[0].id = focus-sync.syncNow`
- `settings.customSettings.__binaries = {}`, `settings.scopedDirectories = []`

所有 5 个扩展文件 + sidecar 都在 `%APPDATA%` 就位。

---

## 四、发现的 manifest 已知 bug(非阻塞)

kizemo.focus-sync 的 package.json 里的 sidecar sha256 **与实际不匹配**:
- Manifest 声明: `sha256:ca0107da383fe08eb651eb6e1d7abb2579c634f058fe8cb7db36eeaf7b21dc94`
- 实际 sidecar: `sha256:98b7b180907ad1b1f87b30bfab55c9fad8a47f4e67bdca201145e81b9749b848`

**影响**: 仅影响 marketplace 下载校验 (`install.rs` 中 `download_to_file_across_urls_with_retry` 才传 `integrity`)。**本地 install 不校验**(`install_local_extension` 不传 hash)。所以本次本地安装可以正常进行。

**修复建议** (给 kizemo maintainer): 重 build sidecar 后,重算 sha256 写到 package.json。

---

## 五、用户接下来要做

1. **打开 Sigma File Manager**(双击 `D:\Program Files\Sigma File Manager\sigma-file-manager.exe`)
2. **Ctrl+Shift+X** → **已安装** tab
3. 应该看到 **Focus Sync**(作者 kizemo, version 0.2.0)
4. 启用 toggle,验证 toolbar 出现 **Sync Now** / **Enable / Disable**
5. 实测 focus sync 功能(打开 Notepad/Chrome/Word 的 Save As 对话框,在 Sigma 切目录,看对话框是否自动跳)

---

## 六、备选 install 流程(已被 round 6 验证可行)

如果 round 6 的手动 patch 也被 Sigma FM 覆盖过,说明**只在 Sigma FM 关闭时改文件**才有效。`register.ps1` 直接调(不走 NSIS UI)等同于 exe 的 Stage 5,效果一样。本会话已经手动跑过一次 register.ps1(Sigma FM 已关),所以即便用户不再跑 exe,效果也应该一致。

如果用户想用 exe 重做一次干净安装:
1. **先关闭 Sigma FM**
2. 双击 `release\kizemo.focus-sync-0.2.0-setup.exe`
3. 跟着向导走
4. 重新打开 Sigma FM

---

## 七、技术沉淀(给未来会话)

### 7.1 Sigma FM storage model(确认)

- **数据源**: `%APPDATA%\com.sigma-file-manager.app\user-data\user-extensions.json` 的 `installedExtensions` 字段
- **Load 机制**: Tauri Rust `get_startup_storage_bootstrap` 在 startup 时一次性读文件 → JS `applyExtensionsDataRecord` → in-memory store
- **Save 机制**: 任何 UI 交互 / 周期 flush 都会 in-memory state → user-extensions.json (覆盖文件)
- **关键陷阱**: Sigma FM 运行时,改 disk 文件无效 — 下次 save 会用 in-memory stale state 覆盖

### 7.2 user-extensions.json 格式要求(Sigma FM 期望)

- Top-level keys 按字母序:`customBinaryPreferences, installedExtensions, recentCommandIds, registryCache, sharedBinaries`
- 缩进: 2 spaces(但其他格式也能被正确解析,Sigma FM 会 re-normalize)
- UTF-8, no BOM
- `installedExtensions[id]` 必须含:`autoUpdate, enabled, installPendingDependencies, installedAt, manifest, settings, version`

### 7.3 NSIS 安装 extension 的固定套路

```
Section "Install":
    SetOutPath "$APPDATA\com.sigma-file-manager.app\extensions\${ID}"
    File "...\package.json"

    SetOutPath "$APPDATA\com.sigma-file-manager.app\extensions\${ID}\dist"
    File "...\index.js" "...\index.js.map"

    SetOutPath "$APPDATA\com.sigma-file-manager.app\extensions\${ID}\locales"
    File "...\en.json" "...\zh-CN.json"

    SetOutPath "$APPDATA\com.sigma-file-manager.app\binaries\${SIDECAR}\${VER}"
    File "...\sidecar.exe"

    SetOutPath "$INSTDIR"  ; installer metadata
    File "register.ps1"

    nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\register.ps1"'
    WriteUninstaller "$INSTDIR\uninst.exe"
SectionEnd
```

### 7.4 NSIS + UTF-8 注意事项

- NSIS source 需要 UTF-8 **with BOM** 才能正确处理非 ASCII 字符(注释里的中文、em-dash 等)
- Bash 命令行 inline `$` 时,NSIS preprocessor 会替换为变量 → 用 `$$p` 才是字面 `$p`(给 PowerShell 用)
- `MessageBox ... IDYES +2` 的 `+2` 跳法容易错 → 用 `${If} $0 == "7"` (IDNO) 更清晰
- 不需要 MUI_ICON 时省略(用 MUI 默认),避免 `${NSISDIR}\Contrib\Graphics\Icons\` 路径依赖

### 7.5 PowerShell ConvertTo-Json 缩进坑

- 默认 4-space base + 字段对齐 padding(不是纯 4-space)
- 不能用简单 regex `(?m)^( {4})` 替换为 `  ` → 嵌套层会产生诡异缩进
- **结论**: 不规范化缩进,Sigma FM 自己会 re-normalize。JSON 内容对就行。

### 7.6 mss 截图工具当前不可用

- 错误: `SyntaxError: source code string cannot contain null bytes`
- 影响: 不能截图验证 Sigma FM UI
- 替代: 用户手动验证后回报

---

## 八、本轮改动文件清单(都未 commit)

```
NEW  release\kizemo.focus-sync-0.2.0-setup.exe          (962 KB, NSIS installer)
NEW  release\extension-installer\installer.nsi          (NSIS source, BOM UTF-8)
NEW  release\extension-installer\register.ps1           (PowerShell register)
NEW  release\extension-installer\unregister.ps1         (PowerShell unregister)
NEW  release\extension-installer\build.ps1              (NSIS build wrapper)
NEW  release\extension-installer\README.md              (Chinese usage docs)
NEW  release\extension-installer\kizemo.focus-sync-0.2.0-setup.exe  (build output, same as release/)
NEW  handoff-2026-09-29-extension-installer.md          (this file)
MOD  C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-extensions.json
     (+ kizemo.focus-sync entry, 47244 bytes, written by register.ps1)
MOD  C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\*  (already in place from round 4)
```

无代码改动在 git repo 内。exe + handoff 都 untracked 等用户决定。

---

## 九、新会话第一句话参考

> 承接 `handoff-2026-09-29-extension-installer.md`(round 7,继承 round 6 patch)。独立 NSIS installer `kizemo.focus-sync-0.2.0-setup.exe` 已构建并复制到 `release/` 顶层。register.ps1 已在本会话执行一次(Sigma FM 已关闭),JSON 验证通过。**下一步:用户启动 Sigma FM → Ctrl+Shift+X → "已安装" tab 应该看到 Focus Sync**。如果还是看不到,下一步排查方向:Sigma FM startup 日志 / manifest schema 严格校验 / 文件 hash 不匹配问题(round 7 §4)。