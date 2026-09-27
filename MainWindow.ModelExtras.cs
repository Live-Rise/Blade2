using System;
using System.Linq;
using System.Text.Json;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Blade2;

/// <summary>
/// P2 模型边缘项：两档说明（P2-2）、lastUsed 模型记忆展示/恢复（P2-3）、
/// 恢复默认模型一键钮（P2-4）、About 品牌/内测声明（P2-6）。
/// P2-5 匿名用户 ID 有意不做。P2-1「目标 · Round」见 MainWindow.Trajectory.cs。
/// </summary>
public partial class MainWindow
{
    /// <summary>P2-3 模型记忆：lastUsed 全量（provider/model/effort/name），供菜单恢复。
    /// _lastUsedModelId 保留作菜单「（上次使用）」标记的轻量键。</summary>
    private (string Provider, string Model, string Effort, string Name)? _modelLastUsed;

    /// <summary>P2-2：模型菜单两档说明（官方 option.deepseekV4Flash/Pro.description）。
    /// deepseek-official 的 Flash/Pro 写产品文案；其余优先 catalog.description（英文原文逐字命中也翻）。</summary>
    private string ModelOptionDescription(string provider, JsonElement model)
    {
        var id = ModelIdOf(model);
        var row = $"{provider}/{id}";
        if (string.Equals(row, "deepseek-official/deepseek-v4-flash", StringComparison.Ordinal) ||
            (id.Length > 0 && id.EndsWith("-flash", StringComparison.OrdinalIgnoreCase)))
        {
            return L("快速、高效且经济；适合目标明确、常规或并行任务。");
        }
        if (string.Equals(row, "deepseek-official/deepseek-v4-pro", StringComparison.Ordinal) ||
            (id.Length > 0 && id.EndsWith("-pro", StringComparison.OrdinalIgnoreCase)))
        {
            return L("更强的自主编码、知识与复杂推理能力；适合复杂或质量优先的任务，但成本更高。");
        }
        if (model.ValueKind == JsonValueKind.Object &&
            model.TryGetProperty("description", out var d) && d.ValueKind == JsonValueKind.String)
        {
            var desc = d.GetString() ?? "";
            return desc switch
            {
                "Fast, efficient, and economical; suited to focused, routine, or parallel tasks."
                    => L("快速、高效且经济；适合目标明确、常规或并行任务。"),
                "Stronger agentic coding, knowledge, and difficult reasoning; suited to complex or quality-critical tasks at higher cost."
                    => L("更强的自主编码、知识与复杂推理能力；适合复杂或质量优先的任务，但成本更高。"),
                _ => desc,
            };
        }
        return "";
    }

    /// <summary>给模型菜单项挂两档说明：可见第二行 + ToolTip + UIA 描述文本（HelpText）。</summary>
    private void AttachModelOptionDescription(MenuFlyoutItem item, string provider, JsonElement model)
    {
        var desc = ModelOptionDescription(provider, model);
        if (desc.Length == 0)
        {
            return;
        }
        item.Text = $"{item.Text}\n{desc}";
        ToolTipService.SetToolTip(item, desc);
        AutomationProperties.SetHelpText(item, desc);
    }

    /// <summary>解析 modelSelection.lastUsed → _modelLastUsed + _lastUsedModelId（调用方持 _projectionLock）。
    /// 字段同步写；只有菜单重铺回 UI 线程。返回是否有变化。</summary>
    private bool CaptureModelLastUsed(JsonElement view)
    {
        if (view.ValueKind != JsonValueKind.Object ||
            !view.TryGetProperty("lastUsed", out var last) ||
            last.ValueKind != JsonValueKind.Object)
        {
            return false;
        }
        var model = Str(last, "model");
        if (model.Length == 0)
        {
            return false;
        }
        var provider = Str(last, "provider");
        var effort = Str(last, "reasoningEffort");
        var prev = _modelLastUsed;
        if (prev is { } p && p.Model == model && p.Provider == provider && p.Effort == effort &&
            _lastUsedModelId == model)
        {
            return false;
        }
        // 字段同步落；菜单显示名在 UI 线程从 catalog 回填
        _modelLastUsed = (provider, model, effort, model);
        _lastUsedModelId = model;
        PostUi(() =>
        {
            var option = _modelOptions.FirstOrDefault(o => ModelIdOf(o.Model) == model);
            if (option.Model.ValueKind == JsonValueKind.Object)
            {
                _modelLastUsed = (provider, model, effort, option.Name);
            }
            RebuildModelFlyout();
        });
        return true;
    }

    /// <summary>P2-3 恢复：把 lastUsed 的模型/档位重新 selectModel 回当前会话。</summary>
    private async Task RestoreLastUsedModelAsync()
    {
        if (_modelLastUsed is not { } last)
        {
            return;
        }
        var option = _modelOptions.FirstOrDefault(o =>
            ModelIdOf(o.Model) == last.Model &&
            (last.Provider.Length == 0 || o.Provider == last.Provider));
        if (option.Model.ValueKind != JsonValueKind.Object)
        {
            ShellToast.Show(L("模型切换失败"), LF("未能切换到 {0}：目录中未找到该模型。", last.Name.Length > 0 ? last.Name : last.Model));
            return;
        }
        await SelectModelAsync(
            option.Model,
            last.Provider.Length > 0 ? last.Provider : option.Provider,
            last.Name.Length > 0 ? last.Name : option.Name,
            last.Effort.Length > 0 ? last.Effort : null);
    }

    /// <summary>模型菜单「上次使用」恢复行（展示 + 一键恢复）。</summary>
    private void AppendLastUsedMenuItems()
    {
        if (_modelLastUsed is not { } last)
        {
            return;
        }
        ModelFlyout.Items.Add(new MenuFlyoutSeparator());
        ModelFlyout.Items.Add(new MenuFlyoutItem { Text = L("上次使用"), IsEnabled = false });
        var label = last.Name.Length > 0 ? last.Name : last.Model;
        var item = new MenuFlyoutItem
        {
            Text = LF("上次使用：{0}", label),
            Tag = "lastUsed",
        };
        Aut(item, "ModelOption_LastUsed", LF("上次使用：{0}", label));
        AutomationProperties.SetHelpText(item, LF("恢复上次使用的模型：{0}", label));
        ToolTipService.SetToolTip(item, LF("恢复上次使用的模型：{0}", label));
        item.Click += (_, _) => _ = RestoreLastUsedModelAsync();
        ModelFlyout.Items.Add(item);
    }

    /// <summary>P2-4 模型设置页一键恢复默认模型卡（按钮 → settings 回写）。</summary>
    private FrameworkElement MakeRestoreDefaultModelCard()
    {
        var card = NewCard(L("默认模型"), L("一键恢复适配器默认模型目录与内核默认模型。自定义提供方的模型列表不受影响。"));
        var button = Aut(new Button
        {
            Content = L("恢复默认模型"),
            Style = AppStyle("CompactButtonStyle"),
        }, "RestoreDefaultModelButton", L("恢复默认模型"));
        ToolTipService.SetToolTip(button, L("将默认模型与模型目录恢复为内核默认。"));
        button.Click += (_, _) => _ = RestoreDefaultModelAsync();
        card.Children.Add(MakeRow(
            L("恢复默认模型"),
            L("将默认模型与模型目录恢复为内核默认。"),
            button));
        return card;
    }

    /// <summary>一键恢复：unset 两家族 models 覆盖 + unset agent-default-model（回到 catalog.default）。
    /// 立即 settings/mutate 回写，不走编辑卡草稿。</summary>
    private async Task RestoreDefaultModelAsync()
    {
        if (_rpc is null)
        {
            SetSettingsSaveError(L("恢复默认模型失败：内核未连接。"));
            return;
        }
        try
        {
            foreach (var ns in new[] { "llm-deepseek", "llm-pi-ai" })
            {
                await _rpc.CallOkAsync("settings/mutate", new
                {
                    ns,
                    ops = new[] { new { op = "unset", path = new[] { "models" } } },
                });
            }
            await _rpc.CallOkAsync("settings/mutate", new
            {
                ns = "agent-default-model",
                ops = new[]
                {
                    new { op = "unset", path = new[] { "provider" } },
                    new { op = "unset", path = new[] { "model" } },
                },
            });
            _settingsSnapshot = null;
            await EnsureSettingsSnapshotAsync();
            _modelsSavedNotice = L("已恢复默认模型。");
            if (_settingsActiveSection == "models")
            {
                await RenderSectionAsync("models");
            }
            // 输入区模型菜单跟上新目录（无会话/无 RPC 时静默）
            _ = RefreshModelCatalogAsync();
        }
        catch (Exception ex)
        {
            SetSettingsSaveError(LF("恢复默认模型失败：{0}", ex.Message));
        }
    }

    /// <summary>P2-6 About 品牌/内测声明卡：预览版 · DSH 本地构建 + 内测声明正文。
    /// 用 NewCardIn 挂到指定 host（NewCard 只挂 SettingsHost，且外壳已入 host，勿再 Add）。</summary>
    private void AppendAboutBrandNotices(StackPanel host)
    {
        var card = NewCardIn(host, L("品牌与声明"), null);
        card.Children.Add(MakeRow(
            L("版本通道"),
            L("DSH 本地构建"),
            new TextBlock
            {
                Text = L("预览版"),
                Style = AppStyle("BodyStrongTextStyle"),
                VerticalAlignment = VerticalAlignment.Center,
            }));
        AddDivider(card);
        card.Children.Add(new TextBlock
        {
            Text = L("内测声明"),
            Style = AppStyle("BodyStrongTextStyle"),
            TextWrapping = TextWrapping.Wrap,
        });
        var body = L("DeepSeek Harness 目前的 0.1 版本仍处在面向 Harness 开发者进行测试的阶段，还有许多地方需要持续改进和打磨，希望听取广大开发者的反馈建议。预计 DeepSeek Harness 的核心插件以及基础 API 都会在接下来的一段时间内快速迭代、持续演化。\n\n我们期待与全球开发者一起，在开源、开放、可复用、可组合的基础设施之上，共同探索智能上限。欢迎全球 Harness 开发者加入 DSH 插件生态。");
        foreach (var para in body.Split("\n\n", StringSplitOptions.RemoveEmptyEntries))
        {
            card.Children.Add(new TextBlock
            {
                Text = para.Trim(),
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextSecondaryBrush"),
                TextWrapping = TextWrapping.Wrap,
                Margin = new Thickness(0, Sp4, 0, 0),
            });
        }
    }
}
