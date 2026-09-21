import AppKit
import UserNotifications

/// 用户注意力助手:窗口不在前台时的审批请求与内核启动失败走系统本地通知,
/// 挂起审批期间显示 Dock 徽标(对应 Win 版 AppNotification,MainWindow.xaml.cs:4666-4668
/// 审批、:1707 启动失败;HIG:需要用户注意而窗口不可见时用通知)。
enum UserAttention {
    /// 无 bundle 的进程(swift run 开发态/无头 E2E)没有通知中心与 Dock,直接跳过。
    private static var guiAvailable: Bool { Bundle.main.bundleIdentifier != nil }

    /// 启动时请求通知授权(仅在未决定时弹一次)。
    static func requestAuthorization() {
        guard guiAvailable else { return }
        let center = UNUserNotificationCenter.current()
        center.getNotificationSettings { settings in
            guard settings.authorizationStatus == .notDetermined else { return }
            center.requestAuthorization(options: [.alert, .sound]) { _, _ in }
        }
    }

    static func notify(title: String, body: String) {
        guard guiAvailable else { return }
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        content.sound = .default
        let request = UNNotificationRequest(identifier: "blade2.\(UUID().uuidString)", content: content, trigger: nil)
        UNUserNotificationCenter.current().add(request)
    }

    /// Dock 徽标:有挂起审批且应用非前台 → "1",否则清除。
    static func updateDockBadge(pending: Bool, appActive: Bool) {
        guard guiAvailable else { return }
        NSApp?.dockTile.badgeLabel = (pending && !appActive) ? "1" : nil
    }
}
