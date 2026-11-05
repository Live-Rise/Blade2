Add-Type -AssemblyName System.Drawing
function CropSave($src, $x, $y, $w, $h, $out) {
  $bmp = [System.Drawing.Bitmap]::FromFile($src)
  $rect = New-Object System.Drawing.Rectangle($x, $y, $w, $h)
  $crop = $bmp.Clone($rect, $bmp.PixelFormat)
  $crop.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
  $crop.Dispose(); $bmp.Dispose()
}
# 持久记忆行右半（toggle 区域）：main 卡右缘约 2470 device(2000*1.28/0.78?) — 直接裁 1500-2560
CropSave 'E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\P-memory.png' 1500 590 1060 110 'E:\Syncthing\DshWinUI\tmp-cap\pxprobe\main-mem-toggle.png'
CropSave 'E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\P-memory.png' 1500 620 1086 120 'E:\Syncthing\DshWinUI\tmp-cap\pxprobe\rust-mem-toggle.png'
