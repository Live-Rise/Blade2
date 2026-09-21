import Foundation

// MARK: - 用量统计模型(壳侧计算,无 usage RPC;口径对齐 Win 版统计面板 MainWindow.xaml.cs:9749-10576)
// 与 Models.swift 的旧 UsageSummary 并存:旧类型不再被 UsageView 使用,保留避免动共享文件。
//   · KPI(累计/峰值/连续天数)按全部历史;时间范围只作用于趋势图与模型用量(Win:10173)。
//   · 活跃日 = 有 token 消耗的本地日历日(Win Streaks:10136)。
//   · 最长聊天时长 = 单会话各 turn(start→end)时长之和的最大值,不是会话存活窗口(Win:10126)。

/// 单模型用量合计(趋势序列与占比条共用,按窗口内合计降序)。
struct UsageModelTotal: Identifiable, Equatable {
    let label: String
    let tokens: Int
    var id: String { label }
}

/// 单日合计(yyyy-MM-dd 为本地时区日键)。
struct UsageDayTotal: Identifiable, Equatable {
    let day: String
    let tokens: Int
    var id: String { day }
}

/// 单日×单模型 token(多模型多色趋势的数据矩阵)。
struct UsageDayModelCell: Equatable {
    let day: String
    let model: String
    let tokens: Int
}

/// 一次全量扫描的统计快照(KPI/热力图按全部历史;窗口筛选由视图侧用 perDayModel 现算)。
struct UsageStats: Equatable {
    var totalTokens = 0
    var sessionsCount = 0            // 走查的非空会话数
    var talkSeconds = 0              // 全部会话 turn 时长之和
    var longestTalkSeconds = 0       // 「最长聊天时长」:单会话 turn 时长和的最大值
    var peakTokens = 0               // 单日峰值
    var peakDay = ""                 // 峰值日 yyyy-MM-dd
    var currentStreakDays = 0        // 当前连续活跃天数(从今天或昨天往回数)
    var longestStreakDays = 0        // 最长连续活跃天数
    var perDay: [UsageDayTotal] = []
    var perDayModel: [UsageDayModelCell] = []
    var perModel: [UsageModelTotal] = []
    var usageMessages = 0            // 计入的 assistant/message 条数(口径自证)
    var sessionsSkipped = 0          // 跳过的空会话/无游标会话数
    var sessionsCapped = 0           // 触到单会话 40 页上限的会话数(部分计入)
    var partial: Bool { sessionsCapped > 0 }
}

/// 热力图口径(对应 Win StatsHeatMetricBar 的 day/week/total,MainWindow.xaml:1149-1156)。
enum UsageHeatMetric: String, CaseIterable, Identifiable {
    case daily = "每日"
    case weekly = "每周"
    case cumulative = "累计"
    var id: String { rawValue }
}

/// 趋势图单模型序列(values 与窗口日序一一对应,缺日补 0;按窗口合计降序)。
struct UsageTrendSeries: Identifiable, Equatable {
    let model: String
    let values: [Int]
    var total: Int { values.reduce(0, +) }
    var id: String { model }
}

/// 趋势图绘制点(日×模型)。
struct UsageTrendPoint: Identifiable, Equatable {
    let day: Date
    let model: String
    let tokens: Int
    var id: String { "\(model)#\(day.timeIntervalSince1970)" }
}

/// 热力图单元格(列 = 周,行 = 周一…周日;Win Canvas 自绘的 Swift Charts 等价物)。
struct UsageHeatCell: Identifiable, Equatable {
    let dayKey: String
    let weekStart: Date // 该列的周一(固定周一起始,不随 locale firstWeekday,Win:10189)
    let dow: Int        // 0=周一 … 6=周日
    let tokens: Int
    var id: String { dayKey }
}

/// 派生指标计算(纯函数;视图切口径/范围时现算,不进 AppState)。
enum UsageMath {
    /// 本地日历日键 yyyy-MM-dd(同 Win DayKey:10077;走 Calendar 不用 DateFormatter,免静态可变依赖)。
    static func dayKey(_ date: Date, calendar: Calendar = .current) -> String {
        let c = calendar.dateComponents([.year, .month, .day], from: date)
        return String(format: "%04d-%02d-%02d", c.year ?? 0, c.month ?? 0, c.day ?? 0)
    }

    /// 日键 → 本地日历日零点。
    private static func date(fromKey key: String, calendar: Calendar) -> Date? {
        let parts = key.split(separator: "-")
        guard parts.count == 3,
              let y = Int(parts[0]), let m = Int(parts[1]), let d = Int(parts[2]) else { return nil }
        return calendar.date(from: DateComponents(year: y, month: m, day: d))
    }

    /// 距本周一的偏移天数(0=周一…6=周日;weekday 1=周日…7=周六,Win:10189 同式)。
    private static func weekdayOffset(_ date: Date, _ calendar: Calendar) -> Int {
        (calendar.component(.weekday, from: date) + 5) % 7
    }

    /// 近 N 天的日键(含今天,升序;对应 Win RangeDays:10580)。
    static func rangeDayKeys(days: Int, calendar: Calendar = .current, now: Date = Date()) -> [String] {
        guard days > 0 else { return [] }
        let today = calendar.startOfDay(for: now)
        return (0..<days).reversed().compactMap {
            calendar.date(byAdding: .day, value: -$0, to: today).map { dayKey($0, calendar: calendar) }
        }
    }

    /// 窗口内多模型趋势:全 N 天补零,序列按窗口合计降序(对应 Win RenderStatsTrend:10307-10339)。
    static func trendSeries(perDayModel: [UsageDayModelCell], rangeDays: Int,
                            calendar: Calendar = .current, now: Date = Date())
        -> (days: [Date], series: [UsageTrendSeries]) {
        let keys = rangeDayKeys(days: rangeDays, calendar: calendar, now: now)
        let keySet = Set(keys)
        var byModelDay: [String: [String: Int]] = [:]
        for row in perDayModel where keySet.contains(row.day) {
            byModelDay[row.model, default: [:]][row.day, default: 0] += row.tokens
        }
        let days = keys.compactMap { date(fromKey: $0, calendar: calendar) }
        let series = byModelDay
            .map { model, byDay in UsageTrendSeries(model: model, values: keys.map { byDay[$0] ?? 0 }) }
            .sorted { $0.total > $1.total }
        return (days, series)
    }

    /// 热力图单元格(近 26 周窗口,末列 = 本周;跳过未来日期;对应 Win RenderStatsHeatmap:10179-10276)。
    /// daily = 当日合计;weekly = 整周合计(周内每天同值,一眼看出整周强度,Win:10201-10210);
    /// cumulative = 窗口起点起的逐日累计(Win:10212-10214)。
    static func heatCells(perDay: [UsageDayTotal], metric: UsageHeatMetric, weeks: Int = 26,
                          calendar: Calendar = .current, now: Date = Date()) -> [UsageHeatCell] {
        var byDay: [String: Int] = [:]
        for d in perDay where d.tokens > 0 { byDay[d.day] = d.tokens }
        let today = calendar.startOfDay(for: now)
        guard let lastMonday = calendar.date(byAdding: .day, value: -weekdayOffset(today, calendar), to: today),
              let start = calendar.date(byAdding: .day, value: -(weeks - 1) * 7, to: lastMonday) else { return [] }

        // 每周口径:同一周内 7 天都取该周合计
        func weekSum(of day: Date) -> Int {
            guard let monday = calendar.date(byAdding: .day, value: -weekdayOffset(day, calendar), to: day) else { return 0 }
            var sum = 0
            for k in 0..<7 {
                if let wd = calendar.date(byAdding: .day, value: k, to: monday) {
                    sum += byDay[dayKey(wd, calendar: calendar)] ?? 0
                }
            }
            return sum
        }

        var cells: [UsageHeatCell] = []
        var running = 0
        for i in 0..<(weeks * 7) {
            guard let day = calendar.date(byAdding: .day, value: i, to: start) else { continue }
            guard day <= today else { break }
            let key = dayKey(day, calendar: calendar)
            let dayTotal = byDay[key] ?? 0
            let value: Int
            switch metric {
            case .daily: value = dayTotal
            case .weekly: value = weekSum(of: day)
            case .cumulative:
                running += dayTotal
                value = running
            }
            let dow = weekdayOffset(day, calendar)
            guard let monday = calendar.date(byAdding: .day, value: -dow, to: day) else { continue }
            cells.append(UsageHeatCell(dayKey: key, weekStart: monday, dow: dow, tokens: value))
        }
        return cells
    }

    /// 热力档位 0(空)…4:按 value/max 比值,阈值同 Win HeatLevel:10278(≤0.25/≤0.5/≤0.75/其余)。
    static func heatLevel(_ value: Int, max: Int) -> Int {
        guard value > 0, max > 0 else { return 0 }
        let ratio = Double(value) / Double(max)
        if ratio <= 0.25 { return 1 }
        if ratio <= 0.5 { return 2 }
        if ratio <= 0.75 { return 3 }
        return 4
    }

    /// 连续活跃天数:活跃 = 当日有 token 消耗;最长为全部历史最长,当前从今天(今天没用就昨天)往回数
    /// (口径同 Win Streaks:10136-10163)。
    static func streaks(dayKeys: [String], calendar: Calendar = .current, now: Date = Date())
        -> (current: Int, longest: Int) {
        let days = Set(dayKeys).compactMap { date(fromKey: $0, calendar: calendar) }.sorted()
        guard let first = days.first else { return (0, 0) }
        var longest = 1
        var run = 1
        var previous = first
        for day in days.dropFirst() {
            if let next = calendar.date(byAdding: .day, value: 1, to: previous), day == next {
                run += 1
            } else {
                run = 1
            }
            if run > longest { longest = run }
            previous = day
        }
        let activeDays = Set(days)
        let today = calendar.startOfDay(for: now)
        var cursor = activeDays.contains(today)
            ? today
            : (calendar.date(byAdding: .day, value: -1, to: today) ?? today)
        var current = 0
        while activeDays.contains(cursor), current < days.count { // current 上限防异常日历死循环
            current += 1
            guard let prior = calendar.date(byAdding: .day, value: -1, to: cursor) else { break }
            cursor = prior
        }
        return (current, longest)
    }
}
