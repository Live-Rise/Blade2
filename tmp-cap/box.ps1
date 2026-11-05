param(
  [string]$Png = "E:\Syncthing\DshWinUI\tmp-cap\new-main.png",
  [int]$X0 = 2250, [int]$X1 = 2450, [int]$Y0 = 1400, [int]$Y1 = 1580
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
$best = $null; $bestN = 0
for ($y = $Y0; $y -le $Y1; $y++) {
  $n = 0
  for ($x = $X0; $x -le $X1; $x++) {
    $p = $bmp.GetPixel($x, $y)
    # 强调粉：R 最大、B 最小、R-B 差大
    if ($p.R -gt 150 -and ($p.R - $p.B) -gt 30 -and $p.R -ge $p.G -and $p.G -ge $p.B) { $n++ }
  }
  if ($n -gt $bestN) { $bestN = $n; $best = $y }
}
Write-Host "widest row y=$best n=$bestN"
# 该行粉色区间
$xmin = -1; $xmax = -1
for ($x = $X0; $x -le $X1; $x++) {
  $p = $bmp.GetPixel($x, $best)
  if ($p.R -gt 150 -and ($p.R - $p.B) -gt 30 -and $p.R -ge $p.G -and $p.G -ge $p.B) { if ($xmin -lt 0) { $xmin = $x }; $xmax = $x }
}
Write-Host "row span: $xmin..$xmax  width=$($xmax-$xmin+1)"
$cx = [int](($xmin + $xmax) / 2); $cy = $best
# 垂直区间
$ymin = -1; $ymax = -1
for ($y = $Y0; $y -le $Y1; $y++) {
  $p = $bmp.GetPixel($cx, $y)
  if ($p.R -gt 150 -and ($p.R - $p.B) -gt 30 -and $p.R -ge $p.G -and $p.G -ge $p.B) { if ($ymin -lt 0) { $ymin = $y }; $ymax = $y }
}
Write-Host "col span: $ymin..$ymax height=$($ymax-$ymin+1)"
Write-Host "CENTER=($cx,$cy)"
# 圆心像素颜色
$c0 = $bmp.GetPixel($cx, $cy)
Write-Host ("center pixel: #{0:X2}{1:X2}{2:X2}" -f $c0.R, $c0.G, $c0.B)
$bmp.Dispose()
