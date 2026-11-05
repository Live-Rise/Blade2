Add-Type -AssemblyName System.Drawing
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S0-default.png')
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\S0-default.png')
$pts = @(@(1200,700),@(1500,500),@(900,400),@(1700,900),@(1300,300),@(650,650))
foreach ($pt in $pts) {
  $pm = $m.GetPixel($pt[0], $pt[1]); $pr = $r.GetPixel($pt[0], $pt[1])
  Write-Host ("{0},{1}  main R={2} G={3} B={4}   rust R={5} G={6} B={7}" -f $pt[0], $pt[1], $pm.R, $pm.G, $pm.B, $pr.R, $pr.G, $pr.B)
}
$m.Dispose(); $r.Dispose()
