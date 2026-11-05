# 视觉比对驱动 v2：前台+CopyFromScreen 截图（主干 PrintWindow 不支持），设置项坐标点击
param(
    [Parameter(Mandatory)][string]$ExePath,
    [Parameter(Mandatory)][string]$ProcName,
    [Parameter(Mandatory)][string]$OutDir,
    [int]$SettleSec = 20
)
$ErrorActionPreference = 'Continue'
$repo = "E:\Syncthing\DshWinUI"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

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

function Shot([int64]$Hwnd, [string]$Out) {
    [Native.W32]::SetForegroundWindow([IntPtr]$Hwnd) | Out-Null
    Start-Sleep -Milliseconds 400
    $r = New-Object Native.W32+RECT
    [Native.W32]::GetWindowRect([IntPtr]$Hwnd, [ref]$r) | Out-Null
    $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
    if ($w -le 0 -or $h -le 0) { Write-Host "bad rect ${w}x${h}"; return $false }
    # SetForegroundWindow 干不过用户在前台的窗口：物理点击标题栏中央抢前台（只激活，不碰控件）
    $tclick = @{ Left = $r.Left + [double]$w / 2 - 8; Top = $r.Top + 12; Right = $r.Left + [double]$w / 2 + 8; Bottom = $r.Top + 28 }
    ClickCenter($tclick)
    Start-Sleep -Milliseconds 900
    $bmp = New-Object System.Drawing.Bitmap($w, $h)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
    $bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Host "saved: $Out (${w}x${h})"
    return $true
}
function ClickCenter($rect) {
    $cx = [int](($rect.Left + $rect.Right) / 2); $cy = [int](($rect.Top + $rect.Bottom) / 2)
    [Native.W32]::SetCursorPos($cx, $cy)
    Start-Sleep -Milliseconds 150
    [Native.W32]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)  # LEFTDOWN
    [Native.W32]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)  # LEFTUP
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
if (-not $p) { Write-Host "FAIL: no window for $ProcName"; exit 1 }
Write-Host "window pid=$($p.Id) hwnd=$($p.MainWindowHandle)"
Start-Sleep -Seconds $SettleSec

Shot ([int64]$p.MainWindowHandle) "$OutDir\S0-default.png" | Out-Null

# 设置页：找 AutomationId=SettingsItem，拿 BoundingRectangle 坐标点击
$desktop = [System.Windows.Automation.AutomationElement]::RootElement
$all = $desktop.FindAll([System.Windows.Automation.TreeScope]::Element -bor [System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
$hit = $null
foreach ($e in $all) {
    try { if ($e.Current.ProcessId -eq $p.Id -and $e.Current.AutomationId -eq 'SettingsItem') { $hit = $e; break } } catch { }
}
if ($hit) {
    ClickCenter($hit.Current.BoundingRectangle)
    Start-Sleep -Seconds 3
    Shot ([int64]$p.MainWindowHandle) "$OutDir\S1-settings.png" | Out-Null
} else {
    Write-Host 'S1 SKIP: SettingsItem not found'
}

Get-Process $ProcName -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Write-Host "done: $OutDir"
