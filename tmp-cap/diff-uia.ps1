# UIA 树逐视图 diff：产出 每视图 主干独有/重写线独有 控件清单（功能完整性审查证据）
# 用法：pwsh -File diff-uia.ps1 -MainDir full-main -RustDir full-rust -OutFile uia-diff.txt
param(
    [string]$MainDir = "E:\Syncthing\DshWinUI\tmp-cap\full-main",
    [string]$RustDir = "E:\Syncthing\DshWinUI\tmp-cap\full-rust",
    [string]$OutFile = "E:\Syncthing\DshWinUI\tmp-cap\uia-diff.txt"
)
$report = New-Object System.Collections.Generic.List[string]
# 全部 uia.txt 基名（S0/S1/S2/P-* 自动覆盖）
$views = Get-ChildItem $MainDir -Filter "*.uia.txt" | ForEach-Object { $_.BaseName.Replace(".uia", "") } | Sort-Object -Unique

foreach ($v in $views) {
    $m = Join-Path $MainDir "$v.uia.txt"
    $r = Join-Path $RustDir "$v.uia.txt"
    $report.Add("===== $v =====")
    if (-not (Test-Path $m) -or -not (Test-Path $r)) {
        $report.Add("  SKIP: 主干 $(Test-Path $m) / 重写线 $(Test-Path $r)")
        continue
    }
    $mLines = Get-Content $m | Where-Object { $_.Trim() -ne "" } | ForEach-Object { $_.TrimStart() } | Sort-Object -Unique
    $rLines = Get-Content $r | Where-Object { $_.Trim() -ne "" } | ForEach-Object { $_.TrimStart() } | Sort-Object -Unique
    $onlyMain = $mLines | Where-Object { $rLines -notcontains $_ }
    $onlyRust = $rLines | Where-Object { $mLines -notcontains $_ }
    $report.Add("  主干独有 $(@($onlyMain).Count) 项 / 重写线独有 $(@($onlyRust).Count) 项")
    foreach ($l in $onlyMain) { $report.Add("  [仅主干] $l") }
    foreach ($l in $onlyRust) { $report.Add("  [仅重写] $l") }
}
[System.IO.File]::WriteAllLines($OutFile, $report)
Write-Host "written: $OutFile ($(@($report).Count) lines)"
Get-Content $OutFile | Select-Object -First 5
