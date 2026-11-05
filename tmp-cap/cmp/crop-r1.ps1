Add-Type -AssemblyName System.Drawing

$main = 'E:\Syncthing\DshWinUI\tmp-cap\parity-main-R1\S0-default.png'
$rust = 'E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R1\S0-default.png'
$out  = 'E:\Syncthing\DshWinUI\tmp-cap\cmp'

# 逻辑坐标空间：主干 2000x1250，重写线 2000x1292
$jobs = @(
  @{src=$main; tag='m'; sx=2560/2000.0; sy=1600/1250.0},
  @{src=$rust; tag='r'; sx=2586/2000.0; sy=1671/1292.0}
)

$regions = @(
  @{name='search'; dx=600;  dy=0;    dw=760;  dh=80},
  @{name='rows';   dx=0;    dy=260;  dw=430;  dh=160},
  @{name='input';  dx=440;  dy=1080; dw=1560; dh=212},
  @{name='empty';  dx=900;  dy=440;  dw=700;  dh=260},
  @{name='bg-top'; dx=1500; dy=60;   dw=500;  dh=320},
  @{name='bg-bot'; dx=600;  dy=950;  dw=700;  dh=260}
)

foreach ($j in $jobs) {
  $img = [System.Drawing.Bitmap]::FromFile($j.src)
  foreach ($rg in $regions) {
    $x = [int]($rg.dx * $j.sx); $y = [int]($rg.dy * $j.sy)
    $w = [int]($rg.dw * $j.sx); $h = [int]($rg.dh * $j.sy)
    if ($x + $w -gt $img.Width)  { $w = $img.Width  - $x }
    if ($y + $h -gt $img.Height) { $h = $img.Height - $y }
    $rect = New-Object System.Drawing.Rectangle($x, $y, $w, $h)
    $crop = $img.Clone($rect, $img.PixelFormat)
    $big = New-Object System.Drawing.Bitmap($crop, ($w*2), ($h*2))
    $big.Save("$out\r1-$($j.tag)-$($rg.name).png", [System.Drawing.Imaging.ImageFormat]::Png)
    $big.Dispose(); $crop.Dispose()
  }
  $img.Dispose()
}

# 发送键中心取色（逻辑坐标：主干 1920,1213；重写线 1919,1226）
$m = [System.Drawing.Bitmap]::FromFile($main)
$r = [System.Drawing.Bitmap]::FromFile($rust)
$mpx = [int](1920 * 2560/2000.0); $mpy = [int](1213 * 1600/1250.0)
$rpx = [int](1919 * 2586/2000.0); $rpy = [int](1226 * 1671/1292.0)
foreach ($off in @(0, -10, 10, 0, -18, 18)) {}
foreach ($off in @(0, -10, 10, -18, 18)) {
  $pm = $m.GetPixel($mpx + $off, $mpy)
  $pr = $r.GetPixel($rpx + $off, $rpy)
  "send px off={0}  main R={1} G={2} B={3}  |  rust R={4} G={5} B={6}" -f $off, $pm.R, $pm.G, $pm.B, $pr.R, $pr.G, $pr.B
}
# 模型名回显区域宽度（逻辑 y≈1213/1226 那一行，从右往左找非背景像素起点）——略，靠裁图目测
$m.Dispose(); $r.Dispose()
Get-ChildItem $out -Filter 'r1-*.png' | Select-Object Name, Length | Format-Table -AutoSize
