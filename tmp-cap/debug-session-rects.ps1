param([string]$ExePath, [string]$ProcName)
$ErrorActionPreference = 'Continue'
Add-Type -Namespace Native -Name Dpi -MemberDefinition '[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();'
[Native.Dpi]::SetProcessDPIAware() | Out-Null
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$desktop = [System.Windows.Automation.AutomationElement]::RootElement
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
Start-Sleep -Seconds 12
$allScope = [System.Windows.Automation.TreeScope]::Element -bor [System.Windows.Automation.TreeScope]::Descendants
$win = $null
foreach ($e in $desktop.FindAll($allScope, [System.Windows.Automation.Condition]::TrueCondition)) {
    try { if ($e.Current.ProcessId -eq $p.Id -and $e.Current.ControlType.ProgrammaticName -eq 'ControlType.Window') { $win = $e; break } } catch { }
}
if (-not $win) { Write-Host 'no window elem'; exit 1 }

# 复刻采集脚本的 BuildAutoIdIndex
function BuildAutoIdIndex($windowElem) {
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
    Walk $windowElem
    return $idx
}
$idx = BuildAutoIdIndex $win
Write-Host "index size: $($idx.Count)"
$keys = $idx.Keys | Where-Object { $_ -like 'Session_*' -and $_ -notlike 'SessionMore_*' } | Sort-Object
Write-Host "session keys: $($keys.Count)"
Add-Type -AssemblyName System.Drawing
$wr = New-Object System.Drawing.Rectangle(0, 0, 0, 0) # placeholder
Add-Type -Namespace Native -Name W32b -MemberDefinition '[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }'
$winRect = New-Object Native.W32b+RECT
[Native.W32b]::GetWindowRect([IntPtr]$p.MainWindowHandle, [ref]$winRect) | Out-Null
Write-Host "winRect: $($winRect.Left),$($winRect.Top) $($winRect.Right-$winRect.Left)x$($winRect.Bottom-$winRect.Top)"
$n = 0
foreach ($k in $keys) {
    try {
        $b = $idx[$k].Current.BoundingRectangle
        $c = $idx[$k].Current
        Write-Host ("{0} rect=({1:N0},{2:N0} {3:N0}x{4:N0}) offscreen={5}" -f $k.Substring(0, [Math]::Min(28, $k.Length)), $b.X, $b.Y, $b.Width, $b.Height, $c.IsOffscreen)
        $n++
        if ($n -ge 5) { break }
    } catch { Write-Host "$k EX: $_" }
}
Get-Process $ProcName -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
