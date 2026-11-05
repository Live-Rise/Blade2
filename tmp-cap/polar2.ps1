# 对任意控件中心做极坐标扫描：比较「+ 按钮」与「发送键」
param(
  [string]$Png = "E:\Syncthing\DshWinUI\tmp-cap\blade2-main.png",
  [string]$Out = "E:\Syncthing\DshWinUI\tmp-cap\polar2.txt",
  [int]$Ax = 621, [int]$Ay = 1515,
  [int]$Sx = 2455, [int]$Sy = 1515
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
$lines = New-Object System.Collections.Generic.List[string]

function Scan($cx, $cy, $label) {
  $base = $bmp.GetPixel($cx, $cy)
  $lines.Add(("=== {0} center=({1},{2}) base=#{3:X2}{4:X2}{5:X2} ===" -f $label, $cx, $cy, $base.R, $base.G, $base.B))
  for ($deg = 0; $deg -lt 360; $deg += 10) {
    $rad = $deg * [Math]::PI / 180.0
    $dx = [Math]::Cos($rad); $dy = [Math]::Sin($rad)
    $pinkEnd = -1.0; $minLum = 999.0; $minR = -1.0; $bgR = -1.0
    for ($r = 2.0; $r -le 42.0; $r += 0.5) {
      $x = [int][Math]::Round($cx + $dx * $r)
      $y = [int][Math]::Round($cy + $dy * $r)
      if ($x -lt 0 -or $y -lt 0 -or $x -ge $bmp.Width -or $y -ge $bmp.Height) { break }
      $p = $bmp.GetPixel($x, $y)
      $lum = 0.299 * $p.R + 0.587 * $p.G + 0.114 * $p.B
      $isBase = ([Math]::Abs($p.R - $base.R) -le 6 -and [Math]::Abs($p.G - $base.G) -le 6 -and [Math]::Abs($p.B - $base.B) -le 6)
      if ($isBase) { $pinkEnd = $r }
      if ($lum -lt $minLum) { $minLum = $lum; $minR = $r }
      if ($bgR -lt 0 -and $lum -gt 215) { $bgR = $r }
    }
    $lines.Add(("{0} deg={1,3} fillEnd={2,5:F1} minLum={3,6:F1} atR={4,5:F1} bgStart={5,5:F1}" -f $label, $deg, $pinkEnd, $minLum, $minR, $bgR))
  }
}
Scan $Ax $Ay 'ADD'
Scan $Sx $Sy 'SEND'
[System.IO.File]::WriteAllLines($Out, $lines)
$bmp.Dispose()
Write-Host "written $Out"
