<#
  T36 focused UIA follow-up: permission gate + trajectory/cordis panels (popup-root aware).
  Usage: attach to running Blade2, or pass -Launch.
#>
[CmdletBinding()]
param(
  [string]$Exe = 'E:\Syncthing\DshWinUI\bin\x64\Debug\net8.0-windows10.0.22621.0\win-x64\Blade2.exe',
  [switch]$Launch,
  [switch]$Kill
)
$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes

$AE = [System.Windows.Automation.AutomationElement]
$TS = [System.Windows.Automation.TreeScope]
$TC = [System.Windows.Automation.Condition]::TrueCondition
$AID = [System.Windows.Automation.AutomationElementIdentifiers]::AutomationIdProperty
$NAME = [System.Windows.Automation.AutomationElementIdentifiers]::NameProperty

function Get-Info($e) { try { $e.Current } catch { $null } }
function Get-Aid($e) { try { [string]$e.GetCurrentPropertyValue($AID) } catch { '' } }
function Get-An($e) { try { [string]$e.GetCurrentPropertyValue($NAME) } catch { '' } }
function Get-Kids($r, $c = 4000) {
  $l = New-Object System.Collections.Generic.List[object]
  try {
    $col = $r.FindAll($TS::Descendants, $TC)
    $n = [Math]::Min($col.Count, $c)
    for ($i = 0; $i -lt $n; $i++) { $l.Add($col[$i]) }
  } catch {}
  return $l
}
function Find-IdDeep($win, $procId, $id) {
  $hits = New-Object System.Collections.Generic.List[object]
  foreach ($e in (Get-Kids $win)) { if ((Get-Aid $e) -eq $id) { $hits.Add($e) } }
  if ($hits.Count -gt 0) { return $hits }
  try {
    $col = $AE::RootElement.FindAll($TS::Children, $TC)
    for ($i = 0; $i -lt $col.Count; $i++) {
      $el = $col[$i]
      $inf = Get-Info $el
      if ($null -eq $inf -or $inf.ProcessId -ne $procId) { continue }
      foreach ($e in (Get-Kids $el)) { if ((Get-Aid $e) -eq $id) { $hits.Add($e) } }
      if ($hits.Count -gt 0) { return $hits }
    }
  } catch {}
  return $hits
}
function Invoke-Main($e, $tag) {
  # menu items: prefer Invoke then Select; never Toggle (Toggle does not raise Click)
  foreach ($p in @(
      @{ n = 'Invoke'; id = [System.Windows.Automation.InvokePatternIdentifiers]::Pattern },
      @{ n = 'Select'; id = [System.Windows.Automation.SelectionItemPatternIdentifiers]::Pattern }
    )) {
    $pat = $null
    try { $pat = $e.GetCurrentPattern($p.id) } catch { $pat = $null }
    if ($null -eq $pat) { continue }
    try {
      if ($p.n -eq 'Invoke') { [void]([System.Windows.Automation.InvokePattern]$pat).Invoke() }
      else { [void]([System.Windows.Automation.SelectionItemPattern]$pat).Select() }
      Write-Host "SEL $tag OK via=$($p.n)"
      Start-Sleep -Milliseconds 1800
      return $true
    } catch {
      Write-Host "SEL $tag fail via=$($p.n) $($_.Exception.Message)"
      continue
    }
  }
  Write-Host "SEL $tag NO-PATTERN"
  return $false
}

if ($Launch) {
  Get-Process Blade2 -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Sleep -Milliseconds 500
  $null = Start-Process -FilePath $Exe
  Start-Sleep -Seconds 8
}

$pi = Get-Process Blade2 -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $pi) { Write-Host 'NO-APP'; exit 1 }
$win = $null
$col = $AE::RootElement.FindAll($TS::Children, $TC)
for ($i = 0; $i -lt $col.Count; $i++) {
  $el = $col[$i]
  $inf = Get-Info $el
  if ($inf -and $inf.ProcessId -eq $pi.Id) { $win = $el; break }
}
if (-not $win) { Write-Host 'NO-WIN'; exit 2 }
Write-Host "WIN=$($win.Current.Name) pid=$($pi.Id)"

# wait for kernel
$deadline = (Get-Date).AddSeconds(90)
while ((Get-Date) -lt $deadline) {
  $panel = Find-IdDeep $win $pi.Id 'KernelBootPanel'
  if ($panel.Count -eq 0) { Write-Host 'BOOT=ready'; break }
  Start-Sleep -Milliseconds 500
}

# open first session so CapabilitySession works
$sess = $null
foreach ($e in (Get-Kids $win)) {
  if ((Get-Aid $e) -like 'Session_*') { $sess = $e; break }
}
if ($sess) { [void](Invoke-Main $sess 'open-session') }

# ---- permission gate ----
$pb = Find-IdDeep $win $pi.Id 'PermissionButton'
if ($pb.Count -gt 0) { [void](Invoke-Main $pb[0] 'permission-button') }
Start-Sleep -Milliseconds 600
$preset = Find-IdDeep $win $pi.Id 'PermissionPreset_danger-full-access'
Write-Host "PRESET count=$($preset.Count)"
if ($preset.Count -gt 0) { [void](Invoke-Main $preset[0] 'danger-preset') }
Start-Sleep -Milliseconds 1200
$risk = Find-IdDeep $win $pi.Id 'RiskAckCheckBox'
$confirm = Find-IdDeep $win $pi.Id 'ConfirmDangerFullAccessButton'
Write-Host "RISK=$($risk.Count) CONFIRM=$($confirm.Count)"
if ($confirm.Count -gt 0) {
  $c = Get-Info $confirm[0]
  Write-Host "CONFIRM en=$($c.IsEnabled) name=$($c.Name)"
}
if ($risk.Count -gt 0) {
  [void](Invoke-Main $risk[0] 'risk-ack')
  Start-Sleep -Milliseconds 500
  $confirm2 = Find-IdDeep $win $pi.Id 'ConfirmDangerFullAccessButton'
  if ($confirm2.Count -gt 0) {
    $c2 = Get-Info $confirm2[0]
    Write-Host "CONFIRM-AFTER-ACK en=$($c2.IsEnabled)"
  }
  # cancel via name
  foreach ($e in (Get-Kids $win)) {
    if ((Get-An $e) -eq '取消') { [void](Invoke-Main $e 'cancel'); break }
  }
  Start-Sleep -Milliseconds 500
  $riskAfter = Find-IdDeep $win $pi.Id 'RiskAckCheckBox'
  Write-Host "RISK-AFTER-CANCEL=$($riskAfter.Count) (expect 0)"
}

# ---- trajectory ----
$ca = Find-IdDeep $win $pi.Id 'ComposerAddButton'
if ($ca.Count -gt 0) { [void](Invoke-Main $ca[0] 'composer') }
Start-Sleep -Milliseconds 600
$ti = Find-IdDeep $win $pi.Id 'TrajectoryMenuItem'
Write-Host "TRAJ-ITEM=$($ti.Count)"
if ($ti.Count -gt 0) { [void](Invoke-Main $ti[0] 'trajectory-item') }
Start-Sleep -Milliseconds 2500
$tp = Find-IdDeep $win $pi.Id 'TrajectoryPanel'
$tt = Find-IdDeep $win $pi.Id 'TrajectoryTiming'
$tl = Find-IdDeep $win $pi.Id 'TrajectoryLedger'
Write-Host "TRAJ-PANEL=$($tp.Count) TIMING=$($tt.Count) LEDGER=$($tl.Count)"
if ($tl.Count -gt 0) {
  $kids = Get-Kids $tl[0] 200
  Write-Host "TRAJ-LEDGER-KIDS=$($kids.Count)"
}
foreach ($e in (Get-Kids $win)) {
  $a = Get-Aid $e
  if ($a -match 'Traj') { Write-Host "TRAJ-ID $a name=$(Get-An $e)" }
}

# ---- cordis ----
$ca = Find-IdDeep $win $pi.Id 'ComposerAddButton'
if ($ca.Count -gt 0) { [void](Invoke-Main $ca[0] 'composer2') }
Start-Sleep -Milliseconds 600
$ci = Find-IdDeep $win $pi.Id 'CordisMenuItem'
Write-Host "CORDIS-ITEM=$($ci.Count)"
if ($ci.Count -gt 0) { [void](Invoke-Main $ci[0] 'cordis-item') }
Start-Sleep -Milliseconds 2500
$cp = Find-IdDeep $win $pi.Id 'CordisPanel'
Write-Host "CORDIS-PANEL=$($cp.Count)"
foreach ($e in (Get-Kids $win)) {
  $a = Get-Aid $e
  if ($a -match 'Cordis') { Write-Host "CORDIS-ID $a name=$(Get-An $e)" }
}

if ($Kill) {
  Get-Process Blade2 -ErrorAction SilentlyContinue | Stop-Process -Force
  Write-Host 'KILLED'
}
exit 0
