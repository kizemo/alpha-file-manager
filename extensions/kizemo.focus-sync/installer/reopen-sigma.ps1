# ============================================================================
# reopen-sigma.ps1 - Re-launch Alpha File Manager after install
# ============================================================================
# Called by installer.nsi Stage 8 if we killed AFM at Stage 1.
#
# The app was renamed from "Sigma File Manager" to "Alpha File Manager",
# so both the registry keys and the binary names are probed.
#
# Search order:
#   1. Uninstall registry keys (HKLM + WOW6432Node) - current Tauri NSIS
#      variant, the legacy makensis variant, and the pre-rename keys.
#   2. Common filesystem paths (D: and C:\Program Files).
#
# Exit codes:
#   0 = Alpha File Manager launched
#   1 = not found, user must launch manually
#
# Pure ASCII on purpose: PowerShell 5.1 reads a BOM-less .ps1 as ANSI.
# ============================================================================

$ErrorActionPreference = 'Continue'

$candidates = @()

# 1. Registry lookups
$regPaths = @(
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Alpha File Manager",
    "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Alpha File Manager",
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Alpha File Manager_is1",
    "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Alpha File Manager_is1",
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Sigma File Manager_is1",
    "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Sigma File Manager_is1",
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\com.sigma-file-manager.app_is1",
    "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\com.sigma-file-manager.app_is1"
)
foreach ($rp in $regPaths) {
    if (Test-Path $rp) {
        $props = Get-ItemProperty $rp -ErrorAction SilentlyContinue
        if ($props.InstallLocation) {
            foreach ($name in @("alpha-file-manager.exe", "Alpha File Manager.exe", "sigma-file-manager.exe", "Sigma File Manager.exe")) {
                $bin = Join-Path $props.InstallLocation $name
                if (Test-Path $bin) { $candidates += $bin }
            }
        }
    }
}

# 2. Filesystem fallbacks
$fallbacks = @(
    "D:\Program Files\Alpha File Manager\alpha-file-manager.exe",
    "D:\Program Files\Alpha File Manager\Alpha File Manager.exe",
    "C:\Program Files\Alpha File Manager\alpha-file-manager.exe",
    "C:\Program Files\Alpha File Manager\Alpha File Manager.exe",
    "D:\Program Files\Sigma File Manager\sigma-file-manager.exe",
    "D:\Program Files\Sigma File Manager\Sigma File Manager.exe",
    "C:\Program Files\Sigma File Manager\sigma-file-manager.exe",
    "C:\Program Files\Sigma File Manager\Sigma File Manager.exe"
)
foreach ($f in $fallbacks) {
    if (Test-Path $f) { $candidates += $f }
}

# 3. Launch the first match
foreach ($bin in $candidates) {
    try {
        Start-Process $bin -ErrorAction Stop
        Write-Host "==> Re-opened Alpha File Manager: $bin"
        exit 0
    } catch {
        Write-Host "==> Failed to start $bin : $_"
    }
}

Write-Host "==> WARNING: Could not find Alpha File Manager binary in registry or fallback paths."
Write-Host "    User must launch Alpha File Manager manually."
exit 1
