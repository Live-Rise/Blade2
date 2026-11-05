param([string]$Png, [int]$X0, [int]$X1, [int]$Y0, [int]$Y1, [int]$Min = 90)
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
$inBand = $false; $start = 0
for ($y = $Y0; $y -le $Y1; $y++) {
  $cnt = 0
  for ($x = $X0; $x -le $X1; $x += 2) {
    $p = $bmp.GetPixel($x, $y)
    if ((($p.R + $p.G + $p.B) / 3) -gt $Min) { $cnt++ }
  }
  $has = $cnt -gt 3
  if ($has -and -not $inBand) { $inBand = $true; $start = $y }
  elseif (-not $has -and $inBand) {
    $inBand = $false
    if ($y - $start -gt 2) { Write-Host ("band {0}-{1}  h={2}" -f $start, ($y - 1), ($y - $start)) }
  }
}
if ($inBand) { Write-Host ("band {0}-{1}+ (open)" -f $start, $Y1) }
$bmp.Dispose()
