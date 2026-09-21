import SwiftUI

/// 聊天页:气泡滚动区 + 悬浮审批/提问卡 + 底部输入区。
struct ChatPageView: View {
    @Environment(AppState.self) private var app

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            ZStack {
                if app.heroVisible {
                    HeroView()
                } else {
                    BubbleList()
                }
                // 审批卡:置顶悬浮(单活跃)
                if let approval = app.activeApproval {
                    ApprovalCard(request: approval)
                        .padding(.top, 10)
                        .frame(maxHeight: .infinity, alignment: .top)
                        .transition(.move(edge: .top).combined(with: .opacity))
                }
                // 提问卡:居中模态感(单活跃)
                if let question = app.activeQuestion {
                    QuestionOverlay(request: question)
                        .transition(.opacity)
                }
            }
            ComposerView()
        }
        .animation(.easeOut(duration: 0.18), value: app.activeApproval?.eventId)
        .animation(.easeOut(duration: 0.18), value: app.activeQuestion?.eventId)
    }

    private var header: some View {
        HStack(spacing: 10) {
            if app.isBusy {
                ProgressView().controlSize(.small)
                Text(L("运行中…")).font(.caption).foregroundStyle(.secondary)
            }
            // 运行状态条:两胶囊点开"会话统计"/"Token 用量"面板(Win 版 composer 上方状态条同构)
            RunStatsStrip()
            Spacer()
            // 作业面板入口:当前会话有任务时显示(数据源 session/control 的 jobs 帧)
            JobsPanelButton()
            if app.activeSessionId != nil {
                Menu {
                    Button(L("打开工作目录")) {
                        Task { await app.openWorkspacePath(app.activeSessionId ?? "") }
                    }
                    Button(L("分叉会话")) {
                        Task { await app.forkSession(app.activeSessionId ?? "") }
                    }
                } label: {
                    Image(systemName: "ellipsis.circle")
                }
                .menuStyle(.borderlessButton)
                .fixedSize()
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 7)
    }
}

/// 气泡列表(新内容自动滚底)。
struct BubbleList: View {
    @Environment(AppState.self) private var app

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 10) {
                    // visibleBubbles = 紧凑模式展示层投影(全量仍在 app.bubbles,切回标准不丢内容)
                    ForEach(app.visibleBubbles) { bubble in
                        BubbleView(bubble: bubble)
                            .id(bubble.id)
                    }
                    Color.clear.frame(height: 6).id("bottom-anchor")
                }
                .padding(.horizontal, 18)
                .padding(.vertical, 14)
            }
            .defaultScrollAnchor(.bottom)
            .onChange(of: app.bubbles.count) { _, _ in
                withAnimation(.easeOut(duration: 0.15)) {
                    proxy.scrollTo("bottom-anchor", anchor: .bottom)
                }
            }
        }
    }
}

// MARK: - 气泡

struct BubbleView: View {
    @Environment(AppState.self) private var app
    let bubble: ChatBubble
    @State private var hovering = false
    @State private var showingNoteEditor = false
    @State private var noteDraft = ""

    var body: some View {
        Group {
            switch bubble.role {
            case .user: userBubble
            case .userImage: userImageBubble
            case .assistant: assistantBubble
            case .reasoning: reasoningBubble
            case .tool: toolBubble
            case .error: errorBubble
            case .deliverable: deliverableBubble
            case .system: systemBubble
            }
        }
        // 挂在常驻的 Group 上:行内视图随悬停消失,alert 不能挂在会消失的子树上
        .alert(L("反馈说明"), isPresented: $showingNoteEditor) {
            TextField(L("这条回复哪里好 / 哪里不好（可选，纯文本）"), text: $noteDraft)
            Button(L("保存")) {
                if let mid = bubble.messageId {
                    Task { await app.saveFeedbackNote(mid, note: noteDraft) }
                }
            }
            Button(L("取消"), role: .cancel) {}
        }
    }

    // 用户:右对齐、强调色底(与 Win 版三态气泡一致);文字色按 WCAG 亮度选黑/白
    private var userBubble: some View {
        HStack(alignment: .bottom) {
            Spacer(minLength: 60)
            Text(bubble.text)
                .font(.system(size: CGFloat(app.themeFontSize)))
                .textSelection(.enabled)
                .foregroundStyle(Color.accentColor.readableTextColor)
                .padding(.horizontal, 13)
                .padding(.vertical, 9)
                .background(Color.accentColor, in: BubbleShape())
        }
    }

    private var userImageBubble: some View {
        HStack(alignment: .bottom) {
            Spacer(minLength: 60)
            VStack(alignment: .trailing, spacing: 3) {
                if let data = bubble.imageData, let image = NSImage(data: data) {
                    Image(nsImage: image)
                        .resizable()
                        .aspectRatio(contentMode: .fit)
                        .frame(maxWidth: 340, maxHeight: 300)
                        .clipShape(RoundedRectangle(cornerRadius: 12))
                        .help(bubble.text)
                } else {
                    Label(bubble.text, systemImage: "photo").foregroundStyle(.secondary)
                }
            }
        }
    }

    // 助手:左对齐 markdown(代码块卡片 + inline 段落,无 WebView)
    private var assistantBubble: some View {
        HStack(alignment: .top) {
            VStack(alignment: .leading, spacing: 5) {
                MarkdownView(text: bubble.text, fontSize: Double(app.themeFontSize))
                    .frame(maxWidth: 760, alignment: .leading)
                if hovering, let mid = bubble.messageId, !mid.isEmpty {
                    HStack(spacing: 4) {
                        feedbackButton("hand.thumbsup", active: rating == "positive", role: "positive")
                        feedbackButton("hand.thumbsdown", active: rating == "negative", role: "negative")
                        // 已评价才开放说明(Win 版 xaml.cs:11838 RenderFeedbackRow)
                        if !rating.isEmpty {
                            Button {
                                noteDraft = app.feedbackNote(messageId: mid)
                                showingNoteEditor = true
                            } label: {
                                Text(app.feedbackNote(messageId: mid).isEmpty ? L("添加说明") : L("编辑说明"))
                                    .font(.caption)
                            }
                            .buttonStyle(.borderless)
                            .foregroundStyle(Color.secondary)
                        }
                    }
                }
            }
            Spacer(minLength: 60)
        }
        .onHover { hovering = $0 }
    }

    private var rating: String { app.feedbackRating(messageId: bubble.messageId) }

    private func feedbackButton(_ systemImage: String, active: Bool, role: String) -> some View {
        Button {
            Task { await app.rateMessage(bubble.messageId ?? "", rating: role) }
        } label: {
            Image(systemName: active ? systemImage + ".fill" : systemImage)
                .font(.caption)
        }
        .buttonStyle(.borderless)
        .foregroundStyle(active ? Color.accentColor : Color.secondary)
    }

    private var reasoningBubble: some View {
        HStack(alignment: .top) {
            DisclosureGroup {
                MarkdownView(text: bubble.text, fontSize: 12.5)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: 720, alignment: .leading)
            } label: {
                Label(L("思考过程"), systemImage: "brain")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer(minLength: 60)
        }
    }

    private var toolBubble: some View {
        HStack {
            Text(bubble.text)
                .font(.system(size: 12, design: .monospaced))
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .textSelection(.enabled)
            Spacer(minLength: 60)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 4)
        .background(.quinary, in: Capsule())
    }

    private var errorBubble: some View {
        HStack(alignment: .top) {
            Label {
                Text(bubble.text)
                    .textSelection(.enabled)
                    .frame(maxWidth: 720, alignment: .leading)
            } icon: {
                Image(systemName: "exclamationmark.octagon.fill")
            }
            .foregroundStyle(.red)
            Spacer(minLength: 60)
        }
        .padding(10)
        .background(Color.red.opacity(0.09), in: RoundedRectangle(cornerRadius: 10))
    }

    /// 交付物:files 数组下标即 present.open 坐标,open/reveal 两动作。
    private var deliverableBubble: some View {
        HStack(alignment: .top) {
            VStack(alignment: .leading, spacing: 6) {
                Label(L("交付物"), systemImage: "shippingbox")
                    .font(.caption).foregroundStyle(.secondary)
                ForEach(bubble.files ?? []) { file in
                    HStack(spacing: 8) {
                        Text((file.path as NSString).lastPathComponent)
                            .lineLimit(1)
                            .help(file.path)
                        if !file.description.isEmpty {
                            Text(file.description).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                        }
                        Button(L("打开")) { Task { await app.openPresentedFile(file, reveal: false) } }
                            .buttonStyle(.link).font(.caption)
                        Button(L("定位")) { Task { await app.openPresentedFile(file, reveal: true) } }
                            .buttonStyle(.link).font(.caption)
                    }
                }
            }
            .padding(10)
            .frame(maxWidth: 560, alignment: .leading)
            .background(.quinary, in: RoundedRectangle(cornerRadius: 10))
            Spacer(minLength: 60)
        }
    }

    private var systemBubble: some View {
        Text(bubble.text)
            .font(.caption)
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .center)
            .padding(.vertical, 2)
    }
}

/// 用户消息气泡圆角。
struct BubbleShape: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        path.addRoundedRect(in: rect, cornerSize: CGSize(width: 15, height: 15), style: .continuous)
        return path
    }
}

// MARK: - 强调色对比度

private extension Color {
    /// 强调色底上的可靠文字色:按 sRGB WCAG 相对亮度,亮底黑字、暗底白字。
    var readableTextColor: Color {
        // SwiftUI Color 无公开反向转换,经 NSColor 桥接取 sRGB 分量;失败退回白字
        guard let srgb = NSColor(self).usingColorSpace(.sRGB) else { return .white }
        let luminance = wcagRelativeLuminance(
            red: Double(srgb.redComponent),
            green: Double(srgb.greenComponent),
            blue: Double(srgb.blueComponent)
        )
        // 与黑/白对比取更大者,临界亮度 (L+0.05)/0.05 == 1.05/(L+0.05) → L ≈ 0.1791
        return luminance > 0.1791 ? .black : .white
    }
}

/// WCAG 相对亮度(线性化 sRGB 通道)。
private func wcagRelativeLuminance(red: Double, green: Double, blue: Double) -> Double {
    func linear(_ channel: Double) -> Double {
        channel <= 0.03928 ? channel / 12.92 : pow((channel + 0.055) / 1.055, 2.4)
    }
    return 0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
}
