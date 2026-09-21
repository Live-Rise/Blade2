# Blade² for macOS

**dsh 内核的原生 SwiftUI 桌面壳——无 WebView、无 Electron、无 Web UI。**

Windows 版([`../README.zh.md`](../README.zh.md))的 macOS 姊妹版本:两版共享**同一内核进程契约与 RPC 协议**,界面各自用平台原生控件实现。本目录是纯 Swift/SwiftPM 工程,不依赖 Xcode 工程文件。

```
┌──────────────── Blade²(macOS 14+, SwiftUI)────────────────┐
│  原生界面:会话侧栏 · 聊天气泡 · 审批卡 · 提问卡 ·         │
│  Schema 驱动设置 · 模型目录 · 用量统计(Swift Charts)      │
└───────────────┬────────────────────────────────────────────┘
                │ 进程契约 + token URL 握手(与 Win 版逐条一致)
   node --expose-internals …/dsh/lib/bin.js web --no-open --port 0
                │
        HTTP JSON RPC  ── 一元调用(session/、workspace/、settings/…)
        mux WebSocket  ── /api/remote.mux · journal / $events 增量流
```

## 环境要求

- macOS 14+(在 macOS 26 上按 Liquid Glass 设计标准渲染,旧系统自动回落)
- Swift 6 编译器(Command Line Tools 即可,**不需要**完整 Xcode)
- node 18+(推荐 Homebrew `node`,与 Win 版内置的 v24 同代)
- dsh 内核(见下)

## 快速开始

```sh
# 1. 部署 macOS 可用的内核副本(复制仓库 Kernel/dsh,补 darwin 平台原生依赖,自检)
scripts/prepare-kernel.sh

# 2. 开发运行
swift run

# 3. 或打包成 .app(ad-hoc 签名)
scripts/build-app.sh                # → build/Blade2.app
scripts/build-app.sh --with-kernel  # 内核树 + node 二进制打进 Bundle(自包含形态)
```

版本单一来源是根目录的 `mac/VERSION`(当前 0.7.7.1,与 Win 版对齐);`build-app.sh` 读它写入 Info.plist,`AppVersion.current` 运行时读取(开发态回落同值)。

### 内核解析顺序(对应 Win 版 BundledKernel / 系统 dsh.cmd 双形态)

1. `DSH_MAC_KERNEL` 环境变量(指向含 `lib/bin.js` 的内核树)
2. 打包内置 `.app/Contents/Resources/Kernel/dsh`(+ `Resources/Kernel/node`)
3. `~/Library/Application Support/Blade2/Kernel/dsh`(`prepare-kernel.sh` 的默认部署位)
4. 系统 npm 全局安装的 `dsh`(which dsh → lib/bin.js)

数据家 `DSH_HOME=~/Library/Application Support/Blade2`,与系统 `~/.dsh` 隔离(对应 Win 版 `%LOCALAPPDATA%\Blade2`)。凭据不自动迁移:把 `~/.dsh/.credentials.yaml` 复制进数据家(空数据家发送后 turn 停在 `turn/start`,属正常现象)。

> **关于 `--expose-internals`**:web profile 默认 `patchReload: "live"`,内核的 HMR 服务要求 node 暴露 internals。这是 node 运行时侧 flag,不改动内核任何代码。
> **关于 darwin 原生依赖**:`sharp`、`koffi` 的 win32 预编译随 Win 包走;`prepare-kernel.sh` 按 registry 元数据把 `@img/sharp-darwin-arm64`、`@img/sharp-libvips-darwin-arm64`、`@koromix/koffi-darwin-arm64` 补进副本(等价内核报错信息给出的标准安装流程,绕开全树 npm 解析——树内含 registry 拉不到的私有 devDep)。

## 与 Win 版的逻辑一致性

协议与编排逻辑逐条对齐(`Sources/Blade2Core`):

| 契约 | 实现位置 |
|---|---|
| 进程契约:`node bin.js web --no-open --port 0` + `dsh web: <url>` 行捕获(90s 超时) | `Dsh/DshKernelHost.swift` |
| HTTP JSON RPC 信封 `{type:"client-request", rpcId:"c2-N", method, payload:{args}}` | `Dsh/DshRpcClient.swift` |
| mux WS 文本帧 open/cancel/item/end/error;streamId 必须字符串 | 同上 |
| `$events`:ready(取 clientId)/emit/cancel/waterfall;回答 POST `/api/$events/result` | 同上 |
| 重连:单循环指数退避 0.5s→30s;恢复后补订 $events + 业务流重开 | 同上 |
| 启动顺序:auth → mux → modelCatalog → 事件注册 → settings/describe → workspace/follow → $events → session/list → session/control(0.7.1 定论顺序) | `Core/AppState.swift` |
| 历史:二分头游标(0..65536)+ 回向翻页;follow 帧回向补页;游标三层去重;400 条上限 | 同上 |
| 轮询兜底:22×4s,through=64 对半收缩("past cursor") | 同上 |
| 发送:无会话先建、selectModel(失败忽略)、图片内联 base64、文件 receiptId 上传、queue/steer 模式;⌘⏎ busy 反向档 | 同上 |
| 参数键实测结论:`session/list {_request}`、`session/cancel|fork {request}`、`commands/execute` 平铺、`goals/*` 平铺、`session/canOpenWorkspacePath` 无参 | 同上 |
| 审批:`approval/request` waterfall → 单活跃悬浮卡 → `allowed-once`/`rejected`;失败保留重试;窗口未聚焦发系统通知 + Dock 徽标 | 同上 + `Core/UserAttention.swift` |
| 提问:`user-questions/request` → radio 单选/checkbox 多选/自定义 → `{answers:[{id,selected,custom?}]}`;跳过 `{kind:"next"}` | 同上 |
| 设置:独立命名空间(ui-theme/ui-chat/ui-conversation/permission…,内核 0.7.7 无 general);`settings/mutate` 400ms 去抖分组;`settings/replace` 带 expectedRevision | `Views/SettingsView.swift` |
| 消息反馈:messageFeedback 双信封 + ifVersion + version-conflict 重试;同评级再点=撤销;note 编辑 | `Core/AppState.swift` |
| 交付物:present.open 宿主路由,files 下标即坐标 | 同上 |
| 会话投影:session/control 的 plan/permissions/jobs 帧(baseline + 增量);计划 chip、会话态权限、作业面板 | `Core/AppState+RunStats.swift` |
| 斜杠命令:commands/list + 补全浮层(↑↓/Tab/⏎/Esc);/export 经 `session.export` 流式下载 ZIP;/feedback 会话反馈表单 | `Views/SlashSuggestView.swift` 等 |
| 运行统计:轮/步/tok/s、缓存命中(cacheRead/(cacheRead+input))、TTFT/TPS、模型/工具用时——口径逐条对齐 MainWindow.RunStats.cs | `Core/AppState+RunStats.swift` |
| 紧凑 transcript:`ui-chat.transcriptView`(内核默认 compact),已完成轮过程气泡折叠 | 同上 |
| 粘贴/拖拽附件:剪贴板位图→「粘贴图片 HHmmss.png」;输入区拖拽图片/文件 | `Views/ComposerAttachments.swift` |
| 用量统计:session/list + page 回向翻页 ≤40 页;5 KPI + 26 周热力图(每日/每周/累计)+ 近 7/30 天 + 多模型趋势——口径对齐 Win | `Views/UsageView.swift` + `Core/UsageModels.swift` |
| i18n:中文源串即键,`Resources/i18n/<locale>.json`(当前 zh/en,en 键与 Win 词典对齐);设置页切换 + 回写内核 `locale.preference` | `Core/L10n.swift` |

### v1 未覆盖(Win 版有、Mac 版后续补)

- 文件右栏(workspaceFiles/list|read|changes 流)
- @-引用面板(fileReferences/list、sessionReferenceResolver/candidates)
- 队列面板(session/updateQueue 的 edit/remove/steer 三操作;排队/插话的**发送**已实现)
- 「在应用中打开」宿主子菜单(Win 的 open-in-app 应用选择器)
- 28 语完整语言包(架构已就位:向 `Resources/i18n/` 加 `<locale>.json` 即生效;当前内置 zh/en)
- Win 侧独有、按平台惯例**有意不移植**:托盘常驻(Mac 用 Dock + 通知)、Mica 背景皮肤(Mac 用系统材质)

## macOS 设计合规(HIG)

参考 [Apple HIG](https://developer.apple.com/design/human-interface-guidelines/) 落实的要点:

- **菜单栏**:App 菜单顺序 About(第一项)→ Settings…(⌘,,`Settings` 场景自动挂接)→ Hide → Quit;About 是独立标准窗口;新会话 ⇧⌘N(避开系统 New Window 的 ⌘N)
- **设置**:独立设置窗口(非主窗口内嵌页),通用页为壳侧策划页聚合 ui-theme/ui-chat/ui-conversation/permission
- **通知与徽标**:`UNUserNotificationCenter` 本地通知(审批请求/内核启动失败,窗口不在前台时)+ Dock 徽标,回前台自动清除
- **材质**:Liquid Glass 只用于控件/导航层(输入条、悬浮卡、补全浮层),内容层用标准材质;macOS 26 以下回落 `.regularMaterial`(`Views/Glass.swift` 统一门控)
- **外观**:内核 ui-theme.preference(light/dark/system)驱动 `preferredColorScheme`,system 实时跟随系统深浅色;用户气泡按 WCAG 亮度自适应黑/白字
- **打包**:App 图标(AppIcon.icns)、`LSApplicationCategoryType`、版本单一来源

## 工程结构

```
mac/
├── VERSION                       # 版本单一来源(build-app.sh 读;AppVersion 回落值需同步)
├── Package.swift                 # Blade2Core(库)+ DshMacUI(GUI)+ Blade2Headless(E2E)
├── Resources/AppIcon.icns        # 应用图标(Win 品牌图转制)
├── Sources/Blade2Core/           # 协议层 + 编排层 + 全部视图(可被无头 E2E 复用)
│   ├── Dsh/                      #   DshKernelHost / DshRpcClient / JSON
│   ├── Core/                     #   AppState(编排)/ Models / L10n / UserAttention
│   │                             #   AppState+Workspace / AppState+RunStats / UsageModels / AppVersion
│   ├── Views/                    #   壳 / 侧栏 / 聊天 / Markdown / 输入区 / 审批 / 设置 / 用量 / 统计面板
│   └── Resources/i18n/           #   en.json 翻译包(zh 为源串)
├── Sources/DshMacUI/             # @main 入口:场景与菜单(App/技术标识对应 Win 版 DshWinUI)
├── Sources/Blade2Headless/       # 无头端到端:boot → send → 渲染断言
├── scripts/prepare-kernel.sh     # 内核部署 + darwin 依赖补齐 + 自检
└── scripts/build-app.sh          # Blade2.app 组装(--with-kernel 自包含形态)
```

## 诊断

- 壳日志:`~/Library/Logs/blade2-shell.log`(启动链路逐阶段 + RPC 收发)
- 内核日志:`~/Library/Logs/blade2-kernel.log`(内核 stdout/stderr 全量 + 退出码)
- 无头契约测试:`swift run Blade2Headless`(对真实内核跑 boot/send,会在数据家创建真实会话并消耗配额——渲染管线部分用合成 journal 页断言)

## 许可

内核树随上游 [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness) 发行(MIT);壳源码许可同 Windows 版约定。
