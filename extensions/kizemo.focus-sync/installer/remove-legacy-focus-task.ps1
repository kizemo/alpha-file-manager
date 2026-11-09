# remove-legacy-focus-task.ps1
#
# Removes the legacy 'KizemoFocusSync' Windows Scheduled Task and the orphan
# "--service" sidecar it used to keep alive. Called by the AFM installer
# (src-tauri/installer/hooks.nsh, STAGE 0.5a and STAGE 9).
#
# Why a bundled script instead of an inline
#   nsExec::ExecToLog 'powershell -NoProfile -Command "..."' :
#   Real-machine install of 3.13.5 (2026-11-10) left the task registered while
#   the installer printed nothing, although the very same inline command
#   removes the task when it is run from a normal (elevated) shell. The inline
#   form is therefore treated as broken inside the installer and the whole
#   cleanup moved into a file that is invoked with `powershell -File` -- the
#   form that is known to run inside the installer (STAGE 6 uses it for
#   register.ps1).
#
# Why schtasks.exe and not the ScheduledTasks PowerShell module:
#   schtasks is a native binary, so it does not depend on the module being
#   loadable in the installer's context. The module call is kept only as a
#   fallback for the case where schtasks refuses the task.
#
# Idempotent: a missing task counts as success.
# Evidence: appends one line per step to %TEMP%\afm-legacy-task-cleanup.log
#
# Pure ASCII on purpose: PowerShell 5.1 reads a BOM-less .ps1 as ANSI.

[CmdletBinding()]
param()

$ErrorActionPreference = 'Continue'

$taskName = 'KizemoFocusSync'
$logPath = Join-Path $env:TEMP 'afm-legacy-task-cleanup.log'

function Write-Log {
    param([string]$Message)
    $line = (Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff') + ' ' + $Message
    Write-Host $line
    try {
        Add-Content -LiteralPath $logPath -Value $line -Encoding ASCII -ErrorAction SilentlyContinue
    } catch { }
}

function Get-AsciiText {
    param($Text)
    if ($null -eq $Text) { return '' }
    $s = ($Text | Out-String).Trim()
    $s = $s -replace '[^\x20-\x7E]', '?'
    return $s
}

function Get-TaskState {
    $null = schtasks /Query /TN $taskName 2>&1
    if ($LASTEXITCODE -eq 0) { return 'present' }
    return 'absent'
}

function Get-TaskRegisteredDate {
    # The task XML carries <Date> = registration time. Comparing it before and
    # after an install tells "never removed" apart from "removed and then
    # created again by something else".
    $xml = Join-Path (Join-Path $env:SystemRoot 'System32\Tasks') $taskName
    try {
        $content = [System.IO.File]::ReadAllText($xml)
        $m = [regex]::Match($content, '<Date[^>]*>([^<]*)</Date>')
        if ($m.Success) { return $m.Groups[1].Value }
    } catch { }
    return 'unknown'
}

Write-Log ('=== remove-legacy-focus-task START pid=' + $PID + ' user=' + $env:USERNAME + ' ===')
Write-Log ('state_before=' + (Get-TaskState) + ' registered_date=' + (Get-TaskRegisteredDate))

# --- Step 1: disable, so the task cannot respawn the sidecar mid-install ---
$out = schtasks /Change /TN $taskName /DISABLE 2>&1
Write-Log ('schtasks /Change /DISABLE exit=' + $LASTEXITCODE + ' out=' + (Get-AsciiText $out))

# --- Step 2: stop the running instance ---
$out = schtasks /End /TN $taskName 2>&1
Write-Log ('schtasks /End exit=' + $LASTEXITCODE + ' out=' + (Get-AsciiText $out))

# --- Step 3: delete the task ---
$out = schtasks /Delete /TN $taskName /F 2>&1
Write-Log ('schtasks /Delete /F exit=' + $LASTEXITCODE + ' out=' + (Get-AsciiText $out))

# --- Step 4: verify, and retry once through the PowerShell module ---
$state = Get-TaskState
if ($state -eq 'present') {
    Write-Log 'RESULT=LEGACY_TASK_STILL_PRESENT (retrying through the module)'
    $out = Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
    Write-Log ('Unregister-ScheduledTask fallback out=' + (Get-AsciiText $out))
    $state = Get-TaskState
}
if ($state -eq 'present') {
    Write-Log 'RESULT=LEGACY_TASK_STILL_PRESENT_AFTER_RETRY'
    exit 1
}
Write-Log 'RESULT=LEGACY_TASK_GONE'

# --- Step 5: kill an orphan sidecar. The task is gone, so nothing respawns
#     it; the app starts its own instance later. ---
$patterns = @(
    '*\Alpha File Manager\tools\focus-sync-sidecar.exe',
    '*\Sigma File Manager\tools\focus-sync-sidecar.exe',
    '*\kizemo.focus-sync\bin\focus-sync-sidecar.exe'
)
$killed = 0
$procs = @(Get-Process focus-sync-sidecar -ErrorAction SilentlyContinue)
foreach ($proc in $procs) {
    $path = ''
    try { $path = [string]$proc.Path } catch { $path = '' }
    if (-not $path) { continue }
    $hit = $false
    foreach ($pat in $patterns) {
        if ($path -like $pat) { $hit = $true; break }
    }
    if (-not $hit) { continue }
    try {
        Stop-Process -Id $proc.Id -Force -ErrorAction Stop
        $killed++
        Write-Log ('killed orphan sidecar pid=' + $proc.Id + ' path=' + $path)
    } catch {
        Write-Log ('WARNING: could not kill pid=' + $proc.Id + ': ' + $proc.Exception.Message)
    }
}
Write-Log ('=== remove-legacy-focus-task DONE killed=' + $killed + ' ===')
exit 0
