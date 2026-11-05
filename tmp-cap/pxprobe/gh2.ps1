Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S0-default.png')
$rect = New-Object System.Drawing.Rectangle(30, 448, 480, 62)
$crop = $bmp.Clone($rect, $bmp.PixelFormat)
$zoom = New-Object System.Drawing.Bitmap(960, 124)
$g = [System.Drawing.Graphics]::FromImage($zoom)
$g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
$g.DrawImage($crop, 0, 0, 960, 124)
$zoom.Save('E:\Syncthing\DshWinUI\tmp-cap\pxprobe\main-group-zoom2.png', [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $zoom.Dispose(); $crop.Dispose(); $bmp.Dispose()
