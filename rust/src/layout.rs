//! #90「三栏拖拽把手 + 右栏 dock」的**纯逻辑**层。
//!
//! 这里只有数值、枚举与一串落盘字节，一个 `View` 都不碰：夹取、拖拽方向、视口推挤、
//! 右栏 dock 的「让位」边界、`layout-columns.json` 的读写 —— 全部可注入数据单测。
//! `main.rs` 只负责把主干 `MakeColumnSplitter` 那一侧的四个指针事件接上（接线片段见
//! `rust/tmp/ly1-report.md` §7，本模块**刻意不画任何可见墨迹**，见 [`SPLITTER_HIT_WIDTH`]）。
//!
//! 规格来源（逐条核对过的工作树行号，2026-09-23）：
//! - 常量 [`SIDEBAR_MIN_WIDTH`] 起 8 颗 = 主干 `MainWindow.LayoutColumns.cs:28-35`
//! - 把手本体 = `MainWindow.LayoutColumns.cs:145-192`（`MakeColumnSplitter`）
//! - 夹取与推挤 = `:198-211`（侧栏）/ `:214-245`（右栏）/ `:247-255`（首开 45%）
//! - dock 同步 = `:257-279`；两格上限 = `:478` + `:618-623`；内层比例 = `:882` / `:892`
//! - 落盘 = `:281-303`（写）/ `:305-330`（读）
//!
//! **DIP / DPI**：本模块所有宽度常量都是主干那批 `double` 的**原值**，即 96 DPI 下的 DIP。
//! 主干交给 XAML 自己做换算；分叉的指针坐标来自 reactor 的 `PointerEventInfo::window_x`
//! （设备像素），**换算归调用方**：进算子前 `px / (dpi as f64 / 96.0)`，出算子后再乘回去。
//! [`clamp_sidebar_width`] 之类一律按「进来的和出去的是同一坐标系」处理，只做取整与夹取。

use std::path::{Path, PathBuf};

use crate::kernel::{data_home_root, DATA_HOME_DIR};

// ------------------------------------------------------------------ 几何常量（DIP）

/// 侧栏下限。主干 `MainWindow.LayoutColumns.cs:28` `SidebarMinWidth = 264`。
pub const SIDEBAR_MIN_WIDTH: f64 = 264.0;
/// 侧栏上限。主干 `:29` `SidebarMaxWidth = 420`。
pub const SIDEBAR_MAX_WIDTH: f64 = 420.0;
/// 主干 `:30` `SidebarDefaultWidth = 280` —— **在主干里是死值**，不要拿它当首启宽度，
/// 判定过程见 `rust/tmp/ly1-report.md` §3。这里保留同名同值只为了「与主干一模一样」的
/// 可追溯性；分叉真正生效的初值是 [`SIDEBAR_INITIAL_WIDTH`]。
pub const SIDEBAR_DEFAULT_WIDTH: f64 = 280.0;
/// 首启实际侧栏宽 = 264：主干 `:91` 用 `Math.Clamp(Nav.OpenPaneLength, 264, 420)` 无条件
/// 覆掉字段初值，而 `MainWindow.xaml:211` 写的是 `OpenPaneLength="264"`。
pub const SIDEBAR_INITIAL_WIDTH: f64 = SIDEBAR_MIN_WIDTH;
/// 右栏下限。主干 `:31` `RightbarMinWidth = 300`。
pub const RIGHTBAR_MIN_WIDTH: f64 = 300.0;
/// 主栏下限（推挤时从视口里扣掉的份）。主干 `:33` `CenterMinWidth = 400`。
pub const CENTER_MIN_WIDTH: f64 = 400.0;
/// 把手命中带宽（DIP）。主干 `:35` `SplitterHitWidth = 8` —— **只决定可点区，不产生墨迹**：
/// 主干那棵 `Border` 的 `Background` 是 `Colors.Transparent`（`:149`）且只挂 4 个指针事件
/// （`:152-190`，没有 `PointerEntered/Exited`）⇒ 可见宽度 0 DIP、rest/hover/pressed 三态逐像素相同。
/// 分叉照抄，**不要**自作主张画一条分隔线。
pub const SPLITTER_HIT_WIDTH: f64 = 8.0;
/// 侧栏/右栏把手那颗 `Margin = (-SplitterHitWidth/2.0, 0, 0, 0)` 的左外溢（主干 `:111` `:125`）。
pub const SPLITTER_EDGE_OVERLAP: f64 = -4.0;
/// 内层分栏把手横向时的左右外溢（主干 `:824` `Margin = (-4, 0, -4, 0)`）。
pub const INNER_SPLITTER_OVERLAP_X: f64 = -4.0;

// ---------------------------------------------------------------- 比例常量（无量纲）

/// 右栏不得超过视口的这一份。主干 `:32` `RightbarMaxRatio = 0.7`（不是 DIP）。
pub const RIGHTBAR_MAX_RATIO: f64 = 0.7;
/// 右栏「尚未定」时首开取视口的这一份。主干 `:34` `RightbarDefaultRatio = 0.45`。
pub const RIGHTBAR_DEFAULT_RATIO: f64 = 0.45;
/// 右栏内层分栏（左右/上下两格）比例的初值。主干 `:74` `_rightInnerSplitRatio = 0.5`，
/// 每次重新分栏复位（`:623`）。
pub const RIGHT_INNER_RATIO_DEFAULT: f64 = 0.5;
/// 内层比例下限。主干 `:882` `Math.Clamp(..., 0.2, 0.8)`。
pub const RIGHT_INNER_RATIO_MIN: f64 = 0.2;
/// 内层比例上限。同上（`:892` 纵向同式）。
pub const RIGHT_INNER_RATIO_MAX: f64 = 0.8;

// --------------------------------------------------------------- 其它与主干同值的钉

/// 主干 `:202` 的回写门槛：与 `Nav.OpenPaneLength` 差**大于** 0.5 才写，等于 0.5 不写。
pub const SIDEBAR_REWRITE_EPSILON: f64 = 0.5;
/// 主干 `:57` `_rightbarWidthPreference = -1` 的「尚未定」哨兵。落盘时 `<= 0` 一律写 0（`:287`）。
pub const RIGHTBAR_UNSET: f64 = -1.0;
/// 右栏 dock 的格数上限（两格）。主干文件头 `:7`「`dockPaneIds.length >= 2` 时禁止再分栏」，
/// 生效点是 `:618-621` 的早退。
pub const RIGHT_DOCK_MAX_PANES: usize = 2;
/// 全屏按钮的字形：未全屏 `\uE740`。主干 `:467` 三元的 `false` 分支。
pub const FULLSCREEN_GLYPH: char = '\u{E740}';
/// 全屏按钮的字形：已全屏 `\uE73F`。主干 `:467` 三元的 `true` 分支。
pub const EXIT_FULLSCREEN_GLYPH: char = '\u{E73F}';

/// 主干 `:37-42` 那颗枚举，照抄三态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightSplitMode {
    /// 没分栏（只有一格）。
    None,
    /// 左右两格（主干注释 `// 左右两格`）。
    Horizontal,
    /// 上下两格（主干注释 `// 上下两格`）。
    Vertical,
}

/// 是哪一条把手 —— 主干两条把手的差别只有**方向**与**基宽来源**（`:106-107` vs `:120-121`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitterEdge {
    /// `LayoutSplitterSidebar`：向右拖变宽。
    Sidebar,
    /// `LayoutSplitterRight`：向左拖变宽。
    Rightbar,
}

impl SplitterEdge {
    /// 主干 `:104` / `:118` 的 `AutomationId`（分叉 UIA 树要出现同名串）。
    pub const fn automation_id(self) -> &'static str {
        match self {
            Self::Sidebar => "LayoutSplitterSidebar",
            Self::Rightbar => "LayoutSplitterRight",
        }
    }

    /// 主干 `:159-161`：按下时的基宽取哪一份（侧栏取偏好、右栏取 `CurrentRightbarWidth()`）。
    /// 分叉调用方按这颗枚举去挑自己那份记账值。
    pub const fn base_width(self, sidebar: f64, rightbar: f64) -> f64 {
        match self {
            Self::Sidebar => sidebar,
            Self::Rightbar => rightbar,
        }
    }

    /// 主干 `:106` `base + dx` / `:120` `base - dx`。
    pub const fn target_width(self, base: f64, dx: f64) -> f64 {
        match self {
            Self::Sidebar => base + dx,
            Self::Rightbar => base - dx,
        }
    }
}

// --------------------------------------------------------- 取整与夹取（.NET 语义复刻）

/// 主干用的是 C# `Math.Round(double)`，默认 **`MidpointRounding.ToEven`（银行家舍入）**，
/// 不是 Rust `f64::round()` 的四舍五入：`264.5 → 264`（不是 265）、`265.5 → 266`（同值但反向）。
/// 主干出现在 `:200` `:225` `:230` `:254`，这里用 Rust 1.77+ 的 `round_ties_even` 逐位对齐。
pub fn round_to_even(px: f64) -> f64 {
    px.round_ties_even()
}

/// `Math.Clamp(Math.Round(px), lo, hi)`（主干 `:200` 的骨架）。
///
/// 用 `f64::max/min`（IEEE `maxNum/minNum`，遇 NaN 取另一操作数）而不是 `f64::clamp`：
/// 后者遇到 NaN 会 **panic**，而主干 `Math.Clamp` 对 NaN 的行为是「塌到下限」。
/// 所以退化输入不会把 GUI 打挂：`NaN → lo`、`-inf → lo`、`+inf → hi`。
pub fn clamp_rounded(px: f64, lo: f64, hi: f64) -> f64 {
    hi.min(px.max(lo))
}

// ---------------------------------------------------------------------- 侧栏那一条把手

/// 主干 `ApplySidebarWidth:198-211` 的取整 + 夹取部分。
pub fn clamp_sidebar_width(px: f64) -> f64 {
    clamp_rounded(round_to_even(px), SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH)
}

/// 侧栏把手的一帧：主干 `:106` `ApplySidebarWidth(_layoutDragBaseWidth + dx)`。
pub fn sidebar_after_drag(base_width: f64, dx: f64) -> f64 {
    clamp_sidebar_width(base_width + dx)
}

/// 主干 `:202` 的「差 >0.5 才回写 `Nav.OpenPaneLength`」。返回 `true` 才允许写 XAML。
/// NaN 参与比较恒 `false` ⇒ 不改（主干同结论，`NaN > 0.5` 也是 `false`）。
pub fn sidebar_should_rewrite_pane(current_pane_length: f64, clamped: f64) -> bool {
    (current_pane_length - clamped).abs() > SIDEBAR_REWRITE_EPSILON
}

// ---------------------------------------------------------------------- 右栏那一条把手

/// 主干 `:216-220` 的视口算式（`ApplyRightbarWidth` 每次自取，`CurrentRightbarWidth:253` 的
/// 兜底支路也在里面）。分叉没有 `ActualWidth`，四个数由调用方从客户区尺寸自己记账。
#[derive(Debug, Clone, Copy)]
pub struct ViewportProbe {
    /// 主干 `ChatPage.ActualWidth`：内容面（不含侧栏）实测宽。
    pub chatpage_width: f64,
    /// 主干 `RightPaneHost.Visibility == Visible`。
    pub right_host_visible: bool,
    /// 主干 `CurrentRightbarWidth()`（仅 `right_host_visible` 时参与求和）。
    pub rightbar_width: f64,
    /// 主干 `RootGrid.ActualWidth`：整窗实测宽（回落支路用）。
    pub root_width: f64,
    /// 主干 `Nav.OpenPaneLength`。
    pub open_pane_length: f64,
}

/// 视口宽：`max(0, chatpage + (host 可见 ? rightbar : 0))`，为 0 才回落
/// `max(0, root - max(0, open_pane))`（主干 `:216-220`，含 `viewport <= 0` 那个判定）。
pub fn rightbar_viewport(probe: &ViewportProbe) -> f64 {
    let viewport = 0.0f64.max(probe.chatpage_width + if probe.right_host_visible { probe.rightbar_width } else { 0.0 });
    if viewport <= 0.0 {
        0.0f64.max(probe.root_width - 0.0f64.max(probe.open_pane_length))
    } else {
        viewport
    }
}

/// 一次右栏应用的结论。主干 `ApplyRightbarWidth` 的两条出口（`:223-228` 早退 / `:229-239` 正常）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RightbarApply {
    /// 记进 `_rightbarWidthPreference` 的新值。
    pub preference: f64,
    /// `true` = 主干那条「available 不足 300 ⇒ 只记偏好、不改宽、右栏让位」（`:223-228`）。
    pub yielded: bool,
    /// 要写给 `RightPaneHost.Width` / `_rightDockRoot.Width` 的值；`None` = 主干**不动** host
    /// （早退，或 host 不可见 / 全屏时 `:232` 的门槛不过）。
    pub host_width: Option<f64>,
}

/// 主干 `ApplyRightbarWidth:214-245` 的纯算式。
///
/// - `available = viewport - 264 - 400`（注意扣的是 **`SIDEBAR_MIN_WIDTH`**，与 `:221` 同）
/// - `available < 300 && !fullscreen` ⇒ `preference = max(300, round(px))`，`yielded = true`，
///   `host_width = None`：**这条就是「右栏 dock 折叠」边界**，拖不动但偏好还留着
/// - 否则 `max = max(300, min(available, viewport*0.7))`、`preference = clamp(round(px), 300, max)`，
///   host 仅在 `host_visible && !fullscreen` 时给值
pub fn apply_rightbar_width(px: f64, viewport: f64, fullscreen: bool, host_visible: bool) -> RightbarApply {
    let available = viewport - SIDEBAR_MIN_WIDTH - CENTER_MIN_WIDTH;
    if available < RIGHTBAR_MIN_WIDTH && !fullscreen {
        return RightbarApply {
            preference: RIGHTBAR_MIN_WIDTH.max(round_to_even(px)),
            yielded: true,
            host_width: None,
        };
    }
    let max = RIGHTBAR_MIN_WIDTH.max(available.min(viewport * RIGHTBAR_MAX_RATIO));
    let clamped = clamp_rounded(round_to_even(px), RIGHTBAR_MIN_WIDTH, max);
    RightbarApply {
        preference: clamped,
        yielded: false,
        host_width: if host_visible && !fullscreen { Some(clamped) } else { None },
    }
}

/// 右栏把手的一帧：主干 `:120` `ApplyRightbarWidth(_layoutDragBaseWidth - dx)`。
pub fn rightbar_after_drag(base_width: f64, dx: f64, viewport: f64, fullscreen: bool, host_visible: bool) -> RightbarApply {
    apply_rightbar_width(base_width - dx, viewport, fullscreen, host_visible)
}

/// 主干 `CurrentRightbarWidth:247-255`：偏好 `> 0` 用偏好，否则 `max(300, round(视口 * 0.45))`。
/// 传进来的 `viewport` 是 `max(0, RootGrid.ActualWidth - max(0, OpenPaneLength))`（`:253`），
/// 侧栏那半已经扣过。
pub fn current_rightbar_width(preference: f64, viewport: f64) -> f64 {
    if preference > 0.0 {
        preference
    } else {
        RIGHTBAR_MIN_WIDTH.max(round_to_even(0.0f64.max(viewport) * RIGHTBAR_DEFAULT_RATIO))
    }
}

/// 主干 `SyncLayoutSplitters:257-279`：两条把手的可见性 + host 宽度。
/// `host_width == None` 对应主干「铺满」那两支：早退分支不写宽度，全屏写 `double.NaN` +
/// `HorizontalAlignment.Stretch`（`:274-278`）—— 分叉调用方把 `None` 翻成 `NaN`。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockSync {
    /// 侧栏条可见 ⟺ `Nav.IsPaneOpen`（`:261`）。
    pub sidebar_splitter_visible: bool,
    /// 右栏条可见 ⟺ host 可见 && 非全屏（`:263-269`）。
    pub rightbar_splitter_visible: bool,
    /// 写给 `RightPaneHost.Width` 的值：`Some(偏好)` 或 `None`（NaN/铺满）。
    pub host_width: Option<f64>,
}

/// `pane_open` = `Nav.IsPaneOpen`；`preference` = `_rightbarWidthPreference`。
pub fn sync_layout(pane_open: bool, host_visible: bool, fullscreen: bool, preference: f64) -> DockSync {
    DockSync {
        sidebar_splitter_visible: pane_open,
        rightbar_splitter_visible: host_visible && !fullscreen,
        host_width: if host_visible && !fullscreen && preference > 0.0 {
            Some(preference)
        } else {
            // 全屏走 `:274-278` 的 NaN+Stretch（这里给 None）；早退/未定则主干**什么都不写**。
            None
        },
    }
}

// ------------------------------------------------------------------------ 拖拽合成入口

/// 一次把手移动的输入（「视口宽 + 当前左右宽 + 指针位移」⇒ 新的左右宽，本模块的唯一门面）。
#[derive(Debug, Clone, Copy)]
pub struct DragInput {
    /// 哪条把手。
    pub edge: SplitterEdge,
    /// [`rightbar_viewport`] 的结果（侧栏把手不用它，但结构体统一给，省调用方分叉）。
    pub viewport: f64,
    /// 当前侧栏宽（= 分叉记账的 `sidebar_w`，主干 `_sidebarWidthPreference`）。
    pub sidebar: f64,
    /// 当前右栏偏好（主干 `_rightbarWidthPreference`；`<= 0` 时先过 [`current_rightbar_width`]）。
    pub rightbar_preference: f64,
    /// 指针相对按下点的位移（root 坐标，见 [`pointer_delta`]）。
    pub dx: f64,
    /// 主干 `_rightFullscreen`。
    pub fullscreen: bool,
    /// 主干 `RightPaneHost.Visibility == Visible`。
    pub host_visible: bool,
}

/// 一帧的结果：两条栏各自的新值（**没被拖的那条原样带回**，主干两条 apply 互不影响）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragResult {
    /// 新的侧栏宽（已 round + clamp 264..420）。
    pub sidebar: f64,
    /// 新的右栏结论。
    pub rightbar: RightbarApply,
    /// `true` = 这帧来自侧栏把手（调用方据此决定 `persist` 落盘落哪一份）。
    pub dragged_sidebar: bool,
}

/// 主干 `MakeColumnSplitter` 的 `onDrag(dx)`（`:152-173`）对应物。
/// **无阈值像素**：任意 1 DIP 位移即改宽（主干 Moved 里没有最小距离判定）。
pub fn drag_columns(input: &DragInput) -> DragResult {
    match input.edge {
        SplitterEdge::Sidebar => DragResult {
            sidebar: sidebar_after_drag(input.sidebar, input.dx),
            rightbar: RightbarApply {
                preference: input.rightbar_preference,
                yielded: false,
                host_width: None,
            },
            dragged_sidebar: true,
        },
        SplitterEdge::Rightbar => DragResult {
            sidebar: input.sidebar,
            rightbar: rightbar_after_drag(
                current_rightbar_width(input.rightbar_preference, input.viewport),
                input.dx,
                input.viewport,
                input.fullscreen,
                input.host_visible,
            ),
            dragged_sidebar: false,
        },
    }
}

/// 主干 `:158` `originX = e.GetCurrentPoint(null).Position.X`（**root 坐标，不是元素坐标**）
/// 与 `:171` `dx = X - originX`。绝对指针 x 形式走这里；非有限值退化成 0（主干 NaN 参与算式后
/// 会被 [`clamp_rounded`] 塌到下限，分叉在算子入口先挡住，行为等价且不 panic）。
pub fn pointer_delta(pointer_x: f64, origin_x: f64) -> f64 {
    let dx = pointer_x - origin_x;
    if dx.is_finite() {
        dx
    } else {
        0.0
    }
}

// ------------------------------------------------------------------------ 右栏 dock 纯判定

/// 主干 `:478` `canSplit = _rightSplitMode == None && _rightTabs.Count > 0` —— 决定两条
/// 「左右分栏 / 上下分栏」菜单项 `IsEnabled`，为假时文案换成 `分栏已满（最多两格）`（`:495-511`）。
pub fn right_dock_can_split(mode: RightSplitMode, tab_count: usize) -> bool {
    mode == RightSplitMode::None && tab_count > 0
}

/// 主干 `SplitRightPane:618-621` 的早退闸（两格上限）：已分栏、或请求 `None` 都不做。
/// 返回 `true` 时调用方才改状态，并把比例复位成 [`RIGHT_INNER_RATIO_DEFAULT`]（`:623`）。
pub fn right_dock_can_split_into(mode: RightSplitMode, requested: RightSplitMode, pane_count: usize) -> bool {
    mode == RightSplitMode::None
        && requested != RightSplitMode::None
        && pane_count < RIGHT_DOCK_MAX_PANES
}

/// 全屏按钮字形（主干 `:467`）。tooltip/Name 在 `全屏`↔`退出全屏` 之间切，同 `:464-466`。
pub fn fullscreen_glyph(fullscreen: bool) -> char {
    if fullscreen {
        EXIT_FULLSCREEN_GLYPH
    } else {
        FULLSCREEN_GLYPH
    }
}

/// 主干内层分栏 `:882`（横向 `dx / ActualWidth`）/ `:892`（纵向 `dy / ActualHeight`）：
/// `clamp(base + delta / extent, 0.2, 0.8)`。`extent <= 0` 时主干整段跳过（`:879` 的 `> 0` 门槛）
/// ⇒ 这里原样返回 `base_ratio`，不产生除零。**比例不落盘**（主干没有它写盘的路径）。
pub fn right_inner_ratio_from_drag(base_ratio: f64, delta_px: f64, extent_px: f64) -> f64 {
    if extent_px > 0.0 {
        let next = base_ratio + delta_px / extent_px;
        RIGHT_INNER_RATIO_MAX.min(next.max(RIGHT_INNER_RATIO_MIN))
    } else {
        base_ratio
    }
}

/// 主干 `:885-886` / `:895-896`：两格拿的是 `ratio*` 与 `(1-ratio)*` 两颗 Star。
pub fn right_inner_ratio_pair(ratio: f64) -> (f64, f64) {
    (ratio, 1.0 - ratio)
}

// ---------------------------------------------------------------------- layout-columns.json

/// 第 5 类壳文件名。主干 `LayoutPrefsPath:297-303`。
pub const PREFS_FILE_NAME: &str = "layout-columns.json";

/// 落盘/读盘的形状载体，字段名与主干 JSON 键**逐字节同小写**（`:287`）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutColumnPrefs {
    /// `_sidebarWidthPreference`（`:56`）。
    pub sidebar: f64,
    /// `_rightbarWidthPreference`（`:57`）。`<= 0` = 尚未定，落盘写成 `0`。
    pub rightbar: f64,
}

impl Default for LayoutColumnPrefs {
    /// 主干无文件时的起法：侧栏 264（`:91` 的 clamp 结果，**不是** 280）、右栏未定（`:57` 的 `-1`）。
    fn default() -> Self {
        Self {
            sidebar: SIDEBAR_INITIAL_WIDTH,
            rightbar: RIGHTBAR_UNSET,
        }
    }
}

impl LayoutColumnPrefs {
    /// 与 `Default` 同义，给不想写 `::default()` 的调用方一个读得通的名字。
    pub const fn defaults() -> Self {
        Self {
            sidebar: SIDEBAR_INITIAL_WIDTH,
            rightbar: RIGHTBAR_UNSET,
        }
    }

    /// 主干 `SaveLayoutColumnPrefs:286-288` 那个 raw string 的产物：**单行**、键名小写、
    /// 数字用不变文化（C# `Double.ToString(InvariantCulture)` = 最短往返十进制，整数不带 `.0`）。
    /// 例：`{"sidebar":344,"rightbar":0}`。
    ///
    /// 右栏 `<= 0` 写 `0` 这件事与主干 `:287` 的三元一致；NaN 会写成 `NaN`（与主干 C# 的
    /// `"NaN"` 同串，两边都产出非法 JSON、都由读侧吞掉 ⇒ 行为等价）。`±inf` 的字符串两边不同
    /// （C# `Infinity` / Rust `inf`），但 [`clamp_rounded`] 与 [`apply_rightbar_width`] 在任何输入下
    /// 都吐有限值，这条分支不可达。
    pub fn json_line(&self) -> String {
        let rightbar = if self.rightbar > 0.0 { self.rightbar } else { 0.0 };
        format!(
            "{{\"sidebar\":{},\"rightbar\":{}}}",
            fmt_invariant(self.sidebar),
            fmt_invariant(rightbar)
        )
    }

    /// 主干 `LoadLayoutColumnPrefs:305-330` 的**逐字段**合并（不是整体替换）：
    /// - `sidebar` 存在且是数字 ⇒ `clamp(v, 264, 420)`（**不 round**，主干同）
    /// - `rightbar` 存在且是数字且 `> 0` ⇒ `max(300, v)`（主干这里不夹上限，上限在
    ///   [`apply_rightbar_width`] 里才生效）
    /// - 缺键 / 类型不对 ⇒ 该字段保持原值；坏 JSON ⇒ 整段吞（调用方看到的就是原值）
    pub fn merge_json_text(&mut self, text: &str) {
        let Ok(root) = serde_json::from_str::<serde_json::Value>(text) else {
            return; // 主干 `catch (Exception) { }`（`:326-329`）
        };
        if let Some(value) = root.get("sidebar").and_then(serde_json::Value::as_f64) {
            self.sidebar = clamp_sidebar_width_no_round(value);
        }
        if let Some(value) = root.get("rightbar").and_then(serde_json::Value::as_f64) {
            if value > 0.0 {
                self.rightbar = RIGHTBAR_MIN_WIDTH.max(value);
            }
        }
    }

    /// 读盘：文件不存在 / 读不动 / JSON 坏 ⇒ 一律按当前值（默认 [`Self::default()`]）起，
    /// 与主干 `:310-312` 的 `File.Exists` 早退 + `:326` 的吞异常同结论。
    pub fn load_from(path: &Path) -> Self {
        let mut prefs = Self::default();
        prefs.load_into(path);
        prefs
    }

    /// 同 [`Self::load_from`]，但合并进调用方已有的记账（主干就是这语义：字段是活的，
    /// `InitRightPaneDock:336` 只是往上盖一层）。返回是否真的读到并改了至少一个字段。
    pub fn load_into(&mut self, path: &Path) -> bool {
        let Ok(text) = std::fs::read_to_string(path) else {
            return false;
        };
        let before = *self;
        self.merge_json_text(&text);
        *self != before
    }

    /// 落盘：先建目录（主干把 `Directory.CreateDirectory` 藏在 `LayoutPrefsPath:301`），
    /// 再 `File.WriteAllText` ⇒ UTF-8 无 BOM、无结尾换行。
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.json_line().as_bytes())
    }
}

/// 主干 `:318` 的 `Math.Clamp(s.GetDouble(), 264, 420)` —— **没有** `Math.Round`，
/// 所以 `{"sidebar":344.5}` 会原样留在偏好里（下次落盘还是 `344.5`）。单独立一个函数，
/// 免得有人哪天「顺手」把 round 加回来。
pub fn clamp_sidebar_width_no_round(px: f64) -> f64 {
    clamp_rounded(px, SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH)
}

/// 真机入口：`%LOCALAPPDATA%\Blade2\layout-columns.json`（主干 `MainWindow.LayoutColumns.cs:297-303`）。
/// 数据家走 `LOCALAPPDATA → data_home_root → kernel 那颗搬迁口 → DATA_HOME_DIR`，与
/// [`crate::shellfiles::data_home`] **同一个目录、同一个口**（这条等价由
/// `shellfiles.rs` 的 `data_home_path_chain_matches_the_layout_precedent` 钉着）。
/// 兜底与 kernel 同一条：拿不到 `LOCALAPPDATA` 退化成 `./Blade2`（既有分叉差异，见 `kernel.rs:266-268`）。
///
/// ⚠ 为什么这条链也要过一次 `Code2 → Blade2` 搬迁（#145 搬迁半；判据是**主干的开机顺序**）：
/// 主干 `LayoutPrefsPath` 自己重拼路径、不碰 `DataHome`，可它**跑在那次搬迁之后** —— 构造函数
/// 第一发 `LoadShellOptions()`（`MainWindow.xaml.cs:2528`）就经
/// `TrayOptionsFile => Path.Combine(DataHome, "shell.json")`（`MainWindow.TraySettings.cs:62`）
/// 同步触发过 getter 里的改名，而 `PostUi(InitLayoutColumns)`（`:2688`）排在构造之后。
/// 分叉实际顺序**相反**：`main.rs:12563 layout::load_prefs()` 早于 :12570 的壳文件读取，
/// 而 lib 写者改不动 main.rs ⇒ 这条链若不搬，升级后首启会读到还没被改名出来的
/// `Blade2\layout-columns.json` ⇒ 列宽白丢一次（主干不丢）。搬迁幂等（新目录已在 ⇒ `Keep`），
/// 挂两条链与挂一条在盘上等价。
/// 仍然**刻意不接** env 覆盖口：主干 `DataHome`（`MainWindow.xaml.cs:8553`）没有 env 口。
pub fn prefs_path() -> PathBuf {
    // 先搬迁（纯判定 + 至多一次 rename，返回值就是下面 `prefs_dir_from` 拼出的那颗家目录），
    // 再走既有那条 `root → DATA_HOME_DIR → PREFS_FILE_NAME` 拼接 —— 形状只留一份真相。
    // 建目录仍留给 [`LayoutColumnPrefs::save_to`]（主干每次调用都建，分叉不跟，见那颗的 doc）。
    let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let root = data_home_root(local_app_data.as_deref());
    crate::kernel::migrate_data_home(&root);
    prefs_dir_from(&root)
}

fn prefs_dir_from(dir: &Path) -> PathBuf {
    dir.join(DATA_HOME_DIR).join(PREFS_FILE_NAME)
}

/// 真机读盘（默认值起，再合并文件）。
pub fn load_prefs() -> LayoutColumnPrefs {
    LayoutColumnPrefs::load_from(&prefs_path())
}

/// 真机落盘。主干 `SaveLayoutColumnPrefs:291-294` 是**吞异常**的（写失败不影响布局本身），
/// 这里同语义；要拿 `io::Result` 的调用方直接用 [`LayoutColumnPrefs::save_to`]。
pub fn save_prefs(prefs: &LayoutColumnPrefs) {
    let _ = prefs.save_to(&prefs_path());
}

/// C# `Double.ToString(InvariantCulture)` 对齐：整数不带 `.0`、小数为最短往返串。
/// Rust 的 `{}` fmt 已经是「最短往返」，两者对 `344.0 → "344"`、`344.5 → "344.5"` 一致。
fn fmt_invariant(value: f64) -> String {
    format!("{value}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 主干 `:200` 的取整是**银行家舍入**，与 Rust `round()` 在 `.5` 上必分歧，钉住。
    #[test]
    fn rounding_is_ties_even_like_dotnet() {
        assert_eq!(round_to_even(344.5), 344.0); // C# Math.Round(344.5) == 344
        assert_eq!(round_to_even(345.5), 346.0); // C# Math.Round(345.5) == 346
        assert_eq!(round_to_even(-264.5), -264.0);
        assert_eq!(round_to_even(344.4), 344.0);
        assert_eq!(round_to_even(344.6), 345.0);
    }

    #[test]
    fn sidebar_clamp_matches_spec_numbers() {
        assert_eq!(clamp_sidebar_width(100.0), 264.0);
        assert_eq!(clamp_sidebar_width(9999.0), 420.0);
        assert_eq!(clamp_sidebar_width(344.4), 344.0);
        assert_eq!(clamp_sidebar_width(263.6), 264.0);
        // 边界值原样通过
        assert_eq!(clamp_sidebar_width(264.0), 264.0);
        assert_eq!(clamp_sidebar_width(420.0), 420.0);
        // 264.5 走 ties-even 落回 264，不是 265
        assert_eq!(clamp_sidebar_width(264.5), 264.0);
    }

    /// 退化输入**不许 panic**（`f64::clamp` 会），主干 `Math.Clamp` 是塌到下限。
    #[test]
    fn degenerate_inputs_never_panic_and_follow_dotnet() {
        assert_eq!(clamp_sidebar_width(f64::NAN), 264.0);
        assert_eq!(clamp_sidebar_width(f64::NEG_INFINITY), 264.0);
        assert_eq!(clamp_sidebar_width(f64::INFINITY), 420.0);
        assert!(sidebar_after_drag(f64::NAN, f64::NAN).is_finite());
        assert_eq!(sidebar_after_drag(300.0, f64::INFINITY), 420.0);
        assert_eq!(pointer_delta(f64::NAN, 0.0), 0.0);
        assert_eq!(pointer_delta(f64::INFINITY, 0.0), 0.0);
        assert_eq!(pointer_delta(740.0, 500.0), 240.0);
        assert_eq!(right_inner_ratio_from_drag(f64::NAN, 1.0, 100.0), 0.2);
        assert_eq!(current_rightbar_width(f64::NAN, f64::NAN), 300.0);
    }

    /// 规格 §1.5 的三条拖拽判据（DIP 口径）。
    #[test]
    fn sidebar_drag_tracks_acceptance_cases() {
        assert_eq!(sidebar_after_drag(264.0, 80.0), 344.0);
        assert_eq!(sidebar_after_drag(264.0, 200.0), 420.0);
        assert_eq!(sidebar_after_drag(264.0, -100.0), 264.0);
        // 方向：侧栏是 base + dx（主干 :106），向右拖变宽
        assert!(SplitterEdge::Sidebar.target_width(300.0, 10.0) > 300.0);
        // 右栏是 base - dx（主干 :120），向左拖变宽
        assert!(SplitterEdge::Rightbar.target_width(300.0, 10.0) < 300.0);
    }

    #[test]
    fn sidebar_rewrite_gate_is_strictly_above_half() {
        assert!(!sidebar_should_rewrite_pane(344.0, 344.0));
        assert!(!sidebar_should_rewrite_pane(344.5, 344.0)); // 差恰为 0.5 ⇒ 不写
        assert!(sidebar_should_rewrite_pane(344.51, 344.0));
        assert!(sidebar_should_rewrite_pane(264.0, 265.0));
        assert!(!sidebar_should_rewrite_pane(f64::NAN, 264.0));
    }

    #[test]
    fn viewport_probe_mirrors_dotnet_branches() {
        // host 可见 ⇒ chatpage + rightbar
        let probe = ViewportProbe {
            chatpage_width: 900.0,
            right_host_visible: true,
            rightbar_width: 320.0,
            root_width: 1500.0,
            open_pane_length: 264.0,
        };
        assert_eq!(rightbar_viewport(&probe), 1220.0);
        // host 不可见 ⇒ 只算 chatpage
        assert_eq!(
            rightbar_viewport(&ViewportProbe { right_host_visible: false, ..probe }),
            900.0
        );
        // chatpage 还没布局出宽度 ⇒ 回落 root - open_pane
        assert_eq!(
            rightbar_viewport(&ViewportProbe {
                chatpage_width: 0.0,
                right_host_visible: false,
                rightbar_width: 0.0,
                root_width: 1264.0,
                open_pane_length: 264.0,
            }),
            1000.0
        );
        // 负的 open_pane 会被 max(0, ..) 兜住（主干 :219 同式）
        assert_eq!(
            rightbar_viewport(&ViewportProbe {
                chatpage_width: 0.0,
                right_host_visible: false,
                rightbar_width: 0.0,
                root_width: 900.0,
                open_pane_length: -50.0,
            }),
            900.0
        );
        // host 可见但 chatpage 只有 rightbar 自己 ⇒ 求和口径（主干 :216 就是这个式子）
        assert_eq!(
            rightbar_viewport(&ViewportProbe {
                chatpage_width: 0.0,
                ..probe
            }),
            320.0
        );
    }

    /// `available = viewport - 264 - 400`；不足 300 且非全屏 ⇒ **让位**：只记偏好、不改宽。
    #[test]
    fn rightbar_yields_when_center_would_starve() {
        // viewport 900 ⇒ available 236 < 300
        let out = apply_rightbar_width(500.0, 900.0, false, true);
        assert!(out.yielded);
        assert_eq!(out.preference, 500.0); // max(300, round(500))
        assert_eq!(out.host_width, None);
        // 偏好太小仍然记成 300（主干 :225 的 Max）
        assert_eq!(apply_rightbar_width(120.0, 900.0, false, true).preference, 300.0);
        // 恰好 available == 300 ⇒ 不让位（主干是 `<`）：viewport 964
        let out = apply_rightbar_width(300.0, 964.0, false, true);
        assert!(!out.yielded);
        assert_eq!(out.host_width, Some(300.0));
    }

    #[test]
    fn fullscreen_bypasses_the_yield_branch_and_host_width() {
        // 同样 viewport 900，但全屏 ⇒ 不走让位支路；max = max(300, min(236, 630)) = 300
        let out = apply_rightbar_width(5000.0, 900.0, true, true);
        assert!(!out.yielded);
        assert_eq!(out.preference, 300.0);
        // 全屏 ⇒ 主干 :232 的门槛不过，不改 host 宽度
        assert_eq!(out.host_width, None);
    }

    /// 上限取 `min(available, viewport*0.7)` 的较小者，再与 300 取较大者。
    #[test]
    fn rightbar_max_is_available_then_ratio_then_floor_300() {
        // viewport 1200：available 536，ratio 840 ⇒ max 536
        assert_eq!(apply_rightbar_width(900.0, 1200.0, false, true).preference, 536.0);
        // viewport 3000：available 2336，ratio 2100 ⇒ ratio 这一支生效
        assert_eq!(apply_rightbar_width(2500.0, 3000.0, false, true).preference, 2100.0);
        // 拖到 0 ⇒ 夹到下限 300
        assert_eq!(apply_rightbar_width(0.0, 3000.0, false, true).preference, 300.0);
        // NaN ⇒ 塌到下限（主干同结论），不 panic
        assert_eq!(apply_rightbar_width(f64::NAN, 3000.0, false, true).preference, 300.0);
        // host 不可见 ⇒ 记偏好但不给宽度
        let out = apply_rightbar_width(400.0, 3000.0, false, false);
        assert_eq!(out.preference, 400.0);
        assert_eq!(out.host_width, None);
    }

    #[test]
    fn rightbar_drag_direction_is_negated() {
        // 主干 :120：向左拖（dx 为负）变宽
        let out = rightbar_after_drag(400.0, -120.0, 3000.0, false, true);
        assert_eq!(out.preference, 520.0);
        // 400 - 120 = 280 < 300 ⇒ 夹回下限（主干 :230 的 Clamp 下限）
        assert_eq!(rightbar_after_drag(400.0, 120.0, 3000.0, false, true).preference, 300.0_f64);
    }

    /// 主干 `CurrentRightbarWidth:247-255`：未定 ⇒ 视口 45%，且下限 300。
    #[test]
    fn current_rightbar_falls_back_to_45_percent() {
        assert_eq!(current_rightbar_width(480.0, 1000.0), 480.0); // 偏好 >0 直接用
        assert_eq!(current_rightbar_width(RIGHTBAR_UNSET, 800.0), 360.0); // round(800*0.45)
        assert_eq!(current_rightbar_width(0.0, 800.0), 360.0);
        assert_eq!(current_rightbar_width(RIGHTBAR_UNSET, 100.0), 300.0); // 夹到下限
        assert_eq!(current_rightbar_width(RIGHTBAR_UNSET, 700.0), 315.0);
        // ties-even：701*0.45 = 315.45 → 315；703*0.45=316.35→316
        assert_eq!(current_rightbar_width(RIGHTBAR_UNSET, 701.0), 315.0);
        assert_eq!(current_rightbar_width(RIGHTBAR_UNSET, f64::NAN), 300.0);
    }

    #[test]
    fn dock_sync_gates_both_splitters() {
        // 侧栏条只看 pane 开合
        assert!(!sync_layout(false, true, false, 400.0).sidebar_splitter_visible);
        let s = sync_layout(true, true, false, 400.0);
        assert!(s.sidebar_splitter_visible);
        assert!(s.rightbar_splitter_visible);
        assert_eq!(s.host_width, Some(400.0));
        // 全屏 ⇒ 右条收起、host 铺满（None = 主干 NaN + Stretch）
        let s = sync_layout(true, true, true, 400.0);
        assert!(!s.rightbar_splitter_visible);
        assert_eq!(s.host_width, None);
        // 偏好未定（<=0）⇒ 不写宽度
        assert_eq!(sync_layout(true, true, false, RIGHTBAR_UNSET).host_width, None);
    }

    #[test]
    fn drag_columns_only_moves_the_dragged_edge() {
        let base = DragInput {
            edge: SplitterEdge::Sidebar,
            viewport: 3000.0,
            sidebar: 264.0,
            rightbar_preference: 400.0,
            dx: 80.0,
            fullscreen: false,
            host_visible: true,
        };
        let out = drag_columns(&base);
        assert_eq!(out.sidebar, 344.0);
        assert_eq!(out.rightbar.preference, 400.0); // 右栏原样带回
        assert!(!out.rightbar.yielded);
        assert!(out.dragged_sidebar);

        // 右栏把手：侧栏不得动
        let out = drag_columns(&DragInput {
            edge: SplitterEdge::Rightbar,
            dx: 100.0,
            ..base
        });
        assert_eq!(out.sidebar, 264.0);
        assert_eq!(out.rightbar.preference, 300.0); // 400 - 100 = 300
        assert!(!out.dragged_sidebar);

        // 偏好未定时基宽走 45% 兜底：viewport 3000 ⇒ 1350，向左拖 150 ⇒ 1500
        let out = drag_columns(&DragInput {
            edge: SplitterEdge::Rightbar,
            rightbar_preference: RIGHTBAR_UNSET,
            dx: -150.0,
            ..base
        });
        assert_eq!(out.rightbar.preference, 1500.0);
    }

    /// 主干内层分栏：`clamp(base + delta/extent, 0.2, 0.8)`，`extent <= 0` 整段跳过。
    #[test]
    fn inner_ratio_clamps_and_ignores_unmeasured_extent() {
        assert_eq!(right_inner_ratio_from_drag(0.5, 100.0, 1000.0), 0.6);
        assert_eq!(right_inner_ratio_from_drag(0.5, -100.0, 1000.0), 0.4);
        assert_eq!(right_inner_ratio_from_drag(0.5, 9999.0, 1000.0), 0.8);
        assert_eq!(right_inner_ratio_from_drag(0.5, -9999.0, 1000.0), 0.2);
        // 未布局（主干 :879 的 `> 0` 门槛）⇒ 原样返回，不做除零
        assert_eq!(right_inner_ratio_from_drag(0.5, 100.0, 0.0), 0.5);
        assert_eq!(right_inner_ratio_from_drag(0.5, 100.0, -1.0), 0.5);
        let (a, b) = right_inner_ratio_pair(0.6);
        assert_eq!((a, b), (0.6, 0.4));
    }

    #[test]
    fn right_dock_split_predicates() {
        assert!(right_dock_can_split(RightSplitMode::None, 1));
        assert!(!right_dock_can_split(RightSplitMode::None, 0)); // 主干 :478 还要求有标签
        assert!(!right_dock_can_split(RightSplitMode::Horizontal, 3));
        assert!(right_dock_can_split_into(RightSplitMode::None, RightSplitMode::Horizontal, 1));
        assert!(!right_dock_can_split_into(RightSplitMode::None, RightSplitMode::None, 1));
        assert!(!right_dock_can_split_into(RightSplitMode::Vertical, RightSplitMode::Horizontal, 2));
        assert!(!right_dock_can_split_into(RightSplitMode::None, RightSplitMode::Horizontal, 2));
        assert_eq!(RIGHT_DOCK_MAX_PANES, 2);
        assert_eq!(fullscreen_glyph(false), '\u{E740}');
        assert_eq!(fullscreen_glyph(true), '\u{E73F}');
        assert_eq!(SplitterEdge::Sidebar.automation_id(), "LayoutSplitterSidebar");
        assert_eq!(SplitterEdge::Rightbar.automation_id(), "LayoutSplitterRight");
    }

    /// 落盘字节形状：**单行、小写键、无 BOM、无结尾换行**。期望串见 ly1-report §5。
    #[test]
    fn json_line_is_one_bomless_newlineless_row() {
        let prefs = LayoutColumnPrefs {
            sidebar: 344.0,
            rightbar: 0.0,
        };
        let line = prefs.json_line();
        assert_eq!(line, "{\"sidebar\":344,\"rightbar\":0}");
        assert_eq!(line.len(), 28);
        assert_eq!(line.as_bytes().len(), 28);
        assert!(!line.contains('\n'));
        assert!(!line.contains('\r'));
        assert!(!line.starts_with('\u{feff}'));
        // 偏好未定 ⇒ 写 0（主干 :287 的三元）
        assert_eq!(LayoutColumnPrefs::default().json_line(), "{\"sidebar\":264,\"rightbar\":0}");
        assert_eq!(
            LayoutColumnPrefs {
                sidebar: 420.0,
                rightbar: -1.0
            }
            .json_line(),
            "{\"sidebar\":420,\"rightbar\":0}"
        );
        // 整数不带 .0；主干读盘不 round，所以小数能原样往返
        assert_eq!(
            LayoutColumnPrefs {
                sidebar: 344.5,
                rightbar: 360.25
            }
            .json_line(),
            "{\"sidebar\":344.5,\"rightbar\":360.25}"
        );
    }

    #[test]
    fn merge_json_text_mirrors_dotnet_reader() {
        // 越界夹取
        let mut p = LayoutColumnPrefs::default();
        p.merge_json_text("{\"sidebar\":100,\"rightbar\":50}");
        assert_eq!((p.sidebar, p.rightbar), (264.0, 300.0));
        let mut p = LayoutColumnPrefs::default();
        p.merge_json_text("{\"sidebar\":9999,\"rightbar\":1e9}");
        assert_eq!((p.sidebar, p.rightbar), (420.0, 1_000_000_000.0));
        // 逐字段：只给 rightbar ⇒ 侧栏保持原值
        let mut p = LayoutColumnPrefs {
            sidebar: 300.0,
            rightbar: -1.0,
        };
        p.merge_json_text("{\"rightbar\":640}");
        assert_eq!((p.sidebar, p.rightbar), (300.0, 640.0));
        // rightbar <= 0 不生效（主干 :321 的 `> 0`）
        let mut p = LayoutColumnPrefs {
            sidebar: 300.0,
            rightbar: 500.0,
        };
        p.merge_json_text("{\"sidebar\":300,\"rightbar\":0}");
        assert_eq!(p.rightbar, 500.0);
        // 类型不对 / 缺键 ⇒ 整段按默认走
        let mut p = LayoutColumnPrefs::default();
        p.merge_json_text("{\"sidebar\":\"300\",\"rightbar\":null}");
        assert_eq!((p.sidebar, p.rightbar), (264.0, -1.0));
        // 坏 JSON ⇒ 吞掉，原值不动
        let mut p = LayoutColumnPrefs {
            sidebar: 388.0,
            rightbar: 404.0,
        };
        p.merge_json_text("{\"sidebar\":");
        assert_eq!((p.sidebar, p.rightbar), (388.0, 404.0));
        // 不 round 这件事单独钉住
        let mut p = LayoutColumnPrefs::default();
        p.merge_json_text("{\"sidebar\":344.5}");
        assert_eq!(p.sidebar, 344.5);
        assert_eq!(p.json_line(), "{\"sidebar\":344.5,\"rightbar\":0}");
    }

    /// 真落盘：目录自建、字节无 BOM 无尾换行、重启读回同值。走临时目录，不碰真 %LOCALAPPDATA%。
    #[test]
    fn save_and_load_round_trip_on_disk() {
        let dir = std::env::temp_dir().join(format!("ly1-layout-{:?}", std::process::id()));
        let path = dir.join(DATA_HOME_DIR).join(PREFS_FILE_NAME);
        let _ = std::fs::remove_dir_all(&dir);

        let prefs = LayoutColumnPrefs {
            sidebar: 344.0,
            rightbar: 0.0,
        };
        prefs.save_to(&path).expect("落盘");
        let bytes = std::fs::read(&path).expect("读回原始字节");
        assert_eq!(bytes.len(), 28);
        assert_ne!(&bytes[0..3], &[0xEF, 0xBB, 0xBF][..], "不许有 BOM");
        assert_ne!(bytes[bytes.len() - 1], b'\n');
        assert_ne!(bytes[bytes.len() - 1], b'\r');
        assert_eq!(&bytes[..], &b"{\"sidebar\":344,\"rightbar\":0}"[..]);

        let back = LayoutColumnPrefs::load_from(&path);
        // 落盘写成 0 的「未定」读回来是 -1：主干 `:321` 要求 `> 0` 才生效 ⇒ 这条往返**故意有损**。
        assert_eq!(back.sidebar, 344.0);
        assert_eq!(back.rightbar, RIGHTBAR_UNSET);
        // 定了的右栏原样往返
        let set = LayoutColumnPrefs {
            sidebar: 344.0,
            rightbar: 404.0,
        };
        set.save_to(&path).expect("落盘（有右栏）");
        assert_eq!(LayoutColumnPrefs::load_from(&path), set);
        assert_eq!(
            std::fs::read_to_string(&path).expect("读串"),
            "{\"sidebar\":344,\"rightbar\":404}"
        );

        // 文件不存在 ⇒ 默认值（264 / -1），不 panic
        let missing = dir.join("nope").join(PREFS_FILE_NAME);
        assert_eq!(LayoutColumnPrefs::load_from(&missing), LayoutColumnPrefs::default());
        assert!(!LayoutColumnPrefs::default().load_into(&missing));

        // 坏 JSON 文件 ⇒ 默认值
        std::fs::write(&path, b"not json at all").expect("写坏文件");
        assert_eq!(LayoutColumnPrefs::load_from(&path), LayoutColumnPrefs::default());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 真机路径拼接：`%LOCALAPPDATA%\Blade2\layout-columns.json`（与 kernel 的数据家同兜底）。
    #[test]
    fn prefs_path_shape() {
        let path = prefs_dir_from(Path::new("C:/fake/LOCALAPPDATA"));
        assert_eq!(
            path.to_string_lossy().replace('\\', "/"),
            "C:/fake/LOCALAPPDATA/Blade2/layout-columns.json"
        );
        // 拿不到 LOCALAPPDATA 的兜底与 kernel::data_home_root 一致（`./Blade2`）
        assert_eq!(data_home_root(None), PathBuf::from("."));
    }

    /// 280 是死值：分叉真正生效的初值必须是 264，且 280 只以「主干字段初值」身份存在。
    #[test]
    fn two_hundred_eighty_is_not_the_live_default() {
        assert_eq!(SIDEBAR_INITIAL_WIDTH, 264.0);
        assert_eq!(LayoutColumnPrefs::default().sidebar, 264.0);
        assert_eq!(SIDEBAR_DEFAULT_WIDTH, 280.0);
        // 主干 :91 的等价式：clamp(OpenPaneLength=264, 264, 420) == 264
        assert_eq!(
            clamp_sidebar_width_no_round(264.0),
            SIDEBAR_INITIAL_WIDTH,
            "280 不该出现在生效路径上"
        );
        // 只有 XAML 哪天改成写 280，初值才会是 280（主干没写）
        assert_eq!(clamp_sidebar_width_no_round(280.0), 280.0);
    }
}
