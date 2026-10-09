# unregister-scheduled-task.ps1
#
# v0.3.0 (focus-sync Scheduled Task migration)
# Removes the KizemoFocusSync Scheduled Task and kills any orphan sidecar
# process left running. Used by POSTUNINSTALL of both Installer A (bundled)
# and Installer B (standalone).
#
# Idempotent: if the task is already gone, exit 0 without error.
#
# v3 review V3.10: delete task BEFORE killing the process. Reason — if the
# sidecar is currently processing an in-flight /set_path, killing it loses
# that work. By deleting the task first, Task Scheduler won't restart it
# when we then kill the process. Sleep 1s lets any in-flight /set_path
# finish.
#
# v3 review V3.5: precise path filter, not "*kizemo*" wildcard. Avoids
# killing unrelated programs whose path happens to contain "kizemo".

[CmdletBinding()]
param()

$ErrorActionPreference = 'Continue'  # never abort unregister; best-effort cleanup

$taskName = 'KizemoFocusSync'

# ---------------------------------------------------------------------------
# Step 1: delete the task. schtasks /Delete is idempotent (returns 0 on
# success, 1 if not found). We treat both as success.
# ---------------------------------------------------------------------------
$deleteOutput = schtasks /Delete /TN $taskName /F 2>&1
$deleteExit = $LASTEXITCODE
if ($deleteExit -ne 0 -and $deleteExit -ne 1) {
    Write-Host "WARNING: schtasks /Delete exit $deleteExit: $deleteOutput"
}

# ---------------------------------------------------------------------------
# Step 2: brief wait so any in-flight /set_path finishes before we kill
# the sidecar. v3 review V3.10 — reduces lost-request rate.
# ---------------------------------------------------------------------------
Start-Sleep -Seconds 1

# ---------------------------------------------------------------------------
# Step 3: kill any orphan sidecar process. v3 review V3.5 — precise path
# match. We accept that focus-sync-sidecar.exe with paths OUTSIDE our
# canonical install dirs is left alone (the user installed manually; not
# our concern to clean up).
# ---------------------------------------------------------------------------
$orphanKilled = $false
Get-Process focus-sync-sidecar -ErrorAction SilentlyContinue |
    Where-Object {
        $_.Path -like '*\Sigma File Manager\tools\focus-sync-sidecar.exe' -or
        $_.Path -like '*\kizemo.focus-sync\bin\focus-sync-sidecar.exe'
    } |
    ForEach-Object {
        try {
            Stop-Process -Id $_.Id -Force -ErrorAction Stop
            $orphanKilled = $true
            Write-Host "Killed orphan sidecar pid=$($_.Id)"
        } catch {
            Write-Host "WARNING: failed to kill pid=$($_.Id): $($_.Exception.Message)"
        }
    }

exit 0
