; ============================================================================
; Sigma File Manager - Custom NSIS installer hooks
; ============================================================================
; This file is referenced by tauri.conf.json via:
;   bundle.windows.nsis.installerHooks = "installer/hooks.nsh"
;
; It uses Tauri 2.x's installerHooks feature to inject custom NSIS
; installation steps at four hook points:
;   NSIS_HOOK_PREINSTALL    - before main install
;   NSIS_HOOK_POSTINSTALL   - after main install (extension deployment)
;   NSIS_HOOK_PREUNINSTALL  - before main uninstall
;   NSIS_HOOK_POSTUNINSTALL - after main uninstall (extension removal)
;
; The kizemo.focus-sync extension and focus-sync-sidecar binary are
; declared in bundle.resources, which Tauri copies to $INSTDIR\
; (using the destination path from tauri.conf.json as a relative
; subpath, e.g. extensions\kizemo.focus-sync\ and bin\focus-sync-sidecar\).
; The POSTINSTALL hook then copies them to
; %APPDATA%\com.sigma-file-manager.app\, which is where Sigma's
; extension runtime expects them.
; ============================================================================

; Round 19d: Custom var to hold the resolved original user's APPDATA path.
; (We can't use $USERAPPDATA because NSIS 3.x reserves it as a built-in constant.)
Var RESOLVEDAPPDATA

!macro NSIS_HOOK_PREINSTALL
  ; ---------------------------------------------------------------------
  ; Round 19e (2026-09-30): Defensive kill of any running sigma-file-manager.exe
  ; before file writes. POSTINSTALL writes user-extensions.json + extension
  ; files; if Sigma FM is running (in-place upgrade without uninstall), its
  ; in-memory state overwrites our disk writes on the next UI save. Tauri
  ; only kills the app during UNINSTALL, not INSTALL. This is a net for both
  ; scenarios — uninstall-then-install (stuck subprocess) AND in-place upgrade.
  ;
  ; v0.5.1 (2026-10-05): Tauri's `CheckIfAppIsRunning` (uses Windows
  ; Restart Manager) runs AFTER our PREINSTALL (line 637) and aborts
  ; with "Failed to kill Sigma File Manager" if RSTRTMGR::RmShutdown
  ; can't gracefully close the app. We pre-kill with taskkill /F /T,
  ; plus also kill common case variants and Electron helper processes
  ; that Tauri apps spawn.
  ; ---------------------------------------------------------------------
  DetailPrint "==> [PREINSTALL HOOK] Killing Sigma FM and helpers (taskkill /F /T) ..."
  ; Case variants seen in the wild: 'sigma-file-manager.exe' (Tauri),
  ; 'SigmaFileManager.exe' (case variant on some forks), and the
  ; Electron helpers 'SigmaFileManagerHelper.exe' + 'Sigma File Manager.exe'.
  ; v0.5.2: extend kill list to cover Chromium renderers + WebView2 +
  ; crashpad + msedgewebview2 (these can hold the .exe via mmap even after
  ; the main process exits). RSTRTMGR::RmGetList finds ALL of these; taskkill /F /T
  ; must clear them before Tauri's CheckIfAppIsRunning passes.
  nsExec::ExecToLog 'taskkill /F /IM sigma-file-manager.exe /T'
  Pop $0
  nsExec::ExecToLog 'taskkill /F /IM SigmaFileManager.exe /T'
  Pop $0
  nsExec::ExecToLog 'taskkill /F /IM "Sigma File Manager.exe" /T'
  Pop $0
  nsExec::ExecToLog 'taskkill /F /IM SigmaFileManagerHelper.exe /T'
  Pop $0
  nsExec::ExecToLog 'taskkill /F /IM SigmaFileManagerRenderer.exe /T'
  Pop $0
  nsExec::ExecToLog 'taskkill /F /IM "Sigma File Manager Renderer.exe" /T'
  Pop $0
  nsExec::ExecToLog 'taskkill /F /IM msedgewebview2.exe /T'
  Pop $0
  nsExec::ExecToLog 'taskkill /F /IM crashpad_handler.exe /T'
  Pop $0
  ; Also any msiexec / WerFault / TiWorker that might be holding files.
  nsExec::ExecToLog 'taskkill /F /IM TiWorker.exe /T 2>nul'
  Pop $0
  nsExec::ExecToLog 'taskkill /F /IM WerFault.exe /T 2>nul'
  Pop $0
  nsExec::ExecToLog 'taskkill /F /IM msiexec.exe /T 2>nul'
  Pop $0
  ${If} $0 == "0"
    DetailPrint "==> Sigma FM was running; terminated (taskkill exit 0)."
  ${ElseIf} $0 == "128"
    DetailPrint "==> No Sigma FM process found (taskkill exit 128 = no match)."
  ${Else}
    DetailPrint "==> taskkill returned $0 (continuing)."
  ${EndIf}

  ; ---------------------------------------------------------------------
  ; v0.5.1 (2026-10-05): Defensive kill of the focus-sync sidecar
  ; (`spike.exe`, NOT `focus-sync-sidecar.exe`) before any file writes.
  ; The Scheduled Task \KizemoFocusSync launches the binary as `spike.exe`,
  ; so a running Scheduled Task holds focus-sync-sidecar.exe open, which
  ; blocks NSIS's MoveFileEx and Windows's DeleteFileW.
  ;
  ; Without this, every install attempt that runs while the sidecar is
  ; alive fails with "Error opening file for writing", and uninstall
  ; fails with "Operation could not be completed because the file is
  ; open in focus-sync-sidecar.exe".
  ;
  ; Also disable the Scheduled Task so it doesn't immediately respawn
  ; spike.exe before our file writes complete.
  ; ---------------------------------------------------------------------
  DetailPrint "==> [PREINSTALL HOOK] Killing any existing focus-sync sidecar (spike.exe) ..."
  nsExec::ExecToLog 'taskkill /F /IM spike.exe /T'
  Pop $0
  ${If} $0 == "0"
    DetailPrint "==> spike.exe was running; terminated (taskkill exit 0)."
  ${ElseIf} $0 == "128"
    DetailPrint "==> No spike.exe process found (taskkill exit 128 = no match)."
  ${Else}
    DetailPrint "==> taskkill returned $0 (continuing)."
  ${EndIf}
  ; Also try the legacy alias name in case an older sidecar still uses it.
  nsExec::ExecToLog 'taskkill /F /IM focus-sync-sidecar.exe /T'
  Pop $0
  ; Disable the Scheduled Task so it doesn't auto-respawn the sidecar
  ; during the brief window between kill and file overwrite.
  nsExec::ExecToLog 'schtasks /Change /TN "\KizemoFocusSync" /DISABLE 2>nul'
  Pop $0
  ; Brief settle delay so the process can release any open handles.
  Sleep 1500

  ; ---------------------------------------------------------------------
  ; Detect an existing Sigma File Manager installation and reuse its
  ; InstallLocation. Priority:
  ;   1. Tauri-style registry key (this fork's installer)
  ;   2. Legacy makensis registry key (upstream AppName = "Sigma File Manager")
  ;   3. Filesystem probe under %PROGRAMFILES% for nested layout
  ;
  ; This makes the installer "smart-upgrade": it lands in the same folder
  ; the user previously chose, instead of switching to %LOCALAPPDATA%.
  ;
  ; Note: Tauri NSIS template also has its own RestorePreviousInstallLocation
  ; in .onInit reading MANUPRODUCTKEY (HKLM\SOFTWARE\sigma-file-manager\Sigma File
  ; Manager). It typically wins for fork-on-fork upgrades. This hook covers
  ; cases Tauri misses (legacy makensis installs, FS-only installs).
  ; ---------------------------------------------------------------------
  DetailPrint "==> [PREINSTALL HOOK START] $INSTDIR (before detection) = $INSTDIR"
  StrCpy $0 ""

  ; 1. Tauri registry key (perMachine install via current installer)
  ReadRegStr $0 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\com.sigma-file-manager.app_is1" "InstallLocation"
  ${If} $0 == ""
    ReadRegStr $0 HKLM "SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\com.sigma-file-manager.app_is1" "InstallLocation"
  ${EndIf}

  ; 2. Legacy makensis registry key (upstream AppName)
  ${If} $0 == ""
    ReadRegStr $0 HKLM "SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Sigma File Manager_is1" "InstallLocation"
  ${EndIf}

  ; 3. Filesystem probe — covers cases where the previous installer didn't
  ;    write the registry (e.g. portable extract, or wiped uninstall key).
  ${If} $0 == ""
    ${If} ${FileExists} "$PROGRAMFILES\Sigma File Manager\Sigma File Manager\Sigma File Manager.exe"
      StrCpy $0 "$PROGRAMFILES\Sigma File Manager\Sigma File Manager"
    ${ElseIf} ${FileExists} "$PROGRAMFILES\Sigma File Manager\Sigma File Manager.exe"
      StrCpy $0 "$PROGRAMFILES\Sigma File Manager"
    ${ElseIf} ${FileExists} "$PROGRAMFILES\Sigma File Manager.exe"
      StrCpy $0 "$PROGRAMFILES"
    ${EndIf}
  ${EndIf}

  ${If} $0 != ""
    DetailPrint "==> [PREINSTALL HOOK] Found existing Sigma FM at: $0"
    DetailPrint "    Reusing InstallDir (smart-upgrade)."
    StrCpy $INSTDIR "$0"
  ${Else}
    DetailPrint "==> [PREINSTALL HOOK] No existing Sigma FM install detected via hook; using default path ($INSTDIR)."
  ${EndIf}
  DetailPrint "==> [PREINSTALL HOOK END] $INSTDIR (after detection) = $INSTDIR"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "==> [POSTINSTALL HOOK START] Deploying kizemo.focus-sync v0.3.0 (Scheduled Task mode) ..."

  ; ---------------------------------------------------------------------
  ; STAGE 0 (round 19d): Resolve the ORIGINAL USER's %APPDATA% before any
  ; file writes or registration. A perMachine installer runs in admin
  ; context, so NSIS's $APPDATA resolves to admin's profile, NOT the
  ; original user's. If we wrote to $APPDATA and the original user is
  ; different, Sigma FM (running as user) would never see the files.
  ;
  ; resolve_user_appdata.ps1 queries WMI for the active console session
  ; user (explorer.exe owner) and writes USER_APPDATA=<path>\AppData\Roaming
  ; to a temp file. Falls back to $env:USERPROFILE\AppData\Roaming if WMI is
  ; unavailable.
  ; ---------------------------------------------------------------------
  DetailPrint "==> Stage 0: Resolving original user's %APPDATA% (round 19d fix)..."
  nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\tools\resolve_user_appdata.ps1" -OutFile "$INSTDIR\tools\user_appdata.txt"'
  Pop $0
  ${If} $0 != "0"
    DetailPrint "==> WARNING: resolve_user_appdata.ps1 exited with code $0"
    DetailPrint "    Falling back to PowerShell $$env:USERPROFILE\AppData\Roaming"
    ; "$$env:..." is required: NSIS parses "$env:USERPROFILE\AppData\Roaming"
    ; as ONE variable name (it allows ':' and '\' in names), swallows the whole
    ; thing and emits nothing. The command then resolved APPDATA to the empty
    ; string "\AppData\Roaming" and wrote the extension to a bogus path --
    ; installed, registered nowhere, i.e. "installed but no functionality".
    nsExec::ExecToLog 'powershell -NoProfile -Command "Write-Output USER_APPDATA=$$env:USERPROFILE\AppData\Roaming | Out-File -FilePath $INSTDIR\tools\user_appdata.txt -Encoding ASCII -Force"'
    Pop $0
    FileOpen $R0 "$INSTDIR\tools\user_appdata.txt" r
    ${If} $R0 == ""
      DetailPrint "==> FATAL: Could not resolve APPDATA via fallback PowerShell"
      StrCpy $RESOLVEDAPPDATA "$PROFILE\AppData\Roaming"
    ${Else}
      FileRead $R0 $R1
      FileClose $R0
      StrCpy $RESOLVEDAPPDATA $R1 "" 13
      DetailPrint "==> Fallback resolved: $RESOLVEDAPPDATA"
    ${EndIf}
  ${Else}
    FileOpen $R0 "$INSTDIR\tools\user_appdata.txt" r
    ${If} $R0 == ""
      DetailPrint "==> WARNING: Could not open user_appdata.txt; using $PROFILE\AppData\Roaming"
      StrCpy $RESOLVEDAPPDATA "$PROFILE\AppData\Roaming"
    ${Else}
      FileRead $R0 $R1
      FileClose $R0
      ; $R1 should be "USER_APPDATA=C:\Users\..." — strip the 13-char prefix
      StrCpy $RESOLVEDAPPDATA $R1 "" 13
      ${If} ${FileExists} "$RESOLVEDAPPDATA"
        DetailPrint "==> Resolved RESOLVEDAPPDATA=$RESOLVEDAPPDATA (verified directory)"
      ${Else}
        DetailPrint "==> WARNING: Resolved RESOLVEDAPPDATA=$RESOLVEDAPPDATA does NOT exist on disk"
        DetailPrint "    Falling back to $PROFILE\AppData\Roaming"
        StrCpy $RESOLVEDAPPDATA "$PROFILE\AppData\Roaming"
      ${EndIf}
    ${EndIf}
  ${EndIf}

  ; ---------------------------------------------------------------------
  ; STAGE 0.5 (v0.3.0 focus-sync): Kill any legacy v0.2.0 spawn-mode sidecar
  ; process. v2 review V3.5: precise PowerShell path filter (NOT wildcard
  ; "*kizemo*") to avoid killing unrelated processes. Without this, the
  ; 37421 port would be held by the old binary, and the new Scheduled Task
  ; would fail-fast exit 0 at startup (see T1.1 config.rs).
  ;
  ; The /IM filter requires the exact exe name; the PS Where-Object filters
  ; by Path to confirm it's our binary before killing.
  ; ---------------------------------------------------------------------
  DetailPrint "==> Stage 0.5: Killing legacy v0.2.0 spawn-mode sidecar (precise path filter)..."
  ; "$$_" is required: NSIS expands a bare "$_" as an (empty) variable, which
  ; turns this filter into the invalid "Where-Object { .Path -like ... }". The
  ; command then failed to parse and this stage silently did nothing -- the old
  ; sidecar survived, kept port 37421, and the new Scheduled Task failed fast.
  ; makensis reported this as "warning 6000", but the Tauri bundler swallows
  ; makensis warnings, so it was invisible in the build log.
  nsExec::ExecToLog 'powershell -NoProfile -Command "Get-Process focus-sync-sidecar,spike -ErrorAction SilentlyContinue | Where-Object { $$_.Path -like ''*\Sigma File Manager\tools\focus-sync-sidecar.exe'' -or $$_.Path -like ''*\kizemo.focus-sync\bin\focus-sync-sidecar.exe'' } | Stop-Process -Force"'
  Pop $0
  ; Non-zero exit is fine (no process to kill is success-equivalent)

  ; v0.5.1 (2026-10-05): Also kill any Scheduled-Task-launched sidecar. The
  ; binary at $INSTDIR\tools\focus-sync-sidecar.exe is launched by the
  ; task as `spike.exe` (Cargo.toml [[bin]] name = "spike"). PREINSTALL
  ; above also kills it; this is belt-and-suspenders for the install case.
  nsExec::ExecToLog 'taskkill /F /IM spike.exe /T 2>nul'
  Pop $0

  ; ---------------------------------------------------------------------
  ; STAGE 1-3 (unchanged from v0.2.0): Copy extension files to APPDATA.
  ; These files are still read by Sigma FM's extension runtime at startup.
  ; The binaries[] segment is now EMPTY in package.json (v0.3.0), so Sigma FM
  ; will NOT try to spawn the sidecar — that's now done by Task Scheduler.
  ; ---------------------------------------------------------------------

  ; 1. Extension package.json (parent dir auto-created by SetOutPath)
  SetOutPath "$RESOLVEDAPPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync"
  CopyFiles /SILENT "$INSTDIR\extensions\kizemo.focus-sync\package.json" \
                    "$RESOLVEDAPPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\package.json"

  ; 2. Extension dist/* (index.js + .map)
  SetOutPath "$RESOLVEDAPPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\dist"
  CopyFiles /SILENT "$INSTDIR\extensions\kizemo.focus-sync\dist\index.js" \
                    "$RESOLVEDAPPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\dist\index.js"
  CopyFiles /SILENT "$INSTDIR\extensions\kizemo.focus-sync\dist\index.js.map" \
                    "$RESOLVEDAPPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\dist\index.js.map"

  ; 3. Extension locales/*
  SetOutPath "$RESOLVEDAPPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\locales"
  CopyFiles /SILENT "$INSTDIR\extensions\kizemo.focus-sync\locales\en.json" \
                    "$RESOLVEDAPPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\locales\en.json"
  CopyFiles /SILENT "$INSTDIR\extensions\kizemo.focus-sync\locales\zh-CN.json" \
                    "$RESOLVEDAPPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\locales\zh-CN.json"

  ; ---------------------------------------------------------------------
  ; STAGE 4 (v0.3.0 focus-sync): STAGES 4 AND 5 (APPDA TA binary copy) REMOVED.
  ; Sigma FM no longer needs the sidecar binary in APPDATA — Scheduled Task
  ; launches it from $INSTDIR\tools\ directly.
  ;
  ; Old v0.2.0 binaries in APPDATA are stale; if user later downgrades to
  ; v0.2.0 they would resurrect — but that's outside our supported scope.
  ; ---------------------------------------------------------------------

  ; ---------------------------------------------------------------------
  ; STAGE 6 (unchanged): Register extension in user-extensions.json
  ; ---------------------------------------------------------------------
  DetailPrint "==> Stage 6: Registering extension in user-extensions.json..."
  nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\tools\register.ps1" -UserAppData "$RESOLVEDAPPDATA"'
  Pop $0
  ${If} $0 != "0"
    DetailPrint "==> WARNING: register.ps1 exited with code $0."
    DetailPrint "    Files were copied, but the extension may not appear in Sigma FM's Installed tab."
  ${Else}
    DetailPrint "==> Registration successful."
  ${EndIf}

  ; ---------------------------------------------------------------------
  ; STAGE 7 (v0.3.0 focus-sync): deploy the sidecar to the CANONICAL path.
  ;
  ; 2026-10-08: this used to keep the sidecar at $INSTDIR\tools\ and point the
  ; Scheduled Task there. That created a SECOND canonical path -- the standalone
  ; installer, manual-install.ps1, register.ps1's canonical probe and the sha
  ; gate all use the per-user extension bin\ dir. Two paths meant installing A
  ; then B silently repointed the task, left an orphan binary, and made the
  ; acceptance gate unsatisfiable. Tauri extracts bundle.resources into $INSTDIR
  ; anyway, so copy FROM there TO the canonical path, then drop the duplicate.
  ; The canonical path is per-user, so no admin rights are required.
  ; ---------------------------------------------------------------------
  DetailPrint "==> Stage 7: Deploying sidecar to the canonical per-user path..."
  ${IfNot} ${FileExists} "$INSTDIR\tools\focus-sync-sidecar.exe"
    DetailPrint "==> WARNING: sidecar binary missing from installer resources."
    DetailPrint "    Sigma FM Focus Sync extension will not function (Task Scheduler creation skipped)."
    Goto stage9_skip
  ${EndIf}

  CreateDirectory "$RESOLVEDAPPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar"
  CopyFiles "$INSTDIR\tools\focus-sync-sidecar.exe" "$RESOLVEDAPPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe"
  ${IfNot} ${FileExists} "$RESOLVEDAPPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe"
    DetailPrint "==> ERROR: failed to copy sidecar to the canonical path."
    Goto stage9_skip
  ${EndIf}
  DetailPrint "    Sidecar deployed to canonical path."

  ; ---------------------------------------------------------------------
  ; STAGE 8 (v0.3.0 focus-sync): Write registry key SidecarPath.
  ; v2 review §V-Q2: use HKLM\SOFTWARE\kizemo\focus-sync\ (NOT
  ; "Sigma FM\FocusSync\") for cross-installer consistency.
  ; ---------------------------------------------------------------------
  DetailPrint "==> Stage 8: Writing SidecarPath registry key..."
  WriteRegStr HKLM "SOFTWARE\kizemo\focus-sync" "SidecarPath" "$RESOLVEDAPPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe"

  ; ---------------------------------------------------------------------
  ; STAGE 9 (v0.3.0 focus-sync): Register Scheduled Task via PS helper.
  ; The PS helper handles idempotent create, cross-installer path-change
  ; detection (v2 M2), and --service flag injection (v3 V3.22).
  ;
  ; We invoke the PS via $INSTDIR\tools\register-scheduled-task.ps1 which
  ; is bundled into the installer by tauri.conf.json bundle.resources.
  ; ---------------------------------------------------------------------
  DetailPrint "==> Stage 9: Registering Scheduled Task 'KizemoFocusSync'..."
  nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\tools\register-scheduled-task.ps1" -SidecarPath "$RESOLVEDAPPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe"'
  Pop $0
  ${If} $0 != "0"
    DetailPrint "==> WARNING: register-scheduled-task.ps1 exited with code $0"
    DetailPrint "    Scheduled Task may not be created. Run manually: schtasks /Create ..."
  ${EndIf}

  ; Drop the duplicate now that the task points at the canonical copy, so a
  ; later install cannot silently fall back to a stale binary.
  Delete "$INSTDIR\tools\focus-sync-sidecar.exe"
  DetailPrint "    Removed the duplicate copy from $INSTDIR\tools\."

  stage9_skip:

  DetailPrint "==> [POSTINSTALL HOOK END] Deployed kizemo.focus-sync v0.3.0"
  DetailPrint "    Extension files: $RESOLVEDAPPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\"
  DetailPrint "    Sidecar binary:  $RESOLVEDAPPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe (canonical, per-user)"
  DetailPrint "    Registry:        HKLM\SOFTWARE\kizemo\focus-sync\SidecarPath"
  DetailPrint "    Scheduled Task:  KizemoFocusSync (AtLogOn, --service mode)"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Tauri already kills the app. We additionally need to kill the focus-sync
  ; sidecar (`spike.exe`) and disable its Scheduled Task so NSIS can
  ; delete the sidecar binary and remove the Scheduled Task without
  ; "file in use" errors.
  DetailPrint "==> [PREUNINSTALL HOOK] Killing focus-sync sidecar (spike.exe) ..."
  nsExec::ExecToLog 'taskkill /F /IM spike.exe /T'
  Pop $0
  nsExec::ExecToLog 'taskkill /F /IM focus-sync-sidecar.exe /T'
  Pop $0
  ; Disable so it doesn't respawn during file deletion.
  nsExec::ExecToLog 'schtasks /Change /TN "\KizemoFocusSync" /DISABLE 2>nul'
  Pop $0
  ; v0.5.2: also kill any residual Sigma FM processes (if user is uninstalling
  ; only the extension, Sigma FM might still be running with WebView2 children
  ; holding extension files). Belt-and-suspenders.
  nsExec::ExecToLog 'taskkill /F /IM sigma-file-manager.exe /T 2>nul'
  Pop $0
  nsExec::ExecToLog 'taskkill /F /IM SigmaFileManager.exe /T 2>nul'
  Pop $0
  nsExec::ExecToLog 'taskkill /F /IM "Sigma File Manager.exe" /T 2>nul'
  Pop $0
  nsExec::ExecToLog 'taskkill /F /IM SigmaFileManagerHelper.exe /T 2>nul'
  Pop $0
  nsExec::ExecToLog 'taskkill /F /IM SigmaFileManagerRenderer.exe /T 2>nul'
  Pop $0
  nsExec::ExecToLog 'taskkill /F /IM msedgewebview2.exe /T 2>nul'
  Pop $0
  ; Brief settle delay so the process can release any open handles.
  Sleep 1000
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DetailPrint "==> [POSTUNINSTALL HOOK START] Removing kizemo.focus-sync v0.3.0 ..."

  ; Round 19d: resolve the original user's APPDATA before cleanup. Uninstall
  ; also runs in admin context, so we need the same WMI-based path resolution
  ; to delete from the correct user profile (where POSTINSTALL actually wrote).
  DetailPrint "==> Stage 0: Resolving original user's %APPDATA% for cleanup..."
  nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\tools\resolve_user_appdata.ps1" -OutFile "$INSTDIR\tools\user_appdata_uninst.txt"'
  Pop $0
  ${If} $0 != "0"
    DetailPrint "==> WARNING: resolve_user_appdata.ps1 failed during uninstall (code $0)"
    DetailPrint "    Falling back to PowerShell $$env:USERPROFILE\AppData\Roaming"
    nsExec::ExecToLog 'powershell -NoProfile -Command "Write-Output USER_APPDATA=$$env:USERPROFILE\AppData\Roaming | Out-File -FilePath $INSTDIR\tools\user_appdata_uninst.txt -Encoding ASCII -Force"'
    Pop $0
    FileOpen $R0 "$INSTDIR\tools\user_appdata_uninst.txt" r
    ${If} $R0 == ""
      StrCpy $RESOLVEDAPPDATA "$PROFILE\AppData\Roaming"
    ${Else}
      FileRead $R0 $R1
      FileClose $R0
      StrCpy $RESOLVEDAPPDATA $R1 "" 13
      DetailPrint "==> Fallback resolved for cleanup: $RESOLVEDAPPDATA"
    ${EndIf}
  ${Else}
    FileOpen $R0 "$INSTDIR\tools\user_appdata_uninst.txt" r
    ${If} $R0 == ""
      StrCpy $RESOLVEDAPPDATA "$PROFILE\AppData\Roaming"
    ${Else}
      FileRead $R0 $R1
      FileClose $R0
      StrCpy $RESOLVEDAPPDATA $R1 "" 13
      DetailPrint "==> Resolved RESOLVEDAPPDATA for cleanup=$RESOLVEDAPPDATA"
    ${EndIf}
  ${EndIf}

  RMDir /r "$RESOLVEDAPPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync"
  RMDir /r "$RESOLVEDAPPDATA\com.sigma-file-manager.app\binaries\focus-sync-sidecar"

  ; ---------------------------------------------------------------------
  ; STAGE 1 (v0.3.0 focus-sync): Unregister Scheduled Task + kill orphan.
  ; The PS helper handles idempotent delete, kill orphan sidecar process
  ; (v2 review S4 fix), and precise path filter (v3 review V3.5).
  ; ---------------------------------------------------------------------
  DetailPrint "==> Stage 1: Unregistering Scheduled Task and killing orphan sidecar..."
  nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\tools\unregister-scheduled-task.ps1"'
  Pop $0
  ${If} $0 != "0"
    DetailPrint "==> WARNING: unregister-scheduled-task.ps1 exited with code $0"
  ${EndIf}

  ; ---------------------------------------------------------------------
  ; STAGE 2 (v0.3.0 focus-sync): Delete registry key SidecarPath.
  ; Note: HKLM keys are deleted here even if the original user is different
  ; (the key is perMachine).
  ; ---------------------------------------------------------------------
  DetailPrint "==> Stage 2: Deleting SidecarPath registry key..."
  DeleteRegKey HKLM "SOFTWARE\kizemo\focus-sync"

  DetailPrint "==> [POSTUNINSTALL HOOK END] Cleanup complete (extension, sidecar binary, task, registry)."
!macroend
