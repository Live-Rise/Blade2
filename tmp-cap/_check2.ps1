$paths = @(
  'E:\Syncthing\DshWinUI\tmp-cap\rustline2\S0-default.png',
  'E:\Syncthing\DshWinUI\tmp-cap\rustline2\S1-settings.png',
  'E:\Syncthing\DshWinUI\tmp-cap\mainline2\S0-default.png',
  'E:\Syncthing\DshWinUI\tmp-cap\mainline2\S1-settings.png',
  'E:\Syncthing\DshWinUI\tmp-cap\rustline\S1-settings.png'
)
Add-Type -AssemblyName System.Drawing
foreach ($p in $paths) {
  $hash = (Get-FileHash -LiteralPath $p -Algorithm MD5).Hash
  $img = [System.Drawing.Image]::FromFile($p)
  Write-Host ("{0}  {1}x{2}  {3}" -f $hash, $img.Width, $img.Height, $p)
  $img.Dispose()
}
