using System;
using System.Net.Http;
using System.Text.Json;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;

namespace Blade2;

/// <summary>
/// 启动静默检查更新：壳亮相后延迟查一次 GitHub Release，发现新版本且该版本还没提醒过，
/// 弹一次系统通知；点击通知跳到「关于」分区让用户手动更新。更新路线与关于页完全一致
/// （不自升级，见 MainWindow.About.cs 的设计注释），这里只补「自动发现 + 主动告知」。
/// </summary>
public partial class MainWindow
{
    /// <summary>启动后延迟多久检查：让内核引导与首帧先走，不与之抢 IO。</summary>
    private const int UpdateCheckDelayMs = 6000;

    /// <summary>静默检查命中的新版本：关于页直接采用（点通知来的用户不必再点「检查更新」）。</summary>
    private (string Tag, string AssetName, string AssetUrl, long AssetSize)? _silentUpdate;

    /// <summary>冷启动 toast 带的跳更新页请求：等内核就绪后再执行（ShowSettingsAsync 依赖内核）。</summary>
    private bool _pendingUpdateDeepLink;

    /// <summary>构造函数尾部调起（fire-and-forget）：延迟一次性静默检查。</summary>
    private void ScheduleUpdateCheck()
    {
        _ = CheckUpdateSilentlyAsync();
    }

    private async Task CheckUpdateSilentlyAsync()
    {
        try
        {
            await Task.Delay(UpdateCheckDelayMs);
            if (string.IsNullOrEmpty(UpdateRepo) || CurrentShellVersion() is not { } current)
            {
                return; // 未配置更新源 / 取不到版本可比
            }
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
            if (string.IsNullOrEmpty(tag) ||
                TryParseVersion(tag) is not { } latest ||
                latest <= current)
            {
                return; // 没新版（含 tag 不可比）
            }
            var (assetName, assetUrl, assetSize) = PickMsixAsset(root);
            _silentUpdate = (tag, assetName, assetUrl, assetSize);

            // 每个新版本只提醒一次：tag 落 shell.json，重启后仍生效（用户点过通知也算提醒过）。
            if (_shellOptions.RemindedUpdateTag == tag)
            {
                return;
            }
            SetRemindedUpdateTag(tag);
            ShellToast.Show(LF("发现新版本 {0}", tag), L("点击查看并更新"), "update");
        }
        catch (Exception)
        {
            // 静默检查：断网 / 限流 / 无包身份都不该打扰用户，留一条诊断即可。
            System.Diagnostics.Debug.WriteLine("[update-check] silent check skipped");
        }
    }

    /// <summary>把启动静默检查的命中直接铺到关于页更新卡（含下载按钮，链路与手动检查一致）。</summary>
    private void ApplySilentUpdateToAboutCard()
    {
        if (_silentUpdate is not { } upd || string.IsNullOrEmpty(upd.AssetUrl))
        {
            return;
        }
        _aboutDownloadUrl = upd.AssetUrl;
        // 直链 .msix → 下载并安装；只有 Release 页面 → 打开浏览器手动下载（同 CheckUpdateAsync 判定）。
        _aboutDownloadIsDirect = upd.AssetUrl.EndsWith(".msix", StringComparison.OrdinalIgnoreCase);
        SetAboutStatus(upd.AssetSize > 0 && _aboutDownloadIsDirect
            ? LF("发现新版本：{0}（约 {1}）。", upd.Tag, FormatBytes(upd.AssetSize))
            : LF("发现新版本：{0}。", upd.Tag));
        if (_aboutDownloadButton is not null)
        {
            _aboutDownloadButton.Content = _aboutDownloadIsDirect
                ? LF("下载并安装 {0}", upd.AssetName)
                : L("打开 Release 页面");
            _aboutDownloadButton.Visibility = Visibility.Visible;
        }
    }

    /// <summary>通知点击/冷启动激活的落地：窗口回前台，带 update 动作即跳到更新页。</summary>
    private void HandleToastActivation(string argument)
    {
        try
        {
            ActivateFromTray(); // 回前台 + 还原（与托盘图标点击同一条路）
            if (argument.Contains("action=update", StringComparison.OrdinalIgnoreCase))
            {
                if (_rpc is null)
                {
                    // 冷启动瞬间内核还没连上，ShowSettingsAsync 会直接返回：挂起，等就绪后补跳
                    _pendingUpdateDeepLink = true;
                    return;
                }
                _ = OpenUpdateSectionAsync();
            }
        }
        catch (Exception) { } // 激活入口兜底
    }

    /// <summary>内核就绪后补一次冷启动的跳页请求（HandleToastActivation 挂起的）。</summary>
    private void FlushPendingUpdateDeepLink()
    {
        if (!_pendingUpdateDeepLink)
        {
            return;
        }
        _pendingUpdateDeepLink = false;
        _ = OpenUpdateSectionAsync();
    }

    /// <summary>深链到「关于」分区：设置页没开过就先开（ShowSettingsAsync 内部默认进通用区）。</summary>
    private async Task OpenUpdateSectionAsync()
    {
        try
        {
            if (SettingsPage.Visibility != Visibility.Visible)
            {
                await ShowSettingsAsync();
            }
            else if (InSettingsSubPage)
            {
                CloseSettingsSubPage(); // 二级页盖在一级分区之上，先收一层
            }
            await ActivateSectionAsync(AboutSectionId);
        }
        catch (Exception)
        {
            // 内核未连上时 ShowSettingsAsync 直接返回（_rpc 为空）：跳不了就安静待着
        }
    }
}
