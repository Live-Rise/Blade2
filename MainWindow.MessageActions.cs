// ---------------- 消息级操作与过程行（对标官方端 chrome） ----------------
// 本文件承载四组功能（全部取数自 journal 事件信封的 seq/time 与 tool/call 参数）：
//   1. 用户消息操作行：时间戳 + 复制（官方 UserStyleBubble 的 clock:"start" chrome）
//   2. 助手操作行扩充：复制 / 在新对话中分支 / 时间戳 / 本轮用时（官方 TurnTailNodeView）
//   3. 推理块：可折叠「思考」行（官方 ReasoningRow，默认折叠、首行作摘要）
//   4. 工具调用行：图标 + 工具标题 + 参数摘要（官方 ToolRow / toolRowModel）
// 分支走内核 session/fork { sessionId, atSeq }（dsh-client-connection：边界 = atSeq 之后
// 第一个 turn/end），子会话标题按官方 increasedForkTitle 追加序号后经 session/rename 回写。

using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using Blade2.Dsh;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Shapes;
using Windows.Foundation;

namespace Blade2;

public sealed partial class MainWindow
{
    // ---------------- 用户消息操作行（时间戳 + 复制） ----------------

    /// <summary>用户气泡操作行装载。ListView 虚拟化会回收容器：挂 DataContextChanged 兜底，
    /// 与助手气泡 MdHost 同一套防串写策略（Tag 幂等挂钩）。</summary>
    private void OnUserBubbleLoaded(object sender, RoutedEventArgs e)
    {
        if (sender is not ContentControl host)
        {
            return;
        }
        if (host.Tag is not "user-actions-hooked")
        {
            host.Tag = "user-actions-hooked";
            host.DataContextChanged += OnUserHostDataContextChanged;
        }
        BuildUserActionsRow(host);
    }

    private void OnUserHostDataContextChanged(FrameworkElement sender, DataContextChangedEventArgs args)
    {
        if (sender is ContentControl host)
        {
            BuildUserActionsRow(host);
        }
    }

    private void BuildUserActionsRow(ContentControl host)
    {
        if (host.DataContext is not ChatBubble bubble)
        {
            return;
        }
        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = TokenDouble("Space4", 4),
            HorizontalAlignment = HorizontalAlignment.Right,
            Opacity = 0.62,
        };
        if (bubble.Time > 0)
        {
            row.Children.Add(new TextBlock
            {
                Text = FormatMessageClock(bubble.Time),
                Style = Application.Current.Resources.TryGetValue("HintTextStyle", out var hint) && hint is Style hs ? hs : null,
                VerticalAlignment = VerticalAlignment.Center,
            });
        }
        // 时间后面跟模型型号（只显示 id，不显示展示名）：内核 request/header 记的是实际发起
        // 请求那次用的模型，比壳里记的"当前选择"更接近真相。没有记录的历史消息不占位。
        if (bubble.Model.Length > 0)
        {
            row.Children.Add(new TextBlock
            {
                Text = "· " + bubble.Model,
                Style = Application.Current.Resources.TryGetValue("HintTextStyle", out var hint2) && hint2 is Style hs2 ? hs2 : null,
                VerticalAlignment = VerticalAlignment.Center,
            });
        }
        row.Children.Add(MakeCopyButton(bubble.Text));
        // 撤回编辑：进程还在跑、本轮又没动过文件时，才允许把这条提问收回去重编
        if (CanWithdrawEdit(bubble))
        {
            row.Children.Add(MakeWithdrawEditButton(bubble));
        }
        // 行随指针显隐（官方 user chrome 的 hover 提亮）
        row.PointerEntered += (_, _) => row.Opacity = 1;
        row.PointerExited += (_, _) => row.Opacity = 0.62;
        host.Content = row;
    }

    /// <summary>能不能撤回这条提问的编辑：候选正是它、进程还在跑（busy 或占位气泡在）、
    /// 且本轮还没动过文件。动过文件就不给——内核侧状态已经改了，界面单方撤回会让用户
    /// 以为「回到了发送前」，实际文件已被改写（那种情况该用轮尾的「撤回本轮修改」）。</summary>
    private bool CanWithdrawEdit(ChatBubble bubble)
    {
        if (bubble.Withdrawn || _turnMutated)
        {
            return false;
        }
        if (_withdrawCandidate is not { } candidate || !ReferenceEquals(candidate, bubble))
        {
            return false;
        }
        // 会话已停（无 turn/end 的失败轮次）：没有什么可停的，按钮不该出现
        return IsSessionBusy() || _pendingBubble is not null;
    }

    /// <summary>撤回编辑按钮（官方 user chrome 的编辑入口）：停止运行 + 原文回填输入框。</summary>
    private Button MakeWithdrawEditButton(ChatBubble bubble)
    {
        var tip = L("撤回编辑（停止运行并把这句放回输入框）");
        var button = new Button
        {
            Width = SmallButtonSize,
            Height = SmallButtonSize,
            MinWidth = 0,
            Padding = new Thickness(0),
            CornerRadius = RadSmall,
            VerticalAlignment = VerticalAlignment.Center,
            Content = new FontIcon { Glyph = "\uE70F", FontSize = GlyphBody }, // Edit
        };
        ToolTipService.SetToolTip(button, tip);
        Aut(button, "UserWithdrawEditButton", tip);
        button.Click += (_, _) => _ = WithdrawEditAsync(bubble);
        return button;
    }

    /// <summary>撤回编辑：先停进程（复用发送键的停止语义 session/cancel），再把提问原文放回
    /// 输入框。内核 journal 只增不改，撤不回已落盘的事件，所以同时把该轮整段从界面上撤下并
    /// 登记抑制，补拉/轮询/切会话回放都不会把它带回来。</summary>
    private async Task WithdrawEditAsync(ChatBubble bubble)
    {
        await StopActiveRunAsync();
        if (DispatcherQueue.HasThreadAccess)
        {
            WithdrawEditCore(bubble);
        }
        else
        {
            PostUi(() => WithdrawEditCore(bubble));
        }
    }

    private void WithdrawEditCore(ChatBubble bubble)
    {
        var sid = Volatile.Read(ref _activeSessionId);
        // 提问原文回填输入框（附件不恢复：图片已在发送时从待发区清掉，重新编辑时按需再贴）
        InputBox.Text = bubble.Text;
        InputBox.Focus(FocusState.Programmatic);
        InputBox.SelectionStart = InputBox.Text.Length;

        // 抑制登记：正在跑的轮次直接标记；turn/start 还没到窗口就挂起，等它到达时补登记
        if (_runTurn > 0 && sid is { Length: > 0 })
        {
            WithdrawnTurns(sid).Add(_runTurn);
        }
        else
        {
            _withdrawPendingTurn = true;
        }
        if (bubble.Seq > 0 && sid is { Length: > 0 })
        {
            WithdrawnSeqs(sid).Add(bubble.Seq);
        }

        // 该轮后续气泡随提问一起撤下：从提问之后直到下一条提问（排队中的消息）为止。
        // 提问气泡本身留在 _messages 里（标记 Withdrawn、不进可见投影）——本地回显认领路径
        // 要靠它把 seq 补记进抑制表，见 NoteWithdrawnSeq。
        var index = _messages.IndexOf(bubble);
        if (index >= 0)
        {
            for (var i = _messages.Count - 1; i > index; i--)
            {
                if (_messages[i].Role == "user")
                {
                    break; // 排队中的下一条提问：它属于下一轮，留着
                }
                var dropped = _messages[i];
                _messages.RemoveAt(i);
                if (dropped.Role == "user-image")
                {
                    // 图片认领登记跟着回显一起作废：否则重发同名图会被误认领成「已在屏上」
                    ReleaseLocalImageEcho(dropped.Text);
                }
            }
            bubble.Withdrawn = true;
        }
        _withdrawCandidate = null;
        // 逐字增量流属于这一轮：气泡撤下后 stray chunk 会按空桶重新插一条，整表作废
        ForgetLiveAttempts();
        RefreshTranscriptView();
        // 进程已停：忙碌态按钮回发送箭头（turn/end 到达时也会算一次，这里先按取消结果重算）
        UpdateComposerRunningState();
    }

    /// <summary>就地重算用户操作行：气泡挂出后模型/时间才被 journal 记录认领（本地回显补 seq 与
    /// 实际模型）时用。容器未 realize 直接返回——屏外气泡将来 realize 时按最新值装配。</summary>
    private void RepaintUserActions(ChatBubble bubble)
    {
        if (ChatList.ContainerFromItem(bubble) is not { } container)
        {
            return;
        }
        if (FindUserActionsHost(container, bubble, 0) is { } host)
        {
            BuildUserActionsRow(host);
        }

        static ContentControl? FindUserActionsHost(DependencyObject node, ChatBubble target, int depth)
        {
            if (depth > 24)
            {
                return null;
            }
            var count = Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChildrenCount(node);
            for (var i = 0; i < count; i++)
            {
                var child = Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChild(node, i);
                // 两个用户模板各有一个操作行宿主（UserTpl / UserImageTpl），x:Name 必须互异，
                // 否则 XamlCompiler 直接失败；这里按名字集合认领两种宿主。
                if (child is ContentControl { Name: "UserActionsHost" or "UserImageActionsHost" } cc &&
                    ReferenceEquals(cc.DataContext, target))
                {
                    return cc;
                }
                if (FindUserActionsHost(child, target, depth + 1) is { } found)
                {
                    return found;
                }
            }
            return null;
        }
    }

    /// <summary>复制按钮：点击写剪贴板，成功即换成对勾 1 秒（官方 copied 反馈）。</summary>
    private Button MakeCopyButton(string text)
    {
        var icon = new FontIcon { Glyph = "\uE8C8", FontSize = GlyphBody };
        var button = new Button
        {
            Width = SmallButtonSize,
            Height = SmallButtonSize,
            MinWidth = 0,
            Padding = new Thickness(0),
            CornerRadius = RadSmall,
            VerticalAlignment = VerticalAlignment.Center,
            Content = icon,
        };
        var tip = L("复制消息");
        ToolTipService.SetToolTip(button, tip);
        Aut(button, "MessageCopyButton", tip);
        button.Click += (_, _) =>
        {
            try
            {
                var package = new Windows.ApplicationModel.DataTransfer.DataPackage { RequestedOperation = Windows.ApplicationModel.DataTransfer.DataPackageOperation.Copy };
                package.SetText(text ?? "");
                Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(package);
                icon.Glyph = "\uE73E";
                ToolTipService.SetToolTip(button, L("已复制"));
                var timer = DispatcherQueue.CreateTimer();
                timer.Interval = TimeSpan.FromSeconds(1);
                timer.IsRepeating = false;
                timer.Tick += (_, _) =>
                {
                    icon.Glyph = "\uE8C8";
                    ToolTipService.SetToolTip(button, L("复制消息"));
                };
                timer.Start();
            }
            catch (Exception) { }
        };
        return button;
    }

    // ---------------- 推理块（可折叠「思考」行） ----------------

    /// <summary>推理块装载。官方 ReasoningRow：默认折叠，折叠态显示首行摘要，
    /// 展开显示完整文本；正文按普通文本渲染（官方 thinkBody 同口径）。</summary>
    private void OnReasoningLoaded(object sender, RoutedEventArgs e)
    {
        if (sender is not ContentControl host)
        {
            return;
        }
        if (host.Tag is not "reasoning-hooked")
        {
            host.Tag = "reasoning-hooked";
            host.DataContextChanged += OnReasoningHostDataContextChanged;
        }
        BuildReasoningRow(host);
    }

    private void OnReasoningHostDataContextChanged(FrameworkElement sender, DataContextChangedEventArgs args)
    {
        if (sender is not ContentControl host)
        {
            return;
        }
        BuildReasoningRow(host);
    }

    // ---------------- 流光扫光（loading 行的高光扫过，对标官方 reasoning-row-sweep） ----------------
    // 跑法：Rectangle 盖在行上，TranslateX 走 RenderTransform 独立动画（合成线程，
    // 不抢逐字 80ms 重绘的 UI 预算）。定稿/卸载即停，避免回收容器野跑。

    private void OnPendingLoaded(object sender, RoutedEventArgs e)
    {
        if (sender is Grid clip && clip.Children.OfType<Rectangle>().FirstOrDefault() is { } sweep)
        {
            StartSweep(sweep, clip);
        }
    }

    private void OnPendingUnloaded(object sender, RoutedEventArgs e)
    {
        if (sender is Grid clip && clip.Children.OfType<Rectangle>().FirstOrDefault() is { } sweep)
        {
            StopSweep(sweep);
        }
    }

    /// <summary>启动扫光：已在扫直接返回（80ms 重绘复用路径不得重启 storyboard）。</summary>
    private void StartSweep(Rectangle sweep, Grid clip)
    {
        if (sweep.Tag is Storyboard)
        {
            return;
        }
        EnsureSweepClip(clip);
        sweep.Fill = SweepBrush();
        sweep.Opacity = 1;
        var trans = new TranslateTransform { X = -200 };
        sweep.RenderTransform = trans;
        var da = new DoubleAnimation
        {
            From = -200,
            To = 800,
            Duration = TimeSpan.FromSeconds(2.2),
            RepeatBehavior = RepeatBehavior.Forever,
        };
        var sb = new Storyboard();
        Storyboard.SetTarget(da, trans);
        Storyboard.SetTargetProperty(da, "X");
        sb.Children.Add(da);
        sweep.Tag = sb;
        sb.Begin();
    }

    private static void StopSweep(Rectangle sweep)
    {
        if (sweep.Tag is Storyboard sb)
        {
            try
            {
                sb.Stop();
            }
            catch (Exception)
            {
            }
            sweep.Tag = null;
        }
        sweep.Opacity = 0;
    }

    /// <summary>扫光裁剪：高光飞出行的部分裁掉。SizeChanged 只刷新裁剪矩形，不重启动画。</summary>
    private static void EnsureSweepClip(Grid clip)
    {
        if (clip.Clip is not RectangleGeometry geo)
        {
            geo = new RectangleGeometry();
            clip.Clip = geo;
            clip.SizeChanged += (_, _) =>
            {
                if (clip.Clip is RectangleGeometry g)
                {
                    g.Rect = new Rect(0, 0, clip.ActualWidth, clip.ActualHeight);
                }
            };
        }
        geo.Rect = new Rect(0, 0, clip.ActualWidth, clip.ActualHeight);
    }

    /// <summary>扫光带：透明 → Info 色弱透明 → 透明。Info 令牌深浅主题都可见。</summary>
    private LinearGradientBrush SweepBrush()
    {
        var glow = ThemeBrush("InfoBrush") is SolidColorBrush solid
            ? solid.Color
            : Microsoft.UI.Colors.SkyBlue;
        glow.A = (byte)(glow.A * 0.35);
        return new LinearGradientBrush
        {
            StartPoint = new Point(0, 0.5),
            EndPoint = new Point(1, 0.5),
            GradientStops =
            {
                new GradientStop { Color = Microsoft.UI.Colors.Transparent, Offset = 0 },
                new GradientStop { Color = glow, Offset = 0.5 },
                new GradientStop { Color = Microsoft.UI.Colors.Transparent, Offset = 1 },
            },
        };
    }

    /// <summary>推理行扫光跟随 streaming 状态：流式中起，定稿停。</summary>
    private void SyncReasoningSweep(ChatBubble bubble, Rectangle sweep, Grid clip)
    {
        if (bubble.IsLiveStreaming)
        {
            StartSweep(sweep, clip);
        }
        else
        {
            StopSweep(sweep);
        }
    }

    private void BuildReasoningRow(ContentControl host)
    {
        if (host.DataContext is not ChatBubble bubble || bubble.Text.Length == 0)
        {
            return;
        }
        // 逐字增量每 80ms 调一次：整行重建会让按钮/可视树反复重挂。同一气泡已有行时只改文本，
        // 扫光按 streaming 状态起停（在跑的不重启）。
        if (host.Content is StackPanel reuse &&
            reuse.Tag is (ChatBubble sameBubble, TextBlock oldSummary, TextBlock oldBody, Rectangle oldSweep, Grid oldClip) &&
            ReferenceEquals(sameBubble, bubble))
        {
            oldSummary.Text = FirstLine(bubble.Text);
            oldBody.Text = bubble.Text;
            SyncReasoningSweep(bubble, oldSweep, oldClip);
            return;
        }
        var chevron = new FontIcon { Glyph = "\uE76B", FontSize = 10 };   // 折叠态朝右
        var title = new TextBlock
        {
            Text = L("思考"),
            Style = Application.Current.Resources.TryGetValue("CaptionTextStyle", out var caption) && caption is Style cs ? cs : null,
            VerticalAlignment = VerticalAlignment.Center,
        };
        var summary = new TextBlock
        {
            Text = FirstLine(bubble.Text),
            Style = Application.Current.Resources.TryGetValue("HintTextStyle", out var hint) && hint is Style hs ? hs : null,
            TextTrimming = TextTrimming.CharacterEllipsis,
            TextWrapping = TextWrapping.NoWrap,
            VerticalAlignment = VerticalAlignment.Center,
            MaxWidth = 480,
        };
        var body = new TextBlock
        {
            Text = bubble.Text,
            TextWrapping = TextWrapping.Wrap,
            IsTextSelectionEnabled = true,
            Opacity = 0.78,
            Margin = new Thickness(TokenDouble("Space20", 20), TokenDouble("Space2", 2), 0, 0),
            Visibility = Visibility.Collapsed,
        };
        var header = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = TokenDouble("Space6", 6),
        };
        header.Children.Add(chevron);
        header.Children.Add(title);
        header.Children.Add(summary);
        var root = new StackPanel
        {
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Margin = new Thickness(6, 2, 6, 2),
        };
        var headerButton = new ToggleButton
        {
            Content = header,
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            Padding = new Thickness(TokenDouble("Space4", 4), TokenDouble("Space2", 2), TokenDouble("Space4", 4), TokenDouble("Space2", 2)),
            HorizontalAlignment = HorizontalAlignment.Left,
            HorizontalContentAlignment = HorizontalAlignment.Left,
            CornerRadius = RadSmall,
        };
        Aut(headerButton, "ReasoningToggle", L("思考"));
        ToolTipService.SetToolTip(headerButton, L("思考"));
        headerButton.Checked += (_, _) => { body.Visibility = Visibility.Visible; chevron.Glyph = "\uE76C"; bubble.ReasoningExpanded = true; };
        headerButton.Unchecked += (_, _) => { body.Visibility = Visibility.Collapsed; chevron.Glyph = "\uE76B"; bubble.ReasoningExpanded = false; };
        // 官方默认折叠；逐字增量每 80ms 重建整行时按气泡上的状态恢复，用户手动展开的不会被弹回
        headerButton.IsChecked = bubble.ReasoningExpanded;
        // 扫光层：header 包一层裁剪 Grid，高光 Rectangle 盖在上（不抢点击）。
        var clip = new Grid { HorizontalAlignment = HorizontalAlignment.Stretch };
        clip.Children.Add(headerButton);
        var sweep = new Rectangle
        {
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Stretch,
            Width = 140,
            Opacity = 0,
            IsHitTestVisible = false,
        };
        clip.Children.Add(sweep);
        root.Children.Add(clip);
        root.Children.Add(body);
        root.Tag = (bubble, summary, body, sweep, clip);
        host.Content = root;
        SyncReasoningSweep(bubble, sweep, clip);
    }

    // ---------------- 工具调用行（图标 + 标题 + 参数摘要） ----------------

    /// <summary>工具调用行装载。行内容从 DataContext（tool/call 气泡）装配，
    /// 回收后按新 DataContext 重建（同助手气泡防串写策略）。</summary>
    private void OnToolCallLoaded(object sender, RoutedEventArgs e)
    {
        if (sender is not ContentControl host)
        {
            return;
        }
        if (host.Tag is not "toolcall-hooked")
        {
            host.Tag = "toolcall-hooked";
            host.DataContextChanged += OnToolCallHostDataContextChanged;
        }
        BuildToolCallRow(host);
    }

    private void OnToolCallHostDataContextChanged(FrameworkElement sender, DataContextChangedEventArgs args)
    {
        if (sender is ContentControl host)
        {
            BuildToolCallRow(host);
        }
    }

    private void BuildToolCallRow(ContentControl host)
    {
        if (host.DataContext is not ChatBubble bubble)
        {
            return;
        }
        // 同气泡再来（result 到、状态翻转）：行内容不变，只按执行态起停扫光。
        // 整行重建会让 sweep Rectangle 连着 Storyboard 一起被丢弃，在跑的动画就此野跑。
        if (host.Content is Grid reuseRoot &&
            reuseRoot.Tag is (ChatBubble sameBubble, Rectangle reuseSweep, Grid reuseClip) &&
            ReferenceEquals(sameBubble, bubble))
        {
            SyncToolSweep(bubble, reuseSweep, reuseClip);
            return;
        }
        // 回收容器换了气泡：旧行的扫光若还在跑（异常路径未收尾），换内容前停掉。
        if (host.Content is Grid oldRoot && oldRoot.Tag is (_, Rectangle oldSweep, _))
        {
            StopSweep(oldSweep);
        }
        var (glyph, title, summary) = ToolRowModel(bubble.ToolName ?? "", bubble.ToolArgs ?? "");
        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = TokenDouble("Space6", 6),
            Padding = new Thickness(6, 2, 6, 2),
            Opacity = 0.72,
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };
        row.Children.Add(new FontIcon { Glyph = glyph, FontSize = 12, Opacity = 0.8 });
        row.Children.Add(new TextBlock
        {
            Text = title,
            Style = Application.Current.Resources.TryGetValue("CaptionTextStyle", out var caption) && caption is Style cs ? cs : null,
            VerticalAlignment = VerticalAlignment.Center,
        });
        if (summary.Length > 0)
        {
            row.Children.Add(new TextBlock
            {
                Text = "·",
                Style = Application.Current.Resources.TryGetValue("CaptionTextStyle", out var sep) && sep is Style ss ? ss : null,
                Opacity = 0.5,
                VerticalAlignment = VerticalAlignment.Center,
            });
            row.Children.Add(new TextBlock
            {
                Text = summary,
                Style = Application.Current.Resources.TryGetValue("CaptionTextStyle", out var hint2) && hint2 is Style hs2 ? hs2 : null,
                Opacity = 0.85,
                TextTrimming = TextTrimming.CharacterEllipsis,
                TextWrapping = TextWrapping.NoWrap,
                VerticalAlignment = VerticalAlignment.Center,
                HorizontalAlignment = HorizontalAlignment.Stretch,
            });
        }
        Aut(row, "ToolCallRow", bubble.Text);
        // 扫光层：行外包一层裁剪 Grid，高光 Rectangle 盖在上（IsHitTestVisible 不抢点击）。
        // 执行中（IsToolRunning）起扫，result 到/轮尾收尾即停——与 reasoning 行同机制。
        var clip = new Grid { HorizontalAlignment = HorizontalAlignment.Stretch };
        clip.Children.Add(row);
        var sweep = new Rectangle
        {
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Stretch,
            Width = 140,
            Opacity = 0,
            IsHitTestVisible = false,
        };
        clip.Children.Add(sweep);
        clip.Tag = (bubble, sweep, clip);
        host.Content = clip;
        SyncToolSweep(bubble, sweep, clip);
    }

    /// <summary>工具行扫光跟随执行态：tool/call 后起，tool/result 到（或轮尾收尾）即停。</summary>
    private void SyncToolSweep(ChatBubble bubble, Rectangle sweep, Grid clip)
    {
        if (bubble.IsToolRunning)
        {
            StartSweep(sweep, clip);
        }
        else
        {
            StopSweep(sweep);
        }
    }

    /// <summary>工具行展示模型（对标官方 classifyTool / TOOL_TITLE_KEYS / deriveSummary）：
    /// 返回（图标、标题、摘要）。TitleKey 为 zh 键（壳 L() 的键即中文原文），
    /// 已知名直接出字面（Grep/Glob/Bash 等官方也不翻译）。</summary>
    private static readonly (string Name, string Glyph, string TitleKey, string Variant)[] ToolTable =
    {
        ("bash",        "\uE756", "Bash",     "bash"),
        ("pwsh",        "\uE756", "Pwsh",     "bash"),
        ("read",        "\uE8A5", "读取",      "read"),
        ("read_image",  "\uEB9F", "读取图片",  "read"),
        ("web_fetch",   "\uE774", "网页获取",  "read"),
        ("web_search",  "\uE721", "网页搜索",  "search"),
        ("grep",        "\uE721", "Grep",     "search"),
        ("glob",        "\uE721", "Glob",     "search"),
        ("write",       "\uE70F", "写入",      "write"),
        ("edit",        "\uE70F", "编辑",      "edit"),
        ("run_code",    "\uE943", "代码",      "code"),
    };

    /// <summary>tool/call 气泡的 Text（=「标题 · 摘要」）：Adjacent 去重与 UIA 名称的数据源。
    /// 摘要取材自 arguments（官方 deriveSummary 键序），解析失败回落原文首行。</summary>
    private string ToolRowFallbackText(string toolName, string argsRaw)
    {
        var (_, title, summary) = ToolRowModel(toolName, argsRaw);
        return summary.Length > 0 ? $"{title} · {summary}" : title;
    }

    /// <summary>官方 toolRowModel 的壳侧移植：变体分类 + 参数摘要（firstLine，不追
    /// relativizeToCwd/abbreviateHome——内核 arguments 里已是工作区相对路径口径）。</summary>
    private (string Glyph, string Title, string Summary) ToolRowModel(string toolName, string argsRaw)
    {
        var variant = "others";
        var glyph = "\uE8A5";
        var titleKey = "";
        foreach (var t in ToolTable)
        {
            if (t.Name == toolName)
            {
                variant = t.Variant;
                glyph = t.Glyph;
                titleKey = t.TitleKey;
                break;
            }
        }
        var title = titleKey.Length > 0 ? L(titleKey) : L("工具调用");
        var summary = DeriveToolSummary(variant, argsRaw);
        // 未知名（官方口径）：标题回落通用「工具调用」，摘要前缀工具名（与官方 `${toolName} · ${base}` 同构）
        if (titleKey.Length == 0 && toolName.Length > 0)
        {
            return (glyph, title, summary.Length > 0 ? $"{toolName} · {summary}" : toolName);
        }
        return (glyph, title, summary);
    }

    /// <summary>官方 SUMMARY_KEYS 的键序（read/search 取 path/file_path/url，bash 取
    /// description/command，search 支持 queries 数组，code 取 description）。</summary>
    private static string DeriveToolSummary(string variant, string argsRaw)
    {
        if (string.IsNullOrEmpty(argsRaw))
        {
            return "";
        }
        JsonElement args;
        try
        {
            using var doc = JsonDocument.Parse(argsRaw);
            // JsonDocument 释放后元素不可用：克隆出独立的 JsonElement
            args = doc.RootElement.Clone();
        }
        catch (JsonException)
        {
            return FirstLine(argsRaw);
        }
        if (args.ValueKind != JsonValueKind.Object)
        {
            return FirstLine(argsRaw);
        }
        switch (variant)
        {
            case "bash":
                if (PickString(args, "description") is { } d) return FirstLine(d);
                if (PickString(args, "command") is { } c) return FirstLine(c);
                break;
            case "read":
                if (PickString(args, "path") is { } p1) return FirstLine(p1);
                if (PickString(args, "file_path") is { } p2) return FirstLine(p2);
                if (PickString(args, "url") is { } u1) return FirstLine(u1);
                break;
            case "search":
                if (args.TryGetProperty("queries", out var queries) && queries.ValueKind == JsonValueKind.Array)
                {
                    var parts = new List<string>();
                    foreach (var q in queries.EnumerateArray())
                    {
                        if (q.ValueKind == JsonValueKind.String && q.GetString() is { Length: > 0 } qs)
                        {
                            parts.Add(FirstLine(qs));
                        }
                    }
                    if (parts.Count > 0) return string.Join(", ", parts);
                }
                if (PickString(args, "query") is { } q1) return FirstLine(q1);
                if (PickString(args, "pattern") is { } p3) return FirstLine(p3);
                if (PickString(args, "url") is { } u2) return FirstLine(u2);
                break;
            case "write":
            case "edit":
                if (PickString(args, "path") is { } w1) return FirstLine(w1);
                if (PickString(args, "file_path") is { } w2) return FirstLine(w2);
                break;
            case "code":
                if (PickString(args, "description") is { } cd) return FirstLine(cd);
                break;
        }
        // 兜底：第一个字符串参数（官方同款）
        foreach (var v in args.EnumerateObject())
        {
            if (v.Value.ValueKind == JsonValueKind.String && v.Value.GetString() is { Length: > 0 } s)
            {
                return FirstLine(s);
            }
        }
        return FirstLine(argsRaw);
    }

    private static string? PickString(JsonElement args, string key)
        => args.TryGetProperty(key, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;

    private static string FirstLine(string text)
    {
        var nl = text.IndexOf('\n');
        return nl == -1 ? text : text[..nl];
    }

    // ---------------- 时间戳与用时（官方 formatMessageClock / formatDuration 口径） ----------------

    /// <summary>官方 formatMessageClock：同日 → HH:mm；同年 → 「{m}月{d}日 HH:mm」；
    /// 跨年 → 「{y}年{m}月{d}日 HH:mm」。24 小时制补零。</summary>
    private string FormatMessageClock(long timeMs)
    {
        var d = DateTimeOffset.FromUnixTimeMilliseconds(timeMs).ToLocalTime();
        var now = DateTimeOffset.Now;
        var clock = d.ToString("HH:mm", CultureInfo.InvariantCulture);
        if (d.Year == now.Year && d.Month == now.Month && d.Day == now.Day)
        {
            return clock;
        }
        if (d.Year == now.Year)
        {
            return LF("{0}月{1}日 {2}", d.Month, d.Day, clock);
        }
        return LF("{0}年{1}月{2}日 {3}", d.Year, d.Month, d.Day, clock);
    }

    /// <summary>官方 formatDuration（compact）：不足 1 分钟给「45.2秒」（0.1s 精度），
    /// 之后给「{m}分{ss}秒」（秒补零，如 6分01秒）。</summary>
    private string FormatTurnDuration(double ms)
    {
        var s = Math.Max(0, ms) / 1000.0;
        if (s < 60)
        {
            var rounded = Math.Round(s * 10) / 10;
            return LF("{0}秒", rounded.ToString("0.#", CultureInfo.InvariantCulture));
        }
        var whole = (long)Math.Round(s);
        return LF("{0}分{1}秒", whole / 60, (whole % 60).ToString("00", CultureInfo.InvariantCulture));
    }

    // ---------------- 在新对话中分支（session/fork + atSeq） ----------------

    /// <summary>从指定事件 seq 分叉出新会话并打开（官方 forkAt 链路）。内核侧边界 =
    /// atSeq 之后第一个 turn/end，因此锚在答案气泡的事件 seq 上恰好保留整轮。</summary>
    private async Task ForkSessionAtAsync(string sessionId, int atSeq)
    {
        if (_rpc is null || atSeq <= 0)
        {
            return;
        }
        try
        {
            // 参数键 request（0.7.0 探针实测口径，同 ForkSessionAsync）
            var result = await _rpc.CallOkAsync("session/fork", new { request = new { sessionId, atSeq } });
            var childId = result.TryGetProperty("sessionId", out var cs) && cs.ValueKind == JsonValueKind.String ? cs.GetString() : null;
            await RefreshSessionsAsync();
            await RefreshWorkspacesAsync();
            if (childId is { Length: > 0 } && childId != sessionId)
            {
                await RenameForkedChildAsync(sessionId, childId);
                var child = _sessions.FirstOrDefault(s => s.SessionId == childId);
                if (child is not null)
                {
                    Volatile.Write(ref _activeSessionId, childId);
                    RebuildNavMenu();
                    await OpenSessionAsync(child);
                    RestoreActiveSessionSelection();
                }
            }
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("分叉失败：{0}", ex.Message));
        }
    }

    /// <summary>官方 increasedForkTitle：给子会话标题追加序号（ASCII/全角括号各自递增，
    /// 无序号则补 " (1)"），经 session/rename 回写。失败不打断分支流程。</summary>
    private async Task RenameForkedChildAsync(string sourceId, string childId)
    {
        try
        {
            var source = _sessions.FirstOrDefault(s => s.SessionId == sourceId);
            var child = _sessions.FirstOrDefault(s => s.SessionId == childId);
            if (source is null || child is null || source.Title.Length == 0)
            {
                return;
            }
            var title = source.Title;
            var ascii = System.Text.RegularExpressions.Regex.Match(title, "^(.*?)\\((\\d+)\\)$");
            var fullWidth = System.Text.RegularExpressions.Regex.Match(title, "^(.*?)（(\\d+)）$");
            var increased = fullWidth.Success
                ? $"{fullWidth.Groups[1].Value}（{long.Parse(fullWidth.Groups[2].Value) + 1}）"
                : ascii.Success
                    ? $"{ascii.Groups[1].Value}({long.Parse(ascii.Groups[2].Value) + 1})"
                    : $"{title} (1)";
            await _rpc!.CallOkAsync("session/rename", new { request = new { sessionId = childId, title = increased } });
            child.Title = increased;
            RebuildNavMenu();
        }
        catch (Exception)
        {
            // 子会话改名失败不影响分支本身（标题保持内核继承值）
        }
    }

    // ---------------- 本轮文件改动（对标 dsh-client-ui-deliverables 的 ProducedFiles） ----------------

    /// <summary>官方 mutationPath 的壳侧移植：从一等突变调用（write / edit /
    /// str_replace_editor）参数中提取目标路径；其余工具或参数不完整 → null。
    /// 只有「执行成功」的调用计入产出（见 tool/result 侧的 isError 过滤）。</summary>
    private static string? MutationPath(string toolName, string argsRaw)
    {
        if (string.IsNullOrEmpty(argsRaw))
        {
            return null;
        }
        JsonElement args;
        try
        {
            using var doc = JsonDocument.Parse(argsRaw);
            args = doc.RootElement.Clone();
        }
        catch (JsonException)
        {
            return null;
        }
        if (args.ValueKind != JsonValueKind.Object)
        {
            return null;
        }
        switch (toolName)
        {
            case "write":
                return args.TryGetProperty("content", out var wc) && wc.ValueKind == JsonValueKind.String
                    ? PathValue(args, "file_path")
                    : null;
            case "edit":
                return ValidEditArgs(args) ? PathValue(args, "file_path") : null;
            case "str_replace_editor":
                return EditorMutationPath(args);
            default:
                return null;
        }
    }

    /// <summary>官方 validEditArgs：old_string 非空、new_string 为字符串、两者不同、
    /// replace_all 可缺省但必须是布尔。</summary>
    private static bool ValidEditArgs(JsonElement args)
    {
        if (!args.TryGetProperty("old_string", out var oldStr) || oldStr.ValueKind != JsonValueKind.String ||
            oldStr.GetString() is not { Length: > 0 })
        {
            return false;
        }
        if (!args.TryGetProperty("new_string", out var newStr) || newStr.ValueKind != JsonValueKind.String)
        {
            return false;
        }
        if (oldStr.GetString() == newStr.GetString())
        {
            return false;
        }
        if (args.TryGetProperty("replace_all", out var ra) && ra.ValueKind is not (JsonValueKind.True or JsonValueKind.False))
        {
            return false;
        }
        return true;
    }

    /// <summary>官方 editorMutationPath：仅完整突变命令（create/str_replace/insert）产出路径。</summary>
    private static string? EditorMutationPath(JsonElement args)
    {
        var path = PathValue(args, "path");
        if (path is null)
        {
            return null;
        }
        switch (Str(args, "command"))
        {
            case "create":
                return args.TryGetProperty("file_text", out var ft) && ft.ValueKind == JsonValueKind.String ? path : null;
            case "str_replace":
                return args.TryGetProperty("old_str", out var os) && os.ValueKind == JsonValueKind.String &&
                    os.GetString() is { Length: > 0 } &&
                    (!args.TryGetProperty("new_str", out var ns) || ns.ValueKind == JsonValueKind.String)
                        ? path
                        : null;
            case "insert":
                return args.TryGetProperty("insert_line", out var il) && il.ValueKind == JsonValueKind.Number &&
                    il.TryGetInt32(out var line) && line >= 0 &&
                    args.TryGetProperty("new_str", out var ins) && ins.ValueKind == JsonValueKind.String
                        ? path
                        : null;
            default:
                return null;
        }
    }

    /// <summary>官方 pathValue：非空白路径按工具收到的原样拼写保留。</summary>
    private static string? PathValue(JsonElement args, string key)
        => args.TryGetProperty(key, out var v) && v.ValueKind == JsonValueKind.String &&
            v.GetString() is { } s && s.Trim().Length > 0 ? s : null;

    /// <summary>官方 producedForClosing：本轮产出按首现顺序去重，并排除收尾答案之后
    /// 才结算的工具结果（seq > 收尾 seq）。closingSeq ≤ 0 视为不设上限。</summary>
    private List<string> ProducedPathsForTurn(int turn, int closingSeq)
    {
        var paths = new List<string>();
        if (turn <= 0 || !_producedByTurn.TryGetValue(turn, out var produced))
        {
            return paths;
        }
        var seen = new HashSet<string>(StringComparer.Ordinal);
        foreach (var (seq, path) in produced)
        {
            if (closingSeq > 0 && seq > closingSeq || !seen.Add(path))
            {
                continue;
            }
            paths.Add(path);
        }
        return paths;
    }

    /// <summary>轮尾「本轮文件改动」行：左侧标签 + 右侧文件 chip（首现顺序，全部展开——
    /// 官方的 +N 溢出是容器宽度响应式裁剪，桌面壳整行换行等价呈现）。
    /// 装配成功时把区与 chips 容器写入 row（turn/end 后按最新产出重建 chips）。</summary>
    private void BuildProducedFilesRow(ChatBubble bubble, FeedbackRow row, StackPanel content)
    {
        var paths = ProducedPathsForTurn(bubble.Turn, bubble.Seq);
        if (paths.Count == 0)
        {
            return;
        }
        var chips = new SimpleWrapPanel { VerticalAlignment = VerticalAlignment.Center };
        foreach (var p in paths)
        {
            chips.Children.Add(MakeProducedFileChip(p));
        }
        var label = new TextBlock
        {
            Text = L("本轮文件改动"),
            Style = Application.Current.Resources.TryGetValue("HintTextStyle", out var hint) && hint is Style hs ? hs : null,
            Opacity = 0.7,
            VerticalAlignment = VerticalAlignment.Center,
        };
        // 标签右侧挂「撤回本轮修改」：与 chips 分列，chips 重建（RefreshProducedChips）不会波及它
        var head = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = TokenDouble("Space6", 6),
            VerticalAlignment = VerticalAlignment.Center,
        };
        head.Children.Add(label);
        row.RevertButton = MakeRevertTurnButton(bubble.Turn);
        if (row.RevertButton is not null)
        {
            head.Children.Add(row.RevertButton);
        }
        var grid = new Grid { ColumnSpacing = TokenDouble("Space8", 8), Margin = new Thickness(6, 2, 6, 2) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.Children.Add(head);
        grid.Children.Add(chips);
        Grid.SetColumn(head, 0);
        Grid.SetColumn(chips, 1);
        grid.Tag = "produced-row";
        Aut(grid, "ProducedFilesRow", L("本轮文件改动"));
        row.ProducedSection = grid;
        row.ProducedChips = chips;
        // 先于动作行插入（官方 turnTail 槽渲染在 MessageIconActions 之前）
        content.Children.Add(grid);
    }

    /// <summary>turn/end 后按最新产出重建 chips（有产出才显示整区，无则整体收起）。</summary>
    private void RefreshProducedChips(FeedbackRow row)
    {
        if (row.ProducedSection is null || row.ProducedChips is null)
        {
            return;
        }
        var paths = ProducedPathsForTurn(row.Bubble.Turn, row.Bubble.Seq);
        row.ProducedChips.Children.Clear();
        foreach (var p in paths)
        {
            row.ProducedChips.Children.Add(MakeProducedFileChip(p));
        }
        row.ProducedSection.Visibility = paths.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        // 撤回按钮的状态随「已撤回」翻（撤成功后行还在屏上时立即转态）
        if (row.RevertButton is not null)
        {
            var reverted = Volatile.Read(ref _activeSessionId) is { } sid && IsTurnReverted(sid, row.Bubble.Turn);
            row.RevertButton.IsEnabled = !reverted;
            if (row.RevertButton.Content is FontIcon icon)
            {
                icon.Glyph = reverted ? "\uE73E" : "\uE7A7";
            }
            ToolTipService.SetToolTip(row.RevertButton, L(reverted
                ? "已撤回本轮修改"
                : "撤回本轮修改（把文件恢复到修改前）"));
        }
    }

    /// <summary>本轮有没有可撤回的修改：突变流水里有可反演的记录、且还没撤过。</summary>
    private bool CanRevertTurn(int turn)
    {
        if (turn <= 0 || !_turnMutations.TryGetValue(turn, out var list) || list.Count == 0)
        {
            return false;
        }
        if (Volatile.Read(ref _activeSessionId) is not { } sid || IsTurnReverted(sid, turn))
        {
            return false;
        }
        return list.Any(m => m.Revertible && !m.Done);
    }

    /// <summary>轮尾「撤回本轮修改」按钮（图标态：Undo 箭头，撤过后转禁用 + 对勾「已撤回」态，
    /// 与 RefreshProducedChips 的口径一致，行重建后外观不变）。有突变流水才装配。</summary>
    private Button? MakeRevertTurnButton(int turn)
    {
        if (turn <= 0 || !_turnMutations.TryGetValue(turn, out var list) || list.Count == 0)
        {
            return null;
        }
        var reverted = Volatile.Read(ref _activeSessionId) is { } sid && IsTurnReverted(sid, turn);
        var tip = L(reverted ? "已撤回本轮修改" : "撤回本轮修改（把文件恢复到修改前）");
        var icon = new FontIcon { Glyph = reverted ? "\uE73E" : "\uE7A7", FontSize = GlyphBody };
        var button = new Button
        {
            Width = SmallButtonSize,
            Height = SmallButtonSize,
            MinWidth = 0,
            Padding = new Thickness(0),
            CornerRadius = RadSmall,
            VerticalAlignment = VerticalAlignment.Center,
            Content = icon,
            Opacity = 0.8,
            IsEnabled = !reverted,
        };
        ToolTipService.SetToolTip(button, tip);
        Aut(button, "RevertTurnChangesButton", tip);
        button.Click += (_, _) => _ = RevertTurnMutationsAsync(turn);
        return button;
    }

    /// <summary>撤回本轮修改：确认后逆序反演本轮每一次文件突变（后改的先撤，前面的改动
    /// 才回得到原位）。逐文件报告成败——内核 meta 不全（大文件/二进制/editor 工具）
    /// 的撤不动，明说「无法恢复」，不假装成功。</summary>
    private async Task RevertTurnMutationsAsync(int turn)
    {
        if (turn <= 0 || !_turnMutations.TryGetValue(turn, out var list) || list.Count == 0 ||
            Volatile.Read(ref _activeSessionId) is not { } sid)
        {
            return;
        }
        var edited = new List<string>();
        var created = new List<string>();
        var skipped = new List<string>();
        foreach (var m in list)
        {
            if (m.Created)
            {
                if (!created.Contains(m.Path)) created.Add(m.Path);
            }
            else if (!edited.Contains(m.Path))
            {
                edited.Add(m.Path);
            }
            if (!m.Revertible && !skipped.Contains(m.Path)) skipped.Add(m.Path);
        }
        var body = new StackPanel { Spacing = TokenDouble("Space8", 8), MaxWidth = TokenDouble("DialogMinWidthCompact", 360) };
        body.Children.Add(new TextBlock
        {
            Text = LF("将把本轮（第 {0} 轮）改过的文件恢复到修改前的状态：", turn),
            TextWrapping = TextWrapping.Wrap,
        });
        if (edited.Count > 0) body.Children.Add(MutationFileList(L("修改过的文件（恢复原内容）"), edited));
        if (created.Count > 0) body.Children.Add(MutationFileList(L("新建的文件（将被删除）"), created));
        if (skipped.Count > 0) body.Children.Add(MutationFileList(L("没有回退依据、撤回时会被跳过的文件"), skipped));
        body.Children.Add(new TextBlock
        {
            Text = L("恢复后不可撤销。若这些文件在本轮之后又被改动过，撤回可能不完整。"),
            TextWrapping = TextWrapping.Wrap,
            Opacity = 0.75,
        });
        var dialog = new ContentDialog
        {
            Title = L("撤回本轮修改"),
            Content = body,
            PrimaryButtonText = L("撤回修改"),
            CloseButtonText = L("取消"),
            // 破坏性操作默认落在「取消」上：误触回车不该直接改文件
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = Content.XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }
        var ok = 0;
        var failed = new List<string>();
        for (var i = list.Count - 1; i >= 0; i--)
        {
            var m = list[i];
            if (m.Done || !m.Revertible)
            {
                continue;
            }
            try
            {
                RevertOneMutation(m);
                m.Done = true;
                ok++;
            }
            catch (Exception ex)
            {
                failed.Add($"{BaseName(m.Path)}：{ex.Message}");
            }
        }
        if (failed.Count == 0)
        {
            RevertedTurns(sid).Add(turn);
            ShellToast.Show(L("撤回本轮修改"), LF("已恢复 {0} 个文件。", ok));
        }
        else
        {
            ShellToast.Show(L("撤回未完成"), LF("成功 {0} 个，失败 {1} 个：{2}", ok, failed.Count, string.Join("；", failed)));
        }
        // 行还在屏上就把按钮转态（未 realize 的行将来装配时按最新状态出按钮）
        if (_transcriptAnswers.TryGetValue(turn, out var answer))
        {
            RefreshTurnActionsRow(answer);
        }
    }

    /// <summary>反演一次文件突变。hunk 反演按「找 New 换 Old」整块替换——上下文行两侧相同，
    /// 天然对齐；纯新增 hunk 的 Old 为空即删掉该块，纯删除 hunk 的 New 只剩上下文，换回 Old
    /// 即把删掉的行插回。任何一步对不上就抛错：宁可报告失败，也不能把文件改到第三种状态。</summary>
    private void RevertOneMutation(TurnFileMutation m)
    {
        if (m.Path.Length == 0)
        {
            throw new InvalidOperationException(L("没有可用的回退依据"));
        }
        if (m.Created)
        {
            // 新建文件：撤回 = 删掉（确认对话框里已逐条列明，见 RevertTurnMutationsAsync）
            if (File.Exists(m.Path))
            {
                File.Delete(m.Path);
            }
            return;
        }
        if (!File.Exists(m.Path))
        {
            throw new FileNotFoundException(L("文件已不存在"), m.Path);
        }
        var bytes = File.ReadAllBytes(m.Path);
        var bom = bytes.Length >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF;
        var raw = System.Text.Encoding.UTF8.GetString(bytes, bom ? 3 : 0, bytes.Length - (bom ? 3 : 0));
        var eol = DetectEol(raw);
        var lines = NormalizeEol(raw).Split('\n');
        if (m.Hunks.Count > 0)
        {
            for (var i = m.Hunks.Count - 1; i >= 0; i--)
            {
                var hunk = m.Hunks[i];
                if (hunk.New.Length == 0)
                {
                    // 纯删除 hunk（内核 newText = ""，只有把文件写空才会有这种形状：无上下文可定位）。
                    // 文件现在必须确实是空的才敢整份换回 Old，否则说明中间又被改过。
                    if (hunk.Old.Length == 0 || lines.Any(l => l.Length > 0))
                    {
                        throw new InvalidOperationException(L("找不到改动后的内容，文件可能已被其他改动覆盖"));
                    }
                    lines = hunk.Old;
                    continue;
                }
                var at = FindLineBlock(lines, hunk.New);
                if (at < 0)
                {
                    throw new InvalidOperationException(hunk.New.Length == 0
                        ? L("没有可用的回退依据")
                        : L("找不到改动后的内容，文件可能已被其他改动覆盖"));
                }
                var merged = new List<string>(lines.Length - hunk.New.Length + hunk.Old.Length);
                merged.AddRange(lines.Take(at));
                merged.AddRange(hunk.Old);
                merged.AddRange(lines.Skip(at + hunk.New.Length));
                lines = merged.ToArray();
            }
            WriteTextAtomic(m.Path, string.Join("\n", lines), eol, bom);
            return;
        }
        if (m.EditorCommand == "insert" && m.EditorNew is { Length: > 0 } inserted)
        {
            var block = NormalizeEol(inserted).Split('\n');
            var at = FindLineBlock(lines, block);
            if (at < 0)
            {
                throw new InvalidOperationException(L("找不到改动后的内容，文件可能已被其他改动覆盖"));
            }
            var merged = new List<string>(lines.Length - block.Length);
            merged.AddRange(lines.Take(at));
            merged.AddRange(lines.Skip(at + block.Length));
            WriteTextAtomic(m.Path, string.Join("\n", merged), eol, bom);
            return;
        }
        if (m.EditorCommand == "str_replace" && m.EditorNew is { Length: > 0 } current)
        {
            var text = NormalizeEol(raw);
            var needle = NormalizeEol(current);
            var occurrences = CountOccurrences(text, needle);
            if (occurrences == 0)
            {
                throw new InvalidOperationException(L("找不到改动后的内容，文件可能已被其他改动覆盖"));
            }
            if (occurrences > 1)
            {
                throw new InvalidOperationException(L("改动位置不唯一，为避免改错已放弃"));
            }
            var restored = text.Replace(needle, NormalizeEol(m.EditorOld ?? ""), StringComparison.Ordinal);
            WriteTextAtomic(m.Path, restored, eol, bom);
            return;
        }
        throw new InvalidOperationException(L("没有可用的回退依据"));
    }

    /// <summary>行块匹配：整块按行对齐才算命中；多处命中返回 -1（定位不唯一，不敢动手）。
    /// 空块无法定位（无上下文的纯删除 hunk），同样返回 -1 交由调用方报错。</summary>
    private static int FindLineBlock(string[] lines, string[] block)
    {
        if (block.Length == 0)
        {
            return -1;
        }
        var found = -1;
        for (var i = 0; i + block.Length <= lines.Length; i++)
        {
            var match = true;
            for (var j = 0; j < block.Length; j++)
            {
                if (!string.Equals(lines[i + j], block[j], StringComparison.Ordinal))
                {
                    match = false;
                    break;
                }
            }
            if (!match)
            {
                continue;
            }
            if (found >= 0)
            {
                return -1;
            }
            found = i;
        }
        return found;
    }

    private static int CountOccurrences(string text, string needle)
    {
        if (needle.Length == 0)
        {
            return 0;
        }
        var count = 0;
        var index = 0;
        while (true)
        {
            var found = text.IndexOf(needle, index, StringComparison.Ordinal);
            if (found < 0)
            {
                return count;
            }
            count++;
            index = found + needle.Length;
        }
    }

    /// <summary>行尾风格（内核 detectLineEndings 同口径：前 4KB 里 CRLF 过半即 CRLF）。</summary>
    private static string DetectEol(string raw)
    {
        var sample = raw.Length > 4096 ? raw[..4096] : raw;
        var crlf = sample.Split("\r\n").Length - 1;
        var lf = sample.Split("\n").Length - 1 - crlf;
        return crlf > lf ? "\r\n" : "\n";
    }

    private static string NormalizeEol(string text) => text.Replace("\r\n", "\n", StringComparison.Ordinal);

    /// <summary>原子写回（先写临时文件再替换）：撤回是破坏性操作，不能写一半留在盘上。
    /// 行尾按原文件风格还原、BOM 原样保留（与内核 restoreLineEndings 同口径）。</summary>
    private static void WriteTextAtomic(string path, string content, string eol, bool bom)
    {
        var text = eol == "\n" ? content : NormalizeEol(content).Replace("\n", "\r\n", StringComparison.Ordinal);
        var payload = System.Text.Encoding.UTF8.GetBytes(text);
        var tmp = path + ".dsh-revert.tmp";
        if (bom)
        {
            var withBom = new byte[payload.Length + 3];
            withBom[0] = 0xEF;
            withBom[1] = 0xBB;
            withBom[2] = 0xBF;
            Array.Copy(payload, 0, withBom, 3, payload.Length);
            File.WriteAllBytes(tmp, withBom);
        }
        else
        {
            File.WriteAllBytes(tmp, payload);
        }
        File.Move(tmp, path, overwrite: true);
    }

    private static string BaseName(string path) => path.Replace('\\', '/').Split('/')[^1];

    /// <summary>确认对话框里的文件清单（标题 + 每条一行，长路径整行换行）。</summary>
    private static StackPanel MutationFileList(string title, List<string> paths)
    {
        var panel = new StackPanel { Spacing = 2 };
        panel.Children.Add(new TextBlock { Text = title, Opacity = 0.85 });
        foreach (var p in paths)
        {
            panel.Children.Add(new TextBlock
            {
                Text = "· " + p,
                TextWrapping = TextWrapping.Wrap,
                Opacity = 0.7,
                Margin = new Thickness(8, 0, 0, 0),
            });
        }
        return panel;
    }
    /// <summary>文件 chip：文件图标 + 基名（官方 fileIcon/fileName），点击用内核
    /// session/openWorkspacePath 交宿主桌面打开（官方 produced.open 同一宿主能力）。</summary>
    private HyperlinkButton MakeProducedFileChip(string path)
    {
        var name = path.Replace('\\', '/').Split('/')[^1];
        var chip = new HyperlinkButton
        {
            Padding = new Thickness(0),
            Margin = new Thickness(0),
            Content = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = TokenDouble("Space6", 6),
                Children =
                {
                    new FontIcon { Glyph = "\uE8A5", FontSize = 12 },
                    new TextBlock { Text = name },
                },
            },
        };
        var tip = LF("打开 {0}", path);
        ToolTipService.SetToolTip(chip, tip);
        Aut(chip, "ProducedFileChip_" + name, tip);
        chip.Click += (_, _) => _ = OpenProducedPathAsync(path);
        return chip;
    }

    /// <summary>经内核把工作区路径交给宿主桌面（默认程序打开；官方 SessionOpenWorkspacePath
    /// 契约：path + 可选 action:'reveal'，缺省即打开）。</summary>
    private async Task OpenProducedPathAsync(string path)
    {
        if (_rpc is null || path.Length == 0)
        {
            return;
        }
        try
        {
            await _rpc.CallOkAsync("session/openWorkspacePath", new { request = new { path } });
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("打开失败：{0}", ex.Message));
        }
    }
}

/// <summary>极简水平流式换行面板（WinUI 3 无内置 WrapPanel；不为此引入 Community
/// Toolkit 依赖）。只服务 ProducedFiles chips 的换行摆放。</summary>
public sealed class SimpleWrapPanel : Panel
{
    protected override Size MeasureOverride(Size availableSize)
    {
        var maxWidth = double.IsInfinity(availableSize.Width) ? double.MaxValue : availableSize.Width;
        double rowWidth = 0, rowHeight = 0, totalHeight = 0;
        foreach (var child in Children)
        {
            child.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
            var w = child.DesiredSize.Width;
            if (rowWidth > 0 && rowWidth + w > maxWidth)
            {
                totalHeight += rowHeight;
                rowWidth = 0;
                rowHeight = 0;
            }
            rowWidth += w;
            rowHeight = Math.Max(rowHeight, child.DesiredSize.Height);
        }
        return new Size(Math.Min(rowWidth, maxWidth), totalHeight + rowHeight);
    }

    protected override Size ArrangeOverride(Size finalSize)
    {
        double x = 0, y = 0, rowHeight = 0;
        foreach (var child in Children)
        {
            var w = child.DesiredSize.Width;
            if (x > 0 && x + w > finalSize.Width)
            {
                x = 0;
                y += rowHeight;
                rowHeight = 0;
            }
            child.Arrange(new Rect(x, y, w, child.DesiredSize.Height));
            x += w;
            rowHeight = Math.Max(rowHeight, child.DesiredSize.Height);
        }
        return finalSize;
    }
}
