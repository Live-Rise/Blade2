# dump UIA rects of AgentModeButton / ModelButton / window. ASCII only.
param(
    [Parameter(Mandatory)][string]$ExePath,
    [Parameter(Mandatory)][string]$ProcName,
    [int]$SettleSec = 12
)
Add-Type -Namespace N -Name D -MemberDefinition '[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();'
[N.D]::SetProcessDPIAware() | Out-Null
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$desktop = [System.Windows.Automation.AutomationElement]::RootElement
$scope = [System.Windows.Automation.TreeScope]::Element -bor [System.Windows.Automation.TreeScope]::Descendants

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
$win = $null
foreach ($e in $desktop.FindAll($scope, [System.Windows.Automation.Condition]::TrueCondition)) {
    try { if ($e.Current.ProcessId -eq $p.Id -and $e.Current.ControlType.ProgrammaticName -eq 'ControlType.Window') { $win = $e; break } } catch { }
}
if (-not $win) { Write-Host 'FAIL: no UIA window'; exit 1 }
$r = $win.Current.BoundingRectangle
Write-Host ("WINDOW rect {0},{1} {2}x{3}" -f $r.X, $r.Y, $r.Width, $r.Height)
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
foreach ($t in @('AgentModeButton', 'ModelButton', 'PermissionButton', 'ComposerAddButton', 'SendButton')) {
    if ($idx.ContainsKey($t)) {
        $b = $idx[$t].Current.BoundingRectangle
        Write-Host ("{0} rect {1},{2} {3}x{4}" -f $t, $b.X, $b.Y, $b.Width, $b.Height)
    } else {
        Write-Host "$t MISSING"
    }
}
Get-Process $ProcName -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
