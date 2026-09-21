import Foundation

// MARK: - 会话运行统计(Win 契约:MainWindow.RunStats.cs 全文件 + xaml.cs:213-310 两面板)
//
// 折叠算法逐条镜像内核 @deepseek-ai/dsh-session-stats 的 sessionStats 投影(Win 版同源):
//   轮/步     step/end 计步;轮号变化才计轮                      —— RunStats.cs:134-145
//   模型用时  assistant/message.time − step/start.time 按步累计  —— RunStats.cs:88-93
//   TTFT      首 token 时刻 − step/start.time,按步平均;首 token
//             时刻 = assistant/attempt|message 的 stream 块数组
//             第一个带 time 的条目                               —— RunStats.cs:81-108,172-188
//   TPS(tok/s) decodeTokens / (decodeMs/1000);decode 区间 =
//             首 token → assistant/message.time,token 数 = usage.outputTokens —— RunStats.cs:100-108,236-239
//   工具用时  tool/call → tool/result 按 callId 配对时差累计     —— RunStats.cs:113-132
//   token     assistant/message.data.usage 累计;totalTokens 缺失
//             时按 input+output+cacheRead+cacheWrite 兜底        —— RunStats.cs:154-170
//   缓存命中  cacheRead / (cacheRead + input)                    —— RunStats.cs:241-245
//             (Win 实装的分母不含 cacheWrite,与任务简报的字面
//             口径不同——以 Win 代码为准)
// 数据入口 = renderEventCore 钩子(page 回放 + follow 实时流都经此);会话切换时
// openSession 先清零再重放完整 transcript,避免翻页重复累计(Win xaml.cs:3124)。

/// 单个会话的运行折叠状态(镜像 Win MainWindow.RunStats.cs:26-46 RunStatsState)。
struct RunStatsState: Equatable {
    var turns = 0
    var steps = 0
    var lastTurn: String?
    /// 模型用时(所有步的 LLM 耗时之和,毫秒)
    var llmMs: Double = 0
    /// 工具调用用时(tool/call → tool/result,毫秒)
    var toolMs: Double = 0
    /// TTFT 累计与计步数(面板取平均)
    var ttftMs: Double = 0
    var ttftSteps = 0
    /// decode 区间累计(毫秒)与区间内输出 token 数(TPS 分子/分母)
    var decodeMs: Double = 0
    var decodeTokens = 0
    var outputTokens = 0
    var totalTokens = 0
    var cacheReadTokens = 0
    var inputTokens = 0
    /// 进行中的步(turn 字符串与起始时刻;assistant/message 关步)
    var openStepTurn: String?
    var openStepStartMs: Double = 0
    /// 本步首 token 时刻(stream 块首条 time;attempt 先到先记)
    var firstTokenMs: Double?
    /// tool/call 的 callId → 派发时刻(结果到达配对算用时)
    var pendingCalls: [String: Double] = [:]
}

/// 运行统计聚合器:按会话 id 保存折叠状态,renderEventCore 逐事件喂入。
/// struct 值语义存于 AppState(可观察),聚合逻辑收在本文件。
struct RunStatsAggregator {
    private var states: [String: RunStatsState] = [:]

    /// journal 事件折叠入口。事件带 time 才计速率;未知事件直接忽略(Win RunStats.cs:61-152)。
    mutating func track(_ ev: JournalEvent, sessionId: String?) {
        guard let sessionId, !sessionId.isEmpty, ev.data.objectValue != nil else { return }
        var st = states[sessionId] ?? RunStatsState()
        let time = ev.time ?? 0
        let turn = ev.data["turn"].intValue.map(String.init)

        switch ev.type {
        case "step/start":
            st.openStepTurn = turn
            st.openStepStartMs = time
            st.firstTokenMs = nil
        case "assistant/attempt":
            // 首 token 时刻:步打开时由流式尝试先记(Win RunStats.cs:81-86)
            if let open = st.openStepTurn, open == turn, st.firstTokenMs == nil {
                st.firstTokenMs = Self.firstTokenFromStream(ev.data)
            }
        case "assistant/message":
            if let open = st.openStepTurn, open == turn {
                if time > st.openStepStartMs && st.openStepStartMs > 0 {
                    st.llmMs += time - st.openStepStartMs
                }
                let first = st.firstTokenMs ?? Self.firstTokenFromStream(ev.data)
                let output = Self.usageField(ev.data, "outputTokens")
                if let first {
                    if first > st.openStepStartMs && st.openStepStartMs > 0 {
                        st.ttftMs += first - st.openStepStartMs
                        st.ttftSteps += 1
                    }
                    if let output, time > first {
                        st.decodeMs += time - first
                        st.decodeTokens += output
                    }
                }
                st.openStepTurn = nil
            }
            Self.accumulateUsage(&st, ev.data)
        case "tool/call":
            if let callId = ev.data["callId"].stringValue, !callId.isEmpty, time > 0 {
                st.pendingCalls[callId] = time
            }
        case "tool/result":
            // 结果信封 message.source.callId(内核 createToolResultMessage:dsh-llm/lib/index.js:94-99)
            let rid = ev.data["message"]["source"]["callId"].stringValue ?? ""
            if !rid.isEmpty {
                if let dispatched = st.pendingCalls[rid], time > dispatched {
                    st.toolMs += time - dispatched
                }
                st.pendingCalls.removeValue(forKey: rid)
            }
        case "step/end":
            if let turn {
                // 轮变化才计轮;步无条件累计(Win RunStats.cs:134-143)
                if st.lastTurn != turn {
                    st.turns += 1
                    st.lastTurn = turn
                }
                st.steps += 1
            }
            st.openStepTurn = nil
        default:
            return // 未跟踪事件不刷新条
        }
        states[sessionId] = st
    }

    /// 会话切换/回放重放前清零:状态由随后的完整 transcript 回放重建(Win RunStats.cs:202-210)。
    mutating func reset(_ sessionId: String?) {
        guard let sessionId, !sessionId.isEmpty else { return }
        states.removeValue(forKey: sessionId)
    }

    func state(for sessionId: String?) -> RunStatsState? {
        guard let sessionId, !sessionId.isEmpty else { return nil }
        return states[sessionId]
    }

    /// usage 累计:totalTokens 缺失时按 input+output+cacheRead+cacheWrite 口径(与统计页一致,RunStats.cs:154-170)。
    private static func accumulateUsage(_ st: inout RunStatsState, _ data: JSON) {
        guard data["usage"].objectValue != nil else { return }
        let total = usageField(data, "totalTokens")
        let input = usageField(data, "inputTokens")
        let output = usageField(data, "outputTokens")
        let cacheRead = usageField(data, "cacheReadTokens")
        // cacheWrite 只参与 total 兜底,不单独入账(Win 同)
        st.totalTokens += total ?? (input ?? 0) + (output ?? 0) + (cacheRead ?? 0) + (usageField(data, "cacheWriteTokens") ?? 0)
        st.cacheReadTokens += cacheRead ?? 0
        st.inputTokens += input ?? 0
        st.outputTokens += output ?? 0
    }

    /// 首 token 时刻 = stream 块数组第一个带 time 的条目(官方 assistantStreamFirstTokenTime 的近似,RunStats.cs:172-188)。
    private static func firstTokenFromStream(_ data: JSON) -> Double? {
        for block in data["stream"].arrayValue ?? [] {
            if let t = block["time"].doubleValue { return t }
        }
        return nil
    }

    /// usage 字段读取:缺失或负值视为无(Win UsageField,RunStats.cs:190-194)。
    private static func usageField(_ data: JSON, _ name: String) -> Int? {
        guard let v = data["usage"][name].doubleValue, v >= 0 else { return nil }
        return Int(v)
    }
}

// MARK: - 数值格式化(Win RunStats.cs:236-253 与 xaml.cs:10750-10763 同口径)

extension RunStatsState {
    /// TPS:decodeTokens / (decodeMs/1000),整数;无 decode 数据时 "—"(Win FormatTps)。
    var tpsText: String {
        guard decodeMs > 0, decodeTokens > 0 else { return "—" }
        return String(Int((Double(decodeTokens) / (decodeMs / 1000.0)).rounded()))
    }

    /// 缓存命中数值(不含 % 号,面板模板自补),一位小数;无输入时 "—"(Win FormatCacheHit)。
    var cacheHitText: String {
        guard cacheReadTokens + inputTokens > 0 else { return "—" }
        return Self.oneDecimal(100.0 * Double(cacheReadTokens) / Double(cacheReadTokens + inputTokens))
    }

    /// 紧凑 token 数:中文环境 ≥1 亿「亿」、≥1 万「万」;其余 1.2K / 6.2M / 千分位(Win FormatTokensCompact)。
    func tokensCompactText(_ value: Int) -> String {
        if L10n.isZhSource {
            if value >= 100_000_000 { return LF("{0} 亿", Self.oneDecimal(Double(value) / 100_000_000.0)) }
            if value >= 10_000 { return LF("{0} 万", Self.oneDecimal(Double(value) / 10_000.0)) }
        }
        if value >= 1_000_000 { return Self.oneDecimal(Double(value) / 1_000_000.0) + "M" }
        if value >= 1_000 { return Self.oneDecimal(Double(value) / 1_000.0) + "K" }
        return Self.grouped(value)
    }

    /// 面板行用 token 数:千分位 + " tok"(Win TokensN0)。
    func tokensN0Text(_ value: Int) -> String {
        Self.grouped(value) + " tok"
    }

    /// 时长可读化:小时 + 分(不足 1 小时只给分,不足 1 分钟给秒)(Win FormatDuration,xaml.cs:10757-10763)。
    func durationText(_ ms: Double) -> String {
        let totalSeconds = max(0, Int(ms / 1000.0))
        if totalSeconds >= 3600 {
            return LF("{0} 小时 {1} 分", String(totalSeconds / 3600), String((totalSeconds % 3600) / 60))
        }
        if totalSeconds >= 60 {
            return LF("{0} 分 {1} 秒", String(totalSeconds / 60), String(totalSeconds % 60))
        }
        return LF("{0} 秒", String(totalSeconds))
    }

    /// "0.#" 等价:一位小数,整值不带小数点(C# "0.#" 格式)。
    static func oneDecimal(_ value: Double) -> String {
        let rounded = (value * 10).rounded() / 10
        if rounded == rounded.rounded() {
            return String(Int(rounded))
        }
        return String(format: "%.1f", rounded)
    }

    /// N0 等价:千分位逗号(固定不变文化,Win 用 InvariantCulture)。
    static func grouped(_ value: Int) -> String {
        var digits = Array(String(value))
        var out: [Character] = []
        for (index, ch) in digits.enumerated() {
            if index > 0, (digits.count - index) % 3 == 0 { out.append(",") }
            out.append(ch)
        }
        digits = out
        return String(out)
    }
}

// MARK: - transcript 紧凑折叠(Win xaml.cs:243-298 ApplyTranscriptView/TrackTranscriptEvent)

/// 折叠状态:轮的起止与「答案气泡」。键 = journal seq(气泡携 seq,等价 Win 的气泡引用相等)。
/// 切会话随 bubbles 清屏重置,由随后的完整 transcript 回放重建(Win xaml.cs:1577-1586)。
struct TranscriptFoldState: Equatable {
    /// 见过 turn/start 的轮
    var startedTurns: Set<Int> = []
    /// 正常完成的轮(turn/end 且 reason.kind == "completed")
    var closedTurns: Set<Int> = []
    /// 轮 → 答案气泡 seq(该轮最后一条无 tool-call 块且有非空 text 的 assistant 消息)
    var answers: [Int: Int] = [:]

    mutating func track(_ ev: JournalEvent) {
        let turn = ev.data["turn"].intValue ?? 0
        guard turn > 0 else { return } // Win TrackTranscriptEvent:turn <= 0 不跟踪
        switch ev.type {
        case "turn/start":
            startedTurns.insert(turn)
            closedTurns.remove(turn)
            answers.removeValue(forKey: turn)
        case "step/start", "tool/invoke", "tool/call":
            // 新步骤/工具调用使之前的答案失效,不能拿上一阶段的说明当结果(Win xaml.cs:281-282)
            answers.removeValue(forKey: turn)
        case "assistant/message":
            answers.removeValue(forKey: turn)
            if Self.isTranscriptAnswer(ev.data["message"]) {
                answers[turn] = ev.seq
            }
        case "turn/end":
            if startedTurns.contains(turn), ev.data["reason"]["kind"].stringValue == "completed" {
                closedTurns.insert(turn)
            }
        default:
            break
        }
    }

    /// 答案判定(Win IsTranscriptAnswer,xaml.cs:292-298):assistant 角色、
    /// content 无 tool-call 块、且有非空 text 块。
    static func isTranscriptAnswer(_ message: JSON) -> Bool {
        guard message["role"].stringValue == "assistant",
              let content = message["content"].arrayValue else { return false }
        var hasText = false
        for block in content {
            let type = block["type"].stringValue
            if type == "tool-call" { return false }
            if type == "text", let text = block["text"].stringValue, !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                hasText = true
            }
        }
        return hasText
    }
}

// MARK: - 内核任务(dsh-jobs;session/control 流 jobs 帧,Win JobVm + ParseJobs)

/// session/control 流的作业条目(Win xaml.cs:162-179 JobVm:字段 {id,label,status,detail?})。
struct KernelJob: Equatable, Identifiable {
    let jobId: String
    let label: String
    let status: String
    let detail: String
    public var id: String { jobId }

    /// 进行中 = running/stopping,其余已结束(Win IsActiveJob,xaml.cs:11138-11139)。
    var isActive: Bool { status == "running" || status == "stopping" }

    /// 状态文案(Win JobVm.StatusLabel,xaml.cs:170-178)。
    var statusLabel: String {
        switch status {
        case "running": L("运行中")
        case "stopping": L("停止中")
        case "completed": L("已完成")
        case "killed": L("已终止")
        case "failed": L("失败")
        default: status
        }
    }
}

// MARK: - AppState 扩展(钩子入口 + 展示投影)

extension AppState {
    // ---- renderEventCore 的两条最小钩子(Win RenderEventCore xaml.cs:3858-3859 的对应位) ----

    /// 运行状态条统计钩子:page 回放 + follow 实时流都经 renderEventCore,历史与实时天然全覆盖。
    func trackRunStats(_ ev: JournalEvent) {
        runStats.track(ev, sessionId: activeSessionId)
    }

    /// 紧凑折叠轮次跟踪钩子。
    func transcriptTrack(_ ev: JournalEvent) {
        transcriptFold.track(ev)
    }

    // ---- 紧凑模式的气泡投影 ----

    /// transcript 显示模式:内核设置 ui-chat.transcriptView(Win xaml.cs:6980 NsString 读法;
    /// 内核 dsh-client-ui-chat/lib/index.js:5-9 normal/compact,默认 compact)。
    /// 兼容读 "general" 命名空间下的同名键(mac 壳既有读法);两处都缺省 ⇒ 紧凑(Win:preference != "normal")。
    var transcriptCompact: Bool {
        let preference = settingsValue(ns: "ui-chat", path: ["transcriptView"]).stringValue
            ?? settingsValue(ns: "general", path: ["ui-chat", "transcriptView"]).stringValue
        return preference != "normal"
    }

    /// 展示层过滤:appendBubble 保留全量 transcript(过程内容不丢),视图只取本投影 ——
    /// 切回标准无需重拉 RPC,也不丢过程内容(Win RefreshTranscriptView 的可见集合投影同构)。
    /// 紧凑时:已正常完成轮的 reasoning/tool/中间 assistant 步骤气泡折叠,仅保留该轮答案
    /// (Win xaml.cs:254-256:过程气泡 IsToolCall|IsReasoning|IsAssistantStep 折叠,答案例外;
    /// 轮未闭合或无答案 ⇒ 整轮保留)。错误/交付物/系统/用户气泡不在过程集合内,恒可见。
    var visibleBubbles: [ChatBubble] {
        guard transcriptCompact else { return bubbles }
        return bubbles.filter { bubble in
            guard bubble.turn > 0, transcriptFold.closedTurns.contains(bubble.turn) else { return true }
            guard bubble.role == .reasoning || bubble.role == .tool || bubble.role == .assistant else { return true }
            guard let answerSeq = transcriptFold.answers[bubble.turn] else { return true }
            return bubble.seq == answerSeq
        }
    }

    // ---- 作业面板(session/control 的 jobs 帧;Win RefreshJobsPanel xaml.cs:11098-11136) ----

    /// jobs 帧 {sessionId, jobs:[…]}:数组缺失 ⇒ 清空该会话(Win xaml.cs:10862-10871)。
    func applyJobsFrame(_ sessionId: String, _ jobs: JSON) {
        runJobs[sessionId] = Self.parseJobs(jobs)
    }

    /// jobs 快照 {<sid>:[job…]}:整表替换(Win baseline 分支 xaml.cs:10826-10832 同构)。
    func applyJobsBaseline(_ jobsBySession: JSON) {
        var rebuilt: [String: [KernelJob]] = [:]
        for (sid, arr) in jobsBySession.objectValue ?? [:] {
            rebuilt[sid] = Self.parseJobs(arr)
        }
        runJobs = rebuilt
    }

    /// 作业数组解析(Win ParseJobs xaml.cs:11004-11022:{id,label,status,detail?})。
    static func parseJobs(_ jobs: JSON) -> [KernelJob] {
        (jobs.arrayValue ?? []).map { job in
            KernelJob(
                jobId: job["id"].stringValue ?? "",
                label: job["label"].stringValue ?? "",
                status: job["status"].stringValue ?? "",
                detail: job["detail"].stringValue ?? ""
            )
        }
    }

    /// 活动会话的作业视图:运行中在前、已结束在后(Win xaml.cs:11121-11127)。
    var activeSessionJobs: [KernelJob] {
        guard let sid = activeSessionId, let jobs = runJobs[sid] else { return [] }
        return jobs.filter(\.isActive) + jobs.filter { !$0.isActive }
    }
}
