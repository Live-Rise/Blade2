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
    $bright = $hits -ge 2
    if ($bright -and -not $inRun) { $inRun = $true; $start = $x }
    elseif (-not $bright -and $inRun) { $inRun = $false; if ($runs.Count -lt 6) { $runs += "$start-$($x-1)" } }
  }
  Write-Host ("{0}: {1}" -f $label, ($runs -join ' '))
}
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S0-default.png')
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\S0-default.png')
ColProfile $m 134 160 30 560 90 'main 新会话'
ColProfile $m 212 237 30 560 90 'main 工作区hdr'
ColProfile $m 291 318 30 560 90 'main DshWinUI'
ColProfile $m 449 480 30 560 90 'main 未分组'
ColProfile $m 534 560 30 560 90 'main 会话1导出'
ColProfile $m 934 960 30 560 90 'main 展开181'
Write-Host ''
ColProfile $r 134 160 30 560 90 'rust 新会话'
ColProfile $r 224 249 30 560 90 'rust 工作区hdr'
ColProfile $r 299 326 30 560 90 'rust DshWinUI'
ColProfile $r 457 488 30 560 90 'rust 未分组'
ColProfile $r 542 568 30 560 90 'rust 会话1导出'
ColProfile $r 942 968 30 560 90 'rust 展开181'
$m.Dispose(); $r.Dispose()
