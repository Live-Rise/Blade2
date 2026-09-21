import SwiftUI

/// 应用壳:侧栏 + 内容面;启动/失败态全覆盖。
public struct ShellView: View {
    @Environment(AppState.self) private var app
    @State private var page: ShellPage = .chat

    public init() {}

    public var body: some View {
        NavigationSplitView {
            SidebarView(page: $page)
                .navigationSplitViewColumnWidth(min: 230, ideal: 260, max: 340)
        } detail: {
            detail
        }
        .navigationTitle("Blade²")
        .navigationSubtitle(subtitle)
        .searchable(
            text: searchBinding,
            placement: .sidebar,
            prompt: L("搜索会话…"),
            suggestions: { searchSuggestions }
        )
        .overlay { phaseOverlay }
        .preferredColorScheme(colorScheme)
        // 回到前台清 Dock 审批徽标(徽标挂起时机见 AppState.showNextApproval)
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
            app.refreshDockBadge()
        }
        // 语言切换时整体重建视图树重取翻译(对应 Win 版语言切换的动态重建;AppState.uiLocaleVersion)
        .id(app.uiLocaleVersion)
    }

    /// 标准 searchable 工具栏:搜索场置于侧栏工具栏区,结果走系统建议列表。
    @ViewBuilder
    private var searchSuggestions: some View {
        ForEach(app.searchResults) { hit in
            Button {
                page = .chat
                Task { await app.openSession(hit.sessionId) }
            } label: {
                VStack(alignment: .leading, spacing: 2) {
                    Text(titleOf(hit.sessionId)).lineLimit(1)
                    Text(hit.snippet).font(.caption).foregroundStyle(.secondary).lineLimit(2)
                }
            }
            .buttonStyle(.plain)
        }
    }

    private var searchBinding: Binding<String> {
        Binding(
            get: { app.searchQuery },
            set: { query in
                app.searchQuery = query
                searchTask?.cancel()
                guard !query.isEmpty else { app.searchResults = []; return }
                searchTask = Task { [weak app] in
                    try? await Task.sleep(nanoseconds: 250_000_000)
                    guard !Task.isCancelled else { return }
                    await app?.performSearch(query)
                }
            }
        )
    }
    @State private var searchTask: Task<Void, Never>?

    private func titleOf(_ sid: String) -> String {
        app.sessions.first(where: { $0.sessionId == sid })?.displayTitle(fallbackBlank: L("新会话")) ?? sid
    }

    @ViewBuilder
    private var detail: some View {
        switch page {
        case .chat: ChatPageView()
        case .usage: UsageView()
        }
    }

    private var subtitle: String {
        guard let sid = app.activeSessionId,
              let item = app.sessions.first(where: { $0.sessionId == sid }) else { return "" }
        return item.displayTitle(fallbackBlank: L("新会话"))
    }

    /// ui-theme.preference(settings/describe)驱动外观。
    private var colorScheme: ColorScheme? {
        switch app.themePreference {
        case "light": return .light
        case "dark": return .dark
        default: return nil
        }
    }

    @ViewBuilder
    private var phaseOverlay: some View {
        switch app.phase {
        case .starting(let message):
            ZStack {
                VisualEffectBackground()
                VStack(spacing: 14) {
                    ProgressView().controlSize(.large)
                    Text("Blade²").font(.system(size: 28, weight: .bold))
                    Text(message).foregroundStyle(.secondary)
                }
            }
        case .failed(let message):
            ZStack {
                VisualEffectBackground()
                VStack(spacing: 14) {
                    Image(systemName: "exclamationmark.triangle").font(.system(size: 40))
                        .foregroundStyle(.orange)
                    Text("Blade²").font(.system(size: 28, weight: .bold))
                    Text(L("内核启动失败")).font(.headline)
                    Text(message)
                        .font(.caption).foregroundStyle(.secondary)
                        .frame(maxWidth: 520)
                        .multilineTextAlignment(.center)
                        .lineLimit(8)
                    Button(L("重试")) {
                        app.retryBoot()
                        Task {}
                    }
                    .buttonStyle(.borderedProminent)
                }
                .padding()
            }
        case .idle, .ready:
            EmptyView()
        }
    }
}

/// 空态品牌面(对应 Win 版 ChatHero)。
struct HeroView: View {
    var body: some View {
        VStack(spacing: 10) {
            Text("Blade²").font(.system(size: 44, weight: .bold))
            Text(L("dsh 内核原生壳 · 会话 · 审批 · 技能 · 模型"))
                .foregroundStyle(.secondary)
            Text(L("输入消息开始;“/”可用斜杠命令, ⇧⏎ 换行"))
                .font(.caption).foregroundStyle(.tertiary)
        }
    }
}

/// 设置窗口根视图:SettingsView 仅模块内可见,公开壳层经此包装进 Settings 场景
/// (AppState 由场景侧注入环境,随视图树传给内部 SettingsView)。
public struct SettingsRootView: View {
    public init() {}

    public var body: some View {
        SettingsView()
    }
}

/// 毛玻璃背景(启动/失败覆盖层)。
struct VisualEffectBackground: NSViewRepresentable {
    func makeNSView(context: Context) -> NSVisualEffectView {
        let view = NSVisualEffectView()
        view.material = .underWindowBackground
        view.blendingMode = .behindWindow
        view.state = .active
        return view
    }
    func updateNSView(_ nsView: NSVisualEffectView, context: Context) {}
}
