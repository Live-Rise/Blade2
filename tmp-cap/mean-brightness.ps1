# 采样截图平均亮度：暗(<80)=应用界面，亮(>150)=被浏览器盖住的误捕
# 用法：pwsh -File mean-brightness.ps1 <目录1> <目录2> ...
param([string[]]$Dirs)
Add-Type -AssemblyName System.Drawing
if (-not $Dirs -or $Dirs.Count -eq 0) { $Dirs = @("mainline4", "rustline4") }
foreach ($sub in $Dirs) {
    $dir = if (Test-Path $sub -PathType Container) { $sub } else { Join-Path "E:\Syncthing\DshWinUI\tmp-cap" $sub }
    foreach ($p in Get-ChildItem $dir -Filter *.png -ErrorAction SilentlyContinue) {
        $bmp = [System.Drawing.Bitmap]::FromFile($p.FullName)
        $sum = 0.0; $n = 0
        for ($x = 0; $x -lt $bmp.Width; $x += 64) {
            for ($y = 0; $y -lt $bmp.Height; $y += 64) {
                $c = $bmp.GetPixel($x, $y)
                $sum += ($c.R + $c.G + $c.B) / 3.0
                $n++
            }
        }
        $mean = $sum / $n
        $bmp.Dispose()
        '{0}\{1}: mean={2:N1} size={3:N0}KB' -f $sub, $p.Name, $mean, ($p.Length / 1KB)
    }
}
