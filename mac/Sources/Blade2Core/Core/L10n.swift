import Foundation

/// 壳层本地化(与 Win 版同构):中文源串即键。
/// 查找顺序:当前 locale 的 JSON 包(Assets/i18n/<locale>.json)→ 内置 zh→en 词典 → 原键。
/// 内核 locale 只接受 zh/en,非中英语言映射 KernelLocale="en"(见 AppState)。
enum L10n {
    static let zhSource = "zh-Hans"

    private static let lock = NSLock()
    private static var table: [String: String] = [:]
    private static var loadedLocale = ""

    // 内置兜底词典:主翻译源是 Resources/i18n/<locale>.json(en.json 为完整包),
    // 这里留空 —— 任何缺翻会的键回落到中文原串,保证永不空串。
    private static let builtinEnglish: [String: String] = [:]

    /// 壳侧界面语言存储键(Win 版对应 LocalSettings["ui-language"],MainWindow.xaml.cs:365-388)。
    /// 存储值为选项:"system"(跟随系统)/"zh-Hans"/"en";旧数据直接是解析值,两者兼容。
    static let localeKey = "blade2.locale"

    static var currentLocale: String {
        get {
            switch UserDefaults.standard.string(forKey: localeKey) ?? "system" {
            case zhSource: return zhSource
            case "en": return "en"
            default: return systemLocale
            }
        }
        set {
            UserDefaults.standard.set(newValue, forKey: localeKey)
            lock.lock(); loadedLocale = ""; lock.unlock()
        }
    }

    /// 设置页 Picker 的原始选项(未解析的存储值;与 Win 版 _uiLanguageOverride 同位)。
    static var storedOption: String {
        UserDefaults.standard.string(forKey: localeKey) ?? "system"
    }

    /// 跟随系统时的解析(系统语言 zh → 中文,其余 → 英文)。
    private static var systemLocale: String {
        Locale.current.language.languageCode?.identifier == "zh" ? zhSource : "en"
    }

    static var isZhSource: Bool { currentLocale == zhSource }

    static func loadIfNeeded() {
        lock.lock(); defer { lock.unlock() }
        let locale = currentLocale
        guard locale != loadedLocale else { return }
        var t: [String: String] = builtinEnglish
        if locale != zhSource {
            if let url = Bundle.module.url(forResource: locale, withExtension: "json", subdirectory: nil),
               let data = try? Data(contentsOf: url),
               let obj = try? JSONSerialization.jsonObject(with: data) as? [String: String] {
                t.merge(obj) { _, new in new }
            }
        }
        table = t
        loadedLocale = locale
    }

    /// 翻译单个 zh 源串键。zh 模式直接返回原串。
    static func translate(_ key: String) -> String {
        loadIfNeeded()
        if isZhSource { return key }
        return table[key] ?? key
    }

    /// 内核参数 locale(zh/en 白名单映射)。
    static var kernelLocale: String { isZhSource ? "zh" : "en" }
}

/// 翻译:zh 源串即键。
public func L(_ key: String) -> String { L10n.translate(key) }

/// 带参翻译(对应 C# LF,{0}/{1} 占位;参数为字符串时同样过一遍 L)。
public func LF(_ format: String, _ args: Any...) -> String {
    let template = L10n.translate(format)
    var result = template
    for (index, arg) in args.enumerated() {
        let text: String
        switch arg {
        case let s as String: text = L10n.translate(s)
        case let i as Int: text = String(i)
        case let d as Double: text = String(d)
        case let b as Bool: text = b ? "true" : "false"
        case let other: text = String(describing: other)
        }
        result = result.replacingOccurrences(of: "{\(index)}", with: text)
    }
    return result
}
