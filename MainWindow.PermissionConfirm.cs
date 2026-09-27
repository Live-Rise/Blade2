using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Blade2;

/// <summary>
/// P0-5：完全权限（danger-full-access）风险确认流。
/// 任何写回路径切到该预设前必须先弹确认；勾选风险声明前确认键禁用；取消则不改权限。
/// 文案对标 @deepseek-ai/dsh-client-ui-permission-presets（confirm.* / accessZh）。
/// </summary>
public sealed partial class MainWindow
{
    /// <summary>须过 GUI 风险门的权限预设机器值（对标包内 FULL_ACCESS_PRESET）。</summary>
    private const string DangerFullAccessPresetId = "danger-full-access";

    private static bool IsDangerFullAccessPreset(string? value) =>
        string.Equals(value, DangerFullAccessPresetId, StringComparison.OrdinalIgnoreCase);

    /// <summary>commands/execute 行是否为切到完全权限的 /permission（UI 预设与输入框直输共用）。</summary>
    private static bool IsDangerFullAccessCommand(string line)
    {
        var parts = line.Trim().Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries);
        return parts.Length >= 2
            && parts[0].Equals("/permission", StringComparison.OrdinalIgnoreCase)
            && parts[1].Trim('"', '\'').Equals(DangerFullAccessPresetId, StringComparison.OrdinalIgnoreCase);
    }

    // ContentDialog 同时只能开一张（COMException）；确认框重入时直接拒绝切换。
    private bool _permissionConfirmShowing;

    /// <summary>
    /// 完全权限风险确认。返回 true 才允许写回；取消/异常/重入一律 false。
    /// forNewSessionDefault：设置默认权限、空态默认权限（文案说「新会话」）；
    /// 否则为会话内 /permission 切换（文案说「智能体 / 当前任务」）。
    /// </summary>
    private async Task<bool> ConfirmDangerFullAccessAsync(bool forNewSessionDefault)
    {
        if (_permissionConfirmShowing)
        {
            return false;
        }
        _permissionConfirmShowing = true;
        try
        {
            // 对标 client.js confirm.description：settings 与 session 两套措辞
            var description = forNewSessionDefault
                ? L("启用完全权限后，新会话将减少确认步骤，并且可以直接执行更多操作，包括敏感操作、文件修改或外部命令。仅建议在你信任后续任务时使用。")
                : L("启用完全权限后，智能体将减少确认步骤，并且可以直接执行更多操作，包括敏感操作、文件修改或外部命令。仅建议在你信任当前任务时使用。");

            var body = new StackPanel { Spacing = Sp12, Width = 420 };
            body.Children.Add(new TextBlock
            {
                Text = description,
                TextWrapping = TextWrapping.Wrap,
                Style = AppStyle("BodyTextStyle"),
            });

            // UIA：RiskAckCheckBox —— 勾选前确认键 IsEnabled=false
            var riskAck = Aut(new CheckBox
            {
                Content = L("我已了解风险，并愿意继续"),
            }, "RiskAckCheckBox", L("我已了解风险，并愿意继续"));
            body.Children.Add(riskAck);

            var dialog = new ContentDialog
            {
                Title = L("确认启用完全权限？"),
                Content = body,
                PrimaryButtonText = L("启用完全权限"),
                CloseButtonText = L("取消"),
                // 危险操作默认键落在「取消」：Enter 不会在未勾选时误开完全权限
                DefaultButton = ContentDialogButton.Close,
                IsPrimaryButtonEnabled = false,
                XamlRoot = Content.XamlRoot,
            };
            riskAck.Checked += (_, _) => dialog.IsPrimaryButtonEnabled = true;
            riskAck.Unchecked += (_, _) => dialog.IsPrimaryButtonEnabled = false;

            // UIA：ConfirmDangerFullAccessButton —— 模板内 PrimaryButton 挂上自动化 Id
            dialog.Opened += (_, _) =>
            {
                if (FindContentDialogButton(dialog, "PrimaryButton") is { } primary)
                {
                    Aut(primary, "ConfirmDangerFullAccessButton", L("启用完全权限"));
                }
            };

            var result = await dialog.ShowAsync();
            return result == ContentDialogResult.Primary && riskAck.IsChecked == true;
        }
        catch (Exception)
        {
            return false; // 异常不得放行完全权限
        }
        finally
        {
            _permissionConfirmShowing = false;
        }
    }

    /// <summary>ContentDialog 模板按钮（x:Name = PrimaryButton / CloseButton …）。</summary>
    private static Button? FindContentDialogButton(DependencyObject node, string name, int depth = 0)
    {
        if (depth > 32)
        {
            return null;
        }
        var count = VisualTreeHelper.GetChildrenCount(node);
        for (var i = 0; i < count; i++)
        {
            var child = VisualTreeHelper.GetChild(node, i);
            if (child is Button button && button.Name == name)
            {
                return button;
            }
            if (FindContentDialogButton(child, name, depth + 1) is { } found)
            {
                return found;
            }
        }
        return null;
    }

    /// <summary>
    /// 设置页「默认权限模式」下拉：与 MakeChoiceField 同结构，但切到 danger-full-access
    /// 先过风险确认；取消则回退上一档且不登记 Edit（避免 400ms 自动保存抢写）。
    /// </summary>
    private ComboBox MakeDefaultPermissionChoiceField(string current, List<(string Value, string Label)> choices)
    {
        var box = new ComboBox { MinWidth = TokenDouble("FieldMinWidth", 200), MaxDropDownHeight = 320 };
        AutomationProperties.SetAutomationId(box, "Setting_permission_defaultPreset");
        AutomationProperties.SetName(box, L("默认权限模式"));
        foreach (var (value, label) in choices)
        {
            var item = new ComboBoxItem { Content = label, Tag = value };
            box.Items.Add(item);
            if (value == current)
            {
                box.SelectedItem = item;
            }
        }
        // 初始赋值会触发 SelectionChanged：先挂"已就绪"标记，只有用户操作后的变化才登记
        var ready = false;
        var lastAccepted = current;
        box.SelectionChanged += async (_, _) =>
        {
            try
            {
                if (!ready)
                {
                    return;
                }
                var value = (box.SelectedItem as ComboBoxItem)?.Tag as string;
                if (value is null || value == lastAccepted)
                {
                    return;
                }
                if (IsDangerFullAccessPreset(value))
                {
                    var ok = await ConfirmDangerFullAccessAsync(forNewSessionDefault: true);
                    if (!ok)
                    {
                        ready = false;
                        box.SelectedItem = box.Items.OfType<ComboBoxItem>()
                            .FirstOrDefault(i => i.Tag as string == lastAccepted);
                        ready = true;
                        return;
                    }
                }
                lastAccepted = value;
                Edit("permission", "defaultPreset", () => value);
            }
            catch (Exception)
            {
                // async void 事件入口兜底（0xc000027b 教训）
            }
        };
        box.Loaded += (_, _) => ready = true;
        return box;
    }
}
