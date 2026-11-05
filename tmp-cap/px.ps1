# 直接打印关键像素的 RGB
param(
  [string]$Png = "E:\Syncthing\DshWinUI\tmp-cap\blade2-main.png",
  [int]$Cx = 2455
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
function Hex($c) { return ("#{0:X2}{1:X2}{2:X2}" -f $c.R, $c.G, $c.B) }
Write-Host "--- vertical line x=$Cx, y=1470..1560 ---"
for ($y = 1470; $y -le 1560; $y += 2) {
  $p = $bmp.GetPixel($Cx, $y)
  Write-Host ("y={0} {1}" -f $y, (Hex $p))
}
Write-Host "--- horizontal line y=1545, x=2425..2485 ---"
for ($x = 2425; $x -le 2485; $x++) {
  $p = $bmp.GetPixel($x, 1545)
  Write-Host ("x={0} {1}" -f $x, (Hex $p))
}
Write-Host "--- horizontal line y=1547, x=2435..2475 ---"
for ($x = 2435; $x -le 2475; $x++) {
  $p = $bmp.GetPixel($x, 1547)
  Write-Host ("x={0} {1}" -f $x, (Hex $p))
}
$bmp.Dispose()
