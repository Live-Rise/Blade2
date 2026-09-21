#if DEBUG
using System;
using System.Collections.Concurrent;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Sockets;
using System.Net.WebSockets;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using Blade2.Dsh;
using Microsoft.UI.Xaml;

namespace Blade2;

public sealed partial class MainWindow
{
    private async Task RunReconnectProbeAsync(string output)
    {
        using var gateway = new ReconnectProbeGateway();
        try
        {
            KernelBootPanel.Visibility = Visibility.Collapsed;
            ShowChatPage();
            ChatHero.Visibility = Visibility.Collapsed;
            _activeSessionId = "synthetic-reconnect";
            _compactTranscript = false;
            _rpc = new DshRpcClient(gateway.Address);
            RegisterStreamRecovery();
            await _rpc.ConnectMuxAsync();
            await RefreshWorkspacesAsync();
            await FollowSessionAsync(_activeSessionId);
            await OpenSessionControlStreamAsync();
            async Task WaitFor(Func<bool> condition, string name)
            {
                using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(15));
                while (!condition()) await Task.Delay(50, timeout.Token);
                if (!condition()) throw new InvalidOperationException(name);
            }
            await WaitFor(() => _visibleMessages.Count == 1 && gateway.ControlOpens >= 1, "Initial conversation");
            var initialStream = _sessionStreamId;
            gateway.Disconnect();
            await WaitFor(() => _visibleMessages.Count == 4 && gateway.ControlOpens >= 2 && gateway.WorkspaceOpens >= 2, "Restored conversation");
            await Task.Delay(300);
            var expected = new[] { "before-disconnect", "during-disconnect", "snapshot-tail", "after-reconnect" };
            var actual = _visibleMessages.Select(m => m.Text).ToArray();
            if (!actual.SequenceEqual(expected)) throw new InvalidOperationException("Missing, duplicate or unordered messages: " + string.Join(",", actual));
            if (initialStream == _sessionStreamId) throw new InvalidOperationException("Stream ID was reused");
            if (ChatList.Items.Count != 4 || ChatList.ActualWidth <= 0) throw new InvalidOperationException("Chat ListView was not populated");
            if (gateway.PageCalls != 1) throw new InvalidOperationException("Snapshot gap was not fetched exactly once");
            await File.WriteAllTextAsync(output, JsonSerializer.Serialize(new { pass = true, messages = actual, items = ChatList.Items.Count, gateway.Connections, gateway.WorkspaceOpens, gateway.ControlOpens, gateway.PageCalls, streamChanged = true }));
        }
        catch (Exception ex) { await File.WriteAllTextAsync(output, ex.ToString()); }
        finally
        {
            if (_rpc is not null) await _rpc.DisposeAsync();
            _rpc = null;
            Close();
        }
    }

    private sealed class ReconnectProbeGateway : IDisposable
    {
        private readonly HttpListener _listener = new();
        private readonly CancellationTokenSource _stop = new();
        private readonly ConcurrentBag<WebSocket> _sockets = new();
        public Uri Address { get; }
        public int Connections, WorkspaceOpens, ControlOpens, PageCalls;
        private int _phase;
        public ReconnectProbeGateway()
        {
            var port = new TcpListener(IPAddress.Loopback, 0);
            port.Start(); var number = ((IPEndPoint)port.LocalEndpoint).Port; port.Stop();
            Address = new Uri($"http://127.0.0.1:{number}/");
            _listener.Prefixes.Add(Address.ToString()); _listener.Start();
            _ = Task.Run(AcceptAsync);
        }
        public void Disconnect()
        {
            Interlocked.Exchange(ref _phase, 1);
            foreach (var socket in _sockets) socket.Abort();
        }
        private static object Event(int seq, string text) => new { type = "user/message", seq, time = 1, data = new { content = new[] { new { type = "text", text } } } };
        private static object Record(int seq, string text) => new { type = "event", @event = Event(seq, text) };
        private async Task AcceptAsync()
        {
            try { while (!_stop.IsCancellationRequested) { var context = await _listener.GetContextAsync(); _ = Task.Run(() => HandleAsync(context)); } }
            catch (Exception) when (_stop.IsCancellationRequested) { }
        }
        private async Task HandleAsync(HttpListenerContext context)
        {
            try
            {
                if (!context.Request.IsWebSocketRequest)
                {
                    object value;
                    if (context.Request.Url!.AbsolutePath.EndsWith("session/page"))
                    {
                        Interlocked.Increment(ref PageCalls);
                        value = new { records = new[] { Record(1, "before-disconnect"), Record(2, "during-disconnect") }, hasMore = false };
                    }
                    else value = new { sessions = Array.Empty<object>() };
                    var bytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(new { type = "server-response", result = new { ok = true, value } }));
                    context.Response.ContentType = "application/json";
                    await context.Response.OutputStream.WriteAsync(bytes);
                    context.Response.Close(); return;
                }
                var accepted = await context.AcceptWebSocketAsync(null);
                var socket = accepted.WebSocket; _sockets.Add(socket);
                Interlocked.Increment(ref Connections);
                async Task Item(string id, object value)
                {
                    var bytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(new { type = "item", streamId = id, value }));
                    await socket.SendAsync(new ArraySegment<byte>(bytes), WebSocketMessageType.Text, true, _stop.Token);
                }
                var buffer = new byte[65536];
                while (!_stop.IsCancellationRequested && socket.State == WebSocketState.Open)
                {
                    using var message = new MemoryStream(); WebSocketReceiveResult result;
                    do { result = await socket.ReceiveAsync(new ArraySegment<byte>(buffer), _stop.Token); if (result.MessageType == WebSocketMessageType.Close) return; message.Write(buffer, 0, result.Count); } while (!result.EndOfMessage);
                    using var doc = JsonDocument.Parse(message.ToArray()); var frame = doc.RootElement;
                    if (frame.GetProperty("type").GetString() != "open") continue;
                    var id = frame.GetProperty("streamId").GetString()!;
                    switch (frame.GetProperty("endpoint").GetString())
                    {
                        case "$events": await Item(id, new { type = "ready", clientId = "probe-" + Connections, host = new { } }); break;
                        case "workspace/follow":
                            Interlocked.Increment(ref WorkspaceOpens);
                            await Item(id, new { type = "baseline", value = new { items = Array.Empty<object>(), archivedSessionIds = Array.Empty<string>() } }); break;
                        case "session/control":
                            Interlocked.Increment(ref ControlOpens);
                            await Item(id, new { type = "baseline", value = new { queues = new { }, jobs = new { }, projections = new { } } }); break;
                        case "session/follow":
                            if (Volatile.Read(ref _phase) == 0)
                                await Item(id, new { type = "snapshot", cursor = 1, records = new[] { Record(1, "before-disconnect") }, hasMore = false });
                            else
                            {
                                await Item(id, new { type = "snapshot", cursor = 3, records = new[] { Record(3, "snapshot-tail") }, hasMore = true });
                                await Item(id, Record(3, "snapshot-tail"));
                                await Item(id, Record(4, "after-reconnect"));
                                await Item(id, Record(4, "after-reconnect"));
                            }
                            break;
                    }
                }
            }
            catch (Exception) { }
        }
        public void Dispose()
        {
            _stop.Cancel(); _listener.Close();
            foreach (var socket in _sockets) socket.Dispose();
        }
    }
}
#endif
