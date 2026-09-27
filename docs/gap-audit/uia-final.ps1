<#
  T27 final UIA pass: dismiss onboarding, open Settings/About/Models, probe remaining ids.
#>
[CmdletBinding()]
param(
  [string]$Exe = 'E:\Syncthing\DshWinUI\bin\x64\Debug\net8.0-windows10.0.22621.0\win-x64\Blade2.exe',
  [switch]$Kill
)
$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
function W([string]$m) { [Console]::Out.WriteLine($m) }
function Stop-ProcTree([int]$id) {
  $kids = @(); try { $kids = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$id" -ErrorAction Stop) } catch { }
  foreach ($k in $kids) { Stop-ProcTree ([int]$k.ProcessId) }
  try { Stop-Process -Id $id -Force -ErrorAction Stop } catch { }
}
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$AE = [System.Windows.Automation.AutomationElement]
$TS = [System.Windows.Automation.TreeScope]
$TRUECOND = [System.Windows.Automation.Condition]::TrueCondition
$AIDPROP = [System.Windows.Automation.AutomationElementIdentifiers]::AutomationIdProperty
$NAMEPROP = [System.Windows.Automation.AutomationElementIdentifiers]::NameProperty
$HELPTEXT = [System.Windows.Automation.AutomationElementIdentifiers]::HelpTextProperty

function Get-ElInfo($el) { try { return $el.Current } catch { return $null } }
function Get-Aid($el) { try { return [string]$el.GetCurrentPropertyValue($AIDPROP) } catch { return '' } }
function Get-AName($el) { try { return [string]$el.GetCurrentPropertyValue($NAMEPROP) } catch { return '' } }
function Get-Help($el) { try { return [string]$el.GetCurrentPropertyValue($HELPTEXT) } catch { return '' } }
function Get-Enabled($el) { $i = Get-ElInfo $el; if ($null -eq $i) { return $null }; try { return [bool]$i.IsEnabled } catch { return $null } }

function Get-Descendants($root, [int]$cap = 3000) {
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
  foreach ($el in (Get-Descendants $root)) { if ((Get-Aid $el) -eq $id) { $hits.Add($el) } }
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
  $cur = $e; if ($null -ne $e.InnerException) { $cur = $e.InnerException }
  return ($cur.GetType().Name + ': ' + (($cur.Message -replace '\s+', ' ')).Trim())
}
function Invoke-Element($el) {
  $tries = @(
    @{ n = 'Invoke'; id = [System.Windows.Automation.InvokePatternIdentifiers]::Pattern },
    @{ n = 'Toggle'; id = [System.Windows.Automation.TogglePatternIdentifiers]::Pattern },
    @{ n = 'ExpandCollapse'; id = [System.Windows.Automation.ExpandCollapsePatternIdentifiers]::Pattern },
    @{ n = 'SelectionItem'; id = [System.Windows.Automation.SelectionItemPatternIdentifiers]::Pattern }
  )
  foreach ($t in $tries) {
    $pat = $null; try { $pat = $el.GetCurrentPattern($t.id) } catch { $pat = $null }
    if ($null -eq $pat) { continue }
    try {
      if ($t.n -eq 'Invoke') { [void]([System.Windows.Automation.InvokePattern]$pat).Invoke() }
      elseif ($t.n -eq 'Toggle') { [void]([System.Windows.Automation.TogglePattern]$pat).Toggle() }
      elseif ($t.n -eq 'ExpandCollapse') {
        $ep = [System.Windows.Automation.ExpandCollapsePattern]$pat
        if ($ep.Current.ExpandCollapseState -eq [System.Windows.Automation.ExpandCollapseState]::Expanded) { $ep.Collapse() } else { $ep.Expand() }
      } else { [void]([System.Windows.Automation.SelectionItemPattern]$pat).Select() }
      return @{ ok = $true; via = $t.n }
    } catch { return @{ ok = $false; via = $t.n; why = (Format-Err $_.Exception) } }
  }
  return @{ ok = $false; via = ''; why = 'no pattern' }
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
  W "INVOKE $tag => FAIL $($r.why)"; return $false
}
function Probe([string]$id, [string]$expect = '') {
  $hits = Find-ByAid $win $id
  if ($hits.Count -eq 0) { W "PROBE $id => MISSING"; return }
  $el = $hits[0]; $en = Get-Enabled $el; $nm = Get-AName $el; $hp = Get-Help $el
  $st = 'PASS'
  if ($expect -eq 'disabled' -and $en -ne $false) { $st = 'FAIL' }
  if ($expect -eq 'enabled' -and $en -ne $true) { $st = 'FAIL' }
  W "PROBE $id => $st count=$($hits.Count) en=$en name=$nm help=$hp"
}
function Dump-Ids($root, [string]$tag) {
  W "=== IDS $tag ==="
  $seen = @{}
  foreach ($el in (Get-Descendants $root)) {
    $aid = Get-Aid $el
    if ([string]::IsNullOrEmpty($aid) -or $seen.ContainsKey($aid)) { continue }
    $seen[$aid] = $true
    $info = Get-ElInfo $el; $ct = ''; $en = ''
    if ($null -ne $info) { $ct = $info.ControlType.ProgrammaticName -replace '^ControlType\.', ''; $en = [string]$info.IsEnabled }
    W "  id=$aid | $ct | en=$en | name=$(Get-AName $el)"
  }
  W "  unique-ids=$($seen.Count)"
}
function Text-Hits([string]$needle) {
  $hits = Find-ByName $win $needle
  W "TEXT '$needle' count=$($hits.Count)"
  if ($hits.Count -gt 0) {
    foreach ($h in $hits | Select-Object -First 3) {
      W "  name=$(Get-AName $h) en=$(Get-Enabled $h) id=$(Get-Aid $h)"
    }
  }
  return $hits
}

# ---- start
Get-Process -Name 'Blade2' -ErrorAction SilentlyContinue | ForEach-Object { Stop-ProcTree ([int]$_.Id) }
Start-Sleep -Milliseconds 400
$pi = Start-Process -FilePath $Exe -PassThru
W "PID=$($pi.Id)"
Add-Type -TypeDefinition @'
using System; using System.Text; using System.Runtime.InteropServices;
public struct Win32UiaF {
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
}
'@
$deadline = (Get-Date).AddSeconds(45); $win = $null
while ((Get-Date) -lt $deadline) {
  if ($pi.HasExited) { W "RESULT=error exited $($pi.ExitCode)"; exit 4 }
  try {
    $col = $AE::RootElement.FindAll($TS::Children, $TRUECOND)
    for ($i = 0; $i -lt $col.Count; $i++) {
      $el = $col[$i]; $info = Get-ElInfo $el
      if ($null -ne $info -and $info.ProcessId -eq $pi.Id) { $win = $el; break }
    }
  } catch { }
  if ($null -ne $win) { break }
  Start-Sleep -Milliseconds 400
}
if ($null -eq $win) { W 'RESULT=error no window'; if ($Kill) { Stop-ProcTree ([int]$pi.Id) }; exit 2 }
W "WINDOW=$(Get-AName $win)"
$settle = (Get-Date).AddSeconds(15)
while ((Get-Date) -lt $settle) { Start-Sleep -Milliseconds 400; if ((Get-Descendants $win 30).Count -ge 10) { break } }

# ---- dismiss onboarding welcome if present
foreach ($needle in @('继续', '开始使用', '我知道了', '关闭')) {
  $hits = Find-ByName $win $needle
  # only dialog primary buttons
  foreach ($h in $hits) {
    $aid = Get-Aid $h
    if ($aid -eq 'PrimaryButton' -or (Get-AName $h) -eq '继续') {
      Do-Invoke $h "dismiss-onboarding-$needle"
      Start-Sleep -Milliseconds 500
      break
    }
  }
}
# light dismiss
$ld = Find-ByName $win '关闭'
foreach ($h in $ld) { if ((Get-Aid $h) -eq '' -or (Get-AName $h) -eq '关闭') { } }

Dump-Ids $win 'after-onboarding'

# ---- open settings
Do-Invoke 'SettingsItem' 'open-settings'
Start-Sleep -Milliseconds 1000
Dump-Ids $win 'settings-open'
foreach ($id in @('SettingsHost','SettingsSection_about','SettingsSection_models','SettingsSection_general','RestoreDefaultModelButton','AboutCheckUpdateButton','Setting_permission_defaultPreset')) {
  Probe $id | Out-Null
}
Text-Hits '内测声明' | Out-Null
Text-Hits '预览版' | Out-Null
Text-Hits 'DSH 本地构建' | Out-Null
Text-Hits '恢复默认模型' | Out-Null
Text-Hits '品牌与声明' | Out-Null

# navigate sections
foreach ($want in @('SettingsSection_about','SettingsSection_models','SettingsSection_general','SettingsSection_personalization')) {
  $hits = Find-ByAid $win $want
  if ($hits.Count -gt 0) {
    Do-Invoke $hits[0] "nav-$want"
    Start-Sleep -Milliseconds 800
    Dump-Ids $win "section-$want"
    Probe 'RestoreDefaultModelButton' | Out-Null
    Probe 'AboutCheckUpdateButton' | Out-Null
    Text-Hits '内测声明' | Out-Null
    Text-Hits '恢复默认模型' | Out-Null
    Text-Hits '预览版' | Out-Null
    Text-Hits 'DSH 本地构建' | Out-Null
  } else {
    W "SECTION $want MISSING"
  }
}

# brute scan sections
foreach ($el in (Get-Descendants $win)) {
  $aid = Get-Aid $el
  if ($aid -like 'SettingsSection_*') {
    Do-Invoke $el "scan-$aid"
    Start-Sleep -Milliseconds 600
    $r = Find-ByAid $win 'RestoreDefaultModelButton'
    if ($r.Count -gt 0) { W "FOUND RestoreDefault under $aid"; Probe 'RestoreDefaultModelButton' | Out-Null; break }
    $b = Find-ByName $win '内测声明'
    if ($b.Count -gt 0) { W "FOUND 内测声明 under $aid" }
  }
}

# ---- back to chat, open files, try tree item -> preview
Do-Invoke 'ShellBackButton' 'back'
Start-Sleep -Milliseconds 400
# if back missing, try settings toggle
if ((Find-ByAid $win 'ShellBackButton').Count -eq 0) { Do-Invoke 'SettingsItem' 'toggle-settings-off' }
Start-Sleep -Milliseconds 400

Do-Invoke 'ComposerAddButton' 'composer'
Start-Sleep -Milliseconds 300
Do-Invoke '工作区文件' 'files-panel'
Start-Sleep -Milliseconds 800

foreach ($id in @('FilesPanelTree','PreviewRoot','RightPaneTabStrip','RightPaneAddTab','RightPaneCell0','RightPaneFullscreen','LayoutSplitterRightInner')) {
  Probe $id | Out-Null
}

# click first tree item if any
$treeItems = New-Object System.Collections.Generic.List[object]
foreach ($el in (Get-Descendants $win)) {
  $info = Get-ElInfo $el
  if ($null -eq $info) { continue }
  $ct = $info.ControlType.ProgrammaticName
  if ($ct -match 'TreeItem|ListItem' -and (Get-AName $el) -notmatch '会话|设置|工作区$|新建|未分组|展开') {
    $treeItems.Add($el)
  }
}
W "TREE-ITEM-CANDIDATES=$($treeItems.Count)"
foreach ($t in $treeItems | Select-Object -First 5) {
  W "  tree-item name=$(Get-AName $t) id=$(Get-Aid $t)"
}
if ($treeItems.Count -gt 0) {
  Do-Invoke $treeItems[0] 'open-tree-item'
  Start-Sleep -Milliseconds 1200
  Dump-Ids $win 'after-tree-item'
  foreach ($id in @('PreviewRoot','PreviewToolbar','PreviewWrapToggle','PreviewCopyAll','PreviewLoadMore','PreviewChangedBanner','PreviewMarkdown','PreviewImage','PreviewPdf','FilesPanelPreviewText','PreviewHtmlRelated')) {
    Probe $id | Out-Null
  }
  Do-Invoke 'PreviewWrapToggle' 'wrap'
  Probe 'PreviewWrapToggle' | Out-Null
  Do-Invoke 'PreviewCopyAll' 'copy'
  Do-Invoke 'PreviewLoadMore' 'more'
  # face texts
  Text-Hits '自动换行' | Out-Null
  Text-Hits '复制全文' | Out-Null
  Text-Hits '加载更多' | Out-Null
  Text-Hits '文件已更新' | Out-Null
}

# right pane tab menu / split
foreach ($el in (Get-Descendants $win)) {
  $aid = Get-Aid $el
  if ($aid -like 'RightPaneTab:*' -or $aid -eq 'RightPaneAddTab') {
    Do-Invoke $el "tab-$aid"
    Start-Sleep -Milliseconds 400
    Dump-Ids $win 'tab-menu'
    Probe 'RightPaneSplitHorizontal' | Out-Null
    Do-Invoke 'RightPaneSplitHorizontal' 'split-h'
    Start-Sleep -Milliseconds 600
    Probe 'RightPaneCell1' | Out-Null
    Probe 'LayoutSplitterRightInner' | Out-Null
    # second split should refuse
    foreach ($el2 in (Get-Descendants $win)) {
      if ((Get-Aid $el2) -like 'RightPaneTab:*') {
        Do-Invoke $el2 'reopen-tab'
        Start-Sleep -Milliseconds 300
        foreach ($s in (Find-ByAid $win 'RightPaneSplitHorizontal')) {
          W "SPLIT2 text=$(Get-AName $s) en=$(Get-Enabled $s)"
        }
        break
      }
    }
    break
  }
}

# subagent list
Do-Invoke 'SubagentList' 'subagent-list'
Start-Sleep -Milliseconds 600
Dump-Ids $win 'subagent-list'
Probe 'SubagentPrompt' | Out-Null
Probe 'SubagentInterrupt' | Out-Null
Text-Hits '续跑' | Out-Null
Text-Hits '打断' | Out-Null

# trajectory entry (session more / jobs?)
Text-Hits '轨迹' | Out-Null

Dump-Ids $win 'final'
W "LEFT-RUNNING pid=$($pi.Id)"
if ($Kill) { Stop-ProcTree ([int]$pi.Id); W 'KILLED' }
exit 0
