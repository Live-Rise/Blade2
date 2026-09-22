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
    ("拒绝", "Reject"),
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
    // 目标条 / 上下文圈 / 轮次轨 / 气泡卡 / 附件 / 使用统计：主线 ShellEnglish 已登记的原文照抄。
    // 管理、管理目标、已用 token、第 {0} 轮 主线在代码里用了却没登记进表（英文界面会露出中文），
    // 这 4 条的译文是分叉自拟的。
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
    ("第 {0} 轮", "Turn {0}"),
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

    pub fn l(&self, key: &str) -> String {
        if self.locale == "zh" {
            return key.to_string();
        }
        if let Some(value) = self.table.get(key) {
            return value.clone();
        }
        if self.locale == "en" {
            return EN
                .iter()
                .find(|(from, _)| *from == key)
                .map(|(_, to)| (*to).to_string())
                .unwrap_or_else(|| key.to_string());
        }
        key.to_string()
    }

    pub fn lf(&self, template: &str, args: &[String]) -> String {
        let text = self.l(template);
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
}
