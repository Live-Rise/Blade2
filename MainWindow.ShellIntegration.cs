using System;
using System.IO;
using System.Runtime.InteropServices;

namespace Blade2;

/// <summary>
/// 壳的系统集成：一条 comctl32 窗口子类化承载两件事。
/// 1) WM_SETTINGCHANGE(ImmersiveColorSet)：system 主题偏好下实时跟随 Windows 深浅色
///    （此前 ApplyShellTheme 只在应用时读一次 UISettings，系统切换要重启才生效——本钩子即修复点）；
/// 2) Shell_NotifyIcon 托盘图标：左键唤起/聚焦窗口，右键原生弹出菜单（打开 / 退出）。
/// 系统级 toast（Windows App SDK AppNotification）也在此文件：内核启动失败、
/// 窗口未聚焦时的工具审批请求 / 用户提问 / 任务完成走系统通知。无包身份的开发形态
/// Register 抛异常 → 退化托盘气球（ShowTrayBalloon）；点击回前台：toast 走
/// NotificationInvoked、气球走 NIN_BALLOONUSERCLICK。
/// </summary>
public partial class MainWindow
{
    private const uint WM_SETTINGCHANGE = 0x001A;
    private const uint WM_DESTROY = 0x0002;
    // 托盘回调消息：WM_APP 段壳内无其它占用者，取首号。
    private const uint WM_TRAYCALLBACK = 0x0400 + 1;

    private IntPtr _hwnd;
    private SubclassProc? _subclassProc; // 字段持有防 GC 回收子类化委托
    private bool _trayAdded;
    private IntPtr _trayIcon;

    private delegate IntPtr SubclassProc(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam, uint id, IntPtr refData);

    // ---------------- 生命周期 ----------------

    /// <summary>构造函数里安装（hwnd 此时已可用）。失败不致命：托盘/跟随深浅色不可用而已。</summary>
    private void InstallShellIntegration()
    {
        try
        {
            _hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            _subclassProc = WndProcSubclass;
            if (!SetWindowSubclass(_hwnd, _subclassProc, 1, IntPtr.Zero))
            {
                return;
            }
            // 托盘图标受设置控制（通用 → 托盘与退出）；关掉时不创建。
            if (_shellOptions.ShowTrayIcon)
            {
                TrayAdd();
            }
        }
        catch (Exception) { }
    }

    /// <summary>窗口关闭时清理：先摘托盘再原子类化（WM_DESTROY 兜底再删一次，重复删除无害）。</summary>
    private void CleanupShellIntegration()
    {
        try
        {
            TrayRemove();
            if (_hwnd != IntPtr.Zero && _subclassProc is not null)
            {
                RemoveWindowSubclass(_hwnd, _subclassProc, 1);
            }
        }
        catch (Exception) { }
    }

    private IntPtr WndProcSubclass(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam, uint id, IntPtr refData)
    {
        try
        {
            switch (msg)
            {
                case WM_SETTINGCHANGE:
                    // lParam 指向变更类目名；ImmersiveColorSet = 系统深浅色/强调色变更。
                    if (_shellPreference == "system" && lParam != IntPtr.Zero &&
                        string.Equals(Marshal.PtrToStringUni(lParam), "ImmersiveColorSet", StringComparison.Ordinal))
                    {
                        ApplyShellTheme("system"); // 重读 UISettings 实况并整体换肤
                    }
                    break;
                case WM_TRAYCALLBACK:
                    // 未设 NOTIFYICON_VERSION_4 的旧语义：lParam = 鼠标消息
                    switch (lParam.ToInt64() & 0xFFFF)
                    {
                        case 0x0202: ActivateFromTray(); break;         // WM_LBUTTONUP
                        case 0x0205: ShowTrayMenu(); break;             // WM_RBUTTONUP
                        case 0x0402: ActivateFromTray(); break;         // NIN_BALLOONUSERCLICK：点气球通知回前台
                    }
                    break;
                case WM_DESTROY:
                    TrayRemove();
                    break;
            }
        }
        catch (Exception) { } // 子类化回调抛异常会拖垮消息泵，一切兜底
        return DefSubclassProc(hwnd, msg, wParam, lParam);
    }

    // ---------------- 托盘 ----------------

    private void TrayAdd()
    {
        if (_trayAdded || _hwnd == IntPtr.Zero)
        {
            return;
        }
        // 大图标取 exe 第一图标组（= csproj ApplicationIcon，即品牌刀锋徽章）
        if (ExtractIconEx(Environment.ProcessPath ?? string.Empty, 0, out var large, out _, 1) == 0 || large == IntPtr.Zero)
        {
            return;
        }
        _trayIcon = large;
        var nid = new NOTIFYICONDATAW
        {
            cbSize = (uint)Marshal.SizeOf<NOTIFYICONDATAW>(),
            hWnd = _hwnd,
            uID = 1,
            uFlags = 0x1 | 0x2 | 0x4, // NIF_MESSAGE | NIF_ICON | NIF_TIP
            uCallbackMessage = WM_TRAYCALLBACK,
            hIcon = large,
            szTip = "Blade²",
            szInfo = string.Empty,
            szInfoTitle = string.Empty,
        };
        _trayAdded = Shell_NotifyIcon(0x00 /* NIM_ADD */, ref nid);
    }

    private void TrayRemove()
    {
        if (!_trayAdded)
        {
            return;
        }
        var nid = new NOTIFYICONDATAW
        {
            cbSize = (uint)Marshal.SizeOf<NOTIFYICONDATAW>(),
            hWnd = _hwnd,
            uID = 1,
            szTip = string.Empty,
            szInfo = string.Empty,
            szInfoTitle = string.Empty,
        };
        Shell_NotifyIcon(0x02 /* NIM_DELETE */, ref nid);
        if (_trayIcon != IntPtr.Zero)
        {
            DestroyIcon(_trayIcon);
            _trayIcon = IntPtr.Zero;
        }
        _trayAdded = false;
    }

    private void ActivateFromTray()
    {
        try
        {
            // 藏到托盘的窗口是隐藏态（无任务栏按钮）：先 Show 再恢复/聚焦。
            AppWindow.Show();
            if (IsIconic(_hwnd))
            {
                ShowWindow(_hwnd, 9 /* SW_RESTORE */);
            }
            SetForegroundWindow(_hwnd);
            Activate();
        }
        catch (Exception) { }
    }

    /// <summary>托盘气球提示：无包身份的 dev 形态（dotnet run）AppNotification 不可用时的兜底。
    /// 图标不在先补建；用户关掉托盘图标则静默失败——那正是他选择不收扰的形式。
    /// 点击气球走 WM_TRAYCALLBACK/NIN_BALLOONUSERCLICK 回前台（见 WndProcSubclass）。</summary>
    private void ShowTrayBalloon(string title, string body)
    {
        try
        {
            // 用户关掉托盘图标时不替他重建：气球必须有所属图标，没有就静默跳过。
            if (!_trayAdded && !_shellOptions.ShowTrayIcon)
            {
                return;
            }
            if (!_trayAdded)
            {
                TrayAdd();
            }
            if (!_trayAdded || _hwnd == IntPtr.Zero)
            {
                return;
            }
            var nid = new NOTIFYICONDATAW
            {
                cbSize = (uint)Marshal.SizeOf<NOTIFYICONDATAW>(),
                hWnd = _hwnd,
                uID = 1,
                uFlags = 0x1 | 0x10, // NIF_MESSAGE | NIF_INFO
                uCallbackMessage = WM_TRAYCALLBACK,
                szTip = "Blade²",
                szInfo = TruncateForBalloon(body, 256),
                szInfoTitle = TruncateForBalloon(title, 64),
                dwInfoFlags = 0x1, // NIIF_INFO
            };
            Shell_NotifyIcon(0x01 /* NIM_MODIFY */, ref nid);
        }
        catch (Exception) { }
    }

    /// <summary>定长 ByValTStr 字段要留给尾部的 \0：截到缓冲区容量减一。</summary>
    private static string TruncateForBalloon(string text, int bufferSize)
        => text.Length < bufferSize ? text : text[..(bufferSize - 1)];

    /// <summary>托盘右键菜单：原生 Win32 菜单（托盘场景没有 XamlRoot，WinUI 弹层不可用）。</summary>
    private void ShowTrayMenu()
    {
        try
        {
            var menu = CreatePopupMenu();
            if (menu == IntPtr.Zero)
            {
                return;
            }
            AppendMenuW(menu, 0x0000, (IntPtr)1, L("打开 Blade²"));
            AppendMenuW(menu, 0x0800 /* MF_SEPARATOR */, IntPtr.Zero, null);
            AppendMenuW(menu, 0x0000, (IntPtr)2, L("退出"));
            if (GetCursorPos(out var pt))
            {
                // TrackPopupMenu 前必须把窗口提到前台，否则点别处菜单不消失
                SetForegroundWindow(_hwnd);
                var cmd = TrackPopupMenuEx(menu, 0x0100 /* TPM_RETURNCMD */ | 0x0002 /* TPM_RIGHTBUTTON */ | 0x0080 /* TPM_NONOTIFY */,
                    pt.X, pt.Y, _hwnd, IntPtr.Zero);
                if (cmd == 1)
                {
                    ActivateFromTray();
                }
                else if (cmd == 2)
                {
                    QuitShell(); // 放行关闭拦截 → Closed → Shutdown：杀内核 + 摘托盘
                }
            }
            DestroyMenu(menu);
        }
        catch (Exception) { }
    }

    // ---------------- P/Invoke ----------------

    [DllImport("comctl32.dll")]
    private static extern bool SetWindowSubclass(IntPtr hWnd, SubclassProc pfnSubclass, uint uIDSubclass, IntPtr dwRefData);
    [DllImport("comctl32.dll")]
    private static extern bool RemoveWindowSubclass(IntPtr hWnd, SubclassProc pfnSubclass, uint uIDSubclass);
    [DllImport("comctl32.dll")]
    private static extern IntPtr DefSubclassProc(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
    [DllImport("shell32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool Shell_NotifyIcon(uint dwMessage, ref NOTIFYICONDATAW lpData);
    [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
    private static extern uint ExtractIconEx(string lpszFile, int nIconIndex, out IntPtr phiconLarge, out IntPtr phiconSmall, uint nIcons);
    [DllImport("user32.dll")]
    private static extern bool DestroyIcon(IntPtr hIcon);
    [DllImport("user32.dll")]
    private static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll")]
    private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")]
    private static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")]
    private static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")]
    private static extern bool GetCursorPos(out POINT lpPoint);
    [DllImport("user32.dll")]
    private static extern IntPtr CreatePopupMenu();
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern bool AppendMenuW(IntPtr hMenu, uint uFlags, IntPtr uIDNewItem, string? lpNewItem);
    [return: MarshalAs(UnmanagedType.I4)]
    [DllImport("user32.dll")]
    private static extern int TrackPopupMenuEx(IntPtr hMenu, uint uFlags, int x, int y, IntPtr hWnd, IntPtr lptpm);
    [DllImport("user32.dll")]
    private static extern bool DestroyMenu(IntPtr hMenu);

    [StructLayout(LayoutKind.Sequential)]
    private struct POINT
    {
        public int X;
        public int Y;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct NOTIFYICONDATAW
    {
        public uint cbSize;
        public IntPtr hWnd;
        public uint uID;
        public uint uFlags;
        public uint uCallbackMessage;
        public IntPtr hIcon;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string szTip;
        public uint dwState;
        public uint dwStateMask;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 256)] public string szInfo;
        public uint uVersion;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 64)] public string szInfoTitle;
        public uint dwInfoFlags;
        public Guid guidItem;
        public IntPtr hBalloonIcon;
    }
}

/// <summary>
/// 系统级 toast 通知（Windows App SDK AppNotification）。
/// 打包形态（MSIX）：Package.appxmanifest 声明 toast 激活器（desktop:ToastNotificationActivation
/// + com:ComServer，CLSID 一致）后 Register 成功，通知走系统 toast；无包身份的开发形态
/// （dotnet run）Register 抛异常，退化为托盘气球（<see cref="BalloonFallback"/>，MainWindow
/// 注入 ShowTrayBalloon）——通知不可用从不影响壳的其它部分。点击回前台：运行中实例走
/// NotificationInvoked、冷启动走 COM 服务激活（App.OnLaunched 透传的 AppNotification 参数），
/// 气球走 NIN_BALLOONUSERCLICK，二者都汇入 <see cref="ActivateRequested"/>。
/// 注册（<see cref="RegisterSender"/>）显式带上发送者名 Blade²：无包形态双参重载一步钉死，
/// 打包形态由 manifest 的 com:ExeServer DisplayName 渲染。通知平台的显示名按身份只认首次
/// 注册，旧版本注册过的 DshWinUI 不会因这些修改自动刷新，故启动时先走
/// <see cref="MigrateStaleSenderName"/> 清一次残留注册（<see cref="RememberSenderName"/> 记住
/// 当前名字，日常启动不再清理）。
/// </summary>
internal static class ShellToast
{
    /// <summary>toast 头部的发送者名：与托盘、开始菜单、窗口标题用同一个品牌名。</summary>
    private const string SenderName = "Blade²";

    private static bool _checked;
    private static bool _registered;

    /// <summary>无包身份时的托盘气球兜底（MainWindow 构造时注入）。</summary>
    internal static Action<string, string>? BalloonFallback;

    /// <summary>通知点击 → 窗口回前台（MainWindow 构造时注入；可能来自后台线程）。</summary>
    internal static Action? ActivateRequested;

    /// <summary>已注册发送者名的标记文件（DataHome 下，与皮肤/会话数据同目录，包外）。</summary>
    private static string SenderStampPath => Path.Combine(MainWindow.DataHome, "shell-toast.sender");

    /// <summary>通知平台里「这个身份当前注册的发送者名」= 身份 + 名字。无包与打包两个形态的
    /// 注册记录互相独立（各自的身份不同），所以身份也写进标记。</summary>
    private static string SenderStamp => NotificationIdentity() + "\n" + SenderName;

    /// <summary>通知身份：打包取包系列名，无包取 exe 路径（Register 把本进程注册成 COM 服务，
    /// 平台按这个身份缓存显示名）。</summary>
    private static string NotificationIdentity()
    {
        try
        {
            return "pkg:" + Windows.ApplicationModel.Package.Current.Id.FamilyName;
        }
        catch (Exception)
        {
            return "exe:" + Environment.ProcessPath;
        }
    }

    internal static void EnsureRegistered()
    {
        if (_checked)
        {
            return;
        }
        _checked = true;
        try
        {
            // NotificationInvoked 必须先于 Register 订阅：顺序反了的话调用会被 OS 路由到新拉的
            // 进程，运行中的实例收不到点击回前台。
            Microsoft.Windows.AppNotifications.AppNotificationManager.Default.NotificationInvoked += OnNotificationInvoked;
            MigrateStaleSenderName();
            RegisterSender();
            _registered = true;
            RememberSenderName();
        }
        catch (Exception ex)
        {
            // 打包形态 Register 依赖 Package.appxmanifest 的 toast 激活器声明（desktop:ToastNotificationActivation
            // + com:ComServer，两者 CLSID 一致）；dev 形态无包身份也落到这里。诊断必须留痕——
            // 这条异常曾被静默吞掉，导致通知实际走兜底数月无人察觉。
            System.Diagnostics.Debug.WriteLine(
                $"[shell-toast] Register failed ({ex.GetType().Name}: {ex.Message}); degrading to tray balloon");
        }
    }

    /// <summary>
    /// 清掉注册记录里残留的旧发送者名。通知平台按身份把显示名只认首次注册：旧版本用无参
    /// Register 注册过之后，改 exe 版本信息（FileDescription）、改 manifest 的
    /// com:ExeServer DisplayName、乃至显式双参 Register("Blade²", icon) 全都不刷新——
    /// 实测重启进程、重编译 exe 后 toast 头部仍是旧名，只有 UnregisterAll 清掉注册记录后
    /// 重新注册才生效（独立探针程序二分验证）。名字与标记一致时什么都不做，日常启动不走
    /// 这条路径；它也顺带覆盖「品牌改名/改图标」以后的再次迁移。
    /// </summary>
    private static void MigrateStaleSenderName()
    {
        try
        {
            if (File.Exists(SenderStampPath) && File.ReadAllText(SenderStampPath) == SenderStamp)
            {
                return;
            }
            Microsoft.Windows.AppNotifications.AppNotificationManager.Default.UnregisterAll();
            System.Diagnostics.Debug.WriteLine(
                $"[shell-toast] stale sender registration cleared for {NotificationIdentity()}; re-registering as {SenderName}");
        }
        catch (Exception ex)
        {
            // 清不动也继续注：最坏结果是头部沿用旧名，通知本身还能用。
            System.Diagnostics.Debug.WriteLine(
                $"[shell-toast] UnregisterAll failed ({ex.GetType().Name}: {ex.Message}); registering anyway");
        }
    }

    /// <summary>注册成功后记住当前发送者名，下次启动不再走清理路径。</summary>
    private static void RememberSenderName()
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(SenderStampPath)!);
            File.WriteAllText(SenderStampPath, SenderStamp);
        }
        catch (Exception)
        {
            // 标记写不了不影响通知本身，只是下次启动会再清一次注册。
        }
    }

    /// <summary>把发送者名注册成 Blade²：toast 头部的应用名必须与托盘/开始菜单一致。
    /// 无包形态（dev/裸 exe）显式双参注册一步钉死名字与图标；打包形态不认这个形状
    /// （双参重载对包内应用抛异常），退回无参注册，由 manifest 的 com:ExeServer
    /// DisplayName 渲染。两条路径的点击回前台能力都不受影响。</summary>
    private static void RegisterSender()
    {
        if (TryRegisterWithIdentity())
        {
            return;
        }
        Microsoft.Windows.AppNotifications.AppNotificationManager.Default.Register();
    }

    /// <returns>显式注册成功 true；打包形态（双参抛异常）或找不到图标时 false（调用方退回无参注册）。</returns>
    private static bool TryRegisterWithIdentity()
    {
        try
        {
            var logo = Path.Combine(AppContext.BaseDirectory, "Assets", "Square44x44Logo.png");
            if (!File.Exists(logo))
            {
                return false;
            }
            Microsoft.Windows.AppNotifications.AppNotificationManager.Default.Register(SenderName, new Uri(logo));
            return true;
        }
        catch (Exception ex)
        {
            System.Diagnostics.Debug.WriteLine(
                $"[shell-toast] explicit Register(displayName, iconUri) failed ({ex.GetType().Name}: {ex.Message}); falling back");
            return false;
        }
    }

    private static void OnNotificationInvoked(
        Microsoft.Windows.AppNotifications.AppNotificationManager sender,
        Microsoft.Windows.AppNotifications.AppNotificationActivatedEventArgs args)
        => ActivateRequested?.Invoke();

    /// <summary>退出时注销 COM 服务：否则注册残留指向已退出的进程，下次激活行为不可预期。</summary>
    internal static void Unregister()
    {
        if (!_registered)
        {
            return;
        }
        try
        {
            Microsoft.Windows.AppNotifications.AppNotificationManager.Default.Unregister();
            _registered = false;
        }
        catch (Exception ex)
        {
            System.Diagnostics.Debug.WriteLine($"[shell-toast] Unregister failed ({ex.GetType().Name}: {ex.Message})");
        }
    }

    internal static void Show(string title, string body)
    {
        try
        {
            EnsureRegistered();
            if (!_registered)
            {
                BalloonFallback?.Invoke(title, body); // dev 无包身份：退化托盘气球
                return;
            }
            var notification = new Microsoft.Windows.AppNotifications.Builder.AppNotificationBuilder()
                .AddText(title, new Microsoft.Windows.AppNotifications.Builder.AppNotificationTextProperties().SetMaxLines(1))
                .AddText(body)
                .BuildNotification();
            Microsoft.Windows.AppNotifications.AppNotificationManager.Default.Show(notification);
        }
        catch (Exception ex)
        {
            System.Diagnostics.Debug.WriteLine($"[shell-toast] Show failed ({ex.GetType().Name}: {ex.Message})");
        }
    }
}
