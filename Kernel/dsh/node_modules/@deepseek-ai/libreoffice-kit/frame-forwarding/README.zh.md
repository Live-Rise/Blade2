# 帧转发 SDK

[English](README.md) | 中文

`@deepseek-ai/libreoffice-kit/frame-forwarding` 将经过授权的原生生产者连接到 Electron Main，像素不经过 DSH Host。这个公开子路径提供 Node/TypeScript 生产者与接收者，以及不依赖 Electron 的 C/C++ SDK。包内明确列出可供编译的源码；macOS 使用 IOSurface 与 Mach 端口传递，Windows 使用 D3D11 共享 NT 句柄和仅拥有者可访问的命名管道。独立 browser 与 Electron 子路径提供呈现和纹理所有权管理；消费方负责鉴权和应用控制协议。

使用 `--omit=optional` 安装 Kit 可仅使用 SDK，不安装 Office 引擎或 Windows Office 管道依赖 Koffi。普通安装保留这些可选依赖的选择规则。SDK 安装和通信均不编译或下载二进制。

## 接收端

每个窗口创建一个 `createBudget(maxBytes)`，传给窗口内所有 `createReceiver({ maxFrames, maxBytes, budget, displayAdapter, readbackTimeoutMs, framePolicy }, onEvent)`。每个接收端是一个通道，其 `tx` 是不透明、可序列化的凭据。将 `tx` 交给生产者进程，例如由启动它的父进程传入；任何持有副本的进程都可连接，复制凭据不会吊销任何持有者。凭据不得进入 Renderer、日志或诊断信息。

帧事件通过 `transport: "shared-texture"` 携带八字节原生句柄、生产者的 `frameId` 和接收端持有的 `deliveryId`。仅在 Electron 发出 `allReferencesReleased` 后调用 `release(deliveryId)`；提交队列和帧超时不解除消费者的所有权。生产者复用已释放的帧 ID 时，交付 ID 仍然不同。重复释放返回 false。`close()` 异步等待已提交 GPU 工作完成并回收所属线程；已交付资源保留至显式释放或接收端销毁。GPU 停滞可能使关闭持续等待；取消操作不会释放该 GPU 仍在使用的资源。

Windows shared-texture 接收端要求 `displayAdapter` 提供合成器实际使用的适配器 LUID；缺失时创建失败。生产者在认证时报告设备 LUID，接收端逐帧验证纹理的实际适配器。同卡时，带 keyed mutex 的生产者纹理直接导入；旧纹理复制到接收端持有的共享目标。`sourceKeyedMutex` 报告实际源纹理描述符，与握手声明无关。跨卡时，原生工作线程读回整张源纹理，将其预乘 BGRA 像素上传到合成器所用适配器上的共享纹理。JavaScript 仅接收句柄。`adapterId` 标识源卡；`presentationAdapterId` 标识导出纹理所属适配器，macOS IOSurface 的该值为 null。

接收端在 Windows 上通过 Chromium 可识别的 keyed mutex 协调目标纹理的 GPU 访问；Electron 的引用释放回调控制生命周期，不能代替这项同步。跨卡复制遵循行跨度，GPU 读回复制完成后立即归还源信用；独立目标仍保持占用，直到 Electron 释放其交付。同卡复制和 macOS IOSurface 的源信用仍随交付释放。GPU 完成通知在设备支持时复用 fence 事件，否则使用有界 query 轮询。`readbackTimeoutMs` 限制原生帧准备与交付的时间；超时或取消会停止新工作，同时保留已提交复制的资源，直到 GPU 完成或设备移除。Worker 终止和接收端垃圾回收使用相同的异步清理。

[Chromium 152 使用 `INFINITE` 获取 key 0](https://github.com/chromium/chromium/blob/24072c1aa400ec4a89dc738b6b6acd12a8589b6f/gpu/command_buffer/service/dxgi_shared_handle_manager.cc#L129)。直接交付前，接收端工作线程有界获取并释放锁，检查源纹理是否就绪。这将可能的 CPU 等待移至该线程，不复制像素，也无法限制 Chromium 自身的等待。生产者必须在提交前释放锁，且在信用归还前不得访问表面。在检查后违反规则的生产者仍可能阻塞 Chromium 或触发 GPU 看门狗。

shared-texture 的 `framePolicy` 默认为 `ordered`，保留 Office 瓦片等独立请求与响应帧。连续流可以选择 `latest`：完成的中转读回替换尚未开始上传的帧，最多保留一帧等待和一帧上传。两个暂存槽允许源复制与等待目标的准备帧并存。目标被持有时停止后续上传；有序交付在两个暂存槽均占用时也停止读回。在 `ordered` 下，producer 标记为可替换的帧会替换尚未开始上传的可替换帧，其他帧保持排队，因此一个接收端可同时服务请求/应答帧和流。

字节预算为每个 macOS 分配或声明 keyed mutex 的 Windows 同卡分配预留一份表面。旧同卡分配预留源和目标各一份。可选 hello 能力声明允许较小的预留；声明 keyed mutex 却提供无锁纹理的生产者会被拒绝。未声明但带锁的源纹理也可直接导入，预算仍按旧路径保守预留。中转最多预留 `N = maxFrames` 张源纹理、`N` 张独立目标及两张暂存纹理。目标和暂存容量按当前连接申请过的最大帧计算；源及池扩容通过一次原子预算授予。中转将每帧限制为 `maxBytes / (2N + 2)` 字节，向生产者声明其 `N` 倍的额度。三帧、256 MiB 配置下，三张 4K BGRA 源与五张接收端纹理合计预留 253.125 MiB。闲置存储保留预算；关闭释放独立池槽，仍被持有的各目标仅保留自己的预算直到释放。渲染进程队列和画布存储有独立限制；该预算不是进程内存上限。

通道同一时间只服务一个生产者连接。每个 `connected` 事件携带递增且永不复用的 `connectionId`，所有连接事件都标明该值。`send(connectionId, json)` 将控制消息加入该连接的有界队列，不让 Main 阻塞于原生 I/O；该连接不再处于连接状态时抛出异常。生产者消息以 `message` 事件到达。生产者在线时，其他候选连接以 `channel-busy` 被拒绝；在线生产者即使停止响应也不会被抢占。它关闭、失败或进程退出后，通道发出 `disconnected` 并进入排空：该连接保留其帧、分配、预算及 GPU 中继状态，直至所有已交付帧释放、GPU 工作完成，再发出 `drained`。排空期间仅一个通过认证的后继连接等待 ready，其余候选连接以 `channel-busy` 被拒绝；后继连接自身的截止时间和取消仍然生效，放弃等待的后继会让出位置。帧 ID 与分配 ID 归属各自连接，因此旧连接迟到的释放不影响复用相同 ID 的后继，信用只返还给发送该帧的连接。`drain-blocked` 表示 GPU 工作完成情况未知：其资源和预算继续保留，排空无超时地等待。`rejected` 表示候选连接被拒绝，`failed` 表示通道停止接受生产者。`close()` 撤回凭据、拒绝等待中的后继并结束当前连接。交付前丢弃的帧释放其原生资源；已交付帧在关闭后仍需显式释放。操作系统报告的对端进程仅按物理连接用于检查数据包来源和句柄复制，不授予任何访问权。

协议版本 1 的生产者与接收端在握手中声明可转交通道。接收端以 `transferable-tx-required` 拒绝未声明该能力的生产者，生产者拒绝 ready 中不含连接 ID 的接收端，因此按 PID 绑定生产者的旧构建无法与这些源码互通。其他协议版本的生产者以 `protocol-mismatch` 被拒绝，拒绝原因写明双方版本。

生产者还声明它指名的每个句柄都属于自己。在 Windows 上，帧携带的是生产者进程中的句柄值，接收端用生产者连接时打开的访问权限从该进程复制它；值若不对应仍在运行的生产者的任何句柄，连接即结束，且不触碰接收端的任何句柄；已退出的生产者带走了自己的句柄，其连接以丢失结束。接收端以 `producer-local-handles-required` 拒绝未作此声明的生产者，ready 回显接收端接受的声明，因此生产者拒绝会把其句柄值当作自身句柄读取的接收端。0.2.2 及更早的构建因此在两个方向上都无法与这些源码互通。为生产者加沙箱的接收端可以沿用该方案，因为只有接收端需要访问另一进程。

Windows CPU 中继按分配保留已导入纹理，供不同 staging 槽使用。仅在 NT 句柄指向同一对象且尺寸一致时复用；每帧复制前仍获取 key 0。分配退役时，先清除缓存导入再返还额度，未完成的复制仍持有其活动源纹理直至完成。不提供 `CompareObjectHandles` 的系统仍逐帧打开并验证纹理。

## Node/TypeScript 生产者

在所属 Node 进程中，从 `@deepseek-ai/libreoffice-kit/frame-forwarding` 导入 `createProducer`。传入通道的 `tx`、`maxPendingOperations` 和 `operationTimeoutMs`。`ready` 在通道的 ready 到达后完成，前一个生产者排空期间会等待；另一生产者在线时以 `channel-busy` 开头的消息拒绝，等待受 `operationTimeoutMs` 限制。在所属插件的销毁流程中注册显式 `close()`：它停止新操作、丢弃未提交的表面并断开连接，完成即表示本地发送端已关闭。后继的 `ready` 标志交接完成。[TypeScript API](index.d.ts) 定义操作和资源所有权。

专用原生工作线程串行执行连接、上传、锁等待及提交。独立轮询线程收到信用和控制事件后唤醒它；表面获取不使用定时器轮询。就绪前可以将操作入队；超出额度的普通操作被拒绝，包括控制消息。每个持有的租约均预留一个 discard 队列位置，因此队列满时仍可归还表面。等待额度的获取操作不会阻塞工作线程执行。获取超时仅拒绝该请求，不关闭生产者。传输故障或无法确认 GPU 完成的错误会关闭连接。`closed` 在排队操作全部完成，且工作线程和客户端销毁后解决。事件监听器异常被隔离；监听器自行负责诊断。Worker 终止通过异步原生清理处理。 设置 `DSH_FRAME_FORWARDING_DIAGNOSTICS=1` 可将已隔离的生产者回调异常报告到 stderr。

`acquire(width, height)` 返回显式表面租约。等待 `writeBgra(buffer, stride)`，再调用 `submit(frameId, metadata)` 或 `discard()`；表面的 `close()` 等同于 discard。忙碌或已消费的表面拒绝复用。垃圾回收不会归还帧额度。提交消耗租约，但底层纹理仍须等待接收端释放该帧后才可复用。

CPU 上传借用非共享的 Node Buffer，直到返回的 promise 完成。在此期间，调用者不得修改 Buffer、分离其底层 ArrayBuffer 或转移所有权。行跨度可以大于紧密排列的行宽。绑定层持有 Buffer，省去绑定层的中间复制；C SDK 仍需复制或上传像素。CPU 上传不属于 GPU 零拷贝。

## C/C++ 生产者

包含 `include/dsh_frame_forwarding.h` 并链接对应的 `libdsh_frame_forwarding_sdk.a` 或 `dsh_frame_forwarding_sdk.lib`，也可以构建 `sdk.json` 列出的源码。API 不依赖 Node 或 Electron。仅一个线程执行轮询；获取、写入、提交和归还表面可以在另一线程执行。调用者必须串行使用同一图形上下文。先中断轮询并等待该线程退出，所有 API 调用返回后再关闭客户端。

唯一公开的 C 头文件是 `include/dsh_frame_forwarding.h`。`src/graphite-interop-internal.hxx` 和 `src/graphite-interop.cc` 中的 Graphite WebGPU 适配器（`dsh::graphite::detail` 命名空间）是引擎内部实现，不属于 SDK API。适配器在发布帧之前等待已提交的 GPU 工作完成；完成失败时，分配的纹理保留到客户端关闭。

在 Windows 上，`dsh_frame_forwarding_connect_with_device` 接受应用的 `ID3D11Device`，并保留引用至关闭。传入空值或使用 `dsh_frame_forwarding_connect` 会创建默认设备。所有连接函数都接受通道的 TX 凭据。`dsh_frame_forwarding_connect_with_options` 还限制连接及握手时长（包括等待前一个生产者排空），并接受在连接线程执行的取消回调；通道忙时以 `channel-busy` 开头的文本失败。应用自行选择 GPU，DSH 选择传输方式。使用被接管设备的即时上下文时须与 SDK 写入串行执行。macOS 要求传入空设备。

`dsh_frame_forwarding_trim` 把所有空闲表面归还接收端的共享字节预算：可用表面、排队中的最新帧、已获批但尚未创建的分配以及尚在申请中的分配；已获取和已提交的表面保留。在视图停止显示帧时调用它，例如应用不可见视口之后，下一次获取会重新申请分配。在 Windows 上它会刷新生产者 D3D11 设备的立即上下文，因为 D3D11 只在下一次刷新时销毁已释放的纹理；请在使用该上下文的线程上调用它。Office 引擎在确认不可见视口后执行收缩。

首次获取表面时返回空值，等待接收端授予分配额度。轮询后在所属应用适合的时机重试，不得忙等。分配请求进入共享 FIFO 预算队列。额度同时覆盖闲置池和显示中的帧。其他获取空值表示额度不足；终止性分配错误由轮询返回 `kind = -1` 和说明文本。轮询返回负数时停止处理，并在关闭前保留诊断信息。消费者必须限制操作总时长，超时后断开连接。

纹理采用左上原点、sRGB、BGRA8 预乘像素。Windows 表面带有使用 key 0 的 keyed mutex。获取租约不加锁：SDK CPU 上传持有写锁；外部 GPU 写入方通过 `dsh_frame_forwarding_surface_keyed_mutex` 或导入 API 自行持锁。写入方将命令排队后释放 key 0，并在提交或归还前释放导入对象和视图。CPU 上传在返回前复制输入，不等待 GPU 完成；`dsh_frame_forwarding_write_bgra_with_timeout` 限制锁获取时间。`AcquireSync` 可能阻塞 CPU。macOS 写入方必须在提交或归还前完成 GPU 工作。复用表面需要重新获取租约，Windows 上还必须取得写锁。GPU 访问释放失败时，应关闭连接，不得复用表面。

帧 ID 必须是 1 到 9007199254740991 之间的整数，且在尚未释放的提交中唯一。`dsh_frame_forwarding_submit` 提交成功返回 1，失败返回 0。提交失败后，有效的获取仍归调用方所有，可在满足相同 GPU 完成要求后重试或归还。

连续流使用 `dsh_frame_forwarding_submit_latest` 提交，它从不等待接收端信用。设接收端帧上限为 `N`：尚未释放的提交少于 `max(1, N - 1)` 时立即发送该帧；否则它成为客户端唯一的排队帧，原先的排队帧未经发送即回到池中。轮询观察到释放时，SDK 发送排队帧，因此接收端无需再次渲染或外部输入即可得到最新的完整帧。没有其他同尺寸空闲表面时，获取会复用排队帧的表面，因此 `N` 至少为 2 时，已分配的流总能得到目标表面，应用状态可按自身节奏推进。内存不超出这 `N` 个分配。`dsh_frame_forwarding_submit_latest` 把它的帧标记为可替换，因此中转接收端呈现最新暂存的流帧。请求/应答帧仍使用 `dsh_frame_forwarding_submit`，它从不丢帧。Node producer 调用 `submitLatest(frameId, metadata)`。

Windows Vulkan 和 D3D12 消费者在表面获取期间，通过 `dsh_frame_forwarding_surface_shared_handle` 借用 NT 句柄。消费者必须使用 SDK 所在的 GPU 适配器，并在导入 API 中启用 keyed mutex 同步，遵守上述所有权规则。句柄归 SDK 所有，调用方不得关闭。macOS 上该函数返回空值。接收端复制会在实际纹理描述符声明 keyed mutex 时有界获取源锁；旧纹理仍要求写入已经完成。

应用 JSON、帧关联、输入语义、视口代次及输入确认由消费者协议定义。控制消息上限为 16383 个 UTF-8 字节。SDK 不向网页暴露任意进程启动或原生句柄。

## 构建与验证

在目标平台运行 `node scripts/build.mjs`，并提供完整且匹配的 Node 头文件。SDK 版本读取公开 Kit 包元数据。需要时显式设置 `DSH_NODE_INCLUDE`；Windows 还需要开发者工具链环境和 `DSH_NODE_LIB`。构建产出预编译 Node-API 插件、静态 SDK 及 SHA-256 回执。正常安装不调用编译器。公开 Kit 打包必须包含同一源码提交的四个平台，不能打包局部产物。校验同时检查插件和静态库架构、二进制和源码哈希、版本、协议及 `transferable-tx` 与 `producer-local-handles` 能力。Desktop 运行时仅保留目标平台插件和必要元数据、许可证。

构建后在本目录运行 `pnpm test`。测试沿用串行范围，覆盖 C 传输、Node 生产者、异步关闭及 Worker 终止。`DSH_FRAME_FORWARDING_PACKAGE_ROOT` 可选择已安装 SDK，`DSH_FRAME_FORWARDING_DIAGNOSTICS` 可将失败诊断保留到包外目录。性能及 Electron 共享纹理导入仍需实际 GPU 验证。

## 浏览器与 Electron 呈现

`@deepseek-ai/libreoffice-kit/frame-forwarding/browser` 导出 `FrameForwardingFrameDisplay`、`createFrameForwardingFramePainter` 及其类型。导入此入口不会加载 Electron、Office 或原生 addon，也不会分配图形资源。泛型视图标识保留应用定义的字符串类型。应用负责视图创建、鉴权、生产者控制和输入仲裁。

`FrameForwardingFrameDisplay.present()` 接管帧租约，在绘制或拒绝后释放。隐藏文档等待恢复可见；代次变化或销毁会取消等待。所属应用另行更新生产者的可见性。移除画布前先销毁 display。

`@deepseek-ai/libreoffice-kit/frame-forwarding/electron` 为 Electron Main 导出 `detectFrameForwardingAdapter` 和 `sendFrameForwardingFrame`。适配器探测在同一指定期限内等待 Chromium 完整 GPU 信息，并观察其导出的 Direct3D11 纹理。不支持的后端或软件合成模式会返回 `null`，不创建离屏窗口。取消操作同时覆盖 GPU 初始化和窗口构造。应用验证接收方和帧元数据后，`sendFrameForwardingFrame` 接管交付信用，直至提交失败或 Electron 释放最后一个 GPU 引用。其完成不代表 GPU 已释放。必需的失败回调在 Main 释放导入引用或归还交付信用前同步停止生产者 I/O；随后提交 Promise 拒绝。

`@deepseek-ai/libreoffice-kit/frame-forwarding/electron/preload` 为沙箱 preload 导出 `createFrameForwardingTextureReceiver`。打包此入口时将 Electron 保持为外部依赖，并在启动生产前安装文档唯一的接收器。每个视图最多注册一个监听器。取消订阅和撤销只停止未来交付；消费者显式释放前仍持有已交付租约。无消费者、提取帧失败或监听器抛错均会释放导入纹理。调用方负责 IPC 鉴权和应用生命周期。

## 分发

公开 Kit 包独立提供 Node SDK、SDK 元数据、校验器、browser 和 Electron 入口。以 SDK JSON 所在目录为根解析 `header`、`sources` 和 `staticLibraries[target]`。正常安装和运行使用包内插件，不编译或下载额外原生二进制。

浏览器绘制器、display、Electron 适配器探测及通用纹理所有权测试源自 DeepSeek Harness 提交 `21261bee8b9a820c62c9e42cade3ff644b747a29` 的 `packages/client/frame-forwarding` 与 `apps/desktop`。单元测试随实现维护；DSH 保留产品集成测试。

MIT SDK 来自 DeepSeek Harness 提交 `4438b67c13d529c9882d89893ed5cd610ace11cc`，WebGPU 适配器保留 LibreOffice Kit 提交 `2ae2de8c7e88df00e74ee538ebd5662f47c7a1df` 的 Office 修复。Node 绑定、独立 C SDK 与 Kit 引擎使用同一源码目录。协议版本 1 保持不变；`sdk.json`、`receipt.json` 与各预构建清单的 `capabilities` 字段记录 `transferable-tx` 与 `producer-local-handles`，用以区分当前构建与早期协议 1 构建。
