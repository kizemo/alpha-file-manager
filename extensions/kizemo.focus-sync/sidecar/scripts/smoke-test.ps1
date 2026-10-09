# Smoke test: start spike, send get_status, send quit.
# Uses PowerShell TcpClient (no nc.exe needed).

param(
    [int]$Port = 37421,
    [string]$SpikeExe = "$PSScriptRoot\..\target\release\spike.exe"
)

$ErrorActionPreference = "Stop"
$LogPath = Join-Path $env:TEMP "spike-smoke-$(Get-Date -Format 'yyyyMMddHHmmss').log"

Write-Host "Starting spike at $SpikeExe on port $Port"
Write-Host "Logging to $LogPath"

$spikeProc = Start-Process -FilePath $SpikeExe `
    -ArgumentList "--port", $Port `
    -RedirectStandardOutput $LogPath `
    -RedirectStandardError "$LogPath.err" `
    -PassThru -NoNewWindow

Start-Sleep -Seconds 2

function Send-Cmd($cmdObj) {
    $json = $cmdObj | ConvertTo-Json -Compress
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($json + "`n")
    $client = New-Object System.Net.Sockets.TcpClient("127.0.0.1", $Port)
    $client.ReceiveTimeout = 3000
    $client.SendTimeout = 3000
    $stream = $client.GetStream()
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.Flush()
    Start-Sleep -Milliseconds 200
    $reader = New-Object System.IO.StreamReader($stream)
    $response = $reader.ReadLine()
    $client.Close()
    return $response
}

try {
    Write-Host "`n=== get_status ==="
    $resp = Send-Cmd @{ cmd = "get_status" }
    Write-Host "Response: $resp"

    Write-Host "`n=== set_path ==="
    $resp = Send-Cmd @{ cmd = "set_path"; path = "C:\Users\Public\Documents" }
    Write-Host "Response: $resp"

    Write-Host "`n=== get_status (after set_path) ==="
    $resp = Send-Cmd @{ cmd = "get_status" }
    Write-Host "Response: $resp"

    Write-Host "`n=== quit ==="
    $resp = Send-Cmd @{ cmd = "quit" }
    Write-Host "Response: $resp"
} catch {
    Write-Host "ERROR: $_"
} finally {
    Start-Sleep -Seconds 1
    if (-not $spikeProc.HasExited) {
        $spikeProc.Kill()
    }
}

Write-Host "`n=== spike stdout ==="
Get-Content $LogPath -ErrorAction SilentlyContinue
Write-Host "`n=== exit OK ==="