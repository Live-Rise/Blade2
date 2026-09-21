using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Text.RegularExpressions;

namespace Blade2.Dsh;

/// <summary>
/// 本机 WE 创意工坊目录（steamapps/workshop/content/431960）的定位与视频预览图查找。
///
/// 为什么壳侧直读本地文件、不走插件 thumb 路由：插件把工坊 item 目录里的 preview.*
/// 记成 thumbSlug，但 /dsh-wallpaper/thumb/&lt;slug&gt; 只服务 THUMBS 预生成目录（本机
/// 为空，实测 404），slug 又与清单 id 对不上，回退分支只会把整个视频文件当图吐出来。
/// 预览图就在本机磁盘上，直读比绕 HTTP 快、也不占带宽；工坊位置的探测逻辑与插件
/// 同源（Steam 注册表 + libraryfolders.vdf 库清单）。
/// </summary>
public static class WeWorkspace
{
    private const string AppId = "431960";
    private const string SteamKey = @"Software\Valve\Steam";
    private const string VideoIdPrefix = "video__";
    private static readonly string[] PreviewExts = { ".jpg", ".jpeg", ".png", ".gif", ".webp", ".bmp" };

    private static string? _root;
    private static bool _probed;

    /// <summary>工坊根目录（进程内只探测一次；本机没装 WE 时为 null）。</summary>
    public static string? Root
    {
        get
        {
            if (!_probed)
            {
                _root = ProbeRoot();
                _probed = true;
            }
            return _root;
        }
    }

    /// <summary>
    /// 视频项的预览图本地路径（工坊 item 目录里的 preview.*，与插件同序枚举）。
    /// 拿不到（没装 WE、目录被移动、没有 preview 文件）时返回 null，调用方回退占位图形。
    /// </summary>
    public static string? PreviewFileFor(string itemId)
    {
        var root = Root;
        if (root is null || !itemId.StartsWith(VideoIdPrefix, StringComparison.Ordinal))
        {
            return null;
        }
        var folder = itemId.Substring(VideoIdPrefix.Length);
        // 目录名直接拼进路径：挡掉分隔符与相对段，防越权读到工坊目录外
        if (folder.Length == 0 || folder.IndexOfAny(new[] { '/', '\\' }) >= 0 || folder.Contains(".."))
        {
            return null;
        }
        var dir = Path.Combine(root, folder);
        foreach (var ext in PreviewExts)
        {
            var candidate = Path.Combine(dir, "preview" + ext);
            if (File.Exists(candidate))
            {
                return candidate;
            }
        }
        return null;
    }

    private static string? ProbeRoot()
    {
        foreach (var library in SteamLibraries())
        {
            var candidate = Path.Combine(library, "steamapps", "workshop", "content", AppId);
            if (Directory.Exists(candidate))
            {
                return candidate;
            }
        }
        return null;
    }

    private static IEnumerable<string> SteamLibraries()
    {
        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var steam in SteamRoots())
        {
            if (Directory.Exists(steam) && seen.Add(steam))
            {
                yield return steam;
            }
            foreach (var lib in ReadLibraryFolders(Path.Combine(steam, "steamapps", "libraryfolders.vdf")))
            {
                if (Directory.Exists(lib) && seen.Add(lib))
                {
                    yield return lib;
                }
            }
        }
    }

    private static IEnumerable<string> SteamRoots()
    {
        var path = ReadRegString(SteamKey, "SteamPath");
        if (path is not null)
        {
            yield return path.Replace('/', '\\');
        }
        var exe = ReadRegString(SteamKey, "SteamExe");
        if (exe is not null)
        {
            var dir = Path.GetDirectoryName(exe);
            if (dir is not null)
            {
                yield return dir;
            }
        }
        yield return @"D:\Steam"; // 插件内置兜底同款：主库默认路径
    }

    /// <summary>libraryfolders.vdf 里每个库一行 "path"，值带 VDF 的双反斜杠转义，正则直取。</summary>
    private static IEnumerable<string> ReadLibraryFolders(string vdfPath)
    {
        if (!File.Exists(vdfPath))
        {
            yield break;
        }
        string text;
        try
        {
            text = File.ReadAllText(vdfPath);
        }
        catch (Exception)
        {
            yield break;
        }
        foreach (Match m in Regex.Matches(text, "\"path\"\\s*\"([^\"]+)\""))
        {
            yield return m.Groups[1].Value.Replace(@"\\", @"\");
        }
    }

    private static string? ReadRegString(string subKey, string value)
    {
        IntPtr handle = IntPtr.Zero;
        try
        {
            if (RegOpenKeyEx(new IntPtr(unchecked((int)0x80000001)), subKey, 0, 0x20019, out handle) != 0)
            {
                return null;
            }
            var size = 0;
            if (RegQueryValueEx(handle, value, IntPtr.Zero, out _, null, ref size) != 0 || size <= 2)
            {
                return null;
            }
            var buffer = new byte[size];
            if (RegQueryValueEx(handle, value, IntPtr.Zero, out _, buffer, ref size) != 0)
            {
                return null;
            }
            return System.Text.Encoding.Unicode.GetString(buffer, 0, size).TrimEnd('\0');
        }
        catch (Exception)
        {
            return null;
        }
        finally
        {
            if (handle != IntPtr.Zero)
            {
                RegCloseKey(handle);
            }
        }
    }

    // ---------------- P/Invoke（advapi32：读 HKCU 不需要 Microsoft.Win32.Registry 包依赖） ----------------

    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern int RegOpenKeyEx(IntPtr hKey, string lpSubKey, int ulOptions, int samDesired, out IntPtr phkResult);

    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern int RegQueryValueEx(IntPtr hKey, string lpValueName, IntPtr lpReserved,
        out int lpType, byte[]? lpData, ref int lpcbData);

    [DllImport("advapi32.dll")]
    private static extern int RegCloseKey(IntPtr hKey);
}
