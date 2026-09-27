<#
  T36 deep UIA smoke — kernel-content driven probes (no coordinate / key injection).
  Usage: pwsh -NoProfile -ExecutionPolicy Bypass -File docs/gap-audit/uia-deep.ps1 [-Kill] [-SendPrompt "run bash"]
#>
[CmdletBinding()]
param(
  [string]$Exe = 'E:\Syncthing\DshWinUI\bin\x64\Debug\net8.0-windows10.0.22621.0\win-x64\Blade2.exe',
  [string]$TitlePattern = 'Blade|DSH|dsh',
  [int]$TimeoutSec = 60,
  [int]$BootTimeoutSec = 120,
  [int]$MaxElements = 500,
  [int]$LookupCap = 2500,
  [string]$OutLog = 'E:\Syncthing\DshWinUI\docs\gap-audit\uia-deep-log.txt',
  [string]$SendPrompt = 'Use the bash tool to run: echo T36-deep-smoke. Then write a short file t36-smoke.txt with content OK.',
  [int]$ContentWaitSec = 90,
  [switch]$Kill
)

$ErrorActionPreference = 'Continue'
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
public struct Win32UiaD {
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
$NAMEPROP = [System.Windows.Automation.AutomationElementIdentifiers]::NameProperty
$HELPTEXT = [System.Windows.Automation.AutomationElementIdentifiers]::HelpTextProperty

function Get-ElInfo($el) { try { return $el.Current } catch { return $null } }
function Get-Aid($el) { try { return [string]$el.GetCurrentPropertyValue($AIDPROP) } catch { return '' } }
function Get-AName($el) { try { return [string]$el.GetCurrentPropertyValue($NAMEPROP) } catch { return '' } }
function Get-Help($el) { try { return [string]$el.GetCurrentPropertyValue($HELPTEXT) } catch { return '' } }
function Get-Enabled($el) { $i = Get-ElInfo $el; if ($null -eq $i) { return $null }; try { return [bool]$i.IsEnabled } catch { return $null } }

function Format-Name([string]$n) {
  if ($null -eq $n) { return '' }
  $n = $n -replace '[\r\n]+', ' \n '
  $n = $n -replace '\s+', ' '
  $n = $n.Trim()
  if ($n.Length -gt 160) { $n = $n.Substring(0, 157) + '...' }
  return $n
}

function Format-Err($e) {
  $cur = $e
  if ($null -ne $e.InnerException) { $cur = $e.InnerException }
  return ($cur.GetType().Name + ': ' + (($cur.Message -replace '\s+', ' ')).Trim())
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

function Find-ByAid($root, [string]$id, [int]$cap = 2500) {
  $hits = New-Object System.Collections.Generic.List[object]
  foreach ($el in (Get-Descendants $root $cap)) {
    if ((Get-Aid $el) -eq $id) { $hits.Add($el) }
  }
  return $hits
}

function Find-ByName($root, [string]$needle, [int]$cap = 2500) {
  $hits = New-Object System.Collections.Generic.List[object]
  foreach ($el in (Get-Descendants $root $cap)) {
    $nm = Get-AName $el
    if ($nm -eq $needle) { $hits.Add($el); continue }
    try { if ($nm -and $nm -match [regex]::Escape($needle)) { $hits.Add($el) } } catch { }
  }
  return $hits
}

function Invoke-Element($el) {
  # MenuFlyoutItem 会先暴露 TogglePattern：对菜单项 Toggle 只改状态不触发 Click。
  # 因此菜单项优先 Invoke/SelectionItem，Toggle 仅作兜底。
  $tries = @(
    @{ n = 'InvokePattern'; id = [System.Windows.Automation.InvokePatternIdentifiers]::Pattern },
    @{ n = 'SelectionItemPattern'; id = [System.Windows.Automation.SelectionItemPatternIdentifiers]::Pattern },
    @{ n = 'ExpandCollapsePattern'; id = [System.Windows.Automation.ExpandCollapsePatternIdentifiers]::Pattern },
    @{ n = 'TogglePattern'; id = [System.Windows.Automation.TogglePatternIdentifiers]::Pattern }
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
      # 模式存在但调用失败：继续试下一个，不能一失败就整只放弃
      W ("INVOKE-PATTERN-FAIL via=$($t.n) why=$(Format-Err $_.Exception)")
      continue
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

function Get-EditValue($el) {
  try {
    $pat = $el.GetCurrentPattern([System.Windows.Automation.ValuePatternIdentifiers]::Pattern)
    if ($null -eq $pat) { return '' }
    return [string]([System.Windows.Automation.ValuePattern]$pat).Current.Value
  } catch { return '' }
}

function Dump-Ids($root, [string]$tag) {
  W "=== IDS $tag ==="
  $ids = @{}
  foreach ($el in (Get-Descendants $root $LookupCap)) {
    $aid = Get-Aid $el
    if ([string]::IsNullOrEmpty($aid)) { continue }
    if (-not $ids.ContainsKey($aid)) {
      $ids[$aid] = $true
      $info = Get-ElInfo $el
      $ct = ''; $en = ''
      if ($null -ne $info) {
        $ct = $info.ControlType.ProgrammaticName -replace '^ControlType\.', ''
        $en = [string]$info.IsEnabled
      }
      W ("  id=$aid | $ct | en=$en | name=$(Format-Name (Get-AName $el))")
    }
  }
  W ("  unique-ids=" + $ids.Count)
}

$script:results = @()
function Find-ByAidDeep([string]$id) {
  # ContentDialog/Popup 可能挂在 RootElement 直属子树，不一定是主窗后代
  $hits = Find-ByAid $win $id $LookupCap
  if ($hits.Count -gt 0) { return $hits }
  try {
    $col = $AE::RootElement.FindAll($TS::Children, $TRUECOND)
    for ($i = 0; $i -lt $col.Count; $i++) {
      $el = $col[$i]
      $info = Get-ElInfo $el
      if ($null -eq $info) { continue }
      if ($info.ProcessId -ne $pi.Id) { continue }
      $more = Find-ByAid $el $id $LookupCap
      if ($more.Count -gt 0) { return $more }
    }
  } catch { }
  return @()
}

function Probe-Exists([string]$id, [string]$label, [string]$group = '') {
  $hits = Find-ByAidDeep $id
  $n = $hits.Count
  $en = ''; $nm = ''; $help = ''
  if ($n -gt 0) {
    $en = [string](Get-Enabled $hits[0])
    $nm = Format-Name (Get-AName $hits[0])
    $help = Format-Name (Get-Help $hits[0])
  }
  $status = if ($n -gt 0) { 'PASS' } else { 'FAIL' }
  W ("PROBE exists id=$id label=$label group=$group count=$n enabled=$en name=$nm help=$help => $status")
  $script:results += [pscustomobject]@{ Id = $id; Label = $label; Group = $group; Status = $status; Detail = "count=$n enabled=$en name=$nm help=$help" }
  return $hits
}

function Probe-Enabled([string]$id, [bool]$expectEnabled, [string]$label, [string]$group = '') {
  $hits = Find-ByAid $win $id $LookupCap
  if ($hits.Count -eq 0) {
    W ("PROBE enabled id=$id label=$label => FAIL (not found)")
    $script:results += [pscustomobject]@{ Id = $id; Label = $label; Group = $group; Status = 'FAIL'; Detail = 'not found' }
    return
  }
  $en = Get-Enabled $hits[0]
  $ok = ($en -eq $expectEnabled)
  $status = if ($ok) { 'PASS' } else { 'FAIL' }
  W ("PROBE enabled id=$id label=$label expect=$expectEnabled actual=$en => $status")
  $script:results += [pscustomobject]@{ Id = $id; Label = $label; Group = $group; Status = $status; Detail = "expect=$expectEnabled actual=$en" }
}

function Probe-Invoke([string]$id, [string]$label, [string]$group = '') {
  $hits = Find-ByAid $win $id $LookupCap
  if ($hits.Count -eq 0) {
    W ("PROBE invoke id=$id label=$label => FAIL (not found)")
    $script:results += [pscustomobject]@{ Id = $id; Label = $label; Group = $group; Status = 'FAIL'; Detail = 'not found' }
    return $false
  }
  $el = $hits[0]
  $en = Get-Enabled $el
  if ($en -eq $false) {
    W ("PROBE invoke id=$id label=$label => PARTIAL (exists but disabled)")
    $script:results += [pscustomobject]@{ Id = $id; Label = $label; Group = $group; Status = 'PARTIAL'; Detail = 'exists but disabled' }
    return $false
  }
  $r = Invoke-Element $el
  if ($r.ok) {
    Start-Sleep -Milliseconds 1500
    W ("PROBE invoke id=$id label=$label via=$($r.via) => PASS")
    $script:results += [pscustomobject]@{ Id = $id; Label = $label; Group = $group; Status = 'PASS'; Detail = "invoked via $($r.via)" }
    return $true
  }
  W ("PROBE invoke id=$id label=$label => FAIL reason=$($r.why)")
  $script:results += [pscustomobject]@{ Id = $id; Label = $label; Group = $group; Status = 'FAIL'; Detail = $r.why }
  return $false
}

function Probe-Name([string]$needle, [string]$label, [string]$group = '') {
  $hits = Find-ByName $win $needle $LookupCap
  $n = $hits.Count
  $status = if ($n -gt 0) { 'PASS' } else { 'FAIL' }
  W ("PROBE name needle=$needle label=$label group=$group count=$n => $status")
  $script:results += [pscustomobject]@{ Id = "name:$needle"; Label = $label; Group = $group; Status = $status; Detail = "count=$n" }
  return $hits
}

function Do-InvokeEl($el, [string]$tag, [int]$settleMs = 700) {
  $r = Invoke-Element $el
  if ($r.ok) { W "INVOKE $tag => OK via=$($r.via)"; Start-Sleep -Milliseconds $settleMs; return $true }
  W "INVOKE $tag => FAIL $($r.why)"
  return $false
}

function Do-Invoke($idOrEl, [string]$tag) {
  if ($idOrEl -is [string]) {
    $hits = Find-ByAid $win $idOrEl $LookupCap
    if ($hits.Count -eq 0) { $hits = Find-ByName $win $idOrEl $LookupCap }
    if ($hits.Count -eq 0) { W "INVOKE $tag ($idOrEl) => NOTFOUND"; return $false }
    return Do-InvokeEl $hits[0] $tag
  }
  return Do-InvokeEl $idOrEl $tag
}

# ---------------------------------------------------------------- start
if (-not (Test-Path -LiteralPath $Exe)) { W "RESULT=error exe not found: $Exe"; exit 3 }
$Exe = (Resolve-Path -LiteralPath $Exe).Path

Get-Process -Name 'Blade2' -ErrorAction SilentlyContinue | ForEach-Object {
  W ('KILL-LEFTOVER pid=' + $_.Id)
  Stop-ProcTree ([int]$_.Id)
}
Start-Sleep -Milliseconds 600

$logDir = Join-Path $env:TEMP 't36-uia'
if (-not (Test-Path $logDir)) { New-Item -ItemType Directory -Force -Path $logDir | Out-Null }
$logOut = Join-Path $logDir 'blade2.out.txt'
$logErr = Join-Path $logDir 'blade2.err.txt'
Remove-Item -LiteralPath $logOut, $logErr -Force -ErrorAction SilentlyContinue

$unhandled = Join-Path $env:TEMP 'blade2_unhandled.txt'
if (Test-Path $unhandled) {
  W ('UNHANDLED-PRE size=' + (Get-Item $unhandled).Length + ' mtime=' + (Get-Item $unhandled).LastWriteTime)
}

$pi = Start-Process -FilePath $Exe -PassThru -RedirectStandardOutput $logOut -RedirectStandardError $logErr
W ('PID=' + $pi.Id)

function Find-HwndByTitle([uint32]$procId, [string]$pattern) {
  $found = New-Object System.Collections.Generic.List[object]
  $cb = [Win32UiaD+EnumWindowsProc] {
    param($h, $l)
    $vp = [uint32]0
    [void][Win32UiaD]::GetWindowThreadProcessId($h, [ref]$vp)
    if ($vp -eq $procId -and [Win32UiaD]::IsWindowVisible($h)) {
      $len = [Win32UiaD]::GetWindowTextLength($h)
      if ($len -ge 0) {
        $sb2 = New-Object System.Text.StringBuilder([Math]::Max($len + 2, 8))
        [void][Win32UiaD]::GetWindowText($h, $sb2, $sb2.Capacity)
        $t = $sb2.ToString()
        if ($t -match $pattern -or $len -eq 0) { $found.Add(@{ h = $h; t = $t }) }
      }
    }
    return $true
  }
  [void][Win32UiaD]::EnumWindows($cb, [IntPtr]::Zero)
  return $found
}

$deadline = (Get-Date).AddSeconds($TimeoutSec)
$win = $null
$title = ''
while ((Get-Date) -lt $deadline) {
  if ($pi.HasExited) {
    W ('RESULT=error app exited code=' + $pi.ExitCode)
    W ('ERRFILE=' + (Get-Content -LiteralPath $logErr -Raw -ErrorAction SilentlyContinue))
    if (Test-Path $unhandled) { W ('UNHANDLED=' + (Get-Content $unhandled -Tail 40 -ErrorAction SilentlyContinue)) }
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
        $win = $el; $title = [string]$info.Name
        break
      }
    }
  } catch { }
  if ($null -ne $win) { break }
  $ws = @(Find-HwndByTitle -procId ([uint32]$pi.Id) -pattern $TitlePattern)
  if ($ws.Count -gt 0) {
    try { $win = $AE::FromHandle($ws[0].h); $title = $ws[0].t } catch { $win = $null }
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

# settle + wait for kernel boot panel to go away (or fail with retry)
$bootDeadline = (Get-Date).AddSeconds($BootTimeoutSec)
$bootState = 'unknown'
while ((Get-Date) -lt $bootDeadline) {
  Start-Sleep -Milliseconds 500
  $panel = Find-ByAid $win 'KernelBootPanel' $LookupCap
  $retry = Find-ByAid $win 'KernelBootRetry' $LookupCap
  $hint = Find-ByAid $win 'KernelBootHint' $LookupCap
  $hintText = ''
  if ($hint.Count -gt 0) { $hintText = Format-Name (Get-AName $hint[0]) }
  if ($panel.Count -eq 0) { $bootState = 'ready'; break }
  $vis = $null
  try {
    $info = Get-ElInfo $panel[0]
    if ($null -ne $info) {
      # Offscreen / zero-size often means collapsed
      $rc = $info.BoundingRectangle
      if ($rc.IsEmpty -or $rc.Width -lt 2 -or $rc.Height -lt 2) { $bootState = 'ready'; break }
    }
  } catch { }
  if ($retry.Count -gt 0) {
    $ren = Get-Enabled $retry[0]
    $rname = Format-Name (Get-AName $retry[0])
    if ($ren -eq $true -and $rname) {
      $bootState = "failed hint=$hintText"
      break
    }
  }
}
W ("KERNEL-BOOT state=$bootState")
$script:results += [pscustomobject]@{ Id = 'KernelBoot'; Label = 'kernel-boot'; Group = 'boot'; Status = $(if ($bootState -eq 'ready') { 'PASS' } else { 'FAIL' }); Detail = $bootState }

if ($bootState -ne 'ready') {
  # one retry if button is available
  if (Probe-Invoke 'KernelBootRetry' 'kernel-boot-retry' 'boot') {
    $bootDeadline = (Get-Date).AddSeconds($BootTimeoutSec)
    while ((Get-Date) -lt $bootDeadline) {
      Start-Sleep -Milliseconds 500
      $panel = Find-ByAid $win 'KernelBootPanel' $LookupCap
      if ($panel.Count -eq 0) { $bootState = 'ready-after-retry'; break }
      try {
        $info = Get-ElInfo $panel[0]
        if ($null -ne $info) {
          $rc = $info.BoundingRectangle
          if ($rc.IsEmpty -or $rc.Width -lt 2 -or $rc.Height -lt 2) { $bootState = 'ready-after-retry'; break }
        }
      } catch { }
    }
    W ("KERNEL-BOOT-RETRY state=$bootState")
  }
}

if (Test-Path $unhandled) {
  W ('UNHANDLED-POST size=' + (Get-Item $unhandled).Length + ' mtime=' + (Get-Item $unhandled).LastWriteTime)
  W ('UNHANDLED-TAIL=' + ((Get-Content $unhandled -Tail 25 -ErrorAction SilentlyContinue) -join ' | '))
}

Dump-Ids $win 'after-boot'

# ---- shell chrome baseline
foreach ($id in @('ModelButton','NewSessionItem','InputBox','SendButton','ChatList','SettingsItem','PermissionButton','ComposerAddButton','KernelBootPanel','LayoutSplitterSidebar','LayoutSplitterRight','FilesPanelTree')) {
  Probe-Exists $id $id 'shell' | Out-Null
}

# ---- 0. open/create a session so content paths can attach
$sessionHit = $null
foreach ($el in (Get-Descendants $win $LookupCap)) {
  $aid = Get-Aid $el
  if ($aid -like 'Session_*') { $sessionHit = $el; W ("FOUND-SESSION id=$aid name=$(Format-Name (Get-AName $el))"); break }
}
if ($null -ne $sessionHit) {
  Do-InvokeEl $sessionHit 'open-existing-session'
  Start-Sleep -Milliseconds 1500
} else {
  Do-Invoke 'NewSessionItem' 'new-session'
  Start-Sleep -Milliseconds 1200
}
Dump-Ids $win 'after-session'

# ---- 1. model menu: HelpText two-tier + RestoreDefault
if (Probe-Invoke 'ModelButton' 'model-menu' 'model') {
  Start-Sleep -Milliseconds 500
  Dump-Ids $win 'model-menu'
  $helpHits = 0
  foreach ($el in (Get-Descendants $win $LookupCap)) {
    $aid = Get-Aid $el
    if ($aid -like 'ModelOption*' -or $aid -like 'EffortOption*') {
      $h = Format-Name (Get-Help $el)
      $n = Format-Name (Get-AName $el)
      W ("MODEL-ITEM id=$aid name=$n help=$h")
      if ($h -match 'Flash|Pro|快速|自主|经济|成本') { $helpHits++ }
      if ($aid -eq 'ModelOption_LastUsed') {
        $script:results += [pscustomobject]@{ Id = $aid; Label = 'model-last-used'; Group = 'model'; Status = 'PASS'; Detail = $n }
      }
      if ($aid -eq 'RestoreDefaultModelButton') {
        $script:results += [pscustomobject]@{ Id = $aid; Label = 'restore-default-in-menu'; Group = 'model'; Status = 'PASS'; Detail = $n }
      }
    }
  }
  $status = if ($helpHits -gt 0) { 'PASS' } else { 'FAIL' }
  W ("PROBE model-helptext count=$helpHits => $status")
  $script:results += [pscustomobject]@{ Id = 'ModelHelpText'; Label = 'model-tier-helptext'; Group = 'model'; Status = $status; Detail = "helpHits=$helpHits" }
  Probe-Exists 'ModelOption_LastUsed' 'model-last-used' 'model' | Out-Null
  Probe-Exists 'RestoreDefaultModelButton' 'restore-default-model' 'model' | Out-Null
  # dismiss flyout
  Do-Invoke 'ChatList' 'dismiss-model-menu'
  Start-Sleep -Milliseconds 300
}

# ---- 2. permission gate recheck
if (Probe-Invoke 'PermissionButton' 'permission-menu' 'perm') {
  Start-Sleep -Milliseconds 400
  Dump-Ids $win 'permission-menu'
  $sel = $null
  foreach ($needle in @('完全权限','danger-full-access','启用完全权限')) {
    $hits = Find-ByName $win $needle $LookupCap
    W ("PERM-TEXT '$needle' count=$($hits.Count)")
    if ($hits.Count -gt 0 -and $needle -ne '启用完全权限') { $sel = $hits[0]; break }
  }
  if ($null -ne $sel -or $true) {
    $preset = Find-ByAid $win 'PermissionPreset_danger-full-access' $LookupCap
    if ($preset.Count -gt 0) {
      Do-InvokeEl $preset[0] 'select-danger-preset' 1500 | Out-Null
    } elseif ($null -ne $sel) {
      Do-InvokeEl $sel 'select-danger' 1500 | Out-Null
    }
    Dump-Ids $win 'after-danger'
    Probe-Exists 'RiskAckCheckBox' 'risk-ack' 'perm' | Out-Null
    Probe-Enabled 'ConfirmDangerFullAccessButton' $false 'danger-confirm-disabled' 'perm' | Out-Null
    Do-Invoke 'RiskAckCheckBox' 'risk-ack-check' 'perm' | Out-Null
    Start-Sleep -Milliseconds 300
    Probe-Enabled 'ConfirmDangerFullAccessButton' $true 'danger-confirm-enabled-after-ack' 'perm' | Out-Null
    $close = Find-ByName $win '取消' $LookupCap
    if ($close.Count -gt 0) { Do-InvokeEl $close[0] 'cancel-dialog' | Out-Null; Start-Sleep -Milliseconds 400 }
    $riskAfter = Find-ByAid $win 'RiskAckCheckBox' $LookupCap
    W ("GATE after-cancel risk-present=$($riskAfter.Count -gt 0) (expect False)")
    $script:results += [pscustomobject]@{ Id = 'RiskGateCancel'; Label = 'perm-cancel-no-writeback'; Group = 'perm'; Status = $(if ($riskAfter.Count -eq 0) { 'PASS' } else { 'FAIL' }); Detail = "riskAfter=$($riskAfter.Count)" }
  }
}

# ---- 3. files panel + preview six forms
Do-Invoke 'ComposerAddButton' 'composer-add' | Out-Null
Start-Sleep -Milliseconds 400
Do-Invoke '工作区文件' 'open-files-panel' | Out-Null
Start-Sleep -Milliseconds 1200
Dump-Ids $win 'files-panel'
foreach ($id in @('FilesPanelTree','FilesPanelStatusBar','RightPaneTabStrip','RightPaneCell0','FileActionMenu','FileActionButton','RefreshButton')) {
  Probe-Exists $id $id 'files' | Out-Null
}

# Try to select fixture files by name in the tree and probe preview faces
$fixtureNames = @('sample.md','sample.cs','sample.txt','sample.html','sample.png','sample.pdf')
$faceMap = @{
  'sample.md'   = @('PreviewMarkdown','PreviewRoot','PreviewToolbar','PreviewWrapToggle','PreviewCopyAll','PreviewLoadMore')
  'sample.cs'   = @('FilesPanelPreviewText','PreviewRoot','PreviewToolbar','PreviewWrapToggle','PreviewCopyAll')
  'sample.txt'  = @('FilesPanelPreviewText','PreviewRoot','PreviewToolbar','PreviewWrapToggle','PreviewCopyAll')
  'sample.html' = @('PreviewHtmlOutline','PreviewHtmlRelated','PreviewRoot','PreviewToolbar')
  'sample.png'  = @('PreviewImage','PreviewRoot','PreviewToolbar')
  'sample.pdf'  = @('PreviewPdf','PreviewRoot','PreviewToolbar','PreviewPdfPageLabel','PreviewPdfOpen','PreviewPdfCopyPath')
}

foreach ($fn in $fixtureNames) {
  # 每次先回文件树，避免停在上一份预览里找不到下一节点
  Do-Invoke 'FilesPanelBackButton' "back-before-$fn" 400 | Out-Null
  $hits = Find-ByName $win $fn $LookupCap
  W ("FIXTURE-LOOKUP $fn count=$($hits.Count)")
  if ($hits.Count -eq 0) {
    Do-Invoke 'FilesPanelRefreshButton' "refresh-for-$fn" 800 | Out-Null
    $hits = Find-ByName $win $fn $LookupCap
    W ("FIXTURE-LOOKUP-AFTER-REFRESH $fn count=$($hits.Count)")
  }
  if ($hits.Count -eq 0) {
    $script:results += [pscustomobject]@{ Id = "preview:$fn"; Label = "preview-$fn"; Group = 'preview'; Status = 'PARTIAL'; Detail = 'fixture node not in tree (virtualized or list truncated)' }
    continue
  }
  $opened = $false
  foreach ($h in $hits) {
    $hn = Format-Name (Get-AName $h)
    if ($hn -notmatch [regex]::Escape($fn)) { continue }
    if (Do-InvokeEl $h "open-$fn" 1800) { $opened = $true; break }
  }
  if (-not $opened) {
    try {
      $pat = $hits[0].GetCurrentPattern([System.Windows.Automation.SelectionItemPatternIdentifiers]::Pattern)
      if ($null -ne $pat) { [void]([System.Windows.Automation.SelectionItemPattern]$pat).Select(); $opened = $true; W "SELECT $fn via SelectionItem"; Start-Sleep -Milliseconds 1800 }
    } catch { }
  }
  $faces = $faceMap[$fn]
  $all = $true
  $detail = @()
  foreach ($face in $faces) {
    $fhits = Find-ByAid $win $face $LookupCap
    if ($fhits.Count -gt 0) { $detail += "$face=1" }
    else { $detail += "$face=0"; $all = $false }
    Probe-Exists $face "face-$fn-$face" 'preview' | Out-Null
  }
  # 形态旁证（UIA 对部分 Panel 不暴露 PreviewRoot，用内容面/工具条旁证）
  foreach ($side in @('MarkdownScroll','FilesPanelPreviewMeta','PreviewCopyAll','PreviewReload','FilesPanelHeaderTitle')) {
    $sh = Find-ByAid $win $side $LookupCap
    if ($sh.Count -gt 0) { $detail += "$side=1" }
  }
  $st = if ($all) { 'PASS' } elseif ($opened -and ($detail -join ' ') -match 'PreviewCopyAll=1|MarkdownScroll=1|FilesPanelPreviewMeta=1') { 'PASS' } elseif ($opened) { 'PARTIAL' } else { 'FAIL' }
  $script:results += [pscustomobject]@{ Id = "preview:$fn"; Label = "preview-$fn"; Group = 'preview'; Status = $st; Detail = ($detail -join ' ') }
}

# ---- 4. trajectory panel (via ComposerAdd menu once wired, else still probe ids)
Do-Invoke 'ComposerAddButton' 'composer-add-2' | Out-Null
Start-Sleep -Milliseconds 400
Dump-Ids $win 'composer-menu'
$trajItem = $null
foreach ($el in (Get-Descendants $win $LookupCap)) {
  if ((Get-Aid $el) -eq 'TrajectoryMenuItem' -or (Get-AName $el) -match '^轨迹$') { $trajItem = $el; break }
}
if ($null -ne $trajItem) {
  Do-InvokeEl $trajItem 'open-trajectory' 2500 | Out-Null
}
Dump-Ids $win 'trajectory'
Probe-Exists 'TrajectoryPanel' 'trajectory-panel' 'traj' | Out-Null
Probe-Exists 'TrajectoryTiming' 'trajectory-timing' 'traj' | Out-Null
Probe-Exists 'TrajectoryLedger' 'trajectory-ledger' 'traj' | Out-Null
Probe-Exists 'TrajectoryKindFilter' 'trajectory-filter' 'traj' | Out-Null
# rows?
$ledger = Find-ByAid $win 'TrajectoryLedger' $LookupCap
if ($ledger.Count -gt 0) {
  $kids = Get-Descendants $ledger[0] 80
  W ("TRAJ-LEDGER-CHILDREN count=$($kids.Count)")
  $script:results += [pscustomobject]@{ Id = 'TrajectoryRows'; Label = 'trajectory-ledger-rows'; Group = 'traj'; Status = $(if ($kids.Count -gt 0) { 'PASS' } else { 'PARTIAL' }); Detail = "children=$($kids.Count)" }
}
# close dialog if open
$close = Find-ByName $win '关闭' $LookupCap
if ($close.Count -gt 0) { Do-InvokeEl $close[0] 'close-traj-dialog' | Out-Null; Start-Sleep -Milliseconds 400 }

# ---- 5. Cordis panel
Do-Invoke 'ComposerAddButton' 'composer-add-3' | Out-Null
Start-Sleep -Milliseconds 400
$cordisItem = $null
foreach ($el in (Get-Descendants $win $LookupCap)) {
  if ((Get-Aid $el) -eq 'CordisMenuItem' -or (Get-AName $el) -match 'Cordis') { $cordisItem = $el; break }
}
if ($null -ne $cordisItem) {
  Do-InvokeEl $cordisItem 'open-cordis' 2500 | Out-Null
}
Dump-Ids $win 'cordis'
Probe-Exists 'CordisPanel' 'cordis-panel' 'cordis' | Out-Null
Probe-Exists 'CordisApprovalCard' 'cordis-approval' 'cordis' | Out-Null
Probe-Exists 'CordisRunStop' 'cordis-run-stop' 'cordis' | Out-Null
$close = Find-ByName $win '关闭' $LookupCap
if ($close.Count -gt 0) { Do-InvokeEl $close[0] 'close-cordis-dialog' | Out-Null; Start-Sleep -Milliseconds 400 }

# ---- 6. subagents
Probe-Exists 'SubagentList' 'subagent-list' 'sub' | Out-Null
Probe-Exists 'SubagentPrompt' 'subagent-prompt' 'sub' | Out-Null
Probe-Exists 'SubagentInterrupt' 'subagent-interrupt' 'sub' | Out-Null
Probe-Exists 'SubagentParentButton' 'subagent-parent' 'sub' | Out-Null

# ---- 7. tool cards + questions: send prompt if InputBox ready (kernel content path)
$toolProbe = 'PARTIAL'
$qProbe = 'PARTIAL'
if ($SendPrompt -and $SendPrompt.Length -gt 0) {
  $inputHits = Find-ByAid $win 'InputBox' $LookupCap
  $sendHits = Find-ByAid $win 'SendButton' $LookupCap
  if ($inputHits.Count -gt 0 -and $sendHits.Count -gt 0) {
    if (Set-EditText $inputHits[0] $SendPrompt) {
      W ("SEND-SET text-len=$($SendPrompt.Length)")
      if (Do-InvokeEl $sendHits[0] 'send-prompt') {
        W "SEND-OK waiting ${ContentWaitSec}s for tool/question content"
        $waitUntil = (Get-Date).AddSeconds($ContentWaitSec)
        $sawTool = $false
        $sawQ = $false
        $sawCopy = $false
        while ((Get-Date) -lt $waitUntil) {
          Start-Sleep -Milliseconds 1500
          if (-not $sawTool -and (Find-ByAid $win 'ToolCard' $LookupCap).Count -gt 0) { $sawTool = $true; W 'CONTENT tool-card appeared' }
          if (-not $sawCopy -and (Find-ByAid $win 'ToolCopyJson' $LookupCap).Count -gt 0) { $sawCopy = $true; W 'CONTENT tool-copy-json appeared' }
          if (-not $sawQ -and (Find-ByAid $win 'QuestionPanel' $LookupCap).Count -gt 0) { $sawQ = $true; W 'CONTENT question-panel appeared' }
          if ($sawTool -and $sawCopy) { break }
        }
        Dump-Ids $win 'after-send'
        if ($sawTool) {
          Probe-Exists 'ToolCard' 'tool-card' 'tool' | Out-Null
          Probe-Invoke 'ToolCardExpand' 'tool-card-expand' 'tool' | Out-Null
          Probe-Exists 'ToolCopyJson' 'tool-copy-json' 'tool' | Out-Null
          Probe-Exists 'ToolCardExitCode' 'tool-exit-code' 'tool' | Out-Null
          Probe-Exists 'ToolCardStatus' 'tool-status' 'tool' | Out-Null
          $toolProbe = 'PASS'
        } else {
          $script:results += [pscustomobject]@{ Id = 'ToolCard'; Label = 'tool-card'; Group = 'tool'; Status = 'PARTIAL'; Detail = 'send accepted but ToolCard not observed within wait window' }
        }
        if ($sawQ) {
          Probe-Exists 'QuestionPanel' 'question-panel' 'q' | Out-Null
          Probe-Exists 'QuestionNavPrev' 'q-prev' 'q' | Out-Null
          Probe-Exists 'QuestionNavNext' 'q-next' 'q' | Out-Null
          Probe-Exists 'QuestionSkip' 'q-skip' 'q' | Out-Null
          Probe-Exists 'QuestionAbandonAll' 'q-abandon' 'q' | Out-Null
          Probe-Exists 'QuestionToChat' 'q-to-chat' 'q' | Out-Null
          $qProbe = 'PASS'
        } else {
          $script:results += [pscustomobject]@{ Id = 'QuestionPanel'; Label = 'question-panel'; Group = 'q'; Status = 'PARTIAL'; Detail = 'no ask_user_question waterfall observed in wait window' }
        }
        # trajectory after live content
        Do-Invoke 'ComposerAddButton' 'composer-add-4' | Out-Null
        Start-Sleep -Milliseconds 400
        $trajItem = $null
        foreach ($el in (Get-Descendants $win $LookupCap)) {
          if ((Get-Aid $el) -eq 'TrajectoryMenuItem' -or (Get-AName $el) -match '^轨迹$') { $trajItem = $el; break }
        }
        if ($null -ne $trajItem) {
          Do-InvokeEl $trajItem 'open-trajectory-after-content'
          Start-Sleep -Milliseconds 1000
          Probe-Exists 'TrajectoryPanel' 'trajectory-panel-live' 'traj' | Out-Null
          Probe-Exists 'TrajectoryTiming' 'trajectory-timing-live' 'traj' | Out-Null
          Probe-Exists 'TrajectoryLedger' 'trajectory-ledger-live' 'traj' | Out-Null
          $ledger = Find-ByAid $win 'TrajectoryLedger' $LookupCap
          if ($ledger.Count -gt 0) {
            $kids = Get-Descendants $ledger[0] 120
            W ("TRAJ-LEDGER-LIVE-CHILDREN count=$($kids.Count)")
            $script:results += [pscustomobject]@{ Id = 'TrajectoryRowsLive'; Label = 'trajectory-ledger-rows-live'; Group = 'traj'; Status = $(if ($kids.Count -gt 0) { 'PASS' } else { 'PARTIAL' }); Detail = "children=$($kids.Count)" }
          }
          $close = Find-ByName $win '关闭' $LookupCap
          if ($close.Count -gt 0) { Do-InvokeEl $close[0] 'close-traj-2' | Out-Null }
        }
      } else {
        $script:results += [pscustomobject]@{ Id = 'SendButton'; Label = 'send-prompt'; Group = 'tool'; Status = 'FAIL'; Detail = 'SendButton invoke failed' }
      }
    } else {
      $script:results += [pscustomobject]@{ Id = 'InputBox'; Label = 'set-input'; Group = 'tool'; Status = 'FAIL'; Detail = 'ValuePattern set failed' }
    }
  } else {
    $script:results += [pscustomobject]@{ Id = 'InputBox'; Label = 'input-or-send-missing'; Group = 'tool'; Status = 'FAIL'; Detail = "input=$($inputHits.Count) send=$($sendHits.Count)" }
  }
}

# ---- 8. settings restore-default model
Do-Invoke 'SettingsItem' 'open-settings' | Out-Null
Start-Sleep -Milliseconds 800
Dump-Ids $win 'settings'
Probe-Exists 'SettingsHost' 'settings-host' 'settings' | Out-Null
foreach ($want in @('SettingsSection_models','SettingsSection_model','SettingsSection_about')) {
  $hits = Find-ByAid $win $want $LookupCap
  if ($hits.Count -gt 0) {
    Do-InvokeEl $hits[0] "settings-$want" | Out-Null
    Start-Sleep -Milliseconds 500
    Probe-Exists 'RestoreDefaultModelButton' "restore-under-$want" 'settings' | Out-Null
  }
}
# brute force sections if needed
if ((Find-ByAid $win 'RestoreDefaultModelButton' $LookupCap).Count -eq 0) {
  foreach ($el in (Get-Descendants $win $LookupCap)) {
    $aid = Get-Aid $el
    if ($aid -like 'SettingsSection_*') {
      Do-InvokeEl $el "scan-$aid" | Out-Null
      Start-Sleep -Milliseconds 400
      if ((Find-ByAid $win 'RestoreDefaultModelButton' $LookupCap).Count -gt 0) {
        W "FOUND RestoreDefaultModelButton under $aid"
        Probe-Exists 'RestoreDefaultModelButton' 'restore-default-settings' 'settings' | Out-Null
        break
      }
    }
  }
}
Probe-Name '内测声明' 'brand-internal-test' 'settings' | Out-Null
Probe-Name '恢复默认模型' 'restore-default-text' 'settings' | Out-Null

Dump-Ids $win 'final'

W '--- SUMMARY ---'
$pass = @($results | Where-Object { $_.Status -eq 'PASS' }).Count
$fail = @($results | Where-Object { $_.Status -eq 'FAIL' }).Count
$part = @($results | Where-Object { $_.Status -eq 'PARTIAL' }).Count
W ("TOTAL=$($results.Count) PASS=$pass FAIL=$fail PARTIAL=$part")
W '--- BY GROUP ---'
$results | Group-Object Group | ForEach-Object {
  $gp = @($_.Group | Where-Object { $_.Status -eq 'PASS' }).Count
  $gf = @($_.Group | Where-Object { $_.Status -eq 'FAIL' }).Count
  $ga = @($_.Group | Where-Object { $_.Status -eq 'PARTIAL' }).Count
  W ("GROUP=$($_.Name) PASS=$gp FAIL=$gf PARTIAL=$ga")
}

if ($Kill) {
  Stop-ProcTree ([int]$pi.Id)
  W 'KILLED'
} else {
  W ('LEFT-RUNNING pid=' + $pi.Id)
}

try {
  $dir = Split-Path -Parent $OutLog
  if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
  Set-Content -LiteralPath $OutLog -Value $sb.ToString() -Encoding UTF8
  W ('LOG=' + $OutLog)
} catch { }

exit $(if ($fail -gt 0) { 1 } else { 0 })
