# ============================================================================
# kill-sigma.ps1 — Gracefully kill Sigma File Manager, fall back to force
# ============================================================================
# Called by installer.nsi Stage 1 when Sigma FM is running.
#
# Why: Sigma FM holds an in-memory copy of user-extensions.json and silently
# overwrites the on-disk file on next save. We must kill it before install
# so register.ps1's write survives.
#
# Flow:
#   1. Send WM_CLOSE to all sigma-file-manager processes with a main window
#      (graceful — apps can prompt user before close if dirty).
#   2. Wait up to 10s for graceful exit.
#   3. Force-kill (Stop-Process -Force) anything still alive.
#
# Exit codes:
#   0 = no process was running, or all processes exited cleanly
#   1 = force-kill failed (extremely rare)
# ============================================================================

$ErrorActionPreference = 'Continue'

$procs = Get-Process -Name sigma-file-manager -ErrorAction SilentlyContinue
if (-not $procs) {
    Write-Host "==> No Sigma FM process running."
    exit 0
}

Write-Host "==> Found $($procs.Count) Sigma FM process(es). Sending WM_CLOSE..."

# Step 1: graceful close
foreach ($p in $procs) {
    if ($p.MainWindowHandle -ne 0) {
        try {
            $p.CloseMainWindow() | Out-Null
            Write-Host "    Sent WM_CLOSE to PID $($p.Id)"
        } catch {
            Write-Host "    Failed to send WM_CLOSE to PID $($p.Id): $_"
        }
    } else {
        Write-Host "    PID $($p.Id) has no main window (background); will force-kill"
    }
}

# Step 2: wait up to 10s
$timeout = 10
$elapsed = 0
while ($elapsed -lt $timeout) {
    Start-Sleep -Seconds 1
    $elapsed++
    $still = Get-Process -Name sigma-file-manager -ErrorAction SilentlyContinue
    if (-not $still) {
        Write-Host "==> Sigma FM exited gracefully after ${elapsed}s."
        exit 0
    }
}

# Step 3: force kill
Write-Host "==> Sigma FM did not exit within ${timeout}s; force-killing."
$still | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2

# Final check
$final = Get-Process -Name sigma-file-manager -ErrorAction SilentlyContinue
if ($final) {
    Write-Host "==> ERROR: Could not kill Sigma FM. PIDs: $($final.Id)"
    exit 1
}

Write-Host "==> Sigma FM force-killed."
exit 0