# ============================================================================
# manual-install.ps1 - Manual install (Sigma FM must be closed first)
# ============================================================================
# This script replicates the manually-verified install workflow that has
# been proven to work as of round-19c / 2026-09-30. Steps:
#   1. Detect if Sigma FM is running - ABORT if yes (per ROOT-CAUSE-2026-09-30).
#   2. Copy extension files to %APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\
#      - 3a: package.json, dist/index.js, locales\*.json
#      - 3b: bin\focus-sync-sidecar\focus-sync-sidecar.exe (round 19c)
#   3. Run register.ps1 to add kizemo.focus-sync to user-extensions.json.
#      Round-2 fix: if entry already exists, leave it alone (preserves Sigma FM's
#      normalized state - see register.ps1 comments).
#   4. (Optional) Re-launch Sigma FM from registry-detected install path.
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File manual-install.ps1
# ============================================================================

$ErrorActionPreference = 'Stop'

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
# The plugin source was merged into this repo, so the extension
# source IS the extension folder itself: package.json / dist / locales / bin
# all live directly under kizemo.focus-sync\ (the old '..\extension'
# sub-folder no longer exists, so every copy below silently found nothing).
$srcDir    = Join-Path $ScriptDir '..'
$appDataDir = Join-Path $env:APPDATA 'com.sigma-file-manager.app'
$extDir    = Join-Path $appDataDir 'extensions\kizemo.focus-sync'
$binDir    = Join-Path $extDir 'bin\focus-sync-sidecar'
$registerScript = Join-Path $ScriptDir 'register.ps1'

# ---- 1. Detect running Sigma FM ----
Write-Host "==> Step 1: Checking if Sigma FM is running..."
# The app process is alpha-file-manager.exe (brand: Alpha File Manager).
# This used to probe 'sigma-file-manager', so the "app is running -> abort"
# guard never fired and the script kept going while the app was live.
$procs = Get-Process -Name alpha-file-manager -ErrorAction SilentlyContinue
if ($procs) {
    Write-Error "Sigma FM is currently running (PIDs: $($procs.Id -join ', ')). Please right-click the taskbar icon and Quit, then re-run this script."
    exit 1
}
Write-Host "    Sigma FM is not running."

# ---- 2. Copy extension files ----
Write-Host ""
Write-Host "==> Step 2: Copying extension files..."
Write-Host "    Source:      $srcDir"
Write-Host "    Destination: $extDir"

# ---- Sandbox gate (focus-21 Phase 4) ----
# MUST run before any copy: once index.js lands in %APPDATA%, Sigma FM will
# try to load it. If any sandbox regex matches (even inside a comment),
# validateExtensionCode() rejects it and loader.ts throws BEFORE new Worker(),
# so the extension silently never loads - no UI error, no notification, and
# the sidecar receives 0 requests. Rules are read LIVE from sandbox.ts.
$srcIndexJs     = Join-Path $srcDir 'dist\index.js'
$sandboxScanner = Join-Path $ScriptDir '..\..\..\scripts\scan-sandbox-dynamic.cjs'
$sandboxTs      = Join-Path $ScriptDir '..\..\..\src\modules\extensions\runtime\sandbox.ts'
Write-Host "==> Validating extension against Sigma FM sandbox (dynamic)"
if (-not (Test-Path $sandboxScanner)) {
    Write-Error "Sandbox scanner not found at $sandboxScanner"
    exit 1
}
if (-not (Test-Path $sandboxTs)) {
    Write-Error "Sandbox rules not found at $sandboxTs"
    exit 1
}
# Pass sandbox.ts explicitly: the scanner's default is CWD-relative, so
# relying on it would FATAL (exit 2) whenever this script runs from
# another directory.
& node $sandboxScanner $srcIndexJs $sandboxTs
if ($LASTEXITCODE -ne 0) {
    Write-Error @"
SANDBOX VALIDATION FAILED (exit $LASTEXITCODE).
Refusing to deploy an extension Sigma FM will reject. It would load
SILENTLY-NEVER - no UI error, no notification, sidecar gets 0 requests.
"@
    exit 1
}
Write-Host "    Extension sandbox validation PASSED"

# Top-level files
New-Item -ItemType Directory -Path $extDir -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $extDir 'dist') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $extDir 'locales') -Force | Out-Null
New-Item -ItemType Directory -Path $binDir -Force | Out-Null

Copy-Item -Path (Join-Path $srcDir 'package.json') -Destination $extDir -Force
Copy-Item -Path (Join-Path $srcDir 'dist\index.js') -Destination (Join-Path $extDir 'dist') -Force
Copy-Item -Path (Join-Path $srcDir 'dist\index.js.map') -Destination (Join-Path $extDir 'dist') -Force
Copy-Item -Path (Join-Path $srcDir 'locales\en.json') -Destination (Join-Path $extDir 'locales') -Force
Copy-Item -Path (Join-Path $srcDir 'locales\zh-CN.json') -Destination (Join-Path $extDir 'locales') -Force
Copy-Item -Path (Join-Path $srcDir 'bin\focus-sync-sidecar.exe') -Destination $binDir -Force

# Verify the deployed sidecar is byte-identical to the repo build output.
# (Was: a hardcoded round-19c hash. The sidecar has been rebuilt many times
#  since, so that comparison had been failing on every single run and printing
#  a warning everyone learned to ignore -- a check that cannot pass is worse
#  than no check. The real invariant is "copy == source", which is what the
#  packaging acceptance gate (scripts\verify-deploy-sha.ps1) asserts too.)
$srcSidecar = Join-Path $srcDir 'bin\focus-sync-sidecar.exe'
$dstSidecar = Join-Path $binDir 'focus-sync-sidecar.exe'
$srcHash = (Get-FileHash -Path $srcSidecar -Algorithm SHA256).Hash
$dstHash = (Get-FileHash -Path $dstSidecar -Algorithm SHA256).Hash
if ($srcHash -ne $dstHash) {
    Write-Error "Sidecar copy mismatch! src=$srcHash dst=$dstHash"
    exit 1
}
Write-Host "    Sidecar sha verified against repo build: $($srcHash.Substring(0,16))..."

# ---- 3. Register in user-extensions.json ----
Write-Host ""
Write-Host "==> Step 3: Registering extension in user-extensions.json..."
if (-not (Test-Path $registerScript)) {
    Write-Error "register.ps1 not found at $registerScript"
    exit 1
}
& $registerScript
if ($LASTEXITCODE -ne 0) {
    Write-Error "register.ps1 failed with exit code $LASTEXITCODE"
    exit 1
}

# ---- 4. (Optional) Re-launch Sigma FM ----
Write-Host ""
Write-Host "==> Step 4: Re-launching Sigma File Manager..."

# Try registry paths first (Tauri fork + legacy makensis)
$binaries = @()
    $regPaths = @(
    "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Alpha File Manager",
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Alpha File Manager",
    "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Alpha File Manager_is1",
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Alpha File Manager_is1"
)
foreach ($rp in $regPaths) {
    if (Test-Path $rp) {
        $props = Get-ItemProperty $rp -ErrorAction SilentlyContinue
        if ($props.InstallLocation) {
            foreach ($name in @('alpha-file-manager.exe', 'Alpha File Manager.exe')) {
                $bin = Join-Path $props.InstallLocation $name
                if (Test-Path $bin) { $binaries += $bin }
            }
        }
    }
}
# Fallback paths
$fallbacks = @(
    "D:\Program Files\Alpha File Manager\alpha-file-manager.exe",
    "D:\Program Files\Alpha File Manager\Alpha File Manager.exe",
    "C:\Program Files\Alpha File Manager\alpha-file-manager.exe",
    "C:\Program Files\Alpha File Manager\Alpha File Manager.exe"
)
foreach ($f in $fallbacks) {
    if (Test-Path $f) { $binaries += $f }
}

$launched = $false
foreach ($bin in $binaries) {
    try {
        Start-Process $bin -ErrorAction Stop
        Write-Host "    Started: $bin"
        $launched = $true
        break
    } catch {
        Write-Host "    Failed to start $bin"
    }
}
if (-not $launched) {
    Write-Warning "Could not auto-launch Sigma FM. Please launch manually."
}

Write-Host ""
Write-Host "==> Done. Alpha File Manager should now have Focus Sync in the Installed tab."
Write-Host "    If not, click the Alpha File Manager menu -> Ctrl+Shift+X -> Installed tab -> Focus Sync -> Toggle on."
exit 0