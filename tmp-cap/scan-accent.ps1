# 全图扫描：用「行 run 合并」找所有强调粉连通区域，并看每个区域底缘是否有暗带
param(
  [string]$Png = "E:\Syncthing\DshWinUI\tmp-cap\blade2-main.png"
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($Png)
$W = $bmp.Width; $H = $bmp.Height
$tR = 0xC6; $tG = 0x9E; $tB = 0x9E

function Is-Pink($px, $py) {
  $p = $bmp.GetPixel($px, $py)
  return ([Math]::Abs($p.R - $tR) -le 4 -and [Math]::Abs($p.G - $tG) -le 4 -and [Math]::Abs($p.B - $tB) -le 4)
}

$labels = New-Object 'int[,]' $W, $H
$next = 1
$regions = @{}
for ($y = 0; $y -lt $H; $y++) {
  for ($x = 0; $x -lt $W; $x++) {
    if (-not (Is-Pink $x $y)) { continue }
    $lbl = 0
    if ($x -gt 0) { $lbl = $labels[($x - 1), $y] }
    if ($lbl -eq 0 -and $y -gt 0) {
      $lo = $x - 1; if ($lo -lt 0) { $lo = 0 }
      $hi = $x + 1; if ($hi -gt $W - 1) { $hi = $W - 1 }
      for ($xx = $lo; $xx -le $hi; $xx++) {
        $v = $labels[$xx, ($y - 1)]
        if ($v -gt 0) { $lbl = $v; break }
      }
    }
    if ($lbl -eq 0) {
      $lbl = $next; $next++
      $regions[$lbl] = [pscustomobject]@{ MinX = $x; MinY = $y; MaxX = $x; MaxY = $y; N = 0 }
    }
    $labels[$x, $y] = $lbl
    $r = $regions[$lbl]
    if ($x -lt $r.MinX) { $r.MinX = $x }; if ($x -gt $r.MaxX) { $r.MaxX = $x }
    if ($y -lt $r.MinY) { $r.MinY = $y }; if ($y -gt $r.MaxY) { $r.MaxY = $y }
    $r.N++
  }
}
Write-Host "regions: $($regions.Count)"
foreach ($k in ($regions.Keys | Sort-Object { $regions[$_].N } -Descending)) {
  $r = $regions[$k]
  if ($r.N -lt 30) { continue }
  Write-Host ("region {0}: ({1},{2}) {3}x{4} px={5}" -f $k, $r.MinX, $r.MinY, ($r.MaxX - $r.MinX + 1), ($r.MaxY - $r.MinY + 1), $r.N)
  $by = $r.MaxY
  for ($k2 = 0; $k2 -ge -4; $k2--) {
    $yy = $by + $k2
    if ($yy -lt 0) { continue }
    $cnt = 0; $sx = -1; $ex = -1
    for ($xx = $r.MinX; $xx -le $r.MaxX; $xx++) {
      $p = $bmp.GetPixel($xx, $yy)
      $lum = 0.299 * $p.R + 0.587 * $p.G + 0.114 * $p.B
      if ($lum -lt 150) { $cnt++; if ($sx -lt 0) { $sx = $xx }; $ex = $xx }
    }
    if ($cnt -gt 0) { Write-Host ("    bottom y={0} dark={1} x={2}..{3}" -f $yy, $cnt, $sx, $ex) }
  }
}
$bmp.Dispose()
