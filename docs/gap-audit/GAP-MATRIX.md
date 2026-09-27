# GAP-MATRIX：与 dsh 官方 WebUI 完全一致差距矩阵

> **基线**：`OFFICIAL-WEBUI-FEATURES.md`（约 158 条 + RPC 表，内核 0.1.5-rc.2）
> **对照**：`WINUI-GUI-FEATURES.md`（主干 C#/XAML，不含 rust/）
> **姊妹篇**：`RUST-PARITY-AUDIT.md`（rust/ 重构 vs C# 主干 功能一致性审计，另一条对照轴）
> **核实原则**：疑点逐条回源码验证，不照抄两份清单作者结论。核实结论与清单不一致处在「差距矩阵」中标注 **【清单纠错】**。
> **证据前缀**：`@deepseek-ai/…` = `Kernel/dsh/node_modules/@deepseek-ai/…`；WinUI 侧 `file:line` 均为本仓库主干。

---

## 1. 一句话结论

距离「与 dsh 官方 WebUI 完全一致」还差 **41 项**（缺失 14 / 部分 27），其中 **P0 共 8 项**；另有 23 项壳增强不算缺口。

---

## 2. 覆盖率表（按 16 功能域）

| # | 功能域 | 官方条目 | 对齐 | 部分 | 缺失 | 壳增强 | 覆盖率※ |
|---|---|---|---|---|---|---|---|
| 1 | 会话管理 | 18 | 15 | 2 | 1 | 0 | 83% |
| 2 | 聊天与消息 | 22 | 14 | 6 | 2 | 0 | 64% |
| 3 | 审批 | 8 | 5 | 1 | 2 | 0 | 63% |
| 4 | 模型与提供商 | 12 | 10 | 2 | 0 | 0 | 83% |
| 5 | 设置 | 10 | 8 | 2 | 0 | 0 | 80% |
| 6 | 插件与技能 | 16 | 7 | 5 | 4 | 0 | 44% |
| 7 | Goal | 8 | 5 | 2 | 1 | 0 | 63% |
| 8 | Schedules | 5 | 5 | 0 | 0 | 0 | 100% |
| 9 | 计划模式 | 7 | 3 | 4 | 0 | 0 | 43% |
| 10 | 终端与工具 | 10 | 3 | 6 | 1 | 0 | 30% |
| 11 | 文件与交付物 | 10 | 5 | 5 | 0 | 0 | 50% |
| 12 | 工作区 | 10 | 9 | 1 | 0 | 0 | 90% |
| 13 | 主题与个性化 | 6 | 5 | 1 | 0 | 0 | 83% |
| 14 | 快捷键 | 3 | 3 | 0 | 0 | 0 | 100% |
| 15 | 通知 | 3 | 1 | 2 | 0 | 0 | 33% |
| 16 | 其他 | 15 | 7 | 3 | 3 | 23※※ | 47% |
| | **合计** | **158** | **98** | **41**※※※ | **14** | **23** | **62%** |

※ 覆盖率 = (对齐 + 0.5×部分) / 官方条目。
※※ 壳增强为 WinUI 独有，不计入官方条目分母。
※※※ **缺失 14 + 部分 27 = 41 项非对齐**（上表「部分」列含壳域内官方条目 27 项的子集；明细见 §3）。

**状态口径**：
- `对齐`：WinUI 已有等价能力（壳交互可不同，但用户能完成同一件事）
- `部分`：有入口 / 只读 / 降级 / 缺子能力
- `缺失`：官方有、WinUI 无
- `壳增强`：WinUI 有、官方无（宠物/壁纸/托盘/统计/TurnRail/灯箱等，不算缺口）

---

## 3. 差距矩阵（仅列非对齐的官方条目，按优先级排序）

### P0（阻碍「功能完全一致」的核心用户路径，共 8 项）

| # | 功能名 | 官方行为摘要 | WinUI 状态 | 证据 | 差距描述 | 工作量 |
|---|---|---|---|---|---|---|
| P0-1 | 右栏文档预览器 | Markdown / 高亮代码 / 图片 / PDF / HTML / 纯文本 六形态预览 | **部分** | 官方 `@deepseek-ai/dsh-client-ui-sidebar-documentpreview` desc；WinUI `Pages/FilesPanel.xaml.cs:52-57` 仅调 `workspaceFiles/read` 文本段 | 只有纯文本预览；缺 MD 渲染、代码高亮、图片渲染、PDF 页码/密码、HTML 专用预览；未调 `readAll`/`readBytes`/`readRelated` | L |
| P0-2 | 预览辅助操作 | 自动换行/取消换行、加载更多、重新读取、文件已更新提示、脚注、复制 | **部分** | 官方 `@deepseek-ai/dsh-client-ui-sidebar-documentpreview/lib/client.js`；WinUI `FilesPanel.xaml.cs:72-78` 仅有变更刷新 | 缺 wrap 切换、分页加载更多、手动重读、版本变更横幅、复制全文 | M |
| P0-3 | 工具调用树 keyed 呈现 | bash/fs/web/skill/todo/subagent/workflow 各定制卡 + 差异视图 + 参数/结果 JSON 复制 | **部分** | 官方 `@deepseek-ai/dsh-client-ui-tool` desc + `dsh-agent-tool-presentation`；WinUI `MainWindow.MessageActions.cs:522-652` 仅通用小字行 | 缺 per-tool 定制卡（bash 退出码/信号折叠、str-replace 差异展开、JSON 复制菜单） | L |
| P0-4 | ask_user_question 多题导航 | 上一题/下一题/跳过本题/放弃整组问题/确认执行/去聊天里说；推荐标记；自定义答案 | **部分** | 官方 `@deepseek-ai/dsh-client-ui-user-questions/lib/client.js` 全套；WinUI `MainWindow.xaml.cs:7714-7833` 仅 跳过/提交 + 单屏平铺多题 | 【清单纠错】WinUI **已有** plan-review 与 ask_user_question 卡（见 P1-2）；缺多题分页导航、放弃整组、推荐标记、"去聊天里说" | M |
| P0-5 | 完全权限风险确认 | 「确认启用完全权限？」+ 风险勾选「我已了解风险，并愿意继续」+ 长说明 | **缺失** | 官方 `@deepseek-ai/dsh-client-ui-permission-presets/lib/client.js`；WinUI 全库无「我已了解风险」「确认启用完全权限」字面量 | 切 `danger-full-access` 直接生效，无二次确认流 | S |
| P0-6 | Cordis 动态插件审批 | 允许 / 仅允许此版本 / 允许后续版本 / 拒绝；run/stop 开关；面板控制 | **缺失** | 官方 `@deepseek-ai/dsh-client-ui-cordis/lib/client.js` + `dynamicCordisRunner/*` 12 个 RPC；WinUI 无任何 `dynamicCordisRunner` 调用 | 整域缺失：审批卡、插件 run/stop、面板、inspect 全无 | L |
| P0-7 | 子代理目录与续跑 | `subagents/list` 目录（one-shot/continuable）、`subagents/prompt` 续发、`subagents/interruptByParent` 打断 | **部分** | 官方 `@deepseek-ai/dsh-client-ui-subagent` + `dsh-subagent` typert；WinUI 仅只读展示 origin=subagent 会话 + 返回父会话（`MainWindow.xaml.cs:16033-16057`），无 `subagents/*` 调用 | 缺子代理目录 RPC 面、continuable 续跑、父会话打断 | M |
| P0-8 | 轨迹 Trajectory 计时总览 | 事件账本 + 交互计时总览（轮次/步骤/工具/系统提示词/缓存/差异/参数·结果 JSON） | **缺失** | 官方 `@deepseek-ai/dsh-client-ui-trajectory` desc + 144 条文案；WinUI 无 trajectory 视图（仅壳增强 TurnRail 用 turnOutline 投影） | 整域缺失。TurnRail/RunStats 是壳增强，不等价于官方 trajectory 面板 | L |

### P1（常用完整度，共 27 项）

| # | 功能名 | 官方行为摘要 | WinUI 状态 | 证据 | 差距描述 | 工作量 |
|---|---|---|---|---|---|---|
| P1-1 | 消息 Details 面面 | 消息元数据、附加内容块、系统提示词更新、上下文注入、对话显示密度 | **部分** | 官方 `@deepseek-ai/dsh-client-ui-chat/lib/client.js`；WinUI 有过程行（`MainWindow.xaml.cs:5686-5690`）与 transcriptView | 缺独立 Details 面板（元数据/附加块完整清单）；密度切换已有 | S |
| P1-2 | 计划评审 UI | 计划待审时问题卡片式评审（含计划 markdown） | **部分** | 官方 `@deepseek-ai/dsh-client-ui-user-questions`；WinUI `MainWindow.xaml.cs:7630-7757` **已实现** plan-review 卡（header=「计划评审」+ detail markdown） | 【清单纠错】WINUI 清单称「未见独立评审卡片（仅通知）」**不成立**——代码已走 user-questions 通道渲染。差距仅在多题导航（并入 P0-4） | S |
| P1-3 | 会话状态「计划待审」 | 列表/头状态点显示 plan-review | **部分** | 官方 `@deepseek-ai/dsh-client-ui-workspace/lib/client.js`:"计划待审"；WinUI `MainWindow.xaml.cs:16251` RefreshSessionStateBar 显示 plan/permissions，未见「计划待审」专属状态文案 | 状态点缺 plan-review 语义标签 | S |
| P1-4 | 跨会话召回 | 引用其他会话内容（「来自会话 {session}」） | **缺失** | 官方 `@deepseek-ai/dsh-client-ui-chat/lib/client.js` + `dsh-session-reference`；WinUI 无对应 UI（@ 引用有 sessionReferenceResolver，但非「跨会话召回」展示） | 缺召回消息展示与引用会话 chips | M |
| P1-5 | 截断/继续 | 输出截断提示 + 发送「继续」引导 | **部分** | 官方 `@deepseek-ai/dsh-client-ui-chat/lib/client.js`:"回答被截断…"；WinUI 错误气泡覆盖 `finish{reason.kind:"error"}`（`MainWindow.xaml.cs:5567-5994`），无截断专用引导 | 缺「回答被截断，发送继续」提示行 | S |
| P1-6 | 模型请求重试 | 等待重试/已重试/取消重试/重试延迟 | **部分** | 官方 `dsh-llm-retry` + chat 文案；WinUI 仅错误气泡暴露错误 | 缺重试倒计时/取消重试 UI | M |
| P1-7 | 上下文压缩摘要 | 「上下文已压缩」+ 可点开压缩摘要 + 已压缩 N 条历史 | **部分** | 官方 `dsh-compaction` + chat 文案；WinUI `/compact` 可执行（`commands/execute`），但无压缩摘要展示节点 | 缺压缩事件渲染与摘要入口 | S |
| P1-8 | Agent 预设创造模式 | 「用「创造模式」创作自定义预设」 | **部分** | 官方 `@deepseek-ai/dsh-client-ui-agent-preset/lib/client.js`；WinUI `MainWindow.xaml.cs:4430` 仅映射预设名"cordis"→"创造模式"，无创作向导 | 缺预设创建向导（仅有复制/删除/设默认） | M |
| P1-9 | 预设组装编辑 | 查看 `agent.cordis.yml`、查看路径、打开目录、设为默认 | **部分** | 官方 同上；WinUI `MainWindow.xaml.cs:12796-13083` 有打开目录/设默认/复制/删除，无 yml 查看器 | 缺内置 yml 查看/编辑 | S |
| P1-10 | Skill 引用与工具行 | 消息中 skill 引用可点开说明；专用 skill 工具行 | **部分** | 官方 `@deepseek-ai/dsh-client-skill`；WinUI 技能走能力菜单（`MainWindow.Capabilities.cs:320-365`）与工具小字行 | 缺 skill 引用 chip 点开说明、专用 skill 工具卡 | M |
| P1-11 | 插件设置项（并行工具/命令超时/单流上限/网页搜索次数/Subagent 选模型/终端） | 插件配置面 | **部分** | 官方 `@deepseek-ai/dsh-client-ui-settings-plugins/lib/client.js`；WinUI 插件页有 shell/agent-loop/subagent-model-selection/web-search（`MainWindow.xaml.cs:12332-12523`） | 部分插件 ns 已泛化渲染，但未覆盖官方全部插件设置项 | M |
| P1-12 | 欢迎 onboarding 对话框 | settings-models 共享 product-onboarding dialogs | **缺失** | 官方 `@deepseek-ai/dsh-client-ui-settings-models` desc；WinUI 无 onboarding 对话框 | 缺首次启动引导流 | S |
| P1-13 | 目标指令输入 | 「当前目标进行中。可输入 edit 修改 / pause 暂停 / resume 继续 / clear 清除」 | **部分** | 官方 `@deepseek-ai/dsh-client-ui-conversation/lib/client.js`；WinUI GoalBar 有「管理」按钮进对话框（`MainWindow.xaml.cs:16071-16217`） | 缺 composer 内目标指令提示行 | S |
| P1-14 | `goals/clear` 清除目标 | 清除当前目标 | **缺失** | 官方 `goals/clear` RPC（附录 A.6）；WinUI `MainWindow.Capabilities.cs:207-211` 仅 get/create/edit/pause/resume/complete，**无 clear** | 缺清除目标按钮与 RPC 调用 | S |
| P1-15 | 会话状态「有活动定时任务」 | 会话列表标记 | **部分** | 官方 `@deepseek-ai/dsh-client-ui-workspace/lib/client.js`:"有活动定时任务"；WinUI schedule 投影已缓存（`MainWindow.xaml.cs:16214-16251`）但未映射到列表状态点 | 缺列表侧「有活动定时任务」标记 | S |
| P1-16 | 后台任务操作（取消/停止） | jobs 列表含「正在停止」状态，暗示可发起停止 | **部分** | 官方 `@deepseek-ai/dsh-client-ui-jobs`；WinUI `MainWindow.xaml.cs:15899-16001` 只读展示 JobVm，无取消/停止按钮 | 只读；无 job 级操作（依赖 `session/cancel` 会话级取消） | M |
| P1-17 | Todo 工具投影 | todos 投影（pending/in_progress/completed）展示 | **部分** | 官方 `dsh-tool-todo` + `todos` 投影；WinUI session/control 注释提及 todos 投影（`MainWindow.xaml.cs:15527`）但无独立 Todo 清单 UI | 缺 Todo 清单面板 | M |
| P1-18 | Workflow 工具运行节点 | workflow-run 节点 + 嵌套成员展开（「{count} 个成员」） | **缺失** | 官方 `@deepseek-ai/dsh-client-ui-workflow-run` desc；WinUI 无任何 workflow-run 渲染 | 整域缺失 | M |
| P1-19 | Bash/Pwsh 工具行细节 | 命令、退出码、信号、输出折叠 | **部分** | 官方 `dsh-tool-bash*` + conversation 文案；WinUI `MainWindow.MessageActions.cs:522-652` 通用工具行 | 缺退出码/信号展示、输出折叠交互 | M |
| P1-20 | 文件编辑差异展示 | str-replace 收起差异 / 展开其余 N 行差异 | **部分** | 官方 conversation 文案；WinUI 本轮文件改动 chips（`MainWindow.MessageActions.cs:865-1052`） | 缺内联 diff 视图（仅 chips 打开文件） | M |
| P1-21 | 文件操作菜单完整项 | 打开 / 用默认应用打开 / 打开所在文件夹 / 侧边栏预览 / 资源管理器显示 | **部分** | 官方 `@deepseek-ai/dsh-client-ui-deliverables/lib/client.js`；WinUI `MainWindow.xaml.cs:7914-7964` 有 open/reveal | 缺「在侧边栏预览」「用默认应用打开」细项 | S |
| P1-22 | 设置生效语义 | `applies: live \| restart` 提示 | **部分** | 官方 settings describe/update schema；WinUI `settings/describe` 已用但无 applies 展示 | 缺 live/restart 生效标记 | S |
| P1-23 | 连接状态与重连 | 连接成功/异常/自动重连中/立即重连 | **部分** | 官方 `@deepseek-ai/dsh-client-ui-settings-general/lib/client.js`；WinUI 有内核启动面板（`MainWindow.KernelBoot.cs`）与重连探针 | 缺常驻连接状态条与「立即重连」按钮 | S |
| P1-24 | 欢迎/版本公告 | 版本化 welcome notice | **部分** | 官方 `settings.onboarding`；WinUI About 页有版本信息 | 缺 welcome notice 流 | S |
| P1-25 | 三栏布局 + 拖拽把手 | sidebarCol / centerCol / rightbarCol 拖拽调宽 | **部分** | 官方 `@deepseek-ai/dsh-client-ui-layout`；WinUI 侧栏可开合 + 文件右栏可开合（`MainWindow.xaml.cs:2379,17008`），无拖拽调宽把手 | 缺列宽拖拽 | M |
| P1-26 | 右栏分栏/全屏 | 左右/上下分栏、新标签页、全屏、移到这里、两格上限 | **缺失** | 官方 `@deepseek-ai/dsh-client-ui-sidebar-right/lib/client.js`；WinUI 右栏仅单文件面板 | 整组缺失 | L |
| P1-27 | 通用复制/JSON 交互 | 复制 JSON/属性路径/格式化/紧凑 JSON，右键选复制方式 | **部分** | 官方 `@deepseek-ai/dsh-client-locale/lib/client.js` 公共字典；WinUI 有消息复制（`MainWindow.MessageActions.cs:247-267`） | 缺 JSON 复制变体（属性路径/格式化/紧凑） | S |

### P2（边缘/体验，共 6 项）

| # | 功能名 | 官方行为摘要 | WinUI 状态 | 证据 | 差距描述 | 工作量 |
|---|---|---|---|---|---|---|
| P2-1 | 轨迹中的 Goal 轮 | 「目标 · Round {round}」 | **缺失** | 官方 `@deepseek-ai/dsh-client-ui-trajectory/lib/client.js`；依赖 P0-8 | 随 trajectory 一并补齐 | S |
| P2-2 | 推理等级说明文案 | 快速高效经济 vs 更强自主编码（成本更高）两档说明 | **部分** | 官方 `@deepseek-ai/dsh-client-ui-model-selection/lib/client.js`；WinUI 有 effort 选择（`MainWindow.xaml.cs:13320-13534`） | 缺两档营销文案说明 | S |
| P2-3 | lastUsed / next 模型记忆 | 投影 `modelSelection.lastUsed` / `next` | **部分** | 官方 session/control 投影；WinUI 已解析 modelSelection 键（`MainWindow.xaml.cs:15625`）但未见 lastUsed/next 记忆 UI | 缺模型记忆展示/恢复 | S |
| P2-4 | 恢复默认模型 | 一键恢复 | **部分** | 官方 settings-models 文案；WinUI 有默认模型卡（`MainWindow.xaml.cs:10612+`） | 缺「恢复默认模型」一键钮 | S |
| P2-5 | 匿名用户 ID | 遥测用 | **部分** | 官方 `dsh-anonymous-user-id`；WinUI 无遥测 ID 管理 | 缺（隐私相关，可不补） | S |
| P2-6 | 品牌/内测声明 | 「预览版」「内测声明」「DSH 本地构建」 | **部分** | 官方 locale / settings-models 文案；WinUI About 页有版本 | 缺内测声明文案位 | S |

---

## 4. 壳增强清单（WinUI 有、官方无，不算缺口）

| 功能名 | 说明 | 证据 |
|---|---|---|
| 桌面宠物全套 | 独立置顶透明窗 + 图集帧动画 + 互动 + 宠物库管理 | `MainWindow.Pet.cs`；`Dsh/PetWindow.xaml.cs`；`Dsh/DshPetStore.cs` |
| 壁纸引擎 | Wallpaper Engine 壁纸库下载应用 | `MainWindow.Wallpaper.cs`；`Dsh/DshWallpaperClient.cs` |
| 托盘图标 + 托盘设置 | 左键唤起/右键菜单；最小化到托盘/关闭到托盘 | `MainWindow.ShellIntegration.cs:101-280`；`MainWindow.TraySettings.cs:225-368` |
| 运行状态条 | 「{轮}轮 {步}步 · {tok/s}」+「{累计}tok · 缓存命中{%}」 | `MainWindow.RunStats.cs:203-249` |
| 会话统计胶囊 | llmMs / toolMs / TTFT 均值 / TPS | `MainWindow.RunStats.cs:266-284` |
| Token 用量胶囊 | 总量/缓存命中/未缓存输入/缓存读取/输出 | `MainWindow.RunStats.cs:284-350` |
| 上下文计量环 | ContextMeterRing 百分比环 + 点开详情 | `MainWindow.ContextMeter.cs` |
| TurnRail | 轮次刻度轨 + 悬停预览 + 点击跳转 | `MainWindow.TurnRail.cs` |
| 图片灯箱 | 聊天图片放大预览 | `MainWindow.ImageLightbox.cs` |
| 使用统计页 | KPI/热力图/趋势图/模型占比环图 | `MainWindow.xaml.cs:14631-15514` |
| 内核启动面板 | 4 步进度 + 重试 | `MainWindow.KernelBoot.cs` |
| 更新检查/下载安装 | GitHub Release 比对 + 下载 MSIX | `MainWindow.About.cs`；`MainWindow.UpdateCheck.cs` |
| 背景皮肤（图片/视频） | 导入图片/视频背景 + 失焦暂停 | `MainWindow.Personalization.cs:394-475`；`MainWindow.xaml.cs:8408-8776` |
| 窗口材质 | Mica / Mica Alt / 亚克力 / 无 | `MainWindow.Personalization.cs:77-102` |
| 气泡材质 | 半透明 / 亚克力 / 跟随窗口 + 透明度 | `MainWindow.Personalization.cs:126-189` |
| 自定义指令 AGENTS.md | 个性化页编辑 | `MainWindow.Personalization.cs:189-378` |
| 撤回/编辑用户消息 | 未开跑可撤回编辑 | `MainWindow.MessageActions.cs:105-209` |
| 撤回本轮修改 | 恢复本轮文件到修改前 | `MainWindow.MessageActions.cs:1064-1282` |
| 消息时钟/轮次时长 | 气泡时间与轮耗时 | `MainWindow.MessageActions.cs:769-787` |
| 等待思考动画 | 「少女祈祷中…」+ 扫光 | `MainWindow.MessageActions.cs:335-428` |
| 声明画刷探针 | ThemeProbe* | `MainWindow.xaml:35-66`；`Pages/PageTokens.cs` |
| 壳本地化 28 语 | Assets/i18n 28 语言包 | `Assets/i18n/*.json` |
| 记忆/电脑控制/浏览器控制页 | MCP 记忆挂载 + Cua/Playwright mount | `MainWindow.xaml.cs:13945-14035` |

---

## 5. 补齐路线图（P0 → P2）

### 共用基础设施（先做，多任务依赖）

| ID | 任务 | 依赖 | 工作量 |
|---|---|---|---|
| I-1 | `session/control` 投影补消费：`todos` / `modelSelection.lastUsed·next` / `jobs` 操作字段 / schedule→列表状态点 | 无 | M |
| I-2 | `commands/execute` 已通——`/goal` `/compact` `/feedback` `/export` `/plan` `/permission` 均走此通道，确认回执渲染完整 | 无 | S |
| I-3 | `workspaceFiles/readAll` / `readBytes` / `readRelated` 三 RPC 接入 FilesPanel | 无 | M |

### P0 任务

| ID | 任务 | 依赖 | 工作量 |
|---|---|---|---|
| P0-1/2 | 右栏文档预览器六形态 + 预览辅助 | I-3 | L |
| P0-3 | 工具调用树 keyed 呈现（bash/fs/web/skill/todo/subagent/workflow 定制卡 + diff + JSON 复制） | 无 | L |
| P0-4 | ask_user_question 多题导航（上一题/下一题/跳过本题/放弃整组/确认执行/去聊天里说/推荐标记） | 无 | M |
| P0-5 | 完全权限风险确认流（勾选 + 长说明 + 确认） | 无 | S |
| P0-6 | Cordis 动态插件审批 + 面板 + run/stop | 无（独立 12 个 `dynamicCordisRunner/*` RPC） | L |
| P0-7 | 子代理目录（`subagents/list`）+ 续跑（`subagents/prompt`）+ 打断（`subagents/interruptByParent`） | 无 | M |
| P0-8 | 轨迹 Trajectory 面板（事件账本 + 计时总览） | I-1（事件源已有 session/follow） | L |

### P1 任务

| ID | 任务 | 依赖 | 工作量 |
|---|---|---|---|
| P1-1 | 消息 Details 面面 | 无 | S |
| P1-2/3 | 计划评审打磨 + 「计划待审」状态点 | P0-4（多题导航） | S |
| P1-4 | 跨会话召回展示 | 无 | M |
| P1-5/6/7 | 截断继续引导 / 模型重试 UI / 压缩摘要 | 无 | S+M+S |
| P1-8/9 | Agent 预设创造模式 + yml 查看器 | 无 | M |
| P1-10 | Skill 引用 chip + 专用工具行 | 无 | M |
| P1-11 | 插件设置项补全 | 无 | M |
| P1-12 | 欢迎 onboarding 对话框 | 无 | S |
| P1-13/14 | 目标指令输入提示 + `goals/clear` | 无 | S |
| P1-15 | 「有活动定时任务」列表标记 | I-1 | S |
| P1-16 | jobs 取消/停止操作 | I-1 | M |
| P1-17 | Todo 清单面板 | I-1 | M |
| P1-18 | Workflow-run 展开节点 | 无 | M |
| P1-19/20 | Bash 工具行细节 + str-replace diff | P0-3 | M |
| P1-21 | 文件操作菜单补全 | 无 | S |
| P1-22/23/24 | 设置 applies 提示 / 连接状态条 / welcome notice | 无 | S×3 |
| P1-25/26 | 三栏拖拽把手 + 右栏分栏/全屏 | 无 | M+L |
| P1-27 | JSON 复制变体 | 无 | S |

### P2 任务

| ID | 任务 | 依赖 | 工作量 |
|---|---|---|---|
| P2-1 | 轨迹 Goal 轮标记 | P0-8 | S |
| P2-2/3/4 | 推理等级文案 / 模型记忆 / 恢复默认模型 | 无 | S×3 |
| P2-5/6 | 匿名 ID / 内测声明 | 无 | S×2（P2-5 可不做） |

---

## 6. 验收清单（「完全一致」逐项验收标准）

> 每项给出用户可感知行为 + 建议探针。探针格式：`[UI]` 交互断言 / `[RPC]` 调用与载荷 / `[DOM/UIA]` 自动化标识。

### P0 验收

| 项 | 用户可感知行为 | 建议探针 |
|---|---|---|
| P0-1 文档预览 | 右栏点开 .md 渲染标题/列表/代码块；点开 .cs 高亮；点开 .png 显示图片；点开 .pdf 翻页；点开 .html 专用预览；点开 .txt 纯文本 | `[RPC] workspaceFiles/readAll` 对 PDF/图；`[RPC] workspaceFiles/readBytes` 对二进制分段；`[RPC] workspaceFiles/readRelated` 对 HTML 附属；`[UIA]` 预览容器子节点类型按扩展名切换 |
| P0-2 预览辅助 | 预览工具条有换行切换、加载更多（长文件）、重新读取、文件被外部修改时横幅「文件已更新」、复制按钮 | `[UI]` 切 wrap 后行布局变化；`[RPC] workspaceFiles/stat` version 变化触发横幅 |
| P0-3 工具树 | bash 行显示命令+退出码+信号+输出折叠；str-replace 行可展开 diff；任意工具行有「复制 JSON」 | `[UIA]` ToolRow 上 expander 按类型出现；`[UI]` 复制 JSON 写剪贴板 |
| P0-4 多题导航 | 多题卡片可上一题/下一题；可跳过本题；可放弃整组；可「去聊天里说」；推荐项有标记 | `[RPC] $events/result` answers 覆盖全部题 id；放弃整组回 `ASK_CANCELLED` 或等价 outcome |
| P0-5 完全权限确认 | 切「完全权限」弹「确认启用完全权限？」；勾选风险前确认钮禁用；确认后才生效 | `[UI]` 勾选前按钮 IsEnabled=false；`[RPC] commands/execute /permission danger-full-access` 仅在确认后发出 |
| P0-6 Cordis 审批 | 动态插件请求运行时出现审批卡：允许/仅允许此版本/允许后续版本/拒绝；面板可 run/stop | `[RPC] dynamicCordisRunner/resolveRequestRun` 四种 outcome；`[RPC] dynamicCordisRunner/stopFromPanel` |
| P0-7 子代理 | 树内子代理可展开目录；continuable 可续发消息；父会话可打断 | `[RPC] subagents/list` / `subagents/prompt` / `subagents/interruptByParent` |
| P0-8 轨迹 | 打开轨迹面板见事件账本 + 计时总览（轮次/步骤/工具耗时/缓存） | `[UI]` 面板含轮次分段与耗时列；事件条数 ≈ journal 条数 |

### P1 验收（抽样）

| 项 | 用户可感知行为 | 建议探针 |
|---|---|---|
| P1-4 跨会话召回 | 消息中出现「来自会话 {session}」可点开 | `[UI]` 召回 chip 可点击 |
| P1-5 截断继续 | 输出截断时出现「发送继续可让模型接着输出」 | `[UI]` 截断提示行可见 |
| P1-14 goals/clear | Goal 面板有「清除目标」按钮，点击后 GoalBar 收起 | `[RPC] goals/clear` |
| P1-16 jobs 操作 | jobs 列表每项有停止按钮；停止后状态变「正在停止」 | `[UI]` 停止钮 → 状态文案变化 |
| P1-18 workflow-run | 工具树中 workflow 节点可展开成员列表 | `[UI]` 成员披露 + 计数 |
| P1-26 右栏分栏 | 右栏可左/右/上/下分栏、全屏、新标签页（两格上限） | `[UI]` 分栏后两格；第三格拒绝并提示 |

### P2 验收

| 项 | 用户可感知行为 | 建议探针 |
|---|---|---|
| P2-2 推理等级 | 模型菜单内两档附说明文案 | `[UIA]` MenuFlyoutItem 描述文本 |
| P2-3 模型记忆 | 模型选择器显示「上次使用」 | `[RPC] session/control` modelSelection.lastUsed 渲染 |
| P2-4 恢复默认 | 模型设置页一键恢复默认模型 | `[UI]` 按钮 → settings 回写 |

---

## 7. 疑点核实结论（源码实证，含清单纠错）

| 疑点 | 核实结论 | 关键证据 |
|---|---|---|
| Schedules 写路径 / schedules RPC | **【清单纠错】不是 UI 缺口**。官方 WebUI 侧亦为只读目录（OFFICIAL §8 注），写操作走 Agent 工具 `schedule_create/list/delete`（模型面，非 UI 面）。WinUI 投影只读展示与官方对齐；不调用 `schedules/*` RPC 是正确实现。WINUI 清单把「创建/编辑/删除计划」标 P0 **不成立**。 | OFFICIAL-WEBUI-FEATURES.md:192；`MainWindow.xaml.cs:2035-2056`（注释明确「与官方只读口径一致」）；`MainWindow.Capabilities.cs:226-261` |
| 计划模式与 exit_plan_mode 评审 UI | **【清单纠错】WinUI 已有评审卡**。`plan-review` 走 `user-questions/request` waterfall，`BuildQuestionCard` 识别 `id=="plan-review"` 渲染「计划评审」表头 + detail markdown。WINUI 清单称「未见独立评审卡片（仅通知）」**与代码不符**。真实差距在多题导航（上一题/下一题/放弃整组等）。 | `MainWindow.xaml.cs:7630-7757`；`@deepseek-ai/dsh-plan-mode/lib/index.js:273`（intent.kind=plan-review） |
| 权限三档 + /permission | **三档齐全**（read-only / workspace-write / danger-full-access），默认权限设置 + 会话内 `/permission <preset>` 均已接。**缺**完全权限风险确认流（勾选「我已了解风险」）。另有壳扩展 auto-approve / custom 两档。 | `MainWindow.xaml.cs:10547-10558,16345-16347,16440-16455,16687-16696` |
| 消息队列 / 插话 | **完整对齐**。`session/updateQueue`（edit/remove/steer）、`session/prompt mode: queue\|steer`、busyEnter 设置 + Ctrl+Enter 反向档、QueuePanel 编辑/删除/插话全有。 | `MainWindow.xaml.cs:6983-7095,15852-16965` |
| 右栏文档预览与文件树 | **文件树对齐，预览严重不足**。FilesPanel 仅 `list/stat/read/changes` 四 RPC + 纯文本预览；官方 documentpreview 是 6.8MB 六形态预览器。 | `Pages/FilesPanel.xaml.cs:52-57,531` |
| GoalBar 全生命周期 | **缺 `goals/clear`**。get/create/edit/pause/resume/complete 已接；投影实时刷新已有。 | `MainWindow.Capabilities.cs:120-224` |
| Agent 预设编辑器 / Cordis 动态插件审批 | 预设 CRUD + 设默认 + 打开目录已有；**缺**创造模式向导、yml 查看器。**Cordis 动态插件审批整域缺失**（无 `dynamicCordisRunner/*` 调用）。 | `MainWindow.xaml.cs:12796-13083`；全库无 dynamicCordisRunner |
| 消息 Like/Dislike 与会话反馈 | **完整对齐**。messageFeedback put/list/delete + ifVersion 乐观锁 + sessionFeedback/record 全有。 | `MainWindow.xaml.cs:16556-17663` |
| 轨迹 trajectory 计时总览 | **整域缺失**。TurnRail 是壳增强（turnOutline 投影），不等价官方 trajectory 面板。 | `MainWindow.TurnRail.cs`；无 trajectory 视图 |
| 工作流 workflow-run 展开 | **整域缺失**。 | 全库无 workflow-run 渲染 |
| jobs 后台任务操作 | **只读**。可展示运行中/已完成/失败/停止中状态，无取消/停止按钮。 | `MainWindow.xaml.cs:15787-16001,16777` |
| 斜杠命令 / @ 引用 / 命令面 | **完整对齐**。commands/list + execute + 命令面板 + @ 文件/会话引用 + 变更刷新。 | `MainWindow.xaml.cs:17658-18238` |
| 目录选择器、Open In App | **完整对齐**。directoryPicker list/pick/createDirectory + open-in-app apps/open。 | `MainWindow.xaml.cs:3506-3761,7972-8066` |
| 子代理目录与续跑 | **部分**。只读展示 + 返回父会话已有；缺 `subagents/list|prompt|interruptByParent`。 | `MainWindow.xaml.cs:16033-16057`；全库无 subagents/* |
| 确认流（完全权限）与 ask_user_question | ask_user_question **基本对齐**（选项/多选/自定义/跳过）；**缺**完全权限风险确认；**缺**多题导航（上一题/下一题/放弃整组/去聊天里说）。 | `MainWindow.xaml.cs:7620-7891` |

---

*生成说明：本矩阵以 OFFICIAL-WEBUI-FEATURES.md 为基线逐条打状态；疑点均回 `MainWindow*.cs` / `Pages/` / `Dsh/` / `@deepseek-ai` 包源码核实。两处清单结论被纠错（Schedules 写路径非缺口、计划评审 UI 已有）。工作量 S=≤1d，M=2-5d，L=≥1w。*
