Add-Type -AssemblyName System.Drawing
function BandExtent($bmp, $y0, $y1, $label) {
  # 找 y 带内最亮行的高亮带 x 范围：band 色 ≈ #2A2A2A（subtle over #1C1C1C），比底亮但暗于文字
  $best = $null
  for ($y = $y0; $y -lt $y1; $y++) {
    $xs = @()
    for ($x = 4; $x -lt 800; $x++) {
      $px = $bmp.GetPixel($x, $y)
      $v = ($px.R + $px.G + $px.B) / 3.0
      if ($v -ge 32 -and $v -le 58) { $xs += $x }
    }
    if ($xs.Count -gt ($best.Count ?? 0)) { $best = $xs }
  }
  if ($best.Count -gt 10) { Write-Host ("{0}: band x {1}-{2} (logical {3:F0}-{4:F0})" -f $label, $best[0], $best[-1], ($best[0]/2), ($best[-1]/2)) }
  else { Write-Host ("{0}: no clean band (n={1})" -f $label, $best.Count) }
}
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S1-settings.png')
BandExtent $m 148 200 'main S1 通用设置选中带'
$m.Dispose()
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\S1-settings.png')
BandExtent $r 148 200 'rust S1 通用设置选中带'
$r.Dispose()
