Add-Type -AssemblyName System.Drawing
function CropSave($src, $x, $y, $w, $h, $out) {
  $bmp = [System.Drawing.Bitmap]::FromFile($src)
  $rect = New-Object System.Drawing.Rectangle($x, $y, $w, $h)
  $crop = $bmp.Clone($rect, $bmp.PixelFormat)
  $crop.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
  $crop.Dispose(); $bmp.Dispose()
}
CropSave 'E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\P-memory.png' 1500 380 1060 110 'E:\Syncthing\DshWinUI\tmp-cap\pxprobe\main-mem-toggle2.png'
CropSave 'E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\P-memory.png' 1500 400 1086 130 'E:\Syncthing\DshWinUI\tmp-cap\pxprobe\rust-mem-toggle2.png'
