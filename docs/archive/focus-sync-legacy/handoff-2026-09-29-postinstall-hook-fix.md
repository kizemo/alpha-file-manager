---
name: 2026-09-29 handoff (round 3) - POSTINSTALL hook fix attempt
description: Round 3. Diagnosed root cause: hook didn't copy to %APPDATA%. Refactored POSTINSTALL to use SetOutPath + CopyFiles + diagnostic DetailPrint. Rebuild in progress.
type: project
---

# Handoff — POSTINSTALL Hook Fix (2026-09-29 round 3)

**Inherits**: `handoff-2026-09-29-smart-upgrade-cleanup.md` (round 2 — PREINSTALL smart-upgrade + cleanup old 7z SFX)

**Status at handoff write**: ✅ **BUILD SUCCESS** — New NSIS installer ready at both `release/installer/Sigma-File-Manager-2.2.0-focus-1-setup.exe` (17,097,468 bytes / 16.31 MiB) and `release/installer/Sigma File Manager_2.2.0_x64-setup.exe`. Build took 4m 35s vite + 2m 36s cargo + ~10s makensis. Round 1 build (before the CopyFiles syntax fix) failed at makensis; round 2 build passed.

---

## 一、本轮发现(关键诊断)

### 1.1 用户机器上的真相(精确验证)

| 项 | 实际值 |
|---|---|
| 旧版 Sigma FM 路径 | `D:\Program Files\Sigma File Manager\` |
| 旧版 installer 类型 | Tauri 1.x(写 `HKLM\SOFTWARE\sigma-file-manager\Sigma File Manager\` default = InstallLocation) |
| 新版 installer 装到 | `D:\Program Files\Sigma File Manager\` ✅(同旧版路径) |
| 新版 `sigma-file-manager.exe` (43.9 MB) | 2026/9/29 09:37:54 |
| `D:\Program Files\Sigma File Manager\extensions\kizemo.focus-sync\package.json` | 2026/9/29 11:09:32 ← Tauri bundle.resources 解压 |
| `D:\Program Files\Sigma File Manager\bin\focus-sync-sidecar\0.2.0\focus-sync-sidecar.exe` | 同上 |
| `HKLM\SOFTWARE\sigma-file-manager\Sigma File Manager\(default)` | `D:\Program Files\Sigma File Manager` ✅ |
| `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Sigma File Manager\InstallLocation` | `"D:\Program Files\Sigma File Manager"` ✅ |
| **`%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\`** | **不存在 ❌** |
| **`%APPDATA%\com.sigma-file-manager.app\binaries\`** | **不存在 ❌** |
| Sigma FM 扩展功能页(用户截图) | 只显示 Hetima Light Themes(96.8 KB),**没有 Focus Sync** |
| `appwiz.cpl` Windows Event Log | 2026/9/29 11:09:30 - RestartManager 关闭 Sigma"(installer 跑过) |

### 1.2 根因:POSTINSTALL hook 没复制到 `%APPDATA%`

**PREINSTALL hook 实际有效**:
- Tauri 模板的 `.onInit` 里调 `RestorePreviousInstallLocation` (line 509),从 `MANUPRODUCTKEY` 读旧版路径(在 line 873)
- 这把 `$INSTDIR` 设为 `D:\Program Files\Sigma File Manager\`
- hooks.nsh 的 PREINSTALL 三层 fallback 实际是 dead code — 它读的 key (`com.sigma-file-manager.app_is1`) 不存在,FS probe 用 `$PROGRAMFILES` 也找不到 D: 盘

**POSTINSTALL hook 失败**(实际症状):
- `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\` 子目录**没被 hook 创建**
- 6 个 `CopyFiles` **都没复制**
- Sigma FM 从 `%APPDATA%\...` 加载 extensions,所以 Focus Sync 不显示

**最可能根因**:hooks.nsh line 78-92 的 `CopyFiles source dest` 形式在 silent 模式下失败不报错。具体机制待重跑时通过 `DetailPrint` 日志确认。

### 1.3 handoff §1.1 hook layer 1 key 是错的

hook 读 `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\com.sigma-file-manager.app_is1\InstallLocation`,但 Tauri 实际写到 `HKLM\SOFTWARE\sigma-file-manager\Sigma File Manager\`(default value),以及 `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Sigma File Manager\InstallLocation`(line 687 named value,**不是** `_is1` 后缀)。

**当前结论**:hook 三层 fallback 全部 dead code,实际起作用的是 Tauri `.onInit`。这点对最终用户体验无影响(installer 仍正确装到 D: 盘),但 hook 本身应该清理掉或改对 key。

### 1.4 Round 3 验证:hook 在编译产物里完整存在(本会话新增)

用 `makensis -V4` 跑 Tauri 生成的 installer.nsi,**所有 4 个 hook 都被 `!insertmacro` 处理,macro 内容完整展开**:

```
!insertmacro: NSIS_HOOK_PREINSTALL
DetailPrint: "==> [PREINSTALL HOOK START] $INSTDIR (before detection) = $INSTDIR"
ReadRegStr $0 HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Sigma File Manager_is1\InstallLocation
...
DetailPrint: "==> [PREINSTALL HOOK END] $INSTDIR (after detection) = $INSTDIR"
!insertmacro: end of NSIS_HOOK_PREINSTALL

!insertmacro: NSIS_HOOK_POSTINSTALL
DetailPrint: "==> [POSTINSTALL HOOK START] Deploying kizemo.focus-sync extension + sidecar ..."
SetOutPath: "$APPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync"
CopyFiles: (silent) "$INSTDIR\extensions\kizemo.focus-sync\package.json" -> "$APPDATA\...", size=0KB
... (其他 5 个 CopyFiles 都是 size=0KB)
DetailPrint: "==> [POSTINSTALL HOOK END] Deployed to: $APPDATA\..."
!insertmacro: end of NSIS_HOOK_POSTINSTALL
```

**`!ifmacrodef` 测试**:独立最小 NSIS 测试证实 `!ifmacrodef NSIS_HOOK_POSTINSTALL` 在 `!macro NSIS_HOOK_POSTINSTALL` 定义后**正确返回 TRUE**,`!insertmacro` 实际展开 macro 内容。

**`size=0KB` 是 NSIS 编译期对含 `$INSTDIR` runtime 变量的源文件无法计算大小,不代表 runtime 失败**。

### 1.5 用户的 installer 状态(round 3:helper 已确认)

| 文件 | 大小 | 时间 | 备注 |
|---|---|---|---|
| `release/Sigma-File-Manager-2.2.0-focus-1-setup.exe` (顶层) | 17,096,151 bytes | Sep 29 09:38 | **Round 2 旧版**,**未被 round 3 覆盖** ❌ |
| `release/installer/Sigma-File-Manager-2.2.0-focus-1-setup.exe` | 17,097,468 bytes | Sep 29 12:43 | **Round 3 新版** ✅ |
| `release/installer/Sigma File Manager_2.2.0_x64-setup.exe` | 17,097,468 bytes | Sep 29 12:43 | Round 3 新版 ✅ |
| `D:\Program Files\Sigma File Manager\uninstall.exe` | 82,079 bytes | Sep 29 11:09 | 时间戳早于 round 3 build(11:09 是 round 2 重装时间) |

**结论**:用户可能**跑过 round 2 旧版 installer**(因为顶层路径文件名相同但版本不同),或者**根本没跑 round 3 installer**。

### 1.6 已采取的紧急 workaround(round 3:helper)

helper 用 PowerShell 直接从 `D:\Program Files\Sigma File Manager\` 复制到 `%APPDATA%\com.sigma-file-manager.app\`:
- `extensions\kizemo.focus-sync\package.json` (3165 B) + dist/index.js (7334 B) + dist/index.js.map (4263 B) + locales/en.json (772 B) + locales/zh-CN.json (820 B)
- `binaries\focus-sync-sidecar\0.2.0\focus-sync-sidecar.exe` (2,927,616 B)

**等用户重启 Sigma FM 验证 Focus Sync 是否出现**。

---

## 二、本轮修复(已完成,等 build)

### 2.1 POSTINSTALL hook 改用 `SetOutPath + CopyFiles`

旧写法(`CopyFiles source dest`):
```nsh
CopyFiles /SILENT "$INSTDIR\extensions\...\package.json" \
                  "$APPDATA\...\package.json"
```

新写法:
```nsh
SetOutPath "$APPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync"
CopyFiles /SILENT "$INSTDIR\extensions\kizemo.focus-sync\package.json"
```

`SetOutPath` 把 out path 设为目标目录,然后 `CopyFiles` 只用源路径,NSIS 自动复制到当前 SetOutPath。这避免了"目标父目录不存在时 CopyFiles silent 失败"的可能。

### 2.2 PRE/POSTINSTALL/POSTUNINSTALL 加 DetailPrint 诊断日志

每步都加 `DetailPrint "==> [HOOK NAME HOOK START] ..."` 让重跑 installer 时能在 NSIS 日志看到 hook 是否真的执行。

### 2.3 文件位置

`F:\soft\00selfmade\filemanager\sigma-file-manager\src-tauri\installer\hooks.nsh` — 已修改(POSTINSTALL line 75-103,PREINSTALL line 23-73,POSTUNINSTALL line 109-114)

---

## 三、当前 build 状态

- 命令: `cargo-tauri build --bundles nsis`
- 环境: rustup 1.90.0 toolchain on PATH (`~/.rustup/toolchains/1.90.0-x86_64-pc-windows-msvc/bin`)
- 镜像: rsproxy.cn (`RUSTUP_DIST_SERVER=https://rsproxy.cn RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup`)
- 进度: vite build ✅ 5m 7s, cargo build 🔄 编译中
- 后台 task: `b4cz109iw`

### Build 完成后

新 installer 预期生成在:
- `F:\soft\00selfmade\filemanager\sigma-file-manager\src-tauri\target\release\bundle\nsis\Sigma File Manager_2.2.0_x64-setup.exe`
- 然后需要复制到 `F:\soft\00selfmade\filemanager\release\installer\`(顶层副本命名 `Sigma-File-Manager-2.2.0-focus-1-setup.exe`)

---

## 四、用户测试 checklist(build 完后)

### 4.1 卸载旧版 + 重装新版

```cmd
appwiz.cpl
```
- 找到 Sigma File Manager → 右键卸载
- 卸载时观察 NSIS 日志是否打印 `[POSTUNINSTALL HOOK START]` / `[POSTUNINSTALL HOOK END]` — 确认 hook 跑了

然后跑新 installer:
```
F:\soft\00selfmade\filemanager\release\installer\Sigma-File-Manager-2.2.0-focus-1-setup.exe
```
(等 build 完成 + 复制后再用)

### 4.2 验证 hook 部署

```powershell
Get-ChildItem "$env:APPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync" -Recurse -Force
Get-ChildItem "$env:APPDATA\com.sigma-file-manager.app\binaries" -Recurse -Force
```

预期:
- `extensions\kizemo.focus-sync\` 下有 `package.json` + `dist\index.js` + `dist\index.js.map` + `locales\en.json` + `locales\zh-CN.json`
- `binaries\focus-sync-sidecar\0.2.0\focus-sync-sidecar.exe`

### 4.3 启动 Sigma FM,验证 Focus Sync 出现

1. 启动新装的 Sigma
3. Ctrl+Shift+X → 已安装 tab
4. 预期: Focus Sync(96.8 KB 那种大小)出现在已安装列表,启用 toggle 正常

### 4.4 卸载验证 cleanup

```powershell
# 卸载后
Test-Path "$env:APPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync"
Test-Path "$env:APPDATA\com.sigma-file-manager.app\binaries"
```
预期: 都 False(被 POSTUNINSTALL hook 清理)

---

## 五、避坑(下次会话必读)

1. **不要再 amort PREINSTALL 三层 fallback** — 它们是 dead code,实际起作用的是 Tauri `.onInit` 的 `RestorePreviousInstallLocation`(读 `MANUPRODUCTKEY`)。如果用户报"装到 C:\Program Files",**那是因为旧版没写过 MANUPRODUCTKEY**(比如手动复制安装),这时 PREINSTALL 的 FS probe 才会起作用 — 但 FS probe 也只查 `$PROGRAMFILES` = C 盘,所以也找不到 D: 盘 → 真修时要扩展 FS probe 探测多盘符。
2. **本次修复核心**:`SetOutPath + CopyFiles`(不再用 `CopyFiles source dest`),因为 silent 失败是源+目标组合下的潜在陷阱。
3. **不要 commit** — 用户的 git 状态显示 `release/` 和 `.other/` 都是 untracked,本次修复也只动了 hooks.nsh。等用户决定。
4. **如果重装后 hook 还是没复制** — 进一步诊断:
   - 看 NSIS 安装日志是否打印 `[POSTINSTALL HOOK START]`,如果没打印,说明 `!ifmacrodef NSIS_HOOK_POSTINSTALL` 评估为 false
   - 这时需要 patch Tauri 模板或在 hooks.nsh 末尾加 `!define NSIS_HOOK_POSTINSTALL` 让 macro 一定被识别
5. **如果 `pnpm: command not found`** — pnpm 不在 PATH,用 `cargo-tauri build --bundles nsis` 直接调(把 `~/.rustup/toolchains/1.90.0-x86_64-pc-windows-msvc/bin` 加到 PATH)

---

## 六、本次改动文件清单(都未 commit)

```
 M sigma-file-manager/src-tauri/installer/hooks.nsh         (POSTINSTALL 用 SetOutPath + CopyFiles;PRE/POST/POSTUNINSTALL 加 DetailPrint)
?? release/installer/Sigma-File-Manager-2.2.0-focus-1-setup.exe  (待 build 完覆盖)
?? release/installer/Sigma File Manager_2.2.0_x64-setup.exe     (待 build 完覆盖)
```

继承自 round 2 的未 commit 改动(未在本轮触动):
- `sigma-file-manager/src-tauri/tauri.conf.json` (perMachine installMode)
- `sigma-file-manager/package.json` + `cargo.lock` + `cargo.toml`
- `sigma-file-manager/src-tauri/installer/hooks.nsh` 的 round 2 三层 fallback(本次保留但加诊断 log)
- `release/extension/`(staging)
- `release/README.md`
- meta-repo 的 `handoff-*.md` + `release/`(顶层)

---

## 七、新会话第一句话参考

> 承接 `handoff-2026-09-29-postinstall-hook-fix.md`(继承 round 2 `handoff-2026-09-29-smart-upgrade-cleanup.md`)。POSTINSTALL hook 改用 `SetOutPath + CopyFiles` 并加诊断 log,build 在后台进行。下一步:build 完复制到 release/ + 用户卸载+重装新版 + 验证 hook 部署到 `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\` + Sigma FM 启动看 Focus Sync 出现。