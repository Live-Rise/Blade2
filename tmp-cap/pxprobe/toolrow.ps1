Add-Type -AssemblyName System.Drawing
function CropZoom($src, $x, $y, $w, $h, $out) {
  $bmp = [System.Drawing.Bitmap]::FromFile($src)
  $rect = New-Object System.Drawing.Rectangle($x, $y, $w, $h)
  $crop = $bmp.Clone($rect, $bmp.PixelFormat)
  $zoom = New-Object System.Drawing.Bitmap(($w*3), ($h*3))
  $g = [System.Drawing.Graphics]::FromImage($zoom)
  $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
  $g.DrawImage($crop, 0, 0, ($w*3), ($h*3))
  $zoom.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $zoom.Dispose(); $crop.Dispose(); $bmp.Dispose()
}
# main S2: 工具调用行 preview y≈735 → device 941;  Pwsh 行 y≈977 → device 1250
CropZoom 'E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S2-session.png' 580 915 900 60 'E:\Syncthing\DshWinUI\tmp-cap\pxprobe\main-toolrow.png'
CropZoom 'E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S2-session.png' 580 1225 900 60 'E:\Syncthing\DshWinUI\tmp-cap\pxprobe\main-toolrow2.png'
