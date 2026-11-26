# ============================================================================
# unregister.ps1 - Remove kizemo.focus-sync from Alpha File Manager's user-extensions.json
# ============================================================================
# Counterpart to register.ps1. Run by the NSIS uninstaller to remove the
# kizemo.focus-sync entry from installedExtensions. Idempotent - does nothing
# if the entry is already absent.
#
# IMPORTANT: Close Sigma File Manager BEFORE running this script.
# ============================================================================

$ErrorActionPreference = 'Stop'

# ---- Paths ----
$appDataDir  = Join-Path $env:APPDATA 'com.sigma-file-manager.app'
$userDataDir = Join-Path $appDataDir 'user-data'
$jsonPath    = Join-Path $userDataDir 'user-extensions.json'

# ---- Validation ----
if (-not (Test-Path $jsonPath)) {
    Write-Warning "Alpha File Manager user-extensions.json not found: $jsonPath. Nothing to unregister."
    exit 0
}

# ---- Read existing JSON ----
# Read as UTF-8 explicitly (same fix as register.ps1, v0.5.9):
# `Get-Content -Raw` in PS 5.1 decodes with the ANSI code page (cp936 here),
# so any non-ASCII byte sequence in the file becomes mojibake BEFORE
# ConvertFrom-Json sees it - and this script writes the result back, which
# would corrupt the em-dash in the other extensions' command titles.
$json = [System.IO.File]::ReadAllText($jsonPath, [System.Text.Encoding]::UTF8) | ConvertFrom-Json

# ---- Backup ----
$timestamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$backupPath = "C:\Temp\user-extensions.json.bak-unregister-$timestamp"
try {
    Copy-Item $jsonPath $backupPath -Force
    Write-Host "==> Backup created: $backupPath"
} catch {
    Write-Warning "Backup failed (continuing anyway): $_"
}

# ---- Remove entry if present ----
$hasEntry = $json.installedExtensions.PSObject.Properties.Name -contains 'kizemo.focus-sync'
if (-not $hasEntry) {
    Write-Host "==> kizemo.focus-sync is not registered. Nothing to do."
    exit 0
}

$json.installedExtensions.PSObject.Properties.Remove('kizemo.focus-sync')
Write-Host "==> Removed kizemo.focus-sync from installedExtensions"

# ---- Serialize (format agnostic; Alpha File Manager re-normalizes on next save) ----
$serialized = $json | ConvertTo-Json -Depth 100

# ---- Write back, no BOM, UTF-8 ----
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($jsonPath, $serialized, $utf8NoBom)

Write-Host "==> Wrote $jsonPath ($((Get-Item $jsonPath).Length) bytes)"
Write-Host "==> Done. Restart Alpha File Manager to see the change."
exit 0