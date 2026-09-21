import Foundation

/// 版本号契约:唯一来源是 mac/VERSION,构建时由 scripts/build-app.sh 写入
/// Info.plist 的 CFBundleShortVersionString,这里只做运行时读取。
/// swift run 开发态没有 Info.plist,走回落值 —— 回落值必须与 mac/VERSION 手工保持一致,
/// 两处都改才算改了版本。
public enum AppVersion {
    /// 当前版本(与 Win 版 DshWinUI 对齐)。
    public static let current = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "0.7.7.1"
}
