param([string]$outFile)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Cap {
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int n);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT r);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, int f);
    public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
$hwnd = [IntPtr]4987410
[Cap]::ShowWindow($hwnd, 9); [Cap]::SetForegroundWindow($hwnd)
Start-Sleep -Milliseconds 300
$r = New-Object Cap+RECT; [Cap]::GetWindowRect($hwnd, [ref]$r)
$w = $r.Right-$r.Left; $h = $r.Bottom-$r.Top
$bmp = New-Object System.Drawing.Bitmap($w, $h)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc(); [Cap]::PrintWindow($hwnd, $hdc, 2) | Out-Null
$g.ReleaseHdc($hdc); $g.Dispose()
$bmp.Save($outFile); $bmp.Dispose()
Write-Host "Captured ${w}x${h} -> $outFile"
