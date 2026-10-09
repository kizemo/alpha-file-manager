# build-with-sidecar.ps1
#
# Builds the Focus Sync sidecar and places it where the installers pick it up.
#
# 2026-10-09: the plugin source moved INTO this repository, under
#   extensions/kizemo.focus-sync/
# Previously this script read from a sibling repository. Both installers now
# resolve everything inside this repo:
#   - Installer A (bundled with Alpha FM): bundles via tauri.conf.json
#     bundle.resources, target tools/focus-sync-sidecar.exe
#   - Installer B (standalone NSIS): bundles via a File directive,
#     source ../bin/focus-sync-sidecar.exe relative to installer/
#
# Usage:
#   1. cd extensions/kizemo.focus-sync/sidecar && cargo build --release
#   2. powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-with-sidecar.ps1
#   3. npm run tauri build

[CmdletBinding()]
param(
    [string]$RepoRoot
)
if (-not $RepoRoot) {
    $RepoRoot = Split-Path $PSScriptRoot -Parent
}

$ErrorActionPreference = 'Stop'

$pluginRoot = Join-Path $RepoRoot 'extensions\kizemo.focus-sync'

# ---- Sandbox gate --------------------------------------------------------
# validateExtensionCode() rejects the WHOLE extension if any sandbox regex
# matches -- including matches inside COMMENTS. On rejection the extension
# never loads and fails SILENTLY: no UI error, no notification, sidecar
# receives 0 requests. Rules are read LIVE from sandbox.ts so this gate
# cannot go stale.
$sandboxScanner = Join-Path $RepoRoot 'scripts\scan-sandbox-dynamic.cjs'
$sandboxTs      = Join-Path $RepoRoot 'src\modules\extensions\runtime\sandbox.ts'
$extensionJs    = Join-Path $pluginRoot 'dist\index.js'

if (Test-Path $extensionJs) {
    if (-not (Test-Path $sandboxScanner)) {
        Write-Error "Sandbox scanner not found at $sandboxScanner"
        exit 1
    }
    if (-not (Test-Path $sandboxTs)) {
        Write-Error "sandbox.ts not found at $sandboxTs"
        exit 1
    }
    Write-Host "==> Validating extension against sandbox rules (dynamic)"
    # Pass sandbox.ts explicitly: the scanner default is CWD-relative, so
    # relying on it would FATAL whenever this runs from another directory.
    & node $sandboxScanner $extensionJs $sandboxTs
    if ($LASTEXITCODE -ne 0) {
        Write-Error @"
SANDBOX VALIDATION FAILED (exit $LASTEXITCODE).
The host will REJECT this extension and it will load SILENTLY-NEVER.
Patterns are read LIVE from sandbox.ts, so this is never stale.
"@
        exit 1
    }
    Write-Host "==> Extension sandbox validation PASSED"
}

$spikeExe     = Join-Path $pluginRoot 'sidecar\target\release\spike.exe'
$extensionBin = Join-Path $pluginRoot 'bin\focus-sync-sidecar.exe'
$installerBin = Join-Path $pluginRoot 'installer\focus-sync-sidecar.exe'

if (-not (Test-Path $spikeExe)) {
    Write-Error "spike.exe not found at $spikeExe.`nRun 'cd extensions/kizemo.focus-sync/sidecar && cargo build --release' first."
    exit 1
}

# Guard against a stale sidecar: Tauri SWALLOWS makensis warnings, so a
# missing or outdated payload ships as a broken build with a clean log.
# Compare mtimes rather than hashes -- a rebuild always rewrites the file.
$stale = @()
foreach ($pair in @(
    @{ Name = 'extension/bin  (bundler)'; Path = $extensionBin },
    @{ Name = 'installer/      (makensis)'; Path = $installerBin }
)) {
    if (-not (Test-Path $pair.Path)) { continue }
    if ((Get-Item $pair.Path).LastWriteTime -lt (Get-Item $spikeExe).LastWriteTime) {
        $stale += $pair.Name
    }
}
if ($stale.Count -gt 0) {
    Write-Error @"
STALE SIDECAR detected in: $($stale -join ', ')
They are OLDER than the freshly built spike.exe, so the installers would
bundle a previous build SILENTLY. Delete them and re-run this script.
"@
    exit 1
}

New-Item -ItemType Directory -Force -Path (Split-Path $extensionBin -Parent) | Out-Null
New-Item -ItemType Directory -Force -Path (Split-Path $installerBin -Parent) | Out-Null

Copy-Item -Path $spikeExe -Destination $extensionBin -Force
Write-Host "Copied spike.exe -> $extensionBin (for tauri bundle.resources)"

Copy-Item -Path $spikeExe -Destination $installerBin -Force
Write-Host "Copied spike.exe -> $installerBin (for makensis File bundling)"

Write-Host ""
Write-Host "Next step: cd $RepoRoot && npm run tauri build"