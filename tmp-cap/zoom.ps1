# 裁出发送键并放大，便于直接看形状
param(
  [string]$Png = "E:\Syncthing\DshWinUI\tmp-cap\blade2-main.png",
  [string]$Out = "E:\Syncthing\DshWinUI\tmp-cap\send-zoom.png",
  [int]$X = 2400, [int]$Y = 1460, [int]$W = 112, [int]$H = 112, [int]$Zoom = 4
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$src = New-Object System.Drawing.Bitmap($Png)
$dw = $W * $Zoom
$dh = $H * $Zoom
$bmp = New-Object System.Drawing.Bitmap($dw, $dh)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.InterpolationMode = 'NearestNeighbor'
$srcRect = New-Object System.Drawing.Rectangle($X, $Y, $W, $H)
$destRect = New-Object System.Drawing.Rectangle(0, 0, $dw, $dh)
$g.DrawImage($src, $destRect, $srcRect, [System.Drawing.GraphicsUnit]::Pixel)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose(); $src.Dispose()
Write-Host "saved: $Out ($($dw)x$($dh))"
