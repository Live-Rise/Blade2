using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;

namespace Blade2.Dsh;

/// <summary>
/// 宠物库与安装器：把 Codex Pet 压缩包（pet.json + 图集）装进
/// $DSH_HOME/pets/&lt;id&gt;/（Blade² 下即 %LocalAppData%\Blade2\pets），
/// 供 @linxin666/dsh-pet 的注册表在下次内核启动时收录。
///
/// 安装入口三种，全部来自用户显式操作：拖放的 zip、浏览选择的 zip、
/// 粘贴的命令行。命令行只解析不执行——识别 petdex/petdex.dev 的 slug、
/// zip 直链和本地 zip 路径，下载与解压由本类自己完成；
/// 任何其它形态（含 curl|unzip 之类管道）一律拒绝，绝不把用户粘贴的
/// 文本送进 shell 或 pnpm。
/// </summary>
public sealed class DshPetStore
{
    /// <summary>zip 体积上限（插件资产路由图集 cap 20MB，留足余量）。</summary>
    private const long MaxZipBytes = 64 * 1024 * 1024;

    /// <summary>下载超时（宠物图集动辄 2MB+，给足时间）。</summary>
    private static readonly TimeSpan DownloadTimeout = TimeSpan.FromSeconds(120);

    /// <summary>pet id 字符集（registry.ts 的 PET_ID_PATTERN）。</summary>
    private static readonly Regex PetIdPattern = new("^[a-z0-9][a-z0-9-]*$", RegexOptions.Compiled);

    /// <summary>zip 里必须跳过的打包垃圾（macOS 资源分叉/系统文件）。</summary>
    private static readonly string[] JunkNames = { "__MACOSX", ".DS_Store", "Thumbs.db", "desktop.ini" };

    private static readonly HttpClient Http = new(new SocketsHttpHandler { AutomaticDecompression = DecompressionMethods.All })
    {
        Timeout = DownloadTimeout,
    };

    static DshPetStore()
    {
        Http.DefaultRequestHeaders.UserAgent.ParseAdd("Blade2/1.0 (+dsh-pet-installer)");
    }

    private readonly string _petsRoot;

    public DshPetStore(string petsRoot) => _petsRoot = petsRoot;

    /// <summary>宠物目录（$DSH_HOME/pets）。</summary>
    public string PetsRoot => _petsRoot;

    // ---------------- 本地库扫描 ----------------

    /// <summary>
    /// 扫描 pets 目录下已安装的宠物（读 pet.json，不碰图集）。
    /// 插件注册表才是权威清单（还含内置宠物与 ~/.codex/pets）；
    /// 这里扫的是「壳自己装过的」，用来标记哪些还没被注册表收录（需重启内核）。
    /// </summary>
    public IReadOnlyList<InstalledPet> ScanLocal()
    {
        var list = new List<InstalledPet>();
        if (!Directory.Exists(_petsRoot)) return list;
        foreach (var dir in Directory.EnumerateDirectories(_petsRoot))
        {
            try
            {
                var name = Path.GetFileName(dir);
                if (name.StartsWith('.')) continue; // .runtime 等插件自用目录
                var manifestPath = Path.Combine(dir, "pet.json");
                if (!File.Exists(manifestPath)) continue;
                var manifest = ReadManifest(manifestPath);
                if (manifest is null) continue;
                var previews = Directory.Exists(Path.Combine(dir, "previews"))
                    ? Directory.EnumerateFiles(Path.Combine(dir, "previews"))
                        .Where(f => f.EndsWith(".gif", StringComparison.OrdinalIgnoreCase) ||
                                    f.EndsWith(".png", StringComparison.OrdinalIgnoreCase) ||
                                    f.EndsWith(".webp", StringComparison.OrdinalIgnoreCase))
                        .Select(f => Path.GetFileName(f)!)
                        .OrderBy(f => f, StringComparer.Ordinal)
                        .ToArray()
                    : Array.Empty<string>();
                list.Add(new InstalledPet(
                    manifest.Id, manifest.DisplayName, manifest.Description, dir,
                    manifest.SpritesheetPath, previews));
            }
            catch (IOException)
            {
                // 单个目录读失败不影响其它条目
            }
        }
        return list.OrderBy(p => p.DisplayName, StringComparer.Ordinal).ToList();
    }

    /// <summary>
    /// Codex 宠物目录（<c>${CODEX_HOME:-~/.codex}/pets</c>，registry.ts 的扫描源之一）
    /// 下已安装的宠物 id。插件注册表同样收录这批，但条目不回带来源，
    /// 设置页的「来源」标记因此自己扫一遍。
    /// </summary>
    public static IReadOnlyList<string> ScanCodexIds()
    {
        var home = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        var raw = Environment.GetEnvironmentVariable("CODEX_HOME");
        raw = string.IsNullOrWhiteSpace(raw) ? Path.Combine(home, ".codex") : raw.Trim();
        // 与 registry.ts 的 codexPetsDir 同规则展开 ~ / ~\
        var root = raw switch
        {
            "~" => home,
            _ => raw.StartsWith("~/", StringComparison.Ordinal) || raw.StartsWith("~\\", StringComparison.Ordinal)
                ? Path.Combine(home, raw[2..])
                : raw,
        };
        var pets = Path.Combine(root, "pets");
        var list = new List<string>();
        if (!Directory.Exists(pets)) return list;
        try
        {
            foreach (var dir in Directory.EnumerateDirectories(pets))
            {
                if (File.Exists(Path.Combine(dir, "pet.json"))) list.Add(Path.GetFileName(dir));
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            // 目录读不到就当没有 Codex 宠物，不影响 Blade² 自己装的那些
        }
        return list;
    }

    // ---------------- 安装：zip 文件 / 字节 ----------------
    /// <summary>安装一个 zip 文件（拖放或浏览选择得到）。</summary>
    public Task<PetInstallResult> InstallZipFileAsync(string zipPath, CancellationToken ct = default)
    {
        byte[] bytes;
        try
        {
            bytes = File.ReadAllBytes(zipPath);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or NotSupportedException or ArgumentException)
        {
            return Task.FromResult(PetInstallResult.Fail($"读不到文件：{ex.Message}"));
        }
        return InstallZipBytesAsync(bytes, ct);
    }

    /// <summary>安装 zip 字节（下载或拖放统一入口）。</summary>
    public Task<PetInstallResult> InstallZipBytesAsync(byte[] bytes, CancellationToken ct = default)
    {
        if (bytes.Length == 0) return Task.FromResult(PetInstallResult.Fail("压缩包是空的。"));
        if (bytes.Length > MaxZipBytes) return Task.FromResult(PetInstallResult.Fail("压缩包超过 64MB，拒绝安装。"));
        // 解压+写盘是纯 IO，丢后台线程，别卡 UI
        return Task.Run(() =>
        {
            try
            {
                using var ms = new MemoryStream(bytes, writable: false);
                using var zip = new ZipArchive(ms, ZipArchiveMode.Read);
                var staged = Stage(zip);
                if (staged.Error is not null) return PetInstallResult.Fail(staged.Error);
                var target = AllocateTargetDir(staged.Manifest!.Id);
                var (finalManifest, manifestJson) = FinalManifest(staged, target);
                Directory.CreateDirectory(target);
                Extract(zip, staged.Prefix, target, manifestJson);
                return PetInstallResult.Succeed(new InstalledPet(
                    finalManifest.Id, finalManifest.DisplayName, finalManifest.Description, target,
                    finalManifest.SpritesheetPath, staged.Previews));
            }
            catch (InvalidDataException)
            {
                return PetInstallResult.Fail("不是有效的 zip 压缩包（Codex 宠物包应为 zip）。");
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or NotSupportedException)
            {
                return PetInstallResult.Fail($"解压失败：{ex.Message}");
            }
        }, ct);
    }

    /// <summary>
    /// 安装一个本地宠物目录（插件自家 CLI「node scripts/dsh-pet install &lt;dir&gt;」的形态：
    /// 校验 pet.json + 图集后拷入 $DSH_HOME/pets/&lt;id&gt;/）。
    /// </summary>
    public async Task<PetInstallResult> InstallDirectoryAsync(string dir, CancellationToken ct = default)
    {
        if (string.IsNullOrWhiteSpace(dir) || !Directory.Exists(dir))
            return PetInstallResult.Fail("目录不存在。");
        var manifestPath = Path.Combine(dir, "pet.json");
        if (!File.Exists(manifestPath))
            return PetInstallResult.Fail("目录里没有 pet.json。");
        var manifest = ReadManifest(manifestPath);
        if (manifest is null) return PetInstallResult.Fail("pet.json 读不出来或格式不对。");
        if (!PetIdPattern.IsMatch(manifest.Id))
            return PetInstallResult.Fail($"pet.json 的 id「{manifest.Id}」不合法（需小写字母/数字/连字符）。");
        var atlas = Path.Combine(dir, manifest.SpritesheetPath.Replace('/', Path.DirectorySeparatorChar));
        if (manifest.SpritesheetPath == "" || !File.Exists(atlas))
            return PetInstallResult.Fail($"图集「{manifest.SpritesheetPath}」不在目录里。");

        var target = AllocateTargetDir(manifest.Id);
        try
        {
            await Task.Run(() =>
            {
                Directory.CreateDirectory(target);
                foreach (var file in Directory.EnumerateFiles(dir, "*", SearchOption.AllDirectories))
                {
                    var rel = Path.GetRelativePath(dir, file).Replace('\\', '/');
                    var segments = rel.Split('/');
                    if (segments.Any(s => JunkNames.Contains(s, StringComparer.OrdinalIgnoreCase) || s.StartsWith('.'))) continue;
                    if (rel.Equals("pet.json", StringComparison.OrdinalIgnoreCase)) continue; // 下方写纠正版
                    var dest = Path.GetFullPath(Path.Combine(target, string.Join(Path.DirectorySeparatorChar.ToString(), segments)));
                    if (!dest.StartsWith(Path.GetFullPath(target) + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase)) continue;
                    Directory.CreateDirectory(Path.GetDirectoryName(dest)!);
                    File.Copy(file, dest, overwrite: true);
                }
                var finalId = Path.GetFileName(target);
                var finalManifest = finalId == manifest.Id ? manifest : manifest with { Id = finalId };
                var json = PatchManifestJson(File.ReadAllText(manifestPath), finalManifest, manifest.SpritesheetPath);
                File.WriteAllText(Path.Combine(target, "pet.json"), json, new UTF8Encoding(false));
            }, ct);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or NotSupportedException)
        {
            return PetInstallResult.Fail($"拷贝失败：{ex.Message}");
        }
        var previews = Directory.Exists(Path.Combine(target, "previews"))
            ? Directory.EnumerateFiles(Path.Combine(target, "previews"))
                .Select(f => Path.GetFileName(f)!).OrderBy(f => f, StringComparer.Ordinal).ToArray()
            : Array.Empty<string>();
        return PetInstallResult.Succeed(new InstalledPet(
            Path.GetFileName(target), manifest.DisplayName, manifest.Description, target,
            manifest.SpritesheetPath, previews));
    }

    /// <summary>从 http(s) 直链下载 zip 并安装。</summary>
    public async Task<PetInstallResult> InstallFromUrlAsync(string url, CancellationToken ct = default)
    {
        if (!Uri.TryCreate(url, UriKind.Absolute, out var uri) ||
            (uri.Scheme != Uri.UriSchemeHttp && uri.Scheme != Uri.UriSchemeHttps))
        {
            return PetInstallResult.Fail("不是有效的 http(s) 链接。");
        }
        byte[] bytes;
        try
        {
            using var resp = await Http.GetAsync(uri, HttpCompletionOption.ResponseHeadersRead, ct);
            if (!resp.IsSuccessStatusCode)
                return PetInstallResult.Fail($"下载失败：服务器返回 {(int)resp.StatusCode}。");
            if (resp.Content.Headers.ContentLength > MaxZipBytes)
                return PetInstallResult.Fail("压缩包超过 64MB，拒绝安装。");
            bytes = await resp.Content.ReadAsByteArrayAsync(ct);
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or IOException)
        {
            return PetInstallResult.Fail($"下载失败：{ex.Message}");
        }
        return await InstallZipBytesAsync(bytes, ct);
    }

    // ---------------- 安装：petdex ----------------

    /// <summary>
    /// 按 petdex.dev 的 slug 安装（清单 API 拿 zipUrl 再下载）。
    /// 清单 4868 只宠物约 1.7MB，按需拉全量后在本地匹配 slug。
    /// </summary>
    public async Task<PetInstallResult> InstallPetdexSlugAsync(string slug, CancellationToken ct = default)
    {
        slug = slug.Trim().ToLowerInvariant();
        if (!PetIdPattern.IsMatch(slug))
            return PetInstallResult.Fail($"「{slug}」不是合法的宠物标识（小写字母/数字/连字符）。");
        string manifestJson;
        try
        {
            using var resp = await Http.GetAsync("https://petdex.dev/api/manifest", ct);
            resp.EnsureSuccessStatusCode();
            manifestJson = await resp.Content.ReadAsStringAsync(ct);
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or IOException)
        {
            return PetInstallResult.Fail($"拉取宠物目录失败：{ex.Message}");
        }
        string? zipUrl = null;
        string? displayName = null;
        try
        {
            using var doc = JsonDocument.Parse(manifestJson);
            if (!doc.RootElement.TryGetProperty("pets", out var pets) || pets.ValueKind != JsonValueKind.Array)
                return PetInstallResult.Fail("宠物目录数据异常。");
            foreach (var pet in pets.EnumerateArray())
            {
                if (pet.TryGetProperty("slug", out var s) &&
                    string.Equals(s.GetString(), slug, StringComparison.OrdinalIgnoreCase))
                {
                    zipUrl = pet.TryGetProperty("zipUrl", out var z) ? z.GetString() : null;
                    displayName = pet.TryGetProperty("displayName", out var d) ? d.GetString() : null;
                    break;
                }
            }
        }
        catch (JsonException)
        {
            return PetInstallResult.Fail("宠物目录数据解析失败。");
        }
        if (zipUrl is null)
            return PetInstallResult.Fail($"宠物目录里没有「{slug}」。可到 petdex.dev 核对标识。");
        var result = await InstallFromUrlAsync(zipUrl, ct);
        return result.Ok && displayName is not null
            ? PetInstallResult.Succeed(result.Pet! with { DisplayName = displayName })
            : result;
    }

    // ---------------- 安装：命令行粘贴 ----------------

    /// <summary>
    /// 解析粘贴的命令行并安装。只认这几种形态，其余一律拒绝：
    ///   petdex install &lt;slug&gt; / npx|pnpm dlx|npm exec petdex install &lt;slug&gt;
    ///   petdex.dev/pets/&lt;slug&gt; 或含 petdex.dev 宠物页链接
    ///   node …/scripts/dsh-pet install &lt;dir&gt;（插件自家 CLI）
    ///   https://.../xxx.zip（含 curl -L/-o 等包装里的单个 zip 直链）
    ///   本地 zip 路径（.zip 结尾）
    /// 命令本身从不执行：下载/解压/拷贝全部由本类完成。
    /// </summary>
    public async Task<PetInstallResult> InstallFromCommandAsync(string command, CancellationToken ct = default)
    {
        var text = (command ?? "").Trim();
        if (text == "") return PetInstallResult.Fail("命令行是空的。");
        // 去掉引号包裹（从终端整行复制常见）
        if (text.Length >= 2 && (text[0] == '"' && text[^1] == '"' || text[0] == '\'' && text[^1] == '\''))
            text = text[1..^1].Trim();

        // 1) petdex CLI 形态：取 install 后的第一个 token 作为 slug
        var cli = Regex.Match(text, @"(?:^|[\s;&|])petdex\s+install\s+([A-Za-z0-9._-]+)", RegexOptions.IgnoreCase);
        if (cli.Success)
            return await InstallPetdexSlugAsync(cli.Groups[1].Value, ct);

        // 2) 插件自家 CLI：node scripts/dsh-pet install <dir>（本地宠物目录）
        var own = Regex.Match(text, @"dsh-pet\s+install\s+(.+?)(?:\s+--?[A-Za-z-]+.*)?$", RegexOptions.IgnoreCase);
        if (own.Success)
        {
            var dir = own.Groups[1].Value.Trim().Trim('"', '\'');
            if (Directory.Exists(dir)) return await InstallDirectoryAsync(dir, ct);
            return PetInstallResult.Fail($"目录「{dir}」不存在。");
        }

        // 3) petdex.dev 宠物页/直链
        var page = Regex.Match(text, @"petdex\.dev/(?:pets/|pet/)?([A-Za-z0-9-]+)", RegexOptions.IgnoreCase);
        if (page.Success && !text.Contains("api/manifest", StringComparison.OrdinalIgnoreCase))
            return await InstallPetdexSlugAsync(page.Groups[1].Value, ct);

        // 4) zip 直链（命令里只允许出现一个 zip URL）
        var zips = Regex.Matches(text, @"https?://[^\s'""<>|]+\.zip(?:\?[^\s'""<>|]*)?", RegexOptions.IgnoreCase)
            .Select(m => m.Value).Distinct(StringComparer.OrdinalIgnoreCase).ToArray();
        if (zips.Length == 1)
            return await InstallFromUrlAsync(zips[0], ct);
        if (zips.Length > 1)
            return PetInstallResult.Fail("命令行里有多个 zip 链接，请只保留一个。");

        // 5) 本地 zip 路径
        var local = Regex.Match(text, @"([A-Za-z]:\\[^\s'""<>|*?]+\.zip|\\\\[^\s'""<>|*?]+\.zip|\.?/[^\s'""<>|*?]+\.zip)", RegexOptions.IgnoreCase);
        if (local.Success && File.Exists(local.Groups[1].Value))
            return await InstallZipFileAsync(local.Groups[1].Value, ct);

        return PetInstallResult.Fail(
            "认不出这个命令。支持：petdex install <宠物标识>、node scripts/dsh-pet install <目录>、" +
            "zip 直链、本地 .zip 路径。为安全起见，粘贴的命令行只会被解析，不会被执行。");
    }

    // ---------------- 删除 ----------------

    /// <summary>
    /// 删除一个已安装宠物（仅限 pets 目录内）。内置宠物装在插件包里，
    /// 不在本目录，传进来也删不到——调用方应先用 API 清单判定来源。
    /// </summary>
    public bool Delete(string petId)
    {
        var dir = Path.Combine(_petsRoot, petId);
        var root = Path.GetFullPath(_petsRoot);
        var full = Path.GetFullPath(dir);
        if (!full.StartsWith(root + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase)) return false;
        if (!Directory.Exists(full)) return false;
        try
        {
            Directory.Delete(full, recursive: true);
            return true;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return false;
        }
    }

    // ---------------- zip 加工 ----------------

    /// <summary>pet.json 清单（安装侧只认这四个字段；其余（tracks/sequences 等）原样保留）。</summary>
    private sealed record PetManifestFile(string Id, string DisplayName, string Description, string SpritesheetPath);

    private sealed class Staged
    {
        public string Prefix = "";                 // zip 内统一顶层目录（有则剥掉）
        public PetManifestFile? Manifest;
        public string ManifestJson = "";           // pet.json 原文（安装时只补 id/spritesheetPath，其余字段原样保留）
        public string? AtlasName;                  // 图集在 zip 内的相对路径（已剥前缀）
        public string[] Previews = Array.Empty<string>();
        public string? Error;
    }

    /// <summary>读 pets 目录里的 pet.json（缺字段给默认值，读不动返回 null）。</summary>
    private static PetManifestFile? ReadManifest(string path)
    {
        try
        {
            using var doc = JsonDocument.Parse(File.ReadAllText(path));
            var root = doc.RootElement;
            var id = root.TryGetProperty("id", out var idEl) && idEl.ValueKind == JsonValueKind.String
                ? idEl.GetString() ?? "" : "";
            if (id == "") return null;
            var name = root.TryGetProperty("displayName", out var nEl) && nEl.ValueKind == JsonValueKind.String
                ? nEl.GetString() ?? id : id;
            var desc = root.TryGetProperty("description", out var dEl) && dEl.ValueKind == JsonValueKind.String
                ? dEl.GetString() ?? "" : "";
            var sheet = root.TryGetProperty("spritesheetPath", out var sEl) && sEl.ValueKind == JsonValueKind.String
                ? sEl.GetString() ?? "" : "";
            return new PetManifestFile(id, name, desc, sheet);
        }
        catch (Exception ex) when (ex is JsonException or IOException or UnauthorizedAccessException)
        {
            return null;
        }
    }

    /// <summary>
    /// 从 zip 里定位 pet.json、图集与预览图。容错点：
    /// ① 单个顶层目录（GitHub release 常见）整体剥掉；
    /// ② pet.json 声称的图集名与实际不符时，改用唯一的图片文件；
    /// ③ __MACOSX/资源分叉等垃圾条目忽略。
    /// </summary>
    private static Staged Stage(ZipArchive zip)
    {
        var staged = new Staged();
        var files = new List<ZipArchiveEntry>();
        foreach (var entry in zip.Entries)
        {
            if (entry.Length == 0 && entry.FullName.EndsWith('/')) continue; // 目录项
            var normalized = entry.FullName.Replace('\\', '/');
            var segments = normalized.Split('/');
            if (segments.Any(s => JunkNames.Contains(s, StringComparer.OrdinalIgnoreCase) ||
                                 s.StartsWith('.')))
            {
                continue;
            }
            if (segments.Any(s => s is ".." or "")) { staged.Error = "压缩包含非法路径条目，拒绝安装。"; return staged; }
            files.Add(entry);
        }
        if (files.Count == 0) { staged.Error = "压缩包里没有文件。"; return staged; }

        // 剥顶层目录：所有条目共享唯一顶层目录时才剥（避免误剥平铺包）
        var topDirs = files.Select(f => f.FullName.Replace('\\', '/').Split('/')[0])
            .Where(s => files.Any(f => f.FullName.Replace('\\', '/').Contains('/')))
            .Distinct(StringComparer.OrdinalIgnoreCase).ToArray();
        if (files.All(f => f.FullName.Replace('\\', '/').Contains('/')) && topDirs.Length == 1)
            staged.Prefix = topDirs[0];

        string Rel(ZipArchiveEntry e) => staged.Prefix == ""
            ? e.FullName.Replace('\\', '/')
            : e.FullName.Replace('\\', '/').Substring(staged.Prefix.Length + 1);

        var manifestEntry = files.FirstOrDefault(f => Rel(f).Equals("pet.json", StringComparison.OrdinalIgnoreCase));
        if (manifestEntry is null)
        {
            staged.Error = "压缩包里找不到 pet.json（Codex 宠物包必须带清单）。";
            return staged;
        }

        PetManifestFile manifest;
        string raw;
        try
        {
            using var stream = manifestEntry.Open();
            using var reader = new StreamReader(stream, Encoding.UTF8, detectEncodingFromByteOrderMarks: true);
            raw = reader.ReadToEnd();
            using var doc = JsonDocument.Parse(raw);
            var root = doc.RootElement;
            var id = root.TryGetProperty("id", out var idEl) && idEl.ValueKind == JsonValueKind.String
                ? idEl.GetString() ?? "" : "";
            if (id == "") { staged.Error = "pet.json 缺 id 字段。"; return staged; }
            if (!PetIdPattern.IsMatch(id))
            {
                var fixedId = Regex.Replace(id.ToLowerInvariant(), "[^a-z0-9-]+", "-").Trim('-');
                if (fixedId == "" || !PetIdPattern.IsMatch(fixedId))
                { staged.Error = $"pet.json 的 id「{id}」不合法（需小写字母/数字/连字符）。"; return staged; }
                id = fixedId;
            }
            var name = root.TryGetProperty("displayName", out var nEl) && nEl.ValueKind == JsonValueKind.String
                ? nEl.GetString() ?? id : id;
            var desc = root.TryGetProperty("description", out var dEl) && dEl.ValueKind == JsonValueKind.String
                ? dEl.GetString() ?? "" : "";
            var sheet = root.TryGetProperty("spritesheetPath", out var sEl) && sEl.ValueKind == JsonValueKind.String
                ? sEl.GetString() ?? "" : "";
            manifest = new PetManifestFile(id, name, desc, sheet);
        }
        catch (Exception ex) when (ex is JsonException or InvalidDataException or IOException)
        {
            staged.Error = $"pet.json 解析失败：{ex.Message}";
            return staged;
        }

        var images = files
            .Where(f => f.FullName.Replace('\\', '/').Split('/').Last().ToLowerInvariant() is var ext &&
                        (ext.EndsWith(".webp") || ext.EndsWith(".png") || ext.EndsWith(".gif") || ext.EndsWith(".jpg") || ext.EndsWith(".jpeg")))
            .ToArray();
        var atlas = images.FirstOrDefault(f => Rel(f).Equals(manifest.SpritesheetPath.Replace('\\', '/'), StringComparison.OrdinalIgnoreCase));
        if (atlas is null)
        {
            // 声称的图集不在包里：只有一个图片文件时认它（petdex 的 petjson.json
            // 与 zip 内 pet.json 偶尔不同名），否则拒绝。
            atlas = images.Length == 1 ? images[0] : null;
            if (atlas is null)
            {
                staged.Error = images.Length == 0
                    ? "压缩包里没有图集（spritesheet.webp/png/gif）。"
                    : "压缩包里有多张图片且 pet.json 的 spritesheetPath 对不上，无法确定图集。";
                return staged;
            }
            manifest = manifest with { SpritesheetPath = Rel(atlas) };
        }
            staged.Manifest = manifest;
            staged.ManifestJson = raw;
            staged.AtlasName = Rel(atlas);
        staged.Previews = files
            .Where(f => Rel(f).StartsWith("previews/", StringComparison.OrdinalIgnoreCase))
            .Select(Rel)
            .ToArray();
        return staged;
    }

    /// <summary>分配目标目录：pets/&lt;id&gt;，重名则 -2/-3…后缀（id 同步改，注册表按 id 索引）。</summary>
    private string AllocateTargetDir(string id)
    {
        var candidate = Path.Combine(_petsRoot, id);
        var suffix = 2;
        while (Directory.Exists(candidate))
            candidate = Path.Combine(_petsRoot, $"{id}-{suffix++}");
        return candidate;
    }

    /// <summary>
    /// 安装侧的最终清单：id 带后缀或图集名被纠正时同步这两项，
    /// 其余字段（cell/columns/rows/atlasRows/tracks/sequences…）**原样保留**——
    /// 早先按四字段重写，非默认网格的宠物（cell 256×256、columns 6 之类）
    /// 会丢掉几何定义，注册表回退默认值后帧裁剪全错位。
    /// </summary>
    private static (PetManifestFile Manifest, string Json) FinalManifest(Staged staged, string dirName)
    {
        var manifest = staged.Manifest!;
        var id = Path.GetFileName(dirName);
        var fixedId = id == manifest.Id ? manifest : manifest with { Id = id };
        return (fixedId, PatchManifestJson(staged.ManifestJson, fixedId, fixedId.SpritesheetPath));
    }

    /// <summary>在原始 pet.json 上就地改 id / spritesheetPath（JSON 节点级改写，其它键一个不动）。</summary>
    private static string PatchManifestJson(string rawJson, PetManifestFile manifest, string spritesheetPath)
    {
        try
        {
            var node = JsonNode.Parse(rawJson) as JsonObject;
            if (node is null) throw new JsonException();
            node["id"] = manifest.Id;
            if (spritesheetPath != "") node["spritesheetPath"] = spritesheetPath;
            return node.ToJsonString();
        }
        catch (Exception ex) when (ex is JsonException or NotSupportedException)
        {
            // 原文解析不动（Stage 已校验过一次，正常到不了这里）：退化成最小清单
            return JsonSerializer.Serialize(new
            {
                id = manifest.Id,
                displayName = manifest.DisplayName,
                description = manifest.Description,
                spritesheetPath,
            });
        }
    }

    /// <summary>
    /// 解压到目标目录。路径逐条净化：只取相对片段、拒 ..，
    /// 且 pet.json 不落原包版本，统一写纠正后的内容。
    /// </summary>
    private static void Extract(ZipArchive zip, string prefix, string target, string manifestJson)
    {
        foreach (var entry in zip.Entries)
        {
            if (entry.Length == 0 && entry.FullName.EndsWith('/')) continue;
            var rel = prefix == "" ? entry.FullName.Replace('\\', '/') : entry.FullName.Replace('\\', '/').Substring(prefix.Length + 1);
            var segments = rel.Split('/');
            if (segments.Any(s => JunkNames.Contains(s, StringComparer.OrdinalIgnoreCase) || s.StartsWith('.'))) continue;
            if (segments.Any(s => s is ".." or "")) continue;
            var safeRel = string.Join('/', segments.Select(Uri.UnescapeDataString));
            var dest = Path.GetFullPath(Path.Combine(target, safeRel));
            if (!dest.StartsWith(Path.GetFullPath(target) + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase)) continue;
            if (safeRel.Equals("pet.json", StringComparison.OrdinalIgnoreCase)) continue; // 统一由下方写纠正版
            Directory.CreateDirectory(Path.GetDirectoryName(dest)!);
            entry.ExtractToFile(dest, overwrite: true);
        }
        File.WriteAllText(Path.Combine(target, "pet.json"), manifestJson, new UTF8Encoding(false));
    }
}

/// <summary>pets 目录里的一只已安装宠物。</summary>
public sealed record InstalledPet(
    string Id,
    string DisplayName,
    string Description,
    string Directory,
    string SpritesheetPath,
    IReadOnlyList<string> Previews);

/// <summary>安装结果（Ok=false 时 Message 是给用户看的原因）。</summary>
public sealed record PetInstallResult(bool Ok, InstalledPet? Pet, string Message)
{
    public static PetInstallResult Fail(string message) => new(false, null, message);
    public static PetInstallResult Succeed(InstalledPet pet) => new(true, pet, "");
}
