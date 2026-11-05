Add-Type -AssemblyName System.Drawing
function CropSave($src, $x, $y, $w, $h, $out) {
  $bmp = [System.Drawing.Bitmap]::FromFile($src)
  $rect = New-Object System.Drawing.Rectangle($x, $y, $w, $h)
  $crop = $bmp.Clone($rect, $bmp.PixelFormat)
  $crop.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
  $crop.Dispose(); $bmp.Dispose()
}
# main: 编辑器底+状态行 (2000-scale y 1190-1240 → device 1523-1587)
CropSave 'E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\P-memory.png' 600 1500 1200 90 'E:\Syncthing\DshWinUI\tmp-cap\pxprobe\main-mem-state.png'
# rust: 编辑器底 (preview 1240-1290 → device 1603-1668)
CropSave 'E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\P-memory.png' 600 1560 1200 110 'E:\Syncthing\DshWinUI\tmp-cap\pxprobe\rust-mem-state.png'
