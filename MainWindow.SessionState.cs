// ---------------- 会话状态域（P1-B：I-1 投影补消费 + P1-2/3/13/15/16） ----------------
// 对齐官方 @deepseek-ai/dsh-client-ui-workspace / dsh-client-ui-jobs / dsh-client-ui-goal /
// dsh-client-ui-conversation 的会话状态语义：
//   · todos 投影 → Todo 清单（MainWindow.Todos.cs）
//   · schedule 投影（wire = active[] 数组；state = {active[]}）→ 列表「有活动定时任务」
//   · plan-review 交互 → 列表/状态条「计划待审」
//   · 目标进行中 → composer 目标指令提示行（edit / pause / resume / clear）
//   · jobs 作业停止钮 → 无 jobs 级 RPC，走 session/cancel 并标明会话级
//
// 线格式核实（Kernel/dsh/node_modules/@deepseek-ai）：
//   schedule wire.view = state.active（数组）——官方 hasActiveSchedule 读 projectionValues.schedule.length。
//   旧 ApplyScheduleProjection 只认 state 对象 {active:[]}，wire 数组会静默丢弃；这里两种都吃。
//   todos wire = TodoItem[] | null（dsh-tool-todo todosProjectionSchema）。
//   jobView 无操作字段（id/kind/label/status/detail/startedAt/finishedAt）；job_kill 是模型工具，
//   不是 remote RPC——壳侧停止只能 session/cancel（会话级）。

using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using Blade2.Dsh;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;

namespace Blade2;

public sealed partial class MainWindow
{
    /// <summary>todos 投影一条（dsh-tool-todo TodoItem）。</summary>
    private sealed class TodoItemVm
    {
        public string Content { get; init; } = "";
        /// <summary>pending | in_progress | completed。</summary>
        public string Status { get; init; } = "pending";
    }

    /// <summary>当前会话 todos 投影缓存（null = 尚未写过 / 本轮已清空）。</summary>
    private List<TodoItemVm>? _todos;
    private readonly object _todosLock = new();

    /// <summary>本地已请求停止的作业 id（session/cancel 发出后、内核 jobs 帧尚未回 stopping 之前）。</summary>
    private readonly HashSet<string> _jobsStopRequested = new(StringComparer.Ordinal);

    /// <summary>composer 目标指令提示行（GoalBar 内第二行，代码挂接）。</summary>
    private TextBlock? _goalCommandHint;

    // ---------------- todos 投影消费（I-1） ----------------

    /// <summary>
    /// todos 投影 → 清单缓存。wire = TodoItem[] | null；null/缺失 = 本回合尚无清单。
    /// 调用方持 _projectionLock（与 plan/permissions 同一解析入口）或已单线程串行。
    /// </summary>
    private void ApplyTodosProjection(JsonElement value)
    {
        List<TodoItemVm>? todos = null;
        if (value.ValueKind == JsonValueKind.Array)
        {
            todos = ParseTodoItems(value);
        }
        else if (value.ValueKind == JsonValueKind.Object && value.TryGetProperty("todos", out var nested) &&
                 nested.ValueKind == JsonValueKind.Array)
        {
            todos = ParseTodoItems(nested);
        }
        lock (_todosLock)
        {
            _todos = todos;
        }
        PostUi(RefreshTodosEntryBadge);
    }

    private static List<TodoItemVm> ParseTodoItems(JsonElement array)
    {
        var list = new List<TodoItemVm>();
        foreach (var item in array.EnumerateArray())
        {
            if (item.ValueKind != JsonValueKind.Object)
            {
                continue;
            }
            var content = item.TryGetProperty("content", out var c) && c.ValueKind == JsonValueKind.String
                ? c.GetString() ?? "" : "";
            if (content.Length == 0)
            {
                continue;
            }
            var status = item.TryGetProperty("status", out var s) && s.ValueKind == JsonValueKind.String
                ? s.GetString() ?? "pending" : "pending";
            if (status is not ("pending" or "in_progress" or "completed"))
            {
                status = "pending";
            }
            list.Add(new TodoItemVm { Content = content, Status = status });
        }
        return list;
    }

    /// <summary>会话切换时清空 todos（与 goal/schedule 同作用域）。</summary>
    private void ResetTodosProjection()
    {
        lock (_todosLock)
        {
            _todos = null;
        }
        PostUi(RefreshTodosEntryBadge);
    }

    private List<TodoItemVm>? SnapshotTodos()
    {
        lock (_todosLock)
        {
            return _todos is null ? null : _todos.ToList();
        }
    }

    // ---------------- schedule 投影补消费（I-1：wire 数组 + state 对象） ----------------

    /// <summary>
    /// schedule 投影 → 记录缓存 + 列表「有活动定时任务」标记。
    /// wire（dsh-schedule scheduleProjectionDefinition.wire.view）= state.active（数组）；
    /// session/control 旧路径可能给 state 对象 {inheritedEventCount, active, seenIds}——两种都解析。
    /// </summary>
    private void ApplyScheduleProjectionWire(JsonElement value)
    {
        var records = new List<(string Id, string Kind, string Prompt, string ScheduledAt, long EverySeconds)>();
        if (value.ValueKind == JsonValueKind.Array)
        {
            ParseScheduleRecordsInto(value, records);
        }
        else if (value.ValueKind == JsonValueKind.Object &&
                 value.TryGetProperty("active", out var active) && active.ValueKind == JsonValueKind.Array)
        {
            ParseScheduleRecordsInto(active, records);
        }
        lock (_scheduleLock)
        {
            _scheduleRecords = records;
            _scheduleSeen = true;
        }
        var sid = Volatile.Read(ref _activeSessionId);
        if (sid is { Length: > 0 })
        {
            lock (_sessionStateLock)
            {
                _sessionHasSchedule[sid] = records.Count > 0;
            }
        }
    }

    private static void ParseScheduleRecordsInto(
        JsonElement array,
        List<(string Id, string Kind, string Prompt, string ScheduledAt, long EverySeconds)> records)
    {
        foreach (var record in array.EnumerateArray())
        {
            var id = record.TryGetProperty("id", out var i) && i.ValueKind == JsonValueKind.String ? i.GetString() ?? "" : "";
            var prompt = record.TryGetProperty("prompt", out var p) && p.ValueKind == JsonValueKind.String ? p.GetString() ?? "" : "";
            if (id.Length == 0 && prompt.Length == 0)
            {
                continue;
            }
            records.Add((
                id,
                record.TryGetProperty("kind", out var k) && k.ValueKind == JsonValueKind.String ? k.GetString() ?? "" : "",
                prompt,
                record.TryGetProperty("scheduledAt", out var s) && s.ValueKind == JsonValueKind.String ? s.GetString() ?? "" : "",
                record.TryGetProperty("everySeconds", out var es) && es.ValueKind == JsonValueKind.Number ? (long)es.GetDouble() : 0));
        }
    }

    /// <summary>从 session/list 的 projections.values 提取 schedule 活动态（列表标记用）。</summary>
    private static bool ScheduleActiveFromProjection(JsonElement schedule)
    {
        if (schedule.ValueKind == JsonValueKind.Array)
        {
            return schedule.GetArrayLength() > 0;
        }
        if (schedule.ValueKind == JsonValueKind.Object &&
            schedule.TryGetProperty("active", out var active) && active.ValueKind == JsonValueKind.Array)
        {
            return active.GetArrayLength() > 0;
        }
        return false;
    }

    // ---------------- 会话列表状态（P1-3 计划待审 / P1-15 有活动定时任务） ----------------

    private readonly object _sessionStateLock = new();
    /// <summary>sessionId → 是否有活动定时任务（schedule 投影 active 非空）。</summary>
    private readonly Dictionary<string, bool> _sessionHasSchedule = new(StringComparer.Ordinal);
    /// <summary>sessionId → 待交互类型：plan-review | question | approval（官方 pendingInteraction 口径）。</summary>
    private readonly Dictionary<string, string> _sessionPendingKind = new(StringComparer.Ordinal);

    /// <summary>登记/清除会话待交互（计划待审 / 等待回答 / 等待审批）。</summary>
    private void SetSessionPendingKind(string sessionId, string? kind)
    {
        if (sessionId.Length == 0)
        {
            return;
        }
        lock (_sessionStateLock)
        {
            if (kind is null)
            {
                _sessionPendingKind.Remove(sessionId);
            }
            else
            {
                _sessionPendingKind[sessionId] = kind;
            }
        }
        PostUi(() =>
        {
            RefreshSessionStateBar();
            RequestSessionsRefresh();
        });
    }

    /// <summary>官方 sessionStatuses 的 pendingInteraction 优先级：approval > plan-review > question。</summary>
    private string? PeekSessionPendingKind(string sessionId)
    {
        // 当前会话以实时提问/审批为唯一真相（投影不到交互态），避免残留标记
        if (sessionId == Volatile.Read(ref _activeSessionId))
        {
            if (_activeQuestion.ValueKind == JsonValueKind.Object)
            {
                var first = ActiveQuestionFirst();
                if (first.ValueKind == JsonValueKind.Object && Str(first, "id") == "plan-review")
                {
                    return "plan-review";
                }
                return "question";
            }
            if (_pendingApprovalEventId is not null || _approvalQueue.Count > 0)
            {
                return "approval";
            }
            return null;
        }
        lock (_sessionStateLock)
        {
            return _sessionPendingKind.TryGetValue(sessionId, out var kind) ? kind : null;
        }
    }

    /// <summary>waterfall 到达时登记待交互（agentId → plan-review/question/approval）。</summary>
    private void TrackInteractivePending(JsonElement frame, string fallbackKind)
    {
        try
        {
            var agentId = Str(frame, "agentId");
            if (agentId.Length == 0)
            {
                return;
            }
            var kind = fallbackKind;
            if (fallbackKind == "question" &&
                frame.TryGetProperty("request", out var request) &&
                request.TryGetProperty("questions", out var questions) &&
                questions.ValueKind == JsonValueKind.Array && questions.GetArrayLength() > 0 &&
                Str(questions[0], "id") == "plan-review")
            {
                kind = "plan-review";
            }
            // approval 优先级最高：已有更高优先级时不降级
            lock (_sessionStateLock)
            {
                if (_sessionPendingKind.TryGetValue(agentId, out var existing))
                {
                    var rank = (string k) => k switch { "approval" => 3, "plan-review" => 2, "question" => 1, _ => 0 };
                    if (rank(existing) >= rank(kind))
                    {
                        return;
                    }
                }
                _sessionPendingKind[agentId] = kind;
            }
            PostUi(() =>
            {
                RefreshSessionStateBar();
                RequestSessionsRefresh();
            });
        }
        catch (Exception)
        {
            // 登记失败不影响提问/审批主流程
        }
    }

    /// <summary>交互取消/结算后清除对应待交互标记。</summary>
    private void ClearInteractivePending(string eventId)
    {
        if (eventId.Length == 0)
        {
            return;
        }
        // 事件 id 不进字典：这里按当前会话清一次即可（列表刷新会再 Peek）
        var sid = Volatile.Read(ref _activeSessionId);
        if (sid is { Length: > 0 })
        {
            lock (_sessionStateLock)
            {
                _sessionPendingKind.Remove(sid);
            }
        }
        PostUi(() =>
        {
            RefreshSessionStateBar();
            RequestSessionsRefresh();
        });
    }

    private JsonElement ActiveQuestionFirst()
    {
        if (_activeQuestion.ValueKind != JsonValueKind.Object ||
            !_activeQuestion.TryGetProperty("request", out var request) ||
            !request.TryGetProperty("questions", out var questions) ||
            questions.ValueKind != JsonValueKind.Array || questions.GetArrayLength() == 0)
        {
            return default;
        }
        return questions[0];
    }

    /// <summary>会话行状态标签（官方 status.*）：计划待审 / 等待回答 / 等待审批。</summary>
    private string SessionPendingLabel(string kind) => kind switch
    {
        "plan-review" => L("计划待审"),
        "question" => L("等待回答"),
        "approval" => L("等待审批"),
        _ => kind,
    };

    /// <summary>把 session/list 投影里的 schedule/plan 活态灌进列表标记表。</summary>
    private void IngestSessionListProjection(string sessionId, JsonElement projValues)
    {
        if (sessionId.Length == 0 || projValues.ValueKind != JsonValueKind.Object)
        {
            return;
        }
        lock (_sessionStateLock)
        {
            if (projValues.TryGetProperty("schedule", out var schedule))
            {
                _sessionHasSchedule[sessionId] = ScheduleActiveFromProjection(schedule);
            }
        }
    }

    private bool SessionHasActiveSchedule(string sessionId)
    {
        if (sessionId == Volatile.Read(ref _activeSessionId))
        {
            lock (_scheduleLock)
            {
                if (_scheduleSeen)
                {
                    return _scheduleRecords.Count > 0;
                }
            }
        }
        lock (_sessionStateLock)
        {
            return _sessionHasSchedule.TryGetValue(sessionId, out var has) && has;
        }
    }

    // ---------------- P1-13 目标指令提示（composer 上） ----------------

    /// <summary>
    /// 目标进行中时在 GoalBar 内挂第二行提示（官方 conversation hint.goal.active）：
    /// 「当前目标进行中。可输入 edit 修改 / pause 暂停 / resume 继续 / clear 清除」。
    /// 代码挂接，不改 MainWindow.xaml。
    /// </summary>
    private void ApplyGoalCommandHint()
    {
        try
        {
            EnsureGoalCommandHint();
            if (_goalCommandHint is null)
            {
                return;
            }
            (string Id, long Revision, string Phase, string Activation, string Objective, int RoundsStarted)? g;
            lock (_goalLock)
            {
                g = _goalSummary;
            }
            var visible = g is { Phase: "active" or "paused" or "blocked" };
            _goalCommandHint.Visibility = visible ? Visibility.Visible : Visibility.Collapsed;
            if (visible)
            {
                _goalCommandHint.Text = L("当前目标进行中。可输入 edit 修改 / pause 暂停 / resume 继续 / clear 清除");
            }
        }
        catch (Exception)
        {
            // 提示行是附属信息：失败不阻塞 GoalBar
        }
    }

    private void EnsureGoalCommandHint()
    {
        if (_goalCommandHint is not null)
        {
            return;
        }
        if (GoalBar.Child is not Grid goalGrid)
        {
            return;
        }
        var hint = new TextBlock
        {
            Style = AppStyle("CaptionTextStyle"),
            Foreground = ThemeBrush("TextTertiaryBrush"),
            TextWrapping = TextWrapping.Wrap,
            Visibility = Visibility.Collapsed,
            Margin = new Thickness(0, 2, 0, 0),
        };
        Aut(hint, "GoalCommandHint", L("当前目标进行中。可输入 edit 修改 / pause 暂停 / resume 继续 / clear 清除"));
        goalGrid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        Grid.SetRow(hint, goalGrid.RowDefinitions.Count - 1);
        Grid.SetColumn(hint, 0);
        Grid.SetColumnSpan(hint, goalGrid.ColumnDefinitions.Count);
        goalGrid.Children.Add(hint);
        _goalCommandHint = hint;
        // 点提示 = 打开目标管理（与「管理」同入口），方便触达 clear
        var tap = new TappedEventHandler((_, _) => _ = OpenGoalManagerFromHintAsync());
        hint.IsTapEnabled = true;
        hint.Tapped += tap;
    }

    private async Task OpenGoalManagerFromHintAsync()
    {
        try
        {
            await ShowCapabilityGoalAsync();
            if (Volatile.Read(ref _activeSessionId) is { } sid)
            {
                await RefreshGoalBarAsync(sid);
            }
        }
        catch (Exception)
        {
            // async void 边界兜底
        }
    }

    // ---------------- P1-16 jobs 停止（无 jobs 专用 RPC → session/cancel 会话级） ----------------

    /// <summary>
    /// 停止作业。全库 typert.remote-client 无 jobs/stop|kill|cancel；job_kill 只是模型工具。
    /// 壳侧唯一可用取消面 = session/cancel（会话级：停掉本会话当前运行，不只这一条 job）。
    /// 点击后本地标记「正在停止」，等 jobs 帧回 stopping/killed 再交还内核状态。
    /// </summary>
    private async Task StopJobViaSessionCancelAsync(JobVm job)
    {
        if (_rpc is null || job.JobId.Length == 0)
        {
            return;
        }
        if (job.Status is not ("running" or "stopping") || job.StopRequested)
        {
            return;
        }
        var sid = Volatile.Read(ref _activeSessionId);
        if (sid is null)
        {
            return;
        }
        job.StopRequested = true;
        lock (_controlLock)
        {
            _jobsStopRequested.Add(job.JobId);
        }
        try
        {
            await _rpc.CallOkAsync("session/cancel", new { request = new { sessionId = sid } });
        }
        catch (DshRpcException ex)
        {
            job.StopRequested = false;
            lock (_controlLock)
            {
                _jobsStopRequested.Remove(job.JobId);
            }
            _ = ShowErrorAsync(LF("取消失败：{0}", ex.Message));
        }
        PostUi(RefreshJobsPanel);
    }

    /// <summary>作业行是否可点停止：仅 live 且尚未请求。</summary>
    private static bool JobCanStop(JobVm job) =>
        (job.Status is "running" or "stopping") && !job.StopRequested;

    /// <summary>作业行停止钮（挂到 MakeJobRow 右侧）。会话级语义写进 UIA/ToolTip。</summary>
    private Button MakeJobStopButton(JobVm job)
    {
        var tip = L("停止（会话级取消：将停止本会话当前运行）");
        var button = Aut(new Button
        {
            Content = new FontIcon { Glyph = "\uE71A", FontSize = GlyphCaption },
            Style = AppStyle("IconButtonStyle"),
            Background = new Microsoft.UI.Xaml.Media.SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            Padding = new Thickness(4),
            MinWidth = 0,
            Width = 28,
            Height = 28,
            VerticalAlignment = VerticalAlignment.Center,
            IsEnabled = JobCanStop(job),
        }, $"JobStop_{job.JobId}", tip);
        ToolTipService.SetToolTip(button, tip);
        button.Click += async (_, _) =>
        {
            try
            {
                await StopJobViaSessionCancelAsync(job);
                BuildJobsFlyout();
            }
            catch (Exception)
            {
                // async void 边界兜底
            }
        };
        return button;
    }

    /// <summary>Jobs 停止后清理本地标记（内核帧已给出终态时）。</summary>
    private void PruneJobStopRequested(IEnumerable<JobVm> jobs)
    {
        var liveIds = jobs.Select(j => j.JobId).ToHashSet(StringComparer.Ordinal);
        lock (_controlLock)
        {
            _jobsStopRequested.RemoveWhere(id =>
                !liveIds.Contains(id) ||
                jobs.Any(j => j.JobId == id && j.Status is "completed" or "killed" or "failed"));
        }
        foreach (var job in jobs)
        {
            if (job.Status is "stopping" or "completed" or "killed" or "failed")
            {
                job.StopRequested = false;
            }
            else
            {
                lock (_controlLock)
                {
                    job.StopRequested = _jobsStopRequested.Contains(job.JobId);
                }
            }
        }
    }

    // ---------------- 会话头 Todo 入口角标（有清单才亮） ----------------

    private Button? _todosEntryButton;

    /// <summary>把「任务」挂进 ComposerAddFlyout（与轨迹同构，代码挂接不改 XAML）。</summary>
    private void EnsureTodosUiEntry()
    {
        if (_todosEntryButton is not null)
        {
            return;
        }
        try
        {
            var item = new MenuFlyoutItem { Text = L("任务") };
            item.Click += async (_, _) =>
            {
                if (_capabilityPanelOpen) return;
                _capabilityPanelOpen = true;
                try { await ShowTodosPanelAsync(); }
                catch (Exception ex)
                {
                    try { await ShowErrorAsync(ex.Message); }
                    catch (Exception) { }
                }
                finally { _capabilityPanelOpen = false; }
            };
            Aut(item, "TodosMenuItem", L("任务"));
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
            // 角标按钮进会话头动作区（JobsTrigger 旁）：有 todos 才显示
            _todosEntryButton = Aut(new Button
            {
                Background = new Microsoft.UI.Xaml.Media.SolidColorBrush(Microsoft.UI.Colors.Transparent),
                BorderThickness = new Thickness(0),
                CornerRadius = new CornerRadius(4),
                Padding = new Thickness(8, 4, 8, 4),
                Visibility = Visibility.Collapsed,
                Content = new StackPanel
                {
                    Orientation = Orientation.Horizontal,
                    Spacing = TokenDouble("Space6", 6),
                    Children =
                    {
                        new FontIcon { Glyph = "\uE73E", FontSize = TokenDouble("GlyphSizeBody", 16) },
                        new TextBlock
                        {
                            Text = L("任务"),
                            Style = AppStyle("CaptionTextStyle"),
                            VerticalAlignment = VerticalAlignment.Center,
                        },
                    },
                },
            }, "TodosTriggerButton", L("任务"));
            ToolTipService.SetToolTip(_todosEntryButton, L("任务"));
            _todosEntryButton.Click += async (_, _) =>
            {
                if (_capabilityPanelOpen) return;
                _capabilityPanelOpen = true;
                try { await ShowTodosPanelAsync(); }
                catch (Exception ex)
                {
                    try { await ShowErrorAsync(ex.Message); }
                    catch (Exception) { }
                }
                finally { _capabilityPanelOpen = false; }
            };
            // 挂到会话头动作 StackPanel（JobsTriggerButton 的父级）
            if (JobsTriggerButton.Parent is StackPanel headerActions)
            {
                headerActions.Children.Insert(Math.Max(0, headerActions.Children.IndexOf(JobsTriggerButton)), _todosEntryButton);
            }
            RefreshTodosEntryBadge();
        }
        catch (Exception)
        {
            // 入口挂接失败不阻塞聊天
        }
    }

    private void RefreshTodosEntryBadge()
    {
        try
        {
            EnsureTodosUiEntry();
            if (_todosEntryButton is null)
            {
                return;
            }
            var todos = SnapshotTodos();
            var show = todos is { Count: > 0 };
            _todosEntryButton.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
            if (show && todos is { } list)
            {
                var done = list.Count(t => t.Status == "completed");
                var active = list.Count(t => t.Status == "in_progress");
                var pending = list.Count - done - active;
                var name = L("任务") + "：" + TodoProgressLabel(done, active, pending);
                Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(_todosEntryButton, name);
                ToolTipService.SetToolTip(_todosEntryButton, name);
            }
        }
        catch (Exception)
        {
            // 角标刷新失败不致命
        }
    }

    /// <summary>官方 TodoPanel.progressLabel：· 连接，零计数段省略。</summary>
    private string TodoProgressLabel(int done, int active, int pending)
    {
        var parts = new List<string>();
        if (done > 0) parts.Add(LF("{0} 已完成", done));
        if (active > 0) parts.Add(LF("{0} 进行中", active));
        if (pending > 0) parts.Add(LF("{0} 待处理", pending));
        return parts.Count == 0 ? LF("{0} 待处理", 0) : string.Join(" · ", parts);
    }
}
