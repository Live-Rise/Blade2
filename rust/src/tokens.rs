//! 主干 Theme/Tokens.xaml + Typography.xaml 的数值复刻。
//! 单位一律是 DIP（f64），与主干 x:Double / Thickness 一致。

pub mod space {
    pub const S2: f64 = 2.0;
    pub const S4: f64 = 4.0;
    pub const S6: f64 = 6.0;
    pub const S8: f64 = 8.0;
    pub const S10: f64 = 10.0;
    pub const S12: f64 = 12.0;
    pub const S14: f64 = 14.0;
    pub const S16: f64 = 16.0;
    pub const S24: f64 = 24.0;
    pub const S32: f64 = 32.0;
    pub const S48: f64 = 48.0;
}

pub mod radius {
    pub const SMALL: f64 = 4.0;
    pub const MEDIUM: f64 = 8.0;
    pub const LARGE: f64 = 12.0;
    pub const XLARGE: f64 = 16.0;
    pub const PILL: f64 = 24.0;
}

/// [left, top, right, bottom]，对应主干 Thickness 令牌。
pub mod pad {
    pub const PAGE_HEADER: [f64; 4] = [36.0, 24.0, 36.0, 4.0];
    pub const PAGE_HINT: [f64; 4] = [1.0, 10.0, 0.0, 24.0];
    pub const PAGE_SCROLL: [f64; 4] = [36.0, 12.0, 36.0, 36.0];
    pub const CARD_WIDE: [f64; 4] = [16.0, 12.0, 16.0, 12.0];
    pub const CARD_COMPACT: [f64; 4] = [12.0, 8.0, 12.0, 8.0];
    pub const CARD_LARGE: [f64; 4] = [18.0, 18.0, 18.0, 18.0];
    pub const CARD_BORDER: [f64; 4] = [16.0, 5.0, 16.0, 5.0];
    pub const CHIP_COMPACT: [f64; 4] = [10.0, 3.0, 10.0, 3.0];
    pub const CHIP_TIGHT: [f64; 4] = [6.0, 1.0, 6.0, 1.0];
    pub const OVERLAY: [f64; 4] = [12.0, 0.0, 12.0, 8.0];
    pub const OVERLAY_HEADER: [f64; 4] = [12.0, 8.0, 12.0, 4.0];
    pub const OVERLAY_LIST: [f64; 4] = [4.0, 0.0, 4.0, 6.0];
    pub const STRIP: [f64; 4] = [12.0, 2.0, 12.0, 2.0];
    pub const COMPOSER_BAR: [f64; 4] = [16.0, 4.0, 16.0, 12.0];
    pub const COMPOSER_CARD: [f64; 4] = [0.0, 8.0, 4.0, 0.0];
    pub const COMPOSER_ROW: [f64; 4] = [8.0, 2.0, 8.0, 6.0];
    pub const COMPOSER_INPUT: [f64; 4] = [14.0, 4.0, 8.0, 0.0];
    pub const COMPOSER_SELECTORS: [f64; 4] = [20.0, 0.0, 16.0, 2.0];
    pub const BUBBLE_USER: [f64; 4] = [14.0, 9.0, 14.0, 9.0];
    pub const BUBBLE_ASSISTANT: [f64; 4] = [14.0, 10.0, 14.0, 10.0];
    /// `ToolTpl`：工具/系统小字行 `Padding="6,2"`。
    pub const BUBBLE_TOOL: [f64; 4] = [6.0, 2.0, 6.0, 2.0];
    /// `PendingTpl`：「少女祈祷中」行 `Padding="10,6"`。
    pub const PENDING_ROW: [f64; 4] = [10.0, 6.0, 10.0, 6.0];
    pub const SESSION_HEADER: [f64; 4] = [16.0, 10.0, 16.0, 2.0];
    pub const CHAT_COLUMN: [f64; 4] = [12.0, 4.0, 12.0, 4.0];
    pub const LIST_ITEM: [f64; 4] = [0.0, 3.0, 0.0, 3.0];
    /// 助手卡片与 pending 行的模板自带 `Margin="0,0,0,2"`，叠在列表项的 0,3 上。
    pub const LIST_ITEM_TAIL: [f64; 4] = [0.0, 3.0, 0.0, 5.0];
    pub const DIVIDER: [f64; 4] = [-16.0, 4.0, -16.0, 4.0];
    pub const SETTINGS_ROW: [f64; 4] = [0.0, 12.0, 0.0, 12.0];
    pub const INLINE_BUTTON: [f64; 4] = [10.0, 4.0, 10.0, 4.0];
    pub const SECTION_HEADER: [f64; 4] = [1.0, 30.0, 0.0, 6.0];
    pub const EMPTY_STATE: [f64; 4] = [1.0, 8.0, 0.0, 8.0];
    pub const ATTACHMENT_STRIP: [f64; 4] = [14.0, 0.0, 8.0, 0.0];
    /// `NewCard()` 里小节头/说明文字自带的下挂 margin。
    pub const CARD_HEADER: [f64; 4] = [0.0, 10.0, 0.0, 2.0];
    pub const CARD_DESCRIPTION: [f64; 4] = [0.0, 0.0, 0.0, 4.0];
    /// `MakeSettingsNavCard` 的 Button Padding（Sp16/Sp12）。
    pub const NAV_CARD: [f64; 4] = [16.0, 12.0, 16.0, 12.0];
    /// SelectorBar（插件页 Tab）上下各 Space8。
    pub const TAB_BAR: [f64; 4] = [0.0, 8.0, 0.0, 8.0];
}

pub mod size {
    pub const CAPTION_STRIP: f64 = 48.0;
    pub const CAPTION_RESERVED: f64 = 152.0;
    pub const OPEN_PANE: f64 = 264.0;
    pub const COMPACT_PANE: f64 = 40.0;
    pub const WINDOW_MIN_WIDTH: f64 = 560.0;
    pub const WINDOW_MIN_HEIGHT: f64 = 420.0;
    pub const WINDOW_DEFAULT_WIDTH: f64 = 1280.0;
    pub const WINDOW_DEFAULT_HEIGHT: f64 = 800.0;
    pub const COMPACT_PANE_BREAKPOINT: f64 = 720.0;
    pub const TOUCH_TARGET: f64 = 32.0;
    pub const SHELL_BACK_BUTTON: f64 = 32.0;
    pub const STATUS_DOT: f64 = 7.0;
    /// WinUI `NavigationViewItemOnLeftMinHeight`。
    pub const NAV_ITEM_MIN_HEIGHT: f64 = 36.0;
    /// `NavigationViewSelectionIndicatorWidth/Height/Radius`。
    pub const NAV_INDICATOR_WIDTH: f64 = 3.0;
    pub const NAV_INDICATOR_HEIGHT: f64 = 16.0;
    pub const NAV_INDICATOR_RADIUS: f64 = 2.0;
    /// 子代理行的 `row.Margin.Left`。
    pub const SUBAGENT_INDENT: f64 = 14.0;
    /// 展开的工作区节点下，子项再缩进一级（WinUI 的嵌套缩进数值待与主干截图核对）。
    pub const NAV_GROUP_INDENT: f64 = 20.0;
    /// 会话行 `⋯` 钮：24×24、Padding 2、RadiusPill、Opacity 0。
    pub const ROW_MORE_BUTTON: f64 = 24.0;
    pub const BRAND_MARK: f64 = 24.0;
    pub const TOP_BAR_MARK: f64 = 20.0;
    pub const EMPTY_STATE_MARK: f64 = 34.0;
    pub const COMPOSER_ADD_BUTTON: f64 = 28.0;
    pub const COMPOSER_SEND_BUTTON: f64 = 34.0;
    pub const COMPOSER_INPUT_MIN_HEIGHT: f64 = 36.0;
    pub const COMPOSER_INPUT_MAX_HEIGHT: f64 = 160.0;
    pub const BUBBLE_MAX_WIDTH: f64 = 560.0;
    pub const PAGE_MAX_WIDTH: f64 = 1064.0;
    pub const SEARCH_MIN_WIDTH: f64 = 120.0;
    pub const SEARCH_MAX_WIDTH: f64 = 420.0;
    pub const FILES_PANEL_WIDTH: f64 = 340.0;
    pub const TURN_RAIL_WIDTH: f64 = 44.0;
    pub const STROKE: f64 = 1.0;
    pub const SESSION_PREVIEW_LIMIT: usize = 5;
    /// 主干 `FieldMinWidth`：设置页 ComboBox 的下拉框最小宽。
    pub const FIELD_MIN_WIDTH: f64 = 200.0;
    /// 主干 `FieldWidth`：NumberBox / PasswordBox 的 MinWidth=MaxWidth。
    pub const FIELD_WIDTH: f64 = 320.0;
    /// 字号步进器中间的读数块 `MinWidth`，以及会话正文字号的取值区间。
    pub const STEPPER_VALUE_MIN_WIDTH: f64 = 44.0;
    pub const FONT_SIZE_MIN: f64 = 12.0;
    pub const FONT_SIZE_MAX: f64 = 17.0;
    /// pending 行里的 ProgressRing 是 14×14，不是默认 32。
    pub const PENDING_RING: f64 = 14.0;
    /// `MainWindow.Personalization.cs`：指令编辑框 160–320，皮肤滑杆宽 200，读数块 MinWidth 44。
    pub const INSTRUCTIONS_MIN_HEIGHT: f64 = 160.0;
    pub const INSTRUCTIONS_MAX_HEIGHT: f64 = 320.0;
    pub const SKIN_SLIDER_WIDTH: f64 = 200.0;
    pub const SKIN_OPACITY_MIN: f64 = 10.0;
    pub const SKIN_OPACITY_MAX: f64 = 80.0;
    pub const SKIN_OPACITY_STEP: f64 = 5.0;
    pub const SKIN_OPACITY_DEFAULT: f64 = 40.0;
    /// `MainWindow.Pet.cs`：宠物大小滑杆 48–320、步长 8、默认单元格高 128。
    pub const PET_SLIDER_WIDTH: f64 = 220.0;
    pub const PET_CELL_MIN: f64 = 48.0;
    pub const PET_CELL_MAX: f64 = 320.0;
    pub const PET_CELL_STEP: f64 = 8.0;
    pub const PET_CELL_DEFAULT: f64 = 128.0;
    /// `RenderMemoryContentCard`：JSONL 编辑框 320–560。
    pub const MEMORY_MIN_HEIGHT: f64 = 320.0;
    pub const MEMORY_MAX_HEIGHT: f64 = 560.0;
    /// `CompactButtonStyle`：12px 字、Padding 10,4、MinHeight 32、圆角 4。
    pub const COMPACT_BUTTON_MIN_HEIGHT: f64 = 32.0;
    /// 使用统计 KPI 卡的最小高。
    pub const KPI_MIN_HEIGHT: f64 = 104.0;
    /// 气泡操作行的图标按钮：主干 `SmallButtonSize` 28×28 + `RadSmall` 4 + Padding 0。
    pub const SMALL_BUTTON: f64 = 28.0;
    /// 主干反馈组的文本钮（`MakeFeedbackButton`）：Height 28 = `SmallButtonSize`，
    /// Padding 走 `Space8`；`icon_button` 那套负 margin 抵默认内边距时按这两个数算。
    pub const FEEDBACK_BUTTON_PADDING: f64 = 8.0;
    /// 主干 `row.NoteText` 的 `MaxWidth=420`（说明文本单行省略号）。
    pub const FEEDBACK_NOTE_MAX_WIDTH: f64 = 420.0;
    /// 主干反馈说明编辑器：`ContentDialog` 里那颗多行 TextBox 的 Height。
    pub const FEEDBACK_NOTE_EDITOR_HEIGHT: f64 = 120.0;
    /// 操作行的静置透明度：助手 0.35、用户 0.62，悬停/聚焦才提到 1。
    pub const ROW_OPACITY_ASSISTANT: f64 = 0.35;
    pub const ROW_OPACITY_USER: f64 = 0.62;
    /// 主干 `GlyphSizeBody` / `GlyphSizeCaption`，以及思考行箭头用的 10px 档。
    /// reactor 的 FontIcon 没有字号 setter，这三个数落到 `mark_size` 的 Viewbox 方块上。
    pub const GLYPH_BODY: f64 = 14.0;
    pub const GLYPH_CAPTION: f64 = 12.0;
    pub const GLYPH_MINI: f64 = 10.0;
    /// 主干 `NavigationViewItem.Icon` 不写 FontSize（走 FontIcon 默认 20），但 NavigationViewItem
    /// 模板把图标夹在 `Viewbox Height=16` 里 ⇒ 左栏那些图标（新会话/工作区/未分组/设置/展开收起）
    /// 实际墨迹高 16，不是 20。
    pub const GLYPH_NAV: f64 = 16.0;
    /// 主干 `IconButtonStyle`（Theme/Styles.xaml:92）：32×32、Padding 0、CornerSmall。
    pub const ICON_BUTTON: f64 = 32.0;
    /// 主线 `KernelBootPanel` 的 `MaxWidth=360`（MainWindow.xaml:695-709）。
    pub const BOOT_CARD_MAX_WIDTH: f64 = 360.0;
    /// 主线 `KernelBootBar` 显式 `Width=260`（同一个 XAML 段）；值域 0–100。
    pub const BOOT_BAR_WIDTH: f64 = 260.0;
}

/// 主线 `MainWindow.KernelBoot.cs` 的加载卡状态机常量（逐条抄自 KC:24-35）。
/// 百分比表达的是「流程走到哪一步」，**不是耗时估计**：每步在自己的区间内推进，
/// 区间内有真刻度的（插件安装）按真值插值，没有的匀速爬到区间上沿。
pub mod boot {
    /// tick 间隔（主线 `DispatcherQueueTimer.Interval = 100ms`，KC:189）。
    pub const TICK_MS: u64 = 100;
    /// 每 tick 前进上限 1.5%（=15%/秒，KC:34）。
    pub const RATE_FORWARD: f64 = 1.5;
    /// 每 tick 回退上限 3.0%（倒退比前进快一倍，回弹一两 tick 就消失，KC:33-35）。
    pub const RATE_BACK: f64 = 3.0;
    /// `CompleteKernelBootProgress` 后撤卡前的驻留（MW:2628 `Task.Delay(350ms)`）。
    pub const DONE_DWELL_MS: u64 = 350;
    /// 插值收敛阈值：`|target − shown| < 0.05` 直接吸附（KC:221）。
    pub const SNAP: f64 = 0.05;
    /// 阶段总数 = `KernelBootStages.Length`（KC:31）。
    pub const STEP_TOTAL: i32 = 4;
    /// 阶段表 `(StageKey 文案键, Step, From, To)`，逐字对齐 KC:26-29。
    pub const STAGES: [(&str, i32, f64, f64); STEP_TOTAL as usize] = [
        ("正在准备内核组件…", 1, 5.0, 30.0),
        ("正在启动内核…", 2, 30.0, 60.0),
        ("正在连接内核…", 3, 60.0, 82.0),
        ("正在加载工作区与会话…", 4, 82.0, 96.0),
    ];
    /// 上屏瞬间的目标值 = 第 1 步的**下沿** 5（KC:60 `_bootTarget = Stages[0].From`）：
    /// 条子从 0 起，首个 tick 才朝 5 爬；步骤行同时写「第 1 步，共 4 步 · 0%」。
    pub const FIRST_TARGET: f64 = STAGES[0].2;
}

/// Segoe Fluent Icons 码位，取自主干 FontIcon Glyph。
pub mod glyph {
    pub const NEW_SESSION: char = '\u{e710}';
    pub const WORKSPACE: char = '\u{e8b7}';
    pub const UNSORTED: char = '\u{e8a5}';
    /// 轮尾「本轮文件改动」chip 的行头图标（与 `UNSORTED` 同码位，主干按用途各记一份）。
    pub const PRODUCED_FILE: char = '\u{e8a5}';
    /// 主干「展开其余 N 个会话」用 E70D、「收起」用 E70E。
    pub const EXPAND_GROUP: char = '\u{e70d}';
    pub const COLLAPSE_GROUP: char = '\u{e70e}';
    pub const SETTINGS: char = '\u{e713}';
    pub const BACK: char = '\u{e72b}';
    pub const SEARCH: char = '\u{e721}';
    /// 主干 `WorkspaceViewOptionsButton`（`MainWindow.xaml:252`，14px）＝漏斗。
    pub const MORE: char = '\u{e71c}';
    pub const REFRESH: char = '\u{e72c}';
    pub const CLOSE: char = '\u{e711}';
    pub const SEND: char = '\u{e74a}';
    /// 主干运行态把发送键换成停止方块 E71A（`MainWindow.xaml.cs:6887`）。
    pub const STOP: char = '\u{e71a}';
    pub const MORE_12: char = '\u{e712}';
    pub const COPY: char = '\u{e8c8}';
    /// 主干「撤回编辑」钮字形（`MainWindow.MessageActions.cs:131`，14px）。
    pub const WITHDRAW_EDIT: char = '\u{e70f}';
    pub const COPIED: char = '\u{e73e}';
    pub const THUMB_UP: char = '\u{e8e1}';
    pub const THUMB_DOWN: char = '\u{e8e0}';
    pub const THINK: char = '\u{e76b}';
    /// 钻取卡右侧的 chevron：主干 `MainWindow.xaml.cs:9302-9304` 用的是 **E76C**（右向），
    /// 并写明「E76B 是左向，右向才是钻取进下一级的方向」。实测 Segoe Fluent Icons 里
    /// E76B＝ChevronLeft、E76C＝ChevronRight，与主干注释一致 ⇒ 这里必须取 E76C。
    /// 思考折叠箭头是另一套（主干 `MainWindow.MessageActions.cs:445/495` 折叠态字面用 E76B），
    /// 走 `THINK` / `THINK_OPEN`，不要复用本常量。
    pub const CHEVRON_RIGHT: char = '\u{e76c}';
    pub const THINK_OPEN: char = '\u{e76c}';
    /// 主干轮尾用量段的堆叠图标（`MainWindow.xaml.cs:16565`，字号 12、不透明度 .8）。
    pub const USAGE: char = '\u{e81e}';
    pub const GOAL: char = '\u{e78b}';
    pub const PLAN: char = '\u{e9d5}';
    pub const AGENT_MODE: char = '\u{e9d9}';
    pub const PERMISSION_DEFAULT: char = '\u{e72e}';
    pub const PERMISSION_READ_ONLY: char = '\u{e890}';
    pub const PERMISSION_WORKSPACE: char = '\u{e70f}';
    pub const PERMISSION_FULL: char = '\u{e7ba}';
    pub const PERMISSION_AUTO: char = '\u{f1ba}';
    pub const FOLDER: char = '\u{e8b7}';
    pub const FILE: char = '\u{e8a5}';
    pub const OPEN_IN_APP: char = '\u{e8a7}';
    pub const FEEDBACK: char = '\u{e939}';
    pub const FOLDER_EMPTY: char = '\u{e7c3}';
}

/// 主干字阶：FontSize / LineHeight / Weight。
#[derive(Clone, Copy)]
pub struct TypeRamp {
    pub size: f64,
    pub line: f64,
    pub semi_bold: bool,
}

pub mod type_ramp {
    use super::TypeRamp;
    pub const CAPTION: TypeRamp = TypeRamp {
        size: 12.0,
        line: 16.0,
        semi_bold: false,
    };
    pub const BODY: TypeRamp = TypeRamp {
        size: 14.0,
        line: 20.0,
        semi_bold: false,
    };
    pub const BODY_STRONG: TypeRamp = TypeRamp {
        size: 14.0,
        line: 20.0,
        semi_bold: true,
    };
    pub const SUBTITLE: TypeRamp = TypeRamp {
        size: 20.0,
        line: 28.0,
        semi_bold: true,
    };
    pub const TITLE: TypeRamp = TypeRamp {
        size: 28.0,
        line: 36.0,
        semi_bold: true,
    };
    pub const CODE: TypeRamp = TypeRamp {
        size: 12.0,
        line: 16.0,
        semi_bold: false,
    };
    /// 主干字形字号：`GlyphSizeCaption` / `GlyphSizeBody`。主干把它写死在每个 FontIcon 上
    /// （按钮内图标 14、会话行 ⋯ 12），但 `NavigationViewItem.Icon` 那些不写 → 走 WinUI 默认 20，
    /// 再被 NavigationViewItem 模板的 `Viewbox Height=16` 夹到 16。
    /// reactor 0.100 的 `FontIcon` 只有 `glyph`（无 font_size/foreground），`TextBlock` 又没有
    /// `font_family`，所以这些档位一律经 `main.rs::mark_size` 的 Viewbox 落地（见 `size::GLYPH_*`）。
    pub const GLYPH_CAPTION: f64 = 12.0;
    pub const GLYPH_BODY: f64 = 14.0;
    pub const GLYPH_ICON_DEFAULT: f64 = 20.0;
}

/// 进入设置页时主干激活的分区（`ShowSettingsAsync` 末尾 `ActivateSectionAsync("general")`）。
pub const DEFAULT_SETTINGS_SECTION: &str = "general";

/// 主干的 AutomationId 用驼峰组名（`PluginsNav_agentLoop`），而分区/插件 id 是 kebab-case。
pub fn camel(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    let mut up = false;
    for ch in id.chars() {
        if ch == '-' {
            up = true;
        } else if up {
            out.extend(ch.to_uppercase());
            up = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// 插件配置页的钻取卡：(二级页 id, 字形, 标题, 描述)。顺序即主干 `SettingsHost` 追加顺序。
/// id 同时是二级页 id —— 主干 `OpenSettingsSubPage` 每次进页现建内容，不缓存。
pub const SETTINGS_PLUGIN_CARDS: &[(&str, char, &str, &str)] = &[
    ("shell", '\u{e756}', "终端", "命令超时、单流输出上限。"),
    (
        "agent-loop",
        '\u{e72c}',
        "Agent 循环",
        "同一步内最多同时运行多少个可并行的调用。",
    ),
    (
        "subagent",
        '\u{e9d9}',
        "Subagent",
        "控制 Agent 为 Subagent 选择模型的权限。",
    ),
    (
        "web-search",
        '\u{e721}',
        "网页搜索",
        "DeepSeek 搜索提供方：密钥、接口地址、搜索次数。",
    ),
    (
        "auto-approval",
        '\u{e73e}',
        "自动审批",
        "越界请求的自动批准预设与判定模型。",
    ),
    (
        "defaults",
        '\u{ea86}',
        "默认插件",
        "默认插件组合的安装与挂载状态。",
    ),
];

/// 各分区的段说明（主干 `MakeSectionDesc` 的第一颗子元素）。这些串写死在渲染函数里，逐字抄。
pub const DESC_PERSONALIZATION: &str = "指令决定它怎么回应你，材质与皮肤决定它看起来是什么样。";
pub const DESC_ABOUT: &str =
    "Blade² 的版本信息与更新。更新通过覆盖安装新版本 MSIX 完成（安装前需先退出应用）。";
pub const DESC_MODELS: &str = "填入各提供方的 API 密钥即可使用其模型。";
pub const DESC_SKILLS: &str = "Blade² 已安装的全部技能。技能是带说明的指令包：在输入框键入 /名称 即可调用，模型也可能在合适的时机自行调用。";
pub const DESC_MEMORY: &str = "记忆通过内核 dsh-mcp-client 挂载 MCP 参考记忆服务器（@modelcontextprotocol/server-memory），模型可跨会话写入与召回信息。开关编辑内核 profile patch 的 disabled 标志，由内核热重载即时生效。";
pub const DESC_AGENT_PRESETS: &str =
    "预设即一个会话的 Agent 所运行的插件组装——它的工具、提示词与能力。";
pub const DESC_COMPUTER_CONTROL: &str = "电脑控制由内核 dsh-computer-use 注册表与 Cua Driver provider（@deepseek-ai/dsh-experimental-computer-use-cua-driver-mcp）提供：模型可截图并操作鼠标、键盘完成桌面任务。开关编辑内核 profile patch 的 disabled 标志，由内核热重载即时生效。";
pub const DESC_BROWSER_CONTROL: &str = "浏览器控制由内核 dsh-browser-use 注册表与 Playwright provider（@deepseek-ai/dsh-experimental-browser-use-playwright-mcp）提供：模型可打开浏览器完成网页任务。开关编辑内核 profile patch 的 disabled 标志，由内核热重载即时生效。";
pub const DESC_PET: &str = "桌面宠物由内核插件 @linxin666/dsh-pet 提供（Codex Pet 兼容）：聊天窗口里常驻一只宠物，模型干活时它跟着动，点它可以逗一逗。宠物素材装在 $DSH_HOME/pets，重启内核后收录。";

/// `ToolTable`（`MainWindow.MessageActions.cs:635-648`）：(内核名, 字形, 标题, 摘要变体)。
pub const TOOL_TABLE: &[(&str, char, &str, &str)] = &[
    ("bash", '\u{e756}', "Bash", "bash"),
    ("pwsh", '\u{e756}', "Pwsh", "bash"),
    ("read", '\u{e8a5}', "读取", "read"),
    ("read_image", '\u{eb9f}', "读取图片", "read"),
    ("web_fetch", '\u{e774}', "网页获取", "read"),
    ("web_search", '\u{e721}', "网页搜索", "search"),
    ("grep", '\u{e721}', "Grep", "search"),
    ("glob", '\u{e721}', "Glob", "search"),
    ("write", '\u{e70f}', "写入", "write"),
    ("edit", '\u{e70f}', "编辑", "edit"),
    ("run_code", '\u{e943}', "代码", "code"),
];

/// 使用统计 KPI 行：(标题, 字形, automation_id, 无数据提示)。序同主干 `StatsKpiRow` 五列。
pub const STATS_KPI_CARDS: &[(&str, char, &str, &str)] = &[
    (
        "累计 Token 数",
        '\u{e9d2}',
        "StatsKpiTotalTokens",
        "暂无用量记录",
    ),
    (
        "峰值 Token 数",
        '\u{e9d9}',
        "StatsKpiPeakTokens",
        "无完整 turn 记录",
    ),
    (
        "最长聊天时长",
        '\u{e823}',
        "StatsKpiLongestChat",
        "今日暂无记录",
    ),
    (
        "当前连续天数",
        '\u{e787}',
        "StatsKpiCurrentStreak",
        "暂无活跃记录",
    ),
    (
        "最长连续天数",
        '\u{e734}',
        "StatsKpiLongestStreak",
        "暂无活跃记录",
    ),
];

/// 设置页分区：(id, 标题, 字形)。顺序即主干侧栏顺序。
pub const SETTINGS_SECTIONS: &[(&str, &str, char)] = &[
    ("general", "通用设置", '\u{e713}'),
    ("personalization", "个性化", '\u{e790}'),
    ("models", "模型", '\u{e8f1}'),
    ("plugins", "插件", '\u{ea86}'),
    ("skills", "技能", '\u{e734}'),
    ("memory", "记忆", '\u{e81c}'),
    ("agent-presets", "Agent 预设", '\u{e9d9}'),
    ("computer-control", "电脑控制", '\u{e7f4}'),
    ("browser-control", "浏览器控制", '\u{e774}'),
    ("usage", "使用统计", '\u{e9d2}'),
    ("pet", "宠物", '\u{e76e}'),
    ("about", "关于", '\u{e946}'),
];

/// 主干 `RenderGeneralSection()` 的下拉选项，顺序即内核值序。
pub const SETTINGS_PERMISSIONS: &[&str] = &["仅可查看", "工作区内修改", "完全权限"];
/// 主干「窗口材质」下拉，序同 `MainWindow.Personalization.cs` 的 `Materials`。
pub const SETTINGS_MATERIALS: &[&str] = &["Mica", "Mica Alt", "亚克力", "无（纯色）"];
pub const SETTINGS_THEMES: &[&str] = &["浅色", "深色", "跟随系统"];
/// `locale_preference` 的 30 项 native 标签，序同主干 `LocaleLabels`。
pub const SETTINGS_LOCALES: &[&str] = &[
    "简体中文",
    "繁體中文",
    "English",
    "日本語",
    "한국어",
    "Français",
    "Italiano",
    "Deutsch",
    "Español (España)",
    "Dansk",
    "Русский",
    "Türkçe",
    "Norsk",
    "Polski",
    "ไทย",
    "Svenska",
    "Suomi",
    "Nederlands",
    "Português (Brasil)",
    "Português (Portugal)",
    "Español (Latinoamérica)",
    "Українська",
    "Български",
    "Magyar",
    "Bahasa Indonesia",
    "Ελληνικά",
    "Čeština",
    "Română",
    "Tiếng Việt",
    "العربية",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_strip_and_pane_match_mainline() {
        assert_eq!(size::CAPTION_STRIP, 48.0);
        assert_eq!(size::CAPTION_RESERVED, 152.0);
        assert_eq!(size::OPEN_PANE, 264.0);
        assert_eq!(size::COMPACT_PANE_BREAKPOINT, 720.0);
    }

    #[test]
    fn asymmetric_composer_padding_is_preserved() {
        assert_eq!(pad::COMPOSER_CARD, [0.0, 8.0, 4.0, 0.0]);
        assert_eq!(pad::COMPOSER_BAR, [16.0, 4.0, 16.0, 12.0]);
    }

    #[test]
    fn settings_sections_follow_mainline_order() {
        assert_eq!(
            SETTINGS_SECTIONS
                .iter()
                .map(|(id, _, _)| *id)
                .collect::<Vec<_>>(),
            vec![
                "general",
                "personalization",
                "models",
                "plugins",
                "skills",
                "memory",
                "agent-presets",
                "computer-control",
                "browser-control",
                "usage",
                "pet",
                "about",
            ],
        );
        assert_eq!(DEFAULT_SETTINGS_SECTION, SETTINGS_SECTIONS[0].0);
    }

    /// 主干 `PluginsNav_*` 六张钻取卡，顺序即 `SettingsHost.Children` 追加顺序。
    #[test]
    fn plugin_nav_cards_match_mainline_order() {
        assert_eq!(
            SETTINGS_PLUGIN_CARDS
                .iter()
                .map(|(id, _, _, _)| *id)
                .collect::<Vec<_>>(),
            vec![
                "shell",
                "agent-loop",
                "subagent",
                "web-search",
                "auto-approval",
                "defaults",
            ]
        );
        assert_eq!(camel("agent-loop"), "agentLoop");
        assert_eq!(camel("auto-approval"), "autoApproval");
        assert_eq!(camel("shell"), "shell");
    }

    /// 气泡几何取自 Tokens.xaml：BubblePaddingUser/Assistant、ToolTpl 的 6,2、PendingTpl 的 10,6。
    #[test]
    fn bubble_geometry_matches_mainline_tokens() {
        assert_eq!(pad::BUBBLE_USER, [14.0, 9.0, 14.0, 9.0]);
        assert_eq!(pad::BUBBLE_ASSISTANT, [14.0, 10.0, 14.0, 10.0]);
        assert_eq!(pad::BUBBLE_TOOL, [6.0, 2.0, 6.0, 2.0]);
        assert_eq!(pad::PENDING_ROW, [10.0, 6.0, 10.0, 6.0]);
        assert_eq!(pad::LIST_ITEM, [0.0, 3.0, 0.0, 3.0]);
        assert_eq!(size::BUBBLE_MAX_WIDTH, 560.0);
        assert_eq!(size::PENDING_RING, 14.0);
    }

    /// composer 两颗钮的几何：`Theme/Tokens.xaml:72`（Add 28）/ `:91`（Send 34）
    /// + `MainWindow.xaml:1055/1254` 的 `CornerPill` / `AccentButtonStyle` / `GlyphSizeBody`。
    #[test]
    fn composer_button_geometry_matches_mainline_tokens() {
        assert_eq!(size::COMPOSER_ADD_BUTTON, 28.0);
        assert_eq!(size::COMPOSER_SEND_BUTTON, 34.0);
        assert_eq!(radius::PILL, 24.0);
        assert_eq!(size::GLYPH_BODY, 14.0);
        assert_eq!(size::ICON_BUTTON, 32.0);
        assert_eq!(size::GLYPH_NAV, 16.0);
    }

    /// 逐字对照主干 `FontIcon Glyph` 的码位（2026-09-21 用 node 扫全仓 .xaml/.cs 得到的表）。
    /// 这里钉的是「同一个 UI 槽位主干写的是哪个码位」，改任何一个都要先在主干源码里找到出处。
    #[test]
    fn glyph_codepoints_match_mainline_fonticon_glyphs() {
        // MainWindow.xaml:224/266/1068 + xaml.cs:4256/10543 —— 新会话 / 添加工作区 / 添加附件 / 添加提供方
        assert_eq!(glyph::NEW_SESSION, '\u{e710}');
        // MainWindow.xaml:1254（SendButton）+ xaml.cs:6887（running ? E71A : E74A）
        assert_eq!(glyph::SEND, '\u{e74a}');
        assert_eq!(glyph::STOP, '\u{e71a}');
        // MainWindow.xaml:252 视图选项（漏斗）；xaml.cs:3178 会话行 ⋯ 用 E712，不是 E72A
        assert_eq!(glyph::MORE, '\u{e71c}');
        assert_eq!(glyph::MORE_12, '\u{e712}');
        // MainWindow.xaml:280 设置 / :107 顶栏返回 / :187 搜索 / :1748 关闭
        assert_eq!(glyph::SETTINGS, '\u{e713}');
        assert_eq!(glyph::BACK, '\u{e72b}');
        assert_eq!(glyph::SEARCH, '\u{e721}');
        assert_eq!(glyph::CLOSE, '\u{e711}');
        // xaml.cs:3046/3061 展开其余 N 个会话 / 收起
        assert_eq!(glyph::EXPAND_GROUP, '\u{e70d}');
        assert_eq!(glyph::COLLAPSE_GROUP, '\u{e70e}');
        // xaml.cs:2881 工作区节点 / :2934 未分组
        assert_eq!(glyph::WORKSPACE, '\u{e8b7}');
        assert_eq!(glyph::UNSORTED, '\u{e8a5}');
        // xaml.cs:9302 钻取卡 chevron ＝ E76C（右向）；MessageActions.cs:445 思考折叠态字面用 E76B
        assert_eq!(glyph::CHEVRON_RIGHT, '\u{e76c}');
        assert_eq!(glyph::THINK, '\u{e76b}');
        assert_eq!(glyph::THINK_OPEN, '\u{e76c}');
        // MessageActions.cs:249/270/277 复制 ↔ 已复制；:131 撤回编辑；:445/495 思考
        assert_eq!(glyph::COPY, '\u{e8c8}');
        assert_eq!(glyph::COPIED, '\u{e73e}');
        assert_eq!(glyph::WITHDRAW_EDIT, '\u{e70f}');
        // xaml.cs:16565 用量段
        assert_eq!(glyph::USAGE, '\u{e81e}');
        // PermissionPresetGlyph（xaml.cs:15792-15799）
        assert_eq!(glyph::PERMISSION_DEFAULT, '\u{e72e}');
        assert_eq!(glyph::PERMISSION_READ_ONLY, '\u{e890}');
        assert_eq!(glyph::PERMISSION_WORKSPACE, '\u{e70f}');
        assert_eq!(glyph::PERMISSION_FULL, '\u{e7ba}');
        assert_eq!(glyph::PERMISSION_AUTO, '\u{f1ba}');
    }

    /// 分叉用到的码位必须全部落在主干用过的集合里（不许自造字形）。
    #[test]
    fn every_glyph_exists_in_mainline_set() {
        const MAINLINE: &[char] = &[
            '\u{e70d}', '\u{e70e}', '\u{e70f}', '\u{e710}', '\u{e711}', '\u{e712}', '\u{e713}',
            '\u{e71a}', '\u{e71c}', '\u{e721}', '\u{e72b}', '\u{e72c}', '\u{e72e}', '\u{e734}',
            '\u{e73e}', '\u{e74a}', '\u{e74d}', '\u{e756}', '\u{e768}', '\u{e76b}', '\u{e76c}',
            '\u{e76e}', '\u{e774}', '\u{e787}', '\u{e78b}', '\u{e790}', '\u{e7ba}', '\u{e7c3}',
            '\u{e7f4}', '\u{e81c}', '\u{e81e}', '\u{e823}', '\u{e890}', '\u{e897}', '\u{e8a5}',
            '\u{e8a7}', '\u{e8b7}', '\u{e8bd}', '\u{e8c8}', '\u{e8e0}', '\u{e8e1}', '\u{e8f1}',
            '\u{e939}', '\u{e943}', '\u{e946}', '\u{e9ce}', '\u{e9d2}', '\u{e9d5}', '\u{e9d9}',
            '\u{ea86}', '\u{eb9f}', '\u{f1ba}',
        ];
        let used = [
            glyph::NEW_SESSION,
            glyph::WORKSPACE,
            glyph::UNSORTED,
            glyph::PRODUCED_FILE,
            glyph::EXPAND_GROUP,
            glyph::COLLAPSE_GROUP,
            glyph::SETTINGS,
            glyph::BACK,
            glyph::SEARCH,
            glyph::MORE,
            glyph::REFRESH,
            glyph::CLOSE,
            glyph::SEND,
            glyph::STOP,
            glyph::MORE_12,
            glyph::COPY,
            glyph::WITHDRAW_EDIT,
            glyph::COPIED,
            glyph::THUMB_UP,
            glyph::THUMB_DOWN,
            glyph::THINK,
            glyph::USAGE,
            glyph::CHEVRON_RIGHT,
            glyph::THINK_OPEN,
            glyph::GOAL,
            glyph::PLAN,
            glyph::AGENT_MODE,
            glyph::PERMISSION_DEFAULT,
            glyph::PERMISSION_READ_ONLY,
            glyph::PERMISSION_WORKSPACE,
            glyph::PERMISSION_FULL,
            glyph::PERMISSION_AUTO,
            glyph::FOLDER,
            glyph::FILE,
            glyph::OPEN_IN_APP,
            glyph::FEEDBACK,
            glyph::FOLDER_EMPTY,
        ];
        for code in used {
            assert!(
                MAINLINE.contains(&code),
                "U+{:04X} 不在主干 FontIcon 码位表里",
                code as u32
            );
        }
        for (id, code, _, _) in SETTINGS_PLUGIN_CARDS {
            let _ = id;
            assert!(MAINLINE.contains(code), "插件卡字形 U+{:04X} 无主干出处", *code as u32);
        }
        for (name, code, _, _) in TOOL_TABLE {
            let _ = name;
            assert!(MAINLINE.contains(code), "工具表字形 U+{:04X} 无主干出处", *code as u32);
        }
        for (title, code, _, _) in STATS_KPI_CARDS {
            let _ = title;
            assert!(MAINLINE.contains(code), "KPI 字形 U+{:04X} 无主干出处", *code as u32);
        }
        for (id, _, code) in SETTINGS_SECTIONS {
            let _ = id;
            assert!(MAINLINE.contains(code), "分区字形 U+{:04X} 无主干出处", *code as u32);
        }
    }
}
