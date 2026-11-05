# 发布 Blade2 v0.8.3.9：删掉被取代的 v0.8.3.8，建新 release 并上传 cer + msix（断点重试）
$ErrorActionPreference = 'Stop'

$Repo     = 'Live-Rise/Blade2'
$Tag      = 'v0.8.3.9'
$OldTag   = 'v0.8.3.8'
$MsixPath = Join-Path $PSScriptRoot 'AppxPkgs0800\Blade2_0.8.3.9_x64_Test\Blade2_0.8.3.9_x64.msix'
$CerPath  = Join-Path $PSScriptRoot 'Blade2.cer'
$BodyFile = "$env:TEMP\rel-v0839.md"

# 1) 凭据（不打印）
$cred = "protocol=https`nhost=github.com`n" | git credential fill 2>$null
$tokenLine = $cred | Where-Object { $_ -like 'password=*' } | Select-Object -First 1
if (-not $tokenLine) { throw 'no github credential found in store' }
$token = $tokenLine.Substring('password='.Length)
$headers = @{ Authorization = "token $token"; 'User-Agent' = 'blade2-release'; Accept = 'application/vnd.github+json' }

function Gh($method, $uri, $body) {
  $p = @{ Method = $method; Uri = $uri; Headers = $headers }
  if ($body) { $p['Body'] = $body; $p['ContentType'] = 'application/json' }
  Invoke-RestMethod @p
}

# 2) 删旧发布（被 0.8.3.9 取代：内含卡 83% 与家族包版本错配两个故障）
try {
  $old = Gh 'GET' "https://api.github.com/repos/$Repo/releases/tags/$OldTag" $null
  Gh 'DELETE' "https://api.github.com/repos/$Repo/releases/$($old.id)" $null | Out-Null
  Write-Host "deleted release $OldTag (id=$($old.id))"
} catch {
  Write-Host "delete release $OldTag skipped: $($_.Exception.Message)"
}
try {
  Gh 'DELETE' "https://api.github.com/repos/$Repo/git/refs/tags/$OldTag" $null | Out-Null
  Write-Host "deleted tag $OldTag"
} catch {
  Write-Host "delete tag $OldTag skipped: $($_.Exception.Message)"
}

# 3) 建 release（tag 打在 main HEAD）
$existing = $null
try { $existing = Gh 'GET' "https://api.github.com/repos/$Repo/releases/tags/$Tag" $null } catch { }
if ($existing) {
  Write-Host "release $Tag already exists (id=$($existing.id))，复用"
} else {
  $payload = @{
    tag_name         = $Tag
    target_commitish = 'main'
    name             = 'Blade2 0.8.3.9'
    body             = Get-Content $BodyFile -Raw
    draft            = $false
    prerelease       = $false
  } | ConvertTo-Json
  $existing = Gh 'POST' "https://api.github.com/repos/$Repo/releases" $payload
  Write-Host "release created: id=$($existing.id) tag=$($existing.tag_name) html=$($existing.html_url)"
}
$upload = $existing.upload_url -replace '\{\?.*$', ''

# 4) 上传资产（已有则跳过；失败重试三次——255MB 长传中途被掐过一次）
function Upload($path, $name, $ctype) {
  if (-not (Test-Path -LiteralPath $path)) { Write-Host "[FAIL] missing $path" -ForegroundColor Red; return }
  $have = $existing.assets | Where-Object { $_.name -eq $name }
  if ($have) { Write-Host "asset $name already uploaded (state=$($have.state))，跳过"; return }
  $uri = "$upload`?name=$name"
  foreach ($try in 1..3) {
    try {
      $sw = [System.Diagnostics.Stopwatch]::StartNew()
      $resp = Invoke-RestMethod -Method Post -Uri $uri -Headers $headers -InFile $path -ContentType $ctype -TimeoutSec 3600
      $sw.Stop()
      Write-Host "uploaded $name ($([math]::Round((Get-Item $path).Length/1MB,2)) MB in $([math]::Round($sw.Elapsed.TotalSeconds,1))s) state=$($resp.state)"
      return
    } catch {
      Write-Host "upload $name attempt $try failed: $($_.Exception.Message)" -ForegroundColor Yellow
      Start-Sleep -Seconds 5
    }
  }
  Write-Host "[FAIL] $name not uploaded" -ForegroundColor Red
}

Upload $CerPath  'Blade2.cer'                 'application/octet-stream'
Upload $MsixPath 'Blade2_0.8.3.9_x64.msix'    'application/octet-stream'
Write-Host 'DONE'
