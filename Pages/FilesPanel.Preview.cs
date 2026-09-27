// 六形态文档预览 + 预览辅助（P0-1/2 + I-3 + T37 路 A）。
// RPC：workspaceFiles/readBytes（二进制分段，图/PDF 主路径）。readAll/readRelated 自内核
// 0.1.7 起移除——「复制全文取全文」与「HTML 附属字节内联」两条路径随之撤除（对齐内核）。
// 文案与交互对齐 @deepseek-ai/dsh-client-ui-sidebar-documentpreview/lib/client.js
// （加载更多 / 文件已更新 / 重新载入 / 重新读取文件 / 自动换行 / 复制）。
// HTML T37 路 A：WebView2 DOM 渲染（默认），仅渲染受信工作区本地内容——
//   IsScriptEnabled=false、AreDefaultScriptDialogsEnabled=false、IsWebMessageEnabled=false、
//   禁右键/DevTools/缩放/内置错误页；NavigationStarting/NewWindowRequested 只放行 about:/data:。
//   附属 CSS/图片不再内联（readRelated 已移除）：DOM 保留结构与文案，样式/图片缺失是如实结果。
//   官方 HtmlBody 是 sandbox="allow-scripts" 的 opaque iframe（会跑页面脚本）；壳刻意关脚本，
//   静态 DOM 布局/CSS 接近官方视觉，JS 驱动交互不等价 → FINAL 口径仍 PARTIAL。
//   WebView2 Runtime 缺失时自动回落「源码+大纲+附属」（路 B），可手动切换。
// PDF 加密：Windows.Data.Pdf 无解锁 API（LoadFromStreamAsync 无 password 参数），降级 UX = 明确「加密 PDF」+ 一键系统打开 + 复制路径。

using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using System.Threading.Tasks;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage.Streams;

namespace Blade2;

public sealed partial class FilesPanel
{
    private enum PreviewFace
    {
        PlainText,
        Code,
        Markdown,
        Image,
        Pdf,
        Html,
    }

    private PreviewFace _previewFace = PreviewFace.PlainText;
    private string _previewBuffer = "";
    private int _previewNextLine = 1;
    private bool _previewEof;
    private bool _previewWrap = true;
    private byte[]? _previewBytes;
    private Windows.Data.Pdf.PdfDocument? _pdfDoc;
    private int _pdfPage;
    private string? _previewAbsolutePath;
    /// <summary>true = HTML DOM（WebView2）；false = 源码+大纲（路 B）。</summary>
    private bool _htmlDomPreferred = true;
    private bool _htmlWebViewReady;
    private bool _htmlWebViewFailed;

    private const int ByteChunkLength = 256 * 1024;
    private const int MaxHtmlRelated = 12;

    // ---------------- 形态判定 ----------------

    private static PreviewFace FaceOf(string path)
    {
        var ext = System.IO.Path.GetExtension(path).ToLowerInvariant();
        return ext switch
        {
            ".md" or ".markdown" => PreviewFace.Markdown,
            ".png" or ".jpg" or ".jpeg" or ".gif" or ".webp" or ".bmp" or ".ico" or ".svg" => PreviewFace.Image,
            ".pdf" => PreviewFace.Pdf,
            ".html" or ".htm" => PreviewFace.Html,
            ".cs" or ".js" or ".mjs" or ".cjs" or ".ts" or ".tsx" or ".jsx" or ".py" or ".rs" or ".go"
                or ".java" or ".c" or ".h" or ".cpp" or ".hpp" or ".cs" or ".xaml" or ".json"
                or ".yml" or ".yaml" or ".toml" or ".xml" or ".css" or ".scss" or ".less" or ".sql"
                or ".sh" or ".ps1" or ".bat" or ".cmd" or ".php" or ".rb" or ".swift" or ".kt"
                or ".scala" or ".vue" or ".svelte" or ".dart" or ".r" or ".gradle" or ".dockerfile"
                or ".ini" or ".cfg" or ".conf" or ".lock" or ".gitignore" or ".editorconfig" => PreviewFace.Code,
            _ => PreviewFace.PlainText,
        };
    }

    private bool IsTextFace(PreviewFace face) =>
        face is PreviewFace.PlainText or PreviewFace.Code or PreviewFace.Markdown or PreviewFace.Html;

    // ---------------- RPC 包装（I-3 三端点） ----------------

    /// <summary>workspaceFiles/readBytes：按字节段读取，拼到 eof（图/PDF）。</summary>
    private async Task<byte[]> ReadFileBytesAsync(string sid, string path, long previewGeneration)
    {
        var rpc = _rpc ?? throw new InvalidOperationException("rpc");
        using var ms = new MemoryStream();
        long offset = 0;
        for (; ; )
        {
            var value = await rpc.CallOkAsync("workspaceFiles/readBytes", new
            {
                workspaceFileScopeId = sid,
                path,
                range = new { offset, length = ByteChunkLength },
            });
            if (previewGeneration != _previewGeneration) throw new OperationCanceledException();
            _previewVersion = value.TryGetProperty("version", out var v) ? v.GetString() : _previewVersion;
            _previewAbsolutePath = value.TryGetProperty("absolutePath", out var ap) ? ap.GetString() : _previewAbsolutePath;
            var chunk = DecodeB64(value);
            ms.Write(chunk, 0, chunk.Length);
            offset += chunk.Length;
            var eof = value.TryGetProperty("eof", out var ef) && ef.ValueKind == JsonValueKind.True;
            if (eof || chunk.Length == 0) break;
        }
        return ms.ToArray();
    }

    private static byte[] DecodeB64(JsonElement value)
    {
        var data = value.TryGetProperty("data", out var d) ? d.GetString() ?? "" : "";
        return data.Length == 0 ? [] : Convert.FromBase64String(data);
    }

    // ---------------- 面切换 ----------------

    private void ApplyPreviewFace(PreviewFace face)
    {
        _previewFace = face;
        bool htmlDom = face == PreviewFace.Html && _htmlDomPreferred && !_htmlWebViewFailed;
        bool htmlSource = face == PreviewFace.Html && !htmlDom;
        TextScroll.Visibility = face is PreviewFace.PlainText or PreviewFace.Code || htmlSource
            ? Visibility.Visible : Visibility.Collapsed;
        MarkdownScroll.Visibility = face == PreviewFace.Markdown ? Visibility.Visible : Visibility.Collapsed;
        ImageScroll.Visibility = face == PreviewFace.Image ? Visibility.Visible : Visibility.Collapsed;
        PdfHost.Visibility = face == PreviewFace.Pdf ? Visibility.Visible : Visibility.Collapsed;
        HtmlWebView.Visibility = htmlDom ? Visibility.Visible : Visibility.Collapsed;
        HtmlModeToggle.Visibility = face == PreviewFace.Html && !_htmlWebViewFailed
            ? Visibility.Visible : Visibility.Collapsed;
        HtmlModeToggle.IsChecked = htmlSource; // 勾选 = 源码
        if (face != PreviewFace.Html)
        {
            HtmlOutlineSummary.Visibility = Visibility.Collapsed;
            HtmlOutlineSummary.Text = "";
            HtmlRelatedSummary.Visibility = Visibility.Collapsed;
            HtmlRelatedSummary.Text = "";
            HtmlRelatedList.Visibility = Visibility.Collapsed;
            HtmlRelatedList.Children.Clear();
        }
        // 换行只作用于文本行；Markdown 自排版、图/PDF/DOM 无行概念
        WrapToggle.Visibility = (IsTextFace(face) && face != PreviewFace.Markdown && face != PreviewFace.Html)
            || htmlSource
            ? Visibility.Visible : Visibility.Collapsed;
        CopyAllButton.Visibility = IsTextFace(face) ? Visibility.Visible : Visibility.Collapsed;
        CopySelectionButton.Visibility = face is PreviewFace.PlainText or PreviewFace.Code || htmlSource
            ? Visibility.Visible : Visibility.Collapsed;
        LoadMoreButton.Visibility = Visibility.Collapsed;
        ChangedBanner.Visibility = Visibility.Collapsed;
    }

    private void ResetPreviewBody()
    {
        _previewBuffer = "";
        _previewNextLine = 1;
        _previewEof = false;
        _previewBytes = null;
        _previewAbsolutePath = null;
        PreviewRich.Blocks.Clear();
        PreviewRich.TextWrapping = _previewWrap ? TextWrapping.Wrap : TextWrapping.NoWrap;
        MarkdownHost.Children.Clear();
        PreviewImage.Source = null;
        PdfPageImage.Source = null;
        PdfPageLabel.Text = "";
        _pdfPage = 0;
        // Windows.Data.Pdf.PdfDocument 不实现 IDisposable；交给 GC 与流关闭即可。
        _pdfDoc = null;
        try { HtmlWebView.CoreWebView2?.NavigateToString("<!doctype html><meta charset=\"utf-8\"><title></title>"); }
        catch (Exception) { }
    }

    // ---------------- 文本三形态加载 ----------------

    private async Task LoadTextPageAsync(string sid, string path, long previewGeneration, bool append)
    {
        var rpc = _rpc!;
        var read = await rpc.CallOkAsync("workspaceFiles/read", new
        {
            workspaceFileScopeId = sid,
            path,
            range = new { offset = _previewNextLine, limit = PreviewLineLimit },
        });
        if (previewGeneration != _previewGeneration) return;
        _previewVersion = read.TryGetProperty("version", out var rv) ? rv.GetString() : _previewVersion;
        _previewAbsolutePath = read.TryGetProperty("absolutePath", out var ap) ? ap.GetString() : _previewAbsolutePath;
        var text = read.TryGetProperty("text", out var txt) ? txt.GetString() ?? "" : "";
        var lines = read.TryGetProperty("lines", out var ln) && ln.ValueKind == JsonValueKind.Number ? ln.GetInt32() : 0;
        _previewEof = read.TryGetProperty("eof", out var ef) && ef.ValueKind == JsonValueKind.True;
        if (append && _previewBuffer.Length > 0 && text.Length > 0)
        {
            _previewBuffer += "\n" + text;
        }
        else if (!append)
        {
            _previewBuffer = text;
        }
        else
        {
            _previewBuffer += text;
        }
        var pageStartLine = _previewNextLine;
        if (lines > 0)
        {
            _previewNextLine += lines;
        }

        if (append && _previewFace is PreviewFace.Code or PreviewFace.Html or PreviewFace.PlainText)
        {
            RenderCurrentTextFace(text, clear: false, startLine: pageStartLine);
        }
        else
        {
            ResetRichText();
            MarkdownHost.Children.Clear();
            RenderCurrentTextFace(_previewBuffer, clear: true, startLine: 1);
        }

        PreviewMeta.Text = MainWindow.TLF("{0} · {1} 行 · 版本 {2}",
            FormatSize(_previewStatBytes), Math.Max(0, _previewNextLine - 1), Short(_previewVersion));
        LoadMoreButton.Visibility = _previewEof ? Visibility.Collapsed : Visibility.Visible;
    }

    private long? _previewStatBytes;

    private void RenderCurrentTextFace(string text, bool clear, int startLine)
    {
        switch (_previewFace)
        {
            case PreviewFace.Markdown:
                RenderMarkdown(text, MarkdownHost);
                break;
            case PreviewFace.Code:
            case PreviewFace.Html:
                RenderCode(text, PreviewRich, clear, startLine);
                break;
            default:
                RenderPlainText(text, PreviewRich, clear);
                break;
        }
    }

    private void ResetRichText()
    {
        PreviewRich.Blocks.Clear();
    }

    // ---------------- 纯文本 ----------------

    private void RenderPlainText(string text, RichTextBlock target, bool clear)
    {
        if (clear) target.Blocks.Clear();
        var paragraph = new Paragraph();
        paragraph.Inlines.Add(new Run { Text = text });
        target.Blocks.Add(paragraph);
    }

    // ---------------- 代码：等宽 + 行号 + 关键字/字符串/注释三档 ----------------

    private static readonly SolidColorBrush CodeKeywordBrush = new(Colors.SteelBlue);
    private static readonly SolidColorBrush CodeStringBrush = new(Colors.DarkOrange);
    private static readonly SolidColorBrush CodeCommentBrush = new(Colors.ForestGreen);
    private static readonly SolidColorBrush CodeLineNoBrush = new(Colors.Gray);

    private static readonly HashSet<string> CodeKeywords = new(StringComparer.Ordinal)
    {
        "abstract","as","async","await","base","bool","break","byte","case","catch","char","class",
        "const","continue","decimal","default","delegate","do","double","else","enum","event","explicit",
        "extern","false","finally","fixed","float","for","foreach","func","function","goto","if","impl",
        "implicit","in","int","interface","internal","is","lock","long","namespace","new","null","object",
        "operator","out","override","params","private","protected","public","readonly","ref","return",
        "sbyte","sealed","short","sizeof","stackalloc","static","string","struct","switch","this","throw",
        "true","try","typeof","uint","ulong","unchecked","unsafe","ushort","use","using","var","virtual",
        "void","volatile","while","with","yield","let","const","type","export","import","from","class",
        "def","elif","except","finally","global","lambda","nonlocal","pass","raise","None","True","False",
        "and","or","not","in","is","match","when","package","impl","fn","mut","pub","trait","where",
        "extension","guard","throws","rethrows","async","await","print",
    };

    /// <summary>整段代码重绘/追加：按行给行号，再做三档着色（关键字 / 字符串 / 注释）。</summary>
    private void RenderCode(string text, RichTextBlock target, bool clear, int startLine = 1)
    {
        if (clear)
        {
            target.Blocks.Clear();
            _codeInBlockComment = false;
        }
        var lines = text.Split('\n');
        for (var i = 0; i < lines.Length; i++)
        {
            var line = lines[i];
            var p = new Paragraph { Margin = new Thickness(0) };
            p.Inlines.Add(new Run
            {
                Text = $"{startLine + i,5}  ",
                Foreground = CodeLineNoBrush,
            });
            AppendHighlighted(p.Inlines, line);
            target.Blocks.Add(p);
        }
    }

    private bool _codeInBlockComment;

    private void AppendHighlighted(InlineCollection inlines, string line)
    {
        var i = 0;
        var n = line.Length;
        while (i < n)
        {
            if (_codeInBlockComment)
            {
                var end = line.IndexOf("*/", i, StringComparison.Ordinal);
                if (end < 0)
                {
                    inlines.Add(new Run { Text = line[i..], Foreground = CodeCommentBrush, FontStyle = Windows.UI.Text.FontStyle.Italic });
                    return;
                }
                inlines.Add(new Run { Text = line[i..(end + 2)], Foreground = CodeCommentBrush, FontStyle = Windows.UI.Text.FontStyle.Italic });
                i = end + 2;
                _codeInBlockComment = false;
                continue;
            }
            if (line.AsSpan(i).StartsWith("//") || (line[i] == '#' && IsHashCommentFile()))
            {
                inlines.Add(new Run { Text = line[i..], Foreground = CodeCommentBrush, FontStyle = Windows.UI.Text.FontStyle.Italic });
                return;
            }
            if (line.AsSpan(i).StartsWith("/*"))
            {
                _codeInBlockComment = true;
                var end = line.IndexOf("*/", i + 2, StringComparison.Ordinal);
                if (end < 0)
                {
                    inlines.Add(new Run { Text = line[i..], Foreground = CodeCommentBrush, FontStyle = Windows.UI.Text.FontStyle.Italic });
                    return;
                }
                inlines.Add(new Run { Text = line[i..(end + 2)], Foreground = CodeCommentBrush, FontStyle = Windows.UI.Text.FontStyle.Italic });
                i = end + 2;
                _codeInBlockComment = false;
                continue;
            }
            if (line[i] is '"' or '\'')
            {
                var quote = line[i];
                var j = i + 1;
                while (j < n)
                {
                    if (line[j] == '\\' && j + 1 < n) { j += 2; continue; }
                    if (line[j] == quote) { j++; break; }
                    j++;
                }
                inlines.Add(new Run { Text = line[i..j], Foreground = CodeStringBrush });
                i = j;
                continue;
            }
            if (char.IsLetter(line[i]) || line[i] == '_')
            {
                var j = i + 1;
                while (j < n && (char.IsLetterOrDigit(line[j]) || line[j] == '_')) j++;
                var word = line[i..j];
                inlines.Add(new Run
                {
                    Text = word,
                    Foreground = CodeKeywords.Contains(word) ? CodeKeywordBrush : null,
                });
                i = j;
                continue;
            }
            inlines.Add(new Run { Text = line[i].ToString() });
            i++;
        }
    }

    private bool IsHashCommentFile()
    {
        var ext = System.IO.Path.GetExtension(_previewPath ?? "").ToLowerInvariant();
        return ext is ".py" or ".sh" or ".yml" or ".yaml" or ".toml" or ".rb" or ".ps1" or ".r";
    }

    // ---------------- Markdown：标题/列表/代码块/链接/粗斜体/行内码 ----------------

    private void RenderMarkdown(string text, Panel target)
    {
        target.Children.Clear();
        var lines = text.Split('\n');
        var i = 0;
        while (i < lines.Length)
        {
            var line = lines[i];
            if (line.StartsWith("```", StringComparison.Ordinal))
            {
                var fenceLang = line[3..].Trim();
                var body = new StringBuilder();
                i++;
                while (i < lines.Length && !lines[i].StartsWith("```", StringComparison.Ordinal))
                {
                    body.Append(lines[i]).Append('\n');
                    i++;
                }
                if (i < lines.Length) i++; // 收尾 ```
                target.Children.Add(BuildCodeBlock(body.ToString().TrimEnd('\n'), fenceLang));
                continue;
            }
            if (line.StartsWith('#'))
            {
                var level = 0;
                while (level < line.Length && line[level] == '#') level++;
                var content = line[level..].Trim();
                target.Children.Add(new TextBlock
                {
                    Text = content,
                    FontSize = level switch { 1 => 20, 2 => 18, 3 => 16, 4 => 14, _ => 13 },
                    FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
                    TextWrapping = TextWrapping.Wrap,
                    Margin = new Thickness(0, level <= 2 ? 8 : 4, 0, 2),
                });
                i++;
                continue;
            }
            if (line.StartsWith("---", StringComparison.Ordinal) || line.StartsWith("***", StringComparison.Ordinal))
            {
                target.Children.Add(new Border
                {
                    Height = 1,
                    Background = (Brush)Application.Current.Resources["StrokeSubtleBrush"],
                    Margin = new Thickness(0, 8, 0, 8),
                });
                i++;
                continue;
            }
            if (line.TrimStart().StartsWith('>') )
            {
                var body = new StringBuilder();
                while (i < lines.Length && lines[i].TrimStart().StartsWith('>'))
                {
                    var t = lines[i].TrimStart();
                    body.Append(t.TrimStart('>').Trim()).Append('\n');
                    i++;
                }
                var quote = new TextBlock
                {
                    TextWrapping = TextWrapping.Wrap,
                    Opacity = 0.85,
                    Margin = new Thickness(8, 2, 0, 2),
                };
                AppendMarkdownInlines(quote.Inlines, body.ToString().TrimEnd('\n'));
                target.Children.Add(new Border
                {
                    BorderBrush = (Brush)Application.Current.Resources["StrokeSubtleBrush"],
                    BorderThickness = new Thickness(2, 0, 0, 0),
                    Padding = new Thickness(0, 2, 0, 2),
                    Child = quote,
                });
                continue;
            }
            if (IsMarkdownListItem(line, out var bullet, out var itemText))
            {
                var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, Margin = new Thickness(4, 1, 0, 1) };
                row.Children.Add(new TextBlock { Text = bullet, Width = 18 });
                var tb = new TextBlock { TextWrapping = TextWrapping.Wrap };
                AppendMarkdownInlines(tb.Inlines, itemText);
                row.Children.Add(tb);
                target.Children.Add(row);
                i++;
                // 连续列表项
                while (i < lines.Length && IsMarkdownListItem(lines[i], out bullet, out itemText))
                {
                    var row2 = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, Margin = new Thickness(4, 1, 0, 1) };
                    row2.Children.Add(new TextBlock { Text = bullet, Width = 18 });
                    var tb2 = new TextBlock { TextWrapping = TextWrapping.Wrap };
                    AppendMarkdownInlines(tb2.Inlines, itemText);
                    row2.Children.Add(tb2);
                    target.Children.Add(row2);
                    i++;
                }
                continue;
            }
            if (line.Trim().Length == 0)
            {
                i++;
                continue;
            }
            var para = new TextBlock
            {
                TextWrapping = TextWrapping.Wrap,
                Margin = new Thickness(0, 2, 0, 2),
            };
            AppendMarkdownInlines(para.Inlines, line);
            target.Children.Add(para);
            i++;
        }
    }

    private static bool IsMarkdownListItem(string line, out string bullet, out string text)
    {
        var t = line.TrimStart();
        if (t.StartsWith("- ") || t.StartsWith("* ") || t.StartsWith("+ "))
        {
            bullet = "•";
            text = t[2..];
            return true;
        }
        var m = Regex.Match(t, @"^(\d+)[.)]\s+(.*)$");
        if (m.Success)
        {
            bullet = m.Groups[1].Value + ".";
            text = m.Groups[2].Value;
            return true;
        }
        bullet = "";
        text = "";
        return false;
    }

    private FrameworkElement BuildCodeBlock(string code, string lang)
    {
        var tb = new TextBlock
        {
            Text = code,
            FontFamily = new FontFamily("Consolas, Cascadia Mono, Courier New"),
            FontSize = 12,
            TextWrapping = _previewWrap ? TextWrapping.Wrap : TextWrapping.NoWrap,
        };
        var border = new Border
        {
            Background = (Brush)Application.Current.Resources["CardBrush"],
            BorderBrush = (Brush)Application.Current.Resources["StrokeSubtleBrush"],
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(4),
            Padding = new Thickness(8),
            Margin = new Thickness(0, 4, 0, 4),
            Child = tb,
        };
        if (!string.IsNullOrEmpty(lang))
        {
            return new StackPanel
            {
                Spacing = 2,
                Children =
                {
                    new TextBlock { Text = lang, FontSize = 11, Opacity = 0.7 },
                    border,
                },
            };
        }
        return border;
    }

    /// <summary>行内：`code`、**bold**、*em*、[text](http… 链接)。</summary>
    private void AppendMarkdownInlines(InlineCollection inlines, string text)
    {
        var i = 0;
        var n = text.Length;
        while (i < n)
        {
            if (text[i] == '\\' && i + 1 < n)
            {
                inlines.Add(new Run { Text = text[i + 1].ToString() });
                i += 2;
                continue;
            }
            if (text[i] == '`')
            {
                var end = text.IndexOf('`', i + 1);
                if (end > i)
                {
                    inlines.Add(new Run
                    {
                        Text = text[(i + 1)..end],
                        FontFamily = new FontFamily("Consolas, Cascadia Mono, Courier New"),
                    });
                    i = end + 1;
                    continue;
                }
            }
            if (text.AsSpan(i).StartsWith("**"))
            {
                var end = text.IndexOf("**", i + 2, StringComparison.Ordinal);
                if (end > i)
                {
                    inlines.Add(new Run { Text = text[(i + 2)..end], FontWeight = Microsoft.UI.Text.FontWeights.Bold });
                    i = end + 2;
                    continue;
                }
            }
            if (text[i] == '*' && i + 1 < n && text[i + 1] != '*')
            {
                var end = text.IndexOf('*', i + 1);
                if (end > i)
                {
                    inlines.Add(new Run { Text = text[(i + 1)..end], FontStyle = Windows.UI.Text.FontStyle.Italic });
                    i = end + 1;
                    continue;
                }
            }
            if (text[i] == '[')
            {
                var close = text.IndexOf(']', i + 1);
                if (close > i && close + 1 < n && text[close + 1] == '(')
                {
                    var end = text.IndexOf(')', close + 2);
                    if (end > close)
                    {
                        var label = text[(i + 1)..close];
                        var href = text[(close + 2)..end];
                        if (Uri.TryCreate(href, UriKind.Absolute, out var uri) &&
                            (uri.Scheme == Uri.UriSchemeHttp || uri.Scheme == Uri.UriSchemeHttps))
                        {
                            var link = new Hyperlink { NavigateUri = uri };
                            link.Inlines.Add(new Run { Text = label });
                            link.Click += OnMarkdownLinkClick;
                            inlines.Add(link);
                        }
                        else
                        {
                            inlines.Add(new Run { Text = label });
                        }
                        i = end + 1;
                        continue;
                    }
                }
            }
            inlines.Add(new Run { Text = text[i].ToString() });
            i++;
        }
    }

    private async void OnMarkdownLinkClick(Hyperlink sender, HyperlinkClickEventArgs args)
    {
        try
        {
            if (sender.NavigateUri is { } uri &&
                (uri.Scheme == Uri.UriSchemeHttp || uri.Scheme == Uri.UriSchemeHttps))
            {
                await Windows.System.Launcher.LaunchUriAsync(uri);
            }
        }
        catch (Exception) { }
    }

    // ---------------- 图片（readBytes） ----------------

    private async Task LoadImageFaceAsync(string sid, string path, long previewGeneration)
    {
        var bytes = await ReadFileBytesAsync(sid, path, previewGeneration);
        if (previewGeneration != _previewGeneration) return;
        _previewBytes = bytes;
        if (bytes.Length == 0)
        {
            ShowPreviewErrorFace(MainWindow.TL("空文件"), MainWindow.TL("文件大小为 0，没有可显示的图像。"));
            return;
        }
        try
        {
            PreviewImage.Source = await DecodeBitmapAsync(bytes);
            PreviewMeta.Text = MainWindow.TLF("{0} · 版本 {1}", FormatSize(bytes.LongLength), Short(_previewVersion));
        }
        catch (Exception)
        {
            // svg / 损坏字节：WinUI 位图解码失败 → 明确降级说明
            ShowPreviewErrorFace(
                MainWindow.TL("无法解码图像"),
                MainWindow.TLF("已读取 {0} 字节，但系统位图解码器无法识别该格式（SVG 等矢量图不在壳内渲染）。", bytes.Length));
        }
    }

    private static async Task<BitmapImage> DecodeBitmapAsync(byte[] bytes)
    {
        using var stream = new InMemoryRandomAccessStream();
        using (var writer = new DataWriter(stream))
        {
            writer.WriteBytes(bytes);
            await writer.StoreAsync();
            await writer.FlushAsync();
            writer.DetachStream();
        }
        stream.Seek(0);
        var bitmap = new BitmapImage();
        await bitmap.SetSourceAsync(stream);
        return bitmap;
    }

    // ---------------- PDF（readBytes + Windows.Data.Pdf；失败降级） ----------------

    private async Task LoadPdfFaceAsync(string sid, string path, long previewGeneration)
    {
        var bytes = await ReadFileBytesAsync(sid, path, previewGeneration);
        if (previewGeneration != _previewGeneration) return;
        _previewBytes = bytes;
        PreviewMeta.Text = MainWindow.TLF("{0} · 版本 {1}", FormatSize(bytes.LongLength), Short(_previewVersion));
        try
        {
            using var stream = new InMemoryRandomAccessStream();
            using (var writer = new DataWriter(stream))
            {
                writer.WriteBytes(bytes);
                await writer.StoreAsync();
                await writer.FlushAsync();
                writer.DetachStream();
            }
            stream.Seek(0);
            _pdfDoc = await Windows.Data.Pdf.PdfDocument.LoadFromStreamAsync(stream);
            _pdfPage = 0;
            await ShowPdfPageAsync(previewGeneration);
        }
        catch (Exception)
        {
            // 加密 / 损坏 / 平台限制：占位 + 明确降级文案 + 用系统打开 + 复制路径
            // Windows.Data.Pdf.PdfDocument.LoadFromStreamAsync 无 password 参数，壳内无法解锁。
            _pdfDoc = null;
            PdfPageImage.Source = null;
            PdfPrevButton.IsEnabled = false;
            PdfNextButton.IsEnabled = false;
            var encrypted = LooksLikeEncryptedPdf(bytes);
            PdfPageLabel.Text = encrypted
                ? MainWindow.TL("加密 PDF")
                : MainWindow.TL("无法在壳内渲染");
            ShowStatus(InfoBarSeverity.Informational, encrypted
                ? MainWindow.TL("加密 PDF：Windows.Data.Pdf 无解锁 API，壳内无法解密渲染。可用「用系统打开」查看，或复制路径后用其它工具解锁。")
                : MainWindow.TL("文件已损坏或平台不支持渲染。"));
        }
    }

    private async Task ShowPdfPageAsync(long previewGeneration)
    {
        if (_pdfDoc is null || _pdfDoc.PageCount == 0)
        {
            PdfPageLabel.Text = MainWindow.TL("无页面");
            return;
        }
        if (_pdfPage < 0) _pdfPage = 0;
        if (_pdfPage >= (int)_pdfDoc.PageCount) _pdfPage = (int)_pdfDoc.PageCount - 1;
        using var page = _pdfDoc.GetPage((uint)_pdfPage);
        using var outStream = new InMemoryRandomAccessStream();
        await page.RenderToStreamAsync(outStream);
        if (previewGeneration != _previewGeneration) return;
        outStream.Seek(0);
        var bitmap = new BitmapImage();
        await bitmap.SetSourceAsync(outStream);
        PdfPageImage.Source = bitmap;
        PdfPageLabel.Text = MainWindow.TLF("第 {0} / {1} 页", _pdfPage + 1, (int)_pdfDoc.PageCount);
        PdfPrevButton.IsEnabled = _pdfPage > 0;
        PdfNextButton.IsEnabled = _pdfPage + 1 < (int)_pdfDoc.PageCount;
    }

    private async void OnPdfPrevClick(object sender, RoutedEventArgs e)
    {
        try
        {
            _pdfPage--;
            await ShowPdfPageAsync(_previewGeneration);
        }
        catch (Exception) { }
    }

    private async void OnPdfNextClick(object sender, RoutedEventArgs e)
    {
        try
        {
            _pdfPage++;
            await ShowPdfPageAsync(_previewGeneration);
        }
        catch (Exception) { }
    }

    /// <summary>用系统打开：优先内核绝对路径，否则落临时文件再 ShellExecute。</summary>
    private async void OnPdfOpenClick(object sender, RoutedEventArgs e)
    {
        try
        {
            var path = _previewAbsolutePath;
            if (path is not null && File.Exists(path))
            {
                System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(path) { UseShellExecute = true });
                return;
            }
            if (_previewBytes is null || _previewBytes.Length == 0) return;
            var tmp = System.IO.Path.Combine(System.IO.Path.GetTempPath(), $"blade2-preview-{Guid.NewGuid():N}.pdf");
            await File.WriteAllBytesAsync(tmp, _previewBytes);
            System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(tmp) { UseShellExecute = true });
        }
        catch (Exception ex)
        {
            ShowStatus(InfoBarSeverity.Warning, MainWindow.TLF("用系统打开失败：{0}", ex.Message));
        }
    }

    /// <summary>复制路径：优先绝对路径，否则工作区相对路径。</summary>
    private async void OnPdfCopyPathClick(object sender, RoutedEventArgs e)
    {
        try
        {
            var text = _previewAbsolutePath ?? _previewPath ?? "";
            if (text.Length == 0) return;
            var package = new Windows.ApplicationModel.DataTransfer.DataPackage();
            package.SetText(text);
            Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(package);
            ShowStatus(InfoBarSeverity.Success, MainWindow.TL("路径已复制"));
            await Task.CompletedTask;
        }
        catch (Exception ex)
        {
            ShowStatus(InfoBarSeverity.Warning, MainWindow.TLF("复制路径失败：{0}", ex.Message));
        }
    }

    /// <summary>PDF 是否像加密文档：trailer/对象里出现 /Encrypt（启发式，覆盖常见加密 PDF）。</summary>
    private static bool LooksLikeEncryptedPdf(byte[] bytes)
    {
        if (bytes.Length < 8) return false;
        // 只扫尾部 64KB：trailer 通常在文件末尾
        var start = Math.Max(0, bytes.Length - 64 * 1024);
        var window = Encoding.Latin1.GetString(bytes, start, bytes.Length - start);
        return window.Contains("/Encrypt", StringComparison.Ordinal);
    }

    // ---------------- HTML：路 A WebView2 DOM（默认）+ 路 B 源码/大纲/附属 ----------------

    private async Task LoadHtmlFaceAsync(string sid, string path, long previewGeneration)
    {
        // 源码缓冲始终维护（Copy 全文 / 源码切换 / 大纲解析共用）
        await LoadTextPageAsync(sid, path, previewGeneration, append: false);
        if (previewGeneration != _previewGeneration) return;

        var outline = BuildHtmlOutline(_previewBuffer);
        var refs = DiscoverHtmlRelated(_previewBuffer);

        if (_htmlDomPreferred && !_htmlWebViewFailed)
        {
            var ok = await TryShowHtmlDomAsync(sid, path, previewGeneration, refs);
            if (ok)
            {
                // DOM 成功：大纲摘要仍给一行（可切源码看细节）
                HtmlOutlineSummary.Visibility = Visibility.Visible;
                HtmlOutlineSummary.Text = MainWindow.TLF(
                    "DOM 预览 · 标题：{0} · 一级标题 {1} · 链接 {2} · 图片 {3}（脚本已禁用）",
                    outline.Title.Length > 0 ? outline.Title : MainWindow.TL("（无）"),
                    outline.H1Count,
                    outline.LinkCount,
                    outline.ImgCount);
                HtmlRelatedSummary.Visibility = refs.Count == 0 ? Visibility.Collapsed : Visibility.Visible;
                if (refs.Count > 0)
                {
                    // 附属不再内联（readRelated 自内核 0.1.7 起移除）：如实标注未取回
                    HtmlRelatedSummary.Text = MainWindow.TLF("附属资源：{0} 项（当前内核不提供读取，未内联）", refs.Count);
                }
                HtmlRelatedList.Visibility = Visibility.Collapsed;
                HtmlRelatedList.Children.Clear();
                ApplyPreviewFace(PreviewFace.Html);
                PreviewMeta.Text = MainWindow.TLF("{0} · 版本 {1} · DOM", FormatSize(_previewStatBytes), Short(_previewVersion));
                return;
            }
            // Runtime 缺失 / 初始化失败：落到路 B 并记住，不再重试
            _htmlDomPreferred = false;
            _htmlWebViewFailed = true;
        }

        // 路 B：源码 + 标签结构大纲 + readRelated 附属可点开
        ApplyPreviewFace(PreviewFace.Html); // 回落源码面
        HtmlOutlineSummary.Visibility = Visibility.Visible;
        HtmlOutlineSummary.Text = MainWindow.TLF(
            "标签结构 · 标题：{0} · 一级标题 {1} · 链接 {2} · 图片 {3}",
            outline.Title.Length > 0 ? outline.Title : MainWindow.TL("（无）"),
            outline.H1Count,
            outline.LinkCount,
            outline.ImgCount);
        if (refs.Count == 0)
        {
            HtmlRelatedSummary.Visibility = Visibility.Visible;
            HtmlRelatedSummary.Text = MainWindow.TL("附属资源：无（未发现相对路径的 script/link/img 引用）。");
            HtmlRelatedList.Visibility = Visibility.Collapsed;
            HtmlRelatedList.Children.Clear();
            return;
        }
        HtmlRelatedSummary.Visibility = Visibility.Visible;
        HtmlRelatedSummary.Text = MainWindow.TL("附属资源（点击在预览中打开）：");
        HtmlRelatedList.Children.Clear();
        HtmlRelatedList.Visibility = Visibility.Visible;
        foreach (var rel in refs.Take(MaxHtmlRelated))
        {
            if (previewGeneration != _previewGeneration) return;
            // 附属字节读取（readRelated）自内核 0.1.7 起移除：条目只做跳转，不再显示大小
            var label = rel;
            // 可点开：点条目在预览里打开该附属文件（相对主文档解析）
            var relCaptured = rel;
            var open = new Button
            {
                Content = label,
                Style = Application.Current.Resources["CompactButtonStyle"] as Microsoft.UI.Xaml.Style,
                HorizontalAlignment = HorizontalAlignment.Left,
                Padding = new Thickness(8, 2, 8, 2),
                FontSize = 11,
            };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(open, $"HtmlRelatedOpen_{rel}");
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(open, MainWindow.TLF("打开附属资源 {0}", relCaptured));
            ToolTipService.SetToolTip(open, MainWindow.TL("打开附属资源"));
            open.Click += (_, _) =>
            {
                try
                {
                    var target = ResolveHtmlRelatedPath(path, relCaptured);
                    if (target is not null)
                    {
                        _ = ShowPreviewAsync(new FileNode { Name = DisplayNameOf(target), Path = target, Type = "file" });
                    }
                }
                catch (Exception) { }
            };
            HtmlRelatedList.Children.Add(open);
        }
        if (refs.Count > MaxHtmlRelated)
        {
            HtmlRelatedList.Children.Add(new TextBlock
            {
                Text = MainWindow.TLF("…共 {0} 项", refs.Count),
                FontSize = 11,
                Opacity = 0.7,
                TextWrapping = TextWrapping.Wrap,
            });
        }
    }

    /// <summary>HTML DOM/源码切换。勾选 = 源码（路 B）。</summary>
    private void OnHtmlModeToggle(object sender, RoutedEventArgs e)
    {
        try
        {
            _htmlDomPreferred = HtmlModeToggle.IsChecked != true;
            if (_previewPath is not null && _session is not null)
            {
                _ = ShowPreviewAsync(new FileNode { Name = DisplayNameOf(_previewPath), Path = _previewPath, Type = "file" });
            }
        }
        catch (Exception) { }
    }

    /// <summary>
    /// WebView2 DOM 渲染。安全边界：
    ///   · IsScriptEnabled=false（官方 sandbox allow-scripts；壳关脚本，JS 交互不等价）
    ///   · AreDefaultScriptDialogsEnabled=false / IsWebMessageEnabled=false
    ///   · 禁右键 / DevTools / 缩放 / 状态栏 / 内置错误页
    ///   · NavigationStarting 只放行 about: / data:；NewWindowRequested 一律拦截
    ///   · 内容自包含：外部 URL 剥离、script 去掉、本地 CSS/图内联
    /// </summary>
    private async Task<bool> TryShowHtmlDomAsync(string sid, string path, long previewGeneration, List<string> refs)
    {
        try
        {
            await EnsureHtmlWebViewAsync();
            if (_htmlWebViewFailed || HtmlWebView.CoreWebView2 is not { } core) return false;
            if (previewGeneration != _previewGeneration) return false;

            var doc = await BuildSelfContainedHtmlAsync(sid, path, previewGeneration, refs);
            if (previewGeneration != _previewGeneration) return false;
            core.NavigateToString(doc);
            return true;
        }
        catch (Exception)
        {
            _htmlWebViewFailed = true;
            return false;
        }
    }

    private async Task EnsureHtmlWebViewAsync()
    {
        if (_htmlWebViewReady && HtmlWebView.CoreWebView2 is not null) return;
        if (_htmlWebViewFailed) return;
        try
        {
            await HtmlWebView.EnsureCoreWebView2Async();
            var core = HtmlWebView.CoreWebView2;
            if (core is null)
            {
                _htmlWebViewFailed = true;
                return;
            }
            var s = core.Settings;
            s.IsScriptEnabled = false;
            s.AreDefaultScriptDialogsEnabled = false;
            s.IsWebMessageEnabled = false;
            s.AreDefaultContextMenusEnabled = false;
            s.AreDevToolsEnabled = false;
            s.IsStatusBarEnabled = false;
            s.IsZoomControlEnabled = false;
            s.IsBuiltInErrorPageEnabled = false;
            s.IsGeneralAutofillEnabled = false;
            s.IsPasswordAutosaveEnabled = false;
            s.IsSwipeNavigationEnabled = false;
            core.NavigationStarting += (_, e) =>
            {
                // 只放行 NavigateToString 产生的 about:blank / data: 文档
                var u = e.Uri ?? "";
                if (u.Length == 0) return;
                if (u.StartsWith("about:", StringComparison.OrdinalIgnoreCase)) return;
                if (u.StartsWith("data:", StringComparison.OrdinalIgnoreCase)) return;
                e.Cancel = true;
            };
            core.NewWindowRequested += (_, e) => e.Handled = true;
            core.ProcessFailed += (_, _) => { _htmlWebViewFailed = true; };
            _htmlWebViewReady = true;
        }
        catch (Exception)
        {
            // WebView2 Runtime 缺失 / 环境不支持
            _htmlWebViewFailed = true;
        }
    }

    /// <summary>
    /// 构建自包含 HTML：剥离 script/外部 URL，本地 CSS 内联、图片转 data:，加 CSP 收紧子资源。
    /// 只读取工作区内相对路径附属（ResolveHtmlRelatedPath 拒绝越出工作区的 ..）。
    /// </summary>
    private async Task<string> BuildSelfContainedHtmlAsync(string sid, string path, long previewGeneration, List<string> refs)
    {
        var html = _previewBuffer;
        // 去 script（含内联/外部）：脚本面由 IsScriptEnabled=false 双保险
        html = Regex.Replace(html, @"<script\b[^>]*>[\s\S]*?</script\s*>", "", RegexOptions.IgnoreCase);
        html = Regex.Replace(html, @"<script\b[^>]*/>", "", RegexOptions.IgnoreCase);
        // 外部 URL 剥离（src/href 含协议或 // 开头 → 置空）
        html = Regex.Replace(html,
            @"(\s(?:src|href)\s*=\s*)([""'])(?:[a-zA-Z][a-zA-Z0-9+.-]*:|//)[^""']*\2",
            "$1$2$2",
            RegexOptions.IgnoreCase);

        // 附属资源（CSS/图片）不再内联：workspaceFiles/readRelated 自内核 0.1.7 起移除，
        // 壳侧不做等价重建——DOM 预览保留结构与文案，相对引用的样式/图片不再取回。
        // 相对引用的 link/img 一并剥离（NavigateToString 基址为 about:blank，本就取不到）。
        html = Regex.Replace(html, @"<link\b[^>]*>", "", RegexOptions.IgnoreCase);
        html = Regex.Replace(html, @"<img\b[^>]*>", "", RegexOptions.IgnoreCase);

        // CSP：禁一切外部子资源；允许内联样式与 data: 图
        const string csp =
            "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data:; style-src 'unsafe-inline'; font-src 'none'; connect-src 'none'; script-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'\">";

        // 确保有 charset；NavigateToString 文档基址为 about:blank，相对 URL 已被剥离
        var inject = "<meta charset=\"utf-8\">" + csp;
        if (Regex.IsMatch(html, @"<head[\s>]", RegexOptions.IgnoreCase))
        {
            html = new Regex(@"<head([\s>])", RegexOptions.IgnoreCase, TimeSpan.FromSeconds(1))
                .Replace(html, "<head$1" + inject, 1);
        }
        else if (Regex.IsMatch(html, @"<html[\s>]", RegexOptions.IgnoreCase))
        {
            html = new Regex(@"<html([\s>])", RegexOptions.IgnoreCase, TimeSpan.FromSeconds(1))
                .Replace(html, "<html$1<head>" + inject + "</head>", 1);
        }
        else
        {
            html = "<!doctype html>" + inject + html;
        }
        return html;
    }

    /// <summary>HTML 基础标签结构：title 文本 + h1/a/img 计数。</summary>
    private static (string Title, int H1Count, int LinkCount, int ImgCount) BuildHtmlOutline(string html)
    {
        var title = "";
        var tm = Regex.Match(html, @"<title[^>]*>(.*?)</title>", RegexOptions.IgnoreCase | RegexOptions.Singleline);
        if (tm.Success) title = Regex.Replace(tm.Groups[1].Value, @"\s+", " ").Trim();
        var h1 = Regex.Matches(html, @"<h1[\s>]", RegexOptions.IgnoreCase).Count;
        var links = Regex.Matches(html, @"<a\s[^>]*href\s*=", RegexOptions.IgnoreCase).Count;
        var imgs = Regex.Matches(html, @"<img\s", RegexOptions.IgnoreCase).Count;
        return (title, h1, links, imgs);
    }

    /// <summary>把 HTML 相对引用解析成工作区路径（主文档目录 + rel，拒绝越出工作区的 ..）。</summary>
    private string? ResolveHtmlRelatedPath(string htmlWorkspacePath, string rel)
    {
        try
        {
            var dir = System.IO.Path.GetDirectoryName(htmlWorkspacePath.Replace('\\', '/')) ?? "";
            var combined = System.IO.Path.GetFullPath(System.IO.Path.Combine(dir, rel.Replace('/', System.IO.Path.DirectorySeparatorChar)))
                .Replace('\\', '/');
            // 只允许工作区相对形态（workspaceFiles 以相对路径寻址）
            if (combined.StartsWith("/", StringComparison.Ordinal) || combined.Contains(":")) return null;
            return combined;
        }
        catch (Exception) { return null; }
    }

    /// <summary>收集相对路径的 script src / link href / img src（跳过绝对 URL 与协议）。</summary>
    private static List<string> DiscoverHtmlRelated(string html)
    {
        var found = new List<string>();
        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (Match m in Regex.Matches(html, @"(?:src|href)\s*=\s*[""']([^""']+)[""']", RegexOptions.IgnoreCase))
        {
            var raw = m.Groups[1].Value.Trim();
            if (raw.Length == 0) continue;
            if (raw.StartsWith("data:", StringComparison.OrdinalIgnoreCase)) continue;
            if (raw.StartsWith("#")) continue;
            if (Regex.IsMatch(raw, @"^[a-zA-Z][a-zA-Z0-9+.-]*:")) continue; // http: mailto: javascript:
            if (raw.StartsWith('/') || raw.StartsWith('\\')) continue;
            var cut = raw.IndexOfAny(['?', '#']);
            var path = cut >= 0 ? raw[..cut] : raw;
            path = Uri.UnescapeDataString(path).Replace('\\', '/');
            if (path.Length == 0 || path.Contains("..")) continue;
            if (seen.Add(path)) found.Add(path);
        }
        return found;
    }

    // ---------------- 工具条 ----------------

    private void OnWrapToggle(object sender, RoutedEventArgs e)
    {
        _previewWrap = WrapToggle.IsChecked == true;
        PreviewRich.TextWrapping = _previewWrap ? TextWrapping.Wrap : TextWrapping.NoWrap;
        TextScroll.HorizontalScrollMode = _previewWrap ? ScrollMode.Disabled : ScrollMode.Auto;
        TextScroll.HorizontalScrollBarVisibility = _previewWrap ? ScrollBarVisibility.Disabled : ScrollBarVisibility.Auto;
        ToolTipService.SetToolTip(WrapToggle, _previewWrap ? MainWindow.TL("取消换行") : MainWindow.TL("自动换行"));
    }

    private async void OnReloadPreviewClick(object sender, RoutedEventArgs e)
    {
        try
        {
            await ReloadPreviewFromAsync();
        }
        catch (Exception) { }
    }

    private async void OnChangedReloadClick(object sender, RoutedEventArgs e)
    {
        try
        {
            await ReloadPreviewFromAsync();
        }
        catch (Exception) { }
    }

    private async Task ReloadPreviewFromAsync()
    {
        if (_previewPath is null) return;
        await ShowPreviewAsync(new FileNode { Name = DisplayNameOf(_previewPath), Path = _previewPath, Type = "file" });
    }

    private async void OnLoadMoreClick(object sender, RoutedEventArgs e)
    {
        try
        {
            if (_rpc is null || _session is null || _previewPath is null || _previewEof) return;
            LoadMoreButton.IsEnabled = false;
            var sid = _session;
            var path = _previewPath;
            var generation = _previewGeneration;
            await LoadTextPageAsync(sid, path, generation, append: true);
        }
        catch (Exception ex)
        {
            if (ex is not OperationCanceledException) ShowError(MainWindow.TL("加载更多失败"), ex);
        }
        finally
        {
            LoadMoreButton.IsEnabled = true;
        }
    }

    private void OnCopyAllClick(object sender, RoutedEventArgs e)
    {
        try
        {
            // 只复制已缓冲内容：workspaceFiles/readAll 自内核 0.1.7 起移除，
            // 「未加载完先取全文」路径随之撤除（需要全文请先加载完或重新载入）。
            var text = _previewBuffer;
            if (text.Length == 0 && _previewBytes is not null)
            {
                return; // 二进制面不提供全文复制
            }
            CopyToClipboard(text);
            ShowStatus(InfoBarSeverity.Success, MainWindow.TL("已复制"));
        }
        catch (Exception) { }
    }

    private void OnCopySelectionClick(object sender, RoutedEventArgs e)
    {
        try
        {
            var selected = PreviewRich.SelectedText;
            if (string.IsNullOrEmpty(selected))
            {
                ShowStatus(InfoBarSeverity.Informational, MainWindow.TL("没有选中的文本。"));
                return;
            }
            CopyToClipboard(selected);
            ShowStatus(InfoBarSeverity.Success, MainWindow.TL("已复制"));
        }
        catch (Exception) { }
    }

    private static void CopyToClipboard(string text)
    {
        var package = new DataPackage();
        package.SetText(text);
        Clipboard.SetContent(package);
    }

    // ---------------- 占位（预览态内的失败面） ----------------

    private void ShowPreviewErrorFace(string title, string detail)
    {
        PreviewRich.Blocks.Clear();
        MarkdownHost.Children.Clear();
        PreviewImage.Source = null;
        PdfPageImage.Source = null;
        ApplyPreviewFace(PreviewFace.PlainText);
        RenderPlainText($"{title}\n{detail}", PreviewRich, clear: true);
        PreviewMeta.Text = "";
        BackButton.Visibility = Visibility.Visible;
    }
}
