; ============================================================================
; kizemo.focus-sync 0.5.9 — 扩展独立安装器（Standalone NSIS Installer）
; ============================================================================
; 把 Focus Sync 扩展注册进 Alpha File Manager：
;   1. 把扩展文件复制到 %APPDATA%\com.sigma-file-manager.app\；
;   2. 在 user-extensions.json 里登记扩展（Alpha File Manager 启动时读它
;      来填充「已安装」标签页）；
;   3. 把侧车二进制部署到 Alpha File Manager 主进程会去找的规范位置。
;
; 侧车生命周期（v0.5.9，2026-11-10 起）：
;   侧车现在是 **Alpha File Manager 主进程的子进程** —— 主进程启动时拉起它，
;   退出时杀掉它。因此本安装器**不再注册** legacy 的 'KizemoFocusSync'
;   计划任务（AtLogOn + --service），只会把旧版本残留的那个任务清掉。
;
; 安装前请注意：
;   - 先彻底退出 Alpha File Manager（任务栏图标右键 → 退出）。
;     运行中的实例内存里留着 user-extensions.json 的旧副本，下次保存时会
;     把磁盘上的注册信息覆盖掉，装完看起来「装了却没生效」，且没有任何报错。
;
; 安装完成后：
;   - 打开 Alpha File Manager
;   - Ctrl+Shift+X → 「已安装」标签页 → 应当能看到 Focus Sync
;   - 打开开关，工具栏应当出现「Sync Now / Enable-Disable」
; ============================================================================

Unicode true
SetCompressor /FINAL /SOLID lzma

!include "MUI2.nsh"
!include "LogicLib.nsh"

; ---------------------------------------------------------------------------
; Metadata
; ---------------------------------------------------------------------------
!define PRODUCT_NAME      "Focus Sync Extension"
!define PRODUCT_VERSION   "0.5.9"
!define PRODUCT_PUBLISHER  "kizemo"
!define PRODUCT_ID         "kizemo.focus-sync"
!define PRODUCT_EXTDIR     "kizemo.focus-sync"
!define PRODUCT_SIDECAR    "focus-sync-sidecar"
!define PRODUCT_SIDECAR_VER "0.5.9"
!define SIGMA_APP_DIR      "com.sigma-file-manager.app"
!define SIGMA_USERDATA     "$APPDATA\${SIGMA_APP_DIR}"

; 侧车的规范部署位置。这一条常量是「部署 sha 闸门」能通过的原因：
; 手动安装脚本（$binDir）、register.ps1 的规范路径探测，以及
; Alpha File Manager 主进程的侧车查找（src-tauri/src/focus_sync_sidecar.rs
; 的 resolve_sidecar_path）必须指向同一个位置。
; 注意主进程**不再**通过注册表找侧车，注册表键只是历史遗留。
!define SIDECAR_DEST_DIR  "${SIGMA_USERDATA}\extensions\${PRODUCT_EXTDIR}\bin\${PRODUCT_SIDECAR}"
!define SIDECAR_DEST_PATH "${SIDECAR_DEST_DIR}\${PRODUCT_SIDECAR}.exe"

; ---------------------------------------------------------------------------
; Custom vars (must be declared before first use)
; ---------------------------------------------------------------------------
Var SIGMAWASRUNNING

Name "${PRODUCT_NAME} ${PRODUCT_VERSION}"
OutFile "kizemo.focus-sync-${PRODUCT_VERSION}-setup.exe"

; Installer metadata lives in a stable local dir (not %APPDATA% to avoid
; colliding with Alpha File Manager's own state).
InstallDir "$LOCALAPPDATA\Programs\kizemo.focus-sync"
InstallDirRegKey HKCU "Software\${PRODUCT_PUBLISHER}\${PRODUCT_ID}" "InstallDir"

RequestExecutionLevel user
ShowInstDetails show
ShowUninstDetails show
BrandingText "${PRODUCT_NAME} ${PRODUCT_VERSION}"

!define MUI_ABORTWARNING
; MUI defaults for icon (omit MUI_ICON/MUI_UNICON to use built-ins).

; ---------------------------------------------------------------------------
; Pages
; ---------------------------------------------------------------------------
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_TITLE "Focus Sync 已安装"
!define MUI_FINISHPAGE_TEXT    "Focus Sync 已经注册到 Alpha File Manager。$\r$\n$\r$\n接下来：$\r$\n1. 打开 Alpha File Manager$\r$\n2. 按 Ctrl+Shift+X 打开扩展面板$\r$\n3. 「已安装」标签页应当列出 Focus Sync$\r$\n4. 打开开关，试用 Save-As 对话框的路径同步$\r$\n$\r$\n如果 Focus Sync 没出现，看安装目录里的 README.md。"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_LANGUAGE "SimpChinese"

; ---------------------------------------------------------------------------
; 关于 nsExec 调用形态（2026-10-08 的教训，2026-11-10 再确认一次）
;
; CheckSigmaFMRunning / KillSigmaFM / ReopenSigmaFM 曾是 NSIS Function，
; 用 "Call" 调用 —— 实测从来没执行过：安装 10 秒就结束了，而 kill 脚本本身
; 至少要 12 秒。现在这些步骤全部内联到调用点（Stage 1 与 Stage 8）。
;
; 另一条更硬的规矩（2026-11-10 真机实测）：安装器上下文里
; `powershell -NoProfile -Command "..."` 这种内联形态**不可靠** ——
; 同一行命令在普通提权 shell 里能跑通，在安装器里却静默不生效。
; 安装器里只用两种形态：`powershell -File "<绝对路径>"` 或原生 exe。
; ---------------------------------------------------------------------------

; ---------------------------------------------------------------------------
; Installation
; ---------------------------------------------------------------------------
Section "Install Focus Sync Extension" SecInstall
    SectionIn RO

    DetailPrint "==> [Focus Sync 安装器] v${PRODUCT_VERSION} BEGIN"

    ; ---- Stage 0: 把 PowerShell helper 脚本放到 $INSTDIR\bin ----
    ; 放 $INSTDIR\bin 而不是 $PLUGINSDIR：这里的 $PLUGINSDIR 会展开成空串，
    ; 曾经导致 Sigma FM 的关闭脚本静默失效（见 Stage 1）。
    DetailPrint "==> Stage 0: 准备 helper 脚本到 $INSTDIR\bin..."
    SetOutPath "$INSTDIR\bin"
    File "kill-sigma.ps1"
    File "reopen-sigma.ps1"
    ; v0.5.9：清 legacy 计划任务 + 结束仍在运行的侧车进程。
    ; 两者都必须用 `powershell -File` 形态调用（见上面的说明）。
    File "remove-legacy-focus-task.ps1"
    File "stop-sidecar-processes.ps1"

    ; ---- Stage 1: 如果 Alpha File Manager 在跑，先关掉它 ----
    ; 为什么必须关：它内存里留着 user-extensions.json 的旧副本，下次保存时
    ; 会把磁盘上的注册信息覆盖掉。所以先杀进程，再写注册信息。
    ; 进程名两个都查：应用已改名为 alpha-file-manager，旧版本是 sigma-file-manager。
    StrCpy $SIGMAWASRUNNING "0"
    DetailPrint "==> Stage 1: 检查 Alpha File Manager 是否在运行..."
    nsExec::ExecToLog 'powershell -NoProfile -Command "$$p = Get-Process -Name alpha-file-manager,sigma-file-manager -ErrorAction SilentlyContinue; if ($$p) { exit 0 } else { exit 1 }"'
    Pop $0
    ${If} $0 == "0"
        DetailPrint "==> 检测到 Alpha File Manager 正在运行。"
        MessageBox MB_YESNO|MB_ICONQUESTION "检测到 Alpha File Manager 正在运行。$\r$\n$\r$\n安装需要先关闭它 —— 否则它会用内存里的旧状态覆盖插件注册信息,$\r$\n导致插件装上了却无法加载(无任何报错)。$\r$\n$\r$\n是否现在关闭 Alpha File Manager 并继续安装?$\r$\n$\r$\n选择「否」将取消本次安装。" /SD IDYES IDYES 0 IDNO 0
        Pop $0
        ; 只有明确的「否」（IDNO = 7）才算拒绝。/S 静默模式下对话框被抑制，
        ; NSIS 返回 0，不能把它当成拒绝 —— 否则无人值守安装永远装不成。
        ; 安全性没有削弱：下面的后置断言仍然会在应用还活着时中止。
        DetailPrint "    (对话框返回: $0)"
        ${If} $0 == 7
            DetailPrint "==> 用户选择不关闭应用,安装已取消。"
            Abort
        ${EndIf}

        DetailPrint "==> 正在关闭 Alpha File Manager(先尝试正常退出,最多等 10 秒)..."
        nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\bin\kill-sigma.ps1"'
        Pop $0
        StrCpy $SIGMAWASRUNNING "1"
    ${Else}
        DetailPrint "==> Alpha File Manager 没有在运行(很好)。"
    ${EndIf}

    ; ---- Stage 1 后置断言：不要相信「杀掉了」，要验证 ----
    ; 应用会用内存里的旧状态重写 user-extensions.json，在它还活着的时候
    ; 继续安装，就会精确复现「装上了却没有加载」这个老故障。
    ; 断言这个不变量，而不是假设它成立。
    nsExec::ExecToLog 'powershell -NoProfile -Command "$$p = Get-Process -Name alpha-file-manager,sigma-file-manager -ErrorAction SilentlyContinue; if ($$p) { exit 0 } else { exit 1 }"'
    Pop $0
    ${If} $0 == "0"
        DetailPrint "==> ERROR: 关闭尝试之后 Alpha File Manager 仍然在运行。"
        DetailPrint "    继续下去会让它覆盖 user-extensions.json。"
        DetailPrint "    请关闭 Alpha File Manager 后重新运行本安装器。"
        ; 静默模式（/S）没有控制台，证据必须落到磁盘上。
        FileOpen $1 "$TEMP\focus-sync-install-FAILED.txt" a
        FileWrite $1 "INSTALL ABORTED: Alpha File Manager was still running after the Stage 1 kill attempt. It would have overwritten user-extensions.json. Close it and re-run.$\r$\n"
        FileClose $1
        DetailPrint "    已中止。标记文件写在 %TEMP%\focus-sync-install-FAILED.txt"
        Abort
    ${EndIf}
    DetailPrint "==> 已确认: Alpha File Manager 没有在运行。"

    ; ---- Stage 1.5 (v0.5.9): 注销 legacy 'KizemoFocusSync' 计划任务 ----
    ; 侧车以前靠这个计划任务（AtLogOn + --service）保活，生命周期与主程序
    ; 完全脱钩。现在侧车归主进程所有，这个任务必须消失，否则它会按
    ; -RestartCount 3 / -RestartInterval 1min 不断把 --service 侧车复活，
    ; 和主进程拉起的侧车抢 37421 端口。
    ; 幂等：任务本来就不存在也算成功。
    DetailPrint "==> Stage 1.5: 注销 legacy 计划任务 'KizemoFocusSync'（如果存在）..."
    nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\bin\remove-legacy-focus-task.ps1"'
    Pop $0
    ${If} $0 != "0"
        DetailPrint "==> WARNING: remove-legacy-focus-task.ps1 退出码 $0"
    ${EndIf}

    ; ---- Stage 1.6 (v0.5.9): 结束仍在运行的侧车进程 ----
    ; 运行中的映像会锁住自己的 exe，不结束它，Stage 2.5 覆盖二进制会失败。
    DetailPrint "==> Stage 1.6: 结束仍在运行的 focus-sync 侧车进程..."
    nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\bin\stop-sidecar-processes.ps1"'
    Pop $0
    ${If} $0 != "0"
        DetailPrint "==> WARNING: stop-sidecar-processes.ps1 退出码 $0"
    ${EndIf}

    ; ---- Stage 2: 创建安装器元数据目录 ----
    DetailPrint "==> Stage 2: 创建安装器元数据目录 $INSTDIR..."
    SetOutPath "$INSTDIR"
    File "register.ps1"
    File "unregister.ps1"
    File "README.md"

    ; ---- Stage 2.5 (v0.5.9): 部署侧车二进制 ----
    ; 只剩规范位置一处：主进程的 resolve_sidecar_path 找的就是这里
    ; （<appdata>\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe）。
    SetOutPath "${SIDECAR_DEST_DIR}"
    File "..\bin\focus-sync-sidecar.exe"

    ; ---- Stage 3: 复制扩展文件到 %APPDATA% ----
    DetailPrint "==> Stage 3: 复制扩展文件到 ${SIGMA_USERDATA}\extensions\${PRODUCT_EXTDIR}"
    SetOutPath "${SIGMA_USERDATA}\extensions\${PRODUCT_EXTDIR}"
    File "..\package.json"

    SetOutPath "${SIGMA_USERDATA}\extensions\${PRODUCT_EXTDIR}\dist"
    File "..\dist\index.js"
    File "..\dist\index.js.map"

    SetOutPath "${SIGMA_USERDATA}\extensions\${PRODUCT_EXTDIR}\locales"
    File "..\locales\en.json"
    File "..\locales\zh-CN.json"

    ; ---- Stage 4 (v0.5.9): 不再由本安装器拉起侧车 ----
    ; 侧车二进制落在 ${SIDECAR_DEST_PATH}，由 **Alpha File Manager 主进程**
    ; 在启动时拉起、退出时杀掉（src-tauri/src/focus_sync_sidecar.rs）。
    ; 主进程会自己找这个路径，不需要注册表，也不需要计划任务。

    ; ---- Stage 5: 用 PowerShell 在 user-extensions.json 里登记扩展 ----
    DetailPrint "==> Stage 5: 在 user-extensions.json 里登记扩展..."
    nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\register.ps1"'

    ; ---- Stage 6: 写注册表键（历史遗留，运行时不再读取）----
    DetailPrint "==> Stage 6: 写注册表键 SidecarPath (HKCU\SOFTWARE\kizemo\focus-sync\)..."
    WriteRegStr HKCU "SOFTWARE\kizemo\focus-sync" "SidecarPath" "${SIDECAR_DEST_PATH}"

    ; ---- Stage 6.5 (v0.5.9): 复核 legacy 计划任务确实没了 ----
    ; 与主安装器的 STAGE 9 一样，这是一道幂等安全网。
    DetailPrint "==> Stage 6.5: 复核 legacy 计划任务 'KizemoFocusSync' 已消失..."
    nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\bin\remove-legacy-focus-task.ps1"'
    Pop $0
    ${If} $0 != "0"
        DetailPrint "==> WARNING: remove-legacy-focus-task.ps1 退出码 $0"
    ${EndIf}

    ; ---- Stage 7: 写卸载器 ----
    DetailPrint "==> Stage 7: 写卸载器..."
    WriteUninstaller "$INSTDIR\uninst.exe"

    ; ---- Stage 8: 如果 Stage 1 杀过应用，重新打开它 ----
    ; 同样的理由：Function 形式从来没跑过，必须内联。
    ${If} $SIGMAWASRUNNING == "1"
        DetailPrint "==> Stage 8: 重新打开 Alpha File Manager..."
        nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\bin\reopen-sigma.ps1"'
        Pop $0
        ${If} $0 == "0"
            DetailPrint "==> Alpha File Manager 已重新打开。"
        ${Else}
            DetailPrint "==> WARNING: 无法自动重新打开 Alpha File Manager。"
            DetailPrint "    请手动从安装目录启动。"
        ${EndIf}
    ${EndIf}

    DetailPrint "==> [Focus Sync 安装器] END"
    DetailPrint "    扩展文件:       ${SIGMA_USERDATA}\extensions\${PRODUCT_EXTDIR}"
    DetailPrint "    侧车二进制:     ${SIDECAR_DEST_PATH}"
    DetailPrint "    侧车生命周期:   由 Alpha File Manager 主进程拉起并随其退出而结束"
    DetailPrint "    PS 脚本:        $INSTDIR\bin\remove-legacy-focus-task.ps1"
    DetailPrint "                    $INSTDIR\bin\stop-sidecar-processes.ps1"
    DetailPrint "    注册表:         HKCU\SOFTWARE\kizemo\focus-sync\SidecarPath（历史遗留）"
    DetailPrint "    user-extensions.json 已更新（isLocal: true）。"
    DetailPrint "    安装开始时应用是否在运行: $SIGMAWASRUNNING (1 = 已关闭并重新打开, 0 = 未运行)"
SectionEnd

; ---------------------------------------------------------------------------
; Uninstaller
; ---------------------------------------------------------------------------
Section "Uninstall"
    DetailPrint "==> [Focus Sync 卸载器] BEGIN"

    ; 删除扩展文件
    DetailPrint "==> 删除扩展文件..."
    RMDir /r "${SIGMA_USERDATA}\extensions\${PRODUCT_EXTDIR}"
    ; 历史遗留清理：早期版本把侧车放在 binaries\ 下面，一并删掉。
    ; （注释行末尾不能带反斜杠：NSIS 会把它读成续行，把下面的 RMDir 吃掉。）
    RMDir /r "${SIGMA_USERDATA}\binaries\${PRODUCT_SIDECAR}"

    ; 从 user-extensions.json 注销
    DetailPrint "==> 从 user-extensions.json 注销..."
    nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\unregister.ps1"'
    Pop $0
    ${If} $0 != "0"
        DetailPrint "==> WARNING: unregister.ps1 退出码 $0"
    ${EndIf}

    ; 注销 legacy 计划任务 + 结束侧车进程
    DetailPrint "==> 注销 legacy 计划任务并结束侧车进程..."
    nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\bin\remove-legacy-focus-task.ps1"'
    Pop $0
    nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\bin\stop-sidecar-processes.ps1"'
    Pop $0

    ; 删除 SidecarPath 注册表键
    DetailPrint "==> 删除 SidecarPath 注册表键..."
    DeleteRegKey HKCU "SOFTWARE\kizemo\focus-sync"

    ; 删除安装器元数据目录
    DetailPrint "==> 删除安装器元数据..."
    RMDir /r "$INSTDIR"

    DetailPrint "==> [Focus Sync 卸载器] END"
SectionEnd

; ---------------------------------------------------------------------------
; Uninstaller init
; ---------------------------------------------------------------------------
Function un.onInit
    ; 检测 Alpha File Manager 是否在运行
    nsExec::ExecToLog 'powershell -NoProfile -Command "$$p = Get-Process -Name alpha-file-manager,sigma-file-manager -ErrorAction SilentlyContinue; if ($$p) { exit 0 } else { exit 1 }"'
    Pop $0
    ${If} $0 == "0"
        ; 应用在跑 —— 确认用户是否继续
        MessageBox MB_ICONQUESTION|MB_YESNO|MB_DEFBUTTON2 "Alpha File Manager 正在运行。$\r$\n$\r$\n它持有 user-extensions.json 的内存状态，卸载无法彻底清理注册信息。$\r$\n$\r$\n仍然继续吗?$\r$\n(想彻底卸载请先关闭 Alpha File Manager)"
        ; MessageBox 把 $0 置为 6 (YES) 或 7 (NO)
        ${If} $0 == "7"
            Abort
        ${EndIf}
    ${EndIf}
FunctionEnd
