Add-Type -AssemblyName System.Drawing
function RowBands($bmp, $label, $x0, $x1) {
  $out = @(); $in = $false; $s = 0
  for ($y = 200; $y -lt 1300; $y++) {
    $hits = 0
    for ($x = $x0; $x -lt $x1; $x += 1) {
      $px = $bmp.GetPixel($x, $y)
      $v = ($px.R + $px.G + $px.B) / 3.0
      if ($v -gt 120) { $hits++ }
    }
    $bright = $hits -ge 4
    if ($bright -and -not $in) { $in = $true; $s = $y }
    elseif (-not $bright -and $in) {
      $in = $false
      if ($y - $s -gt 8) { $out += "$s-$($y-1)" }
    }
  }
  Write-Host ("{0}: {1}" -f $label, ($out -join ' '))
}
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\P-models.png')
RowBands $m 'main 编辑/删除钮列' 2340 2500
$m.Dispose()
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\P-models.png')
RowBands $r 'rust 编辑/删除钮列' 2340 2530
$r.Dispose()
