Add-Type -AssemblyName System.Drawing

$jobs = @(
  @{src='E:\Syncthing\DshWinUI\tmp-cap\mainline4\S0-default.png'; out='E:\Syncthing\DshWinUI\tmp-cap\cmp\m-send.png';    dx=1570; dy=1140; dw=425; dh=100},
  @{src='E:\Syncthing\DshWinUI\tmp-cap\rustline4\S0-default.png'; out='E:\Syncthing\DshWinUI\tmp-cap\cmp\r-send.png';    dx=1840; dy=1180; dw=160; dh=112},
  @{src='E:\Syncthing\DshWinUI\tmp-cap\mainline4\S0-default.png'; out='E:\Syncthing\DshWinUI\tmp-cap\cmp\m-tools.png';   dx=440;  dy=1145; dw=780;  dh=95},
  @{src='E:\Syncthing\DshWinUI\tmp-cap\rustline4\S0-default.png'; out='E:\Syncthing\DshWinUI\tmp-cap\cmp\r-tools.png';   dx=440;  dy=1185; dw=1060; dh=100},
  @{src='E:\Syncthing\DshWinUI\tmp-cap\mainline4\S0-default.png'; out='E:\Syncthing\DshWinUI\tmp-cap\cmp\m-groups.png';  dx=0;    dy=270;  dw=430;  dh=135},
  @{src='E:\Syncthing\DshWinUI\tmp-cap\rustline4\S0-default.png'; out='E:\Syncthing\DshWinUI\tmp-cap\cmp\r-groups.png';  dx=0;    dy=275;  dw=430;  dh=140},
  @{src='E:\Syncthing\DshWinUI\tmp-cap\mainline4\S0-default.png'; out='E:\Syncthing\DshWinUI\tmp-cap\cmp\m-edge.png';    dx=0;    dy=180;  dw=36;   dh=950},
  @{src='E:\Syncthing\DshWinUI\tmp-cap\rustline4\S0-default.png'; out='E:\Syncthing\DshWinUI\tmp-cap\cmp\r-edge.png';    dx=0;    dy=180;  dw=36;   dh=960}
)

foreach ($j in $jobs) {
  $img = [System.Drawing.Bitmap]::FromFile($j.src)
  $sx = $img.Width / 2000.0
  $sy = $img.Height / 1250.0
  if ($j.src -like '*rustline*') { $sy = $img.Height / 1292.0 }
  $x = [int]($j.dx * $sx); $y = [int]($j.dy * $sy)
  $w = [int]($j.dw * $sx); $h = [int]($j.dh * $sy)
  if ($x + $w -gt $img.Width)  { $w = $img.Width  - $x }
  if ($y + $h -gt $img.Height) { $h = $img.Height - $y }
  $rect = New-Object System.Drawing.Rectangle($x, $y, $w, $h)
  $crop = $img.Clone($rect, $img.PixelFormat)
  # 放大 2 倍便于观察
  $big = New-Object System.Drawing.Bitmap($crop, ($w*2), ($h*2))
  $big.Save($j.out, [System.Drawing.Imaging.ImageFormat]::Png)
  $big.Dispose(); $crop.Dispose()
  "{0} -> {1} (crop at {2},{3} {4}x{5})" -f (Split-Path $j.src -Leaf), $j.out, $x, $y, $w, $h
  $img.Dispose()
}

# 取发送按钮中心像素颜色
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\mainline4\S0-default.png')
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\rustline4\S0-default.png')
"$($m.Width)x$($m.Height)  /  $($r.Width)x$($r.Height)"
foreach ($off in @(0, -12, 12)) {
  $pm = $m.GetPixel([int](1921*1.28)+$off, [int](1184*1.28))
  $pr = $r.GetPixel([int](1921*(2586.0/2000))+0, [int](1226*(1671.0/1292))+$off)
  "mainline send px({0}): R={1} G={2} B={3} | rustline send px({4}): R={5} G={6} B={7}" -f $off,$pm.R,$pm.G,$pm.B,$off,$pr.R,$pr.G,$pr.B
}
$m.Dispose(); $r.Dispose()
