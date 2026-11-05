Add-Type -AssemblyName System.Drawing
function CropRow($src, $y0, $y1, $out) {
  $bmp = [System.Drawing.Bitmap]::FromFile($src)
  $rect = New-Object System.Drawing.Rectangle(0, $y0, 560, ($y1-$y0))
  $crop = $bmp.Clone($rect, $bmp.PixelFormat)
  $crop.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
  $crop.Dispose(); $bmp.Dispose()
}
CropRow 'E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S0-default.png' 449 482 'E:\Syncthing\DshWinUI\tmp-cap\pxprobe\main-groupright.png'
CropRow 'E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\S0-default.png' 457 490 'E:\Syncthing\DshWinUI\tmp-cap\pxprobe\rust-groupright.png'
