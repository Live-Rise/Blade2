using System;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Blade2.Dsh;

namespace Blade2;

/// <summary>
/// 内核后台静默引导。壳直接进界面：引导全程不产生任何加载显示（没有进度条、没有阶段文案、
/// 没有提示气泡），主页正中间只有一张失败卡——仅当内核起不来时才上屏，给原因与重试钮。
/// 引导期间按下的发送静默排队（输入框原样保留），引导完成后自动补发一次
/// （排队标记见 <see cref="SendAsync"/>，补发见 <see cref="FlushSendQueuedWhileBooting"/>）。
/// </summary>
public sealed partial class MainWindow
{
    private CancellationTokenSource? _kernelBootCts;

    /// <summary>失败态原文（reasonKey 可本地化，detail 是异常原文）：语言切换时按当前语言重刷失败卡。</summary>
    private (string ReasonKey, string Detail)? _kernelBootFailure;

    /// <summary>内核就绪前按过发送：只记一笔，不提示不动输入框，就绪后补发一次。</summary>
    private bool _sendQueuedWhileBooting;

    /// <summary>内核就绪前点过快照侧栏上的会话（工作区优先上屏的代价：清单先于内核可用）。
    /// 记 id 一笔，就绪后按最新清单找到就打开；基线里已消失（被删）则作废。</summary>
    private volatile string? _pendingOpenSessionId;

    /// <summary>
    /// 引导失败：原因摆在主页正中的失败卡上 + 重试钮，再走系统通知——内核起不来是
    /// 用户必须知道的事。detail 是异常原文（本地化不了的排障信息），reasonKey 是壳可翻译的失败定性。
    /// </summary>
    private void FailKernelBoot(string reasonKey, string? detail = null)
    {
        _kernelBootFailure = (reasonKey, detail ?? "");
        var message = L(reasonKey);
        var full = string.IsNullOrEmpty(detail) ? message : $"{message}：{detail}";
        PostUi(() =>
        {
            KernelBootPanel.Visibility = Visibility.Visible;
            ChatHero.Visibility = Visibility.Collapsed; // 失败卡在正中：空态标题让位
            KernelBootStage.Text = L("内核启动失败");
            KernelBootHint.Text = full;
            KernelBootHint.Visibility = Visibility.Visible;
            KernelBootRetry.Content = L("重试");
            KernelBootRetry.Visibility = Visibility.Visible;
            // 引导期点过的快照会话没开成：撤选中让空态回来（重试成功后 Flush 会再打开它）
            if (_pendingOpenSessionId is not null)
            {
                Volatile.Write(ref _activeSessionId, null);
                UpdateEmptyState();
                RestoreActiveSessionSelection();
            }
        });
        ShellToast.Show(L("Blade²"), full);
    }

    /// <summary>撤下失败卡（只对真在上屏的卡生效）：空态/会话清单的显隐交回正常逻辑。</summary>
    private void HideKernelBootPanel()
    {
        PostUi(() =>
        {
            if (KernelBootPanel.Visibility != Visibility.Visible)
            {
                return;
            }
            KernelBootPanel.Visibility = Visibility.Collapsed;
            UpdateEmptyState();
        });
    }

    /// <summary>
    /// 语言切换时重刷失败卡：原因含插值/异常原文，可视树扫描按当前文本反查不到字典键，
    /// 只能按当前状态整卡重算（面板不可见或无失败态时无事可做）。
    /// </summary>
    private void RefreshKernelBootTexts()
    {
        if (KernelBootPanel.Visibility != Visibility.Visible || _kernelBootFailure is not { } fail)
        {
            return;
        }
        var message = L(fail.ReasonKey);
        KernelBootStage.Text = L("内核启动失败");
        KernelBootHint.Text = fail.Detail.Length == 0 ? message : $"{message}：{fail.Detail}";
        KernelBootHint.Visibility = Visibility.Visible;
        KernelBootRetry.Content = L("重试");
    }

    /// <summary>
    /// 起一轮内核引导（构造末尾调一次；失败卡重试钮再调一次）。窗口已经亮相、界面已可用，
    /// 这条链路只在后台把内核拉起来，全程不产生任何加载显示。
    /// </summary>
    private void StartKernelBoot()
    {
        HideKernelBootPanel(); // 重试入口：先撤掉上一轮的失败卡，回到正常界面静默等结果
        var cts = new CancellationTokenSource();
        _kernelBootCts = cts;
        // 异常已在 RunKernelBootAsync 内部全数收敛（失败态/取消），这里不需要再兜一层
        _ = RunKernelBootAsync(cts.Token);
    }

    /// <summary>
    /// 内核就绪后补发引导期间被按下的发送、补开引导期间被点过的会话。静默排队（见
    /// SendAsync）：不提示、不动输入框，就绪后按输入框当前内容发一次——期间用户改了字，
    /// 就以最新内容发出；输入框空了则无事发生。点过的会话按最新清单找到就打开。
    /// </summary>
    private void FlushSendQueuedWhileBooting()
    {
        var pendingOpen = Interlocked.Exchange(ref _pendingOpenSessionId, null);
        if (pendingOpen is { } openId)
        {
            // 清单查找与打开都在 UI 线程：_sessions 只在 PostUi 内变更
            PostUi(() =>
            {
                if (_sessions.FirstOrDefault(s => s.SessionId == openId) is { } vm)
                {
                    Volatile.Write(ref _activeSessionId, openId);
                    _ = OpenSessionAsync(vm);
                }
                // 基线里已消失（引导期间被删）：作废，不打扰
            });
        }
        if (!_sendQueuedWhileBooting)
        {
            return;
        }
        _sendQueuedWhileBooting = false;
        PostUi(() => _ = SendAsync());
    }

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
