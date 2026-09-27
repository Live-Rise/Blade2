// ---------------- P1 聊天域：Details 面板 / 跨会话召回 / 截断继续 / 模型重试 / 压缩摘要 / workflow-run ----------------
// 对标 @deepseek-ai/dsh-client-ui-chat 的 CompactionItem / ModelRetryItem / TurnMaxTokensItem /
// ContextInjectionRow / UserStyleBubble.referenceSummary / workflow-run 成员展开。
// 状态挂旁路表（不改 ChatBubble）；RenderEventCore 唯一钩子 NoteMessageDomainEvent。

using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;

namespace Blade2;

public sealed partial class MainWindow
{
    // ---------------- 本域文案（中文键 + L()/ShellEnglish 回落 + en 兜底） ----------------

    private string DetText(string zh, string en)
    {
        var s = L(zh);
        if (!string.Equals(s, zh, StringComparison.Ordinal)) return s;
        return _shellLocale.StartsWith("zh", StringComparison.OrdinalIgnoreCase) ? zh : en;
    }

    private string DetFormat(string zhTemplate, string enTemplate, params object?[] args)
    {
        try { return string.Format(DetText(zhTemplate, enTemplate), args); }
        catch (FormatException) { return zhTemplate; }
    }

    // ---------------- 旁路状态 ----------------

    /// <summary>一条消息的 Details 数据：元数据补充、附加内容块、上下文注入清单、跨会话引用。</summary>
    private sealed class MessageDetailState
    {
        public readonly List<(string Label, string Json)> ExtraBlocks = new();
        public readonly List<(string Title, string Body)> ContextEntries = new();
        public readonly List<(string SessionId, string Label)> References = new();
        public string? RelaySessionId;
        public string Provider = "";
        public string Model = "";
        public long? ContextWindow;
        public bool SystemPromptUpdate;
        public string? SystemPromptText;
    }

    private sealed class ModelRetryState
    {
        public string RetryId = "";
        public int Retry;
        public int MaxRetries;
        public long DelayMs;
        public string Mode = "normal";
        public string FailureMessage = "";
        public string FailureCode = "";
        public string State = "scheduled"; // scheduled | started | cancelled
        public long DeadlineMs;
        public ChatBubble? Bubble;
        public Microsoft.UI.Dispatching.DispatcherQueueTimer? Timer;
        public TextBlock? StatusText;
    }

    private sealed class CompactionState
    {
        public string CompactionId = "";
        public string? Summary;
        public long ShadowedItemCount;
        public long ShadowedTokenCount;
        public bool Running;
        public bool Expanded;
        public ChatBubble? Bubble;
    }

    private sealed class WorkflowMemberVm
    {
        public long Seq;
        public string Label = "";
        public string? Phase;
        public string ChildId = "";
        public string? Outcome;
    }

    private sealed class WorkflowRunState
    {
        public string RunId = "";
        public string Name = "";
        public readonly List<WorkflowMemberVm> Members = new();
        public string? StopReason;
        public ChatBubble? Bubble;
    }

    private readonly Dictionary<ChatBubble, MessageDetailState> _messageDetails = new();
    private readonly Dictionary<string, ModelRetryState> _modelRetries = new(); // key = retryId
    private readonly Dictionary<string, CompactionState> _compactions = new(); // key = compactionId
    private readonly Dictionary<string, WorkflowRunState> _workflowRuns = new(); // key = runId
    private readonly Dictionary<string, string> _workflowBubbleRun = new(); // callId/runName → runId
    private readonly Dictionary<ChatBubble, bool> _maxTokensBubbles = new();

    private MessageDetailState DetailState(ChatBubble bubble)
    {
        if (!_messageDetails.TryGetValue(bubble, out var st))
        {
            _messageDetails[bubble] = st = new MessageDetailState();
        }
        return st;
    }

    /// <summary>清会话时旁路表一并作废（_messages.Reset 触发）。</summary>
    private void ResetMessageDomainState()
    {
        foreach (var r in _modelRetries.Values)
        {
            r.Timer?.Stop();
            r.Timer = null;
        }
        _modelRetries.Clear();
        _compactions.Clear();
        _workflowRuns.Clear();
        _workflowBubbleRun.Clear();
        _maxTokensBubbles.Clear();
        _messageDetails.Clear();
    }

    // ---------------- RenderEventCore 唯一钩子 ----------------

    /// <summary>P1 聊天域事件折叠。返回 true = 本事件已完整消费，RenderEventCore 不再走 switch。</summary>
    private bool NoteMessageDomainEvent(JsonElement ev, string type, JsonElement data, long envTime, int envSeq, int turn)
    {
        try
        {
            switch (type)
            {
                case "system/message":
                    return HandleSystemMessage(data, turn, envTime, envSeq);
                case "user/message":
                    return HandleUserMessage(data, turn, envTime, envSeq);
                case "request/context":
                    NoteRequestContext(data);
                    return false;
                case "turn/end":
                    NoteTurnEndReason(data, turn, envTime);
                    return false;
                case "assistant/attempt":
                    NoteAttemptMaxTokens(data, turn, envTime);
                    return false;
                case "llm/retry":
                    HandleLlmRetry(data, turn, envTime);
                    return true;
                case "llm/retry-started":
                    HandleLlmRetryStarted(data);
                    return true;
                case "compaction/start":
                    HandleCompactionStart(data, turn, envTime);
                    return true;
                case "compaction/summary":
                    HandleCompactionSummary(data);
                    return true;
                case "compaction/end":
                    HandleCompactionEnd(data);
                    return true;
                case "tool-workflow/run-start":
                    HandleWorkflowRunStart(data, turn, envTime, envSeq);
                    return true;
                case "tool-workflow/agent-start":
                    HandleWorkflowAgentStart(data);
                    return true;
                case "tool-workflow/agent-end":
                    HandleWorkflowAgentEnd(data);
                    return true;
                case "tool-workflow/run-end":
                    HandleWorkflowRunEnd(data);
                    return true;
                case "tool/call":
                    NoteWorkflowToolCall(data);
                    return false;
                case "tool/result":
                    return false;
            }
        }
        catch (Exception) { } // P1 折叠失败不挡主渲染
        return false;
    }

    // ---------------- system/message → 系统提示词 / 更新（可展开） ----------------

    private bool HandleSystemMessage(JsonElement data, int turn, long envTime, int envSeq)
    {
        var sid = Volatile.Read(ref _activeSessionId);
        var first = true;
        if (!string.IsNullOrEmpty(sid))
        {
            var stSys = GetOrCreateRunStats(sid);
            first = !stSys.SystemPromptSeen;
            stSys.SystemPromptSeen = true;
        }
        var body = ExtractMessageText(data, "message");
        var bubble = new ChatBubble
        {
            Role = "tool-call",
            Text = DetText(first ? "系统提示词" : "系统提示词更新", first ? "System prompt" : "System prompt update"),
            IsToolCall = true,
            ToolName = "system-prompt",
            Turn = turn,
            Time = envTime,
            Seq = envSeq,
        };
        var st = DetailState(bubble);
        st.SystemPromptUpdate = !first;
        st.SystemPromptText = body;
        st.ContextEntries.Add((
            DetText(first ? "系统提示词" : "系统提示词更新", first ? "System prompt" : "System prompt update"),
            body));
        AppendBubble(bubble);
        return true;
    }

    // ---------------- user/message：召回 / 中继 / 上下文注入 / 附加块 ----------------

    private bool HandleUserMessage(JsonElement data, int turn, long envTime, int envSeq)
    {
        if (data.ValueKind != JsonValueKind.Object ||
            !data.TryGetProperty("content", out var content) ||
            content.ValueKind != JsonValueKind.Array)
        {
            return false;
        }

        // 非用户来源：跨会话召回 / 中继 / 上下文注入 → 定制过程行
        if (data.TryGetProperty("source", out var src) && src.ValueKind == JsonValueKind.Object &&
            src.TryGetProperty("kind", out var sk) && sk.ValueKind == JsonValueKind.String &&
            sk.GetString() is { Length: > 0 } kind && kind != "user")
        {
            var form = Str(src, "form");
            if (kind == "agent-message" && form == "relay")
            {
                var sender = Str(src, "senderSessionId");
                var bubble = new ChatBubble
                {
                    Role = "tool-call",
                    Text = DetFormat("来自会话 {0}", "From session {0}", sender),
                    IsToolCall = true,
                    ToolName = "session-recall",
                    Turn = turn,
                    Time = envTime,
                    Seq = envSeq,
                };
                var st = DetailState(bubble);
                st.RelaySessionId = sender.Length > 0 ? sender : null;
                st.ContextEntries.Add((DetText("跨会话中继", "Session relay"), ExtractMessageText(data, "")));
                AppendBubble(bubble);
                return true;
            }
            if (kind == "session-reference" && form == "recall")
            {
                var labels = new List<string>();
                var bubble = new ChatBubble
                {
                    Role = "tool-call",
                    Text = "",
                    IsToolCall = true,
                    ToolName = "session-recall",
                    Turn = turn,
                    Time = envTime,
                    Seq = envSeq,
                };
                var st = DetailState(bubble);
                if (src.TryGetProperty("references", out var refs) && refs.ValueKind == JsonValueKind.Array)
                {
                    foreach (var r in refs.EnumerateArray())
                    {
                        var label = Str(r, "label");
                        var sid = Str(r, "sessionId");
                        var retained = r.TryGetProperty("retainedMessages", out var rv) && rv.ValueKind == JsonValueKind.Number ? rv.GetInt64() : 0;
                        var omitted = r.TryGetProperty("omittedMessages", out var ov) && ov.ValueKind == JsonValueKind.Number ? ov.GetInt64() : 0;
                        var truncated = r.TryGetProperty("truncated", out var tv) && tv.ValueKind == JsonValueKind.True;
                        if (label.Length > 0) labels.Add(label);
                        if (sid.Length > 0 || label.Length > 0)
                        {
                            st.References.Add((sid, label.Length > 0 ? label : sid));
                        }
                        st.ContextEntries.Add((
                            DetFormat("跨会话召回 · {0}", "Session recall · {0}", label.Length > 0 ? label : sid),
                            DetFormat("保留 {0} 条 · 省略 {1} 条", "{0} kept · {1} omitted", retained, omitted) +
                            (truncated ? " · " + DetText("已截断", "truncated") : "")));
                    }
                }
                var sep = DetText("、", ", ");
                bubble.Text = labels.Count > 0
                    ? DetFormat("引用会话 · {0}", "Referenced session · {0}", string.Join(sep, labels))
                    : DetText("跨会话召回", "Session recall");
                AppendBubble(bubble);
                return true;
            }

            // 其余注入（instructions / catalog / snapshot / notice…）→ 上下文注入行
            var label2 = Str(src, "plugin");
            if (label2.Length == 0) label2 = Str(src, "path");
            if (label2.Length == 0) label2 = Str(src, "label");
            if (label2.Length == 0) label2 = kind;
            var injBubble = new ChatBubble
            {
                Role = "tool-call",
                Text = DetText("上下文注入", "Context injection") + " · " + label2,
                IsToolCall = true,
                ToolName = "context-injection",
                Turn = turn,
                Time = envTime,
                Seq = envSeq,
            };
            var injState = DetailState(injBubble);
            injState.ContextEntries.Add((label2, ExtractContentText(content)));
            CollectInstructionChanges(src, injState);
            CollectCatalogEntries(src, injState);
            CollectSnapshotSections(src, injState);
            AppendBubble(injBubble);
            return true;
        }

        // 普通用户消息：登记附加内容块 + @ 会话引用标签（Details / 召回 chips 数据源）
        var text = string.Join("\n", content.EnumerateArray()
            .Where(c => Str(c, "type") == "text")
            .Select(c => Str(c, "text")));
        foreach (var block in content.EnumerateArray())
        {
            var bt = Str(block, "type");
            if (bt is "text" or "image") continue;
            var label = bt.Length > 0 ? bt : "block";
            var json = block.GetRawText();
            // 先挂到“下一条用户气泡”：RenderEventCore 的 user 分支随后创建气泡后补登记
            _pendingUserExtras.Add((label, json));
        }
        CollectMentionReferences(text);
        return false; // 让主路径继续创建用户气泡
    }

    private readonly List<(string Label, string Json)> _pendingUserExtras = new();
    private readonly List<(string SessionId, string Label)> _pendingUserRefs = new();

    /// <summary>主路径创建用户气泡后调用：把待登记的附加块/引用标签挂到该气泡。</summary>
    private void AttachPendingUserDetail(ChatBubble bubble)
    {
        if (_pendingUserExtras.Count == 0 && _pendingUserRefs.Count == 0) return;
        var st = DetailState(bubble);
        st.ExtraBlocks.AddRange(_pendingUserExtras);
        st.References.AddRange(_pendingUserRefs);
        _pendingUserExtras.Clear();
        _pendingUserRefs.Clear();
    }

    private void CollectMentionReferences(string text)
    {
        // @[label](dsh-session:…) / 裸 dsh-session: URI
        var span = text.AsSpan();
        int i = 0;
        while (i < text.Length)
        {
            var idx = text.IndexOf("dsh-session:", i, StringComparison.Ordinal);
            if (idx < 0) break;
            var uriStart = idx;
            var uriEnd = uriStart + "dsh-session:".Length;
            while (uriEnd < text.Length && (char.IsLetterOrDigit(text[uriEnd]) || text[uriEnd] is '-' or '_' or '+' or '/' or '='))
            {
                uriEnd++;
            }
            var uri = text[uriStart..uriEnd];
            var label = "";
            // 向前找 @[label](
            if (uriStart >= 2)
            {
                var close = text.LastIndexOf(']', uriStart - 1);
                var open = close > 0 ? text.LastIndexOf('@', close - 1) : -1;
                if (open >= 0 && close > open && uriStart > close && text[close + 1] == '(')
                {
                    label = text[(open + 2)..close];
                }
            }
            if (label.Length == 0) label = uri;
            var sid = DecodeSessionMention(uri);
            _pendingUserRefs.Add((sid, label));
            i = uriEnd;
        }
    }

    private static string DecodeSessionMention(string uri)
    {
        const string scheme = "dsh-session:";
        if (!uri.StartsWith(scheme, StringComparison.Ordinal)) return "";
        var payload = uri[scheme.Length..];
        try
        {
            var bytes = Convert.FromBase64String(PadBase64(payload));
            var s = System.Text.Encoding.UTF8.GetString(bytes);
            if (s.StartsWith("{", StringComparison.Ordinal))
            {
                using var doc = JsonDocument.Parse(s);
                if (doc.RootElement.ValueKind == JsonValueKind.Object &&
                    doc.RootElement.TryGetProperty("sessionId", out var id) &&
                    id.ValueKind == JsonValueKind.String)
                {
                    return id.GetString() ?? "";
                }
            }
            return s.Length > 0 && !s.Contains('\0') ? s : payload;
        }
        catch (Exception)
        {
            return payload;
        }

        static string PadBase64(string v)
        {
            var t = v.Replace('-', '+').Replace('_', '/');
            switch (t.Length % 4)
            {
                case 2: return t + "==";
                case 3: return t + "=";
                default: return t;
            }
        }
    }

    private void CollectInstructionChanges(JsonElement source, MessageDetailState st)
    {
        if (!source.TryGetProperty("changes", out var list) || list.ValueKind != JsonValueKind.Array) return;
        foreach (var change in list.EnumerateArray())
        {
            var path = Str(change, "path");
            var action = Str(change, "action");
            if (path.Length == 0) continue;
            var actionLabel = action switch
            {
                "remove" => DetText("已移除", "removed"),
                "set" => DetText("已新增", "added"),
                _ => DetText("已更新", "updated"),
            };
            if (source.TryGetProperty("baseline", out var b) && b.ValueKind == JsonValueKind.True && action != "remove")
            {
                actionLabel = DetText("已载入", "loaded");
            }
            st.ContextEntries.Add((path, actionLabel));
        }
    }

    private void CollectCatalogEntries(JsonElement source, MessageDetailState st)
    {
        if (!source.TryGetProperty("entries", out var list) || list.ValueKind != JsonValueKind.Array) return;
        var shown = 0;
        foreach (var entry in list.EnumerateArray())
        {
            var name = Str(entry, "name");
            var desc = Str(entry, "description");
            if (name.Length == 0) continue;
            shown++;
            if (shown <= 8) st.ContextEntries.Add((name, desc));
        }
        if (shown > 8)
        {
            st.ContextEntries.Add((DetText("…还有 {0} 条", "… {0} more"), DetFormat("…还有 {0} 条", "… {0} more", shown - 8)));
        }
    }

    private void CollectSnapshotSections(JsonElement source, MessageDetailState st)
    {
        if (!source.TryGetProperty("sections", out var list) || list.ValueKind != JsonValueKind.Array) return;
        st.ContextEntries.Add(("", DetText("取代先前的快照", "Supersedes earlier snapshots")));
        foreach (var section in list.EnumerateArray())
        {
            var name = Str(section, "name");
            var text = Str(section, "text");
            if (name.Length == 0) continue;
            st.ContextEntries.Add((name, text));
        }
    }

    private void NoteRequestContext(JsonElement data)
    {
        // request/context：provider/model/contextWindow/systemPromptUpdate —— 记到“当前待认领”消息的 Details
        if (data.ValueKind != JsonValueKind.Object) return;
        var provider = Str(data, "provider");
        var model = Str(data, "model");
        long? window = data.TryGetProperty("contextWindow", out var w) && w.ValueKind == JsonValueKind.Number ? w.GetInt64() : null;
        var update = Str(data, "systemPromptUpdate");
        // 挂到最近一条用户/助手气泡（同一请求窗口）
        var anchor = LastUserBubble() ?? (_messages.Count > 0 ? _messages[^1] : null);
        if (anchor is null) return;
        var st = DetailState(anchor);
        if (provider.Length > 0) st.Provider = provider;
        if (model.Length > 0) st.Model = model;
        if (window is > 0) st.ContextWindow = window;
        if (update.Length > 0) st.SystemPromptUpdate = true;
    }

    private void NoteTurnEndReason(JsonElement data, int turn, long envTime)
    {
        if (data.ValueKind != JsonValueKind.Object ||
            !data.TryGetProperty("reason", out var reason) ||
            reason.ValueKind != JsonValueKind.Object ||
            Str(reason, "kind") != "max-tokens")
        {
            return;
        }
        AppendMaxTokensRow(turn, envTime);
    }

    private void NoteAttemptMaxTokens(JsonElement data, int turn, long envTime)
    {
        if (data.ValueKind != JsonValueKind.Object ||
            !data.TryGetProperty("stream", out var stream) ||
            stream.ValueKind != JsonValueKind.Array)
        {
            return;
        }
        foreach (var piece in stream.EnumerateArray())
        {
            if (Str(piece, "type") != "finish") continue;
            if (!piece.TryGetProperty("reason", out var reason) || reason.ValueKind != JsonValueKind.Object) continue;
            if (Str(reason, "kind") == "max-tokens")
            {
                AppendMaxTokensRow(turn, envTime);
            }
        }
    }

    // ---------------- P1-5 截断继续 ----------------

    private void AppendMaxTokensRow(int turn, long envTime)
    {
        // 同轮只出一条
        foreach (var b in _maxTokensBubbles.Keys.ToList())
        {
            if (b.Turn == turn) return;
        }
        var bubble = new ChatBubble
        {
            Role = "tool-call",
            Text = DetText("已达到输出 token 上限", "Output token limit reached"),
            IsToolCall = true,
            ToolName = "max-tokens",
            Turn = turn,
            Time = envTime,
        };
        _maxTokensBubbles[bubble] = true;
        AppendBubble(bubble);
    }

    private void SendContinuePrompt()
    {
        InputBox.Text = DetText("继续", "continue");
        _ = SendAsync();
    }

    // ---------------- P1-6 模型重试 ----------------

    private void HandleLlmRetry(JsonElement data, int turn, long envTime)
    {
        var retryId = Str(data, "retryId");
        if (retryId.Length == 0) retryId = "retry-" + envTime + "-" + turn;
        var retry = data.TryGetProperty("retry", out var rv) && rv.ValueKind == JsonValueKind.Number ? rv.GetInt32() : 1;
        var max = data.ValueKind == JsonValueKind.Object && data.TryGetProperty("maxRetries", out var mv) && mv.ValueKind == JsonValueKind.Number
            ? mv.GetInt32()
            : 0;
        var delay = data.TryGetProperty("delayMs", out var dv) && dv.ValueKind == JsonValueKind.Number ? dv.GetInt64() : 0L;
        var mode = Str(data, "mode");
        if (mode.Length == 0) mode = "normal";
        var failureMessage = "";
        var failureCode = "";
        if (data.TryGetProperty("failure", out var fl) && fl.ValueKind == JsonValueKind.Object)
        {
            failureMessage = Str(fl, "message");
            failureCode = Str(fl, "code");
        }

        if (!_modelRetries.TryGetValue(retryId, out var st))
        {
            _modelRetries[retryId] = st = new ModelRetryState { RetryId = retryId };
        }
        st.Retry = retry;
        st.MaxRetries = max;
        st.DelayMs = delay;
        st.Mode = mode;
        st.FailureMessage = failureMessage;
        st.FailureCode = failureCode;
        st.State = "scheduled";
        st.DeadlineMs = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds() + delay;

        if (st.Bubble is null)
        {
            st.Bubble = new ChatBubble
            {
                Role = "tool-call",
                Text = DetText("等待重试模型请求", "Waiting to retry model request"),
                IsToolCall = true,
                ToolName = "model-retry",
                Turn = turn,
                Time = envTime,
            };
            AppendBubble(st.Bubble);
        }
        else
        {
            RepaintBubble(st.Bubble);
        }
        StartRetryCountdown(st);
    }

    private void HandleLlmRetryStarted(JsonElement data)
    {
        var retryId = Str(data, "retryId");
        if (retryId.Length == 0 || !_modelRetries.TryGetValue(retryId, out var st)) return;
        st.State = "started";
        st.Timer?.Stop();
        st.Timer = null;
        if (st.Bubble is not null) RepaintBubble(st.Bubble);
    }

    private void StartRetryCountdown(ModelRetryState st)
    {
        st.Timer?.Stop();
        var timer = DispatcherQueue.CreateTimer();
        timer.Interval = TimeSpan.FromMilliseconds(250);
        timer.IsRepeating = true;
        timer.Tick += (_, _) =>
        {
            if (st.State != "scheduled")
            {
                timer.Stop();
                return;
            }
            if (st.StatusText is { } tb)
            {
                tb.Text = RetryStatusLine(st);
            }
            var remain = st.DeadlineMs - DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
            if (remain <= 0)
            {
                timer.Stop();
            }
        };
        st.Timer = timer;
        timer.Start();
    }

    private string RetryStatusLine(ModelRetryState st)
    {
        var label = st.State switch
        {
            "started" => DetText("已重试模型请求", "Retried model request"),
            "cancelled" => DetText("模型请求重试已取消", "Model request retry cancelled"),
            _ => DetText("等待重试模型请求", "Waiting to retry model request"),
        };
        var maximum = st.Mode == "normal" ? st.MaxRetries.ToString() : "∞";
        int seconds;
        if (st.State == "scheduled")
        {
            var remain = st.DeadlineMs - DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
            seconds = (int)Math.Max(1, Math.Ceiling(remain / 1000.0));
        }
        else
        {
            seconds = (int)Math.Max(1, Math.Ceiling(st.DelayMs / 1000.0));
        }
        return DetFormat("{0}（{1}/{2}） · {3}s", "{0} ({1}/{2}) · {3}s", label, st.Retry, maximum, seconds);
    }

    private void CancelModelRetry(ModelRetryState st)
    {
        st.State = "cancelled";
        st.Timer?.Stop();
        st.Timer = null;
        if (st.Bubble is not null) RepaintBubble(st.Bubble);
        _ = StopActiveRunAsync();
    }

    // ---------------- P1-7 压缩摘要 ----------------

    private void HandleCompactionStart(JsonElement data, int turn, long envTime)
    {
        var id = Str(data, "compactionId");
        if (id.Length == 0) id = "compaction-" + envTime;
        if (!_compactions.TryGetValue(id, out var st))
        {
            _compactions[id] = st = new CompactionState { CompactionId = id };
        }
        st.Running = true;
        if (st.Bubble is null)
        {
            st.Bubble = new ChatBubble
            {
                Role = "tool-call",
                Text = DetText("上下文已压缩", "Context compacted"),
                IsToolCall = true,
                ToolName = "compaction",
                Turn = turn,
                Time = envTime,
            };
            AppendBubble(st.Bubble);
        }
        else
        {
            RepaintBubble(st.Bubble);
        }
    }

    private void HandleCompactionSummary(JsonElement data)
    {
        var id = Str(data, "compactionId");
        if (id.Length == 0 || !_compactions.TryGetValue(id, out var st)) return;
        st.ShadowedItemCount = data.TryGetProperty("shadowedSeqs", out var seqs) && seqs.ValueKind == JsonValueKind.Array
            ? seqs.GetArrayLength()
            : data.TryGetProperty("shadowedRange", out _) ? 0 : 0;
        if (data.TryGetProperty("shadowedTokenCount", out var tok) && tok.ValueKind == JsonValueKind.Number)
        {
            st.ShadowedTokenCount = tok.GetInt64();
        }
        if (data.TryGetProperty("shadowedSeqs", out var seqs2) && seqs2.ValueKind == JsonValueKind.Array)
        {
            st.ShadowedItemCount = seqs2.GetArrayLength();
        }
        st.Summary = ExtractContentText(data.TryGetProperty("summary", out var sum) ? sum : default);
        st.Running = false;
        if (st.Bubble is not null) RepaintBubble(st.Bubble);
    }

    private void HandleCompactionEnd(JsonElement data)
    {
        var id = Str(data, "compactionId");
        if (id.Length == 0 || !_compactions.TryGetValue(id, out var st)) return;
        st.Running = false;
        if (st.Bubble is not null) RepaintBubble(st.Bubble);
    }

    // ---------------- P1-18 workflow-run ----------------

    private void HandleWorkflowRunStart(JsonElement data, int turn, long envTime, int envSeq)
    {
        var runId = Str(data, "runId");
        if (runId.Length == 0) runId = "run-" + envTime;
        var name = Str(data, "name");
        if (!_workflowRuns.TryGetValue(runId, out var st))
        {
            _workflowRuns[runId] = st = new WorkflowRunState { RunId = runId };
        }
        st.Name = name.Length > 0 ? name : runId;
        if (st.Bubble is null)
        {
            st.Bubble = new ChatBubble
            {
                Role = "tool-call",
                Text = st.Name,
                IsToolCall = true,
                ToolName = "workflow-run",
                ToolArgs = $"{{\"runId\":\"{runId}\"}}",
                Turn = turn,
                Time = envTime,
                Seq = envSeq,
            };
            AppendBubble(st.Bubble);
        }
        else
        {
            RepaintBubble(st.Bubble);
        }
    }

    private void HandleWorkflowAgentStart(JsonElement data)
    {
        var runId = Str(data, "runId");
        if (runId.Length == 0 || !_workflowRuns.TryGetValue(runId, out var st)) return;
        var seq = data.TryGetProperty("seq", out var sv) && sv.ValueKind == JsonValueKind.Number ? sv.GetInt64() : st.Members.Count + 1;
        var member = new WorkflowMemberVm
        {
            Seq = seq,
            Label = Str(data, "label"),
            Phase = data.TryGetProperty("phase", out var pv) && pv.ValueKind == JsonValueKind.String ? pv.GetString() : null,
            ChildId = Str(data, "childId"),
        };
        st.Members.Add(member);
        if (st.Bubble is not null) RepaintBubble(st.Bubble);
    }

    private void HandleWorkflowAgentEnd(JsonElement data)
    {
        var runId = Str(data, "runId");
        if (runId.Length == 0 || !_workflowRuns.TryGetValue(runId, out var st)) return;
        var seq = data.TryGetProperty("seq", out var sv) && sv.ValueKind == JsonValueKind.Number ? sv.GetInt64() : 0;
        var outcome = Str(data, "outcome");
        foreach (var m in st.Members)
        {
            if (m.Seq == seq) m.Outcome = outcome;
        }
        if (st.Bubble is not null) RepaintBubble(st.Bubble);
    }

    private void HandleWorkflowRunEnd(JsonElement data)
    {
        var runId = Str(data, "runId");
        if (runId.Length == 0 || !_workflowRuns.TryGetValue(runId, out var st)) return;
        st.StopReason = Str(data, "stopReason");
        if (st.Bubble is not null) RepaintBubble(st.Bubble);
    }

    private void NoteWorkflowToolCall(JsonElement data)
    {
        var name = Str(data, "name");
        if (name != "workflow") return;
        var callId = Str(data, "callId");
        var args = Str(data, "arguments");
        string? runName = null;
        try
        {
            using var doc = JsonDocument.Parse(args);
            if (doc.RootElement.ValueKind == JsonValueKind.Object &&
                doc.RootElement.TryGetProperty("meta", out var meta) &&
                meta.ValueKind == JsonValueKind.Object)
            {
                runName = Str(meta, "name");
            }
        }
        catch (Exception) { }
        // 把最近一个同名 run 记到 callId，工具卡展开成员时用
        var match = _workflowRuns.Values.LastOrDefault(r => runName is null || r.Name == runName);
        if (match is not null && callId.Length > 0)
        {
            _workflowBubbleRun[callId] = match.RunId;
        }
    }

    /// <summary>工具卡（workflow）展开成员：返回 run 状态供 ToolCards 使用。</summary>
    private WorkflowRunState? WorkflowRunForTool(ChatBubble bubble)
    {
        // ToolCallCardState.CallId 在 NoteToolResult 时写入；这里按最近 run 兜底
        if (_toolCardStates.TryGetValue(bubble, out var st) && st.CallId.Length > 0 &&
            _workflowBubbleRun.TryGetValue(st.CallId, out var runId) &&
            _workflowRuns.TryGetValue(runId, out var run))
        {
            return run;
        }
        var name = "";
        try
        {
            using var doc = JsonDocument.Parse(bubble.ToolArgs ?? "{}");
            if (doc.RootElement.ValueKind == JsonValueKind.Object &&
                doc.RootElement.TryGetProperty("meta", out var meta) &&
                meta.ValueKind == JsonValueKind.Object)
            {
                name = Str(meta, "name");
            }
        }
        catch (Exception) { }
        return _workflowRuns.Values.LastOrDefault(r => name.Length == 0 || r.Name == name);
    }

    // ---------------- 工具抽取辅助 ----------------

    private static string ExtractMessageText(JsonElement data, string messageKey)
    {
        if (data.ValueKind != JsonValueKind.Object) return "";
        var msg = data;
        if (messageKey.Length > 0)
        {
            if (!data.TryGetProperty(messageKey, out msg) || msg.ValueKind != JsonValueKind.Object) return "";
        }
        return ExtractContentText(msg.TryGetProperty("content", out var c) ? c : default);
    }

    private static string ExtractContentText(JsonElement content)
    {
        if (content.ValueKind != JsonValueKind.Array) return "";
        var parts = new List<string>();
        foreach (var b in content.EnumerateArray())
        {
            var t = Str(b, "type");
            if (t == "text") parts.Add(Str(b, "text"));
            else if (t == "reasoning") parts.Add(Str(b, "text"));
            else parts.Add(b.GetRawText());
        }
        return string.Join("\n", parts);
    }

    // ---------------- 过程卡装配（BuildToolCard 分派） ----------------

    private static readonly HashSet<string> MessageDomainTools = new(StringComparer.Ordinal)
    {
        "system-prompt", "context-injection", "session-recall", "compaction", "model-retry", "max-tokens", "workflow-run",
    };

    /// <summary>BuildToolCard 入口分派：P1 过程卡走本方法，其余回 keyed 工具卡。</summary>
    private void BuildMessageDomainCard(ContentControl host, ChatBubble bubble)
    {
        var tool = bubble.ToolName ?? "";
        var root = new StackPanel
        {
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Margin = new Thickness(4, 1, 4, 1),
            Spacing = Sp2,
        };
        AutomationProperties.SetAutomationId(root, "MessageDomainCard_" + tool);
        switch (tool)
        {
            case "max-tokens":
                BuildMaxTokensCard(root, bubble);
                break;
            case "model-retry":
                BuildRetryCard(root, bubble);
                break;
            case "compaction":
                BuildCompactionCard(root, bubble);
                break;
            case "workflow-run":
                BuildWorkflowRunCard(root, bubble);
                break;
            case "session-recall":
                BuildSessionRecallCard(root, bubble);
                break;
            case "context-injection":
            case "system-prompt":
            default:
                BuildContextCard(root, bubble);
                break;
        }
        host.Content = root;
    }

    private void BuildMaxTokensCard(StackPanel root, ChatBubble bubble)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6, VerticalAlignment = VerticalAlignment.Center };
        row.Children.Add(new FontIcon { Glyph = "\uE7BA", FontSize = 12, Foreground = ThemeBrush("WarningBrush") });
        row.Children.Add(new TextBlock
        {
            Text = DetText("已达到输出 token 上限", "Output token limit reached"),
            Style = AppTextStyle("CaptionTextStyle"),
            FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
            Foreground = ThemeBrush("WarningBrush"),
        });
        root.Children.Add(row);
        root.Children.Add(new TextBlock
        {
            Text = DetText(
                "回答被截断，已有输出保留在对话中。发送“继续”可让模型接着输出。",
                "The reply was cut off; earlier output is preserved in the conversation. Send \"continue\" to let the model resume."),
            Style = AppTextStyle("HintTextStyle"),
            TextWrapping = TextWrapping.Wrap,
        });
        var btn = new Button
        {
            Content = DetText("发送继续", "Send continue"),
            Style = AppStyle("CompactButtonStyle"),
            HorizontalAlignment = HorizontalAlignment.Left,
            Margin = new Thickness(0, Sp2, 0, 0),
        };
        Aut(btn, "MaxTokensContinueButton", DetText("发送继续", "Send continue"));
        btn.Click += (_, _) => SendContinuePrompt();
        root.Children.Add(btn);
    }

    private void BuildRetryCard(StackPanel root, ChatBubble bubble)
    {
        ModelRetryState? st = null;
        foreach (var r in _modelRetries.Values)
        {
            if (ReferenceEquals(r.Bubble, bubble)) { st = r; break; }
        }
        st ??= _modelRetries.Values.LastOrDefault();

        var header = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6 };
        header.Children.Add(new FontIcon { Glyph = "\uE72C", FontSize = 12, Foreground = ThemeBrush("InfoBrush") });
        var status = new TextBlock
        {
            Text = st is null ? DetText("等待重试模型请求", "Waiting to retry model request") : RetryStatusLine(st),
            Style = AppTextStyle("CaptionTextStyle"),
        };
        st!.StatusText = status;
        header.Children.Add(status);
        root.Children.Add(header);

        if (st is not null)
        {
            var detail = new StackPanel { Spacing = Sp2, Margin = new Thickness(18, 0, 0, 0) };
            detail.Children.Add(new TextBlock
            {
                Text = DetFormat("重试延迟：{0} ms", "Retry delay: {0} ms", st.DelayMs),
                Style = AppTextStyle("HintTextStyle"),
            });
            if (st.FailureMessage.Length > 0 || st.FailureCode.Length > 0)
            {
                detail.Children.Add(new TextBlock
                {
                    Text = DetText("失败原因：", "Failure reason: ") +
                           (st.FailureCode.Length > 0 ? $"[{st.FailureCode}] " : "") + st.FailureMessage,
                    Style = AppTextStyle("HintTextStyle"),
                    TextWrapping = TextWrapping.Wrap,
                });
            }
            root.Children.Add(detail);

            if (st.State == "scheduled")
            {
                var cancel = new Button
                {
                    Content = DetText("取消重试", "Cancel retry"),
                    Style = AppStyle("CompactButtonStyle"),
                    HorizontalAlignment = HorizontalAlignment.Left,
                    Margin = new Thickness(0, Sp2, 0, 0),
                };
                Aut(cancel, "CancelRetryButton", DetText("取消重试", "Cancel retry"));
                cancel.Click += (_, _) => CancelModelRetry(st);
                root.Children.Add(cancel);
            }
        }
    }

    private void BuildCompactionCard(StackPanel root, ChatBubble bubble)
    {
        CompactionState? st = null;
        foreach (var c in _compactions.Values)
        {
            if (ReferenceEquals(c.Bubble, bubble)) { st = c; break; }
        }
        st ??= _compactions.Values.LastOrDefault();

        var summaryText = st switch
        {
            null => DetText("点击查看压缩摘要", "View compaction summary"),
            { Running: true } => DetText("正在压缩…", "Compacting context…"),
            { ShadowedItemCount: > 0, ShadowedTokenCount: > 0 } =>
                DetFormat("已压缩 {0} 条历史记录（约 {1} tokens）", "Compacted {0} history items (~{1} tokens)",
                    st.ShadowedItemCount, st.ShadowedTokenCount),
            { Summary: not null } => DetText("点击查看压缩摘要", "View compaction summary"),
            _ => DetText("压缩摘要不可用", "Compaction summary unavailable"),
        };
        var expandable = st?.Summary is { Length: > 0 };

        var header = new ToggleButton
        {
            Background = new SolidColorBrush(Colors.Transparent),
            BorderThickness = new Thickness(0),
            Padding = new Thickness(Sp4, Sp2, Sp2, Sp2),
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Left,
            IsChecked = st?.Expanded == true,
            IsEnabled = expandable,
        };
        Aut(header, "CompactionExpand", DetText("上下文已压缩", "Context compacted"));
        var inner = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6, VerticalAlignment = VerticalAlignment.Center };
        var chevron = new FontIcon { Glyph = st?.Expanded == true ? "\uE76C" : "\uE76B", FontSize = 10 };
        inner.Children.Add(chevron);
        inner.Children.Add(new FontIcon { Glyph = "\uE943", FontSize = 12, Opacity = 0.85 });
        inner.Children.Add(new TextBlock
        {
            Text = DetText("上下文已压缩", "Context compacted"),
            Style = AppTextStyle("CaptionTextStyle"),
            FontWeight = Microsoft.UI.Text.FontWeights.Medium,
        });
        inner.Children.Add(new TextBlock
        {
            Text = "·",
            Style = AppTextStyle("CaptionTextStyle"),
            Opacity = 0.45,
        });
        inner.Children.Add(new TextBlock
        {
            Text = summaryText,
            Style = AppTextStyle("HintTextStyle"),
            TextTrimming = TextTrimming.CharacterEllipsis,
        });
        header.Content = inner;
        root.Children.Add(header);

        var body = new StackPanel
        {
            Margin = new Thickness(22, Sp2, 0, 0),
            Visibility = st?.Expanded == true ? Visibility.Visible : Visibility.Collapsed,
        };
        if (st?.Summary is { Length: > 0 } summary)
        {
            body.Children.Add(MakeCodeBlock(summary, wrap: true, maxHeight: 160));
        }
        root.Children.Add(body);

        if (st is not null && expandable)
        {
            header.Checked += (_, _) => { st.Expanded = true; body.Visibility = Visibility.Visible; chevron.Glyph = "\uE76C"; };
            header.Unchecked += (_, _) => { st.Expanded = false; body.Visibility = Visibility.Collapsed; chevron.Glyph = "\uE76B"; };
        }
    }

    private void BuildWorkflowRunCard(StackPanel root, ChatBubble bubble)
    {
        WorkflowRunState? st = null;
        var runId = "";
        try
        {
            using var doc = JsonDocument.Parse(bubble.ToolArgs ?? "{}");
            if (doc.RootElement.ValueKind == JsonValueKind.Object)
            {
                runId = Str(doc.RootElement, "runId");
            }
        }
        catch (Exception) { }
        if (runId.Length > 0) _workflowRuns.TryGetValue(runId, out st);
        st ??= _workflowRuns.Values.LastOrDefault(r => ReferenceEquals(r.Bubble, bubble)) ?? _workflowRuns.Values.LastOrDefault();

        var name = st?.Name is { Length: > 0 } n ? n : bubble.Text;
        var count = st?.Members.Count ?? 0;
        var status = st?.StopReason switch
        {
            "completed" => DetText("已完成", "Completed"),
            "cancelled" => DetText("已取消", "Cancelled"),
            "error" => DetText("失败", "Failed"),
            null => DetText("运行中", "Running"),
            _ => st!.StopReason!,
        };

        var header = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6, VerticalAlignment = VerticalAlignment.Center };
        header.Children.Add(new FontIcon { Glyph = "\uE945", FontSize = 12, Opacity = 0.85 });
        header.Children.Add(new TextBlock
        {
            Text = name,
            Style = AppTextStyle("CaptionTextStyle"),
            FontWeight = Microsoft.UI.Text.FontWeights.Medium,
        });
        header.Children.Add(new TextBlock { Text = "·", Style = AppTextStyle("CaptionTextStyle"), Opacity = 0.45 });
        header.Children.Add(new TextBlock
        {
            Text = count == 0
                ? DetText("没有启动成员", "No members started")
                : DetFormat("{0} 个成员", "{0} members", count),
            Style = AppTextStyle("HintTextStyle"),
        });
        header.Children.Add(new TextBlock { Text = "·", Style = AppTextStyle("CaptionTextStyle"), Opacity = 0.45 });
        header.Children.Add(new TextBlock { Text = status, Style = AppTextStyle("HintTextStyle") });
        root.Children.Add(header);

        if (st is null || st.Members.Count == 0) return;

        var expanded = false;
        var list = new StackPanel { Spacing = Sp2, Margin = new Thickness(22, Sp2, 0, 0), Visibility = Visibility.Collapsed };
        foreach (var m in st.Members)
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6 };
            var outcomeBrush = m.Outcome switch
            {
                "completed" => ThemeBrush("SuccessBrush"),
                "failed" => ThemeBrush("ErrorBrush"),
                "cancelled" => ThemeBrush("WarningBrush"),
                null => ThemeBrush("InfoBrush"),
                _ => ThemeBrush("TextSecondaryBrush"),
            };
            row.Children.Add(MakeChip(
                m.Outcome switch
                {
                    "completed" => DetText("已完成", "Completed"),
                    "failed" => DetText("失败", "Failed"),
                    "cancelled" => DetText("已取消", "Cancelled"),
                    null => DetText("运行中", "Running"),
                    _ => m.Outcome!,
                },
                outcomeBrush, mono: false, id: "WorkflowMemberStatus"));
            var label = m.Label.Length > 0 ? m.Label : DetText("空成员名", "Empty member name");
            if (m.Phase is { Length: > 0 } ph) label = ph + " · " + label;
            row.Children.Add(new TextBlock
            {
                Text = label,
                Style = AppTextStyle("HintTextStyle"),
                TextTrimming = TextTrimming.CharacterEllipsis,
            });
            list.Children.Add(row);
        }
        root.Children.Add(list);

        var toggle = new Button
        {
            Content = new TextBlock
            {
                Text = DetFormat("{0} 个成员", "{0} members", count),
                Style = AppTextStyle("CaptionTextStyle"),
            },
            Style = AppStyle("CompactButtonStyle"),
            HorizontalAlignment = HorizontalAlignment.Left,
            Margin = new Thickness(0, Sp2, 0, 0),
        };
        Aut(toggle, "WorkflowMembersToggle", DetFormat("{0} 个成员", "{0} members", count));
        toggle.Click += (_, _) =>
        {
            expanded = !expanded;
            list.Visibility = expanded ? Visibility.Visible : Visibility.Collapsed;
            ((TextBlock)toggle.Content!).Text = expanded
                ? DetText("收起成员", "Hide members")
                : DetFormat("{0} 个成员", "{0} members", count);
        };
        root.Children.Add(toggle);
    }

    private void BuildSessionRecallCard(StackPanel root, ChatBubble bubble)
    {
        var st = DetailState(bubble);
        var header = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6, VerticalAlignment = VerticalAlignment.Center };
        header.Children.Add(new FontIcon { Glyph = "\uE8F2", FontSize = 12, Opacity = 0.85 });

        // 「来自会话 {session}」/「引用会话 · {labels}」——可点开
        if (st.RelaySessionId is { Length: > 0 } relay)
        {
            var link = new HyperlinkButton
            {
                Content = DetFormat("来自会话 {0}", "From session {0}", ShortSessionLabel(relay)),
                Padding = new Thickness(0),
                Margin = new Thickness(0),
                FontSize = 12,
            };
            Aut(link, "SessionRecallLink", DetFormat("来自会话 {0}", "From session {0}", relay));
            link.Click += (_, _) => _ = OpenReferencedSessionAsync(relay);
            header.Children.Add(link);
        }
        else if (st.References.Count > 0)
        {
            var labels = string.Join(DetText("、", ", "), st.References.Select(r => r.Label));
            var link = new HyperlinkButton
            {
                Content = DetFormat("引用会话 · {0}", "Referenced session · {0}", labels),
                Padding = new Thickness(0),
                Margin = new Thickness(0),
                FontSize = 12,
            };
            Aut(link, "SessionReferenceLink", DetFormat("引用会话 · {0}", "Referenced session · {0}", labels));
            link.Click += (_, _) => ShowSessionReferenceFlyout(link, st.References);
            header.Children.Add(link);
        }
        else
        {
            header.Children.Add(new TextBlock
            {
                Text = bubble.Text.Length > 0 ? bubble.Text : DetText("跨会话召回", "Session recall"),
                Style = AppTextStyle("CaptionTextStyle"),
            });
        }
        root.Children.Add(header);

        // 召回明细（保留/省略）
        if (st.ContextEntries.Count > 0)
        {
            var body = new StackPanel { Spacing = Sp2, Margin = new Thickness(18, Sp2, 0, 0) };
            foreach (var (title, text) in st.ContextEntries)
            {
                if (title.Length > 0)
                {
                    body.Children.Add(new TextBlock
                    {
                        Text = title,
                        Style = AppTextStyle("CaptionTextStyle"),
                        FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
                    });
                }
                if (text.Length > 0)
                {
                    body.Children.Add(new TextBlock
                    {
                        Text = text.Length > 240 ? text[..240] + "…" : text,
                        Style = AppTextStyle("HintTextStyle"),
                        TextWrapping = TextWrapping.Wrap,
                    });
                }
            }
            root.Children.Add(body);
        }
    }

    private void BuildContextCard(StackPanel root, ChatBubble bubble)
    {
        var st = DetailState(bubble);
        var title = bubble.ToolName == "system-prompt"
            ? (st.SystemPromptUpdate
                ? DetText("系统提示词更新", "System prompt update")
                : DetText("系统提示词", "System prompt"))
            : DetText("上下文注入", "Context injection");

        var expandable = st.SystemPromptText is { Length: > 0 } || st.ContextEntries.Count > 0;
        var summary = "";
        if (st.ContextEntries.Count > 0)
        {
            var first = st.ContextEntries[0];
            summary = first.Item1.Length > 0 ? first.Item1 : first.Item2;
            if (summary.Length > 48) summary = summary[..48] + "…";
        }
        else if (bubble.Text.Contains("·", StringComparison.Ordinal))
        {
            summary = bubble.Text[(bubble.Text.IndexOf('·') + 1)..].Trim();
        }

        var header = new ToggleButton
        {
            Background = new SolidColorBrush(Colors.Transparent),
            BorderThickness = new Thickness(0),
            Padding = new Thickness(Sp4, Sp2, Sp2, Sp2),
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Left,
            IsEnabled = expandable,
        };
        Aut(header, "ContextExpand", title);
        var inner = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6, VerticalAlignment = VerticalAlignment.Center };
        var chevron = new FontIcon { Glyph = "\uE76B", FontSize = 10 };
        inner.Children.Add(chevron);
        inner.Children.Add(new FontIcon { Glyph = "\uE943", FontSize = 12, Opacity = 0.85 });
        inner.Children.Add(new TextBlock
        {
            Text = title,
            Style = AppTextStyle("CaptionTextStyle"),
            FontWeight = Microsoft.UI.Text.FontWeights.Medium,
        });
        if (summary.Length > 0)
        {
            inner.Children.Add(new TextBlock { Text = "·", Style = AppTextStyle("CaptionTextStyle"), Opacity = 0.45 });
            inner.Children.Add(new TextBlock
            {
                Text = summary,
                Style = AppTextStyle("HintTextStyle"),
                TextTrimming = TextTrimming.CharacterEllipsis,
            });
        }
        header.Content = inner;
        root.Children.Add(header);

        var body = new StackPanel { Margin = new Thickness(22, Sp2, 0, 0), Visibility = Visibility.Collapsed, Spacing = Sp2 };
        foreach (var (entryTitle, text) in st.ContextEntries)
        {
            if (entryTitle.Length > 0)
            {
                body.Children.Add(new TextBlock
                {
                    Text = entryTitle,
                    Style = AppTextStyle("CaptionTextStyle"),
                    FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
                });
            }
            if (text.Length > 0)
            {
                body.Children.Add(new TextBlock
                {
                    Text = text.Length > 2000 ? text[..2000] + "…" : text,
                    Style = AppTextStyle("HintTextStyle"),
                    TextWrapping = TextWrapping.Wrap,
                });
            }
        }
        if (st.SystemPromptText is { Length: > 0 } sp && st.ContextEntries.Count == 0)
        {
            body.Children.Add(MakeCodeBlock(sp.Length > 4000 ? sp[..4000] + "…" : sp, wrap: true, maxHeight: 200));
        }
        if (st.ExtraBlocks.Count > 0)
        {
            body.Children.Add(MakeSectionLabel(DetText("附加内容块", "Extra content block")));
            foreach (var (label, json) in st.ExtraBlocks)
            {
                body.Children.Add(MakeCodeBlock(label + "\n" + PrettyJson(json), wrap: true, maxHeight: 120));
            }
        }
        root.Children.Add(body);
        header.Checked += (_, _) => { body.Visibility = Visibility.Visible; chevron.Glyph = "\uE76C"; };
        header.Unchecked += (_, _) => { body.Visibility = Visibility.Collapsed; chevron.Glyph = "\uE76B"; };
    }

    private static string ShortSessionLabel(string sessionId)
        => sessionId.Length <= 12 ? sessionId : sessionId[..8] + "…";

    private async Task OpenReferencedSessionAsync(string sessionId)
    {
        if (sessionId.Length == 0) return;
        SessionVm? vm = null;
        foreach (var s in _sessions)
        {
            if (s.SessionId == sessionId) { vm = s; break; }
        }
        if (vm is null)
        {
            try { await RefreshSessionsAsync(); } catch (Exception) { }
            foreach (var s in _sessions)
            {
                if (s.SessionId == sessionId) { vm = s; break; }
            }
        }
        if (vm is not null)
        {
            await OpenSessionAsync(vm);
        }
        else
        {
            AppendSystemMessage(DetFormat("找不到会话 {0}", "Session {0} not found", sessionId));
        }
    }

    private void ShowSessionReferenceFlyout(Control anchor, List<(string SessionId, string Label)> refs)
    {
        var panel = new StackPanel { Spacing = Sp4, MinWidth = 220 };
        foreach (var (sid, label) in refs)
        {
            var link = new HyperlinkButton
            {
                Content = label,
                Padding = new Thickness(0),
                HorizontalAlignment = HorizontalAlignment.Left,
            };
            Aut(link, "SessionRefItem_" + label, label);
            var captured = sid;
            link.Click += (_, _) =>
            {
                if (anchor is Button ab && ab.Flyout is Flyout f) f.Hide();
                _ = OpenReferencedSessionAsync(captured.Length > 0 ? captured : label);
            };
            panel.Children.Add(link);
        }
        var flyout = new Flyout { Content = panel };
        if (anchor is Button b) b.Flyout = flyout;
        flyout.ShowAt(anchor);
    }

    // ---------------- P1-1 Details 面板 ----------------

    private Button MakeMessageDetailsButton(ChatBubble bubble)
    {
        var tip = DetText("消息详情", "Message details");
        var button = new Button
        {
            Width = SmallButtonSize,
            Height = SmallButtonSize,
            MinWidth = 0,
            Padding = new Thickness(0),
            CornerRadius = RadSmall,
            VerticalAlignment = VerticalAlignment.Center,
            Content = new FontIcon { Glyph = "\uE946", FontSize = GlyphBody }, // Info
        };
        ToolTipService.SetToolTip(button, tip);
        Aut(button, "MessageDetailsButton", tip);
        button.Click += (_, _) => ShowMessageDetailsFlyout(button, bubble);
        return button;
    }

    private void ShowMessageDetailsFlyout(Control anchor, ChatBubble bubble)
    {
        var st = _messageDetails.TryGetValue(bubble, out var s) ? s : null;
        var panel = new StackPanel { Spacing = Sp6, MinWidth = 280, MaxWidth = 420 };

        panel.Children.Add(MakeSectionLabel(DetText("元数据", "Metadata")));
        var meta = new StackPanel { Spacing = Sp2 };
        void Field(string zhKey, string enKey, string value)
        {
            if (value.Length == 0) return;
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6 };
            row.Children.Add(new TextBlock
            {
                Text = DetText(zhKey, enKey),
                Style = AppTextStyle("HintTextStyle"),
                MinWidth = 72,
            });
            row.Children.Add(new TextBlock
            {
                Text = value,
                Style = AppTextStyle("CaptionTextStyle"),
                TextWrapping = TextWrapping.Wrap,
            });
            meta.Children.Add(row);
        }
        Field("角色", "Role", bubble.Role);
        if (bubble.Turn > 0) Field("轮次", "Turn", bubble.Turn.ToString());
        if (bubble.Seq > 0) Field("序号", "Seq", bubble.Seq.ToString());
        if (bubble.Time > 0) Field("时间", "Time", FormatMessageClock(bubble.Time));
        if (bubble.Model.Length > 0) Field("模型", "Model", bubble.Model);
        else if (st?.Model is { Length: > 0 } m) Field("模型", "Model", m);
        if (st?.Provider is { Length: > 0 } p) Field("提供方", "Provider", p);
        if (st?.ContextWindow is > 0) Field("上下文窗口", "Context window", st.ContextWindow!.Value.ToString());
        if (bubble.DurationMs > 0) Field("用时", "Duration", FormatTurnDuration(bubble.DurationMs));
        if (bubble.Tokens > 0) Field("用量", "Tokens", FormatTokensCompact(bubble.Tokens) + " tok");
        if (bubble.MessageId is { Length: > 0 } mid) Field("消息 ID", "Message ID", mid);
        if (bubble.ToolName is { Length: > 0 } tn) Field("工具", "Tool", tn);
        if (st?.SystemPromptUpdate == true) Field("系统提示词", "System prompt", DetText("已更新", "updated"));
        panel.Children.Add(meta);

        if (st is { ExtraBlocks.Count: > 0 })
        {
            panel.Children.Add(MakeSectionLabel(DetText("附加内容块", "Extra content block")));
            foreach (var (label, json) in st.ExtraBlocks)
            {
                panel.Children.Add(MakeCodeBlock(label + "\n" + PrettyJson(json), wrap: true, maxHeight: 100));
            }
        }

        if (st is { ContextEntries.Count: > 0 })
        {
            panel.Children.Add(MakeSectionLabel(DetText("上下文注入", "Context injection")));
            var list = new StackPanel { Spacing = Sp2 };
            foreach (var (title, text) in st.ContextEntries)
            {
                if (title.Length > 0)
                {
                    list.Children.Add(new TextBlock
                    {
                        Text = title,
                        Style = AppTextStyle("CaptionTextStyle"),
                        FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
                    });
                }
                if (text.Length > 0)
                {
                    list.Children.Add(new TextBlock
                    {
                        Text = text.Length > 400 ? text[..400] + "…" : text,
                        Style = AppTextStyle("HintTextStyle"),
                        TextWrapping = TextWrapping.Wrap,
                    });
                }
            }
            panel.Children.Add(list);
        }

        if (st is { References.Count: > 0 })
        {
            panel.Children.Add(MakeSectionLabel(DetText("跨会话召回", "Session recall")));
            foreach (var (sid, label) in st.References)
            {
                var link = new HyperlinkButton
                {
                    Content = label,
                    Padding = new Thickness(0),
                    HorizontalAlignment = HorizontalAlignment.Left,
                };
                Aut(link, "DetailRef_" + label, label);
                link.Click += (_, _) => _ = OpenReferencedSessionAsync(sid.Length > 0 ? sid : label);
                panel.Children.Add(link);
            }
        }

        if (st?.RelaySessionId is { Length: > 0 } relay)
        {
            panel.Children.Add(MakeSectionLabel(DetText("跨会话中继", "Session relay")));
            var link = new HyperlinkButton
            {
                Content = DetFormat("来自会话 {0}", "From session {0}", relay),
                Padding = new Thickness(0),
                HorizontalAlignment = HorizontalAlignment.Left,
            };
            Aut(link, "DetailRelay", link.Content.ToString() ?? "");
            link.Click += (_, _) => _ = OpenReferencedSessionAsync(relay);
            panel.Children.Add(link);
        }

        if (st?.SystemPromptText is { Length: > 0 } sp)
        {
            panel.Children.Add(MakeSectionLabel(DetText("系统提示词", "System prompt")));
            panel.Children.Add(MakeCodeBlock(sp.Length > 2000 ? sp[..2000] + "…" : sp, wrap: true, maxHeight: 140));
        }

        // 复制 JSON（P1-27 变体）
        var copyRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp4 };
        var payload = BuildDetailJson(bubble, st);
        foreach (var (zh, en, text) in new[]
        {
            ("复制格式化 JSON", "Copy pretty JSON", PrettyJson(payload)),
            ("复制紧凑 JSON", "Copy compact JSON", CompactJson(payload)),
            ("复制属性路径", "Copy property path", DetailPropertyPath(bubble)),
        })
        {
            var b = new Button { Content = DetText(zh, en), Style = AppStyle("CompactButtonStyle") };
            Aut(b, "DetailCopy_" + en.Replace(' ', '_'), DetText(zh, en));
            var captured = text;
            var label = DetText(zh, en);
            b.Click += (_, _) =>
            {
                SetClipboardText(captured);
                b.Content = DetText("已复制", "Copied");
                var timer = DispatcherQueue.CreateTimer();
                timer.Interval = TimeSpan.FromSeconds(1);
                timer.IsRepeating = false;
                timer.Tick += (_, _) => b.Content = label;
                timer.Start();
            };
            copyRow.Children.Add(b);
        }
        panel.Children.Add(copyRow);

        var flyout = new Flyout
        {
            Content = new ScrollViewer
            {
                Content = panel,
                MaxHeight = 480,
                VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            },
        };
        if (anchor is Button db) db.Flyout = flyout;
        flyout.ShowAt(anchor);
    }

    private static string BuildDetailJson(ChatBubble bubble, MessageDetailState? st)
    {
        using var stream = new System.IO.MemoryStream();
        using (var writer = new Utf8JsonWriter(stream, new JsonWriterOptions { Indented = false }))
        {
            writer.WriteStartObject();
            writer.WriteString("role", bubble.Role);
            writer.WriteString("text", bubble.Text);
            if (bubble.Turn > 0) writer.WriteNumber("turn", bubble.Turn);
            if (bubble.Seq > 0) writer.WriteNumber("seq", bubble.Seq);
            if (bubble.Time > 0) writer.WriteNumber("time", bubble.Time);
            if (bubble.Model.Length > 0) writer.WriteString("model", bubble.Model);
            if (bubble.MessageId is { Length: > 0 } mid) writer.WriteString("messageId", mid);
            if (bubble.Tokens > 0) writer.WriteNumber("tokens", bubble.Tokens);
            if (bubble.ToolName is { Length: > 0 } tn) writer.WriteString("tool", tn);
            if (bubble.ToolArgs is { Length: > 0 } ta && JsonDocument.Parse(ta) is { } doc)
            {
                writer.WritePropertyName("arguments");
                doc.RootElement.WriteTo(writer);
            }
            if (st is not null)
            {
                if (st.ExtraBlocks.Count > 0)
                {
                    writer.WriteStartArray("extraBlocks");
                    foreach (var (_, json) in st.ExtraBlocks)
                    {
                        try
                        {
                            using var d = JsonDocument.Parse(json);
                            d.RootElement.WriteTo(writer);
                        }
                        catch (Exception) { writer.WriteStringValue(json); }
                    }
                    writer.WriteEndArray();
                }
                if (st.References.Count > 0)
                {
                    writer.WriteStartArray("references");
                    foreach (var (sid, label) in st.References)
                    {
                        writer.WriteStartObject();
                        writer.WriteString("sessionId", sid);
                        writer.WriteString("label", label);
                        writer.WriteEndObject();
                    }
                    writer.WriteEndArray();
                }
            }
            writer.WriteEndObject();
        }
        return System.Text.Encoding.UTF8.GetString(stream.ToArray());
    }

    private static string DetailPropertyPath(ChatBubble bubble) => bubble.ToolName switch
    {
        { Length: > 0 } => "tool." + bubble.ToolName,
        _ => "message",
    };

    private void SetClipboardText(string text)
    {
        try
        {
            var package = new Windows.ApplicationModel.DataTransfer.DataPackage
            {
                RequestedOperation = Windows.ApplicationModel.DataTransfer.DataPackageOperation.Copy,
            };
            package.SetText(text ?? "");
            Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(package);
        }
        catch (Exception) { }
    }

    private static string CompactJson(string raw)
    {
        if (string.IsNullOrEmpty(raw)) return "";
        try
        {
            using var doc = JsonDocument.Parse(raw);
            return JsonSerializer.Serialize(doc.RootElement, new JsonSerializerOptions { WriteIndented = false });
        }
        catch (Exception)
        {
            return raw;
        }
    }
}
