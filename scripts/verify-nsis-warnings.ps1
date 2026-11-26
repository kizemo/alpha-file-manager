# ============================================================================
# verify-nsis-warnings.ps1 -- gate for the Sigma FM integrated installer (B)
# ============================================================================
# WHY THIS EXISTS
#   The Tauri bundler runs makensis but SWALLOWS its warnings. A full build
#   log can be completely clean while the generated installer is broken.
#   On 2026-10-08 two real defects shipped that way, both invisible in the
#   build log and both silent at runtime:
#     1. "$_.Path"  -> NSIS ate "$_", turning the filter into
#                      "Where-Object { .Path -like ... }" (syntax error).
#                      The stage never ran; the old sidecar survived.
#     2. "$env:USERPROFILE\AppData\Roaming" -> NSIS treats the WHOLE thing as
#                      one variable name (':' and '\' are legal in NSIS
#                      variable names), so the fallback resolved %APPDATA% to
#                      the empty string "\AppData\Roaming".
#                      Installed, registered nowhere => "no functionality".
#
#   Both are fixed in src-tauri/installer/hooks.nsh. This script is the
#   regression gate: it runs makensis directly (bypassing Tauri) and fails if
#   ANY warning appears.
#
# NOTE: ASCII-only on purpose (hard constraint 9 -- PS 5.1 reads .ps1 as ANSI).
#
# Usage (run from the repo root):
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-nsis-warnings.ps1
# ============================================================================

[CmdletBinding()]
param(
    [string]$RepoRoot = ""
)

$ErrorActionPreference = 'Stop'

# 2026-10-09: this gate now runs from inside the fork itself. Default to the
# repo containing this script; the old default pointed at a sibling checkout
# and silently FAILed with a confusing "hooks.nsh not found".
if (-not $RepoRoot) { $RepoRoot = Split-Path $PSScriptRoot -Parent }

$makensis = 'C:\Program Files (x86)\NSIS\makensis.exe'
$nsisDir  = Join-Path $RepoRoot 'src-tauri\target\release\nsis\x64'
$nsi      = Join-Path $nsisDir 'installer.nsi'
$hooks    = Join-Path $RepoRoot 'src-tauri\installer\hooks.nsh'
$probe    = Join-Path $env:TEMP 'nsis-warning-probe.exe'

function Fail([string]$m) { Write-Host "RESULT: FAIL -- $m"; exit 1 }

if (-not (Test-Path $makensis)) { Fail "makensis not found at $makensis" }
if (-not (Test-Path $hooks))    { Fail "hooks.nsh not found at $hooks" }
if (-not (Test-Path $nsi))      { Fail "generated installer.nsi not found at $nsi -- run 'npm run tauri build' first" }

# Guard against the stale-artifact trap: if installer.nsi is older than
# hooks.nsh, we would be checking an old include and passing trivially.
$hAge = (Get-Item $hooks).LastWriteTime
$nAge = (Get-Item $nsi).LastWriteTime
if ($nAge -lt $hAge) {
    Fail ("generated installer.nsi ($($nAge)) is OLDER than hooks.nsh ($hAge). " +
          "A warning check now would be meaningless -- rebuild first.")
}

Push-Location $nsisDir
try {
    $out = & $makensis /V3 $nsi "/OUT:$probe" 2>&1
} finally {
    Pop-Location
}

$exit = $LASTEXITCODE
if ($exit -ne 0) { Fail "makensis exited $exit" }

$warn = @($out | Select-String -Pattern 'warning\s+\d+')
if ($warn.Count -gt 0) {
    Write-Host "makensis emitted $($warn.Count) warning(s):"
    $warn | Select-Object -First 20 | ForEach-Object { Write-Host ("  " + $_.Line.Trim()) }
    Write-Host ""
    Write-Host "Each NSIS 'unknown variable/constant' warning is a value that was"
    Write-Host "silently swallowed at compile time -- the command ships broken."
    Fail "$($warn.Count) NSIS warning(s)"
}

Write-Host ("=" * 76)
Write-Host "NSIS WARNING GATE: PASS -- 0 warnings (hooks.nsh compiles clean)"
Write-Host ("=" * 76)
exit 0