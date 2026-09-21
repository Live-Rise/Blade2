import SwiftUI
import Blade2Core
import Observation

@main
struct Blade2App: App {
    @State private var appState = AppState()
    /// commands 无独立环境树,openWindow 需声明在 App 上;且只能在按钮 action
    /// 时机调用(求值期调用会静默失效),这是 SwiftUI 多窗口模式的既定用法。
    @Environment(\.openWindow) private var openWindow

    var body: some Scene {
        WindowGroup {
            ShellView()
                .environment(appState)
                .frame(minWidth: 1060, minHeight: 660)
                .task { await appState.boot() }
                .onReceive(NotificationCenter.default.publisher(for: NSApplication.willTerminateNotification)) { _ in
                    appState.shutdown()
                }
        }
        .windowToolbarStyle(.unified)
        // 设置走独立 Settings 场景:声明即自动启用 App 菜单 Settings… 项(⌘,),
        // 无需手动菜单项;App 菜单顺序为 About → Settings… → Hide → Quit
        // (HIG "The menu bar")。SettingsView 仅 Blade2Core 模块内可见,经公开的
        // SettingsRootView 包装;Settings 场景独立于 WindowGroup,AppState 需单独注入。
        Settings {
            SettingsRootView()
                .environment(appState)
                // 界面语言切换后强制重建(与 ShellView 根同款);读 uiLocaleVersion
                // 建立 @Observable 依赖,计数变化即重建整个设置视图树。
                .id(appState.uiLocaleVersion)
                .frame(minWidth: 760, minHeight: 480)
        }
        // 标准关于窗口(App 菜单第一项,见 HIG "The menu bar");内容定宽、不可拉伸。
        Window(Text(verbatim: L("关于 Blade²")), id: "about") {
            AboutView()
        }
        .windowResizability(.contentSize)
        .commands {
            // WindowGroup 在 macOS 自动提供 File > New Window(⌘N),
            // "新会话"改用 ⇧⌘N,避免同快捷键出现两个菜单项。
            CommandGroup(after: .newItem) {
                Button(L("新会话")) {
                    NotificationCenter.default.post(name: .blade2NewSession, object: nil)
                }
                .keyboardShortcut("n", modifiers: [.command, .shift])
            }
            // About 归位 App 菜单(取代 SwiftUI 默认项),不再放进 Help 菜单。
            CommandGroup(replacing: .appInfo) {
                Button(L("关于 Blade²")) {
                    openWindow(id: "about")
                }
            }
        }
    }
}
