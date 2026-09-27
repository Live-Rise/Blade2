//! SF1 轮的**独立门禁目标**：从 crate 外面锁 `blade2_rs::shellfiles` 补上的两类壳本地文件
//! —— `shell-skin.we` 与 `shell-toast.sender`。
//!
//! 为什么另开一个文件而不是只往 `src/shellfiles.rs` 里加测：单测跟被测码同一次编译，
//! 码写歪了断言也会跟着被写歪（本仓撞过好几次「注释级假绿」）。这个文件只走 **public API**，
//! 期望字节全部按 `od -c` 抄成真产物面，不看实现。
//!
//! 两条取证尺都在 `sf1-report.md` §2/§3：
//! - **源码**：主干 `MainWindow.Wallpaper.cs:368`；`MainWindow.ShellIntegration.cs:339`、`:416`、`:394`。
//! - **真机**：`%LOCALAPPDATA%\Blade2\shell-skin.we`（17 B）与 `shell-toast.sender`（32 B），
//!   2026-09-26 由 `od -c` 直接抄下。
//!
//! 纪律：本文件**不** `include_str!` 任何 `main.rs` —— 那是接线层的锁，`main.rs` 正被并发改写，
//! 据它「修」就是帮倒忙。这里只 `include_str!` `shellfiles.rs` 一次（锁模块头的「七类」口径），
//! 那不是我自己的文件 ⇒ 不存在自匹配；needle 仍按本仓口径用 `concat!` 拆开写。

use std::path::Path;

use blade2_rs::shellfiles::{
    clear_skin_we_id_from, data_home, data_home_from, load_skin_we_id_from, notification_identity,
    parse_skin_we_id, render_sender_stamp, render_skin_we_id, save_sender_stamp_to,
    save_skin_we_id_to, sender_stamp_is_fresh_at, sender_stamp_matches, skin_we_path,
    toast_sender_path, SkinWeClear, SKIN_WE_FILE, TOAST_IDENTITY_EXE_PREFIX,
    TOAST_IDENTITY_PKG_PREFIX, TOAST_SENDER_FILE, TOAST_SENDER_NAME, TOAST_SENDER_SEPARATOR,
};

/// `%LOCALAPPDATA%\Blade2\shell-skin.we` 实测 **17 B**：
/// `76 69 64 65 6f 5f 5f 33 33 37 33 36 39 39 34 38 32`。
const SF1_REAL_WE: &str = "video__3373699482";

/// `%LOCALAPPDATA%\Blade2\shell-toast.sender` 实测 **32 B**：
/// `pkg:Blade2_wr2brkarxqkxy`(24) + `0a`(1) + `42 6c 61 64 65 c2 b2`(7)。
/// Rust 的 `"\n"` 就是 `0a` —— 这里**不许**换成平台行尾。
const SF1_REAL_SENDER: &str = "pkg:Blade2_wr2brkarxqkxy\nBlade\u{b2}";

/// 真机上那串通知身份 = `pkg:` + 包系列名（24 B）。
const SF1_REAL_IDENTITY: &str = "pkg:Blade2_wr2brkarxqkxy";

fn sandbox(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("sf1gate-{}-{:?}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

// ============================== 形状 ==============================

/// 两类的文件名是主干的字面量，拼错一个字就读不到用户盘上已有的文件。
#[test]
fn sf1_the_two_new_file_names_are_the_mainline_literals() {
    assert_eq!(SKIN_WE_FILE, "shell-skin.we");
    assert_eq!(TOAST_SENDER_FILE, "shell-toast.sender");
    // 与既有五类不撞名（「漏项」的根因就是没逐颗对过落点）
    let all = [
        "shell.json",
        "shell-skin.opacity",
        "shell-skin.videopause",
        "AGENTS.md",
        "pet-window.json",
        SKIN_WE_FILE,
        TOAST_SENDER_FILE,
    ];
    assert_eq!(all.len(), 7);
    for (i, a) in all.iter().enumerate() {
        for b in all.iter().skip(i + 1) {
            assert_ne!(a, b);
        }
    }
}

/// `shell-skin.we` = **恒等**裸文本：17 B 逐字节、无尾换行、不 Trim、不转义。
#[test]
fn sf1_skin_we_renders_the_measured_17_bytes_verbatim() {
    assert_eq!(SF1_REAL_WE.len(), 17, "真产物基线自己先钉住");
    let rendered = render_skin_we_id(SF1_REAL_WE);
    assert_eq!(rendered.as_bytes(), SF1_REAL_WE.as_bytes());
    assert_eq!(*rendered.as_bytes().last().unwrap(), b'2', "末字节是 id 的最后一位，不是换行");
    assert!(!rendered.contains('\n') && !rendered.contains('\r'));
    // 恒等 ⇒ 连「脏」输入也不许被顺手整理
    for dirty in ["  x  ", "x\n", "x\r\n", "a\\b", ""] {
        assert_eq!(render_skin_we_id(dirty).as_bytes(), dirty.as_bytes());
    }
}

/// `shell-toast.sender` = `身份 + 单 LF + 发送者名`，32 B 逐字节。
/// 行内那个 `0a` **不是** `0d 0a`；末两字节 `c2 b2` 是 U+00B2，不是 ASCII `2`。
#[test]
fn sf1_sender_stamp_renders_the_measured_32_bytes() {
    assert_eq!(SF1_REAL_SENDER.len(), 32, "真产物基线自己先钉住");
    assert_eq!(SF1_REAL_IDENTITY.len(), 24);
    let rendered = render_sender_stamp(SF1_REAL_IDENTITY, TOAST_SENDER_NAME);
    assert_eq!(rendered.as_bytes(), SF1_REAL_SENDER.as_bytes(), "渲染器 = 真机产物");
    assert_eq!(rendered.as_bytes()[24], 0x0a, "第 25 字节 = 行内 LF");
    assert_eq!(rendered.matches('\n').count(), 1);
    assert_eq!(rendered.matches('\r').count(), 0, "一个 0d 都不许有");
    assert!(!rendered.starts_with('\u{feff}'), "UTF-8 无 BOM");
    assert_eq!(*rendered.as_bytes().last().unwrap(), 0xb2, "无尾换行");
    assert_eq!(TOAST_SENDER_NAME.as_bytes(), b"Blade\xc2\xb2");
    assert_eq!(TOAST_SENDER_NAME.len(), 7, "U+00B2 在 UTF-8 里占 2 字节");
    assert_eq!(TOAST_SENDER_SEPARATOR, '\n');
    assert_eq!(TOAST_SENDER_SEPARATOR.len_utf8(), 1);
    assert_eq!(TOAST_IDENTITY_PKG_PREFIX, "pkg:");
    assert_eq!(TOAST_IDENTITY_EXE_PREFIX, "exe:");
}

/// 身份两臂：打包 `pkg:`+系列名、无包 `exe:`+进程路径（主干靠异常分流，分叉摊成 Option）。
#[test]
fn sf1_notification_identity_two_arms() {
    assert_eq!(notification_identity(Some("Blade2_wr2brkarxqkxy"), "ignored"), SF1_REAL_IDENTITY);
    assert_eq!(notification_identity(None, "C:/x/blade2.exe"), "exe:C:/x/blade2.exe");
    // 刻意不判空：主干那边 FamilyName 真是空串也照样落成 `pkg:`
    assert_eq!(notification_identity(Some(""), "x"), "pkg:");
}

// ============================== 读端脆性 ==============================

/// 主干 `MainWindow.ShellIntegration.cs:394` 是
/// `File.ReadAllText(path) == SenderStamp`（全等、不 Trim）。
/// 四种「看着更合理」的写法必须**全部**判过期，否则接线侧会以为没事 —— 代价是每次启动
/// 白跑一次 `UnregisterAll()`，toast 头部应用名反复被清，且没有任何用户可见报错。
#[test]
fn sf1_sender_stamp_rejects_the_plausible_wrong_shapes() {
    let fresh = |text: &str| sender_stamp_matches(text, SF1_REAL_IDENTITY, TOAST_SENDER_NAME);
    assert!(fresh(SF1_REAL_SENDER));
    assert!(!fresh(&SF1_REAL_SENDER.replace('\n', "\r\n")), "CRLF 版（Windows 上最自然的错法）");
    assert!(!fresh(&format!("{}\n", SF1_REAL_SENDER)), "尾 LF 版");
    assert!(!fresh(&format!("{}\r\n", SF1_REAL_SENDER)), "尾 CRLF 版");
    assert!(!fresh(&SF1_REAL_SENDER.replace("Blade\u{b2}", "Blade2")), "把上标二写成 ASCII 2");
    assert!(!fresh(&SF1_REAL_SENDER.replace('\n', " ")), "拿空格替 LF");
    assert!(!fresh(&format!(" {} ", SF1_REAL_SENDER)), "读端不 Trim");
    // 另一臂：换个身份也应当不等（把身份写进标记的理由，主干 `:337-338`）
    assert!(!fresh("pkg:OtherFamilyName\nBlade\u{b2}"));
}

/// 反直觉的一臂：开头带 UTF-8 BOM **仍然相等**。这不是宽容，是复刻 .NET
/// `File.ReadAllText` 默认的 BOM 嗅探。分叉若拿裸 `read_to_string` 直接比，
/// 会在这一格比主干多跑一次 `UnregisterAll`。
#[test]
fn sf1_sender_stamp_replicates_the_dotnet_bom_sniffing() {
    let with_bom = format!("\u{feff}{SF1_REAL_SENDER}");
    assert!(with_bom.as_bytes().starts_with(&[0xef, 0xbb, 0xbf]));
    assert!(sender_stamp_matches(&with_bom, SF1_REAL_IDENTITY, TOAST_SENDER_NAME));
    // BOM 只许在开头：夹在中间就是不等
    assert!(!sender_stamp_matches(
        &SF1_REAL_SENDER.replace("Blade", "B\u{feff}lade"),
        SF1_REAL_IDENTITY,
        TOAST_SENDER_NAME
    ));
    // 渲染侧永不产 BOM
    assert!(!render_sender_stamp(SF1_REAL_IDENTITY, TOAST_SENDER_NAME).contains('\u{feff}'));
}

/// `shell-skin.we` 读端的三条空态臂（缺文件 / 0 字节 / 纯空白）都通向 `None`。
#[test]
fn sf1_skin_we_three_empty_arms() {
    assert_eq!(parse_skin_we_id(""), None);
    assert_eq!(parse_skin_we_id("     "), None);
    assert_eq!(parse_skin_we_id("\r\n\t "), None);
    assert_eq!(parse_skin_we_id(SF1_REAL_WE), Some(SF1_REAL_WE));
    assert_eq!(parse_skin_we_id("  video__1  "), Some("video__1"), "首尾空白被主干的 Trim 抹掉");
    assert_eq!(parse_skin_we_id("VIDEO__1"), Some("VIDEO__1"), "大小写不折叠：比较侧是 Ordinal");
    let dir = sandbox("we-empty-arm");
    assert_eq!(load_skin_we_id_from(&dir.join(SKIN_WE_FILE)), None, "缺文件是默认态");
    let _ = std::fs::remove_dir_all(&dir);
}

// ============================== 落盘 ==============================

/// 写盘 ⇒ 字节 == 真产物面；且**清空 = 删文件**，不是留一个 0 字节文件。
/// 后者是这一类跟 `AGENTS.md` 同型的语义：0 字节的 `.we` 在主干读回来也是「没有 WE 背景」，
/// 但盘上会永远留一颗空文件。
#[test]
fn sf1_skin_we_disk_bytes_and_clear_deletes() {
    let dir = sandbox("we-disk");
    let path = data_home_from(Some(&dir)).join(SKIN_WE_FILE);
    assert_eq!(clear_skin_we_id_from(&path).unwrap(), SkinWeClear::NoChange);
    save_skin_we_id_to(SF1_REAL_WE, &path).expect("落盘");
    assert_eq!(std::fs::read(&path).unwrap(), SF1_REAL_WE.as_bytes());
    assert_eq!(load_skin_we_id_from(&path).as_deref(), Some(SF1_REAL_WE));
    assert_eq!(clear_skin_we_id_from(&path).unwrap(), SkinWeClear::Deleted);
    assert!(!path.exists(), "清空必须是删，不是写空串");
    assert_eq!(clear_skin_we_id_from(&path).unwrap(), SkinWeClear::NoChange);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `shell-toast.sender` 的写盘字节 == 真产物 32 B；读端三臂：缺文件 / 盘上脏 / 盘上正确。
#[test]
fn sf1_sender_disk_bytes_and_missing_file_counts_as_stale() {
    let dir = sandbox("sender-disk");
    let path = data_home_from(Some(&dir)).join(TOAST_SENDER_FILE);
    assert!(!sender_stamp_is_fresh_at(&path, SF1_REAL_IDENTITY, TOAST_SENDER_NAME));
    save_sender_stamp_to(SF1_REAL_IDENTITY, TOAST_SENDER_NAME, &path).expect("落盘");
    assert_eq!(std::fs::read(&path).unwrap(), SF1_REAL_SENDER.as_bytes());
    assert_eq!(std::fs::read(&path).unwrap().len(), 32);
    assert!(sender_stamp_is_fresh_at(&path, SF1_REAL_IDENTITY, TOAST_SENDER_NAME));
    std::fs::write(&path, SF1_REAL_SENDER.replace('\n', "\r\n")).expect("写脏");
    assert!(!sender_stamp_is_fresh_at(&path, SF1_REAL_IDENTITY, TOAST_SENDER_NAME));
    let _ = std::fs::remove_dir_all(&dir);
}

/// 两颗新文件与既有五类落在**同一个**数据家下（同一条 `LOCALAPPDATA → …` 链）。
#[test]
fn sf1_new_paths_join_the_same_data_home_as_the_other_five() {
    // 真机链：两颗新文件的父目录必须与其它五颗完全同一颗（纯路径算术，不碰盘）
    let home = data_home();
    assert_eq!(skin_we_path().parent(), Some(home.as_path()));
    assert_eq!(toast_sender_path().parent(), Some(home.as_path()));
    // 可注入链：命名规则与兜底同 `layout::prefs_path` 那条
    let injected = data_home_from(Some(Path::new("C:/fake/LOCALAPPDATA")));
    assert_eq!(
        injected.join(SKIN_WE_FILE).to_string_lossy().replace('\\', "/"),
        "C:/fake/LOCALAPPDATA/Blade2/shell-skin.we"
    );
    assert_eq!(
        injected.join(TOAST_SENDER_FILE).to_string_lossy().replace('\\', "/"),
        "C:/fake/LOCALAPPDATA/Blade2/shell-toast.sender"
    );
    // 兜底：拿不到 LOCALAPPDATA 时退化成 ./Blade2 下，两颗仍不撞
    let fallback = data_home_from(None);
    assert_ne!(fallback.join(SKIN_WE_FILE), fallback.join(TOAST_SENDER_FILE));
}

// ============================== 模块头口径锁 ==============================

/// 本模块头注原来是「四类 / 覆盖的四类」，那本身就是**错误事实源**。锁住它已经改口成七类，
/// 并且两类新文件的名字与主干出处都进了表、表体正好七行。
#[test]
fn sf1_module_header_says_seven_classes_not_five() {
    let source = include_str!("../src/shellfiles.rs");
    let header = &source[..source.find("use std::path").expect("找不到模块头边界")];

    assert!(header.contains(concat!("七类", "壳本地文件")), "头注第一行必须是七类");
    assert!(!header.contains("四类壳本地文件"), "旧的「四类」自述必须已改掉");
    assert!(!header.contains(concat!("覆盖的", "四类")), "旧的「覆盖的四类」自述必须已改掉");
    assert!(!header.contains(concat!("第五类 ", "layout-columns")), "不得再把列宽偏好算进本模块");
    // 两类新文件都进了表，且各带一处主干出处
    for needle in [
        concat!("shell-skin.", "we"),
        concat!("shell-toast.", "sender"),
        concat!("MainWindow.", "Wallpaper.cs"),
        concat!("MainWindow.", "ShellIntegration.cs"),
    ] {
        assert!(header.contains(needle), "表里缺：{}", needle);
    }
    // 表体行数 = 7（漏项的根因是「数过一遍但没逐颗对过落点」，那就直接把颗数钉住）
    let rows = header
        .lines()
        .filter(|line| line.starts_with("//! | `"))
        .count();
    assert_eq!(rows, 7, "壳落盘表必须是七行");
}
