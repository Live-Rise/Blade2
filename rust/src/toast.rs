//! 系统通知原语（**TT2 第 1 把刀**，台账 #164）：托盘气球，走 `shell32!Shell_NotifyIconW` 裸 FFI、零新依赖。
//!
//! 口径 = 主干 `ShellToast` 的**退化臂**（`SI:` = `MainWindow.ShellIntegration.cs`，本轮逐字现测）：
//! - 正臂 `SI:482` `internal static void Show(string title, string body, string? action = null)` →
//!   `SI:502 AppNotificationManager.Default.Show(notification)`，**依赖 MSIX 包身份**（`SI:307-309`）。分叉零包身份、
//!   零 Windows App SDK，且 `Cargo.toml` 不许动 ⇒ 本刀不碰正臂。
//! - 无包身份时主干自己退化成托盘气球：`SI:489 BalloonFallback?.Invoke(title, body); // dev 无包身份：退化托盘气球`。
//!   挂钩签名 `SI:328 internal static Action<string, string>? BalloonFallback;` ⇒ **气球只带 title+body，丢掉 action**。
//!   实臂 = `SI:170-201 ShowTrayBalloon`：手写 `NOTIFYICONDATAW`（`SI:285-302`，`guidItem :300`、`hBalloonIcon :301`）
//!   + `uFlags = 0x1 | 0x10 // NIF_MESSAGE | NIF_INFO`（`SI:193`）+ `Shell_NotifyIcon(0x01 /* NIM_MODIFY */)`（`SI:200`）。
//! - 入口声明 `SI:251-252`：`[DllImport("shell32.dll", CharSet = CharSet.Unicode, SetLastError = true)]`
//!   ⇒ Unicode 实体 = `Shell_NotifyIconW`。本模块照 `clipboard.rs` 那条成规自己声明（`#[link]` 一块，零新依赖）。
//!
//! **主干没有的东西，本刀一律不自造**：无队列、无去重、无自定义时长（`SI:482-508` 全文 27 行里 `Duration` /
//! `Audio` / `AddButton` 零命中，并存策略归 OS ⇒ 多做 = 缺陷）。`action` 是**点击激活参数、不是按钮**
//! （`SI:499`，全仓只 `"update"` 那一发用它），且气球这一臂**拿不到**它。
//!
//! ⚠ **验收前提（用户明令，本轮逐字遵守）**：**全程没有真发过一次通知**（不发 toast、不发气球、不置顶、
//! 不 SendInput、零前台验证）。所以模块切成两半：
//! - **纯逻辑半** = [`should_notify`]（三腿准入，逐字照 `MW:7897-7898`）与 [`Toast`]（24 字门 `UQ:104-116`
//!   + `TruncateForBalloon` 的 64/256 定长门 `SI:203-206`）⇒ 由本文件 `#[cfg(test)]` 的真值表逐档覆盖。
//! - **真 FFI 半** = [`show`] 里那一发 `Shell_NotifyIconW` ⇒ **任何测试都不执行它**，形制由
//!   `this_module_locks_the_balloon_shape` 与 `the_struct_mirrors_the_mainline_field_order` 两枚**源码锁**钉住。
//!   ⇒ **实机通知腿待用户单独点头**；UIA 尺对这枚原语天然失效（主干那枚根本不在本进程视觉树内，`tt1 §1.7`）。
//!
//! 主干 11 发调用面**一颗都不接**（本刀只交原语）；`src/lib.rs` 的 mod 表**不动**（先例 = `clipboard.rs`
//! 住 bin 侧私有 mod）。零新 i18n 键（`tt1 §2` 已判净新增 = 0），所以本模块不引 catalog / 渲染层类型
//! （照 `dock.rs` 那条「纯逻辑层不引渲染层」的成规，见 `this_module_locks_the_balloon_shape` 的反锁段）。

use std::borrow::Cow;
use std::ffi::c_void;

// 主干 `SI:251-252` 那颗 `[DllImport("shell32.dll", CharSet = Unicode)] Shell_NotifyIcon` ⇒ `Shell_NotifyIconW`。
// 返回 `BOOL`（非零 = OS 收了这条消息），所以调用方按返回值判成败、不许只看「没崩」。
// （这里用 `//` 不用 `///`：rustc 不给 extern 块生成文档，`///` 会叫 `unused_doc_comments` 判一条警告。）
#[link(name = "shell32")]
unsafe extern "system" {
    fn Shell_NotifyIconW(message: u32, data: *const NotifyIconDataW) -> i32;
}

// ------------------------------------------------------- 消息号与旗（逐字取主干现值）

/// `SI:200`：`Shell_NotifyIcon(0x01 /* NIM_MODIFY */, ref nid)` ⇒ 气球只有 MODIFY 一档。
/// `NIM_ADD`（`SI:125`）与 `NIM_DELETE`（`SI:143`）属托盘那一族，本模块**不发**（见 `§6` 半截件登记）。
const NIM_MODIFY: u32 = 0x01;
/// `SI:193` `uFlags = 0x1 | 0x10`。
const NIF_MESSAGE: u32 = 0x1;
const NIF_INFO: u32 = 0x10;
/// `SI:197` `dwInfoFlags = 0x1, // NIIF_INFO`。
const NIIF_INFO: u32 = 0x1;
/// `SI:22`：「托盘回调消息：WM_APP 段壳内无其它占用者，取首号」= `WM_TRAYCALLBACK`。
const WM_TRAY_CALLBACK: u32 = 0x0400 + 1;
/// `SI:117` / `SI:191`：托盘条目号（与 `TrayAdd` 同一颗）。
const TRAY_UID: u32 = 1;
/// 三枚 `ByValTStr` 缓冲区的 `SizeConst`（`SI:292` `szTip` 128 / `SI:295` `szInfo` 256 /
/// `SI:298` `szInfoTitle` 64），单位 = UTF-16 码元。
const SZ_TIP_CHARS: usize = 128;
const SZ_INFO_CHARS: usize = 256;
const SZ_INFO_TITLE_CHARS: usize = 64;
/// `UQ:108` 那枚 24 字门（**上界**，不是下界）：`headerText.Length <= 24`。
const TITLE_MAX_UNITS: usize = 24;
/// `UQ:114` 长 header 折进正文时的连接符，逐字 `headerText + "\n" + body`。
const BODY_JOIN: &str = "\n";
/// `SI:206` 截断补的那颗省略号（主干用它替代「半字处断开」）。
const BALLOON_ELLIPSIS: char = '…';

// ------------------------------------------------------------------------ FFI 形制

/// 与主干 `SI:285-302` 那颗手写 `NOTIFYICONDATAW` **同序同数**（`#[repr(C)]` 的填充与
/// `Marshal.SizeOf<NOTIFYICONDATAW>()` 同值 ⇒ 发出去的 `cbSize` 逐字等于主干那枚）。
/// 主干那份只声明到 `hBalloonIcon`（`SI:301`），本模块一字不加。
/// `Debug` 只为让「整颗结构体是 FFI 载荷」这件事在死码分析里可读，不参与任何逻辑。
/// **不 derive `Default`**：core 只给到 32 格的数组实现，`[u16; 128]` 拿不到 ⇒ 投递那一段把 15 颗字段
/// 逐颗写全（这也更贴主干 `SI:188-198` 那颗对象初始化器的形：写了 9 颗、其余按零值）。
#[repr(C)]
#[derive(Debug)]
struct NotifyIconDataW {
    cb_size: u32,
    hwnd: *mut c_void,
    u_id: u32,
    u_flags: u32,
    u_callback_message: u32,
    h_icon: *mut c_void,
    sz_tip: [u16; SZ_TIP_CHARS],
    dw_state: u32,
    dw_state_mask: u32,
    sz_info: [u16; SZ_INFO_CHARS],
    u_timeout_or_version: u32,
    sz_info_title: [u16; SZ_INFO_TITLE_CHARS],
    dw_info_flags: u32,
    /// 主干 `SI:300 public Guid guidItem;`（16 字节）。气球留零值 ⇒ 这里用同尺寸的 `[u8; 16]`，
    /// 不为布局保真再声明一颗没人读的 `Guid`。
    guid_item: [u8; 16],
    /// 主干 `SI:301 public IntPtr hBalloonIcon;`。
    h_balloon_icon: *mut c_void,
}

// ------------------------------------------------------------------ 纯逻辑：载荷组装

/// 载荷 = 主干 `Show` 的三枚参数（`SI:482`）。`action` 是**点击激活参数**（`SI:499 AddArgument("action", action)`），
/// 不是按钮 ⇒ 气球臂按 `SI:328` 的挂钩签名丢掉它。形状留着是为了将来换正臂后端时调用点一行不动（`tt1 §5`）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast<'a> {
    pub title: Cow<'a, str>,
    pub body: Cow<'a, str>,
    pub action: Option<&'a str>,
}

/// 气球的两枚定长字段（`szInfoTitle` / `szInfo`），已过 [`truncate_for_balloon`]。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BalloonFields {
    pub info_title: String,
    pub info: String,
}

impl<'a> Toast<'a> {
    /// 标题位只走短标签（`UQ:102-103` 逐字：长 header 进标题「会在 toast 标题位被裁成半行」）：
    /// `UQ:106` header 非空才动 ⇒ `:108` **<= 24 码元**进标题（`:110`）/ 否则折进正文
    /// （`:114` `body.Length > 0 ? header + "\n" + body : header`）。
    /// 尺 = UTF-16 码元数（与 C# `string.Length` 同尺），**不是** `char` 数。
    pub fn with_header(self, header: &'a str) -> Toast<'a> {
        if header.is_empty() {
            return self;
        }
        if header.encode_utf16().count() <= TITLE_MAX_UNITS {
            return Toast { title: Cow::Borrowed(header), body: self.body, action: self.action };
        }
        let joined = if self.body.is_empty() {
            header.to_string()
        } else {
            format!("{header}{BODY_JOIN}{}", self.body)
        };
        Toast { title: self.title, body: Cow::Owned(joined), action: self.action }
    }

    /// 气球位定长钳制：标题走 64、正文走 256（`SI:195-196`）。
    pub fn clamped_for_balloon(&self) -> BalloonFields {
        BalloonFields {
            info_title: truncate_for_balloon(&self.title, SZ_INFO_TITLE_CHARS),
            info: truncate_for_balloon(&self.body, SZ_INFO_CHARS),
        }
    }
}

/// 主干 `SI:203-206` 逐字：`text.Length < bufferSize ? text : text[..(bufferSize - 2)] + "…"`
/// —— 注释口径：「定长 ByValTStr 字段要留给尾部的 `\0`…避免像 toast 硬裁那样在半字处断开」。
/// 唯一偏离（登记 `tmp/tt2-report.md §6`）：裁点若正好落进一对代理中间，分叉**退一格**
/// （C# `text[..n]` 会劈开代理对，而 Rust 侧留半对就写不出合法 `String`）。
fn truncate_for_balloon(text: &str, buffer_units: usize) -> String {
    if text.encode_utf16().count() < buffer_units {
        return text.to_string();
    }
    let mut kept: Vec<u16> = text.encode_utf16().take(buffer_units - 2).collect();
    if kept.last().is_some_and(|unit| (0xD800..=0xDBFF).contains(unit)) {
        kept.pop();
    }
    let mut out = String::from_utf16_lossy(&kept);
    out.push(BALLOON_ELLIPSIS);
    out
}

/// 定长 `ByValTStr`：文本按 UTF-16 写入、尾格留 `0`（就是 [`truncate_for_balloon`] 那条注释要的 `\0` 位）。
/// 进这里的文本已钳过，所以只会「写完 + 填零」；万一超长，`zip` 也把多出来的码元截掉而不是越界。
fn fixed_utf16<const N: usize>(text: &str) -> [u16; N] {
    let mut buf = [0u16; N];
    for (slot, unit) in buf.iter_mut().zip(text.encode_utf16()) {
        *slot = unit;
    }
    buf
}

// ------------------------------------------------------------------ 纯逻辑：准入门

/// 主干 `MW:7898` 那三个因子，**全部由调用点喂进来**：
/// `ShowNotifications` 开关（分叉同名旗 = `TRAY_NOTIFICATIONS_KEY` / `shellfiles.rs:205 show_notifications`）、
/// 窗柄（分叉通道 `scroll.rs::hwnd() -> Option<usize>`）、前台窗（本刀**不许**自己去要，
/// 所以 `foreground_hwnd` 是显式入参，见 `tt1 §5-3`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gate {
    pub show_notifications: bool,
    pub hwnd: Option<usize>,
    pub foreground_hwnd: Option<usize>,
    /// 主干 `SI:176-186`：「气球必须有所属图标，没有就静默跳过」。
    pub tray_added: bool,
}

/// 三腿逐字照 `MW:7898`：`ShowNotifications && _hwnd != IntPtr.Zero && GetForegroundWindow() != _hwnd`
/// （聚焦时不打扰）。`foreground = None` 对应主干 `GetForegroundWindow()` 返回 `IntPtr.Zero` ⇒ 照旧「不是自己」。
pub fn should_notify(gate: &Gate) -> bool {
    let own = gate.hwnd.filter(|hwnd| *hwnd != 0);
    gate.show_notifications && own.is_some() && gate.foreground_hwnd != own
}

/// 投递结果。主干 `SI:504-507` 把整发 `Show` 包在 `try/catch` 里、失败只 `Debug.WriteLine`、**从不抛**
/// （`SI:310` 逐字「通知不可用从不影响壳的其它部分」）⇒ 这里也只用一枚码表态，不引错误类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    Sent,
    /// 准入门挡回（开关关 / 没窗柄 / 自己就在前台）。
    Skipped,
    /// `Gate::tray_added = false` ⇒ 静默跳过（主干同档：用户关掉托盘就是不收扰，也不替他重建）。
    NoTrayIcon,
    /// `Shell_NotifyIconW` 返回零（OS 没收）。
    Failed,
}

// --------------------------------------------------------------------- 真 FFI 半

/// 对外一颗入口：准入 → 组装 → 一发 `Shell_NotifyIconW(NIM_MODIFY, …)`。
/// ⚠ 测试只打两枚**挡回**臂（`Skipped` / `NoTrayIcon`，都在 FFI 之前 return）；
/// `Sent` / `Failed` 那一腿**本轮零执行**，形制归源码锁。
#[allow(dead_code)] // 唯一的读者 = 主干那 11 发调用面的移植刀（后续刀）；本刀只交原语 ⇒ 届时删这行属性。
pub fn show(gate: &Gate, title: &str, body: &str, action: Option<&str>) -> Delivery {
    if !should_notify(gate) {
        return Delivery::Skipped;
    }
    if !gate.tray_added {
        return Delivery::NoTrayIcon;
    }
    // header 这里传空串 = `UQ:106` 那一判不成立 ⇒ 标题/正文原样进钳制，与主干 `SI:489
    // BalloonFallback(title, body)` 的直传形状逐字相同；带 header 的调用点自己先走 `with_header`。
    let toast = Toast { title: Cow::Borrowed(title), body: Cow::Borrowed(body), action }.with_header("");
    // 气球这一臂拿不到激活参数（`SI:328` 的挂钩是 `Action<string,string>`）⇒ 显式取出来丢掉，
    // 让「丢了 action」在代码里可见，而不只在注释里。
    let _activation = toast.action;
    let fields = toast.clamped_for_balloon();
    let nid = NotifyIconDataW {
        cb_size: size_of::<NotifyIconDataW>() as u32,
        hwnd: gate.hwnd.unwrap_or(0) as *mut c_void,
        u_id: TRAY_UID,
        u_flags: NIF_MESSAGE | NIF_INFO,
        u_callback_message: WM_TRAY_CALLBACK,
        // 主干 `SI:188-198` 没写 `hIcon`（气球不改图标）⇒ 留零值，与 `Marshal` 出来的字节同形。
        h_icon: std::ptr::null_mut(),
        sz_tip: fixed_utf16::<SZ_TIP_CHARS>(crate::shellfiles::TOAST_SENDER_NAME),
        dw_state: 0,
        dw_state_mask: 0,
        sz_info: fixed_utf16::<SZ_INFO_CHARS>(&fields.info),
        u_timeout_or_version: 0,
        sz_info_title: fixed_utf16::<SZ_INFO_TITLE_CHARS>(&fields.info_title),
        dw_info_flags: NIIF_INFO,
        guid_item: [0; 16],
        h_balloon_icon: std::ptr::null_mut(),
    };
    if unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) } != 0 {
        Delivery::Sent
    } else {
        Delivery::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 本文件的**生产段代码**（`#[cfg(test)]` 之前、剥掉注释行）。剥注释是必需的：本文件大量引用
    /// 主干原文与 `NIM_ADD` 之类的反锁串，让注释去过字面量锁 = 自己咬自己（`dock.rs` 同一条成规）。
    fn production_code() -> String {
        let source = include_str!("toast.rs");
        // 切分点必须带上 `mod tests` 那颗锚：本文件的模块头注释里**逐字**出现过 `#[cfg(test)]`
        // （「由本文件 `#[cfg(test)]` 的真值表逐档覆盖」），只按前缀切会得到一截**注释残片** ⇒ 假绿。
        let mark = concat!("#[cfg(", "test)]\nmod tests");
        let head = source.split(mark).next().unwrap_or(source);
        head.lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn gate(show: bool, own: Option<usize>, foreground: Option<usize>) -> Gate {
        Gate { show_notifications: show, hwnd: own, foreground_hwnd: foreground, tray_added: true }
    }

    /// 准入 = 三腿**合取**，一档都不许多（`MW:7898`）。含两枚主干边界：`hwnd = 0` 等同「没窗柄」
    /// （`_hwnd != IntPtr.Zero`），前台窗取到 0/`None` 都算「不是自己」。
    #[test]
    fn the_admission_gate_is_exactly_the_three_leg_conjunction() {
        for (flag, own, foreground, want) in [
            (true, Some(5), Some(9), true),   // 开关开 + 有窗柄 + 前台是别人 ⇒ 发
            (true, Some(5), Some(5), false),  // 自己就在前台 ⇒ 不打扰
            (false, Some(5), Some(9), false), // 开关关 ⇒ 挡
            (true, None, Some(9), false),     // 没窗柄 ⇒ 挡
            (true, Some(0), Some(9), false),  // 窗柄是 0 ⇒ 挡（主干 `!= IntPtr.Zero`）
            (true, Some(0), None, false),
            (true, Some(5), None, true),      // 前台无窗 = Zero ≠ 自己 ⇒ 发
            (false, None, None, false),
        ] {
            assert_eq!(
                should_notify(&gate(flag, own, foreground)),
                want,
                "准入真值表漂了：flag={flag} own={own:?} foreground={foreground:?}"
            );
        }
        // tray_added 不参与准入（主干 `SI:176-186` 把它放在投递前的静默跳过，不是准入门的第四腿）。
        assert!(should_notify(&gate(true, Some(5), Some(9))));
        let mut no_tray = gate(true, Some(5), Some(9));
        no_tray.tray_added = false;
        assert!(should_notify(&no_tray), "tray_added 被误当成准入的第四腿");
    }

    /// 24 字门（`UQ:106-114`）：非空 header 才动；`<= 24` 码元进标题，否则折进正文用 `"\n"` 连；
    /// 正文原本为空就只留 header（`:114` 的后一支）。尺 = UTF-16 码元 ⇒ 13 颗 emoji（26 码元）算长。
    #[test]
    fn the_header_gate_routes_by_utf16_units_and_stops_at_24() {
        let base = Toast { title: Cow::Borrowed("需要你的输入"), body: Cow::Borrowed("题干"), action: None };
        // 空 header ⇒ 两枚都不动。
        assert_eq!(base.clone().with_header(""), base);
        // 恰 24 码元（上界本身）⇒ 进标题，正文不动。
        let exactly_24 = "计".repeat(24);
        let routed = base.clone().with_header(&exactly_24);
        assert_eq!(routed.title.as_ref(), exactly_24.as_str());
        assert_eq!(routed.body.as_ref(), "题干");
        // 25 码元 ⇒ 折进正文，标题保持调用点给的那枚短标签。
        let over = "计".repeat(25);
        let routed = base.clone().with_header(&over);
        assert_eq!(routed.title.as_ref(), "需要你的输入");
        assert_eq!(routed.body.as_ref(), format!("{over}\n题干"));
        // 码元尺而非 char 尺：12 颗 astral emoji = 24 码元进标题，13 颗 = 26 码元折正文。
        let twelve = "🍃".repeat(12);
        assert_eq!(base.clone().with_header(&twelve).title.as_ref(), twelve.as_str());
        let thirteen = "🍃".repeat(13);
        assert_eq!(base.clone().with_header(&thirteen).body.as_ref(), format!("{thirteen}\n题干"));
        // 空正文那支（`UQ:114` 后一支）：只留 header，不许冒出前导换行。
        let empty_body = Toast { title: Cow::Borrowed("需要你的输入"), body: Cow::Borrowed(""), action: None };
        assert_eq!(empty_body.with_header(&over).body.as_ref(), over.as_str());
        // 门是**上界**形制：本模块的判据里不许出现下界比较（见 §5 硬纪律）。
        assert!(!production_code().contains(">="), "出现了 `>= N` 下界形制判据");
    }

    /// `TruncateForBalloon`（`SI:203-206`）的算术：`units < buffer` 原样，否则「buffer-2 码元 + …」，
    /// 总长恰比缓冲区少一格留给 `\0`。两档缓冲区（标题 64 / 正文 256）各自验。
    #[test]
    fn the_balloon_clamp_keeps_the_mainline_buffer_math() {
        // 标题档 64。
        assert_eq!(truncate_for_balloon(&"字".repeat(63), 64).chars().count(), 63);
        let hit = truncate_for_balloon(&"字".repeat(64), 64);
        assert_eq!(hit.encode_utf16().count(), 63, "64 码元该被压到 buffer-1（留 \\0 位）");
        assert!(hit.ends_with(BALLOON_ELLIPSIS));
        assert_eq!(hit.strip_suffix(BALLOON_ELLIPSIS), Some("字".repeat(62).as_str()));
        let over = truncate_for_balloon(&"字".repeat(500), 64);
        assert_eq!(over, hit, "超长与恰界同形：都是 62 码元 + …");
        // 正文档 256。
        assert_eq!(truncate_for_balloon(&"句".repeat(256), 256).encode_utf16().count(), 255);
        assert_eq!(truncate_for_balloon(&"句".repeat(255), 256).chars().count(), 255);
        // 代理对：裁点落进一对中间 ⇒ 退一格，绝不留半个（半个写不出合法 UTF-8，也违背主干那条注释的本意）。
        let astral = "a".repeat(61) + "\u{1D11E}\u{1D11E}"; // 65 码元 ⇒ 裁点落进第二对代理的中间
        let clipped = truncate_for_balloon(&astral, 64);
        assert_eq!(clipped.chars().count(), 62, "61 个 BMP + 省略号，代理对被整对退回");
        assert!(!clipped.contains('\u{FFFD}'), "从 UTF-16 复原时出现了替换字符 = 留下了半对代理");
        assert!(clipped.ends_with(BALLOON_ELLIPSIS));
    }

    /// 钳制后的两枚定长字段：标题走 64、正文走 256，且**都进不去 `action`**（气球丢深链）。
    #[test]
    fn the_balloon_arm_drops_the_activation_argument() {
        let toast = Toast {
            title: Cow::Owned("题".repeat(100)),
            body: Cow::Owned("段".repeat(400)),
            action: Some("update"),
        };
        let fields = toast.clamped_for_balloon();
        assert_eq!(fields.info_title.encode_utf16().count(), 63);
        assert_eq!(fields.info.encode_utf16().count(), 255);
        assert!(!fields.info_title.contains("update"));
        assert!(!fields.info.contains("update"), "气球不许带激活参数：SI:328 的挂钩签名里没有它");
    }

    /// 两枚**挡回**臂可以在测试里走（都排在 FFI 之前）；这同时证明「没发出去」与「发了」是可区分的码。
    #[test]
    fn the_two_refusals_return_before_any_ffi() {
        let blocked = gate(false, Some(5), Some(9));
        assert_eq!(show(&blocked, "标题", "正文", Some("update")), Delivery::Skipped);
        let no_tray = Gate { tray_added: false, ..gate(true, Some(5), Some(9)) };
        assert_eq!(show(&no_tray, "标题", "正文", None), Delivery::NoTrayIcon);
        // `Sent` / `Failed` 只能由真投递产生 ⇒ 本轮零执行（源码锁见下面两枚）。
        assert_ne!(Delivery::Sent, Delivery::Failed);
    }

    /// **源码锁（真 FFI 半的形制）**：入口、消息号、旗、字段全取主干现值，一条都不许漂。
    #[test]
    fn this_module_locks_the_balloon_shape() {
        let code = production_code();
        // 成规：`#[link]` 裸 extern，Unicode 实体一颗。
        assert!(code.contains(concat!("#[link(", "name = \"shell32\")")), "shell32 的 #[link] 块没了（要引依赖就得先过用户）");
        assert!(code.contains("fn Shell_NotifyIconW(message: u32, data: *const NotifyIconDataW) -> i32;"));
        assert_eq!(code.matches("Shell_NotifyIconW").count(), 2, "声明 1 + 投递 1：多一颗入口就是第二份真相");
        // 只发 NIM_MODIFY 那一档（SI:200），托盘建/销那一族（SI:125/:143）不归本模块。
        assert_eq!(code.matches("NIM_MODIFY").count(), 2, "1 颗常量 + 1 发引用：多一处就是第二份真相");
        assert!(code.contains("Shell_NotifyIconW(NIM_MODIFY, &nid)"), "气球腿的实发形制漂了");
        assert!(!code.contains("NIM_ADD"), "本刀不许自造托盘建图标腿（归托盘那一刀）");
        assert!(!code.contains("NIM_DELETE"));
        // 旗与缓冲区逐字对主干 SI:193/197/292/295/298/22/117。
        for needle in [
            "const NIM_MODIFY: u32 = 0x01;",
            "const NIF_MESSAGE: u32 = 0x1;",
            "const NIF_INFO: u32 = 0x10;",
            "const NIIF_INFO: u32 = 0x1;",
            "const WM_TRAY_CALLBACK: u32 = 0x0400 + 1;",
            "const TRAY_UID: u32 = 1;",
            "const SZ_TIP_CHARS: usize = 128;",
            "const SZ_INFO_CHARS: usize = 256;",
            "const SZ_INFO_TITLE_CHARS: usize = 64;",
            "const TITLE_MAX_UNITS: usize = 24;",
            "u_id: TRAY_UID",
            "u_flags: NIF_MESSAGE | NIF_INFO",
            "u_callback_message: WM_TRAY_CALLBACK",
            "dw_info_flags: NIIF_INFO",
            "sz_tip: fixed_utf16::<SZ_TIP_CHARS>(crate::shellfiles::TOAST_SENDER_NAME)",
        ] {
            assert!(code.contains(needle), "形制漂了：{needle}");
        }
        // 纯逻辑半不许自己去碰窗柄/前台/线程 —— 那是调用点的活（`tt1 §5-3`）。
        for forbidden in [
            concat!("Get", "ForegroundWindow"),
            concat!("Enum", "ThreadWindows"),
            concat!("win", "dows_reactor"),
            concat!("VecDe", "que"),
            concat!("HashS", "et"),
            concat!("Duration", "::from"),
            concat!("thread::sle", "ep"),
            concat!("RemindedUpdate", "Tag"),
        ] {
            assert!(!code.contains(forbidden), "多出/借用了不该有的东西：{forbidden}");
        }
        // 发送者名只许**读**既有资产，不许在分叉再钉一份字面量（`tt1 §4a` 那条「已在册」）。
        assert_eq!(code.matches("Blade").count(), 0, "sender 名必须走 shellfiles::TOAST_SENDER_NAME，别再写死");
    }

    /// **源码锁（FFI 结构体的字段序）**：与 `SI:285-302` 逐字段同序，`#[repr(C)]` 不许换 packing。
    #[test]
    fn the_struct_mirrors_the_mainline_field_order() {
        let code = production_code();
        let body_start = code
            .find("struct NotifyIconDataW {")
            .expect("找不到 FFI 结构体（声明漂了）");
        let rest = &code[body_start..];
        let body = &rest[..rest.find("\n}").expect("结构体没有闭括号")];
        let order = [
            "cb_size",
            "hwnd",
            "u_id",
            "u_flags",
            "u_callback_message",
            "h_icon",
            "sz_tip",
            "dw_state",
            "dw_state_mask",
            "sz_info",
            "u_timeout_or_version",
            "sz_info_title",
            "dw_info_flags",
            "guid_item",
            "h_balloon_icon",
        ];
        assert_eq!(body.matches(": ").count(), order.len(), "字段数漂了：主干 15 颗，一颗不许多一颗不许少");
        let mut previous = 0usize;
        for field in order {
            let at = body.find(field).expect("字段丢了：{field}");
            assert!(at > previous, "字段序漂了：{field} 不在它那一格（cbSize 靠序不靠名字）");
            previous = at;
        }
        assert!(code.contains("#[repr(C)]\n#[derive(Debug)]"), "FFI 结构体的形制属性漂了（repr(C) 是 cbSize 成立的唯一凭据）");
        assert!(!code.contains("Default"), "又去 derive/调用 Default：数组缓冲区拿不到，且逐颗写字段才是主干那形");
    }

    /// `fixed_utf16` 的尾格留 `0`：定长缓冲区里「写完即停」，剩下的全是零 ⇒ 就是主干那条 `\0` 注释。
    #[test]
    fn the_fixed_buffers_keep_the_terminating_zero_slots() {
        let buf = fixed_utf16::<SZ_INFO_TITLE_CHARS>("Blade\u{b2}");
        let name: Vec<u16> = "Blade\u{b2}".encode_utf16().collect();
        assert_eq!(&buf[..name.len()], &name[..]);
        assert!(buf[name.len()..].iter().all(|unit| *unit == 0), "尾格没留零 = ByValTStr 会把脏内存当文本");
        let long = fixed_utf16::<16>(&"字".repeat(20));
        let packed = String::from_utf16(&long).expect("定长缓冲区复原失败");
        assert_eq!(packed.matches('字').count(), 16, "写满 16 格，多出来的码元被挤掉而不是越界");
        let tail = String::from_utf16_lossy(&fixed_utf16::<16>(&"字".repeat(3)));
        assert_eq!(tail.replace('\0', ""), "字字字", "只写 3 格、剩下 13 格留零 = ByValTStr 的 \\0 尾");
    }
}
