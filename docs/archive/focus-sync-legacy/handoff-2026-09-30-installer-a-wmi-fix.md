# Handoff — Round 19d: WMI-based original user APPDATA resolution (2026-09-30 14:45)

**Inherits**:
- `handoff-2026-09-30-installer-a-register-fix.md` (round 19c — failed)
- `handoff-2026-09-30-fix-ipc-http-done.md` (round 19 base)

## Status: ✅ **修复实现完成 + build 成功,等待用户 clean-machine 验证**

## 本轮新事实
根因 B(perMachine installer 在 admin context 下写 `$APPDATA` 指向 admin profile 而非原始 user)经修复 A 实施 + 重新 build 完成。

## 改动汇总

| 文件 | 类型 | 改动 |
|---|---|---|
| `release/extension-installer/resolve_user_appdata.ps1` | 新增 (4917 B) | WMI 3-strategy fallback,输出 USER_APPDATA=... |
| `release/extension-installer/register.ps1` | 修改 (11210 B) | 新增 `-UserAppData` param,4 处 `$env:APPDATA` → `$resolvedAppData` |
| `sigma-file-manager/src-tauri/installer/hooks.nsh` | 修改 | Stage 0 调用 resolve,`Var RESOLVEDAPPDATA`(NSIS 3.x `$USERAPPDATA` 是常量),POSTINSTALL + POSTUNINSTALL 都改 |
| `sigma-file-manager/src-tauri/tauri.conf.json` | 修改 | bundle 列表 + `tools/resolve_user_appdata.ps1` |
| `release/Sigma-File-Manager-2.2.0-focus-2-setup.exe` | 新 build | 16,944,574 bytes |

## Build artifact
- 路径: `F:\soft\00selfmade\filemanager\release\Sigma-File-Manager-2.2.0-focus-2-setup.exe`
- SHA1: `40C6703D9889A944288AFA0D3EE11EA52A7BB018`
- SHA256: `FB9D61BFC992C1ED7B3928FDB3A349D392B82D2C9B775EEF2169A7AE14AEE593`
- 时间: 2026-09-30 14:43

## NSIS 陷阱已踩
- `$USERAPPDATA` 是 NSIS 3.x 内置常量(类似 `$APPDATA`),StrCpy 报 "cannot change constants" — 改用 `Var RESOLVEDAPPDATA`
- StrCpy 跳过 prefix 的 idiom:`StrCpy $X $Y "" 13`(空字符串 maxlen = 无限长,13 = 起始偏移)

## 验证证据

### 1. 本机手动验证 resolve_user_appdata.ps1
```
[resolve] Strategy 1 failed: 无效的查询
[resolve] method=explorer-GetOwner sid=S-1-5-21-485535000-4071386851-89427845-1001
[resolve] USER_APPDATA=C:\Users\Duanyi\AppData\Roaming
[resolve] wrote to C:\Temp\user_appdata_test.txt
```
输出文件:`USER_APPDATA=C:\Users\Duanyi\AppData\Roaming` ✓

### 2. 7z 解压 installer 验证 bundle
- `tools/resolve_user_appdata.ps1` (4917 B) ✓
- `tools/register.ps1` (11210 B) ✓
- `extensions/kizemo.focus-sync/{package.json, dist/, locales/, bin/focus-sync-sidecar/focus-sync-sidecar.exe}` ✓
- `bin/focus-sync-sidecar/` legacy 路径 ✓

## 用户验证步骤(下一步必做)

1. **卸载** clean 机器上的 Sigma FM(包括 `com.sigma-file-manager.app` 目录)
2. **运行新 installer**:`release\Sigma-File-Manager-2.2.0-focus-2-setup.exe`
3. **观察 installer 日志**:Stage 0 应打印 "Resolved RESOLVEDAPPDATA=C:\Users\<User>\AppData\Roaming"
4. **启动 Sigma FM** → Extensions → Installed tab → **Focus Sync 应出现**
5. **DevTools Console** 应看到 `[focus-sync]` 日志(说明 entry 被读了)
6. **验证文件位置**:`%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\` 应有完整文件

## 失败时回滚
若新 installer 仍不显示 Focus Sync:
- 检查 installer 日志中 Stage 0 的 RESOLVEDAPPDATA 值 — 是否指向原始用户?
- 检查 Stage 6 的 register.ps1 是否 exit 0
- 手动跑 register.ps1 看是否成功
- 看是否落到 `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\` 完整文件
- 回退路径:用回 `Sigma-File-Manager-2.2.0-focus-1-setup.exe`(round 19c),回到失败 path 重做诊断

## 当前未 commit 改动
| 文件 | git status |
|---|---|
| `sigma-file-manager/src-tauri/Cargo.lock` | M (依赖未变) |
| `sigma-file-manager/src-tauri/Cargo.toml` | M |
| `sigma-file-manager/src-tauri/tauri.conf.json` | M (+resolve_user_appdata.ps1 bundle) |
| `sigma-file-manager/src-tauri/installer/hooks.nsh` | M (+Stage 0 + Var RESOLVEDAPPDATA) |
| `release/extension-installer/register.ps1` | M (+UserAppData param) |
| `release/extension-installer/resolve_user_appdata.ps1` | 新文件 |
| `release/Sigma-File-Manager-2.2.0-focus-2-setup.exe` | 新 build |

## 避坑提示
- **NSIS `$USERAPPDATA` 是常量**:别再用这个名字,改 `Var XXXAPPDATA`
- **POSTUNINSTALL 也要改**:uninstall 也跑 admin context,不 resolve 会删错地方
- **Strategy 1 (Win32_LoggedOnUser) 在某些 hardened 机器上 WMI 报错**:Strategy 2 (explorer.exe) 是更可靠的 fallback