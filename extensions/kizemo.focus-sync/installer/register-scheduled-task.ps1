# register-scheduled-task.ps1
#
# v0.3.0 (focus-sync Scheduled Task migration)
# Creates a per-user Windows Scheduled Task that runs focus-sync-sidecar.exe
# at user logon. The task is the lifecycle owner of the sidecar — Sigma FM
# no longer spawns/kills it, so 7 classes of reload triggers (locale watcher,
# binary edit, install/update, Sigma FM restart, etc.) cannot kill the sidecar.
#
# Called by:
#   - Sigma FM bundled installer (Installer A) POSTINSTALL stage 9 via
#     sigma-file-manager/src-tauri/installer/hooks.nsh
#   - Standalone installer (Installer B) install section
#
# Idempotent: re-running on an already-installed user is safe (skips create,
# restarts task).
#
# Designed for PowerShell 5.1 (Windows 11 default).

# NOTE: v0.3.0 hotfix — removed [CmdletBinding()] because NSIS nsExec::ExecToLog
# invocation (via cmd.exe) was triggering "AmbiguousParameterSet" errors when
# -SidecarPath contained spaces. Without CmdletBinding, the function-style
# parameter binding is unambiguous.

param(
    [Parameter(Mandatory = $true)]
    [string]$SidecarPath
)

$ErrorActionPreference = 'Stop'

$taskName = 'KizemoFocusSync'

# ---------------------------------------------------------------------------
# v2 review M2 (v0.4) — cross-installer path-change detection.
# If an existing task points to a DIFFERENT sidecar binary path
# (e.g. user upgraded from Installer B's $LOCALAPPDATA\... to
# Installer A's $PROGRAMFILES\...$), unregister the stale task before
# creating the new one. Without this, the old task continues to launch
# the old binary, leaving two sidecars fighting over 37421.
# ---------------------------------------------------------------------------
$existingTask = Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
if ($existingTask) {
    $existingAction = $existingTask.Actions[0].Execute
    # v4 review V4.1: PS 5.1 / 6.x may return IExecAction object vs raw string.
    # Force .ToString() for safe -ne comparison.
    if ($existingAction -isnot [string]) {
        $existingAction = $existingAction.ToString()
    }
    if ($existingAction -ne $SidecarPath) {
        Write-Host "INFO: existing task points to '$existingAction', but new path is '$SidecarPath'. Unregistering old task."
        schtasks /Delete /TN $taskName /F | Out-Null
        $existingTask = $null
    }
}

# ---------------------------------------------------------------------------
# Create task if not already present. Idempotent — re-run is safe.
# v3 review V3.22: --service flag is appended by the task action so the
# sidecar knows to use fail-fast port mode. Without it, the sidecar would
# try retry-bind 800ms x 3, which on multi-user machines triggers Task
# Scheduler restart-spam (round 19g root cause).
# ---------------------------------------------------------------------------
if (-not $existingTask) {
    $action = New-ScheduledTaskAction -Execute $SidecarPath -Argument '--service'
    $trigger = New-ScheduledTaskTrigger -AtLogOn
    # v0.3.0 fix: -RunLevel Limited MUST be on New-ScheduledTaskPrincipal ONLY.
    # Setting it on BOTH Principal and Register-ScheduledTask triggers
    # "AmbiguousParameterSet" errors in PowerShell 5.1 (parameter binding
    # conflict between the principal's RunLevel and the cmdlet-level RunLevel).
    $principal = New-ScheduledTaskPrincipal -User $env:USERNAME -LogonType Interactive -RunLevel Limited
    $settings = New-ScheduledTaskSettingsSet `
        -AllowStartIfOnBatteries `
        -DontStopIfGoingOnBatteries `
        -RestartCount 3 `
        -RestartInterval (New-TimeSpan -Minutes 1) `
        -ExecutionTimeLimit (New-TimeSpan -Seconds 0)  # 0 = unlimited (v0.3.0 H1.4 fix)

    Register-ScheduledTask `
        -TaskName $taskName `
        -Action $action `
        -Trigger $trigger `
        -Principal $principal `
        -Settings $settings `
        -Description 'Kizemo Focus Sync sidecar helper (v0.3.0 Scheduled Task mode)' | Out-Null

    Write-Host "OK: Created Scheduled Task '$taskName' pointing to '$SidecarPath'."
}

# ---------------------------------------------------------------------------
# Always try to start the task (covers both first-install and re-run cases
# where user just wants to ensure the task is running NOW, not waiting for
# next logon). v0.3.0 M7: 0x800710D5 = "task already running" — silently OK.
# ---------------------------------------------------------------------------
try {
    Start-ScheduledTask -TaskName $taskName -ErrorAction Stop | Out-Null
} catch {
    $hresult = $_.Exception.HResult
    if ($hresult -ne 0x800710D5) {
        Write-Host "WARNING: Start-ScheduledTask failed: $($_.Exception.Message)"
    }
}

exit 0
