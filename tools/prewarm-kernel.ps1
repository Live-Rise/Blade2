# 内核登录预热：无常驻内核时拉起一颗并写状态文件（壳下次启动直连 0.6s，冷启只剩升级/杀进程场景）
# 已有活内核（状态文件 pid 活着）则跳过。窗口隐藏由任务计划侧（wscript 包装）负责。
$ErrorActionPreference = 'Continue'
$dataHome = Join-Path $env:LOCALAPPDATA 'Blade2'
$stateFile = Join-Path $dataHome 'blade2-resident-kernel.json'
$repo = 'E:\Syncthing\DshWinUI'
$node = Join-Path $repo 'Kernel\node.exe'
$binJs = Join-Path $repo 'Kernel\dsh\lib\bin.js'

# 0) 已有活内核：跳过
if (Test-Path $stateFile) {
    try {
        $st = Get-Content $stateFile -Raw | ConvertFrom-Json
        $alive = Get-Process -Id $st.pid -ErrorAction SilentlyContinue
        if ($alive -and -not $alive.HasExited) { Write-Host "resident kernel alive pid=$($st.pid), skip"; exit 0 }
    } catch { }
}

# 1) 版本读 bundle package.json
$ver = ''
try { $ver = (Get-Content (Join-Path $repo 'Kernel\dsh\package.json') -Raw | ConvertFrom-Json).version } catch { }

# 2) 拉起（隐藏窗口 + 编译缓存 + 内置 pnpm PATH）
Start-Process -FilePath $node -ArgumentList "`"$binJs`" web --no-open --port 0" -WindowStyle Hidden -RedirectStandardOutput "$env:TEMP\prewarm-out.log" -RedirectStandardError "$env:TEMP\prewarm-err.log"
$deadline = (Get-Date).AddSeconds(90)
$url = $null
while ((Get-Date) -lt $deadline -and -not $url) {
    Start-Sleep -Milliseconds 300
    try {
        $line = Select-String -Path "$env:TEMP\prewarm-out.log" -Pattern 'dsh web: (\S+)' -ErrorAction SilentlyContinue | Select-Object -Last 1
        if ($line) { $url = $line.Matches[0].Groups[1].Value }
    } catch { }
}
if (-not $url) { Write-Host 'prewarm FAIL: no url in 90s'; exit 1 }

# 3) 写状态文件（五键与壳两侧互通；exe 记 repo node——壳 attach 按记录 exe 核对，跨安装可复用）
$proc = Get-CimInstance Win32_Process -Filter "Name='node.exe'" | Where-Object { $_.CommandLine -like "*$binJs*" } | Select-Object -First 1
$state = [ordered]@{ Home = $dataHome; pid = [int]$proc.ProcessId; url = $url; exe = $node; ver = $ver }
$tmp = "$stateFile.tmp"
$state | ConvertTo-Json -Compress | Set-Content $tmp -Encoding UTF8
Move-Item $tmp $stateFile -Force
Write-Host "prewarm OK pid=$($proc.ProcessId) url=$url"
