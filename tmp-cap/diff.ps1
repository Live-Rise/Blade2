param(
  [string]$PathA = "E:\Syncthing\DshWinUI\tmp-cap\old-main.png",
  [string]$PathB = "E:\Syncthing\DshWinUI\tmp-cap\new-main.png",
  [int]$X0 = 2400, [int]$X1 = 2510, [int]$Y0 = 1460, [int]$Y1 = 1560
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$imgA = New-Object System.Drawing.Bitmap($PathA)
$imgB = New-Object System.Drawing.Bitmap($PathB)
$n = 0; $maxd = 0; $sx = -1; $sy = -1
for ($y = $Y0; $y -le $Y1; $y++) {
  for ($x = $X0; $x -le $X1; $x++) {
    $pa = $imgA.GetPixel($x, $y); $pb = $imgB.GetPixel($x, $y)
    $d = [Math]::Max([Math]::Abs($pa.R - $pb.R), [Math]::Max([Math]::Abs($pa.G - $pb.G), [Math]::Abs($pa.B - $pb.B)))
    if ($d -gt 6) { $n++; if ($d -gt $maxd) { $maxd = $d; $sx = $x; $sy = $y } }
  }
}
Write-Host ("diff pixels(>6): {0}  maxDelta={1} at ({2},{3})" -f $n, $maxd, $sx, $sy)
$imgA.Dispose(); $imgB.Dispose()
