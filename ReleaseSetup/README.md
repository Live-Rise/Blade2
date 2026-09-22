# Blade2-Setup.exe — 下一步式安装向导构建

面向普通用户的单文件安装器：双击 → UAC 授权 → 欢迎页 → 安装说明页 → 确认页 → 安装进度 → 完成页（可立即启动）。MSIX 内嵌在 exe 里，用户只需下载 `Blade2-Setup.exe` 一个文件。每页支持回车（默认项）确认、`B` 上一步、`X` 取消。

## 重新构建（发新版时）

```powershell
# 1. 先跑 build-0800.ps1 产出 MSIX
# 2. 把最新 MSIX 复制进来（会被编译进 exe；该目录不进 git）
Copy-Item ..\AppxPkgs0800\Blade2_<版本>_x64_Test\Blade2_<版本>_x64.msix src\payload\

# 3. 发布单文件（.NET 8 SDK，无需 Visual Studio）
dotnet publish src -c Release
# 产物: src\bin\Release\net8.0-windows\win-x64\publish\Blade2-Setup.exe (~152 MB)
```

## 本地测试（不弹 UAC 的方式）

```powershell
Start-Process src\bin\Release\net8.0-windows\win-x64\publish\Blade2-Setup.exe -Verb RunAs -ArgumentList --no-pause
```

`--no-pause`：无人值守模式（每页按默认项走，装完不等人按键）。本机管理员账户已设「提权不提示」时可直接脚本化；否则 `-Verb RunAs` 仍会弹一次 UAC。

## 换 MSIX 版本时还要改

- `src/Program.cs` 的 `TargetVersion` 常量
- `src/Blade2Setup.csproj` 的 `<Version>`

> 证书会过期（当前到 2027-09-11）。换证书后同步改 `Program.cs` 里的 `PublisherThumbprint` 和 `PublisherCertBase64`（公钥 base64：`Get-ChildItem Cert:\CurrentUser\My | ? Thumbprint -eq <指纹> | % { [Convert]::ToBase64String($_.RawData) }`）。
