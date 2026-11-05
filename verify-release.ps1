# 核验远端 release 列表与 v0.8.3.9 资产
$ErrorActionPreference = 'Stop'
$cred = "protocol=https`nhost=github.com`n" | git credential fill 2>$null
$tokenLine = $cred | Where-Object { $_ -like 'password=*' } | Select-Object -First 1
if (-not $tokenLine) { throw 'no github credential found in store' }
$token = $tokenLine.Substring('password='.Length)
$headers = @{ Authorization = "token $token"; 'User-Agent' = 'blade2-release'; Accept = 'application/vnd.github+json' }
$rels = Invoke-RestMethod -Uri 'https://api.github.com/repos/Live-Rise/Blade2/releases?per_page=5' -Headers $headers
foreach ($r in $rels) {
  $assets = ($r.assets | ForEach-Object { "$($_.name)/$([math]::Round($_.size/1MB,1))MB" }) -join ', '
  Write-Host "$($r.tag_name) | $($r.name) | draft=$($r.draft) | $assets"
}
$latest = Invoke-RestMethod -Uri 'https://api.github.com/repos/Live-Rise/Blade2/releases/latest' -Headers $headers
Write-Host "latest = $($latest.tag_name) ($($latest.name))"
