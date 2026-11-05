param(
  [string]$Png = "E:\Syncthing\DshWinUI\tmp-cap\new-main.png",
  [int]$Cx = 2455, [int]$Cy = 1515,
  [int]$X0 = 2418, [int]$X1 = 2492, [int]$Y0 = 1476, [int]$Y1 = 1556
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
# 图例：底色用字符表示亮度档
function Ch($p) {
  $lum = 0.299 * $p.R + 0.587 * $p.G + 0.114 * $p.B
  if ($lum -lt 130) { return '#' }        # 暗（描边/黑）
  if ($lum -lt 160) { return '+' }        # 中暗
  if ($lum -lt 185) { return 'o' }        # 粉
  if ($lum -lt 215) { return '-' }        # 过渡
  return '.'                               # 亮背景
}
$hdr = "     "
for ($x = $X0; $x -le $X1; $x += 5) { $hdr += "{0}" -f ([int]($x % 100) / 10) }
Write-Host $hdr
for ($y = $Y0; $y -le $Y1; $y++) {
  $row = ""
  for ($x = $X0; $x -le $X1; $x++) { $row += (Ch $bmp.GetPixel($x, $y)) }
  Write-Host ("{0} {1}" -f $y, $row)
}
$bmp.Dispose()
