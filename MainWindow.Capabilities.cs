using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Blade2;

public sealed partial class MainWindow
{
    private bool _capabilityPanelOpen;
    private string CapabilityText(string zh, string en) => L(zh);

    /// <summary>Native entry points only; the caller attaches this flyout to a button.</summary>
    public MenuFlyout BuildCapabilityMenu()
    {
        var menu = new MenuFlyout();
        void Add(string zh, string en, Func<Task> action)
        {
            var item = new MenuFlyoutItem { Text = CapabilityText(zh, en) };
            item.Click += async (_, _) =>
            {
                if (_capabilityPanelOpen) return;
                _capabilityPanelOpen = true;
                try { await action(); }
                catch (Exception ex)
                {
                    // Only reached outside our ContentDialog; in-dialog errors are inline.
                    try { await ShowErrorAsync(ex.Message); }
                    catch (Exception) { /* Another application dialog may already own XamlRoot. */ }
                }
                finally { _capabilityPanelOpen = false; }
            };
            menu.Items.Add(item);
        }
        Add("目标", "Goal", ShowCapabilityGoalAsync);
        Add("计划（只读）", "Schedules (read-only)", ShowCapabilitySchedulesAsync);
        Add("技能", "Skills", ShowCapabilitySkillsAsync);
        return menu;
    }

    private ContentDialog CapabilityDialog(string title, object content) => new()
    {
        Title = title,
        Content = content,
        CloseButtonText = CapabilityText("关闭", "Close"),
        XamlRoot = Content.XamlRoot,
    };

    private static string CapabilityString(JsonElement value, string key) =>
        value.ValueKind == JsonValueKind.Object && value.TryGetProperty(key, out var field) &&
        field.ValueKind == JsonValueKind.String ? field.GetString() ?? "" : "";

    private string CapabilitySession()
    {
        if (_rpc is null || string.IsNullOrWhiteSpace(_activeSessionId))
            throw new InvalidOperationException(CapabilityText("请先连接内核并选择会话。", "Connect and select a session first."));
        return _activeSessionId;
    }

    private void CheckCapabilitySession(string sessionId)
    {
        if (_activeSessionId != sessionId)
            throw new InvalidOperationException(CapabilityText("会话已切换，请关闭后重新打开。", "Session changed. Close and reopen this panel."));
    }

    private string TargetPanelText(JsonElement goal)
    {
        if (goal.ValueKind is JsonValueKind.Undefined or JsonValueKind.Null)
            return CapabilityText("尚未设置目标", "Not set");
        if (goal.ValueKind != JsonValueKind.Object)
            throw new InvalidOperationException("Unexpected goals/get response.");
        var text = $"{CapabilityString(goal, "phase")} / {CapabilityString(goal, "activation")}\n" +
            $"ID: {CapabilityString(goal, "id")}  revision: {goal.GetProperty("revision")}\n" +
            CapabilityText("已启动轮数：", "Rounds started: ") + goal.GetProperty("roundsStarted");
        if (goal.TryGetProperty("blockedReason", out var reason))
            text += "\n" + CapabilityString(reason, "message");
        return text;
    }

    private async Task ShowCapabilityGoalAsync()
    {
        var sessionId = CapabilitySession();
        var rpc = _rpc!;
        using var lifetime = new CancellationTokenSource();
        var state = new TextBlock { TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true };
        var error = new TextBlock { TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true };
        var objective = new TextBox { Header = CapabilityText("目标内容", "Objective"), AcceptsReturn = true, TextWrapping = TextWrapping.Wrap, MaxHeight = 160 };
        var rounds = new TextBox { Header = CapabilityText("最大轮数（创建时可留空使用内核默认值）", "Maximum rounds (optional on creation)") };
        var actions = new StackPanel { Spacing = 8 };
        var body = new StackPanel { Spacing = 12, Width = 440 };
        body.Children.Add(state);
        body.Children.Add(objective);
        body.Children.Add(rounds);
        body.Children.Add(new TextBlock
        {
            Text = CapabilityText("创建或恢复目标可能启动内核执行；仅在确认目标后操作。", "Creating or resuming a goal may start kernel execution. Act only after confirming the objective."),
            TextWrapping = TextWrapping.Wrap,
        });
        body.Children.Add(actions);
        body.Children.Add(error);
        var dialog = CapabilityDialog(CapabilityText("目标", "Goal"), new ScrollViewer { Content = body, MaxHeight = 540 });
        JsonElement goal = default;
        bool busy = false, loaded = false, closed = false;
        var buttons = new Dictionary<string, Button>();

        void UpdateButtons()
        {
            bool hasGoal = goal.ValueKind == JsonValueKind.Object;
            var phase = CapabilityString(goal, "phase");
            var activation = CapabilityString(goal, "activation");
            foreach (var pair in buttons)
            {
                bool allowed = pair.Key switch
                {
                    "get" => true,
                    "create" => loaded && !hasGoal,
                    "edit" => loaded && hasGoal && phase != "complete",
                    "pause" => loaded && phase == "active" && activation == "armed",
                    "resume" => loaded && (phase == "paused" || phase == "active" && activation == "disarmed"),
                    "complete" => loaded && hasGoal && phase != "complete",
                    _ => false,
                };
                pair.Value.IsEnabled = !busy && !closed && _activeSessionId == sessionId && allowed;
            }
            objective.IsEnabled = rounds.IsEnabled = !busy && loaded;
        }

        async Task Refresh()
        {
            loaded = false;
            goal = default; // Never permit mutation with an unverified/stale CAS ref.
            var next = await rpc.CallOkAsync("goals/get", new { agentId = sessionId }, lifetime.Token);
            CheckCapabilitySession(sessionId);
            if (closed) return;
            goal = next;
            loaded = true;
            state.Text = TargetPanelText(goal);
            if (goal.ValueKind is JsonValueKind.Undefined or JsonValueKind.Null)
            {
                objective.Text = "";
                rounds.Text = "";
                return;
            }
            objective.Text = CapabilityString(goal, "objective");
            rounds.Text = goal.GetProperty("maxGoalRounds").ToString();
        }

        async Task Run(string verb)
        {
            if (busy || closed) return;
            busy = true;
            error.Text = "";
            UpdateButtons();
            bool mutationAttempted = false;
            try
            {
                CheckCapabilitySession(sessionId);
                if (verb == "get") { await Refresh(); return; }
                var request = new Dictionary<string, object>();
                if (verb is "create" or "edit")
                {
                    if (string.IsNullOrWhiteSpace(objective.Text))
                        throw new InvalidOperationException(CapabilityText("目标内容不能为空。", "Objective cannot be empty."));
                    request["objective"] = objective.Text.Trim();
                    if (!string.IsNullOrWhiteSpace(rounds.Text))
                    {
                        if (!int.TryParse(rounds.Text, NumberStyles.None, CultureInfo.InvariantCulture, out var count) || count < 1)
                            throw new InvalidOperationException(CapabilityText("最大轮数必须为正整数。", "Maximum rounds must be a positive integer."));
                        request["maxGoalRounds"] = count;
                    }
                }
                var args = new Dictionary<string, object> { ["agentId"] = sessionId };
                if (verb != "create")
                {
                    if (!loaded || goal.ValueKind != JsonValueKind.Object)
                        throw new InvalidOperationException(CapabilityText("请先刷新目标。", "Refresh the goal first."));
                    args["ref"] = new { id = goal.GetProperty("id").GetString(), revision = goal.GetProperty("revision").Clone() };
                }
                if (verb is "create" or "edit") args["request"] = request;
                mutationAttempted = true;
                await rpc.CallOkAsync("goals/" + verb, args, lifetime.Token);
                await Refresh();
            }
            catch (Exception ex)
            {
                if (!closed) error.Text = ex.Message;
                // Failure may be a revision conflict or an ambiguous transport result.
                // Read once, never retry the mutation with a newer revision automatically.
                if (mutationAttempted && !closed)
                {
                    try { await Refresh(); }
                    catch (Exception refreshError)
                    {
                        loaded = false;
                        if (!closed) error.Text += "\n" + CapabilityText("重新读取失败：", "Refresh failed: ") + refreshError.Message;
                    }
                }
            }
            finally { busy = false; if (!closed) UpdateButtons(); }
        }

        foreach (var entry in new[]
        {
            ("get", "刷新", "Refresh"), ("create", "创建目标", "Create goal"),
            ("edit", "保存编辑", "Save edits"), ("pause", "暂停", "Pause"),
            ("resume", "恢复", "Resume"), ("complete", "标记完成", "Mark complete"),
        })
        {
            var verb = entry.Item1;
            var button = new Button { Content = CapabilityText(entry.Item2, entry.Item3) };
            button.Click += async (_, _) => await Run(verb);
            buttons.Add(verb, button);
            actions.Children.Add(button);
        }
        dialog.Opened += async (_, _) => await Run("get");
        dialog.Closed += (_, _) => { closed = true; lifetime.Cancel(); };
        UpdateButtons();
        await dialog.ShowAsync();
    }

    private async Task ShowCapabilitySchedulesAsync()
    {
        var sessionId = CapabilitySession();
        List<(string Id, string Kind, string Prompt, string ScheduledAt, long EverySeconds)> records;
        bool seen;
        lock (_scheduleLock)
        {
            records = _scheduleRecords.ToList();
            seen = _scheduleSeen;
        }
        // 官方 ScheduleCatalogAction 口径：已到期在前，其余按目标时间升序（同刻稳定）
        var now = DateTimeOffset.UtcNow;
        var ordered = records
            .Select(r => (Record: r, At: DateTimeOffset.TryParse(
                r.ScheduledAt, CultureInfo.InvariantCulture,
                DateTimeStyles.AdjustToUniversal | DateTimeStyles.AssumeUniversal, out var at) ? at : (DateTimeOffset?)null))
            .OrderByDescending(x => x.At is { } at && at <= now)
            .ThenBy(x => x.At ?? DateTimeOffset.MaxValue)
            .ToList();

        var body = new StackPanel { Spacing = 10, Width = 440 };
        body.Children.Add(new TextBlock
        {
            Text = CapabilityText(
                "计划为只读清单（数据来自会话投影 schedule）。此入口不创建、编辑或删除计划，也不调用 schedules RPC。",
                "Schedules are a read-only list (data from the session schedule projection). No schedules are created, edited or deleted; no schedules RPC is called."),
            TextWrapping = TextWrapping.Wrap,
        });
        if (ordered.Count == 0)
        {
            // 没收到过投影 ≠ 没有计划：内核按会话惰性推送投影，两种空必须分开说
            body.Children.Add(new TextBlock
            {
                Text = seen
                    ? CapabilityText("当前会话没有计划。", "No schedules in this session.")
                    : CapabilityText("尚未收到本会话的计划投影（投影随会话活动到达），此时不表示没有计划。", "The schedule projection has not arrived yet (the kernel pushes it lazily); this does not mean there are no schedules."),
                TextWrapping = TextWrapping.Wrap,
                IsTextSelectionEnabled = true,
            });
        }
        foreach (var (record, at) in ordered)
        {
            var row = new StackPanel { Spacing = 2 };
            row.Children.Add(new TextBlock
            {
                Text = record.Prompt,
                TextWrapping = TextWrapping.Wrap,
                IsTextSelectionEnabled = true,
            });
            var meta = ScheduleFrequencyLabel(record.Kind, record.EverySeconds);
            if (at is { } target)
            {
                var local = target.ToLocalTime().ToString("g", CultureInfo.CurrentCulture);
                var left = target - now;
                meta += " · " + local + " · " + (left > TimeSpan.Zero
                    ? LF("还剩 {0}", ScheduleMagnitudeLabel(left))
                    : LF("已过期 {0}", ScheduleMagnitudeLabel(-left)));
            }
            row.Children.Add(new TextBlock
            {
                Text = meta,
                TextWrapping = TextWrapping.Wrap,
                Foreground = ThemeBrush("TextTertiaryBrush"),
                Style = AppStyle("CaptionTextStyle"),
            });
            body.Children.Add(row);
        }
        await CapabilityDialog(
            CapabilityText("计划（只读）", "Schedules (read-only)"),
            new ScrollViewer { Content = body, MaxHeight = 480 }).ShowAsync();
    }

    /// <summary>计划频率标签（官方 formatScheduleFrequency 口径：取能整除的最大自然单位）。</summary>
    private string ScheduleFrequencyLabel(string kind, long everySeconds)
    {
        if (kind != "every" || everySeconds <= 0)
        {
            return L("一次性");
        }
        if (everySeconds % 86400 == 0) return LF("每 {0} 天", everySeconds / 86400);
        if (everySeconds % 3600 == 0) return LF("每 {0} 小时", everySeconds / 3600);
        if (everySeconds % 60 == 0) return LF("每 {0} 分", everySeconds / 60);
        return LF("每 {0} 秒", everySeconds);
    }

    /// <summary>时长的人读标签（最大自然单位，供计划相对时间用）。</summary>
    private string ScheduleMagnitudeLabel(TimeSpan span)
    {
        if (span >= TimeSpan.FromDays(1)) return LF("{0}天", (int)span.TotalDays);
        if (span >= TimeSpan.FromHours(1)) return LF("{0}小时", (int)span.TotalHours);
        if (span >= TimeSpan.FromMinutes(1)) return LF("{0}分钟", (int)span.TotalMinutes);
        return LF("{0}秒", (int)span.TotalSeconds);
    }

    private async Task ShowCapabilitySkillsAsync()
    {
        var sessionId = CapabilitySession();
        var result = await _rpc!.CallOkAsync("skills/list", new { request = new { sessionId } });
        CheckCapabilitySession(sessionId);
        if (result.ValueKind != JsonValueKind.Object || !result.TryGetProperty("skills", out var skills) || skills.ValueKind != JsonValueKind.Array)
            throw new InvalidOperationException(CapabilityText("技能列表响应格式不正确。", "Unexpected skills/list response."));
        var body = new StackPanel { Spacing = 8, Width = 440 };
        body.Children.Add(new TextBlock
        {
            Text = CapabilityText("选择技能只会插入 /名称 到输入框，不会发送或执行。", "Selecting a skill only inserts /name into the input box; it does not send or execute it."),
            TextWrapping = TextWrapping.Wrap,
        });
        string? selected = null;
        var dialog = CapabilityDialog(CapabilityText("技能", "Skills"), new ScrollViewer { Content = body, MaxHeight = 480 });
        var error = new TextBlock { TextWrapping = TextWrapping.Wrap };
        body.Children.Add(error);
        int count = 0;
        foreach (var skill in skills.EnumerateArray())
        {
            var name = CapabilityString(skill, "name");
            if (string.IsNullOrWhiteSpace(name) || name.Any(char.IsWhiteSpace) || name.StartsWith('/')) continue;
            count++;
            var description = CapabilityString(skill, "description");
            var button = new Button
            {
                Content = new TextBlock { Text = "/" + name + (description.Length > 0 ? "\n" + description : ""), TextWrapping = TextWrapping.Wrap },
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Left,
            };
            button.Click += (_, _) =>
            {
                if (_activeSessionId != sessionId)
                {
                    error.Text = CapabilityText("会话已切换，请关闭后重新打开。", "Session changed. Close and reopen this panel.");
                    return;
                }
                selected = name;
                dialog.Hide();
            };
            body.Children.Add(button);
        }
        if (count == 0) body.Children.Add(new TextBlock { Text = CapabilityText("没有可用技能。", "No skills available.") });
        await dialog.ShowAsync();
        if (selected is null) return;
        CheckCapabilitySession(sessionId);
        // Plain prompt text only: preserve the draft and selection, do not call commands/run or session/prompt.
        int start = InputBox.SelectionStart;
        string token = (start > 0 && !char.IsWhiteSpace(InputBox.Text[start - 1]) ? " " : "") + "/" + selected + " ";
        InputBox.SelectedText = token;
        InputBox.SelectionStart = start + token.Length;
        InputBox.SelectionLength = 0;
        InputBox.Focus(FocusState.Programmatic);
    }
}
