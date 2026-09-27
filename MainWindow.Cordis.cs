using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;

namespace Blade2;

/// <summary>
/// P0-6：Cordis 动态插件审批卡 + run/stop 面板（对标 @deepseek-ai/dsh-client-ui-cordis）。
/// RPC 走 dynamicCordisRunner/*（typert.remote-client.js 12 方法）终态：
///   已接且有调用方（8）：
///     inventory / runHostHalf / resolveRequestRun / settleUserRun / stopFromPanel /
///     undefineFromPanel / getClientCode（inspect 只读）/ syncInspectManifest（空 Client 目录）。
///   已接 RPC 面、桌面壳无调用方（4，架构限制，见下）：
///     invoke / resolveInspectQuery / reportClientGuardFailure / reportRenderFailure。
/// 事件：cordis/request-run、cordis/request-run-resolved、cordis/dynamic-package、cordis/dynamic-retract。
/// UIA：CordisApprovalCard / CordisPanel / CordisRunStop。
/// 入口：BuildCapabilityMenu「Cordis 插件」。挂接：AttachCordisEvents（boot 唯一上下文补丁）。
///
/// 架构限制（桌面壳 = WinUI，无浏览器 Client 半边运行时；官方 Client 在 opaque iframe 里跑
/// dsh-cordis-client-runner）：
///   1) resolveRequestRun / settleUserRun 的 reason:"client-half-failed" 分支桌面壳不可达——
///      Client 半边从不加载，失败只可能是 rejected / host-half-failed。禁止伪造该分支。
///   2) invoke：Client 半边调用 Host 半边已注册方法（run.handlers）。面板 inspect 对齐官方
///      只读源码（getClientCode），不需要 invoke；桌面壳无 Client 可发起调用方。
///   3) resolveInspectQuery / syncInspectManifest：官方 Client 向 Host 镜像注册
///      Service/Event/Builtin/Slots/Theme 五个 inspect provider，并应答 cordis/inspect-query。
///      桌面壳无 Client 运行时（无 slots/theme service），无法提供等价 provider。
///      syncInspectManifest([]) 显式声明「Client inspect 目录为空」；resolveInspectQuery 仅接受
///      ok:true 结果（Host 对 !ok 直接 accepted:false，不结算），无真实 provider 数据时不可伪造应答。
///   4) reportClientGuardFailure / reportRenderFailure：Client 半边渲染/守卫失败回传 Host。
///      无 Client 半边 = 无失败源，桌面壳不可达，不伪造调用。
/// </summary>
public sealed partial class MainWindow
{
    // ---------------- 文案（中文键 + L()/ShellEnglish；未收录用 en 兜底，勿写入 ShellEnglish） ----------------

    private string CordisText(string zh, string en) => TrajText(zh, en);

    // ---------------- 状态（UI 线程访问） ----------------

    /// <summary>inventory 一行：已定义动态插件 + 可选活动运行。</summary>
    private sealed class CordisPluginRow
    {
        public string PluginId = "";
        public string AgentId = "";
        public string Name = "";
        public string Purpose = "";
        public string PackageId = "";
        public string CurrentPackageId = "";
        public string NextPackageId = "";
        public bool HasClientHalf;
        public bool HasHostHalf;
        public string? ActiveRunId;
        public string? ActivePackageId;
        /// <summary>idle / awaiting-approval / running / failed / stopped / client-pending。</summary>
        public string Status = "idle";
        public string? ApprovalRequestId;
        public string? LatestError;
    }

    /// <summary>一条待审的 cordis/request-run。</summary>
    private sealed class CordisPendingApproval
    {
        public string RequestId = "";
        public string AgentId = "";
        public string PluginId = "";
        public string PackageId = "";
        public string Mode = "run";
        public string Name = "";
        public string Purpose = "";
    }

    private readonly List<CordisPluginRow> _cordisRows = new();
    private readonly Queue<CordisPendingApproval> _cordisApprovalQueue = new();
    private readonly HashSet<string> _cordisSeenRequests = new(StringComparer.Ordinal);
    private CordisPendingApproval? _cordisActiveApproval;
    private bool _cordisSubmitting;
    private bool _cordisCardWired;
    private bool _cordisPanelOpen;

    private Border? _cordisCard;
    private TextBlock? _cordisCardTitle;
    private TextBlock? _cordisCardSummary;
    private TextBlock? _cordisCardDetail;
    private Button? _cordisBtnAllow;
    private Button? _cordisBtnOnce;
    private Button? _cordisBtnFuture;
    private Button? _cordisBtnDecline;

    // ---------------- boot 挂接（MainWindow.xaml.cs 唯一上下文补丁调用） ----------------

    /// <summary>订阅 Cordis 动态插件事件。必须在 SubscribeEventsAsync 前注册。</summary>
    private void AttachCordisEvents()
    {
        if (_rpc is null) return;
        _rpc.OnEvent("cordis/request-run", OnCordisRequestRunAsync);
        _rpc.OnEvent("cordis/request-run-resolved", OnCordisRequestRunResolvedAsync);
        _rpc.OnEvent("cordis/dynamic-package", OnCordisInventoryChangedAsync);
        _rpc.OnEvent("cordis/dynamic-retract", OnCordisInventoryChangedAsync);
        // 声明 Client inspect 目录为空（桌面壳无 Client 运行时）。失败静默：不影响审批/面板。
        _ = SyncCordisInspectManifestAsync(Array.Empty<object>());
    }

    // ---------------- 事件入口（接收线程 → UI） ----------------

    private Task OnCordisRequestRunAsync(JsonElement ev)
    {
        try
        {
            var requestId = CapabilityString(ev, "requestId");
            var requires = ev.TryGetProperty("requiresApproval", out var ra) && ra.ValueKind == JsonValueKind.True;
            var owned = ev.Clone();
            RunEventUi(() =>
            {
                try
                {
                    _ = RefreshCordisInventoryAsync();
                    if (!requires || requestId.Length == 0) return;
                    if (!_cordisSeenRequests.Add(requestId)) return;
                    _cordisApprovalQueue.Enqueue(new CordisPendingApproval
                    {
                        RequestId = requestId,
                        AgentId = CapabilityString(owned, "agentId"),
                        PluginId = CapabilityString(owned, "pluginId"),
                        PackageId = CapabilityString(owned, "packageId"),
                        Mode = CapabilityString(owned, "mode") is { Length: > 0 } m ? m : "run",
                        Name = CapabilityString(owned, "name"),
                        Purpose = CapabilityString(owned, "purpose"),
                    });
                    if (ShouldNotify())
                    {
                        ShellToast.Show(
                            CordisText("Cordis 插件审批", "Cordis plugin approval"),
                            CapabilityString(owned, "name") is { Length: > 0 } n ? n : CapabilityString(owned, "pluginId"));
                    }
                    ShowNextCordisApproval();
                }
                catch (Exception) { }
            });
        }
        catch (Exception) { }
        return Task.CompletedTask;
    }

    private Task OnCordisRequestRunResolvedAsync(JsonElement ev)
    {
        try
        {
            var requestId = CapabilityString(ev, "requestId");
            RunEventUi(() =>
            {
                try
                {
                    if (requestId.Length > 0)
                    {
                        _cordisSeenRequests.Add(requestId);
                        if (_cordisActiveApproval?.RequestId == requestId)
                        {
                            _cordisActiveApproval = null;
                            _cordisSubmitting = false;
                            ShowNextCordisApproval();
                        }
                        // 队列里同 id 的残留直接丢弃
                        var keep = new Queue<CordisPendingApproval>();
                        while (_cordisApprovalQueue.Count > 0)
                        {
                            var item = _cordisApprovalQueue.Dequeue();
                            if (item.RequestId != requestId) keep.Enqueue(item);
                        }
                        while (keep.Count > 0) _cordisApprovalQueue.Enqueue(keep.Dequeue());
                    }
                    _ = RefreshCordisInventoryAsync();
                }
                catch (Exception) { }
            });
        }
        catch (Exception) { }
        return Task.CompletedTask;
    }

    private Task OnCordisInventoryChangedAsync(JsonElement _payload)
    {
        RunEventUi(() => { try { _ = RefreshCordisInventoryAsync(); } catch (Exception) { } });
        return Task.CompletedTask;
    }

    // ---------------- RPC（dynamicCordisRunner/*，CallOkAsync） ----------------

    private async Task RefreshCordisInventoryAsync()
    {
        if (_rpc is null) return;
        try
        {
            var value = await _rpc.CallOkAsync("dynamicCordisRunner/inventory", new { });
            var rows = new List<CordisPluginRow>();
            if (value.ValueKind == JsonValueKind.Array)
            {
                foreach (var item in value.EnumerateArray())
                {
                    rows.Add(ParseCordisRow(item));
                }
            }
            // 待审优先
            rows.Sort((a, b) =>
            {
                int aa = a.Status == "awaiting-approval" ? 0 : 1;
                int bb = b.Status == "awaiting-approval" ? 0 : 1;
                int c = aa.CompareTo(bb);
                return c != 0 ? c : string.CompareOrdinal(a.PluginId, b.PluginId);
            });
            RunEventUi(() =>
            {
                _cordisRows.Clear();
                _cordisRows.AddRange(rows);
                // 与审批队列对账：inventory 里已消失/不再待审的请求收掉
                ReconcileCordisApprovals();
            });
        }
        catch (Exception)
        {
            // 清单读失败：面板内联报错，不打断其它功能
        }
    }

    private static CordisPluginRow ParseCordisRow(JsonElement item)
    {
        var row = new CordisPluginRow
        {
            PluginId = CapabilityString(item, "pluginId"),
            AgentId = CapabilityString(item, "agentId"),
            CurrentPackageId = CapabilityString(item, "currentPackageId"),
            NextPackageId = CapabilityString(item, "nextPackageId"),
        };
        if (item.TryGetProperty("packages", out var pkgs) && pkgs.ValueKind == JsonValueKind.Array)
        {
            foreach (var pkg in pkgs.EnumerateArray())
            {
                var pid = CapabilityString(pkg, "packageId");
                // 默认展示 current → next → 首个
                if (row.PackageId.Length == 0 ||
                    (row.CurrentPackageId.Length > 0 && pid == row.CurrentPackageId) ||
                    (row.CurrentPackageId.Length == 0 && row.NextPackageId.Length > 0 && pid == row.NextPackageId))
                {
                    row.PackageId = pid;
                    row.Name = CapabilityString(pkg, "name");
                    row.Purpose = CapabilityString(pkg, "purpose");
                    row.HasClientHalf = pkg.TryGetProperty("hasClientHalf", out var hc) && hc.ValueKind == JsonValueKind.True;
                    row.HasHostHalf = pkg.TryGetProperty("hasHostHalf", out var hh) && hh.ValueKind == JsonValueKind.True;
                }
                if (row.Name.Length == 0)
                {
                    row.PackageId = pid;
                    row.Name = CapabilityString(pkg, "name");
                    row.Purpose = CapabilityString(pkg, "purpose");
                    row.HasClientHalf = pkg.TryGetProperty("hasClientHalf", out var hc2) && hc2.ValueKind == JsonValueKind.True;
                    row.HasHostHalf = pkg.TryGetProperty("hasHostHalf", out var hh2) && hh2.ValueKind == JsonValueKind.True;
                }
            }
        }
        if (item.TryGetProperty("activeRun", out var ar) && ar.ValueKind == JsonValueKind.Object)
        {
            row.ActiveRunId = CapabilityString(ar, "pluginRunId");
            row.ActivePackageId = CapabilityString(ar, "packageId");
        }
        if (item.TryGetProperty("latestRun", out var lr) && lr.ValueKind == JsonValueKind.Object)
        {
            var status = CapabilityString(lr, "status");
            row.ApprovalRequestId = lr.TryGetProperty("approvalRequestId", out var rid) && rid.ValueKind == JsonValueKind.String
                ? rid.GetString() : null;
            if (lr.TryGetProperty("error", out var err) && err.ValueKind == JsonValueKind.Object)
                row.LatestError = CapabilityString(err, "message");
            row.Status = status switch
            {
                "awaiting-approval" => "awaiting-approval",
                "running" or "waiting" or "starting-host" or "client-pending" =>
                    row.ActiveRunId is not null ? "running" : status,
                "failed" => "failed",
                "rejected" or "cancelled" or "stopped" => "stopped",
                _ => row.ActiveRunId is not null ? "running" : "idle",
            };
        }
        else if (row.ActiveRunId is not null)
        {
            row.Status = "running";
        }
        return row;
    }

    /// <summary>对账：inventory 已不再 awaiting 的请求从队列/当前卡收掉。</summary>
    private void ReconcileCordisApprovals()
    {
        static bool StillPending(List<CordisPluginRow> rows, string requestId) =>
            rows.Any(r => r.ApprovalRequestId == requestId && r.Status == "awaiting-approval");

        if (_cordisActiveApproval is { } active && !StillPending(_cordisRows, active.RequestId))
        {
            _cordisActiveApproval = null;
            _cordisSubmitting = false;
        }
        var keep = new Queue<CordisPendingApproval>();
        while (_cordisApprovalQueue.Count > 0)
        {
            var item = _cordisApprovalQueue.Dequeue();
            if (StillPending(_cordisRows, item.RequestId)) keep.Enqueue(item);
        }
        while (keep.Count > 0) _cordisApprovalQueue.Enqueue(keep.Dequeue());
        if (_cordisActiveApproval is null) ShowNextCordisApproval();
        else UpdateCordisCard();
    }

    /// <summary>runHostHalf：启动 Host 半边（审批放行或面板直接 run）。</summary>
    private async Task<JsonElement> CordisRunHostHalfAsync(
        string agentId, string pluginId, string packageId, string mode, string? requestId, bool approveFutureVersions)
    {
        if (_rpc is null) throw new InvalidOperationException(CordisText("内核未连接。", "Kernel is not connected."));
        return await _rpc.CallOkAsync("dynamicCordisRunner/runHostHalf", new
        {
            agentId,
            pluginId,
            packageId,
            mode,
            requestId,
            approveFutureVersions,
        });
    }

    /// <summary>
    /// resolveRequestRun 四种 outcome（DynamicCordisRunResolution）：
    ///   1) {ok:true, pluginRunId, waitingFor?}           — 允许并完成激活
    ///   2) {ok:false, reason:"rejected"}                 — 用户拒绝
    ///   3) {ok:false, reason:"host-half-failed", …}      — Host 半边失败
    ///   4) {ok:false, reason:"client-half-failed", …}    — Client 半边失败
    /// </summary>
    private async Task CordisResolveRequestRunAsync(string requestId, object resolution)
    {
        if (_rpc is null) return;
        await _rpc.CallOkAsync("dynamicCordisRunner/resolveRequestRun", new { requestId, resolution });
    }

    /// <summary>settleUserRun：面板直接 run（无 requestId）后的结算。</summary>
    private async Task CordisSettleUserRunAsync(string agentId, string pluginId, object resolution)
    {
        if (_rpc is null) return;
        await _rpc.CallOkAsync("dynamicCordisRunner/settleUserRun", new { agentId, pluginId, resolution });
    }

    private async Task<JsonElement> CordisStopFromPanelAsync(string agentId, string pluginId)
    {
        if (_rpc is null) throw new InvalidOperationException(CordisText("内核未连接。", "Kernel is not connected."));
        return await _rpc.CallOkAsync("dynamicCordisRunner/stopFromPanel", new { agentId, pluginId });
    }

    private async Task<JsonElement> CordisUndefineFromPanelAsync(string agentId, string pluginId)
    {
        if (_rpc is null) throw new InvalidOperationException(CordisText("内核未连接。", "Kernel is not connected."));
        return await _rpc.CallOkAsync("dynamicCordisRunner/undefineFromPanel", new { agentId, pluginId });
    }

    /// <summary>inspect 只读：取 Client 源码（无 Client 半边时抛错，调用方内联显示）。</summary>
    private async Task<JsonElement> CordisGetClientCodeAsync(string agentId, string pluginId, string pluginRunId)
    {
        if (_rpc is null) throw new InvalidOperationException(CordisText("内核未连接。", "Kernel is not connected."));
        return await _rpc.CallOkAsync("dynamicCordisRunner/getClientCode", new { agentId, pluginId, pluginRunId });
    }

    // ---------------- RPC：Client 半边专用面（typert 12 方法补接） ----------------
    // 下列五个包装对齐 typert.remote-client.js，保证 wire 面完整。调用方在官方是 dsh-cordis-client-runner
    // （浏览器 iframe）；桌面壳无 Client 半边。仅 syncInspectManifest 有桌面调用（空目录声明），
    // 其余四个无 UI/事件入口——这是架构限制，不是遗漏。禁止伪造 client-half-failed / 假 report 回传。

    /// <summary>
    /// invoke：Client 半边调用 Host 半边已注册方法（run.handlers）。面板 inspect 只用 getClientCode，
    /// 官方亦不从面板 invoke；桌面壳无 Client 可发起。包装仅供完整 wire 面 / 未来调试入口。
    /// </summary>
    private async Task<JsonElement> CordisInvokeAsync(string pluginId, string pluginRunId, string method, object? args)
    {
        if (_rpc is null) throw new InvalidOperationException(CordisText("内核未连接。", "Kernel is not connected."));
        return await _rpc.CallOkAsync("dynamicCordisRunner/invoke", new { pluginId, pluginRunId, method, args });
    }

    /// <summary>
    /// syncInspectManifest：把 Client inspect provider 目录镜像到 Host。桌面壳无 Client 运行时，
    /// 传空数组 = 显式「无 Client inspect 提供方」（与官方 Service/Event/Builtin/Slots/Theme 相对）。
    /// </summary>
    private async Task SyncCordisInspectManifestAsync(object providers)
    {
        if (_rpc is null) return;
        try { await _rpc.CallOkAsync("dynamicCordisRunner/syncInspectManifest", new { providers }); }
        catch (Exception) { /* 连接未就绪 / 重复 sync：忽略 */ }
    }

    /// <summary>
    /// resolveInspectQuery：应答 Host 的 Client-platform inspect 查询。Host 仅接受 ok:true
    /// （!ok 直接 accepted:false 不结算）；桌面壳无真实 Client provider 数据，不可伪造应答。
    /// 未注册 provider 时 Host 在 queryClient 直接抛 "not registered"，不会挂起 pending。
    /// </summary>
    private async Task<JsonElement> CordisResolveInspectQueryAsync(string agentId, string requestId, object resolution)
    {
        if (_rpc is null) throw new InvalidOperationException(CordisText("内核未连接。", "Kernel is not connected."));
        return await _rpc.CallOkAsync("dynamicCordisRunner/resolveInspectQuery", new { agentId, requestId, resolution });
    }

    /// <summary>reportClientGuardFailure：Client 守卫失败回传。无 Client 半边 = 无失败源，桌面壳不可达。</summary>
    private async Task CordisReportClientGuardFailureAsync(string agentId, string pluginId, string pluginRunId, object failure)
    {
        if (_rpc is null) throw new InvalidOperationException(CordisText("内核未连接。", "Kernel is not connected."));
        await _rpc.CallOkAsync("dynamicCordisRunner/reportClientGuardFailure", new { agentId, pluginId, pluginRunId, failure });
    }

    /// <summary>reportRenderFailure：Client 渲染失败回传。无 Client 半边 = 无失败源，桌面壳不可达。</summary>
    private async Task CordisReportRenderFailureAsync(string agentId, string pluginId, string pluginRunId, object failure)
    {
        if (_rpc is null) throw new InvalidOperationException(CordisText("内核未连接。", "Kernel is not connected."));
        await _rpc.CallOkAsync("dynamicCordisRunner/reportRenderFailure", new { agentId, pluginId, pluginRunId, failure });
    }

    // ---------------- 审批卡（浮卡，仿 ApprovalHost） ----------------

    private void EnsureCordisApprovalCard()
    {
        if (_cordisCardWired && _cordisCard is not null) return;
        try
        {
            var titleRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp8 };
            titleRow.Children.Add(new Ellipse { Width = 8, Height = 8, Fill = (Brush)Application.Current.Resources["InfoBrush"] });
            _cordisCardTitle = new TextBlock
            {
                Text = CordisText("动态插件运行审批", "Cordis plugin run approval"),
                Style = AppStyle("BodyStrongTextStyle"),
            };
            titleRow.Children.Add(_cordisCardTitle);

            _cordisCardSummary = new TextBlock { TextWrapping = TextWrapping.Wrap };
            _cordisCardDetail = new TextBlock
            {
                TextWrapping = TextWrapping.Wrap,
                Opacity = 0.7,
                Style = AppStyle("CaptionTextStyle"),
                Visibility = Visibility.Collapsed,
            };

            // 官方文案（dsh-client-ui-cordis action.*）：允许 / 仅允许此版本 / 允许此插件的后续版本 / 拒绝
            _cordisBtnAllow = Aut(new Button { Content = CordisText("允许", "Allow") }, "CordisApproveAllow", CordisText("允许", "Allow"));
            _cordisBtnOnce = Aut(new Button { Content = CordisText("仅允许此版本", "Allow this version only") }, "CordisApproveOnce", CordisText("仅允许此版本", "Allow this version only"));
            _cordisBtnFuture = Aut(new Button { Content = CordisText("允许此插件的后续版本", "Allow future versions of this plugin") }, "CordisApproveFuture", CordisText("允许此插件的后续版本", "Allow future versions of this plugin"));
            _cordisBtnDecline = Aut(new Button { Content = CordisText("拒绝", "Decline") }, "CordisApproveDecline", CordisText("拒绝", "Decline"));

            _cordisBtnAllow.Click += async (_, _) => await ResolveCordisApprovalAsync("allow");
            _cordisBtnOnce.Click += async (_, _) => await ResolveCordisApprovalAsync("once");
            _cordisBtnFuture.Click += async (_, _) => await ResolveCordisApprovalAsync("future");
            _cordisBtnDecline.Click += async (_, _) => await ResolveCordisApprovalAsync("decline");

            var buttons = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = Sp8,
                HorizontalAlignment = HorizontalAlignment.Right,
            };
            buttons.Children.Add(_cordisBtnDecline);
            buttons.Children.Add(_cordisBtnAllow);
            buttons.Children.Add(_cordisBtnOnce);
            buttons.Children.Add(_cordisBtnFuture);

            var inner = new StackPanel { Spacing = Sp8 };
            inner.Children.Add(titleRow);
            inner.Children.Add(_cordisCardSummary);
            inner.Children.Add(_cordisCardDetail);
            inner.Children.Add(buttons);

            _cordisCard = Aut(new Border
            {
                Background = (Brush)Application.Current.Resources["SurfaceBrush"],
                BorderBrush = (Brush)Application.Current.Resources["InfoBrush"],
                BorderThickness = new Thickness(1),
                CornerRadius = (CornerRadius)Application.Current.Resources["CornerMedium"],
                Padding = (Thickness)Application.Current.Resources["CardPaddingLarge"],
                Margin = new Thickness(Sp12),
                MinWidth = 320,
                MaxWidth = 520,
                HorizontalAlignment = HorizontalAlignment.Left,
                Visibility = Visibility.Collapsed,
                Child = inner,
            }, "CordisApprovalCard", CordisText("动态插件运行审批", "Cordis plugin run approval"));

            // 挂到 ApprovalHost 同级（聊天区底部居中浮卡）
            if (ApprovalHost.Parent is Grid grid)
            {
                Grid.SetRow(_cordisCard, Grid.GetRow(ApprovalHost));
                Grid.SetColumn(_cordisCard, Grid.GetColumn(ApprovalHost));
                _cordisCard.VerticalAlignment = VerticalAlignment.Bottom;
                _cordisCard.HorizontalAlignment = HorizontalAlignment.Center;
                grid.Children.Add(_cordisCard);
            }
            _cordisCardWired = true;
        }
        catch (Exception) { }
    }

    private void ShowNextCordisApproval()
    {
        if (_cordisSubmitting || _cordisActiveApproval is not null) return;
        EnsureCordisApprovalCard();
        if (_cordisCard is null) return;
        if (_cordisApprovalQueue.Count == 0)
        {
            _cordisCard.Visibility = Visibility.Collapsed;
            return;
        }
        _cordisActiveApproval = _cordisApprovalQueue.Dequeue();
        UpdateCordisCard();
        _cordisCard.Visibility = Visibility.Visible;
    }

    private void UpdateCordisCard()
    {
        if (_cordisCard is null || _cordisActiveApproval is not { } approval) return;
        if (_cordisCardSummary is not null)
        {
            var name = approval.Name.Length > 0 ? approval.Name : approval.PluginId;
            _cordisCardSummary.Text = $"{name} · {approval.PackageId}";
        }
        if (_cordisCardDetail is not null)
        {
            var purpose = approval.Purpose.Length > 0
                ? approval.Purpose
                : CordisText("(未填写用途)", "(no purpose given)");
            _cordisCardDetail.Text = $"{approval.PluginId} · {purpose}";
            _cordisCardDetail.Visibility = Visibility.Visible;
        }
        var enabled = !_cordisSubmitting;
        if (_cordisBtnAllow is not null) _cordisBtnAllow.IsEnabled = enabled;
        if (_cordisBtnOnce is not null) _cordisBtnOnce.IsEnabled = enabled;
        if (_cordisBtnFuture is not null) _cordisBtnFuture.IsEnabled = enabled;
        if (_cordisBtnDecline is not null) _cordisBtnDecline.IsEnabled = enabled;
    }

    /// <summary>
    /// 审批四种用户 outcome → resolveRequestRun 四种 resolution：
    ///   allow / once  → runHostHalf(approveFutureVersions=false) → resolve {ok:true}（失败则 host-half-failed）
    ///   future        → runHostHalf(approveFutureVersions=true)  → resolve {ok:true}（失败则 host-half-failed）
    ///   decline       → resolve {ok:false, reason:"rejected"}
    /// Client 半边无法在桌面壳加载时，以 {ok:true} 近似完成；若 Host 已起则接受。
    /// 架构限制：reason:"client-half-failed" 分支桌面壳不可达（无浏览器 Client 运行时），禁止伪造。
    /// </summary>
    private async Task ResolveCordisApprovalAsync(string kind)
    {
        var approval = _cordisActiveApproval;
        if (approval is null || _cordisSubmitting || _rpc is null) return;
        _cordisSubmitting = true;
        UpdateCordisCard();
        try
        {
            if (kind == "decline")
            {
                await CordisResolveRequestRunAsync(approval.RequestId, new { ok = false, reason = "rejected" });
            }
            else
            {
                bool future = kind == "future";
                JsonElement started;
                try
                {
                    started = await CordisRunHostHalfAsync(
                        approval.AgentId, approval.PluginId, approval.PackageId,
                        approval.Mode, approval.RequestId, future);
                }
                catch (Exception ex)
                {
                    await TryCordisResolveAsync(approval.RequestId, new
                    {
                        ok = false,
                        reason = "host-half-failed",
                        message = ex.Message,
                    });
                    throw;
                }

                bool ok = started.ValueKind == JsonValueKind.Object &&
                          started.TryGetProperty("ok", out var okEl) && okEl.ValueKind == JsonValueKind.True;
                if (!ok)
                {
                    var message = started.ValueKind == JsonValueKind.Object
                        ? CapabilityString(started, "message")
                        : CordisText("Host 启动失败", "Host half failed");
                    await TryCordisResolveAsync(approval.RequestId, new
                    {
                        ok = false,
                        reason = "host-half-failed",
                        message = message.Length > 0 ? message : "host-half-failed",
                    });
                }
                else
                {
                    var pluginRunId = CapabilityString(started, "pluginRunId");
                    // 桌面壳无浏览器 Client 运行时：以 ok:true 结算（Host 半边已启动）。
                    await TryCordisResolveAsync(approval.RequestId, new
                    {
                        ok = true,
                        pluginRunId,
                        waitingFor = Array.Empty<string>(),
                    });
                }
            }
        }
        catch (Exception ex)
        {
            try { await ShowErrorAsync(CordisText("审批回传失败：", "Approval failed: ") + ex.Message); }
            catch (Exception) { }
        }
        finally
        {
            _cordisSubmitting = false;
            if (_cordisActiveApproval?.RequestId == approval.RequestId)
            {
                _cordisActiveApproval = null;
            }
            _ = RefreshCordisInventoryAsync();
            ShowNextCordisApproval();
        }
    }

    private async Task TryCordisResolveAsync(string requestId, object resolution)
    {
        try { await CordisResolveRequestRunAsync(requestId, resolution); }
        catch (Exception) { /* 已结算/超时：忽略 */ }
    }

    // ---------------- 面板（ContentDialog，UIA CordisPanel） ----------------

    /// <summary>能力菜单入口：已加载/待审动态插件列表 + run/stop。</summary>
    private async Task ShowCordisPanelAsync()
    {
        if (_cordisPanelOpen) return;
        _cordisPanelOpen = true;
        try
        {
            await RefreshCordisInventoryAsync();
            var root = Aut(new StackPanel { Spacing = Sp10, Width = 520 }, "CordisPanel", CordisText("Cordis 插件面板", "Cordis plugins panel"));
            var note = new TextBlock
            {
                TextWrapping = TextWrapping.Wrap,
                Text = CordisText("读取中…", "Reading…"),
            };
            var listHost = new StackPanel { Spacing = Sp8 };
            root.Children.Add(note);
            root.Children.Add(new ScrollViewer
            {
                Content = listHost,
                MaxHeight = 480,
                VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            });

            var dialog = CapabilityDialog(CordisText("Cordis 插件", "Cordis plugins"), root);

            void Render()
            {
                listHost.Children.Clear();
                if (_cordisRows.Count == 0)
                {
                    note.Text = CordisText("还没有定义任何插件", "No plugins defined yet");
                    return;
                }
                note.Text = string.Format(CordisText("共 {0} 个插件", "{0} plugins"), _cordisRows.Count);
                foreach (var row in _cordisRows)
                {
                    listHost.Children.Add(BuildCordisRowCard(row, Render, dialog));
                }
            }

            Render();
            await dialog.ShowAsync();
        }
        finally
        {
            _cordisPanelOpen = false;
        }
    }

    private FrameworkElement BuildCordisRowCard(CordisPluginRow row, Action refresh, ContentDialog dialog)
    {
        var statusLabel = row.Status switch
        {
            "awaiting-approval" => CordisText("待审批", "Awaiting approval"),
            "running" => CordisText("运行中", "Running"),
            "failed" => CordisText("运行失败", "Run failed"),
            "stopped" => CordisText("已停止", "Stopped"),
            "client-pending" => CordisText("Client 待激活", "Client ready to activate"),
            _ => CordisText("待激活", "Ready"),
        };

        var head = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp8 };
        var nameText = row.Name.Length > 0 ? row.Name : row.PluginId;
        head.Children.Add(new TextBlock
        {
            Text = nameText,
            Style = AppStyle("BodyStrongTextStyle"),
            TextWrapping = TextWrapping.Wrap,
        });
        head.Children.Add(new TextBlock
        {
            Text = row.PackageId,
            Opacity = 0.7,
            Style = AppStyle("CaptionTextStyle"),
            VerticalAlignment = VerticalAlignment.Center,
        });
        head.Children.Add(new TextBlock
        {
            Text = statusLabel,
            Style = AppStyle("CaptionTextStyle"),
            VerticalAlignment = VerticalAlignment.Center,
        });

        var purpose = new TextBlock
        {
            Text = row.Purpose.Length > 0 ? row.Purpose : CordisText("(未填写用途)", "(no purpose given)"),
            TextWrapping = TextWrapping.Wrap,
            Opacity = 0.8,
            Style = AppStyle("CaptionTextStyle"),
        };
        var source = new TextBlock
        {
            Text = $"{CordisText("来源", "Source")}: {row.PluginId} · {row.AgentId}",
            TextWrapping = TextWrapping.Wrap,
            Opacity = 0.7,
            Style = AppStyle("CaptionTextStyle"),
        };

        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6 };

        // run/stop 开关（UIA CordisRunStop）
        bool running = row.Status is "running" or "client-pending";
        bool awaiting = row.Status == "awaiting-approval";
        var runStop = Aut(new Button
        {
            Content = running ? CordisText("停止", "Stop") : CordisText("运行", "Run"),
            Style = AppStyle("CompactButtonStyle"),
            IsEnabled = !awaiting,
        }, "CordisRunStop", running ? CordisText("停止", "Stop") : CordisText("运行", "Run"));
        runStop.Click += async (_, _) =>
        {
            try
            {
                runStop.IsEnabled = false;
                if (running)
                {
                    await CordisStopFromPanelAsync(row.AgentId, row.PluginId);
                }
                else
                {
                    var mode = row.CurrentPackageId.Length > 0 && row.PackageId != row.CurrentPackageId ? "update" : "run";
                    var started = await CordisRunHostHalfAsync(row.AgentId, row.PluginId, row.PackageId, mode, null, false);
                    bool ok = started.ValueKind == JsonValueKind.Object &&
                              started.TryGetProperty("ok", out var okEl) && okEl.ValueKind == JsonValueKind.True;
                    if (!ok)
                    {
                        var msg = CapabilityString(started, "message");
                        throw new InvalidOperationException(msg.Length > 0 ? msg : CordisText("运行失败", "Run failed"));
                    }
                    // 有 Client 半边时补 settleUserRun（桌面壳无浏览器加载，以 ok:true 结算）
                    if (row.HasClientHalf)
                    {
                        var pluginRunId = CapabilityString(started, "pluginRunId");
                        await CordisSettleUserRunAsync(row.AgentId, row.PluginId, new
                        {
                            ok = true,
                            pluginRunId,
                            waitingFor = Array.Empty<string>(),
                        });
                    }
                }
                await RefreshCordisInventoryAsync();
                refresh();
            }
            catch (Exception ex)
            {
                try { await ShowErrorAsync(ex.Message); } catch (Exception) { }
                runStop.IsEnabled = true;
            }
        };
        actions.Children.Add(runStop);

        // 待审：四 outcome 按钮
        if (awaiting && row.ApprovalRequestId is { Length: > 0 } rid)
        {
            void AddApprove(string zh, string en, string kind)
            {
                var btn = Aut(new Button { Content = CordisText(zh, en), Style = AppStyle("CompactButtonStyle") },
                    "CordisApprovalCard", CordisText(zh, en));
                btn.Click += async (_, _) =>
                {
                    try
                    {
                        btn.IsEnabled = false;
                        if (kind == "decline")
                        {
                            await CordisResolveRequestRunAsync(rid, new { ok = false, reason = "rejected" });
                        }
                        else
                        {
                            bool future = kind == "future";
                            var started = await CordisRunHostHalfAsync(
                                row.AgentId, row.PluginId, row.PackageId,
                                row.CurrentPackageId.Length > 0 && row.PackageId != row.CurrentPackageId ? "update" : "run",
                                rid, future);
                            bool ok = started.ValueKind == JsonValueKind.Object &&
                                      started.TryGetProperty("ok", out var okEl) && okEl.ValueKind == JsonValueKind.True;
                            if (ok)
                            {
                                await CordisResolveRequestRunAsync(rid, new
                                {
                                    ok = true,
                                    pluginRunId = CapabilityString(started, "pluginRunId"),
                                    waitingFor = Array.Empty<string>(),
                                });
                            }
                            else
                            {
                                await CordisResolveRequestRunAsync(rid, new
                                {
                                    ok = false,
                                    reason = "host-half-failed",
                                    message = CapabilityString(started, "message"),
                                });
                            }
                        }
                        await RefreshCordisInventoryAsync();
                        refresh();
                    }
                    catch (Exception ex)
                    {
                        try { await ShowErrorAsync(ex.Message); } catch (Exception) { }
                    }
                };
                actions.Children.Add(btn);
            }
            // 官方文案（dsh-client-ui-cordis action.*）
            AddApprove("允许", "Allow", "allow");
            AddApprove("仅允许此版本", "Allow this version only", "once");
            AddApprove("允许此插件的后续版本", "Allow future versions of this plugin", "future");
            AddApprove("拒绝", "Decline", "decline");
        }
        else if (!awaiting)
        {
            var remove = Aut(new Button
            {
                Content = CordisText("移除", "Remove"),
                Style = AppStyle("CompactButtonStyle"),
            }, "CordisRemove", CordisText("移除", "Remove"));
            remove.Click += async (_, _) =>
            {
                try
                {
                    remove.IsEnabled = false;
                    await CordisUndefineFromPanelAsync(row.AgentId, row.PluginId);
                    await RefreshCordisInventoryAsync();
                    refresh();
                }
                catch (Exception ex)
                {
                    try { await ShowErrorAsync(ex.Message); } catch (Exception) { }
                    remove.IsEnabled = true;
                }
            };
            actions.Children.Add(remove);
        }

        // inspect 只读（getClientCode）
        var inspectHost = new StackPanel { Spacing = Sp4, Visibility = Visibility.Collapsed };
        var inspectBtn = Aut(new Button
        {
            Content = CordisText("查看", "Inspect"),
            Style = AppStyle("CompactButtonStyle"),
            IsEnabled = row.ActiveRunId is { Length: > 0 },
        }, "CordisInspect", CordisText("查看", "Inspect"));
        inspectBtn.Click += async (_, _) =>
        {
            try
            {
                if (inspectHost.Visibility == Visibility.Visible)
                {
                    inspectHost.Visibility = Visibility.Collapsed;
                    return;
                }
                inspectHost.Children.Clear();
                if (row.ActiveRunId is not { Length: > 0 } runId)
                {
                    inspectHost.Children.Add(new TextBlock
                    {
                        Text = CordisText("当前没有活动运行。", "No active run."),
                        TextWrapping = TextWrapping.Wrap,
                    });
                }
                else
                {
                    var code = await CordisGetClientCodeAsync(row.AgentId, row.PluginId, runId);
                    var src = CapabilityString(code, "code");
                    inspectHost.Children.Add(new TextBlock
                    {
                        Text = CordisText("Client 源码（只读）", "Client source (read-only)"),
                        Style = AppStyle("CaptionTextStyle"),
                    });
                    inspectHost.Children.Add(new ScrollViewer
                    {
                        Content = new TextBlock
                        {
                            Text = src.Length > 0 ? src : CordisText("(空)", "(empty)"),
                            TextWrapping = TextWrapping.Wrap,
                            IsTextSelectionEnabled = true,
                            FontFamily = new FontFamily("Consolas"),
                            FontSize = 11,
                        },
                        MaxHeight = 200,
                        VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
                    });
                }
                inspectHost.Visibility = Visibility.Visible;
            }
            catch (Exception ex)
            {
                inspectHost.Children.Clear();
                inspectHost.Children.Add(new TextBlock
                {
                    Text = ex.Message,
                    TextWrapping = TextWrapping.Wrap,
                });
                inspectHost.Visibility = Visibility.Visible;
            }
        };
        actions.Children.Add(inspectBtn);

        var body = new StackPanel { Spacing = Sp4 };
        body.Children.Add(head);
        body.Children.Add(purpose);
        body.Children.Add(source);
        if (row.LatestError is { Length: > 0 } err)
        {
            body.Children.Add(new TextBlock
            {
                Text = err,
                TextWrapping = TextWrapping.Wrap,
                Style = AppStyle("CaptionTextStyle"),
            });
        }
        body.Children.Add(actions);
        body.Children.Add(inspectHost);

        return Aut(new Border
        {
            Padding = new Thickness(Sp10),
            CornerRadius = (CornerRadius)Application.Current.Resources["CornerMedium"],
            BorderThickness = new Thickness(1),
            BorderBrush = (Brush)Application.Current.Resources["InfoBrush"],
            Background = (Brush)Application.Current.Resources["SurfaceBrush"],
            Child = body,
        }, "CordisPluginRow_" + row.PluginId, $"{nameText} {row.PackageId} {statusLabel}");
    }
}
