//! 台账 #140（六颗死钮）+ #136 并案（MW2 第一梯队）的**离线源码锁**。
//!
//! 口径与 `tests/shellfiles_wiring.rs` 同一套：本轮禁止一切前台行为（不起 exe、不截图、
//! 不 UIA、不真调文件选择器），所以「这颗钮真的接了动作」只能拿 `src/main.rs` 的源码文本
//! 本身取证。本文件**不在** `include_str!("main.rs")` 的射程内 ⇒ 所有 needle 整串写死，
//! 计数只吃产品码一层（两层自匹配坑在这里天然免疫）。
//!
//! 判据两态（§3 表）：接线前 = `rust/tmp/mw2-main.rs.bak`（刀1 之前整文件备份）跑本文件，
//! 所有「接线形状」正锁必须真红（附失败原文）；接线后全绿。负锁（「死钮仍死」护栏）两态皆绿，
//! 表里单独标注。

const SRC: &str = include_str!("../src/main.rs");

/// 窗口助手（口径同 `shellfiles_wiring.rs`）：锚点找不到就 panic，绝不静默将下一颗函数
/// 卷进断言 —— 「窗口内不含 X」的负锁被平移洗绿是本文件树的第一大坑。
fn window<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let (_, rest) = source
        .split_once(start)
        .unwrap_or_else(|| panic!("源码里找不到锚点 {start}"));
    rest.split_once(end)
        .unwrap_or_else(|| panic!("锚点 {start} 之后找不到 {end}：窗口会一路扩到文件末尾"))
        .0
}

fn at(haystack: &str, needle: &str) -> usize {
    haystack
        .find(needle)
        .unwrap_or_else(|| panic!("窗口里找不到探针串 {needle}"))
}

// ============ SkinClearButton（#140 第一梯队） ============

#[test]
fn the_skin_clear_button_is_live_and_import_stays_the_dead_twin() {
    let card = window(SRC, "fn settings_personalization(", "fn settings_about(");
    assert!(
        card.contains("self.settings_action_button("),
        "清除皮肤钮没走活姊妹助手 ⇒ 还是画出来的死控件"
    );
    assert_eq!(
        card.matches("self.settings_action_button(").count(),
        1,
        "个性化页只该有一颗接了动作的钮（清除皮肤）"
    );
    assert_eq!(
        card.matches("self.settings_button(").count(),
        1,
        "导入图片钮本轮**故意**仍走死助手（选择器/槽位宿主未齐，见报告 §5）；颗数变了就是漂移"
    );
    for needle in [
        "Msg::SkinClear",
        "\"SkinClearButton\"",
        "\"清除背景皮肤\"",
        "\"导入背景皮肤\"",
    ] {
        assert!(card.contains(needle), "个性化窗缺串 {needle}");
    }
}

#[test]
fn the_clear_reducer_arm_clears_memory_and_disk_in_one_move() {
    assert_eq!(
        SRC.matches("Msg::SkinClear => {").count(),
        1,
        "清除臂必须恰好一臂（两臂 = 两份真相）"
    );
    let arm = window(SRC, "Msg::SkinClear => {", "Msg::PetInstall => {");
    assert!(arm.contains("self.skin_path = None;"), "内存格没清 ⇒ 钮按了画面不变");
    assert!(
        arm.contains("mw2_clear_skin_files();"),
        "盘上两枚槽没清 ⇒ 重启皮肤复活，与主干 ClearSkin 不同判据"
    );
}

#[test]
fn the_skin_backdrop_is_a_real_layer_under_both_pages() {
    let body = window(SRC, "fn body(", "fn column_splitter(");
    assert!(
        body.contains("match self.mw2_skin_backdrop() {"),
        "内容区底下的皮肤层不入了 ⇒ 清除钮永远看不见效果"
    );
    assert!(
        body.contains("Grid::new().children((backdrop, pages))"),
        "皮肤层必须压在两页**底下**（主干 ContentSurface.Background 的等值宿主）"
    );
    let helper = window(SRC, "fn mw2_skin_backdrop(", "fn mw2_pet_status_text(");
    for needle in [
        "self.skin_path.as_ref()?",
        ".source_file(",
        "Stretch::UniformToFill",
        "(percent / 100.0).clamp(0.0, 1.0)",
        "SKIN_OPACITY_KEY",
    ] {
        assert!(helper.contains(needle), "皮肤层助手缺形状 {needle}");
    }
}

// ============ PetInstallButton（#140 第一梯队） ============

#[test]
fn the_pet_install_button_is_live_over_a_status_line() {
    let card = window(SRC, "fn settings_pet(", "fn settings_skills(");
    assert_eq!(
        card.matches("self.settings_action_button(").count(),
        1,
        "宠物页只该接「安装」这一颗（浏览钮的 zip 解压器未备，故意仍死）"
    );
    assert_eq!(
        card.matches("self.settings_button(").count(),
        1,
        "浏览并安装钮仍走死助手 ⇒ 颗数变了就是漂移"
    );
    for needle in [
        "Msg::PetInstall",
        "\"PetInstallButton\"",
        "\"按命令行安装宠物\"",
        "self.mw2_pet_status_text(p)",
    ] {
        assert!(card.contains(needle), "宠物窗缺串 {needle}");
    }
    // 状态行必须挂在命令行**正下方**（主干 `_petInstallStatus` 的宿主位置）。
    assert!(
        at(card, "command_line, self.mw2_pet_status_text(p)") > at(card, "let command_line"),
        "状态行跑到了命令行上方/外侧 ⇒ 与主干 Pet.cs 的常驻位置不同形"
    );
}

#[test]
fn the_pet_reducer_arm_reads_the_field_and_folds_the_table() {
    assert_eq!(
        SRC.matches("Msg::PetInstall => {").count(),
        1,
        "安装臂必须恰好一臂"
    );
    let arm = window(SRC, "Msg::PetInstall => {", "Msg::OpenSettingsDocument =>");
    for needle in [
        "self.field(\"pet_command\", \"\")",
        "mw2_pet_install_status(&text)",
        "self.pet_status = Some((self.catalog.lf(key, &args), warn));",
    ] {
        assert!(arm.contains(needle), "安装臂缺形状 {needle}");
    }
    assert!(
        !arm.contains("Command") && !arm.contains("install_from"),
        "安装臂里出现执行/落盘形态 = 越过了「只解析不执行」的红线"
    );
}

#[test]
fn the_pet_status_host_has_no_invented_uia_id() {
    let helper = window(SRC, "fn mw2_pet_status_text(", "fn mw2_open_settings_document(");
    assert!(
        !helper.contains("automation_id"),
        "主干那行 `_petInstallStatus` 的 TextBlock 没有 x:Name ⇒ 分叉不许自造 id"
    );
    for needle in ["p.error", "p.text_secondary", "self.pet_status.as_ref()"] {
        assert!(helper.contains(needle), "状态行助手缺档位 {needle}");
    }
}

// ============ 活助手本体（on_click 在 content 之前的链序锁） ============

#[test]
fn the_live_twin_clicks_before_erasing_and_carries_no_resource_overrides() {
    let helper = window(SRC, "fn settings_action_button(", "fn settings_mount_card(");
    let click = at(helper, ".on_click(context.message(message))");
    // 探针带行尾换行：助手头顶的链序注释里有一句反引号包裹的 `.content()`，不带换行就吃到注释
    //（注释级假红/假绿是本仓源码锁的第一 landmine，`file` 实测本文件为 LF）。
    let content = at(helper, ".content(\n");
    assert!(
        click < content,
        "on_click 必须在 content() 之前：content 收尾即擦成 View（E0599 实证的链序锁）"
    );
    assert!(
        !helper.contains("resource_overrides"),
        "style + overrides 同挂 = 0xC000027B 先例，这颗不许挂回去"
    );
}

// ============ #136 OpenSettingsDocumentButton（母本 §7 并案） ============

#[test]
fn the_document_button_no_longer_posts_a_note_stub() {
    let card = window(SRC, "fn settings_general(", "fn font_stepper(");
    assert!(
        card.contains("Msg::OpenSettingsDocument"),
        "维护卡的文档钮又退回死桩了"
    );
    assert_eq!(
        SRC.matches("分叉未接入 settings/openSettingsDocument").count(),
        0,
        "假活 Msg::Note 桩串必须整文件归零（母本抓的就是这种「装活」）"
    );
    assert_eq!(
        SRC.matches("分叉未接入 settings/replace").count(),
        1,
        "恢复默认钮本轮**故意**保留假桩（确认层 + SectionNamespaces 未接，报告 §5 备案）"
    );
}

#[test]
fn the_document_rpc_is_the_null_gated_spawn_with_an_empty_object() {
    let arm = window(SRC, "Msg::OpenSettingsDocument =>", "Msg::SettingsDocument(Err");
    assert!(
        arm.contains("self.mw2_open_settings_document(context),"),
        "臂没把请求委托出去 = 又变成同步假回执"
    );
    let call = window(
        SRC,
        "fn mw2_open_settings_document(&self",
        "const MW2_SKIN_IMG_FILE",
    );
    for needle in [
        "let Some(shared) = self.kernel.clone() else",
        "spawn_background",
        "settings/openSettingsDocument",
        "mw2_open_settings_document_args()",
        "Msg::SettingsDocument(outcome)",
    ] {
        assert!(call.contains(needle), "文档 RPC 缺形状 {needle}");
    }
    let args = window(
        SRC,
        "fn mw2_open_settings_document_args",
        "enum Mw2PetKind",
    );
    assert!(
        args.contains("json!({})"),
        "主干是裸的 new 空对象 —— 参数必须恰好一个空对象，strict codec 红线"
    );
    assert!(
        SRC.contains("Msg::SettingsDocument(Err(error)) =>"),
        "失败回执没有落点 = 静默吞失败"
    );
}

// ============ 仍死三颗的「如实」护栏 + 检查更新那颗的接线现状（UK2 S2 已翻） ============

#[test]
fn the_four_shelved_buttons_are_still_honestly_dead() {
    // UK2 S2（#160 第二刀）：关于页「检查更新」那颗**已经接活**了 —— 本用例原先两条断言的
    // 理由「HTTPS 通路未批」被 UD1（WinHTTP 裸 FFI 零依赖路线）+ UC-K1（`updatecheck` 纯层）
    // + S0（`format_bytes`）三刀推翻。原判据钉的是源码事实，接线之后按事实翻成下面这形：
    // 正锁「走活姊妹助手 + 投 Msg::AboutCheckUpdate」，负锁「不许再留一份未接动作的双胞胎构造」。
    let about = window(SRC, "fn settings_about(", "fn settings_pet(");
    assert!(
        about.contains("\"AboutCheckUpdateButton\"")
            && about.contains("self.settings_action_button(")
            && about.contains("Msg::AboutCheckUpdate,"),
        "检查更新钮没画 = 母本数到的六颗里少了一颗；画了却没投消息 = 又退回死控件"
    );
    assert!(
        !about.contains("self.settings_button(\"检查更新\""),
        "这颗已接活的钮不许同时留一份未接动作的双胞胎构造（两份真相 = 哪颗生效说不清）"
    );
    let pet = window(SRC, "fn settings_pet(", "fn settings_skills(");
    assert!(
        pet.contains("\"PetBrowseButton\"") && !pet.contains("Msg::PetBrowse"),
        "PetBrowseButton 本轮必须仍死（zip 解压器离线不可用，报告 §5）"
    );
    let skills = window(SRC, "fn settings_skills(", "fn settings_agent_presets(");
    assert!(
        skills.contains("\"SkillsAddButton\"") && !skills.contains("Msg::SkillsAdd"),
        "SkillsAddButton 本轮必须仍死（技能根扫描宿主未齐，报告 §5）"
    );
    let reset = window(SRC, "fn settings_general(", "fn font_stepper(");
    assert!(
        reset.contains("\"ResetSettingsSectionButton\""),
        "恢复默认钮还在（假桩按备案保留，删了画 = 母本第二罪）"
    );
}
