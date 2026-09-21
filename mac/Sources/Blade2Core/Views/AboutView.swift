import SwiftUI

/// 标准关于窗口(替代旧 AboutSheet,由 App 菜单第一项"关于 Blade²"以独立
/// Window 场景打开)。信息层次对应 macOS 系统 About 面板:应用名、版本、
/// 定位描述、版权行。
public struct AboutView: View {
    public init() {}

    public var body: some View {
        VStack(spacing: 10) {
            Text("Blade²").font(.system(size: 32, weight: .bold))
            Text(LF("版本 {0}", versionString))
                .font(.callout).foregroundStyle(.secondary)
            Text(L("dsh 内核的 macOS 原生壳(SwiftUI)"))
                .foregroundStyle(.secondary)
            Text(L("版权所有 © 2026 Blade²"))
                .font(.caption).foregroundStyle(.tertiary)
        }
        .padding(28)
        .frame(width: 320)
    }

    /// 版本号:单一来源是打包层 AppVersion.current(读 Bundle.main 的
    /// CFBundleShortVersionString;开发态 swift run 无 Info.plist 时回落,
    /// 与 mac/VERSION 手工对齐)。
    private var versionString: String { AppVersion.current }
}
