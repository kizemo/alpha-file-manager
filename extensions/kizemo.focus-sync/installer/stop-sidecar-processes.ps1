# stop-sidecar-processes.ps1
#
# Stops every running focus-sync sidecar that was deployed from a known
# canonical location, so that a new binary can be written over the old one.
#
# Why this exists:
#   On Windows a running image keeps its own .exe locked, so copying a new
#   sidecar over it fails with "file is open by another process". Since
#   v0.5.9 the sidecar is a child process of the Alpha File Manager main
#   process, so it may well be running from the very path the installer is
#   about to overwrite.
#
# Why `powershell -File` and not an inline `-Command`:
#   The real-machine install of 3.13.5 (2026-11-10) proved that the inline
#   `powershell -NoProfile -Command "..."` form does not reliably execute in
#   the NSIS installer context, while `powershell -File "<path>"` does
#   (STAGE 5 has always used it for register.ps1).
#
# Scope: processes only. Removing the legacy Scheduled Task is
# remove-legacy-focus-task.ps1's job, so the two concerns stay separate.
#
# Idempotent: no matching process is a success.
# Pure ASCII on purpose: PowerShell 5.1 reads a BOM-less .ps1 as ANSI.

[CmdletBinding()]
param()

$ErrorActionPreference = 'Continue'

# Every location a sidecar is deployed to by the AFM installer or by this
# standalone installer. The canonical extension path has one more directory
# level than the legacy shared path, so both are listed explicitly.
$patterns = @(
    '*\Alpha File Manager\tools\focus-sync-sidecar.exe',
    '*\Sigma File Manager\tools\focus-sync-sidecar.exe',
    '*\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe',
    '*\kizemo.focus-sync\bin\focus-sync-sidecar.exe'
)

function Get-MatchingSidecars {
    $result = @()
    $procs = @(Get-Process focus-sync-sidecar -ErrorAction SilentlyContinue)
    foreach ($proc in $procs) {
        $path = ''
        try { $path = [string]$proc.Path } catch { $path = '' }
        if (-not $path) { continue }
        foreach ($pat in $patterns) {
            if ($path -like $pat) {
                $result += $proc
                break
            }
        }
    }
    return $result
}

$matching = @(Get-MatchingSidecars)
$killed = 0
foreach ($proc in $matching) {
    $path = ''
    try { $path = [string]$proc.Path } catch { $path = '' }
    try {
        Stop-Process -Id $proc.Id -Force -ErrorAction Stop
        $killed++
        Write-Host ("Killed sidecar pid=" + $proc.Id + " path=" + $path)
    } catch {
        Write-Host ("WARNING: could not kill pid=" + $proc.Id + ": " + $proc.Exception.Message)
    }
}

# The file handle is released only once the process is really gone, so wait
# for it before the installer writes the new binary over the old one.
$waited = 0
while ($waited -lt 20) {
    if (@(Get-MatchingSidecars).Count -eq 0) { break }
    Start-Sleep -Milliseconds 500
    $waited++
}

if (@(Get-MatchingSidecars).Count -ne 0) {
    Write-Host "WARNING: some sidecar processes are still running after 10s."
    exit 1
}

Write-Host ("==> Stopped sidecar processes: " + $killed)
exit 0
