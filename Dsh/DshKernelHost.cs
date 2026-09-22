using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
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

    /// <summary>作业对象句柄（JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE）：壳进程消失即带走内核整棵树。
    /// 本实例存活期间必须保持开启（最后一个句柄关闭才是触发点），故只在 <see cref="Dispose"/> 里收。</summary>
    private IntPtr _job = IntPtr.Zero;

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
        AttachToKillJob();
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
        finally
        {
            // 作业对象句柄关闭即触发 KILL_ON_JOB_CLOSE：正常退出也确保内核整棵树彻底消失
            if (_job != IntPtr.Zero)
            {
                NativeMethods.CloseHandle(_job);
                _job = IntPtr.Zero;
            }
        }
    }

    /// <summary>
    /// 把内核进程挂进"壳亡即杀"的作业对象。内核持有每条会话目录的写租约（内核进程死亡才释放），
    /// 壳 hard crash 时子进程不会跟着退：不套这层，崩溃一次就留一个占着租约的孤儿内核，
    /// 之后重进同一条会话必撞 SessionAlreadyOwnedError（resume failed），写操作全废。
    /// 句柄保留到 <see cref="Dispose"/>——KILL_ON_JOB_CLOSE 的触发点正是"最后一个句柄关闭"。
    /// 任一步失败都只静默退出：内核照常能用，只是少了这层保底（仍有 <see cref="SweepOrphanKernels"/> 兜底）。
    /// </summary>
    private void AttachToKillJob()
    {
        try
        {
            var job = NativeMethods.CreateJobObjectW(IntPtr.Zero, null);
            if (job == IntPtr.Zero)
            {
                return;
            }
            var info = new JOBOBJECT_EXTENDED_LIMIT_INFORMATION
            {
                BasicLimitInformation = new JOBOBJECT_BASIC_LIMIT_INFORMATION
                {
                    LimitFlags = NativeMethods.JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                },
            };
            var length = Marshal.SizeOf<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>();
            var ptr = Marshal.AllocHGlobal(length);
            try
            {
                Marshal.StructureToPtr(info, ptr, false);
                if (!NativeMethods.SetInformationJobObject(
                        job, NativeMethods.JobObjectExtendedLimitInformation, ptr, (uint)length))
                {
                    NativeMethods.CloseHandle(job);
                    return;
                }
            }
            finally
            {
                Marshal.FreeHGlobal(ptr);
            }
            try
            {
                if (!NativeMethods.AssignProcessToJobObject(job, _process!.Handle))
                {
                    // 已在别的作业里（宿主/沙箱场景）：别人的作业不改，只收掉自己这个空作业
                    System.Diagnostics.Debug.WriteLine(
                        $"[kernel] 挂作业失败（可能已在别的作业里）Win32Error={Marshal.GetLastWin32Error()}");
                    NativeMethods.CloseHandle(job);
                    return;
                }
                // 套没套上必须回头看：这类静默失败不抛异常，只有实机杀掉壳进程才暴露得出来
                if (!NativeMethods.IsProcessInJob(_process.Handle, job, out var armed) || !armed)
                {
                    System.Diagnostics.Debug.WriteLine(
                        $"[kernel] 作业未生效 Win32Error={Marshal.GetLastWin32Error()}，内核崩溃残留将依赖启动清扫兜底");
                    NativeMethods.CloseHandle(job);
                    return;
                }
            }
            catch (Exception)
            {
                // 取不到进程句柄（已退出/权限）：作业留着也没人可杀，关掉
                NativeMethods.CloseHandle(job);
                throw;
            }
            _job = job;
        }
        catch (Exception)
        {
            // 保底层缺失不影响内核本体
        }
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
    /// 清理上次异常退出残留的内核进程。套了作业对象（<see cref="AttachToKillJob"/>）后不该再有新的孤儿，
    /// 但升级过来的机器上还留着老版本崩溃时留下的：它占着会话写租约，新内核 resume 同一条会话就撞
    /// SessionAlreadyOwnedError。判定只认两条硬条件，缺一不杀——
    /// 可执行路径正是本安装的 Kernel\node.exe，且父进程链上没有任何活着的 Blade²
    /// （并发第二个实例的内核父进程还活着，绝不动；别的 node 进程路径对不上，也绝不动）。
    /// </summary>
    /// <returns>杀掉的内核主进程数。</returns>
    public static int SweepOrphanKernels()
    {
        try
        {
            return SweepOrphanKernelsCore();
        }
        catch (Exception)
        {
            // 带外异常（枚举/快照失败）不能让调用点出事：调用点可能是异常筛选器
            return 0;
        }
    }

    private static int SweepOrphanKernelsCore()
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
                if (HasLiveAppAncestor(proc.Id, parents))
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
    /// 找不到活壳就是崩溃残留。深度封顶防 PID 回环。</summary>
    private static bool HasLiveAppAncestor(int pid, Dictionary<int, int> parents)
    {
        var visited = new HashSet<int>();
        for (var depth = 0; depth < 16; depth++)
        {
            if (!parents.TryGetValue(pid, out var parent) || parent <= 0 || !visited.Add(pid))
            {
                return false;
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

    [StructLayout(LayoutKind.Sequential)]
    private struct JOBOBJECT_BASIC_LIMIT_INFORMATION
    {
        public long PerProcessUserTimeLimit;
        public long PerJobUserTimeLimit;
        public uint LimitFlags;
        public UIntPtr MinimumWorkingSetSize;
        public UIntPtr MaximumWorkingSetSize;
        public uint ActiveProcessLimit;
        public UIntPtr Affinity;
        public uint PriorityClass;
        public uint SchedulingClass;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct IO_COUNTERS
    {
        public ulong ReadOperationCount;
        public ulong WriteOperationCount;
        public ulong OtherOperationCount;
        public ulong ReadTransferCount;
        public ulong WriteTransferCount;
        public ulong OtherTransferCount;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION
    {
        public JOBOBJECT_BASIC_LIMIT_INFORMATION BasicLimitInformation;
        public IO_COUNTERS IoInfo;
        public UIntPtr ProcessMemoryLimit;
        public UIntPtr JobMemoryLimit;
        public UIntPtr PeakProcessMemoryUsed;
        public UIntPtr PeakJobMemoryUsed;
    }

    private static partial class NativeMethods
    {
        internal static readonly IntPtr INVALID_HANDLE_VALUE = new(-1);
        internal const uint JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x00002000;
        internal const uint TH32CS_SNAPPROCESS = 0x00000002;
        // JOBOBJECTINFOCLASS::JobObjectExtendedLimitInformation。传错类（如 2 与 144 字节的
        // 扩展结构长度不匹配）SetInformationJobObject 会以 ERROR_BAD_LENGTH 静默失败，
        // 结果就是作业没套上、内核照样留孤儿——这类失败不抛异常，只能靠实机验证兜。
        internal const int JobObjectExtendedLimitInformation = 9;

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        internal static extern IntPtr CreateJobObjectW(IntPtr jobAttributes, string? name);

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        internal static extern bool SetInformationJobObject(
            IntPtr job, int infoClass, IntPtr info, uint length);

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        internal static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        internal static extern bool IsProcessInJob(IntPtr process, IntPtr job, out bool result);

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
