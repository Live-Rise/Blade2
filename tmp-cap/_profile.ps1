param([string]$Path)
Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::FromFile($Path)
$w = $bmp.Width; $h = $bmp.Height
Write-Host ("FILE {0}  {1}x{2}" -f $Path, $w, $h)
# 内容区 x 32%..96%，逐行平均亮度（步进4px采样）
$x0 = [int]($w * 0.32); $x1 = [int]($w * 0.96)
# 用紧凑直方图字符：每8px行聚合一格，输出亮度0-9
$sb = New-Object System.Text.StringBuilder
$rowVals = @()
for ($y = 0; $y -lt $h; $y += 4) {
  $sum = 0.0; $n = 0
  for ($x = $x0; $x -lt $x1; $x += 8) {
    $px = $bmp.GetPixel($x, $y)
    $sum += ($px.R + $px.G + $px.B) / 3.0; $n++
  }
  $avg = $sum / [Math]::Max(1,$n)
  $rowVals += $avg
}
# 找最大最小
$min = ($rowVals | Measure-Object -Minimum).Minimum
$max = ($rowVals | Measure-Object -Maximum).Maximum
Write-Host ("row-profile min={0:N1} max={1:N1}  (y step 4px, value scaled 0-9)" -f $min, $max)
$i = 0
foreach ($v in $rowVals) {
  $lv = if ($max -gt $min) { [int](9 * ($v - $min) / ($max - $min)) } else { 0 }
  [void]$sb.Append($lv)
  $i++
  if ($i % 120 -eq 0) { [void]$sb.Append(" | y=" + ($i*4)) ; [void]$sb.AppendLine() }
}
Write-Host $sb.ToString()
$bmp.Dispose()
