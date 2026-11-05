# 校验 MSIX 内的内核文件补丁标记
param(
  [string]$Msix = 'E:\Syncthing\DshWinUI\AppxPkgs0800\Blade2_0.8.3.3_x64_Test\Blade2_0.8.3.3_x64.msix',
  [string]$Marker = 'portReady'
)
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [System.IO.Compression.ZipFile]::OpenRead($Msix)
try {
  $entry = $zip.Entries | Where-Object { $_.FullName -like '*dsh-web-app/lib/index.js' } | Select-Object -First 1
  $reader = New-Object System.IO.StreamReader($entry.Open())
  $text = $reader.ReadToEnd()
  $reader.Close()
  Write-Host "entry: $($entry.FullName)"
  Write-Host "marker '$Marker' in package: $($text.Contains($Marker))"
} finally {
  $zip.Dispose()
}
