using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Net.WebSockets;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

namespace Blade2.Dsh;

/// <summary>
/// dsh 内核 RPC 客户端：HTTP JSON RPC（POST /api/&lt;endpoint&gt;）+ mux WebSocket
/// （/api/remote.mux）+ 事件订阅（$events 流）+ 审批回传（$events/result）。
/// 协议契约见 dsh-api-gateway / dsh-api-session-controller 的 typert.remote-client.js。
/// </summary>
public sealed class DshRpcClient : IAsyncDisposable
{
    private readonly HttpClient _http;
    private readonly CookieContainer _cookies = new();
    private readonly Uri _base;          // http://127.0.0.1:<port>/
    private ClientWebSocket? _mux;
    private int _rpcId;
    private int _streamSeq;
    /// <summary>mux 流 id → 回调。键是字符串：内核 mux 协议的 streamId 必须是字符串，
    /// 数字 id 的 open 帧被服务端静默忽略（开流永不产生帧——0.7.0 探针定论）。</summary>
    private readonly System.Collections.Concurrent.ConcurrentDictionary<string, Action<JsonElement>> _streamSinks = new();
    private string? _eventsStreamId;
    private readonly SemaphoreSlim _connectionGate = new(1, 1);
    private readonly SemaphoreSlim _sendGate = new(1, 1);
    private readonly CancellationTokenSource _lifetime = new();
    public event Action<string>? EventCancelled;
    /// <summary>流终态通知（服务端 end/error 帧）。error 为终态错误描述（end 帧为 null）。
    /// 业务错误是永久性终态：客户端不自动重试，调用方据此决定是否重开业务流；
    /// 连接级断开不走此事件，走 <see cref="StreamsReset"/>。</summary>
    public event Action<string, string?>? StreamEnded;
    private readonly Dictionary<string, List<Func<JsonElement, Task>>> _eventHandlers = new();
    /// <summary>waterfall 事件的回答者（$events 推来的"内核等人回答"帧）。
    /// 与 emit 分开：waterfall 帧带 eventId，宿主挂起等待，必须回传 $events/result 才继续。</summary>
    private readonly Dictionary<string, List<Func<JsonElement, Task>>> _waterfallHandlers = new();
    private Task? _receiveLoop;
    private Task? _reconnectLoop;
    /// <summary>0=无重连循环在跑；1=已有单循环（StartReconnectLoop 用 CAS 保证唯一）。</summary>
    private int _reconnectRunning;
    private volatile bool _disposed;
    /// <summary>意外断开重连退避：首次 0.5s，指数翻倍，封顶 30s（单循环串行重试）。</summary>
    private static readonly TimeSpan ReconnectInitialDelay = TimeSpan.FromMilliseconds(500);
    private static readonly TimeSpan ReconnectMaxDelay = TimeSpan.FromSeconds(30);
    private string? _eventsClientId;

    /// <summary>内核事件流 ready 帧的全局回执（收到 ready 才能回传 $events/result）。</summary>
    private TaskCompletionSource _eventsReady = new(TaskCreationOptions.RunContinuationsAsynchronously);
    /// <summary>$events 已收到 ready 的凭据；未 ready 时回传审批结果需要先等它。</summary>
    public Task EventsReady => _eventsReady.Task;

    public DshRpcClient(Uri baseUrl)
    {
        _base = baseUrl;
        _http = new HttpClient(
            new HttpClientHandler { CookieContainer = _cookies, UseCookies = true },
            disposeHandler: true)
        {
            BaseAddress = baseUrl,
            Timeout = TimeSpan.FromSeconds(120),
        };
    }

    /// <summary>
    /// token 握手：GET /?token=... 换 dsh-auth cookie。此后 HttpClient 与
    /// WebSocket 都带该 cookie（内核浏览器鉴权协议）。
    /// </summary>
    public async Task AuthenticateAsync(string tokenUrl, CancellationToken ct = default)
    {
        var resp = await _http.GetAsync(tokenUrl, ct);
        resp.EnsureSuccessStatusCode();
    }

    /// <summary>把 HttpClient 持有的 dsh-auth cookie 拼成请求头（供 mux WebSocket）。</summary>
    private string GetCookieHeader()
    {
        var list = _cookies.GetCookies(_base);
        return string.Join("; ", list.Select(c => $"{c.Name}={c.Value}"));
    }

    /// <summary>HTTP JSON RPC：POST /api/&lt;endpoint&gt;，信封 {type,rpcId,method,payload:{args}}。</summary>
    public async Task<JsonElement> CallAsync(string endpoint, object? args = null, CancellationToken ct = default)
    {
        var id = Interlocked.Increment(ref _rpcId);
        var envelope = new
        {
            type = "client-request",
            rpcId = $"c2-{id}",
            method = endpoint,
            payload = new { args = args ?? new { } },
        };
        var body = JsonSerializer.Serialize(envelope);
        using var content = new StringContent(body, Encoding.UTF8, "application/json");
        using var resp = await _http.PostAsync($"api/{endpoint}", content, ct);
        resp.EnsureSuccessStatusCode();
        using var doc = JsonDocument.Parse(await resp.Content.ReadAsStringAsync(ct));
        return doc.RootElement.GetProperty("result").Clone();
    }

    /// <summary>便捷：调用并断言 ok=true，返回 value；否则抛出内核错误码。
    /// 一切失败（网络层、非 JSON 响应、信封缺字段）都归一成 DshRpcException——
    /// 调用点普遍只 catch 这一类型，漏网异常会沿 async void 上抛成 0xc000027b 闪退。</summary>
    public async Task<JsonElement> CallOkAsync(string endpoint, object? args = null, CancellationToken ct = default)
    {
        JsonElement result;
        try
        {
            result = await CallAsync(endpoint, args, ct);
        }
        catch (DshRpcException) { throw; }
        catch (Exception ex)
        {
            throw new DshRpcException("network", $"{endpoint}: {ex.Message}");
        }
        if (!result.TryGetProperty("ok", out var okEl) || okEl.ValueKind != JsonValueKind.True)
        {
            var err = result.TryGetProperty("error", out var e) && e.ValueKind == JsonValueKind.Object ? e : default;
            var code = err.ValueKind == JsonValueKind.Object && err.TryGetProperty("code", out var c) && c.ValueKind == JsonValueKind.String
                ? c.GetString() ?? "unknown" : "unknown";
            var message = err.ValueKind == JsonValueKind.Object && err.TryGetProperty("message", out var m) && m.ValueKind == JsonValueKind.String
                ? m.GetString() ?? "dsh RPC failed" : "dsh RPC failed";
            throw new DshRpcException(code, message);
        }
        return result.TryGetProperty("value", out var value) ? value.Clone() : default;
    }

    /// <summary>服务器基地址（http://127.0.0.1:port/），附件上传等直连 URL 用。</summary>
    public Uri BaseUri => _base;

    /// <summary>
    /// 附件上传：POST api/session/uploadFileBinary?sessionId=&name=，
    /// body=原始字节（application/octet-stream，cookie 鉴权）→ 返回 receiptId。
    /// </summary>
    public async Task<string?> UploadBytesAsync(Uri url, byte[] bytes, CancellationToken ct = default)
    {
        using var content = new ByteArrayContent(bytes);
        content.Headers.ContentType = new System.Net.Http.Headers.MediaTypeHeaderValue("application/octet-stream");
        using var resp = await _http.PostAsync(url, content, ct);
        resp.EnsureSuccessStatusCode();
        using var doc = JsonDocument.Parse(await resp.Content.ReadAsStringAsync(ct));
        // 响应形态可能是 {receiptId} 或 result 信封——两种都兼容
        var root = doc.RootElement;
        if (root.TryGetProperty("receiptId", out var rid))
        {
            return rid.GetString();
        }
        if (root.TryGetProperty("result", out var result) && result.ValueKind == JsonValueKind.Object)
        {
            if (result.TryGetProperty("value", out var value))
            {
                if (value.ValueKind == JsonValueKind.String)
                {
                    return value.GetString();
                }
                if (value.TryGetProperty("receiptId", out var rid2))
                {
                    return rid2.GetString();
                }
            }
        }
        return root.ValueKind == JsonValueKind.String ? root.GetString() : null;
    }

    /// <summary>
    /// 带鉴权的 GET 文本（插件域路由，如 /api/pet/state）。
    /// 复用握手后的 cookie——插件路由虽然只监听回环，但内核 webserver 有鉴权层，
    /// 裸 HttpClient 会被 401/404 吞掉真实错误，因此走同一个 _http。
    /// </summary>
    internal async Task<string> GetTextAsync(string url, CancellationToken ct = default)
    {
        using var resp = await _http.GetAsync(url, ct);
        resp.EnsureSuccessStatusCode();
        return await resp.Content.ReadAsStringAsync(ct);
    }

    /// <summary>带鉴权的 GET 字节流（宠物图集/预览图）。</summary>
    internal async Task<byte[]> GetBytesAsync(string url, CancellationToken ct = default)
    {
        using var resp = await _http.GetAsync(url, ct);
        resp.EnsureSuccessStatusCode();
        return await resp.Content.ReadAsByteArrayAsync(ct);
    }

    /// <summary>
    /// 带鉴权的流式下载（壁纸视频等大文件直落盘，不经内存）。先写临时文件再 Move 覆盖，
    /// 避免下载中断在半路留下一个能启动的坏文件（皮肤加载按文件存在性判断）。
    /// </summary>
    internal async Task DownloadToFileAsync(string url, string destPath, CancellationToken ct = default)
    {
        using var resp = await _http.GetAsync(url, HttpCompletionOption.ResponseHeadersRead, ct);
        resp.EnsureSuccessStatusCode();
        var dir = Path.GetDirectoryName(destPath);
        if (!string.IsNullOrEmpty(dir))
        {
            Directory.CreateDirectory(dir);
        }
        var tmp = destPath + ".part";
        try
        {
            await using (var target = new FileStream(tmp, FileMode.Create, FileAccess.Write, FileShare.None))
            {
                await using var source = await resp.Content.ReadAsStreamAsync(ct);
                await source.CopyToAsync(target, ct);
            }
            File.Move(tmp, destPath, overwrite: true);
        }
        catch (Exception)
        {
            try { File.Delete(tmp); } catch (Exception) { }
            throw;
        }
    }

    /// <summary>带鉴权的 POST JSON（宠物 set-pet/set-config/interact 等），返回响应文本。</summary>
    internal async Task<string> PostJsonAsync(string url, string json, CancellationToken ct = default)
    {
        using var content = new StringContent(json, Encoding.UTF8, "application/json");
        using var resp = await _http.PostAsync(url, content, ct);
        resp.EnsureSuccessStatusCode();
        return await resp.Content.ReadAsStringAsync(ct);
    }

    /// <summary>
    /// 打开 mux WebSocket（$events 事件流自动随连订阅）。
    /// </summary>
    public async Task ConnectMuxAsync(CancellationToken ct = default)
    {
        await SubscribeEventsAsync(ct);
    }

    /// <summary>
    /// 只建 mux 连接，不订阅 $events——供需要先触碰其他服务（如 workspace 域的
    /// bootstrap）再订阅事件的调用方使用（0.7.1：订阅顺序会影响内核服务初始化次序）。
    /// </summary>
    public async Task ConnectMuxEventsDeferredAsync(CancellationToken ct = default)
    {
        await EnsureMuxAsync(ct);
    }

    private async Task ConnectMuxSocketAsync(CancellationToken ct)
    {
        var socket = new ClientWebSocket();
        try
        {
            var cookieHeader = GetCookieHeader();
            if (!string.IsNullOrEmpty(cookieHeader))
                socket.Options.SetRequestHeader("Cookie", cookieHeader);
            var scheme = _base.Scheme == "https" ? "wss" : "ws";
            await socket.ConnectAsync(new Uri($"{scheme}://{_base.Authority}/api/remote.mux"), ct);
            ct.ThrowIfCancellationRequested();
            _mux = socket;
            _receiveLoop = Task.Run(() => ReceiveLoopAsync(socket, _lifetime.Token));
        }
        catch
        {
            socket.Dispose();
            throw;
        }
    }

    /// <summary>$events 事件流订阅（供延迟订阅路径；已订阅则幂等跳过）。</summary>
    public async Task SubscribeEventsAsync(CancellationToken ct = default)
    {
        _eventsSubscribed = true;
        await EnsureMuxAsync(ct);
    }

    private bool _eventsSubscribed;

    // text-frame-only mux：流 id（字符串）→ 回调。开流返回 id，取消用 CancelStream(id)。

    /// <summary>在 mux 上开一条流（open 帧），逐帧回调 sink（$events 走事件分发，不入 sink 表）。</summary>
    private async Task<string> OpenStreamAsync(string endpoint, object payload, Action<JsonElement>? sink = null, CancellationToken ct = default)
    {
        var id = $"c2s{Interlocked.Increment(ref _streamSeq)}";
        var open = JsonSerializer.Serialize(new
        {
            type = "open",
            streamId = id,
            endpoint,
            payload,
        });
        if (sink is not null) _streamSinks[id] = sink;
        if (endpoint == "$events") _eventsStreamId = id;
        try
        {
            await SendMuxAsync(open, ct);
            return id;
        }
        catch
        {
            _streamSinks.TryRemove(id, out _);
            if (_eventsStreamId == id) _eventsStreamId = null;
            throw;
        }
    }

    private async Task SendMuxAsync(string text, CancellationToken ct)
    {
        var socket = _mux ?? throw new InvalidOperationException("事件连接尚未建立。");
        await _sendGate.WaitAsync(ct);
        try
        {
            await socket.SendAsync(Encoding.UTF8.GetBytes(text), WebSocketMessageType.Text, true, ct);
        }
        finally { _sendGate.Release(); }
    }

    private async Task CancelStreamAsync(string streamId)
    {
        try { await SendMuxAsync(JsonSerializer.Serialize(new { type = "cancel", streamId }), _lifetime.Token); }
        catch (Exception ex) when (ex is WebSocketException or OperationCanceledException or ObjectDisposedException or InvalidOperationException) { }
    }

    /// <summary>取消一条已开的流（cancel 帧）；不存在则忽略。</summary>
    public void CancelStream(string streamId)
    {
        _streamSinks.TryRemove(streamId, out _);
        if (_mux is { State: WebSocketState.Open })
        {
            try
            {
                _ = CancelStreamAsync(streamId);
            }
            catch (Exception) { }
        }
    }

    /// <summary>
    /// session/follow：跟随一个会话的 journal 增量流，返回流 id（取消用 CancelStream）。
    /// mux 已断时先自动重连再开流；失败抛给调用方。
    /// assistantStream=true 让内核在持久事件之外再叠一条逐字增量帧流（type:"assistant-stream"，
    /// 见 dsh-api-session-controller 的 follow：仅在该参数为 true 时订阅 agent/assistant-stream 总线）。
    /// </summary>
    public async Task<string> OpenFollowStreamAsync(string sessionId, Action<JsonElement> onFrame, bool assistantStream = false, CancellationToken ct = default)
    {
        await EnsureMuxAsync(ct);
        var request = new Dictionary<string, object>
        {
            ["address"] = new { kind = "session", sessionId },
        };
        // 内核 z-schema 只认字面量 true（null/false 会 arguments-invalid），故按开关决定是否带键
        if (assistantStream)
        {
            request["assistantStream"] = true;
        }
        return await OpenStreamAsync("session/follow", new { args = new { request } }, onFrame, ct);
    }

    /// <summary>
    /// workspace/follow：工作区树的 baseline + 增量流（upsert/remove/order/archived）。
    /// 返回流 id 供取消。
    /// </summary>
    public async Task<string> OpenWorkspaceFollowStreamAsync(Action<JsonElement> onFrame, CancellationToken ct = default)
    {
        await EnsureMuxAsync(ct);
        return await OpenStreamAsync("workspace/follow", new { args = new { } }, onFrame, ct);
    }

    /// <summary>
    /// 通用 mux 流：任意 <c>mode=stream</c> 的 Remote 端点（如 workspaceFiles/changes）。
    /// args 键名仍以内核 typert 描述符的 parameters[].wire 为准；逐帧回调收到的是**结果值本身**
    /// （dsh-api-gateway 的 mux pump 直接把流元素放进 item.value，不套 {ok,value} 信封）。
    /// 返回流 id 供 CancelStream；
    /// 注意：流内错误帧（{type:"error"}）与结束帧（{type:"end"}）不进 onFrame，
    /// 仅经 <see cref="StreamEnded"/> 通知（终态，不自动重试，见 Dispatch）。
    /// </summary>
    public async Task<string> OpenRemoteStreamAsync(string endpoint, object args, Action<JsonElement> onFrame, CancellationToken ct = default)
    {
        await EnsureMuxAsync(ct);
        return await OpenStreamAsync(endpoint, new { args }, onFrame, ct);
    }

    /// <summary>mux 重连：重置连接与流表（旧流的 sink 随连接消失全部作废）。
    /// 重连后 $events 事件流自动补订（订阅凭据 _eventsSubscribed 不随连接重置）；
    /// 调用方的长驻业务流（session/control、workspace/follow）由 <see cref="StreamsReset"/> 通知重开。
    /// 意外断开的后台自动重连（<see cref="ReconnectLoopAsync"/>）复用本方法做幂等修复；
    /// 按需调用与后台循环都在 gate 内判定，不会重复建连/重复订阅。</summary>
    private bool _streamsResetPending;
    private async Task EnsureMuxAsync(CancellationToken ct = default)
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
        using var linked = CancellationTokenSource.CreateLinkedTokenSource(ct, _lifetime.Token);
        ct = linked.Token;
        var reset = false;
        await _connectionGate.WaitAsync(ct);
        try
        {
            ObjectDisposedException.ThrowIf(_disposed, this);
            if (_mux is null || _mux.State != WebSocketState.Open)
            {
                // 保留到恢复成功：第一次重连失败后 _mux 已为 null，不能丢失业务通知。
                _streamsResetPending |= _mux is not null;
                _mux?.Dispose();
                _mux = null;
                _streamSinks.Clear();
                _eventsStreamId = null;
                _eventsClientId = null;
                if (_eventsReady.Task.IsCompleted)
                    _eventsReady = new(TaskCreationOptions.RunContinuationsAsynchronously);
                await ConnectMuxSocketAsync(ct);
            }
            if (_eventsSubscribed && _eventsStreamId is null)
                await OpenStreamAsync("$events", new { args = new { } }, ct: ct);
            reset = _streamsResetPending;
            _streamsResetPending = false;
        }
        finally { _connectionGate.Release(); }
        // 在 gate 外通知，允许处理器重开流；单个业务处理器异常不能破坏传输恢复。
        if (reset && !_disposed && StreamsReset is { } handlers)
            foreach (Action handler in handlers.GetInvocationList())
                try { handler(); } catch (Exception) { }
    }

    /// <summary>mux 重连后触发（流表已作废）：调用方在此重开自己的长驻流。
    /// 仅在连接级重连时触发；$events 单独收到 end/error 时不触发，
    /// 此类终态需要显式 SubscribeEventsAsync 才重试。</summary>
    public event Action? StreamsReset;

    /// <summary>意外断开的自动重连：单循环 + 指数退避（0.5s 起翻倍、封顶 30s）。
    /// 触发点：接收循环退出（socket 死亡）；服务端流 end/error 不启动传输重试。
    /// 每轮复用 <see cref="EnsureMuxAsync"/>（gate 内幂等判定，绝不重复订阅 $events）；
    /// 修复成功即退出循环，失败按退避等待后重试。<see cref="DisposeAsync"/> 经
    /// _disposed 标志与 _lifetime 取消令牌终止循环，Dispose 后不再发起新连接。</summary>
    private void StartReconnectLoop()
    {
        if (_disposed || Interlocked.CompareExchange(ref _reconnectRunning, 1, 0) != 0)
        {
            return;
        }
        _reconnectLoop = Task.Run(() => ReconnectLoopAsync(_lifetime.Token));
    }

    private async Task ReconnectLoopAsync(CancellationToken ct)
    {
        try
        {
            var delay = ReconnectInitialDelay;
            while (!_disposed && !ct.IsCancellationRequested)
            {
                try
                {
                    await EnsureMuxAsync(ct);
                    return; // 连接与 $events 订阅均已恢复，单循环退出
                }
                catch (Exception ex) when (ex is not OperationCanceledException)
                {
                    // 退避后重试；Dispose/取消经 Task.Delay 抛 OCE 由外层接住
                    await Task.Delay(delay, ct);
                    delay = TimeSpan.FromTicks(Math.Min(delay.Ticks * 2, ReconnectMaxDelay.Ticks));
                }
            }
        }
        catch (OperationCanceledException) { } // Dispose 取消：静默退出
        finally
        {
            Interlocked.Exchange(ref _reconnectRunning, 0);
            // 关闭"退出与再次断开"竞态窗口：若退出瞬间连接又已断开/$events 缺失，
            // 重新武装循环（_disposed 时 StartReconnectLoop 自行拒绝）。
            if (!_disposed && (_mux is null || _mux.State != WebSocketState.Open
                || (_eventsSubscribed && _eventsStreamId is null)))
            {
                StartReconnectLoop();
            }
        }
    }

    private async Task ReceiveLoopAsync(ClientWebSocket socket, CancellationToken ct)
    {
        var buffer = new byte[1024 * 512];
        try
        {
            while (!ct.IsCancellationRequested && socket.State == WebSocketState.Open)
            {
                using var ms = new System.IO.MemoryStream();
                WebSocketReceiveResult received;
                do
                {
                    received = await socket.ReceiveAsync(new ArraySegment<byte>(buffer), ct);
                    if (received.MessageType == WebSocketMessageType.Close)
                    {
                        return;
                    }
                    ms.Write(buffer, 0, received.Count);
                } while (!received.EndOfMessage);

                using var doc = JsonDocument.Parse(Encoding.UTF8.GetString(ms.ToArray()));
                if (ReferenceEquals(socket, _mux)) Dispatch(doc.RootElement.Clone());
            }
        }
        catch (OperationCanceledException) { }
        catch (WebSocketException) { }
        catch (JsonException) { }
        catch (ObjectDisposedException) { }
        finally
        {
            await _connectionGate.WaitAsync();
            try
            {
                if (ReferenceEquals(socket, _mux))
                {
                    // JSON/回调异常退出时 socket 可能仍 Open；必须使状态失效才能真正重连。
                    socket.Abort();
                    _eventsClientId = null;
                    _eventsStreamId = null;
                    _eventsReady.TrySetCanceled();
                    if (!_disposed && !ct.IsCancellationRequested) StartReconnectLoop();
                }
            }
            finally { _connectionGate.Release(); }
        }
    }

    private void Dispatch(JsonElement frame)
    {
        var type = frame.GetProperty("type").GetString();
        var streamId = frame.GetProperty("streamId").GetString();
        if (type != "item")
        {
            if (streamId is not null && type is "end" or "error")
            {
                _streamSinks.TryRemove(streamId, out _);
                if (streamId == _eventsStreamId)
                {
                    _eventsStreamId = null;
                    _eventsClientId = null;
                    _eventsReady.TrySetCanceled();
                    _eventsReady = new(TaskCreationOptions.RunContinuationsAsynchronously);
                    // 服务端终态不等于传输断开：可能是永久授权/业务错误。
                    // 停止自动订阅，由调用方显式 SubscribeEventsAsync 决定是否重试。
                    _eventsSubscribed = false;
                }
                // 业务流的 end/error 是终态：不自动重试（永久性业务错误如 session 已删除，
                // 重试只会无限循环），错误经 StreamEnded 交给调用方自行决定是否重开。
                StreamEnded?.Invoke(streamId, frame.TryGetProperty("error", out var error) ? error.ToString() : null);
            }
            return;
        }
        var value = frame.GetProperty("value");

        // $events 流（0.7.0 探针定论帧形态）：
        //   ready：{type:"ready", clientId, host} —— 回传 $events/result 的前置凭据
        //   emit ：{event:"<名>", args:[...位置参数]} —— 事件载荷是数组，
        //          单参事件（api-session/status 等）args[0] 即载荷；审批等带 eventId 的
        //          事件 clientId 也在 value 顶层
        if (streamId is not null && streamId == _eventsStreamId)
        {
            var frameType = value.ValueKind == JsonValueKind.Object && value.TryGetProperty("type", out var ft) ? ft.GetString() : null;
            if (frameType == "ready")
            {
                _eventsClientId = value.TryGetProperty("clientId", out var cid) ? cid.GetString() : null;
                if (!string.IsNullOrEmpty(_eventsClientId)) _eventsReady.TrySetResult();
                return;
            }
            if (frameType == "cancel")
            {
                if (value.TryGetProperty("eventId", out var cancelled) && cancelled.GetString() is { } eventId)
                    EventCancelled?.Invoke(eventId);
                return;
            }
            if (frameType == "emit" &&
                value.TryGetProperty("event", out var ev) && ev.GetString() is { } eventName)
            {
                // 载荷 = 完整 emit 帧（{type,event,args:[...位置参数]}）：
                // 单参事件 args[0] 是主载荷；多参事件（api-session/status 的 [sessionId,running]）
                // 处理器自取 args。ready 帧的 clientId 已在上面单独取。
                if (_eventHandlers.TryGetValue(eventName, out var handlers))
                {
                    foreach (var h in handlers)
                    {
                        _ = Task.Run(() => h(value), CancellationToken.None);
                    }
                }
                // 通用事件监听（调试/日志）
                if (_eventHandlers.TryGetValue("*", out var anys))
                {
                    foreach (var h in anys)
                    {
                        _ = Task.Run(() => h(value), CancellationToken.None);
                    }
                }
            }
            // waterfall 帧（内核等待回答，必须回传 $events/result 才解挂）：
            //   {type:"waterfall", event:"<名>", eventId, agentId, request:{…}}
            // 载荷在 request 字段（网关已剥离 agent/signal 两个非 JSON 字段）。
            // 实测来源：dsh-api-gateway startRemoteEvent；白名单见 dsh-api-remotes
            // API_REMOTE_FORWARDED_EVENTS（本部署 waterfall 模式两条：approval/request、
            // user-questions/request）。审批走 OnEvent 既有路径（0.7.0 已接），此处只补未接的。
            if (frameType == "waterfall" &&
                value.TryGetProperty("event", out var wev) && wev.GetString() is { } waterfallName)
            {
                if (_waterfallHandlers.TryGetValue(waterfallName, out var whandlers))
                {
                    foreach (var h in whandlers)
                    {
                        _ = Task.Run(() => h(value), CancellationToken.None);
                    }
                }
            }
            return;
        }

        if (streamId is not null && _streamSinks.TryGetValue(streamId, out var sink))
        {
            sink(value);
        }
    }

    /// <summary>订阅内核事件（approval/request、api-session/added/status…）。必须在 ConnectMuxAsync 前注册。</summary>
    public void OnEvent(string eventName, Func<JsonElement, Task> handler)
    {
        if (!_eventHandlers.TryGetValue(eventName, out var list))
        {
            list = new();
            _eventHandlers[eventName] = list;
        }
        list.Add(handler);
    }

    /// <summary>
    /// 订阅 waterfall 事件（内核在 $events 上推 {type:"waterfall",…} 并挂起等待回传）。
    /// 处理器收到的是完整 waterfall 帧，必须调用 <see cref="ResolveWaterfallAsync"/> 才会解挂。
    /// 同名事件若已用 <see cref="OnEvent"/> 注册（本部署的 approval/request 走那条），
    /// 此处不重复派发——避免同一条帧被两条链路各答一次。
    /// </summary>
    public void OnWaterfall(string eventName, Func<JsonElement, Task> handler)
    {
        if (!_waterfallHandlers.TryGetValue(eventName, out var list))
        {
            list = new();
            _waterfallHandlers[eventName] = list;
        }
        list.Add(handler);
    }

    /// <summary>
    /// waterfall 回答回传：POST /api/$events/result {clientId,eventId,outcome:{kind:"result",value}}。
    /// value 形态由事件本身决定（user-questions/request = {answers:[{id,selected:[…],custom?}]}，
    /// 见 dsh-tool-ask-user 的 execute 返回与 dsh-client-ui-user-questions 的 answerQuestion）。
    /// </summary>
    public Task ResolveWaterfallAsync(string eventId, object value, CancellationToken ct = default)
        => SendEventOutcomeAsync(eventId, new { kind = "result", value }, ct);

    public Task SkipWaterfallAsync(string eventId, CancellationToken ct = default)
        => SendEventOutcomeAsync(eventId, new { kind = "next" }, ct);

    private async Task SendEventOutcomeAsync(string eventId, object outcome, CancellationToken ct)
    {
        await _eventsReady.Task.WaitAsync(TimeSpan.FromSeconds(10), ct);
        var clientId = _eventsClientId;
        if (string.IsNullOrEmpty(clientId))
            throw new InvalidOperationException("事件连接已失效，请等待重新连接后再回答。");
        await CallOkAsync("$events/result", new { clientId, eventId, outcome }, ct);
    }

    /// <summary>
    /// 裸 GET（Cookie 鉴权，不带 /api 前缀）：内核宿主网页路由直连。
    /// 用途：dsh-host-open-in-app 的 GET /open-in-app/apps、
    /// dsh-client-ui-deliverables 的 GET /api/present.host。
    /// </summary>
    public async Task<(int Status, string Body)> GetRawAsync(string relativePath, CancellationToken ct = default)
    {
        var path = relativePath.TrimStart('/');
        using var resp = await _http.GetAsync(path, ct);
        return ((int)resp.StatusCode, await resp.Content.ReadAsStringAsync(ct));
    }

    /// <summary>
    /// 裸 POST JSON（Cookie 鉴权，不带 /api 前缀）：内核宿主网页路由直连。
    /// 用途：POST /open-in-app/open（body {app,path}）、
    /// POST /api/present.open?sessionId=&amp;seq=&amp;index=&amp;action=open|reveal（无 body）。
    /// </summary>
    public async Task<(int Status, string Body)> PostRawAsync(string relativePath, object? body = null, CancellationToken ct = default)
    {
        var path = relativePath.TrimStart('/');
        HttpContent content = body is null
            ? new ByteArrayContent(Array.Empty<byte>())
            : new StringContent(JsonSerializer.Serialize(body), Encoding.UTF8, "application/json");
        using var resp = await _http.PostAsync(path, content, ct);
        return ((int)resp.StatusCode, await resp.Content.ReadAsStringAsync(ct));
    }

    /// <summary>审批决定回传：POST /api/$events/result {clientId,eventId,outcome}。
    /// clientId 必须用 $events ready 帧发来的那个（0.7.0 前误用过 emit 帧 clientId——
    /// 探针实测回传报 "identifies no active event stream"）；ready 未到时先等它。</summary>
    public Task ResolveEventAsync(string eventId, string outcome, CancellationToken ct = default)
    {
        if (outcome is not ("allowed-once" or "rejected"))
            throw new ArgumentOutOfRangeException(nameof(outcome));
        return ResolveWaterfallAsync(eventId, outcome, ct);
    }

    public async ValueTask DisposeAsync()
    {
        _disposed = true;   // 先置位：接收循环/重连循环不会再启动新连接
        _lifetime.Cancel();
        _eventsReady.TrySetCanceled();
        // 等待被 lifetime 取消的握手退出，避免 Connect 完成/Dispose 的发布竞态。
        await _connectionGate.WaitAsync();
        try
        {
            _mux?.Dispose();
            _http.Dispose();
        }
        finally { _connectionGate.Release(); }
    }
}

/// <summary>内核 RPC 失败：Code = 内核错误码（typert/gateway/业务码）。</summary>
public sealed class DshRpcException(string code, string message) : Exception($"[{code}] {message}")
{
    /// <summary>稳定错误码，如 workspace-file/not-text、messageFeedback 的 version-conflict。</summary>
    public string Code { get; } = code;
}
