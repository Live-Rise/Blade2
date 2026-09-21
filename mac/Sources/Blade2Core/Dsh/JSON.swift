import Foundation

/// 内核协议 JSON 的强类型表示(对应 C# 侧的 JsonElement)。
/// 协议字段访问全部经由本类型,避免 Any 桥接的脆弱性。
public enum JSON: Equatable, Sendable {
    case null
    case bool(Bool)
    case int(Int)
    case double(Double)
    case string(String)
    case array([JSON])
    case object([String: JSON])

    // MARK: - 构造便捷

    public static func obj(_ pairs: (String, JSON)...) -> JSON { .object(Dictionary(uniqueKeysWithValues: pairs.map { ($0.0, $0.1) })) }
    public static func dict(_ pairs: [(String, JSON)]) -> JSON { .object(Dictionary(uniqueKeysWithValues: pairs)) }
    public static func dict(_ d: [String: JSON]) -> JSON { .object(d) }
    public static func arr(_ items: [JSON]) -> JSON { .array(items) }

    // MARK: - 读取便捷

    public var isNull: Bool { if case .null = self { return true }; return false }
    public var boolValue: Bool? { if case .bool(let b) = self { return b }; return nil }
    public var intValue: Int? {
        switch self {
        case .int(let i): return i
        case .double(let d): return d == d.rounded() ? Int(d) : nil
        default: return nil
        }
    }
    public var doubleValue: Double? {
        switch self {
        case .int(let i): return Double(i)
        case .double(let d): return d
        default: return nil
        }
    }
    public var stringValue: String? { if case .string(let s) = self { return s }; return nil }
    public var arrayValue: [JSON]? { if case .array(let a) = self { return a }; return nil }
    public var objectValue: [String: JSON]? { if case .object(let o) = self { return o }; return nil }

    /// 下标取值:对象键缺失或数组越界一律回 null,永不崩溃(协议演化容错)。
    public subscript(key: String) -> JSON {
        if case .object(let o) = self { return o[key] ?? .null }
        return .null
    }
    public subscript(index: Int) -> JSON {
        if case .array(let a) = self, a.indices.contains(index) { return a[index] }
        return .null
    }

    public var description: String { jsonEncoded(pretty: false) }

    // MARK: - Codable

    public func encoded() -> Data {
        jsonEncoded(pretty: false).data(using: .utf8) ?? Data("null".utf8)
    }

    public func jsonEncoded(pretty: Bool) -> String {
        var body: String
        switch self {
        case .null: body = "null"
        case .bool(let b): body = b ? "true" : "false"
        case .int(let i): body = String(i)
        case .double(let d): body = formatDouble(d)
        case .string(let s): body = Self.escape(s)
        case .array(let items): body = "[" + items.map { $0.jsonEncoded(pretty: false) }.joined(separator: ",") + "]"
        case .object(let dict):
            let sorted = dict.sorted { $0.key < $1.key }
            body = "{" + sorted.map { Self.escape($0.key) + ":" + $0.value.jsonEncoded(pretty: false) }.joined(separator: ",") + "}"
        }
        return body
    }

    private static func escape(_ s: String) -> String {
        var out = "\""
        for scalar in s.unicodeScalars {
            switch scalar {
            case "\"": out += "\\\""
            case "\\": out += "\\\\"
            case "\n": out += "\\n"
            case "\r": out += "\\r"
            case "\t": out += "\\t"
            default:
                if scalar.value < 0x20 {
                    out += String(format: "\\u%04x", scalar.value)
                } else {
                    out.unicodeScalars.append(scalar)
                }
            }
        }
        return out + "\""
    }

    private func formatDouble(_ d: Double) -> String {
        if d == d.rounded() && abs(d) < 1e15 {
            return String(Int64(d))
        }
        return String(d)
    }

    public static func decode(_ data: Data) throws -> JSON {
        let element = try JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])
        return fromAny(element)
    }

    public static func decode(_ text: String) throws -> JSON {
        try decode(Data(text.utf8))
    }

    static func fromAny(_ any: Any) -> JSON {
        switch any {
        case is NSNull: return .null
        case let n as NSNumber:
            // Boolean 与整数在 NSNumber 里同形,按 objCType 区分
            let type = String(cString: n.objCType)
            switch type {
            case "c", "B": return .bool(n.boolValue)
            case "f", "d": return .double(n.doubleValue)
            default:
                if n.doubleValue == n.doubleValue.rounded() && abs(n.doubleValue) < 9.007e15 {
                    return .int(n.intValue)
                }
                return .double(n.doubleValue)
            }
        case let s as String: return .string(s)
        case let a as [Any]: return .array(a.map(fromAny))
        case let o as [String: Any]:
            var dict: [String: JSON] = [:]
            for (k, v) in o { dict[k] = fromAny(v) }
            return .object(dict)
        default: return .null
        }
    }

    public func toAnyObject() -> Any {
        switch self {
        case .null: return NSNull()
        case .bool(let b): return b
        case .int(let i): return i
        case .double(let d): return d
        case .string(let s): return s
        case .array(let items): return items.map { $0.toAnyObject() }
        case .object(let dict):
            var out: [String: Any] = [:]
            for (k, v) in dict { out[k] = v.toAnyObject() }
            return out
        }
    }
}
