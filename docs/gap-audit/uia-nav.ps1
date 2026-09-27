<#
  T27 interactive UIA navigation — open lazy surfaces then probe AutomationIds.
#>
[CmdletBinding()]
param(
  [string]$Exe = 'E:\Syncthing\DshWinUI\bin\x64\Debug\net8.0-windows10.0.22621.0\win-x64\Blade2.exe',
  [string]$TitlePattern = 'Blade',
  [int]$TimeoutSec = 45,
  [switch]$Kill
)

$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

function W([string]$m) { [Console]::Out.WriteLine($m) }

function Stop-ProcTree([int]$id) {
  $kids = @()
  try { $kids = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$id" -ErrorAction Stop) } catch { }
  foreach ($k in $kids) { Stop-ProcTree ([int]$k.ProcessId) }
  try { Stop-Process -Id $id -Force -ErrorAction Stop } catch { }
}

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$AE       = [System.Windows.Automation.AutomationElement]
$TS       = [System.Windows.Automation.TreeScope]
$TRUECOND = [System.Windows.Automation.Condition]::TrueCondition
$WALKER   = [System.Windows.Automation.TreeWalker]::ControlViewWalker
$AIDPROP  = [System.Windows.Automation.AutomationElementIdentifiers]::AutomationIdProperty
$NAMEPROP = [System.Windows.Automation.AutomationElementIdentifiers]::NameProperty

function Get-ElInfo($el) { try { return $el.Current } catch { return $null } }
function Get-Aid($el) { try { return [string]$el.GetCurrentPropertyValue($AIDPROP) } catch { return '' } }
function Get-AName($el) { try { return [string]$el.GetCurrentPropertyValue($NAMEPROP) } catch { return '' } }
function Get-Enabled($el) { $i = Get-ElInfo $el; if ($null -eq $i) { return $null }; try { return [bool]$i.IsEnabled } catch { return $null } }

function Get-Descendants($root, [int]$cap = 2000) {
  $list = New-Object System.Collections.Generic.List[object]
  try {
    $col = $root.FindAll($TS::Descendants, $TRUECOND)
    $n = [Math]::Min($col.Count, $cap)
    for ($i = 0; $i -lt $n; $i++) { $el = $col[$i]; if ($null -ne $el) { $list.Add($el) } }
  } catch { }
  return $list
}

function Find-ByAid($root, [string]$id) {
  $hits = New-Object System.Collections.Generic.List[object]
  foreach ($el in (Get-Descendants $root)) {
    if ((Get-Aid $el) -eq $id) { $hits.Add($el) }
  }
  return $hits
}

function Find-ByName($root, [string]$needle) {
  $hits = New-Object System.Collections.Generic.List[object]
  foreach ($el in (Get-Descendants $root)) {
    $nm = Get-AName $el
    if ($nm -eq $needle) { $hits.Add($el); continue }
    try { if ($nm -match $needle) { $hits.Add($el) } } catch { }
  }
  return $hits
}

function Format-Err($e) {
  $cur = $e
  if ($null -ne $e.InnerException) { $cur = $e.InnerException }
  return ($cur.GetType().Name + ': ' + (($cur.Message -replace '\s+', ' ')).Trim())
}

function Invoke-Element($el) {
  $tries = @(
    @{ n = 'Invoke'; id = [System.Windows.Automation.InvokePatternIdentifiers]::Pattern },
    @{ n = 'Toggle'; id = [System.Windows.Automation.TogglePatternIdentifiers]::Pattern },
    @{ n = 'ExpandCollapse'; id = [System.Windows.Automation.ExpandCollapsePatternIdentifiers]::Pattern },
    @{ n = 'SelectionItem'; id = [System.Windows.Automation.SelectionItemPatternIdentifiers]::Pattern },
    @{ n = 'ExpandCollapseExpand'; id = [System.Windows.Automation.ExpandCollapsePatternIdentifiers]::Pattern; force = 'Expand' }
  )
  foreach ($t in $tries) {
    $pat = $null
    try { $pat = $el.GetCurrentPattern($t.id) } catch { $pat = $null }
    if ($null -eq $pat) { continue }
    try {
      if ($t.n -eq 'Invoke') { [void]([System.Windows.Automation.InvokePattern]$pat).Invoke() }
      elseif ($t.n -eq 'Toggle') { [void]([System.Windows.Automation.TogglePattern]$pat).Toggle() }
      elseif ($t.n -eq 'ExpandCollapse') {
        $ep = [System.Windows.Automation.ExpandCollapsePattern]$pat
        if ($ep.Current.ExpandCollapseState -eq [System.Windows.Automation.ExpandCollapseState]::Expanded) { $ep.Collapse() } else { $ep.Expand() }
      }
      elseif ($t.n -eq 'ExpandCollapseExpand') {
        $ep = [System.Windows.Automation.ExpandCollapsePattern]$pat
        $ep.Expand()
      }
      else { [void]([System.Windows.Automation.SelectionItemPattern]$pat).Select() }
      return @{ ok = $true; via = $t.n }
    } catch {
      return @{ ok = $false; via = $t.n; why = (Format-Err $_.Exception) }
    }
  }
  return @{ ok = $false; via = ''; why = 'no pattern' }
}

function Dump-Ids($root, [string]$tag) {
  W "=== IDS $tag ==="
  $ids = @{}
  foreach ($el in (Get-Descendants $root 2500)) {
    $aid = Get-Aid $el
    if ([string]::IsNullOrEmpty($aid)) { continue }
    $info = Get-ElInfo $el
    $ct = ''
    $en = ''
    $nm = Get-AName $el
    if ($null -ne $info) {
      $ct = $info.ControlType.ProgrammaticName -replace '^ControlType\.', ''
      $en = [string]$info.IsEnabled
    }
    $key = $aid
    if (-not $ids.ContainsKey($key)) {
      $ids[$key] = $true
      W ("  id=$aid | $ct | en=$en | name=$nm")
    }
  }
  W ("  unique-ids=" + $ids.Count)
}

function Probe([string]$id, [string]$expect = '') {
  $hits = Find-ByAid $win $id
  if ($hits.Count -eq 0) {
    W ("PROBE $id => MISSING")
    return @{ status = 'MISSING'; count = 0 }
  }
  $el = $hits[0]
  $en = Get-Enabled $el
  $nm = Get-AName $el
  $st = 'PASS'
  if ($expect -eq 'disabled' -and $en -ne $false) { $st = 'FAIL' }
  if ($expect -eq 'enabled' -and $en -ne $true) { $st = 'FAIL' }
  W ("PROBE $id => $st count=$($hits.Count) en=$en name=$nm")
  return @{ status = $st; count = $hits.Count; en = $en; name = $nm; el = $el }
}

function Do-Invoke($elOrId, [string]$tag) {
  $el = $null
  if ($elOrId -is [string]) {
    $hits = Find-ByAid $win $elOrId
    if ($hits.Count -eq 0) { $hits = Find-ByName $win $elOrId }
    if ($hits.Count -eq 0) { W "INVOKE $tag ($elOrId) => NOTFOUND"; return $false }
    $el = $hits[0]
  } else { $el = $elOrId }
  $r = Invoke-Element $el
  if ($r.ok) { W "INVOKE $tag => OK via=$($r.via)"; Start-Sleep -Milliseconds 700; return $true }
  W "INVOKE $tag => FAIL $($r.why)"
  return $false
}

# ---------------------------------------------------------------- start
Get-Process -Name 'Blade2' -ErrorAction SilentlyContinue | ForEach-Object { Stop-ProcTree ([int]$_.Id) }
Start-Sleep -Milliseconds 400

$pi = Start-Process -FilePath $Exe -PassThru
W "PID=$($pi.Id)"

Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public struct Win32UiaN {
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
}
'@

function Find-Hwnd([uint32]$procId) {
  $found = New-Object System.Collections.Generic.List[object]
  $cb = [Win32UiaN+EnumWindowsProc] {
    param($h, $l)
    $vp = [uint32]0
    [void][Win32UiaN]::GetWindowThreadProcessId($h, [ref]$vp)
    if ($vp -eq $procId -and [Win32UiaN]::IsWindowVisible($h)) {
      $len = [Win32UiaN]::GetWindowTextLength($h)
      $sb2 = New-Object System.Text.StringBuilder([Math]::Max($len + 2, 8))
      [void][Win32UiaN]::GetWindowText($h, $sb2, $sb2.Capacity)
      $found.Add(@{ h = $h; t = $sb2.ToString() })
    }
    return $true
  }
  [void][Win32UiaN]::EnumWindows($cb, [IntPtr]::Zero)
  return $found
}

$deadline = (Get-Date).AddSeconds($TimeoutSec)
$win = $null
while ((Get-Date) -lt $deadline) {
  if ($pi.HasExited) { W "RESULT=error exited $($pi.ExitCode)"; exit 4 }
  try {
    $col = $AE::RootElement.FindAll($TS::Children, $TRUECOND)
    for ($i = 0; $i -lt $col.Count; $i++) {
      $el = $col[$i]
      $info = Get-ElInfo $el
      if ($null -eq $info) { continue }
      if ($info.ProcessId -eq $pi.Id) { $win = $el; break }
    }
  } catch { }
  if ($null -ne $win) { break }
  Start-Sleep -Milliseconds 400
}
if ($null -eq $win) { W 'RESULT=error no window'; if ($Kill) { Stop-ProcTree ([int]$pi.Id) }; exit 2 }
W "WINDOW=$(Get-AName $win)"

# settle
$settle = (Get-Date).AddSeconds(15)
while ((Get-Date) -lt $settle) {
  Start-Sleep -Milliseconds 400
  $els = Get-Descendants $win 50
  if ($els.Count -ge 10) { break }
}

Dump-Ids $win 'startup'

# ---- 1. New session (may reveal PermissionButton)
Do-Invoke 'NewSessionItem' 'new-session'
Start-Sleep -Milliseconds 800
Dump-Ids $win 'after-new-session'
Probe 'PermissionButton' | Out-Null
Probe 'AgentModeButton' | Out-Null
Probe 'WorkspacePickerButton' | Out-Null

# ---- 2. Open files panel via ComposerAddButton -> 工作区文件
Do-Invoke 'ComposerAddButton' 'composer-add'
Start-Sleep -Milliseconds 400
# flyout items appear as MenuItems
Do-Invoke '工作区文件' 'open-files-panel'
Start-Sleep -Milliseconds 900
Dump-Ids $win 'after-files-panel'
foreach ($id in @('PreviewRoot','PreviewToolbar','PreviewWrapToggle','PreviewCopyAll','PreviewCopySelection','PreviewReload','PreviewLoadMore','PreviewChangedBanner','PreviewMarkdown','PreviewImage','PreviewPdf','FilesPanelPreviewText','PreviewHtmlRelated','FilesPanelTree','FilesPanelStatusBar','LayoutSplitterSidebar','LayoutSplitterRight','RightPaneTabStrip','RightPaneCell0','RightPaneCell1','FileActionMenu')) {
  Probe $id | Out-Null
}

# try wrap toggle / copy / load more if present
Do-Invoke 'PreviewWrapToggle' 'wrap-toggle'
Probe 'PreviewWrapToggle' | Out-Null
Do-Invoke 'PreviewCopyAll' 'copy-all'
Do-Invoke 'PreviewLoadMore' 'load-more'

# ---- 3. Right pane split via tab menu if present
$tabHits = Find-ByAid $win 'RightPaneTabStrip'
if ($tabHits.Count -gt 0) {
  # find a tab chip
  foreach ($el in (Get-Descendants $win)) {
    $aid = Get-Aid $el
    if ($aid -like 'RightPaneTab:*') {
      Do-Invoke $el "open-tab-menu $aid"
      Start-Sleep -Milliseconds 400
      Dump-Ids $win 'tab-menu'
      Do-Invoke 'RightPaneSplitHorizontal' 'split-h'
      Start-Sleep -Milliseconds 600
      Probe 'RightPaneCell1' | Out-Null
      Probe 'LayoutSplitterRightInner' | Out-Null
      # try split again -> should refuse (text becomes 分栏已满)
      foreach ($el2 in (Get-Descendants $win)) {
        if ((Get-Aid $el2) -like 'RightPaneTab:*') {
          Do-Invoke $el2 'reopen-tab-menu'
          Start-Sleep -Milliseconds 300
          $sh = Find-ByAid $win 'RightPaneSplitHorizontal'
          foreach ($s in $sh) {
            W ("SPLIT-MENU-TEXT id=RightPaneSplitHorizontal name=$(Get-AName $s) en=$(Get-Enabled $s)")
          }
          break
        }
      }
      break
    }
  }
}

# ---- 4. Settings
Do-Invoke 'SettingsItem' 'open-settings'
Start-Sleep -Milliseconds 800
Dump-Ids $win 'settings'
foreach ($id in @('SettingsHost','SettingsBreadcrumbParent','RestoreDefaultModelButton','Setting_permission_defaultPreset','ConnectionStatusBar','AboutCheckUpdateButton')) {
  Probe $id | Out-Null
}

# navigate settings sections looking for model / about
foreach ($el in (Get-Descendants $win)) {
  $aid = Get-Aid $el
  if ($aid -like 'SettingsSection_*') {
    W "SETTINGS-SECTION id=$aid name=$(Get-AName $el)"
  }
}

# try model section
foreach ($want in @('SettingsSection_models','SettingsSection_model','SettingsSection_about','SettingsSection_permission','SettingsSection_permissions')) {
  $hits = Find-ByAid $win $want
  if ($hits.Count -gt 0) {
    Do-Invoke $hits[0] "settings-$want"
    Start-Sleep -Milliseconds 600
    Dump-Ids $win "after-$want"
    Probe 'RestoreDefaultModelButton' | Out-Null
  }
}

# brute: if About / Restore not found, click every SettingsSection
$restore = Find-ByAid $win 'RestoreDefaultModelButton'
if ($restore.Count -eq 0) {
  foreach ($el in (Get-Descendants $win)) {
    $aid = Get-Aid $el
    if ($aid -like 'SettingsSection_*') {
      Do-Invoke $el "scan-$aid"
      Start-Sleep -Milliseconds 500
      $r = Find-ByAid $win 'RestoreDefaultModelButton'
      if ($r.Count -gt 0) {
        W "FOUND RestoreDefaultModelButton under $aid"
        Probe 'RestoreDefaultModelButton' | Out-Null
        break
      }
      # also look for 内测声明 text
      $brand = Find-ByName $win '内测声明'
      if ($brand.Count -gt 0) {
        W "FOUND 内测声明 text under $aid name=$(Get-AName $brand[0])"
        break
      }
    }
  }
}

# look for brand notice text anywhere
foreach ($needle in @('内测声明','预览版','DSH 本地构建','恢复默认模型','上次使用')) {
  $hits = Find-ByName $win $needle
  W "TEXT-HIT '$needle' count=$($hits.Count)"
  if ($hits.Count -gt 0) {
    $en = Get-Enabled $hits[0]
    W "  first: name=$(Get-AName $hits[0]) en=$en"
  }
}

# ---- 5. Model menu (back to chat first)
Do-Invoke 'ShellBackButton' 'back-to-chat'
Start-Sleep -Milliseconds 500
Do-Invoke 'ModelButton' 'model-menu'
Start-Sleep -Milliseconds 500
Dump-Ids $win 'model-menu'
foreach ($id in @('ModelOption_LastUsed','RestoreDefaultModelButton')) {
  Probe $id | Out-Null
}
# model option items
foreach ($el in (Get-Descendants $win)) {
  $aid = Get-Aid $el
  if ($aid -like 'ModelOption*' -or $aid -like 'EffortOption*') {
    W "MODEL-ITEM id=$aid name=$(Get-AName $el) help=$(try { $el.GetCurrentPropertyValue([System.Windows.Automation.AutomationElementIdentifiers]::HelpTextProperty) } catch { '' })"
  }
}
foreach ($needle in @('快速、高效','更强的自主','Flash','Pro','上次使用')) {
  $hits = Find-ByName $win $needle
  W "MODEL-TEXT '$needle' count=$($hits.Count)"
}

# ---- 6. Permission dropdown -> try danger-full-access
$perm = Find-ByAid $win 'PermissionButton'
if ($perm.Count -eq 0) {
  # after new session it may appear
  Do-Invoke 'NewSessionItem' 'new-session-2'
  Start-Sleep -Milliseconds 800
  $perm = Find-ByAid $win 'PermissionButton'
}
if ($perm.Count -gt 0) {
  Do-Invoke $perm[0] 'permission-menu'
  Start-Sleep -Milliseconds 400
  Dump-Ids $win 'permission-menu'
  # look for danger / 完全权限 menu item
  foreach ($needle in @('完全权限','danger-full-access','启用完全权限')) {
    $hits = Find-ByName $win $needle
    W "PERM-TEXT '$needle' count=$($hits.Count)"
    if ($hits.Count -gt 0 -and $needle -ne '启用完全权限') {
      Do-Invoke $hits[0] "select-$needle"
      Start-Sleep -Milliseconds 800
      Dump-Ids $win 'after-danger-select'
      Probe 'RiskAckCheckBox' | Out-Null
      Probe 'ConfirmDangerFullAccessButton' 'disabled' | Out-Null
      # try confirm without ack -> should stay disabled
      $btn = Find-ByAid $win 'ConfirmDangerFullAccessButton'
      if ($btn.Count -gt 0) {
        $en = Get-Enabled $btn[0]
        W "GATE confirm-enabled=$en (expect False)"
        # check the risk box
        Do-Invoke 'RiskAckCheckBox' 'risk-ack-check'
        Start-Sleep -Milliseconds 300
        $btn2 = Find-ByAid $win 'ConfirmDangerFullAccessButton'
        if ($btn2.Count -gt 0) {
          $en2 = Get-Enabled $btn2[0]
          W "GATE after-check confirm-enabled=$en2 (expect True)"
        }
        # cancel dialog (CloseButton)
        $close = Find-ByName $win '取消'
        if ($close.Count -gt 0) {
          Do-Invoke $close[0] 'cancel-dialog'
          Start-Sleep -Milliseconds 400
        }
        # verify RiskAck gone (dialog closed) and no writeback: reopen permission menu
        $riskAfter = Find-ByAid $win 'RiskAckCheckBox'
        W "GATE after-cancel risk-present=$($riskAfter.Count -gt 0) (expect False)"
      }
      break
    }
  }
} else {
  W 'PROBE PermissionButton => STILL-MISSING (collapsed without session)'
}

# ---- 7. Subagent / Trajectory / Cordis / ToolCard presence (content-dependent)
foreach ($id in @('SubagentList','SubagentPrompt','SubagentInterrupt','SubagentParentButton','TrajectoryPanel','CordisPanel','CordisApprovalCard','CordisRunStop','ToolCard','ToolCardExpand','ToolCopyJson','QuestionPanel','QuestionNavPrev','QuestionNavNext','QuestionSkip','QuestionAbandonAll','QuestionToChat','ChatSessionTitle')) {
  Probe $id | Out-Null
}

Dump-Ids $win 'final'

W "LEFT-RUNNING pid=$($pi.Id)"
if ($Kill) { Stop-ProcTree ([int]$pi.Id); W 'KILLED' }
exit 0
