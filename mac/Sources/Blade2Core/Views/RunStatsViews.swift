import SwiftUI

// MARK: - 运行状态条 + 统计面板 + 作业面板
//
// 对标 Win 版:状态条两条胶囊 + 点开的"会话统计"/"Token 用量"面板(MainWindow.RunStats.cs:212-312
// 与 MainWindow.xaml.cs:699-704),作业面板(Win RefreshJobsPanel/MakeJobRow,xaml.cs:11098-11136,
// 11654-11697)。文案逐条对齐 Win 的 L/LF 键。

/// 运行状态条:左胶囊「{轮} 轮 {步} 步 · {tok/s}」→ 会话统计面板;
/// 右胶囊「{累计} tok · 缓存命中 {%}」→ Token 用量面板。
/// 当前会话没有已完成步时整条隐藏(Win UpdateRunStatsStrip,RunStats.cs:213-234)。
struct RunStatsStrip: View {
    @Environment(AppState.self) private var app
    @State private var showStatsPanel = false
    @State private var showTokenPanel = false

    var body: some View {
        Group {
            if let st = app.runStats.state(for: app.activeSessionId), st.steps > 0 {
                HStack(spacing: 8) {
                    capsule(LF("{0} 轮 {1} 步 · {2} tok/s", st.turns, st.steps, st.tpsText), $showStatsPanel)
                        .popover(isPresented: $showStatsPanel, arrowEdge: .bottom) {
                            SessionStatsPanel(state: st)
                        }
                    // 右胶囊显式拼接:单位与 % 各出现一次——LF 模板字面量 + 值内单位会叠加(%% / tok tok 事故)
                    capsule(
                        st.tokensCompactText(st.totalTokens) + " tok · " + L("缓存命中") + " " + st.cacheHitText + "%",
                        $showTokenPanel
                    )
                    .popover(isPresented: $showTokenPanel, arrowEdge: .bottom) {
                        TokenUsagePanel(state: st)
                    }
                }
            }
        }
    }

    private func capsule(_ text: String, _ expanded: Binding<Bool>) -> some View {
        Button {
            expanded.wrappedValue = true
        } label: {
            Text(text)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .padding(.horizontal, 10)
                .padding(.vertical, 4)
                .background(.quinary, in: Capsule())
        }
        .buttonStyle(.plain)
    }
}

/// 面板容器:卡片底 + 1px 描边(Win ShowRunFlyout,RunStats.cs:348-363)。
private struct RunStatsPanelHost<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            content
        }
        .padding(14)
        .frame(minWidth: 250, alignment: .leading)
        .background(.background, in: RoundedRectangle(cornerRadius: 8))
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .strokeBorder(Color.primary.opacity(0.12), lineWidth: 1)
        )
    }
}

/// 「label 在左、value 右对齐」的一行(Win StatRow,RunStats.cs:334-346)。
private struct StatRow: View {
    let label: String
    let value: String

    var body: some View {
        HStack(spacing: 16) {
            Text(label)
                .foregroundStyle(.secondary)
            Spacer(minLength: 0)
            Text(value)
        }
        .font(.callout)
    }
}

private struct PanelSeparator: View {
    var body: some View {
        Rectangle().fill(Color.primary.opacity(0.12)).frame(height: 1)
    }
}

/// 会话统计面板(Win ShowStatsPanel,RunStats.cs:264-280):
/// 模型用时 / 工具调用用时 / 首 token 平均(TTFT)/ 输出速度(TPS)。
private struct SessionStatsPanel: View {
    let state: RunStatsState

    var body: some View {
        RunStatsPanelHost {
            Text(L("会话统计"))
                .fontWeight(.semibold)
            PanelSeparator()
            StatRow(label: L("模型用时"), value: state.durationText(state.llmMs))
            StatRow(label: L("工具调用用时"), value: state.durationText(state.toolMs))
            StatRow(
                label: L("首 token 平均（TTFT）"),
                value: state.ttftSteps > 0 ? state.durationText(state.ttftMs / Double(state.ttftSteps)) : "—"
            )
            StatRow(label: L("输出速度（TPS）"), value: state.tpsText + " tok/s")
        }
    }
}

/// Token 用量面板(Win ShowTokenPanel,RunStats.cs:282-312):
/// 标题行右侧总量,下接缓存命中 / 未缓存输入 / 缓存读取 / 输出。
private struct TokenUsagePanel: View {
    let state: RunStatsState

    var body: some View {
        RunStatsPanelHost {
            HStack(spacing: 16) {
                Text(L("Token 用量")).fontWeight(.semibold)
                Spacer(minLength: 0)
                Text(state.tokensN0Text(state.totalTokens))
            }
            .font(.callout)
            PanelSeparator()
            StatRow(label: L("缓存命中"), value: state.cacheHitText + "%")
            StatRow(label: L("未缓存输入"), value: state.tokensN0Text(state.inputTokens))
            StatRow(label: L("缓存读取"), value: state.tokensN0Text(state.cacheReadTokens))
            StatRow(label: L("输出"), value: state.tokensN0Text(state.outputTokens))
        }
    }
}

/// 作业面板入口(header 图标,仅当前会话有作业时显示):
/// 运行中在前、已结束在后;状态点 running 蓝 / failed·killed 红 / 其余灰(Win MakeJobRow)。
struct JobsPanelButton: View {
    @Environment(AppState.self) private var app
    @State private var showPanel = false

    var body: some View {
        Group {
            if !app.activeSessionJobs.isEmpty {
                Button {
                    showPanel = true
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "list.bullet.rectangle")
                            .font(.callout)
                        Text(LF("作业 · {0} 个", app.activeSessionJobs.count))
                            .font(.caption)
                    }
                    .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .help(L("作业"))
                .popover(isPresented: $showPanel, arrowEdge: .bottom) {
                    JobsPanel(jobs: app.activeSessionJobs)
                }
            }
        }
    }
}

/// 作业面板(Win RefreshJobsPanel xaml.cs:11098-11136):头部「作业 · {0} 个」
/// + 「{0} 个运行中」/「均已完成」,行 = 状态点 + 标签(· 详情)+ 状态文案。
private struct JobsPanel: View {
    /// 活动会话的作业,运行中在前、已结束在后(Win active.Concat(done))
    let jobs: [KernelJob]

    private var activeCount: Int { jobs.filter(\.isActive).count }

    var body: some View {
        RunStatsPanelHost {
            VStack(alignment: .leading, spacing: 4) {
                Text(LF("作业 · {0} 个", jobs.count)).fontWeight(.semibold)
                Text(activeCount > 0 ? LF("{0} 个运行中", activeCount) : L("均已完成"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            PanelSeparator()
            ForEach(jobs) { job in
                jobRow(job)
            }
        }
    }

    private func jobRow(_ job: KernelJob) -> some View {
        let line = job.detail.isEmpty ? job.label : job.label + " · " + job.detail
        return HStack(spacing: 8) {
            Circle()
                .fill(dotColor(job.status))
                .frame(width: 7, height: 7)
            Text(line)
                .font(.caption)
                .lineLimit(1)
                .truncationMode(.tail)
                .help(line)
            Spacer(minLength: 8)
            Text(job.statusLabel)
                .font(.caption)
                .foregroundStyle(.tertiary)
        }
    }

    /// 状态点配色(Win MakeJobRow:running→Info,failed/killed→Error,其余三级文本色)。
    private func dotColor(_ status: String) -> Color {
        switch status {
        case "running": return .blue
        case "failed", "killed": return .red
        default: return .gray
        }
    }
}
