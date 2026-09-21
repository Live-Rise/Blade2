<#
  run-dev.ps1 —— 免安装预览：直接启动本轮 Debug 构建的未打包 Blade2.exe

  用途：不装 MSIX、不动系统，直接看本轮 UI 改动的实际效果。
  启动前会先结束同名的残留进程（严格匹配，绝不误杀其它 node）。

  用法：
    pwsh -File run-dev.ps1
    pwsh -File run-dev.ps1 -DryRun     # 只列出将被结束的进程
#>
[CmdletBinding()]
param(
  [string]$ExePath,
  [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

$ExeName = 'Blade2.exe'

function Write-Head($t) { Write-Host ''; Write-Host "=== $t ===" -ForegroundColor Cyan }
function Write-Ok($t)   { Write-Host "  [OK] $t" -ForegroundColor Green }
function Write-Warn2($t) { Write-Host "  [警告] $t" -ForegroundColor Yellow }

# ---------- 0. 定位 exe ----------
if (-not $ExePath) {
  $candidate = Join-Path $PSScriptRoot "bin\x64\Debug\net8.0-windows10.0.22621.0\win-x64\$ExeName"
  if (Test-Path -LiteralPath $candidate) { $ExePath = $candidate }
}
if (-not $ExePath -or -not (Test-Path -LiteralPath $ExePath)) {
  throw "未找到 Debug 未打包 exe。请先跑：pwsh -File build.ps1 -Configuration Debug
       或用 -ExePath 指定路径。"
}

Write-Head "目标程序"
$exeItem = Get-Item -LiteralPath $ExePath
Write-Host "  路径: $($exeItem.FullName)"
Write-Host "  构建时间: $($exeItem.LastWriteTime)   大小: $($exeItem.Length) bytes"

# ---------- 1. 结束残留 ----------
# 与 install-074.ps1 同一套匹配策略：A 子树 + B 命令行签名兜底。
# 本机常驻 node.exe 均为自动化宿主自身进程（命令行不含 dsh 签名），不会被命中。
Write-Head "结束残留进程"

$allProc = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue
$reasons = @{}

$roots = @($allProc | Where-Object { $_.Name -eq $ExeName })
foreach ($r in $roots) { $reasons[[int]$r.ProcessId] = 'A: Blade2 主进程' }

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
foreach ($id in $tree) { if (-not $reasons.ContainsKey($id)) { $reasons[$id] = 'A: Blade2 子进程树' } }

foreach ($p in $allProc) {
  if ($p.Name -notin @('node.exe','cmd.exe')) { continue }
  $cl = $p.CommandLine
  if (-not $cl) { continue }
  if ($cl -match 'dsh' -and $cl -match 'web\s+--no-open\s+--port\s+0') {
    $id = [int]$p.ProcessId
    if (-not $reasons.ContainsKey($id)) { $reasons[$id] = 'B: dsh 内核命令行签名' }
  }
}

$targets = @()
foreach ($id in $reasons.Keys) {
  $p = $allProc | Where-Object { [int]$_.ProcessId -eq $id } | Select-Object -First 1
  $name = if ($p) { $p.Name } else { '(已退出)' }
  $targets += [pscustomobject]@{ PID = $id; Name = $name; Reason = $reasons[$id] }
}

if ($targets.Count -eq 0) {
  Write-Ok "无残留进程需要结束"
} else {
  Write-Host "  将结束 $($targets.Count) 个进程："
  $targets | Sort-Object Name, PID | Format-Table -AutoSize | Out-String | Write-Host
  if ($DryRun) {
    Write-Host '  -DryRun：仅列出，未结束任何进程。' -ForegroundColor Yellow
  } else {
    $ordered = $targets | Sort-Object { if ($_.Reason -like 'A: Blade2 主进程*') { 0 } else { 1 } }
    foreach ($t in $ordered) {
      try { Stop-Process -Id $t.PID -Force -ErrorAction Stop; Write-Ok "已结束 PID $($t.PID) ($($t.Name))" }
      catch { Write-Warn2 "PID $($t.PID) 结束失败: $($_.Exception.Message)" }
    }
    Start-Sleep -Seconds 2
  }
}

if ($DryRun) {
  Write-Host ''
  Write-Host '  -DryRun：未启动程序。' -ForegroundColor Yellow
  return
}

# ---------- 2. 启动 ----------
Write-Head "启动"
$p = Start-Process -FilePath $ExePath -WorkingDirectory (Split-Path -Parent $ExePath) -PassThru
Start-Sleep -Seconds 4

$alive = Get-Process -Id $p.Id -ErrorAction SilentlyContinue
if ($alive) {
  Write-Ok "已启动 PID $($p.Id)"
  Write-Host ''
  Write-Host '  说明：未打包形态无包标识，内核按 DshKernelHost 的解析顺序回退到'
  Write-Host '        %APPDATA%\npm\dsh.cmd（仓库内无 Kernel\ 目录，故不含内置内核）。'
} else {
  Write-Warn2 "进程 PID $($p.Id) 已退出（退出码 $($p.ExitCode)）。可查看事件日志或直接手动运行 exe 排查。"
}
