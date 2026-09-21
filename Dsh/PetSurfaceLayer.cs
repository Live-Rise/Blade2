using System;
using System.Runtime.InteropServices;
using System.Threading.Tasks;

namespace Blade2.Dsh;

/// <summary>一帧已经栅格化好的内容：BGRA8 预乘、自上而下、布局与 UpdateLayeredWindow 的源完全一致。</summary>
internal sealed class PetSurfaceFrame
{
    public required byte[] Pixels { get; init; }
    public required int Width { get; init; }
    public required int Height { get; init; }
}

/// <summary>
/// 宠物的真正显示面：一个逐像素透明的 Win32 分层窗口
/// （<c>WS_EX_LAYERED</c> + <c>UpdateLayeredWindow</c>，源就是 32 位 ARGB DIB）。
///
/// 为什么不能再拿 XAML 窗口当载体：WinUI 3 的窗口内容装在
/// <c>Microsoft.UI.Content.DesktopChildSiteBridge</c> 合成岛里，岛的根视觉有一层
/// 不透明白。这层白没有任何开关——<c>Microsoft.UI.Content</c> 命名空间下只有
/// <c>DesktopSiteBridge</c>/<c>DesktopChildSiteBridge</c> 两个类型，反射确认两者
/// 都没有可写的背景色；<c>SystemBackdrop</c> 只压在比它更上面的一层；DWM 垫底、
/// 颜色键、类画刷、宿主背景画刷全都试过，白矩形照旧。
///
/// 所以分工改成：XAML 那棵树只当离屏内容源（主题、图集帧动画一处不用改），
/// 每帧用 RenderTargetBitmap 量出带 alpha 的像素，搬到分层窗口里去。
/// 分层窗口的白色区域 alpha=0，桌面自然透出来；Win32 还会按 alpha 做命中测试，
/// 宠物四周的点不到家具上，点身子才拖动/互动。
/// </summary>
internal sealed class PetSurfaceLayer : IDisposable
{
    // ---------------- Win32 常量 ----------------
    private const uint WmMouseMove = 0x0200;
    private const uint WmLButtonDown = 0x0201;
    private const uint WmLButtonUp = 0x0202;
    private const uint WmMove = 0x0003;
    private const uint WmCaptureChanged = 0x0215;
    private const uint WmDestroy = 0x0002;
    private const uint WmNcHitTest = 0x0084;

    private const int HtClient = 1;
    private const int HtTransparent = -1;

    private const int ErrorClassAlreadyExists = 1410;

    private const int WsPopup = unchecked((int)0x80000000);
    private const int WsClipSiblings = 0x04000000;
    private const int GwlExStyle = -20;
    private const int WsExLayered = 0x00080000;
    private const int WsExTopmost = 0x00000008;
    private const int WsExToolWindow = 0x00000080;
    private const int WsExNoActivate = 0x08000000;

    private const uint SwpNoSize = 0x0001;
    private const uint SwpNoMove = 0x0002;
    private const uint SwpNoZOrder = 0x0004;
    private const uint SwpNoActivate = 0x0010;
    private const uint SwpShowWindow = 0x0040;
    private static readonly IntPtr HwndTopmost = new(-1);

    private const byte AcSrcOver = 0x00;
    private const byte AcSrcAlpha = 0x01;
    private const uint UlwAlpha = 0x00000002;

    private const int SmXVirtualScreen = 76;
    private const int SmYVirtualScreen = 77;
    private const int SmCxVirtualScreen = 78;
    private const int SmCyVirtualScreen = 79;

    /// <summary>拖动判定阈值（px）：位移小于它算点击（互动），大于算拖动。</summary>
    private const int DragThresholdPx = 6;
    /// <summary>贴边时至少留这么多像素在屏幕内，否则宠物被拖出屏幕就抓不回来了。</summary>
    private const int KeepOnScreenPx = 64;

    /// <summary>DIB 高度上限：宠物极小，这个是防呆不是配额。</summary>
    private const int MaxSide = 4096;

    private delegate IntPtr WindowProc(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam);

    private readonly Func<Task<PetSurfaceFrame?>> _capture;
    private readonly Func<Task>? _interact;
    private readonly Microsoft.UI.Dispatching.DispatcherQueue _dispatcher;
    private readonly WindowProc _classProc;
    private IntPtr _classInstance;

    private IntPtr _hwnd;
    private IntPtr _screenDc;
    private IntPtr _memDc;
    private IntPtr _dib;
    private IntPtr _bits;

    private int _width, _height;
    private int _x, _y;
    private bool _visible;
    private bool _disposed;

    private bool _renderQueued;
    private bool _rendering;

    // 拖动状态
    private bool _dragging;
    private int _dragCursorX, _dragCursorY;
    private int _dragWindowX, _dragWindowY;
    private int _dragMoved;

    /// <summary>拖动结束（位置已变）：壳拿去持久化。</summary>
    public event Action? DragEnded;

    internal PetSurfaceLayer(Func<Task<PetSurfaceFrame?>> capture, Func<Task>? interact)
    {
        _capture = capture;
        _interact = interact;
        _dispatcher = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
        _classProc = WndProc;
        var hInstance = GetModuleHandle(null);
        _classInstance = hInstance;

        var wc = new WndClass
        {
            CbSize = Marshal.SizeOf<WndClass>(),
            Style = 0,
            LpfnWndProc = Marshal.GetFunctionPointerForDelegate(_classProc),
            CbClsExtra = 0,
            CbWndExtra = 0,
            HInstance = hInstance,
            HIcon = IntPtr.Zero,
            HCursor = LoadCursor(IntPtr.Zero, 32512),   // IDC_ARROW
            HbrBackground = IntPtr.Zero,
            LpszMenuName = null,
            LpszClassName = ClassName,
        };

        if (RegisterClassExW(ref wc) == 0)
        {
            var err = Marshal.GetLastWin32Error();
            if (err != ErrorClassAlreadyExists)
            {
                throw new InvalidOperationException($"[pet] 注册分层窗口类失败 err={err}");
            }
            // 类已存在：指针来自上一个还活着的实例，直接复用，不必反注册。
        }

        try
        {
            _hwnd = CreateWindowExW(
                WsExLayered | WsExTopmost | WsExToolWindow | WsExNoActivate,
                ClassName, "pet", WsPopup | WsClipSiblings, 0, 0, 64, 64,
                IntPtr.Zero, IntPtr.Zero, hInstance, IntPtr.Zero);

            if (_hwnd == IntPtr.Zero)
            {
                throw new InvalidOperationException(
                    $"[pet] 建分层窗口失败 err={Marshal.GetLastWin32Error()}");
            }
        }
        catch
        {
            // 窗口没建成，类必须撤掉：否则留一个指向即将被回收的委托的类，
            // 后面每次重试都只会撞 ERROR_CLASS_ALREADY_EXISTS，再也建不出窗口。
            UnregisterClassW(ClassName, hInstance);
            throw;
        }

        _screenDc = GetDC(IntPtr.Zero);
        _memDc = CreateCompatibleDC(_screenDc);

        // 分层窗口有 DWM 外框时，边缘会留一圈钉到窗上的阴影；把外框铺满客户区。
        var m = new Margins { Left = -1, Top = -1, Right = -1, Bottom = -1 };
        DwmExtendFrameIntoClientArea(_hwnd, ref m);
    }

    private const string ClassName = "Blade2.PetSurface";

    internal bool Ready => _hwnd != IntPtr.Zero && _memDc != IntPtr.Zero && !_disposed;

    internal IntPtr Hwnd => _hwnd;

    internal int WindowX => _x;

    internal int WindowY => _y;

    internal int PixelWidth => _width;

    internal int PixelHeight => _height;

    /// <summary>内容自然尺寸变了：DIB 和窗口一起改，尺寸没变就什么都不做。</summary>
    internal void SetContentSize(int w, int h)
    {
        if (_disposed || _hwnd == IntPtr.Zero) return;
        w = Math.Clamp(w, 8, MaxSide);
        h = Math.Clamp(h, 8, MaxSide);
        if (w == _width && h == _height) return;
        AllocateDib(w, h);
        SetWindowPos(_hwnd, IntPtr.Zero, _x, _y, w, h,
            SwpNoZOrder | SwpNoActivate | SwpNoMove);
        Invalidate();
    }

    internal void Show()
    {
        if (_disposed || _hwnd == IntPtr.Zero) return;
        _visible = true;
        ShowWindow(_hwnd, 5 /*SW_SHOW*/);
        BringToFront();
        Invalidate();
    }

    internal void Hide()
    {
        if (_disposed || _hwnd == IntPtr.Zero) return;
        _visible = false;
        ShowWindow(_hwnd, 0 /*SW_HIDE*/);
    }

    /// <summary>别的应用也可能抢置顶：每次显示/唤醒都补一次 Z 序。</summary>
    internal void BringToFront()
    {
        if (_disposed || _hwnd == IntPtr.Zero) return;
        SetWindowPos(_hwnd, HwndTopmost, 0, 0, 0, 0,
            SwpNoMove | SwpNoSize | SwpNoActivate | SwpShowWindow);
    }

    internal void MoveTo(int x, int y)
    {
        if (_disposed || _hwnd == IntPtr.Zero) return;
        _x = x;
        _y = y;
        SetWindowPos(_hwnd, IntPtr.Zero, x, y, 0, 0,
            SwpNoSize | SwpNoZOrder | SwpNoActivate);
    }

    /// <summary>请求重绘一帧。消息泵自然合并连续的请求：一帧最多一次 UpdateLayeredWindow。</summary>
    internal void Invalidate()
    {
        if (_disposed || !Ready || _renderQueued) return;
        _renderQueued = true;
        _dispatcher.TryEnqueue(RenderQueued);
    }

    private void RenderQueued()
    {
        _renderQueued = false;
        _ = RenderAsync();
    }

    private async Task RenderAsync()
    {
        if (_rendering || _disposed || !Ready) return;
        _rendering = true;
        try
        {
            var frame = await _capture();
            if (_disposed || frame is null) return;
            if (frame.Width != _width || frame.Height != _height) AllocateDib(frame.Width, frame.Height);
            var pixels = frame.Pixels;
            if (pixels.Length < _width * _height * 4 || _bits == IntPtr.Zero) return;

            Marshal.Copy(pixels, 0, _bits, _width * _height * 4);

            var dst = new PointInt32 { X = _x, Y = _y };
            var size = new SizeInt32 { Width = _width, Height = _height };
            var src = new PointInt32 { X = 0, Y = 0 };
            var blend = new BlendFunction
            {
                BlendOp = AcSrcOver,
                BlendFlags = 0,
                SourceConstantAlpha = 255,
                AlphaFormat = AcSrcAlpha,
            };
            UpdateLayeredWindow(_hwnd, _screenDc, ref dst, ref size, _memDc, ref src,
                0, ref blend, UlwAlpha);
        }
        catch (Exception ex)
        {
            // 丢一帧远好过拖垮消息泵；下一帧又来
            System.Diagnostics.Debug.WriteLine($"[pet] surface render failed: {ex}");
        }
        finally
        {
            _rendering = false;
        }
    }

    // ---------------- DIB ----------------

    private void AllocateDib(int w, int h)
    {
        w = Math.Clamp(w, 8, MaxSide);
        h = Math.Clamp(h, 8, MaxSide);
        if (w == _width && h == _height && _dib != IntPtr.Zero) return;

        var oldDib = _dib;
        var bi = new BitmapInfo
        {
            Header =
            {
                Size = Marshal.SizeOf<BitmapInfoHeader>(),
                Width = w,
                // 负高度 = 自上而下，这样网络传播的像素顺序和 UpdateLayeredWindow 期望的一致
                Height = -h,
                Planes = 1,
                BitCount = (short)32,
                Compression = 0 /*BI_RGB*/,
                SizeImage = w * h * 4,
            },
        };
        var bits = IntPtr.Zero;
        var dib = CreateDIBSection(_screenDc, ref bi, 0, ref bits, IntPtr.Zero, 0);
        if (dib == IntPtr.Zero || bits == IntPtr.Zero)
        {
            throw new InvalidOperationException(
                $"[pet] 建分层窗口 DIB 失败 err={Marshal.GetLastWin32Error()}");
        }

        SelectObject(_memDc, dib);
        _dib = dib;
        _bits = bits;
        _width = w;
        _height = h;
        if (oldDib != IntPtr.Zero) DeleteObject(oldDib);
        SetWindowPos(_hwnd, IntPtr.Zero, _x, _y, w, h,
            SwpNoZOrder | SwpNoActivate | SwpNoMove);
    }

    private IntPtr WndProc(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam)
    {
        try
        {
            switch (msg)
            {
                case WmNcHitTest:
                    return HitTest(wParam, lParam);
                case WmLButtonDown:
                    StartDrag();
                    break;
                case WmMouseMove:
                    MoveDrag();
                    break;
                case WmLButtonUp:
                    EndDrag();
                    break;
                case WmCaptureChanged:
                    // 捕获被外力抢走（切桌面/按 Esc）：结束拖动且不算点击
                    _dragging = false;
                    break;
                case WmMove:
                    _x = unchecked((short)(lParam.ToInt64() & 0xFFFF));
                    _y = unchecked((short)((lParam.ToInt64() >> 16) & 0xFFFF));
                    break;
            }
        }
        catch (Exception ex)
        {
            // 窗口过程抛异常会拖垮消息泵，一切兜底
            System.Diagnostics.Debug.WriteLine($"[pet] surface wndproc failed: {ex}");
        }
        return DefWindowProcW(hwnd, msg, wParam, lParam);
    }

    /// <summary>
    /// Win32 对分层窗口本来就按 alpha 做命中测试，这里只做同一件事并显式处理一处：
    /// 抽掉 DIB 里写的溢出边（缩放后的宠物边缘常有半透明羽化，别让它们挡住点穿透）。
    /// </summary>
    private IntPtr HitTest(IntPtr wParam, IntPtr lParam)
    {
        try
        {
            if (_bits == IntPtr.Zero || _width <= 0 || _height <= 0) return new IntPtr(HtClient);
            var sx = unchecked((short)(lParam.ToInt64() & 0xFFFF));
            var sy = unchecked((short)((lParam.ToInt64() >> 16) & 0xFFFF));
            var lx = sx - _x;
            var ly = sy - _y;
            if (lx < 0 || ly < 0 || lx >= _width || ly >= _height) return new IntPtr(HtTransparent);

            var alpha = Marshal.ReadByte(_bits, (ly * _width + lx) * 4 + 3);
            var hit = alpha >= 24 ? HtClient : HtTransparent;
            _ = wParam;
            return new IntPtr(hit);
        }
        catch (Exception ex)
        {
            System.Diagnostics.Debug.WriteLine($"[pet] surface hittest failed: {ex}");
            return new IntPtr(HtClient);
        }
    }

    private void StartDrag()
    {
        GetCursorPos(out var cursor);
        _dragCursorX = cursor.X;
        _dragCursorY = cursor.Y;
        _dragWindowX = _x;
        _dragWindowY = _y;
        _dragMoved = 0;
        _dragging = true;
        SetCapture(_hwnd);
    }

    private void MoveDrag()
    {
        if (!_dragging || _hwnd == IntPtr.Zero) return;
        GetCursorPos(out var cursor);
        var dx = cursor.X - _dragCursorX;
        var dy = cursor.Y - _dragCursorY;
        _dragMoved = Math.Max(_dragMoved, Math.Max(Math.Abs(dx), Math.Abs(dy)));
        var x = _dragWindowX + dx;
        var y = _dragWindowY + dy;
        ClampToVirtualScreen(ref x, ref y);
        _x = x;
        _y = y;
        SetWindowPos(_hwnd, IntPtr.Zero, x, y, 0, 0, SwpNoSize | SwpNoZOrder | SwpNoActivate);
        // 拖动过程中内容没变，但分层窗口的位置变了，得补一帧把新位置讲给它
        UpdateLayeredPosition();
    }

    private void UpdateLayeredPosition()
    {
        if (_bits == IntPtr.Zero) return;
        var dst = new PointInt32 { X = _x, Y = _y };
        var size = new SizeInt32 { Width = _width, Height = _height };
        var src = new PointInt32 { X = 0, Y = 0 };
        var blend = new BlendFunction
        {
            BlendOp = AcSrcOver,
            BlendFlags = 0,
            SourceConstantAlpha = 255,
            AlphaFormat = AcSrcAlpha,
        };
        UpdateLayeredWindow(_hwnd, _screenDc, ref dst, ref size, _memDc, ref src,
            0, ref blend, UlwAlpha);
    }

    private void EndDrag()
    {
        if (!_dragging) return;
        _dragging = false;
        ReleaseCapture();
        if (_dragMoved < DragThresholdPx)
        {
            // 位移不够 = 点击：喂一次互动
            var handler = _interact;
            if (handler is not null) _ = FireAndForget(handler);
        }
        else
        {
            DragEnded?.Invoke();
        }
    }

    private static async Task FireAndForget(Func<Task> handler)
    {
        try
        {
            await handler();
        }
        catch (Exception ex)
        {
            System.Diagnostics.Debug.WriteLine($"[pet] interact handler failed: {ex}");
        }
    }

    /// <summary>别让宠物被整个拖出屏幕：至少留 <see cref="KeepOnScreenPx"/> 在虚拟屏内。</summary>
    private void ClampToVirtualScreen(ref int x, ref int y)
    {
        var vx = GetSystemMetrics(SmXVirtualScreen);
        var vy = GetSystemMetrics(SmYVirtualScreen);
        var vw = GetSystemMetrics(SmCxVirtualScreen);
        var vh = GetSystemMetrics(SmCyVirtualScreen);
        if (vw <= 0 || vh <= 0) return;
        var w = Math.Max(_width, 1);
        var h = Math.Max(_height, 1);
        if (x > vx + vw - KeepOnScreenPx) x = vx + vw - KeepOnScreenPx;
        if (x < vx + KeepOnScreenPx - w) x = vx + KeepOnScreenPx - w;
        if (y > vy + vh - KeepOnScreenPx) y = vy + vh - KeepOnScreenPx;
        if (y < vy + KeepOnScreenPx - h) y = vy + KeepOnScreenPx - h;
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        try
        {
            if (_hwnd != IntPtr.Zero)
            {
                DestroyWindow(_hwnd);
            }

            if (_classInstance != IntPtr.Zero)
            {
                UnregisterClassW(ClassName, _classInstance);
            }
        }
        catch (Exception)
        {
        }

        if (_memDc != IntPtr.Zero && _dib != IntPtr.Zero)
        {
            SelectObject(_memDc, IntPtr.Zero);
            DeleteObject(_dib);
        }
        if (_memDc != IntPtr.Zero) DeleteDC(_memDc);
        if (_screenDc != IntPtr.Zero) ReleaseDC(IntPtr.Zero, _screenDc);
        _dib = IntPtr.Zero;
        _memDc = IntPtr.Zero;
        _bits = IntPtr.Zero;
        DragEnded = null;
    }

    // ---------------- 互操作 ----------------

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr GetModuleHandle(string? lpModuleName);

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern ushort RegisterClassExW(ref WndClass lpwcx);

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool UnregisterClassW(string lpClassName, IntPtr hInstance);

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr CreateWindowExW(
        int dwExStyle, string lpClassName, string lpWindowName, int dwStyle,
        int x, int y, int nWidth, int nHeight,
        IntPtr hWndParent, IntPtr hMenu, IntPtr hInstance, IntPtr lpParam);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool DestroyWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    private static extern IntPtr DefWindowProcW(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll")]
    private static extern IntPtr LoadCursor(IntPtr hInstance, int lpCursorName);

    [DllImport("user32.dll")]
    private static extern IntPtr GetDC(IntPtr hWnd);

    [DllImport("user32.dll")]
    private static extern int ReleaseDC(IntPtr hWnd, IntPtr hDC);

    [DllImport("gdi32.dll")]
    private static extern IntPtr CreateCompatibleDC(IntPtr hdc);

    [DllImport("gdi32.dll")]
    private static extern bool DeleteDC(IntPtr hdc);

    [DllImport("gdi32.dll")]
    private static extern IntPtr SelectObject(IntPtr hdc, IntPtr h);

    [DllImport("gdi32.dll", SetLastError = true)]
    private static extern IntPtr CreateDIBSection(IntPtr hdc, ref BitmapInfo pbmi,
        uint usage, ref IntPtr ppvBits, IntPtr hSection, uint offset);

    [DllImport("gdi32.dll")]
    private static extern bool DeleteObject(IntPtr hObject);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool UpdateLayeredWindow(IntPtr hwnd, IntPtr hdcDst,
        ref PointInt32 pptDst, ref SizeInt32 psize, IntPtr hdcSrc, ref PointInt32 pptSrc,
        int crKey, ref BlendFunction pblend, uint dwFlags);

    [DllImport("user32.dll")]
    private static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter,
        int x, int y, int cx, int cy, uint uFlags);

    [DllImport("user32.dll")]
    private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

    [DllImport("user32.dll")]
    private static extern bool GetCursorPos(out POINT lpPoint);

    [DllImport("user32.dll")]
    private static extern bool SetCapture(IntPtr hWnd);

    [DllImport("user32.dll")]
    private static extern bool ReleaseCapture();

    [DllImport("user32.dll")]
    private static extern int GetSystemMetrics(int nIndex);

    [DllImport("dwmapi.dll")]
    private static extern int DwmExtendFrameIntoClientArea(IntPtr hWnd, ref Margins pMarInset);

    [StructLayout(LayoutKind.Sequential)]
    private struct POINT
    {
        public int X;
        public int Y;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct WndClass
    {
        public int CbSize;
        public int Style;
        public IntPtr LpfnWndProc;
        public int CbClsExtra;
        public int CbWndExtra;
        public IntPtr HInstance;
        public IntPtr HIcon;
        public IntPtr HCursor;
        public IntPtr HbrBackground;

        [MarshalAs(UnmanagedType.LPWStr)]
        public string? LpszMenuName;

        [MarshalAs(UnmanagedType.LPWStr)]
        public string LpszClassName;

        public IntPtr HIconSm;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Margins
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct PointInt32
    {
        public int X;
        public int Y;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct SizeInt32
    {
        public int Width;
        public int Height;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct BlendFunction
    {
        public byte BlendOp;
        public byte BlendFlags;
        public byte SourceConstantAlpha;
        public byte AlphaFormat;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct BitmapInfo
    {
        public BitmapInfoHeader Header;

        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 256)]
        public int[] Masks;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct BitmapInfoHeader
    {
        public int Size;
        public int Width;
        public int Height;
        public short Planes;
        public short BitCount;
        public int Compression;
        public int SizeImage;
        public int XPelsPerMeter;
        public int YPelsPerMeter;
        public int ClrUsed;
        public int ClrImportant;
    }
}
