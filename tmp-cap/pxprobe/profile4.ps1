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
    elseif (-not $bright -and $inRun) { $inRun = $false; if ($runs.Count -lt 8) { $runs += "$start-$($x-1)" } }
  }
  Write-Host ("{0}: {1}" -f $label, ($runs -join ' '))
}
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S0-default.png')
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\S0-default.png')
# main: 新会话行 y≈210-250（预览 165*1.28≈211）；工作区段头 y≈335-370；DshWinUI 行 y≈450-490? 
ColProfile $m 205 255 30 400 90 'main 新会话'
ColProfile $m 330 375 30 400 90 'main 工作区段头'
ColProfile $m 448 495 30 400 90 'main DshWinUI行'
# rust: 新会话 y≈205-260（预览 112*1.293≈145? 按整图重估 208-255）；工作区段头 y≈345-390; DshWinUI y≈455-505
ColProfile $r 208 258 30 400 90 'rust 新会话'
ColProfile $r 345 392 30 400 90 'rust 工作区段头'
ColProfile $r 455 505 30 400 90 'rust DshWinUI行'
$m.Dispose(); $r.Dispose()
