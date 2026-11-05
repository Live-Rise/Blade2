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
    elseif (-not $bright -and $inRun) { $inRun = $false; $runs += "$start-$($x-1)" }
  }
  Write-Host ("{0}: {1}" -f $label, ($runs -join ' '))
}
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S0-default.png')
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\S0-default.png')
# 先定位行：打印每行 y 上「未分组/会话行」的亮像素行分布，找文字带
# main S0: 未分组行约 y 530-565；会话行1 y 620-655（从整图观察）
ColProfile $m 528 566 40 400 100 'main 未分组(y528-566)'
ColProfile $m 622 656 40 400 100 'main 会话1(y622-656)'
ColProfile $m 692 726 40 400 100 'main 会话2(y692-726)'
ColProfile $m 835 870 40 400 100 'main 展开181(y835-870)'
# rust S0: 侧栏行距不同，未分组 y≈556-590，会话1 y≈648-682，展开 y≈1030-1065
ColProfile $r 552 592 40 400 100 'rust 未分组(y552-592)'
ColProfile $r 646 684 40 400 100 'rust 会话1(y646-684)'
ColProfile $r 716 754 40 400 100 'rust 会话2(y716-754)'
ColProfile $r 1030 1068 40 400 100 'rust 展开181(y1030-1068)'
$m.Dispose(); $r.Dispose()
