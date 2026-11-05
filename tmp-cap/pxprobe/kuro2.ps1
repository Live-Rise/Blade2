Add-Type -AssemblyName System.Drawing
foreach ($pair in @(@('P-usage',1950,1480), @('S1-settings',1950,1480), @('S0-default',1950,1480))) {
  $bmp = [System.Drawing.Bitmap]::FromFile("E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\$($pair[0]).png")
  $rect = New-Object System.Drawing.Rectangle($pair[1], $pair[2], 630, 185)
  $crop = $bmp.Clone($rect, $bmp.PixelFormat)
  $crop.Save("E:\Syncthing\DshWinUI\tmp-cap\pxprobe\rust-$($pair[0])-br.png", [System.Drawing.Imaging.ImageFormat]::Png)
  $crop.Dispose(); $bmp.Dispose()
}
