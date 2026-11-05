$R = 48.0
$th = 1.0
$a = 48 + $R * [Math]::Cos($th)
Write-Host ('a=' + $a + ' type=' + $a.GetType().FullName)
Write-Host ('R type=' + $R.GetType().FullName)
