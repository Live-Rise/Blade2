Add-Type -AssemblyName System.Drawing
# 在 S0 截图上找「未分组」与会话标题的左缘（扫描文字起始列）
function LeftEdge($bmp, $y0, $y1, $x0, $x1, $thresh) {
  # 逐列统计亮度 > thresh 的像素数，返回第一处连续亮列
  for ($x = $x0; $x -lt $x1; $x++) {
    $hits = 0
    for ($y = $y0; $y -lt $y1; $y++) {
      $px = $bmp.GetPixel($x, $y)
      if ((($px.R + $px.G + $px.B) / 3.0) -gt $thresh) { $hits++ }
    }
    if ($hits -ge 2) { return $x }
  }
  return -1
}
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S0-default.png')
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\S0-default.png')
# main 2560x1600: 未分组 行 y≈530-560 (2x), 会话行1 y≈620-650
Write-Host ("main 未分组 icon/text: " + (LeftEdge $m 530 560 60 400 90))
Write-Host ("main 会话1标题:       " + (LeftEdge $m 620 650 60 400 90))
Write-Host ("main 会话2标题:       " + (LeftEdge $m 690 720 60 400 90))
# rust 2586x1671: 未分组 y≈555-585, 会话 y≈640-670
Write-Host ("rust 未分组 icon/text: " + (LeftEdge $r 550 590 60 400 90))
Write-Host ("rust 会话1标题:       " + (LeftEdge $r 640 675 60 400 90))
Write-Host ("rust 会话2标题:       " + (LeftEdge $r 710 745 60 400 90))
$m.Dispose(); $r.Dispose()
