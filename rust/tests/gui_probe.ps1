param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [string]$TitlePattern = '.',
    [string]$Out = '',
    [int]$TimeoutSec = 25,
    [double[]]$Click = @(),
    [string]$ClickRel = '',
    [string]$Type = '',
    [switch]$Kill
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public struct WndEnum {
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr hWnd);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int n);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
    [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c, string n);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
'@

function Find-Windows([uint32]$procId, [string]$pattern) {
    $found = New-Object System.Collections.Generic.List[object]
    $cb = [WndEnum+EnumWindowsProc] {
        param($h, $l)
        $vp = [uint32]0
        [void][WndEnum]::GetWindowThreadProcessId($h, [ref]$vp)
        if ($vp -eq $procId -and [WndEnum]::IsWindowVisible($h)) {
            $len = [WndEnum]::GetWindowTextLength($h)
            if ($len -gt 0) {
                $sb = New-Object System.Text.StringBuilder($len + 2)
                [void][WndEnum]::GetWindowText($h, $sb, $sb.Capacity)
                if ($sb.ToString() -match $pattern) { $found.Add(@{ h = $h; t = $sb.ToString() }) }
            }
        }
        return $true
    }
    [void][WndEnum]::EnumWindows($cb, [IntPtr]::Zero)
    return $found
}

$pi = Start-Process -FilePath $Exe -PassThru
Write-Output ("PID=" + $pi.Id)
$deadline = (Get-Date).AddSeconds($TimeoutSec)
$wins = @()
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 500
    if ($pi.HasExited) { Write-Output ("EXITED code=" + $pi.ExitCode); break }
    $wins = @(Find-Windows -procId ([uint32]$pi.Id) -pattern $TitlePattern)
    if ($wins.Count -gt 0) { break }
}
if ($wins.Count -eq 0) {
    Write-Output 'RESULT=no-window'
    if (-not $pi.HasExited) { Stop-Process -Id $pi.Id -Force }
    exit 2
}
$w = $wins[0]
Write-Output ("WINDOW=" + $w.t)
[void][WndEnum]::ShowWindow($w.h, 9)
[void][WndEnum]::SetForegroundWindow($w.h)
Start-Sleep -Milliseconds 800
$r = New-Object WndEnum+RECT
[void][WndEnum]::GetWindowRect($w.h, [ref]$r)
Write-Output ("RECT=" + $r.Left + ',' + $r.Top + ',' + ($r.Right - $r.Left) + 'x' + ($r.Bottom - $r.Top))

if ($ClickRel -ne '') {
    $p = $ClickRel.Split(',')
    $Click = @([int]($r.Left + [int]$p[0]), [int]($r.Top + [int]$p[1]))
}
if ($Click.Count -eq 2) {
    [void][WndEnum]::SetCursorPos([int]$Click[0], [int]$Click[1])
    Start-Sleep -Milliseconds 250
    [WndEnum]::mouse_event(2, 0, 0, 0, [IntPtr]::Zero)
    [WndEnum]::mouse_event(4, 0, 0, 0, [IntPtr]::Zero)
    Write-Output ("CLICKED=" + $Click[0] + ',' + $Click[1])
    Start-Sleep -Milliseconds 900
    [void][WndEnum]::GetWindowRect($w.h, [ref]$r)
}if ($Type -ne '') {
    Set-Clipboard -Value $Type
    Start-Sleep -Milliseconds 200
    [System.Windows.Forms.SendKeys]::SendWait('^v')
    Start-Sleep -Milliseconds 900
}

if ($Out -ne '') {
    $bmp = New-Object System.Drawing.Bitmap(($r.Right - $r.Left), ($r.Bottom - $r.Top))
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
    $g.Dispose()
    $bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Output ("SHOT=" + $Out)
}
Write-Output 'RESULT=ok'
if ($Kill) { Stop-Process -Id $pi.Id -Force; Write-Output 'KILLED' }
