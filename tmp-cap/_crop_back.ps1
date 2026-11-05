Add-Type -AssemblyName System.Drawing
foreach ($name in @('parity-main-R2', 'parity-rust-R2')) {
    $path = "E:\Syncthing\DshWinUI\tmp-cap\$name\S1-settings.png"
    $src = [System.Drawing.Image]::FromFile($path)
    $rect = New-Object System.Drawing.Rectangle(0, 0, 300, 70)
    $bmp = New-Object System.Drawing.Bitmap(300, 70)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $dest = New-Object System.Drawing.Rectangle(0, 0, 300, 70)
    $g.DrawImage($src, $dest, $rect, [System.Drawing.GraphicsUnit]::Pixel)
    $g.Dispose()
    $bmp.Save("E:\Syncthing\DshWinUI\tmp-cap\back-s1-$name.png")
    $bmp.Dispose()
    $src.Dispose()
}
'done'
