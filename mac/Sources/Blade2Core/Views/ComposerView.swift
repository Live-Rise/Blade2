import SwiftUI
import AppKit
import UniformTypeIdentifiers

/// 底部输入区:附件条 + 多行输入(⏎ 发送 / ⇧⏎ 换行)+ 模型/工作区/预设选择 + 发送与停止。
/// busy 态下显示 queue/steer 模式切换(取自 ui-conversation.busyEnter 默认值)。
struct ComposerView: View {
    @Environment(AppState.self) private var app
    @State private var steerOverride: Bool = false // busy 时显式选 steer
    @FocusState private var inputFocused: Bool

    @State private var dropTargeted = false // 拖入视觉反馈

    var body: some View {
        @Bindable var app = app
        VStack(spacing: 0) {
            attachmentsStrip
            HStack(alignment: .bottom, spacing: 8) {
                attachMenu
                VStack(alignment: .leading, spacing: 4) {
                    ComposerTextView(
                        text: $app.composerText,
                        onSubmit: { Task { await send() } },
                        onPasteImage: { data, name in addPastedImage(data, name) },
                        onPaletteKey: handleSlashPaletteKey,
                        focused: $inputFocused
                    )
                    // 高度由 sizeThatFits 回报(贴文本实高,夹 38…150):弹性 .frame 无 ideal
                    // 时会按提案拉伸到 maxHeight,空态玻璃卡被撑大,故不用。
                    pickers
                }
                sendControls
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
        }
        // 新 macOS 设计标准:底部悬浮玻璃输入条
        .padding(.horizontal, 12)
        .padding(.bottom, 12)
        .padding(.top, 4)
        .code2Glass(in: RoundedRectangle(cornerRadius: 24), interactive: true)
        .shadow(color: .black.opacity(0.10), radius: 10, y: 3)
        // 拖拽附件:范围限输入区;图片→image 附件、其他→文件附件(Win 输入区无拖拽契约,本壳补充,
        // 分类与 MIME 规则复用 Win AddAttachmentAsync 的扩展名映射,xaml.cs:4235-4266)
        .overlay(
            RoundedRectangle(cornerRadius: 24)
                .strokeBorder(dropTargeted ? Color.accentColor.opacity(0.8) : .clear, lineWidth: 2)
        )
        .onDrop(of: [.image, .data], isTargeted: $dropTargeted) { handleDrop($0) }
        // 斜杠补全浮层:输入以 / 开头时浮出,悬浮在输入条上方(见 slashPaletteOverlay)
        .overlay(alignment: .top) { slashPaletteOverlay }
        // /feedback 内置命令的会话反馈表单(Win 顶栏"会话反馈"同构)
        .sheet(isPresented: $app.sessionFeedbackSheetVisible) {
            SessionFeedbackSheet()
        }
        .onChange(of: app.composerText) { _, text in
            Task { await updateSlashSuggestions(text) }
        }
        .onChange(of: app.phase) { _, phase in
            if phase == .ready { inputFocused = true }
        }
    }

    private func send() async {
        let mode = app.isBusy ? (steerOverride ? "steer" : "queue") : nil
        await app.send(text: app.composerText, mode: mode)
        steerOverride = false
        inputFocused = true
    }

    /// ⌘⏎:繁忙时取 busyEnter 设置的反向档 queue↔steer,空闲时等同普通发送
    /// (Win xaml.cs:4346-4349 Ctrl+Enter 语义;反向档定义 4398-4400 AlternateBusyEnter)。
    private func sendReverseMode() async {
        let mode = app.isBusy ? (app.kernelBusyEnter == "steer" ? "queue" : "steer") : nil
        await app.send(text: app.composerText, mode: mode)
        steerOverride = false
        inputFocused = true
    }

    // MARK: 斜杠补全浮层(Win CommandPalette:UpdateCommandPalette xaml.cs:12661,
    // HandlePaletteKey :12732,AcceptCommandSelection :12780,OnCommandItemClick :12800)

    @State private var slashItems: [SlashSuggestion] = []
    @State private var slashSelection = 0
    @State private var slashQuery = ""
    /// 击键代际:异步取目录期间有更新的按键时丢弃旧响应(同 Win _paletteGeneration)。
    @State private var slashGeneration = 0

    /// 输入文本变化 → 刷新候选:仅前导 "/" 且未出现空白时浮出(与 Win 触发规则一致,
    /// xaml.cs:12661-12670);候选 = 内核目录(commandsFor 的 3s 缓存)+ 壳内置兜底,
    /// 合并、过滤与排序在 AppState.slashSuggestions(同 Win ShowCommandPaletteAsync)。
    private func updateSlashSuggestions(_ text: String) async {
        guard text.hasPrefix("/"),
              !text.contains(where: { $0 == " " || $0 == "\t" || $0 == "\n" || $0 == "\r" }) else {
            slashGeneration += 1
            slashItems = []
            return
        }
        slashGeneration += 1
        let generation = slashGeneration
        let query = String(text.dropFirst())
        let items = await app.slashSuggestions(query: query, sessionId: app.activeSessionId)
        guard generation == slashGeneration else { return }
        slashItems = items
        slashSelection = 0
        slashQuery = query
    }

    /// 浮层打开时的按键拦截;返回 true = 已消费(浮层未开一律 false,输入框行为完全不变)。
    /// 键位与 Win HandlePaletteKey 逐条对齐:↑↓ 移动(不回绕)、Tab/Enter 采纳、Esc 关闭。
    private func handleSlashPaletteKey(_ key: ComposerTextView.PaletteKey) -> Bool {
        guard !slashItems.isEmpty else { return false }
        switch key {
        case .up:
            slashSelection = max(0, slashSelection - 1) // 同 Win MoveCommandSelection:Clamp 不回绕
        case .down:
            slashSelection = min(slashItems.count - 1, slashSelection + 1)
        case .tab, .enter:
            guard slashItems.indices.contains(slashSelection) else { return false }
            acceptSlashSelection(slashItems[slashSelection])
        case .escape:
            closeSlashPalette()
        }
        return true
    }

    /// 采纳:需要参数的命令只补全命令名 + 空格(Win AcceptCommandSelection 的 HasInput 分支,
    /// xaml.cs:12780-12798),无参数命令清空输入框后直接执行。
    private func acceptSlashSelection(_ item: SlashSuggestion) {
        closeSlashPalette()
        if item.hasInput {
            app.composerText = "/\(item.name) "
        } else {
            app.composerText = ""
            let name = item.name
            Task { await app.runSlashSuggestion(name) }
        }
        inputFocused = true // Win OnCommandItemClick:点选后焦点还给输入框
    }

    private func closeSlashPalette() {
        slashGeneration += 1
        slashItems = []
    }

    /// 浮层挂载:用 alignmentGuide 让浮层底边贴输入条顶边、内容向上溢出(overlay 默认不裁剪,
    /// 悬浮展示,不挤压聊天区)。
    @ViewBuilder
    private var slashPaletteOverlay: some View {
        if !slashItems.isEmpty {
            SlashSuggestView(
                query: slashQuery,
                items: slashItems,
                selection: $slashSelection,
                onAccept: { acceptSlashSelection($0) }
            )
            .alignmentGuide(.top) { dimensions in dimensions[.bottom] }
            .offset(y: -6)
        }
    }

    /// 粘贴图片 → 附件(Win OnInputPaste 语义;位图已统一为 PNG 数据,文件名带扩展名时按扩展名映射)。
    private func addPastedImage(_ data: Data, _ name: String) {
        let mediaType = ComposerAttachments.imageMime(forName: name) ?? "image/png"
        app.pendingAttachments.append(PendingAttachment(name: name, mediaType: mediaType, data: data))
    }

    /// 拖拽 → 附件:文件 URL 多文件逐个转换(图片→image、其他→file),纯图片数据走粘贴位图命名。
    private func handleDrop(_ providers: [NSItemProvider]) -> Bool {
        var accepted = false
        let app = app
        for provider in providers {
            if provider.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
                accepted = true
                Task {
                    guard let url = await ComposerAttachments.fileURL(from: provider),
                          let att = ComposerAttachments.attachmentFromFileURL(url) else { return }
                    app.pendingAttachments.append(att)
                }
            } else if provider.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
                accepted = true
                Task {
                    guard let data = await ComposerAttachments.imageData(from: provider),
                          let att = ComposerAttachments.attachmentFromImageData(data, suggestedName: nil) else { return }
                    app.pendingAttachments.append(att)
                }
            }
        }
        return accepted
    }

    // MARK: 附件

    @ViewBuilder
    private var attachmentsStrip: some View {
        if !app.pendingAttachments.isEmpty {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(app.pendingAttachments) { att in
                        attachmentChip(att)
                    }
                }
                .padding(.horizontal, 14)
                .padding(.top, 8)
            }
        }
    }

    private func attachmentChip(_ att: PendingAttachment) -> some View {
        HStack(spacing: 6) {
            Image(systemName: att.isImage ? "photo" : "doc")
            Text(att.name).lineLimit(1).frame(maxWidth: 160)
            let size = ByteCountFormatter.string(fromByteCount: Int64(att.data.count), countStyle: .file)
            Text(size).font(.caption2).foregroundStyle(.secondary)
            Button {
                app.pendingAttachments.removeAll { $0.id == att.id }
            } label: {
                Image(systemName: "xmark.circle.fill")
            }
            .buttonStyle(.plain)
            .foregroundStyle(.secondary)
        }
        .font(.caption)
        .padding(.horizontal, 9)
        .padding(.vertical, 5)
        .background(.quinary, in: Capsule())
    }

    private var attachMenu: some View {
        Menu {
            Button(L("添加图片…")) { addAttachments(images: true) }
            Button(L("添加文件…")) { addAttachments(images: false) }
        } label: {
            Image(systemName: "plus.circle")
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .padding(.bottom, 6)
        .help(L("添加附件"))
    }

    private func addAttachments(images: Bool) {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = true
        panel.canChooseDirectories = false
        if images {
            panel.allowedContentTypes = [.image]
        } else {
            panel.allowedContentTypes = [.data]
        }
        guard panel.runModal() == .OK else { return }
        for url in panel.urls {
            guard let data = try? Data(contentsOf: url) else { continue }
            let type = attachmentsType(for: url)
            app.pendingAttachments.append(PendingAttachment(name: url.lastPathComponent, mediaType: type, data: data))
        }
    }

    private func attachmentsType(for url: URL) -> String {
        ComposerAttachments.fileMime(for: url)
    }

    // MARK: 选择器(模型 / 工作区 / 预设)

    @ViewBuilder
    private var pickers: some View {
        HStack(spacing: 10) {
            modelMenu
            if app.activeSessionId == nil {
                workspaceMenu
                presetMenu
            }
            permissionMenu
            planChip
            Spacer()
            if app.isBusy {
                Text(steerOverride ? L("追问(立即送达)") : L("排队(回合结束后送达)"))
                    .font(.caption2).foregroundStyle(.secondary)
            }
        }
    }

    // MARK: 权限档位(仅可查看 / 工作区内修改 / 完全权限)
    //
    // 内核契约(Win MainWindow.xaml.cs):plan/permissions **没有任何对应 RPC**(xaml.cs:1384-1391
    // 明示:内核 15 个 type 里没有 plan/permission 命名空间),session/create 只带
    // workspaceId/cwd/agentPreset(xaml.cs:3195-3221),session/prompt 也不带权限键。
    // 两条路:
    // - 空态(尚无活动会话):显示并编辑 settings 的 permission.defaultPreset(默认 workspace-write),
    //   写入 settings/mutate {ns:"permission",ops:[{op:"set",path:["defaultPreset"],value}]}
    //   (Win xaml.cs:11132-11212;新会话按该默认档开局)。Mac 走 AppState.mutateSetting 的既有
    //   去抖分组,最终 RPC 形态与 Win 一致。
    // - 会话态:读 permissions 投影 {options:[{value,description?}],currentValue}(xaml.cs:10747-10753,
    //   来源 session/control 流与 session/list 快照),写入发 /permission <preset> 命令(xaml.cs:11385-11397)。

    @ViewBuilder
    private var permissionMenu: some View {
        if app.activeSessionId == nil {
            permissionPicker(choices: emptyStatePermissionChoices, current: app.defaultPermissionPreset) { value in
                selectDefaultPermission(value)
            }
        } else if let sessionPermission = app.sessionPermission,
                  !sessionPermission.options.isEmpty, sessionPermission.current != nil {
            // 会话态:投影 options 为空或 currentValue 缺失时收起(Win xaml.cs:11020-11023)
            permissionPicker(choices: sessionPermission.options, current: sessionPermission.current ?? "") { value in
                Task { await app.setSessionPermissionPreset(value) }
            }
        }
    }

    private func permissionPicker(choices: [PermissionPresetOption], current: String,
                                  action: @escaping (String) -> Void) -> some View {
        Menu {
            ForEach(choices, id: \.value) { choice in
                Button {
                    action(choice.value)
                } label: {
                    if choice.value == current {
                        Label(permissionPresetName(choice.value), systemImage: "checkmark")
                    } else {
                        Text(permissionPresetName(choice.value))
                    }
                }
                .help(permissionPresetDesc(choice.value, choice.description))
            }
        } label: {
            Label(permissionPresetName(current), systemImage: "lock.shield")
                .font(.caption)
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .frame(maxWidth: 180)
        .help(LF("访问模式，当前：{0}", permissionPresetName(current))) // Win xaml.cs:11030
    }

    /// 空态可选档:settings/describe 的 permission.defaultPreset union(Win xaml.cs:11146-11152),
    /// schema 缺失时回落三档(11153-11161)。
    private var emptyStatePermissionChoices: [PermissionPresetOption] {
        var choices: [PermissionPresetOption] = []
        if let namespace = app.settingsNamespaces.first(where: { $0.ns == "permission" }) {
            let field = namespace.rehydrated()["dict"]["defaultPreset"]
            for item in SettingsNamespace.unionOptions(field) {
                guard let value = item.value.stringValue else { continue }
                choices.append(PermissionPresetOption(value: value, description: item.description))
            }
        }
        if choices.isEmpty {
            choices = [
                PermissionPresetOption(value: "read-only", description: L("只读：可浏览工作区，不能写文件或执行修改")),
                PermissionPresetOption(value: "workspace-write", description: L("可写工作区与允许的临时目录；更广范围的重试需审批")),
                PermissionPresetOption(value: "danger-full-access", description: L("完全文件访问，不再弹出审批确认")),
            ]
        }
        return choices
    }

    /// 空态选择 → settings/mutate permission.defaultPreset(Win xaml.cs:11190-11212)。
    private func selectDefaultPermission(_ value: String) {
        app.mutateSetting(ns: "permission", path: ["defaultPreset"], value: .string(value))
    }

    /// 权限预设中文名(Win xaml.cs:11062-11069 PermissionPresetZh;投影 name 是英文 id,必须映射)。
    private func permissionPresetName(_ value: String) -> String {
        switch value {
        case "read-only": return L("仅可查看")
        case "workspace-write": return L("工作区内修改")
        case "danger-full-access": return L("完全权限")
        case "custom": return L("自定义")
        default: return value
        }
    }

    /// 权限预设说明(Win xaml.cs:11072-11086 PermissionPresetDescZh:已知档位用产品文案,未知回落内核描述)。
    private func permissionPresetDesc(_ value: String, _ kernelDescription: String) -> String {
        switch value {
        case "read-only": return L("只读：可浏览工作区，不能写文件或执行修改")
        case "workspace-write": return L("可写工作区与允许的临时目录；更广范围的重试需审批")
        case "danger-full-access": return L("完全文件访问，不再弹出审批确认")
        default: return kernelDescription
        }
    }

    // MARK: 计划模式 chip
    //
    // 内核契约(非壳私有):plan 投影 {active,pending}(Win xaml.cs:10735-10745,dsh-plan-mode),
    // chip 仅在开启中/切换中出现、关闭态不占输入区(xaml.cs:11002-11016);
    // 退出走 /plan off 命令(xaml.cs:11369-11383;命令回执经 commands/execute,投影随后刷新)。

    @ViewBuilder
    private var planChip: some View {
        if let plan = app.planState, plan.active || plan.pending {
            HStack(spacing: 4) {
                Image(systemName: "list.bullet.clipboard")
                Text(plan.pending ? L("计划模式（切换中…）") : L("计划模式已开启"))
                    .font(.caption)
                if plan.active { // 退出按钮只在 active 时可见(Win xaml.cs:11015)
                    Button {
                        Task { await app.togglePlanMode() }
                    } label: {
                        Image(systemName: "xmark.circle")
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(.secondary)
                    .help(L("退出计划模式"))
                }
            }
            .padding(.horizontal, 9)
            .padding(.vertical, 5)
            .background(.quinary, in: Capsule())
        }
    }

    private var modelMenu: some View {
        Menu {
            ForEach(providersInCatalog, id: \.self) { provider in
                Menu(provider) {
                    ForEach(app.catalog.models.filter { $0.provider == provider }) { model in
                        Button(model.display) {
                            app.selectedProvider = model.provider
                            app.selectedModel = model.id
                            app.selectedEffort = model.defaultEffort
                        }
                    }
                }
            }
            if !efforts.isEmpty {
                Menu(L("推理等级")) {
                    ForEach(efforts, id: \.self) { effort in
                        Button(effort.isEmpty ? L("默认") : effort) {
                            app.selectedEffort = effort
                        }
                    }
                }
            }
        } label: {
            Label(currentModelLabel, systemImage: "cpu")
                .font(.caption)
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .frame(maxWidth: 260)
    }

    private var providersInCatalog: [String] {
        var seen: [String] = []
        for m in app.catalog.models where !seen.contains(m.provider) { seen.append(m.provider) }
        return seen
    }

    private var currentEfforts: [String] {
        app.catalog.models.first(where: { $0.provider == app.selectedProvider && $0.id == app.selectedModel })?.efforts ?? []
    }
    private var efforts: [String] { currentEfforts }

    private var currentModelLabel: String {
        let model = app.catalog.models.first(where: { $0.provider == app.selectedProvider && $0.id == app.selectedModel })
        let name = model?.display ?? app.selectedModel
        let effort = app.selectedEffort.isEmpty ? "" : " · \(app.selectedEffort)"
        return name + effort
    }

    private var workspaceMenu: some View {
        Menu {
            Button(L("不指定(使用文稿目录)")) { app.newSessionWorkspaceId = nil }
            ForEach(app.workspaces) { ws in
                Button(ws.title.isEmpty ? ws.path : ws.title) {
                    app.newSessionWorkspaceId = ws.workspaceId
                }
            }
        } label: {
            Label(workspaceLabel, systemImage: "folder")
                .font(.caption)
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .frame(maxWidth: 220)
    }

    private var workspaceLabel: String {
        guard let wid = app.newSessionWorkspaceId,
              let ws = app.workspaces.first(where: { $0.workspaceId == wid }) else { return L("新会话工作区") }
        return ws.title.isEmpty ? ws.path : ws.title
    }

    @State private var presets: [(id: String, name: String)] = []

    private var presetMenu: some View {
        Menu {
            Button(L("默认预设")) { app.newSessionAgentPreset = nil }
            ForEach(presets, id: \.id) { p in
                Button(p.name.isEmpty ? p.id : p.name) { app.newSessionAgentPreset = p.id }
            }
        } label: {
            Label(presetLabel, systemImage: "person.crop.square.badge.clock")
                .font(.caption)
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .frame(maxWidth: 200)
        .task { await loadPresets() }
    }

    private var presetLabel: String {
        guard let pid = app.newSessionAgentPreset else { return L("预设") }
        return presets.first(where: { $0.id == pid })?.name ?? pid
    }

    private func loadPresets() async {
        guard let rpc = app.rpcClient else { return }
        if let value = try? await rpc.callOk("agentPresets/list", .object([:])) {
            presets = (value["presets"].arrayValue ?? []).compactMap { p in
                guard let id = p["id"].stringValue else { return nil }
                return (id, p["name"].stringValue ?? "")
            }
        }
    }

    // MARK: 发送 / 停止

    private var sendControls: some View {
        HStack(spacing: 8) {
            if app.isBusy {
                Picker("", selection: $steerOverride) {
                    Text(L("排队")).tag(false)
                    Text(L("追问")).tag(true)
                }
                .pickerStyle(.segmented)
                .frame(width: 120)
                Button {
                    Task { await app.cancelActive() }
                } label: {
                    Image(systemName: "stop.fill")
                        .foregroundStyle(.red)
                }
                .buttonStyle(.bordered)
                .help(L("停止当前回合"))
            }
            Button {
                Task { await send() }
            } label: {
                Image(systemName: "paperplane.fill")
            }
            .code2ProminentButton()
            .disabled(app.composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && app.pendingAttachments.isEmpty)
            // 提示与 queue/steer 分段控件联动(同 Win 语义:发送按钮本身是普通发送,xaml.cs:4402-4413)
            .help(app.isBusy
                  ? (steerOverride ? L("追问(立即送达)") : L("排队(回合结束后送达)"))
                  : L("发送"))
            // ⌘⏎ 承载在隐藏按钮上:busy=busyEnter 反向档、空闲=普通发送(Win xaml.cs:4346-4349、4398-4400)。
            // 不能挂在发送按钮上,否则点击与快捷键无法区分。
            .background {
                Button("") {
                    Task { await sendReverseMode() }
                }
                .keyboardShortcut(.return, modifiers: .command)
                .opacity(0)
                .frame(width: 0, height: 0)
                .accessibilityHidden(true)
            }
        }
        .padding(.bottom, 4)
    }
}

// MARK: - 多行输入(⏎ 发送、⇧⏎ 换行 —— NSTextView doCommandBy)

struct ComposerTextView: NSViewRepresentable {
    /// 斜杠补全浮层的键位(ComposerTextView.PaletteKey)。
    enum PaletteKey { case up, down, tab, enter, escape }

    @Binding var text: String
    let onSubmit: () -> Void
    /// 粘贴图片回调(数据 + 附件名);由 PasteAwareTextView 触发(Win xaml.cs:4212-4233)。
    var onPasteImage: (Data, String) -> Void
    /// 斜杠浮层键位拦截(Win HandlePaletteKey xaml.cs:12732-12766):返回 true = 浮层已消费。
    /// 浮层未打开时实现方必须返回 false,输入框行为不变(⏎ 发送 / ⇧⏎ 换行 / 粘贴照旧)。
    var onPaletteKey: (PaletteKey) -> Bool
    var focused: FocusState<Bool>.Binding

    func makeNSView(context: Context) -> NSTextView {
        let tv = PasteAwareTextView()
        tv.isRichText = false
        tv.importsGraphics = false
        tv.allowsUndo = true
        tv.font = NSFont.systemFont(ofSize: NSFont.systemFontSize)
        tv.isVerticallyResizable = true
        tv.isHorizontallyResizable = false
        tv.textContainer?.widthTracksTextView = true
        tv.autoresizingMask = [.width]
        tv.delegate = context.coordinator
        tv.drawsBackground = false
        tv.focusRingType = .none
        tv.string = text
        // 经 coordinator 转发:updateNSView 每帧刷新 parent,回调始终拿到最新闭包
        let coordinator = context.coordinator
        tv.onPasteImage = { [coordinator] data, name in
            coordinator.parent.onPasteImage(data, name)
        }
        return tv
    }

    func updateNSView(_ nsView: NSTextView, context: Context) {
        if nsView.string != text {
            nsView.string = text
            (nsView as? PasteAwareTextView)?.invalidateHeight()
        }
        context.coordinator.parent = self
        if focused.wrappedValue && nsView.window?.firstResponder !== nsView {
            nsView.window?.makeFirstResponder(nsView)
        }
    }

    /// 输入区高度贴内容:AppKit representable 的默认测尺寸走 fittingSize(NSTextView 会给
    /// 出偏大的值),这里按文本实高回报,夹在 38…150;空态玻璃卡不再被撑到最大高。
    func sizeThatFits(_ proposal: ProposedViewSize, nsView: NSTextView, context: Context) -> CGSize? {
        var used: CGFloat = 18
        if let layout = nsView.layoutManager, let container = nsView.textContainer {
            used = layout.usedRect(for: container).height
        }
        let h = min(max(used + nsView.textContainerInset.height * 2 + 6, 38), 150)
        return CGSize(width: proposal.width ?? nsView.frame.width, height: h)
    }

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    final class Coordinator: NSObject, NSTextViewDelegate {
        var parent: ComposerTextView
        init(_ parent: ComposerTextView) { self.parent = parent }

        func textDidChange(_ notification: Notification) {
            guard let tv = notification.object as? NSTextView else { return }
            parent.text = tv.string
            (tv as? PasteAwareTextView)?.invalidateHeight()
        }

        func textView(_ textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            // ⏎ 发送;⇧⏎ 原生换行(与主流 Mac 聊天应用一致)
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                if let event = textView.window?.currentEvent, event.modifierFlags.contains(.shift) {
                    return false // 交给系统换行(浮层打开时 ⇧⏎ 仍是换行,同 Win HandlePaletteKey)
                }
                if parent.onPaletteKey(.enter) { return true } // 浮层打开:⏎ = 采纳,不发送
                parent.onSubmit()
                return true
            }
            // 浮层键位:↑↓ 导航 / Tab 采纳 / Esc 关闭;浮层未开时 onPaletteKey 返回 false,系统接管
            if commandSelector == #selector(NSResponder.moveUp(_:)) { return parent.onPaletteKey(.up) }
            if commandSelector == #selector(NSResponder.moveDown(_:)) { return parent.onPaletteKey(.down) }
            if commandSelector == #selector(NSResponder.insertTab(_:)) { return parent.onPaletteKey(.tab) }
            if commandSelector == #selector(NSResponder.cancelOperation(_:)) { return parent.onPaletteKey(.escape) }
            return false
        }
    }
}

/// 会话反馈表单(/feedback 内置命令与 Win 顶栏"会话反馈"按钮同构;Win OnSessionFeedbackClick
/// xaml.cs:11402-11489):分类 7 选 1(可留空)+ 说明文本;提交 sessionFeedback/record。
/// text/category 在内核是 optional(非 nullable)——为空时**不带键**,带 null 会被 zod 拒。
private struct SessionFeedbackSheet: View {
    /// 内核 FEEDBACK_CATEGORIES 线值与中文标签(顺序即产品呈现顺序;Win xaml.cs:11491-11499)。
    private static let categories: [(wire: String, labelKey: String)] = [
        ("task-result", "任务结果"),
        ("instruction-following", "指令遵循"),
        ("product-interaction", "产品交互"),
        ("service-stability", "服务稳定性"),
        ("resource-cost", "资源消耗"),
        ("security-privacy-permission", "安全隐私与权限"),
        ("other", "其他"),
    ]

    @Environment(AppState.self) private var app
    @Environment(\.dismiss) private var dismiss
    @State private var categoryIndex: Int? = nil // nil = 不选(线格式省略 category 键)
    @State private var text = ""
    @State private var submitting = false

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(L("会话反馈"))
                .font(.headline)
            Picker(L("分类（可留空）"), selection: $categoryIndex) {
                Text(L("（不选择分类）")).tag(Int?.none)
                ForEach(Self.categories.indices, id: \.self) { index in
                    Text(L(Self.categories[index].labelKey)).tag(Int?.some(index))
                }
            }
            .pickerStyle(.menu)
            VStack(alignment: .leading, spacing: 4) {
                Text(L("说明（可留空）"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                TextEditor(text: $text)
                    .frame(height: 96)
                    .scrollContentBackground(.hidden)
                    .background(.quinary, in: RoundedRectangle(cornerRadius: 6))
                    .overlay(alignment: .topLeading) {
                        if text.isEmpty {
                            Text(L("这条会话的体验如何？"))
                                .foregroundStyle(.tertiary)
                                .padding(.top, 8)
                                .padding(.leading, 5)
                                .allowsHitTesting(false)
                        }
                    }
            }
            HStack {
                Spacer()
                Button(L("取消")) { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Button(L("记录")) { submit() }
                    .keyboardShortcut(.defaultAction)
                    .buttonStyle(.borderedProminent)
                    .disabled(submitting)
            }
        }
        .padding(18)
        .frame(width: 380)
    }

    private func submit() {
        submitting = true
        let category = categoryIndex.flatMap { Self.categories.indices.contains($0) ? Self.categories[$0].wire : nil }
        Task {
            await app.submitSessionFeedback(category: category, text: text)
            submitting = false
            dismiss()
        }
    }
}
