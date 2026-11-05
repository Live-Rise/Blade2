Add-Type -AssemblyName System.Drawing
function Mean($bmp) {
  $sum = 0.0; $n = 0
  for ($x = 440; $x -lt $bmp.Width - 10; $x += 24) {
    for ($y = 90; $y -lt $bmp.Height - 10; $y += 24) {
      $px = $bmp.GetPixel($x, $y)
      $sum += ($px.R + $px.G + $px.B) / 3.0; $n++
    }
  }
  return ($sum / [Math]::Max(1, $n))
}
foreach ($name in @('S0-default','S1-settings','P-usage','P-pet','P-memory','P-models')) {
  $m = [System.Drawing.Bitmap]::FromFile("E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\$name.png")
  $r = [System.Drawing.Bitmap]::FromFile("E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\$name.png")
  $mm = Mean $m; $rr = Mean $r
  Write-Host ("{0}  main({1}x{2}) mean={3:F1}   rust({4}x{5}) mean={6:F1}" -f $name, $m.Width, $m.Height, $mm, $r.Width, $r.Height, $rr)
  $m.Dispose(); $r.Dispose()
}
