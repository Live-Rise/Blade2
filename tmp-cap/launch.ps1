$pkg = Get-AppxPackage -Name Blade2 | Sort-Object Version -Descending | Select-Object -First 1
$appId = ($pkg | Get-AppxPackageManifest).Package.Applications.Application.Id
Write-Host "launching $($pkg.PackageFamilyName)!$appId (v$($pkg.Version))"
Start-Process "explorer.exe" -ArgumentList "shell:AppsFolder\$($pkg.PackageFamilyName)!$appId"
Start-Sleep -Seconds 15
