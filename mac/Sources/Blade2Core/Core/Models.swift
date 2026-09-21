import Foundation

// MARK: - 工作区 / 会话

/// workspace/follow 树节点(对应 C# WorkspaceVm)。
public struct WorkspaceVm: Identifiable, Equatable {
    let workspaceId: String
    var title: String
    var path: String
    var sessionIds: [String]
    public var id: String { workspaceId }
}

/// session/list 条目(对应 C# SessionVm 的数据面)。
public struct SessionItem: Identifiable, Equatable {
    public let sessionId: String
    public let cwd: String
    let blank: Bool
    let updatedAt: Date?
    /// projections.values(sessionStats.turns、plan、permissions…)
    let projections: JSON
    /// api-session/status 缓存的运行态(侧栏活动点、busy 发送模式)。
    var running: Bool
    var busySince: Date?
    /// 壳侧覆盖标题(session/title 事件、手动重命名;优先于 projections)。
    var durableTitleOverride: String?

    public var id: String { sessionId }

    /// 标题规则(与 Win 版一致):blank ⇒ "新会话";否则覆盖/持久标题 ⇒ cwd 末段 ⇒ sessionId。
    func displayTitle(fallbackBlank: String) -> String {
        if blank { return fallbackBlank }
        if let o = durableTitleOverride, !o.isEmpty { return o }
        if let durable = durableTitle { return durable }
        if !cwd.isEmpty {
            let base = (cwd as NSString).lastPathComponent
            if !base.isEmpty { return base }
        }
        return sessionId
    }

    var durableTitle: String? {
        let values = projections["values"]
        if let t = values["session/title"].stringValue, !t.isEmpty { return t }
        // 部分内核版本把标题直接放在 values.title
        if let t2 = values["title"].stringValue, !t2.isEmpty { return t2 }
        return nil
    }

    var turns: Int? { projections["values"]["sessionStats"]["turns"].intValue }
}

// MARK: - 聊天气泡

public enum BubbleRole: String, Equatable {
    case user
    case userImage
    case assistant
    case reasoning
    case tool
    case error
    case deliverable
    case system
}

struct PresentedFile: Identifiable, Equatable {
    let index: Int
    let path: String
    let description: String
    /// present.open 回查坐标
    let sessionId: String
    let seq: Int
    public var id: String { sessionId + "#" + String(seq) + "#" + String(index) }
}

/// 聊天气泡(对应 C# ChatBubble)。
public struct ChatBubble: Identifiable, Equatable {
    public let id: UUID
    public var role: BubbleRole
    public var text: String
    var turn: Int
    var isReasoning: Bool
    /// assistant/message 的 message.id —— messageFeedback 目标标识
    var messageId: String?
    /// userImage 气泡:base64 图像数据(session/attachment 回读)
    var imageData: Data?
    var imageMediaType: String?
    var files: [PresentedFile]?
    var seq: Int

    init(role: BubbleRole, text: String, turn: Int = 0, isReasoning: Bool = false,
         messageId: String? = nil, imageData: Data? = nil, imageMediaType: String? = nil,
         files: [PresentedFile]? = nil, seq: Int = 0) {
        self.id = UUID()
        self.role = role
        self.text = text
        self.turn = turn
        self.isReasoning = isReasoning
        self.messageId = messageId
        self.imageData = imageData
        self.imageMediaType = imageMediaType
        self.files = files
        self.seq = seq
    }
}

/// journal 事件(records[].event)。
struct JournalEvent {
    let seq: Int
    let time: Double?
    let type: String
    let data: JSON
}

// MARK: - 模型目录

public struct ModelEntry: Identifiable, Equatable {
    public let provider: String
    public let id: String
    public let name: String
    public let efforts: [String]
    public let defaultEffort: String
    public var key: String { provider + "/" + id }
    public var display: String { name.isEmpty ? id : name }

    public init(provider: String, id: String, name: String, efforts: [String], defaultEffort: String) {
        self.provider = provider
        self.id = id
        self.name = name
        self.efforts = efforts
        self.defaultEffort = defaultEffort
    }
}

public struct ModelCatalog {
    public var defaultProvider: String
    public var defaultModel: String
    public var models: [ModelEntry]

    static let empty = ModelCatalog(defaultProvider: "", defaultModel: "", models: [])

    static func parse(_ value: JSON, fallbackProvider: String, fallbackModel: String) -> ModelCatalog {
        var models: [ModelEntry] = []
        for group in value["groups"].arrayValue ?? [] {
            let groupProvider = group["provider"].stringValue ?? fallbackProvider
            for m in group["models"].arrayValue ?? [] {
                let id = m["id"].stringValue ?? ""
                guard !id.isEmpty else { continue }
                models.append(ModelEntry(
                    provider: groupProvider,
                    id: id,
                    name: m["name"].stringValue ?? "",
                    efforts: (m["reasoning"]["efforts"].arrayValue ?? []).compactMap { $0.stringValue },
                    defaultEffort: m["reasoning"]["defaultEffort"].stringValue ?? ""
                ))
            }
        }
        return ModelCatalog(
            defaultProvider: value["default"]["provider"].stringValue ?? fallbackProvider,
            defaultModel: value["default"]["model"].stringValue ?? fallbackModel,
            models: models
        )
    }
}

// MARK: - 审批 / 提问(waterfall)

struct ApprovalRequest: Identifiable, Equatable {
    let eventId: String
    let agentId: String
    let toolName: String
    let reason: String
    public var id: String { eventId }
}

struct QuestionOption: Identifiable, Equatable {
    let label: String
    let description: String
    public var id: String { label }
}

struct QuestionItem: Identifiable, Equatable {
    let id: String
    let question: String
    let header: String
    let options: [QuestionOption]
    let multiSelect: Bool
    /// plan review 特殊形态:intent.kind == "plan-review",detail 为计划 markdown
    let intentKind: String
    let detail: String
}

struct QuestionRequest: Identifiable, Equatable {
    let eventId: String
    let agentId: String
    let questions: [QuestionItem]
    public var id: String { eventId }

    var isPlanReview: Bool {
        questions.first.map { $0.id == "plan-review" && $0.intentKind == "plan-review" } ?? false
    }
}

// MARK: - 斜杠命令

struct SlashCommand: Identifiable, Equatable {
    let name: String
    let description: String
    let inputHint: String
    var id: String { name }
}

// MARK: - 附件

struct PendingAttachment: Identifiable, Equatable {
    let id: UUID
    let name: String
    let mediaType: String
    let data: Data
    var isImage: Bool { mediaType.hasPrefix("image/") }

    init(name: String, mediaType: String, data: Data) {
        self.id = UUID()
        self.name = name
        self.mediaType = mediaType
        self.data = data
    }
}

// MARK: - 设置快照

/// settings/describe 的一个命名空间:revision + value + schemastery refs 树。
struct SettingsNamespace: Identifiable, Equatable {
    let ns: String
    var revision: Int
    var value: JSON
    var schema: JSON
    var id: String { ns }

    /// schemastery refs 表复原:schema 是 {uid, refs:{…}};命中 refs 键的整数按引用展开。
    /// plain 十进制数不是引用——只有命中 refs 表键的才展开。
    func rehydrated() -> JSON { Self.rehydrate(schema["dict"], refs: schema["refs"]) }

    static func rehydrate(_ node: JSON, refs: JSON) -> JSON {
        switch node {
        case .array(let items):
            return .array(items.map { rehydrate($0, refs: refs) })
        case .object(let dict):
            if let uid = dict["uid"]?.intValue, refs[String(uid)].objectValue != nil {
                return rehydrate(refs[String(uid)], refs: refs)
            }
            var out: [String: JSON] = [:]
            for (k, v) in dict { out[k] = rehydrate(v, refs: refs) }
            return .object(out)
        default:
            return node
        }
    }

    /// union 节点的枚举项:[{type:"const", value, meta:{description}}]。
    static func unionOptions(_ node: JSON) -> [(value: JSON, description: String)] {
        guard node["type"].stringValue == "union" else { return [] }
        var out: [(JSON, String)] = []
        for item in node["list"].arrayValue ?? [] where item["type"].stringValue == "const" {
            out.append((item["value"], item["meta"]["description"].stringValue ?? ""))
        }
        return out
    }
}

/// 一次待提交的 settings 变更(mutate 分组去抖用)。
struct SettingsMutation: Equatable {
    var ns: String
    var path: [String]
    var value: JSON
}

// MARK: - 消息反馈

struct MessageFeedbackItem: Equatable {
    let messageId: String
    let rating: String   // positive | negative
    let note: String
    let version: String
}

// MARK: - 用量统计(壳侧计算,无 usage RPC)

struct UsageModelStat: Equatable {
    let label: String
    let tokens: Int
}

struct UsageDayStat: Identifiable, Equatable {
    let day: String // yyyy-MM-dd
    let tokens: Int
    public var id: String { day }
}

struct UsageSummary: Equatable {
    var totalTokens = 0
    var sessionsCount = 0
    var talkSeconds = 0
    var perModel: [UsageModelStat] = []
    var perDay: [UsageDayStat] = []
    var partial = false
}


// MARK: - 壳层导航(视图层共享)

/// 主内容页枚举(与 Win 版 NavigationView 内容面一致:聊天 / 用量)。
/// 设置已迁出主窗口,由 Settings 独立场景承载(⌘,/侧栏按钮打开设置窗口)。
enum ShellPage: String, CaseIterable, Identifiable {
    case chat
    case usage
    var id: String { rawValue }
}

public extension Notification.Name {
    static let blade2NewSession = Notification.Name("blade2.newSession")
}
