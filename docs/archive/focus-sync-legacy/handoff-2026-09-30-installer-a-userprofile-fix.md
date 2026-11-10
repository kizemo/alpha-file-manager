# Handoff — Round 19d FIX: NSIS $APPDATA/$USERPROFILE bugs (2026-09-30 16:35)

**Inherits**:
- `handoff-2026-09-30-installer-a-wmi-fix.md` (round 19d attempt 1 — half-broken)
- `handoff-2026-09-30-installer-a-register-fix.md` (round 19c)

## Status: ✅ **REAL FIX BUILT + VERIFIED ON CLEAN INSTALL**

focus-6 silent install test (admin/elevated) on this machine:
- `RESOLVED=C:\Users\Duanyi\AppData\Roaming` (correct user path, not ProgramData!)
- `extensions\kizemo.focus-sync\`: ALL files copied ✓
- `binaries\focus-sync-sidecar\`: binary copied ✓
- `user-extensions.json`: kizemo.focus-sync entry with isLocal=true ✓
- After Sigma FM launch: entry preserved ✓

## 本轮新事实(关键 bug)

Round 19d 第一次 build (focus-5) **失败** — marker 文件 `RESOLVED=$USERPROFILE\AppData\Roaming$EOL`,NSIS 3.x 暴露两个独立 bug:

| NSIS 变量 | 实际行为 | 期望 |
|---|---|---|
| `$APPDATA` | 返回 `C:\ProgramData` (那是 `$PROGRAMDATA` 的值) | 用户 AppData\Roaming |
| `$USERPROFILE` | StrCpy 不展开 — 字面量保留 | 用户 home dir |

所以两轮 build 都失败:第一次 fallback 用 `$APPDATA` → 写到 `C:\ProgramData`;第二次 fallback 用 `$USERPROFILE` → 字面量不展开,写到 `$USERPROFILE\AppData\Roaming`(无效路径)。

## 真修复(round 19d attempt 2)

**完全不用 NSIS 的变量做 fallback。** 用 PowerShell one-liner 通过 `$env:USERPROFILE` 算路径:

```nsi
${If} $0 != "0"
  nsExec::ExecToLog 'powershell -NoProfile -Command "Write-Output USER_APPDATA=$env:USERPROFILE\AppData\Roaming | Out-File -FilePath $PLUGINSDIR\user_appdata.txt -Encoding ASCII -Force"'
  Pop $0
  FileOpen $R0 "$PLUGINSDIR\user_appdata.txt" r
  ...
  FileRead $R0 $R1
  FileClose $R0
  StrCpy $RESOLVEDAPPDATA $R1 "" 13
${EndIf}
```

PowerShell 在 elevated context 下,`$env:USERPROFILE` 解析到 admin 用户(在 admin 提权场景下就是当前登录用户,因为 Duanyi 在 Administrators 组)。

最终 fallback 用 `$PROFILE`(NSIS 老的 synonym,确认能展开)。

## Build artifact

- 路径: `F:\soft\00selfmade\filemanager\release\Sigma-File-Manager-2.2.0-focus-6-setup.exe`
- 16,946,584 bytes (16.16 MiB)
- SHA1: `6F7938EB6159513097E6FBB20985B664FC9CF386`
- Built: 2026-09-30 16:30:11

## 改动汇总(本轮新增)

| 文件 | 改动 |
|---|---|
| `sigma-file-manager/src-tauri/installer/hooks.nsh` | Stage 0 fallback 改用 PowerShell `$env:USERPROFILE` one-liner;最终 fallback 用 `$PROFILE`(确认能展开);POSTUNINSTALL 同步修复 |

未 commit:
- hooks.nsh 修改
- 之前的 release/extension-installer/resolve_user_appdata.ps1 + register.ps1(从 round 19d 首次尝试)
- tauri.conf.json 的 resolve_user_appdata.ps1 bundle

## 用户验证步骤

1. 用 `release\Sigma-File-Manager-2.2.0-focus-6-setup.exe` 重装
2. 检查 installer 日志(看 Stage 0 应打印 "Falling back to PowerShell $env:USERPROFILE" + "Resolved RESOLVEDAPPDATA=C:\Users\<user>\AppData\Roaming")
3. 启动 Sigma FM → Extensions → Installed tab → Focus Sync 应出现
4. 验证文件位置:`%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\` 应有完整文件

## 失败回滚

若 focus-6 仍失败:看 NSIS 安装日志(需用户在 installer 最后页面点 "Show details" 然后右键 save log)中的 "==> Stage 0" 输出 + "==> Resolved" 行。

如果 resolved 路径错误:
- WMI 也 fail:检查 PowerShell 在 elevated 下的策略(可能 Restricted)
- WMI 返回错误 SID:可能有 domain controller 问题
- 检查 `tools\resolve_user_appdata.ps1` 是否真的 bundle 进 installer

## 避坑提示(供未来 session)

1. **NSIS 3.x `$APPDATA` 在 elevated context 可能返回 `C:\ProgramData`** — 不可信
2. **NSIS `$USERPROFILE` 在 StrCpy 里不展开** — 必须用 `$PROFILE`
3. **`$PROFILE` 是 NSIS 老常量,确认能展开** — 用作最终 fallback
4. **PowerShell `$env:USERPROFILE` 在 admin context 仍是当前用户** — 这才是 fallback 路径的正确来源
5. **优先用 PowerShell 算路径,然后读文件 + StrCpy 解析 prefix** — 不要依赖 NSIS 变量

## 当前未 commit 改动(等用户验证后再决定)

| File | git status |
|---|---|
| `sigma-file-manager/src-tauri/tauri.conf.json` | M (+resolve_user_appdata.ps1 bundle) |
| `sigma-file-manager/src-tauri/installer/hooks.nsh` | M (Stage 0 + fallback fixes, NSIS $APPDATA/$USERPROFILE workarounds) |
| `release/extension-installer/register.ps1` | M (+UserAppData param) |
| `release/extension-installer/resolve_user_appdata.ps1` | 新文件 (3-strategy fallback) |
| `release/Sigma-File-Manager-2.2.0-focus-2-setup.exe` | round 19d 第一次 — **BROKEN**,勿用 |
| `release/Sigma-File-Manager-2.2.0-focus-3-setup.exe` | 调试 marker 版,丢弃 |
| `release/Sigma-File-Manager-2.2.0-focus-4-setup.exe` | 调试 USERPROFILE 版 — **BROKEN**,勿用 |
| `release/Sigma-File-Manager-2.2.0-focus-5-setup.exe` | 同上 — **BROKEN**,勿用 |
| `release/Sigma-File-Manager-2.2.0-focus-6-setup.exe` | **CURRENT WORKING BUILD** (16.94 MiB, SHA1=6F7938EB6159513097E6FBB20985B664FC9CF386) |