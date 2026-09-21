import SwiftUI

/// 设置页:与 Win 版同构的分区 —— general / models / plugins / agent-presets。
/// general 等命名空间由 settings/describe 的 schemastery refs 树驱动渲染;
/// 写入走 settings/mutate(按 ns 去抖分组),重置走 settings/replace。
struct SettingsView: View {
    @Environment(AppState.self) private var app
    @State private var section = "general"

    static let sectionOrder = ["general", "models", "plugins", "agent-presets"]
    static let sectionTitles = [
        "general": "通用",
        "models": "模型",
        "plugins": "插件",
        "agent-presets": "代理预设",
    ]

    var body: some View {
        HStack(spacing: 0) {
            List(selection: $section) {
                ForEach(SettingsView.sectionOrder, id: \.self) { key in
                    Label(L(SettingsView.sectionTitles[key] ?? key), systemImage: icon(key))
                        .tag(key)
                }
            }
            .listStyle(.sidebar)
            .navigationSplitViewColumnWidth(min: 170, ideal: 190, max: 240)

            Group {
                switch section {
                case "models": ModelsSection()
                case "plugins": PluginsSection()
                case "agent-presets": AgentPresetsSection()
                default: GeneralSection()
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
    }

    private func icon(_ key: String) -> String {
        switch key {
        case "models": return "cpu"
        case "plugins": return "puzzlepiece.extension"
        case "agent-presets": return "person.crop.square.badge.clock"
        default: return "switch.2"
        }
    }
}

// MARK: - 通用(schema 驱动)

/// 通用设置 = 壳侧策划页(与 Win 版同构,MainWindow.xaml.cs:6779-6856):
/// 内核 0.7.7 无 general 命名空间,本页聚合 ui-theme / ui-chat / ui-conversation /
/// permission 四个命名空间的壳相关字段;其余插件命名空间照旧走"插件"节。
struct GeneralSection: View {
    @Environment(AppState.self) private var app

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                uiLanguageRow
                themeRow
                fontSizeRow
                transcriptRow
                busyEnterRow
                permissionRow
                resetBar
            }
            .padding(22)
            .frame(maxWidth: 760, alignment: .leading)
        }
        .task {
            try? await app.refreshSettingsSnapshot()
        }
    }

    /// 壳侧界面语言:存本地 UserDefaults(L10n.localeKey),不进内核 general 设置树
    /// (对应 Win 版壳内建字段 MakeUiLanguageField,MainWindow.xaml.cs:6409,同样只映射 zh/en 回写)。
    private var uiLanguageRow: some View {
        LabeledRow(title: L("界面语言"), hint: L("仅本壳的显示语言;切换后同步内核命令描述语言")) {
            Picker("", selection: Binding(
                get: { L10n.storedOption },
                set: { option in Task { await app.setUiLocale(option) } }
            )) {
                Text(L("跟随系统")).tag("system")
                Text("简体中文").tag("zh-Hans")
                Text("English").tag("en")
            }
            .labelsHidden()
            .frame(maxWidth: 240)
        }
    }

    /// 主题三选(ui-theme.preference;Win 同款三态,xaml.cs:6790 一带)。
    private var themeRow: some View {
        LabeledRow(title: L("主题"), hint: L("跟随系统将实时响应系统深浅色切换")) {
            Picker("", selection: nsPickerBinding(ns: "ui-theme", path: ["preference"], fallback: "system")) {
                Label(L("浅色"), systemImage: "sun.max").tag("light")
                Label(L("深色"), systemImage: "moon").tag("dark")
                Label(L("跟随系统"), systemImage: "circle.lefthalf.filled").tag("system")
            }
            .labelsHidden()
            .frame(maxWidth: 240)
        }
    }

    /// 字号 stepper(ui-theme.fontSize;内核 zod 限 12-17,见 dsh-client-ui-theme)。
    private var fontSizeRow: some View {
        LabeledRow(title: L("对话字号"), hint: L("作用与会话正文(bubble)基础字号")) {
            Stepper(value: Binding(
                get: { app.themeFontSize },
                set: { app.mutateSetting(ns: "ui-theme", path: ["fontSize"], value: .int($0)) }
            ), in: 12...17) {
                Text("\(app.themeFontSize) pt").monospacedDigit()
            }
        }
    }

    /// 对话显示 标准/紧凑(ui-chat.transcriptView;内核默认 compact,见 dsh-client-ui-chat)。
    private var transcriptRow: some View {
        LabeledRow(title: L("对话显示"), hint: L("紧凑模式折叠已完成轮次的过程气泡")) {
            Picker("", selection: nsPickerBinding(ns: "ui-chat", path: ["transcriptView"], fallback: "compact")) {
                Text(L("标准")).tag("normal")
                Text(L("紧凑")).tag("compact")
            }
            .labelsHidden()
            .pickerStyle(.segmented)
            .frame(maxWidth: 240)
        }
    }

    /// 繁忙时发送行为 排队/插话(ui-conversation.busyEnter;Win xaml.cs:4536 同款键)。
    private var busyEnterRow: some View {
        LabeledRow(title: L("繁忙时发送"), hint: L("回合进行中再次发送的入队方式;⌘⏎ 总是发反向档")) {
            Picker("", selection: nsPickerBinding(ns: "ui-conversation", path: ["busyEnter"], fallback: "queue")) {
                Text(L("排队")).tag("queue")
                Text(L("插话")).tag("steer")
            }
            .labelsHidden()
            .pickerStyle(.segmented)
            .frame(maxWidth: 240)
        }
    }

    /// 默认权限档(permission.defaultPreset;Win xaml.cs:11144 同款键,枚举见 Win 11062-11069)。
    private var permissionRow: some View {
        LabeledRow(title: L("默认权限"), hint: L("新会话的访问模式")) {
            Picker("", selection: nsPickerBinding(ns: "permission", path: ["defaultPreset"], fallback: "workspace-write")) {
                Text(L("仅可查看")).tag("read-only")
                Text(L("工作区内修改")).tag("workspace-write")
                Text(L("完全权限")).tag("danger-full-access")
            }
            .labelsHidden()
            .frame(maxWidth: 240)
        }
    }

    private func nsPickerBinding(ns: String, path: [String], fallback: String) -> Binding<String> {
        Binding(
            get: { app.settingsValue(ns: ns, path: path).stringValue ?? fallback },
            set: { app.mutateSetting(ns: ns, path: path, value: .string($0)) }
        )
    }

    /// 恢复本页默认:清空四个聚合命名空间的用户覆盖(settings/replace 空 section)。
    private var resetBar: some View {
        HStack {
            Spacer()
            Button(L("恢复本页默认")) {
                Task {
                    for ns in ["ui-theme", "ui-chat", "ui-conversation", "permission"] {
                        await app.replaceSetting(ns: ns)
                    }
                }
            }
            .buttonStyle(.bordered)
        }
    }
}

/// schemastery 字段渲染:union→Picker、boolean→Toggle、number→TextField、string→TextField、object→递归分组。
struct SchemaFields: View {
    @Environment(AppState.self) private var app
    let ns: SettingsNamespace
    @State private var expandAll = false

    var body: some View {
        let fields = ns.rehydrated()
        VStack(alignment: .leading, spacing: 12) {
            ForEach(Array((fields.objectValue ?? [:]).keys).sorted(), id: \.self) { key in
                schemaNode(ns, path: [key], key: key, node: fields[key])
            }
        }
    }

    private func schemaNode(_ ns: SettingsNamespace, path: [String], key: String, node: JSON) -> AnyView {
        let current = value(at: path, ns: ns)
        let options = SettingsNamespace.unionOptions(node)
        let type = node["type"].stringValue

        if !options.isEmpty {
            return AnyView(
                LabeledRow(title: key, hint: options.first(where: { $0.value == current })?.description ?? node["meta"]["description"].stringValue ?? "") {
                    Picker("", selection: Binding(
                        get: { describe(current) },
                        set: { app.mutateSetting(ns: ns.ns, path: path, value: parse($0)) }
                    )) {
                        ForEach(Array(options.enumerated()), id: \.offset) { _, opt in
                            Text(constLabel(opt.value)).tag(describe(opt.value))
                        }
                    }
                    .labelsHidden()
                    .frame(maxWidth: 240)
                }
            )
        }
        if type == "boolean" || current.boolValue != nil {
            return AnyView(
                LabeledRow(title: key, hint: node["meta"]["description"].stringValue ?? "") {
                    Toggle("", isOn: Binding(
                        get: { current.boolValue ?? false },
                        set: { app.mutateSetting(ns: ns.ns, path: path, value: .bool($0)) }
                    ))
                    .labelsHidden()
                }
            )
        }
        if type == "object" || node["dict"].objectValue != nil {
            return AnyView(
                GroupBox {
                    VStack(alignment: .leading, spacing: 10) {
                        Text(key).font(.callout).fontWeight(.semibold)
                        let dict = node["dict"]
                        ForEach(Array((dict.objectValue ?? [:]).keys).sorted(), id: \.self) { child in
                            schemaNode(ns, path: path + [child], key: child, node: dict[child])
                        }
                    }
                    .padding(6)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            )
        }
        if current.intValue != nil || type == "number" {
            return AnyView(
                LabeledRow(title: key, hint: node["meta"]["description"].stringValue ?? "") {
                    TextField("", value: Binding(
                        get: { current.intValue ?? 0 },
                        set: { app.mutateSetting(ns: ns.ns, path: path, value: .int($0)) }
                    ), formatter: NumberFormatter())
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 120)
                }
            )
        }
        if current.stringValue != nil || type == "string" {
            return AnyView(
                LabeledRow(title: key, hint: node["meta"]["description"].stringValue ?? "") {
                    TextField("", text: Binding(
                        get: { current.stringValue ?? "" },
                        set: { app.mutateSetting(ns: ns.ns, path: path, value: .string($0)) }
                    ))
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 240)
                }
            )
        }
        return AnyView(EmptyView())
    }

    private func value(at path: [String], ns: SettingsNamespace) -> JSON {
        app.settingsValue(ns: ns.ns, path: path)
    }

    private func describe(_ value: JSON) -> String {
        switch value {
        case .string(let s): return s
        case .int(let i): return "int:\(i)"
        case .double(let d): return "dbl:\(d)"
        case .bool(let b): return "bool:\(b)"
        default: return "null"
        }
    }

    private func parse(_ tag: String) -> JSON {
        if tag.hasPrefix("int:") { return .int(Int(tag.dropFirst(4)) ?? 0) }
        if tag.hasPrefix("dbl:") { return .double(Double(tag.dropFirst(4)) ?? 0) }
        if tag.hasPrefix("bool:") { return .bool(tag.dropFirst(5) == "true") }
        if tag == "null" { return .null }
        return .string(tag)
    }

    private func constLabel(_ value: JSON) -> String {
        switch value {
        case .string(let s): return s
        case .int(let i): return String(i)
        case .double(let d): return String(d)
        case .bool(let b): return b ? "true" : "false"
        case .null: return "(null)"
        default: return value.description
        }
    }
}

struct LabeledRow<Content: View>: View {
    let title: String
    let hint: String
    @ViewBuilder let content: Content

    var body: some View {
        HStack(alignment: .firstTextBaseline) {
            VStack(alignment: .leading, spacing: 1) {
                Text(title)
                if !hint.isEmpty {
                    Text(hint).font(.caption).foregroundStyle(.secondary)
                }
            }
            Spacer()
            content
        }
        .padding(.vertical, 2)
    }
}

// MARK: - 模型

struct ConfigurableProviderRow: Identifiable, Equatable {
    let provider: String
    let displayName: String
    let settingsNs: String
    let settingsPath: [String]
    let error: String
    let credentialRef: String
    let catalogOnly: Bool
    var configured = false
    var writable = false
    var id: String { provider }
}

struct ModelsSection: View {
    @Environment(AppState.self) private var app
    @State private var routes: [(id: String, name: String)] = []
    @State private var configurables: [ConfigurableProviderRow] = []
    @State private var drafts: [String: String] = [:]
    @State private var statusMessage = ""

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                if !statusMessage.isEmpty {
                    Text(statusMessage).font(.caption).foregroundStyle(.secondary)
                }
                keyCard
                defaultModelCard
                catalogCard
            }
            .padding(22)
            .frame(maxWidth: 820, alignment: .leading)
        }
        .task { await load() }
    }

    private var keyCard: some View {
        GroupBox(L("API 密钥")) {
            VStack(alignment: .leading, spacing: 10) {
                if configurables.isEmpty {
                    Text(L("内核未给出可配置提供方。")).foregroundStyle(.secondary).font(.callout)
                }
                ForEach(configurables) { p in
                    if p.credentialRef.isEmpty {
                        LabeledRow(title: p.displayName, hint: LF("{0} 未声明 apiKeyEnv(无需密钥)", p.settingsNs)) {
                            Text(p.error.isEmpty ? "—" : p.error).font(.caption).foregroundStyle(.secondary)
                        }
                    } else {
                        LabeledRow(
                            title: p.configured ? "\(p.displayName) · \(p.credentialRef)" : LF("{0} · {1}(密钥缺失)", p.displayName, p.credentialRef),
                            hint: p.configured ? L("已保存,输入新值可替换") : L("输入密钥后即可使用其模型")
                        ) {
                            HStack {
                                SecureField(
                                    p.configured ? L("已保存,输入新值可替换") : L("输入 API 密钥"),
                                    text: Binding(
                                        get: { drafts[p.credentialRef] ?? "" },
                                        set: { drafts[p.credentialRef] = $0 }
                                    )
                                )
                                .textFieldStyle(.roundedBorder)
                                .frame(width: 210)
                                Button(L("保存")) { Task { await setCredential(p) } }
                                    .disabled((drafts[p.credentialRef] ?? "").isEmpty)
                                Button(L("清除")) { Task { await unsetCredential(p) } }
                                    .disabled(!p.configured || !p.writable)
                                Button(L("发现模型")) { Task { await discoverModels(p) } }
                                if !p.error.isEmpty {
                                    Text("(\(p.error))").font(.caption).foregroundStyle(.orange)
                                }
                            }
                        }
                    }
                    Divider()
                }
            }
            .padding(6)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var defaultModelCard: some View {
        GroupBox(L("默认模型(agent-default-model)")) {
            VStack(alignment: .leading, spacing: 10) {
                LabeledRow(title: L("服务商"), hint: L("内核已注册的提供方路由(llm/listProviders)")) {
                    Picker("", selection: nsBinding("provider")) {
                        ForEach(routes, id: \.id) { r in
                            Text(r.name.isEmpty || r.name == r.id ? r.id : "\(r.name)(\(r.id))").tag(r.id)
                        }
                    }
                    .frame(width: 240)
                }
                LabeledRow(title: L("模型"), hint: L("可从提供方端点发现(见上方 API 密钥卡的「发现模型」)")) {
                    TextField("", text: nsBinding("model"))
                        .textFieldStyle(.roundedBorder).frame(width: 240)
                }
                LabeledRow(title: L("推理等级"), hint: "off / low / high / max") {
                    TextField("", text: nsBinding("reasoningEffort"))
                        .textFieldStyle(.roundedBorder).frame(width: 240)
                }
            }
            .padding(6)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var catalogCard: some View {
        GroupBox(L("会话内可选模型(session/modelCatalog)")) {
            VStack(alignment: .leading, spacing: 6) {
                Text(LF("默认:{0}/{1}", app.catalog.defaultProvider, app.catalog.defaultModel))
                    .font(.caption).foregroundStyle(.secondary)
                ForEach(app.catalog.models) { m in
                    HStack {
                        Text(m.display).fontWeight(.medium)
                        Text(m.key).font(.caption).foregroundStyle(.secondary)
                        Spacer()
                        if !m.efforts.isEmpty {
                            Text(m.efforts.joined(separator: "/")).font(.caption2).foregroundStyle(.tertiary)
                        }
                    }
                }
            }
            .padding(6)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    // MARK: 数据

    private func load() async {
        guard let rpc = app.rpcClient else { return }
        do {
            try await app.refreshSettingsSnapshot()
            let routesValue = try await rpc.callOk("llm/listProviders", .object([:]))
            routes = (routesValue.arrayValue ?? []).compactMap { r in
                guard let id = r["id"].stringValue else { return nil }
                return (id, r["name"].stringValue ?? "")
            }
            let confValue = try await rpc.callOk("llm/listConfigurableProviders", .object([:]))
            var rows: [ConfigurableProviderRow] = []
            for p in confValue.arrayValue ?? [] {
                let ns = p["settingsNs"].stringValue ?? ""
                let path = (p["settingsPath"].arrayValue ?? []).compactMap { $0.stringValue }
                let credRef = credentialRef(ns: ns, settingsPath: path)
                let declared = p["declared"].boolValue ?? true
                rows.append(ConfigurableProviderRow(
                    provider: p["provider"].stringValue ?? "",
                    displayName: p["displayName"].stringValue ?? "",
                    settingsNs: ns,
                    settingsPath: path,
                    error: p["error"].stringValue ?? "",
                    credentialRef: credRef,
                    catalogOnly: !declared
                ))
            }
            // 凭据状态
            let refs = Array(Set(rows.map { $0.credentialRef }.filter { !$0.isEmpty }))
            if !refs.isEmpty {
                if let states = try? await rpc.callOk("credentials/describe", .obj(("refs", .array(refs.map { .string($0) })))) {
                    for i in rows.indices {
                        let st = states[rows[i].credentialRef]
                        rows[i].configured = st["configured"].boolValue ?? false
                        rows[i].writable = st["writable"].boolValue ?? false
                    }
                }
            }
            configurables = rows.filter { !$0.catalogOnly } + rows.filter { $0.catalogOnly }
        } catch let err as DshRpcError {
            statusMessage = LF("加载失败:{0}", err.message)
        } catch {}
    }

    /// 凭据引用 = 沿 settingsPath 取该提供方 value 节点的 apiKeyEnv(缺失回落 schema meta.default)。
    private func credentialRef(ns: String, settingsPath: [String]) -> String {
        guard let snap = app.settingsNamespaces.first(where: { $0.ns == ns }) else { return "" }
        var node = snap.value
        for seg in settingsPath { node = node[seg] }
        if let env = node["apiKeyEnv"].stringValue { return env }
        // 回落 schema 默认
        var schemaNode = ns == "" ? .null : snap.schema["dict"][settingsPath.first ?? "apiKeyEnv"]
        for seg in settingsPath.dropFirst() { schemaNode = schemaNode["dict"][seg] }
        let def = schemaNode["dict"]["apiKeyEnv"]["meta"]["default"].stringValue
        return def ?? ""
    }

    private func setCredential(_ p: ConfigurableProviderRow) async {
        guard let rpc = app.rpcClient, let value = drafts[p.credentialRef], !value.isEmpty else { return }
        do {
            try await rpc.callOk("credentials/set", .obj(("ref", .string(p.credentialRef)), ("value", .string(value))))
            drafts[p.credentialRef] = ""
            statusMessage = LF("{0} 密钥已保存。", p.displayName)
            await load()
        } catch let err as DshRpcError {
            statusMessage = LF("保存失败:{0}", err.message)
        } catch {}
    }

    private func unsetCredential(_ p: ConfigurableProviderRow) async {
        guard let rpc = app.rpcClient else { return }
        try? await rpc.callOk("credentials/unset", .obj(("ref", .string(p.credentialRef))))
        await load()
    }

    private func discoverModels(_ p: ConfigurableProviderRow) async {
        guard let rpc = app.rpcClient else { return }
        do {
            let value = try await rpc.callOk("llm/discoverModels", .obj(
                ("settingsNs", .string(p.settingsNs)),
                ("request", .obj(("provider", .string(p.provider))))
            ))
            let names = (value.arrayValue ?? []).compactMap { $0["id"].stringValue }
            statusMessage = LF("发现 {0} 个模型:{1}", String(names.count), names.prefix(12).joined(separator: "、"))
        } catch let err as DshRpcError {
            statusMessage = LF("发现模型失败:{0}", err.message)
        } catch {}
    }

    /// agent-default-model 命名空间的字段绑定(settings/mutate 去抖)。
    private func nsBinding(_ field: String) -> Binding<String> {
        Binding(
            get: { app.settingsValue(ns: "agent-default-model", path: [field]).stringValue ?? "" },
            set: { app.mutateSetting(ns: "agent-default-model", path: [field], value: .string($0)) }
        )
    }
}

// MARK: - 插件

struct PluginsSection: View {
    @Environment(AppState.self) private var app
    @State private var entries: [(entryId: String, moduleName: String, enabled: Bool, phase: String)] = []

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                GroupBox(L("插件清单(pluginInventory/list,只读)")) {
                    VStack(alignment: .leading, spacing: 6) {
                        if entries.isEmpty { Text(L("暂无条目")).foregroundStyle(.secondary) }
                        ForEach(entries, id: \.entryId) { e in
                            HStack {
                                phaseDot(e.phase)
                                Text(e.moduleName).fontWeight(.medium)
                                Text(e.entryId).font(.caption).foregroundStyle(.secondary)
                                Spacer()
                                Text(e.phase.isEmpty ? "—" : e.phase).font(.caption).foregroundStyle(.secondary)
                                Text(e.enabled ? L("启用") : L("停用"))
                                    .font(.caption)
                                    .foregroundStyle(e.enabled ? Color.green : Color.secondary)
                            }
                        }
                    }
                    .padding(6)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                // 其余命名空间(内核插件注册的配置树):通用 schema 渲染
                ForEach(genericNamespaces) { ns in
                    GroupBox(ns.ns) {
                        SchemaFields(ns: ns).padding(6)
                    }
                }
            }
            .padding(22)
            .frame(maxWidth: 820, alignment: .leading)
        }
        .task { await load() }
    }

    private var genericNamespaces: [SettingsNamespace] {
        // 通用页策划的四个命名空间不在此重复渲染(见 GeneralSection)
        let curated: Set<String> = ["ui-theme", "ui-chat", "ui-conversation", "permission"]
        return app.settingsNamespaces.filter { ns in
            !SettingsView.sectionOrder.contains(ns.ns) && ns.ns != "agent-default-model" && !curated.contains(ns.ns)
        }
    }

    private func phaseDot(_ phase: String) -> some View {
        let color: Color = switch phase {
        case "active": .green
        case "loading", "pending": .orange
        case "failed": .red
        default: .gray
        }
        return Circle().fill(color).frame(width: 7, height: 7)
    }

    private func load() async {
        guard let rpc = app.rpcClient else { return }
        if let value = try? await rpc.callOk("pluginInventory/list", .object([:])) {
            entries = (value["entries"].arrayValue ?? []).compactMap { e in
                guard let entryId = e["entryId"].stringValue else { return nil }
                return (entryId, e["moduleName"].stringValue ?? "", e["enabled"].boolValue ?? false, e["fiberPhase"].stringValue ?? "")
            }
        }
    }
}

// MARK: - 代理预设

struct AgentPresetsSection: View {
    @Environment(AppState.self) private var app
    @State private var presets: [(id: String, name: String, description: String, isDefault: Bool, broken: Bool)] = []
    @State private var statusMessage = ""

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if !statusMessage.isEmpty {
                    Text(statusMessage).font(.caption).foregroundStyle(.secondary)
                }
                ForEach(presets, id: \.id) { p in
                    GroupBox {
                        VStack(alignment: .leading, spacing: 6) {
                            HStack {
                                Text(p.name.isEmpty ? p.id : p.name).font(.headline)
                                if p.isDefault {
                                    Text(L("默认")).font(.caption2).padding(.horizontal, 6).padding(.vertical, 2)
                                        .background(Color.accentColor.opacity(0.18), in: Capsule())
                                }
                                if p.broken {
                                    Text(L("损坏")).font(.caption2).foregroundStyle(.red)
                                }
                                Spacer()
                                if !p.isDefault {
                                    Button(L("设为默认")) { Task { await setDefault(p.id) } }
                                    Button(L("复制副本")) { Task { await copyPreset(p) } }
                                }
                                Button(L("打开目录")) { Task { await openDirectory(p.id) } }
                                Button(L("删除"), role: .destructive) { Task { await deletePreset(p.id) } }
                            }
                            if !p.description.isEmpty {
                                Text(p.description).font(.caption).foregroundStyle(.secondary)
                            }
                            Text(p.id).font(.caption2).foregroundStyle(.tertiary)
                        }
                        .padding(4)
                    }
                }
                if presets.isEmpty {
                    Text(L("暂无预设")).foregroundStyle(.secondary)
                }
            }
            .padding(22)
            .frame(maxWidth: 820, alignment: .leading)
        }
        .task { await load() }
    }

    private func load() async {
        guard let rpc = app.rpcClient else { return }
        if let value = try? await rpc.callOk("agentPresets/list", .object([:])) {
            presets = (value["presets"].arrayValue ?? []).compactMap { p in
                guard let id = p["id"].stringValue else { return nil }
                return (id, p["name"].stringValue ?? "", p["description"].stringValue ?? "",
                        p["isDefault"].boolValue ?? false, p["broken"].boolValue ?? false)
            }
        }
    }

    private func setDefault(_ id: String) async {
        await app.updateSettings(ns: "agent-presets", patch: ["default": .string(id)])
        await load()
    }

    private func copyPreset(_ p: (id: String, name: String, description: String, isDefault: Bool, broken: Bool)) async {
        guard let rpc = app.rpcClient else { return }
        let newId = p.id + "-copy-\(Int(Date().timeIntervalSince1970) % 100000)"
        try? await rpc.callOk("agentPresets/copy", .obj(("from", .string(p.id)), ("id", .string(newId)), ("name", .string(p.name.isEmpty ? newId : p.name + " 副本"))))
        await load()
    }

    private func deletePreset(_ id: String) async {
        guard let rpc = app.rpcClient else { return }
        try? await rpc.callOk("agentPresets/deletePreset", .obj(("id", .string(id))))
        await load()
    }

    private func openDirectory(_ id: String) async {
        guard let rpc = app.rpcClient else { return }
        _ = try? await rpc.callOk("settings/openAgentPresetDirectory", .obj(("agentPreset", .string(id))))
    }
}
