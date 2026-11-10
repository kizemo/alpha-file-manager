$ErrorActionPreference = 'Stop'
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class W2 {
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr l);
  public delegate bool EnumWindowsProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr p, EnumWindowsProc cb, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint msg, IntPtr wp, IntPtr lp);
}
"@

function Get-Win([string]$titleLike) {
  $script:found = [IntPtr]::Zero
  $cb = [W2+EnumWindowsProc]{
    param($h, $l)
    if ([W2]::IsWindowVisible($h)) {
      $sb = New-Object Text.StringBuilder 512
      [void][W2]::GetWindowTextW($h, $sb, 512)
      if ($sb.ToString() -like $titleLike) { $script:found = $h; return $false }
    }
    return $true
  }
  [void][W2]::EnumWindows($cb, [IntPtr]::Zero)
  return $script:found
}

function Get-Ctrls([IntPtr]$parent) {
  $list = New-Object System.Collections.ArrayList
  $cb = [W2+EnumWindowsProc]{
    param($h, $l)
    $sb = New-Object Text.StringBuilder 512
    [void][W2]::GetWindowTextW($h, $sb, 512)
    $cn = New-Object Text.StringBuilder 256
    [void][W2]::GetClassNameW($h, $cn, 256)
    [void]$list.Add([pscustomobject]@{
      H=$h; Id=[W2]::GetDlgCtrlID($h); Text=$sb.ToString(); Class=$cn.ToString(); Enabled=[W2]::IsWindowEnabled($h)
    })
    return $true
  }
  [void][W2]::EnumChildWindows($parent, $cb, [IntPtr]::Zero)
  return $list
}

$deadline = (Get-Date).AddSeconds(420)
$done = $false
while ((Get-Date) -lt $deadline) {
    $hwnd = Get-Win '*Alpha File Manager*'
    if ($hwnd -eq [IntPtr]::Zero) { Write-Output 'wizard closed'; break }
    $ctrls = Get-Ctrls $hwnd
    $fin = $ctrls | Where-Object { $_.Text -like '*Finish*' -and $_.Enabled } | Select-Object -First 1
    if ($fin) {
        Write-Output ('Finish button: id=' + $fin.Id + ' text=' + $fin.Text)
        $statics = $ctrls | Where-Object { $_.Class -eq 'Static' -and $_.Text -and $_.Text.Trim() -ne '' }
        foreach ($s in $statics) { Write-Output ('  static: ' + $s.Text) }
        [void][W2]::SendMessageW($hwnd, 0x0111, [IntPtr]1, [IntPtr]::Zero)
        Write-Output 'clicked Finish'
        $done = $true
        break
    }
    $progress = $ctrls | Where-Object { $_.Id -eq 1006 -and $_.Text } | Select-Object -First 1
    if ($progress) { Write-Output ('  ... ' + $progress.Text) }
    Start-Sleep -Seconds 5
}
if (-not $done) { Write-Output 'Finish button never appeared within timeout' }
Write-Output 'DONE'