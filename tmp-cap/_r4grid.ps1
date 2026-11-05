param([string]$Png, [int]$X0, [int]$X1, [int]$Y0, [int]$Y1)
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
for ($y = $Y0; $y -le $Y1; $y++) {
  $row = ""
  for ($x = $X0; $x -le $X1; $x++) {
    $p = $bmp.GetPixel($x, $y)
    $lum = [int](($p.R + $p.G + $p.B) / 3)
    $row += ("{0,4}" -f $lum)
  }
  Write-Host ("y={0,4}: {1}" -f $y, $row)
}
$bmp.Dispose()
