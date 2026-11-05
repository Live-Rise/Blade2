param(
  [string]$Path = "E:\Syncthing\DshWinUI\tmp-cap\new-main.png",
  [int]$X0 = 2436, [int]$X1 = 2474, [int]$Y0 = 1543, [int]$Y1 = 1550
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$img = New-Object System.Drawing.Bitmap($Path)
for ($y = $Y0; $y -le $Y1; $y++) {
  $parts = @()
  for ($x = $X0; $x -le $X1; $x++) {
    $p = $img.GetPixel($x, $y)
    $parts += ("{0:X2}{1:X2}{2:X2}" -f $p.R, $p.G, $p.B)
  }
  Write-Host ("y={0}: {1}" -f $y, ($parts -join ' '))
}
$img.Dispose()
