import Foundation

/// 内核 RPC 失败:code 为内核稳定错误码(gateway/lookup-not-found、arguments-invalid…)。
struct DshRpcError: Error, @unchecked Sendable {
    let code: String
    let message: String
    var errorDescription: String? { "[\(code)] \(message)" }
    var isPastCursor: Bool { message.contains("past cursor") }
}

/// 跨隔离域回调的 Sendable 盒:MainActor 同步赋值,actor 内读取。
final class CallbackBox<T>: @unchecked Sendable {
    var value: T?
    init(_ value: T? = nil) { self.value = value }
}

/// dsh 内核 RPC 客户端(与 Windows 版 DshRpcClient.cs 协议逐条对齐):
///   HTTP JSON RPC:POST /api/<endpoint>,信封 {type:"client-request", rpcId, method, payload:{args}}
///   mux WebSocket:/api/remote.mux,纯文本帧 open/cancel/item/end/error;streamId 必须是字符串
///   $events 流:ready(取 clientId)/emit/cancel/waterfall;回答走 POST /api/$events/result
///   重连:意外断开单循环指数退避 0.5s→30s;恢复后自动补订 $events,业务流由 StreamsReset 通知重开
actor DshRpcClient {
    // MARK: 回调类型(实现方内部自行切主线程)
    typealias FrameSink = @Sendable (JSON) -> Void
    typealias StreamEnd = @Sendable (String?) -> Void

    // MARK: - 状态

    nonisolated let base: URL
    private let http: URLSession
    private var rpcId = 0
    private var streamSeq = 0

    private var mux: URLSessionWebSocketTask?
    private var receiveTask: Task<Void, Never>?
    private var streams: [String: (sink: FrameSink, end: StreamEnd?)] = [:]
    private var eventsStreamId: String?
    private var eventsClientId: String?
    private var eventsSubscribed = false
    private let eventsReady = Signal()
    private var eventHandlers: [String: [FrameSink]] = [:]
    private var waterfallHandlers: [String: [FrameSink]] = [:]

    private var disposed = false
    private var reconnectRunning = false
    private var resetPending = false

    /// 连接级重连后触发(流表已作废):调用方重开长驻业务流。
    /// (Sendable 盒:MainActor 同步赋值,actor 内读取)
    nonisolated let streamsResetBox = CallbackBox<@Sendable () -> Void>()
    /// $events 收到 {type:"cancel", eventId}(内核撤销了某个挂起的交互事件)。
    nonisolated let eventCancelledBox = CallbackBox<@Sendable (String) -> Void>()
    /// $events 流本身 end/error(终态,可能是永久授权/业务错误——不自动重订)。
    nonisolated let eventsStreamEndedBox = CallbackBox<@Sendable (String?) -> Void>()

    static let reconnectInitialDelay: Double = 0.5
    static let reconnectMaxDelay: Double = 30

    // MARK: - 初始化

    nonisolated init(base: URL) {
        self.base = base
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = 120
        config.timeoutIntervalForResource = 300
        config.httpShouldSetCookies = true
        self.http = URLSession(configuration: config)
    }

    nonisolated func dispose() {
        Task { await disposeAsync() }
    }

    private func disposeAsync() {
        disposed = true
        mux?.cancel(with: .goingAway, reason: nil)
        mux = nil
        receiveTask?.cancel()
        http.finishTasksAndInvalidate()
    }

    // MARK: - 鉴权

    /// token 握手:GET /?token=… 换 dsh-auth cookie(HTTPCookieStorage 自动持有)。
    func authenticate(tokenURL: URL) async throws {
        var req = URLRequest(url: tokenURL)
        req.timeoutInterval = 30
        let (_, resp) = try await http.data(for: req)
        guard let httpResp = resp as? HTTPURLResponse, (200..<300).contains(httpResp.statusCode) else {
            let status = (resp as? HTTPURLResponse)?.statusCode ?? -1
            throw DshRpcError(code: "network", message: "token handshake failed: HTTP \(status)")
        }
    }

    /// 把 cookie 罐拼成 Cookie 头(供 mux WebSocket,对应 C# GetCookieHeader)。
    private func cookieHeader() -> String {
        guard let storage = http.configuration.httpCookieStorage,
              let cookies = storage.cookies(for: base), !cookies.isEmpty else { return "" }
        return cookies.map { "\($0.name)=\($0.value)" }.joined(separator: "; ")
    }

    // MARK: - HTTP JSON RPC

    /// 规范拼 URL:base 可能带尾斜杠,路径保留 query;不能用 appendingPathComponent
    /// (会把端点名中的 "/" 转义成 %2F → 400)。
    nonisolated static func log(_ line: String) {
        let url = FileManager.default.urls(for: .libraryDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Logs/blade2-shell.log")
        let text = "\(Date().formatted(date: .abbreviated, time: .standard)) rpc: \(line)\n"
        if let fh = FileHandle(forWritingAtPath: url.path) {
            defer { try? fh.close() }
            fh.seekToEndOfFile()
            fh.write(Data(text.utf8))
        }
    }

    nonisolated static func apiURL(_ base: URL, _ path: String) -> URL? {
        var root = base.absoluteString
        while root.hasSuffix("/") { root.removeLast() }
        let suffix = path.hasPrefix("/") ? path : "/" + path
        return URL(string: root + suffix)
    }

    /// 裸调用:返回信封 result 字段({ok,value}|{ok:false,error}),不校验 ok。
    func call(_ endpoint: String, _ args: JSON = .object([:])) async throws -> JSON {
        rpcId += 1
        let envelope: JSON = .obj(
            ("type", .string("client-request")),
            ("rpcId", .string("c2-\(rpcId)")),
            ("method", .string(endpoint)),
            ("payload", .obj(("args", args)))
        )
                guard let url = Self.apiURL(base, "api/" + endpoint) else {
            throw DshRpcError(code: "network", message: "bad endpoint url: \(endpoint)")
        }
        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = envelope.encoded()
        Self.log("rpc \(endpoint) request body: \(String(data: req.httpBody ?? Data(), encoding: .utf8) ?? "?")")
        let data: Data, resp: URLResponse
        do {
            (data, resp) = try await http.data(for: req)
        } catch {
            throw DshRpcError(code: "network", message: "\(endpoint): \(error.localizedDescription)")
        }
        guard let httpResp = resp as? HTTPURLResponse, (200..<300).contains(httpResp.statusCode) else {
            let status = (resp as? HTTPURLResponse)?.statusCode ?? -1
            let bodyText = String(data: data, encoding: .utf8) ?? ""
            Self.log("rpc \(endpoint) -> HTTP \(status) method=\(req.httpMethod ?? "?") url=\(req.url?.absoluteString ?? "?") body=\(bodyText.prefix(200))")
            throw DshRpcError(code: "http-\(status)", message: "\(endpoint): HTTP \(status)")
        }
        guard let doc = try? JSON.decode(data), doc["result"].objectValue != nil else {
            throw DshRpcError(code: "network", message: "\(endpoint): 非 JSON 响应")
        }
        return doc["result"]
    }

    /// 便捷:断言 ok=true 返回 value;一切失败归一为 DshRpcError。
    func callOk(_ endpoint: String, _ args: JSON = .object([:])) async throws -> JSON {
        let result: JSON
        do {
            result = try await call(endpoint, args)
        } catch let err as DshRpcError {
            throw err
        } catch {
            throw DshRpcError(code: "network", message: "\(endpoint): \(error.localizedDescription)")
        }
        guard result["ok"].boolValue == true else {
            let err = result["error"]
            throw DshRpcError(
                code: err["code"].stringValue ?? "unknown",
                message: err["message"].stringValue ?? "dsh RPC failed"
            )
        }
        return result["value"]
    }

    /// 裸 GET(cookie 鉴权):宿主网页路由直连(如 /api/present.host)。
    func getRaw(_ path: String) async throws -> (Int, String) {
                guard let url = Self.apiURL(base, path) else {
            throw DshRpcError(code: "network", message: "bad url: \(path)")
        }
        var req = URLRequest(url: url)
        req.timeoutInterval = 30
        let (data, resp) = try await http.data(for: req)
        return ((resp as? HTTPURLResponse)?.statusCode ?? -1, String(data: data, encoding: .utf8) ?? "")
    }

    /// 裸 GET 二进制(cookie 鉴权):宿主网页路由直连(/api/session.export 会话导出 ZIP 流;
    /// getRaw 的 String 形态会破坏二进制,故独立成 Data 出入口)。
    func getBinary(_ path: String) async throws -> (status: Int, data: Data) {
        guard let url = Self.apiURL(base, path) else {
            throw DshRpcError(code: "network", message: "bad url: \(path)")
        }
        var req = URLRequest(url: url)
        req.timeoutInterval = 300 // 会话日志 ZIP 可能较大
        let (data, resp) = try await http.data(for: req)
        return ((resp as? HTTPURLResponse)?.statusCode ?? -1, data)
    }

    /// 裸 POST JSON(无 body = 空 octet):如 POST /api/present.open?sessionId=&seq=&index=&action=。
    func postRaw(_ path: String, body: JSON? = nil) async throws -> (Int, String) {
                guard let url = Self.apiURL(base, path) else {
            throw DshRpcError(code: "network", message: "bad url: \(path)")
        }
        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        if let body {
            req.setValue("application/json", forHTTPHeaderField: "Content-Type")
            req.httpBody = body.encoded()
        } else {
            req.setValue("application/octet-stream", forHTTPHeaderField: "Content-Type")
            req.httpBody = Data()
        }
        let (data, resp) = try await http.data(for: req)
        return ((resp as? HTTPURLResponse)?.statusCode ?? -1, String(data: data, encoding: .utf8) ?? "")
    }

    /// 附件上传:POST api/session/uploadFileBinary?sessionId=&name=,body=原始字节 → receiptId。
    func uploadBytes(url: URL, bytes: Data) async throws -> String? {
        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        req.setValue("application/octet-stream", forHTTPHeaderField: "Content-Type")
        req.httpBody = bytes
        let (data, resp) = try await http.data(for: req)
        guard let httpResp = resp as? HTTPURLResponse, (200..<300).contains(httpResp.statusCode) else {
            let status = (resp as? HTTPURLResponse)?.statusCode ?? -1
            throw DshRpcError(code: "http-\(status)", message: "uploadFileBinary: HTTP \(status)")
        }
        let root = try JSON.decode(data)
        if let rid = root["receiptId"].stringValue { return rid }
        if let value = root["result"]["value"].objectValue, let rid = value["receiptId"]?.stringValue { return rid }
        if let s = root["result"]["value"].stringValue { return s }
        return root.stringValue
    }

    // MARK: - mux 连接

    /// 只建 mux 连接,不订 $events(启动顺序契约:先触碰 workspace 域再订阅事件)。
    func connectMuxEventsDeferred() async throws {
        try await ensureMux()
    }

    /// 订阅 $events(幂等;mux 未建则先建)。
    func subscribeEvents() async throws {
        eventsSubscribed = true
        try await ensureMux()
    }

    private func connectSocket() async throws {
        // http(s)://host:port/ → ws(s)://host:port/api/remote.mux
        var wsString = base.absoluteString
        if wsString.hasSuffix("/") { wsString.removeLast() }
        wsString = wsString.replacingOccurrences(of: "^http", with: "ws", options: .regularExpression)
        wsString += "/api/remote.mux"
        var req = URLRequest(url: URL(string: wsString)!)
        let cookie = cookieHeader()
        if !cookie.isEmpty { req.setValue(cookie, forHTTPHeaderField: "Cookie") }
        let task = http.webSocketTask(with: req)
        task.resume()
        mux = task
        receiveTask = Task { [weak self] in
            await self?.receiveLoop(task)
        }
    }

    /// mux 重连 + 按需补订 $events;连接级恢复时通知业务流重开(gate 外)。
    private func ensureMux() async throws {
        guard !disposed else { throw DshRpcError(code: "disposed", message: "RPC client disposed") }
        var needResetNotice = false
        if mux == nil || mux?.state != .running {
            // 保留到恢复成功:首次重连失败后 mux 已为 nil,不能丢业务通知。
            resetPending = resetPending || (mux != nil)
            mux?.cancel(with: .goingAway, reason: nil)
            mux = nil
            streams.removeAll()
            eventsStreamId = nil
            eventsClientId = nil
            eventsReady.reset()
            try await connectSocket()
        }
        if eventsSubscribed && eventsStreamId == nil {
            try await openStreamRaw(endpoint: "$events", payload: .obj(("args", .object([:]))), sink: nil, end: nil)
        }
        needResetNotice = resetPending
        resetPending = false
        if needResetNotice {
            streamsResetBox.value?()
        }
    }

    // MARK: - 流管理

    /// 通用 mux 流:任意 mode=stream 端点。onFrame 收到的是结果值本身(网关不套 {ok,value} 信封)。
    func openStream(endpoint: String, args: JSON, onFrame: @escaping FrameSink, onEnd: StreamEnd? = nil) async throws -> String {
        try await ensureMux()
        return try await openStreamRaw(endpoint: endpoint, payload: .obj(("args", args)), sink: onFrame, end: onEnd)
    }

    /// session/follow:跟随会话 journal 增量流。
    func openFollowStream(sessionId: String, onFrame: @escaping FrameSink, onEnd: StreamEnd? = nil) async throws -> String {
        try await ensureMux()
        let payload: JSON = .obj(
            ("args", .obj(("request", .obj(("address", .obj(("kind", .string("session")), ("sessionId", .string(sessionId))))))))
        )
        return try await openStreamRaw(endpoint: "session/follow", payload: payload, sink: onFrame, end: onEnd)
    }

    /// workspace/follow:工作区树 baseline + 增量流。
    func openWorkspaceFollowStream(onFrame: @escaping FrameSink, onEnd: StreamEnd? = nil) async throws -> String {
        try await ensureMux()
        return try await openStreamRaw(endpoint: "workspace/follow", payload: .obj(("args", .object([:]))), sink: onFrame, end: onEnd)
    }

    private func openStreamRaw(endpoint: String, payload: JSON, sink: FrameSink?, end: StreamEnd?) async throws -> String {
        streamSeq += 1
        let id = "c2s\(streamSeq)"
        let open: JSON = .obj(
            ("type", .string("open")),
            ("streamId", .string(id)),
            ("endpoint", .string(endpoint)),
            ("payload", payload)
        )
        if endpoint == "$events" { eventsStreamId = id }
        if let sink { streams[id] = (sink, end) }
        do {
            try await sendText(open.encoded())
        } catch {
            streams.removeValue(forKey: id)
            if eventsStreamId == id { eventsStreamId = nil }
            throw error
        }
        return id
    }

    /// 取消一条已开的流(cancel 帧);不存在则忽略。
    func cancelStream(_ streamId: String) {
        streams.removeValue(forKey: streamId)
        guard let socket = mux, socket.state == .running else { return }
        let cancel: JSON = .obj(("type", .string("cancel")), ("streamId", .string(streamId)))
        Task { try? await self.sendText(cancel.encoded()) }
    }

    /// 发送队列:帧严格按序出站(actor 重入下 open→cancel 顺序不可乱)。
    private let sendQueue = SendQueue()

    private func sendText(_ text: Data) async throws {
        guard let socket = mux else {
            throw DshRpcError(code: "network", message: "事件连接尚未建立。")
        }
        try await sendQueue.enqueue { [socket] in
            try await socket.send(.data(text))
        }
    }

    // MARK: - 接收循环

    private func receiveLoop(_ socket: URLSessionWebSocketTask) async {
        while !disposed, socket.state == .running {
            let message: URLSessionWebSocketTask.Message
            do {
                message = try await socket.receive()
            } catch {
                break
            }
            switch message {
            case .string(let text):
                guard let frame = try? JSON.decode(text) else { continue }
                dispatch(frame)
            case .data:
                continue // 协议约定纯文本帧
            @unknown default:
                continue
            }
        }
        // 连接死亡:作废状态并启动自动重连(单循环 + 退避)
        if !disposed {
            mux = nil
            eventsClientId = nil
            eventsStreamId = nil
            eventsReady.reset()
            startReconnectLoop()
        }
    }

    private func startReconnectLoop() {
        guard !disposed, !reconnectRunning else { return }
        reconnectRunning = true
        Task { [weak self] in
            await self?.reconnectLoop()
        }
    }

    private func reconnectLoop() async {
        var delay = Self.reconnectInitialDelay
        while !disposed {
            do {
                try await ensureMux()
                reconnectRunning = false
                // 关闭"退出与再次断开"竞态窗口:退出瞬间连接又断则重新武装
                if !disposed, mux?.state != .running || (eventsSubscribed && eventsStreamId == nil) {
                    startReconnectLoop()
                }
                return
            } catch {
                if disposed { reconnectRunning = false; return }
                try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
                delay = min(delay * 2, Self.reconnectMaxDelay)
            }
        }
        reconnectRunning = false
    }

    // MARK: - 帧分发

    private func dispatch(_ frame: JSON) {
        let type = frame["type"].stringValue ?? ""
        let streamId = frame["streamId"].stringValue

        guard type == "item" else {
            if let streamId, type == "end" || type == "error" {
                let handler = streams.removeValue(forKey: streamId)
                if streamId == eventsStreamId {
                    eventsStreamId = nil
                    eventsClientId = nil
                    eventsReady.reset()
                    // 服务端终态 ≠ 传输断开:可能是永久授权/业务错误,停止自动订阅。
                    eventsSubscribed = false
                    if let cb = eventsStreamEndedBox.value {
                        cb(frame["error"].isNull ? nil : frame["error"].stringValue)
                    }
                }
                handler?.end?(type == "error" ? (frame["error"].stringValue ?? "stream error") : nil)
            }
            return
        }

        let value = frame["value"]

        // $events 流帧:ready / emit / cancel / waterfall
        if let streamId, streamId == eventsStreamId {
            let frameType = value["type"].stringValue
            if frameType == "ready" {
                eventsClientId = value["clientId"].stringValue
                if eventsClientId != nil { eventsReady.fire() }
                return
            }
            if frameType == "cancel" {
                if let eventId = value["eventId"].stringValue, let cb = eventCancelledBox.value {
                    cb(eventId)
                }
                return
            }
            if frameType == "emit", let eventName = value["event"].stringValue {
                // 载荷 = 完整 emit 帧 {type,event,args:[…位置参数]}
                for h in eventHandlers[eventName] ?? [] { h(value) }
                for h in eventHandlers["*"] ?? [] { h(value) }
            }
            if frameType == "waterfall", let eventName = value["event"].stringValue {
                // 内核等人回答:必须回传 $events/result 才解挂
                for h in waterfallHandlers[eventName] ?? [] { h(value) }
            }
            return
        }

        if let streamId, let handler = streams[streamId] {
            handler.sink(value)
        }
    }

    // MARK: - 事件订阅与 waterfall 回答

    /// 订阅内核 emit 事件(approval 撤销、api-session/*、commands/change…)。
    func onEvent(_ name: String, _ handler: @escaping FrameSink) {
        eventHandlers[name, default: []].append(handler)
    }

    /// 订阅 waterfall 事件(内核挂起等待回答)。
    func onWaterfall(_ name: String, _ handler: @escaping FrameSink) {
        waterfallHandlers[name, default: []].append(handler)
    }

    /// waterfall 回答:POST /api/$events/result {clientId, eventId, outcome:{kind:"result", value}}。
    /// value 形态由事件决定(审批 = "allowed-once"|"rejected";提问 = {answers:[…]});跳过 = {kind:"next"}。
    func resolveWaterfall(eventId: String, value: JSON) async throws {
        try await eventsReady.wait(seconds: 10)
        guard let clientId = eventsClientId else {
            throw DshRpcError(code: "events", message: "事件连接已失效,请等待重新连接后再回答。")
        }
        let outcome: JSON = .obj(("kind", .string("result")), ("value", value))
        try await callOk("$events/result", .obj(("clientId", .string(clientId)), ("eventId", .string(eventId)), ("outcome", outcome)))
    }

    func skipWaterfall(eventId: String) async throws {
        try await eventsReady.wait(seconds: 10)
        guard let clientId = eventsClientId else {
            throw DshRpcError(code: "events", message: "事件连接已失效,请等待重新连接后再回答。")
        }
        try await callOk("$events/result", .obj(
            ("clientId", .string(clientId)),
            ("eventId", .string(eventId)),
            ("outcome", .obj(("kind", .string("next"))))
        ))
    }

    /// 审批决定(outcome 值是裸字符串)。
    func resolveEvent(eventId: String, outcome: String) async throws {
        try await resolveWaterfall(eventId: eventId, value: .string(outcome))
    }
}

/// 简单可重置的一次/多次信号(对应 C# TaskCompletionSource 的 EventsReady)。
final class Signal: @unchecked Sendable {
    private let lock = NSLock()
    private var fired = false
    private var waiters: [UUID: CheckedContinuation<Void, Never>] = [:]

    func fire() {
        lock.lock()
        fired = true
        let conts = Array(waiters.values)
        waiters.removeAll()
        lock.unlock()
        conts.forEach { $0.resume() }
    }

    /// reset 后再 fire 才放行(mux 重连 / $events 终态时调用)。
    func reset() {
        lock.lock()
        fired = false
        lock.unlock()
    }

    /// 等待 fire 或超时。取消安全:任务被取消时摘除等待者。
    func wait(seconds: Double) async throws {
        lock.lock()
        if fired { lock.unlock(); return }
        let id = UUID()
        lock.unlock()
        defer {
            lock.lock(); waiters.removeValue(forKey: id); lock.unlock()
        }
        try await withThrowingTaskGroup(of: Void.self) { group in
            group.addTask {
                await withCheckedContinuation { (cont: CheckedContinuation<Void, Never>) in
                    self.lock.lock()
                    if self.fired {
                        self.lock.unlock()
                        cont.resume()
                        return
                    }
                    self.waiters[id] = cont
                    self.lock.unlock()
                }
            }
            group.addTask {
                try await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
                throw DshRpcError(code: "timeout", message: "等待 $events ready 超时")
            }
            try await group.next()
            group.cancelAll()
        }
    }
}

/// 严格串行的异步发送队列(对应 C# _sendGate 信号量)。
final class SendQueue: @unchecked Sendable {
    private let lock = NSLock()
    private var tail: Task<Void, Error>?

    func enqueue(_ op: @escaping @Sendable () async throws -> Void) async throws {
        lock.lock()
        let prev = tail
        let task = Task<Void, Error> {
            if let prev { try await prev.value }
            try await op()
        }
        tail = task
        lock.unlock()
        // 与 C# _sendGate 语义一致:自身失败抛出,但不阻塞后续帧入队
        try? await prev?.value
        do {
            try await task.value
        } catch is CancellationError {
            // 前序帧失败导致的连锁取消:静默
        } catch {
            throw error
        }
    }
}
