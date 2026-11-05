Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::FromFile('C:\Users\Admin\AppData\Local\Blade2\cache\shell-skin.poster.bmp')
$rect = New-Object System.Drawing.Rectangle(($bmp.Width-1400), ($bmp.Height-260), 1400, 260)
$crop = $bmp.Clone($rect, $bmp.PixelFormat)
$crop.Save('E:\Syncthing\DshWinUI\tmp-cap\pxprobe\poster-br.png', [System.Drawing.Imaging.ImageFormat]::Png)
$crop.Dispose(); $bmp.Dispose()
# 主干 P-usage 右下也裁一张
$bmp2 = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\P-usage.png')
$rect2 = New-Object System.Drawing.Rectangle(1900, 1450, 660, 150)
$crop2 = $bmp2.Clone($rect2, $bmp2.PixelFormat)
$crop2.Save('E:\Syncthing\DshWinUI\tmp-cap\pxprobe\main-usage-br.png', [System.Drawing.Imaging.ImageFormat]::Png)
$crop2.Dispose(); $bmp2.Dispose()
