using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Net;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

namespace Blade2.Dsh;

/// <summary>
/// dsh 宠物插件（@linxin666/dsh-pet，Codex Pet 兼容）的 HTTP API 客户端。
/// 路由契约见该包 src/routes.ts：GET /api/pet/{state,pets,diagnostics}，
/// POST /api/pet/{set-pet,set-config,set-visible,interact}；宠物素材走
/// /pet/&lt;id&gt;/pet.json、/pet/&lt;id&gt;/&lt;图集&gt;、/pet/&lt;id&gt;/previews/&lt;图&gt;。
/// 全部走 <see cref="DshRpcClient"/> 已握手的 HttpClient（内核 webserver 有
/// cookie 鉴权，裸客户端连不上回环路由）。
/// 本类永不向调用方抛网络异常：插件未安装/内核重启中时返回 null/false，
/// 由 <see cref="Status"/> 表达「路由在不在」，UI 据此给出安装提示。
/// </summary>
public sealed class DshPetClient
{
    private readonly Func<DshRpcClient?> _rpc;

    public DshPetClient(Func<DshRpcClient?> rpc) => _rpc = rpc;

    /// <summary>插件路由可用性（首次成功/404 探测后确定）。</summary>
    public PetRouteStatus Status { get; private set; } = PetRouteStatus.Unknown;

    private static readonly JsonSerializerOptions Json = new()
    {
        PropertyNameCaseInsensitive = true,
        NumberHandling = System.Text.Json.Serialization.JsonNumberHandling.AllowReadingFromString,
    };

    // ---------------- 读 ----------------

    /// <summary>宠物清单（GET /api/pet/pets）。插件不在时返回 null。</summary>
    public async Task<IReadOnlyList<PetDefinition>?> GetPetsAsync(CancellationToken ct = default)
    {
        var text = await GetAsync("api/pet/pets", ct);
        if (text is null) return null;
        try
        {
            using var doc = JsonDocument.Parse(text);
            if (doc.RootElement.ValueKind != JsonValueKind.Array) return null;
            var list = new List<PetDefinition>();
            foreach (var el in doc.RootElement.EnumerateArray())
            {
                var pet = ParsePet(el);
                if (pet is not null) list.Add(pet);
            }
            return list;
        }
        catch (JsonException)
        {
            return null;
        }
    }

    private static PetDefinition? ParsePet(JsonElement el)
    {
        if (el.ValueKind != JsonValueKind.Object) return null;
        var id = Str(el, "id") ?? "";
        if (id == "") return null;
        var cellW = 192;
        var cellH = 208;
        if (el.TryGetProperty("cell", out var cell) && cell.ValueKind == JsonValueKind.Object)
        {
            cellW = cell.TryGetProperty("width", out var w) && w.TryGetInt32(out var wv) ? wv : 192;
            cellH = cell.TryGetProperty("height", out var h) && h.TryGetInt32(out var hv) ? hv : 208;
        }
        var columns = el.TryGetProperty("columns", out var c) && c.TryGetInt32(out var cv) ? cv : 8;
        var rows = IntArray(el, "rows") ?? new[] { 6, 8, 8, 4, 5, 8, 6, 6, 6 };
        var atlasRows = el.TryGetProperty("atlasRows", out var ar) && ar.TryGetInt32(out var arv) ? arv : 9;

        var tracks = new Dictionary<string, PetTrack>(StringComparer.Ordinal);
        if (el.TryGetProperty("tracks", out var tracksEl) && tracksEl.ValueKind == JsonValueKind.Object)
        {
            foreach (var prop in tracksEl.EnumerateObject())
            {
                if (prop.Value.ValueKind != JsonValueKind.Object) continue;
                var frames = IntArray(prop.Value, "frames") ?? Array.Empty<int>();
                var durations = IntArray(prop.Value, "durations") ?? Array.Empty<int>();
                var loop = prop.Value.TryGetProperty("loop", out var l) && l.ValueKind == JsonValueKind.True;
                var fallback = prop.Value.TryGetProperty("fallback", out var f) && f.ValueKind == JsonValueKind.String
                    ? f.GetString() : null;
                tracks[prop.Name] = new PetTrack(frames, durations, loop, fallback);
            }
        }

        return new PetDefinition(
            id,
            Str(el, "displayName") ?? id,
            Str(el, "description") ?? "",
            Str(el, "renderer") ?? "sprite2d",
            cellW, cellH, columns, rows, atlasRows,
            tracks,
            Str(el, "atlasUrl") ?? "");
    }

    /// <summary>当前宠物状态（GET /api/pet/state）。</summary>
    public async Task<PetStateView?> GetStateAsync(CancellationToken ct = default)
    {
        var text = await GetAsync("api/pet/state", ct);
        if (text is null) return null;
        try
        {
            using var doc = JsonDocument.Parse(text);
            var root = doc.RootElement;
            if (root.ValueKind != JsonValueKind.Object) return null;
            var petId = "";
            if (root.TryGetProperty("pet", out var pet) && pet.ValueKind == JsonValueKind.Object)
                petId = Str(pet, "id") ?? "";
            var visible = false;
            var size = 0;
            if (root.TryGetProperty("display", out var display) && display.ValueKind == JsonValueKind.Object)
            {
                visible = display.TryGetProperty("visible", out var v) && v.ValueKind == JsonValueKind.True;
                size = display.TryGetProperty("size", out var s) && s.TryGetInt32(out var sv) ? sv : 0;
            }
            return new PetStateView(
                Str(root, "animation") ?? "idle",
                Str(root, "phase") ?? "idle",
                petId,
                Str(root, "name") ?? "",
                visible,
                size);
        }
        catch (JsonException)
        {
            return null;
        }
    }

    /// <summary>注册表诊断（GET /api/pet/diagnostics）：清单加载的 error/warning 列表。</summary>
    public async Task<IReadOnlyList<PetDiagnostic>> GetDiagnosticsAsync(CancellationToken ct = default)
    {
        var text = await GetAsync("api/pet/diagnostics", ct);
        if (text is null) return Array.Empty<PetDiagnostic>();
        try
        {
            using var doc = JsonDocument.Parse(text);
            if (!doc.RootElement.TryGetProperty("diagnostics", out var arr) ||
                arr.ValueKind != JsonValueKind.Array)
            {
                return Array.Empty<PetDiagnostic>();
            }
            var list = new List<PetDiagnostic>();
            foreach (var item in arr.EnumerateArray())
            {
                list.Add(new PetDiagnostic(
                    item.TryGetProperty("level", out var l) ? l.GetString() ?? "warning" : "warning",
                    item.TryGetProperty("source", out var s) ? s.GetString() ?? "" : "",
                    item.TryGetProperty("message", out var m) ? m.GetString() ?? "" : ""));
            }
            return list;
        }
        catch (JsonException)
        {
            return Array.Empty<PetDiagnostic>();
        }
    }

    /// <summary>下载宠物素材字节（图集/预览图，走 /pet/&lt;id&gt;/... 资产路由）。</summary>
    public async Task<byte[]?> GetAssetBytesAsync(string url, CancellationToken ct = default)
    {
        var rpc = _rpc();
        if (rpc is null || url is null) return null;
        try
        {
            return await rpc.GetBytesAsync(url, ct);
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or IOException or InvalidOperationException)
        {
            return null;
        }
    }

    // ---------------- 写 ----------------

    /// <summary>切换宠物（POST /api/pet/set-pet）。返回 false = 目标 id 不认识。</summary>
    public async Task<bool> SetPetAsync(string petId, CancellationToken ct = default)
    {
        var body = $"{{\"petId\":{JsonSerializer.Serialize(petId, Json)}}}";
        var text = await PostAsync("api/pet/set-pet", body, ct);
        if (text is null) return false;
        try
        {
            using var doc = JsonDocument.Parse(text);
            return doc.RootElement.TryGetProperty("ok", out var ok) && ok.ValueKind == JsonValueKind.True;
        }
        catch (JsonException)
        {
            return false;
        }
    }

    /// <summary>更新显示配置（POST /api/pet/set-config）：尺寸/显隐。位置由壳自绘，不持久化。</summary>
    public async Task<bool> SetConfigAsync(double? size = null, bool? visible = null, CancellationToken ct = default)
    {
        var parts = new List<string>(2);
        if (size.HasValue)
            parts.Add("\"size\":" + Math.Round(size.Value).ToString(CultureInfo.InvariantCulture));
        if (visible.HasValue)
            parts.Add($"\"visible\":{(visible.Value ? "true" : "false")}");
        if (parts.Count == 0) return true;
        return await PostAsync("api/pet/set-config", "{" + string.Join(',', parts) + "}", ct) is not null;
    }

    /// <summary>显隐开关（POST /api/pet/set-visible，set-config 的 visible 等价物）。</summary>
    public Task<bool> SetVisibleAsync(bool visible, CancellationToken ct = default)
        => SetConfigAsync(size: null, visible: visible, ct: ct);

    /// <summary>互动（POST /api/pet/interact）：pet=点击，feed=喂小鱼干。壳只发不显示回执。</summary>
    public async Task<string?> InteractAsync(string kind = "pet", CancellationToken ct = default)
    {
        var body = $"{{\"kind\":{JsonSerializer.Serialize(kind, Json)}}}";
        var text = await PostAsync("api/pet/interact", body, ct);
        if (text is null) return null;
        try
        {
            using var doc = JsonDocument.Parse(text);
            return doc.RootElement.ValueKind == JsonValueKind.Object ? Str(doc.RootElement, "reaction") : null;
        }
        catch (JsonException)
        {
            return null;
        }
    }

    // ---------------- 传输 ----------------

    private static string? Str(JsonElement el, string name)
        => el.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;

    private static int[]? IntArray(JsonElement el, string name)
    {
        if (!el.TryGetProperty(name, out var v) || v.ValueKind != JsonValueKind.Array) return null;
        var list = new List<int>(v.GetArrayLength());
        foreach (var item in v.EnumerateArray())
            if (item.TryGetInt32(out var n)) list.Add(n);
        return list.ToArray();
    }

    private async Task<string?> GetAsync(string url, CancellationToken ct)
    {
        var rpc = _rpc();
        if (rpc is null)
        {
            Status = PetRouteStatus.Missing;
            return null;
        }
        try
        {
            var text = await rpc.GetTextAsync(url, ct);
            Status = PetRouteStatus.Available;
            return text;
        }
        catch (HttpRequestException ex) when (ex.StatusCode == HttpStatusCode.NotFound)
        {
            // 路由不存在 = 插件未安装或未启用（cordis bundle 未选入）
            Status = PetRouteStatus.Missing;
            return null;
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or IOException or InvalidOperationException)
        {
            return null;
        }
    }

    private async Task<string?> PostAsync(string url, string json, CancellationToken ct)
    {
        var rpc = _rpc();
        if (rpc is null) return null;
        try
        {
            var text = await rpc.PostJsonAsync(url, json, ct);
            Status = PetRouteStatus.Available;
            return text;
        }
        catch (HttpRequestException ex) when (ex.StatusCode == HttpStatusCode.NotFound)
        {
            Status = PetRouteStatus.Missing;
            return null;
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or IOException or InvalidOperationException)
        {
            return null;
        }
    }
}

/// <summary>宠物插件路由的可用性。</summary>
public enum PetRouteStatus
{
    /// <summary>还没探过（设置页未打开或内核刚起）。</summary>
    Unknown,
    /// <summary>/api/pet/* 有响应：插件已挂载。</summary>
    Available,
    /// <summary>404：插件没装/没选入，需要引导安装或重启内核。</summary>
    Missing,
}

/// <summary>
/// 一只宠物（GET /api/pet/pets 的元素，registry.ts 的 PetDefinition）。
/// 图集帧动画的轨道已完全解析（帧列号 + 每帧毫秒 + 循环/回退），壳侧据此
/// 自绘图集帧；live2d 宠物 renderer=live2d、没有可自绘的轨道，壳不渲染。
/// 注意 renderer 的 2D 名有 sprite2d / frames2d 两种（见 ShellRenderable）。
/// </summary>
public sealed record PetDefinition(
    string Id,
    string DisplayName,
    string Description,
    string Renderer,
    int CellWidth,
    int CellHeight,
    int Columns,
    IReadOnlyList<int> Rows,
    int AtlasRows,
    IReadOnlyDictionary<string, PetTrack> Tracks,
    string AtlasUrl)
{
    /// <summary>
    /// 壳能否自绘：图集帧动画（renderer=sprite2d/frames2d，两者是同一种渲染的
    /// 新旧名——插件 0.3.x 起 2D 宠物统一报 sprite2d）。live2d 需 cubism 运行时，
    /// 交给插件的 Web UI。按「非 live2d」判而不是枚举 2D 名：插件改渲染器名时
    /// 不至于把宠物全灭（2026-09-21 事故即 frames2d→sprite2d 改名所致）。
    /// </summary>
    public bool ShellRenderable =>
        Renderer != "live2d" && Tracks.Count > 0 && CellWidth > 0 && CellHeight > 0;
}

/// <summary>一条动画轨道：帧列号序列 + 每帧时长（ms）+ 循环与回退。</summary>
public sealed record PetTrack(
    IReadOnlyList<int> Frames,
    IReadOnlyList<int> DurationsMs,
    bool Loop,
    string? Fallback);

/// <summary>
/// GET /api/pet/state 的裁剪视图：壳自绘只需要动画/相位/显隐/尺寸，
/// 其余（affinity、treats、气泡栈、gameplay）留给 Web UI，不在此建模。
/// </summary>
public sealed record PetStateView(
    string Animation,
    string Phase,
    string PetId,
    string PetName,
    bool Visible,
    int Size);

/// <summary>注册表诊断条目（level = error | warning）。</summary>
public sealed record PetDiagnostic(string Level, string Source, string Message);
