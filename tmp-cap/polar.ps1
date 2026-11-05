# 极坐标扫描：每个角度从中心向外，记录粉色结束的半径、最暗半径与最暗值
param(
  [string]$Png = "E:\Syncthing\DshWinUI\tmp-cap\blade2-main.png",
  [string]$Out = "E:\Syncthing\DshWinUI\tmp-cap\polar.txt",
  [int]$Cx = 2455, [int]$Cy = 1515
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
$base = $bmp.GetPixel($Cx, $Cy)
$lines = New-Object System.Collections.Generic.List[string]
$lines.Add(("base=#{0:X2}{1:X2}{2:X2}" -f $base.R, $base.G, $base.B))
for ($deg = 0; $deg -lt 360; $deg += 5) {
  $rad = $deg * [Math]::PI / 180.0
  $dx = [Math]::Cos($rad); $dy = [Math]::Sin($rad)
  $pinkEnd = -1.0; $minLum = 999.0; $minR = -1.0; $bgR = -1.0
  for ($r = 2.0; $r -le 40.0; $r += 0.5) {
    $x = [int][Math]::Round($Cx + $dx * $r)
    $y = [int][Math]::Round($Cy + $dy * $r)
    if ($x -lt 0 -or $y -lt 0 -or $x -ge $bmp.Width -or $y -ge $bmp.Height) { break }
    $p = $bmp.GetPixel($x, $y)
    $lum = 0.299 * $p.R + 0.587 * $p.G + 0.114 * $p.B
    $isPink = ([Math]::Abs($p.R - $base.R) -le 6 -and [Math]::Abs($p.G - $base.G) -le 6 -and [Math]::Abs($p.B - $base.B) -le 6)
    if ($isPink) { $pinkEnd = $r }
    if ($lum -lt $minLum) { $minLum = $lum; $minR = $r }
    if ($bgR -lt 0 -and $lum -gt 215) { $bgR = $r }
  }
  $lines.Add(("deg={0,3} pinkEnd={1,5:F1} minLum={2,6:F1} atR={3,5:F1} bgStart={4,5:F1}" -f $deg, $pinkEnd, $minLum, $minR, $bgR))
}
[System.IO.File]::WriteAllLines($Out, $lines)
$bmp.Dispose()
Write-Host "written $Out"
