using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Net.Http;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;

namespace Blade2.Dsh;

/// <summary>
/// dsh 内核宿主：解析内核位置（优先打包内置，回退系统 npm 安装）、
/// 启动 web 服务进程、从 stdout 捕获带 token 的 URL、管理生命周期。
/// 内核与壳的进程契约只有两个：node bin.js web --no-open --port 0 与
/// stdout 的 "dsh web: &lt;url&gt;" 行；升级 dsh 版本只影响 RPC 契约（DshRpcClient）。
/// 内核常驻（0.9.0.4 起）：壳退出不再杀内核——冷启 ~6.6s 的大头是 ~200 插件的
/// 模块装载风暴，进程级复用是唯一能到亚秒的路径。连接信息（pid/url/版本/exe 路径）
/// 原子落 DSH_HOME 状态文件，下次启动 <see cref="TryAttachResidentAsync"/> 探活复用；
/// 失效常驻（版本不符/pid 已死/探活失败）按记录路径杀树回收（防 pid 复用误杀），
/// 无状态文件的崩溃残留仍由 <see cref="SweepOrphanKernels"/> 兜底。
/// </summary>
public sealed partial class DshKernelHost : IDisposable
{
    private Process? _process;

    /// <summary>当前内核主进程 pid（spawn 或 attach 得到）；0 = 尚无内核。
    /// attached 内核的父进程是上一次的壳（已死），在清扫眼里就是孤儿——
    /// 会话中途的清扫调用必须拿它豁免，否则常驻内核会被误杀。</summary>
    internal int KernelPid => _kernelPid;

    private int _kernelPid;

    /// <summary>本次的数据家（状态文件落点），spawn/attach 时记下，重试清理用。</summary>
    private string? _dshHome;

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
        // V8 编译缓存（node ≥ 22.1）：内核模块树大，冷编译占启动的大头。缓存落 DSH_HOME
        // （随包家隔离），node 按 hash 自校验/自失效（内核或 node 升级自动重建），写失败
        // 静默忽略——第一次启动没有缓存照常起，第二次起模块加载显著提速。
        psi.EnvironmentVariables["NODE_COMPILE_CACHE"] = Path.Combine(dshHome, "kernel-compile-cache");
        // 自带 pnpm 优先（Kernel\bin\pnpm.cmd → 内置 node + pnpm.cjs）：
        // 内核 plugin-manager 与 `dsh plugin` 都经 PATH 解析 pnpm，
        // Blade² 承诺零环境要求，不能假设用户机器装了 pnpm。
        DshPluginBootstrap.PrependPath(psi, BundledBinDir);

        _process = new Process { StartInfo = psi, EnableRaisingEvents = true };
        if (!_process.Start())
        {
            DiagLine("spawn FAILED");
            return null;
        }
        DiagLine($"spawn pid={_process.Id} node={launcher.Value.fileName}");
        _kernelPid = _process.Id;
        _dshHome = dshHome;
        var tcs = new TaskCompletionSource<string?>(TaskCreationOptions.RunContinuationsAsynchronously);
        var regex = UrlPattern();

        void OnLine(string? line)
        {
            if (line is null)
            {
                return;
            }
            DiagLine($"[line] {line}");
            var match = regex.Match(line);
            if (match.Success)
            {
                DiagLine($"[url-captured] {match.Groups[1].Value}");
                tcs.TrySetResult(match.Groups[1].Value);
            }
        }

        _process.OutputDataReceived += (_, e) => OnLine(e.Data);
        _process.ErrorDataReceived += (_, e) => OnLine(e.Data);
        _process.BeginOutputReadLine();
        _process.BeginErrorReadLine();
        _process.Exited += (_, _) =>
        {
            int code = -1;
            try { if (_process.HasExited) { code = _process.ExitCode; } } catch (Exception) { }
            DiagLine($"[exited] code={code}");
            tcs.TrySetResult(null);
        };

        var completed = await Task.WhenAny(tcs.Task, Task.Delay(TimeSpan.FromSeconds(90), ct)).ConfigureAwait(false);
        if (completed != tcs.Task)
        {
            DiagLine("[timeout] no url in 90s, killing");
            try { _process.Kill(entireProcessTree: true); } catch (Exception) { }
            DeleteResidentState(dshHome);
            return null;
        }
        var url = tcs.Task.Result;
        if (url is null)
        {
            // 进程已退出；日志由 _process.ExitCode/诊断覆盖。上一轮的状态文件随之作废
            DeleteResidentState(dshHome);
            return null;
        }
        // URL 到手即登记常驻：壳退出后内核继续活，下次启动直接复用
        WriteResidentState(dshHome, new ResidentState
        {
            pid = _process.Id,
            url = url,
            exe = TryGetProcessPath(_process),
            ver = BundledDshVersion() ?? "",
        });
        DiagLine($"[resident] state written pid={_process.Id}");
        return url;
    }

    /// <summary>
    /// 常驻内核复用：读状态文件 → 版本守卫（升级/降级后旧内核代码不同，不复用）→
    /// pid 活性 + exe 路径核对（防 pid 被无关进程复用）→ HTTP 探活（带 token GET，
    /// 200 = 服务在位）。任一环失败：能确定身份的按记录路径杀树回收，然后返回 null
    /// 走冷启；探活只证 HTTP 面，RPC/mux 面由引导链既有的重试兜（真坏了重试钮兜底）。
    /// </summary>
    public async Task<string?> TryAttachResidentAsync(string dshHome, CancellationToken ct = default)
    {
        var st = ReadResidentState(dshHome);
        if (st is null || st.pid <= 0 || string.IsNullOrEmpty(st.url))
        {
            return null;
        }
        _dshHome = dshHome;
        var current = BundledDshVersion();
        if (current is null || !string.Equals(current, st.ver, StringComparison.Ordinal))
        {
            DiagLine($"[resident] version skew state={st.ver} current={current} -> recycle");
            RecycleResident(st);
            return null;
        }
        Process? proc;
        try
        {
            proc = Process.GetProcessById(st.pid);
        }
        catch (ArgumentException)
        {
            DiagLine("[resident] pid gone -> cold");
            DeleteResidentState(dshHome);
            return null;
        }
        var path = TryGetProcessPath(proc);
        if (path is null || !PathsEqual(path, st.exe))
        {
            // pid 活着但不是那个内核（复用给了别的进程）：身份对不上不杀，只清状态
            DiagLine($"[resident] pid {st.pid} reused by other process -> cold");
            proc.Dispose();
            DeleteResidentState(dshHome);
            return null;
        }
        var ok = false;
        try
        {
            using var http = new HttpClient(new HttpClientHandler { UseProxy = false })
            {
                Timeout = TimeSpan.FromSeconds(2),
            };
            using var resp = await http.GetAsync(st.url, ct).ConfigureAwait(false);
            ok = resp.IsSuccessStatusCode;
        }
        catch (Exception)
        {
            // 连接拒绝/超时/取消都算探活失败
        }
        if (!ok)
        {
            DiagLine("[resident] probe failed -> recycle");
            proc.Dispose();
            RecycleResident(st);
            return null;
        }
        _process = proc;
        _kernelPid = proc.Id;
        DiagLine($"[resident] attached pid={proc.Id}");
        return st.url;
    }

    /// <summary>重试用：这轮内核可能半坏（探活只证 HTTP 面），收掉整棵树并清状态，
    /// 下一次引导冷启。正常退出不走这里——<see cref="Dispose"/> 保活。</summary>
    public void KillAndClear()
    {
        try
        {
            if (_process is { HasExited: false })
            {
                _process.Kill(entireProcessTree: true);
            }
        }
        catch (Exception) { }
        if (_dshHome is not null)
        {
            DeleteResidentState(_dshHome);
        }
    }

    /// <summary>按状态文件的记录杀常驻内核：exe 路径逐字核对，pid 复用到无关进程时绝不误杀。</summary>
    private static void RecycleResident(ResidentState st)
    {
        if (!string.IsNullOrEmpty(st.exe))
        {
            try
            {
                using var proc = Process.GetProcessById(st.pid);
                var path = TryGetProcessPath(proc);
                if (path is not null && PathsEqual(path, st.exe))
                {
                    proc.Kill(entireProcessTree: true);
                }
            }
            catch (Exception)
            {
                // 已退出/权限不足：无可回收
            }
        }
        if (st.Home is not null)
        {
            DeleteResidentState(st.Home);
        }
    }

    private static string? TryGetProcessPath(Process proc)
    {
        try
        {
            return proc.MainModule?.FileName;
        }
        catch (Exception)
        {
            return null;
        }
    }

    private static bool PathsEqual(string a, string b)
    {
        try
        {
            return string.Equals(
                Path.TrimEndingDirectorySeparator(Path.GetFullPath(a)),
                Path.TrimEndingDirectorySeparator(Path.GetFullPath(b)),
                StringComparison.OrdinalIgnoreCase);
        }
        catch (Exception)
        {
            return false;
        }
    }

    /// <summary>打包内置内核的版本（Kernel\dsh\package.json 的 version）——
    /// 常驻复用的版本守卫用它对状态文件；读不到返回 null（守卫按不匹配处理，走冷启）。</summary>
    internal static string? BundledDshVersion()
    {
        try
        {
            var p = Path.Combine(AppContext.BaseDirectory, "Kernel", "dsh", "package.json");
            if (!File.Exists(p))
            {
                return null;
            }
            using var doc = JsonDocument.Parse(File.ReadAllText(p));
            return doc.RootElement.TryGetProperty("version", out var v) ? v.GetString() : null;
        }
        catch (Exception)
        {
            return null;
        }
    }

    private sealed class ResidentState
    {
        /// <summary>写状态时的数据家，回收时清文件用（读旧状态缺该字段则为 null）。</summary>
        public string? Home { get; set; }
        public int pid { get; set; }
        public string url { get; set; } = "";
        public string exe { get; set; } = "";
        public string ver { get; set; } = "";
    }

    private static string ResidentStatePath(string dshHome) =>
        Path.Combine(dshHome, "blade2-resident-kernel.json");

    private static ResidentState? ReadResidentState(string dshHome)
    {
        try
        {
            var p = ResidentStatePath(dshHome);
            if (!File.Exists(p))
            {
                return null;
            }
            var st = JsonSerializer.Deserialize<ResidentState>(File.ReadAllText(p));
            if (st is not null)
            {
                st.Home = dshHome;
            }
            return st;
        }
        catch (Exception)
        {
            return null;
        }
    }

    private static void WriteResidentState(string dshHome, ResidentState st)
    {
        try
        {
            Directory.CreateDirectory(dshHome);
            var p = ResidentStatePath(dshHome);
            var tmp = p + ".tmp";
            st.Home = dshHome;
            File.WriteAllText(tmp, JsonSerializer.Serialize(st));
            File.Move(tmp, p, overwrite: true);
        }
        catch (Exception)
        {
            // 写不进（权限/磁盘）：本轮照常用，下次冷启，不影响功能
        }
    }

    private static void DeleteResidentState(string dshHome)
    {
        try
        {
            File.Delete(ResidentStatePath(dshHome));
        }
        catch (Exception) { }
    }

    /// <summary>内核启动观测仪：0.8.3.3 起的"Unable to connect"失败卡需要内核侧 stdout/
    /// stderr 逐行现场（打包环境与开发环境差异无法离线复现）。无条件落 %TEMP%，量级是
    /// 每次启动几十行，定位后移除。</summary>
    internal static void DiagLine(string line)
    {
        try
        {
            File.AppendAllText(Path.Combine(Path.GetTempPath(), "blade2-kernel-boot.log"),
                $"{DateTime.Now:HH:mm:ss.fff} {line}\r\n");
        }
        catch (Exception)
        {
            // 诊断写不进去不影响主流程
        }
    }

    /// <summary>正常退出（关窗/系统关机/Shutdown）：内核保活常驻，只释放本端的进程句柄——
    /// 下次启动 <see cref="TryAttachResidentAsync"/> 探活复用，模块装载风暴整段跳过。
    /// 需要真正收内核（引导失败重试）走 <see cref="KillAndClear"/>。</summary>
    public void Dispose()
    {
        try
        {
            _process?.Dispose();
        }
        catch (Exception) { }
        _process = null;
        _kernelPid = 0;
    }

    /// <summary>
    /// 判定内核错误是不是"写租约被占"：`SessionAlreadyOwnedError: session "…" is already owned
    /// by an active write handle`（从 resume 失败里冒出来）。这不是发送内容的问题，是上一个没死透的
    /// 内核还占着会话目录；按发送失败报给用户解决不了问题，得清掉残留进程再发。
    /// </summary>
    public static bool IsSessionLeaseBusy(Exception ex) =>
        !string.IsNullOrEmpty(ex.Message)
        && (ex.Message.Contains("SessionAlreadyOwnedError", StringComparison.Ordinal)
            || ex.Message.Contains("already owned by an active write handle", StringComparison.Ordinal));

    /// <summary>
    /// 清理残留内核进程（本安装 Kernel\node.exe 且父进程链上没有活着的 Blade²）。
    /// 常驻设计（0.9.0.4）下壳正常退出也留内核：它占着会话写租约正是复用的前提，
    /// 会话中途的清扫调用必须传 <paramref name="sparePid"/>（当前内核 pid）豁免；
    /// 引导期清扫发生在 attach 之后，要么已复用（跳过清扫）要么已回收（无需豁免）。
    /// 判定只认两条硬条件，缺一不杀——可执行路径正是本安装的 Kernel\node.exe，
    /// 且父进程链上没有任何活着的 Blade²（并发第二个实例的内核父进程还活着，绝不动；
    /// 别的 node 进程路径对不上，也绝不动）。
    /// </summary>
    /// <returns>杀掉的内核主进程数。</returns>
    public static int SweepOrphanKernels(int sparePid = 0)
    {
        try
        {
            return SweepOrphanKernelsCore(sparePid);
        }
        catch (Exception)
        {
            // 带外异常（枚举/快照失败）不能让调用点出事：调用点可能是异常筛选器
            return 0;
        }
    }

    private static int SweepOrphanKernelsCore(int sparePid)
    {
        var bundled = BundledNode;
        if (bundled is null)
        {
            return 0;
        }
        var parents = SnapshotProcessParents();
        var killed = 0;
        foreach (var proc in Process.GetProcessesByName("node"))
        {
            try
            {
                if (proc.Id == sparePid)
                {
                    continue; // 本轮在用/常驻内核：调用方点名豁免
                }
                string? path;
                try
                {
                    path = proc.MainModule?.FileName;
                }
                catch (Exception)
                {
                    continue; // 取不到模块路径（已退出/权限）：无法确认身份就不碰
                }
                if (path is null || !string.Equals(
                        Path.TrimEndingDirectorySeparator(Path.GetFullPath(path)),
                        Path.TrimEndingDirectorySeparator(Path.GetFullPath(bundled)),
                        StringComparison.OrdinalIgnoreCase))
                {
                    continue;
                }
                // 祖先链未知（进程在快照之后才 spawn，快照里没有它的条目）一律跳过：
                // 本轮刚拉起的内核/子进程绝不能凭"查不到父进程"处决——真孤儿（上次崩溃
                // 残留）在快照时点就已存在，条目齐全，照常可杀。
                if (HasLiveAppAncestor(proc.Id, parents) is var verdict && verdict is null)
                {
                    continue;
                }
                if (verdict.Value)
                {
                    continue; // 还有活着的壳在撑它：不是孤儿
                }
                // 连带内核自己起的 MCP 子进程（server-memory、playwright 等）一起收
                proc.Kill(entireProcessTree: true);
                killed++;
            }
            catch (Exception)
            {
                // 抢不动的（正在退出/权限不足）跳过：下一次启动还会再扫
            }
            finally
            {
                proc.Dispose();
            }
        }
        return killed;
    }

    /// <summary>快照当前全部进程的 子→父 关系（一次 toolhelp 抓取，够本进程一次性判定用）。</summary>
    private static Dictionary<int, int> SnapshotProcessParents()
    {
        var map = new Dictionary<int, int>();
        var handle = NativeMethods.CreateToolhelp32Snapshot(
            NativeMethods.TH32CS_SNAPPROCESS, 0);
        if (handle == NativeMethods.INVALID_HANDLE_VALUE)
        {
            return map;
        }
        try
        {
            var entry = new PROCESSENTRY32 { dwSize = (uint)Marshal.SizeOf<PROCESSENTRY32>() };
            if (!NativeMethods.Process32FirstW(handle, ref entry))
            {
                return map;
            }
            do
            {
                map[(int)entry.th32ProcessID] = (int)entry.th32ParentProcessID;
            }
            while (NativeMethods.Process32NextW(handle, ref entry));
        }
        finally
        {
            NativeMethods.CloseHandle(handle);
        }
        return map;
    }

    /// <summary>沿父链往上找活着的 Blade²：内核必然由壳（直系或 cmd.exe 壳）启动，
    /// 找不到活壳就是崩溃残留。深度封顶防 PID 回环。返回 null = 祖先链未知
    /// （进程不在快照里，spawn 晚于快照）：不能判死，调用方必须跳过。</summary>
    private static bool? HasLiveAppAncestor(int pid, Dictionary<int, int> parents)
    {
        var visited = new HashSet<int>();
        for (var depth = 0; depth < 16; depth++)
        {
            if (!parents.TryGetValue(pid, out var parent) || parent <= 0 || !visited.Add(pid))
            {
                return null; // 不在快照内：本轮 spawn 的新进程，不可判孤儿
            }
            if (IsAppProcess(parent))
            {
                return true;
            }
            pid = parent;
        }
        return false;
    }

    private static bool IsAppProcess(int pid)
    {
        try
        {
            using var proc = Process.GetProcessById(pid);
            // 打包形态进程名 Blade2；未打包自包含 exe 也是 Blade2（dotnet 宿主形态是 dotnet，
            // 那种形态内核走 dsh.cmd 回退、路径对不上本安装，本函数参与不到判定）
            return string.Equals(proc.ProcessName, "Blade2", StringComparison.OrdinalIgnoreCase);
        }
        catch (ArgumentException)
        {
            return false; // 父进程已退出 = 没有活壳
        }
        catch (InvalidOperationException)
        {
            return false;
        }
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct PROCESSENTRY32
    {
        public uint dwSize;
        public uint cntUsage;
        public uint th32ProcessID;
        public IntPtr th32DefaultHeapID;
        public uint th32ModuleID;
        public uint cntThreads;
        public IntPtr th32ParentProcessID;
        public int pcPriClassBase;
        public uint dwFlags;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)]
        public string szExeFile;
    }

    private static partial class NativeMethods
    {
        internal static readonly IntPtr INVALID_HANDLE_VALUE = new(-1);
        internal const uint TH32CS_SNAPPROCESS = 0x00000002;

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        internal static extern bool CloseHandle(IntPtr handle);

        [DllImport("kernel32.dll", SetLastError = true)]
        internal static extern IntPtr CreateToolhelp32Snapshot(uint flags, uint processId);

        [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        [return: MarshalAs(UnmanagedType.Bool)]
        internal static extern bool Process32FirstW(IntPtr snapshot, ref PROCESSENTRY32 entry);

        [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        [return: MarshalAs(UnmanagedType.Bool)]
        internal static extern bool Process32NextW(IntPtr snapshot, ref PROCESSENTRY32 entry);
    }
}
