//! #65「轮次轨 TurnRail」的**纯逻辑**层。
//!
//! 这里只有数值与字符串变换，一个 `View` 都不碰：轨的三态宽度、宿主高度、预览卡的 y、
//! 悬停取槽、长 token 断行 —— 全部是可以注入数据单测的函数，`main.rs` 只负责把它们
//! 填进 reactor 的构建器。规格来源是主干 `MainWindow.xaml` 里 `x:Name="TurnRailHost"` 那一整段
//! （块起 = 它上方那段「历史快速定位」注释，块止 = 与它配对的 `</Grid>`）+ 主干 `MainWindow.TurnRail.cs`
//! 现版（2026-09-23 抄），逐条对应在下面的常量与文档里。
//! **刻意不写 xaml 行号**：主干工作树每提交一次就可能把整块推走若干行（本文件旧版写的
//! `MainWindow.xaml:611-688` 就是这么静默失效的），只留 `x:Name` 这类 grep 得到的锚 ——
//! 要定位就 `grep -n TurnRailHost MainWindow.xaml`。
//!
//! 数据源不在本模块：刻度读 `kernel::ControlState::turn_outline(active_session_id)`，
//! 那份表是 `session/control` 流的投影快照（`Projections::turn_outline`），轨这边**不另存真相**。

use crate::kernel::TurnOutlineItem;

// ---------------------------------------------------------------- 规格钉死的几何常量

/// 宿主列宽（DIP）。主干 `MainWindow.xaml` 里 `x:Name="TurnRailHost"` 那颗 `Grid` 的 `Width="48"`。
pub const HOST_WIDTH: f64 = 48.0;
/// 宿主 `Margin 0,0,4,0` 的右内缩。
pub const HOST_RIGHT_MARGIN: f64 = 4.0;
/// 条目列表的上下内衬（`Padding 0,2`）。
pub const LIST_INSET: f64 = 2.0;
/// 一条刻度的槽高（主干每槽那颗 `Grid Height="20"`）。
pub const SLOT_PITCH: f64 = 20.0;
/// 刻度本体高。
pub const MARK_HEIGHT: f64 = 6.0;
/// 刻度本体的 `RadiusX/RadiusY`（= `MARK_HEIGHT/2`，两端正好收成半圆）。
pub const MARK_RADIUS: f64 = 3.0;
/// 刻度条数低于这个数整条不显示（主干「`< 2` 不显示」）。
pub const MIN_MARKS: usize = 2;
/// 宿主高度的绝对上限（`min(natural, band - 64, 420)` 里的 420）。
pub const HOST_MAX_HEIGHT: f64 = 420.0;
/// 带子参与夹紧的门槛：`band > 64` 才敢拿 `band - 64` 当上限（首帧没布局出高度时用 natural）。
pub const HOST_BAND_GATE: f64 = 64.0;
/// `band - 64` 里那 64 DIP 的留白。
pub const HOST_BAND_SLACK: f64 = 64.0;
/// 预览卡宽。
pub const CARD_WIDTH: f64 = 360.0;
/// 预览卡 `Margin = (-370, y, 0, 0)` 的左偏移（负 = 往宿主左缘外面画）。
pub const CARD_LEFT_OFFSET: f64 = -370.0;
/// 首帧没有实测卡高时的标称高（值 100）。
///
/// 主干那个 100 **不是**定位处写死的字面量，而是常量 `TurnRailPreviewHeight`；
/// `MainWindow.TurnRail.cs` 的 `ShowTurnRailPreview` 在定位段走
/// `TurnRailPreviewCard.ActualHeight > 0 ? ActualHeight : TurnRailPreviewHeight`
/// ⇒ 只有**首帧还没布局出高度**时才落到它，一旦有实测值主干就用实测。
/// 分叉的 reactor 读不到 `ActualHeight`（表3「卡高实测」那条已备案不可移植）⇒ 恒用这一枚。
pub const CARD_NOMINAL_HEIGHT: f64 = 100.0;
/// 预览卡里 `ScrollViewer MaxHeight="480"`。
pub const CARD_MAX_HEIGHT: f64 = 480.0;

/// 三态的宽度（DIP）与不透明度 —— 主干那张表：常态 24/0.55、悬停 32/0.9、活动轮 40/1.0。
pub const IDLE_WIDTH: f64 = 24.0;
pub const HOVER_WIDTH: f64 = 32.0;
pub const ACTIVE_WIDTH: f64 = 40.0;
pub const IDLE_OPACITY: f64 = 0.55;
pub const HOVER_OPACITY: f64 = 0.9;
pub const ACTIVE_OPACITY: f64 = 1.0;

// ---------------------------------------------------------------- 三态

/// 一条刻度的视觉态。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarkState {
    /// 静置。
    #[default]
    Idle,
    /// 指针正落在这条上（或最近的一条）。
    Hover,
    /// 这条就是活动轮。
    Active,
}

/// 同时成立时**活动档赢**：主干那三档里活动轮最宽最不透明，而且它描述的是「视口里正在看哪一轮」
/// 这件事，比「指针恰好掠过」更稳定 ⇒ 悬停在活动轮上时不该把它压回 32/0.9。
pub fn state_of(hovered: bool, active: bool) -> MarkState {
    if active {
        MarkState::Active
    } else if hovered {
        MarkState::Hover
    } else {
        MarkState::Idle
    }
}

/// 该态的刻度宽（DIP）。
pub const fn mark_width(state: MarkState) -> f64 {
    match state {
        MarkState::Idle => IDLE_WIDTH,
        MarkState::Hover => HOVER_WIDTH,
        MarkState::Active => ACTIVE_WIDTH,
    }
}

/// 该态的不透明度。
pub const fn mark_opacity(state: MarkState) -> f64 {
    match state {
        MarkState::Idle => IDLE_OPACITY,
        MarkState::Hover => HOVER_OPACITY,
        MarkState::Active => ACTIVE_OPACITY,
    }
}

/// 刻度条数够不够上屏（不足 `MIN_MARKS` 条时整条轨**不放进子树** —— reactor 没有
/// `Visibility.Collapsed`，分叉的「隐藏」一律是「不生成那个节点」）。
pub const fn rail_visible(count: usize) -> bool {
    count >= MIN_MARKS
}

// ---------------------------------------------------------------- 槽位几何

/// 一条刻度的数据：轮号 + 该轮用户消息的 seq + 预览卡要用的问答摘要。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TurnRailMark {
    pub turn: i64,
    pub seq: i64,
    pub prompt: String,
    pub response: String,
}

impl TurnRailMark {
    /// 从内核投影建一条。**断行处理不在这里**（那是渲染期变换，见 [`break_long_tokens`]），
    /// 本函数只搬数据，保证「同一个 outline 反复建出来的刻度相等」可被单测断言。
    pub fn from_item(item: &TurnOutlineItem) -> Self {
        Self {
            turn: item.turn,
            seq: item.seq,
            prompt: item.prompt.clone(),
            response: item.response.clone(),
        }
    }
}

/// 投影表 → 刻度表（顺序照内核给的顺序，内核已按 `turn` 升序排过）。
pub fn marks_from(outline: &[TurnOutlineItem]) -> Vec<TurnRailMark> {
    outline
        .iter()
        .map(TurnRailMark::from_item)
        .collect()
}

/// 第 `index` 条刻度的槽顶（相对宿主内容面原点，DIP）。
pub fn slot_top(index: usize, inset: f64, pitch: f64) -> f64 {
    inset + index as f64 * pitch
}

/// 宿主高：`natural = 2*inset + count*pitch`；带子够高时夹到 `min(natural, band - 64, 420)`，
/// 否则（首帧没布局出高度、或带子比门槛还矮）用 `natural`。
///
/// 非有限输入一律退到 `natural`（宁可不夹，也别把轨压成 0 高）。
pub fn host_height(inset: f64, pitch: f64, count: usize, band: f64) -> f64 {
    let natural = 2.0 * inset + count as f64 * pitch;
    if !band.is_finite() || band <= HOST_BAND_GATE {
        return natural;
    }
    natural.min(band - HOST_BAND_SLACK).min(HOST_MAX_HEIGHT).max(0.0)
}

/// 槽中心（预览卡要对着它）。
pub fn slot_center(index: usize, inset: f64, pitch: f64) -> f64 {
    slot_top(index, inset, pitch) + pitch / 2.0
}

/// 预览卡的 y：`center - card/2`，再夹进 `[0, max(0, host - card)]`。
///
/// 卡片比宿主高时 `max_top` 取 0 ⇒ 贴宿主顶（主干那句 `Math.Max(0, …)` 同口径），
/// 不会因为算出负的可用区间而把卡推到宿主外面去。
pub fn preview_y(center: f64, card_height: f64, host_height: f64) -> f64 {
    let card = if card_height.is_finite() && card_height > 0.0 {
        card_height
    } else {
        CARD_NOMINAL_HEIGHT
    };
    let host = if host_height.is_finite() { host_height.max(0.0) } else { 0.0 };
    let max_top = (host - card).max(0.0);
    (center - card / 2.0).clamp(0.0, max_top)
}

/// 第 `index` 条刻度对应的预览卡 y。
pub fn preview_y_for_slot(
    index: usize,
    inset: f64,
    pitch: f64,
    card_height: f64,
    host_height: f64,
) -> f64 {
    preview_y(slot_center(index, inset, pitch), card_height, host_height)
}

/// 悬停取槽：指针在宿主内的 y（DIP）→ 该高亮哪一条。
///
/// 规则照主干 `MainWindow.TurnRail.cs` 的 `OnTurnRailPointerMoved`：落在某个槽的上下沿之间就是它；
/// 落在空隙（列表的 2 DIP 内衬、槽与槽之间若有任何缝、以及列表之外的上/下沿外）就取
/// **中心距离最近**的那条。主干那段注释写明了为什么要自己算 —— 早先用
/// `FindElementsInHostCoordinates` 永远对不准。
///
/// **这一枚分叉宿主刻意不接**（除本文件的自测外全仓零调用点 = 表4 D-3「实现面多做」），两条理由：
/// 1. 分叉的槽是连续 `SLOT_PITCH`(20 DIP) 的 `Border`，整列只有上下各 `LIST_INSET`(2 DIP) 内衬
///    ⇒ 指针落不进「槽与槽之间的缝」；宿主侧每槽各挂 `PointerEntered` 已覆盖主干那条命中语义，
///    「就近」这一发只在槽列表**之外**（那 2 DIP 内衬与列表上下沿外）才有意义。
/// 2. 主干的「最近」是在**已滚动的**轨上量的：`ContainerFromItem` + `TransformToVisual(TurnRailMarks)`
///    拿的是容器实测位置，天然含轨内滚动偏移。分叉没有 `ScrollViewer.ViewChanged` /
///    `VerticalOffset` 出口 ⇒ 滚动偏移读不到，只能吃 `slot_top(index)` 这套**未滚动的静态坐标**，
///    用户滚过轨之后再接「就近取槽」就是造假近似。
///
/// 缺 API 的正式口径写在 `main.rs` 里 `fn turn_rail` 那份「不可移植」清单（同一族：
/// `EnsureActiveMarkInView` / `TopVisibleBubbleTurn` 六兄弟）—— **那份清单不归本文件改**，
/// 这里只留指针、不写它的行号。本函数保留 `pub` 的意义只剩几何判据可单测：
/// 改槽距/内衬时先在这里红，好过在宿主里红。
///
/// `count == 0`、`pitch <= 0` 或 y 非有限 ⇒ `None`（一个像素都不该动）。
pub fn nearest_slot(pointer_y: f64, count: usize, inset: f64, pitch: f64) -> Option<usize> {
    if count == 0 || !pointer_y.is_finite() || !inset.is_finite() || pitch <= 0.0 || !pitch.is_finite() {
        return None;
    }
    let mut nearest: Option<(f64, usize)> = None;
    for index in 0..count {
        let top = slot_top(index, inset, pitch);
        if pointer_y >= top && pointer_y < top + pitch {
            return Some(index); // 命中槽内：不再看距离
        }
        let distance = (pointer_y - slot_center(index, inset, pitch)).abs();
        if nearest.is_none_or(|(best, _)| distance < best) {
            nearest = Some((distance, index));
        }
    }
    nearest.map(|(_, index)| index)
}

// ---------------------------------------------------------------- 点击跳转

/// 一条已渲染气泡在「点击跳转」这一层需要的三个事实。
///
/// 刻意不引用 `main.rs` 的 `Bubble`：本模块是 lib 侧，那个类型在 bin 侧。
/// 这里只吃一份能被注入数据单测的最小投影，调用方（`Shell::turn_target_bubble`）每次现算、
/// 不另存一份状态 —— 三级判据只有这一份实现，bin 侧不许再抄一遍。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BubbleRow<'a> {
    pub turn: i64,
    pub role: &'a str,
    /// 该轮的**答案**气泡（主干 `_transcriptAnswers[turn]` 且它在可见序列里）。
    pub is_answer: bool,
}

/// 主干 `TR:436` 的那句 `b.Role is "user" or "user-image"`，逐字抄。
///
/// 分叉今天只产 `"user"`（`main.rs` 全仓 `user-image` 0 命中 —— 图片气泡还没移植），
/// 但这一档留着：图片气泡落地时不必再改判据，而且它钉的是主干的语义而不是分叉的现状。
pub const USER_ROLES: [&str; 2] = ["user", "user-image"];

/// 第 `turn` 轮的跳转目标行下标 = 主干 `OnTurnRailMarkClick` 的三级回落（`TR:436-441`）：
/// 1) 该轮首条 `user` / `user-image`（轮次起点，主干 `ScrollIntoViewAlignment.Leading` 对齐它）；
/// 2) 该轮的答案气泡（compact 折叠视图可能把用户气泡裁掉，`TR:24-26` 明写了这条回退）；
/// 3) 该轮的首条任意气泡；
/// 三级都落空 ⇒ `None`（主干 `target is null ⇒ return`，一个像素都不滚）。
pub fn jump_target(rows: &[BubbleRow<'_>], turn: i64) -> Option<usize> {
    if turn <= 0 {
        return None;
    }
    let same_turn = |row: &BubbleRow<'_>| row.turn == turn;
    rows.iter()
        .position(|row| same_turn(row) && USER_ROLES.contains(&row.role))
        .or_else(|| rows.iter().position(|row| same_turn(row) && row.is_answer))
        .or_else(|| rows.iter().position(|row| same_turn(row)))
}

/// 一次跳转的估计结果：要走的 DIP（负 = 往回滚）与要补的滚轮格数。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jump {
    pub dip: f64,
    pub notches: i32,
}

/// 分叉版「滚到第 `target` 行」，对齐主干 `ScrollIntoView(target, Leading)`（详见 `main.rs`
/// 的 `jump_to_rail_turn` 文档与报告 §5 的 not-portable 备案）。
///
/// **这是近似，而且是双重近似**：
/// * reactor 0.100.0 没有 `ScrollIntoView`，分叉唯一的滚动出口是 `scroll::post_wheel*`
///   ⇒ 只有**相对量**，没有「绝对对齐到第几 DIP」这条路。
/// * 视口当前停在哪儿读不到（无 `ViewportChanged` / `VerticalOffset`）⇒ 只能按主干自己在
///   `RebindTurnRail`（`TR:168-172`）里用过的同一个前提假设「**视口贴底**」：估计首行 =
///   `rows − 一屏行数`，而那同时就是**最大可滚行位**。
/// * `row_dip` / `page_dip` 都是调用方给的估计常量（气泡是多行卡，真实行高量不到）。
///
/// 于是「Leading 对齐」在这一假设下等于 `want = min(target, 最大可滚行位)`，位移
/// `dip = (want − 估计首行) * row_dip` —— **恒 ≤ 0**，只做「往回滚」。这不是偷工：贴底假设下
/// 目标本来就在视口里（含靠尾那几条）时，`want` 夹在最大可滚行位上，位移算出来就是 0，
/// 与主干「滚不动了 ⇒ 原地不动」完全一致；反过来说，用户手动往上翻过之后再点尾部刻度，
/// 分叉这一发会「原地不动」，那是同一个缺口的另一面（记录在报告 §4 验收项）。
///
/// 量化与夹取一律复用 [`crate::scroll::notches_for_dip`]（±[`crate::scroll::MAX_NOTCHES`]）
/// 与 [`crate::scroll::rows_visible`]，本函数不另造一套滚轮账。
///
/// `Some` 的两种含义要分开看：`notches != 0` = 发这一发；`notches == 0`（目标已在视口里、
/// 或内容不满一屏）= 不用发。`None` = 输入根本不合法（`rows == 0`、目标不在序列里、行高坏掉、
/// 每格几 DIP 未知）⇒ 一个像素都不许动。
pub fn jump_estimate(target: usize, rows: usize, band: crate::scroll::Band) -> Option<Jump> {
    if rows == 0 || target >= rows || !band.row_dip.is_finite() || band.row_dip <= 0.0 {
        return None;
    }
    if !band.dip_per_notch.is_finite() || band.dip_per_notch <= 0.0 {
        // 每格几 DIP 不知道就不该发 —— 与 `scroll::notches_for_dip` 同一条底线，
        // 只是这里要把它升成 `None`（调用方据此连日志都另写一句），不能悄悄发 0 格。
        return None;
    }
    let visible = crate::scroll::rows_visible(band) as f64;
    let top = (rows as f64 - visible).max(0.0);
    let dip = ((target as f64).min(top) - top) * band.row_dip;
    Some(Jump { dip, notches: crate::scroll::notches_for_dip(dip, band) })
}

// ---------------------------------------------------------------- 断词

/// 零宽空格：插进去只为给换行器一个断点，屏幕上不占任何宽度。
pub const WORD_JOINER: char = '\u{200b}';

/// 允许在其后断行的字符（主干 `MainWindow.TurnRail.cs:404-420` 那张表）。
///
/// 动机：`C:\Users\…\very\long\nested\path\xyz.rs` 里没有任何空白，`TextWrapping=Wrap`
/// 也折不动它 ⇒ 360 DIP 的预览卡会把整条路径裁成半截。在这些字符后面补一个 [`WORD_JOINER`]，
/// 折行点就有了。
pub const BREAK_AFTER: [char; 11] = ['\\', '/', '-', '_', '.', ':', '?', '&', '=', '#', '%'];

/// 在每个断点字符后补一个零宽空格（后面已经跟一个零宽空格时不重复补 ⇒ 幂等）。
///
/// 纯字符串变换，不改内容顺序、不吞字符，所以「原文里本来就有 ZWSP」也安全。
pub fn break_long_tokens(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + text.len() / 8 + 8);
    for (index, ch) in chars.iter().enumerate() {
        out.push(*ch);
        if BREAK_AFTER.contains(ch) && chars.get(index + 1) != Some(&WORD_JOINER) {
            out.push(WORD_JOINER);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(turn: i64, seq: i64, prompt: &str, response: &str) -> TurnOutlineItem {
        TurnOutlineItem {
            turn,
            seq,
            prompt: prompt.to_string(),
            response: response.to_string(),
        }
    }

    #[test]
    fn three_states_carry_the_spec_table() {
        assert_eq!(mark_width(MarkState::Idle), 24.0);
        assert_eq!(mark_opacity(MarkState::Idle), 0.55);
        assert_eq!(mark_width(MarkState::Hover), 32.0);
        assert_eq!(mark_opacity(MarkState::Hover), 0.9);
        assert_eq!(mark_width(MarkState::Active), 40.0);
        assert_eq!(mark_opacity(MarkState::Active), 1.0);
        // 半径 = 高的一半，两端是半圆而不是圆角矩形。
        assert_eq!(MARK_RADIUS * 2.0, MARK_HEIGHT);
    }

    #[test]
    fn active_beats_hover_and_idle_is_the_floor() {
        assert_eq!(state_of(false, false), MarkState::Idle);
        assert_eq!(state_of(true, false), MarkState::Hover);
        assert_eq!(state_of(false, true), MarkState::Active);
        assert_eq!(state_of(true, true), MarkState::Active, "活动档不能被悬停压回去");
    }

    #[test]
    fn rail_hides_below_two_marks() {
        assert!(!rail_visible(0));
        assert!(!rail_visible(1));
        assert!(rail_visible(2));
        assert!(rail_visible(99));
    }

    #[test]
    fn marks_carry_turn_seq_and_both_sides_of_the_outline() {
        let outline = [item(1, 3, "第一问", ""), item(2, 88, "第二问", "第二答")];
        let marks = marks_from(&outline);
        assert_eq!(
            marks,
            vec![
                TurnRailMark { turn: 1, seq: 3, prompt: "第一问".into(), response: String::new() },
                TurnRailMark { turn: 2, seq: 88, prompt: "第二问".into(), response: "第二答".into() },
            ]
        );
    }

    #[test]
    fn host_height_uses_natural_until_the_band_is_known() {
        // natural = 2*2 + 5*20 = 104。带子缺席（首帧 0 / NaN / 比门槛矮）就用 natural。
        for band in [0.0, 64.0, -10.0, f64::NAN] {
            assert_eq!(host_height(2.0, 20.0, 5, band), 104.0, "band={band}");
        }
        // 带子够高但装得下：还是 natural（轨永远不会被拉长）。
        assert_eq!(host_height(2.0, 20.0, 5, 900.0), 104.0);
        // 带子不够高：夹到 band - 64。
        assert_eq!(host_height(2.0, 20.0, 30, 300.0), 236.0, "30 条 natural=604，band-64=236");
        // 带子很富裕但条数很多：夹到 420 上限。
        assert_eq!(host_height(2.0, 20.0, 60, 5000.0), 420.0);
        // 三条同时候选时取最小。
        assert_eq!(host_height(2.0, 20.0, 100, 300.0), 236.0);
    }

    #[test]
    fn preview_y_centers_on_the_slot_and_clamps_into_the_host() {
        // 槽 2（inset 2 / pitch 20）中心 = 2 + 2*20 + 10 = 52；卡高 100 ⇒ 52-50 = 2，宿主 300 装得下。
        assert_eq!(preview_y_for_slot(2, 2.0, 20.0, 100.0, 300.0), 2.0);
        // 靠底的槽会被宿主夹住：槽 14 中心 = 292，292-50 = 242 > 300-100=200 ⇒ 200。
        assert_eq!(preview_y_for_slot(14, 2.0, 20.0, 100.0, 300.0), 200.0);
        // 顶上的槽不会算出负 y：槽 0 中心 = 12，12-50 = -38 ⇒ 0。
        assert_eq!(preview_y_for_slot(0, 2.0, 20.0, 100.0, 300.0), 0.0);
        // 卡比宿主还高 ⇒ max_top 取 0 ⇒ 贴顶，不溢出成负数。
        assert_eq!(preview_y_for_slot(3, 2.0, 20.0, 500.0, 120.0), 0.0);
        // 卡高不合法（首帧没实测 / NaN / 负）⇒ 用标称 100。
        assert_eq!(preview_y(52.0, 0.0, 300.0), 2.0);
        assert_eq!(preview_y(52.0, f64::NAN, 300.0), 2.0);
        assert_eq!(preview_y(52.0, -8.0, 300.0), 2.0);
    }

    #[test]
    fn nearest_slot_hits_inside_and_snaps_across_gaps() {
        // 5 条：槽 i = [2+20i, 2+20i+20)。
        assert_eq!(nearest_slot(2.0, 5, 2.0, 20.0), Some(0));
        assert_eq!(nearest_slot(21.999, 5, 2.0, 20.0), Some(0));
        assert_eq!(nearest_slot(22.0, 5, 2.0, 20.0), Some(1));
        assert_eq!(nearest_slot(101.0, 5, 2.0, 20.0), Some(4));
        // 上沿外（列表的 2 DIP 内衬之上）取最近 ⇒ 第一条；下沿外 ⇒ 最后一条。
        assert_eq!(nearest_slot(-500.0, 5, 2.0, 20.0), Some(0));
        assert_eq!(nearest_slot(1000.0, 5, 2.0, 20.0), Some(4));
        // 列表正中（槽与槽之间没有缝：栈面板的槽是连续的）⇒ 走的还是「命中槽内」那一支。
        assert_eq!(nearest_slot(62.0, 5, 2.0, 20.0), Some(3));
        // 距离那一支在缝里怎么取胜要人造一条缝才看得到，而这里的槽表没有缝可造 ⇒ 上面的
        // 两个「沿外」用例就是它的覆盖点（沿外必然全部走距离比较）。
        // 退化输入一个都不接。
        assert_eq!(nearest_slot(30.0, 0, 2.0, 20.0), None);
        assert_eq!(nearest_slot(f64::NAN, 5, 2.0, 20.0), None);
        assert_eq!(nearest_slot(30.0, 5, 2.0, 0.0), None);
        assert_eq!(nearest_slot(30.0, 5, 2.0, f64::INFINITY), None);
    }

    /// 主干 `TR:436-441` 的三级回落：用户气泡 → 答案气泡 → 该轮任意气泡 → 不滚。
    #[test]
    fn jump_target_prefers_the_first_user_bubble_then_the_answer() {
        let rows = vec![
                BubbleRow { turn: 1, role: "assistant", is_answer: false },
                BubbleRow { turn: 1, role: "user", is_answer: false },
                BubbleRow { turn: 2, role: "tool", is_answer: false },
                BubbleRow { turn: 2, role: "user", is_answer: false },
                BubbleRow { turn: 2, role: "assistant", is_answer: false },
                BubbleRow { turn: 2, role: "assistant", is_answer: true },
            BubbleRow { turn: 3, role: "assistant", is_answer: true },
        ];
        // 1) 轮次起点优先：第 1 轮的首条 user 在第 1 行。
        assert_eq!(jump_target(&rows, 1), Some(1));
        // 2) 该轮没有用户气泡（compact 折叠 / 图片轮）⇒ 回落答案气泡。
        assert_eq!(jump_target(&rows, 3), Some(6));
        // 3) 连答案都没有时回落该轮首条任意气泡。
        let only_tool = [BubbleRow { turn: 4, role: "tool", is_answer: false }];
        assert_eq!(jump_target(&only_tool, 4), Some(0));
        // 该轮压根不在序列里 / 轮号非法 ⇒ None（主干这时直接 return，一个像素都不滚）。
        assert_eq!(jump_target(&rows, 9), None);
        assert_eq!(jump_target(&rows, 0), None);
        assert_eq!(jump_target(&rows, -3), None);
        assert_eq!(jump_target(&[], 1), None);
        // `user-image` 与 `user` 同档（主干那句 `is "user" or "user-image"`）。
        let with_image = [
            BubbleRow { turn: 5, role: "assistant", is_answer: true },
            BubbleRow { turn: 5, role: "user-image", is_answer: false },
        ];
        assert_eq!(jump_target(&with_image, 5), Some(1));
        assert_eq!(USER_ROLES, ["user", "user-image"]);
    }

    /// 跳转量化（主干 `ScrollIntoView(Leading)` 的相对量近似）：贴底假设、只做往回滚、
    /// 夹到 `MAX_NOTCHES`、目标已在视口里就是 0 格、非法几何一律 `None`。
    #[test]
    fn jump_estimate_quantises_against_the_bottom_anchored_guess() {
        let band = crate::scroll::Band { row_dip: 48.0, page_dip: 752.0, dip_per_notch: 48.0 };
        // 一屏 15 行（752/48=15.67 向下取整）；40 行 ⇒ 估计首行 = 40-15 = 25（也是最大可滚行位）。
        // 目标第 0 行在首行之上 25 行 = 1200 DIP → ceil(1200/48)=25 格 → 夹到 -12。
        let up = jump_estimate(0, 40, band).expect("第 0 行在估计首行之上");
        assert_eq!(up.dip, -1200.0);
        assert_eq!(up.notches, -crate::scroll::MAX_NOTCHES);
        // 只差几行时没被夹，格数按 1:1 走（这里 row_dip == dip_per_notch）。
        assert_eq!(jump_estimate(23, 40, band), Some(Jump { dip: -96.0, notches: -2 }));
        assert_eq!(jump_estimate(24, 40, band), Some(Jump { dip: -48.0, notches: -1 }));
        // 恰好就是估计首行 ⇒ 零位移、零格（调用方据此写「无需滚动」那句日志）。
        assert_eq!(jump_estimate(25, 40, band), Some(Jump { dip: 0.0, notches: 0 }));
        // 目标在首行之下（本来就在视口里）：Leading 要往下滚，但贴底假设下已经没有下行余量
        // ⇒ 夹在最大可滚行位上，还是 0 格。主干此刻同样「原地不动」。
        assert_eq!(jump_estimate(30, 40, band), Some(Jump { dip: 0.0, notches: 0 }));
        assert_eq!(jump_estimate(39, 40, band), Some(Jump { dip: 0.0, notches: 0 }));
        // 不满一屏（rows <= visible）⇒ 根本没有可滚余量 ⇒ 0 格。
        assert_eq!(jump_estimate(2, 6, band), Some(Jump { dip: 0.0, notches: 0 }));
        // 越界 / 空表 ⇒ None。
        assert_eq!(jump_estimate(40, 40, band), None);
        assert_eq!(jump_estimate(0, 0, band), None);
        // 行高坏了 ⇒ None（连 DIP 都算不出，不该猜）。
        assert_eq!(jump_estimate(1, 40, crate::scroll::Band { row_dip: 0.0, page_dip: 752.0, dip_per_notch: 48.0 }), None);
        assert_eq!(jump_estimate(1, 40, crate::scroll::Band { row_dip: f64::NAN, page_dip: 752.0, dip_per_notch: 48.0 }), None);
        // 每格几 DIP 不知道 ⇒ None，而不是「发 0 格」糊过去（`notches_for_dip` 那条底线在
        // 这里升成「整发丢掉」）。
        assert_eq!(jump_estimate(1, 40, crate::scroll::Band { row_dip: 48.0, page_dip: 752.0, dip_per_notch: 0.0 }), None);
    }

    #[test]
    fn long_tokens_get_break_points_after_the_separator_table() {
        let path = r"C:\Users\me\proj\src\main.rs";
        let broken = break_long_tokens(path);
        // 每个断点字符后一个 ZWSP：`\` × 5 + `.` × 1（`:` 也在表里 ⇒ `C:` 后面那个）。
        assert_eq!(broken.matches(WORD_JOINER).count(), 7);
        // 剥掉零宽空格必须逐字还原 ⇒ 只加断点、不动内容。
        assert_eq!(broken.replace(WORD_JOINER, ""), path);
        // 表里的 11 个字符逐个生效。
        for ch in BREAK_AFTER {
            let broken = break_long_tokens(&format!("a{ch}b"));
            assert_eq!(broken, format!("a{ch}{WORD_JOINER}b"), "分隔符 {ch} 没补上断点");
        }
        // 幂等：已经跟了一个 ZWSP 就不再补，连续两个分隔符各补一个。
        assert_eq!(break_long_tokens(&break_long_tokens("a-b")), break_long_tokens("a-b"));
        assert_eq!(break_long_tokens("a--b"), format!("a-{WORD_JOINER}-{WORD_JOINER}b"));
        // 空串、纯普通文本不加东西。
        assert_eq!(break_long_tokens(""), String::new());
        assert_eq!(break_long_tokens("普通的一句话"), "普通的一句话");
    }
}
