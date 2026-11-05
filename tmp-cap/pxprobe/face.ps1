Add-Type -AssemblyName System.Drawing
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
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S1-settings.png')
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\S1-settings.png')
# 人脸亮部（无文字覆盖的壁纸区）
Write-Host ("face max: main={0:F0} rust={1:F0}" -f (RegionMax $m 1300 380 1450 500), (RegionMax $r 1440 390 1590 510))
# 头发银色高光
Write-Host ("hair max: main={0:F0} rust={1:F0}" -f (RegionMax $m 1150 250 1350 400), (RegionMax $r 1300 260 1500 410))
$m.Dispose(); $r.Dispose()
