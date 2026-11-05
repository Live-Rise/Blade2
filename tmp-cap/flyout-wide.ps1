# capture window rect + 500px right extension while model flyout is open. ASCII only.
param(
    [Parameter(Mandatory)][string]$ExePath,
    [Parameter(Mandatory)][string]$ProcName,
    [Parameter(Mandatory)][string]$Out,
    [int]$SettleSec = 12
)
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -Namespace N -Name D -MemberDefinition '[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();'
[N.D]::SetProcessDPIAware() | Out-Null
Add-Type -Namespace N -Name W -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
[DllImport("user32.dll")] public static extern void SetCursorPos(int x, int y);
[DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
'@
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$desktop = [System.Windows.Automation.AutomationElement]::RootElement
$scope = [System.Windows.Automation.TreeScope]::Element -bor [System.Windows.Automation.TreeScope]::Descendants

$wa = [System.Windows.Forms.Screen]::PrimaryScreen
Write-Host ("primary bounds {0},{1} {2}x{3}  work {4},{5} {6}x{7}" -f $wa.Bounds.X, $wa.Bounds.Y, $wa.Bounds.Width, $wa.Bounds.Height, $wa.WorkingArea.X, $wa.WorkingArea.Y, $wa.WorkingArea.Width, $wa.WorkingArea.Height)
$all = [System.Windows.Forms.Screen]::AllScreens
foreach ($s in $all) { Write-Host ("screen {0}: bounds {1},{2} {3}x{4} work {5}x{6}" -f $s.DeviceName, $s.Bounds.X, $s.Bounds.Y, $s.Bounds.Width, $s.Bounds.Height, $s.WorkingArea.Width, $s.WorkingArea.Height) }

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
if (-not $p) { Write-Host "FAIL: no window"; exit 1 }
Start-Sleep -Seconds $SettleSec
[N.W]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 400

$win = $null
foreach ($e in $desktop.FindAll($scope, [System.Windows.Automation.Condition]::TrueCondition)) {
    try { if ($e.Current.ProcessId -eq $p.Id -and $e.Current.ControlType.ProgrammaticName -eq 'ControlType.Window') { $win = $e; break } } catch { }
}
if (-not $win) { Write-Host 'FAIL: no UIA window'; exit 1 }
$idx = @{}
function Walk($e) {
    try {
        $c = $e.Current
        $aid = $c.AutomationId
        if ($aid -and -not $idx.ContainsKey($aid)) { $idx[$aid] = $e }
    } catch { return }
    $kids = $e.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($k in $kids) { Walk $k }
}
Walk $win
if (-not $idx.ContainsKey('ModelButton')) { Write-Host 'FAIL: no ModelButton'; exit 1 }
$r = New-Object N.W+RECT
[N.W]::GetWindowRect($p.MainWindowHandle, [ref]$r) | Out-Null
Write-Host ("window rect {0},{1} {2}x{3}" -f $r.Left, $r.Top, ($r.Right - $r.Left), ($r.Bottom - $r.Top))
$b = $idx['ModelButton'].Current.BoundingRectangle
$cx = [int](($b.Left + $b.Right) / 2); $cy = [int](($b.Top + $b.Bottom) / 2)
[N.W]::SetCursorPos($cx, $cy)
Start-Sleep -Milliseconds 150
[N.W]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
[N.W]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 1500
$wide = 500
$bmp = New-Object System.Drawing.Bitmap(($r.Right - $r.Left + $wide), ($r.Bottom - $r.Top))
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Host "saved $Out"
Get-Process $ProcName -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
