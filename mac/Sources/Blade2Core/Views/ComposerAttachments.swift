import AppKit
import UniformTypeIdentifiers

/// 输入区附件的粘贴/拖拽转换层。
/// 内核契约出处(Win MainWindow.xaml.cs):
/// - 粘贴位图 → 图片附件,命名「粘贴图片 {0:HHmmss}.png」(本地时间)、mediaType 固定 image/png(xaml.cs:4212-4233)
/// - 图片文件按扩展名映射 MIME:png/jpg/jpeg/webp/gif,其余文件走通用推断(xaml.cs:4235-4266)
enum ComposerAttachments {
    // MARK: 粘贴

    /// 剪贴板图片提取:位图数据优先(对应 Win 的 Bitmap 检查,xaml.cs:4216),其次图片文件 URL。
    /// 命中返回(图像数据, 附件名);TIFF 统一转 PNG —— 粘贴位图的契约 mediaType 是 image/png。
    static func pasteboardImage(_ pasteboard: NSPasteboard) -> (data: Data, name: String)? {
        // 1) 原生 PNG
        if let png = pasteboard.data(forType: .png) {
            return (png, pasteImageName())
        }
        // 2) TIFF(macOS 位图剪贴板主流形态)→ PNG
        if let tiff = pasteboard.data(forType: .tiff), let png = pngData(fromTIFF: tiff) {
            return (png, pasteImageName())
        }
        // 3) 图片文件 URL(命中即取第一个可读的;MIME 由 ComposerView 按扩展名映射)
        if let urls = pasteboard.readObjects(forClasses: [NSURL.self],
                                             options: [.urlReadingFileURLsOnly: true]) as? [URL] {
            for url in urls where UTType(filenameExtension: url.pathExtension)?.conforms(to: .image) == true {
                if let data = try? Data(contentsOf: url) {
                    return (data, url.lastPathComponent)
                }
            }
        }
        // 4) 兜底:NSImage(覆盖罕见图片形态)
        if let image = NSImage(pasteboard: pasteboard), let png = pngData(from: image) {
            return (png, pasteImageName())
        }
        return nil
    }

    /// 粘贴位图的附件名(Win xaml.cs:4227「粘贴图片 {0:HHmmss}.png」;Win 用 DateTime 格式化占位,
    /// Mac 由壳侧先格式化时间再过 LF)。
    static func pasteImageName(_ now: Date = Date()) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "HHmmss"
        return LF("粘贴图片 {0}.png", formatter.string(from: now))
    }

    // MARK: 拖拽(NSItemProvider → 附件)

    /// NSItemProvider 的文件 URL 提取(loadItem 回包形态:URL / URL 数据)。
    static func fileURL(from provider: NSItemProvider) async -> URL? {
        let identifier = UTType.fileURL.identifier
        return await withCheckedContinuation { continuation in
            provider.loadItem(forTypeIdentifier: identifier, options: nil) { item, _ in
                if let url = item as? URL {
                    continuation.resume(returning: url)
                } else if let data = item as? Data, let url = try? URL(dataRepresentation: data, relativeTo: nil) {
                    continuation.resume(returning: url)
                } else {
                    continuation.resume(returning: nil)
                }
            }
        }
    }

    /// NSItemProvider 的图片数据(取第一个 conform .image 的注册类型;统一走 loadItem 通道)。
    static func imageData(from provider: NSItemProvider) async -> Data? {
        for identifier in provider.registeredTypeIdentifiers {
            guard let utType = UTType(identifier), utType.conforms(to: .image) else { continue }
            if let data = await providerItemData(provider, typeIdentifier: identifier) {
                // 嗅探层只认字节:图片对象/TIFF 表现先落成字节(attachmentFromImageData 里再统一转 PNG)
                return data
            }
        }
        return nil
    }

    private static func providerItemData(_ provider: NSItemProvider, typeIdentifier: String) async -> Data? {
        await withCheckedContinuation { continuation in
            provider.loadItem(forTypeIdentifier: typeIdentifier, options: nil) { item, _ in
                if let data = item as? Data {
                    continuation.resume(returning: data)
                } else if let image = item as? NSImage, let tiff = image.tiffRepresentation {
                    continuation.resume(returning: tiff)
                } else if let url = item as? URL {
                    continuation.resume(returning: try? Data(contentsOf: url))
                } else {
                    continuation.resume(returning: nil)
                }
            }
        }
    }

    /// 文件 URL → 附件:图片读字节并按扩展名映射 MIME(Win xaml.cs:4243-4260),其他文件原样入列(4264)。
    static func attachmentFromFileURL(_ url: URL) -> PendingAttachment? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        if UTType(filenameExtension: url.pathExtension)?.conforms(to: .image) == true {
            let mediaType = imageMime(forName: url.lastPathComponent) ?? "image/png"
            return PendingAttachment(name: url.lastPathComponent, mediaType: mediaType, data: data)
        }
        return PendingAttachment(name: url.lastPathComponent, mediaType: fileMime(for: url), data: data)
    }

    /// 拖入的裸图片数据 → 附件:按魔数嗅探 MIME;TIFF 等转 PNG(对齐粘贴位图的 image/png 契约)。
    /// 命名沿用粘贴位图的「粘贴图片 HHmmss.<ext>」风格(建议名带已知图片扩展名时优先)。
    static func attachmentFromImageData(_ data: Data, suggestedName: String?) -> PendingAttachment? {
        // PNG:89 50 4E 47
        if data.starts(with: [0x89, 0x50, 0x4E, 0x47]) {
            return PendingAttachment(name: imageName(suggestedName, ext: "png"), mediaType: "image/png", data: data)
        }
        // JPEG:FF D8 FF
        if data.starts(with: [0xFF, 0xD8, 0xFF]) {
            return PendingAttachment(name: imageName(suggestedName, ext: "jpg"), mediaType: "image/jpeg", data: data)
        }
        // GIF:"GIF8"
        if data.starts(with: Array("GIF8".utf8)) {
            return PendingAttachment(name: imageName(suggestedName, ext: "gif"), mediaType: "image/gif", data: data)
        }
        // WEBP:"RIFF"...."WEBP"
        if data.count > 12, data.starts(with: Array("RIFF".utf8)), data.subdata(in: 8..<12) == Data("WEBP".utf8) {
            return PendingAttachment(name: imageName(suggestedName, ext: "webp"), mediaType: "image/webp", data: data)
        }
        // TIFF / 其他 → PNG
        guard let png = pngData(fromTIFF: data) ?? (NSImage(data: data).flatMap(pngData(from:))) else { return nil }
        return PendingAttachment(name: imageName(suggestedName, ext: "png"), mediaType: "image/png", data: png)
    }

    // MARK: MIME 推断

    /// 通用文件 MIME(UTType 扩展名推断;与 ComposerView.addAttachments 同一逻辑,收敛到这一份)。
    static func fileMime(for url: URL) -> String {
        if let utType = UTType(filenameExtension: url.pathExtension) {
            return utType.preferredMIMEType ?? "application/octet-stream"
        }
        return "application/octet-stream"
    }

    /// 图片附件 MIME:按扩展名(Win xaml.cs:4246-4253),未知扩展名返回 nil(调用方回落 image/png)。
    static func imageMime(forName name: String) -> String? {
        switch (name as NSString).pathExtension.lowercased() {
        case "png": return "image/png"
        case "jpg", "jpeg": return "image/jpeg"
        case "webp": return "image/webp"
        case "gif": return "image/gif"
        default: return nil
        }
    }

    // MARK: 编码转换

    static func pngData(fromTIFF tiff: Data) -> Data? {
        guard let rep = NSBitmapImageRep(data: tiff) else { return nil }
        return rep.representation(using: .png, properties: [:])
    }

    static func pngData(from image: NSImage) -> Data? {
        guard let tiff = image.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff) else { return nil }
        return rep.representation(using: .png, properties: [:])
    }

    private static func imageName(_ suggested: String?, ext: String) -> String {
        if let suggested, imageMime(forName: suggested) != nil {
            return suggested
        }
        // 扩展名走第二占位符:键必须是静态串,插值进键会让 L10n 查不到条目
        return LF("粘贴图片 {0}.{1}", stamp(), ext)
    }

    private static func stamp(_ now: Date = Date()) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "HHmmss"
        return formatter.string(from: now)
    }
}

// MARK: - 带图片粘贴的 NSTextView

/// 剪贴板有位图/图片文件时截为附件回调(对应 Win OnInputPaste,xaml.cs:4212-4233),
/// 无图片回落原生文本粘贴。与 doCommandBy(⏎/⇧⏎)互不干扰 —— paste 是 responder 动作,
/// ⌘V 经 NSApplication 键等价 → responder 链 paste:,不走 insertText 命令路径。
final class PasteAwareTextView: NSTextView {
    /// (图像数据, 附件名);文件 URL 粘贴时名字是文件名,位图粘贴时是「粘贴图片 HHmmss.png」。
    var onPasteImage: ((Data, String) -> Void)?

    /// 输入区高度贴内容(NSViewRepresentable 的 .frame(min/max) 夹紧):
    /// NSTextView 默认无 intrinsic 尺寸,SwiftUI 会给到最大可用高,空态下玻璃卡被撑到 ~200pt。
    override var intrinsicContentSize: NSSize {
        var used: CGFloat = 18
        if let layout = layoutManager, let container = textContainer {
            used = layout.usedRect(for: container).height
        }
        let h = min(max(used + textContainerInset.height * 2 + 6, 38), 150)
        return NSSize(width: NSView.noIntrinsicMetric, height: h)
    }

    /// 文本变化后让 SwiftUI 重新取 intrinsic 尺寸(输入/外部清空都会走这里)。
    func invalidateHeight() {
        invalidateIntrinsicContentSize()
    }

    override func paste(_ sender: Any?) {
        if let image = ComposerAttachments.pasteboardImage(NSPasteboard.general) {
            onPasteImage?(image.data, image.name)
            return // 不执行 super.paste():图片不进文本
        }
        super.paste(sender)
    }
}
