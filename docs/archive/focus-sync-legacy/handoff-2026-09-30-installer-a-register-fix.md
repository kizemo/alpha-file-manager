# Handoff — Installer A 仍然无法在 clean 机器显示 Focus Sync (2026-09-30 13:45)

**Inherits**:
- `handoff-2026-09-30-fix-ipc-http-done.md` (Round 19c 基础)
- `handoff-2026-09-30-package-both-installers.md` (Installer A/B 初始打包)
- `handoff-2026-09-30-installer-a-register-fix.md` (上一轮,POSTINSTALL 加了 register.ps1 调用)

**Status**: ⚠️ **Installer A 在用户 clean 机器上仍然没显示 Focus Sync**。POSTINSTALL 步骤 6 调 register.ps1 修了 user-extensions.json 写入,但根因(可能)更深,新会话必须继续 debug。

---

## 一、本轮新事实

用户反馈:干净机器(卸载 Sigma FM 后)重装 `release/Sigma-File-Manager-2.2.0-focus-1-setup.exe`(13:38 新版,16.16 MiB),Sigma FM 启动后**Installed tab 仍然空**。DevTools 控制台没看到任何 `[focus-sync]` 日志(说明 entry 都没被读)。

## 二、Postmortem:为什么上轮修复失败

| 假设 | 状态 | 证据 |
|---|---|---|
| A. POSTINSTALL 没运行 | ❓ 未验证 | NSIS hook 是否报错未知 |
| B. POSTINSTALL 写到 admin %APPDATA% 而非 user 的 | 🔥 **最可能** | perMachine installer 跑在 admin context,`$APPDATA` 解析到 admin profile。若 admin ≠ user(用户从非 admin 提权),文件就装错地方。`register.ps1` 通过 PowerShell 写 `${env:APPDATA}` 也是 admin 的 |
| C. register.ps1 有 bug 静默退出 | ❓ 未验证 | 上次跑成功(自己手动时),但 installer 内跑时路径可能不同 |
| D. Sigma FM 已跑着,POSTINSTALL 写完但 Sigma FM save 覆盖 | ❌ | 用户先重装后启 Sigma FM,顺序正确 |
| E. 入口的 extension files 没 copy 成功 | ❓ 未验证 | 同样受 B 影响 |
| F. `__binaries` populate 失败 | ❌ | 同 B,register.ps1 写到 admin path |

**最可能根因 = B**:perMachine install 的 `$APPDATA` 在 admin context 下不指向原始 user。

## 三、新会话必须先验证的事

### 3.1 关键检查(必做)

```powershell
# 1. 确认 Sigma FM 用的是哪个 user
$who = Get-Process -Name sigma-file-manager -ErrorAction SilentlyContinue | Select-Object -First 1
Write-Host "Sigma FM running as: $($who.StartInfo.UserName)"  # 期望: 当前用户

# 2. 检查 installer 实际把文件写到了哪
$paths = @(
  "$env:APPDATA\com.sigma-file-manager.app\extensions",
  "$env:LOCALAPPDATA\com.sigma-file-manager.app\extensions"
)
foreach ($p in $paths) {
  Write-Host "$p"
  Test-Path $p  # True = installer 写到这里
}

# 3. 直接看 user-extensions.json 写了没
$userExtJson = "$env:APPDATA\com.sigma-file-manager.app\user-data\user-extensions.json"
Get-Content $userExtJson -Raw | ConvertFrom-Json | Select-Object -ExpandProperty installedExtensions | Format-List

# 4. 看 binaries 路径 vs sidecar 实际位置
$canonical = "$env:APPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe"
$shared = "$env:APPDATA\com.sigma-file-manager.app\binaries\focus-sync-sidecar\0.2.0\focus-sync-sidecar.exe"
Test-Path $canonical
Test-Path $shared
```

### 3.2 修复方向(候选)

| 方向 | 改动 | 风险 |
|---|---|---|
| **A. 让 POSTINSTALL 用原始 user 路径** | NSIS hook 用 WTS API 查 active console user → 写那个用户的 %APPDATA% | 中(复杂但对 perMachine 唯一可行) |
| **B. 改 installMode 为 perUser** | tauri.conf.json 改 `installMode: "currentUser"` + `RequestExecutionLevel user` | 高(失去写 %PROGRAMFILES% 能力,Sigma FM 二进制装位置变) |
| **C. 让 Sigma FM 自启动检测** | 在 Sigma FM 启动时,若 extensions/ 有 kizemo 但 storage 没 entry,自动 addInstalledExtension | 改 fork 代码,~10 分钟重 build |
| **D. 让用户用 Sigma FM UI 装** | 装完开 Sigma FM → 装本地扩展 → 文件夹选 %APPDATA%\...\extensions\kizemo.focus-sync | 简单但违反用户"全自动 EXE"诉求 |

**最推荐 = A**(向后兼容 perMachine 不变)。具体实现见 §四。

## 四、候选修复 A 的具体实现

需要修改 `sigma-file-manager/src-tauri/installer/hooks.nsh` 的 POSTINSTALL。NSIS 在 admin 上下文跑,但可以用 `${AutoLoginUserName}` 拿原始登录 user,然后通过 WTS API 拿 token 拿真实 %APPDATA%。

简化版本:用 PowerShell 调用 WMI 在 POSTINSTALL 阶段查 active console session user 的 profile path。

```nsi
; hooks.nsh 追加在 Stage 6 之前
DetailPrint "==> Resolving original user's %APPDATA%..."
nsExec::ExecToLog 'powershell -NoProfile -Command "$explorer = Get-CimInstance Win32_Process -Filter \"Name=''explorer.exe''\" | Select-Object -First 1; $owner = (Get-CimInstance -ClassName Win32_LoggedOnUser -Filter \"LogonType=2\" | Select-Object -First 1).Antecedent; $sid = ($owner -replace ''.*Sid='''','''''') -replace ''\).*''',''''''; $profile = (Get-ItemProperty \"HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\ProfileList\\$sid\").ProfileImagePath; Write-Host \"USER_APPDATA=$profile\\AppData\\Roaming\"" > $TEMP\user-appdata.txt'
Pop $0
; 读 $TEMP\user-appdata.txt,提取 USER_APPDATA
; 然后用该路径 CopyFiles + 调 register.ps1 (也用 USER_APPDATA)
```

**风险**:WMI 在某些 hardened 系统上可能没权限。备选方案:用 NSIS 的 `${UserInfo}` 函数(需 `UserInfo` 插件)。

## 五、Source 现状(已完成)

| 路径 | 状态 |
|---|---|
| `release/Sigma-File-Manager-2.2.0-focus-1-setup.exe` | 16.16 MiB,**13:38 构建** |
| `release/kizemo.focus-sync-0.2.0-setup.exe` | 840,810 B,**11:44 构建**(Installer B) |
| `sigma-file-manager/src-tauri/tauri.conf.json` | + `register.ps1` bundle 到 `tools/` |
| `sigma-file-manager/src-tauri/installer/hooks.nsh` | + Stage 6 调 register.ps1 |
| `release/extension-installer/register.ps1` | 含 `__binaries` populate + `isLocal: true` |

## 六、完整 Source 文档路径(新 session 必读)

按相关性排序(都在 fork repo 或 meta-repo):
1. **`E:\Users\Duanyi\.claude\projects\F--soft-00selfmade-filemanager\memory\sigma_fm_extension_registration.md`** — 注册机制总览(5 个必记约束:storage register、binary path、__binaries、isLocal、permissions)
2. **`handoff-2026-09-29-focus-sync-registration.md`** (round 5) — Installed tab 读 user-extensions.json 不是 disk scan
3. **`handoff-2026-09-29-sidecar-path-fix.md`** (round 9) — sidecar 必须 `extensions/<id>/bin/<binary_id>/<executable>`
4. **`handoff-2026-09-29-focus-sync-json-patch.md`** (round 6) — entry 的最小字段集参考
5. **`sigma-file-manager/src/modules/extensions/api/create-binary-api.ts`** — `binary.getPath(id)` 实际从 `__binaries` 读,**不调 Rust `get_extension_binary_path`**
6. **`sigma-file-manager/src/modules/extensions/runtime/manifest-binaries.ts`** — populate `__binaries` 的唯一入口 `syncManifestBinariesForExtension`
7. **`sigma-file-manager/src/stores/runtime/extensions.ts::init`** — Sigma FM 启动时 extension 加载流程
8. **`sigma-file-manager/src/stores/runtime/extensions.ts::removeUnapprovedInstalledExtensions`** — `isLocal: true` 保护条目的关键

## 七、**新会话进入指引**(必读顺序)

1. 读本文档(已读)
2. 读 §六列出的 8 个 source(优先级 1-4 必读,5-8 进阶 debug 用)
3. **第一步**:让用户跑 §3.1 的检查命令,确认 B 假设(admin 路径不匹配)。**先确认根因,再修复**(Phase 1 systematic-debugging)
4. 如果 B 确认:实现 §四 候选修复 A,在 hooks.nsh 用 WMI 找原始 user 路径
5. 重 build Tauri(~7 min),让用户重装 clean 机器
6. 验证 Installed tab 出 Focus Sync

## 八、Item 3(向原开发者提交扩展收录)— 暂缓

本任务卡在 EXE 路径,需先解决注册流程。提交 PR 的工作(adapt extension marketplace schema、PR description)可以等 EXE 路径稳定后做。

## 九、本会话已改文件

| 文件 | 状态 |
|---|---|
| `sigma-file-manager/src-tauri/tauri.conf.json` | + register.ps1 bundle |
| `sigma-file-manager/src-tauri/installer/hooks.nsh` | + Stage 6 调 register.ps1 |
| `release/extension-installer/register.ps1` | 含 __binaries populate + isLocal=true |
| `release/extension/bin/focus-sync-sidecar-canonical.exe` | 临时 source(已用,可删) |
| `release/Sigma-File-Manager-2.2.0-focus-1-setup.exe` | **13:38 构建,16.16 MiB,但有 bug** |
| `release/kizemo.focus-sync-0.2.0-setup.exe` | 11:44 构建,840 KB |
| `release/extension-installer/kizemo.focus-sync-0.2.0-setup.exe` | 同步副本 |
