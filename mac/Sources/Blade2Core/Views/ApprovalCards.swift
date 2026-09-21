import SwiftUI

/// 审批卡(waterfall approval/request):单活跃悬浮;Allow → allowed-once / Reject → rejected。
/// 失败保留卡片允许重试(与 Win 版一致)。
struct ApprovalCard: View {
    @Environment(AppState.self) private var app
    let request: ApprovalRequest
    @State private var working = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Label(L("需要你的许可"), systemImage: "hand.raised.fill")
                .font(.headline)
                .foregroundStyle(.orange)
            HStack(spacing: 8) {
                Image(systemName: "wrench.and.screwdriver")
                Text(request.toolName).fontWeight(.semibold)
            }
            if !request.reason.isEmpty {
                Text(request.reason)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
            }
            if let error = app.approvalSubmitError {
                Text(LF("提交失败:{0}", error)).font(.caption).foregroundStyle(.red)
            }
            HStack {
                Spacer()
                Button(L("拒绝"), role: .destructive) {
                    working = true
                    Task { await app.answerApproval(request, allowed: false); working = false }
                }
                .disabled(working)
                Button(L("允许一次")) {
                    working = true
                    Task { await app.answerApproval(request, allowed: true); working = false }
                }
                .code2ProminentButton()
                .disabled(working)
            }
        }
        .padding(14)
        .frame(width: 420)
        .code2Glass(in: RoundedRectangle(cornerRadius: 18), interactive: true)
        .shadow(color: .black.opacity(0.14), radius: 16, y: 5)
    }
}

/// 用户提问卡(user-questions/request):单选/多选 + 自由文本;plan-review 呈现计划正文。
/// 提交 = {answers:[{id, selected:[labels], custom?}]};跳过 = {kind:"next"}。
struct QuestionOverlay: View {
    @Environment(AppState.self) private var app
    let request: QuestionRequest

    var body: some View {
        ZStack {
            Color.black.opacity(0.18).ignoresSafeArea()
            QuestionCard(request: request)
                .frame(maxWidth: 620, maxHeight: 560)
                .code2Glass(in: RoundedRectangle(cornerRadius: 20))
                .shadow(color: .black.opacity(0.18), radius: 20, y: 6)
                .padding(30)
        }
    }
}

struct QuestionCard: View {
    @Environment(AppState.self) private var app
    let request: QuestionRequest
    @State private var selections: [String: Set<String>] = [:]
    @State private var customTexts: [String: String] = [:]
    @State private var working = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            ForEach(request.questions) { q in
                questionSection(q)
            }
            if let error = app.questionSubmitError {
                Text(LF("提交失败:{0}", error)).font(.caption).foregroundStyle(.red)
            }
            HStack {
                Button(L("跳过")) {
                    working = true
                    Task { await app.skipQuestion(request); working = false }
                }
                .disabled(working)
                Spacer()
                Button(L("提交")) { submit() }
                    .code2ProminentButton()
                    .disabled(working || !allAnswered)
            }
        }
        .padding(18)
    }

    private var allAnswered: Bool {
        request.questions.allSatisfy { q in
            let chosen = selections[q.id]?.isEmpty == false
            let custom = !(customTexts[q.id] ?? "").trimmingCharacters(in: .whitespaces).isEmpty
            return chosen || custom || q.options.isEmpty
        }
    }

    @ViewBuilder
    private func questionSection(_ q: QuestionItem) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            if !q.header.isEmpty {
                Text(q.header).font(.caption).foregroundStyle(.secondary)
            }
            Text(q.question).font(.headline)
            // plan review:detail 是计划 markdown
            if q.intentKind == "plan-review", !q.detail.isEmpty {
                ScrollView {
                    MarkdownView(text: q.detail, fontSize: Double(app.themeFontSize))
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .frame(maxHeight: 190)
                .background(.quinary, in: RoundedRectangle(cornerRadius: 8))
            }
            if q.multiSelect {
                ForEach(q.options) { opt in
                    Toggle(isOn: multiBinding(q.id, opt.label)) {
                        optionLabel(opt)
                    }
                    .toggleStyle(.checkbox)
                }
            } else {
                // 单选:原生 radio 组(语义同 Toggle 版:selections 仍存 [label])
                Picker(selection: singleSelectionBinding(q.id)) {
                    ForEach(q.options) { opt in
                        optionLabel(opt).tag(opt.label)
                    }
                } label: { EmptyView() }
                .pickerStyle(.radioGroup)
            }
            TextField(L("其他(自定义回答,可选)"), text: customBinding(q.id))
                .textFieldStyle(.roundedBorder)
        }
        .onAppear {
            if selections[q.id] == nil { selections[q.id] = [] }
        }
    }

    private func multiBinding(_ qid: String, _ label: String) -> Binding<Bool> {
        Binding(
            get: { self.selections[qid]?.contains(label) ?? false },
            set: { on in
                var set = self.selections[qid] ?? []
                if on { set.insert(label) } else { set.remove(label) }
                self.selections[qid] = set
            }
        )
    }

    /// 单选 Picker 绑定:空串 = 未选;写入仍归一为 selections[qid] = [label]。
    private func singleSelectionBinding(_ qid: String) -> Binding<String> {
        Binding(
            get: { self.selections[qid]?.first ?? "" },
            set: { label in self.selections[qid] = label.isEmpty ? [] : [label] }
        )
    }

    private func customBinding(_ qid: String) -> Binding<String> {
        Binding(
            get: { self.customTexts[qid] ?? "" },
            set: { self.customTexts[qid] = $0 }
        )
    }

    private func optionLabel(_ opt: QuestionOption) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(opt.label)
            if !opt.description.isEmpty {
                Text(opt.description).font(.caption).foregroundStyle(.secondary)
            }
        }
    }

    private func submit() {
        working = true
        let answers = request.questions.map { q -> (String, [String], String?) in
            let selected = Array(selections[q.id] ?? [])
            let custom = (customTexts[q.id] ?? "").trimmingCharacters(in: .whitespaces)
            return (q.id, selected, custom.isEmpty ? nil : custom)
        }
        Task {
            await app.answerQuestions(request, answers: answers)
            working = false
        }
    }
}
