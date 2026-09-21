# 壳构建（Debug/Release，x64）：正常 Windows 会话直接可用；本仓库偶尔在"环境变量被裁剪"的
# 宿主 shell（某些自动化/沙箱 shell 不含 PROCESSOR_ARCHITECTURE、ProgramData、ProgramFiles 等）里
# 执行时，MSBuild 的两个环节会失败：
#   · NuGet restore → "Value cannot be null. (Parameter 'path1')"（机器级 NuGet 配置目录 = %ProgramData%）
#   · MSIX 配方生成 → WinAppSdkGenerateAppxPackageRecipe NullReferenceException（MrmSupport 定位依赖 PROCESSOR_ARCHITECTURE）
# 本脚本只补齐缺失的标准变量，再执行与基线完全一致的命令。
# 用法：pwsh -File build.ps1 [-Configuration Debug|Release|Both]
param([string]$Configuration = 'Debug')

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$dotnet = 'C:\Program Files\dotnet\dotnet.exe'

# 缺失才补（已有值不覆盖）
$defaults = [ordered]@{
  'PROCESSOR_ARCHITECTURE' = 'AMD64'
  'ProgramData'            = 'C:\ProgramData'
  'ProgramFiles'           = 'C:\Program Files'
  'ProgramFiles(x86)'      = 'C:\Program Files (x86)'
  'CommonProgramFiles'     = 'C:\Program Files\Common Files'
  'ALLUSERSPROFILE'        = 'C:\ProgramData'
}
foreach ($kv in $defaults.GetEnumerator()) {
  if (-not [Environment]::GetEnvironmentVariable($kv.Key, 'Process')) {
    [Environment]::SetEnvironmentVariable($kv.Key, $kv.Value, 'Process')
    Write-Host "[env] $($kv.Key) = $($kv.Value)"
  }
}

$configs = if ($Configuration -eq 'Both') { @('Debug', 'Release') } else { @($Configuration) }
$failed = 0
foreach ($cfg in $configs) {
  Write-Host "=== build $cfg x64 ==="
  & $dotnet build (Join-Path $root 'Blade2.csproj') -c $cfg -p:Platform=x64
  if ($LASTEXITCODE -ne 0) { $failed++ }
}
exit $failed
