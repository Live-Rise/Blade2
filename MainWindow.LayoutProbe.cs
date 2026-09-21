#if DEBUG
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Threading.Tasks;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Blade2;

public sealed partial class MainWindow
{
    private async Task RunLayoutProbeAsync(string output)
    {
        var rows = new List<object>();
        try
        {
            KernelBootPanel.Visibility = Visibility.Collapsed;
            ShowChatPage();
            ChatHero.Visibility = Visibility.Collapsed;
            _compactTranscript = false;
            AppendBubble(new ChatBubble { Role = "user", Text = string.Concat(Enumerable.Repeat("Synthetic user message for resizing. ", 30)) });
            AppendBubble(new ChatBubble { Role = "assistant", Text = string.Concat(Enumerable.Repeat("合成测试文本用于验证窄窗口换行。Adaptive paragraph with long words abcdefghijklmnopqrstuvwxyz. ", 25)) + "\n```text\n" + new string('X', 300) + "\n```" });
            await Task.Delay(1000);
            foreach (var rail in new[] { false })
            {
                SetFilesPanelOpen(rail);
                foreach (var width in new[] { 700, 450 })
                {
                    var scale = RootGrid.XamlRoot.RasterizationScale;
                    AppWindow.Resize(new Windows.Graphics.SizeInt32((int)Math.Round(width * scale), (int)Math.Round(800 * scale)));
                    await Task.Delay(650);
                    await Task.Delay(150);
                    var viewport = ChatViewport.ActualWidth;
                    var cap = Math.Max(0, viewport - ChatColumn.Margin.Left - ChatColumn.Margin.Right);
                    var rich = Descendants(ChatList).OfType<RichTextBlock>().ToArray();
                    rows.Add(new { requestedWidthDip = width, actualWindowWidthDip = AppWindow.Size.Width / scale, rootWidthDip = RootGrid.ActualWidth, scale, rail, viewport, column = ChatColumn.ActualWidth, list = ChatList.ActualWidth, cap, richCount = rich.Length, richWidths = rich.Select(x => x.ActualWidth).ToArray(), richHeights = rich.Select(x => x.ActualHeight).ToArray(), pass = ChatColumn.ActualWidth <= cap + 1 && ChatList.ActualWidth <= cap + 1 && rich.Length > 0 && rich.All(x => x.ActualWidth <= ChatList.ActualWidth + 1) });
                }
            }
            await File.WriteAllTextAsync(output, JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
        }
        catch (Exception ex)
        {
            await File.WriteAllTextAsync(output, ex.ToString());
        }
        finally { Close(); }
    }

    private static IEnumerable<DependencyObject> Descendants(DependencyObject root)
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(root); i++)
        {
            var child = VisualTreeHelper.GetChild(root, i);
            yield return child;
            foreach (var nested in Descendants(child)) yield return nested;
        }
    }
}
#endif
