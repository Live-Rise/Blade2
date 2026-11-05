param([string]$Path, [int]$Ya, [int]$Yb, [double]$X0f = 0.32, [double]$X1f = 0.96, [int]$Step = 2)
Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::FromFile($Path)
$w = $bmp.Width; $h = $bmp.Height
$x0 = [int]($w * $X0f); $x1 = [int]($w * $X1f)
$bgs = @()
for ($y = 40; $y -lt $h - 40; $y += 37) {
  $rowVals = @()
  for ($x = $x0; $x -lt $x1; $x += 16) { $px = $bmp.GetPixel($x, $y); $rowVals += ($px.R+$px.G+$px.B)/3.0 }
  $sorted = $rowVals | Sort-Object
  $bgs += $sorted[[int]($sorted.Count/2)]
}
$bg = ($bgs | Sort-Object)[[int]($bgs.Count/2)]
$thresh = $bg + 30
Write-Host ("FILE {0} bg~{1:N1} thresh~{2:N1} y {3}..{4}" -f $Path, $bg, $thresh, $Ya, $Yb)
$sb = New-Object System.Text.StringBuilder
for ($y = $Ya; $y -lt $Yb; $y += $Step) {
  $cnt = 0; $n = 0
  for ($x = $x0; $x -lt $x1; $x += 6) {
    $px = $bmp.GetPixel($x, $y); $L = ($px.R+$px.G+$px.B)/3.0
    if ($L -gt $thresh) { $cnt++ }
    $n++
  }
  $p = $cnt/[Math]::Max(1,$n)
  # 每行 1 字符：空=无内容, 1-9/H=占比分级（对数感）
  $ch = ' '
  if ($p -gt 0.001) {
    $lv = [Math]::Ceiling([Math]::Log10($p*1000+1) * 3)
    if ($lv -gt 9) { $lv = 9 }
    $ch = [string]$lv
    if ($lv -eq 9) { $ch = 'H' }
  }
  [void]$sb.Append($ch)
  if ((($y - $Ya) / $Step + 1) % 100 -eq 0) { [void]$sb.Append(" |y=" + ($y + $Step)); [void]$sb.AppendLine() }
}
Write-Host $sb.ToString()
$bmp.Dispose()
