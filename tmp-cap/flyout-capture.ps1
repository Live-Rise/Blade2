# 弹出层取证：启动→就绪→点开指定按钮→截图（弹层是同进程 popup，屏幕拷贝可截）
param(
    [Parameter(Mandatory)][string]$ExePath,
    [Parameter(Mandatory)][string]$ProcName,
    [Parameter(Mandatory)][string]$OutPng,
    [string]$ButtonId = 'AgentModeButton',
    [int]$SettleSec = 12
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
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
'@
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$desktop = [System.Windows.Automation.AutomationElement]::RootElement
$allScope = [System.Windows.Automation.TreeScope]::Element -bor [System.Windows.Automation.TreeScope]::Descendants

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
if (-not $p) { Write-Host 'FAIL no window'; exit 1 }
Start-Sleep -Seconds $SettleSec

# 抢前台（物理点标题栏中央）
$r = New-Object Native.W32+RECT
[Native.W32]::GetWindowRect([IntPtr]$p.MainWindowHandle, [ref]$r) | Out-Null
[Native.W32]::SetCursorPos(([int](($r.Left + $r.Right) / 2)), ($r.Top + 12))
Start-Sleep -Milliseconds 120
[Native.W32]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); [Native.W32]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 800

# 找按钮并点开
$all = $desktop.FindAll($allScope, [System.Windows.Automation.Condition]::TrueCondition)
$hit = $null
foreach ($e in $all) {
    try { if ($e.Current.ProcessId -eq $p.Id -and $e.Current.AutomationId -eq $ButtonId) { $hit = $e; break } } catch { }
}
if (-not $hit) { Write-Host "FAIL $ButtonId not found"; Get-Process $ProcName -ErrorAction SilentlyContinue | Stop-Process -Force; exit 1 }
$b = $hit.Current.BoundingRectangle
[Native.W32]::SetCursorPos([int](($b.Left + $b.Right) / 2), [int](($b.Top + $b.Bottom) / 2))
Start-Sleep -Milliseconds 150
[Native.W32]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); [Native.W32]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 900

# 截全窗
$r2 = New-Object Native.W32+RECT
[Native.W32]::GetWindowRect([IntPtr]$p.MainWindowHandle, [ref]$r2) | Out-Null
$w = $r2.Right - $r2.Left; $h = $r2.Bottom - $r2.Top
$bmp = New-Object System.Drawing.Bitmap($w, $h)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r2.Left, $r2.Top, 0, 0, $bmp.Size)
$bmp.Save($OutPng, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Host "saved: $OutPng"

Get-Process $ProcName -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
