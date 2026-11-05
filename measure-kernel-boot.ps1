# 内核启动计时：spawn → "dsh web:" 打印的耗时（隔离 DSH_HOME，不碰真实数据）
param(
    [string]$DshHome,
    [int]$Runs = 2
)
$repo = "E:\Syncthing\DshWinUI"
$node = Join-Path $repo "Kernel\node.exe"
$bin = Join-Path $repo "Kernel\dsh\lib\bin.js"
$env:DSH_HOME = $DshHome
$env:NODE_COMPILE_CACHE = Join-Path $DshHome "kernel-compile-cache"
for ($i = 1; $i -le $Runs; $i++) {
    $bootLog = Join-Path $env:TEMP "dsh-boot-$i.log"
    $bootErr = Join-Path $env:TEMP "dsh-boot-$i.err.log"
    Remove-Item $bootLog, $bootErr -ErrorAction SilentlyContinue
    $p = Start-Process -FilePath $node -ArgumentList "`"$bin`"","web","--no-open","--port","0" `
        -RedirectStandardOutput $bootLog -RedirectStandardError $bootErr -PassThru -NoNewWindow
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $found = $false
    while ($sw.ElapsedMilliseconds -lt 120000) {
        if ($p.HasExited) { break }
        if ((Test-Path $bootLog) -and (Select-String -Path $bootLog -Pattern "dsh web:" -Quiet)) { $found = $true; break }
        Start-Sleep -Milliseconds 20
    }
    $ms = $sw.ElapsedMilliseconds
    $urlLine = if ($found) { (Select-String -Path $bootLog -Pattern "dsh web:.*").Line } else { "(timeout/exited code=$($p.ExitCode))" }
    Write-Host "run $i : ${ms} ms  $urlLine"
    if (-not $p.HasExited) {
        taskkill /PID $p.Id /T /F 2>&1 | Out-Null
    }
    Start-Sleep -Milliseconds 500
}
