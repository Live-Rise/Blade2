//! #57「上下文容量圈 + 明细面板」**宿主接线轮（MW4）的离线源码锁**。
//!
//! 口径与 `tests/shellfiles_wiring.rs` 同一套：本轮禁止一切前台行为（不起 exe、不截图、
//! 不 UIA、不 SendInput），所以「这颗圈真的有宿主、真的吃得到读数」只能靠**源码文本**取证。
//! 本文件**不在** `src/main.rs` 那 96+ 颗 `include_str!("main.rs")` 锁的射程内 ⇒ 这里的
//! needle 一律**整串写死**，不必 `concat!` 拆词防自匹配；计数探针只吃产品码一层。
//!
//! 接手时的现场（MW3 残留）：`Shell::context_meter_cell` 一族五件（外加三枚面板常量）
//! 全部写完但**没人调** ⇒ `cargo check --all-targets` 报「multiple associated items are
//! never used」。本文件里 `the_ring_has_exactly_one_host_in_the_middle_composer_cell`
//! 就是专门钉这一发的：**唯一**宿主，掉回零宿主或造出第二宿主都红。
//!
//! 三态（`RingFace::TrackOnly` / `Arc` / `Full`）在两处取证：
//! · 文本侧 —— 三个分支臂必须齐挂在 `context_meter_cell` 窗内（少一臂 = 某一档画不出来）；
//! · 行为侧 —— 直接调 lib 的 `kernel::mw3_ring_chords` + `contextmeter::face_of`，
//!   验「哪档出弦、出几段、端点落不落回真弧终点」，以及「缺分母 ⇒ 无读数，绝不是 0%」。

use blade2_rs::contextmeter::{
    ContextMeterBook, FULL_PERCENT, RING_CENTER, RING_RADIUS, RingFace, face_of, tooltip_and_aria,
};
use blade2_rs::kernel::{MW3_RING_CHORD_SEGMENTS, mw3_ring_chords};
use serde_json::json;

const SRC: &str = include_str!("../src/main.rs");
const KRN: &str = include_str!("../src/kernel.rs");
const I18N: &str = include_str!("../src/i18n.rs");

/// 窗口助手（口径同 `main.rs` 内部 mod 那份）：锚点找不到就 panic，绝不静默把下一颗函数
/// 卷进断言 —— 那会把「窗口内不含 X」这种负锁洗成假绿。
fn mw4_window<'a>(hay: &'a str, start: &str, end: &str) -> &'a str {
    let (_, rest) = hay
        .split_once(start)
        .unwrap_or_else(|| panic!("源码里找不到起点锚 {start}"));
    rest.split_once(end)
        .unwrap_or_else(|| panic!("锚点 {start} 之后找不到终点锚 {end}：窗口会一路扩到文件末尾"))
        .0
}

/// 起点之后的窗口（终点锚在起点之前时也能给出有限窗）。
fn mw4_window_after<'a>(hay: &'a str, start: &str, end: &str) -> &'a str {
    let i = hay
        .find(start)
        .unwrap_or_else(|| panic!("源码里找不到起点锚 {start}"));
    let j = hay[i + start.len()..]
        .find(end)
        .unwrap_or_else(|| panic!("起点 {start} 之后找不到终点锚 {end}"));
    &hay[i..i + start.len() + j]
}

/// 整文件词界计数（`context_meter_cell` 不该数到 `context_meter_cell_x`）。
fn mw4_count_word(hay: &str, needle: &str) -> usize {
    let mut n = 0;
    let mut at = 0;
    while let Some(k) = hay[at..].find(needle) {
        let after = at + k + needle.len();
        match hay.as_bytes().get(after) {
            Some(&c) if c.is_ascii_alphanumeric() || c == b'_' => {}
            _ => n += 1,
        }
        at = after;
    }
    n
}

/// 圈宿主锁（本轮消警的正半边的）：`composer_band` 必须**逐字**点这一颗，且全文件
/// 只有这一处调用点；反向半边：`composer` 那三列一列不许加。
#[test]
fn the_ring_has_exactly_one_host_in_the_middle_composer_cell() {
    let band = mw4_window(SRC, "fn composer_band(", "fn agent_mode_trigger(");
    assert!(
        band.contains("self.context_meter_cell(context)"),
        "band 里没挂容量圈 ⇒ MW3 那五件关联项再次失去宿主（接手时那条警告的根因）"
    );
    assert!(
        band.contains(r#""context-meter", self.context_meter_cell(context)"#),
        "容量圈没挂进那枚槽位循环 ⇒ 「不在场就不入树」这条 `Option` 等价物失效"
    );
    // 反「注释级假绿」：那一行必须是**真代码**（行首就是元组左括号），不是被 `//` 供起来的串。
    let ring_lines: Vec<&str> = band
        .lines()
        .filter(|l| l.contains("self.context_meter_cell(context)"))
        .collect();
    assert_eq!(
        ring_lines.len(),
        1,
        "band 里提到容量圈宿主的行不止一行 ⇒ 计数判据会被注释撑绿"
    );
    assert!(
        ring_lines[0].trim_start().starts_with('('),
        "容量圈那一格被注释掉了 ⇒ 宿主是画出来的假象（警告会重新长回来）"
    );
    assert!(
        band.contains(r#"KeyedView::new(key, cell)"#),
        "槽位循环被换成直推 ⇒ 选择器那颗的「不在场就不入树」一起塌"
    );
    // 唯一宿主：全文件只许 `fn` 声明 + band 一处调用（声明用词界数，调用用整串）。
    assert_eq!(
        mw4_count_word(SRC, "fn context_meter_cell("),
        1,
        "容量圈出现了第二份脸 ⇒ 两本真相"
    );
    assert_eq!(
        mw4_count_word(SRC, "self.context_meter_cell(context)"),
        1,
        "宿主不止一处（或干脆为零）⇒ 挂法必须收在 `composer_band` 那一格里"
    );
    // 反向半边（地雷 4）：为圈单开一列 = 把正文挤窄。
    let composer = mw4_window(SRC, "fn composer(&self", "fn kernel_boot_panel(");
    assert_eq!(composer.matches("GridLength::Auto").count(), 2);
    assert_eq!(composer.matches("GridLength::STAR").count(), 1);
    assert_eq!(composer.matches(".columns(").count(), 1, "给圈加了列");
}

/// 三态齐挂 + 主干裸真名 + 一发两贴（tooltip 与无障碍名同串）+ 弦近似有宿主。
/// 反向半边：圈内只许一枚 `automation_id(`，且不许自造第二枚 id（台账 #101 红过）。
#[test]
fn the_ring_wears_the_mainline_name_and_builds_all_three_faces() {
    let cell = mw4_window(SRC, "fn context_meter_cell(", "fn mw3_context_meter_flyout(");
    // 齐备闸：读数缺位 ⇒ 整棵不入树（主干 `Visibility=Collapsed` 的分叉等价）。
    assert!(
        cell.contains("let occ = self.active_occupancy()?;"),
        "齐备闸被绕过 ⇒ 没有读数也会画出一颗圈（主干是收起）"
    );
    for face in [
        "cm::RingFace::TrackOnly",
        "cm::RingFace::Arc(",
        "cm::RingFace::Full",
    ] {
        assert!(cell.contains(face), "三态里少了 {face} 那一档");
    }
    assert!(
        cell.contains("cm::face_of(occ.percent)"),
        "档位不是由算子层给的 ⇒ 分叉自己重算了 percent"
    );
    assert!(
        cell.contains("mw3_ring_chords(occ.percent, MW3_RING_CHORD_SEGMENTS)"),
        "弧档没吃弦近似几何 ⇒ reactor 无 `Path` 这一档又变成死码"
    );
    // UIA：主干裸真名，恰好一枚 id。
    assert_eq!(
        cell.matches("automation_id(").count(),
        1,
        "圈里的 `automation_id(` 不再是恰好一枚 ⇒ 要么丢了主干真名，要么自造了第二枚"
    );
    assert!(cell.contains(r#".automation_id("ContextMeterRing")"#));
    assert!(!cell.contains("automation_id(format!"), "自造了动态 id");
    // 一发两贴：`tooltip_and_aria` 出一枚串，同时喂 tooltip 与 Name。
    assert!(cell.contains("cm::tooltip_and_aria(&self.catalog, occ)"));
    assert!(cell.contains(".automation_name(aria.clone())"));
    assert!(cell.contains(".tooltip(aria)"));
    // 面板锚点 = 这颗圈本身（原生 `Flyout`，见下一颗锁）。
    assert!(cell.contains(".flyout_with(self.mw3_context_meter_flyout(occ))"));
}

/// 明细面板走**原生 `Flyout`**，不是第九张自绘层（#57 的锚点就是那颗圈，框架三件行为全给：
/// 点圈即开 / 点外面即收 / 同刻至多一张 ⇒ 不需要 Msg、不需要状态格、不需要 13 处互斥落点）。
/// 这一发的可证伪半边：若哪天改回自绘层（进 `chat_page` 第九槽），这两枚哨兵会红，
/// 逼着改动者回去处理 `tests/dock_chrome.rs` 那颗 `dock_menu_close == 19` 同级互斥闸
/// （19 = 批 M2c 之后：目标面板是第八张兄弟层，开它也要收 dock 菜单；18 = 批 M4 轨迹）。
#[test]
fn the_detail_panel_is_a_native_flyout_not_a_ninth_self_drawn_layer() {
    let flyout = mw4_window(SRC, "fn mw3_context_meter_flyout(", "fn mw3_context_stat_row(");
    assert!(
        flyout.contains("Flyout::rich(self.mw3_context_panel_body(occ))"),
        "面板不再由那颗圈自己承载 ⇒ 锚点丢了"
    );
    assert!(
        flyout.contains(".placement(FlyoutPlacement::Top)"),
        "主干 `ShowRunFlyout` 的 `Placement = Top` 丢了"
    );
    assert!(
        !SRC.contains("context_panel: Option<String>"),
        "自绘层的第九个状态格出现了 ⇒ 同级互斥清单（8 张 ⇒ 9 张）必须同步改，见本文件文档"
    );
    assert!(
        !SRC.contains("Msg::ContextPanelToggle"),
        "自绘层的开关 Msg 出现了 ⇒ 上面那条互斥闸的口径要重算"
    );
}

/// 面板表体的数值逐格对主干 `CM:226-258` + `RunStats.cs:336-365`；三行齐、外壳四件齐。
#[test]
fn the_panel_body_carries_the_mainline_numbers_and_three_rows() {
    let panel = mw4_window(SRC, "fn mw3_context_panel_body(", "fn composer_band(&self");
    assert!(panel.contains(".minimum(0.0)"));
    assert!(panel.contains(".maximum(100.0)"));
    assert!(panel.contains(".value(occ.percent as f64)"));
    assert!(
        panel.contains("Self::CONTEXT_PANEL_BAR_HEIGHT"),
        "占比条高度不再读那颗局部常量 ⇒ 值漂了也没人知道"
    );
    assert!(panel.contains("Self::CONTEXT_PANEL_MIN_WIDTH"));
    assert!(panel.contains("Self::CONTEXT_PANEL_PADDING"));
    for row in [
        "let used_row = self.mw3_context_stat_row(",
        "let used_tokens_row = self.mw3_context_stat_row(",
        "let capacity_row = self.mw3_context_stat_row(",
    ] {
        assert!(panel.contains(row), "表体少了 {row} 那一行");
    }
    // 紧凑档与裸整数档各归其位：不许起第三份格式化函数。
    assert_eq!(panel.matches("tokens_text(locale, occ.").count(), 2, "行 1 的两格");
    assert_eq!(panel.matches("n0(occ.").count(), 2, "行 2/3 的裸整数格");
}

/// 面板四枚文案全过 i18n 表，且**零新键**：串必须本来就住在 `src/i18n.rs` 里。
/// 反向半边（地雷 7 的注释级假红同源）：面板窗内不许出现主干那两枚模板串的**裸抄**，
/// 也不许蹭详情浮层那格 `上下文窗口`（SP11 §4.1 明确禁止混用）。
#[test]
fn every_panel_string_is_already_in_the_i18n_table() {
    let panel = mw4_window(SRC, "fn mw3_context_panel_body(", "fn composer_band(&self");
    for key in ["上下文", "已用", "已用 token", "上下文容量"] {
        let needle = format!(".l(\"{key}\")");
        assert!(panel.contains(&needle), "面板里 {key} 没过表（裸抄中文）");
        assert!(
            I18N.contains(&format!("\"{key}\"")),
            "`i18n.rs` 里本来就没有 {key} ⇒ SP11 §4.1「零新键」这条断言不成立，要补键"
        );
    }
    assert!(
        !panel.contains("上下文窗口"),
        "蹭了详情浮层那格（`上下文窗口` 不是 #57 的串）"
    );
    assert!(!panel.contains("上下文已用"), "把 tooltip 模板抄进了表体");
}

/// 折叠点必须在 `match kind {` **之前**（主干 `TrackContextMeter` 与 `switch (type)` 的同一
/// 位置关系）；反向：不许在臂内另起第二发喂帧。
///
/// #58 M1 之后臂首那格有**两发**喂帧，批 M4（#108 轨迹面板）之后是**三发**：
/// `context_meters.track` → `run_stats.note` → `trajectory_note`。
/// ⚠ 如实登记两件事，别再拿「逐字对主干」当挡箭牌：
/// ① 主干那三发住在 `MainWindow.xaml.cs:5976-5979`，**真实顺序**是
///    `TrackRunStats(5977)` → `TrackContextMeter(5978)` → `TrajectoryObserve(5979)`
///    ⇒ 分叉前两发与主干**相反**（既有偏差，#58 M1 落的，本锁当时那句"逐字对齐"是错的）。
///    三发各自独立折叠、互不读对方的输出 ⇒ 顺序**行为上不可见**，故保留分叉现状、只把注释改正。
/// ② 第三发 `trajectory_note` 排在最后**与主干同位**（`TrajectoryObserve` 也是最后一发）⇒ 直接收进白名单。
/// 白名单之外别的语句一律仍然不许塞进来。
#[test]
fn the_meters_fold_before_the_switch_and_nowhere_else() {
    let arm = mw4_window(SRC, "fn apply_journal_event(", "fn apply_stream_frame(");
    let feed = arm
        .find(r#"let _ = self.context_meters.track(session, kind, &event["data"]);"#)
        .expect("唯一的喂帧口丢了 ⇒ 分子分母再也进不了账");
    // ⚠ 分流点必须**从喂帧行往后找**：本仓产品注释里出现过那枚分流点的裸串（地雷 7 的
    // 注释级假红，MW4 本轮在同一扇窗里实证并拆掉了），从 0 找会先撞上注释。
    let tail = &arm[feed..];
    let gap = tail
        .find("match kind {")
        .expect("`apply_journal_event` 里喂帧之后找不到分流点");
    let between = &tail[..gap];
    // `tail` 从喂帧串的中段起算 ⇒ 丢掉首行（那是喂帧行自己的尾巴）。
    let gap_lines: Vec<String> = between
        .split('\n')
        .skip(1)
        .map(|l| l.trim().to_string())
        .filter(|t| !t.is_empty() && !t.starts_with("//"))
        .collect();
    assert_eq!(
        gap_lines,
        vec![
            "self.run_stats.note(session, event);".to_string(),
            "self.trajectory_note(session, event);".to_string(),
        ],
        "喂帧与分流点之间只许有三发折叠喂料（`run_stats.note` 后跟批 M4 的 `trajectory_note`）\
         ⇒ 多一行少一行都是臂首次序被改（地雷 1）"
    );
    assert!(
        tail[gap..].contains("\"turn/start\" => {"),
        "喂帧之后找到的那枚 `match kind {{` 不是事件分流点 ⇒ 上面那条序闸是假绿"
    );
    assert_eq!(
        SRC.matches("self.context_meters.track(").count(),
        1,
        "出现了第二发喂帧 ⇒ 同一条帧会被折叠两次（与主干单入口反例）"
    );
    // 喂的是 `data` 那一层，不是整枚信封（`apply_event` 的 `!data.is_object()` 闸）。
    assert!(!arm.contains(r#"self.context_meters.track(session, kind, event)"#));
    // 折叠结果不进脏位：这一发喂帧不许带 `if` 判据（主干无条件刷）。
    let line = arm
        .lines()
        .find(|l| l.contains("self.context_meters.track("))
        .expect("喂帧行");
    assert!(
        line.trim_start().starts_with("let _ ="),
        "喂帧行的形状变了 ⇒ 要么在条件里，要么把返回值当成了刷新判据"
    );
}

/// 换会话 / 进子会话两扇门**都**要复位那本格（地雷 10：只补一扇 = 上一会话的容量留给
/// 下一会话，#137 同族真 bug）。位置判据：`Msg::Select` 那扇里复位必须**晚于**换 active
/// 之前取旧值那一发，且不许自造 `let previous` 绕过现成的 `left` 门。
#[test]
fn switching_sessions_drops_the_left_slot_not_the_new_one() {
    let select = mw4_window(SRC, "Msg::Select(id) => {", "Msg::Search(text)");
    let branched = mw4_window_after(SRC, "Msg::Branched(Ok((rows, child, title))) => {", "Msg::Revoke(message)");
    for (label, arm) in [("Select", &select), ("Branched", &branched)] {
        assert!(
            arm.contains("self.context_meters.reset(left);"),
            "{label} 那扇门没复位容量账 ⇒ 旧会话读数会跟到新会话"
        );
        let take = arm
            .find("let left = self.active.clone();")
            .unwrap_or_else(|| panic!("{label} 那扇门里取旧值的现成门没了"));
        let reset = arm
            .find("self.context_meters.reset(left);")
            .unwrap_or_else(|| panic!("{label} 没找到复位行"));
        assert!(
            reset > take,
            "{label}：复位排在取旧值之前 ⇒ 复位读到的 `left` 是新值"
        );
    }
    assert_eq!(
        mw4_count_word(SRC, "self.context_meters.reset(left);"),
        2,
        "复位点枚数不是恰好两处 ⇒ 与 #135 那两扇 `forget(left)` 门不同口径了"
    );
    // 反向：不许出现「清整本账」那种偏离主干的写法。
    assert!(!SRC.contains("context_meters.clear("), "主干没有整表复位这一发");
}

/// 齐备闸取数口：读的是**当前会话**那一格，缺位返回 `None`。
/// 面板三枚局部常量的值也在这里钉（母本 = `PALETTE_MAX_WIDTH` 那一族的钉法）。
#[test]
fn the_gate_reads_the_active_session_and_the_consts_are_pinned() {
    let gate = mw4_window(SRC, "fn active_occupancy(", "const CONTEXT_PANEL_MIN_WIDTH");
    assert!(gate.contains("let session = self.active.as_deref()?;"));
    assert!(gate.contains("self.context_meters.occupancy(session)"));
    assert_eq!(
        SRC.matches("self.context_meters.occupancy(").count(),
        1,
        "读数口不止一处 ⇒ 齐备闸有了第二条真相"
    );
    assert!(SRC.contains("const CONTEXT_PANEL_MIN_WIDTH: f64 = 250.0;"));
    assert!(SRC.contains("const CONTEXT_PANEL_BAR_HEIGHT: f64 = 4.0;"));
    assert!(SRC.contains("const CONTEXT_PANEL_PADDING: [f64; 4] = [14.0, 12.0, 14.0, 12.0];"));
}

/// 本仓验收口径是 0 error / **0 warning**，靠 `#[allow]` 压掉的警告算缺陷。
/// 这一发只扫 #57 那一族函数的窗（别处历史 allow 不归本片），防「拿 allow 糊死码」。
#[test]
fn the_ring_family_buries_nothing_under_an_allow_attribute() {
    let block = mw4_window(
        SRC,
        "fn active_occupancy(",
        "fn composer_band(&self",
    );
    assert!(!block.contains("#[allow"), "容量圈这一族里出现了 allow ⇒ 警告被糊掉了");
    assert!(!block.contains("#![allow"));
    // 只扫 #57 那一族新落的 kernel 段（3317/3341 那两枚 `unused_imports` 是本轮之前就有的）。
    let geom = &KRN[KRN
        .find("pub const MW3_RING_CHORD_SEGMENTS")
        .expect("kernel.rs 里没有弦近似段")..];
    assert!(
        !geom.contains("#[allow"),
        "弦近似段里出现了 allow ⇒ 这一族的死码被糊掉了"
    );
}

/// 弦近似的宿主半边：段数常量与函数从 `kernel` 里 `use` 进来，且几何住在 kernel、
/// 不落到只读的算子层（`contextmeter.rs` 本轮禁改）。
#[test]
fn the_chord_geometry_lives_in_the_kernel_and_is_imported_once() {
    assert!(SRC.contains("MW3_RING_CHORD_SEGMENTS, mw3_ring_chords,"), "use 面丢了宿主");
    assert_eq!(KRN.matches("pub fn mw3_ring_chords(").count(), 1);
    assert_eq!(mw4_count_word(KRN, "pub const MW3_RING_CHORD_SEGMENTS"), 1);
    // 算子层（禁改面）里不许长出近似：那会造出第二份几何真相。
    let op = include_str!("../src/contextmeter.rs");
    assert!(!op.contains("chord"), "算子层里出现了弦近似 ⇒ 两份真相");
}

/// 行为侧（零依赖 lib 公开面）：**缺分母画空，绝不显 0%**；三态与档位一致。
/// 这一颗是「圈永远收起 vs 圈在但 0%」两档的主判据，文本锁替不了。
#[test]
fn a_missing_denominator_is_absent_not_zero_percent() {
    let mut book = ContextMeterBook::new();
    // 只有分子、没有分母 ⇒ 没有读数（主干 `CM:130-133` 的 `window > 0` 闸）。
    book.track(
        "s1",
        "assistant/message",
        &json!({"usage": {"inputTokens": 1200, "cacheReadTokens": 0, "cacheWriteTokens": 0}}),
    );
    assert_eq!(
        book.occupancy("s1"),
        None,
        "缺分母居然给了读数 ⇒ 圈会凭空显一个百分比"
    );
    // 补上分母 ⇒ 立刻有数，且不是 0%。
    book.track("s1", "request/context", &json!({"contextWindow": 200_000}));
    let occ = book.occupancy("s1").expect("补齐后该有读数");
    assert_eq!((occ.used, occ.window), (1200, 200_000));
    assert_eq!(occ.percent, 1, "1200/200000 四舍五入应为 1%，不是 0");
    // 采到的 0 是合法读数：圈在场、只剩轨道，与「没有读数」两回事。
    book.track(
        "s2",
        "assistant/message",
        &json!({"usage": {"inputTokens": 0, "cacheReadTokens": 0, "cacheWriteTokens": 0}}),
    );
    book.track("s2", "request/context", &json!({"contextWindow": 200_000}));
    let zero = book.occupancy("s2").expect("全零 usage 是合法采样");
    assert_eq!(zero.percent, 0);
    assert!(matches!(face_of(zero.percent), RingFace::TrackOnly));
    // 复位只删那一格。
    assert!(book.reset("s1"));
    assert_eq!(book.occupancy("s2").map(|o| o.percent), Some(0));
    assert_eq!(book.occupancy("s1"), None);
}

/// 行为侧第二颗：三态各自的弦数与端点归属（`TrackOnly`/`Full` 出空表、`Arc` 出满 N 段）。
#[test]
fn the_three_faces_map_to_the_expected_chord_shape() {
    assert!(matches!(face_of(0), RingFace::TrackOnly));
    assert!(matches!(face_of(100), RingFace::Full));
    assert_eq!(mw3_ring_chords(0, MW3_RING_CHORD_SEGMENTS).len(), 0);
    assert_eq!(mw3_ring_chords(100, MW3_RING_CHORD_SEGMENTS).len(), 0);
    let chords = mw3_ring_chords(38, MW3_RING_CHORD_SEGMENTS);
    assert_eq!(chords.len(), MW3_RING_CHORD_SEGMENTS);
    let last = chords.last().expect("非空");
    let RingFace::Arc(g) = face_of(38) else {
        panic!("38% 不是弧档");
    };
    assert!(
        (last[2] - g.end_x).abs() < 1e-9 && (last[3] - g.end_y).abs() < 1e-9,
        "弦末点没落回真弧终点"
    );
    // 所有端点在圆上 ⇒ 内接折线，鼓不出 14×14 的盒。
    for seg in chords {
        for (x, y) in [(seg[0], seg[1]), (seg[2], seg[3])] {
            let d = ((x - RING_CENTER).powi(2) + (y - RING_CENTER).powi(2)).sqrt();
            assert!((d - RING_RADIUS).abs() < 1e-9);
        }
    }
    assert_eq!(FULL_PERCENT, 100);
}

/// 文案一发两贴的行为半边：`tooltip_and_aria` 只出一枚串（宿主把它同时喂两处）。
#[test]
fn the_tooltip_string_is_one_value_from_the_template() {
    let catalog = blade2_rs::i18n::Catalog::load("zh", None);
    let occ = blade2_rs::contextmeter::ContextOccupancy {
        used: 76_000,
        window: 200_000,
        percent: 38,
    };
    let s = tooltip_and_aria(&catalog, occ);
    assert!(s.contains("38"), "百分比没进串：{s}");
    assert_eq!(s.matches('%').count(), 1, "串里出现了第二枚百分号 ⇒ 模板被改坏");
}

/// #57 的**七档亮/收表**（母本 = `specs/st5-report.md` §3 帧台账 + §5B-5 的矩阵），在宿主这一侧
/// 离线复现：桩侧 `--ctx=1..7` 的片形逐字搬进来，喂给 `ContextMeterBook`（也就是
/// `apply_journal_event` 那一发 `track` 走的同一本账），断言「圈亮在哪档、收在哪档、
/// 各档的 percent 落在哪」。
///
/// 为什么值得钉：这七档里三档考的是**折叠器最容易写错的三条闸** ——
/// · 档 3 毒分母排在正常那发**之后** ⇒ 「非数/0/负数要忽略并保留上一份」（认错过一次就红）；
/// · 档 4 的 `chunk.type` 是数字 ⇒ 主干那发 `ct.GetString()` 抛 ⇒ **整帧作废**，
///   「跳过毒片继续扫」的实现会记上 777000 而露馅；
/// · 档 7 那颗 `assistant/attempt` 带着 `data.usage` 诱饵 ⇒ 官方对 attempt **不读** `data.usage`，
///   读错口径就是一张假满圈（555000/128000 封顶 100%）。
/// 端到端那半边（真 socket + 逐档帧预算 36/37/37/40/37/37/37/37）住在 `tests/ipc.rs`，
/// 那是 MW4 的禁改面 ⇒ 记进报告 §5「需接力」。
#[test]
fn the_seven_ctx_tiers_fold_to_the_documented_light_and_dark_outcomes() {
    // 三桶 900 的那发正文 usage：同帧 `totalTokens` 是 1234 ⇒ 两值不等就是「别拿
    // totalTokens / outputTokens 当分子」的哨兵（st5 §3.4）。
    let message_usage = json!({
        "usage": {
            "inputTokens": 900,
            "outputTokens": 420,
            "totalTokens": 1234,
            "cacheReadTokens": 0,
            "cacheWriteTokens": 0
        }
    });
    let window_128k = json!({"contextWindow": 128_000, "model": "m", "provider": "p"});

    // 档 1：齐备 ⇒ 亮，900/128000 = 0.70% → 1%（弧档，绝不是 TrackOnly）。
    let mut b = ContextMeterBook::new();
    b.track("t1", "request/context", &window_128k);
    b.track("t1", "assistant/message", &message_usage);
    let one = b.occupancy("t1").expect("档 1 该亮");
    assert_eq!((one.used, one.window, one.percent), (900, 128_000, 1));
    assert!(matches!(face_of(one.percent), RingFace::Arc(_)), "1% 画成轨道 = 看不见");

    // 档 2：内核省略形（整格不带 `contextWindow`）⇒ 缺分母 ⇒ 收，且**不显 0%**。
    let mut b = ContextMeterBook::new();
    b.track("t2", "request/context", &json!({"model": "m", "provider": "p"}));
    b.track("t2", "assistant/message", &message_usage);
    assert_eq!(b.occupancy("t2"), None, "档 2 该收（分母整格缺位）");

    // 档 3：正常一发之后紧跟三发毒分母（字符串 / 0 / 负数）⇒ 全部忽略，仍是档 1 的 1%。
    let mut b = ContextMeterBook::new();
    b.track("t3", "request/context", &window_128k);
    b.track("t3", "assistant/message", &message_usage);
    for poisoned in ["128000", "0", "-5"] {
        let frame = if poisoned == "128000" {
            json!({"contextWindow": "128000"})
        } else if poisoned == "0" {
            json!({"contextWindow": 0})
        } else {
            json!({"contextWindow": -5})
        };
        b.track("t3", "request/context", &frame);
    }
    assert_eq!(b.occupancy("t3"), Some(one), "档 3：毒分母覆盖了上一份正分母");

    // 档 4：`chunk.type` 是数字 ⇒ 整帧作废 ⇒ 分子从没采到过 ⇒ 收。
    let mut b = ContextMeterBook::new();
    b.track("t4", "request/context", &window_128k);
    b.track(
        "t4",
        "assistant/message",
        &json!({
            "usage": {"inputTokens": 900},
            "stream": [{"type": "chunk", "chunk": {"type": 7, "usage": {"inputTokens": 777_000}}}]
        }),
    );
    assert_eq!(b.occupancy("t4"), None, "档 4 该收：毒片要是被「跳过继续扫」就会记上 777000");

    // 档 5：stream 的 usage 片覆盖 `data.usage`（last-wins）⇒ 1200+800+50000 = 52000 → 41%。
    let mut b = ContextMeterBook::new();
    b.track("t5", "request/context", &window_128k);
    b.track(
        "t5",
        "assistant/message",
        &json!({
            "usage": {"inputTokens": 900, "outputTokens": 420},
            "stream": [{
                "type": "chunk",
                "chunk": {"type": "usage", "usage": {
                    "inputTokens": 50_000, "outputTokens": 900,
                    "cacheReadTokens": 1200, "cacheWriteTokens": 800
                }}
            }]
        }),
    );
    let five = b.occupancy("t5").expect("档 5 该亮");
    assert_eq!((five.used, five.percent), (52_000, 41), "last-wins 那档漂了");

    // 档 6：`data.usage` 整格删掉 ⇒ Absent ⇒ 收（与档 1 逐字只差这一个键）。
    let mut b = ContextMeterBook::new();
    b.track("t6", "request/context", &window_128k);
    b.track("t6", "assistant/message", &json!({}));
    assert_eq!(b.occupancy("t6"), None, "档 6 该收");

    // 档 7：`assistant/attempt` 自带的 `data.usage` 是诱饵（官方对 attempt 不读）⇒ 分子仍是 900。
    let mut b = ContextMeterBook::new();
    b.track("t7", "request/context", &window_128k);
    b.track("t7", "assistant/message", &message_usage);
    b.track(
        "t7",
        "assistant/attempt",
        &json!({
            "usage": {"inputTokens": 555_000, "outputTokens": 1},
            "stream": [{"type": "chunk", "chunk": {"type": "finish", "reason": {"kind": "error"}}}]
        }),
    );
    assert_eq!(b.occupancy("t7"), Some(one), "档 7：读了 attempt 的 usage 就会画一张假满圈");

    // 复位是**按格**删的：同一本账里另一条会话不许被连带清掉（`Msg::Select` 那扇门要的正是这个）。
    let mut book = ContextMeterBook::new();
    for sid in ["keep", "drop"] {
        book.track(sid, "request/context", &window_128k);
        book.track(sid, "assistant/message", &message_usage);
    }
    assert!(book.reset("drop"), "第一发复位该真的删掉一格");
    assert_eq!(book.occupancy("drop"), None);
    assert_eq!(book.occupancy("keep").map(|o| o.percent), Some(1));
    assert!(!book.reset("drop"), "第二发复位该 false（格已经没了）");
}
