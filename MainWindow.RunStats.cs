using System;
using System.Collections.Generic;
using System.Text.Json;
using System.Threading;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;

namespace Blade2;

/// <summary>
/// 会话运行状态条（composer 上方）+ 两块可点开的统计面板（对标官方端）：
/// 左胶囊「{轮} 轮 {步} 步 · {tok/s}」→ 会话统计面板（模型用时/工具调用用时/TTFT/TPS）；
/// 右胶囊「{累计} tok · 缓存命中 {%}」→ Token 用量面板（总量/缓存命中/未缓存输入/缓存读取/输出）。
/// （输入动作行里的「上下文 nn%」胶囊是第三块面板，实现见 MainWindow.ContextMeter.cs。）
/// 折叠算法逐条镜像内核 @deepseek-ai/dsh-session-stats 的 sessionStats 投影——
/// step/end 计轮/步（轮变化才计轮）、assistant/message 关步并按首 token 时差累计
/// decodeMs/decodeTokens 与 llmMs/ttft、tool/call→tool/result 累计 toolMs，
/// usage（缺失时按 input+output+cacheRead+cacheWrite 口径）累计 token 总量与缓存命中。
/// 数据入口 = RenderEventCore（page 回放 + follow 实时流都经此），历史与实时天然全覆盖；
/// 会话切换时状态随回放重建（先清零再重放，避免翻页重复累计）。
/// </summary>
public partial class MainWindow
{
    /// <summary>单个会话的运行折叠状态（仅 UI 线程访问）。</summary>
    private sealed class RunStatsState
    {
        public long Turns;
        public long Steps;
        public string? LastTurn;
        public long LlmMs;
        public long ToolMs;
        public long TtftMs;
        public long TtftSteps;
        public long DecodeMs;
        public long DecodeTokens;
        public long OutputTokens;
        public long TotalTokens;
        public long CacheReadTokens;
        public long InputTokens;
        public bool SystemPromptSeen;
        public string? OpenStepTurn;
        public long OpenStepStartMs;
        public long? FirstTokenMs;
        public readonly Dictionary<string, long> PendingCalls = new(StringComparer.Ordinal);
    }

    private readonly Dictionary<string, RunStatsState> _runStats = new(StringComparer.Ordinal);

    private RunStatsState GetOrCreateRunStats(string sid)
    {
        if (!_runStats.TryGetValue(sid, out var st))
        {
            st = new RunStatsState();
            _runStats[sid] = st;
        }
        return st;
    }

    /// <summary>journal 事件折叠入口。UI 线程调用（RenderEventCore 保证），事件带 time 才计速率。</summary>
    private void TrackRunStats(JsonElement ev, string type, JsonElement data)
    {
        try
        {
            var sid = Volatile.Read(ref _activeSessionId);
            if (string.IsNullOrEmpty(sid) || data.ValueKind != JsonValueKind.Object)
            {
                return;
            }
            var st = GetOrCreateRunStats(sid);
            var time = ev.TryGetProperty("time", out var tv) && tv.ValueKind == JsonValueKind.Number ? (long)tv.GetDouble() : 0L;
            var turn = NumToString(data, "turn");

            switch (type)
            {
                case "step/start":
                    st.OpenStepTurn = turn;
                    st.OpenStepStartMs = time;
                    st.FirstTokenMs = null;
                    break;
                case "assistant/attempt":
                    if (st.OpenStepTurn is not null && st.OpenStepTurn == turn && st.FirstTokenMs is null)
                    {
                        st.FirstTokenMs = FirstTokenFromStream(data);
                    }
                    break;
                case "assistant/message":
                    if (st.OpenStepTurn is not null && st.OpenStepTurn == turn)
                    {
                        if (time > st.OpenStepStartMs && st.OpenStepStartMs > 0)
                        {
                            st.LlmMs += time - st.OpenStepStartMs;
                        }
                        var first = st.FirstTokenMs ?? FirstTokenFromStream(data);
                        var output = UsageField(data, "outputTokens");
                        if (first is long f)
                        {
                            if (f > st.OpenStepStartMs && st.OpenStepStartMs > 0)
                            {
                                st.TtftMs += f - st.OpenStepStartMs;
                                st.TtftSteps++;
                            }
                            if (output is long o && time > f)
                            {
                                st.DecodeMs += time - f;
                                st.DecodeTokens += o;
                            }
                        }
                        st.OpenStepTurn = null;
                    }
                    AccumulateUsage(st, data);
                    break;
                case "tool/call":
                    var callId = Str(data, "callId");
                    if (callId.Length > 0 && time > 0)
                    {
                        st.PendingCalls[callId] = time;
                    }
                    break;
                case "tool/result":
                    if (data.TryGetProperty("message", out var msg) &&
                        msg.ValueKind == JsonValueKind.Object &&
                        msg.TryGetProperty("source", out var src2) &&
                        src2.ValueKind == JsonValueKind.Object)
                    {
                        var rid = Str(src2, "callId");
                        if (rid.Length > 0 && st.PendingCalls.TryGetValue(rid, out var dispatched) && time > dispatched)
                        {
                            st.ToolMs += time - dispatched;
                        }
                        st.PendingCalls.Remove(rid);
                    }
                    break;
                case "step/end":
                    if (turn is not null)
                    {
                        if (st.LastTurn != turn)
                        {
                            st.Turns++;
                            st.LastTurn = turn;
                        }
                        st.Steps++;
                    }
                    st.OpenStepTurn = null;
                    break;
                default:
                    return; // 未跟踪事件不刷新条
            }
            UpdateRunStatsStrip();
        }
        catch (Exception) { } // 统计失败不致命：条保持上次值
    }

    /// <summary>usage 累计：totalTokens 缺失时按 input+output+cacheRead+cacheWrite 口径（与统计页一致）。</summary>
    private static void AccumulateUsage(RunStatsState st, JsonElement data)
    {
        if (!data.TryGetProperty("usage", out var u) || u.ValueKind != JsonValueKind.Object)
        {
            return;
        }
        var total = UsageField(data, "totalTokens");
        var input = UsageField(data, "inputTokens");
        var output = UsageField(data, "outputTokens");
        var cacheRead = UsageField(data, "cacheReadTokens");
        var cacheWrite = UsageField(data, "cacheWriteTokens");
        st.TotalTokens += total ?? (input ?? 0) + (output ?? 0) + (cacheRead ?? 0) + (cacheWrite ?? 0);
        st.CacheReadTokens += cacheRead ?? 0;
        st.InputTokens += input ?? 0;
        st.OutputTokens += output ?? 0;
    }

    /// <summary>首 token 时刻 = stream 块数组第一个带 time 的条目（官方 assistantStreamFirstTokenTime 的近似）。</summary>
    private static long? FirstTokenFromStream(JsonElement data)
    {
        if (!data.TryGetProperty("stream", out var stream) || stream.ValueKind != JsonValueKind.Array)
        {
            return null;
        }
        foreach (var block in stream.EnumerateArray())
        {
            if (block.ValueKind == JsonValueKind.Object && block.TryGetProperty("time", out var t) &&
                t.ValueKind == JsonValueKind.Number)
            {
                return (long)t.GetDouble();
            }
        }
        return null;
    }

    private static long? UsageField(JsonElement data, string name)
        => data.TryGetProperty("usage", out var u) && u.ValueKind == JsonValueKind.Object &&
           u.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number && v.GetDouble() >= 0
            ? (long)v.GetDouble()
            : null;

    private static string? NumToString(JsonElement data, string name)
        => data.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number
            ? v.GetDouble().ToString("0", System.Globalization.CultureInfo.InvariantCulture)
            : null;

    /// <summary>会话切换/回放重放前清零：状态由随后的完整 transcript 回放重建。</summary>
    private void ResetRunStats()
    {
        var sid = Volatile.Read(ref _activeSessionId);
        if (!string.IsNullOrEmpty(sid))
        {
            _runStats.Remove(sid);
        }
        UpdateRunStatsStrip();
    }

    /// <summary>刷新状态条（composer 上方）：当前会话有已完成步才显示。</summary>
    private void UpdateRunStatsStrip()
    {
        try
        {
            var sid = Volatile.Read(ref _activeSessionId);
            RunStatsState? st = null;
            if (!string.IsNullOrEmpty(sid))
            {
                _runStats.TryGetValue(sid, out st);
            }
            if (st is null || st.Steps <= 0)
            {
                RunStatsStrip.Visibility = Visibility.Collapsed;
                return;
            }
            RunStatsTurnsText.Text = LF("{0} 轮 {1} 步 · {2} tok/s", st.Turns, st.Steps, FormatTps(st));
            // 右胶囊显式拼接：单位与 % 各出现一次——LF 模板字面量 + 值内单位会叠加（%% / tok tok 事故）
            RunStatsTokensText.Text = FormatTokensCompact(st.TotalTokens) + " tok · " + L("缓存命中") + " " + FormatCacheHit(st) + "%";
            RunStatsStrip.Visibility = Visibility.Visible;
        }
        catch (Exception) { }
    }

    private string FormatTps(RunStatsState st)
        => st.DecodeMs > 0 && st.DecodeTokens > 0
            ? (st.DecodeTokens / (st.DecodeMs / 1000.0)).ToString("0", System.Globalization.CultureInfo.InvariantCulture)
            : "—";

    /// <summary>缓存命中数值（不含 % 号——LF 模板里已带字面 %，重复会显示 %%）。</summary>
    private string FormatCacheHit(RunStatsState st)
        => st.CacheReadTokens + st.InputTokens > 0
            ? string.Create(System.Globalization.CultureInfo.InvariantCulture, $"{100.0 * st.CacheReadTokens / (st.CacheReadTokens + st.InputTokens):0.#}")
            : "—";

    /// <summary>紧凑 token 数：1.2K / 6.2M（官方条样式），中文环境大数走 万/亿（与统计页同思路）。</summary>
    private string FormatTokensCompact(long value)
        => value >= 100_000_000 && _shellLocale.StartsWith("zh", StringComparison.OrdinalIgnoreCase) ? LF("{0:0.#} 亿", value / 100_000_000.0)
         : value >= 10_000 && _shellLocale.StartsWith("zh", StringComparison.OrdinalIgnoreCase) ? LF("{0:0.#} 万", value / 10_000.0)
         : value >= 1_000_000 ? string.Create(System.Globalization.CultureInfo.InvariantCulture, $"{value / 1_000_000.0:0.#}M")
         : value >= 1_000 ? string.Create(System.Globalization.CultureInfo.InvariantCulture, $"{value / 1_000.0:0.#}K")
         : value.ToString("N0");

    // ---------------- 两个可查看面板（胶囊 Tapped 弹出） ----------------

    /// <summary>胶囊点击接线（ctor 调一次）。</summary>
    private void WireRunStatsUi()
    {
        RunStatsTurnsText.Tapped += (_, _) => ShowStatsPanel((Microsoft.UI.Xaml.FrameworkElement)RunStatsTurnsText);
        RunStatsTokensText.Tapped += (_, _) => ShowTokenPanel((Microsoft.UI.Xaml.FrameworkElement)RunStatsTokensText);
        ContextMeterRing.Tapped += (_, _) => ShowContextPanel((Microsoft.UI.Xaml.FrameworkElement)ContextMeterRing);
    }

    private void ShowStatsPanel(Microsoft.UI.Xaml.FrameworkElement anchor)
    {
        try
        {
            var sid = Volatile.Read(ref _activeSessionId);
            _runStats.TryGetValue(sid ?? "", out var st);
            var panel = new StackPanel { Spacing = 10, MinWidth = 250 };
            panel.Children.Add(MakePanelHeader("会话统计"));
            panel.Children.Add(StatRow(L("模型用时"), FormatDuration(st?.LlmMs ?? 0)));
            panel.Children.Add(StatRow(L("工具调用用时"), FormatDuration(st?.ToolMs ?? 0)));
            panel.Children.Add(StatRow(L("首 token 平均（TTFT）"),
                st is { TtftSteps: > 0 } ? FormatDuration((double)st.TtftMs / st.TtftSteps) : "—"));
            panel.Children.Add(StatRow(L("输出速度（TPS）"), st is null ? "—" : FormatTps(st) + " tok/s"));
            ShowRunFlyout(anchor, panel);
        }
        catch (Exception) { }
    }

    private void ShowTokenPanel(Microsoft.UI.Xaml.FrameworkElement anchor)
    {
        try
        {
            var sid = Volatile.Read(ref _activeSessionId);
            _runStats.TryGetValue(sid ?? "", out var st);
            var panel = new StackPanel { Spacing = 10, MinWidth = 250 };
            // 头部：Token 用量 …… 总量（官方把总量放标题行右侧）
            var head = new Grid { ColumnSpacing = 16 };
            head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var hl = new TextBlock { Text = L("Token 用量") };
            var hv = new TextBlock
            {
                Text = (st?.TotalTokens ?? 0).ToString("N0", System.Globalization.CultureInfo.InvariantCulture) + " tok",
                HorizontalAlignment = HorizontalAlignment.Right,
            };
            Microsoft.UI.Xaml.Controls.Grid.SetColumn(hl, 0);
            Microsoft.UI.Xaml.Controls.Grid.SetColumn(hv, 1);
            head.Children.Add(hl);
            head.Children.Add(hv);
            panel.Children.Add(head);
            panel.Children.Add(MakePanelSeparator());
            panel.Children.Add(StatRow(L("缓存命中"), st is null ? "—" : FormatCacheHit(st) + "%"));
            panel.Children.Add(StatRow(L("未缓存输入"), TokensN0(st?.InputTokens ?? 0)));
            panel.Children.Add(StatRow(L("缓存读取"), TokensN0(st?.CacheReadTokens ?? 0)));
            panel.Children.Add(StatRow(L("输出"), TokensN0(st?.OutputTokens ?? 0)));
            ShowRunFlyout(anchor, panel);
        }
        catch (Exception) { }
    }

    private static string TokensN0(long v)
        => v.ToString("N0", System.Globalization.CultureInfo.InvariantCulture) + " tok";

    /// <summary>面板头行：加粗标签。随后接 1px 分隔线，与官方面板同构。</summary>
    private StackPanel MakePanelHeader(string label)
    {
        var sp = new StackPanel { Spacing = 8 };
        sp.Children.Add(new TextBlock { Text = label, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold });
        sp.Children.Add(MakePanelSeparator());
        return sp;
    }

    private Border MakePanelSeparator()
        => new()
        {
            Height = 1,
            Background = ThemeBrush("StrokeBrush") is SolidColorBrush b ? b : new SolidColorBrush(Microsoft.UI.Colors.Transparent),
        };

    /// <summary>label 在左、value 右对齐的一行。</summary>
    private static Grid StatRow(string label, string value)
    {
        var g = new Grid { ColumnSpacing = 16 };
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var l = new TextBlock { Text = label, Opacity = 0.72 };
        var v = new TextBlock { Text = value, HorizontalAlignment = HorizontalAlignment.Right };
        Microsoft.UI.Xaml.Controls.Grid.SetColumn(l, 0);
        Microsoft.UI.Xaml.Controls.Grid.SetColumn(v, 1);
        g.Children.Add(l);
        g.Children.Add(v);
        return g;
    }

    private void ShowRunFlyout(Microsoft.UI.Xaml.FrameworkElement anchor, StackPanel panel)
    {
        var host = new Border
        {
            Background = ThemeBrush("CardBrush") is SolidColorBrush cb
                ? cb
                : new SolidColorBrush(Microsoft.UI.Colors.White),
            BorderBrush = ThemeBrush("StrokeBrush") is SolidColorBrush sb ? sb : new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Microsoft.UI.Xaml.Thickness(1),
            CornerRadius = new Microsoft.UI.Xaml.CornerRadius(8),
            Padding = new Microsoft.UI.Xaml.Thickness(14, 12, 14, 12),
            Child = panel,
        };
        var flyout = new Flyout { Placement = FlyoutPlacementMode.Top, Content = host };
        flyout.ShowAt(anchor);
    }
}
