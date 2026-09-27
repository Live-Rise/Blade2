<#
  T27 UIA smoke probe — lookup by AutomationId (no coordinate / key injection).
  Usage: pwsh -NoProfile -ExecutionPolicy Bypass -File docs/gap-audit/uia-smoke.ps1 [-Kill]
#>
[CmdletBinding()]
param(
  [string]$Exe = 'E:\Syncthing\DshWinUI\bin\x64\Debug\net8.0-windows10.0.22621.0\win-x64\Blade2.exe',
  [string]$TitlePattern = 'Blade|DSH|dsh',
  [int]$TimeoutSec = 45,
  [int]$MaxElements = 400,
  [int]$LookupCap = 1500,
  [string]$OutLog = 'E:\Syncthing\DshWinUI\docs\gap-audit\uia-smoke-log.txt',
  [switch]$Kill
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$sb = New-Object System.Text.StringBuilder
function W([string]$m) {
  [void]$sb.AppendLine($m)
  [Console]::Out.WriteLine($m)
}

function Stop-ProcTree([int]$id) {
  $kids = @()
  try { $kids = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$id" -ErrorAction Stop) } catch { }
  foreach ($k in $kids) { Stop-ProcTree ([int]$k.ProcessId) }
  try { Stop-Process -Id $id -Force -ErrorAction Stop } catch { }
}

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public struct Win32UiaS {
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
}
'@

$AE       = [System.Windows.Automation.AutomationElement]
$TS       = [System.Windows.Automation.TreeScope]
$TRUECOND = [System.Windows.Automation.Condition]::TrueCondition
$WALKER   = [System.Windows.Automation.TreeWalker]::ControlViewWalker
$AIDPROP  = [System.Windows.Automation.AutomationElementIdentifiers]::AutomationIdProperty

function Get-ElInfo($el) {
  if ($null -eq $el) { return $null }
  try { return $el.Current } catch { return $null }
}

function Format-Name([string]$n) {
  if ($null -eq $n) { return '' }
  $n = $n -replace '[\r\n]+', ' \n '
  $n = $n -replace '\s+', ' '
  $n = $n.Trim()
  if ($n.Length -gt 140) { $n = $n.Substring(0, 137) + '...' }
  return $n
}

function Format-Rect($rc) {
  try {
    if ($rc.IsEmpty) { return 'empty' }
    return ('{0},{1} {2}x{3}' -f [int]$rc.X, [int]$rc.Y, [int]$rc.Width, [int]$rc.Height)
  } catch { return '?' }
}

function Get-Descendants($root, [int]$cap) {
  $list = New-Object System.Collections.Generic.List[object]
  try {
    $col = $root.FindAll($TS::Descendants, $TRUECOND)
    $n = [Math]::Min($col.Count, $cap)
    for ($i = 0; $i -lt $n; $i++) {
      $el = $col[$i]
      if ($null -ne $el) { $list.Add($el) }
    }
  } catch { W ('WARN=FindAll failed: ' + $_.Exception.Message) }
  return $list
}

function Get-AutomationId($el) {
  try { return [string]$el.GetCurrentPropertyValue($AIDPROP) } catch { return '' }
}

function Find-ByAutomationId($root, [string]$id, [int]$cap) {
  $hits = New-Object System.Collections.Generic.List[object]
  foreach ($el in (Get-Descendants $root $cap)) {
    $aid = Get-AutomationId $el
    if ($aid -eq $id) { $hits.Add($el) }
  }
  return $hits
}

function Get-EditValue($el) {
  try {
    $pat = $el.GetCurrentPattern([System.Windows.Automation.ValuePatternIdentifiers]::Pattern)
    if ($null -eq $pat) { return '' }
    return [string]([System.Windows.Automation.ValuePattern]$pat).Current.Value
  } catch { return '' }
}

function Get-IsEnabled($el) {
  $info = Get-ElInfo $el
  if ($null -eq $info) { return $null }
  try { return [bool]$info.IsEnabled } catch { return $null }
}

function Format-Err($e) {
  $cur = $e
  if ($null -ne $e.InnerException) { $cur = $e.InnerException }
  return ($cur.GetType().Name + ': ' + (($cur.Message -replace '\s+', ' ')).Trim())
}

function Invoke-Element($el) {
  $tries = @(
    @{ n = 'InvokePattern'; id = [System.Windows.Automation.InvokePatternIdentifiers]::Pattern },
    @{ n = 'TogglePattern'; id = [System.Windows.Automation.TogglePatternIdentifiers]::Pattern },
    @{ n = 'ExpandCollapsePattern'; id = [System.Windows.Automation.ExpandCollapsePatternIdentifiers]::Pattern },
    @{ n = 'SelectionItemPattern'; id = [System.Windows.Automation.SelectionItemPatternIdentifiers]::Pattern }
  )
  foreach ($t in $tries) {
    $pat = $null
    try { $pat = $el.GetCurrentPattern($t.id) } catch { $pat = $null }
    if ($null -eq $pat) { continue }
    try {
      if ($t.n -eq 'InvokePattern') { [void]([System.Windows.Automation.InvokePattern]$pat).Invoke() }
      elseif ($t.n -eq 'TogglePattern') { [void]([System.Windows.Automation.TogglePattern]$pat).Toggle() }
      elseif ($t.n -eq 'ExpandCollapsePattern') {
        $ep = [System.Windows.Automation.ExpandCollapsePattern]$pat
        if ($ep.Current.ExpandCollapseState -eq [System.Windows.Automation.ExpandCollapseState]::Expanded) { $ep.Collapse() }
        else { $ep.Expand() }
      }
      else { [void]([System.Windows.Automation.SelectionItemPattern]$pat).Select() }
      return @{ ok = $true; via = $t.n; why = '' }
    } catch {
      return @{ ok = $false; via = $t.n; why = (Format-Err $_.Exception) }
    }
  }
  return @{ ok = $false; via = ''; why = 'no invoke/toggle/expand pattern' }
}

function Set-EditText($el, [string]$val) {
  $pat = $null
  try { $pat = $el.GetCurrentPattern([System.Windows.Automation.ValuePatternIdentifiers]::Pattern) } catch { $pat = $null }
  if ($null -eq $pat) { return $false }
  $vp = [System.Windows.Automation.ValuePattern]$pat
  if ($vp.Current.IsReadOnly) { return $false }
  try { $vp.SetValue($val); return $true } catch { return $false }
}

function Dump-Tree($root, [int]$cap) {
  $count = 0
  $queue = New-Object System.Collections.Generic.Queue[object]
  $queue.Enqueue(@{ e = $root; d = 0 })
  while ($queue.Count -gt 0) {
    $cur = $queue.Dequeue()
    $el = $cur.e
    $depth = $cur.d
    $info = Get-ElInfo $el
    if ($null -ne $info) {
      $ct = $info.ControlType.ProgrammaticName -replace '^ControlType\.', ''
      $aid = Get-AutomationId $el
      W ('{0} | {1} | id={2} | {3} | en={4} | {5}' -f $depth, $ct, $aid, (Format-Name $info.Name), $info.IsEnabled, (Format-Rect $info.BoundingRectangle))
    }
    $count++
    if ($count -ge $cap) { W ('... capped at ' + $cap); break }
    try {
      $k = $WALKER.GetFirstChild($el)
      while ($null -ne $k) {
        $queue.Enqueue(@{ e = $k; d = $depth + 1 })
        try { $k = $WALKER.GetNextSibling($k) } catch { $k = $null }
      }
    } catch { }
  }
  W ('elements=' + $count)
}

# ---------------------------------------------------------------- start
if (-not (Test-Path -LiteralPath $Exe)) { W "RESULT=error exe not found: $Exe"; exit 3 }
$Exe = (Resolve-Path -LiteralPath $Exe).Path

# kill leftover Blade2
Get-Process -Name 'Blade2' -ErrorAction SilentlyContinue | ForEach-Object {
  W ('KILL-LEFTOVER pid=' + $_.Id)
  Stop-ProcTree ([int]$_.Id)
}
Start-Sleep -Milliseconds 500

$logDir = Join-Path $env:TEMP 't27-uia'
if (-not (Test-Path $logDir)) { New-Item -ItemType Directory -Force -Path $logDir | Out-Null }
$logOut = Join-Path $logDir 'blade2.out.txt'
$logErr = Join-Path $logDir 'blade2.err.txt'
Remove-Item -LiteralPath $logOut, $logErr -Force -ErrorAction SilentlyContinue

$pi = Start-Process -FilePath $Exe -PassThru -RedirectStandardOutput $logOut -RedirectStandardError $logErr
W ('PID=' + $pi.Id)

function Find-HwndByTitle([uint32]$procId, [string]$pattern) {
  $found = New-Object System.Collections.Generic.List[object]
  $cb = [Win32UiaS+EnumWindowsProc] {
    param($h, $l)
    $vp = [uint32]0
    [void][Win32UiaS]::GetWindowThreadProcessId($h, [ref]$vp)
    if ($vp -eq $procId -and [Win32UiaS]::IsWindowVisible($h)) {
      $len = [Win32UiaS]::GetWindowTextLength($h)
      if ($len -ge 0) {
        $sb2 = New-Object System.Text.StringBuilder([Math]::Max($len + 2, 8))
        [void][Win32UiaS]::GetWindowText($h, $sb2, $sb2.Capacity)
        $t = $sb2.ToString()
        if ($t -match $pattern -or $len -eq 0) { $found.Add(@{ h = $h; t = $t }) }
      }
    }
    return $true
  }
  [void][Win32UiaS]::EnumWindows($cb, [IntPtr]::Zero)
  return $found
}

$deadline = (Get-Date).AddSeconds($TimeoutSec)
$win = $null
$title = ''
$how = ''
while ((Get-Date) -lt $deadline) {
  if ($pi.HasExited) {
    W ('RESULT=error app exited code=' + $pi.ExitCode)
    W ('ERRFILE=' + (Get-Content -LiteralPath $logErr -Raw -ErrorAction SilentlyContinue))
    exit 4
  }
  try {
    $col = $AE::RootElement.FindAll($TS::Children, $TRUECOND)
    for ($i = 0; $i -lt $col.Count; $i++) {
      $el = $col[$i]
      $info = Get-ElInfo $el
      if ($null -eq $info) { continue }
      if ($info.ProcessId -ne $pi.Id) { continue }
      if ([string]$info.Name -match $TitlePattern -or $info.Name -eq '') {
        $win = $el; $title = [string]$info.Name; $how = 'uia-children'
        break
      }
    }
  } catch { }
  if ($null -ne $win) { break }
  $ws = @(Find-HwndByTitle -procId ([uint32]$pi.Id) -pattern $TitlePattern)
  if ($ws.Count -gt 0) {
    try {
      $win = $AE::FromHandle($ws[0].h)
      $title = $ws[0].t
      $how = 'win32+fromhandle'
    } catch { $win = $null }
    if ($null -ne $win) { break }
  }
  Start-Sleep -Milliseconds 500
}

if ($null -eq $win) {
  W 'RESULT=error no window matched'
  if ($Kill) { Stop-ProcTree ([int]$pi.Id); W 'KILLED' }
  exit 2
}
W ('WINDOW=' + $title)
W ('FOUND-VIA=' + $how)

# settle
$settleDeadline = (Get-Date).AddSeconds(20)
$elements = $null
while ((Get-Date) -lt $settleDeadline) {
  Start-Sleep -Milliseconds 400
  $elements = Get-Descendants $win $MaxElements
  if ($elements.Count -ge 8) { break }
}
W '--- TREE ---'
Dump-Tree $win $MaxElements
W '--- END TREE ---'

# ---------------------------------------------------------------- probes
$results = @()

function Probe-Exists([string]$id, [string]$label) {
  $hits = Find-ByAutomationId $win $id $LookupCap
  $n = $hits.Count
  $en = ''
  $nm = ''
  if ($n -gt 0) {
    $info = Get-ElInfo $hits[0]
    if ($null -ne $info) { $en = [string]$info.IsEnabled; $nm = Format-Name ([string]$info.Name) }
  }
  $status = if ($n -gt 0) { 'PASS' } else { 'FAIL' }
  W ("PROBE exists id=$id label=$label count=$n enabled=$en name=$nm => $status")
  $script:results += [pscustomobject]@{ Id = $id; Label = $label; Status = $status; Detail = "count=$n enabled=$en name=$nm" }
  return $hits
}

function Probe-Enabled([string]$id, [bool]$expectEnabled, [string]$label) {
  $hits = Find-ByAutomationId $win $id $LookupCap
  if ($hits.Count -eq 0) {
    W ("PROBE enabled id=$id label=$label => FAIL (not found)")
    $script:results += [pscustomobject]@{ Id = $id; Label = $label; Status = 'FAIL'; Detail = 'not found' }
    return
  }
  $en = Get-IsEnabled $hits[0]
  $ok = ($en -eq $expectEnabled)
  $status = if ($ok) { 'PASS' } else { 'FAIL' }
  W ("PROBE enabled id=$id label=$label expect=$expectEnabled actual=$en => $status")
  $script:results += [pscustomobject]@{ Id = $id; Label = $label; Status = $status; Detail = "expect=$expectEnabled actual=$en" }
}

function Probe-Invoke([string]$id, [string]$label, [string]$waitName = '') {
  $hits = Find-ByAutomationId $win $id $LookupCap
  if ($hits.Count -eq 0) {
    W ("PROBE invoke id=$id label=$label => FAIL (not found)")
    $script:results += [pscustomobject]@{ Id = $id; Label = $label; Status = 'FAIL'; Detail = 'not found' }
    return
  }
  $el = $hits[0]
  $en = Get-IsEnabled $el
  if ($en -eq $false) {
    W ("PROBE invoke id=$id label=$label => PARTIAL (exists but disabled)")
    $script:results += [pscustomobject]@{ Id = $id; Label = $label; Status = 'PARTIAL'; Detail = 'exists but disabled' }
    return
  }
  $r = Invoke-Element $el
  if ($r.ok) {
    Start-Sleep -Milliseconds 600
    W ("PROBE invoke id=$id label=$label via=$($r.via) => PASS")
    $script:results += [pscustomobject]@{ Id = $id; Label = $label; Status = 'PASS'; Detail = "invoked via $($r.via)" }
  } else {
    W ("PROBE invoke id=$id label=$label => FAIL reason=$($r.why)")
    $script:results += [pscustomobject]@{ Id = $id; Label = $label; Status = 'FAIL'; Detail = $r.why }
  }
}

# 1. File preview scaffolding
Probe-Exists 'PreviewRoot' 'file-preview-root'
Probe-Exists 'PreviewToolbar' 'file-preview-toolbar'
Probe-Exists 'PreviewWrapToggle' 'line-wrap-toggle'
Probe-Exists 'PreviewCopyAll' 'copy-all'
Probe-Exists 'PreviewLoadMore' 'load-more'
Probe-Exists 'PreviewChangedBanner' 'file-updated-banner'
Probe-Exists 'PreviewMarkdown' 'md-face'
Probe-Exists 'PreviewImage' 'image-face'
Probe-Exists 'PreviewPdf' 'pdf-face'
Probe-Exists 'FilesPanelPreviewText' 'text-face'
Probe-Exists 'PreviewHtmlRelated' 'html-related'

# 2. Tool cards
Probe-Exists 'ToolCard' 'tool-card'
Probe-Exists 'ToolCardExpand' 'tool-card-expand'
Probe-Exists 'ToolCopyJson' 'tool-copy-json'

# 3. Multi-question nav
Probe-Exists 'QuestionPanel' 'question-panel'
Probe-Exists 'QuestionNavPrev' 'q-prev'
Probe-Exists 'QuestionNavNext' 'q-next'
Probe-Exists 'QuestionSkip' 'q-skip'
Probe-Exists 'QuestionAbandonAll' 'q-abandon'
Probe-Exists 'QuestionToChat' 'q-to-chat'

# 4. Permission gate
Probe-Exists 'RiskAckCheckBox' 'risk-ack'
Probe-Exists 'ConfirmDangerFullAccessButton' 'danger-confirm'
Probe-Enabled 'ConfirmDangerFullAccessButton' $false 'danger-confirm-disabled-until-ack'

# 5. Cordis
Probe-Exists 'CordisPanel' 'cordis-panel'
Probe-Exists 'CordisApprovalCard' 'cordis-approval'
Probe-Exists 'CordisRunStop' 'cordis-run-stop'

# 6. Subagents
Probe-Exists 'SubagentList' 'subagent-list'
Probe-Exists 'SubagentPrompt' 'subagent-prompt'
Probe-Exists 'SubagentInterrupt' 'subagent-interrupt'

# 7. Trajectory
Probe-Exists 'TrajectoryPanel' 'trajectory-panel'
Probe-Exists 'TrajectoryTiming' 'trajectory-timing'
Probe-Exists 'TrajectoryLedger' 'trajectory-ledger'

# 8. Layout
Probe-Exists 'LayoutSplitterSidebar' 'sidebar-splitter'
Probe-Exists 'LayoutSplitterRight' 'right-splitter'
Probe-Exists 'LayoutSplitterRightInner' 'right-inner-splitter'
Probe-Exists 'RightPaneTabStrip' 'right-pane-tabs'
Probe-Exists 'RightPaneCell0' 'right-pane-cell0'
Probe-Exists 'RightPaneCell1' 'right-pane-cell1'

# 9. P2 model / about
Probe-Exists 'RestoreDefaultModelButton' 'restore-default-model'
Probe-Exists 'ModelButton' 'model-button'
Probe-Exists 'NewSessionItem' 'new-session'
Probe-Exists 'InputBox' 'input-box'
Probe-Exists 'SendButton' 'send-button'
Probe-Exists 'ChatList' 'chat-list'

# shell chrome
Probe-Exists 'KernelBootPanel' 'kernel-boot-panel'
Probe-Exists 'SettingsItem' 'settings-item'
Probe-Exists 'PermissionButton' 'permission-button'

W '--- SUMMARY ---'
$pass = @($results | Where-Object { $_.Status -eq 'PASS' }).Count
$fail = @($results | Where-Object { $_.Status -eq 'FAIL' }).Count
$part = @($results | Where-Object { $_.Status -eq 'PARTIAL' }).Count
W ("TOTAL=$($results.Count) PASS=$pass FAIL=$fail PARTIAL=$part")

if ($Kill) {
  Stop-ProcTree ([int]$pi.Id)
  W 'KILLED'
} else {
  W ('LEFT-RUNNING pid=' + $pi.Id)
}

[void]$sb.ToString()
# write log
try {
  $dir = Split-Path -Parent $OutLog
  if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
  Set-Content -LiteralPath $OutLog -Value $sb.ToString() -Encoding UTF8
  W ('LOG=' + $OutLog)
} catch { }

exit $(if ($fail -gt 0) { 1 } else { 0 })
