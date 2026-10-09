; ============================================================================
; kizemo.focus-sync 0.5.8 — Standalone NSIS Installer
; ============================================================================
; This installer is a one-click solution to register the Focus Sync extension
; with Sigma File Manager. It:
;   1. Copies extension files to %APPDATA%\com.sigma-file-manager.app\
;   2. Registers the extension in user-extensions.json (the storage Sigma FM
;      reads from at startup to populate its "已安装" tab).
;
; IMPORTANT — Before running this installer:
;   - CLOSE Sigma File Manager completely (right-click taskbar icon → Quit).
;   - Sigma FM uses a one-shot startup bootstrap. If it's running, its
;     in-memory state has stale data and will overwrite user-extensions.json
;     on its next save, undoing this install.
;
; After install completes:
;   - Open Sigma File Manager
;   - Ctrl+Shift+X → Installed tab → Focus Sync should be listed
;   - Toggle it on; toolbar should show Sync Now / Enable-Disable items
; ============================================================================

Unicode true
SetCompressor /FINAL /SOLID lzma

!include "MUI2.nsh"
!include "LogicLib.nsh"

; ---------------------------------------------------------------------------
; Metadata
; ---------------------------------------------------------------------------
!define PRODUCT_NAME      "Focus Sync Extension"
!define PRODUCT_VERSION   "0.5.8"
!define PRODUCT_PUBLISHER  "kizemo"
!define PRODUCT_ID         "kizemo.focus-sync"
!define PRODUCT_EXTDIR     "kizemo.focus-sync"
!define PRODUCT_SIDECAR    "focus-sync-sidecar"
!define PRODUCT_SIDECAR_VER "0.5.8"
!define SIGMA_APP_DIR      "com.sigma-file-manager.app"
!define SIGMA_USERDATA     "$APPDATA\${SIGMA_APP_DIR}"

; Canonical sidecar location. This single constant is the reason the deploy
; sha gate can pass: manual-install.ps1 ($binDir), register.ps1's canonical
; probe and the KizemoFocusSync task must all agree with it.
!define SIDECAR_DEST_DIR  "${SIGMA_USERDATA}\extensions\${PRODUCT_EXTDIR}\bin\${PRODUCT_SIDECAR}"
!define SIDECAR_DEST_PATH "${SIDECAR_DEST_DIR}\${PRODUCT_SIDECAR}.exe"

; ---------------------------------------------------------------------------
; Custom vars (must be declared before first use)
; ---------------------------------------------------------------------------
Var SIGMAWASRUNNING

Name "${PRODUCT_NAME} ${PRODUCT_VERSION}"
OutFile "kizemo.focus-sync-${PRODUCT_VERSION}-setup.exe"

; Installer metadata lives in a stable local dir (not %APPDATA% to avoid
; colliding with Sigma FM's own state).
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
!define MUI_FINISHPAGE_TITLE "Focus Sync installed"
!define MUI_FINISHPAGE_TEXT    "Focus Sync has been registered with Sigma File Manager.$\r$\n$\r$\nNext steps:$\r$\n1. Open Sigma File Manager$\r$\n2. Press Ctrl+Shift+X to open Extensions$\r$\n3. The Installed tab should now list Focus Sync$\r$\n4. Toggle it on and try Save-As dialog focus sync$\r$\n$\r$\nIf Focus Sync does not appear, see README.md in the installer directory."
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_LANGUAGE "SimpChinese"

; ---------------------------------------------------------------------------
; NOTE (2026-10-08): CheckSigmaFMRunning / KillSigmaFM / ReopenSigmaFM were
; NSIS Functions wrapping nsExec, invoked with "Call". Observed: they never
; executed -- an install finished in 10s while kill-sigma.ps1 needs >=12s.
; All three are now inlined at their call sites (Stages 1 and 8), which is the
; form that Stage 5 has always used successfully.
; ---------------------------------------------------------------------------

; ---------------------------------------------------------------------------
; Installation
; ---------------------------------------------------------------------------
Section "Install Focus Sync Extension" SecInstall
    SectionIn RO

    DetailPrint "==> [Focus Sync Installer] v0.3.0 (Scheduled Task mode) BEGIN"

    ; ---- Stage 0: Stage the PowerShell helpers ----
    ; They go to $INSTDIR\bin, NOT $PLUGINSDIR: $PLUGINSDIR expands to an empty
    ; string here, which silently broke the Sigma FM kill (see Stage 1).
    DetailPrint "==> Stage 0: Preparing helper scripts in $INSTDIR\bin..."
    SetOutPath "$INSTDIR\bin"
    File "kill-sigma.ps1"
    File "reopen-sigma.ps1"

    ; ---- Stage 1: If Sigma FM is running, kill it gracefully + reopen later ----
    ; Round-2 of class fix (2026-09-30 09:55 follow-up): replace HARD-BLOCK
    ; dialog with auto-kill + auto-reopen. Reason: Sigma FM holds an
    ; in-memory copy of user-extensions.json and silently overwrites the
    ; on-disk file on next save. We must kill it before install so
    ; register.ps1's write survives. User explicitly preferred auto-kill
    ; over the manual-close popup.
    StrCpy $SIGMAWASRUNNING "0"
    DetailPrint "==> Stage 1: Checking if Sigma FM is running..."
    ; ROOT CAUSE of the "auto-close never worked" bug (2026-10-08):
    ; the helper scripts were staged into $PLUGINSDIR and invoked as
    ;   powershell ... -File "$PLUGINSDIR\kill-sigma.ps1"
    ; but $PLUGINSDIR expands to an EMPTY string at that point, so the command
    ; actually ran as  -File "\kill-sigma.ps1"  and failed. Proven with an
    ; isolated NSIS harness: $PLUGINSDIR printed as "[]" while two control
    ; probes proved nsExec itself returns exit codes correctly (7 and 3).
    ; The scripts now live in $INSTDIR\bin, which is known-good -- that is the
    ; same mechanism Stage 5's register.ps1 call has always used.
    ;
    ; The detection + kill also used to live in NSIS Functions invoked with
    ; "Call", which never executed either. Everything is inline now.
    nsExec::ExecToLog 'powershell -NoProfile -Command "$$p = Get-Process -Name sigma-file-manager -ErrorAction SilentlyContinue; if ($$p) { exit 0 } else { exit 1 }"'
    Pop $0
    ${If} $0 == "0"
        ; ---- Ask the user before closing their app ----
        DetailPrint "==> Sigma FM is running."
        MessageBox MB_YESNO|MB_ICONQUESTION "检测到 Sigma File Manager 正在运行。$\r$\n$\r$\n安装需要先关闭它 —— 否则它会用内存里的旧状态覆盖插件注册信息,$\r$\n导致插件装上了却无法加载(无任何报错)。$\r$\n$\r$\n是否现在关闭 Sigma FM 并继续安装?$\r$\n$\r$\n选择「否」将取消本次安装。" /SD IDYES IDYES 0 IDNO 0
        Pop $0
        ; Only an EXPLICIT "No" (IDNO = 7) cancels. Under /S the dialog is
        ; suppressed and NSIS returns 0, which must NOT be read as a refusal --
        ; otherwise unattended installs could never close a running app.
        ; Safety is not weakened: the post-condition assertion below still
        ; aborts if the app is still alive afterwards.
        DetailPrint "    (prompt returned: $0)"
        ${If} $0 == 7
            DetailPrint "==> 用户选择不关闭 Sigma FM,安装已取消。"
            Abort
        ${EndIf}

        DetailPrint "==> 用户已确认。正在关闭 Sigma FM(先尝试正常退出,最多等 10 秒)..."
        nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\bin\kill-sigma.ps1"'
        Pop $0
        StrCpy $SIGMAWASRUNNING "1"
    ${Else}
        DetailPrint "==> Sigma FM is not running (good)."
    ${EndIf}

    ; ---- Stage 1 post-condition: do not trust the kill, verify it ----
    ; Sigma FM rewrites user-extensions.json from memory on its next save, so
    ; installing while it is alive produces exactly the "installed but the
    ; registration silently vanished" failure this project has hit before.
    ; Assert the invariant instead of assuming it.
    nsExec::ExecToLog 'powershell -NoProfile -Command "$$p = Get-Process -Name sigma-file-manager -ErrorAction SilentlyContinue; if ($$p) { exit 0 } else { exit 1 }"'
    Pop $0
    ${If} $0 == "0"
        DetailPrint "==> ERROR: Sigma FM is STILL running after the kill attempt."
        DetailPrint "    Continuing would let it overwrite user-extensions.json."
        DetailPrint "    Close Sigma File Manager and run this installer again."
        ; Silent mode (/S) prints to no console, so leave evidence on disk.
        FileOpen $1 "$TEMP\focus-sync-install-FAILED.txt" a
        FileWrite $1 "INSTALL ABORTED: Sigma FM (process 'sigma-file-manager') was still running after the Stage 1 kill attempt.$\r$\nIt would have overwritten user-extensions.json. Close it and re-run.$\r$\n"
        FileClose $1
        ; No MessageBox here on purpose: NSIS still opens the dialog under /S
        ; and blocks forever with nobody there to click it. DetailPrint covers
        ; interactive runs; the marker file covers silent ones.
        DetailPrint "    Aborted. Marker written to %TEMP%\focus-sync-install-FAILED.txt"
        Abort
    ${EndIf}
    DetailPrint "==> Verified: Sigma FM is not running."

    ; ---- Stage 1.5 (v0.3.0): Kill legacy v0.2.0 spawn-mode sidecar. ----
    ; v2 review V3.5: precise PowerShell path filter (NOT wildcard "*kizemo*").
    DetailPrint "==> Stage 1.5: Killing legacy v0.2.0 spawn-mode sidecar (precise path filter)..."
    ; "$$_" is required: NSIS expands a bare "$_" as an (empty) variable, which
    ; turns the filter into the invalid "Where-Object { .Path -like ... }" and
    ; makes this whole stage a silent no-op. Same escaping reason as "$$p" above.
    nsExec::ExecToLog 'powershell -NoProfile -Command "Get-Process focus-sync-sidecar -ErrorAction SilentlyContinue | Where-Object { $$_.Path -like ''*\Sigma File Manager\tools\focus-sync-sidecar.exe'' -or $$_.Path -like ''*\kizemo.focus-sync\bin\focus-sync-sidecar.exe'' } | Stop-Process -Force"'
    Pop $0
    ; Non-zero exit is fine (no legacy sidecar is the normal case).

    ; ---- Stage 2: Create installer metadata dir ----
    DetailPrint "==> Stage 2: Creating installer metadata dir at $INSTDIR..."
    SetOutPath "$INSTDIR"
    File "register.ps1"
    File "unregister.ps1"
    File "README.md"

    ; ---- Stage 2.5 (v0.3.0): Bundle PS scripts + deploy the sidecar ----
    ; v2 review S2: use NSIS `File` directive (NOT `!include` which is a
    ; preprocessor directive). The PS scripts go to $INSTDIR\bin\ so the
    ; uninstaller can find them again.
    ;
    ; 2026-10-08 FIX: the sidecar used to land in "$INSTDIR\bin\", i.e.
    ; %LOCALAPPDATA%\Programs\kizemo.focus-sync\bin\. Nothing else ever
    ; looked there -- manual-install.ps1, register.ps1's canonical probe and
    ; the live KizemoFocusSync Scheduled Task all use the extension's own
    ; bin\ dir. The result was a second, orphaned copy of the binary and a
    ; deploy-sha gate that could never pass. One canonical location now.
    ; ----
    SetOutPath "$INSTDIR\bin"
    File "register-scheduled-task.ps1"
    File "unregister-scheduled-task.ps1"

    SetOutPath "${SIDECAR_DEST_DIR}"
    File "..\bin\focus-sync-sidecar.exe"

    ; ---- Stage 3: Copy extension files to %APPDATA% ----
    DetailPrint "==> Stage 3: Copying extension files to ${SIGMA_USERDATA}\extensions\${PRODUCT_EXTDIR}\"
    SetOutPath "${SIGMA_USERDATA}\extensions\${PRODUCT_EXTDIR}"
    File "..\package.json"

    SetOutPath "${SIGMA_USERDATA}\extensions\${PRODUCT_EXTDIR}\dist"
    File "..\dist\index.js"
    File "..\dist\index.js.map"

    SetOutPath "${SIGMA_USERDATA}\extensions\${PRODUCT_EXTDIR}\locales"
    File "..\locales\en.json"
    File "..\locales\zh-CN.json"

    ; ---- Stage 4 (v0.3.0): REMOVED — Sigma FM no longer spawns sidecar. ----
    ; The sidecar binary lives at ${SIDECAR_DEST_PATH} and is launched by
    ; Windows Task Scheduler (stage 6.5 below), NOT by Sigma FM.

    ; ---- Stage 5: Register in user-extensions.json via PowerShell ----
    DetailPrint "==> Stage 5: Registering extension in user-extensions.json..."
    nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\register.ps1"'

    ; ---- Stage 6 (v0.3.0): Write registry key + register Scheduled Task ----
    DetailPrint "==> Stage 6: Writing SidecarPath registry key (HKCU\SOFTWARE\kizemo\focus-sync\)..."
    WriteRegStr HKCU "SOFTWARE\kizemo\focus-sync" "SidecarPath" "${SIDECAR_DEST_PATH}"

    DetailPrint "==> Stage 6.5: Registering Scheduled Task 'KizemoFocusSync'..."
    nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\bin\register-scheduled-task.ps1" -SidecarPath "${SIDECAR_DEST_PATH}"'
    Pop $0
    ${If} $0 != "0"
        DetailPrint "==> WARNING: register-scheduled-task.ps1 exited with code $0"
    ${EndIf}

    ; ---- Stage 7: Write uninstaller ----
    DetailPrint "==> Stage 7: Writing uninstaller..."
    WriteUninstaller "$INSTDIR\uninst.exe"

    ; ---- Stage 8: Re-open Sigma FM if we killed it at Stage 1 ----
    ; Same reason as Stage 1: inline nsExec, the Function form did not run.
    ${If} $SIGMAWASRUNNING == "1"
        DetailPrint "==> Stage 8: Re-opening Sigma File Manager..."
        nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\bin\reopen-sigma.ps1"'
        Pop $0
        ${If} $0 == "0"
            DetailPrint "==> Sigma FM re-opened."
        ${Else}
            DetailPrint "==> WARNING: Could not re-open Sigma FM automatically."
            DetailPrint "    User must launch manually from the install directory."
        ${EndIf}
    ${EndIf}

    DetailPrint "==> [Focus Sync Installer] END"
    DetailPrint "    Extension files:    ${SIGMA_USERDATA}\extensions\${PRODUCT_EXTDIR}"
    DetailPrint "    Sidecar binary:     ${SIDECAR_DEST_PATH}"
    DetailPrint "    PS scripts:         $INSTDIR\bin\register-scheduled-task.ps1"
    DetailPrint "                        $INSTDIR\bin\unregister-scheduled-task.ps1"
    DetailPrint "    Registry:           HKCU\SOFTWARE\kizemo\focus-sync\SidecarPath"
    DetailPrint "    Scheduled Task:     KizemoFocusSync (AtLogOn, --service mode)"
    DetailPrint "    user-extensions.json updated (with isLocal: true)."
    DetailPrint "    Sigma FM was running at start: $SIGMAWASRUNNING (1 = killed+reopened, 0 = not running)"
SectionEnd

; ---------------------------------------------------------------------------
; Uninstaller
; ---------------------------------------------------------------------------
Section "Uninstall"
    DetailPrint "==> [Focus Sync Uninstaller] BEGIN"

    ; Remove extension files
    DetailPrint "==> Removing extension files..."
    RMDir /r "${SIGMA_USERDATA}\extensions\${PRODUCT_EXTDIR}"
    ; Legacy cleanup: round 7 placed sidecar under binaries\ instead of
    ; extensions\<id>\bin\ -- remove that older location too.
    ; (No trailing backslash on a comment line: NSIS reads it as a line
    ; continuation and silently swallows the RMDir below.)
    RMDir /r "${SIGMA_USERDATA}\binaries\${PRODUCT_SIDECAR}"

    ; Unregister from user-extensions.json
    DetailPrint "==> Unregistering from user-extensions.json..."
    nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\unregister.ps1"'
    Pop $0
    ${If} $0 != "0"
        DetailPrint "==> WARNING: PowerShell unregister failed (exit $0)."
    ${EndIf}

    ; ---- Stage v0.3.0: Unregister Scheduled Task + kill orphan sidecar. ----
    DetailPrint "==> Unregistering Scheduled Task and killing orphan sidecar..."
    nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\bin\unregister-scheduled-task.ps1"'
    Pop $0

    ; Delete SidecarPath registry key
    DetailPrint "==> Deleting SidecarPath registry key..."
    DeleteRegKey HKCU "SOFTWARE\kizemo\focus-sync"

    ; Remove installer metadata dir
    DetailPrint "==> Removing installer metadata..."
    RMDir /r "$INSTDIR"

    DetailPrint "==> [Focus Sync Uninstaller] END"
SectionEnd

; ---------------------------------------------------------------------------
; Uninstaller init (write uninstaller.exe to known path)
; ---------------------------------------------------------------------------
Function un.onInit
    ; Detect if Sigma FM is running
    nsExec::ExecToLog 'powershell -NoProfile -Command "$$p = Get-Process -Name sigma-file-manager -ErrorAction SilentlyContinue; if ($$p) { exit 0 } else { exit 1 }"'
    Pop $0
    ${If} $0 == "0"
        ; Sigma FM is running — confirm user wants to continue
        MessageBox MB_ICONQUESTION|MB_YESNO|MB_DEFBUTTON2 "Sigma FM is currently running.$\r$\n$\r$\nUninstall cannot fully clean user-extensions.json while Sigma FM holds the in-memory state.$\r$\n$\r$\nContinue anyway?$\r$\n(You should close Sigma FM first for a clean uninstall)"
        ; MessageBox sets $0 to 6 (YES) or 7 (NO)
        ${If} $0 == "7"
            Abort
        ${EndIf}
    ${EndIf}
FunctionEnd