# 安装向导控件枚举脚本（2026-10-11）
# 用途：验证 MUI 安装向导的控件状态与「下一步」是否可用
#
# 为什么必须枚举控件：安装器在很多失败模式下依然 exit 0，
# 退出码不能作为「向导可用」的证据。id=1 是「下一步」/「安装」按钮，
# enabled=False 是确定结论，比肉眼截图更硬。

function Initialize-EnumWin {
    # Add-Type 必须在本函数作用域内执行：
    # dot-source 整个脚本时，顶层 Add-Type 定义的类型在调用脚本的会话里不一定可见，
    # 表现为 `EnumWin::Collect` 被当成 cmdlet 找不到。
    if(-not ('EnumWin' -as [type])){
        Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class EnumWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr p, EnumProc cb, IntPtr l);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);

    public static string Text(IntPtr h){ var sb=new StringBuilder(512); GetWindowTextW(h,sb,sb.Capacity); return sb.ToString(); }
    public static string Cls(IntPtr h){ var sb=new StringBuilder(256); GetClassNameW(h,sb,sb.Capacity); return sb.ToString(); }

    public static IntPtr[] Collect(IntPtr parent){
        var res = new System.Collections.Generic.List<IntPtr>();
        EnumChildWindows(parent, (h,l)=>{ res.Add(h); return true; }, IntPtr.Zero);
        return res.ToArray();
    }
}
'@
    }
}

function Show-InstallPage {
    param([IntPtr]$Hwnd, [string]$Label)
    Initialize-EnumWin
    Write-Output "--- $Label  (hwnd=$Hwnd) ---"
    $ids = [EnumWin]::Collect($Hwnd)
    Write-Output "子控件数: $($ids.Count)"
    $rows = foreach($h in $ids){
        [PSCustomObject]@{
            Id     = [EnumWin]::GetDlgCtrlID($h)
            Class  = [EnumWin]::Cls($h)
            Text   = (([EnumWin]::Text($h)) -replace "`r|`n",' ').Trim()
            Enabled= [EnumWin]::IsWindowEnabled($h)
            Visible= [EnumWin]::IsWindowVisible($h)
        }
    }
    $rows | Where-Object { $_.Text -ne '' -or $_.Id -eq 1 } | Format-Table -AutoSize
    $next = $rows | Where-Object { $_.Id -eq 1 } | Select-Object -First 1
    if($next){
        Write-Output ">>> id=1「下一步/安装」: enabled=$($next.Enabled)  text='$($next.Text)'"
    } else {
        Write-Output '>>> 未找到 id=1 的按钮'
    }
    return $rows
}

function Click-Ctrl {
    param([IntPtr]$Hwnd, [int]$CtrlId)
    [void][EnumWin]::SendMessageW($Hwnd, 0x00F5, [IntPtr]$CtrlId, [IntPtr]::Zero)   # BM_CLICK
}
