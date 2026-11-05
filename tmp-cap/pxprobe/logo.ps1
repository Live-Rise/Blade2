Add-Type -AssemblyName System.Drawing
function RegionMax($bmp, $x0, $y0, $x1, $y1) {
  $max = 0.0
  for ($x = $x0; $x -lt $x1; $x += 2) {
    for ($y = $y0; $y -lt $y1; $y += 2) {
      $px = $bmp.GetPixel($x, $y)
      $v = ($px.R + $px.G + $px.B) / 3.0
      if ($v -gt $max) { $max = $v }
    }
  }
  return $max
}
function RegionMean($bmp, $x0, $y0, $x1, $y1) {
  $sum = 0.0; $n = 0
  for ($x = $x0; $x -lt $x1; $x += 3) {
    for ($y = $y0; $y -lt $y1; $y += 3) {
      $px = $bmp.GetPixel($x, $y)
      $sum += ($px.R + $px.G + $px.B) / 3.0; $n++
    }
  }
  return ($sum / [Math]::Max(1, $n))
}
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S1-settings.png')
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\S1-settings.png')
# 鸣潮 logo 区域（内容区左上，避开侧栏与标题文字：main 2560x1600, rust 2586x1671）
Write-Host ("logo region max:  main={0:F0}  rust={1:F0}" -f (RegionMax $m 620 200 900 330), (RegionMax $r 620 200 900 330))
Write-Host ("logo region mean: main={0:F0}  rust={1:F0}" -f (RegionMean $m 620 200 900 330), (RegionMean $r 620 200 900 330))
# 人物亮部（中央偏右）
Write-Host ("girl max:  main={0:F0}  rust={1:F0}" -f (RegionMax $m 1500 500 2000 900), (RegionMax $r 1500 500 2000 900))
Write-Host ("girl mean: main={0:F0}  rust={1:F0}" -f (RegionMean $m 1500 500 2000 900), (RegionMean $r 1500 500 2000 900))
# 暗部（左下角壁纸区）
Write-Host ("dark mean: main={0:F0}  rust={1:F0}" -f (RegionMean $m 560 1200 900 1500), (RegionMean $r 560 1250 900 1560))
$m.Dispose(); $r.Dispose()
