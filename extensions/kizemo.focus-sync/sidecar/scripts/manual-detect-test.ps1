# Trigger a standard Win32 OpenFileDialog via PowerShell, then start spike in background
# to see if spike's monitor detects it.

# Step 1: Start spike in background.
$spikeProc = Start-Process -FilePath "F:\soft\00selfmade\filemanager\sigma-listary-spike\target\release\spike.exe" `
    -ArgumentList "--port", "37423", "--initial-path", "C:\Users\Public" `
    -RedirectStandardOutput "C:\Users\Duanyi\AppData\Local\Temp\spike-manual-test.log" `
    -RedirectStandardError "C:\Users\Duanyi\AppData\Local\Temp\spike-manual-test.log.err" `
    -PassThru -NoNewWindow

Start-Sleep -Seconds 2

# Step 2: Show a real Win32 OpenFileDialog (this is a standard #32770 dialog).
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Title = "Open File - Spike Detection Test"
$dialog.InitialDirectory = "C:\Users\Public"
Write-Host "Showing Win32 OpenFileDialog..."
$null = $dialog.ShowDialog()  # This blocks until user closes the dialog

# Step 3: After dialog closes, check spike log.
Start-Sleep -Seconds 1
Write-Host "`n=== spike stdout ==="
Get-Content "C:\Users\Duanyi\AppData\Local\Temp\spike-manual-test.log" -ErrorAction SilentlyContinue

# Step 4: Send quit.
$bytes = [System.Text.Encoding]::UTF8.GetBytes('{"cmd":"quit"}' + "`n")
$client = New-Object System.Net.Sockets.TcpClient("127.0.0.1", 37423)
$client.SendTimeout = 1000
$stream = $client.GetStream()
try {
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.Flush()
} catch {}
$client.Close()

Start-Sleep -Seconds 1
if (-not $spikeProc.HasExited) { $spikeProc.Kill() }
Write-Host "`n=== spike process killed ==="