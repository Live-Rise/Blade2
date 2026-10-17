<#
  install-msix.ps1 - 安装 rust 分叉（blade2-rs）的 MSIX。

  镜像主干 install-0800.ps1：杀残留 → 验签 → Add-AppxPackage → 报版本/装机位/启动命令。
  与主干的差异只在**目标包名与进程名**（Blade2Rs / blade2-rs.exe），其余口径同一套。

  用法：
    pwsh -File install-msix.ps1
    pwsh -File install-msix.ps1 -DryRun
    pwsh -File install-msix.ps1 -MsixPath <path>
#>
[CmdletBinding()]
param(
  [string]$MsixPath,
  [switch]$DryRun,
  [switch]$SkipSignatureCheck
)

$ErrorActionPreference = 'Stop'

$ExpectedVersion = '0.8.3.9'
$Thumbprint      = 'E2B4870249B661186E86F816E2A403261E74F6A0'
$PackageName     = 'Blade2Rs'
$ConsoleProc     = 'blade2-rs.exe'
$Signtool        = 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\signtool.exe'

function Write-Head($t) { Write-Host ''; Write-Host "=== $t ===" -ForegroundColor Cyan }
function Write-Ok($t)   { Write-Host "  [OK] $t" -ForegroundColor Green }
function Write-Warn2($t) { Write-Host "  [WARN] $t" -ForegroundColor Yellow }

# ---------- 0. 定位 msix ----------
if (-not $MsixPath) {
  $candidate = Join-Path $PSScriptRoot ('AppxPkgsRs\{0}_{1}_x64.msix' -f $PackageName, $ExpectedVersion)
  if (Test-Path -LiteralPath $candidate) { $MsixPath = $candidate }
}
if (-not $MsixPath -or -not (Test-Path -LiteralPath $MsixPath)) {
  $found = Get-ChildItem -LiteralPath $PSScriptRoot -Recurse -Filter '*.msix' -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($found) { $MsixPath = $found.FullName }
}
if (-not $MsixPath -or -not (Test-Path -LiteralPath $MsixPath)) {
  throw "没找到分叉 msix。先跑 build-msix.ps1，或用 -MsixPath 指定。"
}

Write-Head 'Target msix'
$msixItem = Get-Item -LiteralPath $MsixPath
Write-Host "  Path: $($msixItem.FullName)"
Write-Host "  Size: $([math]::Round($msixItem.Length/1MB,2)) MB ($($msixItem.Length) bytes)"

# ---------- 1. 预检 ----------
Write-Head 'Preflight'

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

# ---------- 2. 杀残留 ----------
Write-Head 'Kill leftover processes'

$allProc = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue
$reasons = @{}

$roots = @($allProc | Where-Object { $_.Name -eq $ConsoleProc })
foreach ($r in $roots) { $reasons[[int]$r.ProcessId] = 'A: blade2-rs main process' }

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
foreach ($id in $tree) { if (-not $reasons.ContainsKey($id)) { $reasons[$id] = 'A: blade2-rs child tree' } }

# 孤儿内核：命令行带 dsh + web --no-open --port 0 的 node。分叉的孤儿清扫本身认
# bundled_kernel_exe()（装机目录下的那颗），这里多扫一刀命令行，与主干同口径。
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
    $ordered = $targets | Sort-Object { if ($_.Reason -like 'A: blade2-rs main*') { 0 } else { 1 } }
    foreach ($t in $ordered) {
      try { Stop-Process -Id $t.PID -Force -ErrorAction Stop; Write-Ok "Killed PID $($t.PID) ($($t.Name))" }
      catch { Write-Warn2 "PID $($t.PID) kill failed: $($_.Exception.Message)" }
    }
    Start-Sleep -Seconds 2
  }
}

# ---------- 3. 装机 ----------
Write-Head 'Add-AppxPackage'
if ($DryRun) {
  Write-Host "  -DryRun: skipped install." -ForegroundColor Yellow
  return
}

$installedBefore = @(Get-AppxPackage -Name $PackageName -ErrorAction SilentlyContinue)
if ($installedBefore.Count -gt 0) {
  Write-Host "  Before: $($installedBefore[0].Version) (per-user $($installedBefore.Count) 份)"
  # 同名同版本换内容会被 0x80073CFB 挡；先卸再装。**只卸我们这个包**（Blade2Rs），
  # 不碰主干的 Blade2，也不会动 %LOCALAPPDATA%\Blade2 数据家（在包外，重装后照旧能用）。
  foreach ($p in $installedBefore) {
    Write-Host "  Remove-AppxPackage $($p.PackageFullName)"
    try { Remove-AppxPackage -Package $p.PackageFullName -ErrorAction Stop; Write-Ok 'removed' }
    catch { Write-Warn2 "remove failed: $($_.Exception.Message)" }
  }
} else {
  Write-Host '  Before: not installed'
}

try {
  Add-AppxPackage -Path $MsixPath -ErrorAction Stop
  Write-Ok 'Add-AppxPackage succeeded'
} catch {
  $hr = '0x{0:X8}' -f $_.Exception.HResult
  Write-Host "  [FAIL] $($_.Exception.Message)" -ForegroundColor Red
  Write-Host "  HRESULT: $hr"
  if ($installedBefore.Count -gt 0) {
    Write-Host "  已装着旧版：先 pwsh -NoProfile -Command `"Remove-AppxPackage -Package $($installedBefore[0].PackageFullName)`"" -ForegroundColor Yellow
  }
  throw
}

# ---------- 4. 结果 ----------
Write-Head 'Install result'
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
