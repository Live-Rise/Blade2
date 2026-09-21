using System;
using System.Collections.Generic;
using System.IO;
using System.Net;
using System.Net.Http;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

namespace Blade2.Dsh;

/// <summary>
/// Wallpaper Engine 壁纸源（@baiiii/dsh-wallpaper-local，默认安装的 dsh 插件）的
/// HTTP API 客户端。路由契约见该包 lib/index.js：GET /dsh-wallpaper/api/list 返回
/// 本机 WE 创意工坊（steamapps/workshop/content/431960）里的壁纸清单，
/// /dsh-wallpaper/video/&lt;工坊目录&gt; 带 Range 流式吐视频壁纸的 mp4/webm，
/// /dsh-wallpaper/{thumb,full}/&lt;id&gt; 吐缩略图与原图。插件 frontend 只注入
/// dsh Web 界面，壳用不到；壳只消费这些服务端路由，把它们当又一个皮肤素材源。
/// 全部走 <see cref="DshRpcClient"/> 已握手的 HttpClient（内核 webserver 有
/// cookie 鉴权，裸客户端连不上回环路由）。
/// 本类永不向调用方抛网络异常：插件未安装/内核重启中/本机没装 WE 时返回 null，
/// 由 <see cref="Status"/> 与 <see cref="LastError"/> 表达原因，UI 据此给提示。
/// </summary>
public sealed class DshWallpaperClient
{
    private readonly Func<DshRpcClient?> _rpc;

    public DshWallpaperClient(Func<DshRpcClient?> rpc) => _rpc = rpc;

    /// <summary>插件路由可用性（首次探测后确定）。</summary>
    public WallpaperRouteStatus Status { get; private set; } = WallpaperRouteStatus.Unknown;

    /// <summary>路由在但取数失败时的原因（插件回的错误文案，可能为 null）。</summary>
    public string? LastError { get; private set; }

    /// <summary>最近一次非 200/404 的 HTTP 状态码（0 = 没有）。</summary>
    public int LastStatus { get; private set; }

    private static readonly JsonSerializerOptions Json = new()
    {
        PropertyNameCaseInsensitive = true,
        NumberHandling = System.Text.Json.Serialization.JsonNumberHandling.AllowReadingFromString,
    };

    /// <summary>壁纸清单（GET dsh-wallpaper/api/list）。插件不在时返回 null。</summary>
    public async Task<WallpaperList?> GetListAsync(CancellationToken ct = default)
    {
        var rpc = _rpc();
        if (rpc is null)
        {
            Status = WallpaperRouteStatus.Missing;
            return null;
        }
        try
        {
            var (status, body) = await rpc.GetRawAsync("dsh-wallpaper/api/list", ct);
            if (status == (int)HttpStatusCode.NotFound)
            {
                // 路由不存在 = 插件未安装或 bundle 未选入（离线首启等）
                Status = WallpaperRouteStatus.Missing;
                LastError = null;
                return null;
            }
            if (status != 200)
            {
                // 路由在但扫不动目录（典型：本机没装 WE，工坊目录不存在）——插件自己
                // 的 catch 会回 500 {error}，把原因带给用户。
                Status = WallpaperRouteStatus.Error;
                LastError = ErrorOf(body);
                LastStatus = status;
                return null;
            }
            var list = ParseList(body);
            if (list is null)
            {
                Status = WallpaperRouteStatus.Error;
                LastError = null;
                LastStatus = status;
                return null;
            }
            Status = WallpaperRouteStatus.Available;
            LastError = null;
            return list;
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or IOException or InvalidOperationException)
        {
            Status = WallpaperRouteStatus.Error;
            LastError = ex.Message;
            return null;
        }
    }

    /// <summary>下载壁纸素材到本地文件（视频直落盘，图片缩略图走内存）。</summary>
    public async Task<bool> DownloadToFileAsync(string url, string destPath, CancellationToken ct = default)
    {
        var rpc = _rpc();
        if (rpc is null) return false;
        try
        {
            await rpc.DownloadToFileAsync(NormalizeUrl(url), destPath, ct);
            return true;
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or IOException or InvalidOperationException)
        {
            return false;
        }
    }

    /// <summary>缩略图字节（列表行预览用）。拿不到返回 null，调用方回落字形占位。</summary>
    public async Task<byte[]?> GetThumbBytesAsync(string url, CancellationToken ct = default)
    {
        var rpc = _rpc();
        if (rpc is null) return null;
        try
        {
            var bytes = await rpc.GetBytesAsync(NormalizeUrl(url), ct);
            return bytes is { Length: > 0 } ? bytes : null;
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or IOException or InvalidOperationException)
        {
            // 视频项没有 preview.jpg 时 thumb 路由会把 mp4 当图吐回来，解码失败也走到这里
            return null;
        }
    }

    private static string NormalizeUrl(string url) =>
        string.IsNullOrEmpty(url) ? url : (url[0] == '/' ? url : "/" + url);

    private static string? ErrorOf(string body)
    {
        try
        {
            using var doc = JsonDocument.Parse(body);
            return doc.RootElement.ValueKind == JsonValueKind.Object &&
                   doc.RootElement.TryGetProperty("error", out var e) &&
                   e.ValueKind == JsonValueKind.String
                ? e.GetString()
                : null;
        }
        catch (JsonException)
        {
            return null;
        }
    }

    private static WallpaperList? ParseList(string body)
    {
        try
        {
            using var doc = JsonDocument.Parse(body);
            var root = doc.RootElement;
            if (root.ValueKind != JsonValueKind.Object ||
                !root.TryGetProperty("items", out var items) ||
                items.ValueKind != JsonValueKind.Array)
            {
                return null;
            }
            var list = new List<WallpaperItem>();
            foreach (var el in items.EnumerateArray())
            {
                var item = ParseItem(el);
                if (item is not null) list.Add(item);
            }
            var excluded = root.TryGetProperty("excluded", out var ex) && ex.TryGetInt32(out var n) ? n : 0;
            return new WallpaperList(list, excluded);
        }
        catch (JsonException)
        {
            return null;
        }
    }

    private static WallpaperItem? ParseItem(JsonElement el)
    {
        if (el.ValueKind != JsonValueKind.Object) return null;
        var id = Str(el, "id") ?? "";
        if (id == "") return null;
        return new WallpaperItem(
            id,
            Str(el, "name") ?? id,
            Str(el, "kind") ?? "image",
            Str(el, "thumb") ?? "",
            Str(el, "full") ?? "",
            Str(el, "video") ?? "",
            Int(el, "width"),
            Int(el, "height"));
    }

    private static string? Str(JsonElement el, string name)
        => el.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;

    private static int Int(JsonElement el, string name)
        => el.TryGetProperty(name, out var v) && v.TryGetInt32(out var n) ? n : 0;
}

/// <summary>壁纸路由状态。</summary>
public enum WallpaperRouteStatus
{
    /// <summary>还没探测过。</summary>
    Unknown,
    /// <summary>路由可用。</summary>
    Available,
    /// <summary>路由 404：插件未安装或未挂载。</summary>
    Missing,
    /// <summary>路由在但取数失败（本机未装 WE 等）。</summary>
    Error,
}

/// <summary>一个 WE 壁纸。视频项的素材在 <see cref="VideoUrl"/>，图片/场景贴图项在 <see cref="FullUrl"/>。</summary>
public sealed record WallpaperItem(
    string Id, string Name, string Kind, string ThumbUrl, string FullUrl, string VideoUrl, int Width, int Height)
{
    /// <summary>是否视频壁纸（mp4/webm，走壳的视频皮肤管线）。</summary>
    public bool IsVideo => string.Equals(Kind, "video", StringComparison.OrdinalIgnoreCase);
}

/// <summary>壁纸清单。Excluded = 插件按分辨率/方形裁剪规则隐藏的低质预览图数量。</summary>
public sealed record WallpaperList(IReadOnlyList<WallpaperItem> Items, int Excluded);
