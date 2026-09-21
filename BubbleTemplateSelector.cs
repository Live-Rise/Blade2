using System;
using System.Collections.Generic;
using System.Text.RegularExpressions;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Media;

namespace Blade2;

/// <summary>聊天气泡模板选择器：按 Role 从容器的 Resources 取 DataTemplate。</summary>
public sealed partial class BubbleTemplateSelector : DataTemplateSelector
{
    protected override DataTemplate? SelectTemplateCore(object item, DependencyObject container)
    {
        if (item is ChatBubble bubble && container is FrameworkElement fe)
        {
            var key = bubble.Role switch
            {
                "user" => "UserTpl",
                // 用户附件图片（历史回读，见聊天流 session/attachment 渲染）
                "user-image" => "UserImageTpl",
                "assistant" => "AssistantTpl",
                // 推理块：可折叠「思考」行（对标官方 ReasoningRow）
                "reasoning" => "ReasoningTpl",
                // 工具调用行：图标 + 工具标题 + 参数摘要（对标官方 ToolRow）
                "tool-call" => "ToolCallTpl",
                // 「少女祈祷中」占位行：会话忙碌、首个增量未到时的脉冲进度环
                "pending" => "PendingTpl",
                // deliverables/presented → 文件卡（打开/显示走内核宿主路由）
                "deliverable" => "DeliverableTpl",
                _ => "ToolTpl",
            };
            var node = fe;
            while (node is not null)
            {
                if (node.Resources.TryGetValue(key, out var t) && t is DataTemplate dt)
                {
                    return dt;
                }
                node = Microsoft.UI.Xaml.Media.VisualTreeHelper.GetParent(node) as FrameworkElement;
            }
        }
        return base.SelectTemplateCore(item, container);
    }
}

/// <summary>
/// Markdown → RichTextBlock 简易渲染器（代码块/行内代码/粗体/斜体/标题/列表/引用）。
/// 覆盖助手回复的主流结构；不追求 CommonMark 全集——复杂表格等按原文保留。
/// </summary>
public static partial class MarkdownRenderer
{
    [GeneratedRegex(@"```(?<lang>\w*)\r?\n(?<code>.*?)```", RegexOptions.Singleline)]
    private static partial Regex CodeBlockRegex();

    [GeneratedRegex(@"`(?<code>[^`\n]+)`|\*\*(?<b>[^*\n]+)\*\*|\*(?<i>[^*\n]+)\*|##\s(?<h>[^\n]+)")]
    private static partial Regex InlineRegex();

    // 逐字流每 80ms 就要解析一次，Regex() 每次都新建实例：缓存下来。
    private static readonly Regex CodeBlock = CodeBlockRegex();
    private static readonly Regex Inline = InlineRegex();

    /// <summary>渲染一个 StackPanel：代码块与富文本段交替排列。
    /// 根面板强制 Stretch，避免在 ContentControl 里按内容期望宽测量导致正文不换行。</summary>
    public static StackPanel Render(string markdown) => Append(null, markdown);

    /// <summary>在已渲染面板上续写增量文本：已完成的块原样留着，只把增量并进尾块。
    /// 整树重建在长回复下每帧都要重排一次，正是逐字「一个字等很久」的来源；
    /// 增量里带 markdown 标记、或前缀对不上（容器被虚拟化回收、文本被定稿覆盖）时退回全量重建。</summary>
    public static StackPanel Append(StackPanel? panel, string markdown)
    {
        panel ??= new StackPanel
        {
            Spacing = 6,
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };
        void Rebuild()
        {
            panel.Children.Clear();
            AppendBlocks(panel, markdown);
            panel.Tag = markdown;
        }
        var keep = 0;         // 可原样保留的块数
        var keptLength = 0;   // 这些块覆盖的原文长度
        RichState? lastState = null;
        while (keep < panel.Children.Count)
        {
            // 代码块一律不参与续写：闭合围栏一出现就该整段重排
            var src = (panel.Children[keep] as RichTextBlock)?.Tag as RichState;
            if (src is null || !markdown.StartsWith(src.Source, StringComparison.Ordinal))
            {
                break;
            }
            keep++;
            keptLength += src.Source.Length;
            lastState = src;
        }
        if (panel.Children.Count > 0 && keep == 0)
        {
            Rebuild();
            return panel;
        }
        while (panel.Children.Count > keep)
        {
            panel.Children.RemoveAt(panel.Children.Count - 1);
        }
        var tail = markdown[keptLength..];
        if (keep > 0 && tail.Length > 0 && lastState is { Tail: { } run } && PlainToExtend(tail) && !run.Text.Contains('`'))
        {
            run.Text += tail;
            lastState.Source += tail;
            panel.Tag = markdown;
            return panel;
        }
        Rebuild();
        return panel;
    }

    /// <summary>富文本段的续写状态：已消费原文 + 末尾可续写的裸 Run（构建期记下，避免事后走文本树）。</summary>
    private sealed class RichState
    {
        public string Source = "";
        public Run? Tail;
    }

    /// <summary>能否直接并进已有富文本段：不含反引号，且未闭合的 * 不会跨帧改变样式归属。</summary>
    private static bool PlainToExtend(string tail)
    {
        var stars = 0;
        foreach (var c in tail)
        {
            if (c == '*')
            {
                stars++;
            }
        }
        return stars % 2 == 0;
    }

    private static void AppendBlocks(StackPanel root, string markdown)
    {
        var matches = CodeBlock.Matches(markdown);
        var cursor = 0;
        foreach (Match m in matches)
        {
            if (m.Index > cursor)
            {
                root.Children.Add(RenderRichText(markdown[cursor..m.Index]));
            }
            var lang = m.Groups["lang"].Value;
            var code = m.Groups["code"].Value.TrimEnd();
            root.Children.Add(MakeCodeBlock(lang, code));
            cursor = m.Index + m.Length;
        }
        if (cursor < markdown.Length)
        {
            root.Children.Add(RenderRichText(markdown[cursor..]));
        }
    }

    /// <summary>富文本段：行内 code/粗体/斜体/标题混排。Tag 记已消费原文与末尾裸 Run 供增量续写。</summary>
    private static RichTextBlock RenderRichText(string text)
    {
        var state = new RichState { Source = text };
        var rtb = new RichTextBlock
        {
            TextWrapping = TextWrapping.Wrap,
            IsTextSelectionEnabled = true,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Tag = state,
        };
        var para = new Paragraph();
        rtb.Blocks.Add(para);

        var inline = Inline;
        var cursor = 0;
        foreach (Match m in inline.Matches(text))
        {
            if (m.Index > cursor)
            {
                para.Inlines.Add(new Run { Text = text[cursor..m.Index] });
            }
            if (m.Groups["code"].Success)
            {
                var run = new Run { Text = m.Groups["code"].Value, FontFamily = new FontFamily("Consolas") };
                para.Inlines.Add(run);
            }
            else if (m.Groups["b"].Success)
            {
                para.Inlines.Add(new Run { Text = m.Groups["b"].Value, FontWeight = Microsoft.UI.Text.FontWeights.Bold });
            }
            else if (m.Groups["i"].Success)
            {
                para.Inlines.Add(new Run { Text = m.Groups["i"].Value, FontStyle = Windows.UI.Text.FontStyle.Italic });
            }
            else if (m.Groups["h"].Success)
            {
                para.Inlines.Add(new Run { Text = m.Groups["h"].Value, FontWeight = Microsoft.UI.Text.FontWeights.Bold, FontSize = 16 });
            }
            cursor = m.Index + m.Length;
        }
        if (cursor < text.Length)
        {
            state.Tail = new Run { Text = text[cursor..] };
            para.Inlines.Add(state.Tail);
        }
        return rtb;
    }

    /// <summary>代码块：深底等宽卡片 + 语言角标（后续可接高亮）。
    /// 长行放在内部 ScrollViewer 里横滚，避免 NoWrap 把整条聊天流撑破。</summary>
    private static Border MakeCodeBlock(string lang, string code)
    {
        var header = new TextBlock
        {
            Text = string.IsNullOrEmpty(lang) ? "code" : lang,
            FontSize = 11,
            Opacity = 0.6,
            Margin = new Thickness(12, 8, 12, 0),
        };
        var bodyText = new TextBlock
        {
            Text = code,
            FontFamily = new FontFamily("Consolas"),
            IsTextSelectionEnabled = true,
            TextWrapping = TextWrapping.NoWrap,
            Margin = new Thickness(12, 2, 12, 10),
        };
        var body = new ScrollViewer
        {
            Content = bodyText,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
            VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
            HorizontalScrollMode = ScrollMode.Enabled,
            VerticalScrollMode = ScrollMode.Disabled,
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };
        var grid = new Grid { HorizontalAlignment = HorizontalAlignment.Stretch };
        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        grid.Children.Add(header);
        Grid.SetRow(header, 0);
        grid.Children.Add(body);
        Grid.SetRow(body, 1);

        return new Border
        {
            // 代码块底随主题：深色用近黑卡片，浅色用浅灰（写死单色会在另一主题下不可读）
            Background = IsDarkTheme()
                ? new SolidColorBrush(Windows.UI.Color.FromArgb(255, 32, 32, 36))
                : new SolidColorBrush(Windows.UI.Color.FromArgb(255, 240, 240, 244)),
            CornerRadius = new CornerRadius(8),
            Margin = new Thickness(0, 2, 0, 2),
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Child = grid,
        };
    }

    /// <summary>窗口主题深浅：气泡在窗口内容树内，按 App.MainWindow 的 RequestedTheme 判断。</summary>
    private static bool IsDarkTheme()
    {
        try
        {
            if (App.MainWindow is MainWindow { Content: FrameworkElement content })
            {
                return content.RequestedTheme != ElementTheme.Light;
            }
        }
        catch (Exception) { }
        return true;
    }
}
