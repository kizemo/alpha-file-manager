# cc-haha Sidebar Rebuild — One-Click Script (E: path corrected)
#
# Run this in PowerShell as Administrator (or normal user, but exit cc-haha first):
#   powershell -ExecutionPolicy Bypass -File scripts\cc-haha-sidebar-rebuild.ps1
#
# What it does:
#   1. Stops cc-haha
#   2. Backs up E: trace-index (small, fast)
#   3. Deletes trace-index-v1.sqlite + (1).sqlite + shm/wal
#   4. Restarts cc-haha (it will rebuild from E:\Users\Dummy\.claude\cc-haha\traces\)
#
# Monitor progress: Get-Item E:\Users\Dummy\.claude\cc-haha\db\trace-index-v1.sqlite |
#                    ForEach-Object { Write-Host "$([math]::Round($_.Length/1MB,1)) MB" }
#
# To roll back: .\scripts\cc-haha-sidebar-rebuild.ps1 -Rollback

param(
    [switch]$Rollback = $false,
    [switch]$DryRun = $false,
    [switch]$NoRestart = $false,
    [int]$WaitMinutes = 30
)

$ErrorActionPreference = 'Stop'

$dbDir = 'E:\Users\Duanyi\.claude\cc-haha\db'
$bakDir = 'E:\Users\Duanyi\.claude\cc-haha.db.bak.pre-rebuild'
$exe = 'C:\Program Files\Claude Code Haha\Claude Code Haha.exe'
$procName = 'Claude Code Haha'

function Write-Step {
    param([string]$Msg, [string]$Color = 'Cyan')
    Write-Host ""
    Write-Host "=== $Msg ===" -ForegroundColor $Color
}

function Test-EHome {
    if (-not (Test-Path 'E:\Users\Duanyi\.claude\.claude.json')) {
        Write-Error "E:\Users\Duanyi\.claude\.claude.json not found. cc-haha may not use E: as primary home. Aborting."
        exit 1
    }
}

function Stop-CC {
    Write-Step "Stopping cc-haha"
    Get-Process -Name $procName -ErrorAction SilentlyContinue | ForEach-Object {
        Write-Host "  Closing PID $($_.Id)..."
        Stop-Process -Id $_.Id -Force
    }
    Start-Sleep -Seconds 2
    $remaining = Get-Process -Name $procName -ErrorAction SilentlyContinue
    if ($remaining) {
        Write-Error "cc-haha still running. Close manually and retry."
        exit 1
    }
    Write-Host "  cc-haha stopped." -ForegroundColor Green
}

function Start-CC {
    Write-Step "Starting cc-haha"
    if (-not (Test-Path $exe)) {
        Write-Host "  Executable not found at $exe, trying Start menu..." -ForegroundColor Yellow
        Start-Process $procName -ErrorAction SilentlyContinue
    } else {
        Start-Process $exe
    }
    Start-Sleep -Seconds 3
    Write-Host "  cc-haha start signal sent." -ForegroundColor Green
}

function Backup-DB {
    Write-Step "Backing up trace-index"
    if (Test-Path $bakDir) {
        Write-Host "  Backup dir already exists: $bakDir" -ForegroundColor Yellow
        return
    }
    New-Item -Path $bakDir -ItemType Directory -Force | Out-Null
    $dbFiles = Get-ChildItem "$dbDir\trace-index-v1*.sqlite*"
    foreach ($f in $dbFiles) {
        Copy-Item $f.FullName $bakDir -Force
        Write-Host "  Copied: $($f.Name) ($([math]::Round($f.Length/1MB,1)) MB)"
    }
    Write-Host "  Backup done: $bakDir" -ForegroundColor Green
}

function Remove-DB {
    Write-Step "Removing trace-index (forces rebuild on restart)"
    $dbFiles = Get-ChildItem "$dbDir\trace-index-v1*.sqlite*"
    foreach ($f in $dbFiles) {
        Write-Host "  Removing: $($f.Name)"
        Remove-Item $f.FullName -Force
    }
}

function Do-Rebuild {
    Test-EHome
    if (-not (Test-Path $dbDir)) {
        Write-Error "DB dir not found: $dbDir"
        exit 1
    }
    Backup-DB
    Remove-DB
    if ($NoRestart) {
        Write-Host ""
        Write-Host "Done (NoRestart). Restart cc-haha manually." -ForegroundColor Yellow
        return
    }
    Start-CC
    Write-Step "Monitoring trace-index growth (will check every 10s)"
    $deadline = (Get-Date).AddMinutes($WaitMinutes)
    $lastSize = -1
    while ((Get-Date) -lt $deadline) {
        $f = "$dbDir\trace-index-v1.sqlite"
        if (Test-Path $f) {
            $sz = (Get-Item $f).Length
            $szMB = [math]::Round($sz/1MB, 1)
            if ($sz -ne $lastSize) {
                Write-Host "  $(Get-Date -Format 'HH:mm:ss')  trace-index: $szMB MB"
                $lastSize = $sz
            }
            if ($sz -gt 130MB) {
                Write-Host ""
                Write-Host "trace-index reached 130+ MB — likely complete." -ForegroundColor Green
                Write-Host "Verify with: python -c `"import sqlite3; c=sqlite3.connect(r'E:\Users\Dummy\.claude\cc-haha\db\trace-index-v1.sqlite'); print(c.execute('SELECT COUNT(*) FROM trace_sessions').fetchone()[0])`""
                return
            }
        } else {
            Write-Host "  $(Get-Date -Format 'HH:mm:ss')  trace-index: not yet created"
        }
        Start-Sleep -Seconds 10
    }
    Write-Host ""
    Write-Host "Timed out after $WaitMinutes minutes. Check cc-haha sidebar manually." -ForegroundColor Yellow
}

function Do-Rollback {
    Test-EHome
    Write-Step "Rolling back trace-index from backup"
    if (-not (Test-Path $bakDir)) {
        Write-Error "No backup found at $bakDir"
        exit 1
    }
    Stop-CC
    Remove-Item "$dbDir\trace-index-v1*.sqlite*" -Force -ErrorAction SilentlyContinue
    Copy-Item "$bakDir\*" $dbDir -Recurse -Force
    Write-Host "  Restored from $bakDir" -ForegroundColor Green
    Start-CC
}

if ($Rollback) {
    Do-Rollback
} else {
    Do-Rebuild
}