param(
  [string]$Png = "E:\Syncthing\DshWinUI\tmp-cap\new-main.png",
  [int]$X0 = 2280, [int]$X1 = 2440, [int]$Y0 = 1440, [int]$Y1 = 1540,
  [int]$TR = 0xC6, [int]$TG = 0x9E, [int]$TB = 0x9E, [int]$Tol = 12
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
$minX = 1e9; $minY = 1e9; $maxX = -1; $maxY = -1; $n = 0
for ($y = $Y0; $y -le $Y1; $y++) {
  for ($x = $X0; $x -le $X1; $x++) {
    $p = $bmp.GetPixel($x, $y)
    if ([Math]::Abs($p.R - $TR) -le $Tol -and [Math]::Abs($p.G - $TG) -le $Tol -and [Math]::Abs($p.B - $TB) -le $Tol) {
      $n++
      if ($x -lt $minX) { $minX = $x }; if ($x -gt $maxX) { $maxX = $x }
      if ($y -lt $minY) { $minY = $y }; if ($y -gt $maxY) { $maxY = $y }
    }
  }
}
Write-Host ("matches={0} box=({1},{2})..({3},{4}) size={5}x{6} center=({7},{8})" -f $n, $minX, $minY, $maxX, $maxY, ($maxX-$minX+1), ($maxY-$minY+1), ([int](($minX+$maxX)/2)), ([int](($minY+$maxY)/2)))
$bmp.Dispose()
