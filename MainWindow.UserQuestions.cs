// ---------------- 用户提问（user-questions/request，$events waterfall） ----------------
// P0-4：多题分页导航，对标 @deepseek-ai/dsh-client-ui-user-questions/lib/client.js。
//
// 协议（dsh-api-gateway startRemoteEvent + dsh-api-remotes forwardWaterfall）：
//   内核推  {type:"waterfall", event:"user-questions/request", eventId, agentId, request}
//   壳回传  POST /api/$events/result {clientId, eventId, outcome}
//   outcome 三态：
//     {kind:"result", value:{answers:[{id, selected:[label…], custom?}]}}  一次汇总全部作答（含跳过）
//     {kind:"next"}                                                       本客户端不回答（委托下一监听）
//     {kind:"rejected", error:{name:"UserQuestionError", code:"ASK_CANCELLED", …}}  放弃整组
// request.questions[i] = {id, question, header?, options?:[{label,description?}],
//                         multiSelect?, intent?, detail?}
// 计划评审（exit_plan_mode）走同一通道：questions[0].id == "plan-review"。
//
// 导航状态机（单屏一题，草稿跨页保留）：
//   Idle → Showing(index, drafts[])
//   Prev/Next          保存本页草稿后移动 index（不提交）
//   单选选中            写 selected、清 custom/skipped；非末题自动 Next
//   多选勾选            切换 selected 位、清 skipped
//   自定义文本          写 custom；非多选时清 selected、清 skipped
//   跳过本题            本题 skipped=true（selected=[]/custom=""），前进或末题即提交
//   确认执行            全部 completed（已答或已跳过）→ 一次 result；否则跳到首道未完成
//   放弃整组问题        rejected + ASK_CANCELLED
//   去聊天里说          同放弃整组 + 聚焦 InputBox（不提交空答案）

using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Text.RegularExpressions;
using System.Threading.Tasks;
using Blade2.Dsh;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Blade2;

public sealed partial class MainWindow
{
    /// <summary>当前亮着的提问（同一时刻只亮一条，与审批卡片同策略：入队顺序处理）。</summary>
    private readonly Queue<JsonElement> _questionQueue = new();
    private JsonElement _activeQuestion = default;

    /// <summary>整组问题的草稿（index 对齐 request.questions）：跨页保留，末题一次回传。</summary>
    private sealed class QuestionDraft
    {
        public readonly List<string> Selected = new();
        public string Custom = "";
        public bool Skipped;
    }

    /// <summary>当前页控件引用：翻页前把 UI 写回草稿。</summary>
    private sealed class QuestionPageUi
    {
        public string Id = "";
        public bool Multi;
        public readonly List<(RadioButton Radio, string Label)> Radios = new();
        public readonly List<(CheckBox Check, string Label)> Checks = new();
        public TextBox Custom = new();
    }

    private readonly List<QuestionDraft> _questionDrafts = new();
    private readonly List<JsonElement> _questionItems = new();
    private int _questionIndex;
    private QuestionPageUi? _questionPage;
    private readonly List<Control> _questionControls = new();
    private TextBlock? _questionProgress;
    private TextBlock? _questionFeedback;
    private Button? _questionPrev;
    private Button? _questionNext;
    private Button? _questionSubmit;

    /// <summary>waterfall 帧入口（接收线程）。只处理 user-questions/request。</summary>
    private Task OnUserQuestionAsync(JsonElement frame)
    {
        var eventId = Str(frame, "eventId");
        if (Str(frame, "event") != "user-questions/request" || eventId.Length == 0 ||
            !frame.TryGetProperty("request", out var request) || request.ValueKind != JsonValueKind.Object ||
            !request.TryGetProperty("questions", out var questions) || questions.ValueKind != JsonValueKind.Array ||
            questions.GetArrayLength() == 0)
        {
            return Task.CompletedTask;
        }
        var ownedFrame = frame.Clone();
        RunEventUi(() =>
        {
            if (!_seenInteractiveEventIds.Add(eventId)) return;
            _questionQueue.Enqueue(ownedFrame);
            // 与审批同策：提问（含 exit_plan_mode 计划评审）同样卡住任务等回答，
            // 窗口不在前台时用户看不到卡片，补一条系统通知。
            if (ShouldNotify() && questions.GetArrayLength() > 0)
            {
                var first = questions[0];
                var header = first.TryGetProperty("header", out var headerEl) && headerEl.ValueKind == JsonValueKind.String
                    ? headerEl.GetString()
                    : null;
                var text = first.TryGetProperty("question", out var q) && q.ValueKind == JsonValueKind.String
                    ? q.GetString()
                    : null;
                // 标题只走短标签；「已就绪的计划（链接 …）：下…」这类长 header 若进标题，
                // 会在 toast 标题位被裁成半行。长文案一律进正文（ShellToast 正文可多行）。
                var body = text ?? string.Empty;
                var title = L("需要你的输入");
                if (header is { Length: > 0 } headerText)
                {
                    if (headerText.Length <= 24)
                    {
                        title = headerText;
                    }
                    else
                    {
                        body = body.Length > 0 ? headerText + "\n" + body : headerText;
                    }
                }
                ShellToast.Show(title, body);
            }
            ShowNextQuestion();
        });
        return Task.CompletedTask;
    }

    /// <summary>亮出队首提问（无待答则收起卡片）。</summary>
    private void ShowNextQuestion()
    {
        if (_activeQuestion.ValueKind == JsonValueKind.Object)
        {
            return;
        }
        try
        {
            ResetQuestionState();
            if (_questionQueue.Count == 0)
            {
                QuestionHost.Children.Clear();
                QuestionPanel.Visibility = Visibility.Collapsed;
                return;
            }
            var next = _questionQueue.Dequeue();
            _activeQuestion = next;
            BuildQuestionCard(next);
            QuestionPanel.Visibility = Visibility.Visible;
        }
        catch (Exception)
        {
            // 卡片构造失败不上抛（0xc000027b 教训）
        }
    }

    private void ResetQuestionState()
    {
        _questionDrafts.Clear();
        _questionItems.Clear();
        _questionIndex = 0;
        _questionPage = null;
        _questionControls.Clear();
        _questionProgress = null;
        _questionFeedback = null;
        _questionPrev = null;
        _questionNext = null;
        _questionSubmit = null;
        SetQuestionInputEnabled(true);
    }

    // label 尾缀 “(Recommended)/(推荐)/（推荐）” 只影响展示；回传仍用原始 label。
    private static readonly Regex RecommendedSuffix = new(
        @"\s*(?:\((?:recommended|推荐)\)|（(?:recommended|推荐)）)\s*$",
        RegexOptions.IgnoreCase | RegexOptions.CultureInvariant | RegexOptions.Compiled);

    private static (string Label, bool Recommended) ParseRecommendedLabel(string label)
        => RecommendedSuffix.IsMatch(label)
            ? (RecommendedSuffix.Replace(label, ""), true)
            : (label, false);

    /// <summary>提问卡片：表头 + 当前题（分页）+ 题号指示 + 跳过/去聊天/放弃/确认执行。</summary>
    private void BuildQuestionCard(JsonElement frame)
    {
        QuestionHost.Children.Clear();
        var request = frame.TryGetProperty("request", out var rq) && rq.ValueKind == JsonValueKind.Object ? rq : default;
        var questions = request.ValueKind == JsonValueKind.Object && request.TryGetProperty("questions", out var qs) &&
                        qs.ValueKind == JsonValueKind.Array ? qs : default;
        if (questions.ValueKind != JsonValueKind.Array || questions.GetArrayLength() == 0)
        {
            return;
        }

        _questionItems.Clear();
        _questionDrafts.Clear();
        foreach (var q in questions.EnumerateArray())
        {
            _questionItems.Add(q.Clone());
            _questionDrafts.Add(new QuestionDraft());
        }
        _questionIndex = 0;

        // 表头：plan-review 有专属文案，其余用问题自带的 header 或统一标题
        var first = _questionItems[0];
        var isPlanReview = Str(first, "id") == "plan-review";
        var header = isPlanReview ? L("计划评审") : (Str(first, "header") is { Length: > 0 } h ? h : L("内核提问"));

        var head = new Grid();
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var titleRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp8 };
        titleRow.Children.Add(new FontIcon { Glyph = isPlanReview ? "\uE9D5" : "\uE897", FontSize = GlyphBody });
        titleRow.Children.Add(new TextBlock { Text = header, Style = AppStyle("BodyStrongTextStyle") });
        var agentId = Str(frame, "agentId");
        if (agentId.Length > 0)
        {
            titleRow.Children.Add(new TextBlock
            {
                Text = agentId,
                Style = AppStyle("CodeTextStyle"),
                Opacity = 0.6,
                VerticalAlignment = VerticalAlignment.Center,
                IsTextSelectionEnabled = true,
            });
        }
        Grid.SetColumn(titleRow, 1);
        head.Children.Add(titleRow);

        // 放弃整组问题（官方 nav.cancel → pending.cancel() → ASK_CANCELLED）
        var abandon = Aut(new Button
        {
            Content = L("放弃整组问题"),
            Style = AppStyle("CompactButtonStyle"),
        }, "QuestionAbandonAll", L("放弃整组问题"));
        abandon.Click += OnQuestionAbandonClick;
        _questionControls.Add(abandon);
        Grid.SetColumn(abandon, 2);
        head.Children.Add(abandon);
        QuestionHost.Children.Add(head);

        // 当前题题面（翻页时整体重建）
        var pageHost = new StackPanel { Spacing = Sp8 };
        QuestionHost.Children.Add(pageHost);
        QuestionHost.Tag = pageHost;

        // 底栏：题号导航 | 反馈 | 跳过 / 去聊天 / 确认执行
        var footer = new Grid();
        footer.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        footer.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        footer.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var pager = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = Sp6,
            VerticalAlignment = VerticalAlignment.Center,
        };
        var prev = Aut(new Button
        {
            Content = new FontIcon { Glyph = "\uE76B", FontSize = GlyphBody },
            Style = AppStyle("CompactButtonStyle"),
            IsEnabled = false,
        }, "QuestionNavPrev", L("上一题"));
        ToolTipService.SetToolTip(prev, L("上一题"));
        prev.Click += OnQuestionPrevClick;
        _questionControls.Add(prev);
        _questionPrev = prev;
        pager.Children.Add(prev);

        var progress = new TextBlock
        {
            Text = _questionItems.Count > 0 ? $"1 / {_questionItems.Count}" : "",
            Style = AppStyle("BodyStrongTextStyle"),
            VerticalAlignment = VerticalAlignment.Center,
            MinWidth = 48,
            TextAlignment = Microsoft.UI.Xaml.TextAlignment.Center,
        };
        Aut(progress, "QuestionNavProgress",
            _questionItems.Count > 0 ? LF("第 {0} / {1} 题", 1, _questionItems.Count) : "");
        _questionProgress = progress;
        pager.Children.Add(progress);

        var next = Aut(new Button
        {
            Content = new FontIcon { Glyph = "\uE76C", FontSize = GlyphBody },
            Style = AppStyle("CompactButtonStyle"),
            IsEnabled = _questionItems.Count > 1,
        }, "QuestionNavNext", L("下一题"));
        ToolTipService.SetToolTip(next, L("下一题"));
        next.Click += OnQuestionNextClick;
        _questionControls.Add(next);
        _questionNext = next;
        pager.Children.Add(next);
        Grid.SetColumn(pager, 0);
        footer.Children.Add(pager);

        var feedback = new TextBlock
        {
            Style = AppStyle("CaptionTextStyle"),
            Foreground = ThemeBrush("ErrorBrush"),
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(Sp8, 0, Sp8, 0),
            TextWrapping = TextWrapping.Wrap,
        };
        _questionFeedback = feedback;
        Grid.SetColumn(feedback, 1);
        footer.Children.Add(feedback);

        var actions = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = Sp8,
            VerticalAlignment = VerticalAlignment.Center,
        };
        var skip = Aut(new Button
        {
            Content = L("跳过本题"),
            Style = AppStyle("CompactButtonStyle"),
        }, "QuestionSkip", L("跳过本题"));
        skip.Click += OnQuestionSkipClick;
        _questionControls.Add(skip);
        actions.Children.Add(skip);

        var toChat = Aut(new Button
        {
            Content = L("去聊天里说"),
            Style = AppStyle("CompactButtonStyle"),
        }, "QuestionToChat", L("去聊天里说"));
        toChat.Click += OnQuestionToChatClick;
        _questionControls.Add(toChat);
        actions.Children.Add(toChat);

        // 末题（或单题/计划评审）主键 = 确认执行：一次回传完整 answers
        var submit = Aut(new Button
        {
            Content = L("确认执行"),
            Style = AppStyle("AccentButtonStyle"),
        }, "QuestionSubmitButton", L("确认执行"));
        submit.Click += OnQuestionSubmitClick;
        _questionControls.Add(submit);
        _questionSubmit = submit;
        actions.Children.Add(submit);
        Grid.SetColumn(actions, 2);
        footer.Children.Add(actions);

        QuestionHost.Children.Add(footer);
        RenderQuestionPage();
    }

    /// <summary>把当前页控件写回草稿（翻页 / 提交前调用）。</summary>
    private void CaptureQuestionDraft()
    {
        var page = _questionPage;
        if (page is null || _questionIndex < 0 || _questionIndex >= _questionDrafts.Count) return;
        var draft = _questionDrafts[_questionIndex];
        draft.Selected.Clear();
        foreach (var (radio, label) in page.Radios)
        {
            if (radio.IsChecked == true) draft.Selected.Add(label);
        }
        foreach (var (check, label) in page.Checks)
        {
            if (check.IsChecked == true) draft.Selected.Add(label);
        }
        draft.Custom = (page.Custom.Text ?? "").Trim();
        if (draft.Selected.Count > 0 || draft.Custom.Length > 0)
        {
            draft.Skipped = false;
        }
    }

    private static bool DraftAnswered(QuestionDraft d) => d.Selected.Count > 0 || d.Custom.Trim().Length > 0;
    private static bool DraftCompleted(QuestionDraft d) => DraftAnswered(d) || d.Skipped;

    /// <summary>重建当前题页：题面 + detail + 选项（推荐标记）+ 自定义。</summary>
    private void RenderQuestionPage()
    {
        if (QuestionHost.Tag is not StackPanel pageHost) return;
        pageHost.Children.Clear();
        _questionPage = null;
        if (_questionIndex < 0 || _questionIndex >= _questionItems.Count) return;

        var q = _questionItems[_questionIndex];
        var draft = _questionDrafts[_questionIndex];
        var qid = Str(q, "id");
        var text = Str(q, "question");
        var multi = q.TryGetProperty("multiSelect", out var ms) && ms.ValueKind == JsonValueKind.True;
        var page = new QuestionPageUi { Id = qid, Multi = multi };

        // 题面
        pageHost.Children.Add(new TextBlock
        {
            Text = text,
            TextWrapping = TextWrapping.Wrap,
            Style = AppStyle("BodyTextStyle"),
        });

        // 计划评审的 detail 是完整计划 markdown：原样呈现（官方客户端同样展示它）
        var detail = Str(q, "detail");
        if (detail.Length > 0)
        {
            pageHost.Children.Add(new Border
            {
                Background = ThemeBrush("CardSecondaryBrush"),
                CornerRadius = RadMedium,
                Padding = TokenThickness("CardPaddingCompact", new Thickness(12, 8, 12, 8)),
                MaxHeight = 260,
                Child = new ScrollViewer
                {
                    VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
                    Content = new ContentControl { Content = detail, HorizontalContentAlignment = HorizontalAlignment.Stretch },
                },
            });
        }

        var options = q.TryGetProperty("options", out var opts) && opts.ValueKind == JsonValueKind.Array ? opts : default;
        var group = $"q-{qid}-{Guid.NewGuid():N}";
        if (options.ValueKind == JsonValueKind.Array)
        {
            foreach (var opt in options.EnumerateArray())
            {
                var rawLabel = Str(opt, "label");
                var desc = Str(opt, "description");
                var (displayLabel, recommended) = ParseRecommendedLabel(rawLabel);

                var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6 };
                line.Children.Add(new TextBlock
                {
                    Text = displayLabel,
                    VerticalAlignment = VerticalAlignment.Center,
                    TextWrapping = TextWrapping.Wrap,
                });
                if (recommended)
                {
                    line.Children.Add(new Border
                    {
                        Background = ThemeBrush("AccentBrush"),
                        CornerRadius = new CornerRadius(8),
                        Padding = new Thickness(6, 1, 6, 1),
                        VerticalAlignment = VerticalAlignment.Center,
                        Child = new TextBlock
                        {
                            Text = L("推荐"),
                            FontSize = 11,
                            Foreground = ThemeBrush("TextPrimaryBrush"),
                        },
                    });
                }
                if (desc.Length > 0)
                {
                    line.Children.Add(new TextBlock
                    {
                        Text = desc,
                        Style = AppStyle("CaptionTextStyle"),
                        Foreground = ThemeBrush("TextSecondaryBrush"),
                        VerticalAlignment = VerticalAlignment.Center,
                        TextWrapping = TextWrapping.Wrap,
                    });
                }

                if (multi)
                {
                    var cb = new CheckBox { Content = line, Tag = rawLabel };
                    Aut(cb, $"QuestionOption_{qid}_{rawLabel}", LF("选项：{0}", rawLabel));
                    cb.IsChecked = draft.Selected.Contains(rawLabel);
                    cb.Checked += (_, _) => OnQuestionOptionToggled(page, rawLabel, multi: true, isChecked: true);
                    cb.Unchecked += (_, _) => OnQuestionOptionToggled(page, rawLabel, multi: true, isChecked: false);
                    page.Checks.Add((cb, rawLabel));
                    _questionControls.Add(cb);
                    pageHost.Children.Add(cb);
                }
                else
                {
                    var rb = new RadioButton { Content = line, GroupName = group, Tag = rawLabel };
                    Aut(rb, $"QuestionOption_{qid}_{rawLabel}", LF("选项：{0}", rawLabel));
                    rb.IsChecked = draft.Selected.Contains(rawLabel);
                    rb.Checked += (_, _) => OnQuestionOptionToggled(page, rawLabel, multi: false, isChecked: true);
                    page.Radios.Add((rb, rawLabel));
                    _questionControls.Add(rb);
                    pageHost.Children.Add(rb);
                }
            }
        }

        // 自由文本：内核 answer.custom 字段（官方 custom.placeholder = 输入你的答案）
        var custom = new TextBox
        {
            PlaceholderText = L("输入你的答案"),
            TextWrapping = TextWrapping.Wrap,
            AcceptsReturn = false,
            Text = draft.Custom,
        };
        Aut(custom, $"QuestionCustom_{qid}", L("补充说明"));
        custom.TextChanged += (_, _) =>
        {
            if (_questionPage is not { } p || !ReferenceEquals(p.Custom, custom)) return;
            var d = _questionDrafts[_questionIndex];
            var value = (custom.Text ?? "").Trim();
            d.Custom = value;
            if (value.Length > 0 && !p.Multi)
            {
                // 单选 + 自定义互斥（官方 draftCustom）：写了自由文本就清选项
                foreach (var (radio, _) in p.Radios) radio.IsChecked = false;
                d.Selected.Clear();
            }
            if (value.Length > 0) d.Skipped = false;
            ClearQuestionFeedback();
        };
        page.Custom = custom;
        _questionControls.Add(custom);
        pageHost.Children.Add(custom);

        _questionPage = page;
        SyncQuestionChrome();
        ClearQuestionFeedback();
    }

    private void OnQuestionOptionToggled(QuestionPageUi page, string label, bool multi, bool isChecked)
    {
        if (!ReferenceEquals(_questionPage, page) || _questionIndex < 0 || _questionIndex >= _questionDrafts.Count) return;
        var draft = _questionDrafts[_questionIndex];
        if (multi)
        {
            if (isChecked)
            {
                if (!draft.Selected.Contains(label)) draft.Selected.Add(label);
            }
            else
            {
                draft.Selected.Remove(label);
            }
            draft.Skipped = false;
            ClearQuestionFeedback();
            return;
        }

        if (!isChecked) return;
        // 单选：官方 choose() 写 [label] 并清 custom；非末题自动进下一题
        draft.Selected.Clear();
        draft.Selected.Add(label);
        draft.Custom = "";
        draft.Skipped = false;
        if (page.Custom is { } box && (box.Text ?? "").Length > 0)
        {
            box.Text = "";
        }
        ClearQuestionFeedback();
        if (_questionIndex < _questionItems.Count - 1)
        {
            GoToQuestion(_questionIndex + 1);
        }
        else
        {
            SyncQuestionChrome();
        }
    }

    private void SyncQuestionChrome()
    {
        var count = _questionItems.Count;
        var index = _questionIndex;
        if (_questionProgress is not null)
        {
            _questionProgress.Text = count > 0 ? $"{index + 1} / {count}" : "";
            AutomationProperties.SetName(_questionProgress,
                count > 0 ? LF("第 {0} / {1} 题", index + 1, count) : "");
        }
        if (_questionPrev is not null) _questionPrev.IsEnabled = index > 0;
        if (_questionNext is not null) _questionNext.IsEnabled = index < count - 1;
    }

    private void ClearQuestionFeedback()
    {
        if (_questionFeedback is not null) _questionFeedback.Text = "";
    }

    private void SetQuestionFeedback(string text)
    {
        if (_questionFeedback is not null) _questionFeedback.Text = text;
    }

    private void GoToQuestion(int index)
    {
        if (_questionItems.Count == 0) return;
        CaptureQuestionDraft();
        _questionIndex = Math.Clamp(index, 0, _questionItems.Count - 1);
        RenderQuestionPage();
    }

    private void OnQuestionPrevClick(object sender, RoutedEventArgs e)
    {
        if (_submittingQuestionEventId is not null) return;
        GoToQuestion(_questionIndex - 1);
    }

    private void OnQuestionNextClick(object sender, RoutedEventArgs e)
    {
        if (_submittingQuestionEventId is not null) return;
        // 与官方 continueFlow 同门槛：未答不能靠下一题跳过；跳过请点「跳过本题」
        CaptureQuestionDraft();
        var draft = _questionDrafts[_questionIndex];
        if (!DraftAnswered(draft))
        {
            SetQuestionFeedback(L("请选择一个选项或填写自定义答案。"));
            return;
        }
        if (_questionIndex < _questionItems.Count - 1)
        {
            GoToQuestion(_questionIndex + 1);
        }
        else
        {
            _ = SubmitQuestionSetAsync();
        }
    }

    /// <summary>跳过本题：草稿 skipped，前进；末题跳过则直接汇总提交（官方 skipQuestion）。</summary>
    private void OnQuestionSkipClick(object sender, RoutedEventArgs e)
    {
        if (_submittingQuestionEventId is not null) return;
        var draft = _questionDrafts[_questionIndex];
        draft.Selected.Clear();
        draft.Custom = "";
        draft.Skipped = true;
        if (_questionPage is { } page)
        {
            foreach (var (radio, _) in page.Radios) radio.IsChecked = false;
            foreach (var (check, _) in page.Checks) check.IsChecked = false;
            page.Custom.Text = "";
        }
        ClearQuestionFeedback();
        if (_questionIndex < _questionItems.Count - 1)
        {
            GoToQuestion(_questionIndex + 1);
        }
        else
        {
            _ = SubmitQuestionSetAsync();
        }
    }

    private async void OnQuestionSubmitClick(object sender, RoutedEventArgs e)
    {
        await SubmitQuestionSetAsync();
    }

    /// <summary>放弃整组问题：outcome 对齐官方 pending.cancel() → ASK_CANCELLED。</summary>
    private async void OnQuestionAbandonClick(object sender, RoutedEventArgs e)
    {
        await AbandonQuestionSetAsync(focusChat: false);
    }

    /// <summary>去聊天里说：关闭卡片（ASK_CANCELLED）并聚焦输入框，不提交空答案。</summary>
    private async void OnQuestionToChatClick(object sender, RoutedEventArgs e)
    {
        await AbandonQuestionSetAsync(focusChat: true);
    }

    private void SetQuestionInputEnabled(bool enabled)
    {
        foreach (var control in _questionControls)
        {
            control.IsEnabled = enabled;
        }
    }

    /// <summary>汇总已答（含跳过题 id）后一次 $events/result 回传完整 answers。</summary>
    private async Task SubmitQuestionSetAsync()
    {
        CaptureQuestionDraft();
        var missing = _questionDrafts.FindIndex(d => !DraftCompleted(d));
        if (missing >= 0)
        {
            GoToQuestion(missing);
            SetQuestionFeedback(L("请先完成这道问题。"));
            return;
        }

        var eventId = Str(_activeQuestion, "eventId");
        if (_rpc is null || eventId.Length == 0 || _submittingQuestionEventId is not null)
        {
            return;
        }
        _submittingQuestionEventId = eventId;
        SetQuestionInputEnabled(false);
        try
        {
            var answers = new List<object>();
            for (var i = 0; i < _questionItems.Count; i++)
            {
                var id = Str(_questionItems[i], "id");
                var draft = _questionDrafts[i];
                if (draft.Skipped)
                {
                    answers.Add(new Dictionary<string, object> { ["id"] = id, ["selected"] = Array.Empty<string>() });
                    continue;
                }
                var multi = _questionItems[i].TryGetProperty("multiSelect", out var ms) && ms.ValueKind == JsonValueKind.True;
                var custom = draft.Custom.Trim();
                // 官方 submitDrafts：单选 + 有 custom → selected 置空，只回 custom
                var selected = custom.Length > 0 && !multi
                    ? new List<string>()
                    : draft.Selected.ToList();
                var answer = new Dictionary<string, object> { ["id"] = id, ["selected"] = selected };
                if (custom.Length > 0) answer["custom"] = custom;
                answers.Add(answer);
            }
            await _rpc.ResolveWaterfallAsync(eventId, new { answers });
            RunEventUi(() => FinishActiveQuestion(eventId, L("已回传提问答复。")));
        }
        catch (Exception ex)
        {
            RunEventUi(() =>
            {
                if (Str(_activeQuestion, "eventId") != eventId) return;
                _submittingQuestionEventId = null;
                SetQuestionInputEnabled(true);
                AppendSystemMessage(LF("提问回传失败，请重试：{0}", ex.Message));
            });
        }
    }

    /// <summary>放弃整组：$events/result outcome={kind:"rejected", error:UserQuestionError/ASK_CANCELLED}。</summary>
    private async Task AbandonQuestionSetAsync(bool focusChat)
    {
        var eventId = Str(_activeQuestion, "eventId");
        if (_rpc is null || eventId.Length == 0 || _submittingQuestionEventId is not null)
        {
            return;
        }
        _submittingQuestionEventId = eventId;
        SetQuestionInputEnabled(false);
        try
        {
            await RejectQuestionEventAsync(eventId);
            RunEventUi(() =>
            {
                FinishActiveQuestion(eventId, L("已放弃该组问题。"));
                if (focusChat)
                {
                    InputBox.Focus(FocusState.Programmatic);
                }
            });
        }
        catch (Exception ex)
        {
            RunEventUi(() =>
            {
                if (Str(_activeQuestion, "eventId") != eventId) return;
                _submittingQuestionEventId = null;
                SetQuestionInputEnabled(true);
                AppendSystemMessage(LF("放弃提问失败，请重试：{0}", ex.Message));
            });
        }
    }

    /// <summary>
    /// 回传 rejected 载荷（对齐 dsh-client-ui-user-questions PendingQuestion.cancel）。
    /// DshRpcClient 只暴露 result/next 两种 outcome，rejected 走 CallOkAsync 拼完整信封；
    /// clientId 必须用 $events ready 帧那个（约束：不改 DshRpcClient.cs，此处只读其私有字段）。
    /// </summary>
    private async Task RejectQuestionEventAsync(string eventId)
    {
        if (_rpc is null) return;
        await _rpc.EventsReady.WaitAsync(TimeSpan.FromSeconds(10));
        var field = typeof(DshRpcClient).GetField("_eventsClientId",
            System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance);
        var clientId = field?.GetValue(_rpc) as string;
        if (string.IsNullOrEmpty(clientId))
        {
            throw new InvalidOperationException("事件连接已失效，请等待重新连接后再回答。");
        }
        await _rpc.CallOkAsync("$events/result", new
        {
            clientId,
            eventId,
            outcome = new
            {
                kind = "rejected",
                error = new
                {
                    name = "UserQuestionError",
                    message = "the user cancelled ask_user_question",
                    code = "ASK_CANCELLED",
                },
            },
        });
    }

    private void FinishActiveQuestion(string eventId, string? systemMessage)
    {
        if (Str(_activeQuestion, "eventId") != eventId) return;
        _activeQuestion = default;
        _submittingQuestionEventId = null;
        if (systemMessage is { Length: > 0 })
        {
            AppendSystemMessage(systemMessage);
        }
        ShowNextQuestion();
    }
}
