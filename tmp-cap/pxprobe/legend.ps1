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
# main P-usage 图例行（preview y≈833*1.28≈1066 device）
CropZoom 'E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\P-usage.png' 1580 1030 900 80 'E:\Syncthing\DshWinUI\tmp-cap\pxprobe\main-legend-zoom.png'
# rust P-usage 图例行（preview y≈754*1.293≈975 device）
CropZoom 'E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\P-usage.png' 1600 940 900 80 'E:\Syncthing\DshWinUI\tmp-cap\pxprobe\rust-legend-zoom.png'
