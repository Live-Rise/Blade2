import SwiftUI

/// 最新 macOS 设计标准(Liquid Glass 一代)的材质门控:
/// macOS 26+ 用原生 glassEffect;旧系统回落 regularMaterial。
/// 本机 CLT 最高 26.2 SDK(27 SDK 需完整 Xcode 27),API 按可用性门控,升级零改动。
extension View {
    /// 悬浮卡/输入区玻璃底。
    @ViewBuilder
    func code2Glass<T: Shape>(in shape: T, interactive: Bool = false) -> some View {
        if #available(macOS 26.0, *) {
            if interactive {
                self.glassEffect(.regular.interactive(), in: shape)
            } else {
                self.glassEffect(.regular, in: shape)
            }
        } else {
            self.background(.regularMaterial, in: shape)
        }
    }

    /// 突出按钮:26+ 用玻璃按钮样式。
    @ViewBuilder
    func code2ProminentButton() -> some View {
        if #available(macOS 26.0, *) {
            self.buttonStyle(.glassProminent)
        } else {
            self.buttonStyle(.borderedProminent)
        }
    }
}
