using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using Blade2.Dsh;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Blade2;

/// <summary>
/// P0-7 子代理目录 + 续跑 + 打断（对齐官方 @deepseek-ai/dsh-client-ui-subagent + dsh-subagent typert）。
/// RPC：subagents/list / subagents/prompt / subagents/interruptByParent。
/// one-shot = 一次性（只读，不可续跑）；continuable = 可续跑（subagents/prompt）。
/// 打断走父会话权威地址（interruptByParent），不依赖父 Agent 在线。
/// </summary>
public sealed partial class MainWindow
{
    // ---------------- RPC 面（typert.remote-client：dsh-subagent） ----------------

    /// request = { requestId, parentSessionId, childSessionId, mode:"continuable", delivery:"queue"|"steer", content:[{type:"text",text}], clientTimeZone? }。
    /// parent 必须在线（parent-unavailable）；one-shot 拒收（not-resumable）。
    /// </summary>
    private async Task SubagentsPromptAsync(string parentSessionId, string childSessionId, string text, string delivery, CancellationToken ct = default)
    {
        if (_rpc is null)
        {
            throw new DshRpcException("network", L("内核未连接"));
        }
        await _rpc.CallOkAsync("subagents/prompt", new
        {
            request = new
            {
                requestId = $"c2-{Guid.NewGuid():N}",
                parentSessionId,
                childSessionId,
                mode = "continuable",
                delivery = delivery is "steer" ? "steer" : "queue",
                content = new object[] { new { type = "text", text } },
            },
        }, ct);
    }

    /// <summary>
    /// subagents/interruptByParent：父会话权威打断子代理。
    /// 参数 (childSessionId, parentSessionId, mode:"continuable")；idle/已完成为接受型 no-op。
    /// </summary>
    private async Task SubagentsInterruptAsync(string childSessionId, string parentSessionId, CancellationToken ct = default)
    {
        if (_rpc is null)
        {
            throw new DshRpcException("network", L("内核未连接"));
        }
        await _rpc.CallOkAsync("subagents/interruptByParent", new
        {
            childSessionId,
            parentSessionId,
            mode = "continuable",
        }, ct);
    }

    // ---------------- 会话头动作钮（RefreshSessionHeader 钩子） ----------------

    /// <summary>
    /// 会话头子代理动作钮刷新：续跑/打断钮（仅 origin=subagent）。
    /// 内核 0.1.7 起 subagents/list 被移除（子代理状态改走 session/follow 订阅流），
    /// 显隐失去了 mode/activity 判据 → 两钮暂以 Collapsed 搁置（诚实死钮）；
    /// prompt/interruptByParent 端点仍在内核，待接入订阅流后按新通道恢复显隐。
    /// </summary>
    private void RefreshSubagentHeaderActions(SessionVm? vm)
    {
        SubagentPromptButton.Visibility = Visibility.Collapsed;
        SubagentInterruptButton.Visibility = Visibility.Collapsed;
    }

    // ---------------- UI 入口：目录 / 续跑 / 打断 ----------------

    private async void OnSubagentPromptClick(object sender, RoutedEventArgs e)
    {
        try
        {
            var vm = _sessions.FirstOrDefault(s => s.SessionId == Volatile.Read(ref _activeSessionId));
            if (vm is not { IsSubagent: true, ParentSessionId: { Length: > 0 } pid })
            {
                return;
            }
            await ShowSubagentPromptAsync(pid, vm.SessionId, vm.Title);
        }
        catch (Exception ex)
        {
            _ = ShowErrorAsync(ex.Message);
        }
    }

    private async void OnSubagentInterruptClick(object sender, RoutedEventArgs e)
    {
        try
        {
            var vm = _sessions.FirstOrDefault(s => s.SessionId == Volatile.Read(ref _activeSessionId));
            if (vm is not { IsSubagent: true, ParentSessionId: { Length: > 0 } pid })
            {
                return;
            }
            await InterruptSubagentAsync(pid, vm.SessionId, vm.Title);
        }
        catch (Exception ex)
        {
            _ = ShowErrorAsync(ex.Message);
        }
    }

    /// <summary>续跑对话框（continuable）：文本框 + 发送。delivery 随 busyEnter 偏好（queue/steer）。</summary>
    private async Task ShowSubagentPromptAsync(string parentSessionId, string childSessionId, string title)
    {
        if (_rpc is null)
        {
            return;
        }
        var box = Aut(new TextBox
        {
            Header = L("续跑消息"),
            PlaceholderText = L("描述你想要构建的内容, / 调用指令, @ 文件或对话"),
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
            MinHeight = 80,
            MinWidth = TokenDouble("DialogMinWidthCompact", 360),
        }, "SubagentPromptTextBox", L("续跑消息"));
        var dialog = new ContentDialog
        {
            Title = LF("续跑：{0}", title),
            Content = box,
            PrimaryButtonText = L("发送消息"),
            CloseButtonText = L("取消"),
            DefaultButton = ContentDialogButton.Primary,
            XamlRoot = Content.XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }
        var text = box.Text.Trim();
        if (text.Length == 0)
        {
            return;
        }
        var delivery = NsString("ui-conversation", "busyEnter", "queue");
        try
        {
            await SubagentsPromptAsync(parentSessionId, childSessionId, text, delivery);
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("续跑失败：{0}", ex.Message));
        }
    }

    /// <summary>打断确认 + interruptByParent（父会话权威，不依赖父 Agent 在线）。</summary>
    private async Task InterruptSubagentAsync(string parentSessionId, string childSessionId, string title)
    {
        if (_rpc is null)
        {
            return;
        }
        var dialog = new ContentDialog
        {
            Title = L("打断子代理"),
            Content = LF("确认打断子代理「{0}」的当前运行？", title),
            PrimaryButtonText = L("打断"),
            CloseButtonText = L("取消"),
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = Content.XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }
        try
        {
            await SubagentsInterruptAsync(childSessionId, parentSessionId);
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("打断失败：{0}", ex.Message));
        }
    }
}
