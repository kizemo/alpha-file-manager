# ============================================================================
# verify-deploy-sha.ps1 -- ACCEPTANCE GATE for the packaging task
# ============================================================================
# Implements handoff-2026-10-08-packaging.md section 1:
#   "the acceptance criterion is NOT 'install succeeded', it is
#    'the installed files are byte-identical to the manual deployment'."
#
# This script IS that criterion, executable. Exit 0 = PASS, exit 1 = FAIL.
#
# Locations checked are exactly the ones listed in handoff section 1.
# Nothing is invented -- a location that does not exist yet is reported
# MISSING and fails the gate on purpose (that is the whole point: the
# historical failure shipped a stale payload and "installed fine").
#
# NOTE: ASCII-only on purpose. PowerShell 5.1 reads .ps1 as ANSI when there
# is no BOM, so a UTF-8 non-ASCII comment can corrupt quoting (this project
# was burned by exactly that -- see hard constraint 9).
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-deploy-sha.ps1
#   powershell ... -File scripts\verify-deploy-sha.ps1 -Json
# ============================================================================

[CmdletBinding()]
param(
    [string]$RepoRoot = "",
    [switch]$Json
)

$ErrorActionPreference = 'Stop'

# 2026-10-09: the plugin moved into THIS repo. Default to the repo that
# contains this script instead of a hardcoded absolute path, so the gate
# works from any clone location and never silently points at a stale sibling.
if (-not $RepoRoot) { $RepoRoot = Split-Path $PSScriptRoot -Parent }

$pluginRoot = Join-Path $RepoRoot 'extensions\kizemo.focus-sync'

$extDir    = Join-Path $env:APPDATA 'com.sigma-file-manager.app\extensions\kizemo.focus-sync'
$sidecarExe = 'focus-sync-sidecar.exe'

# Which sidecar is actually installed? Ask the Scheduled Task -- it is the one
# Windows launches at logon, so it is authoritative and machine-independent.
#
# 2026-10-08: this used to hardcode the installer-A path
# (%APPDATA%\...\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\).
# Installer B (the integrated Sigma FM build) ships it to $INSTDIR\tools\
# instead, so the gate FAILED after a perfectly correct B install.
# Two installers, two layouts -- the gate must follow reality, not assume.
$taskSidecar = $null
try {
    $t = Get-ScheduledTask -TaskName 'KizemoFocusSync' -ErrorAction Stop
    $taskSidecar = $t.Actions[0].Execute
} catch { }

$sidecarPaths = @(
    (Join-Path $pluginRoot "sidecar\target\release\spike.exe"),
    (Join-Path $pluginRoot "bin\$sidecarExe"),
    (Join-Path $pluginRoot "installer\$sidecarExe")
)
if ($taskSidecar) { $sidecarPaths += $taskSidecar }
# Optional: installer-A canonical location. Checked only when present.
# Dedupe: after unification both resolve to the same file.
$aPath = Join-Path $extDir "bin\focus-sync-sidecar\$sidecarExe"
if ((Test-Path -LiteralPath $aPath) -and ($sidecarPaths -notcontains $aPath)) { $sidecarPaths += $aPath }

# Baseline (source of truth) first in each group; every other path in the
# group must be byte-identical to it.
$groups = @(
    [pscustomobject]@{
        Label = 'index.js'
        Paths = @(
            (Join-Path $RepoRoot 'release\extension\dist\index.js'),
            (Join-Path $extDir 'dist\index.js')
        )
    },
    [pscustomobject]@{
        Label = 'sidecar.exe'
        Paths = $sidecarPaths
    }
)

function Get-Sha([string]$p) {
    if (-not (Test-Path -LiteralPath $p)) { return $null }
    return (Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash
}

$results  = @()
$failures = 0

foreach ($g in $groups) {
    $rows = @()
    foreach ($p in $g.Paths) {
        $h = Get-Sha $p
        $rows += [pscustomobject]@{
            Exists = ($null -ne $h)
            Sha    = $h
            Path   = $p
        }
    }
    $baseline = $null
    foreach ($r in $rows) { if ($r.Exists) { $baseline = $r.Sha; break } }

    # The first path in each group is the repo source of truth. If it is
    # missing, "every existing file matches" degenerates into comparing the
    # installed file against ITSELF -- a vacuous PASS that proves nothing.
    # A missing baseline must be a hard failure.
    if ($null -eq $baseline) {
        $failures++
        Write-Host ("[{0}] ----NO-BASELINE---- {1,-13} {2}" -f 'FAIL', $g.Label, $rows[0].Path)
        continue
    }

    foreach ($r in $rows) {
        $ok = ($null -ne $baseline) -and $r.Exists -and ($r.Sha -eq $baseline)
        if (-not $ok) { $failures++ }
        $results += [pscustomobject]@{
            Group    = $g.Label
            Path     = $r.Path
            Exists   = $r.Exists
            Sha      = $r.Sha
            Expected = $baseline
            Ok       = $ok
        }
    }
}

if ($Json) {
    $results | ConvertTo-Json -Depth 4
} else {
    Write-Output ("=" * 104)
    Write-Output ("DEPLOY SHA VERIFICATION  --  baseline = first existing path in each group")
    Write-Output ("=" * 104)
    foreach ($r in $results) {
        $flag = if ($r.Ok) { "OK  " } else { "FAIL" }
        $sha  = if ($r.Exists) { $r.Sha.Substring(0, 16) } else { "----MISSING--------" }
        Write-Output ("[{0}] {1}  {2,-13} {3}" -f $flag, $sha, $r.Group, $r.Path)
    }
    Write-Output ("-" * 104)
    Write-Output ("  sidecar actually launched by task KizemoFocusSync:")
    Write-Output ("    {0}" -f $(if ($taskSidecar) { $taskSidecar } else { "<TASK MISSING - cannot verify>" }))
    if ($failures -eq 0) {
        Write-Output "RESULT: PASS  -- every present file is byte-identical to the manual deployment"
    } else {
        Write-Output "RESULT: FAIL  -- $failures path(s) mismatched or missing"
    }
}

# Independent of the path comparison: a sidecar MUST actually be installed.
# Otherwise "all existing files match" could pass on a fresh machine that has
# nothing installed at all -- the PASS would be trivial (see the rule about
# vacuous passes).
if (-not $taskSidecar) {
    Write-Output "RESULT: FAIL  -- Scheduled Task 'KizemoFocusSync' not found; nothing to verify"
    exit 1
}

if ($failures -gt 0) { exit 1 }
exit 0