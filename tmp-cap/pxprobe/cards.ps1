Add-Type -AssemblyName System.Drawing
function CardEdges($bmp, $label, $x0, $x1) {
  # 扫 x 带中点的水平亮线（卡片描边）：某行 y 上该带内多列都是 60-100 亮度
  $edges = @()
  for ($y = 200; $y -lt 1300; $y++) {
    $hits = 0
    for ($x = $x0; $x -lt $x1; $x += 4) {
      $px = $bmp.GetPixel($x, $y)
      $v = ($px.R + $px.G + $px.B) / 3.0
      if ($v -gt 55 -and $v -lt 110) { $hits++ }
    }
    $total = [Math]::Floor(($x1 - $x0) / 4)
    if ($hits -gt ($total * 0.7)) { $edges += $y }
  }
  # 合并相邻
  $out = @(); $prev = -10
  foreach ($e in $edges) { if ($e - $prev -gt 4) { $out += $e }; $prev = $e }
  Write-Host ("{0}: card edges y = {1}" -f $label, ($out -join ','))
}
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\P-models.png')
CardEdges $m 'main P-models' 900 2000
$m.Dispose()
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\P-models.png')
CardEdges $r 'rust P-models' 900 2000
$r.Dispose()
