# ============================================================================
# read-sidecar-log.ps1 - decoded, human-readable view of the sidecar dialog leg
# ============================================================================
# WHY THIS EXISTS
#   1. The sidecar splits its diagnostics across TWO files with DIFFERENT
#      purposes, and neither has a `charset` on the wire:
#        - spike.log            JSONL events (http_server::log_event)
#        - spike.log.YYYY-MM-DD tracing (uia_inject / writer / uia_event)
#      Reading only one of them loses the actual reason a write failed.
#   2. BOTH are UTF-8. PowerShell 5.1's Get-Content / Invoke-RestMethod default
#      to the system ANSI code page (936/GBK on zh-CN), which renders correct
#      UTF-8 Chinese as mojibake. That misreading caused a FALSE root-cause
#      investigation on 2026-10-07. This script ALWAYS decodes as UTF-8 and
#      requires shared file access because the sidecar holds the handle open.
#
# USAGE
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\read-sidecar-log.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\read-sidecar-log.ps1 -Tail 30
# ============================================================================

[CmdletBinding()]
param(
    [int]$Tail = 24,
    [switch]$SkipTracing
)

$ErrorActionPreference = 'Stop'
$logDir = Join-Path $env:LOCALAPPDATA 'kizemo\focus-sync\logs'

function Read-Shared([string]$Path) {
    if (-not (Test-Path $Path)) { return $null }
    try {
        $fs = [System.IO.File]::Open($Path, [System.IO.FileMode]::Open,
                                     [System.IO.FileAccess]::Read,
                                     [System.IO.FileShare]::ReadWrite)
        $sr = New-Object System.IO.StreamReader($fs, [System.Text.Encoding]::UTF8)
        $txt = $sr.ReadToEnd()
        $sr.Close(); $fs.Close()
        return $txt
    } catch {
        Write-Warning "cannot read $Path : $_"
        return $null
    }
}

$ESC = [char]27
function Strip-Ansi([string]$s) { return ($s -replace "$ESC\[[0-9;]*m", '') }

# ---------------------------------------------------------------- live state
Write-Host ""
Write-Host "=============================================================="
Write-Host "SIDECAR LIVE STATE"
Write-Host "=============================================================="
try {
    foreach ($ep in @('health', 'get_status')) {
        try {
            $r = Invoke-WebRequest -Uri "http://127.0.0.1:37421/$ep" -TimeoutSec 5 -UseBasicParsing
            # Explicit UTF-8: the sidecar sends no charset, so client defaults lie.
            $body = [System.Text.Encoding]::UTF8.GetString($r.RawContentStream.ToArray())
            Write-Host ("  /$ep -> " + $body)
        } catch {
            Write-Host ("  /$ep -> UNREACHABLE (" + $_.Exception.Message + ")")
        }
    }
} catch { }

# ------------------------------------------------------------ live dialogs
Add-Type -TypeDefinition @'
using System; using System.Text; using System.Runtime.InteropServices;
public class DlgProbe {
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
}
'@
$live = @()
$cb = [DlgProbe+EnumProc] {
    param($h, $l)
    $c = New-Object System.Text.StringBuilder 256
    [void][DlgProbe]::GetClassName($h, $c, 256)
    if ($c.ToString() -eq '#32770') {
        $t = New-Object System.Text.StringBuilder 512
        [void][DlgProbe]::GetWindowText($h, $t, 512)
        $script:live += [pscustomobject]@{
            Hwnd = $h.ToInt64(); Title = $t.ToString(); IsForeground = ($h -eq [DlgProbe]::GetForegroundWindow())
        }
    }
    return $true
}
[void][DlgProbe]::EnumWindows($cb, [IntPtr]::Zero)
Write-Host ""
Write-Host "--- LIVE #32770 dialogs on this desktop ---"
if ($live.Count -eq 0) {
    Write-Host "  (none)  <-- if /get_status says active_dialogs>0, the registry holds DEAD hwnds"
} else {
    $live | ForEach-Object { Write-Host ("  hwnd=$($_.Hwnd) fg=$($_.IsForeground) title='$($_.Title)'") }
}

# --------------------------------------------------------------- JSONL log
$jsonl = Read-Shared (Join-Path $logDir 'spike.log')
if ($jsonl) {
    $lines = ($jsonl -split "`r?`n") | Where-Object { $_ -ne '' }
    Write-Host ""
    Write-Host "=============================================================="
    Write-Host "EVENT LOG (spike.log) - last $Tail"
    Write-Host "=============================================================="
    foreach ($l in ($lines | Select-Object -Last $Tail)) {
        try {
            $e = $l | ConvertFrom-Json
            $loc = ([DateTimeOffset]::Parse($e.ts)).ToLocalTime().ToString('HH:mm:ss')
            switch ($e.event) {
                'Write'       { Write-Host ("  $loc  WRITE       hwnd=$($e.hwnd) app=$($e.app) -> '$($e.target)' [$($e.strategy)]") }
                'WriteFailed' { Write-Host ("  $loc  WRITE-FAIL  hwnd=$($e.hwnd) app=$($e.app) -> '$($e.target)' reason=$($e.reason)") }
                'DialogDetected' { Write-Host ("  $loc  DIALOG-OPEN hwnd=$($e.hwnd) app=$($e.app)") }
                'Read'        { Write-Host ("  $loc  READ        hwnd=$($e.hwnd) app=$($e.app) path='$($e.path)' [$($e.strategy)]") }
                default {
                    $msg = $e.message
                    if (-not $msg) { $msg = ($e | ConvertTo-Json -Compress -Depth 4) }
                    Write-Host ("  $loc  $($e.event): $msg")
                }
            }
        } catch { Write-Host "  <unparsed> $l" }
    }
}

# ------------------------------------------------------------- tracing log
if (-not $SkipTracing) {
    $today = (Get-Date).ToString('yyyy-MM-dd')
    $t1 = Read-Shared (Join-Path $logDir "spike.log.$today")
    if (-not $t1) { $t1 = Read-Shared (Join-Path $logDir 'spike.log') }
    if ($t1) {
        $tl = ($t1 -split "`r?`n") | Where-Object { $_ -ne '' }
        Write-Host ""
        Write-Host "=============================================================="
        Write-Host "TRACING (spike.log.$today) - last $Tail   [reasons live here]"
        Write-Host "=============================================================="
        foreach ($l in ($tl | Select-Object -Last $Tail)) {
            $c = Strip-Ansi $l
            if ($c -match 'dialog lost|foreground|Injection|SetFolder|fallback|WinUI 3|H1 |H2 |F3 |write_path') {
                Write-Host ("  " + $c.Trim())
            }
        }
    }
}

Write-Host ""
Write-Host "=============================================================="
Write-Host "HOW TO READ THIS"
Write-Host "  WRITE      -> the dialog was navigated (success)"
Write-Host "  WRITE-FAIL -> unsupported_dialog_type (intentional, WinUI 3)"
Write-Host "                OR a dead hwnd left in the registry (BUG)"
Write-Host "  Compare:   /get_status active_dialogs  vs  LIVE #32770 count."
Write-Host "             A mismatch means the registry is holding dead handles."
Write-Host "  'is not foreground (current=HWND(0x0))' means GetForegroundWindow()"
Write-Host "             returned NULL, so SendInput (H1) can never run."
Write-Host "=============================================================="
