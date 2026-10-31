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
# redundant launch harmless: when THIS VERY PACKAGE has already produced the
# installed build, there is nothing to do -- so exit silently, before any
# window exists.
#
# THE RULES (2026-10-31: build identity, not version number)
# ---------------------------------------------------------
#   not installed at all          -> proceed  (normal first install)
#   installer version NEWER       -> proceed  (normal upgrade)
#   installer version OLDER       -> proceed  (downgrade; the NSIS template
#                                            has its own confirmation page)
#   installer version SAME        -> compare BUILD IDENTITY:
#        same package file        -> quit     (this exact file already
#                                            installed this exact build)
#        different package file   -> proceed  (same version number, new
#                                            build; the template's "already
#                                            installed" page asks the user)
#   anything cannot be determined -> proceed  (fail open: a broken guard must
#                                           never make the product
#                                           uninstallable)
#
# WHY BUILD IDENTITY INSTEAD OF THE VERSION NUMBER
# -------------------------------------------------
# The version number alone is NOT the identity of an installer package: one
# version number can ship more than one build (a rebuilt package, a patched
# build, a second packaging run), and the version number cannot tell them
# apart. Guarding on the version number therefore blocked a LEGAL install:
# the user double clicked a fresh 3.8.0 package while 3.8.0 was already
# installed, the guard decided "same version, nothing to do" and the
# installer exited before a single window appeared.
#
# The build identity is the SHA256 of the installer package itself. The
# install log (write-install-log.ps1) records that hash on every successful
# installation, so the guard can tell "this exact file already installed
# this exact build" (the redundant host auto-open, which must stay silent)
# from "a different file that happens to carry the same version number"
# (a deliberate reinstall, which must show the wizard).
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

  # Full path of the setup executable that is running right now. The guard
  # hashes it to identify the BUILD this package carries (see the header
  # comment: the version number is not a build identity).
  [string]$InstallerPath = '',

  # Path of the durable install log that records which installer package the
  # last successful installation used. The installer does not pass it (the
  # default below is the real location); tests override it.
  [string]$InstallLogFile = '',

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

# The durable install log written by write-install-log.ps1. It is the only
# place that remembers WHICH installer package produced the installed build.
$ScriptInstallLogFile = if ($InstallLogFile) { $InstallLogFile } else {
  Join-Path $env:TEMP 'alpha-file-manager-install.log'
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

  # (?i)(?:^|[\s"'])[-/]{0,2}FORCE(?:[\s"']|$) - the token is delimited by
  # whitespace or a quote on both sides, and may carry up to two leading
  # dashes/slashes. So "setup.exe /FORCE" (the NSIS spelling), "setup.exe
  # /force", "setup.exe -force" and "setup.exe --force" all match, while
  # "/XFORCE" and "C:\FORCE\setup.exe" do not.
  if ($text -match '(?i)(?:^|[\s"''])[-/]{0,2}FORCE(?:[\s"'']|$)') { return $true }

  return $false
}

# SHA256 of a file, lower case, or '' when it cannot be computed.
#
# Used to identify the BUILD carried by the installer package. Any failure
# (missing file, unreadable file) yields '' -- and the caller treats '' as
# "unknown", which fails OPEN.
#
# WHY .NET INSTEAD OF Get-FileHash
# --------------------------------
# Get-FileHash lives in the Microsoft.PowerShell.Utility module, and module
# resolution depends on $env:PSModulePath. Measured on the test machine
# (2026-10-31): a Windows PowerShell 5.1 child process inherits a PSModulePath
# in which the cmdlet is NOT resolvable -- "The term 'Get-FileHash' is not
# recognized" -- while the very same script works when started from another
# shell. The installer spawns PowerShell itself, so that case is the normal
# case, not the exotic one. System.Security.Cryptography.SHA256 is part of
# .NET Framework itself: always available, no module, no new dependency.
function Get-FileSha256 {
  param([string]$Path)

  if (-not $Path) { return '' }

  $sha = $null
  $stream = $null

  try {
    if (-not (Test-Path -LiteralPath $Path)) { return '' }

    $sha = [System.Security.Cryptography.SHA256]::Create()
    # FileShare.ReadWrite: the installer hashes its own executable.
    $stream = [System.IO.File]::Open($Path, 'Open', 'Read', 'ReadWrite')
    $bytes = $sha.ComputeHash($stream)

    $text = [System.BitConverter]::ToString($bytes).Replace('-', '').ToLowerInvariant()
    if ($text) { return $text }
  }
  catch {
  }
  finally {
    if ($null -ne $stream) { $stream.Close() }
    if ($null -ne $sha) { $sha.Dispose() }
  }

  return ''
}

# SHA256 of the installer package used by the most recent SUCCESSFUL
# installation, or '' when it cannot be determined.
#
# The install log is appended, one line per hook phase. Only the POSTINSTALL
# lines describe a completed installation, and the LAST such line describes
# the build that is installed right now. Older lines are skipped, so a later
# upgrade (or a failed run) never shadows the record we care about.
function Get-LastInstalledInstallerSha256 {
  param([string]$LogPath)

  if (-not $LogPath) { return '' }
  if (-not (Test-Path -LiteralPath $LogPath)) { return '' }

  $text = ''

  try {
    $fs = [System.IO.File]::Open($LogPath, 'Open', 'Read', 'ReadWrite')
    $sr = New-Object System.IO.StreamReader($fs)
    $text = $sr.ReadToEnd()
    $sr.Close()
    $fs.Close()
  }
  catch {
    return ''
  }

  if (-not $text) { return '' }

  $result = ''

  foreach ($line in ($text -split "`r?`n")) {
    if ($line -notmatch 'phase=POSTINSTALL(?:\s|$)') { continue }
    if ($line -notmatch 'installerSha256=([0-9a-fA-F]{64})') { continue }

    $candidate = $Matches[1].ToLowerInvariant()
    if ($candidate -ne '') { $result = $candidate }
  }

  return $result
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

$installerSha = ''
$lastInstalledSha = ''

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
    # Upgrade -> let the wizard handle it.
    $reason = 'upgrade'
  }
  elseif ($cmp -lt 0) {
    # Downgrade -> let the wizard handle it. The NSIS template has its own
    # "a newer version is installed" page; blocking it here removed the
    # user's only chance to decide.
    $reason = 'downgrade'
  }
  else {
    # Same version number. The version number is NOT a build identity, so
    # the build identity decides: is THIS package file the one that produced
    # the installed build?
    $installerSha = Get-FileSha256 -Path $InstallerPath
    $lastInstalledSha = Get-LastInstalledInstallerSha256 -LogPath $ScriptInstallLogFile

    Write-Log ("[early-guard] installer sha256='{0}' last-install-sha256='{1}'" -f $installerSha, $lastInstalledSha)

    if ($installerSha -eq '' -or $lastInstalledSha -eq '') {
      # Fail open: without both hashes there is no PROOF that this is the
      # very same package, and a legal install must never be blocked.
      $reason = 'same-version-unknown-build'
    }
    elseif ($installerSha -eq $lastInstalledSha) {
      # Byte for byte the same package that produced the installed build.
      # This is the redundant host auto-open the guard exists for.
      $decision = 'quit'
      $reason = 'same-build'
    }
    else {
      # Same version number, different build -> a real reinstall.
      $reason = 'same-version-new-build'
    }
  }
}

Write-Log ("[early-guard] OUTCOME={0} reason={1}" -f $decision, $reason)

# --- 3. publish the verdict ----------------------------------------------
try {
  Set-Content -LiteralPath $ScriptDecisionFile -Encoding ASCII -Force -ErrorAction Stop `
    -Value ("{0} installer={1} installed={2} decision={3} reason={4} installerSha256={5} lastInstallSha256={6}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'), $InstallerVersion, $installedVersion, $decision, $reason, $installerSha, $lastInstalledSha)
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
