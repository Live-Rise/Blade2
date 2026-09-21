import AppKit // NSSavePanel(/export 会话导出选目标位置)
import UniformTypeIdentifiers // UTType.zip
import Foundation
import SwiftUI
import Observation

/// 单条搜索结果。
struct SearchHit: Identifiable, Equatable {
    let sessionId: String
    let snippet: String
    var id: String { sessionId }
}

/// 斜杠命令缓存条目(3s 负缓存)。
struct CommandCacheEntry {
    let commands: [SlashCommand]?
    let fetchedAt: Date
}

/// permissions 投影的选项(Win xaml.cs:10747-10753:{options:[{value,description?}],currentValue})。
struct PermissionPresetOption: Equatable {
    let value: String
    let description: String
}

/// Blade² macOS 编排层(对应 Win 版 MainWindow.xaml.cs 的 UI/RPC 编排职责)。
/// 一切 UI 变更都收敛在 MainActor;RPC 走 actor;协议契约见 DshRpcClient.swift。
@MainActor
@Observable
public final class AppState {
    public init() {}
    public enum Phase: Equatable {
        case idle
        case starting(String)
        case ready
        case failed(String)
    }

    // MARK: 生命周期

    public private(set) var phase: Phase = .idle
    private let kernel = DshKernelHost()
    private var rpc: DshRpcClient?
    /// 设置页等需要的直连通道(与 _rpc 等价;只读暴露)。
    var rpcClient: DshRpcClient? { rpc }

    // MARK: 数据面

    public private(set) var workspaces: [WorkspaceVm] = []
    var archivedSessionIds: Set<String> = []
    public private(set) var sessions: [SessionItem] = []
    public private(set) var catalog: ModelCatalog = .empty
    var runningBySession: [String: Bool] = [:]
    var busySinceBySession: [String: Date] = [:]

    // MARK: 活动会话与聊天

    var activeSessionId: String?
    public private(set) var bubbles: [ChatBubble] = []
    var heroVisible: Bool { bubbles.isEmpty && !isBusy }
    var isBusy: Bool { activeSessionId.map { runningBySession[$0] ?? false } ?? false }

    private var journalCursor: Int = -1
    private var sessionFollowGeneration = 0
    private var sessionStreamId: String?
    private var workspaceStreamId: String?
    private var controlStreamId: String?
    private var workspaceStreamOpen = false
    private var renderedAttachmentIds: Set<String> = []
    private var followGate = AsyncGate()
    private var businessStreamsResetPending = false
    private var restoringBusinessStreams = false
    private var sessionListTask: Task<Void, Never>?

    // MARK: 选择器状态(新会话)

    var newSessionWorkspaceId: String?
    var newSessionAgentPreset: String?
    var selectedProvider = "deepseek-official"
    var selectedModel = "deepseek-flash"
    var selectedEffort = ""
    /// 空态权限档位:settings 的 permission.defaultPreset(Win xaml.cs:11144,默认 workspace-write)。
    /// Win 没有"随 session/create 或 session/prompt 传权限"的 RPC 字段(xaml.cs:3195-3221 只带
    /// workspaceId/cwd/agentPreset;1384-1391 明示 plan/permissions 无对应 RPC),新会话按该默认档开局。
    var defaultPermissionPreset: String {
        settingsValue(ns: "permission", path: ["defaultPreset"]).stringValue ?? "workspace-write"
    }

    // MARK: 交互事件(审批 / 提问,单活跃 + 队列)

    private var approvalQueue: [ApprovalRequest] = []
    private var questionQueue: [QuestionRequest] = []
    private var seenInteractiveEventIds: Set<String> = []
    private var submittingApprovalEventId: String?
    private var submittingQuestionEventId: String?
    /// 当前展示的审批卡(单活跃)
    var activeApproval: ApprovalRequest? = nil
    var activeQuestion: QuestionRequest? = nil

    // MARK: 输入区

    var composerText = ""
    var pendingAttachments: [PendingAttachment] = []

    // MARK: 斜杠命令

    private var commandCache: [String: CommandCacheEntry] = [:]

    // MARK: 设置

    var settingsNamespaces: [SettingsNamespace] = []
    private var settingsMutateWork: [String: Task<Void, Never>] = [:]
    private var settingsPending: [String: [(path: [String], value: JSON)]] = [:]

    // 主题与对话行为:内核 0.7.7 起为独立命名空间(无 general;与 Win 同构读法
    // NsString,MainWindow.xaml.cs:4536/4579/1863)。通用页的字段行见 SettingsView。
    var themePreference: String { settingsValue(ns: "ui-theme", path: ["preference"]).stringValue ?? "system" }
    var themeFontSize: Int { settingsValue(ns: "ui-theme", path: ["fontSize"]).intValue ?? 14 }
    var kernelBusyEnter: String { settingsValue(ns: "ui-conversation", path: ["busyEnter"]).stringValue ?? "queue" }

    // MARK: 界面语言

    /// 语言切换计数器:ShellView 根与设置场景 `.id(app.uiLocaleVersion)` 依赖它整体重建
    /// 视图树重取翻译(对应 Win 版 RefreshShellLanguage 的动态重建,MainWindow.xaml.cs:1298-1312)。
    /// public:设置场景在 DshMacUI 模块声明,需跨模块建立依赖。
    public var uiLocaleVersion = 0

    /// 切换界面语言(设置页"界面语言"行):更新 L10n、触发视图重建,并把内核只认的 zh/en
    /// 回写 locale.preference(Win 版 xaml.cs:6435 `Edit("locale","preference",kernelLocale)`
    /// → settings/mutate;"跟随系统"按系统语言先解析成 zh/en)。
    func setUiLocale(_ option: String) async {
        guard option != L10n.storedOption else { return }
        L10n.currentLocale = option
        uiLocaleVersion += 1
        mutateSetting(ns: "locale", path: ["preference"], value: .string(L10n.kernelLocale))
    }

    // MARK: 消息反馈

    private var feedbackByMessage: [String: MessageFeedbackItem] = [:]

    // MARK: 搜索

    var searchQuery = ""
    var searchResults: [SearchHit] = []
    var searchKernelDisabled = false

    // MARK: 用量

    var usage: UsageStats?
    var usagePartial = false

    // MARK: 会话运行统计(聚合/折叠/格式化见 AppState+RunStats.swift;Win 契约 MainWindow.RunStats.cs)

    /// 运行状态条聚合器(renderEventCore 钩子逐事件喂入;键 = sessionId)。
    var runStats = RunStatsAggregator()
    /// transcript 紧凑折叠状态(轮起止 + 答案 seq;切会话清屏重置后随回放重建)。
    var transcriptFold = TranscriptFoldState()
    /// session/control 流 jobs 帧缓存(键 = sessionId;作业面板数据源,Win xaml.cs:11093)。
    var runJobs: [String: [KernelJob]] = [:]

    // MARK: - 启动(BootAsync,顺序为协议契约,不可调换)

    /// 失败重试入口(phase 由壳层重置)。
    public func retryBoot() {
        phase = .idle
        Task { await boot() }
    }

    public func boot() async {
        guard phase == .idle || phase.isFailed else { return }
        Self.log("boot: start")
        // 通知授权尽早请求(审批/启动失败提醒用;未决定才弹一次)
        UserAttention.requestAuthorization()
        phase = .starting(L("正在启动内核…"))
        do {
            let dshHome = DshKernelHost.appSupportRoot()
            try FileManager.default.createDirectory(at: dshHome, withIntermediateDirectories: true)
            let url = try await kernel.start(dshHome: dshHome)
            Self.log("boot: kernel url captured")
            let base = URL(string: url.absoluteString.split(separator: "?", maxSplits: 1).map(String.init)[0]) ?? url
            let client = DshRpcClient(base: base)
            rpc = client
            phase = .starting(L("正在连接…"))

            // 2. token 握手
            try await client.authenticate(tokenURL: url)
            Self.log("boot: authenticated")

            // 3. commands/change 事件使斜杠命令缓存失效
            await client.onEvent("commands/change") { [weak self] _ in
                Task { @MainActor in self?.commandCache.removeAll() }
            }

            // 4. 先建 mux、不订 $events(0.7.1 定论:先触碰 workspace 域)
            try await client.connectMuxEventsDeferred()

            // 5. 模型目录(不硬编码默认模型)
            let catalogValue = try await client.callOk("session/modelCatalog", .object([:]))
            catalog = ModelCatalog.parse(catalogValue, fallbackProvider: "deepseek-official", fallbackModel: "deepseek-flash")
            if let def = catalog.models.first(where: { $0.provider == catalog.defaultProvider && $0.id == catalog.defaultModel }) {
                selectedProvider = def.provider
                selectedModel = def.id
                selectedEffort = def.defaultEffort
            } else if let first = catalog.models.first {
                selectedProvider = first.provider
                selectedModel = first.id
                selectedEffort = first.defaultEffort
            }

            // 6. 事件处理器注册(必须在 $events 订阅前)
            await registerEventHandlers(client)

            // 7. 设置快照:先应用主题再显示 UI
            try await refreshSettingsSnapshot()

            // 8. workspace/follow(顺序承重)
            try await openWorkspaceFollowStream()

            // 9. $events 订阅
            try await client.subscribeEvents()

            // 10. 会话清单
            try await refreshSessions()

            // 11. session/control 流(最后开,避免扰动 workspace 初始化次序)
            openSessionControlStream()

            phase = .ready
            Self.log("boot: ready")
        } catch let err as DshRpcError {
            Self.log("boot: rpc error \(err.code) \(err.message)")
            phase = .failed(LF("{0}", err.message))
            UserAttention.notify(title: L("Blade² 内核启动失败"), body: err.message)
        } catch {
            Self.log("boot: error \(error)")
            phase = .failed(error.localizedDescription)
            UserAttention.notify(title: L("Blade² 内核启动失败"), body: error.localizedDescription)
        }
    }

    /// 轻量壳日志(诊断启动链路;对应 Win 版 %TEMP%\blade2_unhandled.txt 的定位)。
    nonisolated static func log(_ line: String) {
        let url = FileManager.default.urls(for: .libraryDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Logs/blade2-shell.log")
        let text = "\(Date().formatted(date: .abbreviated, time: .standard)) \(line)\n"
        if let fh = FileHandle(forWritingAtPath: url.path) {
            defer { try? fh.close() }
            fh.seekToEndOfFile()
            fh.write(Data(text.utf8))
        } else {
            try? Data(text.utf8).write(to: url)
        }
    }

    public func shutdown() {
        rpc?.dispose()
        rpc = nil
        kernel.stop()
        phase = .idle
    }

    private func registerEventHandlers(_ client: DshRpcClient) async {
        // 审批(waterfall:内核挂起等待回答)
        await client.onWaterfall("approval/request") { [weak self] frame in
            Task { @MainActor in self?.onApprovalRequest(frame) }
        }
        // 用户提问(waterfall)
        await client.onWaterfall("user-questions/request") { [weak self] frame in
            Task { @MainActor in self?.onQuestionsRequest(frame) }
        }
        // 会话增删 → 刷新清单
        await client.onEvent("api-session/added") { [weak self] _ in
            Task { @MainActor in self?.refreshAllLists() }
        }
        await client.onEvent("api-session/removed") { [weak self] _ in
            Task { @MainActor in self?.refreshAllLists() }
        }
        // 会话运行态:args[0]=sessionId, args[1]=running
        await client.onEvent("api-session/status") { [weak self] frame in
            Task { @MainActor in
                guard let sid = frame["args"][0].stringValue else { return }
                let running = frame["args"][1].boolValue ?? false
                self?.setSessionRunning(sid, running: running)
            }
        }
        // 内核撤销挂起事件
        client.eventCancelledBox.value = { [weak self] eventId in
            Task { @MainActor in self?.onInteractiveEventCancelled(eventId) }
        }
        // 连接级重连 → 业务流重开
        client.streamsResetBox.value = { [weak self] in
            Task { @MainActor in self?.onStreamsReset() }
        }
    }

    // MARK: - 业务流恢复

    private func onStreamsReset() {
        businessStreamsResetPending = true
        workspaceStreamOpen = false
        sessionStreamId = nil
        controlStreamId = nil
        sessionFollowGeneration += 1
        Task { await restoreBusinessStreams() }
    }

    private func restoreBusinessStreams() async {
        guard !restoringBusinessStreams else { return }
        restoringBusinessStreams = true
        defer { restoringBusinessStreams = false }
        while businessStreamsResetPending {
            businessStreamsResetPending = false
            try? await refreshWorkspaces()
            if let sid = activeSessionId {
                try? await followSession(sid)
            }
            openSessionControlStream()
        }
    }

    // MARK: - 工作区(workspace/follow 树)

    private func openWorkspaceFollowStream() async throws {
        guard let rpc else { return }
        workspaceStreamId = try await rpc.openWorkspaceFollowStream(
            onFrame: { [weak self] value in Task { @MainActor in self?.applyWorkspaceFrame(value) } },
            onEnd: { [weak self] _ in Task { @MainActor in self?.workspaceStreamOpen = false } }
        )
        workspaceStreamOpen = true
    }

    func refreshWorkspaces() async throws {
        guard let rpc else { return }
        if !workspaceStreamOpen {
            try await openWorkspaceFollowStream()
        }
        await refreshSessions()
    }

    private func applyWorkspaceFrame(_ value: JSON) {
        switch value["type"].stringValue {
        case "baseline":
            var list: [WorkspaceVm] = []
            for item in value["value"]["items"].arrayValue ?? [] {
                list.append(WorkspaceVm(
                    workspaceId: item["workspaceId"].stringValue ?? "",
                    title: item["title"].stringValue ?? "",
                    path: item["path"].stringValue ?? "",
                    sessionIds: (item["sessionIds"].arrayValue ?? []).compactMap { $0.stringValue }
                ))
            }
            workspaces = list
            archivedSessionIds = Set((value["value"]["archivedSessionIds"].arrayValue ?? []).compactMap { $0.stringValue })
        case "upsert":
            let ws = value["workspace"]
            let vm = WorkspaceVm(
                workspaceId: ws["workspaceId"].stringValue ?? "",
                title: ws["title"].stringValue ?? "",
                path: ws["path"].stringValue ?? "",
                sessionIds: (ws["sessionIds"].arrayValue ?? []).compactMap { $0.stringValue }
            )
            if let idx = workspaces.firstIndex(where: { $0.workspaceId == vm.workspaceId }) {
                workspaces[idx] = vm
            } else {
                workspaces.append(vm)
            }
        case "remove":
            if let wid = value["workspaceId"].stringValue {
                workspaces.removeAll { $0.workspaceId == wid }
            }
        case "order":
            let order = (value["workspaceIds"].arrayValue ?? []).compactMap { $0.stringValue }
            let dict = Dictionary(uniqueKeysWithValues: workspaces.map { ($0.workspaceId, $0) })
            var ordered: [WorkspaceVm] = []
            for id in order { if let ws = dict[id] { ordered.append(ws) } }
            for ws in workspaces where !order.contains(ws.workspaceId) { ordered.append(ws) }
            workspaces = ordered
        case "archived":
            archivedSessionIds = Set((value["archivedSessionIds"].arrayValue ?? []).compactMap { $0.stringValue })
        default:
            break
        }
    }

    // MARK: - 会话清单

    /// session/list 的参数键是 _request(0.7.0 探针实测,request 会 arguments-invalid)。
    func refreshSessions() async {
        guard let rpc else { return }
        let value = try? await rpc.callOk("session/list", .obj(("_request", .object([:]))))
        guard let value else { return }
        var list: [SessionItem] = []
        for item in value["items"].arrayValue ?? [] {
            let sid = item["sessionId"].stringValue ?? ""
            guard !sid.isEmpty else { continue }
            let updatedAt = item["updatedAt"].doubleValue.map { Date(timeIntervalSince1970: $0 / 1000) }
            list.append(SessionItem(
                sessionId: sid,
                cwd: item["cwd"].stringValue ?? "",
                blank: item["blank"].boolValue ?? false,
                updatedAt: updatedAt,
                projections: item["projections"],
                running: runningBySession[sid] ?? false,
                busySince: busySinceBySession[sid]
            ))
        }
        sessions = list
    }

    func refreshAllLists() {
        Task { [weak self] in
            guard let self else { return }
            try? await self.refreshWorkspaces()
            await self.refreshSessions()
        }
    }

    func setSessionRunning(_ sid: String, running: Bool) {
        runningBySession[sid] = running
        busySinceBySession[sid] = running ? (busySinceBySession[sid] ?? Date()) : nil
        if let idx = sessions.firstIndex(where: { $0.sessionId == sid }) {
            sessions[idx].running = running
            sessions[idx].busySince = busySinceBySession[sid]
        }
        if sid == activeSessionId {
            // 运行态变化经由 isBusy 计算属性自动反映;此处无需手动通知
        }
    }

    // MARK: - 会话打开 / 历史(二分头游标 + 回向分页)

    /// 新会话草稿态:清空聊天面(对应 Win 版 NewSessionItem;发送时才真正建会话)。
    func startNewSessionDraft() {
        activeSessionId = nil
        sessionFollowGeneration += 1
        bubbles = []
        journalCursor = -1
        renderedAttachmentIds = []
        composerText = ""
        pendingAttachments = []
        newSessionWorkspaceId = nil
        newSessionAgentPreset = nil
        transcriptFold = TranscriptFoldState() // 折叠轮次随清屏重置(Win xaml.cs:1577-1586)
    }

    func openSession(_ sessionId: String) async {
        guard sessionId != activeSessionId || bubbles.isEmpty else { return }
        activeSessionId = sessionId
        sessionFollowGeneration += 1
        bubbles = []
        journalCursor = -1
        renderedAttachmentIds = []
        feedbackByMessage = [:]
        // 运行状态条/紧凑折叠:清零后由随后的完整 transcript 回放重建,避免翻页重复累计
        // (Win xaml.cs:3124 ResetRunStats + 1577-1586 清屏重置折叠状态)
        runStats.reset(sessionId)
        transcriptFold = TranscriptFoldState()

        guard let rpc else { return }
        // 历史:二分 0..65536 探头("past cursor" ⇒ hi=mid-1),再回向翻页,old→new 渲染
        var events: [JournalEvent] = []
        do {
            events = try await loadSessionHistory(sessionId, rpc: rpc)
        } catch {
            appendSystemMessage(LF("会话历史加载失败:{0}", (error as? DshRpcError)?.message ?? error.localizedDescription))
        }
        // 标题兜底:日志里最新的 session/title(旧会话 projections 缺失时)
        if let titleEvent = events.last(where: { $0.type == "session/title" }),
           let title = titleEvent.data["title"].stringValue, !title.isEmpty {
            applyKernelTitle(sessionId, title)
        }
        for ev in events {
            renderEventCore(ev)
        }
        try? await followSession(sessionId)
        Task { await loadFeedback(sessionId) }
    }

    private func loadSessionHistory(_ sessionId: String, rpc: DshRpcClient) async throws -> [JournalEvent] {
        var lo = 0
        var hi = 1 << 16
        while lo < hi {
            let mid = lo + ((hi - lo + 1) >> 1)
            do {
                _ = try await rpc.callOk("session/page", pageArgs(sessionId, throughSeq: mid))
                lo = mid
            } catch let err as DshRpcError where err.isPastCursor {
                hi = mid - 1
            }
        }

        var pages: [[JournalEvent]] = []
        var through = lo
        while through >= 0 {
            let page: JSON
            do {
                page = try await rpc.callOk("session/page", pageArgs(sessionId, throughSeq: through))
            } catch let err as DshRpcError where err.isPastCursor {
                break
            }
            let records = page["records"].arrayValue ?? []
            guard !records.isEmpty else { break }
            let batch = records.compactMap { rec -> JournalEvent? in
                guard let ev = rec["event"].objectValue else { return nil }
                return JournalEvent(
                    seq: rec["event"]["seq"].intValue ?? 0,
                    time: rec["event"]["time"].doubleValue,
                    type: rec["event"]["type"].stringValue ?? "",
                    data: rec["event"]["data"]
                )
            }
            pages.append(batch)
            let firstSeq = records[0]["event"]["seq"].intValue ?? 0
            if page["hasMore"].boolValue == true && firstSeq > 0 {
                through = firstSeq - 1
            } else {
                break
            }
        }
        // pages 新→旧收集,倒回成 seq 升序
        var events: [JournalEvent] = []
        for i in stride(from: pages.count - 1, through: 0, by: -1) {
            events.append(contentsOf: pages[i])
        }
        return events
    }

    private func pageArgs(_ sessionId: String, throughSeq: Int, maxMessages: Int? = nil) -> JSON {
        var request: [(String, JSON)] = [
            ("address", .obj(("kind", .string("session")), ("sessionId", .string(sessionId)))),
            ("throughSeq", .int(throughSeq)),
        ]
        if let maxMessages { request.append(("maxMessages", .int(maxMessages))) }
        return .obj(("request", .dict(request)))
    }

    /// session/follow:跟随增量流(重开时 generation++ 使旧帧失效)。
    private func followSession(_ sessionId: String) async throws {
        guard let rpc else { return }
        let generation = sessionFollowGeneration
        sessionStreamId = try await rpc.openFollowStream(
            sessionId: sessionId,
            onFrame: { [weak self] frame in
                Task { @MainActor in
                    await self?.applyFollowFrame(sessionId, generation, frame)
                }
            },
            onEnd: { [weak self] error in
                Task { @MainActor in
                    if self?.sessionStreamId != nil, sessionId == self?.activeSessionId {
                        // 业务终态(如会话已删除):不自动重试,给出提示
                        if let error { self?.appendSystemMessage(LF("会话流结束:{0}", error)) }
                    }
                }
            }
        )
    }

    /// follow 帧处理:页帧(records)+ hasMore 回向补页;单事件帧;游标去重。
    private func applyFollowFrame(_ sessionId: String, _ generation: Int, _ frame: JSON) async {
        await followGate.run {
            let current = generation == self.sessionFollowGeneration && sessionId == self.activeSessionId
            guard current, let rpc = self.rpc else { return }

            if let records = frame["records"].arrayValue, !records.isEmpty {
                var pending: [JournalEvent] = records.compactMap { rec in
                    guard let ev = rec["event"].objectValue else { return nil }
                    return JournalEvent(
                        seq: rec["event"]["seq"].intValue ?? 0,
                        time: rec["event"]["time"].doubleValue,
                        type: rec["event"]["type"].stringValue ?? "",
                        data: rec["event"]["data"]
                    )
                }
                var page = frame
                while page["hasMore"].boolValue == true && !pending.isEmpty {
                    let first = pending.map(\.seq).min() ?? 0
                    if first <= self.journalCursor + 1 { break }
                    guard let earlierPage = try? await rpc.callOk("session/page", self.pageArgs(sessionId, throughSeq: first - 1)) else { break }
                    guard generation == self.sessionFollowGeneration && sessionId == self.activeSessionId else { return }
                    let earlier = (earlierPage["records"].arrayValue ?? []).compactMap { rec -> JournalEvent? in
                        guard let ev = rec["event"].objectValue else { return nil }
                        return JournalEvent(
                            seq: rec["event"]["seq"].intValue ?? 0,
                            time: rec["event"]["time"].doubleValue,
                            type: rec["event"]["type"].stringValue ?? "",
                            data: rec["event"]["data"]
                        )
                    }
                    if earlier.isEmpty || (earlier.map(\.seq).min() ?? Int.max) >= first {
                        self.appendSystemMessage(L("会话历史分页未前进。"))
                        break
                    }
                    pending.append(contentsOf: earlier)
                    page = earlierPage
                }
                for ev in pending.sorted(by: { $0.seq < $1.seq }) {
                    guard ev.seq > self.journalCursor else { continue }
                    self.renderEventCore(ev)
                    self.journalCursor = ev.seq
                }
            } else if frame["event"]["seq"].intValue.map({ $0 > self.journalCursor }) == true, frame["event"]["type"].stringValue != nil {
                let ev = JournalEvent(
                    seq: frame["event"]["seq"].intValue ?? 0,
                    time: frame["event"]["time"].doubleValue,
                    type: frame["event"]["type"].stringValue ?? "",
                    data: frame["event"]["data"]
                )
                self.renderEventCore(ev)
                self.journalCursor = ev.seq
                await self.refreshSessions()
            }
        }
    }

    // MARK: - journal → 气泡(RenderEventCore)

    private func renderEventCore(_ ev: JournalEvent) {
        let data = ev.data
        var turn = data["turn"].intValue ?? 0
        var bubble: ChatBubble? = nil

        // 统计/折叠钩子(逻辑在 AppState+RunStats.swift;Win RenderEventCore xaml.cs:3858-3859):
        // page 回放 + follow 实时流 + 补拉页都经此,历史与实时天然全覆盖
        trackRunStats(ev)
        transcriptTrack(ev)

        switch ev.type {
        case "turn/start", "step/start", "turn/end":
            return // 书签事件不产生气泡(v1 采用完整 transcript 模式)

        case "user/message":
            // 跳过内核注入消息:source.kind != "user"
            let sourceKind = data["source"]["kind"].stringValue ?? ""
            if !sourceKind.isEmpty && sourceKind != "user" { return }
            let content = data["content"]
            var texts: [String] = []
            for block in content.arrayValue ?? [] where block["type"].stringValue == "text" {
                if let t = block["text"].stringValue { texts.append(t) }
            }
            let text = texts.joined(separator: "\n")
            if !text.isEmpty {
                bubble = ChatBubble(role: .user, text: text, seq: ev.seq)
            }
            // 图片块:journal 只存 attachmentId,字节经 session/attachment 回读
            if let sid = activeSessionId {
                for block in content.arrayValue ?? [] where block["type"].stringValue == "image" {
                    if let attachmentId = block["attachment"]["attachmentId"].stringValue, !attachmentId.isEmpty {
                        let name = block["attachment"]["name"].stringValue ?? L("图片")
                        Task { await renderAttachmentImage(sid, attachmentId, name) }
                    }
                }
            }

        case "assistant/attempt":
            // 流式尝试的错误块(finish{reason.kind:"error"})要用户可见
            for piece in data["stream"].arrayValue ?? [] {
                let chunk = piece["chunk"]
                if chunk["type"].stringValue == "finish" {
                    let reason = chunk["reason"]
                    if reason["kind"].stringValue == "error" {
                        let message = reason["failure"]["message"].stringValue ?? L("内核执行失败")
                        let code = reason["failure"]["code"].stringValue ?? ""
                        bubble = ChatBubble(role: .error, text: code.isEmpty ? "⚠ \(message)" : "⚠ \(code): \(message)", seq: ev.seq)
                    }
                }
            }

        case "assistant/message":
            let msg = data["message"]
            if msg["role"].stringValue == "assistant" {
                var reasoning = ""
                var text = ""
                for block in msg["content"].arrayValue ?? [] {
                    switch block["type"].stringValue {
                    case "reasoning": reasoning += block["text"].stringValue ?? ""
                    case "text": text += block["text"].stringValue ?? ""
                    default: break
                    }
                }
                if !reasoning.isEmpty {
                    appendBubble(ChatBubble(role: .reasoning, text: reasoning, turn: turn, isReasoning: true, seq: ev.seq))
                }
                if !text.isEmpty {
                    bubble = ChatBubble(
                        role: .assistant, text: text, turn: turn,
                        messageId: msg["id"].stringValue, seq: ev.seq
                    )
                }
            }

        case "tool/invoke":
            if let name = data["name"].stringValue {
                let detail = name == "bash" ? LF("⚙ {0}(命令执行)", name) : "⚙ \(name)"
                bubble = ChatBubble(role: .tool, text: detail, seq: ev.seq)
            }

        case "session/title":
            if let title = data["title"].stringValue, !title.isEmpty {
                applyKernelTitle(activeSessionId, title)
            }

        case "deliverables/presented":
            // files 数组下标即 present.open 的内核坐标,按下标原样保留
            var files: [PresentedFile] = []
            let all = data["files"].arrayValue ?? []
            for (index, f) in all.enumerated() {
                if let path = f["path"].stringValue, !path.trimmingCharacters(in: .whitespaces).isEmpty {
                    files.append(PresentedFile(
                        index: index, path: path,
                        description: f["description"].stringValue ?? "",
                        sessionId: activeSessionId ?? "", seq: ev.seq
                    ))
                }
            }
            if !files.isEmpty {
                bubble = ChatBubble(
                    role: .deliverable,
                    text: LF("交付物 seq={0}:{1}", String(ev.seq), files.map(\.path).joined(separator: "、")),
                    files: files, seq: ev.seq
                )
            }

        default:
            return
        }

        guard var finalBubble = bubble else { return }
        finalBubble.turn = ev.type == "user/message" ? 0 : turn
        appendBubble(finalBubble)
    }

    /// 相邻去重(同角色同文本同轮) + 400 条上限(对应 C# AppendBubble)。
    private func appendBubble(_ bubble: ChatBubble) {
        if let previous = bubbles.last,
           previous.role == bubble.role, previous.text == bubble.text,
           previous.turn == bubble.turn, previous.isReasoning == bubble.isReasoning {
            return
        }
        bubbles.append(bubble)
        if bubbles.count > 400 {
            bubbles.removeFirst(bubbles.count - 400)
        }
    }

    private func appendSystemMessage(_ text: String) {
        bubbles.append(ChatBubble(role: .system, text: text))
    }

    /// 历史回放里的图片:session/attachment 回读 base64 → user-image 气泡(attachmentId 去重,≤16MB)。
    private func renderAttachmentImage(_ sessionId: String, _ attachmentId: String, _ name: String) async {
        guard !renderedAttachmentIds.contains(attachmentId), let rpc else { return }
        renderedAttachmentIds.insert(attachmentId)
        do {
            let value = try await rpc.callOk(
                "session/attachment",
                .obj(("request", .obj(("sessionId", .string(sessionId)), ("attachmentId", .string(attachmentId)))))
            )
            guard let b64 = value["data"].stringValue,
                  let data = Data(base64Encoded: b64), data.count <= 16 * 1024 * 1024 else { return }
            let mediaType = value["attachment"]["mediaType"].stringValue ?? "image/png"
            bubbles.append(ChatBubble(role: .userImage, text: name, imageData: data, imageMediaType: mediaType))
        } catch {
            // 回读失败静默(与 Win 版一致:不阻塞主渲染)
        }
    }

    private func applyKernelTitle(_ sessionId: String?, _ title: String) {
        guard let sessionId else { return }
        if let idx = sessions.firstIndex(where: { $0.sessionId == sessionId }) {
            sessions[idx].durableTitleOverride = title
        }
    }

    // MARK: - 发送(队列 / 追问)

    /// 发送消息。无活动会话时先创建;busy 时按 busyEnter 或显式模式选 queue/steer。
    public func send(text: String, mode: String? = nil) async {
        Self.log("send: enter phase=\(String(describing: phase)) rpc=\(rpc != nil) sid=\(activeSessionId ?? "nil")")
        guard let rpc, phase == .ready else { return }
        var body = text.trimmingCharacters(in: .whitespacesAndNewlines)
        let attachments = pendingAttachments
        guard !body.isEmpty || !attachments.isEmpty else { return }

        // 斜杠命令路由:前导 /word 命中 commands/list → commands/execute
        if body.hasPrefix("/") {
            let word = body.split(separator: " ", maxSplits: 1).dropFirst().first.map(String.init) ?? ""
            let commands = await commandsFor(activeSessionId ?? "")
            if commands.contains(where: { $0.name == String(word.dropFirst()) }) {
                await executeSlashCommand(line: body)
                return
            }
        }

        var sid = activeSessionId
        do {
            // 无会话先建:workspaceId 优先,回退 cwd(用户文稿)
            if sid == nil {
                var request: [String: JSON] = [:]
                if let wid = newSessionWorkspaceId {
                    request["workspaceId"] = .string(wid)
                } else {
                    let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
                    request["cwd"] = .string(documents.path)
                }
                if let preset = newSessionAgentPreset { request["agentPreset"] = .string(preset) }
                let created = try await rpc.callOk("session/create", .obj(("request", .dict(request))))
                sid = created["sessionId"].stringValue
                guard let sid else { return }
                await refreshSessions()
            }
            guard let sid else { return }
            if activeSessionId != sid {
                await openSession(sid)
            }

            // 模型选择(失败忽略,内核用会话当前模型)
            try? await rpc.callOk("session/selectModel", .obj(
                ("sessionId", .string(sid)),
                ("provider", .string(selectedProvider)),
                ("model", .string(selectedModel)),
                ("reasoningEffort", .string(selectedEffort))
            ))

            // content:文本 + 图片内联 base64 + 文件 receiptId
            var content: [JSON] = []
            if !body.isEmpty { content.append(.obj(("type", .string("text")), ("text", .string(body)))) }
            for att in attachments {
                if att.isImage {
                    content.append(.obj(
                        ("type", .string("image")),
                        ("mediaType", .string(att.mediaType)),
                        ("data", .string(att.data.base64EncodedString())),
                        ("name", .string(att.name))
                    ))
                } else {
                    let receipt = try? await uploadAttachment(sid, att)
                    if let receipt {
                        content.append(.obj(("type", .string("file")), ("receiptId", .string(receipt))))
                    }
                }
            }

            Self.log("send: prompt sid=\(sid) content=\(content.count)")
            let sendMode = mode ?? (isBusy ? kernelBusyEnter : "queue")
            try await rpc.callOk("session/prompt", .obj(
                ("request", .obj(
                    ("requestId", .string("c2-\(UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased())")),
                    ("sessionId", .string(sid)),
                    ("mode", .string(sendMode)),
                    ("content", .array(content))
                ))
            ))
            pendingAttachments = []
            composerText = ""
        } catch let err as DshRpcError {
            appendSystemMessage(LF("发送失败:{0}", err.message))
            return
        } catch {
            return
        }

        guard let sid else { return }
        // 重开 follow + 兜底页拉(follow 流可能未及时送达;through 过大会 "past cursor",从 64 对半收缩)+ 清单刷新
        try? await followSession(sid)
        var through = 64
        while through >= 0 {
            do {
                let page = try await rpc.callOk("session/page", pageArgs(sid, throughSeq: through, maxMessages: 64))
                renderJournalPage(page)
                break
            } catch let err as DshRpcError where err.isPastCursor {
                through = through >= 8 ? through / 2 : 0
                continue
            } catch let err as DshRpcError {
                Self.log("send: page failed \(err.code) \(err.message)")
                break
            } catch {
                Self.log("send: page failed non-rpc")
                break
            }
        }
        await refreshSessions()
        await pollReply(sid)
    }

    /// 发送后轮询补拉:22×4s,through=64 对半收缩("past cursor"),单页成功即止。
    private func pollReply(_ sid: String) async {
        guard let rpc else { return }
        for _ in 0..<22 {
            guard activeSessionId == sid else { return }
            try? await Task.sleep(nanoseconds: 4_000_000_000)
            guard activeSessionId == sid else { return }
            var through = 64
            while through >= 0 {
                do {
                    let page = try await rpc.callOk("session/page", pageArgs(sid, throughSeq: through, maxMessages: 64))
                    renderJournalPage(page)
                    break
                } catch let err as DshRpcError where err.isPastCursor {
                    through = through >= 8 ? through / 2 : 0
                } catch {
                    return
                }
            }
        }
    }

    /// 契约自测入口:注入合成 journal 页并按正常管线渲染(无头 E2E 用)。
    public func debugRenderPage(_ page: JSON) {
        renderJournalPage(page)
    }

    /// 页帧渲染(游标去重)。
    private func renderJournalPage(_ page: JSON) {
        let records = (page["records"].arrayValue ?? []).compactMap { rec -> JournalEvent? in

            guard rec["event"]["type"].stringValue != nil else { return nil }
            return JournalEvent(
                seq: rec["event"]["seq"].intValue ?? 0,
                time: rec["event"]["time"].doubleValue,
                type: rec["event"]["type"].stringValue ?? "",
                data: rec["event"]["data"]
            )
        }
        for ev in records.sorted(by: { $0.seq < $1.seq }) {
            guard ev.seq > journalCursor else { continue }
            renderEventCore(ev)
            journalCursor = ev.seq
        }
    }

    func cancelActive() async {
        guard let sid = activeSessionId, let rpc else { return }
        // 参数键是 request(_request 会 arguments-invalid)
        try? await rpc.callOk("session/cancel", .obj(("request", .obj(("sessionId", .string(sid))))))
    }

    // MARK: - 会话投影(plan / permissions;对应 Win xaml.cs:1384-1395)

    /// session/control 流的 plan/permissions 投影覆盖(键 sid → key → value)。
    /// Win 只消费这两键(xaml.cs:10651-10656);按 sid 存,切会话无需清理
    /// (基线读 session/list 快照,见 planState/sessionPermission 的读序)。
    private var projectionOverrides: [String: [String: JSON]] = [:]

    /// 活动会话 plan 投影 {active, pending}(Win xaml.cs:10735-10745,dsh-plan-mode)。
    /// 读序:session/control 增量覆盖 > session/list 快照(Win xaml.cs:10785-10795 两源同构复用)。
    var planState: (active: Bool, pending: Bool)? {
        guard let sid = activeSessionId else { return nil }
        let view = projectionOverrides[sid]?["plan"]
            ?? sessions.first(where: { $0.sessionId == sid })?.projections["values"]["plan"]
            ?? .null
        guard view.objectValue != nil else { return nil }
        return (view["active"].boolValue == true, view["pending"].boolValue == true)
    }

    /// 活动会话 permissions 投影(选项 + 当前值;Win xaml.cs:10747-10753)。
    var sessionPermission: (options: [PermissionPresetOption], current: String?)? {
        guard let sid = activeSessionId else { return nil }
        let view = projectionOverrides[sid]?["permissions"]
            ?? sessions.first(where: { $0.sessionId == sid })?.projections["values"]["permissions"]
            ?? .null
        guard view.objectValue != nil else { return nil }
        let options = (view["options"].arrayValue ?? []).compactMap { option -> PermissionPresetOption? in
            guard let value = option["value"].stringValue else { return nil }
            return PermissionPresetOption(value: value, description: option["description"].stringValue ?? "")
        }
        return (options, view["currentValue"].stringValue)
    }

    /// session/control 流帧 → plan/permissions 投影(Win xaml.cs:10618-10732 OnControlFrame 的对应部分:
    /// projection 增量 {sessionId,key,value,seq} 10691-10721;baseline 快照
    /// {value:{projections:{<sid>:{asOfSeq,values:{…}}}}} 10626-10666)。
    /// jobs 帧(作业面板数据源)见 AppState+RunStats.swift 的 applyJobsFrame/applyJobsBaseline;
    /// queues 等其余帧与壳无关,不消费。
    func applyControlProjectionFrame(_ frame: JSON) {
        switch frame["type"].stringValue ?? "" {
        case "projection":
            guard let sid = frame["sessionId"].stringValue,
                  let key = frame["key"].stringValue,
                  key == "plan" || key == "permissions" else { return }
            projectionOverrides[sid, default: [:]][key] = frame["value"]
        case "baseline":
            // jobs 快照 {value:{jobs:{<sid>:[job…]}}}:整表替换(Win xaml.cs:10826-10832)
            applyJobsBaseline(frame["value"]["jobs"])
            // 快照是权威基线:整表替换(本覆盖表只存 plan/permissions 两键)
            var rebuilt: [String: [String: JSON]] = [:]
            for (sid, block) in frame["value"]["projections"].objectValue ?? [:] {
                for key in ["plan", "permissions"] where block["values"][key].objectValue != nil {
                    rebuilt[sid, default: [:]][key] = block["values"][key]
                }
            }
            projectionOverrides = rebuilt
        case "jobs":
            // jobs 增量 {sessionId, jobs:[…]},数组缺失 = 清空(Win xaml.cs:10862-10871)
            if let sid = frame["sessionId"].stringValue {
                applyJobsFrame(sid, frame["jobs"])
            }
        default:
            break
        }
    }

    // MARK: 会话投影命令(plan / permissions;写入只有命令,内核无对应 RPC —— Win xaml.cs:1384-1391)

    /// 会话态权限切换:/permission <preset>(Win xaml.cs:11385-11397 OnPermissionPresetClick)。
    /// 命令在会话(agent)作用域内执行(Win ExecuteCommandAsync 12856-12861 要求活动会话)。
    func setSessionPermissionPreset(_ preset: String) async {
        guard activeSessionId != nil else { return }
        await executeSlashCommand(line: "/permission \(preset)")
    }

    /// 计划模式切换:/plan 进入、/plan off 退出(Win xaml.cs:11369-11383 OnPlanToggleClick,
    /// 按投影 active 取向;命令回执经 commands/execute,投影随后到达刷新 chip)。
    func togglePlanMode() async {
        guard activeSessionId != nil else { return }
        await executeSlashCommand(line: planState?.active == true ? "/plan off" : "/plan")
    }

    // MARK: - 附件上传

    private func uploadAttachment(_ sessionId: String, _ att: PendingAttachment) async throws -> String? {
        guard let rpc else { return nil }
        var components = URLComponents(url: rpc.base, resolvingAgainstBaseURL: false)!
        components.path = "/api/session/uploadFileBinary"
        components.queryItems = [
            URLQueryItem(name: "sessionId", value: sessionId),
            URLQueryItem(name: "name", value: att.name),
        ]
        return try await rpc.uploadBytes(url: components.url!, bytes: att.data)
    }

    // MARK: - 审批(approval/request waterfall)

    private func onApprovalRequest(_ frame: JSON) {
        let eventId = frame["eventId"].stringValue ?? ""
        let request = frame["request"]
        guard !eventId.isEmpty, request.objectValue != nil else { return }
        let toolName = request["toolName"].stringValue ?? ""
        let reason = request["reason"].stringValue ?? ""
        guard seenInteractiveEventIds.insert(eventId).inserted else { return }
        approvalQueue.append(ApprovalRequest(
            eventId: eventId, agentId: frame["agentId"].stringValue ?? "",
            toolName: toolName.isEmpty ? L("(工具)") : toolName, reason: reason
        ))
        showNextApproval()
    }

    private func showNextApproval() {
        guard activeApproval == nil, !approvalQueue.isEmpty else { return }
        activeApproval = approvalQueue.removeFirst()
        submittingApprovalEventId = nil
        // 窗口不在前台 → 系统通知 + Dock 徽标(Win 版 xaml.cs:4666-4668 同场景)
        if NSApp?.isActive != true, let request = activeApproval {
            UserAttention.notify(title: L("Blade² 请求许可"), body: LF("{0}:{1}", request.toolName, request.reason))
        }
        refreshDockBadge()
    }

    /// Dock 徽标随挂起审批/前台状态刷新(app 回到前台时由壳层didBecomeActive 再次调用)。
    func refreshDockBadge() {
        UserAttention.updateDockBadge(pending: activeApproval != nil, appActive: NSApp?.isActive ?? true)
    }

    func answerApproval(_ request: ApprovalRequest, allowed: Bool) async {
        guard let rpc, submittingApprovalEventId != request.eventId else { return }
        submittingApprovalEventId = request.eventId
        defer { submittingApprovalEventId = nil }
        do {
            try await rpc.resolveEvent(eventId: request.eventId, outcome: allowed ? "allowed-once" : "rejected")
            seenInteractiveEventIds.insert(request.eventId)
            if activeApproval?.eventId == request.eventId {
                activeApproval = nil
                showNextApproval()
            }
            refreshDockBadge()
        } catch let err as DshRpcError {
            // 失败保留输入允许重试:卡片不关,错误就地显示
            approvalSubmitError = err.message
        } catch {}
    }

    var approvalSubmitError: String?

    // MARK: - 用户提问(user-questions/request waterfall)

    private func onQuestionsRequest(_ frame: JSON) {
        let eventId = frame["eventId"].stringValue ?? ""
        let request = frame["request"]
        guard !eventId.isEmpty, request.objectValue != nil else { return }
        guard seenInteractiveEventIds.insert(eventId).inserted else { return }
        var questions: [QuestionItem] = []
        for q in request["questions"].arrayValue ?? [] {
            let options = (q["options"].arrayValue ?? []).map { o in
                QuestionOption(label: o["label"].stringValue ?? "", description: o["description"].stringValue ?? "")
            }
            questions.append(QuestionItem(
                id: q["id"].stringValue ?? "",
                question: q["question"].stringValue ?? "",
                header: q["header"].stringValue ?? "",
                options: options,
                multiSelect: q["multiSelect"].boolValue ?? false,
                intentKind: q["intent"]["kind"].stringValue ?? "",
                detail: q["detail"].stringValue ?? ""
            ))
        }
        guard !questions.isEmpty else { return }
        questionQueue.append(QuestionRequest(eventId: eventId, agentId: frame["agentId"].stringValue ?? "", questions: questions))
        showNextQuestion()
    }

    private func showNextQuestion() {
        guard activeQuestion == nil, !questionQueue.isEmpty else { return }
        activeQuestion = questionQueue.removeFirst()
        submittingQuestionEventId = nil
    }

    /// 提交回答:answers = [{id, selected:[option label…], custom?}](selected 是标签数组)。
    func answerQuestions(_ request: QuestionRequest, answers: [(id: String, selected: [String], custom: String?)]) async {
        guard let rpc, submittingQuestionEventId != request.eventId else { return }
        submittingQuestionEventId = request.eventId
        defer { submittingQuestionEventId = nil }
        var answerArray: [JSON] = []
        for a in answers {
            var fields: [(String, JSON)] = [
                ("id", .string(a.id)),
                ("selected", .array(a.selected.map { .string($0) })),
            ]
            if let custom = a.custom, !custom.isEmpty {
                fields.append(("custom", .string(custom)))
            }
            answerArray.append(.dict(fields))
        }
        do {
            try await rpc.resolveWaterfall(eventId: request.eventId, value: .obj(("answers", .array(answerArray))))
            seenInteractiveEventIds.insert(request.eventId)
            if activeQuestion?.eventId == request.eventId {
                activeQuestion = nil
                showNextQuestion()
            }
        } catch let err as DshRpcError {
            questionSubmitError = err.message
        } catch {}
    }

    var questionSubmitError: String?

    func skipQuestion(_ request: QuestionRequest) async {
        guard let rpc else { return }
        try? await rpc.skipWaterfall(eventId: request.eventId)
        if activeQuestion?.eventId == request.eventId {
            activeQuestion = nil
            showNextQuestion()
        }
    }

    private func onInteractiveEventCancelled(_ eventId: String) {
        seenInteractiveEventIds.insert(eventId)
        if activeApproval?.eventId == eventId { activeApproval = nil; showNextApproval() }
        approvalQueue.removeAll { $0.eventId == eventId }
        if activeQuestion?.eventId == eventId { activeQuestion = nil; showNextQuestion() }
        questionQueue.removeAll { $0.eventId == eventId }
        refreshDockBadge()
    }

    // MARK: - 会话 CRUD

    func renameSession(_ sid: String, title: String) async {
        guard let rpc else { return }
        try? await rpc.callOk("session/rename", .obj(("request", .obj(("sessionId", .string(sid)), ("title", .string(title))))))
        if let idx = sessions.firstIndex(where: { $0.sessionId == sid }) {
            sessions[idx].durableTitleOverride = title
        }
    }

    func forkSession(_ sid: String) async {
        guard let rpc else { return }
        // 参数键是 request(_request 会 arguments-invalid)
        try? await rpc.callOk("session/fork", .obj(("request", .obj(("sessionId", .string(sid))))))
        await refreshSessions()
    }

    func archiveSession(_ sid: String) async {
        guard let rpc else { return }
        try? await rpc.callOk("workspace/archiveSession", .obj(("request", .obj(("sessionId", .string(sid))))))
        await refreshSessions()
    }

    func moveSession(_ sid: String, toWorkspace workspaceId: String) async {
        guard let rpc else { return }
        try? await rpc.callOk("workspace/insertSessionBefore", .obj(("request", .obj(("workspaceId", .string(workspaceId)), ("sessionId", .string(sid))))))
        await refreshSessions()
    }

    func createWorkspace(path: String) async {
        guard let rpc, !path.isEmpty else { return }
        // 严格 zod:必须包 request(裸 {path} 拒收)
        try? await rpc.callOk("workspace/create", .obj(("request", .obj(("path", .string(path))))))
        await refreshSessions()
    }

    func renameWorkspace(_ wid: String, title: String) async {
        guard let rpc else { return }
        try? await rpc.callOk("workspace/rename", .obj(("request", .obj(("workspaceId", .string(wid)), ("title", .string(title))))))
    }

    func deleteWorkspace(_ wid: String) async {
        guard let rpc else { return }
        try? await rpc.callOk("workspace/delete", .obj(("request", .obj(("workspaceId", .string(wid))))))
    }

    /// 打开会话工作目录(内核侧打开 Finder)。契约(内核 dsh-api-session-controller/lib/index.js:2864-2901
    /// 与 Win xaml.cs:2796-2830 一致):canOpenWorkspacePath 无参 → bool;openWorkspacePath
    /// request{path, action?},缺省 action 走 openPath(进入目录;reveal 是在父级选中)。
    func openWorkspacePath(_ sid: String) async {
        guard let rpc, let item = sessions.first(where: { $0.sessionId == sid }) else { return }
        var can = false
        if let result = try? await rpc.callOk("session/canOpenWorkspacePath", .object([:])) {
            can = result.boolValue ?? false
        }
        guard can else {
            appendSystemMessage(L("当前部署不支持打开工作目录。"))
            return
        }
        _ = try? await rpc.callOk("session/openWorkspacePath", .obj(
            ("request", .obj(("path", .string(item.cwd))))
        ))
    }

    /// 交付物打开 / 定位(present.open 宿主路由,files 下标即坐标)。
    func openPresentedFile(_ file: PresentedFile, reveal: Bool) async {
        guard let rpc else { return }
        let action = reveal ? "reveal" : "open"
        let path = "api/present.open?sessionId=\(file.sessionId)&seq=\(file.seq)&index=\(file.index)&action=\(action)"
        _ = try? await rpc.postRaw(path)
    }

    // MARK: - 斜杠命令

    /// commands/list(会话域;gateway/lookup-not-found 3s 负缓存)。
    func commandsFor(_ agentId: String) async -> [SlashCommand] {
        guard let rpc else { return [] }
        if let cached = commandCache[agentId], Date().timeIntervalSince(cached.fetchedAt) < 3 {
            return cached.commands ?? []
        }
        do {
            let value = try await rpc.callOk("commands/list", .obj(("agentId", .string(agentId))))
            let commands = (value.arrayValue ?? []).map { c in
                SlashCommand(
                    name: c["name"].stringValue ?? "",
                    description: c["description"].stringValue ?? "",
                    inputHint: c["input"]["hint"].stringValue ?? ""
                )
            }
            commandCache[agentId] = CommandCacheEntry(commands: commands, fetchedAt: Date())
            return commands
        } catch {
            commandCache[agentId] = CommandCacheEntry(commands: nil, fetchedAt: Date())
            return []
        }
    }

    /// commands/execute:业务层自带信封,参数为平铺 {agentId, line, submittedAttachments}。
    /// 返回值可能是 undefined(未识别/格式错误的行);成功/失败以 result.kind|text 呈现为系统消息
    /// (Win ExecuteCommandAsync xaml.cs:12822-12937;含壳表面命令拦截,见函数内注释)。
    private func executeSlashCommand(line: String) async {
        // 命令在会话(agent)作用域内执行,无活动会话先提示(Win xaml.cs:12827-12830)
        guard let sid = activeSessionId, !sid.isEmpty else {
            appendSystemMessage(L("请先选择一个会话：命令在会话（agent）作用域内执行。"))
            return
        }
        // 壳表面命令拦截(两个 Mac 原生面,行为出处见各函数注释):
        // - /export:内核命令是 Web-only(命令回执仅 "Session log download requested.",
        //   dsh-session-log-export),真实 ZIP 由 GET /api/session.export 路由流式给出;
        //   Mac 壳补全该流程(NSSavePanel + 下载落盘),只拦无参数形态,带参仍交内核报错。
        // - 裸 /feedback:内核本体要求文本参数(dsh-command-feedback:空文本报错),Mac 接到
        //   会话反馈表单(RPC 与 Win 表单完全一致);带文本的 /feedback 仍走内核原始文本反馈。
        let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed == "/export" {
            if composerText == trimmed { composerText = "" } // 同 Win:执行后清空未再改动的输入框
            await exportSessionLog()
            return
        }
        if trimmed == "/feedback" {
            if composerText == trimmed { composerText = "" }
            openSessionFeedbackSheet()
            return
        }
        guard let rpc else { return }
        let agentId = sid
        var submitted: [JSON] = []
        for att in pendingAttachments {
            if att.isImage {
                submitted.append(.obj(
                    ("type", .string("image")),
                    ("mediaType", .string(att.mediaType)),
                    ("data", .string(att.data.base64EncodedString())),
                    ("name", .string(att.name))
                ))
            } else if let receipt = try? await uploadAttachment(agentId, att) {
                submitted.append(.obj(("type", .string("file")), ("receiptId", .string(receipt))))
            }
        }
        do {
            let value = try await rpc.call("commands/execute", .obj(
                ("agentId", .string(agentId)),
                ("line", .string(line)),
                ("submittedAttachments", .array(submitted))
            ))
            let result = value["result"]
            pendingAttachments = []
            if result["kind"].stringValue == "error", let text = result["text"].stringValue, !text.isEmpty {
                appendSystemMessage(text)
            }
            // 执行后清空输入框(未被用户再改动时;Win xaml.cs:12929 `if (InputBox.Text == submittedText)`)
            if composerText == trimmed { composerText = "" }
        } catch let err as DshRpcError {
            appendSystemMessage(LF("命令执行失败:{0}", err.message))
        } catch {}
        await refreshSessions()
    }

    // MARK: 斜杠补全候选(浮层数据源;Win ShowCommandPaletteAsync xaml.cs:12673-12719)

    /// 壳内置命令表(与内核 host 命令同名同义,即 Win HostCommandDescriptions 的 6 条,
    /// xaml.cs:12351-12361;文案取官方中文侧键,中文即键)。**只作内核 commands/list 缺项时的
    /// 兜底**——同名命令冲突以内核为准(send 的 commands/list 路由与浮层合并规则均如此)。
    /// hasInput/hint 取内核注册处字面量:dsh-command-goal(lib/index.js:175-186)、
    /// dsh-plan-mode(:181-190)、dsh-client-ui-permission-presets(:479);compact/export 无参数。
    static let builtInSlashCommands: [SlashSuggestion] = [
        SlashSuggestion(name: "compact", description: "压缩以上对话内容", hint: "", hasInput: false, isBuiltIn: true),
        SlashSuggestion(name: "export", description: "将当前会话内容导出为 ZIP", hint: "", hasInput: false, isBuiltIn: true),
        SlashSuggestion(name: "feedback", description: "发送关于当前会话的反馈", hint: "", hasInput: false, isBuiltIn: true),
        SlashSuggestion(name: "goal", description: "设置或查看长期任务目标", hint: "[<objective>|clear|edit <objective>|pause|resume]", hasInput: true, isBuiltIn: true),
        SlashSuggestion(name: "permission", description: "切换权限预设（沙箱模式与审批策略）", hint: "<preset>", hasInput: true, isBuiltIn: true),
        SlashSuggestion(name: "plan", description: "进入或退出计划模式", hint: "[off|message]", hasInput: true, isBuiltIn: true),
    ]

    /// 内核内置命令描述的官方英文原文:仅当内核返回的描述**逐字等于**官方英文原文时才替换为
    /// 官方中文,第三方/作用域命令的描述原样保留(同 Win LocalizeHostCommand xaml.cs:12342-12349)。
    private static let hostCommandDescriptions: [String: (en: String, zh: String)] = [
        "compact": ("Compact older conversation history", "压缩以上对话内容"),
        "export": ("Download this Session log as a ZIP archive", "将当前会话内容导出为 ZIP"),
        "feedback": ("record feedback about this session", "发送关于当前会话的反馈"),
        "goal": ("set or view the goal for a long-running task", "设置或查看长期任务目标"),
        "permission": ("Switch the permission preset (sandbox mode + approval policy)", "切换权限预设（沙箱模式与审批策略）"),
        "plan": ("Enter or leave plan mode", "进入或退出计划模式"),
    ]

    private static func localizeHostCommand(_ name: String, _ description: String) -> String {
        guard let pair = hostCommandDescriptions[name], description == pair.en else { return description }
        return L(pair.zh)
    }

    /// 补全浮层候选:内核目录(commandsFor,3s 缓存)+ 壳内置兜底(只列内核没有的名字),
    /// 匹配与排序同 Win ShowCommandPaletteAsync(xaml.cs:12690-12696):Contains → 前缀优先 →
    /// 字典序,取 20。注:Mac SlashCommand 只有 inputHint 字符串,hasInput 以 hint 非空近似
    /// (内核 input 对象存在但 hint 为空的形态在现有目录中不存在)。例外:feedback 在 Mac 壳
    /// 接到会话反馈表单(见 runSlashSuggestion),即使内核目录给出 hint 也不按"补名输入"处理;
    /// 要发原文文本反馈可直接输入 "/feedback <文本>" 提交(仍走内核)。
    func slashSuggestions(query: String, sessionId: String?) async -> [SlashSuggestion] {
        let kernel = await commandsFor(sessionId ?? "")
        let kernelNames = Set(kernel.map(\.name))
        var merged = kernel.map { command in
            SlashSuggestion(
                name: command.name,
                description: Self.localizeHostCommand(command.name, command.description),
                hint: command.inputHint,
                hasInput: !command.inputHint.isEmpty && command.name != "feedback",
                isBuiltIn: false
            )
        }
        for builtin in Self.builtInSlashCommands where !kernelNames.contains(builtin.name) {
            merged.append(builtin)
        }
        let lowered = query.lowercased()
        return Array(merged
            .filter { $0.name.lowercased().contains(lowered) }
            .sorted { a, b in
                let aPrefix = a.name.lowercased().hasPrefix(lowered)
                let bPrefix = b.name.lowercased().hasPrefix(lowered)
                if aPrefix != bPrefix { return aPrefix }
                return a.name.lowercased() < b.name.lowercased()
            }
            .prefix(20))
    }

    /// 补全浮层采纳入口(无参数命令):内核命令走 commands/execute(与 send 路由同路径),
    /// 壳内置命令按各自实现派发;有参数命令由浮层只补全命令名,提交时经 send 的内核目录路由执行。
    func runSlashSuggestion(_ name: String) async {
        switch name {
        case "export":
            await exportSessionLog() // Mac 原生:见 exportSessionLog 注释
        case "feedback":
            openSessionFeedbackSheet() // Mac 原生:会话反馈表单
        default:
            // compact/goal/permission/plan 与内核命令同构(permissions/plan 会话态已有
            // setSessionPermissionPreset/togglePlanMode,同样落在本路径,不另造 RPC)
            await executeSlashCommand(line: "/\(name)")
        }
    }

    // MARK: /export 会话导出(Mac 原生面)

    /// /export:会话导出 ZIP。Win 对应实现为 Web-only(dsh-session-log-export):命令回执仅
    /// "Session log download requested.",真实 ZIP 由内核 GET /api/session.export
    /// ?sessionId=<id>&includeDescendants=true|false 流式给出(content-disposition 文件名为
    /// dsh-session-<sid>.zip,lib/index.js:224-227),Windows 壳没有导出 UI(xaml.cs 无 export 实现)。
    /// Mac 按同一路由补全壳面:NSSavePanel 选目标位置 → 下载落盘 → 系统消息回报路径。
    /// includeDescendants=true 连同分叉子会话一并导出(路由支持 true/false,取全量语义)。
    func exportSessionLog() async {
        guard let rpc, let sid = activeSessionId, !sid.isEmpty else {
            appendSystemMessage(L("请先选择一个会话：命令在会话（agent）作用域内执行。"))
            return
        }
        let panel = NSSavePanel()
        // 文件名按内核 sessionLogZipFilename 命名(dsh-session-<sid>.zip)
        panel.nameFieldStringValue = "dsh-session-\(sid).zip"
        panel.allowedContentTypes = [.zip]
        panel.canCreateDirectories = true
        guard panel.runModal() == .OK, let target = panel.url else { return }
        do {
            let (status, data) = try await rpc.getBinary(
                "api/session.export?sessionId=\(sid)&includeDescendants=true")
            switch status {
            case 200:
                try data.write(to: target)
                appendSystemMessage(LF("会话已导出：{0}", target.path))
            case 404:
                // 内核路由 "session not found"(Win 同文案,xaml.cs:12222)
                appendSystemMessage(L("会话不存在或已归档"))
            default:
                appendSystemMessage(LF("导出失败：HTTP {0}", String(status)))
            }
        } catch let err as DshRpcError {
            appendSystemMessage(LF("会话导出失败：{0}", err.message))
        } catch {
            appendSystemMessage(LF("会话导出失败：{0}", error.localizedDescription))
        }
    }

    // MARK: /feedback 会话反馈(Mac 原生面;Win OnSessionFeedbackClick xaml.cs:11402-11489)

    /// 会话反馈表单可见性(/feedback 命令触发;表单与提交在 ComposerView.SessionFeedbackSheet)。
    var sessionFeedbackSheetVisible = false

    /// 打开会话反馈表单。Win 的入口是顶栏"会话反馈"按钮;Mac 壳把裸 /feedback 命令接到同一表单,
    /// 表单提交的 RPC 与 Win 完全一致(sessionFeedback/record,见 submitSessionFeedback)。
    func openSessionFeedbackSheet() {
        guard activeSessionId != nil else {
            appendSystemMessage(L("请先选择一个会话：会话反馈作用在具体会话上。"))
            return
        }
        sessionFeedbackSheetVisible = true
    }

    /// 会话反馈提交:sessionFeedback/record,request{ sessionId, text?, category? }(Win xaml.cs:11457-11481)。
    /// text/category 都是 optional(非 nullable)——空值必须**不带键**,带 null 会被内核 zod 边界拒。
    /// 回执 { ok:true, value:{ recorded:true } } 或 { ok:false, error:{ code:"session-not-found" } };
    /// 语义只是往内核会话日志追加一条 feedback/record 事件,不做网络上报(dsh-command-feedback)。
    func submitSessionFeedback(category: String?, text: String) async {
        guard let rpc, let sid = activeSessionId else { return }
        var request: [(String, JSON)] = [("sessionId", .string(sid))]
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty { request.append(("text", .string(trimmed))) }
        if let category, !category.isEmpty { request.append(("category", .string(category))) }
        do {
            // 双信封:RPC result 本身是 {ok, value|error}(与 messageFeedback/* 同形态)
            let result = try await rpc.call("sessionFeedback/record", .obj(("request", .dict(request))))
            if result["ok"].boolValue == true, result["value"]["recorded"].boolValue == true {
                appendSystemMessage(L("会话反馈已记录（内核会话日志追加 feedback/record 事件）。"))
            } else {
                let code = result["error"]["code"].stringValue ?? result["value"]["error"]["code"].stringValue ?? ""
                appendSystemMessage(code == "session-not-found"
                    ? L("记录失败：该会话在内核里已不存在（session-not-found）。")
                    : LF("记录失败：内核返回 {0}。", code))
            }
        } catch let err as DshRpcError {
            appendSystemMessage(LF("会话反馈失败：{0}", err.message))
        } catch {}
    }

    // MARK: - 设置(settings/describe + mutate 去抖)

    @discardableResult
    func refreshSettingsSnapshot() async throws -> [SettingsNamespace] {
        guard let rpc else { return [] }
        let value = try await rpc.callOk("settings/describe", .object([:]))
        var list: [SettingsNamespace] = []
        for ns in value["namespaces"].arrayValue ?? [] {
            guard let name = ns["ns"].stringValue else { continue }
            list.append(SettingsNamespace(
                ns: name,
                revision: ns["revision"].intValue ?? 0,
                value: ns["value"],
                schema: ns["schema"]
            ))
        }
        settingsNamespaces = list
        return list
    }

    func settingsValue(ns: String, path: [String]) -> JSON {
        guard let snapshot = settingsNamespaces.first(where: { $0.ns == ns }) else { return .null }
        var node = snapshot.value
        for key in path { node = node[key] }
        return node
    }

    /// settings/mutate:按 ns 分组去抖 400ms;成功后快照失效重拉。
    func mutateSetting(ns: String, path: [String], value: JSON) {
        settingsPending[ns, default: []].append((path, value))
        settingsMutateWork[ns]?.cancel()
        let task = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 400_000_000)
            guard !Task.isCancelled else { return }
            await self?.flushMutations(ns: ns)
        }
        settingsMutateWork[ns] = task
    }

    private func flushMutations(ns: String) async {
        guard let rpc else { return }
        let ops = settingsPending[ns] ?? []
        settingsPending[ns] = nil
        guard !ops.isEmpty else { return }
        let opArray = ops.map { op -> JSON in
            .obj(("op", .string("set")), ("path", .array(op.path.map { .string($0) })), ("value", op.value))
        }
        do {
            try await rpc.callOk("settings/mutate", .obj(("ns", .string(ns)), ("ops", .array(opArray))))
            try await refreshSettingsSnapshot()
        } catch let err as DshRpcError {
            appendSystemMessage(LF("设置保存失败:{0}", err.message))
        } catch {}
    }

    /// settings/replace:重置一节为默认(空 section = 清空用户覆盖)。
    func replaceSetting(ns: String, section: JSON = .object([:])) async {
        guard let rpc else { return }
        var args: [String: JSON] = ["ns": .string(ns), "section": section]
        if let snapshot = settingsNamespaces.first(where: { $0.ns == ns }) {
            // expectedRevision 是 union([undefined, number]):已知时带上,null 会被拒
            args["expectedRevision"] = .int(snapshot.revision)
        }
        try? await rpc.callOk("settings/replace", .dict(args))
        try? await refreshSettingsSnapshot()
    }

    /// settings/update:agent-presets 默认预设等 patch 场景。
    func updateSettings(ns: String, patch: [String: JSON]) async {
        guard let rpc else { return }
        try? await rpc.callOk("settings/update", .obj(("ns", .string(ns)), ("patch", .dict(patch))))
        try? await refreshSettingsSnapshot()
    }

    // MARK: - 消息反馈(messageFeedback/*)

    private func loadFeedback(_ sessionId: String) async {
        guard let rpc else { return }
        // 双信封:RPC result 本身是 {ok, value|error}(内核 dsh-command-feedback)
        guard let result = try? await rpc.call("messageFeedback/list", .obj(("request", .obj(("sessionId", .string(sessionId)))))) else { return }
        guard result["ok"].boolValue == true else { return }
        for item in result["value"]["items"].arrayValue ?? [] {
            if let mid = item["messageId"].stringValue {
                feedbackByMessage[mid] = MessageFeedbackItem(
                    messageId: mid,
                    rating: item["rating"].stringValue ?? "",
                    note: item["note"].stringValue ?? "",
                    version: item["version"].stringValue ?? ""
                )
            }
        }
    }

    /// 当前消息的反馈评级(气泡 👍/👎 状态)。
    func feedbackRating(messageId: String?) -> String {
        guard let messageId else { return "" }
        return feedbackByMessage[messageId]?.rating ?? ""
    }

    /// 当前消息已提交的说明(气泡"添加/编辑说明"入口的文案与初值)。
    func feedbackNote(messageId: String?) -> String {
        guard let messageId else { return "" }
        return feedbackByMessage[messageId]?.note ?? ""
    }

    /// 赞/踩:同一评价再点一次 = 撤销(Win 版 xaml.cs:11937 RateAsync:current.Rating == rating
    /// → DeleteFeedbackAsync),否则 put 改判并保留已写说明(xaml.cs:11954)。
    func rateMessage(_ messageId: String, rating: String) async {
        let current = feedbackByMessage[messageId]
        if current?.rating == rating {
            await deleteFeedback(messageId)
            return
        }
        // put 是整体替换,不带 note 键会丢掉已写的说明
        await putFeedback(messageId, rating: rating, note: current?.note)
    }

    /// messageFeedback/put:request{ sessionId, messageId, rating, note?, ifVersion }。
    /// note 在内核是 optional(非 nullable)——没有说明时必须**不带键**,带 null 会被 zod 拒
    /// (Win 版 xaml.cs:11958 PutFeedbackAsync);version-conflict 时采纳 error.current(内核
    /// 权威值,null = 已被撤销)重试一次。
    private func putFeedback(_ messageId: String, rating: String, note: String?) async {
        guard let rpc, let sid = activeSessionId else { return }
        for attempt in 0..<2 {
            let current = feedbackByMessage[messageId]
            var request: [String: JSON] = [
                "sessionId": .string(sid),
                "messageId": .string(messageId),
                "rating": .string(rating),
                "ifVersion": .string(current?.version ?? ""),
            ]
            if let note, !note.isEmpty { request["note"] = .string(note) }
            guard let result = try? await rpc.call("messageFeedback/put", .obj(("request", .dict(request)))) else { return }
            if result["ok"].boolValue == true {
                // 回执 value 即完整条目(rating/note/version),按 Win 版 StoreFeedbackItem 原样入账
                let value = result["value"]
                if let version = value["version"].stringValue {
                    feedbackByMessage[messageId] = MessageFeedbackItem(
                        messageId: messageId,
                        rating: value["rating"].stringValue ?? rating,
                        note: value["note"].stringValue ?? "",
                        version: version
                    )
                }
                return
            }
            guard result["error"]["code"].stringValue == "version-conflict", attempt == 0 else { return }
            if let cur = result["error"]["current"].objectValue {
                feedbackByMessage[messageId] = MessageFeedbackItem(
                    messageId: messageId,
                    rating: cur["rating"]?.stringValue ?? "",
                    note: cur["note"]?.stringValue ?? "",
                    version: cur["version"]?.stringValue ?? ""
                )
            } else {
                feedbackByMessage.removeValue(forKey: messageId)
                return
            }
        }
    }

    /// 撤销评价:messageFeedback/delete,request{ sessionId, messageId, ifVersion }。
    /// 契约要点(Win 版 xaml.cs:12021 DeleteFeedbackAsync):delete 的 ifVersion 是 required
    /// **string**——与 put 的 union(null, string) 不同,传 null 会被内核拒成 gateway/input-invalid;
    /// 本地没有观察到的版本时先 list 对齐,内核侧也没有就直接本地撤销(幂等)。
    private func deleteFeedback(_ messageId: String) async {
        guard let rpc, let sid = activeSessionId else { return }
        if (feedbackByMessage[messageId]?.version ?? "").isEmpty {
            await loadFeedback(sid)
            if (feedbackByMessage[messageId]?.version ?? "").isEmpty {
                feedbackByMessage.removeValue(forKey: messageId)
                return
            }
        }
        for attempt in 0..<2 {
            guard let version = feedbackByMessage[messageId]?.version, !version.isEmpty else { return } // 已被撤销:本地状态已对齐
            let request: [String: JSON] = [
                "sessionId": .string(sid),
                "messageId": .string(messageId),
                "ifVersion": .string(version),
            ]
            guard let result = try? await rpc.call("messageFeedback/delete", .obj(("request", .dict(request)))) else { return }
            if result["ok"].boolValue == true {
                feedbackByMessage.removeValue(forKey: messageId)
                return
            }
            guard result["error"]["code"].stringValue == "version-conflict", attempt == 0 else { return }
            // error.current 是内核权威值(null = 已被别人撤销);采纳后重试一次(Win 版 AdoptFeedbackConflict)
            if let cur = result["error"]["current"].objectValue {
                feedbackByMessage[messageId] = MessageFeedbackItem(
                    messageId: messageId,
                    rating: cur["rating"]?.stringValue ?? "",
                    note: cur["note"]?.stringValue ?? "",
                    version: cur["version"]?.stringValue ?? ""
                )
            } else {
                feedbackByMessage.removeValue(forKey: messageId)
                return
            }
        }
    }

    /// 添加/编辑说明:put 同评级 + note,清空说明 = 不带 note 键的整体替换
    /// (Win 版 xaml.cs:12113 EditFeedbackNoteAsync:put 同评级 + note;空白说明不回传)。
    func saveFeedbackNote(_ messageId: String, note: String) async {
        guard let current = feedbackByMessage[messageId], !current.rating.isEmpty else { return }
        let trimmed = note.trimmingCharacters(in: .whitespacesAndNewlines)
        await putFeedback(messageId, rating: current.rating, note: trimmed.isEmpty ? nil : trimmed)
    }

    // MARK: - 搜索

    func performSearch(_ query: String) async {
        guard let rpc, !query.isEmpty else {
            searchResults = []
            return
        }
        do {
            let value = try await rpc.callOk("session/search", .obj(("request", .obj(("query", .string(query))))))
            searchResults = (value["items"].arrayValue ?? []).compactMap { item in
                guard let sid = item["sessionId"].stringValue else { return nil }
                return SearchHit(sessionId: sid, snippet: item["snippet"].stringValue ?? "")
            }
        } catch let err as DshRpcError {
            // 内核禁用搜索 → 回退本地标题匹配
            if err.message.contains("session search is disabled") || err.message.contains("SESSION_QUERY_SEARCH_DISABLED") {
                searchKernelDisabled = true
                let q = query.lowercased()
                searchResults = sessions.filter { $0.durableTitleOverride?.lowercased().contains(q) == true || $0.displayTitle(fallbackBlank: L("新会话")).lowercased().contains(q) }
                    .map { SearchHit(sessionId: $0.sessionId, snippet: $0.displayTitle(fallbackBlank: L("新会话"))) }
            }
        } catch {}
    }

    // MARK: - 会话控制流(session/control;queues/jobs 不接,plan/permissions 投影见上方专属区)

    private func openSessionControlStream() {
        guard let rpc, controlStreamId == nil else { return }
        Task { [weak self] in
            guard let self, let rpc = self.rpc else { return }
            // 最后开,避免扰动 workspace 域初始化次序(0.7.1 定论)
            self.controlStreamId = try? await rpc.openStream(
                endpoint: "session/control",
                args: .object([:]),
                onFrame: { [weak self] value in
                    // plan/permissions 投影增量消费(Win xaml.cs:10691-10721);
                    // jobs 帧接给作业面板(AppState+RunStats.swift);
                    // queues 等其余帧不接,保持原"调度面不进壳"的占位语义
                    Task { @MainActor in self?.applyControlProjectionFrame(value) }
                }
            )
        }
    }

    // MARK: - 用量统计(壳侧计算:session/list + session/page 回向翻页 ≤40 页)

    func computeUsage() async {
        guard let rpc else { return }
        var summary = UsageStats()
        let list = (try? await rpc.callOk("session/list", .obj(("_request", .object([:]))))) ?? .null
        var perModelDict: [String: Int] = [:]
        var perDayDict: [String: Int] = [:]
        var perDayModelDict: [String: [String: Int]] = [:] // 日 → 模型 → token(多模型多色趋势矩阵)
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"

        for item in list["items"].arrayValue ?? [] {
            let sid = item["sessionId"].stringValue ?? ""
            let blank = item["blank"].boolValue ?? false
            guard !sid.isEmpty, !blank else { continue }
            guard let asOfSeq = item["projections"]["asOfSeq"].intValue else {
                summary.sessionsSkipped += 1 // 无游标:无法定位分页起点(Win:9913)
                continue
            }
            summary.sessionsCount += 1

            // 回向翻页最多 40 页
            var through = asOfSeq
            var pages = 0
            var turnStart: Date?
            var sessionTalkSeconds = 0 // 本会话各 turn 时长之和(「最长聊天时长」的口径,Win:10126)
            while through >= 0 && pages < 40 {
                pages += 1
                guard let page = try? await rpc.callOk("session/page", pageArgs(sid, throughSeq: through, maxMessages: 500)) else { break }
                let records = page["records"].arrayValue ?? []
                guard !records.isEmpty else { break }
                for rec in records {
                    let ev = rec["event"]
                    let time = ev["time"].doubleValue.map { Date(timeIntervalSince1970: $0 / 1000) }
                    switch ev["type"].stringValue {
                    case "turn/start":
                        turnStart = time
                    case "turn/end":
                        if let start = turnStart, let end = time {
                            sessionTalkSeconds += max(0, Int(end.timeIntervalSince(start)))
                        }
                        turnStart = nil
                    case "assistant/message":
                        let usageJson = ev["data"]["usage"]
                        let tokens = usageJson["totalTokens"].intValue
                            ?? (usageJson["inputTokens"].intValue.map { $0 + (usageJson["outputTokens"].intValue ?? 0) + (usageJson["cacheReadTokens"].intValue ?? 0) + (usageJson["cacheWriteTokens"].intValue ?? 0) })
                        if let tokens, tokens > 0 {
                            summary.usageMessages += 1
                            let model = ev["data"]["message"]["source"]["model"].stringValue ?? ""
                            let provider = ev["data"]["message"]["source"]["provider"].stringValue ?? ""
                            // 标注口径同 Win:10042-10051:无名记「未标注模型」,有模型无服务商只记模型名
                            let label = model.isEmpty
                                ? L("未标注模型")
                                : (provider.isEmpty ? model : "\(provider)/\(model)")
                            perModelDict[label, default: 0] += tokens
                            if let time {
                                let day = formatter.string(from: time)
                                perDayDict[day, default: 0] += tokens
                                perDayModelDict[day, default: [:]][label, default: 0] += tokens
                            }
                        }
                    default:
                        break
                    }
                }
                let firstSeq = records[0]["event"]["seq"].intValue ?? 0
                if page["hasMore"].boolValue == true && firstSeq > 0 {
                    through = firstSeq - 1
                } else {
                    break
                }
            }
            if pages >= 40 { summary.sessionsCapped += 1 }
            summary.talkSeconds += sessionTalkSeconds
            summary.longestTalkSeconds = max(summary.longestTalkSeconds, sessionTalkSeconds)
        }
        summary.totalTokens = perDayDict.values.reduce(0, +)
        summary.perModel = perModelDict.map { UsageModelTotal(label: $0.key, tokens: $0.value) }.sorted { $0.tokens > $1.tokens }
        summary.perDay = perDayDict.map { UsageDayTotal(day: $0.key, tokens: $0.value) }.sorted { $0.day < $1.day }
        summary.perDayModel = perDayModelDict.flatMap { day, byModel in
            byModel.map { UsageDayModelCell(day: day, model: $0.key, tokens: $0.value) }
        }
        let streaks = UsageMath.streaks(dayKeys: Array(perDayDict.keys))
        summary.currentStreakDays = streaks.current
        summary.longestStreakDays = streaks.longest
        for day in perDayDict.keys.sorted() { // 峰值:严格更大才替换,平峰取最早日(同 Win:10109-10112)
            let v = perDayDict[day] ?? 0
            if v > summary.peakTokens {
                summary.peakTokens = v
                summary.peakDay = day
            }
        }
        usage = summary
    }
}

extension AppState.Phase {
    var isFailed: Bool {
        if case .failed = self { return true }
        return false
    }
}

/// 串行异步门(对应 C# SemaphoreSlim(1,1) 的 followFrameGate)。
final class AsyncGate: @unchecked Sendable {
    private var locked = false
    private var waiters: [CheckedContinuation<Void, Never>] = []

    func run(_ body: @escaping () async -> Void) async {
        await acquire()
        defer { release() }
        await body()
    }

    private func acquire() async {
        lock.lock()
        if !locked {
            locked = true
            lock.unlock()
            return
        }
        lock.unlock()
        await withCheckedContinuation { cont in
            lock.lock()
            if !locked {
                locked = true
                lock.unlock()
                cont.resume()
                return
            }
            waiters.append(cont)
            lock.unlock()
        }
    }

    private func release() {
        lock.lock()
        if let next = waiters.first {
            waiters.removeFirst()
            lock.unlock()
            next.resume()
            return
        }
        locked = false
        lock.unlock()
    }

    private let lock = NSLock()
}
