//! #138「右栏 dock 宿主 + chrome 19 枚」配套源码锁的**可证伪性取证台**（FX1 轮）。
//!
//! 产品态那 6 颗锁住在 `src/main.rs` 的 `mod dock_chrome_tests` 里（母本 §3 要求的形态：
//! 与产品码同文件、走 `include_str!("main.rs")`）。本文件不重复断言产品态，只回答一个
//! 主代理会追问的问题：**「把那一发接线拆掉，对应那把锁会不会真的红」**。
//!
//! 做法：在**内存里**对同一份 `src/main.rs` 文本施加定向变异（绝不落盘、绝不改 `src/`、
//! 绝不 `git` 写），把变异前/变异后各跑一遍那条 needle 判据，要求
//! 「变异前成立 + 变异后不成立」。变异后仍成立 ⇒ 那颗锁是恒真形状，本文件直接红。
//! 这里的 needle 整串写死无妨：本文件的文本不会被 `include_str!("main.rs")` 数到。

const FX1_SRC: &str = include_str!("../src/main.rs");

/// 窗 = `start` 首次出现 → 其后 `end` 首次出现（与产品态那 6 颗同一口径）。
fn fx1_window(hay: &str, start: &str, end: &str) -> String {
    let i = hay.find(start).unwrap_or_else(|| panic!("起点锚丢了：{start}"));
    let j = hay[i + start.len()..]
        .find(end)
        .unwrap_or_else(|| panic!("窗 {start} 里没有终点锚 {end}"));
    hay[i..i + start.len() + j].to_string()
}

/// 带词界的计数（`dock::chrome::MENU` 不该数到 `MENU_OPEN`）。
fn fx1_count_word(hay: &str, needle: &str) -> usize {
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

/// 两态：判据在当前产品态必须成立；定向变异之后必须**不**成立。
fn fx1_two_state(label: &str, from: &str, to: &str, holds: impl Fn(&str) -> bool) {
    assert!(
        holds(FX1_SRC),
        "{label}：当前产品态判据就不成立 ⇒ 被取证的那颗锁本身已经坏了"
    );
    assert!(
        FX1_SRC.contains(from),
        "{label}：变异串 `{from}` 没打进产品码 ⇒ 这一发变异是空操作，取证无效"
    );
    let mutated = FX1_SRC.replace(from, to);
    assert_ne!(
        mutated,
        FX1_SRC.to_string(),
        "{label}：变异没改动文本"
    );
    assert!(
        !holds(&mutated),
        "{label}：拆掉 `{from}` 之后判据仍然成立 ⇒ 那颗锁是恒真形状，锁不住这一发接线"
    );
}

/// 判据①（`every_dock_msg_is_declared_once_armed_once_and_reached_from_the_view`）：
/// 渲染层里那一发视图引用被拆走 ⇒ 锁必须红。
#[test]
fn fx1_lock_dock_msg_view_trigger_is_falsifiable() {
    fx1_two_state(
        "锁①·视图入口那一发（DockInnerDragEnd）",
        ".on_pointer_released(context.callback(|_| Msg::DockInnerDragEnd));",
        ".on_pointer_released(context.callback(|_| Msg::DockMenuClose));",
        |s| {
            fx1_window(s, "fn dock_host(", "fn session_row(")
                .contains("Msg::DockInnerDragEnd")
        },
    );
    fx1_two_state(
        "锁①·菜单行的 DockCloseTab 那一发",
        "Some(dock::chrome::CLOSE_TAB) => Msg::DockCloseTab(tab_id.to_string()),",
        "Some(dock::chrome::CLOSE_TAB) => Msg::DockAddTab,",
        |s| {
            fx1_window(s, "fn dock_host(", "fn session_row(").contains("Msg::DockCloseTab")
        },
    );
    // 臂被复制成两发（同一发两个写者）⇒ 「恰好一臂」那一半也必须红，不能只靠视图那一半。
    fx1_two_state(
        "锁①·DockSplit 的 reducer 臂被复制",
        "Msg::DockSplit(mode) => self.dock_split(mode),",
        "Msg::DockSplit(mode) => self.dock_split(mode),\n            Msg::DockSplit(mode) => self.dock_split(mode),",
        |s| {
            s.lines()
                .filter(|l| l.trim_start().starts_with("Msg::DockSplit") && l.contains("=>"))
                .count()
                == 1
        },
    );
}

/// 判据②（`the_chrome_roster_is_reached_cell_by_cell_and_the_untree_one_stays_out`）：
/// 一格 id 被摘掉 ⇒ 红；给「在册不入树」那颗挂上树 ⇒ 反向哨兵必须红。
#[test]
fn fx1_lock_chrome_roster_is_falsifiable() {
    fx1_two_state(
        "锁②·CELL1 那一格",
        "(dock::chrome::CELL1, \"右栏第二格\", Msg::DockClosePane1)",
        "(dock::chrome::CELL0, \"右栏第二格\", Msg::DockClosePane1)",
        |s| fx1_window(s, "fn dock_host(", "fn session_row(").contains("dock::chrome::CELL1"),
    );
    fx1_two_state(
        "锁②·在册不入树（PANE1_PANEL 被挂进树）",
        "Some(dock::chrome::CLOSE_TAB) => self.dock.can_close_tab(),",
        "Some(dock::chrome::PANE1_PANEL) => self.dock.can_close_tab(),",
        |s| fx1_count_word(s, "dock::chrome::PANE1_PANEL") == 0,
    );
    fx1_two_state(
        "锁②·自造 UIA 裸串（多一处 automation_id 裸串）",
        ".automation_id(\"FilesPanelCloseButton\")",
        ".automation_id(\"FilesPanelCloseButton\")\n            .automation_id(\"MyMadeUpId\")",
        |s| {
            fx1_window(s, "fn dock_host(", "fn session_row(")
                .matches(".automation_id(\"")
                .count()
                == 1
        },
    );
}

/// 判据③（`the_right_dock_is_one_gated_grid_column_with_no_second_truth`）：
/// 闸少读一个条件 / 键改名 / 冒第二处键 ⇒ 各自都要红。
#[test]
fn fx1_lock_host_gate_is_falsifiable() {
    fx1_two_state(
        "锁③·宿主闸丢条件",
        "let host_visible = self.dock.open && on_chat;",
        "let host_visible = on_chat;",
        |s| {
            fx1_window(s, "fn body(", "fn column_splitter(")
                .contains("let host_visible = self.dock.open && on_chat;")
        },
    );
    fx1_two_state(
        "锁③·宿主键漂移",
        "cells.push(KeyedView::new(\"right-dock\", self.dock_host(context, p)));",
        "cells.push(KeyedView::new(\"right-dock-x\", self.dock_host(context, p)));",
        |s| {
            fx1_window(s, "fn body(", "fn column_splitter(")
                .contains("KeyedView::new(\"right-dock\", self.dock_host(context, p))")
        },
    );
    fx1_two_state(
        "锁③·浮层键丢一处（改两处同名键）",
        "cells.push(KeyedView::new(\"dock-menu\", self.dock_tab_menu_layer(context, p)));",
        "cells.push(KeyedView::new(\"dock-menu\", self.dock_tab_menu_layer(context, p)));\n            cells.push(KeyedView::new(\"dock-menu\", self.dock_tab_menu_layer(context, p)));",
        |s| s.matches("\"dock-menu\"").count() == 1,
    );
    fx1_two_state(
        "锁③·宿主被塞回聊天页内部",
        "fn chat_page(&self, context: &mut ViewContext<Shell>, p: Palette) -> View {",
        "fn chat_page(&self, context: &mut ViewContext<Shell>, p: Palette) -> View {\n        let _ = self.dock_host(context, p);",
        |s| {
            !fx1_window(s, "fn chat_page(", "fn command_palette(")
                .contains("self.dock_host(")
        },
    );
    fx1_two_state(
        "锁③·第二份真相复活",
        "let host_visible = self.dock.open && on_chat;",
        "let host_visible = self.right_host_visible;",
        |s| fx1_count_word(s, "self.right_host_visible") == 0,
    );
}

/// 判据④（`opening_the_dock_menu_retires_every_sibling_layer`）：兄弟层漏收一处 /
/// 收点枚数漂走 ⇒ 都要红。
#[test]
fn fx1_lock_dock_menu_mutex_is_falsifiable() {
    for (sibling, field) in [
        ("file_menu_close", "file_menu"),
        ("message_details_close", "message_details"),
        ("session_ref_list_close", "session_refs"),
        ("workspace_menu_close", "workspace_menu"),
        ("todos_panel_close", "todos_panel"),
        ("selector_close", "selectors"),
        ("danger_confirm_close", "permission_confirm"),
    ] {
        let needle = format!("{sibling}(&mut self.{field});");
        let arm_window = |s: &str| {
            fx1_window(s, "Msg::DockMenuToggle(id) => {", "Msg::DockMenuClose =>")
        };
        let needle2 = needle.clone();
        fx1_two_state(
            &format!("锁④·dock 菜单臂漏收 {sibling}"),
            &needle,
            &format!("// {sibling} 被撤掉了"),
            move |s| arm_window(s).contains(&needle2),
        );
    }
    fx1_two_state(
        "锁④·收点枚数 19 漂走（把 19 处全换成直写）",
        "dock_menu_close(&mut self.dock_menu);",
        "self.dock_menu = None;",
        |s| fx1_count_word(s, "dock_menu_close(&mut self.dock_menu);") == 19,
    );
}

/// 判据⑤（`the_inner_splitter_wires_exactly_three_pointer_events_and_persists_nothing`）。
#[test]
fn fx1_lock_inner_splitter_is_falsifiable() {
    let inner = |s: &str| fx1_window(s, "fn dock_inner_splitter(", "fn dock_tab_menu_layer(");
    fx1_two_state(
        "锁⑤·第四发事件（released 写成 pressed）",
        ".on_pointer_released(context.callback(|_| Msg::DockInnerDragEnd));",
        ".on_pointer_pressed(context.callback(|_| Msg::DockInnerDragEnd));",
        |s| inner(s).matches(".on_pointer_released(").count() == 1,
    );
    fx1_two_state(
        "锁⑤·补了一发 capture-lost（主干这一条没有）",
        ".on_pointer_released(context.callback(|_| Msg::DockInnerDragEnd));",
        ".on_pointer_released(context.callback(|_| Msg::DockInnerDragEnd))\n            .on_pointer_capture_lost(context.callback(|_| Msg::DockInnerDragEnd));",
        |s| !inner(s).contains("capture_lost"),
    );
    fx1_two_state(
        "锁⑤·拖拽路上落盘",
        ".capture_pointer_on_press(Some(true))",
        ".capture_pointer_on_press(Some(false))",
        |s| inner(s).contains("capture_pointer_on_press(Some(true))"),
    );
    fx1_two_state(
        "锁⑤·只骑一侧（竖档那发负边距写成 0）",
        ".margin(th([0.0, overlap, 0.0, overlap]))",
        ".margin(th([0.0, 0.0, 0.0, 0.0]))",
        |s| inner(s).contains("th([0.0, overlap, 0.0, overlap])"),
    );
}

/// 判据⑥（`the_chrome_numeric_cells_match_the_trunk_literals`）。
#[test]
fn fx1_lock_numeric_cells_are_falsifiable() {
    let chip = |s: &str| fx1_window(s, "fn dock_tab_chip(", "fn dock_body(");
    let tail = |s: &str| fx1_window(s, "fn dock_tail_button(", "fn dock_tab_chip(");
    let strip = |s: &str| fx1_window(s, "fn dock_tab_strip(", "fn dock_tail_button(");
    fx1_two_state(
        "锁⑥·chip MaxWidth 140 → 148",
        ".max_width(140.0)",
        ".max_width(148.0)",
        |s| chip(s).contains(".max_width(140.0)"),
    );
    fx1_two_state(
        "锁⑥·chip 内边距 10,4,10,4 漂移",
        "set(\"ButtonPadding\", th([10.0, 4.0, 10.0, 4.0]))",
        "set(\"ButtonPadding\", th([10.0, 4.0, 10.0, 8.0]))",
        |s| chip(s).contains("th([10.0, 4.0, 10.0, 4.0])"),
    );
    fx1_two_state(
        "锁⑥·chip 圆角 6 → 8",
        ".corner_radius(6.0)",
        ".corner_radius(8.0)",
        |s| chip(s).contains(".corner_radius(6.0)"),
    );
    fx1_two_state(
        "锁⑥·尾部钮那颗 28 边长换成别的档",
        "side: size::COMPOSER_ADD_BUTTON,",
        "side: size::PILL,",
        |s| tail(s).contains("size::COMPOSER_ADD_BUTTON"),
    );
    fx1_two_state(
        "锁⑥·标签条内边距 6,4,6,0 丢档",
        ".padding(th([6.0, 4.0, 6.0, 0.0]))",
        ".padding(th([6.0, 6.0, 6.0, 6.0]))",
        |s| strip(s).contains("th([6.0, 4.0, 6.0, 0.0])"),
    );
}
