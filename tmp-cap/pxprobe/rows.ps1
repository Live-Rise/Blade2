Add-Type -AssemblyName System.Drawing
function RowBands($bmp, $label) {
  Write-Host "== $label"
  $in = $false; $s = 0
  for ($y = 120; $y -lt 1200; $y++) {
    $hits = 0
    for ($x = 40; $x -lt 540; $x += 2) {
      $px = $bmp.GetPixel($x, $y)
      if ((($px.R + $px.G + $px.B) / 3.0) -gt 90) { $hits++ }
    }
    $bright = $hits -ge 3
    if ($bright -and -not $in) { $in = $true; $s = $y }
    elseif (-not $bright -and $in) {
      $in = $false
      if ($y - $s -gt 8) { Write-Host ("  band y {0}-{1} (h={2})" -f $s, ($y-1), ($y-$s)) }
    }
  }
}
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S0-default.png')
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\S0-default.png')
RowBands $m 'main'
RowBands $r 'rust'
$m.Dispose(); $r.Dispose()
