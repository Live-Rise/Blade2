param(
  [string]$Png = "E:\Syncthing\DshWinUI\tmp-cap\new-main.png",
  [int]$X = 0, [int]$Y = 0, [int]$W = 0, [int]$H = 0,
  [int]$Scale = 1,
  [string]$Out = "E:\Syncthing\DshWinUI\tmp-cap\crop2.png"
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
if ($W -le 0) { $W = $bmp.Width - $X }
if ($H -le 0) { $H = $bmp.Height - $Y }
$cw = $W * $Scale; $ch = $H * $Scale
$c = New-Object System.Drawing.Bitmap($cw, $ch)
$g = [System.Drawing.Graphics]::FromImage($c)
$g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
$g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::Half
$g.DrawImage($bmp, (New-Object System.Drawing.Rectangle 0, 0, $cw, $ch), $X, $Y, $W, $H, ([System.Drawing.GraphicsUnit]::Pixel))
$c.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
Write-Host "crop $X,$Y ${W}x${H} x$Scale -> $Out"
$g.Dispose(); $c.Dispose(); $bmp.Dispose()
