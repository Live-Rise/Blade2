import SwiftUI
import Charts

/// 用量页(壳侧计算:session/list + session/page 回向翻页):5 KPI + Token 活动热力图 + 多模型趋势 + 模型占比。
/// 口径对齐 Win 版统计面板(MainWindow.xaml.cs:9749-10576):KPI/热力图按全部历史,
/// 时间范围只作用于趋势图与模型用量;Win 自绘 canvas,macOS 用 Swift Charts 原生渲染。
struct UsageView: View {
    @Environment(AppState.self) private var app
    @State private var loading = false
    @State private var heatMetric: UsageHeatMetric = .daily // 热力口径(对应 Win StatsHeatMetricBar,默认每日)
    @State private var rangeDays = 7                        // 时间范围(对应 Win StatsRangeBar,默认近 7 天)

    /// 趋势/占比条共用系列色(与 Win ChartSeries1..6 同思路:超出色板数循环取色)。
    private static let seriesPalette: [Color] = [.blue, .orange, .purple, .teal, .pink, .indigo]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                headerRow
                if let usage = app.usage {
                    content(usage)
                } else if loading {
                    Text(L("正在扫描会话日志…")).foregroundStyle(.secondary)
                } else {
                    Text(L("点击「重新计算」扫描全部会话日志。"))
                        .foregroundStyle(.secondary)
                }
            }
            .padding(22)
            .frame(maxWidth: 900, alignment: .leading)
        }
        .task {
            if app.usage == nil {
                loading = true
                await app.computeUsage()
                loading = false
            }
        }
    }

    @ViewBuilder
    private func content(_ usage: UsageStats) -> some View {
        let window = UsageMath.trendSeries(perDayModel: usage.perDayModel, rangeDays: rangeDays)
        kpiRow(usage)
        heatCard(usage)
        rangeRow
        trendCard(window)
        modelCard(window)
        sourceNote(usage)
    }

    // MARK: - 工具栏(刷新 + 加载环 + 部分数据告警)

    private var headerRow: some View {
        HStack {
            Button(L("重新计算")) {
                loading = true
                Task { await app.computeUsage(); loading = false }
            }
            .buttonStyle(.bordered)
            .disabled(loading)
            if loading { ProgressView().controlSize(.small) }
            Spacer()
            if app.usage?.partial == true {
                Text(L("部分会话超过 40 页上限,统计不完整"))
                    .font(.caption).foregroundStyle(.orange)
            }
        }
    }

    // MARK: - 5 KPI(累计/峰值/最长聊天时长/当前连续/最长连续;零值显示「—」不编造,同 Win:10102-10134)

    private func kpiRow(_ usage: UsageStats) -> some View {
        HStack(spacing: 10) {
            kpi(L("累计 Token 数"), usage.totalTokens > 0 ? format(usage.totalTokens) : "—")
            kpi(L("峰值 Token 数"), usage.peakTokens > 0 ? format(usage.peakTokens) : "—",
                help: usage.peakTokens > 0 ? LF("单日峰值：{0}，{1} tokens", usage.peakDay, format(usage.peakTokens)) : nil)
            kpi(L("最长聊天时长"), usage.longestTalkSeconds > 0 ? duration(usage.longestTalkSeconds) : "—",
                help: L("单会话对话累计时长最大值（按 turn/start→turn/end 求和）"))
            kpi(L("当前连续天数"), usage.currentStreakDays > 0 ? LF("{0} 天", usage.currentStreakDays) : "—")
            kpi(L("最长连续天数"), usage.longestStreakDays > 0 ? LF("{0} 天", usage.longestStreakDays) : "—")
        }
    }

    private func kpi(_ title: String, _ value: String, help: String? = nil) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).font(.caption).foregroundStyle(.secondary)
            Text(value).font(.system(size: 22, weight: .semibold))
                .lineLimit(1)
                .minimumScaleFactor(0.6)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(12)
        .background(.quinary, in: RoundedRectangle(cornerRadius: 12))
        .help(help ?? title)
    }

    // MARK: - Token 活动热力图(周列×星期行;口径切换 每日/每周/累计,Win:10179-10276)

    private func heatCard(_ usage: UsageStats) -> some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    Text(L("Token 活动")).font(.headline)
                    Spacer()
                    Picker(L("统计口径"), selection: $heatMetric) {
                        ForEach(UsageHeatMetric.allCases) { metric in
                            Text(L(metric.rawValue)).tag(metric)
                        }
                    }
                    .pickerStyle(.segmented)
                    .controlSize(.small)
                    .fixedSize()
                    .help(L("口径按全部历史窗口(近 26 周)计算"))
                }
                heatChart(usage)
                heatLegend
            }
            .padding(6)
        }
    }

    private func heatChart(_ usage: UsageStats) -> some View {
        let cells = UsageMath.heatCells(perDay: usage.perDay, metric: heatMetric)
        let maxV = cells.map(\.tokens).max() ?? 0
        return Chart(cells) { cell in
            RectangleMark(
                x: .value(L("周"), cell.weekStart, unit: .weekOfYear),
                y: .value(L("星期"), cell.dow),
                width: .ratio(0.82),
                height: .ratio(0.82)
            )
            .cornerRadius(2)
            .foregroundStyle(Self.heatColor(level: UsageMath.heatLevel(cell.tokens, max: maxV)))
        }
        .chartXAxis {
            AxisMarks(values: .stride(by: .month)) {
                AxisValueLabel(format: .dateTime.month(.narrow))
            }
        }
        .chartYAxis(.hidden) // 行 = 周一…周日固定顺序;Win 版同样不画行标签
        .frame(height: 132)
    }

    /// 热力档位色:空档 + 强调色 4 档不透明度(25/45/70/100%,同 Win HeatBrush:10295)。
    private static func heatColor(level: Int) -> Color {
        switch level {
        case 1: return Color.accentColor.opacity(0.25)
        case 2: return Color.accentColor.opacity(0.45)
        case 3: return Color.accentColor.opacity(0.7)
        case 4: return Color.accentColor
        default: return Color.secondary.opacity(0.12)
        }
    }

    private var heatLegend: some View {
        HStack(spacing: 5) {
            Spacer()
            Text(L("少")).font(.caption2).foregroundStyle(.secondary)
            ForEach(0..<5, id: \.self) { level in
                RoundedRectangle(cornerRadius: 2)
                    .fill(Self.heatColor(level: level))
                    .frame(width: 11, height: 11)
            }
            Text(L("多")).font(.caption2).foregroundStyle(.secondary)
        }
    }

    // MARK: - 时间范围(只作用于趋势图与模型用量,Win:9850-9862 / 10173)

    private var rangeRow: some View {
        HStack {
            Text(L("时间范围")).font(.headline)
            Spacer()
            Picker(L("时间范围"), selection: $rangeDays) {
                Text(L("近 7 天")).tag(7)
                Text(L("近 30 天")).tag(30)
            }
            .pickerStyle(.segmented)
            .controlSize(.small)
            .fixedSize()
        }
    }

    // MARK: - 每日 Token 趋势(多模型多色折线,序列按窗口合计降序,Win:10307-10528)

    private func trendCard(_ window: (days: [Date], series: [UsageTrendSeries])) -> some View {
        GroupBox(L("每日 Token 趋势图")) {
            VStack(alignment: .leading, spacing: 10) {
                if window.series.isEmpty {
                    Text(L("所选时间范围内暂无用量记录"))
                        .font(.caption).foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity, minHeight: 120) // 空态不画空坐标轴(同 Win:10346-10364)
                } else {
                    trendLegend(window.series)
                    trendChart(window)
                }
            }
            .padding(6)
        }
    }

    /// 图例与曲线同序同色;模型多时横向滚动(展示全部,不合并 TopN——与 Win 图例行为一致)。
    private func trendLegend(_ series: [UsageTrendSeries]) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 14) {
                ForEach(Array(series.enumerated()), id: \.offset) { index, series in
                    HStack(spacing: 5) {
                        RoundedRectangle(cornerRadius: 2)
                            .fill(Self.seriesPalette[index % Self.seriesPalette.count])
                            .frame(width: 10, height: 10)
                        Text(series.model).font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
        }
    }

    private func trendChart(_ window: (days: [Date], series: [UsageTrendSeries])) -> some View {
        let scaleColors = window.series.indices.map { Self.seriesPalette[$0 % Self.seriesPalette.count] }
        return Chart {
            ForEach(window.series) { series in // 面积垫底不盖折线(同 Win:10437 的两遍绘制)
                ForEach(Self.trendPoints(series, days: window.days)) { point in
                    AreaMark(x: .value(L("日期"), point.day), y: .value(L("Token"), point.tokens))
                        .foregroundStyle(by: .value(L("模型"), point.model))
                        .opacity(0.10)
                }
            }
            ForEach(window.series) { series in
                ForEach(Self.trendPoints(series, days: window.days)) { point in
                    LineMark(x: .value(L("日期"), point.day), y: .value(L("Token"), point.tokens))
                        .foregroundStyle(by: .value(L("模型"), point.model))
                }
            }
        }
        .chartForegroundStyleScale(domain: window.series.map(\.model), range: scaleColors)
        .chartXAxis {
            AxisMarks(values: .automatic(desiredCount: 6))
        }
        .frame(height: 200)
    }

    private static func trendPoints(_ series: UsageTrendSeries, days: [Date]) -> [UsageTrendPoint] {
        zip(days, series.values).map { UsageTrendPoint(day: $0, model: series.model, tokens: $1) }
    }

    // MARK: - 模型用量占比(横向条列表 + 合计;行同构 Win StatsModelVm,MainWindow.xaml:1246-1264)

    private func modelCard(_ window: (days: [Date], series: [UsageTrendSeries])) -> some View {
        let total = window.series.map(\.total).reduce(0, +)
        return GroupBox {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    Text(L("模型用量")).font(.headline)
                    Spacer()
                    if total > 0 {
                        Text(LF("合计 {0} tokens", format(total)))
                            .font(.caption).foregroundStyle(.secondary)
                    }
                }
                if total <= 0 {
                    Text(L("所选时间范围内暂无用量记录"))
                        .font(.caption).foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity)
                } else {
                    ForEach(window.series.indices, id: \.self) { index in
                        modelRow(window.series[index], index: index, total: total)
                    }
                }
            }
            .padding(6)
        }
    }

    private func modelRow(_ series: UsageTrendSeries, index: Int, total: Int) -> some View {
        let share = total > 0 ? Double(series.total) / Double(total) * 100 : 0
        return VStack(alignment: .leading, spacing: 5) {
            HStack {
                Text(series.model).lineLimit(1).truncationMode(.middle)
                Spacer()
                Text("\(format(series.total)) tokens")
            }
            ProgressView(value: share, total: 100)
                .tint(Self.seriesPalette[index % Self.seriesPalette.count])
            Text(LF("占比 {0}%", String(format: "%.0f", share)))
                .font(.caption).foregroundStyle(.secondary)
        }
    }

    // MARK: - 数据来源与口径脚注(同 Win RenderStatsSourceNote:10165-10175)

    private func sourceNote(_ usage: UsageStats) -> some View {
        var note = LF("数据来源：内核 journal（session/list + session/page）的用量记录；已聚合 {0} 个非空会话、{1} 条记录",
                      usage.sessionsCount, usage.usageMessages)
        if usage.sessionsSkipped > 0 {
            note += LF("，跳过 {0} 个空会话", usage.sessionsSkipped)
        }
        if usage.sessionsCapped > 0 {
            note += LF("；{0} 个超长会话触到分页上限，其数据为部分计入", usage.sessionsCapped)
        }
        note += "。"
        note += L("「累计/峰值/连续天数」按全部历史，「时间范围」只作用于趋势图与模型用量；")
        note += L("费用/金额内核未提供对应字段，故不展示。")
        return Text(note)
            .font(.caption2).foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func format(_ tokens: Int) -> String {
        if tokens >= 1_000_000 { return String(format: "%.1fM", Double(tokens) / 1_000_000) }
        if tokens >= 1_000 { return String(format: "%.1fk", Double(tokens) / 1_000) }
        return String(tokens)
    }

    private func duration(_ seconds: Int) -> String {
        let h = seconds / 3600, m = (seconds % 3600) / 60
        if h > 0 { return "\(h)h \(m)m" }
        return "\(m)m"
    }
}
