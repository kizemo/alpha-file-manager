<#
.SYNOPSIS
  Terminate Alpha/Sigma File Manager process trees WITHOUT touching the caller's own subtree,
  then WAIT until the binaries are really released and PROVE it with an exclusive open.

.DESCRIPTION
  WHY NOT taskkill /F /IM <image> /T

  The old PREINSTALL hook used `taskkill /F /IM alpha-file-manager.exe /T`.
  /T means "kill the whole descendant tree". But when the user double-clicks the
  installer from inside Alpha, the process tree is:

      alpha-file-manager.exe
        +- Alpha File Manager_3.0.3_x64-setup.exe   <- the installer itself
             +- taskkill.exe

  The installer is a CHILD of Alpha, so that single command killed the installer
  mid-install, with no readable error.

  This script reproduces taskkill /T semantics safely:

    1. Snapshot every process (pid / image name / parent pid / image path) BEFORE
       touching anything, because all tree maths is derived from this one snapshot.
    2. protected = caller pid + every descendant of the caller.
       This covers the installer, plus taskkill / powershell / msiexec that the
       installer itself spawns.
    3. targets = processes whose image name matches the kill list.
    4. Every descendant of a target joins the victim set -- that is exactly what
       /T would have done (WebView2 / crashpad children that mmap the exe).
    5. Subtract the protected set, then terminate deepest-first.

  Ordering matters: descendants MUST be collected before the parent is killed.
  Once a parent dies Windows reparents its orphans, and the tree link is gone.

  When the caller is NOT a descendant of a target -- the normal case, user
  started the installer from Explorer -- the protected and victim sets are
  disjoint, so behaviour is identical to the old `taskkill /T`, including the
  WebView2 cleanup that Tauri's CheckIfAppIsRunning depends on.

  v3.1.1 (2026-10-14): the "caller + descendants" rule ALONE was not enough,
  and it killed the installer during an upgrade. Real-machine log:

      08:33:30.529  caller pid resolved to 121796
      08:33:30.563  SKIP protected PID=121796 uninstall.exe
      08:33:30.667  terminated PID=83836 Alpha File Manager_3.1.0_x64-setup.exe
      08:33:30.708  OUTCOME=ok exit=0

  Why: during an upgrade hooks.nsh:667 runs the SAME macro through
  `un.AlphaTerminateAppProcesses`, so the caller is `uninstall.exe`, not the
  setup process. Its protected subtree is only itself + nsExec + conhost. The
  setup process is an ANCESTOR of the uninstaller (setup launched it), and
  Get-Descendants only walks downward, so setup was never protected. When the
  user had launched the setup from inside the app, setup was additionally a
  descendant of alpha-file-manager.exe, so it landed in the VICTIM set and was
  terminated mid-install. The install then failed with no readable error.

  The fix is NOT "protect the whole ancestor chain". That would also protect
  alpha-file-manager.exe itself, the app would survive, keep its exe locked, and
  Tauri CheckIfAppIsRunning would abort the install with "Failed to kill". The
  test in _acceptance/test-kill-protects-installer.ps1 asserts BOTH halves for
  exactly this reason.

  The fix is an image-name invariant that does not care about tree position:
  any process whose image ends in `setup.exe` or `uninstall.exe` is an installer
  or uninstaller and is never a victim. It covers the upgrade case above, the
  mirror case (a parallel uninstaller during a plain install), and a user who
  double-clicks the setup twice.

  v3.6.6 (2026-10-29): the upgrade path failed because this script returned as
  soon as the kill request had been issued. Tauri's NSIS template then asked the
  Windows Restart Manager whether anything still used the installed binary. The
  answer was still "yes" -- the process had not finished dying yet -- so the
  installer showed "the application is running, click OK to terminate it", the
  graceful shutdown failed, the uninstaller aborted, and the whole upgrade died
  with "the application could not be terminated". Real-machine timeline:

      10:27:20.474  snapshot: 301 processes
      10:27:20.841  terminated PID=403472 alpha-file-manager.exe
      10:27:20.900  done: killed=8 failed=0, installer alive.
      10:27:23.033  PREUNINSTALL  (the uninstaller kept going)
      10:27:23.xxx  Restart Manager still saw a user of the binary -> dialog

  Three things are new in this revision:

    1. -DisableScheduledTask stops \KizemoFocusSync BEFORE anything is killed, so
       the sidecar cannot be respawned while the application is being closed.
    2. After the kill we WAIT until every terminated pid is really gone, and we
       kill the stubborn ones a second time.
    3. We VERIFY the result by opening the main binary with FileShare.None. If
       that open succeeds, no process holds the file open or mapped, so the
       Restart Manager cannot report it and Tauri will not ask the user anything.

  Exit codes:
    0  everything closed, everything verified
    1  at least one process survived the kill
    2  the process list could not be enumerated (nothing was killed)
    3  the binary is still locked after the retry window

  ASCII ONLY, ON PURPOSE.
  This file is embedded verbatim into installer/hooks.nsh and materialised at
  run time with NSIS FileWrite, which converts strings through the system ANSI
  code page. Non-ASCII characters in this body would be written to disk as
  mojibake and the script would fail to parse. The Chinese rationale lives in
  hooks.nsh instead, where makensis handles UTF-8 source fine.

.PARAMETER SelfPid
  Optional. Caller's process id, protected along with its whole subtree.

  v0.5.11 (2026-10-13): this parameter is NO LONGER PASSED BY THE INSTALLER, and
  passing it was the bug that made this whole hook a no-op.

  The installer used to run:

      powershell ... -File "$PLUGINSDIR/kill-app-tree.ps1" -SelfPid $0

  but NSIS `$0` at that point is NOT the installer's PID. Tauri seeds it at
  `installer.nsi:196` with `StrCpy $0 0` and then uses it as the index of the
  wix_detect EnumRegKey loop over the HKLM Uninstall hive, so by PREINSTALL it
  holds "however many installed programs this machine has" (76 on this box).
  Whatever PID that happens to be, it is an accident: if it ever lands on an
  ancestor of a target process, `protected` swallows the victim and nothing
  gets killed; if it is absent the mandatory-parameter binding fails and the
  script dies before it does anything at all. Both produce the same visible
  symptom - the app and the sidecar survive the install.

  So the script now derives the caller itself: nsExec spawns powershell.exe as
  a direct child of the installer, so the installer's PID is simply this
  process's parent. That is correct by construction and cannot drift.

.PARAMETER InstDir
  Optional. The directory the application is installed in. When given, the main
  binary below it is verified with an exclusive open after the kill.

.PARAMETER DisableScheduledTask
  Optional. Stop and disable the \KizemoFocusSync scheduled task before killing.

.PARAMETER WaitTimeoutMs
  Optional. How long to wait for the killed processes to disappear. Default 20000.

.PARAMETER VerifyTimeoutMs
  Optional. How long to wait for the binary to become exclusively openable.
  Default 20000.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File kill-app-tree.ps1
#>
[CmdletBinding()]
param(
  [int]$SelfPid = 0,

  # Test-only override for the log destination. Production calls never pass it
  # and keep using %TEMP%\alpha-file-manager-kill.log. It exists so the
  # regression harness in _acceptance/ can assert on its own log without
  # interleaving entries into the production log that real installs rely on.
  [string]$LogFileOverride = '',

  # The directory the application is installed in. When it is given, the main
  # binary below it is opened exclusively after the kill, which proves that no
  # process holds it any more.
  [string]$InstDir = '',

  # Stop and disable \KizemoFocusSync before killing, so the sidecar cannot be
  # respawned while the application is being closed.
  [switch]$DisableScheduledTask,

  # How long to wait for the killed processes to disappear.
  [int]$WaitTimeoutMs = 20000,

  # How long to wait for the binary to become exclusively openable.
  [int]$VerifyTimeoutMs = 20000,

  [string[]]$AppImages = @(
    'alpha-file-manager.exe',
    'SigmaFileManager.exe',
    'Sigma File Manager.exe',
    'SigmaFileManagerHelper.exe',
    'SigmaFileManagerRenderer.exe',
    'Sigma File Manager Renderer.exe',
    'Alpha File Manager.exe',
    'AlphaFileManager.exe',
    'AlphaFileManagerHelper.exe',
    'AlphaFileManagerRenderer.exe',
    'Alpha File Manager Renderer.exe'
  ),

  [string[]]$HelperImages = @(
    'spike.exe',
    'focus-sync-sidecar.exe'
  )
)

$ErrorActionPreference = 'Continue'

# v0.5.11: persist every line to a file as well as to stdout.
#
# WHY: the installer's only log is the MUI progress pane, which vanishes when the
# window closes. NSIS was not writing a log file at all, so "did the kill script
# run, what caller pid did it resolve, which pids did it skip" was unanswerable
# after the fact -- which is exactly the question 2026-10-13 needed answered.
#
# ASCII + no backslashes is still mandatory (see embed-kill-script.mjs), so the
# path is built with Join-Path rather than written literally.
$ScriptLogFile = if ($LogFileOverride) { $LogFileOverride } else {
  Join-Path $env:TEMP 'alpha-file-manager-kill.log'
}

function Write-Log {
  param([string]$Message)
  # nsExec::ExecToLog pipes stdout into the installer's DetailPrint log.
  Write-Output $Message
  try {
    Add-Content -LiteralPath $ScriptLogFile -Encoding ASCII `
      -Value ("{0} {1}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'), $Message) `
      -ErrorAction Stop
  }
  catch {
    # Never let logging itself break the kill.
  }
}

# --- helpers -----------------------------------------------------------------

function Get-ProcTable {
  # One snapshot of every process. The image path is included because it tells
  # us WHICH file has to be unlocked afterwards.
  $table = @{}

  Get-CimInstance Win32_Process -ErrorAction Stop | ForEach-Object {
    $path = ''
    try { $path = [string]$_.ExecutablePath } catch { $path = '' }

    $table[[int]$_.ProcessId] = [pscustomobject]@{
      Pid  = [int]$_.ProcessId
      Name = [string]$_.Name
      Ppid = [int]$_.ParentProcessId
      Path = $path
    }
  }

  return $table
}

function Get-Descendants {
  param([int]$RootPid, [hashtable]$Table)

  $out = New-Object System.Collections.ArrayList
  $seen = @{}
  $frontier = New-Object System.Collections.ArrayList
  [void]$frontier.Add($RootPid)

  while ($frontier.Count -gt 0) {
    $current = $frontier[0]
    $frontier.RemoveAt(0)

    foreach ($p in $Table.Values) {
      if ($p.Ppid -ne $current) { continue }
      if ($seen.ContainsKey($p.Pid)) { continue }
      $seen[$p.Pid] = $true
      [void]$out.Add($p)
      [void]$frontier.Add($p.Pid)
    }
  }

  return $out
}

function Disable-AfmScheduledTask {
  # The sidecar is started by \KizemoFocusSync with -RestartCount 3 /
  # -RestartInterval 1min. If the task is still enabled, it can respawn the
  # sidecar in the middle of the kill and lock the binaries again.
  try {
    $task = Get-ScheduledTask -TaskName 'KizemoFocusSync' -ErrorAction SilentlyContinue
    if ($null -eq $task) {
      Write-Log "    [kill-app-tree] scheduled task KizemoFocusSync is not present; nothing to disable."
      return
    }

    Stop-ScheduledTask -TaskName 'KizemoFocusSync' -ErrorAction SilentlyContinue
    Disable-ScheduledTask -TaskName 'KizemoFocusSync' -ErrorAction Stop
    Write-Log "    [kill-app-tree] scheduled task KizemoFocusSync stopped and disabled."
  }
  catch {
    Write-Log "    [kill-app-tree] WARNING: could not disable scheduled task KizemoFocusSync: $($_.Exception.Message)"
  }
}

function Wait-PidsGone {
  # Poll until every pid in the list is really gone, or the timeout expires.
  # Returns the pids that are still alive.
  param([int[]]$Pids, [int]$TimeoutMs)

  $remaining = @()
  if ($null -eq $Pids) { return $remaining }
  $remaining = @($Pids)
  if ($remaining.Count -eq 0) { return $remaining }

  $deadline = (Get-Date).AddMilliseconds([Math]::Max($TimeoutMs, 0))

  while ($remaining.Count -gt 0) {
    if ((Get-Date) -ge $deadline) { break }
    Start-Sleep -Milliseconds 50

    $still = @()
    foreach ($pidValue in $remaining) {
      if (Get-Process -Id $pidValue -ErrorAction SilentlyContinue) { $still += $pidValue }
    }
    $remaining = $still
  }

  return $remaining
}

function Test-ExclusiveOpen {
  # TRUE when the file can be opened with FileShare.None, i.e. when NO other
  # process holds it open and NO section maps it. That is exactly the condition
  # the Windows Restart Manager looks for, so a successful open proves that the
  # Restart Manager cannot report this file as "in use".
  param([string]$Path)

  if ([string]::IsNullOrWhiteSpace($Path)) { return $true }
  if (-not (Test-Path -LiteralPath $Path)) { return $true }

  try {
    $stream = [System.IO.File]::Open(
      $Path,
      [System.IO.FileMode]::Open,
      [System.IO.FileAccess]::Read,
      [System.IO.FileShare]::None
    )
    $stream.Close()
    return $true
  }
  catch {
    return $false
  }
}

# --- main --------------------------------------------------------------------

Write-Log "    [kill-app-tree] start (pid=$PID)."

if ($DisableScheduledTask) {
  Disable-AfmScheduledTask
}

# The image set we are allowed to terminate.
$imageSet = @{}
$appImageSet = @{}
foreach ($n in (@($AppImages) + @($HelperImages))) { $imageSet[$n.ToLowerInvariant()] = $true }
foreach ($n in @($AppImages)) { $appImageSet[$n.ToLowerInvariant()] = $true }

# v3.1.1 (2026-10-14): nothing named *setup.exe or *uninstall.exe is ever a
# victim, no matter where it sits in the process tree.
$InstallerImagePattern = '(?i)(setup|uninstall)\.exe$'

$killed = 0
$failed = 0
$verifyPaths = @{}
$verifiedKills = $false

for ($round = 1; $round -le 2; $round++) {
  # --- 1. snapshot ----------------------------------------------------------
  try {
    $procs = Get-ProcTable
  }
  catch {
    Write-Log "    [kill-app-tree] ERROR: cannot enumerate processes ($($_.Exception.Message)); killing nothing."
    Write-Log "    [kill-app-tree] OUTCOME=enum-failed exit=2 (killed nothing)"
    exit 2
  }

  Write-Log "    [kill-app-tree] snapshot (round $round): $($procs.Count) processes"

  # --- 2. protected set: caller + all caller descendants --------------------
  # The installer lives here when launched from Alpha, so nothing that belongs
  # to the running installation can ever be terminated by mistake.
  #
  # v0.5.11: derive the caller instead of trusting -SelfPid (see .PARAMETER).
  $callerPid = if ($SelfPid -gt 0) { $SelfPid } else {
    try {
      $parent = Get-CimInstance Win32_Process -Filter "ProcessId=$PID" -ErrorAction Stop
      [int]$parent.ParentProcessId
    }
    catch {
      $PID
    }
  }
  $callerPid = if ($callerPid -le 0) { $PID } else { $callerPid }

  $protected = @{}

  if ($procs.ContainsKey($callerPid)) {
    $protected[$callerPid] = $true
  }
  foreach ($d in (Get-Descendants -RootPid $callerPid -Table $procs)) {
    $protected[$d.Pid] = $true
  }

  Write-Log "    [kill-app-tree] caller pid resolved to $callerPid (this script pid=$PID)"

  if ($protected.Count -gt 1) {
    Write-Log "    [kill-app-tree] protected subtree: $($protected.Count) processes (installer itself + descendants)"
  }

  # --- 2b. installer / uninstaller images are never victims ------------------
  foreach ($p in $procs.Values) {
    if ($p.Name -notmatch $InstallerImagePattern) { continue }
    if ($protected.ContainsKey($p.Pid)) { continue }
    Write-Log "    [kill-app-tree] SKIP PID=$($p.Pid) $($p.Name) (installer/uninstaller image; never a kill target)"
    $protected[$p.Pid] = $true
  }

  # --- 3. targets -----------------------------------------------------------
  $targets = @($procs.Values | Where-Object {
    $imageSet.ContainsKey($_.Name.ToLowerInvariant()) -and -not $protected.ContainsKey($_.Pid)
  })

  if ($targets.Count -eq 0) {
    Write-Log "    [kill-app-tree] no running target process found; nothing to terminate."
    break
  }

  foreach ($t in $targets) {
    Write-Log "    [kill-app-tree] target found: PID=$($t.Pid) $($t.Name)"
  }

  # --- 4. victims = targets + every descendant (= taskkill /T) ---------------
  $victims = New-Object System.Collections.ArrayList
  $depth = @{}

  foreach ($t in $targets) {
    [void]$victims.Add($t)
    $depth[$t.Pid] = 0

    $d = 0
    foreach ($child in (Get-Descendants -RootPid $t.Pid -Table $procs)) {
      [void]$victims.Add($child)
      $depth[$child.Pid] = ++$d
    }
  }

  # Subtracting the protected set is the whole point of this script.
  $toKill = @($victims | Where-Object { -not $protected.ContainsKey($_.Pid) })

  foreach ($s in @($victims | Where-Object { $protected.ContainsKey($_.Pid) })) {
    Write-Log "    [kill-app-tree] SKIP protected PID=$($s.Pid) $($s.Name) (belongs to the installer subtree)"
  }

  if ($toKill.Count -eq 0) {
    Write-Log "    [kill-app-tree] target subtree contains nothing outside the installer; nothing to terminate."
    break
  }

  # --- 5. terminate deepest-first -------------------------------------------
  $ordered = @($toKill | Sort-Object -Property `
    @{ Expression = { $depth[$_.Pid] } }, `
    @{ Expression = { $_.Pid } } -Descending)

  $killedPids = @()

  foreach ($v in $ordered) {
    try {
      Stop-Process -Id $v.Pid -Force -ErrorAction Stop
      Write-Log "    [kill-app-tree] terminated PID=$($v.Pid) $($v.Name)"
      $killed++
      $killedPids += $v.Pid
    }
    catch {
      if (Get-Process -Id $v.Pid -ErrorAction SilentlyContinue) {
        Write-Log "    [kill-app-tree] FAILED to terminate PID=$($v.Pid) $($v.Name): $($_.Exception.Message)"
        $failed++
        $killedPids += $v.Pid
      }
      else {
        Write-Log "    [kill-app-tree] PID=$($v.Pid) $($v.Name) already exited on its own."
      }
    }
  }

  # Remember which binaries have to be unlocked afterwards. Only the application's
  # OWN images are interesting: the WebView2 runtime is shared with other
  # applications and is regularly held open by them, so verifying it would
  # report a false "still locked".
  foreach ($v in $toKill) {
    if ([string]::IsNullOrWhiteSpace($v.Path)) { continue }
    $leaf = [System.IO.Path]::GetFileName($v.Path).ToLowerInvariant()
    if (-not $appImageSet.ContainsKey($leaf)) { continue }
    $verifyPaths[$v.Path] = $true
  }

  # --- 6. wait for the processes to REALLY be gone --------------------------
  # Stop-Process only requests the termination. The process may still be
  # tearing down, and the Restart Manager sees a live process for a while
  # after the kill -- that is what broke the upgrade on 2026-10-29.
  $survivors = @(Wait-PidsGone -Pids $killedPids -TimeoutMs $WaitTimeoutMs)

  if ($survivors.Count -gt 0) {
    Write-Log "    [kill-app-tree] still alive after ${WaitTimeoutMs}ms: $($survivors -join ', '); killing again."
    foreach ($s in $survivors) {
      try { Stop-Process -Id $s -Force -ErrorAction Stop } catch { }
    }
    $survivors = @(Wait-PidsGone -Pids $survivors -TimeoutMs 5000)
  }

  if ($survivors.Count -gt 0) {
    Write-Log "    [kill-app-tree] SURVIVORS: $($survivors -join ', ')"
    $failed += $survivors.Count
  }
  else {
    Write-Log "    [kill-app-tree] all terminated processes are gone (waited up to ${WaitTimeoutMs}ms)."
  }

  $verifiedKills = $true

  # A second round is only needed when something refused to die.
  if ($survivors.Count -eq 0) { break }
}

# --- 7. verify that the binaries are really released ------------------------
$toVerify = @()

foreach ($p in $verifyPaths.Keys) { $toVerify += $p }

if (-not [string]::IsNullOrWhiteSpace($InstDir)) {
  $toVerify += (Join-Path $InstDir 'alpha-file-manager.exe')
}

$toVerify = @($toVerify | Select-Object -Unique)

if ($toVerify.Count -eq 0) {
  Write-Log "    [kill-app-tree] nothing to verify (no running instance and no InstDir given)."
  Write-Log "    [kill-app-tree] OUTCOME=ok exit=0 (log file: $ScriptLogFile)"
  exit 0
}

$locked = @()
$verifyDeadline = (Get-Date).AddMilliseconds([Math]::Max($VerifyTimeoutMs, 0))

while ($true) {
  $locked = @()
  foreach ($p in $toVerify) {
    if (-not (Test-ExclusiveOpen -Path $p)) { $locked += $p }
  }

  if ($locked.Count -eq 0) { break }
  if ((Get-Date) -ge $verifyDeadline) { break }
  Start-Sleep -Milliseconds 200
}

if ($locked.Count -gt 0) {
  foreach ($p in $locked) {
    Write-Log "    [kill-app-tree] STILL LOCKED: $p"
  }

  # Name the processes that are still running from one of those paths, so the
  # next round of forensics does not have to guess again.
  try {
    $now = Get-ProcTable
    foreach ($lp in $locked) {
      foreach ($proc in $now.Values) {
        if ([string]::IsNullOrWhiteSpace($proc.Path)) { continue }
        if (-not [System.String]::Equals($proc.Path, $lp, [System.StringComparison]::OrdinalIgnoreCase)) { continue }
        Write-Log "    [kill-app-tree] holder candidate: PID=$($proc.Pid) $($proc.Name) $($proc.Path)"
      }
    }
  }
  catch {
    Write-Log "    [kill-app-tree] could not enumerate processes for the holder report: $($_.Exception.Message)"
  }

  Write-Log "    [kill-app-tree] OUTCOME=locked exit=3 (the binary is still in use after ${VerifyTimeoutMs}ms)"
  exit 3
}

Write-Log "    [kill-app-tree] verified: $($toVerify.Count) binary path(s) can be opened exclusively."
Write-Log "    [kill-app-tree] done: killed=$killed failed=$failed, installer alive."

if ($failed -gt 0) { Write-Log "    [kill-app-tree] OUTCOME=partial exit=1"; exit 1 }
Write-Log "    [kill-app-tree] OUTCOME=ok exit=0 (log file: $ScriptLogFile)"
exit 0
