Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::FromFile('C:\Users\Admin\AppData\Local\Blade2\cache\shell-skin.poster.bmp')
Write-Host ("poster: {0}x{1}" -f $bmp.Width, $bmp.Height)
function RegionMax($bmp, $x0, $y0, $x1, $y1) {
  $max = 0.0
  for ($x = $x0; $x -lt $x1; $x += 1) {
    for ($y = $y0; $y -lt $y1; $y += 1) {
      $px = $bmp.GetPixel($x, $y)
      $v = ($px.R + $px.G + $px.B) / 3.0
      if ($v -gt $max) { $max = $v }
    }
  }
  return $max
}
Write-Host ("poster hair max: {0:F0}" -f (RegionMax $bmp 1300 260 1500 410))
Write-Host ("poster face max: {0:F0}" -f (RegionMax $bmp 1440 390 1590 510))
$bmp.Dispose()
