using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.Json;
using System.Text.Encodings.Web;
using System.Threading;
using System.Threading.Tasks;

namespace Blade2.Dsh;

/// <summary>
/// 默认插件引导器：为 web profile 安装 Blade² 默认插件组合，并把内核侧挂载条目
/// 写进 profile patch 层（$DSH_HOME/profiles/web/cordis.patch.yml）。
///
/// 全部走 dsh 官方插件机制：包经 `dsh plugin --profile web add`（pnpm 装入 profile，
/// bundle 插件自动选入启用），挂载条目用 loader patch 的 insert 语法（与官方
/// apps/cli/config/examples/mcp-memory 参考配置同款）。profile 的 patchReload=live
/// 使 patch 变化由内核热重载，无需重启。
///
/// 壳与插件完全隔离：壳不拥有任何插件逻辑，只 (a) 在包缺失时调内核 CLI 安装、
/// (b) 编辑内核标准 patch 文件。安装失败（离线等）只记录诊断——patch 条目仅在
/// 其引用的包就绪后写入，内核永远能裸启动；下次启动自动重试。
/// </summary>
internal static class DshPluginBootstrap
{
    /// <summary>安装进度：<see cref="Ready"/> 个默认插件包已就绪，共 <see cref="Total"/> 个。</summary>
    internal sealed record PluginInstallProgress(int Ready, int Total);

    /// <summary>web profile 的 profile 名（`dsh web` 的别名等价物，壳启动固定用它）。</summary>
    private const string ProfileName = "web";

    /// <summary>单次 pnpm 安装的超时上限。</summary>
    private static readonly TimeSpan InstallTimeout = TimeSpan.FromSeconds(300);

    /// <summary>
    /// 默认安装清单（2026-09 选型：各品类生态声量最高、维护最稳的版本）。
    /// bundle 插件（自带 dsh.bundle patch，add 后自动选入启用）与库依赖（由
    /// DefaultsPatch 提供的 insert 条目挂载）混排，add 一条命令全部装入。
    /// 记忆包固定版本：npm 官方 latest（DSH 文档测试版 2026.7.4 的后继），与
    /// 上游保持一致；升级时只改 <see cref="MemoryPinnedVersion"/> 一处。
    /// </summary>
    private const string MemoryPackageDir = @"@modelcontextprotocol/server-memory";

    /// <summary>记忆 MCP 服务器的锁定版本（npm 官方 latest）。</summary>
    private const string MemoryPinnedVersion = "2026.8.31";

    /// <summary>记忆包的安装规格（pnpm add 用，含版本；目录名仍是 <see cref="MemoryPackageDir"/>）。</summary>
    private static string MemoryPackageSpec => $"{MemoryPackageDir}@{MemoryPinnedVersion}";

    private static readonly string[] RequiredPackages =
    {
        "echocat-skill-panel-3.0",                                   // 技能管理面板（生态 187★）
        "@deepseek-ai/dsh-browser-use",                              // 官方浏览器操作注册表
        "@deepseek-ai/dsh-experimental-browser-use-playwright-mcp",  // Playwright MCP provider
        "@deepseek-ai/dsh-computer-use",                             // 官方计算机操作注册表
        "@deepseek-ai/dsh-experimental-computer-use-cua-driver-mcp", // Cua Driver provider（默认禁用，见 DefaultsPatch）
        MemoryPackageSpec,                                           // 记忆 MCP 服务器（MCP 官方参考实现，版本见 MemoryPinnedVersion）
        "@linxin666/dsh-pet",                                        // 桌面宠物（生态 7857★/530 fork，Codex Pet 兼容，cordis bundle）
        // Wallpaper Engine 壁纸源：零依赖 57KB 的 cordis bundle，服务端路由把本机 WE
        // 创意工坊目录的清单/视频/图片吐出来，壳（个性化→背景皮肤→Wallpaper Engine 行）
        // 只消费这些路由当第三种皮肤素材源；插件自带的 Web 前端只注入 dsh Web 界面，
        // 壳用不到。启动成本仅「缺失时进同一条 pnpm add」+ 每次启动一次目录存在性检查。
        "@baiiii/dsh-wallpaper-local",
    };

    /// <summary>
    /// 与内核核心 lockstep 发版的 @deepseek-ai 家族包（browser-use / computer-use 及其实验
    /// provider）：实测整个 dsh-* 家族与 dsh 核心同版本（0.1.7-rc.2 全家族一致），而 npm 的
    /// latest tag 仍指向 0.1.6-alpha.1。profile 的 node_modules 会遮蔽 bundle 的同名包——
    /// 版本滞后时家族包按旧内核 API 编译/运行，会话创建直接死在
    /// 「mcp-client(playwright-mcp): initial connection or tool synchronization failed」
    /// （0.8.3.8 事故）。安装规格钉内核版本，见 <see cref="ResolveFamilySpec"/>。
    /// </summary>
    private static readonly string[] KernelFamilyPackages =
    {
        "@deepseek-ai/dsh-browser-use",
        "@deepseek-ai/dsh-experimental-browser-use-playwright-mcp",
        "@deepseek-ai/dsh-computer-use",
        "@deepseek-ai/dsh-experimental-computer-use-cua-driver-mcp",
    };

    /// <summary>
    /// 内核 0.2.1-rc.2 起的官方自动审查 bundle（内核标题「自动授权审查」），取代社区包
    /// dsh-approval-gate。它是内核 bundle 的依赖，经 installAnchor 就能解析，shell 不必
    /// pnpm add；但它列在内核的 OPTIONAL_BUNDLES 里、出厂关闭，只有显式写进 profile 的
    /// dsh.profile.bundles 才会挂上自己的 patch 层，进而 registerAuto 注册出「auto」
    /// 权限预设——那才是权限选择器里出现「自动审查」的唯一来源。
    /// </summary>
    private const string AutoReviewBundle = "@deepseek-ai/dsh-experimental-auto-review";

    /// <summary>
    /// 已被内核官方件取代、要从 profile 清单里撤下的社区包。dsh-approval-gate 自带
    /// dsh.bundle.patch，只要还在 bundles 里，它每次启动都会用自己的
    /// ensureAutoApprovePreset 把 auto-approve 预设写回 profile patch——光删壳侧的
    /// 写入逻辑挡不住它，必须从 dependencies 和 bundles 两个清单里一起摘掉
    /// （SelectInstalledBundles 每启动扫 dependencies，只摘 bundles 会被选回来）。
    /// node_modules 里的目录留着不删：删文件要动同步盘上的包树，而包不进 bundles 就不会加载。
    /// </summary>
    private static readonly string[] RetiredPluginPackages = { "dsh-approval-gate" };

    /// <summary>
    /// 期望版本（版本 enforcement 用）：家族包 = 内置内核版本，记忆包 = 锁定版本；
    /// 其余（生态包/bundle）返回 null，不参与强制对齐。
    /// </summary>
    private static string? PinnedVersion(string spec, string? kernelVersion) =>
        PackageDir(spec) switch
        {
            var d when KernelFamilyPackages.Contains(d) => kernelVersion,
            var d when d == MemoryPackageDir => MemoryPinnedVersion,
            _ => null,
        };

    /// <summary>
    /// 家族包的安装规格：钉内核版本（读不到内核版本时退回裸名，并留诊断）。
    /// 这是「幂等确保」里唯一会主动升级已安装包的路径——没有它，内核升级后 profile
    /// 里的旧家族包永远驻留，而目录存在性检查对此完全无感。
    /// </summary>
    private static string ResolveFamilySpec(string packageName, string? kernelVersion, StringBuilder diag)
    {
        if (kernelVersion is null)
        {
            diag.Append($"警告：读不到内核版本（Kernel\\dsh\\package.json），{packageName} 按默认 latest 安装\n");
            return packageName;
        }
        return $"{packageName}@{kernelVersion}";
    }

    /// <summary>读内置内核 dsh 的版本（&lt;kernelBinDir&gt;\..\dsh\package.json）：家族包钉版本的数据源。</summary>
    private static string? ReadKernelVersion(string kernelBinDir)
    {
        try
        {
            var manifest = Path.GetFullPath(Path.Combine(kernelBinDir, "..", "dsh", "package.json"));
            if (!File.Exists(manifest))
            {
                return null;
            }
            using var doc = JsonDocument.Parse(File.ReadAllText(manifest));
            return doc.RootElement.TryGetProperty("version", out var v) && v.ValueKind == JsonValueKind.String
                ? v.GetString()
                : null;
        }
        catch (Exception)
        {
            return null;
        }
    }

    /// <summary>本次运行的有效安装清单：家族包按内核版本钉版本，其余按固定规格。</summary>
    private static List<string> ResolveRequiredPackages(string? kernelVersion, StringBuilder diag)
    {
        var list = new List<string>(RequiredPackages.Length);
        foreach (var p in RequiredPackages)
        {
            var dir = PackageDir(p);
            list.Add(KernelFamilyPackages.Contains(dir)
                ? ResolveFamilySpec(dir, kernelVersion, diag)
                : p);
        }
        return list;
    }

    /// <summary>
    /// 引导器维护的 insert 条目（写入 profile patch 层）。每条绑定其依赖的包目录：
    /// 只有包已就绪的条目才会写入，保证 patch 引用永远可解析（loader 对解析失败的
    /// 条目会让整个内核启动失败，见 2026-09-08 桌面外部插件决策）。
    /// </summary>
    private static readonly (string MountId, string depPath)[] MountEntries =
    {
        ("memory-reference", @"@modelcontextprotocol/server-memory"),
        ("browser-use-registry", @"@deepseek-ai/dsh-browser-use"),
        ("browser-use-playwright", @"@deepseek-ai/dsh-experimental-browser-use-playwright-mcp"),
        ("computer-use-registry", @"@deepseek-ai/dsh-computer-use"),
        ("computer-use-cua-mcp", @"@deepseek-ai/dsh-experimental-computer-use-cua-driver-mcp"),
    };

    /// <summary>记忆条目的 id（设置页「记忆」开关定位 patch 块用）。</summary>
    internal const string MemoryMountId = "memory-reference";

    /// <summary>
    /// 幂等确保：依赖包齐全 + patch 条目就绪。已完成时毫秒级返回。
    /// 返回诊断摘要（空串 = 无事发生）。
    /// </summary>
    public static Task<string> EnsureAsync(string dshHome, string nodeExe, string kernelBinDir)
        => EnsureAsync(dshHome, nodeExe, kernelBinDir, null, CancellationToken.None);

    /// <summary>
    /// 同 <see cref="EnsureAsync(string, string, string)"/>，另向 <paramref name="progress"/>
    /// 上报插件安装进度（pnpm 安装期间每 250ms 一次）。壳把它画在主页加载卡上：
    /// 首启拉十几个包是最长的一步，没有它加载卡只能干转。
    /// </summary>
    public static async Task<string> EnsureAsync(
        string dshHome, string nodeExe, string kernelBinDir,
        IProgress<PluginInstallProgress>? progress, CancellationToken ct)
    {
        return await Task.Run(() => Ensure(dshHome, nodeExe, kernelBinDir, progress, ct)).ConfigureAwait(false);
    }

    private static string Ensure(
        string dshHome, string nodeExe, string kernelBinDir,
        IProgress<PluginInstallProgress>? progress, CancellationToken ct)
    {
        var profileDir = Path.Combine(dshHome, "profiles", ProfileName);
        var modulesDir = Path.Combine(profileDir, "node_modules");
        var diag = new StringBuilder();

        // 0) pnpm 安装参数：store 收进 profile 内、纯复制导入（不建 junction/symlink）。
        //    MSIX 包身份下 AppData 写虚拟化可能仍把 pnpm 的默认 store 重定向进包家，
        //    虚拟卷上的链接操作会报「拒绝访问」；copy + 本地 store 完全绕开链接机制。
        EnsurePnpmWorkspaceSettings(profileDir, diag);

        // 1) 包安装：node_modules 里缺谁就 add 谁（一条命令）。
        //    RequiredPackages 里的条目可带 @version 安装规格，存在性检查按目录名
        //    （去版本后缀），安装时按完整规格，保证锁定版本生效。
        //    家族包（@deepseek-ai/dsh-browser-use 等）的规格在这里钉成内核版本：
        //    它们与内核核心 lockstep 发版，装错版本的后果是会话创建整链失败。
        var kernelVersion = ReadKernelVersion(kernelBinDir);
        var required = ResolveRequiredPackages(kernelVersion, diag);
        var missing = required
            .Where(p => !Directory.Exists(Path.Combine(modulesDir, PackageDir(p))))
            .ToList();
        // 1.1) 版本 enforcement：已安装但版本不是期望版本的一律进重装队列
        //      （目录存在性检查发现不了版本；老版本会永远驻留下来遮蔽 bundle 的
        //      同名包——0.8.3.8 的 playwright-mcp 握手失败就是这么来的）。
        foreach (var p in required)
        {
            var dir = PackageDir(p);
            if (missing.Any(m => PackageDir(m) == dir))
            {
                continue;
            }
            var want = PinnedVersion(p, kernelVersion);
            if (want is null)
            {
                continue;
            }
            var installed = ReadInstalledVersion(modulesDir, dir);
            if (installed != want)
            {
                missing.Add(p);
                diag.Append($"版本不符重装: {dir} 期望 {want} 已装 {installed}\n");
            }
        }
        if (missing.Count > 0)
        {
            // 安装期间报进度：node_modules 里就绪的包数是 pnpm 真实推进的刻度
            // （包边下载边落盘），比 indeterministic 的"正在装"可信得多。
            using var install = WatchInstall(modulesDir, required, progress, ct);
            diag.Append(InstallMissing(nodeExe, kernelBinDir, profileDir, dshHome, missing.ToArray()));
        }
        else if (progress is not null)
        {
            ReportInstall(modulesDir, required, progress);
        }

        // 1.5) bundle 选入每次启动都跑（幂等）：dsh CLI 重建 profile 会把 bundles 重置
        //      回默认两条，只在缺包时选入会让插件装好却永远不挂载
        //      （2026-09-20 自动审批未启用事故）。
        SelectInstalledBundles(profileDir, dshHome, diag);
        // 1.6) 出厂关闭的可选 bundle：内核只给 bundles 清单里的条目挂 patch 层，
        //      自动审查必须在这里显式选入，否则权限选择器里永远没有「自动审查」。
        SelectOptionalBundle(profileDir, dshHome, AutoReviewBundle, diag);
        // 1.7) 撤下被内核官方件取代的社区包，并清掉它/老壳写进 patch 的 auto-approve
        //      死预设（0.2.1 内核只认 registerAuto 注册的 auto，配置表里的
        //      auto-approve 是个什么都不做的空档）。
        RetirePlugins(profileDir, dshHome, diag);
        RetireAutoApprovePreset(profileDir, dshHome, diag);

        // 2) patch 条目：只写包已就绪的条目，追加到 profile patch 层（不覆盖已有内容）。
        AppendMissingMountEntries(profileDir, modulesDir, diag);
        // 2.0) 会话全文检索覆盖：base patch 把 session-query-sqlite 配成 openAt=never
        //      （搜索关闭），shell 顶条检索会整体回落为标题匹配。这里按内核官方覆盖
        //      写法（id-targeted config 整体替换）改成 first-search + 持久化路径，
        //      patchReload=live 监视下不需要重启内核。
        EnsureSessionSearchOverride(profileDir, diag);
        // 2.1) 存量记忆条目迁移：老模板缺 MEMORY_FILE_PATH/cwd（实际落盘到包内
        //      dist/memory.jsonl，与 UI 显示的 ~/.dsh-mcp-reference-memory.jsonl 脱节），
        //      保留 disabled 状态就地升级到官方同款 env。
        MigrateMemoryBlockIfNeeded(profileDir, diag);
        return diag.ToString();
    }

    /// <summary>
    /// 安装进度观察器：每 250ms 数一次 node_modules 里已就绪的包，向 <see cref="IProgress{T}"/> 上报。
    /// 只在有包缺失时启用（不缺 = 无事可报）。
    /// </summary>
    private static IDisposable? WatchInstall(
        string modulesDir, List<string> required, IProgress<PluginInstallProgress>? progress, CancellationToken ct)
    {
        if (progress is null)
        {
            return null;
        }
        ReportInstall(modulesDir, required, progress);
        var timer = new System.Threading.Timer(_ =>
        {
            if (ct.IsCancellationRequested)
            {
                return;
            }
            ReportInstall(modulesDir, required, progress);
        }, state: null, dueTime: TimeSpan.FromMilliseconds(250), period: TimeSpan.FromMilliseconds(250));
        return new TimerHandle(timer);
    }

    private static void ReportInstall(
        string modulesDir, List<string> required, IProgress<PluginInstallProgress> progress)
    {
        var ready = 0;
        foreach (var spec in required)
        {
            if (Directory.Exists(Path.Combine(modulesDir, PackageDir(spec))))
            {
                ready++;
            }
        }
        try
        {
            progress.Report(new PluginInstallProgress(ready, required.Count));
        }
        catch (Exception)
        {
            // 上报失败（壳已关窗等）不影响安装本身
        }
    }

    private sealed class TimerHandle(System.Threading.Timer timer) : IDisposable
    {
        public void Dispose()
        {
            try { timer.Dispose(); } catch (Exception) { }
        }
    }

    /// <summary>安装规格转目录名：去掉尾部 @version（注意 scoped 包首字符的 @ 不是版本）。</summary>
    internal static string PackageDir(string spec)
    {
        if (string.IsNullOrEmpty(spec))
        {
            return spec;
        }
        var at = spec.LastIndexOf('@');
        // "@scope/name@1.2.3" → at > 0；"name@1.2.3" → at > 0；无版本 → at <= 0（scoped 包首字符）
        return at > 0 ? spec[..at] : spec;
    }

    /// <summary>读取已安装包的 version（node_modules/&lt;dir&gt;/package.json），失败返回空串。</summary>
    private static string ReadInstalledVersion(string modulesDir, string packageDir)
    {
        try
        {
            var manifest = Path.Combine(modulesDir, packageDir, "package.json");
            if (!File.Exists(manifest))
            {
                return "";
            }
            using var doc = JsonDocument.Parse(File.ReadAllText(manifest));
            if (doc.RootElement.TryGetProperty("version", out var v) &&
                v.ValueKind == JsonValueKind.String)
            {
                return v.GetString() ?? "";
            }
            return "";
        }
        catch (Exception)
        {
            return "";
        }
    }

    /// <summary>记忆 JSONL 文件路径：与 MountBlock 的 env 表达式同语义——
    /// 预设 MEMORY_FILE_PATH 则用它，否则 ~/.dsh-mcp-reference-memory.jsonl。</summary>
    internal static string MemoryFilePath()
    {
        var overridePath = Environment.GetEnvironmentVariable("MEMORY_FILE_PATH")?.Trim();
        if (!string.IsNullOrEmpty(overridePath))
        {
            return overridePath;
        }
        return Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
            ".dsh-mcp-reference-memory.jsonl");
    }

    /// <summary>读取记忆文件原文（不存在/不可读返回空串）。</summary>
    internal static string ReadMemoryText()
    {
        try
        {
            return File.ReadAllText(MemoryFilePath());
        }
        catch (Exception)
        {
            return "";
        }
    }

    /// <summary>
    /// 读取记忆文件原文，区分三种结果：文件不存在 = 空串（删除即清空）；
    /// 读失败（并发写窗口期的共享冲突等）= null。watch 回调不能把一次读失败
    /// 当成「内容被清空」把编辑器抹掉，所以需要这个可区分的形态。
    /// </summary>
    internal static string? ReadMemoryTextOrNull()
    {
        try
        {
            return File.ReadAllText(MemoryFilePath());
        }
        catch (FileNotFoundException)
        {
            return "";
        }
        catch (DirectoryNotFoundException)
        {
            return "";
        }
        catch (Exception)
        {
            return null;
        }
    }

    private static readonly JsonSerializerOptions MemoryLineOptions = new()
    {
        // 本地文件、人要读：中文 observation 保持原文，不转 \uXXXX
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    /// <summary>
    /// 把编辑器原文归一成合法 JSONL。两类行分开处理：
    ///   · 以 { 起头的行 = 手写 entity/relation，原样保留，但必须是带 type 的合法 JSON
    ///     （坏行号随第三个返回值带回，由调用方挡住保存并显示原因）；
    ///   · 其余非空行 = 用户写的自然语言。server-memory 只认 entity/relation，
    ///     非 JSON 行会让 loadGraph 整体抛错、全部记忆工具失效；直接挡下保存又等于
    ///     编辑器对普通用户不可用。折中：包成一条 entity 观察行落盘——话还是用户的原话，
    ///     模型能通过记忆工具读到，loadGraph 也不炸。
    /// 返回 (归一化文本, 转换行数, 坏 JSON 行号[同 MemoryBadLines 编号])。
    /// </summary>
    internal static (string Text, int Converted, List<int> Bad) MemoryNormalize(string text)
    {
        var newline = text.Contains("\r\n", StringComparison.Ordinal) ? "\r\n" : "\n";
        var lines = text.Split('\n');
        var bad = new List<int>();
        var converted = 0;
        var lineNo = 0;
        for (var i = 0; i < lines.Length; i++)
        {
            var line = lines[i].Trim();
            if (line.Length == 0)
            {
                continue;
            }
            lineNo++;
            if (line.StartsWith("{", StringComparison.Ordinal))
            {
                try
                {
                    using var doc = JsonDocument.Parse(line);
                    if (!doc.RootElement.TryGetProperty("type", out var t) ||
                        t.ValueKind != JsonValueKind.String)
                    {
                        bad.Add(lineNo);
                    }
                }
                catch (Exception)
                {
                    bad.Add(lineNo);
                }
                continue;
            }
            var label = line.Length <= 24 ? line : line[..24] + "…";
            lines[i] = JsonSerializer.Serialize(new
            {
                type = "entity",
                name = label,
                entityType = "note",
                observations = new[] { line },
            }, MemoryLineOptions);
            converted++;
        }
        var normalized = string.Join(newline, lines.Select(l => l.TrimEnd('\r')));
        return (normalized, converted, bad);
    }

    /// <summary>
    /// 原子保存记忆原文（临时文件 + Move 覆盖，与 server-memory 的 saveGraph 同策略，
    /// 避免崩溃时截断唯一副本）。返回错误信息，空串 = 成功。
    /// </summary>
    internal static string SaveMemoryText(string text)
    {
        try
        {
            var path = MemoryFilePath();
            var dir = Path.GetDirectoryName(path);
            if (!string.IsNullOrEmpty(dir))
            {
                Directory.CreateDirectory(dir);
            }
            var tmp = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
            try
            {
                File.WriteAllText(tmp, text);
                File.Move(tmp, path, overwrite: true);
            }
            catch (Exception)
            {
                try { File.Delete(tmp); } catch (Exception) { }
                throw;
            }
            return "";
        }
        catch (Exception ex)
        {
            return ex.Message;
        }
    }

    /// <summary>
    /// 确保 profile 的 pnpm-workspace.yaml 带上引导器需要的安装参数（幂等，不覆盖
    /// dsh 初始化生成的既有键）。失败不阻塞——add 仍可能以默认参数成功（非 MSIX 场景）。
    /// </summary>
    private static void EnsurePnpmWorkspaceSettings(string profileDir, StringBuilder diag)
    {
        try
        {
            Directory.CreateDirectory(profileDir);
            var path = Path.Combine(profileDir, "pnpm-workspace.yaml");
            var text = File.Exists(path) ? File.ReadAllText(path) : "";
            var sb = new StringBuilder(text);
            if (!text.Contains("storeDir", StringComparison.Ordinal))
            {
                if (sb.Length > 0 && !text.EndsWith("\n"))
                {
                    sb.AppendLine();
                }
                sb.AppendLine("storeDir: .pnpm-store");
            }
            if (!text.Contains("packageImportMethod", StringComparison.Ordinal))
            {
                if (sb.Length > 0 && !text.EndsWith("\n"))
                {
                    sb.AppendLine();
                }
                sb.AppendLine("packageImportMethod: copy");
            }
            if (sb.ToString() != text)
            {
                File.WriteAllText(path, sb.ToString());
                diag.Append("pnpm-workspace.yaml 已补 storeDir/packageImportMethod\n");
            }
        }
        catch (Exception ex)
        {
            diag.Append($"pnpm-workspace.yaml 写入失败: {ex.Message}\n");
        }
    }

    /// <summary>
    /// 包安装执行链：壳直接 CreateProcess（包内 node.exe → 包内 pnpm.cjs），
    /// 不经 `dsh plugin`/cmd shim —— WindowsApps 的 ACL 会拒绝外部 cmd.exe 解释执行
    /// 包内 .cmd（实测「拒绝访问」），而 node.exe 直跑与内核启动同链路，已验证可用。
    /// 安装参数经 --config 收进 profile：store 在 profile 内、纯复制导入（不建链接）。
    /// </summary>
    private static string InstallMissing(
        string nodeExe, string kernelBinDir, string profileDir, string dshHome, string[] missing)
    {
        var pnpmCjs = Path.Combine(kernelBinDir, "pnpm", "bin", "pnpm.cjs");
        var quoted = string.Join(" ", missing.Select(p => $"\"{p}\""));
        var psi = new ProcessStartInfo
        {
            FileName = nodeExe,
            Arguments =
                $"\"{pnpmCjs}\" add --dir \"{profileDir}\" " +
                $"--config.store-dir=\"{Path.Combine(profileDir, ".pnpm-store")}\" " +
                $"--config.package-import-method=copy {quoted}",
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
        };

        using var cts = new CancellationTokenSource(InstallTimeout);
        using var process = new Process { StartInfo = psi };
        var output = new StringBuilder();
        try
        {
            process.Start();
            var pumpOut = pump(process.StandardOutput, output);
            var pumpErr = pump(process.StandardError, output);
            var exited = process.WaitForExitAsync(cts.Token);
            exited.Wait(InstallTimeout);
            if (!process.HasExited)
            {
                try { process.Kill(entireProcessTree: true); } catch (Exception) { }
                return AppendDiag(dshHome, $"安装超时（{InstallTimeout.TotalSeconds:s}s）: {string.Join(' ', missing)}\n{output}");
            }
            Task.WaitAll(new[] { pumpOut, pumpErr }, TimeSpan.FromSeconds(5));
            var ok = process.ExitCode == 0;
            var summary = $"add {(ok ? "完成" : "失败")}（exit {process.ExitCode}）: {string.Join(' ', missing)}\n{Tail(output, 4000)}";
            AppendDiag(dshHome, summary);
            return summary + "\n";
        }
        catch (Exception ex)
        {
            var summary = $"add 异常: {ex.Message}\n{Tail(output, 2000)}";
            AppendDiag(dshHome, summary);
            return summary + "\n";
        }

        static Task pump(StreamReader reader, StringBuilder sink) =>
            Task.Run(() =>
            {
                // 转发 + 截断：pnpm 的进度条输出量大，只保留尾部诊断。
                while (reader.ReadLine() is { } line)
                {
                    lock (sink) sink.AppendLine(line);
                }
            });
    }

    /// <summary>
    /// JsonNode.ToJsonString 用的序列化项。必须显式给 TypeInfoResolver：宿主环境
    /// （裁剪/AOT 上下文）下默认 resolver 不可用，抛「JsonSerializerOptions instance
    /// must specify a TypeInfoResolver setting」——2026-09-20 自动审批未启用事故的根因，
    /// 当时该异常被静默吞掉，bundle 从未写入。
    /// </summary>
    private static readonly JsonSerializerOptions IndentedJson = new()
    {
        WriteIndented = true,
        TypeInfoResolver = new System.Text.Json.Serialization.Metadata.DefaultJsonTypeInfoResolver(),
    };

    /// <summary>
    /// bundle 选入：dsh CLI 的 `plugin add` 在转发 pnpm 之外还会把「声明了 dsh.bundle 的
    /// 新装包」追加进 package.json 的有序 bundles 列表；直跑 pnpm 没有这一步，这里补上。
    /// 已选入的包不重复；写回保留其他字段（patchReload 等）。结果落 plugin-bootstrap.log，
    /// 否则选入静默失败时（如 package.json 解析异常）设置页只会显示「未挂载」无从诊断。
    /// </summary>
    private static void SelectInstalledBundles(string profileDir, string dshHome, StringBuilder diag)
    {
        try
        {
            var pkgPath = Path.Combine(profileDir, "package.json");
            if (!File.Exists(pkgPath))
            {
                return;
            }
            var node = System.Text.Json.Nodes.JsonNode.Parse(File.ReadAllText(pkgPath));
            var dsh = node?["dsh"];
            var profile = dsh?["profile"];
            var bundles = profile?["bundles"] as System.Text.Json.Nodes.JsonArray;
            if (bundles is null)
            {
                diag.Append("bundle 选入跳过：package.json 无 dsh.profile.bundles 列表\n");
                return;
            }
            var modulesDir = Path.Combine(profileDir, "node_modules");
            var selected = bundles.Select(b => b?.GetValue<string>() ?? "").ToHashSet(StringComparer.Ordinal);
            var added = new List<string>();

            var deps = node?["dependencies"];
            if (deps is not System.Text.Json.Nodes.JsonObject depObj)
            {
                diag.Append("bundle 选入跳过：package.json 无 dependencies 对象\n");
                return;
            }
            foreach (var name in depObj.Select(kv => kv.Key))
            {
                if (selected.Contains(name))
                {
                    continue;
                }
                var manifest = Path.Combine(modulesDir, name.Replace('/', Path.DirectorySeparatorChar), "package.json");
                if (!File.Exists(manifest))
                {
                    continue;
                }
                try
                {
                    using var doc = JsonDocument.Parse(File.ReadAllText(manifest));
                    if (doc.RootElement.TryGetProperty("dsh", out var d) &&
                        d.TryGetProperty("bundle", out var b) &&
                        b.ValueKind == JsonValueKind.Object)
                    {
                        bundles.Add(name);
                        added.Add(name);
                    }
                }
                catch (Exception)
                {
                    // 单个包清单异常不阻塞其余扫描
                }
            }
            if (added.Count > 0)
            {
                File.WriteAllText(pkgPath, node!.ToJsonString(IndentedJson));
                diag.Append($"bundle 选入: {string.Join(' ', added)}\n");
                AppendDiag(dshHome, $"bundle 选入: {string.Join(' ', added)}");
            }
        }
        catch (Exception ex)
        {
            diag.Append($"bundle 选入失败: {ex.Message}\n");
            AppendDiag(dshHome, $"bundle 选入失败: {ex.Message}");
        }
    }

    /// <summary>
    /// 把出厂关闭的可选 bundle 显式选入 profile 的 dsh.profile.bundles。内核的
    /// loadProfileDirectory 只给清单里的 bundle 挂 patch 层，没选入的连默认配置都不参与；
    /// 这类包通常是内核 bundle 的依赖（installAnchor 可解析），shell 不必 pnpm add，
    /// 缺的只是「选入」这一步。
    /// </summary>
    private static void SelectOptionalBundle(string profileDir, string dshHome, string bundleName, StringBuilder diag)
    {
        try
        {
            var pkgPath = Path.Combine(profileDir, "package.json");
            if (!File.Exists(pkgPath))
            {
                return;
            }
            var node = System.Text.Json.Nodes.JsonNode.Parse(File.ReadAllText(pkgPath));
            var bundles = node?["dsh"]?["profile"]?["bundles"] as System.Text.Json.Nodes.JsonArray;
            if (bundles is null)
            {
                diag.Append($"bundle 选入跳过：package.json 无 dsh.profile.bundles 列表\n");
                return;
            }
            if (bundles.Any(b => b?.GetValue<string>() == bundleName))
            {
                return;
            }
            bundles.Add(bundleName);
            File.WriteAllText(pkgPath, node!.ToJsonString(IndentedJson));
            diag.Append($"bundle 选入: {bundleName}\n");
            AppendDiag(dshHome, $"bundle 选入: {bundleName}");
        }
        catch (Exception ex)
        {
            diag.Append($"bundle 选入失败({bundleName}): {ex.Message}\n");
            AppendDiag(dshHome, $"bundle 选入失败({bundleName}): {ex.Message}");
        }
    }

    /// <summary>
    /// 把 bundle 从 profile 的 dsh.profile.bundles 摘掉（<see cref="SelectOptionalBundle"/> 的逆操作）。
    /// 摘掉只是不再挂它的 patch 层：包本身还在（内核 bundle 的依赖，installAnchor 照旧能解析），
    /// 下次想开直接选入即可，不必重装。
    /// </summary>
    private static void DeselectBundle(string profileDir, string dshHome, string bundleName, StringBuilder diag)
    {
        try
        {
            var pkgPath = Path.Combine(profileDir, "package.json");
            if (!File.Exists(pkgPath))
            {
                return;
            }
            var node = System.Text.Json.Nodes.JsonNode.Parse(File.ReadAllText(pkgPath));
            if (!RemoveBundleFromNode(node, bundleName))
            {
                return;
            }
            File.WriteAllText(pkgPath, node!.ToJsonString(IndentedJson));
            diag.Append($"bundle 摘出: {bundleName}\n");
            AppendDiag(dshHome, $"bundle 摘出: {bundleName}");
        }
        catch (Exception ex)
        {
            diag.Append($"bundle 摘出失败({bundleName}): {ex.Message}\n");
            AppendDiag(dshHome, $"bundle 摘出失败({bundleName}): {ex.Message}");
        }
    }

    /// <summary>从已解析的 profile manifest 里摘掉 bundle 条目；确有摘除才返回 true。
    /// 只改内存节点、由调用方落盘——多个调用方共写一份文件时，后落盘的会覆盖先落盘的。</summary>
    private static bool RemoveBundleFromNode(System.Text.Json.Nodes.JsonNode? node, string bundleName)
    {
        var bundles = node?["dsh"]?["profile"]?["bundles"] as System.Text.Json.Nodes.JsonArray;
        if (bundles is null)
        {
            return false;
        }
        var removed = false;
        for (var i = bundles.Count - 1; i >= 0; i--)
        {
            if (bundles[i]?.GetValue<string>() == bundleName)
            {
                bundles.RemoveAt(i);
                removed = true;
            }
        }
        return removed;
    }

    /// <summary>
    /// 撤下被内核官方件取代的社区包：从 dependencies 与 dsh.profile.bundles 两个清单摘掉。
    /// 两个清单都得摘——SelectInstalledBundles 每启动扫 dependencies 重新选入，
    /// 只摘 bundles 下次启动又会回来。
    /// </summary>
    private static void RetirePlugins(string profileDir, string dshHome, StringBuilder diag)
    {
        if (RetiredPluginPackages.Length == 0)
        {
            return;
        }
        try
        {
            var pkgPath = Path.Combine(profileDir, "package.json");
            if (!File.Exists(pkgPath))
            {
                return;
            }
            var node = System.Text.Json.Nodes.JsonNode.Parse(File.ReadAllText(pkgPath));
            var changed = false;
            foreach (var name in RetiredPluginPackages)
            {
                var deps = node?["dependencies"] as System.Text.Json.Nodes.JsonObject;
                if (deps is not null && deps.Remove(name))
                {
                    changed = true;
                }
                // 直接在同一个 node 上改：各自读文件再落盘会互相覆盖（见 RemoveBundleFromNode）。
                changed |= RemoveBundleFromNode(node, name);
            }
            if (!changed)
            {
                return;
            }
            File.WriteAllText(pkgPath, node!.ToJsonString(IndentedJson));
            diag.Append($"已撤下取代插件: {string.Join(' ', RetiredPluginPackages)}\n");
            AppendDiag(dshHome, $"已撤下取代插件: {string.Join(' ', RetiredPluginPackages)}");
        }
        catch (Exception ex)
        {
            diag.Append($"撤下取代插件失败: {ex.Message}\n");
            AppendDiag(dshHome, $"撤下取代插件失败: {ex.Message}");
        }
    }

    /// <summary>
    /// 一次性迁移：清掉写进 profile patch 的 auto-approve 残留。0.2.1 内核的自动审查
    /// 只认 registerAuto 动态注册的 "auto" 预设，配置表里的 "auto-approve" 是死档——
    /// 留着会在权限选择器里多出一项什么都不做的「自动审批」。幂等：没有该块即无事。
    /// 另清 permission 覆盖块的 defaultPreset: auto-approve 毒值（见
    /// <see cref="RemoveDefaultPresetTrap"/>：那是服务级启动失败，不是死档）。
    /// </summary>
    private static void RetireAutoApprovePreset(string profileDir, string dshHome, StringBuilder diag)
    {
        try
        {
            var path = Path.Combine(profileDir, "cordis.patch.yml");
            if (!File.Exists(path))
            {
                return;
            }
            var text = File.ReadAllText(path);
            if (!text.Contains("auto-approve", StringComparison.Ordinal))
            {
                return;
            }
            var next = RemovePreset(text) ?? text;
            next = RemoveDefaultPresetTrap(next) ?? next;
            if (next == text)
            {
                return;
            }
            File.WriteAllText(path, next);
            diag.Append("patch 已清除 auto-approve 死预设\n");
            AppendDiag(dshHome, "patch 已清除 auto-approve 死预设");
        }
        catch (Exception ex)
        {
            diag.Append($"清除 auto-approve 死预设失败: {ex.Message}\n");
            AppendDiag(dshHome, $"清除 auto-approve 死预设失败: {ex.Message}");
        }
    }

    /// <summary>把 PATH 最前置注入（让 `pnpm` 解析到 Kernel\bin\pnpm.cmd）。</summary>
    internal static void PrependPath(ProcessStartInfo psi, string kernelBinDir)
    {
        if (string.IsNullOrEmpty(kernelBinDir) || !Directory.Exists(kernelBinDir))
        {
            return;
        }
        var current = psi.EnvironmentVariables["PATH"];
        psi.EnvironmentVariables["PATH"] = string.IsNullOrEmpty(current)
            ? kernelBinDir
            : kernelBinDir + Path.PathSeparator + current;
    }

    // ---------------- profile patch 层（cordis.patch.yml） ----------------

    /// <summary>内核全文检索的覆盖条目：把 base patch 的 openAt=never 改成 first-search。
    /// session-query-sqlite 由 base patch 挂载（dsh-base cordis.patch.yml），其配置
    /// 经 cordis-plugin-include 的 id-targeted patch 整体替换；dshHomePath 是
    /// dsh-app-boot 暴露给配置表达式的路径助手，path 落到 DSH_HOME 下独立于
    /// session-persistence 的派生索引库。</summary>
    private static void EnsureSessionSearchOverride(string profileDir, StringBuilder diag)
    {
        const string targetId = "session-query-sqlite";
        var path = Path.Combine(profileDir, "cordis.patch.yml");
        var text = File.Exists(path) ? File.ReadAllText(path) : "";
        if (text.Contains($"id: {targetId}", StringComparison.Ordinal))
        {
            return; // 已有（含用户手工改的）：不动
        }
        var block =
            "# ---- Blade2 会话全文检索（覆盖 base patch 的 openAt=never） ----\n" +
            $"- id: {targetId}\n" +
            "  name: '@deepseek-ai/dsh-session-query-sqlite'\n" +
            "  config:\n" +
            "    path: !!js dshHomePath('session-search.db')\n" +
            "    openAt: first-search\n";
        var header = text.Length == 0 || text.Trim('\ufeff', '\r', '\n', ' ', '\t').Length == 0
            ? ""
            : (text.EndsWith("\n") ? "" : "\n");
        try
        {
            File.WriteAllText(path, text + header + block);
            diag.Append("patch 条目已写入 session-query-sqlite 覆盖（openAt=first-search）\n");
        }
        catch (Exception ex)
        {
            diag.Append($"patch 覆盖 session-query-sqlite 失败: {ex.Message}\n");
        }
    }

    /// <summary>profile patch 文件路径（内核用户 patch 层，HMR live 监视）。</summary>
    internal static string ProfilePatchPath(string dshHome) =>
        Path.Combine(dshHome, "profiles", ProfileName, "cordis.patch.yml");

    /// <summary>insert 条目模板。{0} = disabled 行（"" 或 "      disabled: true\n"）。</summary>
    private static string MountBlock(string mountId, bool disabled) => mountId switch
    {
        "memory-reference" =>
            "- insert:\n" +
            "    - id: memory-reference\n" +
            (disabled ? "      disabled: true\n" : "") +
            "      name: '@deepseek-ai/dsh-mcp-client'\n" +
            "      config:\n" +
            "        serverName: reference_memory\n" +
            "        transport: stdio\n" +
            // command/args 与官方示例不同：官方用 PATH 上的 mcp-server-memory 全局 shim，
            // 壳用 profile 内 node 直跑 dist/index.js（免全局安装，MSIX 沙箱可用）。
            // env/cwd 与官方示例同款：MEMORY_FILE_PATH 缺省即 ~/.dsh-mcp-reference-memory.jsonl，
            // 与设置页显示路径一致；允许用户预设 MEMORY_FILE_PATH 覆盖。
            "        command: !!js process.execPath\n" +
            "        args:\n" +
            "          - !!js >-\n" +
            "            process.getBuiltinModule('node:path').join(process.env.DSH_HOME || '',\n" +
            "            'profiles', 'web', 'node_modules', '@modelcontextprotocol',\n" +
            "            'server-memory', 'dist', 'index.js')\n" +
            "        cwd: !!js process.cwd()\n" +
            "        env:\n" +
            "          MEMORY_FILE_PATH: !!js >-\n" +
            "            process.env.MEMORY_FILE_PATH?.trim() || process.getBuiltinModule('node:path').join(process.getBuiltinModule('node:os').homedir(), '.dsh-mcp-reference-memory.jsonl')\n",
        "browser-use-registry" =>
            "- insert:\n    - id: browser-use-registry\n" +
            (disabled ? "      disabled: true\n" : "") +
            "      name: '@deepseek-ai/dsh-browser-use'\n",
        "browser-use-playwright" =>
            "- insert:\n    - id: browser-use-playwright\n" +
            (disabled ? "      disabled: true\n" : "") +
            "      name: '@deepseek-ai/dsh-experimental-browser-use-playwright-mcp'\n" +
            "      config:\n        mode: launch\n        headless: true\n",
        "computer-use-registry" =>
            "- insert:\n    - id: computer-use-registry\n" +
            (disabled ? "      disabled: true\n" : "") +
            "      name: '@deepseek-ai/dsh-computer-use'\n",
        "computer-use-cua-mcp" =>
            "- insert:\n    - id: computer-use-cua-mcp\n" +
            (disabled ? "      disabled: true\n" : "") +
            "      name: '@deepseek-ai/dsh-experimental-computer-use-cua-driver-mcp'\n" +
            "      config:\n        command: cua-driver\n        args: [mcp]\n",
        _ => "",
    };

    private static void AppendMissingMountEntries(string profileDir, string modulesDir, StringBuilder diag)
    {
        try
        {
            Directory.CreateDirectory(profileDir);
            var path = Path.Combine(profileDir, "cordis.patch.yml");
            var text = File.Exists(path) ? File.ReadAllText(path) : "";
            var sb = new StringBuilder();
            foreach (var (mountId, depPath) in MountEntries)
            {
                // 已有条目（含用户手工迁移的）不重复写；包未就绪的条目不写（保证可解析）。
                if (text.Contains($"id: {mountId}", StringComparison.Ordinal))
                {
                    continue;
                }
                if (!Directory.Exists(Path.Combine(modulesDir, depPath)))
                {
                    diag.Append($"patch 跳过 {mountId}（包未就绪: {depPath}）\n");
                    continue;
                }
                var disabled = mountId == "computer-use-cua-mcp"; // cua-driver 需用户自装，默认禁用
                sb.Append(MountBlock(mountId, disabled));
            }
            if (sb.Length == 0)
            {
                return;
            }
            var header = "# ---- Blade2 默认插件（壳引导器维护；可手工调整，勿删 id 标记） ----\n";
            // dsh CLI 重建 profile 时会把 patch 重置为「注释模板 + 独立 [] 行」；
            // 必须在追加前去掉该 [] 根节点，否则文件出现两个根序列，非法 YAML
            // 会让内核 loader 解析失败、启动即退（2026-09-20 空白窗口事故）。
            var bodyLines = text.Replace("\r\n", "\n").Split('\n')
                .Where(l => l.Trim() != "[]").ToArray();
            var body = string.Join("\n", bodyLines);
            var trimmed = body.Trim('\ufeff', '\r', '\n', ' ', '\t');
            if (trimmed.Length == 0 || trimmed.All(c => c == '#'))
            {
                // 只剩注释（或全空）：重写为带引导头部的单根序列。
                var commentPart = bodyLines.Where(l => l.TrimStart().StartsWith('#')).ToArray();
                File.WriteAllText(path,
                    (commentPart.Length > 0 ? string.Join("\n", commentPart) + "\n" : "") + header + sb + "\n");
            }
            else
            {
                var nl = body.EndsWith("\n") ? "" : "\n";
                File.WriteAllText(path, body + nl + "# ---- Blade2 默认插件（追加） ----\n" + sb + "\n");
            }
            diag.Append("patch 条目已写入 cordis.patch.yml\n");
        }
        catch (Exception ex)
        {
            diag.Append($"patch 写入失败: {ex.Message}\n");
        }
    }

    // ---------------- 挂载条目开关（设置页「记忆 / 电脑控制 / 浏览器控制」分区用） ----------------

    /// <summary>
    /// 记忆条目的当前启用状态。条目不存在视为启用（引导器写入的默认态）。
    /// </summary>
    internal static bool IsMemoryEnabled(string dshHome) => IsMountEnabled(dshHome, MemoryMountId);

    /// <summary>
    /// 任意挂载条目的当前启用状态。条目不存在视为启用（引导器写入的默认态）。
    /// </summary>
    internal static bool IsMountEnabled(string dshHome, string mountId)
    {
        var text = ReadPatchSafe(dshHome);
        var block = RewriteBlock(text, mountId, setDisabled: null);
        return block is null || !block.Value.Block.Contains("disabled: true", StringComparison.Ordinal);
    }

    /// <summary>
    /// 切换记忆条目的 disabled 标志并落盘；内核 patchReload=live 监视该文件，热生效。
    /// 条目不存在时（引导器未跑过/被删）追加完整条目块。
    /// </summary>
    internal static bool SetMemoryEnabled(string dshHome, bool enabled)
        => SetMountEnabled(dshHome, MemoryMountId, enabled);

    /// <summary>
    /// 切换任意挂载条目的 disabled 标志并落盘（语义同 <see cref="SetMemoryEnabled"/>）。
    /// 条目不存在时追加完整条目块；条目依赖的包未安装时拒绝写入
    /// （loader 对解析失败的条目会让整个内核启动失败）。
    /// </summary>
    internal static bool SetMountEnabled(string dshHome, string mountId, bool enabled)
    {
        try
        {
            var path = ProfilePatchPath(dshHome);
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);
            var text = File.Exists(path) ? File.ReadAllText(path) : "";
            var found = RewriteBlock(text, mountId, setDisabled: !enabled);
            if (found is null)
            {
                if (!MountPackageExists(dshHome, mountId))
                {
                    return false; // 包不在，写条目会让内核启动失败
                }
                var trimmed = text.Trim('\ufeff', '\r', '\n', ' ', '\t');
                var head = trimmed.Length == 0
                    ? "# ---- Blade2 默认插件（壳引导器维护） ----\n"
                    : text.EndsWith("\n") ? text : text + "\n";
                File.WriteAllText(path, head + MountBlock(mountId, !enabled) + "\n");
                return true;
            }

            if (found.Value.NewText != text)
            {
                File.WriteAllText(path, found.Value.NewText);
            }
            return true;
        }
        catch (Exception)
        {
            return false;
        }
    }

    /// <summary>挂载条目依赖的包是否已安装（条目不在清单里 = 未知依赖，按未安装拒写）。</summary>
    private static bool MountPackageExists(string dshHome, string mountId)
    {
        var dep = Array.Find(MountEntries, e => e.MountId == mountId).depPath;
        return dep is not null &&
               Directory.Exists(Path.Combine(dshHome, "profiles", ProfileName, "node_modules", dep));
    }

    /// <summary>
    /// 存量记忆条目迁移：老模板缺 MEMORY_FILE_PATH/cwd 时，用新模板原地替换
    /// 其所属的 "- insert:" 文档，保留 disabled 状态。已是新模板则无操作。
    /// </summary>
    private static void MigrateMemoryBlockIfNeeded(string profileDir, StringBuilder diag)
    {
        try
        {
            var path = Path.Combine(profileDir, "cordis.patch.yml");
            if (!File.Exists(path))
            {
                return;
            }
            var text = File.ReadAllText(path);
            var block = RewriteBlock(text, MemoryMountId, setDisabled: null);
            if (block is null || block.Value.Block.Contains("MEMORY_FILE_PATH", StringComparison.Ordinal))
            {
                return; // 无条目（稍后由 Append 写入）或已是新模板
            }
            var disabled = block.Value.Block.Contains("disabled: true", StringComparison.Ordinal);
            var lines = text.Replace("\r\n", "\n").Split('\n');
            var marker = $"- id: {MemoryMountId}";
            var idIdx = Array.FindIndex(lines, l => l.Trim() == marker);
            if (idIdx < 0)
            {
                return;
            }
            // 所属文档起点：id 行往上最近的顶格 "- " 行（含 "- insert:"）。
            var docStart = idIdx;
            for (var i = idIdx; i >= 0; i--)
            {
                if (lines[i].StartsWith("- "))
                {
                    docStart = i;
                    break;
                }
            }
            // 文档终点：id 行之后下一个顶格 "- " 行或文件尾。
            var docEnd = lines.Length;
            for (var i = idIdx + 1; i < lines.Length; i++)
            {
                if (lines[i].StartsWith("- "))
                {
                    docEnd = i;
                    break;
                }
            }
            var fresh = MountBlock(MemoryMountId, disabled).TrimEnd('\r', '\n');
            var prefix = string.Join('\n', lines[..docStart]);
            var suffix = string.Join('\n', lines[docEnd..]);
            var next = prefix + (docStart > 0 ? "\n" : "") + fresh + "\n" + suffix;
            if (next != text.Replace("\r\n", "\n"))
            {
                File.WriteAllText(path, next);
                diag.Append($"patch 记忆条目已升级到 {MemoryPinnedVersion} 模板（保留 {(disabled ? "停用" : "启用")}）\n");
            }
        }
        catch (Exception ex)
        {
            diag.Append($"patch 记忆条目升级失败: {ex.Message}\n");
        }
    }

    /// <summary>读取 patch 文件（不存在/不可读返回空串）。</summary>
    private static string ReadPatchSafe(string dshHome)
    {
        try
        {
            var path = ProfilePatchPath(dshHome);
            return File.Exists(path) ? File.ReadAllText(path) : "";
        }
        catch (Exception)
        {
            return "";
        }
    }

    /// <summary>
    /// 在顶层数组流里定位 id 块（从 "- id: {id}" 行到下一个顶格 "- " 行或文件尾）。
    /// setDisabled=null 仅定位；true/false 重组块内 disabled 行并返回整文件新文本。
    /// </summary>
    private static (string Block, string NewText)? RewriteBlock(
        string text, string mountId, bool? setDisabled)
    {
        var lines = text.Split('\n');
        var marker = $"- id: {mountId}";
        int start = -1;
        for (var i = 0; i < lines.Length; i++)
        {
            if (lines[i].TrimEnd('\r').Trim() == marker)
            {
                start = i;
                break;
            }
        }
        if (start < 0)
        {
            return null;
        }
        int end = lines.Length;
        for (var i = start + 1; i < lines.Length; i++)
        {
            if (lines[i].TrimEnd('\r').StartsWith("- "))
            {
                end = i;
                break;
            }
        }

        var oldBlock = string.Join('\n', lines[start..end]);
        if (setDisabled is null)
        {
            return (oldBlock, text);
        }

        var body = lines[start..end]
            .Where(l => !l.Trim().StartsWith("disabled:"))
            .ToList();
        if (setDisabled.Value)
        {
            var idIdx = body.FindIndex(l => l.TrimEnd('\r').Trim() == marker);
            body.Insert(idIdx + 1, "      disabled: true");
        }
        var newBlock = string.Join('\n', body);
        // 前缀/块/后缀三段都是「无首尾换行」的 join 结果：三段直接相加会把前一段末行与
        // 后一段首行粘成一行（"- insert:" + "    - id: xxx" → "- insert:    - id: xxx"），
        // 而 loader 对解析失败的 overlay 是整体启动失败。分段必须显式补换行
        // （LF 文件没有 \r 兜底，粘行必然发生；2026-09-20 内核启动失败事故的根因）。
        var prefix = start > 0 ? string.Join('\n', lines[..start]) + '\n' : "";
        var suffix = end < lines.Length ? '\n' + string.Join('\n', lines[end..]) : "";
        var newText = prefix + newBlock + suffix;
        return (oldBlock, newText);
    }

    // ---------------- 自动审查开关（设置页「插件」分区的自动审查块用） ----------------

    /// <summary>自动审查是否已选入（profile bundles 清单里有 auto-review 条目）。</summary>
    internal static bool IsAutoReviewEnabled(string dshHome)
        => ReadBundles(dshHome).Contains(AutoReviewBundle, StringComparer.Ordinal);

    /// <summary>
    /// 切换自动审查 bundle 的选入状态：开=写进 dsh.profile.bundles，关=从清单摘掉。
    /// 内核 patchReload=live 监视该清单，改动热生效；摘掉后 auto-review 不再 apply，
    /// registerAuto 不再被调用，「auto」预设从权限选择器消失，正在用它的会话按内核
    /// 自己的 dispose 逻辑落到 danger-full-access（shell 不代为决定，交给内核）。
    /// </summary>
    internal static bool SetAutoReviewEnabled(string dshHome, bool enabled)
    {
        try
        {
            var profileDir = Path.Combine(dshHome, "profiles", ProfileName);
            if (IsAutoReviewEnabled(dshHome) == enabled)
            {
                return true;
            }
            var diag = new StringBuilder();
            if (enabled)
            {
                SelectOptionalBundle(profileDir, dshHome, AutoReviewBundle, diag);
            }
            else
            {
                DeselectBundle(profileDir, dshHome, AutoReviewBundle, diag);
                // 关掉后预设表里若还留着迁移没清干净的 auto-approve 死档，一并清掉
                RetireAutoApprovePreset(profileDir, dshHome, diag);
            }
            return IsAutoReviewEnabled(dshHome) == enabled;
        }
        catch (Exception)
        {
            return false;
        }
    }

    /// <summary>从 patch 里移除 auto-approve 预设块（迁移用：0.2.1 起该档是死预设，
    /// 见 <see cref="RetireAutoApprovePreset"/>；块从 id 行到下一个同级 presets 子项/条目边界）。</summary>
    private static string? RemovePreset(string text)
    {
        var lines = text.Replace("\r\n", "\n").Split('\n');
        var start = Array.FindIndex(lines, l => l.TrimEnd('\r') == "      auto-approve:");
        if (start < 0)
        {
            return null;
        }
        var end = lines.Length;
        for (var i = start + 1; i < lines.Length; i++)
        {
            var t = lines[i].TrimEnd('\r');
            // 同级 presets 子项（缩进 6 且以冒号结尾）或离开 presets 区
            if (t.StartsWith("- ", StringComparison.Ordinal)) { end = i; break; }
            if (t.Length > 0 && !t.StartsWith("        ", StringComparison.Ordinal) &&
                !t.StartsWith("      ", StringComparison.Ordinal)) { end = i; break; }
            if (t.StartsWith("      ", StringComparison.Ordinal) && !t.StartsWith("        ", StringComparison.Ordinal) &&
                t.TrimEnd().EndsWith(':')) { end = i; break; }
        }
        var outLines = new List<string>(lines.Length);
        outLines.AddRange(lines[..start]);
        outLines.AddRange(lines[end..]);
        return string.Join("\n", outLines);
    }

    /// <summary>
    /// 删掉 permission 覆盖块里的 defaultPreset: auto-approve 毒值。这不是「死档」而是
    /// 服务级毒化：0.2.1 内核构造 PermissionPresetService 时 resolve(defaultPreset) 遇
    /// 未知预设直接抛错，整个 permissionPresets 服务起不来，registerAuto 从未被调用，
    /// 「auto」预设从未注册，auto-review bundle 永远停在 pending——权限选择器整块消失。
    /// 旧社区包 dsh-approval-gate 的 ensureAutoApprovePreset 会把这行写进用户 patch，
    /// 且每次启动被设置服务当作 permission.defaultPreset 的当前值自延续，只能靠删行根治。
    /// presets 三键与 base 完全重复、无害，保留（用户可能改过其中的值）。
    /// </summary>
    private static string? RemoveDefaultPresetTrap(string text)
    {
        var lines = text.Replace("\r\n", "\n").Split('\n');
        // 只在 - id: permission 块内动手，避免误伤其它条目的同名配置键
        int start = -1;
        for (var i = 0; i < lines.Length; i++)
        {
            if (lines[i].TrimEnd('\r').Trim() == "- id: permission")
            {
                start = i;
                break;
            }
        }
        if (start < 0)
        {
            return null;
        }
        int end = lines.Length;
        for (var i = start + 1; i < lines.Length; i++)
        {
            if (lines[i].TrimEnd('\r').StartsWith("- "))
            {
                end = i;
                break;
            }
        }
        var trap = new System.Text.RegularExpressions.Regex(
            @"^\s*defaultPreset:\s*""?auto-approve""?\s*$", System.Text.RegularExpressions.RegexOptions.Compiled);
        var outLines = new List<string>(lines.Length);
        var removed = false;
        for (var i = 0; i < lines.Length; i++)
        {
            var line = lines[i].TrimEnd('\r');
            if (i > start && i < end && trap.IsMatch(line))
            {
                removed = true;
                continue;
            }
            outLines.Add(line);
        }
        return removed ? string.Join("\n", outLines) : null;
    }

    // ---------------- 状态摘要（设置页展示） ----------------

    /// <summary>单个默认插件的呈现状态。</summary>
    internal sealed record PluginStatus(string Id, bool Installed, bool Mounted, bool Disabled);

    /// <summary>读取当前默认插件状态（安装/挂载/禁用），设置页展示用。</summary>
    internal static System.Collections.Generic.IReadOnlyList<PluginStatus> GetStatus(string dshHome)
    {
        var modulesDir = Path.Combine(dshHome, "profiles", ProfileName, "node_modules");
        var bundles = ReadBundles(dshHome);
        var patch = ReadPatchSafe(dshHome);
        var list = new System.Collections.Generic.List<PluginStatus>();
        foreach (var (mountId, depPath) in MountEntries)
        {
            var installed = Directory.Exists(Path.Combine(modulesDir, depPath));
            var mounted = patch.Contains($"id: {mountId}", StringComparison.Ordinal);
            var block = RewriteBlock(patch, mountId, setDisabled: null);
            var disabled = mounted && block is not null &&
                           block.Value.Block.Contains("disabled: true", StringComparison.Ordinal);
            list.Add(new PluginStatus(mountId, installed, mounted, disabled));
        }
        foreach (var pkg in new[] { "echocat-skill-panel-3.0", "@linxin666/dsh-pet", "@baiiii/dsh-wallpaper-local" })
        {
            var installed = Directory.Exists(Path.Combine(modulesDir, pkg));
            list.Add(new PluginStatus(pkg, installed, bundles.Contains(pkg, StringComparer.Ordinal), Disabled: false));
        }
        return list;
    }

    private static string[] ReadBundles(string dshHome)
    {
        try
        {
            var path = Path.Combine(dshHome, "profiles", ProfileName, "package.json");
            if (!File.Exists(path))
            {
                return Array.Empty<string>();
            }
            using var doc = JsonDocument.Parse(File.ReadAllText(path));
            if (doc.RootElement.TryGetProperty("dsh", out var dsh) &&
                dsh.TryGetProperty("profile", out var profile) &&
                profile.TryGetProperty("bundles", out var bundles) &&
                bundles.ValueKind == JsonValueKind.Array)
            {
                return bundles.EnumerateArray()
                    .Where(b => b.ValueKind == JsonValueKind.String)
                    .Select(b => b.GetString() ?? "")
                    .ToArray();
            }
            return Array.Empty<string>();
        }
        catch (Exception)
        {
            return Array.Empty<string>();
        }
    }

    private static string AppendDiag(string dshHome, string message)
    {
        try
        {
            var dir = Path.Combine(dshHome, "logs");
            Directory.CreateDirectory(dir);
            File.AppendAllText(
                Path.Combine(dir, "plugin-bootstrap.log"),
                $"[{DateTime.Now:O}] {message.TrimEnd()}\n\n");
        }
        catch (Exception)
        {
            // 诊断写不进去不影响主流程
        }
        return message;
    }

    private static string Tail(StringBuilder sb, int max)
    {
        lock (sb)
        {
            var s = sb.ToString();
            return s.Length <= max ? s : "…\n" + s[^max..];
        }
    }
}
