# 官方 dsh Web UI 完整功能面清单

> **用途**：与本仓库 Blade²（WinUI 3 原生壳）主干 GUI 做功能完全一致差距校对。
> **版本对齐**：内核 `0.1.5-rc.2`（`Kernel/dsh/package.json`）。
> **架构事实**（`check-dsh-contract.ps1:18-21` 注释，本地运行时实测）：页面 DOM **不**来自 `dsh-web-frontend` 静态 bundle，而是服务端把 **~162 个 `dsh-client-*` 插件**的 `client.js` 注入 HTML（`/plugins/??.../client.js`）。功能面的真实载体是这些插件包，而非单一前端。
>
> **信息置信度标记**：
> - **[本地]** = 本地 node_modules 代码/字面量/typert 描述符证实
> - **[契约]** = `check-dsh-contract.ps1` 注释或本仓库桥接脚本已实测确认
> - **[文档]** = 官方在线文档证实（本次抓取 `deepseek-harness.github.io` / GitHub README 均 404/失败，故**无独立文档条目**；产品级描述均回落到本地 package.json description）
> - **[推测]** = 由投影 schema / 方法签名 / 文案合理推断，未直接读到对应 UI 组件源码
>
> **证据路径前缀约定**：`@deepseek-ai/…` 均指 `E:\Syncthing\DshWinUI\Kernel\dsh\node_modules\@deepseek-ai\…`。`lib/client.js` 为浏览器端 UI 入口；`lib/typert.remote-client.js` 为 RPC 方法契约。

---

## 0. 功能域总览

| 功能域 | 主要承载插件 | 条目数 |
|---|---|---|
| 会话管理 | `dsh-client-ui-workspace`, `dsh-client-ui-sidebar`, `dsh-api-session-controller`, `dsh-api-workspace-controller` | 18 |
| 聊天与消息 | `dsh-client-ui-chat`, `dsh-client-ui-conversation`, `dsh-client-ui-attachment`, `dsh-client-ui-message-feedback` | 22 |
| 审批 | `dsh-client-ui-approval`, `dsh-user-approval`, `dsh-client-ui-permission-presets` | 8 |
| 模型与提供商 | `dsh-client-ui-model-selection`, `dsh-client-ui-settings-models`, `dsh-llm` | 12 |
| 设置 | `dsh-client-ui-settings`, `dsh-client-ui-settings-general`, `dsh-api-settings-controller` | 10 |
| 插件与技能 | `dsh-client-ui-settings-plugins`, `dsh-client-ui-settings-plugin-inventory`, `dsh-client-ui-skill`, `dsh-client-ui-cordis`, `dsh-client-ui-agent-preset` | 16 |
| Goal | `dsh-client-ui-goal`, `dsh-goal`, `dsh-tool-goal` | 8 |
| Schedules | `dsh-client-ui-schedule`, `dsh-schedule` | 5 |
| 计划模式 | `dsh-client-ui-plan`, `dsh-plan-mode`, `dsh-client-ui-user-questions` | 7 |
| 终端与工具 | `dsh-client-ui-tool`, `dsh-terminal*`, `dsh-tool-*` | 10 |
| 文件与交付物 | `dsh-client-ui-deliverables`, `dsh-client-ui-sidebar-files`, `dsh-client-ui-sidebar-documentpreview`, `dsh-api-workspace-files` | 14 |
| 工作区 | `dsh-client-ui-workspace`, `dsh-client-ui-directory-picker-*`, `dsh-api-workspace-controller` | 10 |
| 主题与个性化 | `dsh-client-ui-theme`, `dsh-client-locale` | 6 |
| 快捷键 | `dsh-client-ui-conversation`, `dsh-client-ui-commands`（局部） | 3 |
| 通知 | `dsh-client-ui-jobs`（后台任务）、连接状态（settings-general） | 3 |
| 其他 | `dsh-client-ui-subagent`, `dsh-client-ui-trajectory`, `dsh-client-ui-workflow-run`, `dsh-client-ui-reference`, `dsh-client-ui-input-trigger`, `dsh-client-ui-open-in-app`, `dsh-client-ui-brand-official` | 16 |

**合计约 158 条功能**（按下方明细条目计）。

---

## 1. 会话管理

| 功能名 | 用户可见行为 | 证据 | 优先级 | 置信度 |
|---|---|---|---|---|
| 会话多级树 | 左侧栏以树形展示会话（含子代理层级），带标题/时间/状态点 | `dsh-client-ui-sidebar` desc："session multi-level tree, search, grouping, state dots"；`dsh-client-ui-sidebar/lib/client.js`（`[role=treeitem]` 契约）；`check-dsh-contract.ps1:10` | 核心 | [本地][契约] |
| 会话搜索 | 按名称/内容搜索历史会话，结果带 snippet；内容搜索不可用时退化为名称匹配 | `session/search` RPC（`dsh-api-session-controller/lib/typert.remote-client.js`）；`dsh-client-ui-workspace/lib/client.js`："搜索会话…"、"内容搜索暂不可用，仅显示名称匹配。"、"仅显示前 {n} 条结果" | 核心 | [本地] |
| 新建会话 | 侧栏按钮创建空白会话（可在指定工作区内） | `session/create` RPC；`dsh-client-ui-sidebar/lib/client.js`："新建会话"/"新会话"；`dsh-client-ui-workspace/lib/client.js`："在"{name}"中新建会话" | 核心 | [本地][契约] |
| 重命名会话 | 改会话标题 | `session/rename` RPC；`dsh-client-ui-workspace/lib/client.js`："重命名会话"、"会话名称" | 核心 | [本地] |
| 会话分叉（Fork） | 从已完成轮次的最后一条消息分出新会话 | `session/fork` RPC（`atSeq` 可选）；`dsh-client-ui-chat/lib/client.js`："在新对话中分支"、"仅可从已完成轮次的最后一条消息分支"；`dsh-client-ui-workspace/lib/client.js`："分叉会话" | 常用 | [本地] |
| 归档会话 | 将会话从工作区列表移除（文件夹与记录保留） | `workspace/archiveSession` RPC；`dsh-client-ui-workspace/lib/client.js`："归档会话"、"将把"{name}"从工作区列表中移除。文件夹与会话记录会保留…" | 常用 | [本地] |
| 会话列表元数据 | 显示相对时间（刚刚/N分钟/N小时/N天/年月日）、运行中/空闲/等待回答/等待审批/计划待审/已完成 状态点 | `session/list` 返回 `updatedAt/running/blank/parentSessionId/origin/cwd` + `sessionListMetadata` 投影；`dsh-client-ui-workspace/lib/client.js` 状态文案 | 核心 | [本地] |
| 会话分组与排序 | 按工作区分组 / 单列表；排序方式；手动排序；视图选项 | `dsh-client-ui-workspace/lib/client.js`："按工作区"、"单列表"、"分组方式"、"排序方式"、"手动排序"、"视图选项" | 常用 | [本地] |
| 会话分页加载 | 「加载更早」历史 | `session/page` RPC；`dsh-client-ui-chat/lib/client.js`："加载更早" | 核心 | [本地] |
| 实时跟随会话 | 流式接收事件/快照/助手流帧 | `session/follow`（stream）、`session/control`（stream）RPC | 核心 | [本地] |
| 会话标题自动生成 | 首 prompt / LLM 生成标题 | `dsh-session-title`, `dsh-session-title-llm`, `dsh-session-title-first-prompt-llm` 包存在；`title` 投影 | 常用 | [本地] |
| 消息队列（Queue/Steer） | 繁忙时排队发送、插话发送（steering）、编辑/移除排队项 | `session/updateQueue` RPC（edit/remove/steer）；`session/prompt` 的 `mode: queue\|steer`；`dsh-client-ui-conversation/lib/client.js`："排队发送"、"插话发送"、"编辑排队消息"、"保存排队消息"、"{n} 条排队消息"、"繁忙时的发送行为" | 核心 | [本地] |
| 取消/停止生成 | 停止当前运行 | `session/cancel` RPC；`dsh-client-ui-conversation/lib/client.js`："停止生成" | 核心 | [本地] |
| 会话统计 | 本轮用量、TTFT、TPS、缓存命中、token 明细、轮次导航 | `dsh-client-ui-chat/lib/client.js`："会话统计"、"Token 用量"、"首 token 用时（TTFT）"、"输出速度（TPS）"、"缓存命中 {percent}%"、"{turns} 轮 {steps} 步"、"跳转到第 {turn} 轮" | 常用 | [本地] |
| 跨会话召回 | 引用其他会话内容 | `dsh-client-ui-chat/lib/client.js`："跨会话召回"、"来自会话 {session}"、"引用会话 · {labels}"；`dsh-session-reference` | 常用 | [本地] |
| 会话导出 ZIP | `/export` 将当前会话内容导出为 ZIP | `dsh-client-ui-commands/lib/client.js`："将当前会话内容导出为 ZIP"；`dsh-session-log-export` 包 | 常用 | [本地] |
| 子代理会话目录 | 树内展示子代理（one-shot / continuable），可切换、续跑、展开下级 | `subagents/list`, `subagents/prompt`, `subagents/interruptByParent` RPC；`dsh-client-ui-subagent/lib/client.js` 全套文案 | 常用 | [本地] |
| 空态工作区选择 | 无会话时展示「选择一个工作区开始」 | `dsh-client-ui-conversation/lib/client.js`："选择一个工作区开始"、"选择工作区"；`dsh-client-ui-workspace` empty-state slots | 核心 | [本地] |

---

## 2. 聊天与消息

| 功能名 | 用户可见行为 | 证据 | 优先级 | 置信度 |
|---|---|---|---|---|
| 对话时间线 | 消息流（用户/助手/工具/推理），支持折叠、截断提示、加载更多 | `dsh-client-ui-chat` desc："Chat Conversation target, node definitions, renderers, and details surface"；`dsh-client-ui-chat/lib/client.js`："对话"、"… 已截断，共 {total} 字符"、"…还有 {count} 条" | 核心 | [本地] |
| Composer 输入框 | 多行输入，占位文案随模式变化（"描述你想要构建的内容, / 调用指令, @ 文件或对话" / "描述你的任务以生成计划" / "发消息或创建任务…"） | `dsh-client-ui-conversation/lib/client.js` | 核心 | [本地] |
| 发送 / 停止 | 发送按钮 + 运行中切换为停止 | `dsh-client-ui-conversation/lib/client.js`："发送消息"、"发送中…"、"停止生成" | 核心 | [本地] |
| 附件（文件） | 拖拽/选择文件附加；上传中/失败重试；待发送附件列表 | `fileUploads/upload` RPC；`dsh-client-ui-conversation/lib/client.js`："添加附件"、"待发送附件"、"待发送文件"、"文件或图片拖动到此处即可添加"、"上传失败，点击重试"、"{count} 个文件" | 核心 | [本地] |
| 附件（图片） | 图片粘贴/拖拽，缩略图、原图预览、限制提示（张数/大小/分辨率/格式 PNG·JPG·WebP·GIF） | `dsh-client-ui-attachment` desc；`dsh-client-ui-conversation/lib/client.js`："图片限制：最多 {count} 张，每张 {size}"、"仅支持 PNG、JPG、WebP、GIF 格式的图片"、"原图预览"、`imageLimits` 投影 | 核心 | [本地] |
| @ 引用（文件/对话/子代理/技能） | 输入 `@` 弹出候选菜单：文件与文件夹、对话、工作区、子智能体、技能 | `dsh-client-ui-reference` desc："Unified Web @file and @session reference source"；`dsh-client-ui-input-trigger`；`dsh-client-ui-reference/lib/client.js`："文件与文件夹"、"对话"、"工作区"；`fileReferences/list`、`sessionReferenceResolver/candidates` RPC | 核心 | [本地] |
| / 斜杠命令菜单 | 输入 `/` 弹出命令候选（三种 UI 形态 + popupSelect） | `dsh-client-ui-commands` desc；`dsh-client-ui-input-trigger`："指令"、"触发候选建议"；`commands/list`、`commands/execute` RPC | 核心 | [本地] |
| 已知内置命令 | `/feedback` 发送会话反馈；`/export` 导出 ZIP；`/plan` 进出计划模式；`/permission` 切换权限预设；`/goal` 设置/查看长期目标；`/compact` 压缩以上对话 | `dsh-client-ui-commands/lib/client.js` 各 description；`dsh-command-compact`, `dsh-command-feedback`, `dsh-command-goal` | 核心 | [本地] |
| 消息点赞/点踩 | 助手消息操作条上 Like/Dislike | `dsh-client-ui-message-feedback` desc；`messageFeedback/put\|list\|delete` RPC；"好的回答"/"有问题的回答" | 常用 | [本地] |
| 反馈对话框 | 分类（任务结果/指令理解与遵循/产品功能与交互/稳定性和速度/资源使用与费用/安全隐私与权限/其他）+ 详情文本 + 提交 | `dsh-client-ui-message-feedback/lib/client.js` 全套分类文案；`sessionFeedback/record` RPC | 常用 | [本地] |
| 推理/思考块 | 显示模型 reasoning，可折叠（"思考"、"已思考"） | `dsh-client-ui-chat/lib/client.js`："思考"、"已思考"；ContentBlock `type: reasoning` | 核心 | [本地] |
| 工具调用树 | 逐步展示工具调用/结果，支持折叠、差异视图、参数/结果 JSON 复制 | `dsh-client-ui-tool` desc："Client Tool call-tree renderer and keyed per-tool presentation slot"；`dsh-client-ui-chat/lib/client.js`："{count} 次工具调用"、"工具调用"、"工具定义" | 核心 | [本地] |
| 消息 Details 面面 | 查看消息元数据、附加内容块、系统提示词更新、上下文注入 | `dsh-client-ui-chat/lib/client.js`："附加内容块"、"系统提示词更新"、"上下文注入"、"对话显示"（标准/紧凑） | 常用 | [本地] |
| 上下文压缩（Compact） | 显示「上下文已压缩」、压缩摘要可点开；已压缩 N 条历史（约 tokens） | `dsh-compaction`, `dsh-compaction-basic`；`dsh-client-ui-chat/lib/client.js`："上下文已压缩"、"点击查看压缩摘要"、"已压缩 {items} 条历史记录（约 {tokens} tokens）" | 常用 | [本地] |
| 上下文占用环 | Composer 旁圆环显示 `contextPressure`，点开 `contextBreakdown`（系统提示/工具/对话） | `dsh-client-ui-conversation` 源码注释（ContextMeter）："上下文已用 {percent}" | 常用 | [本地] |
| 截断/继续 | 输出被截断时提示可发「继续」 | `dsh-client-ui-chat/lib/client.js`："回答被截断，已有输出保留在对话中。发送"继续"可让模型接着输出。"、"已达到输出 token 上限" | 常用 | [本地] |
| 模型请求重试 | 等待重试模型请求 / 已重试 / 取消重试 / 重试延迟 | `dsh-llm-retry`；`dsh-client-ui-chat/lib/client.js`："等待重试模型请求"、"正在重试模型请求" | 常用 | [本地] |
| 轮次导航 | 第 N 轮锚点，点击跳转/加载 | `dsh-client-ui-chat/lib/client.js`："轮次导航"、"加载并跳转到第 {turn} 轮" | 边缘 | [本地] |
| 对话显示密度 | 标准 / 紧凑 | `dsh-client-ui-chat/lib/client.js`："标准"、"紧凑"、"控制已完成轮次的过程内容" | 边缘 | [本地] |
| 繁忙发送行为设置 | Enter / Cmd+Ctrl+Enter 行为可配（排队 vs 插话） | `dsh-client-ui-conversation/lib/client.js`："繁忙时的发送行为"、"智能体运行时 Enter 键和发送按钮的行为；Cmd/Ctrl+Enter 使用另一行为"、"Cmd/Ctrl+Enter 插话发送全部排队消息" | 常用 | [本地] |
| 交付文件尾注 | 回合末尾展示产出文件列表（见「文件与交付物」） | `dsh-client-ui-deliverables` | 核心 | [本地] |
| 轨迹视图 | Trajectory 事件账本 + 交互计时总览（见「其他」） | `dsh-client-ui-trajectory` | 常用 | [本地] |

---

## 3. 审批

| 功能名 | 用户可见行为 | 证据 | 优先级 | 置信度 |
|---|---|---|---|---|
| 工具越权审批卡 | 「工具 {toolName} 请求越权执行」+ 允许一次 / 拒绝 / 审批详情 | `dsh-client-ui-approval/lib/client.js`："等待审批"、"工具 {toolName} 请求越权执行"、"拒绝"、"审批详情"、"允许一次"；`dsh-client-ui-approval` desc："Approval composer takeover over the scoped Remote Event waterfall" | 核心 | [本地] |
| 权限预设（新会话默认） | General 设置里选默认权限模式 | `dsh-client-ui-permission-presets` desc："a new-session default in General settings"；"选择新会话的默认权限模式" | 核心 | [本地] |
| 权限预设（本会话） | `/permission` 弹层切换当前会话权限 | `dsh-client-ui-permission-presets` desc："a current-session /permission popup over the permissions projection" | 核心 | [本地] |
| 三档权限模式 | 仅可查看 / 工作区内修改 / 完全权限 | `dsh-client-ui-permission-presets/lib/client.js`："仅可查看"、"工作区内修改"、"完全权限"；`dsh-permission-presets` keys: `readonly`, `workspace-write`, `danger-full-access`, `sandbox/mode`, `approval/policy` | 核心 | [本地] |
| 完全权限确认 | 二次确认 + 风险勾选「我已了解风险，并愿意继续」 | `dsh-client-ui-permission-presets/lib/client.js`："确认启用完全权限？"、"我已了解风险，并愿意继续"、"启用完全权限后…"长说明 | 核心 | [本地] |
| 会话状态点「等待审批」 | 会话列表/头部显示待审批 | `dsh-client-ui-workspace/lib/client.js`："等待审批" | 常用 | [本地] |
| 审批策略（approval/policy） | ask / allow / deny 类策略（设置项） | `dsh-permission-presets` keys `approval`, `approval/policy`；`dsh-user-approval` 包 | 常用 | [本地][推测] |
| Cordis 插件运行审批 | 动态插件运行需批准（允许/仅允许此版本/允许后续版本/拒绝） | `dsh-client-ui-cordis/lib/client.js`："Cordis 审批"、"待审批"、"允许"、"仅允许此版本"、"允许此插件的后续版本"、"拒绝" | 边缘 | [本地] |

---

## 4. 模型与提供商

| 功能名 | 用户可见行为 | 证据 | 优先级 | 置信度 |
|---|---|---|---|---|
| 会话级模型选择器 | 头部选本会话模型 + 推理等级（efforts） | `dsh-client-ui-model-selection` desc："Model selection over the shared model catalog, Session projection, and session.selectModel"；`session/selectModel`、`session/modelCatalog` RPC；"选择模型，当前 {model}，推理等级 {effort}" | 核心 | [本地] |
| 模型目录 | 按 provider 分组列出模型（name/description/reasoning.efforts/defaultEffort） | `session/modelCatalog` 返回 `groups[]/models[]`；`dsh-client-ui-model-selection/lib/client.js`："模型目录"、"模型与推理等级" | 核心 | [本地] |
| 推理等级说明 | 快速高效经济 vs 更强自主编码/复杂推理（成本更高）两档文案 | `dsh-client-ui-model-selection/lib/client.js` 两条 description | 常用 | [本地] |
| lastUsed / next 模型记忆 | 投影 `modelSelection.lastUsed` / `next` | `session/control` 投影 schema | 常用 | [本地] |
| 提供方管理（设置） | 添加/编辑/删除自定义提供方；API Key；baseURL；API 风格 | `dsh-client-ui-settings-models/lib/client.js`："添加提供方"、"创建提供方"、"编辑 {provider}"、"删除 {provider}"、"输入 API 密钥"、"接口地址"、"自定义设置"（baseURL 等） | 核心 | [本地] |
| 模型发现 | 「获取可用模型」调用 `llm/discoverModels` | `llm/discoverModels` RPC；"获取可用模型"、"请先填写 API 地址，再获取。"、"该提供方没有列出任何模型，请手动添加。" | 核心 | [本地] |
| 手动添加模型 ID | 添加模型 / 删除模型 / 搜索模型 / 全选取消全选；ID 唯一性校验 | `dsh-client-ui-settings-models/lib/client.js`："添加模型"、"删除模型"、"搜索模型"、"模型 ID 不能为空。"、"每个模型 ID 只能出现一次。" | 核心 | [本地] |
| 模型容量/上下文窗口 | 容量（可 K/M 后缀）、上下文窗口（131072 / 256K / 1M） | 同上 | 常用 | [本地] |
| 恢复默认模型 | 一键恢复 | "恢复默认模型" | 常用 | [本地] |
| 凭证管理 | `credentials/describe\|set\|unset`；API Key 留空保持已存/环境认证 | `dsh-api-settings-controller` credentials RPC；"请输入 API 密钥；留空则保持已存储的密钥。" | 核心 | [本地] |
| 可配置提供方列表 | `llm/listConfigurableProviders`、`llm/listProviders` | RPC 描述符 | 常用 | [本地] |
| DeepSeek 官方 onboarding | 「配置 DeepSeek 官方模型，即可开始使用。」引导；内测声明 | `dsh-client-ui-settings-models/lib/client.js`："配置 DeepSeek 官方模型…"、"内测声明"、"添加一个 API Key 开始使用" | 常用 | [本地] |

---

## 5. 设置

| 功能名 | 用户可见行为 | 证据 | 优先级 | 置信度 |
|---|---|---|---|---|
| 设置 Modal | 右上/侧栏「设置」打开 modal（`aria-haspopup=dialog`），VOzbGW_overlay/close 类 | `dsh-client-ui-settings-general` desc；`check-dsh-contract.ps1:13,232-239` | 核心 | [本地][契约] |
| 设置分区导航 | general / models / plugins / locale（`general.nav` 字典键） | `dsh-client-ui-settings-general/lib/client.js` keys: `general`, `locale`, `models`, `plugins` | 核心 | [本地] |
| 通用设置（General） | 「通用设置」区块；含权限默认、外观、语言等（各插件向 settings 命名空间注册行） | `dsh-client-ui-settings` desc："settings-namespace scope service and the canonical settings slot-type contract"；`dsh-client-ui-settings-general`："通用设置" | 核心 | [本地] |
| 打开配置文件 | 「打开配置文件」直接打开 settings 文档 | `settings/openSettingsDocument` RPC；"打开配置文件" / "无法打开配置文件" | 常用 | [本地] |
| 设置读写与修订 | describe / update / replace / mutate，带 `expectedRevision` 乐观锁；只读部署提示 | `dsh-api-settings-controller` 全套 RPC；"当前部署的设置文档为只读。"、"设置已在其他位置更新。请放弃修改后重试。" | 核心 | [本地] |
| 设置生效语义 | `applies: live \| restart` | settings describe/update 结果 schema | 常用 | [本地] |
| 连接状态与重连 | 顶栏连接指示：连接成功 / 连接异常 / 自动重连中 / 立即重连 | `dsh-client-ui-settings-general/lib/client.js` 全套文案 | 核心 | [本地] |
| 欢迎/版本公告 | 版本化 welcome notice（`settings.onboarding`） | `dsh-client-ui-settings-general` desc："the versioned welcome notice" | 常用 | [本地] |
| 语言切换 | locale 设置行（中文/…） | `dsh-client-locale` desc；`dsh-client-locale/lib/client.js`："语言"、"中文" | 核心 | [本地] |
| 字号/外观设置行 | 见「主题与个性化」 | `dsh-client-ui-theme` | 常用 | [本地] |

---

## 6. 插件与技能

| 功能名 | 用户可见行为 | 证据 | 优先级 | 置信度 |
|---|---|---|---|---|
| 插件设置区 | 「插件」分区：插件配置 + 插件视图 tabs；可配置 host-plane 插件卡片 | `dsh-client-ui-settings-plugins` desc；"插件"、"插件配置"、"插件视图"、"配置和查看本部署已安装的插件。" | 核心 | [本地] |
| 插件清单（只读） | Cordis Loader inventory tab：全局插件/会话插件、运行状态（运行中/未运行/加载中/卸载中/启动失败/等待依赖）、启用于、禁用条件、搜索 | `dsh-client-ui-settings-plugin-inventory` desc + 全套中文文案；`pluginInventory/list` RPC | 常用 | [本地] |
| Agent 预设按会话组插件 | 预设行显示 enabled/conditional/fiberPhase；「由 Agent 预设按会话组成」 | `pluginInventory/list` 返回 `agentPresets[].rows[]`；inventory UI 文案 | 常用 | [本地] |
| Agent 预设选择器 | 新会话默认预设 + 本会话席位（开始时固定） | `dsh-client-ui-agent-preset` desc："the default for later sessions, this session's seat, and the composition editor"；"即将开始的这个会话所用的 Agent 预设"、"本会话运行的 Agent 预设，开始时即固定"；`agentPresets/select\|list\|read` RPC | 核心 | [本地] |
| Agent 预设复制/创建/删除 | 复制预设（本机复制一份，标识符=目录名）、创造模式、删除 | `agentPresets/copy\|deletePreset`；"复制预设"、"用「创造模式」创作自定义预设"、"删除该预设？" | 常用 | [本地] |
| 内置预设描述 | 标准模式（完整编码 Agent）/ 极简模式（仅持久 shell）/ PTC 模式 / 创造模式 | `dsh-client-ui-agent-preset/lib/client.js` 四段 description | 常用 | [本地] |
| 预设组装编辑 | 查看 `agent.cordis.yml`、查看路径、打开目录、设为默认 | 同上："组装（agent.cordis.yml）"、"设为默认"、"打开目录" | 边缘 | [本地] |
| Skill 引用与工具行 | 消息中 skill 引用可点开说明；专用 skill 工具行 | `dsh-client-ui-skill` desc："Web skill references and the dedicated skill tool row"；"查看"、"说明"、"仅用户"；`skills/list` RPC | 常用 | [本地] |
| Skill 目录 | 按会话列出 skills（name/description/whenToUse/modelInvocable） | `skills/list` RPC 结果 schema | 常用 | [本地] |
| Cordis 动态插件卡 | `cordis_define` 工具行 + run/stop 开关；注册/运行/停止/移除/更新 | `dsh-client-ui-cordis` desc + 全套文案；`dynamicCordisRunner/*` RPC | 边缘 | [本地] |
| Cordis 面板 | 左下角设置上方的 Cordis 面板控制运行 | "运行控制在左下角设置上方的 Cordis 面板" | 边缘 | [本地] |
| 插件设置项（已见） | 并行工具调用数、命令超时、单流输出上限、网页搜索次数、Subagent 选模型权限、终端 | `dsh-client-ui-settings-plugins/lib/client.js` | 常用 | [本地] |
| 欢迎 onboarding 对话框 | settings-models 中的共享 product-onboarding dialogs | `dsh-client-ui-settings-models` desc | 常用 | [本地] |

---

## 7. Goal（长期目标）

| 功能名 | 用户可见行为 | 证据 | 优先级 | 置信度 |
|---|---|---|---|---|
| GoalBar | Composer 上方停靠目标条（读 goal 投影） | `dsh-client-ui-goal` desc："GoalBar docked above the composer, read from the goal session projection" | 核心 | [本地] |
| 创建/编辑目标 | 「保存目标」「编辑目标」「取消编辑」；objective + maxGoalRounds | `goals/create\|edit\|get` RPC；"目标内容"、"保存目标" | 核心 | [本地] |
| 暂停/恢复/清除/完成 | 暂停目标 / 恢复目标 / 清除目标 | `goals/pause\|resume\|clear\|complete` RPC；对应按钮文案 | 核心 | [本地] |
| 目标状态展示 | 进行中 / 已暂停 / 受阻 / 未运行；blockedReason | `dsh-client-ui-goal/lib/client.js`；goal 投影 `phase: active\|paused\|blocked\|complete` | 核心 | [本地] |
| 目标指令输入 | 「当前目标进行中。可输入 edit 修改 / pause 暂停 / resume 继续 / clear 清除」 | `dsh-client-ui-conversation/lib/client.js` | 常用 | [本地] |
| `/goal` 命令 | 「设置或查看长期任务目标」 | `dsh-client-ui-commands/lib/client.js`；`dsh-command-goal` | 常用 | [本地] |
| Goal 轮次驱动 | 自动多轮推进（roundsStarted / maxGoalRounds） | `dsh-goal-round-driver`；goal 投影 `roundsStarted` | 常用 | [本地][推测] |
| 轨迹中的 Goal 轮 | 「目标 · Round {round}」 | `dsh-client-ui-trajectory/lib/client.js` | 边缘 | [本地] |

---

## 8. Schedules（计划任务/提醒）

| 功能名 | 用户可见行为 | 证据 | 优先级 | 置信度 |
|---|---|---|---|---|
| 活动提醒目录（只读） | 会话头展示 active Schedule 目录 | `dsh-client-ui-schedule` desc："Read-only active Schedule catalog in the Web Session header" | 常用 | [本地] |
| 提醒时间语义 | 单次 / 每 {value}{unit}一次 / {value}{unit}后；秒/分钟/小时/天；现在到期 / 已逾期 | `dsh-client-ui-schedule/lib/client.js` 全套文案 | 常用 | [本地] |
| 多提醒计数 | 「{count} 个提醒」 | 同上 | 常用 | [本地] |
| 会话状态「有活动定时任务」 | 会话列表标记 | `dsh-client-ui-workspace/lib/client.js`："有活动定时任务" | 边缘 | [本地] |
| Agent 工具：schedule_create/list/delete | 模型可创建/列出/删除会话内提醒 | `dsh-schedule/lib/types/tools.js`：`schedule_create`, `schedule_list`, `schedule_delete` | 常用 | [本地] |

> 注：Web UI 侧为**只读目录**；写操作走 Agent 工具面（`schedule_*`）。未见独立 Schedules 管理页。

---

## 9. 计划模式（Plan Mode）

| 功能名 | 用户可见行为 | 证据 | 优先级 | 置信度 |
|---|---|---|---|---|
| Plan 开关（Composer 控件） | 「plan mode 已开启/已关闭 — 点击关闭/开启（/plan）」 | `dsh-client-ui-plan` desc："Plan-mode composer control: the conversation.input.plan seat over the plan projection and the /plan command channel"；`dsh-client-ui-plan/lib/client.js` | 核心 | [本地] |
| `/plan` 命令 | 「进入或退出计划模式」；`/plan off` | `dsh-client-ui-commands/lib/client.js` | 核心 | [本地] |
| 计划模式 Composer 占位 | 「描述你的任务以生成计划」 | `dsh-client-ui-conversation/lib/client.js` | 核心 | [本地] |
| 计划评审 UI | 计划待审时展示问题卡片式评审 | `dsh-client-ui-user-questions` desc："ask_user_question composer takeover and plan-review presentation UI"；"计划待审" | 核心 | [本地] |
| ask_user_question 接管 | 多题卡片：上一题/下一题/跳过本题/放弃整组问题/确认执行/去聊天里说；推荐标记；自定义答案 | `dsh-client-ui-user-questions/lib/client.js` 全套；`dsh-tool-ask-user`、`dsh-user-questions` | 核心 | [本地] |
| 会话状态「计划待审」 | 列表/头状态点 | `dsh-client-ui-workspace/lib/client.js`："计划待审" | 常用 | [本地] |
| plan projection | 消费 `plan` 投影驱动 UI | `dsh-client-ui-plan` desc | 常用 | [本地][推测] |

---

## 10. 终端与工具

| 功能名 | 用户可见行为 | 证据 | 优先级 | 置信度 |
|---|---|---|---|---|
| 工具调用树渲染 | 按工具的 keyed 呈现插槽（bash/fs/web/skill/todo/subagent/workflow…各有定制卡） | `dsh-client-ui-tool` desc；`dsh-agent-tool-presentation` | 核心 | [本地] |
| Bash/Pwsh 工具行 | 命令、退出码、信号、输出折叠 | `dsh-tool-bash`, `dsh-tool-pwsh`, `dsh-tool-bash-persistent`, `dsh-tool-pwsh-persistent`；`dsh-client-ui-conversation`："退出码 {code}"、"信号 {signal}" | 核心 | [本地] |
| 持久 Shell | 会话内持久终端会话 | `dsh-tool-bash-persistent`, `dsh-tool-pwsh-persistent`, `dsh-terminal`, `dsh-terminal-bash`；"终端 {sessionId}" | 常用 | [本地][推测] |
| 文件编辑（str-replace） | 工作区内修改 / 差异展示 | `dsh-tool-str-replace-editor`；`dsh-client-ui-conversation`："工作区内修改"、"收起差异"、"展开其余 {count} 行差异" | 核心 | [本地] |
| Web 搜索 / 网页获取 | 工具行展示搜索/抓取 | `dsh-tool-web`, `dsh-web-fetch-http`, `dsh-web-search-deepseek`；"网页搜索"、"网页获取" | 核心 | [本地] |
| Todo 工具 | 更新任务清单 | `dsh-tool-todo`；"更新任务清单"；`todos` 投影（pending/in_progress/completed） | 常用 | [本地] |
| Subagent 工具 | 派生子代理 | `dsh-tool-subagent`, `dsh-tool-subagent-control` | 常用 | [本地] |
| Workflow 工具 | 工作流运行节点 + 嵌套成员展开 | `dsh-client-ui-workflow-run` desc；`dsh-tool-workflow`；"{count} 个成员"、"运行中 {count}"、"已完成 {count}" | 边缘 | [本地] |
| Skill 工具 | 调用技能 | `dsh-tool-skill` | 常用 | [本地] |
| 后台任务列表 | 会话头 jobs：运行中/已完成/已失败/已取消/正在停止 + 耗时 | `dsh-client-ui-jobs` desc："Session-header background-job list"；`jobs` 投影；`dsh-jobs`, `dsh-tool-jobs` | 常用 | [本地] |

---

## 11. 文件与交付物

| 功能名 | 用户可见行为 | 证据 | 优先级 | 置信度 |
|---|---|---|---|---|
| 交付文件尾注 | 回合末尾「交付文件」列表；全部 N 个文件；展开/收起 | `dsh-client-ui-deliverables` desc："Produced-files turn tail and clickable final-response file references for Web"；"交付文件"、"本轮文件改动"、"全部 {count} 个文件" | 核心 | [本地] |
| 文件操作菜单 | 打开 / 用默认应用打开 / 打开所在文件夹 / 在侧边栏预览 / 在文件资源管理器(访达)中显示 | `dsh-client-ui-deliverables/lib/client.js` 全套 | 核心 | [本地] |
| 最终回复中的文件引用 | 可点击文件引用跳转/预览 | `dsh-client-ui-deliverables` desc | 核心 | [本地] |
| 右栏文件树 | 工作区文件 tab，懒加载目录列表，点开进侧栏 | `dsh-client-ui-sidebar-files` desc："Workspace file tree tab type for the right Sidebar: lazy directory listing over the workspaceFiles Remote namespace" | 核心 | [本地] |
| 文档预览器 | Markdown、高亮代码、图片、PDF、HTML、纯文本 | `dsh-client-ui-sidebar-documentpreview` desc；"代码"、"图片"、"纯文本"、"HTML 文档预览"、PDF 页码/密码限制 | 核心 | [本地] |
| 预览辅助 | 自动换行/取消换行、加载更多、重新读取、文件已更新提示、脚注、复制 | `dsh-client-ui-sidebar-documentpreview/lib/client.js` | 常用 | [本地] |
| 文件读取 RPC | list/read/readAll/readBytes/readRelated/stat/changes（监视变更） | `dsh-api-workspace-files` 全套 RPC | 核心 | [本地] |
| 文件上传 | 流式接收 + staged receipt（`fileUploads/upload`） | `dsh-client-file-upload` | 核心 | [本地] |
| Open In...（会话头） | 「Open In...」分按钮：文件管理器/访达/终端/选择打开方式，在本地应用打开工作目录 | `dsh-client-ui-open-in-app` desc + 文案；`session/openWorkspacePath`, `session/canOpenWorkspacePath` RPC | 常用 | [本地] |
| 原图查看 | 图片消息点开原图预览 | `dsh-client-ui-conversation/lib/client.js`："查看原图"、"关闭原图预览" | 常用 | [本地] |

---

## 12. 工作区

| 功能名 | 用户可见行为 | 证据 | 优先级 | 置信度 |
|---|---|---|---|---|
| 工作区选择器 | 侧栏 WorkspacePicker；空态工作区槽 | `dsh-client-ui-workspace` desc："Workspace picker plugin: one WorkspacePicker registered into the sidebar and empty-state workspace slots" | 核心 | [本地] |
| 添加工作区 | 「添加工作区…」走目录选择流 | `workspace/create` RPC；"添加工作区" | 核心 | [本地] |
| 目录选择（应用内浏览） | 浏览目录列表、面包屑、新建文件夹、显示隐藏文件、编辑路径、主目录 | `dsh-client-ui-directory-picker-browse` desc + 文案；`directoryPicker/list\|createDirectory\|pick` RPC | 核心 | [本地] |
| 目录选择（OS 原生） | 驱动宿主 OS 目录选择器 | `dsh-client-ui-directory-picker-native` desc："the renderless workspace directory-flow occupant driving the host's OS chooser" | 核心 | [本地] |
| 工作区重命名/删除 | 重命名工作区；删除工作区（会话移到「未分组」） | `workspace/rename\|delete` RPC；"重命名工作区"、"删除工作区"、"将把…移除。…其会话将显示在「未分组」下。" | 常用 | [本地] |
| 工作区手动排序 | `workspace/insertBefore` | RPC + "手动排序" | 边缘 | [本地] |
| 会话在工作区内排序 | `workspace/insertSessionBefore` | RPC | 边缘 | [本地] |
| 工作区实时同步 | `workspace/follow`（baseline/upsert/remove/order/archived） | RPC stream | 核心 | [本地] |
| 未分组会话 | 无工作区归属的会话展示在「未分组」 | `dsh-client-ui-workspace/lib/client.js`："未分组" | 常用 | [本地] |
| 工作区名称冲突校验 | 「已存在名为"{name}"的工作区。」 | 同上 | 边缘 | [本地] |

---

## 13. 主题与个性化

| 功能名 | 用户可见行为 | 证据 | 优先级 | 置信度 |
|---|---|---|---|---|
| 浅色/深色/跟随系统 | Appearance 设置行三选 | `dsh-client-ui-theme` desc："ThemeRuntime for light/dark/system state…Appearance settings row"；"浅色"、"深色"、"跟随系统"、"外观" | 核心 | [本地][契约] |
| 主题 token 体系 | `--dsw-*` / `--dsw-alias-*` CSS 变量；`body[data-ds-dark-theme]` | `dsh-client-ui-theme` desc；`check-dsh-contract.ps1:15,223-224` | 核心 | [契约] |
| 字号大小 | 增大/减小字号（仅影响会话内容） | `dsh-client-ui-theme/lib/client.js`："字号大小"、"增大字号"、"减小字号"、"仅影响会话内容的字号" | 常用 | [本地] |
| 语言（locale） | 中文等，Host-backed preference + 浏览器 fallback | `dsh-client-locale` desc | 核心 | [本地] |
| 品牌位 | 侧栏官方 DeepSeek Harness 品牌 | `dsh-client-ui-brand-official` desc | 边缘 | [本地] |
| 通用复制/JSON 交互 | 复制、复制 JSON/属性路径/格式化/紧凑 JSON，右键选复制方式 | `dsh-client-locale/lib/client.js` 公共字典 | 常用 | [本地] |

---

## 14. 快捷键

| 功能名 | 用户可见行为 | 证据 | 优先级 | 置信度 |
|---|---|---|---|---|
| Enter / Cmd·Ctrl+Enter 双行为 | 发送 vs 插话/排队 可配置 | `dsh-client-ui-conversation/lib/client.js`："繁忙时的发送行为"说明 | 核心 | [本地] |
| `/` `/` 命令、`@` 引用触发 | 输入触发管道 | `dsh-client-ui-input-trigger` desc："Input trigger pipeline: '/' and '@' detection, candidate menu, pick routing" | 核心 | [本地] |
| Plan 开关键 | 文案「按下开启/关闭」暗示键盘触发 | `dsh-client-ui-plan/lib/client.js`："plan mode 已关闭，按下开启" | 边缘 | [推测] |

> 未发现独立的「快捷键设置页」或完整快捷键表（如 Esc/Up-Edit 等）在 locale 字典中显式登记。**壳对照时注意：官方 Web UI 也未暴露快捷键自定义 UI。**

---

## 15. 通知

| 功能名 | 用户可见行为 | 证据 | 优先级 | 置信度 |
|---|---|---|---|---|
| 后台任务通知位 | 会话头 jobs 列表（运行中数量气泡） | `dsh-client-ui-jobs` | 常用 | [本地] |
| 连接状态提示 | 连接异常/自动重连中/立即重连 | `dsh-client-ui-settings-general` | 核心 | [本地] |
| 欢迎公告 | 版本化 welcome notice | `dsh-client-ui-settings-general` desc | 常用 | [本地] |

> 未见系统级 Toast/Notification Center 组件；主要靠状态条与 jobs 列表。

---

## 16. 其他

| 功能名 | 用户可见行为 | 证据 | 优先级 | 置信度 |
|---|---|---|---|---|
| 三栏布局 + 拖拽把手 | AppFrame：sidebarCol / centerCol / rightbarCol，拖拽调宽，`data-slot="sidebar"` | `dsh-client-ui-layout` desc："three-column AppFrame with drag handles, ctx.layout viewing-state service"；`check-dsh-contract.ps1:14,215-221` | 核心 | [本地][契约] |
| 右侧栏停靠 | 右栏面板/头部展开控制、导航服务 | `dsh-client-ui-sidebar-right` desc | 核心 | [本地] |
| 右栏分栏/全屏 | 左右/上下分栏、新标签页、全屏、移到这里、两格上限 | `dsh-client-ui-sidebar-right/lib/client.js`："分栏"、"左分栏"、"右分栏"、"上分栏"、"下分栏"、"全屏"、"新标签页"、"已达两格上限" | 常用 | [本地] |
| 侧栏开合 | 「打开侧边栏」「收起侧边栏」（全局面板） | `dsh-client-ui-sidebar`；`check-dsh-contract.ps1:11` | 核心 | [契约] |
| 轨迹（Trajectory） | 事件账本 + 交互计时总览：轮次/步骤/工具调用/系统提示词/缓存/差异/参数·结果 JSON | `dsh-client-ui-trajectory` desc + 144 条文案 | 常用 | [本地] |
| 工作流运行节点 | 持久 workflow-run 会话节点 + 嵌套成员披露 | `dsh-client-ui-workflow-run` desc | 边缘 | [本地] |
| React 插槽渲染器 | `ctx.uiRenderer` + 应用根；插件动态挂 slot | `dsh-client-ui-renderer` desc | 核心（架构） | [本地] |
| 资源模型 `useResource` | URL 地址 → 活值 | `dsh-client-resources` desc | 边缘 | [本地] |
| 文件上传服务 | 流式 intake + staged receipt | `dsh-client-file-upload` desc | 核心 | [本地] |
| HMR（仅开发） | SSE 重建帧热替换 | `dsh-client-hmr` desc | 边缘 | [本地] |
| 匿名用户 ID | 遥测用 | `dsh-anonymous-user-id` | 边缘 | [本地][推测] |
| GitHub Webhook | 外部触发 | `dsh-webhook`, `dsh-webhook-github` | 边缘 | [本地][推测] |
| 品牌/内测声明 | 「预览版」「内测声明」「DSH 本地构建」 | locale / settings-models / conversation 文案 | 边缘 | [本地] |
| 公共对话框字典 | 确定/取消/关闭/复制/重试/加载中/提交/下一步/上一步/跳过/删除/编辑/保存/搜索/更多/收起/展开/返回 | `dsh-client-locale/lib/client.js` | 常用 | [本地] |
| 代码运行时 | worker-thread 代码执行 | `dsh-code-runtime`, `dsh-code-runtime-worker-thread` | 边缘 | [本地][推测] |

---

## 附录 A：RPC 方法表

> 全部来自各包 `lib/typert.remote-client.js` 的 `TYPERT_REMOTE.descriptors`（**本地代码证实**）。
> 形如 `namespace/method`。

### A.1 会话控制 `dsh-api-session-controller`

| 方法 | 对应 UI 能力 |
|---|---|
| `session/create` | 新建会话（可带 workspaceId/cwd/agentPreset） |
| `session/list` | 侧栏会话树/列表（分页 cursor） |
| `session/search` | 会话搜索框（返回 sessionId+snippet） |
| `session/rename` | 重命名会话 |
| `session/fork` | 分叉会话（可 atSeq） |
| `session/prompt` | 发送消息（queue/steer；text/image/file 附件；clientTimeZone） |
| `session/cancel` | 停止生成 |
| `session/follow` (stream) | 实时会话事件/快照/assistant-stream |
| `session/control` (stream) | 全局 baseline：queues + jobs + projections |
| `session/page` | 加载更早历史 |
| `session/updateQueue` | 编辑/移除/steer 排队消息 |
| `session/selectModel` | 模型选择器 |
| `session/modelCatalog` | 模型目录（groups/models/reasoning.efforts） |
| `session/attachment` | 取回附件（图片 base64） |
| `session/openWorkspacePath` | Open In... / 打开所在文件夹 / reveal |
| `session/canOpenWorkspacePath` | Open In... 可用性探测 |
| `fileReferences/list` | `@` 文件引用候选 |
| `skills/list` | Skill 目录/引用候选 |

### A.2 设置与凭证 `dsh-api-settings-controller`

| 方法 | 对应 UI 能力 |
|---|---|
| `settings/describe` | 设置 modal 加载各命名空间（schema/value/base/user/applies/secrets/revision） |
| `settings/update` | 设置补丁保存 |
| `settings/replace` | 整段替换保存 |
| `settings/mutate` | 精确 set/unset 操作序列 |
| `settings/openSettingsDocument` | 「打开配置文件」 |
| `settings/openAgentPresetDirectory` | 预设「打开目录」 |
| `settings/canOpenAgentPresetDirectory` | 预设目录可打开性 |
| `credentials/describe` | API Key 配置状态（configured/source/writable） |
| `credentials/set` | 保存 API Key |
| `credentials/unset` | 清除 API Key |

### A.3 工作区 `dsh-api-workspace-controller`

| 方法 | 对应 UI 能力 |
|---|---|
| `workspace/create` | 添加工作区 |
| `workspace/rename` | 重命名工作区 |
| `workspace/delete` | 删除工作区 |
| `workspace/follow` (stream) | 工作区列表实时同步 |
| `workspace/archiveSession` | 归档会话 |
| `workspace/insertBefore` | 工作区手动排序 |
| `workspace/insertSessionBefore` | 会话在工作区内排序 |
| `directoryPicker/list` | 应用内目录浏览 |
| `directoryPicker/createDirectory` | 新建文件夹 |
| `directoryPicker/pick` | OS 原生目录选择 |

### A.4 工作区文件 `dsh-api-workspace-files`

| 方法 | 对应 UI 能力 |
|---|---|
| `workspaceFiles/list` | 右栏文件树目录列表 |
| `workspaceFiles/read` | 文本预览（分段） |
| `workspaceFiles/readAll` | 全文/完整文件内容（PDF/图） |
| `workspaceFiles/readBytes` | 二进制分段读取 |
| `workspaceFiles/readRelated` | 相关资源（如 HTML 附属文件） |
| `workspaceFiles/stat` | 文件版本/大小 |
| `workspaceFiles/changes` (stream) | 文件变更监视（「文件已更新」） |

### A.5 命令与反馈

| 方法 | 对应 UI 能力 |
|---|---|
| `commands/list` | `/` 命令候选 |
| `commands/execute` | 执行斜杠命令（可带附件） |
| `messageFeedback/put` | 消息点赞/点踩提交 |
| `messageFeedback/list` | 反馈状态加载 |
| `messageFeedback/delete` | 取消标记 |
| `sessionFeedback/record` | `/feedback` 会话级反馈 |

### A.6 Goal `dsh-goal`

| 方法 | 对应 UI 能力 |
|---|---|
| `goals/create` | 创建目标 |
| `goals/get` | 读取当前目标 |
| `goals/edit` | 编辑目标 |
| `goals/pause` | 暂停目标 |
| `goals/resume` | 恢复目标 |
| `goals/complete` | 完成目标 |
| `goals/clear` | 清除目标 |

### A.7 模型 `dsh-llm`

| 方法 | 对应 UI 能力 |
|---|---|
| `llm/listProviders` | 提供方列表 |
| `llm/listConfigurableProviders` | 可配置提供方（含 settingsNs） |
| `llm/discoverModels` | 「获取可用模型」 |

### A.8 Agent 预设 `dsh-agent-presets`

| 方法 | 对应 UI 能力 |
|---|---|
| `agentPresets/list` | 预设选择器列表（trust/isDefault/broken） |
| `agentPresets/read` | 查看预设内容 |
| `agentPresets/copy` | 复制预设 |
| `agentPresets/deletePreset` | 删除预设 |
| `agentPresets/select` | 为会话选择预设 |

### A.9 子代理 `dsh-subagent`

| 方法 | 对应 UI 能力 |
|---|---|
| `subagents/list` | 子代理目录（one-shot/continuable/diagnostic） |
| `subagents/prompt` | 对 continuable 子代理续发消息 |
| `subagents/interruptByParent` | 父会话打断子代理 |

### A.10 引用与上传

| 方法 | 对应 UI 能力 |
|---|---|
| `sessionReferenceResolver/candidates` | `@` 会话引用候选 |
| `fileUploads/upload` | 附件上传（得 receiptId） |

### A.11 插件 `dsh-host-plugin-inventory` / `dsh-cordis-host-runner`

| 方法 | 对应 UI 能力 |
|---|---|
| `pluginInventory/list` | 插件清单 tab |
| `dynamicCordisRunner/inventory` | Cordis 动态插件清单 |
| `dynamicCordisRunner/getClientCode` | 加载动态插件 client 代码 |
| `dynamicCordisRunner/runHostHalf` | 运行插件（触发审批） |
| `dynamicCordisRunner/resolveRequestRun` | 审批放行/拒绝 |
| `dynamicCordisRunner/settleUserRun` | 结算用户运行 |
| `dynamicCordisRunner/stopFromPanel` | 停止 Cordis 插件 |
| `dynamicCordisRunner/undefineFromPanel` | 移除 Cordis 插件 |
| `dynamicCordisRunner/invoke` | 调用动态插件方法 |
| `dynamicCordisRunner/resolveInspectQuery` | 插件 inspect 查询应答 |
| `dynamicCordisRunner/reportClientGuardFailure` | 客户端守卫失败上报 |
| `dynamicCordisRunner/reportRenderFailure` | 渲染失败上报 |
| `dynamicCordisRunner/syncInspectManifest` | 同步 inspect 清单 |

---

## 附录 B：会话投影（Projection）能力面

`session/control` / `session/follow` 返回的 `projections.values` 字段（**本地代码证实**）直接对应一批 UI：

| 投影键 | UI 能力 |
|---|---|
| `title` | 会话标题 |
| `agentPreset` | 当前预设席位 |
| `todos` | Todo 清单（pending/in_progress/completed） |
| `sessionListMetadata` | 列表时间/空白标记 |
| `imageLimits` | 图片限制提示 |
| `modelSelection` | lastUsed / next 模型 |
| `subagent` / `subagentCatalog` / `subagentTiming` | 子代理目录与计时 |
| `goal` | GoalBar（objective/phase/blockedReason/maxGoalRounds/roundsStarted） |
| `inbox` (`next-turn` / `next-step`) | 队列/steering 注入 |

---

## 附录 C：权限预设取值（`dsh-permission-presets`）

| 取值 | 含义 |
|---|---|
| `readonly` / 仅可查看 | 只读 |
| `workspace-write` / 工作区内修改 | 可写工作区 |
| `danger-full-access` / 完全权限 | 少确认、可敏感操作（需风险勾选） |
| `sandbox/mode` | 沙箱模式 |
| `approval/policy` | 审批策略（ask/allow/deny 类） |

---

## 附录 D：斜杠命令（已证实子集）

| 命令 | 行为 | 证据 |
|---|---|---|
| `/plan`（及 `/plan off`） | 进入或退出计划模式 | `dsh-client-ui-commands/lib/client.js` |
| `/permission` | 切换权限预设（沙箱模式与审批策略） | 同上 |
| `/goal` | 设置或查看长期任务目标 | 同上 |
| `/feedback` | 发送关于当前会话的反馈 | 同上 |
| `/export` | 将当前会话内容导出为 ZIP | 同上 |
| `/compact` | 压缩以上对话内容 | 同上；`dsh-command-compact` |

> 完整命令列表运行时由 `commands/list` 返回（含 hint/attachments 标记），壳应动态拉取而非写死。

---

## 附录 E：与 Blade² 壳对照时最易漏掉的功能（预判）

1. **右栏多形态停靠**（文件树 + 文档预览 + 分栏/全屏/新标签页）— 体量大（documentpreview 6.8MB），交互复杂。
2. **消息队列 / 插话发送（queue vs steer）** — 官方把「繁忙时的发送行为」做成可配置，且排队项可编辑/移除/steer。
3. **审批 + 三档权限预设 + 完全权限风险确认流** — 与工具越权卡、`/permission` 弹层耦合。
4. **GoalBar 生命周期**（create/edit/pause/resume/clear/complete + 状态投影）与 **Schedules 只读目录**。
5. **Agent 预设体系**（选择/复制/创造模式/组装 agent.cordis.yml/按会话组插件）与 **Cordis 动态插件审批运行**。

---

*生成说明：本清单基于本地 `Kernel/dsh/node_modules/@deepseek-ai` 内 0.1.5-rc.2 的 package.json description、各 `lib/client.js` 公开中文/英文 UI 字面量、以及全部 `lib/typert.remote-client.js` RPC 描述符汇总。官方在线文档（deepseek-harness.github.io / GitHub README）本次抓取失败（404），故无独立「文档证实」条目；架构级结论引用 `check-dsh-contract.ps1` 注释（运行时实测）。*
