$R = 48.0
function T($acx, $acy, $d0, $d1) {
  for ($i = 0; $i -le 4; $i++) {
    $th = ($d0 + ($d1 - $d0) * $i / 4) * [Math]::PI / 180.0
    Write-Host ('th type=' + $th.GetType().FullName + ' val=' + $th)
    $c = [Math]::Cos($th)
    Write-Host ('cos type=' + $c.GetType().FullName)
    $m = $R * $c
    Write-Host ('mul ok: ' + $m)
    $s = $acx + $m
    Write-Host ('sum ok: ' + $s)
  }
}
T 48 48 270 180
