# 用诊断日志里的 token 自证内核数据面 + 拉高窗口截图
$ErrorActionPreference = 'Stop'
$log = Get-Content "$env:TEMP\blade2-kernel-boot.log"
$urlLine = $log | Where-Object { $_.Contains('[url-captured]') } | Select-Object -Last 1
$url = ($urlLine -split '\[url-captured\] ')[1].Trim()
Write-Host "kernel url: $($url.Substring(0, 40))..."
$base = $url.Split('?')[0]

# 认证（拿 cookie）
Invoke-WebRequest -Uri $url -UseBasicParsing -TimeoutSec 10 -SessionVariable sess | Out-Null
function Probe($method) {
  $body = @{ type = "client-request"; rpcId = "c2-diag"; method = $method; payload = @{ args = @{ _request = @{} } } } | ConvertTo-Json -Depth 6
  $resp = Invoke-WebRequest -Uri "${base}api/$method" -Method Post -Body $body -ContentType "application/json" -WebSession $sess -UseBasicParsing -TimeoutSec 10
  return ($resp.Content | ConvertFrom-Json).result
}
$sessions = Probe "session/list"
$items = $sessions.value.items
Write-Host "session/list ok=$($sessions.ok) count=$(@($items).Count)"
@($items) | Select-Object -First 5 | ForEach-Object { Write-Host "  session: $($_.sessionId.Substring(0, 12))... cwd=$($_.cwd)" }

# 拉高窗口再截图
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type '
using System;
using System.Runtime.InteropServices;
public static class W2 {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr hWnd, int x, int y, int w, int h, bool repaint);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}'
$p = Get-Process Blade2 | Select-Object -First 1
[W2]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
[W2]::MoveWindow($p.MainWindowHandle, 60, 60, 1200, 940, $true) | Out-Null
Start-Sleep -Milliseconds 1200
$rect = New-Object W2+RECT
[W2]::GetWindowRect($p.MainWindowHandle, [ref]$rect) | Out-Null
$bmp = New-Object System.Drawing.Bitmap(($rect.Right - $rect.Left), ($rect.Bottom - $rect.Top))
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bmp.Size)
$bmp.Save("$env:TEMP\blade2-window2.png", [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Host "saved: $env:TEMP\blade2-window2.png"
