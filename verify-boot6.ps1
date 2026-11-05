# 重启 Blade2 两轮，验证引导里程碑齐全（六段全记录 → 100%）
foreach ($i in 1..2) {
  Get-Process Blade2 -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Sleep -Seconds 2
  $k = Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -match 'bin\.js" web' }
  foreach ($p in $k) { Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue }
  Start-Sleep -Seconds 2
  $log = Join-Path $env:TEMP 'blade2-kernel-boot.log'
  Remove-Item $log -ErrorAction SilentlyContinue
  Start-Process 'shell:AppsFolder\Blade2_wr2brkarxqkxy!App'
  Start-Sleep -Seconds 22
  Write-Host "=== boot #$i ==="
  Get-Content $log | Select-String -Pattern '\[boot\]' | ForEach-Object { $_.Line }
  Write-Host "--- 当前进程:"
  Get-Process Blade2 -ErrorAction SilentlyContinue | Select-Object Id, ProcessName | Format-Table -AutoSize | Out-String | Write-Host
}
