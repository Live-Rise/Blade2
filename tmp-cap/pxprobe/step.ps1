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
    elseif (-not $bright -and $inRun) { $inRun = $false; $runs += "$start-$($x-1)" }
  }
  Write-Host ("{0}: {1}" -f $label, ($runs -join ' '))
}
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S1-settings.png')
# main 字号行：preview y 760 → device 973；按钮区 x 2100-2530
ColProfile $m 950 1000 2050 2560 95 'main 字号stepper'
$m.Dispose()
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\S1-settings.png')
# rust 字号行：preview y 700 → device 905
ColProfile $r 880 935 2050 2560 95 'rust 字号stepper'
$r.Dispose()
