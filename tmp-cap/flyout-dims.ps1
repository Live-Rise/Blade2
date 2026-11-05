Add-Type -AssemblyName System.Drawing
foreach ($f in @('flyout2-main-agentmode','flyout2-main-model','flyout2-rust-agentmode','flyout2-rust-model')) {
    $i = [System.Drawing.Image]::FromFile("E:\Syncthing\DshWinUI\tmp-cap\$f.png")
    Write-Host "$f : $($i.Width)x$($i.Height)"
    $i.Dispose()
}
Get-Process Blade2, blade2-rs -ErrorAction SilentlyContinue | Select-Object Name, Id
