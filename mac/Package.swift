// swift-tools-version:5.10
// Blade² for macOS — dsh 内核的原生 SwiftUI 壳。
// 目标布局:
//   Blade2Core     协议层 + 编排层 + 视图(库,可被无头 E2E 复用)
//   DshMacUI       GUI 可执行(技术标识与 Win 版 DshWinUI 对应)
//   Blade2Headless 无头 E2E:boot → send → journal 渲染断言(对应 check-dsh-contract.ps1 的角色)
// 内核不进 Swift 包:scripts/prepare-kernel.sh 部署到 ~/Library/Application Support/Blade2/Kernel/dsh。
import PackageDescription

let package = Package(
    name: "DshMacUI",
    platforms: [.macOS(.v14)],
    targets: [
        .target(
            name: "Blade2Core",
            path: "Sources/Blade2Core",
            resources: [
                .process("Resources/i18n"),
            ]
        ),
        .executableTarget(
            name: "DshMacUI",
            dependencies: ["Blade2Core"],
            path: "Sources/DshMacUI"
        ),
        .executableTarget(
            name: "Blade2Headless",
            dependencies: ["Blade2Core"],
            path: "Sources/Blade2Headless"
        ),
    ]
)
