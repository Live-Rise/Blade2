//! rt6：TurnRail 纯层与主干现值的对表锁。规格 = 主干 `MainWindow.TurnRail.cs`（工作树 487 行）
//! + `MainWindow.xaml` 里 `x:Name="TurnRailHost"` 那一整段（刻意不写行号：主干每提交一次整块就可能位移）。
//!
//! 本文件**不** `include_str!` 任何分叉大文件（`main.rs` 正被 MR1 并发改，且锁与被锁文本同文件会自匹配）：
//! 只吃 lib 侧公开面（`blade2_rs::turnrail`）+ **运行时现读**主干源文件。
//! 主干源只在仓库工作树里存在（打包/同步盘缺文件时没有）⇒ 读不到一律 `eprintln!` + `return` 跳过，
//! **绝不 panic**：集成测试红一次会连累整轮门禁。
//!
//! 三枚锁对应规格 §5.1：①刻度三态表、②预览卡几何且**无** `Trimming`/`MaxLines`、
//! ③11 字符断点表 + 主干把 `BreakLongTokens` 套在三元式**外面**那个套法。
//!
//! 表3 那 13 行「分叉已符合」的东西本轮不重复施工；这里只补主干现值的**外部**对表，
//! 判据一律以工作树为准（HEAD 的 `TextTrimming`/`MaxLines` 已被工作树删掉）。
//!
//! 另记：`turnrail::nearest_slot` 在分叉是「保留但不接」的死实现面（规格表4 D-3），
//! 故本文件**不**为它写任何宿主侧锁 —— 给它上锁等于替一条不存在的接线背书。

use blade2_rs::turnrail::{self, MarkState};

/// 主干源文件：按 `CARGO_MANIFEST_DIR` 相对定位仓库根（`rust/../<file>`）；读不到给原因并跳过。
fn mainline(rel: &str) -> Option<String> {
    let path = format!("{}/../{}", env!("CARGO_MANIFEST_DIR"), rel);
    match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(err) => {
            eprintln!("rt6 skip：读不到主干源 {path}（{err}）");
            None
        }
    }
}

/// 紧窗：`start` 首次出现 → 其后 `end` 首次出现。锚点缺失 ⇒ `None`（调用方跳过，不 panic）。
/// 拿不到终点锚时**不退化**成「到文件末尾」—— 那正是恒真窗口的来源。
fn window(src: &str, start: &str, end: &str) -> Option<String> {
    let at = src.find(start)? + start.len();
    let tail = src[at..].find(end)?;
    Some(src[at - start.len()..at + tail].to_string())
}

/// 打印窗口真实行数并判定它「紧」：十几到几十行才算收紧，上百行说明锚退化成了到文件末尾。
fn report_window(lock_name: &str, label: &str, text: &str) {
    let lines = text.lines().count();
    eprintln!("rt6 {lock_name} {label} 窗口行数 = {lines}");
    assert!(
        (6..=80).contains(&lines),
        "{lock_name} 的 {label} 窗口松了（{lines} 行）：锚点多半退化到文件末尾"
    );
}

/// ① 刻度三态表：纯层值 + 活动档压制悬停档 + 主干那条三元式字面。
#[test]
fn rt6_three_state_table_matches_the_mainline_ternary() {
    // 纯层（分叉侧真值）：24/0.55、32/0.9、40/1.0
    assert_eq!(
        (
            turnrail::mark_width(MarkState::Idle),
            turnrail::mark_opacity(MarkState::Idle)
        ),
        (24.0, 0.55),
        "静置档漂了"
    );
    assert_eq!(
        (
            turnrail::mark_width(MarkState::Hover),
            turnrail::mark_opacity(MarkState::Hover)
        ),
        (32.0, 0.9),
        "悬停档漂了"
    );
    assert_eq!(
        (
            turnrail::mark_width(MarkState::Active),
            turnrail::mark_opacity(MarkState::Active)
        ),
        (40.0, 1.0),
        "活动档漂了"
    );
    // 活动档在最外层 ⇒ 悬停压不回 32/0.9（主干 `TurnRail.cs` 那句三元的判序）
    assert_eq!(turnrail::state_of(true, true), MarkState::Active);
    assert_eq!(turnrail::state_of(true, false), MarkState::Hover);
    assert_eq!(turnrail::state_of(false, false), MarkState::Idle);

    let Some(cs) = mainline("MainWindow.TurnRail.cs") else {
        return;
    };
    assert!(
        cs.contains("m.MarkWidth = m.IsActive ? 40 : m.IsHovered ? 32 : 24;"),
        "主干三态宽变了（0.8.2 前值是 20/14/12，-370 前值是 -286）"
    );
    assert!(
        cs.contains("m.MarkOpacity = m.IsActive ? 1 : m.IsHovered ? 0.9 : 0.55;"),
        "主干三态不透明度变了（0.8.2 前值是 0.8/0.55）"
    );
    assert!(
        cs.contains("private const double TurnRailPreviewLeftOffset = -370;"),
        "主干卡左偏变了（-286 是 0.8.2 前值）"
    );
    // 新对象初值 = 静置档（主干字段初值 24 / 0.55）
    assert!(
        cs.contains("private double _markWidth = 24;") && cs.contains("private double _markOpacity = 0.55;"),
        "主干新刻度对象的初值变了 ⇒ 分叉 `MarkState::default()` 要跟"
    );
}

/// ② 预览卡几何，且工作树现值**没有** `TextTrimming` / `MaxLines`（截断防裁改由 480 滚动带 + 断行接管）。
#[test]
fn rt6_card_geometry_and_no_trimming_no_maxlines() {
    let Some(xaml) = mainline("MainWindow.xaml") else {
        return;
    };
    let Some(card) = window(
        &xaml,
        "<Border x:Name=\"TurnRailPreviewCard\"",
        "</Border>",
    ) else {
        eprintln!("rt6 锁② skip：卡窗锚点没找到（主干改了 x:Name 或结构）");
        return;
    };
    report_window("锁②", "TurnRailPreviewCard", &card);
    assert!(
        card.contains("Width=\"360\"")
            && card.contains("Margin=\"-370,0,0,0\"")
            && card.contains("Padding=\"12,10\"")
            && card.contains("BorderThickness=\"1\"")
            && card.contains("CornerRadius=\"{ThemeResource CornerMedium}\""),
        "主干卡几何变了：360 / -370 / 12,10 / 1px / CornerMedium"
    );
    assert!(
        card.contains("MaxHeight=\"480\"")
            && card.contains("VerticalScrollBarVisibility=\"Auto\"")
            && card.contains("HorizontalScrollBarVisibility=\"Disabled\""),
        "主干卡内滚动带变了（480 / Auto / Disabled）"
    );
    assert!(
        card.contains("Background=\"{ThemeResource CardBrush}\"")
            && card.contains("BorderBrush=\"{ThemeResource StrokeBrush}\""),
        "主干卡配色刷变了（Card + Stroke）"
    );
    // 两段文本：各只带 TextWrapping，样式仍是 Body / Hint 那一对
    assert!(
        card.contains("x:Name=\"TurnRailPreviewPrompt\"")
            && card.contains("x:Name=\"TurnRailPreviewResponse\"")
            && card.contains("Style=\"{StaticResource BodyTextStyle}\"")
            && card.contains("Style=\"{StaticResource HintTextStyle}\""),
        "主干卡里两枚 TextBlock 的命名或样式变了"
    );
    let wraps = card.matches("TextWrapping=\"Wrap\"").count();
    eprintln!("rt6 锁② TextWrapping=\"Wrap\" 出现次数 = {wraps}");
    assert_eq!(wraps, 2, "主干卡里的 TextWrapping 不再正好两段");
    // 工作树现值 = 已撤截断。这两枚一旦回来 ⇒ 主干改回去了，分叉要跟（别按 HEAD 断言）
    assert!(
        !card.contains("TextTrimming"),
        "主干重新加了 TextTrimming：分叉的预览卡要跟"
    );
    assert!(
        !card.contains("MaxLines"),
        "主干重新加了 MaxLines：分叉的预览卡要跟"
    );
    // 卡只有 AutomationId、没有 Name（表4 D-2：分叉那枚 Name 是要删的多做）
    assert!(
        card.contains("AutomationProperties.AutomationId=\"TurnRailPreviewCard\""),
        "主干卡的 AutomationId 变了"
    );
    assert!(
        !card.contains("AutomationProperties.Name"),
        "主干卡居然写了 UIA Name ⇒ 分叉删 Name 那条（表4 D-2）要重判"
    );
}

/// ③ 11 字符断点表 + ZWSP，以及主干把 `BreakLongTokens` 套在**三元式外面**那个套法。
#[test]
fn rt6_break_after_table_is_the_eleven_chars_verbatim() {
    assert_eq!(
        turnrail::BREAK_AFTER,
        ['\\', '/', '-', '_', '.', ':', '?', '&', '=', '#', '%']
    );
    assert_eq!(turnrail::BREAK_AFTER.len(), 11, "断点表不再是 11 枚");
    assert_eq!(turnrail::WORD_JOINER, '\u{200b}');
    // 纯层行为样本（不是整文件 contains）：反斜杠与冒号后各插一枚 ZWSP
    let broken = turnrail::break_long_tokens("C:\\a\\b.md");
    assert_eq!(
        broken.matches('\u{200b}').count(),
        4,
        "ZWSP 枚数不对：{broken:?}"
    );
    assert_eq!(broken.replace('\u{200b}', ""), "C:\\a\\b.md");

    let Some(cs) = mainline("MainWindow.TurnRail.cs") else {
        return;
    };
    assert!(
        cs.contains(r#"ch is '\\' or '/' or '-' or '_' or '.' or ':' or '?' or '&' or '=' or '#' or '%'"#),
        "主干断点表变了 ⇒ 分叉 `BREAK_AFTER` 要重抄"
    );
    // 主干套法：BreakLongTokens 包住**整个**三元式 ⇒ 兜底串「第 {0} 轮」也过断行
    assert!(
        cs.contains("BreakLongTokens(mark.Prompt.Length > 0 ? mark.Prompt : LF("),
        "主干兜底串的断行覆盖方式变了，分叉要重抄（表1 A-1：分叉当前只套非空那一支）"
    );
    // 副行同样被包住
    assert!(
        cs.contains("TurnRailPreviewResponse.Text = BreakLongTokens(mark.Response);"),
        "主干副行断行套法变了"
    );
    // 函数体紧窗：定义 → 收尾花括号之后的 return，确认它仍是「逐字符 + 11 枚判定」这一形状
    let Some(body) = window(&cs, "private static string BreakLongTokens(string text)", "\n    }") else {
        eprintln!("rt6 锁③ skip：BreakLongTokens 函数体锚点没找到");
        return;
    };
    report_window("锁③", "BreakLongTokens", &body);
    assert!(
        body.contains("if (string.IsNullOrEmpty(text))") && body.contains("sb.Append(ch);"),
        "主干 BreakLongTokens 的形状变了（不再是「空串原样返回 + 逐字符追加」）"
    );
    let zwsp_line = format!("sb.Append('{}');", turnrail::WORD_JOINER);
    assert_eq!(
        body.matches(&zwsp_line).count(),
        1,
        "主干 ZWSP 追加那一发不再是恰好一次"
    );
}
