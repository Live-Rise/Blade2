$R = 48.0
$pts = New-Object System.Collections.Generic.List[double[]]
function AddArc($acx, $acy, $d0, $d1) {
  for ($i = 0; $i -le 4; $i++) {
    $th = ($d0 + ($d1 - $d0) * $i / 4) * [Math]::PI / 180.0
    Write-Host ('R in func: ' + $R.GetType().FullName + ' acx=' + $acx.GetType().FullName)
    $pts.Add([double[]]@($acx + $R * [Math]::Cos($th), $acy + $R * [Math]::Sin($th)))
  }
}
AddArc 48 48 270 180
Write-Host ('count=' + $pts.Count)
