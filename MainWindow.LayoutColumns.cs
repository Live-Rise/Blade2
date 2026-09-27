// P1-25 三栏拖拽把手 + P1-26 右栏分栏/全屏（对标 dsh-client-ui-layout / dsh-client-ui-sidebar-right）。
// 列宽契约（dsh-client-ui-layout/lib/client.js columns.ts）：
//   sidebar  clamp 264..420（收起时 rail 56，壳侧栏由 NavigationView 自管）
//   rightbar clamp 300..viewport*0.7，且 available = viewport - sidebar - 400 < 300 时右栏让位为 0
//   center   star，仅在无右栏轨时可压到 0
// 右栏 dock 契约（dsh-client-ui-sidebar-right stores.js）：
//   两格上限（dockPaneIds.length >= 2 时禁止再分栏）、左右/上下分栏、新标签页、
//   全屏（mode=fullscreen，覆盖内容面）、移到这里（placeTab/dropTab）、页面唯一性。
// UIA：LayoutSplitter* / RightPaneTab / FileActionMenu。

using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.Foundation;

namespace Blade2;

public sealed partial class MainWindow
{
    // ---------------- 列宽契约常量（与官方 columns.ts 同值） ----------------

    private const double SidebarMinWidth = 264;
    private const double SidebarMaxWidth = 420;
    private const double SidebarDefaultWidth = 280;
    private const double RightbarMinWidth = 300;
    private const double RightbarMaxRatio = 0.7;
    private const double CenterMinWidth = 400;
    private const double RightbarDefaultRatio = 0.45;
    private const double SplitterHitWidth = 8;

    private enum RightSplitMode
    {
        None,
        Horizontal, // 左右两格
        Vertical,   // 上下两格
    }

    /// <summary>右栏一个标签（对标 sidebar-right 的 tab 记录；Kind 决定内容面）。</summary>
    private sealed class RightPaneTab
    {
        public string Id = "";
        public string Title = "";
        public string Kind = "files"; // files | preview
        public string? Path;
        public int PaneIndex;
    }

    // ---------------- 运行时状态 ----------------

    private double _sidebarWidthPreference = SidebarDefaultWidth;
    private double _rightbarWidthPreference = -1; // <0 = 尚未定（首开取 45%）
    private bool _layoutColumnsReady;
    private bool _layoutDragging;

    private Border? _sidebarSplitter;
    private Border? _rightSplitter;
    private Grid? _rightDockRoot;
    private Grid? _rightDockBody;
    private StackPanel? _rightTabStrip;
    private FrameworkElement? _rightSplitHandle;
    private FilesPanel? _rightPane0;
    private FilesPanel? _rightPane1;
    private ContentControl? _rightPane0Host;
    private ContentControl? _rightPane1Host;

    private RightSplitMode _rightSplitMode = RightSplitMode.None;
    private bool _rightFullscreen;
    private double _rightInnerSplitRatio = 0.5;
    private readonly List<RightPaneTab> _rightTabs = new();
    private string _activeRightTabId = "tab-0";
    private int _rightTabSerial;

    // ---------------- 初始化 ----------------

    /// <summary>装配三栏把手与右栏 dock 宿主。构造末尾调用一次。</summary>
    private void InitLayoutColumns()
    {
        if (_layoutColumnsReady)
        {
            return;
        }
        _layoutColumnsReady = true;

        // 列宽偏好：侧栏沿用 OpenPaneLength（XAML 默认 264），右栏首开取 45%。
        _sidebarWidthPreference = Math.Clamp(Nav.OpenPaneLength, SidebarMinWidth, SidebarMaxWidth);

        InstallColumnSplitters();
        InitRightPaneDock();
        ApplySidebarWidth(_sidebarWidthPreference);
    }

    // ---------------- P1-25：三栏拖拽把手 ----------------

    private void InstallColumnSplitters()
    {
        // 侧栏/主栏把手：压在内容面左缘（NavigationView 内容区左边界 = 侧栏右边界）。
        _sidebarSplitter = MakeColumnSplitter(
            "LayoutSplitterSidebar",
            L("调整侧栏宽度"),
            onDrag: dx => ApplySidebarWidth(_layoutDragBaseWidth + dx, persist: false),
            onEnd: () => ApplySidebarWidth(_sidebarWidthPreference, persist: true));
        _sidebarSplitter.HorizontalAlignment = HorizontalAlignment.Left;
        _sidebarSplitter.VerticalAlignment = VerticalAlignment.Stretch;
        _sidebarSplitter.Width = SplitterHitWidth;
        _sidebarSplitter.Margin = new Thickness(-SplitterHitWidth / 2.0, 0, 0, 0);
        Canvas.SetZIndex(_sidebarSplitter, 20);
        Grid.SetRowSpan(_sidebarSplitter, 5);
        ChatPage.Children.Add(_sidebarSplitter);

        // 主栏/右栏把手：压在右栏左缘。
        _rightSplitter = MakeColumnSplitter(
            "LayoutSplitterRight",
            L("调整右栏宽度"),
            onDrag: dx => ApplyRightbarWidth(_layoutDragBaseWidth - dx, persist: false),
            onEnd: () => ApplyRightbarWidth(_rightbarWidthPreference > 0 ? _rightbarWidthPreference : CurrentRightbarWidth(), persist: true));
        _rightSplitter.HorizontalAlignment = HorizontalAlignment.Left;
        _rightSplitter.VerticalAlignment = VerticalAlignment.Stretch;
        _rightSplitter.Width = SplitterHitWidth;
        _rightSplitter.Margin = new Thickness(-SplitterHitWidth / 2.0, 0, 0, 0);
        _rightSplitter.Visibility = Visibility.Collapsed;
        Canvas.SetZIndex(_rightSplitter, 20);
        Grid.SetColumn(_rightSplitter, 1);
        Grid.SetRowSpan(_rightSplitter, 5);
        ChatPage.Children.Add(_rightSplitter);

        // 全屏时右栏铺满内容面，右把手让位；推挤态才可拖。
        RightPaneHost.SizeChanged += (_, _) => SyncLayoutSplitters();
        SizeChanged += (_, _) =>
        {
            if (_rightbarWidthPreference > 0)
            {
                ApplyRightbarWidth(_rightbarWidthPreference, persist: false);
            }
            SyncLayoutSplitters();
        };
        SyncLayoutSplitters();
    }

    private Border MakeColumnSplitter(string automationId, string name, Action<double> onDrag, Action onEnd)
    {
        var handle = new Border
        {
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
        };
        Aut(handle, automationId, name);
        handle.PointerPressed += (_, e) =>
        {
            if (e.GetCurrentPoint(handle).Properties.IsLeftButtonPressed)
            {
                handle.CapturePointer(e.Pointer);
                _layoutDragging = true;
                _layoutDragOriginX = e.GetCurrentPoint(null).Position.X;
                _layoutDragBaseWidth = automationId.EndsWith("Sidebar", StringComparison.Ordinal)
                    ? _sidebarWidthPreference
                    : CurrentRightbarWidth();
                e.Handled = true;
            }
        };
        handle.PointerMoved += (_, e) =>
        {
            if (!_layoutDragging)
            {
                return;
            }
            var dx = e.GetCurrentPoint(null).Position.X - _layoutDragOriginX;
            onDrag(dx);
        };
        handle.PointerReleased += (_, e) =>
        {
            try { handle.ReleasePointerCapture(e.Pointer); } catch (Exception) { }
            if (_layoutDragging)
            {
                _layoutDragging = false;
                onEnd();
            }
        };
        handle.PointerCaptureLost += (_, _) =>
        {
            if (_layoutDragging)
            {
                _layoutDragging = false;
                onEnd();
            }
        };
        return handle;
    }

    private double _layoutDragOriginX;
    private double _layoutDragBaseWidth;

    /// <summary>侧栏宽度：clamp 264..420，写回 NavigationView.OpenPaneLength。</summary>
    private void ApplySidebarWidth(double px, bool persist = true)
    {
        var clamped = Math.Clamp(Math.Round(px), SidebarMinWidth, SidebarMaxWidth);
        _sidebarWidthPreference = clamped;
        if (Math.Abs(Nav.OpenPaneLength - clamped) > 0.5)
        {
            Nav.OpenPaneLength = clamped;
        }
        if (persist)
        {
            SaveLayoutColumnPrefs();
        }
        SyncLayoutSplitters();
    }

    /// <summary>右栏宽度：clamp 300..min(available, viewport*0.7)；available 不足 300 时收起轨。</summary>
    private void ApplyRightbarWidth(double px, bool persist = true)
    {
        var viewport = Math.Max(0, ChatPage.ActualWidth + (RightPaneHost.Visibility == Visibility.Visible ? CurrentRightbarWidth() : 0));
        if (viewport <= 0)
        {
            viewport = Math.Max(0, RootGrid.ActualWidth - Math.Max(0, Nav.OpenPaneLength));
        }
        var available = viewport - SidebarMinWidth - CenterMinWidth;
        // 与官方同式：available < 300 时右栏轨直接为 0（不让主栏饿死）。
        if (available < RightbarMinWidth && !_rightFullscreen)
        {
            _rightbarWidthPreference = Math.Max(RightbarMinWidth, Math.Round(px));
            SyncLayoutSplitters();
            return;
        }
        var max = Math.Max(RightbarMinWidth, Math.Min(available, viewport * RightbarMaxRatio));
        var clamped = Math.Clamp(Math.Round(px), RightbarMinWidth, max);
        _rightbarWidthPreference = clamped;
        if (RightPaneHost.Visibility == Visibility.Visible && !_rightFullscreen)
        {
            RightPaneHost.Width = clamped;
            if (_rightDockRoot is not null)
            {
                _rightDockRoot.Width = clamped;
            }
        }
        if (persist)
        {
            SaveLayoutColumnPrefs();
        }
        SyncLayoutSplitters();
    }

    private double CurrentRightbarWidth()
    {
        if (_rightbarWidthPreference > 0)
        {
            return _rightbarWidthPreference;
        }
        var viewport = Math.Max(0, RootGrid.ActualWidth - Math.Max(0, Nav.OpenPaneLength));
        return Math.Max(RightbarMinWidth, Math.Round(viewport * RightbarDefaultRatio));
    }

    private void SyncLayoutSplitters()
    {
        if (_sidebarSplitter is not null)
        {
            _sidebarSplitter.Visibility = Nav.IsPaneOpen ? Visibility.Visible : Visibility.Collapsed;
        }
        if (_rightSplitter is not null)
        {
            _rightSplitter.Visibility =
                RightPaneHost.Visibility == Visibility.Visible && !_rightFullscreen
                    ? Visibility.Visible
                    : Visibility.Collapsed;
        }
        if (RightPaneHost.Visibility == Visibility.Visible && !_rightFullscreen && _rightbarWidthPreference > 0)
        {
            RightPaneHost.Width = _rightbarWidthPreference;
        }
        else if (_rightFullscreen)
        {
            RightPaneHost.Width = double.NaN;
            RightPaneHost.HorizontalAlignment = HorizontalAlignment.Stretch;
        }
    }

    private void SaveLayoutColumnPrefs()
    {
        try
        {
            var path = LayoutPrefsPath();
            var json = $$"""
                {"sidebar":{{_sidebarWidthPreference.ToString(System.Globalization.CultureInfo.InvariantCulture)}},"rightbar":{{(_rightbarWidthPreference > 0 ? _rightbarWidthPreference : 0).ToString(System.Globalization.CultureInfo.InvariantCulture)}}}
                """;
            System.IO.File.WriteAllText(path, json);
        }
        catch (Exception)
        {
            // 列宽偏好写盘失败不影响布局本身
        }
    }

    private static string LayoutPrefsPath()
    {
        var dir = System.IO.Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Blade2");
        System.IO.Directory.CreateDirectory(dir);
        return System.IO.Path.Combine(dir, "layout-columns.json");
    }

    private void LoadLayoutColumnPrefs()
    {
        try
        {
            var path = LayoutPrefsPath();
            if (!System.IO.File.Exists(path))
            {
                return;
            }
            using var doc = System.Text.Json.JsonDocument.Parse(System.IO.File.ReadAllText(path));
            var root = doc.RootElement;
            if (root.TryGetProperty("sidebar", out var s) && s.ValueKind == System.Text.Json.JsonValueKind.Number)
            {
                _sidebarWidthPreference = Math.Clamp(s.GetDouble(), SidebarMinWidth, SidebarMaxWidth);
                Nav.OpenPaneLength = _sidebarWidthPreference;
            }
            if (root.TryGetProperty("rightbar", out var r) && r.ValueKind == System.Text.Json.JsonValueKind.Number && r.GetDouble() > 0)
            {
                _rightbarWidthPreference = Math.Max(RightbarMinWidth, r.GetDouble());
            }
        }
        catch (Exception)
        {
            // 偏好损坏按默认值起
        }
    }

    // ---------------- P1-26：右栏分栏 / 全屏 / 标签 ----------------

    private void InitRightPaneDock()
    {
        LoadLayoutColumnPrefs();

        _rightDockRoot = new Grid { RowSpacing = 0 };
        _rightDockRoot.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        _rightDockRoot.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });

        _rightTabStrip = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 2,
            Padding = new Thickness(6, 4, 6, 0),
        };
        AutomationProperties.SetAutomationId(_rightTabStrip, "RightPaneTabStrip");
        AutomationProperties.SetName(_rightTabStrip, L("右栏标签栏"));
        Grid.SetRow(_rightTabStrip, 0);
        _rightDockRoot.Children.Add(_rightTabStrip);

        // 标签栏尾部 chrome：新标签页 + 全屏切换（对标 PanelChrome）。
        var chrome = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 2 };
        var addTab = MakeDockIconButton("\uE710", "RightPaneAddTab", L("新标签页"), OnRightPaneAddTab);
        var fullscreen = MakeDockIconButton("\uE740", "RightPaneFullscreen", L("全屏"), OnRightPaneToggleFullscreen);
        fullscreen.Tag = "fullscreen-chrome";
        chrome.Children.Add(addTab);
        chrome.Children.Add(fullscreen);
        _rightTabStrip.Children.Add(chrome);

        _rightDockBody = new Grid { Name = "RightPaneDockBody" };
        AutomationProperties.SetAutomationId(_rightDockBody, "RightPaneDockBody");
        AutomationProperties.SetName(_rightDockBody, L("右栏分栏宿主"));
        Grid.SetRow(_rightDockBody, 1);
        _rightDockRoot.Children.Add(_rightDockBody);

        // 把 XAML 里的主文件面板收编为 0 号格内容。
        // 必须先从 XAML 父级摘下：ContentControl.Content 拒绝仍在可视树里的元素
        // （WinUI 抛 ArgumentException: Value does not fall within the expected range）。
        _rightPane0 = FilesPanelView;
        RightPaneHost.Children.Clear();
        _rightPane0Host = new ContentControl
        {
            Content = _rightPane0,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            VerticalContentAlignment = VerticalAlignment.Stretch,
        };
        AutomationProperties.SetAutomationId(_rightPane0Host, "RightPaneCell0");
        AutomationProperties.SetName(_rightPane0Host, L("右栏第一格"));
        _rightDockBody.Children.Add(_rightPane0Host);

        // 默认一个「工作区文件」标签。
        _rightTabs.Add(new RightPaneTab
        {
            Id = "tab-0",
            Title = L("工作区文件"),
            Kind = "files",
            PaneIndex = 0,
        });
        _activeRightTabId = "tab-0";
        RebuildRightTabStrip();

        // dock 根接管右栏宽度；FilesPanel 自身不再定宽。
        RightPaneHost.Children.Add(_rightDockRoot);
        _rightPane0.StretchToHost();
    }

    private Button MakeDockIconButton(string glyph, string automationId, string name, RoutedEventHandler onClick)
    {
        var btn = new Button
        {
            Content = new FontIcon { Glyph = glyph, FontSize = 12 },
            Width = 28,
            Height = 28,
            MinWidth = 0,
            Padding = new Thickness(0),
            CornerRadius = new CornerRadius(14),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
        };
        Aut(btn, automationId, name);
        ToolTipService.SetToolTip(btn, name);
        btn.Click += onClick;
        return btn;
    }

    private void RebuildRightTabStrip()
    {
        if (_rightTabStrip is null)
        {
            return;
        }
        // 保留尾部 chrome（最后两个子级）
        var chrome = _rightTabStrip.Children.Skip(Math.Max(0, _rightTabStrip.Children.Count - 2)).ToList();
        _rightTabStrip.Children.Clear();

        foreach (var tab in _rightTabs)
        {
            var active = tab.Id == _activeRightTabId;
            var chip = new Button
            {
                Content = tab.Title,
                MinWidth = 0,
                MaxWidth = 140,
                Padding = new Thickness(10, 4, 10, 4),
                Margin = new Thickness(0, 0, 2, 0),
                FontSize = 12,
                CornerRadius = new CornerRadius(6),
                Background = active ? ThemeBrush("CardBrush") : new SolidColorBrush(Microsoft.UI.Colors.Transparent),
                BorderBrush = ThemeBrush(active ? "AccentBrush" : "StrokeSubtleBrush"),
                BorderThickness = new Thickness(1),
                HorizontalContentAlignment = HorizontalAlignment.Left,
            };
            Aut(chip, $"RightPaneTab:{tab.Id}", tab.Title);
            chip.Click += (_, _) => ActivateRightTab(tab.Id);
            chip.RightTapped += (_, e) =>
            {
                ShowRightTabMenu(tab, chip);
                e.Handled = true;
            };
            _rightTabStrip.Children.Add(chip);
        }

        foreach (var c in chrome)
        {
            _rightTabStrip.Children.Add(c);
        }

        // 全屏 chrome 图标文案随态切换
        foreach (var c in _rightTabStrip.Children.OfType<Button>())
        {
            if (AutomationProperties.GetAutomationId(c) == "RightPaneFullscreen")
            {
                var label = _rightFullscreen ? L("退出全屏") : L("全屏");
                ToolTipService.SetToolTip(c, label);
                AutomationProperties.SetName(c, label);
                if (c.Content is FontIcon icon)
                {
                    icon.Glyph = _rightFullscreen ? "\uE73F" : "\uE740";
                }
            }
        }
    }

    private void ShowRightTabMenu(RightPaneTab tab, FrameworkElement anchor)
    {
        var flyout = new MenuFlyout();
        var canSplit = _rightSplitMode == RightSplitMode.None && _rightTabs.Count > 0;

        var openItem = new MenuFlyoutItem { Text = L("打开"), Tag = "open" };
        Aut(openItem, "FileActionMenuOpen", L("打开"));
        openItem.Click += (_, _) => ActivateRightTab(tab.Id);

        var previewItem = new MenuFlyoutItem { Text = L("在侧边栏预览"), Tag = "preview" };
        Aut(previewItem, "FileActionMenuPreview", L("在侧边栏预览"));
        previewItem.Click += (_, _) => _ = OpenPathInRightPaneAsync(tab.Path ?? tab.Title, preferPreview: true);

        var splitH = new MenuFlyoutItem
        {
            Text = L("左右分栏"),
            Tag = "split-h",
            IsEnabled = canSplit,
        };
        Aut(splitH, "RightPaneSplitHorizontal", L("左右分栏"));
        if (!canSplit)
        {
            splitH.Text = L("分栏已满（最多两格）");
        }
        splitH.Click += (_, _) => SplitRightPane(RightSplitMode.Horizontal);

        var splitV = new MenuFlyoutItem
        {
            Text = L("上下分栏"),
            Tag = "split-v",
            IsEnabled = canSplit,
        };
        Aut(splitV, "RightPaneSplitVertical", L("上下分栏"));
        if (!canSplit)
        {
            splitV.Text = L("分栏已满（最多两格）");
        }
        splitV.Click += (_, _) => SplitRightPane(RightSplitMode.Vertical);

        var newTab = new MenuFlyoutItem { Text = L("新标签页"), Tag = "new-tab" };
        Aut(newTab, "RightPaneNewTab", L("新标签页"));
        newTab.Click += (_, _) => OnRightPaneAddTab(this, new RoutedEventArgs());

        var moveHere = new MenuFlyoutItem
        {
            Text = L("移到这里"),
            Tag = "move-here",
            IsEnabled = _rightSplitMode != RightSplitMode.None && tab.PaneIndex != TargetMovePane(),
        };
        Aut(moveHere, "RightPaneMoveHere", L("移到这里"));
        moveHere.Click += (_, _) => MoveRightTabHere(tab.Id);

        var full = new MenuFlyoutItem { Text = _rightFullscreen ? L("退出全屏") : L("全屏"), Tag = "fullscreen" };
        Aut(full, "RightPaneFullscreenItem", full.Text);
        full.Click += (_, _) => OnRightPaneToggleFullscreen(this, new RoutedEventArgs());

        var close = new MenuFlyoutItem { Text = L("关闭"), Tag = "close" };
        Aut(close, "RightPaneCloseTab", L("关闭"));
        close.IsEnabled = _rightTabs.Count > 1;
        close.Click += (_, _) => CloseRightTab(tab.Id);

        // FileActionMenu 语义挂在分栏菜单上（UIA 契约：文件/分栏动作共用此 id 族）。
        AutomationProperties.SetAutomationId(flyout, "FileActionMenu");
        flyout.Items.Add(openItem);
        flyout.Items.Add(previewItem);
        flyout.Items.Add(new MenuFlyoutSeparator());
        flyout.Items.Add(splitH);
        flyout.Items.Add(splitV);
        flyout.Items.Add(newTab);
        flyout.Items.Add(moveHere);
        flyout.Items.Add(new MenuFlyoutSeparator());
        flyout.Items.Add(full);
        flyout.Items.Add(close);
        flyout.ShowAt(anchor);
    }

    private int TargetMovePane() => _rightSplitMode == RightSplitMode.None ? 0 : 1;

    private void ActivateRightTab(string id)
    {
        _activeRightTabId = id;
        RebuildRightTabStrip();
        SyncRightPaneContent();
    }

    private void OnRightPaneAddTab(object sender, RoutedEventArgs e)
    {
        // 官方 addTab：往当前格塞一个 guide/默认页。壳侧默认再开一个文件树标签。
        var pane = _rightSplitMode == RightSplitMode.None ? 0 : ActiveTabPane();
        var id = $"tab-{++_rightTabSerial}";
        _rightTabs.Add(new RightPaneTab
        {
            Id = id,
            Title = TLF("文件 {0}", _rightTabs.Count + 1),
            Kind = "files",
            PaneIndex = pane,
        });
        _activeRightTabId = id;
        EnsureRightPaneInstance(pane);
        RebuildRightTabStrip();
        SyncRightPaneContent();
        SetFilesPanelOpen(true);
    }

    private void OnRightPaneToggleFullscreen(object sender, RoutedEventArgs e)
    {
        _rightFullscreen = !_rightFullscreen;
        // 官方：fullscreen 覆盖整个 frame；推挤态保留轨道宽。
        if (_rightFullscreen)
        {
            Grid.SetColumn(RightPaneHost, 0);
            Grid.SetColumnSpan(RightPaneHost, 2);
            RightPaneHost.HorizontalAlignment = HorizontalAlignment.Stretch;
            RightPaneHost.Width = double.NaN;
            if (_rightDockRoot is not null)
            {
                _rightDockRoot.Width = double.NaN;
                _rightDockRoot.HorizontalAlignment = HorizontalAlignment.Stretch;
            }
        }
        else
        {
            Grid.SetColumn(RightPaneHost, 1);
            Grid.SetColumnSpan(RightPaneHost, 1);
            RightPaneHost.HorizontalAlignment = HorizontalAlignment.Stretch;
            if (_rightbarWidthPreference <= 0)
            {
                _rightbarWidthPreference = Math.Max(RightbarMinWidth, CurrentRightbarWidth());
            }
            RightPaneHost.Width = _rightbarWidthPreference;
            if (_rightDockRoot is not null)
            {
                _rightDockRoot.Width = _rightbarWidthPreference;
            }
        }
        RebuildRightTabStrip();
        SyncLayoutSplitters();
        SetFilesPanelOpen(true);
    }

    /// <summary>两格上限：None→目标方向开第二格；已分栏则拒绝（官方 splitPane 的 dockPaneIds>=2 短路）。</summary>
    private void SplitRightPane(RightSplitMode mode)
    {
        if (_rightSplitMode != RightSplitMode.None || mode == RightSplitMode.None)
        {
            return; // 两格上限
        }
        _rightSplitMode = mode;
        _rightInnerSplitRatio = 0.5;
        EnsureRightPaneInstance(1);

        // 新标签落到第二格（官方 splitPane 的 settled 回调：给新 pane 种默认页）。
        var id = $"tab-{++_rightTabSerial}";
        _rightTabs.Add(new RightPaneTab
        {
            Id = id,
            Title = TLF("文件 {0}", _rightTabs.Count + 1),
            Kind = "files",
            PaneIndex = 1,
        });
        _activeRightTabId = id;

        RebuildRightDockBody();
        RebuildRightTabStrip();
        SyncRightPaneContent();
        SetFilesPanelOpen(true);
    }

    private void MoveRightTabHere(string tabId)
    {
        if (_rightSplitMode == RightSplitMode.None)
        {
            return;
        }
        var tab = _rightTabs.FirstOrDefault(t => t.Id == tabId);
        if (tab is null)
        {
            return;
        }
        var target = TargetMovePane();
        if (tab.PaneIndex == target)
        {
            return;
        }
        // 同 kind 页面唯一性：目标格已有同 kind 时合并（官方 arriving()）。
        var existing = _rightTabs.FirstOrDefault(t => t.PaneIndex == target && t.Kind == tab.Kind);
        if (existing is not null && existing.Id != tab.Id)
        {
            _rightTabs.RemoveAll(t => t.Id == tab.Id);
            _activeRightTabId = existing.Id;
        }
        else
        {
            tab.PaneIndex = target;
            _activeRightTabId = tab.Id;
        }
        RebuildRightTabStrip();
        SyncRightPaneContent();
    }

    private void CloseRightTab(string id)
    {
        if (_rightTabs.Count <= 1)
        {
            return;
        }
        var tab = _rightTabs.FirstOrDefault(t => t.Id == id);
        if (tab is null)
        {
            return;
        }
        _rightTabs.Remove(tab);
        // 空格合并：官方 settle —— 展开态不留空 pane。
        if (_rightSplitMode != RightSplitMode.None)
        {
            var pane0 = _rightTabs.Count(t => t.PaneIndex == 0);
            var pane1 = _rightTabs.Count(t => t.PaneIndex == 1);
            if (pane0 == 0 || pane1 == 0)
            {
                _rightSplitMode = RightSplitMode.None;
                foreach (var t in _rightTabs)
                {
                    t.PaneIndex = 0;
                }
                DetachRightPaneInstance(1);
                RebuildRightDockBody();
            }
        }
        if (_activeRightTabId == id)
        {
            _activeRightTabId = _rightTabs[0].Id;
        }
        RebuildRightTabStrip();
        SyncRightPaneContent();
    }

    private int ActiveTabPane()
    {
        var tab = _rightTabs.FirstOrDefault(t => t.Id == _activeRightTabId);
        return tab?.PaneIndex ?? 0;
    }

    private void EnsureRightPaneInstance(int paneIndex)
    {
        if (paneIndex == 0)
        {
            _rightPane0 ??= FilesPanelView;
            _rightPane0Host ??= new ContentControl
            {
                Content = _rightPane0,
                HorizontalContentAlignment = HorizontalAlignment.Stretch,
                VerticalContentAlignment = VerticalAlignment.Stretch,
            };
            return;
        }
        if (_rightPane1 is not null)
        {
            return;
        }
        _rightPane1 = new FilesPanel();
        Aut(_rightPane1, "FilesPanelPane1", L("右栏第二格文件面板"));
        _rightPane1.Attach(
            _rpc ?? throw new InvalidOperationException("rpc"),
            () => Volatile.Read(ref _activeSessionId),
            () => _sessions.FirstOrDefault(s => s.SessionId == Volatile.Read(ref _activeSessionId))?.Cwd);
        _rightPane1.CloseRequested += () =>
        {
            // 第二格关闭 = 取消分栏（回到单格）
            if (_rightSplitMode != RightSplitMode.None)
            {
                _rightSplitMode = RightSplitMode.None;
                foreach (var t in _rightTabs)
                {
                    t.PaneIndex = 0;
                }
                DetachRightPaneInstance(1);
                RebuildRightDockBody();
                RebuildRightTabStrip();
                SyncRightPaneContent();
            }
        };
        _rightPane1.StretchToHost();
        _rightPane1Host = new ContentControl
        {
            Content = _rightPane1,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            VerticalContentAlignment = VerticalAlignment.Stretch,
        };
        AutomationProperties.SetAutomationId(_rightPane1Host, "RightPaneCell1");
        AutomationProperties.SetName(_rightPane1Host, L("右栏第二格"));
    }

    private void DetachRightPaneInstance(int paneIndex)
    {
        if (paneIndex != 1)
        {
            return;
        }
        if (_rightPane1 is not null)
        {
            _rightPane1.OnVisibilityChanged(false);
            _rightPane1 = null;
        }
        _rightPane1Host = null;
    }

    private void RebuildRightDockBody()
    {
        if (_rightDockBody is null)
        {
            return;
        }
        _rightDockBody.Children.Clear();
        _rightDockBody.ColumnDefinitions.Clear();
        _rightDockBody.RowDefinitions.Clear();
        _rightSplitHandle = null;

        if (_rightSplitMode == RightSplitMode.None)
        {
            EnsureRightPaneInstance(0);
            if (_rightPane0Host is not null)
            {
                _rightDockBody.Children.Add(_rightPane0Host);
            }
            return;
        }

        EnsureRightPaneInstance(0);
        EnsureRightPaneInstance(1);

        var handle = new Border
        {
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
        };
        Aut(handle, "LayoutSplitterRightInner", L("调整分栏比例"));
        WireInnerSplitDrag(handle);
        _rightSplitHandle = handle;

        if (_rightSplitMode == RightSplitMode.Horizontal)
        {
            _rightDockBody.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(_rightInnerSplitRatio, GridUnitType.Star) });
            _rightDockBody.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            _rightDockBody.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1 - _rightInnerSplitRatio, GridUnitType.Star) });
            if (_rightPane0Host is not null)
            {
                Grid.SetColumn(_rightPane0Host, 0);
                _rightDockBody.Children.Add(_rightPane0Host);
            }
            handle.Width = SplitterHitWidth;
            handle.Margin = new Thickness(-SplitterHitWidth / 2.0, 0, -SplitterHitWidth / 2.0, 0);
            Grid.SetColumn(handle, 1);
            _rightDockBody.Children.Add(handle);
            if (_rightPane1Host is not null)
            {
                Grid.SetColumn(_rightPane1Host, 2);
                _rightDockBody.Children.Add(_rightPane1Host);
            }
        }
        else
        {
            _rightDockBody.RowDefinitions.Add(new RowDefinition { Height = new GridLength(_rightInnerSplitRatio, GridUnitType.Star) });
            _rightDockBody.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            _rightDockBody.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1 - _rightInnerSplitRatio, GridUnitType.Star) });
            if (_rightPane0Host is not null)
            {
                Grid.SetRow(_rightPane0Host, 0);
                _rightDockBody.Children.Add(_rightPane0Host);
            }
            handle.Height = SplitterHitWidth;
            handle.Margin = new Thickness(0, -SplitterHitWidth / 2.0, 0, -SplitterHitWidth / 2.0);
            Grid.SetRow(handle, 1);
            _rightDockBody.Children.Add(handle);
            if (_rightPane1Host is not null)
            {
                Grid.SetRow(_rightPane1Host, 2);
                _rightDockBody.Children.Add(_rightPane1Host);
            }
        }
    }

    private void WireInnerSplitDrag(Border handle)
    {
        var dragging = false;
        Point origin = default;
        double baseRatio = 0.5;
        handle.PointerPressed += (_, e) =>
        {
            if (!e.GetCurrentPoint(handle).Properties.IsLeftButtonPressed)
            {
                return;
            }
            handle.CapturePointer(e.Pointer);
            dragging = true;
            origin = e.GetCurrentPoint(null).Position;
            baseRatio = _rightInnerSplitRatio;
            e.Handled = true;
        };
        handle.PointerMoved += (_, e) =>
        {
            if (!dragging)
            {
                return;
            }
            var p = e.GetCurrentPoint(null).Position;
            if (_rightSplitMode == RightSplitMode.Horizontal && _rightDockBody is not null && _rightDockBody.ActualWidth > 0)
            {
                var dx = p.X - origin.X;
                _rightInnerSplitRatio = Math.Clamp(baseRatio + dx / _rightDockBody.ActualWidth, 0.2, 0.8);
                if (_rightDockBody.ColumnDefinitions.Count >= 3)
                {
                    _rightDockBody.ColumnDefinitions[0].Width = new GridLength(_rightInnerSplitRatio, GridUnitType.Star);
                    _rightDockBody.ColumnDefinitions[2].Width = new GridLength(1 - _rightInnerSplitRatio, GridUnitType.Star);
                }
            }
            else if (_rightSplitMode == RightSplitMode.Vertical && _rightDockBody is not null && _rightDockBody.ActualHeight > 0)
            {
                var dy = p.Y - origin.Y;
                _rightInnerSplitRatio = Math.Clamp(baseRatio + dy / _rightDockBody.ActualHeight, 0.2, 0.8);
                if (_rightDockBody.RowDefinitions.Count >= 3)
                {
                    _rightDockBody.RowDefinitions[0].Height = new GridLength(_rightInnerSplitRatio, GridUnitType.Star);
                    _rightDockBody.RowDefinitions[2].Height = new GridLength(1 - _rightInnerSplitRatio, GridUnitType.Star);
                }
            }
        };
        handle.PointerReleased += (_, e) =>
        {
            try { handle.ReleasePointerCapture(e.Pointer); } catch (Exception) { }
            dragging = false;
        };
    }

    private void SyncRightPaneContent()
    {
        if (_rightPane0 is null)
        {
            return;
        }
        var active = _rightTabs.FirstOrDefault(t => t.Id == _activeRightTabId) ?? _rightTabs.FirstOrDefault();
        if (active is null)
        {
            return;
        }
        // 当前激活标签所在格的面板显示其路径（files 树 / preview 共用 FilesPanel）。
        var pane = active.PaneIndex == 1 ? _rightPane1 : _rightPane0;
        if (pane is null)
        {
            return;
        }
        if (active.Kind == "preview" && active.Path is { Length: > 0 } path)
        {
            _ = pane.OpenPathInPreviewAsync(path);
        }
        if (_rightPane0 is not null)
        {
            _rightPane0.OnVisibilityChanged(RightPaneHost.Visibility == Visibility.Visible);
        }
        if (_rightPane1 is not null)
        {
            _rightPane1.OnVisibilityChanged(RightPaneHost.Visibility == Visibility.Visible && _rightSplitMode != RightSplitMode.None);
        }
    }

    /// <summary>从交付物/文件菜单「在侧边栏预览」进右栏；无预览标签时就地开一个。</summary>
    private async System.Threading.Tasks.Task OpenPathInRightPaneAsync(string path, bool preferPreview)
    {
        SetFilesPanelOpen(true);
        var paneIndex = ActiveTabPane();
        EnsureRightPaneInstance(paneIndex);
        var pane = paneIndex == 1 ? _rightPane1 : _rightPane0;
        if (pane is null)
        {
            return;
        }
        var tab = _rightTabs.FirstOrDefault(t => t.Id == _activeRightTabId);
        if (tab is not null && preferPreview)
        {
            tab.Kind = "preview";
            tab.Path = path;
            tab.Title = System.IO.Path.GetFileName(path.Replace('\\', '/'));
            RebuildRightTabStrip();
        }
        await pane.OpenPathInPreviewAsync(path);
    }

    /// <summary>交付物/产出文件统一文件动作入口（P1-21）。</summary>
    /// <param name="action">open | defaultApp | folder | preview | reveal</param>
    internal async System.Threading.Tasks.Task RunFileActionAsync(string path, string action, PresentedFileVm? presented = null)
    {
        switch (action)
        {
            case "preview":
                await OpenPathInRightPaneAsync(path, preferPreview: true);
                return;
            case "open":
            case "defaultApp":
                if (presented is not null)
                {
                    await OpenPresentedFileAsync(presented, "open");
                }
                else
                {
                    await OpenProducedPathAsync(path);
                }
                return;
            case "reveal":
                if (presented is not null)
                {
                    await OpenPresentedFileAsync(presented, "reveal");
                }
                else
                {
                    await OpenProducedPathAsync(path, reveal: true);
                }
                return;
            case "folder":
                var dir = System.IO.Path.GetDirectoryName(path.Replace('/', '\\'));
                if (string.IsNullOrEmpty(dir))
                {
                    dir = path;
                }
                if (presented is not null)
                {
                    // 交付物没有独立 folder 动作：走 reveal（官方 fileManager=directory 时同一动作）。
                    await OpenPresentedFileAsync(presented, "reveal");
                }
                else
                {
                    await OpenProducedPathAsync(dir);
                }
                return;
        }
    }
}
