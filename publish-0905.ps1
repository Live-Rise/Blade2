# 创建 GitHub Release v0.9.0.5 并上传资产（token 从 git credential manager 取，不落屏）
$ErrorActionPreference = 'Stop'

$credInput = "protocol=https`nhost=github.com`n"
$cred = $credInput | git credential fill 2>$null
$tokenLine = $cred | Where-Object { $_ -like 'password=*' } | Select-Object -First 1
if (-not $tokenLine) { throw 'no github credential found in store' }
$token = $tokenLine.Substring('password='.Length)
$headers = @{ Authorization = "token $token"; 'User-Agent' = 'blade2-release' ; Accept = 'application/vnd.github+json' }

$bodyFile = Join-Path $PSScriptRoot 'tmp-cap\rel-v0905.md'
$bodyText = Get-Content $bodyFile -Raw -Encoding UTF8
$payload = @{
  tag_name         = 'v0.9.0.5'
  target_commitish = 'main'
  name             = 'Blade2 0.9.0.5'
  body             = $bodyText
  draft            = $false
  prerelease       = $false
} | ConvertTo-Json
$rel = Invoke-RestMethod -Method Post -Uri 'https://api.github.com/repos/Live-Rise/Blade2/releases' -Headers $headers -Body $payload -ContentType 'application/json'
Write-Host "release created: id=$($rel.id) tag=$($rel.tag_name) html=$($rel.html_url)"
$upload = $rel.upload_url -replace '\{\?.*$', ''

function Upload($path, $name, $ctype) {
  $uri = "$upload`?name=$name"
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  $resp = Invoke-RestMethod -Method Post -Uri $uri -Headers $headers -InFile $path -ContentType $ctype -TimeoutSec 3600
  $sw.Stop()
  Write-Host "uploaded $name ($([math]::Round((Get-Item $path).Length/1MB,2)) MB in $([math]::Round($sw.Elapsed.TotalSeconds,1))s) state=$($resp.state)"
}
Upload (Join-Path $PSScriptRoot 'Blade2.cer') 'Blade2.cer' 'application/octet-stream'
Upload (Join-Path $PSScriptRoot 'AppxPkgs0800\Blade2_0.9.0.5_x64_Test\Blade2_0.9.0.5_x64.msix') 'Blade2_0.9.0.5_x64.msix' 'application/octet-stream'
Write-Host 'DONE'
