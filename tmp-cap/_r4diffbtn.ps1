param([string]$PngA, [int]$AX, [int]$AY, [string]$PngB, [int]$BX, [int]$BY, [int]$R)
Add-Type -AssemblyName System.Drawing
$ba = New-Object System.Drawing.Bitmap($PngA)
$bb = New-Object System.Drawing.Bitmap($PngB)
$diffs = @()
for ($dy = -$R; $dy -le $R; $dy++) {
  $row = ""
  for ($dx = -$R; $dx -le $R; $dx++) {
    $pa = $ba.GetPixel($AX + $dx, $AY + $dy)
    $pb = $bb.GetPixel($BX + $dx, $BY + $dy)
    $la = [int](($pa.R + $pa.G + $pa.B) / 3)
    $lb = [int](($pb.R + $pb.G + $pb.B) / 3)
    $d = $lb - $la
    if ($d -gt 15) { $row += "#" }
    elseif ($d -gt 6) { $row += "+" }
    elseif ($d -lt -15) { $row += "@" }
    elseif ($d -lt -6) { $row += "-" }
    else { $row = $row + "." }
  }
  Write-Host $row
}
$ba.Dispose(); $bb.Dispose()
