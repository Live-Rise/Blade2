# 环带扫描：对每个 dy，找该行圆内的最暗像素及其 |dx| 与半径；同时给出行内亮度剖面摘要
param(
  [string]$Png = "E:\Syncthing\DshWinUI\tmp-cap\blade2-main.png",
  [string]$Out = "E:\Syncthing\DshWinUI\tmp-cap\ring.txt",
  [int]$Cx = 2455, [int]$Cy = 1515, [int]$R = 34
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
$lines = New-Object System.Collections.Generic.List[string]
$base = $bmp.GetPixel($Cx, $Cy)
$bl = 0.299 * $base.R + 0.587 * $base.G + 0.114 * $base.B
$lines.Add(("base=#{0:X2}{1:X2}{2:X2} lum={3:F1}  center=({4},{5})" -f $base.R, $base.G, $base.B, $bl, $Cx, $Cy))
for ($dy = -34; $dy -le 34; $dy++) {
  $y = $Cy + $dy
  $half = [Math]::Sqrt([Math]::Max(0, $R * $R - $dy * $dy))
  if ($half -lt 1) { continue }
  $minLum = 999.0; $minX = -1
  $prof = New-Object System.Collections.Generic.List[string]
  for ($x = $Cx - [int]$half; $x -le $Cx + [int]$half; $x++) {
    $p = $bmp.GetPixel($x, $y)
    $lum = 0.299 * $p.R + 0.587 * $p.G + 0.114 * $p.B
    if ($lum -lt $minLum) { $minLum = $lum; $minX = $x }
  }
  $mdx = [Math]::Abs($minX - $Cx)
  $rad = [Math]::Sqrt($mdx * $mdx + $dy * $dy)
  # 每隔 4 px 采一个剖面点（从中心往外）
  $samples = @()
  for ($d = 0; $d -le [int]$half; $d += 4) {
    $p = $bmp.GetPixel($Cx + $d, $y)
    $lum = [int](0.299 * $p.R + 0.587 * $p.G + 0.114 * $p.B)
    $samples += ("{0}:{1}" -f $d, $lum)
  }
  $lines.Add(("dy={0,3} half={1,5:F1} minLum={2,6:F1} at|dx|={3,3} rad={4,5:F1}   {5}" -f $dy, $half, $minLum, $mdx, $rad, ($samples -join ' ')))
}
[System.IO.File]::WriteAllLines($Out, $lines)
$bmp.Dispose()
Write-Host "written $Out"
