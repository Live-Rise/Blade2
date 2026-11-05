$R = 48.0
$acx = 48; $th = 1.0
$pts = New-Object System.Collections.Generic.List[double[]]
$pts.Add([double[]]@($acx + $R * [Math]::Cos($th), 48 + $R * [Math]::Sin($th)))
Write-Host ('count=' + $pts.Count + ' x=' + $pts[0][0] + ' y=' + $pts[0][1])
