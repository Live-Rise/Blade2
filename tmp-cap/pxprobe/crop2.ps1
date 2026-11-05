Add-Type -AssemblyName System.Drawing
function CropSave($src, $x, $y, $w, $h, $out) {
  $bmp = [System.Drawing.Bitmap]::FromFile($src)
  $rect = New-Object System.Drawing.Rectangle($x, $y, $w, $h)
  $crop = $bmp.Clone($rect, $bmp.PixelFormat)
  $crop.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
  $crop.Dispose(); $bmp.Dispose()
}
CropSave 'E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\S1-settings.png' 1200 240 500 300 'E:\Syncthing\DshWinUI\tmp-cap\pxprobe\rust-hair.png'
CropSave 'E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S1-settings.png' 1150 240 500 300 'E:\Syncthing\DshWinUI\tmp-cap\pxprobe\main-hair.png'
