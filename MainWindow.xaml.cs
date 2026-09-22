using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Net.Http;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.UI;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Windows.Graphics;
using Blade2.Dsh;

namespace Blade2;

/// <summary>设置分区条目（对应内核 settings.section 注册表：general/models/plugins/agent-presets）。</summary>
public sealed class SettingsSectionVm
{
    public string Id { get; init; } = "";
    public string Title { get; init; } = "";
    public string Glyph { get; init; } = "\uE713";
}

/// <summary>会话列表条目。</summary>
public sealed class SessionVm
{
    public string SessionId { get; set; } = "";
    public string Title { get; set; } = "";
    public string Subtitle { get; set; } = "";
    public string Cwd { get; set; } = "";
    /// <summary>内核 session/list 的 updatedAt（epoch 毫秒）：侧栏相对时间的唯一数据源。</summary>
    public long UpdatedAt { get; set; }
    /// <summary>是否为空会话（内核 blank 位；侧栏相对时间与新会话命名共用）。</summary>
    public bool Blank { get; set; }
    /// <summary>内核 session/list 的 origin："subagent" = 子代理会话（其余为空 = 普通会话）。</summary>
    public string? Origin { get; set; }
    /// <summary>父会话 id（session/list 的 parentSessionId；子代理会话非空，普通会话为空）。</summary>
    public string? ParentSessionId { get; set; }

    /// <summary>子代理会话（内核 origin 位）。侧栏标记、只读 composer、会话头回跳都按它判。</summary>
    public bool IsSubagent => Origin == "subagent";
}

/// <summary>工作区（会话的文件夹分类）。</summary>
public sealed class WorkspaceVm
{
    public string WorkspaceId { get; set; } = "";
    public string Title { get; set; } = "";
    public string Path { get; set; } = "";
    public List<string> SessionIds { get; set; } = new();
}

/// <summary>待发送附件：图片（base64 内联）或文件（上传换 receiptId）。</summary>
public sealed class AttachmentVm
{
    public string Name { get; init; } = "";
    public bool IsImage { get; init; }
    public string MediaType { get; init; } = "";
    public string DataBase64 { get; init; } = "";
    public string FilePath { get; init; } = "";
}

/// <summary>聊天气泡：user / user-image / assistant / reasoning / tool / tool-call / deliverable。</summary>
public sealed class ChatBubble
{
    public string Role { get; init; } = "user";   // user | user-image | assistant | reasoning | tool | tool-call | system | deliverable
    public string Text { get; set; } = "";
    public bool IsToolCall { get; init; }
    public int Turn { get; set; }
    public bool IsReasoning { get; init; }
    public bool IsAssistantStep { get; init; }

    /// <summary>该气泡仍在吃逐字增量（session/follow 的 assistant-stream 帧）。
    /// 定稿的持久 assistant/message 到达时按此标志回填同一条气泡而不是再追加一条。</summary>
    public bool IsLiveStreaming { get; set; }

    /// <summary>工具执行窗口（tool/call 已下发、tool/result 未回）：执行中行头起流光扫光
    /// （对标 reasoning 行的 streaming 扫光），result 到达或轮次闭合即停。</summary>
    public bool IsToolRunning { get; set; }

    /// <summary>「思考」行展开态：逐字重绘会整行重建，展开状态挂在气泡上才不会每帧被折叠回去。</summary>
    public bool ReasoningExpanded { get; set; }

    /// <summary>该气泡所属的提问已被「撤回编辑」撤回：仍留在 _messages 里（本地回显认领路径
    /// 要靠它把 journal 记录的 seq 补记进抑制表），但不进可见投影（见 RefreshTranscriptView）。</summary>
    public bool Withdrawn { get; set; }

    /// <summary>来源 journal 事件信封的 time（epoch 毫秒）：消息时间戳（formatMessageClock 口径）。
    /// 0 = 事件未带时间（旧日志），不显示时间戳。</summary>
    public long Time { get; set; }

    /// <summary>来源 journal 事件的 seq：「在新对话中分支」的 atSeq 锚点。
    /// 0 = 不可分支。内核按「atSeq 之后第一个 turn/end」定截断边界，因此
    /// 锚在本轮任一事件上都落在同一轮（见 dsh-client-connection 的 session/fork）。</summary>
    public int Seq { get; set; }

    /// <summary>本轮用时（turn/start→turn/end 信封时差，毫秒）。turn/end 到达时回填到该轮
    /// 答案气泡（ChatBubble 可变项仅此一处），随操作行重绘显示。</summary>
    public long DurationMs { get; set; }

    /// <summary>该消息由哪个模型产出（内核 request/header 的 config.model，只记型号 id）。
    /// 用户消息记的是回答它的那次请求所用模型；空 = 日志里没有 header 记录，不显示。</summary>
    public string Model { get; set; } = "";

    /// <summary>tool/call 事件的 arguments 原文（JSON 串）：工具行摘要的取材来源。</summary>
    public string? ToolArgs { get; init; }

    /// <summary>tool/call 事件的工具名（wire 原名，如 grep/read）：工具行按它解析
    /// 图标与标题（行重建发生在本地化之后，不能靠 Text 里的译名反查）。</summary>
    public string? ToolName { get; init; }

    /// <summary>user-image 气泡的位图：来自内核 session/attachment 的历史图片字节。</summary>
    public Microsoft.UI.Xaml.Media.ImageSource? Image { get; init; }

    /// <summary>内核 assistant/message 事件的 message.id —— messageFeedback 的目标标识
    /// （内核按 assistant/message 事件的 message.id 校验，不匹配即 target-not-found）。
    /// 非 assistant/message 来源的气泡（错误占位、工具行）没有 id，因此不给反馈入口。</summary>
    public string? MessageId { get; set; }

    /// <summary>该消息的 token 用量（assistant/message 事件的 usage：totalTokens，缺失时按
    /// input+output+cacheRead+cacheWrite 口径）。0 = 无用量记录，操作行不显示「用量」。</summary>
    public long Tokens { get; set; }

    /// <summary>deliverable 气泡交付的文件（deliverables/presented 事件的 files）。</summary>
    public List<PresentedFileVm> Files { get; init; } = new();
}

/// <summary>一条交付物声明（deliverables/presented 事件的 files[i]）。Index 是它在 files
/// 数组里的原始下标——内核按 (sessionId, seq, index) 三元组回查会话日志定位该文件
/// （dsh-client-ui-deliverables 的 handlePresentOpen），所以必须原样保留。</summary>
public sealed class PresentedFileVm
{
    public int Index { get; init; }
    public string Path { get; init; } = "";
    public string Description { get; init; } = "";
    public string SessionId { get; init; } = "";
    public int Seq { get; init; }

    public string Name => Path.Replace('\\', '/').Split('/')[^1];
    public Visibility DescriptionVisibility => Description.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
}

/// <summary>一轮里一次成功的文件突变：「撤回修改」的取证与反演依据。
/// write/edit 有内核 presentationMeta 的 diffs（before===null 即新建文件，diffs 为空数组），
/// 每个 hunk 带 3 行上下文，按「找 New 换 Old」整块反演；str_replace_editor 没有 meta，
/// 只能按调用参数反推（EditorOld/EditorNew）。</summary>
public sealed class TurnFileMutation
{
    /// <summary>绝对路径（取结果文本里的 backend 解析路径，args 里可能是工作区相对路径）。</summary>
    public string Path = "";
    /// <summary>该文件由这次调用新建：撤回 = 删除文件（write 的 before===null / editor 的 create）。</summary>
    public bool Created;
    /// <summary>write/edit 的改动块（文件序）。</summary>
    public List<TurnHunk> Hunks = new();
    /// <summary>str_replace_editor 的 old_str（str_replace 命令）。</summary>
    public string? EditorOld;
    /// <summary>str_replace_editor 的 new_str（str_replace / insert 命令）。</summary>
    public string? EditorNew;
    /// <summary>str_replace_editor 的 command（create / str_replace / insert）。</summary>
    public string EditorCommand = "";
    /// <summary>是否已成功回退过（重试时跳过，避免二次反演）。</summary>
    public bool Done;

    /// <summary>有没有可用的回退依据：新建可删、有 hunk 可反演、editor 的 new_str 非空可回填。
    /// 内核没给 meta 的 edit、或写的是大文件/二进制（diff 依据取不到）时为 false——
    /// 界面上照常展示，但撤回时会明确报告「无法恢复」而不是假装成功。</summary>
    public bool Revertible =>
        Created || Hunks.Count > 0 ||
        (EditorCommand == "str_replace" && EditorNew is { Length: > 0 }) ||
        (EditorCommand == "insert" && EditorNew is not null);
}

/// <summary>一个改动块（含上下文行）。Old 为空 = 纯新增（meta.oldText 为 null），
/// New 为空 = 纯删除。行已按 LF 切分，匹配时先归一化行尾。</summary>
public sealed class TurnHunk
{
    public string[] Old = Array.Empty<string>();
    public string[] New = Array.Empty<string>();
}

/// <summary>@ 引用候选（fileReferences/list 的一行，或 sessionReferenceResolver/candidates 的一行）。
/// Insert 是选中后整段插入输入内容的文本：文件 = 相对路径；会话 = 内核给好的
/// @[标签](dsh-session:&lt;base64&gt;) mention 串（不自行拼装，避免与内核解析器不一致）。</summary>
public sealed class ReferenceVm
{
    public string Glyph { get; init; } = "";
    public string Label { get; init; } = "";
    public string Detail { get; init; } = "";
    public string Insert { get; init; } = "";
    public bool IsSession { get; init; }

    /// <summary>UIA/列表用：内核 mention 串对会话更可读，文件用路径。</summary>
    public override string ToString() => Label;
}

/// <summary>顶条搜索的一条建议项：内核 session/search 命中（会话 id + 片段）。
/// 内核未开启全文检索时回落为本地标题匹配（Snippet 说明来源）。
/// IsNotice = 无结果提示行（不可选中的纯文字，对齐 Win11 设置搜索的空态样式）。</summary>
public sealed class SearchHitVm
{
    public string SessionId { get; init; } = "";
    public string Title { get; init; } = "";
    public string Snippet { get; init; } = "";

    /// <summary>是否来自内核检索（false = 本地标题回退）。</summary>
    public bool FromKernel { get; init; }

    /// <summary>无结果提示行：只渲染 Title，不参与选中跳转。</summary>
    public bool IsNotice { get; init; }

    /// <summary>结果行/提示行的显隐互斥（单一 ItemTemplate 同时承载两种行）。</summary>
    public Visibility HitVisibility => IsNotice ? Visibility.Collapsed : Visibility.Visible;
    public Visibility NoticeVisibility => IsNotice ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>AutoSuggestBox 的 TextMemberPath 与 UIA 名都读它。</summary>
    public override string ToString() => Title;
}

/// <summary>排队中的一条消息（session/control 的 baseline/queue 帧）。
/// Content 保留内核原始 content 块数组：编辑只换文本块，图片/文件块原样回写。</summary>
public sealed class QueueItemVm
{
    public string SessionId { get; init; } = "";
    public string ItemId { get; init; } = "";
    public string Placement { get; init; } = "queued";   // queued | steering | context
    public string Text { get; init; } = "";
    public JsonElement Content { get; init; }

    public string PlacementLabel => Placement switch
    {
        "steering" => "插话",
        "context" => "上下文",
        _ => "排队",
    };
}

/// <summary>内核任务（session/control 的 jobs 帧）：运行中的命令 / 子代理等。
/// 字段与内核 jobView（dsh-api-session-controller）逐一对齐：kind 是作业类型标识
/// （bash-1 / subagent-2 这类，由内核注册表按 &lt;kind&gt;-N 生成），startedAt/finishedAt
/// 是 epoch 毫秒——时长只由这两个字段算出，内核没给就不显示，不估算。</summary>
public sealed class JobVm
{
    public string JobId { get; init; } = "";
    public string Kind { get; init; } = "";
    public string Label { get; init; } = "";
    public string Status { get; init; } = "";
    public string? Detail { get; init; }
    public long StartedAt { get; init; }
    public long FinishedAt { get; init; }

    /// <summary>内核口径的"存活"作业：运行中/停止中。官方 JobListAction 的 isLive 同判据。</summary>
    public bool IsLive => Status is "running" or "stopping";

    public string StatusLabel => Status switch
    {
        "running" => "运行中",
        "stopping" => "停止中",
        "completed" => "已完成",
        "killed" => "已终止",
        "failed" => "失败",
        _ => Status,
    };

    /// <summary>官方口径时长（至多两个相邻单位，小时为最宽单位）：live = now − startedAt，
    /// 已结束 = finishedAt − startedAt。缺 startedAt（或已结束但缺 finishedAt）返回 null。</summary>
    public string? DurationText(long nowEpochMs)
    {
        var end = IsLive ? nowEpochMs : (FinishedAt > 0 ? FinishedAt : 0);
        if (StartedAt <= 0 || end <= 0 || end < StartedAt)
        {
            return null;
        }
        var total = (end - StartedAt) / 1000;
        if (total < 0)
        {
            return null;
        }
        var seconds = total % 60;
        var minutes = total / 60 % 60;
        var hours = total / 3600;
        if (hours > 0)
        {
            return MainWindow.TLF("{0}小时{1}分", hours, minutes);
        }
        if (minutes > 0)
        {
            return MainWindow.TLF("{0}分{1}秒", minutes, seconds);
        }
        return MainWindow.TLF("{0}秒", seconds);
    }
}

/// <summary>斜杠命令条目（commands/list 的一行）。</summary>
public sealed class CommandVm
{
    public string Name { get; init; } = "";
    /// <summary>commands/list 的 description（内核按 locale 出中文）。</summary>
    public string Description { get; init; } = "";
    /// <summary>input.hint：有值表示该命令需要参数（选中时先补全命令名，不直接执行）。</summary>
    public string Hint { get; init; } = "";
    public bool HasInput { get; init; }
    public string SlashName => "/" + Name;

    /// <summary>ListViewItem 的 UIA 名回落到内容 ToString：给辅助技术与自动化一个可读名。</summary>
    public override string ToString() => string.IsNullOrEmpty(Description) ? SlashName : $"{SlashName}  {Description}";
}

/// <summary>统计面板的一条图例项（折线图的模型 / 环形图的扇区）：色块 + 名称。</summary>
public sealed class StatsLegendVm
{
    public string Name { get; init; } = "";
    public Brush Swatch { get; init; } = null!;
}

/// <summary>统计面板「模型用量」占比条的一行：模型名 + 右侧 tokens + 占比条（存储页式横向条形）。
/// 占比条用 ProgressBar 默认前景（系统强调色），行身不携带系列色。</summary>
public sealed class StatsModelVm
{
    public string Name { get; init; } = "";
    /// <summary>右侧绝对量（已本地化格式，如「45.7 万 tokens」）。</summary>
    public string TokensText { get; init; } = "";
    /// <summary>占比条取值 0..100（ProgressBar.Value）。</summary>
    public double BarValue { get; init; }
    /// <summary>条下的占比说明（如「占比 34%」）。</summary>
    public string ShareText { get; init; } = "";
}

/// <summary>单个会话的用量汇总（session/page 走查 journal 得到；页内不再二次请求）。</summary>
public sealed class SessionUsageVm
{
    public string SessionId { get; init; } = "";
    /// <summary>会话最后活动时刻（session/list 的 updatedAt，epoch 毫秒）。</summary>
    public long UpdatedAt { get; init; }
    /// <summary>journal 首末事件的时间跨度（毫秒）：会话存活窗口，不是「聊天时长」。</summary>
    public double SpanMs { get; init; }
    /// <summary>各 turn（turn/start → turn/end）时长之和（毫秒）：对话累计时长。</summary>
    public double TalkMs { get; init; }
}

public sealed partial class MainWindow : Window
{
    private DshKernelHost _kernel = new();
    private DshRpcClient? _rpc;
    private string? _activeSessionId;
    private int _journalCursor;                    // 当前会话 journal 游标
    private readonly ObservableCollection<ChatBubble> _messages = new();
    // 原始气泡保留，显示集合仅做投影；切回 normal 不重新拉 RPC，也不丢过程内容。
    private readonly ObservableCollection<ChatBubble> _visibleMessages = new();
    private readonly HashSet<int> _closedTranscriptTurns = new();
    private readonly HashSet<int> _startedTranscriptTurns = new();
    private readonly Dictionary<int, ChatBubble> _transcriptAnswers = new();
    private int _transcriptTurn;
    private bool _compactTranscript = true;
    /// <summary>进行中轮次的起点（turn/start 信封时间，epoch 毫秒）：turn/end 作差得本轮用时。
    /// 会话切换/清屏即失效，随 transcript 状态一并重置。</summary>
    private readonly Dictionary<int, long> _turnStartMs = new();
    /// <summary>本轮文件改动累加器（对标 dsh-client-ui-deliverables 的 deliverablesDefinition）：
    /// turn → 成功突变结果（result 事件 seq, 路径），首现顺序。turn/end 后由答案气泡按
    /// seq ≤ 收尾 seq 过滤去重渲染。</summary>
    private readonly Dictionary<int, List<(int Seq, string Path)>> _producedByTurn = new();
    /// <summary>未决突变调用：callId → 路径（write/edit/str_replace_editor；非突变为空串）。
    /// tool/result 成功时取路径落 _producedByTurn，turn/start 清空。</summary>
    private readonly Dictionary<string, string> _openMutationPaths = new(StringComparer.Ordinal);
    /// <summary>执行中的工具调用：callId → 气泡。tool/call 登记、tool/result 命中后停扫光注销；
    /// turn/end 未等到 result（中断/越权拒绝）时按表统一收尾，别让行头流光野跑。</summary>
    private readonly Dictionary<string, ChatBubble> _runningToolCalls = new(StringComparer.Ordinal);

    // ---------------- 撤回（编辑 / 文件修改）的会话作用域状态 ----------------
    // 内核 journal 只增不改：撤回过的事件不会从内核消失，补拉、轮询兜底、切会话回放都会把它们
    // 带回界面。所以「撤回编辑」不是删消息，而是把该轮整段标记为已撤回，在渲染入口统一抑制。

    /// <summary>撤回编辑的候选提问气泡：真正起了一轮的那次发送登记（会话忙时的排队/插话发送不登记
    /// ——还没开跑，撤回了也停不掉正在跑的那轮），turn/start 时按最后一个用户气泡移交。
    /// 动过文件或进程停下即失效（见 CanWithdrawEdit）。</summary>
    private ChatBubble? _withdrawCandidate;
    /// <summary>本轮是否已发生文件突变（write/edit/str_replace_editor 成功结算）：动过文件就不给
    /// 撤回编辑——内核侧状态已改，光撤回界面会造成「看着撤回了、文件却已改」的假象。</summary>
    private bool _turnMutated;
    /// <summary>正在运行的轮次号（turn/start 置位、turn/end 清零）：撤回编辑按它把整轮标记为已撤回。</summary>
    private int _runTurn;
    /// <summary>撤回发生在 turn/start 到达之前的窗口：下一个 turn/start 到达时把该轮补登记为已撤回。
    /// 期间来了新的提问（发送或 user/message 落屏）即作废——说明被撤回的那次 prompt 没起轮，
    /// 不能把随后真正开跑的那轮搭进去。</summary>
    private bool _withdrawPendingTurn;
    /// <summary>已撤回编辑的轮次（sessionId → turn 集合）。按会话保留：切走再切回来要靠它继续抑制。</summary>
    private readonly Dictionary<string, HashSet<int>> _withdrawnTurns = new(StringComparer.Ordinal);
    /// <summary>已撤回编辑的提问事件 seq（sessionId → seq 集合）。user/message 事件不带 turn 字段，
    /// 只能按信封 seq 抑制；撤回时回显还没被 journal 认领（Seq=0）的，由认领路径补记。</summary>
    private readonly Dictionary<string, HashSet<int>> _withdrawnSeqs = new(StringComparer.Ordinal);
    /// <summary>已撤回修改的轮次（sessionId → turn 集合）：撤过后按钮转「已撤回」态，不重复执行。</summary>
    private readonly Dictionary<string, HashSet<int>> _revertedTurns = new(StringComparer.Ordinal);
    /// <summary>本轮文件突变流水（turn → 有序记录）：撤回修改的取证来源。turn/start 清空，
    /// tool/result 成功结算时追加；切会话回放会重走一遍 journal，记录自然重建。</summary>
    private readonly Dictionary<int, List<TurnFileMutation>> _turnMutations = new();

    private HashSet<int> WithdrawnTurns(string sid)
        => _withdrawnTurns.TryGetValue(sid, out var set) ? set : _withdrawnTurns[sid] = new();

    private HashSet<int> WithdrawnSeqs(string sid)
        => _withdrawnSeqs.TryGetValue(sid, out var set) ? set : _withdrawnSeqs[sid] = new();

    private HashSet<int> RevertedTurns(string sid)
        => _revertedTurns.TryGetValue(sid, out var set) ? set : _revertedTurns[sid] = new();

    private bool IsTurnReverted(string sid, int turn) => _revertedTurns.TryGetValue(sid, out var set) && set.Contains(turn);

    private void ApplyTranscriptView(string preference)
    {
        var compact = preference != "normal";
        if (_compactTranscript == compact) return;
        _compactTranscript = compact;
        RefreshTranscriptView();
    }

    private void RefreshTranscriptView()
    {
        // 只折叠见过起点、正常完成且末步有无工具调用的持久答案的轮次。
        // 已撤回编辑的提问气泡也不进投影（对象仍留在 _messages，见 ChatBubble.Withdrawn）。
        var visible = _messages.Where(b => !b.Withdrawn && (!_compactTranscript || b.Turn <= 0 ||
            !_closedTranscriptTurns.Contains(b.Turn) || !_transcriptAnswers.TryGetValue(b.Turn, out var answer) ||
            (!(b.IsToolCall || b.IsReasoning || b.IsAssistantStep) || ReferenceEquals(b, answer)))).ToList();
        // 增量同步，普通流式追加不 Clear 列表，避免焦点和滚动反复跳动。
        var index = 0;
        foreach (var bubble in visible)
        {
            while (index < _visibleMessages.Count && !ReferenceEquals(_visibleMessages[index], bubble) &&
                !visible.Contains(_visibleMessages[index])) _visibleMessages.RemoveAt(index);
            if (index >= _visibleMessages.Count || !ReferenceEquals(_visibleMessages[index], bubble))
                _visibleMessages.Insert(index, bubble);
            index++;
        }
        while (_visibleMessages.Count > index) _visibleMessages.RemoveAt(index);
        // 「少女祈祷中」占位气泡不属于 journal，永远挂在最末（紧凑视图也不裁剪它）
        if (_pendingBubble is { } pending && !ReferenceEquals(_visibleMessages.Count > 0 ? _visibleMessages[^1] : null, pending))
        {
            _visibleMessages.Add(pending);
        }
    }

    private void TrackTranscriptEvent(string? type, JsonElement data, int turn)
    {
        if (turn <= 0) return;
        if (type == "turn/start")
        {
            _transcriptTurn = turn;
            _startedTranscriptTurns.Add(turn);
            _closedTranscriptTurns.Remove(turn);
            _transcriptAnswers.Remove(turn);
        }
        // 新步骤或新的持久消息使之前的答案失效，不能拿上一阶段的说明当结果。
        if (type is "step/start" or "assistant/message" or "tool/invoke" or "tool/call")
            _transcriptAnswers.Remove(turn);
        if (type == "turn/end")
        {
            _closedTranscriptTurns.Remove(turn);
            if (_startedTranscriptTurns.Contains(turn) && data.ValueKind == JsonValueKind.Object &&
                data.TryGetProperty("reason", out var reason) && Str(reason, "kind") == "completed")
                _closedTranscriptTurns.Add(turn);
        }
    }

    private static bool IsTranscriptAnswer(JsonElement message)
    {
        return Str(message, "role") == "assistant" &&
            message.TryGetProperty("content", out var content) && content.ValueKind == JsonValueKind.Array &&
            !content.EnumerateArray().Any(b => Str(b, "type") == "tool-call") &&
            content.EnumerateArray().Any(b => Str(b, "type") == "text" && !string.IsNullOrWhiteSpace(Str(b, "text")));
    }

    private string _shellLocale = "zh";
    // 壳支持的界面语言（Id = 语言包文件名 / 本地持久化值；KernelLocale = 内核 locale.preference 只认 zh/en，
    // 非中文界面语言让内核按英文出命令描述）。Label 一律用该语言的自称（native name）。
    private static readonly (string Id, string Label, string KernelLocale)[] UiLanguages =
    {
        ("zh", "简体中文", "zh"),
        ("zh-TW", "繁體中文", "zh"),
        ("en", "English", "en"),
        ("ja", "日本語", "en"),
        ("ko", "한국어", "en"),
        ("fr", "Français", "en"),
        ("it", "Italiano", "en"),
        ("de", "Deutsch", "en"),
        ("es-ES", "Español (España)", "en"),
        ("da", "Dansk", "en"),
        ("ru", "Русский", "en"),
        ("tr", "Türkçe", "en"),
        ("no", "Norsk", "en"),
        ("pl", "Polski", "en"),
        ("th", "ไทย", "en"),
        ("sv", "Svenska", "en"),
        ("fi", "Suomi", "en"),
        ("nl", "Nederlands", "en"),
        ("pt-BR", "Português (Brasil)", "en"),
        ("pt-PT", "Português (Portugal)", "en"),
        ("es-419", "Español (Latinoamérica)", "en"),
        ("uk", "Українська", "en"),
        ("bg", "Български", "en"),
        ("hu", "Magyar", "en"),
        ("id", "Bahasa Indonesia", "en"),
        ("el", "Ελληνικά", "en"),
        ("cs", "Čeština", "en"),
        ("ro", "Română", "en"),
        ("vi", "Tiếng Việt", "en"),
        ("ar", "العربية", "en"),
    };
    // 非 en 语言的翻译表：Assets/i18n/<id>.json（键 = ShellEnglish 的中文键），缺失键回落英文再回落中文。
    private readonly Dictionary<string, Dictionary<string, string>> _localeTables = new(StringComparer.Ordinal);

    private Dictionary<string, string> LoadLocaleTable(string locale)
    {
        if (locale is "zh" or "en") return ShellEnglish;
        if (_localeTables.TryGetValue(locale, out var cached)) return cached;
        var table = new Dictionary<string, string>(StringComparer.Ordinal);
        try
        {
            var path = Path.Combine(AppContext.BaseDirectory, "Assets", "i18n", locale + ".json");
            if (File.Exists(path))
            {
                using var doc = JsonDocument.Parse(File.ReadAllText(path));
                if (doc.RootElement.ValueKind == JsonValueKind.Object)
                    foreach (var prop in doc.RootElement.EnumerateObject())
                        if (prop.Value.ValueKind == JsonValueKind.String)
                            table[prop.Name] = prop.Value.GetString() ?? "";
            }
        }
        catch (Exception ex)
        {
            System.Diagnostics.Debug.WriteLine($"i18n load {locale} failed: {ex.Message}");
        }
        _localeTables[locale] = table;
        return table;
    }

    /// <summary>壳界面语言本地覆盖（设置页语言下拉写入）；null = 跟随内核 locale.preference。</summary>
    private string? _uiLanguageOverride = LoadUiLanguageOverride();

    private static string? LoadUiLanguageOverride()
    {
        try
        {
            var v = Windows.Storage.ApplicationData.Current.LocalSettings.Values["ui-language"] as string;
            return string.IsNullOrEmpty(v) || v == "zh" ? null : v;
        }
        catch (Exception)
        {
            return null; // 非打包形态没有 ApplicationData
        }
    }

    private static void SaveUiLanguageOverride(string? locale)
    {
        try
        {
            Windows.Storage.ApplicationData.Current.LocalSettings.Values["ui-language"] = locale ?? "zh";
        }
        catch (Exception)
        {
            // 非打包形态：仅本次运行生效
        }
    }

    // 中文原文就是键与中文值；未登记的文案原样回落，不翻译内核内容或用户输入。
    private static readonly Dictionary<string, string> ShellEnglish = new(StringComparer.Ordinal)
    {
        ["发送消息"] = "Send message",
        ["停止运行"] = "Stop run",
        ["少女祈祷中…"] = "Praying…",
        ["消息输入框"] = "Message input",
        ["描述你想要构建的内容, / 调用指令, @ 文件或对话"] = "Describe what you want to build, / for commands, @ for files or conversations",
        ["Enter 发送，Shift+Enter 换行，输入 / 唤起命令，@ 引用文件或会话"] = "Enter to send, Shift+Enter for a new line, / for commands, @ for files or conversations",
        ["开始一段新的会话"] = "Start a new conversation",
        ["空态：开始一段新的会话"] = "Start a new conversation",
        ["新会话"] = "New conversation",
        ["搜索会话…"] = "Search conversations…",
        ["设置"] = "Settings",
        ["返回"] = "Back",
        ["返回聊天"] = "Back to chat",
        ["工作区"] = "Workspaces",
        ["添加工作区"] = "Add workspace",
        ["视图选项"] = "View options",
        ["通用设置"] = "General",
        ["模型"] = "Models",
        ["插件"] = "Plugins",
        ["Agent 预设"] = "Agent presets",
        ["使用统计"] = "Usage",
        ["设置内容列"] = "Settings content",
        ["新会话的默认权限、外观与对话偏好。"] = "Default permissions, appearance and conversation preferences for new sessions.",
        ["填入各提供方的 API 密钥即可使用其模型；提供方与凭据位置来自内核目录。"] = "Enter provider API keys to use their models. Providers and credential locations come from the kernel catalog.",
        ["配置和查看本部署已安装的插件。"] = "Configure and inspect plugins installed in this deployment.",
        ["内核 Loader 实际加载的插件条目（pluginInventory/list 只读快照），以及各 Agent 预设的组装行。"] = "Plugins loaded by the kernel (a read-only pluginInventory/list snapshot), and the composition of each agent preset.",
        ["预设即一个会话的 Agent 所运行的插件组装——它的工具、提示词与能力。"] = "A preset defines the plugins an agent runs: its tools, prompts and capabilities.",
        ["权限"] = "Permissions",
        ["选择新会话的默认访问模式"] = "Choose the default access mode for new conversations",
        ["默认权限"] = "Default permissions",
        ["默认权限模式"] = "Default permission mode",
        ["仅可查看 / 工作区内修改 / 完全权限"] = "Read only / Workspace write / Full access",
        ["仅可查看"] = "Read only",
        ["工作区内修改"] = "Workspace write",
        ["完全权限"] = "Full access",
        ["外观"] = "Appearance",
        ["主题与会话正文字号"] = "Theme and conversation font size",
        ["主题"] = "Theme",
        ["浅色 / 深色 / 跟随系统"] = "Light / Dark / System",
        ["浅色"] = "Light",
        ["深色"] = "Dark",
        ["跟随系统"] = "System",
        ["字号大小"] = "Font size",
        ["仅影响会话内容的字号"] = "Applies only to conversation content",
        ["对话"] = "Conversation",
        ["已完成轮次的展示方式与繁忙时的发送行为"] = "Completed turn display and send behavior while busy",
        ["对话显示"] = "Transcript view",
        ["标准显示过程内容；紧凑只保留结果"] = "Normal shows all steps; compact folds completed turns to their answers",
        ["标准"] = "Normal",
        ["紧凑"] = "Compact",
        ["繁忙时的发送行为"] = "Send behavior while busy",
        ["智能体运行时 Enter 键和发送按钮的行为；Ctrl+Enter 使用另一行为"] = "Enter and Send while the agent runs; Ctrl+Enter uses the alternate behavior",
        ["排队发送"] = "Queue message",
        ["插话发送"] = "Steer now",
        ["语言"] = "Language",
        ["界面显示语言"] = "Interface language",
        ["背景皮肤"] = "Background image",
        ["导入图片或视频作为写代码时的内容区背景，用滑杆调到既能看出图、又不吃正文的浓度"] =
            "Import an image or video as the content background. Use the slider to balance visibility against text readability",
        ["导入图片"] = "Import image",
        ["导入视频"] = "Import video",
        ["导入视频背景皮肤"] = "Import video background",
        ["清除皮肤"] = "Clear background",
        ["背景图"] = "Background image",
        ["png / jpg / webp / bmp · mp4 / webm / mov（视频静音循环，与图片二选一）"] =
            "png / jpg / webp / bmp · mp4 / webm / mov (videos play muted on loop; a video and an image are mutually exclusive)",
        ["背景透明度"] = "Background opacity",
        ["拖动即时生效；没有导入图片时先记下，导入后按这个浓度显示"] =
            "Applies as you drag; without an image the value is kept for the next import",
        ["失焦暂停视频"] = "Pause video when unfocused",
        ["视频背景只在窗口聚焦时播放；切到别的程序时自动暂停，回焦继续"] =
            "The video background plays only while the window is focused; it pauses automatically when you switch to another program and resumes on return",
        ["窗口失焦时暂停视频背景"] = "Pause the video background when the window is unfocused",
        // ---- Wallpaper Engine 壁纸源（个性化 → 背景皮肤） ----
        ["Wallpaper Engine"] = "Wallpaper Engine",
        ["用本机 Wallpaper Engine 订阅的壁纸做背景（视频静音循环），与上面导入的图片/视频二选一"] =
            "Use wallpapers from this machine's Wallpaper Engine subscriptions as the background (videos play muted on loop); mutually exclusive with the imported image/video above",
        ["点缩略图即套用；视频项显示工坊预览图，与上面导入的图片/视频二选一。"] =
            "Click a thumbnail to apply it; video items show their workshop preview image; mutually exclusive with the imported image/video above.",
        ["Wallpaper Engine 壁纸缩略图"] = "Wallpaper Engine wallpaper thumbnails",
        ["正在读取…"] = "Loading…",
        ["读取失败"] = "Load failed",
        ["读取失败：{0}"] = "Load failed: {0}",
        ["读取失败：壁纸服务没响应。本机需要装有 Wallpaper Engine 并订阅壁纸。"] =
            "Load failed: the wallpaper service did not respond. This machine needs Wallpaper Engine installed with subscribed wallpapers.",
        ["未检测到壁纸插件。重启 Blade² 会自动重试安装；离线时这条会一直出现，连上网再试。"] =
            "Wallpaper plugin not detected. Restarting Blade² retries the install; while offline this keeps showing until you are back online.",
        ["本机没有可用的 Wallpaper Engine 壁纸：需要安装 Wallpaper Engine 并订阅壁纸。"] =
            "No usable Wallpaper Engine wallpapers on this machine: install Wallpaper Engine and subscribe to wallpapers.",
        ["没有可用壁纸"] = "No wallpapers available",
        ["共 {0} 个壁纸，点缩略图套用"] = "{0} wallpapers — click a thumbnail to apply",
        ["（已隐藏 {0} 个低分辨率预览图）"] = " ({0} low-resolution previews hidden)",
        ["视频"] = "Video",
        ["把壁纸「{0}」设为背景"] = "Set wallpaper \"{0}\" as the background",
        ["正在下载「{0}」…"] = "Downloading \"{0}\"…",
        ["这个壁纸没有可用的文件。"] = "This wallpaper has no usable file.",
        ["下载失败：壁纸服务没返回这个文件。"] = "Download failed: the wallpaper service did not return this file.",
        ["已应用「{0}」"] = "Applied \"{0}\"",
        // ---- 设置分区「个性化」（自定义指令 + 窗口材质 + 背景皮肤） ----
        ["个性化"] = "Personalization",
        ["指令决定它怎么回应你，材质与皮肤决定它看起来是什么样。"] =
            "Instructions shape how it responds to you; the material and skin shape how it looks.",
        ["窗口材质"] = "Window material",
        ["侧栏与顶栏透出的系统材质：Mica 柔和、亚克力更透；系统不支持时自动回退。"] =
            "The system material the sidebar and top bar show through: Mica is subtle, acrylic is more transparent; falls back automatically when unsupported.",
        ["材质"] = "Material",
        ["切换后立即生效，重启后保持"] = "Applies immediately and persists across restarts",
        ["消息气泡"] = "Message bubbles",
        ["气泡背景：半透明直接透出背后画面，亚克力是系统材质；两项都即时生效。"] =
            "Bubble background: the translucent option shows the content behind it, the acrylic option is the system material; both apply immediately.",
        ["气泡材质"] = "Bubble material",
        ["半透明最透，亚克力是系统材质，跟随跟窗口材质走"] =
            "Translucent is the most see-through, acrylic is the system material, follow tracks the window material",
        ["半透明"] = "Translucent",
        ["跟随窗口材质"] = "Follow window material",
        ["气泡不透明度"] = "Bubble opacity",
        ["数值越大气泡自身越实，背后画面透出越少"] =
            "Higher values make bubbles more solid and show less of what is behind them",
        ["Mica"] = "Mica",
        ["Mica Alt"] = "Mica Alt",
        ["亚克力"] = "Acrylic",
        ["无（纯色）"] = "None (solid)",
        ["自定义指令"] = "Custom instructions",
        ["写给所有对话的长期说明：怎么称呼你、用什么语气、有哪些固定约束。对所有会话始终生效。"] =
            "Standing notes for every conversation: how to address you, what tone to use, which constraints always apply.",
        ["指令内容"] = "Instructions",
        ["新会话立即生效；进行中的会话在下一条消息生效。"] =
            "New conversations pick it up right away; a running conversation applies it on the next message.",
        ["保存失败：{0}"] = "Save failed: {0}",
        ["共 0 字 · 停止输入后自动保存"] = "0 characters · Auto-saves when you stop typing",
        ["字数 {0} / {1}"] = "{0} / {1} characters",
        ["插件配置"] = "Plugin settings",
        ["插件列表"] = "Plugin inventory",
        ["默认模型"] = "Default model",
        ["服务商"] = "Provider",
        ["API 密钥"] = "API key",
        ["发现模型"] = "Discover models",
        ["刷新"] = "Refresh",
        ["等待审批"] = "Awaiting approval",
        ["拒绝"] = "Reject",
        ["允许一次"] = "Allow once",
        ["计划评审"] = "Plan review",
        ["内核提问"] = "Question from the agent",
        ["补充说明"] = "Additional details",
        ["补充说明（可选，随答案回传为 custom）"] = "Additional details (optional, sent with your answer)",
        ["跳过（不回答）"] = "Skip (do not answer)",
        ["提交"] = "Submit",
        ["提交答案"] = "Submit answer",
        ["模型与推理等级"] = "Model and reasoning effort",
        ["文件"] = "Files",
        ["工作区文件"] = "Workspace files",
        ["打开文件面板"] = "Open files panel",
        ["累计 Token 数"] = "Total tokens",
        ["峰值 Token 数"] = "Peak tokens",
        ["最长聊天时长"] = "Longest conversation",
        ["当前连续天数"] = "Current streak",
        // —— 顶栏 / 侧栏 / 会话与工作区操作 ——
        ["返回聊天页"] = "Back to chat",
        ["搜索会话"] = "Search conversations",
        ["按标题或内容检索会话"] = "Search conversations by title or content",
        ["搜索"] = "Search",
        ["新建会话"] = "New session",
        ["分组方式与排序方式"] = "Grouping and sorting",
        ["未分组"] = "Ungrouped",
        ["未分组会话"] = "Ungrouped conversations",
        ["收起"] = "Collapse",
        ["会话操作"] = "Session actions",
        ["重命名"] = "Rename",
        ["分叉会话"] = "Fork conversation",
        ["归档会话"] = "Archive conversation",
        ["移动到工作区"] = "Move to workspace",
        ["取消运行"] = "Cancel run",
        ["打开工作区路径"] = "Open workspace path",
        ["重命名工作区"] = "Rename workspace",
        ["上移"] = "Move up",
        ["下移"] = "Move down",
        ["删除工作区"] = "Delete workspace",
        ["新标题"] = "New title",
        ["新名称"] = "New name",
        ["确定"] = "OK",
        ["取消"] = "Cancel",
        ["重命名会话"] = "Rename conversation",
        ["新建工作区"] = "New workspace",
        ["新建工作区…"] = "New workspace…",
        ["文件夹路径"] = "Folder path",
        ["浏览…（内核选择器）"] = "Browse… (kernel picker)",
        ["创建"] = "Create",
        ["使用此目录"] = "Use this folder",
        ["新建文件夹…"] = "New folder…",
        ["新建文件夹"] = "New folder",
        ["文件夹名"] = "Folder name",
        ["目录项"] = "Directory entries",
        ["目录项超过内核上限，仅显示前若干项。"] = "Too many entries for the kernel limit; showing only the first ones.",
        // —— 输入区 / 浮层 / 启动 ——
        ["添加附件或操作"] = "Add attachment or action",
        ["添加图片或文件"] = "Add image or file",
        ["放大查看 {0}"] = "Enlarge {0}",
        ["在应用中打开"] = "Open in app",
        ["会话反馈"] = "Session feedback",
        ["选择工作区"] = "Select workspace",
        ["新会话将在此工作区中创建"] = "New conversations will be created in this workspace",
        ["Agent 模式"] = "Agent mode",
        ["即将开始的这个会话所用的 Agent 预设"] = "The agent preset for the conversation you are about to start",
        ["访问模式"] = "Access mode",
        ["计划模式"] = "Plan mode",
        ["退出"] = "Exit",
        ["退出计划模式"] = "Exit plan mode",
        ["命令（↑↓ 选择，Enter 执行，Esc 关闭）"] = "Commands (↑↓ select, Enter run, Esc close)",
        ["引用（↑↓ 选择，Enter 插入，Esc 关闭）"] = "References (↑↓ select, Enter insert, Esc close)",
        ["正在启动 dsh 内核…"] = "Starting the dsh kernel…",
        // —— 使用统计页 ——
        ["使用统计概览"] = "Usage overview",
        ["最长连续天数"] = "Longest streak",
        ["Token 活动"] = "Token activity",
        ["Token 活动统计口径"] = "Token activity metric",
        ["每日"] = "Daily",
        ["每周"] = "Weekly",
        ["累计"] = "Cumulative",
        ["少"] = "Less",
        ["多"] = "More",
        ["时间范围"] = "Time range",
        ["统计时间范围"] = "Statistics time range",
        ["近 7 天"] = "Last 7 days",
        ["近 30 天"] = "Last 30 days",
        ["每日 Token 趋势图"] = "Daily token trend",
        ["模型用量"] = "Model usage",
        // —— 设置：通用页 / 设置文件段 ——
        ["设置文件"] = "Settings file",
        ["设置文档"] = "Settings document",
        ["打开设置文档"] = "Open settings document",
        ["在内核主机上用默认文本编辑器打开 settings 文档（settings/openSettingsDocument）"] = "Opens the settings document with the default text editor on the kernel host (settings/openSettingsDocument)",
        ["把内核侧设置文档落到本地并用默认编辑器打开。"] = "Materializes the kernel-side settings document locally and opens it with the default editor.",
        ["恢复默认"] = "Restore defaults",
        ["恢复本页默认"] = "Restore this page to defaults",
        ["清空本页命名空间的用户设置段（settings/replace），回到内核默认值"] = "Clears this page's user settings sections (settings/replace) and falls back to kernel defaults",
        ["导入背景皮肤"] = "Import background image",
        ["清除背景皮肤"] = "Clear background image",
        ["重试保存"] = "Retry save",
        ["设置尚未保存"] = "Settings not saved",
        ["恢复默认未完成，未成功的草稿已保留。请检查连接后重试恢复默认。"] = "Restore-defaults failed; drafts that did not commit are kept. Check the connection and retry.",
        // —— 设置：模型 / API 密钥 ——
        ["凭据引用由内核目录给出（settingsNs/settingsPath 下的 apiKeyEnv）。"] = "Credential references come from the kernel catalog (apiKeyEnv under settingsNs/settingsPath).",
        ["内核未声明任何可配置的提供方。"] = "The kernel declares no configurable providers.",
        ["输入 API 密钥"] = "Enter API key",
        ["已保存，输入新值可替换"] = "Saved; type a new value to replace",
        ["已保存"] = "Saved",
        ["输入密钥后即可使用其模型"] = "Enter the key to use this provider's models",
        ["内核未给出配置位置"] = "The kernel gave no configuration location",
        ["尚未配置密钥"] = "No key configured yet",
        ["清除"] = "Clear",
        ["清除内核凭据库中的该密钥"] = "Clears this key from the kernel credential store",
        ["该来源不可写（环境变量等）"] = "This source is not writable (environment variable, etc.)",
        ["调用提供方端点列出可用模型（llm/discoverModels）"] = "Calls the provider endpoint to list available models (llm/discoverModels)",
        ["其他可配置提供方"] = "Other configurable providers",
        ["未启用"] = "Not enabled",
        ["清除 API 密钥"] = "Clear API key",
        ["密钥清除失败，原有草稿已保留。请检查连接后重试清除。"] = "Key clearing failed; the existing draft is kept. Check the connection and retry clearing.",
        ["内核已注册的提供方路由（llm/listProviders）"] = "Provider routes registered by the kernel (llm/listProviders)",
        ["可从提供方端点发现（见上方 API 密钥卡的「发现模型」）"] = "Discoverable from provider endpoints (see Discover models in the API key card above)",
        ["推理等级"] = "Reasoning effort",
        ["off / low / high / max；留空跟随模型默认"] = "off / low / high / max; empty follows the model default",
        ["发现的模型"] = "Discovered models",
        ["用作默认模型"] = "Use as default model",
        ["关闭"] = "Close",
        // —— 设置：模型页（提供方卡 + 添加/编辑流，对照 dsh-client-ui-settings-models） ——
        ["填入各提供方的 API 密钥即可使用其模型。"] = "Enter each provider's API key to use its models.",
        ["已保存 {0}。"] = "Saved {0}.",
        ["添加提供方"] = "Add provider",
        ["目录里没有可添加的提供方"] = "No providers left to add in the catalog",
        ["提供方"] = "Provider",
        ["自定义"] = "Custom",
        ["自定义提供方"] = "Custom provider",
        ["编辑 {0}"] = "Edit {0}",
        ["删除 {0}"] = "Delete {0}",
        ["删除 {0}？"] = "Delete {0}?",
        ["删除 {0} 会移除其配置和存储的 API 密钥。"] = "Deleting {0} removes its configuration and stored API key.",
        ["删除 {0} 会移除其配置；其使用的凭据（如有）由其他位置管理，将会保留。"] = "Deleting {0} removes its configuration; any credential it uses is managed elsewhere and will be kept.",
        ["API 密钥已配置"] = "API key configured",
        ["API 密钥缺失"] = "API key missing",
        ["已配置——输入新值可替换"] = "Configured — type a new value to replace",
        ["输入 API 密钥，或留空使用环境认证"] = "Enter an API key, or leave blank to use environment authentication",
        ["由启动环境提供（只读）"] = "Provided by the launch environment (read-only)",
        ["请输入 API 密钥；留空则保持已存储的密钥。"] = "Enter the API key, or leave the field empty to keep the stored one.",
        ["请输入 API 密钥；若该提供方以其他方式鉴权，可以留空。"] = "Enter the API key, or leave the field empty if this provider authenticates another way.",
        ["该 API 密钥格式错误，请检查。"] = "This API key is not in a valid format. Please check it.",
        ["自定义设置"] = "Customized settings",
        ["API 地址"] = "Base URL",
        ["API 协议"] = "API protocol",
        ["未选择"] = "Not selected",
        ["API 协议不能为空。"] = "The API protocol cannot be empty.",
        ["显示名称"] = "Display name",
        ["可选，默认使用 Provider ID"] = "Optional; defaults to the provider ID",
        ["Provider ID"] = "Provider ID",
        ["Provider ID 不能为空。"] = "The provider ID cannot be empty.",
        ["以小写字母开头的标识，在请求中唯一标识该提供方，并用于派生凭据名。"] = "A lowercase identifier starting with a letter that uniquely names this provider in requests and derives its credential name.",
        ["需以小写字母开头，之后可用小写字母、数字和短横线。"] = "Start with a lowercase letter; then lowercase letters, digits, and dashes.",
        ["已有提供方使用了这个 ID。"] = "A provider already uses this ID.",
        ["自定义提供方需要填写 API 地址。"] = "A custom provider needs a base URL.",
        ["请输入有效的 HTTP 或 HTTPS 地址。"] = "Enter a valid HTTP or HTTPS URL.",
        ["自定义提供方至少需要一个模型。"] = "A custom provider needs at least one model.",
        ["创建提供方"] = "Create provider",
        ["写入失败，请检查连接和字段值后重试。"] = "The write failed; check the connection and field values, then retry.",
        ["{0}：密钥保存失败，请检查连接和凭据来源是否可写后重试。"] = "{0}: key save failed; check the connection and whether the credential source is writable, then retry.",
        ["这张卡片打开期间，这些设置已被其他地方改动。请关闭后重新打开，在当前值上编辑。"] = "These settings changed elsewhere while this card was open. Close and reopen it to edit the current values.",
        ["模型目录"] = "Models",
        ["正在使用适配器默认模型"] = "Using the adapter defaults",
        ["已自定义模型目录"] = "Customized model catalog",
        ["恢复默认模型"] = "Restore defaults",
        ["获取可用模型"] = "Fetch available models",
        ["模型选择器中将不显示任何模型；目录外 ID 仍可直接发送。"] = "No models will be shown in the selector. Unlisted IDs can still be sent directly.",
        ["模型 ID"] = "Model ID",
        ["模型 ID 不能为空。"] = "The model ID cannot be empty.",
        ["上下文窗口"] = "Context window",
        ["最大输出 token 数"] = "Max output tokens",
        ["容量"] = "Capacities",
        ["容量需为数字，可加 K 或 M 后缀。"] = "A capacity must be a number, optionally suffixed K or M.",
        ["显示名称不能为空。"] = "The display name cannot be empty.",
        ["模型 {0}：模型 ID 不能为空。"] = "Model {0}: the model ID cannot be empty.",
        ["模型 {0}：模型 ID 不能重复。"] = "Model {0}: each model ID may appear once.",
        ["模型 {0}：显示名称不能为空。"] = "Model {0}: the display name cannot be empty.",
        ["模型 {0}：上下文窗口必须是正数，例如 131072、256K 或 1M。"] = "Model {0}: the context window must be a positive count, like 131072, 256K, or 1M.",
        ["模型 {0}：最大输出 token 数必须是正数，例如 8192、64K 或 1M。"] = "Model {0}: max output tokens must be a positive count, like 8192, 64K, or 1M.",
        ["添加模型"] = "Add model",
        ["删除模型"] = "Delete model",
        ["该提供方没有列出任何模型，请手动添加。"] = "The provider listed no models. Add them by hand.",
        ["选择要添加的模型"] = "Choose models to add",
        ["以下是模型提供方的可用模型，勾选要添加的模型。"] = "These are the models this provider has available. Choose the ones to add.",
        ["搜索模型"] = "Search models",
        ["全选"] = "Select all",
        ["取消全选"] = "Deselect all",
        ["添加所选"] = "Add selected",
        ["没有匹配的模型。"] = "No matching models.",
        // —— 设置：插件区 ——
        ["插件视图"] = "Plugins view",
        ["终端"] = "Terminal",
        ["限制 agent 运行的每一条命令。"] = "Governs every command the agent runs.",
        ["命令超时（毫秒）"] = "Command timeout (ms)",
        ["单条命令允许运行多久，超时即终止。"] = "How long a single command may run before being terminated.",
        ["单流输出上限（字节）"] = "Per-stream output limit (bytes)",
        ["超出部分会转存到临时文件，而不是被丢弃。"] = "Overflow is spilled to a temp file instead of being dropped.",
        ["Agent 循环"] = "Agent loop",
        ["Agent 如何派发工具调用。"] = "How the agent dispatches tool calls.",
        ["并行工具调用数"] = "Parallel tool calls",
        ["同一步内最多同时运行多少个可并行的调用。"] = "How many parallelizable calls run at once within a step.",
        ["控制 Agent 为 Subagent 选择模型的权限。"] = "Controls whether the agent may pick models for subagents.",
        ["允许 Agent 为 Subagent 选择模型"] = "Allow the agent to choose models for subagents",
        ["开启后，Agent 可以为每个 Subagent 选择提供方和模型。仅影响新会话。"] = "When on, the agent may pick provider and model per subagent. New conversations only.",
        ["内核要求开启时至少有一个允许模型；当前读不到默认模型，开启可能被内核拒绝并在此提示。"] = "The kernel requires at least one allowed model when this is on; no default model is readable now, so enabling may be rejected and reported here.",
        ["网页搜索"] = "Web search",
        ["DeepSeek 搜索提供方。"] = "The DeepSeek search provider.",
        ["API Key（未配置）"] = "API key (not configured)",
        ["配置之前搜索不可用"] = "Search is unavailable until configured",
        ["插件搜索 API 密钥"] = "Plugin search API key",
        ["接口地址"] = "Endpoint URL",
        ["留空则使用提供方默认地址。"] = "Leave empty to use the provider default.",
        ["单次请求最多搜索次数"] = "Max searches per request",
        ["一次请求在必须作答前最多可以搜索多少次。"] = "How many searches one request may run before it must answer.",
        ["概览"] = "Overview",
        ["计数直接来自本次快照，不做缓存。"] = "Counts come straight from this snapshot; nothing is cached.",
        ["按模块名或条目 id 过滤"] = "Filter by module name or entry id",
        ["按模块名或条目 id 过滤…"] = "Filter by module name or entry id…",
        // —— 设置：Agent 预设区 ——
        ["Agent 预设组装行"] = "Agent preset assembly rows",
        ["预设的 rows 才是模型可见插件的运行位置（Loader 条目之外的第二处清单，与内核清单一致）。"] = "A preset's rows are where model-visible plugins actually run (a second inventory beyond loader entries, matching the kernel list).",
        ["（该预设没有组装行）"] = "(this preset has no assembly rows)",
        ["预设目录"] = "Preset directory",
        ["新增自定义预设"] = "New custom preset",
        ["用「复制为…」从任一预设派生一份用户预设，再用「打开目录」编辑其定义。"] = "Use Copy as… to derive a user preset, then Open folder to edit its definition.",
        ["本部署声明不可创作预设（agentPresets/list 的 authorable=false）。"] = "This deployment reports presets as not authorable (agentPresets/list authorable=false).",
        ["本部署无法原生打开预设目录（settings/canOpenAgentPresetDirectory 返回 false）。"] = "This deployment cannot open preset directories natively (settings/canOpenAgentPresetDirectory returned false).",
        ["可用"] = "Available",
        ["不可用"] = "Unavailable",
        ["复制为…"] = "Copy as…",
        ["当前会话使用"] = "Use for this session",
        ["先在左侧选择一个会话"] = "Select a conversation in the left sidebar first",
        ["agentPresets/select 应用到当前会话；内核只允许在会话的 agent 启动前切换（已开始会回 agent-preset/locked）"] = "Applies to the current session via agentPresets/select; the kernel allows switching only before the session's agent starts (started sessions return agent-preset/locked)",
        ["设为默认"] = "Set as default",
        ["打开目录"] = "Open folder",
        ["在内核主机上打开该预设所在目录"] = "Opens this preset's folder on the kernel host",
        ["本部署无法原生打开目录（settings/canOpenAgentPresetDirectory=false）"] = "This deployment cannot open folders natively (settings/canOpenAgentPresetDirectory=false)",
        ["删除"] = "Delete",
        ["新预设 id"] = "New preset id",
        ["小写字母/数字/短横线，如 my-agent"] = "lowercase letters/digits/hyphens, e.g. my-agent",
        ["显示名"] = "Display name",
        ["复制"] = "Duplicate",
        ["删除预设"] = "Delete preset",
        ["内置"] = "Built-in",
        ["自定义"] = "Custom",
        // —— 队列 / 反馈 ——
        ["在新对话中分支"] = "Branch into a new conversation",
        ["仅可从已完成轮次的最后一条消息分支"] = "Available only on the last message of a completed turn",
        ["复制消息"] = "Copy message",
        ["已复制"] = "Copied",
        ["思考"] = "Think",
        ["工具调用"] = "Tool call",
        ["读取"] = "Read",
        ["读取图片"] = "Read image",
        ["网页获取"] = "Fetch",
        ["写入"] = "Write",
        ["代码"] = "Code",
        ["用时 {0}"] = "Ran for {0}",
        ["用量 {0} tok"] = "Usage {0} tok",
        ["{0}秒"] = "{0}s",
        ["{0}分{1}秒"] = "{0}m{1}s",
        ["{0}小时{1}分"] = "{0}h{1}m",
        ["{0}月{1}日 {2}"] = "{0}/{1} {2}",
        ["{0}年{1}月{2}日 {3}"] = "{0}-{1}-{2} {3}",
        ["本轮文件改动"] = "Files changed",
        ["打开 {0}"] = "Open {0}",
        ["打开失败：{0}"] = "Open failed: {0}",
        // —— 撤回（编辑 / 文件修改） ——
        ["撤回编辑"] = "Withdraw edit",
        ["撤回编辑（停止运行并把这句放回输入框）"] = "Withdraw edit (stop the run and put this message back in the composer)",
        ["撤回本轮修改"] = "Revert changes this turn",
        ["撤回本轮修改（把文件恢复到修改前）"] = "Revert changes this turn (restore the files to their state before the change)",
        ["已撤回本轮修改"] = "Changes reverted",
        ["撤回修改"] = "Revert",
        ["将把本轮（第 {0} 轮）改过的文件恢复到修改前的状态："] = "Restore the files changed in turn {0} to their state before the change:",
        ["修改过的文件（恢复原内容）"] = "Edited files (original content restored)",
        ["新建的文件（将被删除）"] = "Files created this turn (will be deleted)",
        ["没有回退依据、撤回时会被跳过的文件"] = "Files with no revert basis; they will be skipped",
        ["恢复后不可撤销。若这些文件在本轮之后又被改动过，撤回可能不完整。"] = "This cannot be undone. If these files were changed again after this turn, the revert may be incomplete.",
        ["已恢复 {0} 个文件。"] = "Restored {0} file(s).",
        ["撤回未完成"] = "Revert incomplete",
        ["成功 {0} 个，失败 {1} 个：{2}"] = "{0} succeeded, {1} failed: {2}",
        ["文件已不存在"] = "File no longer exists",
        ["找不到改动后的内容，文件可能已被其他改动覆盖"] = "The changed content was not found; the file may have been overwritten by other changes",
        ["改动位置不唯一，为避免改错已放弃"] = "The change site is not unique; aborted to avoid editing the wrong place",
        ["没有可用的回退依据"] = "No revert basis available",
        ["内核已收到、等当前轮结束后发送"] = "Received by the kernel; sent after the current turn",
        ["均已完成"] = "all done",
        ["编辑"] = "Edit",
        ["插话"] = "Steer",
        ["编辑排队消息"] = "Edit queued message",
        ["保存"] = "Save",
        ["分类（可留空）"] = "Category (optional)",
        ["说明（可留空）"] = "Details (optional)",
        ["这条会话的体验如何？"] = "How was this session?",
        ["反馈分类"] = "Feedback category",
        ["反馈说明"] = "Feedback notes",
        ["记录"] = "Record",
        ["这条回复哪里好 / 哪里不好（可选，纯文本）"] = "What was good / bad about this reply (optional, plain text)",
        ["任务结果"] = "Task result",
        ["指令遵循"] = "Instruction following",
        ["产品交互"] = "Product interaction",
        ["服务稳定性"] = "Service stability",
        ["资源消耗"] = "Resource cost",
        ["安全隐私与权限"] = "Security, privacy and permissions",
        ["其他"] = "Other",
        // —— 错误提示 / 系统消息 / 工作区选择 ——
        ["自动保存失败，未保存的草稿已保留。请重试。"] = "Auto-save failed; unsaved drafts are kept. Please retry.",
        ["内核未连接，未保存的草稿已保留。连接后请重试。"] = "The kernel is not connected; unsaved drafts are kept. Retry after reconnection.",
        ["保存未完成，未保存的草稿已保留。请重试。"] = "Save did not complete; unsaved drafts are kept. Please retry.",
        ["文档目录（默认）"] = "Documents folder (default)",
        ["无法在应用中打开：当前会话没有已知的工作目录（cwd）。"] = "Cannot open in app: the current session has no known working directory (cwd).",
        ["请先选择一个会话：会话反馈作用在具体会话上。"] = "Select a conversation first: session feedback applies to a specific conversation.",
        ["会话反馈已记录（内核会话日志追加 feedback/record 事件）。"] = "Session feedback recorded (a feedback/record event was appended to the kernel session log).",
        ["请先选择一个会话：命令在会话（agent）作用域内执行。"] = "Select a conversation first: commands run within the conversation (agent) scope.",
        // ---- 二轮补译：插值模板与动态区文案 ----
        ["未标注模型"] = "Unnamed model",
        ["{0}：{1} tokens"] = "{0}: {1} tokens",
        ["{0} 失败"] = "{0} failed",
        ["内核启动失败"] = "Kernel failed to start",
        // ---- 内核异步引导：主页加载卡的阶段文案（壳先亮相，内核在后台起） ----
        ["正在准备内核组件…"] = "Preparing kernel components…",
        ["正在启动内核…"] = "Starting the kernel…",
        ["正在连接内核…"] = "Connecting to the kernel…",
        ["正在加载工作区与会话…"] = "Loading workspaces and conversations…",
        ["内核加载已进行 {0} 秒"] = "Kernel loading for {0} seconds",
        ["第 {0} 步，共 {1} 步 · {2}%"] = "Step {0} of {1} · {2}%",
        ["正在安装插件 {0}/{1}"] = "Installing plugins {0}/{1}",
        ["重试"] = "Retry",
        ["内核还在加载中，请稍候再发。"] = "The kernel is still loading; please try sending again shortly.",
        ["未找到内置内核；请安装 npm 版 dsh 或重新安装 Blade²"] = "Bundled kernel not found; install the npm version of dsh or reinstall Blade²",
        ["打开 Blade²"] = "Open Blade²",
        ["退出"] = "Exit",
        ["工具审批请求"] = "Tool approval request",
        ["{0} 轮 {1} 步 · {2} tok/s"] = "{0} turns {1} steps · {2} tok/s",
        ["{0} tok · 缓存命中 {1}%"] = "{0} tok · cache hit {1}%",
        ["系统提示词"] = "System prompt",
        ["系统提示词更新"] = "System prompt update",
        ["上下文注入"] = "Context injection",
        ["会话统计"] = "Session stats",
        ["模型用时"] = "Model time",
        ["工具调用用时"] = "Tool-call time",
        ["首 token 平均（TTFT）"] = "Avg. time to first token (TTFT)",
        ["输出速度（TPS）"] = "Output speed (TPS)",
        ["Token 用量"] = "Token usage",
        ["缓存命中"] = "Cache hit",
        ["未缓存输入"] = "Uncached input",
        ["缓存读取"] = "Cache read",
        ["输出"] = "Output",
        ["上下文"] = "Context",
        ["上下文 {0}%"] = "Context {0}%",
        ["上下文已用 {0}%"] = "{0}% of context used",
        ["已用"] = "Used",
        ["上下文容量"] = "Context capacity",
        ["会话创建响应缺少 sessionId。"] = "Session creation response is missing a sessionId.",
        ["没有找到与 {0} 相关的结果"] = "No results found for {0}",
        ["刚刚"] = "just now",
        ["{0}分钟前"] = "{0} min ago",
        ["{0}小时前"] = "{0} h ago",
        ["{0}天前"] = "{0} d ago",
        ["{0}个月前"] = "{0} mo ago",
        ["{0}年前"] = "{0} yr ago",
        ["分组方式"] = "Group by",
        ["按工作区"] = "By workspace",
        ["单列表"] = "Single list",
        ["排序方式"] = "Sort by",
        ["手动排序"] = "Manual order",
        ["最近更新"] = "Recently updated",
        ["{0} 轮对话"] = "{0} conversation turns",
        ["展开其余 {0} 个会话"] = "Expand the other {0} sessions",
        ["重命名失败：{0}"] = "Rename failed: {0}",
        ["将把“{0}”从工作区列表中移除。文件夹与会话记录会保留，其会话将显示在“未分组”下。"] = "\"{0}\" will be removed from the workspace list. The folder and session records are kept; its sessions will appear under \"Ungrouped\".",
        ["删除失败：{0}"] = "Delete failed: {0}",
        ["取消失败：{0}"] = "Cancel failed: {0}",
        ["内核目录选择器不可用：{0}"] = "Kernel directory picker unavailable: {0}",
        ["创建失败：{0}"] = "Create failed: {0}",
        ["内核目录浏览不可用：本部署使用原生目录选择器，请用「浏览…（内核选择器）」按钮选取文件夹，或直接输入路径。"] = "Kernel directory browsing is unavailable: this deployment uses the native picker. Use the \"Browse… (kernel picker)\" button to choose a folder, or type the path directly.",
        ["内核目录浏览不可用：{0}"] = "Kernel directory browsing unavailable: {0}",
        ["读取目录失败：{0}"] = "Failed to read directory: {0}",
        ["新建文件夹失败：{0}"] = "Failed to create folder: {0}",
        ["该工作区没有路径记录。"] = "This workspace has no recorded path.",
        ["本部署无法在内核主机打开路径（session/canOpenWorkspacePath=false）。"] = "This deployment cannot open paths on the kernel host (session/canOpenWorkspacePath=false).",
        ["打开路径失败：{0}"] = "Failed to open path: {0}",
        ["调整顺序失败：{0}"] = "Failed to reorder: {0}",
        ["归档失败：{0}"] = "Archive failed: {0}",
        ["移动失败：{0}"] = "Move failed: {0}",
        ["分叉失败：{0}"] = "Fork failed: {0}",
        ["内核未连接"] = "Kernel not connected",
        ["新建会话失败：{0}"] = "Failed to create session: {0}",
        ["会话历史分页未前进。"] = "Session history pagination did not advance.",
        ["会话同步失败：{0}"] = "Session sync failed: {0}",
        ["连接恢复后订阅失败：{0}"] = "Subscription failed after reconnect: {0}",
        ["图片"] = "Image",
        ["内核执行失败"] = "Kernel execution failed",
        ["⚙ {0}（命令执行）"] = "⚙ {0} (command execution)",
        ["粘贴图片 {0:HHmmss}.png"] = "Pasted image {0:HHmmss}.png",
        ["拖入图片 {0:HHmmss}.png"] = "Dropped image {0:HHmmss}.png",
        ["发送失败：{0}"] = "Send failed: {0}",
        ["图片读取失败，未添加附件：{0}"] = "Failed to read the image; no attachment added: {0}",
        ["有 {0} 张图片无法按图片识别，已改为文件发送：{1}"] =
            "{0} image(s) could not be recognized as images and were sent as files: {1}",
        ["(工具)"] = "(tool)",
        ["工具 {0} 请求越权执行"] = "Tool {0} requests elevated execution",
        ["审批回传失败，请重试：{0}"] = "Approval reply failed, please retry: {0}",
        ["已跳过该提问（内核侧按未作答继续）。"] = "Question skipped (the kernel continues as unanswered).",
        ["已回传提问答复。"] = "Question answer submitted.",
        ["提问回传失败，请重试：{0}"] = "Question reply failed, please retry: {0}",
        ["无法打开：宿主桌面不可用（内核 workspaceDesktop().available=false）。"] = "Cannot open: host desktop unavailable (kernel workspaceDesktop().available=false).",
        ["无法打开：内核在会话日志里找不到该交付物（文件可能已被移动或删除）。"] = "Cannot open: the kernel cannot find this deliverable in the session log (the file may have been moved or deleted).",
        ["无法打开：该文件没有经过校验的宿主路径（可能在工作区沙箱之外）。"] = "Cannot open: the file has no validated host path (it may be outside the workspace sandbox).",
        ["无法打开交付物（HTTP {0}）：{1}"] = "Cannot open deliverable (HTTP {0}): {1}",
        ["无法打开交付物：{0}"] = "Cannot open deliverable: {0}",
        ["应用清单不可用（HTTP {0}）"] = "App manifest unavailable (HTTP {0})",
        ["内核未探测到可用应用"] = "The kernel detected no available apps",
        ["应用清单读取失败：{0}"] = "Failed to read app manifest: {0}",
        ["文件资源管理器"] = "File Explorer",
        ["Windows 终端"] = "Windows Terminal",
        ["命令提示符"] = "Command Prompt",
        ["在应用中打开失败（HTTP {0}）：{1}"] = "Failed to open in app (HTTP {0}): {1}",
        ["在应用中打开失败：{0}"] = "Failed to open in app: {0}",
        ["导入皮肤失败：{0}"] = "Failed to import skin: {0}",
        ["应用皮肤失败：{0}"] = "Failed to apply skin: {0}",
        ["读取设置失败：内核未返回设置描述。"] = "Failed to read settings: the kernel returned no settings description.",
        ["该分区加载失败：{0}"] = "This section failed to load: {0}",
        ["清空本页用户设置：{0}。"] = "Clear user settings on this page: {0}.",
        ["打开设置文档失败：{0}"] = "Failed to open settings document: {0}",
        ["恢复「{0}」默认"] = "Restore \"{0}\" defaults",
        ["将清空这些命名空间的用户设置：{0}。"] = "This clears the user settings of these namespaces: {0}.",
        ["内核默认值会立即生效，此操作不可撤销。"] = "Kernel defaults take effect immediately; this action cannot be undone.",
        ["恢复"] = "Restore",
        ["读取提供方目录失败：{0}"] = "Failed to read provider catalog: {0}",
        ["读取可配置提供方失败：{0}"] = "Failed to read configurable providers: {0}",
        ["{0} 未声明 apiKeyEnv（无需密钥）"] = "{0} declares no apiKeyEnv (no key required)",
        ["{0} · {1}（密钥缺失）"] = "{0} · {1} (key missing)",
        [" 等 {0} 个"] = " and {0} more",
        ["{0}{1}：内核目录已声明但尚未配置，在设置里补齐对应条目后才会出现密钥行。"] = "{0}{1}: declared in the kernel catalog but not configured. The key row appears after you complete the entries in Settings.",
        ["{0}（当前值）"] = "{0} (current value)",
        ["确定清除凭据 {0} 吗？清除后使用该提供方模型的会话会立即失败，直到重新填入。"] = "Clear credential {0}? Sessions using this provider's models will fail immediately until the key is filled in again.",
        ["（该命名空间未注册模型发现能力）"] = "(this namespace registers no model discovery)",
        ["发现模型失败{0}：{1}"] = "Model discovery failed{0}: {1}",
        ["上下文 {0:N0}"] = "Context {0:N0}",
        ["输出上限 {0:N0}"] = "Max output {0:N0}",
        ["提供方 {0} 未返回任何模型。"] = "Provider {0} returned no models.",
        ["{0} 声明 {1} 个模型（选择后填入默认模型）："] = "{0} declares {1} models (select one to fill the default model):",
        ["内核要求开启时至少有一个允许模型，首次开启会自动写入当前默认模型（{0} / {1}）。"] = "The kernel requires at least one allowed model when enabled; the first enable writes the current default model ({0} / {1}).",
        ["提供方默认"] = "Provider default",
        ["插件清单不可用：{0}"] = "Plugin manifest unavailable: {0}",
        ["共 {0} 个 Loader 条目（active {1}，failed {2}，未启用 {3}）。"] = "{0} loader entries total (active {1}, failed {2}, disabled {3}).",
        ["（无 fiber）"] = "(no fiber)",
        ["Loader 条目 {0} 个"] = "{0} loader entries",
        ["匹配 “{0}”：{1} / {2} 个"] = "Matching \"{0}\": {1} / {2}",
        ["已启用"] = "Enabled",
        ["已禁用"] = "Disabled",
        ["{0}（{1} 行）"] = "{0} ({1} rows)",
        ["无法加载 Agent 预设：{0}"] = "Failed to load agent presets: {0}",
        ["{0}（加载失败）"] = "{0} (load failed)",
        ["设为默认失败：{0}"] = "Failed to set default: {0}",
        ["读取预设失败：{0}"] = "Failed to read preset: {0}",
        ["{0} 副本"] = "{0} copy",
        ["复制预设（来源：{0}）"] = "Copy preset (from: {0})",
        ["新预设 id 不能为空。"] = "The new preset id cannot be empty.",
        ["复制失败：{0}"] = "Copy failed: {0}",
        ["确定删除自定义预设「{0}」（id: {1}）吗？该预设的目录会被移除，此操作不可撤销。"] = "Delete custom preset \"{0}\" (id: {1})? Its directory will be removed; this action cannot be undone.",
        ["请先在左侧选择一个会话：预设切换作用在会话的 agent 上。"] = "Select a session in the left pane first: preset switching applies to the session's agent.",
        ["会话已切换到预设「{0}」（内核回执：{1}）。"] = "Session switched to preset \"{0}\" (kernel receipt: {1}).",
        ["切换预设失败：{0}"] = "Failed to switch preset: {0}",
        ["内核无法直接打开目录，路径：{0}"] = "The kernel cannot open the directory directly; path: {0}",
        ["打开预设目录失败：{0}"] = "Failed to open preset directory: {0}",
        ["{0}：有空值或无效值，请修正后重试"] = "{0}: contains empty or invalid values; fix them and retry",
        ["{0}：写入失败，请检查连接和字段值后重试"] = "{0}: write failed; check the connection and field values, then retry",
        ["{0}：密钥保存失败，请检查连接和凭据来源是否可写后重试"] = "{0}: key save failed; check the connection and whether the credential source is writable, then retry",
        ["未保存的草稿已保留（仅当前窗口内）。{0}"] = "Unsaved drafts are kept (current window only). {0}",
        ["读取模型目录失败：{0}"] = "Failed to read model catalog: {0}",
        ["选择模型"] = "Select model",
        ["低"] = "Low",
        ["高"] = "High",
        ["最大"] = "Max",
        ["内核全文检索无匹配：{0}"] = "Kernel full-text search returned no match: {0}",
        ["内核未开启会话全文检索，已按标题匹配（{0} 条）。"] = "Kernel session full-text search is off; matched by title ({0} results).",
        ["内核未开启会话全文检索（session-query 索引 openAt=never），已回退为标题匹配。"] = "Kernel session full-text search is off (session-query index openAt=never); fell back to title matching.",
        ["如需全文检索，请在部署的 cordis.patch.yml 里把 session-query-sqlite 的 openAt 改为 first-search。"] = "To enable full-text search, set session-query-sqlite's openAt to first-search in the deployment's cordis.patch.yml.",
        ["检索失败（{0}），已回退为标题匹配（{1} 条）。"] = "Search failed ({0}); fell back to title matching ({1} results).",
        ["本地标题匹配"] = "Local title match",
        ["没有匹配「{0}」的会话。"] = "No sessions match \"{0}\".",
        ["内核未返回会话清单（session/list），使用统计不可用。"] = "The kernel returned no session list (session/list); usage statistics unavailable.",
        ["；{0} 个超长会话触到分页上限，其数据为部分计入"] = "; {0} very long sessions hit the pagination cap and are partially counted",
        ["，跳过 {0} 个空会话"] = ", {0} empty sessions skipped",
        ["读取使用统计失败：{0}"] = "Failed to read usage statistics: {0}",
        ["{0:N0} tokens（{1} 条用量记录）"] = "{0:N0} tokens ({1} usage records)",
        ["内核未返回任何 token 用量记录"] = "The kernel returned no token usage records",
        ["单日峰值：{0}，{1:N0} tokens"] = "Daily peak: {0}, {1:N0} tokens",
        ["单会话对话累计时长最大值：{0}（按 turn/start→turn/end 求和）"] = "Longest per-session conversation time: {0} (summed over turn/start→turn/end)",
        ["内核 journal 未包含任何完整 turn（turn/start → turn/end），无法计算"] = "The kernel journal contains no complete turns (turn/start → turn/end); cannot compute",
        ["{0} 天"] = "{0} days",
        ["{0} 条用量记录"] = "{0} usage records",
        ["单日峰值 · {0}"] = "Peak day · {0}",
        ["{0} 个会话中的最大值"] = "Max across {0} sessions",
        ["截至今日"] = "As of today",
        ["共活跃 {0} 天"] = "{0} active days in total",
        ["暂无用量记录"] = "No usage records",
        ["无完整 turn 记录"] = "No complete turns",
        ["今日暂无记录"] = "No records today",
        ["暂无活跃记录"] = "No active days",
        ["数据来源：内核 journal（session/list + session/page）的用量记录；已聚合 {0} 个非空会话、{1} 条记录{2}{3}。"] = "Data source: kernel journal (session/list + session/page) usage records; aggregated {0} non-empty sessions and {1} records{2}{3}.",
        ["「累计/峰值/连续天数」按全部历史，「时间范围」只作用于趋势图与模型用量；"] = "Totals/peaks/streaks cover all history; the time range only affects the trend chart and model usage;",
        ["费用/金额内核未提供对应字段，故不展示。"] = "Cost/amount fields are not provided by the kernel, so they are not shown.",
        ["所选时间范围内暂无用量记录"] = "No usage records in the selected range",
        ["{0}月"] = "{0}",
        ["{0}月{1}日"] = "{0}/{1}",
        ["每日 Token 趋势图：近 {0} 天暂无用量"] = "Daily token trend: no usage in the last {0} days",
        ["每日 Token 趋势图：近 {0} 天共 {1} tokens，{2} 个模型序列"] = "Daily token trend: {1} tokens over the last {0} days, {2} model series",
        ["模型用量：近 {0} 天共 {1} tokens，{2} 个模型"] = "Model usage: {1} tokens over the last {0} days, {2} models",
        ["合计 {0} tokens"] = "Total {0} tokens",
        ["占比 {0}%"] = "{0}% of total",
        ["{0:0.#} 亿"] = "{0:0.#} 亿",
        ["{0:0.#} 万"] = "{0:0.#} 万",
        ["{0} 小时 {1} 分"] = "{0}h {1}m",
        ["{0} 分 {1} 秒"] = "{0}m {1}s",
        ["{0} 秒"] = "{0}s",
        ["[图片]"] = "[Image]",
        ["[文件 {0}]"] = "[File {0}]",
        ["排队中 · {0} 条"] = "Queued · {0} items",
        ["作业 · {0} 个"] = "Jobs · {0}",
        ["{0} 个运行中"] = "{0} running",
        ["计划模式（切换中…）"] = "Plan mode (switching…)",
        ["计划模式已开启"] = "Plan mode is on",
        ["访问模式，当前：{0}"] = "Access mode, currently: {0}",
        ["只读：可浏览工作区，不能写文件或执行修改"] = "Read-only: browse the workspace, no file writes or modifications",
        ["可写工作区与允许的临时目录；更广范围的重试需审批"] = "Workspace and allowed temp dirs writable; broader retries need approval",
        ["完全文件访问，不再弹出审批确认"] = "Full file access; no approval prompts",
        ["Flash 预判写入/命令是否不可回补：安全自动批准，有风险转人工审批"] = "Flash pre-judges whether writes/commands are irreversible: safe ones auto-approve, risky ones go to human approval",
        ["需要确认"] = "Requires confirmation",
        ["直接执行"] = "Execute directly",
        ["审批策略已从「{0}」切换为「{1}」（由你更改）"] = "Approval policy switched from \"{0}\" to \"{1}\" (by you)",
        ["设置默认权限模式失败：{0}"] = "Failed to set default permission mode: {0}",
        ["记录失败：该会话在内核里已不存在（session-not-found）。"] = "Record failed: the session no longer exists in the kernel (session-not-found).",
        ["记录失败：内核返回 {0}。"] = "Record failed: the kernel returned {0}.",
        ["记录失败：[{0}] {1}"] = "Record failed: [{0}] {1}",
        ["会话反馈失败：{0}"] = "Session feedback failed: {0}",
        ["(空)"] = "(empty)",
        ["队列操作失败：{0}"] = "Queue operation failed: {0}",
        ["有帮助（再点一次撤销）"] = "Helpful (click again to revoke)",
        ["没帮助（再点一次撤销）"] = "Not helpful (click again to revoke)",
        ["添加说明"] = "Add note",
        ["撤销"] = "Revoke",
        ["说明：{0}"] = "Note: {0}",
        ["编辑说明"] = "Edit note",
        ["该消息不在本会话日志里（子代理或已裁剪），无法反馈"] = "This message is not in the session log (sub-agent or trimmed); feedback unavailable",
        ["会话不存在或已归档"] = "Session does not exist or is archived",
        ["说明不能是空白"] = "The note cannot be blank",
        ["说明超长（上限 {0} 字节）"] = "Note too long (limit {0} bytes)",
        ["说明超长"] = "Note too long",
        ["反馈已被其他地方改动，请重试"] = "Feedback changed elsewhere; please retry",
        ["操作失败"] = "Operation failed",
        ["失败（{0}）"] = "Failed ({0})",
        ["目录"] = "Directory",
        ["文件引用不可用（{0}）"] = "File reference unavailable ({0})",
        ["会话 · 同工作区"] = "Session · same workspace",
        ["会话"] = "Session",
        ["会话引用不可用（{0}）"] = "Session reference unavailable ({0})",
        ["引用不可用：{0}"] = "Reference unavailable: {0}",
        ["引用 {0} 条（@ 后输入可过滤；↑↓ 选择，Enter 插入，Esc 关闭）"] = "{0} references (@ to filter; ↑↓ select, Enter insert, Esc close)",
        ["“@{0}” 匹配 {1} 条（↑↓ 选择，Enter 插入，Esc 关闭）"] = "\"@{0}\" matched {1} (↑↓ select, Enter insert, Esc close)",
        ["命令目录不可用：{0}"] = "Command catalog unavailable: {0}",
        ["“/{0}” 匹配 {1} 条（↑↓ 选择，Enter 执行，Esc 关闭）"] = "\"/{0}\" matched {1} (↑↓ select, Enter run, Esc close)",
        ["附件上传失败，命令未提交；请重试。"] = "Attachment upload failed; the command was not submitted. Please retry.",
        ["命令失败：{0}"] = "Command failed: {0}",
        ["内核拒绝"] = "rejected by the kernel",
        ["未知或格式不正确的命令：{0}"] = "Unknown or malformed command: {0}",
        ["{0} 已提交（内核未返回即时应答）"] = "{0} submitted (no immediate kernel response)",
        ["{0} 执行完成"] = "{0} completed",
        ["{0} 执行失败"] = "{0} failed",
        ["请先连接内核并选择会话。"] = "Connect and select a session first.",
        ["会话已切换，请关闭后重新打开。"] = "Session changed. Close and reopen this panel.",
        ["尚未设置目标"] = "Not set",
        ["已启动轮数："] = "Rounds started: ",
        ["目标内容"] = "Objective",
        ["最大轮数（创建时可留空使用内核默认值）"] = "Maximum rounds (optional on creation)",
        ["创建或恢复目标可能启动内核执行；仅在确认目标后操作。"] = "Creating or resuming a goal may start kernel execution. Act only after confirming the objective.",
        ["目标"] = "Goal",
        ["目标内容不能为空。"] = "Objective cannot be empty.",
        ["最大轮数必须为正整数。"] = "Maximum rounds must be a positive integer.",
        ["请先刷新目标。"] = "Refresh the goal first.",
        ["重新读取失败："] = "Refresh failed: ",
        ["计划仅支持只读。当前主窗口未缓存 session/control 的 schedule 投影，无法显示计划列表；这不表示会话没有计划。\n此入口不创建、编辑或删除计划，也不调用 schedules RPC。"] = "Schedules are read-only. The main window does not currently cache the session/control schedule projection, so the list is unavailable; this does not mean the session has no schedules.\nNo schedules are created, edited or deleted; no schedules RPC is called.",
        ["计划（只读）"] = "Schedules (read-only)",
        ["技能列表响应格式不正确。"] = "Unexpected skills/list response.",
        ["适用场景：{0}"] = "When to use: {0}",
        // 设置分区「技能」：枚举/添加/删除/开关，见 MainWindow.Skills.cs。
        ["技能名称"] = "Skill name",
        ["技能说明"] = "Skill description",
        ["已安装技能"] = "Installed skills",
        ["添加技能"] = "Add skill",
        ["创建"] = "Create",
        ["删除"] = "Delete",
        ["取消"] = "Cancel",
        ["开"] = "On",
        ["关"] = "Off",
        ["名称"] = "Name",
        ["说明（必填）"] = "Description (required)",
        ["适用场景（可选）"] = "When to use (optional)",
        ["指令正文（可选）"] = "Instructions (optional)",
        ["安装到"] = "Install into",
        ["安装到哪个根目录"] = "Which root directory to install into",
        ["kebab-case，例如 pdf-report"] = "kebab-case, e.g. pdf-report",
        ["一句话说明这个技能做什么；调用时会显示它"] = "One line on what this skill does; shown in the skill menu",
        ["什么时候该用它，例如「生成周报时」"] = "When it should be used, e.g. \"when writing a weekly report\"",
        ["模型加载这个技能后读到的具体指令"] = "The instructions the model reads after loading this skill",
        ["名称不能为空。"] = "The name cannot be empty.",
        ["名称只能用小写字母、数字和短横线（kebab-case）。"] = "The name may contain only lowercase letters, digits and hyphens (kebab-case).",
        ["说明不能为空：调用菜单里只显示名称和说明。"] = "The description cannot be empty: the skill menu shows only the name and description.",
        ["已有同名技能：内核只会加载优先级最高的那个，请先处理已有的。"] = "A skill with this name already exists: the kernel only loads the highest-priority one; deal with the existing one first.",
        ["Blade² 已安装的全部技能。技能是带说明的指令包：在输入框键入 /名称 即可调用，模型也可能在合适的时机自行调用。"] =
            "Every skill installed in Blade². A skill is a documented instruction bundle: type /name in the input box to invoke it, and the model may also call it on its own at a suitable moment.",
        ["按内核 @deepseek-ai/dsh-skill-filesystem 的根目录与优先级扫描，与内核看到的是同一份真相。"] =
            "Scanned across the same roots and priorities as the kernel's @deepseek-ai/dsh-skill-filesystem, so this is the same truth the kernel sees.",
        ["同名技能只生效优先级最高的一个：项目 > 预设 > 用户；被遮蔽的条目在列表里标注。"] =
            "Only the highest-priority skill of a given name takes effect: project > preset > user. Shadowed entries are marked in the list.",
        ["还没有安装任何技能。用上面的「添加技能」写第一个，或在项目的 .dsh/skills 目录里放一个。"] =
            "No skills installed yet. Write the first one with \"Add skill\" above, or drop one into the project's .dsh/skills directory.",
        ["项目技能需要先选中一个会话：它们按会话所在的项目目录解析。"] =
            "Project skills need a session selected first: they are resolved from the session's project directory.",
        ["（这个目录里还没有技能）"] = "(no skills in this directory yet)",
        ["（本部署没有预设自带技能）"] = "(no preset ships skills in this deployment)",
        ["（已被遮蔽，不生效）"] = "(shadowed; not in effect)",
        ["模型不可自行调用（仅你能调用）"] = "The model cannot call it on its own (you only)",
        ["共 {0} 个技能：{1}"] = "{0} skills: {1}",
        ["{0} {1} 个"] = "{0} {1}",
        ["{0}（{1}）"] = "{0} ({1})",
        ["项目"] = "Project",
        ["项目（兼容）"] = "Project (legacy)",
        ["预设"] = "Preset",
        ["用户"] = "User",
        ["用户（兼容）"] = "User (legacy)",
        ["扫描技能目录失败：{0}"] = "Failed to scan skill directories: {0}",
        ["启用技能 {0}"] = "Enable skill {0}",
        ["删除技能 {0}"] = "Delete skill {0}",
        ["删除 {0}"] = "Delete {0}",
        ["删除「{0}」？"] = "Delete \"{0}\"?",
        ["将删除这个技能文件，内核随即不再加载它。此操作不可撤销。"] =
            "This deletes the skill file, and the kernel stops loading it immediately. This cannot be undone.",
        ["这个技能随预设分发，不能在这里删除。"] = "This skill ships with a preset and cannot be deleted here.",
        ["这个技能随预设分发，不能在这里关闭。"] = "This skill ships with a preset and cannot be disabled here.",
        ["这个文件没有 frontmatter，无法改写它的开关。"] = "This file has no frontmatter, so its switches cannot be rewritten.",
        ["切换技能开关失败：{0}"] = "Failed to toggle the skill: {0}",
        ["「{0}」已开启。"] = "\"{0}\" is enabled.",
        ["「{0}」已关闭：/ 菜单与模型都不会再看到它。"] = "\"{0}\" is disabled: neither the / menu nor the model will see it.",
        ["创建技能失败：{0}"] = "Failed to create the skill: {0}",
        ["选择技能只会插入 /名称 到输入框，不会发送或执行。"] = "Selecting a skill only inserts /name into the input box; it does not send or execute it.",
        ["技能"] = "Skills",
        ["没有可用技能。"] = "No skills available.",
        ["创建目标"] = "Create goal",
        ["保存编辑"] = "Save edits",
        ["暂停"] = "Pause",
        ["标记完成"] = "Mark complete",
        ["未选择会话"] = "No session selected",
        ["在左侧选择一个会话后，这里显示该会话工作区的文件树。"] = "Select a session on the left to see its workspace file tree here.",
        ["读取工作区失败"] = "Failed to read workspace",
        ["目录项超过内核上限，仅显示前 {0} 项。"] = "The directory exceeds the kernel cap; showing the first {0} entries only.",
        ["列目录失败（{0}）"] = "Failed to list directory ({0})",
        ["（空目录）"] = "(empty directory)",
        ["刷新失败"] = "Refresh failed",
        ["正在读取…"] = "Reading…",
        ["{0} · {1} 行 · 版本 {2}"] = "{0} · {1} lines · version {2}",
        ["文件较长，仅显示前 {0} 行（内核单页上限 {1} 行 / 2 MiB）。"] = "The file is long; showing the first {0} lines only (kernel page limit {1} lines / 2 MiB).",
        ["不支持预览"] = "Preview not supported",
        ["该文件是二进制或非 UTF-8 文本（含 NUL 字节），壳内只预览文本。"] = "The file is binary or non-UTF-8 text (contains NUL bytes); the shell previews text only.",
        ["文件过大"] = "File too large",
        ["超出内核单页读取上限（2 MiB / 5000 行），无法在这里完整预览。"] = "Exceeds the kernel single-page read limit (2 MiB / 5000 lines); cannot preview it fully here.",
        ["文件不存在"] = "File not found",
        ["路径已消失（可能被移动或删除）。"] = "The path is gone (it may have been moved or deleted).",
        ["该路径不是普通文件。"] = "The path is not a regular file.",
        ["超出工作区"] = "Outside workspace",
        ["该路径不在当前会话的工作区内。"] = "The path is not inside the current session's workspace.",
        ["会话不可用"] = "Session unavailable",
        ["该会话没有活跃 agent，内核无法解析工作区。"] = "The session has no active agent, so the kernel cannot resolve the workspace.",
        ["读取失败"] = "Read failed",
        ["返回文件树"] = "Back to file tree",
        ["刷新文件树"] = "Refresh file tree",
        ["关闭文件面板"] = "Close file panel",
        ["路径不存在（可能已被移动或删除）。"] = "The path does not exist (it may have been moved or deleted).",
        ["该路径不是目录。"] = "The path is not a directory.",
        ["路径超出该会话工作区。"] = "The path is outside this session's workspace.",
        ["内容超出内核单页上限。"] = "The content exceeds the kernel page limit.",
        ["不是 UTF-8 文本，无法预览。"] = "Not UTF-8 text; cannot preview.",
        ["该会话没有活跃 agent（会话未打开或已归档）。"] = "The session has no active agent (not opened or archived).",
        ["大小未知"] = "Size unknown",
        ["交付物"] = "Deliverable",
        ["打开"] = "Open",
        ["显示"] = "Reveal",
        ["对话消息列表"] = "Conversation messages",
        ["开"] = "On",
        ["关"] = "Off",
        // ---- 三轮补译：Agent 模式四档与统计提示 ----
        ["标准模式"] = "Standard mode",
        ["PTC 模式"] = "PTC mode",
        ["极简模式"] = "Minimal mode",
        ["创造模式"] = "Creative mode",
        ["默认预设"] = "Default preset",
        ["减小字号"] = "Decrease font size",
        ["增大字号"] = "Increase font size",
        ["浏览文件夹"] = "Browse folders",
        // 队列/任务状态标签（PlacementLabel/StatusLabel 显示点已包 L）
        ["上下文"] = "Context",
        ["排队"] = "Queued",
        ["运行中"] = "Running",
        ["停止中"] = "Stopping",
        ["已完成"] = "Completed",
        ["已终止"] = "Terminated",
        ["失败"] = "Failed",
        // 命令面板内置命令描述（LocalizeHostCommand 的中文侧键）
        ["压缩以上对话内容"] = "Compress earlier conversation history",
        ["将当前会话内容导出为 ZIP"] = "Export this session as a ZIP archive",
        ["发送关于当前会话的反馈"] = "Send feedback about this session",
        ["设置或查看长期任务目标"] = "Set or view the goal for a long-running task",
        ["切换权限预设（沙箱模式与审批策略）"] = "Switch the permission preset (sandbox mode and approval policy)",
        ["进入或退出计划模式"] = "Enter or leave plan mode",
        // 内核内置预设描述（agentPresets/list 的 description 原文，逐字精确匹配才翻）
        ["功能完整的编码 Agent，支持文件编辑、Shell、文件与网页检索、Skills、计划、目标、子代理和工作流。"] =
            "A full-featured coding agent: file editing, shell, file and web search, skills, plans, goals, subagents and workflows.",
        ["功能完整的编码 Agent，但默认不提供 workflow 工具；其他工具通过 PTC 模式 SDK 呈现，让模型用一个 TypeScript 程序组合多步操作。"] =
            "A full-featured coding agent without the workflow tool by default; other tools surface through the PTC-mode SDK so the model composes multi-step work in one TypeScript program.",
        ["仅提供持久 shell 的单工具编码 Agent。"] =
            "A single-tool coding agent that only provides a persistent shell.",
        ["用于创建自定义 Agent preset：具备标准模式的全部能力，并提供运行时检查、插件实验和 preset 创作指导。"] =
            "For authoring custom agent presets: full standard-mode capabilities plus runtime inspection, plugin experimentation and preset-authoring guidance.",
        // ---- 四轮补译：动态拼接的 UIA Name 模板键 ----
        ["会话操作：{0}"] = "Session actions: {0}",
        ["模型与推理等级，当前 {0}"] = "Model and reasoning effort, currently {0}",
        ["外观：{0}"] = "Appearance: {0}",
        ["当前字号 {0}px"] = "Current font size {0}px",
        ["{0} API 密钥"] = "{0} API key",
        ["清除 {0} 的 API 密钥"] = "Clear the API key for {0}",
        ["发现 {0} 的模型"] = "Discover models for {0}",
        ["复制预设 {0}"] = "Copy preset {0}",
        ["当前会话使用预设 {0}"] = "Use preset {0} for this session",
        ["设为默认预设 {0}"] = "Set {0} as default preset",
        ["打开预设 {0} 的目录"] = "Open the directory of preset {0}",
        ["删除预设 {0}"] = "Delete preset {0}",
        ["{0} 曲线"] = "{0} curve",
        ["访问模式：{0}"] = "Access mode: {0}",
        ["访问模式：{0}。{1}"] = "Access mode: {0}. {1}",
        ["分组方式：{0}"] = "Group by: {0}",
        ["排序方式：{0}"] = "Sort by: {0}",
        ["打开工作区路径：{0}"] = "Open workspace path: {0}",
        ["重命名工作区：{0}"] = "Rename workspace: {0}",
        ["上移：{0}"] = "Move up: {0}",
        ["下移：{0}"] = "Move down: {0}",
        ["删除工作区：{0}"] = "Delete workspace: {0}",
        ["移动到工作区：{0}"] = "Move to workspace: {0}",
        ["跳转到 {0}"] = "Navigate to {0}",
        ["交付物 seq={0}：{1}"] = "Deliverable seq={0}: {1}",
        ["移除附件 {0}"] = "Remove attachment {0}",
        ["选项：{0}"] = "Option: {0}",
        ["预设 {0} 的内容"] = "Content of preset {0}",
        ["模型：{0}"] = "Model: {0}",
        ["推理等级：{0}"] = "Reasoning effort: {0}",
        ["收起已展开的会话"] = "Collapse expanded sessions",
        ["编辑排队项"] = "Edit queued item",
        ["删除排队项"] = "Delete queued item",
        ["排队项内容"] = "Queued item content",
        // ---- 设置分区「宠物」（默认插件 · dsh-pet，Codex Pet 兼容） ----
        ["桌面宠物由内核插件 @linxin666/dsh-pet 提供（Codex Pet 兼容）：聊天窗口里常驻一只宠物，模型干活时它跟着动，点它可以逗一逗。宠物素材装在 $DSH_HOME/pets，重启内核后收录。"] =
            "The desktop pet is provided by the kernel plugin @linxin666/dsh-pet (Codex Pet compatible): a pet lives in the chat window and reacts while the model works; click it to play. Pet assets live in $DSH_HOME/pets and are picked up after a kernel restart.",
        ["宠物"] = "Pets",
        ["显示"] = "Display",
        ["宠物显示在聊天窗口右下角，跟着模型的工作状态切换动画；点它可以逗一逗。"] =
            "The pet sits in the bottom-right of the chat window and switches animations with the model's activity; click it to play.",
        ["显示宠物"] = "Show the pet",
        ["关掉后窗口里不再显示，插件其余功能照常。"] =
            "When off, it no longer shows in the window; the rest of the plugin keeps working.",
        ["开"] = "On",
        ["关"] = "Off",
        ["宠物大小"] = "Pet size",
        ["单元格高度的像素值，拖动即时生效。"] =
            "The cell height in pixels; changes apply while dragging.",
        ["{0} px"] = "{0} px",
        ["宠物插件还没就位：安装包缺失或内核还没加载它。装过宠物后重启一次 Blade² 即可；引导器下次启动会自动重试安装。"] =
            "The pet plugin is not ready yet: the package is missing or the kernel has not loaded it. Install a pet and restart Blade² once; the bootstrapper retries the install on the next launch.",
        ["正在读取宠物清单…"] = "Reading the pet list…",
        ["一只宠物都没有。装一个：把 Codex 宠物 zip 拖到下面的安装区，或粘贴 petdex install <宠物标识>。"] =
            "No pets yet. Install one: drop a Codex pet zip onto the install area below, or paste petdex install <pet-id>.",
        ["宠物库"] = "Pet library",
        ["已收录的宠物（插件内置 + ~/.codex/pets + 本机安装的）。切换立即生效。"] =
            "Pets on record (plugin built-ins + ~/.codex/pets + locally installed). Switching applies immediately.",
        ["使用中"] = "In use",
        ["使用"] = "Use",
        ["删除"] = "Delete",
        ["本机安装"] = "Installed locally",
        ["Codex 宠物"] = "Codex pet",
        ["插件内置"] = "Plugin built-in",
        ["Live2D（壳内不渲染）"] = "Live2D (not rendered by the shell)",
        ["使用宠物 {0}"] = "Use pet {0}",
        ["删除宠物 {0}"] = "Delete pet {0}",
        ["删除宠物「{0}」？"] = "Delete pet \"{0}\"?",
        ["只删除本机安装目录里的文件。插件内置与 Codex 目录里的宠物不受影响；删除后重启 Blade² 生效。"] =
            "Only the locally installed directory is deleted. Plugin built-ins and pets in the Codex directory are untouched; the deletion takes effect after restarting Blade².",
        ["取消"] = "Cancel",
        ["安装宠物"] = "Install a pet",
        ["兼容 Codex 宠物包（pet.json + 图集）。拖放 zip 到下方区域，或粘贴安装命令。"] =
            "Compatible with Codex pet packages (pet.json + spritesheet). Drop a zip onto the area below, or paste an install command.",
        ["拖放以安装"] = "Drop to install",
        ["把 Codex 宠物 zip 从文件资源管理器拖到这里即可安装。"] =
            "Drag a Codex pet zip here from File Explorer to install it.",
        ["浏览并安装"] = "Browse and install",
        ["浏览并安装宠物压缩包"] = "Browse for a pet archive and install it",
        ["petdex install <宠物标识>"] = "petdex install <pet-slug>",
        ["宠物安装命令行"] = "Pet install command line",
        ["安装"] = "Install",
        ["按命令行安装宠物"] = "Install a pet from the command line",
        ["正在安装 {0}…"] = "Installing {0}…",
        ["正在解析并安装…"] = "Parsing and installing…",
        ["已安装「{0}」到 {1}。重启 Blade² 后出现在宠物库里。"] =
            "Installed \"{0}\" to {1}. It appears in the pet library after restarting Blade².",
        ["只支持 Codex 宠物 zip（里面有 pet.json 和图集）。"] =
            "Only Codex pet zips (containing pet.json and a spritesheet) are supported.",
        ["安装失败：{0}"] = "Install failed: {0}",
        ["选择文件失败：{0}"] = "Picking the file failed: {0}",
        ["先粘贴一条安装命令，例如 petdex install whale-girl。"] =
            "Paste an install command first, for example petdex install whale-girl.",
        ["诊断"] = "Diagnostics",
        ["宠物不显示时先看这里：插件在不在、注册表报了什么、壳能不能画。"] =
            "Start here when the pet does not show: whether the plugin is present, what the registry reports, and whether the shell can draw it.",
        ["宠物插件（@linxin666/dsh-pet）未安装。"] = "The pet plugin (@linxin666/dsh-pet) is not installed.",
        ["宠物插件已安装并选入内核 bundle。"] = "The pet plugin is installed and selected into the kernel bundle.",
        ["宠物插件已安装但没选进 package.json 的 dsh.profile.bundles，内核不会加载它。"] =
            "The pet plugin is installed but is not selected into dsh.profile.bundles in package.json, so the kernel will not load it.",
        ["正在读取注册表诊断…"] = "Reading registry diagnostics…",
        ["注册表没有报错。"] = "The registry reports no problems.",
        ["插件路由没有响应（/api/pet/* 404）：宠物功能整体不可用，重启 Blade² 让内核重新加载插件。"] =
            "The plugin routes do not respond (/api/pet/* 404): the pet feature is unavailable; restart Blade² so the kernel reloads the plugin.",
        ["若宠物显示不出来：Windows 需要 WebP 映像扩展才能解 .webp 图集，缺失时壳会退到预览 GIF，再不行就连精灵一起隐藏。"] =
            "If the pet does not render: Windows needs the WebP image extension to decode .webp atlases; without it the shell falls back to the preview GIF, and if that fails too the sprite is hidden.",
        // ---- 设置分区「记忆」（Blade² 默认插件 · 记忆服务器） ----
        ["记忆通过内核 dsh-mcp-client 挂载 MCP 参考记忆服务器（@modelcontextprotocol/server-memory），模型可跨会话写入与召回信息。开关编辑内核 profile patch 的 disabled 标志，由内核热重载即时生效。"] =
            "Memory is provided by the kernel's dsh-mcp-client mounting the MCP Reference Memory server (@modelcontextprotocol/server-memory), so the model can write and recall information across sessions. The toggle edits the disabled flag in the kernel profile patch; the kernel hot-reloads it on the fly.",
        ["持久记忆"] = "Persistent memory",
        ["启用持久记忆"] = "Enable persistent memory",
        ["关闭后模型不再写入或读取记忆；已有记忆文件保留不动。"] =
            "When off, the model no longer writes or reads memory; the existing memory file is left untouched.",
        ["已启用（内核热重载后生效）"] = "Enabled (applies via kernel hot reload)",
        ["已停用"] = "Disabled",
        ["记忆存储"] = "Memory storage",
        ["知识图谱 JSONL（由记忆服务器自身管理，删除即清空记忆）："] =
            "Knowledge-graph JSONL (managed by the memory server itself; deleting it clears all memory):",
        ["默认插件"] = "Bundled plugins",
        ["参考记忆服务器"] = "Reference memory server",
        ["自动审批"] = "Auto approval",
        ["技能面板"] = "Skill panel",
        ["Blade² 随内核插件机制默认启用；安装由引导器幂等完成，失败时下次启动自动重试。"] =
            "Enabled by default via the kernel plugin mechanism; the bootstrapper installs idempotently and retries on the next launch after a failure.",
        ["已启用"] = "Enabled",
        ["默认停用（需先安装 Cua Driver）"] = "Disabled by default (install Cua Driver first)",
        ["已安装，未挂载"] = "Installed, not mounted",
        ["未安装（离线？启动后自动重试）"] = "Not installed (offline? retried on next launch)",
        ["记忆内容"] = "Memory contents",
        ["直接编辑记忆文件（JSONL，每行一个实体或关系），停止输入后自动保存；模型写入时自动重新载入。"] =
            "Edit the memory file (JSONL, one entity or relation per line) directly; it auto-saves when you stop typing and reloads automatically when the model writes.",
        ["记忆文件内容"] = "Memory file contents",
        ["共 {0} 行 · {1}"] = "{0} lines · {1}",
        ["有未保存的修改…"] = "Unsaved changes…",
        ["已自动保存 · {0}"] = "Auto-saved · {0}",
        ["停止输入后自动保存"] = "Auto-saves when you stop typing",
        ["第 {0} 行不是合法 JSON，已阻止保存。"] = "Line {0} is not valid JSON; save blocked.",
        ["保存失败：{0}"] = "Save failed: {0}",
        // ---- 设置分区「电脑控制 / 浏览器控制」（默认插件 · 挂载条目开关） ----
        ["电脑控制"] = "Computer control",
        ["浏览器控制"] = "Browser control",
        ["电脑控制由内核 dsh-computer-use 注册表与 Cua Driver provider（@deepseek-ai/dsh-experimental-computer-use-cua-driver-mcp）提供：模型可截图并操作鼠标、键盘完成桌面任务。开关编辑内核 profile patch 的 disabled 标志，由内核热重载即时生效。"] =
            "Computer control is provided by the kernel's dsh-computer-use registry and the Cua Driver provider (@deepseek-ai/dsh-experimental-computer-use-cua-driver-mcp): the model can take screenshots and operate the mouse and keyboard for desktop tasks. The toggles edit the disabled flag in the kernel profile patch; the kernel hot-reloads it on the fly.",
        ["开启后模型获得桌面操作能力（截图、鼠标、键盘）；关闭后相关工具从模型视野移除。"] =
            "When on, the model gains desktop control (screenshots, mouse, keyboard); when off, the related tools are removed from the model's view.",
        ["Cua Driver（桌面驱动）"] = "Cua Driver (desktop driver)",
        ["电脑操作的执行驱动：需在本机安装 cua-driver 命令行工具后启用，默认停用。"] =
            "The driver that executes computer control: enable it after installing the cua-driver command-line tool locally. Disabled by default.",
        ["启用电脑控制"] = "Enable computer control",
        ["启用 Cua Driver 桌面驱动"] = "Enable the Cua Driver desktop driver",
        ["浏览器控制由内核 dsh-browser-use 注册表与 Playwright provider（@deepseek-ai/dsh-experimental-browser-use-playwright-mcp）提供：模型可打开浏览器完成网页任务。开关编辑内核 profile patch 的 disabled 标志，由内核热重载即时生效。"] =
            "Browser control is provided by the kernel's dsh-browser-use registry and the Playwright provider (@deepseek-ai/dsh-experimental-browser-use-playwright-mcp): the model can open a browser for web tasks. The toggles edit the disabled flag in the kernel profile patch; the kernel hot-reloads it on the fly.",
        ["开启后模型获得浏览器操作能力（打开网页、点击、填写、读取内容）；关闭后相关工具从模型视野移除。"] =
            "When on, the model gains browser control (open pages, click, fill in, read content); when off, the related tools are removed from the model's view.",
        ["Playwright（浏览器驱动）"] = "Playwright (browser driver)",
        ["浏览器操作的执行驱动：随包安装，launch 模式无头运行，默认启用。"] =
            "The driver that executes browser control: installed with the package, launched headless. Enabled by default.",
        ["启用浏览器控制"] = "Enable browser control",
        ["启用 Playwright 浏览器驱动"] = "Enable the Playwright browser driver",
        ["挂载状态"] = "Mount status",
        ["电脑控制注册表"] = "Computer-use registry",
        ["Cua Driver 驱动"] = "Cua Driver provider",
        ["浏览器控制注册表"] = "Browser-use registry",
        ["Playwright 驱动"] = "Playwright provider",
        // ---- 设置「插件」分区内的自动审批块（dsh-approval-gate · auto-approve 预设开关 + 判定模型） ----
        ["自动审批由内核插件 dsh-approval-gate 提供：当会话权限预设切到「自动审批」时，由判定模型预判每次写入/命令是否不可回补——安全则自动批准，涉及删除、凭据、系统配置等硬类别转人工确认。开关增删内核 profile patch 里的 auto-approve 预设，判定模型写在默认模型设置里，两者都由内核热重载即时生效。"] =
            "Auto approval is provided by the kernel plugin dsh-approval-gate: when a session's permission preset is switched to Auto approval, the judging model pre-judges whether each write/command is irreversible — safe ones are approved automatically, while hard categories such as deletion, credentials, and system configuration are escalated to a human. The toggle adds or removes the auto-approve preset in the kernel profile patch and the judging model lives in the default-model setting; both are hot-reloaded by the kernel on the fly.",
        ["开启后权限模式里多出「自动审批」：判定模型预判越界请求，安全自动批准、有风险转人工。"] =
            "When on, an Auto approval option appears in the permission presets: the judging model pre-judges out-of-bounds requests, approving safe ones automatically and escalating risky ones to a human.",
        ["启用自动审批"] = "Enable auto approval",
        ["自动审批判定模型"] = "Auto approval judging model",
        ["判定模型"] = "Judging model",
        ["自动审批每次预判写入/命令是否不可回补时所用的模型，选项来自内核模型目录。"] =
            "The model that pre-judges whether each write/command is irreversible during auto approval; the options come from the kernel model catalog.",
        ["预判模型"] = "Pre-judgment model",
        ["切换后新会话的默认模型也随之改变；已在会话里选过模型的会话不受影响。"] =            "Switching also changes the default model for new sessions; sessions that already picked a model are unaffected.",
        ["模型目录不可用：{0}"] = "Model catalog unavailable: {0}",
        ["切换后会发生什么"] = "What happens when you switch",
        ["开启：权限选择器（输入区左下角）里出现「自动审批」，新会话可选用它。\n" +
         "关闭：该预设从选择器消失；正在使用它的会话回落到自定义权限，审批恢复人工确认。\n" +
         "判定记录与学习状态不受影响，重新开启后继续生效。"] =
            "On: an Auto approval option appears in the permission picker (bottom-left of the composer), available to new sessions.\n" +
            "Off: that preset disappears from the picker; sessions using it fall back to custom permissions, and approval returns to manual confirmation.\n" +
            "Decision logs and learned state are unaffected and resume when you switch it back on.",
        // ---- 设置分区「关于」（版本与 GitHub Release 更新） ----
        ["关于"] = "About",
        ["Blade² 的版本信息与更新。更新通过覆盖安装新版本 MSIX 完成（安装前需先退出应用）。"] =
            "Blade² version info and updates. Updates are applied by installing a newer MSIX over the current one (quit the app before installing).",
        ["版本"] = "Version",
        ["当前版本"] = "Current version",
        ["壳版本（打包形态与安装包版本一致）"] = "Shell version (matches the package version when packaged)",
        ["内核版本"] = "Kernel version",
        ["随包发行的 dsh 内核版本"] = "Bundled dsh kernel version",
        ["更新"] = "Updates",
        ["更新源：{0}"] = "Update source: {0}",
        ["更新源尚未配置：发布到 GitHub 后在 MainWindow.About.cs 填入仓库地址即可启用在线检查更新。"] =
            "Update source not configured yet: fill in the repo address in MainWindow.About.cs after publishing to GitHub.",
        ["尚未配置更新源。"] = "Update source not configured yet.",
        ["检查更新"] = "Check for updates",
        ["正在检查更新…"] = "Checking for updates…",
        ["已是最新版本。"] = "You're up to date.",
        ["发现新版本：{0}。"] = "New version available: {0}.",
        ["发现新版本：{0}（约 {1}）。"] = "New version available: {0} (about {1}).",
        ["检查更新失败：{0}"] = "Update check failed: {0}",
        ["检查更新失败：Release 返回缺少 tag_name。"] = "Update check failed: the release response has no tag_name.",
        ["检查更新失败：未找到可用的安装包。"] = "Update check failed: no installable package found.",
        ["下载并安装"] = "Download and install",
        ["下载并安装 {0}"] = "Download and install {0}",
        ["打开 Release 页面"] = "Open the Releases page",
        ["正在下载更新包：{0}…"] = "Downloading update package: {0}…",
        ["已下载：{0}"] = "Downloaded: {0}",
        ["下载更新失败：{0}"] = "Update download failed: {0}",
        ["安装更新"] = "Install update",
        ["将退出 Blade²（含内置内核）并覆盖安装新版本。继续吗？"] =
            "Blade² (including the bundled kernel) will quit and the new version will be installed over it. Continue?",
        ["退出并安装"] = "Quit and install",
        ["稍后"] = "Later",
        ["GitHub Release"] = "GitHub Releases",
        ["未知"] = "Unknown",
        // ---- 通用分区「托盘与退出」（壳本地 shell.json） ----
        ["托盘与退出"] = "Tray & exit",
        ["最小化 / 关闭窗口时的去向"] = "Where the window goes when minimized or closed",
        ["显示托盘图标"] = "Show tray icon",
        ["关闭后仍可从托盘恢复窗口；托盘右键菜单可退出"] =
            "Restore the window from the tray after hiding; quit from the tray menu",
        ["最小化时隐藏到托盘"] = "Minimize to tray",
        ["点最小化按钮后窗口藏进托盘，不占任务栏"] =
            "Hides into the tray on minimize, freeing the taskbar",
        ["关闭时最小化到托盘"] = "Close to tray",
        ["点关闭按钮后窗口藏进托盘继续运行，用托盘菜单退出"] =
            "Keeps running in the tray on close; quit from the tray menu",
        // ---- 系统通知（shell.json showNotifications） ----
        ["系统通知"] = "System notifications",
        ["任务完成、审批请求等关键时刻的 Windows 通知"] =
            "Windows notifications for task completion, approval requests and other key moments",
        ["启用系统通知"] = "Enable system notifications",
        ["窗口不在前台时提醒；点击通知可回到 Blade²"] =
            "Notifies while the window is in the background; click a notification to bring Blade² back",
        ["任务完成"] = "Task completed",
        ["本轮对话已完成"] = "This turn has finished",
        ["需要你的输入"] = "Your input needed",
        // ---- 会话头 / 后台作业 / 子代理 / 目标条 / 计划（0.7.8 多任务与会话体系） ----
        ["后台作业"] = "Background jobs",
        ["{0} 个作业运行中"] = "{0} background jobs running",
        ["子代理"] = "Subagent",
        ["子代理会话为只读：可查看其工作过程，消息请在父会话中发送。"] =
            "Subagent sessions are read-only: you can watch the work, but send messages in the parent session.",
        ["返回父会话"] = "Back to parent session",
        ["目标进行中"] = "Goal active",
        ["目标待命"] = "Goal armed off",
        ["目标已暂停"] = "Goal paused",
        ["目标受阻"] = "Goal blocked",
        // 计划（schedule 投影只读清单）：频率标签 + 相对时间
        ["一次性"] = "One-time",
        ["每 {0} 天"] = "Every {0} days",
        ["每 {0} 小时"] = "Every {0} hours",
        ["每 {0} 分"] = "Every {0} minutes",
        ["每 {0} 秒"] = "Every {0} seconds",
        ["尚未收到本会话的计划投影（投影随会话活动到达），此时不表示没有计划。"] =
            "The schedule projection has not arrived yet (the kernel pushes it lazily); this does not mean there are no schedules.",
        ["还剩 {0}"] = "{0} left",
        ["已过期 {0}"] = "{0} overdue",
        ["{0}天"] = "{0}d",
        ["{0}小时"] = "{0}h",
        ["{0}分钟"] = "{0}m",
    };
    private string L(string key)
    {
        if (_shellLocale == "zh") return key;
        var table = LoadLocaleTable(_shellLocale);
        if (table.TryGetValue(key, out var translated)) return translated;
        return ShellEnglish.GetValueOrDefault(key, key);
    }

    /// <summary>插值文案：template 为含 {0}/{1} 占位符的中文原文（同时作字典键）；译文缺失回退中文再格式化。</summary>
    private string LF(string template, params object?[] args)
    {
        try { return string.Format(L(template), args); }
        catch (FormatException) { return template; }
    }

    // 壳本地化静态入口：供不在 RefreshShellLanguage 扫描范围内的视图（FilesPanel 等）使用。
    private static Func<string, string> ShellTranslateFunc = static s => s;
    internal static string TL(string key) => ShellTranslateFunc(key);
    internal static string TLF(string template, params object?[] args)
    {
        try { return string.Format(ShellTranslateFunc(template), args); }
        catch (FormatException) { return template; }
    }

    // 弱引用保留每个展示属性的原文，反复 zh/en 可逆；不写 TextBox.Text、Tag 或选项协议值。
    private readonly System.Runtime.CompilerServices.ConditionalWeakTable<DependencyObject,
        Dictionary<DependencyProperty, (string Key, string Last)>> _localizedProperties = new();
    private bool _localizingShell;
    private string? _lastRebuildLocale;
    // 只登记壳独占的区域；新加的数据控件默认不进入本地化扫描。
    private readonly List<DependencyObject> _generalShellRegions = new();

    private void LocalizeProperty(DependencyObject target, DependencyProperty property)
    {
        if (target.GetValue(property) is not string current) return;
        var entries = _localizedProperties.GetOrCreateValue(target);
        var key = entries.TryGetValue(property, out var prior) && prior.Last == current ? prior.Key : current;
        if (!ShellEnglish.ContainsKey(key)) return;
        var translated = L(key);
        entries[property] = (key, translated);
        if (current != translated) target.SetValue(property, translated);
    }

    private void LocalizeShellTree(DependencyObject node)
    {
        // 不扫描聊天正文、文件面板和提问载荷；它们可能包含与字典键相同的用户内容。
        if (ReferenceEquals(node, ChatList) || ReferenceEquals(node, FilesPanelView) ||
            ReferenceEquals(node, QuestionPanel) || ReferenceEquals(node, ApprovalReason) ||
            ReferenceEquals(node, ApprovalDetail) || node is NavigationViewItem { Tag: SessionVm or WorkspaceVm }) return;
        // 枚举标签可来自内核 schema / 提供方；只处理壳拥有固定选项的字段。
        if (node is ComboBox && AutomationProperties.GetAutomationId(node) is not
            ("Setting_ui-chat_transcriptView" or "Setting_ui-conversation_busyEnter" or "Setting_locale_preference"
             or "Setting_permission_defaultPreset" or "Setting_ui-theme_preference"))
        {
            // 白名单外的 ComboBox 仍翻自身 Name/HelpText（壳设的标签，如「服务商」）；只跳过选项与递归。
            LocalizeProperty(node, AutomationProperties.NameProperty);
            LocalizeProperty(node, AutomationProperties.HelpTextProperty);
            LocalizeProperty(node, ToolTipService.ToolTipProperty);
            return;
        }
        LocalizeProperty(node, AutomationProperties.NameProperty);
        LocalizeProperty(node, AutomationProperties.HelpTextProperty);
        LocalizeProperty(node, ToolTipService.ToolTipProperty);
        if (node is TextBox)
        {
            LocalizeProperty(node, TextBox.PlaceholderTextProperty);
            return; // 绝不递归到可编辑文本的模板
        }
        if (node is PasswordBox)
        {
            LocalizeProperty(node, PasswordBox.PlaceholderTextProperty);
            return;
        }
        if (node is AutoSuggestBox asb)
        {
            LocalizeProperty(node, AutoSuggestBox.PlaceholderTextProperty);
            // 模板内层 TextBox 在应用模板时复制了外层 Name，UIA 的 Edit 节点读它自己的本地值，需单独同步。
            if (FindFirstTextBox(asb) is { } innerBox)
                LocalizeProperty(innerBox, AutomationProperties.NameProperty);
            return;
        }
        if (node is TextBlock) LocalizeProperty(node, TextBlock.TextProperty);
        if (node is ContentControl) LocalizeProperty(node, ContentControl.ContentProperty);
        // ToggleSwitch 的 On/Off 标签不是 ContentControl 路径，也不在模板 TextBlock 里。
        if (node is ToggleSwitch tsw)
        {
            LocalizeProperty(tsw, ToggleSwitch.OnContentProperty);
            LocalizeProperty(tsw, ToggleSwitch.OffContentProperty);
        }
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(node); i++)
            LocalizeShellTree(VisualTreeHelper.GetChild(node, i));
        // 未展开的 ComboBox 选项不在视觉树里。
        if (node is ComboBox box)
            foreach (var item in box.Items.OfType<ComboBoxItem>())
                LocalizeProperty(item, ContentControl.ContentProperty);
        // SelectorBar（统计页口径/范围切换）条目的文案是自己的 DP，不走模板内 TextBlock。
        if (node is SelectorBar bar)
            foreach (var item in bar.Items.OfType<SelectorBarItem>())
                LocalizeProperty(item, SelectorBarItem.TextProperty);
    }

    private static TextBox? FindFirstTextBox(DependencyObject node)
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(node); i++)
        {
            var child = VisualTreeHelper.GetChild(node, i);
            if (child is TextBox tb) return tb;
            if (FindFirstTextBox(child) is { } found) return found;
        }
        return null;
    }

    /// <summary>翻译 MenuFlyout 声明项（打开才具现化，视觉树扫描到不了）。</summary>
    private void LocalizeMenuItems(IList<MenuFlyoutItemBase> items)
    {
        foreach (var item in items)
        {
            switch (item)
            {
                case MenuFlyoutItem mi:
                    LocalizeProperty(mi, MenuFlyoutItem.TextProperty);
                    break;
                case MenuFlyoutSubItem sub:
                    LocalizeProperty(sub, MenuFlyoutSubItem.TextProperty);
                    LocalizeMenuItems(sub.Items);
                    break;
            }
        }
    }

    /// <summary>ChatList 在可视树扫描排除区，容器回收时单独走这几处壳文案。</summary>
    private void LocalizeDeliverableCard(DependencyObject node)
    {
        if (node is TextBlock tb && AutomationProperties.GetAutomationId(tb) == "DeliverableCardTitle")
        {
            LocalizeProperty(tb, TextBlock.TextProperty);
        }
        else if (node is TextBlock pending && AutomationProperties.GetAutomationId(pending) == "PendingThinkingText")
        {
            LocalizeProperty(pending, TextBlock.TextProperty);
        }
        else if (node is Button btn &&
                 AutomationProperties.GetAutomationId(btn) is "PresentedFileOpenButton" or "PresentedFileRevealButton")
        {
            LocalizeProperty(btn, ContentControl.ContentProperty);
        }
        else
        {
            for (var i = 0; i < VisualTreeHelper.GetChildrenCount(node); i++)
                LocalizeDeliverableCard(VisualTreeHelper.GetChild(node, i));
        }
    }

    private void RefreshShellLanguage()
    {
        if (_localizingShell) return;
        _localizingShell = true;
        try
        {
            foreach (var region in new DependencyObject[] { ChatHero, KernelBootPanel, InputBox, SendButton, SearchBox,
                SearchIconButton, ShellBackButton, WorkspaceSectionHeaderText, WorkspaceViewOptionsButton, AddWorkspaceButton,
                // 整行扫（不只是标题）：面包屑上级名也是本地化串，切语言时要跟着翻
                SettingsHeaderRow, ApprovalReject, ApprovalAllow,
                ComposerBar, CommandPaletteHeader, ReferencePaletteHeader, ApprovalHost, StatsPage })
                LocalizeShellTree(region);
            // ChatList 本体（不进递归）的 UIA Name：XAML 静态「对话消息列表」。
            LocalizeProperty(ChatList, AutomationProperties.NameProperty);
            foreach (var region in _generalShellRegions) LocalizeShellTree(region);
            // XAML 静态菜单项定义在 Button.Flyout 里，打开才具现化，视觉树扫不到；直接按声明翻译。
            LocalizeMenuItems(ComposerAddFlyout.Items);
            foreach (var item in Nav.MenuItems.Concat(Nav.FooterMenuItems).OfType<NavigationViewItem>())
            {
                if (item.Tag is SessionVm or WorkspaceVm) continue;
                LocalizeProperty(item, ContentControl.ContentProperty);
                LocalizeProperty(item, AutomationProperties.NameProperty);
            }
            // 仅更新提问的壳控件，保留选中状态、自定义输入、内核 header/detail/选项原文。
            foreach (var child in QuestionHost.Children)
            {
                if (child is TextBox custom)
                {
                    LocalizeProperty(custom, TextBox.PlaceholderTextProperty);
                    LocalizeProperty(custom, AutomationProperties.NameProperty);
                }
                if (child is StackPanel panel)
                    foreach (var button in panel.Children.OfType<Button>())
                    {
                        LocalizeProperty(button, ContentControl.ContentProperty);
                        LocalizeProperty(button, AutomationProperties.NameProperty);
                    }
            }
            if (_activeQuestion.ValueKind == JsonValueKind.Object &&
                _activeQuestion.TryGetProperty("request", out var request) &&
                request.TryGetProperty("questions", out var questions) && questions.GetArrayLength() > 0 &&
                QuestionHost.Children.FirstOrDefault() is StackPanel header)
            {
                var first = questions[0];
                if (Str(first, "id") == "plan-review" || Str(first, "header").Length == 0)
                    foreach (var text in header.Children.OfType<TextBlock>().Take(1))
                        LocalizeProperty(text, TextBlock.TextProperty);
            }
            // ApprovalReason 可能与壳兜底句完全相同；不能凭字符串相等推断来源。
            // 动态重建区（可视树扫描覆盖不到）：只在语言真正切换时整体重建。
            // 绝不能每帧重建——本方法挂在 RootGrid.LayoutUpdated 上，重建本身会再次触发布局，
            // 0.7.6.2 曾因无条件重建导致 LayoutCycleException 白屏。
            if (_shellLocale != _lastRebuildLocale)
            {
                _lastRebuildLocale = _shellLocale;
                try
                {
                    RebuildNavMenu();
                    RebuildViewOptionsMenu();
                    UpdateComposerSelectors();
                    RefreshSessionStateBar();
                    UpdateModelEffortLabel();
                    RebuildModelFlyout();
                    RefreshQueuePanel();
                    RefreshJobsPanel();
                    FilesPanelView.ApplyLocale();
                    LocalizeDeliverableCard(ChatList); // 已具现的交付物卡就地重刷（容器不会自动回收触发）
                RefreshKernelBootTexts(); // 加载卡文案含插值/异常原文，扫描反查不到，按当前语言重刷
                }
                catch (Exception) { /* 局部重建失败不阻断语言切换主流程 */ }
            }
        }
        finally { _localizingShell = false; }
    }

    private void ConsumeShellSetting(string ns, string field, object? value)
    {
        if (value is not string preference) return;
        if (ns == "ui-chat" && field == "transcriptView") ApplyTranscriptView(preference);
        if (ns == "locale" && field == "preference")
        {
            var mapped = preference == "en" ? "en" : "zh";
            var locale = _uiLanguageOverride ?? mapped;
            if (_shellLocale == locale) return;
            _shellLocale = locale;
            RefreshShellLanguage();
        }
    }
    private readonly List<SessionVm> _sessions = new();
    private readonly List<WorkspaceVm> _workspaces = new();
    private readonly List<string> _archivedSessions = new();
    /// <summary>session/list 的 projections.values 缓存（sessionId → 已克隆值）：
    /// 打开会话时的投影补齐直接读缓存，免掉一次全量 session/list 往返。
    /// 由 RefreshSessionsAsync 每次拉取清单时同步刷新。</summary>
    private readonly ConcurrentDictionary<string, JsonElement> _sessionProjections = new();
    private string? _sessionStreamId;              // 当前 session/follow 流 id（切会话时取消）
    /// <summary>各会话 running 状态（api-session/status 事件缓存；原版侧栏活动圆点数据源）。</summary>
    private readonly Dictionary<string, bool> _sessionRunning = new();
    private readonly object _sessionRunningLock = new();
    /// <summary>当前活动会话进入繁忙的时刻（busy 语义：Enter 用另一行为、Ctrl+Enter 直发）。</summary>
    private DateTimeOffset? _busySince;
    private readonly List<AttachmentVm> _pendingAttachments = new();
    private string? _pendingApprovalEventId;
    /// <summary>审批事件队列：$events 会同时推多条 approval/request（多工具并发），
    /// 0.7.0 前只保留最后一条——早到的被后到的覆盖，用户永远批不到第一条。逐条出队。</summary>
    private readonly Queue<(string EventId, string ToolName, string? Reason)> _approvalQueue = new();
    /// <summary>当前推理等级（模式）：随消息经 session/selectModel 应用到当前会话。
    /// 档位来自内核 catalog（off/low/high/max——0.7.0 前写死的 medium 不存在）。</summary>
    private string _reasoningEffort = "high";
    /// <summary>壳主题状态：_shellPreference = 内核 ui-theme.preference（light/dark/system），
    /// _shellDark = 当前实际生效的深浅（system 偏好解析后落于此）。</summary>
    private string _shellPreference = "dark";
    private bool _shellDark = true;
    private DispatcherTimer? _backdropResetTimer;
    /// <summary>侧栏固定项数（新会话 + 工作区段标题）：RebuildNavMenu 只重建其后的会话树。</summary>
    private int _fixedMenuItemCount;
    /// <summary>每个工作区分组当前是否展开全部会话（键 = workspaceId，"" = 未分组）。</summary>
    private readonly HashSet<string> _expandedGroups = new(StringComparer.Ordinal);
    /// <summary>侧栏视图选项：分组方式（workspace 按工作区 / flat 单列表）。</summary>
    private string _navGroupBy = "workspace";
    /// <summary>侧栏视图选项：排序方式（manual 手动 / updated 最近更新）。</summary>
    private string _navOrderBy = "manual";
    /// <summary>新会话待用的工作区（composer 工作区选择器；null = 用内核默认 cwd）。</summary>
    private (string Id, string Title)? _pendingWorkspace;
    /// <summary>新会话待用的 Agent 预设（composer 模式选择器；默认 = 内核 standard/标准模式）。</summary>
    private (string Id, string Name)? _pendingPreset = ("standard", "标准模式");
    /// <summary>当前选中模型的显示名（模型+推理档触发器文案左半部分）。</summary>
    private string _selectedModelName = "";
    /// <summary>当前选中模型的 id + provider（selectModel 参数；显示名可能 ≠ id，不能混用）。
    /// 发送路径也用它，避免用显示名重设模型被内核拒绝后静默回落默认。</summary>
    private string _selectedModelId = "";
    private string? _selectedModelProvider;
    /// <summary>模型触发器菜单里的模型项（catalog groups 展开，点击时重建菜单用）。
    /// 只在 UI 线程改写，但发送路径（后台线程）要按 id 查档位能力，读写都走这把锁。</summary>
    private readonly List<(string Name, string Provider, JsonElement Model)> _modelOptions = new();
    private readonly object _modelOptionsLock = new();
    /// <summary>推理档菜单项（当前模型的 reasoning.efforts；名称取内核 catalog 的 name）。</summary>
    private readonly List<(string Id, string Label)> _effortOptions = new();
    /// <summary>最近一次 request/header 里的模型 id。内核每发一次 LLM 请求就记一条 journal 事件
    /// （dsh-agent-loop），这才是"这条消息实际由哪个模型产出"的唯一真相，比壳里记的选择可靠。
    /// 助手/思考气泡在创建时就打上它（同轮 header 必早于答案事件到达）。</summary>
    private string _lastHeaderModel = "";
    /// <summary>等 header 认领模型的本轮用户气泡：journal 的 user/message 一定早于回答它的那次
    /// request/header（header 没有 turn 字段，无法反查归属），所以用户消息的模型只能等 header
    /// 到达再补。轮尾清零——没等到 header 的轮次（请求未发出就失败）不该被下一轮的模型认领。</summary>
    private ChatBubble? _pendingUserBubble;

    // ---------------- 会话控制（session/control 流 + session/updateQueue）状态 ----------------
    /// <summary>各会话排队项（内核 session/control 的 baseline/queue 帧是唯一真相）。</summary>
    private readonly Dictionary<string, List<QueueItemVm>> _queues = new(StringComparer.Ordinal);
    /// <summary>各会话运行中任务（session/control 的 baseline/jobs 帧）。</summary>
    private readonly Dictionary<string, List<JobVm>> _jobs = new(StringComparer.Ordinal);
    private readonly object _controlLock = new();
    /// <summary>session/control 长驻流 id；mux 重连后置空并重开。</summary>
    private string? _controlStreamId;
    /// <summary>当前会话的作业快照（RefreshJobsPanel 按官方口径排序后供会话头触发器与下拉渲染；
    /// 全会话原始数据在 _jobs，这里只放当前会话这一份）。</summary>
    private readonly List<JobVm> _activeJobs = new();
    /// <summary>live 作业时长 tick（1s）：仅下拉打开期间重建行，无 live 作业时停表。</summary>
    private Microsoft.UI.Dispatching.DispatcherQueueTimer? _jobsTickTimer;
    /// <summary>计划（schedule）投影缓存：{id, kind:"at"|"every", prompt, scheduledAt(RFC3339 UTC),
    /// everySeconds}。只读展示用，不创建/编辑/删除（内核无 schedules RPC 暴露面，与官方只读口径一致）。</summary>
    private List<(string Id, string Kind, string Prompt, string ScheduledAt, long EverySeconds)> _scheduleRecords = new();
    private readonly object _scheduleLock = new();
    /// <summary>是否收到过 schedule 投影（内核惰性推送）：没收到过时列表为空 ≠ 没有计划，
    /// 能力对话框要按"未到达"措辞，不能谎称没有计划。</summary>
    private bool _scheduleSeen;
    /// <summary>当前会话目标摘要（GoalBar 常驻条数据源；null = 无目标或已完结）。
    /// 写方有两条：RefreshGoalBarAsync（goals/get，带 activation 全量）与 ApplyGoalProjection
    /// （投影增量，只有 phase/objective/rounds）——后者跑在事件线程，用 _goalLock 护住读写。</summary>
    private (string Id, long Revision, string Phase, string Activation, string Objective, int RoundsStarted)? _goalSummary;
    private readonly object _goalLock = new();

    // ---------------- 会话投影：plan / permissions / schedule / goal（同一条 session/control 流） ----------------
    // 四者的写入路径都只有命令（/plan、/permission、/schedule、/goal）——内核 typert.remote-client.js
    // 里没有 plan / permission 命名空间，即**没有任何对应 RPC**；读路径是会话投影：
    //   plan         { active:bool, pending:bool }        （dsh-plan-mode planProjectionDefinition）
    //   permissions  { options:[{value,name,description?}], currentValue:string }
    //                                                      （dsh-permission-presets 的 select 视图）
    //   schedule     { inheritedEventCount, active:[record], seenIds }
    //                （dsh-schedule scheduleProjectionDefinition；record = {id, kind:"at"|"every",
    //                 prompt, scheduledAt(RFC3339), everySeconds?}，官方端同样只读展示）
    //   goal         null | { goal:{id,revision,objective,phase,blockedReason?,maxGoalRounds},
    //                 roundsStarted, createdAt, updatedAt }（dsh-goal goalProjectionDefinition）
    // 投影值从两处来：session/control 的 baseline.value.projections[<sid>].values（快照）与
    // 后续 projection 帧 {sessionId,key,value,seq}（增量）。四个键都在其中。
    private bool? _planActive;
    private bool? _planPending;
    private List<(string Value, string Name, string Description)> _permissionOptions = new();
    private string? _permissionCurrentValue;
    /// <summary>锁：projection 帧在接收线程解析，UI 刷新在 UI 线程读。</summary>
    private readonly object _projectionLock = new();

    /// <summary>历史图片字节缓存（attachmentId → 位图），避免同一图片回读两次。</summary>
    private readonly Dictionary<string, Microsoft.UI.Xaml.Media.ImageSource?> _attachmentImages = new(StringComparer.Ordinal);

    /// <summary>已贴过图的历史附件（按会话重置）：journal 补拉与 follow 流会重放同一事件。</summary>
    private readonly HashSet<string> _renderedAttachmentIds = new(StringComparer.Ordinal);

    /// <summary>
    /// 本地图片回显登记（图片名 → 待 journal 认领的次数）。发送成功即上屏，随后 journal 的
    /// user/message 带同一图片名到达时按名认领，不再回读 attachmentId 贴第二张。
    /// 与文本回显（FindLocalUserEcho 按文本认领）同思路：journal 记录要等内核落盘，
    /// 回显先画，记录到了就合并而不是重复。
    /// </summary>
    private readonly Dictionary<string, int> _localImageEchoes = new(StringComparer.Ordinal);

    // ---------------- 设置：模型页（提供方卡）状态 ----------------
    // 对照内核官方设置页（dsh-client-ui-settings-models）的卡片状态机：同一时刻至多一张编辑卡，
    // 关闭/切换互不保留草稿（参考页 setState 同语义）；保存成功的回执在下一次渲染显示一次。
    /// <summary>当前展开的编辑卡；null = 收起（只显示行卡与「添加提供方」按钮）。</summary>
    private ModelsEditorState? _modelsEditor;
    /// <summary>编辑卡由「添加提供方」打开（顶部带提供方下拉）；false = 某行卡内的编辑态。</summary>
    private bool _modelsAdding;
    /// <summary>保存成功回执（「已保存 {0}。」）；渲染时消费一次。</summary>
    private string? _modelsSavedNotice;
    /// <summary>提供方下拉里「自定义」入口的选项值（\0 前缀保证不与真实 route 撞名）。</summary>
    private const string ModelsCustomTag = "\u0000custom";

    /// <summary>打开中编辑卡的草稿态：字段直接挂在 UI 控件上回写这里，保存时一次性组装 path ops。</summary>
    private sealed class ModelsEditorState
    {
        public required string Provider;        // route id（自定义流 = 正在输入的 Provider ID）
        public required string DisplayName;     // 行卡显示名（自定义流 = 固定标题）
        public required string SettingsNs;
        public required string[] SettingsPath;  // 自定义流 = ["providers", route]（随输入更新）
        public bool Declared;                   // pi-ai 手工声明路由：拥有显示名称/API 协议字段
        public bool IsCustom;                   // 「自定义提供方」创建流
        public bool FamilyPiAi;                 // ns = llm-pi-ai（密钥占位/发现/协议字段按家族分支）
        public bool FamilyDeepSeek;             // ns = llm-deepseek（占位取 schema 默认容量）

        public JsonObject? Committed;           // 打开时的 user 子树（pathOps 的 before）
        public string KeyRef = "";              // 点名 ?? 派生凭据引用
        public string? FallbackApiKeyEnv;       // 生效值里解析出的 apiKeyEnv（派生位的判定基准）
        public string? FallbackBaseURL;         // 生效值里的 API 地址（占位）
        public bool FallbackMissing;            // 生效值里没有该子树（休眠目录路由）
        public long? DefaultContextWindow;      // deepseek 家族容量占位
        public long? DefaultMaxTokens;
        public List<string> Protocols = new();  // 手声明路由可选线协议（schema union）

        // 字段草稿（与参考页 setField 一致：清空 = unset）
        public string Route = "";
        public string CustomName = "";
        public string BaseURL = "";
        public string Api = "";
        public string Key = "";

        // 模型目录：Models 始终是工作副本（打开时从继承清单克隆，避免与展示别名共享实例）；
        // ModelsOverridden = 用户是否触碰过（false = 继承默认，保存时对 models 出 unset）。
        public List<JsonObject> Models = new();
        public List<JsonObject> InheritedModels = new(); // 继承清单（恢复默认回到这里）
        public bool ModelsOverridden;
        public int ExpandedModelRow = -1;
        public HashSet<(JsonObject Model, string Field)> InvalidCapacities = new();
        public string? FetchFailure;            // 「获取可用模型」失败文案
        public string? ModelError;              // 模型目录行内校验（首错）
        public string? Failure;                 // 保存/校验失败（面板底部错误行）
        public bool Busy;                       // 保存中（禁用按钮防重复提交）
        public List<string> Taken = new();      // 已占用 route（自定义流查重）
    }

    /// <summary>目录 × 设置 × 凭据联接后的一个提供方（对照参考页 store 的 join 结果）。</summary>
    private sealed class ProviderRow
    {
        public required string Provider;
        public required string DisplayName;
        public required string SettingsNs;
        public required string[] SettingsPath;
        public bool Declared;         // declared:true = 手工声明的 pi-ai 路由（行卡带「自定义」标记）
        public string? Error;         // 目录诊断（profile 解析错误等）
        public bool Configured;       // 设置地址已解析（settingsPath 命中生效值）
        public bool Removable;        // 命中 user 层且 base 层没有 = 用户添加的，可整行删除
        public string? ApiKeyEnv;     // profile 点名的凭据引用（null = 环境认证/未写）
        public string KeyRef = "";    // ApiKeyEnv ?? 派生 <ROUTE>_API_KEY
        public JsonElement Credential;      // credentials/describe 的 KeyRef 条目
        public bool HasCredential;
        public JsonElement? Value;    // 生效值子树（含 schema/base 默认）
        public JsonElement? User;     // 用户层子树（编辑草稿底稿、可移除判定）
        public JsonElement? Base;     // 组合层子树（继承模型目录）
        public JsonElement SchemaRoot;      // rehydrate 后的命名空间 schema（继承模型默认的兜底）
    }

    // ---------------- 顶条检索（session/search）状态 ----------------
    /// <summary>检索代次：只有最后一次输入的响应允许改写建议列表。</summary>
    private int _searchSeq;

    // ---------------- 消息反馈（messageFeedback/*）状态 ----------------
    /// <summary>当前会话已提交的反馈：messageId → 条目（list 拉取 + put/delete 回写）。</summary>
    private readonly Dictionary<string, FeedbackItem> _feedback = new(StringComparer.Ordinal);
    /// <summary>已实装的气泡反馈行：messageId → 行控件（ListView 回收后由新的 Loaded 覆盖）。</summary>
    private readonly Dictionary<string, FeedbackRow> _feedbackRows = new(StringComparer.Ordinal);
    /// <summary>全部助手操作行（含无反馈入口的）：bubble → 行。turn/end 回填本轮用时、
    /// 分支可用态变化时按气泡定位行重绘（引用相等键，回收后由新的 Loaded 覆盖）。</summary>
    private readonly Dictionary<ChatBubble, FeedbackRow> _rowsByBubble = new();
    /// <summary>反馈 RPC 进行中的消息（防连点造成 version-conflict 抖动）。</summary>
    private readonly HashSet<string> _feedbackBusy = new(StringComparer.Ordinal);

    // ---------------- 斜杠命令（commands/*）状态 ----------------
    /// <summary>命令目录缓存（会话作用域；commands/change 事件或换会话时失效）。</summary>
    private readonly List<CommandVm> _commands = new();
    private string? _commandsSession;
    private string? _commandsError;
    private DateTimeOffset? _commandsErrorAt;   // 失败时刻（3s 重试窗口）
    /// <summary>当前浮层候选与选中项下标。</summary>
    private List<CommandVm> _commandMatches = new();
    private int _commandIndex = -1;
    /// <summary>补全请求代次：只有最后一次按键的结果允许改写浮层（避免旧查询后到覆盖新查询）。</summary>
    private int _paletteGeneration;

    /// <summary>一条已提交的消息反馈（内核 messageFeedback 的 item）。</summary>
    private sealed class FeedbackItem
    {
        public string MessageId = "";
        public string Rating = "";     // positive | negative
        public string? Note;
        public string? Category;
        public string? Version;        // 乐观并发：put/delete 必须带回观察到的版本
    }

    /// <summary>气泡里的统一操作行（反馈 + 复制 + 分支 + 时间戳 + 用时；代码装配，
    /// 随 DataTemplate 实例重建）。无 message.id 的气泡反馈控件为 null（仅操作组）。</summary>
    private sealed class FeedbackRow
    {
        public string MessageId = "";
        public ChatBubble Bubble = null!;
        public StackPanel Root = null!;
        public ToggleButton? Like;
        public ToggleButton? Dislike;
        public Button? Note;
        public TextBlock? NoteText;
        public Button? Revoke;
        public TextBlock? Status;
        public Button? Copy;
        public Button? Branch;
        public TextBlock? TimeText;
        /// <summary>时间后面的型号（request/header 的实际模型 id）：空 = 无 header 记录，收起不占位。</summary>
        public TextBlock? ModelText;
        public TextBlock? DurationText;
        /// <summary>用量段（「用量 X tok」，对标官方 TurnTailNodeView）：堆叠图标 + 文本一对，
        /// 0 = 无 usage 记录，整体收起不占位。</summary>
        public StackPanel? TokensGroup;
        public TextBlock? TokensText;
        /// <summary>轮尾「本轮文件改动」区（仅答案气泡装配；turn/end 时按最新产出重建 chips）。</summary>
        public Grid? ProducedSection;
        public SimpleWrapPanel? ProducedChips;
        /// <summary>「本轮文件改动」区里的撤回按钮（有可反演突变且未撤回时才装配）。
        /// 撤回成功后转「已撤回」禁用态，不整行重建（ chips 容器重建不影响它）。</summary>
        public Button? RevertButton;
        /// <summary>空闲态不透明度：常驻可见但压低，悬停 / 键盘聚焦 / 已有反馈时升到 1。</summary>
        public double IdleOpacity = 0.35;
        public bool Hover;
        public bool Focused;
    }

    public MainWindow()
    {
        InitializeComponent();
        ShellTranslateFunc = L;
        Title = "Blade²";
        // 标准 WinUI 3 标题栏：内容延伸进标题栏，caption 深浅两套显式配色
        ExtendsContentIntoTitleBar = true;
        _shellDark = IsSystemDark(); // 启动默认跟随系统；内核偏好在 Boot 后覆盖
        LoadShellOptions();          // 壳本地偏好（托盘开关 + 窗口材质）：材质要用于下面的 ApplyBackdrop
        ApplyBackdrop();
        ApplyBubbleMaterial();       // 气泡笔刷按气泡材质/不透明度偏好落到根网格本地资源上
        ApplyCaptionButtonTheme();
        WireTrayBehavior();          // 最小化/关闭拦截（必须在托盘图标创建之前）
        InstallShellIntegration();   // 窗口子类化：system 偏好实时跟随 Windows 深浅色 + 托盘图标
        ShellToast.EnsureRegistered(); // 系统 toast 提前注册（打包形态应成功；失败走托盘气球兜底并留诊断）
        ShellToast.BalloonFallback = ShowTrayBalloon;                   // dev 无包身份：退化托盘气球
        ShellToast.ActivateRequested = () => PostUi(ActivateFromTray);  // 通知点击回前台
        WireRunStatsUi();            // 运行状态条两胶囊的面板弹出
        ResizeToWorkableDefault();
        Activated += OnWindowActivated; // 视频皮肤失焦暂停：焦点变化驱动播放/暂停
        Closed += (_, _) =>
        {
            // 关窗前最后一班岗：编辑器里防抖没到点的草稿存掉再走 Shutdown。
            FlushMemoryPendingSave();
            FlushInstructionsPendingSave();
            Shutdown();
        };

        ChatList.ItemsSource = _visibleMessages;
        InitTurnRail(); // 右侧历史快速定位（turn rail）：事件与数据源一次性挂接
        ChatList.ContainerContentChanging += (_, args) =>
        {
            if (args.ItemContainer is not { } container)
            {
                return;
            }
            // 模板部分在内容变更回调时可能尚未具现化；只在下一帧补写。
            // 绝不能在回调内同步 SetValue——那发生在列表布局过程中，会触发布局循环（LayoutCycleException 白屏）。
            PostUi(() => LocalizeDeliverableCard(container));
        };
        _messages.CollectionChanged += (_, e) =>
        {
            if (e.Action == System.Collections.Specialized.NotifyCollectionChangedAction.Reset)
            {
                _transcriptTurn = 0;
                _closedTranscriptTurns.Clear();
                _startedTranscriptTurns.Clear();
                _transcriptAnswers.Clear();
                _turnStartMs.Clear();
                _producedByTurn.Clear();
                _openMutationPaths.Clear();
                // 执行中工具登记表同属旧会话：气泡已随旧会话撤下，扫光由回收重建时停掉
                _runningToolCalls.Clear();
                // 撤回态同属会话作用域：候选/轮次/突变流水随旧会话作废（_withdrawnTurns 等
                // 按会话 id 分表保留，切回来还要靠它们继续抑制回放）
                _withdrawCandidate = null;
                _turnMutated = false;
                _runTurn = 0;
                _withdrawPendingTurn = false;
                _turnMutations.Clear();
                // 换会话/清空：逐字增量状态属于旧会话，整表作废（旧流也会被取消）
                ForgetLiveAttempts();
                _pendingBubble = null;
                // 占位气泡已随旧会话作废：按钮忙碌态要按新会话重算，否则会停在停止键。
                UpdateComposerRunningState();
            }
            RefreshTranscriptView();
        };
        // 静态 XAML 与异步构造的设置/审批控件共用入口；只写实际变化的属性。
        RootGrid.LayoutUpdated += (_, _) => RefreshShellLanguage();

        // 侧栏固定项（新会话 + 工作区段标题）只建一次：段标题里挂着视图选项/添加工作区两个动作按钮，
        // 每轮会话刷新重建会丢焦点/状态。会话树部分仍整体重建（见 RebuildNavMenu）。
        // 进设置页时固定项整体折叠让位给分区项，退出时展开再重建（见 ShowChatPage）。
        _fixedMenuItemCount = Nav.MenuItems.Count;

        // 品牌图标：文件优先（开发形态跑 bin 下的 exe 时 ms-appx 解析不出来），回落 ms-appx
        TopBarBrandMark.Source = BrandImageSource();
        HeroMarkImage.Source = BrandImageSource();
        KernelBootMark.Source = BrandImageSource();

        LoadSavedSkin();

        // 视窗标题栏拖拽区：顶条那条 48px 透明带（品牌与搜索框压在它上层）。
        TitleBarDragRegion.Loaded += (_, _) => ApplyTitleBarDragRegion(TitleBarDragRegion);

        // 输入卡获得焦点：第 5 条要求「聚焦零颜色反应」——不换强调色、不改底色、不加光晕，
        // 只用同色描边加粗作为非彩色焦点提示（内边距等量扣回，内容不位移）。
        InputBox.GotFocus += (_, _) => SetComposerFocusVisual(true);
        InputBox.LostFocus += (_, _) => SetComposerFocusVisual(false);

        // 「+」菜单里的「在应用中打开」子菜单：展开外层菜单时才拉内核应用清单
        // （内核侧有懒解析与失败重试，缓存反而会锁住早期失败结果）。
        ComposerAddFlyout.Opening += (_, _) => _ = LoadOpenInAppMenuAsync();

        // 输入区上方两个选择器：展开时按最新工作区/预设清单重建（工作区可增删，预设目录可写）
        WorkspacePickerFlyout.Opening += (_, _) => UpdateComposerSelectors();
        AgentModeFlyout.Opening += (_, _) => _ = LoadAgentModeMenuAsync();
        // 模型菜单：展开时重拉 catalog（同 AgentMode 模式）。此前 OnModelButtonClick
        // 从未接线，_modelOptions 恒空 → 菜单只有禁用标题，模型无法切换。
        ModelFlyout.Opening += (_, _) => _ = RefreshModelCatalogAsync();
        // 侧栏「视图选项」菜单同法：展开时重建。此前只由进设置页/切换分组排序时重建，
        // 冷启动（未进过设置页）直接展开得到的是空菜单——实测菜单项数 0、弹出层仅 118x52dip。
        WorkspaceViewOptionsFlyout.Opening += (_, _) => RebuildViewOptionsMenu();

        // 固定项/分区导航统一走 NavigationView.ItemInvoked（标准事件）；
        // 会话项仍由 SelectionChanged 驱动（SelectsOnInvoked 默认选中）
        Nav.ItemInvoked += OnNavItemInvoked;
        // Ctrl+S 加速键与点击同路（KeyboardAccelerator 走 Invoked，不等 Tapped）
        SettingsItem.KeyboardAccelerators[0].Invoked += (_, _) => ToggleSettingsPage();
        Nav.SelectionChanged += OnNavSelectionChanged;
        SearchBox.TextChanged += OnSearchBoxChanged;
        SearchBox.QuerySubmitted += OnSearchBoxQuerySubmitted;
        // 搜索框：正椭圆（半径 = 实测控件高 / 2）。AutoSuggestBox 的圆角不传导内层 TextBox，
        // 因此视觉树里外层与内层同时设；高度随字号/DPI 变化，必须按实测高重算而非写死令牌。
        SearchBox.Loaded += (_, _) => ApplySearchPill();
        SearchBox.SizeChanged += (_, _) => ApplySearchPill();

        // 设置页页头左对齐：内容列（含居中限宽与滚动条占位）的实际左偏移决定页头左边距
        SettingsHost.SizeChanged += (_, _) => AlignSettingsHeader();
        SettingsScroller.SizeChanged += (_, _) => AlignSettingsHeader();

        // 侧栏形态变化（窄窗自动收起等）→ 刷新顶栏左侧那一组（返回 → 字标）
        Nav.PaneOpened += (_, _) => UpdateShellCompactState();
        Nav.PaneClosed += (_, _) => UpdateShellCompactState();
        // 窄窗：内容列被 264px 侧栏挤到不可用时自动收起侧栏（跨阈值才动作，不与用户手动展开互抢）
        RootGrid.SizeChanged += (_, e) => ApplyNarrowPaneRule(e.NewSize.Width);

        // 趋势图是 Canvas 自绘：绘图区宽度来自布局，拉伸后必须重绘（数据不变，只重算坐标）
        StatsTrendCanvas.SizeChanged += (_, _) => RenderStatsTrend();

        // 输入区自适应：宽度变化时重算胶囊上限/换行（幂等，见方法注释）
        ComposerBar.SizeChanged += (_, _) => ApplyComposerAdaptiveLayout();
        ComposerBar.Loaded += (_, _) => ApplyComposerAdaptiveLayout();

        // 聊天列自适应：ChatColumn / ChatList 纯 Stretch、无 MaxWidth，
        // 宽度由星型列原生驱动，不再需要代码按 clamp(680,64%,920) 手动设宽
        // （手动设宽正是大窗冻住、小窗溢出的根因，已删除）。

        // 拉伸后 DesktopAcrylic 会丢：防抖重建材质
        _backdropResetTimer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(400) };
        _backdropResetTimer.Tick += (_, _) =>
        {
            _backdropResetTimer.Stop();
            ApplyBackdrop();
        };
        SizeChanged += (_, _) => _backdropResetTimer.Start();

        // 文件右栏的关闭钮：直接收起右栏（入口已并入输入区的「+」菜单）
        FilesPanelView.CloseRequested += () =>
        {
            try
            {
                SetFilesPanelOpen(false);
            }
            catch (Exception) { } // 事件入口兜底
        };

        // 内核不再先于界面启动：壳先亮相，内核在后台起，进度摆在主页加载卡上。
        StartKernelBoot();
    }

    /// <summary>
    /// 品牌图（鲸标）的图源：优先从应用目录读文件，失败回落 ms-appx。
    /// 原因：ms-appx:/// 走包图，只有 MSIX 安装形态（有包标识）能解析；开发形态直接运行
    /// bin 下的 exe 时两张图都不显示（实测截图：品牌行与空态主标位置为空）。打包形态下
    /// 安装目录里同样有该文件，因此文件优先在所有形态下都成立。
    /// </summary>
    private static Microsoft.UI.Xaml.Media.ImageSource BrandImageSource()
    {
        try
        {
            var path = Path.Combine(AppContext.BaseDirectory, "Assets", "Square44x44Logo.png");
            if (File.Exists(path))
            {
                return new Microsoft.UI.Xaml.Media.Imaging.BitmapImage(new Uri(path));
            }
        }
        catch (Exception) { }
        return new Microsoft.UI.Xaml.Media.Imaging.BitmapImage(new Uri("ms-appx:///Assets/Square44x44Logo.png"));
    }

    /// <summary>
    /// 标题栏拖拽区：顶条 48px 那条透明带。SetTitleBar 接管后窗口默认的"顶部 32px 整宽拖拽带"
    /// 不再吞掉顶条里搜索框的点击（未接管时实测 x=300,y=20 返回 HTCAPTION）。
    /// </summary>
    private void ApplyTitleBarDragRegion(UIElement element)
    {
        try
        {
            SetTitleBar(element);
        }
        catch (Exception) { } // 拖拽区设置失败不致命：窗口仍可用键盘/系统菜单移动
    }

    // ---------------- 内核启动与连接 ----------------

    /// <summary>
    /// 内核异步引导的入口（壳亮相后由 <see cref="StartKernelBoot"/> 调起）：
    /// 取消静默返回，其余任何异常都收敛成加载卡上的失败态 + 重试钮
    /// （绝不沿 async void/无人观察的 Task 上抛成进程级未处理异常）。
    /// </summary>
    private async Task RunKernelBootAsync(CancellationToken ct)
    {
        try
        {
            await RunKernelBootCoreAsync(ct);
        }
        catch (OperationCanceledException)
        {
            // 取消 = 重试或关窗：旧引导到此为止，新一轮引导/进程退出接手
        }
        catch (Exception ex)
        {
            FailKernelBoot("内核启动失败", ex.Message);
        }
    }

    /// <summary>
    /// 内核异步引导本体：分阶段往主页加载卡上报进度，任一步失败都停在加载卡上给原因与重试入口，
    /// 绝不用全屏加载层把界面挡住。ct 是硬取消（重试/关窗）：中途 await 被取消即刻返回，
    /// 内核进程由 <see cref="_kernel"/> 的处置权收回（重试时新建宿主，关窗时 Dispose）。
    /// </summary>
    private async Task RunKernelBootCoreAsync(CancellationToken ct)
    {
        var dshHome = DataHome;
        try
        {
            Directory.CreateDirectory(dshHome);
        }
        catch (Exception)
        {
            // 建家目录失败（权限/磁盘满）不在这里报：内核启动阶段的失败信息会把后果带出来
        }

        // 默认插件引导（幂等）：缺失的包走内核 CLI 安装、挂载条目写入 profile patch 层。
        // 必须先于内核启动——patch 引用的包不就绪会让内核加载失败；失败只记诊断不阻塞裸启动。
        // 首次启动要拉十几个包（pnpm 直跑，可能几分钟），这段时间界面全可用，只有加载卡在转。
        ReportKernelBootStage("正在准备内核组件…");
        try
        {
            var node = DshKernelHost.BundledNode;
            if (node is not null)
            {
                var bootDiag = await DshPluginBootstrap.EnsureAsync(
                    dshHome, node, DshKernelHost.BundledBinDir, CreatePluginProgress(), ct);
                if (bootDiag.Length > 0)
                {
                    System.Diagnostics.Debug.WriteLine($"[plugin-bootstrap] {bootDiag.TrimEnd()}");
                }
            }
        }
        catch (Exception)
        {
            // 引导失败不阻塞内核裸启动；诊断已落 DSH_HOME\logs\plugin-bootstrap.log
        }

        ReportKernelBootStage("正在启动内核…");
        var url = await _kernel.StartAsync(dshHome, ct);
        if (url is null)
        {
            FailKernelBoot(_kernel.IsBundled
                ? "内核启动失败"
                : "未找到内置内核；请安装 npm 版 dsh 或重新安装 Blade²");
            return;
        }

        var baseUri = new Uri(url.Split('?')[0]);
        _rpc = new DshRpcClient(baseUri);
        await _rpc.AuthenticateAsync(url, ct);

        ReportKernelBootStage("正在连接内核…");
        // 桌面宠物：内核 dsh-pet 插件的状态轮询 + 窗口内精灵层（插件没装时只探 404，
        // 不影响任何其它功能）。设置分区由「宠物」入口渲染。
        StartPetRuntime();

        // 工作区文件右栏：会话 id + 该会话的工作区根（session/list 的 cwd —— 内核
        // workspaceFileScope lookup 正是用 header.cwd 解工作区根）。
        FilesPanelView.Attach(
            _rpc,
            () => Volatile.Read(ref _activeSessionId),
            () => _sessions.FirstOrDefault(s => s.SessionId == Volatile.Read(ref _activeSessionId))?.Cwd);

        // commands/change：命令注册表变化（插件装/卸、agent 预设切换）时作废目录缓存。
        // 该事件在内核 $events 转发白名单里（dsh-api-remotes API_REMOTE_FORWARDED_EVENTS）。
        _rpc.OnEvent("commands/change", _ =>
        {
            _commandsSession = null;
            _commandsError = null;
            _commandsErrorAt = null;
            return Task.CompletedTask;
        });

        // 启动即空态：品牌标 + 输入区选择器 + 默认权限选择器（不等待会话列表）。
        // 内核就绪前 ChatHero 让位给加载卡（见 UpdateEmptyState），这一步先把其余部件配齐。
        PostUi(UpdateEmptyState);

        // 时序（0.7.1）：workspace 域必须最先订阅/触碰，让 WorkspaceRegistry 的服务
        // 初始化（含 cwd 自动归组 bootstrap）先于 $events 流完成——若 $events 先牵动
        // 各服务初始化，workspace 域可能把 initialized 落成"空表已初始化"，bootstrap
        // 从此永久跳过（曾致全部会话平铺无文件夹）。ConnectMuxAsync 只建连接，
        // $events 订阅移到 RefreshWorkspacesAsync 之后。
        await _rpc.ConnectMuxEventsDeferredAsync(ct);

        // 模型目录：默认模型/provider + 默认档位菜单（不再硬编码 deepseek-V4-Pro/high）
        try
        {
            var catalog = await _rpc.CallOkAsync("session/modelCatalog", new { });
            if (catalog.TryGetProperty("default", out var def) && def.ValueKind == JsonValueKind.Object)
            {
                var provider = def.TryGetProperty("provider", out var p) ? p.GetString() ?? "deepseek-official" : "deepseek-official";
                var modelId = def.TryGetProperty("model", out var m) ? m.GetString() ?? "" : "";
                _catalogDefault = (provider, modelId);
                _selectedModelId = modelId;
                _selectedModelProvider = provider;
                // 按钮显示模型的可读名（catalog groups 里找 name）
                if (catalog.TryGetProperty("groups", out var groups) && groups.ValueKind == JsonValueKind.Array)
                {
                    PostUi(() => PopulateModelOptions(catalog));
                    foreach (var g in groups.EnumerateArray())
                    {
                        if (g.TryGetProperty("models", out var models) && models.ValueKind == JsonValueKind.Array)
                        {
                            foreach (var mm in models.EnumerateArray())
                            {
                                if (mm.TryGetProperty("id", out var mid) && mid.GetString() == modelId)
                                {
                                    PostUi(() =>
                                    {
                                        if (mm.TryGetProperty("name", out var nm) && nm.ValueKind == JsonValueKind.String)
                                        {
                                            _selectedModelName = nm.GetString() ?? modelId;
                                        }
                                        SetEffortMenuFromModel(mm);
                                        if (mm.TryGetProperty("reasoning", out var rr) && rr.ValueKind == JsonValueKind.Object &&
                                            rr.TryGetProperty("defaultEffort", out var de) && de.ValueKind == JsonValueKind.String)
                                        {
                                            SetEffortUi(de.GetString() ?? "high");
                                        }
                                        RebuildModelFlyout();
                                        UpdateModelEffortLabel();
                                    });
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        catch (Exception) { } // 目录失败：保持 UI 默认值，发送时再试

        _rpc.OnWaterfall("approval/request", OnApprovalRequestAsync);
        _rpc.EventCancelled += OnEventCancelled;
        _rpc.OnEvent("api-session/added", OnSessionAddedAsync);
        _rpc.OnEvent("api-session/removed", OnSessionRemovedAsync);
        _rpc.OnEvent("api-session/status", OnSessionStatusAsync);
        // 提问（user-questions/request）走的是 **waterfall** 而不是 emit：
        // 内核在 $events 上推 {type:"waterfall",event,eventId,agentId,request} 并挂起等待，
        // 必须回传 $events/result {outcome:{kind:"result",value:{answers:[…]}}} 才解挂。
        // 该事件同样在 API_REMOTE_FORWARDED_EVENTS 白名单里（mode: "waterfall"）。
        _rpc.OnWaterfall("user-questions/request", OnUserQuestionAsync);

        RegisterStreamRecovery();

        // 启动换肤：恢复内核保存的 ui-theme.preference（壳跟随 dsh 设置，与原版一致）
        try
        {
            var snapshot = await EnsureSettingsSnapshotAsync();
            foreach (var entry in snapshot?.Values ?? Enumerable.Empty<(JsonElement Value, double Revision)>())
            {
                var ns = entry.Value;
                if (Str(ns, "ns") != "ui-theme") continue;
                if (ns.TryGetProperty("value", out var v) && v.ValueKind == JsonValueKind.Object)
                {
                    if (v.TryGetProperty("preference", out var p) && p.ValueKind == JsonValueKind.String)
                    {
                        ApplyShellTheme(p.GetString() ?? "system");
                    }
                    // 字号：会话内容字号 12–17，跟随内核保存值
                    if (v.TryGetProperty("fontSize", out var f) && f.ValueKind == JsonValueKind.Number &&
                        f.GetDouble() is >= 12 and <= 17)
                    {
                        ChatList.FontSize = f.GetDouble();
                    }
                }
            }
        }
        catch (Exception) { } // 设置读取失败：保持系统默认，不阻塞启动

        ReportKernelBootStage("正在加载工作区与会话…");
        // 时序（0.7.1）：workspace 域必须最先订阅/触碰，让 WorkspaceRegistry 的服务
        // 初始化（含 cwd 自动归组 bootstrap）先于 $events 流完成——若 $events 先牵动
        // 各服务初始化，workspace 域可能把 initialized 落成"空表已初始化"，bootstrap
        // 从此永久跳过（曾致全部会话平铺无文件夹）。$events 订阅在两步之后补上。
        await RefreshWorkspacesAsync();
        await _rpc.SubscribeEventsAsync(ct);
        await RefreshSessionsAsync();
        // 会话控制面（排队项 + 任务）：长驻流的启动时机放在 workspace/$events 之后，
        // 避免影响 workspace 域的初始化次序（见上面 0.7.1 时序注释）。
        await OpenSessionControlStreamAsync();

        // 内核就绪、加载卡撤下后再刷一次空态：上面那次 UpdateEmptyState（连接阶段）被加载卡
        // 挡住了品牌标，这里补上；会话清单此时也已到位。
        // 目标值先推满，等条子动画爬到 100% 再撤卡——不然用户只看到条子半路消失。
        CompleteKernelBootProgress();
        await Task.Delay(TimeSpan.FromMilliseconds(350), ct);
        HideKernelBootPanel();
        PostUi(UpdateEmptyState);
    }

    private Task OnSessionAddedAsync(JsonElement _) => RefreshAllListsAsync();
    private Task OnSessionRemovedAsync(JsonElement _) => RefreshAllListsAsync();

    /// <summary>api-session/status 事件（emit 帧 args=[sessionId, running]）：会话忙碌实时源。
    /// 原版语义 = 侧栏运行会话标活动状态 + 繁忙时输入区改变发送行为。这里：
    /// ① 缓存各会话 running 状态；② 增量刷新侧栏（不重拉列表——status 就是权威信号）；
    /// ③ 繁忙态由 SendAsync 发送前查（见 busyEnter 语义）。</summary>
    private Task OnSessionStatusAsync(JsonElement frame)
    {
        if (frame.ValueKind == JsonValueKind.Object && frame.TryGetProperty("args", out var args) &&
            args.ValueKind == JsonValueKind.Array && args.GetArrayLength() >= 2 &&
            args[0].ValueKind == JsonValueKind.String && (args[1].ValueKind == JsonValueKind.True || args[1].ValueKind == JsonValueKind.False))
        {
            var sid = args[0].GetString()!;
            var running = args[1].GetBoolean();
            var changed = false;
            lock (_sessionRunningLock)
            {
                // 流式期间内核会重复推送相同 running 值：没变就不碰侧栏
                // （RebuildNavMenu 全量重建会话项，挤占逐字重绘的帧预算）。
                changed = !_sessionRunning.TryGetValue(sid, out var prev) || prev != running;
                _sessionRunning[sid] = running;
            }
            if (running)
            {
                _busySince ??= DateTimeOffset.Now;
            }
            else if (sid == Volatile.Read(ref _activeSessionId))
            {
                _busySince = null;
            }
            if (changed)
            {
                // 侧栏活动指示：RebuildNavMenu 读 _sessionRunning 画圆点（UI 线程编组）。
                PostUi(() =>
                {
                    RebuildNavMenu();
                    UpdateComposerRunningState(force: true);   // 权威忙碌信号：发送键 ↔ 停止键
                });
            }
            return Task.CompletedTask;
        }
        return Task.CompletedTask;
    }

    private async Task RefreshAllListsAsync()
    {
        await RefreshSessionsAsync();
        await RefreshWorkspacesAsync();
    }

    /// <summary>会话标题规则（displayTitleOf）：blank 显示"新会话"；
    /// 否则 durable title → cwd 目录名（workspaceTitleOf：路径最后一段非空目录名）→ sessionId。</summary>
    private static string SessionDisplayTitle(JsonElement projValues, bool blank, string cwd, string sessionId)
    {
        if (blank)
        {
            return "新会话";
        }
        if (projValues.ValueKind == JsonValueKind.Object &&
            projValues.TryGetProperty("title", out var t) && t.ValueKind == JsonValueKind.String && !string.IsNullOrEmpty(t.GetString()))
        {
            return t.GetString()!;
        }
        var baseName = WorkspaceBasename(cwd);
        return baseName.Length > 0 ? baseName : sessionId;
    }

    /// <summary>路径最后一段非空目录名（workspaceTitleOf：容忍尾部斜杠）。</summary>
    private static string WorkspaceBasename(string path)
    {
        var trimmed = path.TrimEnd('/', '\\');
        var sep = Math.Max(trimmed.LastIndexOf('/'), trimmed.LastIndexOf('\\'));
        return trimmed[(sep + 1)..];
    }

    /// <summary>
    /// 侧栏会话行的相对时间（内核 workspace 字典 time.* 文案）：
    /// 刚刚 / N分钟 / N小时 / N天 / N个月 / N年，统一后缀"前"；时间戳缺失（0）返回空串。
    /// </summary>
    private string RelativeTimeLabel(long updatedAtMs)
    {
        if (updatedAtMs <= 0)
        {
            return "";
        }
        var span = DateTimeOffset.Now - DateTimeOffset.FromUnixTimeMilliseconds(updatedAtMs);
        if (span < TimeSpan.Zero)
        {
            span = TimeSpan.Zero;
        }
        return span.TotalMinutes switch
        {
            < 1 => L("刚刚"),
            < 60 => LF("{0}分钟前", (int)span.TotalMinutes),
            < 60 * 24 => LF("{0}小时前", (int)span.TotalHours),
            < 60 * 24 * 30 => LF("{0}天前", (int)span.TotalDays),
            < 60 * 24 * 365 => LF("{0}个月前", (int)(span.TotalDays / 30)),
            _ => LF("{0}年前", (int)(span.TotalDays / 365)),
        };
    }

    /// <summary>侧栏"视图选项"菜单（分组方式 + 排序方式）。</summary>
    private void RebuildViewOptionsMenu()
    {
        WorkspaceViewOptionsFlyout.Items.Clear();
        var groupHeader = new MenuFlyoutItem { Text = L("分组方式"), IsEnabled = false };
        WorkspaceViewOptionsFlyout.Items.Add(groupHeader);
        foreach (var (id, label) in new[] { ("workspace", L("按工作区")), ("flat", L("单列表")) })
        {
            var item = new RadioMenuFlyoutItem
            {
                Text = label,
                Tag = id,
                GroupName = "nav-group-by",
                IsChecked = _navGroupBy == id,
            };
            Aut(item, $"NavGroupBy_{id}", LF("分组方式：{0}", label));
            item.Click += (_, _) => SetNavGroupBy(id);
            WorkspaceViewOptionsFlyout.Items.Add(item);
        }
        WorkspaceViewOptionsFlyout.Items.Add(new MenuFlyoutSeparator());
        WorkspaceViewOptionsFlyout.Items.Add(new MenuFlyoutItem { Text = L("排序方式"), IsEnabled = false });
        foreach (var (id, label) in new[] { ("manual", L("手动排序")), ("updated", L("最近更新")) })
        {
            var item = new RadioMenuFlyoutItem
            {
                Text = label,
                Tag = id,
                GroupName = "nav-order-by",
                IsChecked = _navOrderBy == id,
            };
            Aut(item, $"NavOrderBy_{id}", LF("排序方式：{0}", label));
            item.Click += (_, _) => SetNavOrderBy(id);
            WorkspaceViewOptionsFlyout.Items.Add(item);
        }
    }

    private void SetNavGroupBy(string id)
    {
        _navGroupBy = id == "flat" ? "flat" : "workspace";
        RebuildViewOptionsMenu();
        RebuildNavMenu();
    }

    private void SetNavOrderBy(string id)
    {
        _navOrderBy = id == "updated" ? "updated" : "manual";
        RebuildViewOptionsMenu();
        RebuildNavMenu();
    }

    /// <summary>工作区段标题的加号：添加工作区（原"新建工作区"入口的位置）。</summary>
    private void OnAddWorkspaceClick(object sender, RoutedEventArgs e) => _ = CreateWorkspaceAsync();

    private Microsoft.UI.Dispatching.DispatcherQueueTimer? _sessionsRefreshTimer;

    /// <summary>会话清单刷新的合帧入口：journal 每条事件都要一次会让整侧栏重建（含 UIA 写入）
    /// 挤占逐字重绘的帧预算；2 秒窗口内多次请求只跑一次。定时器一窗口一个（同 ScheduleLiveRepaint）。</summary>
    private void RequestSessionsRefresh()
    {
        if (_sessionsRefreshTimer is not null)
        {
            return;
        }
        var timer = DispatcherQueue.CreateTimer();
        timer.Interval = TimeSpan.FromSeconds(2);
        timer.IsRepeating = false;
        timer.Tick += (_, _) =>
        {
            _sessionsRefreshTimer = null;
            _ = RefreshSessionsAsync();
        };
        _sessionsRefreshTimer = timer;
        timer.Start();
    }

    private async Task RefreshSessionsAsync()
    {
        if (_rpc is null)

        {
            return;
        }
        try
        {
            var value = await _rpc.CallOkAsync("session/list", new { _request = new { } });
            var items = value.GetProperty("items");
            var vms = new List<SessionVm>();
            foreach (var item in items.EnumerateArray())
            {
                var id = item.GetProperty("sessionId").GetString() ?? "";
                var cwd = item.TryGetProperty("cwd", out var cwdEl) && cwdEl.ValueKind == JsonValueKind.String ? cwdEl.GetString() ?? "" : "";
                var blank = item.TryGetProperty("blank", out var b) && b.ValueKind == JsonValueKind.True;
                // projections 是内核惰性缓存：旧会话（v0 迁移来的）整块缺失，必须容错。
                var proj = item.TryGetProperty("projections", out var p) && p.ValueKind == JsonValueKind.Object ? p : default;
                var projValues = proj.ValueKind == JsonValueKind.Object && proj.TryGetProperty("values", out var pv) && pv.ValueKind == JsonValueKind.Object ? pv : default;
                var stats = projValues.ValueKind == JsonValueKind.Object && projValues.TryGetProperty("sessionStats", out var s) && s.ValueKind == JsonValueKind.Object ? s : default;
                var turns = stats.ValueKind == JsonValueKind.Object && stats.TryGetProperty("turns", out var tn) && tn.ValueKind == JsonValueKind.Number ? tn.GetInt32() : 0;
                // projections.values 同步进缓存：打开会话时的投影补齐读它，免一次全量清单请求
                if (projValues.ValueKind == JsonValueKind.Object)
                {
                    _sessionProjections[id] = projValues.Clone();
                }
                vms.Add(new SessionVm
                {
                    SessionId = id,
                    Title = SessionDisplayTitle(projValues, blank, cwd, id),
                    Subtitle = turns > 0 ? LF("{0} 轮对话", turns) : "",
                    Cwd = cwd,
                    // updatedAt 是 SessionSummary 的字段（内核 session/list 每项都带）
                    UpdatedAt = item.TryGetProperty("updatedAt", out var ua) && ua.ValueKind == JsonValueKind.Number
                        ? (long)ua.GetDouble()
                        : 0,
                    Blank = blank,
                    // origin/parentSessionId：子代理会话与普通会话同在 session/list 返回
                    // （内核 SessionSummary 契约），壳据此做侧栏标记、只读 composer 与回跳
                    Origin = item.TryGetProperty("origin", out var org) && org.ValueKind == JsonValueKind.String ? org.GetString() : null,
                    ParentSessionId = item.TryGetProperty("parentSessionId", out var psid) && psid.ValueKind == JsonValueKind.String && psid.GetString() is { Length: > 0 } parent ? parent : null,
                });
            }
            PostUi(() =>
            {
                // 渲染口径无变化就不碰侧栏：RebuildNavMenu 会销毁并重建全部会话项的
                // 可视化树（含 UIA 写入），流式期间 2 秒合帧刷一次清单，旧会话的
                // updatedAt/turns 抖动若不改变显示文本，重建纯属白费帧预算。
                if (SessionsUnchanged(vms))
                {
                    return;
                }
                // Nav 菜单项重建（固定项 = 新建会话 + 分隔线）
                var selectedTitle = _sessions.FirstOrDefault(s => s.SessionId == _activeSessionId)?.Title;
                _sessions.Clear();
                _sessions.AddRange(vms);
                RebuildNavMenu();
                _ = selectedTitle; // 选中态靠 SessionId 比对（RebuildNavMenu 内处理）
                // 会话状态条跟着会话清单刷新：有活动会话时就该出现（plan/permissions 值
                // 可能稍后由投影帧补齐，但"会话反馈"入口只依赖会话是否存在）。
                RefreshSessionStateBar();
            });
        }
        catch (Exception)
        {
            // 会话列表失败不阻塞聊天。
        }
    }

    /// <summary>
    /// 侧栏渲染口径的清单比对：顺序、标题、副标题（轮数）、空会话位、子代理位与
    /// 相对时间显示文本全部一致才判定无变化。updatedAt 的分钟内抖动不改变显示文本，
    /// 视为无变化（相对时间本身只有分钟级粒度）。
    /// </summary>
    private bool SessionsUnchanged(List<SessionVm> vms)
    {
        if (_sessions.Count != vms.Count)
        {
            return false;
        }
        for (var i = 0; i < vms.Count; i++)
        {
            var a = _sessions[i];
            var b = vms[i];
            if (a.SessionId != b.SessionId ||
                a.Title != b.Title ||
                a.Subtitle != b.Subtitle ||
                a.Blank != b.Blank ||
                a.Origin != b.Origin ||
                a.ParentSessionId != b.ParentSessionId ||
                RelativeTimeLabel(a.UpdatedAt) != RelativeTimeLabel(b.UpdatedAt))
            {
                return false;
            }
        }
        return true;
    }

    /// <summary>
    /// 侧栏渲染（导航分区语义）：分组 1 = 各工作区（文件夹），其会话作为子节点；
    /// 分组 2 = "未分组"（不在任何工作区的会话）。每个分组默认只展开前若干个会话，
    /// 其余藏在"展开其余 N 个会话"条目后（同内核的 sessions 折叠语义）。
    /// 归档会话（archivedSessionIds）不显示，与原版一致。
    /// 固定项（新会话 + 工作区段标题）在 XAML 里声明且只建一次，本方法只重建其后的部分。
    /// </summary>
    private void RebuildNavMenu()
    {
        if (SettingsPage is not null && SettingsPage.Visibility == Visibility.Visible)
        {
            // 设置页打开时侧栏是分区项（会话树不存在）：后台流帧（session/follow→RefreshSessions）
            // 到这里一律 no-op——不写 SelectedItem、不碰 MenuItems（写 UIA 树是 0xc000027b 源头）。
            // 会话数据已落在 _sessions/_workspaces，回聊天页时本方法自然重建。
            return;
        }

        // 先解除选中引用再裁项（SelectedItem 指着集合内项时 Remove 是 0x80004005 温床）
        Nav.SelectedItem = null;
        while (Nav.MenuItems.Count > _fixedMenuItemCount)
        {
            Nav.MenuItems.RemoveAt(Nav.MenuItems.Count - 1);
        }

        var archived = _archivedSessions.ToHashSet();
        var filed = new HashSet<string>();

        if (_navGroupBy == "flat")
        {
            // 单列表：不分组，全部会话平铺（官方 groupBy.flat）
            var all = _sessions.Where(s => !archived.Contains(s.SessionId)).ToList();
            foreach (var group in new[] { all })
            {
                AppendSessionRows(group, Nav.MenuItems, "flat");
            }
            UpdateEmptyState();
            RestoreActiveSessionSelection();
            return;
        }

        // 工作区节点（各自挂会话子项）
        foreach (var ws in _workspaces)
        {
            var wsItem = new NavigationViewItem
            {
                Content = ws.Title,
                MinWidth = 0,
                Tag = ws,
                Icon = new FontIcon { Glyph = "\uE8B7" }, // 文件夹图标
            };
            // AutomationId 取工作区 id（空 id 的"未分组"节点走下面单独的项）
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(wsItem, $"Workspace_{ws.WorkspaceId}");
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(wsItem, ws.Title);
            var children = new List<SessionVm>();
            foreach (var sid in ws.SessionIds)
            {
                filed.Add(sid);
                var vm = _sessions.FirstOrDefault(s => s.SessionId == sid);
                if (vm is null || archived.Contains(sid))
                {
                    continue;
                }
                children.Add(vm);
            }
            AppendSessionRows(children, wsItem.MenuItems, ws.WorkspaceId);
            // 工作区右键：打开路径 / 重命名 / 上移 / 下移 / 删除（原版语义：删除保留记录，会话回未分组）
            var wsMenu = new MenuFlyout();
            var openPath = new MenuFlyoutItem { Text = L("打开工作区路径"), Tag = ws };
            Aut(openPath, "OpenWorkspacePathMenuItem", LF("打开工作区路径：{0}", ws.Title));
            openPath.Click += (_, _) => _ = OpenWorkspacePathAsync(ws);
            wsMenu.Items.Add(openPath);
            var renameWs = new MenuFlyoutItem { Text = L("重命名工作区"), Tag = ws };
            Aut(renameWs, "RenameWorkspaceMenuItem", LF("重命名工作区：{0}", ws.Title));
            renameWs.Click += (_, _) => _ = RenameWorkspaceAsync(ws);
            wsMenu.Items.Add(renameWs);
            // 排序：workspace/insertBefore（改的是内核侧工作区顺序，列表随 follow 流刷新）
            var moveUp = new MenuFlyoutItem { Text = L("上移"), Tag = ws };
            Aut(moveUp, "MoveWorkspaceUpMenuItem", LF("上移：{0}", ws.Title));
            moveUp.Click += (_, _) => _ = MoveWorkspaceAsync(ws, -1);
            wsMenu.Items.Add(moveUp);
            var moveDown = new MenuFlyoutItem { Text = L("下移"), Tag = ws };
            Aut(moveDown, "MoveWorkspaceDownMenuItem", LF("下移：{0}", ws.Title));
            moveDown.Click += (_, _) => _ = MoveWorkspaceAsync(ws, 1);
            wsMenu.Items.Add(moveDown);
            var deleteWs = new MenuFlyoutItem { Text = L("删除工作区"), Tag = ws };
            Aut(deleteWs, "DeleteWorkspaceMenuItem", LF("删除工作区：{0}", ws.Title));
            deleteWs.Click += (_, _) => _ = DeleteWorkspaceAsync(ws);
            wsMenu.Items.Add(deleteWs);
            wsItem.ContextFlyout = wsMenu;
            Nav.MenuItems.Add(wsItem);
        }

        // 未分组区
        var unfiled = _sessions.Where(s => !filed.Contains(s.SessionId) && !archived.Contains(s.SessionId)).ToList();
        if (unfiled.Count > 0 || _workspaces.Count == 0)
        {
            var groupItem = new NavigationViewItem
            {
                Content = L("未分组"),
                MinWidth = 0,
                Tag = new WorkspaceVm { WorkspaceId = "", Title = "未分组" },
                Icon = new FontIcon { Glyph = "\uE8A5" },
                IsExpanded = true,
            };
            Aut(groupItem, "UngroupedSessionsItem", L("未分组会话"));
            AppendSessionRows(unfiled, groupItem.MenuItems, "");
            Nav.MenuItems.Add(groupItem);
        }
        UpdateEmptyState();
        // 重建后恢复当前会话选中（RebuildNavMenu 会先清 SelectedItem；流帧刷新也会走到这里）
        RestoreActiveSessionSelection();
    }

    /// <summary>把侧栏选中态对齐到 _activeSessionId（找不到则不动，避免闪到设置/固定项）。
    /// 层级 NavigationView 里只写 Nav.SelectedItem 常不亮选中条：必须同时把叶子项 IsSelected=true。</summary>
    private void RestoreActiveSessionSelection()
    {
        try
        {
            var sid = Volatile.Read(ref _activeSessionId);
            if (sid is null)
            {
                return;
            }
            foreach (var obj in Nav.MenuItems)
            {
                if (obj is NavigationViewItem parent)
                {
                    // 层级项：先展开父分组，子项选中才可见/可导航
                    if (FindSessionNavItem(parent, sid) is { } hit)
                    {
                        ExpandNavAncestors(Nav.MenuItems, sid);
                        if (!ReferenceEquals(Nav.SelectedItem, hit))
                        {
                            Nav.SelectedItem = hit;
                        }
                        hit.IsSelected = true;
                        return;
                    }
                }
            }
        }
        catch (Exception) { }
    }

    private static void ExpandNavAncestors(IList<object> items, string sessionId)
    {
        foreach (var obj in items)
        {
            if (obj is not NavigationViewItem item)
            {
                continue;
            }
            if (item.Tag is SessionVm vm && vm.SessionId == sessionId)
            {
                return;
            }
            var childHit = FindSessionNavItem(item, sessionId);
            if (childHit is not null)
            {
                item.IsExpanded = true;
                ExpandNavAncestors(item.MenuItems, sessionId);
                return;
            }
        }
    }

    private static NavigationViewItem? FindSessionNavItem(object node, string sessionId)
    {
        if (node is NavigationViewItem item)
        {
            if (item.Tag is SessionVm vm && vm.SessionId == sessionId)
            {
                return item;
            }
            foreach (var child in item.MenuItems)
            {
                if (FindSessionNavItem(child, sessionId) is { } nested)
                {
                    return nested;
                }
            }
        }
        return null;
    }

    /// <summary>侧栏每个分组默认展示的会话条数；超出部分走"展开其余 N 个会话"（官方同形）。</summary>
    private const int SessionPreviewLimit = 5;

    /// <summary>
    /// 往一个分组里填会话行：手动排序时按内核给定顺序，最近更新时按 updatedAt 倒序；
    /// 超过 SessionPreviewLimit 且未展开过 → 追加"展开其余 N 个会话"条目（点击后展开全量）。
    /// </summary>
    private void AppendSessionRows(List<SessionVm> sessions, IList<object> host, string groupKey)
    {
        var ordered = _navOrderBy == "updated"
            ? sessions.OrderByDescending(s => s.UpdatedAt).ToList()
            : sessions.ToList();
        var expanded = _expandedGroups.Contains(groupKey);
        var shown = expanded ? ordered : ordered.Take(SessionPreviewLimit).ToList();
        foreach (var vm in shown)
        {
            host.Add(MakeSessionItem(vm));
        }
        var hidden = ordered.Count - shown.Count;
        if (hidden > 0)
        {
            var more = new NavigationViewItem
            {
                Content = LF("展开其余 {0} 个会话", hidden),
                SelectsOnInvoked = false,
                MinWidth = 0,
                Tag = ("expand-group", groupKey),
                Icon = new FontIcon { Glyph = "\uE70D" },
            };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(more, $"ExpandGroup_{groupKey}");
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(more, LF("展开其余 {0} 个会话", hidden));
            host.Add(more);
        }
        else if (expanded && ordered.Count > SessionPreviewLimit)
        {
            // 已展开且确实有收起的会话：给一条"收起"回路（官方 sessions.collapse）
            var less = new NavigationViewItem
            {
                Content = L("收起"),
                SelectsOnInvoked = false,
                MinWidth = 0,
                Tag = ("collapse-group", groupKey),
                Icon = new FontIcon { Glyph = "\uE70E" },
            };
            Aut(less, $"CollapseGroup_{groupKey}", L("收起已展开的会话"));
            host.Add(less);
        }
    }

    private NavigationViewItem MakeSessionItem(SessionVm s)
    {
        var item = new NavigationViewItem
        {
            MinWidth = 0,
            Tag = s,
        };
        // 会话项是侧栏最主要的导航目标，必须带稳定 AutomationId（供 UIA/自动化按会话定位）
        // 与可读 Name（标题，或在标题外带活动圆点时仍报标题本身）。
        // "新会话" 是壳显示哨兵（协议比较仍用原文），只在展示边界翻译。
        var shownTitle = s.Title == "新会话" ? L("新会话") : s.Title;
        // 子代理会话（内核 origin=subagent）：标题加"↳ "前缀 + 整行缩进，与父会话的
        // 从属关系一眼可辨（官方 SubagentHeaderLineage 的侧栏同语义）；UIA 名带"子代理"
        // 标记，读屏不丢身份。
        var displayTitle = s.IsSubagent ? "↳ " + shownTitle : shownTitle;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(item, $"Session_{s.SessionId}");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(item, s.IsSubagent ? L("子代理") + " · " + shownTitle : shownTitle);
        // 行内容 = 标题（占满）+ 活动圆点 + 右侧相对时间 + 更多钮
        // 选中条用 NavigationView 内置指示器（IsSelected 驱动），不另画竖条
        var row = new Grid { ColumnSpacing = TokenDouble("Space6", 6) };
        if (s.IsSubagent)
        {
            row.Margin = new Thickness(14, 0, 0, 0);
        }
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var titleText = new TextBlock
        {
            Text = displayTitle,
            TextTrimming = TextTrimming.CharacterEllipsis,
            TextWrapping = TextWrapping.NoWrap,
            VerticalAlignment = VerticalAlignment.Center,
        };
        // 运行中的会话：标题后跟活动圆点（api-session/status 实时维护，原版侧栏活动指示）。
        // 圆点走并列子元素而不是 TextBlock.Inlines 的 InlineUIContainer：后者在 NavigationView
        // 内容树里 Add 会抛 ArgumentException，每次重建侧栏都刷一屏 [ui-post] 异常 + 同步写日志，
        // 流式期间足以把逐字重绘的帧预算吃光。可读名仍由 item 上的 AutomationProperties.Name 给。
        var titleSlot = new Grid();
        titleSlot.Children.Add(titleText);
        lock (_sessionRunningLock)
        {
            if (_sessionRunning.TryGetValue(s.SessionId, out var isRunning) && isRunning)
            {
                var dot = new Microsoft.UI.Xaml.Shapes.Ellipse
                {
                    Width = StatusDot,
                    Height = StatusDot,
                    Fill = ThemeBrush("InfoBrush"),
                    HorizontalAlignment = HorizontalAlignment.Right,
                    VerticalAlignment = VerticalAlignment.Center,
                    Margin = new Thickness(0, 0, 2, 0),
                };
                titleText.Margin = new Thickness(0, 0, StatusDot + 6, 0);
                titleSlot.Children.Add(dot);
                StartStatusPulse(dot);
            }
        }
        Grid.SetColumn(titleSlot, 0);
        row.Children.Add(titleSlot);
        if (RelativeTimeLabel(s.UpdatedAt) is { Length: > 0 } ago)
        {
            var time = new TextBlock
            {
                Text = ago,
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextTertiaryBrush"),
                VerticalAlignment = VerticalAlignment.Center,
            };
            Grid.SetColumn(time, 1);
            row.Children.Add(time);
        }
        item.Content = row;

        // 选中时标题加重（视觉条由 NavigationView 内置指示器负责）
        void ApplySelectedVisual(bool selected)
        {
            titleText.FontWeight = selected
                ? Microsoft.UI.Text.FontWeights.SemiBold
                : Microsoft.UI.Text.FontWeights.Normal;
        }
        item.RegisterPropertyChangedCallback(
            SelectorItem.IsSelectedProperty,
            (_, _) => ApplySelectedVisual(item.IsSelected));
        ApplySelectedVisual(s.SessionId == Volatile.Read(ref _activeSessionId));

        // 会话操作菜单延迟到真正弹出时才构建（dsh 同构：重命名 / 分叉 / 归档 + 本壳扩展项）：
        // BuildSessionMenu 每项含 4-6 个 MenuFlyoutItem 与逐条 UIA 写入，侧栏重建时
        // 为全部会话预建纯属浪费——右键/点「…」的都是单个会话，按需构建即可。
        item.ContextRequested += (_, args) =>
        {
            try
            {
                args.Handled = true;
                var menu = BuildSessionMenu(s);
                if (args.TryGetPosition(item, out var at))
                {
                    menu.ShowAt(item, at);
                }
                else
                {
                    menu.ShowAt(item);
                }
            }
            catch (Exception) { }
        };
        // 行内「…」更多钮：与右键菜单同源，悬停可见（dsh 客户端同形）
        var moreBtn = new Button
        {
            Content = new FontIcon { Glyph = "", FontSize = GlyphCaption },
            Background = new Microsoft.UI.Xaml.Media.SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            Padding = new Thickness(2),
            MinWidth = 0,
            Width = 24,
            Height = 24,
            VerticalAlignment = VerticalAlignment.Center,
            Opacity = 0,
        };
        moreBtn.Click += (_, _) =>
        {
            try { BuildSessionMenu(s).ShowAt(moreBtn); }
            catch (Exception) { }
        };
        CornerRadius rad = new(TokenDouble("RadiusPill", 24));
        moreBtn.CornerRadius = rad;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(moreBtn, $"SessionMore_{s.SessionId}");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(moreBtn, LF("会话操作：{0}", shownTitle));
        ToolTipService.SetToolTip(moreBtn, L("会话操作"));
        row.PointerEntered += (_, _) => moreBtn.Opacity = 1;
        row.PointerExited += (_, _) => moreBtn.Opacity = 0;
        Grid.SetColumn(moreBtn, 2);
        row.Children.Add(moreBtn);
        return item;
    }

    /// <summary>运行指示圆点呼吸：Opacity 1→0.45 往返，1.2s 单程。这是状态动效而非进度控件——
    /// 运行中的会话不阻塞交互（可切去别处），按官方 progress controls 规范不能用
    /// 表阻塞语义的 ProgressRing；Opacity 走合成线程独立动画，不抢逐字重绘帧预算。
    /// 系统「动画效果」开关（辅助功能/前庭功能障碍用户依赖）关闭时保持静态圆点。</summary>
    private static void StartStatusPulse(Microsoft.UI.Xaml.Shapes.Ellipse dot)
    {
        if (dot.Tag is Storyboard || !SystemAnimationsEnabled())
        {
            return;
        }
        try
        {
            var breathe = new DoubleAnimation
            {
                From = 1,
                To = 0.45,
                Duration = new Duration(TimeSpan.FromSeconds(1.2)),
                AutoReverse = true,
                RepeatBehavior = RepeatBehavior.Forever,
            };
            Storyboard.SetTarget(breathe, dot);
            Storyboard.SetTargetProperty(breathe, "Opacity");
            var sb = new Storyboard();
            sb.Children.Add(breathe);
            dot.Tag = sb;
            // 侧栏重建会丢弃旧项：不主动停的话 Storyboard 会持有已离树的元素一直跑（野跑）。
            dot.Unloaded += static (sender, _) =>
            {
                if (sender is not Microsoft.UI.Xaml.Shapes.Ellipse { Tag: Storyboard running } gone)
                {
                    return;
                }
                try
                {
                    running.Stop();
                }
                catch (Exception)
                {
                }
                gone.Tag = null;
            };
            sb.Begin();
        }
        catch (Exception)
        {
            // 动画起不来不影响状态表达：圆点本身已说明会话在运行
            dot.Tag = null;
        }
    }

    private MenuFlyout BuildSessionMenu(SessionVm s)
    {
        var menu = new MenuFlyout();

        var renameItem = new MenuFlyoutItem { Text = L("重命名"), Tag = s };
        Aut(renameItem, "RenameSessionMenuItem", L("重命名会话"));
        renameItem.Click += (_, _) => _ = RenameSessionAsync(s);
        menu.Items.Add(renameItem);

        var fork = new MenuFlyoutItem { Text = L("分叉会话"), Tag = s };
        Aut(fork, "ForkSessionMenuItem", L("分叉会话"));
        fork.Click += (_, _) => _ = ForkSessionAsync(s);
        menu.Items.Add(fork);

        var archiveItem = new MenuFlyoutItem { Text = L("归档会话"), Tag = s };
        Aut(archiveItem, "ArchiveSessionMenuItem", L("归档会话"));
        archiveItem.Click += (_, _) => _ = ArchiveSessionAsync(s);
        menu.Items.Add(archiveItem);

        if (_workspaces.Count > 0)
        {
            var moveTo = new MenuFlyoutSubItem { Text = L("移动到工作区") };
            foreach (var ws in _workspaces)
            {
                var target = new MenuFlyoutItem { Text = ws.Title, Tag = (ws, s) };
                Aut(target, $"MoveSessionTo_{ws.WorkspaceId}", LF("移动到工作区：{0}", ws.Title));
                target.Click += (_, _) => _ = MoveSessionToWorkspaceAsync(ws, s);
                moveTo.Items.Add(target);
            }
            menu.Items.Add(moveTo);
        }

        var cancel = new MenuFlyoutItem { Text = L("取消运行"), Tag = s };
        Aut(cancel, "CancelRunMenuItem", L("取消运行"));
        cancel.Click += (_, _) => _ = CancelSessionAsync(s);
        menu.Items.Add(cancel);

        return menu;
    }

    private async Task RenameSessionAsync(SessionVm s)
    {
        if (_rpc is null)
        {
            return;
        }
        var box = Aut(new TextBox { Header = L("新标题"), Text = s.Title, MinWidth = TokenDouble("DialogMinWidthCompact", 360) }, "RenameSessionTextBox", L("新标题"));
        var dialog = new ContentDialog
        {
            Title = L("重命名会话"),
            Content = box,
            PrimaryButtonText = L("确定"),
            CloseButtonText = L("取消"),
            DefaultButton = ContentDialogButton.Primary,
            XamlRoot = Content.XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }
        try
        {
            await _rpc.CallOkAsync("session/rename", new { request = new { sessionId = s.SessionId, title = box.Text.Trim() } });
            _ = RefreshAllListsAsync();
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("重命名失败：{0}", ex.Message));
        }
    }

    private async Task RenameWorkspaceAsync(WorkspaceVm ws)
    {
        if (_rpc is null || string.IsNullOrEmpty(ws.WorkspaceId))
        {
            return;
        }
        var box = Aut(new TextBox { Header = L("新名称"), Text = ws.Title, MinWidth = TokenDouble("DialogMinWidthCompact", 360) }, "RenameWorkspaceTextBox", L("新名称"));
        var dialog = new ContentDialog
        {
            Title = L("重命名工作区"),
            Content = box,
            PrimaryButtonText = L("确定"),
            CloseButtonText = L("取消"),
            DefaultButton = ContentDialogButton.Primary,
            XamlRoot = Content.XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }
        try
        {
            await _rpc.CallOkAsync("workspace/rename", new { request = new { workspaceId = ws.WorkspaceId, title = box.Text.Trim() } });
            _ = RefreshAllListsAsync();
        }
        catch (DshRpcException ex)
        {
            // 0.7.1 教训：参数键错时这里曾静默吞错——工作区操作失败必须可见
            _ = ShowErrorAsync(LF("重命名失败：{0}", ex.Message));
        }
    }

    private async Task DeleteWorkspaceAsync(WorkspaceVm ws)
    {
        if (_rpc is null || string.IsNullOrEmpty(ws.WorkspaceId))
        {
            return;
        }
        var dialog = new ContentDialog
        {
            Title = L("删除工作区"),
            Content = LF("将把“{0}”从工作区列表中移除。文件夹与会话记录会保留，其会话将显示在“未分组”下。", ws.Title),
            PrimaryButtonText = L("删除"),
            CloseButtonText = L("取消"),
            XamlRoot = Content.XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }
        try
        {
            await _rpc.CallOkAsync("workspace/delete", new { request = new { workspaceId = ws.WorkspaceId } });
            _ = RefreshAllListsAsync();
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("删除失败：{0}", ex.Message));
        }
    }

    private async Task CancelSessionAsync(SessionVm s)
    {
        if (_rpc is null)
        {
            return;
        }
        try
        {
            // 0.7.0 探针实测：cancel 的参数键是 request（_request 会 arguments-invalid）
            await _rpc.CallOkAsync("session/cancel", new { request = new { sessionId = s.SessionId } });
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("取消失败：{0}", ex.Message));
        }
    }

    /// <summary>新建工作区：内核目录选择器（directoryPicker/pick）为主，手输路径为回退；
    /// 内核目录浏览（directoryPicker/list + createDirectory）在具备 browse 能力的部署上提供
    /// 目录树/新建文件夹；本部署由原生选择器服务，这两个端点在打开时返回
    /// directory-picker/unavailable，此时按能力探测结果收起浏览区并说明原因。
    /// 返回新建的工作区视图（取消/失败返回 null）：调用方可据此直接选中它。</summary>
    private async Task<WorkspaceVm?> CreateWorkspaceAsync()
    {
        if (_rpc is null)
        {
            return null;
        }
        var box = Aut(new TextBox { Header = L("文件夹路径"), Text = DefaultSessionCwd(), MinWidth = TokenDouble("DialogMinWidth", 420) }, "NewWorkspacePathTextBox", L("文件夹路径"));

        var browseButton = Aut(new Button { Content = L("浏览…（内核选择器）") }, "BrowseDirectoryButton", L("浏览文件夹"));
        var host = new StackPanel { Spacing = Sp10, MinWidth = TokenDouble("DialogMinWidth", 420) };
        var browseRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp8 };
        browseRow.Children.Add(browseButton);
        var browser = new StackPanel { Spacing = Sp6, Visibility = Visibility.Collapsed };
        var capabilityNote = new TextBlock
        {
            Style = AppStyle("CaptionTextStyle"),
            TextWrapping = TextWrapping.Wrap,
            Foreground = ThemeBrush("TextSecondaryBrush"),
            Visibility = Visibility.Collapsed,
        };
        host.Children.Add(box);
        host.Children.Add(browseRow);
        host.Children.Add(capabilityNote);
        host.Children.Add(browser);

        // 内核选择器（原生对话框）：选中即回填路径；取消返回 null（保留手输值）
        browseButton.Click += async (_, _) =>
        {
            try
            {
                var picked = await _rpc.CallOkAsync("directoryPicker/pick", new { });
                if (picked.ValueKind == JsonValueKind.String && picked.GetString() is { Length: > 0 } path)
                {
                    box.Text = path;
                }
            }
            catch (DshRpcException ex)
            {
                _ = ShowErrorAsync(LF("内核目录选择器不可用：{0}", ex.Message));
            }
        };

        var dialog = new ContentDialog
        {
            Title = L("新建工作区"),
            Content = host,
            PrimaryButtonText = L("创建"),
            CloseButtonText = L("取消"),
            DefaultButton = ContentDialogButton.Primary,
            XamlRoot = Content.XamlRoot,
        };

        // 打开即探测内核目录浏览能力（directoryPicker/list 无 path 时用内核默认起点）
        _ = LoadDirectoryBrowserAsync(browser, capabilityNote, box, _ => { });
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return null;
        }
        try
        {
            // create 的 descriptor 是 request:{path}（0.7.1 实测：裸 {path} 会被
            // zod strict codec 拒成 arguments-invalid——曾致"新建工作区"从未工作过）
            var value = await _rpc.CallOkAsync("workspace/create", new { request = new { path = box.Text.Trim() } });
            _ = RefreshAllListsAsync();
            // 回执带 WorkspaceView（workspace/create 的 WorkspaceCreateValue）
            return value.TryGetProperty("workspace", out var w) && w.ValueKind == JsonValueKind.Object
                ? new WorkspaceVm
                {
                    WorkspaceId = Str(w, "workspaceId"),
                    Title = Str(w, "title"),
                    Path = Str(w, "path"),
                }
                : null;
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("创建失败：{0}", ex.Message));
            return null;
        }
    }

    /// <summary>
    /// 内核目录浏览器（directoryPicker/list + createDirectory）：
    /// 打开时先 list 探测能力——本部署由原生选择器（native）服务，list/createDirectory 会回
    /// directory-picker/unavailable，此时隐藏浏览区并说明"用浏览…按钮（原生选择器）"。
    /// 面包屑来自 list 的 crumbs，条目点击进入子目录，当前目录可回填路径或建子目录。
    /// </summary>
    private async Task LoadDirectoryBrowserAsync(
        StackPanel browser, TextBlock note, TextBox pathBox, Action<string> onPick)
    {
        if (_rpc is null)
        {
            return;
        }
        JsonElement listing;
        try
        {
            listing = await _rpc.CallOkAsync("directoryPicker/list", new { });
        }
        catch (DshRpcException ex)
        {
            note.Text = ex.Message.Contains("directory-picker/unavailable", StringComparison.OrdinalIgnoreCase)
                ? L("内核目录浏览不可用：本部署使用原生目录选择器，请用「浏览…（内核选择器）」按钮选取文件夹，或直接输入路径。")
                : LF("内核目录浏览不可用：{0}", ex.Message);
            note.Visibility = Visibility.Visible;
            browser.Visibility = Visibility.Collapsed;
            return;
        }
        note.Visibility = Visibility.Collapsed;
        browser.Visibility = Visibility.Visible;
        RenderDirectoryListing(browser, listing, pathBox, onPick);
    }

    private void RenderDirectoryListing(StackPanel browser, JsonElement listing, TextBox pathBox, Action<string> onPick)
    {
        browser.Children.Clear();
        var current = listing.TryGetProperty("path", out var p) ? p.GetString() ?? "" : "";
        browser.Children.Add(new TextBlock
        {
            Text = current,
            Style = AppStyle("CaptionTextStyle"),
            Foreground = ThemeBrush("TextTertiaryBrush"),
            TextTrimming = TextTrimming.CharacterEllipsis,
        });

        // 面包屑（内核给的分段：name/path/hidden）
        var crumbs = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp4 };
        if (listing.TryGetProperty("crumbs", out var crumbList) && crumbList.ValueKind == JsonValueKind.Array)
        {
            foreach (var crumb in crumbList.EnumerateArray())
            {
                var crumbPath = crumb.TryGetProperty("path", out var cp) ? cp.GetString() ?? "" : "";
                var crumbName = crumb.TryGetProperty("name", out var cn) ? cn.GetString() ?? "" : "";
                if (crumbPath.Length == 0)
                {
                    continue;
                }
                var link = new Button
                {
                    Content = crumbName.Length > 0 ? crumbName : crumbPath,
                    Style = AppStyle("CompactButtonStyle"),
                };
                Aut(link, $"DirectoryCrumb_{crumbPath}", LF("跳转到 {0}", crumbName.Length > 0 ? crumbName : crumbPath));
                link.Click += async (_, _) => await NavigateDirectoryAsync(browser, pathBox, onPick, crumbPath);
                crumbs.Children.Add(link);
            }
        }
        browser.Children.Add(crumbs);

        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp8 };
        var use = Aut(new Button { Content = L("使用此目录"), Style = AppStyle("CompactButtonStyle") }, "UseDirectoryButton", L("使用此目录"));
        use.Click += (_, _) =>
        {
            pathBox.Text = current;
            onPick(current);
        };
        actions.Children.Add(use);
        var mkdir = Aut(new Button { Content = L("新建文件夹…"), Style = AppStyle("CompactButtonStyle"), IsEnabled = current.Length > 0 }, "NewFolderButton", L("新建文件夹"));
        mkdir.Click += async (_, _) => await CreateDirectoryAsync(browser, pathBox, onPick, current);
        actions.Children.Add(mkdir);
        browser.Children.Add(actions);

        var entries = new ListView
        {
            MaxHeight = 200,
            SelectionMode = ListViewSelectionMode.None,
            // 目录项要能"进入"：IsItemClickEnabled + ItemClick 让每行具备键盘 Enter/Space 触发路径
            // （纯 Tapped 手势只有鼠标/触摸能用，键盘用户进不去子目录）。
            IsItemClickEnabled = true,
        };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(entries, "DirectoryEntriesList");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(entries, L("目录项"));
        entries.ItemClick += async (_, e) =>
        {
            if (e.ClickedItem is ListViewItem { Tag: string target })
            {
                await NavigateDirectoryAsync(browser, pathBox, onPick, target);
            }
        };
        if (listing.TryGetProperty("entries", out var entryList) && entryList.ValueKind == JsonValueKind.Array)
        {
            foreach (var entry in entryList.EnumerateArray())
            {
                var name = entry.TryGetProperty("name", out var en) ? en.GetString() ?? "" : "";
                var entryPath = entry.TryGetProperty("path", out var ep) ? ep.GetString() ?? "" : "";
                if (entryPath.Length == 0)
                {
                    continue;
                }
                // 行高用 ListViewItem 默认值（≥32 的触控目标），不再压到 0
                entries.Items.Add(new ListViewItem { Content = name, Tag = entryPath });
            }
        }
        browser.Children.Add(entries);
        if (listing.TryGetProperty("truncated", out var tr) && tr.ValueKind == JsonValueKind.True)
        {
            browser.Children.Add(new TextBlock
            {
                Text = L("目录项超过内核上限，仅显示前若干项。"),
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextTertiaryBrush"),
            });
        }
    }

    private async Task NavigateDirectoryAsync(StackPanel browser, TextBox pathBox, Action<string> onPick, string path)
    {
        if (_rpc is null)
        {
            return;
        }
        try
        {
            var listing = await _rpc.CallOkAsync("directoryPicker/list", new { path });
            RenderDirectoryListing(browser, listing, pathBox, onPick);
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("读取目录失败：{0}", ex.Message));
        }
    }

    /// <summary>内核侧新建目录（directoryPicker/createDirectory：path + name → 新目录路径）。</summary>
    private async Task CreateDirectoryAsync(StackPanel browser, TextBox pathBox, Action<string> onPick, string parent)
    {
        if (_rpc is null)
        {
            return;
        }
        var box = Aut(new TextBox { Header = L("文件夹名"), PlaceholderText = "new-folder" }, "NewFolderNameTextBox", L("文件夹名"));
        var dialog = new ContentDialog
        {
            Title = L("新建文件夹"),
            Content = box,
            PrimaryButtonText = L("创建"),
            CloseButtonText = L("取消"),
            DefaultButton = ContentDialogButton.Primary,
            XamlRoot = Content.XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }
        var name = box.Text.Trim();
        if (name.Length == 0)
        {
            return;
        }
        try
        {
            var created = await _rpc.CallOkAsync("directoryPicker/createDirectory", new { path = parent, name });
            if (created.ValueKind == JsonValueKind.String && created.GetString() is { Length: > 0 } newPath)
            {
                pathBox.Text = newPath;
                await NavigateDirectoryAsync(browser, pathBox, onPick, newPath);
            }
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("新建文件夹失败：{0}", ex.Message));
        }
    }

    /// <summary>打开工作区路径（session/openWorkspacePath：request{path, action?} → {opened:true}）。
    /// 是否可用由 session/canOpenWorkspacePath（无参数 → bool）判定，不可用时按钮禁用并说明原因。</summary>
    private async Task OpenWorkspacePathAsync(WorkspaceVm ws)
    {
        if (_rpc is null)
        {
            return;
        }
        if (ws.Path.Length == 0)
        {
            _ = ShowErrorAsync(L("该工作区没有路径记录。"));
            return;
        }
        try
        {
            if (!await CanOpenWorkspacePathAsync())
            {
                _ = ShowErrorAsync(L("本部署无法在内核主机打开路径（session/canOpenWorkspacePath=false）。"));
                return;
            }
            await _rpc.CallOkAsync("session/openWorkspacePath", new { request = new { path = ws.Path, action = "reveal" } });
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("打开路径失败：{0}", ex.Message));
        }
    }

    /// <summary>session/canOpenWorkspacePath（无参数 → bool）；结果缓存，失败按不可用处理。</summary>
    private bool? _canOpenWorkspacePath;

    private async Task<bool> CanOpenWorkspacePathAsync()
    {
        if (_rpc is null)
        {
            return false;
        }
        if (_canOpenWorkspacePath is { } cached)
        {
            return cached;
        }
        try
        {
            var value = await _rpc.CallOkAsync("session/canOpenWorkspacePath", new { });
            _canOpenWorkspacePath = value.ValueKind == JsonValueKind.True;
        }
        catch (DshRpcException)
        {
            _canOpenWorkspacePath = false;
        }
        return _canOpenWorkspacePath ?? false;
    }

    /// <summary>工作区排序（workspace/insertBefore：request{workspaceId, beforeWorkspaceId?} → {workspaceIds}）。
    /// 上移 = 插到前一个工作区之前；下移 = 插到后一个工作区之后（末位则省略 beforeWorkspaceId 追加到尾部）。</summary>
    private async Task MoveWorkspaceAsync(WorkspaceVm ws, int delta)
    {
        if (_rpc is null)
        {
            return;
        }
        var index = _workspaces.FindIndex(w => w.WorkspaceId == ws.WorkspaceId);
        if (index < 0)
        {
            return;
        }
        var target = index + delta;
        if (target < 0 || target >= _workspaces.Count)
        {
            return; // 已在端点，无操作
        }
        try
        {
            var beforeId = delta < 0
                ? _workspaces[target].WorkspaceId
                : target + 1 < _workspaces.Count ? _workspaces[target + 1].WorkspaceId : null;
            object args = beforeId is null
                ? new { request = new { workspaceId = ws.WorkspaceId } }
                : new { request = new { workspaceId = ws.WorkspaceId, beforeWorkspaceId = beforeId } };
            await _rpc.CallOkAsync("workspace/insertBefore", args);
            await RefreshWorkspacesAsync();
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("调整顺序失败：{0}", ex.Message));
        }
    }

    private async Task ArchiveSessionAsync(SessionVm s)
    {
        if (_rpc is null)
        {
            return;
        }
        try
        {
            await _rpc.CallOkAsync("workspace/archiveSession", new { request = new { sessionId = s.SessionId } });
            await RefreshWorkspacesAsync();
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("归档失败：{0}", ex.Message));
        }
    }

    private async Task MoveSessionToWorkspaceAsync(WorkspaceVm ws, SessionVm s)
    {
        if (_rpc is null)
        {
            return;
        }
        try
        {
            // insertSessionBefore(workspaceId, sessionId)：加入目标工作区（不加 before 则追加尾部）
            await _rpc.CallOkAsync("workspace/insertSessionBefore", new { request = new { workspaceId = ws.WorkspaceId, sessionId = s.SessionId } });
            await RefreshWorkspacesAsync();
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("移动失败：{0}", ex.Message));
        }
    }

    private async Task ForkSessionAsync(SessionVm s)
    {
        if (_rpc is null)
        {
            return;
        }
        try
        {
            // 0.7.0 探针实测：fork 参数键是 request（_request 会 arguments-invalid）
            await _rpc.CallOkAsync("session/fork", new { request = new { sessionId = s.SessionId } });
            await RefreshSessionsAsync();
            await RefreshWorkspacesAsync();
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("分叉失败：{0}", ex.Message));
        }
    }

    /// <summary>全量刷新工作区树（workspace/follow baseline 或列表重建兜底）。</summary>
    /// <summary>
    /// 工作区树：单条 workspace/follow 流长驻，baseline 建树、增量帧（upsert/remove/
    /// order/archived）在原流内局部应用。绝不能在增量帧里重开流——每次重开旧流不死，
    /// 事件一多即流风暴（曾拖死整个壳）。
    /// </summary>
    private bool _workspaceStreamOpen;

    private async Task RefreshWorkspacesAsync()
    {
        if (_rpc is null || _workspaceStreamOpen)
        {
            return;
        }
        _workspaceStreamOpen = true;
        try
        {
            await _rpc.OpenWorkspaceFollowStreamAsync(frame =>
            {
                var type = frame.TryGetProperty("type", out var t) ? t.GetString() : null;
                switch (type)
                {
                    case "baseline":
                    {
                        var items = frame.GetProperty("value").GetProperty("items");
                        var list = new List<WorkspaceVm>();
                        foreach (var w in items.EnumerateArray())
                        {
                            list.Add(new WorkspaceVm
                            {
                                WorkspaceId = w.GetProperty("workspaceId").GetString() ?? "",
                                Title = w.GetProperty("title").GetString() ?? "",
                                Path = w.GetProperty("path").GetString() ?? "",
                                SessionIds = w.TryGetProperty("sessionIds", out var sids) && sids.ValueKind == JsonValueKind.Array
                                    ? sids.EnumerateArray().Select(x => x.GetString() ?? "").ToList()
                                    : new List<string>(),
                            });
                        }
                        var archived = frame.GetProperty("value").TryGetProperty("archivedSessionIds", out var ar) && ar.ValueKind == JsonValueKind.Array
                            ? ar.EnumerateArray().Select(x => x.GetString() ?? "").ToList()
                            : new List<string>();
                        PostUi(() =>
                        {
                            _workspaces.Clear();
                            _workspaces.AddRange(list);
                            _archivedSessions.Clear();
                            _archivedSessions.AddRange(archived);
                            RebuildNavMenu();
                        });
                        break;
                    }
                    case "upsert":
                    {
                        var w = frame.GetProperty("workspace");
                        var id = w.GetProperty("workspaceId").GetString() ?? "";
                        var vm = new WorkspaceVm
                        {
                            WorkspaceId = id,
                            Title = w.GetProperty("title").GetString() ?? "",
                            Path = w.GetProperty("path").GetString() ?? "",
                            SessionIds = w.TryGetProperty("sessionIds", out var sids) && sids.ValueKind == JsonValueKind.Array
                                ? sids.EnumerateArray().Select(x => x.GetString() ?? "").ToList()
                                : new List<string>(),
                        };
                        PostUi(() =>
                        {
                            var old = _workspaces.FindIndex(x => x.WorkspaceId == id);
                            if (old >= 0)
                            {
                                _workspaces[old] = vm;
                            }
                            else
                            {
                                _workspaces.Add(vm);
                            }
                            RebuildNavMenu();
                        });
                        break;
                    }
                    case "remove":
                    {
                        var id = frame.GetProperty("workspaceId").GetString() ?? "";
                        PostUi(() =>
                        {
                            _workspaces.RemoveAll(x => x.WorkspaceId == id);
                            RebuildNavMenu();
                        });
                        break;
                    }
                    case "order":
                    {
                        var ids = frame.GetProperty("workspaceIds").EnumerateArray().Select(x => x.GetString() ?? "").ToList();
                        PostUi(() =>
                        {
                            _workspaces.Sort((a, b) => ids.IndexOf(a.WorkspaceId) - ids.IndexOf(b.WorkspaceId));
                            RebuildNavMenu();
                        });
                        break;
                    }
                    case "archived":
                    {
                        var ids = frame.GetProperty("archivedSessionIds").EnumerateArray().Select(x => x.GetString() ?? "").ToList();
                        PostUi(() =>
                        {
                            _archivedSessions.Clear();
                            _archivedSessions.AddRange(ids);
                            RebuildNavMenu();
                        });
                        break;
                    }
                }
            });
        }
        catch (Exception)
        {
            // 流断开时允许下次重建（含 mux 重连后的恢复）。
            _workspaceStreamOpen = false;
        }
    }

    /// <summary>侧栏项点击（标准 ItemInvoked）：统一按 Tag 判别路由——固定项 =
    /// "settings"/"new-session"/"workspace-section" 标记字符串（文本路由会与会话标题撞名，
    /// 如用户把会话命名为"设置"）；设置分区项 = SettingsSectionVm；分组展开/收起 =
    /// ("expand-group"|"collapse-group", key)；会话项由 SelectionChanged 驱动。</summary>
    private async void OnNavItemInvoked(NavigationView sender, NavigationViewItemInvokedEventArgs args)
    {
        try
        {
            switch (args.InvokedItemContainer?.Tag)
            {
                case "settings":
                    ToggleSettingsPage();
                    return;
                case SettingsSectionVm section:
                    await ActivateSectionAsync(section.Id);
                    return;
                case "new-session":
                    await CreateSessionAsync();
                    return;
                case SessionVm sessionVm:
                {
                    // 层级 NavigationView：选中子项时 SelectionChanged 常把父节点当 SelectedItem，
                    // 会话路由必须走 ItemInvoked 的 InvokedItemContainer.Tag
                    if (sessionVm.SessionId != _activeSessionId)
                    {
                        Volatile.Write(ref _activeSessionId, sessionVm.SessionId);
                        await OpenSessionAsync(sessionVm);
                    }
                    return;
                }
                case "workspace-section":
                    return; // 段标题本身不可点（视图选项/加号是它内部的标准按钮）
                case ValueTuple<string, string> expand when expand.Item1 == "expand-group":
                    _expandedGroups.Add(expand.Item2);
                    RebuildNavMenu();
                    return;
                case ValueTuple<string, string> collapse when collapse.Item1 == "collapse-group":
                    _expandedGroups.Remove(collapse.Item2);
                    RebuildNavMenu();
                    return;
            }
        }
        catch (Exception) { } // async void 事件入口兜底（0xc000027b 教训）
    }

    /// <summary>NavigationView 选中变化（含程序化/UIA 选择）：会话项加载会话。</summary>
    private async void OnNavSelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (args.SelectedItem is NavigationViewItem item && item.Tag is SessionVm vm)
        {
            if (vm.SessionId != _activeSessionId)
            {
                Volatile.Write(ref _activeSessionId, vm.SessionId);
                await OpenSessionAsync(vm);
            }
        }
    }

    /// <summary>选中会话：清屏、拉历史、开跟随流；附属视图（文件右栏/反馈状态/命令目录）随会话切换。</summary>
    private async Task OpenSessionAsync(SessionVm vm)
    {
        Volatile.Write(ref _journalCursor, 0);
        _messages.Clear();
        // 模型标注同属会话作用域：旧会话最后一条 header 不许标到新会话的消息上，
        // 清零后由本次回放里的 request/header 重新建立。
        _lastHeaderModel = "";
        _pendingUserBubble = null;
        UpdateEmptyState(); // 清屏即回空态（主区品牌标 + 输入区选择器）
        // 会话切换即重置会话作用域的辅助状态：反馈条目属于原会话，命令目录按 agent 作用域缓存
        ResetFeedbackState();
        ResetRunStats(); // 运行状态条：清零后由随后的完整 transcript 回放重建，避免翻页重复累计
        ResetContextMeter(); // 上下文容量：同样清零后由回放重建（不残留上个会话的容量/采样）
        _renderedAttachmentIds.Clear(); // 历史图片按会话去重
        _localImageEchoes.Clear(); // 本地图片回显登记同属会话作用域
        ResetTurnRail(); // 历史快速定位 rail：清掉旧会话刻度，随后由 turnOutline 快照重建
        _commandsSession = null;
        _commandsError = null;
        _commandsErrorAt = null;
        HideCommandPalette();
        HideReferencePalette();
        FilesPanelView.SetSession(vm.SessionId, vm.Cwd);
        // 队列/作业面板跟随会话（session/control 已缓存各会话状态）
        RefreshQueuePanel();
        RefreshJobsPanel();
        // 会话头（标题/子代理回跳/只读态）与目标条跟随会话：目标条异步拉取，失败自收起
        RefreshSessionHeader();
        // 先复位再拉取：目标/计划投影缓存按会话作用域，旧会话的值不许残留到新会话
        lock (_goalLock)
        {
            _goalSummary = null;
        }
        lock (_scheduleLock)
        {
            _scheduleRecords = new();
            _scheduleSeen = false;
        }
        _ = RefreshGoalBarAsync(vm.SessionId);
        // 会话状态条（plan/permissions 投影）跟随会话：先清空再按新会话重取，
        // 避免上一个会话的预设标签停留在新会话上。
        lock (_projectionLock)
        {
            _planActive = null;
            _planPending = null;
            _permissionCurrentValue = null;
        }
        RefreshSessionStateBar();
        await RefreshSessionStateFromListAsync(vm.SessionId);
        await LoadSessionHistoryAsync(vm.SessionId);
        await FollowSessionAsync(vm.SessionId);
        await LoadFeedbackAsync(vm.SessionId);
        // 控制流在 mux 重连后会被作废：切会话时补开一次（幂等）
        await OpenSessionControlStreamAsync();
        UpdateEmptyState(); // 历史回读完成：有消息则收起空态
    }

    /// <summary>
    /// 补取契约：session/control 的 baseline 只在开流时推一次；壳若在 baseline 之后
    /// 才切会话，新会话的 plan/permissions 投影不会自动到齐。会话投影同样挂在
    /// session/list 的 projections.values 上（内核惰性缓存），这里做一次补齐——
    /// 优先读 RefreshSessionsAsync 维护的缓存，未命中（冷启动首次打开）才拉清单。
    /// </summary>
    private async Task RefreshSessionStateFromListAsync(string sessionId)
    {
        if (_rpc is null)
        {
            return;
        }
        if (_sessionProjections.TryGetValue(sessionId, out var cached) && cached.ValueKind == JsonValueKind.Object)
        {
            lock (_projectionLock)
            {
                ApplyProjectionValues(cached);
            }
            PostUi(RefreshSessionStateBar);
            return;
        }
        try
        {
            var value = await _rpc.CallOkAsync("session/list", new { _request = new { } });
            if (!value.TryGetProperty("items", out var items) || items.ValueKind != JsonValueKind.Array)
            {
                return;
            }
            foreach (var item in items.EnumerateArray())
            {
                if (Str(item, "sessionId") != sessionId)
                {
                    continue;
                }
                if (item.TryGetProperty("projections", out var proj) && proj.ValueKind == JsonValueKind.Object &&
                    proj.TryGetProperty("values", out var values) && values.ValueKind == JsonValueKind.Object)
                {
                    _sessionProjections[sessionId] = values.Clone();
                    lock (_projectionLock)
                    {
                        ApplyProjectionValues(values);
                    }
                }
                break;
            }
        }
        catch (Exception)
        {
            // 投影补齐失败不阻塞会话打开（状态条保持收起，不显示猜测值）
        }
        PostUi(RefreshSessionStateBar);
    }

    // ---------------- 会话操作 ----------------

    /// <summary>新建会话的 cwd：MSIX 打包应用进程 CWD 是 system32（不可写，工具全废），
    /// 必须显式给内核一个可写目录——用户文档目录。</summary>
    private static string DefaultSessionCwd() =>
        Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments);

    /// <summary>
    /// session/create（内核 SessionCreateRequest）：identity + location + Agent preset 三件套。
    /// 工作区来自输入区上方的选择器（workspaceId，官方首选）；未选时回落显式 cwd（文档目录）。
    /// 返回新会话 id。
    /// </summary>
    private async Task<string> CreateSessionCoreAsync()
    {
        if (_rpc is null)
        {
            throw new InvalidOperationException(L("内核未连接"));
        }
        Dictionary<string, object> request = new();
        if (_pendingWorkspace is { Id.Length: > 0 } ws)
        {
            request["workspaceId"] = ws.Id;
        }
        else
        {
            request["cwd"] = DefaultSessionCwd();
        }
        if (_pendingPreset is { Id.Length: > 0 } preset)
        {
            request["agentPreset"] = preset.Id;
        }
        var value = await _rpc.CallOkAsync("session/create", new { request });
        // 响应缺 sessionId / 非字符串：归一成 DshRpcException，防裸异常沿 async void 上抛闪退
        if (!value.TryGetProperty("sessionId", out var sidEl) || sidEl.ValueKind != JsonValueKind.String)
        {
            throw new DshRpcException("bad-response", L("会话创建响应缺少 sessionId。"));
        }
        return sidEl.GetString()!;
    }

    private async Task CreateSessionAsync()
    {
        if (_rpc is null)
        {
            return;
        }
        try
        {
            // 已有空会话（内核 blank / 本地无消息）：不再另建，只保当前选中
            if (_activeSessionId is { Length: > 0 } existing &&
                _messages.Count == 0 &&
                _sessions.FirstOrDefault(s => s.SessionId == existing) is { } cur &&
                cur.Blank)
            {
                PostUi(() =>
                {
                    RebuildNavMenu();
                    RestoreActiveSessionSelection();
                });
                return;
            }
            // 活动会话本地无消息（历史未回读完或刚清屏）也视为可复用的空会话
            if (_activeSessionId is { Length: > 0 } existing2 &&
                _messages.Count == 0 &&
                _sessions.Any(s => s.SessionId == existing2))
            {
                PostUi(() =>
                {
                    RebuildNavMenu();
                    RestoreActiveSessionSelection();
                });
                return;
            }

            var sid = await CreateSessionCoreAsync();
            await RefreshSessionsAsync();
            // 刷新后列表里可能还没有这条（follow 流延迟）：本地合成兜底，保证能选中并打开
            var vm = _sessions.FirstOrDefault(s => s.SessionId == sid)
                     ?? new SessionVm
                     {
                         SessionId = sid,
                         Title = "新会话",
                         Blank = true,
                         UpdatedAt = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(),
                     };
            if (!_sessions.Any(s => s.SessionId == sid))
            {
                _sessions.Insert(0, vm);
            }
            Volatile.Write(ref _activeSessionId, sid);
            PostUi(() =>
            {
                RebuildNavMenu();
                RestoreActiveSessionSelection();
                _ = OpenSessionAsync(vm);
            });
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("新建会话失败：{0}", ex.Message));
        }
    }

    // ---------------- 输入区选择器（工作区 + Agent 模式） ----------------

    /// <summary>
    /// 输入区动作行里的两个选择器：工作区 + Agent 模式（第 6 条已从输入卡上方的独立
    /// 选择器行迁入动作行，紧邻权限胶囊）。可见性联动与迁移前完全等价：
    /// 只在"即将开始新会话"时出现（无活动会话 / 空会话），有会话时收起——
    /// 内核侧会话的工作区与预设开始时即固定（agent-preset/locked）。
    /// </summary>
    private void UpdateComposerSelectors()
    {
        try
        {
            var hero = IsHeroState();
            var visibility = hero ? Visibility.Visible : Visibility.Collapsed;
            WorkspacePickerButton.Visibility = visibility;
            AgentModeButton.Visibility = visibility;
            ApplyComposerAdaptiveLayout(force: true);
            if (!hero)
            {
                return;
            }
            WorkspaceChipLabel.Text = _pendingWorkspace?.Title is { Length: > 0 } t ? t : "选择工作区";
            WorkspacePickerFlyout.Items.Clear();
            var defaultItem = new RadioMenuFlyoutItem
            {
                Text = L("文档目录（默认）"),
                GroupName = "workspace-pick",
                IsChecked = _pendingWorkspace is null,
            };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(defaultItem, "WorkspacePickDefault");
            defaultItem.Click += (_, _) => PickWorkspace(null);
            WorkspacePickerFlyout.Items.Add(defaultItem);
            foreach (var ws in _workspaces)
            {
                var item = new RadioMenuFlyoutItem
                {
                    Text = ws.Title,
                    Tag = ws,
                    GroupName = "workspace-pick",
                    IsChecked = _pendingWorkspace?.Id == ws.WorkspaceId,
                };
                Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(item, $"WorkspacePick_{ws.WorkspaceId}");
                item.Click += (_, _) => PickWorkspace(ws);
                WorkspacePickerFlyout.Items.Add(item);
            }
            // 自定义新工作区：与侧栏加号同一套对话框（手输路径 + 内核目录选择器），
            // 建完即把新工作区设为待用值——下一个新会话就在它里面开局
            WorkspacePickerFlyout.Items.Add(new MenuFlyoutSeparator());
            var createItem = Aut(new MenuFlyoutItem
            {
                Text = L("新建工作区…"),
                Icon = new FontIcon { Glyph = "\uE710", FontSize = GlyphBody },
            }, "WorkspacePickCreate", L("新建工作区"));
            createItem.Click += async (_, _) => await CreateWorkspaceFromPickerAsync();
            WorkspacePickerFlyout.Items.Add(createItem);
            // 默认预设 = 内核内置 standard（标准模式）；未显式挑选时不显示「默认预设」占位文案
            AgentModeLabel.Text = L(_pendingPreset?.Name is { Length: > 0 } pn ? pn : "标准模式");
        }
        catch (Exception) { } // 选择器刷新异常不上抛（0xc000027b 教训）
    }

    private void PickWorkspace(WorkspaceVm? ws)
    {
        // 选中即生效于下一个新会话（列表式选择器需要可见的选中态，故在闭包里重建菜单）
        _pendingWorkspace = ws is null ? null : (ws.WorkspaceId, ws.Title);
        UpdateComposerSelectors();
    }

    /// <summary>输入区工作区选择器的「新建工作区…」：复用侧栏加号那套对话框，
    /// 创建成功后把新工作区直接设为待用值（下一个新会话用它）。</summary>
    private async Task CreateWorkspaceFromPickerAsync()
    {
        try
        {
            if (await CreateWorkspaceAsync() is { WorkspaceId.Length: > 0 } created)
            {
                PickWorkspace(created);
            }
        }
        catch (Exception) { } // 选择器入口兜底（0xc000027b 教训）
    }

    private void PickAgentPreset(string id, string name)
    {
        _pendingPreset = (id, name);
        UpdateComposerSelectors();
    }

    /// <summary>
    /// 空态判定（尚无消息的相位）：没有活动会话，或活动会话里还没有**对话**消息。
    /// 壳本地的 system 行（命令回显与回执）不算对话内容——空会话上改权限模式走
    /// /permission 命令，它的两条回显曾把空态打掉，使工作区/模式选择器连带消失。
    /// </summary>
    private bool IsHeroState() => _activeSessionId is null || !_messages.Any(m => m.Role != "system" && !m.Withdrawn);

    private void UpdateEmptyState()
    {
        // 内核还在后台引导时中间让位给加载卡：此刻摆"开始一段新的会话"会给人「可以开聊」的
        // 错觉，而发送要等内核就绪（SendAsync 会给一次性提示，见 MainWindow.KernelBoot.cs）。
        ChatHero.Visibility = IsHeroState() && KernelBootPanel.Visibility != Visibility.Visible
            ? Visibility.Visible
            : Visibility.Collapsed;
        // 聊天流始终顶对齐：避免空列表/短历史时内容被垂直居中
        ChatList.VerticalAlignment = VerticalAlignment.Top;
        UpdateComposerSelectors();
        RefreshSessionStateBar(); // 权限选择器：空态显示设置默认值，会话态显示会话投影
        RefreshSessionHeader(); // 会话头：无会话整条收起，有会话按 origin 刷只读态与回跳钮
    }

    /// <summary>Agent 模式菜单：内置四档（dsh agentPresets 内置预设）+ 内核已注册的自定义预设，
    /// 选中即作为下一个新会话的 agentPreset。</summary>
    private async Task LoadAgentModeMenuAsync()
    {
        if (_rpc is null)
        {
            return;
        }
        AgentModeFlyout.Items.Clear();
        var builtIn = new (string Id, string Name)[]
        {
            ("standard", "标准模式"),
            ("ptc", "PTC 模式"),
            ("minimal", "极简模式"),
            ("cordis", "创造模式"),
        };
        var seen = new HashSet<string>(StringComparer.Ordinal);
        foreach (var (id, name) in builtIn)
        {
            var item = new RadioMenuFlyoutItem
            {
                Text = L(name),
                Tag = id,
                GroupName = "agent-mode",
                IsChecked = _pendingPreset?.Id == id,
            };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(item, $"AgentMode_{id}");
            item.Click += (_, _) => PickAgentPreset(id, name);
            AgentModeFlyout.Items.Add(item);
            seen.Add(id);
        }
        try
        {
            var roster = await _rpc.CallOkAsync("agentPresets/list", new { });
            var items = roster.ValueKind == JsonValueKind.Array ? roster
                : roster.TryGetProperty("items", out var arr) && arr.ValueKind == JsonValueKind.Array ? arr
                : default;
            if (items.ValueKind == JsonValueKind.Array)
            {
                foreach (var preset in items.EnumerateArray())
                {
                    var id = Str(preset, "id") is { Length: > 0 } pid ? pid : Str(preset, "presetId");
                    if (id.Length == 0 || !seen.Add(id))
                    {
                        continue;
                    }
                    var name = Str(preset, "name") is { Length: > 0 } n ? n : id;
                    var item = new RadioMenuFlyoutItem
                    {
                        Text = name,
                        Tag = id,
                        GroupName = "agent-mode",
                        IsChecked = _pendingPreset?.Id == id,
                    };
                    Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(item, $"AgentMode_{id}");
                    item.Click += (_, _) => PickAgentPreset(id, name);
                    AgentModeFlyout.Items.Add(item);
                }
            }
        }
        catch (Exception)
        {
            // 预设目录取不到：保留内置四档，不阻塞选择
        }
    }

    /// <summary>旧事件处理器占位（会话选择已改由 Nav 菜单项 Tapped 驱动）。</summary>

    /// <summary>分页拉历史 journal，重建聊天流。
    /// 旧版会话（v0 迁移）游标可达数千 seq：先探测真实游标，再按 hasMore
    /// 从最新页向更早翻页，直到拉全。</summary>
    private async Task LoadSessionHistoryAsync(string sessionId)
    {
        if (_rpc is null)
        {
            return;
        }
        try
        {
            // 探测真实游标：内核 past-cursor 错误消息自带真实游标
            // （"… is past cursor &lt;sourceCursor&gt;"，dsh-api-session-controller 的 page），
            // 一次足够大的 throughSeq 即可解析出精确值——旧对半探测要 ~16 次往返。
            var lo = await ProbeJournalCursorAsync(sessionId);

            // 从最新页向更早翻页，拉全 journal；同时提取 session/title 回填标题
            var pages = new List<JsonElement>();
            string? fallbackTitle = null;
            var through = lo;
            while (through >= 0)
            {
                JsonElement page;
                try
                {
                    page = await _rpc.CallOkAsync("session/page", new
                    {
                        request = new { address = new { kind = "session", sessionId }, throughSeq = through, maxMessages = HistoryPageMessages },
                    });
                }
                catch (DshRpcException ex) when (ex.Message.Contains("past cursor"))
                {
                    break;
                }
                var records = page.TryGetProperty("records", out var recs) ? recs : default;
                if (records.ValueKind != JsonValueKind.Array || records.GetArrayLength() == 0)
                {
                    break;
                }
                // page 是 CallOkAsync 已克隆的独立元素，直接持有即可（旧版多 Clone 一次整页 JSON）
                pages.Add(page);
                foreach (var rec in records.EnumerateArray())
                {
                    if (rec.TryGetProperty("event", out var ev) &&
                        ev.TryGetProperty("type", out var et) && et.GetString() == "session/title" &&
                        ev.TryGetProperty("data", out var td) && td.TryGetProperty("title", out var tt) &&
                        tt.ValueKind == JsonValueKind.String && !string.IsNullOrEmpty(tt.GetString()))
                    {
                        fallbackTitle = tt.GetString();
                    }
                }
                var firstSeq = records[0].GetProperty("event").GetProperty("seq").GetInt32();
                if (page.TryGetProperty("hasMore", out var hm) && hm.ValueKind == JsonValueKind.True && firstSeq > 0)
                {
                    through = firstSeq - 1;
                }
                else
                {
                    break;
                }
            }

            // 时间序渲染（pages 是新→旧收的，倒回来）
            for (var i = pages.Count - 1; i >= 0; i--)
            {
                RenderJournal(pages[i]);
            }

            // projections 缺失的旧会话：列表显示的是 cwd 目录名兜底，
            // journal 里存在 durable title（session/title）时回填替换。
            // journal 按 seq 时间序扫描时逐条更新 fallbackTitle——取到最后一条
            // session/title 即最新 durable title（foldSessionTitle 同式）。
            if (fallbackTitle is not null)
            {
                var vm = _sessions.FirstOrDefault(s => s.SessionId == sessionId);
                if (vm is not null && vm.Title != fallbackTitle &&
                    (vm.Title == "新会话" ||
                     (vm.Cwd.Length > 0 && vm.Title == WorkspaceBasename(vm.Cwd)) ||
                     vm.Title == sessionId))
                {
                    vm.Title = fallbackTitle;
                    PostUi(RebuildNavMenu);
                }
            }
        }
        catch (Exception)
        {
            // 历史拉取失败不影响发消息。
        }
    }

    /// <summary>session/page 单次请求的每页消息数：内核默认 50（paginate 只对
    /// user/assistant 消息计数，实际随页返回的记录远多于此）。内核 maxMessages
    /// 只校验正整数、无上限，一次拉全可省掉大会话几十次翻页往返。</summary>
    private const int HistoryPageMessages = 5000;

    /// <summary>游标探测用的 throughSeq：内核 past-cursor 错误消息自带真实游标，
    /// 取一个远超任何真实 journal 长度的值，一次请求即可解析出精确游标。</summary>
    private const int JournalProbeSeq = 1 << 30;

    /// <summary>
    /// 解析内核 past-cursor 错误里的真实游标：消息形如
    /// "session page through seq &lt;n&gt; is past cursor &lt;sourceCursor&gt;"
    /// （dsh-api-session-controller 的 page 实现）。取不到返回 null（内核版本差异）。
    /// </summary>
    private static int? ParsePastCursorSeq(string message)
    {
        const string marker = "past cursor ";
        var at = message.LastIndexOf(marker, StringComparison.Ordinal);
        if (at < 0)
        {
            return null;
        }
        return int.TryParse(message.AsSpan(at + marker.Length).Trim(), out var seq) ? seq : null;
    }

    /// <summary>
    /// 探测会话 journal 的真实游标（= 最后一条事件的 seq；空日志为 -1）。
    /// 首选解析 past-cursor 错误消息里的游标（1 次往返）；消息格式不符时
    /// 回落对半探测（旧行为，最坏 ~16 次往返）。
    /// </summary>
    private async Task<int> ProbeJournalCursorAsync(string sessionId, CancellationToken ct = default)
    {
        try
        {
            await _rpc!.CallOkAsync("session/page", new
            {
                request = new { address = new { kind = "session", sessionId }, throughSeq = JournalProbeSeq },
            }, ct);
            // 探测值未越界：真实游标 ≥ 探测值（千万级事件的会话才会走到），
            // 以探测值为起点，hasMore 链会继续带到 journal 头部。
            return JournalProbeSeq;
        }
        catch (DshRpcException ex) when (ex.Message.Contains("past cursor"))
        {
            if (ParsePastCursorSeq(ex.Message) is { } cursor && cursor >= -1)
            {
                return cursor;
            }
        }
        // 兜底：对半探测（throughSeq 过大会 "past cursor"）
        var lo = 0;
        var hi = 1 << 16;
        while (lo < hi)
        {
            ct.ThrowIfCancellationRequested();
            var mid = lo + ((hi - lo + 1) >> 1);
            try
            {
                await _rpc.CallOkAsync("session/page", new
                {
                    request = new { address = new { kind = "session", sessionId }, throughSeq = mid },
                }, ct);
                lo = mid;
            }
            catch (DshRpcException ex) when (ex.Message.Contains("past cursor"))
            {
                hi = mid - 1;
            }
        }
        return lo;
    }

    /// <summary>
    /// 拉取某会话的**全部** journal 事件（按 seq 升序）。与 LoadSessionHistoryAsync 同为
    /// session/page 翻页，但只返回事件、不碰聊天流——供轨迹页等只读视图复用。
    /// </summary>
    private async Task<List<JsonElement>> FetchSessionEventsAsync(string sessionId, CancellationToken ct)
    {
        var events = new List<JsonElement>();
        if (_rpc is null)
        {
            return events;
        }
        // 探测真实游标（错误消息自带游标，1 次往返；详见 ProbeJournalCursorAsync）
        var lo = await ProbeJournalCursorAsync(sessionId, ct);

        var pages = new List<List<JsonElement>>();
        var through = lo;
        while (through >= 0)
        {
            ct.ThrowIfCancellationRequested();
            JsonElement page;
            try
            {
                page = await _rpc.CallOkAsync("session/page", new
                {
                    request = new { address = new { kind = "session", sessionId }, throughSeq = through, maxMessages = HistoryPageMessages },
                }, ct);
            }
            catch (DshRpcException ex) when (ex.Message.Contains("past cursor"))
            {
                break;
            }
            if (!page.TryGetProperty("records", out var records) ||
                records.ValueKind != JsonValueKind.Array || records.GetArrayLength() == 0)
            {
                break;
            }
            var batch = new List<JsonElement>();
            foreach (var rec in records.EnumerateArray())
            {
                if (rec.TryGetProperty("event", out var ev))
                {
                    batch.Add(ev.Clone());
                }
            }
            pages.Add(batch);
            var firstSeq = records[0].GetProperty("event").GetProperty("seq").GetInt32();
            if (page.TryGetProperty("hasMore", out var hm) && hm.ValueKind == JsonValueKind.True && firstSeq > 0)
            {
                through = firstSeq - 1;
            }
            else
            {
                break;
            }
        }
        // pages 是新→旧收的，倒回来变成 seq 升序
        for (var i = pages.Count - 1; i >= 0; i--)
        {
            events.AddRange(pages[i]);
        }
        return events;
    }

    private void RegisterStreamRecovery()
    {
        _rpc!.StreamsReset += () => PostUi(() =>
        {
            _businessStreamsResetPending = true;
            _workspaceStreamOpen = false;
            _sessionStreamId = null;
            _controlStreamId = null;
            Interlocked.Increment(ref _sessionFollowGeneration);
            _ = RestoreBusinessStreamsAsync();
        });
    }

    private bool _businessStreamsResetPending;
    private bool _restoringBusinessStreams;
    private int _sessionFollowGeneration;
    private readonly SemaphoreSlim _followFrameGate = new(1, 1);

    private async Task ApplyFollowFrameAsync(string sessionId, int generation, JsonElement frame)
    {
        await _followFrameGate.WaitAsync();
        try
        {
            bool Current() => generation == _sessionFollowGeneration && sessionId == _activeSessionId;
            if (!Current() || _rpc is null) return;
            var frameType = Str(frame, "type");
            if (frameType == "assistant-stream")
            {
                if (frame.TryGetProperty("frame", out var streamFrame))
                {
                    ApplyAssistantStreamFrame(streamFrame.Clone());
                }
                return;
            }
            if (frameType == "snapshot" &&
                frame.TryGetProperty("assistantStream", out var baseline) &&
                baseline.ValueKind == JsonValueKind.Object &&
                baseline.TryGetProperty("activeAttempt", out var active) &&
                active.ValueKind == JsonValueKind.Object)
            {
                // 重连基线：内核把仍在生成的 attempt 的紧凑增量流随快照带回，重建逐字气泡
                RebuildLiveAttempt(active.Clone());
            }
            if (frame.TryGetProperty("records", out var records))
            {
                var pending = records.EnumerateArray().Select(r => r.GetProperty("event").Clone()).ToList();
                var page = frame;
                while (page.TryGetProperty("hasMore", out var more) && more.ValueKind == JsonValueKind.True && pending.Count > 0)
                {
                    var first = pending.Min(e => e.GetProperty("seq").GetInt32());
                    if (first <= _journalCursor + 1) break;
                    page = await _rpc.CallOkAsync("session/page", new
                    {
                        request = new { address = new { kind = "session", sessionId }, throughSeq = first - 1, maxMessages = HistoryPageMessages },
                    });
                    if (!Current()) return;
                    var earlier = page.GetProperty("records").EnumerateArray().Select(r => r.GetProperty("event").Clone()).ToList();
                    if (earlier.Count == 0 || earlier.Min(e => e.GetProperty("seq").GetInt32()) >= first)
                        throw new InvalidOperationException(L("会话历史分页未前进。"));
                    pending.AddRange(earlier);
                }
                foreach (var ev in pending.OrderBy(e => e.GetProperty("seq").GetInt32()))
                {
                    var seq = ev.GetProperty("seq").GetInt32();
                    if (seq <= _journalCursor) continue;
                    RenderEventCore(ev);
                    if (Str(ev, "type") == "turn/end") MaybeNotifyTurnEnded(ev);
                    _journalCursor = seq;
                }
            }
            else if (frame.TryGetProperty("event", out var ev) &&
                     ev.TryGetProperty("seq", out var seqEl) && seqEl.TryGetInt32(out var seq) && seq > _journalCursor)
            {
                RenderEventCore(ev);
                if (Str(ev, "type") == "turn/end") MaybeNotifyTurnEnded(ev);
                _journalCursor = seq;
                RequestSessionsRefresh();
            }
        }
        catch (Exception ex)
        {
            if (generation == _sessionFollowGeneration && sessionId == _activeSessionId)
                AppendSystemMessage(LF("会话同步失败：{0}", ex.Message));
        }
        finally { _followFrameGate.Release(); }
    }

    // ---------------- 逐字增量流（session/follow 的 assistant-stream 帧） ----------------

    /// <summary>内核增量流的降级开关：带 assistantStream 参数开流失败过一次（老内核不认该字段）
    /// 即退回持久事件模式，由 PollReplyAsync 兜底出字。</summary>
    private bool _liveStreamBroken;
    private bool LiveStreamRequested => !_liveStreamBroken;
    /// <summary>revision 基线：首帧/重开流记录，内核档位重置时说明流序号已失配。</summary>
    private int _liveStreamRevision = -1;
    /// <summary>attemptId → 该次模型尝试的逐字气泡（按内容块 index 分桶，reasoning 共用一桶）。</summary>
    private readonly Dictionary<string, LiveAttemptState> _liveAttempts = new(StringComparer.Ordinal);
    /// <summary>已收到 end 帧、等持久 assistant/message 定稿的 attempt（end 通常先于落定事件）。</summary>
    private readonly List<LiveAttemptState> _settlingAttempts = new();
    private Microsoft.UI.Dispatching.DispatcherQueueTimer? _liveRepaintTimer;
    /// <summary>「等待首个增量」的占位气泡：不随 journal 裁剪，单独挂在可见列表尾部。</summary>
    private ChatBubble? _pendingBubble;
    /// <summary>发送键当前呈现的是「发送」还是「停止」（见 UpdateComposerRunningState）。</summary>
    private bool _composerRunning;

    private sealed class LiveBucket
    {
        public int Order;
        /// <summary>增量累加器。字符串拼接在长回复下是 O(n²)，且气泡的 Text 必须跟着涨——
        /// 早先只更新这里、不回写气泡，重绘读到的永远是首帧旧文本（表现为「半天才蹦字」）。</summary>
        public readonly System.Text.StringBuilder Text = new();
        public ChatBubble? Bubble;
        public bool Pending;
    }

    private sealed class LiveAttemptState
    {
        public int Turn;
        public long Time;
        /// <summary>key = 文本块 index；内核 reasoning-delta 的 index 是 reasoning 块序号，
        /// 为避免与文本块撞名，reasoning 统一记在 -1 桶。</summary>
        public readonly Dictionary<int, LiveBucket> Buckets = new();
    }

    private void ApplyAssistantStreamFrame(JsonElement frame)
    {
        var kind = Str(frame, "type");
        var attemptId = Str(frame, "attemptId");
        if (attemptId.Length == 0)
        {
            return;
        }
        if (frame.TryGetProperty("revision", out var rev) && rev.ValueKind == JsonValueKind.Number)
        {
            if (_liveStreamRevision < 0)
            {
                _liveStreamRevision = rev.GetInt32();
            }
            else if (rev.GetInt32() < _liveStreamRevision)
            {
                // 内核换了新 attempt 序号基准：本侧增量可能缺帧，重开流让快照基线重建
                _liveStreamRevision = rev.GetInt32();
                RebaselineLiveStream();
                return;
            }
        }
        switch (kind)
        {
            case "start":
                _liveAttempts[attemptId] = new LiveAttemptState
                {
                    Turn = Num(frame, "turn"),
                    Time = NowMs(),
                };
                break;
            case "chunk":
                if (frame.TryGetProperty("chunk", out var chunk) && chunk.ValueKind == JsonValueKind.Object &&
                    _liveAttempts.TryGetValue(attemptId, out var attempt))
                {
                    ApplyLiveChunk(attemptId, attempt, chunk);
                    // 增量帧必须自己约重绘：早先这里直接 return，气泡要等 end 帧才刷新，
                    // 表现就是"整块蹦字、没有逐字输出"。
                    ScheduleLiveRepaint();
                }
                return;                 // 增量不推进持久游标
            case "end":
                EndLiveAttempt(attemptId);
                break;
        }
        ScheduleLiveRepaint();
    }

    private void ApplyLiveChunk(string attemptId, LiveAttemptState attempt, JsonElement chunk)
    {
        var chunkType = Str(chunk, "type");
        var text = Str(chunk, "text");
        int bucketKey;
        bool reasoning;
        switch (chunkType)
        {
            case "text-delta":
                bucketKey = Num(chunk, "index");
                reasoning = false;
                break;
            case "reasoning-delta":
                bucketKey = -1;
                reasoning = true;
                break;
            default:
                return;                 // tool-call-delta / block-* / usage / finish 不进逐字流
        }
        if (text.Length == 0)
        {
            return;
        }
        if (!attempt.Buckets.TryGetValue(bucketKey, out var bucket))
        {
            bucket = new LiveBucket { Order = attempt.Buckets.Count };
            attempt.Buckets[bucketKey] = bucket;
        }
        var wasEmpty = bucket.Text.Length == 0;
        bucket.Text.Append(text);
        bucket.Pending = true;
        if (wasEmpty)
        {
            bucket.Bubble = InsertLiveBubble(attempt, bucket, reasoning, bucketKey);
        }
        SetPendingThinking(false);
    }

    /// <summary>首个增量到达时挂出气泡；后续帧只改 Text，重绘由 ScheduleLiveRepaint 合帧。
    /// 返回挂出的气泡；会话已切走时返回 null。</summary>
    private ChatBubble? InsertLiveBubble(LiveAttemptState attempt, LiveBucket bucket, bool reasoning, int bucketKey)
    {
        var sid = Volatile.Read(ref _activeSessionId);
        if (sid is null)
        {
            return null;
        }
        // 本轮在流气泡是尾部连续的一段（用户回显按 UserInsertIndex 排在它们之前），新气泡按
        // 「思考在前、文本块按 index 升序」插进这一段：既不受逐字到达先后影响（首块文本先到时
        // 思考仍要排它上面），也不会像按轮号回找插入点那样把回答插到思考前面、把用户回显甩到回答之后。
        var key = reasoning ? -1 : bucketKey;
        var before = 0;
        foreach (var existing in attempt.Buckets)
        {
            if (existing.Key < key)
            {
                before++;
            }
        }
        var insertAt = _messages.Count;
        var i = _messages.Count - 1;
        while (i >= 0 && _messages[i] is { IsLiveStreaming: true, Turn: var t } && t == attempt.Turn)
        {
            i--;
        }
        insertAt = Math.Min(i + 1 + before, _messages.Count);
        var bubble = new ChatBubble
        {
            Role = reasoning ? "reasoning" : "assistant",
            Text = bucket.Text.ToString(),
            Turn = attempt.Turn,
            IsReasoning = reasoning,
            IsAssistantStep = !reasoning,
            Time = attempt.Time,
            IsLiveStreaming = true,
            Model = _lastHeaderModel,
        };
        _messages.Insert(insertAt, bubble);
        if (_messages.Count > 400)
        {
            _messages.RemoveAt(0);
        }
        bucket.Bubble = bubble;
        ScrollTranscriptToBottom();
        return bubble;
    }

    /// <summary>把重排/重绘合并到下一帧：内核每 token 一帧，逐帧重解析 markdown 会让列表反复重排。
    /// 已有窗口在计时时必须直接返回：Start() 会把倒计时重新拉满，而内核 token 间隔常小于 80ms，
    /// 每帧 Start() 等于永不触发（0.7.8.7 就这么把逐字重绘饿死了，只剩首帧文本）。</summary>
    private void ScheduleLiveRepaint()
    {
        if (_liveRepaintTimer is not null)
        {
            return;
        }
        var timer = DispatcherQueue.CreateTimer();
        timer.Interval = TimeSpan.FromMilliseconds(80);
        timer.IsRepeating = false;
        timer.Tick += (_, _) =>
        {
            _liveRepaintTimer = null;
            RepaintLiveBubbles();
        };
        _liveRepaintTimer = timer;
        timer.Start();
    }

    private void RepaintLiveBubbles()
    {
        foreach (var attempt in _liveAttempts.Values)
        {
            foreach (var bucket in attempt.Buckets.Values)
            {
                if (bucket.Bubble is not { } bubble)
                {
                    continue;
                }
                if (bucket.Pending)
                {
                    bucket.Pending = false;
                    bubble.Text = bucket.Text.ToString();
                }
                RepaintBubble(bubble);
            }
        }
        if (_liveAttempts.Count > 0)
        {
            ScrollTranscriptToBottom();
        }
    }

    /// <summary>就地重绘一条已上屏气泡。容器未 realize 时直接返回——那说明它在屏外，
    /// 将来 realize 时 Loaded 会按最新 Text 渲染，无需（也不能）整表 Refresh。</summary>
    private void RepaintBubble(ChatBubble bubble)
    {
        if (FindBubbleHost(bubble) is not { } host)
        {
            return;
        }
        if (bubble.IsReasoning)
        {
            BuildReasoningRow(host);
        }
        else if (bubble.Role == "tool-call")
        {
            BuildToolCallRow(host);
        }
        else
        {
            RenderAssistantMarkdown(host);
        }
    }

    /// <summary>轮次收尾：仍在执行窗口的工具行全部停扫光（中断/越权拒绝等 result 不来的路径）。
    /// 行在屏外未 realize 也无妨——下次 realize 时按 IsToolRunning=false 装配。</summary>
    private void StopRunningToolSweeps()
    {
        if (_runningToolCalls.Count == 0)
        {
            return;
        }
        foreach (var running in _runningToolCalls.Values)
        {
            running.IsToolRunning = false;
            RepaintBubble(running);
        }
        _runningToolCalls.Clear();
    }

    /// <summary>按气泡找当前存活的模板宿主（AssistantTpl 的 MdHost / ReasoningTpl 的 ReasoningHost /
    /// ToolCallTpl 的 ToolCallHost）。容器会被虚拟化回收复用：Name 留着但 DataContext 可能已换成
    /// 别的气泡，只认 DataContext 是自己的。</summary>
    private ContentControl? FindBubbleHost(ChatBubble bubble)
    {
        if (ChatList.ContainerFromItem(bubble) is not { } container)
        {
            return null;
        }
        var wanted = bubble.Role switch
        {
            "reasoning" => "ReasoningHost",
            "tool-call" => "ToolCallHost",
            _ => "MdHost",
        };
        return FindByName(container, bubble, wanted, 0);

        static ContentControl? FindByName(DependencyObject node, ChatBubble target, string name, int depth)
        {
            if (depth > 24)
            {
                return null;
            }
            var count = Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChildrenCount(node);
            for (var i = 0; i < count; i++)
            {
                var child = Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChild(node, i);
                if (child is ContentControl { Name: var n } cc && n == name && ReferenceEquals(cc.DataContext, target))
                {
                    return cc;
                }
                if (FindByName(child, target, name, depth + 1) is { } found)
                {
                    return found;
                }
            }
            return null;
        }
    }

    /// <summary>持久 assistant/message 落位时回填逐字气泡：命中则就地定稿，未命中才新建。
    /// 定稿只有一条正文，多块增量合并后多余的撤下。</summary>
    private bool SettleLiveTextBubble(LiveAttemptState attempt, string text, string? messageId, int seq, out ChatBubble? settled)
    {
        var ordered = attempt.Buckets.Where(kv => kv.Key >= 0).OrderBy(kv => kv.Value.Order).ToList();
        ChatBubble? keep = null;
        settled = null;
        foreach (var (_, bucket) in ordered)
        {
            if (bucket.Bubble is not { } bubble)
            {
                continue;
            }
            if (keep is null && text.Length > 0)
            {
                keep = bubble;
            }
            else
            {
                _messages.Remove(bubble);
            }
            bucket.Bubble = null;
        }
        if (keep is null)
        {
            return false;
        }
        keep.Text = text;
        keep.IsLiveStreaming = false;
        keep.MessageId = messageId;
        if (seq > 0) keep.Seq = seq;
        RepaintBubble(keep);
        settled = keep;
        return true;
    }

    private bool SettleLiveReasoningBubble(LiveAttemptState attempt, string text)
    {
        if (attempt.Buckets.TryGetValue(-1, out var bucket) && bucket.Bubble is { } bubble)
        {
            if (text.Length > 0)
            {
                bubble.Text = text;
                bubble.IsLiveStreaming = false;
                RepaintBubble(bubble);
            }
            else
            {
                _messages.Remove(bubble);   // 只有空白增量的推理块：定稿不会落它，撤下
            }
            bucket.Bubble = null;
            return true;
        }
        return false;
    }

    /// <summary>attempt 结束（end 帧）：转入待定稿队列——持久 assistant/message 随后到达时
    /// 按 turn 认领同一条气泡就地定稿。会话被中断时气泡就地停在已收到的文本上。</summary>
    private void EndLiveAttempt(string attemptId)
    {
        if (_liveAttempts.TryGetValue(attemptId, out var attempt))
        {
            _liveAttempts.Remove(attemptId);
            // end 帧后不再被逐字重绘遍历：把最后一批增量落到气泡上，否则尾部文字丢失
            foreach (var bucket in attempt.Buckets.Values)
            {
                if (bucket.Pending && bucket.Bubble is { } tailBubble)
                {
                    bucket.Pending = false;
                    tailBubble.Text = bucket.Text.ToString();
                    RepaintBubble(tailBubble);
                }
            }
            _settlingAttempts.Add(attempt);
            // 兜底：被中断且永不落持久消息的 attempt 没人认领，不淘汰会随轮次累积。
            while (_settlingAttempts.Count > 32)
            {
                ForgetLiveAttempt(_settlingAttempts[0]);
                _settlingAttempts.RemoveAt(0);
            }
        }
        SetPendingThinking(IsAnySessionRunning());
    }

    /// <summary>放弃一个待定稿 attempt：气泡停在已收到的文本上，摘掉 live 标记不再重绘。</summary>
    private void ForgetLiveAttempt(LiveAttemptState attempt)
    {
        foreach (var bucket in attempt.Buckets.Values)
        {
            if (bucket.Bubble is { } bubble)
            {
                if (bucket.Pending)
                {
                    bucket.Pending = false;
                    bubble.Text = bucket.Text.ToString();
                }
                bubble.IsLiveStreaming = false;
            }
            bucket.Bubble = null;
        }
    }

    /// <summary>定稿认领：优先该轮仍在流式的 attempt，其次最早待定稿的同轮 attempt。</summary>
    private LiveAttemptState? TakeLiveAttemptForSettlement(int turn)
    {
        var key = _liveAttempts.FirstOrDefault(kv => kv.Value.Turn == turn).Key;
        if (key is not null)
        {
            var live = _liveAttempts[key];
            _liveAttempts.Remove(key);
            return live;
        }
        for (var i = 0; i < _settlingAttempts.Count; i++)
        {
            if (_settlingAttempts[i].Turn != turn) continue;
            var pending = _settlingAttempts[i];
            _settlingAttempts.RemoveAt(i);
            return pending;
        }
        return null;
    }

    /// <summary>作废全部逐字状态（换会话 / revision 回退重基线）：气泡摘掉 live 标记，
    /// 停在已收到的文本上——留着标记会让后续增量按错误的插入点定位。</summary>
    private void ForgetLiveAttempts()
    {
        foreach (var attempt in _liveAttempts.Values)
        {
            ForgetLiveAttempt(attempt);
        }
        foreach (var attempt in _settlingAttempts)
        {
            ForgetLiveAttempt(attempt);
        }
        _liveAttempts.Clear();
        _settlingAttempts.Clear();
    }

    /// <summary>快照重连基线：用内核带回的紧凑增量流重建仍在生成的 attempt。</summary>
    private void RebuildLiveAttempt(JsonElement active)
    {
        var attemptId = Str(active, "attemptId");
        if (attemptId.Length == 0)
        {
            return;
        }
        var attempt = new LiveAttemptState { Turn = Num(active, "turn"), Time = NowMs() };
        _liveAttempts[attemptId] = attempt;
        if (!active.TryGetProperty("stream", out var stream) || stream.ValueKind != JsonValueKind.Array)
        {
            return;
        }
        // 紧凑记录：text-chunks / reasoning-chunks 的 texts 直接拼接（顺序无关，只做整桶合并）
        foreach (var record in stream.EnumerateArray())
        {
            var type = Str(record, "type");
            if (type == "chunk")
            {
                if (record.TryGetProperty("chunk", out var chunk) && chunk.ValueKind == JsonValueKind.Object)
                {
                    ApplyLiveChunk(attemptId, attempt, chunk);
                }
                continue;
            }
            if (type != "text-chunks" && type != "reasoning-chunks")
            {
                continue;
            }
            var synthetic = JsonDocument.Parse(
                $"{{\"type\":\"{(type == "text-chunks" ? "text-delta" : "reasoning-delta")}\",\"index\":{Num(record, "index")}," +
                $"\"text\":{JsonSerializer.Serialize(string.Concat(record.GetProperty("texts").EnumerateArray().Select(t => t.GetString())))}}}").RootElement;
            ApplyLiveChunk(attemptId, attempt, synthetic);
        }
        ScheduleLiveRepaint();
    }

    private void RebaselineLiveStream()
    {
        ForgetLiveAttempts();
        _liveStreamRevision = -1;
        if (Volatile.Read(ref _activeSessionId) is { } sid)
        {
            _ = FollowSessionAsync(sid);
        }
    }

    private static long NowMs() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();

    /// <summary>JSON 对象的整数字段（缺失/非数字 → 0）。</summary>
    private static int Num(JsonElement obj, string name)
        => obj.ValueKind == JsonValueKind.Object && obj.TryGetProperty(name, out var el) && el.TryGetInt32(out var v)
            ? v
            : 0;

    private bool IsAnySessionRunning()
    {
        lock (_sessionRunningLock)
        {
            return _sessionRunning.Values.Any(v => v);
        }
    }

    /// <summary>「少女祈祷中」占位气泡：挂在可见列表尾部，不进 journal 源列表，故不受紧凑裁剪。</summary>
    private void SetPendingThinking(bool on)
    {
        if (!on)
        {
            if (_pendingBubble is null)
            {
                UpdateComposerRunningState();
                return;
            }
            _pendingBubble = null;
            RefreshTranscriptView();
            UpdateComposerRunningState();
            return;
        }
        // 只在"根本没有会话"时不挂占位（空会话首页也挂着 hero，看不见）；
        // 不能用 IsHeroState() 判：新会话发送后 _messages 还是空的，正是等得最久的一段。
        if (_pendingBubble is not null || Volatile.Read(ref _activeSessionId) is null)
        {
            return;
        }
        _pendingBubble = new ChatBubble { Role = "pending", Text = "" };
        RefreshTranscriptView();
        UpdateComposerRunningState();
        ScrollTranscriptToBottom(force: true);
    }

    /// <summary>贴底判定阈值：离底不到一个滚动条滑块的距离就算「在看最新内容」。</summary>
    private const double StickToBottomPx = 96;

    /// <summary>滚动到聊天流绝对底部。走内部 ScrollViewer 而不是 ScrollIntoView：后者只把末项
    /// **顶部**对齐视口，逐字增高的长气泡会造成「跳上去再弹回来」的抖动。
    /// force=false 时只在用户本就贴底才跟随——上翻读旧消息不该被逐字增量拽回底部。</summary>
    private void ScrollTranscriptToBottom(bool force = false)
    {
        if (!force && !IsTranscriptAtBottom())
        {
            return;
        }
        if (TranscriptScroller(ChatList) is not null)
        {
            // 下一帧定位：本轮内容刚改完，等布局算出新的 ScrollableHeight。
            PostUi(() =>
            {
                if (TranscriptScroller(ChatList) is { } sv && sv.ScrollableHeight > 0)
                {
                    sv.ChangeView(null, sv.ScrollableHeight, null, disableAnimation: true);
                }
            });
            return;
        }
        // 列表还没 realize 出滚动面（首屏/空会话）：退一步按项对齐。
        if (_visibleMessages.Count > 0)
        {
            ChatList.ScrollIntoView(_visibleMessages[^1]);
        }
    }

    /// <summary>是否已贴在底部。拿不到滚动面（列表尚未 realize）时按贴底处理，别错过首屏。</summary>
    private bool IsTranscriptAtBottom()
    {
        if (TranscriptScroller(ChatList) is not { } sv)
        {
            return true;
        }
        return sv.ScrollableHeight - sv.VerticalOffset <= StickToBottomPx;
    }

    private static ScrollViewer? TranscriptScroller(ListView chatList)
    {
        return FindScroller(chatList, 0);

        static ScrollViewer? FindScroller(DependencyObject node, int depth)
        {
            if (depth > 12)
            {
                return null;
            }
            var count = Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChildrenCount(node);
            for (var i = 0; i < count; i++)
            {
                var child = Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChild(node, i);
                if (child is ScrollViewer sv)
                {
                    return sv;
                }
                if (FindScroller(child, depth + 1) is { } found)
                {
                    return found;
                }
            }
            return null;
        }
    }

    private async Task RestoreBusinessStreamsAsync()
    {
        if (_restoringBusinessStreams) return;
        _restoringBusinessStreams = true;
        try
        {
            while (_businessStreamsResetPending)
            {
                _businessStreamsResetPending = false;
                await RefreshWorkspacesAsync();
                if (_activeSessionId is { } sessionId)
                    await FollowSessionAsync(sessionId);
                await OpenSessionControlStreamAsync();
            }
        }
        catch (Exception ex)
        {
            AppendSystemMessage(LF("连接恢复后订阅失败：{0}", ex.Message));
        }
        finally { _restoringBusinessStreams = false; }
    }

    /// <summary>session/follow 流跟随当前会话（journal 增量 → 聊天流）。</summary>
    private async Task FollowSessionAsync(string sessionId)
    {
        if (_rpc is null)
        {
            return;
        }
        try
        {
            // 单流跟随：取消旧流再开新流（避免流累积），活动会话才渲染
            if (_sessionStreamId is { } oldId)
            {
                _rpc.CancelStream(oldId);
            }
            var generation = Interlocked.Increment(ref _sessionFollowGeneration);
            var streamId = await _rpc.OpenFollowStreamAsync(sessionId, frame =>
            {
                var owned = frame.Clone();
                PostUi(() => _ = ApplyFollowFrameAsync(sessionId, generation, owned));
            }, assistantStream: LiveStreamRequested);
            if (generation != _sessionFollowGeneration || sessionId != _activeSessionId)
            {
                _rpc.CancelStream(streamId);
            }
            else
            {
                _sessionStreamId = streamId;
                // 带增量参数开流失败过一次就不会再到这里；恢复开关只为手动重连
                _liveStreamBroken = false;
            }
        }
        catch (Exception)
        {
            // mux 断开（内核 idle 关闭/网络闪断）时静默：聊天主流程靠 RefreshSessions 与
            // 下次操作重连。绝不让流异常沿 async void 上抛成未处理异常（0xc000027b 根因）。
            // 若失败发生在带 assistantStream 参数的开流上，判定内核不认该参数，退回持久事件模式。
            if (LiveStreamRequested)
            {
                _liveStreamBroken = true;
            }
        }
    }

    // ---------------- journal 事件 → 聊天流渲染 ----------------

    /// <summary>气泡面装载：把当前生效的气泡笔刷实例直接赋给 Border.Background。
    /// 不走字典查找：ThemeResource 的本地覆盖对 DataTemplate 内容不生效（App 级 ThemeDictionaries
    /// 才是模板内容实际解析到的那一层，RootGrid 本地覆盖够不着），代码直赋实例是确定生效的路径。
    /// 所有气泡共用同一实例：改不透明度只改实例属性，已装载气泡即时重绘；换材质档换实例，
    /// 由 RebindChatList 重建容器后在这里逐个拿到新实例。高对比度 _bubbleBrush 为 null，
    /// 跳过直赋、保留 XAML 基线的 SystemColorWindowColor 纯色（半透明会吃掉文字对比）。</summary>
    private void OnBubbleBorderLoaded(object sender, RoutedEventArgs e)
    {
        if (sender is Border border && _bubbleBrush is Brush brush)
        {
            border.Background = brush;
        }
    }

    /// <summary>助手气泡装载：把 markdown 文本替换成 MarkdownRenderer 渲染面板，并装配反馈行。
    /// ListView 虚拟化会回收容器：必须每次从 DataContext 取文本重渲染，不能只看 Content 是否已是 string
    /// （回收后 Content 可能残留上一帧的 StackPanel，或已被 Presenter 转成 NoWrap TextBlock）。</summary>
    private void OnAssistantBubbleLoaded(object sender, RoutedEventArgs e)
    {
        if (sender is not ContentControl host)
        {
            return;
        }
        // 回收时 Loaded 不一定重发：挂 DataContextChanged 兜底重渲染
        if (host.Tag is not "assistant-md-hooked")
        {
            host.Tag = "assistant-md-hooked";
            host.DataContextChanged += OnAssistantHostDataContextChanged;
        }
        RenderAssistantMarkdown(host);
        BuildFeedbackRow(host);
    }

    private void OnAssistantHostDataContextChanged(FrameworkElement sender, DataContextChangedEventArgs args)
    {
        if (sender is not ContentControl host)
        {
            return;
        }
        RenderAssistantMarkdown(host);
        BuildFeedbackRow(host);
    }

    /// <summary>按当前 DataContext 渲染助手 markdown；已渲染同一文本时跳过，避免悬停反馈行重建时反复重排。</summary>
    private void RenderAssistantMarkdown(ContentControl host)
    {
        var text = host.DataContext is ChatBubble { Text: { Length: > 0 } t } ? t : null;
        if (text is null)
        {
            return;
        }
        if (host.Content is StackPanel existing && existing.Tag is string rendered && rendered == text)
        {
            return;
        }
        // 逐字增量：在旧面板上续写尾块，避免每 80ms 重建整棵视觉树导致整段重排
        var prev = host.Content is StackPanel p && p.Tag is string ? p : null;
        var panel = MarkdownRenderer.Append(prev, text);
        panel.HorizontalAlignment = HorizontalAlignment.Stretch;
        host.Content = panel;
    }

    /// <summary>历史回放入口。任意线程可调：非 UI 线程自动编组。</summary>
    private void RenderJournal(JsonElement page)
    {
        if (DispatcherQueue.HasThreadAccess)
        {
            RenderJournalCore(page);
        }
        else
        {
            PostUi(() => RenderJournalCore(page));
        }
    }

    private void RenderJournalCore(JsonElement page)
    {
        if (!page.TryGetProperty("records", out var records))
        {
            return;
        }
        foreach (var rec in records.EnumerateArray())
        {
            if (rec.TryGetProperty("event", out var ev))
            {
                var seq = ev.GetProperty("seq").GetInt32();
                // 补拉/轮询会重放整页：按全局游标跳过已渲染事件
                if (seq <= Volatile.Read(ref _journalCursor) && seq != 0)
                {
                    continue;
                }
                _journalCursor = Math.Max(_journalCursor, seq);
                RenderEventCore(ev);
            }
        }
    }

    /// <summary>单条 journal 事件 → 气泡。任意线程可调：非 UI 线程自动编组。</summary>
    private void RenderEvent(JsonElement ev)
    {
        if (DispatcherQueue.HasThreadAccess)
        {
            RenderEventCore(ev);
        }
        else
        {
            PostUi(() => RenderEventCore(ev));
        }
    }

    /// <summary>纯 UI 线程渲染核心。绝不能从后台线程直调。</summary>
    private void RenderEventCore(JsonElement ev)
    {
        var type = ev.GetProperty("type").GetString();
        var data = ev.TryGetProperty("data", out var d) ? d : default;
        // 事件信封自带的坐标（wire 契约：seq: int + time: epoch ms）：seq 作分支锚点，
        // time 作消息时间戳与本轮用时的两端（见 session-wire-event 校验）。
        var envSeq = ev.TryGetProperty("seq", out var envSeqEl) && envSeqEl.ValueKind == JsonValueKind.Number && envSeqEl.TryGetInt32(out var es) ? es : 0;
        var envTime = ev.TryGetProperty("time", out var envTimeEl) && envTimeEl.ValueKind == JsonValueKind.Number ? (long)envTimeEl.GetDouble() : 0L;
        ChatBubble? bubble = null;
        var turn = data.ValueKind == JsonValueKind.Object && data.TryGetProperty("turn", out var turnValue) &&
            turnValue.TryGetInt32(out var number) ? number : _transcriptTurn;

        TrackTranscriptEvent(type, data, turn);
        TrackRunStats(ev, type, data); // 运行状态条折叠（page 回放 + follow 流共用入口）
        TrackContextMeter(type, data); // 上下文容量胶囊折叠（同上共用入口，last-wins 无累计）
        // 使用统计实时刷新：带用量/时长口径的事件到达即防抖重算（手动「刷新」已移除）。
        if (type is "assistant/message" or "turn/end") RequestStatsLiveRefresh();
        // 已撤回编辑的轮次/提问：journal 只增不改，补拉、轮询兜底、切会话回放都会把它们带回，
        // 在这里统一抑制（用量/上下文投影仍照记——那是内核侧事实，与界面撤不撤无关）。
        // turn/end 必须放行：占位气泡、扫光收尾、用时账全靠它善后。
        if (type != "turn/end" && turn > 0 && Volatile.Read(ref _activeSessionId) is { } wsid &&
            _withdrawnTurns.TryGetValue(wsid, out var withdrawn) && withdrawn.Contains(turn))
        {
            return;
        }
        if (type == "user/message" && envSeq > 0 && Volatile.Read(ref _activeSessionId) is { } wseqSid &&
            _withdrawnSeqs.TryGetValue(wseqSid, out var withdrawnSeqs) && withdrawnSeqs.Contains(envSeq))
        {
            return;
        }
        switch (type)
        {
            case "turn/start":
                // 本轮起点（turn/start 信封时间）：turn/end 作差得「用时」。
                // 文件改动累加器同步重置（官方 deliverablesDefinition 的 start()）
                if (turn > 0)
                {
                    _turnStartMs[turn] = envTime;
                    _producedByTurn.Remove(turn);
                    _turnMutations.Remove(turn);
                }
                _openMutationPaths.Clear();
                // 上一轮若中断（无 turn/end 直接开新轮）：未收 result 的工具行扫光在此统一收尾
                StopRunningToolSweeps();
                // 撤回编辑候选随轮次移交：上一条提问的按钮到此作废（它那轮已经在跑/已结束），
                // 新候选是刚开跑的这条提问（队列消息按序起轮时同样正确）。
                RefreshWithdrawEditButton();
                _withdrawCandidate = LastUserBubble();
                _turnMutated = false;
                _runTurn = turn;
                // 撤回点在 turn/start 之前的窗口：这一轮就是被撤回的那次 prompt 起的轮
                var withdrawnNow = false;
                if (_withdrawPendingTurn && turn > 0 && Volatile.Read(ref _activeSessionId) is { } psid)
                {
                    _withdrawPendingTurn = false;
                    WithdrawnTurns(psid).Add(turn);
                    withdrawnNow = true;
                }
                // 撤回的这轮不出「思考中」：进程正在取消，万一内核不再补 turn/end，
                // 卡着一个不会收尾的指示器比压根不显示更糟
                if (!withdrawnNow)
                {
                    SetPendingThinking(true);
                }
                RefreshTranscriptView();
                return;
            case "step/start":
                RefreshTranscriptView();
                return;
            case "request/header":
                // 内核每发一次 LLM 请求记一条（dsh-agent-loop 的 buildRequest）：header.config
                // 就是这次实际生效的模型/档位。壳侧的"当前选择"只是意图（内核可能因档位不支持
                // 回落），消息上显示的型号以这里为准，同时补发给等认领的本轮用户气泡。
                {
                    var headerModel = data.ValueKind == JsonValueKind.Object &&
                        data.TryGetProperty("header", out var reqHdr) && reqHdr.ValueKind == JsonValueKind.Object &&
                        reqHdr.TryGetProperty("config", out var reqCfg) && reqCfg.ValueKind == JsonValueKind.Object
                        ? Str(reqCfg, "model")
                        : "";
                    if (headerModel.Length > 0)
                    {
                        _lastHeaderModel = headerModel;
                        if (_pendingUserBubble is { } pending && _messages.Contains(pending))
                        {
                            if (pending.Model != headerModel)
                            {
                                pending.Model = headerModel;
                                RepaintUserActions(pending);
                            }
                            _pendingUserBubble = null;
                        }
                    }
                    return;
                }
            case "turn/end":
                SetPendingThinking(false);
                // 轮尾清掉没等到 header 的用户气泡（请求未发出就失败的轮次）：否则下一轮的
                // header 会把上一轮的提问标成这一轮的模型。
                _pendingUserBubble = null;
                // 越权拒绝/中断的调用没有 result：轮尾把仍在扫光的工具行一并停掉
                StopRunningToolSweeps();
                _runTurn = 0;
                // 进程停下了：撤回编辑按钮随之失效（重绘一次，否则要等容器回收才消失）
                RefreshWithdrawEditButton();
                // 本轮终点：把用时回填到该轮答案气泡（操作行若已装配则重绘）
                if (turn > 0 && _turnStartMs.TryGetValue(turn, out var startMs) &&
                    envTime >= startMs && startMs > 0)
                {
                    _turnStartMs.Remove(turn);
                    if (_transcriptAnswers.TryGetValue(turn, out var turnAnswer))
                    {
                        turnAnswer.DurationMs = envTime - startMs;
                        RefreshTurnActionsRow(turnAnswer);
                    }
                }
                RefreshTranscriptView();
                // 紧凑视图在收尾时折叠中间步骤，列表高度突变：贴底的用户不该被留在空白处。
                ScrollTranscriptToBottom();
                return;
            case "system/message":
                // 过程行（对标官方端）：role=system 的 journal 事件 → 「系统提示词」；
                // 同会话后续出现 = 内核 in-history 更新 → 「系统提示词更新」。
                {
                    var sidSys = Volatile.Read(ref _activeSessionId);
                    var first = true;
                    if (!string.IsNullOrEmpty(sidSys))
                    {
                        var stSys = GetOrCreateRunStats(sidSys);
                        first = !stSys.SystemPromptSeen;
                        stSys.SystemPromptSeen = true;
                    }
                    bubble = new ChatBubble { Role = "tool", Text = L(first ? "系统提示词" : "系统提示词更新"), Turn = turn, IsToolCall = true };
                }
                break;
            case "user/message":
                if (data.ValueKind == JsonValueKind.Object &&
                    data.TryGetProperty("content", out var content) &&
                    content.ValueKind == JsonValueKind.Array)
                {
                    // 内核注入的消息（agent-instructions 的 AGENTS.md、plugin 的 runtime context
                    // 快照等）不再静默跳过：渲染成「上下文注入 · {来源}」过程行，对标官方端。
                    // 标签取 source.plugin / path / label，兜底 kind 本身。
                    if (data.TryGetProperty("source", out var src) &&
                        src.ValueKind == JsonValueKind.Object &&
                        src.TryGetProperty("kind", out var sk) &&
                        sk.ValueKind == JsonValueKind.String &&
                        sk.GetString() != "user")
                    {
                        var label = Str(src, "plugin");
                        if (label.Length == 0) label = Str(src, "path");
                        if (label.Length == 0) label = Str(src, "label");
                        if (label.Length == 0) label = Str(src, "kind");
                        bubble = new ChatBubble { Role = "tool", Text = L("上下文注入") + " · " + label, Turn = turn, IsToolCall = true };
                        break;
                    }
                    var text = string.Join("\n", content.EnumerateArray()
                        .Where(c => c.GetProperty("type").GetString() == "text")
                        .Select(c => c.GetProperty("text").GetString()));
                    if (!string.IsNullOrEmpty(text))
                    {
                        bubble = new ChatBubble { Role = "user", Text = text, Time = envTime, Seq = envSeq };
                        // 新提问落屏：撤回候选若还挂在 turn/start 之前的窗口，到此作废——
                        // journal 按序落盘，被撤回那次 prompt 的 turn/start 若会来，早就排在这条前面。
                        _withdrawPendingTurn = false;
                        // 这条提问的模型要等回答它的 request/header 到达才能确定（header 无 turn
                        // 字段，只能按到达顺序认领）：先登记，header 分支里补。
                        _pendingUserBubble = bubble;
                    }
                    // 图片块：journal 里只存 attachmentId 引用（不存字节），字节经
                    // session/attachment 回读后作为 user-image 气泡渲染。
                    // 与发送链路不重叠：发送走内联 base64/receiptId，这里只服务于历史回放。
                    if (Volatile.Read(ref _activeSessionId) is { } imgSid)
                    {
                        foreach (var block in content.EnumerateArray())
                        {
                            if (block.TryGetProperty("type", out var bt) && bt.GetString() == "image" &&
                                block.TryGetProperty("attachment", out var att) &&
                                att.TryGetProperty("attachmentId", out var aid) &&
                                aid.ValueKind == JsonValueKind.String && aid.GetString() is { Length: > 0 } attachmentId)
                            {
                                var attName = att.TryGetProperty("name", out var an) && an.ValueKind == JsonValueKind.String
                                    ? an.GetString() ?? L("图片")
                                    : L("图片");
                                // 这张图已在发送成功时本地回显过（同名登记）：认领而不回读字节重贴
                                if (!ClaimLocalImageEcho(attName))
                                {
                                    _ = RenderAttachmentImageAsync(imgSid, attachmentId, attName);
                                }
                            }
                        }
                    }
                }
                break;

            case "assistant/attempt":
                // 内核的流式尝试（含错误块 finish{reason.kind:"error"}）——失败要让用户看见
                if (data.ValueKind == JsonValueKind.Object && data.TryGetProperty("stream", out var stream) && stream.ValueKind == JsonValueKind.Array)
                {
                    foreach (var piece in stream.EnumerateArray())
                    {
                        if (piece.TryGetProperty("chunk", out var chunk) &&
                            chunk.TryGetProperty("type", out var ct) && ct.GetString() == "finish")
                        {
                            var reason = chunk.TryGetProperty("reason", out var rr) ? rr : default;
                            if (reason.ValueKind == JsonValueKind.Object &&
                                reason.TryGetProperty("kind", out var rk) && rk.GetString() == "error")
                            {
                                var failure = reason.TryGetProperty("failure", out var fl) ? fl : default;
                                var message = failure.ValueKind == JsonValueKind.Object && failure.TryGetProperty("message", out var fm)
                                    ? fm.GetString() : L("内核执行失败");
                                var code = failure.ValueKind == JsonValueKind.Object && failure.TryGetProperty("code", out var fc)
                                    ? fc.GetString() : null;
                                bubble = new ChatBubble { Role = "assistant", Text = $"⚠ {code}: {message}", Model = _lastHeaderModel };
                            }
                        }
                    }
                }
                break;

            case "assistant/message":
                if (data.ValueKind == JsonValueKind.Object &&
                    data.TryGetProperty("message", out var msg) &&
                    msg.ValueKind == JsonValueKind.Object)
                {
                    var role = msg.TryGetProperty("role", out var r) ? r.GetString() : null;
                    if (role == "assistant" &&
                        msg.TryGetProperty("content", out var ac) && ac.ValueKind == JsonValueKind.Array)
                    {
                        var reasoning = string.Join("", ac.EnumerateArray()
                            .Where(c => Str(c, "type") == "reasoning")
                            .Select(c => Str(c, "text")));
                        var text = string.Join("", ac.EnumerateArray()
                            .Where(c => c.GetProperty("type").GetString() == "text")
                            .Select(c => c.GetProperty("text").GetString()));
                        // message.id 是 messageFeedback 的目标标识（内核按 assistant/message
                        // 事件的 message.id 校验）；缺失时该气泡不给反馈入口。
                        var messageId = msg.TryGetProperty("id", out var mid) && mid.ValueKind == JsonValueKind.String
                            ? mid.GetString()
                            : null;
                        // 用量（对标官方 TurnTailNodeView 的 token 段）：取 usage.totalTokens，
                        // 缺失时按 input+output+cacheRead+cacheWrite 口径（与运行状态条/统计页同源）。
                        var tokens = UsageTokens(data);
                        // 逐字增量已把这条消息铺在屏上：就地定稿同一条气泡，不再追加重复的。
                        ChatBubble? settled = null;
                        var live = TakeLiveAttemptForSettlement(turn);
                        var reasoningDone = live is not null && SettleLiveReasoningBubble(live, reasoning);
                        var textDone = live is not null && SettleLiveTextBubble(live, text, messageId, envSeq, out settled);
                        if (!reasoningDone && reasoning.Length > 0)
                            AppendBubble(new ChatBubble { Role = "reasoning", Text = reasoning, Turn = turn, IsReasoning = true, Time = envTime, Model = _lastHeaderModel });
                        if (!textDone && !string.IsNullOrEmpty(text))
                        {
                            bubble = new ChatBubble
                            {
                                Role = "assistant",
                                Text = text,
                                IsAssistantStep = true,
                                Time = envTime,
                                Seq = envSeq,
                                MessageId = messageId,
                                Tokens = tokens,
                                Model = _lastHeaderModel,
                            };
                            if (IsTranscriptAnswer(msg)) _transcriptAnswers[turn] = bubble;
                        }
                        else if (textDone && settled is not null && IsTranscriptAnswer(msg))
                        {
                            _transcriptAnswers[turn] = settled;
                        }
                        // 逐字气泡就地定稿：补回用量并重绘操作行（行随气泡首次上屏已装配）
                        if (textDone && settled is not null)
                        {
                            settled.Tokens = tokens;
                            RefreshTurnActionsRow(settled);
                        }
                    }
                }
                break;

            case "tool/call" when data.ValueKind == JsonValueKind.Object:
                // 工具调用行（对标官方 ToolRow）：信封事件即 tool/call（带 callId/name/arguments），
                // 壳早期按 web 端旧名 tool/invoke 处理，当前内核树已不存在该事件名 → 工具行从未渲染。
                if (data.TryGetProperty("name", out var tn) && !string.IsNullOrEmpty(tn.GetString()))
                {
                    var args = Str(data, "arguments");
                    var name = tn.GetString()!;
                    // 「本轮文件改动」数据面：登记本调用的突变目标路径（非突变 = 空串）
                    var callId = Str(data, "callId");
                    if (callId.Length > 0)
                    {
                        _openMutationPaths[callId] = MutationPath(name, args) ?? "";
                    }
                    bubble = new ChatBubble
                    {
                        Role = "tool-call",
                        Text = ToolRowFallbackText(name, args),
                        IsToolCall = true,
                        IsToolRunning = true,
                        Time = envTime,
                        ToolArgs = args,
                        ToolName = name,
                    };
                    if (callId.Length > 0)
                    {
                        _runningToolCalls[callId] = bubble;
                    }
                }
                break;

            case "tool/result" when data.ValueKind == JsonValueKind.Object:
                // 「本轮文件改动」数据面（官方 deliverablesDefinition 的 update）：
                // 结果非 error 且 callId 命中突变调用 → 按 result 信封 seq 记一条产出。
                {
                    var rCallId = "";
                    var rIsError = false;
                    if (data.TryGetProperty("message", out var rmsg) && rmsg.ValueKind == JsonValueKind.Object)
                    {
                        if (rmsg.TryGetProperty("source", out var rsrc) && rsrc.ValueKind == JsonValueKind.Object)
                        {
                            rCallId = Str(rsrc, "callId");
                        }
                        if (rmsg.TryGetProperty("content", out var rc) && rc.ValueKind == JsonValueKind.Array)
                        {
                            foreach (var b in rc.EnumerateArray())
                            {
                                if (b.TryGetProperty("isError", out var ie) && ie.ValueKind == JsonValueKind.True)
                                {
                                    rIsError = true;
                                }
                                break; // 官方只看 content[0]
                            }
                        }
                    }
                    // 执行窗口闭合：不管成败，行头流光到此为止（error 结果同样要收尾）。
                    ChatBubble? running = null;
                    if (rCallId.Length > 0) _runningToolCalls.Remove(rCallId, out running);
                    if (running is not null)
                    {
                        running.IsToolRunning = false;
                        RepaintBubble(running);
                    }
                    if (!rIsError && rCallId.Length > 0 &&
                        _openMutationPaths.TryGetValue(rCallId, out var producedPath) && producedPath.Length > 0)
                    {
                        if (!_producedByTurn.TryGetValue(turn, out var plist))
                        {
                            _producedByTurn[turn] = plist = new List<(int Seq, string Path)>();
                        }
                        plist.Add((envSeq, producedPath));
                        // 撤回修改的取证：同一条结果再记一份可反演的突变流水（见 RecordTurnMutation）。
                        // 动过文件即撤掉「撤回编辑」入口——内核侧状态已改，界面单方撤回会造成假象。
                        RecordTurnMutation(turn, running, producedPath, data);
                    }
                }
                break;

            case "session/title":
                // 内核命名链（dsh-session-title）：首条消息立即 fallback 截断（5 词/40 字节），
                // LLM provider 随后追发 provider 命名（first-prompt 模式：仅在会话尚无
                // 标题时生成）。壳在标题事件到达时即时更新侧栏（网页端行为，
                // 曾致壳跟随中会话长期显示"新会话"）。
                if (data.ValueKind == JsonValueKind.Object &&
                    data.TryGetProperty("title", out var st) && st.ValueKind == JsonValueKind.String &&
                    !string.IsNullOrEmpty(st.GetString()))
                {
                    _ = ApplyTitleFromKernelAsync(Volatile.Read(ref _activeSessionId), st.GetString());
                }
                break;

            case "deliverables/presented":
                // 交付物声明（dsh-tool-present 的 present 工具产出；事件形状见
                // dsh-client-ui-deliverables 的 isPresentedData/isPresentedFile 校验）：
                //   data = { turn:<int≥1>, callId:<string>, files:[{path,description?}] }
                // files 的数组下标即内核回查坐标的一部分，必须按下标原样保留。
                // 打开/定位不回内核 RPC，而是宿主路由 /api/present.open（见 OpenPresentedFileAsync）。
                if (data.ValueKind == JsonValueKind.Object &&
                    data.TryGetProperty("files", out var pfiles) && pfiles.ValueKind == JsonValueKind.Array)
                {
                    var seq = ev.TryGetProperty("seq", out var seqEl) && seqEl.ValueKind == JsonValueKind.Number ? seqEl.GetInt32() : 0;
                    var sid = Volatile.Read(ref _activeSessionId) ?? "";
                    var files = new List<PresentedFileVm>();
                    var index = 0;
                    foreach (var f in pfiles.EnumerateArray())
                    {
                        var path = Str(f, "path");
                        if (path.Trim().Length > 0)
                        {
                            files.Add(new PresentedFileVm
                            {
                                Index = index,
                                Path = path,
                                Description = Str(f, "description"),
                                SessionId = sid,
                                Seq = seq,
                            });
                        }
                        index++;
                    }
                    if (files.Count > 0)
                    {
                        // Text 只用于 AppendBubble 的相邻去重（同角色同文本才跳过）——
                        // 交付物气泡不显示 Text，带 seq 保证两次交付不会被误判为重复。
                        bubble = new ChatBubble
                        {
                            Role = "deliverable",
                            Text = LF("交付物 seq={0}：{1}", seq, string.Join("、", files.Select(x => x.Path))),
                            Files = files,
                        };
                    }
                }
                break;
        }

        if (bubble is null)
        {
            return;
        }
        bubble.Turn = type == "user/message" ? 0 : turn;
        AppendBubble(bubble);
    }

    /// <summary>tool/result 成功结算时记一条突变流水（「撤回修改」的取证）。
    /// write/edit 取内核 presentationMeta 的 diffs（before===null 即新建，diffs 为空数组）；
    /// str_replace_editor 没有 meta，按调用参数反推。路径以结果文本里的绝对路径为准——
    /// args 里的 file_path 可能是工作区相对路径，直接拿来读写会指错文件。</summary>
    private void RecordTurnMutation(int turn, ChatBubble? call, string fallbackPath, JsonElement data)
    {
        if (turn <= 0 || call is null)
        {
            // 没有 call 气泡就取不到调用参数（tool/call 被抑制/漏渲染）：无从反演，不记
            return;
        }
        var resultText = ToolResultText(data);
        var path = "";
        var created = false;
        var hunks = new List<TurnHunk>();
        string? editorOld = null;
        string? editorNew = null;
        var editorCommand = "";
        switch (call.ToolName)
        {
            case "write":
                path = ResultPathFromEnvelope(resultText) ?? fallbackPath;
                hunks = HunksFromMeta(data);
                // 新建的唯一正据是结果信封明说 Created file（内核 formatWriteOutput 的口径）。
                // 空 diffs 同时对应「before 为 null 的新建」和「内容与原文件一致的空写」，
                // 结果文本读不到时两者无法区分——此时宁可认不出，也不能把撤回变成删文件。
                created = resultText.Contains("Created file", StringComparison.Ordinal);
                break;
            case "edit":
                path = ResultPathFromEnvelope(resultText) ?? fallbackPath;
                hunks = HunksFromMeta(data);
                break;
            case "str_replace_editor":
                path = ResultPathFromSentence(resultText) ?? fallbackPath;
                editorCommand = ToolArgString(call.ToolArgs, "command") ?? "";
                created = editorCommand == "create";
                editorOld = editorCommand == "str_replace" ? ToolArgString(call.ToolArgs, "old_str") : null;
                editorNew = editorCommand is "str_replace" or "insert" ? ToolArgString(call.ToolArgs, "new_str") : null;
                break;
            default:
                return;
        }
        if (path.Length == 0)
        {
            return;
        }
        if (!_turnMutations.TryGetValue(turn, out var list))
        {
            _turnMutations[turn] = list = new List<TurnFileMutation>();
        }
        list.Add(new TurnFileMutation
        {
            Path = path,
            Created = created,
            Hunks = hunks,
            EditorOld = editorOld,
            EditorNew = editorNew,
            EditorCommand = editorCommand,
        });
        // 动过文件：撤回编辑入口作废（按钮还挂着的话就地重绘撤掉）
        if (!_turnMutated)
        {
            _turnMutated = true;
            RefreshWithdrawEditButton();
        }
    }

    /// <summary>tool/result 事件 data → 结果正文。journal 里 content 是 tool-result 块数组，
    /// 正文在块的 content 里再嵌一层（createToolResultMessage 的形状）。</summary>
    private static string ToolResultText(JsonElement data)
    {
        if (data.ValueKind != JsonValueKind.Object ||
            !data.TryGetProperty("message", out var msg) || msg.ValueKind != JsonValueKind.Object ||
            !msg.TryGetProperty("content", out var blocks) || blocks.ValueKind != JsonValueKind.Array)
        {
            return "";
        }
        var parts = new List<string>();
        foreach (var block in blocks.EnumerateArray())
        {
            if (!block.TryGetProperty("content", out var inner) || inner.ValueKind != JsonValueKind.Array)
            {
                continue;
            }
            foreach (var piece in inner.EnumerateArray())
            {
                if (Str(piece, "type") == "text" && piece.TryGetProperty("text", out var tx) &&
                    tx.ValueKind == JsonValueKind.String && tx.GetString() is { Length: > 0 } text)
                {
                    parts.Add(text);
                }
            }
        }
        return string.Join("\n", parts);
    }

    /// <summary>write/edit 结果信封里的绝对路径（&lt;path&gt;…&lt;/path&gt;，backend 解析后的口径）。</summary>
    private static string? ResultPathFromEnvelope(string text)
    {
        if (text.Length == 0)
        {
            return null;
        }
        var start = text.IndexOf("<path>", StringComparison.Ordinal);
        if (start < 0)
        {
            return null;
        }
        start += "<path>".Length;
        var end = text.IndexOf("</path>", start, StringComparison.Ordinal);
        return end < 0 ? null : text[start..end].Trim();
    }

    /// <summary>str_replace_editor 结果句里的绝对路径（"The file X has been edited successfully." /
    /// "New file created successfully at: X"）。</summary>
    private static string? ResultPathFromSentence(string text)
    {
        const string created = "New file created successfully at: ";
        const string edited = "The file ";
        if (text.StartsWith(created, StringComparison.Ordinal))
        {
            return text[created.Length..].Trim();
        }
        if (text.StartsWith(edited, StringComparison.Ordinal))
        {
            var rest = text[edited.Length..];
            var end = rest.IndexOf(" has been", StringComparison.Ordinal);
            return end < 0 ? null : rest[..end].Trim();
        }
        return null;
    }

    /// <summary>meta.diffs → 改动块（文件序）。内核口径：oldText 为 null = 纯新增，
    /// newText 恒为字符串（空串 = 纯删除）。行尾在反演时才归一化，这里按 LF 切分。</summary>
    private static List<TurnHunk> HunksFromMeta(JsonElement data)
    {
        var list = new List<TurnHunk>();
        if (data.ValueKind != JsonValueKind.Object ||
            !data.TryGetProperty("meta", out var meta) || meta.ValueKind != JsonValueKind.Object ||
            !meta.TryGetProperty("diffs", out var diffs) || diffs.ValueKind != JsonValueKind.Array)
        {
            return list;
        }
        foreach (var diff in diffs.EnumerateArray())
        {
            if (diff.ValueKind != JsonValueKind.Object ||
                !diff.TryGetProperty("newText", out var nt) || nt.ValueKind != JsonValueKind.String)
            {
                continue;
            }
            var oldText = diff.TryGetProperty("oldText", out var ot) && ot.ValueKind == JsonValueKind.String
                ? ot.GetString()
                : null;
            var newText = nt.GetString() ?? "";
            list.Add(new TurnHunk
            {
                Old = oldText is null ? Array.Empty<string>() : oldText.Split('\n'),
                New = newText.Length == 0 ? Array.Empty<string>() : newText.Split('\n'),
            });
        }
        return list;
    }

    /// <summary>从 tool/call 气泡的 arguments 原文里取一个字符串参数（反演 str_replace_editor 用）。</summary>
    private static string? ToolArgString(string? argsRaw, string key)
    {
        if (string.IsNullOrEmpty(argsRaw))
        {
            return null;
        }
        try
        {
            using var doc = JsonDocument.Parse(argsRaw);
            var args = doc.RootElement;
            return args.ValueKind == JsonValueKind.Object && args.TryGetProperty(key, out var v) &&
                   v.ValueKind == JsonValueKind.String
                ? v.GetString()
                : null;
        }
        catch (JsonException)
        {
            return null;
        }
    }

    private void AppendBubble(ChatBubble bubble)
    {
        // 首个气泡上屏即退出空态（品牌标/选择器随消息出现而收起）
        var wasHero = IsHeroState();
        // journal 补拉与本地回显可能相邻重放同一条消息（同角色同文本）：跳过。
        // 已撤回编辑的提问气泡不参与去重——它只是留在表里供认领补记 seq，撤回后重发同一
        // 文本必须照常上屏，不能被它吞掉。
        if (_messages.Count > 0 &&
            _messages[^1] is { Role: var r, Text: var t2 } previous &&
            r == bubble.Role && t2 == bubble.Text && previous.Turn == bubble.Turn &&
            previous.IsReasoning == bubble.IsReasoning && !previous.Withdrawn)
        {
            // 相邻的就是本地回显（无 seq）：受理到首字之间 journal 的用户记录可能先到，此时既没有
            // 逐字气泡可插、也不能把这条记录丢掉——回显要认领 seq/时间/模型，否则它停在本地发送
            // 时间且带不上内核确认的信息，重开会话后与历史记录也对不上。
            if (bubble.Role == "user" && bubble.Seq > 0 && previous is { Seq: 0 })
            {
                previous.Seq = bubble.Seq;
                NoteWithdrawnSeq(previous);
                if (bubble.Time > 0)
                {
                    previous.Time = bubble.Time;
                }
                if (bubble.Model.Length > 0 && previous.Model != bubble.Model)
                {
                    previous.Model = bubble.Model;
                    RepaintUserActions(previous);
                }
            }
            return;
        }
        if (bubble.Role == "user")
        {
            // prompt 一受理内核就开始出字，逐字气泡可能比 journal 的 user 记录先到：
            // 用户消息要插在那批气泡之前，否则提问会排到回答下面。
            if (bubble.Seq > 0 && FindLocalUserEcho(bubble.Text) is { } echo)
            {
                echo.Seq = bubble.Seq;          // 认领本地回显，不重复建气泡
                NoteWithdrawnSeq(echo);
                if (bubble.Time > 0) echo.Time = bubble.Time;
                if (bubble.Model.Length > 0 && echo.Model != bubble.Model)
                {
                    echo.Model = bubble.Model;
                    RepaintUserActions(echo);
                }
                return;
            }
            // 反方向配对：本地回显晚于 journal 记录上屏（图片解码的等待窗口里，
            // 模型的逐字回答已挤到两者中间，相邻去重够不着）。记录已在屏上，
            // 回显只把待定信息（模型标注/撤回候选）补到记录上，不再插一条。
            if (bubble.Seq == 0 && FindJournalUserEcho(bubble.Text) is { } journal)
            {
                if (bubble.Model.Length > 0 && journal.Model != bubble.Model)
                {
                    journal.Model = bubble.Model;
                    RepaintUserActions(journal);
                }
                if (ReferenceEquals(_pendingUserBubble, bubble))
                {
                    _pendingUserBubble = journal; // header 认领要落在屏上的气泡
                }
                if (ReferenceEquals(_withdrawCandidate, bubble))
                {
                    _withdrawCandidate = journal;
                }
                return;
            }
            _messages.Insert(UserInsertIndex(), bubble);
        }
        else
        {
            _messages.Add(bubble);
        }
        if (_messages.Count > 400)
        {
            _messages.RemoveAt(0);
        }
        ScrollTranscriptToBottom(force: true);
        // 首个**对话**气泡落屏才收起空态：system 回显不改变相位（见 IsHeroState），
        // 空会话上改权限模式后工作区/模式选择器必须留在原处。
        if (wasHero && !IsHeroState())
        {
            UpdateEmptyState();
        }
    }

    /// <summary>就近找同文本的本地回显用户气泡（回显没有 seq，journal 记录带 seq）。
    /// 已撤回编辑的气泡跳过：它的 seq 只用于补记抑制表（见 NoteWithdrawnSeq），
    /// 不能被新发送的同文本消息认领走。</summary>
    private ChatBubble? FindLocalUserEcho(string? text)
    {
        for (var i = _messages.Count - 1; i >= 0 && i >= _messages.Count - 12; i--)
        {
            if (_messages[i] is { Role: "user", Seq: 0 } candidate && !candidate.Withdrawn && candidate.Text == text)
            {
                return candidate;
            }
        }
        return null;
    }

    /// <summary>就近找同文本的 journal 用户气泡（Seq>0）。本地回显因图片解码晚到时，
    /// 对应的 journal 记录可能已先上屏且被逐字回答挤到后面——回显认领它而不重复插一条
    /// （FindLocalUserEcho 的反方向配对，扫描方向相反取各自最近的配对）。
    /// 已撤回编辑的气泡跳过：重发同一文本必须照常上屏。</summary>
    private ChatBubble? FindJournalUserEcho(string? text)
    {
        for (var i = _messages.Count - 1; i >= 0 && i >= _messages.Count - 12; i--)
        {
            if (_messages[i] is { Role: "user", Seq: > 0 } candidate && !candidate.Withdrawn && candidate.Text == text)
            {
                return candidate;
            }
        }
        return null;
    }

    /// <summary>撤回编辑的提问气泡被 journal 记录认领到时补记 seq：撤回点在回显尚未落定前
    /// （Seq 仍为 0）时，seq 只能在这里拿到；不补记的话切会话回放会把这条提问带回来。</summary>
    private void NoteWithdrawnSeq(ChatBubble bubble)
    {
        if (!bubble.Withdrawn || bubble.Seq <= 0 || Volatile.Read(ref _activeSessionId) is not { } sid)
        {
            return;
        }
        WithdrawnSeqs(sid).Add(bubble.Seq);
    }

    /// <summary>释放一次本地图片回显登记（撤回编辑把该消息的图片气泡撤下时用）：
    /// 登记不跟着作废的话，重发同名图会被误认领成「已在屏上」而不再贴出。</summary>
    private void ReleaseLocalImageEcho(string name)
    {
        if (!_localImageEchoes.TryGetValue(name, out var left))
        {
            return;
        }
        if (left > 1)
        {
            _localImageEchoes[name] = left - 1;
        }
        else
        {
            _localImageEchoes.Remove(name);
        }
    }

    /// <summary>最后一个用户提问气泡（turn/start 时按它移交撤回编辑候选）。</summary>
    private ChatBubble? LastUserBubble()
    {
        for (var i = _messages.Count - 1; i >= 0; i--)
        {
            if (_messages[i] is { Role: "user" } user)
            {
                return user;
            }
        }
        return null;
    }

    /// <summary>撤回编辑按钮的可见性随「进程在跑 / 本轮动过文件」变化，状态翻转时就地重绘候选行。
    /// 容器未 realize 时重绘自然跳过，将来 realize 会按最新值装配。</summary>
    private void RefreshWithdrawEditButton()
    {
        if (_withdrawCandidate is { } candidate && _messages.Contains(candidate))
        {
            RepaintUserActions(candidate);
        }
    }

    /// <summary>
    /// 按图片名认领一次本地回显：登记里有同名待认领计数就扣掉一并返回 true（这条 journal
    /// 图片已在屏上，不重复贴）；计数归零或从未登记返回 false（走历史回读）。
    /// 计数而非布尔：一次发送可带多张同名图（如多次粘贴同默认名），逐个对应。
    /// </summary>
    private bool ClaimLocalImageEcho(string name)
    {
        if (_localImageEchoes.TryGetValue(name, out var left) && left > 0)
        {
            if (left > 1)
            {
                _localImageEchoes[name] = left - 1;
            }
            else
            {
                _localImageEchoes.Remove(name);
            }
            return true;
        }
        return false;
    }

    /// <summary>用户气泡落位：末尾那段仍在逐字生成的气泡是这条提问的下游，插在它们之前。</summary>
    private int UserInsertIndex()
    {
        var i = _messages.Count;
        while (i > 0 && _messages[i - 1].IsLiveStreaming)
        {
            i--;
        }
        return i;
    }

    /// <summary>把内核 session/title 事件应用到侧栏（dsh 原版 web 的即时命名行为：
    /// 标题事件到达即更新列表显示）。任意线程可调。</summary>
    private async Task ApplyTitleFromKernelAsync(string? sessionId, string title)
    {
        if (sessionId is null)
        {
            return;
        }
        var vm = _sessions.FirstOrDefault(s => s.SessionId == sessionId);
        if (vm is null)
        {
            return;
        }
        if (DispatcherQueue.HasThreadAccess)
        {
            ApplyTitleCore(vm, title);
        }
        else
        {
            PostUi(() => ApplyTitleCore(vm, title));
        }
        await Task.CompletedTask;
    }

    /// <summary>标题落位：fallback/provider 命名只替换兜底显示（新会话/cwd 目录名/sessionId）；
    /// 内核链里 user 改名优先级最高，但事件流按到达顺序回放——非兜底且不同的标题
    /// 也允许落位（RefreshSessions 随后以 list 投影为准自然校正）。</summary>
    private void ApplyTitleCore(SessionVm vm, string title)
    {
        if (vm.Title == title)
        {
            return;
        }
        vm.Title = title;
        RebuildNavMenu();
        // 会话头标题是侧栏之外唯一的身份展示位：改名要同步（当前会话才可见）
        if (vm.SessionId == Volatile.Read(ref _activeSessionId))
        {
            RefreshSessionHeader();
        }
    }

    // ---------------- 输入与发送 ----------------

    // ---------------- 附件（图片粘贴/文件选取 → 预览条 → 随消息发送） ----------------
    /// <summary>＋按钮：FileOpenPicker 选图片或文件。</summary>
    private async void OnAttachClick(object sender, RoutedEventArgs e)
    {
        var picker = new Windows.Storage.Pickers.FileOpenPicker();
        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
        WinRT.Interop.InitializeWithWindow.Initialize(picker, hwnd);
        picker.FileTypeFilter.Add("*");
        picker.SuggestedStartLocation = Windows.Storage.Pickers.PickerLocationId.DocumentsLibrary;
        var files = await picker.PickMultipleFilesAsync();
        foreach (var f in files)
        {
            await AddAttachmentAsync(f.Path, f.Name);
        }
    }

    /// <summary>
    /// 历史图片回读（session/attachment：request{sessionId, attachmentId} → {attachment, data:base64}）。
    /// journal 只带 attachmentId，字节要从内核取；结果按 attachmentId 缓存（同一图片只取一次），
    /// 已渲染过的 attachmentId 记入 _renderedAttachmentIds，避免 journal 补拉与 follow 流重复贴图。
    /// </summary>
    private async Task RenderAttachmentImageAsync(string sessionId, string attachmentId, string name)
    {
        if (_rpc is null || !_renderedAttachmentIds.Add(attachmentId))
        {
            return;
        }
        Microsoft.UI.Xaml.Media.ImageSource? image = null;
        if (_attachmentImages.TryGetValue(attachmentId, out var cached))
        {
            image = cached;
        }
        else
        {
            try
            {
                var value = await _rpc.CallOkAsync("session/attachment", new { request = new { sessionId, attachmentId } });
                var base64 = value.TryGetProperty("data", out var d) && d.ValueKind == JsonValueKind.String ? d.GetString() : null;
                if (!string.IsNullOrEmpty(base64))
                {
                    var bytes = Convert.FromBase64String(base64);
                    // 壳内不回读超大图（内核单图上限 10MB 量级，超过就只留占位行）
                    if (bytes.Length <= 16 * 1024 * 1024)
                    {
                        image = await MakeBitmapAsync(bytes);
                    }
                }
            }
            catch (Exception)
            {
                // 取不到字节（已过期/未引用）时保持静默：文本气泡照常显示
            }
            _attachmentImages[attachmentId] = image;
        }
        if (image is null)
        {
            return;
        }
        PostUi(() =>
        {
            // 会话已切换：丢弃（图片属于原会话）
            if (sessionId != Volatile.Read(ref _activeSessionId))
            {
                return;
            }
            _messages.Add(new ChatBubble { Role = "user-image", Text = name, Image = image });
            if (_messages.Count > 400)
            {
                _messages.RemoveAt(0);
            }
            ScrollTranscriptToBottom(force: true);
        });
    }

    /// <summary>字节 → BitmapImage（必须在 UI 线程调用；DataWriter 写完再 Seek(0) 交给解码器）。
    /// decodePixelWidth：按目标尺寸解码（缩略图用小值，省掉全尺寸位图的内存）。</summary>
    private static async Task<Microsoft.UI.Xaml.Media.ImageSource> MakeBitmapAsync(byte[] bytes, int? decodePixelWidth = null)
    {
        using var stream = new Windows.Storage.Streams.InMemoryRandomAccessStream();
        using (var writer = new Windows.Storage.Streams.DataWriter(stream))
        {
            writer.WriteBytes(bytes);
            await writer.StoreAsync();
            await writer.FlushAsync();
            writer.DetachStream();
        }
        stream.Seek(0);
        var bitmap = new Microsoft.UI.Xaml.Media.Imaging.BitmapImage();
        if (decodePixelWidth is > 0)
        {
            bitmap.DecodePixelWidth = decodePixelWidth.Value;
        }
        await bitmap.SetSourceAsync(stream);
        return bitmap;
    }

    /// <summary>输入框粘贴：剪贴板有位图时截为图片附件。</summary>
    private async void OnInputPaste(object sender, TextControlPasteEventArgs e)
    {
        var content = Windows.ApplicationModel.DataTransfer.Clipboard.GetContent();
        if (content.Contains(Windows.ApplicationModel.DataTransfer.StandardDataFormats.Bitmap))
        {
            e.Handled = true;
            await AddBitmapAttachmentAsync(content, LF("粘贴图片 {0:HHmmss}.png", DateTime.Now));
        }
    }

    /// <summary>按魔数识别内核接受的四种图片格式。扩展名会撒谎（.png 里可能是 JPEG），
    /// 剪贴板位图流也不保证是 PNG（可能是 BMP/DIB），字节本身不会。</summary>
    private static string? SniffImageMediaType(ReadOnlySpan<byte> b)
    {
        if (b.Length >= 8 && b[0] == 0x89 && b[1] == 0x50 && b[2] == 0x4E && b[3] == 0x47 &&
            b[4] == 0x0D && b[5] == 0x0A && b[6] == 0x1A && b[7] == 0x0A)
        {
            return "image/png";
        }
        if (b.Length >= 3 && b[0] == 0xFF && b[1] == 0xD8 && b[2] == 0xFF)
        {
            return "image/jpeg";
        }
        if (b.Length >= 6 && b[0] == (byte)'G' && b[1] == (byte)'I' && b[2] == (byte)'F' && b[3] == (byte)'8' &&
            (b[4] == (byte)'7' || b[4] == (byte)'9') && b[5] == (byte)'a')
        {
            return "image/gif";
        }
        if (b.Length >= 12 && b[0] == (byte)'R' && b[1] == (byte)'I' && b[2] == (byte)'F' && b[3] == (byte)'F' &&
            b[8] == (byte)'W' && b[9] == (byte)'E' && b[10] == (byte)'B' && b[11] == (byte)'P')
        {
            return "image/webp";
        }
        return null;
    }

    /// <summary>
    /// 附件图片规范化：任意来源字节 → 内核必收的 (bytes, mediaType, name)。
    /// 内核只认 png/jpeg/webp/gif 四种，且按声明的 mediaType 与字节实际格式严格比对：
    /// 把 BMP/DIB（剪贴位图流的常见形态）或扩展名不符的字节按 image/png 上报，
    /// 会被 sharp 拒成 session/attachment-invalid「Unsupported or malformed image data」。
    /// 这里按魔数定真实类型；壳还能解的其它格式（BMP/TIFF/ICO）转码成 PNG 再发；
    /// 壳也解不了（HEIC 等）或已损坏的返回 null，由调用方降级为普通文件附件——发送永不失败。
    /// </summary>
    private static async Task<(byte[] Bytes, string MediaType, string Name)?> NormalizeImageAsync(byte[] bytes, string name)
    {
        if (bytes.Length == 0)
        {
            return null;
        }
        if (SniffImageMediaType(bytes) is { } sniffed)
        {
            return (bytes, sniffed, name);
        }
        try
        {
            var png = await TranscodeToPngAsync(bytes);
            if (png is { Length: > 0 })
            {
                return (png, "image/png", System.IO.Path.ChangeExtension(name, ".png"));
            }
        }
        catch (Exception)
        {
            // 壳解不了：降级由调用方处理
        }
        return null;
    }

    /// <summary>任意壳内可解码位图（BMP/TIFF/ICO…）→ PNG 字节（UI 线程调用）。</summary>
    private static async Task<byte[]?> TranscodeToPngAsync(byte[] bytes)
    {
        using var source = new Windows.Storage.Streams.InMemoryRandomAccessStream();
        using (var writer = new Windows.Storage.Streams.DataWriter(source))
        {
            writer.WriteBytes(bytes);
            await writer.StoreAsync();
            await writer.FlushAsync();
            writer.DetachStream();
        }
        source.Seek(0);
        var decoder = await Windows.Graphics.Imaging.BitmapDecoder.CreateAsync(source);
        using var target = new Windows.Storage.Streams.InMemoryRandomAccessStream();
        var encoder = await Windows.Graphics.Imaging.BitmapEncoder.CreateAsync(
            Windows.Graphics.Imaging.BitmapEncoder.PngEncoderId, target);
        using var bitmap = await decoder.GetSoftwareBitmapAsync();
        encoder.SetSoftwareBitmap(bitmap);
        await encoder.FlushAsync();
        target.Seek(0);
        using var outStream = target.AsStreamForRead();
        using var ms = new MemoryStream();
        await outStream.CopyToAsync(ms);
        return ms.ToArray();
    }

    /// <summary>位图载荷（剪贴板粘贴 / 从浏览器或截图工具拖入）→ 图片附件。
    /// 粘贴与拖图共用同一条「位图规范化」路径：流不保证是 PNG，按魔数定类型、必要时转码，
    /// 避免硬编码 image/png 被内核拒收（session/attachment-invalid）。</summary>
    private async Task AddBitmapAttachmentAsync(Windows.ApplicationModel.DataTransfer.DataPackageView content, string name)
    {
        var bmp = await content.GetBitmapAsync();
        using var ras = await bmp.OpenReadAsync();
        using var stream = ras.AsStreamForRead();
        using var ms = new MemoryStream();
        await stream.CopyToAsync(ms);
        var normalized = await NormalizeImageAsync(ms.ToArray(), name);
        if (normalized is null)
        {
            AppendSystemMessage(LF("图片读取失败，未添加附件：{0}", name));
            return;
        }
        await AddPendingAttachment(new AttachmentVm
        {
            Name = normalized.Value.Name,
            IsImage = true,
            MediaType = normalized.Value.MediaType,
            DataBase64 = Convert.ToBase64String(normalized.Value.Bytes),
        });
    }

    /// <summary>拖拽悬停：只接「带文件或位图」的载荷（给复制光标 + 卡片描边高亮）；
    /// 纯文本等不接也不标记已处理，输入框原生的文本拖放照常生效。</summary>
    private void OnComposerDragOver(object sender, DragEventArgs e)
    {
        var view = e.DataView;
        if (view.Contains(Windows.ApplicationModel.DataTransfer.StandardDataFormats.StorageItems) ||
            view.Contains(Windows.ApplicationModel.DataTransfer.StandardDataFormats.Bitmap))
        {
            e.AcceptedOperation = Windows.ApplicationModel.DataTransfer.DataPackageOperation.Copy;
            e.Handled = true;
            ComposerCard.BorderBrush = ComposerThemeBrush("AccentBrush");
        }
    }

    private void OnComposerDragLeave(object sender, DragEventArgs e)
        => ComposerCard.BorderBrush = ComposerThemeBrush("StrokeBrush");

    /// <summary>拖拽落入输入卡：文件走既有附件管线（图片 base64 / 其余 receipt 上传）；
    /// 没有文件载荷时（从浏览器拖图）回退到位图 → PNG 附件。</summary>
    private async void OnComposerDrop(object sender, DragEventArgs e)
    {
        var view = e.DataView;
        var hasFiles = view.Contains(Windows.ApplicationModel.DataTransfer.StandardDataFormats.StorageItems);
        var hasBitmap = view.Contains(Windows.ApplicationModel.DataTransfer.StandardDataFormats.Bitmap);
        if (!hasFiles && !hasBitmap)
        {
            return; // 纯文本等：不接，留给输入框原生处理
        }
        // 读存储项/位图都要 await：拿 deferral 把 Drop 事件挂起，读完再放行
        var deferral = e.GetDeferral();
        try
        {
            var added = 0;
            if (hasFiles)
            {
                var items = await view.GetStorageItemsAsync();
                foreach (var item in items)
                {
                    if (item is Windows.Storage.StorageFile file)
                    {
                        await AddAttachmentAsync(file.Path, file.Name);
                        added++;
                    }
                }
            }
            if (added == 0 && hasBitmap)
            {
                await AddBitmapAttachmentAsync(view, LF("拖入图片 {0:HHmmss}.png", DateTime.Now));
            }
        }
        catch (Exception)
        {
            // 拖入的文件读不了（被占用/无权限）时静默：附件条保持原样，不打断输入
        }
        finally
        {
            deferral.Complete();
        }
        e.Handled = true;
        ComposerCard.BorderBrush = ComposerThemeBrush("StrokeBrush");
    }

    /// <summary>主题笔刷（Token 字典随亮/暗主题切换，每次现取不缓存）。</summary>
    private static Microsoft.UI.Xaml.Media.Brush ComposerThemeBrush(string key)
        => (Microsoft.UI.Xaml.Media.Brush)Application.Current.Resources[key];

    private async Task AddAttachmentAsync(string path, string name)
    {
        if (path.Length == 0)
        {
            return;
        }
        var ext = System.IO.Path.GetExtension(path).ToLowerInvariant();
        // 图片候选扩展名放宽到壳能解码的格式（bmp/tif/ico 等）：真实类型由字节魔数定，
        // 规范化失败（HEIC/损坏文件）才降级为普通文件附件。
        var maybeImage = ext is ".png" or ".jpg" or ".jpeg" or ".webp" or ".gif" or ".bmp" or ".dib" or ".tif" or ".tiff" or ".ico";
        if (maybeImage)
        {
            var bytes = await File.ReadAllBytesAsync(path);
            if (await NormalizeImageAsync(bytes, name) is { } normalized)
            {
                await AddPendingAttachment(new AttachmentVm
                {
                    Name = normalized.Name,
                    IsImage = true,
                    MediaType = normalized.MediaType,
                    DataBase64 = Convert.ToBase64String(normalized.Bytes),
                });
                return;
            }
            // 壳解不了/已损坏：落到下面的文件附件分支，消息照常发出（模型拿到的是文件而非图片）
        }
        await AddPendingAttachment(new AttachmentVm { Name = name, IsImage = false, FilePath = path });
    }

    /// <summary>附件 chip：图片显示缩略图（40dip 取景框 + 白色圆角移除键，对标官方端），
    /// 其余文件保持「📄 名 + 移除键」。根节点恒为 StackPanel、移除按钮恒为直接子级——
    /// 发送与命令两条路径的清理都按「chip.Children 里找 Tag=AttachmentVm 的 Button」
    /// 识别附件（见 OnSendClick / ExecuteCommandAsync），结构不能动：缩略图预览 Button
    /// 不设 Tag，清理扫描天然跳过它。</summary>
    private async Task AddPendingAttachment(AttachmentVm att)
    {
        _pendingAttachments.Add(att);
        var chip = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6, VerticalAlignment = VerticalAlignment.Center };
        Microsoft.UI.Xaml.Controls.Image? thumb = null;
        if (att.IsImage && att.DataBase64.Length > 0)
        {
            thumb = new Microsoft.UI.Xaml.Controls.Image
            {
                Width = 38,
                Height = 38,
                Stretch = Microsoft.UI.Xaml.Media.Stretch.UniformToFill,
            };
            // 取景框：1dip 描边 + 小圆角，把预览图框住（解码完成前是空框占位）。
            // 取景框外包一层透明 Button = 发送前点缩略图放大全图：chip 里只解了 96dip
            // 缩略，放大层按全尺寸 DataBase64 现解（与发送后气泡共用同一浮层）。
            var preview = Aut(new Button
            {
                Content = new Microsoft.UI.Xaml.Controls.Border
                {
                    Width = 40,
                    Height = 40,
                    CornerRadius = new CornerRadius(4),
                    BorderThickness = new Thickness(1),
                    BorderBrush = ComposerThemeBrush("StrokeBrush"),
                    Child = thumb,
                },
                Padding = new Thickness(0),
                MinWidth = 0,
                MinHeight = 0,
                Background = new Microsoft.UI.Xaml.Media.SolidColorBrush(Windows.UI.Color.FromArgb(0, 0, 0, 0)),
                BorderThickness = new Thickness(0),
                CornerRadius = new CornerRadius(4),
            }, "PendingAttachmentPreviewButton", LF("放大查看 {0}", att.Name));
            preview.Click += async (_, _) =>
            {
                try
                {
                    var full = await MakeBitmapAsync(Convert.FromBase64String(att.DataBase64));
                    // 等待解码期间附件可能已被移除：chip 都没了就不弹
                    if (_pendingAttachments.Contains(att))
                    {
                        ShowImageLightbox(full, att.Name);
                    }
                }
                catch (Exception)
                {
                    // 图裂了/超大：不弹放大层，附件本身照常可发送
                }
            };
            chip.Children.Add(preview);
        }
        else
        {
            chip.Children.Add(new TextBlock
            {
                Text = "📄 " + att.Name,
                VerticalAlignment = VerticalAlignment.Center,
                TextTrimming = Microsoft.UI.Xaml.TextTrimming.CharacterEllipsis,
                MaxWidth = 180,
            });
        }
        var remove = Aut(new Button
        {
            Content = "×",
            Style = AppStyle("CompactButtonStyle"),
            Tag = att,
            Width = 18,
            Height = 18,
            MinWidth = 0,
            MinHeight = 0,
            Padding = new Thickness(0),
            FontSize = 11,
            CornerRadius = new CornerRadius(9),
            Background = new Microsoft.UI.Xaml.Media.SolidColorBrush(Windows.UI.Color.FromArgb(0xFF, 0xFF, 0xFF, 0xFF)),
            Foreground = new Microsoft.UI.Xaml.Media.SolidColorBrush(Windows.UI.Color.FromArgb(0xFF, 0x44, 0x44, 0x44)),
        }, "RemovePendingAttachmentButton", LF("移除附件 {0}", att.Name));
        remove.Click += (_, _) =>
        {
            _pendingAttachments.Remove(att);
            AttachmentList.Children.Remove(chip);
            if (_pendingAttachments.Count == 0)
            {
                AttachmentStrip.Visibility = Visibility.Collapsed;
            }
        };
        chip.Children.Add(remove);
        AttachmentList.Children.Add(chip);
        AttachmentStrip.Visibility = Visibility.Visible;
        if (thumb is not null)
        {
            // 缩略图异步解码后回填（按 96dip 解码：chip 只用到 38dip，省掉全尺寸位图）；
            // 期间附件已可发送（发送读的是 base64，不等缩略图），用户已移除则丢弃结果。
            try
            {
                var source = await MakeBitmapAsync(Convert.FromBase64String(att.DataBase64), decodePixelWidth: 96);
                if (_pendingAttachments.Contains(att))
                {
                    thumb.Source = source;
                }
            }
            catch (Exception)
            {
                // 图裂了/超大：留空框占位，附件本身照常可发送
            }
        }
    }

    /// <summary>上传文件附件：POST /api/session/uploadFileBinary?sessionId=&name=（octet-stream）→ receiptId。
    /// 失败直接抛：附件传不上去还照发文本，模型收不到文件，用户以为发出去了（静默丢附件的教训）。</summary>
    private async Task<string?> UploadFileAsync(string sessionId, AttachmentVm att)
    {
        var bytes = await File.ReadAllBytesAsync(att.FilePath);
        return await UploadFileBytesAsync(sessionId, bytes, att.Name);
    }

    /// <summary>字节直传（无本地路径时用：降级重发的图片字节来自 base64，落盘再读纯属绕路）。</summary>
    private async Task<string?> UploadFileBytesAsync(string sessionId, byte[] bytes, string name)
    {
        if (_rpc is null)
        {
            throw new DshRpcException("no-kernel", "内核未连接");
        }
        var baseUri = new Uri(_rpc.BaseUri, $"api/session/uploadFileBinary?sessionId={Uri.EscapeDataString(sessionId)}&name={Uri.EscapeDataString(name)}");
        return await _rpc.UploadBytesAsync(baseUri, bytes);
    }

    /// <summary>
    /// 输入框 PreviewKeyDown：浮层优先，其次 Enter 默认发送。
    ///
    /// 为什么必须用 PreviewKeyDown 而不是 KeyDown：InputBox 是 AcceptsReturn=True 的多行
    /// TextBox，Enter 会被控件自身的编辑逻辑先行消费——实测（Diag 日志）Escape/Down 都能到
    /// KeyDown，Enter 到不了。Tab 能到是因为 Tab 不是编辑键。走隧道阶段（Preview）才能保证
    /// "浮层打开时 Enter = 选中候选"，以及"浮层关闭时 Enter = 发送"而不是插入换行。
    /// Shift+Enter 一律放行（换行，TextBox 默认行为）。
    /// </summary>
    private void OnInputPreviewKeyDown(object sender, KeyRoutedEventArgs e)
    {
        try
        {
            // 命令补全浮层打开时，上下/Enter/Esc/Tab 归浮层（焦点始终留在输入框）
            if (CommandPalette.Visibility == Visibility.Visible && HandlePaletteKey(e))
            {
                return;
            }
            // @ 引用浮层同理（命令浮层优先：前导 / 时不会同时开两个）
            if (ReferencePalette.Visibility == Visibility.Visible && HandleReferenceKey(e))
            {
                return;
            }
            // 浮层没消费时：Enter 默认为发送（必须在隧道阶段拦截，否则 TextBox 直接换行）。
            if (e.Key == Windows.System.VirtualKey.Enter)
            {
                TryHandleEnterSend(e);
            }
        }
        catch (Exception)
        {
            // 浮层按键处理异常不上抛（0xc000027b 教训）
        }
    }

    /// <summary>Enter 发送的统一判定：Shift+Enter 放行换行，其余置 Handled 并提交。</summary>
    /// <returns>是否已消费（发送）。</returns>
    private bool TryHandleEnterSend(KeyRoutedEventArgs e)
    {
        var shift = Microsoft.UI.Input.InputKeyboardSource.GetKeyStateForCurrentThread(Windows.System.VirtualKey.Shift);
        if (shift.HasFlag(Windows.UI.Core.CoreVirtualKeyStates.Down))
        {
            return false; // Shift+Enter 换行（TextBox 默认行为）
        }
        e.Handled = true;
        var ctrl = Microsoft.UI.Input.InputKeyboardSource.GetKeyStateForCurrentThread(Windows.System.VirtualKey.Control);
        if (ctrl.HasFlag(Windows.UI.Core.CoreVirtualKeyStates.Down))
        {
            // Ctrl+Enter：繁忙时取 busyEnter 的反向档，空闲时沿用默认发送。
            _ = SubmitInputAsync(forceMode: IsSessionBusy() ? AlternateBusyEnter() : null);
        }
        else
        {
            _ = SubmitInputAsync();
        }
        return true;
    }

    private void OnInputKeyDown(object sender, KeyRoutedEventArgs e)
    {
        // 主路径已在 OnInputPreviewKeyDown（隧道阶段）处理；此处仅作兜底
        //（万一某条键路没走隧道，Enter 仍默认发送）。
        if (e.Key == Windows.System.VirtualKey.Enter && !e.Handled)
        {
            TryHandleEnterSend(e);
        }
    }

    /// <summary>
    /// 输入框 Enter 的统一入口：前导 "/" 且命中会话命令目录 → commands/execute；
    /// 否则照旧 session/prompt（既有发送路径完全不变）。
    /// 目录未命中（未知命令/目录取不到）时不吞消息，按普通提示词发送。
    /// </summary>
    private async Task SubmitInputAsync(string? forceMode = null)
    {
        var text = InputBox.Text ?? "";
        if (TryLeadingCommandName(text, out var name))
        {
            await EnsureCommandsAsync();
            if (_commands.Any(c => string.Equals(c.Name, name, StringComparison.OrdinalIgnoreCase)))
            {
                var line = text.Trim();
                HideCommandPalette();
                await ExecuteCommandAsync(line, fromComposer: true);
                return;
            }
        }
        await SendAsync(forceMode);
    }

    /// <summary>取前导斜杠命令名（"/compact now" → compact）；不是命令形态返回 false。</summary>
    private static bool TryLeadingCommandName(string text, out string name)
    {
        name = "";
        var trimmed = text.TrimStart();
        if (trimmed.Length < 2 || trimmed[0] != '/')
        {
            return false;
        }
        var end = trimmed.IndexOfAny([' ', '\t', '\r', '\n'], 1);
        name = (end < 0 ? trimmed[1..] : trimmed[1..end]).Trim();
        return name.Length > 0;
    }

    /// <summary>会话繁忙态（api-session/status 缓存）。</summary>
    private bool IsSessionBusy() =>
        _activeSessionId is { } sid && _sessionRunning.TryGetValue(sid, out var running) && running;

    /// <summary>busyEnter 设置的反向档（queue↔steer）：Ctrl+Enter 用的"另一行为"。</summary>
    private string AlternateBusyEnter() =>
        NsString("ui-conversation", "busyEnter", "queue") == "steer" ? "queue" : "steer";

    private async void OnSendClick(object sender, RoutedEventArgs e)
    {
        // async void 边界兜底：发送链路任何漏网异常都在此处消化，不进未处理异常（0xc000027b）
        try
        {
            // 会话忙碌时这颗键就是「停止」（图标已切成方块）：直接取消运行，不排队发送。
            // 占位气泡在挂上时按钮也显示停止（prompt 已受理、status 事件还没回的窗口），点击语义要一致。
            if (IsSessionBusy() || _pendingBubble is not null)
            {
                await StopActiveRunAsync();
                return;
            }
            // 子代理会话只读（官方 SubagentReadOnlyComposer）：输入区已禁用，此处按会话 origin
            // 兜底——键盘/自动化路径若绕过禁用态，也不允许往子代理会话写消息。提示在会话头常驻
            // （SubagentReadOnlyHint），这里静默拒绝即可，不用错误级弹窗。
            if (_sessions.FirstOrDefault(s => s.SessionId == Volatile.Read(ref _activeSessionId)) is { IsSubagent: true })
            {
                return;
            }
            await SubmitInputAsync();
        }
        catch (Exception ex)
        {
            _ = ShowErrorAsync(ex.Message);
        }
    }

    /// <summary>停止当前会话的运行（session/cancel）。输入区无内容时的发送键语义。</summary>
    private async Task StopActiveRunAsync()
    {
        if (_rpc is null || Volatile.Read(ref _activeSessionId) is not { } sid)
        {
            return;
        }
        try
        {
            // 0.7.0 探针实测：cancel 的参数键是 request（_request 会 arguments-invalid）
            await _rpc.CallOkAsync("session/cancel", new { request = new { sessionId = sid } });
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("取消失败：{0}", ex.Message));
        }
    }

    /// <summary>发送键的忙碌态外观：忙碌 = 方块停止图标 + 「停止运行」，空闲 = 箭头 + 「发送消息」。
    /// 幂等：状态没变时不重设（避免每帧刷 AutomationProperties 打断读屏）。</summary>
    private void UpdateComposerRunningState(bool force = false)
    {
        // 占位气泡也算忙碌：prompt 已发出、status 事件还没回的窗口里，按钮就该是停止。
        var running = IsSessionBusy() || _pendingBubble is not null;
        if (!force && _composerRunning == running)
        {
            return;
        }
        _composerRunning = running;
        var label = running ? L("停止运行") : L("发送消息");
        SendButton.Content = new FontIcon
        {
            Glyph = running ? "\uE71A" : "\uE74A",   // Stop / Send
            FontSize = TokenDouble("GlyphSizeBody", 16),
        };
        ToolTipService.SetToolTip(SendButton, label);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(SendButton, label);
    }

    /// <summary>模式菜单单选：更新选中态与按钮标签（ToggleMenuFlyoutItem 不互斥，手动清其他项）。</summary>
    private void OnEffortMenuClick(object sender, RoutedEventArgs e)
    {
        if (sender is not RadioMenuFlyoutItem item || item.Tag is not string effort)
        {
            return;
        }
        SetEffortUi(effort);
    }

    /// <summary>发送。forceMode 指定时用其覆盖 prompt mode（Ctrl+Enter 的"另一行为"）；
    /// 否则按 busyEnter 语义：会话繁忙时 Enter=设置的 busyEnter（queue 排队/steer 插话），
    /// 空闲时一律 queue。</summary>
    private async Task SendAsync(string? forceMode = null)
    {
        var text = (InputBox.Text ?? "").Trim();
        var attachments = _pendingAttachments.ToList();
        if (text.Length == 0 && attachments.Count == 0)
        {
            return;
        }
        if (_rpc is null)
        {
            // 内核还在后台引导：消息留在输入框里，别让用户以为发出去了（只提示一次，
            // 不弹错误级对话框——内核就绪后正常发送即可）。
            if (!_kernelWaitHintShown)
            {
                _kernelWaitHintShown = true;
                AppendSystemMessage(L("内核还在加载中，请稍候再发。"));
            }
            return;
        }
        string mode = "queue";
        if (forceMode is "queue" or "steer")
        {
            mode = forceMode;
        }
        else if (IsSessionBusy())
        {
            mode = NsString("ui-conversation", "busyEnter", "queue");
        }
        if (_activeSessionId is null)
        {
            // 未选会话：自动新建并立即打开（消息落在新会话里，聊天区可见）。
            // 工作区/Agent 预设取输入区上方选择器的待用值（新会话待用席位）。
            try
            {
                var newSid = await CreateSessionCoreAsync();
                Volatile.Write(ref _activeSessionId, newSid);
                // 输入区状态（plan/permissions 投影 + 权限下拉）跟随新会话
                await RefreshSessionStateFromListAsync(newSid);
                await RefreshSessionsAsync();
                var vm = _sessions.FirstOrDefault(s => s.SessionId == newSid);
                if (vm is not null)
                {
                    Volatile.Write(ref _journalCursor, 0);
                    PostUi(() =>
                    {
                        _messages.Clear();
                        RebuildNavMenu();
                        RestoreActiveSessionSelection();
                    });
                    await FollowSessionAsync(newSid);
                }
            }
            catch (Exception ex)
            {
                // 不筛类型：任何建会话失败都弹窗返回，绝不让异常沿 OnSendClick(async void) 上抛
                _ = ShowErrorAsync(ex.Message);
                return;
            }
        }

        InputBox.Text = "";
        var sid = _activeSessionId!;
        // 降级标记提到 try 外：发送成功后的本地回显也要读它（决定是否登记 journal 图片认领）
        var imagesAsFiles = false;
        try
        {
            // 模式选择：随消息把当前会话模型+推理档落到所选值（用 selectModel；失败不阻塞发送）。
            // 必须用 id/provider——_selectedModelName 是显示名，可能 ≠ id，直接当 id 传会被内核拒绝。
            try
            {
                var provider = _selectedModelProvider ?? _catalogDefault?.Provider ?? "deepseek-official";
                var currentModel = _selectedModelId;
                if (string.IsNullOrEmpty(currentModel) && _catalogDefault is { } cd)
                {
                    currentModel = cd.Model;
                }
                if (string.IsNullOrEmpty(currentModel))
                {
                    currentModel = "deepseek-flash";
                }
                // 档位按目标模型的能力给：不支持的档位会让整个 selectModel 被内核拒掉
                // （见 EffortForModelId），曾经这里无条件带 _reasoningEffort，切到不带
                // reasoning 的模型后每次发送都静默失败，模型切换看着"失效"。
                var effort = EffortForModelId(currentModel);
                var payload = new Dictionary<string, object>
                {
                    ["sessionId"] = sid,
                    ["provider"] = provider,
                    ["model"] = currentModel,
                };
                if (effort is not null)
                {
                    payload["reasoningEffort"] = effort;
                }
                // typert wire：参数必须包在 request 字段里（descriptor 的 wire 名）。
                // 扁平传会被 gateway 的 assertExactArguments 拒掉——missing "request"，
                // 整次切换失败（失败只进 toast，不抛异常，曾长期无人联想到调用形状）。
                await _rpc.CallOkAsync("session/selectModel", new { request = payload });
                if (effort is not null && effort != _reasoningEffort)
                {
                    // 内核按模型默认档收下了：UI 的档位要跟上，否则触发器显示的不是实际生效值
                    PostUi(() => SetEffortUi(effort));
                }
            }
            catch (DshRpcException ex)
            {
                // 模型没落成本次请求仍会按会话现有选择发（发送不阻塞），但用户必须知道切失败了
                ShellToast.Show(L("模型切换失败"), LF("未能应用模型 {0}：{1}", _selectedModelName.Length > 0 ? _selectedModelName : _selectedModelId, ex.Message));
            }

            // content 数组：文本 + 附件（图片 base64 内联 / 文件 receiptId）。
            // imagesAsFiles：内核拒收图片字节时的降级重发——图片改走原始字节上传（verbatim，
            // 不过图片校验），保证消息不丢。
            async Task<List<object>> BuildContentAsync()
            {
                var parts = new List<object>();
                if (text.Length > 0)
                {
                    parts.Add(new { type = "text", text });
                }
                foreach (var att in attachments)
                {
                    if (att.IsImage && !imagesAsFiles)
                    {
                        parts.Add(new { type = "image", mediaType = att.MediaType, data = att.DataBase64, name = att.Name });
                    }
                    else
                    {
                        var receipt = att.IsImage
                            ? await UploadFileBytesAsync(sid, Convert.FromBase64String(att.DataBase64), att.Name)
                            : await UploadFileAsync(sid, att);
                        if (receipt is null)
                        {
                            // 收不到 receiptId 就没有内容块可发：整条消息失败，绝不能少个附件照发
                            throw new DshRpcException("attachment-upload-failed",
                                LF("附件 {0} 上传失败，消息未发送。", att.Name));
                        }
                        parts.Add(new { type = "file", receiptId = receipt });
                    }
                }
                return parts;
            }

            var content = await BuildContentAsync();
            try
            {
                await _rpc.CallOkAsync("session/prompt", new
                {
                    request = new
                    {
                        requestId = $"c2-{Guid.NewGuid():N}",
                        sessionId = sid,
                        mode,
                        content,
                    },
                });
            }
            catch (DshRpcException ex) when (!imagesAsFiles && attachments.Any(a => a.IsImage) &&
                                             ex.Code.EndsWith("attachment-invalid", StringComparison.Ordinal))
            {
                // 内核 sharp 拒收图片字节（壳认不出的格式/文件损坏）。附件入口已按魔数
                // 规范化，能走到这里的是字节本身坏掉的情况：降级为文件附件重发一次。
                imagesAsFiles = true;
                content = await BuildContentAsync();
                await _rpc.CallOkAsync("session/prompt", new
                {
                    request = new
                    {
                        requestId = $"c2-{Guid.NewGuid():N}",
                        sessionId = sid,
                        mode,
                        content,
                    },
                });
                var names = attachments.Where(a => a.IsImage).Select(a => a.Name).ToList();
                PostUi(() => AppendSystemMessage(LF("有 {0} 张图片无法按图片识别，已改为文件发送：{1}", names.Count, string.Join("、", names))));
            }

            // 发送成功后清空附件
            PostUi(() =>
            {
                _pendingAttachments.Clear();
                AttachmentList.Children.Clear();
                AttachmentStrip.Visibility = Visibility.Collapsed;
            });
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("发送失败：{0}", ex.Message));
            return;
        }
        catch (Exception)
        {
            // 网络/连接层异常：不沿 async void 上抛（0xc000027b 教训）
            return;
        }
        await FollowSessionAsync(sid);
        // 图片本地回显先解码（BitmapImage.SetSourceAsync 要在 UI 线程跑），再随文本一起上屏
        var echoedImages = new List<(string Name, Microsoft.UI.Xaml.Media.ImageSource Image)>();
        foreach (var att in attachments)
        {
            if (!att.IsImage)
            {
                continue;
            }
            try
            {
                echoedImages.Add((att.Name, await MakeBitmapAsync(Convert.FromBase64String(att.DataBase64))));
            }
            catch (Exception)
            {
                // 图裂了不挡发送：文本与思考占位照常
            }
        }
        // prompt 已受理：本地立即回显这条提问（文本 + 图片）并挂「少女祈祷中」占位（按钮随之转停止），
        // 不等 journal 的 user 记录——它要等内核落盘，正是这段空窗让界面看着没反应。
        PostUi(() =>
        {
            if (text.Length > 0)
            {
                var echo = new ChatBubble
                {
                    Role = "user",
                    Text = text,
                    Time = NowMs(),
                    // 先按即将发出的模型标注；内核 request/header 到达后按实际生效值校正
                    Model = _selectedModelId.Length > 0 ? _selectedModelId : _catalogDefault?.Model ?? "",
                };
                // 登记等 header 认领：journal 的 user 记录可能先到并认领同一条气泡（Seq>0），
                // 指针指的是同一个对象，两种先后顺序都不会漏标。
                _pendingUserBubble = echo;
                // 撤回编辑候选：只有真正起了一轮的那次发送才登记。会话忙时的发送是排队/插话，
                // 那一轮还没开跑，按钮得留在正在跑的提问上，否则点下去停错轮、撤错消息。
                if (!IsSessionBusy())
                {
                    _withdrawCandidate = echo;
                }
                AppendBubble(echo);
            }
            foreach (var (name, image) in echoedImages)
            {
                // 登记待认领：journal 的 user/message 到达时按名认领，本地回显与历史回放不重复贴图。
                // 降级为文件发送时不登记：journal 回放的是文件块（本就不贴图），登记反而会
                // 误认领后续同名真图片。
                if (!imagesAsFiles)
                {
                    _localImageEchoes[name] = _localImageEchoes.TryGetValue(name, out var left) ? left + 1 : 1;
                }
                _messages.Insert(UserInsertIndex(), new ChatBubble { Role = "user-image", Text = name, Image = image });
            }
            if (echoedImages.Count > 0)
            {
                if (_messages.Count > 400)
                {
                    _messages.RemoveAt(0);
                }
                ScrollTranscriptToBottom(force: true);
            }
            SetPendingThinking(true);
        });
        // 兜底：follow 流可能未及时送达（内核 mux 心跳窗口），发送后补拉一页增量。
        // throughSeq 过大会 "past cursor"：从 64 对半收缩试探（与 LoadSessionHistory 同法）。
        try
        {
            var through = 64;
            while (through >= 0)
            {
                JsonElement? page = null;
                try
                {
                    page = await _rpc.CallOkAsync("session/page", new
                    {
                        request = new { address = new { kind = "session", sessionId = sid }, throughSeq = through, maxMessages = 64 },
                    });
                }
                catch (DshRpcException ex) when (ex.Message.Contains("past cursor"))
                {
                    through = through >= 8 ? through / 2 : 0;
                    continue;
                }
                if (page is { } p)
                {
                    RenderJournal(p);
                    break;
                }
            }
        }
        catch (Exception) { }
        _ = RefreshSessionsAsync();

        // 逐字增量在流上时不再轮询整页（轮询是 4s 一批，正是"整块刷新"的来源）。
        // 只有内核不认 assistantStream 参数、退回持久事件模式时才靠轮询出字。
        if (!LiveStreamRequested)
        {
            _ = PollReplyAsync(sid);
        }
    }

    /// <summary>轮询会话 journal 增量直到 turn 结束（RenderJournal 内部按游标去重）。</summary>
    private async Task PollReplyAsync(string sid)
    {
        try
        {
            for (var i = 0; i < 22 && Volatile.Read(ref _activeSessionId) == sid; i++)
            {
                await Task.Delay(TimeSpan.FromSeconds(4));
                var through = 64;
                while (through >= 0)
                {
                    JsonElement? page = null;
                    try
                    {
                        page = await _rpc!.CallOkAsync("session/page", new
                        {
                            request = new { address = new { kind = "session", sessionId = sid }, throughSeq = through, maxMessages = 64 },
                        });
                    }
                    catch (DshRpcException ex) when (ex.Message.Contains("past cursor"))
                    {
                        through = through >= 8 ? through / 2 : 0;
                        continue;
                    }
                    if (page is { } p)
                    {
                        RenderJournal(p);
                        // turn/end 已在页内且无新增 → 停止轮询（由游标判断：游标不再前进两次即止）
                        break;
                    }
                }
            }
        }
        catch (Exception) { }
    }

    // ---------------- 审批 ----------------

    // 以下交互状态只在 UI 线程访问。已完成/取消的 ID 也保留，防止重连重投复活卡片。
    private readonly HashSet<string> _seenInteractiveEventIds = new(StringComparer.Ordinal);
    private string? _submittingApprovalEventId;
    private string? _submittingQuestionEventId;

    private void RunEventUi(Action action)
    {
        if (DispatcherQueue.HasThreadAccess)
        {
            action();
        }
        else
        {
            PostUi(() => action());
        }
    }

    /// <summary>waterfall 顶层 eventId 标识请求，request 包含工具与原因。</summary>
    private Task OnApprovalRequestAsync(JsonElement frame)
    {
        var eventId = Str(frame, "eventId");
        if (eventId.Length == 0 || !frame.TryGetProperty("request", out var request) ||
            request.ValueKind != JsonValueKind.Object)
        {
            return Task.CompletedTask;
        }
        var toolName = Str(request, "toolName");
        var reason = Str(request, "reason");
        RunEventUi(() =>
        {
            if (!_seenInteractiveEventIds.Add(eventId))
            {
                return;
            }
            var tool = toolName.Length > 0 ? toolName : L("(工具)");
            _approvalQueue.Enqueue((eventId, tool, reason.Length > 0 ? reason : null));
            // 窗口不在前台时审批卡用户看不到：补一条系统通知（聚焦时不打扰）
            if (ShouldNotify())
            {
                ShellToast.Show(L("工具审批请求"), reason.Length > 0 ? tool + " — " + reason : tool);
            }
            ShowNextApproval();
        });
        return Task.CompletedTask;
    }

    /// <summary>系统通知统一准入：设置里开着 + 窗口确实不在前台（聚焦时不打扰）。</summary>
    private bool ShouldNotify() =>
        _shellOptions.ShowNotifications && _hwnd != IntPtr.Zero && GetForegroundWindow() != _hwnd;

    /// <summary>任务完成通知。只由跟随流的实时路径调用——历史回放（session/page 翻页）
    /// 不经这里，翻旧会话不会补出一堆「任务完成」。
    /// 新鲜度窗 10 分钟：跟随流快照补拉会把断线期间的事件当增量整卷重放（历史加载失败时
    /// 游标为 0，整本 journal 都会被当增量），旧 turn 不补通知。</summary>
    private void MaybeNotifyTurnEnded(JsonElement ev)
    {
        if (!ShouldNotify())
        {
            return;
        }
        var time = ev.TryGetProperty("time", out var t) && t.ValueKind == JsonValueKind.Number
            ? (long)t.GetDouble()
            : 0L;
        if (time > 0 && DateTimeOffset.UtcNow.ToUnixTimeMilliseconds() - time > 10 * 60 * 1000)
        {
            return;
        }
        var title = _sessions.FirstOrDefault(s => s.SessionId == Volatile.Read(ref _activeSessionId))?.Title;
        ShellToast.Show(L("任务完成"), title is { Length: > 0 } sessionTitle ? sessionTitle : L("本轮对话已完成"));
    }

    /// <summary>只在当前审批结束后亮出队首（UI 线程）。</summary>
    private void ShowNextApproval()
    {
        if (_pendingApprovalEventId is not null)
        {
            return;
        }
        ApprovalHost.IsEnabled = true;
        if (_approvalQueue.Count == 0)
        {
            ApprovalHost.Visibility = Visibility.Collapsed;
            return;
        }
        var approval = _approvalQueue.Dequeue();
        _pendingApprovalEventId = approval.EventId;
        ApprovalReason.Text = LF("工具 {0} 请求越权执行", approval.ToolName);
        // reason 是内核/模型生成的英文自由文本，壳里翻不动：原样作详情行，界面语言保持中文
        if (approval.Reason is { Length: > 0 } reason)
        {
            ApprovalDetail.Text = reason;
            ApprovalDetail.Visibility = Visibility.Visible;
        }
        else
        {
            ApprovalDetail.Visibility = Visibility.Collapsed;
        }
        ApprovalHost.Visibility = Visibility.Visible;
    }

    /// <summary>取消只清对应 ID；记下墓碑以处理接收线程中取消先于请求回调的情况。</summary>
    private void OnEventCancelled(string eventId)
    {
        RunEventUi(() =>
        {
            _seenInteractiveEventIds.Add(eventId);
            var approvals = _approvalQueue.Count;
            for (var i = 0; i < approvals; i++)
            {
                var item = _approvalQueue.Dequeue();
                if (item.EventId != eventId) _approvalQueue.Enqueue(item);
            }
            var questions = _questionQueue.Count;
            for (var i = 0; i < questions; i++)
            {
                var item = _questionQueue.Dequeue();
                if (Str(item, "eventId") != eventId) _questionQueue.Enqueue(item);
            }
            if (_pendingApprovalEventId == eventId)
            {
                _pendingApprovalEventId = null;
                _submittingApprovalEventId = null;
                ShowNextApproval();
            }
            if (Str(_activeQuestion, "eventId") == eventId)
            {
                _activeQuestion = default;
                _submittingQuestionEventId = null;
                ShowNextQuestion();
            }
        });
    }

    private async void OnApprovalAllow(object sender, RoutedEventArgs e) => await ResolveApprovalAsync("allowed-once");
    private async void OnApprovalReject(object sender, RoutedEventArgs e) => await ResolveApprovalAsync("rejected");

    private async Task ResolveApprovalAsync(string outcome)
    {
        var eventId = _pendingApprovalEventId;
        if (eventId is null || _rpc is null || _submittingApprovalEventId is not null)
        {
            return;
        }
        _submittingApprovalEventId = eventId;
        ApprovalHost.IsEnabled = false;
        try
        {
            await _rpc.ResolveEventAsync(eventId, outcome);
            RunEventUi(() =>
            {
                if (_pendingApprovalEventId != eventId) return;
                _pendingApprovalEventId = null;
                _submittingApprovalEventId = null;
                ShowNextApproval();
            });
        }
        catch (Exception ex)
        {
            RunEventUi(() =>
            {
                // 取消可能已亮出下一条；旧 HTTP 完成不得改动下一条的提交锁或 UI。
                if (_pendingApprovalEventId != eventId) return;
                _submittingApprovalEventId = null;
                ApprovalHost.IsEnabled = true;
                AppendSystemMessage(LF("审批回传失败，请重试：{0}", ex.Message));
            });
        }
    }

    // ---------------- 用户提问（user-questions/request，$events waterfall） ----------------
    //
    // 协议（dsh-api-gateway startRemoteEvent + dsh-api-remotes forwardWaterfall 实测）：
    //   内核推  {type:"waterfall", event:"user-questions/request", eventId, agentId, request}
    //   壳回传  POST /api/$events/result {clientId, eventId, outcome:{kind:"result", value}}
    //   value   {answers:[{id, selected:[标签…], custom?}]}   —— 与 dsh-tool-ask-user 的
    //           output schema 完全一致（selected 是选项 label 数组，custom 是自由文本）。
    //   若回传 {kind:"next"}，内核会走 waterfall 的 next()（即"本客户端不回答"）。
    // request.questions[i] = {id, question, header?, options?:[{label,description?}],
    //                         multiSelect?, intent?, detail?}
    // 计划评审（exit_plan_mode）走的就是这条通道：questions[0].id == "plan-review"，
    // intent.kind == "plan-review"，detail 是计划 markdown。

    /// <summary>当前亮着的提问（同一时刻只亮一条，与审批卡片同策略：入队顺序处理）。</summary>
    private readonly Queue<JsonElement> _questionQueue = new();
    private JsonElement _activeQuestion = default;

    /// <summary>waterfall 帧入口（接收线程）。只处理 user-questions/request。</summary>
    private Task OnUserQuestionAsync(JsonElement frame)
    {
        var eventId = Str(frame, "eventId");
        if (Str(frame, "event") != "user-questions/request" || eventId.Length == 0 ||
            !frame.TryGetProperty("request", out var request) || request.ValueKind != JsonValueKind.Object ||
            !request.TryGetProperty("questions", out var questions) || questions.ValueKind != JsonValueKind.Array ||
            questions.GetArrayLength() == 0)
        {
            return Task.CompletedTask;
        }
        var ownedFrame = frame.Clone();
        RunEventUi(() =>
        {
            if (!_seenInteractiveEventIds.Add(eventId)) return;
            _questionQueue.Enqueue(ownedFrame);
            // 与审批同策：提问（含 exit_plan_mode 计划评审）同样卡住任务等回答，
            // 窗口不在前台时用户看不到卡片，补一条系统通知。
            if (ShouldNotify() && questions.GetArrayLength() > 0)
            {
                var first = questions[0];
                var header = first.TryGetProperty("header", out var h) && h.ValueKind == JsonValueKind.String
                    ? h.GetString()
                    : null;
                var text = first.TryGetProperty("question", out var q) && q.ValueKind == JsonValueKind.String
                    ? q.GetString()
                    : null;
                var body = text ?? string.Empty;
                if (body.Length > 120) body = body[..120] + "…";
                ShellToast.Show(header is { Length: > 0 } ? header : L("需要你的输入"), body);
            }
            ShowNextQuestion();
        });
        return Task.CompletedTask;
    }

    /// <summary>亮出队首提问（无待答则收起卡片）。</summary>
    private void ShowNextQuestion()
    {
        if (_activeQuestion.ValueKind == JsonValueKind.Object)
        {
            return;
        }
        try
        {
            SetQuestionInputEnabled(true);
            _questionSelections.Clear();
            if (_questionQueue.Count == 0)
            {
                QuestionHost.Children.Clear();
                QuestionPanel.Visibility = Visibility.Collapsed;
                return;
            }
            var next = _questionQueue.Dequeue();
            _activeQuestion = next;
            BuildQuestionCard(next);
            QuestionPanel.Visibility = Visibility.Visible;
        }
        catch (Exception)
        {
            // 卡片构造失败不上抛（0xc000027b 教训）
        }
    }

    /// <summary>提问卡片：表头 + 每个问题一组选项（单选/多选）+ 自由文本 + 提交/跳过。</summary>
    private void BuildQuestionCard(JsonElement frame)
    {
        QuestionHost.Children.Clear();
        var request = frame.TryGetProperty("request", out var rq) && rq.ValueKind == JsonValueKind.Object ? rq : default;
        var questions = request.ValueKind == JsonValueKind.Object && request.TryGetProperty("questions", out var qs) &&
                        qs.ValueKind == JsonValueKind.Array ? qs : default;
        if (questions.ValueKind != JsonValueKind.Array || questions.GetArrayLength() == 0)
        {
            return;
        }

        // 表头：plan-review 有专属文案，其余用问题自带的 header 或统一标题
        var first = questions[0];
        var isPlanReview = Str(first, "id") == "plan-review";
        var header = isPlanReview ? "计划评审" : (Str(first, "header") is { Length: > 0 } h ? h : "内核提问");

        var head = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp8 };
        head.Children.Add(new FontIcon { Glyph = isPlanReview ? "\uE9D5" : "\uE897", FontSize = GlyphBody });
        head.Children.Add(new TextBlock { Text = header, Style = AppStyle("BodyStrongTextStyle") });
        var agentId = Str(frame, "agentId");
        if (agentId.Length > 0)
        {
            head.Children.Add(new TextBlock
            {
                Text = agentId,
                Style = AppStyle("CodeTextStyle"),
                Opacity = 0.6,
                VerticalAlignment = VerticalAlignment.Center,
                IsTextSelectionEnabled = true,
            });
        }
        QuestionHost.Children.Add(head);

        var selections = new List<(string Id, bool Multi, List<RadioButton> Radios, List<CheckBox> Checks, TextBox Custom)>();
        foreach (var q in questions.EnumerateArray())
        {
            var qid = Str(q, "id");
            var text = Str(q, "question");
            var multi = q.TryGetProperty("multiSelect", out var ms) && ms.ValueKind == JsonValueKind.True;

            QuestionHost.Children.Add(new TextBlock { Text = text, TextWrapping = TextWrapping.Wrap });

            // 计划评审的 detail 是完整计划 markdown：原样呈现（官方客户端同样展示它）
            var detail = Str(q, "detail");
            if (detail.Length > 0)
            {
                QuestionHost.Children.Add(new Border
                {
                    Background = ThemeBrush("CardSecondaryBrush"),
                    CornerRadius = RadMedium,
                    Padding = TokenThickness("CardPaddingCompact", new Thickness(12, 8, 12, 8)),
                    MaxHeight = 260,
                    Child = new ScrollViewer
                    {
                        VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
                        Content = new ContentControl { Content = detail, HorizontalContentAlignment = HorizontalAlignment.Stretch },
                    },
                });
            }

            var options = q.TryGetProperty("options", out var opts) && opts.ValueKind == JsonValueKind.Array ? opts : default;
            var radios = new List<RadioButton>();
            var checks = new List<CheckBox>();
            var group = $"q-{qid}-{Guid.NewGuid():N}";
            if (options.ValueKind == JsonValueKind.Array)
            {
                foreach (var opt in options.EnumerateArray())
                {
                    var label = Str(opt, "label");
                    var desc = Str(opt, "description");
                    var content = desc.Length > 0 ? $"{label} — {desc}" : label;
                    if (multi)
                    {
                        var cb = new CheckBox { Content = content, Tag = label };
                        Aut(cb, $"QuestionOption_{qid}_{label}", LF("选项：{0}", label));
                        checks.Add(cb);
                        QuestionHost.Children.Add(cb);
                    }
                    else
                    {
                        var rb = new RadioButton { Content = content, GroupName = group, Tag = label };
                        Aut(rb, $"QuestionOption_{qid}_{label}", LF("选项：{0}", label));
                        radios.Add(rb);
                        QuestionHost.Children.Add(rb);
                    }
                }
            }

            // 自由文本：内核 answer.custom 字段，任何问题都可带（选项之外补充说明）
            var custom = new TextBox
            {
                PlaceholderText = "补充说明（可选，随答案回传为 custom）",
                TextWrapping = TextWrapping.Wrap,
                AcceptsReturn = false,
            };
            Aut(custom, $"QuestionCustom_{qid}", L("补充说明"));
            QuestionHost.Children.Add(custom);

            selections.Add((qid, multi, radios, checks, custom));
        }

        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = Sp8,
            HorizontalAlignment = HorizontalAlignment.Right,
        };
        var skip = new Button { Content = "跳过（不回答）" };
        Aut(skip, "QuestionSkipButton", L("跳过（不回答）"));
        skip.Click += OnQuestionSkipClick;
        buttons.Children.Add(skip);
        var submit = Aut(new Button { Content = "提交", Style = AppStyle("AccentButtonStyle") }, "QuestionSubmitButton", L("提交答案"));
        Aut(submit, "QuestionSubmitButton", L("提交答案"));
        submit.Click += OnQuestionSubmitClick;
        buttons.Children.Add(submit);
        QuestionHost.Children.Add(buttons);

        _questionSelections = selections;
    }

    /// <summary>卡片当前的作答控件（问题 id → 控件组）。</summary>
    private List<(string Id, bool Multi, List<RadioButton> Radios, List<CheckBox> Checks, TextBox Custom)> _questionSelections = new();

    private void SetQuestionInputEnabled(bool enabled)
    {
        foreach (var child in QuestionHost.Children)
        {
            if (child is Control control) control.IsEnabled = enabled;
            if (child is StackPanel row)
            {
                foreach (var button in row.Children.OfType<Button>()) button.IsEnabled = enabled;
            }
        }
    }

    private async void OnQuestionSubmitClick(object sender, RoutedEventArgs e)
    {
        await ResolveQuestionAsync(skip: false);
    }

    private async void OnQuestionSkipClick(object sender, RoutedEventArgs e)
    {
        await ResolveQuestionAsync(skip: true);
    }

    /// <summary>提交/跳过共享提交锁；失败保留输入，成功或取消才推进队列。</summary>
    private async Task ResolveQuestionAsync(bool skip)
    {
        var eventId = Str(_activeQuestion, "eventId");
        if (_rpc is null || eventId.Length == 0 || _submittingQuestionEventId is not null)
        {
            return;
        }
        _submittingQuestionEventId = eventId;
        SetQuestionInputEnabled(false);
        try
        {
            if (skip)
            {
                await _rpc.SkipWaterfallAsync(eventId);
            }
            else
            {
                var answers = new List<object>();
                foreach (var (id, _, radios, checks, custom) in _questionSelections)
                {
                    var selected = radios.Where(r => r.IsChecked == true)
                        .Select(r => r.Tag as string ?? "")
                        .Concat(checks.Where(c => c.IsChecked == true).Select(c => c.Tag as string ?? ""))
                        .Where(s => s.Length > 0)
                        .ToList();
                    var answer = new Dictionary<string, object> { ["id"] = id, ["selected"] = selected };
                    var text = (custom.Text ?? "").Trim();
                    if (text.Length > 0) answer["custom"] = text;
                    answers.Add(answer);
                }
                await _rpc.ResolveWaterfallAsync(eventId, new { answers });
            }
            RunEventUi(() =>
            {
                if (Str(_activeQuestion, "eventId") != eventId) return;
                _activeQuestion = default;
                _submittingQuestionEventId = null;
                AppendSystemMessage(skip ? L("已跳过该提问（内核侧按未作答继续）。") : L("已回传提问答复。"));
                ShowNextQuestion();
            });
        }
        catch (Exception ex)
        {
            RunEventUi(() =>
            {
                if (Str(_activeQuestion, "eventId") != eventId) return;
                _submittingQuestionEventId = null;
                SetQuestionInputEnabled(true);
                AppendSystemMessage(LF("提问回传失败，请重试：{0}", ex.Message));
            });
        }
    }

    // ---------------- 交付物（deliverables/presented） ----------------

    /// <summary>
    /// 在应用中打开 / 定位交付物：回内核宿主路由
    /// POST /api/present.open?sessionId=&amp;seq=&amp;index=&amp;action=open|reveal
    /// （由 dsh-client-ui-deliverables 的 node 半边注册，Cookie 鉴权）。
    /// action=open → 默认应用打开；reveal → 文件管理器定位。内核按 (sessionId, seq, index)
    /// 回查会话日志定位文件，因此三个坐标必须原样传。
    /// </summary>
    private async Task OpenPresentedFileAsync(PresentedFileVm? file, string action)
    {
        if (_rpc is null || file is null)
        {
            return;
        }
        try
        {
            var (status, body) = await _rpc.PostRawAsync(
                $"/api/present.open?sessionId={Uri.EscapeDataString(file.SessionId)}&seq={file.Seq}&index={file.Index}&action={action}");
            if (status is 204 or 200)
            {
                return; // 204 = 已交给系统打开（内核不返回正文）
            }
            AppendSystemMessage(status switch
            {
                409 => L("无法打开：宿主桌面不可用（内核 workspaceDesktop().available=false）。"),
                404 => L("无法打开：内核在会话日志里找不到该交付物（文件可能已被移动或删除）。"),
                422 => L("无法打开：该文件没有经过校验的宿主路径（可能在工作区沙箱之外）。"),
                _ => LF("无法打开交付物（HTTP {0}）：{1}", status, body),
            });
        }
        catch (Exception ex)
        {
            AppendSystemMessage(LF("无法打开交付物：{0}", ex.Message));
        }
    }

    private async void OnPresentedFileOpenClick(object sender, RoutedEventArgs e)
    {
        try
        {
            await OpenPresentedFileAsync(PresentedFileOf(sender), "open");
        }
        catch (Exception) { }
    }

    private async void OnPresentedFileRevealClick(object sender, RoutedEventArgs e)
    {
        try
        {
            await OpenPresentedFileAsync(PresentedFileOf(sender), "reveal");
        }
        catch (Exception) { }
    }

    private static PresentedFileVm? PresentedFileOf(object sender)
        => sender is FrameworkElement { DataContext: PresentedFileVm file } ? file : null;

    // ---------------- 在应用中打开（dsh-host-open-in-app 的网页路由） ----------------
    //
    // 内核把该能力做成宿主 HTTP 路由，不是 RPC：
    //   GET  /open-in-app/apps            → {apps:[<id>…]}（已探测到可用的应用 id）
    //   GET  /open-in-app/icon/<id>       → PNG（本壳不用图标，走字体图标）
    //   POST /open-in-app/open {app,path} → 200 {ok:true}；path 必须是存在的绝对目录
    // 两条路由都在连接鉴权围栏内（Cookie），因此复用同一个 HttpClient。

    /// <summary>拉取可用应用清单并填菜单（每次展开都重读：内核侧有懒解析 + 失败重试）。</summary>
    private async Task LoadOpenInAppMenuAsync()
    {
        OpenInAppSubMenu.Items.Clear();
        if (_rpc is null)
        {
            return;
        }
        try
        {
            var (status, body) = await _rpc.GetRawAsync("/open-in-app/apps");
            if (status != 200)
            {
                OpenInAppSubMenu.Items.Add(new MenuFlyoutItem { Text = LF("应用清单不可用（HTTP {0}）", status), IsEnabled = false });
                return;
            }
            using var doc = JsonDocument.Parse(body);
            var apps = doc.RootElement.TryGetProperty("apps", out var a) && a.ValueKind == JsonValueKind.Array ? a : default;
            if (apps.ValueKind != JsonValueKind.Array || apps.GetArrayLength() == 0)
            {
                OpenInAppSubMenu.Items.Add(new MenuFlyoutItem { Text = L("内核未探测到可用应用"), IsEnabled = false });
                return;
            }
            foreach (var app in apps.EnumerateArray())
            {
                var id = app.GetString();
                if (string.IsNullOrEmpty(id))
                {
                    continue;
                }
                var item = new MenuFlyoutItem { Text = OpenInAppLabel(id), Tag = id };
                Aut(item, $"OpenInApp_{id}", OpenInAppLabel(id));
                item.Click += OnOpenInAppClick;
                OpenInAppSubMenu.Items.Add(item);
            }
        }
        catch (Exception ex)
        {
            OpenInAppSubMenu.Items.Add(new MenuFlyoutItem { Text = LF("应用清单读取失败：{0}", ex.Message), IsEnabled = false });
        }
    }

    /// <summary>内核 catalog 的应用 id → 中文菜单名（未收录的 id 原样显示，不猜）。</summary>
    private string OpenInAppLabel(string id) => id switch
    {
        "explorer" => L("文件资源管理器"),
        "finder" => "Finder",
        "terminal" => L("终端"),
        "vscode" => "Visual Studio Code",
        "cursor" => "Cursor",
        "windsurf" => "Windsurf",
        "idea" => "IntelliJ IDEA",
        "pycharm" => "PyCharm",
        "webstorm" => "WebStorm",
        "goland" => "GoLand",
        "clion" => "CLion",
        "rider" => "Rider",
        "rustrover" => "RustRover",
        "sublime" => "Sublime Text",
        "git" => "Git",
        "fork" => "Fork",
        "sourcetree" => "Sourcetree",
        "gh" => "GitHub Desktop",
        "gitkraken" => "GitKraken",
        "windowsterminal" => L("Windows 终端"),
        "powershell" => "PowerShell",
        "cmd" => L("命令提示符"),
        _ => id,
    };

    /// <summary>用指定应用打开当前会话的工作目录（path 取会话 cwd——内核要求绝对且存在的目录）。</summary>
    private async void OnOpenInAppClick(object sender, RoutedEventArgs e)
    {
        try
        {
            if (_rpc is null || sender is not FrameworkElement { Tag: string app })
            {
                return;
            }
            var sid = Volatile.Read(ref _activeSessionId);
            var cwd = _sessions.FirstOrDefault(s => s.SessionId == sid)?.Cwd;
            if (string.IsNullOrEmpty(cwd))
            {
                AppendSystemMessage(L("无法在应用中打开：当前会话没有已知的工作目录（cwd）。"));
                return;
            }
            var (status, body) = await _rpc.PostRawAsync("/open-in-app/open", new { app, path = cwd });
            if (status == 200)
            {
                return;
            }
            AppendSystemMessage(LF("在应用中打开失败（HTTP {0}）：{1}", status, body));
        }
        catch (Exception ex)
        {
            AppendSystemMessage(LF("在应用中打开失败：{0}", ex.Message));
        }
    }

    // ---------------- 设置（分区导航 + 卡片列表） ----------------

    /// <summary>一个待写回的设置变更（ns + 字段路径 + **已求值**的取值）。
    ///
    /// Value 不再是延迟闭包，而是登记那一刻就求好的值。实测根因：切换「外观」会走
    /// ApplyShellTheme → RenderSectionAsync 重建整个分区，旧实现里 `() => box.SelectedItem.Tag`
    /// 这类闭包指向的控件已被丢弃，保存时求值得到 null → 该字段被当作「无值」静默跳过：
    /// 用户看到「已保存。」，但权限这一项没写进内核（其余同分区字段正常）。
    /// 同一改动还让「重建后的控件显示待写回值」成为可能（见 NsString/NsNumber/NsBool）。</summary>
    private sealed class PendingEdit
    {
        public string Ns = "";
        public string[] Path = Array.Empty<string>();
        public object? Value;
    }

    private List<SettingsSectionVm> _settingsSections = new();
    private string _settingsActiveSection = "general";
    private readonly List<PendingEdit> _settingsEdits = new();
    // 草稿属于窗口而不是控件；切页/重建只更换视图，不丢弃未提交内容。
    // 密钥只暂存在内存，不落盘、不读取已保存的密钥，也不写入诊断信息。
    private sealed class PendingCredentialWrite
    {
        public string Value = "";
    }

    private readonly Dictionary<string, PendingCredentialWrite> _pendingCredentialWrites = new(StringComparer.Ordinal);
    private readonly Dictionary<string, WeakReference<PasswordBox>> _settingsCredentialBoxes = new(StringComparer.Ordinal);
    private readonly SemaphoreSlim _settingsCommitGate = new(1, 1);
    private bool _updatingCredentialBox;
    private InfoBar? _settingsSaveInfo;
    private string? _settingsSaveError;
    private Dictionary<string, (JsonElement Value, double Revision)>? _settingsSnapshot;

    /// <summary>设置分区清单（内核 settings.section 注册表：general=0 / models=10 / plugins=15 / agent-presets=20）。
    /// 壳侧在原四区之后追加八个 **壳内建分区**："memory"（记忆：Blade² 默认插件的记忆服务器开关，
    /// 编辑内核 profile patch 文件而非内核设置命名空间）与 "pet"（宠物：桌面宠物的显隐/尺寸、
    /// 宠物库、安装与诊断）、"computer-control"（电脑控制）与
    /// "browser-control"（浏览器控制）（默认插件组合里 computer-use / browser-use 挂载条目的开关）、
    /// "personalization"（个性化：自定义指令 + 背景皮肤）、"usage"（使用统计）、"skills"（技能：
    /// 当前会话可调用的技能清单）与 "about"（关于：版本与更新）：
    /// 八者不进内核 settings/mutate，也不参与「恢复本页默认」；四项内核分区的顺序与内容不变。
    /// 原「自动审批」分区已撤下侧栏入口，内容并入「插件」分区配置 Tab 的默认插件卡上方。</summary>
    private static readonly string[] SectionOrder =
        { "general", "personalization", "models", "plugins", "skills", "memory", "pet", "agent-presets", "computer-control", "browser-control", "usage", "about" };

    /// <summary>壳内建分区 id（不进内核 settings/mutate，也不参与「恢复本页默认」）。</summary>
    private const string UsageSectionId = "usage";
    private const string MemorySectionId = "memory";
    private const string SkillsSectionId = "skills";
    private const string ComputerControlSectionId = "computer-control";
    private const string BrowserControlSectionId = "browser-control";
    private const string PetSectionId = "pet";

    /// <summary>壳内建分区集合：无待写回字段，不挂「设置文件」维护卡（恢复默认对它无意义）。</summary>
    private static readonly HashSet<string> ShellBuiltinSections = new(StringComparer.Ordinal)
    {
        UsageSectionId, MemorySectionId, SkillsSectionId, ComputerControlSectionId, BrowserControlSectionId,
        PetSectionId, AboutSectionId,
        PersonalizationSectionId,
    };

    private static string SectionTitle(string id) => id switch
    {
        "general" => "通用设置",
        "personalization" => "个性化",
        "models" => "模型",
        "plugins" => "插件",
        "skills" => "技能",
        "memory" => "记忆",
        "agent-presets" => "Agent 预设",
        "computer-control" => "电脑控制",
        "browser-control" => "浏览器控制",
        "usage" => "使用统计",
        "pet" => "宠物",
        "about" => "关于",
        _ => id,
    };

    private static string SectionGlyph(string id) => id switch
    {
        "general" => "\uE713",       // 设置齿轮
        "personalization" => "\uE790", // 调色板（个性化）
        "models" => "\uE8F1",        // 芯片
        "plugins" => "\uEA86",       // 插件
        "skills" => "\uE734",        // 星形（技能；与内核技能行的四角星图标同族，字形本应用已在用）
        "memory" => "\uE81C",        // 历史（记忆/时间线）
        "agent-presets" => "\uE9D9", // 机器人
        "computer-control" => "\uE7F4", // 显示器（电脑控制）
        "browser-control" => "\uE774",  // 地球（浏览器控制）
        "usage" => "\uE9D2",         // 图表（原「使用统计」入口的图标）
        "pet" => "\uE76E",           // 笑脸（宠物：MDL2 里最接近「伙伴/ mascot」的字形）
        "about" => "\uE946",         // 信息（关于）
        _ => "\uE713",
    };

    private void ToggleSettingsPage()
    {
        try
        {
            if (SettingsPage.Visibility == Visibility.Visible)
            {
                ShowChatPage();
            }
            else
            {
                _ = ShowSettingsAsync();
            }
        }
        catch (Exception) { } // async void 事件入口兜底（0xc000027b 教训）
    }

    // ---------------- 设置页（页内面板，与聊天页同层切换） ----------------

    /// <summary>返回钮（窗口左上角）的显隐：只有设置页（含其中的使用统计分区）需要它，
    /// 聊天页收起。返回钮位置与 NavigationView 内建返回钮无关（后者在侧栏区域）。</summary>
    private void UpdateShellBackButton()
    {
        var toolPageActive = SettingsPage.Visibility == Visibility.Visible;
        ShellBackButton.Visibility = toolPageActive ? Visibility.Visible : Visibility.Collapsed;
    }

    /// <summary>窗口左上角的返回钮：等价 Esc。设置页里先收二级页，收空了才回聊天页 ——
    /// 返回入口只有这一枚（标题旁不放返回钮），因此「退回上一层」的层级全压在它身上。</summary>
    private void OnShellBackClick(object sender, RoutedEventArgs e)
    {
        try
        {
            // 设置页可见 → 回聊天；状态不一致（聊天页未显示）也兜底回聊天
            if (SettingsPage.Visibility == Visibility.Visible ||
                ChatPage.Visibility != Visibility.Visible)
            {
                if (InSettingsSubPage)
                {
                    CloseSettingsSubPage();
                    return;
                }
                ShowChatPage();
            }
        }
        catch (Exception) { } // 事件入口兜底
    }

    /// <summary>是否已因窄窗自动收起侧栏（只在跨阈值时动作，避免与用户手动展开互抢）。</summary>
    private bool _narrowCompactApplied;

    /// <summary>
    /// 窄窗自动进入压缩态：侧栏展开占 264dip，窗口一窄内容列就不够用（实测 450dip 宽时
    /// 输入卡与空态标题被裁切）。跨过阈值才切换，且只由窗口宽度驱动。
    /// 阈值 = 侧栏 264 + 内容列可用下限 ≈ 720dip。
    /// </summary>
    private void ApplyNarrowPaneRule(double windowWidthDip)
    {
        try
        {
            if (!double.IsFinite(windowWidthDip) || windowWidthDip <= 0)
            {
                return;
            }
            var wantCompact = windowWidthDip < TokenDouble("CompactPaneBreakpoint", 720);
            if (wantCompact == _narrowCompactApplied)
            {
                return;
            }
            _narrowCompactApplied = wantCompact;
            Nav.IsPaneOpen = !wantCompact;
            UpdateShellCompactState();
        }
        catch (Exception) { } // 布局兜底：自动收起失败不影响其余功能
    }

    /// <summary>压缩态点放大镜：临时展开搜索框（不强制展开侧栏），焦点进输入。</summary>
    private void OnSearchIconButtonClick(object sender, RoutedEventArgs e)
    {
        try
        {
            SearchIconButton.Visibility = Visibility.Collapsed;
            SearchBox.Visibility = Visibility.Visible;
            SearchPillSurface.Visibility = Visibility.Visible;
            SearchBox.Focus(FocusState.Programmatic);
        }
        catch (Exception) { }
    }

    /// <summary>
    /// 侧栏形态相关的顶栏/侧栏刷新（返回钮、搜索、工作区段头）。
    /// 侧栏收起按钮已移除，不再维护汉堡显隐。
    /// </summary>
    private void UpdateShellCompactState()
    {
        try
        {
            var compact = !Nav.IsPaneOpen;
            Nav.IsPaneToggleButtonVisible = false;
            // 压缩态：搜索框收成放大镜图标（对齐 Windows 11 设置顶栏）
            SearchBox.Visibility = compact ? Visibility.Collapsed : Visibility.Visible;
            SearchPillSurface.Visibility = compact ? Visibility.Collapsed : Visibility.Visible;
            SearchPillGlyph.Visibility = compact ? Visibility.Collapsed : Visibility.Visible;
            SearchIconButton.Visibility = compact ? Visibility.Visible : Visibility.Collapsed;
            // 工作区段头：聊天页 + 侧栏展开时才出现。
            // 设置页里固定项由 ShowSettingsAsync 折叠；这里不得再强行 Visible（否则设置侧栏会冒出「工作区」）。
            var sectionVisible = !compact && SettingsPage.Visibility != Visibility.Visible;
            var sectionRow = sectionVisible ? Visibility.Visible : Visibility.Collapsed;
            WorkspaceSectionItem.Visibility = sectionRow;
            WorkspaceSectionHeaderText.Visibility = sectionRow;
            WorkspaceViewOptionsButton.Visibility = sectionRow;
            AddWorkspaceButton.Visibility = sectionRow;
        }
        catch (Exception) { } // 顶栏形态刷新失败不致命
    }

    /// <summary>
    /// 设置页固定页头的左边缘对齐内容列（第 3 条）：左边距取内容列在页面坐标系里的实测偏移。
    /// 用实测值而不是固定 36dip，是因为内容列在宽窗里居中限宽（1064）、窄窗里铺满，
    /// 且纵向滚动条出现时会再占掉一条宽度——只有实测偏移能同时覆盖这三种情形。
    /// 实测锚点取 SettingsBodyGrid（滚动区里包住一级/二级两个内容列的 Grid）：它无内外边距、
    /// 铺满滚动区，左偏移与两个内容列相同，且不因某一列被收起而失效（页头要同时服务两级页面）。
    /// 边距落在 SettingsHeaderRow（页头行 = 面包屑 + 标题）上，两者因此与内容列同一条左缘。
    /// </summary>
    private void AlignSettingsHeader()
    {
        try
        {
            if (SettingsPage.Visibility != Visibility.Visible)
            {
                return;
            }
            // SettingsBodyGrid 始终可见（它只是滚动区里包住一级/二级两列的容器），
            // 收起的只是其中一个子列，所以锚点固定取它，不需要回落分支。
            var host = SettingsBodyGrid.TransformToVisual(SettingsPage).TransformPoint(new Windows.Foundation.Point(0, 0));
            var left = Math.Max(0, host.X);
            if (Math.Abs(SettingsHeaderRow.Margin.Left - left) < 0.5)
            {
                return; // 幂等：避免「设边距 → 重排 → 再设边距」自激
            }
            var m = SettingsHeaderRow.Margin;
            SettingsHeaderRow.Margin = new Thickness(left, m.Top, m.Right, m.Bottom);
        }
        catch (Exception) { } // 布局未就绪时保持兜底边距
    }

    /// <summary>进设置页：聊天页让位，主侧栏切成设置分区（固定项折叠）。
    /// 返回钮是顶栏左上角那枚（不占侧栏顶格，因此侧栏的折叠钮保持可见）。</summary>
    private void ShowSettingsPage()
    {
        ChatPage.Visibility = Visibility.Collapsed;
        SettingsPage.Visibility = Visibility.Visible;
        // 宽窗展开分区列表；窄窗维持压缩态（内容列优先，用户点顶栏那枚开关再展开）。
        // 不能无条件展开：窄窗里 264dip 的侧栏会把设置内容挤到不可用。
        Nav.IsPaneOpen = !_narrowCompactApplied;
        UpdateShellCompactState();
        UpdateShellBackButton();
        FadeInPage(SettingsPage, "DurationSlow");
    }

    /// <summary>回聊天页：撤掉分区项（RebuildNavMenu 顺带裁掉尾部项并重建会话树）、
    /// 收起返回钮、把焦点还给输入框。</summary>
    private void ShowChatPage()
    {
        // 离开设置前把两个编辑器的防抖草稿落盘：LostFocus 之外的兜底，
        // 免得点返回后 1 秒内关窗/切会话丢内容。
        FlushMemoryPendingSave();
        FlushInstructionsPendingSave();
        SettingsPage.Visibility = Visibility.Collapsed;
        ChatPage.Visibility = Visibility.Visible;
        ResetSettingsSubPages(); // 二级页状态不跨次进入设置残留（下次进插件页应回到一级导航卡）
        for (var i = 0; i < _fixedMenuItemCount && i < Nav.MenuItems.Count; i++)
        {
            // 固定项从设置态恢复（MenuItems 是 object 集合，取出来是 NavigationViewItem/UIElement）
            if (Nav.MenuItems[i] is UIElement fixedItem)
            {
                fixedItem.Visibility = Visibility.Visible;
            }
        }
        Nav.SelectedItem = null; // 工具页选中态不残留（会话项由 SelectionChanged 驱动）
        UpdateShellCompactState();
        UpdateShellBackButton();
        RebuildNavMenu();
        FadeInPage(ChatPage);
        try
        {
            InputBox.Focus(FocusState.Programmatic);
        }
        catch (Exception) { }
    }

    /// <summary>NavigationView 内建返回钮的 BackRequested：壳已把返回入口移到窗口左上角
    /// （ShellBackButton），内建钮始终 Collapsed；这里保留同一路由，避免任何残余触发路径
    /// （如系统返回键）落到空实现。层级与 ShellBackButton 一致：二级页开着时只收二级页。</summary>
    private void OnNavBackRequested(NavigationView sender, NavigationViewBackRequestedEventArgs args)
    {
        try
        {
            if (SettingsPage.Visibility == Visibility.Visible)
            {
                if (InSettingsSubPage)
                {
                    CloseSettingsSubPage();
                    return;
                }
                ShowChatPage();
            }
        }
        catch (Exception) { } // 事件入口兜底
    }

    // ---------------- 背景皮肤（侧栏「皮肤」：导入图片作内容区背景） ----------------

    // 内核数据家：品牌更名后为 Blade2。首次访问时把旧 Code2 目录整体搬迁，升级不丢会话/皮肤。
    // ShellToast 的发送者名标记文件也落在这里（internal：通知注册状态与皮肤同属壳本地状态）。
    internal static string DataHome
    {
        get
        {
            var root = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
            var home = Path.Combine(root, "Blade2");
            var legacy = Path.Combine(root, "Code2");
            if (!Directory.Exists(home) && Directory.Exists(legacy))
            {
                try { Directory.Move(legacy, home); }
                catch (Exception) { } // 旧目录被占用等原因搬迁失败：用全新 Blade2，旧目录原样保留
            }
            return home;
        }
    }

    private string SkinFile => Path.Combine(DataHome, "shell-skin.img");

    /// <summary>视频皮肤文件（与图片皮肤二选一：导入任一种会换下另一种）。</summary>
    private string SkinVideoFile => Path.Combine(DataHome, "shell-skin.video");

    // 皮肤透明度：0.10–0.80（滑杆百分比 /100）。壳本地配置，与皮肤图同目录的 sidecar 文件，
    // 不动内核 settings、也不混进托盘选项的 shell.json（两个功能各自独立演进）。
    private string SkinOpacityFile => Path.Combine(DataHome, "shell-skin.opacity");
    private double _skinOpacity = DefaultSkinOpacity;
    // 当前生效的背景笔：滑杆拖动直接改它的 Opacity，不必重载图片。
    private Microsoft.UI.Xaml.Media.ImageBrush? _skinBrush;
    // 视频皮肤：播放器 + 挂在 ContentSurface 内容 Grid 底层的 MediaPlayerElement。
    // 图片皮肤走 Border.Background，视频元素是子级，所以视频层必须在 Grid Children[0]
    // （在所有页面之下、Border 背景之上）才不影响聊天页的点线与滚动。
    private Windows.Media.Playback.MediaPlayer? _skinVideoPlayer;
    private Microsoft.UI.Xaml.Controls.MediaPlayerElement? _skinVideoElement;
    /// <summary>
    /// 窗口失焦时暂停视频背景（sidecar 持久化，默认开）：切到别的程序（或最小化）
    /// 时停掉解码循环，省电也不抢注意力，回焦恢复；关掉则焦点无关始终播放。
    /// </summary>
    private bool _skinVideoPauseOnBlur = true;
    private string SkinVideoPauseFile => Path.Combine(DataHome, "shell-skin.videopause");
    /// <summary>窗口焦点状态（Activated 事件维护；启动时按前台算，记录不到就播）。</summary>
    private bool _windowActive = true;
    private TextBlock? _skinOpacityValue;
    private Microsoft.UI.Dispatching.DispatcherQueueTimer? _skinOpacitySaveTimer;
    private const double DefaultSkinOpacity = 0.40;
    /// <summary>滑杆取值（整数百分比）。</summary>
    private int SkinOpacityPercent => (int)Math.Round(_skinOpacity * 100);

    /// <summary>读持久化的透明度；文件缺失/损坏/越界都回默认 0.40。</summary>
    private void LoadSkinOpacity()    {
        try
        {
            if (!File.Exists(SkinOpacityFile))
            {
                return;
            }
            var text = File.ReadAllText(SkinOpacityFile).Trim();
            if (double.TryParse(text, System.Globalization.NumberStyles.Float,
                    System.Globalization.CultureInfo.InvariantCulture, out var value))
            {
                _skinOpacity = Math.Clamp(value, 0.10, 0.80);
            }
        }
        catch (Exception) { } // 读不到按默认，不影响启动
    }

    /// <summary>读失焦暂停开关；文件缺失按默认开（这个行为本身就是需求默认态）。</summary>
    private void LoadSkinVideoPauseOnBlur()
    {
        try
        {
            if (!File.Exists(SkinVideoPauseFile))
            {
                return;
            }
            var text = File.ReadAllText(SkinVideoPauseFile).Trim();
            if (text is "0" or "false")
            {
                _skinVideoPauseOnBlur = false;
            }
        }
        catch (Exception) { } // 读不到按默认，不影响启动
    }

    private void SaveSkinVideoPauseOnBlur()
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(SkinVideoPauseFile)!);
            File.WriteAllText(SkinVideoPauseFile, _skinVideoPauseOnBlur ? "1" : "0");
        }
        catch (Exception) { } // 落盘失败不影响当前会话行为
    }

    /// <summary>
    /// 窗口焦点变化：开关开着时视频背景只在聚焦时播放——失焦暂停、回焦恢复
    /// （最小化也走 Deactivated，同样算失焦）。失焦期间视频被换掉/清掉时播放器
    /// 已释放，判空跳过；开关关掉则焦点变化与播放无关。
    /// </summary>
    private void OnWindowActivated(object sender, WindowActivatedEventArgs args)
    {
        _windowActive = args.WindowActivationState != WindowActivationState.Deactivated;
        if (!_skinVideoPauseOnBlur || _skinVideoPlayer is null)
        {
            return;
        }
        try
        {
            if (_windowActive)
            {
                _skinVideoPlayer.Play();
            }
            else
            {
                _skinVideoPlayer.Pause();
            }
        }
        catch (Exception) { } // 播放器正在换源的瞬间可能拒停，下一轮事件再纠正
    }

    /// <summary>设置页开关切换：落盘并立即按新值执行一次（开着且正失焦就停）。</summary>
    private void OnSkinVideoPauseToggled(bool on)
    {
        _skinVideoPauseOnBlur = on;
        SaveSkinVideoPauseOnBlur();
        if (_skinVideoPlayer is null)
        {
            return;
        }
        try
        {
            if (!on || _windowActive)
            {
                _skinVideoPlayer.Play();
            }
            else
            {
                _skinVideoPlayer.Pause();
            }
        }
        catch (Exception) { }
    }

    private void SaveSkinOpacity()
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(SkinOpacityFile)!);
            File.WriteAllText(SkinOpacityFile,
                _skinOpacity.ToString("0.##", System.Globalization.CultureInfo.InvariantCulture));
        }
        catch (Exception) { } // 落盘失败不影响当前会话的显示
    }

    /// <summary>皮肤透明度防抖计时（300ms）：滑杆一次拖动只写一次文件。</summary>
    private void StartSkinOpacitySaveTimer()
    {
        if (_skinOpacitySaveTimer is not null)
        {
            return;
        }
        var timer = DispatcherQueue.CreateTimer();
        timer.Interval = TimeSpan.FromMilliseconds(300);
        timer.Tick += (_, _) =>
        {
            timer.Stop();
            SaveSkinOpacity();
        };
        _skinOpacitySaveTimer = timer;
    }

    private async Task PickAndApplySkinAsync()
    {
        try
        {
            var picker = new Windows.Storage.Pickers.FileOpenPicker();
            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            WinRT.Interop.InitializeWithWindow.Initialize(picker, hwnd);
            picker.FileTypeFilter.Add(".png");
            picker.FileTypeFilter.Add(".jpg");
            picker.FileTypeFilter.Add(".jpeg");
            picker.FileTypeFilter.Add(".webp");
            picker.FileTypeFilter.Add(".bmp");
            picker.SuggestedStartLocation = Windows.Storage.Pickers.PickerLocationId.PicturesLibrary;
            var file = await picker.PickSingleFileAsync();
            if (file is null)
            {
                return;
            }
            // 单槽位：图片换下视频（与「导入视频换下图片」对称）
            StopSkinVideo();
            ClearSkinWeId(); // 本地导入后当前背景不再是 WE 来源
            try
            {
                if (File.Exists(SkinVideoFile))
                {
                    File.Delete(SkinVideoFile);
                }
            }
            catch (Exception) { }
            Directory.CreateDirectory(Path.GetDirectoryName(SkinFile)!);
            File.Copy(file.Path, SkinFile, overwrite: true);
            ApplySkinFromPath(SkinFile);
        }
        catch (Exception ex)
        {
            _ = ShowErrorAsync(LF("导入皮肤失败：{0}", ex.Message));
        }
    }

    /// <summary>导入视频皮肤：复制进 DataHome 并静音循环播放。单槽位语义——
    /// 视频换下图片（删图片文件，与「导入图片换下视频」对称）。</summary>
    private async Task PickAndApplySkinVideoAsync()
    {
        try
        {
            var picker = new Windows.Storage.Pickers.FileOpenPicker();
            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            WinRT.Interop.InitializeWithWindow.Initialize(picker, hwnd);
            picker.FileTypeFilter.Add(".mp4");
            picker.FileTypeFilter.Add(".webm");
            picker.FileTypeFilter.Add(".mov");
            picker.FileTypeFilter.Add(".m4v");
            picker.SuggestedStartLocation = Windows.Storage.Pickers.PickerLocationId.VideosLibrary;
            var file = await picker.PickSingleFileAsync();
            if (file is null)
            {
                return;
            }
            // 先停正在播的视频：播放器持着槽位文件时覆盖它会失败（与 WE 套用同序）
            StopSkinVideo();
            ClearSkinWeId(); // 本地导入后当前背景不再是 WE 来源
            Directory.CreateDirectory(Path.GetDirectoryName(SkinVideoFile)!);
            File.Copy(file.Path, SkinVideoFile, overwrite: true);
            try
            {
                if (File.Exists(SkinFile))
                {
                    File.Delete(SkinFile);
                }
            }
            catch (Exception) { } // 旧图片删不掉不影响新视频生效（下次启动优先视频）
            ApplySkinVideoFromPath(SkinVideoFile);
        }
        catch (Exception ex)
        {
            _ = ShowErrorAsync(LF("导入皮肤失败：{0}", ex.Message));
        }
    }

    private void ClearSkin()
    {
        StopSkinVideo();
        ClearSkinWeId();
        try
        {
            if (File.Exists(SkinFile))
            {
                File.Delete(SkinFile);
            }
            if (File.Exists(SkinVideoFile))
            {
                File.Delete(SkinVideoFile);
            }
        }
        catch (Exception) { }
        _skinBrush = null;
        ContentSurface.Background = ThemeBrush("SurfaceAltBrush");
    }

    private void LoadSavedSkin()
    {
        LoadSkinOpacity();
        LoadSkinVideoPauseOnBlur();
        StartSkinOpacitySaveTimer();
        try
        {
            // 视频优先于图片：单槽位语义下两者不会同时存在，防御性按视频优先。
            if (File.Exists(SkinVideoFile))
            {
                ApplySkinVideoFromPath(SkinVideoFile);
            }
            else if (File.Exists(SkinFile))
            {
                ApplySkinFromPath(SkinFile);
            }
        }
        catch (Exception) { }
    }

    /// <summary>
    /// 内容区背景：图片按用户调的透明度显示（默认 40%），下层 Mica/纯色透出，正文可读；
    /// 透明度由「个性化」分区的滑杆持久化（DataHome/shell-skin.opacity）。
    /// </summary>
    private void ApplySkinFromPath(string path)
    {
        try
        {
            var bitmap = new Microsoft.UI.Xaml.Media.Imaging.BitmapImage(new Uri(path));
            var brush = new Microsoft.UI.Xaml.Media.ImageBrush
            {
                ImageSource = bitmap,
                Stretch = Microsoft.UI.Xaml.Media.Stretch.UniformToFill,
                Opacity = _skinOpacity,
            };
            _skinBrush = brush;
            ContentSurface.Background = brush;
        }
        catch (Exception ex)
        {
            _ = ShowErrorAsync(LF("应用皮肤失败：{0}", ex.Message));
        }
    }

    /// <summary>
    /// 视频皮肤：静音循环播放（背景视频出声只会干扰工作），按滑杆透明度叠在内容区最底层。
    /// 挂在 ContentSurface 子 Grid 的 Children[0]：在所有页面之下，因此不挡聊天页交互；
    /// Border 背景复位为 SurfaceAlt，让视频自己透出（图片皮肤则继续走 Background）。
    /// </summary>
    private void ApplySkinVideoFromPath(string path)
    {
        StopSkinVideo();
        _skinBrush = null; // 图片笔已不在树上，透明度滑杆只需管视频层
        try
        {
            ContentSurface.Background = ThemeBrush("SurfaceAltBrush");
            var player = new Windows.Media.Playback.MediaPlayer
            {
                IsLoopingEnabled = true,
                IsMuted = true,
                AutoPlay = true,
            };
            player.Source = Windows.Media.Core.MediaSource.CreateFromUri(new Uri(path));
            // 解码不了的容器/编码（如 HEVC）走 Failed 给出可见原因，不留一片空白
            player.MediaFailed += (sender, args) =>
            {
                _ = ShowErrorAsync(LF("视频皮肤播放失败：{0}（建议改用 H.264 编码的 mp4）", args.ErrorMessage));
            };
            var element = new Microsoft.UI.Xaml.Controls.MediaPlayerElement
            {
                Stretch = Microsoft.UI.Xaml.Media.Stretch.UniformToFill,
                Opacity = _skinOpacity,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                VerticalAlignment = VerticalAlignment.Stretch,
            };
            element.SetMediaPlayer(player);
            if (ContentSurface.Child is Grid rootGrid)
            {
                rootGrid.Children.Insert(0, element);
            }
            _skinVideoPlayer = player;
            _skinVideoElement = element;
            // 启动/套用时窗口就不在前台（托盘恢复等）：开关开着就别自动播，等回焦
            if (_skinVideoPauseOnBlur && !_windowActive)
            {
                try
                {
                    player.Pause();
                }
                catch (Exception) { }
            }
        }
        catch (Exception ex)
        {
            _ = ShowErrorAsync(LF("应用皮肤失败：{0}", ex.Message));
        }
    }

    /// <summary>拆掉视频皮肤层并释放播放器（换皮肤/清除皮肤/重入前调用，幂等）。</summary>
    private void StopSkinVideo()
    {
        if (_skinVideoElement is not null && ContentSurface.Child is Grid rootGrid)
        {
            rootGrid.Children.Remove(_skinVideoElement);
        }
        _skinVideoElement = null;
        if (_skinVideoPlayer is not null)
        {
            try
            {
                _skinVideoPlayer.Pause();
            }
            catch (Exception) { }
            _skinVideoPlayer.Dispose();
            _skinVideoPlayer = null;
        }
    }

    /// <summary>打开设置页：describe 一次，默认进通用区；分区项按 SectionOrder 铺进主侧栏。</summary>
    private async Task ShowSettingsAsync()
    {
        if (_rpc is null)
        {
            return;
        }
        if (await EnsureSettingsSnapshotAsync() is null)
        {
            _ = ShowErrorAsync(L("读取设置失败：内核未返回设置描述。"));
            return;
        }

        _settingsSections = SectionOrder
            .Select(id => new SettingsSectionVm { Id = id, Title = SectionTitle(id), Glyph = SectionGlyph(id) })
            .ToList();
        _settingsActiveSection = "";
        // 侧栏切成设置分区：先裁掉会话树项（固定项保留但折叠让位），再追加分区项。
        // 退出时 ShowChatPage → RebuildNavMenu 会把尾部这些分区项裁掉并重建会话树。
        Nav.SelectedItem = null;
        while (Nav.MenuItems.Count > _fixedMenuItemCount)
        {
            Nav.MenuItems.RemoveAt(Nav.MenuItems.Count - 1);
        }
        for (var i = 0; i < _fixedMenuItemCount && i < Nav.MenuItems.Count; i++)
        {
            // 固定项折叠让位给分区项（MenuItems 是 object 集合）
            if (Nav.MenuItems[i] is UIElement fixedItem)
            {
                fixedItem.Visibility = Visibility.Collapsed;
            }
        }
        foreach (var section in _settingsSections)
        {
            var item = new NavigationViewItem
            {
                Content = section.Title,
                MinWidth = 0,
                // 分区项走 NavigationView 原生选中（SelectsOnInvoked 默认 true，与会话项同一路）：
                // 实测 SelectsOnInvoked=false + 手写 SelectedItem 的组合，控件会在选中转换时把
                // SelectedItem 重置回 null、选中条卡在旧分区（代码怎么写都亮不了）。
                // 点击选中由控件自己完成；ItemInvoked 仍会触发 → OnNavItemInvoked 渲染对应分区。
                Tag = section,
                Icon = new FontIcon { Glyph = section.Glyph },
            };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(item, $"SettingsSection_{section.Id}");
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(item, section.Title);
            Nav.MenuItems.Add(item);
        }

        ShowSettingsPage();
        RebuildViewOptionsMenu();
        await ActivateSectionAsync("general");
        try
        {
            Nav.Focus(FocusState.Programmatic);
        }
        catch (Exception) { }
    }

    private async Task ActivateSectionAsync(string id)
    {
        if (id == _settingsActiveSection)
        {
            return; // 重复激活同分区：幂等跳过（Tapped 与选中同步双路径防抖）
        }
        _settingsActiveSection = id;
        // 侧栏选中态同步（程序化路径：进入设置页的初始 general、UIA Select 等）。
        // 分区项本身是 SelectsOnInvoked=true 的原生选中（见 ShowSettingsAsync 的注释），
        // 这里只做对齐，不再承担点击路径的选中职责。
        foreach (var item in Nav.MenuItems.OfType<NavigationViewItem>())
        {
            if (item.Tag is SettingsSectionVm vm && vm.Id == id)
            {
                if (!ReferenceEquals(Nav.SelectedItem, item))
                {
                    Nav.SelectedItem = item;
                }
                item.IsSelected = true;
                break;
            }
        }
        await RenderSectionAsync(id);
        SettingsScroller.ChangeView(null, 0, null, true); // 换分区回到顶部（否则停在上一个分区的滚动位置）
        PlaySettingsContentEntrance(); // 新内容自下方滑入并淡入（WinUI Page refresh 语义）
    }

    // ---- 设置二级页（Windows 设置「应用」式钻取）----

    /// <summary>二级页栈：元素 = (标题, 内容面板)。内容进页时现建、返回即整棵丢弃——
    /// 内核快照在页外也可能变（别的入口写 settings），缓存内容会让用户看到旧值。</summary>
    private readonly List<(string Title, StackPanel Content)> _settingsSubPages = new();

    /// <summary>是否停在设置二级页。Esc 与返回钮的优先级高于「回聊天页」：先收二级页。</summary>
    private bool InSettingsSubPage => _settingsSubPages.Count > 0;

    /// <summary>
    /// 页头按当前钻取深度重排：一级页只显示分区名；二级及更深显示面包屑「上级 › 当前」。
    /// 上级名取紧邻的上一层（一级页时为分区名），与分隔符同走二级文字色 —— 同字号不同色，
    /// 让「现在在哪一层、这一层叫什么」一眼可分；上级名同时是返回入口（点它收一层，
    /// 见 OnSettingsBreadcrumbParentClick），因此标题旁不需要另放返回钮。
    /// 一级页收起面包屑两段（StackPanel 的 Spacing 不占已收起子项的位置，标题左缘仍与内容列对齐）。
    /// </summary>
    private void UpdateSettingsBreadcrumb()
    {
        var depth = _settingsSubPages.Count;
        if (depth == 0)
        {
            SettingsBreadcrumbParent.Visibility = Visibility.Collapsed;
            SettingsBreadcrumbSeparator.Visibility = Visibility.Collapsed;
            SettingsPageHeader.Text = SectionTitle(_settingsActiveSection);
            return;
        }
        var parent = depth == 1
            ? SectionTitle(_settingsActiveSection)
            : _settingsSubPages[depth - 2].Title;
        SettingsBreadcrumbParentText.Text = parent;
        SettingsPageHeader.Text = _settingsSubPages[depth - 1].Title;
        SettingsBreadcrumbParent.Visibility = Visibility.Visible;
        SettingsBreadcrumbSeparator.Visibility = Visibility.Visible;
    }

    /// <summary>点页头面包屑的上级名：收一层二级页 —— 与左上角 ShellBackButton、Esc 完全同一条
    /// 路由（CloseSettingsSubPage），只是在标题上多一个返回入口；原返回钮不做任何改动。
    /// 该钮只在二级页可见（一级页时 Collapsed），进来必有栈可收；收空即回到分区一级页。</summary>
    private void OnSettingsBreadcrumbParentClick(object sender, RoutedEventArgs e)
    {
        try
        {
            CloseSettingsSubPage();
        }
        catch (Exception) { } // 事件入口兜底
    }

    /// <summary>
    /// 进入设置二级页：内容挂入 SettingsSubHost，页头换成「上级 › 本级」面包屑，
    /// 一级内容列向左推出、二级内容自右向左滑入（WinUI「Drill」语义，Windows 设置的钻取方向）。
    /// 多级钻取（二级页里再点卡）走同一栈：只推走一级页一次，返回逐级弹出。
    /// </summary>
    private void OpenSettingsSubPage(string title, StackPanel content)
    {
        try
        {
            _settingsSubPages.Add((title, content));
            var firstLevel = _settingsSubPages.Count == 1;
            if (firstLevel)
            {
                // 一级列的收起交给推出动画的结束回调：提前 Collapsed 的话，「推走」的半程
                // 没有内容可看，只剩新页在空背景上滑入（旧实现的观感就是内容凭空出现）。
                SettingsSubHost.Visibility = Visibility.Visible;
            }
            SettingsSubHost.Children.Add(content);
            UpdateSettingsBreadcrumb();
            SettingsScroller.ChangeView(null, 0, null, true); // 进二级页回到顶部
            AlignSettingsHeader();
            PlaySettingsDrillIn(firstLevel);
        }
        catch (Exception ex)
        {
            // 钻取失败（内容构建抛错）不能带走界面：退回一级页，根因留档
            System.Diagnostics.Debug.WriteLine($"[settings/sub-open] {ex}");
            ResetSettingsSubPages();
        }
    }

    /// <summary>钻取进二级页，内容由 build 现建（不缓存，理由见 _settingsSubPages 注释）。
    /// build 抛错时页面照常打开，卡内显示失败原因——用户至少看得见点了什么、错在哪。</summary>
    private void OpenSettingsSubPage(string title, Action<StackPanel> build)
    {
        var content = new StackPanel();
        try
        {
            build(content);
        }
        catch (Exception ex)
        {
            content.Children.Add(new TextBlock
            {
                Text = LF("这一组设置加载失败：{0}", ex.Message),
                Style = AppStyle("CardDescriptionTextStyle"),
                TextWrapping = TextWrapping.Wrap,
            });
            System.Diagnostics.Debug.WriteLine($"[settings/sub-build] {ex}");
        }
        OpenSettingsSubPage(title, content);
    }

    /// <summary>返回上一级：弹出栈顶二级页；弹空则回到分区一级页（页头还原分区名）。
    /// 返回走与进入相反方向的转场（二级页右出、露出的一页左回），结束后再动显隐与栈。</summary>
    private void CloseSettingsSubPage()
    {
        try
        {
            if (_settingsSubPages.Count == 0)
            {
                return;
            }
            var (_, content) = _settingsSubPages[^1];
            _settingsSubPages.RemoveAt(_settingsSubPages.Count - 1);
            if (_settingsSubPages.Count == 0)
            {
                // 回一级页：一级列先回可视树才能播「自左滑回」，整棵复位交给转场结束回调
                SettingsHost.Visibility = Visibility.Visible;
                PlaySettingsDrillOut(content, SettingsHost, ResetSettingsSubPages);
            }
            else
            {
                UpdateSettingsBreadcrumb();
                AlignSettingsHeader();
                PlaySettingsDrillOut(content, _settingsSubPages[^1].Content,
                    () => SettingsSubHost.Children.Remove(content));
            }
        }
        catch (Exception) { } // 事件入口兜底
    }

    /// <summary>清空二级页状态回到分区一级页（切分区 / 回聊天页 / 钻取失败时调用）。</summary>
    private void ResetSettingsSubPages()
    {
        // 转场可能正占着一级列的 Opacity（HoldEnd 的动画让后续本地赋值失效），先停掉再复位
        StopSettingsTransition();
        try
        {
            _settingsSubPages.Clear();
            SettingsSubHost.Children.Clear();
            SettingsSubHost.Visibility = Visibility.Collapsed;
            SettingsHost.Visibility = Visibility.Visible;
            UpdateSettingsBreadcrumb();
            AlignSettingsHeader();
            // 被打断的转场落点：两列都回「可见 + 原位」，不留半透明/偏移残影
            SettingsHost.Opacity = 1;
            SettingsHost.RenderTransform = null;
            SettingsSubHost.Opacity = 1;
            SettingsSubHost.RenderTransform = null;
        }
        catch (Exception) { } // 状态复位失败不致命：最差是停在二级页，用户仍可点返回
    }

    // ---- 设置页转场（WinUI 动效标准：Page refresh 向上滑入 / Drill 自右向左推入）----

    /// <summary>在跑的设置页转场。开新转场或复位二级页前先停掉：HoldEnd 的动画会一直占着目标
    /// 属性，之后再赋本地值不生效，必须先 Stop 让属性回落基值。所有被转场元素的基值一律是
    /// 「可见 + 原位」，所以转场被打断时落点始终是内容看得见，不会把界面留在半透明里。</summary>
    private Storyboard? _settingsTransition;

    private void StopSettingsTransition()
    {
        try
        {
            _settingsTransition?.Stop();
        }
        catch (Exception) { } // 已结束或从未开始都无所谓
        _settingsTransition = null;
    }

    /// <summary>设计令牌里的转场时长；缺失时用兜底毫秒。</summary>
    private static Duration TransitionDuration(string key, double fallbackMs)
        => Application.Current.Resources.TryGetValue(key, out var v) && v is Duration d
            ? d
            : new Duration(TimeSpan.FromMilliseconds(fallbackMs));

    /// <summary>设计令牌里的缓动曲线；缺失时返回 null，动画退化为线性（仍可播放）。
    /// 只用来读控制点：KeySpline 在 WinUI 里是 DependencyObject，一个实例同时只能挂在一个
    /// 关键帧上，所以真正赋值前必须先复制（见 SplineSlide）。</summary>
    private static KeySpline? TransitionSpline(string key)
        => Application.Current.Resources.TryGetValue(key, out var v) && v is KeySpline s ? s : null;

    /// <summary>
    /// 单属性滑移动画 from → to，曲线走 KeySpline（cubic-bezier 控制点，非线性）。
    /// 用关键帧而不是 DoubleAnimation.EasingFunction：WinUI 的缓动函数只有 CubicEase 一档
    /// 三次幂曲线，给不出系统转场的控制点；WinUI 自己的 generic.xaml 也是
    /// SplineDoubleKeyFrame + ControlFastOutSlowInKeySpline 这套写法。
    /// 首帧离散落到 from：调用方把基值设成终态（可见 + 原位），没有这一帧动画会从终态起跑。
    /// KeySpline 在 WinUI 里是 DependencyObject，一个实例同时只能挂在一个关键帧上，所以这里
    /// 按控制点复制一份新的：同一个 spline 要喂 slideX/slideY/fade 三个关键帧，复用同一实例
    /// 第二次赋值就抛 ArgumentException，整条转场被 catch 吞掉退化成瞬切。
    /// </summary>
    private static DoubleAnimationUsingKeyFrames SplineSlide(double from, double to, Duration duration, KeySpline? spline)
    {
        var frames = new DoubleAnimationUsingKeyFrames();
        frames.KeyFrames.Add(new DiscreteDoubleKeyFrame { KeyTime = TimeSpan.Zero, Value = from });
        var last = new SplineDoubleKeyFrame { KeyTime = duration.TimeSpan, Value = to };
        if (spline is not null)
        {
            last.KeySpline = new KeySpline { ControlPoint1 = spline.ControlPoint1, ControlPoint2 = spline.ControlPoint2 };
        }
        frames.KeyFrames.Add(last);
        return frames;
    }

    /// <summary>进场：从 (fromX, fromY) 滑回原位并淡入。基值 = 终态（可见 + 原位）。</summary>
    private static void AddEntrance(Storyboard sb, FrameworkElement element, double fromX, double fromY,
        Duration duration, KeySpline? spline)
    {
        element.Opacity = 1;
        var trans = new TranslateTransform();
        element.RenderTransform = trans;
        var slideX = SplineSlide(fromX, 0, duration, spline);
        Storyboard.SetTarget(slideX, trans);
        Storyboard.SetTargetProperty(slideX, "X");
        var slideY = SplineSlide(fromY, 0, duration, spline);
        Storyboard.SetTarget(slideY, trans);
        Storyboard.SetTargetProperty(slideY, "Y");
        var fade = SplineSlide(0, 1, duration, spline);
        Storyboard.SetTarget(fade, element);
        Storyboard.SetTargetProperty(fade, "Opacity");
        sb.Children.Add(slideX);
        sb.Children.Add(slideY);
        sb.Children.Add(fade);
    }

    /// <summary>退场：向右（toX 为正）或向左（toX 为负）滑出并淡出。基值 = 终态（可见 + 原位），
    /// 所以 Stop 之后元素回到可见态，由调用方在同一回调里收起/移除。</summary>
    private static void AddExit(Storyboard sb, FrameworkElement element, double toX, Duration duration, KeySpline? spline)
    {
        element.Opacity = 1;
        var trans = new TranslateTransform();
        element.RenderTransform = trans;
        var slide = SplineSlide(0, toX, duration, spline);
        Storyboard.SetTarget(slide, trans);
        Storyboard.SetTargetProperty(slide, "X");
        var fade = SplineSlide(1, 0, duration, spline);
        Storyboard.SetTarget(fade, element);
        Storyboard.SetTargetProperty(fade, "Opacity");
        sb.Children.Add(slide);
        sb.Children.Add(fade);
    }

    /// <summary>
    /// 设置内容列进场（WinUI「Page refresh」语义：向上滑入 + 淡入）。
    /// 左栏分区项是同级页面导航：进新内容要从下方滑入并淡入，让用户感到「重新开始」，
    /// 而不是旧内容原地被换掉。只用于进入设置页与切换分区 —— 保存后的分区重渲染不走这里，
    /// 否则每存一次设置内容就滑一次，像跳走了页。
    /// </summary>
    private void PlaySettingsContentEntrance()
    {
        try
        {
            StopSettingsTransition();
            if (!SystemAnimationsEnabled())
            {
                // 辅助功能/视觉效果里的「动画效果」开关（前庭功能障碍用户依赖）：关闭后不做位移
                SettingsHost.Opacity = 1;
                SettingsHost.RenderTransform = null;
                return;
            }
            var duration = TransitionDuration("DurationPageTransition", 250);
            var sb = new Storyboard();
            AddEntrance(sb, SettingsHost, 0, TokenDouble("SettingsContentSlideOffset", 40),
                duration, TransitionSpline("SplineFastOutSlowIn"));
            _settingsTransition = sb;
            sb.Begin();
        }
        catch (Exception)
        {
            SettingsHost.Opacity = 1; // 动画起不来也必须看得见：内容优先于动效
        }
    }

    /// <summary>
    /// 钻取进二级页（WinUI「Drill」语义）：一级内容列向左推出并淡出，二级内容自右向左滑入并淡入。
    /// 一级列要留在可视树里直到推出动画结束再收起 —— 提前 Collapsed 的话，「推走」的半程
    /// 没有内容可看，只剩新页在空背景上滑入。
    /// 多级钻取时一级列早已收起，pushFirstLevel=false 就只让新内容滑入。
    /// </summary>
    private void PlaySettingsDrillIn(bool pushFirstLevel)
    {
        try
        {
            StopSettingsTransition();
            var duration = TransitionDuration("DurationPageTransition", 250);
            var offset = TokenDouble("SettingsDrillSlideOffset", 120);
            var sb = new Storyboard();
            if (!SystemAnimationsEnabled())
            {
                if (pushFirstLevel)
                {
                    SettingsHost.Visibility = Visibility.Collapsed;
                }
                SettingsSubHost.Opacity = 1;
                SettingsSubHost.RenderTransform = null;
                return;
            }
            AddEntrance(sb, SettingsSubHost, offset, 0, duration, TransitionSpline("SplineFastOutSlowIn"));
            if (pushFirstLevel)
            {
                AddExit(sb, SettingsHost, -offset, duration, TransitionSpline("SplineSlowOutFastIn"));
                sb.Completed += (_, _) =>
                {
                    if (!ReferenceEquals(_settingsTransition, sb))
                    {
                        return; // 已被新转场接管：显隐归新转场负责，这里不能再动
                    }
                    StopSettingsTransition();
                    SettingsHost.Visibility = Visibility.Collapsed;
                    SettingsHost.RenderTransform = null;
                };
            }
            _settingsTransition = sb;
            sb.Begin();
        }
        catch (Exception)
        {
            if (pushFirstLevel)
            {
                SettingsHost.Visibility = Visibility.Collapsed;
            }
            SettingsSubHost.Opacity = 1;
        }
    }

    /// <summary>
    /// 钻取返回（WinUI GoBack 语义）：离开的那页向右滑出并淡出，露出的一页自左滑回并淡入。
    /// 方向与进入相反 —— 用户靠方向判断自己在层级里往上走还是往下走。
    /// onArrived 在转场结束后跑（移除离开的那页、复位一级页显隐与页头）。
    /// </summary>
    private void PlaySettingsDrillOut(FrameworkElement leaving, FrameworkElement arriving, Action onArrived)
    {
        try
        {
            StopSettingsTransition();
            var duration = TransitionDuration("DurationPageTransition", 250);
            var offset = TokenDouble("SettingsDrillSlideOffset", 120);
            if (!SystemAnimationsEnabled())
            {
                onArrived();
                return;
            }
            var sb = new Storyboard();
            AddExit(sb, leaving, offset, duration, TransitionSpline("SplineSlowOutFastIn"));
            AddEntrance(sb, arriving, -offset, 0, duration, TransitionSpline("SplineFastOutSlowIn"));
            sb.Completed += (_, _) =>
            {
                if (!ReferenceEquals(_settingsTransition, sb))
                {
                    return; // 已被新转场接管
                }
                StopSettingsTransition();
                onArrived();
            };
            _settingsTransition = sb;
            sb.Begin();
        }
        catch (Exception)
        {
            onArrived(); // 动效失败也要把状态走完：页面必须回到正确层级
        }
    }



    /// <summary>页面淡入：时长/缓动取设计令牌（Theme/Tokens.xaml），不写死毫秒与贝塞尔。</summary>
    private static void FadeInPage(FrameworkElement page)
        => FadeInPage(page, "DurationNormal");

    /// <summary>
    /// 页面/浮层淡入。durationKey 指向 Tokens.xaml 的时长令牌（DurationFast/Normal/Slow），
    /// 缓动固定取 EaseOut（进场用减速曲线，与 WinUI 内建转场一致）。
    /// 系统"动画效果"开关关闭时不播放动画，直接落终态。
    /// </summary>
    private static void FadeInPage(FrameworkElement page, string durationKey)
    {
        try
        {
            if (!SystemAnimationsEnabled())
            {
                // 设置 → 辅助功能 → 视觉效果 → 动画效果：关闭后不得再有位移/淡入，
                // 这是前庭功能障碍用户依赖的系统级开关。
                page.Opacity = 1;
                return;
            }
            page.Opacity = 0;
            var animation = new DoubleAnimation
            {
                To = 1,
                Duration = Application.Current.Resources.TryGetValue(durationKey, out var duration) && duration is Duration d
                    ? d
                    : new Duration(TimeSpan.FromMilliseconds(200)),
                EasingFunction = Application.Current.Resources.TryGetValue("EaseOut", out var ease) && ease is EasingFunctionBase easing
                    ? easing
                    : null,
            };
            Storyboard.SetTarget(animation, page);
            Storyboard.SetTargetProperty(animation, "Opacity");
            var storyboard = new Storyboard();
            storyboard.Children.Add(animation);
            storyboard.Begin();
        }
        catch (Exception)
        {
            // 动画失败不影响可用性：直接把不透明度还原
            page.Opacity = 1;
        }
    }

    /// <summary>
    /// 浮层（命令补全 / @ 引用补全）出现：显示 + 淡入。
    /// 用比页面转场更快的一档（DurationFast）——浮层是击键反馈，延迟感必须低。
    /// </summary>
    private static void ShowOverlay(FrameworkElement overlay)
    {
        overlay.Visibility = Visibility.Visible;
        FadeInPage(overlay, "DurationFast");
    }

    /// <summary>
    /// 系统"动画效果"开关的当前值（带缓存；开关变化后自动失效重读）。
    /// 取不到时按"开"处理——保持既有视觉，不因读设置失败而改变行为。
    /// </summary>
    private static bool SystemAnimationsEnabled()
    {
        if (_animationsEnabled is bool cached)
        {
            return cached;
        }
        try
        {
            _uiSettings ??= new Windows.UI.ViewManagement.UISettings();
            _uiSettings.AnimationsEnabledChanged += (_, _) => _animationsEnabled = null;
            _animationsEnabled = _uiSettings.AnimationsEnabled;
            return _animationsEnabled.Value;
        }
        catch (Exception)
        {
            return true;
        }
    }

    private static Windows.UI.ViewManagement.UISettings? _uiSettings;
    private static bool? _animationsEnabled;

    /// <summary>设计令牌里的数值（间距/圆角/控件尺寸）：与主题无关，直接从应用资源读，缺失时用兜底值。</summary>
    private static double TokenDouble(string key, double fallback)
        => Application.Current.Resources.TryGetValue(key, out var value) && value is double d ? d : fallback;

    /// <summary>设计令牌里的 Thickness（复合内边距/外边距）。</summary>
    private static Thickness TokenThickness(string key, Thickness fallback)
        => Application.Current.Resources.TryGetValue(key, out var value) && value is Thickness t ? t : fallback;

    /// <summary>样式查找（Typography.xaml 的应用字阶层 + Styles.xaml 的控件样式层）：
    /// 代码构造的控件只引用样式键，不在 C# 里写 FontSize/FontWeight/内边距。</summary>
    private static Style AppStyle(string key) => (Style)Application.Current.Resources[key];

    /// <summary>给代码构造的交互控件补 UIA 身份：AutomationId（自动化定位）+ Name（屏幕阅读器朗读）。
    /// 代码构造的控件没有 x:Name 可依托，两个属性必须显式给，否则 UIA 树里只剩控件类型。</summary>
    private static T Aut<T>(T element, string id, string name) where T : UIElement
    {
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(element, id);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(element, name);
        return element;
    }

    /// <summary>node 是否位于 root 的可视子树内（行级 Tapped 区分“点在开关上/行空白处”用）。</summary>
    private static bool IsInSubtree(Microsoft.UI.Xaml.DependencyObject? root, Microsoft.UI.Xaml.DependencyObject? node)
    {
        while (node is not null)
        {
            if (ReferenceEquals(node, root))
            {
                return true;
            }
            node = Microsoft.UI.Xaml.Media.VisualTreeHelper.GetParent(node);
        }
        return false;
    }

    // 间距栅格（Tokens.xaml Space*）——代码构造控件的 Padding/Margin/Spacing 只用这些档
    private static double Sp2 => TokenDouble("Space2", 2);
    private static double Sp4 => TokenDouble("Space4", 4);
    private static double Sp6 => TokenDouble("Space6", 6);
    private static double Sp8 => TokenDouble("Space8", 8);
    private static double Sp10 => TokenDouble("Space10", 10);
    private static double Sp12 => TokenDouble("Space12", 12);
    private static double Sp14 => TokenDouble("Space14", 14);
    private static double Sp16 => TokenDouble("Space16", 16);
    private static double Sp24 => TokenDouble("Space24", 24);
    private static double Sp32 => TokenDouble("Space32", 32);

    // 圆角（Tokens.xaml Corner*）
    private static CornerRadius RadSmall => new(TokenDouble("RadiusSmall", 4));
    private static CornerRadius RadMedium => new(TokenDouble("RadiusMedium", 8));
    private static CornerRadius RadLarge => new(TokenDouble("RadiusLarge", 12));
    private static CornerRadius RadPill => new(TokenDouble("RadiusPill", 24));

    // 控件尺寸（Tokens.xaml 尺寸段）
    private static Thickness Stroke1 => TokenThickness("StrokeThickness", new Thickness(1));
    private static double GlyphBody => TokenDouble("GlyphSizeBody", 14);
    private static double GlyphCaption => TokenDouble("GlyphSizeCaption", 12);
    private static double StatusDot => TokenDouble("StatusDotSize", 7);
    private static double SmallButtonSize => TokenDouble("ComposerAddButtonSize", 28);

    /// <summary>
    /// 根层隧道键：Esc = 从平级工具页（设置/使用统计）返回聊天页。
    /// 键盘走查实测：进设置后除鼠标点返回钮外没有键路可退，而 Esc 是用户对"退回上一层"的通用预期。
    /// 返回钮移到窗口左上角后，Esc 与返回钮同路由（ShowChatPage）。
    /// 让路规则：命令浮层/引用浮层可见时交给输入框的 HandlePaletteKey/HandleReferenceKey
    /// 去关闭浮层（那里会置 e.Handled），本处理器不抢；ContentDialog 的焦点在其
    /// 独立 popup 树内，隧道事件根本到不了这里，因此对话框自己的 Esc 不受影响。
    /// 图片放大浮层可见时 Esc 优先关图：它是盖住一切的最上层模态，压过下面所有浮层与页面。
    /// </summary>
    private void OnRootPreviewKeyDown(object sender, KeyRoutedEventArgs e)
    {
        try
        {
            if (e.Key != Windows.System.VirtualKey.Escape)
            {
                return;
            }
            if (ImageLightboxLayer.Visibility == Visibility.Visible)
            {
                e.Handled = true;
                HideImageLightbox();
                return;
            }
            if (CommandPalette.Visibility == Visibility.Visible || ReferencePalette.Visibility == Visibility.Visible)
            {
                return;
            }
            if (SettingsPage.Visibility == Visibility.Visible)
            {
                e.Handled = true;
                // 二级页开着时 Esc 只收二级页（Windows 设置的返回层级），再按才回聊天页
                if (InSettingsSubPage)
                {
                    CloseSettingsSubPage();
                    return;
                }
                ShowChatPage();
            }
        }
        catch (Exception) { } // 事件入口兜底
    }

    // ---- 通用小部件 ----

    /// <summary>
    /// 输入区自适应布局：按输入区实际可用宽度连续计算，不用 AdaptiveTrigger——
    /// 断点按窗口宽算，而这里的真实约束是「内容列宽 - 侧栏 - 文件右栏」，三者在不同窗口下是
    /// 不同的比例，用窗口断点会在分栏场景下错档。
    ///
    /// 规则：
    ///   · 动作行固定件（+ 28 / 发送 34 / 六条列间距 48 / 计划 chip）先扣掉，剩下的给
    ///     「工作区选择器 + 预设选择器 + 权限胶囊 + 模型胶囊」这一组；
    ///   · 前三个（第 6 条要求紧邻）各自封顶：选择器 220、权限 220，内部标签走省略，
    ///     Auto 列本身不可收缩，靠上限把「溢出」变成「受控收缩」；
    ///   · 剩余宽度够（≥560）→ 单行：模型胶囊按内容贴合、封顶 280，与大窗外观一致；
    ///   · 剩余宽度不够 → 模型胶囊换到动作行第二行并占满整行（换行而不是把胶囊压成竖条），
    ///     第一行始终留「+ 工作区 预设 权限 … 发送」相邻不错位。
    /// 幂等：宽度未变化时直接返回；选择器显隐不改变输入区宽度，走 force 通道重算。
    /// </summary>
    private void ApplyComposerAdaptiveLayout() => ApplyComposerAdaptiveLayout(force: false);

    /// <summary>自适应重算中：选择器显隐会改变动作行的可用宽度但**不会**改变 ComposerBar 宽度，
    /// 因此需要一条强制重算路径；重入保护避免「重算 → 重排 → 再重算」自激。</summary>
    private bool _composerLayoutApplying;

    private void ApplyComposerAdaptiveLayout(bool force)
    {
        try
        {
            if (_composerLayoutApplying)
            {
                return;
            }
            var barW = ComposerBar.ActualWidth;
            if (barW <= 0 || (!force && Math.Abs(barW - _composerLayoutWidth) < 0.5))
            {
                return;
            }
            _composerLayoutApplying = true;

            // 卡描边(2) + 卡右内边距(4) + 行左右内边距(16)：动作行可用宽 = 输入区宽 - 22
            var rowW = Math.Max(120.0, barW - 22.0);
            var planW = PlanChip.Visibility == Visibility.Visible ? 128.0 : 0.0;
            // 动作行固定件：+ 28 / 发送 34 / 六条 8dip 列间距 = 110（计划 chip 占 * 列，另扣）。
            // 上下文容量圈（模型与发送之间）可见时再扣一份：圈 28 + 一条 8dip 列间距 = 36。
            var ctxW = ContextMeterRing.Visibility == Visibility.Visible ? 36.0 : 0.0;
            // 选择器（工作区/预设）已迁入本行，与权限胶囊一起构成"可收缩组"，共享剩下的宽度。
            var pillsW = Math.Max(0.0, rowW - 110.0 - planW - ctxW);
            // 模型胶囊是否换行：换行后第一行只放「+ 工作区 预设 权限 … 发送」
            var wrapModel = pillsW < 560.0;

            // 上限之和 ≤ pillsW 是硬约束：Auto 列不会自行收缩，超了就溢出（实测 450dip 窗口下
            // 发送钮被压成 1dip 宽）。份额之和取 ≤ 1，下限保证标签在最窄的行里也留得住可读宽度；
            // 下限之和本身仍可能超过极窄行，此时按同一比例整体收敛，保证永不溢出。
            var selectorCap = Math.Min(TokenDouble("ComposerSelectorMaxWidth", 220), Math.Max(96.0, pillsW * (wrapModel ? 0.28 : 0.22)));
            var permissionCap = Math.Min(TokenDouble("ComposerPillMaxWidth", 220), Math.Max(88.0, pillsW * (wrapModel ? 0.34 : 0.24)));
            var modelCap = wrapModel
                ? 0.0   // 换行时模型胶囊独占第二行，不参与第一行的宽度分配
                : Math.Min(TokenDouble("ComposerModelPillMaxWidth", 280), Math.Max(120.0, pillsW * 0.32));
            var capSum = selectorCap * 2 + permissionCap + modelCap;
            if (capSum > pillsW && capSum > 0)
            {
                var k = pillsW / capSum;
                selectorCap *= k;
                permissionCap *= k;
                modelCap *= k;
            }
            WorkspacePickerButton.MaxWidth = selectorCap;
            AgentModeButton.MaxWidth = selectorCap;

            if (wrapModel)
            {
                // 窄窗：模型胶囊换行占满整行（第 6 条要求三个控件在窄窗下仍相邻不错位），
                // 权限胶囊吃掉单行剩余（拿不满时文本省略）
                Grid.SetRow(ModelButton, 1);
                Grid.SetColumn(ModelButton, 0);
                Grid.SetColumnSpan(ModelButton, 6);
                ModelButton.Margin = new Thickness(0, TokenDouble("Space4", 4), 0, 0);
                ModelButton.HorizontalAlignment = HorizontalAlignment.Stretch;
                ModelButton.MaxWidth = double.PositiveInfinity;
                PermissionButton.MaxWidth = permissionCap;
            }
            else
            {
                // 常规及更宽：单行，胶囊按内容贴合（封顶大于自然宽度即不截断）
                Grid.SetRow(ModelButton, 0);
                Grid.SetColumn(ModelButton, 5);
                Grid.SetColumnSpan(ModelButton, 1);
                ModelButton.Margin = new Thickness(0);
                ModelButton.HorizontalAlignment = HorizontalAlignment.Left;
                PermissionButton.MaxWidth = permissionCap;
                ModelButton.MaxWidth = modelCap;
            }
        }
        catch (Exception) { } // 布局兜底：自适应失败不致命，最差退回 XAML 基线布局
        finally
        {
            _composerLayoutApplying = false;
        }
    }

    /// <summary>主题感知 brush 查找：键 = Tokens.xaml 的语义颜色令牌（与 XAML 消费点同源，
    /// 代码不直接引 WinUI 原始色键），必须按窗口当前 RequestedTheme 解析——
    /// Application.Current.Resources 按应用主题（跟随系统）解析且不随窗口换肤，
    /// 窗壳偏好 dark 而系统为浅色时会解析出浅色卡底画在深色窗口上（设置页全灰的根因）。
    /// 探针 = XAML 根 Grid 里以 {ThemeResource} 绑定的 Collapsed TextBlock：框架随窗口
    /// 换肤重估其 Foreground，每次直读即窗口当前主题的 brush 实例（不缓存，换肤后
    /// ApplyShellTheme 先 UpdateLayout 冲刷再重渲染当前分区，消费的即新主题值）。</summary>
    private Brush ThemeBrush(string key) => key switch
    {
        "TextPrimaryBrush" => ThemeProbeTextPrimaryBrush.Foreground,
        "TextSecondaryBrush" => ThemeProbeTextSecondaryBrush.Foreground,
        "TextTertiaryBrush" => ThemeProbeTextTertiaryBrush.Foreground,
        "TextDisabledBrush" => ThemeProbeTextDisabledBrush.Foreground,
        // 「无材质」时根网格的纯色底（不透明 SolidBackgroundFillColorBase）
        "SurfaceBrush" => ThemeProbeSurfaceBrush.Foreground,
        // 气泡笔刷（半透明实色或系统 acrylic；ApplyBubbleMaterial 按气泡材质/不透明度实时换实例）
        "BubbleMicaBrush" => ThemeProbeBubbleBrush.Background,
        "CardBrush" => ThemeProbeCardBrush.Foreground,
        "CardSecondaryBrush" => ThemeProbeCardSecondaryBrush.Foreground,
        "CardHoverBrush" => ThemeProbeCardHoverBrush.Foreground,
        "StrokeBrush" => ThemeProbeStrokeBrush.Foreground,
        "StrokeSubtleBrush" => ThemeProbeStrokeSubtleBrush.Foreground,
        "InfoBrush" => ThemeProbeInfoBrush.Foreground,
        "WarningBrush" => ThemeProbeWarningBrush.Foreground,
        "SuccessBrush" => ThemeProbeSuccessBrush.Foreground,
        "ErrorBrush" => ThemeProbeErrorBrush.Foreground,
        "AccentBrush" => ThemeProbeAccentBrush.Foreground,
        "ControlFillBrush" => ThemeProbeControlFillBrush.Foreground,
        "ControlHoverBrush" => ThemeProbeControlHoverBrush.Foreground,
        "ControlPressedBrush" => ThemeProbeControlPressedBrush.Foreground,
        "ChartSeries1Brush" => ThemeProbeChartSeries1Brush.Foreground,
        "ChartSeries2Brush" => ThemeProbeChartSeries2Brush.Foreground,
        "ChartSeries3Brush" => ThemeProbeChartSeries3Brush.Foreground,
        "ChartSeries4Brush" => ThemeProbeChartSeries4Brush.Foreground,
        "ChartSeries5Brush" => ThemeProbeChartSeries5Brush.Foreground,
        "ChartSeries6Brush" => ThemeProbeChartSeries6Brush.Foreground,
        "ChartHeatEmptyBrush" => ThemeProbeChartHeatEmptyBrush.Foreground,
        "ChartGridBrush" => ThemeProbeChartGridBrush.Foreground,
        _ => (Brush)Application.Current.Resources[key],
    };

    /// <summary>JSON 对象里的字符串字段（缺失/非字符串 → 空串）。事件载荷解析统一用它，
    /// 免得每处都写 TryGetProperty + ValueKind 判断。</summary>
    private static string Str(JsonElement obj, string name)
        => obj.ValueKind == JsonValueKind.Object && obj.TryGetProperty(name, out var el) && el.ValueKind == JsonValueKind.String
            ? el.GetString() ?? ""
            : "";

    /// <summary>设置行（Windows 11 设置卡内行）：左标题/描述，右控件。
    /// 窄窗自动堆叠为上下两行（控件整行左对齐），避免标题被压成竖条。</summary>
    private Grid MakeRow(string title, string? description, FrameworkElement control)
    {
        var grid = new Grid
        {
            Padding = new Thickness(0, TokenDouble("Space12", 12), 0, TokenDouble("Space12", 12)),
            ColumnSpacing = TokenDouble("Space24", 24),
            RowSpacing = TokenDouble("Space8", 8),
        };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        var text = new StackPanel { Spacing = Sp2, VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(new TextBlock { Text = title, Style = AppStyle("BodyTextStyle") });
        if (!string.IsNullOrEmpty(description))
        {
            text.Children.Add(new TextBlock
            {
                Text = description,
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextSecondaryBrush"),
                TextWrapping = TextWrapping.Wrap,
            });
        }
        Grid.SetColumn(text, 0);
        Grid.SetColumn(control, 1);
        control.VerticalAlignment = VerticalAlignment.Center;
        grid.Children.Add(text);
        grid.Children.Add(control);
        // 窄窗：控件挪到第二行整行（自适应伸缩）
        grid.SizeChanged += (_, e) =>
        {
            try
            {
                var narrow = e.NewSize.Width is > 0 and < 520;
                if (narrow)
                {
                    Grid.SetRow(control, 1);
                    Grid.SetColumn(control, 0);
                    Grid.SetColumnSpan(control, 2);
                    control.HorizontalAlignment = HorizontalAlignment.Stretch;
                }
                else
                {
                    Grid.SetRow(control, 0);
                    Grid.SetColumn(control, 1);
                    Grid.SetColumnSpan(control, 1);
                    control.HorizontalAlignment = HorizontalAlignment.Right;
                }
            }
            catch (Exception) { }
        };
        return grid;
    }

    private void AddDivider(StackPanel host)
        => host.Children.Add(new Microsoft.UI.Xaml.Shapes.Rectangle { Style = AppStyle("DividerStyle") });

    /// <summary>
    /// 设置一级页的导航卡（Windows 设置「应用」页形态）：图标 + 标题/说明 + 右侧 chevron，
    /// 整卡即一枚按钮，点它钻取进二级页（调用方给 open，通常包 OpenSettingsSubPage）。
    /// 卡壳与其它设置卡同源（CardBrush/StrokeBrush/RadiusMedium），内边距取 16/12 令牌；
    /// 用按钮而不是 Border+Tapped：悬停/按下/禁用四态与指针光标都由按钮模板给全，
    /// 不需要自己画一套交互反馈（WinUI 里 Border 没有这些态）。
    /// </summary>
    private Button MakeSettingsNavCard(string automationId, string glyph, string title, string desc, Action open)
    {
        var row = new Grid { ColumnSpacing = Sp16 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var icon = new FontIcon
        {
            Glyph = glyph,
            FontSize = GlyphBody,
            VerticalAlignment = VerticalAlignment.Center,
        };
        Grid.SetColumn(icon, 0);
        row.Children.Add(icon);

        var text = new StackPanel { Spacing = Sp2, VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(new TextBlock { Text = title, Style = AppStyle("BodyTextStyle") });
        if (!string.IsNullOrEmpty(desc))
        {
            text.Children.Add(new TextBlock
            {
                Text = desc,
                Style = AppStyle("CardDescriptionTextStyle"),
                TextWrapping = TextWrapping.Wrap,
            });
        }
        Grid.SetColumn(text, 1);
        row.Children.Add(text);

        var chevron = new FontIcon
        {
            // chevron right（E76C）：E76B 是左向，右向才是「钻取进下一级」的方向
            Glyph = "\uE76C",
            FontSize = GlyphCaption,
            Foreground = ThemeBrush("TextTertiaryBrush"),
            VerticalAlignment = VerticalAlignment.Center,
        };
        Grid.SetColumn(chevron, 2);
        row.Children.Add(chevron);

        var button = new Button
        {
            Content = row,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Padding = new Thickness(Sp16, Sp12, Sp16, Sp12),
            MinWidth = 0,
            CornerRadius = new CornerRadius(TokenDouble("RadiusMedium", 8)),
            Background = ThemeBrush("CardBrush"),
            BorderBrush = ThemeBrush("StrokeBrush"),
            BorderThickness = Stroke1,
        };
        button.Click += (_, _) => open();
        Aut(button, automationId, title);
        return button;
    }

    /// <summary>分区节标题（节头样式：BodyStrong 字号字重 + 分区间距，取值在 Styles.xaml）。</summary>
    private TextBlock MakeSectionTitle(string text) => new()
    {
        Text = text,
        Style = AppStyle("SectionHeaderTextStyle"),
    };

    /// <summary>分区说明文字（节标题下，次要色小字）。</summary>
    private TextBlock MakeSectionDesc(string text) => new()
    {
        Text = text,
        Style = AppStyle("CardDescriptionTextStyle"),
        Margin = new Thickness(0, 0, 0, TokenDouble("Space4", 4)),
    };

    /// <summary>设置卡外壳（Windows 11 设置页卡片：卡底填充 + 1px 描边 + 8px 圆角——
    /// Windows 11 设置卡形态：内边距 16,5,16,5，行距靠卡内 StackPanel 天然分隔）。
    /// 描边/填充必须走 ThemeBrush 按窗口主题解析（换肤后重渲染同源）。</summary>
    private Border CardShell() => new()
    {
        Background = ThemeBrush("CardBrush"),
        BorderBrush = ThemeBrush("StrokeBrush"),
        BorderThickness = Stroke1,
        CornerRadius = new CornerRadius(TokenDouble("RadiusMedium", 8)),
        Padding = new Thickness(
            TokenDouble("Space16", 16), 5,
            TokenDouble("Space16", 16), 5),
        // 铺满内容列：与其它分区卡片同宽，避免各自被 StackPanel 压成内容宽而显得「单独居中」
        HorizontalAlignment = HorizontalAlignment.Stretch,
    };

    /// <summary>开一张设置卡挂到指定容器：返回卡内行容器。
    /// cardHeader/cardDesc = 卡内小节头（14px SemiBold）与说明；可全空（纯行卡）。
    /// 内边距已由 CardShell 提供（16,5），此处只排内容。
    /// host 参数为二级页而设：同一段建卡逻辑既要能挂分区一级页，也要能挂钻取进来的二级页，
    /// 因此容器由调用方给，不默认写死 SettingsHost。</summary>
    private StackPanel NewCardIn(StackPanel host, string? cardHeader = null, string? cardDesc = null)
    {
        var rows = new StackPanel();
        var card = CardShell();
        card.Child = rows;
        if (!string.IsNullOrEmpty(cardHeader))
        {
            rows.Children.Add(new TextBlock
            {
                Text = cardHeader,
                Style = AppStyle("CardHeaderTextStyle"),
            });
            if (!string.IsNullOrEmpty(cardDesc))
            {
                rows.Children.Add(new TextBlock
                {
                    Text = cardDesc,
                    Style = AppStyle("CardDescriptionTextStyle"),
                });
            }
        }
        host.Children.Add(card);
        return rows;
    }

    /// <summary>开一张设置卡挂到设置页一级内容列（二级页用 NewCardIn 指定宿主）。</summary>
    private StackPanel NewCard(string? cardHeader = null, string? cardDesc = null)
        => NewCardIn(SettingsHost, cardHeader, cardDesc);

    /// <summary>
    /// schemastery schema 的 rehydrate：toJSON 产生 {uid, refs:{id:node}}（list/dict 里是 uid
    /// 索引而非内联对象），解引用为普通可遍历的节点树（内核 schema 的 rehydrate 语义）。
    /// 根节点 = refs[uid]（toJSON 的 uid 指向根定义，引用表本体不是树）。
    /// </summary>
    private static JsonElement? RehydrateSchema(JsonElement schema)
    {
        if (schema.ValueKind != JsonValueKind.Object ||
            !schema.TryGetProperty("uid", out var uidEl) || uidEl.ValueKind != JsonValueKind.Number ||
            !schema.TryGetProperty("refs", out var refs) || refs.ValueKind != JsonValueKind.Object)
        {
            return null; // 已是展开形态（或空）
        }
        if (!uidEl.TryGetInt32(out var uid) || !refs.TryGetProperty(uid.ToString(), out var root))
        {
            return null;
        }
        var buffer = new MemoryStream();
        using (var writer = new Utf8JsonWriter(buffer))
        {
            WriteRehydrated(writer, root, schema);
        }
        using var doc = JsonDocument.Parse(buffer.ToArray());
        return doc.RootElement.Clone();
    }

    /// <summary>递归解引用：整数且命中 refs 表 = 引用索引 → 展开；其余数字原样写出。
    /// 注意：schema 里的普通数字（温度 0.7、上下文长度 128000…）不是引用，
    /// 早期实现无条件 GetInt32 会在 llm-pi-ai 这类含小数的 schema 上抛 FormatException
    /// （曾致设置→模型整页加载失败）。</summary>
    private static void WriteRehydrated(Utf8JsonWriter writer, JsonElement node, JsonElement root)
    {
        switch (node.ValueKind)
        {
            case JsonValueKind.Number:
                // 只有"整数 + refs 命中"才是引用索引；其余（小数/超 int32/普通数值）原样写出
                if (node.TryGetInt32(out var index) &&
                    root.TryGetProperty("refs", out var refs) &&
                    refs.TryGetProperty(index.ToString(), out var target))
                {
                    WriteRehydrated(writer, target, root);
                }
                else
                {
                    node.WriteTo(writer);
                }
                break;
            case JsonValueKind.Object:
                writer.WriteStartObject();
                foreach (var prop in node.EnumerateObject())
                {
                    if (prop.Name == "refs" || prop.Name == "uid")
                    {
                        continue; // 解引用后引用表不再需要
                    }
                    writer.WritePropertyName(prop.Name);
                    WriteRehydrated(writer, prop.Value, root);
                }
                writer.WriteEndObject();
                break;
            case JsonValueKind.Array:
                writer.WriteStartArray();
                foreach (var item in node.EnumerateArray())
                {
                    WriteRehydrated(writer, item, root);
                }
                writer.WriteEndArray();
                break;
            default:
                node.WriteTo(writer);
                break;
        }
    }

    /// <summary>schemastery union 节点 → 选项列表（const 值 + meta.description 标签）。</summary>
    private static List<(string Value, string Label)> UnionChoices(JsonElement? schemaNode)
    {
        var result = new List<(string, string)>();
        if (schemaNode is not { } node || node.ValueKind != JsonValueKind.Object)
        {
            return result;
        }
        if (node.TryGetProperty("type", out var t) && t.GetString() == "union" &&
            node.TryGetProperty("list", out var list) && list.ValueKind == JsonValueKind.Array)
        {
            foreach (var entry in list.EnumerateArray())
            {
                if (entry.ValueKind != JsonValueKind.Object ||
                    !entry.TryGetProperty("type", out var et) || et.GetString() != "const" ||
                    !entry.TryGetProperty("value", out var v) || v.ValueKind != JsonValueKind.String)
                {
                    continue;
                }
                var label = v.GetString()!;
                if (entry.TryGetProperty("meta", out var meta) &&
                    meta.ValueKind == JsonValueKind.Object &&
                    meta.TryGetProperty("description", out var desc) &&
                    desc.ValueKind == JsonValueKind.String && desc.GetString() is { Length: > 0 } d)
                {
                    label = d;
                }
                result.Add((v.GetString()!, label));
            }
        }
        return result;
    }

    /// <summary>从 namespace schema（rehydrate 后的节点树）取字段节点：object.dict[field]。</summary>
    private static JsonElement? SchemaField(JsonElement schema, string field)
    {
        // 引用表形态：先 rehydrate
        var rehydrated = RehydrateSchema(schema);
        var node = rehydrated is { } r ? r : schema;
        if (node.ValueKind != JsonValueKind.Object)
        {
            return null;
        }
        if (node.TryGetProperty("dict", out var dict) && dict.ValueKind == JsonValueKind.Object &&
            dict.TryGetProperty(field, out var f))
        {
            return f;
        }
        return null;
    }

    /// <summary>该 (ns, 字段) 是否有尚未写回的用户改动，有则给出其值（否则返回 null）。
    ///
    /// 渲染时优先用待写回值：实测里切换「外观」会重建整个分区，若不优先取待写回值，
    /// 重建后的控件会退回内核快照里的旧值——用户刚改的东西"自己变回去了"（比不写回更误导）。</summary>
    private object? PendingValue(string ns, string field)
    {
        foreach (var e in _settingsEdits)
        {
            if (e.Ns == ns && e.Path.Length == 1 && e.Path[0] == field)
            {
                return e.Value;
            }
        }
        return null;
    }

    /// <summary>命名空间当前字符串值（用户待写回值优先 → 内核快照 → schema 默认）。</summary>
    private string NsString(string ns, string field, string fallback)
    {
        if (PendingValue(ns, field) is string pending)
        {
            return pending;
        }
        if (_settingsSnapshot is not null &&
            _settingsSnapshot.TryGetValue(ns, out var snap) &&
            snap.Value.TryGetProperty("value", out var v) && v.ValueKind == JsonValueKind.Object &&
            v.TryGetProperty(field, out var f) && f.ValueKind == JsonValueKind.String)
        {
            return f.GetString() ?? fallback;
        }
        return fallback;
    }

    private double NsNumber(string ns, string field, double fallback)
    {
        // 字号步进器登记的是 int，数值框登记的是 double：两种都认
        switch (PendingValue(ns, field))
        {
            case double d: return d;
            case int i: return i;
            case long l: return l;
        }
        if (_settingsSnapshot is not null &&
            _settingsSnapshot.TryGetValue(ns, out var snap) &&
            snap.Value.TryGetProperty("value", out var v) && v.ValueKind == JsonValueKind.Object &&
            v.TryGetProperty(field, out var f) && f.ValueKind == JsonValueKind.Number)
        {
            return f.GetDouble();
        }
        return fallback;
    }

    private bool NsBool(string ns, string field, bool fallback)
    {
        if (PendingValue(ns, field) is bool pending)
        {
            return pending;
        }
        if (_settingsSnapshot is not null &&
            _settingsSnapshot.TryGetValue(ns, out var snap) &&
            snap.Value.TryGetProperty("value", out var v) && v.ValueKind == JsonValueKind.Object &&
            v.TryGetProperty(field, out var f) && (f.ValueKind is JsonValueKind.True or JsonValueKind.False))
        {
            return f.GetBoolean();
        }
        return fallback;
    }

    /// <summary>
    /// 登记一条待写回编辑并自动保存（无保存/放弃页脚：改完即写回内核）。
    /// 值在登记当下求值；同一 (ns, 字段) 覆盖。防抖 400ms，避免连续滑杆/下拉打爆 RPC。
    /// </summary>
    private void Edit(string ns, string field, Func<object?> value)
    {
        var resolved = value();
        ConsumeShellSetting(ns, field, resolved);
        for (var i = 0; i < _settingsEdits.Count; i++)
        {
            if (_settingsEdits[i].Ns == ns && _settingsEdits[i].Path.Length == 1 && _settingsEdits[i].Path[0] == field)
            {
                // 替换对象而非原地改值：提交回执只能移除自己捕获的那一版。
                _settingsEdits[i] = new PendingEdit { Ns = ns, Path = new[] { field }, Value = resolved };
                ScheduleSettingsAutoSave();
                return;
            }
        }
        _settingsEdits.Add(new PendingEdit { Ns = ns, Path = new[] { field }, Value = resolved });
        ScheduleSettingsAutoSave();
    }

    private DispatcherTimer? _settingsAutoSaveTimer;

    private void ScheduleSettingsAutoSave()
    {
        _settingsAutoSaveTimer ??= new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(400) };
        _settingsAutoSaveTimer.Stop();
        _settingsAutoSaveTimer.Tick -= OnSettingsAutoSaveTick;
        _settingsAutoSaveTimer.Tick += OnSettingsAutoSaveTick;
        _settingsAutoSaveTimer.Start();
    }

    private async void OnSettingsAutoSaveTick(object? sender, object e)
    {
        _settingsAutoSaveTimer?.Stop();
        try
        {
            if (HasPendingSettings)
            {
                await SaveSettingsAsync();
            }
        }
        catch (Exception)
        {
            SetSettingsSaveError(L("自动保存失败，未保存的草稿已保留。请重试。"));
        }
    }

    private bool HasPendingSettings => _settingsEdits.Count > 0 || _pendingCredentialWrites.Count > 0;

    private void BindCredentialDraft(string refName, PasswordBox box)
    {
        if (_pendingCredentialWrites.TryGetValue(refName, out var draft))
        {
            box.Password = draft.Value;
        }
        _settingsCredentialBoxes[refName] = new WeakReference<PasswordBox>(box);
        box.PasswordChanged += (_, _) =>
        {
            if (_updatingCredentialBox) return;
            var value = box.Password.Trim();
            if (value.Length == 0)
            {
                // 留空只取消尚未提交的替换；删除已保存密钥仍须显式确认。
                _pendingCredentialWrites.Remove(refName);
            }
            else
            {
                _pendingCredentialWrites[refName] = new PendingCredentialWrite { Value = value };
            }
            ScheduleSettingsAutoSave();
        };
    }

    private void ClearCommittedCredentialBox(string refName)
    {
        if (!_settingsCredentialBoxes.TryGetValue(refName, out var weak) || !weak.TryGetTarget(out var box)) return;
        _updatingCredentialBox = true;
        try
        {
            box.Password = "";
            box.PlaceholderText = "已保存，输入新值可替换";
        }
        finally { _updatingCredentialBox = false; }
    }

    private void SetSettingsSaveError(string? message)
    {
        _settingsSaveError = message;
        if (_settingsSaveInfo is null) return;
        _settingsSaveInfo.Message = message ?? "";
        _settingsSaveInfo.IsOpen = message is not null;
    }

    private void RenderSettingsSaveStatus()
    {
        var retry = new Button { Content = L("重试保存") };
        retry.Click += async (_, _) =>
        {
            retry.IsEnabled = false;
            try { await SaveSettingsAsync(); }
            finally { retry.IsEnabled = true; }
        };
        _settingsSaveInfo = new InfoBar
        {
            Severity = InfoBarSeverity.Error,
            IsClosable = false,
            Title = L("设置尚未保存"),
            Message = _settingsSaveError ?? "",
            IsOpen = _settingsSaveError is not null,
            ActionButton = retry,
        };
        SettingsHost.Children.Insert(0, _settingsSaveInfo);
    }

    // ---- 控件工厂（保存时经 settings/mutate op:"set" path:[字段] 写回） ----

    /// <summary>枚举下拉（ComboBox）：选项值 = union const 字面值。
    /// readableName 用作 UIA 可读名（字段名是英文标识符，直接当 Name 对屏幕阅读器不可读）。</summary>
    private ComboBox MakeChoiceField(string ns, string field, string current, List<(string Value, string Label)> choices, string readableName = null)
    {
        var box = new ComboBox { MinWidth = TokenDouble("FieldMinWidth", 200), MaxDropDownHeight = 320 };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(box, $"Setting_{ns}_{field}");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, readableName ?? field);
        foreach (var (value, label) in choices)
        {
            var item = new ComboBoxItem { Content = label, Tag = value };
            box.Items.Add(item);
            if (value == current)
            {
                box.SelectedItem = item;
            }
        }
        // 初始赋值会触发 SelectionChanged：先挂"已就绪"标记，只有用户操作后的变化才登记
        var ready = false;
        box.SelectionChanged += (_, _) =>
        {
            if (ready)
            {
                Edit(ns, field, () => (box.SelectedItem as ComboBoxItem)?.Tag as string);
            }
        };
        box.Loaded += (_, _) => ready = true;
        return box;
    }

    /// <summary>界面语言下拉（壳本地设置，30 种）：写本地覆盖 + 刷新壳翻译，并把内核只认的 zh/en
    /// 映射回写 locale.preference（命令描述等内核文案跟随）。</summary>
    private ComboBox MakeUiLanguageField()
    {
        var box = new ComboBox { MinWidth = TokenDouble("FieldMinWidth", 200), MaxDropDownHeight = 420 };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(box, "Setting_locale_preference");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, L("语言"));
        foreach (var (id, label, _) in UiLanguages)
        {
            var item = new ComboBoxItem { Content = label, Tag = id };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(item, $"UiLanguage_{id}");
            box.Items.Add(item);
        }
        box.SelectedItem = box.Items.OfType<ComboBoxItem>()
            .FirstOrDefault(i => (string)i.Tag == _shellLocale) ?? box.Items.OfType<ComboBoxItem>().First();
        var ready = false;
        box.SelectionChanged += (_, _) =>
        {
            if (!ready) return;
            var id = (box.SelectedItem as ComboBoxItem)?.Tag as string ?? "zh";
            _uiLanguageOverride = id == "zh" ? null : id;
            SaveUiLanguageOverride(_uiLanguageOverride);
            if (_shellLocale != id)
            {
                _shellLocale = id;
                RefreshShellLanguage();
            }
            var kernelLocale = UiLanguages.First(l => l.Id == id).KernelLocale;
            Edit("locale", "preference", () => kernelLocale);
        };
        box.Loaded += (_, _) => ready = true;
        return box;
    }

    /// <summary>设置行输入控件的统一宽度（PasswordBox/TextBox/NumberBox 同档，右对齐成一列）。</summary>
    private static double FieldWidth => TokenDouble("FieldWidth", 320);

    /// <summary>数值输入（NumberBox 标准控件：步进钮 + spinner + 非法值自动复原）。</summary>
    private NumberBox MakeNumberBox(string ns, string field, double current)
    {
        var box = Aut(new NumberBox
        {
            Value = current,
            MinWidth = FieldWidth,
            MaxWidth = FieldWidth,
            SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Compact,
        }, $"Setting_{ns}_{field}", field);
        var ready = false;
        box.ValueChanged += (_, _) =>
        {
            if (ready)
            {
                Edit(ns, field, () => double.IsFinite(box.Value) ? box.Value : null);
            }
        };
        box.Loaded += (_, _) => ready = true;
        return box;
    }

    /// <summary>文本输入（TextBox 标准控件：宽度有界不随内容无限变宽）。</summary>
    private TextBox MakeTextBox(string ns, string field, string current, string? placeholder = null)
    {
        var box = Aut(new TextBox { Text = current, MinWidth = FieldWidth, MaxWidth = FieldWidth, PlaceholderText = placeholder ?? "" }, $"Setting_{ns}_{field}", field);
        box.TextChanged += (_, _) => Edit(ns, field, () => box.Text.Length > 0 ? box.Text : null);
        return box;
    }

    /// <summary>当前生效的 subagent-model-selection.allowedModels（待写回值优先 → 内核快照 → 空）。
    /// 待写回值以 {provider,model} 字典数组登记（JsonSerializer 直接产出内核要的形态）。</summary>
    private List<(string Provider, string Model)> NsAllowedModels()
    {
        const string ns = "subagent-model-selection";
        var result = new List<(string, string)>();
        if (PendingValue(ns, "allowedModels") is object[] draft)
        {
            foreach (var item in draft)
            {
                if (item is Dictionary<string, string> d &&
                    d.TryGetValue("provider", out var p) && d.TryGetValue("model", out var m))
                {
                    result.Add((p, m));
                }
            }
            return result;
        }
        if (_settingsSnapshot is not null &&
            _settingsSnapshot.TryGetValue(ns, out var snap) &&
            snap.Value.TryGetProperty("value", out var v) && v.ValueKind == JsonValueKind.Object &&
            v.TryGetProperty("allowedModels", out var list) && list.ValueKind == JsonValueKind.Array)
        {
            foreach (var item in list.EnumerateArray())
            {
                if (item.ValueKind != JsonValueKind.Object) continue;
                var p = item.TryGetProperty("provider", out var pe) ? pe.GetString() ?? "" : "";
                var m = item.TryGetProperty("model", out var me) ? me.GetString() ?? "" : "";
                if (p.Length > 0 || m.Length > 0)
                {
                    result.Add((p, m));
                }
            }
        }
        return result;
    }

    /// <summary>allowedModels 的待写回形态（{provider,model} 字典数组 → 内核 {provider,model} 对象数组）。</summary>
    private static object AllowedModelsDraft(List<(string Provider, string Model)> list)
        => list.Select(e => new Dictionary<string, string> { ["provider"] = e.Provider, ["model"] = e.Model }).ToArray();

    /// <summary>
    /// Subagent 模型选择开关（ns=subagent-model-selection）。
    ///
    /// **实测根因**（对内核 HTTP JSON RPC 直调，证据见 artifacts/kernel-settings-probe.json）：
    /// 该 ns 在 settings/describe 里是存在的、writable=true，schema =
    /// { enabled: boolean, allowedModels: [{provider,model}] }；但内核在写入时做耦合校验，
    /// 只写 enabled=true 会被拒绝：
    ///   settings/rejected "enabled subagent model selection requires at least one allowed model"
    /// 旧实现只登记 path:["enabled"]，于是保存必然抛错 → 开关看起来"点了不生效、重开就回退"。
    ///
    /// 修法：开启时同时给出至少一个允许模型——不凭空造模型名，种子取当前默认模型
    /// （agent-default-model 的 provider/model）；已配置过 allowedModels 时原样保留。
    /// 拿不到默认模型时不猜，直接交给内核报错并原样透出（不误导）。
    /// </summary>
    private ToggleSwitch MakeSubagentToggle()
    {
        const string ns = "subagent-model-selection";
        var toggle = Aut(new ToggleSwitch { IsOn = NsBool(ns, "enabled", false), OnContent = L("开"), OffContent = L("关"), Style = AppStyle("SettingsToggleSwitchStyle") },
            $"Setting_{ns}_enabled", L("允许 Agent 为 Subagent 选择模型"));
        toggle.Toggled += (_, _) =>
        {
            var on = toggle.IsOn;
            Edit(ns, "enabled", () => on);
            if (!on || NsAllowedModels().Count > 0)
            {
                return; // 关闭无需种子；已有允许模型也无需改写
            }
            var provider = NsString("agent-default-model", "provider", "");
            var model = NsString("agent-default-model", "model", "");
            if (provider.Length == 0 || model.Length == 0)
            {
                return; // 拿不到默认模型时不编造，让内核的拒绝原因在保存结果里原样可见
            }
            Edit(ns, "allowedModels", () => AllowedModelsDraft(new List<(string, string)> { (provider, model) }));
        };
        return toggle;
    }

    /// <summary>外观选择（浅色/深色/跟随系统）：ComboBox 下拉。
    /// 选择即时换肤（壳不等保存），保存时再把偏好持久化到内核；
    /// 打开设置页时选中态 = 壳当前生效偏好（不是快照默认值）。</summary>
    private ComboBox MakeThemeChoice(string current)
    {
        var active = _shellPreference is "light" or "dark" or "system" ? _shellPreference : current;
        var box = new ComboBox { MinWidth = TokenDouble("FieldMinWidth", 200) };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(box, "Setting_ui-theme_preference");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, "主题");
        foreach (var (id, label) in new[] { ("light", "浅色"), ("dark", "深色"), ("system", "跟随系统") })
        {
            box.Items.Add(new ComboBoxItem { Content = label, Tag = id });
            if (id == active)
            {
                box.SelectedItem = box.Items[^1];
            }
        }
        // 先赋初值后挂事件：初始 SelectionChanged 不落这里
        box.SelectionChanged += (_, _) =>
        {
            if ((box.SelectedItem as ComboBoxItem)?.Tag is not string id)
            {
                return;
            }
            ApplyShellTheme(id);
            Edit("ui-theme", "preference", () => id);
        };
        return box;
    }

    /// <summary>字号步进器（−/值px/+，12–17）。
    /// ± 即时应用到聊天流（ChatList.FontSize 继承到气泡文本），保存时持久化到内核。</summary>
    private StackPanel MakeFontSizeStepper(double current)
    {
        var panel = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6 };
        var value = (int)current;
        var display = new TextBlock { Text = $"{value}px", VerticalAlignment = VerticalAlignment.Center, MinWidth = 44, TextAlignment = TextAlignment.Center };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(display, "FontSizeValue");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(display, LF("当前字号 {0}px", $"{value}"));
        var minus = new Button { Content = "−", Style = AppStyle("CompactButtonStyle") };
        var plus = new Button { Content = "＋", Style = AppStyle("CompactButtonStyle") };
        // 符号按钮（−/＋）单独读出来无意义，补足动作语义
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(minus, "FontSizeDecreaseButton");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(minus, L("减小字号"));
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(plus, "FontSizeIncreaseButton");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(plus, L("增大字号"));
        void Step(int delta)
        {
            value = Math.Clamp(value + delta, 12, 17);
            display.Text = $"{value}px";
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(display, LF("当前字号 {0}px", $"{value}"));
            ChatList.FontSize = value;
            Edit("ui-theme", "fontSize", () => value);
        }
        minus.Click += (_, _) => Step(-1);
        plus.Click += (_, _) => Step(1);
        panel.Children.Add(minus);
        panel.Children.Add(display);
        panel.Children.Add(plus);
        return panel;
    }

    // ---- 分区渲染 ----

    /// <summary>渲染一个设置分区。
    ///
    /// keepPendingEdits 参数保留以兼容现有调用；所有渲染都保留内存草稿，
    /// 包括快速切页、换主题和插件 Tab。只有成功提交/显式恢复默认才移除对应版本。
    ///
    /// 渲染前先确保设置快照可用：SaveSettingsAsync 成功后会置空快照（"下次重拉"），
    /// 旧实现里 RenderSectionAsync 不重拉，于是保存后切分区渲染出来的是**schema 默认值**
    /// 而不是内核实际值（实测：保存后 plugins 页显示 timeoutMs=120000，内核里是 150000），
    /// 且 permission 卡片因快照为空整块不渲染。</summary>
    private async Task RenderSectionAsync(string id, bool keepPendingEdits = false)
    {
        // 0xc000027b 兜底：分区渲染内部任何异常（JsonElement 形态变化、XAML 构造失败）
        // 绝不能沿 async void 事件链上抛成 stowed exception（Debug 数据家形态恰好掩盖过此路径）。
        try
        {
            // 密码框是按 refName 重建的弱引用视图：不在此处清空 _pendingCredentialWrites，
            // 否则快切页会丢掉已输入、尚未提交的密钥。
            await EnsureSettingsSnapshotAsync();
            // 记忆分区的监听随分区卸载：先把未保存的修改落盘（防抖可能还没到点），
            // 再停 watcher / 计时器，避免后台事件写已丢弃的控件树。
            FlushMemoryPendingSave();
            // 自定义指令同理：切分区前把防抖没到点的草稿落盘，再停计时器、解控件引用。
            FlushInstructionsPendingSave();
            UnloadInstructionsCard();
            _memoryWatcher?.Dispose();
            _memoryWatcher = null;
            // 停计时器必须置空（同 UnloadInstructionsCard）：留着已停的旧计时器引用，
            // 将来 start 处加守卫就会跨分区复用坏掉的计时器，防抖静默失效 = 自动保存失败。
            _memorySaveTimer?.Stop();
            _memorySaveTimer = null;
            _memoryRefreshTimer?.Stop();
            _memoryRefreshTimer = null;
            _memoryRawBox = null;
            _memorySaveState = null;
            _generalShellRegions.Clear();
            ResetSettingsSubPages(); // 换分区 = 回到一级页：上一个分区的二级页栈不得带过来
            SettingsHost.Children.Clear();
            RenderSettingsSaveStatus();
            UpdateSettingsBreadcrumb(); // 固定页头（Title 层级，滚动区外驻留）
            switch (id)
            {
                case "general":
                    RenderGeneralSection();
                    break;
                case PersonalizationSectionId:
                    RenderPersonalizationSection();
                    break;
                case "models":
                    await RenderModelsSectionAsync();
                    break;
                case "plugins":
                    await RenderPluginsSectionAsync();
                    break;
                case SkillsSectionId:
                    await RenderSkillsSectionAsync();
                    break;
                case "agent-presets":
                    await RenderPresetsSectionAsync();
                    break;
                case MemorySectionId:
                    RenderMemorySection();
                    break;
                case ComputerControlSectionId:
                    RenderComputerControlSection();
                    break;
                case BrowserControlSectionId:
                    RenderBrowserControlSection();
                    break;
                case UsageSectionId:
                    await RenderUsageSectionAsync();
                    break;
                case PetSectionId:
                    RenderPetSection();
                    break;
                case AboutSectionId:
                    RenderAboutSection();
                    break;
            }
            // 「使用统计」「记忆」「电脑控制」「浏览器控制」「关于」「个性化」是壳内建分区：没有待写回字段。
            // 已去掉「放弃修改/保存」页脚——设置改动由 Edit() 防抖自动写回。
            // 设置文件维护卡只挂「通用」一张。
            if (!ShellBuiltinSections.Contains(id))
            {
                if (id == "general")
                {
                    RenderSettingsMaintenanceCard(id);
                }
                if (_settingsEdits.Count > 0)
                {
                    MarkSettingsDirty();
                }
            }
            // 整棵设置树进本地化扫描区：各分区（含维护卡/预设页/统计页）动态建卡，
            // 逐段登记会漏后渲染的区；LocalizeProperty 只翻字典键命中的精确串，内核数据不受影响。
            _generalShellRegions.Add(SettingsHost);
            AlignSettingsHeader();
        }
        catch (Exception ex)
        {
            SettingsHost.Children.Clear();
            SettingsHost.Children.Add(new TextBlock
            {
                Text = LF("该分区加载失败：{0}", ex.Message),
                Foreground = ThemeBrush("TextSecondaryBrush"),
                TextWrapping = TextWrapping.Wrap,
            });
            System.Diagnostics.Debug.WriteLine($"[settings/{id}] {ex}");
            try
            {
                File.AppendAllText(
                    Path.Combine(Path.GetTempPath(), "blade2_settings_error.txt"),
                    $"[{DateTime.Now:O}] section={id}\n{ex}\n\n");
            }
            catch (Exception)
            {
                // 诊断落盘失败不影响 UI
            }
        }
    }

    /// <summary>分区 → 该页承载的设置命名空间（"恢复本页默认" 的 settings/replace 目标）。
    /// 对照各分区渲染里实际读写的 ns：未列出的 ns 不受影响。
    /// 「插件」页的判定模型卡写的是 agent-default-model，但故意不列进来：那是全局默认模型
    /// （输入区模型选择器也落这里），「恢复插件页默认」把它一起清掉会波及所有新会话，
    /// 超出本页默认的含义。要重置默认模型请去「模型」页。</summary>
    private static readonly Dictionary<string, string[]> SectionNamespaces = new(StringComparer.Ordinal)
    {
        ["general"] = new[] { "ui-theme", "locale", "ui-chat", "ui-conversation", "permission" },
        ["models"] = new[] { "agent-default-model", "llm-deepseek", "llm-pi-ai" },
        ["plugins"] = new[] { "shell", "agent-loop", "subagent-model-selection", "web-search-deepseek" },
        ["agent-presets"] = new[] { "agent-presets" },
    };

    /// <summary>设置文件维护卡（每个分区末尾）：
    /// settings/openSettingsDocument（无参数 → {opened:true}）与 settings/replace（清空本页用户段）。</summary>
    private void RenderSettingsMaintenanceCard(string sectionId)
    {
        if (!SectionNamespaces.TryGetValue(sectionId, out var namespaces))
        {
            return;
        }
        var card = NewCard("设置文件");

        var open = new Button { Content = "打开设置文档", Style = AppStyle("CompactButtonStyle") };
        ToolTipService.SetToolTip(open, "在内核主机上用默认文本编辑器打开 settings 文档（settings/openSettingsDocument）");
        Aut(open, "OpenSettingsDocumentButton", "打开设置文档");
        open.Click += (_, _) => _ = OpenSettingsDocumentAsync();
        card.Children.Add(MakeRow(
            "设置文档",
            "把内核侧设置文档落到本地并用默认编辑器打开。",
            open));

        AddDivider(card);
        var reset = new Button { Content = "恢复本页默认", Style = AppStyle("CompactButtonStyle") };
        ToolTipService.SetToolTip(reset, "清空本页命名空间的用户设置段（settings/replace），回到内核默认值");
        Aut(reset, "ResetSettingsSectionButton", "恢复本页默认");
        reset.Click += (_, _) => _ = ResetSectionAsync(sectionId, namespaces);
        card.Children.Add(MakeRow(
            "恢复默认",
            LF("清空本页用户设置：{0}。", string.Join("、", namespaces)),
            reset));
    }

    /// <summary>打开设置文档（settings/openSettingsDocument：无参数 → {opened:true}）。
    /// 会在内核主机拉起默认编辑器窗口——只由用户点击触发。</summary>
    private async Task OpenSettingsDocumentAsync()
    {
        if (_rpc is null)
        {
            return;
        }
        try
        {
            await _rpc.CallOkAsync("settings/openSettingsDocument", new { });
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("打开设置文档失败：{0}", ex.Message));
        }
    }

    /// <summary>恢复本页默认：对每个命名空间调用 settings/replace（section = 空对象 = 清空用户段）。
    /// expectedRevision 只在快照里有 revision 时才带（描述符是 z.union([undefined, number])，
    /// 显式 null 会被 strict codec 判为非法）。</summary>
    private async Task ResetSectionAsync(string sectionId, string[] namespaces)
    {
        if (_rpc is null)
        {
            return;
        }
        var dialog = new ContentDialog
        {
            Title = LF("恢复「{0}」默认", SectionTitle(sectionId)),
            Content = new TextBlock
            {
                Text = LF("将清空这些命名空间的用户设置：{0}。", string.Join("、", namespaces))
                    + "\n" + L("内核默认值会立即生效，此操作不可撤销。"),
                TextWrapping = TextWrapping.Wrap,
            },
            PrimaryButtonText = L("恢复"),
            CloseButtonText = L("取消"),
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = Content.XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }
        // 仅撤销确认时已有的本页草稿；等待提交锁期间的新编辑必须保留。
        var discardedEdits = _settingsEdits.Where(e => namespaces.Contains(e.Ns)).ToArray();
        await _settingsCommitGate.WaitAsync();
        try
        {
            foreach (var ns in namespaces)
            {
                double? revision = _settingsSnapshot is not null && _settingsSnapshot.TryGetValue(ns, out var snap)
                    ? snap.Revision
                    : null;
                object args = revision is { } rev
                    ? new { ns, section = new { }, expectedRevision = rev }
                    : new { ns, section = new { } };
                await _rpc.CallOkAsync("settings/replace", args);
                _settingsSnapshot = null;
                foreach (var entry in discardedEdits.Where(e => e.Ns == ns)) _settingsEdits.Remove(entry);
            }
            if (_settingsActiveSection == sectionId) await RenderSectionAsync(sectionId);
        }
        catch (Exception)
        {
            SetSettingsSaveError(L("恢复默认未完成，未成功的草稿已保留。请检查连接后重试恢复默认。"));
        }
        finally
        {
            _settingsCommitGate.Release();
        }
    }

    /// <summary>通用区（按内核 general.item 注册顺序：
    /// permission(-20) → appearance(10) → font-size(11) → transcript-view(12) → composer-enter(20) → locale）。</summary>
    private void RenderGeneralSection()
    {
        SettingsHost.Children.Add(MakeSectionDesc("新会话的默认权限、外观与对话偏好。"));

        // 权限（defaultPreset 枚举从 schema union 动态读，标签优先用产品中文文案）
        if (_settingsSnapshot is not null && _settingsSnapshot.TryGetValue("permission", out var perm) &&
            perm.Value.TryGetProperty("schema", out var permSchema))
        {
            var choices = UnionChoices(SchemaField(permSchema, "defaultPreset"));
            if (choices.Count > 0)
            {
                var zh = new Dictionary<string, string>
                {
                    ["read-only"] = "仅可查看",
                    ["workspace-write"] = "工作区内修改",
                    ["danger-full-access"] = "完全权限",
                };
                var current = NsString("permission", "defaultPreset", "workspace-write");
                var box = MakeChoiceField("permission", "defaultPreset", current,
                    choices.Select(c => (c.Value, zh.GetValueOrDefault(c.Value, c.Label))).ToList(), "默认权限模式");
                box.MinWidth = TokenDouble("FieldMinWidth", 200);
                var card = NewCard("权限", "选择新会话的默认访问模式");
                card.Children.Add(MakeRow("默认权限", "仅可查看 / 工作区内修改 / 完全权限", box));
            }
        }

        // 外观 + 字号
        var uiCard = NewCard("外观", "主题与会话正文字号");
        uiCard.Children.Add(MakeRow("主题", "浅色 / 深色 / 跟随系统",
            MakeThemeChoice(NsString("ui-theme", "preference", "system"))));
        AddDivider(uiCard);
        uiCard.Children.Add(MakeRow("字号大小", "仅影响会话内容的字号", MakeFontSizeStepper(NsNumber("ui-theme", "fontSize", 14))));

        // 对话显示与发送行为
        var chatCard = NewCard("对话", "已完成轮次的展示方式与繁忙时的发送行为");
        chatCard.Children.Add(MakeRow(
            "对话显示",
            "标准显示过程内容；紧凑只保留结果",
            MakeChoiceField("ui-chat", "transcriptView", NsString("ui-chat", "transcriptView", "compact"),
                new List<(string, string)> { ("normal", "标准"), ("compact", "紧凑") }, "对话显示")));
        AddDivider(chatCard);
        chatCard.Children.Add(MakeRow(
            "繁忙时的发送行为",
            "智能体运行时 Enter 键和发送按钮的行为；Ctrl+Enter 使用另一行为",
            MakeChoiceField("ui-conversation", "busyEnter", NsString("ui-conversation", "busyEnter", "queue"),
                new List<(string, string)> { ("queue", "排队发送"), ("steer", "插话发送") }, "繁忙时的发送行为")));

        // 语言
        var localeCard = NewCard("语言", "界面显示语言");
        localeCard.Children.Add(MakeRow(
            "语言",
            null,
            MakeUiLanguageField()));

        // 背景皮肤已移到「个性化」分区（见 MainWindow.Personalization.cs）

        // 系统通知（壳本地 shell.json，不进内核）
        RenderNotificationCard();

        // 托盘与退出（壳本地 shell.json，不进内核）
        RenderTrayCard();
        RefreshShellLanguage();
    }

    // ---------------- 设置：模型页（提供方卡 + 添加/编辑流） ----------------
    //
    // 对照内核官方设置页（@deepseek-ai/dsh-client-ui-settings-models）重排：
    //   · 每个已解析设置地址的提供方一张行卡：显示名 + 凭据状态点（绿=已配置 / 红=缺失）+
    //     「自定义」标记 + 编辑（用户添加的可整行删除）；编辑面板内嵌在行卡里。
    //   · 「添加提供方」= 虚线按钮 → 展开一张添加卡：提供方下拉（休眠目录项 + 自定义入口；
    //     参考页拆成两张添加卡，产品决策在壳里并成一个下拉）+ API 密钥 + 自定义设置
    //     （API 地址 / API 协议 / 模型目录）+ 取消/保存。
    //   · 保存 = settings/mutate 的最小 path ops（只动卡片看得见的字段）+ credentials/set；
    //     删除 = 先清页面托管的凭据（credentials/unset）再 unset 该提供方子树，两步都幂等。
    //   数据源：llm/listConfigurableProviders（目录）× settings/describe（value/user/base/schema）
    //   × credentials/describe；密钥不落设置文档，只落凭据库。

    /// <summary>模型区渲染入口：行卡 + 添加块 + 默认模型卡。</summary>
    private async Task RenderModelsSectionAsync()
    {
        if (_rpc is null)
        {
            return;
        }
        SettingsHost.Children.Add(MakeSectionDesc("填入各提供方的 API 密钥即可使用其模型。"));

        var rows = await LoadProviderRowsAsync();

        if (_modelsSavedNotice is { Length: > 0 } saved)
        {
            _modelsSavedNotice = null; // 一次性回执：本次渲染显示，换页回来不再出现
            SettingsHost.Children.Add(Aut(new TextBlock
            {
                Text = saved,
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("SuccessBrush"),
            }, "ModelsSavedNotice", saved));
        }

        foreach (var row in rows.Where(r => r.Configured))
        {
            SettingsHost.Children.Add(MakeProviderCard(row, rows));
        }

        if (_modelsEditor is { } editor && _modelsAdding)
        {
            SettingsHost.Children.Add(MakeModelsAddCard(editor, rows));
        }
        else if (_modelsEditor is null)
        {
            SettingsHost.Children.Add(MakeAddProviderButton(rows));
        }
        // 行卡内编辑态（_modelsEditor 非 null 且 !_modelsAdding）已内嵌在对应行卡里，不再铺添加按钮。
        // 默认模型卡（agent-default-model）已移除：模型在会话内选择，提供方配置以本页行卡为准。
    }

    /// <summary>联接提供方目录、设置视图与凭据状态（对照参考页 store.load）：
    /// 目录条目逐一解析设置地址命中（configured）、可移除位（user 命中且 base 没有）、
    /// 点名凭据引用与派生引用；随后一次批量 credentials/describe。</summary>
    private async Task<List<ProviderRow>> LoadProviderRowsAsync()
    {
        var rows = new List<ProviderRow>();
        var entries = new List<(string Provider, string DisplayName, string Ns, string[] Path, bool? Declared, string? Error)>();
        try
        {
            foreach (var p in (await _rpc!.CallOkAsync("llm/listConfigurableProviders", new { })).EnumerateArray())
            {
                var provider = p.TryGetProperty("provider", out var pv) ? pv.GetString() ?? "" : "";
                if (provider.Length == 0)
                {
                    continue;
                }
                var display = p.TryGetProperty("displayName", out var dn) && dn.ValueKind == JsonValueKind.String ? dn.GetString() ?? provider : provider;
                var ns = p.TryGetProperty("settingsNs", out var sns) ? sns.GetString() ?? "" : "";
                var segments = new List<string>();
                if (p.TryGetProperty("settingsPath", out var sp) && sp.ValueKind == JsonValueKind.Array)
                {
                    foreach (var seg in sp.EnumerateArray())
                    {
                        if (seg.ValueKind == JsonValueKind.String)
                        {
                            segments.Add(seg.GetString() ?? "");
                        }
                    }
                }
                bool? declared = null;
                if (p.TryGetProperty("declared", out var dc))
                {
                    declared = dc.ValueKind == JsonValueKind.True;
                }
                var error = p.TryGetProperty("error", out var err) && err.ValueKind == JsonValueKind.String ? err.GetString() : null;
                entries.Add((provider, display, ns, segments.ToArray(), declared, error));
            }
        }
        catch (DshRpcException ex)
        {
            SettingsHost.Children.Add(new TextBlock
            {
                Text = LF("读取可配置提供方失败：{0}", ex.Message),
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextSecondaryBrush"),
                TextWrapping = TextWrapping.Wrap,
            });
        }

        var snapshot = await EnsureSettingsSnapshotAsync();
        foreach (var (provider, display, ns, path, declared, error) in entries)
        {
            var row = new ProviderRow
            {
                Provider = provider,
                DisplayName = display,
                SettingsNs = ns,
                SettingsPath = path,
                Declared = declared == true,
                Error = error,
            };
            if (ns.Length > 0 && snapshot is not null && snapshot.TryGetValue(ns, out var view))
            {
                var value = view.Value.TryGetProperty("value", out var v) ? v : default;
                var user = view.Value.TryGetProperty("user", out var u) ? u : default;
                var baseLayer = view.Value.TryGetProperty("base", out var b) ? b : default;
                row.Value = SettingsValueAt(value, path);
                row.User = SettingsValueAt(user, path);
                row.Base = SettingsValueAt(baseLayer, path);
                row.Configured = path.Length == 0 || row.Value is not null;
                row.Removable = path.Length > 0 && row.User is not null && row.Base is null;
                if (view.Value.TryGetProperty("schema", out var schema))
                {
                    row.SchemaRoot = RehydrateSchema(schema) is { } root ? root : schema;
                }
                row.ApiKeyEnv = StringAt(row.Value, "apiKeyEnv");
            }
            row.KeyRef = row.ApiKeyEnv is { Length: > 0 } named ? named : DeriveKeyRef(provider);
            rows.Add(row);
        }

        var refs = rows.Select(r => r.KeyRef).Where(r => r.Length > 0).Distinct().ToList();
        if (refs.Count > 0)
        {
            try
            {
                var described = await _rpc!.CallOkAsync("credentials/describe", new { refs });
                if (described.ValueKind == JsonValueKind.Object)
                {
                    foreach (var row in rows)
                    {
                        if (described.TryGetProperty(row.KeyRef, out var state))
                        {
                            row.Credential = state;
                            row.HasCredential = true;
                        }
                    }
                }
            }
            catch (Exception)
            {
                // 凭据读取失败不阻塞列表：行卡退化为无状态点，编辑卡退化为普通占位
            }
        }
        return rows;
    }

    /// <summary>一张提供方行卡：显示名 + 凭据状态点 +（自定义标记）+ 编辑/删除；编辑态内嵌面板。</summary>
    private FrameworkElement MakeProviderCard(ProviderRow row, List<ProviderRow> rows)
    {
        var body = new StackPanel { Spacing = Sp12 };
        var shell = CardShell();
        shell.Child = body;
        shell.Padding = new Thickness(Sp16, Sp12, Sp16, Sp12);

        var head = new Grid { ColumnSpacing = Sp10 };
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var identity = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = Sp6,
            VerticalAlignment = VerticalAlignment.Center,
        };
        identity.Children.Add(new TextBlock
        {
            Text = row.DisplayName,
            Style = AppStyle("BodyStrongTextStyle"),
            VerticalAlignment = VerticalAlignment.Center,
        });
        if (row.Declared)
        {
            identity.Children.Add(MakeProviderTag(L("自定义")));
        }
        // 状态点：profile 点名的凭据已配置 = 绿 / 已知缺失 = 红；未点名（环境认证）不显示
        if (row.ApiKeyEnv is { Length: > 0 } && row.HasCredential)
        {
            var configured = row.Credential.TryGetProperty("configured", out var cfg) && cfg.ValueKind == JsonValueKind.True;
            var dotText = configured ? L("API 密钥已配置") : L("API 密钥缺失");
            var dot = Aut(new Microsoft.UI.Xaml.Shapes.Ellipse
            {
                Width = 8,
                Height = 8,
                VerticalAlignment = VerticalAlignment.Center,
                Fill = configured ? ThemeBrush("SuccessBrush") : ThemeBrush("ErrorBrush"),
            }, configured ? "ProviderKeyConfiguredDot" : "ProviderKeyMissingDot", dotText);
            ToolTipService.SetToolTip(dot, dotText);
            identity.Children.Add(dot);
        }
        Grid.SetColumn(identity, 0);
        head.Children.Add(identity);

        var actions = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = Sp4,
            VerticalAlignment = VerticalAlignment.Center,
        };
        bool IsOpen() => _modelsEditor is { } current && !_modelsAdding &&
            current.Provider == row.Provider && current.SettingsNs == row.SettingsNs;
        var edit = new Button { Content = L("编辑"), Style = AppStyle("CompactButtonStyle") };
        Aut(edit, $"ProviderEdit_{row.Provider}", LF("编辑 {0}", row.DisplayName));
        edit.Click += (_, _) =>
        {
            if (IsOpen())
            {
                CloseModelsEditor();
            }
            else
            {
                OpenModelsEditor(row);
            }
            _ = RenderSectionAsync("models");
        };
        actions.Children.Add(edit);
        if (row.Removable)
        {
            var remove = new Button
            {
                Content = L("删除"),
                Style = AppStyle("CompactButtonStyle"),
                Foreground = ThemeBrush("ErrorBrush"),
            };
            Aut(remove, $"ProviderRemove_{row.Provider}", LF("删除 {0}", row.DisplayName));
            remove.Click += (_, _) => _ = RemoveProviderAsync(row);
            actions.Children.Add(remove);
        }
        Grid.SetColumn(actions, 1);
        head.Children.Add(actions);
        body.Children.Add(head);

        if (row.Error is { Length: > 0 } errorText)
        {
            body.Children.Add(new TextBlock
            {
                Text = errorText,
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("ErrorBrush"),
                TextWrapping = TextWrapping.Wrap,
            });
        }

        if (IsOpen() && _modelsEditor is { } editor)
        {
            AddDivider(body);
            body.Children.Add(MakeProviderEditorPanel(editor, row));
        }
        return shell;
    }

    /// <summary>「自定义」小徽标（参考页 rowTag：细描边圆角小签）。</summary>
    private FrameworkElement MakeProviderTag(string text)
        => new Border
        {
            BorderBrush = ThemeBrush("StrokeBrush"),
            BorderThickness = Stroke1,
            CornerRadius = RadSmall,
            Padding = new Thickness(6, 1, 6, 1),
            VerticalAlignment = VerticalAlignment.Center,
            Child = new TextBlock
            {
                Text = text,
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextSecondaryBrush"),
            },
        };

    /// <summary>虚线「添加提供方」按钮（参考页 addButton 的 WinUI 对应物）：
    /// 默认按钮承担交互与焦点（BorderThickness=0 防双框），虚线框用 pointer-transparent 的
    /// Rectangle 画在其上；悬停/按压仍走按钮模板的标准状态底色，虚线始终可见。</summary>
    private FrameworkElement MakeAddProviderButton(List<ProviderRow> rows)
    {
        var addable = rows.Where(r => r.SettingsNs.Length > 0 && !r.Configured).ToList();
        var bareStyle = new Style(typeof(Button));
        bareStyle.Setters.Add(new Setter(Button.BackgroundProperty, new SolidColorBrush(Microsoft.UI.Colors.Transparent)));
        bareStyle.Setters.Add(new Setter(Button.BorderThicknessProperty, new Thickness(0)));
        bareStyle.Setters.Add(new Setter(Button.PaddingProperty, new Thickness(0)));
        var button = new Button
        {
            Style = bareStyle,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Center,
            VerticalContentAlignment = VerticalAlignment.Center,
            MinHeight = 44,
        };
        var content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6, IsHitTestVisible = false };
        content.Children.Add(new FontIcon { Glyph = "\uE710", FontSize = GlyphBody });
        content.Children.Add(new TextBlock { Text = L("添加提供方"), VerticalAlignment = VerticalAlignment.Center });
        button.Content = content;
        Aut(button, "AddProviderButton", L("添加提供方"));
        button.IsEnabled = addable.Count > 0;
        ToolTipService.SetToolTip(button, addable.Count > 0 ? null : L("目录里没有可添加的提供方"));
        button.Click += (_, _) =>
        {
            OpenModelsEditor(addable[0], adding: true);
            _ = RenderSectionAsync("models");
        };
        var outline = new Microsoft.UI.Xaml.Shapes.Rectangle
        {
            Stroke = ThemeBrush("StrokeBrush"),
            StrokeThickness = 1,
            StrokeDashArray = new DoubleCollection { 2, 2 },
            RadiusX = 8,
            RadiusY = 8,
            IsHitTestVisible = false,
        };
        return new Grid { Children = { button, outline } };
    }

    /// <summary>「添加提供方」卡：提供方下拉（休眠目录项 + 自定义入口）+ 编辑面板。
    /// 切换目标重铺整卡：草稿随目标切换丢弃（参考页 select 的 setEditing 语义）。</summary>
    private FrameworkElement MakeModelsAddCard(ModelsEditorState editor, List<ProviderRow> rows)
    {
        var body = new StackPanel { Spacing = Sp12 };
        var shell = CardShell();
        shell.Child = body;
        shell.Padding = new Thickness(Sp16, Sp12, Sp16, Sp12);

        var addable = rows.Where(r => r.SettingsNs.Length > 0 && !r.Configured).ToList();
        var providerBox = new ComboBox
        {
            MinWidth = TokenDouble("FieldMinWidth", 200),
            MaxDropDownHeight = 320,
            HorizontalAlignment = HorizontalAlignment.Left,
        };
        Aut(providerBox, "ModelsAddProviderSelect", L("提供方"));
        foreach (var candidate in addable)
        {
            providerBox.Items.Add(new ComboBoxItem { Content = candidate.DisplayName, Tag = candidate.Provider });
            if (!editor.IsCustom && editor.Provider == candidate.Provider)
            {
                providerBox.SelectedItem = providerBox.Items[^1];
            }
        }
        providerBox.Items.Add(new ComboBoxItem { Content = L("自定义"), Tag = ModelsCustomTag });
        if (editor.IsCustom)
        {
            providerBox.SelectedItem = providerBox.Items[^1];
        }
        // 先赋初值后挂事件：初始 SelectionChanged 不落这里
        providerBox.SelectionChanged += (_, _) =>
        {
            var tag = (providerBox.SelectedItem as ComboBoxItem)?.Tag as string;
            if (string.IsNullOrEmpty(tag))
            {
                return;
            }
            if (tag == ModelsCustomTag)
            {
                if (_modelsEditor is { IsCustom: true })
                {
                    return;
                }
                _modelsEditor = NewCustomEditorState(rows);
            }
            else
            {
                var picked = addable.FirstOrDefault(r => r.Provider == tag);
                if (picked is null ||
                    (_modelsEditor is { IsCustom: false } current && current.Provider == picked.Provider))
                {
                    return;
                }
                OpenModelsEditor(picked, adding: true);
            }
            _ = RenderSectionAsync("models");
        };
        body.Children.Add(new StackPanel
        {
            Spacing = Sp4,
            Children =
            {
                new TextBlock { Text = L("提供方"), Style = AppStyle("FieldLabelTextStyle") },
                providerBox,
            },
        });
        body.Children.Add(MakeProviderEditorPanel(editor, null));
        return shell;
    }

    /// <summary>编辑面板（行卡内嵌 / 添加卡共用）：API 密钥 + 自定义设置 Expander + 取消/保存。
    /// row = null 表示「自定义提供方」创建流：Provider ID / 显示名称 / API 协议平铺（必填），
    /// 模型目录必填 ≥1 行；已有提供方的流里可选字段收进「自定义设置」Expander（参考页布局）。</summary>
    private FrameworkElement MakeProviderEditorPanel(ModelsEditorState editor, ProviderRow? row)
    {
        var panel = new StackPanel { Spacing = Sp12 };

        if (editor.IsCustom)
        {
            panel.Children.Add(new TextBlock
            {
                Text = L("自定义提供方"),
                Style = AppStyle("BodyStrongTextStyle"),
            });
        }

        TextBlock Hint(string text, bool error = false) => new()
        {
            Text = text,
            Style = AppStyle("CaptionTextStyle"),
            Foreground = error ? ThemeBrush("ErrorBrush") : ThemeBrush("TextTertiaryBrush"),
            TextWrapping = TextWrapping.Wrap,
        };
        void Field(string label, FrameworkElement control)
        {
            panel.Children.Add(new StackPanel
            {
                Spacing = Sp4,
                Children =
                {
                    new TextBlock { Text = label, Style = AppStyle("FieldLabelTextStyle") },
                    control,
                },
            });
        }

        // —— 尾区控件先建：UpdateSaveState 在下方字段 lambda 里被调用，C# 要求局部函数
        //     的捕获目标在调用点已明确赋值；布局顺序仍由下方 panel.Children.Add 次序决定 ——
        var keyHint = Hint("", error: true);
        keyHint.Visibility = Visibility.Collapsed;
        var modelHint = Hint("", error: true);
        modelHint.Visibility = Visibility.Collapsed;
        var failureHint = Hint(editor.Failure ?? "", error: true);
        failureHint.Visibility = string.IsNullOrEmpty(editor.Failure) ? Visibility.Collapsed : Visibility.Visible;
        var actions = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = Sp8,
            HorizontalAlignment = HorizontalAlignment.Right,
        };
        var cancel = new Button { Content = L("取消"), Style = AppStyle("CompactButtonStyle") };
        Aut(cancel, editor.IsCustom ? "ModelsCustomCancel" : "ModelsEditorCancel", L("取消"));
        cancel.Click += (_, _) =>
        {
            CloseModelsEditor();
            _ = RenderSectionAsync("models");
        };
        actions.Children.Add(cancel);
        var save = new Button
        {
            Style = AppStyle("AccentButtonStyle"),
            MinWidth = 96,
        };
        Aut(save, editor.IsCustom ? "ModelsCustomSave" : "ModelsEditorSave", editor.IsCustom ? L("创建提供方") : L("保存"));
        save.Content = editor.IsCustom ? L("创建提供方") : L("保存");
        save.Click += (_, _) => _ = ApplyModelsEditorAsync(editor, row);
        actions.Children.Add(save);

        // —— 自定义流：Provider ID（route 即凭据名前缀，实时校验查重） ——
        if (editor.IsCustom)
        {
            var routeHint = Hint(L("以小写字母开头的标识，在请求中唯一标识该提供方，并用于派生凭据名。"));
            var routeBox = Aut(new TextBox { Text = editor.Route, PlaceholderText = "acme-gateway" }, "ModelsCustomRoute", L("Provider ID"));
            routeBox.TextChanged += (_, _) =>
            {
                editor.Route = routeBox.Text;
                editor.SettingsPath = new[] { "providers", editor.Route.Trim() };
                var route = editor.Route.Trim();
                var invalid = route.Length > 0 && !RoutePattern.IsMatch(route);
                var taken = route.Length > 0 && editor.Taken.Contains(route);
                routeHint.Text = invalid ? L("需以小写字母开头，之后可用小写字母、数字和短横线。")
                    : taken ? L("已有提供方使用了这个 ID。")
                    : L("以小写字母开头的标识，在请求中唯一标识该提供方，并用于派生凭据名。");
                routeHint.Foreground = invalid || taken ? ThemeBrush("ErrorBrush") : ThemeBrush("TextTertiaryBrush");
                UpdateSaveState();
            };
            Field(L("Provider ID"), routeBox);
            panel.Children.Add(routeHint);
        }

        // —— 自定义流：显示名称 / API 地址 / API 协议平铺（必填项不在 Expander 里藏） ——
        if (editor.IsCustom)
        {
            var nameBox = Aut(new TextBox { Text = editor.CustomName, PlaceholderText = L("可选，默认使用 Provider ID") }, "ModelsCustomDisplayName", L("显示名称"));
            nameBox.TextChanged += (_, _) =>
            {
                editor.CustomName = nameBox.Text;
                UpdateSaveState();
            };
            Field(L("显示名称"), nameBox);

            var baseURLHint = Hint("", error: true);
            baseURLHint.Visibility = Visibility.Collapsed;
            var baseURLBox = Aut(new TextBox { Text = editor.BaseURL, PlaceholderText = "https://gateway.example/v1" }, "ModelsCustomBaseURL", L("API 地址"));
            baseURLBox.TextChanged += (_, _) =>
            {
                editor.BaseURL = baseURLBox.Text;
                var value = editor.BaseURL.Trim();
                baseURLHint.Text = L("请输入有效的 HTTP 或 HTTPS 地址。");
                baseURLHint.Visibility = value.Length > 0 && !IsHttpUrl(value) ? Visibility.Visible : Visibility.Collapsed;
                UpdateSaveState();
            };
            Field(L("API 地址"), baseURLBox);
            panel.Children.Add(baseURLHint);

            var apiBox = Aut(new ComboBox
            {
                MinWidth = TokenDouble("FieldMinWidth", 200),
                HorizontalAlignment = HorizontalAlignment.Left,
            }, "ModelsCustomApi", L("API 协议"));
            foreach (var protocol in editor.Protocols)
            {
                apiBox.Items.Add(new ComboBoxItem { Content = protocol, Tag = protocol });
                if (editor.Api.Length == 0)
                {
                    editor.Api = protocol; // 参考页 protocols[0] ?? ""
                }
                if (protocol == editor.Api)
                {
                    apiBox.SelectedItem = apiBox.Items[^1];
                }
            }
            apiBox.SelectionChanged += (_, _) =>
            {
                editor.Api = (apiBox.SelectedItem as ComboBoxItem)?.Tag as string ?? "";
                UpdateSaveState();
            };
            Field(L("API 协议"), apiBox);
        }

        // —— API 密钥（两种流都有；占位按凭据状态/家族，锁定来源禁输入） ——
        var credConfigured = row is { } r1 && r1.HasCredential && r1.Credential.ValueKind == JsonValueKind.Object &&
            r1.Credential.TryGetProperty("configured", out var cfgEl) && cfgEl.ValueKind == JsonValueKind.True;
        var credWritable = row is { } r2 && r2.HasCredential && r2.Credential.ValueKind == JsonValueKind.Object &&
            r2.Credential.TryGetProperty("writable", out var wrEl) && wrEl.ValueKind == JsonValueKind.True;
        var keyLocked = row is { } r3 && r3.HasCredential && !credWritable;
        var keyPlaceholder = keyLocked ? L("由启动环境提供（只读）")
            : credConfigured ? L("已配置——输入新值可替换")
            : editor.FamilyPiAi ? L("输入 API 密钥，或留空使用环境认证")
            : L("输入 API 密钥");
        var keyBox = Aut(new PasswordBox
        {
            PlaceholderText = keyPlaceholder,
            IsEnabled = !keyLocked,
        }, editor.IsCustom ? "ModelsCustomKey" : "ModelsEditorKey", L("API 密钥"));
        keyBox.PasswordChanged += (_, _) =>
        {
            editor.Key = keyBox.Password;
            UpdateSaveState();
        };
        Field(L("API 密钥"), keyBox);
        panel.Children.Add(keyHint);

        // —— 自定义设置 Expander（已有提供方）：显示名称（手声明）/ API 地址 / API 协议（手声明）/ 模型目录 ——
        if (!editor.IsCustom)
        {
            var inner = new StackPanel { Spacing = Sp12, Margin = new Thickness(0, Sp4, 0, 0) };
            if (editor.FamilyPiAi && editor.Declared)
            {
                var nameBox = Aut(new TextBox { Text = editor.CustomName }, "ModelsEditorDisplayName", L("显示名称"));
                nameBox.TextChanged += (_, _) =>
                {
                    editor.CustomName = nameBox.Text;
                    UpdateSaveState();
                };
                inner.Children.Add(new StackPanel
                {
                    Spacing = Sp4,
                    Children =
                    {
                        new TextBlock { Text = L("显示名称"), Style = AppStyle("FieldLabelTextStyle") },
                        nameBox,
                    },
                });
            }
            var baseURLBox = Aut(new TextBox
            {
                Text = editor.BaseURL,
                PlaceholderText = editor.FamilyDeepSeek
                    ? "https://api.deepseek.com"
                    : (string.IsNullOrEmpty(editor.FallbackBaseURL) ? L("提供方默认") : editor.FallbackBaseURL),
            }, "ModelsEditorBaseURL", L("API 地址"));
            baseURLBox.TextChanged += (_, _) => editor.BaseURL = baseURLBox.Text;
            inner.Children.Add(new StackPanel
            {
                Spacing = Sp4,
                Children =
                {
                    new TextBlock { Text = L("API 地址"), Style = AppStyle("FieldLabelTextStyle") },
                    baseURLBox,
                },
            });
            if (editor.FamilyPiAi && editor.Declared)
            {
                var apiBox = Aut(new ComboBox
                {
                    MinWidth = TokenDouble("FieldMinWidth", 200),
                    HorizontalAlignment = HorizontalAlignment.Left,
                }, "ModelsEditorApi", L("API 协议"));
                if (editor.Api.Length == 0)
                {
                    apiBox.Items.Add(new ComboBoxItem { Content = L("未选择"), Tag = "" });
                }
                foreach (var protocol in editor.Protocols)
                {
                    apiBox.Items.Add(new ComboBoxItem { Content = protocol, Tag = protocol });
                    if (protocol == editor.Api)
                    {
                        apiBox.SelectedItem = apiBox.Items[^1];
                    }
                }
                apiBox.SelectionChanged += (_, _) => editor.Api = (apiBox.SelectedItem as ComboBoxItem)?.Tag as string ?? "";
                inner.Children.Add(new StackPanel
                {
                    Spacing = Sp4,
                    Children =
                    {
                        new TextBlock { Text = L("API 协议"), Style = AppStyle("FieldLabelTextStyle") },
                        apiBox,
                    },
                });
            }
            inner.Children.Add(MakeModelCatalog(editor, UpdateSaveState));
            panel.Children.Add(new Expander
            {
                Header = L("自定义设置"),
                Content = inner,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Stretch,
            });
        }
        else
        {
            panel.Children.Add(MakeModelCatalog(editor, UpdateSaveState));
        }

        panel.Children.Add(modelHint);
        panel.Children.Add(failureHint);
        panel.Children.Add(actions);
        UpdateSaveState();
        return panel;

        // ---- 局部函数：草稿变化后重算校验与保存可用性 ----
        void UpdateSaveState()
        {
            var keyFailure = ValidateApiKeyDraft(editor.Key);
            if (keyFailure is null)
            {
                keyHint.Visibility = Visibility.Collapsed;
            }
            else
            {
                keyHint.Text = keyFailure == "keyBlank"
                    ? L("请输入 API 密钥；留空则保持已存储的密钥。")
                    : L("该 API 密钥格式错误，请检查。");
                keyHint.Visibility = Visibility.Visible;
            }
            editor.ModelError = ValidateModelRows(editor.Models);
            if (editor.InvalidCapacities.Count > 0 && editor.ModelError is null)
            {
                editor.ModelError = L("容量需为数字，可加 K 或 M 后缀。");
            }
            if (editor.ModelError is null)
            {
                modelHint.Visibility = Visibility.Collapsed;
            }
            else
            {
                modelHint.Text = editor.ModelError;
                modelHint.Visibility = Visibility.Visible;
            }
            var ok = keyFailure is null && editor.ModelError is null && !editor.Busy;
            if (ok && editor.IsCustom)
            {
                var route = editor.Route.Trim();
                var baseURL = editor.BaseURL.Trim();
                ok = route.Length > 0 && RoutePattern.IsMatch(route) && !editor.Taken.Contains(route)
                    && baseURL.Length > 0 && IsHttpUrl(baseURL)
                    && editor.Api.Length > 0
                    && editor.Models.Count > 0;
            }
            save.IsEnabled = ok;
        }
    }

    /// <summary>模型目录编辑器（DeepSeek / pi-ai 两家族共用）：行内编辑 + 获取可用模型 + 恢复默认。
    /// 结构变化（增删行/展开收起/恢复默认/采纳候选）整块重铺；文本输入原地改草稿不重铺（保焦点）。</summary>
    private FrameworkElement MakeModelCatalog(ModelsEditorState editor, Action onChanged)
    {
        var root = new StackPanel { Spacing = Sp8 };
        var rowsHost = new StackPanel { Spacing = Sp8 };
        var metaText = new TextBlock { Style = AppStyle("CaptionTextStyle"), Foreground = ThemeBrush("TextTertiaryBrush") };
        var resetLink = new Button { Style = AppStyle("CompactButtonStyle"), Content = L("恢复默认模型") };
        var fetchLink = new Button { Style = AppStyle("CompactButtonStyle"), Content = L("获取可用模型") };

        var head = new Grid { ColumnSpacing = Sp8 };
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var headText = new StackPanel { Spacing = Sp2 };
        headText.Children.Add(new TextBlock { Text = L("模型目录"), Style = AppStyle("FieldLabelTextStyle") });
        headText.Children.Add(metaText);
        Grid.SetColumn(headText, 0);
        headText.VerticalAlignment = VerticalAlignment.Center;
        var headActions = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = Sp8,
            VerticalAlignment = VerticalAlignment.Center,
        };
        headActions.Children.Add(resetLink);
        headActions.Children.Add(fetchLink);
        Grid.SetColumn(headActions, 1);
        head.Children.Add(headText);
        head.Children.Add(headActions);
        root.Children.Add(head);
        root.Children.Add(rowsHost);

        void Rebuild()
        {
            rowsHost.Children.Clear();
            metaText.Text = editor.ModelsOverridden ? L("已自定义模型目录") : L("正在使用适配器默认模型");
            resetLink.Visibility = editor.ModelsOverridden ? Visibility.Visible : Visibility.Collapsed;
            fetchLink.Visibility = editor.FamilyDeepSeek ? Visibility.Collapsed : Visibility.Visible;
            if (editor.Models.Count == 0)
            {
                rowsHost.Children.Add(new TextBlock
                {
                    Text = L("模型选择器中将不显示任何模型；目录外 ID 仍可直接发送。"),
                    Style = AppStyle("CaptionTextStyle"),
                    Foreground = ThemeBrush("TextTertiaryBrush"),
                    TextWrapping = TextWrapping.Wrap,
                });
            }
            else
            {
                for (var index = 0; index < editor.Models.Count; index++)
                {
                    rowsHost.Children.Add(MakeModelEntry(editor, editor.Models[index], index, Rebuild, onChanged));
                }
            }
            if (editor.FetchFailure is { Length: > 0 } failure)
            {
                rowsHost.Children.Add(new TextBlock
                {
                    Text = failure,
                    Style = AppStyle("CaptionTextStyle"),
                    Foreground = ThemeBrush("ErrorBrush"),
                    TextWrapping = TextWrapping.Wrap,
                });
            }
        }

        resetLink.Click += (_, _) =>
        {
            // 恢复默认 = 放弃覆盖：工作副本回到继承清单，保存时对 models 出 unset
            editor.Models = InheritedCopy(editor);
            editor.ModelsOverridden = false;
            editor.ExpandedModelRow = -1;
            editor.InvalidCapacities.Clear();
            Rebuild();
            onChanged();
        };
        fetchLink.Click += (_, _) => _ = FetchCandidatesForEditorAsync(editor, Rebuild);
        Rebuild();
        return root;
    }

    /// <summary>继承模型目录的工作副本（打开编辑卡 / 恢复默认时克隆，避免与展示清单别名共享）。</summary>
    private static List<JsonObject> InheritedCopy(ModelsEditorState editor)
        => editor.InheritedModels.Select(CloneNode).ToList();

    /// <summary>模型目录的一行：模型 ID / 显示名称 + 容量展开 + 删除。</summary>
    private FrameworkElement MakeModelEntry(ModelsEditorState editor, JsonObject model, int index, Action structural, Action onChanged)
    {
        var card = new Border
        {
            BorderBrush = ThemeBrush("StrokeBrush"),
            BorderThickness = Stroke1,
            CornerRadius = RadSmall,
            Padding = new Thickness(Sp8),
        };
        var body = new StackPanel { Spacing = Sp8 };
        card.Child = body;

        var row = new Grid { ColumnSpacing = Sp6 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(14, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(10, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var idBox = Aut(new TextBox { Text = StringOf(model, "id") ?? "", PlaceholderText = L("模型 ID") }, $"ModelId_{index}", L("模型 ID"));
        idBox.TextChanged += (_, _) =>
        {
            editor.ModelsOverridden = true;
            if (idBox.Text.Length == 0)
            {
                model.Remove("id");
            }
            else
            {
                model["id"] = idBox.Text;
            }
            onChanged();
        };
        var nameBox = Aut(new TextBox { Text = StringOf(model, "name") ?? "", PlaceholderText = L("显示名称") }, $"ModelName_{index}", L("显示名称"));
        nameBox.TextChanged += (_, _) =>
        {
            editor.ModelsOverridden = true;
            if (nameBox.Text.Length == 0)
            {
                model.Remove("name");
            }
            else
            {
                model["name"] = nameBox.Text;
            }
            onChanged();
        };
        var expanded = editor.ExpandedModelRow == index;
        var toggle = Aut(new Button
        {
            Style = AppStyle("IconButtonStyle"),
            Content = new FontIcon { Glyph = expanded ? "\uE70D" : "\uE76C", FontSize = GlyphCaption },
        }, $"ModelAdvanced_{index}", L("容量与模态"));
        ToolTipService.SetToolTip(toggle, L("容量与模态"));
        toggle.Click += (_, _) =>
        {
            editor.ExpandedModelRow = editor.ExpandedModelRow == index ? -1 : index;
            structural();
        };
        var remove = Aut(new Button
        {
            Style = AppStyle("IconButtonStyle"),
            Content = new FontIcon { Glyph = "\uE74D", FontSize = GlyphCaption },
        }, $"ModelRemove_{index}", L("删除模型"));
        ToolTipService.SetToolTip(remove, L("删除模型"));
        remove.Click += (_, _) =>
        {
            editor.Models.RemoveAt(index);
            editor.InvalidCapacities.RemoveWhere(key => key.Model == model);
            if (editor.ExpandedModelRow == index)
            {
                editor.ExpandedModelRow = -1;
            }
            else if (editor.ExpandedModelRow > index)
            {
                editor.ExpandedModelRow--;
            }
            structural();
            onChanged();
        };
        Grid.SetColumn(idBox, 0);
        Grid.SetColumn(nameBox, 1);
        Grid.SetColumn(toggle, 2);
        Grid.SetColumn(remove, 3);
        row.Children.Add(idBox);
        row.Children.Add(nameBox);
        row.Children.Add(toggle);
        row.Children.Add(remove);
        body.Children.Add(row);

        if (expanded)
        {
            var capacities = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp8 };
            capacities.Children.Add(MakeCapacityField(editor, model, "contextWindow", L("上下文窗口"),
                editor.FamilyDeepSeek && editor.DefaultContextWindow is { } cw ? FormatCapacity(cw) : "256K", onChanged));
            capacities.Children.Add(MakeCapacityField(editor, model, "maxTokens", L("最大输出 token 数"),
                editor.FamilyDeepSeek && editor.DefaultMaxTokens is { } mt ? FormatCapacity(mt) : "32K", onChanged));
            body.Children.Add(capacities);

            // 视觉模态：写模型目录的 input: ["text","image"]。缺这个键时 pi-ai 默认
            // 纯文本，session/prompt 带 image 块会被内核以 attachment-invalid 拒掉
            // （壳只能降级成文件重发，图就白贴了）。自定义提供方（stepfun 等）尤其要勾。
            var vision = new StackPanel { Spacing = Sp2 };
            var visionCheck = Aut(new CheckBox
            {
                Content = L("支持图片输入（视觉）"),
                IsChecked = ModelSupportsImage(model),
            }, $"ModelVision_{index}", L("支持图片输入"));
            visionCheck.Checked += (_, _) =>
            {
                editor.ModelsOverridden = true;
                model["input"] = new JsonArray { "text", "image" };
                onChanged();
            };
            visionCheck.Unchecked += (_, _) =>
            {
                editor.ModelsOverridden = true;
                model.Remove("input");
                onChanged();
            };
            vision.Children.Add(visionCheck);
            vision.Children.Add(new TextBlock
            {
                Text = L("未勾选时内核按纯文本模型处理：发送图片会被拒收并降级为普通文件，模型看不到图。"),
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextTertiaryBrush"),
                TextWrapping = TextWrapping.Wrap,
            });
            body.Children.Add(vision);
        }
        return card;
    }

    /// <summary>模型目录条目是否声明了 image 模态（input 数组含 "image"）。</summary>
    private static bool ModelSupportsImage(JsonObject model)
    {
        if (model.TryGetPropertyValue("input", out var node) && node is JsonArray arr)
        {
            foreach (var item in arr)
            {
                if (item is JsonValue v && v.TryGetValue<string>(out var s) && s == "image")
                {
                    return true;
                }
            }
        }
        return false;
    }

    /// <summary>容量输入（256K / 1M / 纯数字；空 = 继承提供方默认）。非法输入不落草稿，
    /// 但登记进 InvalidCapacities 挡住保存（对照参考页 parseCapacity → NaN → 校验拒绝）。</summary>
    private FrameworkElement MakeCapacityField(ModelsEditorState editor, JsonObject model, string field, string label, string placeholder, Action onChanged)
    {
        var stack = new StackPanel { Spacing = Sp2 };
        stack.Children.Add(new TextBlock
        {
            Text = label,
            Style = AppStyle("CaptionTextStyle"),
            Foreground = ThemeBrush("TextSecondaryBrush"),
        });
        var box = Aut(new TextBox
        {
            Text = LongOf(model, field) is { } value ? FormatCapacity(value) : "",
            PlaceholderText = placeholder,
            MinWidth = 140,
        }, $"ModelCapacity_{field}", label);
        box.TextChanged += (_, _) =>
        {
            var (value, invalid) = ParseCapacity(box.Text);
            if (invalid)
            {
                editor.InvalidCapacities.Add((model, field));
                return;
            }
            editor.InvalidCapacities.Remove((model, field));
            editor.ModelsOverridden = true;
            if (value is { } count)
            {
                model[field] = count;
            }
            else
            {
                model.Remove(field);
            }
            onChanged();
        };
        stack.Children.Add(box);
        return stack;
    }

    /// <summary>获取可用模型（llm/discoverModels）：探针 = 面板当前形态（含未保存的密钥），
    /// 一次面板内完成添加（参考页 ModelListEditor.fetchModels 语义）；回执只做候选，勾选才采纳。</summary>
    private async Task FetchCandidatesForEditorAsync(ModelsEditorState editor, Action refresh)
    {
        if (_rpc is null)
        {
            return;
        }
        var request = new Dictionary<string, object?>();
        if (!editor.IsCustom)
        {
            request["provider"] = editor.Provider;
        }
        var baseURL = editor.BaseURL.Trim();
        if (baseURL.Length > 0)
        {
            request["baseURL"] = baseURL;
        }
        if (editor.Api.Length > 0)
        {
            request["api"] = editor.Api;
        }
        var key = editor.Key.Trim();
        if (key.Length > 0)
        {
            request["apiKey"] = key;
        }
        JsonElement models;
        try
        {
            models = await _rpc.CallOkAsync("llm/discoverModels", new { settingsNs = editor.SettingsNs, request });
        }
        catch (DshRpcException ex)
        {
            editor.FetchFailure = ex.Message;
            refresh();
            return;
        }
        var candidates = new List<(string Id, string? Name, long Ctx, long Max)>();
        if (models.ValueKind == JsonValueKind.Array)
        {
            foreach (var m in models.EnumerateArray())
            {
                var id = m.TryGetProperty("id", out var i) ? i.GetString() ?? "" : "";
                if (id.Length == 0)
                {
                    continue;
                }
                var name = m.TryGetProperty("name", out var n) && n.ValueKind == JsonValueKind.String ? n.GetString() : null;
                var ctx = m.TryGetProperty("contextWindow", out var c) && c.ValueKind == JsonValueKind.Number ? c.GetInt64() : 0;
                var max = m.TryGetProperty("maxTokens", out var mt) && mt.ValueKind == JsonValueKind.Number ? mt.GetInt64() : 0;
                candidates.Add((id, name, ctx, max));
            }
        }
        if (candidates.Count == 0)
        {
            editor.FetchFailure = L("该提供方没有列出任何模型，请手动添加。");
            refresh();
            return;
        }
        editor.FetchFailure = null;
        var known = editor.Models.Select(m => StringOf(m, "id") ?? "").Where(id => id.Length > 0).ToHashSet(StringComparer.Ordinal);
        var adopted = await ShowCandidatePickerAsync(candidates, known);
        if (adopted.Count > 0)
        {
            // 按 id 合并：已有行原样保留（含本地编辑），新候选带名称/容量落行
            var byId = editor.Models.Where(m => StringOf(m, "id") is { Length: > 0 })
                .ToDictionary(m => StringOf(m, "id")!, m => m, StringComparer.Ordinal);
            foreach (var candidate in adopted)
            {
                if (byId.ContainsKey(candidate.Id))
                {
                    continue;
                }
                var rowModel = new JsonObject { ["id"] = candidate.Id };
                if (candidate.Name is { Length: > 0 })
                {
                    rowModel["name"] = candidate.Name;
                }
                if (candidate.Ctx > 0)
                {
                    rowModel["contextWindow"] = candidate.Ctx;
                }
                if (candidate.Max > 0)
                {
                    rowModel["maxTokens"] = candidate.Max;
                }
                editor.Models.Add(rowModel);
                byId[candidate.Id] = rowModel;
            }
            editor.ModelsOverridden = true;
        }
        refresh();
    }

    /// <summary>候选模型挑选对话框：搜索 + 全选/取消全选 + 勾选列表；新候选默认勾选（known 之外的）。</summary>
    private async Task<List<(string Id, string? Name, long Ctx, long Max)>> ShowCandidatePickerAsync(
        List<(string Id, string? Name, long Ctx, long Max)> candidates, HashSet<string> known)
    {
        var picked = candidates.Where(c => !known.Contains(c.Id)).Select(c => c.Id).ToHashSet(StringComparer.Ordinal);
        var host = new StackPanel { Spacing = Sp8, MinWidth = 400 };
        var search = Aut(new TextBox { PlaceholderText = L("搜索模型") }, "ModelCandidateSearch", L("搜索模型"));
        var toggleAll = new Button { Style = AppStyle("CompactButtonStyle"), Content = L("全选") };
        var toolbar = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp8 };
        toolbar.Children.Add(search);
        toolbar.Children.Add(toggleAll);
        host.Children.Add(toolbar);
        host.Children.Add(new TextBlock
        {
            Text = L("以下是模型提供方的可用模型，勾选要添加的模型。"),
            Style = AppStyle("CaptionTextStyle"),
            Foreground = ThemeBrush("TextTertiaryBrush"),
            TextWrapping = TextWrapping.Wrap,
        });
        var listPanel = new StackPanel { Spacing = Sp2 };
        var emptyHint = new TextBlock
        {
            Text = L("没有匹配的模型。"),
            Style = AppStyle("CaptionTextStyle"),
            Foreground = ThemeBrush("TextTertiaryBrush"),
            Visibility = Visibility.Collapsed,
        };
        host.Children.Add(new ScrollViewer
        {
            MaxHeight = 300,
            Content = new StackPanel { Children = { listPanel, emptyHint } },
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        });

        List<(string Id, string? Name, long Ctx, long Max)> Visible() => string.IsNullOrWhiteSpace(search.Text)
            ? candidates
            : candidates.Where(c => c.Id.Contains(search.Text.Trim(), StringComparison.OrdinalIgnoreCase) ||
                (c.Name?.Contains(search.Text.Trim(), StringComparison.OrdinalIgnoreCase) ?? false)).ToList();
        void RebuildList()
        {
            listPanel.Children.Clear();
            var visible = Visible();
            foreach (var candidate in visible)
            {
                var check = Aut(new CheckBox { Content = candidate.Id, IsChecked = picked.Contains(candidate.Id) },
                    $"ModelCandidate_{candidate.Id}", candidate.Id);
                check.Checked += (_, _) => picked.Add(candidate.Id);
                check.Unchecked += (_, _) => picked.Remove(candidate.Id);
                listPanel.Children.Add(check);
            }
            emptyHint.Visibility = visible.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
            toggleAll.Content = visible.Count > 0 && visible.All(c => picked.Contains(c.Id)) ? L("取消全选") : L("全选");
        }
        search.TextChanged += (_, _) => RebuildList();
        toggleAll.Click += (_, _) =>
        {
            var visible = Visible();
            if (visible.Count > 0 && visible.All(c => picked.Contains(c.Id)))
            {
                foreach (var candidate in visible)
                {
                    picked.Remove(candidate.Id);
                }
            }
            else
            {
                foreach (var candidate in visible)
                {
                    picked.Add(candidate.Id);
                }
            }
            RebuildList();
        };
        RebuildList();

        var dialog = new ContentDialog
        {
            Title = L("选择要添加的模型"),
            Content = host,
            PrimaryButtonText = L("添加所选"),
            CloseButtonText = L("取消"),
            DefaultButton = ContentDialogButton.Primary,
            XamlRoot = Content.XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return new List<(string, string?, long, long)>();
        }
        return candidates.Where(c => picked.Contains(c.Id)).ToList();
    }

    /// <summary>保存编辑卡：settings/mutate 最小 path ops + credentials/set（输入了新密钥时）。
    /// 设置先落、密钥后落（参考页 applyOnce 顺序）：密钥失败时配置已生效，卡上只留密钥错误可重试；
    /// 成功后收卡、挂一次性回执并重渲染。</summary>
    private async Task ApplyModelsEditorAsync(ModelsEditorState editor, ProviderRow? row)
    {
        if (_rpc is null || editor.Busy)
        {
            return;
        }
        var key = editor.Key.Trim();
        var keyDraftFailure = ValidateApiKeyDraft(editor.Key);
        if (keyDraftFailure is not null)
        {
            editor.Failure = keyDraftFailure == "keyBlank"
                ? L("请输入 API 密钥；若该提供方以其他方式鉴权，可以留空。")
                : L("该 API 密钥格式错误，请检查。");
            _ = RenderSectionAsync("models");
            return;
        }

        object[] ops;
        string keyRef;
        if (editor.IsCustom)
        {
            // 自定义提供方：整份 profile 一次 set（providers.<route>），必填项在保存前逐项点名
            var route = editor.Route.Trim();
            var baseURL = editor.BaseURL.Trim();
            editor.Failure = route.Length == 0 ? L("Provider ID 不能为空。")
                : !RoutePattern.IsMatch(route) ? L("需以小写字母开头，之后可用小写字母、数字和短横线。")
                : editor.Taken.Contains(route) ? L("已有提供方使用了这个 ID。")
                : baseURL.Length == 0 ? L("自定义提供方需要填写 API 地址。")
                : !IsHttpUrl(baseURL) ? L("请输入有效的 HTTP 或 HTTPS 地址。")
                : editor.Api.Length == 0 ? L("API 协议不能为空。")
                : editor.Models.Count == 0 ? L("自定义提供方至少需要一个模型。")
                : ValidateModelRows(editor.Models);
            if (editor.Failure is not null)
            {
                _ = RenderSectionAsync("models");
                return;
            }
            keyRef = DeriveKeyRef(route);
            var profile = new JsonObject();
            if (editor.CustomName.Trim() is { Length: > 0 } dn)
            {
                profile["displayName"] = dn;
            }
            if (key.Length > 0)
            {
                profile["apiKeyEnv"] = keyRef;
            }
            profile["api"] = editor.Api;
            profile["baseURL"] = baseURL;
            profile["models"] = ModelArray(editor.Models);
            ops = new object[] { new { op = "set", path = new[] { "providers", route }, value = profile } };
        }
        else
        {
            // 已有提供方：next = user 子树克隆 + 卡片可见字段；diff 出最小 ops（不动卡片外的字段）
            var next = editor.Committed is { } committed ? CloneNode(committed) : new JsonObject();
            if (editor.BaseURL.Trim().Length == 0)
            {
                next.Remove("baseURL");
            }
            else
            {
                next["baseURL"] = editor.BaseURL.Trim();
            }
            if (editor.FamilyPiAi && editor.Declared)
            {
                if (editor.CustomName.Trim().Length == 0)
                {
                    next.Remove("displayName");
                }
                else
                {
                    next["displayName"] = editor.CustomName.Trim();
                }
                if (editor.Api.Length == 0)
                {
                    next.Remove("api");
                }
                else
                {
                    next["api"] = editor.Api;
                }
            }
            if (editor.ModelsOverridden)
            {
                next["models"] = ModelArray(editor.Models);
            }
            else
            {
                next.Remove("models"); // 恢复默认模型 = unset（继承适配器目录）
            }
            keyRef = editor.KeyRef;
            // 参考页语义：pi-ai 路由既没在 user 层也没解析出 apiKeyEnv、而用户输入了密钥时，
            // 把派生引用写进 profile，密钥随后落 credentials/set
            if (editor.FamilyPiAi && key.Length > 0 &&
                StringOf(next, "apiKeyEnv") is null && editor.FallbackApiKeyEnv is null)
            {
                next["apiKeyEnv"] = keyRef;
            }
            var opsList = new List<object>();
            if (editor.FamilyPiAi && editor.FallbackMissing && editor.Committed is null && next.Count == 0)
            {
                // 物化空 profile：休眠目录路由不加密钥不填地址也要「存在」（环境认证语义）
                opsList.Add(new { op = "set", path = editor.SettingsPath, value = new JsonObject() });
            }
            else
            {
                opsList.AddRange(PathOps(editor.SettingsPath, editor.Committed, next));
            }
            ops = opsList.ToArray();
        }

        editor.Busy = true;
        string? settingsFailure = null;
        string? credentialFailure = null;
        await _settingsCommitGate.WaitAsync();
        try
        {
            if (ops.Length > 0)
            {
                try
                {
                    await _rpc.CallOkAsync("settings/mutate", new { ns = editor.SettingsNs, ops });
                    _settingsSnapshot = null;
                }
                catch (DshRpcException ex)
                {
                    settingsFailure = ex.Code == "settings/conflict"
                        ? L("这张卡片打开期间，这些设置已被其他地方改动。请关闭后重新打开，在当前值上编辑。")
                        : L("写入失败，请检查连接和字段值后重试。");
                }
                catch (Exception)
                {
                    settingsFailure = L("写入失败，请检查连接和字段值后重试。");
                }
            }
            if (settingsFailure is null && key.Length > 0)
            {
                try
                {
                    await _rpc.CallOkAsync("credentials/set", new { @ref = keyRef, value = key });
                }
                catch (Exception)
                {
                    credentialFailure = LF("{0}：密钥保存失败，请检查连接和凭据来源是否可写后重试。", keyRef);
                }
            }
        }
        finally
        {
            _settingsCommitGate.Release();
        }
        if (settingsFailure is not null || credentialFailure is not null)
        {
            editor.Busy = false;
            editor.Failure = settingsFailure ?? credentialFailure;
            _ = RenderSectionAsync("models"); // 卡保持打开，草稿保留，错误落在卡上
            return;
        }
        CloseModelsEditor();
        _modelsSavedNotice = LF("已保存 {0}。", editor.DisplayName);
        _ = RenderSectionAsync("models");
    }

    /// <summary>删除用户添加的提供方：先清页面托管的凭据（仅当 profile 点名派生引用且已配置可写），
    /// 再 unset 该提供方的设置子树；先凭据后设置，失败时整操作可安全重试（参考页语义）。</summary>
    private async Task RemoveProviderAsync(ProviderRow row)
    {
        if (_rpc is null)
        {
            return;
        }
        var managedRef = DeriveKeyRef(row.Provider);
        var removesCredential = row.ApiKeyEnv == managedRef && row.HasCredential &&
            row.Credential.ValueKind == JsonValueKind.Object &&
            row.Credential.TryGetProperty("configured", out var c) && c.ValueKind == JsonValueKind.True &&
            row.Credential.TryGetProperty("writable", out var w) && w.ValueKind == JsonValueKind.True;
        var dialog = new ContentDialog
        {
            Title = LF("删除 {0}？", row.DisplayName),
            Content = new TextBlock
            {
                Text = removesCredential
                    ? LF("删除 {0} 会移除其配置和存储的 API 密钥。", row.DisplayName)
                    : LF("删除 {0} 会移除其配置；其使用的凭据（如有）由其他位置管理，将会保留。", row.DisplayName),
                TextWrapping = TextWrapping.Wrap,
            },
            PrimaryButtonText = LF("删除 {0}", row.DisplayName),
            CloseButtonText = L("取消"),
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = Content.XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }
        DshRpcException? failure = null;
        await _settingsCommitGate.WaitAsync();
        try
        {
            if (removesCredential)
            {
                await _rpc.CallOkAsync("credentials/unset", new { @ref = managedRef });
            }
            await _rpc.CallOkAsync("settings/mutate", new
            {
                ns = row.SettingsNs,
                ops = new object[] { new { op = "unset", path = row.SettingsPath } },
            });
            _settingsSnapshot = null;
        }
        catch (DshRpcException ex)
        {
            failure = ex;
        }
        catch (Exception ex)
        {
            failure = new DshRpcException("network", ex.Message);
        }
        finally
        {
            _settingsCommitGate.Release();
        }
        if (failure is not null)
        {
            _ = ShowErrorAsync(LF("删除失败：{0}", failure.Message));
            return;
        }
        // 被删行的编辑卡若开着，一并收起
        if (_modelsEditor is { } editor && !editor.IsCustom &&
            editor.Provider == row.Provider && editor.SettingsNs == row.SettingsNs)
        {
            CloseModelsEditor();
        }
        _ = RenderSectionAsync("models");
    }

    /// <summary>打开一个提供方的编辑卡（行卡内嵌 / 添加卡）：草稿字段从 user 子树起步，
    /// 模型目录工作副本从继承清单（base 子树或 schema 默认）克隆。</summary>
    private void OpenModelsEditor(ProviderRow row, bool adding = false)
    {
        var displayName = StringAt(row.User, "displayName") ?? "";
        var api = StringAt(row.User, "api") ?? "";
        var editor = new ModelsEditorState
        {
            Provider = row.Provider,
            DisplayName = row.DisplayName,
            SettingsNs = row.SettingsNs,
            SettingsPath = row.SettingsPath,
            Declared = row.Declared,
            IsCustom = false,
            FamilyPiAi = row.SettingsNs == "llm-pi-ai",
            FamilyDeepSeek = row.SettingsNs == "llm-deepseek",
            Committed = row.User is { } user && user.ValueKind == JsonValueKind.Object &&
                JsonNode.Parse(user.GetRawText()) is JsonObject parsed ? parsed : null,
            KeyRef = row.KeyRef,
            FallbackApiKeyEnv = row.ApiKeyEnv,
            FallbackBaseURL = StringAt(row.Value, "baseURL"),
            FallbackMissing = row.Value is null,
            DefaultContextWindow = LongAt(row.Value, "defaultContextWindow"),
            DefaultMaxTokens = LongAt(row.Value, "maxTokens"),
            CustomName = displayName,
            Api = api,
            BaseURL = StringAt(row.User, "baseURL") ?? "",
            InheritedModels = InheritedModelsOf(row),
            Models = InheritedModelsOf(row).Select(CloneNode).ToList(),
            ModelsOverridden = false,
            Protocols = row.SettingsNs == "llm-pi-ai" ? ProtocolChoices() : new List<string>(),
            Taken = new List<string>(),
        };
        _modelsEditor = editor;
        _modelsAdding = adding;
    }

    /// <summary>「自定义提供方」创建卡状态：手声明一条 llm-pi-ai 路由（providers.<route> 整份 profile）。
    /// 必填：Provider ID / API 地址 / API 协议 / ≥1 个模型；模型目录从空清单起步。</summary>
    private ModelsEditorState NewCustomEditorState(List<ProviderRow> rows)
    {
        var protocols = ProtocolChoices();
        return new ModelsEditorState
        {
            Provider = "",
            DisplayName = "自定义提供方",
            SettingsNs = "llm-pi-ai",
            SettingsPath = new[] { "providers", "" },
            Declared = true,
            IsCustom = true,
            FamilyPiAi = true,
            Api = protocols.FirstOrDefault() ?? "",
            Protocols = protocols,
            Models = new(),
            ModelsOverridden = true,
            Taken = rows.Select(r => r.Provider).ToList(),
        };
    }

    private void CloseModelsEditor()
    {
        _modelsEditor = null;
        _modelsAdding = false;
    }

    /// <summary>继承模型目录：base 子树的 models，缺省回落 schema 默认（llm-deepseek 的内置目录）。</summary>
    private static List<JsonObject> InheritedModelsOf(ProviderRow row)
    {
        JsonElement? inherited = null;
        if (row.Base is { } baseNode && baseNode.ValueKind == JsonValueKind.Object &&
            baseNode.TryGetProperty("models", out var bm) && bm.ValueKind == JsonValueKind.Array)
        {
            inherited = bm;
        }
        inherited ??= SchemaDefaultAt(SchemaNodeAt(row.SchemaRoot, row.SettingsPath.Append("models").ToArray()));
        var list = new List<JsonObject>();
        if (inherited is { } node && node.ValueKind == JsonValueKind.Array)
        {
            foreach (var item in node.EnumerateArray())
            {
                if (item.ValueKind == JsonValueKind.Object && JsonNode.Parse(item.GetRawText()) is JsonObject obj)
                {
                    list.Add(obj);
                }
            }
        }
        return list;
    }

    /// <summary>手声明路由可选的线协议：读 llm-pi-ai schema 里 providers.<任意键>.api 的 union
    /// （占位键 \0probe 与参考页 protocolChoices 同源取法），保证选项与适配器接受的协议一致。</summary>
    private List<string> ProtocolChoices()
    {
        var choices = new List<string>();
        if (_settingsSnapshot is not null && _settingsSnapshot.TryGetValue("llm-pi-ai", out var view) &&
            view.Value.TryGetProperty("schema", out var schema))
        {
            foreach (var (value, _) in UnionChoices(SchemaNodeAt(schema, new[] { "providers", "\u0000probe", "api" })))
            {
                choices.Add(value);
            }
        }
        return choices;
    }

    /// <summary>最小 path ops（参考页 pathOps）：diff 出 set/unset，只命名卡片看得见的字段。</summary>
    private static List<object> PathOps(string[] basePath, JsonObject? before, JsonObject after)
    {
        var ops = new List<object>();
        var previous = before ?? new JsonObject();
        foreach (var (name, value) in after)
        {
            if (previous[name] is { } old && value is { } now && JsonNode.DeepEquals(old, now))
            {
                continue;
            }
            ops.Add(new { op = "set", path = basePath.Append(name).ToArray(), value });
        }
        foreach (var name in previous.Select(p => p.Key))
        {
            if (!after.ContainsKey(name))
            {
                ops.Add(new { op = "unset", path = basePath.Append(name).ToArray() });
            }
        }
        return ops;
    }

    private static JsonArray ModelArray(IEnumerable<JsonObject> models)
    {
        var array = new JsonArray();
        foreach (var model in models)
        {
            array.Add(CloneNode(model));
        }
        return array;
    }

    /// <summary>API 密钥输入判定（对照 apiKeyFailure）：空 = 保留现状；
    /// 纯空白 / NAME=value 环境行 / 带引号 / 非可打印 ASCII = 拒绝。</summary>
    private static string? ValidateApiKeyDraft(string draft)
    {
        if (draft.Length == 0)
        {
            return null;
        }
        var value = draft.Trim();
        if (value.Length == 0)
        {
            return "keyBlank";
        }
        if (EnvLinePattern.IsMatch(value) || IsQuotedValue(value) || !LegalApiKeyPattern.IsMatch(value))
        {
            return "keyIllegal";
        }
        return null;
    }

    private static readonly Regex EnvLinePattern = new("^[A-Z][A-Z0-9_]*=[^=]", RegexOptions.Compiled);
    private static readonly Regex LegalApiKeyPattern = new("^[\\x21-\\x7E]+$", RegexOptions.Compiled);

    /// <summary>Provider ID 规则（参考页 ROUTE_PATTERN）：小写字母开头，其后小写字母/数字/短横线段。</summary>
    private static readonly Regex RoutePattern = new("^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$", RegexOptions.Compiled);

    private static readonly Regex CapacityPattern = new("^(\\d+(?:\\.\\d+)?)([km])?$", RegexOptions.Compiled | RegexOptions.IgnoreCase);

    /// <summary>值是否被一对匹配的引号包裹（引号本身不是密钥的合法字符）。</summary>
    private static bool IsQuotedValue(string value)
    {
        var first = value[0];
        if (first is not ('"' or '\'' or '`'))
        {
            return false;
        }
        return value.Length > 1 && value.EndsWith(first);
    }

    /// <summary>容量输入解析：数字可带 K/M 后缀（K=1000，十进制口径同参考页）；空 = 继承。</summary>
    private static (long? Value, bool Invalid) ParseCapacity(string text)
    {
        var trimmed = text.Trim();
        if (trimmed.Length == 0)
        {
            return (null, false);
        }
        var match = CapacityPattern.Match(trimmed);
        if (!match.Success)
        {
            return (null, true);
        }
        var scale = match.Groups[2].Value.ToLowerInvariant() switch
        {
            "k" => 1000.0,
            "m" => 1000000.0,
            _ => 1.0,
        };
        var scaled = double.Parse(match.Groups[1].Value, CultureInfo.InvariantCulture) * scale;
        return ((long)Math.Round(scaled), false);
    }

    /// <summary>容量最短可逆拼写（256000 → 256K），与输入解析共用一套 K/M 词汇。</summary>
    private static string FormatCapacity(long value)
    {
        if (value <= 0)
        {
            return value.ToString();
        }
        if (value % 1_000_000 == 0)
        {
            return $"{value / 1_000_000}M";
        }
        if (value % 1000 == 0)
        {
            return $"{value / 1000}K";
        }
        return value.ToString();
    }

    /// <summary>模型目录行校验（对照 validateDeepSeekModels）：ID 必填且唯一、可选字段形态正确。
    /// 返回首错文案；null = 通过。</summary>
    private string? ValidateModelRows(List<JsonObject> models)
    {
        var seen = new HashSet<string>(StringComparer.Ordinal);
        for (var i = 0; i < models.Count; i++)
        {
            var model = models[i];
            var id = StringOf(model, "id")?.Trim();
            if (string.IsNullOrEmpty(id))
            {
                return LF("模型 {0}：模型 ID 不能为空。", i + 1);
            }
            if (!seen.Add(id))
            {
                return LF("模型 {0}：模型 ID 不能重复。", i + 1);
            }
            if (model.ContainsKey("name") && StringOf(model, "name") is not { Length: > 0 })
            {
                return LF("模型 {0}：显示名称不能为空。", i + 1);
            }
            if (LongOf(model, "contextWindow") is < 1)
            {
                return LF("模型 {0}：上下文窗口必须是正数，例如 131072、256K 或 1M。", i + 1);
            }
            if (LongOf(model, "maxTokens") is < 1)
            {
                return LF("模型 {0}：最大输出 token 数必须是正数，例如 8192、64K 或 1M。", i + 1);
            }
        }
        return null;
    }

    /// <summary>HTTP(S) 绝对地址校验（自定义提供方的 API 地址必填）。</summary>
    private static bool IsHttpUrl(string value)
        => Uri.TryCreate(value, UriKind.Absolute, out var uri) &&
           (uri.Scheme == Uri.UriSchemeHttp || uri.Scheme == Uri.UriSchemeHttps);

    /// <summary>沿路径取设置值子树；缺失返回 null（default(JsonElement) 的 ValueKind 是 Undefined）。</summary>
    private static JsonElement? SettingsValueAt(JsonElement value, string[] path)
    {
        var node = value;
        foreach (var seg in path)
        {
            if (node.ValueKind != JsonValueKind.Object || !node.TryGetProperty(seg, out var next))
            {
                return null;
            }
            node = next;
        }
        return node.ValueKind == JsonValueKind.Undefined ? null : node;
    }

    /// <summary>JSON 对象里的字符串字段（缺失/非字符串 → null）。</summary>
    private static string? StringAt(JsonElement? value, string name)
        => value is { } node && node.ValueKind == JsonValueKind.Object &&
           node.TryGetProperty(name, out var el) && el.ValueKind == JsonValueKind.String
            ? el.GetString()
            : null;

    /// <summary>JSON 对象里的整数字段（缺失/非整数 → null）。</summary>
    private static long? LongAt(JsonElement? value, string name)
        => value is { } node && node.ValueKind == JsonValueKind.Object &&
           node.TryGetProperty(name, out var el) && el.ValueKind == JsonValueKind.Number &&
           el.TryGetInt64(out var number)
            ? number
            : null;

    /// <summary>JsonNode 深拷贝（JsonObject/JsonArray 都走 ToJsonString 往返）。</summary>
    private static T CloneNode<T>(T node) where T : JsonNode => (T)JsonNode.Parse(node.ToJsonString())!;

    /// <summary>JsonObject 里的字符串字段（缺失/非字符串 → null）。</summary>
    private static string? StringOf(JsonObject obj, string key)
        => obj[key] is JsonValue value && value.TryGetValue<string>(out var s) ? s : null;

    /// <summary>JsonObject 里的整数字段（缺失/非整数 → null）。</summary>
    private static long? LongOf(JsonObject obj, string key)
        => obj[key] is JsonValue value && value.TryGetValue<long>(out var l) ? l : null;

    /// <summary>rehydrate 后的 schema 节点按路径取节点：object 走 dict 具名字段，dict 走 inner（开放键）。
    /// 路径里的占位键（\0probe）也走 inner，与参考页 protocolChoices 的取法一致。</summary>
    private static JsonElement? SchemaNodeAt(JsonElement schema, string[] path)
    {
        var node = RehydrateSchema(schema) is { } root ? root : schema;
        foreach (var seg in path)
        {
            if (node.ValueKind != JsonValueKind.Object)
            {
                return null;
            }
            if (node.TryGetProperty("dict", out var dict) && dict.ValueKind == JsonValueKind.Object &&
                dict.TryGetProperty(seg, out var field))
            {
                node = field; // object 的具名字段
            }
            else if (node.TryGetProperty("type", out var t) && t.GetString() == "dict" &&
                     node.TryGetProperty("inner", out var inner))
            {
                node = inner; // dict 的开放键：任意键命中原型
            }
            else
            {
                return null;
            }
        }
        return node;
    }

    /// <summary>schema 节点的 meta.default（继承模型目录的兜底来源）。</summary>
    private static JsonElement? SchemaDefaultAt(JsonElement? schemaNode)
        => schemaNode is { } node && node.ValueKind == JsonValueKind.Object &&
           node.TryGetProperty("meta", out var meta) && meta.TryGetProperty("default", out var def)
            ? def
            : null;

    /// <summary>派生凭据引用（参考页 deriveKeyRef）：<ROUTE>_API_KEY，非字母数字段折叠成下划线。</summary>
    private static string DeriveKeyRef(string provider)
        => $"{Regex.Replace(provider.ToUpperInvariant(), "[^A-Z0-9]+", "_")}_API_KEY";

    /// <summary>插件区当前 Tab（config = 插件配置 / inventory = 插件列表）。</summary>
    private string _pluginsTab = "config";

    /// <summary>
    /// 插件区（Tab 插件配置 / 插件列表 + 配置卡片）。
    /// 「插件列表」= 原独立分区的 pluginInventory 只读清单，按官方形态并入本页 Tab。
    /// </summary>
    private async Task RenderPluginsSectionAsync()
    {
        if (_rpc is null)
        {
            return;
        }
        SettingsHost.Children.Add(MakeSectionDesc("配置和查看本部署已安装的插件。"));

        // Tab（官方 settings.plugins：configurableTab = 插件配置 / inventory tab = 插件列表）
        var tabs = new SelectorBar
        {
            Margin = new Thickness(0, TokenDouble("Space8", 8), 0, TokenDouble("Space8", 8)),
        };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(tabs, "PluginsTabBar");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(tabs, L("插件视图"));
        var configTab = new SelectorBarItem
        {
            Text = "插件配置",
            Tag = "config",
            IsSelected = _pluginsTab == "config",
        };
        Aut(configTab, "PluginsTabConfig", "插件配置");
        var listTab = new SelectorBarItem
        {
            Text = "插件列表",
            Tag = "inventory",
            IsSelected = _pluginsTab == "inventory",
        };
        Aut(listTab, "PluginsTabInventory", "插件列表");
        tabs.Items.Add(configTab);
        tabs.Items.Add(listTab);
        tabs.SelectionChanged += (_, _) =>
        {
            var next = tabs.SelectedItem?.Tag as string ?? "config";
            if (next != _pluginsTab)
            {
                _pluginsTab = next;
                _ = RenderSectionAsync("plugins"); // 换 Tab 重渲染（内容源不同，两套渲染各自独立）
            }
        };
        SettingsHost.Children.Add(tabs);

        if (_pluginsTab == "inventory")
        {
            await RenderPluginInventoryContentAsync();
            return;
        }

        // 插件配置一级页：Windows 设置「应用」页形态 —— 每个选项组收成一张导航卡，
        // 点卡钻取进二级页看该组的设置行（OpenSettingsSubPage 进页时现建内容）。
        // 收起来的理由：六组配置平铺在一级页要滚三四屏，分组后一级页一屏可见、扫一眼就知道
        // 有哪些组；组内增行也不再挤占首页。二级页不缓存内容，每次进页都按当前快照重建。
        //
        // 六张卡另用一个紧排 StackPanel 包起来（Spacing=4，参考 Windows 设置「游戏」页的 3dip）：
        // 组内是并列的同级入口，间距要小于 SettingsHost 的节间距 12，否则六张卡散成六节、
        // 一眼看不出它们属于同一组。只收这一组的间距，不动 SettingsHost 的全局节间距。
        var navCards = new StackPanel { Spacing = Sp4 };
        navCards.Children.Add(MakeSettingsNavCard(
            "PluginsNav_shell", "\uE756", L("终端"), L("命令超时、单流输出上限。"),
            () => OpenSettingsSubPage(L("终端"), AddShellCard)));
        navCards.Children.Add(MakeSettingsNavCard(
            "PluginsNav_agentLoop", "\uE72C", L("Agent 循环"), L("同一步内最多同时运行多少个可并行的调用。"),
            () => OpenSettingsSubPage(L("Agent 循环"), AddAgentLoopCard)));
        navCards.Children.Add(MakeSettingsNavCard(
            "PluginsNav_subagent", "\uE9D9", L("Subagent"), L("控制 Agent 为 Subagent 选择模型的权限。"),
            () => OpenSettingsSubPage(L("Subagent"), AddSubagentCard)));
        navCards.Children.Add(MakeSettingsNavCard(
            "PluginsNav_webSearch", "\uE721", L("网页搜索"), L("DeepSeek 搜索提供方：密钥、接口地址、搜索次数。"),
            () => _ = OpenWebSearchSubPageAsync()));
        // 自动审批块（原「自动审批」分区整体并入此处）：dsh-approval-gate 的 auto-approve 预设开关
        // 与判定模型选择。归位理由：它本质是默认插件组合里一个 bundle 插件的预设开关，
        // 与默认插件卡同源同维护方。
        navCards.Children.Add(MakeSettingsNavCard(
            "PluginsNav_autoApproval", "\uE73E", L("自动审批"), L("越界请求的自动批准预设与判定模型。"),
            () => _ = OpenAutoApprovalSubPageAsync()));
        // 默认插件卡（原在「记忆」分区底部）：本次选型组合的安装/挂载状态，
        // 引导器幂等维护、只读呈现——安装类信息归位到插件页，不再散落到记忆分区。
        navCards.Children.Add(MakeSettingsNavCard(
            "PluginsNav_defaults", "\uEA86", L("默认插件"), L("默认插件组合的安装与挂载状态。"),
            () => OpenSettingsSubPage(L("默认插件"), AddDefaultPluginsCard)));
        SettingsHost.Children.Add(navCards);
    }

    /// <summary>终端卡（shell：命令超时 + 单流输出上限——对照 BashCardController 暴露字段）。</summary>
    private void AddShellCard(StackPanel host)
    {
        var card = NewCardIn(host, "终端", "限制 agent 运行的每一条命令。");
        card.Children.Add(MakeRow("命令超时（毫秒）", "单条命令允许运行多久，超时即终止。",
            MakeNumberBox("shell", "timeoutMs", NsNumber("shell", "timeoutMs", 120000))));
        AddDivider(card);
        card.Children.Add(MakeRow("单流输出上限（字节）", "超出部分会转存到临时文件，而不是被丢弃。",
            MakeNumberBox("shell", "maxOutputBytes", NsNumber("shell", "maxOutputBytes", 64000))));
    }

    /// <summary>Agent 循环卡：并行工具调用数。</summary>
    private void AddAgentLoopCard(StackPanel host)
    {
        var card = NewCardIn(host, "Agent 循环", "Agent 如何派发工具调用。");
        card.Children.Add(MakeRow("并行工具调用数", "同一步内最多同时运行多少个可并行的调用。",
            MakeNumberBox("agent-loop", "maxParallelToolCalls", NsNumber("agent-loop", "maxParallelToolCalls", 10))));
    }

    /// <summary>Subagent 卡（enabled 开关 + 允许模型清单；内核耦合校验见 MakeSubagentToggle 注释）。</summary>
    private void AddSubagentCard(StackPanel host)
    {
        var card = NewCardIn(host, "Subagent", L("控制 Agent 为 Subagent 选择模型的权限。"));
        var seedProvider = NsString("agent-default-model", "provider", "");
        var seedModel = NsString("agent-default-model", "model", "");
        var seedNote = seedProvider.Length > 0 && seedModel.Length > 0
            ? LF("内核要求开启时至少有一个允许模型，首次开启会自动写入当前默认模型（{0} / {1}）。", seedProvider, seedModel)
            : L("内核要求开启时至少有一个允许模型；当前读不到默认模型，开启可能被内核拒绝并在此提示。");
        var subToggle = MakeSubagentToggle();
        var subToggleRow = MakeRow(L("允许 Agent 为 Subagent 选择模型"),
            L("开启后，Agent 可以为每个 Subagent 选择提供方和模型。仅影响新会话。") + seedNote,
            subToggle);
        // Windows 11 设置惯例：带开关的行整行可点。开关自身的命中由控件处理，
        // 行级 Tapped 里跳过来自开关子树的点击，避免双切换。
        subToggleRow.Tapped += (_, args) =>
        {
            if (IsInSubtree(subToggle, args.OriginalSource as Microsoft.UI.Xaml.DependencyObject))
            {
                return;
            }
            subToggle.IsOn = !subToggle.IsOn;
        };
        card.Children.Add(subToggleRow);
    }

    /// <summary>网页搜索二级页：API Key 行的「已配置 / 未配置」要问内核 credentials/describe，
    /// 所以点卡后先等这个本地往返（毫秒级）再开页，页面因此一次性建全、没有占位骨架。</summary>
    private async Task OpenWebSearchSubPageAsync()
    {
        try
        {
            var searchKeyEnv = NsString("web-search-deepseek", "apiKeyEnv", "DEEPSEEK_API_KEY");
            var creds = await DescribeSearchCredentialsAsync(searchKeyEnv);
            OpenSettingsSubPage(L("网页搜索"), host => AddWebSearchCard(host, searchKeyEnv, creds));
        }
        catch (Exception ex)
        {
            System.Diagnostics.Debug.WriteLine($"[settings/web-search] {ex}");
        }
    }

    /// <summary>凭据状态查询（credentials/describe）。读失败返回空表：网页搜索页照常开，
    /// 只是没有 API Key 行（与改造前 try/catch 吞掉异常的行为一致），不阻塞其它两项设置。</summary>
    private async Task<List<(string Ref, bool Configured)>> DescribeSearchCredentialsAsync(string searchKeyEnv)
    {
        var result = new List<(string, bool)>();
        if (_rpc is null)
        {
            return result;
        }
        try
        {
            var credStates = await _rpc.CallOkAsync("credentials/describe", new { refs = new[] { searchKeyEnv } });
            foreach (var prop in credStates.EnumerateObject())
            {
                var configured = prop.Value.TryGetProperty("configured", out var cfg) && cfg.GetBoolean();
                result.Add((prop.Name, configured));
            }
        }
        catch (Exception) { }
        return result;
    }

    /// <summary>网页搜索卡（apiKey 凭据 + baseURL + maxUses——对照 WebSearchCardController）。</summary>
    private void AddWebSearchCard(StackPanel host, string searchKeyEnv, List<(string Ref, bool Configured)> creds)
    {
        var card = NewCardIn(host, "网页搜索", "DeepSeek 搜索提供方。");
        foreach (var (refName, configured) in creds)
        {
            var keyBox = Aut(new PasswordBox
            {
                PlaceholderText = configured ? "已保存，输入新值可替换" : "输入 API 密钥",
                MinWidth = FieldWidth,
                MaxWidth = FieldWidth,
                HorizontalAlignment = HorizontalAlignment.Right,
            }, $"PluginSearchKey_{searchKeyEnv}", "插件搜索 API 密钥");
            card.Children.Add(MakeRow(
                configured ? "API Key" : "API Key（未配置）",
                configured ? "已保存" : "配置之前搜索不可用",
                keyBox));
            // 已配置项输入了新值同样要写回（credentials/set 覆盖旧值）
            BindCredentialDraft(refName, keyBox);
        }

        card.Children.Add(MakeRow("接口地址", "留空则使用提供方默认地址。",
            MakeTextBox("web-search-deepseek", "baseURL", NsString("web-search-deepseek", "baseURL", ""), L("提供方默认"))));
        AddDivider(card);
        card.Children.Add(MakeRow("单次请求最多搜索次数", "一次请求在必须作答前最多可以搜索多少次。",
            MakeNumberBox("web-search-deepseek", "maxUses", NsNumber("web-search-deepseek", "maxUses", 5))));
    }

    /// <summary>自动审批二级页：先开页（说明 + 开关立即可见），判定模型目录是异步的，回来再补第二张卡。
    /// 返回 Task 而非 async void：调用点在导航卡的 lambda 里，用 `_ =` 承接便于异常不外泄到同步化路径。</summary>
    private async Task OpenAutoApprovalSubPageAsync()
    {
        try
        {
            var host = new StackPanel();
            OpenSettingsSubPage(L("自动审批"), host);
            await AddAutoApprovalCardsAsync(host);
        }
        catch (Exception ex)
        {
            System.Diagnostics.Debug.WriteLine($"[settings/auto-approval] {ex}");
        }
    }

    /// <summary>默认插件卡：本次选型组合的安装/挂载状态（只读，引导器幂等维护）。</summary>
    private void AddDefaultPluginsCard(StackPanel host)
    {
        var pluginCard = NewCardIn(
            host,
            L("默认插件"),
            L("Blade² 随内核插件机制默认启用；安装由引导器幂等完成，失败时下次启动自动重试。"));
        foreach (var status in DshPluginBootstrap.GetStatus(DataHome))
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6 };
            row.Children.Add(new TextBlock
            {
                Text = MountDisplayName(status.Id),
                Style = AppStyle("BodyTextStyle"),
                VerticalAlignment = VerticalAlignment.Center,
            });
            row.Children.Add(Spacer());
            row.Children.Add(new TextBlock
            {
                Text = MountStatusText(status),
                Style = AppStyle("CardDescriptionTextStyle"),
                VerticalAlignment = VerticalAlignment.Center,
            });
            pluginCard.Children.Add(row);
        }
    }

    /// <summary>
    /// 「插件」页的「插件列表」Tab 内容（原独立分区的只读清单，按官方形态并入 Tab）。
    /// 端点：pluginInventory/list {} → { entries:[{entryId,moduleName,enabled,fiberPhase}],
    ///                                   agentPresets?:[{…, rows:[{…, fiberPhase}]}] }
    /// 语义（dsh-host-plugin-inventory 的 PluginInventoryGateway.list 实测/读源确认）：
    ///   entries      = Cordis Loader 当前的非 group 条目，Loader 顺序；
    ///   fiberPhase   = active/loading/pending/failed/unloading/null（null = 无 fiber，即未实例化）；
    ///   enabled      = !entry.disabled（内核的启用位，与 fiberPhase 不是同一件事）；
    ///   agentPresets = 组合了预设名册时，每个预设的组装行（模型可见插件真正的运行位置）。
    /// 只读：本 Tab 不写任何设置，也不提供开关（内核未提供对应写入口）。
    /// </summary>
    private async Task RenderPluginInventoryContentAsync()
    {
        if (_rpc is null)
        {
            return;
        }
        SettingsHost.Children.Add(MakeSectionDesc(
            "内核 Loader 实际加载的插件条目（pluginInventory/list 只读快照），以及各 Agent 预设的组装行。"));

        JsonElement value;
        try
        {
            value = await _rpc.CallOkAsync("pluginInventory/list", new { });
        }
        catch (Exception ex) when (ex is DshRpcException or InvalidOperationException or HttpRequestException or TaskCanceledException)
        {
            SettingsHost.Children.Add(MakeSectionDesc(LF("插件清单不可用：{0}", ex.Message)));
            return;
        }

        var entries = value.TryGetProperty("entries", out var e) && e.ValueKind == JsonValueKind.Array ? e : default;
        var total = entries.ValueKind == JsonValueKind.Array ? entries.GetArrayLength() : 0;
        var active = 0;
        var failed = 0;
        var disabled = 0;
        if (entries.ValueKind == JsonValueKind.Array)
        {
            foreach (var entry in entries.EnumerateArray())
            {
                if (Str(entry, "fiberPhase") == "active")
                {
                    active++;
                }
                if (Str(entry, "fiberPhase") == "failed")
                {
                    failed++;
                }
                if (!(entry.TryGetProperty("enabled", out var en) && en.ValueKind == JsonValueKind.True))
                {
                    disabled++;
                }
            }
        }

        // ---- 概览卡：计数 + 过滤 ----
        var summaryCard = NewCard("概览", "计数直接来自本次快照，不做缓存。");
        summaryCard.Children.Add(new TextBlock
        {
            Text = LF("共 {0} 个 Loader 条目（active {1}，failed {2}，未启用 {3}）。", total, active, failed, disabled),
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, Sp8, 0, Sp4),
        });

        var listHost = new StackPanel { Spacing = 0 };
        var filterBox = new TextBox
        {
            PlaceholderText = "按模块名或条目 id 过滤…",
            Margin = new Thickness(0, Sp4, 0, Sp8),
        };
        Aut(filterBox, "PluginInventoryFilterBox", "按模块名或条目 id 过滤");
        summaryCard.Children.Add(filterBox);

        var countText = new TextBlock
        {
            Style = AppStyle("CaptionTextStyle"),
            Foreground = ThemeBrush("TextSecondaryBrush"),
            Margin = new Thickness(0, 0, 0, Sp4),
        };
        summaryCard.Children.Add(countText);

        var scroller = new ScrollViewer
        {
            MaxHeight = 360,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            Content = listHost,
        };
        // NewCard 内部已经把卡壳挂到 SettingsHost 并只返回行容器——这里不要再 Add 一次
        // （重复挂载同一 UIElement 会抛 0x800F1000「没有检测到已安装的组件」）。
        summaryCard.Children.Add(scroller);

        var entryRows = new List<(string Module, string EntryId, string Phase, bool Enabled)>();
        if (entries.ValueKind == JsonValueKind.Array)
        {
            foreach (var entry in entries.EnumerateArray())
            {
                entryRows.Add((
                    Str(entry, "moduleName"),
                    Str(entry, "entryId"),
                    Str(entry, "fiberPhase") is { Length: > 0 } p ? p : L("（无 fiber）"),
                    entry.TryGetProperty("enabled", out var en) && en.ValueKind == JsonValueKind.True));
            }
        }

        void Rebuild(string query)
        {
            listHost.Children.Clear();
            var shown = entryRows
                .Where(r => query.Length == 0
                            || r.Module.Contains(query, StringComparison.OrdinalIgnoreCase)
                            || r.EntryId.Contains(query, StringComparison.OrdinalIgnoreCase))
                .ToList();
            countText.Text = query.Length == 0
                ? LF("Loader 条目 {0} 个", shown.Count)
                : LF("匹配 “{0}”：{1} / {2} 个", query, shown.Count, entryRows.Count);
            foreach (var row in shown)
            {
                var line = new Grid { ColumnSpacing = Sp12, Padding = new Thickness(0, Sp6, 0, Sp6) };
                line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
                line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
                line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

                var left = new StackPanel { Spacing = Sp2 };
                left.Children.Add(new TextBlock { Text = row.Module, Style = AppStyle("BodyTextStyle"), TextWrapping = TextWrapping.Wrap });
                left.Children.Add(new TextBlock
                {
                    Text = row.EntryId,
                    Style = AppStyle("CodeTextStyle"),
                    Foreground = ThemeBrush("TextTertiaryBrush"),
                    IsTextSelectionEnabled = true,
                });
                Grid.SetColumn(left, 0);
                line.Children.Add(left);

                var phase = new TextBlock
                {
                    Text = row.Phase,
                    Style = AppStyle("CaptionTextStyle"),
                    VerticalAlignment = VerticalAlignment.Center,
                    Foreground = row.Phase switch
                    {
                        "failed" => ThemeBrush("InfoBrush"),
                        "active" => ThemeBrush("TextSecondaryBrush"),
                        _ => ThemeBrush("TextTertiaryBrush"),
                    },
                };
                Grid.SetColumn(phase, 1);
                line.Children.Add(phase);

                var flag = new TextBlock
                {
                    Text = row.Enabled ? L("已启用") : L("已禁用"),
                    Style = AppStyle("CaptionTextStyle"),
                    VerticalAlignment = VerticalAlignment.Center,
                    Foreground = row.Enabled ? ThemeBrush("TextSecondaryBrush") : ThemeBrush("TextTertiaryBrush"),
                };
                Grid.SetColumn(flag, 2);
                line.Children.Add(flag);

                listHost.Children.Add(line);
                listHost.Children.Add(new Microsoft.UI.Xaml.Shapes.Rectangle
                {
                    Height = 1,
                    Fill = ThemeBrush("StrokeSubtleBrush"),
                });
            }
        }

        filterBox.TextChanged += (_, _) =>
        {
            try
            {
                Rebuild((filterBox.Text ?? "").Trim());
            }
            catch (Exception) { }
        };
        Rebuild("");

        // ---- Agent 预设组装行（模型可见插件真正的运行位置） ----
        if (value.TryGetProperty("agentPresets", out var presets) && presets.ValueKind == JsonValueKind.Array &&
            presets.GetArrayLength() > 0)
        {
            var byPreset = new StackPanel { Spacing = Sp8 };
            foreach (var preset in presets.EnumerateArray())
            {
                var id = Str(preset, "presetId") is { Length: > 0 } pid ? pid : Str(preset, "id");
                var rows = preset.TryGetProperty("rows", out var rr) && rr.ValueKind == JsonValueKind.Array ? rr : default;
                var names = new List<string>();
                if (rows.ValueKind == JsonValueKind.Array)
                {
                    foreach (var row in rows.EnumerateArray())
                    {
                        var module = Str(row, "moduleName");
                        if (module.Length == 0)
                        {
                            module = Str(row, "name");
                        }
                        var phase = Str(row, "fiberPhase");
                        if (module.Length > 0)
                        {
                            names.Add(phase is { Length: > 0 } ? $"{module}（{phase}）" : module);
                        }
                    }
                }
                byPreset.Children.Add(new TextBlock
                {
                    Text = LF("{0}（{1} 行）", id, names.Count),
                    Style = AppStyle("BodyTextStyle"),
                    TextWrapping = TextWrapping.Wrap,
                });
                byPreset.Children.Add(new TextBlock
                {
                    Text = names.Count > 0 ? string.Join("，", names) : "（该预设没有组装行）",
                    Style = AppStyle("CaptionTextStyle"),
                    Foreground = ThemeBrush("TextTertiaryBrush"),
                    TextWrapping = TextWrapping.Wrap,
                });
            }
            var presetCard = NewCard("Agent 预设组装行",
                "预设的 rows 才是模型可见插件的运行位置（Loader 条目之外的第二处清单，与内核清单一致）。");
            // 同上：卡壳由 NewCard 挂载，此处只填行容器
            presetCard.Children.Add(byPreset);
        }
    }

    /// <summary>Agent 预设区：列表 + 查看/复制/删除/设为默认/当前会话使用/打开目录。
    /// 端点：agentPresets/list|read|copy|deletePreset|select + settings/canOpenAgentPresetDirectory|openAgentPresetDirectory。</summary>
    private async Task RenderPresetsSectionAsync()
    {
        if (_rpc is null)
        {
            return;
        }
        SettingsHost.Children.Add(MakeSectionDesc("预设即一个会话的 Agent 所运行的插件组装——它的工具、提示词与能力。"));

        var roster = default(JsonElement);
        var authorable = false;
        try
        {
            roster = await _rpc.CallOkAsync("agentPresets/list", new { });
            authorable = roster.TryGetProperty("authorable", out var au) && au.ValueKind == JsonValueKind.True;
        }
        catch (DshRpcException ex)
        {
            SettingsHost.Children.Add(new TextBlock
            {
                Text = LF("无法加载 Agent 预设：{0}", ex.Message),
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextSecondaryBrush"),
            });
            return;
        }

        // 预设目录是否可由内核原生打开（settings/canOpenAgentPresetDirectory：无参数 → bool）
        var canOpenDirectory = false;
        try
        {
            var v = await _rpc.CallOkAsync("settings/canOpenAgentPresetDirectory", new { });
            canOpenDirectory = v.ValueKind == JsonValueKind.True;
        }
        catch (Exception)
        {
            // 判定失败按不可用处理（按钮禁用 + 说明）
        }

        var activeSession = Volatile.Read(ref _activeSessionId);

        if (roster.TryGetProperty("presets", out var presets) && presets.ValueKind == JsonValueKind.Array)
        {
            foreach (var preset in presets.EnumerateArray())
            {
                var id = preset.GetProperty("id").GetString() ?? "";
                var name = (preset.TryGetProperty("name", out var n) && n.ValueKind == JsonValueKind.String ? n.GetString() : null) ?? id;
                var desc = preset.TryGetProperty("description", out var d) && d.ValueKind == JsonValueKind.String ? d.GetString() : null;
                var isDefault = preset.TryGetProperty("isDefault", out var def) && def.ValueKind == JsonValueKind.True;
                var isUser = preset.TryGetProperty("trust", out var t) && t.GetString() == "user";
                var trust = isUser ? "自定义" : "内置";
                var broken = preset.TryGetProperty("broken", out var b) && b.ValueKind == JsonValueKind.String ? b.GetString() : null;

                var card = CardShell();
                card.BorderBrush = isDefault ? ThemeBrush("AccentBrush") : ThemeBrush("StrokeBrush");
                card.Margin = new Thickness(0, Sp2, 0, Sp2);
                // 内置预设名是壳字典键（标准模式…）；用户自定义名不在键集，原样显示。
                var shownName = ShellEnglish.ContainsKey(name) ? L(name) : name;
                var layout = new Grid { ColumnSpacing = Sp16 };
                layout.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
                layout.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
                var left = new StackPanel { Spacing = Sp2, VerticalAlignment = VerticalAlignment.Center };
                left.Children.Add(new TextBlock
                {
                    Text = broken is not null ? LF("{0}（加载失败）", shownName)
                        : $"{shownName} · {L(trust)}{(isDefault ? " · " + L("默认预设") : "")}",
                    Style = AppStyle("BodyStrongTextStyle"),
                });
                if (desc is not null)
                {
                    left.Children.Add(new TextBlock { Text = desc, Style = AppStyle("CaptionTextStyle"), Foreground = ThemeBrush("TextSecondaryBrush"), TextWrapping = TextWrapping.Wrap });
                }
                if (broken is not null)
                {
                    left.Children.Add(new TextBlock { Text = broken, Style = AppStyle("CaptionTextStyle"), Foreground = ThemeBrush("InfoBrush"), TextWrapping = TextWrapping.Wrap });
                }
                left.Children.Add(new TextBlock
                {
                    Text = $"id: {id}",
                    Style = AppStyle("CodeTextStyle"),
                    Foreground = ThemeBrush("TextTertiaryBrush"),
                });
                Grid.SetColumn(left, 0);
                layout.Children.Add(left);

                // 操作列：复制 / 删除 / 当前会话使用 / 设为默认 / 打开目录
                var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6, VerticalAlignment = VerticalAlignment.Center };

                var copy = new Button { Content = "复制为…", Style = AppStyle("CompactButtonStyle") };
                Aut(copy, $"CopyPreset_{id}", LF("复制预设 {0}", shownName));
                copy.Click += (_, _) => _ = CopyPresetAsync(id, name);
                actions.Children.Add(copy);

                var useForSession = new Button
                {
                    Content = "当前会话使用",
                    Style = AppStyle("CompactButtonStyle"),
                    IsEnabled = activeSession is not null,
                };
                // 内核语义（探针实测）：preset 在会话的 agent 启动前才可切换，已开始的会话返回
                // agent-preset/locked（"has already started; its agent preset is fixed"）。
                ToolTipService.SetToolTip(useForSession, activeSession is null
                    ? "先在左侧选择一个会话"
                    : "agentPresets/select 应用到当前会话；内核只允许在会话的 agent 启动前切换（已开始会回 agent-preset/locked）");
                Aut(useForSession, $"UsePresetForSession_{id}", LF("当前会话使用预设 {0}", shownName));
                useForSession.Click += (_, _) => _ = SelectPresetAsync(id, name);
                actions.Children.Add(useForSession);

                if (!isDefault && broken is null)
                {
                    var btn = Aut(new Button { Content = "设为默认", Style = AppStyle("CompactButtonStyle") }, $"SetDefaultPreset_{id}", LF("设为默认预设 {0}", shownName));
                    // 新会话的默认预设：settings/update agent-presets.default
                    btn.Click += async (_, _) =>
                    {
                        try
                        {
                            await _rpc.CallOkAsync("settings/update", new { ns = "agent-presets", patch = new { @default = id } });
                            await RenderSectionAsync("agent-presets"); // 刷新默认标记
                        }
                        catch (DshRpcException ex)
                        {
                            _ = ShowErrorAsync(LF("设为默认失败：{0}", ex.Message));
                        }
                    };
                    actions.Children.Add(btn);
                }

                if (isUser)
                {
                    var openDir = new Button
                    {
                        Content = "打开目录",
                        Style = AppStyle("CompactButtonStyle"),
                        IsEnabled = canOpenDirectory,
                    };
                    ToolTipService.SetToolTip(openDir, canOpenDirectory
                        ? "在内核主机上打开该预设所在目录"
                        : "本部署无法原生打开目录（settings/canOpenAgentPresetDirectory=false）");
                    Aut(openDir, $"OpenPresetDirectory_{id}", LF("打开预设 {0} 的目录", shownName));
                    openDir.Click += (_, _) => _ = OpenPresetDirectoryAsync(id);
                    actions.Children.Add(openDir);

                    var del = Aut(new Button { Content = "删除", Style = AppStyle("CompactButtonStyle") }, $"DeletePreset_{id}", LF("删除预设 {0}", shownName));
                    del.Click += (_, _) => _ = DeletePresetAsync(id, name);
                    actions.Children.Add(del);
                }

                Grid.SetColumn(actions, 1);
                layout.Children.Add(actions);
                card.Child = layout;
                SettingsHost.Children.Add(card);
            }
        }

        // 预设目录能力与作者位（authorable=false 表示部署不允许新增自定义预设）
        var footer = NewCard("预设目录");
        if (!canOpenDirectory)
        {
            footer.Children.Add(new TextBlock
            {
                Text = "本部署无法原生打开预设目录（settings/canOpenAgentPresetDirectory 返回 false）。",
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextSecondaryBrush"),
                TextWrapping = TextWrapping.Wrap,
            });
        }
        footer.Children.Add(MakeRow(
            "新增自定义预设",
            authorable ? "用「复制为…」从任一预设派生一份用户预设，再用「打开目录」编辑其定义。" : "本部署声明不可创作预设（agentPresets/list 的 authorable=false）。",
            new TextBlock
            {
                Text = authorable ? "可用" : "不可用",
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextTertiaryBrush"),
            }));
    }

    /// <summary>复制为新预设（agentPresets/copy：from/id/name → void）。</summary>
    private async Task CopyPresetAsync(string fromId, string fromName)
    {
        if (_rpc is null)
        {
            return;
        }
        var idBox = Aut(new TextBox { Header = L("新预设 id"), PlaceholderText = L("小写字母/数字/短横线，如 my-agent") }, "NewPresetIdTextBox", "新预设 id");
        var nameBox = Aut(new TextBox { Header = L("显示名"), Text = LF("{0} 副本", fromName) }, "NewPresetNameTextBox", "显示名");
        var host = new StackPanel { Spacing = Sp10, MinWidth = TokenDouble("DialogMinWidthCompact", 360) };
        host.Children.Add(idBox);
        host.Children.Add(nameBox);
        var dialog = new ContentDialog
        {
            Title = LF("复制预设（来源：{0}）", fromName),
            Content = host,
            PrimaryButtonText = L("复制"),
            CloseButtonText = L("取消"),
            DefaultButton = ContentDialogButton.Primary,
            XamlRoot = Content.XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }
        var newId = idBox.Text.Trim();
        if (newId.Length == 0)
        {
            _ = ShowErrorAsync(L("新预设 id 不能为空。"));
            return;
        }
        try
        {
            await _rpc.CallOkAsync("agentPresets/copy", new { from = fromId, id = newId, name = nameBox.Text.Trim() });
            await RenderSectionAsync("agent-presets");
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("复制失败：{0}", ex.Message));
        }
    }

    /// <summary>删除用户预设（agentPresets/deletePreset：id → void），二次确认。</summary>
    private async Task DeletePresetAsync(string id, string name)
    {
        if (_rpc is null)
        {
            return;
        }
        var dialog = new ContentDialog
        {
            Title = L("删除预设"),
            Content = new TextBlock
            {
                Text = LF("确定删除自定义预设「{0}」（id: {1}）吗？该预设的目录会被移除，此操作不可撤销。", name, id),
                TextWrapping = TextWrapping.Wrap,
            },
            PrimaryButtonText = L("删除"),
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
            await _rpc.CallOkAsync("agentPresets/deletePreset", new { id });
            await RenderSectionAsync("agent-presets");
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("删除失败：{0}", ex.Message));
        }
    }

    /// <summary>选择当前会话使用的预设（agentPresets/select：agentId=会话 id + agentPreset → 生效的预设 id）。</summary>
    private async Task SelectPresetAsync(string id, string name)
    {
        if (_rpc is null)
        {
            return;
        }
        var sid = Volatile.Read(ref _activeSessionId);
        if (sid is null)
        {
            _ = ShowErrorAsync(L("请先在左侧选择一个会话：预设切换作用在会话的 agent 上。"));
            return;
        }
        try
        {
            var applied = await _rpc.CallOkAsync("agentPresets/select", new { agentId = sid, agentPreset = id });
            _ = ShowErrorAsync(LF("会话已切换到预设「{0}」（内核回执：{1}）。", name, applied.GetString()));
        }
        catch (DshRpcException ex)
        {
            // 常见内核拒绝：agent-preset/locked（会话的 agent 已启动，预设随之固定）
            _ = ShowErrorAsync(LF("切换预设失败：{0}", ex.Message));
        }
    }

    /// <summary>打开用户预设目录（settings/openAgentPresetDirectory：agentPreset → {opened} 或 {opened:false,path}）。</summary>
    private async Task OpenPresetDirectoryAsync(string id)
    {
        if (_rpc is null)
        {
            return;
        }
        try
        {
            var value = await _rpc.CallOkAsync("settings/openAgentPresetDirectory", new { agentPreset = id });
            var opened = value.TryGetProperty("opened", out var o) && o.ValueKind == JsonValueKind.True;
            if (!opened)
            {
                // 内核无法原生打开时回传路径：壳把它显示出来，让用户自己打开
                var p = value.TryGetProperty("path", out var path) ? path.GetString() : null;
                _ = ShowErrorAsync(LF("内核无法直接打开目录，路径：{0}", p));
            }
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("打开预设目录失败：{0}", ex.Message));
        }
    }

    /// <summary>遗留空实现：页脚按钮已移除；自动保存路径不依赖它。</summary>
    private void MarkSettingsDirty()
    {
    }

    /// <summary>串行提交稳定的草稿快照。成功仅移除同一对象版本，失败和提交期间的新编辑均保留。
    /// 不重建控件、不弹成功对话框；失败按项提示且可重试，避免打断输入或丢失焦点。</summary>
    private async Task SaveSettingsAsync()
    {
        await _settingsCommitGate.WaitAsync();
        try
        {
            if (!HasPendingSettings)
            {
                SetSettingsSaveError(null);
                return;
            }
            var rpc = _rpc;
            if (rpc is null)
            {
                SetSettingsSaveError(L("内核未连接，未保存的草稿已保留。连接后请重试。"));
                return;
            }
            // 所有集合在第一个 RPC await 前物化，UI 事件可以继续登记新版本。
            var edits = _settingsEdits.ToArray();
            var credentials = _pendingCredentialWrites.ToArray();
            var failures = new List<string>();
            var snapshotStale = false;
            foreach (var group in edits.GroupBy(e => e.Ns))
            {
                var entries = group.Where(e => e.Value is not null).ToArray();
                if (entries.Length != group.Count())
                {
                    failures.Add(LF("{0}：有空值或无效值，请修正后重试", group.Key));
                }
                if (entries.Length == 0) continue;
                var ops = entries.Select(entry => new { op = "set", path = entry.Path, value = entry.Value }).ToArray();
                try
                {
                    await rpc.CallOkAsync("settings/mutate", new { ns = group.Key, ops });
                    snapshotStale = true;
                    foreach (var entry in entries)
                    {
                        _settingsEdits.Remove(entry); // 引用身份：不会移除提交期间替换的新版本
                    }
                }
                catch (Exception)
                {
                    // 不透传 RPC 的异常正文，服务端可能在其中回显请求内容。
                    failures.Add(LF("{0}：写入失败，请检查连接和字段值后重试", group.Key));
                }
            }
            if (snapshotStale)
            {
                // 保存成功后立即重拉快照（旧实现只置空、"下次渲染再拉"）：
                // 渲染之间的读数（NsBool/NsString/allowedModels 清单）会拿到空快照——
                // 实测开关 Off→On 时种子逻辑因此误判"无允许模型"，后续追加覆盖了内核已有条目。
                await EnsureSettingsSnapshotAsync();
            }
            foreach (var (refName, draft) in credentials)
            {
                // 前面的 RPC 等待期间可能已取消/替换此项，尚未发出的旧值不再提交。
                if (!_pendingCredentialWrites.TryGetValue(refName, out var current) || !ReferenceEquals(current, draft)) continue;
                try
                {
                    await rpc.CallOkAsync("credentials/set", new { @ref = refName, value = draft.Value });
                    if (_pendingCredentialWrites.TryGetValue(refName, out current) && ReferenceEquals(current, draft))
                    {
                        _pendingCredentialWrites.Remove(refName);
                        ClearCommittedCredentialBox(refName);
                    }
                }
                catch (Exception)
                {
                    failures.Add(LF("{0}：密钥保存失败，请检查连接和凭据来源是否可写后重试", refName));
                }
            }
            SetSettingsSaveError(failures.Count == 0 ? null
                : LF("未保存的草稿已保留（仅当前窗口内）。{0}", string.Join("；", failures)));
        }
        catch (Exception)
        {
            SetSettingsSaveError(L("保存未完成，未保存的草稿已保留。请重试。"));
        }
        finally
        {
            _settingsCommitGate.Release();
        }
    }

    // ---------------- 模型选择（session/modelCatalog → session/selectModel） ----------------

    /// <summary>模型菜单展开时重拉目录：catalog → _modelOptions → MenuFlyout。
    /// 与 ComposerAdd/AgentMode 菜单同模式（内核侧懒解析，缓存会锁住早期失败结果）。</summary>
    private async Task RefreshModelCatalogAsync()
    {
        if (_rpc is null)
        {
            return;
        }
        JsonElement catalog;
        try
        {
            catalog = await _rpc.CallOkAsync("session/modelCatalog", new { });
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("读取模型目录失败：{0}", ex.Message));
            return;
        }

        // catalog.default 是新会话的默认模型（provider 不再硬编码 deepseek-official）
        if (catalog.TryGetProperty("default", out var def) && def.ValueKind == JsonValueKind.Object)
        {
            _catalogDefault = (
                def.TryGetProperty("provider", out var p) ? p.GetString() ?? "deepseek-official" : "deepseek-official",
                def.TryGetProperty("model", out var m) ? m.GetString() ?? "" : "");
        }

        PopulateModelOptions(catalog);
        if (_selectedModelId.Length == 0 && _catalogDefault is { } cd && cd.Model.Length > 0)
        {
            _selectedModelId = cd.Model;
            _selectedModelProvider = cd.Provider;
        }
        if (_selectedModelName.Length == 0)
        {
            var dflt = _modelOptions.FirstOrDefault(o => ModelIdOf(o.Model) == _selectedModelId);
            if (dflt.Model.ValueKind == JsonValueKind.Object)
            {
                _selectedModelName = dflt.Name;
            }
        }
        // 当前模型的推理档（取不到就保持上一份档位表，不清空）
        var current = _modelOptions.FirstOrDefault(m => ModelIdOf(m.Model) == _selectedModelId);
        if (current.Model.ValueKind == JsonValueKind.Object)
        {
            SetEffortMenuFromModel(current.Model);
        }
        RebuildModelFlyout();
        UpdateModelEffortLabel();
    }

    /// <summary>catalog 里模型的 id（_modelOptions 匹配用；显示名可能 ≠ id）。</summary>
    private static string ModelIdOf(JsonElement model) =>
        model.TryGetProperty("id", out var id) ? id.GetString() ?? "" : "";

    /// <summary>把 catalog.groups 摊平成 _modelOptions（菜单项数据源；调用方随后 RebuildModelFlyout）。</summary>
    private void PopulateModelOptions(JsonElement catalog)
    {
        lock (_modelOptionsLock)
        {
            _modelOptions.Clear();
        }
        if (!catalog.TryGetProperty("groups", out var groups) || groups.ValueKind != JsonValueKind.Array)
        {
            return;
        }
        var added = new List<(string Name, string Provider, JsonElement Model)>();
        foreach (var group in groups.EnumerateArray())
        {
            var providerId = group.TryGetProperty("id", out var gid) ? gid.GetString() : null;
            if (!group.TryGetProperty("models", out var models) || models.ValueKind != JsonValueKind.Array)
            {
                continue;
            }
            foreach (var model in models.EnumerateArray())
            {
                var id = model.GetProperty("id").GetString() ?? "";
                var name = model.GetProperty("name").GetString() ?? id;
                added.Add((name, providerId ?? _catalogDefault?.Provider ?? "deepseek-official", model.Clone()));
            }
        }
        lock (_modelOptionsLock)
        {
            _modelOptions.AddRange(added);
        }
    }

    /// <summary>重建模型触发器菜单：模型区 + 分隔线 + 推理等级区）。</summary>
    private void RebuildModelFlyout()
    {
        ModelFlyout.Items.Clear();
        ModelFlyout.Items.Add(new MenuFlyoutItem { Text = L("模型"), IsEnabled = false });
        foreach (var (name, provider, model) in _modelOptions)
        {
            var item = new MenuFlyoutItem { Text = name, Tag = name };
            Aut(item, $"ModelOption_{name}", LF("模型：{0}", name));
            item.Click += (_, _) => _ = SelectModelAsync(model, provider, name);
            ModelFlyout.Items.Add(item);
        }
        ModelFlyout.Items.Add(new MenuFlyoutSeparator());
        ModelFlyout.Items.Add(new MenuFlyoutItem { Text = L("推理等级"), IsEnabled = false });
        foreach (var (id, label) in _effortOptions)
        {
            // 推理等级是单选：用官方 RadioMenuFlyoutItem（同组自动互斥、UIA 读作单选），
            // 不再用 Toggle 菜单项 + 手写取消选中来模拟。
            var item = new RadioMenuFlyoutItem
            {
                Text = label,
                Tag = id,
                GroupName = "ReasoningEffort",
                IsChecked = id == _reasoningEffort,
            };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(item, $"EffortOption_{id}");
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(item, LF("推理等级：{0}", label));
            item.Click += OnEffortMenuClick;
            ModelFlyout.Items.Add(item);
        }
    }

    /// <summary>触发器文案（也用作无障碍名称）：模型 + 推理等级（如"DeepSeek-V4-Pro High"）。</summary>
    private void UpdateModelEffortLabel()
    {
        var model = _selectedModelName.Length > 0 ? _selectedModelName : L("选择模型");
        var effort = _effortOptions.FirstOrDefault(o => o.Id == _reasoningEffort).Label;
        ModelEffortLabel.Text = string.IsNullOrEmpty(effort) ? model : $"{model} {effort}";
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(ModelButton, LF("模型与推理等级，当前 {0}", ModelEffortLabel.Text));
    }

    /// <summary>把模型选择应用到当前会话（selectModel），无活动会话时只更新触发器显示。
    /// 若 catalog 给了该模型的推荐推理等级，同步推理档（触发器显示 = 实际将应用的档）。</summary>
    private async Task SelectModelAsync(JsonElement model, string provider, string? displayName = null)
    {
        var id = model.GetProperty("id").GetString() ?? "";
        var name = displayName ?? (model.TryGetProperty("name", out var n) ? n.GetString() ?? id : id);
        _selectedModelName = name;
        _selectedModelId = id;
        _selectedModelProvider = provider;

        // 推理等级默认取 catalog 里该模型的推荐档（defaultEffort，没有则 efforts 最后一档）
        string? recommendedEffort = null;
        SetEffortMenuFromModel(model);
        if (model.TryGetProperty("reasoning", out var r) && r.ValueKind == JsonValueKind.Object)
        {
            if (r.TryGetProperty("defaultEffort", out var de) && de.ValueKind == JsonValueKind.String)
            {
                recommendedEffort = de.GetString();
            }
            else if (r.TryGetProperty("efforts", out var efforts) && efforts.ValueKind == JsonValueKind.Array && efforts.GetArrayLength() > 0)
            {
                recommendedEffort = efforts[efforts.GetArrayLength() - 1].TryGetProperty("id", out var eid) ? eid.GetString() : null;
            }
        }
        if (recommendedEffort is not null)
        {
            SetEffortUi(recommendedEffort);
        }
        RebuildModelFlyout();
        UpdateModelEffortLabel();

        if (_activeSessionId is null || _rpc is null)
        {
            return;
        }
        try
        {
            var payload = new Dictionary<string, object>
            {
                ["sessionId"] = _activeSessionId,
                ["provider"] = provider,
                ["model"] = id,
            };
            if (recommendedEffort is not null)
            {
                payload["reasoningEffort"] = recommendedEffort;
            }
            // typert wire：参数包在 request 字段里（同 session/create/prompt 的形状）；
            // 扁平传会被 gateway assertExactArguments 拒掉（missing "request"）。
            await _rpc.CallOkAsync("session/selectModel", new { request = payload });
        }
        catch (DshRpcException ex)
        {
            // 切换失败只影响当前会话，不打断聊天；但必须让用户看见——静默失败时触发器
            // 显示已变、内核实际没变，用户只会以为"模型切换失效了"。
            ShellToast.Show(L("模型切换失败"), LF("未能切换到 {0}：{1}", name, ex.Message));
        }
    }

    /// <summary>这次 selectModel 该给目标模型带哪个推理档（null = 不带这个字段）。
    /// 内核对档位是硬校验（dsh-llm resolveCallWithInfo）：模型没有 reasoning 能力时带任何档位
    /// 都抛 UNSUPPORTED_REASONING_EFFORT，档位不在 efforts 里同样抛，且不做 clamping——
    /// 整个 selectModel 因此失败，模型切换静默失效（自建 provider 的模型都没声明 reasoning）。
    /// 取用户当前档位（该模型支持时），否则模型默认档，否则最后一档；目录还没拉到时不猜，
    /// 省略字段让内核用模型自己的默认档（省略对所有模型都是合法的）。</summary>
    private string? EffortForModelId(string modelId)
    {
        (string Name, string Provider, JsonElement Model)[] snapshot;
        lock (_modelOptionsLock)
        {
            snapshot = _modelOptions.ToArray();
        }
        var entry = snapshot.FirstOrDefault(o => ModelIdOf(o.Model) == modelId);
        if (entry.Model.ValueKind != JsonValueKind.Object ||
            !entry.Model.TryGetProperty("reasoning", out var reasoning) ||
            reasoning.ValueKind != JsonValueKind.Object ||
            !reasoning.TryGetProperty("efforts", out var efforts) ||
            efforts.ValueKind != JsonValueKind.Array)
        {
            return null;
        }
        var ids = new List<string>();
        foreach (var e in efforts.EnumerateArray())
        {
            if (e.TryGetProperty("id", out var eid) && eid.GetString() is { Length: > 0 } id)
            {
                ids.Add(id);
            }
        }
        if (ids.Count == 0)
        {
            return null;
        }
        if (ids.Contains(_reasoningEffort, StringComparer.Ordinal))
        {
            return _reasoningEffort;
        }
        var fallback = reasoning.TryGetProperty("defaultEffort", out var de) && de.ValueKind == JsonValueKind.String
            ? de.GetString()
            : null;
        return fallback is { Length: > 0 } && ids.Contains(fallback, StringComparer.Ordinal) ? fallback : ids[^1];
    }

    /// <summary>modelSelection 投影视图：{lastUsed, next}，next = 待生效选择 ?? 本会话上次实际
    /// 使用（内核 modelSelectionProjection 的 wire.view）。官方触发器口径就是 next ?? catalog.default：
    /// 会话自己选过模型就跟随会话，没选过才用全局默认（= 上次使用的模型，由 selectModel 写进
    /// agent-default-model）。内核的判定优先于壳里记的意图——档位不被支持时内核会按模型默认档
    /// 收下，这里负责把壳拉回真相（消息上的型号同样以 request/header 为准）。</summary>
    private void ApplyModelSelectionProjection(JsonElement view)
    {
        // 调用方持 _projectionLock
        if (view.ValueKind != JsonValueKind.Object ||
            !view.TryGetProperty("next", out var next) || next.ValueKind != JsonValueKind.Object)
        {
            return;
        }
        var model = Str(next, "model");
        if (model.Length == 0 || model == _selectedModelId)
        {
            return;
        }
        var provider = Str(next, "provider");
        var effort = Str(next, "reasoningEffort");
        // _modelOptions 只在 UI 线程改动，查表也放回 UI 线程做
        PostUi(() =>
        {
            _selectedModelId = model;
            if (provider.Length > 0)
            {
                _selectedModelProvider = provider;
            }
            var option = _modelOptions.FirstOrDefault(o => ModelIdOf(o.Model) == model);
            if (option.Model.ValueKind == JsonValueKind.Object)
            {
                _selectedModelName = option.Name;
                SetEffortMenuFromModel(option.Model);
            }
            if (effort.Length > 0)
            {
                SetEffortUi(effort);
            }
            RebuildModelFlyout();
            UpdateModelEffortLabel();
        });
    }

    /// <summary>按档位 id 同步推理档（选中态 + 触发器文案 + 字段）。
    /// 档位来自内核 catalog（off/low/high/max），SetEffortMenuFromModel 已把菜单项生成好；
    /// 这里只按 Tag 比对，未知档（如旧内核 medium）不崩，显示原文。</summary>
    private void SetEffortUi(string effort)
    {
        _reasoningEffort = effort;
        // 同组 RadioMenuFlyoutItem 由控件自己保证互斥，这里只把选中态对齐到当前值
        foreach (var item in ModelFlyout.Items.OfType<RadioMenuFlyoutItem>())
        {
            item.IsChecked = item.Tag as string == effort;
        }
        UpdateModelEffortLabel();
    }

    /// <summary>effort 档位中文标签（内核 catalog 未给 name 时的兜底文案）。</summary>
    private string EffortLabelZh(string id) => id switch
    {
        "off" => L("关闭"),
        "low" => L("低"),
        "high" => L("高"),
        "max" => L("最大"),
        _ => id,
    };

    /// <summary>模型目录快照（catalog.default 的 provider/model——发送时 selectModel 用，不再硬编码）。</summary>
    private (string Provider, string Model)? _catalogDefault;

    /// <summary>按内核 catalog 同步推理档选项（档位 = 当前模型 reasoning.efforts）。
    /// 文案取内核 name（与官方触发器"模型 + High"一致），缺 name 才回落到中文档位名。</summary>
    private void SetEffortMenuFromModel(JsonElement model)
    {
        var reasoning = model.ValueKind == JsonValueKind.Object && model.TryGetProperty("reasoning", out var r) ? r : default;
        var options = new List<(string Id, string Label)>();
        if (reasoning.ValueKind == JsonValueKind.Object &&
            reasoning.TryGetProperty("efforts", out var efforts) && efforts.ValueKind == JsonValueKind.Array)
        {
            foreach (var e in efforts.EnumerateArray())
            {
                var id = e.TryGetProperty("id", out var idEl) ? idEl.GetString() : null;
                if (id is null)
                {
                    continue;
                }
                var name = e.TryGetProperty("name", out var nEl) && nEl.ValueKind == JsonValueKind.String ? nEl.GetString() : null;
                options.Add((id, name is { Length: > 0 } ? name : EffortLabelZh(id)));
            }
        }
        if (options.Count == 0)
        {
            // catalog 没给 efforts：静态四档兜底（老内核兼容）
            options.AddRange(new[]
            {
                ("off", EffortLabelZh("off")),
                ("low", EffortLabelZh("low")),
                ("high", EffortLabelZh("high")),
                ("max", EffortLabelZh("max")),
            });
        }
        _effortOptions.Clear();
        _effortOptions.AddRange(options);
        if (_effortOptions.All(o => o.Id != _reasoningEffort))
        {
            _reasoningEffort = _effortOptions[^1].Id;
        }
    }

    // ---------------- 杂项 ----------------

    // WinUI3 中上一张 ContentDialog 未关完时再次 ShowAsync 会抛 COMException
    // （"cannot be shown while another is open"）：闸门串行化 + 重入直接跳过。
    private bool _errorDialogShowing;

    private async Task ShowErrorAsync(string message)
    {
        if (_errorDialogShowing)
        {
            return;
        }
        _errorDialogShowing = true;
        try
        {
            ContentDialog dialog = new()
            {
                Title = "Blade²",
                Content = message,
                CloseButtonText = "确定",
                XamlRoot = Content.XamlRoot,
            };
            _ = await dialog.ShowAsync();
        }
        catch (Exception) { }
        finally { _errorDialogShowing = false; }
    }

    // ---------------- 顶条搜索（session/search RPC） ----------------

    /// <summary>
    /// 顶条检索：内核全文检索（session/search，args = {request:{query}}）与本地
    /// 标题匹配并行合并。内核只检索消息内容（user/assistant message），会话名
    /// 由本地标题表补齐——两者缺一都会漏掉一侧（标题里独有的词、或只在消息里
    /// 的词）。内核未开启全文检索（session search is disabled）或检索失败时，
    /// 静默回落为纯标题匹配。无任何命中时下拉显示一条"没有找到…"提示行
    /// （对齐 Win11 设置搜索空态），不再弹独立状态面板。
    /// 输入按 220ms 防抖：AutoSuggestBox 每键一次 TextChanged，直接打 RPC 会打爆内核。
    /// </summary>
    private async void OnSearchBoxChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        if (args.Reason != AutoSuggestionBoxTextChangeReason.UserInput)
        {
            return;
        }
        var text = sender.Text.Trim();
        if (text.Length == 0 || _rpc is null)
        {
            _searchSeq++; // 作废在途检索：清空后不再回填建议
            sender.ItemsSource = null;
            return;
        }
        var seq = ++_searchSeq;
        await Task.Delay(220);
        if (seq != _searchSeq)
        {
            return; // 已有更新的输入，丢弃本次
        }

        List<SearchHitVm> kernelHits;
        try
        {
            var value = await _rpc.CallOkAsync("session/search", new { request = new { query = text } });
            kernelHits = KernelSearchHits(value);
        }
        catch (DshRpcException ex) when (IsSearchDisabled(ex))
        {
            kernelHits = new List<SearchHitVm>(); // 内核未开启全文检索
        }
        catch (Exception)
        {
            kernelHits = new List<SearchHitVm>(); // 检索失败：静默回落，不叠加错误面板
        }

        // 内核内容命中在前（带片段与相关性排序），标题命中按 id 去重后补在后。
        var hits = MergeHits(kernelHits, LocalTitleHits(text));

        if (seq != _searchSeq)
        {
            return; // 响应后到：不覆盖新查询的建议列表
        }
        sender.ItemsSource = hits.Count > 0
            ? hits
            : new List<SearchHitVm> { new() { Title = LF("没有找到与 {0} 相关的结果", text), IsNotice = true } };
    }

    /// <summary>内核内容命中 + 本地标题命中合并：内核结果在前，标题命中按会话 id 去重。</summary>
    private static List<SearchHitVm> MergeHits(List<SearchHitVm> kernelHits, List<SearchHitVm> titleHits)
    {
        var seen = new HashSet<string>(kernelHits.Select(h => h.SessionId), StringComparer.Ordinal);
        var merged = new List<SearchHitVm>(kernelHits);
        foreach (var h in titleHits)
        {
            if (h.SessionId.Length > 0 && seen.Add(h.SessionId))
            {
                merged.Add(h);
            }
        }
        return merged;
    }

    /// <summary>session/search 的 value → 建议项（{items:[{sessionId,snippet}],hasMore}）。
    /// 标题取本地会话表（检索结果只带 id 与片段）；新增 hasMore 提示见调用方。</summary>
    private List<SearchHitVm> KernelSearchHits(JsonElement value)
    {
        var hits = new List<SearchHitVm>();
        if (!value.TryGetProperty("items", out var items) || items.ValueKind != JsonValueKind.Array)
        {
            return hits;
        }
        foreach (var item in items.EnumerateArray())
        {
            var sid = item.TryGetProperty("sessionId", out var s) ? s.GetString() ?? "" : "";
            if (sid.Length == 0)
            {
                continue;
            }
            var snippet = item.TryGetProperty("snippet", out var sn) && sn.ValueKind == JsonValueKind.String ? sn.GetString() ?? "" : "";
            hits.Add(new SearchHitVm
            {
                SessionId = sid,
                Title = _sessions.FirstOrDefault(x => x.SessionId == sid)?.Title ?? sid,
                Snippet = snippet,
                FromKernel = true,
            });
        }
        return hits;
    }

    /// <summary>本地标题匹配（内核检索不可用时的回退，语义与内核检索语义一致）。</summary>
    private List<SearchHitVm> LocalTitleHits(string text) =>
        _sessions
            .Where(s => s.Title.Contains(text, StringComparison.OrdinalIgnoreCase))
            .Select(s => new SearchHitVm
            {
                SessionId = s.SessionId,
                Title = s.Title,
                Snippet = string.IsNullOrEmpty(s.Subtitle) ? L("本地标题匹配") : s.Subtitle,
                FromKernel = false,
            })
            .ToList();

    /// <summary>内核"全文检索未启用"的判定：DshRpcException 只带 code/message（details 不回流），
    /// 因此按内核源码里的稳定文案匹配（SESSION_QUERY_SEARCH_DISABLED 的说法见
    /// dsh-session-query-sqlite/lib/index.js refuseSearch）。</summary>
    private static bool IsSearchDisabled(DshRpcException ex) =>
        ex.Message.Contains("session search is disabled", StringComparison.OrdinalIgnoreCase) ||
        ex.Message.Contains("SESSION_QUERY_SEARCH_DISABLED", StringComparison.Ordinal);

    private async void OnSearchBoxQuerySubmitted(AutoSuggestBox sender, AutoSuggestBoxQuerySubmittedEventArgs args)
    {
        var query = args.QueryText.Trim();
        if (query.Length == 0 || _rpc is null)
        {
            return;
        }
        // 选中建议项：内核命中直接带会话 id（可能不在本地列表里，如未加载的历史会话）
        if (args.ChosenSuggestion is SearchHitVm notice && notice.IsNotice)
        {
            return; // 无结果提示行：不可选中，原查询留在输入框
        }
        SessionVm? vm = null;
        if (args.ChosenSuggestion is SearchHitVm hit && hit.SessionId.Length > 0)
        {
            vm = _sessions.FirstOrDefault(s => s.SessionId == hit.SessionId)
                 ?? new SessionVm { SessionId = hit.SessionId, Title = hit.Title };
        }
        vm ??= _sessions.FirstOrDefault(s => s.Title.Contains(query, StringComparison.OrdinalIgnoreCase));
        if (vm is null)
        {
            return; // 下拉里已给出"没有找到…"提示行，这里不再另弹面板
        }
        Volatile.Write(ref _activeSessionId, vm.SessionId);
        await OpenSessionAsync(vm);
    }

    /// <summary>AutoSuggestBox 的圆角不传导内层 TextBox：视觉树遍历单独设胶囊。
    /// 前导放大镜是叠在左侧的 FontIcon（AutoSuggestBox 的 QueryIcon 只能落右端），
    /// 因此这里同时把内层 TextBox 的左侧内边距让出来，占位文字不会压到图标上。
    /// 高度与半径都由调用方按**实测**控件高给出：写死令牌会在换字号/换 DPI 后失真。</summary>
    private static void RoundInnerSearchTextBox(DependencyObject parent, double height, double radius)
    {
        var count = Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChildrenCount(parent);
        for (var i = 0; i < count; i++)
        {
            var child = Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChild(parent, i);
            if (child is TextBox tb)
            {
                tb.CornerRadius = new CornerRadius(radius, radius, radius, radius);
                tb.MinHeight = height;
                // 内层 TextBox 不画自己的边界：模板描边与「浮起线」（TextControlElevationBorder*）
                // 都长在 BorderThickness 上，置 0 后两者一起消失，胶囊外观全交给 SearchPillSurface。
                tb.BorderThickness = new Thickness(0);
                tb.Background = null;
                // 文字与光标垂直居中：上下内边距归零 + 内容垂直居中，
                // 占位文字与已输入文字落在同一条中线上（不再被默认基线顶高裁切）。
                tb.VerticalContentAlignment = VerticalAlignment.Center;
                // 左侧让出放大镜的位置（图标 14 + 左外边距 12 + 图标与文字间距 8）
                tb.Padding = new Thickness(
                    TokenDouble("GlyphSizeBody", 14) + TokenDouble("Space12", 12) + TokenDouble("Space8", 8),
                    0, TokenDouble("Space12", 12), 0);
                continue;
            }
            RoundInnerSearchTextBox(child, height, radius);
        }
    }

    /// <summary>搜索框正椭圆（第 4 条）：外层 AutoSuggestBox 与内层 TextBox 的圆角同时取
    /// **实测控件高 / 2**，两端严格半圆；文字内边距上下归零并垂直居中，占位与光标同高。
    /// 不在 XAML 写死半径的理由：高度随字号/DPI/换行变化，令牌推导的常量一旦小于高/2
    /// 就不再是正椭圆（实测症状：小圆角矩形 + 内层下边框浮起线）。</summary>
    private void ApplySearchPill()
    {
        try
        {
            var h = SearchBox.ActualHeight;
            if (!double.IsFinite(h) || h <= 0)
            {
                h = TokenDouble("TouchTargetSize", 32);
            }
            var r = h / 2.0;
            SearchBox.CornerRadius = new CornerRadius(r, r, r, r);
            SearchPillSurface.CornerRadius = new CornerRadius(r, r, r, r);
            RoundInnerSearchTextBox(SearchBox, h, r);
            _searchPillHeight = h;
            _searchPillRadius = r;
        }
        catch (Exception) { } // 视觉细节失败不致命，最差退回默认圆角
    }

    /// <summary>最近一次实测的搜索框高度 / 圆角（供探针与报告核对 R == H/2）。</summary>
    private double _searchPillHeight;
    private double _searchPillRadius;

    /// <summary>
    /// 输入卡聚焦时的视觉处理（第 5 条）：**什么都不改**。
    ///
    /// 需求是"聚焦不得出现高亮描边/背景变色/光晕"，并"保留 caret、键盘焦点仍可感知"。
    /// 这里只保留 caret 作为焦点提示（文本框的规范焦点指示就是 caret，它对键盘焦点逐一对应、
    /// 且不是颜色语义）。三种"额外提示"都被实测否掉：
    ///   · 卡描边转强调色（改动前的实现）——正是要移除的颜色反应；
    ///   · 描边加粗 + 内边距等量扣回——ComposerCardPadding 的下内边距是 0，
    ///     扣不动，卡高会多 1px，导致整块输入区连同字形抗锯齿整体位移
    ///     （实测：聚焦前后 53418 像素差异、最大通道差 226，集中在动作行与模型胶囊的字形上）；
    ///   · 额外叠一圈同色描边——像素级无颜色差异但仍是"多出来的一圈描边"，语义上仍是高亮描边。
    /// 判定口径与实测证据见 artifacts/verify-input-focus.log。
    /// </summary>
    private void SetComposerFocusVisual(bool focused)
    {
        try
        {
            // 幂等占位：保持焦点事件的接线与语义（后续若要加零布局影响的提示，挂在这里）
            _ = focused;
        }
        catch (Exception) { } // 焦点提示失败不致命
    }

    // ---------------- 使用统计（统计面板；数据全部取自内核，图表全部自绘） ----------------
    //
    // 数据源（内核侧实测，不是推测）：
    //   1) session/list（_request 分页）→ 会话清单：sessionId / updatedAt / cwd / blank，
    //      以及 projections.asOfSeq（该会话 journal 的当前游标，session/page 的 throughSeq 用它）。
    //   2) session/page（{address,throughSeq,maxMessages}）→ 原始 journal 事件。
    //      只有 assistant/message.data.usage 带真实 token 数，且同一条记录带
    //      data.message.source.{provider,model} —— 这是内核唯一能「按模型聚合」的路径：
    //      tokenUsage 投影只有 4 个桶、没有 model 维度，内核也没有任何 usage/stats/metrics RPC。
    //   3) turn/start 与 turn/end 的 time → 每会话的对话累计时长（「最长聊天时长」的口径）。
    //
    // 口径（与内核 token-meter 的 4 桶一致）：
    //   token 数 = usage.totalTokens（缺失时 = inputTokens + outputTokens + cacheReadTokens + cacheWriteTokens）
    //   其中 inputTokens 是「未命中缓存的输入」，缓存命中走 cacheReadTokens。
    //   日期分桶按本地时区。
    //
    // 明确不可用项（内核没有对应数据，界面标注占位而不编造）：
    //   · 费用/金额：内核不落 cost 字段，任何 RPC 都不返回 → 不做金额 KPI。
    //   · 按模型的精确「输出/输入」拆分：journal 有，但按模型只保证 totalTokens 的拆分口径。

    /// <summary>日期键（本地时区 yyyy-MM-dd）→ 当日 token 合计。</summary>
    private readonly SortedDictionary<string, long> _statsDayTotals = new(StringComparer.Ordinal);
    /// <summary>日期键 → (模型 → token)，趋势图的多曲线。</summary>
    private readonly Dictionary<string, Dictionary<string, long>> _statsDayModel = new(StringComparer.Ordinal);
    /// <summary>会话 → 用量/时长（最长聊天时长 KPI）。</summary>
    private readonly List<SessionUsageVm> _statsSessionUsage = new();
    private int _statsUsageMessages;      // 累计计入的 assistant/message 条数（口径自证）
    private int _statsSessionsScanned;
    private int _statsSessionsSkipped;    // 空会话 / 无 journal 游标，未参与聚合
    private int _statsPagesCapped;        // 触到单会话分页上限的会话数（部分数据，界面标注）
    private bool _statsLoaded;
    private bool _statsLoading;
    /// <summary>实时刷新防抖计时器：内核事件到达后 2s 无新事件才触发一次全量重算。</summary>
    private Microsoft.UI.Dispatching.DispatcherQueueTimer? _statsLiveTimer;
    /// <summary>全量聚合途中又到达事件：本轮结束后补一次重算（实时刷新不丢数据）。</summary>
    private bool _statsReloadPending;
    /// <summary>上次加载后用量数据变过（事件到达即置位）：决定切回统计页时是否重拉内核。</summary>
    private bool _statsDirty;
    /// <summary>上一次应用自适应布局时的输入区宽度（幂等判断，避免布局↔重排互相触发）。</summary>
    private double _composerLayoutWidth = -1;
    private string _statsHeatMetric = "day";
    private int _statsRangeDays = 7;

    /// <summary>壳内建分区「使用统计」的渲染（第 2 条）：不重建任何图表，直接把既有元素实例
    /// （StatsHeaderRow 徽章行 + StatsBody 面板）从 XAML 里的隐藏容器搬到设置页内容列。
    /// 元素实例与其 x:Name 绑定不变，取数与 Canvas 自绘代码零改动，数值与迁移前一一对应。</summary>
    // ---------------- 设置分区「记忆」 ----------------

    /// <summary>JSONL 原文编辑器（分区重渲染时重建；离开分区别忘 Dispose watcher / 落盘未存修改）。</summary>
    private TextBox? _memoryRawBox;
    /// <summary>编辑器下方状态行：行数 + 保存状态。</summary>
    private TextBlock? _memorySaveState;
    /// <summary>有未保存修改：watcher 不得覆盖用户正在编辑的内容。</summary>
    private bool _memoryDirty;
    /// <summary>程序化写 Text 时抑制 TextChanged（否则会把「载入」误判成用户修改）。</summary>
    private bool _memorySuppressChange;
    /// <summary>状态行的保存注记（错误信息也走这里，保留 dirty 让用户继续改）。</summary>
    private string _memorySaveNote = "";
    private FileSystemWatcher? _memoryWatcher;
    private Microsoft.UI.Dispatching.DispatcherQueueTimer? _memorySaveTimer;
    private Microsoft.UI.Dispatching.DispatcherQueueTimer? _memoryRefreshTimer;
    /// <summary>编辑器草稿（归窗口，同自定义指令 _instructionsDraft）：分区重渲染只换视图，
    /// 不重读磁盘——用户改到一半被坏行挡住没存成，切走再回来内容还在，不会凭空丢失。
    /// null = 尚未载入过。保存成功时与 _memoryLoaded 一起落定。</summary>
    private string? _memoryDraft;
    /// <summary>上次成功保存/载入的内容：草稿与它相同即视为无修改（改回原样撤销待保存状态，
    /// 与自定义指令的 dirty 判定同口径），防抖计时器也不会再空转。</summary>
    private string _memoryLoaded = "";

    /// <summary>壳内建分区「记忆」：Blade² 默认插件组合里记忆服务器的开关与状态。
    /// 记忆由内核 dsh-mcp-client 挂载 MCP 参考记忆服务器提供（官方 examples/mcp-memory
    /// 参考配置的接入形态）。开关编辑内核 profile patch 文件
    /// （$DSH_HOME/profiles/web/cordis.patch.yml）中的标准 disabled 标志，记忆能力
    /// 完全由内核组合树定义；profile patchReload=live 让改动被内核热重载即时生效。
    /// 内容卡直接编辑同一 JSONL 原文（server-memory 每次调用重读文件，天然实时一致）。</summary>
    private void RenderMemorySection()
    {
        SettingsHost.Children.Add(MakeSectionDesc(
            L("记忆通过内核 dsh-mcp-client 挂载 MCP 参考记忆服务器（@modelcontextprotocol/server-memory），模型可跨会话写入与召回信息。开关编辑内核 profile patch 的 disabled 标志，由内核热重载即时生效。")));

        // 开关卡：与「电脑控制 / 浏览器控制」同款（文字在左、开关贴右缘）
        AddMountToggleCard(
            DshPluginBootstrap.MemoryMountId,
            L("持久记忆"),
            L("关闭后模型不再写入或读取记忆；已有记忆文件保留不动。"),
            "Setting_memory_enabled", L("启用持久记忆"));

        // 存储卡：服务器数据文件位置（与挂载条目 env 同源；内容卡直接读写它）
        var storeCard = NewCard(L("记忆存储"), null);
        storeCard.Children.Add(new TextBlock
        {
            Text = L("知识图谱 JSONL（由记忆服务器自身管理，删除即清空记忆）："),
            Style = AppStyle("CardDescriptionTextStyle"),
        });
        storeCard.Children.Add(new TextBlock
        {
            Text = DshPluginBootstrap.MemoryFilePath(),
            Style = AppStyle("CodeTextStyle"),
            TextWrapping = TextWrapping.Wrap,
        });

        // 内容卡：JSONL 原文直接编辑 + 自动保存
        RenderMemoryContentCard();
        AlignSettingsHeader();
    }

    /// <summary>挂载条目的状态文案（安装/挂载/禁用三态，插件页与各能力分区共用）。</summary>
    private string MountStatusText(DshPluginBootstrap.PluginStatus s) => (s.Installed, s.Mounted, s.Disabled) switch
    {
        (true, true, false) => L("已启用"),
        (true, true, true) => L("默认停用（需先安装 Cua Driver）"),
        (true, false, _) => L("已安装，未挂载"),
        (false, _, _) => L("未安装（离线？启动后自动重试）"),
    };

    // ---------------- 电脑控制 / 浏览器控制（壳内建分区 · 默认插件挂载开关） ----------------

    /// <summary>壳内建分区「电脑控制」：Blade² 默认插件组合里 computer-use 注册表与
    /// Cua Driver provider 两个挂载条目的开关。能力完全由内核组合树定义：注册表持有
    /// provider 独占槽位，provider（cua-driver）真正提供截图/鼠标/键盘工具。开关编辑内核
    /// profile patch（$DSH_HOME/profiles/web/cordis.patch.yml）的 disabled 标志，
    /// patchReload=live 使改动被内核热重载即时生效。总开关关闭时从开关失去意义，随置灰。</summary>
    private void RenderComputerControlSection()
    {
        SettingsHost.Children.Add(MakeSectionDesc(
            L("电脑控制由内核 dsh-computer-use 注册表与 Cua Driver provider（@deepseek-ai/dsh-experimental-computer-use-cua-driver-mcp）提供：模型可截图并操作鼠标、键盘完成桌面任务。开关编辑内核 profile patch 的 disabled 标志，由内核热重载即时生效。")));

        var registry = AddMountToggleCard(
            "computer-use-registry",
            L("电脑控制"),
            L("开启后模型获得桌面操作能力（截图、鼠标、键盘）；关闭后相关工具从模型视野移除。"),
            "Setting_computer-control_enabled", L("启用电脑控制"));
        var provider = AddMountToggleCard(
            "computer-use-cua-mcp",
            L("Cua Driver（桌面驱动）"),
            L("电脑操作的执行驱动：需在本机安装 cua-driver 命令行工具后启用，默认停用。"),
            "Setting_computer-use-cua-driver_enabled", L("启用 Cua Driver 桌面驱动"));
        // 总开关关闭时 provider 无槽位可注：随主开关置灰，避免无意义状态。
        provider.IsEnabled = registry.IsOn;
        registry.Toggled += (_, _) => provider.IsEnabled = registry.IsOn;

        RenderMountStatusCard("computer-use");
    }

    /// <summary>壳内建分区「浏览器控制」：默认插件组合里 browser-use 注册表与 Playwright
    /// provider 两个挂载条目的开关（provider 随包安装，launch 模式无头运行，默认启用）。
    /// 开关机制同「电脑控制」：编辑内核 profile patch 的 disabled 标志，内核热重载生效。</summary>
    private void RenderBrowserControlSection()
    {
        SettingsHost.Children.Add(MakeSectionDesc(
            L("浏览器控制由内核 dsh-browser-use 注册表与 Playwright provider（@deepseek-ai/dsh-experimental-browser-use-playwright-mcp）提供：模型可打开浏览器完成网页任务。开关编辑内核 profile patch 的 disabled 标志，由内核热重载即时生效。")));

        var registry = AddMountToggleCard(
            "browser-use-registry",
            L("浏览器控制"),
            L("开启后模型获得浏览器操作能力（打开网页、点击、填写、读取内容）；关闭后相关工具从模型视野移除。"),
            "Setting_browser-control_enabled", L("启用浏览器控制"));
        var provider = AddMountToggleCard(
            "browser-use-playwright",
            L("Playwright（浏览器驱动）"),
            L("浏览器操作的执行驱动：随包安装，launch 模式无头运行，默认启用。"),
            "Setting_browser-use-playwright_enabled", L("启用 Playwright 浏览器驱动"));
        provider.IsEnabled = registry.IsOn;
        registry.Toggled += (_, _) => provider.IsEnabled = registry.IsOn;

        RenderMountStatusCard("browser-use");
    }

    // ---------------- 自动审批（并入「插件」分区 · auto-approve 预设开关 + 判定模型） ----------------

    /// <summary>「插件」分区里的自动审批块（原独立分区，入口撤下后内容整体并入此处）：
    /// dsh-approval-gate 插件的 auto-approve 权限预设开关、判定模型选择与切换后果提示，
    /// 挂在默认插件卡上方。
    /// 该插件挂在审批瀑布最前，仅当会话权限预设为 auto-approve 时工作（模型预判
    /// 写入/命令是否不可回补：安全自动批准，硬类别转人工）。开关不碰插件挂载（bundle 插件，
    /// 由引导器每次启动幂等选入），而是增删 profile patch 里 permission 条目的 auto-approve
    /// 预设块：关闭后该预设从权限选择器消失，正在使用它的会话落到 custom 预设，
    /// 审批瀑布回落到默认人工确认。patchReload=live 使改动被内核热重载即时生效。</summary>
    private async Task AddAutoApprovalCardsAsync(StackPanel host)
    {
        host.Children.Add(MakeSectionDesc(
            L("自动审批由内核插件 dsh-approval-gate 提供：当会话权限预设切到「自动审批」时，由判定模型预判每次写入/命令是否不可回补——安全则自动批准，涉及删除、凭据、系统配置等硬类别转人工确认。开关增删内核 profile patch 里的 auto-approve 预设，判定模型写在默认模型设置里，两者都由内核热重载即时生效。")));

        AddAutoApprovalToggleCard(host);

        await AddAutoApprovalModelCardAsync(host);

        // 提示卡：关掉之后会话里正在用这个预设会怎样（用户最关心的后果）
        var card = NewCardIn(host, L("切换后会发生什么"), null);
        card.Children.Add(new TextBlock
        {
            Text = L("开启：权限选择器（输入区左下角）里出现「自动审批」，新会话可选用它。\n" +
                     "关闭：该预设从选择器消失；正在使用它的会话回落到自定义权限，审批恢复人工确认。\n" +
                     "判定记录与学习状态不受影响，重新开启后继续生效。"),
            Style = AppStyle("CardDescriptionTextStyle"),
            TextWrapping = TextWrapping.Wrap,
        });
    }

    /// <summary>
    /// auto-approve 预设开关卡（与「记忆/电脑控制」的挂载开关同构：标题+说明在左、开关在右）。
    /// 写入失败（patch 文件不可写、permission 条目缺 presets 键）时开关回滚，保持与 patch 一致。
    /// </summary>
    private void AddAutoApprovalToggleCard(StackPanel host)
    {
        var card = NewCardIn(host);
        var row = new Grid { HorizontalAlignment = HorizontalAlignment.Stretch };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var text = new StackPanel
        {
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, Sp12, 0),
        };
        text.Children.Add(new TextBlock { Text = L("自动审批"), Style = AppStyle("BodyStrongTextStyle") });
        text.Children.Add(new TextBlock
        {
            Text = L("开启后权限模式里多出「自动审批」：判定模型预判越界请求，安全自动批准、有风险转人工。"),
            Style = AppStyle("CardDescriptionTextStyle"),
            TextWrapping = TextWrapping.Wrap,
        });
        Grid.SetColumn(text, 0);
        row.Children.Add(text);
        var toggle = Aut(new ToggleSwitch
        {
            IsOn = DshPluginBootstrap.IsAutoApprovePresetPresent(DataHome),
            OnContent = L("开"),
            OffContent = L("关"),
            VerticalAlignment = VerticalAlignment.Center,
            Style = AppStyle("SettingsToggleSwitchStyle"),
        },
            "Setting_auto-approval_enabled", L("启用自动审批"));
        Grid.SetColumn(toggle, 1);
        row.Children.Add(toggle);
        card.Children.Add(row);
        var state = new TextBlock
        {
            Text = AutoApprovalStateLine(),
            Style = AppStyle("CardDescriptionTextStyle"),
        };
        var syncing = false; // 回滚 IsOn 会重入 Toggled：用哨兵避免递归
        toggle.Toggled += (_, _) =>
        {
            if (syncing)
            {
                return;
            }
            try
            {
                if (DshPluginBootstrap.SetAutoApprovePresetPresent(DataHome, toggle.IsOn))
                {
                    state.Text = AutoApprovalStateLine();
                    return;
                }
                syncing = true;
                toggle.IsOn = !toggle.IsOn;
            }
            catch (Exception) { } // 事件入口兜底（async void 教训）
            finally
            {
                syncing = false;
            }
        };
        card.Children.Add(state);
    }

    /// <summary>auto-approve 预设的状态行：patch 里预设块的当前态（内核热重载后生效）。</summary>
    private string AutoApprovalStateLine()
        => DshPluginBootstrap.IsAutoApprovePresetPresent(DataHome)
            ? L("已启用（内核热重载后生效）")
            : L("已停用");

    /// <summary>判定模型卡：自动审批每次预判所用的模型。
    /// 内核 dsh-approval-gate 没有独立的模型配置项——判定时它读 agentDefaultModel.currentSelection()
    /// （读不到才回落 deepseek-v4-flash），所以这里切的就是「默认模型」本身，与输入区模型选择器
    /// 经 session/selectModel 落的是同一个设置。切换对门控即时生效（settings 改动内核直接可见），
    /// 新会话的默认模型也随之改变；已在会话里选过模型的会话沿用自己那份，不受影响。</summary>
    private async Task AddAutoApprovalModelCardAsync(StackPanel host)
    {
        var card = NewCardIn(
            host,
            L("判定模型"),
            L("自动审批每次预判写入/命令是否不可回补时所用的模型，选项来自内核模型目录。"));
        if (_rpc is null)
        {
            return;
        }

        var options = new List<(string Provider, string Id, string Name)>();
        try
        {
            var catalog = await _rpc.CallOkAsync("session/modelCatalog", new { });
            if (catalog.TryGetProperty("groups", out var groups) && groups.ValueKind == JsonValueKind.Array)
            {
                foreach (var group in groups.EnumerateArray())
                {
                    var provider = group.TryGetProperty("id", out var gid) ? gid.GetString() ?? "" : "";
                    if (!group.TryGetProperty("models", out var models) || models.ValueKind != JsonValueKind.Array)
                    {
                        continue;
                    }
                    foreach (var model in models.EnumerateArray())
                    {
                        var id = model.TryGetProperty("id", out var mid) ? mid.GetString() ?? "" : "";
                        if (id.Length == 0)
                        {
                            continue;
                        }
                        var name = model.TryGetProperty("name", out var mname) ? mname.GetString() ?? id : id;
                        options.Add((provider, id, name));
                    }
                }
            }
        }
        catch (Exception ex) when (ex is DshRpcException or InvalidOperationException or HttpRequestException or TaskCanceledException)
        {
            card.Children.Add(new TextBlock
            {
                Text = LF("模型目录不可用：{0}", ex.Message),
                Style = AppStyle("CardDescriptionTextStyle"),
                TextWrapping = TextWrapping.Wrap,
            });
            return;
        }

        var currentProvider = NsString("agent-default-model", "provider", "");
        var currentModel = NsString("agent-default-model", "model", "");
        var box = new ComboBox { MinWidth = TokenDouble("FieldMinWidth", 200), MaxDropDownHeight = 320 };
        Aut(box, "Setting_auto-approval_model", L("自动审批判定模型"));
        if (currentModel.Length > 0 && !options.Any(o => o.Id == currentModel))
        {
            // 目录里没有当前默认模型（自建提供方，或该提供方凭据未配置）：留一项原值，
            // 否则下拉显示空白，用户看不出现在用的是什么。
            options.Insert(0, (currentProvider, currentModel, currentModel));
        }
        foreach (var (_, id, name) in options)
        {
            box.Items.Add(new ComboBoxItem { Content = name, Tag = id });
            if (id == currentModel)
            {
                box.SelectedItem = box.Items[^1];
            }
        }
        // 先赋初值后挂事件：初始 SelectionChanged 不落这里
        box.SelectionChanged += (_, _) =>
        {
            if (box.SelectedItem is not ComboBoxItem { Tag: string id } || id.Length == 0)
            {
                return;
            }
            var picked = options.FirstOrDefault(o => o.Id == id);
            // provider 与 model 必须成对写：门控的 currentSelection() 两个字段都读，
            // 只写一个会落到「新 model + 旧 provider」的错配组合上。
            Edit("agent-default-model", "provider", () => picked.Provider);
            Edit("agent-default-model", "model", () => id);
        };
        card.Children.Add(MakeRow(
            L("预判模型"),
            L("切换后新会话的默认模型也随之改变；已在会话里选过模型的会话不受影响。"),
            box));
    }

    /// <summary>
    /// 挂载开关卡（与「记忆」分区的开关同构）：标题+说明在左、开关在右，卡内带状态行。
    /// 开关直接改写 profile patch 里该条目的 disabled 标志；写入被拒（依赖包未安装，
    /// 写了会让内核启动失败）时开关回滚，保持与 patch 一致。
    /// </summary>
    private ToggleSwitch AddMountToggleCard(
        string mountId, string title, string desc, string automationId, string automationName)
    {
        var card = NewCard();
        // 两列 Grid：文字列 Star 吃掉剩余宽度，开关列 Auto 贴右缘。
        // 早先是水平 StackPanel + Spacer：文字列只占内容宽，开关会紧贴文字而非贴右。
        var row = new Grid { HorizontalAlignment = HorizontalAlignment.Stretch };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var text = new StackPanel
        {
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, Sp12, 0),
        };
        text.Children.Add(new TextBlock { Text = title, Style = AppStyle("BodyStrongTextStyle") });
        text.Children.Add(new TextBlock { Text = desc, Style = AppStyle("CardDescriptionTextStyle") });
        Grid.SetColumn(text, 0);
        row.Children.Add(text);
        var toggle = Aut(new ToggleSwitch
        {
            IsOn = DshPluginBootstrap.IsMountEnabled(DataHome, mountId),
            OnContent = L("开"),
            OffContent = L("关"),
            VerticalAlignment = VerticalAlignment.Center,
            Style = AppStyle("SettingsToggleSwitchStyle"),
        },
            automationId, automationName);
        Grid.SetColumn(toggle, 1);
        row.Children.Add(toggle);
        card.Children.Add(row);
        var state = new TextBlock
        {
            Text = MountStateLine(mountId),
            Style = AppStyle("CardDescriptionTextStyle"),
        };
        var syncing = false; // 回滚 IsOn 会重入 Toggled：用哨兵避免递归
        toggle.Toggled += (_, _) =>
        {
            if (syncing)
            {
                return;
            }
            try
            {
                if (DshPluginBootstrap.SetMountEnabled(DataHome, mountId, toggle.IsOn))
                {
                    state.Text = MountStateLine(mountId);
                    return;
                }
                syncing = true;
                toggle.IsOn = !toggle.IsOn;
            }
            catch (Exception) { } // 事件入口兜底（async void 教训）
            finally
            {
                syncing = false;
            }
        };
        card.Children.Add(state);
        // card 是 NewCard() 返回的内层 rows：外壳已由 NewCard 加进 SettingsHost，勿重复 Add
        return toggle;
    }

    /// <summary>挂载条目的状态行：patch 里 disabled 标志的当前态（内核热重载后生效）。</summary>
    private string MountStateLine(string mountId)
        => DshPluginBootstrap.IsMountEnabled(DataHome, mountId)
            ? L("已启用（内核热重载后生效）")
            : L("已停用");

    /// <summary>挂载状态卡：某一能力的全部挂载条目的安装/挂载/禁用状态（引导器幂等维护，只读呈现）。</summary>
    private void RenderMountStatusCard(string mountIdPrefix)
    {
        var card = NewCard(
            L("挂载状态"),
            L("Blade² 随内核插件机制默认启用；安装由引导器幂等完成，失败时下次启动自动重试。"));
        foreach (var status in DshPluginBootstrap.GetStatus(DataHome)
            .Where(s => s.Id.StartsWith(mountIdPrefix, StringComparison.Ordinal)))
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6 };
            row.Children.Add(new TextBlock
            {
                Text = MountDisplayName(status.Id),
                Style = AppStyle("BodyTextStyle"),
                VerticalAlignment = VerticalAlignment.Center,
            });
            row.Children.Add(Spacer());
            row.Children.Add(new TextBlock
            {
                Text = MountStatusText(status),
                Style = AppStyle("CardDescriptionTextStyle"),
                VerticalAlignment = VerticalAlignment.Center,
            });
            card.Children.Add(row);
        }
    }

    /// <summary>挂载条目 id → 设置页展示名（id 本身是 patch 定位标记，保持诊断可追）。</summary>
    private string MountDisplayName(string mountId) => mountId switch
    {
        "memory-reference" => L("参考记忆服务器"),
        "computer-use-registry" => L("电脑控制注册表"),
        "computer-use-cua-mcp" => L("Cua Driver 驱动"),
        "browser-use-registry" => L("浏览器控制注册表"),
        "browser-use-playwright" => L("Playwright 驱动"),
        "dsh-approval-gate" => L("自动审批"),
        "echocat-skill-panel-3.0" => L("技能面板"),
        _ => mountId,
    };

    // ---------------- 记忆内容（JSONL 原文编辑 + 自动保存） ----------------

    /// <summary>
    /// 记忆内容卡：直接编辑 JSONL 原文。server-memory 每次工具调用都重读文件，
    /// 停手约 1 秒后原子写盘（临时文件 + Move，与 server 的 saveGraph 同策略），
    /// 模型写入时由 FileSystemWatcher 自动重新载入（用户正在编辑时不覆盖）。
    /// </summary>
    private void RenderMemoryContentCard()
    {
        var rows = NewCard(
            L("记忆内容"),
            L("直接编辑记忆文件（JSONL，每行一个实体或关系），停止输入后自动保存；模型写入时自动重新载入。"));

        _memoryRawBox = new TextBox
        {
            AcceptsReturn = true,
            TextWrapping = TextWrapping.NoWrap,
            MinHeight = 320,
            MaxHeight = 560,
            FontFamily = new FontFamily("Consolas"),
            FontSize = 12,
        };
        Aut(_memoryRawBox, "MemoryContentEditor", L("记忆文件内容"));
        _memoryRawBox.TextChanged += (_, _) => OnMemoryRawTextChanged();
        // 失焦即落盘：鼠标点到别处（同页其他控件/回聊天页）不再等 1s 防抖。
        _memoryRawBox.LostFocus += (_, _) => FlushMemoryPendingSave();
        rows.Children.Add(_memoryRawBox);

        _memorySaveState = new TextBlock
        {
            Style = AppStyle("CardDescriptionTextStyle"),
            TextWrapping = TextWrapping.Wrap,
        };
        rows.Children.Add(_memorySaveState);

        StartMemoryWatcher();
        LoadMemoryRaw();
    }

    /// <summary>从文件载入原文到编辑器（程序化写 Text，不算用户修改）。
    /// 草稿归窗口（同自定义指令 _instructionsDraft）：有未保存修改时切分区再回来，
    /// 内容原样还在；草稿已落定则以磁盘为准，模型新写的记忆照常进来。</summary>
    private void LoadMemoryRaw()
    {
        if (_memoryRawBox is null)
        {
            return;
        }
        if (_memoryDraft is null || !_memoryDirty)
        {
            _memoryDraft = DshPluginBootstrap.ReadMemoryText();
            _memoryLoaded = _memoryDraft;
        }
        _memoryDirty = _memoryDraft != _memoryLoaded;
        _memorySuppressChange = true;
        try
        {
            _memoryRawBox.Text = _memoryDraft;
        }
        finally
        {
            _memorySuppressChange = false;
        }
        if (!_memoryDirty)
        {
            _memorySaveNote = L("停止输入后自动保存");
        }
        UpdateMemoryStateLine();
    }

    /// <summary>外部变更（模型写入）整份重载：草稿与磁盘落定为一致，非 dirty 时才走到这。</summary>
    private void ReloadMemoryFromDisk()
    {
        var text = DshPluginBootstrap.ReadMemoryText();
        _memoryDraft = text;
        _memoryLoaded = text;
        _memoryDirty = false;
        if (_memoryRawBox is not null)
        {
            _memorySuppressChange = true;
            try
            {
                _memoryRawBox.Text = text;
            }
            finally
            {
                _memorySuppressChange = false;
            }
        }
        UpdateMemoryStateLine();
    }

    private void OnMemoryRawTextChanged()
    {
        if (_memorySuppressChange || _memoryRawBox is null)
        {
            return;
        }
        _memoryDraft = _memoryRawBox.Text ?? "";
        // 与上次落定内容比对：改回原样即撤销待保存状态（unconditional dirty 会让
        // 改回原内容后永远显示"有未保存的修改"，防抖计时器也一直空转）。
        _memoryDirty = _memoryDraft != _memoryLoaded;
        UpdateMemoryStateLine();
        // 防抖：停手约 1 秒才落盘，避免逐字符写文件
        if (_memorySaveTimer is { } timer)
        {
            timer.Stop();
            if (_memoryDirty)
            {
                timer.Start();
            }
        }
    }

    /// <summary>立即保存（防抖到点、离开分区时调用）。坏行挡住保存——server 的 loadGraph 遇坏行会整体抛错。
    /// 被挡时 dirty 保持、草稿留住内容：用户改对再存，期间切分区也不会丢。</summary>
    private void SaveMemoryRaw()
    {
        if (_memoryRawBox is null || !_memoryDirty)
        {
            return;
        }
        var text = _memoryDraft ?? _memoryRawBox.Text ?? "";
        var bad = DshPluginBootstrap.MemoryBadLines(text);
        if (bad.Count > 0)
        {
            var shown = string.Join(", ", bad.Take(5));
            if (bad.Count > 5)
            {
                shown += "…";
            }
            _memorySaveNote = LF("第 {0} 行不是合法 JSON，已阻止保存。", shown);
            UpdateMemoryStateLine();
            return; // 保持 dirty：用户改对再存
        }
        var err = DshPluginBootstrap.SaveMemoryText(text);
        if (err.Length > 0)
        {
            _memorySaveNote = LF("保存失败：{0}", err);
            UpdateMemoryStateLine();
            return;
        }
        _memoryLoaded = text;
        _memoryDraft = text;
        _memoryDirty = false;
        _memorySaveNote = LF("已自动保存 · {0}", DateTime.Now.ToString("HH:mm:ss"));
        UpdateMemoryStateLine();
    }

    /// <summary>状态行：非空行数 + 保存注记。</summary>
    private void UpdateMemoryStateLine()
    {
        if (_memorySaveState is null)
        {
            return;
        }
        var lines = (_memoryDraft ?? _memoryRawBox?.Text ?? "").Split('\n').Count(l => l.Trim().Length > 0);
        var note = _memoryDirty ? L("有未保存的修改…") : _memorySaveNote;
        _memorySaveState.Text = LF("共 {0} 行 · {1}", lines, note);
    }

    /// <summary>离开分区前落盘未保存修改（防抖计时器可能还没到点）。</summary>
    private void FlushMemoryPendingSave()
    {
        try
        {
            SaveMemoryRaw();
        }
        catch (Exception ex)
        {
            System.Diagnostics.Debug.WriteLine($"[memory/save] {ex}");
        }
    }

    /// <summary>启动记忆文件监听（保存防抖 1s；外部变更刷新防抖 200ms）。</summary>
    private void StartMemoryWatcher()
    {
        try
        {
            _memoryWatcher?.Dispose();
            _memoryWatcher = null;
            var file = DshPluginBootstrap.MemoryFilePath();
            var dir = Path.GetDirectoryName(file);
            if (string.IsNullOrEmpty(dir))
            {
                return;
            }
            Directory.CreateDirectory(dir);

            var saveTimer = DispatcherQueue.CreateTimer();
            saveTimer.Interval = TimeSpan.FromMilliseconds(1000);
            saveTimer.Tick += (_, _) =>
            {
                saveTimer.Stop();
                SaveMemoryRaw();
            };
            _memorySaveTimer = saveTimer;

            var refreshTimer = DispatcherQueue.CreateTimer();
            refreshTimer.Interval = TimeSpan.FromMilliseconds(200);
            refreshTimer.Tick += (_, _) =>
            {
                refreshTimer.Stop();
                // 自己保存触发的回环：内容相同就不重载，避免编辑器光标被重置。
                // 行结尾差异（用户输入 \r\n / server 写 \n）不算内容差异。
                // 读不到文件（并发写窗口期的共享冲突）时保持原样：把一次读失败
                // 当成"文件被清空"会把编辑器内容整个抹掉。
                var onDisk = DshPluginBootstrap.ReadMemoryTextOrNull()?.Replace("\r\n", "\n");
                if (onDisk is null)
                {
                    return;
                }
                var inBox = (_memoryDraft ?? "").Replace("\r\n", "\n");
                if (onDisk == inBox)
                {
                    return;
                }
                ReloadMemoryFromDisk();
            };
            _memoryRefreshTimer = refreshTimer;

            var watcher = new FileSystemWatcher(dir, Path.GetFileName(file))
            {
                NotifyFilter = NotifyFilters.LastWrite | NotifyFilters.FileName | NotifyFilters.Size,
            };
            watcher.Changed += OnMemoryFileChanged;
            watcher.Created += OnMemoryFileChanged;
            watcher.Deleted += OnMemoryFileChanged;
            watcher.Renamed += OnMemoryFileChanged;
            watcher.EnableRaisingEvents = true;
            _memoryWatcher = watcher;
        }
        catch (Exception ex)
        {
            System.Diagnostics.Debug.WriteLine($"[memory/watch] {ex}");
        }
    }

    private void OnMemoryFileChanged(object sender, FileSystemEventArgs e)
    {
        if (_memoryDirty)
        {
            return; // 用户正在编辑：不覆盖编辑器内容
        }
        if (_memoryRefreshTimer is not { } timer)
        {
            return;
        }
        DispatcherQueue.TryEnqueue(() =>
        {
            timer.Stop();
            timer.Start();
        });
    }

    /// <summary>横向弹簧：在水平 StackPanel 里把后续元素推到行尾。</summary>
    private static FrameworkElement Spacer() => new Microsoft.UI.Xaml.Controls.Grid
    {
        HorizontalAlignment = HorizontalAlignment.Stretch,
        VerticalAlignment = VerticalAlignment.Stretch,
        MinWidth = Sp12,
    };

    private async Task RenderUsageSectionAsync()
    {
        // 搬移前先从原容器摘除：XAML 元素只能有一个父级（重复 Add 会抛）。
        DetachFromParent(StatsBody);
        SettingsHost.Children.Add(StatsBody);

        StatsBody.MaxWidth = double.PositiveInfinity;
        StatsBody.HorizontalAlignment = HorizontalAlignment.Stretch;

        // 数据加载：上次加载后数据变过才重拉内核（force:false = 命中缓存不重复拉取）
        await LoadUsageStatsAsync(force: _statsDirty);
        AlignSettingsHeader();
    }

    /// <summary>把元素从当前父级摘下来（换父级前必须做；未挂载时为 no-op）。</summary>
    private static void DetachFromParent(FrameworkElement element)
    {
        switch (element.Parent)
        {
            case Panel panel:
                panel.Children.Remove(element);
                break;
            case Border border:
                border.Child = null;
                break;
            case ContentControl content:
                content.Content = null;
                break;
        }
    }

    /// <summary>使用统计实时刷新：用量/时长口径变化的内核事件到达后防抖重算（手动「刷新」已移除）。</summary>
    private void RequestStatsLiveRefresh()
    {
        // 数据变过就记账：分区不可见时不重算，但切回去时要按它决定是否重拉（不显陈旧值）。
        _statsDirty = true;
        if (!IsUsageSectionVisible())
        {
            return;
        }
        if (_statsLiveTimer is not { } timer)
        {
            timer = DispatcherQueue.CreateTimer();
            timer.Interval = TimeSpan.FromSeconds(2);
            timer.IsRepeating = false;
            timer.Tick += (_, _) =>
            {
                timer.Stop();
                // 倒计时期间可能已切走：离开统计页就不该再为它翻一遍 journal。
                if (!IsUsageSectionVisible())
                {
                    return;
                }
                _ = LoadUsageStatsAsync(force: true);
            };
            _statsLiveTimer = timer;
        }
        // Stop + Start = 重置倒计时：一轮里的连续事件只在末尾触发一次全量重算。
        timer.Stop();
        timer.Start();
    }

    /// <summary>统计分区当前是否摆在设置页内容列里（决定实时刷新要不要真去翻 journal）。</summary>
    private bool IsUsageSectionVisible()
        => SettingsPage.Visibility == Visibility.Visible && _settingsActiveSection == UsageSectionId;

    private void OnStatsHeatMetricChanged(SelectorBar sender, SelectorBarSelectionChangedEventArgs args)
    {
        try
        {
            if (sender.SelectedItem?.Tag is string tag)
            {
                _statsHeatMetric = tag;
                RenderStatsHeatmap();
            }
        }
        catch (Exception) { } // 事件入口兜底
    }

    private void OnStatsRangeChanged(SelectorBar sender, SelectorBarSelectionChangedEventArgs args)
    {
        try
        {
            if (sender.SelectedItem?.Tag is string tag && int.TryParse(tag, out var days))
            {
                _statsRangeDays = days;
                RenderStatsTrend();
                RenderStatsModelUsage();
            }
        }
        catch (Exception) { } // 事件入口兜底
    }

    /// <summary>
    /// 拉取并聚合使用统计。force=false 时用已缓存结果（页面反复切换不打内核）。
    /// 单次全量成本实测：78 个会话里 36 个非空，走查 36 次 session/page 约 2s（本机）。
    /// </summary>
    private async Task LoadUsageStatsAsync(bool force)
    {
        if (_rpc is null)
        {
            return;
        }
        if (_statsLoading)
        {
            // 实时刷新在上一次全量聚合途中又到事件：这一轮算完后立刻再算一轮，不丢最新数据。
            _statsReloadPending = true;
            return;
        }
        if (_statsLoaded && !force)
        {
            RenderStatsAll();
            return;
        }
        _statsLoading = true;
        try
        {
            var value = await _rpc.CallOkAsync("session/list", new { _request = new { } });
            if (!value.TryGetProperty("items", out var items) || items.ValueKind != JsonValueKind.Array)
            {
                ShowStatsInfo(InfoBarSeverity.Error, L("内核未返回会话清单（session/list），使用统计不可用。"));
                return;
            }

            _statsDayTotals.Clear();
            _statsDayModel.Clear();
            _statsSessionUsage.Clear();
            _statsUsageMessages = 0;
            _statsSessionsScanned = 0;
            _statsSessionsSkipped = 0;
            _statsPagesCapped = 0;

            foreach (var item in items.EnumerateArray())
            {
                var sid = Str(item, "sessionId");
                if (sid.Length == 0) continue;
                var blank = item.TryGetProperty("blank", out var b) && b.ValueKind == JsonValueKind.True;
                if (blank) { _statsSessionsSkipped++; continue; }
                var updatedAt = item.TryGetProperty("updatedAt", out var ua) && ua.ValueKind == JsonValueKind.Number
                    ? (long)ua.GetDouble() : 0L;
                var asOfSeq = -1L;
                if (item.TryGetProperty("projections", out var proj) && proj.ValueKind == JsonValueKind.Object &&
                    proj.TryGetProperty("asOfSeq", out var aos) && aos.ValueKind == JsonValueKind.Number)
                {
                    asOfSeq = (long)aos.GetDouble();
                }
                if (asOfSeq < 0) { _statsSessionsSkipped++; continue; } // 没有 journal 游标：无法定位分页起点
                _statsSessionsScanned++;
                await AccumulateSessionUsageAsync(sid, updatedAt, asOfSeq);
            }

            _statsLoaded = true;
            _statsDirty = false;
            // 成功聚合不再弹 InfoBar：会话/记录数与分页上限提示并入页尾口径脚注
            // （RenderStatsSourceNote），顶部横幅只留给错误与告警。
            RenderStatsAll();
        }
        catch (Exception ex)
        {
            ShowStatsInfo(InfoBarSeverity.Warning, LF("读取使用统计失败：{0}", ex.Message));
        }
        finally
        {
            _statsLoading = false;
            if (_statsReloadPending)
            {
                _statsReloadPending = false;
                _ = LoadUsageStatsAsync(force: true);
            }
        }
    }

    /// <summary>单个会话的 journal 走查：从 asOfSeq 往回翻页，累计 assistant/message 用量与 turn 时长。</summary>
    private async Task AccumulateSessionUsageAsync(string sessionId, long updatedAt, long asOfSeq)
    {
        if (_rpc is null)
        {
            return;
        }
        var through = asOfSeq;
        long minTime = long.MaxValue, maxTime = long.MinValue;
        double talkMs = 0;
        long turnStartTime = -1;
        var pages = 0;
        const int maxPages = 40; // 单会话安全上限：40 页 × 500 条消息

        while (through >= 0 && pages < maxPages)
        {
            JsonElement page;
            try
            {
                page = await _rpc.CallOkAsync("session/page", new
                {
                    request = new { address = new { kind = "session", sessionId }, throughSeq = through, maxMessages = 500 },
                });
            }
            catch (DshRpcException)
            {
                break; // 会话在两次调用之间被归档/删除：保留已聚合部分
            }
            pages++;
            if (!page.TryGetProperty("records", out var records) || records.ValueKind != JsonValueKind.Array)
            {
                break;
            }
            var count = records.GetArrayLength();
            if (count == 0)
            {
                break;
            }
            long firstSeq = -1;
            foreach (var rec in records.EnumerateArray())
            {
                if (!rec.TryGetProperty("event", out var ev) || ev.ValueKind != JsonValueKind.Object)
                {
                    continue;
                }
                if (firstSeq < 0 && ev.TryGetProperty("seq", out var sq) && sq.ValueKind == JsonValueKind.Number)
                {
                    firstSeq = (long)sq.GetDouble();
                }
                var time = ev.TryGetProperty("time", out var t) && t.ValueKind == JsonValueKind.Number ? (long)t.GetDouble() : 0L;
                if (time > 0)
                {
                    if (time < minTime) minTime = time;
                    if (time > maxTime) maxTime = time;
                }
                var type = Str(ev, "type");
                switch (type)
                {
                    case "turn/start":
                        turnStartTime = time;
                        break;
                    case "turn/end":
                        if (turnStartTime > 0 && time >= turnStartTime) talkMs += time - turnStartTime;
                        turnStartTime = -1;
                        break;
                    case "assistant/message":
                        AccumulateUsage(ev, time);
                        break;
                }
            }
            if (page.TryGetProperty("hasMore", out var hm) && hm.ValueKind == JsonValueKind.True && firstSeq > 0)
            {
                through = firstSeq - 1;
                continue;
            }
            break;
        }
        if (pages >= maxPages)
        {
            _statsPagesCapped++;
        }

        _statsSessionUsage.Add(new SessionUsageVm
        {
            SessionId = sessionId,
            UpdatedAt = updatedAt,
            SpanMs = maxTime > minTime ? maxTime - minTime : 0,
            TalkMs = talkMs,
        });
    }

    /// <summary>一条 assistant/message 的用量入账（按本地日期 + 模型双维度）。</summary>
    private void AccumulateUsage(JsonElement ev, long time)
    {
        if (!ev.TryGetProperty("data", out var data) || data.ValueKind != JsonValueKind.Object)
        {
            return;
        }
        if (!data.TryGetProperty("usage", out var usage) || usage.ValueKind != JsonValueKind.Object)
        {
            return;
        }
        var tokens = TokenOf(usage);
        if (tokens <= 0)
        {
            return;
        }
        var model = "未标注模型";
        if (data.TryGetProperty("message", out var msg) && msg.ValueKind == JsonValueKind.Object &&
            msg.TryGetProperty("source", out var src) && src.ValueKind == JsonValueKind.Object)
        {
            var provider = Str(src, "provider");
            var name = Str(src, "model");
            if (name.Length > 0)
            {
                model = provider.Length > 0 ? $"{provider}/{name}" : name;
            }
        }
        var day = DayKey(time);
        _statsUsageMessages++;
        _statsDayTotals[day] = _statsDayTotals.TryGetValue(day, out var cur) ? cur + tokens : tokens;
        if (!_statsDayModel.TryGetValue(day, out var byModel))
        {
            byModel = new Dictionary<string, long>(StringComparer.Ordinal);
            _statsDayModel[day] = byModel;
        }
        byModel[model] = byModel.TryGetValue(model, out var mcur) ? mcur + tokens : tokens;
    }

    /// <summary>事件 data 里的 usage（assistant/message 等）转 token 数。0 = 无用量记录。
    /// data.usage 缺失时返回 0，与「没有用量就不显示用量行」的展示约定一致。</summary>
    private static long UsageTokens(JsonElement data)
    {
        return data.TryGetProperty("usage", out var u) && u.ValueKind == JsonValueKind.Object
            ? TokenOf(u)
            : 0L;
    }

    /// <summary>usage 的 token 口径：优先 totalTokens，缺失时按 4 桶相加（与内核 token-meter 一致）。</summary>
    private static long TokenOf(JsonElement usage)
    {
        long Num(string name) => usage.TryGetProperty(name, out var el) && el.ValueKind == JsonValueKind.Number
            ? (long)el.GetDouble() : 0L;
        if (usage.TryGetProperty("totalTokens", out var tot) && tot.ValueKind == JsonValueKind.Number)
        {
            var v = (long)tot.GetDouble();
            if (v > 0) return v;
        }
        return Num("inputTokens") + Num("outputTokens") + Num("cacheReadTokens") + Num("cacheWriteTokens");
    }

    private static string DayKey(long epochMs)
        => DateTimeOffset.FromUnixTimeMilliseconds(epochMs).ToLocalTime().ToString("yyyy-MM-dd", CultureInfo.InvariantCulture);

    private static DateTime DayOf(string key)
        => DateTime.SpecifyKind(DateTime.ParseExact(key, "yyyy-MM-dd", CultureInfo.InvariantCulture), DateTimeKind.Local);

    // ---------------- 统计面板渲染 ----------------

    private void RenderStatsAll()
    {
        RenderStatsKpi();
        RenderStatsHeatmap();
        RenderStatsTrend();
        RenderStatsModelUsage();
        RenderStatsSourceNote();
        // 热力档位图例（0 = 空档，1..4 = 强调色 25/45/70/100%）
        var accent = ThemeBrush("AccentBrush");
        Brush[] legend = { ThemeBrush("ChartHeatEmptyBrush"), HeatBrush(accent, 1), HeatBrush(accent, 2), HeatBrush(accent, 3), HeatBrush(accent, 4) };
        var swatches = new[] { StatsHeatLegend0, StatsHeatLegend1, StatsHeatLegend2, StatsHeatLegend3, StatsHeatLegend4 };
        for (var i = 0; i < swatches.Length; i++)
        {
            swatches[i].Fill = legend[i];
        }
    }

    /// <summary>顶部 5 个 KPI。全部来自 journal 聚合；无数据时留 "—"，不编造。</summary>
    private void RenderStatsKpi()
    {
        long total = 0;
        foreach (var v in _statsDayTotals.Values) total += v;
        long peak = 0;
        var peakDay = "";
        foreach (var (day, v) in _statsDayTotals)
        {
            if (v > peak) { peak = v; peakDay = day; }
        }
        var longestTalk = _statsSessionUsage.Count == 0 ? 0 : _statsSessionUsage.Max(s => s.TalkMs);
        var (current, longest) = Streaks();

        StatsKpiTotalTokens.Text = total > 0 ? FormatTokens(total) : "—";
        AutomationProperties.SetHelpText(StatsKpiTotalTokens, total > 0
            ? LF("{0:N0} tokens（{1} 条用量记录）", total, _statsUsageMessages)
            : L("内核未返回任何 token 用量记录"));
        StatsKpiTotalTokensHint.Text = total > 0
            ? LF("{0} 条用量记录", _statsUsageMessages)
            : L("暂无用量记录");

        StatsKpiPeakTokens.Text = peak > 0 ? FormatTokens(peak) : "—";
        AutomationProperties.SetHelpText(StatsKpiPeakTokens, peak > 0
            ? LF("单日峰值：{0}，{1:N0} tokens", peakDay, peak)
            : L("内核未返回任何 token 用量记录"));
        StatsKpiPeakTokensHint.Text = peak > 0 ? LF("单日峰值 · {0}", peakDay) : L("暂无用量记录");

        // 「聊天时长」= 各 turn（turn/start → turn/end）时长之和的最大值，不是会话存活窗口
        StatsKpiLongestChat.Text = longestTalk > 0 ? FormatDuration(longestTalk) : "—";
        AutomationProperties.SetHelpText(StatsKpiLongestChat, longestTalk > 0
            ? LF("单会话对话累计时长最大值：{0}（按 turn/start→turn/end 求和）", FormatDuration(longestTalk))
            : L("内核 journal 未包含任何完整 turn（turn/start → turn/end），无法计算"));
        StatsKpiLongestChatHint.Text = longestTalk > 0
            ? LF("{0} 个会话中的最大值", _statsSessionUsage.Count)
            : L("无完整 turn 记录");

        StatsKpiCurrentStreak.Text = current > 0 ? LF("{0} 天", current) : "—";
        StatsKpiCurrentStreakHint.Text = current > 0 ? L("截至今日") : L("今日暂无记录");
        StatsKpiLongestStreak.Text = longest > 0 ? LF("{0} 天", longest) : "—";
        StatsKpiLongestStreakHint.Text = longest > 0
            ? LF("共活跃 {0} 天", _statsDayTotals.Count)
            : L("暂无活跃记录");
    }

    /// <summary>连续活跃天数：以「有 token 用量记录的本地日期」为口径。</summary>
    private (int Current, int Longest) Streaks()
    {
        var days = _statsDayTotals.Keys.Select(DayOf).OrderBy(d => d).ToList();
        if (days.Count == 0)
        {
            return (0, 0);
        }
        var longest = 1;
        var run = 1;
        for (var i = 1; i < days.Count; i++)
        {
            if (days[i].Date == days[i - 1].Date.AddDays(1)) { run++; }
            else { run = 1; }
            if (run > longest) longest = run;
        }
        // 当前连续：从今天（或昨天，今天还没产生用量时）往回数
        var today = DateTime.Today;
        var set = days.Select(d => d.Date).ToHashSet();
        var cursor = set.Contains(today) ? today : today.AddDays(-1);
        var current = 0;
        while (set.Contains(cursor))
        {
            current++;
            cursor = cursor.AddDays(-1);
        }
        return (current, longest);
    }

    private void RenderStatsSourceNote()
    {
        // 聚合自证 + 口径说明集中在页尾一处（曾用 InfoBar 横幅展示，视觉噪声大且与卡片争层级）
        var skipped = _statsSessionsSkipped > 0 ? LF("，跳过 {0} 个空会话", _statsSessionsSkipped) : "";
        var capped = _statsPagesCapped > 0 ? LF("；{0} 个超长会话触到分页上限，其数据为部分计入", _statsPagesCapped) : "";
        StatsSourceNote.Text =
            LF("数据来源：内核 journal（session/list + session/page）的用量记录；已聚合 {0} 个非空会话、{1} 条记录{2}{3}。",
                _statsSessionsScanned, _statsUsageMessages, skipped, capped)
            + L("「累计/峰值/连续天数」按全部历史，「时间范围」只作用于趋势图与模型用量；")
            + L("费用/金额内核未提供对应字段，故不展示。");
    }

    // ---- Token 活动热力图（Canvas 自绘：列 = 周，行 = 周一…周日） ----

    private void RenderStatsHeatmap()
    {
        const int weeks = 26;
        var cell = TokenDouble("StatsHeatCellSize", 14);
        var gap = TokenDouble("StatsHeatCellGap", 4);
        var radius = TokenDouble("StatsHeatCellRadius", 3);
        var step = cell + gap;

        var today = DateTime.Today;
        // 最后一列的周一
        var lastMonday = today.AddDays(-(((int)today.DayOfWeek + 6) % 7));
        var start = lastMonday.AddDays(-(weeks - 1) * 7);

        // 每个单元的值（按当前口径换算）
        var values = new Dictionary<DateTime, long>();
        var running = 0L;
        for (var d = start; d <= today; d = d.AddDays(1))
        {
            var key = d.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture);
            _statsDayTotals.TryGetValue(key, out var v);
            switch (_statsHeatMetric)
            {
                case "week":
                    // 每周口径：同一周内 7 天都取该周合计，行内一眼看出整周强度
                    var monday = d.AddDays(-(((int)d.DayOfWeek + 6) % 7));
                    var sum = 0L;
                    for (var k = 0; k < 7; k++)
                    {
                        var wd = monday.AddDays(k);
                        if (_statsDayTotals.TryGetValue(wd.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture), out var wv)) sum += wv;
                    }
                    values[d] = sum;
                    break;
                case "total":
                    running += v;
                    values[d] = running;
                    break;
                default:
                    values[d] = v;
                    break;
            }
        }

        var max = values.Count == 0 ? 0L : values.Values.Max();
        StatsHeatCanvas.Children.Clear();
        StatsHeatMonthCanvas.Children.Clear();
        StatsHeatCanvas.Width = weeks * step;
        StatsHeatMonthCanvas.Width = weeks * step;

        var accent = ThemeBrush("AccentBrush");
        var empty = ThemeBrush("ChartHeatEmptyBrush");
        var monthBrush = ThemeBrush("TextTertiaryBrush");

        for (var w = 0; w < weeks; w++)
        {
            for (var r = 0; r < 7; r++)
            {
                var day = start.AddDays(w * 7 + r);
                if (day > today)
                {
                    continue;
                }
                values.TryGetValue(day, out var v);
                var level = HeatLevel(v, max);
                var rect = new Microsoft.UI.Xaml.Shapes.Rectangle
                {
                    Width = cell,
                    Height = cell,
                    RadiusX = radius,
                    RadiusY = radius,
                    Fill = level == 0 ? empty : HeatBrush(accent, level),
                };
                Canvas.SetLeft(rect, w * step);
                Canvas.SetTop(rect, r * step);
                var tip = LF("{0}：{1} tokens", day.ToString("yyyy-MM-dd"), v > 0 ? v.ToString("N0") : "0");
                ToolTipService.SetToolTip(rect, tip);
                AutomationProperties.SetName(rect, tip);
                StatsHeatCanvas.Children.Add(rect);
            }
            // 月份标签：该列周一是本月第一天所在周，或与上一列跨月时打点
            var monday = start.AddDays(w * 7);
            var prevMonday = w == 0 ? monday.AddDays(-7) : start.AddDays((w - 1) * 7);
            if (monday.Month != prevMonday.Month || w == 0)
            {
                var label = new TextBlock
                {
                    Text = LF("{0}月", monday.Month),
                    FontSize = TokenDouble("CaptionTextBlockFontSize", 12),
                    Foreground = monthBrush,
                };
                Canvas.SetLeft(label, w * step);
                Canvas.SetTop(label, 0);
                AutomationProperties.SetAccessibilityView(label, AccessibilityView.Raw);
                StatsHeatMonthCanvas.Children.Add(label);
            }
        }
        StatsHeatCanvas.Height = 7 * step - gap;
    }

    private static int HeatLevel(long value, long max)
    {
        if (value <= 0 || max <= 0)
        {
            return 0;
        }
        var ratio = (double)value / max;
        return ratio switch
        {
            <= 0.25 => 1,
            <= 0.5 => 2,
            <= 0.75 => 3,
            _ => 4,
        };
    }

    /// <summary>热力档位：强调色按档位压不透明度（不新增色值，随主题换色）。</summary>
    private static Brush HeatBrush(Brush accent, int level)
    {
        var opacity = level switch { 1 => 0.25, 2 => 0.45, 3 => 0.7, _ => 1.0 };
        if (accent is SolidColorBrush solid)
        {
            return new SolidColorBrush(solid.Color) { Opacity = opacity };
        }
        return accent;
    }

    // ---- 每日 Token 趋势图（Canvas + Polyline 自绘，多模型多曲线） ----

    private void RenderStatsTrend()
    {
        StatsTrendCanvas.Children.Clear();
        var width = StatsTrendCanvas.ActualWidth;
        if (width <= 0)
        {
            return; // 尚未布局：SizeChanged 会再调一次
        }
        var height = TokenDouble("StatsChartHeight", 220);

        var days = RangeDays(_statsRangeDays);
        // 模型序列：按窗口内合计降序，颜色按序取色板
        var series = new List<(string Model, long[] Values)>();
        var totals = new Dictionary<string, long>(StringComparer.Ordinal);
        foreach (var day in days)
        {
            var key = day.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture);
            if (!_statsDayModel.TryGetValue(key, out var byModel)) continue;
            foreach (var (model, v) in byModel)
            {
                totals[model] = totals.TryGetValue(model, out var cur) ? cur + v : v;
            }
        }
        foreach (var model in totals.OrderByDescending(kv => kv.Value).Select(kv => kv.Key))
        {
            var values = new long[days.Count];
            for (var i = 0; i < days.Count; i++)
            {
                var key = days[i].ToString("yyyy-MM-dd", CultureInfo.InvariantCulture);
                if (_statsDayModel.TryGetValue(key, out var byModel) && byModel.TryGetValue(model, out var v)) values[i] = v;
            }
            series.Add((model, values));
        }

        // 图例（与曲线同色）
        StatsTrendLegend.ItemsSource = series
            .Select((s, i) => new StatsLegendVm { Name = s.Model, Swatch = ChartBrush(i) })
            .ToList();

        // 空态：所选范围内没有任何用量时不画空坐标轴，居中给一句提示
        if (series.Count == 0)
        {
            StatsTrendCanvas.Children.Add(new Border
            {
                Width = Math.Max(10, width),
                Height = height,
                Child = new TextBlock
                {
                    Text = L("所选时间范围内暂无用量记录"),
                    Style = AppStyle("HintTextStyle"),
                    HorizontalAlignment = HorizontalAlignment.Center,
                    VerticalAlignment = VerticalAlignment.Center,
                },
            });
            AutomationProperties.SetName(StatsTrendCanvas,
                LF("每日 Token 趋势图：近 {0} 天暂无用量", _statsRangeDays));
            return;
        }

        const double leftPad = 64, rightPad = 16, topPad = 10, bottomPad = 30;
        var plotW = Math.Max(10, width - leftPad - rightPad);
        var plotH = Math.Max(10, height - topPad - bottomPad);

        long maxV = 0;
        foreach (var s in series) foreach (var v in s.Values) if (v > maxV) maxV = v;
        if (maxV == 0) maxV = 1;

        var gridBrush = ThemeBrush("ChartGridBrush");
        var axisBrush = ThemeBrush("TextTertiaryBrush");
        var captionSize = TokenDouble("CaptionTextBlockFontSize", 12);

        // 横向网格 + Y 轴刻度（5 档）
        for (var i = 0; i <= 4; i++)
        {
            var y = topPad + plotH * i / 4.0;
            StatsTrendCanvas.Children.Add(new Microsoft.UI.Xaml.Shapes.Line
            {
                X1 = leftPad, Y1 = y, X2 = leftPad + plotW, Y2 = y,
                Stroke = gridBrush,
                StrokeThickness = i == 4 ? 1 : 1,
            });
            var label = new TextBlock
            {
                Text = FormatTokens((long)(maxV * (4 - i) / 4.0)),
                FontSize = captionSize,
                Foreground = axisBrush,
            };
            Canvas.SetLeft(label, 0);
            Canvas.SetTop(label, y - captionSize);
            AutomationProperties.SetAccessibilityView(label, AccessibilityView.Raw);
            StatsTrendCanvas.Children.Add(label);
        }

        // X 轴日期刻度：按天数取 7 档以内的等距标签，避免 30 天时挤成一团。
        // 末位标签一定会画（右端点是读者定位「今天」的锚），因此与它过近的前一个标签让位，
        // 否则 30 天档下最后两枚日期会叠在一起。
        var labelStep = Math.Max(1, (int)Math.Ceiling(days.Count / 8.0));
        const double minLabelGap = 56; // 日期标签「9月15日」的实测宽度 + 留白
        var lastX = leftPad + (days.Count == 1 ? plotW / 2 : plotW);
        var drawn = new List<double>();
        for (var i = 0; i < days.Count; i++)
        {
            var x = leftPad + (days.Count == 1 ? plotW / 2 : plotW * i / (days.Count - 1.0));
            var isLast = i == days.Count - 1;
            if (!isLast && i % labelStep != 0)
            {
                continue;
            }
            if (!isLast && lastX - x < minLabelGap)
            {
                continue; // 会被末位标签压住，让位
            }
            if (drawn.Count > 0 && x - drawn[^1] < minLabelGap)
            {
                continue;
            }
            drawn.Add(x);
            var label = new TextBlock
            {
                Text = LF("{0}月{1}日", days[i].Month, days[i].Day),
                FontSize = captionSize,
                Foreground = axisBrush,
                TextWrapping = TextWrapping.NoWrap,
            };
            Canvas.SetLeft(label, Math.Max(0, Math.Min(x - 18, width - 52)));
            Canvas.SetTop(label, topPad + plotH + 8);
            AutomationProperties.SetAccessibilityView(label, AccessibilityView.Raw);
            StatsTrendCanvas.Children.Add(label);
        }

        // 逐序列求点集：先全部算出，再「先面积、后曲线」两遍绘制（面积垫底不盖折线）。
        // PointCollection 不可复用（一个实例只挂一个 Points），每个形状各建一份。
        static PointCollection ToCollection(List<Windows.Foundation.Point> pts)
        {
            var col = new PointCollection();
            foreach (var p in pts) col.Add(p);
            return col;
        }
        var plotted = new List<(Brush Brush, List<Windows.Foundation.Point> Points)>();
        for (var si = 0; si < series.Count; si++)
        {
            var values = series[si].Values;
            var points = new List<Windows.Foundation.Point>(values.Length);
            for (var i = 0; i < values.Length; i++)
            {
                var x = leftPad + (values.Length == 1 ? plotW / 2 : plotW * i / (values.Length - 1.0));
                var y = topPad + plotH * (1 - (double)values[i] / maxV);
                points.Add(new Windows.Foundation.Point(x, y));
            }
            plotted.Add((ChartBrush(si), points));
        }

        // 面积填充：曲线下方到底边、自上而下淡出的同色渐变，量感一眼可读。
        // 高对比度跳过：系统配色不做半透明（与 Tokens.xaml HC 色板同一条约定）。
        if (!IsHighContrast())
        {
            var baseline = topPad + plotH;
            foreach (var (brush, points) in plotted)
            {
                if (brush is not SolidColorBrush solid || points.Count < 2) continue;
                var areaPts = new List<Windows.Foundation.Point>(points)
                {
                    new(points[^1].X, baseline),
                    new(points[0].X, baseline),
                };
                var fade = new LinearGradientBrush
                {
                    MappingMode = BrushMappingMode.Absolute,
                    StartPoint = new Windows.Foundation.Point(0, topPad),
                    EndPoint = new Windows.Foundation.Point(0, baseline),
                };
                fade.GradientStops.Add(new GradientStop
                {
                    Color = Windows.UI.Color.FromArgb(80, solid.Color.R, solid.Color.G, solid.Color.B),
                });
                fade.GradientStops.Add(new GradientStop
                {
                    Color = Windows.UI.Color.FromArgb(0, solid.Color.R, solid.Color.G, solid.Color.B),
                    Offset = 1,
                });
                var area = new Microsoft.UI.Xaml.Shapes.Polygon
                {
                    Points = ToCollection(areaPts),
                    Fill = fade,
                };
                AutomationProperties.SetAccessibilityView(area, AccessibilityView.Raw);
                StatsTrendCanvas.Children.Add(area);
            }
        }

        // 曲线 + 端点
        for (var si = 0; si < plotted.Count; si++)
        {
            var (brush, points) = plotted[si];
            var line = new Microsoft.UI.Xaml.Shapes.Polyline
            {
                Points = ToCollection(points),
                Stroke = brush,
                StrokeThickness = 2,
                StrokeLineJoin = PenLineJoin.Round,
            };
            if (SeriesDash(si) is { } dash)
            {
                line.StrokeDashArray = dash; // 高对比度下多序列靠线型区分（颜色落到同一系统色）
            }
            AutomationProperties.SetName(line, LF("{0} 曲线", series[si].Model));
            StatsTrendCanvas.Children.Add(line);
            // 端点圆点：单模型时曲线可能是一条水平线，端点更容易读
            if (points.Count > 0)
            {
                var dot = new Microsoft.UI.Xaml.Shapes.Ellipse { Width = 7, Height = 7, Fill = brush };
                Canvas.SetLeft(dot, points[^1].X - 3.5);
                Canvas.SetTop(dot, points[^1].Y - 3.5);
                AutomationProperties.SetAccessibilityView(dot, AccessibilityView.Raw);
                StatsTrendCanvas.Children.Add(dot);
            }
        }

        var totalInRange = series.Sum(s => s.Values.Sum());
        AutomationProperties.SetName(StatsTrendCanvas,
            LF("每日 Token 趋势图：近 {0} 天共 {1} tokens，{2} 个模型序列", _statsRangeDays, FormatTokens(totalInRange), series.Count));
    }

    // ---- 模型用量占比条（存储页式横向条形：名称 + tokens + 占比条 + 占比说明） ----

    private void RenderStatsModelUsage()
    {
        var days = RangeDays(_statsRangeDays);

        var totals = new Dictionary<string, long>(StringComparer.Ordinal);
        foreach (var day in days)
        {
            var key = day.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture);
            if (!_statsDayModel.TryGetValue(key, out var byModel)) continue;
            foreach (var (model, v) in byModel) totals[model] = totals.TryGetValue(model, out var cur) ? cur + v : v;
        }
        var ordered = totals.OrderByDescending(kv => kv.Value).ToList();
        var sum = ordered.Sum(kv => kv.Value);

        // 卡头右侧合计（原环形图中心的总量信息挪到这里）
        StatsModelTotal.Text = sum > 0 ? LF("合计 {0} tokens", FormatTokens(sum)) : "";
        AutomationProperties.SetName(StatsModelLegend,
            sum > 0 ? LF("模型用量：近 {0} 天共 {1} tokens，{2} 个模型", _statsRangeDays, FormatTokens(sum), ordered.Count)
                    : L("模型用量"));

        if (sum <= 0)
        {
            StatsModelLegend.ItemsSource = null;
            StatsModelEmptyHint.Visibility = Visibility.Visible;
            return;
        }
        StatsModelEmptyHint.Visibility = Visibility.Collapsed;

        var legend = new List<StatsModelVm>();
        for (var i = 0; i < ordered.Count; i++)
        {
            var (model, value) = ordered[i];
            var share = 100.0 * value / sum;
            legend.Add(new StatsModelVm
            {
                // L() 未命中键时原样返回，provider/model 这类真实模型名不受影响
                Name = L(model),
                TokensText = $"{FormatTokens(value)} tokens",
                BarValue = Math.Round(share, 1),
                ShareText = LF("占比 {0}%", Math.Round(share)),
            });
        }
        StatsModelLegend.ItemsSource = legend;
    }

    // ---- 通用小工具 ----

    private List<DateTime> RangeDays(int days)
    {
        var list = new List<DateTime>(days);
        var today = DateTime.Today;
        for (var i = days - 1; i >= 0; i--)
        {
            list.Add(today.AddDays(-i));
        }
        return list;
    }

    private Brush ChartBrush(int index) => ThemeBrush($"ChartSeries{index % 6 + 1}Brush");

    /// <summary>高对比度下数据色板全落到系统窗口文本色，多序列改由线型区分；常规主题不加虚线。</summary>
    private DoubleCollection? SeriesDash(int index)
        => IsHighContrast() && index % 3 > 0
            ? new DoubleCollection { 3 * (index % 3), 2 }
            : null;

    /// <summary>系统高对比度开关（ElementTheme 只有 Default/Light/Dark，没有 HighContrast 成员）。</summary>
    private static bool IsHighContrast()
    {
        try
        {
            return new Windows.UI.ViewManagement.AccessibilitySettings().HighContrast;
        }
        catch (Exception)
        {
            return false;
        }
    }

    /// <summary>token 数的可读化：中文 ≥1 亿用「亿」，≥1 万用「万」；其他语言千分位。</summary>
    private string FormatTokens(long value)
        => _shellLocale.StartsWith("zh", StringComparison.OrdinalIgnoreCase) && value >= 100_000_000 ? LF("{0:0.#} 亿", value / 100_000_000.0)
         : _shellLocale.StartsWith("zh", StringComparison.OrdinalIgnoreCase) && value >= 10_000 ? LF("{0:0.#} 万", value / 10_000.0)
         : value.ToString("N0");

    /// <summary>时长可读化：小时 + 分（不足 1 小时只给分，不足 1 分钟给秒）。</summary>
    private string FormatDuration(double ms)
    {
        var span = TimeSpan.FromMilliseconds(ms);
        if (span.TotalHours >= 1) return LF("{0} 小时 {1} 分", (int)span.TotalHours, span.Minutes);
        if (span.TotalMinutes >= 1) return LF("{0} 分 {1} 秒", span.Minutes, span.Seconds);
        return LF("{0} 秒", Math.Max(0, (int)span.TotalSeconds));
    }

    private void ShowStatsInfo(InfoBarSeverity severity, string message)
    {
        StatsInfoBar.Severity = severity;
        StatsInfoBar.Message = message;
        StatsInfoBar.IsOpen = true;
    }

    // ---------------- 会话控制流（session/control：排队项 + 运行中任务） ----------------
    //
    // session/control 是内核的"会话控制面"流（descriptor: mode="stream"，无参数）：
    //   baseline   {value:{queues:{sessionId:[item…]}, jobs:{sessionId:[job…]}, projections:{…}}}
    //   queue      {sessionId, items:[…]}      —— 该会话队列的完整替换
    //   jobs       {sessionId, jobs:[…]}       —— 该会话任务的完整替换
    //   projection {sessionId, key, value, seq} —— 投影增量（title/goal/todos…，壳已有别的事件源）
    // item = {id, placement:"queued"|"steering"|"context", rpcId?, message:{id, content:[…]}}
    // （0.7.x 实测：排队项可能只出现在 baseline 里，其后无变化就不再推 queue 帧。）

    /// <summary>打开长驻 session/control 流（幂等：已开则跳过）。</summary>
    private async Task OpenSessionControlStreamAsync()
    {
        if (_rpc is null || _controlStreamId is not null)
        {
            return;
        }
        try
        {
            _controlStreamId = await _rpc.OpenRemoteStreamAsync("session/control", new { }, OnControlFrame);
        }
        catch (Exception)
        {
            // mux 未就绪/内核拒绝：下次会话打开时再试（不影响聊天主流程）
            _controlStreamId = null;
        }
    }

    /// <summary>控制流帧（接收循环线程）：更新队列/任务缓存后编组刷新面板。</summary>
    private void OnControlFrame(JsonElement frame)
    {
        try
        {
            var type = frame.TryGetProperty("type", out var t) ? t.GetString() : null;
            switch (type)
            {
                case "baseline":
                    if (frame.TryGetProperty("value", out var value) && value.ValueKind == JsonValueKind.Object)
                    {
                        // projections 快照：{<sid>:{asOfSeq, values:{plan,permissions,…}}}。
                        // 在 _controlLock 内只做拷贝，出锁后再套 _projectionLock 应用——
                        // 锁序单一（controlLock → projectionLock），不会有反向嵌套。
                        JsonElement activeProjections = default;
                        lock (_controlLock)
                        {
                            _queues.Clear();
                            _jobs.Clear();
                            if (value.TryGetProperty("queues", out var queues) && queues.ValueKind == JsonValueKind.Object)
                            {
                                foreach (var entry in queues.EnumerateObject())
                                {
                                    _queues[entry.Name] = ParseQueueItems(entry.Value, entry.Name);
                                }
                            }
                            if (value.TryGetProperty("jobs", out var jobs) && jobs.ValueKind == JsonValueKind.Object)
                            {
                                foreach (var entry in jobs.EnumerateObject())
                                {
                                    _jobs[entry.Name] = ParseJobs(entry.Value);
                                }
                            }
                            // 取当前会话的投影值对象（plan/permissions/schedule/goal 四键），
                            // 由 ApplyProjectionValues 按键分发。
                            if (value.TryGetProperty("projections", out var proj) && proj.ValueKind == JsonValueKind.Object &&
                                Volatile.Read(ref _activeSessionId) is { Length: > 0 } active &&
                                proj.TryGetProperty(active, out var block) && block.ValueKind == JsonValueKind.Object &&
                                block.TryGetProperty("values", out var values) && values.ValueKind == JsonValueKind.Object)
                            {
                                activeProjections = values.Clone();
                            }
                        }
                        if (activeProjections.ValueKind == JsonValueKind.Object)
                        {
                            lock (_projectionLock)
                            {
                                ApplyProjectionValues(activeProjections);
                            }
                        }
                    }
                    break;
                case "queue":
                    if (frame.TryGetProperty("sessionId", out var qsid) && qsid.ValueKind == JsonValueKind.String)
                    {
                        var sid = qsid.GetString()!;
                        var items = frame.TryGetProperty("items", out var qitems) ? ParseQueueItems(qitems, sid) : new List<QueueItemVm>();
                        lock (_controlLock)
                        {
                            _queues[sid] = items;
                        }
                    }
                    break;
                case "jobs":
                    if (frame.TryGetProperty("sessionId", out var jsid) && jsid.ValueKind == JsonValueKind.String)
                    {
                        var sid = jsid.GetString()!;
                        var jobs = frame.TryGetProperty("jobs", out var jitems) ? ParseJobs(jitems) : new List<JobVm>();
                        lock (_controlLock)
                        {
                            _jobs[sid] = jobs;
                        }
                    }
                    break;
                case "projection":
                    // 投影增量：{sessionId, key, value, seq}。壳关心当前会话的
                    // plan / permissions / schedule / goal / turnOutline / modelSelection 六键；
                    // 其余（title/todos/inbox…）与本面板无关。
                    if (frame.TryGetProperty("sessionId", out var psid) && psid.ValueKind == JsonValueKind.String &&
                        psid.GetString() == Volatile.Read(ref _activeSessionId) &&
                        frame.TryGetProperty("key", out var pkey) && pkey.ValueKind == JsonValueKind.String)
                    {
                        var key = pkey.GetString();
                        if (key is "plan" or "permissions" or "schedule" or "goal")
                        {
                            var pv = frame.TryGetProperty("value", out var pval) ? pval : default;
                            lock (_projectionLock)
                            {
                                switch (key)
                                {
                                    case "plan": ApplyPlanProjection(pv); break;
                                    case "permissions": ApplyPermissionsProjection(pv); break;
                                    case "schedule": ApplyScheduleProjection(pv); break;
                                    default: ApplyGoalProjection(pv); break;
                                }
                            }
                        }
                        else if (key == "modelSelection")
                        {
                            var pv = frame.TryGetProperty("value", out var pval) ? pval : default;
                            lock (_projectionLock)
                            {
                                ApplyModelSelectionProjection(pv);
                            }
                            return;
                        }
                        else if (key == "turnOutline")
                        {
                            // 历史快速定位 rail：wire.view 是全量轮次（非增量补丁），整表替换。
                            var pv = frame.TryGetProperty("value", out var pval) ? pval : default;
                            ApplyTurnOutlineFrame(pv, psid.GetString()!);
                            return; // rail 自刷，不碰输入区状态条
                        }
                        else
                        {
                            return; // 其它投影键与本面板无关
                        }
                        // 计划/目标两个键的渲染入口各自不同（goal 自带 PostUi），
                        // 这里只刷 plan/permissions 共用的输入区状态条。
                        PostUi(RefreshSessionStateBar);
                        return;
                    }
                    return;
                default:
                    return;
            }
        }
        catch (Exception)
        {
            return; // 帧形态异常不影响壳
        }
        PostUi(RefreshQueuePanel);
        PostUi(RefreshJobsPanel);
        PostUi(RefreshSessionStateBar);
    }

    /// <summary>plan 投影视图：{active:bool, pending:bool}（dsh-plan-mode 的 wire.view 裁剪结果）。</summary>
    private void ApplyPlanProjection(JsonElement view)
    {
        if (view.ValueKind != JsonValueKind.Object)
        {
            return;
        }
        // 调用方持 _projectionLock
        _planActive = view.TryGetProperty("active", out var a) && a.ValueKind == JsonValueKind.True;
        _planPending = view.TryGetProperty("pending", out var p) && p.ValueKind == JsonValueKind.True;
    }

    /// <summary>permissions 投影视图：{options:[{value,name,description?}], currentValue}。</summary>
    private void ApplyPermissionsProjection(JsonElement view)
    {
        if (view.ValueKind != JsonValueKind.Object)
        {
            return;
        }
        var options = new List<(string, string, string)>();
        if (view.TryGetProperty("options", out var opts) && opts.ValueKind == JsonValueKind.Array)
        {
            foreach (var opt in opts.EnumerateArray())
            {
                if (opt.ValueKind != JsonValueKind.Object)
                {
                    continue;
                }
                var value = opt.TryGetProperty("value", out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() ?? "" : "";
                var name = opt.TryGetProperty("name", out var n) && n.ValueKind == JsonValueKind.String ? n.GetString() ?? "" : value;
                var desc = opt.TryGetProperty("description", out var d) && d.ValueKind == JsonValueKind.String ? d.GetString() ?? "" : "";
                if (value.Length > 0)
                {
                    options.Add((value, name, desc));
                }
            }
        }
        if (options.Count > 0)
        {
            _permissionOptions = options;
        }
        _permissionCurrentValue = view.TryGetProperty("currentValue", out var cv) && cv.ValueKind == JsonValueKind.String
            ? cv.GetString() : null;
    }

    /// <summary>会话投影快照里的 plan / permissions 值（session/control baseline 与 session/list 复用）。</summary>
    private void ApplyProjectionValues(JsonElement values)
    {
        // 调用方持 _projectionLock
        if (values.TryGetProperty("plan", out var plan))
        {
            ApplyPlanProjection(plan);
        }
        if (values.TryGetProperty("permissions", out var perms))
        {
            ApplyPermissionsProjection(perms);
        }
        if (values.TryGetProperty("schedule", out var schedule))
        {
            ApplyScheduleProjection(schedule);
        }
        if (values.TryGetProperty("goal", out var goal))
        {
            ApplyGoalProjection(goal);
        }
        if (values.TryGetProperty("modelSelection", out var modelSelection))
        {
            ApplyModelSelectionProjection(modelSelection);
        }
        // turnOutline（右侧历史快速定位）：键名与解析在 MainWindow.TurnRail.cs。
        // 调用方作用域即当前活动会话（baseline 帧已按会话解析；session/list 补齐只对当前会话生效）。
        ApplyTurnOutlineSnapshot(values, Volatile.Read(ref _activeSessionId) ?? "");
    }

    private List<QueueItemVm> ParseQueueItems(JsonElement items, string sessionId)
    {
        var list = new List<QueueItemVm>();
        if (items.ValueKind != JsonValueKind.Array)
        {
            return list;
        }
        foreach (var item in items.EnumerateArray())
        {
            var id = item.TryGetProperty("id", out var idEl) ? idEl.GetString() ?? "" : "";
            if (id.Length == 0)
            {
                continue;
            }
            var placement = item.TryGetProperty("placement", out var p) && p.ValueKind == JsonValueKind.String ? p.GetString() ?? "queued" : "queued";
            var content = item.TryGetProperty("message", out var msg) && msg.TryGetProperty("content", out var c) ? c : default;
            list.Add(new QueueItemVm
            {
                SessionId = sessionId,
                ItemId = id,
                Placement = placement,
                Text = ContentText(content),
                Content = content.Clone(),
            });
        }
        return list;
    }

    private static List<JobVm> ParseJobs(JsonElement jobs)
    {
        var list = new List<JobVm>();
        if (jobs.ValueKind != JsonValueKind.Array)
        {
            return list;
        }
        foreach (var job in jobs.EnumerateArray())
        {
            list.Add(new JobVm
            {
                JobId = job.TryGetProperty("id", out var id) ? id.GetString() ?? "" : "",
                Kind = job.TryGetProperty("kind", out var k) && k.ValueKind == JsonValueKind.String ? k.GetString() ?? "" : "",
                Label = job.TryGetProperty("label", out var l) && l.ValueKind == JsonValueKind.String ? l.GetString() ?? "" : "",
                Status = job.TryGetProperty("status", out var s) && s.ValueKind == JsonValueKind.String ? s.GetString() ?? "" : "",
                Detail = job.TryGetProperty("detail", out var d) && d.ValueKind == JsonValueKind.String ? d.GetString() : null,
                StartedAt = job.TryGetProperty("startedAt", out var st) && st.ValueKind == JsonValueKind.Number ? (long)st.GetDouble() : 0,
                FinishedAt = job.TryGetProperty("finishedAt", out var ft) && ft.ValueKind == JsonValueKind.Number ? (long)ft.GetDouble() : 0,
            });
        }
        return list;
    }

    /// <summary>官方 JobListAction 的 ordered()：存活作业在前（按开始时间升序，保持启动次序），
    /// 已结束作业按完成时间倒序（新的在前）；同毫秒平手回落开始次序，排序不依赖宿主枚举顺序。</summary>
    private static List<JobVm> OrderJobs(List<JobVm> jobs)
    {
        return jobs
            .OrderBy(j => !j.IsLive)
            .ThenBy(j => j.IsLive ? j.StartedAt : 0)
            .ThenByDescending(j => j.IsLive ? 0 : (j.FinishedAt > 0 ? j.FinishedAt : j.StartedAt))
            .ToList();
    }

    /// <summary>content 块数组 → 可读文本（text 块拼接；图片/文件块出角标）。</summary>
    private string ContentText(JsonElement content)
    {
        if (content.ValueKind != JsonValueKind.Array)
        {
            return "";
        }
        var parts = new List<string>();
        foreach (var block in content.EnumerateArray())
        {
            var type = block.TryGetProperty("type", out var t) ? t.GetString() : null;
            switch (type)
            {
                case "text":
                    if (block.TryGetProperty("text", out var text) && text.ValueKind == JsonValueKind.String)
                    {
                        parts.Add(text.GetString() ?? "");
                    }
                    break;
                case "image":
                    parts.Add(L("[图片]"));
                    break;
                case "file":
                    var name = block.TryGetProperty("attachment", out var att) && att.TryGetProperty("name", out var n) ? n.GetString() : null;
                    parts.Add(LF("[文件 {0}]", name));
                    break;
            }
        }
        return string.Join(" ", parts).Trim();
    }

    /// <summary>队列面板（仅当前会话；无排队项时整条收起）。作业已从底部横条拆到
    /// 会话头触发器 + 下拉（RefreshJobsPanel），本面板只负责排队项。</summary>
    private void RefreshQueuePanel()
    {
        try
        {
            QueueHost.Children.Clear();
            var sid = Volatile.Read(ref _activeSessionId);
            if (sid is null || _rpc is null)
            {
                QueuePanel.Visibility = Visibility.Collapsed;
                return;
            }
            List<QueueItemVm> items;
            lock (_controlLock)
            {
                items = _queues.TryGetValue(sid, out var q) ? q : new List<QueueItemVm>();
            }
            // placement=context 是内核插件注入的模型上下文（如 user-approval 的审批策略变更通知），
            // 不是用户消息：不算「排队中」，也不给编辑/插话/删除——网页端同样不把它当用户排队项。
            items = items.Where(i => i.Placement is "queued" or "steering").ToList();
            if (items.Count == 0)
            {
                QueuePanel.Visibility = Visibility.Collapsed;
                return;
            }
            QueuePanel.Visibility = Visibility.Visible;
            QueueHost.Children.Add(MakeQueueHeaderRow(LF("排队中 · {0} 条", items.Count), L("内核已收到、等当前轮结束后发送")));
            foreach (var item in items)
            {
                QueueHost.Children.Add(MakeQueueRow(item));
            }
        }
        catch (Exception)
        {
            // 面板刷新失败不阻塞聊天（0xc000027b 教训：任何 UI 构造异常都不上抛）
        }
    }

    /// <summary>
    /// 作业入口（会话头触发器 + 下拉列表，对标官方 JobListAction）。数据源与队列同一条
    /// session/control 流的 jobs 帧（{sessionId, jobs:[…]}），只展示当前会话。
    /// 触发器显隐 = 当前会话有无作业；下拉行按官方口径排序（OrderJobs），live 时长 1s tick。
    /// </summary>
    private void RefreshJobsPanel()
    {
        try
        {
            var sid = Volatile.Read(ref _activeSessionId);
            List<JobVm> jobs;
            lock (_controlLock)
            {
                jobs = sid is null ? new List<JobVm>() : _jobs.TryGetValue(sid, out var j) ? j.ToList() : new List<JobVm>();
            }
            _activeJobs.Clear();
            _activeJobs.AddRange(OrderJobs(jobs));

            // 触发器显隐只取决于当前会话有无作业（会话头是独立行，与聊天区是否空态无关：
            // 空会话也可能已有后台作业在跑，官方端同样在无消息时显示作业入口）
            var show = jobs.Count > 0;
            JobsTriggerButton.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
            if (!show)
            {
                StopJobsTick();
                return;
            }

            var live = jobs.Count(j => j.IsLive);
            JobsTriggerDot.Visibility = live > 0 ? Visibility.Visible : Visibility.Collapsed;
            var countText = live > 0 ? LF("{0} 个作业运行中", live) : LF("作业 · {0} 个", jobs.Count);
            JobsTriggerText.Text = countText;
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(JobsTriggerButton, L("后台作业") + "：" + countText);

            // 下拉开着时行内时长必须持续走时：有 live 作业才挂 1s tick，全结束后停表
            if (live > 0)
            {
                StartJobsTick();
            }
            else
            {
                StopJobsTick();
            }
            if (JobsFlyout.IsOpen)
            {
                BuildJobsFlyout();
            }
        }
        catch (Exception)
        {
            // 面板刷新失败不阻塞聊天（0xc000027b 教训：任何 UI 构造异常都不上抛）
        }
    }

    /// <summary>下拉打开时重建行（Opening 与 tick 共用）：时长按当前时钟现算。</summary>
    private void BuildJobsFlyout()
    {
        try
        {
            JobsFlyoutHost.Children.Clear();
            var now = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
            foreach (var job in _activeJobs)
            {
                JobsFlyoutHost.Children.Add(MakeJobRow(job, now));
            }
        }
        catch (Exception)
        {
            // 同上：UI 构造异常不上抛
        }
    }

    private void OnJobsFlyoutOpening(object sender, object e)
    {
        BuildJobsFlyout();
    }

    /// <summary>live 作业时长 tick：1s 重建下拉行。仅下拉打开期间重建，关着不耗帧。</summary>
    private void StartJobsTick()
    {
        if (_jobsTickTimer is null)
        {
            _jobsTickTimer = DispatcherQueue.CreateTimer();
            _jobsTickTimer.Interval = TimeSpan.FromSeconds(1);
            _jobsTickTimer.Tick += (_, _) =>
            {
                if (JobsFlyout.IsOpen)
                {
                    BuildJobsFlyout();
                }
            };
        }
        _jobsTickTimer.Start();
    }

    private void StopJobsTick()
    {
        _jobsTickTimer?.Stop();
    }

    // ---------------- 会话头：标题 / 子代理回跳 / 只读输入 ----------------

    /// <summary>
    /// 会话头刷新（对标官方会话身份区）：标题 + 子代理回跳/只读提示 + 输入区只读态。
    /// 调用点：OpenSessionAsync（切会话即时刷）与 UpdateEmptyState（历史回读/空态变化后）。
    /// 子代理判定取 session/list 的 origin 位（SessionVm.IsSubagent）——与官方端同源。
    /// </summary>
    private void RefreshSessionHeader()
    {
        try
        {
            var vm = _sessions.FirstOrDefault(s => s.SessionId == Volatile.Read(ref _activeSessionId));
            if (vm is null)
            {
                ChatSessionHeader.Visibility = Visibility.Collapsed;
                return;
            }
            ChatSessionHeader.Visibility = Visibility.Visible;
            ChatSessionTitle.Text = vm.Title == "新会话" ? L("新会话") : vm.Title;
            var parent = vm.IsSubagent && vm.ParentSessionId is { Length: > 0 } pid
                ? _sessions.FirstOrDefault(s => s.SessionId == pid)
                : null;
            SubagentParentButton.Visibility = parent is not null ? Visibility.Visible : Visibility.Collapsed;
            SubagentReadOnlyHint.Visibility = vm.IsSubagent ? Visibility.Visible : Visibility.Collapsed;
            if (vm.IsSubagent)
            {
                SubagentReadOnlyHint.Text = L("子代理会话为只读：可查看其工作过程，消息请在父会话中发送。");
            }
            ApplySubagentReadOnly(vm.IsSubagent);
        }
        catch (Exception)
        {
            // 会话头是附属信息：失败不阻塞聊天主链路
        }
    }

    /// <summary>子代理回跳父会话（官方 SubagentHeaderLineage 的最小同语义：本壳只展示直接父，
    /// 血缘链由逐级回跳覆盖）。导航路径与搜索框跳转一致：先落 _activeSessionId 再
    /// OpenSessionAsync，保证队列/作业/投影/历史全套状态按新会话重建。</summary>
    private async void OnSubagentParentClick(object sender, RoutedEventArgs e)
    {
        try
        {
            var vm = _sessions.FirstOrDefault(s => s.SessionId == Volatile.Read(ref _activeSessionId));
            var parent = vm?.IsSubagent == true && vm.ParentSessionId is { Length: > 0 } pid
                ? _sessions.FirstOrDefault(s => s.SessionId == pid)
                : null;
            if (parent is null)
            {
                return;
            }
            Volatile.Write(ref _activeSessionId, parent.SessionId);
            await OpenSessionAsync(parent);
            RestoreActiveSessionSelection();
        }
        catch (Exception)
        {
            // async void 边界兜底：导航异常不进未处理异常（0xc000027b 教训）
        }
    }

    /// <summary>子代理会话只读（对标官方 SubagentReadOnlyComposer）：输入框与发送键禁用。
    /// OnSendClick 另按会话 origin 兜底，双保险防越权写入。</summary>
    private void ApplySubagentReadOnly(bool readOnly)
    {
        InputBox.IsEnabled = !readOnly;
        SendButton.IsEnabled = !readOnly;
        ToolTipService.SetToolTip(InputBox, readOnly ? L("子代理会话为只读：可查看其工作过程，消息请在父会话中发送。") : null);
    }

    // ---------------- 目标常驻条（对标官方 GoalBar） ----------------

    /// <summary>
    /// 目标条（对标官方 GoalBar，composer 上方常驻）：切会话时拉 goals/get，有未完结目标才显示。
    /// 失败按"无目标"静默收起——目标条是附属信息，不阻塞聊天。会话运行中的目标推进由
    /// session/control 的 projection/goal 帧驱动（ApplyGoalProjection）。
    /// </summary>
    private async Task RefreshGoalBarAsync(string sessionId)
    {
        try
        {
            if (_rpc is null)
            {
                PostUi(ApplyGoalBar);
                return;
            }
            (string Id, long Revision, string Phase, string Activation, string Objective, int RoundsStarted)? summary = null;
            try
            {
                var goal = await _rpc.CallOkAsync("goals/get", new { agentId = sessionId });
                // 快切会话时丢弃过期响应：只允许当前会话的目标落进目标条
                if (Volatile.Read(ref _activeSessionId) == sessionId)
                {
                    summary = ParseGoalSummary(goal);
                }
            }
            catch (DshRpcException)
            {
                // 无目标/内核拒绝：官方 GoalBar 对空目标同样不渲染，按无目标处理
            }
            lock (_goalLock)
            {
                _goalSummary = summary;
            }
            PostUi(ApplyGoalBar);
        }
        catch (Exception)
        {
            // 目标条失败不阻塞会话打开
        }
    }

    /// <summary>goals/get 响应 → 目标摘要。phase 为 complete/缺失时返回 null（目标条整条收起）。
    /// 响应字段与内核 dsh-goal 的 goalProjectionSchema 对齐：objective/phase/id/revision/
    /// roundsStarted（activation 仅 goals/get 有，投影增量里没有）。</summary>
    private static (string Id, long Revision, string Phase, string Activation, string Objective, int RoundsStarted)? ParseGoalSummary(JsonElement goal)
    {
        if (goal.ValueKind != JsonValueKind.Object)
        {
            return null;
        }
        var phase = goal.TryGetProperty("phase", out var p) && p.ValueKind == JsonValueKind.String ? p.GetString() ?? "" : "";
        if (phase is not ("active" or "paused" or "blocked"))
        {
            return null;
        }
        return (
            goal.TryGetProperty("id", out var i) && i.ValueKind == JsonValueKind.String ? i.GetString() ?? "" : "",
            goal.TryGetProperty("revision", out var r) && r.ValueKind == JsonValueKind.Number ? (long)r.GetDouble() : 0,
            phase,
            goal.TryGetProperty("activation", out var a) && a.ValueKind == JsonValueKind.String ? a.GetString() ?? "" : "",
            goal.TryGetProperty("objective", out var o) && o.ValueKind == JsonValueKind.String ? o.GetString() ?? "" : "",
            goal.TryGetProperty("roundsStarted", out var rs) && rs.ValueKind == JsonValueKind.Number ? rs.GetInt32() : 0);
    }

    /// <summary>目标条渲染：阶段标签（官方 phase × activation 语义）+ 目标 + 显隐。</summary>
    private void ApplyGoalBar()
    {
        try
        {
            (string Id, long Revision, string Phase, string Activation, string Objective, int RoundsStarted)? g;
            lock (_goalLock)
            {
                g = _goalSummary;
            }
            if (g is not { } goal)
            {
                GoalBar.Visibility = Visibility.Collapsed;
                return;
            }
            GoalBar.Visibility = Visibility.Visible;
            var phaseLabel = GoalPhaseLabel(goal.Phase, goal.Activation);
            GoalBarPhase.Text = phaseLabel;
            GoalBarObjective.Text = goal.Objective;
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(
                GoalBar, L("目标") + "：" + phaseLabel + " · " + goal.Objective);
        }
        catch (Exception)
        {
            // 同上：UI 构造异常不上抛
        }
    }

    /// <summary>官方 GoalBar 的阶段文案：phase（active/paused/blocked） × activation
    /// （armed/disarmed）——active + disarmed = 待命（未武装），active = 进行中。
    /// activation 为空表示只拿到投影增量（投影不带该字段），按进行中显示。</summary>
    private string GoalPhaseLabel(string phase, string activation) => (phase, activation) switch
    {
        ("active", "disarmed") => L("目标待命"),
        ("active", _) => L("目标进行中"),
        ("paused", _) => L("目标已暂停"),
        ("blocked", _) => L("目标受阻"),
        _ => L("目标"),
    };

    /// <summary>目标条「管理」入口：复用目标能力面板（创建/编辑/暂停/恢复/完成）。
    /// 关闭后刷新目标条——面板里的 mutation 直接改写目标状态，条要跟上。</summary>
    private async void OnGoalBarManageClick(object sender, RoutedEventArgs e)
    {
        try
        {
            await ShowCapabilityGoalAsync();
            if (Volatile.Read(ref _activeSessionId) is { } sid)
            {
                await RefreshGoalBarAsync(sid);
            }
        }
        catch (Exception)
        {
            // async void 边界兜底：面板异常不进未处理异常
        }
    }

    /// <summary>投影 goal 键 → 目标条。值 = null | {goal:{…}, roundsStarted,…}（dsh-goal）。
    /// 投影不带 activation：沿用上次 goals/get 的 activation（同 goal id 才沿用，换目标即失效）。
    /// 调用方持 _projectionLock（与 plan/permissions 同一解析入口）。</summary>
    private void ApplyGoalProjection(JsonElement value)
    {
        (string Id, long Revision, string Phase, string Activation, string Objective, int RoundsStarted)? summary = null;
        if (value.ValueKind == JsonValueKind.Object
            && value.TryGetProperty("goal", out var inner) && inner.ValueKind == JsonValueKind.Object)
        {
            var parsed = ParseGoalSummary(inner);
            if (parsed is { } p)
            {
                string activation;
                lock (_goalLock)
                {
                    activation = _goalSummary is { Id: var lastId } last && lastId == p.Id ? last.Activation : "";
                }
                summary = (p.Id, p.Revision, p.Phase, activation, p.Objective, p.RoundsStarted);
            }
        }
        lock (_goalLock)
        {
            _goalSummary = summary;
        }
        PostUi(ApplyGoalBar);
    }

    /// <summary>投影 schedule 键 → 计划记录缓存（只读）。值 = {inheritedEventCount, active:[record],
    /// seenIds}（dsh-schedule）；record = {id, kind:"at"|"every", prompt, scheduledAt(RFC3339),
    /// everySeconds?}。调用方持 _projectionLock。</summary>
    private void ApplyScheduleProjection(JsonElement value)
    {
        var records = new List<(string Id, string Kind, string Prompt, string ScheduledAt, long EverySeconds)>();
        if (value.ValueKind == JsonValueKind.Object
            && value.TryGetProperty("active", out var active) && active.ValueKind == JsonValueKind.Array)
        {
            foreach (var record in active.EnumerateArray())
            {
                var id = record.TryGetProperty("id", out var i) && i.ValueKind == JsonValueKind.String ? i.GetString() ?? "" : "";
                var prompt = record.TryGetProperty("prompt", out var p) && p.ValueKind == JsonValueKind.String ? p.GetString() ?? "" : "";
                if (id.Length == 0 && prompt.Length == 0)
                {
                    continue;
                }
                records.Add((
                    id,
                    record.TryGetProperty("kind", out var k) && k.ValueKind == JsonValueKind.String ? k.GetString() ?? "" : "",
                    prompt,
                    record.TryGetProperty("scheduledAt", out var s) && s.ValueKind == JsonValueKind.String ? s.GetString() ?? "" : "",
                    record.TryGetProperty("everySeconds", out var es) && es.ValueKind == JsonValueKind.Number ? (long)es.GetDouble() : 0));
            }
        }
        lock (_scheduleLock)
        {
            _scheduleRecords = records;
            _scheduleSeen = true;
        }
    }

    /// <summary>
    /// 输入区状态同步：计划模式 chip + 权限预设选择器（都读会话投影 plan / permissions）。
    /// 写入只能发命令（/plan、/permission &lt;preset&gt;）——内核没有对应 RPC。
    /// 位置：输入卡左下，无会话或投影尚未到达时各自收起。
    /// </summary>
    private void RefreshSessionStateBar()
    {
        try
        {
            var sid = Volatile.Read(ref _activeSessionId);
            if (_rpc is null)
            {
                PlanChip.Visibility = Visibility.Collapsed;
                PermissionButton.Visibility = Visibility.Collapsed;
                return;
            }
            if (sid is null)
            {
                // 空态（即将开始的会话）：权限选择器显示设置里的默认模式，
                // 选中即改 permission.defaultPreset（与会话态那条"发 /permission 命令"的路互不干扰）
                PlanChip.Visibility = Visibility.Collapsed;
                _ = ShowDefaultPermissionAsync();
                return;
            }
            bool? planActive;
            bool? planPending;
            List<(string Value, string Name, string Description)> options;
            string? current;
            lock (_projectionLock)
            {
                planActive = _planActive;
                planPending = _planPending;
                options = _permissionOptions;
                current = _permissionCurrentValue;
            }

            // ---- 计划模式 ----
            // 只在开启中/切换中时出现；关闭态不占输入区（用户反馈：新会话多出「计划模式已关闭」chip）
            var planOn = planActive == true || planPending == true;
            if (planActive is null || !planOn)
            {
                PlanChip.Visibility = Visibility.Collapsed;
            }
            else
            {
                PlanChip.Visibility = Visibility.Visible;
                PlanChipText.Text = planPending == true
                    ? L("计划模式（切换中…）")
                    : L("计划模式已开启");
                PlanChipExit.Visibility = planActive == true ? Visibility.Visible : Visibility.Collapsed;
            }

            // ---- 权限预设 ----
            // 权限触发器：显示当前预设名（内核 projection 的 name 是英文 id，必须过 PermissionPresetZh）
            if (current is null || options.Count == 0)
            {
                PermissionButton.Visibility = Visibility.Collapsed;
            }
            else
            {
                PermissionButton.Visibility = Visibility.Visible;
                var opt = options.FirstOrDefault(o => o.Value == current);
                var currentName = PermissionPresetZh(current.Length > 0 ? current : opt.Value);
                PermissionLabel.Text = currentName;
                PermissionGlyph.Glyph = PermissionPresetGlyph(current.Length > 0 ? current : opt.Value);
                ToolTipService.SetToolTip(PermissionButton, LF("访问模式，当前：{0}", currentName));
                Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(PermissionButton, LF("访问模式，当前：{0}", currentName));
                PermissionFlyout.Items.Clear();
                foreach (var (value, name, description) in options)
                {
                    // 下拉只留模式名（用户明确要求不要 — 描述）；说明走 ToolTip/Automation
                    var label = PermissionPresetZh(value);
                    var desc = PermissionPresetDescZh(value, description);
                    var item = new RadioMenuFlyoutItem
                    {
                        Text = label,
                        Tag = value,
                        IsChecked = value == current,
                        GroupName = "permission-preset",
                    };
                    if (desc.Length > 0)
                    {
                        ToolTipService.SetToolTip(item, desc);
                    }
                    Aut(item, $"PermissionPreset_{value}", desc.Length > 0 ? LF("访问模式：{0}。{1}", label, desc) : LF("访问模式：{0}", label));
                    item.Click += OnPermissionPresetClick;
                    PermissionFlyout.Items.Add(item);
                }
            }
        }
        catch (Exception)
        {
            // 状态条刷新异常不上抛（0xc000027b 教训）
        }
    }

    /// <summary>权限预设的中文可读名（对照 dsh access.preset.*；内核 projection.name 是英文 id）。</summary>
    private string PermissionPresetZh(string value) => value switch
    {
        "read-only" => L("仅可查看"),
        "workspace-write" => L("工作区内修改"),
        "danger-full-access" => L("完全权限"),
        "auto-approve" => L("自动审批"),
        "custom" => L("自定义"),
        _ => value,
    };

    /// <summary>权限预设的图标（Segoe MDL2/Fluent 码点）：每个预设各用一个、互不重复——
    /// 锁在「自动审批/完全权限」下是反直觉的。只读=查看、工作区内修改=编辑、完全权限=警告、
    /// 自动审批=闪光（Flash 智能预判）、自定义=设置；未知预设回落锁。</summary>
    private static string PermissionPresetGlyph(string value) => value switch
    {
        "read-only" => "\uE890",        // View
        "workspace-write" => "\uE70F",  // Edit
        "danger-full-access" => "\uE7BA", // Warning
        "auto-approve" => "\uF1BA",     // SquareSparkle
        "custom" => "\uE713",           // Settings
        _ => "\uE72E",                  // Lock
    };

    /// <summary>权限预设说明中文（内核 permission-presets 表的英文 description 的产品文案对照）。</summary>
    private string PermissionPresetDescZh(string value, string? kernelDesc = null)
    {
        var zh = value switch
        {
            "read-only" => L("只读：可浏览工作区，不能写文件或执行修改"),
            "workspace-write" => L("可写工作区与允许的临时目录；更广范围的重试需审批"),
            "danger-full-access" => L("完全文件访问，不再弹出审批确认"),
            "auto-approve" => L("Flash 预判写入/命令是否不可回补：安全自动批准，有风险转人工审批"),
            _ => "",
        };
        if (zh.Length > 0)
        {
            return zh;
        }
        return kernelDesc ?? "";
    }

    /// <summary>审批策略中文（dsh-user-approval：ask / never）。</summary>
    private string ApprovalPolicyZh(string value) => value switch
    {
        "ask" => L("需要确认"),
        "never" => L("直接执行"),
        _ => value,
    };

    /// <summary>
    /// 队列/上下文条目的展示文案。内核 user-approval 在切换权限时会注入英文 user 消息
    /// （模型可见的 system 上下文），壳侧只改显示，不改回写 Content。
    /// </summary>
    private string LocalizeQueueDisplayText(string text)
    {
        if (text.Length == 0)
        {
            return text;
        }
        // The approval policy changed from "ask" to "never" (changed by the user).
        const string prefix = "The approval policy changed from \"";
        if (text.StartsWith(prefix, StringComparison.Ordinal))
        {
            var fromStart = prefix.Length;
            var fromEnd = text.IndexOf('"', fromStart);
            if (fromEnd > fromStart)
            {
                var toMarker = "\" to \"";
                var toStart = text.IndexOf(toMarker, fromEnd, StringComparison.Ordinal);
                if (toStart > 0)
                {
                    var toValueStart = toStart + toMarker.Length;
                    var toEnd = text.IndexOf('"', toValueStart);
                    if (toEnd > toValueStart)
                    {
                        var from = text[fromStart..fromEnd];
                        var to = text[toValueStart..toEnd];
                        return LF("审批策略已从「{0}」切换为「{1}」（由你更改）", ApprovalPolicyZh(from), ApprovalPolicyZh(to));
                    }
                }
            }
        }
        return text;
    }

    /// <summary>
    /// 空态权限选择器（尚无活动会话时的样子）：
    /// 显示并编辑设置里的默认权限模式（permission/defaultPreset），即"即将开始的这个会话"的访问模式。
    /// 会话已存在时由 RefreshSessionStateBar 走会话投影那条路（写入口是 /permission 命令）。
    /// </summary>
    private async Task ShowDefaultPermissionAsync()
    {
        var snapshot = await EnsureSettingsSnapshotAsync();
        if (snapshot is null || _activeSessionId is not null)
        {
            return; // 设置取不到或期间已开会话：交给会话态那条路，不显示猜测值
        }
        string current = NsString("permission", "defaultPreset", "workspace-write");
        var choices = new List<(string Value, string Label)>();
        if (snapshot.TryGetValue("permission", out var perm) && perm.Value.TryGetProperty("schema", out var schema))
        {
            foreach (var option in UnionChoices(SchemaField(schema, "defaultPreset")))
            {
                choices.Add((option.Value, PermissionPresetZh(option.Value)));
            }
        }
        if (choices.Count == 0)
        {
            choices.AddRange(new[]
            {
                ("read-only", PermissionPresetZh("read-only")),
                ("workspace-write", PermissionPresetZh("workspace-write")),
                ("danger-full-access", PermissionPresetZh("danger-full-access")),
            });
        }
        PostUi(() =>
        {
            if (_activeSessionId is not null)
            {
                return;
            }
            PermissionButton.Visibility = Visibility.Visible;
            var currentName = choices.FirstOrDefault(c => c.Value == current).Label ?? PermissionPresetZh(current);
            PermissionLabel.Text = currentName;
            PermissionGlyph.Glyph = PermissionPresetGlyph(current);
            ToolTipService.SetToolTip(PermissionButton, LF("访问模式，当前：{0}", currentName));
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(PermissionButton, LF("访问模式，当前：{0}", currentName));
            PermissionFlyout.Items.Clear();
            foreach (var (value, label) in choices)
            {
                var item = new RadioMenuFlyoutItem
                {
                    Text = label,
                    Tag = value,
                    IsChecked = value == current,
                    GroupName = "permission-preset",
                };
                Aut(item, $"PermissionPreset_{value}", LF("访问模式：{0}", label));
                item.Click += async (_, _) => await SetDefaultPermissionAsync(value, label);
                PermissionFlyout.Items.Add(item);
            }
        });
    }

    /// <summary>写回默认权限模式（settings/mutate permission.defaultPreset）——新会话按它开局。</summary>
    private async Task SetDefaultPermissionAsync(string value, string label)
    {
        if (_rpc is null)
        {
            return;
        }
        try
        {
            await _rpc.CallOkAsync("settings/mutate", new
            {
                ns = "permission",
                ops = new[] { new { op = "set", path = new[] { "defaultPreset" }, value } },
            });
            PermissionLabel.Text = label;
            ToolTipService.SetToolTip(PermissionButton, LF("访问模式，当前：{0}", label));
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(PermissionButton, LF("访问模式，当前：{0}", label));
        }
        catch (DshRpcException ex)
        {
            _ = ShowErrorAsync(LF("设置默认权限模式失败：{0}", ex.Message));
        }
    }

    /// <summary>设置快照（settings/describe）。需要读设置值的入口（设置模态、空态权限选择器）共用；
    /// 保存后置空以强制下次重拉。</summary>
    private void ApplyShellSettingsSnapshot()
    {
        if (!DispatcherQueue.HasThreadAccess)
        {
            PostUi(ApplyShellSettingsSnapshot);
            return;
        }
        if (_settingsSnapshot is null) return;
        // 与控件共用解析顺序：未保存草稿 > describe 有效值 > 壳默认值。
        // 只更新展示，不渲染分区、不拉 RPC，避免快照/刷新相互递归。
        ConsumeShellSetting("locale", "preference", NsString("locale", "preference", "zh"));
        ConsumeShellSetting("ui-chat", "transcriptView", NsString("ui-chat", "transcriptView", "compact"));
    }

    private async Task<Dictionary<string, (JsonElement Value, double Revision)>?> EnsureSettingsSnapshotAsync()
    {
        if (_rpc is null)
        {
            return null;
        }
        if (_settingsSnapshot is not null)
        {
            return _settingsSnapshot;
        }
        try
        {
            var described = await _rpc.CallOkAsync("settings/describe", new { });
            var map = new Dictionary<string, (JsonElement, double)>();
            foreach (var ns in described.GetProperty("namespaces").EnumerateArray())
            {
                var name = ns.GetProperty("ns").GetString() ?? "";
                map[name] = (ns.Clone(), ns.TryGetProperty("revision", out var rev) ? rev.GetDouble() : 0);
            }
            _settingsSnapshot = map;
            ApplyShellSettingsSnapshot();
            return map;
        }
        catch (Exception ex) when (ex is DshRpcException or InvalidOperationException)
        {
            return null;
        }
    }

    // ---------------- 会话反馈（sessionFeedback/record） ----------------
    //
    // 与 messageFeedback 的区别（读内核 dsh-command-feedback 确认）：
    //   messageFeedback/put|delete  针对**单条助手消息**（messageId + 赞/踩 rating + ifVersion）
    //   sessionFeedback/record      针对**整个会话**的一条备注，无 rating；
    //                               请求是 { request: { sessionId, text?, category? } }，
    //                               category ∈ 固定 7 类（FEEDBACK_CATEGORIES）：
    //                                 task-result / instruction-following / product-interaction /
    //                                 service-stability / resource-cost / security-privacy-permission / other
    //                               text 与 category 都是 optional（非 nullable）——空文本必须**不带键**，
    //                               带 null 会被 zod 拒。
    //   返回：{ ok:true, value:{ recorded:true } } 或 { ok:false, error:{ code:"session-not-found", sessionId } }。
    //   语义：只是往会话日志追加一条 feedback/record 事件（recordFeedback 调 session.append），
    //   不做网络上报；本部署的组合里也没有上报插件。

    private async void OnSessionFeedbackClick(object sender, RoutedEventArgs e)
    {
        try
        {
            var sid = Volatile.Read(ref _activeSessionId);
            if (_rpc is null || string.IsNullOrEmpty(sid))
            {
                AppendSystemMessage(L("请先选择一个会话：会话反馈作用在具体会话上。"));
                return;
            }
            var categoryBox = new ComboBox
            {
                Header = L("分类（可留空）"),
                ItemsSource = SessionFeedbackCategories.Select(c => L(c.Label)).ToList(),
                HorizontalAlignment = HorizontalAlignment.Stretch,
            };
            Aut(categoryBox, "SessionFeedbackCategoryBox", L("反馈分类"));
            var textBox = new TextBox
            {
                Header = L("说明（可留空）"),
                PlaceholderText = L("这条会话的体验如何？"),
                AcceptsReturn = true,
                TextWrapping = TextWrapping.Wrap,
                Height = 110,
            };
            Aut(textBox, "SessionFeedbackTextBox", L("反馈说明"));
            var content = new StackPanel { Spacing = Sp10, Width = 380 };
            content.Children.Add(categoryBox);
            content.Children.Add(textBox);
            var dialog = new ContentDialog
            {
                Title = L("会话反馈"),
                Content = content,
                PrimaryButtonText = L("记录"),
                CloseButtonText = L("取消"),
                DefaultButton = ContentDialogButton.Primary,
                XamlRoot = Content.XamlRoot,
            };
            if (await dialog.ShowAsync() != ContentDialogResult.Primary)
            {
                return;
            }
            var text = (textBox.Text ?? "").Trim();
            var category = categoryBox.SelectedIndex >= 0 ? SessionFeedbackCategories[categoryBox.SelectedIndex].Wire : null;

            // text / category 都是 optional：为空时**不带键**（带 null 会被内核 zod 边界拒）
            var request = new Dictionary<string, object> { ["sessionId"] = sid };
            if (text.Length > 0)
            {
                request["text"] = text;
            }
            if (category is not null)
            {
                request["category"] = category;
            }
            var envelope = await _rpc.CallAsync("sessionFeedback/record", new { request });
            if (envelope.TryGetProperty("ok", out var ok) && ok.ValueKind == JsonValueKind.True)
            {
                var value = envelope.TryGetProperty("value", out var v) ? v : default;
                var recorded = value.ValueKind == JsonValueKind.Object &&
                               value.TryGetProperty("ok", out var innerOk) && innerOk.ValueKind == JsonValueKind.True;
                if (recorded)
                {
                    AppendSystemMessage(L("会话反馈已记录（内核会话日志追加 feedback/record 事件）。"));
                }
                else
                {
                    // 业务错误分支：{ ok:false, error:{ code:"session-not-found", sessionId } }
                    var code = value.ValueKind == JsonValueKind.Object && value.TryGetProperty("error", out var err) && err.ValueKind == JsonValueKind.Object
                        ? Str(err, "code") : "";
                    AppendSystemMessage(code == "session-not-found"
                        ? L("记录失败：该会话在内核里已不存在（session-not-found）。")
                        : LF("记录失败：内核返回 {0}。", code));
                }
            }
            else
            {
                var code = envelope.TryGetProperty("error", out var err) && err.ValueKind == JsonValueKind.Object ? Str(err, "code") : "";
                var message = envelope.TryGetProperty("error", out var err2) && err2.ValueKind == JsonValueKind.Object ? Str(err2, "message") : "";
                AppendSystemMessage(LF("记录失败：[{0}] {1}", code, message));
            }
        }
        catch (Exception ex)
        {
            AppendSystemMessage(LF("会话反馈失败：{0}", ex.Message));
        }
    }

    /// <summary>内核 FEEDBACK_CATEGORIES 的线值与中文标签（顺序与内核一致：产品呈现顺序）。</summary>
    private static readonly (string Wire, string Label)[] SessionFeedbackCategories =
    {
        ("task-result", "任务结果"),
        ("instruction-following", "指令遵循"),
        ("product-interaction", "产品交互"),
        ("service-stability", "服务稳定性"),
        ("resource-cost", "资源消耗"),
        ("security-privacy-permission", "安全隐私与权限"),
        ("other", "其他"),
    };

    /// <summary>计划模式切换：走内核 /plan 命令（进入）与 /plan off（退出）。
    /// 命令回执由 commands/execute 的 result.text 呈现，投影随后到达会刷新 chip。</summary>
    private async void OnPlanToggleClick(object sender, RoutedEventArgs e)
    {
        try
        {
            bool active;
            lock (_projectionLock)
            {
                active = _planActive == true;
            }
            await ExecuteCommandAsync(active ? "/plan off" : "/plan");
        }
        catch (Exception) { } // async void 事件入口兜底（0xc000027b 教训）
    }

    /// <summary>权限预设切换：内核 /permission &lt;preset&gt;（presets 表来自 permissions 投影的 options）。</summary>
    private async void OnPermissionPresetClick(object sender, RoutedEventArgs e)
    {
        try
        {
            if (sender is not FrameworkElement { Tag: string preset } || preset.Length == 0)
            {
                return;
            }
            await ExecuteCommandAsync($"/permission {preset}");
        }
        catch (Exception) { }
    }

    private StackPanel MakeQueueHeaderRow(string title, string? hint)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp8 };
        row.Children.Add(new TextBlock
        {
            Text = title,
            Style = AppStyle("BodyStrongTextStyle"),
            Foreground = ThemeBrush("TextSecondaryBrush"),
        });
        if (hint is not null)
        {
            row.Children.Add(new TextBlock
            {
                Text = hint,
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextTertiaryBrush"),
                VerticalAlignment = VerticalAlignment.Center,
            });
        }
        return row;
    }

    /// <summary>一条排队项：文本 + 位置标签 + 编辑/插话/删除。</summary>
    private FrameworkElement MakeQueueRow(QueueItemVm item)
    {
        var grid = new Grid { ColumnSpacing = Sp8 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var badge = new Border
        {
            Background = ThemeBrush("ControlFillBrush"),
            CornerRadius = new CornerRadius(TokenDouble("RadiusSmall", 4)),
            Padding = TokenThickness("ChipPaddingTight", new Thickness(6, 1, 6, 1)),
            VerticalAlignment = VerticalAlignment.Center,
            Child = new TextBlock { Text = L(item.PlacementLabel), Style = AppStyle("CaptionTextStyle") },
        };
        Grid.SetColumn(badge, 0);
        grid.Children.Add(badge);

        var displayText = LocalizeQueueDisplayText(item.Text);
        var text = new TextBlock
        {
            Text = displayText.Length == 0 ? L("(空)") : displayText,
            TextTrimming = TextTrimming.CharacterEllipsis,
            TextWrapping = TextWrapping.NoWrap,
            VerticalAlignment = VerticalAlignment.Center,
        };
        Grid.SetColumn(text, 1);
        grid.Children.Add(text);

        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp4 };
        var edit = Aut(new Button { Content = L("编辑"), Style = AppStyle("CompactButtonStyle") }, $"EditQueueItem_{item.ItemId}", L("编辑排队项"));
        edit.Click += (_, _) => _ = EditQueueItemAsync(item);
        var steer = new Button
        {
            Content = L("插话"),
            Style = AppStyle("CompactButtonStyle"),
            // 已在 next-step 的项无需再插话（基线里 placement 已是 steering）
            IsEnabled = item.Placement != "steering",
        };
        steer.Click += (_, _) => _ = UpdateQueueAsync(item, "steer", null);
        var remove = Aut(new Button { Content = L("删除"), Style = AppStyle("CompactButtonStyle") }, $"RemoveQueueItem_{item.ItemId}", L("删除排队项"));
        remove.Click += (_, _) => _ = UpdateQueueAsync(item, "remove", null);
        actions.Children.Add(edit);
        actions.Children.Add(steer);
        actions.Children.Add(remove);
        Grid.SetColumn(actions, 2);
        grid.Children.Add(actions);
        return grid;
    }

    /// <summary>作业下拉行（对标官方 JobListAction 的行）：状态点 | kind 徽章 | 标签（等宽、
    /// 省略）| 右侧 状态/时长（detail 优先于状态词，同官方 status 列口径）。时长由调用方
    /// 传入当前时钟现算，live 行随 1s tick 走时。</summary>
    private FrameworkElement MakeJobRow(JobVm job, long nowEpochMs)
    {
        var grid = new Grid { ColumnSpacing = Sp6, Padding = new Thickness(0, 3, 0, 3) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });                          // 0 状态点
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });                          // 1 kind 徽章
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });     // 2 标签
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });                          // 3 状态/时长

        var dot = new Microsoft.UI.Xaml.Shapes.Ellipse
        {
            Width = StatusDot,
            Height = StatusDot,
            VerticalAlignment = VerticalAlignment.Center,
            Fill = job.Status switch
            {
                // 官方 JobListAction 的 dotState 语义：stopping 与 killed 同为"按意愿结束"
                // 的注意色，completed 为完成色，failed 为错误色
                "running" => ThemeBrush("InfoBrush"),
                "stopping" or "killed" => ThemeBrush("WarningBrush"),
                "completed" => ThemeBrush("SuccessBrush"),
                "failed" => ThemeBrush("ErrorBrush"),
                _ => ThemeBrush("TextTertiaryBrush"),
            },
        };
        Grid.SetColumn(dot, 0);
        grid.Children.Add(dot);

        // kind 徽章（bash-1 / subagent-2…）：内核生成的类型标识，与标签分列才可扫读。
        // 徽章缺 kind（旧内核）时整块不占位。
        if (job.Kind is { Length: > 0 } kind)
        {
            var chip = new Border
            {
                Background = ThemeBrush("ControlFillBrush"),
                CornerRadius = new CornerRadius(4),
                Padding = new Thickness(5, 1, 5, 1),
                VerticalAlignment = VerticalAlignment.Center,
                Child = new TextBlock
                {
                    Text = kind,
                    Style = AppStyle("CaptionTextStyle"),
                    Foreground = ThemeBrush("TextSecondaryBrush"),
                },
            };
            Grid.SetColumn(chip, 1);
            grid.Children.Add(chip);
        }

        var label = new TextBlock
        {
            Text = job.Detail is { Length: > 0 } ? $"{job.Label} · {job.Detail}" : job.Label,
            TextTrimming = TextTrimming.CharacterEllipsis,
            TextWrapping = TextWrapping.NoWrap,
            VerticalAlignment = VerticalAlignment.Center,
            Style = AppStyle("CodeTextStyle"),
        };
        ToolTipService.SetToolTip(label, label.Text);
        Grid.SetColumn(label, 2);
        grid.Children.Add(label);

        var duration = job.DurationText(nowEpochMs);
        var statusText = job.Detail is { Length: > 0 } ? L(job.StatusLabel) : L(job.StatusLabel);
        var right = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp6, VerticalAlignment = VerticalAlignment.Center };
        right.Children.Add(new TextBlock
        {
            Text = statusText,
            Style = AppStyle("CaptionTextStyle"),
            Foreground = ThemeBrush("TextTertiaryBrush"),
            VerticalAlignment = VerticalAlignment.Center,
        });
        if (duration is { Length: > 0 } d)
        {
            right.Children.Add(new TextBlock
            {
                Text = d,
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextTertiaryBrush"),
                VerticalAlignment = VerticalAlignment.Center,
            });
        }
        Grid.SetColumn(right, 3);
        grid.Children.Add(right);
        return grid;
    }

    /// <summary>打开编辑对话框：只替换文本块，图片/文件块按内核原始结构原样回写
    /// （updateQueue 的 edit.content 是 content 块数组，不是 prompt 的内联 base64 形态）。</summary>
    private async Task EditQueueItemAsync(QueueItemVm item)
    {
        if (_rpc is null)
        {
            return;
        }
        var box = Aut(new TextBox
        {
            Text = item.Text,
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
            MinWidth = TokenDouble("DialogMinWidth", 420),
            MaxHeight = 220,
        }, "EditQueueItemTextBox", L("排队项内容"));
        var dialog = new ContentDialog
        {
            Title = L("编辑排队消息"),
            Content = box,
            PrimaryButtonText = L("保存"),
            CloseButtonText = L("取消"),
            DefaultButton = ContentDialogButton.Primary,
            XamlRoot = Content.XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }
        await UpdateQueueAsync(item, "edit", box.Text);
    }

    /// <summary>session/updateQueue：{request:{sessionId, itemId, action}}，
    /// action = {kind:"edit", content:[…]} | {kind:"remove"} | {kind:"steer"} → {accepted:true}。</summary>
    private async Task UpdateQueueAsync(QueueItemVm item, string kind, string? text)
    {
        if (_rpc is null)
        {
            return;
        }
        object action = kind switch
        {
            "edit" => new { kind = "edit", content = EditedContent(item, text ?? "") },
            "steer" => new { kind = "steer" as object },
            _ => new { kind = "remove" as object },
        };
        try
        {
            await _rpc.CallOkAsync("session/updateQueue", new
            {
                request = new { sessionId = item.SessionId, itemId = item.ItemId, action },
            });
        }
        catch (DshRpcException ex)
        {
            // session/queue-item-not-found = 该条已被内核消费或删除（不是错误状态，刷新即可）
            if (ex.Message.Contains("no longer pending", StringComparison.OrdinalIgnoreCase))
            {
                RefreshQueuePanel();
                return;
            }
            _ = ShowErrorAsync(LF("队列操作失败：{0}", ex.Message));
            return;
        }
        // 本地乐观更新（内核随后会推 queue 帧覆盖）：面板不必等流帧
        lock (_controlLock)
        {
            if (_queues.TryGetValue(item.SessionId, out var list))
            {
                var index = list.FindIndex(x => x.ItemId == item.ItemId);
                if (index >= 0)
                {
                    if (kind == "edit")
                    {
                        list[index] = new QueueItemVm
                        {
                            SessionId = item.SessionId,
                            ItemId = item.ItemId,
                            Placement = item.Placement,
                            Text = text ?? "",
                            Content = item.Content,
                        };
                    }
                    else if (kind == "remove")
                    {
                        list.RemoveAt(index);
                    }
                }
            }
        }
        RefreshQueuePanel();
    }

    /// <summary>edit 的 content：内核原始块数组里的首个 text 块换成新文本，其余块（图片/文件）原样保留。</summary>
    private static List<object> EditedContent(QueueItemVm item, string text)
    {
        var content = new List<object>();
        var replaced = false;
        if (item.Content.ValueKind == JsonValueKind.Array)
        {
            foreach (var block in item.Content.EnumerateArray())
            {
                var isText = block.TryGetProperty("type", out var t) && t.GetString() == "text";
                if (isText && !replaced)
                {
                    content.Add(new { type = "text", text });
                    replaced = true;
                }
                else
                {
                    content.Add(block); // JsonElement 原样序列化回线格式
                }
            }
        }
        if (!replaced)
        {
            content.Insert(0, new { type = "text", text });
        }
        return content;
    }

    // ---------------- 工作区文件右栏（workspaceFiles/*） ----------------

    /// <summary>「+」菜单里的右栏开关：展开即拉取当前会话工作区（workspaceFiles/list）。</summary>
    private void OnFilesPanelToggleClick(object sender, RoutedEventArgs e)
    {
        try
        {
            SetFilesPanelOpen(FilesPanelView.Visibility != Visibility.Visible);
        }
        catch (Exception) { } // 事件入口兜底（0xc000027b 教训）
    }

    /// <summary>右栏开合的唯一入口（菜单项与右栏自身的关闭钮同路）。</summary>
    private void SetFilesPanelOpen(bool open)
    {
        FilesPanelView.Visibility = open ? Visibility.Visible : Visibility.Collapsed;
        FilesPanelView.OnVisibilityChanged(open);
    }

    // ---------------- 消息反馈（messageFeedback/*） ----------------

    /// <summary>会话切换时清空反馈状态（条目与行都按会话作用域）。</summary>
    private void ResetFeedbackState()
    {
        _feedback.Clear();
        _feedbackRows.Clear();
        _rowsByBubble.Clear();
        _feedbackBusy.Clear();
    }

    /// <summary>
    /// 装配助手气泡统一操作行（官方 MessageIconActions 口径）：
    ///   反馈组（赞/踩/说明/撤销，仅 message.id 存在时）＋ 复制 ＋ 在新对话中分支 ＋ 时间戳 ＋ 本轮用时。
    /// 每次 Loaded 重建，天然幂等（ListView 虚拟化会回收容器并重发 Loaded）。
    /// 行始终在布局里（Opacity 常驻 0.35，悬停/键盘聚焦/已反馈时 1.0）——浮在透明态
    /// 也能被 Tab 聚焦，所以"悬停显示 + 键盘可达"同时成立。
    /// </summary>
    private void BuildFeedbackRow(ContentControl host)
    {
        if (host.DataContext is not ChatBubble bubble || host.Parent is not StackPanel content)
        {
            return;
        }
        for (var i = content.Children.Count - 1; i >= 0; i--)
        {
            if (content.Children[i] is FrameworkElement { Tag: "feedback-row" })
            {
                content.Children.RemoveAt(i);
            }
        }

        var row = new FeedbackRow { MessageId = bubble.MessageId ?? "", Bubble = bubble };
        row.Root = new StackPanel
        {
            Tag = "feedback-row",
            Orientation = Orientation.Horizontal,
            Spacing = TokenDouble("Space4", 4),
            Margin = new Thickness(0, TokenDouble("Space4", 4), 0, 0),
            Opacity = row.IdleOpacity,
        };

        // 反馈组：内核按 assistant/message 的 message.id 校验，无 id 的气泡（错误占位）不给入口
        if (bubble.MessageId is { Length: > 0 })
        {
            row.Like = MakeFeedbackToggle("\uE8E1", L("有帮助（再点一次撤销）"), "FeedbackLike_" + bubble.MessageId, () => _ = RateAsync(bubble, "positive"));
            row.Dislike = MakeFeedbackToggle("\uE8E0", L("没帮助（再点一次撤销）"), "FeedbackDislike_" + bubble.MessageId, () => _ = RateAsync(bubble, "negative"));
            row.Note = MakeFeedbackButton(L("添加说明"), "FeedbackNote_" + bubble.MessageId, () => _ = EditFeedbackNoteAsync(bubble));
            row.NoteText = new TextBlock
            {
                Style = Application.Current.Resources.TryGetValue("HintTextStyle", out var hint) && hint is Style hs ? hs : null,
                MaxWidth = 420,
                TextTrimming = TextTrimming.CharacterEllipsis,
                TextWrapping = TextWrapping.NoWrap,
                VerticalAlignment = VerticalAlignment.Center,
                Visibility = Visibility.Collapsed,
                Margin = new Thickness(TokenDouble("Space4", 4), 0, 0, 0),
            };
            row.Revoke = MakeFeedbackButton(L("撤销"), "FeedbackRevoke_" + bubble.MessageId, () => _ = DeleteFeedbackAsync(bubble.MessageId));
            row.Status = new TextBlock
            {
                Style = Application.Current.Resources.TryGetValue("HintTextStyle", out var hint2) && hint2 is Style hs2 ? hs2 : null,
                VerticalAlignment = VerticalAlignment.Center,
                Visibility = Visibility.Collapsed,
                Margin = new Thickness(TokenDouble("Space4", 4), 0, 0, 0),
            };
            foreach (var child in new UIElement?[] { row.Like, row.Dislike, row.Note, row.NoteText, row.Revoke, row.Status })
            {
                if (child is not null) row.Root.Children.Add(child);
            }
            _feedbackRows[bubble.MessageId] = row;
        }

        // 操作组：复制 / 分支 / 时间戳 / 本轮用时（官方 TurnTailNodeView 的动作排序）
        row.Copy = MakeCopyButton(bubble.Text);
        row.Root.Children.Add(row.Copy);
        // 分支只挂在该轮答案气泡上（官方：仅已完成轮次的最后一条消息可分支）
        var isTurnAnswer = _transcriptAnswers.TryGetValue(bubble.Turn, out var answer) && ReferenceEquals(answer, bubble);
        if (isTurnAnswer && bubble.Seq > 0)
        {
            row.Branch = new Button
            {
                Width = SmallButtonSize,
                Height = SmallButtonSize,
                MinWidth = 0,
                Padding = new Thickness(0),
                CornerRadius = RadSmall,
                VerticalAlignment = VerticalAlignment.Center,
                Content = new FontIcon { Glyph = "\uE8A7", FontSize = GlyphBody }, // OpenInNewWindow
            };
            Aut(row.Branch, "MessageBranchButton", L("在新对话中分支"));
            row.Branch.Click += (_, _) =>
            {
                var sid = Volatile.Read(ref _activeSessionId);
                if (sid is { Length: > 0 } && bubble.Seq > 0 && _closedTranscriptTurns.Contains(bubble.Turn))
                {
                    _ = ForkSessionAtAsync(sid, bubble.Seq);
                }
            };
            row.Root.Children.Add(row.Branch);
        }
        if (bubble.Time > 0)
        {
            row.TimeText = new TextBlock
            {
                Style = Application.Current.Resources.TryGetValue("HintTextStyle", out var hint3) && hint3 is Style hs3 ? hs3 : null,
                VerticalAlignment = VerticalAlignment.Center,
            };
            row.Root.Children.Add(row.TimeText);
        }
        // 型号跟在时间后面（与用户消息行同序）：取内核 request/header 记的实际模型 id
        row.ModelText = new TextBlock
        {
            Style = Application.Current.Resources.TryGetValue("HintTextStyle", out var hint5) && hint5 is Style hs5 ? hs5 : null,
            VerticalAlignment = VerticalAlignment.Center,
            Visibility = Visibility.Collapsed,
        };
        row.Root.Children.Add(row.ModelText);
        // 用量段（对标官方 TurnTailNodeView 的 token 槽）：堆叠图标 + 「用量 X tok」，
        // 无 usage 记录（0）整体收起不占位；位于用时之前（官方同序）。
        row.TokensGroup = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = TokenDouble("Space6", 6),
            VerticalAlignment = VerticalAlignment.Center,
            Visibility = Visibility.Collapsed,
        };
        row.TokensGroup.Children.Add(new FontIcon { Glyph = "\uE81E", FontSize = 12, Opacity = 0.8 });
        row.TokensText = new TextBlock
        {
            Style = Application.Current.Resources.TryGetValue("HintTextStyle", out var hint6) && hint6 is Style hs6 ? hs6 : null,
            VerticalAlignment = VerticalAlignment.Center,
        };
        row.TokensGroup.Children.Add(row.TokensText);
        row.Root.Children.Add(row.TokensGroup);
        if (isTurnAnswer)
        {
            row.DurationText = new TextBlock
            {
                Style = Application.Current.Resources.TryGetValue("HintTextStyle", out var hint4) && hint4 is Style hs4 ? hs4 : null,
                VerticalAlignment = VerticalAlignment.Center,
                Visibility = Visibility.Collapsed,
            };
            row.Root.Children.Add(row.DurationText);
        }

        // 轮尾文件改动区先于动作行（官方 turnTail 槽渲染在 MessageIconActions 之前）
        if (isTurnAnswer)
        {
            BuildProducedFilesRow(bubble, row, content);
        }

        content.Children.Add(row.Root);
        _rowsByBubble[bubble] = row;
        RenderFeedbackRow(row);
        RenderTurnActions(row);
        HookBubbleHover(content, bubble);
    }

    /// <summary>按当前状态重绘操作组的动态部分（时间戳、用时、分支可用态与提示）。</summary>
    private void RenderTurnActions(FeedbackRow row)
    {
        var bubble = row.Bubble;
        if (row.TimeText is not null)
        {
            row.TimeText.Text = FormatMessageClock(bubble.Time);
        }
        if (row.ModelText is not null)
        {
            var showModel = bubble.Model.Length > 0;
            row.ModelText.Visibility = showModel ? Visibility.Visible : Visibility.Collapsed;
            if (showModel)
            {
                row.ModelText.Text = "· " + bubble.Model;
            }
        }
        if (row.DurationText is not null)
        {
            var showDuration = bubble.DurationMs > 0;
            row.DurationText.Visibility = showDuration ? Visibility.Visible : Visibility.Collapsed;
            if (showDuration)
            {
                row.DurationText.Text = LF("用时 {0}", FormatTurnDuration(bubble.DurationMs));
            }
        }
        if (row.TokensGroup is not null && row.TokensText is not null)
        {
            var showTokens = bubble.Tokens > 0;
            row.TokensGroup.Visibility = showTokens ? Visibility.Visible : Visibility.Collapsed;
            if (showTokens)
            {
                row.TokensText.Text = LF("用量 {0} tok", FormatTokensCompact(bubble.Tokens));
            }
        }
        if (row.Branch is not null)
        {
            var canBranch = _closedTranscriptTurns.Contains(bubble.Turn);
            row.Branch.IsEnabled = canBranch;
            ToolTipService.SetToolTip(row.Branch, L(canBranch ? "在新对话中分支" : "仅可从已完成轮次的最后一条消息分支"));
        }
        RefreshProducedChips(row);
    }

    /// <summary>turn/end 回填本轮用时、轮次闭合后分支转可用：按气泡定位行重绘。</summary>
    private void RefreshTurnActionsRow(ChatBubble bubble)
    {
        if (_rowsByBubble.TryGetValue(bubble, out var row))
        {
            RenderTurnActions(row);
        }
    }

    /// <summary>悬停/聚焦显隐：挂在气泡卡（模板根 Border，<see cref="ContentControl"/> 的逻辑父）上。</summary>
    private void HookBubbleHover(StackPanel content, ChatBubble bubble)
    {
        if (Microsoft.UI.Xaml.Media.VisualTreeHelper.GetParent(content) is not Border border ||
            border.Tag is "feedback-hover")
        {
            return;
        }
        border.Tag = "feedback-hover";
        border.PointerEntered += (_, _) => SetFeedbackHover(bubble, true);
        border.PointerExited += (_, _) => SetFeedbackHover(bubble, false);
        border.GotFocus += (_, _) => SetFeedbackFocus(bubble, true);
        // LostFocus 会因焦点在行内按钮之间转移而连续触发：编组到下一轮再按"焦点是否还在卡内"定夺
        border.LostFocus += (_, _) => PostUi(() => SetFeedbackFocus(bubble, IsFocusWithin(border)));
    }

    private void SetFeedbackHover(ChatBubble bubble, bool hover)
    {
        // 行定位走 _rowsByBubble：无反馈入口的气泡（仅操作组）同样享受悬停提亮
        if (!_rowsByBubble.TryGetValue(bubble, out var row))
        {
            return;
        }
        row.Hover = hover;
        ApplyFeedbackOpacity(row);
    }

    private void SetFeedbackFocus(ChatBubble bubble, bool focused)
    {
        if (!_rowsByBubble.TryGetValue(bubble, out var row))
        {
            return;
        }
        row.Focused = focused;
        ApplyFeedbackOpacity(row);
    }

    private static bool IsFocusWithin(FrameworkElement root)
    {
        var focused = Microsoft.UI.Xaml.Input.FocusManager.GetFocusedElement(root.XamlRoot) as DependencyObject;
        while (focused is not null)
        {
            if (ReferenceEquals(focused, root))
            {
                return true;
            }
            focused = Microsoft.UI.Xaml.Media.VisualTreeHelper.GetParent(focused);
        }
        return false;
    }

    private ToggleButton MakeFeedbackToggle(string glyph, string tip, string automationId, Action onClick)
    {
        var button = new ToggleButton
        {
            Width = SmallButtonSize,
            Height = SmallButtonSize,
            MinWidth = 0,
            Padding = new Thickness(0),
            CornerRadius = RadSmall,
            VerticalAlignment = VerticalAlignment.Center,
            Content = new FontIcon { Glyph = glyph, FontSize = GlyphBody },
        };
        ToolTipService.SetToolTip(button, tip);
        Aut(button, automationId, tip);
        button.Click += (_, _) => { try { onClick(); } catch (Exception) { } };
        return button;
    }

    private Button MakeFeedbackButton(string label, string automationId, Action onClick)
    {
        var button = new Button
        {
            MinWidth = 0,
            Height = SmallButtonSize,
            Padding = new Thickness(TokenDouble("Space8", 8), 0, TokenDouble("Space8", 8), 0),
            CornerRadius = RadSmall,
            VerticalAlignment = VerticalAlignment.Center,
            Content = label,
            Visibility = Visibility.Collapsed,
        };
        Aut(button, automationId, label);
        button.Click += (_, _) => { try { onClick(); } catch (Exception) { } };
        return button;
    }

    /// <summary>按当前状态重画一行（按钮选中态、说明/撤销可见性、行不透明度）。</summary>
    private void RenderFeedbackRow(FeedbackRow row)
    {
        FeedbackItem? item = null;
        var has = row.MessageId.Length > 0 && _feedback.TryGetValue(row.MessageId, out item);
        if (row.Like is not null && row.Dislike is not null)
        {
            row.Like.IsChecked = has && item!.Rating == "positive";
            row.Dislike.IsChecked = has && item!.Rating == "negative";
        }
        if (row.Note is not null)
        {
            row.Note.Visibility = has ? Visibility.Visible : Visibility.Collapsed;
        }
        if (row.Revoke is not null)
        {
            row.Revoke.Visibility = has ? Visibility.Visible : Visibility.Collapsed;
        }
        var note = has ? item!.Note : null;
        if (row.NoteText is not null)
        {
            row.NoteText.Text = string.IsNullOrEmpty(note) ? "" : LF("说明：{0}", note);
            row.NoteText.Visibility = string.IsNullOrEmpty(note) ? Visibility.Collapsed : Visibility.Visible;
        }
        if (row.Note is not null)
        {
            row.Note.Content = string.IsNullOrEmpty(note) ? L("添加说明") : L("编辑说明");
        }
        ApplyFeedbackOpacity(row);
    }

    private void ApplyFeedbackOpacity(FeedbackRow row)
    {
        var has = row.MessageId.Length > 0 && _feedback.ContainsKey(row.MessageId);
        row.Root.Opacity = has || row.Hover || row.Focused ? 1 : row.IdleOpacity;
    }

    private void RenderAllFeedbackRows()
    {
        foreach (var row in _feedbackRows.Values)
        {
            RenderFeedbackRow(row);
        }
    }

    private void SetFeedbackStatus(string messageId, string? text)
    {
        if (!_feedbackRows.TryGetValue(messageId, out var row) || row.Status is null)
        {
            return;
        }
        row.Status.Text = text ?? "";
        row.Status.Visibility = string.IsNullOrEmpty(text) ? Visibility.Collapsed : Visibility.Visible;
    }

    /// <summary>会话打开后拉一次已提交反馈（messageFeedback/list，按会话回读日志）。</summary>
    private async Task LoadFeedbackAsync(string sessionId)
    {
        if (_rpc is null)
        {
            return;
        }
        try
        {
            var (ok, payload) = await CallFeedbackAsync("messageFeedback/list", new { request = new { sessionId } });
            if (!ok)
            {
                return; // 业务失败（session-not-found 等）不打扰聊天
            }
            _feedback.Clear();
            if (payload.GetProperty("value").TryGetProperty("items", out var items) && items.ValueKind == JsonValueKind.Array)
            {
                foreach (var item in items.EnumerateArray())
                {
                    StoreFeedbackItem(item);
                }
            }
            RenderAllFeedbackRows();
        }
        catch (Exception)
        {
            // 反馈状态拉取失败不阻塞聊天主流程
        }
    }

    /// <summary>
    /// messageFeedback 的调用：内核方法的返回体本身就是 {ok,value|error} 业务结果，
    /// 因此 RPC 信封里是"两层 ok"——外层 = typert/gateway 层，内层 = 反馈业务层。
    /// </summary>
    private async Task<(bool Ok, JsonElement Payload)> CallFeedbackAsync(string endpoint, object args)
    {
        var envelope = await _rpc!.CallAsync(endpoint, args);
        if (!envelope.TryGetProperty("ok", out var okFlag) || okFlag.ValueKind != JsonValueKind.True)
        {
            throw DshError(envelope, endpoint);
        }
        var value = envelope.TryGetProperty("value", out var v) ? v : default;
        var inner = value.ValueKind == JsonValueKind.Object &&
                    value.TryGetProperty("ok", out var io) && io.ValueKind == JsonValueKind.True;
        return (inner, value);
    }

    private static DshRpcException DshError(JsonElement envelope, string endpoint)
    {
        if (envelope.TryGetProperty("error", out var err) && err.ValueKind == JsonValueKind.Object)
        {
            var code = err.TryGetProperty("code", out var c) ? c.GetString() ?? "unknown" : "unknown";
            var message = err.TryGetProperty("message", out var m) ? m.GetString() ?? endpoint : endpoint;
            return new DshRpcException(code, message);
        }
        return new DshRpcException("unknown", TLF("{0} 失败", endpoint));
    }

    private void StoreFeedbackItem(JsonElement item)
    {
        if (item.ValueKind != JsonValueKind.Object)
        {
            return;
        }
        var id = item.TryGetProperty("messageId", out var mid) ? mid.GetString() : null;
        if (string.IsNullOrEmpty(id))
        {
            return;
        }
        _feedback[id] = new FeedbackItem
        {
            MessageId = id,
            Rating = item.TryGetProperty("rating", out var r) ? r.GetString() ?? "" : "",
            Note = item.TryGetProperty("note", out var n) && n.ValueKind == JsonValueKind.String ? n.GetString() : null,
            Category = item.TryGetProperty("category", out var c) && c.ValueKind == JsonValueKind.String ? c.GetString() : null,
            Version = item.TryGetProperty("version", out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null,
        };
    }

    /// <summary>赞/踩：同一评价再点一次 = 撤销（与原版 retract 同义）。</summary>
    private async Task RateAsync(ChatBubble bubble, string rating)
    {
        if (bubble.MessageId is not { Length: > 0 } id)
        {
            return;
        }
        var current = _feedback.TryGetValue(id, out var item) ? item : null;
        if (current?.Rating == rating)
        {
            await DeleteFeedbackAsync(id);
            return;
        }
        // 改判时保留已写的说明（put 是整体替换，不带 note 键会丢掉它）
        await PutFeedbackAsync(id, rating, current?.Note);
    }

    /// <summary>
    /// messageFeedback/put：request{ sessionId, messageId, rating, note?, category?, ifVersion }。
    /// note/category 在内核是 optional（非 nullable）——没有说明时必须**不带键**，带 null 会被 zod 拒。
    /// ifVersion 用观察到的版本（无反馈时 null）；version-conflict 时采纳内核回的 current 重试一次。
    /// </summary>
    private async Task PutFeedbackAsync(string messageId, string rating, string? note)
    {
        var sid = Volatile.Read(ref _activeSessionId);
        if (_rpc is null || string.IsNullOrEmpty(sid) || !_feedbackBusy.Add(messageId))
        {
            return;
        }
        try
        {
            for (var attempt = 0; ; attempt++)
            {
                var current = _feedback.TryGetValue(messageId, out var item) ? item : null;
                var request = new Dictionary<string, object?>
                {
                    ["sessionId"] = sid,
                    ["messageId"] = messageId,
                    ["rating"] = rating,
                    ["ifVersion"] = current?.Version,
                };
                if (!string.IsNullOrEmpty(note))
                {
                    request["note"] = note;
                }
                if (!string.IsNullOrEmpty(current?.Category))
                {
                    request["category"] = current!.Category;
                }

                var (ok, payload) = await CallFeedbackAsync("messageFeedback/put", new { request });
                if (ok)
                {
                    StoreFeedbackItem(payload.GetProperty("value"));
                    RenderAllFeedbackRows();
                    SetFeedbackStatus(messageId, null);
                    return;
                }
                var error = payload.GetProperty("error");
                var code = error.TryGetProperty("code", out var c) ? c.GetString() : null;
                if (code == "version-conflict" && attempt == 0)
                {
                    AdoptFeedbackConflict(messageId, error);
                    continue;
                }
                SetFeedbackStatus(messageId, FeedbackErrorText(code, error));
                return;
            }
        }
        catch (Exception ex) when (ex is DshRpcException or InvalidOperationException or KeyNotFoundException)
        {
            SetFeedbackStatus(messageId, ex.Message);
        }
        finally
        {
            _feedbackBusy.Remove(messageId);
        }
    }

    /// <summary>
    /// messageFeedback/delete：request{ sessionId, messageId, ifVersion }。
    /// 契约要点（探针实测）：delete 的 ifVersion 是 required **string**——与 put 的
    /// z.union([z.literal(null), z.string()]) 不同，传 null 会被内核拒成 gateway/input-invalid。
    /// 因此本地没有观察到的版本时先 list 对齐，内核侧也没有就直接本地撤销（幂等）。
    /// </summary>
    private async Task DeleteFeedbackAsync(string messageId)
    {
        var sid = Volatile.Read(ref _activeSessionId);
        if (_rpc is null || string.IsNullOrEmpty(sid) || !_feedbackBusy.Add(messageId))
        {
            return;
        }
        try
        {
            var current = _feedback.TryGetValue(messageId, out var known) ? known : null;
            if (current?.Version is null)
            {
                await LoadFeedbackAsync(sid);
                current = _feedback.TryGetValue(messageId, out var aligned) ? aligned : null;
                if (current?.Version is null)
                {
                    _feedback.Remove(messageId);
                    RenderAllFeedbackRows();
                    return;
                }
            }
            for (var attempt = 0; ; attempt++)
            {
                var version = current!.Version;
                var (ok, payload) = await CallFeedbackAsync("messageFeedback/delete", new
                {
                    request = new Dictionary<string, object?>
                    {
                        ["sessionId"] = sid,
                        ["messageId"] = messageId,
                        ["ifVersion"] = version,
                    },
                });
                if (ok)
                {
                    _feedback.Remove(messageId);
                    RenderAllFeedbackRows();
                    SetFeedbackStatus(messageId, null);
                    return;
                }
                var error = payload.GetProperty("error");
                var code = error.TryGetProperty("code", out var c) ? c.GetString() : null;
                if (code == "version-conflict" && attempt == 0)
                {
                    AdoptFeedbackConflict(messageId, error);
                    if (!_feedback.TryGetValue(messageId, out current) || current.Version is null)
                    {
                        return; // 已被撤销：本地状态已对齐
                    }
                    continue;
                }
                SetFeedbackStatus(messageId, FeedbackErrorText(code, error));
                return;
            }
        }
        catch (Exception ex) when (ex is DshRpcException or InvalidOperationException or KeyNotFoundException)
        {
            SetFeedbackStatus(messageId, ex.Message);
        }
        finally
        {
            _feedbackBusy.Remove(messageId);
        }
    }

    /// <summary>version-conflict 的 current 是内核权威值（null = 已被别人撤销）。</summary>
    private void AdoptFeedbackConflict(string messageId, JsonElement error)
    {
        if (error.TryGetProperty("current", out var current) && current.ValueKind == JsonValueKind.Object)
        {
            StoreFeedbackItem(current);
        }
        else
        {
            _feedback.Remove(messageId);
        }
        RenderAllFeedbackRows();
    }

    private string FeedbackErrorText(string? code, JsonElement error) => code switch
    {
        "target-not-found" => L("该消息不在本会话日志里（子代理或已裁剪），无法反馈"),
        "session-not-found" => L("会话不存在或已归档"),
        "note-blank" => L("说明不能是空白"),
        "note-too-large" => error.TryGetProperty("maxBytes", out var mb) && mb.ValueKind == JsonValueKind.Number
            ? LF("说明超长（上限 {0} 字节）", mb.GetInt64())
            : L("说明超长"),
        "version-conflict" => L("反馈已被其他地方改动，请重试"),
        _ => code is null ? L("操作失败") : LF("失败（{0}）", code),
    };

    /// <summary>文字说明：put 同评级 + note（清空说明即只保留评级）。</summary>
    private async Task EditFeedbackNoteAsync(ChatBubble bubble)
    {
        if (bubble.MessageId is not { Length: > 0 } id || !_feedback.TryGetValue(id, out var current))
        {
            return;
        }
        var box = Aut(new TextBox
        {
            Text = current.Note ?? "",
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
            Height = 120,
            PlaceholderText = L("这条回复哪里好 / 哪里不好（可选，纯文本）"),
        }, "FeedbackNoteTextBox", "反馈说明");
        var dialog = new ContentDialog
        {
            Title = L("反馈说明"),
            Content = box,
            PrimaryButtonText = L("保存"),
            CloseButtonText = L("取消"),
            DefaultButton = ContentDialogButton.Primary,
            XamlRoot = Content.XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }
        var note = box.Text.Trim();
        await PutFeedbackAsync(id, current.Rating, note.Length == 0 ? null : note);
    }

    // ---------------- 斜杠命令（commands/*） ----------------

    /// <summary>
    /// 命令目录（会话作用域）：commands/list 的 args 只有 agentId（内核 lookup "agent" = 会话 id）。
    /// 成功按会话缓存；失败**不永久缓存**——旧会话的 agent 由内核惰性拉起，首次调用常报
    /// gateway/lookup-not-found（实测），隔一会儿再问就有；因此失败只留 3 秒重试窗口，
    /// 既避免每次按键都打内核，又能在 agent 就绪后自愈。
    /// </summary>
    private async Task EnsureCommandsAsync()
    {
        var sid = Volatile.Read(ref _activeSessionId);
        if (_rpc is null || string.IsNullOrEmpty(sid))
        {
            return;
        }
        if (_commandsSession == sid && _commandsError is null)
        {
            return;
        }
        if (_commandsError is not null && _commandsErrorAt is { } at &&
            DateTimeOffset.UtcNow - at < TimeSpan.FromSeconds(3))
        {
            return; // 3s 内不重复问
        }
        try
        {
            var value = await _rpc.CallOkAsync("commands/list", new { agentId = sid });
            _commands.Clear();
            foreach (var item in value.EnumerateArray())
            {
                if (item.ValueKind != JsonValueKind.Object)
                {
                    continue;
                }
                var input = item.TryGetProperty("input", out var i) && i.ValueKind == JsonValueKind.Object ? i : default;
                var name = item.TryGetProperty("name", out var n) ? n.GetString() ?? "" : "";
                var description = item.TryGetProperty("description", out var d) ? d.GetString() ?? "" : "";
                _commands.Add(new CommandVm
                {
                    Name = name,
                    Description = LocalizeHostCommand(name, description),
                    Hint = input.ValueKind == JsonValueKind.Object && input.TryGetProperty("hint", out var h) ? h.GetString() ?? "" : "",
                    HasInput = input.ValueKind == JsonValueKind.Object,
                });
            }
            _commandsSession = sid;
            _commandsError = null;
            _commandsErrorAt = null;
        }
        catch (Exception ex)
        {
            // 全类型兜底：网络层异常也不能逃逸（SubmitInputAsync 斜杠命令路径无自己的 catch）
            _commands.Clear();
            _commandsSession = sid;
            _commandsError = ex.Message;
            _commandsErrorAt = DateTimeOffset.UtcNow;
        }
    }

    /// <summary>
    /// 内核内置命令描述的中文本地化：与内核 hostDescription 规则一致——
    /// 仅当内核返回的描述**逐字等于**官方英文原文时才替换为官方中文，第三方/作用域命令的描述原样保留。
    /// 中英文字面量取自内核自带词典：dsh-client-ui-commands/lib/client.js 的 zh/en 两套
    /// （shell 没有客户端 locale 服务，内置命令这 6 条是 CLDR 之外唯一需要中文的固定文案）。
    /// </summary>
    private string LocalizeHostCommand(string name, string description)
    {
        if (HostCommandDescriptions.TryGetValue(name, out var copy) && description == copy.En)
        {
            return L(copy.Zh);
        }
        return description;
    }

    private static readonly Dictionary<string, (string En, string Zh)> HostCommandDescriptions = new(StringComparer.Ordinal)
    {
        ["compact"] = ("Compact older conversation history", "压缩以上对话内容"),
        ["export"] = ("Download this Session log as a ZIP archive", "将当前会话内容导出为 ZIP"),
        ["feedback"] = ("record feedback about this session", "发送关于当前会话的反馈"),
        ["goal"] = ("set or view the goal for a long-running task", "设置或查看长期任务目标"),
        ["permission"] = ("Switch the permission preset (sandbox mode + approval policy)", "切换权限预设（沙箱模式与审批策略）"),
        ["plan"] = ("Enter or leave plan mode", "进入或退出计划模式"),
    };

    private void OnInputTextChanged(object sender, TextChangedEventArgs e)
    {
        try
        {
            UpdateCommandPalette();
            UpdateReferencePalette();
        }
        catch (Exception) { } // 事件入口兜底
    }

    // ---------------- @ 输入引用（fileReferences/list + sessionReferenceResolver/candidates） ----------------
    //
    // 触发规则：光标前的当前"词"以 @ 开头（词 = 从上一个空白到光标），@ 之后不含空白。
    // 候选来自两个内核端点（都按 agent 作用域，lookup "agent" = 会话 id）：
    //   fileReferences/list                 { agentId, query } → [{path, kind:"file"|"directory"}]
    //   sessionReferenceResolver/candidates { agentId, query }
    //     → [{sessionId, label, cwd, sameWorkspace, createdAt, mention}]
    // mention 是内核给好的引用串 "  @[标签](dsh-session:<base64(JSON 字符串 id)>)"，
    // 选中即整串插入——不自行拼装 dsh-session: 的编码（内核解析器是唯一权威）。
    // 两路查询并行发出；任一路失败不影响另一路（不静默吞错：标题上标注失败原因）。

    private int _referenceGeneration;
    private List<ReferenceVm> _referenceMatches = new();
    private int _referenceIndex = -1;
    private (int Start, int Length) _referenceSpan;
    private bool _referenceBusy;

    /// <summary>光标前是否处在 @ 触发段里；是则给出 @ 的绝对下标。</summary>
    private bool TryGetReferenceTrigger(out int atIndex, out string query)
    {
        atIndex = -1;
        query = "";
        var text = InputBox.Text ?? "";
        var caret = Math.Clamp(InputBox.SelectionStart, 0, text.Length);
        // 往前找 @（遇到空白/换行即停）
        var i = caret - 1;
        while (i >= 0 && !char.IsWhiteSpace(text[i]))
        {
            if (text[i] == '@')
            {
                atIndex = i;
                query = text[(i + 1)..caret];
                return true;
            }
            i--;
        }
        return false;
    }

    /// <summary>输入变化 → 是否显示 @ 候选浮层。</summary>
    private void UpdateReferencePalette()
    {
        try
        {
            if (_rpc is null || Volatile.Read(ref _activeSessionId) is not { Length: > 0 })
            {
                HideReferencePalette();
                return;
            }
            // 命令浮层优先（前导 / 时不同时开两个浮层）
            if (CommandPalette.Visibility == Visibility.Visible)
            {
                HideReferencePalette();
                return;
            }
            if (!TryGetReferenceTrigger(out var at, out var query))
            {
                HideReferencePalette();
                return;
            }
            var generation = ++_referenceGeneration;
            _referenceSpan = (at, query.Length + 1); // 含 @ 的待替换长度
            _ = ShowReferencePaletteAsync(query, generation);
        }
        catch (Exception)
        {
            HideReferencePalette();
        }
    }

    private async Task ShowReferencePaletteAsync(string query, int generation)
    {
        if (_referenceBusy)
        {
            return;
        }
        _referenceBusy = true;
        try
        {
            var sid = Volatile.Read(ref _activeSessionId);
            if (_rpc is null || string.IsNullOrEmpty(sid))
            {
                return;
            }
            var notes = new List<string>();

            // 两路并行：文件引用 + 会话引用
            var filesTask = _rpc.CallOkAsync("fileReferences/list", new { agentId = sid, query });
            var sessionsTask = _rpc.CallOkAsync("sessionReferenceResolver/candidates", new { agentId = sid, query });
            var results = new List<ReferenceVm>();

            try
            {
                var files = await filesTask;
                if (files.ValueKind == JsonValueKind.Array)
                {
                    foreach (var f in files.EnumerateArray())
                    {
                        var path = Str(f, "path");
                        if (path.Length == 0)
                        {
                            continue;
                        }
                        var kind = Str(f, "kind");
                        results.Add(new ReferenceVm
                        {
                            Glyph = kind == "directory" ? "\uE8B7" : "\uE8A5",
                            Label = path,
                            Detail = kind == "directory" ? L("目录") : L("文件"),
                            Insert = path,
                        });
                    }
                }
            }
            catch (Exception ex) when (ex is DshRpcException or InvalidOperationException)
            {
                notes.Add(LF("文件引用不可用（{0}）", ex.Message));
            }

            try
            {
                var sessions = await sessionsTask;
                if (sessions.ValueKind == JsonValueKind.Array)
                {
                    foreach (var s in sessions.EnumerateArray())
                    {
                        var mention = Str(s, "mention");
                        if (mention.Length == 0)
                        {
                            continue;
                        }
                        var label = Str(s, "label");
                        var same = s.TryGetProperty("sameWorkspace", out var sw) && sw.ValueKind == JsonValueKind.True;
                        results.Add(new ReferenceVm
                        {
                            Glyph = "\uE8BD",
                            Label = label.Length > 0 ? label : Str(s, "sessionId"),
                            Detail = same ? L("会话 · 同工作区") : L("会话"),
                            Insert = mention,
                            IsSession = true,
                        });
                    }
                }
            }
            catch (Exception ex) when (ex is DshRpcException or InvalidOperationException)
            {
                notes.Add(LF("会话引用不可用（{0}）", ex.Message));
            }

            if (generation != _referenceGeneration)
            {
                return; // 已有更新的按键
            }
            if (results.Count == 0)
            {
                ReferenceList.ItemsSource = null;
                ReferenceList.Visibility = Visibility.Collapsed;
                _referenceMatches = new List<ReferenceVm>();
                _referenceIndex = -1;
                if (notes.Count > 0)
                {
                    // 两路都失败：把原因摊在浮层上（不静默收起——用户需要知道为什么没有候选）。
                    // 实测原因之一：内核 gateway/internal + SessionAlreadyOwnedError
                    //（该会话仍被内核的活跃写句柄持有，resume 被拒；与壳无关，换会话即可）。
                    ReferencePaletteHeader.Text = LF("引用不可用：{0}", string.Join("；", notes));
                    ShowOverlay(ReferencePalette);
                }
                else
                {
                    HideReferencePalette();
                }
                return;
            }
            _referenceMatches = results;
            _referenceIndex = 0;
            ReferenceList.Visibility = Visibility.Visible;
            ReferenceList.ItemsSource = results;
            ReferenceList.SelectedIndex = 0;
            ReferencePaletteHeader.Text = query.Length == 0
                ? LF("引用 {0} 条（@ 后输入可过滤；↑↓ 选择，Enter 插入，Esc 关闭）", results.Count)
                    + (notes.Count > 0 ? $" · {string.Join("；", notes)}" : "")
                : LF("“@{0}” 匹配 {1} 条（↑↓ 选择，Enter 插入，Esc 关闭）", query, results.Count);
            ShowOverlay(ReferencePalette);
        }
        catch (Exception ex)
        {
            HideReferencePalette();
        }
        finally
        {
            _referenceBusy = false;
        }
    }

    private void HideReferencePalette()
    {
        _referenceGeneration++;
        _referenceMatches = new List<ReferenceVm>();
        _referenceIndex = -1;
        ReferencePalette.Visibility = Visibility.Collapsed;
        ReferenceList.ItemsSource = null;
    }

    /// <summary>浮层打开时的按键（与命令浮层同构；返回 true 表示已消费）。</summary>
    private bool HandleReferenceKey(KeyRoutedEventArgs e)
    {
        switch (e.Key)
        {
            case Windows.System.VirtualKey.Escape:
                e.Handled = true;
                HideReferencePalette();
                return true;
            case Windows.System.VirtualKey.Up:
                e.Handled = true;
                MoveReferenceSelection(-1);
                return true;
            case Windows.System.VirtualKey.Down:
                e.Handled = true;
                MoveReferenceSelection(1);
                return true;
            case Windows.System.VirtualKey.Tab:
                e.Handled = true;
                AcceptReferenceSelection();
                return true;
            case Windows.System.VirtualKey.Enter:
            {
                var shift = Microsoft.UI.Input.InputKeyboardSource.GetKeyStateForCurrentThread(Windows.System.VirtualKey.Shift);
                if (shift.HasFlag(Windows.UI.Core.CoreVirtualKeyStates.Down))
                {
                    return false; // Shift+Enter 仍是换行
                }
                e.Handled = true;
                AcceptReferenceSelection();
                return true;
            }
            default:
                return false;
        }
    }

    private void MoveReferenceSelection(int delta)
    {
        if (_referenceMatches.Count == 0)
        {
            return;
        }
        _referenceIndex = Math.Clamp(_referenceIndex + delta, 0, _referenceMatches.Count - 1);
        ReferenceList.SelectedIndex = _referenceIndex;
        ReferenceList.ScrollIntoView(_referenceMatches[_referenceIndex]);
    }

    /// <summary>Enter/Tab 选中：把 @query 整段替换成候选的 Insert 文本，光标落到插入内容之后。</summary>
    private void AcceptReferenceSelection()
    {
        if (_referenceIndex < 0 || _referenceIndex >= _referenceMatches.Count)
        {
            HideReferencePalette();
            return;
        }
        var pick = _referenceMatches[_referenceIndex];
        var text = InputBox.Text ?? "";
        var (start, length) = _referenceSpan;
        if (start < 0 || start + length > text.Length)
        {
            HideReferencePalette();
            return;
        }
        // 引用后缀补一个空格：紧接着继续输入时不会粘成同一段引用
        var inserted = pick.Insert + " ";
        InputBox.Text = text[..start] + inserted + text[(start + length)..];
        InputBox.SelectionStart = start + inserted.Length;
        HideReferencePalette();
    }

    private void OnReferenceItemClick(object sender, ItemClickEventArgs e)
    {
        try
        {
            if (e.ClickedItem is not ReferenceVm pick)
            {
                return;
            }
            _referenceIndex = _referenceMatches.IndexOf(pick);
            AcceptReferenceSelection();
            InputBox.Focus(FocusState.Programmatic); // 鼠标点选后焦点还给输入框
        }
        catch (Exception) { }
    }

    /// <summary>输入框变化 → 是否显示补全浮层（只认前导 "/"，进入参数段即收起）。</summary>
    private void UpdateCommandPalette()
    {
        var text = InputBox.Text ?? "";
        if (text.Length == 0 || text[0] != '/' || text.IndexOfAny([' ', '\t', '\r', '\n']) >= 0)
        {
            HideCommandPalette();
            return;
        }
        var generation = ++_paletteGeneration;
        _ = ShowCommandPaletteAsync(text[1..], generation);
    }

    private async Task ShowCommandPaletteAsync(string query, int generation)
    {
        try
        {
            await EnsureCommandsAsync();
            if (generation != _paletteGeneration)
            {
                return; // 已有更新的按键
            }
            if (_commandsError is not null)
            {
                // 目录取不到不静默：把原因摊在浮层标题上（agent 未就绪等），列表区收起
                _commandMatches = new List<CommandVm>();
                _commandIndex = -1;
                CommandList.ItemsSource = null;
                CommandList.Visibility = Visibility.Collapsed;
                CommandPaletteHeader.Text = LF("命令目录不可用：{0}", _commandsError);
                ShowOverlay(CommandPalette);
                return;
            }
            var matches = _commands
                .Where(c => c.Name.Contains(query, StringComparison.OrdinalIgnoreCase))
                .OrderByDescending(c => c.Name.StartsWith(query, StringComparison.OrdinalIgnoreCase))
                .ThenBy(c => c.Name, StringComparer.OrdinalIgnoreCase)
                .Take(20)
                .ToList();
            if (matches.Count == 0)
            {
                HideCommandPalette();
                return;
            }
            _commandMatches = matches;
            _commandIndex = 0;
            CommandList.Visibility = Visibility.Visible;
            CommandList.ItemsSource = matches;
            CommandList.SelectedIndex = 0;
            CommandPaletteHeader.Text = query.Length == 0
                ? "命令（↑↓ 选择，Enter 执行，Esc 关闭）"
                : LF("“/{0}” 匹配 {1} 条（↑↓ 选择，Enter 执行，Esc 关闭）", query, matches.Count);
            ShowOverlay(CommandPalette);
        }
        catch (Exception)
        {
            HideCommandPalette();
        }
    }

    private void HideCommandPalette()
    {
        _paletteGeneration++;
        _commandMatches = new List<CommandVm>();
        _commandIndex = -1;
        CommandPalette.Visibility = Visibility.Collapsed;
        CommandList.ItemsSource = null;
        // 命令浮层让位时把 @ 引用浮层也收掉：两者互斥，同时开着会叠在同一位置
        HideReferencePalette();
    }

    /// <summary>浮层打开时的按键（焦点始终留在输入框；返回 true 表示已消费）。</summary>
    private bool HandlePaletteKey(KeyRoutedEventArgs e)
    {
        switch (e.Key)
        {
            case Windows.System.VirtualKey.Escape:
                e.Handled = true;
                HideCommandPalette();
                return true;
            case Windows.System.VirtualKey.Up:
                e.Handled = true;
                MoveCommandSelection(-1);
                return true;
            case Windows.System.VirtualKey.Down:
                e.Handled = true;
                MoveCommandSelection(1);
                return true;
            case Windows.System.VirtualKey.Tab:
                e.Handled = true;
                AcceptCommandSelection();
                return true;
            case Windows.System.VirtualKey.Enter:
            {
                var shift = Microsoft.UI.Input.InputKeyboardSource.GetKeyStateForCurrentThread(Windows.System.VirtualKey.Shift);
                if (shift.HasFlag(Windows.UI.Core.CoreVirtualKeyStates.Down))
                {
                    return false; // Shift+Enter 仍是换行
                }
                e.Handled = true;
                AcceptCommandSelection();
                return true;
            }
            default:
                return false;
        }
    }

    private void MoveCommandSelection(int delta)
    {
        if (_commandMatches.Count == 0)
        {
            return;
        }
        _commandIndex = Math.Clamp(_commandIndex + delta, 0, _commandMatches.Count - 1);
        CommandList.SelectedIndex = _commandIndex;
        CommandList.ScrollIntoView(_commandMatches[_commandIndex]);
    }

    /// <summary>Enter/Tab 选中：需要参数的命令只补全命令名（原版 leading claim 语义），否则直接执行。</summary>
    private void AcceptCommandSelection()
    {
        if (_commandIndex < 0 || _commandIndex >= _commandMatches.Count)
        {
            HideCommandPalette();
            return;
        }
        var command = _commandMatches[_commandIndex];
        if (command.HasInput)
        {
            InputBox.Text = command.SlashName + " ";
            InputBox.SelectionStart = InputBox.Text.Length;
            HideCommandPalette();
            return;
        }
        InputBox.Text = "";
        HideCommandPalette();
        _ = ExecuteCommandAsync(command.SlashName);
    }

    private void OnCommandItemClick(object sender, ItemClickEventArgs e)
    {
        try
        {
            if (e.ClickedItem is not CommandVm command)
            {
                return;
            }
            _commandIndex = _commandMatches.IndexOf(command);
            AcceptCommandSelection();
            InputBox.Focus(FocusState.Programmatic); // 鼠标点选后焦点还给输入框
        }
        catch (Exception) { }
    }

    /// <summary>
    /// commands/execute：args { agentId, line, submittedAttachments }（线格式以内核描述符为准）。
    /// 返回值可能是 undefined（未识别/格式错误的行）——内核 JSON 里直接没有 value 字段；
    /// 成功/失败以 result.kind（success|error）与 result.text 呈现为系统消息。
    /// fromComposer：命令由输入框 Enter 提交时才有。界面控件（权限胶囊、计划 chip、
    /// 命令目录）发起的命令传 false——输入框里的草稿与待发附件是用户正在编辑的内容，
    /// 命令只做命令，不把它们一起带走。
    /// </summary>
    private bool _commandSubmitting;

    private async Task ExecuteCommandAsync(string line, bool fromComposer = false)
    {
        if (_commandSubmitting || _rpc is null)
        {
            return;
        }
        var sid = Volatile.Read(ref _activeSessionId);
        if (string.IsNullOrEmpty(sid))
        {
            AppendSystemMessage(L("请先选择一个会话：命令在会话（agent）作用域内执行。"));
            return;
        }
        _commandSubmitting = true;
        var submittedText = fromComposer ? InputBox.Text : null;
        var attachments = fromComposer ? _pendingAttachments.ToList() : new List<AttachmentVm>();
        AppendSystemMessage($"> {line}");
        try
        {
            var submittedAttachments = new List<object>();
            foreach (var att in attachments)
            {
                if (att.IsImage)
                {
                    submittedAttachments.Add(new { type = "image", mediaType = att.MediaType, data = att.DataBase64, name = att.Name });
                }
                else
                {
                    var receipt = await UploadFileAsync(sid, att);
                    if (string.IsNullOrEmpty(receipt))
                    {
                        throw new InvalidOperationException(L("附件上传失败，命令未提交；请重试。"));
                    }
                    submittedAttachments.Add(new { type = "file", receiptId = receipt });
                }
            }
            var envelope = await _rpc.CallAsync("commands/execute", new
            {
                agentId = sid,
                line,
                submittedAttachments,
            });
            if (!envelope.TryGetProperty("ok", out var okFlag) || okFlag.ValueKind != JsonValueKind.True)
            {
                var error = envelope.TryGetProperty("error", out var err) && err.ValueKind == JsonValueKind.Object
                    ? err.TryGetProperty("message", out var m) ? m.GetString() : null
                    : null;
                AppendSystemMessage(LF("命令失败：{0}", error ?? L("内核拒绝")));
                return;
            }
            if (!envelope.TryGetProperty("value", out var value) || value.ValueKind != JsonValueKind.Object)
            {
                AppendSystemMessage(LF("未知或格式不正确的命令：{0}", line));
                return;
            }
            if (!value.TryGetProperty("result", out var result) || result.ValueKind != JsonValueKind.Object)
            {
                AppendSystemMessage(LF("{0} 已提交（内核未返回即时应答）", line));
                return;
            }
            var kind = result.TryGetProperty("kind", out var k) ? k.GetString() : null;
            var text = result.TryGetProperty("text", out var t) && t.ValueKind == JsonValueKind.String ? t.GetString() : null;
            AppendSystemMessage(kind switch
            {
                "success" => string.IsNullOrEmpty(text) ? LF("{0} 执行完成", line) : text!,
                "error" => text is null ? LF("{0} 执行失败", line) : $"⚠ {text}",
                _ => $"{line} → {kind}",
            });
            if (kind == "success" && sid == _activeSessionId && fromComposer)
            {
                if (InputBox.Text == submittedText)
                {
                    InputBox.Text = "";
                }
                foreach (var att in attachments)
                {
                    _pendingAttachments.Remove(att);
                }
                foreach (var chip in AttachmentList.Children.OfType<StackPanel>().ToList())
                {
                    if (chip.Children.OfType<Button>().Any(b => b.Tag is AttachmentVm att && attachments.Contains(att)))
                    {
                        AttachmentList.Children.Remove(chip);
                    }
                }
                AttachmentStrip.Visibility = _pendingAttachments.Count == 0 ? Visibility.Collapsed : Visibility.Visible;
            }
        }
        catch (Exception ex)
        {
            AppendSystemMessage(LF("命令失败：{0}", ex.Message));
        }
        finally
        {
            _commandSubmitting = false;
        }
    }

    /// <summary>系统消息（命令回执等）走 Role=system 的小字气泡。任意线程可调。</summary>
    private void AppendSystemMessage(string text)
    {
        var bubble = new ChatBubble { Role = "system", Text = text };
        if (DispatcherQueue.HasThreadAccess)
        {
            AppendBubble(bubble);
        }
        else
        {
            PostUi(() => AppendBubble(bubble));
        }
    }

    /// <summary>UI 队列投递统一入口：回调自带 try/catch。WinUI3 的 DispatcherQueue 回调异常
    /// 绕过一切托管全局处理器直接 stow 终止进程（0xc000027b，microsoft-ui-xaml #8940），
    /// 唯一出口就是回调内部自吞；异常留档后界面继续可用。</summary>
    private void PostUi(Action action) => DispatcherQueue.TryEnqueue(() =>
    {
        try
        {
            action();
        }
        catch (Exception ex)
        {
            LogUiFault(ex);
        }
    });

    private static void LogUiFault(Exception ex)
    {
        try
        {
            File.AppendAllText(
                Path.Combine(Path.GetTempPath(), "blade2_unhandled.txt"),
                $"\n=== {DateTime.Now:MM-dd HH:mm:ss} [ui-post] ===\n{ex}\n---STACK---\n{ex.StackTrace}");
        }
        catch { }
    }

    // ---------------- 材质（Mica / Mica Alt / 亚克力 / 无） ----------------

    /// <summary>窗口材质偏好（壳本地 shell.json 的 material 字段；合法值见 ShellOptions 上方的常量）。
    /// 由 LoadShellOptions 在启动时读入，个性化分区的下拉切换后经 SetShellMaterial 更新。</summary>
    private string _shellMaterial = MaterialMica;

    /// <summary>气泡材质偏好（壳本地 shell.json 的 bubbleMaterial 字段；合法值同上处的常量）。
    /// translucent = 半透明实色面（默认）；acrylic = 系统 acrylic；follow = 跟随窗口材质。
    /// 由 LoadShellOptions 读入，个性化分区的气泡材质下拉经 SetBubbleMaterial 更新。</summary>
    private string _bubbleMaterial = BubbleMaterialTranslucent;

    /// <summary>气泡不透明度（shell.json 的 bubbleOpacity 字段，0.2–1.0）。
    /// 半透明档直接当 alpha，亚克力/跟随档当 TintOpacity，语义统一：气泡自身有多不透。</summary>
    private double _bubbleOpacity = BubbleOpacityDefault;

    /// <summary>根网格本地资源里覆盖 Tokens.xaml 的那支气泡笔刷（半透明档与亚克力档各自的实例）。
    /// 半透明档改不透明度只改这支的 Color，已渲染气泡共用实例、无需重建列表；
    /// 换材质档才换实例（类型都变了），换完必须重建容器才会解析到新笔刷。</summary>
    private Brush? _bubbleBrush;
    private SolidColorBrush? _bubbleSolid;
    private AcrylicBrush? _bubbleAcrylic;
    private const string BubbleBrushKey = "BubbleMicaBrush";

    /// <summary>按当前材质偏好应用窗口级 SystemBackdrop：侧栏/顶栏透出系统材质。
    /// 内容区有不透明皮肤时由 ContentSurface 盖住，材质仍作用于侧栏与顶栏。
    /// 每次调用都挂全新实例（先断开再连接）：换肤/拉伸后材质不会自动按新主题重配，
    /// 必须重建——Acrylic 胶囊与拉伸后丢失都走这条路径。</summary>
    private void ApplyBackdrop()
    {
        SystemBackdrop? backdrop;
        try
        {
            backdrop = BuildBackdrop(_shellMaterial);
        }
        catch (Exception)
        {
            // 材质构造失败（旧系统上 Mica 系不可用）：回退亚克力，再失败就留空
            try
            {
                backdrop = new DesktopAcrylicBackdrop();
            }
            catch (Exception)
            {
                backdrop = null;
            }
        }
        SystemBackdrop = null;
        SystemBackdrop = backdrop;
        // 「无材质」时窗口没有透出层：给根网格一个不透明底，否则侧栏/顶栏的半透明层会透出桌面
        RootGrid.Background = _shellMaterial == MaterialNone ? ThemeBrush("SurfaceBrush") : null;
    }

    /// <summary>材质 id → SystemBackdrop 实例。none = 不加材质（纯色，见 ApplyBackdrop 的根网格兜底）。</summary>
    private static SystemBackdrop? BuildBackdrop(string material) => material switch
    {
        MaterialMicaAlt => new MicaBackdrop
        {
            Kind = Microsoft.UI.Composition.SystemBackdrops.MicaKind.BaseAlt,
        },
        MaterialAcrylic => new DesktopAcrylicBackdrop(),
        MaterialNone => null,
        _ => new MicaBackdrop(),
    };

    /// <summary>切换窗口材质：即时应用 + 写回壳本地 shell.json（个性化分区是壳内建分区，
    /// 不进内核 settings）。未知 id 按 mica 处理，重复选择不重写盘。</summary>
    private void SetShellMaterial(string material)
    {
        var id = material is MaterialMica or MaterialMicaAlt or MaterialAcrylic or MaterialNone
            ? material
            : MaterialMica;
        if (id == _shellMaterial)
        {
            return;
        }
        _shellMaterial = id;
        _shellOptions.Material = id;
        SaveShellOptions();
        ApplyBackdrop();
        ApplyBubbleMaterial();
    }

    /// <summary>切换气泡材质：即时应用 + 写回 shell.json。未知 id 按半透明处理，重复选择不重写盘。</summary>
    private void SetBubbleMaterial(string material)
    {
        var id = material is BubbleMaterialTranslucent or BubbleMaterialAcrylic or BubbleMaterialFollow
            ? material
            : BubbleMaterialTranslucent;
        if (id == _bubbleMaterial)
        {
            return;
        }
        _bubbleMaterial = id;
        _shellOptions.BubbleMaterial = id;
        SaveShellOptions();
        ApplyBubbleMaterial();
    }

    /// <summary>调节气泡不透明度（0.2–1.0）：即时应用 + 写回 shell.json。滑块以步进走动，
    /// 每次 ValueChanged 落一次盘（几百字节），不做防抖也够轻。</summary>
    private void SetBubbleOpacity(double opacity)
    {
        var value = Math.Clamp(opacity, BubbleOpacityMin, BubbleOpacityMax);
        if (Math.Abs(value - _bubbleOpacity) < 0.0001)
        {
            return;
        }
        _bubbleOpacity = value;
        _shellOptions.BubbleOpacity = value;
        SaveShellOptions();
        ApplyBubbleMaterial();
    }

    /// <summary>气泡外观：按「设置→气泡材质 + 气泡不透明度 + 当前深浅主题」算出气泡背景笔刷，
    /// 写进根网格的本地资源（覆盖 Tokens.xaml 的同名基线），窗口里所有气泡取同一支。
    /// 半透明档 = 当前主题面色 + 不透明度当 alpha，背后是视频/壁纸就直接透出画面；
    /// 亚克力档 = WinUI 3 自带的 AcrylicBrush（TintOpacity 取不透明度、取样亮度拉高出玻璃感）；
    /// 跟随档按窗口材质定配方：mica/mica-alt 走平面着色、acrylic 走玻璃、none 走纯色卡。
    /// 生效方式：笔刷实例由气泡模板的 Loaded（OnBubbleBorderLoaded）直赋到 Border.Background，
    /// 不依赖字典查找——DataTemplate 内容的 ThemeResource 实际解析到 App 级 ThemeDictionaries，
    /// 根网格本地覆盖对模板内容够不着（这是 0.7.9 之前换材质没反应的根因）。
    /// 换不透明度不改实例身份（只改实例的 Color），已渲染气泡共用同一实例、立刻全量生效；
    /// 换材质档才换实例，随后 RebindChatList 重建容器，新容器在 Loaded 逐个拿到新实例。
    /// 高对比度清空实例并重建：气泡回落 Tokens.xaml 高对比字典里的纯色，半透明会吃掉文字对比。</summary>
    private void ApplyBubbleMaterial()
    {
        try
        {
            if (IsHighContrast())
            {
                if (_bubbleBrush is not null)
                {
                    _bubbleBrush = null;
                    _bubbleSolid = null;
                    _bubbleAcrylic = null;
                    if (RootGrid.Resources.ContainsKey(BubbleBrushKey))
                    {
                        RootGrid.Resources.Remove(BubbleBrushKey);
                    }
                    RebindChatList();
                }
                return;
            }
            var next = BuildBubbleBrush();
            if (next is null)
            {
                return;
            }
            if (!ReferenceEquals(next, _bubbleBrush))
            {
                // 换实例（切材质档）：写根网格本地资源 + 重建列表，新容器在 OnBubbleBorderLoaded 逐个拿到新实例
                _bubbleBrush = next;
                RootGrid.Resources[BubbleBrushKey] = next;
                RebindChatList();
            }
            // 同实例（拖不透明度滑块）：BuildBubbleBrush 已就地改实例属性，
            // 所有气泡共用该实例，框架自动重绘，无需重建列表
        }
        catch (Exception) { } // 配方失败不致命：气泡退回 Tokens.xaml 的静态基线
    }

    /// <summary>按当前偏好造出气泡笔刷。两支实例（实色 / acrylic）按档位复用：
    /// 改不透明度时返回的还是同一支，ApplyBubbleMaterial 因此不会触发列表重建。</summary>
    private Brush? BuildBubbleBrush()
    {
        var surfaceColor = ThemeBrush("SurfaceBrush") is SolidColorBrush surface ? surface.Color : default;
        // 「跟随窗口材质」时 none 走纯色卡，其余按窗口材质取配方；「亚克力」固定走 acrylic 配方
        var material = _bubbleMaterial == BubbleMaterialFollow ? _shellMaterial : MaterialAcrylic;
        if (_bubbleMaterial == BubbleMaterialTranslucent
            || (_bubbleMaterial == BubbleMaterialFollow && material == MaterialNone))
        {
            // 跟随即无材质：纯色卡（与去材质化的窗口一致），不透明度滑块不参与
            var alpha = _bubbleMaterial == BubbleMaterialTranslucent ? _bubbleOpacity : 1.0;
            _bubbleSolid ??= new SolidColorBrush();
            _bubbleSolid.Color = Windows.UI.Color.FromArgb(
                (byte)Math.Round(Math.Clamp(alpha, 0, 1) * 255), surfaceColor.R, surfaceColor.G, surfaceColor.B);
            return _bubbleSolid;
        }
        _bubbleAcrylic ??= new AcrylicBrush();
        _bubbleAcrylic.TintColor = material == MaterialMicaAlt ? CoolShift(surfaceColor) : surfaceColor;
        _bubbleAcrylic.TintOpacity = Math.Clamp(_bubbleOpacity, 0, 1);
        // mica/mica-alt 本体是平面着色层，几乎不透出取样亮度；acrylic 靠取样亮度透出背后画面成玻璃
        _bubbleAcrylic.TintLuminosityOpacity = material == MaterialAcrylic ? 0.9 : 0.12;
        _bubbleAcrylic.AlwaysUseFallback = false;
        return _bubbleAcrylic;
    }

    /// <summary>Mica-alt 的冷峻倾向：同亮度下红降 3%、绿降 1.5%、蓝不动
    /// （#F3F3F3 → #EEF2F6，#202020 → #1F2022：两个主题方向一致，幅度可辨但不刺眼）。
    /// 浅色主题下 mica 与 mica-alt 的底色本来就接近，不冷移的话选 alt 看不出变化。</summary>
    private static Windows.UI.Color CoolShift(Windows.UI.Color c)
        => Windows.UI.Color.FromArgb(c.A, Scale(c.R, 0.97), Scale(c.G, 0.985), c.B);

    private static byte Scale(byte value, double factor)
        => (byte)Math.Clamp((int)Math.Round(value * factor), 0, 255);

    /// <summary>重建聊天列表容器：气泡 Background 是模板里的 {ThemeResource}，材质档位换了笔刷
    /// 实例后，已生成的容器仍持有旧实例，重新挂一次 ItemsSource 才会解析到根网格的新笔刷。
    /// SelectionMode=None、无选中态，重建不丢状态；只改不透明度走同实例直改，不进这里。</summary>
    private void RebindChatList()
    {
        if (ChatList is null)
        {
            return;
        }
        var source = ChatList.ItemsSource;
        ChatList.ItemsSource = null;
        ChatList.ItemsSource = source;
    }

    /// <summary>
    /// 应用壳主题：light/dark 直接生效；system 跟随系统（UISettings 获取实况）。
    /// system 偏好的实时性由 MainWindow.ShellIntegration 的 WM_SETTINGCHANGE(ImmersiveColorSet)
    /// 钩子保证——Windows 切换深浅色的瞬间即触发本方法重应用（0.7.6.x 及之前要重启才生效）。
    /// XAML RequestedTheme 换肤 + caption 按钮深浅配色 + 材质重建（Acrylic 胶囊在
    /// RequestedTheme 变更后不会自动按新主题重配，必须重赋）。
    /// </summary>
    private void ApplyShellTheme(string preference)
    {
        try
        {
            _shellPreference = preference is "light" or "dark" or "system" ? preference : "system";
            var dark = _shellPreference switch
            {
                "dark" => true,
                "light" => false,
                _ => IsSystemDark(),
            };
            if (dark == _shellDark && Content is FrameworkElement { RequestedTheme: not ElementTheme.Default })
            {
                return; // 实际深浅没变：不重复重建材质
            }
            _shellDark = dark;
            if (Content is FrameworkElement root)
            {
                root.RequestedTheme = dark ? ElementTheme.Dark : ElementTheme.Light;
            }
            // 宠物在独立窗口里：它不继承主窗口的 RequestedTheme，得显式同步
            _petWindow?.SetTheme(dark);
            ApplyCaptionButtonTheme();
            ApplyBackdrop();
            // 气泡笔刷同样要按新主题重算（半透明档的面色取自主题）：探针的 {ThemeResource} 重估
            // 随框架换肤 walk 进行，先冲刷一次再读，拿到的才是新主题的面色（同设置页重渲染的套路）。
            (Content as FrameworkElement)?.UpdateLayout();
            ApplyBubbleMaterial();
            // 设置页 brush 是启动时取的资源快照：换主题后重渲染当前分区才有正确的深浅资源。
            // 探针的 {ThemeResource} 重估随框架的换肤 walk 进行，UpdateLayout 冲刷后再读才是新值。
            if (SettingsPage.Visibility == Visibility.Visible)
            {
                (Content as FrameworkElement)?.UpdateLayout();
                // keepPendingEdits：重渲染只为换肤，用户尚未保存的改动必须原样保留
                _ = RenderSectionAsync(_settingsActiveSection, keepPendingEdits: true);
            }
        }
        catch (Exception) { } // 换肤失败不致命：壳保持原主题
    }

    private static bool IsSystemDark()
    {
        try
        {
            var ui = new Windows.UI.ViewManagement.UISettings();
            var fg = ui.GetColorValue(Windows.UI.ViewManagement.UIColorType.Foreground);
            // 深色系统 = 白色前景（R 高）；浅色系统 = 黑色前景（R 低）。
            // 0.6.6 前的判断 fg.R < 128 极性反了：浅色系统被误判成深色。
            return fg.R > 128;
        }
        catch (Exception)
        {
            return true;
        }
    }

    /// <summary>延伸标题栏 caption 按钮配色：全部取自语义令牌（浅/深/高对比度三态随令牌自动跟随，
    /// caption 钮由系统绘制，只能交色值，所以这里把令牌画刷的颜色取出来给它）。
    /// 换肤后必须重调（系统不会跟着 RequestedTheme 变）。</summary>
    private void ApplyCaptionButtonTheme()
    {
        try
        {
            var tb = AppWindow.TitleBar;
            if (tb == null)
            {
                return;
            }
            tb.ButtonBackgroundColor = Colors.Transparent;          // 无底色：标题栏与内容面同底
            tb.ButtonInactiveBackgroundColor = Colors.Transparent;
            tb.ButtonForegroundColor = ThemeColor("TextPrimaryBrush");
            tb.ButtonInactiveForegroundColor = ThemeColor("TextDisabledBrush");
            tb.ButtonHoverBackgroundColor = ThemeColor("ControlHoverBrush");
            tb.ButtonHoverForegroundColor = ThemeColor("TextPrimaryBrush");
            tb.ButtonPressedBackgroundColor = ThemeColor("ControlPressedBrush");
        }
        catch (Exception) { }
    }

    /// <summary>语义令牌画刷的颜色值（caption 按钮这类只收 Color 的系统 API 用）。</summary>
    private Windows.UI.Color ThemeColor(string key)
        => ThemeBrush(key) is SolidColorBrush brush ? brush.Color : Colors.Transparent;

    /// <summary>
    /// 默认窗口尺寸。AppWindow.Resize 收物理像素（WinUI 3 interop 文档的 DPI 换算法）：
    /// 逻辑 1280×800 × GetDpiForWindow 缩放比，并夹紧到所在显示器工作区（DisplayArea.WorkArea）。
    ///
    /// 同时设置窗口宽度下限：本窗无最小尺寸时可以被拉到 OS 允许的极限（本机 131dip），
    /// 此时侧栏导轨 + 页边距吃满宽度，设置页内容列只剩 ~52dip —— 实测文字被压成一字一行、
    /// 与相邻行重叠并被卡片裁掉（artifacts/screenshots/usage-narrow-min220dip.png）。
    /// 下限取令牌 WindowMinWidth（560 dip = 内容列可读下限 420 + 侧栏导轨 65 + 页边距 36×2）。
    /// </summary>
    private void ResizeToWorkableDefault()
    {
        if (AppWindow is null)
        {
            return;
        }
        const int logicalWidth = 1280, logicalHeight = 800;
        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
        var scale = GetDpiForWindow(hwnd) / 96.0;
        ApplyMinimumWindowWidth(scale);
        var width = (int)Math.Round(logicalWidth * scale);
        var height = (int)Math.Round(logicalHeight * scale);
        if (DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Nearest)?.WorkArea is { } work)
        {
            width = Math.Min(width, work.Width);
            height = Math.Min(height, work.Height);
        }
        AppWindow.Resize(new SizeInt32(width, height));
    }

    /// <summary>窗口尺寸下限（dip，令牌 WindowMinWidth/WindowMinHeight）：本窗原无最小尺寸，
    /// 可被拉到 OS 允许的极限；此时侧栏导轨 + 页边距把设置内容列压到 ~52dip
    /// （实测 220dip 窗口下文字一字一行且相互重叠，见 artifacts/verify-usage-narrow.log）。
    ///
    /// 实现走 Win32 的 WM_GETMINMAXINFO：WindowsAppSDK 1.6 的 OverlappedPresenter 还没有
    /// PreferredMinimumWidth/Height（1.7 才加），所以挂一个窗口子类化回调改写
    /// MINMAXINFO.ptMinTrackSize（该结构按物理像素计，故乘 DPI 缩放比）。</summary>
    private void ApplyMinimumWindowWidth(double scale)
    {
        try
        {
            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            if (hwnd == IntPtr.Zero || _minSizeSubclass is not null)
            {
                return;
            }
            _minTrackWidth = (int)Math.Round(TokenDouble("WindowMinWidth", 560) * scale);
            _minTrackHeight = (int)Math.Round(TokenDouble("WindowMinHeight", 420) * scale);
            // 委托要保活：子类化回调是原生调用的入口，被 GC 回收会直接崩
            _minSizeSubclass = OnMinSizeMessage;
            SetWindowSubclass(hwnd, _minSizeSubclass, (UIntPtr)1, IntPtr.Zero);
        }
        catch (Exception)
        {
            // 子类化失败：退化为无下限（原行为），不影响启动
        }
    }

    private MinSizeSubclass? _minSizeSubclass;
    private int _minTrackWidth;
    private int _minTrackHeight;

    private const uint WM_GETMINMAXINFO = 0x0024;

    private delegate IntPtr MinSizeSubclass(IntPtr hWnd, uint uMsg, IntPtr wParam, IntPtr lParam, UIntPtr uIdSubclass, IntPtr dwRefData);

    [DllImport("comctl32.dll", SetLastError = true)]
    private static extern bool SetWindowSubclass(IntPtr hWnd, MinSizeSubclass pfnSubclass, UIntPtr uIdSubclass, IntPtr dwRefData);

    // DefSubclassProc 的 P/Invoke 声明在 MainWindow.ShellIntegration.cs（壳子类化同用一个入口），
    // 这里不再重复声明：同属 MainWindow partial 类，private 成员互通。

    [StructLayout(LayoutKind.Sequential)]
    private struct WinPoint
    {
        public int X;
        public int Y;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct MinMaxInfo
    {
        public WinPoint Reserved;
        public WinPoint MaxSize;
        public WinPoint MaxPosition;
        public WinPoint MinTrackSize;
        public WinPoint MaxTrackSize;
    }

    private IntPtr OnMinSizeMessage(IntPtr hWnd, uint uMsg, IntPtr wParam, IntPtr lParam, UIntPtr uIdSubclass, IntPtr dwRefData)
    {
        if (uMsg == WM_GETMINMAXINFO && lParam != IntPtr.Zero && _minTrackWidth > 0)
        {
            var info = Marshal.PtrToStructure<MinMaxInfo>(lParam);
            info.MinTrackSize.X = Math.Max(info.MinTrackSize.X, _minTrackWidth);
            info.MinTrackSize.Y = Math.Max(info.MinTrackSize.Y, _minTrackHeight);
            Marshal.StructureToPtr(info, lParam, false);
            return IntPtr.Zero;
        }
        return DefSubclassProc(hWnd, uMsg, wParam, lParam);
    }

    [DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(IntPtr hwnd);

    private void Shutdown()
    {
        try
        {
            // 先取消后台引导：内核可能在半路上，收掉 CTS 让引导链路即刻返回
            CancelKernelBoot();
            ShellToast.Unregister();     // 先注销通知 COM 服务，再摘托盘退出
            CleanupShellIntegration(); // 先摘托盘再退出，避免残留死图标
            StopSkinVideo();            // 视频皮肤播放器占着媒体管线，退出前释放
            StopPetRuntime();           // 关独立宠物窗口，否则壳进程赖在后台
            _rpc?.DisposeAsync().AsTask().Wait(TimeSpan.FromSeconds(3));
            _kernel.Dispose();
        }
        catch (Exception) { }
    }
}
