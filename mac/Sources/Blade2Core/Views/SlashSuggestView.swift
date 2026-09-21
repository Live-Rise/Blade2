import SwiftUI

/// 斜杠候选条目:内核命令(commands/list)或壳内置命令(内核目录没有同名命令时的兜底)。
/// 由 AppState 斜杠命令区构造(AppState.slashSuggestions),浮层与执行路径共用。
struct SlashSuggestion: Identifiable, Equatable {
    let name: String
    /// 展示文案:中文即键,传入前已过 L()/LocalizeHostCommand(内核英文原文才替换)。
    let description: String
    /// commands/list 的 input.hint(内核按描述符给;内置命令取内核注册处的 hint 字面量)。
    let hint: String
    /// 需要参数的命令选中时只补全命令名,不直接执行(Win AcceptCommandSelection 的
    /// HasInput 分支,xaml.cs:12780-12798)。
    let hasInput: Bool
    /// 是否壳内置(仅诊断展示用;内核同名命令永远优先)。
    let isBuiltIn: Bool
    var id: String { name }
}

/// 斜杠命令补全浮层(对应 Win 版 CommandPalette):
/// 触发/过滤规则同 Win UpdateCommandPalette(xaml.cs:12661-12700)——输入以 "/" 开头且未出现空白,
/// Contains 匹配 → 前缀命中优先 → 字典序,取 20;键位契约同 HandlePaletteKey(xaml.cs:12732-12795):
/// ↑↓ 导航(不回绕)/ Tab·Enter 采纳 / Esc 关闭 / ⇧⏎ 仍是换行;点击采纳同 OnCommandItemClick(:12800)。
struct SlashSuggestView: View {
    let query: String
    let items: [SlashSuggestion]
    @Binding var selection: Int
    let onAccept: (SlashSuggestion) -> Void

    // 行高与列表上限取固定值:浮层高度可预计算,避免 ScrollView 在 overlay 里被
    // 容器 proposal 拉伸或塌陷(macOS 菜单单行约 28pt,取 30 含分隔留白)。
    private static let rowHeight: CGFloat = 30
    private static let listMaxHeight: CGFloat = 216

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(header)
                .font(.caption2)
                .foregroundStyle(.secondary)
                .padding(.horizontal, 12)
                .padding(.top, 8)
                .padding(.bottom, 6)
            ScrollViewReader { proxy in
                ScrollView {
                    VStack(alignment: .leading, spacing: 2) {
                        ForEach(Array(items.enumerated()), id: \.element.id) { index, item in
                            row(item, index: index)
                        }
                    }
                    .padding(.horizontal, 6)
                    .padding(.bottom, 6)
                }
                .frame(height: min(CGFloat(items.count) * 32 + 4, Self.listMaxHeight))
                .onChange(of: selection) { _, newValue in
                    // 键盘导航跟随(同 Win ScrollIntoView,xaml.cs:12768-12778)
                    guard items.indices.contains(newValue) else { return }
                    proxy.scrollTo(items[newValue].id)
                }
            }
        }
        .frame(maxWidth: 460, alignment: .leading)
        // 材质取舍:浮层是覆盖在聊天内容上的临时悬浮层,用与输入条一致的 code2Glass
        // (macOS 26 glassEffect / 旧系统 regularMaterial)才能"浮"起来;.quinary 是
        // 内容层内的淡底,叠在聊天文字上会透底,不适合这里。
        .code2Glass(in: RoundedRectangle(cornerRadius: 12))
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .strokeBorder(Color.primary.opacity(0.08), lineWidth: 1)
        )
        .shadow(color: .black.opacity(0.14), radius: 12, y: 4)
    }

    private var header: String {
        query.isEmpty
            ? L("命令（↑↓ 选择，Enter 执行，Esc 关闭）")
            : LF("“/{0}” 匹配 {1} 条（↑↓ 选择，Enter 执行，Esc 关闭）", query, items.count)
    }

    private func row(_ item: SlashSuggestion, index: Int) -> some View {
        HStack(spacing: 8) {
            Text("/\(item.name)")
                .font(.system(.body, design: .monospaced).weight(.medium))
                .foregroundStyle(index == selection ? Color.accentColor : Color.primary)
            if !item.hint.isEmpty {
                Text(item.hint)
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
                    .layoutPriority(-1)
            }
            Spacer(minLength: 8)
            Text(item.description)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .layoutPriority(1)
        }
        .padding(.horizontal, 8)
        .frame(height: Self.rowHeight)
        // 选中态按 macOS 惯例:强调色淡底 + 命令名转强调色(菜单式全彩填充在浅玻璃上过重)
        .background {
            if index == selection {
                RoundedRectangle(cornerRadius: 6)
                    .fill(Color.accentColor.opacity(0.14))
            }
        }
        .contentShape(Rectangle())
        .onHover { hovering in
            if hovering { selection = index } // 悬停高亮(macOS 菜单惯例)
        }
        .onTapGesture { onAccept(item) }
    }
}
