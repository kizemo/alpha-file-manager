$ErrorActionPreference = 'Stop'
$log = 'F:\soft\00selfmade\filemanager\scripts\build-10pct.log'
function Say([string]$m) {
    $line = ("[{0}] {1}" -f (Get-Date -Format 'HH:mm:ss'), $m)
    Add-Content -LiteralPath $log -Value $line -Encoding UTF8
    Write-Output $line
}

Set-Content -LiteralPath $log -Value '' -Encoding UTF8

$tc = 'C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin'
if (-not (Test-Path -LiteralPath "$tc\cargo.exe")) {
    Say "FATAL: cargo.exe not found at $tc"
    exit 3
}
[Environment]::SetEnvironmentVariable('PATH', "$tc;$env:PATH", 'Process')
Say ("cargo version: " + (& "$tc\cargo.exe" --version 2>&1))
Say ("node version:  " + (& node --version 2>&1))

Set-Location 'F:\soft\00selfmade\filemanager\sigma-file-manager'
Say 'npx tauri build start'
& npx tauri build 2>&1 | ForEach-Object {
    Add-Content -LiteralPath $log -Value $_ -Encoding UTF8
}
$code = $LASTEXITCODE
Say "tauri build exit=$code"

$setup = 'F:\soft\00selfmade\filemanager\sigma-file-manager\src-tauri\target\release\bundle\nsis\Alpha File Manager_2.2.2_x64-setup.exe'
if (Test-Path -LiteralPath $setup) {
    $fi = Get-Item -LiteralPath $setup
    $sha = (Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash.ToLower()
    Say ("setup bytes={0} mtime={1} sha256={2}" -f $fi.Length, $fi.LastWriteTime.ToString('yyyy-MM-dd HH:mm:ss'), $sha)
} else {
    Say "FATAL: setup missing"
}
Say 'DONE'
exit $code