using System;
using System.IO;
using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;

namespace Blade2;

public partial class App : Application
{
    public static Window MainWindow { get; private set; } = null!;

    public App()
    {
        InitializeComponent();
        // stowed exception 的托管侧出口：写文件定位（0x802b000a 教训）。
        // 写完把 Handled 置 true：非致命 XAML COM 异常（0x80004005 等）不让进程带走——
        // 日志已留档，界面可继续操作（曾有一次使用统计页重渲染引爆此类异常无堆栈可用）。
        UnhandledException += (s, e) =>
        {
            try
            {
                File.AppendAllText(
                    Path.Combine(Path.GetTempPath(), "blade2_unhandled.txt"),
                    $"\n=== {DateTime.Now:MM-dd HH:mm:ss} [app] handled={e.Handled} ===\n{e.Message}\n{e.Exception}\n---STACK---\n{e.Exception?.StackTrace}\n---UI-THREAD---\n{Environment.StackTrace}");
            }
            catch { }
            e.Handled = true;
        };
        // 非 UI 线程的裸线程异常出口（只留档，CLR 仍会终止进程，但日志能定位）。
        AppDomain.CurrentDomain.UnhandledException += (s, e) =>
        {
            try
            {
                var ex = e.ExceptionObject as Exception;
                File.AppendAllText(
                    Path.Combine(Path.GetTempPath(), "blade2_unhandled.txt"),
                    $"\n=== {DateTime.Now:MM-dd HH:mm:ss} [appdomain] fatal={e.IsTerminating} ===\n{ex}\n---STACK---\n{ex?.StackTrace}");
            }
            catch { }
        };
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        // 应用未运行时点击系统 toast：Windows 按 Package.appxmanifest 的 COM 服务声明以
        // ----AppNotificationActivated: 拉起本进程，激活参数经 AppInstance 透传。正常建窗并
        // 置前即完成响应；已在运行的实例不走这条路，走 ShellToast.NotificationInvoked。
        var activated = AppInstance.GetCurrent().GetActivatedEventArgs();
        var toastArgument = "";
        if (activated.Kind == ExtendedActivationKind.AppNotification)
        {
            // AppInstance 只给基类 AppActivationArguments，具体参数在 Data 里：
            // AppNotifications 的 AppNotificationActivatedEventArgs，Argument 形如 "action=update"。
            toastArgument = activated.Data is Microsoft.Windows.AppNotifications.AppNotificationActivatedEventArgs notificationArgs
                ? notificationArgs.Argument ?? ""
                : "";
            System.Diagnostics.Debug.WriteLine($"[shell-toast] cold-start via app notification activation: {toastArgument}");
        }
        MainWindow = new MainWindow(toastArgument);
        MainWindow.Activate();
    }
}
