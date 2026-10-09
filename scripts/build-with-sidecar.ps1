# build-with-sidecar.ps1
#
# v0.3.0 (focus-sync Scheduled Task migration)
#
# Synchronizes the freshly built spike.exe from sigma-listary-spike/target/release/
# to the release/ folder so both installers can pick it up:
#   - Installer A (Sigma FM bundled): bundles it via tauri.conf.json bundle.resources
#     (target: tools/focus-sync-sidecar.exe → unpacked to $INSTDIR\tools\)
#   - Installer B (standalone NSIS): bundles it via File directive
#     (target: $INSTDIR\bin\focus-sync-sidecar.exe)
#
# Run after `cd sigma-listary-spike && cargo build --release`.
# Then run Sigma FM's build: `cd sigma-file-manager && npm run tauri build`.

[CmdletBinding()]
param(
    [string]$RepoRoot
)
if (-not $RepoRoot) {
    $RepoRoot = if ($env:FILEMANAGER_REPO) { $env:FILEMANAGER_REPO } else { "F:\soft\00selfmade\filemanager" }
}

$ErrorActionPreference = 'Stop'

# ---- Sandbox gate (focus-21 Phase 4) ----
# Sigma FM's validateExtensionCode() rejects the WHOLE extension if any sandbox
# regex matches — including matches inside COMMENTS. On rejection loader.ts
# throws BEFORE `new Worker()`, so the extension never loads and fails
# SILENTLY: no UI error, no notification, sidecar receives 0 requests.
# Rules are read LIVE from sandbox.ts (Rule 79), so this gate never goes stale.
$sandboxScanner = Join-Path $RepoRoot "scripts\scan-sandbox-dynamic.cjs"
$sandboxTs      = Join-Path $RepoRoot "sigma-file-manager\src\modules\extensions\runtime\sandbox.ts"
$extensionJs    = Join-Path $RepoRoot "release\extension\dist\index.js"
if (Test-Path $extensionJs) {
    if (-not (Test-Path $sandboxScanner)) {
        Write-Error "Sandbox scanner not found at $sandboxScanner"
        exit 1
    }
    Write-Host "==> Validating extension against Sigma FM sandbox (dynamic)"
    # Pass sandbox.ts explicitly: the scanner's default is CWD-relative, so
    # relying on it would FATAL (exit 2) whenever this script runs from
    # another directory.
    & node $sandboxScanner $extensionJs $sandboxTs
    if ($LASTEXITCODE -ne 0) {
        Write-Error @"
SANDBOX VALIDATION FAILED (exit $LASTEXITCODE).
Sigma FM validateExtensionCode will REJECT this extension and it will
load SILENTLY-NEVER — no UI error, no notification.
Patterns are read LIVE from sandbox.ts, so this is never stale.
"@
        exit 1
    }
    Write-Host "==> Extension sandbox validation PASSED"
}

$spikeExe = Join-Path $RepoRoot "sigma-listary-spike\target\release\spike.exe"
$extensionBin = Join-Path $RepoRoot "release\extension\bin\focus-sync-sidecar.exe"
$installerBin = Join-Path $RepoRoot "release\extension-installer\focus-sync-sidecar.exe"

if (-not (Test-Path $spikeExe)) {
    Write-Error "spike.exe not found at $spikeExe.`nRun 'cd sigma-listary-spike && cargo build --release' first."
    exit 1
}

# Ensure target dirs exist
$extensionBinDir = Split-Path $extensionBin -Parent
$installerBinDir = Split-Path $installerBin -Parent
New-Item -ItemType Directory -Force -Path $extensionBinDir | Out-Null
New-Item -ItemType Directory -Force -Path $installerBinDir | Out-Null

# Copy to extension/bin/ for Installer A bundling
Copy-Item -Path $spikeExe -Destination $extensionBin -Force
Write-Host "Copied spike.exe → $extensionBin (for Tauri bundle.resources)"

# v4 review V4.3: also copy to extension-installer/ for Installer B's `File`
# bundling. Without this, makensis fails to find the binary relative to its cwd.
Copy-Item -Path $spikeExe -Destination $installerBin -Force
Write-Host "Copied spike.exe → $installerBin (for makensis File bundling)"

Write-Host ""
Write-Host "Next step: cd $RepoRoot\sigma-file-manager && npm run tauri build"
