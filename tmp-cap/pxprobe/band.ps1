Add-Type -AssemblyName System.Drawing
function BandExtent($bmp, $y, $label) {
  # 找该行高亮带的 x 范围（比底色亮一点的连续区）
  $xs = @()
  for ($x = 10; $x -lt 840; $x++) {
    $px = $bmp.GetPixel($x, $y)
    $v = ($px.R + $px.G + $px.B) / 3.0
    if ($v -gt 38 -and $v -lt 75) { $xs += $x }
  }
  if ($xs.Count -gt 0) { Write-Host ("{0}: band x {1}-{2}  (logical {3:F1}-{4:F1})" -f $label, $xs[0], $xs[-1], ($xs[0]/2), ($xs[-1]/2)) }
  else { Write-Host ("{0}: no band" -f $label) }
}
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S0-default.png')
BandExtent $m 626 'main 选中行带(y626)'
$m.Dispose()
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\S0-default.png')
BandExtent $r 634 'rust 选中行带(y634)'
$r.Dispose()
