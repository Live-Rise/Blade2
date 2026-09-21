import SwiftUI

/// 侧栏:新会话 + 工作区/会话树 + 搜索 + 设置窗口/用量入口。
/// 树数据来自 workspace/follow(baseline/upsert/remove/order/archived),
/// 会话标题规则与 Win 版一致。
struct SidebarView: View {
    @Environment(AppState.self) private var app
    @Environment(\.openSettings) private var openSettings
    @Binding var page: ShellPage
    @State private var selection: String?
    @State private var searchTask: Task<Void, Never>?
    @State private var renameTarget: SessionItem?
    @State private var renameWorkspaceTarget: WorkspaceVm?
    @State private var deleteWorkspaceTarget: WorkspaceVm?
    @State private var actionNotice: String?
    @State private var showWorkspaceSheet = false

    var body: some View {
        @Bindable var app = app
        List(selection: $selection) {
            if isSearching {
                searchSection
            } else {
                newSessionRow
                ungroupedSection
                workspaceSections
            }
        }
        .listStyle(.sidebar)
        .safeAreaInset(edge: .bottom, spacing: 0) { bottomActions }
        .overlay {
            if app.phase == .ready && app.workspaces.isEmpty && app.sessions.isEmpty && !isSearching {
                ContentUnavailableView(L("暂无会话"), systemImage: "bubble.left.and.bubble.right")
            }
        }
        .onChange(of: selection) { _, newValue in
            guard let sid = newValue else { return }
            page = .chat
            Task { await app.openSession(sid) }
        }
        .onChange(of: app.activeSessionId) { old, new in
            if old != new { selection = new }
        }
        .onReceive(NotificationCenter.default.publisher(for: .blade2NewSession)) { _ in
            app.startNewSessionDraft()
            selection = nil
            page = .chat
        }
        .sheet(item: $renameTarget) { item in
            RenameSheet(title: L("重命名会话"), initial: item.displayTitle(fallbackBlank: L("新会话"))) { newTitle in
                Task { await app.renameSession(item.sessionId, title: newTitle) }
            }
        }
        .sheet(item: $renameWorkspaceTarget) { ws in
            RenameSheet(title: L("重命名工作区"), initial: ws.title) { newTitle in
                Task {
                    await app.renameWorkspace(ws.workspaceId, title: newTitle)
                    app.refreshAllLists()
                }
            }
        }
        .sheet(isPresented: $showWorkspaceSheet) {
            AddWorkspaceSheet { path in
                Task { await app.createWorkspace(path: path) }
            }
        }
        // 侧栏动作失败提示(Win 版 ShowErrorAsync 对话框同位;RPC 返回 nil=成功静默)。
        .alert(
            L("提示"),
            isPresented: Binding(
                get: { actionNotice != nil },
                set: { if !$0 { actionNotice = nil } }
            )
        ) {
            Button(L("确定"), role: .cancel) {}
        } message: {
            Text(actionNotice ?? "")
        }
        // 删除工作区确认(Win xaml.cs:2488-2496 同文案):仅移出列表,目录与会话保留。
        .alert(
            L("删除工作区"),
            isPresented: Binding(
                get: { deleteWorkspaceTarget != nil },
                set: { if !$0 { deleteWorkspaceTarget = nil } }
            )
        ) {
            Button(L("删除"), role: .destructive) {
                guard let ws = deleteWorkspaceTarget else { return }
                deleteWorkspaceTarget = nil
                Task {
                    await app.deleteWorkspace(ws.workspaceId)
                    app.refreshAllLists()
                }
            }
            Button(L("取消"), role: .cancel) {}
        } message: {
            Text(deleteWorkspaceTarget.map {
                LF("将把“{0}”从工作区列表中移除。文件夹与会话记录会保留，其会话将显示在“未分组”下。", $0.title)
            } ?? "")
        }
    }

    private var isSearching: Bool { !app.searchQuery.isEmpty }

    // MARK: 搜索

    @ViewBuilder
    private var searchSection: some View {
        Section(L("搜索结果")) {
            if app.searchResults.isEmpty {
                Text(L("无匹配结果")).font(.caption).foregroundStyle(.secondary)
            }
            ForEach(app.searchResults) { hit in
                VStack(alignment: .leading, spacing: 2) {
                    Text(titleOf(hit.sessionId)).lineLimit(1)
                    Text(hit.snippet).font(.caption).foregroundStyle(.secondary).lineLimit(2)
                }
                .tag(hit.sessionId)
            }
        }
    }

    private func titleOf(_ sid: String) -> String {
        app.sessions.first(where: { $0.sessionId == sid })?.displayTitle(fallbackBlank: L("新会话")) ?? sid
    }

    // MARK: 固定行

    private var newSessionRow: some View {
        Section {
            Label(L("新会话"), systemImage: "plus.message")
                .tag("__new_session__")
                .onTapGesture {
                    app.startNewSessionDraft()
                    selection = nil
                    page = .chat
                }
        }
    }

    /// 不属于任何工作区的会话(Win 版同样存在未分组形态)。
    @ViewBuilder
    private var ungroupedSection: some View {
        let grouped = Set(app.workspaces.flatMap { $0.sessionIds })
        let orphans = app.sessions.filter { !grouped.contains($0.sessionId) && !app.archivedSessionIds.contains($0.sessionId) }
        if !orphans.isEmpty {
            Section(L("会话")) {
                ForEach(orphans) { item in
                    sessionRow(item)
                }
            }
        }
    }

    @ViewBuilder
    private var workspaceSections: some View {
        ForEach(app.workspaces) { ws in
            Section {
                let items = ws.sessionIds.compactMap { id in app.sessions.first(where: { $0.sessionId == id }) }
                if items.isEmpty {
                    Text(L("暂无会话")).font(.caption).foregroundStyle(.tertiary)
                }
                ForEach(items) { item in
                    sessionRow(item)
                }
                Label(L("新建会话"), systemImage: "plus")
                    .font(.caption)
                    .onTapGesture {
                        app.newSessionWorkspaceId = ws.workspaceId
                        app.startNewSessionDraft()
                        selection = nil
                        page = .chat
                    }
            } header: {
                // 工作区条目右键(Win xaml.cs:2101-2124):打开路径/重命名/上移/下移/删除。
                Text(ws.title.isEmpty ? L("工作区") : ws.title)
                    .contextMenu { workspaceMenu(ws) }
            }
        }
    }

    /// 工作区右键菜单(Win xaml.cs:2103-2123 同项)。
    @ViewBuilder
    private func workspaceMenu(_ ws: WorkspaceVm) -> some View {
        Button(L("打开工作区路径")) {
            Task { actionNotice = await app.openWorkspacePath(ws) }
        }
        Button(L("重命名工作区")) { renameWorkspaceTarget = ws }
        Button(L("上移")) {
            Task { actionNotice = await app.moveWorkspace(ws, delta: -1) }
        }
        Button(L("下移")) {
            Task { actionNotice = await app.moveWorkspace(ws, delta: 1) }
        }
        Divider()
        Button(L("删除工作区"), role: .destructive) { deleteWorkspaceTarget = ws }
    }

    private func sessionRow(_ item: SessionItem) -> some View {
        HStack(spacing: 6) {
            if item.running {
                ProgressView().controlSize(.mini)
            } else {
                Circle().fill(.secondary.opacity(0.25)).frame(width: 6, height: 6)
            }
            VStack(alignment: .leading, spacing: 1) {
                Text(item.displayTitle(fallbackBlank: L("新会话"))).lineLimit(1)
                if let caption = sessionCaption(item) {
                    Text(caption).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                }
            }
        }
        .tag(item.sessionId)
        .contextMenu {
            Button(L("重命名")) { renameTarget = item }
            Button(L("分叉会话")) { Task { await app.forkSession(item.sessionId) } }
            Button(L("打开工作目录")) { Task { await app.openWorkspacePath(item.sessionId) } }
            Menu(L("移动到工作区…")) {
                ForEach(app.workspaces) { ws in
                    // 子菜单直选直移(Win xaml.cs:2403-2406),不再二次弹选择表。
                    Button(ws.title.isEmpty ? ws.path : ws.title) {
                        Task { await app.moveSession(item.sessionId, toWorkspace: ws.workspaceId) }
                    }
                }
            }
            if item.running {
                Button(L("取消运行")) {
                    Task { actionNotice = await app.cancelSession(item.sessionId) }
                }
            }
            Divider()
            Button(L("归档会话"), role: .destructive) {
                Task { await app.archiveSession(item.sessionId) }
            }
        }
    }

    /// 会话行副标题:相对时间 + 轮数,一行合成(Win 行内相对时间见 xaml.cs:2323-2334)。
    private func sessionCaption(_ item: SessionItem) -> String? {
        var parts: [String] = []
        let ago = relativeTimeLabel(item.updatedAt)
        if !ago.isEmpty { parts.append(ago) }
        if let turns = item.turns, turns > 0 {
            parts.append(LF("{0} 轮对话", turns))
        }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }

    /// 相对时间(镜像 Win RelativeTimeLabel,xaml.cs:1908-1928):
    /// 刚刚 / N分钟前 / N小时前 / N天前 / N个月前 / N年前;时间戳缺失返回空串。
    private func relativeTimeLabel(_ date: Date?) -> String {
        guard let date else { return "" }
        var span = Date.now.timeIntervalSince(date)
        if span < 0 { span = 0 }
        let totalMinutes = Int(span / 60)
        switch totalMinutes {
        case 0: return L("刚刚")
        case 1..<60: return LF("{0}分钟前", totalMinutes)
        case 60..<(60 * 24): return LF("{0}小时前", totalMinutes / 60)
        case (60 * 24)..<(60 * 24 * 30): return LF("{0}天前", totalMinutes / (60 * 24))
        case (60 * 24 * 30)..<(60 * 24 * 365): return LF("{0}个月前", totalMinutes / (60 * 24 * 30))
        default: return LF("{0}年前", totalMinutes / (60 * 24 * 365))
        }
    }

    // MARK: 底部入口

    private var bottomActions: some View {
        HStack(spacing: 4) {
            // 设置走独立窗口(openSettings 环境动作,macOS 14+),不再是主窗口内嵌页,恒不高亮。
            toggleButton(L("设置"), systemImage: "gearshape", active: false) { openSettings() }
            toggleButton(L("用量"), systemImage: "chart.bar", active: page == .usage) { page = .usage }
            toggleButton(L("聊天"), systemImage: "bubble.left.and.text.bubble.right", active: page == .chat) { page = .chat }
            Spacer()
            Button {
                showWorkspaceSheet = true
            } label: {
                Image(systemName: "folder.badge.plus")
            }
            .buttonStyle(.borderless)
            .help(L("新建工作区"))
        }
        .font(.callout)
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(.bar)
    }

    private func toggleButton(_ title: String, systemImage: String, active: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Label(title, systemImage: systemImage)
                .labelStyle(.iconOnly)
        }
        .buttonStyle(.borderless)
        .help(title)
        .foregroundStyle(active ? Color.accentColor : Color.secondary)
    }
}

// MARK: - Sheets

struct RenameSheet: View {
    let title: String
    let initial: String
    let onCommit: (String) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var text = ""

    var body: some View {
        VStack(spacing: 14) {
            Text(title).font(.headline)
            TextField(L("名称"), text: $text)
                .textFieldStyle(.roundedBorder)
                .frame(minWidth: 320)
            HStack {
                Button(L("取消")) { dismiss() }.keyboardShortcut(.cancelAction)
                Button(L("确定")) {
                    onCommit(text)
                    dismiss()
                }
                .keyboardShortcut(.defaultAction)
                .disabled(text.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(22)
        .onAppear { text = initial }
    }
}

struct AddWorkspaceSheet: View {
    let onCreate: (String) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var path = ""

    var body: some View {
        VStack(spacing: 14) {
            Text(L("新建工作区")).font(.headline)
            HStack {
                TextField(L("目录路径"), text: $path)
                    .textFieldStyle(.roundedBorder)
                    .frame(minWidth: 360)
                Button(L("选择…")) { pickDirectory() }
            }
            Text(L("内核将把该目录登记为工作区(不移动文件)。"))
                .font(.caption).foregroundStyle(.secondary)
            HStack {
                Button(L("取消")) { dismiss() }.keyboardShortcut(.cancelAction)
                Button(L("创建")) {
                    onCreate(path)
                    dismiss()
                }
                .keyboardShortcut(.defaultAction)
                .disabled(path.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(22)
    }

    private func pickDirectory() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        if panel.runModal() == .OK, let url = panel.url {
            path = url.path
        }
    }
}
