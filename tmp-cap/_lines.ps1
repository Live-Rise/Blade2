param([string]$Path, [double]$X0f = 0.32, [double]$X1f = 0.96)
Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::FromFile($Path)
$w = $bmp.Width; $h = $bmp.Height
$x0 = [int]($w * $X0f); $x1 = [int]($w * $X1f)
Write-Host ("FILE {0}  {1}x{2}  content x {3}..{4}" -f $Path, $w, $h, $x0, $x1)

# 估背景：采样内容区若干行的中位亮度
$bgs = @()
for ($y = 40; $y -lt $h - 40; $y += 37) {
  $rowVals = @()
  for ($x = $x0; $x -lt $x1; $x += 16) { $px = $bmp.GetPixel($x, $y); $rowVals += ($px.R+$px.G+$px.B)/3.0 }
  $sorted = $rowVals | Sort-Object
  $bgs += $sorted[[int]($sorted.Count/2)]
}
$bg = ($bgs | Sort-Object)[[int]($bgs.Count/2)]
Write-Host ("bg~{0:N1}" -f $bg)

# 逐 2px 行统计亮像素占比与峰值亮度
$thresh = $bg + 30
$rows = @()
for ($y = 20; $y -lt $h - 20; $y += 2) {
  $cnt = 0; $n = 0; $maxL = 0.0
  for ($x = $x0; $x -lt $x1; $x += 6) {
    $px = $bmp.GetPixel($x, $y); $L = ($px.R+$px.G+$px.B)/3.0
    if ($L -gt $thresh) { $cnt++ }
    if ($L -gt $maxL) { $maxL = $L }
    $n++
  }
  $rows += [pscustomobject]@{y=$y; p=$cnt/[Math]::Max(1,$n); peak=$maxL}
}
# 合并连续有内容行（p>0.004）
$bands = @(); $cur = $null
foreach ($r in $rows) {
  if ($r.p -gt 0.004) {
    if ($null -eq $cur) { $cur = @{y0=$r.y; y1=$r.y; peak=$r.peak; pmax=$r.p} }
    else { $cur.y1 = $r.y; if ($r.peak -gt $cur.peak) {$cur.peak=$r.peak}; if ($r.p -gt $cur.pmax) {$cur.pmax=$r.p} }
  } else {
    if ($null -ne $cur) { if ($cur.y1 - $cur.y0 -ge 6) { $bands += $cur }; $cur = $null }
  }
}
if ($null -ne $cur) { $bands += $cur }
Write-Host ("content bands: {0}" -f $bands.Count)
foreach ($b in $bands) {
  $mid = [int](($b.y0+$b.y1)/2)
  # 行中心最亮点在哪个 x（定位控件/文字位置）
  $bestX = 0; $bestV = 0
  for ($x = $x0; $x -lt $x1; $x += 4) {
    $px = $bmp.GetPixel($x, $mid); $L = ($px.R+$px.G+$px.B)/3.0
    if ($L -gt $bestV) { $bestV = $L; $bestX = $x }
  }
  Write-Host ("  y {0,4}-{1,4} h={2,3} peak={3,5:N0} @{4,5} ({5:P0})" -f $b.y0, $b.y1, ($b.y1-$b.y0), $b.peak, $bestX, ($bestX/[double]$w))
}
$bmp.Dispose()
