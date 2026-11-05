Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S0-default.png')
$rect = New-Object System.Drawing.Rectangle(30, 515, 480, 60)
$crop = $bmp.Clone($rect, $bmp.PixelFormat)
$zoom = New-Object System.Drawing.Bitmap(960, 120)
$g = [System.Drawing.Graphics]::FromImage($zoom)
$g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
$g.DrawImage($crop, 0, 0, 960, 120)
$zoom.Save('E:\Syncthing\DshWinUI\tmp-cap\pxprobe\main-group-zoom.png', [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $zoom.Dispose(); $crop.Dispose(); $bmp.Dispose()
$bmp2 = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\S0-default.png')
$rect2 = New-Object System.Drawing.Rectangle(30, 462, 480, 60)
$crop2 = $bmp2.Clone($rect2, $bmp2.PixelFormat)
$zoom2 = New-Object System.Drawing.Bitmap(960, 120)
$g2 = [System.Drawing.Graphics]::FromImage($zoom2)
$g2.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
$g2.DrawImage($crop2, 0, 0, 960, 120)
$zoom2.Save('E:\Syncthing\DshWinUI\tmp-cap\pxprobe\rust-group-zoom.png', [System.Drawing.Imaging.ImageFormat]::Png)
$g2.Dispose(); $zoom2.Dispose(); $crop2.Dispose(); $bmp2.Dispose()
