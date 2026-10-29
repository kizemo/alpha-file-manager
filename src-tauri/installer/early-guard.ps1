# early-guard.ps1
#
# Decides whether the Alpha File Manager installer should show its wizard at
# all. The installer calls it from .onInit, i.e. BEFORE any page is created,
# so a "no" answer produces zero windows.
#
# WHY THIS EXISTS (2026-10-29 real-machine report)
# ------------------------------------------------
# The Alpha File Manager setup.exe is shipped to users as a downloadable
# file. Chat / agent UIs render such an attachment with a preview component
# that auto-opens it. An .exe has no in-app preview, so the host falls back
# to "open with the system handler", which LAUNCHES the installer. Every
# time the conversation view re-renders, the installer is launched again and
# the NSIS wizard pops up over whatever the user is doing.
#
# The installer cannot change the host application, but it can make a
# redundant launch harmless: when the version carried by this package is not
# newer than the version already installed on this machine, there is nothing
# to do -- so exit silently, before any window exists.
#
# THE RULES
# ---------
#   not installed at all         -> proceed  (normal first install)
#   installer version NEWER      -> proceed  (normal upgrade)
#   installer version SAME       -> quit     (a reinstall changes nothing)
#   installer version OLDER      -> quit     (would be a downgrade; /FORCE
#                                           allows a deliberate one)
#   version cannot be determined  -> proceed  (fail open: a broken guard must
#                                           never make the product
#                                           uninstallable)
#
# /FORCE
# ------
# "setup.exe /FORCE" skips the version rule and always shows the wizard.
# That is the escape hatch for a deliberate reinstall or downgrade.
#
# The flag is detected HERE, not in NSIS. The installer hands over the raw
# command line (see -CommandLineFile) and this script looks for the /FORCE
# token with a regular expression, which is both reliable and unit tested.
# NSIS' own StrCmp wildcard matching was measured to be non functional on
# this machine (see the probe referenced in hooks.nsh), and the installer
# process cannot see its arguments in $1..$9 either.
#
# EXIT CODES
# ----------
#   0  -> proceed with the wizard
#   10 -> quit silently
#   2  -> the check itself failed; the caller treats that as "proceed"
#
# ASCII ONLY, ON PURPOSE. Same reason as check-single-instance.ps1: the file
# is materialised verbatim into $PLUGINSDIR and Windows PowerShell 5.1 reads
# a BOM-less .ps1 using the system ANSI code page.

[CmdletBinding()]
param(
  # Version of the package that is running right now.
  [Parameter(Mandatory = $true)]
  [string]$InstallerVersion,

  # Sub-key under Software\Microsoft\Windows\CurrentVersion\Uninstall that
  # holds the installed product record (DisplayName / DisplayVersion).
  [string]$UninstKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\Alpha File Manager',

  # Where to drop the "do not show the wizard" marker file. The installer
  # passes a path under $PLUGINSDIR; tests pass a scratch path.
  [string]$VerdictFile = '',

  # Test-only override for the log destination.
  [string]$LogFileOverride = '',

  # 1 when the caller was invoked with /FORCE.
  [int]$Force = 0,

  # Path to a file that holds the command line the installer was started
  # with. The installer writes that file before calling this script; the
  # script itself decides whether the command line asks for /FORCE.
  #
  # Why the command line is handed over as a file instead of being parsed
  # in NSIS: NSIS' StrCmp does not match the "* /FORCE" wildcard on this
  # machine (measured 2026-10-29, minimal repro probe-force.nsi), and the
  # installer process cannot see the arguments in $1..$9 either. PowerShell
  # has a regular expression, and a regular expression can be unit tested.
  [string]$CommandLineFile = ''
)

$ErrorActionPreference = 'Continue'

$ScriptLogFile = if ($LogFileOverride) { $LogFileOverride } else {
  Join-Path $env:TEMP 'alpha-file-manager-early-guard.log'
}

$ScriptDecisionFile = if ($LogFileOverride) { "$LogFileOverride.last" } else {
  Join-Path $env:TEMP 'alpha-file-manager-early-guard.last.txt'
}

function Write-Log {
  param([string]$Message)

  # nsExec::ExecToLog pipes stdout into the installer's DetailPrint log, so
  # the same line shows up in the install log and in the durable file.
  Write-Output $Message

  try {
    Add-Content -LiteralPath $ScriptLogFile -Encoding ASCII `
      -Value ("{0} {1}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'), $Message) `
      -ErrorAction Stop
  }
  catch {
    # Never let logging itself break the install.
  }
}

function Get-RegistryCandidates {
  param([string]$SubKey)

  # Drop the leading "Software\" so the remainder can be joined to a hive
  # root. A regex replace is used on purpose: "Software\" is 9 characters,
  # and the Win32 registry API does NOT tolerate a doubled separator
  # ("SOFTWARE\\Microsoft\..." simply is not a key).
  $rel = $SubKey -replace '^Software\\', ''

  # A 32 bit installer writes into the WOW6432Node view, a 64 bit one into
  # the plain view. Check both views and both hives so the guard behaves the
  # same no matter which bitness built the package.
  return @(
    ('HKLM:\' + $SubKey),
    ('HKLM:\Software\WOW6432Node\' + $rel),
    ('HKCU:\' + $SubKey),
    ('HKCU:\Software\WOW6432Node\' + $rel)
  )
}

function Read-InstalledVersion {
  param([string]$SubKey)

  # Drop the leading "Software\" so the remainder can be joined to a hive
  # root. A regex replace is used on purpose: "Software\" is 9 characters,
  # and the Win32 registry API does NOT tolerate a doubled separator
  # ("SOFTWARE\\Microsoft\..." simply is not a key).
  $rel = $SubKey -replace '^Software\\', ''

  # A 32 bit process sees the WOW6432Node view of HKLM\SOFTWARE, so a plain
  # "HKLM:\..." read returns NOTHING when the helper runs in a 32 bit
  # PowerShell - which is exactly what happens inside the installer: the NSIS
  # setup.exe is a 32 bit process, and WOW64 FILE SYSTEM redirection makes
  # "powershell" resolve to SysWOW64\powershell.exe.
  # Measured on the test machine (2026-10-29): the 32 bit shell reports
  # "Cannot find path 'HKLM:\Software\...\Uninstall\Alpha File Manager'",
  # while the 64 bit shell reads 3.6.6 from the very same key. The installer
  # writes the key through SetRegView 64, so only the 64 bit view holds it.
  #
  # The RegistryView API addresses a view EXPLICITLY, so the same code reads
  # the right key from either bitness. Both hives and both views are tried.
  $targets = @(
    @{ Hive = [Microsoft.Win32.RegistryHive]::LocalMachine; Root = ('SOFTWARE\' + $rel) },
    @{ Hive = [Microsoft.Win32.RegistryHive]::CurrentUser; Root = ('SOFTWARE\' + $rel) }
  )

  # NOTE ON THE API CHOICE (measured 2026-10-29 on the test machine):
  #   OpenBaseKey(hive, view)      -> returns 3.6.6 in BOTH the 32 bit and the
  #                                   64 bit shell. This is the only variant
  #                                   that works from inside the installer.
  #   OpenRemoteBaseKey(hive, "")  -> the base key itself is already
  #                                   redirected, so the view flag on the
  #                                   sub-key open cannot recover the 64 bit
  #                                   view, and the 3 argument OpenSubKey
  #                                   overload does not even exist in .NET
  #                                   Framework (the 32 bit shell reports
  #                                   "Cannot find an overload for OpenSubKey
  #                                   and the argument count: 3").
  foreach ($target in $targets) {
    foreach ($view in @([Microsoft.Win32.RegistryView]::Registry64, [Microsoft.Win32.RegistryView]::Registry32)) {
      try {
        $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey($target.Hive, $view)
        if ($null -eq $base) { continue }
        $key = $base.OpenSubKey($target.Root)
        if ($null -ne $key) {
          $value = $key.GetValue('DisplayVersion')
          $key.Close()
          $base.Close()
          if ($null -ne $value) {
            $text = ([string]$value).Trim()
            if ($text -ne '') { return $text }
          }
        }
        else {
          $base.Close()
        }
      }
      catch {
        # Try the next hive / view.
      }
    }
  }

  # Fallback for the (unlikely) case where the .NET view API is unavailable.
  # The plain provider read only works when the process happens to have the
  # bitness that matches the key, so it stays a fallback and not the main path.
  foreach ($candidate in (Get-RegistryCandidates -SubKey $SubKey)) {
    try {
      $item = Get-ItemProperty -LiteralPath $candidate -Name 'DisplayVersion' -ErrorAction Stop
      $value = [string]$item.DisplayVersion
      if ($value.Trim() -ne '') { return $value.Trim() }
    }
    catch {
      # Key or value missing -> try the next candidate.
    }
  }

  return ''
}

# Decides whether the run was asked to skip the version rule.
#
# Two sources:
#   - the -Force switch (used by the unit tests and by a direct caller)
#   - the command line the installer was started with (the real world case:
#     "setup.exe /FORCE" asks for the full wizard even when the installed
#     version is not older than the package)
#
# /FORCE has to be a whole word: "/XFORCE" or "FORCE" on its own are not a
# request to force, and neither is a path that merely contains "/FORCE".
function Test-ForceRequested {
  param([int]$Force, [string]$CommandLineFile)

  if ($Force -ne 0) { return $true }

  if (-not $CommandLineFile) { return $false }
  if (-not (Test-Path -LiteralPath $CommandLineFile)) { return $false }

  $text = ''

  try {
    $fs = [System.IO.File]::Open($CommandLineFile, 'Open', 'Read', 'ReadWrite')
    $sr = New-Object System.IO.StreamReader($fs)
    $text = $sr.ReadToEnd()
    $sr.Close()
    $fs.Close()
  }
  catch {
    return $false
  }

  if (-not $text) { return $false }

  # (?:^|[\s"'])/FORCE(?:[\s"']|$) - the token is delimited by whitespace or
  # a quote on both sides, so "setup.exe /FORCE", "setup.exe /FORCE /S" and
  # "setup.exe /force" all match while "/XFORCE" does not.
  if ($text -match '(?i)(?:^|[\s"''])/FORCE(?:[\s"'']|$)') { return $true }

  return $false
}

function ConvertTo-Semver {
  param([string]$Version)

  if (-not $Version) { return $null }

  $text = $Version.Trim()

  # Build metadata is ignored by semver.
  $plus = $text.IndexOf('+')
  if ($plus -ge 0) { $text = $text.Substring(0, $plus) }

  $pre = ''
  $dash = $text.IndexOf('-')
  if ($dash -ge 0) {
    $pre = $text.Substring($dash + 1)
    $text = $text.Substring(0, $dash)
  }

  if ($text -notmatch '^[0-9]+(\.[0-9]+)*$') { return $null }

  $parts = @($text -split '\.')
  $core = @(0, 0, 0)

  for ($i = 0; $i -lt $parts.Count; $i++) {
    if ($i -ge 3) { break }
    $n = 0
    if (-not [int]::TryParse($parts[$i], [ref]$n)) { return $null }
    $core[$i] = $n
  }

  return [pscustomobject]@{ Core = $core; Pre = $pre }
}

function Compare-Semver {
  param([string]$Left, [string]$Right)

  $l = ConvertTo-Semver -Version $Left
  if ($null -eq $l) { return $null }

  $r = ConvertTo-Semver -Version $Right
  if ($null -eq $r) { return $null }

  for ($i = 0; $i -lt 3; $i++) {
    if ($l.Core[$i] -lt $r.Core[$i]) { return -1 }
    if ($l.Core[$i] -gt $r.Core[$i]) { return 1 }
  }

  # Same numeric core: a release outranks a prerelease.
  if ($l.Pre -eq '' -and $r.Pre -eq '') { return 0 }
  if ($l.Pre -eq '') { return 1 }
  if ($r.Pre -eq '') { return -1 }

  $lp = @($l.Pre -split '\.')
  $rp = @($r.Pre -split '\.')
  $count = [Math]::Max($lp.Count, $rp.Count)

  for ($i = 0; $i -lt $count; $i++) {
    if ($i -ge $lp.Count) { return -1 }
    if ($i -ge $rp.Count) { return 1 }

    $a = $lp[$i]
    $b = $rp[$i]
    $aNum = $a -match '^[0-9]+$'
    $bNum = $b -match '^[0-9]+$'

    if ($aNum -and $bNum) {
      $av = [int]$a
      $bv = [int]$b
      if ($av -lt $bv) { return -1 }
      if ($av -gt $bv) { return 1 }
    }
    elseif ($aNum) { return -1 }
    elseif ($bNum) { return 1 }
    else {
      $cmp = [string]::Compare($a, $b, [System.StringComparison]::OrdinalIgnoreCase)
      if ($cmp -lt 0) { return -1 }
      if ($cmp -gt 0) { return 1 }
    }
  }

  return 0
}

# --- 1. what is installed -------------------------------------------------
$installedVersion = Read-InstalledVersion -SubKey $UninstKey

$forceRequested = if (Test-ForceRequested -Force $Force -CommandLineFile $CommandLineFile) { 1 } else { 0 }

Write-Log ("[early-guard] installer version='{0}' installed version='{1}' force={2}" -f $InstallerVersion, $installedVersion, $forceRequested)

# --- 2. decide ------------------------------------------------------------
$decision = 'proceed'
$reason = 'unknown'

if ($forceRequested -ne 0) {
  $reason = 'forced'
}
elseif ($installedVersion -eq '') {
  $reason = 'not-installed'
}
else {
  $cmp = Compare-Semver -Left $InstallerVersion -Right $installedVersion

  if ($null -eq $cmp) {
    # Cannot compare -> fail open.
    $reason = 'version-unparsable'
  }
  elseif ($cmp -gt 0) {
    $reason = 'upgrade'
  }
  elseif ($cmp -eq 0) {
    $decision = 'quit'
    $reason = 'same-version'
  }
  else {
    $decision = 'quit'
    $reason = 'older-version'
  }
}

Write-Log ("[early-guard] OUTCOME={0} reason={1}" -f $decision, $reason)

# --- 3. publish the verdict ----------------------------------------------
try {
  Set-Content -LiteralPath $ScriptDecisionFile -Encoding ASCII -Force -ErrorAction Stop `
    -Value ("{0} installer={1} installed={2} decision={3} reason={4}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'), $InstallerVersion, $installedVersion, $decision, $reason)
}
catch {
  # A missing decision file only costs us diagnostics, not correctness.
}

if ($decision -eq 'quit') {
  if ($VerdictFile) {
    try {
      Set-Content -LiteralPath $VerdictFile -Encoding ASCII -Force -ErrorAction Stop -Value $reason
    }
    catch {
      # No verdict file -> the caller keeps going. The installer treats a
      # missing verdict as "show the wizard", which is the safe direction.
    }
  }
  exit 10
}

exit 0
