param(
  [string]$Png = "E:\Syncthing\DshWinUI\tmp-cap\new-main.png",
  [int]$Cx = 2455, [int]$Cy = 1515, [int]$R = 40
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
for ($y = $Cy - $R; $y -le $Cy + $R; $y++) {
  $p = $bmp.GetPixel($Cx, $y)
  $lum = 0.299 * $p.R + 0.587 * $p.G + 0.114 * $p.B
  $l2 = $bmp.GetPixel($Cx - 20, $y)
  $l3 = $bmp.GetPixel($Cx + 20, $y)
  $lum2 = 0.299 * $l2.R + 0.587 * $l2.G + 0.114 * $l2.B
  $lum3 = 0.299 * $l3.R + 0.587 * $l3.G + 0.114 * $l3.B
  Write-Host ("y={0} dy={1,3} c=#{2:X2}{3:X2}{4:X2} lum={5,6:F1} | x-20=#{6:X2}{7:X2}{8:X2} lum={9,6:F1} | x+20=#{10:X2}{11:X2}{12:X2} lum={13,6:F1}" -f `
    $y, ($y - $Cy), $p.R, $p.G, $p.B, $lum, $l2.R, $l2.G, $l2.B, $lum2, $l3.R, $l3.G, $l3.B, $lum3)
}
$bmp.Dispose()
