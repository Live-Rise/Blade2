# build-0800.ps1 - Release MSIX build for Blade2 0.8.2 (ASCII only)
# 路径一律按脚本所在目录解析，换机器/clone 后可直接跑；仅 MSBuild 与 signtool
# 从常见安装位置发现（vswhere / 最新 Windows Kits），找不到时回退默认路径。
$ErrorActionPreference = 'Stop'
$env:DOTNET_ROOT = 'C:\Program Files\dotnet'
$env:PATH = "C:\Program Files\dotnet;$env:PATH"

$root = $PSScriptRoot
$proj = Join-Path $root 'Blade2.csproj'
$pkgDir = Join-Path $root 'AppxPkgs0800'
# AppxPackageDir 必须以分隔符结尾（命令行 /p: 全局属性项目内改不动），正斜杠最稳
$pkgDirArg = $pkgDir.Replace('\', '/') + '/'

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$msbuild = $null
if (Test-Path $vswhere) {
  # 不能加 -requires Microsoft.Component.MSBuild：实测会把只装了部分工作负载的
  # VS 实例整个过滤掉（本机即如此）；-latest -products * 足以定位最新实例。
  $msbuild = & $vswhere -latest -products * -find 'MSBuild\Current\Bin\MSBuild.exe' | Select-Object -First 1
}
if (-not $msbuild) {
  $msbuild = @(
    'C:\Program Files\Microsoft Visual Studio\2022\Community\MSBuild\Current\Bin\MSBuild.exe',
    'C:\Program Files\Microsoft Visual Studio\2022\Professional\MSBuild\Current\Bin\MSBuild.exe',
    'C:\Program Files\Microsoft Visual Studio\2022\Enterprise\MSBuild\Current\Bin\MSBuild.exe',
    'C:\Program Files\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\MSBuild.exe'
  ) | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $msbuild) { throw 'MSBuild not found: install Visual Studio 2022 MSBuild (any edition or Build Tools)' }

$kitsBin = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
$signtool = Get-ChildItem $kitsBin -Directory -ErrorAction SilentlyContinue |
  Sort-Object Name -Descending |
  ForEach-Object { Join-Path $_.FullName 'x64\signtool.exe' } |
  Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $signtool) { $signtool = Join-Path $kitsBin '10.0.22621.0\x64\signtool.exe' }
if (-not (Test-Path $signtool)) { throw "signtool not found: $signtool" }

Write-Host '=== Restore ==='
& $msbuild $proj /t:Restore /p:Configuration=Release /p:Platform=x64 /v:m /nologo
if ($LASTEXITCODE -ne 0) { throw "restore failed: $LASTEXITCODE" }

Write-Host '=== Build + Package ==='
& $msbuild $proj /t:Build /p:Configuration=Release /p:Platform=x64 `
  /p:AppxPackageDir=$pkgDirArg `
  /p:GenerateAppxPackageOnBuild=true /p:AppxBundle=Never /v:m /nologo
if ($LASTEXITCODE -ne 0) { throw "build failed: $LASTEXITCODE" }

Write-Host '=== Sign ==='
# 包身份已从 DshWinUI 改名为 Blade2（0.8.1）；Publisher 仍为 CN=DshWinUI，证书不变。
$msix = Get-ChildItem $pkgDir -Recurse -Filter '*_0.8.2_x64*.msix' | Select-Object -First 1
if (-not $msix) { $msix = Get-ChildItem $pkgDir -Recurse -Filter '*.msix' | Where-Object { $_.FullName -like '*0.8.2*' } | Sort-Object LastWriteTime -Descending | Select-Object -First 1 }
if (-not $msix) { throw 'no msix produced' }
$signtool = 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\signtool.exe'
& $signtool sign /fd SHA256 /sha1 E2B4870249B661186E86F816E2A403261E74F6A0 $msix.FullName
if ($LASTEXITCODE -ne 0) { throw "sign failed: $LASTEXITCODE" }
& $signtool verify /pa $msix.FullName
Write-Host "DONE: $($msix.FullName)"
