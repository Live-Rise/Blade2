Add-Type -AssemblyName System.Drawing
$poster = [System.Drawing.Bitmap]::FromFile('C:\Users\Admin\AppData\Local\Blade2\cache\shell-skin.poster.bmp')
$m = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-main-R3\S1-settings.png')
$r = [System.Drawing.Bitmap]::FromFile('E:\Syncthing\DshWinUI\tmp-cap\parity-rust-R3\S1-settings.png')
# 窗口几何（device px, 2x DPI）
# main: 2560x1600; 侧栏宽≈537, 内容顶≈102  (顶条 48*2 + 余量)
# rust: 2586x1671; 侧栏宽≈543, 内容顶≈96
# 海报 3840x2130 UniformToFill
function MapMain([int]$sx, [int]$sy) {
  $cw = 2560 - 537; $ch = 1600 - 102
  $scale = [Math]::Max($cw / 3840.0, $ch / 2130.0)
  $cropX = (3840 - $cw / $scale) / 2.0; $cropY = (2130 - $ch / $scale) / 2.0
  ,@( [int](537 + ($sx - $cropX) * $scale), [int](102 + ($sy - $cropY) * $scale) )
}
function MapRust([int]$sx, [int]$sy) {
  $cw = 2586 - 543; $ch = 1671 - 96
  $scale = [Math]::Max($cw / 3840.0, $ch / 2130.0)
  $cropX = (3840 - $cw / $scale) / 2.0; $cropY = (2130 - $ch / $scale) / 2.0
  ,@( [int](543 + ($sx - $cropX) * $scale), [int](96 + ($sy - $cropY) * $scale) )
}
function Val($bmp, $x, $y) { $px = $bmp.GetPixel($x, $y); return (($px.R + $px.G + $px.B) / 3.0) }
$pts = @(@(700,300),@(900,500),@(1200,300),@(1600,400),@(1900,600),@(2200,800),@(2500,1100),@(2800,1400),@(3100,1600),@(1700,1800),@(2100,1900),@(600,1800))
foreach ($p in $pts) {
  $v = Val $poster $p[0] $p[1]
  $mm = MapMain $p[0] $p[1]; $rr = MapRust $p[0] $p[1]
  $vm = Val $m $mm[0] $mm[1]; $vr = Val $r $rr[0] $rr[1]
  Write-Host ("src({0},{1}) V={2:F0}   main@({3},{4})={5:F0}   rust@({6},{7})={8:F0}" -f $p[0], $p[1], $v, $mm[0], $mm[1], $vm, $rr[0], $rr[1], $vr)
}
$poster.Dispose(); $m.Dispose(); $r.Dispose()
