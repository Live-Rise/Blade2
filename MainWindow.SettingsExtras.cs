using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Threading.Tasks;
using Blade2.Dsh;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Automation;

namespace Blade2;

/// <summary>
/// P1-C 设置域补齐（对标 dsh-client-ui-settings* / dsh-client-ui-agent-preset）：
///  · P1-8  用「创造模式」创作自定义预设（stage cordis + 新会话向导）
///  · P1-9  预设组装 yml 只读查看器（agentPresets/read）
///  · P1-22 设置 applies（live | restart）生效标记
///  · P1-23 连接状态条（异常 / 自动重连中 / 立即重连；连接正常时不显示任何字样）
///  · P1-11 插件设置补全（Subagent 允许模型清单；其余官方 ns 已在插件页）
///  · T28  插件设置 ns 1:1（官方文案/控件对齐 + 字段级恢复默认/已覆盖 + 泛化 describe 兜底）
/// 全部为壳侧 UI，不改内核命名空间语义。
/// </summary>
public partial class MainWindow
{
    // ---------------- P1-9 预设组装 yml 只读查看器 ----------------

    /// <summary>「查看」按钮：agentPresets/read → 只读组装文件（官方 composition / agent.cordis.yml）。</summary>
    private Button MakePresetViewButton(string id, string shownName)
    {
        var view = Aut(new Button { Content = L("查看"), Style = AppStyle("CompactButtonStyle") },
            $"ViewPresetComposition_{id}", LF("查看预设 {0} 的组装", shownName));
        ToolTipService.SetToolTip(view, L("组装（agent.cordis.yml）"));
        view.Click += (_, _) => _ = ShowPresetCompositionAsync(id, shownName);
        return view;
    }

    /// <summary>只读 yml 查看器：官方「Composition (agent.cordis.yml)」对话框，不提供编辑。</summary>
    private async Task ShowPresetCompositionAsync(string id, string shownName)
    {
        if (_rpc is null)
        {
            return;
        }
        try
        {
            var value = await _rpc.CallOkAsync("agentPresets/read", new { agentPreset = id });
            var title = (value.TryGetProperty("name", out var n) && n.ValueKind == JsonValueKind.String
                ? n.GetString() : null) ?? shownName;
            var content = value.TryGetProperty("content", out var c) && c.ValueKind == JsonValueKind.String
                ? c.GetString() ?? ""
                : "";
            var box = Aut(new TextBox
            {
                Text = content,
                IsReadOnly = true,
                AcceptsReturn = true,
                TextWrapping = TextWrapping.NoWrap,
                FontFamily = CodeFont(),
                MinHeight = 280,
                MinWidth = TokenDouble("DialogMinWidth", 420),
                MaxWidth = TokenDouble("DialogMinWidthWide", 640),
            }, "PresetCompositionBox", L("组装（agent.cordis.yml）"));
            var scroll = new ScrollViewer
            {
                Content = box,
                MaxHeight = 420,
                HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
                VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            };
            var dialog = new ContentDialog
            {
                Title = LF("组装（agent.cordis.yml）· {0}", title),
                Content = scroll,
                CloseButtonText = L("关闭"),
                XamlRoot = Content.XamlRoot,
            };
            _ = await dialog.ShowAsync();
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("无法读取预设组装：{0}", ex.Message));
        }
    }

    // ---------------- P1-8 创造模式创作自定义预设 ----------------

    /// <summary>「用「创造模式」创作自定义预设」入口（对标 creatorDraft：
    /// seat.stage("cordis") + startSession）。</summary>
    private Button MakeCreatorDraftButton()
    {
        var btn = Aut(new Button
        {
            Content = L("用「创造模式」创作自定义预设"),
            Style = AppStyle("CompactButtonStyle"),
            Margin = new Thickness(0, Sp8, 0, 0),
        }, "PresetCreatorDraftButton", L("用「创造模式」创作自定义预设"));
        ToolTipService.SetToolTip(btn,
            L("切到创造模式并开始新会话，让 Agent 帮你创建自定义预设。"));
        btn.Click += (_, _) => _ = StartCreatorDraftAsync();
        return btn;
    }

    /// <summary>
    /// 创造模式向导：把下一个会话的预设定为 cordis（创造模式）并开新会话。
    /// 官方 startSession 总是新会话；壳侧若已有空会话，用 agentPresets/select 切到 cordis
    /// （agent 未启动时内核允许切换），否则走 session/create。
    /// </summary>
    private async Task StartCreatorDraftAsync()
    {
        if (_rpc is null)
        {
            _ = ShowErrorAsync(L("内核未连接"));
            return;
        }
        try
        {
            PickAgentPreset("cordis", "创造模式");
            ShowChatPage();
            var blank = _activeSessionId is { Length: > 0 } existing
                        && _messages.Count == 0
                        && _sessions.Any(s => s.SessionId == existing && s.Blank);
            if (blank && _activeSessionId is { Length: > 0 } sid)
            {
                try
                {
                    await _rpc.CallOkAsync("agentPresets/select", new { agentId = sid, agentPreset = "cordis" });
                }
                catch (DshRpcException)
                {
                    // agent 已启动锁死：改建新会话
                    await CreateSessionAsync();
                }
            }
            else
            {
                await CreateSessionAsync();
            }
            AppendSystemMessage(L(
                "已进入创造模式。告诉 Agent 你想要的工具、提示词与能力，它会帮你写出自定义预设的 agent.cordis.yml。"));
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("无法开始创造模式：{0}", ex.Message));
        }
    }

    // ---------------- P1-22 设置 applies（live | restart） ----------------

    /// <summary>该命名空间的生效语义（settings/describe 的 applies 字段）。</summary>
    private string SettingsNsApplies(string ns)
    {
        if (_settingsSnapshot is not null &&
            _settingsSnapshot.TryGetValue(ns, out var snap) &&
            snap.Value.TryGetProperty("applies", out var a) &&
            a.ValueKind == JsonValueKind.String)
        {
            return a.GetString() switch
            {
                "restart" => "restart",
                _ => "live",
            };
        }
        return "live";
    }

    /// <summary>生效标记小胶囊：live=即时生效 / restart=需重启。</summary>
    private Border MakeAppliesBadge(string ns)
    {
        var applies = SettingsNsApplies(ns);
        var text = applies == "restart" ? L("需重启生效") : L("即时生效");
        var brush = applies == "restart" ? ThemeBrush("WarningBrush") : ThemeBrush("TextTertiaryBrush");
        return Aut(new Border
        {
            Child = new TextBlock
            {
                Text = text,
                Style = AppStyle("CaptionTextStyle"),
                Foreground = brush,
            },
            Background = SoftBrushFrom(brush, 0.1),
            CornerRadius = new CornerRadius(4),
            Padding = new Thickness(Sp4, 1, Sp4, 1),
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(Sp6, 0, 0, 0),
        }, $"AppliesBadge_{ns}", text);
    }

    /// <summary>把 applies 标记贴到设置行描述旁（MakeRow 返回的 Grid）。</summary>
    private void AttachAppliesBadge(Grid row, string ns)
    {
        try
        {
            var badge = MakeAppliesBadge(ns);
            // MakeRow 的左列是文本 StackPanel：把标记塞进描述行同列底部
            if (row.Children.Count > 0 && row.Children[0] is StackPanel text)
            {
                var line = new StackPanel
                {
                    Orientation = Orientation.Horizontal,
                    Spacing = Sp4,
                    VerticalAlignment = VerticalAlignment.Center,
                };
                line.Children.Add(new TextBlock
                {
                    Text = SettingsNsApplies(ns) == "restart" ? L("保存后需重启内核才完全生效") : L("保存后即时生效"),
                    Style = AppStyle("CaptionTextStyle"),
                    Foreground = ThemeBrush("TextTertiaryBrush"),
                });
                line.Children.Add(badge);
                text.Children.Add(line);
            }
        }
        catch (Exception) { }
    }

    /// <summary>带 applies 标记的设置行（插件页官方 ns 共用）。</summary>
    private Grid MakeRowWithApplies(string ns, string title, string? description, FrameworkElement control)
    {
        var row = MakeRow(title, description, control);
        AttachAppliesBadge(row, ns);
        return row;
    }

    // ---------------- P1-23 连接状态条 ----------------

    private StackPanel? _connectionStatusHost;
    private TextBlock? _connectionStatusText;
    private Button? _connectionReconnectButton;
    /// <summary>connected | disconnected | connecting | recovered</summary>
    private string _connectionVisual = "connected";

    /// <summary>
    /// 常驻连接状态条：挂在设置页头右侧（对标官方 ConnectionIndicator 贴设置触发器）。
    /// 只在异常态露字（异常 / 自动重连中 / 立即重连）；连接正常（connected/recovered）不占位。
    /// </summary>
    private void MountConnectionStatusBar()
    {
        if (_connectionStatusHost is not null)
        {
            return;
        }
        var host = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = Sp8,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(Sp24, 0, 0, 0),
        };
        AutomationProperties.SetAutomationId(host, "ConnectionStatusBar");
        AutomationProperties.SetName(host, L("连接状态"));

        var text = new TextBlock
        {
            Style = AppStyle("CaptionTextStyle"),
            Foreground = ThemeBrush("TextTertiaryBrush"),
            VerticalAlignment = VerticalAlignment.Center,
            Visibility = Visibility.Collapsed,
        };
        AutomationProperties.SetAutomationId(text, "ConnectionStatusText");

        var reconnect = Aut(new Button
        {
            Content = L("立即重连"),
            Style = AppStyle("CompactButtonStyle"),
            Visibility = Visibility.Collapsed,
        }, "ConnectionReconnectButton", L("立即重连"));
        ToolTipService.SetToolTip(reconnect, L("连接异常，点击立即重连"));
        reconnect.Click += (_, _) => _ = ReconnectNowAsync();

        host.Children.Add(text);
        host.Children.Add(reconnect);
        SettingsHeaderRow.Children.Add(host);

        // 聊天/内容区顶条：仅连接异常时露出（对标官方常驻 ConnectionIndicator 的异常反馈）
        if (ContentSurface.Child is Grid contentGrid)
        {
            var strip = new Border
            {
                Background = SoftBrushFrom(ThemeBrush("WarningBrush"), 0.15),
                Padding = new Thickness(Sp12, Sp4, Sp12, Sp4),
                HorizontalAlignment = HorizontalAlignment.Stretch,
                VerticalAlignment = VerticalAlignment.Top,
                Visibility = Visibility.Collapsed,
            };
            var stripRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp8 };
            var stripText = new TextBlock
            {
                Style = AppStyle("CaptionTextStyle"),
                VerticalAlignment = VerticalAlignment.Center,
            };
            var stripBtn = Aut(new Button
            {
                Content = L("立即重连"),
                Style = AppStyle("CompactButtonStyle"),
            }, "ConnectionStripReconnect", L("立即重连"));
            stripBtn.Click += (_, _) => _ = ReconnectNowAsync();
            stripRow.Children.Add(stripText);
            stripRow.Children.Add(stripBtn);
            strip.Child = stripRow;
            AutomationProperties.SetAutomationId(strip, "ConnectionStatusStrip");
            contentGrid.Children.Add(strip);
            _connectionStatusStrip = strip;
            _connectionStatusStripText = stripText;
        }

        _connectionStatusHost = host;
        _connectionStatusText = text;
        _connectionReconnectButton = reconnect;
        UpdateConnectionStatusBar();
    }

    private Border? _connectionStatusStrip;
    private TextBlock? _connectionStatusStripText;

    /// <summary>按当前视觉态刷新状态条文案/按钮：异常态露字，正常态（connected/recovered）整条收起。</summary>
    private void UpdateConnectionStatusBar()
    {
        if (_connectionStatusText is null || _connectionReconnectButton is null)
        {
            return;
        }
        void Ui()
        {
            switch (_connectionVisual)
            {
                case "disconnected":
                    _connectionStatusText.Text = L("连接异常");
                    _connectionStatusText.Foreground = ThemeBrush("ErrorBrush");
                    _connectionStatusText.Visibility = Visibility.Visible;
                    _connectionReconnectButton.Visibility = Visibility.Visible;
                    ToolTipService.SetToolTip(_connectionReconnectButton, L("连接异常，点击立即重连"));
                    if (_connectionStatusStrip is not null && _connectionStatusStripText is not null)
                    {
                        _connectionStatusStripText.Text = L("连接异常，点击立即重连");
                        _connectionStatusStrip.Background = SoftBrushFrom(ThemeBrush("ErrorBrush"), 0.15);
                        _connectionStatusStrip.Visibility = Visibility.Visible;
                    }
                    break;
                case "connecting":
                    _connectionStatusText.Text = L("自动重连中");
                    _connectionStatusText.Foreground = ThemeBrush("WarningBrush");
                    _connectionStatusText.Visibility = Visibility.Visible;
                    _connectionReconnectButton.Visibility = Visibility.Visible;
                    ToolTipService.SetToolTip(_connectionReconnectButton, L("连接中断，正在自动重试，点击立即重连"));
                    if (_connectionStatusStrip is not null && _connectionStatusStripText is not null)
                    {
                        _connectionStatusStripText.Text = L("连接中断，正在自动重试，点击立即重连");
                        _connectionStatusStrip.Background = SoftBrushFrom(ThemeBrush("WarningBrush"), 0.15);
                        _connectionStatusStrip.Visibility = Visibility.Visible;
                    }
                    break;
                default:
                    // connected / recovered：连接正常不显示任何字样，状态条只负责异常反馈
                    _connectionStatusText.Visibility = Visibility.Collapsed;
                    _connectionReconnectButton.Visibility = Visibility.Collapsed;
                    if (_connectionStatusStrip is not null)
                    {
                        _connectionStatusStrip.Visibility = Visibility.Collapsed;
                    }
                    break;
            }
        }
        if (DispatcherQueue.HasThreadAccess) Ui();
        else PostUi(Ui);
    }

    private void SetConnectionVisual(string state)
    {
        _connectionVisual = state;
        UpdateConnectionStatusBar();
    }

    /// <summary>立即重连：复用内核 mux 幂等修复（ConnectMuxEventsDeferredAsync → EnsureMux）。</summary>
    private async Task ReconnectNowAsync()
    {
        if (_rpc is null)
        {
            return;
        }
        SetConnectionVisual("connecting");
        try
        {
            await _rpc.ConnectMuxEventsDeferredAsync();
            await _rpc.SubscribeEventsAsync();
            SetConnectionVisual("recovered");
            _ = Task.Run(async () =>
            {
                await Task.Delay(2000);
                PostUi(() =>
                {
                    if (_connectionVisual == "recovered")
                    {
                        SetConnectionVisual("connected");
                    }
                });
            });
        }
        catch (Exception)
        {
            SetConnectionVisual("disconnected");
        }
    }

    // ---------------- P1-11 Subagent 允许模型清单（官方 subagent-model-selection） ----------------

    /// <summary>
    /// Subagent「Agent 可选择的模型」清单：开关已由 MakeSubagentToggle 处理，
    /// 这里补官方 allowedModels 复选清单（subagent-model-selection.allowedModels）。
    /// </summary>
    private void AppendSubagentAllowedModels(StackPanel card)
    {
        const string ns = "subagent-model-selection";
        var current = NsAllowedModels();
        var listHost = new StackPanel { Spacing = Sp4, Margin = new Thickness(0, Sp4, 0, 0) };
        Aut(listHost, "SubagentAllowedModelsList", L("Agent 可选择的模型"));

        var options = new List<(string Provider, string Model, string Label)>();
        try
        {
            // 目录：llm/listConfigurableProviders 展开出的已配置模型；回落当前默认 + 已保存项
            var seen = new HashSet<string>(StringComparer.Ordinal);
            void Add(string provider, string model)
            {
                var key = provider + "/" + model;
                if (provider.Length == 0 && model.Length == 0) return;
                if (!seen.Add(key)) return;
                options.Add((provider, model, string.IsNullOrEmpty(provider) ? model : $"{provider} / {model}"));
            }
            foreach (var (p, m) in current) Add(p, m);
            Add(NsString("agent-default-model", "provider", ""), NsString("agent-default-model", "model", ""));
            if (_modelOptions.Count > 0)
            {
                foreach (var opt in _modelOptions)
                {
                    Add(opt.Provider, opt.Name);
                }
            }
        }
        catch (Exception) { }

        if (options.Count == 0)
        {
            listHost.Children.Add(new TextBlock
            {
                Text = L("当前没有模型提供方公布模型。"),
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextSecondaryBrush"),
                TextWrapping = TextWrapping.Wrap,
            });
            card.Children.Add(MakeRow(
                L("Agent 可选择的模型"),
                L("开启后，Agent 可以从下方授权模型中，为每个 Subagent 选择提供方、模型和推理强度。仅影响新会话。"),
                listHost));
            return;
        }

        foreach (var (provider, model, label) in options)
        {
            var selected = current.Any(c => c.Provider == provider && c.Model == model);
            var box = Aut(new CheckBox
            {
                Content = label,
                IsChecked = selected,
            }, $"SubagentModel_{provider}_{model}", label);
            box.Checked += (_, _) => WriteAllowedModels(ns, options, listHost, forced: (provider, model, true));
            box.Unchecked += (_, _) => WriteAllowedModels(ns, options, listHost, forced: (provider, model, false));
            listHost.Children.Add(box);
        }

        listHost.Children.Add(new TextBlock
        {
            Text = L("保存前请至少选择一个模型。"),
            Style = AppStyle("CaptionTextStyle"),
            Foreground = ThemeBrush("TextTertiaryBrush"),
            TextWrapping = TextWrapping.Wrap,
        });
        card.Children.Add(MakeRow(
            L("Agent 可选择的模型"),
            L("开启后，Agent 可以从下方授权模型中，为每个 Subagent 选择提供方、模型和推理强度。仅影响新会话。"),
            listHost));
    }

    // ---------------- T28 插件设置 ns 1:1（官方 settings-plugins 控件语义） ----------------

    /// <summary>官方 4 张插件卡的 ns：硬编码 1:1；其余 writable ns 走泛化兜底。</summary>
    private static readonly HashSet<string> KnownPluginNs = new(StringComparer.Ordinal)
    {
        "shell", "agent-loop", "subagent-model-selection", "web-search-deepseek",
    };

    /// <summary>该字段是否被用户层覆盖（user 有值且与 base 不同）——官方「已覆盖」标记。</summary>
    private bool IsNsFieldOverridden(string ns, string field)
    {
        try
        {
            if (_settingsSnapshot is null || !_settingsSnapshot.TryGetValue(ns, out var snap)) return false;
            if (!snap.Value.TryGetProperty("user", out var user) || user.ValueKind != JsonValueKind.Object) return false;
            if (!user.TryGetProperty(field, out var userVal)) return false;
            if (!snap.Value.TryGetProperty("base", out var bas) || bas.ValueKind != JsonValueKind.Object) return true;
            return !bas.TryGetProperty(field, out var baseVal)
                || !string.Equals(userVal.GetRawText(), baseVal.GetRawText(), StringComparison.Ordinal);
        }
        catch (Exception) { return false; }
    }

    /// <summary>「已覆盖」小胶囊（官方 overridden）。</summary>
    private Border MakeOverriddenBadge(string ns, string field)
    {
        return Aut(new Border
        {
            Child = new TextBlock { Text = L("已覆盖"), Style = AppStyle("CaptionTextStyle"), Foreground = ThemeBrush("TextTertiaryBrush") },
            Background = SoftBrushFrom(ThemeBrush("TextTertiaryBrush"), 0.1),
            CornerRadius = new CornerRadius(4),
            Padding = new Thickness(Sp4, 1, Sp4, 1),
            VerticalAlignment = VerticalAlignment.Center,
            Visibility = IsNsFieldOverridden(ns, field) ? Visibility.Visible : Visibility.Collapsed,
        }, $"OverriddenBadge_{ns}_{field}", L("已覆盖"));
    }

    /// <summary>字段级「恢复默认」（官方 reset：settings/mutate op=unset）。</summary>
    private Button MakeFieldResetButton(string ns, string field, Action? after = null)
    {
        var btn = Aut(new Button { Content = L("恢复默认"), Style = AppStyle("CompactButtonStyle") },
            $"ResetField_{ns}_{field}", LF("恢复 {0} 的默认值", field));
        ToolTipService.SetToolTip(btn, L("清除本字段的用户覆盖，回到内核默认值。"));
        btn.IsEnabled = IsNsFieldOverridden(ns, field);
        btn.Click += (_, _) => _ = ResetNsFieldAsync(ns, field, after);
        return btn;
    }

    /// <summary>立即 unset 该字段的用户覆盖并回读快照。</summary>
    private async Task ResetNsFieldAsync(string ns, string field, Action? after = null)
    {
        if (_rpc is null) return;
        try
        {
            // 丢弃同字段未提交草稿，避免 unset 后又被草稿写回
            for (var i = _settingsEdits.Count - 1; i >= 0; i--)
            {
                if (_settingsEdits[i].Ns == ns && _settingsEdits[i].Path.Length == 1 && _settingsEdits[i].Path[0] == field)
                {
                    _settingsEdits.RemoveAt(i);
                }
            }
            await _rpc.CallOkAsync("settings/mutate", new
            {
                ns,
                ops = new[] { new { op = "unset", path = new[] { field } } },
            });
            _settingsSnapshot = null;
            await EnsureSettingsSnapshotAsync();
            after?.Invoke();
        }
        catch (Exception ex)
        {
            _ = ShowErrorAsync(LF("恢复默认失败：{0}", ex.Message));
        }
    }

    /// <summary>带「已覆盖 / 恢复默认」的官方形态设置行（标题 + 描述 + 控件 + 字段维护钮）。</summary>
    private Grid MakeOfficialFieldRow(string ns, string field, string title, string? description, FrameworkElement control, Action? afterReset = null)
    {
        var host = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6, VerticalAlignment = VerticalAlignment.Center };
        host.Children.Add(control);
        host.Children.Add(MakeOverriddenBadge(ns, field));
        host.Children.Add(MakeFieldResetButton(ns, field, afterReset));
        return MakeRowWithApplies(ns, title, description, host);
    }

    /// <summary>
    /// 泛化兜底：settings/describe 里其余 writable 插件式 ns（非官方 4 卡、非壳/模型/权限域）
    /// 按 schema 字段生成等价行。官方全量 ns 未列出时仍可配置，不丢能力。
    /// </summary>
    private void AppendGenericPluginNsFallback(StackPanel navHost)
    {
        if (_settingsSnapshot is null) return;
        var extras = new List<(string Ns, string Title)>();
        foreach (var (ns, snap) in _settingsSnapshot)
        {
            if (KnownPluginNs.Contains(ns)) continue;
            if (ns.StartsWith("llm-", StringComparison.Ordinal)) continue;
            if (ns is "permission" or "ui-theme" or "ui-chat" or "ui-conversation" or "locale"
                or "agent-default-model" or "agent-presets" or "settings.onboarding") continue;
            if (!snap.Value.TryGetProperty("schema", out var schema)) continue;
            var rehydrated = RehydrateSchema(schema) is { } r ? r : schema;
            if (rehydrated.ValueKind != JsonValueKind.Object) continue;
            if (!rehydrated.TryGetProperty("dict", out var dict) || dict.ValueKind != JsonValueKind.Object) continue;
            if (dict.EnumerateObject().All(p => p.Name is "refs" or "uid")) continue;
            var writable = !snap.Value.TryGetProperty("writable", out var w) || w.ValueKind != JsonValueKind.False;
            if (!writable) continue;
            extras.Add((ns, ns));
        }
        if (extras.Count == 0) return;
        foreach (var (ns, title) in extras.OrderBy(e => e.Ns, StringComparer.Ordinal))
        {
            var id = ns;
            navHost.Children.Add(MakeSettingsNavCard(
                $"PluginsNav_generic_{id}", "\uE730", title, L("由 settings/describe 泛化渲染的附加插件设置。"),
                () => OpenSettingsSubPage(title, host => AddGenericPluginNsCard(host, ns))));
        }
    }

    /// <summary>泛化 ns 卡：按 describe 的 schema.dict 生成 number/string/bool/union 行。</summary>
    private void AddGenericPluginNsCard(StackPanel host, string ns)
    {
        if (_settingsSnapshot is null || !_settingsSnapshot.TryGetValue(ns, out var snap)) return;
        if (!snap.Value.TryGetProperty("schema", out var schema)) return;
        var rehydrated = RehydrateSchema(schema) is { } r ? r : schema;
        if (rehydrated.ValueKind != JsonValueKind.Object ||
            !rehydrated.TryGetProperty("dict", out var dict) || dict.ValueKind != JsonValueKind.Object) return;

        var card = NewCardIn(host, ns, L("由 settings/describe 泛化渲染；字段语义以内核 schema 为准。"));
        var first = true;
        foreach (var prop in dict.EnumerateObject())
        {
            if (prop.Name is "refs" or "uid") continue;
            var field = prop.Name;
            var node = prop.Value;
            if (!first) AddDivider(card);
            first = false;
            var type = node.TryGetProperty("type", out var t) ? t.GetString() : null;
            var desc = node.TryGetProperty("meta", out var meta) && meta.ValueKind == JsonValueKind.Object &&
                       meta.TryGetProperty("description", out var d) && d.ValueKind == JsonValueKind.String
                ? d.GetString() : null;
            FrameworkElement? control = null;
            if (type == "number")
            {
                control = MakeNumberBox(ns, field, NsNumber(ns, field, 0));
            }
            else if (type == "boolean")
            {
                var toggle = Aut(new ToggleSwitch
                {
                    IsOn = NsBool(ns, field, false),
                    OnContent = L("开"),
                    OffContent = L("关"),
                    Style = AppStyle("SettingsToggleSwitchStyle"),
                }, $"Setting_{ns}_{field}", field);
                toggle.Toggled += (_, _) => Edit(ns, field, () => toggle.IsOn);
                control = toggle;
            }
            else if (type == "string")
            {
                control = MakeTextBox(ns, field, NsString(ns, field, ""));
            }
            else
            {
                var choices = UnionChoices(node);
                if (choices.Count > 0)
                {
                    control = MakeChoiceField(ns, field, NsString(ns, field, ""), choices, field);
                }
                else
                {
                    control = MakeTextBox(ns, field, NsString(ns, field, ""));
                }
            }
            if (control is not null)
            {
                card.Children.Add(MakeOfficialFieldRow(ns, field, field, desc, control,
                    () => _ = RenderSectionAsync("plugins")));
            }
        }
    }

    /// <summary>把清单勾选落成 allowedModels 待写回值（与 MakeSubagentToggle 的种子写回同一形态）。</summary>
    private void WriteAllowedModels(
        string ns,
        List<(string Provider, string Model, string Label)> options,
        StackPanel listHost,
        (string Provider, string Model, bool On) forced)
    {
        try
        {
            var picked = new List<(string Provider, string Model)>();
            for (var i = 0; i < options.Count && i < listHost.Children.Count; i++)
            {
                if (listHost.Children[i] is not CheckBox box) continue;
                var opt = options[i];
                var on = (opt.Provider, opt.Model) == (forced.Provider, forced.Model)
                    ? forced.On
                    : box.IsChecked == true;
                if (on) picked.Add((opt.Provider, opt.Model));
            }
            if (picked.Count == 0 && forced.On)
            {
                picked.Add((forced.Provider, forced.Model));
            }
            Edit(ns, "allowedModels", () => AllowedModelsDraft(picked));
        }
        catch (Exception) { }
    }
}
