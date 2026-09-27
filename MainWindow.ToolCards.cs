// ---------------- 工具调用 keyed 定制卡（对标 @deepseek-ai/dsh-client-ui-tool） ----------------
// 按 toolName 分派定制卡体：bash/pwsh 命令+退出码+信号+stdout/stderr；write/edit/
// str_replace_editor 统一差异着色；fs/web/skill/todo/subagent/workflow 补关键字段。
// 任意工具行「…」/右键：复制参数 JSON / 复制结果 JSON / 复制属性路径。
// 状态挂在气泡旁路表（不改 ChatBubble），result 到达时 MainWindow.xaml.cs 的
// RenderEventCore tool/result 分支调 NoteToolResult 落盘。

using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
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
    /// <summary>一张工具卡的旁路状态：result 载荷、展开态、差异展开态。
    /// 挂在 ChatBubble 上（字典），避免改 ChatBubble 类型定义。</summary>
    private sealed class ToolCallCardState
    {
        public string? ResultText;
        public string? ResultJson;
        public string? MetaJson;
        public bool IsError;
        public long ResultTime;
        /// <summary>NoteToolResult 自增；BuildToolCard 用它判断是否要重建（在跑的扫光行不重建）。</summary>
        public int ResultStamp;
        public int PaintedStamp = -1;
        public bool Expanded;
        public bool DiffExpanded;
        public string CallId = "";
    }

    private sealed class ToolDiffHunk
    {
        public string Path = "";
        public string? OldText;
        public string NewText = "";
    }

    private sealed class ToolDiffLine
    {
        public char Kind; // ' ' 上下文 / '-' 删 / '+' 增
        public string Text = "";
    }

    private readonly Dictionary<ChatBubble, ToolCallCardState> _toolCardStates = new();

    /// <summary>tool/result 到达：把结果正文 / 原始 JSON / meta.diffs / isError 落到旁路状态。
    /// 由 RenderEventCore 的 tool/result 分支调用（document 仍存活，立刻 GetRawText 拷走）。</summary>
    private void NoteToolResult(ChatBubble bubble, JsonElement data, long resultTime)
    {
        if (!_toolCardStates.TryGetValue(bubble, out var st))
        {
            _toolCardStates[bubble] = st = new ToolCallCardState();
        }
        st.ResultText = ToolResultText(data);
        try
        {
            st.ResultJson = data.GetRawText();
        }
        catch (Exception)
        {
            st.ResultJson = null;
        }
        st.MetaJson = data.TryGetProperty("meta", out var meta) && meta.ValueKind == JsonValueKind.Object
            ? meta.GetRawText()
            : null;
        st.IsError = false;
        if (data.TryGetProperty("message", out var msg) && msg.ValueKind == JsonValueKind.Object &&
            msg.TryGetProperty("content", out var blocks) && blocks.ValueKind == JsonValueKind.Array)
        {
            foreach (var b in blocks.EnumerateArray())
            {
                if (b.TryGetProperty("isError", out var ie) && ie.ValueKind == JsonValueKind.True)
                {
                    st.IsError = true;
                }
                break;
            }
        }
        st.ResultTime = resultTime;
        st.ResultStamp++;
    }

    private ToolCallCardState ToolState(ChatBubble bubble)
    {
        if (!_toolCardStates.TryGetValue(bubble, out var st))
        {
            _toolCardStates[bubble] = st = new ToolCallCardState();
        }
        return st;
    }

    // ---------------- 装配入口（MessageActions.BuildToolCallRow 转入） ----------------

    /// <summary>工具定制卡装配。在跑的同气泡只起停扫光（保住 Storyboard）；
    /// result 到/展开态翻转则重建整卡。</summary>
    private void BuildToolCard(ContentControl host, ChatBubble bubble)
    {
        // P1 聊天域过程卡（系统提示词/上下文注入/召回/压缩/重试/截断/workflow-run）
        if (bubble.ToolName is { } dn && MessageDomainTools.Contains(dn))
        {
            BuildMessageDomainCard(host, bubble);
            return;
        }
        var state = ToolState(bubble);
        var toolName = bubble.ToolName ?? "";
        var sameResult = state.PaintedStamp == state.ResultStamp;
        if (host.Content is Grid reuseRoot &&
            reuseRoot.Tag is (ChatBubble sameBubble, Rectangle reuseSweep, Grid reuseClip, bool paintedExpanded, bool paintedRunning) &&
            ReferenceEquals(sameBubble, bubble) &&
            sameResult &&
            paintedExpanded == state.Expanded &&
            paintedRunning == bubble.IsToolRunning)
        {
            SyncToolSweep(bubble, reuseSweep, reuseClip);
            return;
        }
        if (host.Content is Grid oldRoot && oldRoot.Tag is (_, Rectangle oldSweep, _, _, _))
        {
            StopSweep(oldSweep);
        }

        var (glyph, title, summary) = ToolRowModel(toolName, bubble.ToolArgs ?? "");
        var running = bubble.IsToolRunning;
        var failed = !running && (state.IsError || (!string.IsNullOrEmpty(state.ResultText) && FirstLine(state.ResultText).StartsWith("Error", StringComparison.OrdinalIgnoreCase)));
        var diffHunks = CollectToolDiffs(toolName, bubble.ToolArgs, state.MetaJson);
        var diffStat = diffHunks.Count > 0 ? DiffTotalsText(diffHunks) : "";

        // ---- 头行：chevron + 图标 + 标题 · 摘要 + 状态/用时/差异统计 + 「…」 ----
        var chevron = new FontIcon { Glyph = state.Expanded ? "\uE76C" : "\uE76B", FontSize = 10 };
        var icon = new FontIcon
        {
            Glyph = glyph,
            FontSize = 12,
            Opacity = 0.85,
            Foreground = failed ? ThemeBrush("ErrorBrush") : null,
        };
        var titleBlock = new TextBlock
        {
            Text = title,
            Style = AppTextStyle("CaptionTextStyle"),
            FontWeight = Microsoft.UI.Text.FontWeights.Medium,
            VerticalAlignment = VerticalAlignment.Center,
            Foreground = failed ? ThemeBrush("ErrorBrush") : ThemeBrush("TextPrimaryBrush"),
        };
        var summaryBlock = new TextBlock
        {
            Text = summary,
            Style = AppTextStyle("HintTextStyle"),
            Opacity = 0.9,
            TextTrimming = TextTrimming.CharacterEllipsis,
            TextWrapping = TextWrapping.NoWrap,
            VerticalAlignment = VerticalAlignment.Center,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Margin = new Thickness(0, 0, Sp4, 0),
        };

        var metaChips = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = Sp4,
            VerticalAlignment = VerticalAlignment.Center,
        };
        foreach (var chip in ToolKindChips(toolName, bubble.ToolArgs, state, running))
        {
            metaChips.Children.Add(chip);
        }
        if (diffStat.Length > 0)
        {
            metaChips.Children.Add(MakeChip(diffStat, ThemeBrush("TextTertiaryBrush"), mono: true, id: "ToolCardDiffStat"));
        }
        if (!running && state.ResultTime > 0 && bubble.Time > 0 && state.ResultTime >= bubble.Time)
        {
            var ms = state.ResultTime - bubble.Time;
            metaChips.Children.Add(MakeChip(FormatTurnDuration(ms), ThemeBrush("TextTertiaryBrush"), mono: false, id: "ToolCardDuration"));
        }
        if (running)
        {
            metaChips.Children.Add(MakeChip(L("执行中"), ThemeBrush("InfoBrush"), mono: false, id: "ToolCardStatus"));
        }
        else if (failed)
        {
            metaChips.Children.Add(MakeChip(L("失败"), ThemeBrush("ErrorBrush"), mono: false, id: "ToolCardStatus"));
        }

        var copyButton = new Button
        {
            Width = SmallButtonSize,
            Height = SmallButtonSize,
            MinWidth = 0,
            Padding = new Thickness(0),
            CornerRadius = RadSmall,
            Background = new SolidColorBrush(Colors.Transparent),
            BorderThickness = new Thickness(0),
            VerticalAlignment = VerticalAlignment.Center,
            Content = new FontIcon { Glyph = "\uE712", FontSize = 12 },
        };
        Aut(copyButton, "ToolCopyJson", L("复制参数 JSON"));
        ToolTipService.SetToolTip(copyButton, L("复制参数 JSON"));
        var copyMenu = BuildToolCopyMenu(bubble, state);
        copyButton.Flyout = copyMenu;

        var headerInner = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = Sp6,
            VerticalAlignment = VerticalAlignment.Center,
        };
        headerInner.Children.Add(chevron);
        headerInner.Children.Add(icon);
        headerInner.Children.Add(titleBlock);
        headerInner.Children.Add(new TextBlock
        {
            Text = "·",
            Style = AppTextStyle("CaptionTextStyle"),
            Opacity = 0.45,
            VerticalAlignment = VerticalAlignment.Center,
        });
        headerInner.Children.Add(summaryBlock);
        headerInner.Children.Add(metaChips);
        headerInner.Children.Add(copyButton);

        var bodyHost = new StackPanel
        {
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Margin = new Thickness(22, Sp4, 0, Sp2),
            Spacing = Sp6,
            Visibility = state.Expanded ? Visibility.Visible : Visibility.Collapsed,
        };
        BuildToolCardBody(host, bodyHost, toolName, bubble, state, diffHunks);

        var headerButton = new ToggleButton
        {
            Content = headerInner,
            Background = new SolidColorBrush(Colors.Transparent),
            BorderThickness = new Thickness(0),
            Padding = new Thickness(Sp4, Sp2, Sp2, Sp2),
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Left,
            CornerRadius = RadSmall,
            IsChecked = state.Expanded,
        };
        Aut(headerButton, "ToolCardExpand", $"{title} {summary}");
        headerButton.Checked += (_, _) =>
        {
            state.Expanded = true;
            bodyHost.Visibility = Visibility.Visible;
            chevron.Glyph = "\uE76C";
        };
        headerButton.Unchecked += (_, _) =>
        {
            state.Expanded = false;
            bodyHost.Visibility = Visibility.Collapsed;
            chevron.Glyph = "\uE76B";
        };

        // 右键 = 同一套复制菜单（官方 ToolRow 无 context menu，这里是壳侧补齐）
        var cardRoot = new StackPanel
        {
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Margin = new Thickness(4, 1, 4, 1),
            Spacing = 0,
        };
        AutomationProperties.SetAutomationId(cardRoot, "ToolCard");
        AutomationProperties.SetName(cardRoot, $"{title} {summary}");
        cardRoot.Children.Add(headerButton);
        cardRoot.Children.Add(bodyHost);
        cardRoot.ContextFlyout = copyMenu;
        // 头行右键也走同一菜单（点在 header 上时 ContextFlyout 冒泡到 cardRoot）
        headerButton.ContextFlyout = copyMenu;

        var clip = new Grid { HorizontalAlignment = HorizontalAlignment.Stretch };
        clip.Children.Add(cardRoot);
        var sweep = new Rectangle
        {
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Stretch,
            Width = 140,
            Opacity = 0,
            IsHitTestVisible = false,
        };
        clip.Children.Add(sweep);
        clip.Tag = (bubble, sweep, clip, state.Expanded, bubble.IsToolRunning);
        host.Content = clip;
        state.PaintedStamp = state.ResultStamp;
        SyncToolSweep(bubble, sweep, clip);
    }

    // ---------------- 卡体（按 toolName 分派） ----------------

    private void BuildToolCardBody(
        ContentControl host,
        StackPanel body,
        string toolName,
        ChatBubble bubble,
        ToolCallCardState state,
        List<ToolDiffHunk> diffHunks)
    {
        var variant = ClassifyToolVariant(toolName);
        if (variant is "bash")
        {
            AppendBashBody(body, bubble, state);
        }
        else if (diffHunks.Count > 0)
        {
            AppendDiffBody(host, body, bubble, diffHunks, state);
            AppendIoBlock(body, L("参数"), FormatToolJson(bubble.ToolArgs), isError: false);
            if (!bubble.IsToolRunning && state.ResultText is { Length: > 0 } rt)
            {
                AppendIoBlock(body, L("结果"), rt, isError: state.IsError);
            }
        }
        else
        {
            AppendKindHint(body, toolName, bubble);
            // P1-18：workflow 工具卡可展开成员（「{count} 个成员」）
            if (toolName == "workflow")
            {
                AppendWorkflowMembers(body, bubble);
            }
            AppendIoBlock(body, L("参数"), FormatToolJson(bubble.ToolArgs), isError: false);
            if (!bubble.IsToolRunning)
            {
                AppendIoBlock(body, L("结果"), state.ResultText ?? "", isError: state.IsError);
            }
        }
    }

    /// <summary>bash / pwsh：命令全文 + 退出码/信号 + stdout/stderr 折叠区（官方 TerminalBlock 口径）。</summary>
    private void AppendBashBody(StackPanel body, ChatBubble bubble, ToolCallCardState state)
    {
        var command = ToolArgString(bubble.ToolArgs, "command") ?? FirstLine(bubble.ToolArgs ?? "");
        if (command.Length > 0)
        {
            body.Children.Add(MakeSectionLabel(L("命令")));
            body.Children.Add(MakeCodeBlock(command, wrap: true, maxHeight: 96));
        }

        var raw = state.ResultText ?? "";
        var status = ParseExitStatus(raw);
        if (!bubble.IsToolRunning)
        {
            var statusRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6 };
            if (status.Signal is { Length: > 0 } sig)
            {
                statusRow.Children.Add(MakeChip(LF("信号 {0}", sig), ThemeBrush("ErrorBrush"), mono: false, id: "ToolCardSignal"));
            }
            else
            {
                var code = status.ExitCode ?? 0;
                var codeBrush = code == 0 ? ThemeBrush("SuccessBrush") : ThemeBrush("ErrorBrush");
                statusRow.Children.Add(MakeChip(LF("退出码 {0}", code), codeBrush, mono: false, id: "ToolCardExitCode"));
            }
            if (status.TimedOut)
            {
                statusRow.Children.Add(MakeChip(L("已超时"), ThemeBrush("WarningBrush"), mono: false, id: "ToolCardTimeout"));
            }
            body.Children.Add(statusRow);

            var (stdout, stderr) = SplitShellStreams(status.Output);
            if (stdout.Length > 0)
            {
                body.Children.Add(MakeFoldableSection(L("标准输出"), stdout, isError: false, collapsedLines: 6));
            }
            else
            {
                body.Children.Add(MakeSectionLabel(L("标准输出")));
                body.Children.Add(MakeChip(L("无输出"), ThemeBrush("TextTertiaryBrush"), mono: false, id: "ToolCardNoOutput"));
            }
            if (stderr.Length > 0)
            {
                body.Children.Add(MakeFoldableSection(L("标准错误"), stderr, isError: true, collapsedLines: 6));
            }
            if (stdout.Length == 0 && stderr.Length == 0 && raw.Length > 0)
            {
                // 非标准包裹（persistent / spill）：整段原样可折叠
                body.Children.Add(MakeFoldableSection(L("结果"), raw, isError: state.IsError, collapsedLines: 6));
            }
        }
    }

    /// <summary>fs / web / skill / todo / subagent / workflow：比通用行多 1-2 个关键字段。</summary>
    private void AppendKindHint(StackPanel body, string toolName, ChatBubble bubble)
    {
        var chips = ToolKindChips(toolName, bubble.ToolArgs, ToolState(bubble), bubble.IsToolRunning);
        if (chips.Count == 0)
        {
            return;
        }
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp4 };
        foreach (var c in chips)
        {
            row.Children.Add(c);
        }
        body.Children.Add(row);
    }

    /// <summary>按工具类型挑关键字段（路径 / URL / 技能名 / 待办计数 / 子代理摘要 / 工作流名）。</summary>
    private List<UIElement> ToolKindChips(string toolName, string? argsRaw, ToolCallCardState state, bool running)
    {
        var chips = new List<UIElement>();
        var args = ParseArgsObject(argsRaw);
        void AddChip(string text, Brush? brush = null, bool mono = false, string id = "ToolCardField")
        {
            if (text.Length == 0) return;
            chips.Add(MakeChip(text, brush ?? ThemeBrush("TextSecondaryBrush"), mono, id));
        }

        switch (ClassifyToolVariant(toolName))
        {
            case "read" or "write" or "edit":
                var path = PickArgString(args, "file_path") ?? PickArgString(args, "path") ?? PickArgString(args, "url");
                AddChip(path ?? "", id: "ToolCardPath");
                break;
            case "search":
                var pattern = PickArgString(args, "pattern") ?? PickArgString(args, "query") ?? PickArgString(args, "glob");
                if (pattern is { Length: > 0 })
                {
                    AddChip(pattern, mono: true, id: "ToolCardPattern");
                }
                var searchPath = PickArgString(args, "path") ?? PickArgString(args, "file_path");
                if (searchPath is { Length: > 0 })
                {
                    AddChip(searchPath, id: "ToolCardPath");
                }
                break;
            case "bash":
                var workdir = PickArgString(args, "workdir");
                if (workdir is { Length: > 0 })
                {
                    AddChip(workdir, id: "ToolCardWorkdir");
                }
                break;
        }

        // —— 专用工具的关键字段 ——
        if (toolName is "web_fetch" or "web_search")
        {
            var url = PickArgString(args, "url");
            if (url is { Length: > 0 })
            {
                AddChip(url, id: "ToolCardUrl");
            }
            else if (args is not null && args.Value.TryGetProperty("queries", out var qs) && qs.ValueKind == JsonValueKind.Array)
            {
                var parts = qs.EnumerateArray().Where(x => x.ValueKind == JsonValueKind.String).Select(x => x.GetString() ?? "").Where(x => x.Length > 0).Take(3).ToList();
                if (parts.Count > 0)
                {
                    AddChip(string.Join(" · ", parts), id: "ToolCardUrl");
                }
            }
        }
        else if (toolName == "skill")
        {
            var skillName = PickArgString(args, "name") ?? PickArgString(args, "skill");
            if (skillName is { Length: > 0 })
            {
                // P1-10：skill 引用 chip可点开技能说明（复用 Skills.cs 的 MakeSkillReferenceChip）
                chips.Add(MakeSkillReferenceChip(skillName));
            }
        }
        else if (toolName is "todo_write" or "todo")
        {
            if (args is not null && args.Value.TryGetProperty("todos", out var todos) && todos.ValueKind == JsonValueKind.Array)
            {
                var total = 0;
                var done = 0;
                var active = 0;
                foreach (var t in todos.EnumerateArray())
                {
                    total++;
                    var st = t.TryGetProperty("status", out var sv) && sv.ValueKind == JsonValueKind.String ? sv.GetString() : "";
                    if (st == "completed") done++;
                    else if (st == "in_progress") active++;
                }
                AddChip(LF("待办 {0}/{1}", done, total), id: "ToolCardTodo");
                if (active > 0)
                {
                    AddChip(LF("进行中 {0}", active), ThemeBrush("InfoBrush"), id: "ToolCardTodoActive");
                }
            }
        }
        else if (toolName is "subagent" or "list_subagent_models")
        {
            var desc = PickArgString(args, "description") ?? PickArgString(args, "prompt");
            if (desc is { Length: > 0 })
            {
                AddChip(LF("子代理 {0}", FirstLine(desc)), id: "ToolCardSubagent");
            }
        }
        else if (toolName == "workflow")
        {
            string? wfName = null;
            string? wfDesc = null;
            if (args is not null && args.Value.TryGetProperty("meta", out var meta) && meta.ValueKind == JsonValueKind.Object)
            {
                wfName = PickArgString(meta, "name");
                wfDesc = PickArgString(meta, "description");
            }
            if (wfName is { Length: > 0 })
            {
                AddChip(LF("工作流 {0}", wfName), id: "ToolCardWorkflow");
            }
            else if (wfDesc is { Length: > 0 })
            {
                AddChip(FirstLine(wfDesc), id: "ToolCardWorkflow");
            }
            // P1-18：workflow-run 成员计数（tool-workflow/* 折叠结果）——在 BuildToolCardBody 内展示
        }

        // 结果摘要（已有 result 且无专用卡体时补一行）
        if (!running && state.ResultText is { Length: > 0 } result &&
            ClassifyToolVariant(toolName) is not ("bash" or "write" or "edit") &&
            toolName is not "str_replace_editor")
        {
            var head = FirstLine(result);
            if (head.Length > 0 && chips.Count < 4)
            {
                AddChip(head.Length > 48 ? head[..48] + "…" : head, ThemeBrush("TextTertiaryBrush"), id: "ToolCardResultSummary");
            }
        }
        return chips;
    }

    // ---------------- 差异视图（unified 风格行着色） ----------------

    private void AppendDiffBody(ContentControl host, StackPanel body, ChatBubble bubble, List<ToolDiffHunk> hunks, ToolCallCardState state)
    {
        body.Children.Add(MakeSectionLabel(L("差异")));
        var lines = new List<(string Path, ToolDiffLine Line)>();
        foreach (var hunk in hunks)
        {
            foreach (var line in DiffLinesOf(hunk))
            {
                lines.Add((hunk.Path, line));
            }
        }
        if (lines.Count == 0)
        {
            return;
        }

        const int collapsedLimit = 8;
        var showAll = state.DiffExpanded || lines.Count <= collapsedLimit;
        var shown = showAll ? lines : lines.Take(collapsedLimit).ToList();
        var hidden = lines.Count - shown.Count;

        var panel = new StackPanel { Spacing = 0 };
        string? lastPath = null;
        foreach (var (path, line) in shown)
        {
            if (path != lastPath)
            {
                panel.Children.Add(new TextBlock
                {
                    Text = path,
                    Style = AppTextStyle("HintTextStyle"),
                    Opacity = 0.75,
                    Margin = new Thickness(0, Sp2, 0, 0),
                    TextTrimming = TextTrimming.CharacterEllipsis,
                });
                lastPath = path;
            }
            panel.Children.Add(MakeDiffLine(line));
        }
        body.Children.Add(MakeCodeHost(panel));

        if (hidden > 0 || state.DiffExpanded)
        {
            var toggleLabel = state.DiffExpanded ? L("收起差异") : LF("展开其余 {0} 行", hidden);
            var toggle = new Button
            {
                Content = new TextBlock
                {
                    Text = toggleLabel,
                    Style = AppTextStyle("CaptionTextStyle"),
                },
                Margin = new Thickness(0, Sp2, 0, 0),
                Padding = new Thickness(Sp6, Sp2, Sp6, Sp2),
                CornerRadius = RadSmall,
                HorizontalAlignment = HorizontalAlignment.Left,
                MinHeight = 0,
            };
            Aut(toggle, "ToolCardDiffExpand", toggleLabel);
            toggle.Click += (_, _) =>
            {
                state.DiffExpanded = !state.DiffExpanded;
                state.PaintedStamp = -1;
                BuildToolCard(host, bubble);
            };
            body.Children.Add(toggle);
        }
    }

    /// <summary>一个 hunk 的 unified 风格行序列：old 全删 / new 全增时直出；否则 LCS 行对齐。</summary>
    private static List<ToolDiffLine> DiffLinesOf(ToolDiffHunk hunk)
    {
        var oldLines = hunk.OldText is null
            ? Array.Empty<string>()
            : SplitKeepEmpty(hunk.OldText);
        var newLines = SplitKeepEmpty(hunk.NewText);
        if (oldLines.Length == 0)
        {
            return newLines.Select(t => new ToolDiffLine { Kind = '+', Text = t }).ToList();
        }
        if (newLines.Length == 0)
        {
            return oldLines.Select(t => new ToolDiffLine { Kind = '-', Text = t }).ToList();
        }
        return AlignDiffLines(oldLines, newLines);
    }

    private static string[] SplitKeepEmpty(string text)
    {
        if (text.Length == 0)
        {
            return Array.Empty<string>();
        }
        var parts = text.Replace("\r\n", "\n").Split('\n');
        // 尾部空行是文本换行产物，不进差异
        if (parts.Length > 0 && parts[^1].Length == 0)
        {
            Array.Resize(ref parts, parts.Length - 1);
        }
        return parts;
    }

    /// <summary>轻量 LCS 行对齐（上限 320×320，超出退化为「整段删 + 整段增」）。</summary>
    private static List<ToolDiffLine> AlignDiffLines(string[] a, string[] b)
    {
        const int cap = 320;
        if (a.Length > cap || b.Length > cap)
        {
            var fallback = new List<ToolDiffLine>();
            fallback.AddRange(a.Select(t => new ToolDiffLine { Kind = '-', Text = t }));
            fallback.AddRange(b.Select(t => new ToolDiffLine { Kind = '+', Text = t }));
            return fallback;
        }
        var dp = new int[a.Length + 1, b.Length + 1];
        for (var i = a.Length - 1; i >= 0; i--)
        {
            for (var j = b.Length - 1; j >= 0; j--)
            {
                dp[i, j] = a[i] == b[j]
                    ? dp[i + 1, j + 1] + 1
                    : Math.Max(dp[i + 1, j], dp[i, j + 1]);
            }
        }
        var lines = new List<ToolDiffLine>();
        var x = 0;
        var y = 0;
        while (x < a.Length && y < b.Length)
        {
            if (a[x] == b[y])
            {
                lines.Add(new ToolDiffLine { Kind = ' ', Text = a[x] });
                x++;
                y++;
            }
            else if (dp[x + 1, y] >= dp[x, y + 1])
            {
                lines.Add(new ToolDiffLine { Kind = '-', Text = a[x] });
                x++;
            }
            else
            {
                lines.Add(new ToolDiffLine { Kind = '+', Text = b[y] });
                y++;
            }
        }
        while (x < a.Length)
        {
            lines.Add(new ToolDiffLine { Kind = '-', Text = a[x] });
            x++;
        }
        while (y < b.Length)
        {
            lines.Add(new ToolDiffLine { Kind = '+', Text = b[y] });
            y++;
        }
        return lines;
    }

    private UIElement MakeDiffLine(ToolDiffLine line)
    {
        var prefix = line.Kind switch
        {
            '-' => "- ",
            '+' => "+ ",
            _ => "  ",
        };
        var brush = line.Kind switch
        {
            '-' => ThemeBrush("ErrorBrush"),
            '+' => ThemeBrush("SuccessBrush"),
            _ => ThemeBrush("TextSecondaryBrush"),
        };
        var bg = line.Kind switch
        {
            '-' => SoftBrush("ErrorBrush", 0.12),
            '+' => SoftBrush("SuccessBrush", 0.12),
            _ => null,
        };
        var tb = new TextBlock
        {
            Text = prefix + line.Text,
            FontFamily = CodeFont(),
            FontSize = 12,
            Foreground = brush,
            TextWrapping = TextWrapping.NoWrap,
            Padding = new Thickness(Sp4, 0, Sp4, 0),
        };
        return bg is null
            ? tb
            : new Border { Background = bg, Child = tb, HorizontalAlignment = HorizontalAlignment.Stretch };
    }

    private string DiffTotalsText(List<ToolDiffHunk> hunks)
    {
        var added = 0;
        var removed = 0;
        foreach (var hunk in hunks)
        {
            foreach (var line in DiffLinesOf(hunk))
            {
                if (line.Kind == '+') added++;
                else if (line.Kind == '-') removed++;
            }
        }
        return $"+{added} -{removed}";
    }

    /// <summary>meta.diffs 优先（内核 presentationMeta）；否则从 write/edit/str_replace_editor 参数反推 intended diff。</summary>
    private static List<ToolDiffHunk> CollectToolDiffs(string toolName, string? argsRaw, string? metaJson)
    {
        var list = new List<ToolDiffHunk>();
        if (!string.IsNullOrEmpty(metaJson))
        {
            try
            {
                using var doc = JsonDocument.Parse(metaJson);
                if (doc.RootElement.ValueKind == JsonValueKind.Object &&
                    doc.RootElement.TryGetProperty("diffs", out var diffs) &&
                    diffs.ValueKind == JsonValueKind.Array)
                {
                    foreach (var d in diffs.EnumerateArray())
                    {
                        if (d.ValueKind != JsonValueKind.Object ||
                            !d.TryGetProperty("newText", out var nt) || nt.ValueKind != JsonValueKind.String)
                        {
                            continue;
                        }
                        var path = d.TryGetProperty("path", out var p) && p.ValueKind == JsonValueKind.String
                            ? p.GetString() ?? ""
                            : "";
                        var oldText = d.TryGetProperty("oldText", out var ot) && ot.ValueKind == JsonValueKind.String
                            ? ot.GetString()
                            : null;
                        list.Add(new ToolDiffHunk
                        {
                            Path = path,
                            OldText = oldText,
                            NewText = nt.GetString() ?? "",
                        });
                    }
                    if (list.Count > 0)
                    {
                        return list;
                    }
                }
            }
            catch (JsonException)
            {
            }
        }
        return IntendedDiffsFromArgs(toolName, argsRaw);
    }

    private static List<ToolDiffHunk> IntendedDiffsFromArgs(string toolName, string? argsRaw)
    {
        var list = new List<ToolDiffHunk>();
        var args = ParseArgsObject(argsRaw);
        if (args is null)
        {
            return list;
        }
        var a = args.Value;
        if (toolName == "str_replace_editor")
        {
            var cmd = PickArgString(a, "command") ?? "";
            var path = PickArgString(a, "path") ?? "";
            if (path.Length == 0)
            {
                return list;
            }
            if (cmd == "create")
            {
                list.Add(new ToolDiffHunk
                {
                    Path = path,
                    OldText = null,
                    NewText = PickArgString(a, "file_text") ?? "",
                });
            }
            else if (cmd == "str_replace")
            {
                list.Add(new ToolDiffHunk
                {
                    Path = path,
                    OldText = PickArgString(a, "old_str"),
                    NewText = PickArgString(a, "new_str") ?? "",
                });
            }
            return list;
        }
        if (toolName == "write")
        {
            var path = PickArgString(a, "file_path") ?? PickArgString(a, "path");
            var content = PickArgString(a, "content");
            if (path is { Length: > 0 } && content is not null)
            {
                list.Add(new ToolDiffHunk { Path = path, OldText = null, NewText = content });
            }
            return list;
        }
        if (toolName == "edit")
        {
            var path = PickArgString(a, "file_path") ?? PickArgString(a, "path");
            var oldText = PickArgString(a, "old_string");
            var newText = PickArgString(a, "new_string");
            if (path is { Length: > 0 } && oldText is not null && newText is not null)
            {
                list.Add(new ToolDiffHunk
                {
                    Path = path,
                    OldText = oldText.Length == 0 ? null : oldText,
                    NewText = newText,
                });
            }
        }
        return list;
    }

    // ---------------- 复制菜单（参数/结果 JSON · 格式化/紧凑/属性路径） ----------------

    /// <summary>P1-18：workflow 工具卡体的可展开成员列表（「{count} 个成员」）。</summary>
    private void AppendWorkflowMembers(StackPanel body, ChatBubble bubble)
    {
        var run = WorkflowRunForTool(bubble);
        if (run is null || run.Members.Count == 0)
        {
            body.Children.Add(MakeChip(DetText("没有启动成员", "No members started"), ThemeBrush("TextTertiaryBrush"), mono: false, id: "ToolCardWorkflowEmpty"));
            return;
        }
        var count = run.Members.Count;
        var list = new StackPanel { Spacing = Sp2, Visibility = Visibility.Collapsed };
        foreach (var m in run.Members)
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6 };
            var brush = m.Outcome switch
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
                brush, mono: false, id: "WorkflowMemberStatus"));
            var label = m.Label.Length > 0 ? m.Label : DetText("空成员名", "Empty member name");
            if (m.Phase is { Length: > 0 } ph) label = ph + " · " + label;
            row.Children.Add(new TextBlock { Text = label, Style = AppTextStyle("HintTextStyle"), TextTrimming = TextTrimming.CharacterEllipsis });
            list.Children.Add(row);
        }
        body.Children.Add(list);
        var expanded = false;
        var toggle = new Button
        {
            Content = new TextBlock { Text = DetFormat("{0} 个成员", "{0} members", count), Style = AppTextStyle("CaptionTextStyle") },
            Style = AppStyle("CompactButtonStyle"),
            HorizontalAlignment = HorizontalAlignment.Left,
        };
        Aut(toggle, "ToolCardWorkflowMembersToggle", DetFormat("{0} 个成员", "{0} members", count));
        toggle.Click += (_, _) =>
        {
            expanded = !expanded;
            list.Visibility = expanded ? Visibility.Visible : Visibility.Collapsed;
            ((TextBlock)toggle.Content!).Text = expanded
                ? DetText("收起成员", "Hide members")
                : DetFormat("{0} 个成员", "{0} members", count);
        };
        body.Children.Add(toggle);
    }

    private MenuFlyout BuildToolCopyMenu(ChatBubble bubble, ToolCallCardState state)
    {
        var menu = new MenuFlyout();

        void AddCopy(string zh, string en, string id, Func<string> payload, bool enabled = true)
        {
            var label = DetText(zh, en);
            var item = new MenuFlyoutItem { Text = label, IsEnabled = enabled };
            Aut(item, id, label);
            item.Click += (_, _) => CopyTextToClipboard(payload(), item, label);
            menu.Items.Add(item);
        }

        AddCopy("复制参数 JSON", "Copy JSON", "ToolCopyArgsJson",
            () => FormatToolJson(bubble.ToolArgs));
        AddCopy("复制格式化 JSON", "Copy pretty JSON", "ToolCopyArgsPrettyJson",
            () => PrettyJson(bubble.ToolArgs ?? ""));
        AddCopy("复制紧凑 JSON", "Copy compact JSON", "ToolCopyArgsCompactJson",
            () => CompactJson(bubble.ToolArgs ?? ""));
        AddCopy("复制结果 JSON", "Copy result JSON", "ToolCopyResultJson",
            () => PrettyJson(state.ResultJson ?? ""),
            enabled: !bubble.IsToolRunning && !string.IsNullOrEmpty(state.ResultJson));
        AddCopy("复制属性路径", "Copy property path", "ToolCopyPropertyPath",
            () => ToolPropertyPath(bubble.ToolName ?? "", bubble.ToolArgs));

        return menu;
    }

    private void CopyTextToClipboard(string text, MenuFlyoutItem item, string restoreLabel)
    {
        try
        {
            var package = new Windows.ApplicationModel.DataTransfer.DataPackage
            {
                RequestedOperation = Windows.ApplicationModel.DataTransfer.DataPackageOperation.Copy,
            };
            package.SetText(text ?? "");
            Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(package);
            item.Text = L("已复制");
            var timer = DispatcherQueue.CreateTimer();
            timer.Interval = TimeSpan.FromSeconds(1);
            timer.IsRepeating = false;
            timer.Tick += (_, _) => item.Text = restoreLabel;
            timer.Start();
        }
        catch (Exception)
        {
        }
    }

    /// <summary>主展示字段的 JSON 属性路径（供调试/定位载荷字段）。</summary>
    private static string ToolPropertyPath(string toolName, string? argsRaw)
    {
        var variant = ClassifyToolVariant(toolName);
        return variant switch
        {
            "bash" => "arguments.command",
            "read" or "write" or "edit" => "arguments.file_path",
            "search" => "arguments.pattern",
            _ => toolName switch
            {
                "web_fetch" => "arguments.url",
                "web_search" => "arguments.queries",
                "skill" => "arguments.name",
                "todo_write" or "todo" => "arguments.todos",
                "subagent" => "arguments.prompt",
                "workflow" => "arguments.meta.name",
                _ => "arguments",
            },
        };
    }

    // ---------------- 小构件 ----------------

    private TextBlock MakeSectionLabel(string text)
    {
        return new TextBlock
        {
            Text = text,
            Style = AppTextStyle("CaptionTextStyle"),
            Opacity = 0.7,
            FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
        };
    }

    private Border MakeChip(string text, Brush brush, bool mono, string id)
    {
        var tb = new TextBlock
        {
            Text = text,
            Style = AppTextStyle("CaptionTextStyle"),
            Foreground = brush,
            TextTrimming = TextTrimming.CharacterEllipsis,
            MaxWidth = 280,
        };
        if (mono)
        {
            tb.FontFamily = CodeFont();
            tb.FontSize = 11;
        }
        AutomationProperties.SetAutomationId(tb, id);
        return new Border
        {
            Child = tb,
            Background = SoftBrushFrom(brush, 0.1),
            CornerRadius = new CornerRadius(4),
            Padding = new Thickness(Sp4, 1, Sp4, 1),
        };
    }

    /// <summary>通用输入/输出块：标签 + 等宽正文（错误态用 Error 色）。</summary>
    private void AppendIoBlock(StackPanel body, string label, string text, bool isError)
    {
        if (string.IsNullOrEmpty(text))
        {
            return;
        }
        body.Children.Add(MakeSectionLabel(label));
        body.Children.Add(MakeCodeBlock(text, wrap: true, maxHeight: 160, isError: isError));
    }

    private UIElement MakeFoldableSection(string label, string text, bool isError, int collapsedLines)
    {
        var lines = text.Replace("\r\n", "\n").Split('\n');
        var panel = new StackPanel { Spacing = Sp4 };
        panel.Children.Add(MakeSectionLabel(label));
        if (lines.Length <= collapsedLines)
        {
            panel.Children.Add(MakeCodeBlock(text, wrap: true, maxHeight: 160, isError: isError));
            return panel;
        }
        var collapsed = string.Join("\n", lines.Take(collapsedLines));
        var bodyBlock = MakeCodeBlock(collapsed, wrap: true, maxHeight: 160, isError: isError);
        panel.Children.Add(bodyBlock);
        var hidden = lines.Length - collapsedLines;
        var toggle = new Button
        {
            Content = new TextBlock
            {
                Text = LF("展开其余 {0} 行", hidden),
                Style = AppTextStyle("CaptionTextStyle"),
            },
            Margin = new Thickness(0, 0, 0, 0),
            Padding = new Thickness(Sp6, Sp2, Sp6, Sp2),
            CornerRadius = RadSmall,
            HorizontalAlignment = HorizontalAlignment.Left,
            MinHeight = 0,
        };
        Aut(toggle, "ToolCardExpand", LF("展开其余 {0} 行", hidden));
        var expanded = false;
        toggle.Click += (_, _) =>
        {
            expanded = !expanded;
            bodyBlock.Child = expanded
                ? MakeCodeText(text, isError)
                : MakeCodeText(collapsed, isError);
            ((TextBlock)toggle.Content!).Text = expanded ? L("收起详情") : LF("展开其余 {0} 行", hidden);
        };
        panel.Children.Add(toggle);
        return panel;
    }

    private Border MakeCodeHost(UIElement child)
    {
        return new Border
        {
            Background = ThemeBrush("CardSecondaryBrush"),
            CornerRadius = new CornerRadius(6),
            Padding = new Thickness(Sp4, Sp2, Sp4, Sp2),
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Child = child,
        };
    }

    private Border MakeCodeBlock(string text, bool wrap, double maxHeight, bool isError = false)
    {
        var inner = MakeCodeText(text, isError);
        var scroll = new ScrollViewer
        {
            Content = inner,
            HorizontalScrollBarVisibility = wrap ? ScrollBarVisibility.Disabled : ScrollBarVisibility.Auto,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            MaxHeight = maxHeight,
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };
        return new Border
        {
            Background = ThemeBrush("CardSecondaryBrush"),
            CornerRadius = new CornerRadius(6),
            Padding = new Thickness(Sp6, Sp2, Sp6, Sp2),
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Child = scroll,
        };
    }

    private TextBlock MakeCodeText(string text, bool isError)
    {
        return new TextBlock
        {
            Text = text,
            FontFamily = CodeFont(),
            FontSize = 12,
            TextWrapping = TextWrapping.Wrap,
            IsTextSelectionEnabled = true,
            Foreground = isError ? ThemeBrush("ErrorBrush") : ThemeBrush("TextSecondaryBrush"),
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };
    }

    private static Style? AppTextStyle(string key)
        => Application.Current.Resources.TryGetValue(key, out var s) && s is Style st ? st : null;

    private static FontFamily CodeFont()
        => Application.Current.Resources.TryGetValue("CodeFontFamily", out var f) && f is FontFamily ff
            ? ff
            : new FontFamily("Consolas");

    private Brush SoftBrush(string key, double alpha)
        => SoftBrushFrom(ThemeBrush(key), alpha);

    private static Brush SoftBrushFrom(Brush brush, double alpha)
    {
        if (brush is SolidColorBrush solid)
        {
            var c = solid.Color;
            c.A = (byte)Math.Clamp((int)(255 * alpha), 0, 255);
            return new SolidColorBrush(c);
        }
        return new SolidColorBrush(Colors.Transparent);
    }

    // ---------------- 结果/参数解析辅助 ----------------

    private static JsonElement? ParseArgsObject(string? argsRaw)
    {
        if (string.IsNullOrWhiteSpace(argsRaw))
        {
            return null;
        }
        try
        {
            using var doc = JsonDocument.Parse(argsRaw);
            return doc.RootElement.ValueKind == JsonValueKind.Object ? doc.RootElement.Clone() : null;
        }
        catch (JsonException)
        {
            return null;
        }
    }

    private static string? PickArgString(JsonElement? args, string key)
        => args is { } a && a.ValueKind == JsonValueKind.Object &&
           a.TryGetProperty(key, out var v) && v.ValueKind == JsonValueKind.String
            ? v.GetString()
            : null;

    private static string FormatToolJson(string? raw)
        => string.IsNullOrEmpty(raw) ? "" : PrettyJson(raw);

    private static string PrettyJson(string raw)
    {
        try
        {
            using var doc = JsonDocument.Parse(raw);
            var options = new JsonSerializerOptions { WriteIndented = true };
            return JsonSerializer.Serialize(doc.RootElement, options);
        }
        catch (Exception)
        {
            return raw;
        }
    }

    /// <summary>官方 classifyTool 口径（子集：壳侧定制卡用）。</summary>
    private static string ClassifyToolVariant(string toolName) => toolName switch
    {
        "bash" or "pwsh" or "terminal_send" => "bash",
        "read" or "read_image" or "web_fetch" => "read",
        "web_search" or "grep" or "glob" => "search",
        "write" => "write",
        "edit" or "str_replace_editor" => "edit",
        "run_code" => "code",
        _ => "others",
    };

    /// <summary>官方 parseExitStatus：尾部 [exit code: N] / [killed by signal: X]。
    /// 无标记按 exit 0（bash 非零才打标记）。[timed out after Nms] 单独记超时。</summary>
    private static (string Output, int? ExitCode, string? Signal, bool TimedOut) ParseExitStatus(string text)
    {
        var timedOut = false;
        var body = text ?? "";
        if (System.Text.RegularExpressions.Regex.IsMatch(body, "\\n\\[timed out after [^\\]\\n]+\\]$", System.Text.RegularExpressions.RegexOptions.None))
        {
            timedOut = true;
            body = System.Text.RegularExpressions.Regex.Replace(body, "\\n\\[timed out after [^\\]\\n]+\\]$", "");
        }
        var signalMatch = System.Text.RegularExpressions.Regex.Match(body, "\\n\\[killed by signal: ([^\\]\\n]+)\\]$");
        if (signalMatch.Success)
        {
            return (body[..signalMatch.Index], null, signalMatch.Groups[1].Value, timedOut);
        }
        var exitMatch = System.Text.RegularExpressions.Regex.Match(body, "\\n\\[exit code: (\\d+)\\]$");
        if (exitMatch.Success)
        {
            var code = int.Parse(exitMatch.Groups[1].Value);
            return (body[..exitMatch.Index], code, null, timedOut);
        }
        return (body, 0, null, timedOut);
    }

    /// <summary>bash 输出拆流：stdout 在前，[stderr] 段之后是 stderr（dsh-tool-bash renderResult）。</summary>
    private static (string Stdout, string Stderr) SplitShellStreams(string body)
    {
        const string marker = "[stderr]\n";
        var text = body ?? "";
        var idx = text.IndexOf("\n" + marker, StringComparison.Ordinal);
        if (idx < 0 && text.StartsWith(marker, StringComparison.Ordinal))
        {
            return ("", text[marker.Length..]);
        }
        if (idx < 0)
        {
            return (text, "");
        }
        return (text[..idx], text[(idx + 1 + marker.Length)..]);
    }

}
