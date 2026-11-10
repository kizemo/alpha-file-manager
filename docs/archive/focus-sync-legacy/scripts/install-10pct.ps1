$ErrorActionPreference = 'Stop'
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class W {
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr l);
  public delegate bool EnumWindowsProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr p, EnumWindowsProc cb, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
  [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint msg, IntPtr wp, IntPtr lp);
}
"@

function Get-Win([string]$titleLike) {
  $found = [IntPtr]::Zero
  $cb = [W+EnumWindowsProc]{
    param($h, $l)
    if ([W]::IsWindowVisible($h)) {
      $sb = New-Object Text.StringBuilder 512
      [void][W]::GetWindowTextW($h, $sb, 512)
      $t = $sb.ToString()
      if ($t -like $titleLike) { $script:found = $h; return $false }
    }
    return $true
  }
  [void][W]::EnumWindows($cb, [IntPtr]::Zero)
  return $script:found
}

function Get-Ctrls([IntPtr]$parent) {
  $list = New-Object System.Collections.ArrayList
  $cb = [W+EnumWindowsProc]{
    param($h, $l)
    $sb = New-Object Text.StringBuilder 512
    [void][W]::GetWindowTextW($h, $sb, 512)
    $cn = New-Object Text.StringBuilder 256
    [void][W]::GetClassNameW($h, $cn, 256)
    [void]$list.Add([pscustomobject]@{
      H = $h; Id = [W]::GetDlgCtrlID($h); Text = $sb.ToString()
      Class = $cn.ToString(); Enabled = [W]::IsWindowEnabled($h)
    })
    return $true
  }
  [void][W]::EnumChildWindows($parent, $cb, [IntPtr]::Zero)
  return $list
}

$setup = 'F:\soft\00selfmade\filemanager\sigma-file-manager\src-tauri\target\release\bundle\nsis\Alpha File Manager_2.2.2_x64-setup.exe'

Write-Output "[1] launching installer (GUI, no /S)"
$p = Start-Process -FilePath $setup -PassThru
Write-Output ("    pid=" + $p.Id)

# 等向导窗口出现
$hwnd = [IntPtr]::Zero
for ($i = 0; $i -lt 40; $i++) {
    Start-Sleep -Milliseconds 500
    $hwnd = Get-Win '*Alpha File Manager*'
    if ($hwnd -ne [IntPtr]::Zero) { break }
}
if ($hwnd -eq [IntPtr]::Zero) { Write-Output "FATAL: wizard window not found"; exit 3 }
Write-Output ("[2] hwnd=0x" + $hwnd.ToString('x'))

for ($page = 1; $page -le 5; $page++) {
    Start-Sleep -Seconds 2
    $ctrls = Get-Ctrls $hwnd
    $btns = $ctrls | Where-Object { $_.Text -like '*Next*' -or $_.Text -like '*Install*' -or $_.Text -like '*Finish*' }
    Write-Output ("--- page " + $page + " controls ---")
    foreach ($c in $ctrls) {
        if ($c.Text -and $c.Text.Trim() -ne '') {
            Write-Output ("    id={0,-4} enabled={1,-5} [{2}] {3}" -f $c.Id, $c.Enabled, $c.Class, $c.Text)
        }
    }
    $btn = $btns | Where-Object { $_.Id -eq 1 } | Select-Object -First 1
    if (-not $btn) { Write-Output "    (no id=1 button; stopping)"; break }
    if (-not $btn.Enabled) { Write-Output ("    id=1 DISABLED on page " + $page); break }
    Write-Output ("    -> WM_COMMAND id=1 (" + $btn.Text + ")")
    [void][W]::SendMessageW($hwnd, 0x0111, [IntPtr]1, [IntPtr]::Zero)
    Start-Sleep -Seconds 2
}

Start-Sleep -Seconds 8
Write-Output "[3] installer exit code: $($p.ExitCode)"
$ctrls = Get-Ctrls $hwnd
foreach ($c in $ctrls) {
    if ($c.Text -and $c.Text.Trim() -ne '') {
        Write-Output ("    final id={0} [{1}] {2}" -f $c.Id, $c.Class, $c.Text)
    }
}
Write-Output "DONE"