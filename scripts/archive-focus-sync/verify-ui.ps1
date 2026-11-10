# Alpha File Manager GUI 验证脚本（2026-10-11）
# 用途：验证三处 UI 改动在真实 exe 上生效
#   1. 启动默认最大化
#   2. 目录树默认宽度占比调宽
#   3. 左下角磁盘图标叠加盘符
#
# 用 Win32 API 读真实窗口状态，而不是靠截图目测 —— 截图无法证明「确实最大化」。

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;

public static class Win32 {
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
    [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr hWnd, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hWnd, out RECT lpRect);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }

    public static string GetTitle(IntPtr h) {
        var sb = new StringBuilder(512);
        GetWindowTextW(h, sb, sb.Capacity);
        return sb.ToString();
    }
}
'@ -ErrorAction SilentlyContinue

function Show-Report {
    param($p)
    Write-Output "=== 进程 pid=$($p.Id) ==="
    $h = $p.MainWindowHandle
    Write-Output "MainWindowHandle = $h"
    if($h -eq [IntPtr]::Zero){ Write-Output "❌ 没有主窗口句柄"; return }

    $isZoomed = [Win32]::IsZoomed($h)
    $isIconic = [Win32]::IsIconic($h)
    $vis = [Win32]::IsWindowVisible($h)
    $r = New-Object Win32+RECT
    [void][Win32]::GetWindowRect($h, [ref]$r)
    $c = New-Object Win32+RECT
    [void][Win32]::GetClientRect($h, [ref]$c)
    $w = $r.Right - $r.Left; $hgt = $r.Bottom - $r.Top
    $screen = [System.Windows.Forms.SystemInformation]::VirtualScreen

    Write-Output "标题       : $([Win32]::GetTitle($h))"
    Write-Output "可见       : $vis"
    Write-Output "最小化     : $isIconic"
    Write-Output "IsZoomed(最大化) : $isZoomed   <-- 改动1 的判据"
    Write-Output "窗口尺寸   : ${w} x ${hgt}"
    Write-Output "客户区     : $($c.Right - $c.Left) x $($c.Bottom - $c.Top)"
    Write-Output "虚拟屏     : $($screen.Width) x $($screen.Height) @ $($screen.X),$($screen.Y)"
    Write-Output "占屏比例   : $([math]::Round(100*$w/$screen.Width,1))% x $([math]::Round(100*$hgt/$screen.Height,1))%"

    $isMaximized = $isZoomed -and ($w -ge $screen.Width - 40) -and ($hgt -ge $screen.Height - 80)
    Write-Output ""
    Write-Output $(if($isMaximized){ "✅ 判定：窗口已最大化（改动1 生效）" } else { "❌ 判定：窗口未最大化（改动1 未生效）" })
    return $isMaximized
}
