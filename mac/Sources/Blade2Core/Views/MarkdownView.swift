import SwiftUI

// MARK: - 聊天 markdown

/// 聊天 markdown 渲染:围栏代码块切分为等宽卡片(语言角标 + 复制 + 横向滚动),
/// 其余段落沿用 inline AttributedString(含行内 `code` 自动样式)。
/// 正文基础字号来自主题设置 ui-theme.fontSize。
struct MarkdownView: View {
    let text: String
    var fontSize: Double = 14

    var body: some View {
        MarkdownBody(text: text, fontSize: fontSize)
            .equatable()
    }
}

/// Equatable 短路:text/fontSize 未变时 SwiftUI 跳过 body,解析不随无关重绘(悬停、全局状态)重算。
private struct MarkdownBody: View, Equatable {
    let text: String
    let fontSize: Double

    static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.text == rhs.text && lhs.fontSize == rhs.fontSize
    }

    var body: some View {
        let segments = MarkdownParser.parse(text)
        VStack(alignment: .leading, spacing: 5) {
            ForEach(Array(segments.enumerated()), id: \.offset) { _, segment in
                switch segment {
                case .text(let content):
                    inlineText(content)
                case .code(let language, let code):
                    CodeBlockCard(language: language, code: code)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func inlineText(_ content: String) -> some View {
        Group {
            if let attr = try? AttributedString(
                markdown: content,
                options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
            ) {
                Text(attr)
            } else {
                Text(content)
            }
        }
        .font(.system(size: fontSize))
        .lineSpacing(3)
        .textSelection(.enabled)
    }
}

// MARK: - 分段解析

/// markdown 分段:代码块与其余文本。
enum MarkdownSegment: Equatable {
    case text(String)
    case code(language: String, code: String)
}

/// 围栏代码块切分:```lang 开栏、纯反引号行闭栏;未闭合兜底渲染到结尾。
enum MarkdownParser {
    static func parse(_ text: String) -> [MarkdownSegment] {
        var segments: [MarkdownSegment] = []
        var pending: [String] = []  // 暂存非代码行
        let lines = text.replacingOccurrences(of: "\r\n", with: "\n")
            .components(separatedBy: "\n")

        func flushText() {
            guard !pending.isEmpty else { return }
            let joined = pending.joined(separator: "\n")
            guard !joined.isEmpty else { return }  // 空文本不成段
            segments.append(.text(joined))
            pending.removeAll()
        }

        var index = 0
        while index < lines.count {
            let trimmed = lines[index].trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("```") {
                flushText()
                // 语言取 info 串首词(如 ```swift title="x" → swift)
                let language = trimmed.dropFirst(3)
                    .trimmingCharacters(in: .whitespaces)
                    .split(whereSeparator: { $0 == " " || $0 == "\t" })
                    .first.map(String.init) ?? ""
                index += 1
                var codeLines: [String] = []
                while index < lines.count {
                    let candidate = lines[index].trimmingCharacters(in: .whitespaces)
                    // 闭栏:整行仅反引号/空白(带 info 串的 ``` 行是内容,不误判)
                    if candidate.hasPrefix("```"),
                       candidate.allSatisfy({ $0 == "`" || $0 == " " || $0 == "\t" }) {
                        break
                    }
                    codeLines.append(lines[index])
                    index += 1
                }
                if index < lines.count { index += 1 }  // 跳过闭栏行
                segments.append(.code(language: language, code: codeLines.joined(separator: "\n")))
            } else {
                pending.append(lines[index])
                index += 1
            }
        }
        flushText()
        return segments
    }
}

// MARK: - 代码块卡片

/// 代码块:主题自适应底色圆角卡片,头部 = 语言角标 + 复制;正文等宽不换行,横向滚动。
private struct CodeBlockCard: View {
    let language: String
    let code: String
    @State private var copied = false
    @State private var copyReset: Task<Void, Never>?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                Text(language.isEmpty ? L("代码") : language)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer(minLength: 8)
                copyButton
            }
            .padding(.horizontal, 12)
            .padding(.top, 7)
            .padding(.bottom, 3)

            ScrollView(.horizontal) {
                Text(code)
                    .font(.system(size: 12.5, design: .monospaced))
                    .textSelection(.enabled)
                    .fixedSize(horizontal: true, vertical: false)  // 不换行,超宽横滚
                    .padding(.horizontal, 12)
            }
            .padding(.bottom, 10)
        }
        .background(Color.primary.opacity(0.06), in: RoundedRectangle(cornerRadius: 8))
    }

    private var copyButton: some View {
        Button {
            copyCode()
        } label: {
            Label(L("复制"), systemImage: copied ? "checkmark" : "doc.on.doc")
                .font(.caption)
                .foregroundStyle(copied ? Color.green : Color.secondary)
        }
        .buttonStyle(.borderless)
        .help(L("复制代码"))
    }

    private func copyCode() {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(code, forType: .string)
        copied = true
        copyReset?.cancel()
        copyReset = Task {
            try? await Task.sleep(nanoseconds: 1_500_000_000)
            guard !Task.isCancelled else { return }
            copied = false
        }
    }
}
