using System;
using System.Diagnostics;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Blade2.Dsh;

namespace Blade2;

/// <summary>
/// 内核异步引导的界面状态机。壳先亮相（首页/导航/皮肤/托盘都立即可用），内核在后台起，
/// 进度摆在主页正中间的加载卡上；失败停在原卡给原因与重试钮。
/// 与旧实现的差别：没有盖住整个窗口的全屏加载层——加载状态是内容面里的一张卡，
/// 内核还没就绪时界面其余部分照常可操作（只有依赖内核的动作给"还在加载"的提示）。
/// </summary>
public sealed partial class MainWindow
{
    /// <summary>
    /// 引导阶段表（主页加载卡上的进度刻度）。百分比表达的是"流程走到哪一步"，
    /// 不是耗时估计：每一步在自己的区间内推进，区间内能测到真实进度的（插件安装）
    /// 按真实刻度插值，测不到的内核进程/RPC/会话清单匀速爬到区间上沿。
    /// </summary>
    private static readonly (string StageKey, int Step, double From, double To)[] KernelBootStages =
    {
        ("正在准备内核组件…", 1, 5, 30),
        ("正在启动内核…", 2, 30, 60),
        ("正在连接内核…", 3, 60, 82),
        ("正在加载工作区与会话…", 4, 82, 96),
    };
    private static readonly int KernelBootStepTotal = KernelBootStages.Length;

    /// <summary>动画步进速率：每 tick（100ms）前进/回退的百分比上限，保证条子平滑不跳。</summary>
    private const double KernelBootRateForward = 1.5;
    private const double KernelBootRateBack = 3.0;

    private CancellationTokenSource? _kernelBootCts;
    private readonly Stopwatch _kernelBootWatch = new();
    private DispatcherQueueTimer? _kernelBootTick;
    private bool _kernelBootTickWired;
    /// <summary>当前阶段（1 起）；失败/就绪后停在原值，供进度行显示"卡在第几步"。</summary>
    private int _bootStep = 1;
    /// <summary>进度条目标值（0-100）与已显示值（动画插值出的现值）。</summary>
    private double _bootTarget;
    private double _bootShown;
    /// <summary>第 1 步的插件安装子进度（null = 本轮无包要装）。</summary>
    private (int Ready, int Total)? _bootPlugins;
    /// <summary>失败态原文（reasonKey 可本地化，detail 是异常原文）：语言切换时按当前语言重刷加载卡。</summary>
    private (string ReasonKey, string Detail)? _kernelBootFailure;
    /// <summary>发送键在内核就绪前被按过时，只提示一次（flag 随每轮引导重置）。</summary>
    private bool _kernelWaitHintShown;

    /// <summary>进加载态：加载卡上屏并让空态标题让位（见 <see cref="UpdateEmptyState"/>）。</summary>
    private void ShowKernelBootPanel()
    {
        _kernelWaitHintShown = false;
        _kernelBootFailure = null;
        _bootPlugins = null;
        _bootStep = 1;
        _bootTarget = KernelBootStages[0].From;
        _bootShown = 0;
        _kernelBootWatch.Restart();
        PostUi(() =>
        {
            KernelBootPanel.Visibility = Visibility.Visible;
            ChatHero.Visibility = Visibility.Collapsed; // 加载态不摆"开始一段新的会话"
            KernelBootStage.Text = L(KernelBootStages[0].StageKey);
            KernelBootBar.Value = 0;
            // 步骤行一上屏就按当前语言写（XAML 初始值是中文，等首个 tick 才本地化会闪一下）
            KernelBootStep.Text = BootStepLine();
            KernelBootRetry.Content = L("重试");
            KernelBootRetry.Visibility = Visibility.Collapsed;
            KernelBootHint.Text = "";
            KernelBootHint.Visibility = Visibility.Collapsed;
        });
        StartKernelBootTick();
    }

    /// <summary>
    /// 阶段上报（引导线程调用）：换一行阶段文案，并把进度条目标值推到该步区间的上沿。
    /// 未登记的 stageKey 直接忽略——宁可不显示，也不拿假进度糊弄界面。
    /// </summary>
    private void ReportKernelBootStage(string stageKey)
    {
        var idx = Array.FindIndex(KernelBootStages, s => s.StageKey == stageKey);
        if (idx < 0)
        {
            return;
        }
        _bootStep = KernelBootStages[idx].Step;
        // 文案跟着换（引导线程 → UI 线程编队）：阶段名与步骤号必须同源，否则步骤行进到
        // 第 4 步而标题还写"正在准备内核组件"，用户看到的是自相矛盾的两行。
        PostUi(() => KernelBootStage.Text = L(stageKey));
        RecomputeBootTarget();
    }

    /// <summary>插件安装进度（bootstrap 的抽样 timer 上报）：只在第 1 步区间内插值。</summary>
    private void ReportKernelBootPlugins(int ready, int total)
    {
        _bootPlugins = total > 0 ? (ready, total) : null;
        RecomputeBootTarget();
    }

    private void RecomputeBootTarget()
    {
        var stage = KernelBootStages[Math.Clamp(_bootStep, 1, KernelBootStepTotal) - 1];
        // 第 1 步有真刻度（node_modules 里就绪的包数）；其余步没有可测的中间量，匀速爬满。
        var fraction = _bootStep == 1 && _bootPlugins is { } p && p.Total > 0
            ? Math.Clamp((double)p.Ready / p.Total, 0, 1)
            : 1.0;
        _bootTarget = stage.From + (stage.To - stage.From) * fraction;
    }

    /// <summary>引导成功：进度条推满并撤下加载卡（空态/会话清单的显隐交回正常逻辑）。</summary>
    private void CompleteKernelBootProgress()
    {
        _bootTarget = 100;
        _bootShown = 100;
        _bootPlugins = null;
    }

    /// <summary>引导成功：进度条推满，加载卡撤下，空态/会话清单的显隐交回正常逻辑。</summary>
    private void HideKernelBootPanel()
    {
        StopKernelBootTick();
        _kernelBootWatch.Stop();
        PostUi(() =>
        {
            KernelBootPanel.Visibility = Visibility.Collapsed;
        });
    }

    /// <summary>
    /// 引导失败：原因摆在原卡上 + 重试钮；内核起不来是用户必须知道的事，再走系统通知。
    /// detail 是异常原文（本地化不了的排障信息），reasonKey 是壳可翻译的失败定性。
    /// 进度条停在失败时的位置：它告诉用户卡在第几步，比重置成 0 更有信息量。
    /// </summary>
    private void FailKernelBoot(string reasonKey, string? detail = null)
    {
        StopKernelBootTick();
        _kernelBootWatch.Stop();
        _kernelBootFailure = (reasonKey, detail ?? "");
        var message = L(reasonKey);
        var full = string.IsNullOrEmpty(detail) ? message : $"{message}：{detail}";
        PostUi(() =>
        {
            KernelBootStage.Text = L("内核启动失败");
            KernelBootRetry.Content = L("重试");
            KernelBootHint.Text = full;
            KernelBootHint.Visibility = Visibility.Visible;
            KernelBootRetry.Visibility = Visibility.Visible;
            // 进度行留着失败瞬间的位置：「第 2 步，共 4 步 · 34%」
            KernelBootStep.Text = BootStepLine();
        });
        ShellToast.Show(L("Blade²"), full);
    }

    /// <summary>进度行文本：第几步 / 共几步 / 当前百分比。</summary>
    private string BootStepLine() =>
        LF("第 {0} 步，共 {1} 步 · {2}%", Math.Clamp(_bootStep, 1, KernelBootStepTotal), KernelBootStepTotal,
            (int)Math.Round(_bootShown));

    /// <summary>
    /// 语言切换时重刷加载卡：进度行/秒表句/失败详情都是插值或异常原文，可视树扫描按
    /// 当前文本反查不到字典键，只能按当前状态整卡重算（面板不可见时无事可做）。
    /// </summary>
    private void RefreshKernelBootTexts()
    {
        if (KernelBootPanel.Visibility != Visibility.Visible)
        {
            return;
        }
        KernelBootRetry.Content = L("重试");
        if (_kernelBootFailure is { } fail)
        {
            var message = L(fail.ReasonKey);
            KernelBootStage.Text = L("内核启动失败");
            KernelBootHint.Text = fail.Detail.Length == 0 ? message : $"{message}：{fail.Detail}";
            KernelBootHint.Visibility = Visibility.Visible;
            return;
        }
        PaintKernelBootTick();
    }

    /// <summary>启动进度动画/秒表 ticker（与 _sessionsRefreshTimer 同款口径：窗口的 DispatcherQueue）。</summary>
    private void StartKernelBootTick()
    {
        _kernelBootTick ??= DispatcherQueue.CreateTimer();
        _kernelBootTick.Interval = TimeSpan.FromMilliseconds(100);
        if (!_kernelBootTickWired)
        {
            _kernelBootTickWired = true;
            _kernelBootTick.Tick += OnKernelBootTick;
        }
        _kernelBootTick.Start();
    }

    private void StopKernelBootTick()
    {
        _kernelBootTick?.Stop();
    }

    /// <summary>
    /// 进度动画 + 提示行走字（每 100ms）。进度条朝目标值爬（进/退速率不同，避免重试时倒退刺眼）；
    /// 提示行优先给插件安装的真实刻度，否则报已用秒数——长任务不能看着像卡死。
    /// </summary>
    private void OnKernelBootTick(DispatcherQueueTimer sender, object args)
    {
        if (_kernelBootFailure is not null)
        {
            sender.Stop(); // 失败态停在原地：那一行让给原因
            return;
        }
        PaintKernelBootTick();
    }

    /// <summary>把当前目标值/插件进度/秒数画到加载卡上（tick 与语言切换共用）。</summary>
    private void PaintKernelBootTick()
    {
        var delta = _bootTarget - _bootShown;
        if (Math.Abs(delta) < 0.05)
        {
            _bootShown = _bootTarget;
        }
        else
        {
            var rate = delta > 0 ? KernelBootRateForward : KernelBootRateBack;
            _bootShown += Math.Clamp(delta, -rate, rate);
        }

        KernelBootBar.Value = _bootShown;
        KernelBootStep.Text = BootStepLine();

        if (_bootStep == 1 && _bootPlugins is { Ready: var ready, Total: var total } && ready < total)
        {
            KernelBootHint.Text = LF("正在安装插件 {0}/{1}", ready, total);
            KernelBootHint.Visibility = Visibility.Visible;
            return;
        }
        var seconds = (int)_kernelBootWatch.Elapsed.TotalSeconds;
        if (seconds <= 0)
        {
            KernelBootHint.Text = "";
            KernelBootHint.Visibility = Visibility.Collapsed;
            return;
        }
        KernelBootHint.Text = LF("内核加载已进行 {0} 秒", seconds);
        KernelBootHint.Visibility = Visibility.Visible;
    }

    /// <summary>
    /// 起一轮内核引导（构造末尾调一次；重试钮再调一次）。窗口此时已经亮相，
    /// 这条链路只是后台把内核拉起来，不阻塞任何界面。
    /// </summary>
    private void StartKernelBoot()
    {
        ShowKernelBootPanel();
        var cts = new CancellationTokenSource();
        _kernelBootCts = cts;
        // 异常已在 RunKernelBootAsync 内部全数收敛（失败态/取消），这里不需要再兜一层
        _ = RunKernelBootAsync(cts.Token);
    }

    /// <summary>插件安装进度桥：bootstrap 的后台抽样 → 加载卡（壳可能已关窗，PostUi 自己兜底）。</summary>
    private IProgress<Blade2.Dsh.DshPluginBootstrap.PluginInstallProgress> CreatePluginProgress() =>
        new Progress<Blade2.Dsh.DshPluginBootstrap.PluginInstallProgress>(p =>
            PostUi(() => ReportKernelBootPlugins(p.Ready, p.Total)));

    /// <summary>重试钮：硬取消上一轮引导，再走一遍完整流程。</summary>
    private async void OnKernelBootRetryClick(object sender, RoutedEventArgs e)
    {
        KernelBootRetry.Visibility = Visibility.Collapsed;
        await RestartKernelBootAsync();
    }

    /// <summary>
    /// 重试：取消上一轮 → 收掉半成品（旧内核进程 / 旧 RPC 连接 / 旧长驻流标记）→ 重新引导。
    /// 旧宿主必须换新：上一轮的内核进程可能还在，复用它第二次 StartAsync 会互相抢占。
    /// </summary>
    private async Task RestartKernelBootAsync()
    {
        CancelKernelBoot();
        // 旧 RPC 先摘：上面挂的事件/流都随旧实例一起作废，新实例由引导链路重新注册
        var staleRpc = _rpc;
        _rpc = null;
        if (staleRpc is not null)
        {
            try
            {
                await staleRpc.DisposeAsync();
            }
            catch (Exception)
            {
                // 旧连接已断的情况下 Dispose 也会抛；收不掉就算了，新连接照样能建
            }
        }
        _kernel.Dispose();
        _kernel = new DshKernelHost();
        // 长驻流标记按旧连接作废：否则新一轮 RefreshWorkspaces/OpenSessionControl 会误判"已开"
        _workspaceStreamOpen = false;
        _controlStreamId = null;
        _sessionStreamId = null;
        _businessStreamsResetPending = false;
        StartKernelBoot();
    }

    /// <summary>取消进行中的引导（重试/关窗各调一次）。CTS 只能取消并释放一次。</summary>
    private void CancelKernelBoot()
    {
        var cts = _kernelBootCts;
        _kernelBootCts = null;
        if (cts is null)
        {
            return;
        }
        try
        {
            cts.Cancel();
        }
        catch (ObjectDisposedException) { }
        catch (AggregateException) { }
        cts.Dispose();
    }
}
