# 决定性测试：.NET HttpClient 默认代理路径 vs 无代理路径 连接内核回环端口
$ErrorActionPreference = 'SilentlyContinue'
$env:DSH_HOME = "$env:TEMP\dsh-real-copy"
$env:NODE_COMPILE_CACHE = "$env:TEMP\dsh-real-copy\kernel-compile-cache"
$out = "$env:TEMP\proxy-test.out"
Remove-Item $out -ErrorAction SilentlyContinue
$kp = Start-Process -FilePath "E:\Syncthing\DshWinUI\Kernel\node.exe" -ArgumentList "`"E:\Syncthing\DshWinUI\Kernel\dsh\lib\bin.js`"","web","--no-open","--port","0" -RedirectStandardOutput $out -RedirectStandardError "$out.err" -PassThru -NoNewWindow
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$url = $null
while ($sw.ElapsedMilliseconds -lt 45000) {
  if ($kp.HasExited) { break }
  if (Test-Path $out) { $line = (Select-String -Path $out -Pattern "dsh web: (\S+)" | Select-Object -First 1); if ($line) { $url = $line.Matches[0].Groups[1].Value; break } }
  Start-Sleep -Milliseconds 25
}
Write-Host "kernel t=$($sw.ElapsedMilliseconds)ms pid=$($kp.Id)"
if (-not $url) { Write-Host "no url"; exit }
$port = ([Uri]($url.Split("?")[0])).Port
Write-Host "port: $port"
$proxyAlive = Test-NetConnection -ComputerName 127.0.0.1 -Port 7897 -WarningAction SilentlyContinue -InformationLevel Quiet
Write-Host "system proxy 127.0.0.1:7897 alive: $proxyAlive"

Add-Type -TypeDefinition @'
using System;
using System.Net.Http;
using System.Threading.Tasks;
public static class Probe {
  public static async Task<string> Run(bool useProxy) {
    try {
      var handler = new HttpClientHandler { UseProxy = useProxy, UseCookies = false };
      using var client = new HttpClient(handler) { Timeout = TimeSpan.FromSeconds(5) };
      var resp = await client.GetAsync("http://127.0.0.1:" + Environment.GetEnvironmentVariable("PROBE_PORT") + "/");
      return $"HTTP {(int)resp.StatusCode}";
    } catch (Exception ex) {
      var inner = ex.InnerException?.Message ?? "";
      return $"FAIL: {ex.Message} | inner: {inner}";
    }
  }
}
'@
$env:PROBE_PORT = "$port"
Write-Host "default handler (system proxy): $([Probe]::Run($true).GetAwaiter().GetResult())"
Write-Host "no-proxy handler:               $([Probe]::Run($false).GetAwaiter().GetResult())"
taskkill /PID $kp.Id /T /F 2>&1 | Out-Null
