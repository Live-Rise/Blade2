using System;
using System.Diagnostics;
using System.IO;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;

namespace Blade2.Dsh;

/// <summary>
/// dsh 内核宿主：解析内核位置（优先打包内置，回退系统 npm 安装）、
/// 启动 web 服务进程、从 stdout 捕获带 token 的 URL、管理生命周期。
/// 内核与壳的进程契约只有两个：node bin.js web --no-open --port 0 与
/// stdout 的 "dsh web: &lt;url&gt;" 行；升级 dsh 版本只影响 RPC 契约（DshRpcClient）。
/// </summary>
public sealed partial class DshKernelHost : IDisposable
{
    private Process? _process;

    /// <summary>打包内置内核的 node.exe（&lt;install&gt;\Kernel\node.exe）。</summary>
    internal static string? BundledNode =>
        File.Exists(Path.Combine(AppContext.BaseDirectory, "Kernel", "node.exe"))
            ? Path.Combine(AppContext.BaseDirectory, "Kernel", "node.exe")
            : null;

    /// <summary>打包内置内核入口（&lt;install&gt;\Kernel\dsh\lib\bin.js）。</summary>
    internal static string? BundledBinJs =>
        File.Exists(Path.Combine(AppContext.BaseDirectory, "Kernel", "dsh", "lib", "bin.js"))
            ? Path.Combine(AppContext.BaseDirectory, "Kernel", "dsh", "lib", "bin.js")
            : null;

    /// <summary>Kernel\bin（自带 pnpm.cmd 所在目录）；PATH 前置注入后 dsh plugin /
    /// plugin-manager 可在无系统 pnpm 的机器上解析到它。</summary>
    internal static string BundledBinDir => Path.Combine(AppContext.BaseDirectory, "Kernel", "bin");

    public bool IsBundled => BundledNode is not null && BundledBinJs is not null;

    /// <summary>
    /// 解析启动命令：内置内核优先（零外部依赖）；否则回退系统 dsh.cmd
    /// （npm 全局安装形态，开发机场景）。两者参数完全一致。
    /// </summary>
    private (string fileName, string arguments)? ResolveLauncher()
    {
        var node = BundledNode;
        var binJs = BundledBinJs;
        if (node is not null && binJs is not null)
        {
            return (node, $"\"{binJs}\" web --no-open --port 0");
        }

        var dshCmd = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "npm", "dsh.cmd");
        if (File.Exists(dshCmd))
        {
            return ("cmd.exe", $"/c \"\"{dshCmd}\" web --no-open --port 0\"");
        }
        return null;
    }

    [GeneratedRegex(@"dsh web: (https?://\S+)")]
    private static partial Regex UrlPattern();

    /// <summary>
    /// 启动内核并等待 stdout 打印 URL。DSH_HOME 指向 Blade² 专属数据目录，
    /// 与系统 ~/.dsh 隔离（内置内核的沙盒数据家）。
    /// </summary>
    public async Task<string?> StartAsync(string dshHome, CancellationToken ct = default)
    {
        var launcher = ResolveLauncher();
        if (launcher is null)
        {
            return null;
        }

        var psi = new ProcessStartInfo
        {
            FileName = launcher.Value.fileName,
            Arguments = launcher.Value.arguments,
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
        };
        psi.EnvironmentVariables["DSH_HOME"] = dshHome;
        // 自带 pnpm 优先（Kernel\bin\pnpm.cmd → 内置 node + pnpm.cjs）：
        // 内核 plugin-manager 与 `dsh plugin` 都经 PATH 解析 pnpm，
        // Blade² 承诺零环境要求，不能假设用户机器装了 pnpm。
        DshPluginBootstrap.PrependPath(psi, BundledBinDir);

        _process = new Process { StartInfo = psi, EnableRaisingEvents = true };
        if (!_process.Start())
        {
            return null;
        }
        var tcs = new TaskCompletionSource<string?>(TaskCreationOptions.RunContinuationsAsynchronously);
        var regex = UrlPattern();

        void OnLine(string? line)
        {
            if (line is null)
            {
                return;
            }
            var match = regex.Match(line);
            if (match.Success)
            {
                tcs.TrySetResult(match.Groups[1].Value);
            }
        }

        _process.OutputDataReceived += (_, e) => OnLine(e.Data);
        _process.ErrorDataReceived += (_, e) => OnLine(e.Data);
        _process.BeginOutputReadLine();
        _process.BeginErrorReadLine();
        _process.Exited += (_, _) => tcs.TrySetResult(null);

        var completed = await Task.WhenAny(tcs.Task, Task.Delay(TimeSpan.FromSeconds(90), ct)).ConfigureAwait(false);
        if (completed != tcs.Task)
        {
            try { _process.Kill(entireProcessTree: true); } catch (Exception) { }
            return null;
        }
        var url = tcs.Task.Result;
        if (url is null)
        {
            return null; // 进程已退出；日志由 _process.ExitCode/诊断覆盖
        }
        return url;
    }

    public void Dispose()
    {
        try
        {
            if (_process is { HasExited: false })
            {
                _process.Kill(entireProcessTree: true);
            }
            _process?.Dispose();
        }
        catch (Exception)
        {
            // 内核可能已自行退出；无需处理。
        }
    }
}
