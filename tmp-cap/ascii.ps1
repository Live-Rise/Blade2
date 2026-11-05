# ASCII 亮度图：发送键底部区域，看暗带的精确形状
param(
  [string]$Png = "E:\Syncthing\DshWinUI\tmp-cap\blade2-main.png",
  [int]$X0 = 2430, [int]$X1 = 2480, [int]$Y0 = 1536, [int]$Y1 = 1556
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
$ramp = ' .:-=+*#%@'
for ($y = $Y0; $y -le $Y1; $y++) {
  $sb = New-Object System.Text.StringBuilder
  $null = $sb.Append("{0,4} " -f $y)
  for ($x = $X0; $x -le $X1; $x++) {
    $p = $bmp.GetPixel($x, $y)
    $lum = [int]((0.299 * $p.R + 0.587 * $p.G + 0.114 * $p.B) / 25.6)
    if ($lum -gt 9) { $lum = 9 }
    if ($lum -lt 0) { $lum = 0 }
    $null = $sb.Append($ramp[$lum])
  }
  Write-Host $sb.ToString()
}
Write-Host ("x from {0} to {1}" -f $X0, $X1)
$bmp.Dispose()
