using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Blade2;

/// <summary>
/// 轨迹 Trajectory 计时总览面板（对标官方 @deepseek-ai/dsh-client-ui-trajectory）：
/// 事件账本（journal 事件按 seq 列出、类型筛选、展开只读 JSON）+ 交互计时总览（轮次分段、
/// steps、工具耗时、llm/ttft/tps/缓存命中）+ 轮次导航（点击跳到对应消息）+ Goal 轮标记
/// 「目标 · Round {round}」（P2-1）。
/// 数据入口 = RenderEventCore 的 TrajectoryObserve 唯一钩子（page 回放 + follow 实时流共用）；
/// 计时折叠镜像 RunStats 的 fold 算法但按轮分段（本文件内自持，不改 MainWindow.RunStats.cs）。
/// 与 TurnRail 的边界：TurnRail 是壳增强的轮次刻度轨（turnOutline 投影），本面板是官方
/// trajectory 的事件账本 + 计时域，二者并行不互相替代。
/// UI 入口：BuildCapabilityMenu「轨迹」+ 输入 + 菜单「轨迹」（代码挂接，不改 MainWindow.xaml）。
/// </summary>
public partial class MainWindow
{
    // ---------------- 账本与计时状态（仅 UI 线程访问） ----------------

    /// <summary>一次工具调用的计时跨度与只读 JSON 载荷。</summary>
    private sealed class TrajectoryToolSpan
    {
        public string CallId = "";
        public string Name = "";
        public long StartMs;
        public long? EndMs;
        public bool IsError;
        public string? ArgsJson;
        public string? ResultJson;
        public string? DiffPath;
        public string? DiffOld;
        public string? DiffNew;
    }

    /// <summary>单轮计时折叠（RunStats fold 的按轮分段版）。</summary>
    private sealed class TrajectoryTurnTiming
    {
        public int Turn;
        public long StartMs;
        public long StartSeq;
        public long? EndMs;
        public int Steps;
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
        public string? OpenStepKey;
        public long OpenStepStartMs;
        public long? FirstTokenMs;
        public string? GoalRoundLabel;
        public string PromptPreview = "";
        public string Model = "";
        public readonly Dictionary<string, long> PendingCalls = new(StringComparer.Ordinal);
        public readonly List<TrajectoryToolSpan> Tools = new();
    }

    /// <summary>一条账本行（journal 事件的只读投影）。</summary>
    private sealed class TrajectoryEntry
    {
        public int Seq;
        public long Time;
        public string Type = "";
        public int Turn;
        /// <summary>筛选桶：user / assistant / tool / step / system / other。</summary>
        public string Kind = "other";
        public string Label = "";
        public string SourceLabel = "";
        public string? PayloadJson;
        public string? ResultJson;
        public string? DiffPath;
        public string? DiffOld;
        public string? DiffNew;
        public string? CallId;
    }

    /// <summary>单会话轨迹账本 + 轮次计时。</summary>
    private sealed class TrajectoryState
    {
        public readonly List<TrajectoryEntry> Entries = new();
        public readonly SortedDictionary<int, TrajectoryTurnTiming> Turns = new();
        public readonly HashSet<int> SeenSeqs = new();
    }

    private readonly Dictionary<string, TrajectoryState> _trajectories = new(StringComparer.Ordinal);
    private bool _trajectoryEntryWired;

    private TrajectoryState GetOrCreateTrajectory(string sid)
    {
        if (!_trajectories.TryGetValue(sid, out var st))
        {
            st = new TrajectoryState();
            _trajectories[sid] = st;
            // 会话数上限：只保留最近若干会话，避免长时间运行无限增长
            if (_trajectories.Count > 8)
            {
                foreach (var old in _trajectories.Keys.Where(k => k != sid).Take(_trajectories.Count - 8).ToList())
                {
                    _trajectories.Remove(old);
                }
            }
        }
        return st;
    }

    private TrajectoryTurnTiming GetOrCreateTrajectoryTurn(TrajectoryState st, int turn)
    {
        if (!st.Turns.TryGetValue(turn, out var tr))
        {
            tr = new TrajectoryTurnTiming { Turn = turn };
            st.Turns[turn] = tr;
        }
        return tr;
    }

    // ---------------- L 文案（中文键 + ShellEnglish 回落 + 本域 en 兜底） ----------------

    /// <summary>本域文案：优先 L()/ShellEnglish；非中文且字典未收录时用 en 兜底（不改 ShellEnglish 表）。</summary>
    private string TrajText(string zh, string en)
    {
        var s = L(zh);
        if (!string.Equals(s, zh, StringComparison.Ordinal)) return s;
        return _shellLocale.StartsWith("zh", StringComparison.OrdinalIgnoreCase) ? zh : en;
    }

    private string TrajFormat(string zhTemplate, string enTemplate, params object?[] args)
    {
        try { return string.Format(TrajText(zhTemplate, enTemplate), args); }
        catch (FormatException) { return zhTemplate; }
    }

    // ---------------- journal 事件折叠入口（RenderEventCore 唯一钩子） ----------------

    /// <summary>journal 事件 → 轨迹账本 + 轮次计时。UI 线程调用（RenderEventCore 保证）。</summary>
    private void TrajectoryObserve(JsonElement ev, string? type, JsonElement data)
    {
        try
        {
            if (string.IsNullOrEmpty(type)) return;
            var sid = Volatile.Read(ref _activeSessionId);
            if (string.IsNullOrEmpty(sid)) return;
            var st = GetOrCreateTrajectory(sid);
            var envSeq = ev.TryGetProperty("seq", out var seqEl) && seqEl.ValueKind == JsonValueKind.Number && seqEl.TryGetInt32(out var s) ? s : 0;
            var envTime = ev.TryGetProperty("time", out var timeEl) && timeEl.ValueKind == JsonValueKind.Number ? (long)timeEl.GetDouble() : 0L;
            var turn = data.ValueKind == JsonValueKind.Object && data.TryGetProperty("turn", out var turnValue) &&
                turnValue.TryGetInt32(out var number) ? number : 0;

            // 会话重开会整页回放：按 seq 去重，避免账本/计时翻倍（与 journal 游标互补）
            if (envSeq > 0 && !st.SeenSeqs.Add(envSeq)) return;

            FoldTrajectoryTiming(st, type, data, envTime, turn, envSeq);
            st.Entries.Add(BuildTrajectoryEntry(type, data, envSeq, envTime, turn));
            if (st.Entries.Count > 4000)
            {
                st.Entries.RemoveRange(0, st.Entries.Count - 4000);
            }
            EnsureTrajectoryUiEntry();
        }
        catch (Exception) { } // 轨迹折叠失败不致命
    }

    /// <summary>会话切换时丢弃非当前会话的账本（当前会话靠 seq 去重可安全重放）。</summary>
    private void TrimTrajectorySessions()
    {
        var sid = Volatile.Read(ref _activeSessionId);
        if (string.IsNullOrEmpty(sid)) return;
        foreach (var old in _trajectories.Keys.Where(k => k != sid).ToList())
        {
            _trajectories.Remove(old);
        }
    }

    // ---------------- 计时折叠（镜像 RunStats fold，按轮分段） ----------------

    private void FoldTrajectoryTiming(
        TrajectoryState st, string type, JsonElement data, long time, int turn, int seq)
    {
        if (data.ValueKind != JsonValueKind.Object) return;
        // request/header 等无 turn 事件挂到最近一次已见轮（与气泡认领同序）
        if (turn <= 0)
        {
            turn = st.Turns.Count > 0 ? st.Turns.Keys.Max() : 0;
        }
        if (turn <= 0 && type is "user/message" or "system/message" or "request/header")
        {
            // 首条消息/系统提示词可能先于 turn/start：合成轮次号，后续 turn/start 同号合并
            turn = st.Turns.Count > 0 ? st.Turns.Keys.Max() + 1 : 1;
        }
        if (turn <= 0) return;
        var tr = GetOrCreateTrajectoryTurn(st, turn);

        switch (type)
        {
            case "user/message":
                if (tr.StartMs <= 0 && time > 0) { tr.StartMs = time; tr.StartSeq = seq; }
                if (tr.PromptPreview.Length == 0)
                {
                    tr.PromptPreview = TrajectoryPreview(data);
                }
                ApplyGoalRound(tr, data);
                break;
            case "turn/start":
                if (time > 0) { tr.StartMs = time; tr.StartSeq = seq; }
                tr.EndMs = null;
                break;
            case "turn/end":
                if (time > 0) tr.EndMs = time;
                break;
            case "step/start":
                tr.OpenStepKey = turn.ToString(CultureInfo.InvariantCulture);
                tr.OpenStepStartMs = time;
                tr.FirstTokenMs = null;
                break;
            case "assistant/attempt":
                if (tr.FirstTokenMs is null)
                {
                    tr.FirstTokenMs = FirstTokenFromStream(data);
                }
                break;
            case "assistant/message":
                if (tr.OpenStepKey is not null)
                {
                    if (time > tr.OpenStepStartMs && tr.OpenStepStartMs > 0)
                    {
                        tr.LlmMs += time - tr.OpenStepStartMs;
                    }
                    var first = tr.FirstTokenMs ?? FirstTokenFromStream(data);
                    var output = UsageField(data, "outputTokens");
                    if (first is long f)
                    {
                        if (f > tr.OpenStepStartMs && tr.OpenStepStartMs > 0)
                        {
                            tr.TtftMs += f - tr.OpenStepStartMs;
                            tr.TtftSteps++;
                        }
                        if (output is long o && time > f)
                        {
                            tr.DecodeMs += time - f;
                            tr.DecodeTokens += o;
                        }
                    }
                    tr.OpenStepKey = null;
                }
                TrajectoryAccumulateUsage(tr, data);
                break;
            case "request/header":
                if (data.TryGetProperty("header", out var hdr) && hdr.ValueKind == JsonValueKind.Object &&
                    hdr.TryGetProperty("config", out var cfg) && cfg.ValueKind == JsonValueKind.Object)
                {
                    var model = Str(cfg, "model");
                    if (model.Length > 0) tr.Model = model;
                }
                break;
            case "tool/call":
                {
                    var callId = Str(data, "callId");
                    var name = Str(data, "name");
                    var argsRaw = Str(data, "arguments");
                    if (callId.Length > 0 && time > 0) tr.PendingCalls[callId] = time;
                    var span = new TrajectoryToolSpan
                    {
                        CallId = callId,
                        Name = name,
                        StartMs = time,
                        ArgsJson = PrettyJson(argsRaw),
                    };
                    ExtractDiff(span, name, argsRaw);
                    tr.Tools.Add(span);
                }
                break;
            case "tool/result":
                {
                    long? dispatched = null;
                    var rid = "";
                    if (data.TryGetProperty("message", out var msg) && msg.ValueKind == JsonValueKind.Object)
                    {
                        if (msg.TryGetProperty("source", out var src) && src.ValueKind == JsonValueKind.Object)
                        {
                            rid = Str(src, "callId");
                        }
                        if (rid.Length > 0 && tr.PendingCalls.TryGetValue(rid, out var d) && time > d)
                        {
                            tr.ToolMs += time - d;
                            dispatched = d;
                        }
                        if (rid.Length > 0) tr.PendingCalls.Remove(rid);
                        var isError = false;
                        if (msg.TryGetProperty("content", out var rc) && rc.ValueKind == JsonValueKind.Array)
                        {
                            foreach (var b in rc.EnumerateArray())
                            {
                                if (b.TryGetProperty("isError", out var ie) && ie.ValueKind == JsonValueKind.True) isError = true;
                                break;
                            }
                        }
                        var resultJson = TrajectoryPrettyJsonElement(msg);
                        var span = rid.Length > 0 ? tr.Tools.LastOrDefault(t => t.CallId == rid) : null;
                        if (span is not null)
                        {
                            span.EndMs = time;
                            span.IsError = isError;
                            span.ResultJson = resultJson;
                        }
                        else
                        {
                            tr.Tools.Add(new TrajectoryToolSpan
                            {
                                CallId = rid,
                                Name = L("工具"),
                                StartMs = dispatched ?? time,
                                EndMs = time,
                                IsError = isError,
                                ResultJson = resultJson,
                            });
                        }
                    }
                }
                break;
            case "step/end":
                tr.Steps++;
                tr.OpenStepKey = null;
                break;
            case "system/message":
                tr.SystemPromptSeen = true;
                break;
        }
    }

    /// <summary>usage 累计（RunStats AccumulateUsage 的按轮版，totalTokens 缺失按四段口径）。</summary>
    private static void TrajectoryAccumulateUsage(TrajectoryTurnTiming tr, JsonElement data)
    {
        if (!data.TryGetProperty("usage", out var u) || u.ValueKind != JsonValueKind.Object) return;
        var total = UsageField(data, "totalTokens");
        var input = UsageField(data, "inputTokens");
        var output = UsageField(data, "outputTokens");
        var cacheRead = UsageField(data, "cacheReadTokens");
        var cacheWrite = UsageField(data, "cacheWriteTokens");
        tr.TotalTokens += total ?? (input ?? 0) + (output ?? 0) + (cacheRead ?? 0) + (cacheWrite ?? 0);
        tr.CacheReadTokens += cacheRead ?? 0;
        tr.InputTokens += input ?? 0;
        tr.OutputTokens += output ?? 0;
    }

    /// <summary>P2-1：user/message 的 source.kind==goal → 「目标 · Round {round}」。</summary>
    private void ApplyGoalRound(TrajectoryTurnTiming tr, JsonElement data)
    {
        if (!data.TryGetProperty("source", out var src) || src.ValueKind != JsonValueKind.Object) return;
        if (Str(src, "kind") != "goal") return;
        var round = src.TryGetProperty("round", out var rv) && rv.ValueKind == JsonValueKind.Number ? rv.GetInt32() : 0;
        tr.GoalRoundLabel = round > 0
            ? TrajFormat("目标 · Round {0}", "Goal · Round {0}", round)
            : TrajText("目标", "Goal");
    }

    // ---------------- 账本行投影 ----------------

    private TrajectoryEntry BuildTrajectoryEntry(string type, JsonElement data, int seq, long time, int turn)
    {
        var e = new TrajectoryEntry
        {
            Seq = seq,
            Time = time,
            Type = type,
            Turn = turn,
            Kind = TrajectoryKindOf(type),
            Label = TrajectoryLabel(type, data),
            SourceLabel = TrajectorySourceLabel(data),
            PayloadJson = TrajectoryPayloadJson(type, data),
            ResultJson = TrajectoryResultJson(type, data),
        };
        if (type == "tool/call" && data.ValueKind == JsonValueKind.Object)
        {
            e.CallId = Str(data, "callId");
            var argsRaw = Str(data, "arguments");
            var probe = new TrajectoryToolSpan { Name = Str(data, "name"), ArgsJson = PrettyJson(argsRaw) };
            ExtractDiff(probe, probe.Name, argsRaw);
            e.DiffPath = probe.DiffPath;
            e.DiffOld = probe.DiffOld;
            e.DiffNew = probe.DiffNew;
            if (e.PayloadJson is null) e.PayloadJson = probe.ArgsJson;
        }
        return e;
    }

    private static string TrajectoryKindOf(string type) => type switch
    {
        "user/message" => "user",
        "assistant/message" or "assistant/attempt" or "assistant/live-chunk" => "assistant",
        "tool/call" or "tool/result" or "deliverables/presented" => "tool",
        "turn/start" or "turn/end" or "step/start" or "step/end" => "step",
        "system/message" or "request/header" or "request/context" or "session/title" => "system",
        _ => "other",
    };

    private string TrajectoryLabel(string type, JsonElement data)
    {
        switch (type)
        {
            case "user/message":
                return TrajectorySourceLabel(data) is { Length: > 0 } src && src != TrajText("用户", "User")
                    ? src
                    : TrajText("用户", "User");
            case "assistant/message":
                {
                    var tokens = UsageField(data, "totalTokens") ?? 0L;
                    return tokens > 0
                        ? TrajFormat("助手 · {0} tok", "Assistant · {0} tok", tokens)
                        : TrajText("助手", "Assistant");
                }
            case "assistant/attempt":
                return TrajText("助手流式尝试", "Assistant attempt");
            case "tool/call":
                return TrajFormat("工具 · {0}", "Tool · {0}", Str(data, "name"));
            case "tool/result":
                {
                    var name = "";
                    if (data.TryGetProperty("message", out var m) && m.ValueKind == JsonValueKind.Object &&
                        m.TryGetProperty("source", out var s) && s.ValueKind == JsonValueKind.Object)
                    {
                        name = Str(s, "callId");
                    }
                    return name.Length > 0
                        ? TrajFormat("工具结果 · {0}", "Tool result · {0}", name)
                        : TrajText("工具结果", "Tool result");
                }
            case "turn/start":
                return TrajFormat("轮次开始 · 第 {0} 轮", "Turn start · Turn {0}", TurnOrDash(data));
            case "turn/end":
                return TrajFormat("轮次结束 · 第 {0} 轮", "Turn end · Turn {0}", TurnOrDash(data));
            case "step/start":
                return TrajFormat("步骤开始 · 第 {0} 轮", "Step start · Turn {0}", TurnOrDash(data));
            case "step/end":
                return TrajFormat("步骤结束 · 第 {0} 轮", "Step end · Turn {0}", TurnOrDash(data));
            case "system/message":
                return TrajText("系统提示词", "System prompt");
            case "request/header":
                {
                    var model = "";
                    if (data.TryGetProperty("header", out var h) && h.ValueKind == JsonValueKind.Object &&
                        h.TryGetProperty("config", out var c) && c.ValueKind == JsonValueKind.Object)
                    {
                        model = Str(c, "model");
                    }
                    return model.Length > 0
                        ? TrajFormat("请求头 · {0}", "Request header · {0}", model)
                        : TrajText("请求头", "Request header");
                }
            case "request/context":
                return TrajText("上下文窗口", "Context window");
            case "session/title":
                return TrajText("会话标题", "Session title");
            case "deliverables/presented":
                return TrajText("交付物", "Deliverables");
            default:
                return type;
        }
    }

    private string TurnOrDash(JsonElement data)
        => data.ValueKind == JsonValueKind.Object && data.TryGetProperty("turn", out var t) && t.ValueKind == JsonValueKind.Number
            ? t.GetInt32().ToString(CultureInfo.InvariantCulture)
            : "—";

    private string TrajectorySourceLabel(JsonElement data)
    {
        if (data.ValueKind != JsonValueKind.Object ||
            !data.TryGetProperty("source", out var src) || src.ValueKind != JsonValueKind.Object)
        {
            return "";
        }
        var kind = Str(src, "kind");
        switch (kind)
        {
            case "user":
                return TrajText("用户", "User");
            case "goal":
                {
                    var round = src.TryGetProperty("round", out var rv) && rv.ValueKind == JsonValueKind.Number ? rv.GetInt32() : 0;
                    return round > 0
                        ? TrajFormat("目标 · Round {0}", "Goal · Round {0}", round)
                        : TrajText("目标", "Goal");
                }
            case "plugin":
                {
                    var plugin = Str(src, "plugin");
                    return plugin.Length > 0
                        ? TrajFormat("插件 · {0}", "Plugin · {0}", plugin)
                        : TrajText("插件", "Plugin");
                }
            case { Length: > 0 }:
                return kind;
            default:
                return TrajText("未知来源", "Unknown source");
        }
    }

    private string TrajectoryPayloadJson(string type, JsonElement data)
    {
        if (data.ValueKind != JsonValueKind.Object) return "";
        switch (type)
        {
            case "tool/call":
                {
                    var args = Str(data, "arguments");
                    return args.Length > 0 ? PrettyJson(args) : TrajText("未捕获参数", "No payload captured");
                }
            case "user/message":
                return data.TryGetProperty("source", out var src) && src.ValueKind == JsonValueKind.Object
                    ? TrajectoryPrettyJsonElement(src)
                    : TrajectoryPrettyJsonElement(data);
            case "request/header":
                return TrajectoryPrettyJsonElement(data);
            case "assistant/message":
                return data.TryGetProperty("message", out var msg) && msg.ValueKind == JsonValueKind.Object
                    ? TrajectoryPrettyJsonElement(msg)
                    : TrajectoryPrettyJsonElement(data);
            default:
                return TrajectoryPrettyJsonElement(data);
        }
    }

    private string TrajectoryResultJson(string type, JsonElement data)
    {
        if (data.ValueKind != JsonValueKind.Object) return "";
        switch (type)
        {
            case "tool/result":
                return data.TryGetProperty("message", out var msg) && msg.ValueKind == JsonValueKind.Object
                    ? TrajectoryPrettyJsonElement(msg)
                    : TrajText("未捕获结果", "No result captured");
            case "assistant/message":
                return data.TryGetProperty("usage", out var u) && u.ValueKind == JsonValueKind.Object
                    ? TrajectoryPrettyJsonElement(u)
                    : "";
            default:
                return "";
        }
    }

    /// <summary>edit / str_replace_editor / write 的简易差异（只读，展开行展示）。</summary>
    private static void ExtractDiff(TrajectoryToolSpan span, string name, string argsRaw)
    {
        if (string.IsNullOrEmpty(argsRaw)) return;
        JsonElement args;
        try
        {
            using var doc = JsonDocument.Parse(argsRaw);
            args = doc.RootElement.Clone();
        }
        catch (JsonException) { return; }
        if (args.ValueKind != JsonValueKind.Object) return;
        string? path = null, oldText = null, newText = null;
        if (name == "edit")
        {
            path = PickString(args, "file_path") ?? PickString(args, "path");
            oldText = PickString(args, "old_string");
            newText = PickString(args, "new_string");
        }
        else if (name == "str_replace_editor")
        {
            path = PickString(args, "path") ?? PickString(args, "file_path");
            oldText = PickString(args, "old_str");
            newText = PickString(args, "new_str");
        }
        else if (name == "write")
        {
            path = PickString(args, "file_path") ?? PickString(args, "path");
            newText = PickString(args, "content");
        }
        if (oldText is null && newText is null) return;
        span.DiffPath = path ?? "";
        span.DiffOld = oldText;
        span.DiffNew = newText;
    }

    private string TrajectoryPreview(JsonElement data)
    {
        if (data.ValueKind != JsonValueKind.Object ||
            !data.TryGetProperty("content", out var content) || content.ValueKind != JsonValueKind.Array)
        {
            return "";
        }
        var text = string.Join("\n", content.EnumerateArray()
            .Where(c => Str(c, "type") == "text")
            .Select(c => Str(c, "text")));
        var flat = text.Replace('\n', ' ').Trim();
        return flat.Length > 80 ? flat[..80] + "…" : flat;
    }

    private static string TrajectoryPrettyJsonElement(JsonElement el)
    {
        try
        {
            return JsonSerializer.Serialize(el, new JsonSerializerOptions { WriteIndented = true });
        }
        catch (Exception) { return el.ToString(); }
    }

    // ---------------- UI 入口挂接（代码构建，不改 MainWindow.xaml） ----------------

    /// <summary>首次事件到达时把「轨迹」挂进输入 + 菜单（与 BuildCapabilityMenu 并行的可见入口）。</summary>
    private void EnsureTrajectoryUiEntry()
    {
        if (_trajectoryEntryWired) return;
        try
        {
            var item = new MenuFlyoutItem { Text = L("轨迹") };
            // 菜单关闭动画期间 ShowAsync 会静默失败：编队到下一 UI 帧再开。
            item.Click += (_, _) => DispatcherQueue.TryEnqueue(async () =>
            {
                if (_capabilityPanelOpen) return;
                _capabilityPanelOpen = true;
                try { await ShowCapabilityTrajectoryAsync(); }
                catch (Exception ex)
                {
                    try { await ShowErrorAsync(ex.Message); }
                    catch (Exception) { }
                }
                finally { _capabilityPanelOpen = false; }
            });
            Aut(item, "TrajectoryMenuItem", L("轨迹"));
            // 插在「会话反馈」前（分隔符之后最后一项之前）
            var items = ComposerAddFlyout.Items;
            var at = items.Count;
            for (var i = 0; i < items.Count; i++)
            {
                if (items[i] is MenuFlyoutItem mi && mi.Text == L("会话反馈"))
                {
                    at = i;
                    break;
                }
            }
            items.Insert(at, item);
            _trajectoryEntryWired = true;
        }
        catch (Exception) { }
    }

    // ---------------- 面板 UI（仿 Capabilities 的代码构建 Dialog） ----------------

    private async Task ShowCapabilityTrajectoryAsync()
    {
        TrimTrajectorySessions();
        var sessionId = CapabilitySession();
        var sid = sessionId;
        _trajectories.TryGetValue(sid, out var state);
        state ??= new TrajectoryState();

        var root = Aut(new StackPanel { Spacing = Sp10, Width = 560 }, "TrajectoryPanel", TrajText("轨迹面板", "Trajectory panel"));
        var filter = Aut(new ComboBox
        {
            MinWidth = 160,
            HorizontalAlignment = HorizontalAlignment.Left,
            Header = TrajText("类型筛选", "Kind filter"),
        }, "TrajectoryKindFilter", TrajText("类型筛选", "Kind filter"));
        var filters = new (string Key, string Zh, string En)[]
        {
            ("all", "全部", "All"),
            ("user", "用户", "User"),
            ("assistant", "助手", "Assistant"),
            ("tool", "工具", "Tool"),
            ("step", "步骤", "Step"),
            ("system", "系统", "System"),
            ("other", "其他", "Other"),
        };
        foreach (var f in filters)
        {
            filter.Items.Add(new ComboBoxItem { Content = TrajText(f.Zh, f.En), Tag = f.Key });
        }
        filter.SelectedIndex = 0;

        var timingHost = Aut(new StackPanel { Spacing = Sp6 }, "TrajectoryTiming", TrajText("计时总览", "Timing overview"));
        var ledgerHost = Aut(new StackPanel { Spacing = Sp4 }, "TrajectoryLedger", TrajText("事件账本", "Event ledger"));
        var status = new TextBlock
        {
            TextWrapping = TextWrapping.Wrap,
            IsTextSelectionEnabled = true,
            Style = AppStyle("CaptionTextStyle"),
            Foreground = ThemeBrush("TextTertiaryBrush"),
        };

        root.Children.Add(new TextBlock
        {
            Text = TrajText(
                "事件账本与交互计时总览（数据来自 session journal 渲染管线）。点击轮次可跳到对应消息；展开行只读查看参数/结果/差异 JSON。",
                "Event ledger and interaction timing overview (from the session journal pipeline). Click a turn to jump to its message; expand a row for read-only payload/result/diff JSON."),
            TextWrapping = TextWrapping.Wrap,
        });
        root.Children.Add(filter);
        root.Children.Add(new TextBlock
        {
            Text = TrajText("计时总览", "Timing overview"),
            Style = AppStyle("BodyStrongTextStyle"),
        });
        root.Children.Add(Aut(new ScrollViewer
        {
            Content = timingHost,
            MaxHeight = 180,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        }, "TrajectoryTimingScroll", TrajText("计时总览", "Timing overview")));
        root.Children.Add(new TextBlock
        {
            Text = TrajText("事件账本", "Event ledger"),
            Style = AppStyle("BodyStrongTextStyle"),
        });
        root.Children.Add(Aut(new ScrollViewer
        {
            Content = ledgerHost,
            MaxHeight = 300,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        }, "TrajectoryLedgerScroll", TrajText("事件账本", "Event ledger")));
        root.Children.Add(status);

        var dialog = CapabilityDialog(TrajText("轨迹", "Trajectory"), root);
        var closed = false;

        void RebuildTiming()
        {
            timingHost.Children.Clear();
            if (state.Turns.Count == 0)
            {
                timingHost.Children.Add(new TextBlock
                {
                    Text = TrajText("暂无轮次计时。", "No turn timing yet."),
                    TextWrapping = TextWrapping.Wrap,
                });
                return;
            }
            foreach (var tr in state.Turns.Values.OrderBy(t => t.Turn))
            {
                timingHost.Children.Add(BuildTrajectoryTurnRow(tr, () =>
                {
                    if (closed) return;
                    closed = true;
                    dialog.Hide();
                    TrajectoryJumpToTurn(tr.Turn);
                }));
            }
        }

        void RebuildLedger()
        {
            ledgerHost.Children.Clear();
            var kind = (filter.SelectedItem as ComboBoxItem)?.Tag as string ?? "all";
            var rows = state.Entries
                .Where(e => kind == "all" || e.Kind == kind)
                .ToList();
            if (rows.Count == 0)
            {
                ledgerHost.Children.Add(new TextBlock
                {
                    Text = state.Entries.Count == 0
                        ? TrajText("暂无事件（打开会话并产生消息后，账本随 journal 回放/实时流填充）。", "No events yet (the ledger fills from journal replay / live follow once the session has messages).")
                        : TrajText("当前筛选没有事件。", "No events for this filter."),
                    TextWrapping = TextWrapping.Wrap,
                });
                return;
            }
            // 账本按 seq 升序（官方 trajectory 时间线语义）；新事件追加后打开面板即可看到
            foreach (var e in rows)
            {
                ledgerHost.Children.Add(BuildTrajectoryLedgerRow(e));
            }
        }

        filter.SelectionChanged += (_, _) => RebuildLedger();
        dialog.Opened += (_, _) =>
        {
            RebuildTiming();
            RebuildLedger();
            status.Text = state.Entries.Count > 0
                ? TrajFormat("共 {0} 条事件 · {1} 轮", "{0} events · {1} turns", state.Entries.Count, state.Turns.Count)
                : "";
        };
        dialog.Closed += (_, _) => { closed = true; };
        await dialog.ShowAsync();
    }

    /// <summary>轮次导航：与 TurnRail 同语义——目标行顶到视口上沿，compact 折叠时回退答案气泡。</summary>
    private void TrajectoryJumpToTurn(int turn)
    {
        try
        {
            var target = _visibleMessages.FirstOrDefault(b => b.Turn == turn && b.Role is "user" or "user-image");
            target ??= _transcriptAnswers.TryGetValue(turn, out var answer) && _visibleMessages.Contains(answer)
                ? answer
                : null;
            target ??= _visibleMessages.FirstOrDefault(b => b.Turn == turn);
            if (target is null) return;
            ChatList.ScrollIntoView(target, ScrollIntoViewAlignment.Leading);
        }
        catch (Exception) { }
    }

    private UIElement BuildTrajectoryTurnRow(TrajectoryTurnTiming tr, Action jump)
    {
        var durationMs = tr is { StartMs: > 0, EndMs: { } end } && end >= tr.StartMs ? end - tr.StartMs
            : tr.StartMs > 0 ? 0L
            : 0L;
        var headText = TrajFormat("第 {0} 轮", "Turn {0}", tr.Turn);
        if (tr.GoalRoundLabel is { Length: > 0 } goal)
        {
            headText += " · " + goal;
        }
        if (tr.PromptPreview.Length > 0)
        {
            headText += " · " + tr.PromptPreview;
        }
        headText += " · " + (tr.EndMs is null && tr.StartMs > 0
            ? TrajText("会话时间戳（运行中）", "Session timestamps (running)")
            : durationMs > 0 ? FormatDuration(durationMs) : "—");
        headText += " · " + TrajFormat("{0} 步", "{0} steps", tr.Steps);

        var body = new StackPanel { Spacing = Sp4 };
        body.Children.Add(TrajectoryStatRow(TrajText("开始时间", "Started"),
            tr.StartMs > 0 ? FormatMessageClock(tr.StartMs) : TrajText("未记录", "Not recorded")));
        body.Children.Add(TrajectoryStatRow(TrajText("总时长", "Total duration"),
            durationMs > 0 ? FormatDuration(durationMs) : "—"));
        body.Children.Add(TrajectoryStatRow(TrajText("模型用时", "LLM time"), FormatDuration(tr.LlmMs)));
        body.Children.Add(TrajectoryStatRow(TrajText("工具调用用时", "Tool time"), FormatDuration(tr.ToolMs)));
        body.Children.Add(TrajectoryStatRow(TrajText("首 token 延迟", "TTFT"),
            tr.TtftSteps > 0 ? FormatDuration((double)tr.TtftMs / tr.TtftSteps) : "—"));
        body.Children.Add(TrajectoryStatRow(TrajText("输出速度（TPS）", "Throughput (TPS)"), TrajectoryTps(tr) + " tok/s"));
        body.Children.Add(TrajectoryStatRow(TrajText("缓存命中", "Cache hit"), TrajectoryCacheHit(tr) + "%"));
        body.Children.Add(TrajectoryStatRow(TrajText("Token", "Tokens"),
            TokensN0(tr.TotalTokens) + " · " + TrajText("输出", "Output") + " " + TokensN0(tr.OutputTokens)));
        if (tr.Model.Length > 0)
        {
            body.Children.Add(TrajectoryStatRow(TrajText("模型", "Model"), tr.Model));
        }
        if (tr.Tools.Count > 0)
        {
            body.Children.Add(new TextBlock
            {
                Text = TrajText("工具耗时", "Tool durations"),
                Style = AppStyle("CaptionTextStyle"),
                FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
            });
            foreach (var tool in tr.Tools)
            {
                var ms = tool.EndMs is long e && tool.StartMs > 0 && e >= tool.StartMs ? e - tool.StartMs : 0L;
                body.Children.Add(TrajectoryStatRow(
                    tool.Name + (tool.IsError ? " · " + TrajText("失败", "Failed") : ""),
                    ms > 0 ? TrajFormat("{0:N0} 毫秒", "{0:N0} ms", ms) : TrajText("未记录", "Not recorded")));
            }
        }

        var jumpBtn = Aut(new Button
        {
            Content = TrajText("跳到消息", "Jump to message"),
            Style = AppStyle("CompactButtonStyle"),
            HorizontalAlignment = HorizontalAlignment.Left,
        }, "TrajectoryJumpTurn_" + tr.Turn, TrajFormat("跳到第 {0} 轮消息", "Jump to turn {0} message", tr.Turn));
        jumpBtn.Click += (_, _) => jump();
        body.Children.Add(jumpBtn);

        return new Expander
        {
            Header = headText,
            Content = body,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
        };
    }

    private UIElement BuildTrajectoryLedgerRow(TrajectoryEntry e)
    {
        var clock = e.Time > 0 ? FormatMessageClock(e.Time) : "—";
        var header = $"#{e.Seq} · {clock} · {e.Label}";
        if (e.SourceLabel.Length > 0 && !e.Label.Contains(e.SourceLabel, StringComparison.Ordinal))
        {
            header += " · " + e.SourceLabel;
        }
        if (e.Turn > 0)
        {
            header += " · " + TrajFormat("第 {0} 轮", "Turn {0}", e.Turn);
        }

        var body = new StackPanel { Spacing = Sp4 };
        body.Children.Add(TrajectoryStatRow(TrajText("事件", "Event"), e.Type));
        if (e.SourceLabel.Length > 0)
        {
            body.Children.Add(TrajectoryStatRow(TrajText("来源", "Source"), e.SourceLabel));
        }
        if (e.DiffOld is not null || e.DiffNew is not null)
        {
            var diff = new StackPanel { Spacing = Sp2 };
            if (e.DiffPath is { Length: > 0 } path)
            {
                diff.Children.Add(new TextBlock
                {
                    Text = path,
                    Style = AppStyle("CodeTextStyle"),
                    IsTextSelectionEnabled = true,
                    TextWrapping = TextWrapping.Wrap,
                });
            }
            if (e.DiffOld is not null)
            {
                diff.Children.Add(new TextBlock
                {
                    Text = "− " + e.DiffOld,
                    Style = AppStyle("CodeTextStyle"),
                    IsTextSelectionEnabled = true,
                    TextWrapping = TextWrapping.Wrap,
                    Foreground = ThemeBrush("ErrorBrush"),
                });
            }
            if (e.DiffNew is not null)
            {
                diff.Children.Add(new TextBlock
                {
                    Text = "+ " + e.DiffNew,
                    Style = AppStyle("CodeTextStyle"),
                    IsTextSelectionEnabled = true,
                    TextWrapping = TextWrapping.Wrap,
                    Foreground = ThemeBrush("SuccessBrush"),
                });
            }
            body.Children.Add(new Expander
            {
                Header = TrajText("差异", "Diff"),
                Content = diff,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Stretch,
            });
        }
        if (!string.IsNullOrEmpty(e.PayloadJson))
        {
            body.Children.Add(TrajectoryJsonExpander(TrajText("参数 JSON", "Payload JSON"), e.PayloadJson));
        }
        if (!string.IsNullOrEmpty(e.ResultJson))
        {
            body.Children.Add(TrajectoryJsonExpander(TrajText("结果 JSON", "Result JSON"), e.ResultJson!));
        }

        return new Expander
        {
            Header = header,
            Content = body,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
        };
    }

    private static UIElement TrajectoryJsonExpander(string title, string json)
        => new Expander
        {
            Header = title,
            Content = new ScrollViewer
            {
                MaxHeight = 180,
                VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
                Content = new TextBlock
                {
                    Text = json,
                    Style = AppStyle("CodeTextStyle"),
                    IsTextSelectionEnabled = true,
                    TextWrapping = TextWrapping.Wrap,
                },
            },
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
        };

    private static Grid TrajectoryStatRow(string label, string value)
    {
        var g = new Grid { ColumnSpacing = Sp16 };
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var l = new TextBlock { Text = label, Opacity = 0.72, TextWrapping = TextWrapping.Wrap };
        var v = new TextBlock
        {
            Text = value,
            HorizontalAlignment = HorizontalAlignment.Right,
            TextWrapping = TextWrapping.Wrap,
        };
        Grid.SetColumn(l, 0);
        Grid.SetColumn(v, 1);
        g.Children.Add(l);
        g.Children.Add(v);
        return g;
    }

    /// <summary>TPS（RunStats FormatTps 口径：decodeTokens / decodeMs）。</summary>
    private static string TrajectoryTps(TrajectoryTurnTiming tr)
        => tr.DecodeMs > 0 && tr.DecodeTokens > 0
            ? (tr.DecodeTokens / (tr.DecodeMs / 1000.0)).ToString("0", CultureInfo.InvariantCulture)
            : "—";

    /// <summary>缓存命中百分数（RunStats FormatCacheHit 口径）。</summary>
    private static string TrajectoryCacheHit(TrajectoryTurnTiming tr)
        => tr.CacheReadTokens + tr.InputTokens > 0
            ? string.Create(CultureInfo.InvariantCulture, $"{100.0 * tr.CacheReadTokens / (tr.CacheReadTokens + tr.InputTokens):0.#}")
            : "—";
}
