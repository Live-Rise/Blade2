using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Linq;
using System.Text.Json;
using System.Threading;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Blade2;

/// <summary>
/// 聊天页右侧历史快速定位（turn rail）：内核 dsh-session-turn-outline 的 turnOutline 投影
/// 提供全量轮次（seq + 首条用户 prompt 预览 + 最终答案预览），壳侧将其渲染成右侧一列
/// 圆点刻度（对齐官方 Web UI TurnNavigator 的最小同语义：hover 预览、点击跳转、
/// 活动轮次高亮），点击跳转滚动到该轮首条用户气泡。
///
/// 数据来源两处，与既有投影键（plan/permissions/schedule/goal）同一路径：
/// 1) session/list 的 projections.values["turnOutline"]（快照，RefreshSessionStateFromListAsync 补齐）；
/// 2) session/control 流 baseline.value.projections[sid].values + projection 增量帧。
/// wire 值即条目数组 [{turn:int, seq:int, prompt:string, response:string}]（projection wire.view 已裁剪）。
///
/// 壳一次拉全 journal（LoadSessionHistoryAsync），不存在官方「unloaded 刻度先翻页」的
/// 情况，因此刻度永远视作已加载直接跳转；compact 折叠视图下目标气泡可能被裁掉，
/// 此时回退到该轮的答案气泡。
/// </summary>
public partial class MainWindow
{
    /// <summary>当前会话的 rail 刻度集合（UI 线程独占，RebindTurnRail 整体替换内容）。</summary>
    private readonly ObservableCollection<TurnRailMark> _turnRailMarks = new();
    /// <summary>当前活动（视口最近）轮次号；0 = 未定（视口内无轮次气泡）。</summary>
    private int _turnRailActiveTurn;
    /// <summary>滚动跟踪节流：ViewChanged 每帧都触发，按内核官方 SCROLL_SAMPLE_INTERVAL_MS 采样。</summary>
    private long _turnRailLastSampleMs;
    /// <summary>ViewChanged 采样在 UI 线程上，同一 tick 内不重复投递。</summary>
    private bool _turnRailSampleQueued;
    /// <summary>rail 面板当前数据源会话：投影帧按会话过滤的依据。</summary>
    private string _turnRailSessionId = "";
    private bool _turnRailScrollHooked;

    private const int TurnRailSampleIntervalMs = 500;

    /// <summary>构造期一次性挂接 rail 事件与数据源。</summary>
    private void InitTurnRail()
    {
        TurnRailMarks.ItemsSource = _turnRailMarks;
        TurnRailMarks.PointerMoved += OnTurnRailPointerMoved;
        TurnRailMarks.PointerExited += OnTurnRailPointerExited;
        // 活动轮次跟踪：只钩一次（内部滚动面在首个非空列表布局后出现，
        // Loaded/SizeChanged 兜底——空会话首屏没有滚动面）。
        ChatList.Loaded += (_, _) => HookTurnRailScroll();
        ChatList.SizeChanged += (_, _) => HookTurnRailScroll();
    }

    private void HookTurnRailScroll()
    {
        if (_turnRailScrollHooked || TranscriptScroller(ChatList) is not { } sv)
        {
            return;
        }
        sv.ViewChanged += (_, _) => OnTurnRailViewChanged();
        _turnRailScrollHooked = true;
    }

    private void OnTurnRailViewChanged()
    {
        var now = Environment.TickCount64;
        if (_turnRailSampleQueued || now - _turnRailLastSampleMs < TurnRailSampleIntervalMs)
        {
            return;
        }
        _turnRailSampleQueued = true;
        _turnRailLastSampleMs = now;
        PostUi(() =>
        {
            _turnRailSampleQueued = false;
            UpdateTurnRailActiveFromViewport();
        });
    }

    /// <summary>
    /// 视口活动轮次 = 视口内与顶线相交的最后一条气泡的 Turn。
    /// ChatBubble.Turn 在 RenderJournal 时已回填，无需为 rail 再建索引。
    /// </summary>
    private void UpdateTurnRailActiveFromViewport()
    {
        if (_turnRailMarks.Count == 0)
        {
            return;
        }
        var turn = TopVisibleBubbleTurn();
        if (turn > 0 && turn != _turnRailActiveTurn)
        {
            _turnRailActiveTurn = turn;
            UpdateTurnRailStyles();
        }
    }

    /// <summary>视口内与顶线相交的最后一条可见气泡的轮次号；视口内没有轮次气泡返回 0。</summary>
    private int TopVisibleBubbleTurn()
    {
        if (TranscriptScroller(ChatList) is not { } sv)
        {
            return 0;
        }
        var top = sv.VerticalOffset + 8; // 8px 余量：行 margin 压线时不丢活动态
        var bottom = sv.VerticalOffset + sv.ViewportHeight;
        if (ChatList.ItemsPanelRoot is not { } panel)
        {
            return 0;
        }
        var found = 0;
        foreach (var container in panel.Children)
        {
            if (container is not ListViewItem lvi || lvi.Content is not ChatBubble bubble || bubble.Turn <= 0)
            {
                continue;
            }
            var t = lvi.TransformToVisual(sv).TransformPoint(new Windows.Foundation.Point(0, 0));
            if (t.Y + lvi.ActualHeight < top)
            {
                continue; // 完全在视口上方
            }
            if (t.Y > bottom)
            {
                break; // 视口之下（容器按序排列，可提前退出）
            }
            found = bubble.Turn; // 与视口相交：取靠下的那个
        }
        return found;
    }

    /// <summary>重绑 rail：快照/增量投影到达、切会话时调用。UI 线程。</summary>
    private void RebindTurnRail(IReadOnlyList<TurnRailMark>? marks, string sessionId)
    {
        _turnRailSessionId = sessionId;
        _turnRailActiveTurn = 0;
        TurnRailPreviewCard.Visibility = Visibility.Collapsed;
        _turnRailMarks.Clear();
        if (marks is not null)
        {
            foreach (var m in marks)
            {
                _turnRailMarks.Add(m);
            }
        }
        UpdateTurnRailVisibility();
        // 首帧活动态：历史回读后视口贴底，活动轮即最后一轮
        if (_turnRailMarks.Count > 0)
        {
            _turnRailActiveTurn = _turnRailMarks[^1].Turn;
        }
        UpdateTurnRailStyles();
    }

    private void UpdateTurnRailVisibility()
    {
        // 两条以上轮次才有定位价值（单轮会话一屏装得下）
        var target = _turnRailMarks.Count >= 2 ? Visibility.Visible : Visibility.Collapsed;
        if (TurnRailHost.Visibility != target)
        {
            TurnRailHost.Visibility = target;
        }
    }

    /// <summary>刻度样式刷新：活动轮长条，悬停轮次中等长度，其余短点。</summary>
    private void UpdateTurnRailStyles()
    {
        for (var i = 0; i < _turnRailMarks.Count; i++)
        {
            var m = _turnRailMarks[i];
            m.IsActive = m.Turn == _turnRailActiveTurn;
            m.MarkWidth = m.IsActive ? 20 : m.IsHovered ? 14 : 12;
            m.MarkOpacity = m.IsActive ? 1 : m.IsHovered ? 0.8 : 0.55;
        }
        // 让活动刻度留在 rail 可视区内（官方：active mark keeps itself in view）
        if (_turnRailActiveTurn > 0 && _turnRailMarks.FirstOrDefault(x => x.Turn == _turnRailActiveTurn) is { } active)
        {
            TurnRailMarks.ScrollIntoView(active, ScrollIntoViewAlignment.Default);
        }
    }

    // ---------------- 投影消费 ----------------

    /// <summary>解析 turnOutline wire 值（条目数组）。坏条目（缺 turn/seq）丢弃，
    /// 预览字段损坏降级为空串——与官方 outlineEntry() 同策略（轮次可导航优先于预览完整）。</summary>
    private static List<TurnRailMark> ParseTurnOutline(JsonElement value)
    {
        var list = new List<TurnRailMark>();
        if (value.ValueKind != JsonValueKind.Array)
        {
            return list;
        }
        foreach (var el in value.EnumerateArray())
        {
            if (el.ValueKind != JsonValueKind.Object)
            {
                continue;
            }
            if (!el.TryGetProperty("turn", out var t) || t.ValueKind != JsonValueKind.Number ||
                !t.TryGetInt32(out var turn) || turn <= 0)
            {
                continue;
            }
            if (!el.TryGetProperty("seq", out var s) || s.ValueKind != JsonValueKind.Number ||
                !s.TryGetInt32(out var seq) || seq < 0)
            {
                continue;
            }
            var prompt = el.TryGetProperty("prompt", out var p) && p.ValueKind == JsonValueKind.String ? p.GetString() ?? "" : "";
            var response = el.TryGetProperty("response", out var r) && r.ValueKind == JsonValueKind.String ? r.GetString() ?? "" : "";
            list.Add(new TurnRailMark { Turn = turn, Seq = seq, Prompt = prompt, Response = response });
        }
        // 内核保证按 turn 严格递增（schema superRefine），防御性排序防跨版本差异
        list.Sort((a, b) => a.Turn.CompareTo(b.Turn));
        return list;
    }

    /// <summary>快照路径（session/list 缓存补齐 / session/control baseline）拿到投影块时调用。接收线程。</summary>
    private void ApplyTurnOutlineSnapshot(JsonElement values, string sessionId)
    {
        if (values.ValueKind != JsonValueKind.Object ||
            !values.TryGetProperty("turnOutline", out var to))
        {
            return;
        }
        ApplyTurnOutlineValue(to, sessionId);
    }

    /// <summary>projection 增量帧的 turnOutline 分支。接收线程。</summary>
    private void ApplyTurnOutlineFrame(JsonElement value, string sessionId) => ApplyTurnOutlineValue(value, sessionId);

    private void ApplyTurnOutlineValue(JsonElement value, string sessionId)
    {
        var marks = ParseTurnOutline(value);
        if (Volatile.Read(ref _activeSessionId) != sessionId)
        {
            return;
        }
        PostUi(() =>
        {
            if (Volatile.Read(ref _activeSessionId) != sessionId)
            {
                return; // 会话已切换，丢弃过期投影
            }
            RebindTurnRail(marks, sessionId);
        });
    }

    // ---------------- 悬停预览 ----------------

    private void OnTurnRailPointerMoved(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        // 指针位置 → 刻度项：命中测试取 DataContext（官方 itemAtPointer 同语义）
        var point = e.GetCurrentPoint(TurnRailMarks).Position;
        var hit = Microsoft.UI.Xaml.Media.VisualTreeHelper.FindElementsInHostCoordinates(point, TurnRailMarks);
        foreach (var el in hit)
        {
            if (el is FrameworkElement fe && fe.DataContext is TurnRailMark mark)
            {
                ShowTurnRailPreview(mark);
                return;
            }
        }
    }

    private void OnTurnRailPointerExited(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        TurnRailPreviewCard.Visibility = Visibility.Collapsed;
        foreach (var m in _turnRailMarks)
        {
            m.IsHovered = false;
        }
        UpdateTurnRailStyles();
    }

    private void ShowTurnRailPreview(TurnRailMark mark)
    {
        foreach (var m in _turnRailMarks)
        {
            m.IsHovered = m == mark;
        }
        UpdateTurnRailStyles();

        // 官方预览卡：prompt 为主行（无 prompt 时回落轮次号），response 为副行
        TurnRailPreviewPrompt.Text = mark.Prompt.Length > 0 ? mark.Prompt : LF("第 {0} 轮", mark.Turn);
        if (mark.Response.Length > 0)
        {
            TurnRailPreviewResponse.Text = mark.Response;
            TurnRailPreviewResponse.Visibility = Visibility.Visible;
        }
        else
        {
            TurnRailPreviewResponse.Visibility = Visibility.Collapsed;
        }

        // 预览卡贴刻度左侧放置，纵向跟随刻度中心
        if (TurnRailMarks.ContainerFromItem(mark) is ListViewItem container)
        {
            var host = TurnRailHost;
            var pos = container.TransformToVisual(host).TransformPoint(new Windows.Foundation.Point(0, 0));
            var y = Math.Max(0, pos.Y + container.ActualHeight / 2 - 24);
            y = Math.Min(y, Math.Max(0, host.ActualHeight - 120));
            TurnRailPreviewCard.Margin = new Thickness(0, y, 0, 0);
        }
        TurnRailPreviewCard.Visibility = Visibility.Visible;
    }

    // ---------------- 点击跳转 ----------------

    private void OnTurnRailMarkClick(object sender, Microsoft.UI.Xaml.Input.TappedRoutedEventArgs e)
    {
        if (sender is not FrameworkElement fe || fe.DataContext is not TurnRailMark mark)
        {
            return;
        }
        e.Handled = true;
        TurnRailPreviewCard.Visibility = Visibility.Collapsed;

        // 目标：该轮首条用户气泡（turn 起点）；compact 视图可能折叠掉——回退答案气泡
        var target = _visibleMessages.FirstOrDefault(b => b.Turn == mark.Turn && b.Role is "user" or "user-image");
        target ??= _transcriptAnswers.TryGetValue(mark.Turn, out var answer) && _visibleMessages.Contains(answer)
            ? answer
            : null;
        target ??= _visibleMessages.FirstOrDefault(b => b.Turn == mark.Turn);
        if (target is null)
        {
            return;
        }

        _turnRailActiveTurn = mark.Turn;
        UpdateTurnRailStyles();

        // 对齐官方「loaded 刻度滚动」语义：目标行顶到视口上沿
        ChatList.ScrollIntoView(target, ScrollIntoViewAlignment.Leading);
    }

    /// <summary>切会话时复位 rail（旧会话刻度不得残留到新会话）。</summary>
    private void ResetTurnRail()
    {
        RebindTurnRail(null, "");
    }
}

/// <summary>rail 刻度项：turnOutline 投影的一个条目（UI 绑定对象）。</summary>
/// <remarks>XAML DataTemplate 以 local:TurnRailMark 绑定，必须是命名空间顶层类型。</remarks>
public sealed class TurnRailMark : INotifyPropertyChanged
{
    public int Turn { get; init; }
    /// <summary>turn/start 的 journal seq：跳转锚点（内核承诺翻回该 seq 的页必含整轮）。</summary>
    public int Seq { get; init; }
    public string Prompt { get; init; } = "";
    public string Response { get; init; } = "";

    private bool _isActive;
    public bool IsActive { get => _isActive; set { if (_isActive != value) { _isActive = value; Changed(nameof(IsActive)); } } }

    private bool _isHovered;
    public bool IsHovered { get => _isHovered; set { if (_isHovered != value) { _isHovered = value; Changed(nameof(IsHovered)); } } }

    /// <summary>刻度宽度（活动态更长），UI 线程由 UpdateTurnRailStyles 赋值。</summary>
    public double MarkWidth { get => _markWidth; set { if (Math.Abs(_markWidth - value) > 0.01) { _markWidth = value; Changed(nameof(MarkWidth)); } } }
    private double _markWidth = 12;

    /// <summary>刻度不透明度：活动轮全显、悬停中等、常态弱化（官方 mark/markActive 三态）。</summary>
    public double MarkOpacity { get => _markOpacity; set { if (Math.Abs(_markOpacity - value) > 0.01) { _markOpacity = value; Changed(nameof(MarkOpacity)); } } }
    private double _markOpacity = 0.55;

    public event PropertyChangedEventHandler? PropertyChanged;
    private void Changed(string name) => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}
