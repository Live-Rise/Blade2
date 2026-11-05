param(
  [string]$Png = "E:\Syncthing\DshWinUI\tmp-cap\new-main.png",
  [int]$Cx = 2455, [int]$Cy = 1515, [int]$R = 40
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
function Hx($c) { return ("#{0:X2}{1:X2}{2:X2}" -f $c.R, $c.G, $c.B) }
Write-Host "=== 水平中线 y=$Cy，x=$($Cx-$R)..$($Cx+$R) ==="
for ($x = $Cx - $R; $x -le $Cx + $R; $x++) {
  $p = $bmp.GetPixel($x, $Cy)
  Write-Host ("x={0} dx={1,3} {2} lum={3,6:F1}" -f $x, ($x - $Cx), (Hx $p), (0.299*$p.R + 0.587*$p.G + 0.114*$p.B))
}
Write-Host "=== 顶部边缘 x=$Cx，y=$($Cy-$R)..$($Cy-$R+12) ==="
for ($y = $Cy - $R; $y -le $Cy - $R + 12; $y++) {
  $p = $bmp.GetPixel($Cx, $y)
  Write-Host ("y={0} dy={1,3} {2} lum={3,6:F1}" -f $y, ($y - $Cy), (Hx $p), (0.299*$p.R + 0.587*$p.G + 0.114*$p.B))
}
$bmp.Dispose()
