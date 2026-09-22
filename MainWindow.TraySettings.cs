using System;
using System.IO;
using System.Text.Json;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Blade2;

/// <summary>
/// 托盘与退出设置 + 窗口材质（壳本地配置，不进内核）。
/// 开关：显示托盘图标 / 最小化时隐藏到托盘 / 关闭时最小化到托盘 / 启用系统通知。
/// 持久化走 <c>DataHome/shell.json</c>（与背景皮肤 shell-skin.img 同目录，壳独占、与内核数据隔离）。
/// 最小化/关闭到托盘只在托盘图标实际显示时生效，否则窗口藏了就找不回来。
/// 材质与托盘同文件：两者都是启动时（内核连接前）就要生效的壳本地偏好。
/// </summary>
public partial class MainWindow
{
    /// <summary>窗口材质 id 全集（shell.json material 字段的合法值）。
    /// mica = 默认；mica-alt = 着色更强的 Mica 变体；acrylic = 桌面亚克力；none = 关闭半透明。</summary>
    private const string MaterialMica = "mica";
    private const string MaterialMicaAlt = "mica-alt";
    private const string MaterialAcrylic = "acrylic";
    private const string MaterialNone = "none";

    /// <summary>气泡材质 id 全集（shell.json bubbleMaterial 字段的合法值）。
    /// translucent = 半透明实色面（默认，背后是视频/壁纸就直接透出来）；
    /// acrylic = 系统 acrylic 材质（WinUI 3 自带 AcrylicBrush，不自己搭）；
    /// follow = 跟随窗口材质：mica/mica-alt 出平面、acrylic 出玻璃、none 出纯色卡。</summary>
    private const string BubbleMaterialTranslucent = "translucent";
    private const string BubbleMaterialAcrylic = "acrylic";
    private const string BubbleMaterialFollow = "follow";

    /// <summary>气泡不透明度取值范围（shell.json bubbleOpacity：0.2–1.0）。</summary>
    private const double BubbleOpacityMin = 0.2;
    private const double BubbleOpacityMax = 1.0;
    private const double BubbleOpacityDefault = 0.6;

    private sealed class ShellOptions
    {
        public bool ShowTrayIcon { get; set; } = true;
        public bool MinimizeToTray { get; set; }
        public bool CloseToTray { get; set; }
        public bool ShowNotifications { get; set; } = true;
        public string Material { get; set; } = MaterialMica;
        public string BubbleMaterial { get; set; } = BubbleMaterialTranslucent;
        public double BubbleOpacity { get; set; } = BubbleOpacityDefault;
        /// <summary>已就此版本弹过更新通知的 Release tag（空 = 没提醒过；每个新版本只提醒一次）。</summary>
        public string RemindedUpdateTag { get; set; } = "";
    }

    private ShellOptions _shellOptions = new();
    // 显式退出放行：托盘菜单「退出」与关于页「退出并安装」走此标志，绕过关闭拦截。
    private bool _allowClose;
    // 开关互锁时程序化回写 IsOn（会触发 Toggled），用它防递归。
    private bool _syncingTrayToggles;
    private ToggleSwitch? _trayShowToggle;
    private ToggleSwitch? _trayMinimizeToggle;
    private ToggleSwitch? _trayCloseToggle;
    private ToggleSwitch? _trayNotifyToggle;

    private string TrayOptionsFile => Path.Combine(DataHome, "shell.json");

    /// <summary>读壳本地配置（托盘开关 + 窗口材质）。构造函数里最先调用：
    /// 材质要用于随后的 ApplyBackdrop，托盘开关要赶在托盘图标创建之前。</summary>
    private void LoadShellOptions()
    {
        try
        {
            if (File.Exists(TrayOptionsFile))
            {
                using var doc = JsonDocument.Parse(File.ReadAllText(TrayOptionsFile));
                var root = doc.RootElement;
                _shellOptions = new ShellOptions
                {
                    ShowTrayIcon = GetShellFlag(root, "showTrayIcon", true),
                    MinimizeToTray = GetShellFlag(root, "minimizeToTray", false),
                    CloseToTray = GetShellFlag(root, "closeToTray", false),
                    ShowNotifications = GetShellFlag(root, "showNotifications", true),
                    Material = GetShellMaterial(root),
                    BubbleMaterial = GetShellBubbleMaterial(root),
                    BubbleOpacity = GetShellBubbleOpacity(root),
                    RemindedUpdateTag = GetShellText(root, "remindedUpdateTag"),
                };
            }
        }
        catch (Exception) { } // 配置损坏 = 回默认，不影响启动
        _shellMaterial = _shellOptions.Material;
        _bubbleMaterial = _shellOptions.BubbleMaterial;
        _bubbleOpacity = _shellOptions.BubbleOpacity;
    }

    /// <summary>材质字段读取：缺失/未知值回 mica（旧版 shell.json 没有这个字段）。</summary>
    private static string GetShellMaterial(JsonElement root)
    {
        if (root.ValueKind == JsonValueKind.Object &&
            root.TryGetProperty("material", out var el) &&
            el.ValueKind == JsonValueKind.String)
        {
            var id = el.GetString() ?? "";
            if (id is MaterialMica or MaterialMicaAlt or MaterialAcrylic or MaterialNone)
            {
                return id;
            }
        }
        return MaterialMica;
    }

    /// <summary>气泡材质字段读取：缺失/未知值回 translucent（旧版 shell.json 没有这个字段）。</summary>
    private static string GetShellBubbleMaterial(JsonElement root)
    {
        if (root.ValueKind == JsonValueKind.Object &&
            root.TryGetProperty("bubbleMaterial", out var el) &&
            el.ValueKind == JsonValueKind.String)
        {
            var id = el.GetString() ?? "";
            if (id is BubbleMaterialTranslucent or BubbleMaterialAcrylic or BubbleMaterialFollow)
            {
                return id;
            }
        }
        return BubbleMaterialTranslucent;
    }

    /// <summary>气泡不透明度读取：缺失/非数字/越界一律夹回 0.2–1.0（滑块步进的合法域）。</summary>
    private static double GetShellBubbleOpacity(JsonElement root)
    {
        if (root.ValueKind == JsonValueKind.Object &&
            root.TryGetProperty("bubbleOpacity", out var el) &&
            (el.ValueKind == JsonValueKind.Number) &&
            el.TryGetDouble(out var value))
        {
            return Math.Clamp(value, BubbleOpacityMin, BubbleOpacityMax);
        }
        return BubbleOpacityDefault;
    }

    private static bool GetShellFlag(JsonElement root, string name, bool fallback)
        => root.ValueKind == JsonValueKind.Object &&
            root.TryGetProperty(name, out var el) &&
            (el.ValueKind == JsonValueKind.True || el.ValueKind == JsonValueKind.False)
            ? el.GetBoolean()
            : fallback;

    /// <summary>字符串字段读取：缺失/非字符串回空串（旧版 shell.json 没有这个字段）。</summary>
    private static string GetShellText(JsonElement root, string name)
        => root.ValueKind == JsonValueKind.Object &&
            root.TryGetProperty(name, out var el) &&
            el.ValueKind == JsonValueKind.String
            ? el.GetString() ?? ""
            : "";

    /// <summary>记下「就该版本弹过更新通知」的 tag：同一版本不再提醒（落盘，重启后仍生效）。</summary>
    private void SetRemindedUpdateTag(string tag)
    {
        _shellOptions.RemindedUpdateTag = tag;
        SaveShellOptions();
    }

    private void SaveShellOptions()
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(TrayOptionsFile)!);
            // camelCase 命名策略必须与读取端（GetShellFlag/GetShellMaterial 的小写字段名）一致：
            // 默认策略写出来是 PascalCase（"ShowTrayIcon"），读取永远命中不了，等于不落盘。
            File.WriteAllText(TrayOptionsFile, JsonSerializer.Serialize(_shellOptions,
                new JsonSerializerOptions
                {
                    WriteIndented = true,
                    PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
                }));
        }
        catch (Exception) { } // 落盘失败不影响当前会话的行为
    }

    /// <summary>接最小化/关闭拦截（配置已由构造函数的 LoadShellOptions 读入）。</summary>
    private void WireTrayBehavior()
    {
        try
        {
            AppWindow.Changed += (_, args) =>
            {
                try
                {
                    if (!args.DidPresenterChange)
                    {
                        return;
                    }
                    // 最小化到托盘：藏窗口（任务栏按钮消失），托盘图标仍在，点托盘恢复。
                    if (AppWindow.Presenter is OverlappedPresenter op &&
                        op.State == OverlappedPresenterState.Minimized &&
                        _shellOptions.MinimizeToTray && _trayAdded)
                    {
                        AppWindow.Hide();
                    }
                }
                catch (Exception) { }
            };
            AppWindow.Closing += (_, args) =>
            {
                try
                {
                    // 关闭到托盘：取消关闭、藏窗口；显式退出（托盘菜单/更新安装）放行。
                    if (!_allowClose && _shellOptions.CloseToTray && _trayAdded)
                    {
                        args.Cancel = true;
                        AppWindow.Hide();
                    }
                }
                catch (Exception) { }
            };
        }
        catch (Exception) { } // AppWindow 不可用时回到原来的直接关闭行为
    }

    /// <summary>托盘菜单「退出」与任何需要真正结束进程的路径走这里（先放行关闭拦截）。</summary>
    private void QuitShell()
    {
        _allowClose = true;
        Close();
    }

    /// <summary>通用分区里的「托盘与退出」卡：三个开关即时生效、无需保存按钮。</summary>
    private void RenderTrayCard()
    {
        _trayShowToggle = null;
        _trayMinimizeToggle = null;
        _trayCloseToggle = null;
        _syncingTrayToggles = false;

        var card = NewCard(L("托盘与退出"), L("最小化 / 关闭窗口时的去向"));
        card.Children.Add(MakeRow(
            L("显示托盘图标"),
            L("关闭后仍可从托盘恢复窗口；托盘右键菜单可退出"),
            MakeTrayToggle(
                _shellOptions.ShowTrayIcon,
                "Setting_tray_showIcon",
                L("显示托盘图标"),
                on => SetShowTrayIcon(on, syncUi: true))));
        AddDivider(card);
        card.Children.Add(MakeRow(
            L("最小化时隐藏到托盘"),
            L("点最小化按钮后窗口藏进托盘，不占任务栏"),
            MakeTrayToggle(
                _shellOptions.MinimizeToTray,
                "Setting_tray_minimizeToTray",
                L("最小化时隐藏到托盘"),
                on => SetMinimizeToTray(on, syncUi: true))));
        AddDivider(card);
        card.Children.Add(MakeRow(
            L("关闭时最小化到托盘"),
            L("点关闭按钮后窗口藏进托盘继续运行，用托盘菜单退出"),
            MakeTrayToggle(
                _shellOptions.CloseToTray,
                "Setting_tray_closeToTray",
                L("关闭时最小化到托盘"),
                on => SetCloseToTray(on, syncUi: true))));
    }

    /// <summary>通用分区里的「系统通知」卡：一个开关即时生效、无需保存按钮（shell.json 持久化）。</summary>
    private void RenderNotificationCard()
    {
        _trayNotifyToggle = null;
        _syncingTrayToggles = false;

        var card = NewCard(L("系统通知"), L("任务完成、审批请求等关键时刻的 Windows 通知"));
        card.Children.Add(MakeRow(
            L("启用系统通知"),
            L("窗口不在前台时提醒；点击通知可回到 Blade²"),
            MakeTrayToggle(
                _shellOptions.ShowNotifications,
                "Setting_tray_notifications",
                L("启用系统通知"),
                on => SetShowNotifications(on))));
    }

    private void SetShowNotifications(bool on)
    {
        _shellOptions.ShowNotifications = on;
        SaveShellOptions();
    }

    private ToggleSwitch MakeTrayToggle(bool isOn, string automationId, string name, Action<bool> apply)
    {
        var toggle = Aut(new ToggleSwitch
        {
            IsOn = isOn,
            OnContent = L("开"),
            OffContent = L("关"),
            Style = AppStyle("SettingsToggleSwitchStyle"),
        }, automationId, name);
        toggle.Toggled += (_, _) =>
        {
            try
            {
                if (_syncingTrayToggles)
                {
                    return;
                }
                apply(toggle.IsOn);
            }
            catch (Exception) { } // 事件入口兜底
        };
        if (automationId == "Setting_tray_showIcon")
        {
            _trayShowToggle = toggle;
        }
        else if (automationId == "Setting_tray_minimizeToTray")
        {
            _trayMinimizeToggle = toggle;
        }
        else if (automationId == "Setting_tray_notifications")
        {
            _trayNotifyToggle = toggle;
        }
        else
        {
            _trayCloseToggle = toggle;
        }
        return toggle;
    }

    private void SetShowTrayIcon(bool on, bool syncUi)
    {
        _shellOptions.ShowTrayIcon = on;
        SaveShellOptions();
        if (on)
        {
            TrayAdd();
        }
        else
        {
            // 藏托盘时顺带关掉藏窗口的两项：否则窗口藏了就找不回来。
            _shellOptions.MinimizeToTray = false;
            _shellOptions.CloseToTray = false;
            SaveShellOptions();
            TrayRemove();
            if (syncUi)
            {
                SyncTrayToggle(_trayMinimizeToggle, false);
                SyncTrayToggle(_trayCloseToggle, false);
            }
        }
    }

    private void SetMinimizeToTray(bool on, bool syncUi)
    {
        _shellOptions.MinimizeToTray = on;
        if (on)
        {
            EnsureTrayVisible(syncUi);
        }
        SaveShellOptions();
    }

    private void SetCloseToTray(bool on, bool syncUi)
    {
        _shellOptions.CloseToTray = on;
        if (on)
        {
            EnsureTrayVisible(syncUi);
        }
        SaveShellOptions();
    }

    /// <summary>藏窗口类选项依赖托盘图标：托盘关着时自动打开，保证窗口永远找得回来。</summary>
    private void EnsureTrayVisible(bool syncUi)
    {
        if (_shellOptions.ShowTrayIcon)
        {
            return;
        }
        _shellOptions.ShowTrayIcon = true;
        TrayAdd();
        if (syncUi)
        {
            SyncTrayToggle(_trayShowToggle, true);
        }
    }

    private void SyncTrayToggle(ToggleSwitch? toggle, bool value)
    {
        if (toggle is null || toggle.IsOn == value)
        {
            return;
        }
        _syncingTrayToggles = true;
        try
        {
            toggle.IsOn = value;
        }
        finally
        {
            _syncingTrayToggles = false;
        }
    }
}
