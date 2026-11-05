param([string]$Png, [int]$X, [int]$Y0, [int]$Y1)
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
$prev = -1
for ($y = $Y0; $y -le $Y1; $y++) {
  $p = $bmp.GetPixel($X, $y)
  $lum = [int](($p.R + $p.G + $p.B) / 3)
  if ($prev -ge 0 -and [math]::Abs($lum - $prev) -gt 4) {
    Write-Host ("y={0} lum {1}->{2}  RGB=({3},{4},{5})" -f $y, $prev, $lum, $p.R, $p.G, $p.B)
  }
  $prev = $lum
}
$bmp.Dispose()
