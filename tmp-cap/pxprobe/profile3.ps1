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
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\S0-default.png')
foreach ($band in @(@(470,512,'未分组?'), @(545,580,'导出row'), @(955,995,'展开181?'))) {
  ColProfile $r $band[0] $band[1] 30 400 90 "rust $($band[2])(y$($band[0]))"
}
# 主干组行带 icon 的墨迹（阈值放低抓图标描边）
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S0-default.png')
foreach ($band in @(@(528,566,'未分组'), @(835,870,'展开181'))) {
  ColProfile $m $band[0] $band[1] 30 400 60 "main $($band[2]) thr60(y$($band[0]))"
}
$r.Dispose(); $m.Dispose()
