using System;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.WindowsRuntime;
using System.Text.Json;
using System.Threading.Tasks;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Foundation;
using Windows.Graphics;

namespace Blade2.Dsh;

/// <summary>
/// 桌面宠物的独立窗口：与主窗口平级的一个置顶、无边框、透明窗口，只养一只精灵。
///
/// 为什么必须是独立窗口而不是主窗口里的一层：插件把宠物定义为 host-global 的悬浮面
/// （/api/pet/* 没有 session 维度，插件 issue #48），用户切到别的应用时它仍得在。
/// 画在主窗口里，主窗口一被盖住/最小化宠物就跟着消失——那不是桌面宠物。
/// 插件自己没有开窗能力（只有 HTTP API，且位置明确交给壳自绘、不持久化），
/// 所以这一层只能由壳来开。
///
/// 渲染复用 <see cref="DshPetSprite"/>：它只依赖「一个视口 Grid + 一个 Image」，
/// 与载体是主窗口还是本窗口无关，图集帧动画逻辑一行不改。
///
/// 本窗口自己不出现在桌上：合成岛那层关不掉的白决定了它只能当离屏内容源，
/// 像素由 <see cref="PetSurfaceLayer"/> 搬进逐像素透明的分层窗口；拖动/点击的
/// 原始鼠标消息也走那一层（WS_EX_NOACTIVATE，不抢当前应用焦点）。
/// </summary>
public sealed partial class PetWindow : Window
{
    // ---------------- Win32 常量 ----------------
    private const int GwlExStyle = -20;
    private const int WsExTopmost = 0x00000008;      // 置顶：盖在别的应用之上
    private const int WsExToolWindow = 0x00000080;   // 不进任务栏与 Alt-Tab
    private const int WsExNoActivate = 0x08000000;   // 点宠物不抢当前应用的焦点

    private const int SmXVirtualScreen = 76;
    private const int SmYVirtualScreen = 77;
    private const int SmCxVirtualScreen = 78;
    private const int SmCyVirtualScreen = 79;

    private readonly string _statePath;
    private IntPtr _hwnd;
    private bool _chromeReady;
    private bool _activated;
    private SizeInt32 _lastSize;

    /// <summary>
    /// 真正的显示面。XAML 这棵树只当离屏内容源：合成岛的根视觉有一层谁关不掉的不透明白，
    /// 所以像素得搬到逐像素透明的分层窗口里去（见 <see cref="PetSurfaceLayer"/>）。
    /// </summary>
    private PetSurfaceLayer? _surface;

    private bool _fitting;

    public PetWindow(string statePath)
    {
        _statePath = statePath ?? "";
        InitializeComponent();
        Sprite = new DshPetSprite(PetSpriteViewport, PetSprite);
        // 每帧图集换页、精灵改尺寸都会改变内容，都得补一次分层窗口的像素
        Sprite.FrameApplied += () => _surface?.Invalidate();
        // 精灵改尺寸会改变内容自然大小，窗口得跟着长缩
        PetSpriteViewport.SizeChanged += (_, _) => FitWindow();
        Closed += (_, _) => TeardownChrome();
    }

    /// <summary>图集帧动画宿主。与主窗口方案是同一个类，载体换成了本窗口。</summary>
    public DshPetSprite Sprite { get; }

    /// <summary>用户点了宠物（按下+抬起且位移小于阈值）。喂什么互动由壳决定。</summary>
    public event Func<Task>? InteractRequested;

    // ---------------- 显隐 ----------------

    /// <summary>显示宠物。首次显示时才建窗口外观并恢复上次位置。</summary>
    public void ShowPet()
    {
        try
        {
            if (!_chromeReady)
            {
                SetupChrome();
                FitWindow();     // 先量出内容尺寸，默认位置/贴边才算得准
                RestorePosition();
            }
            if (_surface is { Ready: true } surface)
            {
                // 分层窗口才是给人看的那一层：本窗口的合成岛留着只为了离屏出图
                surface.Show();
                HideXamlWindow();
            }
            else if (!_activated)
            {
                // 分层窗口没建起来（极端情况）：退回直接显示 XAML 窗，至少有宠物可见
                Activate();
                _activated = true;
            }
            else
            {
                AppWindow.Show();
            }
            // 别的应用也可能抢置顶：每次显示都补一次 Z 序。
            // 只碰分层窗口：本窗口是隐藏的离屏内容源，谁再把它置顶/显示一次，
            // 合成岛那层不透明白就会盖在宠物上面——桌面宠物直接变成一只白块。
            _surface?.BringToFront();
            FitWindow();
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[pet] show failed: {ex}");
        }
    }

    /// <summary>收回宠物（插件说不可见/内核不在）。帧定时器一起停，省 CPU。</summary>
    public void HidePet()
    {
        try
        {
            _surface?.Hide();
        }
        catch (Exception)
        {
        }
        Sprite.Stop();
    }

    /// <summary>XAML 窗只是离屏内容源，不能出现在桌上（合成岛那层白会露出来）。</summary>
    private void HideXamlWindow()
    {
        try
        {
            AppWindow.Hide();
        }
        catch (Exception)
        {
            // 还没有 AppWindow 可用时本来也看不见
        }
        // AppWindow.Hide 只是走一遍 presentr 的显隐；hwnd 这层再按一次，
        // 免得 presentr 状态和真实 WS_VISIBLE 不一致，白底又浮回来。
        if (_hwnd != IntPtr.Zero) ShowWindow(_hwnd, 0 /*SW_HIDE*/);
    }

    /// <summary>跟随主窗口的深浅主题（换肤时由壳调用）。</summary>
    public void SetTheme(bool dark)
    {
        try
        {
            PetRoot.RequestedTheme = dark ? ElementTheme.Dark : ElementTheme.Light;
            PetRoot.UpdateLayout();
        }
        catch (Exception)
        {
            // 主题跟着主窗口走即可，本窗口配不上不至于不显示
        }
        _surface?.Invalidate();
    }

    // ---------------- 窗口外观 ----------------

    /// <summary>
    /// 无题栏无边框 + 透明内容 + 置顶/工具窗/不抢焦点。
    /// 在 <see cref="InitializeComponent"/> 之后调用：hwnd 此刻已可用
    /// （MainWindow 的 InstallShellIntegration 同样是构造期取 hwnd）。
    /// </summary>
    private void SetupChrome()
    {
        if (_chromeReady) return;
        try
        {
            _hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            Debug.WriteLine($"[pet] chrome hwnd=0x{_hwnd.ToInt64():X}");

            if (AppWindow.Presenter is OverlappedPresenter op)
            {
                // 没有题栏就没有最小化/最大化/关闭按钮，桌面上只留一只宠物
                op.SetBorderAndTitleBar(false, false);
                op.IsResizable = false;
                op.IsMaximizable = false;
                op.IsMinimizable = false;
            }

            ExtendsContentIntoTitleBar = true;   // 内容铺满整个窗口（没有题栏可让）
            MakeTitleBarTransparent();            // 题栏区就是整个客户区，底色不透明会按主题糊一层白

            var style = GetWindowLong(_hwnd, GwlExStyle).ToInt32();
            SetWindowLong(_hwnd, GwlExStyle,
                new IntPtr(style | WsExTopmost | WsExToolWindow | WsExNoActivate));

            // 桌面上看到的其实是分层窗口；本窗口从此只当离屏内容源，不能再被人看见
            _surface = new PetSurfaceLayer(CaptureAsync, () => InteractRequested?.Invoke() ?? Task.CompletedTask);
            _surface.DragEnded += SavePosition;
            HideXamlWindow();
            _chromeReady = true;
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[pet] chrome setup failed: {ex}");
        }
    }

    /// <summary>
    /// 把已经排布好的 XAML 内容栅格化成 BGRA8 预乘像素，交给分层窗口。
    /// 这里必须显式 Measure + Arrange：本窗口是隐藏的，没人替它走布局。
    /// </summary>
    private async Task<PetSurfaceFrame?> CaptureAsync()
    {
        PetStack.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        PetStack.Arrange(new Rect(0, 0, PetStack.DesiredSize.Width, PetStack.DesiredSize.Height));
        PetRoot.UpdateLayout();

        var rtb = new Microsoft.UI.Xaml.Media.Imaging.RenderTargetBitmap();
        await rtb.RenderAsync(PetStack);
        var pixels = (await rtb.GetPixelsAsync()).ToArray();

        var w = rtb.PixelWidth;
        var h = rtb.PixelHeight;
        if (w < 8 || h < 8 || pixels.Length < w * h * 4) return null;

        return new PetSurfaceFrame { Pixels = pixels, Width = w, Height = h };
    }

    /// <summary>题栏/按钮底色全透明：题栏区就是整个客户区。</summary>
    private void MakeTitleBarTransparent()
    {
        try
        {
            var tb = AppWindow.TitleBar;
            var c = Microsoft.UI.Colors.Transparent;
            tb.BackgroundColor = c;
            tb.InactiveBackgroundColor = c;
            // caption 按钮底色一并透明：SetBorderAndTitleBar(false,false) 通常已把
            // 最小化/最大化/关闭钮去掉，万一哪条路径上按钮还在，默认按钮底色会在
            // 角落糊一块不透明白。
            tb.ButtonBackgroundColor = c;
            tb.ButtonInactiveBackgroundColor = c;
            tb.ButtonHoverBackgroundColor = c;
            tb.ButtonPressedBackgroundColor = c;
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[pet] titlebar transparent failed: {ex}");
        }
    }

    private void TeardownChrome()
    {
        try
        {
            SavePosition();
        }
        catch (Exception)
        {
        }
        finally
        {
            var surface = _surface;
            _surface = null;
            surface?.Dispose();
        }
    }

    // ---------------- 窗口尺寸跟着内容走 ----------------

    /// <summary>
    /// 分层窗口尺寸 = 内容自然尺寸。这里必须主动 <see cref="FrameworkElement.Measure"/>
    /// 一次：窗口本身就是被内容裁出来的，不能用 ActualWidth/ActualHeight 反推
    /// （那只会恒等于当前窗口尺寸，永远量不出该长还是该缩）。
    /// </summary>
    private void FitWindow()
    {
        if (_fitting) return;
        _fitting = true;
        try
        {
            PetStack.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
            var w = PetStack.DesiredSize.Width;
            var h = PetStack.DesiredSize.Height;
            if (w < 8 || h < 8) return;
            var scale = RasterizationScale;
            var px = new SizeInt32((int)Math.Ceiling(w * scale), (int)Math.Ceiling(h * scale));
            if (px.Width == _lastSize.Width && px.Height == _lastSize.Height) return;
            _lastSize = px;
            _surface?.SetContentSize(px.Width, px.Height);
            // 顺带把离屏 XAML 窗也排一次：它还兼职给 RenderTargetBitmap 供内容，
            // 且要跟分层窗停在同一块屏幕上，DPI 才跟着用户走
            PetStack.Arrange(new Rect(0, 0, w, h));
            PetRoot.UpdateLayout();
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[pet] fit failed: {ex}");
        }
        finally
        {
            _fitting = false;
        }
    }

    /// <summary>
    /// 内容的缩放比（1.0 = 96 DPI）。分层窗口吃的是物理像素，XAML 量的是 DIP。
    /// 取的是真实显示面（分层窗）的 DPI：用户把宠物拖到别的显示器上，它得跟着变。
    /// </summary>
    private double RasterizationScale
    {
        get
        {
            var hwnd = _surface is { Ready: true, Hwnd: var sh } && sh != IntPtr.Zero ? sh : _hwnd;
            var dpi = hwnd == IntPtr.Zero ? 0 : GetDpiForWindow(hwnd);
            return dpi > 0 ? dpi / 96.0 : 1.0;
        }
    }

    /// <summary>
    /// 拖动/点击已经搬进 <see cref="PetSurfaceLayer"/>：真正给人点的是分层窗口，
    /// 本窗口是隐藏的、收不到鼠标消息。这里只剩位置持久化与还原。
    /// </summary>

    // ---------------- 位置持久化 ----------------

    private void SavePosition()
    {
        if (_statePath == "" || _surface is not { Ready: true } surface) return;
        try
        {
            File.WriteAllText(_statePath,
                JsonSerializer.Serialize(new PetWindowPosition(surface.WindowX, surface.WindowY)));
        }
        catch (Exception)
        {
            // 落盘失败不影响当前会话：下次回到默认位置而已
        }
    }

    private void RestorePosition()
    {
        if (_statePath != "" && File.Exists(_statePath))
        {
            try
            {
                var saved = JsonSerializer.Deserialize<PetWindowPosition>(File.ReadAllText(_statePath));
                if (saved is not null)
                {
                    MoveTo(saved.X, saved.Y);
                    return;
                }
            }
            catch (Exception)
            {
                // 配置文件坏了：落到默认位置
            }
        }
        MoveToDefault();
    }

    /// <summary>默认落在虚拟屏右下角——桌面宠物的常规位置。</summary>
    private void MoveToDefault()
    {
        var vx = GetSystemMetrics(SmXVirtualScreen);
        var vy = GetSystemMetrics(SmYVirtualScreen);
        var vw = GetSystemMetrics(SmCxVirtualScreen);
        var vh = GetSystemMetrics(SmCyVirtualScreen);
        if (vw <= 0 || vh <= 0) return;
        var x = vx + vw - Math.Max(_lastSize.Width, 1) - 48;
        var y = vy + vh - Math.Max(_lastSize.Height, 1) - 48;
        MoveTo(x, y);
    }

    private void MoveTo(int x, int y)
    {
        _surface?.MoveTo(x, y);
    }

    private sealed record PetWindowPosition(int X, int Y);

    // ---------------- P/Invoke ----------------

    [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW", ExactSpelling = true)]
    private static extern IntPtr GetWindowLong(IntPtr hWnd, int nIndex);
    [DllImport("user32.dll", EntryPoint = "SetWindowLongPtrW", ExactSpelling = true)]
    private static extern IntPtr SetWindowLong(IntPtr hWnd, int nIndex, IntPtr dwNewLong);

    [DllImport("user32.dll")]
    private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")]
    private static extern int GetSystemMetrics(int nIndex);
    [DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(IntPtr hWnd);
}
