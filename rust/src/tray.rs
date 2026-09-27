//! 托盘底座**原语**（**TR1 第 1 把刀**，台账 `docs/gap-audit/RUST-PARITY-AUDIT.md` 里
//! 「P1-9 | 托盘 + toast」那一行的**托盘半**）：`shell32!Shell_NotifyIconW` 的 **`NIM_ADD` 与
//! `NIM_DELETE` 两档** + `hIcon` 解析。裸 FFI、零新依赖（`Cargo.toml` / `Cargo.lock` 一字未动）。
//!
//! # 为什么必须是**另一颗模块**（这刀的存在理由）
//!
//! 主干 `MainWindow.ShellIntegration.cs`（下文 `SI:` = 该文件行号，**本轮逐字现测**，基线
//! md5 `4a6feb050d236ae6d658a6bf0ffd43ff`）里 `Shell_NotifyIcon` 只有三发：`NIM_ADD`（`SI:125`）/
//! `NIM_DELETE`（`SI:143`）/ `NIM_MODIFY`（`SI:200`）。分叉已落地的 `src/toast.rs`（TT2）**只发
//! `NIM_MODIFY` 那一档**，并且被 TT2 自己的源码锁反锁死 `!code.contains("NIM_ADD")` /
//! `!code.contains("NIM_DELETE")`（`toast.rs:390-391`）⇒ 托盘图标**从未被建过** ⇒ 实机永远走
//! `Delivery::NoTrayIcon`（`toast.rs:217`）。底座只能是新模块，不可能是 `toast.rs` 长出来的腿。
//!
//! # 本刀的**范围**（四件，一件都不许多）
//!
//! ① [`NotifyIconDataW`] 形状 + 三颗 `#[link]` 声明；② [`add`]（`NIM_ADD`）；③ [`remove`]
//! （`NIM_DELETE` + `DestroyIcon`）；④ [`app_icon_path`] / [`extract_tray_icon`]（`hIcon` 解析）。
//! **明确不做**（逐条登记在 `tmp/tr1-report.md` §8，归下一批）：comctl32 窗口子类化与 wndproc 回调臂、
//! 原生 `CreatePopupMenu` + `TrackPopupMenuEx` 菜单、`0x0202`/`0x0205`/`0x0402` 三档鼠标消息分发、
//! `ActivateFromTray`（`SI:152-166`）等价物、`shell.json` 那枚 `show_tray_icon` 的准入位、
//! 以及一切 `main.rs` 面（`main.rs` 一个字没改）。
//!
//! ⚠ **今天发出去的消息无人接**（如实备案，别读成「已接通」）：[`add`] 照 `SI:119` 把
//! `WM_APP + 1 = 0x0401`（`SI:22`）写进 `u_callback_message`，但本刀**不装**任何接收端
//! （`SetWindowSubclass` / `DefSubclassProc` / 钩子一概没有 ⇒ 源码锁 `the_module_carries_no_receiver_yet`
//! 把这件事钉成机器可见的）。主干那条「subclass 先于 `TrayAdd`」（`SI:40` → `SI:47`，中间不留
//! 「图标有了、回调没人接」的窗口期）是**下一批**的硬序；本刀交出去的形状**故意**是主干那两条硬序的
//! 后半截缺着的状态 —— 接线刀不许直接调 [`add`] 而不先装回调。
//!
//! # 三条主代理裁定的落点
//!
//! **裁定 1 —— `hIcon` 走「运行时 `Assets/app.ico`」**（TB1 §4 的裁 B，**不是**裁 A 的嵌图标）：
//! 分叉 `blade2-rs.exe` 的 PE 里**连 `.rsrc` 段都没有**（资源目录 `rva=0 size=0`；本轮 TR1 现测复核过，
//! 见 `tmp/tr1-report.md` §5），所以主干那一发 `ExtractIconEx(Environment.ProcessPath, 0, …)`（`SI:108`）
//! 对分叉必返回 0 ⇒ 照 `SI:108` 逐字写的 `add()` 永远当场早退。API **一个换字都不动**，只把第一参换成
//! `<资产根>/Assets/app.ico`（Win32 的 `lpszFile` 收 `.exe`/`.dll`/`.ico`），路径解析复用分叉既有成规
//! （`main.rs` 的 `fn brand_mark_path()` 那套六层回溯找资产根）。
//! **备案可见差（TB-8）**：部署树之外拿不到 ico ⇒ 这一发**不许**自造兜底（不许退系统默认图标、
//! 不许内嵌 base64、不许造第二张 `.ico`），[`add`] 必须**如实**回 [`AddOutcome::NoIcon`]。
//! ⚠ 不许走 `WindowVisuals::icon(path)`：那是 `AppWindow.SetIcon`（**窗口图标**，任务栏/标题栏），
//! 不是 `HICON`，喂不进 `nid.hIcon`（TB1 §1.3-B 已证）。
//!
//! **裁定 2 —— `hwnd` 由 lib 侧自己拿**：`crate::scroll::hwnd()`（`EnumThreadWindows` + 类名匹配 +
//! `thread_local` 缓存）已是分叉在架的唯一 HWND 通路 ⇒ 本模块**不新增**由 `main.rs` 传进来的窗柄参数。
//! 约束照 `scroll.rs` 现测：它只能在 UI 线程（那条消息泵上）调 ⇒ 拿不到就是拿不到，[`add`] 回
//! [`AddOutcome::NoHwnd`]，**全程零 `unwrap` / 零 panic**。
//!
//! **裁定 3 —— 结构体与 `Shell_NotifyIconW` 自带一份，禁止重构 `toast.rs`**：本文件那份
//! [`NotifyIconDataW`] 与 `src/toast.rs:77` 那份**同序同数**，这是**设计上的两份真相**、
//! **不是**「重复代码待清理」。两边不许合并、不许谁 `use` 谁：`toast.rs` 是 `main.rs` 的**私有 bin mod**
//! （跨 crate 边界本来就 `use` 不到），而搬走任何一枚字段都会当场打红 TT2 钉在 `toast.rs` 里的
//! 两把源码锁（共 4 处：`this_module_locks_the_balloon_shape` 的 `contains` + 两处
//! `matches(..).count() == 2`、`the_struct_mirrors_the_mainline_field_order` 的
//! `find("struct NotifyIconDataW {")`）。⇒ 本刀对 `toast.rs` **零改动**。
//!
//! # 照抄的主干缺陷（保真 > 「顺手修好」；TB-3 / TB-4）
//!
//! 1. **`NIM_ADD` 失败时句柄既不 `DestroyIcon` 也不清零**：主干 `_trayIcon = large`（`SI:112`）发生在
//!    `NIM_ADD`（`SI:125`）**之前**；那一发失败 ⇒ `_trayAdded` 假 ⇒ `TrayRemove()` 被 `SI:130` 的守卫
//!    挡回 ⇒ 那颗句柄永远挂在槽里。[`add`] **逐字保持这个次序**（源码锁
//!    [`the_failed_add_keeps_the_icon_handle_in_the_slot`]），不许「修好」。
//! 2. **硬崩溃后的死图标无人清**：主干只有「正常退出」与 `WM_DESTROY` 两道闸，`NIM_SETVERSION` /
//!    `NIM_SETFOCUS` / `NIS_HIDDEN` / `NIN_SELECT` 全文件零命中 ⇒ 本模块一概不加。
//! 3. **（TR1 现测新发现，简报没点名）`SI:108` 的 `out _` 丢掉 small 句柄**：`ExtractIconEx` 交出的
//!    第二颗句柄主干既不接也不销 ⇒ 每次建图标泄一颗小图标句柄。本刀**照抄**（仍把出参接进变量再丢掉，
//!    不 `DestroyIcon`），理由同上：改了它就等于和主干不可比。
//!
//! # 单测纪律（零前台硬令）
//!
//! **绝对禁止**在任何测试里真调 `Shell_NotifyIconW`（三档全禁，含 `NIM_ADD`）—— 那一发会真的往用户
//! 托盘塞图标。测试只走三条腿：①**FFI 之前的早退臂**（[`add`] 的 `NoHwnd` / [`remove`] 的 `NotAdded`，
//! 打 [`add`] 之前先复刻 `scroll.rs:1752` 那条离线守卫）；②**纯层**真值表（[`add_step`] /
//! [`icon_from_extract`] / [`add_payload`] / [`delete_payload`]）+ 源码锁；③**只读取证**的真 FFI：
//! [`extract_tray_icon`] 与 `LoadImageW` 双口径（只造句柄、只 `DestroyIcon`，不显示、不建条目）与
//! comctl32 符号**查询**（[`the_next_batch_subclass_symbols_are_measured_not_assumed`]，只
//! `GetProcAddress`、一颗窗口都不建）。测试函数名一律 snake_case（`src/` + `tests/` 零
//! `#[allow(non_snake_case)]` 先例）。

use std::cell::Cell;
use std::env;
use std::ffi::c_void;
use std::mem::size_of;
use std::path::{Path, PathBuf};

// 主干 `SI:251-252` `[DllImport("shell32.dll", CharSet = Unicode, SetLastError = true)]`
// `Shell_NotifyIcon` ⇒ Unicode 实体 `Shell_NotifyIconW`；`SI:253-254` `ExtractIconEx` ⇒ `ExtractIconExW`。
// `SetLastError = true` 那半截**不跟着做**（TB-14：主干体内零处读 `Marshal.GetLastWin32Error()`，
// 成败判据只有返回值非零）。返回 `BOOL` ⇒ 按 `!= 0` 判，不许只看「没崩」。
// ⚠ 这里用 `//` 不用 `///`：rustc 不给 extern 块生成文档，`///` 会叫 `unused_doc_comments` 判一条警告。
// 与 `toast.rs:36-39` 各声明一份是**成规**（同 `nativepick.rs` 与 `kernel.rs` 各带一份 shell32 块），
// 不是遗漏：`toast.rs` 住 bin 侧，本模块住 lib 侧，物理上共不了。
#[link(name = "shell32")]
unsafe extern "system" {
    fn Shell_NotifyIconW(message: u32, data: *const NotifyIconDataW) -> i32;
    fn ExtractIconExW(
        file: *const u16,
        index: i32,
        large: *mut *mut c_void,
        small: *mut *mut c_void,
        count: u32,
    ) -> u32;
}

// 主干 `SI:255-256` `[DllImport("user32.dll")] DestroyIcon`。`remove()` 的**后半截**才用它：
// 硬序 = `NIM_DELETE`（`SI:143`）**先于** `DestroyIcon`（`SI:146`）。
#[link(name = "user32")]
unsafe extern "system" {
    fn DestroyIcon(icon: *mut c_void) -> i32;
}

// ------------------------------------------------------------------- 消息档与旗（逐字取主干现值）

/// `SI:125`：`_trayAdded = Shell_NotifyIcon(0x00 /* NIM_ADD */, ref nid)`。全主干**只有这一发**。
const NIM_ADD: u32 = 0x00;
/// `SI:143`：`Shell_NotifyIcon(0x02 /* NIM_DELETE */, ref nid)`。全主干**只有这一发**。
const NIM_DELETE: u32 = 0x02;
// ⚠ **本模块不定义 `NIM_MODIFY`**：那一档（`SI:200`）归 `toast.rs` 的气球腿，两处都发就是第二份真相。
//   源码锁 `this_module_sends_exactly_two_message_kinds` 反锁 `NIM_MODIFY` 在本文件产品码里零命中。

/// `SI:118` `uFlags = 0x1 | 0x2 | 0x4 // NIF_MESSAGE | NIF_ICON | NIF_TIP`（建图标那发的三枚旗）。
const NIF_MESSAGE: u32 = 0x1;
const NIF_ICON: u32 = 0x2;
const NIF_TIP: u32 = 0x4;

/// `SI:22`：「托盘回调消息：WM_APP 段壳内无其它占用者，取首号」`WM_TRAYCALLBACK = 0x0400 + 1` = `0x0401`。
/// 与 `toast.rs:52` 逐字同值（两模块各钉各的是裁定 3 的代价，不是待办）。
/// ⚠ 本刀只把它**写进载荷**（`SI:119`），**不装**接收端 —— 见模块头「今天发出去的消息无人接」。
const WM_TRAY_CALLBACK: u32 = 0x0400 + 1;

/// `SI:117` / `SI:138` 逐字 `uID = 1`。`NIF_GUID`（`0x20`）与 `guidItem` 赋值在主干全仓**零命中** ⇒
/// 条目只由 `(hWnd, uID)` 这一对认得。不许「顺手现代化」成 GUID 条目。
const TRAY_UID: u32 = 1;

/// 三枚 `ByValTStr` 缓冲区的 `SizeConst`：`SI:293` `szTip` = 128、`SI:296` `szInfo` = 256、
/// `SI:298` `szInfoTitle` = 64（单位 = UTF-16 码元）。
const SZ_TIP_CHARS: usize = 128;
const SZ_INFO_CHARS: usize = 256;
const SZ_INFO_TITLE_CHARS: usize = 64;

/// 六层回溯找资产根的层数，**逐字照 `main.rs` 的 `fn brand_mark_path()`**（`for _ in 0..6` +
/// `dir = dir.parent()?`）；裁定 1 只换了要找的那颗文件名。
const ASSET_ROOT_UPWARD_PROBES: usize = 6;

/// 资产根里的图标文件名（`Blade2.csproj` 的 `<ApplicationIcon>Assets\app.ico` 是**主干 exe** 的编译期
/// 图标；分叉没嵌 ⇒ 这一颗今天只能从仓/部署树里读，见模块头裁定 1 的可见差备案）。
const ASSET_ICON_FILE: &str = "app.ico";

// ------------------------------------------------------------------------- FFI 载荷形状

/// 与主干 `SI:285-302` 那颗手写 `NOTIFYICONDATAW` **同序同数**（15 颗字段）：`#[repr(C)]` 的自然填充
/// 在 x64 上是 **976** 字节 —— 简报转述与 `tmp/tb1-spec.md:702` 写的 984 是**误测**（现测推导 + 两条
/// 独立 oracle 见 `the_struct_is_the_x64_mainline_size`，纠正登记在报告 §9）。15 颗字段的偏移也与
/// C 头逐字相同 ⇒ 发出去的 `cbSize` 与主干 `Marshal.SizeOf<NOTIFYICONDATAW>()` 同值。主干那份只声明到
/// `hBalloonIcon`（`SI:301`），本模块**一字不加**。
///
/// ⚠ **这是分叉侧第二份同形结构体**（第一份 = `src/toast.rs:77`）：设计上的两份真相，
/// 不是待清理的重复代码 —— 理由与不许动的四把锁写在模块头裁定 3。
///
/// `Debug` 是**必需**的而非装饰：15 颗字段里有 6 颗（`dw_state` / `dw_state_mask` / `u_timeout_or_version`
/// / `dw_info_flags` / `guid_item` / `h_balloon_icon`）主干从不赋值、本模块也只写零值 ⇒ 没有 derive
/// 就把它们算成「写而不读」，`dead_code` 会当场点名（`toast.rs:72` 同一格）。
/// **不 derive `Default`**：core 只给到 32 格的数组实现，`[u16; 128]` 拿不到；而且逐颗写全字段才是
/// 主干 `SI:113-124` 那形（写了 9 颗、其余按零值）。
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
    /// 主干 `SI:297 public uint uVersion;`（与 `uTimeout` 共 union）。**恒 0**：主干三颗初始化器
    /// 一枚都不赋 ⇒ 才有 `lParam = 鼠标消息` 的旧语义（`SI:82` 注释逐字为凭）。设成 4 就得改读
    /// `GET_X_LPARAM`，整块回调臂的形制全漂（TB-5 不许）。
    u_timeout_or_version: u32,
    sz_info_title: [u16; SZ_INFO_TITLE_CHARS],
    dw_info_flags: u32,
    /// 主干 `SI:300 public Guid guidItem;`（16 字节）。**恒 0**（`NIF_GUID` 零命中）⇒ 用同尺寸的
    /// `[u8; 16]` 占位，不为布局保真再声明一颗没人读的 `Guid`。
    guid_item: [u8; 16],
    /// 主干 `SI:301 public IntPtr hBalloonIcon;`。**恒 0**（TB-11：气球图标旗主干根本没有）。
    h_balloon_icon: *mut c_void,
}

// ------------------------------------------------------------------- 主干实例字段的落点

thread_local! {
    /// 主干 `SI:26 private bool _trayAdded;` 的等价物。主干那是 `MainWindow` 的**实例**字段，而 lib 侧
    /// 没有那个实例 ⇒ 落 `thread_local!`（成规 `scroll.rs:781` / `keys.rs:300`）：托盘条目归**装它的那条
    /// 线程**的消息泵，而本模块的窗柄来自 `scroll::hwnd()`（同一线程局部），两半天然对齐。
    static ADDED: Cell<bool> = const { Cell::new(false) };
    /// 主干 `SI:27 private IntPtr _trayIcon;`。**只在 [`remove`] 里 `DestroyIcon`**；`NIM_ADD` 失败时
    /// 这一格留着句柄不清零 —— 那是主干的形状（模块头「照抄的主干缺陷」第 1 条），别修。
    static ICON: Cell<*mut c_void> = const { Cell::new(std::ptr::null_mut()) };
}

// ----------------------------------------------------------------------- 纯判据（零 FFI）

/// 建图标两枚前置的**准入半**（零 FFI ⇒ 可在单测里逐档跑）。
/// 主干 `SI:103` 把两判并成一发 `if (_trayAdded || _hwnd == IntPtr.Zero) return;`；这里拆成两枚码
/// 只是为了**如实报告**（TB-12 不许谎报），**行为逐字相同**：都是「不发那一发、当场返回」。
/// 顺序照主干：`_trayAdded` 在前（已建过就不再看窗柄）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddStep {
    /// 守卫放行，可以组载荷发那一发。
    Proceed { hwnd: usize },
    /// 主干 `SI:103` 前半：已经建过（幂等 —— 主干 `TrayAdd` 的四发调用方靠这一档才不会建出两颗条目）。
    AlreadyAdded,
    /// 主干 `SI:103` 后半：`_hwnd == IntPtr.Zero`。分叉的等价态 = 不在 UI 线程上 / 窗还没建
    /// （`crate::scroll::hwnd()` 回 `None`），含 `Some(0)` 那一档（主干的 `IntPtr.Zero` 就是 0）。
    NoHwnd,
}

/// [`AddStep`] 的算法本体（就是主干那两枚 `||` 条件）。
pub fn add_step(added: bool, hwnd: Option<usize>) -> AddStep {
    if added {
        return AddStep::AlreadyAdded;
    }
    match hwnd {
        Some(hwnd) if hwnd != 0 => AddStep::Proceed { hwnd },
        _ => AddStep::NoHwnd,
    }
}

/// 主干 `SI:108` 那个守卫的**纯半**：`ExtractIconEx(…) == 0 || large == IntPtr.Zero` ⇒ 取不到句柄。
/// 两判**都在**（TB1 §1.3 现测：只数返回值不数句柄就不是主干那一挡 —— 返回 1 而 `large` 为零是
/// 可能发生的形状，`icon_from_extract` 的真值表把这一档钉住）。
pub fn icon_from_extract(count: u32, large: *mut c_void) -> Option<*mut c_void> {
    if count == 0 || large.is_null() {
        return None;
    }
    Some(large)
}

/// 主干 `SI:113-124` 那颗对象初始化器的逐字翻译（9 颗写全、其余 6 颗按 C# 默认 = 零值）。
///
/// 抽成独立一颗纯函数的唯一理由：`add()` 那一发 `Shell_NotifyIconW` **永不能在单测里跑**（零前台硬令），
/// 而**载荷**可以逐字段断言 —— 形制先例 = `nativepick.rs` 的 `outcome_from`（把判断从 FFI 里抽出来才测得到）。
fn add_payload(hwnd: usize, icon: *mut c_void) -> NotifyIconDataW {
    NotifyIconDataW {
        cb_size: size_of::<NotifyIconDataW>() as u32,
        hwnd: hwnd as *mut c_void,
        u_id: TRAY_UID,
        u_flags: NIF_MESSAGE | NIF_ICON | NIF_TIP, // 逐字 SI:118
        u_callback_message: WM_TRAY_CALLBACK,
        h_icon: icon,
        // 主干 `SI:121 szTip = "Blade²"` 是**写死的字面量**、且**不截断**（`TruncateForBalloon`
        // （`SI:205-208`，工作树现值 `bufferSize - 2` + `"…"`）只喂 `szInfo` / `szInfoTitle` 两格）。
        // 分叉这枚串读既有资产 `shellfiles::TOAST_SENDER_NAME`（= 同一串 UTF-8 字节，
        // `shellfiles.rs:117` 处有逐字节锁），不在这里再钉第三份 `Blade²` 字面量。
        sz_tip: fixed_utf16::<SZ_TIP_CHARS>(crate::shellfiles::TOAST_SENDER_NAME),
        // 以下 6 颗 = 主干那发**一枚都不赋值**的字段（`SI:1.2` 判语表；TB-4 / TB-5 / TB-11）。
        dw_state: 0,
        dw_state_mask: 0,
        sz_info: fixed_utf16::<SZ_INFO_CHARS>(""), // 主干 `SI:122 szInfo = string.Empty`
        u_timeout_or_version: 0,
        sz_info_title: fixed_utf16::<SZ_INFO_TITLE_CHARS>(""), // 主干 `SI:123`
        dw_info_flags: 0,
        guid_item: [0; 16],
        h_balloon_icon: std::ptr::null_mut(),
    }
}

/// 主干 `SI:134-142` 那颗初始化器的逐字翻译 —— ⚠ 那颗**没有 `uFlags`、没有 `hIcon`、
/// 没有 `uCallbackMessage`**（`SI:136-141` 只写五格），因为 `NIM_DELETE` 只按 `(hWnd, uID)` 认条目。
/// 补上任何一枚都是多做。
fn delete_payload(hwnd: usize) -> NotifyIconDataW {
    NotifyIconDataW {
        cb_size: size_of::<NotifyIconDataW>() as u32,
        hwnd: hwnd as *mut c_void,
        u_id: TRAY_UID,
        u_flags: 0,
        u_callback_message: 0,
        h_icon: std::ptr::null_mut(),
        // 主干 `SI:139 szTip = string.Empty`：销的那一发把提示位清空（不是「沿用建立那发的 Blade²」）。
        sz_tip: fixed_utf16::<SZ_TIP_CHARS>(""),
        dw_state: 0,
        dw_state_mask: 0,
        sz_info: fixed_utf16::<SZ_INFO_CHARS>(""),
        u_timeout_or_version: 0,
        sz_info_title: fixed_utf16::<SZ_INFO_TITLE_CHARS>(""),
        dw_info_flags: 0,
        guid_item: [0; 16],
        h_balloon_icon: std::ptr::null_mut(),
    }
}

/// 定长 `ByValTStr`：文本按 UTF-16 写入、剩下的格留 `0`（就是主干那颗 `\0` 尾）。
/// 与 `toast.rs:164-170` 同形（裁定 3：各留一份，两边不许 `use` 谁）。
fn fixed_utf16<const N: usize>(text: &str) -> [u16; N] {
    let mut buf = [0u16; N];
    for (slot, unit) in buf.iter_mut().zip(text.encode_utf16()) {
        *slot = unit;
    }
    buf
}

/// UTF-16 + 尾 `0`（Win32 宽串）。成规出处 = `nativepick.rs:187` / `updatecheck.rs:449` 同形。
/// 这里的入参来自 `Path::to_string_lossy()`：路径的每一段都出自 `env::current_exe()` 与
/// `ASSET_ICON_FILE` 那两颗常量，合法 Windows 路径在 Rust 的 `OsString` 里就是 UTF-8 ⇒ 往返无损；
/// 唯一会被换掉的是「路径里含孤立代理对」那种 exotic 形态（分叉今天没有任何资产路径函数处理它）。
fn wide_units(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

// ----------------------------------------------------------------------- 图标句柄（裁定 1）

/// 从 exe 所在目录**向上六层**找 `<dir>/Assets/app.ico`（`main.rs` 的 `fn brand_mark_path()` 那套成规，
/// 只把要找的文件名换成 `ASSET_ICON_FILE`）。找不到 ⇒ `None` = 模块头备案的那枚**可见差**
/// （部署树之外没有图标 ⇒ [`add`] 走 [`AddOutcome::NoIcon`]，不自造兜底 = TB-8）。
pub fn app_icon_path() -> Option<PathBuf> {
    icon_path_from_dir(env::current_exe().ok()?.parent()?)
}

/// [`app_icon_path`] 的可注入半（把起点交出来才能在单测里跑「第 5 层命中 / 第 7 层落空」两档）。
fn icon_path_from_dir(start: &Path) -> Option<PathBuf> {
    let mut dir: &Path = start;
    for _ in 0..ASSET_ROOT_UPWARD_PROBES {
        let candidate = dir.join("Assets").join(ASSET_ICON_FILE);
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = dir.parent()?;
    }
    None
}

/// 主干 `SI:108` 那一发的分叉形：**API、序号、颗数、守卫全照原样**，只有第一参来自
/// [`app_icon_path`]（裁定 1）。取不到就回 `None`（不猜、不兜底）。
///
/// 这发 FFI 是**只读取证**：读文件 + 造一颗句柄，不显示、不建托盘条目 ⇒ 允许进单测
/// （同 `nativepick.rs` 的 `known_folder_units` 那一族；测试须自己 `DestroyIcon` 收干净）。
pub fn extract_tray_icon(path: &Path) -> Option<*mut c_void> {
    let file = wide_units(&path.to_string_lossy());
    let mut large: *mut c_void = std::ptr::null_mut();
    // 主干 `SI:108` 的 `out _`：**small 句柄接出来就丢、不 Destroy**（模块头「照抄的主干缺陷」第 3 条）。
    let mut small: *mut c_void = std::ptr::null_mut();
    let count = unsafe { ExtractIconExW(file.as_ptr(), 0, &mut large, &mut small, 1) };
    icon_from_extract(count, large)
}

// ---------------------------------------------------------------------- 两档入口（真 FFI 半）

/// `add()` 的结果码。主干 `TrayAdd()` 返回 `void`（`SI:125` 只把结果写进 `_trayAdded`）；这里必须
/// 把「为什么没建成」如实报出来（TB-12：谎报成功就是往一个不存在的条目上发 `NIM_MODIFY`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddOutcome {
    /// `NIM_ADD` 非零：OS 收了，条目已建。
    Added,
    /// 主干 `SI:103` 前半：已经建过，**幂等**（不重发）。
    AlreadyAdded,
    /// 主干 `SI:103` 后半：没有窗柄（不在 UI 线程 / 窗未建）。
    NoHwnd,
    /// 主干 `SI:108-111` 那一支：取不到 `hIcon`（部署树外没有 `Assets/app.ico`，或 `ExtractIconExW` 回零）。
    NoIcon,
    /// `Shell_NotifyIconW(NIM_ADD, …)` 返回零（OS 没收）。
    Failed,
}

/// 销图标的结果码。主干 `TrayRemove()` 同样返回 `void`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoveOutcome {
    /// 那一发 `NIM_DELETE` 发出去了 + 句柄按主干硬序处理完了。
    Removed,
    /// 主干 `SI:130` 的守卫：`!_trayAdded` ⇒ 什么都没发（**不谎报**成「销过」）。
    NotAdded,
}

/// 建托盘图标：主干 `TrayAdd()`（`SI:101-126`）的逐字移植。
/// 五步序一步不许并：守卫（[`add_step`]）→ 取 `hIcon`（`SI:108`）→ **句柄先落槽**（`SI:112`）→
/// 组载荷（[`add_payload`]）→ `NIM_ADD`（`SI:125`）。
///
/// ⚠ 全模块**唯一的** `NIM_ADD` 出口 —— 主干 `NIM_ADD` 有 4 位调用方（`InstallShellIntegration`
/// / `ShowTrayBalloon` 懒建 / `SetShowTrayIcon` / `EnsureTrayVisible`），但它们都在**调用面**那一侧；
/// 这里再造第二颗入口就是第二份真相。
#[allow(dead_code)] // 非测试读者 = 0（本刀只交原语；读者 = 下一批 `main.rs` 接线刀 ⇒ 届时删这行属性）。
                    // 先例 = `toast.rs:212` 与刚落地的 `nativepick.rs:566`。**不许**为了「让它有用」造调用点。
pub fn add() -> AddOutcome {
    // 裁定 2：窗柄自取（`scroll::hwnd()` 只在 UI 线程上有值 ⇒ 拿不到就如实回，全程零 unwrap）。
    match add_step(ADDED.with(|slot| slot.get()), crate::scroll::hwnd()) {
        AddStep::AlreadyAdded => AddOutcome::AlreadyAdded,
        AddStep::NoHwnd => AddOutcome::NoHwnd,
        AddStep::Proceed { hwnd } => {
            // 主干 `SI:108-111`：取不到句柄就走「不建图标」那一支。
            let Some(icon) = app_icon_path().and_then(|path| extract_tray_icon(&path)) else {
                return AddOutcome::NoIcon;
            };
            // 主干 `SI:112`：`_trayIcon = large` 在 `NIM_ADD` **之前**落槽（失败也不回滚 = 照抄的缺陷 1）。
            ICON.with(|slot| slot.set(icon));
            let nid = add_payload(hwnd, icon);
            // 主干 `SI:125`：`_trayAdded = Shell_NotifyIcon(0x00 /* NIM_ADD */, ref nid)` —— 成败只认返回值。
            let sent = unsafe { Shell_NotifyIconW(NIM_ADD, &nid) } != 0;
            ADDED.with(|slot| slot.set(sent));
            if sent { AddOutcome::Added } else { AddOutcome::Failed }
        }
    }
}

/// 销托盘图标：主干 `TrayRemove()`（`SI:128-150`）的逐字移植。
/// 硬序 = `NIM_DELETE`（`SI:143`）**先于** `DestroyIcon`（`SI:146`）：先把条目从 OS 摘掉，再还句柄；
/// 反过来就是把一颗仍被托盘引用的句柄销毁。
///
/// ⚠ 主干那一发**丢弃** `Shell_NotifyIcon` 的返回值（`SI:143` 前面没有赋值），照样往下 `DestroyIcon`
/// 并清旗 ⇒ 照抄（`the_delete_is_sent_before_the_icon_is_destroyed` 钉着）。重复删除无害
/// （主干 `SI:53` 注释逐字：「WM_DESTROY 兜底再删一次，重复删除无害」）。
#[allow(dead_code)] // 非测试读者 = 0（与 [`add`] 同一条：读者 = 下一批的退出闸 `CleanupShellIntegration` 等价物）。
pub fn remove() -> RemoveOutcome {
    if !ADDED.with(|slot| slot.get()) {
        return RemoveOutcome::NotAdded;
    }
    // 主干 `SI:137 hWnd = _hwnd`：分叉的 `_hwnd` 等价物就是 `scroll::hwnd()` 那一枚缓存。
    // `add()` 只在它回 `Some` 时才会把 `ADDED` 抬起来，而 `scroll` 的 HWND 槽一旦解析就常驻 ⇒
    // 同一线程上这里读到的与建图标那次是同一颗；读不到时给 0（主干的 `IntPtr.Zero` 同形），不 panic。
    let nid = delete_payload(crate::scroll::hwnd().unwrap_or(0));
    unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
    let icon = ICON.with(|slot| slot.get());
    if !icon.is_null() {
        unsafe { DestroyIcon(icon) };
        ICON.with(|slot| slot.set(std::ptr::null_mut()));
    }
    ADDED.with(|slot| slot.set(false));
    RemoveOutcome::Removed
}

/// 托盘条目当前是否真的在架（主干 `_trayAdded` 的只读出口）。
/// 下一批喂 `toast::Gate::tray_added`（`toast.rs:184`）**只有这一条诚实写法**：现读，不许常量 `true`
/// （TB-12；`toast.rs` 的 `Delivery::NoTrayIcon` 那一挡就是靠这枚旗才成立的）。
#[allow(dead_code)] // 非测试读者 = 0（读者 = 接线刀 TB-f 那一把）。
pub fn added() -> bool {
    ADDED.with(|slot| slot.get())
}

// =========================================================================== 单测（零真 FFI 副作用）
//
// 纪律见模块头「单测纪律」：**任何测试都不真调 `Shell_NotifyIconW`**（三档全禁）；`NIM_ADD` 一旦真调
// 就是在用户托盘里塞真图标 = 违反零前台硬令。下面 [`this_module_never_calls_the_notify_api_in_tests`]
// 那把源码锁把这条纪律本身钉成机器可见的。

#[cfg(test)]
mod tests {
    use super::*;

    /// 剥掉注释行（`//` / `///` / `//!` 一律以 `//` 开头 ⇒ 一把尺全覆盖）。
    /// 剥注释是**必需**的：本文件大量引用主干原文与 `NIM_MODIFY` 这类反锁串，让注释去过字面量锁
    /// = 自己咬自己（成规出处 = `toast.rs:258-268` / `nativepick.rs` 的 `product_source`）。
    fn strip_comment_lines(source: &str) -> String {
        source
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 本文件的**产品码**（`#[cfg(test)]` 之前、剥掉注释行）。形制逐字照 `toast.rs:258-268`：
    /// 切分锚必须带上 `mod tests` 那颗（只按 `#[cfg(test)]` 前缀切会得到一截**注释残片** ⇒ 假绿）。
    fn production_code() -> String {
        let source = include_str!("tray.rs");
        let mark = concat!("#[cfg(", "test)]\nmod tests");
        let head = source.split(mark).next().unwrap_or(source);
        strip_comment_lines(head)
    }

    /// **全文剥注释**（产品码 + 测试码）：给「本刀一个接收端都没有」那把锁用 —— 它要钉的是
    /// 「代码里没长出那一族」，而不是「注释里不许提到那一族」（模块头与非 §U-2 的那些说明必须能提）。
    fn all_code() -> String {
        strip_comment_lines(include_str!("tray.rs"))
    }

    /// 测试窗（`mod tests` 之后）剥注释。锁里的禁串一律**运行时现拼**（`str::concat`），
    /// 否则断言数组自己就被自己扫出来 = 假红（`nativepick.rs:798` 那条「锁扫到自己」的教训）。
    fn tests_code() -> String {
        let source = include_str!("tray.rs");
        let mark = concat!("#[cfg(", "test)]\nmod tests");
        let tail = source.split(mark).nth(1).expect("切分锚没命中：产品码窗会含测试码，假绿");
        strip_comment_lines(tail)
    }

    /// 产品码里 [`add`] 那一窗（`pub fn add()` 起、到 `remove` 的文档注释前）。
    fn add_window(code: &str) -> String {
        let start = code.find("pub fn add() -> AddOutcome {").expect("找不到 add() 的窗锚");
        let end = code.find("pub fn remove()").expect("找不到 remove() 的窗锚");
        code[start..end].to_string()
    }

    /// 产品码里 [`remove`] 那一窗（到 `added()` 前）。
    fn remove_window(code: &str) -> String {
        let start = code.find("pub fn remove() -> RemoveOutcome {").expect("找不到 remove() 的窗锚");
        let end = code.find("pub fn added()").expect("找不到 added() 的窗锚");
        code[start..end].to_string()
    }

    fn null_icon() -> *mut c_void {
        std::ptr::null_mut()
    }

    // ------------------------------------------------------------------ 一、早退臂（FFI 之前）

    /// 两枚**早退臂**可以在测试里真走：[`add`] 的 `NoHwnd` 与 [`remove`] 的 `NotAdded` 都排在
    /// 那一发 `Shell_NotifyIconW` 之前。⚠ 打 [`add`] 之前**先复刻** `scroll.rs:1752` 那条离线守卫：
    /// 单测线程上要是竟然有 `WinUIDesktopWin32WindowClass`，这一发就会真去碰托盘 ⇒ 立刻停手（不硬闯）。
    #[test]
    fn the_two_early_arms_return_before_any_notify_call() {
        assert!(
            crate::scroll::hwnd().is_none(),
            "单测线程上竟解析到了 WinUI 窗柄 ⇒ 本测试会真发 `NIM_ADD`，违反零前台硬令"
        );
        assert_eq!(add(), AddOutcome::NoHwnd, "没窗柄就不许往下走到取图标那一步");
        assert!(!added(), "没建成就不许把旗抬起来（TB-12 不许谎报）");
        assert_eq!(remove(), RemoveOutcome::NotAdded, "没在架就不许发 `NIM_DELETE`");
        // 早退之后状态仍然干净：两格都不许被写过。
        assert!(!added());
        assert!(ICON.with(|slot| slot.get()).is_null(), "没取句柄就不许有句柄落槽");
    }

    // ------------------------------------------------------------------------ 二、纯层真值表

    /// 主干 `SI:103` 的两判：顺序（`_trayAdded` 在前）+ `IntPtr.Zero` 档（`Some(0)` 等同没窗柄）。
    #[test]
    fn the_guard_table_is_the_mainline_early_returns() {
        assert_eq!(add_step(true, Some(0x1234)), AddStep::AlreadyAdded);
        assert_eq!(add_step(true, None), AddStep::AlreadyAdded, "主干那一发的顺序：已建过就不再看窗柄");
        assert_eq!(add_step(false, None), AddStep::NoHwnd);
        assert_eq!(add_step(false, Some(0)), AddStep::NoHwnd, "_hwnd == IntPtr.Zero 就是这一档");
        assert_eq!(add_step(false, Some(0x1234)), AddStep::Proceed { hwnd: 0x1234 });
    }

    /// 主干 `SI:108` 的守卫是**两判**（`== 0 || large == IntPtr.Zero`）：四行真值表一行不许并。
    #[test]
    fn the_icon_refuses_when_count_or_handle_is_zero() {
        let held = 0x7f00usize as *mut c_void;
        assert!(icon_from_extract(0, null_icon()).is_none());
        assert!(icon_from_extract(0, held).is_none(), "返回 0 ⇒ 一颗句柄都不取");
        assert!(icon_from_extract(1, null_icon()).is_none(), "返回 1 但句柄为零：主干仍判失败");
        assert_eq!(icon_from_extract(1, held), Some(held));
        assert_eq!(icon_from_extract(2, held), Some(held), "count > 1 也一样放行（主干只要第一颗）");
    }

    /// 建图标那发的载荷 = 主干 `SI:113-124` 逐字（含 `cbSize` 的算法、`uID`、三枚旗、回调号、szTip）。
    #[test]
    fn the_add_payload_carries_the_mainline_initializer_values() {
        let icon = 0xabcdusize as *mut c_void;
        let nid = add_payload(0x1234, icon);
        assert_eq!(nid.cb_size, size_of::<NotifyIconDataW>() as u32, "cbSize 必须等于 size_of（禁字面量）");
        assert_eq!(nid.cb_size, 976, "x64 上主干 Marshal.SizeOf<NOTIFYICONDATAW>() 现算就是 976（不是 984；见 the_struct_is_the_x64_mainline_size）");
        assert_eq!(nid.hwnd, 0x1234usize as *mut c_void);
        assert_eq!(nid.u_id, TRAY_UID);
        assert_eq!(nid.u_id, 1, "只用 uID = 1（NIF_GUID 全仓零命中）");
        assert_eq!(nid.u_flags, 0x1 | 0x2 | 0x4, "NIF_MESSAGE | NIF_ICON | NIF_TIP，逐字 SI:118");
        assert_eq!(nid.u_flags, 7);
        assert_eq!(nid.u_callback_message, 0x0401, "WM_APP + 1（SI:22）");
        assert_eq!(nid.h_icon, icon);
        let tip: Vec<u16> = "Blade\u{b2}".encode_utf16().collect();
        assert_eq!(&nid.sz_tip[..tip.len()], &tip[..], "szTip 必须逐字等于主干那枚 Blade²");
        assert!(nid.sz_tip[tip.len()..].iter().all(|unit| *unit == 0), "szTip 尾格要留 \\0");
        assert_eq!(nid.u_timeout_or_version, 0, "uVersion 恒 0 才有 lParam = 鼠标消息 的旧语义");
        assert_eq!(nid.dw_info_flags, 0, "建图标那一发不带气球旗（NIF_INFO 归 toast.rs 那颗）");
        assert_eq!(nid.dw_state, 0);
        assert_eq!(nid.dw_state_mask, 0);
        assert_eq!(nid.guid_item, [0; 16]);
        assert!(nid.h_balloon_icon.is_null());
        assert!(nid.sz_info.iter().all(|unit| *unit == 0), "主干 SI:122 szInfo = string.Empty");
        assert!(nid.sz_info_title.iter().all(|unit| *unit == 0), "主干 SI:123");
        // 载荷**不许**被截断逻辑碰过（TB1 §7-7 现测：`TruncateForBalloon` 的算术在漂，但它只喂气球两格）。
        assert_eq!(nid.sz_tip.len(), SZ_TIP_CHARS, "定长缓冲区就是 128 格，不裁");
    }

    /// 销图标那发（`SI:134-142`）**只有五格**：三枚身份/尺寸字段 + 两枚空串。
    /// `uFlags` / `hIcon` / `uCallbackMessage` 一律为 0 —— 补上任何一枚都是多做。
    #[test]
    fn the_delete_payload_carries_no_identifying_flags() {
        let nid = delete_payload(0x9fff);
        assert_eq!(nid.cb_size, size_of::<NotifyIconDataW>() as u32);
        assert_eq!(nid.u_id, TRAY_UID);
        assert_eq!(nid.hwnd, 0x9fffusize as *mut c_void);
        assert_eq!(nid.u_flags, 0, "主干那颗初始化器压根没写 uFlags");
        assert_eq!(nid.u_callback_message, 0);
        assert!(nid.h_icon.is_null());
        assert!(nid.sz_tip.iter().all(|unit| *unit == 0), "销的那一发 szTip = string.Empty");
        assert!(nid.sz_info.iter().all(|unit| *unit == 0));
        assert!(nid.sz_info_title.iter().all(|unit| *unit == 0));
        assert_eq!(nid.dw_state, 0);
        assert_eq!(nid.dw_state_mask, 0);
        assert_eq!(nid.u_timeout_or_version, 0);
        assert_eq!(nid.dw_info_flags, 0);
        assert_eq!(nid.guid_item, [0; 16]);
        assert!(nid.h_balloon_icon.is_null());
    }

    /// 恒 0 那三枚（`guidItem` / `uVersion` / `dwState`）在**产品码**里也只许以零值出现（反锁）。
    #[test]
    fn the_never_assigned_fields_are_never_assigned_anywhere() {
        let code = production_code();
        for needle in [
            "dw_state: 0",
            "dw_state_mask: 0",
            "u_timeout_or_version: 0",
            "dw_info_flags: 0",
            "guid_item: [0; 16]",
            "h_balloon_icon: std::ptr::null_mut()",
        ] {
            assert_eq!(code.matches(needle).count(), 2, "两发载荷各一颗，多一处就是有人开始赋值：{needle}");
        }
        for forbidden in [
            "NIF_GUID",
            "NIM_SETVERSION",
            "NIM_SETFOCUS",
            concat!("NOTIFYICON_", "VERSION"),
            "NIN_SELECT",
            concat!("NIS_", "HIDDEN"),
            concat!("NIS_", "SHOW"),
            concat!("NIF_", "SHOWTIP"),
            concat!("NIF_", "REALTIME"),
            "dw_state: 1",
            "u_timeout_or_version: 4",
            "guid_item: [1",
        ] {
            assert!(!code.contains(forbidden), "主干没有的现代做法长出来了：{forbidden}（TB-4 / TB-5 / TB-11）");
        }
    }

    // -------------------------------------------------------------------- 三、结构与尺寸的形制锁

    /// x64 上的布局：现算 **976** 字节 + 十五颗字段的偏移，逐格钉死。
    ///
    /// **为什么不是简报里的 984**（`tmp/tb1-spec.md:702` 的那枚数是误测，纠正登记报告 §9）——两条独立 oracle：
    /// 1. 本机 SDK 头 `Windows Kits\10\Include\10.0.22621.0\um\shellapi.h`：`:56-58` 是
    ///    `#if !defined(_WIN64)` / `#include <pshpack1.h>` / `#endif` ⇒ **pack(1) 只在 32 位生效**，
    ///    x64 走自然对齐；`:1041-1071` 那十五颗按自然对齐相加 =
    ///    `4(+4 垫) 8 4 4 4(+4 垫) 8 256 4 4 512 4 128 4 |GUID 16| 8` = **976**（x86 才是 pack(1) 的 956）。
    /// 2. 货架里微软自己的官方绑定 `~/.cargo/registry/.../windows-sys-0.61.2/src/Windows/Win32/UI/Shell/mod.rs`
    ///    （元数据派生，等于头文件的真形）：x86_64 档写 `#[repr(C)]`、**只有** `target_arch = "x86"` 档写
    ///    `#[repr(C, packed(1))]` —— 正是那颗 `#if !defined(_WIN64)` 的镜像。
    ///
    /// `guid_item` 用 `[u8; 16]`（对齐 1）代 C 的 `GUID`（对齐 4）：它前面正好停在 952，而 `952 % 4 == 0`
    /// ⇒ 那一格与 C **同偏移**，十五颗偏移全同 ⇒ `cbSize` 与主干 `Marshal.SizeOf` 同值，无一位错车。
    #[test]
    fn the_struct_is_the_x64_mainline_size() {
        assert_eq!(size_of::<NotifyIconDataW>(), 976, "x64 自然对齐的 NOTIFYICONDATAW 现算 976（shellapi.h:56-58 + windows-sys 官方绑定两条 oracle）");
        assert_eq!(std::mem::align_of::<NotifyIconDataW>(), 8, "x64 上有指针 ⇒ 结构体对齐 8");
        assert_eq!(
            std::mem::offset_of!(NotifyIconDataW, h_balloon_icon) + size_of::<*mut c_void>(),
            size_of::<NotifyIconDataW>(),
            "末格之后还有垫 ⇒ 发出去的 cbSize 比真实字节多，主干那枚不是这个形"
        );
        let offsets = [
            (std::mem::offset_of!(NotifyIconDataW, cb_size), 0),
            (std::mem::offset_of!(NotifyIconDataW, hwnd), 8),
            (std::mem::offset_of!(NotifyIconDataW, u_id), 16),
            (std::mem::offset_of!(NotifyIconDataW, u_flags), 20),
            (std::mem::offset_of!(NotifyIconDataW, u_callback_message), 24),
            (std::mem::offset_of!(NotifyIconDataW, h_icon), 32),
            (std::mem::offset_of!(NotifyIconDataW, sz_tip), 40),
            (std::mem::offset_of!(NotifyIconDataW, dw_state), 296),
            (std::mem::offset_of!(NotifyIconDataW, dw_state_mask), 300),
            (std::mem::offset_of!(NotifyIconDataW, sz_info), 304),
            (std::mem::offset_of!(NotifyIconDataW, u_timeout_or_version), 816),
            (std::mem::offset_of!(NotifyIconDataW, sz_info_title), 820),
            (std::mem::offset_of!(NotifyIconDataW, dw_info_flags), 948),
            (std::mem::offset_of!(NotifyIconDataW, guid_item), 952),
            (std::mem::offset_of!(NotifyIconDataW, h_balloon_icon), 968),
        ];
        for (got, want) in offsets {
            assert_eq!(got, want, "偏移漂了就会把 `cbSize` 之外的一切字节错位");
        }
    }

    /// `cbSize` 的**算法**锁（比数值锁更硬）：产品码里那两格必须从 `size_of` 现取，写字面量就红。
    #[test]
    fn the_struct_size_comes_from_size_of_never_a_literal() {
        let code = production_code();
        assert_eq!(
            code.matches("cb_size: size_of::<NotifyIconDataW>() as u32").count(),
            2,
            "两发载荷各一颗，且都必须从 size_of 现取"
        );
        for forbidden in [
            "cb_size: 984",
            "cb_size: 976",
            "cb_size: 968",
            "cb_size: 88",
            "cb_size: size_of::<u32>()",
            "cb_size: TRAY_UID",
        ] {
            assert!(!code.contains(forbidden), "把 cbSize 钉成字面量：{forbidden}");
        }
    }

    /// 字段序锁（照 `toast.rs:431-464` 那把的形状，但**只数本文件**）：主干 `SI:287-301` 十五颗同序。
    #[test]
    fn the_struct_field_order_mirrors_the_mainline_declaration() {
        let code = production_code();
        let start = code.find("struct NotifyIconDataW {").expect("找不到 FFI 结构体（声明漂了）");
        let rest = &code[start..];
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
            let at = body.find(field).expect("字段丢了");
            assert!(at > previous, "字段序漂了：{field} 不在它那一格（cbSize 靠序不靠名字）");
            previous = at;
        }
        assert!(code.contains("#[repr(C)]\n#[derive(Debug)]"), "repr(C) 是 cbSize 成立的唯一凭据");
        assert!(!code.contains("Default"), "又去 derive/调用 Default：数组缓冲区拿不到，逐颗写字段才是主干那形");
    }

    // ------------------------------------------------------------------------ 四、消息档与两发出口

    /// **本模块只发两档**（`NIM_ADD` / `NIM_DELETE`）、零 `NIM_MODIFY`（防与 `toast.rs` 长成两份真相）、
    /// 零第二颗入口（主干 `NIM_ADD` 1 发 / `NIM_DELETE` 1 发 ⇒ 这里各只发一次）。
    #[test]
    fn this_module_sends_exactly_two_message_kinds() {
        let code = production_code();
        assert!(code.contains(concat!("#[link(", "name = \"shell32\")")), "shell32 的 #[link] 块没了（要引依赖就得先过用户）");
        assert!(code.contains(concat!("#[link(", "name = \"user32\")")), "user32 的 #[link] 块没了（DestroyIcon 要它）");
        assert_eq!(code.matches("Shell_NotifyIconW").count(), 3, "声明 1 + 建 1 + 销 1：多一颗入口就是第二份真相");
        assert_eq!(code.matches("NIM_ADD").count(), 2, "1 颗常量 + 1 发引用");
        assert_eq!(code.matches("NIM_DELETE").count(), 2, "1 颗常量 + 1 发引用");
        assert_eq!(code.matches("NIM_MODIFY").count(), 0, "气球那一档归 toast.rs:45，本模块不许发也不许重定义");
        assert_eq!(code.matches("NIF_INFO").count(), 0, "NIF_INFO 归气球那颗（SI:193）");
        assert_eq!(code.matches("ExtractIconExW").count(), 2, "声明 1 + 取图标 1：hIcon 只有这一颗来源（TB-7）");
        assert_eq!(code.matches("DestroyIcon").count(), 2, "声明 1 + remove() 里 1 发");
        assert_eq!(code.matches("LoadImage").count(), 0, "LoadImageW 只许当单测的第二口径，不许长成产品码的 hIcon 来源");
        assert!(code.contains(&["Shell_Notify", "IconW(", "NIM_ADD", ", &nid)"].concat()), "建图标那发的实发形制漂了");
        assert!(code.contains(&["Shell_Notify", "IconW(", "NIM_DELETE", ", &nid)"].concat()), "销图标那发的实发形制漂了");
        assert!(!code.contains("#[test]"), "切分锚失效：产品码窗里混进了测试码，上面那些计数全成假绿");
        for needle in [
            "const NIM_ADD: u32 = 0x00;",
            "const NIM_DELETE: u32 = 0x02;",
            "const NIF_MESSAGE: u32 = 0x1;",
            "const NIF_ICON: u32 = 0x2;",
            "const NIF_TIP: u32 = 0x4;",
            "const WM_TRAY_CALLBACK: u32 = 0x0400 + 1;",
            "const TRAY_UID: u32 = 1;",
            "const SZ_TIP_CHARS: usize = 128;",
            "const SZ_INFO_CHARS: usize = 256;",
            "const SZ_INFO_TITLE_CHARS: usize = 64;",
            "const ASSET_ROOT_UPWARD_PROBES: usize = 6;",
            "u_id: TRAY_UID",
            "u_flags: NIF_MESSAGE | NIF_ICON | NIF_TIP",
            "u_callback_message: WM_TRAY_CALLBACK",
        ] {
            assert!(code.contains(needle), "形制漂了：{needle}");
        }
        // 串锚之外的一条硬禁：**hIcon 只有 ExtractIconExW 一颗来源**（TB-7 不许自造通道）。
        for forbidden in [
            concat!("Load", "IconW"),
            concat!("IDI_", "APPLICATION"),
            concat!("GetSystemMetrics", "("),
            concat!("CreateIcon", "("),
            concat!("Bitmap", "::new"),
        ] {
            assert!(!code.contains(forbidden), "自造的 hIcon 通道长出来了：{forbidden}");
        }
        // 本刀不引渲染层 / 框架层（成规 = `dock.rs`「纯逻辑层不引渲染层」）。
        for forbidden in [concat!("windows_", "reactor"), concat!("VecDeque", "::"), concat!("thread::sle", "ep")] {
            assert!(!code.contains(forbidden), "多出/借用了不该有的东西：{forbidden}");
        }
        // 主干 `TrayAdd` 不调 `TruncateForBalloon` ⇒ szTip 不截断（§7-7 的现测判语）。
        assert!(!code.contains("truncate"), "szTip 不截断：主干那一发是写死字面量进 128 格缓冲");
    }

    /// **本刀零回调接收端**（模块头那条「今天发出去的消息无人接」的机器可见形态）：
    /// 全文（含测试窗）都不许出现子类化/菜单/唤起那三族符号。⚠ §U-2 探针里那几枚查询串是
    /// **分段现拼**的（成规出处 = `nativepick.rs` 的「禁串一律分段现拼，否则锁扫到自己」）⇒
    /// 这把锁与那枚探针可以共存，而「本刀没装回调」仍然是真话。
    #[test]
    fn the_module_carries_no_receiver_yet() {
        // 扫的是**全文剥注释**（代码没长出那一族 = 真话；注释里必须能提它，否则模块头没法写）。
        // ⚠ 每一枚禁串都**分段现拼**：数组本身就在被扫的那一窗里，字面禁串会被自己扫出来 = 假红
        //   （成规出处 = `nativepick.rs:798`「禁串一律分段现拼」）。
        let code = all_code();
        for forbidden in [
            concat!("SetWindow", "Subclass"),
            concat!("Remove", "WindowSubclass"),
            concat!("Def", "SubclassProc"),
            concat!("comct", "l32"),
            concat!("Create", "PopupMenu"),
            concat!("TrackPopupMenu", "Ex"),
            concat!("AppendMenu", "W"),
            concat!("DestroyMenu", "("),
            concat!("GetCursorPos", "("),
            concat!("WM_SETTING", "CHANGE"),
            concat!("Immersive", "ColorSet"),
            concat!("SetForeground", "Window"),
            concat!("IsIconic", "("),
            concat!("ShowWindow", "("),
            concat!("GWLP_", "WNDPROC"),
            concat!("WH_GET", "MESSAGE"),
        ] {
            assert!(!code.contains(forbidden), "下一批的接收端提前长出来了：{forbidden}（本刀范围外）");
        }
        let code = production_code();
        assert!(!code.contains("0x0202"), "鼠标消息分发不归本刀");
        assert!(!code.contains("0x0205"));
        assert!(!code.contains("0x0402"), "NIN_BALLOONUSERCLICK 那一臂归下一批（TB-9 不许顺手接）");
        assert_eq!(code.matches("WM_TRAY_CALLBACK").count(), 2, "常量 1 + 载荷 1：本刀只写号、不接消息");
    }

    /// 纪律本身也要被钉住：测试窗里**零发** `Shell_NotifyIconW` 的调用形状（三档全禁，含 `NIM_ADD`）。
    /// 真调一发就是在用户托盘里塞真图标 = 违反零前台硬令。
    #[test]
    fn this_module_never_calls_the_notify_api_in_tests() {
        let tests = tests_code();
        // 禁串运行时现拼（`[..].concat()`），否则这一行自己就被自己扫出来。
        let call_shape = ["Shell_Notify", "IconW("].concat();
        assert!(!tests.contains(&call_shape), "测试窗里出现了那一发的调用形状 ⇒ 立刻停手，本仓零前台");
        // 顺带钉住「两档消息号都不许被测试窗拿去当实参」：形状级，不数出现次数（数次数会被改一句
        // 断言文案就漂，那是假紧度）。
        for arg_shape in [["NIM_", "ADD"].concat(), ["NIM_", "DELETE"].concat()] {
            assert!(
                !tests.contains(&["(", arg_shape.as_str(), ", "].concat()),
                "测试窗里出现了把消息档直接实参化的形状 {arg_shape} ⇒ 就是在绕开纯层真调 FFI"
            );
        }
    }

    // ---------------------------------------------------------------------- 五、主干硬序与照抄的缺陷

    /// 主干 `SI:112` → `SI:125` 的次序：句柄**先**落槽、`NIM_ADD` **后**发；而 `add()` 那一窗里
    /// **一次 `DestroyIcon` 都没有** ⇒ 那一发失败时句柄既不被销毁也不被清零（TB-3：照抄才是保真）。
    #[test]
    fn the_failed_add_keeps_the_icon_handle_in_the_slot() {
        let code = production_code();
        let window = add_window(&code);
        let set_at = window.find("ICON.with(|slot| slot.set(icon))").expect("SI:112 那一格没了");
        // needle 分段现拼：本测试那一窗也在「测试窗零真调」那把锁的扫描面里（见 tests_code 的说明）。
        let send_at = window.find(&["Shell_Notify", "IconW(", "NIM_ADD", ", &nid)"].concat()).expect("SI:125 那一发没了");
        assert!(set_at < send_at, "句柄必须在 NIM_ADD **之前**落槽（主干 SI:112 → SI:125）");
        assert_eq!(window.matches("DestroyIcon").count(), 0, "add() 里出现 DestroyIcon = 把主干那枚泄漏「修」掉了");
        assert_eq!(window.matches("slot.set(std::ptr::null_mut())").count(), 0, "同上：失败路径不许清零");
        // 失败那档的**可达性**（纯层）：守卫放行 ≠ 建成 —— `ADDED` 只随返回值落，与主干 `SI:125` 同形。
        assert_eq!(add_step(false, Some(0x20)), AddStep::Proceed { hwnd: 0x20 });
        // 裁定 1 的核心一格：**落旗必须随 `NIM_ADD` 的返回值落**。写死 `true` = 「没建成也谎报建成」，
        // 下一批拿 [`added`] 喂 `toast::Gate::tray_added` 时会把整条气球腿打死（TB-12 同源）。
        assert!(
            window.contains("ADDED.with(|slot| slot.set(sent))"),
            "落旗不随那一发的返回值 = 谎报成功（裁定 1 明令：取不到 ico / 发不出去都要如实回失败态）"
        );
        for lying in ["slot.set(true)", "slot.set(1)", "ADDED.set(true)"] {
            assert!(!window.contains(lying), "谎报成功的形状长出来了：{lying}");
        }
        // 无图标那一支必须在发那一发**之前**回（`AddOutcome::NoIcon`），且不动旗 ⇒ 失败态不夹带半成品状态。
        let no_icon_at = window.find("AddOutcome::NoIcon").expect("无 ico 的失败态没了（裁定 1 不许兜底谎报）");
        assert!(no_icon_at < send_at, "取不到图标就该早退，不许带着空句柄去发 NIM_ADD");
    }

    /// 拆除硬序：`NIM_DELETE`（`SI:143`）**先于** `DestroyIcon`（`SI:146`），且那一发的**返回值被丢弃**
    /// （主干 `SI:143` 前面没有赋值），句柄为零时不销毁（`SI:144`），旗最后落下（`SI:149`）。
    #[test]
    fn the_delete_is_sent_before_the_icon_is_destroyed() {
        let code = production_code();
        let window = remove_window(&code);
        // 禁串/needle 运行时现拼（`tests_code()` 那把锁会扫本窗，字面量写在这里会被自己扫出来）。
        let send_shape = ["Shell_Notify", "IconW(", "NIM_DELETE", ", &nid)"].concat();
        let send_at = window.find(&send_shape).expect("SI:143 那一发没了");
        let destroy_at = window.find("DestroyIcon(icon)").expect("SI:146 那一发没了");
        assert!(send_at < destroy_at, "先摘条目再还句柄；反过来就是把一颗仍被托盘引用的句柄销毁");
        let bare_call = ["unsafe { ", send_shape.as_str(), " };"].concat();
        assert!(window.contains(&bare_call), "主干那一发前面没有赋值（SI:143）⇒ 返回值被丢弃，照抄别改");
        assert_eq!(window.matches("DestroyIcon").count(), 1, "remove() 里一发销毁，多一处就是第二份真相");
        let zero_at = window.find("ICON.with(|slot| slot.set(std::ptr::null_mut()))").expect("SI:147 清零没了");
        let flag_at = window.find("ADDED.with(|slot| slot.set(false))").expect("SI:149 落旗没了");
        assert!(destroy_at < zero_at, "DestroyIcon 之后才清零（SI:146 → SI:147）");
        assert!(zero_at < flag_at, "_trayAdded = false 是最后一步（SI:149）");
    }

    // -------------------------------------------------------------------------- 六、hIcon 取证（裁定 1）

    /// 路径解析成规：起点向上数、**第 6 层（= 上限那一层）命中**、**第 7 层落空**
    /// （层数逐字照 `main.rs` 的 `fn brand_mark_path()` 那套六层回溯）。
    /// 只建目录 + 写一颗假 `app.ico`（本测试只看路径，不喂给任何图标 API）。
    #[test]
    fn the_asset_root_walk_hits_within_six_layers_and_misses_beyond() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("blade2-rs-tr1-walk-{}-{stamp}", std::process::id()));
        // 起点 `root/a/b/c/d/e`：向上探针依次落 e→d→c→b→a→root，**第 6 发**正好是上限 ⇒ 命中。
        let deep = root.join("a").join("b").join("c").join("d").join("e");
        std::fs::create_dir_all(&deep).expect("起点目录建不出来");
        std::fs::create_dir_all(root.join("Assets")).expect("假资产根建不出来");
        std::fs::write(root.join("Assets").join(ASSET_ICON_FILE), b"not a real icon").expect("假图标写不下去");
        let hit = icon_path_from_dir(&deep).expect("第 6 层（上限内）该命中资产根");
        assert_eq!(hit, root.join("Assets").join(ASSET_ICON_FILE));
        assert!(hit.is_file());
        // 再深两层 ⇒ `root` 落到第 8 发 ⇒ 六层上限挡死 ⇒ 如实 None，也就是裁定 1 备案的那枚可见差。
        let too_deep = deep.join("f").join("g");
        std::fs::create_dir_all(&too_deep).expect("深目录建不出来");
        assert!(icon_path_from_dir(&too_deep).is_none(), "超过 ASSET_ROOT_UPWARD_PROBES 层就该回 None（不许自造兜底）");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 真盘上的那一发：分叉仓里 `Assets/app.ico` **确在**（六层回溯从测试可执行文件向上真的命中），
    /// 而且**内容真的是一颗图标**（`.ico` 的头部形状：`reserved=0 / type=1 / 张数 ≥ 1`）。
    /// ⚠ 这里**不钉字节总数**（本仓硬教训：总数一夜五变 ⇒ 拿形状串锚，不拿尺寸当判据）。
    #[test]
    fn the_real_asset_resolves_from_the_repo_tree() {
        let path = app_icon_path().expect("分叉测试跑在仓树里，六层回溯该命中 Assets/app.ico");
        assert_eq!(path.file_name().expect("图标路径没有文件名").to_string_lossy(), ASSET_ICON_FILE);
        assert_eq!(path.parent().expect("图标不在目录下").file_name().expect("资产根缺名字").to_string_lossy(), "Assets");
        assert!(path.is_file(), "命中了却不是文件");
        let bytes = std::fs::read(&path).expect("命中了却读不到内容");
        assert!(bytes.len() > 6, "太短就不是图标");
        assert_eq!(&bytes[..4], &[0, 0, 1, 0], "ICONDIR 头：reserved 两字节为 0 + type=1（图标）");
        assert_ne!(bytes[4], 0, "目录里的张数为 0 ⇒ 这颗文件不是可用图标源");
    }

    /// **hIcon 双口径**（本仓硬教训：Win32 句柄/常量抄错也会静默失败 ⇒ 必附第二条独立 oracle）：
    /// ① 产品码那颗 `ExtractIconExW(path, 0, …)`；② 单测专用的 `LoadImageW(…, LR_LOADFROMFILE)`。
    /// 两条都只**加载**（不显示、不建条目），断言的是**非空句柄**本身，不是「函数返回成功」。
    /// 两颗句柄都按主干那颗 `DestroyIcon` 还掉（测试不留泄漏；产品码那条泄漏是照抄主干的，见上）。
    #[test]
    fn the_icon_handle_is_non_null_by_two_independent_routes() {
        let path = app_icon_path().expect("本测试要在仓树里跑（裁定 1 的路径成规）");
        let by_extract = extract_tray_icon(&path).expect("口径①：ExtractIconExW 对该 ico 必须交出句柄");
        assert!(!by_extract.is_null());
        let wide = wide_units(&path.to_string_lossy());
        let by_load = unsafe {
            LoadImageW(
                std::ptr::null(),
                wide.as_ptr(),
                IMAGE_ICON,
                0,
                0,
                LR_LOADFROMFILE | LR_DEFAULTSIZE,
            )
        };
        assert!(!by_load.is_null(), "口径②：LoadImageW(LR_LOADFROMFILE) 交出零句柄 = 那颗文件根本不是有效图标");
        // 两条独立路径给出的都是真 HICON ⇒ 断言完就还掉（零可见副作用、零残留句柄）。
        assert_ne!(unsafe { DestroyIcon(by_extract) }, 0, "DestroyIcon 回零 = 那枚句柄本来就不是图标句柄");
        assert_ne!(unsafe { DestroyIcon(by_load) }, 0);
        // 反证档：非 PE / 无图标组的文件走同一发 FFI 必须被守卫挡回（无 ico ⇒ 如实失败态）。
        let cargo = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert!(extract_tray_icon(&cargo).is_none(), "对着一颗 .toml 取到句柄 = 守卫形同虚设");
    }

    /// **U-1 的现测**（TB1 §6 只做了 PE 静态解析、明写「没有真调过一次 ExtractIconExW」）：
    /// 拿**当前测试可执行文件**（与 `blade2-rs.exe` 同一条 rustc 链接路径：`.rsrc` 段缺失、资源目录
    /// `rva=0 size=0`，本轮 TR1 用 `tmp/tr1-report.md` §5 那条 perl 尺现测）真调一次 ⇒ 必须取不到句柄。
    /// 这一格把裁 B 的**必要性**从「由无资源段强推断」变成实测：主干 `SI:108` 那发对分叉必早退。
    #[test]
    fn the_fork_executable_itself_yields_no_icon_handle() {
        let exe = env::current_exe().expect("测试跑不起来才拿不到自己的路径");
        assert!(
            extract_tray_icon(&exe).is_none(),
            "分叉 exe 竟取到了图标句柄 ⇒ 说明它已带 .rsrc 图标资源，裁定 1 的前提需要重开"
        );
    }

    /// [`extract_tray_icon`] 的守卫与 [`icon_from_extract`] 的**一致**性（把纯层真值表和真 FFI 缝起来）：
    /// 取不到时既不落句柄也不 panic（裁定 2 的「零 unwrap / 零 panic」在图标这一腿同样成立）。
    #[test]
    fn a_missing_asset_yields_no_handle_without_panicking() {
        let nowhere = std::env::temp_dir().join("blade2-rs-tr1-definitely-absent-file.ico");
        assert!(extract_tray_icon(&nowhere).is_none(), "不存在的路径必须走守卫那一支");
        assert!(icon_from_extract(0, std::ptr::null_mut()).is_none());
    }

    // ------------------------------------------------------------ 七、U-2 最小探针（下一批的前置）

    /// **U-2 最小探针**（TB1 §8-5 明写「写者刀第一步应是这枚最小探针，不是写代码」）：
    /// 不建窗、不 subclass，只查**符号**与**版本**在不在 —— 分叉 exe 无 `.rsrc` ⇒ 无内嵌 manifest
    /// （perl 尺现测）⇒ 下一批那颗子类化导出到底落不落地，这一格**必须量**不能假设。
    ///
    /// ⚠ 简报转述的前提「`SetWindowSubclass` 是 comctl32 **v6 专有**」在本机**现测为假**：
    /// 两枚 PE 导出表直接解盘（命令与输出记在报告 §5）—— `System32\comctl32.dll`（与 WinSxS
    /// `…_5.82.26100.8941_none_…` 是同一颗文件）119 枚命名导出里 `ImageList_Create` ✓ /
    /// `SetWindowSubclass` ✓ / `TaskDialogIndirect` ✗；WinSxS `…_6.0.26100.9568_none_…` 150 枚里三枚全 ✓。
    /// ⇒ 真正的 v6 判据是 `TaskDialogIndirect` 那一颗，**不是** subclass 族；主干 `app.manifest` 没有
    /// Common-Controls 的 `dependentAssembly` 却在生产里用着 subclass（`SI:40`）—— 到这里已从
    /// 「反向旁证」升成「直接解释」。
    ///
    /// 三条口径互相印证、且**只断言断言得起的**：①kernel32 的一颗必有导出 ⇒ 证 `GetProcAddress` 这条路
    /// 本身可用（它不通就整个探针作废）；②本进程此前是否已加载那一颗 ⇒ 决定读数来自哪一档；
    /// ③模块**实际路径**里那颗 side-by-side 版本号 ⇒ 不受映射档位影响的版本判据。
    /// 两颗入口都只**读**（不发消息、不建窗口、不 subclass）：`GetModuleHandleW` 不惊动加载器，
    /// 不在架才落到 `LoadLibraryExW(…, DONT_RESOLVE_DLL_REFERENCES)`（TB1 §6-U2 给的两种做法之一，
    /// 只映射不跑 DllMain ⇒ 零窗口零 UI）。
    /// ⚠ 本探针**第一轮**那枚「三枚符号全 false」的伪负，根因**在探针自己**（`&str` 的 `as_ptr()`
    /// 不带零结尾 ⇒ `GetProcAddress` 读出界外，见 [`lookup`] 的补零）；修好后符号档可读，
    /// 与 ③ 的版本档、与报告 §5 那两枚 PE 导出表现测三方向同一。
    #[test]
    fn the_next_batch_subclass_symbols_are_measured_not_assumed() {
        // ① 探针自身的可信档：kernel32 必在架，它的一颗命名导出查不到就是这条路漂了，不是模块的问题。
        let kernel = unsafe { GetModuleHandleW(wide_units("kernel32.dll").as_ptr()) };
        assert!(!kernel.is_null(), "连 kernel32 的模块句柄都拿不到 ⇒ 本进程的加载器状态异常，探针作废");
        let kernel_control = lookup(kernel, concat!("GetModule", "FileNameW"));
        assert!(kernel_control.is_some(), "kernel32 的必有导出查不到 = GetProcAddress 这条路本身不可信（探针作废并登记）");

        // ② 先只问「此前已在架吗」（不惊动加载器）；不在架才落到只映射不解析的那一档。
        let direct = unsafe { GetModuleHandleW(com_concat_name().as_ptr()) };
        let loaded_before = !direct.is_null();
        let module = if loaded_before {
            direct
        } else {
            unsafe { LoadLibraryExW(com_concat_name().as_ptr(), std::ptr::null_mut(), DONT_RESOLVE_DLL_REFERENCES) }
        };
        assert!(!module.is_null(), "两颗入口都拿不到句柄 ⇒ 这颗库在本机不存在（下一批的前置要重开）");
        let path = module_file_path(module);
        let control = lookup(module, concat!("ImageList_", "Create"));
        let subclass = lookup(module, concat!("SetWindow", "Subclass"));
        let task_dialog = lookup(module, concat!("TaskDialog", "Indirect"));
        let symbols_readable = control.is_some();

        // ③ 版本判据：WinSxS 目录名里那颗 `5.82.*` / `6.0.*`（不依赖符号档）。
        let version = assembly_version_from_path(&path);
        let five = version.as_deref().is_some_and(|v| v.starts_with("5."));
        let six = version.as_deref().is_some_and(|v| v.starts_with("6."));
        assert!(five || six, "路径里读不出 5.82 / 6.0 ⇒ 版本判据落空，这一格要如实登记：{path}");
        if symbols_readable {
            assert_eq!(
                task_dialog.is_some(),
                six,
                "符号档与版本档互相矛盾（v6 专有那颗是 TaskDialogIndirect）⇒ 加载器状态异常，读数不作判据"
            );
        }

        // 现测量打进 stdout（报告 §5 引用的就是这一行）；串本身分段现拼，别撞另一把锁。
        eprintln!(
            "U-2 现测｜此前已加载 = {}｜模块 = {path}｜版本 = {:?}｜{} = {}｜{}{} = {}｜{} = {}｜符号档可读 = {}（不可读 ⇒ 本次符号读数不作判据，取版本档）",
            loaded_before,
            version,
            ["ImageList_", "Create"].concat(),
            control.is_some(),
            ["SetWindow", "Sub"].concat(),
            "class",
            subclass.is_some(),
            ["TaskDialog", "Indirect"].concat(),
            task_dialog.is_some(),
            symbols_readable
        );
    }

    /// WinSxS 目录名里那颗版本号（`…common-controls_6595b64144ccf1df_5.82.26100.8941_none_…`
    /// ⇒ `5.82.26100.8941`）；非 side-by-side 那一路（目录名没这颗令牌）⇒ `None`，探针要如实记下。
    fn assembly_version_from_path(path: &str) -> Option<String> {
        let tail = path.split("_6595b64144ccf1df_").nth(1)?;
        let version = tail.split('_').next()?;
        (!version.is_empty()).then(|| version.to_string())
    }

    /// `comctl32.dll` 的模块名宽串：**分段现拼**（同 `nativepick.rs` 的成规）—— 本文件那把
    /// 「本刀没装回调」的锁（[`the_module_carries_no_receiver_yet`】）扫的是全文，原样写会被自己扫出来。
    fn com_concat_name() -> Vec<u16> {
        wide_units(concat!("comct", "l32.dll"))
    }

    /// 两颗入口都只**读**（`GetModuleHandleW` 不惊动加载器 / `LoadLibraryExW(…, DONT_RESOLVE_DLL_REFERENCES)`
    /// 只映射不跑 DllMain ⇒ 零窗口零 UI，TB1 §6-U2 给的做法之一），走的是哪一档由探针自己记下。
    /// 这一颗只是把句柄翻成路径 ⇒ 版本判据（`assembly_version_from_path`）靠的就是它。
    fn module_file_path(module: *mut c_void) -> String {
        let mut buf = vec![0u16; 1024];
        let len = unsafe { GetModuleFileNameW(module, buf.as_mut_ptr(), buf.len() as u32) };
        if len == 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..len as usize])
    }

    /// `GetProcAddress` 要的是**以零结尾**的 ANSI 名字：`&str` 的 `as_ptr()` **不带**结尾零，
    /// 直接递过去就是本探针**第一轮**那枚「三枚全 false」伪负的根因（本轮现测定位 —— 连 kernel32
    /// 必有的那颗都查不到，才把矛头从模块转向探针自己）。这里现补零再递，只读、不解析、不发消息。
    fn lookup(module: *mut c_void, name: &str) -> Option<*const c_void> {
        let mut bytes = name.as_bytes().to_vec();
        bytes.push(0);
        let addr = unsafe { GetProcAddress(module, bytes.as_ptr() as *const i8) };
        (!addr.is_null()).then_some(addr as *const c_void)
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        // 只给上面那枚探针用（`//` 而非 `///`：rustc 不给 extern 块生成文档，`///` 会叫
        // `unused_doc_comments` 判一条警告）。`scroll.rs:134` 也有一份同名声明，但那是 scroll 模块的
        // 私有颗 ⇒ 跨模块各声明一份是仓内成规（见 `nativepick.rs` 的 SHGetKnownFolderPath 同款注释）。
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
        fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
        fn GetModuleFileNameW(module: *mut c_void, buffer: *mut u16, size: u32) -> u32;
        fn GetProcAddress(module: *mut c_void, name: *const i8) -> *const c_void;
    }

    const DONT_RESOLVE_DLL_REFERENCES: u32 = 0x0000_0001;

    // 第二口径（hIcon 用）的那颗 user32 导出：**只在测试里声明**，因为产品码的 hIcon 来源
    // 只有 `ExtractIconExW` 一颗（TB-7；`this_module_sends_exactly_two_message_kinds` 反锁 `LoadImage` 零命中）。
    // ⚠ 这里用 `//` 不用 `///`：rustc 不给 extern 块生成文档（同产品码那两颗 `#[link]` 块的理由）。
    #[link(name = "user32")]
    unsafe extern "system" {
        fn LoadImageW(
            instance: *const c_void,
            name: *const u16,
            image_type: u32,
            desired_x: i32,
            desired_y: i32,
            flags: u32,
        ) -> *mut c_void;
    }

    /// `winuser.h`：`IMAGE_ICON = 1` / `LR_LOADFROMFILE = 0x0010` / `LR_DEFAULTSIZE = 0x0040`。
    /// 数值**不许凭记忆抄** ⇒ 本测试断言的是「拿到的句柄非空」这一可观测结果，常量抄错就是当场红。
    const IMAGE_ICON: u32 = 1;
    const LR_LOADFROMFILE: u32 = 0x0010;
    const LR_DEFAULTSIZE: u32 = 0x0040;
}
