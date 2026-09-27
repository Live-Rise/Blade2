//! #142 / SW1 落盘层接线轮的**离线源码锁**。
//!
//! 口径：本轮禁止一切前台行为（不起分叉 exe、不截图、不 UIA），所以「这处触发点真的调了
//! shellfiles 那个函数」只能靠**源码文本本身**取证 —— 与 `src/main.rs` 末尾那 92 颗
//! `include_str!("main.rs")` 锁同一套方法论，只是这里把它搬到**独立文件**里，好处很实在：
//!
//! · 本文件不在 `include_str!` 的射程内 ⇒ 所有 needle 都可以**整串写死**，不必像 `main.rs`
//!   内部 mod 那样 `concat!` 拆词防自匹配；计数探针也只吃产品码一层（两层自匹配坑）。
//! · 每颗锁都只锁**调用形状**（带 `shellfiles::` 前缀、带实参），不锁注释、不锁文档串，
//!   所以「画了一张卡但从不写盘」这种死代码骗不过去。
//!
//! ⚠ 锚点全是**首次出现**语义（`split_once`）：改名、删锚、或在更上方造出一枚同名锚，
//!   都会让对应窗口平移 —— 这正是 `main.rs` 里那把 L4（皮肤滑杆读值窗）的踩法，本文件
//!   的 `the_skin_opacity_key_stays_a_bare_string_at_exactly_one_site` 就是专门钉它的。

const SRC: &str = include_str!("../src/main.rs");

/// 窗口助手（口径同 `main.rs` 内部 mod 那份）：锚点找不到就 panic，绝不静默把下一颗函数
/// 卷进断言 —— 那会把「窗口内不含 X」这种负锁洗成假绿。
fn window<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let (_, rest) = source
        .split_once(start)
        .unwrap_or_else(|| panic!("源码里找不到锚点 {start}"));
    rest.split_once(end)
        .unwrap_or_else(|| panic!("锚点 {start} 之后找不到 {end}：窗口会一路扩到文件末尾"))
        .0
}

/// 位置探针（比 `contains` 强在能锁**先后**）；找不到就 panic。
fn at(haystack: &str, needle: &str) -> usize {
    haystack
        .find(needle)
        .unwrap_or_else(|| panic!("窗口里找不到探针串 {needle}"))
}

/// 整文件计数（只吃 `src/main.rs` 一层，本测试文件不在射程内）。
fn count(needle: &str) -> usize {
    SRC.matches(needle).count()
}

fn need(win: &str, needles: &[&str]) {
    for needle in needles {
        assert!(win.contains(needle), "窗口里缺探针串 {needle}");
    }
}

/// 触发点 ①（启动）：开壳读三类已落盘的东西，并**只读不写**。
///
/// 主干口径：`LoadShellOptions`（`MainWindow.TraySettings.cs:66-91`）、`LoadSkinOpacity`
/// （`MainWindow.xaml.cs:8618-8633`）、`LoadInstructions`（`MainWindow.Personalization.cs:222-243`）
/// 都在构造期读、都不写盘 ⇒ 分叉这里若在 boot 里写一次盘，就会凭空多出主干不会多的字节。
#[test]
fn boot_reads_the_three_persisted_sources_and_never_writes() {
    let boot = window(
        SRC,
        "let layout_prefs = layout::load_prefs();",
        "shellfiles::data_home().display()",
    );
    need(
        boot,
        &[
            "shellfiles::load_shell_options()",
            "shellfiles::load_skin_opacity()",
            "shellfiles::load_instructions()",
            "shellfiles::render_shell_json(&shell_options)",
            "shellfiles::window_material_index_for_id(shell_options.normalized_material())",
            "shellfiles::bubble_material_index_for_id(",
            "shellfiles::percent_from_bubble_opacity(shell_options.bubble_opacity)",
            "DIAG: SHELL boot",
        ],
    );
    // 负锁：boot 段一发盘都不许写（这三串在整文件里另有其位，所以必须**在窗口内**判）。
    for forbidden in [
        "shellfiles::save_shell_options(",
        "shellfiles::save_skin_opacity(",
        "shellfiles::write_instructions(",
        "self.write_shell_json(",
        "self.flush_instructions_draft(",
        "self.flush_skin_opacity(",
    ] {
        assert!(
            !boot.contains(forbidden),
            "启动段出现了写盘调用 {forbidden}：主干构造期只读不写"
        );
    }
    // 读盘结果必须**并进状态格**（否则就是读了不用的死码）。
    assert!(
        at(boot, "shellfiles::render_shell_json(&shell_options)")
            < at(boot, "shell_json_saved = ")
            || boot.contains("shell_json_saved = shellfiles::render_shell_json"),
        "渲染结果没被存成脏检基线"
    );
}

/// 触发点 ②（材质下拉）：两档材质 ⇒ `shell.json`。
///
/// 同时钉主干顺序：写格子在前、落盘在后（`SetShellMaterial` 先 `if (id == _x) return;`
/// 再 `SaveShellOptions()` 再应用视觉，`MainWindow.xaml.cs:18750-18771`）。
#[test]
fn material_picks_reach_shell_json_after_the_state_grid() {
    let arm = window(SRC, "Msg::Pick(key, index) => {", "Msg::Toggle(key, on) => {");
    need(
        arm,
        &[
            "SHELL_MATERIAL_KEY | BUBBLE_MATERIAL_KEY => self.write_shell_json(),",
            "self.set_pick(&key, index);",
        ],
    );
    assert_eq!(
        count("SHELL_MATERIAL_KEY | BUBBLE_MATERIAL_KEY => self.write_shell_json(),"),
        1,
        "材质 ⇒ 落盘这条臂只该有一处真相"
    );
    // 顺序：`permission` 那颗有闸的仍排最前，`set_pick` 排在材质分流之前。
    assert!(
        at(arm, "if key == PERMISSION_PICK_KEY {") < at(arm, "self.set_pick(&key, index);"),
        "权限档的闸门被挪到了通用写点之后"
    );
    assert!(
        at(arm, "self.set_pick(&key, index);") < at(arm, "self.write_shell_json()"),
        "材质分流跑到了状态格前面：主干先改值再落盘"
    );
}

/// 触发点 ③（四颗托盘开关）：写格子无条件在前 + 两条互锁都在 + 整条臂只落盘一次。
///
/// 主干四把 setter（`MainWindow.TraySettings.cs:324-363`）都**没有**脏检 ⇒ 分叉这边
/// `write_shell_json` 的内容脏检是唯一那道闸，不能拿它当少写的借口。
#[test]
fn tray_switches_reach_shell_json_and_both_interlocks_survive() {
    let arm = window(SRC, "Msg::Toggle(key, on) => {", "Msg::Note(text) =>");
    need(
        arm,
        &[
            "self.set_on(&key, on);",
            "TRAY_SHOW_ICON_KEY | TRAY_MINIMIZE_KEY | TRAY_CLOSE_KEY",
            "options.with_tray_icon_off();",
            "options.show_tray_icon = true;",
            "self.sync_shell_switches(&options);",
            "self.write_shell_json();",
        ],
    );
    assert!(
        at(arm, "self.set_on(&key, on);") < at(arm, "match key.as_str()"),
        "`set_on` 被挪进了判据里：所有开关都会丢状态"
    );
    assert_eq!(
        arm.matches("self.write_shell_json();").count(),
        1,
        "托盘臂里落盘发数应为 1（互锁只改状态格，不各写一发）"
    );
    // 互锁①：关父项必须先压子项、再落盘（顺序错了就会写出一份「托盘没有却有藏窗口」的盘）。
    assert!(
        at(arm, "options.with_tray_icon_off();") < at(arm, "self.sync_shell_switches(&options);"),
        "先回填状态格再改快照 ⇒ 落盘的还是旧值"
    );
    assert!(
        at(arm, "if key == TRAY_SHOW_ICON_KEY && !on {") < at(arm, "options.show_tray_icon = true;"),
        "两条互锁的先后与主干相反（主干先 `SetShowTrayIcon` 再 `EnsureTrayVisible`）"
    );
}

/// 触发点 ④（AGENTS.md 输入）：每次按键**只武装**，一发盘都不许摸。
///
/// 主干 `TextBox_TextChanged` → `StartInstructionsSaveTimer()`（1 s `DispatcherQueueTimer`，
/// `MainWindow.Personalization.cs:361-375`）；写盘只在 `SaveInstructions`（`:278-309`）。
#[test]
fn instructions_field_arms_the_debounce_without_touching_disk() {
    let arm = window(SRC, "Msg::Field(key, value) => {", "Msg::KeyFocus(owner) =>");
    need(
        arm,
        &[
            "shellfiles::truncate_instructions(&value).to_string()",
            "set_pair(&mut self.fields, key, value);",
            "self.request_instructions_save(&draft, context);",
        ],
    );
    assert!(
        at(arm, "set_pair(&mut self.fields, key, value);")
            < at(arm, "self.request_instructions_save(&draft, context);"),
        "状态格没排在落盘分流之前：这一臂今天所有输入框都靠它写格子"
    );
    for forbidden in [
        "shellfiles::write_instructions(",
        "shellfiles::save_shell_options(",
        "self.write_shell_json(",
    ] {
        assert!(
            !arm.contains(forbidden),
            "输入臂里出现裸写盘 {forbidden}：每次按键落一次盘就是主干明令避免的形状"
        );
    }
    assert_eq!(
        arm.matches("self.request_instructions_save(&draft, context);").count(),
        1,
        "输入臂里武装发数应为 1"
    );
    assert_eq!(
        count("self.request_instructions_save(&draft, context);"),
        2,
        "武装点应为「输入臂」+「到点复查发现又改过 ⇒ 重武装」两处，多一处就是两份真相"
    );
}

/// 触发点 ④ 的落盘那一发：`write_instructions` 全仓只此一处，且失败要出声。
#[test]
fn instructions_flush_is_the_only_writer_and_reports_both_outcomes() {
    let flush = window(SRC, "fn flush_instructions_draft", "fn flush_skin_opacity");
    need(
        flush,
        &[
            "let live = self.field(INSTRUCTIONS_KEY, \"\");",
            "shellfiles::truncate_instructions(&live).to_string()",
            "if draft != self.instructions_pending {",
            "if draft == self.instructions_saved {",
            "match shellfiles::write_instructions(&draft) {",
            "self.instructions_failed = true;",
            "self.instructions_note = Some(error.to_string());",
            "shellfiles::agents_path().display()",
        ],
    );
    assert_eq!(
        count("shellfiles::write_instructions("),
        1,
        "AGENTS.md 的写点必须只有一个：两个就是两份真相"
    );
    // 脏检排在写盘之前（主干 `if (!_instructionsDirty) return;`，`Personalization.cs:279`）。
    assert!(
        at(flush, "if draft == self.instructions_saved {")
            < at(flush, "shellfiles::write_instructions("),
        "没做脏检就落盘：每次停手都会重写一遍 AGENTS.md"
    );
    assert!(
        at(flush, "if draft != self.instructions_pending {")
            < at(flush, "shellfiles::write_instructions("),
        "丢了「窗口内又改过 ⇒ 重武装、这一发不写」那一支"
    );
    // 状态行必须真的被视图读走（否则失败/成功都无人可见 = 死码）。
    assert_eq!(count("settings_line(\"status\", &self.instructions_status(), p)"), 1);
    assert!(SRC.contains("lf(\"保存失败：{0}\""));
    assert!(SRC.contains("lf(\"已自动保存 · {0}\""));
}

/// 触发点 ⑤（两颗滑杆）：**相反**的落盘时机必须分流。
///
/// 主干 `SetBubbleOpacity`（`MainWindow.xaml.cs:18782-18795`）文档注释原话「不做防抖也够轻」
/// ⇒ 每发都写、只带一道 `< 0.0001` 脏检；`skin.opacity` 反而有 300 ms 防抖
/// （`MainWindow.xaml.cs:8723-8738`）。接反了就是一边多写、一边少写。
#[test]
fn the_two_opacity_sliders_split_their_write_policies() {
    let arm = window(SRC, "Msg::Number(key, value) => {", "Msg::FontStep(step) => {");
    need(
        arm,
        &[
            "set_pair(&mut self.numbers, key, value);",
            "shellfiles::parse_shell_json(&self.shell_json_saved);",
            "shellfiles::bubble_opacity_from_percent(value)",
            "shellfiles::bubble_opacity_unchanged(written.bubble_opacity, next)",
            "self.write_shell_json();",
            "self.request_skin_opacity_save(context);",
        ],
    );
    assert!(
        at(arm, "set_pair(&mut self.numbers, key, value);") < at(arm, "if bubble"),
        "状态格没排在分流之前：所有滑杆都会丢值"
    );
    // 气泡：脏检夹在「改格子」与「落盘」之间，且**不**武装定时器。
    assert!(
        at(arm, "if !shellfiles::bubble_opacity_unchanged(")
            < at(arm, "self.write_shell_json();"),
        "气泡不脏检 ⇒ 拖动期间每像素重写整份 shell.json（主干有 0.0001 那道闸）"
    );
    assert!(
        !arm.contains("self.request_skin_opacity_save(context);")
            || at(arm, "else if skin") < at(arm, "self.request_skin_opacity_save(context);"),
        "皮肤那颗的武装没排在 skin 分支里"
    );
    // 皮肤：这一臂绝不直接写盘。
    assert!(
        !arm.contains("shellfiles::save_skin_opacity("),
        "皮肤滑杆在按键臂里直接落盘：主干是 300 ms 防抖"
    );
    assert!(
        !arm.contains("self.request_instructions_save("),
        "滑杆臂不该武装 AGENTS.md"
    );
}

/// 触发点 ⑤ 的另一半：到点那一发只写百分数标度的 `shell-skin.opacity`。
#[test]
fn skin_opacity_flush_writes_the_percent_form_exactly_once() {
    let flush = window(SRC, "fn flush_skin_opacity", "fn shell_clock_hms");
    need(
        flush,
        &[
            "let percent = self.number(SKIN_OPACITY_KEY, size::SKIN_OPACITY_DEFAULT);",
            "if percent != self.skin_opacity_target {",
            "shellfiles::save_skin_opacity(percent);",
            "shellfiles::render_skin_opacity(percent)",
            "shellfiles::skin_opacity_path().display()",
        ],
    );
    assert_eq!(count("shellfiles::save_skin_opacity("), 1);
    assert!(
        at(flush, "if percent != self.skin_opacity_target {")
            < at(flush, "shellfiles::save_skin_opacity("),
        "没复查「窗口内又拖过」这一判 ⇒ 会落一个用户已经不要的值"
    );
    // 单位锁：`load_skin_opacity` 回的是百分数标度，这里再乘/除 100 就是双份换算。
    assert!(
        !flush.contains("percent / 100") && !flush.contains("percent * 100"),
        "在调用方二次换算：`render_skin_opacity` 自己会 /100"
    );
}

/// 两发防抖的**时钟**必须与主干同值：AGENTS.md 1 s、`shell-skin.opacity` 300 ms。
#[test]
fn both_debounce_intervals_match_the_mainline_timers() {
    need(
        SRC,
        &[
            "const INSTRUCTIONS_SAVE_DEBOUNCE: Duration = Duration::from_millis(1000);",
            "const SKIN_OPACITY_SAVE_DEBOUNCE: Duration = Duration::from_millis(300);",
        ],
    );
    // 两发各自只吃自己那颗常量：窗口必须逐颗切（`arm_skin_opacity_tick` 就夹在中间）。
    let arm = window(SRC, "fn arm_instructions_tick", "fn arm_skin_opacity_tick");
    need(arm, &["std::thread::sleep(INSTRUCTIONS_SAVE_DEBOUNCE)", "Msg::InstructionsTick"]);
    assert!(!arm.contains("SKIN_OPACITY_SAVE_DEBOUNCE"));
    assert!(!arm.contains("Msg::SkinOpacityTick"));
    assert!(arm.contains("spawn_background"));
    let skin = window(SRC, "fn arm_skin_opacity_tick", "fn request_instructions_save");
    need(skin, &["std::thread::sleep(SKIN_OPACITY_SAVE_DEBOUNCE)", "Msg::SkinOpacityTick"]);
    assert!(!skin.contains("INSTRUCTIONS_SAVE_DEBOUNCE"));
    assert!(!skin.contains("Msg::InstructionsTick"));
    assert!(skin.contains("spawn_background"));
    // 「至多一枚在飞」护栏：两发 armed 判据各自存在。
    assert!(SRC.contains("if self.instructions_armed {"));
    assert!(SRC.contains("if self.skin_opacity_armed {"));
}

/// 两发 `Msg::` 必须有派发臂（只声明不派发 = 死码）。
#[test]
fn the_two_tick_messages_are_declared_and_dispatched() {
    need(
        SRC,
        &[
            "InstructionsTick,",
            "SkinOpacityTick,",
            "Msg::InstructionsTick => self.flush_instructions_draft(context),",
            "Msg::SkinOpacityTick => self.flush_skin_opacity(context),",
        ],
    );
    assert_eq!(count("Msg::InstructionsTick => "), 1);
    assert_eq!(count("Msg::SkinOpacityTick => "), 1);
}

/// `shell.json` 的触发点总数：三发（材质 / 托盘 / 气泡不透明度），不多不少。
#[test]
fn shell_json_has_exactly_three_triggers() {
    assert_eq!(
        count("self.write_shell_json()"),
        3,
        "落盘出口的发数变了：要么漏了一处触发点，要么把同一处写了两遍"
    );
    assert_eq!(count("fn write_shell_json(&mut self)"), 1);
}

/// 刀6 键名扫常量的末态：四颗托盘键 + 两颗材质键走常量，`skin_opacity` **故意**留裸串一处。
///
/// 那一处不能换常量的理由写在调用点上方注释里：本文件末尾那把主干部位的源码锁把
/// 「键名串紧跟逗号」那一形当窗口**起点锚**，换掉就直接 panic（见报告 §B/§5）。
#[test]
fn the_skin_opacity_key_stays_a_bare_string_at_exactly_one_site() {
    assert_eq!(count("\"tray_notifications\","), 0);
    assert_eq!(count("\"tray_showIcon\","), 0);
    assert_eq!(count("\"tray_minimizeToTray\","), 0);
    assert_eq!(count("\"tray_closeToTray\","), 0);
    assert_eq!(count("\"shell_material\","), 0);
    assert_eq!(count("\"shell_bubbleMaterial\","), 0);
    assert_eq!(count("\"personalization_instructions\".to_string()"), 0);
    assert_eq!(
        count("\"skin_opacity\","),
        1,
        "裸串发数不是 1：要么把该换的常量漏了，要么把锚点那一处也换掉了"
    );
    let idx = SRC.find("\"skin_opacity\",").unwrap();
    let call = SRC[..idx]
        .rfind("self.settings_slider(")
        .expect("唯一的裸串不在某发 settings_slider 的实参位上");
    let between = &SRC[call + "self.settings_slider(".len()..idx];
    assert!(
        between.lines().all(|line| {
            let line = line.trim();
            line.is_empty() || line.starts_with("//")
        }),
        "裸串前面不是紧跟 `self.settings_slider(` 的首发实参（中间混进了非注释内容）\
         ⇒ L4 的窗口起点锚已经平移，锁在骗人：{between}"
    );
}

/// 未移植的两类保持零调用点（这是**设计结论**，不是遗漏）：`pet-window.json` 与
/// `shell-skin.videopause`。
///
/// ⚠ 诚实备案：这颗锁在接线前后**都是绿**（它锁的是「不做」），不属于 §3 的两态表。
#[test]
fn the_two_unported_kinds_stay_uncalled() {
    for forbidden in [
        "shellfiles::save_pet_window(",
        "shellfiles::render_pet_window_json(",
        "shellfiles::pet_move_to_default(",
        "shellfiles::save_skin_video_pause(",
        "shellfiles::load_skin_video_pause(",
        "shellfiles::parse_pet_window(",
    ] {
        assert_eq!(count(forbidden), 0, "{forbidden} 本片不该调用");
    }
    // 第五类 `layout-columns.json` 走的是 #90 那条 `layout::save_prefs`，本刀不碰。
    assert_eq!(count("shellfiles::save_layout_columns("), 0);
    assert!(SRC.contains("layout::save_prefs"));
}

/// `shell-skin.we` 的**写/读两钩在 main.rs 侧确实没有宿主**（MR2 §B 的结论，非遗漏）。
///
/// 主干那两枚钩挂在 WinRT toast 注册通路上：`Register` 之后才写 stamp、`GetString` 之后才判
/// 「新鲜」（`MainWindow.Wallpaper.cs:374-394` 那一族）。分叉今天没有 `ShellToast` 原语 ⇒ 半接
/// 比不接坏：只写 stamp 不 Register，会**伪造**主干 `:394` 的新鲜判定，让启动少清一次槽。
///
/// ⚠ 诚实备案：这颗锁接线前后**都是绿**（它锁的是「不做」），口径同
/// [`the_two_unported_kinds_stay_uncalled`]。它防的是「将来有人顺手补一半」。
/// 另记一笔已核实的差距：主干 `ClearSkinWeId()` 有 **3 个调用点**（`MainWindow.xaml.cs:8742`、
/// `:8782` 本地导入两径 + `:8804` 清除皮肤），分叉只接了 `:8804` 那一档 —— 另两径卡在
/// 「导入图片」那颗 `SkinImportButton` 至今无 `on_click`（#140 死控件），**不在本片**。
#[test]
fn the_skin_we_write_and_read_hooks_have_no_host_yet() {
    for hostless in [
        "shellfiles::load_skin_we_id(",
        "shellfiles::load_skin_we_id_from(",
        "shellfiles::save_skin_we_id(",
        "shellfiles::save_skin_we_id_to(",
        "shellfiles::render_skin_we_id(",
        "shellfiles::parse_skin_we_id(",
    ] {
        assert_eq!(count(hostless), 0, "{hostless} 今天没有宿主，接上要先证明 toast 原语存在");
    }
    // 唯一那一发清除钩（顺序判据在 main.rs 自带 mod 里，这里只钉「不长出第二份真相」）。
    assert_eq!(
        count("shellfiles::clear_skin_we_id();"),
        1,
        "清除臂发数不是 1：要么开了第二处清标记，要么把 MR2 那一发改没了"
    );
}
