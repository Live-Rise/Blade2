import Foundation

/// dsh 内核宿主(macOS 版):与 Windows 版 DshKernelHost.cs 完全相同的进程契约——
///   node bin.js web --no-open --port 0
/// 并从 stdout/stderr 捕获 "dsh web: <url>" 行(90 秒超时)。
/// 内核位置解析(优先级从高到低):
///   1. DSH_MAC_KERNEL 环境变量(指向含 lib/bin.js 的内核树)
///   2. 打包内置 .app/Contents/Resources/Kernel/dsh/lib/bin.js(+ Resources/Kernel/node)
///   3. ~/Library/Application Support/Blade2/Kernel/dsh(scripts/prepare-kernel.sh 部署)
///   4. 系统 npm 全局安装的 dsh(which dsh → lib/bin.js)
/// 数据家 DSH_HOME 指向 ~/Library/Application Support/Blade2,与系统 ~/.dsh 隔离。
@MainActor
final class DshKernelHost {
    struct KernelLaunchError: LocalizedError {
        let message: String
        var errorDescription: String? { message }
    }

    private var process: Process?
    /// 子进程管道与行泵必须强持有:释放会让管道读端关闭 → 内核写 stdout 时被 SIGPIPE 杀死。
    private var stdinPipe: Pipe?
    private var outPipe: Pipe?
    private var errPipe: Pipe?
    private var linePump: LinePump?
    private(set) var isBundled = false

    // MARK: - 解析

    /// node 可执行:内置 node → Homebrew → 系统路径 → PATH。
    static func resolveNode(bundle: Bundle = .main) -> String? {
        if let bundled = bundle.path(forResource: "node", ofType: nil, inDirectory: "Kernel"),
           FileManager.default.isExecutableFile(atPath: bundled) {
            return bundled
        }
        for candidate in ["/opt/homebrew/bin/node", "/usr/local/bin/node"] {
            if FileManager.default.isExecutableFile(atPath: candidate) { return candidate }
        }
        return which("node")
    }

    /// 内核树(lib/bin.js 所在目录)+ 是否为打包内置形态。
    static func resolveKernelTree(bundle: Bundle = .main) -> (path: String, bundled: Bool)? {
        if let env = ProcessInfo.processInfo.environment["DSH_MAC_KERNEL"],
           FileManager.default.fileExists(atPath: env + "/lib/bin.js") {
            return (env, false)
        }
        if let binJs = bundle.path(forResource: "bin", ofType: "js", inDirectory: "Kernel/dsh/lib") {
            // <Resources>/Kernel/dsh/lib/bin.js → 树根 = lib 上两级
            let tree = ((binJs as NSString).deletingLastPathComponent as NSString).deletingLastPathComponent
            if FileManager.default.fileExists(atPath: tree + "/lib/bin.js") { return (tree, true) }
        }
        let support = appSupportRoot().appendingPathComponent("Kernel/dsh")
        if FileManager.default.fileExists(atPath: support.path + "/lib/bin.js") {
            return (support.path, false)
        }
        // npm 全局安装形态:which dsh → 符号链接 → .../@deepseek-ai/dsh/lib/bin.js
        if let dshBin = which("dsh") {
            let resolved = (try? FileManager.default.destinationOfSymbolicLink(atPath: dshBin))
                .map { link in link.hasPrefix("/") ? link : ((dshBin as NSString).deletingLastPathComponent as NSString).appendingPathComponent(link) }
                ?? dshBin
            let tree = ((resolved as NSString).deletingLastPathComponent as NSString).deletingLastPathComponent
            if FileManager.default.fileExists(atPath: tree + "/lib/bin.js") { return (tree, false) }
        }
        return nil
    }

    static func appSupportRoot() -> URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Blade2", isDirectory: true)
    }

    static func which(_ name: String) -> String? {
        let path = ProcessInfo.processInfo.environment["PATH"] ?? "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin"
        for dir in path.split(separator: ":") {
            let candidate = dir + "/" + name
            if FileManager.default.isExecutableFile(atPath: String(candidate)) { return String(candidate) }
        }
        return nil
    }

    // MARK: - 启动

    /// 启动内核,等待 "dsh web:" URL 行。返回完整 token URL。
    func start(dshHome: URL) async throws -> URL {
        guard let tree = Self.resolveKernelTree() else {
            throw KernelLaunchError(message: L(
                "未找到 blade2 内核。请运行 scripts/prepare-kernel.sh,或 npm i -g @deepseek-ai/dsh,或设置 DSH_MAC_KERNEL。"
            ))
        }
        guard let node = Self.resolveNode() else {
            throw KernelLaunchError(message: L("未找到 node 运行时(需要 node 18+)。请安装 Homebrew node。"))
        }
        isBundled = tree.bundled

        let binJs = tree.path + "/lib/bin.js"
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: node)
        // --expose-internals:web profile 默认 patchReload:"live",HMR 服务需要 node 暴露
        // internals(仅 node 运行时 flag,启动器侧参数,不触碰内核代码)。
        proc.arguments = ["--expose-internals", binJs, "web", "--no-open", "--port", "0"]
        var env = ProcessInfo.processInfo.environment
        env["DSH_HOME"] = dshHome.path
        proc.environment = env

        let outPipe = Pipe()
        let errPipe = Pipe()
        proc.standardOutput = outPipe
        proc.standardError = errPipe
        // stdin 必须保持打开:内核在 stdin 关闭时会自行退出(web 服务形态);
        // 给一个永不关闭的管道,等价于 Win 版 CreateNoWindow 后台进程的 stdin 语义。
        let stdinPipe = Pipe()
        proc.standardInput = stdinPipe
        self.stdinPipe = stdinPipe

        let gate = URLGate()
        // 内核输出持久化(诊断启动失败;对应 Win 版内核 stdout/stderr 诊断面)
        let logURL = FileManager.default.urls(for: .libraryDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Logs/blade2-kernel.log")
        try? FileManager.default.createDirectory(at: logURL.deletingLastPathComponent(), withIntermediateDirectories: true)
        if !FileManager.default.fileExists(atPath: logURL.path) {
            FileManager.default.createFile(atPath: logURL.path, contents: nil)
        }
        let logHandle = FileHandle(forWritingAtPath: logURL.path)
        logHandle?.seekToEndOfFile()
        let pump = LinePump { line in
            logHandle?.write(Data((line + "\n").utf8))
            guard let range = line.range(of: #"dsh web: (https?://\S+)"#, options: .regularExpression) else { return }
            let raw = line[range].replacingOccurrences(of: "dsh web: ", with: "")
            if let url = URL(string: raw) { gate.fulfill(url) }
        }
        outPipe.fileHandleForReading.readabilityHandler = pump.handler
        errPipe.fileHandleForReading.readabilityHandler = pump.handler
        self.outPipe = outPipe
        self.errPipe = errPipe
        self.linePump = pump

        // 内核退出即失败(带退出码)
        proc.terminationHandler = { [gate] procExit in
            logHandle?.write(Data(("\n[kernel exited] status=\(procExit.terminationStatus) reason=\(procExit.terminationReason == .uncaughtSignal ? "signal" : "exit")\n").utf8))
            gate.fail(KernelLaunchError(message: LF("内核进程已退出(状态 {0})。日志:{1}", String(procExit.terminationStatus), logURL.path)))
        }

        try proc.run()
        process = proc

        // 90s 硬超时(与 Win 版一致):到点杀整树并失败
        let timeoutTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 90_000_000_000)
            guard !Task.isCancelled else { return }
            self?.stop()
            gate.fail(KernelLaunchError(message: L("内核启动超时(90s),已终止。")))
        }

        let url: URL
        do {
            url = try await gate.wait()
        } catch {
            timeoutTask.cancel()
            throw error
        }
        timeoutTask.cancel()
        return url
    }

    func stop() {
        guard let proc = process else { return }
        process = nil
        if proc.isRunning {
            Self.killTree(pid: proc.processIdentifier)
        }
    }

    /// 进程树终止:node 可能派生 worker,先杀子进程再杀自身(对应 C# Kill(entireProcessTree))。
    nonisolated static func killTree(pid: Int32) {
        kill(pid, SIGTERM)
        DispatchQueue.global().asyncAfter(deadline: .now() + 3) {
            if kill(pid, 0) == 0 {
                _ = try? Subprocess0.run(["/usr/bin/pkill", "-KILL", "-P", String(pid)])
                kill(pid, SIGKILL)
            }
        }
    }
}

enum Subprocess0 {
    static func run(_ argv: [String]) throws -> Process {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: argv[0])
        if argv.count > 1 { p.arguments = Array(argv.dropFirst()) }
        p.standardOutput = FileHandle.nullDevice
        p.standardError = FileHandle.nullDevice
        try p.run()
        p.waitUntilExit()
        return p
    }
}

/// 按行切分的 stdout 泵:readabilityHandler 逐块读取,缓存半行,完整行回调。
final class LinePump: @unchecked Sendable {
    private let lock = NSLock()
    private var pending = Data()
    private let onLine: @Sendable (String) -> Void

    init(onLine: @escaping @Sendable (String) -> Void) {
        self.onLine = onLine
    }

    var handler: (FileHandle) -> Void {
        { [weak self] fh in
            guard let self else { return }
            let data = fh.availableData
            if data.isEmpty {
                fh.readabilityHandler = nil
                self.flush()
                return
            }
            self.lock.lock()
            self.pending.append(data)
            var lines: [String] = []
            while let idx = self.pending.firstIndex(of: 0x0A) {
                let lineData = self.pending.subdata(in: self.pending.startIndex..<idx)
                self.pending.removeSubrange(self.pending.startIndex...idx)
                lines.append(String(data: lineData, encoding: .utf8) ?? "")
            }
            self.lock.unlock()
            for line in lines where !line.isEmpty {
                self.onLine(line)
            }
        }
    }

    private func flush() {
        lock.lock()
        let rest = pending
        pending.removeAll(keepingCapacity: true)
        lock.unlock()
        if !rest.isEmpty, let line = String(data: rest, encoding: .utf8), !line.isEmpty {
            onLine(line)
        }
    }
}

/// 单次 URL 捕获的异步门(对应 C# TaskCompletionSource)。
final class URLGate: @unchecked Sendable {
    private let lock = NSLock()
    private var result: Result<URL, Error>?
    private var continuations: [CheckedContinuation<URL, Error>] = []

    func fulfill(_ url: URL) { settle(.success(url)) }
    func fail(_ error: Error) { settle(.failure(error)) }

    private func settle(_ r: Result<URL, Error>) {
        lock.lock()
        guard result == nil else { lock.unlock(); return }
        result = r
        let conts = continuations
        continuations.removeAll()
        lock.unlock()
        conts.forEach { $0.resume(with: r) }
    }

    func wait() async throws -> URL {
        lock.lock()
        if let r = result {
            lock.unlock()
            return try r.get()
        }
        return try await withCheckedThrowingContinuation { cont in
            continuations.append(cont)
            lock.unlock()
        }
    }
}
