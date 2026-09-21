using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Blade2.Dsh;

namespace Blade2;

/// <summary>文件树节点数据（TreeViewNode.Content）。</summary>
public sealed class FileNode
{
    public string Name { get; init; } = "";
    /// <summary>绝对路径："工作区根/子/孙"（"/" 分隔，与 dsh 原版 sidebar-files 的 childPath 同式）。</summary>
    public string Path { get; init; } = "";
    public string Type { get; init; } = "file";   // file | directory | other
    public long? Size { get; init; }
    public bool IsPlaceholder { get; init; }

    public bool IsDirectory => Type == "directory";

    /// <summary>行图标（Segoe Fluent Icons）：目录 = 文件夹，其余按扩展名给文档/图片/代码。</summary>
    public string Glyph => IsPlaceholder
        ? "\uE73E"
        : IsDirectory
            ? "\uE8B7"
            : FileGlyph(Name);

    /// <summary>占位行（"（空目录）"等）半透明，与真实条目区分。</summary>
    public double RowOpacity => IsPlaceholder ? 0.6 : 1.0;

    /// <summary>TreeViewItem 的 UIA 名回落到 Content.ToString：不给这一行的话辅助技术
    /// 读到的会是类型名（"Blade2.FileNode"，实测），而不是文件名。</summary>
    public override string ToString() => Name;

    private static string FileGlyph(string name)
    {
        var ext = System.IO.Path.GetExtension(name).ToLowerInvariant();
        return ext switch
        {
            ".png" or ".jpg" or ".jpeg" or ".gif" or ".webp" or ".bmp" or ".ico" or ".svg" => "\uEB9F",
            ".cs" or ".js" or ".mjs" or ".ts" or ".tsx" or ".jsx" or ".py" or ".rs" or ".go" or ".java" or ".c" or ".h" or ".cpp" or ".ps1" or ".sh" or ".xaml" or ".json" or ".yml" or ".yaml" or ".toml" or ".xml" => "\uE943",
            _ => "\uE7C3",
        };
    }
}

/// <summary>
/// 工作区文件面板（右侧栏）：文件树 + 文本预览 + 变更自动刷新。
/// RPC 契约（内核 @deepseek-ai/dsh-api-workspace-files 的 typert 描述符）：
///   workspaceFiles/list    args { workspaceFileScopeId, path } → { path, entries[{name,type,size?}], truncated }
///   workspaceFiles/stat    args { workspaceFileScopeId, path } → { absolutePath, version, bytes? }
///   workspaceFiles/read    args { workspaceFileScopeId, path, range{offset,limit} } → { offset, text, lines, eof, absolutePath, version, bytes? }
///   workspaceFiles/changes args { workspaceFileScopeId }      → 流 {kind:"ready"} | {kind:"change", change:{absolutePath,version}|{absolutePath,absent:true}}
/// workspaceFileScopeId = 会话 id（内核 lookup "workspaceFileScope" 由会话 header.cwd 解出工作区根）；
/// path 用**绝对路径**（工作区根 + "/" + 每级名），相对路径同样可解析但绝对路径无歧义。
/// </summary>
public sealed partial class FilesPanel : UserControl
{
    private DshRpcClient? _rpc;
    private Func<string?>? _sessionIdProvider;
    private Func<string?>? _workspaceRootProvider;

    private string? _session;              // 当前树所属会话 id（变更即整树重载）
    private string? _root;                 // 当前工作区根（"/" 分隔，无尾斜杠）
    private string? _changesStreamId;      // workspaceFiles/changes 流 id
    private DispatcherTimer? _changesDebounce;
    private bool _changesReady;
    private string? _previewPath;          // 预览中的绝对路径；null = 树模式
    private string? _previewVersion;       // 最近一次 read 的版本号（变更比对用）
    private bool _busy;                    // 当前树请求是否进行中
    private long _generation;              // 作废旧会话/旧树请求及其 UI 回写
    private long _previewGeneration;       // 作废旧预览（含返回后仍在途的读取）
    private long _streamGeneration;        // 作废已关闭或重连前的流回调
    private bool _changesOpening;

    public FilesPanel()
    {
        InitializeComponent();
        _changesDebounce = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(450) };
        _changesDebounce.Tick += (_, _) =>
        {
            _changesDebounce!.Stop();
            _ = AutoRefreshAsync();
        };
    }

    /// <summary>面板请求关闭（标题栏关闭钮）——由宿主收起本栏。</summary>
    public event Action? CloseRequested;

    /// <summary>绑定 RPC 与"当前会话/工作区根"提供器（宿主在启动时调用一次）。</summary>
    public void Attach(DshRpcClient rpc, Func<string?> sessionId, Func<string?> workspaceRoot)
    {
        if (_rpc is not null)
        {
            _rpc.StreamsReset -= OnStreamsReset;
            CloseChangesStream();
        }
        _generation++;
        _previewGeneration++;
        _busy = false;
        _rpc = rpc;
        _rpc.StreamsReset += OnStreamsReset;
        _sessionIdProvider = sessionId;
        _workspaceRootProvider = workspaceRoot;
    }

    /// <summary>面板可见性由宿主控制：显示时重载，隐藏时收掉变更流（不留长驻流）。</summary>
    public void OnVisibilityChanged(bool visible)
    {
        if (visible)
        {
            _ = ReloadAsync();
        }
        else
        {
            _generation++;
            _previewGeneration++;
            _busy = false;
            CloseChangesStream();
        }
    }

    /// <summary>会话切换：工作区根随会话变，整树与预览重置。</summary>
    public void SetSession(string? sessionId, string? workspaceRoot)
    {
        var root = NormalizeRoot(workspaceRoot);
        if (sessionId == _session && root == _root)
        {
            return;
        }
        ResetSession(sessionId, root);
        if (Visibility == Visibility.Visible)
        {
            // 使用传入的新会话，不能被 provider 的更新时序或旧请求的 busy 阻止。
            _ = ReloadCurrentAsync();
        }
    }

    private void ResetSession(string? sessionId, string? root)
    {
        _generation++;
        _busy = false;
        _session = sessionId;
        _root = root;
        CloseChangesStream();
        ShowTree();
        FileTree.RootNodes.Clear();
        HeaderTitle.Text = "工作区文件";
        HeaderPath.Text = root?.Replace('/', '\\') ?? "";
        HideStatus();
    }

    private bool IsCurrent(long generation, string? sid) =>
        generation == _generation && sid == _session;

    // ---------------- 数据加载 ----------------

    /// <summary>整树重载：拉工作区根目录 + （重新）订阅变更流。</summary>
    public Task ReloadAsync()
    {
        var sid = _sessionIdProvider is null ? _session : _sessionIdProvider();
        var root = _workspaceRootProvider is null ? _root : NormalizeRoot(_workspaceRootProvider());
        if (sid != _session || root != _root)
        {
            ResetSession(sid, root);
        }
        return ReloadCurrentAsync();
    }

    private async Task ReloadCurrentAsync()
    {
        if (_rpc is null)
        {
            return;
        }
        var sid = _session;
        var root = _root ?? "."; // cwd 缺失时让内核按 session 的 sandbox 根解析
        var generation = ++_generation;
        _busy = true;
        try
        {
            ShowTree();
            HideStatus(); // 必须在 list 前清理，保留本次 truncated 提示
            FileTree.RootNodes.Clear();
            HeaderTitle.Text = MainWindow.TL("工作区文件");
            HeaderPath.Text = _root?.Replace('/', '\\') ?? "";
            if (sid is null)
            {
                ShowPlaceholder("\uE8B7", MainWindow.TL("未选择会话"),
                    MainWindow.TL("在左侧选择一个会话后，这里显示该会话工作区的文件树。"));
                return;
            }

            var ok = await LoadRootAsync(sid, root, generation);
            if (IsCurrent(generation, sid) && ok)
            {
                await EnsureChangesStreamAsync(sid);
            }
        }
        catch (Exception ex)
        {
            if (IsCurrent(generation, sid)) ShowError(MainWindow.TL("读取工作区失败"), ex);
        }
        finally
        {
            if (IsCurrent(generation, sid)) _busy = false;
        }
    }

    /// <summary>建根节点并展开一层。</summary>
    private async Task<bool> LoadRootAsync(string sid, string root, long generation)
    {
        var entries = await ListDirectoryAsync(sid, root, generation);
        if (!IsCurrent(generation, sid)) return false;
        if (entries is null)
        {
            FileTree.RootNodes.Clear();
            return false;
        }
        var rootNode = new TreeViewNode
        {
            Content = new FileNode { Name = DisplayNameOf(root), Path = root, Type = "directory" },
        };
        FileTree.RootNodes.Clear();
        FileTree.RootNodes.Add(rootNode);
        FillNode(rootNode, entries);
        // 先填后展：IsExpanded 在 HasUnrealizedChildren=true 时置位会触发 Expanding（重复 list 一次）
        rootNode.IsExpanded = true;
        return true;
    }

    /// <summary>目录 listing → FileNode 列表（目录在前，名字序）。失败返回 null 并显示状态。</summary>
    private async Task<List<FileNode>?> ListDirectoryAsync(string sid, string path, long generation)
    {
        var rpc = _rpc!;
        try
        {
            var value = await rpc.CallOkAsync("workspaceFiles/list", new { workspaceFileScopeId = sid, path });
            if (!IsCurrent(generation, sid)) return null;
            var list = new List<FileNode>();
            foreach (var e in value.GetProperty("entries").EnumerateArray())
            {
                var name = e.TryGetProperty("name", out var n) ? n.GetString() ?? "" : "";
                if (name.Length == 0)
                {
                    continue;
                }
                list.Add(new FileNode
                {
                    Name = name,
                    Path = JoinPath(path, name),
                    Type = e.TryGetProperty("type", out var t) ? t.GetString() ?? "other" : "other",
                    Size = e.TryGetProperty("size", out var s) && s.ValueKind == JsonValueKind.Number ? s.GetInt64() : null,
                });
            }
            var truncated = value.TryGetProperty("truncated", out var tr) && tr.ValueKind == JsonValueKind.True;
            if (truncated)
            {
                ShowStatus(InfoBarSeverity.Informational, MainWindow.TLF("目录项超过内核上限，仅显示前 {0} 项。", list.Count));
            }
            return [.. list.OrderByDescending(x => x.IsDirectory).ThenBy(x => x.Name, StringComparer.OrdinalIgnoreCase)];
        }
        catch (Exception ex)
        {
            if (!IsCurrent(generation, sid)) return null;
            ShowError(MainWindow.TLF("列目录失败（{0}）", path), ex);
            return null;
        }
    }

    /// <summary>把子项填进已有节点（懒加载 + 自动刷新共用）。</summary>
    private static void FillNode(TreeViewNode node, List<FileNode> entries)
    {
        node.Children.Clear();
        if (entries.Count == 0)
        {
            node.Children.Add(new TreeViewNode
            {
                Content = new FileNode { Name = MainWindow.TL("（空目录）"), IsPlaceholder = true },
                HasUnrealizedChildren = false,
            });
        }
        else
        {
            foreach (var entry in entries)
            {
                node.Children.Add(new TreeViewNode
                {
                    Content = entry,
                    HasUnrealizedChildren = entry.IsDirectory,
                });
            }
        }
        node.HasUnrealizedChildren = false;
    }

    private void OnNodeExpanding(TreeView sender, TreeViewExpandingEventArgs args)
    {
        if (!args.Node.HasUnrealizedChildren || args.Node.Content is not FileNode node || _session is null)
        {
            return;
        }
        var target = args.Node;
        var sid = _session;
        var generation = _generation;
        _ = ExpandAsync(target, node, sid, generation);
    }

    private async Task ExpandAsync(TreeViewNode target, FileNode node, string sid, long generation)
    {
        var entries = await ListDirectoryAsync(sid, node.Path, generation);
        if (entries is null)
        {
            if (IsCurrent(generation, sid)) target.HasUnrealizedChildren = false; // 读失败不快照重试，等用户手动刷新
            return;
        }
        if (!IsCurrent(generation, sid) || !ReferenceEquals(target.Content, node)) return;
        FillNode(target, entries);
    }

    private async void OnRefreshClick(object sender, RoutedEventArgs e)
    {
        try
        {
            await RefreshAsync(force: true);
        }
        catch (Exception) { } // 事件入口兜底
    }

    /// <summary>刷新：保留展开状态与选中项，重列所有已展开目录。</summary>
    private async Task RefreshAsync(bool force)
    {
        if (_rpc is null || _session is null || (_busy && !force))
        {
            return;
        }
        var sid = _session;
        var root = _root ?? ".";
        var generation = ++_generation;
        _busy = true;
        try
        {
            if (FileTree.RootNodes.Count == 0)
            {
                _busy = false;
                await ReloadCurrentAsync();
                return;
            }
            var rootNode = FileTree.RootNodes[0];
            var selectedPath = (FileTree.SelectedNode?.Content as FileNode)?.Path;
            var expanded = new List<string>();
            CollectExpanded(rootNode, expanded);
            HideStatus();
            var listed = await ListDirectoryAsync(sid, root, generation);
            if (listed is null || !IsCurrent(generation, sid))
            {
                return;
            }
            FillNode(rootNode, listed);
            // 逐层恢复展开：展开动作由内核按路径重列（不递归，深度可控）
            foreach (var path in expanded.Skip(1))
            {
                var node = FindNode(rootNode, path);
                if (node is null)
                {
                    continue;
                }
                if (node.HasUnrealizedChildren)
                {
                    await ExpandAsync(node, (FileNode)node.Content, sid, generation);
                }
                if (!IsCurrent(generation, sid)) return;
                node.IsExpanded = true;
            }
            if (_previewPath is not null)
            {
                await RefreshPreviewAsync();
            }
            if (!IsCurrent(generation, sid)) return;
            if (selectedPath is not null)
            {
                SelectNodeByPath(selectedPath);
            }
            await EnsureChangesStreamAsync(sid);
        }
        catch (Exception ex)
        {
            if (IsCurrent(generation, sid)) ShowError(MainWindow.TL("刷新失败"), ex);
        }
        finally
        {
            if (IsCurrent(generation, sid)) _busy = false;
        }
    }

    private static void CollectExpanded(TreeViewNode node, List<string> into)
    {
        if (node.Content is FileNode { IsDirectory: true, IsPlaceholder: false } fn && node.IsExpanded)
        {
            into.Add(fn.Path);
        }
        if (node.IsExpanded)
        {
            foreach (var child in node.Children)
            {
                CollectExpanded(child, into);
            }
        }
    }

    private void SelectNodeByPath(string path)
    {
        if (FileTree.RootNodes.Count == 0) return;
        var node = FindNode(FileTree.RootNodes[0], path) ?? FileTree.RootNodes[0];
        FileTree.SelectedNode = node;
    }

    private static TreeViewNode? FindNode(TreeViewNode node, string path)
    {
        foreach (var child in node.Children)
        {
            if (child.Content is FileNode fn && string.Equals(fn.Path, path, StringComparison.OrdinalIgnoreCase))
            {
                return child;
            }
            if (child.IsExpanded && child.Children.Count > 0 && FindNode(child, path) is { } deep)
            {
                return deep;
            }
        }
        return null;
    }

    // ---------------- 预览 ----------------

    private async void OnTreeItemInvoked(TreeView sender, TreeViewItemInvokedEventArgs args)
    {
        try
        {
            if (args.InvokedItem is not TreeViewNode node || node.Content is not FileNode fn || fn.IsPlaceholder)
            {
                return;
            }
            if (fn.IsDirectory)
            {
                node.IsExpanded = !node.IsExpanded; // 点目录行 = 展开/折叠（原版语义）
                return;
            }
            await ShowPreviewAsync(fn);
        }
        catch (Exception) { } // 事件入口兜底
    }

    /// <summary>
    /// 键盘可达：方向键选中的节点用 Enter/Space 打开（目录 = 展开/折叠，文件 = 预览）。
    /// TreeView 只会给鼠标点击发 ItemInvoked，纯键盘用户没有入口；这里补齐。
    /// </summary>
    private async void OnTreeKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key is not (Windows.System.VirtualKey.Enter or Windows.System.VirtualKey.Space))
        {
            return;
        }
        if (FileTree.SelectedNode is not { } node || node.Content is not FileNode fn || fn.IsPlaceholder)
        {
            return;
        }
        e.Handled = true;
        try
        {
            if (fn.IsDirectory)
            {
                node.IsExpanded = !node.IsExpanded;
                return;
            }
            await ShowPreviewAsync(fn);
        }
        catch (Exception) { }
    }

    private void OnBackClick(object sender, RoutedEventArgs e)
    {
        try
        {
            HeaderTitle.Text = "工作区文件";
            HeaderPath.Text = _root?.Replace('/', '\\') ?? "";
            SyncTreeNodeSelection(); // ShowTree 会清掉预览路径，须先恢复选中
            ShowTree();
            HideStatus();
        }
        catch (Exception) { }
    }

    private void OnCloseClick(object sender, RoutedEventArgs e)
    {
        try
        {
            CloseRequested?.Invoke();
        }
        catch (Exception) { }
    }

    /// <summary>打开一个文件的预览：stat 取版本/大小，read 取文本页（内核按行窗口与字节上限截断）。</summary>
    private async Task ShowPreviewAsync(FileNode file)
    {
        if (_rpc is null || _session is null)
        {
            return;
        }
        var sid = _session;
        ShowPreviewHost();
        HeaderTitle.Text = file.Name;
        HeaderPath.Text = file.Path;
        PreviewText.Text = "";
        PreviewMeta.Text = MainWindow.TL("正在读取…");
        HideStatus();
        _previewPath = file.Path;
        _previewVersion = null;
        var previewGeneration = ++_previewGeneration;
        try
        {
            var rpc = _rpc;
            var stat = await rpc.CallOkAsync("workspaceFiles/stat", new { workspaceFileScopeId = sid, path = file.Path });
            if (previewGeneration != _previewGeneration) return; // 已返回树/换文件/换会话：丢弃过期回写
            var version = stat.TryGetProperty("version", out var v) ? v.GetString() : null;
            long? bytes = stat.TryGetProperty("bytes", out var b) && b.ValueKind == JsonValueKind.Number ? b.GetInt64() : null;
            var read = await rpc.CallOkAsync("workspaceFiles/read", new
            {
                workspaceFileScopeId = sid,
                path = file.Path,
                range = new { offset = 1, limit = PreviewLineLimit },
            });
            if (previewGeneration != _previewGeneration) return;
            _previewVersion = read.TryGetProperty("version", out var rv) ? rv.GetString() : version;
            PreviewText.Text = read.TryGetProperty("text", out var txt) ? txt.GetString() ?? "" : "";
            var lines = read.TryGetProperty("lines", out var ln) && ln.ValueKind == JsonValueKind.Number ? ln.GetInt32() : 0;
            var eof = read.TryGetProperty("eof", out var ef) && ef.ValueKind == JsonValueKind.True;
            PreviewMeta.Text = MainWindow.TLF("{0} · {1} 行 · 版本 {2}", FormatSize(bytes), lines, Short(version));
            if (!eof)
            {
                ShowStatus(InfoBarSeverity.Informational,
                    MainWindow.TLF("文件较长，仅显示前 {0} 行（内核单页上限 {1} 行 / 2 MiB）。", lines, PreviewLineLimit));
            }
        }
        catch (Exception ex)
        {
            if (previewGeneration != _previewGeneration) return;
            // 二进制（含 NUL / 非 UTF-8）→ 明确"不支持预览"占位；其余按错误码给友好提示。
            // 这里刻意捕全部：预览失败必须是**可见**的，不能被事件入口的兜底 catch 吞掉。
            ShowPreviewPlaceholder(ex);
        }
    }

    /// <summary>变更流触发的预览重载：版本没变则不动（不打断阅读位置）。</summary>
    private async Task RefreshPreviewAsync()
    {
        if (_previewPath is null || _rpc is null || _session is null)
        {
            return;
        }
        var sid = _session;
        var path = _previewPath;
        var rpc = _rpc;
        var previewGeneration = _previewGeneration;
        try
        {
            var stat = await rpc.CallOkAsync("workspaceFiles/stat", new { workspaceFileScopeId = sid, path });
            if (previewGeneration != _previewGeneration) return;
            var version = stat.TryGetProperty("version", out var v) ? v.GetString() : null;
            if (string.Equals(version, _previewVersion, StringComparison.Ordinal))
            {
                return;
            }
            await ShowPreviewAsync(new FileNode { Name = DisplayNameOf(path), Path = path, Type = "file" });
        }
        catch (Exception ex)
        {
            if (previewGeneration != _previewGeneration) return;
            if (ex is DshRpcException { Code: "workspace-file/not-found" })
            {
                ShowPreviewPlaceholder(ex);
            }
        }
    }

    private void ShowPreviewPlaceholder(Exception ex)
    {
        var code = ex is DshRpcException rpc ? rpc.Code : "";
        var (title, detail, severity) = code switch
        {
            "workspace-file/not-text" => (MainWindow.TL("不支持预览"), MainWindow.TL("该文件是二进制或非 UTF-8 文本（含 NUL 字节），壳内只预览文本。"), InfoBarSeverity.Informational),
            "workspace-file/too-large" => (MainWindow.TL("文件过大"), MainWindow.TL("超出内核单页读取上限（2 MiB / 5000 行），无法在这里完整预览。"), InfoBarSeverity.Warning),
            "workspace-file/not-found" => (MainWindow.TL("文件不存在"), MainWindow.TL("路径已消失（可能被移动或删除）。"), InfoBarSeverity.Warning),
            "workspace-file/not-regular-file" => (MainWindow.TL("不支持预览"), MainWindow.TL("该路径不是普通文件。"), InfoBarSeverity.Informational),
            "workspace-file/outside-workspace" => (MainWindow.TL("超出工作区"), MainWindow.TL("该路径不在当前会话的工作区内。"), InfoBarSeverity.Warning),
            "gateway/lookup-not-found" => (MainWindow.TL("会话不可用"), MainWindow.TL("该会话没有活跃 agent，内核无法解析工作区。"), InfoBarSeverity.Warning),
            _ => (MainWindow.TL("读取失败"), ex.Message, InfoBarSeverity.Error),
        };
        ShowPlaceholder("\uE7BA", title, detail);
        // 占位属于"预览态"，返回钮要留在原位，否则用户没有回文件树的路径
        BackButton.Visibility = Visibility.Visible;
        ShowStatus(severity, $"{title}：{detail}");
    }

    private void SyncTreeNodeSelection()
    {
        if (FileTree.RootNodes.Count == 0)
        {
            return;
        }
        var node = _previewPath is null ? FileTree.RootNodes[0] : FindNode(FileTree.RootNodes[0], _previewPath);
        if (node is not null)
        {
            FileTree.SelectedNode = node;
        }
    }

    // ---------------- 变更流（自动刷新） ----------------

    /// <summary>订阅 workspaceFiles/changes：ready 后任何写入/删除都触发一次去抖刷新。</summary>
    private async Task EnsureChangesStreamAsync(string sid)
    {
        if (_rpc is null || _session != sid || Visibility != Visibility.Visible ||
            _changesStreamId is not null || _changesOpening)
        {
            return;
        }
        var rpc = _rpc;
        var streamGeneration = _streamGeneration;
        _changesOpening = true;
        try
        {
            var streamId = await rpc.OpenRemoteStreamAsync(
                "workspaceFiles/changes",
                new { workspaceFileScopeId = sid },
                frame => OnChangesFrame(frame, streamGeneration));
            if (streamGeneration != _streamGeneration || _session != sid || Visibility != Visibility.Visible)
            {
                rpc.CancelStream(streamId);
                return;
            }
            _changesStreamId = streamId;
        }
        catch (Exception)
        {
            // 变更流建不起来不影响浏览：退化为手动刷新。
            if (streamGeneration == _streamGeneration) _changesStreamId = null;
        }
        finally
        {
            if (streamGeneration == _streamGeneration) _changesOpening = false;
        }
    }

    private void OnStreamsReset()
    {
        DispatcherQueue.TryEnqueue(() =>
        {
            CloseChangesStream(); // 旧 id、在途开流和排队中的回调全部作废
            if (Visibility == Visibility.Visible && _session is { } sid)
            {
                _ = EnsureChangesStreamAsync(sid);
                _changesDebounce?.Start(); // 补取断连期间的变更
            }
        });
    }

    private void OnChangesFrame(JsonElement frame, long streamGeneration)
    {
        // sink 在 mux 收包线程执行：一切 UI/DispatcherTimer 操作都必须编组到 UI 线程
        var kind = frame.ValueKind == JsonValueKind.Object && frame.TryGetProperty("kind", out var k) ? k.GetString() : null;
        if (kind is not ("ready" or "change"))
        {
            return;
        }
        DispatcherQueue.TryEnqueue(() =>
        {
            if (streamGeneration != _streamGeneration || Visibility != Visibility.Visible) return;
            if (kind == "ready")
            {
                _changesReady = true;
                return;
            }
            if (!_changesReady)
            {
                // ready 前的变更也刷新一次（订阅窗口内发生的写入不丢）
                _changesReady = true;
            }
            _changesDebounce?.Stop();
            _changesDebounce?.Start();
        });
    }

    private void CloseChangesStream()
    {
        _streamGeneration++;
        _changesOpening = false;
        _changesReady = false;
        if (_changesStreamId is not null)
        {
            _rpc?.CancelStream(_changesStreamId);
            _changesStreamId = null;
        }
        _changesDebounce?.Stop();
    }

    private async Task AutoRefreshAsync()
    {
        if (_rpc is null || _session is null)
        {
            return;
        }
        if (Visibility != Visibility.Visible)
        {
            return;
        }
        await RefreshAsync(force: false);
    }

    // ---------------- 视图状态 ----------------

    private const int PreviewLineLimit = 2000;

    private void ShowTree()
    {
        _previewGeneration++;
        _previewPath = null;
        _previewVersion = null;
        BackButton.Visibility = Visibility.Collapsed;
        TreeScroll.Visibility = Visibility.Visible;
        PreviewHost.Visibility = Visibility.Collapsed;
        PlaceholderHost.Visibility = Visibility.Collapsed;
    }

    private void ShowPreviewHost()
    {
        BackButton.Visibility = Visibility.Visible;
        TreeScroll.Visibility = Visibility.Collapsed;
        PreviewHost.Visibility = Visibility.Visible;
        PlaceholderHost.Visibility = Visibility.Collapsed;
    }

    private void ShowPlaceholder(string glyph, string title, string detail)
    {
        BackButton.Visibility = Visibility.Collapsed;
        TreeScroll.Visibility = Visibility.Collapsed;
        PreviewHost.Visibility = Visibility.Collapsed;
        PlaceholderHost.Visibility = Visibility.Visible;
        PlaceholderIcon.Glyph = glyph;
        PlaceholderTitle.Text = title;
        PlaceholderDetail.Text = detail;
    }

    /// <summary>全局语言切换后重刷壳文案：FilesPanel 不在 RefreshShellLanguage 的可视树扫描范围内。</summary>
    public void ApplyLocale()
    {
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(BackButton, MainWindow.TL("返回文件树"));
        Microsoft.UI.Xaml.Controls.ToolTipService.SetToolTip(BackButton, MainWindow.TL("返回文件树"));
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(RefreshButton, MainWindow.TL("刷新文件树"));
        Microsoft.UI.Xaml.Controls.ToolTipService.SetToolTip(RefreshButton, MainWindow.TL("刷新"));
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(CloseButton, MainWindow.TL("关闭文件面板"));
        Microsoft.UI.Xaml.Controls.ToolTipService.SetToolTip(CloseButton, MainWindow.TL("关闭"));
        if (TreeScroll.Visibility == Visibility.Visible)
        {
            HeaderTitle.Text = MainWindow.TL("工作区文件");
        }
        if (PlaceholderHost.Visibility == Visibility.Visible && _session is null)
        {
            ShowPlaceholder("\uE8B7", MainWindow.TL("未选择会话"),
                MainWindow.TL("在左侧选择一个会话后，这里显示该会话工作区的文件树。"));
        }
    }

    private void ShowStatus(InfoBarSeverity severity, string message, bool closable = true)
    {
        StatusBar.Severity = severity;
        StatusBar.Message = message;
        StatusBar.IsClosable = closable;
        StatusBar.IsOpen = true;
    }

    private void HideStatus() => StatusBar.IsOpen = false;

    private void ShowError(string prefix, Exception ex)
    {
        var code = ex is DshRpcException rpc ? rpc.Code : null;
        var friendly = code switch
        {
            "workspace-file/not-found" => MainWindow.TL("路径不存在（可能已被移动或删除）。"),
            "workspace-file/not-directory" => MainWindow.TL("该路径不是目录。"),
            "workspace-file/outside-workspace" => MainWindow.TL("路径超出该会话工作区。"),
            "workspace-file/too-large" => MainWindow.TL("内容超出内核单页上限。"),
            "workspace-file/not-text" => MainWindow.TL("不是 UTF-8 文本，无法预览。"),
            "gateway/lookup-not-found" => MainWindow.TL("该会话没有活跃 agent（会话未打开或已归档）。"),
            _ => ex.Message,
        };
        ShowStatus(InfoBarSeverity.Error, $"{prefix}：{friendly}");
    }

    /// <summary>工作区根归一化：反斜杠转正斜杠、去尾斜杠（与原版 sidebar-files 的路径键同式）。</summary>
    private static string? NormalizeRoot(string? path)
    {
        if (string.IsNullOrWhiteSpace(path))
        {
            return null;
        }
        var normalized = path.Replace('\\', '/').TrimEnd('/');
        return normalized.Length == 0 ? null : normalized;
    }

    private static string JoinPath(string parent, string name) => $"{parent.TrimEnd('/')}/{name}";

    private static string DisplayNameOf(string path)
    {
        var trimmed = path.TrimEnd('/');
        var index = trimmed.LastIndexOf('/');
        var name = index >= 0 ? trimmed[(index + 1)..] : trimmed;
        return name.Length == 0 ? trimmed : name;
    }

    private static string FormatSize(long? bytes)
    {
        if (bytes is not { } b)
        {
            return MainWindow.TL("大小未知");
        }
        return b switch
        {
            < 1024 => $"{b} B",
            < 1024 * 1024 => $"{b / 1024.0:0.#} KiB",
            _ => $"{b / (1024.0 * 1024.0):0.##} MiB",
        };
    }

    private static string Short(string? version) => version is null or "" ? "-" : version[..Math.Min(8, version.Length)];
}
