Add-Type -AssemblyName System.Drawing
function ColProfile($bmp, $y0, $y1, $x0, $x1, $thresh, $label) {
  $runs = @()
  $inRun = $false; $start = 0
  for ($x = $x0; $x -lt $x1; $x++) {
    $hits = 0
    for ($y = $y0; $y -lt $y1; $y++) {
      $px = $bmp.GetPixel($x, $y)
      if ((($px.R + $px.G + $px.B) / 3.0) -gt $thresh) { $hits++ }
    }
    $bright = $hits -ge 1
    if ($bright -and -not $inRun) { $inRun = $true; $start = $x }
    elseif (-not $bright -and $inRun) { $inRun = $false; if ($runs.Count -lt 14) { $runs += "$start-$($x-1)" } }
  }
  Write-Host ("{0}: {1}" -f $label, ($runs -join ' '))
}
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\P-usage.png')
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\P-usage.png')
# 网格首行 y：main 热力格约 preview 590-600 → device 755-770；rust preview 505 → device 653
ColProfile $m 752 772 600 2500 60 'main 网格第一行'
ColProfile $r 648 668 600 2560 60 'rust 网格第一行'
# 月标行：main preview 788 → device 1009；rust preview 713 → device 922
ColProfile $m 1000 1030 600 2500 80 'main 月标行'
ColProfile $r 912 940 600 2560 80 'rust 月标行'
$m.Dispose(); $r.Dispose()
