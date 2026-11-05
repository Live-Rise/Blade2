Add-Type -AssemblyName System.Drawing
function CropZoom($src, $x, $y, $w, $h, $out) {
  $bmp = [System.Drawing.Bitmap]::FromFile($src)
  $rect = New-Object System.Drawing.Rectangle($x, $y, $w, $h)
  $crop = $bmp.Clone($rect, $bmp.PixelFormat)
  $zoom = New-Object System.Drawing.Bitmap(($w*2), ($h*2))
  $g = [System.Drawing.Graphics]::FromImage($zoom)
  $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
  $g.DrawImage($crop, 0, 0, ($w*2), ($h*2))
  $zoom.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $zoom.Dispose(); $crop.Dispose(); $bmp.Dispose()
}
# P-agent-presets 右侧操作列
CropZoom 'E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\P-agent-presets.png' 2150 480 400 220 'E:\Syncthing\DshWinUI\tmp-cap\pxprobe\main-preset-btn.png'
CropZoom 'E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\P-agent-presets.png' 2150 480 420 220 'E:\Syncthing\DshWinUI\tmp-cap\pxprobe\rust-preset-btn.png'
