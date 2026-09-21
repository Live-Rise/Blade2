# build-0800.ps1 - Release MSIX build for Blade2 0.8.0 (ASCII only)
$ErrorActionPreference = 'Stop'
$env:DOTNET_ROOT = 'C:\Program Files\dotnet'
$env:PATH = "C:\Program Files\dotnet;$env:PATH"
$msbuild = 'C:\Program Files\Microsoft Visual Studio\2022\Community\MSBuild\Current\Bin\MSBuild.exe'
$proj = 'E:\Syncthing\DshWinUI\Blade2.csproj'

Write-Host '=== Restore ==='
& $msbuild $proj /t:Restore /p:Configuration=Release /p:Platform=x64 /v:m /nologo
if ($LASTEXITCODE -ne 0) { throw "restore failed: $LASTEXITCODE" }

Write-Host '=== Build + Package ==='
& $msbuild $proj /t:Build /p:Configuration=Release /p:Platform=x64 `
  /p:AppxPackageDir=E:/Syncthing/DshWinUI/AppxPkgs0800/ `
  /p:GenerateAppxPackageOnBuild=true /p:AppxBundle=Never /v:m /nologo
if ($LASTEXITCODE -ne 0) { throw "build failed: $LASTEXITCODE" }

Write-Host '=== Sign ==='
# 包身份已从 DshWinUI 改名为 Blade2（0.8.0）；Publisher 仍为 CN=DshWinUI，证书不变。
$msix = Get-ChildItem 'E:\Syncthing\DshWinUI\AppxPkgs0800' -Recurse -Filter '*_0.8.0_x64*.msix' | Select-Object -First 1
if (-not $msix) { $msix = Get-ChildItem 'E:\Syncthing\DshWinUI\AppxPkgs0800' -Recurse -Filter '*.msix' | Where-Object { $_.FullName -like '*0.8.0*' } | Sort-Object LastWriteTime -Descending | Select-Object -First 1 }
if (-not $msix) { throw 'no msix produced' }
$signtool = 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\signtool.exe'
& $signtool sign /fd SHA256 /sha1 E2B4870249B661186E86F816E2A403261E74F6A0 $msix.FullName
if ($LASTEXITCODE -ne 0) { throw "sign failed: $LASTEXITCODE" }
& $signtool verify /pa $msix.FullName
Write-Host "DONE: $($msix.FullName)"
