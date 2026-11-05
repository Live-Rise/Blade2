param([string]$Png, [int]$X, [int]$Y)
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
$p = $bmp.GetPixel($X, $Y)
Write-Host ("{0} @ {1},{2} = R{3} G{4} B{5} A{6}" -f (Split-Path $Png -Leaf), $X, $Y, $p.R, $p.G, $p.B, $p.A)
$bmp.Dispose()
