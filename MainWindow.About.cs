using System;
using System.Diagnostics;
using System.IO;
using System.Net.Http;
using System.Reflection;
using System.Text.Json;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Blade2;

/// <summary>
/// 设置「关于」分区：显示当前版本号 + GitHub Release 更新。
/// 壳内建分区：不进内核 settings/mutate、不参与「恢复本页默认」（同 usage/memory）。
/// 更新路线 = 覆盖安装新 MSIX（项目既定路线：无自动更新服务，见 DESIGN.zh.md）。
/// GitHub 仓库就绪前保持 <see cref="UpdateRepo"/> 为空：检查更新入口照常渲染，
/// 点击时给出配置提示，仓库填好后零改动启用。
/// </summary>
public partial class MainWindow
{
    private const string AboutSectionId = "about";

    /// <summary>GitHub 更新仓库（owner/repo）。空 = 未配置，检查更新时提示配置。</summary>
    private const string UpdateRepo = "Live-Rise/Blade2";

    private static string UpdateReleasesApiUrl => string.IsNullOrEmpty(UpdateRepo)
        ? ""
        : $"https://api.github.com/repos/{UpdateRepo}/releases/latest";

    private static string UpdateReleasesPageUrl => string.IsNullOrEmpty(UpdateRepo)
        ? ""
        : $"https://github.com/{UpdateRepo}/releases/latest";

    private TextBlock? _aboutUpdateStatus;
    private Button? _aboutCheckButton;
    private Button? _aboutDownloadButton;
    private string? _aboutDownloadUrl;
    private bool _aboutDownloadIsDirect;
    private bool _aboutBusy;

    /// <summary>壳版本号：打包形态取包版本（manifest Identity，发布口径），开发形态回落程序集版本。</summary>
    private static string ShellVersionText()
    {
        try
        {
            var pv = Windows.ApplicationModel.Package.Current.Id.Version;
            return $"{pv.Major}.{pv.Minor}.{pv.Build}.{pv.Revision}";
        }
        catch (Exception) { } // 无包身份（开发形态 run/dev）取不到包版本
        try
        {
            return Assembly.GetExecutingAssembly().GetName().Version?.ToString() ?? L_static("未知");
        }
        catch (Exception)
        {
            return L_static("未知");
        }
    }

    private static string L_static(string key) => ShellTranslateFunc(key);

    /// <summary>随包发行的 dsh 内核版本（Kernel/dsh/package.json）。</summary>
    private static string KernelVersionText()
    {
        try
        {
            var pkg = Path.Combine(AppContext.BaseDirectory, "Kernel", "dsh", "package.json");
            using var doc = JsonDocument.Parse(File.ReadAllText(pkg));
            if (doc.RootElement.TryGetProperty("version", out var v) &&
                v.ValueKind == JsonValueKind.String &&
                !string.IsNullOrEmpty(v.GetString()))
            {
                return v.GetString()!;
            }
        }
        catch (Exception) { } // 读不到即未知，不影响关于页其余内容
        return L_static("未知");
    }

    /// <summary>关于区渲染入口：版本卡 + 更新卡。</summary>
    private void RenderAboutSection()
    {
        _aboutUpdateStatus = null;
        _aboutCheckButton = null;
        _aboutDownloadButton = null;
        _aboutDownloadUrl = null;

        SettingsHost.Children.Add(MakeSectionDesc(
            L("Blade² 的版本信息与更新。更新通过覆盖安装新版本 MSIX 完成（安装前需先退出应用）。")));

        var verCard = NewCard(L("版本"), null);
        verCard.Children.Add(MakeRow(
            L("当前版本"),
            L("壳版本（打包形态与安装包版本一致）"),
            SelectableVersionText(ShellVersionText())));
        AddDivider(verCard);
        verCard.Children.Add(MakeRow(
            L("内核版本"),
            L("随包发行的 dsh 内核版本"),
            SelectableVersionText(KernelVersionText())));

        var updCard = NewCard(L("更新"), null);
        _aboutUpdateStatus = new TextBlock
        {
            Text = string.IsNullOrEmpty(UpdateRepo)
                ? L("更新源尚未配置：发布到 GitHub 后填入仓库地址即可启用在线检查更新。")
                : LF("更新源：{0}", $"github.com/{UpdateRepo}"),
            Style = AppStyle("CardDescriptionTextStyle"),
            TextWrapping = TextWrapping.Wrap,
        };
        updCard.Children.Add(_aboutUpdateStatus);

        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = TokenDouble("Space8", 8),
        };
        _aboutCheckButton = Aut(new Button
        {
            Content = L("检查更新"),
            Style = AppStyle("CompactButtonStyle"),
        }, "AboutCheckUpdateButton", L("检查更新"));
        _aboutCheckButton.Click += (_, _) => _ = CheckUpdateAsync();
        row.Children.Add(_aboutCheckButton);
        _aboutDownloadButton = Aut(new Button
        {
            Content = L("下载并安装"),
            Style = AppStyle("AccentButtonStyle"),
            Visibility = Visibility.Collapsed,
        }, "AboutDownloadUpdateButton", L("下载并安装"));
        _aboutDownloadButton.Click += (_, _) => _ = DownloadAndInstallUpdateAsync();
        row.Children.Add(_aboutDownloadButton);
        updCard.Children.Add(MakeRow(L("GitHub Release"), string.IsNullOrEmpty(UpdateRepo) ? null : UpdateReleasesPageUrl, row));
        // 启动静默检查已命中的新版本直接铺到卡上：点通知跳来的用户不必再点「检查更新」。
        ApplySilentUpdateToAboutCard();
    }

    private TextBlock SelectableVersionText(string version) => new()
    {
        Text = version,
        Style = AppStyle("CodeTextStyle"),
        IsTextSelectionEnabled = true,
        VerticalAlignment = VerticalAlignment.Center,
    };

    /// <summary>检查更新：拉 releases/latest，比对 tag 与当前壳版本。新版出现才露出下载按钮。</summary>
    private async Task CheckUpdateAsync()
    {
        if (_aboutBusy)
        {
            return;
        }
        if (string.IsNullOrEmpty(UpdateRepo))
        {
            SetAboutStatus(L("更新源尚未配置：发布到 GitHub 后填入仓库地址即可启用在线检查更新。"));
            return;
        }
        _aboutBusy = true;
        if (_aboutCheckButton is not null)
        {
            _aboutCheckButton.IsEnabled = false;
        }
        if (_aboutDownloadButton is not null)
        {
            _aboutDownloadButton.Visibility = Visibility.Collapsed;
        }
        _aboutDownloadUrl = null;
        SetAboutStatus(L("正在检查更新…"));
        try
        {
            using var http = new HttpClient();
            http.DefaultRequestHeaders.UserAgent.ParseAdd("Blade2-WinUI");
            http.DefaultRequestHeaders.Accept.ParseAdd("application/vnd.github+json");
            using var resp = await http.GetAsync(UpdateReleasesApiUrl);
            resp.EnsureSuccessStatusCode();
            using var doc = JsonDocument.Parse(await resp.Content.ReadAsStringAsync());
            var root = doc.RootElement;
            var tag = root.TryGetProperty("tag_name", out var tagEl) &&
                tagEl.ValueKind == JsonValueKind.String
                ? tagEl.GetString() ?? ""
                : "";
            var (assetName, assetUrl, assetSize) = PickMsixAsset(root);
            if (string.IsNullOrEmpty(tag))
            {
                SetAboutStatus(L("检查更新失败：Release 返回缺少 tag_name。"));
                return;
            }
            if (TryParseVersion(tag) is { } latest &&
                CurrentShellVersion() is { } current &&
                latest > current)
            {
                _aboutDownloadUrl = assetUrl;
                // 直链 .msix → 下载并安装；只有 Release 页面 → 打开浏览器手动下载。
                _aboutDownloadIsDirect = assetUrl.EndsWith(".msix", StringComparison.OrdinalIgnoreCase);
                SetAboutStatus(assetSize > 0 && _aboutDownloadIsDirect
                    ? LF("发现新版本：{0}（约 {1}）。", tag, FormatBytes(assetSize))
                    : LF("发现新版本：{0}。", tag));
                if (_aboutDownloadButton is not null && !string.IsNullOrEmpty(assetUrl))
                {
                    _aboutDownloadButton.Content = _aboutDownloadIsDirect
                        ? LF("下载并安装 {0}", assetName)
                        : L("打开 Release 页面");
                    _aboutDownloadButton.Visibility = Visibility.Visible;
                }
            }
            else if (!string.IsNullOrEmpty(tag))
            {
                SetAboutStatus(L("已是最新版本。"));
            }
            else
            {
                SetAboutStatus(L("检查更新失败：未找到可用的安装包。"));
            }
        }
        catch (Exception ex)
        {
            SetAboutStatus(LF("检查更新失败：{0}", ex.Message));
        }
        finally
        {
            _aboutBusy = false;
            if (_aboutCheckButton is not null)
            {
                _aboutCheckButton.IsEnabled = true;
            }
        }
    }

    /// <summary>下载 .msix 到 %TEMP%\Blade2Update，确认后起分离式 powershell 等待本进程退出再 Add-AppxPackage。</summary>
    private async Task DownloadAndInstallUpdateAsync()
    {
        if (_aboutBusy || string.IsNullOrEmpty(_aboutDownloadUrl))
        {
            return;
        }
        _aboutBusy = true;
        if (_aboutDownloadButton is not null)
        {
            _aboutDownloadButton.IsEnabled = false;
        }
        try
        {
            // 非直链（Release 页面）：直接用浏览器打开，用户手动下载安装。
            if (!_aboutDownloadIsDirect)
            {
                Process.Start(new ProcessStartInfo
                {
                    FileName = _aboutDownloadUrl,
                    UseShellExecute = true,
                });
                return;
            }
            var fileName = _aboutDownloadUrl.Split('?')[0].Split('/')[^1];
            if (string.IsNullOrEmpty(fileName))
            {
                fileName = "Blade2Update.msix";
            }
            var dir = Path.Combine(Path.GetTempPath(), "Blade2Update");
            Directory.CreateDirectory(dir);
            var dest = Path.Combine(dir, fileName);
            SetAboutStatus(LF("正在下载更新包：{0}…", fileName));
            using (var http = new HttpClient())
            {
                http.DefaultRequestHeaders.UserAgent.ParseAdd("Blade2-WinUI");
                using var resp = await http.GetAsync(_aboutDownloadUrl, HttpCompletionOption.ResponseHeadersRead);
                resp.EnsureSuccessStatusCode();
                await using var net = await resp.Content.ReadAsStreamAsync();
                await using var file = new FileStream(dest, FileMode.Create, FileAccess.Write, FileShare.None);
                await net.CopyToAsync(file);
            }
            SetAboutStatus(LF("已下载：{0}", dest));
            var dialog = new ContentDialog
            {
                Title = L("安装更新"),
                Content = new TextBlock
                {
                    Text = L("将退出 Blade²（含内置内核）并覆盖安装新版本。继续吗？"),
                    TextWrapping = TextWrapping.Wrap,
                },
                PrimaryButtonText = L("退出并安装"),
                CloseButtonText = L("稍后"),
                DefaultButton = ContentDialogButton.Close,
                XamlRoot = Content.XamlRoot,
            };
            if (await dialog.ShowAsync() != ContentDialogResult.Primary)
            {
                return;
            }
            // 分离式安装：本进程退出后 Add-AppxPackage（ running 包无法覆盖安装，
            // 与 install-*.ps1 的先杀进程再安装同理；MSI 无窗口，静默等待）。
            var self = Environment.ProcessId;
            var ps = $"while (Get-Process -Id {self} -ErrorAction SilentlyContinue) " +
                "{ Start-Sleep -Milliseconds 500 }; " +
                $"Add-AppxPackage -Path '{dest.Replace("'", "''")}'";
            Process.Start(new ProcessStartInfo
            {
                FileName = "powershell.exe",
                Arguments = $"-NoProfile -ExecutionPolicy Bypass -Command \"{ps}\"",
                UseShellExecute = false,
                CreateNoWindow = true,
            });
            _allowClose = true; // 放行关闭拦截（用户开了「关闭时最小化到托盘」时 Exit 也会被拦）
            Application.Current.Exit();
        }
        catch (Exception ex)
        {
            SetAboutStatus(LF("下载更新失败：{0}", ex.Message));
        }
        finally
        {
            _aboutBusy = false;
            if (_aboutDownloadButton is not null)
            {
                _aboutDownloadButton.IsEnabled = true;
            }
        }
    }

    private void SetAboutStatus(string text)
    {
        if (_aboutUpdateStatus is not null)
        {
            _aboutUpdateStatus.Text = text;
        }
    }

    /// <summary>Release assets 里挑 .msix（GitHub 发布产物）；没有则回落第一个带下载地址的 asset。</summary>
    private static (string Name, string Url, long Size) PickMsixAsset(JsonElement release)
    {
        string fallbackName = "", fallbackUrl = "";
        long fallbackSize = 0;
        if (release.TryGetProperty("assets", out var assets) &&
            assets.ValueKind == JsonValueKind.Array)
        {
            foreach (var a in assets.EnumerateArray())
            {
                var name = a.TryGetProperty("name", out var n) && n.ValueKind == JsonValueKind.String
                    ? n.GetString() ?? "" : "";
                var url = a.TryGetProperty("browser_download_url", out var u) && u.ValueKind == JsonValueKind.String
                    ? u.GetString() ?? "" : "";
                var size = a.TryGetProperty("size", out var s) && s.ValueKind == JsonValueKind.Number &&
                    s.TryGetInt64(out var bytes) ? bytes : 0;
                if (string.IsNullOrEmpty(url))
                {
                    continue;
                }
                if (name.EndsWith(".msix", StringComparison.OrdinalIgnoreCase))
                {
                    return (name, url, size);
                }
                if (string.IsNullOrEmpty(fallbackUrl))
                {
                    fallbackName = name;
                    fallbackUrl = url;
                    fallbackSize = size;
                }
            }
        }
        // 无 asset 时回落 Release 页面：至少让用户能手动下载。
        if (string.IsNullOrEmpty(fallbackUrl) &&
            release.TryGetProperty("html_url", out var html) &&
            html.ValueKind == JsonValueKind.String)
        {
            fallbackUrl = html.GetString() ?? "";
        }
        return (fallbackName, fallbackUrl, fallbackSize);
    }

    private static Version? CurrentShellVersion()
    {
        try
        {
            var pv = Windows.ApplicationModel.Package.Current.Id.Version;
            return new Version(pv.Major, pv.Minor, pv.Build, pv.Revision);
        }
        catch (Exception) { }
        try
        {
            return Assembly.GetExecutingAssembly().GetName().Version;
        }
        catch (Exception)
        {
            return null;
        }
    }

    /// <summary>Release tag（v0.7.9.0 / 0.7.9.0）→ Version。带着非数字后缀的 tag 视为不可比。</summary>
    private static Version? TryParseVersion(string tag)
    {
        var t = tag.Trim().TrimStart('v', 'V');
        return Version.TryParse(t, out var v) ? v : null;
    }

    private static string FormatBytes(long bytes) => bytes switch
    {
        < 1024 => $"{bytes} B",
        < 1024 * 1024 => $"{bytes / 1024.0:F1} KB",
        _ => $"{bytes / (1024.0 * 1024):F1} MB",
    };
}
