//! 台账 #138「右栏 dock 宿主 + chrome 整块缺」的 **K1 那一刀**：纯 dock 状态机。
//!
//! 这里只有记录、枚举与判据，**不引任何渲染层的类型**（见 `dock_stays_free_of_rendering_types`
//! 那枚自锁）：标签集合、当前标签、分栏方向、内层比例、全屏旗、开合旗、两枚格的存活位、
//! 内层拖拽的在途态 —— 全部可注入数据单测。几何与落点在 `main.rs`（K2–K4），面板内容在 #76。
//!
//! 规格来源 = 主干 `MainWindow.LayoutColumns.cs`（下称 `LC:`，**1008 行**）+ `MainWindow.xaml.cs`
//! （下称 `MW:`）逐条核对（工作树 md5 `78c41be518cda150c70f54e0a40b4037`，2026-09-25 现测）：
//! - 载荷与运行时字段 = `LC:45-52`（标签）+ `LC:56-77`（dock 那 8 枚）
//! - 装配 = `LC:334-397`（`InitRightPaneDock`，种 `tab-0`）
//! - 转移 = `LC:553`（激活）/ `:560`（新标签）/ `:579`（全屏）/ `:616`（分栏）/ `:643`（移到这里）
//!   / `:675`（关闭）/ `:711`（当前格）/ `:717`+`:767`（两枚格的建与销）/ `:740`（第二格自关）
//!   / `:907`（内容同步）/ `:939`（预览落格）/ `:855-905`（内层拖拽）
//! - 两处「两格上限」判据 = 展示侧 `LC:478` + 权威侧 `LC:618-621`
//! - 开合的唯一驱动者 = `MW:17335-17350`（`SetFilesPanelOpen`）
//!
//! **dock 一律不落盘**（DP3 §1.6 逐字证据：`LC:281-295` 那个写盘点只写 `sidebar`/`rightbar`
//! 两枚字段）。本模块连一字节 IO 都没有 —— 有自锁钉着（`dock_touches_no_disk_and_no_prefs`）。
//!
//! 与 [`layout`] 的分工：夹取、取整、比例算子、把手可见性、右栏宽偏好全在 `layout.rs`；
//! 本模块只持「有哪些标签 / 在哪一格 / 当前哪枚 / 分没分 / 全屏没全屏」这一圈**离散状态**，
//! 并且只**调用** `layout::right_dock_can_split*` 与 `layout::right_inner_ratio_*`，
//! **不复制**它们的判据（DP3 §4.2 明令：复制即造第二份真相，还会红 `layout.rs` 自己那族用例）。

use crate::layout::{self, RightSplitMode};

/// 主干 `LC:76` `private string _activeRightTabId = "tab-0";` 的初值，也是装配期种下的
/// 那枚唯一标签的 id（`LC:386`）。**它不占号**：`_rightTabSerial` 从 0 起，第一枚新标签是
/// `tab-1`（`LC:564` 的 `$"tab-{++_rightTabSerial}"` 是前置自增）。
pub const INITIAL_TAB_ID: &str = "tab-0";

/// 主干 `LC:564` / `:627` 的发号格式 `$"tab-{serial}"` 的前缀。
/// 条带上每枚 chip 的 UIA id 是 `RightPaneTab:{id}`（`LC:444`），分叉的键复用同一枚 id。
pub const TAB_ID_PREFIX: &str = "tab-";

/// 主干 `LC:568` / `:631` 里 `TLF("文件 {0}", _rightTabs.Count + 1)` 那个 `{0}` 的算法。
/// 本模块**不碰 i18n**：只把这枚序号算出来交给调用方去格式化（口径同
/// [`Dock::add_tab`] 的闭包入参）。
pub const DEFAULT_TITLE_NUMBER_BASE: u32 = 1;

/// 一格最多能有的分栏方向数上限的补集说明 —— 真正的上限是
/// [`layout::RIGHT_DOCK_MAX_PANES`]（两格），本模块不另立一颗。

// ---------------------------------------------------------------------- 标签载荷

/// 主干 `LC:49` `public string Kind = "files"; // files | preview`。
/// 主干存字符串、比较用 `==`（`LC:660` 的同 Kind 唯一性、`LC:924` 的预览判据）；
/// 分叉收成枚举，[`TabKind::as_str`] 留着给落盘/日志按主干原文比对（**本模块不落盘**）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabKind {
    /// 工作区文件树（主干缺省型，`LC:388`）。
    Files,
    /// 侧边栏预览（`LC:952` 就地改写出来的型）。
    Preview,
}

impl TabKind {
    /// 主干原文那两枚字面量：`"files"` / `"preview"`。
    /// 主干把 `Kind` 存成字符串（`LC:49`），比较用 `==`（`LC:660` 的同 Kind 唯一性、
    /// `LC:924` 的预览判据）；分叉收成枚举，这两枚原文只作比对/日志用。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::Preview => "preview",
        }
    }
}

/// 主干 `LC:45-52` 那颗 `private sealed class RightPaneTab`，五枚字段一字不多不少。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockTab {
    /// `Id`：`tab-{serial}`，装配期那枚是 [`INITIAL_TAB_ID`]。
    pub id: String,
    /// `Title`：条带上显示的文本，同时是 chip 的 UIA Name（`LC:444`）。
    pub title: String,
    /// `Kind`。
    pub kind: TabKind,
    /// `Path`：`string? Path`（`LC:50`），只有预览型会被写（`LC:953`）。
    pub path: Option<String>,
    /// `PaneIndex`：`int`，主干只出现 0 / 1 两值。
    pub pane: u8,
}

impl DockTab {
    /// 主干 `LC:386-389` 装配期那枚的构造：`Kind="files"`、`Path` 留空、格号由入参给。
    fn seeded(id: &str, title: &str, pane: u8) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            kind: TabKind::Files,
            path: None,
            pane,
        }
    }
}

// ------------------------------------------------------------------ 内层拖拽在途态

/// 主干 `WireInnerSplitDrag:857-859` 那三枚被闭包吃掉的局部量
/// （`var dragging = false; Point origin = default; double baseRatio = 0.5;`）。
/// 分叉的 `Msg` 臂必须有一处能存它们 —— 存这里，别在 `main.rs` 再造一份。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InnerDrag {
    /// 拖的是哪个方向（主干 `:879` / `:889` 按 `_rightSplitMode` 分流；按下时快照）。
    pub axis: RightSplitMode,
    /// 按下那一刻的 `_rightInnerSplitRatio`（`:869`）。
    pub base_ratio: f64,
    /// `origin = e.GetCurrentPoint(null).Position` 的 X（`:868`，root 坐标）。
    pub origin_x: f64,
    /// 同上 Y。
    pub origin_y: f64,
}

// ---------------------------------------------------------------------- 内容同步计划

/// [`Dock::content_plan`] 的结果 = 主干 `SyncRightPaneContent:907-936` 里那两发对面板的动作。
/// #76 落地前调用方只可能用到 `pane`（决定哪一格该被渲染成「有内容的那格」）；
/// `preview_path` 那一发对应的就是 `pane.OpenPathInPreviewAsync(path)`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentPlan {
    /// 目标格（主干 `:919` `active.PaneIndex == 1 ? _rightPane1 : _rightPane0`）。
    pub pane: u8,
    /// 预览路径 ⟹ 主干 `:924` `active.Kind == "preview" && active.Path is { Length: > 0 }`。
    /// `None` ⇒ 不发这一发（files 型、或 path 空串都算不发）。
    pub preview_path: Option<String>,
}

// ---------------------------------------------------------------------------- 状态机

/// 主干 dock 的运行时状态本体（`LC:56-77` 里那 8 枚与标签有关的 + `MW:17337` 那枚可见性）。
///
/// **默认值 = 主干字段初始化器的快照**（标签表还是空的，`_activeRightTabId` 已经是 `"tab-0"`，
/// `_rightInnerSplitRatio` 已经是 `0.5`）。真正的装配在 [`Dock::new`]，对应
/// `InitRightPaneDock` 往表里种 `tab-0` 那一步。这一区分是主干原样：
/// 「空 dock」这一态在装配之后不存在（`LC:677` 的 `Count <= 1` 守卫 ⇒ 场上永远 ≥1 枚）。
#[derive(Debug, Clone, PartialEq)]
pub struct Dock {
    /// `_rightTabs`（`LC:75`）。有序；条带顺序即此序（`LC:427` 的 `foreach` 不做排序）。
    pub tabs: Vec<DockTab>,
    /// `_activeRightTabId`（`LC:76`）。
    pub active: String,
    /// `_rightSplitMode`（`LC:72`）。
    pub mode: RightSplitMode,
    /// `_rightInnerSplitRatio`（`LC:74`，初值 0.5，每次重新分栏复位 `:623`）。
    pub inner_ratio: f64,
    /// `_rightFullscreen`（`LC:73`）。
    pub fullscreen: bool,
    /// 分叉记账位 = 主干 `RightPaneHost.Visibility == Visible`（`MW:17337`）。
    /// 主干没有对应字段（它直接读控件），分叉把可见性收进状态机，渲染层现读这颗。
    pub open: bool,
    /// `_rightPane0 is not null`。主干 0 号格**永不销毁**（`DetachRightPaneInstance:769` 只处理
    /// `paneIndex != 1` 早退），这颗装配后恒真。留着是为了让 [`Dock::content_plan`] 的
    /// 第一道守卫（`LC:909`）与主干逐字同形，不是给分叉开第二条路。
    pub pane0_live: bool,
    /// `_rightPane1 is not null`（懒建懒销：`LC:730-733` / `:773-778`）。
    pub pane1_live: bool,
    /// 内层拖拽的在途态（`None` = 主干那个闭包里的 `dragging == false`）。
    pub inner_drag: Option<InnerDrag>,
    /// `_rightTabSerial`（`LC:77`）。**私有**：只准经 [`Dock::next_tab_id`] 递增，
    /// 保证「发号只增不减」这条不变量在类型层面也守得住。
    serial: u32,
}

impl Default for Dock {
    /// 主干字段初始化器（`LC:57/72-77`）的等价物：表空、`active = "tab-0"`、`ratio = 0.5`。
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            active: INITIAL_TAB_ID.to_string(),
            mode: RightSplitMode::None,
            inner_ratio: layout::RIGHT_INNER_RATIO_DEFAULT,
            fullscreen: false,
            open: false,
            pane0_live: false,
            pane1_live: false,
            inner_drag: None,
            serial: 0,
        }
    }
}

impl Dock {
    /// 主干 `InitRightPaneDock:384-391`：装配完的 dock 带着唯一那枚「工作区文件」标签。
    ///
    /// `workspace_files_title` 由调用方从 catalog 取（主干 `L("工作区文件")`）——
    /// 本模块不碰 i18n，见模块头。
    pub fn new(workspace_files_title: impl Into<String>) -> Self {
        let mut dock = Self::default();
        dock.pane0_live = true; // LC:371-381：0 号格从 XAML 收编，与 dock 同时诞生。
        dock.tabs
            .push(DockTab::seeded(INITIAL_TAB_ID, &workspace_files_title.into(), 0));
        dock.active = INITIAL_TAB_ID.to_string(); // LC:391
        dock
    }

    // ---------------------------------------------------------------- 开合（MW:17335）

    /// 主干 `SetFilesPanelOpen(open)` 的状态机半边（`MW:17337`）。
    /// 面板可见性、宽度偏好的懒初始化都在 [`Dock::pane_visibility`] /
    /// [`Dock::rightbar_seed`] 里给回调用方，别在 `main.rs` 重写判据。
    pub fn set_open(&mut self, open: bool) {
        self.open = open;
    }

    /// 主干 `MW:17329` `SetFilesPanelOpen(RightPaneHost.Visibility != Visible)` ——
    /// 「切换」那一发的真值就是翻转后的可见位。
    pub fn toggle_open(&mut self) -> bool {
        self.open = !self.open;
        self.open
    }

    /// 主干 `MW:17339-17340` + `LC:930-934` 两处同式：
    /// `(0 号格该不该显示, 1 号格该不该显示)` = `(open, open && mode != None)`。
    /// #76 落地时这两枚就是两发 `OnVisibilityChanged(bool)` 的实参。
    pub fn pane_visibility(&self) -> (bool, bool) {
        (self.open, self.open && self.mode != RightSplitMode::None)
    }

    /// 主干 `MW:17341-17344`（开栏时）与 `LC:600-603`（退出全屏时）**同一条**懒初始化判据：
    /// `if (open && _rightbarWidthPreference <= 0) pref = max(RightbarMinWidth, CurrentRightbarWidth())`。
    ///
    /// `current` 由调用方给 `layout::current_rightbar_width(pref, viewport)` 的结果；
    /// 返回 `Some(要写进偏好的值)`，`None` = 主干这一支什么都不做。
    pub fn rightbar_seed(open: bool, preference: f64, current: f64) -> Option<f64> {
        if open && preference <= 0.0 {
            Some(layout::RIGHTBAR_MIN_WIDTH.max(current))
        } else {
            None
        }
    }

    // ---------------------------------------------------------------- 标签：查与激活

    /// 按 id 查一枚（主干 `_rightTabs.FirstOrDefault(t => t.Id == id)` 到处都是）。
    pub fn tab_by_id(&self, id: &str) -> Option<&DockTab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    /// 可变版本，给 `main.rs` 读标题/改标题用（主干是直接改类实例字段）。
    pub fn tab_by_id_mut(&mut self, id: &str) -> Option<&mut DockTab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    /// 当前那枚；**不做回落**（回落只发生在 [`Dock::content_plan`]，对应 `LC:913` 的 `??`）。
    pub fn active_tab(&self) -> Option<&DockTab> {
        self.tab_by_id(&self.active)
    }

    /// `RebuildRightTabStrip:427-452` 的铺条顺序 = `tabs` 顺序。别排序。
    pub fn visible_tabs(&self) -> &[DockTab] {
        &self.tabs
    }

    /// 主干 `LC:429` `var active = tab.Id == _activeRightTabId;` —— chip 高亮判据。
    pub fn is_active(&self, id: &str) -> bool {
        self.active == id
    }

    /// 主干 `ActivateRightTab:553-558`：**无任何校验**，`_activeRightTabId = id;` 直写。
    ///
    /// 别「顺手」加 id 存在性校验 —— 加了就与主干不同（DP3 §1.3 那一行标了粗体）。
    /// 主干之所以能这么横，是因为调用点只有 chip 与菜单两处，id 必然来自表内。
    pub fn activate(&mut self, id: &str) {
        self.active = id.to_string();
    }

    /// 主干 `ActiveTabPane:711-715`：找不到当前标签 ⇒ `0`（`tab?.PaneIndex ?? 0`）。
    pub fn active_pane(&self) -> u8 {
        self.active_tab().map(|t| t.pane).unwrap_or(0)
    }

    // ---------------------------------------------------------------- 标签：新建

    /// 发号：`$"tab-{++_rightTabSerial}"`（`LC:564` / `:627`，前置自增 ⇒ 首枚是 `tab-1`）。
    fn next_tab_id(&mut self) -> String {
        self.serial = self.serial.wrapping_add(1);
        format!("{TAB_ID_PREFIX}{}", self.serial)
    }

    /// 下一个默认标题里的 `{0}` = 主干 `_rightTabs.Count + 1`（**新增前**计数）。
    pub fn next_default_title_number(&self) -> u32 {
        self.tabs.len() as u32 + DEFAULT_TITLE_NUMBER_BASE
    }

    /// 主干 `OnRightPaneAddTab:560-577`。
    ///
    /// - 格号 = `mode == None ? 0 : ActiveTabPane()`（`:563`）—— 分栏态下新标签落在**当前格**，
    ///   不是永远落 0 号格。
    /// - 标题由 `default_title(序号)` 现算（主干 `TLF("文件 {0}", Count + 1)`）；本模块只给序号。
    /// - 尾部那发 `SetFilesPanelOpen(true)`（`:576`）= **四类动作自带开栏**之一，所以这里
    ///   直接写 `open = true`。
    ///
    /// 返回新标签 id（主干 `:572` 也拿它当 active），调用方要发文案/发键都用它。
    pub fn add_tab(&mut self, default_title: impl FnOnce(u32) -> String) -> String {
        let pane = if self.mode == RightSplitMode::None {
            0
        } else {
            self.active_pane()
        };
        let number = self.next_default_title_number();
        let id = self.next_tab_id();
        self.tabs.push(DockTab {
            id: id.clone(),
            title: default_title(number),
            kind: TabKind::Files,
            path: None,
            pane,
        });
        self.active = id.clone();
        self.ensure_pane(pane); // :573
        self.open = true; // :576
        id
    }

    // ---------------------------------------------------------------- 两格上限（两处判据）

    /// 当前活着的格数：不分栏 = 1 格，分栏 = 2 格（主干 `_rightSplitMode` 只有这三态）。
    pub fn pane_count(&self) -> usize {
        if self.mode == RightSplitMode::None {
            1
        } else {
            layout::RIGHT_DOCK_MAX_PANES
        }
    }

    /// **展示侧**判据（主干 `LC:478` `canSplit = _rightSplitMode == None && _rightTabs.Count > 0`）：
    /// 决定两条分栏菜单项的 `IsEnabled`，为假时**换文案不换 UIA Name**
    /// （`:495-498` / `:508-511` 在 `Aut(...)` 之后才改 `Text`）。
    ///
    /// 判据本体在 [`layout::right_dock_can_split`]，这里只喂参数 —— **不许**与
    /// [`Dock::can_split`] 合并（合并即造第二份真相，DP3 §1.3 明令两处各照）。
    pub fn can_show_split(&self) -> bool {
        layout::right_dock_can_split(self.mode, self.tabs.len())
    }

    /// **权威侧**判据（主干 `SplitRightPane:618-621` 的早退
    /// `if (_rightSplitMode != None || mode == None) return;`）：
    /// **故意不看标签数** ⇒ 0 枚标签时直接调用照样能开分栏。
    ///
    /// 判据本体在 [`layout::right_dock_can_split_into`]（它还带 `pane_count < 2` 那半截，
    /// 即官方 `dockPaneIds.length >= 2` 短路）。
    pub fn can_split(&self, requested: RightSplitMode) -> bool {
        layout::right_dock_can_split_into(self.mode, requested, self.pane_count())
    }

    /// 分栏满员后那两行菜单要换成「分栏已满（最多两格）」：`!can_show_split()` 的同义句。
    /// 单独留一颗，是为了让 K3/K4 的渲染层不必把判据再抄一遍。
    pub fn split_items_show_full_label(&self) -> bool {
        !self.can_show_split()
    }

    /// 主干 `SplitRightPane:616-641`。
    ///
    /// 走**权威**判据（[`Dock::can_split`]）早退；成功时：
    /// 1. `mode = requested`、`inner_ratio` **复位 0.5**（`:623`，即使之前拖过）；
    /// 2. `EnsureRightPaneInstance(1)`（`:624`）；
    /// 3. **再种一枚新标签进第二格**并置 active（`:627-635`，同一套 `文件 {0}` 标题式）；
    /// 4. 自带开栏（`:640`）。
    ///
    /// 返回「是否真的分了栏」。
    pub fn split(&mut self, requested: RightSplitMode, default_title: impl FnOnce(u32) -> String) -> bool {
        if !self.can_split(requested) {
            return false;
        }
        self.mode = requested; // :622
        self.inner_ratio = layout::RIGHT_INNER_RATIO_DEFAULT; // :623
        self.inner_drag = None; // 重新分栏后在途拖拽作废（主干 handle 在 RebuildRightDockBody 里重建）
        self.ensure_pane(1); // :624
        let number = self.next_default_title_number();
        let id = self.next_tab_id(); // :627
        self.tabs.push(DockTab {
            id: id.clone(),
            title: default_title(number),
            kind: TabKind::Files,
            path: None,
            pane: 1, // :633
        });
        self.active = id; // :635
        self.open = true; // :640
        true
    }

    // ---------------------------------------------------------------- 标签：移到这里

    /// 主干 `TargetMovePane:551`：`_rightSplitMode == None ? 0 : 1`。
    pub fn target_move_pane(&self) -> u8 {
        if self.mode == RightSplitMode::None {
            0
        } else {
            1
        }
    }

    /// 主干 `LC:522` 那行 `IsEnabled`：
    /// `_rightSplitMode != None && tab.PaneIndex != TargetMovePane()`。
    /// 未分栏（K3 期间）恒 `false` —— 那是**自然的禁用形制**，不是缺控件。
    pub fn can_move_here(&self, id: &str) -> bool {
        self.mode != RightSplitMode::None
            && self.tab_by_id(id).is_some_and(|t| t.pane != self.target_move_pane())
    }

    /// 主干 `MoveRightTabHere:643-673`，四道守卫一字不差：
    /// 1. `mode == None` ⇒ 直退；
    /// 2. 表里找不到 ⇒ 直退；
    /// 3. `tab.PaneIndex == target` ⇒ 直退；
    /// 4. **同 Kind 页面唯一性**（官方 `arriving()`）：目标格已有**另一枚**同 Kind ⇒
    ///    把被移动那枚**删掉**、active 指向已有的那枚；否则只换格 + active。
    ///
    /// 主干这里**不**调 `RebuildRightDockBody`（宿主结构没变，只是标签换格），也不开栏。
    pub fn move_here(&mut self, id: &str) {
        if self.mode == RightSplitMode::None {
            return;
        }
        let target = self.target_move_pane();
        let (from_pane, from_kind) = match self.tab_by_id(id) {
            Some(tab) => (tab.pane, tab.kind),
            None => return,
        };
        if from_pane == target {
            return;
        }
        let existing_id = self
            .tabs
            .iter()
            .find(|t| t.pane == target && t.kind == from_kind)
            .map(|t| t.id.clone());
        match existing_id {
            Some(existing) if existing != id => {
                self.tabs.retain(|t| t.id != id); // :663 RemoveAll(t => t.Id == tab.Id)
                self.active = existing; // :664
            }
            _ => {
                if let Some(tab) = self.tab_by_id_mut(id) {
                    tab.pane = target; // :668
                }
                self.active = id.to_string(); // :669
            }
        }
    }

    // ---------------------------------------------------------------- 标签：关闭

    /// 主干 `LC:533` `close.IsEnabled = _rightTabs.Count > 1;` —— 与
    /// [`Dock::close_tab`] 的第一道守卫同源（⇒ 场上永远 ≥1 枚标签）。
    pub fn can_close_tab(&self) -> bool {
        self.tabs.len() > 1
    }

    /// 主干 `CloseRightTab:675-709`：
    /// 1. `tabs.Count <= 1` ⇒ 直退（**不是**「关到空」）；
    /// 2. 找不到 ⇒ 直退；
    /// 3. **空格合并**（官方 `settle`，`:688-702`）：分栏态下任一空格 ⇒ 取消分栏、
    ///    所有标签 `PaneIndex = 0`、销第二格、重铺 body；
    /// 4. 关的正是 active ⇒ 回落 `_rightTabs[0].Id`（**不是**右邻居，主干就是 `[0]`）。
    ///
    /// 这里**不**收起宿主、**不**清 `open`（主干没有这一发；收起只有两条路：菜单再点一次 /
    /// 0 号面板自身的关闭钮）。
    pub fn close_tab(&mut self, id: &str) {
        if self.tabs.len() <= 1 {
            return;
        }
        if self.tab_by_id(id).is_none() {
            return;
        }
        self.tabs.retain(|t| t.id != id); // :686
        if self.mode != RightSplitMode::None {
            let pane0 = self.tabs.iter().filter(|t| t.pane == 0).count();
            let pane1 = self.tabs.iter().filter(|t| t.pane == 1).count();
            if pane0 == 0 || pane1 == 0 {
                // 主干 `LC:692` 就这一行：任一空格 ⇒ 取消分栏。别加第三种判据。
                self.mode = RightSplitMode::None;
                for tab in &mut self.tabs {
                    tab.pane = 0; // :697
                }
                self.detach_pane(1); // :699
                self.inner_drag = None;
            }
        }
        if self.active == id {
            // :703-706 —— 上面 retain 之后 len >= 1，索引 0 安全。
            self.active = self.tabs[0].id.clone();
        }
    }

    // ---------------------------------------------------------------- 全屏

    /// 主干 `OnRightPaneToggleFullscreen:579-613` 的**状态机半边**：`_rightFullscreen = !…`。
    /// 几何（`Grid.SetColumn` / `ColumnSpan` / `Width = NaN`）在 `main.rs`，
    /// 由 [`Dock::host_column_span`] + `layout::sync_layout` 供给；尾部那颗钮的 glyph/名/ToolTip
    /// 走 [`Dock::fullscreen_label_is_exit`] + `layout::fullscreen_glyph`。
    ///
    /// 尾部那发 `SetFilesPanelOpen(true)`（`:612`）照抄 ⇒ 全屏切换自带开栏。
    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        self.open = true; // :612
    }

    /// 主干 `:585-586` / `:597-598`：全屏时宿主挪到第 0 列跨 2 列，退出时回第 1 列跨 1 列。
    /// 分叉 `body()` 第三列的等价刻法是 `grid_column(0).grid_column_span(2)`（DP3 §3.2 已核到
    /// reactor 的属性入口在货架上）。
    pub fn host_column_span(&self) -> (i32, i32) {
        if self.fullscreen {
            (0, 2)
        } else {
            (1, 1)
        }
    }

    /// 尾部 `RightPaneFullscreen` 与菜单 `RightPaneFullscreenItem` 的文案档：
    /// `true` ⇒ 「退出全屏」，`false` ⇒ 「全屏」（主干 `LC:464` / `:527`）。文本本体交给 catalog。
    pub fn fullscreen_label_is_exit(&self) -> bool {
        self.fullscreen
    }

    // ---------------------------------------------------------------- 两枚格的建与销

    /// 主干 `EnsureRightPaneInstance:717-765`。
    ///
    /// ⚠ 主干只判 `paneIndex == 0`，**非 0 一律走第二格**（`LC:730` 起整段没有再比 `paneIndex`）
    /// ⇒ 这里照抄，不加「第三格非法」的校验（两格上限由 [`Dock::can_split`] 那道闸守）。
    ///
    /// 返回 `true` = 这一次真的把第二格建出来（主干 `if (_rightPane1 is not null) return;`）。
    pub fn ensure_pane(&mut self, pane: u8) -> bool {
        if pane == 0 {
            let fresh = !self.pane0_live; // :721 `_rightPane0 ??= ...`
            self.pane0_live = true;
            return fresh;
        }
        if self.pane1_live {
            return false; // :730-733
        }
        self.pane1_live = true; // :734-764（分叉 K4 只建壳；`FilesPanelPane1` 那颗型等 #76）
        true
    }

    /// 主干 `DetachRightPaneInstance:767-779`：`if (paneIndex != 1) return;` ——
    /// **0 号格永不销毁**。返回 `true` = 真的销掉了。
    pub fn detach_pane(&mut self, pane: u8) -> bool {
        if pane != 1 {
            return false;
        }
        let was = self.pane1_live;
        self.pane1_live = false; // :776-778（丢引用；主干还给面板发了一发 OnVisibilityChanged(false)）
        was
    }

    /// 主干 `LC:740-755` 那发 `CloseRequested`（第二格**面板自身**的关闭钮，不是
    /// `RightPaneCloseTab` 那条关标签的通路）：**取消分栏**。
    ///
    /// `if (_rightSplitMode != None)` 守卫 ⇒ mode 复位、所有标签 `PaneIndex = 0`、销第二格、
    /// 重铺 body。**不收起宿主、不删标签** ⇒ 第二格那枚标签留到 0 号格上。
    ///
    /// 返回是否真的做了这一发。
    pub fn close_pane1_requested(&mut self) -> bool {
        if self.mode == RightSplitMode::None {
            return false;
        }
        self.mode = RightSplitMode::None;
        for tab in &mut self.tabs {
            tab.pane = 0;
        }
        self.detach_pane(1);
        self.inner_drag = None;
        true
    }

    // ---------------------------------------------------------------- 内容同步（#76 交接位）

    /// 主干 `SyncRightPaneContent:907-936` 的**纯判据**半边：三道早退 + 预览那一发的入参。
    ///
    /// `LC:928-935` 那两发 `OnVisibilityChanged` 的实参就是 [`Dock::pane_visibility`]，
    /// 分叉别在这里读第二遍真相。
    pub fn content_plan(&self) -> Option<ContentPlan> {
        if !self.pane0_live {
            return None; // LC:909-912
        }
        let active = self.active_tab().or_else(|| self.tabs.first())?; // LC:913-917
        let target_live = if active.pane == 1 {
            self.pane1_live
        } else {
            self.pane0_live
        };
        if !target_live {
            return None; // LC:920-923
        }
        let preview_path = match (&active.kind, &active.path) {
            (TabKind::Preview, Some(path)) if !path.is_empty() => Some(path.clone()), // LC:924
            _ => None,
        };
        Some(ContentPlan {
            pane: active.pane,
            preview_path,
        })
    }

    /// 主干 `OpenPathInRightPaneAsync:939-958`：从交付物「打开」/菜单「在侧边栏预览」落进右栏。
    ///
    /// 顺序照主干：
    /// 1. **第一句** `SetFilesPanelOpen(true)`（`:941`）⇒ 这里直接写 `open`；
    /// 2. `paneIndex = ActiveTabPane()` + `EnsureRightPaneInstance(paneIndex)`（`:942-943`）；
    /// 3. `if (pane is null) return;`（`:945`）—— 分叉在 2 之后该格必活，这一支**不可达**，
    ///    留在此处只为对表；
    /// 4. `preferPreview` ⇒ **就地改写当前 active 那枚**：`Kind = "preview"`、`Path = path`、
    ///    `Title = Path.GetFileName(path.Replace('\\','/'))`（`:950-955`）。
    ///
    /// 返回被改写的那枚（`None` = active 在表里找不到，主干 `tab is not null` 那一支）。
    /// `await pane.OpenPathInPreviewAsync(path)`（`:957`）是 #76 的那一发，调用方按
    /// [`Dock::content_plan`] 发。
    pub fn open_preview(&mut self, path: &str, prefer_preview: bool) -> Option<&mut DockTab> {
        self.open = true; // :941
        let pane = self.active_pane(); // :942
        self.ensure_pane(pane); // :943
        let id = self.active.clone();
        let tab = self.tab_by_id_mut(&id)?; // :949
        if prefer_preview {
            tab.kind = TabKind::Preview; // :952
            tab.path = Some(path.to_string()); // :953
            tab.title = file_name_of(path); // :954
        }
        Some(tab)
    }

    // ---------------------------------------------------------------- 内层分栏拖拽（K4 在途态）

    /// 主干 `WireInnerSplitDrag` 的 `PointerPressed:860-871`。
    ///
    /// 唯一守卫就是 `if (!IsLeftButtonPressed) return;`（`:862`）⇒ 判据由调用方从
    /// `PointerEventInfo` 里读出来传进来；本模块**不**碰事件类型。
    /// 按下**不改**比例，只快照 `origin` 与 `baseRatio`（`:867-869`）。
    pub fn inner_drag_start(&mut self, left_button: bool, x: f64, y: f64) -> bool {
        if !left_button {
            return false;
        }
        self.inner_drag = Some(InnerDrag {
            axis: self.mode,
            base_ratio: self.inner_ratio,
            origin_x: x,
            origin_y: y,
        });
        true
    }

    /// 主干 `PointerMoved:872-899`：`dragging` 为假直接 return；横向用
    /// `dx / ActualWidth`、纵向用 `dy / ActualHeight`（分叉的 extent 由调用方现读 DIP）。
    /// 夹取与 `extent <= 0` 的门槛**全权**交给
    /// [`layout::right_inner_ratio_from_drag`]，本模块不复制算式。
    ///
    /// 返回比例是否变了（主干变完就地改 `ColumnDefinitions[0]/[2]`，不重建）。
    pub fn inner_drag_move(&mut self, x: f64, y: f64, extent_px: f64) -> bool {
        let Some(drag) = self.inner_drag else {
            return false;
        };
        if drag.axis == RightSplitMode::None {
            return false; // :879/:889 两支的 mode 判据都不成立 ⇒ 什么都不做
        }
        let delta = match drag.axis {
            RightSplitMode::Horizontal => x - drag.origin_x,
            RightSplitMode::Vertical => y - drag.origin_y,
            RightSplitMode::None => 0.0,
        };
        let next = layout::right_inner_ratio_from_drag(drag.base_ratio, delta, extent_px);
        if next == self.inner_ratio {
            return false;
        }
        self.inner_ratio = next;
        true
    }

    /// 主干 `PointerReleased:900-904`：只 `ReleasePointerCapture` + `dragging = false`。
    /// **不落盘、没有 CaptureLost 臂**（DP3 §1.3 末行）⇒ 这里除了清在途态什么都不做。
    pub fn inner_drag_end(&mut self) {
        self.inner_drag = None;
    }

    /// 两格拿的是 `ratio*` 与 `(1-ratio)*` 两颗 Star（[`layout::right_inner_ratio_pair`]）。
    pub fn inner_ratio_pair(&self) -> (f64, f64) {
        layout::right_inner_ratio_pair(self.inner_ratio)
    }

    // ---------------------------------------------------------------- 渲染侧要用的字形

    /// 尾部那颗全屏钮的字形（[`layout::fullscreen_glyph`]：`E740` ↔ `E73F`）。
    /// 留这一颗是为了让 K2 不必自己记两枚码位。
    pub fn fullscreen_glyph(&self) -> char {
        layout::fullscreen_glyph(self.fullscreen)
    }
}

// ---------------------------------------------------------------------- 纯函数工具

/// 主干 `LC:954` `System.IO.Path.GetFileName(path.Replace('\\', '/'))`。
///
/// .NET 语义：先把 `\` 全换成 `/`，再取**最后一个 `/` 之后**的子串；没有 `/` 就整串返回。
/// ⇒ 末段为空时返回空串（`"a\b\"` → `""`），空串返回空串。
pub fn file_name_of(path: &str) -> String {
    let slashed = path.replace('\\', "/");
    match slashed.rsplit_once('/') {
        Some((_, name)) => name.to_string(),
        None => slashed,
    }
}

/// 主干 `LC:71` 注释里的格号：0 = 第一格，1 = 第二格。两格上限另有其主
/// （[`layout::RIGHT_DOCK_MAX_PANES`]）。
pub const FIRST_PANE: u8 = 0;
/// 同上，第二格。
pub const SECOND_PANE: u8 = 1;

// ------------------------------------------------------------------ chrome id 登记表
//
// 只**登记契约**、不渲染：这 19 枚 + 菜单顺序是 DP3 §1.4 逐枚数出来的那张表在分叉侧的
// 唯一名册。K2/K3/K4 落 UIA id 时从这里取串，别在 `main.rs` 里手打第二份（那正是
// 台账 #101「自造 id」那一族的成因）。

/// 右栏 chrome 的 UIA id 名册（主干 `MainWindow.LayoutColumns.cs` + `MainWindow.xaml` 原文）。
pub mod chrome {
    /// 1 宿主（主干 `MainWindow.xaml:1354` 的 `x:Name` 派生 id）。
    pub const HOST: &str = "RightPaneHost";
    /// 2 标签条。
    pub const TAB_STRIP: &str = "RightPaneTabStrip";
    /// 3 chip 的 id 前缀：`RightPaneTab:{tab-id}`（`LC:444`）。
    pub const TAB_PREFIX: &str = "RightPaneTab:";
    /// 4 尾部「新标签页」。
    pub const ADD_TAB: &str = "RightPaneAddTab";
    /// 5 尾部「全屏 / 退出全屏」。
    pub const FULLSCREEN: &str = "RightPaneFullscreen";
    /// 6 分栏体。
    pub const DOCK_BODY: &str = "RightPaneDockBody";
    /// 7 第一格的壳。
    pub const CELL0: &str = "RightPaneCell0";
    /// 8 第二格的壳（仅 mode != None 存在）。
    pub const CELL1: &str = "RightPaneCell1";
    /// 9 第二格的面板本体 —— **属于 #76 那颗型，K1–K4 不许挂**（DP3 §1.7 红线 2）。
    pub const PANE1_PANEL: &str = "FilesPanelPane1";
    /// 10 内层分栏把手。
    pub const INNER_SPLITTER: &str = "LayoutSplitterRightInner";
    /// 11 菜单「左右分栏」。
    pub const SPLIT_HORIZONTAL: &str = "RightPaneSplitHorizontal";
    /// 12 菜单「上下分栏」。
    pub const SPLIT_VERTICAL: &str = "RightPaneSplitVertical";
    /// 13 菜单「新标签页」。
    pub const NEW_TAB: &str = "RightPaneNewTab";
    /// 14 菜单「移到这里」。
    pub const MOVE_HERE: &str = "RightPaneMoveHere";
    /// 15 菜单「全屏 / 退出全屏」。
    pub const FULLSCREEN_ITEM: &str = "RightPaneFullscreenItem";
    /// 16 菜单「关闭」。
    pub const CLOSE_TAB: &str = "RightPaneCloseTab";
    /// 17 菜单「打开」—— 与交付物卡 ⋯ 菜单**撞名**（主干既成事实，`LC:536` 注释自认）。
    pub const MENU_OPEN: &str = "FileActionMenuOpen";
    /// 18 菜单「在侧边栏预览」—— 同上撞名族。
    pub const MENU_PREVIEW: &str = "FileActionMenuPreview";
    /// 19 菜单本体（`LC:537`）—— 同上撞名族 ⇒ 按 id 反查必须带祖先作用域。
    pub const MENU: &str = "FileActionMenu";

    /// 19 枚全册（`PANE1_PANEL` 在册但 K1–K4 **不入树**）。
    pub const ALL: [&str; 19] = [
        HOST,
        TAB_STRIP,
        TAB_PREFIX,
        ADD_TAB,
        FULLSCREEN,
        DOCK_BODY,
        CELL0,
        CELL1,
        PANE1_PANEL,
        INNER_SPLITTER,
        SPLIT_HORIZONTAL,
        SPLIT_VERTICAL,
        NEW_TAB,
        MOVE_HERE,
        FULLSCREEN_ITEM,
        CLOSE_TAB,
        MENU_OPEN,
        MENU_PREVIEW,
        MENU,
    ];

    /// 主干 `LC:538-547` 的 8 项 + 2 分隔符顺序；`None` = `MenuFlyoutSeparator`。
    /// K3 落 6 项（两枚分栏项留给 K4，届时 `mode` 恒 None 会造成状态与视图脱节）。
    pub const MENU_ORDER: [Option<&str>; 10] = [
        Some(MENU_OPEN),
        Some(MENU_PREVIEW),
        None,
        Some(SPLIT_HORIZONTAL),
        Some(SPLIT_VERTICAL),
        Some(NEW_TAB),
        Some(MOVE_HERE),
        None,
        Some(FULLSCREEN_ITEM),
        Some(CLOSE_TAB),
    ];
}

// ====================================================================== 离线单测

#[cfg(test)]
mod tests {
    use super::*;

    /// 主干 `TLF("文件 {0}", n)` 的等价：测试里用中文原文，形状与 catalog 一致。
    fn files_title(n: u32) -> String {
        format!("文件 {n}")
    }

    /// 装配后的 dock：等价 `InitRightPaneDock` 跑完。
    fn assembled() -> Dock {
        Dock::new("工作区文件")
    }

    fn ids(dock: &Dock) -> Vec<&str> {
        dock.tabs.iter().map(|t| t.id.as_str()).collect()
    }

    // ---------------------------------------------------------------- 1 装配

    /// DP3 用例 1：`new()` 的全部初值。
    #[test]
    fn new_seeds_one_files_tab_and_every_default_field() {
        let dock = assembled();
        assert_eq!(dock.tabs.len(), 1);
        assert_eq!(dock.tabs[0].id, INITIAL_TAB_ID);
        assert_eq!(dock.tabs[0].title, "工作区文件");
        assert_eq!(dock.tabs[0].kind, TabKind::Files);
        assert_eq!(dock.tabs[0].path, None);
        assert_eq!(dock.tabs[0].pane, FIRST_PANE);
        assert_eq!(dock.active, "tab-0");
        assert_eq!(dock.mode, RightSplitMode::None);
        assert_eq!(dock.inner_ratio, 0.5);
        assert!(!dock.fullscreen);
        assert!(!dock.open); // 主干 XAML 初始 Collapsed
        assert!(dock.pane0_live);
        assert!(!dock.pane1_live);
        assert_eq!(dock.visible_tabs().len(), 1);
        assert!(dock.is_active("tab-0"));
    }

    /// `Default` = **主干字段初始化器**（表还空着），与 `new()`（装配后）两态分得清。
    #[test]
    fn default_is_the_field_initializer_not_the_assembled_state() {
        let bare = Dock::default();
        assert!(bare.tabs.is_empty());
        assert_eq!(bare.active, INITIAL_TAB_ID); // LC:76 初值就是 "tab-0"
        assert_eq!(bare.inner_ratio, layout::RIGHT_INNER_RATIO_DEFAULT); // 0.5，不是 0
        assert!(!bare.pane0_live);
        assert_eq!(bare.content_plan(), None); // LC:909 的 0 号格守卫
    }

    /// 主干 `Kind` 的两枚字面量要能被原文比对（自测脚本与日志按主干串核）。
    #[test]
    fn tab_kind_carries_the_trunk_literals() {
        assert_eq!(TabKind::Files.as_str(), "files");
        assert_eq!(TabKind::Preview.as_str(), "preview");
    }

    // ---------------------------------------------------------------- 2 发号

    /// DP3 用例 2：发号只增不减（`++_rightTabSerial`，关掉的那枚不回收）。
    #[test]
    fn tab_serial_never_rewinds() {
        let mut dock = assembled();
        let first = dock.add_tab(files_title);
        let second = dock.add_tab(files_title);
        assert_eq!(first, "tab-1");
        assert_eq!(second, "tab-2");
        dock.close_tab("tab-2");
        let third = dock.add_tab(files_title);
        assert_eq!(third, "tab-3", "主干是前置自增的号，不是复用空位");
        assert_eq!(ids(&dock), vec!["tab-0", "tab-1", "tab-3"]);
    }

    /// DP3 用例 3：装配那枚 `tab-0` 不占号 ⇒ 第一枚新标签是 `tab-1`。
    #[test]
    fn initial_tab_does_not_consume_a_number() {
        let mut dock = assembled();
        assert_eq!(dock.add_tab(files_title), "tab-1");
    }

    /// DP3 用例 4：`文件 {0}` 的 `{0}` = **新增前** `tabs.Count + 1`。
    #[test]
    fn default_title_number_counts_before_the_insert() {
        let mut dock = assembled();
        assert_eq!(dock.add_tab(files_title), "tab-1"); // 已有 1 枚 ⇒ 文件 2
        assert_eq!(dock.tabs[1].title, "文件 2");
        dock.add_tab(files_title); // 已有 2 枚 ⇒ 文件 3
        assert_eq!(dock.tabs[2].title, "文件 3");
    }

    // ---------------------------------------------------------------- 3 激活

    /// DP3 用例 5：`activate` **无守卫直写**（主干 `LC:553-558` 原样，不校验 id 存在）。
    #[test]
    fn activate_writes_through_without_any_validation() {
        let mut dock = assembled();
        dock.activate("tab-404");
        assert_eq!(dock.active, "tab-404");
        assert_eq!(dock.tabs.len(), 1, "表一字没动");
        assert_eq!(dock.active_tab(), None);
        assert_eq!(dock.active_pane(), 0, "LC:714 的 ?? 0 回落");
    }

    /// 主干 `RebuildRightTabStrip:429` 的高亮判据就是 id 相等。
    #[test]
    fn is_active_tracks_only_the_active_id() {
        let mut dock = assembled();
        let second = dock.add_tab(files_title);
        assert!(dock.is_active(&second));
        dock.activate("tab-0");
        assert!(!dock.is_active(&second));
    }

    // ---------------------------------------------------------------- 4 关闭

    /// DP3 用例 6：只剩 1 枚时 `close_tab` 无操作 ⇒ 不变量 `len >= 1`。
    #[test]
    fn close_tab_refuses_to_empty_the_strip() {
        let mut dock = assembled();
        dock.close_tab("tab-0");
        assert_eq!(dock.tabs.len(), 1);
        assert_eq!(dock.active, "tab-0");
        assert!(!dock.can_close_tab(), "LC:533 的 IsEnabled 同源判据");
    }

    /// 关不存在的 id ⇒ 直退（`LC:682`）。
    #[test]
    fn close_unknown_tab_is_a_no_op() {
        let mut dock = assembled();
        dock.add_tab(files_title);
        let before = dock.clone();
        dock.close_tab("tab-999");
        assert_eq!(dock, before);
    }

    /// DP3 用例 7：关的是 active ⇒ 回落 `tabs[0].id`，**不是**右邻居。
    #[test]
    fn closing_the_active_tab_falls_back_to_the_first_row() {
        let mut dock = assembled();
        dock.add_tab(files_title); // tab-1
        let third = dock.add_tab(files_title); // tab-2
        dock.activate(&third);
        assert_eq!(dock.active, "tab-2");
        dock.close_tab("tab-2");
        assert_eq!(dock.active, "tab-0", "主干 `_rightTabs[0].Id`");
        assert_eq!(ids(&dock), vec!["tab-0", "tab-1"]);
    }

    /// 关非 active ⇒ active 不动。
    #[test]
    fn closing_another_tab_keeps_the_active_one() {
        let mut dock = assembled();
        let second = dock.add_tab(files_title);
        dock.close_tab("tab-0");
        assert_eq!(dock.active, second);
        assert_eq!(ids(&dock), vec!["tab-1"]);
    }

    /// DP3 用例 15（前半）：两格各 1 枚 ⇒ 关掉其中一枚 ⇒ 取消分栏、剩余那枚回 0 号格。
    #[test]
    fn closing_a_tab_that_empties_a_pane_merges_back_to_one_pane() {
        let mut dock = assembled();
        let second = dock.add_tab(files_title);
        assert!(dock.split(RightSplitMode::Horizontal, files_title));
        // 现在：tab-0(格0) / tab-1(格0) / tab-2(格1)
        assert_eq!(dock.tabs.iter().map(|t| t.pane).collect::<Vec<_>>(), vec![0, 0, 1]);
        dock.close_tab(&second); // 关格 0 里的一枚，格 0 仍有 tab-0 ⇒ 不分栏态不变
        assert_eq!(dock.mode, RightSplitMode::Horizontal, "两格都还有标签 ⇒ 保持分栏");
        dock.close_tab("tab-0");
        assert_eq!(dock.mode, RightSplitMode::None, "格 0 空了 ⇒ settle");
        assert!(dock.tabs.iter().all(|t| t.pane == FIRST_PANE));
        assert!(!dock.pane1_live, "LC:699 DetachRightPaneInstance(1)");
    }

    /// DP3 用例 15（后半）：两枚都挤在格 1 ⇒ 同样触发合并。
    #[test]
    fn merge_triggers_when_the_other_pane_is_already_empty() {
        let mut dock = assembled();
        assert!(dock.split(RightSplitMode::Vertical, files_title)); // tab-1 在格 1
        assert_eq!(dock.mode, RightSplitMode::Vertical);
        dock.close_tab("tab-0"); // 关完只剩格 1 那枚 ⇒ pane0 计数 0
        assert_eq!(dock.mode, RightSplitMode::None);
        assert_eq!(dock.tabs[0].pane, FIRST_PANE);
        assert_eq!(dock.active, "tab-1");
    }

    /// 关闭**不**收起宿主（主干没有这一发；`LC:675-709` 里没有 `SetFilesPanelOpen`）。
    #[test]
    fn close_tab_never_collapses_the_host() {
        let mut dock = assembled();
        dock.add_tab(files_title);
        assert!(dock.open);
        dock.close_tab("tab-1");
        assert!(dock.open);
    }

    // ---------------------------------------------------------------- 5 两处两格上限判据

    /// DP3 用例 9：**展示判据 ≠ 权威判据**的活体证据（谁合并判据谁红这一条）。
    #[test]
    fn the_two_split_predicates_disagree_on_purpose() {
        let mut dock = assembled();
        dock.tabs.clear(); // 手工造主干不可达的快照，只为把两枚判据分开
        assert!(!dock.can_show_split(), "LC:478 还要求有标签");
        assert!(dock.can_split(RightSplitMode::Horizontal), "LC:618 故意不看标签数");
        assert!(dock.split_items_show_full_label());
    }

    /// DP3 用例 8：已分栏 ⇒ 再分栏返回 false、状态一字不变（换方向也不行）。
    #[test]
    fn split_is_refused_once_a_pane_pair_exists() {
        let mut dock = assembled();
        assert!(dock.split(RightSplitMode::Horizontal, files_title));
        dock.inner_drag_move(0.0, 0.0, 0.0);
        let before = dock.clone();
        assert!(!dock.split(RightSplitMode::Vertical, files_title));
        assert_eq!(dock, before);
        assert!(!dock.can_split(RightSplitMode::None), "请求 None 也是早退（LC:618 后半）");
    }

    /// DP3 用例 10：分栏成功 ⇒ 新增一枚格 1 且成为 active、比例复位 0.5（即使之前拖过）。
    #[test]
    fn split_seeds_a_second_pane_tab_and_resets_the_ratio() {
        let mut dock = assembled();
        let first = dock.add_tab(files_title);
        assert_eq!(dock.active, first);
        // 手工把比例拖歪
        dock.mode = RightSplitMode::None;
        dock.inner_ratio = 0.77;
        assert!(dock.split(RightSplitMode::Vertical, files_title));
        assert_eq!(dock.mode, RightSplitMode::Vertical);
        assert_eq!(dock.inner_ratio, 0.5, "LC:623 复位");
        assert!(dock.pane1_live, "LC:624 EnsureRightPaneInstance(1)");
        assert_eq!(dock.tabs.len(), 3);
        assert_eq!(dock.tabs[2].pane, SECOND_PANE);
        assert_eq!(dock.tabs[2].title, "文件 3", "新增前 Count + 1");
        assert_eq!(dock.active, "tab-2");
        assert!(dock.open, "LC:640 自带开栏");
    }

    /// 分栏后条带顺序照 `tabs` 序，格号 0/0/1。
    #[test]
    fn split_leaves_strip_order_untouched() {
        let mut dock = assembled();
        dock.split(RightSplitMode::Horizontal, files_title);
        assert_eq!(ids(&dock), vec!["tab-0", "tab-1"]);
        assert_eq!(dock.tabs.iter().map(|t| t.pane).collect::<Vec<_>>(), vec![0, 1]);
    }

    // ---------------------------------------------------------------- 6 移到这里

    /// DP3 用例 11：未分栏 ⇒ 直退，什么都不改。
    #[test]
    fn move_here_is_dead_without_a_split() {
        let mut dock = assembled();
        let second = dock.add_tab(files_title);
        let before = dock.clone();
        dock.move_here(&second);
        assert_eq!(dock, before);
        assert!(!dock.can_move_here(&second));
    }

    /// DP3 用例 12：同 Kind ⇒ 被移动那枚**被删**、active = 目标格已有的那枚。
    #[test]
    fn same_kind_arrival_merges_by_deleting_the_moved_tab() {
        let mut dock = assembled();
        let second = dock.add_tab(files_title); // tab-1 在格 0
        dock.split(RightSplitMode::Horizontal, files_title); // tab-2 在格 1，且 active
        assert_eq!(dock.tab_by_id(&second).unwrap().pane, FIRST_PANE);
        dock.activate(&second);
        dock.move_here(&second); // 目标格 1 已有 files 型 tab-2
        assert_eq!(dock.active, "tab-2", "LC:664 active = 已有的那枚");
        assert_eq!(dock.tabs.len(), 2);
        assert_eq!(ids(&dock), vec!["tab-0", "tab-2"]);
    }

    /// DP3 用例 13：异 Kind ⇒ 只换格 + active，`len` 不变。
    #[test]
    fn different_kind_arrival_only_moves_the_tab() {
        let mut dock = assembled();
        let second = dock.add_tab(files_title);
        dock.split(RightSplitMode::Horizontal, files_title);
        dock.open_preview("D:\\tmp\\note.md", true); // 把格 1 那枚改成 preview 型
        assert_eq!(dock.tab_by_id("tab-2").unwrap().kind, TabKind::Preview);
        dock.activate(&second);
        dock.move_here(&second); // 格 1 只有 preview 型 ⇒ 异 Kind
        assert_eq!(dock.tabs.len(), 3, "没删东西");
        assert_eq!(dock.tab_by_id(&second).unwrap().pane, SECOND_PANE);
        assert_eq!(dock.active, second);
        assert!(dock.can_move_here(&second) == false, "已在目标格 ⇒ 菜单该项置灰");
    }

    /// DP3 用例 14：已在目标格 ⇒ 直退。
    #[test]
    fn move_here_same_pane_is_a_no_op() {
        let mut dock = assembled();
        dock.split(RightSplitMode::Horizontal, files_title);
        let before = dock.clone();
        dock.move_here("tab-2"); // 已在格 1 = target
        assert_eq!(dock, before);
    }

    /// 找不到 id ⇒ 直退（`LC:650`）。
    #[test]
    fn move_unknown_tab_is_a_no_op() {
        let mut dock = assembled();
        dock.split(RightSplitMode::Horizontal, files_title);
        let before = dock.clone();
        dock.move_here("tab-404");
        assert_eq!(dock, before);
    }

    /// `TargetMovePane:551` 两档。
    #[test]
    fn target_move_pane_follows_the_mode() {
        let mut dock = assembled();
        assert_eq!(dock.target_move_pane(), 0);
        dock.split(RightSplitMode::Vertical, files_title);
        assert_eq!(dock.target_move_pane(), 1);
        assert!(dock.can_move_here("tab-0"));
    }

    // ---------------------------------------------------------------- 7 全屏

    /// 全屏只翻旗 + 自带开栏；几何判据给出主干那两档列位。
    #[test]
    fn toggle_fullscreen_flips_the_flag_and_opens_the_host() {
        let mut dock = assembled();
        assert_eq!(dock.host_column_span(), (1, 1));
        assert_eq!(dock.fullscreen_glyph(), layout::FULLSCREEN_GLYPH);
        assert!(!dock.fullscreen_label_is_exit());
        dock.toggle_fullscreen();
        assert!(dock.fullscreen);
        assert!(dock.open, "LC:612 SetFilesPanelOpen(true)");
        assert_eq!(dock.host_column_span(), (0, 2));
        assert_eq!(dock.fullscreen_glyph(), layout::EXIT_FULLSCREEN_GLYPH);
        assert!(dock.fullscreen_label_is_exit());
        dock.toggle_fullscreen();
        assert!(!dock.fullscreen);
        assert_eq!(dock.tabs.len(), 1, "不碰标签");
        assert_eq!(dock.mode, RightSplitMode::None, "不碰分栏");
    }

    /// `pane_visibility` 三档 + 全屏不影响格可见性（主干全屏照旧两格）。
    #[test]
    fn pane_visibility_tracks_open_and_mode() {
        let mut dock = assembled();
        assert_eq!(dock.pane_visibility(), (false, false));
        dock.set_open(true);
        assert_eq!(dock.pane_visibility(), (true, false));
        dock.split(RightSplitMode::Horizontal, files_title);
        assert_eq!(dock.pane_visibility(), (true, true));
        dock.toggle_fullscreen();
        assert_eq!(dock.pane_visibility(), (true, true));
    }

    /// 右栏宽偏好的懒初始化（MW:17341 与 LC:600 同一条判据，`<= 0` 才算未定）。
    #[test]
    fn rightbar_seed_only_fires_when_unset_and_open() {
        assert_eq!(Dock::rightbar_seed(false, -1.0, 500.0), None);
        assert_eq!(Dock::rightbar_seed(true, 420.0, 500.0), None, "已定 ⇒ 不动");
        assert_eq!(Dock::rightbar_seed(true, -1.0, 500.0), Some(500.0));
        assert_eq!(
            Dock::rightbar_seed(true, 0.0, 100.0),
            Some(layout::RIGHTBAR_MIN_WIDTH),
            "max(RightbarMinWidth, current)：current 太小要抬到 300"
        );
    }

    // ---------------------------------------------------------------- 8 预览落格

    /// 主干 `OpenPathInRightPaneAsync`：就地改写 active + 自带开栏 + 标题取末段文件名。
    #[test]
    fn open_preview_rewrites_the_active_tab_in_place() {
        let mut dock = assembled();
        let second = dock.add_tab(files_title);
        dock.activate(&second);
        let before = dock.tabs.len();
        let tab = dock.open_preview("E:\\Syncthing\\DshWinUI\\README.md", true).unwrap();
        assert_eq!(tab.id, second, "改的是 active 那枚，不新增");
        assert_eq!(tab.kind, TabKind::Preview);
        assert_eq!(tab.path.as_deref(), Some("E:\\Syncthing\\DshWinUI\\README.md"));
        assert_eq!(tab.title, "README.md");
        assert_eq!(dock.tabs.len(), before);
        assert!(dock.open, "LC:941 第一句就是开栏");
    }

    /// `preferPreview = false` ⇒ 只开栏、不改写（主干 `:950` 的 `&&`）。
    #[test]
    fn open_preview_without_preference_leaves_the_tab_alone() {
        let mut dock = assembled();
        let title_before = dock.tabs[0].title.clone();
        let tab = dock.open_preview("/tmp/x.txt", false).unwrap();
        assert_eq!(tab.title, title_before);
        assert_eq!(tab.kind, TabKind::Files);
        assert_eq!(tab.path, None);
        assert!(dock.open);
    }

    /// active 不在表里 ⇒ 返回 `None`（主干 `tab is not null` 那一支），但**开栏照样发生**
    /// （主干 `LC:941` 那一发在函数第一句，与 tab 在不在无关）。
    #[test]
    fn open_preview_without_a_live_tab_reports_none() {
        let mut dock = assembled();
        dock.activate("tab-404");
        assert!(!dock.open);
        assert!(dock.open_preview("/tmp/x", true).is_none());
        assert!(dock.open, "LC:941 第一句就是 SetFilesPanelOpen(true)");
    }

    /// `content_plan`：预览型 + 非空 path ⇒ 给路径；files 型或空 path ⇒ 不给。
    #[test]
    fn content_plan_mirrors_sync_right_pane_content() {
        let mut dock = assembled();
        assert_eq!(
            dock.content_plan(),
            Some(ContentPlan {
                pane: 0,
                preview_path: None
            })
        );
        dock.open_preview("a/b/c.txt", true);
        assert_eq!(
            dock.content_plan(),
            Some(ContentPlan {
                pane: 0,
                preview_path: Some("a/b/c.txt".into())
            })
        );
        // 空 path 不发那一发（主干 `is { Length: > 0 }`）
        dock.tab_by_id_mut("tab-0").unwrap().path = Some(String::new());
        assert_eq!(dock.content_plan().unwrap().preview_path, None);
        // active 找不到 ⇒ 回落 tabs.first()（LC:913 的 ??）
        dock.activate("tab-404");
        assert_eq!(dock.content_plan().unwrap().pane, 0);
        // 目标格没建起来 ⇒ 整发早退（LC:920）
        let mut bare = assembled();
        bare.tab_by_id_mut("tab-0").unwrap().pane = SECOND_PANE;
        assert_eq!(bare.content_plan(), None, "格 1 还活着 ⇒ 不该发");
        bare.pane1_live = false;
        assert_eq!(bare.content_plan(), None);
    }

    // ---------------------------------------------------------------- 9 内层比例 × layout 咬合

    /// `inner_drag_*` 与 `layout::right_inner_ratio_from_drag` 的咬合：
    /// 按下只快照、拖动才改、夹取上下限全交给 layout。
    #[test]
    fn inner_drag_delegate_the_clamp_to_layout() {
        let mut dock = assembled();
        // 未分栏 ⇒ 按下的 axis 是 None，move 不做任何事（LC:879/:889 两支都不成立）
        assert!(dock.inner_drag_start(true, 100.0, 100.0));
        assert!(!dock.inner_drag_move(900.0, 900.0, 1000.0));
        assert_eq!(dock.inner_ratio, 0.5);
        dock.inner_drag_end();
        assert_eq!(dock.inner_drag, None);

        assert!(dock.split(RightSplitMode::Horizontal, files_title));
        assert!(dock.inner_drag_start(true, 200.0, 40.0));
        assert_eq!(dock.inner_drag.unwrap().axis, RightSplitMode::Horizontal);
        assert_eq!(dock.inner_drag.unwrap().base_ratio, 0.5);
        assert!(dock.inner_drag_move(300.0, 40.0, 1000.0), "dx=100/1000 ⇒ 0.6");
        assert_eq!(dock.inner_ratio, 0.6);
        assert!(dock.inner_drag_move(-9000.0, 40.0, 1000.0));
        assert_eq!(dock.inner_ratio, layout::RIGHT_INNER_RATIO_MIN);
        assert!(dock.inner_drag_move(90000.0, 40.0, 1000.0));
        assert_eq!(dock.inner_ratio, layout::RIGHT_INNER_RATIO_MAX);
        // extent <= 0 ⇒ layout 原样返回 base（主干 `> 0` 门槛），于是「没变」报 false
        dock.inner_drag_start(true, 0.0, 0.0);
        assert!(!dock.inner_drag_move(500.0, 500.0, 0.0));
    }

    /// 纵向走 dy，横向走 dx —— 同一枚 origin 快照，两档互不串。
    #[test]
    fn inner_drag_axis_picks_the_right_delta() {
        let mut dock = assembled();
        dock.split(RightSplitMode::Vertical, files_title);
        assert!(dock.inner_drag_start(true, 10.0, 10.0));
        assert!(dock.inner_drag_move(9999.0, 110.0, 100.0), "只认 dy=100 ⇒ +1.0 后夹到 0.8");
        assert_eq!(dock.inner_ratio, layout::RIGHT_INNER_RATIO_MAX);
    }

    /// 抬手**不落盘**（主干只解捕获）；`inner_drag_start` 的左键判据。
    #[test]
    fn inner_drag_has_no_release_side_effects_and_needs_the_left_button() {
        let mut dock = assembled();
        dock.split(RightSplitMode::Horizontal, files_title);
        assert!(!dock.inner_drag_start(false, 0.0, 0.0), "LC:862 非左键直接 return");
        assert_eq!(dock.inner_drag, None);
        dock.inner_drag_start(true, 0.0, 0.0);
        let before = dock.clone();
        dock.inner_drag_end();
        assert_eq!(dock.inner_ratio, before.inner_ratio, "抬手不改比例");
        assert_eq!(dock.open, before.open, "抬手既不开栏也不关栏（更不落盘）");
        assert_eq!(dock.mode, before.mode, "也不取消分栏");
    }

    /// `inner_ratio_pair` 直通 layout（两格 Star 的 `r` 与 `1-r`）。
    #[test]
    fn inner_ratio_pair_comes_from_layout() {
        let mut dock = assembled();
        dock.split(RightSplitMode::Horizontal, files_title);
        assert_eq!(dock.inner_ratio_pair(), (0.5, 0.5));
        dock.inner_ratio = 0.3;
        let (a, b) = dock.inner_ratio_pair();
        assert_eq!((a, b), layout::right_inner_ratio_pair(0.3));
        assert!((a + b - 1.0).abs() < 1e-12);
    }

    /// 重新分栏后在途拖拽作废（壳重建，主干 handle 是新造的）。
    #[test]
    fn split_invalidates_an_in_flight_drag() {
        let mut dock = assembled();
        dock.mode = RightSplitMode::None;
        dock.inner_drag = Some(InnerDrag {
            axis: RightSplitMode::None,
            base_ratio: 0.42,
            origin_x: 1.0,
            origin_y: 2.0,
        });
        dock.split(RightSplitMode::Horizontal, files_title);
        assert_eq!(dock.inner_drag, None);
    }

    // ---------------------------------------------------------------- 10 两枚格的建与销

    /// 主干 `EnsureRightPaneInstance` / `DetachRightPaneInstance` 的守卫逐条。
    #[test]
    fn pane_ensure_and_detach_copy_the_trunk_guards() {
        let mut dock = assembled();
        assert!(!dock.ensure_pane(0), "0 号格已在 ⇒ `??=` 不算新建");
        assert!(dock.pane0_live);
        assert!(dock.detach_pane(0) == false, "LC:769 `paneIndex != 1` 直接 return");
        assert!(dock.pane0_live, "0 号格永不销毁");
        assert!(dock.ensure_pane(1));
        assert!(!dock.ensure_pane(1), "LC:730 已存在 ⇒ 幂等");
        assert!(dock.detach_pane(1));
        assert!(!dock.detach_pane(1), "已经没了");
        // 主干只判 `paneIndex == 0`，非 0 一律走第二格 —— 照抄，不加第三格校验
        assert!(dock.ensure_pane(7));
        assert!(dock.pane1_live);
    }

    /// 第二格自身那发 `CloseRequested` = 取消分栏，**不删标签、不收宿主**。
    #[test]
    fn pane1_self_close_cancels_the_split_only() {
        let mut dock = assembled();
        dock.split(RightSplitMode::Horizontal, files_title);
        let before_ids: Vec<String> = dock.tabs.iter().map(|t| t.id.clone()).collect();
        assert!(dock.close_pane1_requested());
        assert_eq!(dock.mode, RightSplitMode::None);
        assert!(!dock.pane1_live);
        assert_eq!(
            dock.tabs.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
            before_ids,
            "一枚都不删"
        );
        assert!(dock.tabs.iter().all(|t| t.pane == FIRST_PANE));
        assert!(dock.open, "不收起宿主");
        assert!(!dock.close_pane1_requested(), "LC:743 未分栏 ⇒ 什么都不做");
    }

    // ---------------------------------------------------------------- 11 文件名

    /// DP3 用例 16：`file_name_of` 五档（含末段空、混用分隔符）。
    #[test]
    fn file_name_of_matches_dotnet_getfilename() {
        assert_eq!(file_name_of("a\\b\\c.txt"), "c.txt");
        assert_eq!(file_name_of("a/b/c.txt"), "c.txt");
        assert_eq!(file_name_of("a\\b\\"), "");
        assert_eq!(file_name_of("x"), "x");
        assert_eq!(file_name_of(""), "");
        // 逐字 `path.Replace('\\','/')` 之后的形状：混用只认最后一个 /
        assert_eq!(file_name_of("a/b\\c.txt"), "c.txt");
        assert_eq!(file_name_of("C:/"), "");
        assert_eq!(file_name_of("a//b"), "b");
    }

    // ---------------------------------------------------------------- 12 chrome 名册

    /// 19 枚 = DP3 §1.4 数出来的那 19 枚；一枚不多（自造 id 是 #101 那一族）。
    #[test]
    fn chrome_roster_is_exactly_the_nineteen_trunk_ids() {
        assert_eq!(chrome::ALL.len(), 19);
        let mut sorted: Vec<&str> = chrome::ALL.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 19, "名册里不许有重复条目");
        assert_eq!(chrome::TAB_PREFIX, "RightPaneTab:");
        assert_eq!(chrome::INNER_SPLITTER, "LayoutSplitterRightInner");
        assert_eq!(chrome::MENU, "FileActionMenu");
        // 撞名族是主干既成事实：这三枚与交付物卡菜单共用，反查要带祖先作用域
        assert_eq!(chrome::MENU_OPEN, "FileActionMenuOpen");
        assert_eq!(chrome::MENU_PREVIEW, "FileActionMenuPreview");
    }

    /// 菜单顺序 = 主干 `LC:538-547` 的 8 项 + 2 分隔符。
    #[test]
    fn menu_order_is_eight_items_and_two_separators() {
        assert_eq!(chrome::MENU_ORDER.len(), 10);
        assert_eq!(chrome::MENU_ORDER.iter().filter(|s| s.is_none()).count(), 2);
        assert_eq!(
            chrome::MENU_ORDER,
            [
                Some(chrome::MENU_OPEN),
                Some(chrome::MENU_PREVIEW),
                None,
                Some(chrome::SPLIT_HORIZONTAL),
                Some(chrome::SPLIT_VERTICAL),
                Some(chrome::NEW_TAB),
                Some(chrome::MOVE_HERE),
                None,
                Some(chrome::FULLSCREEN_ITEM),
                Some(chrome::CLOSE_TAB),
            ]
        );
    }

    /// 第二格面板那颗型属 #76 —— 本轮**不许**在状态机之外被引用（登记 ≠ 上树）。
    #[test]
    fn pane1_panel_id_is_registered_but_not_assembled() {
        let dock = assembled();
        assert!(chrome::ALL.contains(&chrome::PANE1_PANEL));
        assert!(!dock.pane1_live, "装配态下第二格不存在，K2 也不该挂 FilesPanelPane1");
    }

    // ---------------------------------------------------------------- 13 本文件的纯净性自锁

    /// `dock.rs` 的**生产段代码**（`#[cfg(test)]` 之前，且剥掉所有注释行）。
    /// 剥注释是必需的：本文件的注释大量引用主干原文与 `layout::` 的函数名，
    /// 让它们去过字面量反锁 = 自己咬自己（仓库 §4.1 那条「拆字写法专治朴素 grep」同理）。
    fn production_code() -> String {
        let source = include_str!("dock.rs");
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        production
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// K1 的全部意义：这层是**纯逻辑**。把渲染层的类型喂进来就失去免冲突属性，
    /// 所以用源码级反锁钉住（拆字写法：本串的 needle 不能自己把自己喂绿）。
    #[test]
    fn dock_stays_free_of_rendering_types() {
        let production = production_code();
        for forbidden in [
            concat!("windows", "_reactor"),
            concat!("aut", "o", "mation", "_id"),
            concat!("Bor", "der"),
            concat!("Vie", "w"),
            concat!("Grid", "Length"),
            concat!("Font", "Icon"),
            concat!("Thick", "ness"),
            concat!("Stack", "Panel"),
            concat!("Text", "Block"),
            "catalog",
            "i18n",
        ] {
            assert!(
                !production.contains(forbidden),
                "dock.rs 的生产段代码混进了渲染/i18n 依赖：{forbidden}"
            );
        }
        // 唯一允许的依赖：layout + std
        assert!(production.contains("use crate::layout"));
        let crate_uses: Vec<&str> = production
            .lines()
            .filter(|line| line.starts_with("use crate::"))
            .collect();
        assert_eq!(
            crate_uses,
            vec!["use crate::layout::{self, RightSplitMode};"],
            "除了 layout 之外不许再引兄弟模块（引 i18n/kernel 就是把这层拖出纯逻辑域）"
        );
    }

    /// dock **一律不落盘**（DP3 §1.6 逐字证据：主干那个写盘点只写 sidebar/rightbar）。
    #[test]
    fn dock_touches_no_disk_and_no_prefs() {
        let production = production_code();
        for forbidden in [
            "save_prefs",
            "load_prefs",
            "PREFS_FILE_NAME",
            concat!("Write", "AllText"),
            concat!("File", "::"),
            concat!("serde", "_json"),
            concat!("std::fs"),
            concat!("Layout", "ColumnPrefs"),
            concat!("local", "app_data"),
        ] {
            assert!(
                !production.contains(forbidden),
                "dock.rs 的生产段代码里出现了落盘字样（主干 dock 不落盘）：{forbidden}"
            );
        }
        // 但也**不许**假装自己管宽度：偏好本体留在 layout
        assert!(!production.contains("rightbar_width"));
    }

    /// 本模块不许出现主干没有的第三种分栏方向 / 第三格上限的副本判据。
    #[test]
    fn no_second_source_of_truth_for_the_two_pane_cap() {
        let source = include_str!("dock.rs");
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        // 上限只有 layout 那一颗；本模块出现字面量 2 就当嫌疑
        assert!(
            production.contains("RIGHT_DOCK_MAX_PANES"),
            "两格上限必须复用 layout 的那颗常量"
        );
        assert!(
            !production.contains("MAX_PANES: usize = 2"),
            "不许在 dock.rs 里重定义格数上限"
        );
        assert!(!production.contains("RIGHT_INNER_RATIO_MIN:"));
        assert!(!production.contains("clamp("));
    }

    /// 不变量收口：任意转移序列之后 `tabs.len() >= 1` 且 active 指向真实标签或回落首枚。
    #[test]
    fn invariants_hold_across_a_long_transition_script() {
        let mut dock = assembled();
        for round in 0..12u32 {
            match round % 6 {
                0 => {
                    dock.add_tab(files_title);
                }
                1 => {
                    dock.split(RightSplitMode::Horizontal, files_title);
                }
                2 => {
                    let id = dock.tabs[dock.tabs.len() - 1].id.clone();
                    dock.move_here(&id);
                }
                3 => {
                    let id = dock.tabs[0].id.clone();
                    dock.close_tab(&id);
                }
                4 => {
                    dock.toggle_fullscreen();
                }
                _ => {
                    dock.open_preview("C:\\temp\\a.txt", true);
                }
            }
            assert!(!dock.tabs.is_empty(), "round {round}：主干永远 >=1 枚");
            assert!(dock.pane0_live);
            assert_eq!(
                dock.mode == RightSplitMode::None,
                !dock.pane1_live,
                "round {round}：mode 与第二格存活同进同退"
            );
            if dock.mode != RightSplitMode::None {
                assert!(dock.tabs.iter().any(|t| t.pane == SECOND_PANE));
            } else {
                assert!(dock.tabs.iter().all(|t| t.pane == FIRST_PANE));
            }
        }
    }
}
