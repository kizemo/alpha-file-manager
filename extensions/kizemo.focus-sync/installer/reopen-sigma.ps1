# ============================================================================
# reopen-sigma.ps1 — Re-launch Sigma File Manager after install
# ============================================================================
# Called by installer.nsi Stage 6 if we killed Sigma FM at Stage 1.
#
# Search order:
#   1. Uninstall registry keys (HKLM + WOW6432Node) — both legacy makensis
#      and current Tauri NSIS variants
#   2. Common filesystem paths (D: sigma & C:\Program Files, both binary
#      names: sigma-file-manager.exe and "Sigma File Manager.exe")
#
# Exit codes:
#   0 = Sigma FM launched
#   1 = not found, user must launch manually
# ============================================================================

$ErrorActionPreference = 'Continue'

$candidates = @()

# 1. Registry lookups
$regPaths = @(
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Sigma File Manager_is1",
    "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Sigma File Manager_is1",
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\com.sigma-file-manager.app_is1",
    "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\com.sigma-file-manager.app_is1"
)
foreach ($rp in $regPaths) {
    if (Test-Path $rp) {
        $props = Get-ItemProperty $rp -ErrorAction SilentlyContinue
        if ($props.InstallLocation) {
            foreach ($name in @("sigma-file-manager.exe", "Sigma File Manager.exe")) {
                $bin = Join-Path $props.InstallLocation $name
                if (Test-Path $bin) { $candidates += $bin }
            }
        }
    }
}

# 2. Filesystem fallbacks
$fallbacks = @(
    "D:\Program Files\Sigma File Manager\sigma-file-manager.exe",
    "D:\Program Files\Sigma File Manager\Sigma File Manager.exe",
    "C:\Program Files\Sigma File Manager\sigma-file-manager.exe",
    "C:\Program Files\Sigma File Manager\Sigma File Manager.exe",
    "C:\Program Files\Sigma File Manager\Sigma File Manager\Sigma File Manager.exe",
    "D:\Program Files\Sigma File Manager\Sigma File Manager\Sigma File Manager.exe"
)
foreach ($f in $fallbacks) {
    if (Test-Path $f) { $candidates += $f }
}

# 3. Launch the first match
foreach ($bin in $candidates) {
    try {
        Start-Process $bin -ErrorAction Stop
        Write-Host "==> Re-opened Sigma FM: $bin"
        exit 0
    } catch {
        Write-Host "==> Failed to start $bin : $_"
    }
}

Write-Host "==> WARNING: Could not find Sigma FM binary in registry or fallback paths."
Write-Host "    User must launch Sigma FM manually."
exit 1