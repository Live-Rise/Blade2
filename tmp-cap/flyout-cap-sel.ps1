# 弹出态证据采集（flyout-cap-agent.ps1 的参数化版）：
# 暖场点击标题栏 → UIA 找触发钮（打印矩形）→ 物理点击 → dump UIA 验证弹层行
# → 直接 CopyFromScreen（不做 SetForegroundWindow / 挪鼠标，排除焦点扰动）→ Esc → 杀进程。
# 用法：pwsh -File flyout-cap-sel.ps1 -ExePath <exe> -ProcName <proc> -Out <png> -ButtonId AgentModeButton -RowPrefix AgentMode_
param(
    [Parameter(Mandatory)][string]$ExePath,
    [Parameter(Mandatory)][string]$ProcName,
    [Parameter(Mandatory)][string]$Out,
    [Parameter(Mandatory)][string]$ButtonId,
    [Parameter(Mandatory)][string]$RowPrefix
)
$ErrorActionPreference = 'Continue'
Add-Type -Namespace Native -Name Dpi -MemberDefinition '[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();'
[Native.Dpi]::SetProcessDPIAware() | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type -Namespace Native -Name W32 -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
[DllImport("user32.dll")] public static extern void SetCursorPos(int x, int y);
[DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
[DllImport("user32.dll")] public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, UIntPtr dwExtraInfo);
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
'@
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$desktop = [System.Windows.Automation.AutomationElement]::RootElement
$allScope = [System.Windows.Automation.TreeScope]::Element -bor [System.Windows.Automation.TreeScope]::Descendants

function ClickCenter($rect) {
    $cx = [int](($rect.Left + $rect.Right) / 2); $cy = [int](($rect.Top + $rect.Bottom) / 2)
    [Native.W32]::SetCursorPos($cx, $cy)
    Start-Sleep -Milliseconds 150
    [Native.W32]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [Native.W32]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
}

Get-Process $ProcName -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500
Start-Process $ExePath
$p = $null
$deadline = (Get-Date).AddSeconds(30)
while ((Get-Date) -lt $deadline) {
    $p = Get-Process $ProcName -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
    if ($p) { break }
    Start-Sleep -Milliseconds 300
}
if (-not $p) { Write-Host 'FAIL: no window'; exit 1 }
Write-Host "window pid=$($p.Id)"
Start-Sleep -Seconds 12
[Native.W32]::SetForegroundWindow([IntPtr]$p.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 600
# 暖场：标题栏空白处点一下，确保窗口吃掉「激活点击」这一发
$wr = New-Object Native.W32+RECT
[Native.W32]::GetWindowRect([IntPtr]$p.MainWindowHandle, [ref]$wr) | Out-Null
[Native.W32]::SetCursorPos([int](($wr.Left + $wr.Right) / 2), $wr.Top + 12) | Out-Null
Start-Sleep -Milliseconds 150
[Native.W32]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
[Native.W32]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 800

$win = $null
foreach ($e in $desktop.FindAll($allScope, [System.Windows.Automation.Condition]::TrueCondition)) {
    try { if ($e.Current.ProcessId -eq $p.Id -and $e.Current.ControlType.ProgrammaticName -eq 'ControlType.Window') { $win = $e; break } } catch { }
}
if (-not $win) { Write-Host 'FAIL: no UIA window'; exit 1 }

$target = $null
function Walk($e) {
    try {
        $c = $e.Current
        if ($c.AutomationId -eq $ButtonId) { $script:target = $e; return }
    } catch { return }
    foreach ($k in $e.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)) { Walk $k; if ($script:target) { return } }
}
Walk $win
if (-not $target) { Write-Host "FAIL: $ButtonId not found"; exit 1 }
$tr = $target.Current.BoundingRectangle
Write-Host ("{0} rect: {1:0.#},{2:0.#} {3:0.#}x{4:0.#}" -f $ButtonId, $tr.X, $tr.Y, $tr.Width, $tr.Height)

ClickCenter($tr)
Start-Sleep -Milliseconds 1800

# 验证弹层行进了 UIA 树
$found = @()
foreach ($e in $win.FindAll($allScope, [System.Windows.Automation.Condition]::TrueCondition)) {
    try { $aid = $e.Current.AutomationId; if ($aid -like "$RowPrefix*") { $found += "$aid|$($e.Current.Name)" } } catch { }
}
Write-Host ("flyout rows in UIA: " + ($found -join ' ; '))

$r = New-Object Native.W32+RECT
[Native.W32]::GetWindowRect([IntPtr]$p.MainWindowHandle, [ref]$r) | Out-Null
$w2 = $r.Right - $r.Left; $h2 = $r.Bottom - $r.Top
$bmp = New-Object System.Drawing.Bitmap($w2, $h2)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Host "saved $Out"

[Native.W32]::keybd_event(0x1B, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 60
[Native.W32]::keybd_event(0x1B, 0, 2, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 500
Get-Process $ProcName -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Write-Host 'done'
