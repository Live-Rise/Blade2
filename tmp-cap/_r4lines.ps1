param([string]$Png, [int]$X0, [int]$X1, [int]$Y0, [int]$Y1)
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
# 扫描横向亮线：一行内多数像素比上下邻居亮一档
for ($y = $Y0; $y -le $Y1; $y++) {
  $cnt = 0; $tot = 0
  for ($x = $X0; $x -le $X1; $x += 4) {
    $tot++
    $p = $bmp.GetPixel($x, $y)
    $u = $bmp.GetPixel($x, $y - 3)
    $d = $bmp.GetPixel($x, $y + 3)
    $lum = ($p.R + $p.G + $p.B) / 3
    $lumU = ($u.R + $u.G + $u.B) / 3
    $lumD = ($d.R + $d.G + $d.B) / 3
    if ($lum -gt 40 -and $lum -gt $lumU + 6 -and $lum -gt $lumD + 6) { $cnt++ }
  }
  if ($cnt / $tot -gt 0.5) { Write-Host "line y=$y  ratio=$([math]::Round($cnt/$tot,2))" }
}
$bmp.Dispose()
