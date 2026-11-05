# 精确输出发送键底部每行的「暗像素 run」与圆跨度，写文件避免终端格式干扰
param(
  [string]$Png = "E:\Syncthing\DshWinUI\tmp-cap\blade2-main.png",
  [string]$Out = "E:\Syncthing\DshWinUI\tmp-cap\crescent-runs.txt",
  [int]$Cx = 2455, [int]$Cy = 1515, [int]$R = 34
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
$lines = New-Object System.Collections.Generic.List[string]
$base = $bmp.GetPixel($Cx, $Cy)
$bl = 0.299 * $base.R + 0.587 * $base.G + 0.114 * $base.B
$lines.Add(("base=#{0:X2}{1:X2}{2:X2} lum={3:F1}" -f $base.R, $base.G, $base.B, $bl))
for ($y = 1540; $y -le 1552; $y++) {
  $dy = $y - $Cy
  $half = [Math]::Sqrt([Math]::Max(0, $R * $R - $dy * $dy))
  $runStrs = New-Object System.Collections.Generic.List[string]
  $a = -1; $b = -1; $n = 0
  for ($x = $Cx - 60; $x -le $Cx + 61; $x++) {
    $p = $bmp.GetPixel($x, $y)
    $lum = 0.299 * $p.R + 0.587 * $p.G + 0.114 * $p.B
    $dark = ($lum -lt ($bl - 22))
    if ($dark) {
      if ($a -lt 0) { $a = $x }
      $b = $x; $n++
    }
    else {
      if ($a -ge 0) { $runStrs.Add(("[{0}..{1} n={2}]" -f $a, $b, $n)); $a = -1; $n = 0 }
    }
  }
  if ($a -ge 0) { $runStrs.Add(("[{0}..{1} n={2}]" -f $a, $b, $n)) }
  $s = ("y={0} dy={1,3} half={2,5:F1} span={3}..{4}  " -f $y, $dy, $half, [int]($Cx - [Math]::Floor($half)), [int]($Cx + [Math]::Floor($half)))
  $s += ($runStrs -join ' ')
  $lines.Add($s)
}
[System.IO.File]::WriteAllLines($Out, $lines)
$bmp.Dispose()
Write-Host "written $Out"
