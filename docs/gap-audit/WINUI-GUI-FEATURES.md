# Blade² WinUI GUI 功能清单（主干 C#/XAML，不含 rust/）

> 用途：与 dsh 官方 Web UI 做「功能完全一致」差距校对的基准盘点。
> 范围：`MainWindow.xaml` + `MainWindow.xaml.cs`（约 905KB）+ 全部 `MainWindow.*.cs` 分部类 + `Pages/` + `Dsh/` + `BubbleTemplateSelector.cs` + `Theme/` + `Assets/i18n/`。
> **明确排除** `rust/`（重构中的 Windows 壳，不在本次盘点范围）。
> 核实原则：以代码为准；`docs/DESIGN.zh.md` 仅作线索。文档与代码不一致项见文末专节。

**图例**
| 完整度 | 含义 |
|---|---|
| 完整 | 用户可见行为已接 RPC/本地持久化，可闭环 |
| 部分 | 有 UI 与部分链路，但缺 CRUD/写回/边界处理 |
| 只读 | 仅展示投影/快照，无创建/编辑/删除 |
| 未接 RPC | 纯壳本地行为，不经内核（标「壳增强」时可接受） |

**优先级**：P0=对齐官方 WebUI 必须有；P1=重要体验/一致性；P2=壳增强/锦上添花。

---

## 1. 会话管理

| 功能名 | 用户可见行为 | 证据（文件:行） | 完整度 | 优先级 |
|---|---|---|---|---|
| 新建会话 | 侧栏「新会话」固定项；未选会话时发送消息自动新建并打开；composer 工作区选择器决定 cwd | `MainWindow.xaml:226-232`；`MainWindow.xaml.cs:4216-4244,7095-7105` | 完整 | P0 |
| 会话列表 | NavigationView 动态项；按工作区/时间分组与排序；子代理会话 `↳` 前缀缩进 | `MainWindow.xaml.cs:2836-2911,2942-3050,3128-3186` | 完整 | P0 |
| 会话标题搜索 | 顶条 AutoSuggestBox 按标题/内容检索，建议列表可点开 | `MainWindow.xaml:135-172`；`MainWindow.xaml.cs:13569-13748` | 完整 | P0 |
| 打开会话 | 点击侧栏项 → `session/list` 状态 + `session/page` 历史回放 + `session/follow` 增量 | `MainWindow.xaml.cs:4095-4157,4487-4605` | 完整 | P0 |
| 分页历史回放 | `session/page` + `throughSeq` 越界「past cursor」对半收缩试探 | `MainWindow.xaml.cs:4487-4710` | 完整 | P0 |
| 增量跟随 | `session/follow` 流；assistantStream 逐字帧；follow 不推增量时每 4s 轮询 `session/page`（≤90s） | `MainWindow.xaml.cs:5412-5462,7414-7453`；`Dsh/DshRpcClient.cs:364-377` | 完整 | P0 |
| 重命名会话 | 侧栏右键/菜单 → 对话框 → `session/rename` | `MainWindow.xaml.cs:3397-3419` | 完整 | P0 |
| 分叉会话 | 侧栏菜单「分叉会话」→ `session/fork` | `MainWindow.xaml.cs:3897-3906`；`MainWindow.MessageActions.cs:803-812` | 完整 | P0 |
| 按消息分叉 | 用户气泡「分叉」→ `session/fork {atSeq}`，子会话标题按官方 increasedForkTitle 递增后 `session/rename` | `MainWindow.MessageActions.cs:803-855,17111` | 完整 | P1 |
| 归档会话 | 侧栏菜单 → `workspace/archiveSession` | `MainWindow.xaml.cs:3862-3870` | 完整 | P0 |
| 移动到工作区 | 侧栏菜单 → `workspace/insertSessionBefore` | `MainWindow.xaml.cs:3879-3888` | 完整 | P0 |
| 取消运行 | 侧栏菜单「取消」→ `session/cancel`；发送按钮变停止 → `session/cancel` | `MainWindow.xaml.cs:3489-3498,7018-7027` | 完整 | P0 |
| 子代理会话只读 | origin=subagent 的会话显示只读提示 + 返回父会话按钮 | `MainWindow.xaml:359-386`；`MainWindow.xaml.cs:7005,16033-16057` | 完整 | P1 |
| 会话状态投影 | plan / permissions / schedule / goal 投影随 `session/control` 流到达 | `MainWindow.xaml.cs:2048-2064,15532-15731` | 完整 | P0 |
| 会话反馈 | composer「…」菜单「会话反馈」→ `sessionFeedback/record` | `MainWindow.xaml:1127-1131`；`MainWindow.xaml.cs:16556-16634` | 完整 | P1 |

## 2. 聊天与消息

| 功能名 | 用户可见行为 | 证据（文件:行） | 完整度 | 优先级 |
|---|---|---|---|---|
| 气泡三态渲染 | user 胶囊右对齐 / assistant 卡片全宽 / tool 小字行；另 reasoning / deliverable / system | `BubbleTemplateSelector.cs`；`MainWindow.xaml:424-617` | 完整 | P0 |
| Markdown 渲染 | assistant 消息经 `RenderAssistantMarkdown` | `MainWindow.xaml.cs:5500-5519` | 完整 | P0 |
| 错误气泡 | `assistant/attempt` `finish{reason.kind:"error"}` → ⚠ 错误气泡（暴露 MISSING_CREDENTIAL 等） | `MainWindow.xaml.cs:5567-5994`（RenderEventCore） | 完整 | P0 |
| 跳过 runtime context 快照 | 内核注入的 runtime context 不上屏 | `MainWindow.xaml.cs:5567+` | 完整 | P1 |
| 三层游标去重 | 全局 `_journalCursor` + follow 流独立 localCursor + 相邻同角色同文本跳过 | `MainWindow.xaml.cs:365,4605-4710,4728-4801` | 完整 | P0 |
| 逐字流（assistant-stream） | `session/follow assistantStream=true` 时叠逐字帧；失败退化为轮询 | `MainWindow.xaml.cs:4834-5286,5412` | 完整 | P1 |
| 发送消息 | Enter 发送；含附件随 `session/prompt` 提交 | `MainWindow.xaml.cs:6951-7238` | 完整 | P0 |
| 发送图片/文件附件 | 添加按钮文件选择、拖拽、粘贴图片；`session/uploadFileBinary` 上传 → `session/prompt` 带 attachmentId | `MainWindow.xaml.cs:6423-6875`；`Dsh/DshRpcClient.cs:145-191` | 完整 | P0 |
| 历史图片回显 | `session/attachment` 拉字节渲染 user-image 气泡；本地回显认领防双贴 | `MainWindow.xaml.cs:6444-6531,305-353` | 完整 | P1 |
| 图片灯箱 | 点击聊天图片放大预览，背景点击/关闭钮退出 | `MainWindow.ImageLightbox.cs`；`MainWindow.xaml:1733-1750` | 完整 | P2（壳增强） |
| 复制消息 | 用户/助手气泡复制按钮 | `MainWindow.MessageActions.cs:247-267,17089` | 完整 | P1 |
| 撤回/编辑用户消息 | 未开跑且未突变时可撤回编辑；本地撤回投影 | `MainWindow.MessageActions.cs:105-209` | 完整 | P1 |
| 本轮文件改动 chips | 对标官方 deliverablesDefinition：write/edit 突变累加 → 可点 chip 打开 | `MainWindow.MessageActions.cs:865-1052,1397-1432` | 完整 | P1 |
| 撤回本轮修改 | 把本轮文件恢复到修改前（删新建/回写旧内容/反向 str_replace） | `MainWindow.MessageActions.cs:1064-1282` | 完整 | P1 |
| 消息反馈（赞/踩/备注） | `messageFeedback/list` 回读 + `put`/`delete`（ifVersion 乐观锁） | `MainWindow.xaml.cs:17015-17663` | 完整 | P0 |
| 过程行 | `system/message` →「系统提示词/更新」；`user/message` source≠user →「上下文注入·来源」 | `MainWindow.xaml.cs:5686-5690`；DESIGN.zh.md:50 | 完整 | P1 |
| 对话显示（标准/紧凑） | 设置 `ui-chat.transcriptView` 折叠已完成轮次 | `MainWindow.xaml.cs:427-435,10568-10573` | 完整 | P1 |
| 等待思考动画 | 「少女祈祷中…」+ 工具/推理扫光 | `MainWindow.xaml:534`；`MainWindow.MessageActions.cs:335-428` | 完整 | P2 |
| 消息时钟/轮次时长 | 气泡上显示时间与轮耗时 | `MainWindow.MessageActions.cs:769-787` | 完整 | P2 |

## 3. 审批

| 功能名 | 用户可见行为 | 证据（文件:行） | 完整度 | 优先级 |
|---|---|---|---|---|
| 工具审批浮卡 | `$events` waterfall `approval/request` → 浮动卡「允许一次 / 拒绝」 | `MainWindow.xaml:724-746`；`MainWindow.xaml.cs:2598,7469-7598` | 完整 | P0 |
| 审批回传 | `ResolveEventAsync` → `$events/result` outcome=allowed-once/rejected | `Dsh/DshRpcClient.cs:712-720` | 完整 | P0 |
| 审批队列 | 多条审批排队逐条展示 | `MainWindow.xaml.cs:1980,7523-7552` | 完整 | P0 |
| 审批取消 | `EventCancelled` 事件 → 移除浮卡 | `MainWindow.xaml.cs:7552-7584` | 完整 | P1 |
| 系统通知审批 | 窗口未聚焦时 toast/气球通知审批请求，点击回前台 | `MainWindow.ShellIntegration.cs`；`MainWindow.xaml.cs:7498-7505` | 完整 | P1 |
| 用户提问（ask-user） | waterfall `user-questions/request` → 选项/多选/自定义输入/跳过 → `answers[]` 回传 | `MainWindow.xaml:922-933`；`MainWindow.xaml.cs:7620-7891` | 完整 | P0 |
| 自动审批预设 | 插件页开关：在内核 profile patch 增删 auto-approve 预设 + 判定模型选择 | `MainWindow.xaml.cs:12528-12543,14048-14301` | 完整 | P1 |

## 4. 模型与提供商

| 功能名 | 用户可见行为 | 证据（文件:行） | 完整度 | 优先级 |
|---|---|---|---|---|
| 模型选择器 | 输入区 ModelButton → `session/modelCatalog` 目录 → MenuFlyout → `session/selectModel` | `MainWindow.xaml:1236-1248`；`MainWindow.xaml.cs:13192-13367` | 完整 | P0 |
| 推理档（reasoningEffort） | 模型菜单内选择推理档；取 catalog 推荐档 | `MainWindow.xaml.cs:7057-7069,13320-13534` | 完整 | P0 |
| 模型设置页 | 提供方行卡（显示名+凭据状态点+自定义标记）+ 添加提供方 + 默认模型 | `MainWindow.xaml.cs:10612-10653` | 完整 | P0 |
| 提供方 CRUD | 添加/编辑/删除提供方；`settings/mutate` + `credentials/set`/`unset` | `MainWindow.xaml.cs:11751-11985` | 完整 | P0 |
| 模型发现 | `llm/discoverModels` 拉候选模型目录并挑选 | `MainWindow.xaml.cs:11564-11751` | 完整 | P1 |
| 可配置提供方目录 | `llm/listConfigurableProviders` | `MainWindow.xaml.cs:10653-10736` | 完整 | P0 |
| 凭据管理 | `credentials/describe` / `set` / `unset`（密钥不落设置文档） | `MainWindow.xaml.cs:10736,11891-11955,13163` | 完整 | P0 |
| 默认模型卡 | agent-default-model 的 provider/model/reasoningEffort | `MainWindow.xaml.cs:10612+` | 完整 | P0 |
| 自动审批判定模型 | 插件页内选择判定模型（走 modelCatalog） | `MainWindow.xaml.cs:14147-14301` | 完整 | P1 |

## 5. 设置

| 功能名 | 用户可见行为 | 证据（文件:行） | 完整度 | 优先级 |
|---|---|---|---|---|
| 设置入口 | Ctrl+S 或 Footer「设置」→ SettingsPage；ShellBackButton 返回聊天 | `MainWindow.xaml:282-290,96-110`；`MainWindow.xaml.cs:8175-8368` | 完整 | P0 |
| 设置分区导航 | 12 分区：general / personalization / models / plugins / skills / memory / pet / agent-presets / computer-control / browser-control / usage / about | `MainWindow.xaml.cs:8123` | 完整 | P0 |
| schema 驱动渲染 | `settings/describe` 返回 ns 的 schema+value；`settings/mutate` JSON-Patch ops 保存 | `MainWindow.xaml.cs:16527-16571,11873,13105-13163` | 完整 | P0 |
| 通用：默认权限模式 | permission.defaultPreset（仅可查看/工作区内修改/完全权限） | `MainWindow.xaml.cs:10538-10558` | 完整 | P0 |
| 通用：主题 | ui-theme.preference 浅色/深色/跟随系统 | `MainWindow.xaml.cs:10560-10563,18557-18616` | 完整 | P0 |
| 通用：字号 | ui-theme.fontSize 会话正文字号步进器 | `MainWindow.xaml.cs:10565,10283` | 完整 | P1 |
| 通用：对话显示 | ui-chat.transcriptView 标准/紧凑 | `MainWindow.xaml.cs:10568-10573` | 完整 | P1 |
| 通用：繁忙发送行为 | ui-conversation.busyEnter 排队/插话 | `MainWindow.xaml.cs:10575-10579` | 完整 | P1 |
| 通用：界面语言 | 壳 28 语 + 跟随内核 locale.preference | `MainWindow.xaml.cs:490-574,10582-10586` | 完整 | P0 |
| 通用：系统通知开关 | shell.json `showNotifications` | `MainWindow.TraySettings.cs:262-278` | 完整 | P1 |
| 通用：托盘与退出 | 显示托盘图标 / 最小化到托盘 / 关闭到托盘 | `MainWindow.TraySettings.cs:225-368` | 完整 | P1 |
| 模型页 | 见 §4 | — | 完整 | P0 |
| 插件页（配置） | shell / agent-loop / subagent-model-selection / web-search-deepseek 泛化 schema 渲染 | `MainWindow.xaml.cs:12332-12523,10424` | 完整 | P0 |
| 插件页（清单） | `pluginInventory/list` 只读快照 + 各 Agent 预设组装行 | `MainWindow.xaml.cs:12580-12592` | 只读 | P1 |
| 插件页（挂载） | 默认插件组合 mount 条目开关（profile patch 热重载） | `MainWindow.xaml.cs:12543-12571`；`Dsh/DshPluginBootstrap.cs:748-807` | 完整 | P1 |
| 技能页 | 扫描技能根目录 + 启用/禁用 + 添加 + 移除（本地文件操作，skills/list 仅元数据） | `MainWindow.Skills.cs:83-729` | 完整 | P1 |
| 记忆页 | memory-reference mount 开关 + JSONL 记忆编辑器（自动保存/规范化/文件监视热更） | `MainWindow.xaml.cs:13945-14631` | 完整 | P1 |
| 宠物页 | 见 §16 | — | 完整 | P2 |
| Agent 预设页 | 列表/复制为/删除/设默认/打开目录/应用到会话 | `MainWindow.xaml.cs:12796-13083` | 完整 | P1 |
| 电脑控制页 | computer-use 注册表 + Cua Driver mount 开关 | `MainWindow.xaml.cs:13992-14011` | 完整 | P1 |
| 浏览器控制页 | browser-use 注册表 + Playwright mount 开关 | `MainWindow.xaml.cs:14017-14035` | 完整 | P1 |
| 使用统计页 | 见 §17 | — | 完整 | P1 |
| 关于页 | 壳版本 / 内核版本 / GitHub Release 检查更新 / 下载安装 | `MainWindow.About.cs:82-320` | 完整 | P1 |
| 维护卡 | 「恢复本页默认」`settings/replace` 重置分区 namespace | `MainWindow.xaml.cs:10430-10520` | 完整 | P1 |
| 打开设置文档 | `settings/openSettingsDocument` | `MainWindow.xaml.cs:10460-10468` | 完整 | P2 |
| 设置子页/面包屑 | 分区下钻子页 + 返回父级 | `MainWindow.xaml.cs:8869-8992` | 完整 | P1 |

## 6. 插件与技能

| 功能名 | 用户可见行为 | 证据（文件:行） | 完整度 | 优先级 |
|---|---|---|---|---|
| 技能目录（能力菜单） | `skills/list` → 列表，点击只插入 `/名称` 到输入框不发送 | `MainWindow.Capabilities.cs:320-365` | 完整 | P1 |
| 技能管理（设置页） | 扫描 user/project/bundle 技能根；启停 user-invocable；添加/移除技能 | `MainWindow.Skills.cs:83-729` | 完整 | P1 |
| 斜杠命令目录 | `commands/list` → CommandPalette（↑↓/Enter/Esc/Tab） | `MainWindow.xaml:757-831`；`MainWindow.xaml.cs:17663-18238` | 完整 | P0 |
| 斜杠命令执行 | `commands/execute`（含 /plan、/permission 等控制命令） | `MainWindow.xaml.cs:18201-18238` | 完整 | P0 |
| 命令变更刷新 | `commands/change` 事件失效缓存 | `MainWindow.xaml.cs:2531` | 完整 | P1 |
| 插件清单只读 | `pluginInventory/list` entries + presets 组装 | `MainWindow.xaml.cs:12580-12592` | 只读 | P1 |
| 自动审批插件控制 | dsh-approval-gate 预设增删 + 判定模型 | `MainWindow.xaml.cs:14048-14301` | 完整 | P1 |
| 网络搜索凭据 | web-search-deepseek 的 apiKeyEnv/baseURL/maxUses + 凭据状态 | `MainWindow.xaml.cs:12462-12523` | 完整 | P1 |

## 7. Goal

| 功能名 | 用户可见行为 | 证据（文件:行） | 完整度 | 优先级 |
|---|---|---|---|---|
| Goal 能力面板 | 能力菜单「目标」→ 对话框：objective/maxGoalRounds + 操作按钮 | `MainWindow.Capabilities.cs:85-226` | 完整 | P1 |
| Goal 读取 | `goals/get` | `MainWindow.Capabilities.cs:137`；`MainWindow.xaml.cs:16083` | 完整 | P1 |
| Goal 创建/编辑 | `goals/create` / `goals/edit` | `MainWindow.Capabilities.cs:165-186` | 完整 | P1 |
| Goal 暂停/恢复/完成 | `goals/pause` / `goals/resume` / `goals/complete` | `MainWindow.Capabilities.cs:209-211` | 完整 | P1 |
| Goal 状态条 | composer 上方 GoalBar 显示 phase/objective + 「管理」 | `MainWindow.xaml:954-987`；`MainWindow.xaml.cs:16071-16217` | 完整 | P1 |
| Goal 投影 | `session/control` projection/goal 帧实时刷新 | `MainWindow.xaml.cs:16190-16217` | 完整 | P1 |

## 8. Schedules

| 功能名 | 用户可见行为 | 证据（文件:行） | 完整度 | 优先级 |
|---|---|---|---|---|
| 计划只读列表 | 能力菜单「计划（只读）」→ 按到期排序展示 kind/prompt/scheduledAt/everySeconds | `MainWindow.Capabilities.cs:226-294` | 只读 | P1 |
| 计划投影 | `session/control` projection/schedule 帧驱动列表 | `MainWindow.xaml.cs:16217-16251` | 只读 | P1 |
| 创建/编辑/删除计划 | **未实现**——不调用任何 schedules RPC（DESIGN.zh.md 已知限制亦然） | 无调用点 | 未接 RPC | P0（与官方对齐时缺口） |

## 9. 计划模式

| 功能名 | 用户可见行为 | 证据（文件:行） | 完整度 | 优先级 |
|---|---|---|---|---|
| 计划模式 chip | composer 显示「计划模式」chip + 退出按钮 | `MainWindow.xaml:1217-1234` | 完整 | P0 |
| 进入/退出计划 | `OnPlanToggleClick` → `commands/execute` `/plan` / `/plan off`（**无独立 RPC**） | `MainWindow.xaml.cs:16671-16688` | 完整 | P0 |
| 计划投影 | plan {active,pending} 随 session/control 刷新 chip | `MainWindow.xaml.cs:15686-15698` | 完整 | P0 |
| 计划评审通知 | exit_plan_mode → 系统 toast（未聚焦时） | `MainWindow.xaml.cs:7505+`（MaybeNotifyTurnEnded） | 完整 | P1 |
| 退出计划弹层/评审 UI | 文档提及「计划评审」文案键，**未见独立评审卡片**（仅通知） | `MainWindow.xaml.cs:726` 仅有 i18n 键 | 部分 | P1 |

## 10. 终端与工具

| 功能名 | 用户可见行为 | 证据（文件:行） | 完整度 | 优先级 |
|---|---|---|---|---|
| 工具调用行 | tool 小字行 + 扫光动画 + 兜底文案 | `MainWindow.MessageActions.cs:522-652` | 完整 | P0 |
| 工具审批 | 见 §3 | — | 完整 | P0 |
| 在应用中打开 | composer「…」→ OpenInApp 子菜单 → `GET /open-in-app/apps` + `POST /open-in-app/open` | `MainWindow.xaml:1121-1125`；`MainWindow.xaml.cs:7981-8066` | 完整 | P1 |
| 后台作业面板 | JobsTriggerButton → 运行中的命令/子代理作业列表（session/control jobs 帧） | `MainWindow.xaml:391-420`；`MainWindow.xaml.cs:15899-16001` | 只读 | P1 |
| 排队面板 | QueuePanel 显示 queued 消息；可编辑/删除/插话 | `MainWindow.xaml:905-916`；`MainWindow.xaml.cs:15854-16957` | 完整 | P0 |
| 排队项编辑 | `session/updateQueue` action=edit/remove/steer | `MainWindow.xaml.cs:16894-16910` | 完整 | P0 |
| 繁忙时排队/插话 | busyEnter 语义 + Ctrl+Enter 反向档 | `MainWindow.xaml.cs:6983-7095` | 完整 | P0 |

## 11. 文件与交付物

| 功能名 | 用户可见行为 | 证据（文件:行） | 完整度 | 优先级 |
|---|---|---|---|---|
| 工作区文件面板 | composer「…」→「工作区文件」→ TreeView 树 + 文本预览 + 刷新 | `Pages/FilesPanel.xaml(.cs)` | 完整 | P0 |
| 文件列表/展开 | `workspaceFiles/list` | `Pages/FilesPanel.xaml.cs:242` | 完整 | P0 |
| 文件预览 | `workspaceFiles/stat` + `workspaceFiles/read` | `Pages/FilesPanel.xaml.cs:527-531` | 完整 | P0 |
| 文件变更流 | `workspaceFiles/changes` mux 流 + 去抖刷新 | `Pages/FilesPanel.xaml.cs:625-638` | 完整 | P1 |
| 交付物卡片 | `deliverables/presented` 事件 → 交付物卡（名称/路径/描述 + 打开/显示） | `MainWindow.xaml:564-600`；`MainWindow.xaml.cs:5938-5956` | 完整 | P0 |
| 交付物打开/定位 | `POST /api/present.open?…&action=open\|reveal`（宿主路由，非 typert RPC） | `MainWindow.xaml.cs:7914-7964` | 完整 | P0 |
| 本轮文件改动 | 见 §2 | — | 完整 | P1 |
| 撤回本轮修改 | 见 §2 | — | 完整 | P1 |

## 12. 工作区

| 功能名 | 用户可见行为 | 证据（文件:行） | 完整度 | 优先级 |
|---|---|---|---|---|
| 工作区列表 | 侧栏「工作区」分组 + 视图选项（分组/排序） | `MainWindow.xaml:235-277`；`MainWindow.xaml.cs:2762-2812,3924-4040` | 完整 | P0 |
| 工作区树跟随 | `workspace/follow` baseline + upsert/remove/order/archived 增量 | `MainWindow.xaml.cs:3924-3933`；`Dsh/DshRpcClient.cs:383-387` | 完整 | P0 |
| 添加工作区 | AddWorkspaceButton → 目录浏览器 → `workspace/create` | `MainWindow.xaml:263-275`；`MainWindow.xaml.cs:3511-3573` | 完整 | P0 |
| 目录浏览器 | `directoryPicker/list` / `pick` / `createDirectory` | `MainWindow.xaml.cs:3541-3761` | 完整 | P0 |
| 重命名工作区 | `workspace/rename` | `MainWindow.xaml.cs:3428-3450` | 完整 | P0 |
| 删除工作区 | `workspace/delete` | `MainWindow.xaml.cs:3460-3480` | 完整 | P0 |
| 工作区排序 | `workspace/insertBefore` 上移/下移 | `MainWindow.xaml.cs:3829-3853` | 完整 | P1 |
| 打开工作区路径 | `session/openWorkspacePath`（reveal） | `MainWindow.xaml.cs:3776-3794` | 完整 | P1 |
| 工作区路径能力探测 | `session/canOpenWorkspacePath` | `MainWindow.xaml.cs:3805-3817` | 完整 | P1 |
| 新会话工作区选择 | WorkspacePickerButton MenuFlyout | `MainWindow.xaml:1139-1159` | 完整 | P0 |
| 会话归档/移动 | 见 §1 | — | 完整 | P0 |

## 13. 主题与个性化

| 功能名 | 用户可见行为 | 证据（文件:行） | 完整度 | 优先级 |
|---|---|---|---|---|
| 主题 light/dark/system | `ui-theme.preference`；system 实时跟随 Windows 深浅色（WM_SETTINGCHANGE） | `MainWindow.xaml.cs:18557+`；`MainWindow.ShellIntegration.cs:34` | 完整 | P0 |
| 窗口材质 | Mica / Mica Alt / 亚克力 / 无 | `MainWindow.Personalization.cs:77-102`；`MainWindow.xaml.cs:18367-18406` | 完整 | P1 |
| 气泡材质 | 半透明 / 亚克力 / 跟随窗口 + 透明度滑杆 | `MainWindow.Personalization.cs:126-189`；`MainWindow.xaml.cs:18423-18539` | 完整 | P1 |
| 自定义指令（AGENTS.md） | 个性化页编辑 → `$DSH_HOME/AGENTS.md`；显式保存/清空删文件；草稿归窗口 | `MainWindow.Personalization.cs:189-378` | 完整 | P1 |
| 背景皮肤（图片） | 导入图片作内容区背景，Opacity 可调，存 `shell-skin.img` | `MainWindow.Personalization.cs:394-475`；`MainWindow.xaml.cs:8405-8706` | 完整 | P1 |
| 背景皮肤（视频） | 导入视频背景 + 失焦暂停开关，存 `shell-skin.video` | `MainWindow.xaml.cs:8408-8776` | 完整 | P2 |
| Wallpaper Engine 皮肤 | 个性化子页：`dsh-wallpaper/api/list` 壁纸库 → 下载应用 | `MainWindow.Wallpaper.cs`；`Dsh/DshWallpaperClient.cs:46-116` | 完整 | P2（壳增强） |
| 声明画刷探针 | ThemeProbe* TextBlock 取窗口主题 brush（非 App 快照） | `MainWindow.xaml:35-66`；`Pages/PageTokens.cs` | 完整 | P2 |

## 14. 快捷键

| 功能名 | 用户可见行为 | 证据（文件:行） | 完整度 | 优先级 |
|---|---|---|---|---|
| Ctrl+S | 打开/切换设置 | `MainWindow.xaml:288-290`；`MainWindow.xaml.cs:2337-2338` | 完整 | P0 |
| Enter | 发送（或 busyEnter 排队/插话） | `MainWindow.xaml.cs:6902-6926` | 完整 | P0 |
| Shift+Enter | 换行 | `MainWindow.xaml.cs:6917-6920` | 完整 | P0 |
| Ctrl+Enter | 繁忙时取 busyEnter 反向档 | `MainWindow.xaml.cs:6923-6926` | 完整 | P1 |
| `/` | 唤起命令面板 | `MainWindow.xaml.cs:17739-18051` | 完整 | P0 |
| `@` | 唤起引用面板（文件/会话） | `MainWindow.xaml.cs:17764-17943` | 完整 | P0 |
| ↑↓ Tab Enter Esc（面板） | 命令/引用面板导航 | `MainWindow.xaml.cs:17953-18146` | 完整 | P1 |
| Esc | 关闭浮层/面板 | `MainWindow.xaml.cs:9375-9389` | 完整 | P1 |

## 15. 通知

| 功能名 | 用户可见行为 | 证据（文件:行） | 完整度 | 优先级 |
|---|---|---|---|---|
| 系统 toast | AppNotification：内核失败 / 未聚焦审批 / 提问 / 任务完成（turn/end） | `MainWindow.ShellIntegration.cs`；`MainWindow.xaml.cs:7498-7523` | 完整 | P1 |
| 托盘气球回退 | 无包身份时 NIF_INFO 气球 | `MainWindow.ShellIntegration.cs:171-211` | 完整 | P2 |
| 通知开关 | shell.json `showNotifications` + `ShouldNotify()` 准入 | `MainWindow.TraySettings.cs:262-278`；`MainWindow.xaml.cs:7498` | 完整 | P1 |
| 点击回前台 | toast `NotificationInvoked` / 气球 `NIN_BALLOONUSERCLICK` → `ActivateFromTray` | `MainWindow.ShellIntegration.cs:152-171` | 完整 | P1 |
| 更新提示 toast | GitHub 新版本 toast 深链到关于页 | `MainWindow.UpdateCheck.cs:98-129` | 完整 | P2 |

## 16. 其他（壳增强）

| 功能名 | 用户可见行为 | 证据（文件:行） | 完整度 | 优先级 |
|---|---|---|---|---|
| 桌面宠物（壳增强） | 独立置顶透明分层窗 + 设置页显隐/尺寸/选择/安装/诊断；点击互动 | `MainWindow.Pet.cs`；`Dsh/PetWindow.xaml.cs`；`Dsh/PetSurfaceLayer.cs`；`Dsh/DshPetClient.cs` | 完整 | P2 |
| 宠物库管理（壳增强） | zip 拖放/命令安装/删除；Codex Pet 包装进 `$DSH_HOME/pets/` | `Dsh/DshPetStore.cs:141-452`；`MainWindow.Pet.cs:548-736` | 完整 | P2 |
| 宠物状态轮询（壳增强） | `GET /api/pet/state` 轮询驱动图集帧动画 | `MainWindow.Pet.cs:109-159`；`Dsh/DshPetClient.cs:106` | 完整 | P2 |
| 壁纸引擎（壳增强） | 见 §13 | — | 完整 | P2 |
| 托盘图标（壳增强） | 左键唤起窗口，右键 Win32 菜单：打开/退出 | `MainWindow.ShellIntegration.cs:101-280` | 完整 | P1 |
| 托盘设置（壳增强） | 显示图标/最小化到托盘/关闭到托盘 | `MainWindow.TraySettings.cs:225-368` | 完整 | P2 |
| 运行状态条（壳增强） | 「{轮}轮 {步}步 · {tok/s}」+「{累计}tok · 缓存命中{%}」 | `MainWindow.RunStats.cs:203-249`；`MainWindow.xaml:999-1003` | 完整 | P1 |
| 会话统计胶囊（壳增强） | llmMs / toolMs / TTFT 均值 / TPS | `MainWindow.RunStats.cs:266-284` | 完整 | P1 |
| Token 用量胶囊（壳增强） | 总量/缓存命中/未缓存输入/缓存读取/输出 | `MainWindow.RunStats.cs:284-350` | 完整 | P1 |
| 上下文计量环（壳增强） | ContextMeterRing 百分比环 + 点开详情 | `MainWindow.ContextMeter.cs`；`MainWindow.xaml:1259-1291` | 完整 | P1 |
| TurnRail（壳增强） | 轮次刻度轨 + 悬停预览 + 点击跳转 | `MainWindow.TurnRail.cs`；`MainWindow.xaml:617-690` | 完整 | P2 |
| 图片灯箱（壳增强） | 见 §2 | — | 完整 | P2 |
| 使用统计页（壳增强） | KPI/热力图/趋势图/模型占比环图；journal 聚合 | `MainWindow.xaml:1411-1715`；`MainWindow.xaml.cs:14631-15514` | 完整 | P1 |
| 内核启动面板（壳增强） | 4 步进度 + 重试 | `MainWindow.xaml:707-720`；`MainWindow.KernelBoot.cs` | 完整 | P1 |
| 更新检查/下载安装（壳增强） | GitHub releases/latest 比对 + 下载 MSIX 安装 | `MainWindow.About.cs:148-320`；`MainWindow.UpdateCheck.cs` | 完整 | P1 |
| 引用面板（@ 文件/会话） | `fileReferences/list` + `sessionReferenceResolver/candidates` | `MainWindow.xaml:833-900`；`MainWindow.xaml.cs:17819-18023` | 完整 | P0 |
| 重连探针（开发） | `RunReconnectProbeAsync` 模拟断连恢复 | `MainWindow.ReconnectProbe.cs` | 完整 | P2 |
| 布局探针（开发） | `RunLayoutProbeAsync` | `MainWindow.LayoutProbe.cs` | 完整 | P2 |
| 壳本地化 | 28 语言包 + ShellEnglish 字典；`RefreshShellLanguage` 扫描回翻 | `Assets/i18n/*.json`（28 个）；`MainWindow.xaml.cs:490-1875` | 完整 | P1 |
| 崩溃防线 | DispatcherQueue 编组 + async void 兜底 + unhandled 文件 | `MainWindow.xaml.cs:18317+`；`App.xaml.cs` | 完整 | P1 |

---

## 已调用 RPC 方法表

### HTTP JSON RPC（`CallOkAsync` / `CallAsync` / `CallFeedbackAsync`）

| 方法 | 调用点 | 用途 |
|---|---|---|
| `session/list` | MainWindow.xaml.cs:2845,4174,14748 | 会话列表 |
| `session/create` | MainWindow.xaml.cs:4235 | 新建会话 |
| `session/page` | MainWindow.xaml.cs:4509,4609,4633,4669,4761,7385,7427,14822 | 历史分页回放 |
| `session/prompt` | MainWindow.xaml.cs:7238 | 发送消息 |
| `session/cancel` | MainWindow.xaml.cs:3498,7027 | 取消运行 |
| `session/fork` | MainWindow.xaml.cs:3906；MessageActions.cs:812 | 分叉会话 |
| `session/rename` | MainWindow.xaml.cs:3419；MessageActions.cs:855 | 重命名会话 |
| `session/selectModel` | MainWindow.xaml.cs:7188,13367 | 选择模型 |
| `session/modelCatalog` | MainWindow.xaml.cs:2553,13201,14161 | 模型目录 |
| `session/search` | MainWindow.xaml.cs:13592 | 会话搜索 |
| `session/attachment` | MainWindow.xaml.cs:6459 | 历史附件字节 |
| `session/openWorkspacePath` | MainWindow.xaml.cs:3794；MessageActions.cs:1432 | 打开/定位路径 |
| `session/canOpenWorkspacePath` | MainWindow.xaml.cs:3817 | 路径能力探测 |
| `session/updateQueue` | MainWindow.xaml.cs:16910 | 队列编辑/删除/插话 |
| `workspace/create` | MainWindow.xaml.cs:3573 | 创建工作区 |
| `workspace/rename` | MainWindow.xaml.cs:3450 | 重命名工作区 |
| `workspace/delete` | MainWindow.xaml.cs:3480 | 删除工作区 |
| `workspace/insertBefore` | MainWindow.xaml.cs:3853 | 工作区排序 |
| `workspace/insertSessionBefore` | MainWindow.xaml.cs:3888 | 会话移到工作区 |
| `workspace/archiveSession` | MainWindow.xaml.cs:3870 | 归档会话 |
| `directoryPicker/pick` | MainWindow.xaml.cs:3541 | 目录选择 |
| `directoryPicker/list` | MainWindow.xaml.cs:3608,3724 | 目录列表 |
| `directoryPicker/createDirectory` | MainWindow.xaml.cs:3761 | 创建子目录 |
| `settings/describe` | MainWindow.xaml.cs:16539 | 设置 schema+value |
| `settings/mutate` | MainWindow.xaml.cs:11873,11955,13137,16496 | 设置 JSON-Patch 保存 |
| `settings/update` | MainWindow.xaml.cs:12911 | 更新 agent-presets 默认项 |
| `settings/replace` | MainWindow.xaml.cs:10516 | 恢复分区默认 |
| `settings/openSettingsDocument` | MainWindow.xaml.cs:10468 | 打开设置文档 |
| `settings/canOpenAgentPresetDirectory` | MainWindow.xaml.cs:12826 | 预设目录能力 |
| `settings/openAgentPresetDirectory` | MainWindow.xaml.cs:13083 | 打开预设目录 |
| `credentials/describe` | MainWindow.xaml.cs:10736,12487 | 凭据状态 |
| `credentials/set` | MainWindow.xaml.cs:11891,13163 | 写凭据 |
| `credentials/unset` | MainWindow.xaml.cs:11953 | 清凭据 |
| `llm/listConfigurableProviders` | MainWindow.xaml.cs:10659 | 可配置提供方目录 |
| `llm/discoverModels` | MainWindow.xaml.cs:11592 | 发现模型 |
| `agentPresets/list` | MainWindow.xaml.cs:4449,12808 | 预设列表 |
| `agentPresets/copy` | MainWindow.xaml.cs:13005 | 复制预设 |
| `agentPresets/deletePreset` | MainWindow.xaml.cs:13040 | 删除预设 |
| `agentPresets/select` | MainWindow.xaml.cs:13064 | 应用预设到会话 |
| `pluginInventory/list` | MainWindow.xaml.cs:12592 | 插件清单（只读） |
| `skills/list` | Capabilities.cs:323 | 技能元数据 |
| `goals/get` | Capabilities.cs:137；MainWindow.xaml.cs:16083 | 读 Goal |
| `goals/create` / `goals/edit` / `goals/pause` / `goals/resume` / `goals/complete` | Capabilities.cs:186（`goals/`+verb） | Goal CRUD |
| `commands/list` | MainWindow.xaml.cs:17681 | 斜杠命令目录 |
| `commands/execute` | MainWindow.xaml.cs:18238 | 执行斜杠命令 |
| `fileReferences/list` | MainWindow.xaml.cs:17836 | 文件引用候选 |
| `sessionReferenceResolver/candidates` | MainWindow.xaml.cs:17837 | 会话引用候选 |
| `messageFeedback/list` | MainWindow.xaml.cs:17377 | 回读消息反馈 |
| `messageFeedback/put` | MainWindow.xaml.cs:17497 | 提交/更新反馈 |
| `messageFeedback/delete` | MainWindow.xaml.cs:17556 | 撤销反馈 |
| `sessionFeedback/record` | MainWindow.xaml.cs:16626 | 会话反馈备注 |
| `$events/result` | Dsh/DshRpcClient.cs:682 | 审批/提问 waterfall 回传 |

### mux 流（`OpenFollowStreamAsync` / `OpenWorkspaceFollowStreamAsync` / `OpenRemoteStreamAsync`）

| 流端点 | 调用点 | 用途 |
|---|---|---|
| `$events` | DshRpcClient.EnsureMuxAsync（随连订阅） | 全局事件流 |
| `session/follow` | MainWindow.xaml.cs:5426 | 会话 journal 增量 + assistantStream |
| `workspace/follow` | MainWindow.xaml.cs:3933 | 工作区树 baseline+增量 |
| `session/control` | MainWindow.xaml.cs:15540 | 队列/作业/plan/permissions/schedule/goal 投影 |
| `workspaceFiles/changes` | Pages/FilesPanel.xaml.cs:637 | 文件变更通知 |

### 二进制/宿主路由（非 typert RPC）

| 端点 | 调用点 | 用途 |
|---|---|---|
| `POST api/session/uploadFileBinary` | DshRpcClient.UploadBytesAsync | 附件上传 |
| `GET /api/pet/*` | DshPetClient | 宠物状态/图集/诊断 |
| `POST /api/pet/set-pet\|set-config\|interact` | DshPetClient | 宠物控制 |
| `GET dsh-wallpaper/api/list` | DshWallpaperClient | 壁纸库 |
| `GET /open-in-app/apps` | MainWindow.xaml.cs:7990 | 可用外部应用 |
| `POST /open-in-app/open` | MainWindow.xaml.cs:8066 | 在应用中打开 |
| `POST /api/present.open?…` | MainWindow.xaml.cs:7931 | 交付物打开/定位 |
| `GET /api/present.host` | DshRpcClient 注释提及 | 交付物宿主页 |

### `$events` 事件订阅

| 事件名 | 注册点 | 用途 |
|---|---|---|
| `approval/request`（waterfall） | MainWindow.xaml.cs:2598 | 工具审批 |
| `user-questions/request`（waterfall） | MainWindow.xaml.cs:2607 | 用户提问 |
| `api-session/added` | MainWindow.xaml.cs:2600 | 列表刷新 |
| `api-session/removed` | MainWindow.xaml.cs:2601 | 列表刷新 |
| `api-session/status` | MainWindow.xaml.cs:2602 | 运行状态 |
| `commands/change` | MainWindow.xaml.cs:2531 | 命令目录失效 |
| `*`（通用） | DshRpcClient.OnEvent | 调试/日志 |

> **明确未调用的官方域**：`schedules/*`（创建/编辑/删除计划——与 DESIGN.zh.md 已知限制一致）。

---

## 文档与代码不一致项

| # | 类型 | DESIGN.zh.md 说法 | 代码实际 | 证据 |
|---|---|---|---|---|
| 1 | 文档写了但代码未做 / 文档过时 | 「已知限制：无自动更新；升级靠覆盖安装新 MSIX」；「目前无自动更新」 | **已有** GitHub Release 检查 + 下载 MSIX 安装（About 页按钮 + 静默检查 + toast 深链） | DESIGN.zh.md:117,128；`MainWindow.About.cs:148-320`；`MainWindow.UpdateCheck.cs` |
| 2 | 代码做了但文档没写 | 设置分区只列「通用/模型/插件/智能体预设/统计/关于/维护」 | 实际 **12 分区**：+personalization / skills / memory / pet / computer-control / browser-control；「维护」是分区内卡片而非独立分区 | DESIGN.zh.md:42；`MainWindow.xaml.cs:8123,10430` |
| 3 | 文档歧义/可能误导 | 「能力菜单：Goal / Schedules（只读投影，不创建编辑删除）/ Skills」 | **Goal 不是只读**——`goals/create/edit/pause/resume/complete` 全有；只有 Schedules 真正只读 | DESIGN.zh.md:45,129；`MainWindow.Capabilities.cs:121-211` |
| 4 | 代码做了但文档没写 | 功能面未提 | 消息反馈（赞/踩/备注）+ 会话反馈（sessionFeedback/record） | `MainWindow.xaml.cs:16556-17663` |
| 5 | 代码做了但文档没写 | 功能面未提 | 排队/插话（queue/steer）+ 队列编辑 + 后台作业面板 | `MainWindow.xaml.cs:15854-16957` |
| 6 | 代码做了但文档没写 | 功能面未提 | 本轮文件改动 chips + 撤回本轮修改 + 撤回/编辑用户消息 | `MainWindow.MessageActions.cs:86-1282` |
| 7 | 代码做了但文档没写 | 功能面未提 | 命令面板（/）与引用面板（@）完整交互 | `MainWindow.xaml.cs:17663-18238` |
| 8 | 代码做了但文档没写 | 功能面未提 | 工作区文件面板（FilesPanel + workspaceFiles/*） | `Pages/FilesPanel.xaml.cs` |
| 9 | 代码做了但文档没写 | 功能面未提 | 交付物卡片 + present.open 打开/定位 | `MainWindow.xaml.cs:5938-7964` |
| 10 | 代码做了但文档没写 | 功能面未提 | 用户提问（ask-user）完整问答卡 | `MainWindow.xaml.cs:7620-7891` |
| 11 | 代码做了但文档没写 | 功能面未提 | 子代理会话只读 + 返回父会话 | `MainWindow.xaml.cs:3181-3190,16033-16057` |
| 12 | 代码做了但文档没写 | 功能面未提 | 计划模式 chip + 权限预设下拉 + Goal 状态条 | `MainWindow.xaml:954-1234`；`MainWindow.xaml.cs:15686-16696` |
| 13 | 代码做了但文档没写 | 功能面未提 | 上下文计量环、TurnRail、图片灯箱、运行状态条两胶囊 | `MainWindow.ContextMeter.cs`；`TurnRail.cs`；`ImageLightbox.cs`；`RunStats.cs` |
| 14 | 代码做了但文档没写 | 功能面未提 | 桌面宠物全套、壁纸引擎、记忆编辑器、电脑/浏览器控制页 | `MainWindow.Pet.cs`；`Wallpaper.cs`；`MainWindow.xaml.cs:13945-14035` |
| 15 | 代码做了但文档没写 | 功能面未提 | 「在应用中打开」外部应用集成 | `MainWindow.xaml.cs:7981-8066` |
| 16 | 代码做了但文档没写 | 版本史 0.7.8–0.8.1 提及但功能面未列 | 技能管理（添加/移除/启停）、自动审批、网络搜索凭据、Agent 预设 CRUD | `MainWindow.Skills.cs`；`MainWindow.xaml.cs:14048-14301,12796-13083` |
| 17 | 文档表述与代码口径 | 「维护」列为独立设置分区 | 实现为各分区内的「恢复本页默认」卡片 | DESIGN.zh.md:42；`MainWindow.xaml.cs:10430-10520` |
| 18 | 文档写了且代码一致（核对通过） | 会话/审批/follow 兜底/i18n 28 语/模型选择/运行状态条/过程行/托盘 toast | 与代码一致 | DESIGN.zh.md:38-52 |

---

## 对照官方 WebUI 的明显缺口（优先关注）

1. **Schedules 写路径完全缺失**——无任何 `schedules/*` RPC 调用，不能创建/编辑/删除计划（P0）。
2. **计划模式评审 UI 不完整**——仅有 chip 切换与 toast 通知，未见官方「计划评审」交互卡片（P1）。
3. **自动更新仍是半自动**——有检查与下载安装按钮，但无静默后台自动更新（对比官方 electron-updater）（P1）。
4. **插件安装/市场**——插件页只读清单 + mount 开关，无插件安装/卸载/市场（P1，视官方是否提供）。
5. **部分投影为只读**——后台作业只读、插件清单只读、Schedules 只读。
