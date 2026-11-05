Add-Type -AssemblyName System.Drawing
function VProfile($bmp, $x, $y0, $y1, $label) {
  $line = @()
  for ($y = $y0; $y -lt $y1; $y++) {
    $px = $bmp.GetPixel($x, $y)
    $line += ("{0}:{1}" -f $y, [int](($px.R + $px.G + $px.B) / 3.0))
  }
  Write-Host ("{0}: {1}" -f $label, ($line -join ' '))
}
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\P-agent-presets.png')
# main 查看钮：device x≈2190（按钮中部），扫描 y 515-590
VProfile $m 2190 515 590 'main 查看钮纵剖面(x=2190)'
$m.Dispose()
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\P-agent-presets.png')
VProfile $r 2210 515 590 'rust 查看钮纵剖面(x=2210)'
$r.Dispose()
