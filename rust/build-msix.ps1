<#
  build-msix.ps1 - 给 rust 分叉（blade2-rs）打 MSIX。

  镜像主干 build-0800.ps1 的路线（makeappx pack + signtool sign），产物交给
  install-msix.ps1 装机。分叉与主干的打包差异只有三条，且都是必要的：

    1. Identity Name=Blade2Rs：与主干的 Blade2 **并存安装**（Publisher 复用同一个自签证书
       CN=DshWinUI ⇒ 信任链不用重建）。数据家两边都是 %LOCALAPPDATA%\Blade2
       （kernel.rs 的 DATA_HOME_DIR），这是既有分叉行为、与主干一致。
    2. 不带 toast 的 com:Extension / desktop:Extension：分叉只发托盘气球
       （tray.rs 的 Shell_NotifyIconW + NIF_INFO），没有 AppNotification COM 激活器那套，
       manifest 里加了反而是死配置。
    3. 不带 Assets\i18n\*.json：Catalog::load 只在传入 assets 目录时才读 json，
       运行时从不传（zh/en 走键直返），带了是 28 份死重。

  用法：
    pwsh -File build-msix.ps1                        # 0.8.3.9 + target/release/blade2-rs.exe
    pwsh -File build-msix.ps1 -ExePath <exe>         # 指定别的 release exe
    pwsh -File build-msix.ps1 -SkipSign              # 只 pack 不签（调试用）
    pwsh -File build-msix.ps1 -WithRuntime           # 把 WinAppSDK 运行期 DLL 一并打进包
#>
[CmdletBinding()]
param(
  [string]$Version = '0.8.3.9',
  [string]$ExePath,
  [switch]$SkipSign,
  [switch]$WithRuntime
)

$ErrorActionPreference = 'Stop'

$RepoRoot    = Split-Path -Parent $PSScriptRoot          # rust\ 的上一级 = 仓根
$PackageName = 'Blade2Rs'
$Publisher   = 'CN=DshWinUI'
$DisplayName = 'Blade² (rust)'
$Thumbprint  = 'E2B4870249B661186E86F816E2A403261E74F6A0'
$ConsoleName = 'blade2-rs.exe'

$OutRoot = Join-Path $PSScriptRoot ('AppxPkgsRs\{0}_{1}_x64_Test' -f $PackageName, $Version)
$MsixOut = Join-Path (Split-Path -Parent $OutRoot) ('{0}_{1}_x64.msix' -f $PackageName, $Version)

function Write-Head($t) { Write-Host ''; Write-Host "=== $t ===" -ForegroundColor Cyan }
function Write-Ok($t)   { Write-Host "  [OK] $t" -ForegroundColor Green }
function Write-Warn2($t) { Write-Host "  [WARN] $t" -ForegroundColor Yellow }

# ---------- 0. 定位工具 ----------
Write-Head 'Locate tools'

$KitsBin = 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64'
$MakeAppx = Join-Path $KitsBin 'makeappx.exe'
$Signtool = Join-Path $KitsBin 'signtool.exe'
foreach ($tool in @($MakeAppx, $Signtool)) {
  if (-not (Test-Path -LiteralPath $tool)) { throw "缺少 $tool —— Windows SDK 10.0.22621 的工具链" }
}
Write-Ok "makeappx : $MakeAppx"
Write-Ok "signtool : $Signtool"

# ---------- 1. exe 与版本号交叉核对 ----------
Write-Head 'Inputs'

if (-not $ExePath) { $ExePath = Join-Path $PSScriptRoot 'target\release\blade2-rs.exe' }
if (-not (Test-Path -LiteralPath $ExePath)) { throw "找不到分叉 exe：$ExePath（先 cargo build --release --bin blade2-rs）" }
$exe = Get-Item -LiteralPath $ExePath
Write-Ok ("exe      : {0} ({1:N0} bytes)" -f $exe.FullName, $exe.Length)

# 版本号三处同源：main.rs 的 SHELL_VERSION、包版本、目录名。这里读源码而不是手抄 ——
# 抄漏一巴掌拍不响（0.8.3.9 那版就是版本不符引发的连锁病）。
$shellVersion = (Select-String -LiteralPath (Join-Path $PSScriptRoot 'src\main.rs') -Pattern 'const SHELL_VERSION' |
  Select-Object -First 1).Line
if ($shellVersion -notmatch '"([0-9]+\.[0-9]+\.[0-9]+(?:\.[0-9]+)?)"') { throw "main.rs 里没解析出 SHELL_VERSION：$shellVersion" }
$shellVersion = $Matches[1]
if ($shellVersion -ne $Version) { throw "版本不符：本脚本 -Version $Version ≠ main.rs SHELL_VERSION $shellVersion" }
Write-Ok "version  : $Version（= main.rs SHELL_VERSION）"

# 装载物：Kernel 在仓根（rust/ 下没有），exe 旁的 Assets 也在仓根。
$KernelSrc = Join-Path $RepoRoot 'Kernel'
$AssetsSrc = Join-Path $RepoRoot 'Assets'
foreach ($d in @($KernelSrc, $AssetsSrc)) {
  if (-not (Test-Path -LiteralPath $d)) { throw "找不到装载物：$d" }
}
Write-Ok "Kernel   : $KernelSrc"
Write-Ok "Assets   : $AssetsSrc"

# ---------- 2. 暂存目录 ----------
Write-Head 'Stage'

if (Test-Path -LiteralPath $OutRoot) {
  # 不清空（不删目录）：只允许「本来就只该有这些」的暂存区继续用；出现任何意外项就停，
  # 让人先看一眼再决定，免得 makeappx 把别的东西打进包。
  $extra = Get-ChildItem -LiteralPath $OutRoot -Force | Where-Object {
    -not ($_.Name -in @('AppxManifest.xml', 'Assets', 'Kernel') -or $_.Name -eq $ConsoleName -or
          ($_.Name -like 'Microsoft.*.dll' -and $WithRuntime) -or $_.Name -eq 'Microsoft.UI.Xaml')
  }
  if ($extra) {
    throw ("暂存区里有预期外的项，先人工确认：`n  " + (($extra | ForEach-Object { $_.Name }) -join "`n  ") + "`n目录：$OutRoot")
  }
  Write-Ok "复用既有暂存区（幂等覆盖，不删任何东西）"
} else {
  New-Item -ItemType Directory -Path $OutRoot -Force | Out-Null
  Write-Ok "新建暂存区"
}

# ---------- 3. 写 AppxManifest ----------
Write-Head 'AppxManifest'

$manifest = @"
<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  xmlns:desktop6="http://schemas.microsoft.com/appx/manifest/desktop/windows10/6">

  <!-- Identity Name 与主干 Blade2 不同 ⇒ 两台壳可同时装机；Publisher 同一个自签证书。 -->
  <Identity Name="$PackageName" Publisher="$Publisher" Version="$Version" ProcessorArchitecture="x64" />

  <Properties>
    <DisplayName>$DisplayName</DisplayName>
    <PublisherDisplayName>DshWinUI</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
    <!-- 关 %LOCALAPPDATA% 写虚拟化：数据家 %LOCALAPPDATA%\Blade2 必须落真实路径
         （凭据/会话/插件 profile 全在真家），否则内核引导器新建的文件被重定向进
         Packages 包家、与既有数据分裂。与主干 Package.appxmanifest 同一条。 -->
    <desktop6:FileSystemWriteVirtualization>disabled</desktop6:FileSystemWriteVirtualization>
  </Properties>

  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.19041.0" MaxVersionTested="10.0.26100.0" />
    <!-- Windows App SDK 框架包依赖。分叉是原生 Rust 进程，reactor 的 is_packaged_process() 对
         打包进程**刻意跳过** bootstrap（bootstrap.rs：TryCreatePackageDependency/AddPackageDependency
         在打包进程里不可用），所以打包形态下唯一拿到 WinUI 类注册的路子就是这条声明 —— 它是
         reactor 那对运行时 API 的**清单静态版**：框架包进到进程的包图里，它 AppxManifest 里
         那份 ActivatableClass 注册（Extensions）才随图可见。少了它实测直接
         REGDB_E_CLASSNOTREG(0x80040154)、壳起不来。MinVersion 取 reactor 自己钉的 2.4.0.0
         （bindings.rs 的 WINDOWSAPPSDK_RUNTIME_VERSION_*），Publisher 逐字抄框架包 Identity。 -->
    <PackageDependency Name="Microsoft.WindowsAppRuntime.2" MinVersion="2.4.0.0" Publisher="CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US" />
  </Dependencies>

  <Resources>
    <Resource Language="en-us" />
    <Resource Language="zh-cn" />
  </Resources>

  <Applications>
    <Application Id="App" Executable="$ConsoleName" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements
        DisplayName="Blade²"
        Description="Blade² (rust fork) — WinUI 3 desktop distribution powered by the dsh kernel"
        BackgroundColor="transparent"
        Square150x150Logo="Assets\Square150x150Logo.png"
        Square44x44Logo="Assets\Square44x44Logo.png">
        <uap:DefaultTile Wide310x150Logo="Assets\Wide310x150Logo.png" />
      </uap:VisualElements>
    </Application>
  </Applications>

  <Capabilities>
    <!-- runFullTrust：壳直连内核 RPC 并管理其进程树。 -->
    <rescap:Capability Name="runFullTrust" />
    <!-- unvirtualizedResources：关文件虚拟化（见 Properties 里那条注释）。 -->
    <rescap:Capability Name="unvirtualizedResources" />
  </Capabilities>
</Package>
"@

$manifestPath = Join-Path $OutRoot 'AppxManifest.xml'
# 不带 BOM 写：makeappx 读 manifest 不吃 BOM，声明是 utf-8 就照 utf-8 解。
[IO.File]::WriteAllText($manifestPath, $manifest, (New-Object Text.UTF8Encoding($false)))
Write-Ok "写 $manifestPath"

# ---------- 4. exe 与资产 ----------
Write-Head 'Payload'

Copy-Item -LiteralPath $exe.FullName -Destination (Join-Path $OutRoot $ConsoleName) -Force
Write-Ok "拷入 $ConsoleName"

$assetFiles = @(
  'StoreLogo.png'
  'Square150x150Logo.png'
  'Square44x44Logo.png'
  'Wide310x150Logo.png'
  'Square44x44Logo.targetsize-16_altform-unplated.png'
  'Square44x44Logo.targetsize-24_altform-unplated.png'
  'Square44x44Logo.targetsize-32_altform-unplated.png'
  'Square44x44Logo.targetsize-48_altform-unplated.png'
  'Square44x44Logo.targetsize-96_altform-unplated.png'
  'Square44x44Logo.targetsize-256_altform-unplated.png'
  # 托盘图标：tray.rs 从 exe 目录向上六层找 Assets\app.ico，装机形态下第一层就命中。
  'app.ico'
)
$assetDst = Join-Path $OutRoot 'Assets'
New-Item -ItemType Directory -Path $assetDst -Force | Out-Null
foreach ($f in $assetFiles) {
  $src = Join-Path $AssetsSrc $f
  if (-not (Test-Path -LiteralPath $src)) { throw "资产缺失：$src" }
  Copy-Item -LiteralPath $src -Destination (Join-Path $assetDst $f) -Force
}
Write-Ok ("拷入 Assets\{0} 个文件" -f $assetFiles.Count)

# ---------- 5. Kernel ----------
Write-Head 'Kernel (约 505 MB，稍慢)'

$kernelDst = Join-Path $OutRoot 'Kernel'
$robocopyArgs = @(
  $KernelSrc, $kernelDst, '/E', '/NFL', '/NDL', '/NJH', '/NJS', '/NP',
  '/MT:8', '/R:1', '/W:1', '/XF', '.DS_Store', 'Thumbs.db'
)
$rc = & robocopy @robocopyArgs
if ($LASTEXITCODE -ge 8) { throw "robocopy Kernel 失败（exit $LASTEXITCODE）" }
Write-Ok ("robocopy 完成（exit {0}）→ {1}" -f $LASTEXITCODE, $kernelDst)

# ---------- 5b. 可选：随包带 WinAppSDK 运行期 ----------
if ($WithRuntime) {
  Write-Head 'WinAppSDK runtime (自包含腿)'
  $fwRoot = Get-ChildItem 'C:\Program Files\WindowsApps' -Directory -Filter 'Microsoft.WindowsAppRuntime.*_x64__8wekyb3d8bbwe' -ErrorAction SilentlyContinue |
            Sort-Object Name -Descending | Select-Object -First 1
  if (-not $fwRoot) { throw '机器上没有 Microsoft.WindowsAppRuntime 框架包，无法自包含' }
  Write-Host "  framework: $($fwRoot.Name)"
  # 只带 DLL/winmd：AppxBlockMap/AppxManifest/Signature 那三件是框架包自己的元数据，
  # 塞进壳的包里只会让 makeappx 困惑。
  $files = Get-ChildItem -LiteralPath $fwRoot.FullName -Recurse -File -Include *.dll, *.winmd, *.png -ErrorAction SilentlyContinue
  $copied = 0
  foreach ($f in $files) {
    $rel = $f.FullName.Substring($fwRoot.FullName.Length).TrimStart('\')
    $dst = Join-Path $OutRoot $rel
    New-Item -ItemType Directory -Path (Split-Path -Parent $dst) -Force | Out-Null
    Copy-Item -LiteralPath $f.FullName -Destination $dst -Force
    $copied++
  }
  Write-Ok ("拷入运行期 {0} 个文件" -f $copied)
}

# ---------- 6. pack ----------
Write-Head 'makeappx pack'

if (Test-Path -LiteralPath $MsixOut) { Remove-Item -LiteralPath $MsixOut -Force }
$null = & $MakeAppx pack /o /d $OutRoot /p $MsixOut
if ($LASTEXITCODE -ne 0) { throw "makeappx pack 失败（exit $LASTEXITCODE）" }
$msix = Get-Item -LiteralPath $MsixOut
Write-Ok ("{0} ({1:N2} MB)" -f $msix.FullName, ($msix.Length / 1MB))

# ---------- 7. sign ----------
if ($SkipSign) {
  Write-Warn2 'SkipSign：没签名，Add-AppxPackage 装不了，仅供 makeappx 输出自查'
} else {
  Write-Head 'signtool sign'
  $null = & $Signtool sign /fd SHA256 /sha1 $Thumbprint /tr 'http://timestamp.digicert.com' /td SHA256 $MsixOut 2>&1
  # 时间戳服务器不通时退回本地签名：装机只验证书链与发布者，不影响 Add-AppxPackage。
  if ($LASTEXITCODE -ne 0) {
    Write-Warn2 "带时间戳签名失败（exit $LASTEXITCODE），改本地签名"
    $null = & $Signtool sign /fd SHA256 /sha1 $Thumbprint $MsixOut
    if ($LASTEXITCODE -ne 0) { throw "signtool sign 失败（exit $LASTEXITCODE）" }
  }
  Write-Ok 'signed'
}

# ---------- 8. 自检 ----------
if (-not $SkipSign) {
  Write-Head 'verify'
  $null = & $Signtool verify /pa $MsixOut 2>&1
  if ($LASTEXITCODE -ne 0) { throw "signtool verify /pa 失败（exit $LASTEXITCODE）" }
  Write-Ok 'signtool verify /pa passed'
}

Write-Head 'Done'
Write-Host "  msix  : $MsixOut"
Write-Host "  stage : $OutRoot"
Write-Host "  装机  : pwsh -File (Join-Path `$PSScriptRoot 'install-msix.ps1')"
