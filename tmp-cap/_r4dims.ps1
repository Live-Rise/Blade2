Add-Type -AssemblyName System.Drawing
$files = @(
  'parity-main-R4\P-pet.png','parity-rust-R4\P-pet.png',
  'parity-main-R4\S0-default.png','parity-rust-R4\S0-default.png',
  'parity-main-R4\S2-session.png','parity-rust-R4\S2-session.png',
  'parity-main-R4\P-memory.png','parity-rust-R4\P-memory.png',
  'parity-main-R4\P-browser-control.png','parity-rust-R4\P-browser-control.png',
  'parity-main-R4\P-general.png','parity-rust-R4\P-general.png'
)
foreach ($f in $files) {
  $p = Join-Path 'E:\Syncthing\DshWinUI\tmp-cap' $f
  $bmp = [System.Drawing.Bitmap]::FromFile($p)
  Write-Host "$f : $($bmp.Width)x$($bmp.Height)"
  $bmp.Dispose()
}
