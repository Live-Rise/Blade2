$paths = @(
  'E:\Syncthing\DshWinUI\tmp-cap\mainline2\S1-settings.png',
  'E:\Syncthing\DshWinUI\tmp-cap\rustline2\S1-settings.png'
)
Add-Type -AssemblyName System.Drawing
foreach ($p in $paths) {
  $item = Get-Item -LiteralPath $p
  $hash = (Get-FileHash -LiteralPath $p -Algorithm MD5).Hash
  $img = [System.Drawing.Image]::FromFile($p)
  Write-Host ("{0}`n  size={1} bytes  modified={2}`n  md5={3}`n  dims={4}x{5}" -f $p, $item.Length, $item.LastWriteTime, $hash, $img.Width, $img.Height)
  $img.Dispose()
}
Get-ChildItem -LiteralPath 'E:\Syncthing\DshWinUI\tmp-cap' -Recurse -File | Select-Object FullName, Length, LastWriteTime | Format-Table -AutoSize | Out-String -Width 300
