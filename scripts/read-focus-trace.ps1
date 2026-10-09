# ============================================================================
# read-focus-trace.ps1 - dump the extension's persistent diagnostic trace
# ============================================================================
# WHY THIS EXISTS
#   The live bug under investigation is a mojibake (UTF-8 bytes decoded as
#   GBK): Sigma FM hands the extension `鍔炲叕鏂鏂困潫` instead of `办公文件`.
#   Console logs vanish on restart, so the extension persists a bounded trace
#   into sigma.storage (customSettings key `__focus_sync_trace`), which lands in
#   user-extensions.json. This script reads that file back.
#
# WHY THE ENCODING VERDICT IS DECISIVE (not a guess)
#   The extension stores UTF-16 CODE UNITS AS NUMBERS, so the evidence cannot
#   itself be corrupted by the charset bug we are chasing. Here we rebuild the
#   string, then:
#       bytes = GBK.GetBytes(s)              # mojibake -> recovers ORIGINAL UTF-8 bytes
#       text  = UTF8_STRICT.GetString(bytes) # throws if bytes are not valid UTF-8
#   - Genuine text  (办公文件): GBK bytes are not valid UTF-8  -> THROWS -> NOT mojibake
#   - Mojibake text (鍔炲叕鏂鏂困潫): GBK bytes ARE the original UTF-8 -> decodes to 办公文件
#
# USAGE
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\read-focus-trace.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\read-focus-trace.ps1 -Tail 12
# ============================================================================

[CmdletBinding()]
param(
    [int]$Tail = 0,
    [string]$UserAppData = $env:APPDATA
)

$ErrorActionPreference = 'Stop'

$jsonPath = Join-Path $UserAppData 'com.sigma-file-manager.app\user-data\user-extensions.json'
if (-not (Test-Path $jsonPath)) {
    Write-Error "user-extensions.json not found: $jsonPath"
    exit 1
}

# Read as UTF-8 explicitly (no BOM expected).
$text = [System.IO.File]::ReadAllText($jsonPath, [System.Text.Encoding]::UTF8)
$json = $text | ConvertFrom-Json

$entry = $json.installedExtensions.'kizemo.focus-sync'
if (-not $entry) {
    Write-Error "kizemo.focus-sync not registered in $jsonPath"
    exit 1
}

$trace = $entry.settings.customSettings.__focus_sync_trace
if (-not $trace -or $trace.Count -eq 0) {
    Write-Host "TRACE EMPTY - extension has not written diagnostics yet."
    Write-Host "That means one of:"
    Write-Host "  a) activate() never ran        -> check DevTools Sources for the worker node"
    Write-Host "  b) sigma.storage is unavailable -> N02.storage.probe would say so"
    Write-Host "  c) Sigma FM has not been restarted since instrumentation was deployed"
    exit 0
}

$GBK = [System.Text.Encoding]::GetEncoding(936)
$UTF8_STRICT = New-Object System.Text.UTF8Encoding($false, $true)

function Test-Mojibake([int[]]$codes) {
    # Rebuild the string from code units, then attempt the GBK->UTF8 recovery.
    $s = -join ($codes | ForEach-Object { [char]$_ })
    try {
        $bytes = $GBK.GetBytes($s)
    } catch {
        return @{ IsMojibake = $false; Recovered = $null }
    }
    try {
        $recovered = $UTF8_STRICT.GetString($bytes)
    } catch {
        # GBK bytes are not valid UTF-8 => the string was genuine, not mojibake.
        return @{ IsMojibake = $false; Recovered = $null }
    }
    # A recovery that changes the text AND yields non-ASCII is a real recovery.
    $hasNonAscii = ($recovered.ToCharArray() | Where-Object { [int]$_ -gt 127 } | Measure-Object).Count -gt 0
    if ($recovered -ne $s -and $hasNonAscii) {
        return @{ IsMojibake = $true; Recovered = $recovered }
    }
    return @{ IsMojibake = $false; Recovered = $null }
}

function Show-Fp([object]$fp) {
    if ($null -eq $fp) { return '(none)' }
    if ($fp.PSObject.Properties.Name -contains 'type') {
        return "type=$($fp.type) value=$($fp.value)"
    }
    $s = -join ($fp.codes | ForEach-Object { [char]$_ })
    $m = Test-Mojibake $fp.codes
    $verdict = if ($fp.nonAscii) {
        if ($m.IsMojibake) { "*** MOJIBAKE -> recovered: $($m.Recovered)" }
        else { "non-ascii, genuine" }
    } else { "ascii" }
    $shown = if ($s.Length -gt 48) { $s.Substring(0, 48) + '...' } else { $s }
    return "len=$($fp.len) `"$shown`" [$verdict]"
}

Write-Host ""
Write-Host "=============================================================="
Write-Host "FOCUS-SYNC DIAGNOSTIC TRACE   ($($trace.Count) entries)"
Write-Host "source: $jsonPath"
Write-Host "=============================================================="

$show = $trace
if ($Tail -gt 0 -and $trace.Count -gt $Tail) {
    $show = $trace[($trace.Count - $Tail)..($trace.Count - 1)]
}

$lastSession = ''
foreach ($e in $show) {
    if ($e.s -ne $lastSession) {
        Write-Host ""
        Write-Host "--- session $($e.s) ---"
        $lastSession = $e.s
    }
    $line = "  #{0,-4} {1} {2}" -f $e.seq, $e.t, $e.node
    $extras = @()
    foreach ($p in $e.PSObject.Properties) {
        if ($p.Name -in @('seq', 't', 's', 'node')) { continue }
        if ($p.Name -match 'Fp$|fp$') { $extras += ("{0}: {1}" -f $p.Name, (Show-Fp $p.Value)) }
        else { $extras += ("{0}: {1}" -f $p.Name, $p.Value) }
    }
    if ($extras.Count -gt 0) { $line += "  |  " + ($extras -join '  |  ') }
    Write-Host $line
}

Write-Host ""
Write-Host "=============================================================="
Write-Host "NODE LEGEND"
Write-Host "  N01 activate.start   extension activate() entered"
Write-Host "  N02 storage.probe    is sigma.storage usable?"
Write-Host "  N03 health.req       GET /health attempted"
Write-Host "  N04 health.resp/err  sidecar answered?"
Write-Host "  N05 path.raw         Sigma FM onPathChange payload  <- KEY NODE"
Write-Host "  N06 push.normalized  after normalizePath()"
Write-Host "  N07 push.req/skip    about to POST /set_path"
Write-Host "  N08 push.resp        sidecar HTTP status"
Write-Host "  N09 push.err         transport failure"
Write-Host "  N10 ctx.initialPath  getCurrentPath() at startup"
Write-Host "  N11 deactivate       extension shutting down"
Write-Host "=============================================================="
