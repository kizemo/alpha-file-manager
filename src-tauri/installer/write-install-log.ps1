# write-install-log.ps1
#
# Appends ONE durable line describing what this installer run did.
#
# WHY THIS EXISTS (2026-10-22)
# ---------------------------
# The installer had two durable logs -- the single-instance guard log and the
# kill-app-tree log -- but **nothing recorded the installer's own decisions**:
# which install directory was chosen, whether an existing installation was
# detected, which version was already installed, whether this run was a
# reinstall of the very same version, and whether the run finished.
#
# That gap is why "I installed it again and it seemed to activate the online
# update" could not be answered from artifacts: the two existing logs showed
# that no duplicate installer ran, but they said nothing about versions or
# about what the installer did with the installation directory.
#
# This script writes that missing line, to:
#     %TEMP%\alpha-file-manager-install.log
# (overridable for tests). The log is APPENDED, so a machine that has been
# installed many times keeps the whole history.
#
# It also records the SHA256 of the installer package itself
# (installerSha256=...). The version number alone cannot identify a package:
# one version number can ship several builds. The early guard needs that hash
# to tell "this exact file already produced the installed build" from "a
# different file that happens to carry the same version number" -- without it
# a legal reinstall of the same version number was silently blocked.
#
# ASCII ONLY, ON PURPOSE.
# The file is materialised verbatim into $PLUGINSDIR at install time. Windows
# PowerShell 5.1 reads a BOM-less .ps1 using the system ANSI code page, so any
# non-ASCII character would arrive as mojibake. The same rule applies to
# hooks.nsh, which embeds this script.

[CmdletBinding()]
param(
  # Which hook is reporting: PREINSTALL / POSTINSTALL / PREUNINSTALL / POSTUNINSTALL.
  [string]$Phase = 'UNKNOWN',

  # Full path of the running setup executable. Its ProductVersion resource is
  # the version THIS installer is about to install.
  [string]$InstallerPath = '',

  # The installation directory the installer settled on.
  [string]$InstDir = '',

  # Version of the installation that was already present, when one was found.
  [string]$DetectedVersion = '',

  # Location of the installation that was already present, when one was found.
  [string]$DetectedLocation = '',

  # Test-only override for the log destination.
  [string]$LogFileOverride = ''
)

$ErrorActionPreference = 'Continue'

$ScriptLogFile = if ($LogFileOverride) { $LogFileOverride } else {
  Join-Path $env:TEMP 'alpha-file-manager-install.log'
}

function Get-InstallerVersion {
  param([string]$Path)

  if (-not $Path) { return '' }

  try {
    if (-not (Test-Path -LiteralPath $Path)) { return '' }
    $version = (Get-Item -LiteralPath $Path).VersionInfo.ProductVersion
    if ($version) { return ([string]$version).Trim() }
  }
  catch {
  }

  return ''
}

<#
  SHA256 of the installer package that produced this log line.

  WHY (2026-10-31): the early guard has to tell "this exact package file
  already produced the installed build" from "a different file that happens
  to carry the same version number". The version number cannot do that -- one
  version number can ship several builds. The package hash can, so it is
  recorded here, on every successful installation.

  WHY .NET INSTEAD OF Get-FileHash
  --------------------------------
  Get-FileHash lives in the Microsoft.PowerShell.Utility module, and module
  resolution depends on $env:PSModulePath. Measured on the test machine
  (2026-10-31): a Windows PowerShell 5.1 child process inherits a PSModulePath
  in which the cmdlet is NOT resolvable -- "The term 'Get-FileHash' is not
  recognized" -- while the very same script works when started from another
  shell. The installer spawns PowerShell itself, so that case is the normal
  case, not the exotic one.

  System.Security.Cryptography.SHA256 is part of .NET Framework itself: it is
  always there, needs no module, and adds no dependency. Any failure yields
  '' -> logged as "none" -> the guard treats it as "unknown build" and fails
  OPEN.
#>
function Get-InstallerSha256 {
  param([string]$Path)

  if (-not $Path) { return '' }

  $sha = $null
  $stream = $null

  try {
    if (-not (Test-Path -LiteralPath $Path)) { return '' }

    $sha = [System.Security.Cryptography.SHA256]::Create()
    # FileShare.ReadWrite: the installer reads its own executable, and other
    # processes may hold it open for reading at the same time.
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

<#
  Builds the log line. Kept separate from the writing so the exact text can be
  asserted in a unit test without touching the filesystem.
#>
function Format-InstallLogLine {
  param(
    [string]$PhaseName,
    [string]$InstallerVersion,
    [string]$InstallerPathValue,
    [string]$InstallDir,
    [string]$ExistingVersion,
    [string]$ExistingLocation,
    [string]$InstallerSha256
  )

  $sameVersion = 'unknown'

  if ($InstallerVersion -and $ExistingVersion) {
    if ($InstallerVersion -eq $ExistingVersion) {
      $sameVersion = 'yes'
    }
    else {
      $sameVersion = 'no'
    }
  }

  $existingText = $ExistingVersion
  if (-not $existingText) { $existingText = 'none' }

  $locationText = $ExistingLocation
  if (-not $locationText) { $locationText = 'none' }

  $instDirText = $InstallDir
  if (-not $instDirText) { $instDirText = 'none' }

  $installerPathText = $InstallerPathValue
  if (-not $installerPathText) { $installerPathText = 'none' }

  $shaText = $InstallerSha256
  if (-not $shaText) { $shaText = 'none' }

  return ("[install] phase={0} installerVersion={1} sameVersionReinstall={2} " +
    "detectedVersion={3} detectedLocation='{4}' instDir='{5}' installer='{6}' installerSha256={7}") -f
    $PhaseName,
    $(if ($InstallerVersion) { $InstallerVersion } else { 'unknown' }),
    $sameVersion,
    $existingText,
    $locationText,
    $instDirText,
    $installerPathText,
    $shaText
}

$installerVersion = Get-InstallerVersion -Path $InstallerPath
$installerSha256 = Get-InstallerSha256 -Path $InstallerPath

$line = Format-InstallLogLine `
  -PhaseName $Phase `
  -InstallerVersion $installerVersion `
  -InstallerPathValue $InstallerPath `
  -InstallDir $InstDir `
  -ExistingVersion $DetectedVersion `
  -ExistingLocation $DetectedLocation `
  -InstallerSha256 $installerSha256

# nsExec::ExecToLog pipes stdout into the installer's DetailPrint log, so the
# same line shows up in the installer's own log and in the durable file.
Write-Output $line

try {
  Add-Content -LiteralPath $ScriptLogFile -Encoding ASCII `
    -Value ("{0} {1}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'), $line) `
    -ErrorAction Stop
}
catch {
  # Never let logging itself break the install.
}

exit 0
