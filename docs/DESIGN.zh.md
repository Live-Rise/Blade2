# Blade² 设计与工程文档（Blade2）

> 本文是深度设计与工程决策文档；项目简介见 [README.zh.md](../README.zh.md) / [README.md](../README.md)。
>
> [对照参考：DeepSeek Harness 官方桌面端 README](https://github.com/deepseek-ai/deepseek-harness/blob/master/apps/desktop/README.zh.md)

Blade² 是包裹 dsh 内核的原生 WinUI 3 壳。它与官方桌面端（Electron 壳）走的是同一条路线的两端：**同样以"壳 + 自包含内核"的形态发行，但壳不消费 dsh Web UI，而是直连内核的公开 RPC 协议，用原生控件自绘全部界面**。壳启动内核 web 服务进程（`--no-open --port 0`），从 stdout 捕获带 token 的 URL 完成 cookie 握手，之后 HTTP JSON RPC 承载一元调用、mux WebSocket 承载 journal / workspace / $events 增量流，工具审批经 `$events/result` 回传。

对照官方桌面端的定位句——"包裹 dsh Web UI 的 Electron 壳，分帧字节管道 + Node IPC + `dsh-app://`"——Blade² 的对应面是：**包裹 dsh 内核（而非其 Web UI）的原生壳，进程契约 + token URL 契约 + 公开 RPC/mux 协议**。官方壳复用 Web 客户端资源，因此其发布必须与 Web 客户端、后端作为组合验证；Blade² 不依赖任何客户端资源与 DOM 结构，升级兼容面收敛为 RPC 方法契约（类型化公开面，远比 CSS 哈希类稳定）。

## 关键技术决策

| 决策 | 原因 | 直接结果 |
|---|---|---|
| 发布身份 | 壳与内置内核作为一个组合发行；独立升级内核会产生未经验证的组合（官方桌面端同一理由）。 | 内核树 `Kernel/`（node.exe + 裁剪 dsh 树）随壳打进同一 MSIX。升级 dsh = 换 `Kernel/dsh` 树重打包，壳代码通常零改动。 |
| 运行时 | 系统运行时与包管理器状态不可控（官方桌面端同一理由）。 | 内置 `Kernel/node.exe`（v24）运行内置 dsh，全链路零外部依赖；系统 Node/npm 不进入执行路径。系统 `dsh.cmd` 仅作开发机回退。 |
| 包来源 | 离线启动 + 核心依赖安装开销（官方桌面端同一理由）。 | `Kernel/dsh/` 携带裁剪后的完整生产依赖树（npm 全局安装树 robocopy，裁 .map/.d.ts/.ts/README）。首启不安装任何包。 |
| 状态归属 | 壳与系统 CLI 共享 `~/.dsh` 会互相改变 dsh、插件与原生模块状态（官方桌面端同一理由）。 | 内核数据家固定 `DSH_HOME=%LOCALAPPDATA%\Blade2`，与 `~/.dsh` 完全隔离。凭据需从 `~/.dsh/.credentials.yaml` 复制（`MISSING_CREDENTIAL` 是空数据家的正常表现）。 |
| 通信 | 官方壳认为监听 Web 服务有暴露风险，用字节管道 + `dsh-app://`；Blade² 是单机原生进程，权衡不同。 | 内核仍以 `web` 形态启动（`--port 0` 随机绑定 127.0.0.1、`--no-open`），壳经 token 握手后走公开 HTTP/mux 协议。暴露面 = 本机回环 + 进程级随机 launchToken。 |
| UI 形态 | dsh Web UI 是 CSS-modules 哈希类 DOM，跨版本脆弱（0.2.x WebView 桥接时代实测）。 | 0.3.0 起放弃 WebView 桥接，全部 UI 原生自绘。壳对内核的接触面收敛为进程/CLI/URL/RPC 四类契约。 |
| 崩溃防线 | mux 回调线程直接动 UI 集合、async void 未捕获异常均上抛成 stowed exception（0xc000027b 实测根因）。 | 一切 UI 变更经 `DispatcherQueue.TryEnqueue` 编组（入口自查 `HasThreadAccess`）；每个 async void 全 try-catch 兜底；`App.UnhandledException` 写 `%TEMP%\blade2_unhandled.txt` 定位。 |

### 运行时与内核启动

内置内核解析顺序（`Dsh/DshKernelHost.cs`）：优先 `<install>\Kernel\node.exe` + `Kernel\dsh\lib\bin.js`（打包形态），回退 `%APPDATA%\npm\dsh.cmd`（开发机形态），两者参数完全一致。壳与内核的进程契约只有两个：`node bin.js web --no-open --port 0` 与 stdout 的 `dsh web: <url>` 行；URL 含进程级随机 launchToken，是唯一的无凭据文件握手通道。启动等待 90s 超时。

内核就绪后（`MainWindow.BootAsync`）：

1. token 握手：`GET /?token=...` 换 `dsh-auth` cookie，此后 HTTP 与 WebSocket 同源鉴权；
2. 开 mux WebSocket（`/api/remote.mux`，text frame only），并随连订阅 `$events` 全量事件流；
3. 注册 `approval/request`（审批浮卡）、`api-session/added/removed`（列表刷新）事件处理器；
4. 拉会话列表与工作区树 baseline，进入聊天主链路。

会话 cwd 显式指向用户文档目录：MSIX 打包应用的进程 CWD 是 system32（不可写，工具全废），不显式指定会让新会话落进不可写工作区——这是 0.3.1 的修复。

## 功能面（全部经内核 RPC 实现）

- **会话**：新建（未选会话时发送消息自动新建并打开）、分页历史回放（`session/page`，`throughSeq` 越界 "past cursor" 时对半收缩试探）、`session/follow` 增量流跟随、分叉（`session/fork`）、归档（`workspace/archiveSession`）、移动到工作区（`workspace/insertSessionBefore`）、顶条标题搜索。
- **聊天渲染**：journal 事件 → 气泡三态（user 胶囊右对齐 / assistant 卡片全宽 / tool 小字行）；`assistant/attempt` 中 `finish{reason.kind:"error"}` 渲染为 ⚠ 错误气泡（用户才能看到 `MISSING_CREDENTIAL` 这类失败）；跳过内核注入的 runtime context 快照。三层游标去重：全局 `_journalCursor` 跳过已渲染 seq、follow 流按会话独立 localCursor（全局单游标曾致跨会话吞帧假死）、相邻同角色同文本跳过。
- **follow 不可靠兜底**：部分内核版本 follow 流帧不推 journal 增量（根因未深挖），生产实现 = 发送后每 4s 轮询 `session/page` 直到 turn 结束（最多 90s）。
- **审批**：`$events` 推 `approval/request`{toolName, eventId, reason} → 原生浮动卡「允许一次 / 拒绝」→ `$events/result` 回传 outcome。
- **设置**（Ctrl+S 或 Footer 入口）：`settings/describe` 返回各 namespace 的 schema+value（dsh 设置是 schema 驱动的）；已落地分区：通用（壳语言 / 字号 / 托盘与退出）、模型（`agent-default-model` 的 provider / model / reasoningEffort + API Key 凭据）、插件（schema 泛化渲染 + 清单过滤）、智能体预设（agent-presets）、统计、关于（壳版本 / 内核版本 / GitHub Release 检查更新）、维护；保存走 `settings/mutate` 的 JSON-Patch 式 ops。
- **个性化（personalization）**：壳内建分区，自定义指令 + 背景皮肤。指令持久化走 `$DSH_HOME/AGENTS.md`——内核 agent-instructions 插件（standard/cordis/ptc 预设均注册，`maxBytes: 65536`）把它当 user-global 指令注入每个会话的上下文，所以「对所有对话始终生效」不需要内核改动，壳也不碰各工作区自己的 AGENTS.md。显式保存（不防抖自动落盘）：误触不该直接写进之后每一轮的上下文；清空 = 删文件，避免往上下文塞空指令。草稿归窗口（`_instructionsDraft`），切分区/换语言只换视图不丢内容。内核按轮做版本比对，写入后下一条消息即生效。皮肤从「通用」迁入：导入图片作内容区背景，ImageBrush Opacity 0.40 叠在 SurfaceAlt 上，存 `shell-skin.img`。
- **统计（usage）**：壳侧自建的 Token 用量页（趋势 + 模型占比环图），数据来自会话 journal 聚合，XAML 静态区经 `DetachFromParent` 搬进 SettingsHost 渲染。
- **能力菜单**：Goal / Schedules（只读投影，不创建编辑删除）/ Skills。
- **壳本地化（i18n）**：28 个语言包（`Assets/i18n/*.json`，1 空格缩进）+ 硬编码中文键→英文的 `ShellEnglish` 字典（603 键；英语不走语言包，直接取字典值）。`L()/LF()` 包裹文案，`RefreshShellLanguage` 在切换语言时扫描已注册视觉树区域回翻（含动态设置分区、组合 UIA 名的 LF 模板键、弹层在调用点直接翻）；缺键时 L() 原样返回中文。
- **模型选择**（输入区左侧按钮）：`session/modelCatalog` 全目录 → MenuFlyout → `session/selectModel` 应用到当前会话（推理档取该模型 catalog 推荐档）。
- **壳层体验**：DesktopAcrylic 材质（拉伸后丢失 → 防抖 400ms 重赋重建；NavigationView 两处主题层覆盖 Transparent 才透出材质）、延伸标题栏 + 显式 caption 按钮配色、启动 LoadingLayer、圆角内容面。
- **运行状态条（0.7.7.0）**：composer 上方实时显示「{轮} 轮 {步} 步 · {tok/s}」与「{累计} tok · 缓存命中 {百分比}%」。折叠算法逐条镜像内核 `dsh-session-stats` 的 sessionStats 投影：`step/end` 计轮/步（轮变化才计轮）、`assistant/message` 的 `usage.outputTokens` 与首 token 时差累计 decodeMs/decodeTokens（tok/s = decodeTokens ÷ decodeMs），缓存命中 = cacheRead ÷ (cacheRead + input)。数据入口挂在 `RenderEventCore`（page 回放与 follow 实时流共用），历史与实时天然全覆盖；会话切换先清零再由完整回放重建，避免重复累计。
- **可查看面板与过程行（0.7.7.1，对标官方端）**：两个胶囊 Tapped 弹 Flyout——「会话统计」（模型用时 llmMs / 工具调用用时 toolMs（tool/call→tool/result 配对）/ TTFT 均值（ttftMs÷ttftSteps）/ TPS）与「Token 用量」（总量 / 缓存命中 / 未缓存输入 / 缓存读取 / 输出，数字 N0 全量显示）。transcript 新增过程行（复用 ToolTpl 紧凑灰行）：`system/message` → 「系统提示词」（同会话后续 = 「系统提示词更新」）；`user/message` 且 `source.kind != user` 不再静默跳过 → 「上下文注入 · {source.plugin / path / label}」。
- **并行编辑防复发**：`_scratch/**` 已从编译排除（Compile/Page/ApplicationDefinition Remove）——草稿半成品 .cs 不再打断构建。
- **系统集成（0.7.7.0）**：一条 comctl32 窗口子类化承载两件事——① `WM_SETTINGCHANGE(ImmersiveColorSet)`：system 主题偏好下实时跟随 Windows 深浅色（此前 UISettings 只在应用时读一次，系统切换要重启才生效）；② `Shell_NotifyIcon` 托盘图标（左键唤起窗口，右键原生 Win32 菜单：打开 / 退出——托盘场景没有 XamlRoot，WinUI 弹层不可用）。系统级 toast 走 Windows App SDK `AppNotification`，覆盖内核启动失败与窗口未聚焦时的工具审批请求、用户提问（含 exit_plan_mode 计划评审）、任务完成（turn/end，仅跟随流实时路径触发，历史回放不补通知；10 分钟新鲜度窗兜底快照补拉重放）；统一准入 `ShouldNotify()` = shell.json `showNotifications` 开关 + 窗口不在前台。无包身份的开发形态 Register 抛异常 → 退化托盘气球（NIF_INFO，图标不在且用户关着托盘图标则静默跳过）；点击回前台：toast 走 `NotificationInvoked`、气球走 `NIN_BALLOONUSERCLICK`，都汇入 `ActivateFromTray`。

## 目录结构

```
DshWinUI/                    # 仓库目录名（技术标识已统一为 Blade2，目录名未改）
├── Blade2.csproj           # .NET 8 + WindowsAppSDK 1.6，x64（版本在 csproj 与 manifest 两处硬编码）
├── Package.appxmanifest     # MSIX 标识 Blade2 / DisplayName Blade²
├── App.xaml(.cs)             # XamlControlsResources + stowed exception 文件出口
├── MainWindow.xaml(.cs)      # 全部 UI 与 RPC 编排（含 ShellEnglish 字典与 RefreshShellLanguage 扫描）
├── MainWindow.Capabilities.cs # Goal / Schedules / Skills 能力面板
├── BubbleTemplateSelector.cs # 气泡三态模板选择
├── Pages/                    # FilesPanel 文件面板等
├── Theme/                    # Tokens/Typography/Styles 设计令牌（语义层）
├── Dsh/
│   ├── DshKernelHost.cs      # 内核进程生命周期 + URL 捕获
│   └── DshRpcClient.cs       # HTTP JSON RPC + mux WebSocket + 事件订阅 + 审批回传
├── Kernel/                   # 内核自包含资源，整目录打进 MSIX
│   ├── node.exe              # v24（~89MB）
│   └── dsh/                  # @deepseek-ai/dsh 0.1.5-rc.2 裁剪树（~162MB，来源树已裁 .map/.d.ts/.ts/README）
├── check-dsh-contract.ps1     # 内核升级契约检查（见下）
└── Assets/                   # 图标/磁贴（黑底白刃刀锋标，manifest BackgroundColor=transparent）、i18n/ 28 语言包
```

## 开发

前置：Windows 10 19041+ x64、.NET 8 SDK、Windows App SDK 1.6。

```sh
dotnet run              # 开发形态：回退系统 dsh.cmd（需 npm install -g @deepseek-ai/dsh）
```

开发形态与打包形态走同一 `DshKernelHost` 解析器，参数完全一致；差异只在内核来源。WebView 桥接时代（0.2.x）的 DOM 契约检查仍保留在 `check-dsh-contract.ps1`，其完整模式可作为内核行为的运行时探针，但当前架构下升级适配只看四类契约：

1. **进程契约**：`dsh web` 的启动形态不变；
2. **CLI 契约**：`--no-open` / `--port 0` 仍被接受；
3. **URL 契约**：stdout 仍打印 `dsh web: http://127.0.0.1:<port>/?token=...`；
4. **RPC 契约**：`DshRpcClient` 消费的方法表（session/、workspace/、settings/、$events 等，各方法 args 键名见内核各包 `lib/typert.remote-client.js` 的 TYPERT_REMOTE 表）。

```powershell
pwsh -File check-dsh-contract.ps1 -CliOnly    # 升级 dsh 后最快检查（CLI + URL 契约）
```

## 打包

单命令构建 + 签名 + 安装（PowerShell 7）：

```powershell
# 1. bump 版本：Blade2.csproj <Version> 与 Package.appxmanifest Identity 两处硬编码，必须同改
# 2. Release 构建：必须用 VS 的 MSBuild（dotnet build 只产 .appxrecipe，从不产 msix），
#    且必须显式开 GenerateAppxPackageOnBuild（默认 false，缺它静默无产物）。
#    AppxPackageDir **必须以分隔符结尾**（正斜杠最稳；缺了会把目录名粘连成
#    AppxPkgs074DshWinUI_0.7.4.0_x64_Test —— 本节旧版写的"不能带反斜杠"正好写反了）
msbuild Blade2.csproj /p:Configuration=Release /p:Platform=x64 `
  /p:AppxPackageDir=C:/AppxPkgs/ /p:GenerateAppxPackageOnBuild=true /p:AppxBundle=Never
# 3. 签名
signtool sign /fd SHA256 /sha1 <thumbprint> <msix>
# 4. 安装前先杀壳与内置 node（0x80073D02 资源占用）
Add-AppxPackage <msix>
```

> 在 Git Bash 里跑 msbuild/signtool 前先 `export MSYS_NO_PATHCONV=1`，否则 `/p:`、`/pa` 会被改写成假路径（MSB1008 / File not found: C:/Program Files/Git/pa）。根目录 `build-0800.ps1` 封装了 Release 构建 + 签名，`install-0800.ps1` 封装了杀进程 + 安装（0.8.1）。

产物为自包含 MSIX（WindowsAppSDKSelfContained + SelfContained，百 MB 级）：.NET 运行时、Windows App SDK、`Kernel/` 内核资源整目录随包发行，最终用户零环境要求。MSIX 安装目录只读，因此内核数据家走 `%LOCALAPPDATA%\Blade2` 而非安装目录。

与官方桌面端更新机制（签名更新单元 + electron-updater 差量块复用）对照：Blade² 目前无自动更新，升级 = 覆盖安装新 MSIX（先杀进程，否则资源占用报 0x80073D02）。

## 验证

- **UI 自动化一律走 UIA**（AutomationId 已挂：InputBox / SendButton / ChatList / NewSessionItem 等）；坐标/键鼠注入在 Parsec 远程会话内全部被会话边界吞掉，不可行。NavigationView 固定项设 `SelectsOnInvoked="False"`，UIA Select 只对动态会话项有效，固定项真人路径走 Tapped。
- **journal 直读**：`%LOCALAPPDATA%\Blade2\sessions\<workspace>\<session>\session.v3.jsonl.zstd`（fzstd 解压，events 按 seq 单调）。
- **崩溃排查**：LocalDumps 自动收 0xc000027b 全转储 → `dotnet-dump analyze <dmp>` 交互式 `pe -nested` / `clrstack -all`；XAML stowed exception 看 `%TEMP%\blade2_unhandled.txt`。
- 聊天主链路回归基线：新发消息 → 气泡上屏 → 内核 journal 落盘，全链路无异常。

## 已知限制

- 无自动更新；升级靠覆盖安装新 MSIX。
- Schedules 能力只读（不创建、编辑或删除计划，不调用 schedules RPC）。
- 凭据不自动迁移：空数据家首用需从 `~/.dsh/.credentials.yaml` 复制凭据文件。
- `session/follow` 流在部分内核版本不推增量，轮询兜底是生产手段而非修复。
- i18n 边界：翻译只对 `RefreshShellLanguage` 已注册的扫描区生效；文案用 L() 包裹但键未进 `ShellEnglish` 时静默回退中文；WinUI 框架默认控件名（ScrollBar、InfoBar 图标）不随壳语言。
- 技术标识（Identity / exe / 命名空间）为 Blade2，Publisher 保持 `CN=DshWinUI` 以复用既有签名证书——`²` 是包标识非法字符，只有 DisplayName 呈现为 Blade²。
- Windows 之外无发布产物；macOS 由 `mac/` 下独立的 SwiftUI 壳覆盖（与 Windows 版共享内核契约，进行中），Linux 无目标。

## 版本史

0.1.9 默认标题栏 + 黑鲸图标 → 0.2.0–0.2.16 WebView 混合壳时代（材质三坑、rc.1/rc.2 契约适配、图标三连修、定名 Code²，已退役）→ 0.3.0 原生 GUI 转向 → 0.3.1 MSIX CWD 修复 → 0.4.0 设置面板 + 模型选择器 → 0.5.x–0.6.x 会话/设置/插件迭代（0.6.3.x 多个装机版）→ 0.7.4 UI 修复批次（悬停圆角底色、文本控件聚焦零色变，取证见 `DELIVERY-NOTES-0.7.4.md`）→ 0.7.6.7 设置分区扩展（通用/模型/插件/预设/统计/维护）+ 统计页 + Goal/Schedules/Skills 能力菜单 + 28 语壳本地化（UIA 逐屏验收）。
→ 0.7.6.8 品牌更名 Blade²——黑底白刃刀锋标全套图标 + 窗口/标题/文案全量替换，技术标识仍为 DshWinUI（0.8.1 起改为 Blade2，见 0.8.1 条），内核数据家 %LOCALAPPDATA%\Code2 → Blade2（首访自动搬迁）。
→ 0.7.6.9 图标资产改为圆角矩形徽章形态（圆角外透明，与品牌源图一致，由徽章原图直接裁取缩放生成）。
→ 0.7.7.0 system 主题实时跟随 Windows 深浅色（WM_SETTINGCHANGE 钩子）+ 托盘图标（打开/退出）+ 系统 toast 通知（内核失败/未聚焦审批）+ 运行状态条（轮/步/tok/s/累计 token/缓存命中，对齐官方端）。
→ 0.7.7.1 状态条两胶囊可点开「会话统计」/「Token 用量」面板（fold 补 llmMs/toolMs/ttft/output 拆分）+ transcript 过程行（系统提示词/系统提示词更新/上下文注入·来源），对标官方端；_scratch/** 移出编译。
→ 0.7.8–0.7.9.x 桌面宠物（壳内建「宠物」分区 + 独立置顶无边框透明分层窗，宠物能力全部来自内核插件 @linxin666/dsh-pet，Codex Pet 包装进 $DSH_HOME/pets/，图集帧动画跟随模型状态，点击互动）+ 壁纸插件驱动背景皮肤（@baiiii/dsh-wallpaper-local）+ 个性化分区（AGENTS.md 自定义指令）+ 托盘设置 + 技能面板 + TurnRail + 图片灯箱。
→ **0.8.1 当前版：壳源码以 MIT 许可发布（© 2026 SAKUSORA，根目录 LICENSE）；技术标识 DshWinUI → Blade2（Identity / exe / 命名空间全量改名，Publisher 保持 CN=DshWinUI 证书不变，升级需卸载重装）；随附发布：0.7.x 周期的宠物/壁纸/个性化特性定型，安装包版本与文档口径统一，新增 .gitignore 排除构建产物与测试残留。**
