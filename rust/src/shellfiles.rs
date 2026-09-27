//! #82「壳落盘」七类壳本地文件的**纯函数层**：渲染 / 解析 / 数据家路径。
//!
//! 规格来源：`rust/tmp/sd3-report.md`（只读代理逐字节取证）+ `sf1-report.md`（本模块补的两类）。
//! 本模块**只有纯函数 + 落盘助手**，不含任何 UI 接线 —— 谁在什么时机调它们，见 `sf1-report.md` §8。
//!
//! ## 壳落盘 = 七类（这个「七」是取证结果，不是估的）
//!
//! 判据：主干 `MainWindow.*.cs` 里所有 `Path.Combine(DataHome, "…")` 的字面量，逐个对到
//! `File.WriteAllText` / `File.Delete` 落点 ⇒ 七颗，全在数据家。
//! 表中后两行（`shell-skin.we` / `shell-toast.sender`）是**本轮补的漏项**：它们在 HEAD
//! 基线里就有（`git show HEAD:MainWindow.Wallpaper.cs` 命中 2 次、
//! `git show HEAD:MainWindow.ShellIntegration.cs` 命中 1 次），**不是主干漂移**，
//! 是这张表原来只数到「四类 + 第五类」，把这两类整格漏了（原 `:1` / `:6` 的自述口径）。
//! 下面每一行对应本模块同名的一节，行序 = 节序。
//!
//! | 文件 | 主干写点（本轮逐条重测） | 形状 |
//! |---|---|---|
//! | `shell.json` | `MainWindow.TraySettings.cs:160-175 SaveShellOptions` | 缩进 JSON：`\r\n` + 2 空格 + camelCase，声明序 8 键，`}` 后无换行 |
//! | `shell-skin.opacity` | `MainWindow.xaml.cs:8694-8703 SaveSkinOpacity`（写 `:8699`） | 裸文本，C# `"0.##"` 形态，无换行 |
//! | `shell-skin.videopause` | `MainWindow.xaml.cs:8635-8644 SaveSkinVideoPauseOnBlur`（写 `:8640`） | 裸文本 `"1"` / `"0"`，一字节 |
//! | `AGENTS.md` | `MainWindow.Personalization.cs:278-309 SaveInstructions`（写 `:297`） | Trim 后**单个** `\r\n` 收尾；**清空 = 删文件** |
//! | `pet-window.json` | `Dsh/PetWindow.xaml.cs:324-336 SavePosition`（写 `:329-330`） | 紧凑 PascalCase `{"X":-543,"Y":974}`，无空白 |
//! | `shell-skin.we` | `MainWindow.Wallpaper.cs:363-371 WriteSkinWeId`（写 `:368`）；读 `:350-361 ReadSkinWeId`；**清 = 删** `:374-378 ClearSkinWeId` | 裸文本**恒等**写入 WE 壁纸 id，无换行、不 Trim；路径 `:54` |
//! | `shell-toast.sender` | `MainWindow.ShellIntegration.cs:411-421 RememberSenderName`（写 `:416`）；读回比对 `:390-409 MigrateStaleSenderName`（`:394`） | `身份` + **单个 `0a`** + `发送者名`，**行内 LF 不是 CRLF**、无尾换行、UTF-8 无 BOM；路径 `:335` |
//!
//! ⚠ `shell-toast.sender` 是七类里**唯一行内带换行**的一类，也是最脆的一类：主干 `:394` 的
//! 读回是 `File.ReadAllText(path) == SenderStamp` —— **`==` 全等、不 Trim**。渲染时多写一个
//! 尾换行、或把行内分隔符写成 `0d 0a`、或把末字段写成 `Blade2`（少那 2 字节的 `c2 b2`），
//! 比对就**每次启动都不等** ⇒ 主干白跑一次 `UnregisterAll()` ⇒ toast 头部应用名反复被清，
//! 且这条路径没有任何用户可见报错。见 [`render_sender_stamp`]。
//!
//! 另两类**不在本表**、也**不归本模块**，别再当漏项补：
//! 列宽偏好 `layout-columns.json` 归 #90（`crate::layout`），本模块不重写；
//! `shell-skin.video` / `shell-skin.jpg` 是壁纸**素材文件**（真机 9 MB 级），不是「壳本地配置」。
//!
//! 两条贯穿全模块的纪律：
//! 1. **数据家 = `LOCALAPPDATA → kernel::data_home_root → kernel 那颗搬迁口`**（与
//!    [`crate::layout::prefs_path`] 同一条链、**同一个口**：两条链都先过一次 `Code2 → Blade2`
//!    改名再拼 `DATA_HOME_DIR` —— 判据是主干的开机顺序，第一发 `LoadShellOptions()`
//!    （`MainWindow.xaml.cs:2528` → `MainWindow.TraySettings.cs:62`）就同步触发过那颗 getter
//!    的改名，故列宽那条链也得搬，细节写在 `layout.rs::prefs_path` 的 doc 上。
//!    逐句对应主干 `MainWindow.xaml.cs:8553` 的
//!    `DataHome` getter：判据只有一条「新目录不在 ∧ 老目录在」，搬不动就静默用全新 `Blade2`，
//!    谁先碰谁触发。仍然**刻意不用** `kernel` 里那颗带 env 覆盖的解析口（它认一枚分叉自测
//!    专用的变量名，主干没有，走它会把这七类文件落到主干永远不会落的位置）⇒ 台账 #145 的
//!    **搬迁半在本节关掉，env 半按主代理裁定不做**（判据见本节末那颗自我源码锁的③）。
//! 2. **七类一律手写渲染器，不走 serde 序列化产出字节**。`shell.json` 那条最急：本仓的
//!    `serde_json` 没开 `preserve_order`，`Map` 底层是 `BTreeMap` ⇒ 键按字母序 ⇒ 逐字节对不上主干。
//!    但 serde 还有第二、第三个坑，对新增的裸文本两类同样致命：**浮点定长**（`0.5` 会被写成
//!    `0.50`）与**转义/换行策略**（serde 按 JSON 字符串转义，而 `.we` / `.sender` 是**裸文本**，
//!    一个反斜杠都不该多）。所以 [`render_skin_we_id`] / [`render_sender_stamp`] 同样是手写拼接。

use std::path::{Path, PathBuf};

use crate::kernel::{data_home_root, DATA_HOME_DIR};

// ============================== 文件名 ==============================

/// 主干 `MainWindow.TraySettings.cs:62`。
pub const SHELL_JSON_FILE: &str = "shell.json";
/// 主干 `MainWindow.xaml.cs:8576`。
pub const SKIN_OPACITY_FILE: &str = "shell-skin.opacity";
/// 主干 `MainWindow.xaml.cs:8590`。
pub const SKIN_VIDEO_PAUSE_FILE: &str = "shell-skin.videopause";
/// 主干 `MainWindow.Wallpaper.cs:54`（`Path.Combine(DataHome, "shell-skin.we")`）。
pub const SKIN_WE_FILE: &str = "shell-skin.we";
/// 主干 `MainWindow.ShellIntegration.cs:335`（`Path.Combine(MainWindow.DataHome, …)`）。
pub const TOAST_SENDER_FILE: &str = "shell-toast.sender";
/// 主干 `MainWindow.Personalization.cs:26`。**这是「用户自定义指令卡」，与仓库根的编码规范
/// `AGENTS.md` 无关**（SD3 §7.1-3 专门纠正过这处歧义）。
pub const AGENTS_MD_FILE: &str = "AGENTS.md";
/// 主干 `MainWindow.Pet.cs:77`（`Path.Combine(DataHome, "pet-window.json")`）。
pub const PET_WINDOW_FILE: &str = "pet-window.json";

/// 主干 `Environment.NewLine` 在 Windows 上的字节。**写死 `\r\n`**，不跟平台走：
/// 主干是 Windows 应用，`AGENTS.md` 的尾字节实测就是 `0d 0a`。
const NEWLINE: &str = "\r\n";

// ============================== 值域常量 ==============================

/// 主干 `TraySettings.cs:21-24` 的窗口材质 id（闭集，序同 `tokens::SETTINGS_MATERIALS` 四颗标签）。
pub const MATERIAL_MICA: &str = "mica";
pub const MATERIAL_MICA_ALT: &str = "mica-alt";
pub const MATERIAL_ACRYLIC: &str = "acrylic";
pub const MATERIAL_NONE: &str = "none";
/// 白名单本体：顺序即分叉下拉的下标序。
pub const WINDOW_MATERIAL_IDS: &[&str] = &[MATERIAL_MICA, MATERIAL_MICA_ALT, MATERIAL_ACRYLIC, MATERIAL_NONE];

/// 主干 `TraySettings.cs:30-32` 的气泡材质 id（序同 `main.rs` 的 `BUBBLE_MATERIALS` 三颗标签）。
pub const BUBBLE_MATERIAL_TRANSLUCENT: &str = "translucent";
pub const BUBBLE_MATERIAL_ACRYLIC: &str = "acrylic";
pub const BUBBLE_MATERIAL_FOLLOW: &str = "follow";
pub const BUBBLE_MATERIAL_IDS: &[&str] =
    &[BUBBLE_MATERIAL_TRANSLUCENT, BUBBLE_MATERIAL_ACRYLIC, BUBBLE_MATERIAL_FOLLOW];

/// 主干 `TraySettings.cs:35-37`：`bubbleOpacity` 合法域 0.2–1.0、默认 0.6。
pub const BUBBLE_OPACITY_MIN: f64 = 0.2;
pub const BUBBLE_OPACITY_MAX: f64 = 1.0;
pub const BUBBLE_OPACITY_DEFAULT: f64 = 0.6;

/// 主干 `SetBubbleOpacity:18787` 的脏检判据：**浮点容差 `< 0.0001`，不是 `==`**。
/// 漏了这一条，滑杆每次 ValueChanged 都会重写整份 `shell.json`。
const BUBBLE_OPACITY_EPSILON: f64 = 0.0001;

/// 主干 `MainWindow.xaml.cs:8595 DefaultSkinOpacity = 0.40` 与 `:8611` 的 `Math.Clamp(v, 0.10, 0.80)`。
/// ⚠ 分叉模型侧那四颗 `tokens::SKIN_OPACITY_*`（10/80/5/40）是**百分数标度**，落盘前必须 `/100`。
pub const SKIN_OPACITY_MIN: f64 = 0.10;
pub const SKIN_OPACITY_MAX: f64 = 0.80;
pub const SKIN_OPACITY_DEFAULT: f64 = 0.40;

/// 主干 `MainWindow.Personalization.cs:30 InstructionsMaxLength = 4000`（TextBox 属性）。
pub const INSTRUCTIONS_MAX_CHARS: usize = 4000;

/// 主干 `MainWindow.ShellIntegration.cs:322`：`private const string SenderName = "Blade²"`。
/// 逐字节是 `42 6c 61 64 65 c2 b2`（7 B）—— 末位是 U+00B2 上标二，**不是** `Blade2`：
/// 真机 `shell-toast.sender` 末 7 字节实测就是它，写成 `Blade2` 会叫 `:394` 全等失败。
pub const TOAST_SENDER_NAME: &str = "Blade\u{b2}";

/// 主干 `MainWindow.ShellIntegration.cs:339` 的 `+ "\n" +` —— 身份与发送者名之间的**行内分隔符**。
///
/// ⚠⚠ 这是 `0a`（**LF 单字节**），不是本模块 [`NEWLINE`] 那颗 `0d 0a`：主干 C# 源码里写的就是
/// 字面量 `"\n"`，它**不跟平台走**。全仓七类里只有这一类带行内换行，也只有它把「LF 还是 CRLF」
/// 变成行为差异 —— 因为读端 `:394` 是 `==` 全等。写成 [`NEWLINE`] 即坏，见
/// [`render_sender_stamp`] 的字节测与 `sf1-report.md` §3。
pub const TOAST_SENDER_SEPARATOR: char = '\n';

/// 主干 `MainWindow.ShellIntegration.cs:343-353 NotificationIdentity()` 的两个前缀。
/// 打包态取 `"pkg:" + Package.Current.Id.FamilyName`，无包态抛异常后取 `"exe:" + ProcessPath`。
pub const TOAST_IDENTITY_PKG_PREFIX: &str = "pkg:";
pub const TOAST_IDENTITY_EXE_PREFIX: &str = "exe:";

// ============================== 数据家路径 ==============================

/// 纯（可注入）：`LOCALAPPDATA` 的根 → 数据家。与 [`crate::layout::prefs_path`] 同一个兜底
/// （拿不到 `LOCALAPPDATA` 退化成 `./Blade2`）。
pub fn data_home_from(local_app_data: Option<&Path>) -> PathBuf {
    data_home_root(local_app_data).join(DATA_HOME_DIR)
}

/// 真机数据家 = `%LOCALAPPDATA%\Blade2`，**先照主干做一次 `Code2 → Blade2` 改名**：
/// 主干 `MainWindow.xaml.cs:8553` 那颗 `DataHome` getter 的等价物 —— 副作用照抄（谁先碰谁触发、
/// 搬不动就静默用全新 `Blade2`），env 覆盖口**不照抄**（主干没有那一枚，理由见模块头纪律 1）。
pub fn data_home() -> PathBuf {
    // 与 `crate::layout::prefs_path` 同一句拼法（var_os → PathBuf → as_deref），且两条链
    // 一起过 kernel 那颗搬迁口 ⇒ 「谁先碰谁搬家」只会发生一次、两边同结果。
    // 纯可注入的那半仍是 [`data_home_from`]（测试面不碰真盘，见模块头纪律 1）。
    let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    crate::kernel::migrate_data_home(&data_home_root(local_app_data.as_deref()))
}

/// 数据家下的一个壳本地文件（七类共用这一条拼接，禁止各处再拼）。
pub fn shell_file(name: &str) -> PathBuf {
    data_home().join(name)
}

/// `shell.json` 的真机路径（主干 `TraySettings.cs:62`）。
pub fn shell_json_path() -> PathBuf {
    shell_file(SHELL_JSON_FILE)
}

/// `shell-skin.opacity` 的真机路径（主干 `MainWindow.xaml.cs:8576`）。
pub fn skin_opacity_path() -> PathBuf {
    shell_file(SKIN_OPACITY_FILE)
}

/// `shell-skin.videopause` 的真机路径（主干 `MainWindow.xaml.cs:8590`）。
pub fn skin_video_pause_path() -> PathBuf {
    shell_file(SKIN_VIDEO_PAUSE_FILE)
}

/// `AGENTS.md` 的真机路径（主干 `MainWindow.Personalization.cs:26`）。
pub fn agents_path() -> PathBuf {
    shell_file(AGENTS_MD_FILE)
}

/// `pet-window.json` 的真机路径（主干 `MainWindow.Pet.cs:77`）。
/// ⚠ 本片**不接线**：分叉没有宠物宿主，见 §4 与 `sf1-report.md` §9。
pub fn pet_window_path() -> PathBuf {
    shell_file(PET_WINDOW_FILE)
}

/// `shell-skin.we` 的真机路径（主干 `MainWindow.Wallpaper.cs:54`）。
pub fn skin_we_path() -> PathBuf {
    shell_file(SKIN_WE_FILE)
}

/// `shell-toast.sender` 的真机路径（主干 `MainWindow.ShellIntegration.cs:335`）。
pub fn toast_sender_path() -> PathBuf {
    shell_file(TOAST_SENDER_FILE)
}

// ============================== 1. shell.json ==============================

/// 主干 `MainWindow.TraySettings.cs:39-50` 那个 `ShellOptions`。
///
/// **字段声明序 = 落盘键序**（System.Text.Json 按声明序写），所以这里的顺序是契约的一部分，
/// 别按字母排、别插入新字段打断它：
/// `showTrayIcon → minimizeToTray → closeToTray → showNotifications → material →
/// bubbleMaterial → bubbleOpacity → remindedUpdateTag`。
#[derive(Clone, Debug, PartialEq)]
pub struct ShellOptions {
    pub show_tray_icon: bool,
    pub minimize_to_tray: bool,
    pub close_to_tray: bool,
    pub show_notifications: bool,
    pub material: String,
    pub bubble_material: String,
    /// 0–1 标度（主干量纲）。分叉滑杆是百分数，换算见 [`bubble_opacity_from_percent`]。
    pub bubble_opacity: f64,
    pub reminded_update_tag: String,
}

impl Default for ShellOptions {
    /// 主干 `ShellOptions` 的字段初值（`:41-49`）：两颗托盘/通知默认开、材质 mica、
    /// 气泡半透明、0.6、tag 空。
    fn default() -> Self {
        Self {
            show_tray_icon: true,
            minimize_to_tray: false,
            close_to_tray: false,
            show_notifications: true,
            material: MATERIAL_MICA.to_string(),
            bubble_material: BUBBLE_MATERIAL_TRANSLUCENT.to_string(),
            bubble_opacity: BUBBLE_OPACITY_DEFAULT,
            reminded_update_tag: String::new(),
        }
    }
}

impl ShellOptions {
    /// 主干 `SetShowTrayIcon:324-345` 的连带语义：关掉托盘图标时，`minimizeToTray` /
    /// `closeToTray` 被一并压回 `false`（否则「关闭时最小化到托盘」指向一个不存在的托盘）。
    /// 主干在那条路径上会**连写两次整份文件**，本函数只负责算值，写几次由调用方定。
    pub fn with_tray_icon_off(&mut self) {
        self.show_tray_icon = false;
        self.minimize_to_tray = false;
        self.close_to_tray = false;
    }

    /// 主干 `SetShellMaterial:18750-18764`：未知 id 归一 `mica`；**同值早退不重写盘**。
    /// 返回归一后的 id，调用方拿它和旧值比即可。
    pub fn normalized_material(&self) -> &str {
        if WINDOW_MATERIAL_IDS.contains(&self.material.as_str()) {
            self.material.as_str()
        } else {
            MATERIAL_MICA
        }
    }

    /// 主干 `SetBubbleMaterial:18767-18780`：同形态，未知 id 归一 `translucent`。
    pub fn normalized_bubble_material(&self) -> &str {
        if BUBBLE_MATERIAL_IDS.contains(&self.bubble_material.as_str()) {
            self.bubble_material.as_str()
        } else {
            BUBBLE_MATERIAL_TRANSLUCENT
        }
    }
}

/// 分叉滑杆（百分数 20–100）→ 主干 `bubbleOpacity`（0.2–1.0）。
/// 对应主干 `SetBubbleOpacity:18786` 的 `Math.Clamp(opacity, 0.2, 1.0)`。
/// 非有限值回默认（主干那条路上 NaN 会让 `JsonSerializer.Serialize` 抛异常、整次写被
/// `catch {}` 吞掉 ⇒ 盘上什么都不留；这里归一成默认值，落盘形状更稳，差异已在报告声明）。
pub fn bubble_opacity_from_percent(percent: f64) -> f64 {
    if !percent.is_finite() {
        return BUBBLE_OPACITY_DEFAULT;
    }
    (percent / 100.0).clamp(BUBBLE_OPACITY_MIN, BUBBLE_OPACITY_MAX)
}

/// 反向换算（主干读到的 0–1 → 分叉滑杆的百分数），对应分叉 `BUBBLE_OPACITY_DEFAULT =
/// theme::BUBBLE_OPACITY_DEFAULT * 100.0` 那一层。
pub fn percent_from_bubble_opacity(opacity: f64) -> f64 {
    if !opacity.is_finite() {
        return BUBBLE_OPACITY_DEFAULT * 100.0;
    }
    opacity.clamp(BUBBLE_OPACITY_MIN, BUBBLE_OPACITY_MAX) * 100.0
}

/// 主干 `SetBubbleOpacity:18787` 的 `Math.Abs(value - _bubbleOpacity) < 0.0001`。
pub fn bubble_opacity_unchanged(current: f64, next: f64) -> bool {
    (current - next).abs() < BUBBLE_OPACITY_EPSILON
}

/// 下拉下标 ↔ 主干 id。越界回落默认档（分叉 `theme::WindowMaterial::from_index` 同语义）。
pub fn window_material_id_for_index(index: usize) -> &'static str {
    WINDOW_MATERIAL_IDS.get(index).copied().unwrap_or(MATERIAL_MICA)
}

/// 主干 id ↔ 下拉下标。白名单外回 0（= mica），与 [`ShellOptions::normalized_material`] 一致。
pub fn window_material_index_for_id(id: &str) -> usize {
    WINDOW_MATERIAL_IDS.iter().position(|m| *m == id).unwrap_or(0)
}

/// 气泡材质下标 ↔ 主干 id，越界回 0（= translucent）。
pub fn bubble_material_id_for_index(index: usize) -> &'static str {
    BUBBLE_MATERIAL_IDS.get(index).copied().unwrap_or(BUBBLE_MATERIAL_TRANSLUCENT)
}

/// 主干 id ↔ 气泡下标，白名单外回 0。
pub fn bubble_material_index_for_id(id: &str) -> usize {
    BUBBLE_MATERIAL_IDS.iter().position(|m| *m == id).unwrap_or(0)
}

/// `serde_json` 的字符串转义通道（`: "…"` 里那一截）。
/// 主干 `remindedUpdateTag` 是自由文本，必须走转义；`material` / `bubbleMaterial` 是闭集，
/// 走同一条通道出来的字节完全一样，所以三处共用一个出口，省掉「哪颗该转义」的心智负担。
fn quoted(value: &str) -> String {
    serde_json::Value::String(value.to_string()).to_string()
}

/// 手写 `shell.json` 渲染器 —— **逐字节配方**（SD3 §1.4-2）：
/// `{\r\n` + 每键行 `  "<key>": <value>`，非末键行尾一个 `,`，每行后 `\r\n`，
/// 最后单独一个 `}`，**`}` 之后无换行**、**无 BOM**（`std::fs::write` 天然无 BOM）。
///
/// 数字走 `{}`（Rust 最短往返），与 System.Text.Json 一致：`0.6 → 0.6`、`1.0 → 1`（**不带 `.0`**）、
/// `0.65 → 0.65`。**绝不能用定长两位小数格式化（`:` 加 `.2`）** —— 那会写出 `0.60`，字节立刻对不上。
pub fn render_shell_json(options: &ShellOptions) -> String {
    // 落盘前统一归一一次：白名单外的材质回默认档，opacity 夹回 0.2–1.0。
    // 主干靠 setter 保证存进 `_shellOptions` 的值已经合法，分叉的 `ShellOptions` 由
    // `fn create` 种值 + `Msg::*` 臂改写，两条路都可能在中间态拿到脏值。
    let opacity = {
        let raw = options.bubble_opacity;
        if !raw.is_finite() {
            BUBBLE_OPACITY_DEFAULT
        } else {
            raw.clamp(BUBBLE_OPACITY_MIN, BUBBLE_OPACITY_MAX)
        }
    };
    let lines: [String; 8] = [
        format!("  \"showTrayIcon\": {}", options.show_tray_icon),
        format!("  \"minimizeToTray\": {}", options.minimize_to_tray),
        format!("  \"closeToTray\": {}", options.close_to_tray),
        format!("  \"showNotifications\": {}", options.show_notifications),
        format!("  \"material\": {}", quoted(options.normalized_material())),
        format!("  \"bubbleMaterial\": {}", quoted(options.normalized_bubble_material())),
        format!("  \"bubbleOpacity\": {opacity}"),
        format!("  \"remindedUpdateTag\": {}", quoted(&options.reminded_update_tag)),
    ];

    let mut out = String::with_capacity(230);
    out.push_str("{");
    out.push_str(NEWLINE);
    for (index, line) in lines.iter().enumerate() {
        out.push_str(line);
        if index + 1 < lines.len() {
            out.push(',');
        }
        out.push_str(NEWLINE);
    }
    out.push('}');
    out
}

/// 主干 `LoadShellOptions:66-175` 的**逐字段独立兜底**（读端五道，SD3 §1.1）：
/// 整段坏 JSON ⇒ 全默认（`catch {}`「配置损坏 = 回默认，不影响启动」）；
/// bool 非 `true`/`false` ⇒ 该键的 fallback；材质白名单外 ⇒ `mica` / `translucent`；
/// `bubbleOpacity` 非数字 ⇒ 0.6，是数字 ⇒ `clamp(0.2, 1.0)`（**夹、不拒**）；
/// 字符串缺 / 非串 ⇒ `""`。**缺键与 `null` 同一条路**（`as_*()` 都给 `None`）。
///
/// 根不是 object（数组 / 裸数字 / 裸串）⇒ 主干那边 `root.ValueKind == Object` 全不成立，
/// 于是每个键都走 fallback ⇒ 与「空 object」等价，这里同结论。
pub fn parse_shell_json(text: &str) -> ShellOptions {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(text) else {
        return ShellOptions::default();
    };

    let mut options = ShellOptions::default();
    // `Value::get` 对非 object 返回 None ⇒ 天然等价于主干那串 `ValueKind == Object` 前置判定。
    let field = |name: &str| root.get(name);

    if let Some(value) = field("showTrayIcon").and_then(serde_json::Value::as_bool) {
        options.show_tray_icon = value;
    }
    if let Some(value) = field("minimizeToTray").and_then(serde_json::Value::as_bool) {
        options.minimize_to_tray = value;
    }
    if let Some(value) = field("closeToTray").and_then(serde_json::Value::as_bool) {
        options.close_to_tray = value;
    }
    if let Some(value) = field("showNotifications").and_then(serde_json::Value::as_bool) {
        options.show_notifications = value;
    }
    if let Some(id) = field("material").and_then(serde_json::Value::as_str) {
        if WINDOW_MATERIAL_IDS.contains(&id) {
            options.material = id.to_string();
        }
    }
    if let Some(id) = field("bubbleMaterial").and_then(serde_json::Value::as_str) {
        if BUBBLE_MATERIAL_IDS.contains(&id) {
            options.bubble_material = id.to_string();
        }
    }
    if let Some(value) = field("bubbleOpacity").and_then(serde_json::Value::as_f64) {
        options.bubble_opacity = value.clamp(BUBBLE_OPACITY_MIN, BUBBLE_OPACITY_MAX);
    }
    if let Some(tag) = field("remindedUpdateTag").and_then(serde_json::Value::as_str) {
        options.reminded_update_tag = tag.to_string();
    }
    options
}

/// 读 `shell.json`：文件不存在 / 读不动 ⇒ 全默认（主干 `File.Exists` 早退 + `catch {}`）。
pub fn load_shell_options_from(path: &Path) -> ShellOptions {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_shell_json(&text),
        Err(_) => ShellOptions::default(),
    }
}

pub fn load_shell_options() -> ShellOptions {
    load_shell_options_from(&shell_json_path())
}

/// 落盘 `shell.json`：先 `create_dir_all`（主干 `SaveShellOptions:164` 就是
/// `Directory.CreateDirectory(Path.GetDirectoryName(…))`），再裸写字节。
pub fn save_shell_options_to(options: &ShellOptions, path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, render_shell_json(options).as_bytes())
}

/// 真机落盘。主干 `SaveShellOptions:174` 是 `catch (Exception) { }` **「落盘失败不影响当前会话」**
/// —— 无日志、无备份、不重试，这里同语义。
pub fn save_shell_options(options: &ShellOptions) {
    let _ = save_shell_options_to(options, &shell_json_path());
}

// ============================== 2. shell-skin.opacity / .videopause ==============================

/// C# `value.ToString("0.##", InvariantCulture)` 的等价实现：**最多两位小数 + 尾零截掉**。
///
/// 走纯整数路径（先 `×100` 四舍五入成「分」，再按位拼串），刻意**不**用
/// 定长两位小数格式化（那会留尾零 `0.50`）也不用最短往返（两位以外会飘）。
/// 取整方向是 ties-away-from-zero，与 .NET 的数值格式化一致；主干实际只会喂到
/// `MainWindow.Personalization.cs:477` 算出来的 `整数百分比 / 100.0`，即**天然落在两位小数网格上**
/// （滑杆步长 5 ⇒ 只有 `0.10 … 0.80` 十五档），网格上两条路线逐串相同，见测
/// `render_skin_opacity_matches_the_fifteen_slider_steps`。
fn render_dotnet_two_max(value: f64) -> String {
    let cents = (value * 100.0).round() as i64;
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    let whole = abs / 100;
    let frac = abs % 100;
    if frac == 0 {
        format!("{sign}{whole}")
    } else if frac % 10 == 0 {
        format!("{sign}{whole}.{}", frac / 10)
    } else {
        format!("{sign}{whole}.{frac:02}")
    }
}

/// 渲染 `shell-skin.opacity`：入参是**分叉模型侧的百分数**（`tokens::SKIN_OPACITY_MIN..MAX`）。
/// 主干链路（`Personalization.cs:477` → `xaml.cs:8700`）：
/// `_skinOpacity = Math.Clamp(percent, 10, 80) / 100.0`，再 `ToString("0.##")`。
///
/// 非有限值（NaN / ±inf）返回默认串 `"0.4"`：主干那条路上会写出 `"NaN"` 这个非数字裸文本，
/// 读回 `double.TryParse(NumberStyles.Float)` 又**真能**吃进去 ⇒ 内存里留一个 NaN Opacity。
/// 与其复刻这个毒值回路，不如归一到默认（差异已在 `sf1-report.md` §5 声明）。
pub fn render_skin_opacity(percent: f64) -> String {
    if !percent.is_finite() {
        return render_dotnet_two_max(SKIN_OPACITY_DEFAULT);
    }
    render_dotnet_two_max((percent / 100.0).clamp(SKIN_OPACITY_MIN, SKIN_OPACITY_MAX))
}

/// 读 `shell-skin.opacity` → **百分数标度**（直接可以种进分叉 `self.numbers`）。
///
/// 主干 `LoadSkinOpacity:8600-8615` 的真实语义（⚠ 与 SD3 §2.1/§2.5 的口径**不同**，见报告 §9）：
/// 越界是 **`Math.Clamp` 夹回来**，不是「拒收并回默认」。只有这三种才落到默认 0.40：
/// ① 文件不存在 ② 读不动 ③ `Trim` 后不是合法不变文化浮点数。
/// `"0.05"` ⇒ `10.0`（夹到下限），`"0.9"` ⇒ `80.0`（夹到上限）。
pub fn parse_skin_opacity(text: &str) -> f64 {
    match text.trim().parse::<f64>() {
        Ok(value) if value.is_finite() => {
            value.clamp(SKIN_OPACITY_MIN, SKIN_OPACITY_MAX) * 100.0
        }
        _ => SKIN_OPACITY_DEFAULT * 100.0,
    }
}

/// 主干读点本体：`File.Exists` 早退（`:8603-8606`）与 `catch {}`（`:8614`）都落到默认 40.0。
pub fn load_skin_opacity_from(path: &Path) -> f64 {
    std::fs::read_to_string(path).map_or(SKIN_OPACITY_DEFAULT * 100.0, |text| {
        parse_skin_opacity(&text)
    })
}

pub fn load_skin_opacity() -> f64 {
    load_skin_opacity_from(&skin_opacity_path())
}

/// 主干 `SaveSkinOpacity:8694-8703`，建目录在 `:8698` **确实有** `Directory.CreateDirectory`
/// （SD3 §2.1 说「没有、赌 DataHome 已被建过」，实测源码有，见报告 §9）。
pub fn save_skin_opacity_to(percent: f64, path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, render_skin_opacity(percent).as_bytes())
}

/// 落盘失败吞掉（主干 `:8702 catch (Exception) { }`「不影响当前会话的显示」）。
pub fn save_skin_opacity(percent: f64) {
    let _ = save_skin_opacity_to(percent, &skin_opacity_path());
}

/// 渲染 `shell-skin.videopause`：裸 `"1"` / `"0"`，无换行（主干 `:8640`）。
pub const fn render_skin_video_pause(on: bool) -> &'static str {
    if on { "1" } else { "0" }
}

/// 主干 `LoadSkinVideoPauseOnBlur:8618-8633`：`Trim()` 后 **`text is "0" or "false"`** 才 `false`，
/// **其余一切（含文件不存在、读异常）⇒ `true`**（默认开）。
///
/// ⚠ C# 的常量模式匹配是**序数、区分大小写**的，所以盘上写 `"FALSE"` 时主干读出来是 `true`
/// （= 当默认开）。SD3 §2.1/§2.5 把它记成「OrdinalIgnoreCase 量级 / 建议 `eq_ignore_ascii_case`」，
/// 那是一处会让 `"FALSE"` 两边反号的错，这里按源码钉死，见报告 §9。
pub fn parse_skin_video_pause(text: &str) -> bool {
    let trimmed = text.trim();
    !(trimmed == "0" || trimmed == "false")
}

pub fn load_skin_video_pause_from(path: &Path) -> bool {
    std::fs::read_to_string(path).map_or(true, |text| parse_skin_video_pause(&text))
}

pub fn load_skin_video_pause() -> bool {
    load_skin_video_pause_from(&skin_video_pause_path())
}

pub fn save_skin_video_pause_to(on: bool, path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, render_skin_video_pause(on).as_bytes())
}

/// 落盘失败吞掉（主干 `:8642 catch (Exception) { }`）。
pub fn save_skin_video_pause(on: bool) {
    let _ = save_skin_video_pause_to(on, &skin_video_pause_path());
}

// ============================== 3. AGENTS.md ==============================

/// 渲染 `AGENTS.md` 的落盘内容。**`None` = 删文件**，不是写空文件 ——
/// 主干 `SaveInstructions:287-293` 的原话就在函数名边上：
/// 「清空 = 删文件：留一个空 `AGENTS.md` 只会往上下文里塞一段空指令」。
///
/// 形状 = `Trim()` 后的正文 + **恰好一个** `\r\n`（主干 `:297 text + Environment.NewLine`）。
/// `Trim` 只碰首尾，**内部换行原样保留**（TextBox 多行输入给什么就存什么）。
pub fn render_instructions(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string() + NEWLINE)
}

/// 读 `AGENTS.md`：`Ok` ⇒ `Trim()`（Rust `str::trim` 与 C# `String.Trim()` 都去全套 Unicode
/// 空白，含 `\r\n`），`Err` ⇒ `""`。主干 `LoadInstructions:230-237` 的 `catch {}`
/// 「读不到按空处理：编辑器仍可写入并覆盖」——**缺文件是常态不是错误**（大多数用户从没写过）。
pub fn parse_instructions(result: std::io::Result<String>) -> String {
    match result {
        Ok(text) => text.trim().to_string(),
        Err(_) => String::new(),
    }
}

pub fn load_instructions_from(path: &Path) -> String {
    parse_instructions(std::fs::read_to_string(path))
}

pub fn load_instructions() -> String {
    load_instructions_from(&agents_path())
}

/// [`write_instructions_to`] 的结果，给调用方的状态行用。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstructionsWrite {
    /// 写了正文（返回的 `&str` 语义由调用方从 [`render_instructions`] 的 `Some` 分支拿）。
    Written,
    /// 正文为空 ⇒ 文件被删掉。
    Deleted,
    /// 正文为空且文件本来就不在 ⇒ 什么都不做（主干 `File.Exists` 前置判定）。
    NoChange,
}

/// 落盘 `AGENTS.md`，逐句对应主干 `SaveInstructions:284-301`。
/// 清空那一支的 `remove_file` 把 `NotFound` 吞掉（对应主干 `if (File.Exists(…))` 的守卫）。
pub fn write_instructions_to(path: &Path, draft: &str) -> std::io::Result<InstructionsWrite> {
    let Some(body) = render_instructions(draft) else {
        match std::fs::remove_file(path) {
            Ok(()) => return Ok(InstructionsWrite::Deleted),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(InstructionsWrite::NoChange)
            }
            Err(error) => return Err(error),
        }
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, body.as_bytes())?;
    Ok(InstructionsWrite::Written)
}

/// 真机落盘。主干失败时 `catch` → 状态行「保存失败：{0}」，**不吞**（它有 UI 反馈）：
/// 所以这里把 `io::Result` 交回调用方，由接线侧决定怎么报错。
pub fn write_instructions(draft: &str) -> std::io::Result<InstructionsWrite> {
    write_instructions_to(&agents_path(), draft)
}

/// 主干 `InstructionsMaxLength = 4000`（`:30`，TextBox 属性，超限即截断）。
///
/// ⚠ 单位差异：C# 的 `MaxLength` / `.Length` 数的是 **UTF-16 码元**，Rust `chars().count()` 数的是
/// Unicode 标量。只有 BMP 外的字符（emoji、生僻字代理对）会差 1：一个 `😀` 在 C# 里算 2、
/// 在这里算 1 ⇒ 分叉会比主干多容 1 个这类字符。要逐字节对齐得自己数 `Encode::utf16`，
/// 为 1 个字符的边界差引入一套 UTF-16 记账不值，这条差异记进报告 §9 备档。
pub fn truncate_instructions(text: &str) -> &str {
    if text.chars().count() <= INSTRUCTIONS_MAX_CHARS {
        return text;
    }
    // 取前 4000 个标量：按 char 边界回退，绝不切断多字节序列。
    match text.char_indices().nth(INSTRUCTIONS_MAX_CHARS) {
        Some((offset, _)) => &text[..offset],
        None => text,
    }
}

// ============================== 4. pet-window.json ==============================

/// 渲染 `pet-window.json`：紧凑、**PascalCase**、冒号后**无空格**、无换行、坐标是 int 直写。
/// 主干 `Dsh/PetWindow.xaml.cs:329-330` 是 `JsonSerializer.Serialize(record)` **不带 options**
/// ⇒ 就是这个形状（实测 18 B：`7b22 5822 3a2d 3534 332c 2259 223a 3937 347d`）。
///
/// ⚠ 不许走 serde 结构体 / `json!`：本仓 `serde_json` 没开 `preserve_order`，`BTreeMap` 会按
/// 字母序排；本例 `X < Y` 恰好看着对，那是运气（SD3 §4.4-1 原话）。
pub fn render_pet_window_json(x: i32, y: i32) -> String {
    format!("{{\"X\":{x},\"Y\":{y}}}")
}

/// 主干 `RestorePosition:338-357` 的读端。`None` = 「落到虚拟屏右下默认位」（`MoveToDefault`）。
///
/// 三条失败面才落到 `None`（对应主干那个 `catch`）：
/// ① 不是合法 JSON ② 根不是 object（**含裸 `null`**：主干 `Deserialize` 给出 `null` 引用，
/// `saved is not null` 不成立 ⇒ 走默认位）③ `X` / `Y` **存在**但类型不是整数 / 超 i32。
///
/// ⚠ 这里刻意复刻一个主干细节（SD3 §4.5 说得不对，见报告 §9）：`PetWindowPosition(int X, int Y)`
/// 是**主构造函数**，System.Text.Json 对匹配不到的构造参数**填 `default(int)` = 0**，
/// 而不是抛异常 ⇒ 盘上是 `{"x":1,"y":2}`（小写，`PropertyNameCaseInsensitive` 默认 false，
/// 匹配不上）或 `{}` 时，主干拿到的是非 null 的 `(0, 0)` 并真的 `MoveTo(0, 0)`。
/// 所以这两种输入本函数返回 `Some((0, 0))` 而不是 `None`。
pub fn parse_pet_window_json(text: &str) -> Option<(i32, i32)> {
    let root = serde_json::from_str::<serde_json::Value>(text).ok()?;
    // 根不是 object（裸 `null` / 数组 / 数字 / 串）⇒ 主干那边 either 反序列化出 `null` 引用
    // （`saved is not null` 不成立）either 直接抛 JsonException，两条都落到 `MoveToDefault`。
    if !root.is_object() {
        return None;
    }
    let read = |key: &str| -> Option<i32> {
        // 键在但值不是整数（浮点 / 串 / null / 超 i32）⇒ 主干那条 Int32 转换抛异常 ⇒ None。
        // 键不在 ⇒ 主干构造参数取 0 ⇒ Some(0)。两条路必须分开，所以先看 `get` 有没有命中。
        match root.get(key) {
            None => Some(0),
            Some(value) => value.as_i64().and_then(|raw| i32::try_from(raw).ok()),
        }
    };
    let x = read("X")?;
    let y = read("Y")?;
    Some((x, y))
}

/// 主干 `RestorePosition` 本体：`_statePath == ""` 或 `File.Exists` 不成立或读不动 ⇒ `None`。
pub fn load_pet_window_position_from(path: &Path) -> Option<(i32, i32)> {
    parse_pet_window_json(&std::fs::read_to_string(path).ok()?)
}

pub fn load_pet_window_position() -> Option<(i32, i32)> {
    load_pet_window_position_from(&pet_window_path())
}

/// 主干 `SavePosition:329`（**不带** `Directory.CreateDirectory`，比 `.opacity` 那一支更赌；
/// 分叉统一在建目录后再写，与 `layout::save_to` 同口径，差异已在报告 §5 声明）。
pub fn save_pet_window_to(x: i32, y: i32, path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, render_pet_window_json(x, y).as_bytes())
}

pub fn save_pet_window(x: i32, y: i32) {
    let _ = save_pet_window_to(x, y, &pet_window_path());
}

/// 主干 `MoveToDefault:360-370` 的算术本体。
/// `vx/vy/vw/vh` = `SMXVIRTUALSCREEN / SMYVIRTUALSCREEN / SMCXVIRTUALSCREEN / SMCYVIRTUALSCREEN`，
/// `w/h` = `_lastSize`（物理像素）。取屏参数留给宠物宿主移植时接（分叉现在**没有宠物宿主**）。
///
/// 两条主干细节：`vw <= 0 || vh <= 0` ⇒ **直接 return，不调 MoveTo**（这里给 `None`）；
/// 减的是 `Math.Max(size, 1)` 再减固定边距 `48`。
pub fn pet_move_to_default(
    vx: i32,
    vy: i32,
    vw: i32,
    vh: i32,
    width: i32,
    height: i32,
) -> Option<(i32, i32)> {
    if vw <= 0 || vh <= 0 {
        return None;
    }
    Some((
        vx + vw - width.max(1) - 48,
        vy + vh - height.max(1) - 48,
    ))
}

// ============================== 5. shell-skin.we ==============================

/// 渲染 `shell-skin.we`：**恒等**。主干 `MainWindow.Wallpaper.cs:368` 是
/// `File.WriteAllText(SkinWeFile, id)` —— 不 Trim、不补换行、不转义，一个字的加工都没有。
///
/// 这个函数看着像废话，但它的价值就是把「恒等」钉成一个**可测的符号**：接线侧顺手加一个
/// `.trim()` 或补一个尾换行，本模块的字节测与 [`parse_skin_we_id`] 的往返立刻红。
///
/// 值域（两把尺）：
/// - **源码**：`id` 来自 `Dsh/DshWallpaperClient.cs:175-177`（`items[].id`；空串在 `:176`
///   被 `return null` 挡掉，所以网格路径给不出空 id）⇒ 这是**远端自由文本**，不是枚举、
///   不是本地拼出来的。比较侧 `MainWindow.Wallpaper.cs:182/201/229` 全是
///   `StringComparison.Ordinal` ⇒ 区分大小写、不规范化。
/// - **真机**：`%LOCALAPPDATA%\Blade2\shell-skin.we` 实测 **17 B**、
///   `76 69 64 65 6f 5f 5f 33 33 37 33 36 39 39 34 38 32`、无 BOM、无尾换行、无行内空白。
///
/// ⚠ 正因为它是远端自由文本，**不能假设它干净**：带首尾空白的 id 写得进、读不回（见
/// [`parse_skin_we_id`] 的 `Trim`）⇒ 往返不对称。这是主干自身的不对称，分叉**照抄不修**。
pub fn render_skin_we_id(id: &str) -> &str {
    id
}

/// 主干 `MainWindow.Wallpaper.cs:350-361 ReadSkinWeId`：`File.ReadAllText(path).Trim()`，
/// 然后 `text.Length > 0 ? text : null`（`:355`）。
///
/// 「空态」有三条路都通向 `None`：① 文件不存在（`:357-360` 的 `catch` 吞掉）② 0 字节
/// ③ 只有空白（被 `Trim` 抹成空）。
///
/// ⚠ 这一类的**默认态就是「没有这个文件」**：壳启动时 `_weAppliedId ??= ReadSkinWeId()`
/// （`:113`），读不到 = 当前背景不是 WE 来的，网格里没有任何一格带「使用中」。
/// 所以**清空 = 删文件**（[`clear_skin_we_id_from`]，走主干 `:374-378 ClearSkinWeId`
/// → `:338-348 DeleteSkinQuietly`），**不是**写一个空串。这一点和 `AGENTS.md` 同型。
pub fn parse_skin_we_id(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// 主干 `ReadSkinWeId` 本体：读不动（缺文件 / 占用 / 非 UTF-8）一律 `None`。
pub fn load_skin_we_id_from(path: &Path) -> Option<String> {
    parse_skin_we_id(&std::fs::read_to_string(path).ok()?).map(str::to_owned)
}

pub fn load_skin_we_id() -> Option<String> {
    load_skin_we_id_from(&skin_we_path())
}

/// 主干 `WriteSkinWeId:363-371`：建目录（`:367`）之后**原样**写 id（`:368`）。
pub fn save_skin_we_id_to(id: &str, path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, render_skin_we_id(id).as_bytes())
}

/// 落盘失败吞掉（主干 `:370 catch (Exception) { }`：「标记写不了只是网格里不标『使用中』，
/// 背景本身已生效」）。
pub fn save_skin_we_id(id: &str) {
    let _ = save_skin_we_id_to(id, &skin_we_path());
}

/// [`clear_skin_we_id_from`] 的结果，语义同 [`InstructionsWrite`] 的删文件两态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkinWeClear {
    /// 文件在、被删掉。
    Deleted,
    /// 本来就没有 ⇒ 什么都不做（主干 `DeleteSkinQuietly:342` 的 `File.Exists` 守卫）。
    NoChange,
}

/// 主干 `ClearSkinWeId:374-378` → `DeleteSkinQuietly:338-348`：`File.Exists` 守卫后
/// `File.Delete`，删不掉吞异常。**这就是本类的「清空」——不是写空文件。**
pub fn clear_skin_we_id_from(path: &Path) -> std::io::Result<SkinWeClear> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(SkinWeClear::Deleted),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(SkinWeClear::NoChange),
        Err(error) => Err(error),
    }
}

pub fn clear_skin_we_id() {
    let _ = clear_skin_we_id_from(&skin_we_path());
}

// ============================== 6. shell-toast.sender ==============================

/// 主干 `MainWindow.ShellIntegration.cs:343-353 NotificationIdentity()` 的纯函数版。
///
/// 主干靠异常分流：`try { "pkg:" + Package.Current.Id.FamilyName } catch { "exe:" + ProcessPath }`
/// —— 无包形态访问 `Package.Current` 会抛。分叉把它摊成 `Option`：拿到包系列名传 `Some`，
/// 拿不到（无包 / dev / 裸 exe）传 `None`。前缀见 [`TOAST_IDENTITY_PKG_PREFIX`] /
/// [`TOAST_IDENTITY_EXE_PREFIX`]。
///
/// ⚠ 刻意**不**给 `Some("")` 加兜底：主干那边 FamilyName 真是空串也照样落成 `pkg:`，
/// 加判空就是发明主干没有的行为。
pub fn notification_identity(pkg_family_name: Option<&str>, process_path: &str) -> String {
    match pkg_family_name {
        Some(family_name) => format!("{TOAST_IDENTITY_PKG_PREFIX}{family_name}"),
        None => format!("{TOAST_IDENTITY_EXE_PREFIX}{process_path}"),
    }
}

/// 渲染 `shell-toast.sender` 的**整份字节**。
///
/// 主干 `MainWindow.ShellIntegration.cs:339`：
/// `SenderStamp => NotificationIdentity() + "\n" + SenderName`，落点 `:416`
/// `File.WriteAllText(SenderStampPath, SenderStamp)`。三条形状规则：
/// 1. 行内分隔符是 **`0a` 单字节 LF**（C# 字面量 `"\n"`，**不跟平台走**），**不是**本模块
///    那颗 `0d 0a` 常量 —— 七类里只有这一类带行内换行，也只有它把「LF 还是 CRLF」变成行为差异；
/// 2. **无尾换行**（`File.WriteAllText` 什么都不追加）；
/// 3. **UTF-8 无 BOM**（.NET 的 `StreamWriter` 默认 `UTF8Encoding(false)`）——
///    末字段 `Blade\u{b2}` 因此落成 `42 6c 61 64 65 c2 b2`，前面不许有 `ef bb bf`。
///    ⚠ 这条是**与真产物逐字节一致**的要求，不是比对的要求：见 [`sender_stamp_matches`] 里
///    关于 BOM 的那段反直觉结论。
///
/// 真机第二把尺：`%LOCALAPPDATA%\Blade2\shell-toast.sender` 实测 **32 B** =
/// `pkg:Blade2_wr2brkarxqkxy`(24) + `0a`(1) + `Blade`+`c2 b2`(7)。
///
/// 三条一条都不能弯的原因在读端：`:394` 是
/// `File.ReadAllText(SenderStampPath) == SenderStamp` —— **`==` 全等、不 `Trim`**。
/// 字节一弯，`MigrateStaleSenderName`（`:390-409`）就判定标记过期 ⇒ 每次启动白跑一次
/// `UnregisterAll()` 再注册 ⇒ toast 头部的应用名反复被清，而这条路径**没有任何用户可见报错**。
pub fn render_sender_stamp(identity: &str, sender_name: &str) -> String {
    let mut out = String::with_capacity(identity.len() + 1 + sender_name.len());
    out.push_str(identity);
    out.push(TOAST_SENDER_SEPARATOR);
    out.push_str(sender_name);
    out
}

/// 主干 `:394` 读回时的 BOM 处理：**`.NET` 的 `File.ReadAllText` 默认嗅探并吞掉开头的
/// UTF-8 BOM**（`detectEncodingFromByteOrderMarks` 默认 `true`），BOM 不进返回的串。
/// Rust 的 `read_to_string` **不吞** ⇒ 不显式去掉的话，「盘上带 BOM」这一格主干判**相等**、
/// 分叉判**不等**，凭空多跑一次 `UnregisterAll()`。真产物无 BOM，这条只影响脏盘。
fn strip_utf8_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

/// 主干 `MigrateStaleSenderName:394` 的**全部**读回语义：盘上文本与现算 stamp **全等**
/// 才算「没过期」，相等 ⇒ `return`（什么都不做）；不等 ⇒ 走 `UnregisterAll` + 重注册。
///
/// 刻意**不做** `trim()`、刻意**不**折叠行尾、刻意**不**忽略大小写 —— 主干那行就是 `==`。
/// 唯一的复刻项是 BOM 嗅探（见 [`strip_utf8_bom`]）。
pub fn sender_stamp_matches(file_text: &str, identity: &str, sender_name: &str) -> bool {
    strip_utf8_bom(file_text) == render_sender_stamp(identity, sender_name)
}

/// 主干 `:394` 那个合取式的完整版（含 `File.Exists` 那一臂）。三条都落到「过期」：
/// ① 文件不存在（`File.Exists` 不成立）② 读不动 / 非 UTF-8（主干读回来的串必然与
/// 现算值不等）③ 字节不等。
pub fn sender_stamp_is_fresh_at(path: &Path, identity: &str, sender_name: &str) -> bool {
    match std::fs::read_to_string(path) {
        Ok(text) => sender_stamp_matches(&text, identity, sender_name),
        Err(_) => false,
    }
}

/// 主干 `RememberSenderName:411-421`：建目录（`:415`）后写整份 stamp（`:416`）。
pub fn save_sender_stamp_to(identity: &str, sender_name: &str, path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, render_sender_stamp(identity, sender_name).as_bytes())
}

/// 主干 `RememberSenderName` 本体（发送者名是 `:322` 的常量，不由调用方给）。
/// 写失败吞掉（主干 `:418`「标记写不了不影响通知本身，只是下次启动会再清一次注册」）。
pub fn save_sender_stamp(identity: &str) {
    let _ = save_sender_stamp_to(identity, TOAST_SENDER_NAME, &toast_sender_path());
}

// ============================== 测试 ==============================

#[cfg(test)]
mod tests {
    use super::*;

    /// 落盘类测试一律走 `std::env::temp_dir()` 下「进程内唯一 + 每测唯一」的子目录，
    /// 语义仿 `layout.rs::save_and_load_round_trip_on_disk`（`ly1-layout-<pid>`），
    /// 自建自删，**绝不碰真 `%LOCALAPPDATA%\Blade2`**。
    /// 每测各带一个 `tag`：同进程的多个测并行跑，只按 pid 分目录会互相踩。
    fn sandbox(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("sf1-{}-{:?}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    // ---------- 真产物字节（只读 `C:\Users\Admin\AppData\Local\Blade2` 的现成文件抄下来的字面量） ----------

    /// `%LOCALAPPDATA%\Blade2\shell.json` 实测 198 B（mtime 09-23 00:28）。
    /// **7 键**：写它的时候 `remindedUpdateTag` 还没进 `ShellOptions`。
    const REAL_SHELL_JSON: &str = concat!(
        "{\r\n",
        "  \"showTrayIcon\": true,\r\n",
        "  \"minimizeToTray\": false,\r\n",
        "  \"closeToTray\": false,\r\n",
        "  \"showNotifications\": true,\r\n",
        "  \"material\": \"acrylic\",\r\n",
        "  \"bubbleMaterial\": \"translucent\",\r\n",
        "  \"bubbleOpacity\": 0.6\r\n",
        "}",
    );

    /// `%LOCALAPPDATA%\Blade2\pet-window.json` 实测 18 B（hexdump `7b22 5822 …347d`）。
    const REAL_PET_WINDOW_JSON: &str = "{\"X\":-543,\"Y\":974}";

    /// `%LOCALAPPDATA%\Blade2\AGENTS.md` 实测 45 B（14 个 CJK + 1 空格 + 单个 `0d 0a`、无 BOM）。
    const REAL_AGENTS_MD: &str = "叫我主人 每次最后都要加一句喵\r\n";

    /// `%LOCALAPPDATA%\Blade2\shell-skin.we` 实测 **17 B**：
    /// `76 69 64 65 6f 5f 5f 33 33 37 33 36 39 39 34 38 32`，纯 ASCII、无 BOM、**无尾换行**。
    /// 拆两段拼，是为了让「末位是 `2` 不是 `0a`」这件事在字节测里显式成立。
    const REAL_SKIN_WE_ID: &str = concat!("video__337369948", "2");

    /// `%LOCALAPPDATA%\Blade2\shell-toast.sender` 里的第一段 = 通知身份（`pkg:` + 包系列名），
    /// 实测 24 B。
    const REAL_SENDER_IDENTITY: &str = concat!("pkg:", "Blade2_wr2brkarxqkxy");

    /// `%LOCALAPPDATA%\Blade2\shell-toast.sender` 实测 **32 B**：
    /// `70 6b 67 3a … 79 | 0a | 42 6c 61 64 65 c2 b2` ⇒ 24 + 1 + 7。
    /// 分隔符这里必须写成 Rust 的 `"\n"`（就是 `0a`），**不能**跟着本模块那颗 `0d 0a` 走。
    const REAL_SENDER_STAMP: &str = concat!("pkg:Blade2_wr2brkarxqkxy", "\n", "Blade\u{b2}");

    // ---------- 1. shell.json ----------

    /// 渲染器逐字节等于真产物外推出的 8 键形：在实测 198 B 的末键行后补逗号 + 新增末键行。
    #[test]
    fn render_shell_json_is_the_measured_artifact_plus_the_eighth_key() {
        assert_eq!(REAL_SHELL_JSON.len(), 198, "真产物基线自己先钉住");
        let expected =
            REAL_SHELL_JSON.replace("0.6\r\n}", "0.6,\r\n  \"remindedUpdateTag\": \"\"\r\n}");
        let options = ShellOptions {
            material: MATERIAL_ACRYLIC.to_string(),
            ..ShellOptions::default()
        };
        let rendered = render_shell_json(&options);
        assert_eq!(rendered, expected);
        // ⚠ SD3 §1.2/§7.3 推的是 **227 B**，实算是 226 B（见报告 §9 的纠正）。
        assert_eq!(rendered.len(), 226);
        assert_eq!(rendered.as_bytes()[rendered.len() - 1], b'}', "末字节该是闭花括号，后面不许有换行");
    }

    /// 键序 = 声明序，**不是**字母序（BTreeMap 那条路的反锁）。
    #[test]
    fn render_shell_json_keys_follow_declaration_order_not_alphabetical() {
        let rendered = render_shell_json(&ShellOptions::default());
        let order = [
            "showTrayIcon",
            "minimizeToTray",
            "closeToTray",
            "showNotifications",
            "material",
            "bubbleMaterial",
            "bubbleOpacity",
            "remindedUpdateTag",
        ];
        let mut previous = 0usize;
        for key in order {
            let at = rendered
                .find(key)
                .unwrap_or_else(|| panic!("少键 {key}: {rendered}"));
            assert!(at > previous, "键序错了：{key} 出现在上一键之前");
            previous = at;
        }
        // 字母序的第一颗会是 bubbleMaterial / bubbleOpacity —— 反锁它。
        assert!(rendered.starts_with("{\r\n  \"showTrayIcon\": "), "落成了字母序");
    }

    /// 行形状三件套：`\r\n` 分隔、2 空格缩进、`": "`，且**没有**裸 `\n`、**没有** BOM。
    #[test]
    fn render_shell_json_shape_is_crlf_two_space_indent_and_bomless() {
        let rendered = render_shell_json(&ShellOptions::default());
        let bytes = rendered.as_bytes();
        assert!(!bytes.starts_with(&[0xEF, 0xBB, 0xBF][..]), "不许 BOM");
        assert_eq!(bytes.iter().filter(|b| **b == b'\n').count(), 9, "换行数应为 1 个左花括号行 + 8 个键行");
        assert_eq!(bytes.iter().filter(|b| **b == b'\r').count(), 9, "每个 \\n 都该配对 \\r");
        assert!(!rendered.ends_with('\n') && !rendered.ends_with('\r'));
        for line in rendered.split(NEWLINE) {
            if line == "{" || line == "}" {
                continue;
            }
            assert!(line.starts_with("  \""), "缩进不是 2 空格: {line:?}");
            assert!(line.contains("\": "), "冒号后不是单个空格: {line:?}");
        }
        // 恰有一个 `}` 独占末行
        assert_eq!(rendered.split(NEWLINE).last().unwrap(), "}");
        assert_eq!(rendered.split(NEWLINE).count(), 10, "1 行左花括号 + 8 行键 + 1 行右花括号");
    }

    /// double 走最短往返：整数化的 1.0 写 `1` 不写 `1.0`，`0.65` 不写成 `0.60`。
    #[test]
    fn render_shell_json_numbers_are_minimal_roundtrip() {
        let one = ShellOptions { bubble_opacity: 1.0, ..ShellOptions::default() };
        assert!(render_shell_json(&one).contains("  \"bubbleOpacity\": 1,\r\n"),
            "1.0 该写成 1：{}", render_shell_json(&one));
        let awkward = ShellOptions { bubble_opacity: 0.65, ..ShellOptions::default() };
        assert!(render_shell_json(&awkward).contains("  \"bubbleOpacity\": 0.65,"));
        assert!(!render_shell_json(&awkward).contains("0.650"));
        assert!(!render_shell_json(&one).contains("1.0"));
    }

    /// 落盘前归一：脏材质 / 越界 opacity 不会写出主干不会写的字节。
    #[test]
    fn render_shell_json_normalizes_before_writing() {
        let dirty = ShellOptions {
            material: "Acrylic".to_string(),
            bubble_material: "transcluent".to_string(),
            bubble_opacity: 9.0,
            ..ShellOptions::default()
        };
        let rendered = render_shell_json(&dirty);
        assert!(rendered.contains("  \"material\": \"mica\","));
        assert!(rendered.contains("  \"bubbleMaterial\": \"translucent\","));
        assert!(rendered.contains("  \"bubbleOpacity\": 1,"));
        let nan = ShellOptions { bubble_opacity: f64::NAN, ..ShellOptions::default() };
        assert!(render_shell_json(&nan).contains("  \"bubbleOpacity\": 0.6,"));
    }

    /// 自由文本 `remindedUpdateTag` 必须过转义通道：内嵌引号 / 反斜杠 / 换行都不塌结构。
    #[test]
    fn render_shell_json_escapes_the_free_form_tag() {
        let options = ShellOptions {
            reminded_update_tag: "a\"b\\c\r\nd".to_string(),
            ..ShellOptions::default()
        };
        let rendered = render_shell_json(&options);
        assert!(rendered.contains("  \"remindedUpdateTag\": \"a\\\"b\\\\c\\r\\nd\"\r\n}"), "{rendered}");
        // 行数不受影响（值里的换行是转义序列不是裸换行）
        assert_eq!(rendered.as_bytes().iter().filter(|b| **b == b'\n').count(), 9);
        assert_eq!(parse_shell_json(&rendered), options);
    }

    /// 真产物（7 键）解析：缺的第 8 键 ⇒ `""`，其余命中实测值。
    #[test]
    fn parse_shell_json_accepts_the_measured_seven_key_artifact() {
        let options = parse_shell_json(REAL_SHELL_JSON);
        assert!(options.show_tray_icon);
        assert!(!options.minimize_to_tray);
        assert!(!options.close_to_tray);
        assert!(options.show_notifications);
        assert_eq!(options.material, "acrylic");
        assert_eq!(options.bubble_material, "translucent");
        assert_eq!(options.bubble_opacity, 0.6);
        assert_eq!(options.reminded_update_tag, "");
        // 读旧写新：补出第 8 键，形状就是上面那条 226 B
        assert_eq!(render_shell_json(&options).len(), 226);
    }

    /// 读端五道兜底，逐条对应主干 `GetShellMaterial/GetShellBubbleMaterial/GetShellBubbleOpacity/
    /// GetShellFlag/GetShellText`。**null 与缺键同一条路**。
    #[test]
    fn parse_shell_json_falls_back_per_field_like_the_tray_reader() {
        // ① 坏 JSON ⇒ 全默认，不 panic
        for broken in ["{", "not json", "", "[", "{\"showTrayIcon\":"] {
            assert_eq!(parse_shell_json(broken), ShellOptions::default(), "坏 JSON: {broken}");
        }
        // ② 材质白名单外 ⇒ mica；气泡白名单外 ⇒ translucent（大小写敏感，主干是 `is` 常量模式）
        assert_eq!(parse_shell_json(r#"{"material":"bogus"}"#).material, "mica");
        assert_eq!(parse_shell_json(r#"{"material":"Acrylic"}"#).material, "mica");
        assert_eq!(parse_shell_json(r#"{"material":null}"#).material, "mica");
        assert_eq!(parse_shell_json(r#"{"bubbleMaterial":"bogus"}"#).bubble_material, "translucent");
        // ③ opacity：越界**夹**、非数字/缺/null ⇒ 默认 0.6
        assert_eq!(parse_shell_json(r#"{"bubbleOpacity":9.0}"#).bubble_opacity, 1.0);
        assert_eq!(parse_shell_json(r#"{"bubbleOpacity":0.05}"#).bubble_opacity, 0.2);
        assert_eq!(parse_shell_json(r#"{"bubbleOpacity":null}"#).bubble_opacity, 0.6);
        assert_eq!(parse_shell_json(r#"{"bubbleOpacity":"0.6"}"#).bubble_opacity, 0.6);
        // ④ bool 只认真 JSON 布尔
        assert!(parse_shell_json(r#"{"showTrayIcon":true}"#).show_tray_icon);
        assert!(!parse_shell_json(r#"{"showTrayIcon":false}"#).show_tray_icon);
        assert!(parse_shell_json(r#"{"showTrayIcon":"yes"}"#).show_tray_icon, "非布尔该回默认 true");
        assert!(!parse_shell_json(r#"{"minimizeToTray":1}"#).minimize_to_tray, "非布尔该回默认 false");
        // ⑤ 字符串：缺 / 非串 ⇒ ""
        assert_eq!(parse_shell_json(r#"{"remindedUpdateTag":5}"#).reminded_update_tag, "");
        assert_eq!(parse_shell_json(r#"{"remindedUpdateTag":"v1.2.3"}"#).reminded_update_tag, "v1.2.3");
        // 根不是 object ⇒ 每个键都走 fallback（主干 `ValueKind == Object` 前置判定）
        for scalar in [r#""str""#, "5", "true", "null", "[]"] {
            assert_eq!(parse_shell_json(scalar), ShellOptions::default(), "非标量根: {scalar}");
        }
    }

    /// parse → render → parse 定点。
    #[test]
    fn shell_json_round_trip_is_stable() {
        let options = ShellOptions {
            show_tray_icon: false,
            minimize_to_tray: true,
            close_to_tray: true,
            show_notifications: false,
            material: MATERIAL_MICA_ALT.to_string(),
            bubble_material: BUBBLE_MATERIAL_FOLLOW.to_string(),
            bubble_opacity: 0.45,
            reminded_update_tag: "0.8.2.0".to_string(),
        };
        let once = render_shell_json(&options);
        let twice = render_shell_json(&parse_shell_json(&once));
        assert_eq!(once, twice);
        assert_eq!(parse_shell_json(&once), options);
    }

    /// 真落盘往返：目录自建、无 BOM、`}` 后无换行、读回同值。走临时目录。
    #[test]
    fn shell_json_disk_round_trip_and_byte_shape() {
        let dir = sandbox("shell-json");
        let path = data_home_from(Some(&dir)).join(SHELL_JSON_FILE);
        let options = ShellOptions {
            material: MATERIAL_ACRYLIC.to_string(),
            ..ShellOptions::default()
        };
        save_shell_options_to(&options, &path).expect("落盘");
        let bytes = std::fs::read(&path).expect("读回原始字节");
        assert_eq!(bytes.len(), 226);
        assert_ne!(&bytes[0..3], &[0xEF, 0xBB, 0xBF][..], "不许有 BOM");
        assert_eq!(bytes[bytes.len() - 1], b'}');
        assert_eq!(load_shell_options_from(&path), options);

        // 缺文件 ⇒ 默认，不 panic；坏文件 ⇒ 默认，不 panic
        assert_eq!(load_shell_options_from(&dir.join("nope.json")), ShellOptions::default());
        std::fs::write(&path, b"{{{").expect("写坏文件");
        assert_eq!(load_shell_options_from(&path), ShellOptions::default());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 下标 ↔ id 映射与分叉下拉/`theme::*::from_index` 同序。
    #[test]
    fn material_index_maps_to_the_mainline_ids_and_back() {
        assert_eq!(
            WINDOW_MATERIAL_IDS,
            ["mica", "mica-alt", "acrylic", "none"]
        );
        assert_eq!(BUBBLE_MATERIAL_IDS, ["translucent", "acrylic", "follow"]);
        for (index, id) in WINDOW_MATERIAL_IDS.iter().enumerate() {
            assert_eq!(window_material_id_for_index(index), *id);
            assert_eq!(window_material_index_for_id(id), index);
        }
        for (index, id) in BUBBLE_MATERIAL_IDS.iter().enumerate() {
            assert_eq!(bubble_material_id_for_index(index), *id);
            assert_eq!(bubble_material_index_for_id(id), index);
        }
        // 越界 / 未知都回第 0 档
        assert_eq!(window_material_id_for_index(99), MATERIAL_MICA);
        assert_eq!(window_material_index_for_id("bogus"), 0);
        assert_eq!(bubble_material_id_for_index(99), BUBBLE_MATERIAL_TRANSLUCENT);
        assert_eq!(bubble_material_index_for_id("bogus"), 0);
    }

    /// 百分数 ↔ 0–1 的换算与主干脏检容差。
    #[test]
    fn opacity_conversions_and_the_epsilon_dirty_check() {
        assert_eq!(bubble_opacity_from_percent(60.0), 0.6);
        assert_eq!(bubble_opacity_from_percent(45.0), 0.45);
        assert_eq!(bubble_opacity_from_percent(5.0), BUBBLE_OPACITY_MIN, "夹到 0.2");
        assert_eq!(bubble_opacity_from_percent(500.0), BUBBLE_OPACITY_MAX, "夹到 1.0");
        assert_eq!(bubble_opacity_from_percent(f64::NAN), BUBBLE_OPACITY_DEFAULT);
        assert_eq!(percent_from_bubble_opacity(0.6), 60.0);
        assert_eq!(percent_from_bubble_opacity(0.0), BUBBLE_OPACITY_MIN * 100.0);
        // 容差 `< 0.0001`，不是 `==`
        assert!(bubble_opacity_unchanged(0.6, 0.6 + 0.00005));
        assert!(!bubble_opacity_unchanged(0.6, 0.6002));
        assert!(bubble_opacity_unchanged(0.6, 0.6));
    }

    // ---------- 2. .opacity / .videopause ----------

    /// SD3 §7.3 欠的那条「15 值全枚举」：C# `"0.##"` 与这条实现在滑杆网格上逐串一致。
    #[test]
    fn render_skin_opacity_matches_the_fifteen_slider_steps() {
        let grid: &[(f64, &str)] = &[
            (10.0, "0.1"),
            (15.0, "0.15"),
            (20.0, "0.2"),
            (25.0, "0.25"),
            (30.0, "0.3"),
            (35.0, "0.35"),
            (40.0, "0.4"),
            (45.0, "0.45"),
            (50.0, "0.5"),
            (55.0, "0.55"),
            (60.0, "0.6"),
            (65.0, "0.65"),
            (70.0, "0.7"),
            (75.0, "0.75"),
            (80.0, "0.8"),
        ];
        for (percent, expected) in grid {
            let rendered = render_skin_opacity(*percent);
            assert_eq!(&rendered, expected, "滑杆 {percent}% 落盘串不对");
            assert!(!rendered.ends_with('0'), "尾零没截掉: {rendered}");
            // 读回的**串**必须一模一样（数值上 f64 有 1 ulp 噪声，见下面的 approx 版）
            assert_eq!(render_skin_opacity(parse_skin_opacity(&rendered)), rendered);
            assert!(approx(parse_skin_opacity(&rendered), *percent), "{rendered} 读不回原值");
        }
    }

    /// 浮点判等：`×100` 这条路在 `0.55` 这类值上有 1 ulp 噪声（`0.55*100` 舍到
    /// `55.000000000000007`），落盘串不受影响（`render_dotnet_two_max` 先四舍五入成「分」），
    /// 只有数值往返要留容差。
    fn approx(actual: f64, expected: f64) -> bool {
        (actual - expected).abs() < 1e-9
    }

    /// SD3 §7.3 标「推的」那条 4 字节样例：`45% ⇒ 30 2e 34 35`。
    #[test]
    fn render_skin_opacity_45_is_four_exact_bytes() {
        let bytes = render_skin_opacity(45.0).into_bytes();
        assert_eq!(bytes, vec![0x30, 0x2e, 0x34, 0x35]);
        assert_eq!(bytes.len(), 4);
        assert!(!bytes.contains(&b'\n') && !bytes.contains(&b'\r'), "裸文本，无换行");
        assert_ne!(&bytes[0..3], &[0xEF, 0xBB, 0xBF][..], "无 BOM");
        // 50% ⇒ 3 字节 `0.5`
        assert_eq!(render_skin_opacity(50.0).as_bytes(), b"0.5");
    }

    /// `{:.2}` 陷阱反锁：绝不出 `0.50` / `0.60` 这种主干不会写的字节。
    #[test]
    fn render_skin_opacity_never_pads_with_trailing_zero() {
        for percent in [10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0] {
            let rendered = render_skin_opacity(percent);
            assert!(rendered.ends_with(|c: char| c != '0'), "{percent}% ⇒ {rendered} 补了尾零");
        }
        assert_ne!(render_skin_opacity(50.0), "0.50");
        assert_ne!(render_skin_opacity(40.0), "0.40");
    }

    /// 越界夹到 [10, 80]（主干 `Personalization.cs:477` 的 `Math.Clamp(percent, 10, 80)`）。
    #[test]
    fn render_skin_opacity_clamps_out_of_range_percent() {
        assert_eq!(render_skin_opacity(5.0), "0.1");
        assert_eq!(render_skin_opacity(0.0), "0.1");
        assert_eq!(render_skin_opacity(95.0), "0.8");
        assert_eq!(render_skin_opacity(100.0), "0.8");
        assert_eq!(render_skin_opacity(-50.0), "0.1");
        // 非有限 ⇒ 一律默认串。主干这条路走不到非有限（`Personalization.cs:477` 的入参是
        // **整数**百分比），所以这里不需要复刻 C# 对 ±inf 的夹取细节；判「不是数」比判「NaN」
        // 更省事，也给接线侧一个不会写出毒值的出口。
        assert_eq!(render_skin_opacity(f64::NAN), "0.4");
        assert_eq!(render_skin_opacity(f64::INFINITY), "0.4");
        assert_eq!(render_skin_opacity(f64::NEG_INFINITY), "0.4");
    }

    /// 读端：**越界是夹不是拒**（这条纠正 SD3 §2.5，见报告 §9）。
    #[test]
    fn parse_skin_opacity_clamps_instead_of_rejecting() {
        assert_eq!(parse_skin_opacity("0.45"), 45.0);
        assert_eq!(parse_skin_opacity("0.5"), 50.0);
        assert_eq!(parse_skin_opacity("0.05"), 10.0, "该夹到下限，不是回默认");
        assert_eq!(parse_skin_opacity("0.9"), 80.0, "该夹到上限，不是回默认");
        assert_eq!(parse_skin_opacity("0.1"), 10.0);
        assert_eq!(parse_skin_opacity("0.8"), 80.0);
        assert_eq!(parse_skin_opacity("1"), 80.0);
    }

    /// 只有「读不到 / 不是数」才落默认 40.0；顺带钉住两边浮点语法一致。
    #[test]
    fn parse_skin_opacity_falls_back_only_on_unparsable_text() {
        for junk in ["", "   ", "junk", "0,45", "NaN", "null", ".5x", "0.45abc", "1e"] {
            assert_eq!(parse_skin_opacity(junk), 40.0, "非数文本: {junk:?}");
        }
        // 主干 `NumberStyles.Float` 允许指数与前后符号 ⇒ Rust `parse::<f64>` 同域
        assert_eq!(parse_skin_opacity("4.5e-1"), 45.0);
        assert_eq!(parse_skin_opacity("+0.45"), 45.0);
        assert_eq!(parse_skin_opacity("  0.45  \r\n"), 45.0, "Trim 后照样吃");
        // 文件不存在 ⇒ 调用方走 40.0
        let dir = sandbox("skin-opacity-missing");
        assert_eq!(load_skin_opacity_from(&dir.join(SKIN_OPACITY_FILE)), 40.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 真落盘往返 + 字节形状（临时目录）。
    #[test]
    fn skin_opacity_disk_round_trip_and_byte_shape() {
        let dir = sandbox("skin-opacity");
        let path = data_home_from(Some(&dir)).join(SKIN_OPACITY_FILE);
        save_skin_opacity_to(45.0, &path).expect("落盘");
        let bytes = std::fs::read(&path).expect("读回");
        assert_eq!(bytes, vec![0x30, 0x2e, 0x34, 0x35]);
        assert_eq!(load_skin_opacity_from(&path), 45.0);
        save_skin_opacity_to(80.0, &path).expect("再落盘");
        assert_eq!(std::fs::read(&path).expect("读回"), b"0.8");
        assert_eq!(load_skin_opacity_from(&path), 80.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `.videopause`：裸 `"1"` / `"0"`，一字节。
    #[test]
    fn render_skin_video_pause_is_one_byte() {
        assert_eq!(render_skin_video_pause(true).as_bytes(), b"1");
        assert_eq!(render_skin_video_pause(false).as_bytes(), b"0");
        assert_eq!(render_skin_video_pause(false).len(), 1);
    }

    /// 读端：**区分大小写的序数匹配**（这条纠正 SD3 §2.1/§2.5，见报告 §9）。
    #[test]
    fn parse_skin_video_pause_is_case_sensitive_like_the_csharp_pattern() {
        assert!(!parse_skin_video_pause("0"));
        assert!(!parse_skin_video_pause("false"));
        assert!(!parse_skin_video_pause("  0 \r\n"), "Trim 后命中");
        // 大小写不同 ⇒ 主干命中不了 `is "0" or "false"` ⇒ 保持默认开
        assert!(parse_skin_video_pause("FALSE"));
        assert!(parse_skin_video_pause("False"));
        assert_eq!(parse_skin_video_pause("0 "), false, "首尾空白被 Trim，命中 \"0\" ⇒ false");
        assert!(parse_skin_video_pause("00"), "两个 0 不是主干认的串");
        assert!(parse_skin_video_pause("1"));
        assert!(parse_skin_video_pause(""));
        assert!(parse_skin_video_pause("junk"));
        assert!(parse_skin_video_pause("true"));
    }

    /// 真落盘往返（临时目录）+ 缺文件默认开。
    #[test]
    fn skin_video_pause_disk_round_trip() {
        let dir = sandbox("skin-videopause");
        let path = data_home_from(Some(&dir)).join(SKIN_VIDEO_PAUSE_FILE);
        assert!(load_skin_video_pause_from(&path), "缺文件 = 默认开");
        save_skin_video_pause_to(false, &path).expect("落盘");
        assert_eq!(std::fs::read(&path).expect("读回"), b"0");
        assert!(!load_skin_video_pause_from(&path));
        save_skin_video_pause_to(true, &path).expect("再落盘");
        assert!(load_skin_video_pause_from(&path));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---------- 3. AGENTS.md ----------

    /// 逐字节等于实测 45 B 真产物（内容字面量焊进断言，不读真目录）。
    #[test]
    fn render_instructions_matches_the_measured_45_byte_artifact() {
        assert_eq!(REAL_AGENTS_MD.len(), 45, "真产物基线自己先钉住");
        let body = "叫我主人 每次最后都要加一句喵";
        let rendered = render_instructions(body).expect("非空该给内容");
        assert_eq!(rendered.as_bytes(), REAL_AGENTS_MD.as_bytes());
        assert!(rendered.ends_with(NEWLINE));
        // 首尾字节：无 BOM、尾恰为 `0d 0a`
        let bytes = rendered.as_bytes();
        assert_ne!(&bytes[0..3], &[0xEF, 0xBB, 0xBF][..]);
        assert_eq!(&bytes[bytes.len() - 2..], &[0x0d, 0x0a]);
        assert_eq!(bytes.iter().filter(|b| **b == b'\n').count(), 1, "尾巴上只许一个换行");
        assert_eq!(bytes.iter().filter(|b| **b == b'\r').count(), 1);
        // 读回来去掉尾巴，与主干 `ReadAllText(...).Trim()` 同结论
        assert_eq!(parse_instructions(Ok(rendered)), body);
    }

    /// 空 / 全空白 ⇒ `None`（= 删文件），不是空串。
    #[test]
    fn render_instructions_treats_blank_as_delete() {
        for blank in ["", "   ", "\n", "\r\n", "  \t \r\n ", "\u{00a0}\n"] {
            assert_eq!(render_instructions(blank), None, "空白该给 None: {blank:?}");
        }
    }

    /// `Trim` 只碰首尾，尾巴补**恰好一个** `\r\n`；内部换行原样保留。
    #[test]
    fn render_instructions_trims_and_appends_exactly_one_crlf() {
        assert_eq!(render_instructions("a"), Some("a\r\n".to_string()));
        assert_eq!(render_instructions("  a\r\n \n"), Some("a\r\n".to_string()));
        assert_eq!(render_instructions("a\n"), Some("a\r\n".to_string()), "内部没有，尾部 LF 被 trim 掉了");
        // 内部多行不展平、不改写；尾巴只加一个 CRLF
        assert_eq!(render_instructions("a\r\nb"), Some("a\r\nb\r\n".to_string()));
        assert_eq!(render_instructions("a\nb"), Some("a\nb\r\n".to_string()));
        assert_eq!(render_instructions("a\r\nb\r\n"), Some("a\r\nb\r\n".to_string()));
        assert_eq!(render_instructions("a\n\nb").unwrap().matches(NEWLINE).count(), 1);
        // 绝不用 `std::env::newline` 之类的平台口：本函数永远出 CRLF
        assert!(render_instructions("x").unwrap().ends_with("\r\n"));
    }

    /// 清空 = **删文件**（不是留一个 0 字节文件）——这条是本类最容易写歪的地方。
    #[test]
    fn write_instructions_to_deletes_on_empty_instead_of_writing_zero_bytes() {
        let dir = sandbox("agents");
        let path = data_home_from(Some(&dir)).join(AGENTS_MD_FILE);

        assert_eq!(write_instructions_to(&path, "喵").unwrap(), InstructionsWrite::Written);
        assert_eq!(std::fs::read(&path).unwrap(), "喵\r\n".as_bytes());
        assert_eq!(std::fs::read(&path).unwrap().len(), 5, "3 字节正文 + 一个 CRLF");

        // 清空 ⇒ 删
        assert_eq!(write_instructions_to(&path, "   \r\n ").unwrap(), InstructionsWrite::Deleted);
        assert!(!path.exists(), "清空必须删文件，不能留 0 字节");
        assert_eq!(load_instructions_from(&path), "");

        // 本来就不在 ⇒ NoChange，且不误创文件
        assert_eq!(write_instructions_to(&path, "").unwrap(), InstructionsWrite::NoChange);
        assert!(!path.exists(), "空写不许把文件造出来");

        // 目录不存在时写入 ⇒ 自建（主干 `SaveInstructions:296` 的 CreateDirectory）
        let deep = dir.join("sub").join("x").join(AGENTS_MD_FILE);
        assert_eq!(write_instructions_to(&deep, "正文").unwrap(), InstructionsWrite::Written);
        assert_eq!(load_instructions_from(&deep), "正文");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 读端 `Err ⇒ ""`（含缺文件与权限异常），对齐主干 catch。
    #[test]
    fn parse_instructions_swallows_every_read_error() {
        assert_eq!(parse_instructions(Ok("  a\r\n".into())), "a");
        assert_eq!(parse_instructions(Err(std::io::Error::other("denied"))), "");
        assert_eq!(parse_instructions(Err(std::io::Error::new(
            std::io::ErrorKind::NotFound, "缺文件"))), "");
        let dir = sandbox("agents-read");
        assert_eq!(load_instructions_from(&dir.join("nope.md")), "");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 4000 上限截断：按 char 边界退，绝不切断多字节序列。
    #[test]
    fn truncate_instructions_stays_on_char_boundaries() {
        assert_eq!(truncate_instructions("短"), "短");
        let exact = "a".repeat(INSTRUCTIONS_MAX_CHARS);
        assert_eq!(truncate_instructions(&exact), exact);
        let over = "a".repeat(INSTRUCTIONS_MAX_CHARS + 10);
        assert_eq!(truncate_instructions(&over).chars().count(), INSTRUCTIONS_MAX_CHARS);
        // 中文/emoji 混排：截点必须落在合法边界上（切断会在 `unwrap` 处炸）
        let mixed = "喵".repeat(INSTRUCTIONS_MAX_CHARS + 5);
        assert_eq!(truncate_instructions(&mixed).chars().count(), INSTRUCTIONS_MAX_CHARS);
        assert!(truncate_instructions(&mixed).is_char_boundary(truncate_instructions(&mixed).len()));
        let emoji = "😀".repeat(INSTRUCTIONS_MAX_CHARS + 2);
        assert_eq!(truncate_instructions(&emoji).chars().count(), INSTRUCTIONS_MAX_CHARS);
        // ⚠ 单位差异备案：主干数 UTF-16 码元，一个 `😀` 算 2；这里算 1 ⇒ 边界上分叉多容一个。
        assert_eq!("😀".encode_utf16().count(), 2);
    }

    // ---------- 4. pet-window.json ----------

    /// 逐字节等于实测 18 B 真产物（hexdump 字面量焊进断言）。
    #[test]
    fn render_pet_window_json_is_the_measured_18_bytes() {
        let rendered = render_pet_window_json(-543, 974);
        assert_eq!(rendered, REAL_PET_WINDOW_JSON);
        assert_eq!(rendered.len(), 18);
        assert_eq!(
            rendered.as_bytes(),
            &[
                0x7b_u8, 0x22, 0x58, 0x22, 0x3a, 0x2d, 0x35, 0x34, 0x33, 0x2c, 0x22, 0x59, 0x22,
                0x3a, 0x39, 0x37, 0x34, 0x7d
            ][..]
        );
        assert!(!rendered.contains(' '), "紧凑：冒号后无空格");
        assert!(!rendered.contains('\n') && !rendered.contains('\r'));
        assert!(!rendered.starts_with('\u{feff}'), "无 BOM");
        // PascalCase 键、X 在 Y 前
        assert!(rendered.starts_with("{\"X\":"), "{rendered}");
        assert_eq!(parse_pet_window_json(REAL_PET_WINDOW_JSON), Some((-543, 974)));
    }

    /// int 直写：0 / 负 / i32 极值都是主干会写的那串。
    #[test]
    fn render_pet_window_json_writes_ints_verbatim() {
        assert_eq!(render_pet_window_json(0, 0), "{\"X\":0,\"Y\":0}");
        assert_eq!(render_pet_window_json(-1, -2147483648), "{\"X\":-1,\"Y\":-2147483648}");
        assert_eq!(render_pet_window_json(2147483647, 1), "{\"X\":2147483647,\"Y\":1}");
        assert_eq!(parse_pet_window_json(&render_pet_window_json(-1, -2147483648)), Some((-1, -2147483648)));
        assert_eq!(parse_pet_window_json(&render_pet_window_json(2147483647, 1)), Some((2147483647, 1)));
    }

    /// 三条真失败面 ⇒ `None`（主干 `catch` → `MoveToDefault`）。
    #[test]
    fn parse_pet_window_json_rejects_broken_or_out_of_range() {
        for broken in ["", "[", "{", "{}x", "null", "5", "\"str\"", "{\"X\":1"] {
            assert_eq!(parse_pet_window_json(broken), None, "坏输入: {broken:?}");
        }
        // 超 i32 ⇒ 主干 Int32 转换抛异常 ⇒ None
        assert_eq!(parse_pet_window_json("{\"X\":99999999999,\"Y\":0}"), None);
        assert_eq!(parse_pet_window_json("{\"X\":0,\"Y\":-2147483649}"), None);
        // 类型不对（串 / 布尔 / null / 带小数）⇒ None
        assert_eq!(parse_pet_window_json("{\"X\":\"1\",\"Y\":0}"), None);
        assert_eq!(parse_pet_window_json("{\"X\":true,\"Y\":0}"), None);
        assert_eq!(parse_pet_window_json("{\"X\":null,\"Y\":0}"), None);
        assert_eq!(parse_pet_window_json("{\"X\":1.5,\"Y\":0}"), None);
        assert_eq!(parse_pet_window_json("{\"X\":1.0,\"Y\":0}"), None, "主干 Int32 也不吃 1.0");
    }

    /// **大小写敏感**这件事的真实后果：匹配不到的构造参数取 `0`，不是抛异常 ⇒ `(0, 0)`。
    /// （SD3 §4.5 断言 camel ⇒ None，那是把 STJ 的构造参数默认值当成了失败，见报告 §9。）
    #[test]
    fn parse_pet_window_json_is_pascal_case_sensitive_and_defaults_missing_keys_to_zero() {
        assert_eq!(parse_pet_window_json("{\"x\":1,\"y\":2}"), Some((0, 0)), "小写键匹配不到 ⇒ 取 default(int)=0");
        assert_eq!(parse_pet_window_json("{}"), Some((0, 0)), "空 object 同理");
        assert_eq!(parse_pet_window_json("{\"X\":7}"), Some((7, 0)), "只缺 Y 时 Y 取 0");
        assert_eq!(parse_pet_window_json("{\"Y\":7}"), Some((0, 7)));
        assert_eq!(parse_pet_window_json("{\"ShowTrayIcon\":1}"), Some((0, 0)));
    }

    /// `MoveToDefault` 算术：右下减 size 再减 48；屏宽/高 ≤0 直接不动。
    #[test]
    fn pet_move_to_default_is_the_bottom_right_formula() {
        assert_eq!(pet_move_to_default(0, 0, 1920, 1080, 200, 240), Some((1672, 792)));
        // 多屏（虚拟屏原点非 0）
        assert_eq!(pet_move_to_default(-1920, 0, 3840, 1080, 200, 240), Some((1672, 792)));
        // 尺寸 0 走 `Math.Max(size, 1)`
        assert_eq!(pet_move_to_default(0, 0, 1920, 1080, 0, 0), Some((1871, 1031)));
        assert_eq!(pet_move_to_default(0, 0, 1920, 1080, -50, -50), Some((1871, 1031)));
        // vw/vh <= 0 ⇒ None（主干 `return`，连 MoveTo 都不调）
        assert_eq!(pet_move_to_default(0, 0, 0, 1080, 200, 240), None);
        assert_eq!(pet_move_to_default(0, 0, 1920, 0, 200, 240), None);
        assert_eq!(pet_move_to_default(0, 0, -1920, -1080, 200, 240), None);
    }

    /// 真落盘往返（临时目录）。本片**不接线**，这条只保证纯函数与盘对齐。
    #[test]
    fn pet_window_disk_round_trip_and_byte_shape() {
        let dir = sandbox("pet-window");
        let path = data_home_from(Some(&dir)).join(PET_WINDOW_FILE);
        assert_eq!(load_pet_window_position_from(&path), None, "缺文件 ⇒ None ⇒ 默认位");
        save_pet_window_to(-543, 974, &path).expect("落盘");
        let bytes = std::fs::read(&path).expect("读回");
        assert_eq!(bytes, REAL_PET_WINDOW_JSON.as_bytes());
        assert_eq!(bytes.len(), 18);
        assert_eq!(load_pet_window_position_from(&path), Some((-543, 974)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---------- 5. shell-skin.we ----------

    /// 真产物那一把尺：17 B、逐字节、**末位是 `32` 那个 `2` 而不是换行**。
    #[test]
    fn real_skin_we_artifact_is_17_bytes_with_no_trailing_newline() {
        assert_eq!(REAL_SKIN_WE_ID.len(), 17, "真产物基线自己先钉住");
        assert_eq!(
            render_skin_we_id(REAL_SKIN_WE_ID).as_bytes(),
            [
                0x76, 0x69, 0x64, 0x65, 0x6f, 0x5f, 0x5f, 0x33, 0x33, 0x37, 0x33, 0x36, 0x39, 0x39,
                0x34, 0x38, 0x32,
            ],
            "逐字节 = od -c 的真机输出"
        );
        assert_eq!(*REAL_SKIN_WE_ID.as_bytes().last().unwrap(), 0x32, "无尾换行");
        assert!(!REAL_SKIN_WE_ID.contains('\n') && !REAL_SKIN_WE_ID.contains('\r'));
        // 往返：干净的 id 原样回
        assert_eq!(parse_skin_we_id(REAL_SKIN_WE_ID), Some(REAL_SKIN_WE_ID));
    }

    /// 渲染是**恒等**：不 Trim、不补换行、不转义。反证「看着更讲究」的三种改写都会红。
    #[test]
    fn render_skin_we_id_is_identity_not_a_polite_normalizer() {
        assert_eq!(render_skin_we_id(""), "");
        // 恒等 ⇒ 字节序列与入参逐字相同（含 CJK、含首尾空白、含尾随换行）
        for probe in ["", "   ", "  留白  ", "a\r\n", "a\n", "a\\b", "\u{1f300}\u{1f300}", REAL_SKIN_WE_ID] {
            assert_eq!(render_skin_we_id(probe).as_bytes(), probe.as_bytes(), "恒等：不改一个字节");
            assert_eq!(render_skin_we_id(probe).len(), probe.len(), "恒等：长度不膨胀");
        }
        assert_eq!(render_skin_we_id("a\\b").as_bytes(), b"a\\b", "裸文本：反斜杠不转义");
    }

    /// 读端的三条空态臂都通向 `None`，外加 Ordinal 大小写敏感。
    #[test]
    fn parse_skin_we_id_three_empty_arms_and_ordinal_case() {
        assert_eq!(parse_skin_we_id(""), None);
        assert_eq!(parse_skin_we_id("   "), None);
        assert_eq!(parse_skin_we_id("\r\n \t "), None, "纯空白被 Trim 抹成空 ⇒ None");
        assert_eq!(parse_skin_we_id("  video__1  "), Some("video__1"), "首尾空白被抹掉（主干 Trim）");
        assert_eq!(parse_skin_we_id("Video__1"), Some("Video__1"), "不折叠大小写：比较侧是 Ordinal");
        // ⚠ 往返不对称是**主干自身**的行为，不是本函数的 bug：脏 id 写得进、读不回。
        assert_eq!(parse_skin_we_id(render_skin_we_id("  a  ")), Some("a"));
        assert_ne!(parse_skin_we_id(render_skin_we_id("  a  ")).unwrap(), "  a  ");
    }

    /// 真落盘往返 + 「清空 = 删文件」而不是写空串。
    #[test]
    fn skin_we_disk_round_trip_and_clear_removes_the_file() {
        let dir = sandbox("skin-we");
        let path = data_home_from(Some(&dir)).join(SKIN_WE_FILE);
        // 空态：文件根本不存在（这是本类的默认态）；「删一个不存在的文件」= NoChange
        assert_eq!(load_skin_we_id_from(&path), None);
        assert_eq!(clear_skin_we_id_from(&path).expect("删不存在"), SkinWeClear::NoChange);
        // 写入 ⇒ 字节 = 真产物那 17 B
        save_skin_we_id_to(REAL_SKIN_WE_ID, &path).expect("落盘");
        assert_eq!(std::fs::read(&path).expect("读回"), REAL_SKIN_WE_ID.as_bytes());
        assert_eq!(std::fs::read(&path).unwrap().len(), 17);
        assert_eq!(load_skin_we_id_from(&path).as_deref(), Some(REAL_SKIN_WE_ID));
        // 0 字节文件 ⇒ 读回 None（第 ② 条空态臂）
        std::fs::write(&path, b"").expect("写空");
        assert_eq!(load_skin_we_id_from(&path), None);
        assert!(path.exists(), "写空串留的是 0 字节文件 —— 本类不这么清空");
        // 清空 = 删
        assert_eq!(clear_skin_we_id_from(&path).expect("删"), SkinWeClear::Deleted);
        assert!(!path.exists(), "删完必须没有这个文件");
        assert_eq!(clear_skin_we_id_from(&path).expect("再删"), SkinWeClear::NoChange);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---------- 6. shell-toast.sender ----------

    /// 真产物那一把尺：32 B、行内**一个** `0a`、**零个** `0d`、无 BOM、无尾换行。
    #[test]
    fn real_sender_stamp_is_32_bytes_one_lf_zero_cr() {
        assert_eq!(REAL_SENDER_STAMP.len(), 32, "真产物基线自己先钉住");
        assert_eq!(REAL_SENDER_IDENTITY.len(), 24);
        assert_eq!(TOAST_SENDER_NAME.as_bytes(), [0x42, 0x6c, 0x61, 0x64, 0x65, 0xc2, 0xb2]);
        assert_eq!(TOAST_SENDER_NAME.len(), 7, "U+00B2 在 UTF-8 里占 2 字节");
        let bytes = render_sender_stamp(REAL_SENDER_IDENTITY, TOAST_SENDER_NAME);
        assert_eq!(bytes.as_bytes(), REAL_SENDER_STAMP.as_bytes(), "渲染器 = 真机产物");
        assert_eq!(bytes.as_bytes()[24], 0x0a, "第 25 字节就是那个行内 LF");
        assert_eq!(bytes.matches('\n').count(), 1);
        assert_eq!(bytes.matches('\r').count(), 0, "一个 0d 都不许有");
        assert_eq!(*bytes.as_bytes().last().unwrap(), 0xb2, "末字节是 U+00B2 的后半 ⇒ 无尾换行");
        assert!(!bytes.starts_with('\u{feff}'), "UTF-8 无 BOM");
    }

    /// 分隔符常量本身：`char` 且 `len_utf8() == 1`。写成 `"\r\n"` 立刻红。
    #[test]
    fn sender_separator_constant_is_a_single_lf_char() {
        assert_eq!(TOAST_SENDER_SEPARATOR, '\n');
        assert_eq!(TOAST_SENDER_SEPARATOR.len_utf8(), 1);
        assert_eq!(TOAST_SENDER_SEPARATOR as u32, 0x0a);
        // 渲染结果对分隔符唯一：换个名字段也不会多出别的换行
        assert_eq!(render_sender_stamp("a", "b").as_bytes(), b"a\nb");
    }

    /// 三种「看着更合理」的写法**全部**判过期 —— 因为主干 `:394` 是 `==` 不 Trim。
    /// CRLF 版尤其阴：它是 Windows 上最自然的写法，也是七类里唯一会坏在这里的一类。
    #[test]
    fn sender_stamp_rejects_the_three_plausible_wrong_shapes() {
        let crlf = REAL_SENDER_STAMP.replace('\n', "\r\n");
        let trailing_lf = format!("{}\n", REAL_SENDER_STAMP);
        let trailing_crlf = format!("{}\r\n", REAL_SENDER_STAMP);
        let ascii_two = REAL_SENDER_STAMP.replace(TOAST_SENDER_NAME, "Blade2");
        let spaced = REAL_SENDER_STAMP.replace('\n', " ");
        assert_eq!(crlf.len(), 33);
        assert!(!sender_stamp_matches(&crlf, REAL_SENDER_IDENTITY, TOAST_SENDER_NAME), "CRLF ⇒ 每次启动白跑 UnregisterAll");
        assert!(!sender_stamp_matches(&trailing_lf, REAL_SENDER_IDENTITY, TOAST_SENDER_NAME));
        assert!(!sender_stamp_matches(&trailing_crlf, REAL_SENDER_IDENTITY, TOAST_SENDER_NAME));
        assert!(!sender_stamp_matches(&ascii_two, REAL_SENDER_IDENTITY, TOAST_SENDER_NAME), "把上标二写成 ASCII 2 ⇒ 少 1 字节");
        assert!(!sender_stamp_matches(&spaced, REAL_SENDER_IDENTITY, TOAST_SENDER_NAME));
        assert!(sender_stamp_matches(REAL_SENDER_STAMP, REAL_SENDER_IDENTITY, TOAST_SENDER_NAME));
        // 也不许 Trim：首尾空白同样是不等
        assert!(!sender_stamp_matches(&format!(" {REAL_SENDER_STAMP} "), REAL_SENDER_IDENTITY, TOAST_SENDER_NAME));
    }

    /// 反直觉的一臂：带 BOM **仍然相等**。这不是宽容，是复刻 `File.ReadAllText` 的
    /// `detectEncodingFromByteOrderMarks`。分叉若用裸 `read_to_string` 比，会在这一格
    /// 比主干多跑一次 `UnregisterAll`。
    #[test]
    fn sender_stamp_matches_replicates_dotnet_bom_sniffing() {
        let with_bom = format!("\u{feff}{REAL_SENDER_STAMP}");
        assert!(with_bom.as_bytes().starts_with(&[0xef, 0xbb, 0xbf]));
        assert!(sender_stamp_matches(&with_bom, REAL_SENDER_IDENTITY, TOAST_SENDER_NAME));
        // 但 BOM 只许在开头：中间一个就是不等
        assert!(!sender_stamp_matches(
            &REAL_SENDER_STAMP.replace("Blade", "B\u{feff}lade"),
            REAL_SENDER_IDENTITY,
            TOAST_SENDER_NAME
        ));
        // 渲染侧永远不产 BOM ⇒ 这条只影响脏盘
        assert!(!render_sender_stamp(REAL_SENDER_IDENTITY, TOAST_SENDER_NAME).contains('\u{feff}'));
    }

    /// 身份两臂（`pkg:` / `exe:`），以及它怎么进 stamp。
    #[test]
    fn notification_identity_has_the_two_mainline_arms() {
        assert_eq!(
            notification_identity(Some("Blade2_wr2brkarxqkxy"), "C:/ignored/blade2.exe"),
            REAL_SENDER_IDENTITY
        );
        assert_eq!(
            notification_identity(None, "C:/Users/Admin/blade2-rs.exe"),
            "exe:C:/Users/Admin/blade2-rs.exe"
        );
        assert_eq!(notification_identity(Some(""), "x"), "pkg:", "刻意不判空：主干也不判");
        assert_eq!(
            render_sender_stamp(&notification_identity(None, "p"), TOAST_SENDER_NAME).as_bytes(),
            b"exe:p\nBlade\xc2\xb2"
        );
    }

    /// 真落盘往返 + 读端三臂（不等 / 缺文件都算过期）。
    #[test]
    fn toast_sender_disk_round_trip_and_missing_file_is_stale() {
        let dir = sandbox("toast-sender");
        let path = data_home_from(Some(&dir)).join(TOAST_SENDER_FILE);
        assert!(!sender_stamp_is_fresh_at(&path, REAL_SENDER_IDENTITY, TOAST_SENDER_NAME),
                "缺文件 = 主干 File.Exists 不成立 ⇒ 过期");
        save_sender_stamp_to(REAL_SENDER_IDENTITY, TOAST_SENDER_NAME, &path).expect("落盘");
        assert_eq!(std::fs::read(&path).unwrap(), REAL_SENDER_STAMP.as_bytes());
        assert_eq!(std::fs::read(&path).unwrap().len(), 32);
        assert!(sender_stamp_is_fresh_at(&path, REAL_SENDER_IDENTITY, TOAST_SENDER_NAME));
        // 换包身份 ⇒ 过期（这正是把身份写进标记的理由，主干 `:337-338` 注释）
        assert!(!sender_stamp_is_fresh_at(&path, "exe:C:/other/blade2.exe", TOAST_SENDER_NAME));
        // 盘上被写成 CRLF ⇒ 过期
        std::fs::write(&path, REAL_SENDER_STAMP.replace('\n', "\r\n")).expect("写脏");
        assert!(!sender_stamp_is_fresh_at(&path, REAL_SENDER_IDENTITY, TOAST_SENDER_NAME));
        // 重写回正确字节 ⇒ 又新鲜（RememberSenderName 每次注册成功后都会这么干）
        save_sender_stamp_to(REAL_SENDER_IDENTITY, TOAST_SENDER_NAME, &path).expect("再落盘");
        assert!(sender_stamp_is_fresh_at(&path, REAL_SENDER_IDENTITY, TOAST_SENDER_NAME));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---------- 7. 路径链与自我锁 ----------

    /// 路径链 = `LOCALAPPDATA → data_home_root → join(DATA_HOME_DIR)`，与 `layout::prefs_path`
    /// 同一条；兜底 `./Blade2`。
    #[test]
    fn data_home_path_chain_matches_the_layout_precedent() {
        let from_drive = data_home_from(Some(Path::new("C:/fake/LOCALAPPDATA")));
        assert_eq!(from_drive.to_string_lossy().replace('\\', "/"), "C:/fake/LOCALAPPDATA/Blade2");
        // 兜底与 kernel::data_home_root 一致
        assert_eq!(data_home_root(None), PathBuf::from("."));
        assert_eq!(data_home_from(None).to_string_lossy().replace('\\', "/"), "./Blade2");
        // 同一条链的产物：本模块与 layout.rs 的数据家必须是同一个目录
        assert_eq!(data_home(), crate::layout::prefs_path().parent().unwrap().to_path_buf());
    }

    /// 七类文件名逐字钉死（主干拼错一个字就读不到用户已有的文件）。
    #[test]
    fn shell_file_names_are_the_mainline_literals() {
        for (path, expected) in [
            (shell_json_path(), SHELL_JSON_FILE),
            (skin_opacity_path(), SKIN_OPACITY_FILE),
            (skin_video_pause_path(), SKIN_VIDEO_PAUSE_FILE),
            (agents_path(), AGENTS_MD_FILE),
            (pet_window_path(), PET_WINDOW_FILE),
            (skin_we_path(), SKIN_WE_FILE),
            (toast_sender_path(), TOAST_SENDER_FILE),
        ] {
            assert_eq!(expected, path.file_name().unwrap().to_str().unwrap());
            assert_eq!(path.parent().unwrap(), &data_home());
        }
        assert_eq!(SHELL_JSON_FILE, "shell.json");
        assert_eq!(SKIN_OPACITY_FILE, "shell-skin.opacity");
        assert_eq!(SKIN_VIDEO_PAUSE_FILE, "shell-skin.videopause");
        assert_eq!(AGENTS_MD_FILE, "AGENTS.md");
        assert_eq!(PET_WINDOW_FILE, "pet-window.json");
        assert_eq!(SKIN_WE_FILE, "shell-skin.we");
        assert_eq!(TOAST_SENDER_FILE, "shell-toast.sender");
        // 第五类归 #90（`layout.rs`），本模块不重写；串不同 ⇒ 不会互相覆盖
        assert_eq!(crate::layout::PREFS_FILE_NAME, "layout-columns.json");
        assert_ne!(crate::layout::PREFS_FILE_NAME, SHELL_JSON_FILE);
        // 七颗互不相同（漏项的根因就是「数过一遍但没逐颗对过落点」）
        let names = [
            SHELL_JSON_FILE, SKIN_OPACITY_FILE, SKIN_VIDEO_PAUSE_FILE, AGENTS_MD_FILE,
            PET_WINDOW_FILE, SKIN_WE_FILE, TOAST_SENDER_FILE,
        ];
        for (i, a) in names.iter().enumerate() {
            for b in names.iter().skip(i + 1) {
                assert_ne!(a, b, "两颗壳本地文件撞名");
            }
        }
        // `shell-skin.we` 与 `.opacity` / `.videopause` 同前缀但不同名 ⇒ 不会互相覆盖
        assert!(SKIN_WE_FILE.starts_with("shell-skin."));
        assert_ne!(SKIN_WE_FILE, SKIN_OPACITY_FILE);
    }

    /// **自我源码锁**（本仓 `include_str!` 口径）：本模块的落盘代码里
    /// ① 不许出现 `{:.2}`（会写出 `0.50` 破坏字节）② 不许出现 `json!(`（BTreeMap 字母序）
    /// ③ 数据家只许有 `LOCALAPPDATA` 这一颗 env 口，不许走分叉私有覆盖口，**且必须**过
    ///    kernel 那颗 `Code2 → Blade2` 搬迁口（#145 的搬迁半就钉在这一枚上）
    /// ④ `shell-toast.sender` 那一节不许复用模块那颗 `0d 0a` 常量、也不许出现 CRLF 转义。
    /// 被搜的串一律 `concat!` 拆开，否则 `include_str!` 会把搜索串本身读进来自匹配。
    #[test]
    fn this_module_source_locks_the_renderers_and_the_path_chain() {
        let source = include_str!("shellfiles.rs");
        let test_mark = concat!("#[cfg(", "test)]");
        let code = &source[..source.find(test_mark).expect("找不到测试模块边界")];

        assert!(
            !code.contains(concat!("{:.", "2}")),
            concat!("禁定长两位小数格式化串：.opacity 与 bubbleOpacity 的字节都会歪（0.50 / 0.60）")
        );
        assert!(!code.contains(concat!("json", "!(")), "禁 json!：serde_json::Map 没有 preserve_order，键按字母序");
        assert!(!code.contains(concat!("BLADE2_", "DSH_HOME")), "禁分叉私有 env 覆盖：会把文件落到主干不会落的地方");
        assert!(!code.contains(concat!("resolve_", "dsh_home")), "禁 kernel 那颗带 env 覆盖的解析口");
        assert!(
            code.contains(concat!("migrate_", "data_home")),
            "数据家这条链绕开了 kernel 的搬迁口 ⇒ 主干那次 Code2→Blade2 改名又掉了（#145 搬迁半）"
        );
        assert_eq!(
            code.matches("env::var").count(),
            1,
            "数据家解析只许有一颗 env 口（LOCALAPPDATA）"
        );
        assert_eq!(code.matches(concat!("\"LOCALAPP", "DATA\"")).count(), 1);
        assert!(code.contains(concat!("use crate::kernel::{data_home_root, DATA_HOME", "_DIR};")));

        // ④ sender 一节单独锁：它的行内分隔符是裸 LF，用错行尾 = 每次启动白跑 UnregisterAll。
        let sender_head = concat!("6. shell-", "toast.sender");
        let sender_region = &code[code.find(sender_head).expect("找不到 sender 节边界")..];
        assert!(
            !sender_region.contains(concat!("\\r", "\\n")),
            "sender 一节不许出现 CRLF 转义：主干 `:339` 写的是字面量单 LF"
        );
        assert!(
            !sender_region.contains("NEWLINE"),
            "sender 一节不许复用模块那颗 0d 0a 常量（它是 AGENTS.md 的行尾，不是这里的）"
        );
        assert!(
            sender_region.contains(concat!("push(", "TOAST_SENDER_SEPARATOR)")),
            "sender 的分母必须走那颗分隔符常量，别在函数里就地写字面量"
        );
        // 恒等渲染器不许偷偷加工：`render_skin_we_id` 的函数体只许是原样返回。
        let we_head = concat!("5. shell-", "skin.we =====");
        let we_region = &code[code.find(we_head).expect("找不到 we 节边界")..code.find(sender_head).unwrap()];
        assert!(
            we_region.contains(concat!("fn render_skin_we_id(id: &str) -> &str {", "\n    id\n}")),
            "本类的形状就是恒等：一旦函数体长出别的事，就是有人替主干加了 Trim 或补了换行"
        );
    }

    /// 纯函数不许有任何副作用：全部函数在临时目录之外**不落一个字节**。
    /// 这条测用「真数据家目录的字节快照」来兜底 —— 只读快照、只比 mtime 个数，不写不删。
    #[test]
    fn pure_functions_touch_nothing_but_the_sandbox() {
        let dir = sandbox("purity");
        let home = data_home_from(Some(&dir));
        assert!(!home.exists(), "纯函数（render_/parse_）不该建任何目录");
        let _ = render_shell_json(&ShellOptions::default());
        let _ = parse_shell_json("{}");
        let _ = render_skin_opacity(45.0);
        let _ = parse_skin_opacity("0.45");
        let _ = render_skin_video_pause(false);
        let _ = render_instructions("喵");
        let _ = render_pet_window_json(1, 2);
        let _ = render_skin_we_id(REAL_SKIN_WE_ID);
        let _ = parse_skin_we_id(REAL_SKIN_WE_ID);
        let _ = notification_identity(Some("x"), "p");
        let _ = render_sender_stamp(REAL_SENDER_IDENTITY, TOAST_SENDER_NAME);
        let _ = sender_stamp_matches(REAL_SENDER_STAMP, REAL_SENDER_IDENTITY, TOAST_SENDER_NAME);
        assert!(!home.exists(), "render_/parse_ 一律不许碰盘");
        assert_eq!(std::fs::read_dir(&dir).err().map(|e| e.kind()),
                   Some(std::io::ErrorKind::NotFound));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
