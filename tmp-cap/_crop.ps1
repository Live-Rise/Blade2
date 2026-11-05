param([string]$Path, [int]$X, [int]$Y, [int]$W, [int]$H, [string]$Out)
Add-Type -AssemblyName System.Drawing
$src = [System.Drawing.Bitmap]::FromFile($Path)
$rect = New-Object System.Drawing.Rectangle($X, $Y, $W, $H)
$dst = $src.Clone($rect, $src.PixelFormat)
$dst.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$src.Dispose(); $dst.Dispose()
Write-Host ("saved {0}  from {1}  ({2},{3}) {4}x{5}" -f $Out, $Path, $X, $Y, $W, $H)
