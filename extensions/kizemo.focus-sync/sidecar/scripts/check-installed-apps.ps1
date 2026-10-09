$targets = @('chrome', 'msedge', 'winword', 'code', 'DingTalk', 'Feishu', 'WeChat')
foreach ($t in $targets) {
    $proc = Get-Process -Name $t -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($proc) {
        Write-Host "RUNNING: $t (PID $($proc.Id))"
    } else {
        $cmd = Get-Command "$t.exe" -ErrorAction SilentlyContinue
        if ($cmd) {
            Write-Host "INSTALLED: $t -> $($cmd.Source)"
        } else {
            Write-Host "MISSING: $t"
        }
    }
}