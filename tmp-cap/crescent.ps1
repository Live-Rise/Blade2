# 精确定位发送键暗月牙：给出每行的圆跨度与暗像素 x 区间
param(
  [string]$Png = "E:\Syncthing\DshWinUI\tmp-cap\blade2-main.png",
  [int]$Cx = 2455, [int]$Cy = 1515, [int]$R = 34
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
Write-Host "image: $($bmp.Width)x$($bmp.Height)"
# 基准粉色 = 圆心像素
$base = $bmp.GetPixel($Cx, $Cy)
Write-Host ("center pixel: #{0:X2}{1:X2}{2:X2}" -f $base.R, $base.G, $base.B)
for ($y = $Cy - $R - 6; $y -le $Cy + $R + 6; $y++) {
  if ($y -lt 0 -or $y -ge $bmp.Height) { continue }
  $dy = [int]$y - $Cy
  $inner = [int][Math]::Floor([Math]::Sqrt([Math]::Max(0, $R * $R - $dy * $dy)))
  # 圆内暗像素
  $dark = @()
  for ($x = $Cx - $inner; $x -le $Cx + $inner; $x++) {
    if ($x -lt 0 -or $x -ge $bmp.Width) { continue }
    $p = $bmp.GetPixel($x, $y)
    $lum = 0.299 * $p.R + 0.587 * $p.G + 0.114 * $p.B
    $bl = 0.299 * $base.R + 0.587 * $base.G + 0.114 * $base.B
    if ($lum -lt $bl - 25) { $dark += $x }
  }
  # 圆外暗像素（±3 列于圆跨度之外）
  $out = @()
  for ($x = $Cx - $inner - 4; $x -le $Cx + $inner + 4; $x++) {
    if ($x -ge $Cx - $inner -and $x -le $Cx + $inner) { continue }
    if ($x -lt 0 -or $x -ge $bmp.Width) { continue }
    $p = $bmp.GetPixel($x, $y)
    $lum = 0.299 * $p.R + 0.587 * $p.G + 0.114 * $p.B
    $bl = 0.299 * $base.R + 0.587 * $base.G + 0.114 * $base.B
    if ($lum -lt $bl - 25) { $out += $x }
  }
  $dr = if ($dark.Count) { "{0}..{1}" -f $dark[0], $dark[-1] } else { "-" }
  $or_ = if ($out.Count) { "{0}..{1}" -f $out[0], $out[-1] } else { "-" }
  Write-Host ("y={0} dy={1,3} inner={1,3} darkIn={2,3} [{3,-12}] darkOut={4,3} [{5}]" -f $y, $dy, $inner, $dr, $out.Count, $or_)
}
$bmp.Dispose()
