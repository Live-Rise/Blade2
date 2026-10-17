# 创建 GitHub Release v0.8.3.9 并上传资产（token 从 git credential manager 取，不落屏）
$ErrorActionPreference = 'Stop'

# 1) 取凭据（不打印）
$credInput = "protocol=https`nhost=github.com`n"
$cred = $credInput | git credential fill 2>$null
$tokenLine = $cred | Where-Object { $_ -like 'password=*' } | Select-Object -First 1
if (-not $tokenLine) { throw 'no github credential found in store' }
$token = $tokenLine.Substring('password='.Length)
$headers = @{ Authorization = "token $token"; 'User-Agent' = 'blade2-release' ; Accept = 'application/vnd.github+json' }

# 2) 建 release（tag v0.8.3.9 打在 main HEAD）
$bodyFile = "$env:TEMP\rel-v0839.md"
$bodyText = Get-Content $bodyFile -Raw
$payload = @{
  tag_name         = 'v0.8.3.9'
  target_commitish = 'main'
  name             = 'Blade² 0.8.3.9'
  body             = $bodyText
  draft            = $false
  prerelease       = $false
} | ConvertTo-Json
$rel = Invoke-RestMethod -Method Post -Uri 'https://api.github.com/repos/Live-Rise/Blade2/releases' -Headers $headers -Body $payload -ContentType 'application/json'
Write-Host "release created: id=$($rel.id) tag=$($rel.tag_name) html=$($rel.html_url)"
$upload = $rel.upload_url -replace '\{\?.*$', ''

# 3) 上传资产（msix 255MB + cer）
function Upload($path, $name, $ctype) {
  $uri = "$upload`?name=$name"
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  $resp = Invoke-RestMethod -Method Post -Uri $uri -Headers $headers -InFile $path -ContentType $ctype -TimeoutSec 3600
  $sw.Stop()
  Write-Host "uploaded $name ($([math]::Round((Get-Item $path).Length/1MB,2)) MB in $([math]::Round($sw.Elapsed.TotalSeconds,1))s) state=$($resp.state)"
}
Upload 'Blade2.cer' 'Blade2.cer' 'application/octet-stream'
Upload 'AppxPkgs0800\Blade2_0.8.3.9_x64_Test\Blade2_0.8.3.9_x64.msix' 'Blade2_0.8.3.9_x64.msix' 'application/octet-stream'
Write-Host 'DONE'

