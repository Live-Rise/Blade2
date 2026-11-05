# 栅格化「半径 24 超过半边 17」的退化圆角矩形路径（68x68 px 盒、r=48 px），
# 输出描边像素的极坐标分布，与实测暗带（半径 32-33.3、角度 65-115）比对。
$W = 68; $H = 68; $R = 48.0; $PI = [Math]::PI
$cx = 34; $cy = 34
$pts = New-Object System.Collections.Generic.List[double[]]
function AddArc($acx, $acy, $d0, $d1) {
  for ($i = 0; $i -le 240; $i++) {
    $th = ($d0 + ($d1 - $d0) * $i / 240) * $PI / 180.0
    $pts.Add([double[]]@($acx + $R * [Math]::Cos($th), $acy + $R * [Math]::Sin($th)))
  }
}
for ($i = 0; $i -le 120; $i++) { $pts.Add([double[]]@(20 + 28 * $i / 120, 0)) }   # 顶边
AddArc 48 48 270 180                                                             # 左上弧
for ($i = 0; $i -le 120; $i++) { $pts.Add([double[]]@(0, 48 - 28 * $i / 120)) }   # 左边
AddArc 48 20 180 90                                                              # 左下弧
for ($i = 0; $i -le 120; $i++) { $pts.Add([double[]]@(48 - 28 * $i / 120, 68)) }  # 底边
AddArc 20 20 90 0                                                                # 右下弧
for ($i = 0; $i -le 120; $i++) { $pts.Add([double[]]@(68, 20 + 28 * $i / 120)) }  # 右边
AddArc 20 48 360 270                                                             # 右上弧
$n = $pts.Count
$rows = New-Object System.Collections.Generic.List[string]
for ($y = 0; $y -lt $H; $y++) {
  $sb = ''
  for ($x = 0; $x -lt $W; $x++) {
    $best = 1e9
    for ($i = 0; $i -lt $n; $i++) {
      $dx = $x - $pts[$i][0]; $dy = $y - $pts[$i][1]
      $d2 = $dx * $dx + $dy * $dy
      if ($d2 -lt $best) { $best = $d2 }
    }
    $d = [Math]::Sqrt($best)
    if ($d -le 0.9) { $sb += '#' } elseif ($d -le 1.7) { $sb += '+' } else { $sb += '.' }
  }
  $rows.Add($sb)
}
$buckets = @{}
for ($y = 0; $y -lt $H; $y++) {
  for ($x = 0; $x -lt $W; $x++) {
    if ($rows[$y][$x] -ne '#') { continue }
    $rad = [Math]::Sqrt(($x - $cx) * ($x - $cx) + ($y - $cy) * ($y - $cy))
    $deg = [Math]::Atan2($y - $cy, $x - $cx) * 180.0 / $PI
    $bd = [int][Math]::Round($deg / 10) * 10
    $k = ('{0}' -f $bd)
    if (-not $buckets.ContainsKey($k)) { $buckets[$k] = New-Object System.Collections.Generic.List[double] }
    $buckets[$k].Add($rad)
  }
}
$out = New-Object System.Collections.Generic.List[string]
$out.Add('--- ASCII: # = 距退化路径 <=0.9px, + = <=1.7px ---')
foreach ($s in $rows) { $out.Add($s) }
$out.Add('--- 描边像素极坐标分布（按角度 10 度分桶）---')
foreach ($k in ($buckets.Keys | Sort-Object { [double]$_ })) {
  $v = $buckets[$k]
  $mn = ($v | Measure-Object -Minimum).Minimum
  $mx = ($v | Measure-Object -Maximum).Maximum
  $out.Add(('deg {0,5}: n={1,3} radius {2,5:F1} .. {3,5:F1}' -f $k, $v.Count, $mn, $mx))
}
[System.IO.File]::WriteAllLines('E:\Syncthing\DshWinUI\tmp-cap\degen.txt', $out)
Write-Host ('written pts=' + $n)
