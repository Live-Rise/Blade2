# 启动 Blade2 并验证内核引导全链路：进程生命周期 + 内核监听端口 + 壳↔内核回环连接
$ErrorActionPreference = 'SilentlyContinue'
explorer.exe 'shell:AppsFolder\Blade2_wr2brkarxqkxy!App'
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$events = New-Object System.Collections.Generic.List[string]
$state = @{}
$kernelPid = 0
while ($sw.ElapsedMilliseconds -lt 30000) {
  $procs = Get-CimInstance Win32_Process -Filter "Name = 'node.exe' OR Name = 'Blade2.exe'"
  $current = @{}
  foreach ($p in $procs) {
    if ($p.Name -eq 'node.exe' -and $p.ExecutablePath -notlike '*Blade2*') { continue }
    $current["$($p.ProcessId)"] = "$($p.Name)/$($p.ProcessId)"
  }
  foreach ($k in @($state.Keys)) {
    if ($state[$k] -ne 'dead' -and -not $current.ContainsKey($k)) {
      $events.Add("t=$([math]::Round($sw.ElapsedMilliseconds))ms  EXIT  $k")
      $state[$k] = 'dead'
    }
  }
  foreach ($k in $current.Keys) {
    if (-not $state.ContainsKey($k)) {
      $events.Add("t=$([math]::Round($sw.ElapsedMilliseconds))ms  SPAWN $($current[$k])")
      if ($current[$k] -like 'node*') { $kernelPid = [int]($k -as [int]) }
    }
    $state[$k] = 'alive'
  }
  Start-Sleep -Milliseconds 100
}
Write-Host "=== lifecycle ==="
$events | ForEach-Object { Write-Host $_ }
Write-Host "=== kernel ($kernelPid) health ==="
if ($kernelPid -gt 0) {
  $stillAlive = Get-Process -Id $kernelPid -ErrorAction SilentlyContinue
  Write-Host "kernel alive: $($null -ne $stillAlive)"
  $conns = Get-NetTCPConnection -OwningProcess $kernelPid -ErrorAction SilentlyContinue | Where-Object { $_.State -in 'Listen','Established' }
  $conns | ForEach-Object { Write-Host "tcp $($_.LocalAddress):$($_.LocalPort) -> $($_.RemoteAddress):$($_.RemotePort) [$($_.State)]" }
  $listen = $conns | Where-Object State -eq 'Listen' | Select-Object -First 1
  if ($listen) {
    $port = $listen.LocalPort
    try {
      $resp = Invoke-WebRequest -Uri "http://127.0.0.1:$port/" -UseBasicParsing -TimeoutSec 5
      Write-Host "http probe /: $($resp.StatusCode)"
    } catch { Write-Host "http probe /: FAIL $($_.Exception.Message)" }
  }
}
