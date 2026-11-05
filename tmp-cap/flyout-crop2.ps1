# 裁弹层区：参数 x,y,w,h（截图内像素），存到 -Out
param(
    [Parameter(Mandatory)][string]$In,
    [Parameter(Mandatory)][string]$Out,
    [int]$X, [int]$Y, [int]$W, [int]$H
)
Add-Type -AssemblyName System.Drawing
$src = [System.Drawing.Image]::FromFile($In)
if ($W -le 0) { $W = $src.Width }
if ($H -le 0) { $H = $src.Height }
if ($X -lt 0) { $X = 0 }
if ($Y -lt 0) { $Y = 0 }
if ($X + $W -gt $src.Width) { $W = $src.Width - $X }
if ($Y + $H -gt $src.Height) { $H = $src.Height - $Y }
$bmp = New-Object System.Drawing.Bitmap($W, $H)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.DrawImage($src, (New-Object System.Drawing.Rectangle(0, 0, $W, $H)), (New-Object System.Drawing.Rectangle($X, $Y, $W, $H)), [System.Drawing.GraphicsUnit]::Pixel)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose(); $src.Dispose()
Write-Host "saved $Out ($W x $H)"
