# 全量采集：一次启动遍历 默认+设置12子页，每视图截图 + UIA 树 dump
# 用法：pwsh -File drive-capture-full.ps1 -ExePath <exe> -ProcName <proc> -OutDir <dir> [-Sections id,id,...]
param(
    [Parameter(Mandatory)][string]$ExePath,
    [Parameter(Mandatory)][string]$ProcName,
    [Parameter(Mandatory)][string]$OutDir,
    [string]$Sections = "general,personalization,models,plugins,skills,memory,pet,agent-presets,computer-control,browser-control,usage,about",
    [int]$SettleSec = 12,
    [int]$SectionSettleMs = 2500
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
$desktop = [System.Windows.Automation.AutomationElement]::RootElement
$allScope = [System.Windows.Automation.TreeScope]::Element -bor [System.Windows.Automation.TreeScope]::Descendants

function Shot([int64]$Hwnd, [string]$Out) {
    [Native.W32]::SetForegroundWindow([IntPtr]$Hwnd) | Out-Null
    Start-Sleep -Milliseconds 400
    $r = New-Object Native.W32+RECT
    [Native.W32]::GetWindowRect([IntPtr]$Hwnd, [ref]$r) | Out-Null
    $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
    if ($w -le 0 -or $h -le 0) { Write-Host "bad rect ${w}x${h}"; return $false }
    $tclick = @{ Left = $r.Left + [double]$w / 2 - 8; Top = $r.Top + 12; Right = $r.Left + [double]$w / 2 + 8; Bottom = $r.Top + 28 }
    ClickCenter($tclick)
    Start-Sleep -Milliseconds 900
    $bmp = New-Object System.Drawing.Bitmap($w, $h)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
    $bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    return $true
}
function ClickCenter($rect) {
    $cx = [int](($rect.Left + $rect.Right) / 2); $cy = [int](($rect.Top + $rect.Bottom) / 2)
    [Native.W32]::SetCursorPos($cx, $cy)
    Start-Sleep -Milliseconds 150
    [Native.W32]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
    [Native.W32]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
}
function FindByProcessIdAndAutomationId($procId, $autoId) {
    $all = $desktop.FindAll($allScope, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($e in $all) {
        try { if ($e.Current.ProcessId -eq $procId -and $e.Current.AutomationId -eq $autoId) { return $e } } catch { }
    }
    # 桌面级枚举可能撞缓存：进窗口子树再找一轮
    if ($script:winElem) {
        $sub = $script:winElem.FindAll($allScope, [System.Windows.Automation.Condition]::TrueCondition)
        foreach ($e in $sub) {
            try { if ($e.Current.AutomationId -eq $autoId) { return $e } } catch { }
        }
    }
    return $null
}
# UIA 树 dump：AutomationId|ControlType|Name（缩进表层级），功能完整性审查的结构证据
function DumpTree($windowElem, $outPath) {
    $lines = New-Object System.Collections.Generic.List[string]
    function Walk($e, $depth) {
        if ($depth -gt 14) { return }
        try {
            $c = $e.Current
            $lines.Add(("  " * $depth) + "$($c.AutomationId)|$($c.ControlType.ProgrammaticName -replace '^ControlType\.','')|$($c.Name)")
        } catch { return }
        $kids = $e.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)
        foreach ($k in $kids) { Walk $k ($depth + 1) }
    }
    Walk $windowElem 0
    [System.IO.File]::WriteAllLines($outPath, $lines)
}

# 一次性枚举窗口子树，按 AutomationId 建索引（每视图都全桌面 FindAll 太慢，600s 超时的根因）
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
Write-Host "window pid=$($p.Id)"
Start-Sleep -Seconds $SettleSec

$win = $null
foreach ($e in $desktop.FindAll($allScope, [System.Windows.Automation.Condition]::TrueCondition)) {
    try { if ($e.Current.ProcessId -eq $p.Id -and $e.Current.ControlType.ProgrammaticName -eq 'ControlType.Window') { $win = $e; break } } catch { }
}
if (-not $win) { Write-Host 'FAIL: no UIA window element'; exit 1 }
$script:winElem = $win

# S0 默认视图
Shot ([int64]$p.MainWindowHandle) "$OutDir\S0-default.png" | Out-Null
DumpTree $win "$OutDir\S0-default.uia.txt"
Write-Host 'S0 done'

# S2 会话内视图：点第一个**视口内可见**的会话项（虚拟化列表里离屏项矩形退化，点了白点）
$script:secIndex = BuildAutoIdIndex $win
$winRect = New-Object Native.W32+RECT
[Native.W32]::GetWindowRect([IntPtr]$p.MainWindowHandle, [ref]$winRect) | Out-Null
$firstSession = $null
foreach ($key in ($script:secIndex.Keys | Where-Object { $_ -like 'Session_*' -and $_ -notlike 'SessionMore_*' } | Sort-Object)) {
    try {
        $b = $script:secIndex[$key].Current.BoundingRectangle
        if ($b.Width -gt 10 -and $b.Height -gt 10 -and $b.Top -ge $winRect.Top -and $b.Bottom -le $winRect.Bottom) { $firstSession = $key; break }
    } catch { }
}
if ($firstSession -and $script:secIndex[$firstSession]) {
    try {
        ClickCenter($script:secIndex[$firstSession].Current.BoundingRectangle)
        Start-Sleep -Seconds 6
        Shot ([int64]$p.MainWindowHandle) "$OutDir\S2-session.png" | Out-Null
        DumpTree $win "$OutDir\S2-session.uia.txt"
        Write-Host "S2 session done ($firstSession)"
    } catch { Write-Host "S2 SKIP: click failed $_" }
} else { Write-Host 'S2 SKIP: no visible Session_* item found' }

# 进设置：分区项要等内核设置快照回来才建（RPC 往返），给足 8 秒
$settings = FindByProcessIdAndAutomationId $p.Id 'SettingsItem'
if ($settings) {
    ClickCenter($settings.Current.BoundingRectangle)
    Start-Sleep -Seconds 8
    Shot ([int64]$p.MainWindowHandle) "$OutDir\S1-settings.png" | Out-Null
    DumpTree $win "$OutDir\S1-settings.uia.txt"
    Write-Host 'S1 settings done'
    # 一次性建分区项索引（SettingsSection_* 挂在 Nav 下，进设置后不再重建）
    $script:secIndex = BuildAutoIdIndex $win
} else { Write-Host 'S1 SKIP: SettingsItem not found' }

# 12 子页逐个进（走缓存索引；缓存没有的退回逐个查找）
foreach ($sec in $Sections.Split(',')) {
    $item = $null
    if ($script:secIndex -and $script:secIndex.ContainsKey("SettingsSection_$sec")) {
        $item = $script:secIndex["SettingsSection_$sec"]
    } else {
        $item = FindByProcessIdAndAutomationId $p.Id "SettingsSection_$sec"
    }
    if ($item) {
        try { ClickCenter($item.Current.BoundingRectangle) } catch { Write-Host "P-$sec SKIP: stale reference"; continue }
        Start-Sleep -Milliseconds $SectionSettleMs
        Shot ([int64]$p.MainWindowHandle) "$OutDir\P-$sec.png" | Out-Null
        DumpTree $win "$OutDir\P-$sec.uia.txt"
        Write-Host "P-$sec done"
    } else { Write-Host "P-$sec SKIP: SettingsSection_$sec not found" }
}

Get-Process $ProcName -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Write-Host "done: $OutDir"
