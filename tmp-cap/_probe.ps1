param([string]$Path, [int]$Cols = 96, [int]$Rows = 40)
Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::FromFile($Path)
$w = $bmp.Width; $h = $bmp.Height
Write-Host ("FILE {0}  {1}x{2}" -f $Path, $w, $h)

# 亮度字符表（暗->亮）
$chars = '@#%*+=-:. '  # 10 级
$cellW = [Math]::Max(1, [Math]::Floor($w / $Cols))
$cellH = [Math]::Max(1, [Math]::Floor($h / $Rows))
$sb = New-Object System.Text.StringBuilder
for ($r = 0; $r -lt $Rows; $r++) {
  $line = New-Object System.Text.StringBuilder
  for ($c = 0; $c -lt $Cols; $c++) {
    $sum = 0.0; $n = 0
    $x0 = $c * $cellW; $y0 = $r * $cellH
    for ($y = $y0; $y -lt [Math]::Min($y0 + $cellH, $h); $y += [Math]::Max(1, [Math]::Floor($cellH / 4))) {
      for ($x = $x0; $x -lt [Math]::Min($x0 + $cellW, $w); $x += [Math]::Max(1, [Math]::Floor($cellW / 4))) {
        $px = $bmp.GetPixel($x, $y)
        $sum += ($px.R + $px.G + $px.B) / 3.0
        $n++
      }
    }
    $avg = if ($n -gt 0) { $sum / $n } else { 0 }
    $idx = [Math]::Min(9, [Math]::Floor($avg / 25.6))
    [void]$line.Append($chars[$idx])
  }
  [void]$sb.AppendLine($line.ToString())
}
$bmp.Dispose()
Write-Host $sb.ToString()

# 关键区域采样色
Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::FromFile($Path)
$probes = @(
  @{n='左上角(logo区)'; x=[int]($w*0.03); y=[int]($h*0.05)},
  @{n='左栏中部';      x=[int]($w*0.10); y=[int]($h*0.50)},
  @{n='左侧栏右边缘';  x=[int]($w*0.20); y=[int]($h*0.50)},
  @{n='内容区上部';    x=[int]($w*0.50); y=[int]($h*0.15)},
  @{n='内容区中部';    x=[int]($w*0.50); y=[int]($h*0.50)},
  @{n='内容区下部';    x=[int]($w*0.50); y=[int]($h*0.85)},
  @{n='右上角';        x=[int]($w*0.95); y=[int]($h*0.05)}
)
foreach ($p in $probes) {
  $px = $bmp.GetPixel($p.x, $p.y)
  Write-Host ("{0} ({1},{2}) = #{3:X2}{4:X2}{5:X2}" -f $p.n, $p.x, $p.y, $px.R, $px.G, $px.B)
}
$bmp.Dispose()
