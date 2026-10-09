# ============================================================================
# resolve_user_appdata.ps1 - Resolve the ORIGINAL USER'S AppData\Roaming path
# ============================================================================
# Used by NSIS installer hooks.nsh POSTINSTALL to recover from a perMachine
# installer's admin-context %APPDATA% resolution bug.
#
# When a non-admin user double-clicks a perMachine installer and clicks "Yes"
# on the UAC prompt, the installer runs as admin (or SYSTEM), and the process's
# $env:APPDATA resolves to the ADMIN's profile, NOT the user's. Any file we
# write to "$APPDATA\..." lands in admin's profile. Sigma FM (running as the
# original user) then reads its own empty %APPDATA% and shows "No extensions".
#
# This script resolves the active console session user's profile path
# regardless of which token we're holding, then writes:
#   USER_APPDATA=<full path>\AppData\Roaming
# to the file specified by -OutFile.
#
# Strategy (with fallbacks):
#   1. Win32_LoggedOnUser (LogonType=2 = interactive console session) - preferred
#   2. explorer.exe owner via Win32_Process::GetOwner
#   3. Fallback to $env:APPDATA (assumes installer context == user context)
# ============================================================================

param(
    [Parameter(Mandatory=$true)]
    [string]$OutFile
)

$ErrorActionPreference = 'Continue'

function Write-Result {
    param(
        [string]$AppData,
        [string]$Sid,
        [string]$Method,
        [string]$OutFile
    )
    $line = "USER_APPDATA=$AppData"
    Set-Content -Path $OutFile -Value $line -Encoding ASCII -Force
    Write-Host "==> [resolve] method=$Method sid=$Sid"
    Write-Host "==> [resolve] $line"
    Write-Host "==> [resolve] wrote to $OutFile"
}

function Get-ProfileBySid {
    param([string]$Sid)
    $keyPath = "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\$Sid"
    if (-not (Test-Path $keyPath)) {
        # WOW6432Node fallback (32-bit registry view on 64-bit OS, sometimes needed)
        $keyPath = "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows NT\CurrentVersion\ProfileList\$Sid"
    }
    if (Test-Path $keyPath) {
        return (Get-ItemProperty $keyPath -ErrorAction Stop).ProfileImagePath
    }
    return $null
}

# ---- Strategy 1: Win32_LoggedOnUser ----
try {
    $loggedOn = Get-CimInstance -ClassName Win32_LoggedOnUser -Filter "LogonType=2" -ErrorAction Stop | Select-Object -First 1
    if ($loggedOn -and $loggedOn.Antecedent) {
        # Antecedent format: Win32_Account.Domain="X",Name="Y",Sid="S-1-..."
        if ($loggedOn.Antecedent -match 'Sid="(S-1-5-[0-9-]+)"') {
            $sid = $matches[1]
            $profile = Get-ProfileBySid -Sid $sid
            if ($profile) {
                $appData = Join-Path $profile 'AppData\Roaming'
                Write-Result -AppData $appData -Sid $sid -Method 'Win32_LoggedOnUser' -OutFile $OutFile
                exit 0
            } else {
                Write-Warning "[resolve] Strategy 1: SID $sid found, but no ProfileImagePath in registry"
            }
        }
    } else {
        Write-Warning "[resolve] Strategy 1: no Win32_LoggedOnUser with LogonType=2"
    }
} catch {
    Write-Warning "[resolve] Strategy 1 failed: $($_.Exception.Message)"
}

# ---- Strategy 2: explorer.exe owner via GetOwner ----
try {
    $explorer = Get-CimInstance -ClassName Win32_Process -Filter "Name='explorer.exe'" -ErrorAction Stop | Select-Object -First 1
    if ($explorer) {
        $owner = Invoke-CimMethod -InputObject $explorer -MethodName GetOwner -ErrorAction Stop
        if ($owner -and $owner.User) {
            $ntAccount = New-Object System.Security.Principal.NTAccount($owner.Domain, $owner.User)
            $sid = $ntAccount.Translate([System.Security.Principal.SecurityIdentifier]).Value
            $profile = Get-ProfileBySid -Sid $sid
            if ($profile) {
                $appData = Join-Path $profile 'AppData\Roaming'
                Write-Result -AppData $appData -Sid $sid -Method 'explorer-GetOwner' -OutFile $OutFile
                exit 0
            } else {
                Write-Warning "[resolve] Strategy 2: SID $sid resolved, but no ProfileImagePath"
            }
        }
    } else {
        Write-Warning "[resolve] Strategy 2: no explorer.exe process found"
    }
} catch {
    Write-Warning "[resolve] Strategy 2 failed: $($_.Exception.Message)"
}

# ---- Strategy 3: Fallback to current $env:APPDATA ----
Write-Warning "[resolve] All strategies failed; falling back to current env:APPDATA"
$fallbackAppData = $env:APPDATA
$fallbackSid = 'fallback'
# Try to detect current user SID via whoami for the log
try {
    $whoamiOut = whoami /user 2>&1
    if ($whoamiOut -match 'S-1-5-[0-9-]+') {
        $fallbackSid = $matches[0]
    }
} catch {}
Write-Result -AppData $fallbackAppData -Sid $fallbackSid -Method 'fallback-env' -OutFile $OutFile
exit 0