<#
  install-0800.ps1 - Install Blade2 0.8.0 (MSIX)

  Steps:
    1) Kill running Blade2 process tree + orphaned dsh kernel (strict matching)
    2) Verify msix signature
    3) Add-AppxPackage
    4) Report installed version / InstallLocation

  Usage:
    pwsh -File install-0800.ps1
    pwsh -File install-0800.ps1 -DryRun
    pwsh -File install-0800.ps1 -MsixPath <path>
#>
[CmdletBinding()]
param(
  [string]$MsixPath,
  [switch]$DryRun,
  [switch]$SkipSignatureCheck
)

$ErrorActionPreference = 'Stop'

$ExpectedVersion = '0.8.0.0'
$Thumbprint      = 'E2B4870249B661186E86F816E2A403261E74F6A0'
$PackageName     = 'Blade2'
$Signtool        = 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\signtool.exe'

function Write-Head($t) { Write-Host ''; Write-Host "=== $t ===" -ForegroundColor Cyan }
function Write-Ok($t)   { Write-Host "  [OK] $t" -ForegroundColor Green }
function Write-Warn2($t) { Write-Host "  [WARN] $t" -ForegroundColor Yellow }

# ---------- 0. Resolve msix path ----------
if (-not $MsixPath) {
  $candidate = Join-Path $PSScriptRoot "AppxPkgs0800\$PackageName`_0.8.0_x64_Test\$PackageName`_0.8.0_x64.msix"
  if (Test-Path -LiteralPath $candidate) {
    $MsixPath = $candidate
  } else {
    $found = Get-ChildItem -LiteralPath $PSScriptRoot -Recurse -Filter "*.msix" -ErrorAction SilentlyContinue |
             Where-Object { $_.Name -like "*0.8.0*" } | Select-Object -First 1
    if ($found) { $MsixPath = $found.FullName }
  }
}
if (-not $MsixPath -or -not (Test-Path -LiteralPath $MsixPath)) {
  throw "msix for 0.8.0 not found. Pass -MsixPath or run the Release build first."
}

Write-Head "Target msix"
$msixItem = Get-Item -LiteralPath $MsixPath
Write-Host "  Path: $($msixItem.FullName)"
Write-Host "  Size: $([math]::Round($msixItem.Length/1MB,2)) MB ($($msixItem.Length) bytes)"

# ---------- 1. Preflight ----------
Write-Head "Preflight"
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
             [Security.Principal.WindowsBuiltInRole]::Administrator)
Write-Host "  Admin session: $isAdmin (not required)"

$cert = Get-ChildItem Cert:\CurrentUser\My, Cert:\LocalMachine\My -ErrorAction SilentlyContinue |
        Where-Object { $_.Thumbprint -eq $Thumbprint } | Select-Object -First 1
if ($cert) { Write-Ok "Signing cert present: $($cert.Subject), expires $($cert.NotAfter.ToString('yyyy-MM-dd'))" }
else       { Write-Warn2 "No cert with thumbprint $Thumbprint found; signature check may fail" }

$trusted = @('Cert:\LocalMachine\TrustedPeople','Cert:\LocalMachine\Root','Cert:\CurrentUser\TrustedPeople') |
           Where-Object { Get-ChildItem $_ -ErrorAction SilentlyContinue | Where-Object { $_.Thumbprint -eq $Thumbprint } }
if ($trusted.Count -gt 0) { Write-Ok "Cert trusted: $($trusted -join ', ')" }
else { Write-Warn2 "Cert not in trusted stores - install may fail with 0x800B0109" }

if (-not $SkipSignatureCheck -and (Test-Path -LiteralPath $Signtool)) {
  $vout = & $Signtool verify /pa "$MsixPath" 2>&1
  if ($LASTEXITCODE -eq 0) { Write-Ok "signtool verify /pa passed" }
  else { Write-Warn2 "signtool verify /pa failed (exit $LASTEXITCODE): $($vout | Select-Object -First 2)" }
}

# ---------- 2. Kill Blade2 + kernel ----------
Write-Head "Kill leftover processes"

$allProc = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue
$reasons = @{}

$roots = @($allProc | Where-Object { $_.Name -eq 'Blade2.exe' })
foreach ($r in $roots) { $reasons[[int]$r.ProcessId] = 'A: Blade2 main process' }

$tree = New-Object 'System.Collections.Generic.HashSet[int]'
foreach ($r in $roots) { [void]$tree.Add([int]$r.ProcessId) }
do {
  $added = $false
  foreach ($p in $allProc) {
    $pid2 = [int]$p.ProcessId
    if (-not $tree.Contains($pid2) -and $tree.Contains([int]$p.ParentProcessId)) {
      [void]$tree.Add($pid2); $added = $true
    }
  }
} while ($added)
foreach ($id in $tree) { if (-not $reasons.ContainsKey($id)) { $reasons[$id] = 'A: Blade2 child tree' } }

foreach ($p in $allProc) {
  if ($p.Name -notin @('node.exe','cmd.exe')) { continue }
  $cl = $p.CommandLine
  if (-not $cl) { continue }
  if ($cl -match 'dsh' -and $cl -match 'web\s+--no-open\s+--port\s+0') {
    $id = [int]$p.ProcessId
    if (-not $reasons.ContainsKey($id)) { $reasons[$id] = 'B: dsh kernel cmdline signature' }
  }
}

$targets = @()
foreach ($id in $reasons.Keys) {
  $p = $allProc | Where-Object { [int]$_.ProcessId -eq $id } | Select-Object -First 1
  $name = if ($p) { $p.Name } else { '(exited)' }
  $targets += [pscustomobject]@{ PID = $id; Name = $name; Reason = $reasons[$id] }
}

if ($targets.Count -eq 0) {
  Write-Ok "No leftover processes"
} else {
  Write-Host "  Killing $($targets.Count) process(es):"
  $targets | Sort-Object Name, PID | Format-Table -AutoSize | Out-String | Write-Host
  if ($DryRun) {
    Write-Host '  -DryRun: listed only, nothing killed.' -ForegroundColor Yellow
  } else {
    $ordered = $targets | Sort-Object { if ($_.Reason -like 'A: Blade2 main*') { 0 } else { 1 } }
    foreach ($t in $ordered) {
      try { Stop-Process -Id $t.PID -Force -ErrorAction Stop; Write-Ok "Killed PID $($t.PID) ($($t.Name))" }
      catch { Write-Warn2 "PID $($t.PID) kill failed: $($_.Exception.Message)" }
    }
    Start-Sleep -Seconds 2
  }
}

# ---------- 3. Install ----------
Write-Head "Add-AppxPackage"
if ($DryRun) {
  Write-Host "  -DryRun: skipped install." -ForegroundColor Yellow
  return
}

$installedBefore = Get-AppxPackage -Name $PackageName -ErrorAction SilentlyContinue
if ($installedBefore) { Write-Host "  Before: $($installedBefore.Version)" }
else { Write-Host '  Before: not installed' }

try {
  Add-AppxPackage -Path $MsixPath -ErrorAction Stop
  Write-Ok 'Add-AppxPackage succeeded'
} catch {
  $hr = '0x{0:X8}' -f $_.Exception.HResult
  Write-Host "  [FAIL] $($_.Exception.Message)" -ForegroundColor Red
  Write-Host "  HRESULT: $hr"
  throw
}

# ---------- 4. Result ----------
Write-Head "Install result"
$pkg = Get-AppxPackage -Name $PackageName -ErrorAction SilentlyContinue
if (-not $pkg) { throw "$PackageName not found after install" }

Write-Host "  Name            : $($pkg.Name)"
Write-Host "  Version         : $($pkg.Version)"
Write-Host "  PackageFullName : $($pkg.PackageFullName)"
Write-Host "  InstallLocation : $($pkg.InstallLocation)"
Write-Host "  SignatureKind   : $($pkg.SignatureKind)"

if ("$($pkg.Version)" -eq $ExpectedVersion) { Write-Ok "Version matches expected $ExpectedVersion" }
else { Write-Warn2 "Version $($pkg.Version) != expected $ExpectedVersion" }

$appId = ($pkg | Get-AppxPackageManifest).Package.Applications.Application.Id
Write-Host ''
Write-Host '  Launch:'
Write-Host "    explorer.exe shell:AppsFolder\$($pkg.PackageFamilyName)!$appId"
