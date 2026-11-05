# 在新截图里定位强调粉圆形按钮（发送键）：按颜色聚类给包围盒
param(
  [string]$Png = "E:\Syncthing\DshWinUI\tmp-cap\new-main.png",
  [int]$Tol = 10
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
Write-Host "image: $($bmp.Width)x$($bmp.Height)"
# 采样一行找一个「粉」像素作为种子（R 明显 > G > B 且中等亮度）
$seed = $null
for ($y = 0; $y -lt $bmp.Height -and -not $seed; $y += 3) {
  for ($x = 0; $x -lt $bmp.Width; $x += 3) {
    $p = $bmp.GetPixel($x, $y)
    if ($p.R -gt 140 -and $p.R -lt 235 -and $p.G -gt 100 -and $p.G -lt 210 -and $p.B -gt 100 -and $p.B -lt 210 -and
        ($p.R - $p.G) -ge 25 -and ($p.G - $p.B) -ge 8) { $seed = $p; Write-Host "seed at ($x,$y) = #$([Convert]::ToString($p.R,16).PadLeft(2,'0'))$([Convert]::ToString($p.G,16).PadLeft(2,'0'))$([Convert]::ToString($p.B,16).PadLeft(2,'0'))"; break }
  }
}
if (-not $seed) { Write-Host 'no seed'; $bmp.Dispose(); exit }
# 洪水填充式聚类：四邻域扩散
$minX = 1e9; $minY = 1e9; $maxX = -1; $maxY = -1; $count = 0
$stack = New-Object System.Collections.Stack
$seen = New-Object 'bool[,]' $bmp.Width, $bmp.Height
$stack.Push(@($seed.GetHashCode())) | Out-Null
# 重新从种子坐标开始
$sy = -1; $sx = -1
for ($y = 0; $y -lt $bmp.Height -and $sy -lt 0; $y += 3) {
  for ($x = 0; $x -lt $bmp.Width; $x += 3) {
    $p = $bmp.GetPixel($x, $y)
    if ($p.R -eq $seed.R -and $p.G -eq $seed.G -and $p.B -eq $seed.B) { $sx = $x; $sy = $y; break }
  }
}
$stack = New-Object System.Collections.Stack
$stack.Push(@($sx, $sy))
while ($stack.Count -gt 0) {
  $pt = $stack.Pop()
  $x = [int]$pt[0]; $y = [int]$pt[1]
  if ($x -lt 0 -or $y -lt 0 -or $x -ge $bmp.Width -or $y -ge $bmp.Height) { continue }
  if ($seen[$x, $y]) { continue }
  $p = $bmp.GetPixel($x, $y)
  if ([Math]::Abs($p.R - $seed.R) -gt $Tol -or [Math]::Abs($p.G - $seed.G) -gt $Tol -or [Math]::Abs($p.B - $seed.B) -gt $Tol) { continue }
  $seen[$x, $y] = $true; $count++
  if ($x -lt $minX) { $minX = $x }; if ($x -gt $maxX) { $maxX = $x }
  if ($y -lt $minY) { $minY = $y }; if ($y -gt $maxY) { $maxY = $y }
  $stack.Push(@($x + 1, $y)); $stack.Push(@($x - 1, $y)); $stack.Push(@($x, $y + 1)); $stack.Push(@($x, $y - 1))
}
Write-Host ("cluster: n={0} box=({1},{1})..({2},{3}) size={4}x{5} center=({6},{7})" -f $count, $minX, $maxX, $maxY, ($maxX - $minX + 1), ($maxY - $minY + 1), (($minX + $maxX) / 2), (($minY + $maxY) / 2))
$bmp.Dispose()
