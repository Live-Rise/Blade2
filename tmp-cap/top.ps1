param([string]$Path)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$img = New-Object System.Drawing.Bitmap($Path)
for ($y = 1478; $y -le 1486; $y++) {
  $p = $img.GetPixel(2455, $y)
  Write-Host ("{0} y={1} #{2:X2}{3:X2}{4:X2} lum={5,6:F1}" -f (Split-Path $Path -Leaf), $y, $p.R, $p.G, $p.B, (0.299*$p.R + 0.587*$p.G + 0.114*$p.B))
}
$img.Dispose()
