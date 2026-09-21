using System;
using System.Collections.Generic;
using System.Text.Json;
using System.Threading;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Blade2;

/// <summary>
/// 上下文容量圈（对标官方端 ContextMeter，落在输入动作行模型胶囊与发送键之间）：
/// 数据 = 内核 contextPressure 投影的壳侧折叠（镜像 dsh-client-connection 的 fixture parallel）——
/// 分子 pressureTokens = 最近一次 usage 采样的 prompt 侧 token
/// （inputTokens + cacheReadTokens + cacheWriteTokens，官方 pressureFrom 口径）；
/// 分母 contextWindow = 最近一条 request/context journal 事件的 data.contextWindow。
/// 采样口径逐条对齐官方 usageSampleOf：assistant/message 以 data.usage 起步、被同一事件
/// stream 里 type=="usage" 的 chunk 覆盖（后到者胜）；assistant/attempt 只认 stream 里的
/// usage chunk（官方对 attempt 不读 data.usage）。
/// 官方主端另有的 projectedTokens（采样 + surface 折叠增量）需要重实现 surface 估算器，
/// 官方 client-connection 明确缺席该字段、所有消费方回退裸采样——本壳同样只做裸采样口径。
/// 入口 = RenderEventCore（page 回放 + follow 实时流共用），历史与实时天然全覆盖；
/// 会话切换先清零再回放重建（last-wins 无累计，翻页不会翻倍）。
/// </summary>
public partial class MainWindow
{
    /// <summary>单会话的容量折叠状态（仅 UI 线程访问）。</summary>
    private sealed class ContextMeterState
    {
        public long? ContextWindow;
        public long? PressureTokens;
    }

    private readonly Dictionary<string, ContextMeterState> _contextMeters = new(StringComparer.Ordinal);

    private ContextMeterState GetOrCreateContextMeter(string sid)
    {
        if (!_contextMeters.TryGetValue(sid, out var st))
        {
            st = new ContextMeterState();
            _contextMeters[sid] = st;
        }
        return st;
    }

    /// <summary>journal 事件折叠入口。UI 线程调用（RenderEventCore 保证）。</summary>
    private void TrackContextMeter(string type, JsonElement data)
    {
        try
        {
            var sid = Volatile.Read(ref _activeSessionId);
            if (string.IsNullOrEmpty(sid) || data.ValueKind != JsonValueKind.Object)
            {
                return;
            }
            var st = GetOrCreateContextMeter(sid);
            switch (type)
            {
                case "request/context":
                    if (data.TryGetProperty("contextWindow", out var cw) &&
                        cw.ValueKind == JsonValueKind.Number && cw.GetDouble() > 0)
                    {
                        st.ContextWindow = (long)cw.GetDouble();
                    }
                    break;
                case "assistant/message":
                case "assistant/attempt":
                    var pressure = PressureTokensOf(data, type == "assistant/message");
                    if (pressure is not null)
                    {
                        st.PressureTokens = pressure;
                    }
                    break;
                default:
                    return; // 其余事件不翻动容量指示
            }
            UpdateContextMeterUi();
        }
        catch (Exception) { } // 容量指示失败不致命：胶囊保持上次值
    }

    /// <summary>
    /// usage 采样 → prompt 侧 token（官方 usageSampleOf + pressureFrom 口径）。
    /// assistant/message 以 data.usage 起步、被 stream 里 type=="usage" 的 chunk 覆盖；
    /// assistant/attempt 只认 stream chunk（官方对 attempt 不读 data.usage）。
    /// </summary>
    private static long? PressureTokensOf(JsonElement data, bool readDirectUsage)
    {
        var usage = default(JsonElement);
        var found = false;
        if (readDirectUsage && data.TryGetProperty("usage", out var direct) && direct.ValueKind == JsonValueKind.Object)
        {
            usage = direct;
            found = true;
        }
        if (data.TryGetProperty("stream", out var stream) && stream.ValueKind == JsonValueKind.Array)
        {
            foreach (var piece in stream.EnumerateArray())
            {
                if (piece.ValueKind != JsonValueKind.Object ||
                    !piece.TryGetProperty("chunk", out var chunk) ||
                    chunk.ValueKind != JsonValueKind.Object)
                {
                    continue;
                }
                if (chunk.TryGetProperty("type", out var ct) && ct.GetString() == "usage" &&
                    chunk.TryGetProperty("usage", out var cu) && cu.ValueKind == JsonValueKind.Object)
                {
                    usage = cu;
                    found = true;
                }
            }
        }
        if (!found)
        {
            return null;
        }
        return (UsageLong(usage, "inputTokens") ?? 0) +
               (UsageLong(usage, "cacheReadTokens") ?? 0) +
               (UsageLong(usage, "cacheWriteTokens") ?? 0);
    }

    private static long? UsageLong(JsonElement usage, string name)
        => usage.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number && v.GetDouble() >= 0
            ? (long)v.GetDouble()
            : null;

    /// <summary>occupancy：分子分母齐备才给数（官方 contextOccupancy：缺一即不显示）。
    /// percent 上限 100（官方 Math.min(100, …)），四舍五入对齐 JS Math.round（half away from zero）。</summary>
    private static (long Used, long Window, int Percent)? ContextOccupancy(ContextMeterState? st)
        => st is { PressureTokens: { } used, ContextWindow: { } win } && win > 0
            ? (used, win, Math.Min(100, (int)Math.Round(100.0 * used / win, MidpointRounding.AwayFromZero)))
            : null;

    /// <summary>会话切换清零：状态由随后的完整 transcript 回放重建（last-wins 无累计，本就不会翻倍；
    /// 清零只为不让上一个会话的容量/采样在新会话首批事件到达前残留显示）。</summary>
    private void ResetContextMeter()
    {
        var sid = Volatile.Read(ref _activeSessionId);
        if (!string.IsNullOrEmpty(sid))
        {
            _contextMeters.Remove(sid);
        }
        UpdateContextMeterUi();
    }

    private bool _contextMeterVisible;

    /// <summary>环几何常量：与官方 viewBox 0 0 14 14、线宽 2 对齐（盒子 14、圆心 7、半径 6，
    /// 12×12 的圆 + 居中线宽 2 → 描边带落在 5..7，不出盒）。</summary>
    private const double RingCenter = 7.0;
    private const double RingRadius = 6.0;

    /// <summary>刷新容量圈（动作行）：数据不齐即隐藏（与官方一致）；显隐翻转时强制重算动作行
    /// 宽度预算——圆环出现会挤占选择器组的可用宽度，与选择器显隐同一处理路径。</summary>
    private void UpdateContextMeterUi()
    {
        try
        {
            var sid = Volatile.Read(ref _activeSessionId);
            _contextMeters.TryGetValue(sid ?? "", out var st);
            var occ = ContextOccupancy(st);
            var visible = occ is not null;
            ContextMeterRing.Visibility = visible ? Visibility.Visible : Visibility.Collapsed;
            if (occ is { } o)
            {
                SetContextMeterRing(o.Percent);
                var aria = LF("上下文已用 {0}%", o.Percent);
                ToolTipService.SetToolTip(ContextMeterRing, aria);
                Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(ContextMeterRing, aria);
            }
            if (visible != _contextMeterVisible)
            {
                _contextMeterVisible = visible;
                ApplyComposerAdaptiveLayout(force: true);
            }
        }
        catch (Exception) { }
    }

    /// <summary>percent → 弧几何：起点恒为 12 点，顺时针扫过 percent×3.6°。
    /// 0 值整弧隐藏；满值走整圆（弧起终重合是退化几何，渲染结果不确定，官方满值同样给整圆）；
    /// 其余用 ArcSegment，大弧标志按扫过角 &gt;180° 置位。</summary>
    private void SetContextMeterRing(int percent)
    {
        var p = Math.Clamp(percent, 0, 100);
        var partial = p is > 0 and < 100;
        ContextMeterArc.Visibility = partial ? Visibility.Visible : Visibility.Collapsed;
        ContextMeterFullRing.Visibility = p >= 100 ? Visibility.Visible : Visibility.Collapsed;
        if (!partial)
        {
            return;
        }
        var angle = p * Math.PI / 50.0; // percent × 3.6° → 弧度
        var figure = new PathFigure
        {
            StartPoint = new Windows.Foundation.Point(RingCenter, RingCenter - RingRadius),
        };
        figure.Segments.Add(new ArcSegment
        {
            Point = new Windows.Foundation.Point(
                RingCenter + RingRadius * Math.Sin(angle),
                RingCenter - RingRadius * Math.Cos(angle)),
            Size = new Windows.Foundation.Size(RingRadius, RingRadius),
            SweepDirection = SweepDirection.Clockwise,
            IsLargeArc = angle > Math.PI,
        });
        var geometry = new PathGeometry();
        geometry.Figures.Add(figure);
        ContextMeterArc.Data = geometry;
    }

    /// <summary>
    /// 容量圈点击 → 明细面板（对标官方端 ContextMeter 面板：percent + 已用/容量 + 占比条）。
    /// 官方 breakdown 三分段（system/tools/messages）依赖 surface 折叠估算器，壳侧不做，
    /// 走官方 breakdownTotal==0 时的单段退化形态。
    /// </summary>
    private void ShowContextPanel(Microsoft.UI.Xaml.FrameworkElement anchor)
    {
        try
        {
            var sid = Volatile.Read(ref _activeSessionId);
            _contextMeters.TryGetValue(sid ?? "", out var st);
            var occ = ContextOccupancy(st);
            if (occ is null)
            {
                return;
            }
            var panel = new StackPanel { Spacing = 10, MinWidth = 250 };
            // 头部：上下文 …… percent（官方把 percent 放标题行右侧）
            var head = new Grid { ColumnSpacing = 16 };
            head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var hl = new TextBlock { Text = L("上下文") };
            var hv = new TextBlock { Text = occ.Value.Percent + "%", HorizontalAlignment = HorizontalAlignment.Right };
            Microsoft.UI.Xaml.Controls.Grid.SetColumn(hl, 0);
            Microsoft.UI.Xaml.Controls.Grid.SetColumn(hv, 1);
            head.Children.Add(hl);
            head.Children.Add(hv);
            panel.Children.Add(head);
            panel.Children.Add(MakePanelSeparator());
            // 占比条：单段总量（官方无 breakdown 时的退化形态）
            panel.Children.Add(new ProgressBar
            {
                Minimum = 0,
                Maximum = 100,
                Value = occ.Value.Percent,
                Height = 4,
                HorizontalAlignment = HorizontalAlignment.Stretch,
            });
            // 官方 figures：~{used} / {window}（紧凑近似值）
            panel.Children.Add(StatRow(L("已用"),
                "~" + FormatTokensCompact(occ.Value.Used) + " / " + FormatTokensCompact(occ.Value.Window) + " tok"));
            panel.Children.Add(StatRow(L("已用 token"), TokensN0(occ.Value.Used)));
            panel.Children.Add(StatRow(L("上下文容量"), TokensN0(occ.Value.Window)));
            ShowRunFlyout(anchor, panel);
        }
        catch (Exception) { }
    }
}
