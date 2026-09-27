// ---------------- Todo 清单面板（P1-17，对标官方 dsh-tool-todo + conversation TodoPanel） ----------------
// 数据源 = session/control / session/list 的 todos 投影（MainWindow.SessionState.cs 已消费）。
// 入口：能力菜单「任务」+ ComposerAddFlyout「任务」+ 会话头角标（有清单才亮）。
// 官方 TodoPanel：标题「任务」+ 进度「N 已完成 · N 进行中 · N 待处理」+ 三态图标行。

using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;

namespace Blade2;

public sealed partial class MainWindow
{
    /// <summary>Todo 清单对话框（只读展示 todos 投影；写路径是模型的 todo_write 工具）。</summary>
    private async Task ShowTodosPanelAsync()
    {
        var sessionId = CapabilitySession();
        var todos = SnapshotTodos();
        var body = new StackPanel { Spacing = 10, Width = 440 };
        body.Children.Add(new TextBlock
        {
            Text = CapabilityText(
                "任务清单来自会话 todos 投影（模型经 todo_write 更新）。此面板只读，不创建、编辑或删除任务。",
                "The to-do list comes from the session todos projection (the model updates it via todo_write). This panel is read-only."),
            TextWrapping = TextWrapping.Wrap,
        });

        if (todos is null || todos.Count == 0)
        {
            body.Children.Add(new TextBlock
            {
                Text = CapabilityText("当前会话没有任务清单。", "No to-do list in this session."),
                TextWrapping = TextWrapping.Wrap,
                IsTextSelectionEnabled = true,
            });
        }
        else
        {
            var done = todos.Count(t => t.Status == "completed");
            var active = todos.Count(t => t.Status == "in_progress");
            var pending = todos.Count - done - active;
            body.Children.Add(new TextBlock
            {
                Text = CapabilityText("任务", "To-dos") + " · " + TodoProgressLabel(done, active, pending),
                Style = AppStyle("BodyStrongTextStyle"),
                TextWrapping = TextWrapping.Wrap,
            });
            foreach (var item in todos)
            {
                body.Children.Add(MakeTodoRow(item));
            }
        }

        await CapabilityDialog(
            CapabilityText("任务", "To-dos"),
            new ScrollViewer { Content = body, MaxHeight = 480 }).ShowAsync();
        CheckCapabilitySession(sessionId);
    }

    /// <summary>官方 StatusGlyph 三态：completed=对勾环、in_progress=进行环、pending=虚线环。</summary>
    private FrameworkElement MakeTodoRow(TodoItemVm item)
    {
        var row = new Grid { ColumnSpacing = Sp8, Padding = new Thickness(0, 3, 0, 3) };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });

        var glyph = MakeTodoGlyph(item.Status);
        Grid.SetColumn(glyph, 0);
        row.Children.Add(glyph);

        var content = new TextBlock
        {
            Text = item.Content,
            TextWrapping = TextWrapping.Wrap,
            IsTextSelectionEnabled = true,
            Opacity = item.Status == "completed" ? 0.55 : 1.0,
            VerticalAlignment = VerticalAlignment.Center,
        };
        Grid.SetColumn(content, 1);
        row.Children.Add(content);

        var statusText = item.Status switch
        {
            "completed" => L("已完成"),
            "in_progress" => L("进行中"),
            _ => L("待处理"),
        };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(row, statusText + " · " + item.Content);
        return row;
    }

    private FrameworkElement MakeTodoGlyph(string status)
    {
        const double size = 16;
        var brush = status switch
        {
            "completed" => ThemeBrush("SuccessBrush"),
            "in_progress" => ThemeBrush("InfoBrush"),
            _ => ThemeBrush("TextTertiaryBrush"),
        };
        var cell = new Grid
        {
            Width = size,
            Height = size,
            VerticalAlignment = VerticalAlignment.Center,
        };
        if (status == "completed")
        {
            // 对勾环：外圆 + 对勾（用 Path 近似官方 SVG）
            cell.Children.Add(new Ellipse
            {
                Width = 14,
                Height = 14,
                Stroke = brush,
                StrokeThickness = 1.2,
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            });
            cell.Children.Add(new FontIcon
            {
                Glyph = "\uE73E",
                FontSize = 10,
                Foreground = brush,
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            });
        }
        else if (status == "in_progress")
        {
            cell.Children.Add(new Ellipse
            {
                Width = 14,
                Height = 14,
                Stroke = brush,
                StrokeThickness = 1.4,
                Opacity = 0.85,
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            });
        }
        else
        {
            // pending：虚线环（DashArray 近似官方 2.4 2.4）
            var ring = new Ellipse
            {
                Width = 14,
                Height = 14,
                Stroke = brush,
                StrokeThickness = 1.2,
                StrokeDashArray = new Microsoft.UI.Xaml.Media.DoubleCollection { 2.4, 2.4 },
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            };
            cell.Children.Add(ring);
        }
        return cell;
    }
}
