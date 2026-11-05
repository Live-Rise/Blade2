Add-Type -AssemblyName System.Drawing
function FindToggle($bmp, $y0, $y1, $label) {
  # toggle 开关轨道：亮色圆角矩形（浅粉 #F5C9C9?），扫 y 带内最亮像素的 x 分布
  $xs = @()
  for ($x = 800; $x -lt 2540; $x++) {
    for ($y = $y0; $y -lt $y1; $y++) {
      $px = $bmp.GetPixel($x, $y)
      if ($px.R -gt 180 -and $px.G -gt 130 -and $px.B -gt 130 -and ($px.R - $px.B) -gt -30) { $xs += $x; break }
    }
  }
  if ($xs.Count -gt 0) { Write-Host ("{0}: bright toggle track x {1}-{2} (logical {3:F0}-{4:F0})" -f $label, $xs[0], $xs[-1], ($xs[0]/2), ($xs[-1]/2)) }
  else { Write-Host ("{0}: not found" -f $label) }
}
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\P-memory.png')
FindToggle $m 600 680 'main P-memory toggle(持久记忆行)'
$m.Dispose()
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\P-memory.png')
FindToggle $r 620 720 'rust P-memory toggle(持久记忆行)'
$r.Dispose()
