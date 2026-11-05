Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\P-memory.png')
$rect = New-Object System.Drawing.Rectangle(600, 1400, 1986, 270)
$crop = $bmp.Clone($rect, $bmp.PixelFormat)
$crop.Save('E:\Syncthing\DshWinUI\tmp-cap\pxprobe\rust-mem-bottom.png', [System.Drawing.Imaging.ImageFormat]::Png)
$crop.Dispose(); $bmp.Dispose()
