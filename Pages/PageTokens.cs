using System;
using System.Collections.Generic;
using System.Text.Json;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Blade2;

/// <summary>
/// 页面级共享小工具：窗口主题画刷探针收集 + $events 载荷解包。
/// 探针机制与 MainWindow 一致（见 MainWindow.xaml 顶部的 ThemeProbe* 说明）：
/// 代码里要用主题画刷时，读的是 XAML 里以 {ThemeResource} 绑定的 Collapsed TextBlock，
/// 这样拿到的是"窗口当前主题"解析出的 brush，而不是 Application.Current.Resources 的
/// 应用主题快照（壳可单独换肤，两者可能不一致）。
/// </summary>
internal static class PageTokens
{
    /// <summary>收集探针容器里名为 Probe* 的 TextBlock：键 = 去掉 "Probe" 前缀的资源名。</summary>
    public static Dictionary<string, Func<Brush>> Collect(Panel probeHost)
    {
        var map = new Dictionary<string, Func<Brush>>(StringComparer.Ordinal);
        foreach (var child in probeHost.Children)
        {
            if (child is TextBlock { Name: var name } tb && name.StartsWith("Probe", StringComparison.Ordinal) && name.Length > 5)
            {
                var key = name[5..];
                map[key] = () => tb.Foreground;
            }
        }
        return map;
    }

    /// <summary>$events 的 emit 帧载荷：args 是位置参数数组，取 args[0] 作单参载荷。</summary>
    public static bool TryEventPayload(JsonElement frame, out JsonElement payload)
    {
        payload = default;
        if (frame.ValueKind != JsonValueKind.Object ||
            !frame.TryGetProperty("args", out var args) || args.ValueKind != JsonValueKind.Array || args.GetArrayLength() == 0)
        {
            return false;
        }
        payload = args[0];
        return true;
    }
}
