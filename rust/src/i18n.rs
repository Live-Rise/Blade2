use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

pub const EN: &[(&str, &str)] = &[
    ("新会话", "New conversation"),
    ("新建会话", "New session"),
    ("工作区", "Workspaces"),
    ("添加工作区", "Add workspace"),
    ("视图选项", "View options"),
    ("未分组", "Ungrouped"),
    ("未分组会话", "Ungrouped conversations"),
    ("收起", "Collapse"),
    ("展开其余 {0} 个会话", "Expand the other {0} sessions"),
    ("设置", "Settings"),
    ("开始一段新的会话", "Start a new conversation"),
    ("刷新", "Refresh"),
    ("等待审批", "Awaiting approval"),
    ("拒绝", "Decline"),
    ("允许一次", "Allow once"),
    ("打开工作区路径", "Open workspace path"),
    ("分组方式", "Group by"),
    ("按工作区", "By workspace"),
    ("单列表", "Single list"),
    ("排序方式", "Sort by"),
    ("手动排序", "Manual order"),
    ("最近更新", "Recently updated"),
    ("刚刚", "just now"),
    ("{0}分钟前", "{0} min ago"),
    ("{0}小时前", "{0} h ago"),
    ("{0}天前", "{0} d ago"),
    ("{0}个月前", "{0} mo ago"),
    ("{0}年前", "{0} yr ago"),
    ("内核未连接", "Kernel not connected"),
    (
        "会话创建响应缺少 sessionId。",
        "Session creation response is missing a sessionId.",
    ),
    // —— 对话正文：操作行 / 轮尾 / 工具行 / 交付物（主线 MainWindow.xaml.cs:924-943、1355-1357）——
    ("在新对话中分支", "Branch into a new conversation"),
    (
        "仅可从已完成轮次的最后一条消息分支",
        "Available only on the last message of a completed turn",
    ),
    ("复制消息", "Copy message"),
    ("已复制", "Copied"),
    ("思考", "Think"),
    ("工具调用", "Tool call"),
    ("读取", "Read"),
    ("读取图片", "Read image"),
    ("网页获取", "Fetch"),
    ("写入", "Write"),
    ("代码", "Code"),
    ("用时 {0}", "Ran for {0}"),
    ("用量 {0} tok", "Usage {0} tok"),
    ("{0}秒", "{0}s"),
    ("{0}分{1}秒", "{0}m{1}s"),
    ("{0}小时{1}分", "{0}h{1}m"),
    ("{0}月{1}日 {2}", "{0}/{1} {2}"),
    ("{0}年{1}月{2}日 {3}", "{0}-{1}-{2} {3}"),
    ("本轮文件改动", "Files changed"),
    ("打开 {0}", "Open {0}"),
    ("打开失败：{0}", "Open failed: {0}"),
    ("少女祈祷中…", "Praying…"),
    ("交付物", "Deliverable"),
    ("打开", "Open"),
    ("显示", "Reveal"),
    ("消息输入框", "Message input"),
    ("添加附件或操作", "Add attachment or action"),
    ("搜索会话…", "Search conversations…"),
    ("拖放以安装", "Drop to install"),
    ("指令内容", "Instructions"),
    ("记忆文件内容", "Memory file contents"),
    ("返回", "Back"),
    ("返回聊天", "Back to chat"),
    ("系统提示词", "System prompt"),
    ("系统提示词更新", "System prompt update"),
    ("正在启动 dsh 内核…", "Starting the dsh kernel…"),
    // —— 消息反馈（主线 MainWindow.xaml.cs:974-976、1209-1222）——
    ("有帮助（再点一次撤销）", "Helpful (click again to revoke)"),
    (
        "没帮助（再点一次撤销）",
        "Not helpful (click again to revoke)",
    ),
    ("添加说明", "Add note"),
    ("撤销", "Revoke"),
    ("说明：{0}", "Note: {0}"),
    ("编辑说明", "Edit note"),
    (
        "该消息不在本会话日志里（子代理或已裁剪），无法反馈",
        "This message is not in the session log (sub-agent or trimmed); feedback unavailable",
    ),
    (
        "会话不存在或已归档",
        "Session does not exist or is archived",
    ),
    ("说明不能是空白", "The note cannot be blank"),
    (
        "说明超长（上限 {0} 字节）",
        "Note too long (limit {0} bytes)",
    ),
    ("说明超长", "Note too long"),
    (
        "反馈已被其他地方改动，请重试",
        "Feedback changed elsewhere; please retry",
    ),
    ("操作失败", "Operation failed"),
    ("失败（{0}）", "Failed ({0})"),
    ("反馈说明", "Feedback notes"),
    (
        "这条回复哪里好 / 哪里不好（可选，纯文本）",
        "What was good / bad about this reply (optional, plain text)",
    ),
    ("保存", "Save"),
    ("取消", "Cancel"),
    // 行内说明编辑器的字节计数（上限 = 内核 maxNoteBytes）
    ("{0}/{1} 字节", "{0}/{1} bytes"),
    // —— 交付物打开失败与零星界面文案（英文取自主干 EN 表）——
    (
        "无法打开：宿主桌面不可用（内核 workspaceDesktop().available=false）。",
        "Cannot open: host desktop unavailable (kernel workspaceDesktop().available=false).",
    ),
    (
        "无法打开：内核在会话日志里找不到该交付物（文件可能已被移动或删除）。",
        "Cannot open: the kernel cannot find this deliverable in the session log (the file may have been moved or deleted).",
    ),
    (
        "无法打开：该文件没有经过校验的宿主路径（可能在工作区沙箱之外）。",
        "Cannot open: the file has no validated host path (it may be outside the workspace sandbox).",
    ),
    (
        "无法打开交付物（HTTP {0}）：{1}",
        "Cannot open deliverable (HTTP {0}): {1}",
    ),
    ("无法打开交付物：{0}", "Cannot open deliverable: {0}"),
    (
        "描述你想要构建的内容, / 调用指令, @ 文件或对话",
        "Describe what you want to build, / for commands, @ for files or conversations",
    ),
    ("发送消息", "Send message"),
    ("停止运行", "Stop run"),
    ("会话操作：{0}", "Session actions: {0}"),
    // —— 分叉自造文案（主干没有这个键）——
    ("暂无会话", "No sessions yet"),
    ("返回上一级", "Back"),
    ("附件尚未移植", "Attachments not ported yet"),
    ("添加工作区尚未移植", "Add workspace not ported yet"),
    ("视图选项尚未移植", "View options not ported yet"),
    (
        "内核未连接，消息没发出去",
        "Kernel not connected, message not sent",
    ),
    ("连接内核", "Connect kernel"),
    ("断开", "Disconnect"),
    (
        "撤回编辑（停止运行并把这句放回输入框）",
        "Withdraw edit (stop the run and put this message back in the composer)",
    ),
    ("取消失败：{0}", "Cancel failed: {0}"),
    // —— 主线 ZH 键表批量对齐（Assets/i18n/*.json 28 份，每份 722 键；键序沿用主线表序）——
    //    以下条目是主线上有、分叉此前缺的键，EN 逐字取自主线 ShellEnglish
    //    （MainWindow.xaml.cs:577-1714，同一份 EN 母表）。只增不改：上方既有 104 条原样保留。
    (
        "Enter 发送，Shift+Enter 换行，输入 / 唤起命令，@ 引用文件或会话",
        "Enter to send, Shift+Enter for a new line, / for commands, @ for files or conversations",
    ),
    ("空态：开始一段新的会话", "Start a new conversation"),
    ("通用设置", "General"),
    ("模型", "Models"),
    ("插件", "Plugins"),
    ("Agent 预设", "Agent presets"),
    ("使用统计", "Usage"),
    ("设置内容列", "Settings content"),
    (
        "新会话的默认权限、外观与对话偏好。",
        "Default permissions, appearance and conversation preferences for new sessions.",
    ),
    (
        "填入各提供方的 API 密钥即可使用其模型；提供方与凭据位置来自内核目录。",
        "Enter provider API keys to use their models. Providers and credential locations come from the kernel catalog.",
    ),
    ("配置和查看本部署已安装的插件。", "Configure and inspect plugins installed in this deployment."),
    (
        "内核 Loader 实际加载的插件条目（pluginInventory/list 只读快照），以及各 Agent 预设的组装行。",
        "Plugins loaded by the kernel (a read-only pluginInventory/list snapshot), and the composition of each agent preset.",
    ),
    (
        "预设即一个会话的 Agent 所运行的插件组装——它的工具、提示词与能力。",
        "A preset defines the plugins an agent runs: its tools, prompts and capabilities.",
    ),
    ("权限", "Permissions"),
    ("选择新会话的默认访问模式", "Choose the default access mode for new conversations"),
    ("默认权限", "Default permissions"),
    ("默认权限模式", "Default permission mode"),
    ("仅可查看 / 工作区内修改 / 完全权限", "Read only / Workspace write / Full access"),
    ("仅可查看", "Read only"),
    ("工作区内修改", "Workspace write"),
    ("完全权限", "Full access"),
    ("外观", "Appearance"),
    ("主题与会话正文字号", "Theme and conversation font size"),
    ("主题", "Theme"),
    ("浅色 / 深色 / 跟随系统", "Light / Dark / System"),
    ("浅色", "Light"),
    ("深色", "Dark"),
    ("跟随系统", "System"),
    ("字号大小", "Font size"),
    ("仅影响会话内容的字号", "Applies only to conversation content"),
    ("对话", "Conversation"),
    ("已完成轮次的展示方式与繁忙时的发送行为", "Completed turn display and send behavior while busy"),
    ("对话显示", "Transcript view"),
    ("标准显示过程内容；紧凑只保留结果", "Normal shows all steps; compact folds completed turns to their answers"),
    ("标准", "Normal"),
    ("紧凑", "Compact"),
    ("繁忙时的发送行为", "Send behavior while busy"),
    (
        "智能体运行时 Enter 键和发送按钮的行为；Ctrl+Enter 使用另一行为",
        "Enter and Send while the agent runs; Ctrl+Enter uses the alternate behavior",
    ),
    ("排队发送", "Queue message"),
    ("插话发送", "Steer now"),
    ("语言", "Language"),
    ("界面显示语言", "Interface language"),
    ("背景皮肤", "Background image"),
    ("导入图片", "Import image"),
    ("清除皮肤", "Clear background"),
    ("背景图", "Background image"),
    ("插件配置", "Plugin settings"),
    ("插件列表", "Plugin inventory"),
    ("默认模型", "Default model"),
    ("服务商", "Provider"),
    ("API 密钥", "API key"),
    ("发现模型", "Discover models"),
    ("计划评审", "Plan review"),
    ("内核提问", "Question from the agent"),
    ("补充说明", "Additional details"),
    ("补充说明（可选，随答案回传为 custom）", "Additional details (optional, sent with your answer)"),
    ("跳过（不回答）", "Skip (do not answer)"),
    ("提交", "Submit"),
    ("提交答案", "Submit answer"),
    ("模型与推理等级", "Model and reasoning effort"),
    ("文件", "Files"),
    ("工作区文件", "Workspace files"),
    ("打开文件面板", "Open files panel"),
    ("累计 Token 数", "Total tokens"),
    ("峰值 Token 数", "Peak tokens"),
    ("最长聊天时长", "Longest conversation"),
    ("当前连续天数", "Current streak"),
    ("返回聊天页", "Back to chat"),
    ("搜索会话", "Search conversations"),
    ("按标题或内容检索会话", "Search conversations by title or content"),
    ("搜索", "Search"),
    ("分组方式与排序方式", "Grouping and sorting"),
    ("会话操作", "Session actions"),
    ("重命名", "Rename"),
    ("分叉会话", "Fork conversation"),
    ("归档会话", "Archive conversation"),
    ("移动到工作区", "Move to workspace"),
    ("取消运行", "Cancel run"),
    ("重命名工作区", "Rename workspace"),
    ("上移", "Move up"),
    ("下移", "Move down"),
    ("删除工作区", "Delete workspace"),
    ("新标题", "New title"),
    ("新名称", "New name"),
    ("确定", "OK"),
    ("重命名会话", "Rename conversation"),
    ("新建工作区", "New workspace"),
    ("文件夹路径", "Folder path"),
    ("浏览…（内核选择器）", "Browse… (kernel picker)"),
    ("创建", "Create"),
    ("使用此目录", "Use this folder"),
    ("新建文件夹…", "New folder…"),
    ("新建文件夹", "New folder"),
    ("文件夹名", "Folder name"),
    ("目录项", "Directory entries"),
    ("目录项超过内核上限，仅显示前若干项。", "Too many entries for the kernel limit; showing only the first ones."),
    ("添加图片或文件", "Add image or file"),
    ("在应用中打开", "Open in app"),
    ("会话反馈", "Session feedback"),
    ("选择工作区", "Select workspace"),
    ("新会话将在此工作区中创建", "New conversations will be created in this workspace"),
    ("Agent 模式", "Agent mode"),
    ("即将开始的这个会话所用的 Agent 预设", "The agent preset for the conversation you are about to start"),
    ("访问模式", "Access mode"),
    ("计划模式", "Plan mode"),
    ("退出", "Exit"),
    ("退出计划模式", "Exit plan mode"),
    ("命令（↑↓ 选择，Enter 执行，Esc 关闭）", "Commands (↑↓ select, Enter run, Esc close)"),
    ("引用（↑↓ 选择，Enter 插入，Esc 关闭）", "References (↑↓ select, Enter insert, Esc close)"),
    ("使用统计概览", "Usage overview"),
    ("最长连续天数", "Longest streak"),
    ("Token 活动", "Token activity"),
    ("Token 活动统计口径", "Token activity metric"),
    ("每日", "Daily"),
    ("每周", "Weekly"),
    ("累计", "Cumulative"),
    ("少", "Less"),
    ("多", "More"),
    ("时间范围", "Time range"),
    ("统计时间范围", "Statistics time range"),
    ("近 7 天", "Last 7 days"),
    ("近 30 天", "Last 30 days"),
    ("每日 Token 趋势图", "Daily token trend"),
    ("模型用量", "Model usage"),
    ("设置文件", "Settings file"),
    ("设置文档", "Settings document"),
    ("打开设置文档", "Open settings document"),
    (
        "在内核主机上用默认文本编辑器打开 settings 文档（settings/openSettingsDocument）",
        "Opens the settings document with the default text editor on the kernel host (settings/openSettingsDocument)",
    ),
    (
        "把内核侧设置文档落到本地并用默认编辑器打开。",
        "Materializes the kernel-side settings document locally and opens it with the default editor.",
    ),
    ("恢复默认", "Restore defaults"),
    ("恢复本页默认", "Restore this page to defaults"),
    (
        "清空本页命名空间的用户设置段（settings/replace），回到内核默认值",
        "Clears this page's user settings sections (settings/replace) and falls back to kernel defaults",
    ),
    ("导入背景皮肤", "Import background image"),
    ("清除背景皮肤", "Clear background image"),
    ("重试保存", "Retry save"),
    ("设置尚未保存", "Settings not saved"),
    (
        "恢复默认未完成，未成功的草稿已保留。请检查连接后重试恢复默认。",
        "Restore-defaults failed; drafts that did not commit are kept. Check the connection and retry.",
    ),
    (
        "凭据引用由内核目录给出（settingsNs/settingsPath 下的 apiKeyEnv）。",
        "Credential references come from the kernel catalog (apiKeyEnv under settingsNs/settingsPath).",
    ),
    ("内核未声明任何可配置的提供方。", "The kernel declares no configurable providers."),
    ("输入 API 密钥", "Enter API key"),
    ("已保存，输入新值可替换", "Saved; type a new value to replace"),
    ("已保存", "Saved"),
    ("输入密钥后即可使用其模型", "Enter the key to use this provider's models"),
    ("内核未给出配置位置", "The kernel gave no configuration location"),
    ("尚未配置密钥", "No key configured yet"),
    ("清除", "Clear"),
    ("清除内核凭据库中的该密钥", "Clears this key from the kernel credential store"),
    ("该来源不可写（环境变量等）", "This source is not writable (environment variable, etc.)"),
    (
        "调用提供方端点列出可用模型（llm/discoverModels）",
        "Calls the provider endpoint to list available models (llm/discoverModels)",
    ),
    ("其他可配置提供方", "Other configurable providers"),
    ("未启用", "Not enabled"),
    ("清除 API 密钥", "Clear API key"),
    (
        "密钥清除失败，原有草稿已保留。请检查连接后重试清除。",
        "Key clearing failed; the existing draft is kept. Check the connection and retry clearing.",
    ),
    (
        "内核已注册的提供方路由（llm/listProviders）",
        "Provider routes registered by the kernel (llm/listProviders)",
    ),
    (
        "可从提供方端点发现（见上方 API 密钥卡的「发现模型」）",
        "Discoverable from provider endpoints (see Discover models in the API key card above)",
    ),
    ("推理等级", "Reasoning effort"),
    ("off / low / high / max；留空跟随模型默认", "off / low / high / max; empty follows the model default"),
    ("发现的模型", "Discovered models"),
    ("用作默认模型", "Use as default model"),
    ("关闭", "Close"),
    ("插件视图", "Plugins view"),
    ("终端", "Terminal"),
    ("限制 agent 运行的每一条命令。", "Governs every command the agent runs."),
    ("命令超时（毫秒）", "Command timeout (ms)"),
    ("单条命令允许运行多久，超时即终止。", "How long a single command may run before being terminated."),
    ("单流输出上限（字节）", "Per-stream output limit (bytes)"),
    ("超出部分会转存到临时文件，而不是被丢弃。", "Overflow is spilled to a temp file instead of being dropped."),
    ("Agent 循环", "Agent loop"),
    ("Agent 如何派发工具调用。", "How the agent dispatches tool calls."),
    ("并行工具调用数", "Parallel tool calls"),
    ("同一步内最多同时运行多少个可并行的调用。", "How many parallelizable calls run at once within a step."),
    ("控制 Agent 为 Subagent 选择模型的权限。", "Controls whether the agent may pick models for subagents."),
    ("允许 Agent 为 Subagent 选择模型", "Allow the agent to choose models for subagents"),
    (
        "内核要求开启时至少有一个允许模型；当前读不到默认模型，开启可能被内核拒绝并在此提示。",
        "The kernel requires at least one allowed model when this is on; no default model is readable now, so enabling may be rejected and reported here.",
    ),
    ("网页搜索", "Web search"),
    ("DeepSeek 搜索提供方。", "The DeepSeek search provider."),
    ("API Key（未配置）", "API key (not configured)"),
    ("配置之前搜索不可用", "Search is unavailable until configured"),
    ("插件搜索 API 密钥", "Plugin search API key"),
    ("接口地址", "Endpoint URL"),
    ("留空则使用提供方默认地址。", "Leave empty to use the provider default."),
    ("单次请求最多搜索次数", "Max searches per request"),
    ("一次请求在必须作答前最多可以搜索多少次。", "How many searches one request may run before it must answer."),
    ("概览", "Overview"),
    ("计数直接来自本次快照，不做缓存。", "Counts come straight from this snapshot; nothing is cached."),
    ("按模块名或条目 id 过滤", "Filter by module name or entry id"),
    ("按模块名或条目 id 过滤…", "Filter by module name or entry id…"),
    ("Agent 预设组装行", "Agent preset assembly rows"),
    (
        "预设的 rows 才是模型可见插件的运行位置（Loader 条目之外的第二处清单，与内核清单一致）。",
        "A preset's rows are where model-visible plugins actually run (a second inventory beyond loader entries, matching the kernel list).",
    ),
    ("（该预设没有组装行）", "(this preset has no assembly rows)"),
    ("预设目录", "Preset directory"),
    ("新增自定义预设", "New custom preset"),
    (
        "用「复制为…」从任一预设派生一份用户预设，再用「打开目录」编辑其定义。",
        "Use Copy as… to derive a user preset, then Open folder to edit its definition.",
    ),
    (
        "本部署声明不可创作预设（agentPresets/list 的 authorable=false）。",
        "This deployment reports presets as not authorable (agentPresets/list authorable=false).",
    ),
    (
        "本部署无法原生打开预设目录（settings/canOpenAgentPresetDirectory 返回 false）。",
        "This deployment cannot open preset directories natively (settings/canOpenAgentPresetDirectory returned false).",
    ),
    ("可用", "Available"),
    ("不可用", "Unavailable"),
    ("复制为…", "Copy as…"),
    ("当前会话使用", "Use for this session"),
    ("先在左侧选择一个会话", "Select a conversation in the left sidebar first"),
    (
        "agentPresets/select 应用到当前会话；内核只允许在会话的 agent 启动前切换（已开始会回 agent-preset/locked）",
        "Applies to the current session via agentPresets/select; the kernel allows switching only before the session's agent starts (started sessions return agent-preset/locked)",
    ),
    ("设为默认", "Set as default"),
    ("打开目录", "Open folder"),
    ("在内核主机上打开该预设所在目录", "Opens this preset's folder on the kernel host"),
    (
        "本部署无法原生打开目录（settings/canOpenAgentPresetDirectory=false）",
        "This deployment cannot open folders natively (settings/canOpenAgentPresetDirectory=false)",
    ),
    ("删除", "Delete"),
    ("新预设 id", "New preset id"),
    ("小写字母/数字/短横线，如 my-agent", "lowercase letters/digits/hyphens, e.g. my-agent"),
    ("显示名", "Display name"),
    ("复制", "Duplicate"),
    ("删除预设", "Delete preset"),
    ("内置", "Built-in"),
    ("自定义", "Custom"),
    ("内核已收到、等当前轮结束后发送", "Received by the kernel; sent after the current turn"),
    ("均已完成", "all done"),
    ("编辑", "Edit"),
    ("插话", "Steer"),
    ("编辑排队消息", "Edit queued message"),
    ("分类（可留空）", "Category (optional)"),
    ("说明（可留空）", "Details (optional)"),
    ("这条会话的体验如何？", "How was this session?"),
    ("反馈分类", "Feedback category"),
    ("记录", "Record"),
    ("任务结果", "Task result"),
    ("指令遵循", "Instruction following"),
    ("产品交互", "Product interaction"),
    ("服务稳定性", "Service stability"),
    ("资源消耗", "Resource cost"),
    ("安全隐私与权限", "Security, privacy and permissions"),
    ("其他", "Other"),
    ("自动保存失败，未保存的草稿已保留。请重试。", "Auto-save failed; unsaved drafts are kept. Please retry."),
    (
        "内核未连接，未保存的草稿已保留。连接后请重试。",
        "The kernel is not connected; unsaved drafts are kept. Retry after reconnection.",
    ),
    ("保存未完成，未保存的草稿已保留。请重试。", "Save did not complete; unsaved drafts are kept. Please retry."),
    ("文档目录（默认）", "Documents folder (default)"),
    (
        "无法在应用中打开：当前会话没有已知的工作目录（cwd）。",
        "Cannot open in app: the current session has no known working directory (cwd).",
    ),
    (
        "请先选择一个会话：会话反馈作用在具体会话上。",
        "Select a conversation first: session feedback applies to a specific conversation.",
    ),
    (
        "会话反馈已记录（内核会话日志追加 feedback/record 事件）。",
        "Session feedback recorded (a feedback/record event was appended to the kernel session log).",
    ),
    (
        "请先选择一个会话：命令在会话（agent）作用域内执行。",
        "Select a conversation first: commands run within the conversation (agent) scope.",
    ),
    ("内核启动失败", "Kernel failed to start"),
    (
        "未找到内置内核；请安装 npm 版 dsh 或重新安装 Blade²",
        "Bundled kernel not found; install the npm version of dsh or reinstall Blade²",
    ),
    ("{0} 轮对话", "{0} conversation turns"),
    ("重命名失败：{0}", "Rename failed: {0}"),
    (
        "将把“{0}”从工作区列表中移除。文件夹与会话记录会保留，其会话将显示在“未分组”下。",
        "\"{0}\" will be removed from the workspace list. The folder and session records are kept; its sessions will appear under \"Ungrouped\".",
    ),
    ("删除失败：{0}", "Delete failed: {0}"),
    ("内核目录选择器不可用：{0}", "Kernel directory picker unavailable: {0}"),
    ("创建失败：{0}", "Create failed: {0}"),
    (
        "内核目录浏览不可用：本部署使用原生目录选择器，请用「浏览…（内核选择器）」按钮选取文件夹，或直接输入路径。",
        "Kernel directory browsing is unavailable: this deployment uses the native picker. Use the \"Browse… (kernel picker)\" button to choose a folder, or type the path directly.",
    ),
    ("内核目录浏览不可用：{0}", "Kernel directory browsing unavailable: {0}"),
    ("读取目录失败：{0}", "Failed to read directory: {0}"),
    ("新建文件夹失败：{0}", "Failed to create folder: {0}"),
    ("该工作区没有路径记录。", "This workspace has no recorded path."),
    (
        "本部署无法在内核主机打开路径（session/canOpenWorkspacePath=false）。",
        "This deployment cannot open paths on the kernel host (session/canOpenWorkspacePath=false).",
    ),
    ("打开路径失败：{0}", "Failed to open path: {0}"),
    ("调整顺序失败：{0}", "Failed to reorder: {0}"),
    ("归档失败：{0}", "Archive failed: {0}"),
    ("移动失败：{0}", "Move failed: {0}"),
    ("分叉失败：{0}", "Fork failed: {0}"),
    ("新建会话失败：{0}", "Failed to create session: {0}"),
    ("会话历史分页未前进。", "Session history pagination did not advance."),
    ("会话同步失败：{0}", "Session sync failed: {0}"),
    // 台账 #137 L2：`历史回读失败：{0}` 是分叉自造句（主干 `LoadSessionHistoryAsync` 那一路
    // 的 catch 是 `catch (Exception) { }` 静默，没有这条文案），故它不在主干 `ShellEnglish`
    // 里、`en_table_matches_every_mainline_shell_english_key` 那道总闸盖不到它。补键前先
    // `grep -cF` 确认过全表零命中 ⇒ 不是重复登记（本表 `Catalog::l` 走 `Iterator::find`
    // = first-wins，重复键会静默盖住后写的那条）。措辞与同族两枚邻居对齐：
    // `会话同步失败：{0}` = `Session sync failed: {0}`、`会话历史分页未前进。` =
    // `Session history pagination did not advance.`
    ("历史回读失败：{0}", "Session history read failed: {0}"),
    ("连接恢复后订阅失败：{0}", "Subscription failed after reconnect: {0}"),
    ("图片", "Image"),
    ("内核执行失败", "Kernel execution failed"),
    ("⚙ {0}（命令执行）", "⚙ {0} (command execution)"),
    ("粘贴图片 {0:HHmmss}.png", "Pasted image {0:HHmmss}.png"),
    ("发送失败：{0}", "Send failed: {0}"),
    ("(工具)", "(tool)"),
    ("工具 {0} 请求越权执行", "Tool {0} requests elevated execution"),
    ("审批回传失败，请重试：{0}", "Approval reply failed, please retry: {0}"),
    ("已跳过该提问（内核侧按未作答继续）。", "Question skipped (the kernel continues as unanswered)."),
    ("已回传提问答复。", "Question answer submitted."),
    ("提问回传失败，请重试：{0}", "Question reply failed, please retry: {0}"),
    ("应用清单不可用（HTTP {0}）", "App manifest unavailable (HTTP {0})"),
    ("内核未探测到可用应用", "The kernel detected no available apps"),
    ("应用清单读取失败：{0}", "Failed to read app manifest: {0}"),
    ("文件资源管理器", "File Explorer"),
    ("Windows 终端", "Windows Terminal"),
    ("命令提示符", "Command Prompt"),
    ("在应用中打开失败（HTTP {0}）：{1}", "Failed to open in app (HTTP {0}): {1}"),
    ("在应用中打开失败：{0}", "Failed to open in app: {0}"),
    ("导入皮肤失败：{0}", "Failed to import skin: {0}"),
    ("应用皮肤失败：{0}", "Failed to apply skin: {0}"),
    ("读取设置失败：内核未返回设置描述。", "Failed to read settings: the kernel returned no settings description."),
    ("该分区加载失败：{0}", "This section failed to load: {0}"),
    ("清空本页用户设置：{0}。", "Clear user settings on this page: {0}."),
    ("打开设置文档失败：{0}", "Failed to open settings document: {0}"),
    ("恢复「{0}」默认", "Restore \"{0}\" defaults"),
    ("将清空这些命名空间的用户设置：{0}。", "This clears the user settings of these namespaces: {0}."),
    (
        "内核默认值会立即生效，此操作不可撤销。",
        "Kernel defaults take effect immediately; this action cannot be undone.",
    ),
    ("恢复", "Restore"),
    ("读取提供方目录失败：{0}", "Failed to read provider catalog: {0}"),
    ("读取可配置提供方失败：{0}", "Failed to read configurable providers: {0}"),
    ("{0} 未声明 apiKeyEnv（无需密钥）", "{0} declares no apiKeyEnv (no key required)"),
    ("{0} · {1}（密钥缺失）", "{0} · {1} (key missing)"),
    (" 等 {0} 个", " and {0} more"),
    (
        "{0}{1}：内核目录已声明但尚未配置，在设置里补齐对应条目后才会出现密钥行。",
        "{0}{1}: declared in the kernel catalog but not configured. The key row appears after you complete the entries in Settings.",
    ),
    ("{0}（当前值）", "{0} (current value)"),
    (
        "确定清除凭据 {0} 吗？清除后使用该提供方模型的会话会立即失败，直到重新填入。",
        "Clear credential {0}? Sessions using this provider's models will fail immediately until the key is filled in again.",
    ),
    ("（该命名空间未注册模型发现能力）", "(this namespace registers no model discovery)"),
    ("发现模型失败{0}：{1}", "Model discovery failed{0}: {1}"),
    ("上下文 {0:N0}", "Context {0:N0}"),
    ("输出上限 {0:N0}", "Max output {0:N0}"),
    ("提供方 {0} 未返回任何模型。", "Provider {0} returned no models."),
    (
        "{0} 声明 {1} 个模型（选择后填入默认模型）：",
        "{0} declares {1} models (select one to fill the default model):",
    ),
    (
        "内核要求开启时至少有一个允许模型，首次开启会自动写入当前默认模型（{0} / {1}）。",
        "The kernel requires at least one allowed model when enabled; the first enable writes the current default model ({0} / {1}).",
    ),
    ("提供方默认", "Provider default"),
    ("插件清单不可用：{0}", "Plugin manifest unavailable: {0}"),
    (
        "共 {0} 个 Loader 条目（active {1}，failed {2}，未启用 {3}）。",
        "{0} loader entries total (active {1}, failed {2}, disabled {3}).",
    ),
    ("（无 fiber）", "(no fiber)"),
    ("Loader 条目 {0} 个", "{0} loader entries"),
    ("匹配 “{0}”：{1} / {2} 个", "Matching \"{0}\": {1} / {2}"),
    ("已启用", "Enabled"),
    ("已禁用", "Disabled"),
    ("{0}（{1} 行）", "{0} ({1} rows)"),
    ("无法加载 Agent 预设：{0}", "Failed to load agent presets: {0}"),
    ("{0}（加载失败）", "{0} (load failed)"),
    ("设为默认失败：{0}", "Failed to set default: {0}"),
    ("读取预设失败：{0}", "Failed to read preset: {0}"),
    ("{0} 副本", "{0} copy"),
    ("复制预设（来源：{0}）", "Copy preset (from: {0})"),
    ("新预设 id 不能为空。", "The new preset id cannot be empty."),
    ("复制失败：{0}", "Copy failed: {0}"),
    (
        "确定删除自定义预设「{0}」（id: {1}）吗？该预设的目录会被移除，此操作不可撤销。",
        "Delete custom preset \"{0}\" (id: {1})? Its directory will be removed; this action cannot be undone.",
    ),
    (
        "请先在左侧选择一个会话：预设切换作用在会话的 agent 上。",
        "Select a session in the left pane first: preset switching applies to the session's agent.",
    ),
    ("会话已切换到预设「{0}」（内核回执：{1}）。", "Session switched to preset \"{0}\" (kernel receipt: {1})."),
    ("切换预设失败：{0}", "Failed to switch preset: {0}"),
    ("内核无法直接打开目录，路径：{0}", "The kernel cannot open the directory directly; path: {0}"),
    ("打开预设目录失败：{0}", "Failed to open preset directory: {0}"),
    ("{0}：有空值或无效值，请修正后重试", "{0}: contains empty or invalid values; fix them and retry"),
    (
        "{0}：写入失败，请检查连接和字段值后重试",
        "{0}: write failed; check the connection and field values, then retry",
    ),
    (
        "{0}：密钥保存失败，请检查连接和凭据来源是否可写后重试",
        "{0}: key save failed; check the connection and whether the credential source is writable, then retry",
    ),
    ("未保存的草稿已保留（仅当前窗口内）。{0}", "Unsaved drafts are kept (current window only). {0}"),
    ("读取模型目录失败：{0}", "Failed to read model catalog: {0}"),
    ("选择模型", "Select model"),
    ("低", "Low"),
    ("高", "High"),
    ("最大", "Max"),
    ("内核全文检索无匹配：{0}", "Kernel full-text search returned no match: {0}"),
    (
        "内核未开启会话全文检索，已按标题匹配（{0} 条）。",
        "Kernel session full-text search is off; matched by title ({0} results).",
    ),
    (
        "内核未开启会话全文检索（session-query 索引 openAt=never），已回退为标题匹配。",
        "Kernel session full-text search is off (session-query index openAt=never); fell back to title matching.",
    ),
    (
        "如需全文检索，请在部署的 cordis.patch.yml 里把 session-query-sqlite 的 openAt 改为 first-search。",
        "To enable full-text search, set session-query-sqlite's openAt to first-search in the deployment's cordis.patch.yml.",
    ),
    (
        "检索失败（{0}），已回退为标题匹配（{1} 条）。",
        "Search failed ({0}); fell back to title matching ({1} results).",
    ),
    ("本地标题匹配", "Local title match"),
    ("没有匹配「{0}」的会话。", "No sessions match \"{0}\"."),
    (
        "内核未返回会话清单（session/list），使用统计不可用。",
        "The kernel returned no session list (session/list); usage statistics unavailable.",
    ),
    (
        "；{0} 个超长会话触到分页上限，其数据为部分计入",
        "; {0} very long sessions hit the pagination cap and are partially counted",
    ),
    ("读取使用统计失败：{0}", "Failed to read usage statistics: {0}"),
    ("{0:N0} tokens（{1} 条用量记录）", "{0:N0} tokens ({1} usage records)"),
    ("内核未返回任何 token 用量记录", "The kernel returned no token usage records"),
    ("单日峰值：{0}，{1:N0} tokens", "Daily peak: {0}, {1:N0} tokens"),
    (
        "单会话对话累计时长最大值：{0}（按 turn/start→turn/end 求和）",
        "Longest per-session conversation time: {0} (summed over turn/start→turn/end)",
    ),
    (
        "内核 journal 未包含任何完整 turn（turn/start → turn/end），无法计算",
        "The kernel journal contains no complete turns (turn/start → turn/end); cannot compute",
    ),
    ("{0} 天", "{0} days"),
    (
        "「累计/峰值/连续天数」按全部历史，「时间范围」只作用于趋势图与模型用量；",
        "Totals/peaks/streaks cover all history; the time range only affects the trend chart and model usage;",
    ),
    (
        "费用/金额内核未提供对应字段，故不展示。",
        "Cost/amount fields are not provided by the kernel, so they are not shown.",
    ),
    ("{0}月", "{0}"),
    ("{0}月{1}日", "{0}/{1}"),
    (
        "每日 Token 趋势图：近 {0} 天共 {1} tokens，{2} 个模型序列",
        "Daily token trend: {1} tokens over the last {0} days, {2} model series",
    ),
    ("{0:0.#} 亿", "{0:0.#} 亿"),
    ("{0:0.#} 万", "{0:0.#} 万"),
    ("{0} 小时 {1} 分", "{0}h {1}m"),
    ("{0} 分 {1} 秒", "{0}m {1}s"),
    ("{0} 秒", "{0}s"),
    ("[图片]", "[Image]"),
    ("[文件 {0}]", "[File {0}]"),
    ("排队中 · {0} 条", "Queued · {0} items"),
    ("作业 · {0} 个", "Jobs · {0}"),
    ("{0} 个运行中", "{0} running"),
    ("计划模式（切换中…）", "Plan mode (switching…)"),
    ("计划模式已开启", "Plan mode is on"),
    ("访问模式，当前：{0}", "Access mode, currently: {0}"),
    ("只读：可浏览工作区，不能写文件或执行修改", "Read-only: browse the workspace, no file writes or modifications"),
    (
        "可写工作区与允许的临时目录；更广范围的重试需审批",
        "Workspace and allowed temp dirs writable; broader retries need approval",
    ),
    ("完全文件访问，不再弹出审批确认", "Full file access; no approval prompts"),
    ("自动审批", "Auto approval"),
    (
        "Flash 预判写入/命令是否不可回补：安全自动批准，有风险转人工审批",
        "Flash pre-judges whether writes/commands are irreversible: safe ones auto-approve, risky ones go to human approval",
    ),
    ("需要确认", "Requires confirmation"),
    ("直接执行", "Execute directly"),
    ("审批策略已从「{0}」切换为「{1}」（由你更改）", "Approval policy switched from \"{0}\" to \"{1}\" (by you)"),
    ("设置默认权限模式失败：{0}", "Failed to set default permission mode: {0}"),
    (
        "记录失败：该会话在内核里已不存在（session-not-found）。",
        "Record failed: the session no longer exists in the kernel (session-not-found).",
    ),
    ("记录失败：内核返回 {0}。", "Record failed: the kernel returned {0}."),
    ("记录失败：[{0}] {1}", "Record failed: [{0}] {1}"),
    ("会话反馈失败：{0}", "Session feedback failed: {0}"),
    ("(空)", "(empty)"),
    ("队列操作失败：{0}", "Queue operation failed: {0}"),
    ("目录", "Directory"),
    ("文件引用不可用（{0}）", "File reference unavailable ({0})"),
    ("会话 · 同工作区", "Session · same workspace"),
    ("会话", "Session"),
    ("会话引用不可用（{0}）", "Session reference unavailable ({0})"),
    ("引用不可用：{0}", "Reference unavailable: {0}"),
    (
        "引用 {0} 条（@ 后输入可过滤；↑↓ 选择，Enter 插入，Esc 关闭）",
        "{0} references (@ to filter; ↑↓ select, Enter insert, Esc close)",
    ),
    (
        "“@{0}” 匹配 {1} 条（↑↓ 选择，Enter 插入，Esc 关闭）",
        "\"@{0}\" matched {1} (↑↓ select, Enter insert, Esc close)",
    ),
    ("命令目录不可用：{0}", "Command catalog unavailable: {0}"),
    (
        "“/{0}” 匹配 {1} 条（↑↓ 选择，Enter 执行，Esc 关闭）",
        "\"/{0}\" matched {1} (↑↓ select, Enter run, Esc close)",
    ),
    ("附件上传失败，命令未提交；请重试。", "Attachment upload failed; the command was not submitted. Please retry."),
    ("命令失败：{0}", "Command failed: {0}"),
    ("内核拒绝", "rejected by the kernel"),
    ("未知或格式不正确的命令：{0}", "Unknown or malformed command: {0}"),
    ("{0} 已提交（内核未返回即时应答）", "{0} submitted (no immediate kernel response)"),
    ("{0} 执行完成", "{0} completed"),
    ("{0} 执行失败", "{0} failed"),
    ("请先连接内核并选择会话。", "Connect and select a session first."),
    ("会话已切换，请关闭后重新打开。", "Session changed. Close and reopen this panel."),
    ("尚未设置目标", "Not set"),
    ("已启动轮数：", "Rounds started: "),
    ("目标内容", "Objective"),
    ("最大轮数（创建时可留空使用内核默认值）", "Maximum rounds (optional on creation)"),
    (
        "创建或恢复目标可能启动内核执行；仅在确认目标后操作。",
        "Creating or resuming a goal may start kernel execution. Act only after confirming the objective.",
    ),
    ("目标", "Goal"),
    ("目标内容不能为空。", "Objective cannot be empty."),
    ("最大轮数必须为正整数。", "Maximum rounds must be a positive integer."),
    ("请先刷新目标。", "Refresh the goal first."),
    ("重新读取失败：", "Refresh failed: "),
    (
        "计划仅支持只读。当前主窗口未缓存 session/control 的 schedule 投影，无法显示计划列表；这不表示会话没有计划。\n此入口不创建、编辑或删除计划，也不调用 schedules RPC。",
        "Schedules are read-only. The main window does not currently cache the session/control schedule projection, so the list is unavailable; this does not mean the session has no schedules.\nNo schedules are created, edited or deleted; no schedules RPC is called.",
    ),
    ("计划（只读）", "Schedules (read-only)"),
    ("技能列表响应格式不正确。", "Unexpected skills/list response."),
    (
        "选择技能只会插入 /名称 到输入框，不会发送或执行。",
        "Selecting a skill only inserts /name into the input box; it does not send or execute it.",
    ),
    ("技能", "Skills"),
    ("没有可用技能。", "No skills available."),
    ("创建目标", "Create goal"),
    ("保存编辑", "Save edits"),
    ("暂停", "Pause"),
    ("标记完成", "Mark complete"),
    ("未选择会话", "No session selected"),
    (
        "在左侧选择一个会话后，这里显示该会话工作区的文件树。",
        "Select a session on the left to see its workspace file tree here.",
    ),
    ("读取工作区失败", "Failed to read workspace"),
    (
        "目录项超过内核上限，仅显示前 {0} 项。",
        "The directory exceeds the kernel cap; showing the first {0} entries only.",
    ),
    ("列目录失败（{0}）", "Failed to list directory ({0})"),
    ("（空目录）", "(empty directory)"),
    ("刷新失败", "Refresh failed"),
    ("正在读取…", "Reading…"),
    ("{0} · {1} 行 · 版本 {2}", "{0} · {1} lines · version {2}"),
    (
        "文件较长，仅显示前 {0} 行（内核单页上限 {1} 行 / 2 MiB）。",
        "The file is long; showing the first {0} lines only (kernel page limit {1} lines / 2 MiB).",
    ),
    ("不支持预览", "Preview not supported"),
    (
        "该文件是二进制或非 UTF-8 文本（含 NUL 字节），壳内只预览文本。",
        "The file is binary or non-UTF-8 text (contains NUL bytes); the shell previews text only.",
    ),
    ("文件过大", "File too large"),
    (
        "超出内核单页读取上限（2 MiB / 5000 行），无法在这里完整预览。",
        "Exceeds the kernel single-page read limit (2 MiB / 5000 lines); cannot preview it fully here.",
    ),
    ("文件不存在", "File not found"),
    ("路径已消失（可能被移动或删除）。", "The path is gone (it may have been moved or deleted)."),
    ("该路径不是普通文件。", "The path is not a regular file."),
    ("超出工作区", "Outside workspace"),
    ("该路径不在当前会话的工作区内。", "The path is not inside the current session's workspace."),
    ("会话不可用", "Session unavailable"),
    (
        "该会话没有活跃 agent，内核无法解析工作区。",
        "The session has no active agent, so the kernel cannot resolve the workspace.",
    ),
    ("读取失败", "Read failed"),
    ("返回文件树", "Back to file tree"),
    ("刷新文件树", "Refresh file tree"),
    ("关闭文件面板", "Close file panel"),
    ("路径不存在（可能已被移动或删除）。", "The path does not exist (it may have been moved or deleted)."),
    ("该路径不是目录。", "The path is not a directory."),
    ("路径超出该会话工作区。", "The path is outside this session's workspace."),
    ("内容超出内核单页上限。", "The content exceeds the kernel page limit."),
    ("不是 UTF-8 文本，无法预览。", "Not UTF-8 text; cannot preview."),
    ("该会话没有活跃 agent（会话未打开或已归档）。", "The session has no active agent (not opened or archived)."),
    ("大小未知", "Size unknown"),
    ("未标注模型", "Unnamed model"),
    ("{0}：{1} tokens", "{0}: {1} tokens"),
    ("{0} 失败", "{0} failed"),
    ("对话消息列表", "Conversation messages"),
    ("标准模式", "Standard mode"),
    ("PTC 模式", "PTC mode"),
    ("极简模式", "Minimal mode"),
    ("创造模式", "Creative mode"),
    ("默认预设", "Default preset"),
    ("减小字号", "Decrease font size"),
    ("增大字号", "Increase font size"),
    ("浏览文件夹", "Browse folders"),
    ("上下文", "Context"),
    ("排队", "Queued"),
    ("运行中", "Running"),
    ("停止中", "Stopping"),
    ("已完成", "Completed"),
    ("已终止", "Terminated"),
    ("失败", "Failed"),
    ("压缩以上对话内容", "Compress earlier conversation history"),
    ("将当前会话内容导出为 ZIP", "Export this session as a ZIP archive"),
    ("发送关于当前会话的反馈", "Send feedback about this session"),
    ("设置或查看长期任务目标", "Set or view the goal for a long-running task"),
    ("切换权限预设（沙箱模式与审批策略）", "Switch the permission preset (sandbox mode and approval policy)"),
    ("进入或退出计划模式", "Enter or leave plan mode"),
    (
        "功能完整的编码 Agent，支持文件编辑、Shell、文件与网页检索、Skills、计划、目标、子代理和工作流。",
        "A full-featured coding agent: file editing, shell, file and web search, skills, plans, goals, subagents and workflows.",
    ),
    (
        "功能完整的编码 Agent，但默认不提供 workflow 工具；其他工具通过 PTC 模式 SDK 呈现，让模型用一个 TypeScript 程序组合多步操作。",
        "A full-featured coding agent without the workflow tool by default; other tools surface through the PTC-mode SDK so the model composes multi-step work in one TypeScript program.",
    ),
    (
        "仅提供持久 shell 的单工具编码 Agent。",
        "A single-tool coding agent that only provides a persistent shell.",
    ),
    (
        "用于创建自定义 Agent preset：具备标准模式的全部能力，并提供运行时检查、插件实验和 preset 创作指导。",
        "For authoring custom agent presets: full standard-mode capabilities plus runtime inspection, plugin experimentation and preset-authoring guidance.",
    ),
    ("开", "On"),
    ("关", "Off"),
    ("模型与推理等级，当前 {0}", "Model and reasoning effort, currently {0}"),
    ("外观：{0}", "Appearance: {0}"),
    ("当前字号 {0}px", "Current font size {0}px"),
    ("{0} API 密钥", "{0} API key"),
    ("清除 {0} 的 API 密钥", "Clear the API key for {0}"),
    ("发现 {0} 的模型", "Discover models for {0}"),
    ("复制预设 {0}", "Copy preset {0}"),
    ("当前会话使用预设 {0}", "Use preset {0} for this session"),
    ("设为默认预设 {0}", "Set {0} as default preset"),
    ("打开预设 {0} 的目录", "Open the directory of preset {0}"),
    ("删除预设 {0}", "Delete preset {0}"),
    ("{0} 曲线", "{0} curve"),
    ("访问模式：{0}", "Access mode: {0}"),
    ("访问模式：{0}。{1}", "Access mode: {0}. {1}"),
    ("分组方式：{0}", "Group by: {0}"),
    ("排序方式：{0}", "Sort by: {0}"),
    ("打开工作区路径：{0}", "Open workspace path: {0}"),
    ("重命名工作区：{0}", "Rename workspace: {0}"),
    ("上移：{0}", "Move up: {0}"),
    ("下移：{0}", "Move down: {0}"),
    ("删除工作区：{0}", "Delete workspace: {0}"),
    ("移动到工作区：{0}", "Move to workspace: {0}"),
    ("跳转到 {0}", "Navigate to {0}"),
    ("交付物 seq={0}：{1}", "Deliverable seq={0}: {1}"),
    ("移除附件 {0}", "Remove attachment {0}"),
    ("选项：{0}", "Option: {0}"),
    ("预设 {0} 的内容", "Content of preset {0}"),
    ("模型：{0}", "Model: {0}"),
    ("推理等级：{0}", "Reasoning effort: {0}"),
    ("收起已展开的会话", "Collapse expanded sessions"),
    ("编辑排队项", "Edit queued item"),
    ("删除排队项", "Delete queued item"),
    ("排队项内容", "Queued item content"),
    ("{0} 轮 {1} 步 · {2} tok/s", "{0} turns {1} steps · {2} tok/s"),
    ("{0} tok · 缓存命中 {1}%", "{0} tok · cache hit {1}%"),
    ("打开 Blade²", "Open Blade²"),
    ("工具审批请求", "Tool approval request"),
    ("会话统计", "Session stats"),
    ("缓存读取", "Cache read"),
    ("输出速度（TPS）", "Output speed (TPS)"),
    ("未缓存输入", "Uncached input"),
    ("首 token 平均（TTFT）", "Avg. time to first token (TTFT)"),
    ("工具调用用时", "Tool-call time"),
    ("模型用时", "Model time"),
    ("上下文注入", "Context injection"),
    ("Token 用量", "Token usage"),
    ("API 密钥已配置", "API key configured"),
    ("API 密钥缺失", "API key missing"),
    ("添加提供方", "Add provider"),
    ("目录里没有可添加的提供方", "No providers left to add in the catalog"),
    ("自定义提供方", "Custom provider"),
    ("创建提供方", "Create provider"),
    (
        "以小写字母开头的标识，在请求中唯一标识该提供方，并用于派生凭据名。",
        "A lowercase identifier starting with a letter that uniquely names this provider in requests and derives its credential name.",
    ),
    ("Provider ID", "Provider ID"),
    (
        "需以小写字母开头，之后可用小写字母、数字和短横线。",
        "Start with a lowercase letter; then lowercase letters, digits, and dashes.",
    ),
    ("已有提供方使用了这个 ID。", "A provider already uses this ID."),
    ("可选，默认使用 Provider ID", "Optional; defaults to the provider ID"),
    ("显示名称", "Display name"),
    ("API 地址", "Base URL"),
    ("请输入有效的 HTTP 或 HTTPS 地址。", "Enter a valid HTTP or HTTPS URL."),
    ("API 协议", "API protocol"),
    ("由启动环境提供（只读）", "Provided by the launch environment (read-only)"),
    ("已配置——输入新值可替换", "Configured — type a new value to replace"),
    ("输入 API 密钥，或留空使用环境认证", "Enter an API key, or leave blank to use environment authentication"),
    ("自定义设置", "Customized settings"),
    (
        "请输入 API 密钥；留空则保持已存储的密钥。",
        "Enter the API key, or leave the field empty to keep the stored one.",
    ),
    ("该 API 密钥格式错误，请检查。", "This API key is not in a valid format. Please check it."),
    ("容量需为数字，可加 K 或 M 后缀。", "A capacity must be a number, optionally suffixed K or M."),
    ("恢复默认模型", "Restore defaults"),
    ("获取可用模型", "Fetch available models"),
    ("已自定义模型目录", "Customized model catalog"),
    ("正在使用适配器默认模型", "Using the adapter defaults"),
    (
        "模型选择器中将不显示任何模型；目录外 ID 仍可直接发送。",
        "No models will be shown in the selector. Unlisted IDs can still be sent directly.",
    ),
    ("模型 ID", "Model ID"),
    ("容量", "Capacities"),
    ("删除模型", "Delete model"),
    ("上下文窗口", "Context window"),
    ("最大输出 token 数", "Max output tokens"),
    ("该提供方没有列出任何模型，请手动添加。", "The provider listed no models. Add them by hand."),
    ("搜索模型", "Search models"),
    ("全选", "Select all"),
    (
        "以下是模型提供方的可用模型，勾选要添加的模型。",
        "These are the models this provider has available. Choose the ones to add.",
    ),
    ("没有匹配的模型。", "No matching models."),
    ("取消全选", "Deselect all"),
    ("选择要添加的模型", "Choose models to add"),
    ("添加所选", "Add selected"),
    (
        "请输入 API 密钥；若该提供方以其他方式鉴权，可以留空。",
        "Enter the API key, or leave the field empty if this provider authenticates another way.",
    ),
    ("Provider ID 不能为空。", "The provider ID cannot be empty."),
    ("自定义提供方需要填写 API 地址。", "A custom provider needs a base URL."),
    ("API 协议不能为空。", "The API protocol cannot be empty."),
    ("自定义提供方至少需要一个模型。", "A custom provider needs at least one model."),
    (
        "这张卡片打开期间，这些设置已被其他地方改动。请关闭后重新打开，在当前值上编辑。",
        "These settings changed elsewhere while this card was open. Close and reopen it to edit the current values.",
    ),
    ("写入失败，请检查连接和字段值后重试。", "The write failed; check the connection and field values, then retry."),
    ("所选时间范围内暂无用量记录", "No usage records in the selected range"),
    ("编辑 {0}", "Edit {0}"),
    ("删除 {0}", "Delete {0}"),
    (
        "{0}：密钥保存失败，请检查连接和凭据来源是否可写后重试。",
        "{0}: key save failed; check the connection and whether the credential source is writable, then retry.",
    ),
    ("已保存 {0}。", "Saved {0}."),
    ("删除 {0}？", "Delete {0}?"),
    ("删除 {0} 会移除其配置和存储的 API 密钥。", "Deleting {0} removes its configuration and stored API key."),
    (
        "删除 {0} 会移除其配置；其使用的凭据（如有）由其他位置管理，将会保留。",
        "Deleting {0} removes its configuration; any credential it uses is managed elsewhere and will be kept.",
    ),
    ("模型 {0}：模型 ID 不能为空。", "Model {0}: the model ID cannot be empty."),
    ("模型 {0}：模型 ID 不能重复。", "Model {0}: each model ID may appear once."),
    ("模型 {0}：显示名称不能为空。", "Model {0}: the display name cannot be empty."),
    (
        "模型 {0}：上下文窗口必须是正数，例如 131072、256K 或 1M。",
        "Model {0}: the context window must be a positive count, like 131072, 256K, or 1M.",
    ),
    (
        "模型 {0}：最大输出 token 数必须是正数，例如 8192、64K 或 1M。",
        "Model {0}: max output tokens must be a positive count, like 8192, 64K, or 1M.",
    ),
    ("没有找到与 {0} 相关的结果", "No results found for {0}"),
    ("，跳过 {0} 个空会话", ", {0} empty sessions skipped"),
    (
        "数据来源：内核 journal（session/list + session/page）的用量记录；已聚合 {0} 个非空会话、{1} 条记录{2}{3}。",
        "Data source: kernel journal (session/list + session/page) usage records; aggregated {0} non-empty sessions and {1} records{2}{3}.",
    ),
    ("每日 Token 趋势图：近 {0} 天暂无用量", "Daily token trend: no usage in the last {0} days"),
    ("合计 {0} tokens", "Total {0} tokens"),
    (
        "模型用量：近 {0} 天共 {1} tokens，{2} 个模型",
        "Model usage: {1} tokens over the last {0} days, {2} models",
    ),
    ("占比 {0}%", "{0}% of total"),
    ("缓存命中", "Cache hit"),
    ("输出", "Output"),
    ("提供方", "Provider"),
    ("未选择", "Not selected"),
    ("模型目录", "Models"),
    ("填入各提供方的 API 密钥即可使用其模型。", "Enter each provider's API key to use its models."),
    ("模型 ID 不能为空。", "The model ID cannot be empty."),
    ("添加模型", "Add model"),
    (
        "开启后，Agent 可以为每个 Subagent 选择提供方和模型。仅影响新会话。",
        "When on, the agent may pick provider and model per subagent. New conversations only.",
    ),
    ("正在准备内核组件…", "Preparing kernel components…"),
    ("正在启动内核…", "Starting the kernel…"),
    ("正在连接内核…", "Connecting to the kernel…"),
    ("正在加载工作区与会话…", "Loading workspaces and conversations…"),
    ("内核加载已进行 {0} 秒", "Kernel loading for {0} seconds"),
    ("第 {0} 步，共 {1} 步 · {2}%", "Step {0} of {1} · {2}%"),
    ("正在安装插件 {0}/{1}", "Installing plugins {0}/{1}"),
    ("重试", "Retry"),
    ("内核还在加载中，请稍候再发。", "The kernel is still loading; please try sending again shortly."),
    // —— 主线 ZH 表里有、但主线 ShellEnglish 没有 EN 的键：EN 为分叉自译（主线在这些键上回退中文），
    //    其中若干条是主线后来改过词的陈旧变体，移植对应分区时以主线现文案为准 ——
    (
        "本机全部会话的用量，来自内核 journal（session/list + session/page）。统计范围：应用用量（所有会话）。",
        "Usage for all sessions on this machine, from the kernel journal (session/list + session/page). Scope: app usage (all sessions).",
    ),
    (
        "导入图片作为写代码时的内容区背景，透明度自动压低以保证正文可读",
        "Imports an image as the content-area background while coding; its opacity is lowered automatically so the text stays readable",
    ),
    ("应用用量", "App usage"),
    ("统计范围：应用用量", "Scope: app usage"),
    ("刷新使用统计", "Refresh usage statistics"),
    ("重新从内核读取会话用量并重绘", "Re-reads session usage from the kernel and repaints the panel"),
    ("所选时间范围内没有用量记录", "No usage records in the selected range"),
    (
        "本机全部会话的用量，来自内核 journal（session/list + session/page）。",
        "Usage for all sessions on this machine, from the kernel journal (session/list + session/page).",
    ),
    ("统计范围：应用用量（所有会话）。", "Scope: app usage (all sessions)."),
    (
        "已聚合 {0} 个非空会话、{1} 条 assistant/message 用量记录",
        "Aggregated {0} non-empty sessions and {1} assistant/message usage records",
    ),
    ("（跳过 {0} 个空会话{1}）。", "({0} empty sessions skipped{1})."),
    (
        "数据来源：内核 session/list + session/page 的 journal 用量记录（assistant/message.usage）；",
        "Data source: kernel session/list + session/page usage records (assistant/message.usage);",
    ),
    ("已聚合 {0} 个非空会话、{1} 条记录。", "Aggregated {0} non-empty sessions and {1} records."),
    (
        "模型用量环形图：近 {0} 天共 {1} tokens，{2} 个模型",
        "Model usage ring: {1} tokens over the last {0} days, {2} models",
    ),
    (
        "自动审批由内核插件 dsh-approval-gate 提供：当会话权限预设切到「自动审批」时，Flash 模型会预判每次写入/命令是否不可回补——安全则自动批准，涉及删除、凭据、系统配置等硬类别转人工确认。开关增删内核 profile patch 里的 auto-approve 预设，由内核热重载即时生效。",
        "Auto approval comes from the kernel plugin dsh-approval-gate: when a session permission preset switches to Auto approval, the Flash model pre-judges whether each write/command is irreversible - safe ones are approved automatically, while hard categories such as deletions, credentials and system configuration fall back to manual confirmation. The toggle adds or removes the auto-approve preset in the kernel profile patch, which the kernel hot-reloads immediately.",
    ),
    ("{0} tokens（近 {1} 天）", "{0} tokens (last {1} days)"),
    // 目标条 / 上下文圈 / 气泡卡 / 附件 / 使用统计：主线 ShellEnglish 已登记的原文照抄。
    // 管理、管理目标、已用 token 主线在代码里用了却没登记进表（英文界面会露出中文），
    // 这 3 条的译文是分叉自拟的。
    // 轮次轨兜底那枚 `第 {0} 轮` 曾按同样口径自拟过一枚 "Turn {0}"，本轮（刀 C）已撤：主干
    // `MainWindow.TurnRail.cs` 用的是 `LF("第 {0} 轮", mark.Turn)`（通道 A），ShellEnglish 里
    // 根本没有这个裸键 ⇒ 主干英文档自己就吐中文 `第 5 轮`。分叉补一枚表值 = 造主干没有的
    // 回落档，属冒名登记；撤表后分叉与主干同串，见
    // `en_table_entries_that_leaked_from_the_bilingual_family_stay_render_compatible`。
    ("目标待命", "Goal armed off"),
    ("目标进行中", "Goal active"),
    ("目标已暂停", "Goal paused"),
    ("目标受阻", "Goal blocked"),
    ("管理", "Manage"),
    ("管理目标", "Manage goal"),
    ("上下文已用 {0}%", "{0}% of context used"),
    ("已用", "Used"),
    ("上下文容量", "Context capacity"),
    ("已用 token", "Used tokens"),
    ("消息气泡", "Message bubbles"),
    (
        "气泡背景：半透明直接透出背后画面，亚克力是系统材质；两项都即时生效。",
        "Bubble background: the translucent option shows the content behind it, the acrylic option is the system material; both apply immediately.",
    ),
    ("气泡材质", "Bubble material"),
    (
        "半透明最透，亚克力是系统材质，跟随跟窗口材质走",
        "Translucent is the most see-through, acrylic is the system material, follow tracks the window material",
    ),
    ("气泡不透明度", "Bubble opacity"),
    (
        "数值越大气泡自身越实，背后画面透出越少",
        "Higher values make bubbles more solid and show less of what is behind them",
    ),
    ("半透明", "Translucent"),
    ("亚克力", "Acrylic"),
    ("跟随窗口材质", "Follow window material"),
    // —— 「窗口材质」卡（主干 MainWindow.Personalization.cs:67-73 的四颗选项 + 卡头行标）——
    ("窗口材质", "Window material"),
    (
        "侧栏与顶栏透出的系统材质：Mica 柔和、亚克力更透；系统不支持时自动回退。",
        "The system material the sidebar and top bar show through: Mica is subtle, acrylic is more transparent; falls back automatically when unsupported.",
    ),
    ("材质", "Material"),
    ("切换后立即生效，重启后保持", "Applies immediately and persists across restarts"),
    ("Mica", "Mica"),
    ("Mica Alt", "Mica Alt"),
    ("无（纯色）", "None (solid)"),
    ("图片读取失败，未添加附件：{0}", "Failed to read the image; no attachment added: {0}"),
    ("拖入图片 {0:HHmmss}.png", "Dropped image {0:HHmmss}.png"),
    (
        "有 {0} 张图片无法按图片识别，已改为文件发送：{1}",
        "{0} image(s) could not be recognized as images and were sent as files: {1}",
    ),
    ("放大查看 {0}", "Enlarge {0}"),
    ("{0} 条用量记录", "{0} usage records"),
    ("单日峰值 · {0}", "Peak day · {0}"),
    ("{0} 个会话中的最大值", "Max across {0} sessions"),
    ("截至今日", "As of today"),
    ("共活跃 {0} 天", "{0} active days in total"),
    ("暂无用量记录", "No usage records"),
    ("无完整 turn 记录", "No complete turns"),
    ("今日暂无记录", "No records today"),
    ("暂无活跃记录", "No active days"),
    // —— 设置·记忆「记忆内容」卡（主干 MainWindow.xaml.cs:14356-14357、14519）——
    // EN 逐字照主干 `ShellEnglish:1583-1591`。`已自动保存 · {0} · {1} 行文本已转为记忆实体`
    // 是 #70（落盘接线）要用的那条，先把文案钉住。
    ("记忆内容", "Memory contents"),
    (
        "直接编辑记忆文件（JSONL）。停止输入或点到别处即自动保存；自然语言行会自动转成记忆实体；模型写入时自动重新载入。",
        "Edit the memory file (JSONL) directly. It auto-saves when you stop typing or click elsewhere; plain-language lines are turned into memory entities automatically; reloads when the model writes.",
    ),
    ("共 {0} 行 · {1}", "{0} lines · {1}"),
    ("停止输入后自动保存", "Auto-saves when you stop typing"),
    (
        "已自动保存 · {0} · {1} 行文本已转为记忆实体",
        "Auto-saved · {0} · {1} line(s) turned into memory entities",
    ),
    // —— 设置·宠物安装命令输入框（主干 MainWindow.Pet.cs:595）——
    (
        "petdex install 或 codex-pets add <宠物标识>",
        "petdex install or codex-pets add <pet-slug>",
    ),
    // —— 设置·关于的更新源一行（主干 MainWindow.About.cs:108）——
    ("更新源：{0}", "Update source: {0}"),
    // —— 任务 #94 批 1/6：i94-keys-missing 数据行 1..50（主干 MainWindow.xaml.cs 625..990 段前半）——
    ("预设即一个会话的 Agent 所运行的插件组装——它的工具、提示词与能力。复制一份既有预设改成自己的，或用「创造模式」让 Agent 帮你创建。", "A preset is the plugin composition one session's agent runs — its tools, prompt, and capabilities. Duplicate an existing one and make it yours, or let the agent draft one for you in Creator mode."),
    ("确认启用完全权限？", "Enable Full access?"),
    ("启用完全权限后，新会话将减少确认步骤，并且可以直接执行更多操作，包括敏感操作、文件修改或外部命令。仅建议在你信任后续任务时使用。", "Full access lets new sessions reduce confirmation steps and perform more actions directly, including sensitive operations, file changes, or external commands. Only use it when you trust subsequent tasks."),
    ("启用完全权限后，智能体将减少确认步骤，并且可以直接执行更多操作，包括敏感操作、文件修改或外部命令。仅建议在你信任当前任务时使用。", "Full access reduces confirmation steps and lets the agent perform more actions directly, including sensitive operations, file changes, or external commands. Only use it when you trust the current task."),
    ("我已了解风险，并愿意继续", "I understand the risks and want to continue"),
    ("启用完全权限", "Enable Full access"),
    ("已覆盖", "Overridden"),
    ("恢复 {0} 的默认值", "Reset {0} to its default"),
    ("清除本字段的用户覆盖，回到内核默认值。", "Clear the user override for this field and restore the kernel default."),
    ("恢复默认失败：{0}", "Reset failed: {0}"),
    ("请填数字；留空表示使用默认值。", "Enter a number, or leave blank to use the default."),
    ("已配置密钥。", "A key is configured."),
    ("未配置密钥；配置之前搜索不可用。", "No key is configured; search is unavailable until one is."),
    ("由 settings/describe 泛化渲染的附加插件设置。", "Additional plugin settings rendered generically from settings/describe."),
    ("由 settings/describe 泛化渲染；字段语义以内核 schema 为准。", "Rendered generically from settings/describe; field semantics follow the kernel schema."),
    ("搜索插件", "Search plugins"),
    ("暂无插件。", "No plugins are available."),
    ("没有匹配的插件。", "No matching plugins."),
    ("已停用", "Disabled"),
    ("加载中", "Loading"),
    ("等待依赖", "Waiting for dependencies"),
    ("卸载中", "Unloading"),
    ("未运行", "Not running"),
    ("启动失败", "Failed to start"),
    ("开启后，Agent 可以从下方授权模型中，为每个 Subagent 选择提供方、模型和推理强度。仅影响新会话。", "When enabled, agents can choose a provider, model, and reasoning effort for each subagent from the authorized models below. Applies only to new sessions."),
    ("标签结构", "Tag outline"),
    ("标题", "Title"),
    ("一级标题", "H1"),
    ("链接", "Links"),
    ("（无）", "(none)"),
    ("打开附属资源", "Open related resource"),
    ("打开附属资源 {0}", "Open related resource {0}"),
    ("标签结构 · 标题：{0} · 一级标题 {1} · 链接 {2} · 图片 {3}", "Tag outline · Title: {0} · H1 ×{1} · Links ×{2} · Images ×{3}"),
    ("DOM 预览 · 标题：{0} · 一级标题 {1} · 链接 {2} · 图片 {3}（脚本已禁用）", "DOM preview · Title: {0} · H1 ×{1} · Links ×{2} · Images ×{3} (scripts disabled)"),
    ("附属资源：{0} 项（已内联进 DOM）", "Related resources: {0} (inlined into DOM)"),
    ("源码", "Source"),
    ("切换 HTML 源码视图", "Toggle HTML source view"),
    ("加密 PDF", "Password-protected PDF"),
    ("加密 PDF：Windows.Data.Pdf 无解锁 API，壳内无法解密渲染。可用「用系统打开」查看，或复制路径后用其它工具解锁。", "Password-protected PDF: Windows.Data.Pdf has no unlock API, so the shell cannot decrypt it. Use Open with system, or copy the path and unlock it elsewhere."),
    ("文件已损坏或平台不支持渲染。", "The file is corrupt, or rendering is unsupported on this platform."),
    ("复制路径", "Copy path"),
    ("路径已复制", "Path copied"),
    ("复制路径失败：{0}", "Copy path failed: {0}"),
    ("导入图片或视频作为写代码时的内容区背景，用滑杆调到既能看出图、又不吃正文的浓度", "Import an image or video as the content background. Use the slider to balance visibility against text readability"),
    ("导入视频", "Import video"),
    ("导入视频背景皮肤", "Import video background"),
    ("png / jpg / webp / bmp · mp4 / webm / mov（视频静音循环，与图片二选一）", "png / jpg / webp / bmp · mp4 / webm / mov (videos play muted on loop; a video and an image are mutually exclusive)"),
    ("背景透明度", "Background opacity"),
    ("拖动即时生效；没有导入图片时先记下，导入后按这个浓度显示", "Applies as you drag; without an image the value is kept for the next import"),
    ("失焦暂停视频", "Pause video when unfocused"),
    // —— 任务 #94 批 2/6：i94-keys-missing 数据行 51..100（主干 MainWindow.xaml.cs 625..990 段后半）——
    ("视频背景只在窗口聚焦时播放；切到别的程序时自动暂停，回焦继续", "The video background plays only while the window is focused; it pauses automatically when you switch to another program and resumes on return"),
    ("窗口失焦时暂停视频背景", "Pause the video background when the window is unfocused"),
    ("Wallpaper Engine", "Wallpaper Engine"),
    ("用本机 Wallpaper Engine 订阅的壁纸做背景（视频静音循环），与上面导入的图片/视频二选一", "Use wallpapers from this machine's Wallpaper Engine subscriptions as the background (videos play muted on loop); mutually exclusive with the imported image/video above"),
    ("点缩略图即套用；视频项显示工坊预览图，与上面导入的图片/视频二选一。", "Click a thumbnail to apply it; video items show their workshop preview image; mutually exclusive with the imported image/video above."),
    ("Wallpaper Engine 壁纸缩略图", "Wallpaper Engine wallpaper thumbnails"),
    ("读取失败：{0}", "Load failed: {0}"),
    ("读取失败：壁纸服务没响应。本机需要装有 Wallpaper Engine 并订阅壁纸。", "Load failed: the wallpaper service did not respond. This machine needs Wallpaper Engine installed with subscribed wallpapers."),
    ("未检测到壁纸插件。重启 Blade² 会自动重试安装；离线时这条会一直出现，连上网再试。", "Wallpaper plugin not detected. Restarting Blade² retries the install; while offline this keeps showing until you are back online."),
    ("本机没有可用的 Wallpaper Engine 壁纸：需要安装 Wallpaper Engine 并订阅壁纸。", "No usable Wallpaper Engine wallpapers on this machine: install Wallpaper Engine and subscribe to wallpapers."),
    ("没有可用壁纸", "No wallpapers available"),
    ("共 {0} 个壁纸，点缩略图套用", "{0} wallpapers — click a thumbnail to apply"),
    ("（已隐藏 {0} 个低分辨率预览图）", " ({0} low-resolution previews hidden)"),
    ("视频", "Video"),
    ("把壁纸「{0}」设为背景", "Set wallpaper \"{0}\" as the background"),
    ("正在下载「{0}」…", "Downloading \"{0}\"…"),
    ("这个壁纸没有可用的文件。", "This wallpaper has no usable file."),
    ("下载失败：壁纸服务没返回这个文件。", "Download failed: the wallpaper service did not return this file."),
    ("已应用「{0}」", "Applied \"{0}\""),
    ("个性化", "Personalization"),
    ("指令决定它怎么回应你，材质与皮肤决定它看起来是什么样。", "Instructions shape how it responds to you; the material and skin shape how it looks."),
    ("自定义指令", "Custom instructions"),
    ("写给所有对话的长期说明：怎么称呼你、用什么语气、有哪些固定约束。对所有会话始终生效。", "Standing notes for every conversation: how to address you, what tone to use, which constraints always apply."),
    ("新会话立即生效；进行中的会话在下一条消息生效。", "New conversations pick it up right away; a running conversation applies it on the next message."),
    ("保存失败：{0}", "Save failed: {0}"),
    ("共 0 字 · 停止输入后自动保存", "0 characters · Auto-saves when you stop typing"),
    ("字数 {0} / {1}", "{0} / {1} characters"),
    ("允许此插件的后续版本", "Allow future versions of this plugin"),
    ("上一题", "Previous question"),
    ("下一题", "Next question"),
    ("跳过本题", "Skip this question"),
    ("放弃整组问题", "Dismiss all questions"),
    ("去聊天里说", "Chat about it"),
    ("推荐", "Recommended"),
    ("请先完成这道问题。", "Please complete this question first."),
    ("请选择一个选项或填写自定义答案。", "Please select an option or enter a custom answer."),
    ("确认执行", "Approve"),
    ("输入你的答案", "Type your answer"),
    ("已放弃该组问题。", "The question set was dismissed."),
    ("放弃提问失败，请重试：{0}", "Failed to dismiss the questions: {0}"),
    ("第 {0} / {1} 题", "Question {0} of {1}"),
    ("新建工作区…", "New workspace…"),
    ("快速、高效、经济", "Fast, efficient, economical"),
    ("更强自主编码/复杂推理（成本更高）", "Stronger autonomous coding / complex reasoning (higher cost)"),
    ("{0}（上次使用）", "{0} (last used)"),
    ("模型：{0}，上次使用", "Model: {0}, last used"),
    ("预览版 · DSH 本地构建 · 当前为内测阶段（完整内测声明见首次启动欢迎页）", "Preview · DSH local build · currently in internal testing (see the first-run welcome notice for the full statement)"),
    ("推理等级：{0}。{1}", "Reasoning effort: {0}. {1}"),
    ("快速、高效且经济；适合目标明确、常规或并行任务。", "Fast, efficient, and economical; suited to focused, routine, or parallel tasks."),
    ("更强的自主编码、知识与复杂推理能力；适合复杂或质量优先的任务，但成本更高。", "Stronger agentic coding, knowledge, and difficult reasoning; suited to complex or quality-critical tasks at higher cost."),
    // —— 任务 #94 批 3/6：i94-keys-missing 数据行 101..200（主干 MainWindow.xaml.cs 992..1506 段）——
    ("上次使用", "Last used"),
    ("上次使用：{0}", "Last used: {0}"),
    ("恢复上次使用的模型：{0}", "Restore the last-used model: {0}"),
    ("模型切换失败", "Model switch failed"),
    ("未能切换到 {0}：{1}", "Failed to switch to {0}: {1}"),
    ("未能切换到 {0}：目录中未找到该模型。", "Failed to switch to {0}: the model is not in the catalog."),
    ("一键恢复适配器默认模型目录与内核默认模型。自定义提供方的模型列表不受影响。", "One-click restore of the adapter default catalog and kernel default model. Custom provider model lists are left alone."),
    ("将默认模型与模型目录恢复为内核默认。", "Restore the default model and model catalog to kernel defaults."),
    ("已恢复默认模型。", "Default models restored."),
    ("恢复默认模型失败：{0}", "Failed to restore default models: {0}"),
    ("预览版", "Preview"),
    ("DSH 本地构建", "DSH Local Build"),
    ("品牌与声明", "Brand & notices"),
    ("版本通道", "Release channel"),
    ("显示名称不能为空。", "The display name cannot be empty."),
    ("复制参数 JSON", "Copy parameters JSON"),
    ("复制结果 JSON", "Copy result JSON"),
    ("复制属性路径", "Copy property path"),
    ("展开详情", "Expand details"),
    ("收起详情", "Collapse details"),
    ("展开其余 {0} 行", "Expand {0} more lines"),
    ("收起差异", "Collapse diff"),
    ("展开差异", "Expand diff"),
    ("差异", "Diff"),
    ("命令", "Command"),
    ("退出码 {0}", "Exit code {0}"),
    ("信号 {0}", "Signal {0}"),
    ("已超时", "Timed out"),
    ("标准输出", "stdout"),
    ("标准错误", "stderr"),
    ("无输出", "(no output)"),
    ("参数", "Input"),
    ("结果", "Output"),
    ("执行中", "Running"),
    ("调用技能", "Use skill"),
    ("技能 {0}", "Skill {0}"),
    ("待办", "Todos"),
    ("待办 {0}/{1}", "Todos {0}/{1}"),
    ("进行中 {0}", "In progress {0}"),
    ("子代理 {0}", "Subagent {0}"),
    ("工作流", "Workflow"),
    ("工作流 {0}", "Workflow {0}"),
    ("撤回编辑", "Withdraw edit"),
    ("撤回本轮修改", "Revert changes this turn"),
    ("撤回本轮修改（把文件恢复到修改前）", "Revert changes this turn (restore the files to their state before the change)"),
    ("已撤回本轮修改", "Changes reverted"),
    ("撤回修改", "Revert"),
    ("将把本轮（第 {0} 轮）改过的文件恢复到修改前的状态：", "Restore the files changed in turn {0} to their state before the change:"),
    ("修改过的文件（恢复原内容）", "Edited files (original content restored)"),
    ("新建的文件（将被删除）", "Files created this turn (will be deleted)"),
    ("没有回退依据、撤回时会被跳过的文件", "Files with no revert basis; they will be skipped"),
    ("恢复后不可撤销。若这些文件在本轮之后又被改动过，撤回可能不完整。", "This cannot be undone. If these files were changed again after this turn, the revert may be incomplete."),
    ("已恢复 {0} 个文件。", "Restored {0} file(s)."),
    ("撤回未完成", "Revert incomplete"),
    ("成功 {0} 个，失败 {1} 个：{2}", "{0} succeeded, {1} failed: {2}"),
    ("文件已不存在", "File no longer exists"),
    ("找不到改动后的内容，文件可能已被其他改动覆盖", "The changed content was not found; the file may have been overwritten by other changes"),
    ("改动位置不唯一，为避免改错已放弃", "The change site is not unique; aborted to avoid editing the wrong place"),
    ("没有可用的回退依据", "No revert basis available"),
    ("上下文 {0}%", "Context {0}%"),
    ("{0} 个作业运行中", "{0} background jobs running"),
    ("适用场景：{0}", "When to use: {0}"),
    ("技能名称", "Skill name"),
    ("技能说明", "Skill description"),
    ("已安装技能", "Installed skills"),
    ("添加技能", "Add skill"),
    ("名称", "Name"),
    ("说明（必填）", "Description (required)"),
    ("适用场景（可选）", "When to use (optional)"),
    ("指令正文（可选）", "Instructions (optional)"),
    ("安装到", "Install into"),
    ("安装到哪个根目录", "Which root directory to install into"),
    ("kebab-case，例如 pdf-report", "kebab-case, e.g. pdf-report"),
    ("一句话说明这个技能做什么；调用时会显示它", "One line on what this skill does; shown in the skill menu"),
    ("什么时候该用它，例如「生成周报时」", "When it should be used, e.g. \"when writing a weekly report\""),
    ("模型加载这个技能后读到的具体指令", "The instructions the model reads after loading this skill"),
    ("名称不能为空。", "The name cannot be empty."),
    ("名称只能用小写字母、数字和短横线（kebab-case）。", "The name may contain only lowercase letters, digits and hyphens (kebab-case)."),
    ("说明不能为空：调用菜单里只显示名称和说明。", "The description cannot be empty: the skill menu shows only the name and description."),
    ("已有同名技能：内核只会加载优先级最高的那个，请先处理已有的。", "A skill with this name already exists: the kernel only loads the highest-priority one; deal with the existing one first."),
    ("Blade² 已安装的全部技能。技能是带说明的指令包：在输入框键入 /名称 即可调用，模型也可能在合适的时机自行调用。", "Every skill installed in Blade². A skill is a documented instruction bundle: type /name in the input box to invoke it, and the model may also call it on its own at a suitable moment."),
    ("按内核 @deepseek-ai/dsh-skill-filesystem 的根目录与优先级扫描，与内核看到的是同一份真相。", "Scanned across the same roots and priorities as the kernel's @deepseek-ai/dsh-skill-filesystem, so this is the same truth the kernel sees."),
    ("同名技能只生效优先级最高的一个：项目 > 预设 > 用户；被遮蔽的条目在列表里标注。", "Only the highest-priority skill of a given name takes effect: project > preset > user. Shadowed entries are marked in the list."),
    ("还没有安装任何技能。用上面的「添加技能」写第一个，或在项目的 .dsh/skills 目录里放一个。", "No skills installed yet. Write the first one with \"Add skill\" above, or drop one into the project's .dsh/skills directory."),
    ("项目技能需要先选中一个会话：它们按会话所在的项目目录解析。", "Project skills need a session selected first: they are resolved from the session's project directory."),
    ("（这个目录里还没有技能）", "(no skills in this directory yet)"),
    ("（本部署没有预设自带技能）", "(no preset ships skills in this deployment)"),
    ("（已被遮蔽，不生效）", "(shadowed; not in effect)"),
    ("模型不可自行调用（仅你能调用）", "The model cannot call it on its own (you only)"),
    ("共 {0} 个技能：{1}", "{0} skills: {1}"),
    ("{0} {1} 个", "{0} {1}"),
    ("{0}（{1}）", "{0} ({1})"),
    ("项目", "Project"),
    ("项目（兼容）", "Project (legacy)"),
    ("预设", "Preset"),
    ("用户", "User"),
    ("用户（兼容）", "User (legacy)"),
    ("扫描技能目录失败：{0}", "Failed to scan skill directories: {0}"),
    ("启用技能 {0}", "Enable skill {0}"),
    ("删除技能 {0}", "Delete skill {0}"),
    // —— 任务 #94 批 4/6：i94-keys-missing 数据行 201..300（主干 MainWindow.xaml.cs 1508..1753 段）——
    ("删除「{0}」？", "Delete \"{0}\"?"),
    ("将删除这个技能文件，内核随即不再加载它。此操作不可撤销。", "This deletes the skill file, and the kernel stops loading it immediately. This cannot be undone."),
    ("这个技能随预设分发，不能在这里删除。", "This skill ships with a preset and cannot be deleted here."),
    ("这个技能随预设分发，不能在这里关闭。", "This skill ships with a preset and cannot be disabled here."),
    ("这个文件没有 frontmatter，无法改写它的开关。", "This file has no frontmatter, so its switches cannot be rewritten."),
    ("切换技能开关失败：{0}", "Failed to toggle the skill: {0}"),
    ("「{0}」已开启。", "\"{0}\" is enabled."),
    ("「{0}」已关闭：/ 菜单与模型都不会再看到它。", "\"{0}\" is disabled: neither the / menu nor the model will see it."),
    ("创建技能失败：{0}", "Failed to create the skill: {0}"),
    ("自动换行", "Word wrap"),
    ("取消换行", "Disable word wrap"),
    ("加载更多", "Load more"),
    ("加载更多失败", "Load more failed"),
    ("重新读取", "Reload"),
    ("重新读取文件", "Reload file"),
    ("复制全文", "Copy all"),
    ("复制选中", "Copy selection"),
    ("没有选中的文本。", "No text is selected."),
    ("文件已更新，当前显示为旧内容。", "The file has changed; the preview is showing the previous content."),
    ("重新载入", "Reload now"),
    ("上一页", "Previous page"),
    ("下一页", "Next page"),
    ("用系统打开", "Open with system"),
    ("用系统打开失败：{0}", "Failed to open with the system: {0}"),
    ("第 {0} / {1} 页", "Page {0} of {1}"),
    ("无页面", "No pages"),
    ("无法在壳内渲染", "Cannot render in the shell"),
    ("PDF 预览降级：文件已读取，但页渲染失败（可能加密或损坏）。可用「用系统打开」查看。", "PDF preview degraded: the file was read but page rendering failed (it may be encrypted or corrupted). Use \"Open with system\" instead."),
    ("{0} · 版本 {1}", "{0} · version {1}"),
    ("空文件", "Empty file"),
    ("文件大小为 0，没有可显示的图像。", "The file is 0 bytes; there is no image to show."),
    ("无法解码图像", "Cannot decode image"),
    ("已读取 {0} 字节，但系统位图解码器无法识别该格式（SVG 等矢量图不在壳内渲染）。", "Read {0} bytes, but the system bitmap decoder cannot recognize the format (vector images such as SVG are not rendered in the shell)."),
    ("附属资源：无（未发现相对路径的 script/link/img 引用）。", "Related assets: none (no relative script/link/img references found)."),
    ("附属资源读取中…", "Loading related assets…"),
    ("附属资源（readRelated）：{0}", "Related assets (readRelated): {0}"),
    ("{0}（不可读）", "{0} (unreadable)"),
    ("…共 {0} 项", "… {0} total"),
    ("正在停止", "stopping"),
    ("已取消", "cancelled"),
    ("已失败", "failed"),
    ("计划待审", "Plan awaiting review"),
    ("等待回答", "Awaiting answer"),
    ("有活动定时任务", "Has active scheduled task"),
    ("清除目标", "Clear goal"),
    ("当前目标进行中。可输入 edit 修改 / pause 暂停 / resume 继续 / clear 清除", "goal active — edit / pause / resume / clear"),
    ("任务", "To-dos"),
    ("{0} 已完成", "{0} completed"),
    ("{0} 进行中", "{0} in progress"),
    ("{0} 待处理", "{0} pending"),
    ("待处理", "Pending"),
    ("进行中", "In progress"),
    ("停止（会话级取消：将停止本会话当前运行）", "Stop (session-level cancel: stops the current run of this session)"),
    ("桌面宠物由内核插件 @linxin666/dsh-pet 提供（Codex Pet 兼容）：聊天窗口里常驻一只宠物，模型干活时它跟着动，点它可以逗一逗。宠物素材装在 $DSH_HOME/pets，重启内核后收录。", "The desktop pet is provided by the kernel plugin @linxin666/dsh-pet (Codex Pet compatible): a pet lives in the chat window and reacts while the model works; click it to play. Pet assets live in $DSH_HOME/pets and are picked up after a kernel restart."),
    ("宠物", "Pets"),
    ("宠物显示在聊天窗口右下角，跟着模型的工作状态切换动画；点它可以逗一逗。", "The pet sits in the bottom-right of the chat window and switches animations with the model's activity; click it to play."),
    ("显示宠物", "Show the pet"),
    ("关掉后窗口里不再显示，插件其余功能照常。", "When off, it no longer shows in the window; the rest of the plugin keeps working."),
    ("宠物大小", "Pet size"),
    ("单元格高度的像素值，拖动即时生效。", "The cell height in pixels; changes apply while dragging."),
    ("{0} px", "{0} px"),
    ("宠物插件还没就位：安装包缺失或内核还没加载它。装过宠物后重启一次 Blade² 即可；引导器下次启动会自动重试安装。", "The pet plugin is not ready yet: the package is missing or the kernel has not loaded it. Install a pet and restart Blade² once; the bootstrapper retries the install on the next launch."),
    ("正在读取宠物清单…", "Reading the pet list…"),
    ("一只宠物都没有。装一个：把 Codex 宠物 zip 拖到下面的安装区，或粘贴 petdex install / codex-pets add <宠物标识>。", "No pets yet. Install one: drop a Codex pet zip onto the install area below, or paste petdex install / codex-pets add <pet-slug>."),
    ("宠物库", "Pet library"),
    ("已收录的宠物（插件内置 + ~/.codex/pets + 本机安装的）。切换立即生效。", "Pets on record (plugin built-ins + ~/.codex/pets + locally installed). Switching applies immediately."),
    ("使用中", "In use"),
    ("使用", "Use"),
    ("本机安装", "Installed locally"),
    ("Codex 宠物", "Codex pet"),
    ("插件内置", "Plugin built-in"),
    ("Live2D（壳内不渲染）", "Live2D (not rendered by the shell)"),
    ("使用宠物 {0}", "Use pet {0}"),
    ("删除宠物 {0}", "Delete pet {0}"),
    ("删除宠物「{0}」？", "Delete pet \"{0}\"?"),
    ("只删除本机安装目录里的文件。插件内置与 Codex 目录里的宠物不受影响；删除后重启 Blade² 生效。", "Only the locally installed directory is deleted. Plugin built-ins and pets in the Codex directory are untouched; the deletion takes effect after restarting Blade²."),
    ("安装宠物", "Install a pet"),
    ("兼容 Codex 宠物包（pet.json + 图集）。拖放 zip 到下方区域，或粘贴安装命令。", "Compatible with Codex pet packages (pet.json + spritesheet). Drop a zip onto the area below, or paste an install command."),
    ("把 Codex 宠物 zip 从文件资源管理器拖到这里即可安装。", "Drag a Codex pet zip here from File Explorer to install it."),
    ("浏览并安装", "Browse and install"),
    ("浏览并安装宠物压缩包", "Browse for a pet archive and install it"),
    ("宠物安装命令行", "Pet install command line"),
    ("安装", "Install"),
    ("按命令行安装宠物", "Install a pet from the command line"),
    ("正在安装 {0}…", "Installing {0}…"),
    ("正在解析并安装…", "Parsing and installing…"),
    ("已安装「{0}」到 {1}。重启 Blade² 后出现在宠物库里。", "Installed \"{0}\" to {1}. It appears in the pet library after restarting Blade²."),
    ("只支持 Codex 宠物 zip（里面有 pet.json 和图集）。", "Only Codex pet zips (containing pet.json and a spritesheet) are supported."),
    ("安装失败：{0}", "Install failed: {0}"),
    ("选择文件失败：{0}", "Picking the file failed: {0}"),
    ("先粘贴一条安装命令，例如 petdex install whale-girl。", "Paste an install command first, for example petdex install whale-girl."),
    ("诊断", "Diagnostics"),
    ("宠物不显示时先看这里：插件在不在、注册表报了什么、壳能不能画。", "Start here when the pet does not show: whether the plugin is present, what the registry reports, and whether the shell can draw it."),
    ("宠物插件（@linxin666/dsh-pet）未安装。", "The pet plugin (@linxin666/dsh-pet) is not installed."),
    ("宠物插件已安装并选入内核 bundle。", "The pet plugin is installed and selected into the kernel bundle."),
    ("宠物插件已安装但没选进 package.json 的 dsh.profile.bundles，内核不会加载它。", "The pet plugin is installed but is not selected into dsh.profile.bundles in package.json, so the kernel will not load it."),
    ("正在读取注册表诊断…", "Reading registry diagnostics…"),
    ("注册表没有报错。", "The registry reports no problems."),
    ("插件路由没有响应（/api/pet/* 404）：宠物功能整体不可用，重启 Blade² 让内核重新加载插件。", "The plugin routes do not respond (/api/pet/* 404): the pet feature is unavailable; restart Blade² so the kernel reloads the plugin."),
    ("若宠物显示不出来：Windows 需要 WebP 映像扩展才能解 .webp 图集，缺失时壳会退到预览 GIF，再不行就连精灵一起隐藏。", "If the pet does not render: Windows needs the WebP image extension to decode .webp atlases; without it the shell falls back to the preview GIF, and if that fails too the sprite is hidden."),
    // —— 任务 #94 批 5/6：i94-keys-missing 数据行 301..400（主干 MainWindow.xaml.cs 1756..1902 段）——
    ("记忆通过内核 dsh-mcp-client 挂载 MCP 参考记忆服务器（@modelcontextprotocol/server-memory），模型可跨会话写入与召回信息。开关编辑内核 profile patch 的 disabled 标志，由内核热重载即时生效。", "Memory is provided by the kernel's dsh-mcp-client mounting the MCP Reference Memory server (@modelcontextprotocol/server-memory), so the model can write and recall information across sessions. The toggle edits the disabled flag in the kernel profile patch; the kernel hot-reloads it on the fly."),
    ("持久记忆", "Persistent memory"),
    ("启用持久记忆", "Enable persistent memory"),
    ("关闭后模型不再写入或读取记忆；已有记忆文件保留不动。", "When off, the model no longer writes or reads memory; the existing memory file is left untouched."),
    ("已启用（内核热重载后生效）", "Enabled (applies via kernel hot reload)"),
    ("记忆存储", "Memory storage"),
    ("知识图谱 JSONL（由记忆服务器自身管理，删除即清空记忆）：", "Knowledge-graph JSONL (managed by the memory server itself; deleting it clears all memory):"),
    ("默认插件", "Bundled plugins"),
    ("参考记忆服务器", "Reference memory server"),
    ("技能面板", "Skill panel"),
    ("Blade² 随内核插件机制默认启用；安装由引导器幂等完成，失败时下次启动自动重试。", "Enabled by default via the kernel plugin mechanism; the bootstrapper installs idempotently and retries on the next launch after a failure."),
    ("默认停用（需先安装 Cua Driver）", "Disabled by default (install Cua Driver first)"),
    ("已安装，未挂载", "Installed, not mounted"),
    ("未安装（离线？启动后自动重试）", "Not installed (offline? retried on next launch)"),
    ("有未保存的修改…", "Unsaved changes…"),
    ("已自动保存 · {0}", "Auto-saved · {0}"),
    ("第 {0} 行不是合法 JSON，已阻止保存。", "Line {0} is not valid JSON; save blocked."),
    ("电脑控制", "Computer control"),
    ("浏览器控制", "Browser control"),
    ("电脑控制由内核 dsh-computer-use 注册表与 Cua Driver provider（@deepseek-ai/dsh-experimental-computer-use-cua-driver-mcp）提供：模型可截图并操作鼠标、键盘完成桌面任务。开关编辑内核 profile patch 的 disabled 标志，由内核热重载即时生效。", "Computer control is provided by the kernel's dsh-computer-use registry and the Cua Driver provider (@deepseek-ai/dsh-experimental-computer-use-cua-driver-mcp): the model can take screenshots and operate the mouse and keyboard for desktop tasks. The toggles edit the disabled flag in the kernel profile patch; the kernel hot-reloads it on the fly."),
    ("开启后模型获得桌面操作能力（截图、鼠标、键盘）；关闭后相关工具从模型视野移除。", "When on, the model gains desktop control (screenshots, mouse, keyboard); when off, the related tools are removed from the model's view."),
    ("Cua Driver（桌面驱动）", "Cua Driver (desktop driver)"),
    ("电脑操作的执行驱动：需在本机安装 cua-driver 命令行工具后启用，默认停用。", "The driver that executes computer control: enable it after installing the cua-driver command-line tool locally. Disabled by default."),
    ("启用电脑控制", "Enable computer control"),
    ("启用 Cua Driver 桌面驱动", "Enable the Cua Driver desktop driver"),
    ("浏览器控制由内核 dsh-browser-use 注册表与 Playwright provider（@deepseek-ai/dsh-experimental-browser-use-playwright-mcp）提供：模型可打开浏览器完成网页任务。开关编辑内核 profile patch 的 disabled 标志，由内核热重载即时生效。", "Browser control is provided by the kernel's dsh-browser-use registry and the Playwright provider (@deepseek-ai/dsh-experimental-browser-use-playwright-mcp): the model can open a browser for web tasks. The toggles edit the disabled flag in the kernel profile patch; the kernel hot-reloads it on the fly."),
    ("开启后模型获得浏览器操作能力（打开网页、点击、填写、读取内容）；关闭后相关工具从模型视野移除。", "When on, the model gains browser control (open pages, click, fill in, read content); when off, the related tools are removed from the model's view."),
    ("Playwright（浏览器驱动）", "Playwright (browser driver)"),
    ("浏览器操作的执行驱动：随包安装，launch 模式无头运行，默认启用。", "The driver that executes browser control: installed with the package, launched headless. Enabled by default."),
    ("启用浏览器控制", "Enable browser control"),
    ("启用 Playwright 浏览器驱动", "Enable the Playwright browser driver"),
    ("挂载状态", "Mount status"),
    ("电脑控制注册表", "Computer-use registry"),
    ("Cua Driver 驱动", "Cua Driver provider"),
    ("浏览器控制注册表", "Browser-use registry"),
    ("Playwright 驱动", "Playwright provider"),
    ("自动审批由内核插件 dsh-approval-gate 提供：当会话权限预设切到「自动审批」时，由判定模型预判每次写入/命令是否不可回补——安全则自动批准，涉及删除、凭据、系统配置等硬类别转人工确认。开关增删内核 profile patch 里的 auto-approve 预设，判定模型写在默认模型设置里，两者都由内核热重载即时生效。", "Auto approval is provided by the kernel plugin dsh-approval-gate: when a session's permission preset is switched to Auto approval, the judging model pre-judges whether each write/command is irreversible — safe ones are approved automatically, while hard categories such as deletion, credentials, and system configuration are escalated to a human. The toggle adds or removes the auto-approve preset in the kernel profile patch and the judging model lives in the default-model setting; both are hot-reloaded by the kernel on the fly."),
    ("开启后权限模式里多出「自动审批」：判定模型预判越界请求，安全自动批准、有风险转人工。", "When on, an Auto approval option appears in the permission presets: the judging model pre-judges out-of-bounds requests, approving safe ones automatically and escalating risky ones to a human."),
    ("启用自动审批", "Enable auto approval"),
    ("自动审批判定模型", "Auto approval judging model"),
    ("判定模型", "Judging model"),
    ("自动审批每次预判写入/命令是否不可回补时所用的模型，选项来自内核模型目录。", "The model that pre-judges whether each write/command is irreversible during auto approval; the options come from the kernel model catalog."),
    ("预判模型", "Pre-judgment model"),
    ("切换后新会话的默认模型也随之改变；已在会话里选过模型的会话不受影响。", "Switching also changes the default model for new sessions; sessions that already picked a model are unaffected."),
    ("模型目录不可用：{0}", "Model catalog unavailable: {0}"),
    ("切换后会发生什么", "What happens when you switch"),
    ("开启：权限选择器（输入区左下角）里出现「自动审批」，新会话可选用它。\n关闭：该预设从选择器消失；正在使用它的会话回落到自定义权限，审批恢复人工确认。\n判定记录与学习状态不受影响，重新开启后继续生效。", "On: an Auto approval option appears in the permission picker (bottom-left of the composer), available to new sessions.\nOff: that preset disappears from the picker; sessions using it fall back to custom permissions, and approval returns to manual confirmation.\nDecision logs and learned state are unaffected and resume when you switch it back on."),
    ("关于", "About"),
    ("Blade² 的版本信息与更新。更新通过覆盖安装新版本 MSIX 完成（安装前需先退出应用）。", "Blade² version info and updates. Updates are applied by installing a newer MSIX over the current one (quit the app before installing)."),
    ("版本", "Version"),
    ("当前版本", "Current version"),
    ("壳版本（打包形态与安装包版本一致）", "Shell version (matches the package version when packaged)"),
    ("内核版本", "Kernel version"),
    ("随包发行的 dsh 内核版本", "Bundled dsh kernel version"),
    // 主干 `MainWindow.xaml.cs:1837-1848`「内核加载」里程碑一族（last-wins 生效值逐字照抄）：
    // `失败` 主干在 `:1609`/`:1847` 登记过两遍、值同，本表按成规只留一枚（已在前文）。
    ("内核加载", "Kernel boot"),
    ("本次启动内核引导里程碑（阶段 · 累计耗时）", "Boot milestones of this launch (stage · elapsed)"),
    ("默认插件就绪", "Default plugins ready"),
    ("默认插件安装", "Installing default plugins"),
    ("默认插件就绪（检查失败，裸启动）", "Default plugins ready (check failed, bare boot)"),
    ("认证完成", "Authenticated"),
    ("事件通道就绪", "Event channel ready"),
    ("工作区清单就绪", "Workspace list ready"),
    ("会话与模型就绪", "Sessions & models ready"),
    ("引导完成", "Boot complete"),
    ("尚未开始", "Not started yet"),
    ("更新", "Updates"),
    ("更新源尚未配置：发布到 GitHub 后在 MainWindow.About.cs 填入仓库地址即可启用在线检查更新。", "Update source not configured yet: fill in the repo address in MainWindow.About.cs after publishing to GitHub."),
    ("尚未配置更新源。", "Update source not configured yet."),
    ("检查更新", "Check for updates"),
    ("正在检查更新…", "Checking for updates…"),
    ("已是最新版本。", "You're up to date."),
    ("发现新版本：{0}。", "New version available: {0}."),
    ("发现新版本：{0}（约 {1}）。", "New version available: {0} (about {1})."),
    ("检查更新失败：{0}", "Update check failed: {0}"),
    ("检查更新失败：Release 返回缺少 tag_name。", "Update check failed: the release response has no tag_name."),
    ("检查更新失败：未找到可用的安装包。", "Update check failed: no installable package found."),
    ("下载并安装", "Download and install"),
    ("下载并安装 {0}", "Download and install {0}"),
    ("打开 Release 页面", "Open the Releases page"),
    ("正在下载更新包：{0}…", "Downloading update package: {0}…"),
    ("已下载：{0}", "Downloaded: {0}"),
    ("下载更新失败：{0}", "Update download failed: {0}"),
    ("安装更新", "Install update"),
    ("将退出 Blade²（含内置内核）并覆盖安装新版本。继续吗？", "Blade² (including the bundled kernel) will quit and the new version will be installed over it. Continue?"),
    ("退出并安装", "Quit and install"),
    ("稍后", "Later"),
    ("GitHub Release", "GitHub Releases"),
    ("未知", "Unknown"),
    ("托盘与退出", "Tray & exit"),
    ("最小化 / 关闭窗口时的去向", "Where the window goes when minimized or closed"),
    ("显示托盘图标", "Show tray icon"),
    ("关闭后仍可从托盘恢复窗口；托盘右键菜单可退出", "Restore the window from the tray after hiding; quit from the tray menu"),
    ("最小化时隐藏到托盘", "Minimize to tray"),
    ("点最小化按钮后窗口藏进托盘，不占任务栏", "Hides into the tray on minimize, freeing the taskbar"),
    ("关闭时最小化到托盘", "Close to tray"),
    ("点关闭按钮后窗口藏进托盘继续运行，用托盘菜单退出", "Keeps running in the tray on close; quit from the tray menu"),
    ("系统通知", "System notifications"),
    ("任务完成、审批请求等关键时刻的 Windows 通知", "Windows notifications for task completion, approval requests and other key moments"),
    ("启用系统通知", "Enable system notifications"),
    ("窗口不在前台时提醒；点击通知可回到 Blade²", "Notifies while the window is in the background; click a notification to bring Blade² back"),
    ("任务完成", "Task completed"),
    ("本轮对话已完成", "This turn has finished"),
    ("需要你的输入", "Your input needed"),
    ("后台作业", "Background jobs"),
    ("子代理", "Subagent"),
    ("子代理会话为只读：可查看其工作过程，消息请在父会话中发送。", "Subagent sessions are read-only: you can watch the work, but send messages in the parent session."),
    ("返回父会话", "Back to parent session"),
    ("子代理目录", "Subagent catalog"),
    ("续跑", "Continue"),
    ("打断", "Interrupt"),
    ("可继续", "Continuable"),
    // —— 任务 #94 批 6/6：i94-keys-missing 数据行 401..492（主干 MainWindow.xaml.cs 1903..2009 段）——
    ("正在运行", "Running"),
    ("当前未运行", "Not running"),
    ("正在加载子代理…", "Loading subagents…"),
    ("当前会话没有子代理", "This session has no subagents"),
    ("无法加载子代理：{0}", "Unable to load subagents: {0}"),
    ("会话记录损坏", "Corrupted session record"),
    ("子代理记录版本不受支持", "Unsupported subagent record version"),
    ("会话记录暂不可用", "Session record temporarily unavailable"),
    ("续跑消息", "Follow-up message"),
    ("续跑：{0}", "Continue: {0}"),
    ("续跑失败：{0}", "Continue failed: {0}"),
    ("打断子代理", "Interrupt subagent"),
    ("确认打断子代理「{0}」的当前运行？", "Interrupt the current run of subagent \"{0}\"?"),
    ("打断失败：{0}", "Interrupt failed: {0}"),
    ("打开子代理会话 {0}", "Open subagent session {0}"),
    ("续跑子代理 {0}", "Continue subagent {0}"),
    ("打断子代理 {0}", "Interrupt subagent {0}"),
    ("展开 {0} 的下级子代理", "Expand descendants of {0}"),
    ("父会话", "Parent session"),
    ("展开", "Expand"),
    ("一次性", "One-time"),
    ("每 {0} 天", "Every {0} days"),
    ("每 {0} 小时", "Every {0} hours"),
    ("每 {0} 分", "Every {0} minutes"),
    ("每 {0} 秒", "Every {0} seconds"),
    ("尚未收到本会话的计划投影（投影随会话活动到达），此时不表示没有计划。", "The schedule projection has not arrived yet (the kernel pushes it lazily); this does not mean there are no schedules."),
    ("还剩 {0}", "{0} left"),
    ("已过期 {0}", "{0} overdue"),
    ("{0}天", "{0}d"),
    ("{0}小时", "{0}h"),
    ("{0}分钟", "{0}m"),
    ("文件操作", "File actions"),
    ("更多文件操作", "More file actions"),
    ("用默认应用打开", "Open in default app"),
    ("打开所在文件夹", "Open containing folder"),
    ("在侧边栏预览", "Preview in sidebar"),
    ("在文件资源管理器中显示", "Show in File Explorer"),
    ("调整侧栏宽度", "Resize sidebar"),
    ("调整右栏宽度", "Resize right pane"),
    ("调整分栏比例", "Resize split"),
    ("右栏标签栏", "Right pane tabs"),
    ("右栏分栏宿主", "Right pane dock"),
    ("右栏第一格", "Right pane cell 1"),
    ("右栏第二格", "Right pane cell 2"),
    ("右栏第二格文件面板", "Right pane file panel"),
    ("新标签页", "New tab"),
    ("左右分栏", "Split left/right"),
    ("上下分栏", "Split top/bottom"),
    ("分栏已满（最多两格）", "Split full (two cells max)"),
    ("移到这里", "Move here"),
    ("全屏", "Fullscreen"),
    ("退出全屏", "Exit fullscreen"),
    ("文件 {0}", "File {0}"),
    ("查看", "View"),
    ("组装（agent.cordis.yml）", "Composition (agent.cordis.yml)"),
    ("组装（agent.cordis.yml）· {0}", "Composition (agent.cordis.yml) · {0}"),
    ("查看预设 {0} 的组装", "View composition of preset {0}"),
    ("无法读取预设组装：{0}", "Unable to read preset composition: {0}"),
    ("用「创造模式」创作自定义预设", "Draft a custom preset with Creator mode"),
    ("切到创造模式并开始新会话，让 Agent 帮你创建自定义预设。", "Switch to Creator mode and start a new session so the agent can draft a custom preset for you."),
    ("已进入创造模式。告诉 Agent 你想要的工具、提示词与能力，它会帮你写出自定义预设的 agent.cordis.yml。", "Creator mode is ready. Tell the agent which tools, prompts and capabilities you want; it will draft your custom agent.cordis.yml."),
    ("无法开始创造模式：{0}", "Unable to start Creator mode: {0}"),
    ("内测声明", "Internal Testing Notice"),
    ("继续", "Continue"),
    ("DeepSeek Harness 目前的 0.1 版本仍处在面向 Harness 开发者进行测试的阶段，还有许多地方需要持续改进和打磨，希望听取广大开发者的反馈建议。预计 DeepSeek Harness 的核心插件以及基础 API 都会在接下来的一段时间内快速迭代、持续演化。\n\n我们期待与全球开发者一起，在开源、开放、可复用、可组合的基础设施之上，共同探索智能上限。欢迎全球 Harness 开发者加入 DSH 插件生态。", "DeepSeek Harness 0.1 remains in testing for Harness developers. Many areas need further improvement, and we welcome feedback from the developer community. DeepSeek Harness's core plugins and foundational APIs will continue to evolve rapidly over the coming months.\n\nWe look forward to exploring the limits of intelligence with developers around the world, building on open-source, open, reusable, and composable infrastructure. We welcome Harness developers everywhere to join the DSH plugin ecosystem."),
    ("暂时无法保存确认状态，请重试。", "The acknowledgement could not be saved. Please try again."),
    ("添加一个 API Key 开始使用", "Add an API key to get started"),
    ("配置 DeepSeek 官方模型，即可开始使用。", "Configure the official DeepSeek provider to start building."),
    ("稍后配置", "Configure later"),
    ("保存并继续", "Save and continue"),
    ("请输入 API 密钥。", "Enter an API key."),
    ("连接状态", "Connection status"),
    ("连接成功", "Connected"),
    ("连接异常", "Disconnected"),
    ("自动重连中", "Reconnecting"),
    ("立即重连", "Reconnect now"),
    ("连接异常，点击立即重连", "Disconnected, reconnect now"),
    ("连接中断，正在自动重试，点击立即重连", "Reconnecting automatically, reconnect now"),
    ("即时生效", "Applies live"),
    ("需重启生效", "Applies after restart"),
    ("保存后即时生效", "Takes effect immediately after save"),
    ("保存后需重启内核才完全生效", "Needs a kernel restart to fully apply"),
    ("点开技能说明", "Open skill details"),
    ("技能 {0}（已被遮蔽，不生效）", "Skill {0} (shadowed, inactive)"),
    ("没有找到技能「{0}」。", "Skill \"{0}\" was not found."),
    ("（这个技能没有指令正文。）", "(this skill has no instruction body.)"),
    ("输入框键入 /名称 可调用；模型也可能自行调用。", "Type /name in the input box to invoke it; the model may also call it on its own."),
    ("指令", "Instructions"),
    ("打开技能说明失败：{0}", "Unable to open skill details: {0}"),
    ("Agent 可选择的模型", "Models agents may choose"),
    ("当前没有模型提供方公布模型。", "No model provider currently advertises a model."),
    ("保存前请至少选择一个模型。", "Select at least one model before saving."),
];

#[derive(Clone)]
pub struct Catalog {
    locale: String,
    table: HashMap<String, String>,
}

impl Catalog {
    /// locale 为 zh 时直接返回中文键，与主线 L(key) 一致；其它 locale 读 Assets/i18n/<locale>.json。
    pub fn load(locale: &str, assets_dir: Option<&Path>) -> Self {
        let mut table = HashMap::new();
        if locale != "zh" && locale != "en" {
            if let Some(dir) = assets_dir {
                if let Ok(text) = std::fs::read_to_string(dir.join(format!("{locale}.json"))) {
                    if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&text) {
                        for (key, value) in map {
                            if let Value::String(value) = value {
                                table.insert(key, value);
                            }
                        }
                    }
                }
            }
        }
        Self {
            locale: locale.to_string(),
            table,
        }
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// 回落顺序与主干 `L(key)`（`MainWindow.xaml.cs:2011-2017`）逐语义一致，四档：
    /// `zh` 直通 → 本语种 json 表 → 英文 `EN` 表 → 中文键。
    ///
    /// 第三档**对所有非 zh 语种生效**（主干是 `ShellEnglish.GetValueOrDefault(key, key)`，
    /// 不带 `if (locale == "en")` 的门）。分叉原先只在 `locale == "en"` 时查 EN，而
    /// `Assets/i18n/*.json` 那 28 份（各约 722 键）覆盖不到 EN 的约 1185 行，差集约 463 键
    /// ⇒ 德语等非中非英语种上这 463 处主干出英文、分叉露中文。`lf()` 走本函数，无需另立回落。
    pub fn l(&self, key: &str) -> String {
        if self.locale == "zh" {
            return key.to_string();
        }
        if let Some(value) = self.table.get(key) {
            return value.clone();
        }
        EN.iter()
            .find(|(from, _)| *from == key)
            .map(|(_, to)| (*to).to_string())
            .unwrap_or_else(|| key.to_string())
    }

    /// 主干 `DetText(zh, en)`（`MainWindow.MessageDetails.cs:26-31`）的分叉等价：**调用点
    /// 自己带一枚英文兜底**的查法。档位序 `zh 直通 → 本语种 json 表 → 显式 EN`。
    ///
    /// 与 `l()` 的差别只有第三档：`l` 的第三档是通用 `EN` 表，这里换成调用点给的 `en`。
    /// 之所以必须换：主干 `ShellEnglish` 是 `Dictionary` 的 indexer 赋值 = **后写覆盖**，
    /// 同一枚 `["显示"]` 写了 `:1593 Reveal` 与 `:1687 Display` 两行，生效值是后写的
    /// `Display`；分叉的 `EN` 是 `Iterator::find` = **first-wins**（见 `l`），同键补第二条
    /// 盖不住，而本文件的 `en_table_has_no_duplicate_keys` 又明令禁止重复键 ⇒ 只能由调用点钉。
    ///
    /// 逐语种与主干等价（主干 `L` 见 `MainWindow.xaml.cs:2011-2017`，其
    /// `LoadLocaleTable:542` 对 `zh`/`en` 直接返回 `ShellEnglish`）：
    /// · `zh` ⇒ 两档都不进，原样回 `zh`（= 主干 `L` 的第一档）。
    /// · `en` ⇒ 分叉 `load()` 对 `en` 不读 json（见 `load`），本语种表空 ⇒ 落到显式 `en`；
    ///   主干走 `ShellEnglish` 的 last-wins 生效值 ⇒ 两侧同一枚字面串，故 `en` 参必须抄
    ///   **主干的生效值**而不是它任一行的字面值。
    /// · 其它语种 ⇒ json 表命中就赢（= 主干同档先赢，例：`de.json` 的 `显示` 出德文），
    ///   json 表缺键才落显式 `en`（= 主干落 `ShellEnglish` 兜底）。
    ///
    /// 用错方向的代价：`en` 参传了主干**被盖掉**的那行（如 `Reveal`）就会译反，所以调用点
    /// 要传生效值。除 `l` 之外的第二把查法，别拿它替代 `l` 的普通用途。
    pub fn dt(&self, zh: &str, en: &str) -> String {
        if self.locale == "zh" {
            return zh.to_string();
        }
        if let Some(value) = self.table.get(zh) {
            return value.clone();
        }
        en.to_string()
    }

    /// 主干 `DetFormat(zhTemplate, enTemplate, args)`（`MainWindow.MessageDetails.cs:33-38`）的
    /// 分叉等价：先按 [`Catalog::dt`] 选档，再由这里替 `{n}`。**回落语义与 `dt` 逐字相同**
    /// （`zh` 直通 → 本语种 json 表 → 调用点显式英文档），本函数只多「填洞」这一步。
    ///
    /// 为什么不走 [`Catalog::lf`]（那把现成的查表 + 填洞）：`lf` 的第三档是**通用 EN 表**，而
    /// 主干那几条带参数的文案压根没进 `ShellEnglish`（`MessageActions.cs:260` /
    /// `MessageDetails.cs:1443,1543` 都是把英文写在调用点第二参），给 `lf` 传
    /// `来自会话 {0}` 等于查一张主干没有的表、非中文界面露中文。
    /// `lf` 那套 `{0:fmt}` 格式说明符这里用不上：主干 `DetFormat` 的调用点只填裸 `{n}`。
    ///
    /// 与主干的另一处一致：`string.Format` 模板里没有那个洞时**不报错**，多余实参被忽略；
    /// 这里同样只遍历实参做替换，缺洞就是原样返回（主干 `catch (FormatException)` 那一支
    /// 返回的是 zh 模板，本函数拿 `dt` 选完档再替换，正常模板下两侧同结果）。
    pub fn dtf(&self, zh: &str, en: &str, args: &[String]) -> String {
        let mut text = self.dt(zh, en);
        for (index, value) in args.iter().enumerate() {
            text = text.replace(&format!("{{{index}}}"), value);
        }
        text
    }

    /// 主干「内联双语」通道 `MainWindow.Trajectory.cs::TrajText` 与
    /// `MainWindow.MessageDetails.cs::DetText`（两枚函数体逐字符相同；`CapabilityText` /
    /// `CordisText` 是 `TrajText` 的纯转发）的分叉等价：**调用点自带英文兜底，但字典先赢**。
    /// 逐行镜像主干那三行 —— `l(zh)` 跑完，只有「`l` 把中文键原样吐回来」时才轮到 locale 门。
    ///
    /// 与 [`Catalog::dt`] 的**三处**差别（`dt` 的语义一行没动：它 bin 侧 26 + 13 处调用点在用，
    /// 动它等于改 `main.rs` 的批次）：
    /// 1. **档位序**：`bt` = [`Catalog::l`] 的全部四档（`zh` 直通 → 本语种 json → `EN` 表 →
    ///    原样中文）跑完才轮到兜底；`dt` 跳过 `EN` 表那一档。⇒ 主干 `ShellEnglish` **已收录**
    ///    的那批内联双语串上，`bt` 出**字典生效值**、调用点的 `en` 是死码（这是主干现状：
    ///    `TrajText` 第一行就是 `var s = L(zh);`，照抄，不「修正」成内联优先）。代价反过来：
    ///    想让内联英文赢，必须先撤 `EN` 里的那枚键，而撤表要 `main.rs` 的调用点同批改。
    /// 2. **locale 门**：`dt` 的直通门是 `locale == "zh"` 精确相等；`bt` 的最后一档是
    ///    **`zh` 前缀门**（`eq_ignore_ascii_case`，镜像主干
    ///    `_shellLocale.StartsWith("zh", OrdinalIgnoreCase)`）⇒ `zh-TW` / `zh-HK` 的未收录键
    ///    落**简体**，而 `dt` 落英文。代价：两把门在 `zh-` 系语种上给的串不同。
    /// 3. **带参形态的失败口径与说明符**：见 [`Catalog::btf`] —— `dtf` 缺洞原样留着、只认裸
    ///    `{n}`；`btf` 缺洞返回**中文模板本体**且认 `{n:格式}`。
    ///
    /// 空串两半照主干，**不许**加仁慈分支：`en` 为空且走到第三档就是空串，不回落 `zh`；
    /// `zh` 为空时只有「locale 过不了 `zh` 前缀门」那条出路（`zh` / `zh-TW` 档下主干 `L("")`
    /// 第一档就返回空串 ⇒ 本函数也返回空串）。
    pub fn bt(&self, zh: &str, en: &str) -> String {
        let s = self.l(zh);
        if s != zh {
            return s;
        }
        if self
            .locale
            .get(..2)
            .is_some_and(|head| head.eq_ignore_ascii_case("zh"))
        {
            zh.to_string()
        } else {
            en.to_string()
        }
    }

    /// 主干 `TrajFormat`（`MainWindow.Trajectory.cs`）/ `DetFormat`
    /// （`MainWindow.MessageDetails.cs`）的分叉等价：先按 [`Catalog::bt`] 选档，再填洞。
    /// 签名是 `(zh_template, en_template, args)` —— **两枚都是模板**，与主干同形。
    ///
    /// 三件必须照主干、不许「更聪明」的事：
    /// · **回落方向**：主干 `catch (FormatException) { return zhTemplate; }` ⇒ 实参少给时
    ///   **即使在英文界面也返回中文模板本体**（不是 `en_template`，更不是选完档那串）。本函数
    ///   按**选中的那串**里出现的最大洞号判，越界即回 `zh_template`；多余实参按 .NET `string.Format`
    ///   的口径静默忽略。现测主干 41 枚 Format 点的洞数与实参数**全部匹配** ⇒ 这一支今天打不到，
    ///   留着是为了「少给实参 = 吐中文模板」这条口径本身。
    /// · **说明符**：`{0:N0}` 是全族唯一带格式说明符的洞（`Trajectory.cs` 的毫秒行
    ///   `TrajFormat("{0:N0} 毫秒", "{0:N0} ms", ms)`），照 `dtf` 那种只替换裸 `{n}` 的写法会
    ///   **静默漏**成字面 `{0:N0}`。这里复用 [`Catalog::lf`] 底下那套认 `{n:fmt}` 的填洞
    ///   （`Self::fill_slots`），不写第三份。
    /// · **谁负责格式化**：`args` 是 `&[String]`，说明符被跳过、实参原样插入 ⇒ 千分位之类的
    ///   分组**必须由调用点自己格式化好**。注意主干自身两档口径不一致：`string.Format` 走**当前
    ///   区域**分隔符，而同族 `TokensN0`（`MainWindow.RunStats.cs`）显式 `InvariantCulture`
    ///   ⇒ 分叉建议统一按 Invariant 传串，风险注记在这里。
    ///
    /// 与主干的两处**已知不复制**：主干非法占位符（裸 `{` / `}`、`{abc}`）与不可格式化的实参也会
    /// 抛 `FormatException` ⇒ 吐中文模板；本函数按分叉既有 `lf`/`dtf` 的口径把这类写法**原样留着**
    /// （分叉全族模板都是自家字面串，现测无此类写法）。`{{` 转义同理不处理。
    pub fn btf(&self, zh_template: &str, en_template: &str, args: &[String]) -> String {
        let text = self.bt(zh_template, en_template);
        if Self::max_slot(&text).is_some_and(|largest| largest >= args.len()) {
            return zh_template.to_string();
        }
        Self::fill_slots(&text, args)
    }

    /// 模板里出现过的最大洞号（`{n}` 与 `{n:fmt}` 都算）；一枚洞都没有 ⇒ `None`。
    fn max_slot(template: &str) -> Option<usize> {
        let mut largest: Option<usize> = None;
        let mut rest = template;
        while let Some(open) = rest.find('{') {
            let after = &rest[open + 1..];
            let Some(close) = after.find('}') else { break };
            if let Ok(value) = after[..close]
                .split(':')
                .next()
                .unwrap_or("")
                .parse::<usize>()
            {
                largest = Some(largest.map_or(value, |current| current.max(value)));
            }
            rest = &after[close..];
        }
        largest
    }

    pub fn lf(&self, template: &str, args: &[String]) -> String {
        Self::fill_slots(&self.l(template), args)
    }

    /// 填洞本体（`lf` 与 `btf` 共用这一份，别再写第三套）：`{n}` 与 `{n:说明符}` 都按第 `n`
    /// 枚实参替换，说明符本身被忽略；越界或写法非法的洞原样留在串里；替换只走一遍，实参自带的
    /// 花括号不再二次展开。
    fn fill_slots(text: &str, args: &[String]) -> String {
        let mut out = String::with_capacity(text.len());
        let bytes = text.as_bytes();
        let mut index = 0usize;
        while index < bytes.len() {
            if bytes[index] == b'{' {
                if let Some(end) = text[index..].find('}') {
                    if let Some(colon) = text[index..index + end].find(':') {
                        let slot = text[index + 1..index + colon].parse::<usize>();
                        if let Ok(slot) = slot {
                            if let Some(value) = args.get(slot) {
                                out.push_str(value);
                            }
                            index += end + 1;
                            continue;
                        }
                    }
                    let slot = text[index + 1..index + end].parse::<usize>();
                    if let Ok(slot) = slot {
                        if let Some(value) = args.get(slot) {
                            out.push_str(value);
                        }
                        index += end + 1;
                        continue;
                    }
                }
            }
            let ch_len = text[index..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
            out.push_str(&text[index..index + ch_len]);
            index += ch_len;
        }
        out
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn relative_time(updated_at_ms: i64, catalog: &Catalog) -> String {
    if updated_at_ms <= 0 {
        return String::new();
    }
    let minutes = (now_ms() - updated_at_ms).max(0) / 60_000;
    match minutes {
        0..=0 => catalog.l("刚刚"),
        1..=59 => catalog.lf("{0}分钟前", &[minutes.to_string()]),
        60..=1439 => catalog.lf("{0}小时前", &[(minutes / 60).to_string()]),
        1440..=43199 => catalog.lf("{0}天前", &[(minutes / 1440).to_string()]),
        43200..=525599 => catalog.lf("{0}个月前", &[(minutes / 43200).to_string()]),
        _ => catalog.lf("{0}年前", &[(minutes / 525600).to_string()]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zh_returns_the_key() {
        let catalog = Catalog::load("zh", None);
        assert_eq!(catalog.l("新会话"), "新会话");
    }

    #[test]
    fn en_falls_back_to_the_built_in_table_then_the_key() {
        let catalog = Catalog::load("en", None);
        assert_eq!(catalog.l("等待审批"), "Awaiting approval");
        assert_eq!(catalog.l("没有这个键"), "没有这个键");
    }

    /// #62：设置页下拉的选项标签必须走 EN 表，EN 值逐字照主干 `ShellEnglish:687-706`。
    /// `Mica` / `Mica Alt` 主干自己也登记了恒等映射，这里一起钉住，防止有人把它们当
    /// 「已回落」删掉。
    #[test]
    fn settings_combo_option_labels_are_localised() {
        let catalog = Catalog::load("en", None);
        assert_eq!(catalog.l("Mica"), "Mica");
        assert_eq!(catalog.l("Mica Alt"), "Mica Alt");
        assert_eq!(catalog.l("亚克力"), "Acrylic");
        assert_eq!(catalog.l("无（纯色）"), "None (solid)");
        assert_eq!(catalog.l("半透明"), "Translucent");
        assert_eq!(catalog.l("跟随窗口材质"), "Follow window material");
        // 同一张卡的卡头 / 行标（主干 `RenderMaterialCard` 的 `L("窗口材质")` / `L("材质")`）
        assert_eq!(catalog.l("窗口材质"), "Window material");
        assert_eq!(catalog.l("材质"), "Material");
        assert_eq!(catalog.l("气泡材质"), "Bubble material");
        // zh 侧照旧原样返回，选项显示串与 `pick` 下标解耦，不会弹选。
        let zh = Catalog::load("zh", None);
        assert_eq!(zh.l("无（纯色）"), "无（纯色）");
    }

    /// #66：主干 `4a9452c..0b1b4e7` 之后搬过来的三份文案。
    #[test]
    fn drifted_settings_copy_matches_mainline() {
        let catalog = Catalog::load("en", None);
        assert_eq!(
            catalog.l("记忆内容"),
            "Memory contents"
        );
        assert_eq!(
            catalog.lf("共 {0} 行 · {1}", &["0".into(), catalog.l("停止输入后自动保存")]),
            "0 lines · Auto-saves when you stop typing"
        );
        assert_eq!(
            catalog.lf(
                "已自动保存 · {0} · {1} 行文本已转为记忆实体",
                &["12:34:56".into(), "3".into()]
            ),
            "Auto-saved · 12:34:56 · 3 line(s) turned into memory entities"
        );
        assert_eq!(
            catalog.l("petdex install 或 codex-pets add <宠物标识>"),
            "petdex install or codex-pets add <pet-slug>"
        );
        assert_eq!(
            catalog.lf("更新源：{0}", &["github.com/Live-Rise/Blade2".into()]),
            "Update source: github.com/Live-Rise/Blade2"
        );
    }

    #[test]
    fn interpolates_indexed_slots_and_ignores_format_specifiers() {
        let catalog = Catalog::load("zh", None);
        assert_eq!(
            catalog.lf("展开其余 {0} 个会话", &["7".into()]),
            "展开其余 7 个会话"
        );
        assert_eq!(catalog.lf("{0:0.#} 亿", &["1.5".into()]), "1.5 亿");
        assert_eq!(catalog.lf("{0} / {1}", &["a".into(), "b".into()]), "a / b");
        assert_eq!(catalog.lf("缺参数 {0}", &[]), "缺参数 ");
    }

    #[test]
    fn relative_time_buckets() {
        let catalog = Catalog::load("zh", None);
        let now = now_ms();
        assert_eq!(relative_time(now, &catalog), "刚刚");
        assert_eq!(relative_time(now - 5 * 60_000, &catalog), "5分钟前");
        assert_eq!(relative_time(now - 3 * 3_600_000, &catalog), "3小时前");
        assert_eq!(relative_time(now - 10 * 86_400_000, &catalog), "10天前");
        assert_eq!(relative_time(0, &catalog), "");
    }

    /// 结构性护栏：`EN` 数组内不得出现重复的中文键。
    ///
    /// 为什么要钉住：分叉查表走 `Iterator::find`（`Catalog::l`，见本文件 `1700` 行附近）
    /// = **首个命中生效**；主干 `ShellEnglish` 是 `Dictionary` 的 indexer 赋值
    /// （`MainWindow.xaml.cs:596` 起）= **后写覆盖**。两侧对「同一中文键写两条」的取舍
    /// 正好相反，表里只要潜伏重复键，抄主干的人就会把界面译反（`显示` 就是主干自己
    /// 一名两译的活例子：`1593` Reveal / `1687` Display）。
    ///
    /// 这里刻意**不改**查表行为（改成 last-wins 会牵动全部分叉文案），只把差异变成
    /// 结构约束：走数组长度 / 去重计数，不靠源码文本锁。
    #[test]
    fn en_table_has_no_duplicate_keys() {
        let mut keys: Vec<&str> = EN.iter().map(|(from, _)| *from).collect();
        let total = keys.len();
        assert!(total > 0, "EN 表被清空了？");
        keys.sort_unstable();
        let mut dups: Vec<&str> = Vec::new();
        for pair in keys.windows(2) {
            if pair[0] == pair[1] && !dups.contains(&pair[0]) {
                dups.push(pair[0]);
            }
        }
        keys.dedup();
        assert_eq!(
            keys.len(),
            total,
            "EN 表存在重复中文键 {dups:?}（分叉 first-wins、主干 last-wins，同键两条必译反）"
        );
    }

    /// 台账 #137 L2：`历史回读失败：{0}` 是**分叉自造**键 —— 主干没有这句文案
    /// （`grep -rnF '历史回读失败：{0}'` 扫 `*.cs` / `*.xaml` 零命中，`历史回读` 四字只出现在
    /// 注释里），所以它既不在主干 `ShellEnglish` 里、也就盖不到上面那道「主干 ⊆ EN」总闸；
    /// JD2 落改动 D 时新增两处调用（`main.rs` 的 `apply_history` 逐条留痕）却没人补键
    /// ⇒ 英文界面三处露中文。这里正向查一次表钉住：必须出英文，且措辞与同族两枚邻居对齐
    /// （`会话同步失败：{0}` = `Session sync failed: {0}`）。
    #[test]
    fn fork_only_history_read_failure_key_is_translated() {
        let en = Catalog::load("en", None);
        assert_eq!(
            en.l("历史回读失败：{0}"),
            "Session history read failed: {0}",
            "缺键回落到中文键本体 ⇒ 英文界面露中文"
        );
        // 三处调用点用的都是 `lf`：占位符必须仍填得进去（`lf` 内部走 `l`，这一发是端到端口径）。
        assert_eq!(
            en.lf("历史回读失败：{0}", &["journal seq 12".to_string()]),
            "Session history read failed: journal seq 12"
        );
        // zh 直通 = 主干 `L` 的第一档，别把中文界面也译了。
        assert_eq!(
            Catalog::load("zh", None).l("历史回读失败：{0}"),
            "历史回读失败：{0}"
        );
    }

    /// #95 两条译名的裁决，钉住防漂移（依据抄在断言旁边，别只写在报告里）。
    ///
    /// - `拒绝`：主干唯一条目 `MainWindow.xaml.cs:795` `["拒绝"] = "Decline"`，另有
    ///   `MainWindow.Cordis.cs:454` / `:848` 两处显式英文兜底同为 `Decline`；分叉原值
    ///   `Reject` 没有任何验收契约钉着（`rust/tests/gui_uia.ps1`、`rust/tmp/qa3_selftest.ps1`
    ///   两份脚本里 `Reject` / `Decline` / `ApprovalReject` 命中数均为 0）⇒ 改值对齐主干。
    /// - `显示`：分叉当前唯一的调用点是宠物设置卡头（`main.rs` 里 `"显示"` 那一处，过
    ///   `self.catalog.l`）。交付物卡原先那颗 reveal 文字钮已在 #74/fam1 换成 ⋯ `FileActionMenu`，
    ///   菜单里对应项的文案是「在文件资源管理器中显示」，不再读这个键 ⇒ 卡头是本表 `Reveal`
    ///   唯一消费者，而主干那一处的生效值是 `Display`。这一侧要同时满足得先给卡头另立键，
    ///   是 `main.rs` + 本表的活（任务 #96），不在本表里翻值。
    #[test]
    fn translated_names_match_mainline_rulings() {
        let catalog = Catalog::load("en", None);
        assert_eq!(catalog.l("拒绝"), "Decline");
        assert_eq!(catalog.l("显示"), "Reveal");
    }

    /// #92 左栏状态点族的四枚文案：值逐字抄主干 `ShellEnglish`——`MainWindow.xaml.cs:1618`
    /// `计划待审`、`:1619` `等待回答`、`:1620` `有活动定时任务`、`:793` `等待审批`。
    /// 点上的 tooltip 与 `AutomationProperties.Name` 都读这一枚键（主干 `SessionPendingLabel`
    /// + `L("有活动定时任务")`），ZH 下不翻译、原样回键。
    #[test]
    fn session_status_dot_labels_match_mainline_shell_english() {
        let en = Catalog::load("en", None);
        assert_eq!(en.l("计划待审"), "Plan awaiting review");
        assert_eq!(en.l("等待回答"), "Awaiting answer");
        assert_eq!(en.l("有活动定时任务"), "Has active scheduled task");
        assert_eq!(en.l("等待审批"), "Awaiting approval");
        let zh = Catalog::load("zh", None);
        assert_eq!(zh.l("计划待审"), "计划待审");
        assert_eq!(zh.l("有活动定时任务"), "有活动定时任务");
    }

    /// 缺键回落对齐主干 `L(key)`（`MainWindow.xaml.cs:2015-2016`
    /// `return ShellEnglish.GetValueOrDefault(key, key)`，**不带 locale == "en" 的门**）：
    /// 非中非英语种（`de`/`ja`…）缺键先回落到英文 EN 表，而不是直接回落到中文键。
    /// 分叉那 28 份 `Assets/i18n/*.json` 各约 722 键，覆盖不到 EN 的约 1185 行，
    /// 差集约 463 键 ⇒ 修复前德语界面上这 463 处主干出英文、分叉露中文。
    #[test]
    fn missing_key_falls_back_to_english_for_non_zh_non_en_locale() {
        // assets_dir = None ⇒ de 的本语种表为空，等价于「de.json 里没这个键」
        let de = Catalog::load("de", None);
        assert!(de.table.is_empty(), "de 不指向 assets 目录时本语种表应为空");
        assert_eq!(de.l("等待审批"), "Awaiting approval");
        assert_eq!(de.l("计划待审"), "Plan awaiting review");
        // EN 表里也没有的键才落到最后一档：原样返回中文键
        assert_eq!(de.l("没有这个键"), "没有这个键");
        // zh 那一档不受影响：仍然直通返回键本身
        assert_eq!(Catalog::load("zh", None).l("等待审批"), "等待审批");
    }

    /// `lf()` 内部就是调 `l()`，回落共用同一套：`de` 下模板键也要走英文 EN 表。
    #[test]
    fn lf_shares_the_english_fallback_for_non_zh_non_en_locale() {
        let de = Catalog::load("de", None);
        assert_eq!(de.lf("{0}分钟前", &["5".into()]), "5 min ago");
        assert_eq!(relative_time(now_ms() - 3 * 3_600_000, &de), "3 h ago");
    }

    /// 钉住四档顺序本身：本语种 json 表 > EN 表（EN 只当第三档，不得盖过本语种译文）。
    #[test]
    fn locale_table_wins_over_the_english_fallback() {
        let de = Catalog {
            locale: "de".to_string(),
            table: HashMap::from([(
                "等待审批".to_string(),
                "Ausstehende Genehmigung (nur Tests)".to_string(),
            )]),
        };
        assert_eq!(de.l("等待审批"), "Ausstehende Genehmigung (nur Tests)");
        // 表里没有的那枚键才继续落到 EN 表
        assert_eq!(de.l("计划待审"), "Plan awaiting review");
    }

    /// #96 剩余半刀：宠物卡头「显示」在 EN 下要出主干的**生效值** `Display`。
    ///
    /// 主干那一处是**单参** `L("显示")`（`MainWindow.Pet.cs:280`），不是 `DetText` 两参；
    /// EN 之所以是 `Display`，全靠 `ShellEnglish` 的重复键后写覆盖（`MainWindow.xaml.cs:1593`
    /// 的 `Reveal` 是被盖死的死行、`:1687` 的 `Display` 才生效）。分叉的 `EN` 表 first-wins
    /// 且本文件禁止重复键 ⇒ 改由调用点走 `dt` 钉住生效值。这里一次钉四件：
    /// ① `l` 那条 `Reveal` 行没被翻值（硬规矩）；② `dt` 的逐语种档位与主干 `L` 等价；
    /// ③ 本语种 json 表先赢，显式 EN 不得盖过它（主干 `LoadLocaleTable:542` 同档先赢）；
    /// ④ `settings_card` 内部对 header 还有一次 `l()` 二次查表 ⇒ 恒等，不会把译文再译一遍。
    #[test]
    fn pet_card_header_pins_the_mainline_effective_english() {
        assert_eq!(Catalog::load("zh", None).dt("显示", "Display"), "显示");
        assert_eq!(Catalog::load("en", None).dt("显示", "Display"), "Display");
        // ① EN 表那行原样保留：另有消费者，且不许在表里翻值
        assert_eq!(Catalog::load("en", None).l("显示"), "Reveal");
        // ③ json 表命中 ⇒ 本语种译文赢（值抄主干 `Assets/i18n/de.json` 的 `显示`）
        let de = Catalog {
            locale: "de".to_string(),
            table: HashMap::from([("显示".to_string(), "Anzeigen".to_string())]),
        };
        assert_eq!(de.dt("显示", "Display"), "Anzeigen");
        // json 表缺键才轮显式 EN（主干此时落 `ShellEnglish`，也是 `Display`）
        assert_eq!(Catalog::load("de", None).dt("显示", "Display"), "Display");
        // ④ 二次查表恒等
        let en = Catalog::load("en", None);
        assert_eq!(en.l(en.dt("显示", "Display").as_str()), "Display");
        assert_eq!(de.l(de.dt("显示", "Display").as_str()), "Anzeigen");
    }

    // ==================== #93f P1-3：`dtf` 的档位判据搬回 lib ====================
    //
    // `pub fn dtf` 挂在**本文件**（`Catalog::dtf`），w93e 那版却只在 bin 侧
    // （`main.rs::message_details_tests`）断了一遍 ⇒ lib 的测试计数一次没涨、`dt` 的档位序
    // 隔着一次 re-export 才看着。下面六条按 `tmp/w93e-audit.md` C2 的清单落地，逐条对主干
    // `DetFormat(zh, en, args)`（`MainWindow.MessageDetails.cs:33-38`）。

    /// 第一档：`zh` 直通（= 主干 `L` 的 `if (_shellLocale == "zh") return key;`），模板原样进、
    /// 洞照样填。
    #[test]
    fn dtf_returns_the_zh_template_verbatim_for_zh_and_still_fills_slots() {
        let zh = Catalog::load("zh", None);
        let one = |value: &str| vec![value.to_string()];
        assert_eq!(zh.dtf("来自会话 {0}", "From session {0}", &one("s-77")), "来自会话 s-77");
        assert_eq!(
            zh.dtf("跨会话中继 · {0} → {1}", "Relay · {0} → {1}", &vec!["a".into(), "b".into()]),
            "跨会话中继 · a → b"
        );
        // 多洞多参：每一枚 `{n}` 各填各的，且替换按实参序走（同洞重复出现也要都填上）。
        assert_eq!(
            zh.dtf("{0} 与 {1} 与 {0}", "{0} and {1} and {0}", &vec!["x".into(), "y".into()]),
            "x 与 y 与 x"
        );
    }

    /// 第三档：本语种 json 表**没有**那枚键时，吃调用点给的显式英文档（= 主干 `DetText` 的第二
    /// 参）。`Catalog::load` 对 `en` 不读 json ⇒ `en` 恒落这一档。
    #[test]
    fn dtf_falls_back_to_the_call_site_en_when_the_locale_table_misses() {
        let en = Catalog::load("en", None);
        let one = |value: &str| vec![value.to_string()];
        assert_eq!(
            en.dtf("来自会话 {0}", "From session {0}", &one("s-77")),
            "From session s-77"
        );
        // 非中非英语种同样落第三档（主干落 `ShellEnglish`，而这几条压根没进那张表）。
        let de = Catalog {
            locale: "de".to_string(),
            table: HashMap::new(),
        };
        assert_eq!(
            de.dtf("找不到会话 {0}", "Session {0} not found", &one("s-9")),
            "Session s-9 not found"
        );
    }

    /// 第二档：本语种 json 表**先赢**于调用点那枚显式英文档（与 `dt` 同一档位序 —— 这正是
    /// `dtf` 首行复用 `self.dt(...)` 的那一条，测在本模块里才隔不到 re-export）。
    #[test]
    fn dtf_prefers_the_locale_json_table_over_the_call_site_en() {
        let fr = Catalog {
            locale: "fr".to_string(),
            table: HashMap::from([("来自会话 {0}".to_string(), "Depuis la session {0}".to_string())]),
        };
        let one = |value: &str| vec![value.to_string()];
        assert_eq!(
            fr.dtf("来自会话 {0}", "From session {0}", &one("s-9")),
            "Depuis la session s-9",
            "本语种表没赢过调用点那枚英文 ⇒ 第二档被跳过了"
        );
        // 同一张表里缺的那枚键才继续落第三档（表命中判的是**键**，不是模板里的洞）。
        assert_eq!(
            fr.dtf("找不到会话 {0}", "Session {0} not found", &one("s-9")),
            "Session s-9 not found"
        );
    }

    /// 无洞 / 无参时 `dtf` 必须与 `dt` 同结果：这把函数只多「填洞」那一步，不许夹带后处理
    /// （trim、大小写、再把文案查一遍表都算）。
    #[test]
    fn dtf_without_placeholders_equals_dt() {
        for locale in ["zh", "en", "de"] {
            let catalog = Catalog::load(locale, None);
            assert_eq!(
                catalog.dtf("消息详情", "Message details", &[]),
                catalog.dt("消息详情", "Message details"),
                "locale {locale} 下无洞模板的 `dtf` 与 `dt` 分叉了"
            );
        }
        // 键进了 json 表时同样等价（第二档共用一条回落）。
        let fr = Catalog {
            locale: "fr".to_string(),
            table: HashMap::from([("元数据".to_string(), "Métadonnées".to_string())]),
        };
        assert_eq!(
            fr.dtf("元数据", "Metadata", &[]),
            fr.dt("元数据", "Metadata")
        );
    }

    /// 洞与实参**不匹配**的两半：多余实参被忽略、缺实参的洞原样留着（`dtf` 的文档
    /// `1749-1752` 承诺的就是这个行为，主干 `string.Format` 同口径）。
    #[test]
    fn dtf_ignores_extra_args_and_leaves_missing_holes_untouched() {
        let zh = Catalog::load("zh", None);
        // 前半：模板只有一洞，多喂的那枚不许生效、也不许报错。
        assert_eq!(
            zh.dtf("只有一洞 {0}", "one {0}", &vec!["x".into(), "y".into()]),
            "只有一洞 x"
        );
        // 后半（w93e 完全没测的那一半）：模板两洞、只喂一枚 ⇒ 另一洞原样留在串里。
        assert_eq!(
            zh.dtf("两洞 {0} 与 {1}", "two {0} and {1}", &["a".to_string()]),
            "两洞 a 与 {1}"
        );
        // 一洞都不喂 = 整串原样。
        assert_eq!(zh.dtf("两洞 {0} 与 {1}", "two", &[]), "两洞 {0} 与 {1}");
        // 实参自己带花括号不许再被填一遍（替换只走一遍，不递归展开）。
        assert_eq!(
            zh.dtf("一层 {0}", "one {0}", &["{1}".to_string()]),
            "一层 {1}"
        );
    }

    /// 键**确实进了** EN 表时，`dtf` 与既有 `lf` 必须给同一串 —— 这条锁住 `dtf` 文档
    /// （`1744-1747`）里「为什么主干那几条不直接用 `lf`」的选择：只有表里有键时两者才等价，
    /// 表里没键就必须走 `dtf` 的调用点英文档。
    #[test]
    fn dtf_and_lf_agree_when_the_key_is_in_the_en_table() {
        let one = |value: &str| vec![value.to_string()];
        // zh：`l` 的第一档直通、`dt` 的第一档直通 ⇒ 同串。
        let zh = Catalog::load("zh", None);
        assert_eq!(
            zh.dtf("{0}分钟前", "{0} min ago", &one("5")),
            zh.lf("{0}分钟前", &one("5"))
        );
        // en：EN 表命中（`l` 走第三档）= `dt` 的显式第二档（本语种表为空）。
        let en = Catalog::load("en", None);
        assert_eq!(
            en.dtf("{0}分钟前", "{0} min ago", &one("5")),
            en.lf("{0}分钟前", &one("5"))
        );
        assert_eq!(en.lf("{0}分钟前", &one("5")), "5 min ago");
        // 反面对照（这一句才是「主干那几条不许改用 `lf`」的证据）：键没进 EN 表时 `lf` 会露中文。
        assert_eq!(en.lf("来自会话 {0}", &one("s-7")), "来自会话 s-7");
        assert_eq!(
            en.dtf("来自会话 {0}", "From session {0}", &one("s-7")),
            "From session s-7"
        );
    }

    // ==================== IN3：主干「内联双语」通道的分叉镜像 `bt` / `btf` ====================
    //
    // 判据来自主干四枚实体 + 两枚转发：`Trajectory.cs` 的 `TrajText` / `TrajFormat`、
    // `MessageDetails.cs` 的 `DetText` / `DetFormat`（后两枚是**同体副本**，不是转发），
    // `Capabilities.cs` 的 `CapabilityText` 与 `Cordis.cs` 的 `CordisText`（两枚纯转发）。
    // 前五条零 IO；第八条运行时解析主干源码，主干缺席即跳过（跳过 ≠ 假绿，消息里写「跳过」）。

    /// 第一档/第三档的分界：调用点自带的那枚英文**只在 `ShellEnglish` 未收录的键上**出场。
    #[test]
    fn bt_returns_the_call_site_english_only_for_keys_shellenglish_misses() {
        let en = Catalog::load("en", None);
        // 未收录 ⇒ 前缀门为假 ⇒ 落调用点英文。
        assert_eq!(en.bt("轨迹面板", "Trajectory panel"), "Trajectory panel");
        assert_eq!(
            en.bt("内核未连接。", "Kernel is not connected."),
            "Kernel is not connected."
        );
        // 已收录 ⇒ `l()` 先赢，内联英文是死码（主干 `TrajText` 第一行就是 `var s = L(zh);`）。
        assert_eq!(en.bt("恢复", "Resume"), "Restore");
        // `zh` 档：`l` 的第一档直通，收录与否都原样。
        let zh = Catalog::load("zh", None);
        assert_eq!(zh.bt("轨迹面板", "Trajectory panel"), "轨迹面板");
        assert_eq!(zh.bt("恢复", "Resume"), "恢复");
        // 空串两半照主干：`en` 为空且走到第三档就是空串，**不**回落 `zh`；`zh` 为空时只有
        // 「locale 不走 `zh` 前缀门」这条出路，`zh` / `zh-TW` 档仍是空串（主干 `L("")` 第一档）。
        assert_eq!(en.bt("轨迹面板", ""), "");
        assert_eq!(en.bt("", "Anything"), "Anything");
        assert_eq!(zh.bt("", "Anything"), "");
        assert_eq!(Catalog::load("zh-TW", None).bt("", "Anything"), "");
    }

    /// 四档顺序（json 先赢 → EN 表 → `zh` 前缀门 → 调用点英文），并把与 [`Catalog::dt`]
    /// 给的**不同**串逐条对照（差别表的行为证据；`dt` 的既有语义一条没动）。
    #[test]
    fn bt_prefers_the_locale_json_table_then_the_en_table_then_the_prefix_gate() {
        // 第二档：本语种 json 先赢，连 `zh-` 前缀门也管不着它。
        let tw_json = Catalog {
            locale: "zh-TW".to_string(),
            table: HashMap::from([("事件账本".to_string(), "事件帳簿".to_string())]),
        };
        assert_eq!(tw_json.bt("事件账本", "Event ledger"), "事件帳簿");
        let fr = Catalog {
            locale: "fr".to_string(),
            table: HashMap::from([("轨迹面板".to_string(), "Panneau de trajectoire".to_string())]),
        };
        assert_eq!(
            fr.bt("轨迹面板", "Trajectory panel"),
            "Panneau de trajectoire",
            "第二档被跳过 ⇒ 本语种 json 没赢过内联英文"
        );
        // 第三档：EN 表（主干 `ShellEnglish` 的镜像）命中就赢过内联英文。`load` 对 `de` 不带
        // assets ⇒ 本语种表空，正好像干 json 缺该键时落 `ShellEnglish` 那一档。
        let de = Catalog::load("de", None);
        assert_eq!(de.bt("查看", "Inspect"), "View");
        // 对照 `dt`：它跳过 EN 档 ⇒ 同一枚调用点字面串给内联英文（那 39 个调用点的账，本轮不改）。
        assert_eq!(de.dt("查看", "Inspect"), "Inspect");
        // 第四档：`zh` **前缀**门（不是 `== "zh"`），且大小写不敏感。
        assert_eq!(
            Catalog::load("zh-TW", None).bt("类型筛选", "Kind filter"),
            "类型筛选",
            "zh- 前缀没落中文档"
        );
        assert_eq!(Catalog::load("zh-HK", None).bt("类型筛选", "Kind filter"), "类型筛选");
        assert_eq!(Catalog::load("ZH-TW", None).bt("类型筛选", "Kind filter"), "类型筛选");
        // `zh-TW` 上已收录的键 ⇒ 主干（json 缺、字典有）出英文 ⇒ 门来不及管。
        assert_eq!(Catalog::load("zh-TW", None).bt("恢复", "Resume"), "Restore");
        // 对照 `dt` 的精确门：`zh-TW` 下未收录键出英文。
        assert_eq!(Catalog::load("zh-TW", None).dt("类型筛选", "Kind filter"), "Kind filter");
    }

    /// 主干 `TrajFormat` / `DetFormat` 的失败口径：实参少给 ⇒ `catch (FormatException)` 返回
    /// **中文模板本体**（英文界面也吐中文）；多给 ⇒ .NET 静默忽略。判洞看的是**选中的那串**。
    #[test]
    fn btf_returns_the_zh_template_when_slots_are_missing_but_ignores_extra_args() {
        let zh = Catalog::load("zh", None);
        let en = Catalog::load("en", None);
        let one = |value: &str| vec![value.to_string()];
        let two = || vec!["a".to_string(), "b".to_string()];
        assert_eq!(zh.btf("第 {0} 轮", "Turn {0}", &one("7")), "第 7 轮");
        // 多余实参：不生效、也不报错。
        assert_eq!(zh.btf("只有一洞 {0}", "one {0}", &two()), "只有一洞 a");
        // 缺洞：两侧都吐**中文模板**（不是选完档那串，也不是 en 模板）。
        assert_eq!(zh.btf("两洞 {0} 与 {1}", "two {0} and {1}", &one("a")), "两洞 {0} 与 {1}");
        assert_eq!(en.btf("两洞 {0} 与 {1}", "two {0} and {1}", &one("a")), "两洞 {0} 与 {1}");
        // 洞按选中的那串数：en 界面选中只有一洞的模板 ⇒ 喂两枚也不回落。
        assert_eq!(en.btf("两洞 {0} 与 {1}", "one {0}", &two()), "one a");
        // 无洞无参 = `bt`（本函数只多填洞那一步，不许夹带后处理）。
        assert_eq!(
            en.btf("消息详情", "Message details", &[]),
            en.bt("消息详情", "Message details")
        );
        assert_eq!(en.btf("轨迹面板", "Trajectory panel", &[]), "Trajectory panel");
    }

    /// 全族唯一带格式说明符的那枚洞（`Trajectory.cs` 的毫秒行 `TrajFormat("{0:N0} 毫秒",
    /// "{0:N0} ms", ms)`）：照 `dtf` 那种只替换裸 `{n}` 的写法会**静默漏**成字面 `{0:N0}`。
    /// 分组由调用点负责（`args` 是 `&[String]`，说明符被跳过、实参原样插入）。
    #[test]
    fn btf_fills_the_only_specifier_slot_the_mainline_family_uses() {
        let zh = Catalog::load("zh", None);
        let en = Catalog::load("en", None);
        let ms = vec!["1,234".to_string()];
        assert_eq!(zh.btf("{0:N0} 毫秒", "{0:N0} ms", &ms), "1,234 毫秒");
        assert_eq!(en.btf("{0:N0} 毫秒", "{0:N0} ms", &ms), "1,234 ms");
        // 反面对照（既有语义，一行没改）：`dtf` 在这枚点上漏出字面说明符。
        assert_eq!(zh.dtf("{0:N0} 毫秒", "{0:N0} ms", &ms), "{0:N0} 毫秒");
        assert_eq!(en.dtf("{0:N0} 毫秒", "{0:N0} ms", &ms), "{0:N0} ms");
        // 一洞不喂 = 主干 `FormatException` 那一支 ⇒ 中文模板本体。
        assert_eq!(en.btf("{0:N0} 毫秒", "{0:N0} ms", &[]), "{0:N0} 毫秒");
    }

    /// 11 枚「字典生效值 ≠ 内联 `en`」的键：断言**字典生效值赢**（= 主干英文界面实际显示的那串）。
    /// 内联那列在这 11 枚上是死码，传错也不显示 ⇒ 这一条同时锁住「宿主带内联字面串也不会译反」。
    #[test]
    fn bt_renders_the_dictionary_value_where_the_inline_english_disagrees() {
        // (中文键, 主干调用点的内联 en, 主干生效值)
        const DIFF: &[(&str, &str, &str)] = &[
            ("交付物", "Deliverables", "Deliverable"),
            ("模型", "Model", "Models"),
            ("模型用时", "LLM time", "Model time"),
            ("工具调用用时", "Tool time", "Tool-call time"),
            ("输出速度（TPS）", "Throughput (TPS)", "Output speed (TPS)"),
            ("插件", "Plugin", "Plugins"),
            ("查看", "Inspect", "View"),
            ("恢复", "Resume", "Restore"),
            ("复制参数 JSON", "Copy JSON", "Copy parameters JSON"),
            ("已取消", "Cancelled", "cancelled"),
            ("继续", "continue", "Continue"),
        ];
        assert_eq!(DIFF.len(), 11);
        // 三档都不带 assets（`load(locale, None)`）⇒ 本语种 json 恒空，断的是主干「该语种 json
        // 缺这枚键」那一列；`zh-TW` 上主干同样只有 `ShellEnglish` 那一档可落 ⇒ 出英文。
        for locale in ["en", "de", "zh-TW"] {
            let catalog = Catalog::load(locale, None);
            for (key, inline, effective) in DIFF {
                assert_eq!(&catalog.bt(key, inline), *effective, "{locale} / {key}");
            }
        }
        // 中文档仍是中文：内联与字典都不许出场。
        let zh = Catalog::load("zh", None);
        for (key, inline, _) in DIFF {
            assert_eq!(zh.bt(key, inline), *key);
        }
    }

    /// 反向锁（纯函数行为锁，零 IO、不碰任何源码文本）：这族刻意留在**调用点那一行**的串，
    /// 主干明文「勿写入 ShellEnglish」⇒ 分叉 EN 表（它的镜像）也不许收。判据用
    /// `l(zh) == zh` 证明「字典里没有」；哪天有人补键，这条立刻红。
    /// 最后破窗的三枚（`第 {0} 轮` + 两枚任务面板长句）现已**全部撤表**并移进下面这份名单：
    /// 两枚长句随 #148 刀 B 撤（同刀把 `main.rs` 调用点换成内联道），`第 {0} 轮` 随刀 C 撤
    /// （它走的是通道 A 的 `LF`，撤表不需要动 `main.rs`）。判据见
    /// `en_table_entries_that_leaked_from_the_bilingual_family`。
    #[test]
    fn bilingual_family_keys_that_mainline_left_out_of_shellenglish_stay_out_of_en() {
        const INLINE_ONLY: &[&str] = &[
            "轨迹面板",
            "计时总览",
            "事件账本",
            "类型筛选",
            "暂无轮次计时。",
            "当前筛选没有事件。",
            "内核未连接。",
            "Cordis 插件面板",
            "还没有定义任何插件",
            "待审批",
            "Client 待激活",
            "跳到第 {0} 轮消息",
            "参数 JSON",
            "结果 JSON",
            "工具耗时",
            "未捕获参数",
            "未捕获结果",
            "Token",
            "当前会话没有任务清单。",
            "任务清单来自会话 todos 投影（模型经 todo_write 更新）。此面板只读，不创建、编辑或删除任务。",
            // 刀1（DetText 第五族 · #148）追加 10 枚：主干 `MainWindow.MessageDetails.cs`
            // 那批 `DetText(zh, en)` / `DetFormat(zh, en, …)` 的内联档串（`:257` / `:290-292` /
            // `:440-442` / `:446` / `:466` / `:473`）——主干没登记进 `ShellEnglish`，英文权威
            // 在调用点那一行 ⇒ 分叉 EN 表（它的镜像）也不许收。模板位写 `{0}`/`{1}` 原样。
            "跨会话中继",
            "保留 {0} 条 · 省略 {1} 条",
            "已截断",
            "跨会话召回 · {0}",
            "已载入",
            "已移除",
            "已新增",
            "已更新",
            "…还有 {0} 条",
            "取代先前的快照",
        ];
        let en = Catalog::load("en", None);
        for key in INLINE_ONLY {
            assert_eq!(en.l(key), *key, "{key} 的英文权威在调用点那一行，不该进 EN 表");
            assert_eq!(en.bt(key, "inline-only sentinel"), "inline-only sentinel");
        }
        // 刀1（#148 · DetText 第五族）追加 10 枚后，名单必须**真的带着这 10 枚**：上面那圈只验
        // 「不在 EN 表里」，不验「在册」⇒ 删掉一枚照样绿（DT2 给这张硬编码名单挂的 [否] 旗）。
        // 这里补**内容闸 + 枚数等号**，一枚 `>=` 下界都没有。
        const DET_FAMILY: &[&str] = &[
            "跨会话中继",
            "保留 {0} 条 · 省略 {1} 条",
            "已截断",
            "跨会话召回 · {0}",
            "已载入",
            "已移除",
            "已新增",
            "已更新",
            "…还有 {0} 条",
            "取代先前的快照",
        ];
        assert_eq!(
            DET_FAMILY.len(),
            10,
            "DetText 第五族刀1 定案 13 枚 = 去重后 10 枚（#11/#12 同串；#1/#2 走字典档、严禁入表）"
        );
        for key in DET_FAMILY {
            assert!(
                INLINE_ONLY.contains(key),
                "刀1 那 10 枚里少了这一枚：{key}"
            );
        }
        assert_eq!(
            INLINE_ONLY.len(),
            30,
            "名单现册 = 20（刀1 之前）+ 10（刀1）；枚数变了必须同时改这条等号，不许静默漂移"
        );
    }

    /// 曾误入 EN 表的内联双语对：**在册三枚已全部撤完**（两枚任务面板长句随 #148 刀 B 撤表并把
    /// `main.rs` 的调用点换成内联道；`第 {0} 轮` 随刀 C 撤 —— 主干 `MainWindow.TurnRail.cs` 走
    /// `LF("第 {0} 轮", mark.Turn)`（通道 A），压根没有内联英文可抄，分叉生产端 `lf` 保留不动）。
    /// 名单自此只留**主干确有内联英文档**的那两枚（撤表后它们改由 `bt` 那一档出英文，本锁把
    /// 「表里 / 表外两条路径给同一串」钉成行为等值，防哪天有人重新登记进 EN 表却改了译名）。
    /// `第 {0} 轮` 已不属这一族（它没有内联档可钉），故随刀 C 一并从名单摘除；它的口径是
    /// 「英文档必须与中文档同串」，由真值端与等价穷尽锁守，不在这张表里挂账。
    /// 判据是**行为等值**而非存在性：撤掉之后内联档仍给同一串 ⇒ 撤表那天这条自然转绿、
    /// 不会留下假红；哪天两侧给的串分叉了才红。
    #[test]
    fn en_table_entries_that_leaked_from_the_bilingual_family_stay_render_compatible() {
        // (中文键, 主干调用点的内联 en, 主干落点符号)
        const DEBT: &[(&str, &str, &str)] = &[
            (
                "任务清单来自会话 todos 投影（模型经 todo_write 更新）。此面板只读，不创建、编辑或删除任务。",
                "The to-do list comes from the session todos projection (the model updates it via todo_write). This panel is read-only.",
                "CapabilityText",
            ),
            (
                "当前会话没有任务清单。",
                "No to-do list in this session.",
                "CapabilityText",
            ),
        ];
        for (key, inline, site) in DEBT {
            let en = Catalog::load("en", None);
            // 撤表前后都必须给主干那一串。
            assert_eq!(en.bt(key, inline), *inline, "{key} @ {site}");
            if en.l(key) != *key {
                // 还在表里 ⇒ 表值必须与主干内联一致，否则撤表会顺带改动 `l` 路径的显示。
                assert_eq!(en.l(key), *inline, "{key} @ {site} 的表值与主干内联不一致");
                eprintln!("在册债：{key} 仍在 EN 表里（主干 {site} 的内联对），本该撤");
            }
        }
    }

    /// 刀 C 撤表后的**面值锁**（英文档必须露中文，这才是主干 `LF` 的行为）：主干
    /// `MainWindow.TurnRail.cs` 写的是 `LF("第 {0} 轮", mark.Turn)` —— `LF` 只查 `ShellEnglish`，
    /// 而那本字典里**没有**裸 `第 {0} 轮`（只有 `将把本轮（第 {0} 轮）改过的文件…` 那枚长键），
    /// 所以主干英文界面此处本就吐 `第 5 轮`。分叉 `lf` 走同一把查法 ⇒ 撤表后同串。
    /// 哪天有人把 `"Turn {0}"` 再塞回 EN 表（= 凭空造一枚主干没有的回落档），这条立刻红。
    #[test]
    fn the_turn_rail_fallback_leaks_chinese_in_english_exactly_like_mainline_lf() {
        let five = || vec!["5".to_string()];
        assert_eq!(Catalog::load("zh", None).lf("第 {0} 轮", &five()), "第 5 轮");
        assert_eq!(
            Catalog::load("en", None).lf("第 {0} 轮", &five()),
            "第 5 轮",
            "撤表后英文档必须与中文档同串：主干 ShellEnglish 无裸「第 {{0}} 轮」键 ⇒ \
             分叉不许靠 EN 表补一枚主干没有的英文名"
        );
        // 表里剩下的那枚**长键**才是主干真登记过的，方向别搞反。
        assert_eq!(
            Catalog::load("en", None).l("将把本轮（第 {0} 轮）改过的文件恢复到修改前的状态："),
            "Restore the files changed in turn {0} to their state before the change:"
        );
    }

    /// 读主干那 7 枚承载内联双语族的文件之一（分叉被单独检出时读不到 ⇒ None，调用方跳过）。
    fn read_mainline_cs(file: &str) -> Option<String> {
        std::fs::read_to_string(format!("../{file}"))
            .or_else(|_| std::fs::read_to_string(file))
            .ok()
    }

    /// 运行时解析主干源码里的 `(zh字面串, en字面串)` 对子，让主干自己当裁判。
    /// 只读主干 `.cs`，**不** `include_str!` 任何分叉大文件（同文件自读会自匹配）。
    /// 非字面串参数的调用点（主干那 12 枚动态传参点）在这里抓不到 ⇒ 由
    /// `mainline_dynamic_bilingual_pairs()` 补齐。
    fn mainline_bilingual_pairs() -> Option<Vec<(String, String)>> {
        let helpers = [
            concat!("Traj", "Text"),
            concat!("Traj", "Format"),
            concat!("Det", "Text"),
            concat!("Det", "Format"),
            concat!("Capability", "Text"),
            concat!("Cordis", "Text"),
        ];
        let mut out: Vec<(String, String)> = Vec::new();
        let mut touched = false;
        for file in [
            "MainWindow.Trajectory.cs",
            "MainWindow.MessageDetails.cs",
            "MainWindow.Cordis.cs",
            "MainWindow.Capabilities.cs",
            "MainWindow.ToolCards.cs",
            "MainWindow.MessageActions.cs",
            "MainWindow.Todos.cs",
        ] {
            let Some(raw) = read_mainline_cs(file) else {
                eprintln!("跳过：读不到主干 {file}");
                continue;
            };
            touched = true;
            let b = raw.as_bytes();
            let mut helper_total = 0usize;
            for helper in helpers {
                let mut from = 0usize;
                let mut hits = 0usize;
                while let Some(relative) = raw[from..].find(helper) {
                    let mut i = from + relative + helper.len();
                    from = i;
                    while i < b.len() && (b[i] as char).is_ascii_whitespace() {
                        i += 1;
                    }
                    if i >= b.len() || b[i] != b'(' {
                        continue;
                    }
                    i += 1;
                    while i < b.len() && (b[i] as char).is_ascii_whitespace() {
                        i += 1;
                    }
                    if i >= b.len() || b[i] != b'"' {
                        continue;
                    }
                    let (zh, after_zh) = read_cs_literal(&raw, i);
                    i = after_zh;
                    while i < b.len() && (b[i] as char).is_ascii_whitespace() {
                        i += 1;
                    }
                    if i >= b.len() || b[i] != b',' {
                        continue;
                    }
                    i += 1;
                    while i < b.len() && (b[i] as char).is_ascii_whitespace() {
                        i += 1;
                    }
                    if i >= b.len() || b[i] != b'"' {
                        continue;
                    }
                    let (en, after_en) = read_cs_literal(&raw, i);
                    from = after_en;
                    out.push((zh, en));
                    hits += 1;
                }
                if hits > 0 {
                    eprintln!("{file} :: {helper} 解析出 {hits} 对字面串");
                    helper_total += hits;
                }
            }
            eprintln!("{file} 合计 {helper_total} 对");
        }
        touched.then_some(out)
    }

    /// 主干 §2.2 那 9 张局部表里的**动态传参**对子（字面量不在调用点那一行，运行时解析抓不到，
    /// 只能写死当夹具；这是解析锁的补集，不是第三份权威）。
    fn mainline_dynamic_bilingual_pairs() -> Vec<(&'static str, &'static str)> {
        vec![
            ("目标", "Goal"),
            ("任务", "To-dos"),
            ("计划（只读）", "Schedules (read-only)"),
            ("技能", "Skills"),
            ("轨迹", "Trajectory"),
            ("Cordis 插件", "Cordis plugins"),
            ("刷新", "Refresh"),
            ("创建目标", "Create goal"),
            ("保存编辑", "Save edits"),
            ("暂停", "Pause"),
            ("恢复", "Resume"),
            ("标记完成", "Mark complete"),
            ("清除目标", "Clear goal"),
            ("全部", "All"),
            ("用户", "User"),
            ("助手", "Assistant"),
            ("工具", "Tool"),
            ("步骤", "Step"),
            ("系统", "System"),
            ("其他", "Other"),
            ("允许", "Allow"),
            ("仅允许此版本", "Allow this version only"),
            ("允许此插件的后续版本", "Allow future versions of this plugin"),
            ("拒绝", "Decline"),
            ("角色", "Role"),
            ("轮次", "Turn"),
            ("序号", "Seq"),
            ("时间", "Time"),
            ("模型", "Model"),
            ("提供方", "Provider"),
            ("上下文窗口", "Context window"),
            ("用时", "Duration"),
            ("用量", "Tokens"),
            ("消息 ID", "Message ID"),
            ("系统提示词", "System prompt"),
            ("复制格式化 JSON", "Copy pretty JSON"),
            ("复制紧凑 JSON", "Copy compact JSON"),
            ("复制属性路径", "Copy property path"),
            ("复制参数 JSON", "Copy JSON"),
            ("复制结果 JSON", "Copy result JSON"),
        ]
    }

    /// 穷尽锁：主干每一对内联双语对，在 `zh` / `en` / `zh-TW` 三档下必须与「主干
    /// `ShellEnglish` 的 last-wins 生效值 + 前缀门」算出来的串逐字相同。
    /// `zh-TW` 这档按「该语种 json 缺这枚键」的口径断（测试不带 assets），与
    /// `bt_returns_the_call_site_english_only_for_keys_shellenglish_misses` 里带 json 的那条互补。
    /// 豁免 `显示`：与总闸 `en_table_matches_every_mainline_shell_english_key` 同一条例外。
    #[test]
    fn bilingual_call_site_pairs_render_identically_to_mainline_in_every_locale() {
        let Some(mut rows) = mainline_bilingual_pairs() else {
            eprintln!("跳过：找不到主干 MainWindow.*.cs（分叉被单独检出时属正常）");
            return;
        };
        let parsed = rows.len();
        for (zh, en) in mainline_dynamic_bilingual_pairs() {
            rows.push((zh.to_string(), en.to_string()));
        }
        rows.sort();
        rows.dedup();
        eprintln!(
            "内联双语对：解析 {parsed} 对 + 动态传参补集 ⇒ 去重后 {} 对",
            rows.len()
        );
        assert!(
            parsed > 150,
            "主干内联双语对只解析出 {parsed} 对，明显不对（抽取式失效了）"
        );
        let Some(shell) = mainline_shell_english() else {
            eprintln!("跳过：找不到主干 MainWindow.xaml.cs，没有裁判");
            return;
        };
        let mut last: HashMap<String, String> = HashMap::new();
        for (key, value) in &shell {
            last.insert(key.clone(), value.clone());
        }
        let mut checked = 0usize;
        for (zh, en) in &rows {
            if zh == "显示" {
                continue;
            }
            let dict = last.get(zh);
            for locale in ["zh", "en", "zh-TW"] {
                // 在册债那三枚（主干 `ShellEnglish` 无键、分叉 EN 表有键）在 `zh-` 前缀档上
                // 两侧本就不同命：主干走门出简体、分叉被 EN 档拦下出英文。判据取「表里到底有没有
                // 这枚键」这个**运行时条件**，所以撤表那天条件转假 ⇒ 这一格自动并回本锁，不写死名单。
                if locale == "zh-TW" && dict.is_none() && EN.iter().any(|(from, _)| *from == *zh) {
                    eprintln!("在册债：{zh} 只在分叉 EN 表里（主干 ShellEnglish 无此键）⇒ zh- 档主干出简体、分叉出英文；撤表后自然并入本锁");
                    continue;
                }
                let want = if locale == "zh" {
                    zh.clone()
                } else if let Some(value) = dict {
                    value.clone()
                } else if locale.to_ascii_lowercase().starts_with("zh") {
                    zh.clone()
                } else {
                    en.clone()
                };
                assert_eq!(
                    Catalog::load(locale, None).bt(zh, en),
                    want,
                    "{locale} / {zh} 与主干选档不一致"
                );
                checked += 1;
            }
        }
        eprintln!("实际断言 {checked} 次（{} 对 × 3 档）", rows.len());
    }

    /// 读一个 C# 常规字符串字面量（`start` 指向开引号），返回解码后的正文与闭引号后一位。
    /// C# 与 Rust 对 `\" \\ \n \r \t` 的拼写完全相同，所以这里解码到真实字符再比对，
    /// 避免「源码转义写法不同」被误判成文案不同。
    fn read_cs_literal(text: &str, start: usize) -> (String, usize) {
        let b = text.as_bytes();
        let mut i = start + 1;
        let mut out = String::new();
        while i < b.len() {
            match b[i] {
                b'"' => return (out, i + 1),
                b'\\' if i + 1 < b.len() => {
                    out.push(match b[i + 1] {
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        c => c as char,
                    });
                    i += 2;
                }
                c => {
                    let s = &text[i..];
                    let ch = s.chars().next().unwrap_or('\u{0}');
                    out.push(ch);
                    i += ch.len_utf8();
                    let _ = c;
                }
            }
        }
        (out, b.len())
    }

    /// 把主干 `MainWindow.xaml.cs` 的 `ShellEnglish` 整表解析成 `中文 -> English` 的**有序**清单。
    /// 只在测试里跑：验收口径要求分叉英文界面与主干一模一样，那就让主干那张表自己当裁判。
    fn mainline_shell_english() -> Option<Vec<(String, String)>> {
        let raw = std::fs::read_to_string("../MainWindow.xaml.cs")
            .or_else(|_| std::fs::read_to_string("MainWindow.xaml.cs"))
            .ok()?;
        let head = raw.find("Dictionary<string, string> ShellEnglish")?;
        let body = &raw[head..];
        let open = body.find('{')?;
        let close = body[open..]
            .find("\n    };")
            .map(|n| open + n)
            .unwrap_or(body.len());
        let text = &body[open..close];
        let b = text.as_bytes();
        let mut out: Vec<(String, String)> = Vec::new();
        let mut i = 0;
        while i + 1 < b.len() {
            if !(b[i] == b'[' && b[i + 1] == b'"') {
                i += 1;
                continue;
            }
            let (key, after_key) = read_cs_literal(text, i + 1);
            let mut j = after_key;
            while j < b.len() && (b[j] as char).is_ascii_whitespace() {
                j += 1;
            }
            if j < b.len() && b[j] == b']' {
                j += 1;
                while j < b.len() && (b[j] as char).is_ascii_whitespace() {
                    j += 1;
                }
                if j < b.len() && b[j] == b'=' {
                    j += 1;
                    while j < b.len() && (b[j] as char).is_ascii_whitespace() {
                        j += 1;
                    }
                    if j < b.len() && b[j] == b'"' {
                        let (val, after_val) = read_cs_literal(text, j);
                        out.push((key, val));
                        i = after_val;
                        continue;
                    }
                }
            }
            i = after_key.max(i + 1);
        }
        Some(out)
    }

    /// 反漂移总闸：主干 `ShellEnglish` 的每一枚键都必须已经在 `EN` 里，且值逐字相等。
    /// 主干同键重复是 **last-wins**（`.cs` 字典初始化器后写覆盖先写），所以按文件顺序
    /// `insert` 折叠，让最后一条生效——这正是任务 #113 实测出来的口径。
    /// 唯一豁免 `显示`：分叉走的是宠物卡头那条裁决，见
    /// `pet_card_header_pins_the_mainline_effective_english`。
    #[test]
    fn en_table_matches_every_mainline_shell_english_key() {
        let Some(rows) = mainline_shell_english() else {
            eprintln!("跳过：找不到主干 MainWindow.xaml.cs（分叉被单独检出时属正常）");
            return;
        };
        let mut last: HashMap<&str, &str> = HashMap::new();
        for (k, v) in &rows {
            last.insert(k, v);
        }
        assert!(
            rows.len() > 1200 && last.len() > 1200,
            "主干表只解析出 {} 条 / {} 枚唯一键，明显不对",
            rows.len(),
            last.len()
        );
        let mut missing: Vec<&str> = Vec::new();
        let mut wrong: Vec<String> = Vec::new();
        let mut keys: Vec<&&str> = last.keys().collect();
        keys.sort_unstable();
        for k in keys {
            let want = last[*k];
            match EN.iter().find(|(from, _)| from == k) {
                None => missing.push(*k),
                Some((_, got)) => {
                    if *got != want && *k != "显示" {
                        wrong.push(format!("  [{k}] 分叉={got} 主干last-wins={want}"));
                    }
                }
            }
        }
        assert!(
            missing.is_empty(),
            "EN 表缺主干键 {} 枚（主干共 {} 枚唯一键）:\n{}",
            missing.len(),
            last.len(),
            missing.iter().map(|k| format!("  [{k}]")).collect::<Vec<_>>().join("\n")
        );
        assert!(wrong.is_empty(), "EN 值与主干 last-wins 不一致 {} 枚:\n{}", wrong.len(), wrong.join("\n"));
    }

    /// 逐字钉住本片复核到的代表性主干文案（占位符 / 真换行键 / 全角标点 / last-wins 碰撞键 /
    /// 长段落），走的是用户可见路径 `Catalog::l`，不是「表里有条目就行」的恒真式。
    #[test]
    fn en_table_pins_representative_mainline_copy_verbatim() {
        let en = Catalog::load("en", None);
        // IN2-PINS-BEGIN
        // mainline:1728
        assert_eq!(en.l("petdex install 或 codex-pets add <宠物标识>"),
            "petdex install or codex-pets add <pet-slug>");
        // mainline:1312
        assert_eq!(en.l("{0} 声明 {1} 个模型（选择后填入默认模型）："),
            "{0} declares {1} models (select one to fill the default model):");
        // mainline:1432
        assert_eq!(en.l("“@{0}” 匹配 {1} 条（↑↓ 选择，Enter 插入，Esc 关闭）"),
            "\"@{0}\" matched {1} (↑↓ select, Enter insert, Esc close)");
        // mainline:1373
        assert_eq!(en.l("「累计/峰值/连续天数」按全部历史，「时间范围」只作用于趋势图与模型用量；"),
            "Totals/peaks/streaks cover all history; the time range only affects the trend chart and model usage;");
        // mainline:1316
        assert_eq!(en.l("共 {0} 个 Loader 条目（active {1}，failed {2}，未启用 {3}）。"),
            "{0} loader entries total (active {1}, failed {2}, disabled {3}).");
        // mainline:1640
        assert_eq!(en.l("功能完整的编码 Agent，支持文件编辑、Shell、文件与网页检索、Skills、计划、目标、子代理和工作流。"),
            "A full-featured coding agent: file editing, shell, file and web search, skills, plans, goals, subagents and workflows.");
        // mainline:1261
        assert_eq!(en.l("图片"),
            "Image");
        // mainline:1240
        assert_eq!(en.l("将把“{0}”从工作区列表中移除。文件夹与会话记录会保留，其会话将显示在“未分组”下。"),
            "\"{0}\" will be removed from the workspace list. The folder and session records are kept; its sessions will appear under \"Ungrouped\".");
        // mainline:1784
        assert_eq!(en.l("已自动保存 · {0} · {1} 行文本已转为记忆实体"),
            "Auto-saved · {0} · {1} line(s) turned into memory entities");
        // mainline:1431
        assert_eq!(en.l("引用 {0} 条（@ 后输入可过滤；↑↓ 选择，Enter 插入，Esc 关闭）"),
            "{0} references (@ to filter; ↑↓ select, Enter insert, Esc close)");
        // mainline:903
        assert_eq!(en.l("恢复默认"),
            "Restore defaults");
        // mainline:1138
        assert_eq!(en.l("撤回编辑（停止运行并把这句放回输入框）"),
            "Withdraw edit (stop the run and put this message back in the composer)");
        // mainline:1372
        assert_eq!(en.l("数据来源：内核 journal（session/list + session/page）的用量记录；已聚合 {0} 个非空会话、{1} 条记录{2}{3}。"),
            "Data source: kernel journal (session/list + session/page) usage records; aggregated {0} non-empty sessions and {1} records{2}{3}.");
        // mainline:1276
        assert_eq!(en.l("无法打开：宿主桌面不可用（内核 workspaceDesktop().available=false）。"),
            "Cannot open: host desktop unavailable (kernel workspaceDesktop().available=false).");
        // mainline:1019
        assert_eq!(en.l("模型 {0}：最大输出 token 数必须是正数，例如 8192、64K 或 1M。"),
            "Model {0}: max output tokens must be a positive count, like 8192, 64K, or 1M.");
        // mainline:1532
        assert_eq!(en.l("正在读取…"),
            "Reading…");
        // mainline:1528
        assert_eq!(en.l("目录项超过内核上限，仅显示前 {0} 项。"),
            "The directory exceeds the kernel cap; showing the first {0} entries only.");
        // mainline:1454
        assert_eq!(en.l("计划仅支持只读。当前主窗口未缓存 session/control 的 schedule 投影，无法显示计划列表；这不表示会话没有计划。\n此入口不创建、编辑或删除计划，也不调用 schedules RPC。"),
            "Schedules are read-only. The main window does not currently cache the session/control schedule projection, so the list is unavailable; this does not mean the session has no schedules.\nNo schedules are created, edited or deleted; no schedules RPC is called.");
        // mainline:1546
        assert_eq!(en.l("读取失败"),
            "Read failed");
        // mainline:973
        assert_eq!(en.l("需以小写字母开头，之后可用小写字母、数字和短横线。"),
            "Start with a lowercase letter; then lowercase letters, digits, and dashes.");
        // mainline:624
        assert_eq!(en.l("预设即一个会话的 Agent 所运行的插件组装——它的工具、提示词与能力。"),
            "A preset defines the plugins an agent runs: its tools, prompts and capabilities.");
        // IN2-PINS-END
    }

    // ────────────────────────────────────────────── C8 · RD9 卡 8「i18n 预铺键」──
    // 本卡实测五族**零键可铺**（主干 `ShellEnglish` 1240 枚唯一键已被 `EN` 1396 枚全覆盖，差集
    // 为空 ⇒ `:2818` 那道总闸今天本就绿）。所以这里铺的不是键，是**按族可归因的通道锁**：
    // 总闸只管「整张表 ⊇ 整张主干字典」，说不出「Files 族 / 能力账本族各自走哪条道」，
    // 而这两族的道数**相反**（Files 全走表、能力账本全走内联），一旦有人拿错道去补键，
    // 总闸照样绿、英文界面却已经和主干不一致。下面这条按族拆一遍，才是本卡的净增量。

    /// 通道 A（shell-English 表）的四个别名。`TL`/`TLF` **不是第三条道**，是 `L`/`LF` 的静态转发：
    /// 主干 `MainWindow.xaml.cs:2020` `internal static string TL(string key) => ShellTranslateFunc(key);`
    /// 与 `:2523` `ShellTranslateFunc = L;`。Files 面板两颗文件 `L(` 计数为 0、全靠 `TL`/`TLF`
    /// 出文案（`Pages/FilesPanel.xaml.cs:202` `MainWindow.TL("工作区文件")`），
    /// 少列这枚别名就会把整族误判成「不走表」⇒ 漏铺。
    const CHANNEL_A_HELPERS: &[&str] = &["L", "LF", "TL", "TLF"];

    /// 通道 B（内联双语）全家：英文写在**调用点第二参**，分叉侧归 [`Catalog::bt`] / [`Catalog::btf`]，
    /// **不许进 EN 表**。`CapabilityText` / `CordisText` 是 `TrajText` 的纯转发，同一条道。
    const CHANNEL_B_HELPERS: &[&str] = &[
        "TrajText",
        "TrajFormat",
        "DetText",
        "DetFormat",
        "CapabilityText",
        "CordisText",
    ];

    /// 五族 × 主干宿主。凭证管理器与 Agent 预设**没有自己的 partial**（简报点名的
    /// `MainWindow.Credentials.cs` / `MainWindow.Presets.cs` 不存在），宿主是 `MainWindow.xaml.cs`：
    /// 凭据编辑区 `:10793-12150`、网页搜索密钥 `:12682-12740`、预设页 `:13037-13320`、
    /// Agent 模式菜单 `:4800-4833`。两族同宿主 ⇒ 各自核一遍是**有意的重复**（归因要落到族名上）。
    const RD9_FAMILIES: &[(&str, &[&str])] = &[
        (
            "files",
            &["Pages/FilesPanel.xaml.cs", "Pages/FilesPanel.Preview.cs"],
        ),
        (
            "goal-skills-ledger",
            &["MainWindow.Cordis.cs", "MainWindow.Capabilities.cs"],
        ),
        ("skills-panel", &["MainWindow.Skills.cs"]),
        ("credentials", &["MainWindow.xaml.cs"]),
        ("agent-presets", &["MainWindow.xaml.cs"]),
        ("subagents", &["MainWindow.Subagents.cs"]),
    ];

    /// 扫主干某文件里 `Helper("字面串"` 的**首参**，返回 `(解码后的串, 行号)`。
    /// 只认字面串：非字面传参（`L(name)` / `TL(text)`）在这里抓不到，那本就不是表键的判据。
    /// 前邻是字母/数字/下划线/非 ASCII 一律不算命中 ⇒ `TL(` 不被 `L(` 抢走、`DetText(` 不被
    /// `L(` 抢走；`L(` 与 `LF(` 互不串味（`L` 后必须紧跟 `(`）。与 `mainline_bilingual_pairs()`
    /// 同一套思路，但那是六枚内联 helper 的两参版，这里要的是**按通道分组 + 行号归因**。
    fn mainline_first_literal_calls(raw: &str, helpers: &[&str]) -> Vec<(String, usize)> {
        let mut out = Vec::new();
        let bytes = raw.as_bytes();
        for helper in helpers {
            let needle = format!("{helper}(");
            let mut from = 0usize;
            while let Some(rel) = raw[from..].find(&needle) {
                let at = from + rel;
                from = at + needle.len();
                if at > 0 {
                    let prev = bytes[at - 1];
                    if prev.is_ascii_alphanumeric() || prev == b'_' || prev >= 0x80 {
                        continue;
                    }
                }
                let mut i = at + needle.len();
                while i < raw.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i >= raw.len() || bytes[i] != b'"' {
                    continue;
                }
                let (text, after) = read_cs_literal(raw, i);
                out.push((text, raw[..at].matches('\n').count() + 1));
                from = after;
            }
        }
        out
    }

    /// 按族双向锁。
    /// · **正向（该在的必须在）**：本族通道 A 的串，凡主干 `ShellEnglish` 收了的，分叉 EN 必须有 ——
    ///   这就是「预铺键」的完成判据，缺一枚立刻红并点名 `文件:行`。
    /// · **反向（不该在的不许在，本卡最有价值的一条）**：本族通道 B 的串，凡主干 `ShellEnglish`
    ///   **没**收的，分叉 EN 也不许有。主干自己登记过的那批（`拒绝` / `查看` / `关闭` / `目标` …，
    ///   它们是别的界面的表键）不在反向判据里 —— 留着才与主干逐档一致，见 [`Catalog::bt`] 文档段
    ///   「字典先赢、调用点的 `en` 是死码」。
    /// 主干源读不到 ⇒ `eprintln!` 后跳过该文件（分叉被单独检出时属正常），**不** `include_str!`。
    #[test]
    fn rd9_five_families_keep_their_copy_on_the_mainline_channel() {
        let Some(shell_rows) = mainline_shell_english() else {
            eprintln!("跳过：找不到主干 MainWindow.xaml.cs，没有通道 A 的裁判");
            return;
        };
        // 主干同键重复是 last-wins（字典初始化器后写盖先写），折叠成生效值。
        let mut effective: HashMap<&str, &str> = HashMap::new();
        for (k, v) in &shell_rows {
            effective.insert(k, v);
        }
        let en = Catalog::load("en", None);
        // 判据用**精确成员资格**，不用 `l(zh) == zh` 那把「回落即缺席」的尺子：
        // 主干登记了一批**恒等映射**（`["Provider ID"] = "Provider ID"`、`["{0:0.#} 亿"]` 同形，
        // 见 `i18n.rs:686/687/929`），它们在表里与不在表里 `l()` 给的是同一串 ⇒ 用 `l()` 判会
        // 把已铺好的恒等键误报成缺键。`en_table_has_no_duplicate_keys` 保证了这里 `any` 即唯一解。
        let in_en = |k: &str| EN.iter().any(|(from, _)| *from == k);
        let mut a_checked = 0usize;
        let mut b_checked = 0usize;
        let mut untranslated: Vec<String> = Vec::new();

        for (family, files) in RD9_FAMILIES {
            let mut absent: Vec<String> = Vec::new();
            let mut leaked: Vec<String> = Vec::new();
            let mut seen_a: Vec<String> = Vec::new();
            let mut seen_b: Vec<String> = Vec::new();
            for file in *files {
                let Some(raw) = read_mainline_cs(file) else {
                    eprintln!("跳过：读不到主干 {file}");
                    continue;
                };
                for (zh, line) in mainline_first_literal_calls(&raw, CHANNEL_A_HELPERS) {
                    match effective.get(zh.as_str()) {
                        Some(want) => {
                            if !seen_a.contains(&zh) {
                                seen_a.push(zh.clone());
                                a_checked += 1;
                            }
                            // 恒等映射也算在表里 ⇒ 用精确成员资格判，见上。
                            if !in_en(&zh) {
                                absent.push(format!("  {file}:{line} [{zh}] 主干={want}"));
                            }
                        }
                        // 主干自己就没登记 ⇒ 主干英文界面此处本就露中文 ⇒ 分叉**不许**补译。
                        // 两族同宿主（`MainWindow.xaml.cs`）会各扫一遍 ⇒ 去重，否则计数虚高。
                        None => {
                            let hole = format!("  {file}:{line} [{zh}]");
                            if !untranslated.contains(&hole) {
                                untranslated.push(hole);
                            }
                        }
                    }
                }
                for (zh, line) in mainline_first_literal_calls(&raw, CHANNEL_B_HELPERS) {
                    if effective.contains_key(zh.as_str()) {
                        continue;
                    }
                    if seen_b.contains(&zh) {
                        continue;
                    }
                    seen_b.push(zh.clone());
                    b_checked += 1;
                    if in_en(&zh) {
                        leaked.push(format!("  {file}:{line} [{zh}] => 表值 {}", en.l(&zh)));
                    }
                }
            }
            assert!(
                absent.is_empty(),
                "族 {family} 有 {} 枚走通道 A 的主干键没铺进 EN 表:\n{}",
                absent.len(),
                absent.join("\n")
            );
            assert!(
                leaked.is_empty(),
                "族 {family} 有 {} 枚内联通道（bt/btf）的串误入 EN 表 = 多做，必须撤:\n{}",
                leaked.len(),
                leaked.join("\n")
            );
        }
        // 防空跑：解析器哪天对不上主干写法，两向都会安静地核到 0 次、锁成恒真式。
        assert!(
            a_checked > 400,
            "通道 A 只核了 {a_checked} 次，解析器明显没抓到主干调用点"
        );
        assert!(
            b_checked > 20,
            "通道 B 只核了 {b_checked} 次，反向锁已经失效"
        );
        assert!(
            untranslated.len() < 60,
            "主干未登记的中文字面串涨到 {} 枚，见报告的反向清单，得先确认主干是不是漏译",
            untranslated.len()
        );
        eprintln!("通道 A 核 {a_checked} 次、通道 B 核 {b_checked} 次，均为 0 缺 0 漏");
        eprintln!(
            "主干自己就没译（分叉刻意不铺）{} 枚:\n{}",
            untranslated.len(),
            untranslated
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
