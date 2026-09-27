using System;
using System.Linq;
using System.Text.Json;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Automation;

namespace Blade2;

/// <summary>
/// P1-12 / P1-24 欢迎 onboarding + 版本化 welcome notice（对标
/// @deepseek-ai/dsh-client-ui-settings-general 的 settings.onboarding 槽
/// 与 settings-models 的 WelcomeNotice / DeepSeekOnboardingDialog）。
///  · 版本化内测声明：ui-settings-general.welcomeNoticeVersion = WELCOME_NOTICE_VERSION（0.1.7 前为 ui-onboarding）
///  · 首次 API Key 向导：无可对话提供方时引导配置 DeepSeek 官方密钥
/// 二者合并为「首次启动 / 版本化 welcome」对话框流，内核就绪后触发一次。
/// </summary>
public partial class MainWindow
{
    /// <summary>与官方 WELCOME_NOTICE_VERSION 对齐；文案实质变更时再升版本。</summary>
    private const string WelcomeNoticeVersion = "2026-08-13.1";
    // ui-onboarding 自内核 0.1.7 起并入 ui-settings-general（LEGACY_SECTION_ENTRIES），
    // 确认状态随之迁到新 ns：settings/update 对旧 ns 会拒绝。
    private const string OnboardingNs = "ui-settings-general";
    private const string WelcomeAckField = "welcomeNoticeVersion";

    private bool _onboardingShown;
    private bool _onboardingDialogOpen;

    /// <summary>内核就绪后调用：版本化 welcome → API Key 向导（各自幂等）。</summary>
    private async Task MaybeShowOnboardingAsync()
    {
        if (_onboardingShown || _rpc is null)
        {
            return;
        }
        _onboardingShown = true;
        try
        {
            if (!IsWelcomeNoticeAcknowledged())
            {
                await ShowWelcomeNoticeAsync();
            }
            await MaybeShowDeepSeekOnboardingAsync();
        }
        catch (Exception ex)
        {
            System.Diagnostics.Debug.WriteLine($"[onboarding] {ex}");
        }
    }

    /// <summary>welcome 是否已确认（ui-settings-general.welcomeNoticeVersion === 当前版本）。</summary>
    private bool IsWelcomeNoticeAcknowledged()
    {
        if (_settingsSnapshot is not null &&
            _settingsSnapshot.TryGetValue(OnboardingNs, out var snap) &&
            snap.Value.TryGetProperty("value", out var v) && v.ValueKind == JsonValueKind.Object &&
            v.TryGetProperty(WelcomeAckField, out var ack) && ack.ValueKind == JsonValueKind.String)
        {
            return string.Equals(ack.GetString(), WelcomeNoticeVersion, StringComparison.Ordinal);
        }
        // 快照缺失时再问一次内核（冷启动 describe 可能尚未拉到 ui-settings-general）
        return false;
    }

    /// <summary>版本化内测声明（官方 welcomeTitle / welcomeBody / welcomeContinue）。</summary>
    private async Task ShowWelcomeNoticeAsync()
    {
        if (_onboardingDialogOpen || _rpc is null)
        {
            return;
        }
        _onboardingDialogOpen = true;
        try
        {
            var body = new StackPanel { Spacing = Sp12, MinWidth = TokenDouble("DialogMinWidth", 420) };
            var paragraphs = L("DeepSeek Harness 目前的 0.1 版本仍处在面向 Harness 开发者进行测试的阶段，还有许多地方需要持续改进和打磨，希望听取广大开发者的反馈建议。预计 DeepSeek Harness 的核心插件以及基础 API 都会在接下来的一段时间内快速迭代、持续演化。\n\n我们期待与全球开发者一起，在开源、开放、可复用、可组合的基础设施之上，共同探索智能上限。欢迎全球 Harness 开发者加入 DSH 插件生态。")
                .Split("\n\n", StringSplitOptions.RemoveEmptyEntries);
            foreach (var para in paragraphs)
            {
                body.Children.Add(new TextBlock
                {
                    Text = para.Trim(),
                    Style = AppStyle("BodyTextStyle"),
                    TextWrapping = TextWrapping.Wrap,
                });
            }
            var error = new TextBlock
            {
                Text = L("暂时无法保存确认状态，请重试。"),
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("ErrorBrush"),
                Visibility = Visibility.Collapsed,
                TextWrapping = TextWrapping.Wrap,
            };
            body.Children.Add(error);

            var dialog = new ContentDialog
            {
                Title = L("内测声明"),
                Content = body,
                PrimaryButtonText = L("继续"),
                DefaultButton = ContentDialogButton.Primary,
                XamlRoot = Content.XamlRoot,
            };
            while (true)
            {
                var result = await dialog.ShowAsync();
                if (result != ContentDialogResult.Primary)
                {
                    // 无关闭钮时仍要能走掉：Esc / 点遮罩按未确认处理，下次再弹
                    return;
                }
                try
                {
                    await AcknowledgeWelcomeNoticeAsync();
                    return;
                }
                catch (Exception)
                {
                    error.Visibility = Visibility.Visible;
                }
            }
        }
        finally
        {
            _onboardingDialogOpen = false;
        }
    }

    /// <summary>把确认版本写进 ui-settings-general（settings/update patch）。</summary>
    private async Task AcknowledgeWelcomeNoticeAsync()
    {
        if (_rpc is null)
        {
            throw new InvalidOperationException(L("内核未连接"));
        }
        await _rpc.CallOkAsync("settings/update", new
        {
            ns = OnboardingNs,
            patch = new { welcomeNoticeVersion = WelcomeNoticeVersion },
        });
        _settingsSnapshot = null;
        await EnsureSettingsSnapshotAsync();
    }

    /// <summary>
    /// 首次 API Key 向导（官方 onboardingTitle/Description/Later/Save）。
    /// 已有可对话提供方则整步跳过；否则引导配置 DeepSeek 官方密钥。
    /// </summary>
    private async Task MaybeShowDeepSeekOnboardingAsync()
    {
        if (_onboardingDialogOpen || _rpc is null)
        {
            return;
        }
        // 任一提供方已可用 → 不打扰
        if (HasUsableProvider())
        {
            return;
        }
        var keyEnv = NsString("llm-deepseek", "apiKeyEnv", "DEEPSEEK_API_KEY");
        var creds = await DescribeSearchCredentialsAsync(keyEnv);
        if (creds.Any(c => c.Configured))
        {
            return;
        }

        _onboardingDialogOpen = true;
        try
        {
            var host = new StackPanel { Spacing = Sp12, MinWidth = TokenDouble("DialogMinWidth", 420) };
            host.Children.Add(new TextBlock
            {
                Text = L("配置 DeepSeek 官方模型，即可开始使用。"),
                Style = AppStyle("BodyTextStyle"),
                TextWrapping = TextWrapping.Wrap,
            });
            var keyBox = Aut(new PasswordBox
            {
                PlaceholderText = L("输入 API 密钥"),
                MinWidth = FieldWidth,
            }, "OnboardingDeepSeekApiKey", L("API Key"));
            host.Children.Add(keyBox);
            var error = new TextBlock
            {
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("ErrorBrush"),
                Visibility = Visibility.Collapsed,
                TextWrapping = TextWrapping.Wrap,
            };
            host.Children.Add(error);

            var dialog = new ContentDialog
            {
                Title = L("添加一个 API Key 开始使用"),
                Content = host,
                PrimaryButtonText = L("保存并继续"),
                CloseButtonText = L("稍后配置"),
                DefaultButton = ContentDialogButton.Primary,
                XamlRoot = Content.XamlRoot,
            };
            while (true)
            {
                var result = await dialog.ShowAsync();
                if (result != ContentDialogResult.Primary)
                {
                    return; // 稍后配置
                }
                var key = keyBox.Password.Trim();
                if (key.Length == 0)
                {
                    error.Text = L("请输入 API 密钥。");
                    error.Visibility = Visibility.Visible;
                    continue;
                }
                try
                {
                    await _rpc.CallOkAsync("credentials/set", new { @ref = keyEnv, value = key });
                    return;
                }
                catch (Exception ex)
                {
                    error.Text = LF("保存失败：{0}", ex.Message);
                    error.Visibility = Visibility.Visible;
                }
            }
        }
        finally
        {
            _onboardingDialogOpen = false;
        }
    }

    /// <summary>是否已有可对话的提供方（默认模型已配，或 llm-* 命名空间下有 providers/apiKey）。</summary>
    private bool HasUsableProvider()
    {
        var provider = NsString("agent-default-model", "provider", "");
        var model = NsString("agent-default-model", "model", "");
        if (provider.Length > 0 && model.Length > 0)
        {
            return true;
        }
        foreach (var ns in new[] { "llm-deepseek", "llm-pi-ai" })
        {
            if (_settingsSnapshot is not null &&
                _settingsSnapshot.TryGetValue(ns, out var snap) &&
                snap.Value.TryGetProperty("value", out var v) && v.ValueKind == JsonValueKind.Object)
            {
                if (v.TryGetProperty("providers", out var providers) && providers.ValueKind == JsonValueKind.Object &&
                    providers.EnumerateObject().Any())
                {
                    return true;
                }
            }
        }
        return false;
    }
}
