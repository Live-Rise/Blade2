//! 滚轮通道：分叉**唯一**可用的滚动出口。
//!
//! ## 为什么只能走这一条
//! `windows-reactor 0.100.0` 一个滚动 API 都没有：`ScrollViewer` 的公开面只有
//! `horizontal/vertical_scroll_bar_visibility` 两根属性（`generated.rs:2681-2711`，全 crate 搜
//! `ScrollIntoView` / `scroll_into_view` 0 命中，`SetScrollOffsets` / `ScrollableLength` /
//! `IScrollView` 都只在 `pub(crate) native::winui::bindings` 里，而 `View::native` 是
//! `pub(crate)` ⇒ 拿不到原生对象）。`ListView` 也没有 `ScrollIntoView`；`FocusControl`
//! （`request_focus` 那个 trait）只实现在输入控件上、容器一个都没有 ⇒ 「抢焦点让框架自己滚进来」
//! 这条路对列表/面板根本不成立。主干那三处 `ScrollIntoView`（`ChatList` / `CommandList` /
//! `ReferenceList`）在分叉里都没有对应物。
//!
//! 实测可行的是**这一发**：`SetCursorPos(落点)` + `SendInput(MOUSEEVENTF_WHEEL)` —— 系统按光标位置
//! 把它路由进 XAML 的输入栈。两条在案证据（都还能对着文件复核）：
//! * `tmp/keys-probe-report.txt:14` 的 `SCROLL-AFTER-REALWHEEL: Pane aid=ProbeScroll … counts=[,,100]`
//!   ⇒ 真滚轮把那条 `ScrollViewer` 的百分比读数从 0 推到 100，注入与真鼠标同值；
//! * `tmp/qa6-report.txt:60-62`「滚轮双向可用：SendInput 在 1600,800 打 12 格 down →
//!   `qa6-p3-charlie` 落到 rect=2434,1137 **off=False**；12 格 up 到顶后再补 3×12 格 up，首行 rect
//!   不变（=2391,227）⇒ 已夹紧；可见行数 顶=70 / up=65 / down=63」
//!   —— 这一条同时是 #89 的**机制已通**证明：正文带里打一发行得通，缺的只是「谁来打」
//!   （对照 `tmp/qa6-report.txt:68-77` 那句 FAIL：回读/落地之后**没人补这一发**）。
//!   像素侧另有一组独立证据：`tmp/qa5-wheel-{body,rail}-{before,after}.png` 四张的 md5 恰好配成两对
//!   （`body-before == rail-after`、`body-after == rail-before`）⇒ 正文一发把画面从 X 推到 Y，
//!   随后瞄准轮次轨那一发又把它推回 X（同一段几何也顺手解释了 #64 为什么存在：轨的命中区和正文带
//!   右缘重叠，落点不扣掉轨宽就会滚错容器）。
//! 代价有两条，都绕不过去：
//! * **只有相对量**：没有 `ChangeView`、没有绝对偏移 ⇒ 只能「补 N 格、期望落到该落的地方」，
//!   绝对贴底那条因此要走「故意过滚 + 让 `ScrollViewer` 自己夹到边界」；
//! * **落点由光标位置决定**，与焦点无关（WinUI 3 桌面窗口的 XAML 内容挂在
//!   `InputSiteWindowClass` / `DesktopChildSiteBridge` / `InputNonClientPointerSource` 这几个
//!   子窗口下）⇒ 点给错了就是滚错区域，比不滚更糟。所以本模块默认**拒绝**在点不可信时发消息。
//!
//! ## #64：`[SCROLL] hits=0` 那条读数是**坏仪器**，不是「路由不通」
//! 本模块从建立起用的都是 `PostMessageW`，qa6 那一轮因此报「发了但没滚」。把那一轮的原始产物
//! 重新对了一遍，定性如下（三条都对着文件，不再引对不上号的那份）：
//! * `tmp/qa6-wheel-wheeldown1.txt` / `…-wheelup-clamp.txt` 打的是
//!   `WHEEL notches=12 sent=12 at=1600,800` + `LOGFILTER [SCROLL] hits=0`，而**同一次运行**的
//!   `tmp/qa6-dump-after-wheel{down,up}.txt` 里同一行「⚠ provider_rate_limited…」分别落在
//!   y=1289 与 y=1221（差 68 物理像素），可见行数 54 vs 56 ⇒ **画面确实被滚走了**。
//!   分叉自持日志一条不响的真正原因是：那一版里就没有任何 `SCROLL ` 前缀的产出点
//!   （`grep -rn "MOUSEWHEEL\|0x020A\|pointer_wheel" ~/.cargo/registry/src/*/windows-reactor-0.100.0/src`
//!   也是零命中 —— 框架侧压根不经过我们的代码）。⇒ **hits=0 只证明计数器没接线，不能拿来判路由。**
//!   ⇒ #64 的收尾必须把判据换成「注入前后 UIA 矩形之差」（`tmp/qa6-report.txt:60-62` 那种口径），
//!   日志只做旁证。
//! * ⚠ 原稿此处那句「`WindowFromPoint` 给 bridge ⇒ 类名白名单这条路根本不成立」**已被推翻**：
//!   命中谁 ≠ 谁收得到滚轮。qs1 把 9 颗窗口逐个直投测正/反向，只有 input-site 真滚
//!   （锚点 `dy=±1188`、抓图 md5 变），顶层与 bridge 全 `dy=0` ⇒ `tmp/qs1-report.md §3`。
//!   兜底直投的目标判据因此**就是**类名子串 `InputSite`（[`pick_post_target`]）；`SendInput`
//!   仍是主通路（主干忠实行为），这里只把注入被挡时的那一发对准会收的窗口。
//!   附带硬条件：`lParam` 必须带屏幕物理像素点 —— 同一颗 input-site，`lp=0` 那一发 `dy=0`。
//!
//! 所以本模块的**唯一有效出口**是 [`inject_wheel`]：`Route::Inject` 打头，`PostMessageW` 降级到
//! 只在注入被系统挡掉时补一发，并且把 `route=` 如实写进日志（`ROUTE_INJECT` / `ROUTE_POST`）。
//! 归属判定保留，但**不写成类名白名单**：[`pick_route`] 只问「`WindowFromPoint` 命中的是不是本进程、
//! 挂在本顶层下的窗口」，是 ⇒ 注入，不是 ⇒ [`Route::Drop`]（绝不往别人进程的窗口里滚）。
//! 归属不含类名，**投递目标**含（[`pick_post_target`] 认 `InputSite` 那截子串）。
//! 投递结果由 [`Posted::target_class`] + [`Posted::route`] 自证。
//!
//! ## #89：新气泡落在视口外（分叉此前**没有**任何贴底通路）
//! 主干的语义抄在 [`Stick`] 与 [`stick_action`] 的文档里（MW `MainWindow.xaml.cs:5721-5775`）：
//! 判据是**每次现算的几何量** `ScrollableHeight - VerticalOffset <= 96`，没有「用户上翻就锁死」
//! 那个闩锁 —— 上翻停止跟随、滚回底部自动恢复都是同一句判据的两个方向。分叉读不回
//! `VerticalOffset`（框架零滚动 API），于是 [`Stick`] 改成记账式的 `gap_dip`（离底部还差多少 DIP），
//! 「什么时候跟随 / 什么时候停 / 什么时候恢复」三条各自是一个纯函数出口，逐条有 `#[test]`。
//!
//! ⚠ 读滚动状态时 `VerticalPercentScrolled` 恒为 0，可用的是 `VerticalScrollPercent`（同份证据）。
//!
//! ## 分工
//! * [`wheel_w_param`] / [`wheel_l_param`] / [`clamp_notches`] / [`clamp_burst`] / [`dip_to_px`] /
//!   [`anchor_to_point`] / [`rows_visible`] / [`notches_for_dip`] / [`keep_in_view`] /
//!   [`pick_route`] / [`pick_post_target`] / [`user_wheel_notches`] / [`merge_user_wheel`] /
//!   [`stick_action`] 全是**纯函数**：打包、符号、夹取、量化、归属判定、
//!   贴底状态机，`cargo test` 不起 GUI 就能钉死。
//! * [`hwnd`] / [`cursor`] / [`client_origin`] / [`wheel_target`] / [`input_site_child`]
//!   / [`inject_wheel`] / [`post_wheel`] / [`post_wheel_at_window_anchor`] / [`install_wheel_watch`] 才碰 Win32
//!   （类名判定的**纯**那半截在 [`class_label`]）。
//!
//! 零依赖：所有 Win32 入口沿用 `keys.rs` 的裸 `extern "system"` + `#[link(name = "user32")]` 写法，
//! **不新增 Cargo 依赖**（`--offline` 必须继续可用，`Cargo.lock` 里没有 `windows` 主 crate）。
//!
//! ## 不许 panic
//! [`enum_proc`] / [`wheel_proc`] 与 [`post_wheel`] 都跑在 UI 线程的消息泵里
//! （`EnumThreadWindows` 与钩子 proc 是直接回调，低级钩子还在系统输入线程之上）。穿过
//! `extern "system"`
//! 边界抛异常 = 进程直接没了、stdout 0 字节（`0xC000027B`，`keys.rs` 头注同一条教训）。
//! 所以：回调体整个套 [`std::panic::catch_unwind`]，Win32 返回值一律 `is_null()` / `== 0` 判，
//! **没有一个 `unwrap()`**，量化那几步全是 `saturating_*` + `is_finite()`。

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::panic::{self, AssertUnwindSafe};

// ---------------------------------------------------------------- Win32 入口

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumThreadWindows(thread: u32, cb: *const c_void, l_param: *mut c_void) -> i32;
    fn GetClassNameW(hwnd: *mut c_void, buf: *mut u16, max: i32) -> i32;
    fn GetCurrentThreadId() -> u32;
    fn GetCursorPos(point: *mut WinPoint) -> i32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn GetWindowRect(hwnd: *mut c_void, rect: *mut WinRect) -> i32;
    /// 客户区宽高（**物理像素**，坐标原点无关，只用 `right-bottom`）。#89 那条「正文带在客户区
    /// 里的矩形」要有它才能算出带子的右下界；`GetWindowRect` 给的是含边框的整窗，替不了。
    fn GetClientRect(hwnd: *mut c_void, rect: *mut WinRect) -> i32;
    fn ClientToScreen(hwnd: *mut c_void, point: *mut WinPoint) -> i32;
    fn GetDpiForWindow(hwnd: *mut c_void) -> u32;
    fn PostMessageW(hwnd: *mut c_void, msg: u32, w_param: usize, l_param: *mut c_void) -> i32;
    /// 屏幕点 → **最深层**那个窗口（含子窗口）。系统自己的鼠标路由用的就是它。
    fn WindowFromPoint(point: WinPoint) -> *mut c_void;
    /// 窗口属主进程 PID（第二参数可以是空指针，这里只要 PID）。
    fn GetWindowThreadProcessId(hwnd: *mut c_void, process_id: *mut u32) -> u32;
    /// **整棵子树**（不只直系子窗口）逐个回调，跨进程也能枚举。找 input-site 用它：那颗窗口挂在
    /// bridge 之下（`tmp/qs1-report.md §1`），只枚举直系子窗口会漏掉。
    fn EnumChildWindows(parent: *mut c_void, cb: *const c_void, l_param: *mut c_void) -> i32;
    /// `GA_ROOT` 用：某个窗口所属的那棵顶层窗口。
    fn GetAncestor(hwnd: *mut c_void, kind: u32) -> *mut c_void;
    /// **系统注入**：把事件塞进与硬件输入同一条队列，由系统按光标位置路由。这是主干忠实行为的
    /// **主通路**，本模块不动它（直投只是注入被挡时的兜底，目标态见 [`pick_post_target`]）。
    /// 返回**真正插入的事件数**（`0` = 全没进去），所以调用方必须按返回值判成败、不许只看 `!= 0`。
    fn SendInput(count: u32, inputs: *const WinInput, size: i32) -> u32;
    /// 低级鼠标钩子（`WH_MOUSE_LL`）三件套 —— 分叉观察「用户自己滚了多少」的唯一出口，
    /// 对应主干的 `ScrollViewer.ViewChanged`。签名与 `keys.rs` 的键盘钩子逐字同形。
    fn SetWindowsHookExW(id_hook: i32, lp_fn: *const c_void, h_mod: *const c_void, thread: u32) -> *mut c_void;
    fn UnhookWindowsHookEx(hhk: *mut c_void) -> i32;
    fn CallNextHookEx(hhk: *mut c_void, code: i32, w_param: usize, l_param: *const c_void) -> usize;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    /// 本进程 PID（`== 0` 只在病态情况下出现，一律当「不可信」处理）。
    fn GetCurrentProcessId() -> u32;
    /// `NULL` → 本模块（exe 自身）的句柄：`WH_MOUSE_LL` 的 `hMod` 要它，探针同一条口径。
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
}

/// `WM_MOUSEWHEEL`。`wParam` 高位字 = 滚轮增量（格数 × [`WHEEL_DELTA`]，向用户方向滚为负），
/// `lParam` = `MAKEPOINTS(screen_x, screen_y)`，**屏幕物理像素**。
pub const WM_MOUSEWHEEL: u32 = 0x020A;

/// 一发滚轮的固定增量（Win32 `WHEEL_DELTA`）。
pub const WHEEL_DELTA: i32 = 120;

/// 单发消息的格数上限。真实滚轮一次手势也就十几格，再多就是把列表甩到头还回不来；
/// 夹住它，越界时最坏是「一次没滚够，下一次按键接着滚」，而不是「滚飞了」。
/// 只用于**相对**通路（浮层跟随、轮次轨点跳）—— 那两条要求「走这么多、不多走」。
pub const MAX_NOTCHES: i32 = 12;

/// **绝对**通路（#89 贴底）的格数上限。为什么允许比 [`MAX_NOTCHES`] 大一个量级：
/// 分叉读不回视口位置，「贴到最底」这件事只能靠**故意过滚**来实现 —— `ScrollViewer`
/// 自己会把偏移夹在 `[0, scrollableHeight]`，多滚的那几格落在边界上就消失了，
/// 而少滚就是「看不见最后一条」这个 bug 本身。90 条 seed 历史 ≈ 96 行 × 64 DIP ≈ 6100 DIP，
/// 按 48 DIP/格要 128 格，所以这里留 200 格的预算（≈ 9600 DIP）。
pub const MAX_BURST_NOTCHES: i32 = 200;

/// `SendInput` 的 `INPUT::type` = `INPUT_MOUSE`。
const INPUT_MOUSE: u32 = 0;

/// `MOUSEINPUT::dwFlags` 的竖向滚轮位（`winuser.h` `MOUSEEVENTF_WHEEL = 0x0800`）。
/// 别写成 `0x0200`：那是**横向**滚轮那一族（`MEMORY.md` 里「正值那一下等于横滚」的坑），
/// 竖向 0x0800 / 横向 0x1000。本模块只发竖向。
const MOUSEEVENTF_WHEEL: u32 = 0x0800;

/// `WM_MOUSEHWHEEL`：横向滚轮。低级钩子看得见它，但分叉所有滚动带都是竖向的 ⇒ 一律不记账。
const WM_MOUSEHWHEEL: usize = 0x020E;

/// `WH_MOUSE_LL`：系统级低级鼠标钩子。它**看得见 `SendInput` 注入的那一发**（`LLMHF_INJECTED`），
/// 也看得见真滚轮 —— 这是分叉唯一能同时观察到「用户滚了多少」与「我自己滚了多少」的出口，
/// 顶替主干的 `ScrollViewer.ViewChanged`（`MainWindow.TurnRail.cs:34-90` 用它在采样视口位置）。
///
/// 与 `WH_KEYBOARD_LL` 同一条约束（探针实测口径 `tmp/probe-keys.rs.bak:185`）：`hMod` 传本模块句柄、
/// **`dwThreadId` 必须传 0**（低级钩子是系统级的，传线程 id 会被 `SetWindowsHookExW` 拒掉）。
/// 回调仍在**装它的那个线程**上被调用（所以 `thread_local!` 的账本是对的），代价是这个线程必须
/// 在泵消息 —— UI 线程满足。
const WH_MOUSE_LL: i32 = 14;

/// 钩子 proc 的 `code == HC_ACTION`（低级钩子只有这一种 code 会带真事件）。
const HC_ACTION: i32 = 0;

/// `MSLLHOOKSTRUCT::flags` 的 `LLMHF_INJECTED` 位：这一发是 `SendInput` 注进来的。
/// 分叉自己那发贴底滚轮会被**同一个钩子**看见，不认这一位就会把「我滚的」记成「用户滚的」，
/// 贴底账本立刻假。**但只认这一位不够**（测试驱动也用 `SendInput`）⇒ 判据在 [`is_own_wheel`]。
const LLMHF_INJECTED: u32 = 0x10;

/// WinUI 3 桌面窗口的顶层窗口类（实测 `tmp/keys-probe-report.txt` 第一行
/// `class=WinUIDesktopWin32WindowClass`）。所有 XAML 内容都在它下面。
const WINDOW_CLASS: &str = "WinUIDesktopWin32WindowClass";

/// 96 DPI = 100% 缩放，即 1 DIP == 1 物理像素。`GetDpiForWindow` 取不到时的回落值。
const DPI_AT_100: f64 = 96.0;

/// XAML 真正**吃鼠标输入**的那个子窗口类（`WindowFromPoint` 落在内容区上时返回的就是它或它的
/// 后代）。本机实测到的是短名（`tmp/keys-probe-report.txt`：
/// `WIN   child hwnd=0xaf174c class=InputSiteWindowClass`，探针里 `TARGETS … xaml=0xAF174C`），
/// WinUI 自述/别家版本里是长名 `Microsoft.UI.Input.InputSite.WindowClass` ⇒ 判定用**子串**
/// `INPUT_SITE_NEEDLE`，两个名字都认。
const INPUT_SITE_CLASS: &str = "InputSiteWindowClass";

/// [`INPUT_SITE_CLASS`] 与长名共有的那截类名。
const INPUT_SITE_NEEDLE: &str = "InputSite";

/// 内容桥窗口类（实测 `class=Microsoft.UI.Content.DesktopChildSiteBridge`）。
const BRIDGE_CLASS: &str = "Microsoft.UI.Content.DesktopChildSiteBridge";

/// 非客户区指针源窗口类（实测 `class=InputNonClientPointerSource`）。
const NONCLIENT_CLASS: &str = "InputNonClientPointerSource";

/// 诊断标签：这一发**没投出去**（拒绝、失败之前的一切路径），或还没投过。类名读不到时也归这一档
/// —— 「不知道」和「没有」在诊断上是同一件事，不许混进 `other` 冒充「读到了一个不认识的类」。
pub const CLASS_NONE: &str = "none";

/// 诊断标签：类名**读到了**，但不是 [`KNOWN_CLASSES`] 里那四张脸。
pub const CLASS_OTHER: &str = "other";

/// 分叉的窗口树里可能出现的四类窗口（`tmp/keys-probe-report.txt` 实测全名单）。
/// 只用来把诊断标签钉死成有限集合 ⇒ 单测能穷举；不是投递白名单（投递目标是「点到谁就是谁」）。
pub const KNOWN_CLASSES: [&str; 4] = [WINDOW_CLASS, INPUT_SITE_CLASS, BRIDGE_CLASS, NONCLIENT_CLASS];

/// #64 实测：内容区上 `WindowFromPoint` 给的是 [`BRIDGE_CLASS`] 那一发（`tmp/qa6-wheel-wheeldown-end.txt`
/// 的 `WHEEL-PROBE hit=199254 class=…DesktopChildSiteBridge`）—— 但「命中谁」不等于「谁收得到滚轮」：
/// 只有 input-site 真滚（`tmp/qs1-report.md §3`），所以它是 [`pick_post_target`] 的目标态。
///
/// 送达方式标签（`Posted::route()` 的取值面，全部 `&'static str` ⇒ 能进 `format!` 与 `assert_eq!`）。
/// 注入那一发实测有效（`tmp/qa6-report.txt:60-62`）；直投那一发只有投对 input-site 时才有效（同一节）。
pub const ROUTE_NONE: &str = "none";
/// `SendInput` + `SetCursorPoint`：系统按光标位置路由 ⇒ XAML 的输入栈真的吃了这一发。
pub const ROUTE_INJECT: &str = "sendinput";
/// `PostMessageW` 直投 WndProc：返回非 0 ≠ 滚了；实测会滚的目标只有 [`INPUT_SITE_CLASS`]
/// 那一颗（`tmp/qs1-report.md §3`），只在注入被系统挡掉时补一发。
pub const ROUTE_POST: &str = "postmessage";

/// `GetAncestor` 的 `GA_ROOT`。
const GA_ROOT: u32 = 2;

/// `POINT`：内存布局必须逐字节对上（两个 `LONG`）。
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct WinPoint {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct WinRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// `MOUSEINPUT`（`winuser.h`）。布局必须是 `4+4+4+4+4(+4 补齐)+8 = 32`：
/// `time` 之后那个 4 字节是 `ULONG_PTR dwExtraInfo` 的 8 字节对齐自动撑出来的，
/// 少一格整条 `SendInput` 就会把后面事件的字段读歪 —— 单测
/// [`tests::sendinput_struct_layout_matches_the_win32_abi`] 钉住尺寸，改字段顺序必炸。
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct WinMouseInput {
    dx: i32,
    dy: i32,
    mouse_data: u32,
    mouse_flags: u32,
    time: u32,
    extra_info: usize,
}

/// `INPUT`：`DWORD type` + 一个 32 字节的 union。我们永远只填 `mi`（`INPUT_MOUSE`），
/// 所以直接把 union 那格声明成 `WinMouseInput` 就是逐字节等价的形状（`type` 后面的 4 字节
/// 是 union 的 8 字节对齐补齐，`_pad` 显式写出来免得读者以为漏了）。
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct WinInput {
    input_type: u32,
    _pad: u32,
    mi: WinMouseInput,
}

/// `MSLLHOOKSTRUCT`（低级鼠标钩子的 lParam）。`mouse_data` 在 `WM_MOUSEWHEEL` 那一发里
/// 装的是**有符号**滚轮增量（`SHORT` 放在 `DWORD` 低半字，高半字是历史遗留的 wheel 转数）。
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct WinMouseHookStruct {
    point: WinPoint,
    mouse_data: u32,
    mouse_flags: u32,
    time: u32,
    extra_info: usize,
}

// ---------------------------------------------------------------- 纯函数：打包与夹取

/// 格数 → 夹到 [`MAX_NOTCHES`] 之后的格数（符号保留，`0` 原样是 `0`）。
///
/// 为什么要夹：唯一的滚动出口是「相对补 N 格」，一格算错就是整页算错；不夹的话一次估算失误
/// 能把列表甩到另一头，而分叉读不回滚动位置、没有自愈通路。
pub const fn clamp_notches(notches: i32) -> i32 {
    if notches > MAX_NOTCHES {
        MAX_NOTCHES
    } else if notches < -MAX_NOTCHES {
        -MAX_NOTCHES
    } else {
        notches
    }
}

/// 贴底那一发的夹取：上限换成 [`MAX_BURST_NOTCHES`]，下限仍对称。
/// 判据本身与 [`clamp_notches`] 一字不差，只是预算不同（为什么允许更大的理由见那个常量）。
pub const fn clamp_burst(notches: i32) -> i32 {
    if notches > MAX_BURST_NOTCHES {
        MAX_BURST_NOTCHES
    } else if notches < -MAX_BURST_NOTCHES {
        -MAX_BURST_NOTCHES
    } else {
        notches
    }
}

/// **已经夹好**的格数 → 滚轮增量。符号见 [`wheel_delta`]。
const fn delta_of(clamped: i32) -> i32 {
    // 取负：本模块对外的 `notches` 读的是「内容怎么走」（正 = 向尾端 = 视觉向下），
    // 而 Win32 的增量读的是「轮子怎么转」（向自己这一侧转 = 向下 = 负）。
    -(clamped * WHEEL_DELTA)
}

/// 格数 → 滚轮增量（`WM_MOUSEWHEEL` 的 `wParam` 高位字，与 `SendInput` 的
/// `MOUSEINPUT::mouseData` 是同一个数）：**向列表尾端（滚轮向下）为负**。
///
/// 符号口径抄探针（`tmp/probe-keys.rs.bak`：`delta=-1200` 是把 10 格**向尾端**滚出去的那一发，
/// UIA 读出 `VerticalScrollPercent 0 → 100`）。所以本模块对外的 `notches` 与这里的**反号**：
/// `notches` 正 = 往尾端滚，落到线上就是负增量。
/// ⚠ #64 顺带纠正的一条符号错：旧实现是 `clamp_notches(n) * WHEEL_DELTA`（同号），
/// 于是「向尾端补 3 格」发出的其实是**向头端** 3 格。它一直没被发现，因为那条通路
/// （`PostMessageW`）当时投的是收不到消息的窗口 ⇒ 一个像素都不动（目标态见 [`pick_post_target`]）。
/// 先夹再乘：夹完的取值域是 ±[`MAX_NOTCHES`]，`× 120` 不可能溢出，用不着 `saturating_*`。
pub const fn wheel_delta(notches: i32) -> i32 {
    delta_of(clamp_notches(notches))
}

/// [`wheel_delta`] 的贴底版：夹到 [`MAX_BURST_NOTCHES`] 而不是 12。
pub const fn burst_delta(notches: i32) -> i32 {
    delta_of(clamp_burst(notches))
}

/// 格数 → `wParam`：增量放**高位字**，低位字恒 0（`MK_*` 修饰键位在那一半，我们不按着键滚）。
///
/// `-1200` 的 `as u16` 是 `0xFB20`，与 Win32 `GET_WHEEL_DELTA_WPARAM` 的 `(short)HIWORD(wParam)`
/// 读回来正好是 `-1200`；这也是探针实测用的那一句。
pub const fn wheel_w_param(notches: i32) -> usize {
    pack_delta(wheel_delta(notches))
}

/// [`burst_delta`] 的 `wParam` 版（贴底那一发直投时也要按同一口径打包）。
pub const fn burst_w_param(notches: i32) -> usize {
    pack_delta(burst_delta(notches))
}

/// 增量 → `wParam`：只留低 16 位（补码原样进高字），低字（`MK_*`）恒 0。
const fn pack_delta(delta: i32) -> usize {
    ((delta as u16) as usize) << 16
}

/// 屏幕点 → `lParam`（`MAKEPOINTS`：低字 x、高字 y，都是 `SHORT`）。
///
/// 负数坐标必须**原样保留**成补码：本机主屏 200% 缩放、副屏在主屏左边，
/// 光标落在副屏上就是 `x < 0`（探针里量到的是正数，那条路径没盖住这一半取值域）。
/// `as u16` 取的是低 16 位，`-1920i32 as u16 == 0xF860`，读回 `i16` 仍是 `-1920` ⇒
/// 只要坐标在 `i16` 范围内就不会漂；超出 `i16` 范围（±32767）的点是病态输入，这里
/// 显式判掉并退化成 `i16::MIN/MAX`，不静默环绕。
pub const fn wheel_l_param(at: Point) -> usize {
    let x = clamp_short(at.x) as u16 as usize;
    let y = clamp_short(at.y) as u16 as usize;
    (y << 16) | x
}

/// 把任意整数钉进 `i16` 取值域（超界取端点，不环绕）。
const fn clamp_short(value: i32) -> i16 {
    if value > i16::MAX as i32 {
        i16::MAX
    } else if value < i16::MIN as i32 {
        i16::MIN
    } else {
        value as i16
    }
}

/// DIP → 物理像素（四舍五入）。`dpi == 0` 视为「问不到」，按 100% 处理。
///
/// 为什么需要这一层：XAML 给出来的坐标（`PointerEventInfo::window_x/window_y`）是 **DIP**，
/// 而 `WM_MOUSEWHEEL` 的 lParam 要的是**屏幕物理像素**。本机 200% 缩放，两者差一倍 ⇒
/// 不换算就是每次都点到别处去。分叉进程是 PerMonitorV2 感知（`windows-reactor`
/// `native/winui/mod.rs:4298` 那句 `SetProcessDpiAwarenessContext`），所以 `GetWindowRect` /
/// `ClientToScreen` / `GetCursorPos` 读到的都是真实物理像素，可以直接叠。
pub fn dip_to_px(dip: f64, dpi: u32) -> i32 {
    if !dip.is_finite() {
        return 0;
    }
    let scale = if dpi == 0 { DPI_AT_100 } else { dpi as f64 } / DPI_AT_100;
    (dip * scale).round() as i32
}

/// 物理像素 → DIP（[`dip_to_px`] 的逆运算）。`dpi == 0` 视为「问不到」，按 100% 处理。
///
/// 为什么需要逆运算：[`client_size`] 给的是**物理像素**的客户区宽高，而 [`ChatBox`] 那套
/// 正文带几何全程用 DIP（与 XAML/`PointerEventInfo` 同一口径）。不换算就是 200% 缩放下
/// 把带子算宽一倍，落点直接甩到窗口外。
#[must_use]
pub fn px_to_dip(px: i32, dpi: u32) -> f64 {
    let scale = if dpi == 0 { DPI_AT_100 } else { dpi as f64 } / DPI_AT_100;
    f64::from(px) / scale
}

/// 窗口内坐标（DIP，XAML 根视觉口径）→ 屏幕物理像素点。
///
/// `origin` 来自 [`client_origin`]（`ClientToScreen(hwnd, {0,0})`，即客户区左上角的屏幕位置；
/// 用 `GetWindowRect` 的左上角会差掉 Win11 那圈不可见边框）。纯函数、可离线测。
pub fn anchor_to_point(window: Point, origin: Point, dpi: u32) -> Point {
    Point {
        x: origin.x.saturating_add(dip_to_px(f64::from(window.x), dpi)),
        y: origin.y.saturating_add(dip_to_px(f64::from(window.y), dpi)),
    }
}

// ---------------------------------------------------------------- 纯函数：可见带与量化

/// 一条滚动带的固定几何（单位：DIP）。
///
/// 分叉**读不到**任何控件的实际布局（框架没有 `ActualHeight` / `TransformToVisual` /
/// `Bounds` 出口），所以带子只能由被滚那一层自己的固定常量估出来 ⇒ [`Band::dip_per_notch`]
/// 与本模块一切「位置」断言都是**估计值**，不是事实。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    /// 一行多高（DIP）。
    pub row_dip: f64,
    /// 视口能装下多少 DIP（DIP，不含视口外的内容）。
    pub page_dip: f64,
    /// 一格滚轮走多少 DIP。**未在本机实测**，见 `main.rs` 的 `PALETTE_DIP_PER_NOTCH`。
    pub dip_per_notch: f64,
}

/// 视口装得下几**整**行（至少 1，几何是垃圾输入时也返回 1）。
pub fn rows_visible(band: Band) -> usize {
    if !band.row_dip.is_finite() || band.row_dip <= 0.0 || !band.page_dip.is_finite() {
        return 1;
    }
    let rows = (band.page_dip / band.row_dip).floor();
    if rows < 1.0 {
        1
    } else {
        // 上限夹到 MAX_NOTCHES 那一档：带子比这还宽说明几何不可信。
        rows as usize
    }
}

/// 「要走的 DIP」→「要补的格数」：向上取整（宁多不少，一次没滚够就是下一发按键接着滚）、
/// 夹到 [`MAX_NOTCHES`]、非零输入至少给 1 格。
pub fn notches_for_dip(dip: f64, band: Band) -> i32 {
    if !dip.is_finite() || dip == 0.0 {
        return 0;
    }
    if !band.dip_per_notch.is_finite() || band.dip_per_notch <= 0.0 {
        // 每格多少 DIP 不知道 ⇒ 一格都不该发：发了就是瞎滚。
        return 0;
    }
    let magnitude = (dip.abs() / band.dip_per_notch).ceil();
    if !magnitude.is_finite() || magnitude < 1.0 {
        return clamp_notches(if dip < 0.0 { -1 } else { 1 });
    }
    clamp_notches(if dip < 0.0 { -(magnitude as i32) } else { magnitude as i32 })
}

// ---------------------------------------------------------------- 纯函数：正文带的几何（#89）

/// 落点与带子右缘（= 轮次轨左缘那一侧）之间留的安全内缩（DIP）。
///
/// 为什么必须有：轨叠在正文**同一格**里（`main.rs` 的 `turn_rail` 用 `grid_row_span(2)` 浮在
/// 正文右缘，不给正文减宽），而滚轮归谁滚**只看落点**。落点压进轨的命中区 ⇒ 滚的是轨自己那条
/// `ScrollViewer`，正文一个像素不动 —— 这个坑 `main.rs` 的 `RAIL_WHEEL_LEFT_OFFSET_DIP` 已经踩过
/// 一次并写死了教训。8 DIP = 一条气泡的圆角量级，比「刚好贴着轨」多一档余量，比半条气泡保守。
pub const WHEEL_EDGE_INSET_DIP: f64 = 8.0;

/// 正文那条滚动带在**客户区 DIP** 里的矩形（左 / 上 / 右 / 下，与 [`client_size`] 同一原点）。
///
/// 为什么立这个类型：#89 的贴底**不许依赖指针位置**。主干 `ScrollTranscriptToBottom`
/// （MW `MainWindow.xaml.cs:5727-5751`）从头到尾没问过光标在哪 —— 用户发 prompt 时指针必然还
/// 在 composer 的输入框里，那时主干照样把新气泡拽进视野。而分叉唯一的滚动出口是「按屏幕点注入
/// 一发滚轮」（见模块头注 #64 那一节）⇒ 必须自己给出一个**恒在正文带内**的点，不能像
/// [`post_wheel_at_window_anchor`] 那样等一次真实指针事件来喂锚点。
///
/// 这个矩形由 `main.rs` 那套**布局常量**（侧栏宽 / 顶条高 / 正文内衬 / composer 占位）加上
/// 客户区实测宽高拼出来 ⇒ 布局常量改了它跟着改。⚠ 它仍然是**算出来的**：框架读不到实际布局
/// （`ActualHeight` / `Bounds` / `TransformToVisual` 零出口），composer 长到 `MAX_HEIGHT`(160)
/// 那一档、或用户把窗口拉到极窄时，矩形会与实际带子脱节 ⇒ 判据一律走 [`ChatBox::is_usable`]
/// 与 [`ChatBox::wheel_anchor`] 的 `Option`，宁可什么都不发。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChatBox {
    /// 带子左界（DIP）：侧栏宽 + 正文那条 `ScrollViewer` 的左内衬。
    pub left: f64,
    /// 带上界（DIP）：顶条高 + 正文内衬上。
    pub top: f64,
    /// 带子右界（DIP）：客户区宽 − 正文内衬右。**不含**轨的扣减（轨是叠层，不减宽）。
    pub right: f64,
    /// 带子下界（DIP）：客户区高 − composer 那一档的占位。
    pub bottom: f64,
}

impl ChatBox {
    /// 这条带子还成个矩形吗（四个量都得是有限数，且宽、高为正）。
    ///
    /// 主干同一位置的守卫是 `sv.ScrollableHeight > 0`（拿不到正的滚动面就什么都不做）⇒
    /// 不成立时贴底**一发都不发**，与 [`stick_action`] 返回 [`HOLD_NO_GEOMETRY`] 同一条回落。
    #[must_use]
    pub const fn is_usable(&self) -> bool {
        self.left.is_finite()
            && self.top.is_finite()
            && self.right.is_finite()
            && self.bottom.is_finite()
            && self.right > self.left
            && self.bottom > self.top
    }

    /// 带内一个**恒定点**（DIP，窗口内坐标，直接喂 [`post_wheel_burst_at_window_point`]；
    /// **不**写进 [`note_anchor`]，那颗是浮层滚轮的可信落点）。`None` = 落点没地方放 ⇒ 什么都不发。
    ///
    /// * `rail_left` = 轮次轨左缘的 DIP 横坐标（拿不到就传 `f64::NAN`：会自动退回到
    ///   「带子右界内缩 [`WHEEL_EDGE_INSET_DIP`]」这一硬边界）。可用横段的右界取
    ///   `min(rail_left, right) - 内缩`，所以**轨在不在屏幕上都不影响正确性**。
    /// * 横向取可用段的中点（不是右界）：左右都留出余量，DIP↔px 取整误差 1~2 px 伤不到。
    /// * 纵向取带高的四成处而非正中：贴底时最后一条气泡压在下沿，落点太靠下就有概率落进气泡
    ///   之间的空白/动作行上；上半段恒在气泡体内。
    #[must_use]
    pub fn wheel_anchor(&self, rail_left: f64) -> Option<Point> {
        if !self.is_usable() {
            return None;
        }
        let edge = if rail_left.is_finite() {
            rail_left.min(self.right)
        } else {
            self.right
        } - WHEEL_EDGE_INSET_DIP;
        let width = edge - self.left;
        if !(width > 0.0) {
            return None; // 窄到正文带里放不下落点：宁可不发
        }
        Some(Point {
            x: (self.left + width * 0.5).round() as i32,
            y: (self.top + (self.bottom - self.top) * 0.4).round() as i32,
        })
    }

    /// 某个客户区 DIP 点是否落在这条带子里（含边界）。病态输入一律判不在。
    #[must_use]
    pub const fn contains_dip(&self, x: f64, y: f64) -> bool {
        self.is_usable() && x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }
}

/// [`keep_in_view`] 的结果：补这几格，以及补完之后**估计**的窗口顶端行号。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Follow {
    /// 要补的格数：正 = 往尾端滚，负 = 往头端滚。恒非零。
    pub notches: i32,
    /// 补完之后估计的「视口第一行」行号（可以是小数：一格走的 DIP 未必整行数）。
    pub top_rows: f64,
}

/// 主干 `CommandList.ScrollIntoView(_commandMatches[_commandIndex])`（MW `MoveCommandSelection`）
/// 在分叉能表达的那半截：**选中行掉出可见带时，补最少的格数把它带回来**。
///
/// * `top_rows` —— **估计**的当前视口首行行号（死算：上一次补了几格就往前推几格的 DIP）。
/// * `index` / `count` —— 选中行与候选条数；`index < 0`（主干收起态的 `-1`）或越界 ⇒ `None`。
///
/// 返回 `None` = 已经在带内（或没得可滚）⇒ 一个像素都不动。
pub fn keep_in_view(band: Band, top_rows: f64, index: i32, count: usize) -> Option<Follow> {
    if index < 0 || index as usize >= count || !top_rows.is_finite() {
        return None;
    }
    let rows = rows_visible(band) as f64;
    let top = top_rows.max(0.0);
    let target = f64::from(index);
    let shortfall = if target < top {
        // 往头端滚：把第 `index` 行带回顶端，多带走一行（省得压边）。
        -(top - target)
    } else if target >= top + rows {
        target - (top + rows - 1.0)
    } else {
        return None;
    };
    if shortfall == 0.0 {
        return None;
    }
    let dip = shortfall * band.row_dip;
    let notches = notches_for_dip(dip, band);
    if notches == 0 {
        return None;
    }
    let moved = if band.row_dip > 0.0 && band.dip_per_notch.is_finite() {
        f64::from(notches) * band.dip_per_notch / band.row_dip
    } else {
        f64::from(notches)
    };
    Some(Follow { notches, top_rows: (top + moved).max(0.0) })
}

// ---------------------------------------------------------------- 纯函数：贴底状态机（#89）

/// 主干的贴底阈值（MW `MainWindow.xaml.cs:5722` `private const double StickToBottomPx = 96`）。
///
/// 名字里写的是 Px，但它比的是 XAML 的 `ScrollableHeight - VerticalOffset` —— 那两个量在 XAML 里
/// 是 **DIP**（effect pixel，与缩放无关）⇒ 分叉这边同一条判据只能按 DIP 记。抄主干抄的是**数值**：
/// 96 不改，改成别的就不是「一模一样」。
pub const STICK_TO_BOTTOM_DIP: f64 = 96.0;

/// 贴底那一发的**过滚**余量（格）。
///
/// 为什么故意多滚：分叉没有 `ChangeView`（主干那句 `sv.ChangeView(null, sv.ScrollableHeight, …)`
/// 是**绝对**定位），只有相对滚轮 ⇒ 少一格就是「最后一条气泡还差半行看不见」这个 bug 本身。
/// `ScrollViewer` 自己把偏移夹在 `[0, scrollableHeight]`，多滚的那几格落在边界上就消失了，
/// 代价只是末尾那一屏早一点到位。主干注释里那句「`ScrollIntoView` 只把末项**顶部**对齐视口，
/// 逐字增高的长气泡会造成『跳上去再弹回来』的抖动」在分叉里同样成立 —— 过滚是朝尾端单向夹取，
/// 不会来回弹。
pub const OVER_ROLL_NOTCHES: i32 = 2;

/// [`stick_action`] 的「不跟随」原因（进 `DIAG:` 日志，取值面有限 ⇒ 单测能穷举）。
pub const HOLD_USER_UP: &str = "user-up";

/// 「离底距离知道，但一格走多少 DIP 不知道」⇒ 发了就是瞎滚，宁可不发。
/// 主干同一形态是 `sv.ScrollableHeight > 0` 那道守卫（拿不到正的滚动面就什么都不做）。
pub const HOLD_NO_GEOMETRY: &str = "no-geometry";

/// 「离正文底部还差多少 DIP」的账本 —— 分叉版的「是不是贴底」。
///
/// 主干不用记账：`IsTranscriptAtBottom()` 每次现读 `ScrollableHeight - VerticalOffset`
/// （MW `MainWindow.xaml.cs:5753-5760`）。分叉读不到任何滚动面 ⇒ 只能把同一个量**推算**出来：
/// `gap_dip == 0` 等价于主干的 `VerticalOffset == ScrollableHeight`。
///
/// 三条判据各自对应主干的一句话，也各自有一颗 `#[test]`：
/// 1. **什么时候跟随** —— [`stick_action`] 里 `gap_dip <= 96`，或调用方给了 `force`
///    （主干 `ScrollTranscriptToBottom(force: true)` 的那五个调用点：用户回显、图片、
///    pending 占位、历史载入、轮次跳转）。
/// 2. **什么时候停止跟随** —— 用户**往头端**滚 ⇒ [`Stick::user_roll`] 把 `gap_dip` 抬过 96。
///    主干原话：「上翻读旧消息不该被逐字增量拽回底部」（`MainWindow.xaml.cs:5726`）。
/// 3. **什么时候恢复** —— 用户**往尾端**滚回 96 以内 ⇒ 同一个几何判据再次成立。
///    ⚠ 这里**没有闩锁**（主干也没有）：不存在 `if (userScrolledAway) return;` 那种一次性标记，
///    所以「恢复」不需要任何额外通路，这是「判据每次现算」白送的性质。
///
/// 记账口径的两条要害：
/// * **增重只在没贴底时把 gap 变大**（见 [`Stick::content_grew`]）—— 主干的判据跑在**改完内容、
///   还没重排布局**的那一刻，所以「本轮变高」 defeat 不了本轮的跟随；贴底时把增量吸掉就是抄这一步。
/// * `gap_dip` 永不为负、非有限值一律当 `0`（= 「按贴底处理，别错过首屏」，与主干
///   `IsTranscriptAtBottom` 在拿不到滚动面时 `return true` 同一条回落）。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stick {
    gap_dip: f64,
}

impl Stick {
    /// 贴在绝对底部（`gap_dip == 0`）。会话绑定 / 历史载入后用这一颗起手：那时**没有**别的证据，
    /// 而主干在同一个位置本来就是 `force: true` + 拿不到滚动面时按贴底处理。
    pub const fn bottom() -> Stick {
        Stick { gap_dip: 0.0 }
    }

    /// 从「离底多少 DIP」建账本。负数、`NEG_INFINITY` 与 NaN 都归 `0`（宁可多跟一拍，也别因账本
    /// 坏了而永久不跟）；`+INFINITY` **原样保留** —— 它的意思是「远得说不清」，那正是「别跟着滚」。
    pub fn new(gap_dip: f64) -> Stick {
        Stick { gap_dip: if gap_dip.is_nan() || gap_dip <= 0.0 { 0.0 } else { gap_dip } }
    }

    /// 当前离底距离（诊断行用）。
    pub const fn gap_dip(&self) -> f64 {
        self.gap_dip
    }

    /// 判据本体：主干 `IsTranscriptAtBottom()` 的那一句 `ScrollableHeight - VerticalOffset <= 96`。
    pub const fn is_at_bottom(&self) -> bool {
        self.gap_dip <= STICK_TO_BOTTOM_DIP
    }

    /// 用户自己滚了 `notches` 格（本模块符号：**正 = 向尾端**）⇒ 离底距离减这么多 DIP。
    ///
    /// 夹在 `[0, +∞)`：`ScrollViewer` 就是夹在边界的，滚过了头不会让「离底距离」变成负数。
    /// `dip_per_notch` 不可信时**原样返回**（没有换算率就别瞎改账）。
    #[must_use]
    pub fn user_roll(&self, notches: i32, band: Band) -> Stick {
        if notches == 0 || !band.dip_per_notch.is_finite() || band.dip_per_notch <= 0.0 {
            return *self;
        }
        Stick::new(self.gap_dip - f64::from(notches) * band.dip_per_notch)
    }

    /// 内容长高了 `grew_dip` DIP（新气泡 / 逐字增高）。
    ///
    /// 贴底时**吸收**（下一条 [`stick_action`] 本来就要把它抹平成 0）；不贴底时视口的偏移不动、
    /// 底却往下跑了 ⇒ 离底距离变大。这两半合起来就是主干「判据跑在布局之前」的可观察结果。
    #[must_use]
    pub fn content_grew(&self, grew_dip: f64) -> Stick {
        if !grew_dip.is_finite() || grew_dip <= 0.0 {
            return *self;
        }
        if self.is_at_bottom() {
            return *self;
        }
        Stick::new(self.gap_dip + grew_dip)
    }

    /// 刚补过一发贴底滚轮 ⇒ 认为已经压在绝对底部（过滚 + 边界夹取，见 [`OVER_ROLL_NOTCHES`]）。
    #[must_use]
    pub fn snapped(&self) -> Stick {
        let _ = self;
        Stick::bottom()
    }
}

/// [`burst_for_gap`] 的结果：为什么要单独抽出来 —— 「补几格」这件事与「该不该补」是两条判据，
/// 主干也是两句（`IsTranscriptAtBottom()` 与 `ChangeView(ScrollableHeight)`）。
#[must_use]
pub fn burst_for_gap(gap_dip: f64, band: Band) -> Option<i32> {
    // 换算率必须是**正的有限数**：NaN / ±∞ / 0 / 负数都答不出「一格走多少 DIP」⇒ 不发。
    if !(band.dip_per_notch.is_finite() && band.dip_per_notch > 0.0) {
        return None; // 一格走多少 DIP 不知道 ⇒ 发了就是瞎滚
    }
    // 病态距离各有归宿：NaN/负数当「就在底部」，`+∞` 当「远到说不清」⇒ 直接给预算上限。
    let gap = if gap_dip.is_nan() || gap_dip <= 0.0 {
        0.0
    } else if gap_dip.is_finite() {
        gap_dip
    } else {
        f64::MAX
    };
    let exact = (gap / band.dip_per_notch).ceil();
    let notches = exact.max(1.0) + f64::from(OVER_ROLL_NOTCHES);
    if !notches.is_finite() {
        return Some(MAX_BURST_NOTCHES);
    }
    // 先夹取值域再 `as i32`：超大浮点走的是「饱和到预算上限」这条路，不是环绕。
    Some(clamp_burst(notches.min(f64::from(i32::MAX)) as i32))
}

/// #89 的**决策本体**（纯函数）：这一次内容变了/新气泡来了，要不要滚到底。
///
/// * `force == true` 抄主干 `ScrollTranscriptToBottom(force: true)` 的那几个调用点：无视账本，
///   直接补（主干是 `if (!force && !IsTranscriptAtBottom()) return;`，一字不差）。
/// * 不 force 时只有 `is_at_bottom()` 才滚 —— 也就是「用户没上翻」这件事**不需要状态**，
///   它就是 `gap_dip <= 96`（主干同理）。
/// * 滚的那一发是 [`burst_for_gap`]：按当前离底距离算格数，再加 [`OVER_ROLL_NOTCHES`] 格过滚。
///
/// 返回值把 `gap_dip` 一并带出来，是为了日志能自证「判据当时看到的是什么」。
#[must_use]
pub fn stick_action(stick: Stick, force: bool, band: Band) -> StickAction {
    let gap = stick.gap_dip();
    if !force && !stick.is_at_bottom() {
        return StickAction::Hold { reason: HOLD_USER_UP, gap_dip: gap };
    }
    match burst_for_gap(gap, band) {
        Some(notches) => StickAction::Roll { notches, gap_dip: gap },
        None => StickAction::Hold { reason: HOLD_NO_GEOMETRY, gap_dip: gap },
    }
}

/// [`stick_action`] 的输出：要么补这么多格，要么这一拍什么都不动。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StickAction {
    /// 补 `notches` 格向尾端（恒 > 0，已含 [`OVER_ROLL_NOTCHES`] 过滚，已夹到 [`MAX_BURST_NOTCHES`]）。
    Roll { notches: i32, gap_dip: f64 },
    /// 不动。`reason` 是 [`HOLD_USER_UP`]（用户上翻了）或 [`HOLD_NO_GEOMETRY`]（换算率不可信）。
    Hold { reason: &'static str, gap_dip: f64 },
}

// ---------------------------------------------------------------- 线程内状态
//
// 与 `keys.rs` 同一条理由：这些只能在 UI 线程上用（`Rc<RefCell<…>>` 非 `Send`），
// 放 `thread_local!` 既免锁又天然正确。

thread_local! {
    /// 解析过的顶层窗口句柄（`0` = 还没解析或解析失败）。窗口活着就不会换，所以只查一次。
    static HWND: Cell<usize> = const { Cell::new(0) };
    /// 最近一次「指针真的在可滚内容上」的窗口内坐标（DIP）+ 它属于哪条滚动带。见 [`note_anchor`]。
    static ANCHOR: RefCell<Option<(AnchorSource, Point)>> = const { RefCell::new(None) };
    /// 低级鼠标钩子攒下来的「用户自己滚的格数 + 屏幕点」（`None` = 还没有）。
    /// 由 [`wheel_proc`] 写、由 [`take_user_wheel`] 一次性取走 —— 分叉没有 `ViewChanged`，
    /// 这是 #89「用户上翻就该停止跟随」那条判据唯一的输入来源。
    static USER_WHEEL: Cell<Option<(i32, Point)>> = const { Cell::new(None) };
    /// 钩子句柄（空指针 = 没装）。与 `keys.rs` 同一条理由放 `thread_local!`：装/回调/卸都在
    /// UI 线程那条消息泵上，跨线程卸不保证安全。
    static WHEEL_HOOK: Cell<*mut c_void> = const { Cell::new(std::ptr::null_mut()) };
    /// 最近一次 `post_wheel` **实际投给的窗口** `(句柄, 类名标签, 送达方式)`
    /// （`0` + [`CLASS_NONE`] + [`ROUTE_NONE`] = 没投）。
    /// 每次进 [`post_wheel`] / [`post_wheel_at_window_anchor`] 先擦，所以读到的永远对应本次调用。
    /// 这里**只有诊断信息**，不参与任何决策 ⇒ 子窗口句柄绝不当缓存用（见 [`post_target`]）。
    static LAST_TARGET: Cell<(usize, &'static str, &'static str)> =
        const { Cell::new((0, CLASS_NONE, ROUTE_NONE)) };
}

/// 锚点属于哪条滚动带 —— #89 需要知道「用户这一发滚轮滚的是不是正文」。
///
/// 分叉拿不到「XAML 里指针在哪个控件上」（框架不给 hit-test 出口），唯一的证据就是
/// 「最后一次真实指针事件是从哪一层 `Border` 发出来的」：那些 `Border` 由 `main.rs` 打标，
/// 所以这个枚举的取值面与 `main.rs` 里挂 `note_anchor` 的位置一一对应。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchorSource {
    /// 聊天正文的一条气泡（`ScrollViewer` = `chat_page` 里那条 `stream`）⇒ #89 的贴底账本跟它。
    Chat,
    /// 命令浮层的一行（`CommandPaletteList`）⇒ 浮层自己的相对跟随跟它。
    Palette,
    /// 轮次轨的一格（`TurnRailMarks`）⇒ 点跳（`jump_to_rail_turn`）。
    Rail,
}

/// 记下「指针此刻在窗口内的位置」（DIP，`PointerEventInfo::window_x/window_y`）以及它属于哪条带。
///
/// 这是分叉唯一**可信**的滚轮落点来源：它来自一次真实指针事件，所以那个点一定在发出事件的那
/// 个控件里。调用方（`main.rs`）只在可滚列表的行上记它。
pub fn note_anchor(source: AnchorSource, window: Point) {
    ANCHOR.with(|slot| *slot.borrow_mut() = Some((source, window)));
}

/// 最近记下的那个窗口内坐标（不知道是哪条带的调用方用这一颗）。
pub fn anchor() -> Option<Point> {
    ANCHOR.with(|slot| slot.borrow().map(|(_, point)| point))
}

/// 最近那次锚点属于哪条带（`None` = 压根没记过）。
pub fn anchor_source() -> Option<AnchorSource> {
    ANCHOR.with(|slot| slot.borrow().map(|(source, _)| source))
}

/// 抹掉锚点。浮层收起时**必须**调：那个点是从浮层某一行记下来的，浮层没了它就只是一个
/// 「碰巧落在聊天流上」的坐标。
pub fn forget_anchor() {
    ANCHOR.with(|slot| *slot.borrow_mut() = None);
}

/// 本进程的 WinUI 顶层窗口句柄（找不到 = `None`：这条线程上压根没有窗口，比如单测线程）。
///
/// 走的是探针实测过的那条路（`EnumThreadWindows` + 类名匹配），分叉此前**没有**任何 HWND 通路
/// （`keys.rs` 只装线程钩子，`main.rs` 里 `hwnd` 零命中），所以这里是第一次立起来的一条。
/// 回调体不许 panic（它是直接回调，穿过 `extern "system"` 边界）。
///
/// 自 #64 起它是滚轮的**回落**目标，不再是默认投递目标（默认走 [`post_target`] 解析到的子窗口）；
/// 另两件事仍然只认它：`client_origin()` 的几何/DPI、`point_in_window()` 的那道窗口归属校验。
/// 顶层缓存是安全的（一个窗口生命周期里不换），子窗口**不许**照这条抄。
pub fn hwnd() -> Option<usize> {
    let cached = HWND.with(|slot| slot.get());
    if cached != 0 {
        return Some(cached);
    }
    let found = Cell::new(0usize);
    let thread = unsafe { GetCurrentThreadId() };
    unsafe {
        EnumThreadWindows(thread, enum_proc as *const c_void, &found as *const Cell<usize> as *mut c_void);
    }
    let hit = found.get();
    if hit != 0 {
        HWND.with(|slot| slot.set(hit));
        Some(hit)
    } else {
        None
    }
}

/// `EnumThreadWindows` 回调：只认 `WinUIDesktopWin32WindowClass` 那一发，其余跳过。
/// 整个体套 `catch_unwind` —— panic 穿过这条边界就是进程没了。
/// 体内**不分配**（类名逐码点比对，不走 `String::from_utf16_lossy`），出事也照样返回 1
/// 让枚举继续，绝不因为一次内部异常把整条窗口枚举掐断。
extern "system" fn enum_proc(hwnd: *mut c_void, l_param: *mut c_void) -> i32 {
    let _ = panic::catch_unwind(AssertUnwindSafe(|| {
        if hwnd.is_null() || l_param.is_null() || !class_matches(hwnd) {
            return;
        }
        let slot = unsafe { &*(l_param as *const Cell<usize>) };
        if slot.get() == 0 {
            slot.set(hwnd as usize);
        }
    }));
    1
}

/// 窗口类名是不是 [`WINDOW_CLASS`]（UTF-16 逐码点，零分配）。
fn class_matches(hwnd: *mut c_void) -> bool {
    let mut buf = [0u16; 64];
    utf16_eq(read_class_name(hwnd, &mut buf), WINDOW_CLASS)
}

/// 顶层窗口下那颗类名含 [`INPUT_SITE_NEEDLE`] 的子窗口（`0` = 没找到）。
///
/// **每次现查、绝不缓存句柄**：`tmp/qs1-run5.txt:111` 坐实它是唯一真滚的目标，而它的句柄每轮都换
/// （run1 `1512068` → run5 `2691232`）。子树里找得到 ≠ 看得见 —— 它 `GetWindowRect` 是 0x0 且
/// `vis=False`，所以任何「按可见性/几何筛窗口」的写法都会把它排除掉，别加那种过滤。
fn input_site_child(top: usize) -> usize {
    if top == 0 {
        return 0;
    }
    let found = Cell::new(0usize);
    unsafe {
        EnumChildWindows(top as *mut c_void, input_site_proc as *const c_void, &found as *const Cell<usize> as *mut c_void);
    }
    found.get()
}

/// `EnumChildWindows` 回调：只认类名含 `InputSite` 的那一发（短名/长名都过 [`utf16_contains`]）。
/// 与 [`enum_proc`] 同一套纪律：整个体套 `catch_unwind`、体内不分配、读不到类名就跳过。
/// 命中后返回 0 提前结束枚举（第一个就是它，继续枚举只会多跑几次 `GetClassNameW`）。
extern "system" fn input_site_proc(hwnd: *mut c_void, l_param: *mut c_void) -> i32 {
    let mut done = false;
    let _ = panic::catch_unwind(AssertUnwindSafe(|| {
        if hwnd.is_null() || l_param.is_null() {
            return;
        }
        let mut buf = [0u16; 64];
        if !utf16_contains(read_class_name(hwnd, &mut buf), INPUT_SITE_NEEDLE) {
            return;
        }
        let slot = unsafe { &*(l_param as *const Cell<usize>) };
        if slot.get() == 0 {
            slot.set(hwnd as usize);
            done = true;
        }
    }));
    if done {
        0
    } else {
        1
    }
}

/// `GetClassNameW` 进调用方的栈缓冲，返回**有效前缀**切片（读不到 / 失败 = 空切片）。零分配。
///
/// 结尾留一格不放：`GetClassNameW` 的返回值不含终止符，但也可能因为缓冲不够而截断 ⇒
/// 截断的名字在任何判定里都只会退化成 [`CLASS_OTHER`]，不会假命中。
fn read_class_name<'a>(hwnd: *mut c_void, buf: &'a mut [u16]) -> &'a [u16] {
    if buf.len() < 2 {
        return &[];
    }
    let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), (buf.len() - 1) as i32) };
    if n <= 0 {
        return &[];
    }
    &buf[..(n as usize).min(buf.len() - 1)]
}

/// UTF-16 类名是否**整串等于**某个 ASCII/ BMP 名字（逐码点，零分配）。
fn utf16_eq(name: &[u16], want: &str) -> bool {
    let mut expected = want.encode_utf16();
    for code in name {
        match expected.next() {
            Some(want) if want == *code => {}
            _ => return false,
        }
    }
    expected.next().is_none()
}

/// UTF-16 类名里是否**含有** `needle`（零分配；`needle` 长过栈缓冲时判不出 ⇒ `false`）。
fn utf16_contains(name: &[u16], needle: &str) -> bool {
    const CAP: usize = 64;
    let need = needle.encode_utf16().count();
    if need == 0 {
        return true;
    }
    if need > CAP {
        return false;
    }
    let mut buf = [0u16; CAP];
    for (slot, code) in buf[..need].iter_mut().zip(needle.encode_utf16()) {
        *slot = code;
    }
    name.len() >= need && name.windows(need).any(|window| window == &buf[..need])
}

/// 类名 → **诊断标签**：这一发到底落进了哪一类窗口。纯函数，离线可穷举。
///
/// 返回的是 [`KNOWN_CLASSES`] 里那几个 `const` 之一（或 [`CLASS_OTHER`] / [`CLASS_NONE`]），
/// 所以能直接进 `format!`、能进 `assert_eq!`。input-site 那一档用**子串**认，因为本机短名
/// `InputSiteWindowClass` 与别处的长名 `Microsoft.UI.Input.InputSite.WindowClass` 都出现过。
fn class_label(name: &[u16]) -> &'static str {
    if utf16_eq(name, WINDOW_CLASS) {
        WINDOW_CLASS
    } else if utf16_contains(name, INPUT_SITE_NEEDLE) {
        INPUT_SITE_CLASS
    } else if utf16_eq(name, BRIDGE_CLASS) {
        BRIDGE_CLASS
    } else if utf16_eq(name, NONCLIENT_CLASS) {
        NONCLIENT_CLASS
    } else if name.is_empty() {
        CLASS_NONE
    } else {
        CLASS_OTHER
    }
}

/// 某个窗口的类名标签（活体 Win32，离线测不了；单测只覆盖 [`class_label`] 那半截纯函数）。
fn class_label_of(hwnd: usize) -> &'static str {
    let mut buf = [0u16; 64];
    class_label(read_class_name(hwnd as *mut c_void, &mut buf))
}

/// 真实光标屏幕位置（物理像素）。`GetCursorPos` 失败 = `None`。
pub fn cursor() -> Option<Point> {
    let mut pt = WinPoint::default();
    if unsafe { GetCursorPos(&mut pt as *mut WinPoint) } == 0 {
        return None;
    }
    Some(Point { x: pt.x, y: pt.y })
}

/// 客户区左上角的屏幕位置 + 这个窗口的 DPI。任一项拿不到 = `None`。
pub fn client_origin() -> Option<(Point, u32)> {
    let hwnd = hwnd()? as *mut c_void;
    let mut rect = WinRect::default();
    let mut origin = WinPoint { x: 0, y: 0 };
    unsafe {
        if GetWindowRect(hwnd, &mut rect as *mut WinRect) == 0 {
            return None;
        }
        if ClientToScreen(hwnd, &mut origin as *mut WinPoint) == 0 {
            return None;
        }
        let dpi = GetDpiForWindow(hwnd);
        // `GetDpiForWindow` 失败返回 0 ⇒ 退到 100%（宁可按 DIP 原值发，也别在缩放未知的路上算飞）。
        Some((Point { x: origin.x, y: origin.y }, if dpi == 0 { 96 } else { dpi }))
    }
}

/// 客户区宽高（**物理像素**）。任一项拿不到 / 非正 = `None`。
///
/// 与 [`client_origin`] 分工：那颗给原点与 DPI，这颗给尺寸，两者合起来才拼得出 [`ChatBox`]。
/// 只用 `GetClientRect` 的 `right/bottom`（该 API 的 `left/top` 恒为 0，是客户区相对坐标，
/// 与 [`GetWindowRect`] 那圈含不可见边框的屏幕坐标**不是一回事**，混用就会整体偏移）。
pub fn client_size() -> Option<(i32, i32)> {
    let raw = hwnd()? as *mut c_void;
    let mut rect = WinRect::default();
    if unsafe { GetClientRect(raw, &mut rect as *mut WinRect) } == 0 {
        return None;
    }
    if rect.right <= 0 || rect.bottom <= 0 {
        return None;
    }
    Some((rect.right, rect.bottom))
}

/// 一次 `post_wheel` 的落点。调用方按这个打 `DIAG:` 行；`Sent` 之外**一个像素都不会动**。
///
/// 变体形状**不许动**（`main.rs` 按 `Posted::Sent { notches, at }` 解构）：#64 新增的投递目标
/// 诊断走 [`Posted::target_class`] / [`Posted::target_hwnd`] 两个只读访问器。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Posted {
    /// 真的发出去了：格子数与实际落点（屏幕物理像素）。
    Sent { notches: i32, at: Point },
    /// `notches == 0`：本来就不该动。
    NothingToSend,
    /// 这条线程上没有本进程的 WinUI 窗口（单测线程、或窗口还没建）。
    NoWindow,
    /// 落点不可信（没记到锚点、`at` 给了 `None` 且 `GetCursorPos` 失败、或点落在窗口外）。
    NoTrustedPoint,
    /// 那个点上**不是本进程的窗口**（[`Route::Drop`]）：注入会把滚轮送进别人的窗口，比不滚更糟。
    /// 与 [`Posted::NoTrustedPoint`] 分开记，是因为「点算对了但被别的程序挡住」是另一件事，
    /// 诊断行上必须能区分（#64：光标停在别的窗口上时，本分叉应当**什么都不做**）。
    NotOurWindow,
    /// `SendInput` 与 `PostMessageW` 都没被收下（注入插进去的条数不够、直投返回 0）。
    Failed,
}

impl Posted {
    /// 这一发**实际到达的窗口**那一类窗口的类名标签（诊断用，见 [`class_label`]）。
    ///
    /// 值是 [`KNOWN_CLASSES`] 之一、或 [`CLASS_OTHER`]、或（没投出去时）[`CLASS_NONE`]。
    /// 直投那发它应当是 [`INPUT_SITE_CLASS`]（唯一实测会滚的那颗，`tmp/qs1-report.md §3`）；
    /// 命中点那一栏仍可能是 [`BRIDGE_CLASS`] —— 两者不同颗是**目标态**，不是 bug（见 [`wheel_target`]）。
    /// 成败判据在 [`Posted::route`]。留这一维只为了一件事：如果哪天它变成
    /// [`CLASS_OTHER`]，说明窗口树换了脸（换了 WinUI 版本 / 出现了遮挡窗口），得重新量。
    ///
    /// **不改变枚举形状**：读的是线程局部槽，所以 `main.rs` 现有的
    /// `Posted::Sent { notches, at }` 解构一行都不用动。前提是**紧跟在本次** [`post_wheel`] /
    /// [`post_wheel_at_window_anchor`] 返回之后读（同一线程、中间没有第二次发送），
    /// 每次进入这两个函数都会先把槽擦成 [`CLASS_NONE`]，所以不会读到上一次的残留。
    pub fn target_class(&self) -> &'static str {
        let _ = self;
        LAST_TARGET.with(|slot| slot.get().1)
    }

    /// 这一发实际投给的窗口句柄（`0` = 没投）。和 `target_class()` 同一套读取约束。
    ///
    /// 给 GUI 代理对账用：拿它跟 `EnumWindows`/`EnumChildWindows` dump 出来的句柄比，
    /// 就能确定「投的到底是哪一个窗口」，而不是靠类名猜。
    pub fn target_hwnd(&self) -> usize {
        let _ = self;
        LAST_TARGET.with(|slot| slot.get().0)
    }

    /// 这一发的**送达方式**：[`ROUTE_INJECT`]（主通路）/ [`ROUTE_POST`]（注入被挡时的兜底直投）/
    /// [`ROUTE_NONE`]（没发出去，或压根没走到发送那一步）。
    ///
    /// 日志里 `route=` 与「UIA 读回的位移」要成对出现：`route=postmessage` 而位移为 0 时**先看
    /// `target_class=`** —— 那栏不是 `InputSiteWindowClass` 就是目标选错了（`tmp/qs1-report.md §3`），
    /// 是它而位移为 0 才是新的未知。
    pub fn route(&self) -> &'static str {
        let _ = self;
        LAST_TARGET.with(|slot| slot.get().2)
    }
}

/// 擦掉上一次的目标诊断；每次发送入口都调一次，保证读到的信息属于本次调用。
fn forget_target() {
    LAST_TARGET.with(|slot| slot.set((0, CLASS_NONE, ROUTE_NONE)));
}

/// 一次滚轮的**送达方式**（#64 判定的落点，纯函数 [`pick_route`] 的输出）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// 系统注入（`SetCursorPos` + `SendInput`）：由系统按光标位置路由 ⇒ XAML 的输入栈会吃。
    /// 在案唯一证明「真滚动了正文」的一条：`tmp/qa6-report.txt:60-62`（1600,800 打 12 格 down ⇒
    /// 本来在视口外的 `qa6-p3-charlie` 落回 `rect=2434,1137 off=False`；补 3×12 格 up 到顶后首行
    /// rect 不再变 = 边界夹紧）。
    Inject,
    /// 直投 WndProc（`PostMessageW`）：返回值非 0 只表示「排进了队列」，与 XAML 滚没滚**没有因果
    /// 关系** ⇒ 每次照实把 `route=postmessage` 写进 [`Posted::route`]。目标态见 [`pick_post_target`]：
    /// 只有 input-site 那颗收得到（`tmp/qs1-report.md §3`），投顶层/bridge 就是 `dy=0` 的空发。
    Post,
    /// 这一点上根本不是本进程的窗口（被别的程序挡住了，或 `WindowFromPoint` 答不上来）。
    /// ⇒ **一个像素都不发**：注入会把滚轮送进别人的窗口里，比不滚更糟。
    Drop,
}

/// 决定这一发滚轮的**送达方式 + 落点窗口**（纯函数，离线可穷举；活体查询在 [`wheel_target`]）。
///
/// `hit` = `WindowFromPoint(屏幕物理像素点)` 命中的**最深层**窗口；`hit_pid` = 它的属主进程；
/// `own_pid` = 本进程；`hit_root` = `GetAncestor(hit, GA_ROOT)`；`top` = 本进程顶层窗口。
///
/// 判据只看**归属**，不看类名：归属是「这一点上是不是我自己的窗口」，而**投递目标**是另一件事，
/// 见 [`pick_post_target`]（那边的目标态是 input-site，不是命中的那一发）。
/// 归属不成立 ⇒ [`Route::Drop`]（绝不往别人进程的窗口里注入消息）。
pub fn pick_route(hit: usize, hit_pid: u32, own_pid: u32, hit_root: usize, top: usize) -> Route {
    if top == 0 || hit == 0 {
        return Route::Drop;
    }
    if hit == top {
        // 点到自家顶层（标题栏/边框/阴影区）：注入仍然只影响这个窗口，发。
        return Route::Inject;
    }
    if own_pid == 0 || hit_pid == 0 || hit_pid != own_pid {
        return Route::Drop;
    }
    if hit_root == 0 || hit_root != top {
        // 同进程但不挂在这个顶层下（另一个弹窗/另一块内容面）⇒ 归属说不清，不发。
        return Route::Drop;
    }
    Route::Inject
}

/// 决定这一发滚轮**直投给谁**（[`Route::Post`] 那条兜底通路用）。
///
/// 目标态是一个两级显式链：`input_site`（自家顶层下那颗类名含 `InputSite` 的子窗口，
/// 由 [`input_site_child`] 现查）→ [`WINDOW_CLASS`] 顶层。命中点（`hit`，内容区上是 bridge）**不再
/// 是目标**：qs1 用 UIA 位移 + 屏幕抓图双口径量过，9 颗窗口里只有 input-site 真滚，bridge 与顶层
/// 都是 `dy=0`（`tmp/qs1-report.md §3`）。
///
/// 归属判定与 [`pick_route`] 同源：`input_site` 之所以可信，是因为它按构造是 `top` 的子孙
/// （[`EnumChildWindows`] 从 `top` 往下枚举）；`Route::Drop` 时它一律**不被采纳**、直接回落 `top`，
/// 而那条路的真正守卫在 [`post_wheel_inner`]——那儿一个像素都不发。
/// `input_site == 0`（没找到，或压根没窗口）⇒ 回落顶层：宁可投一个「大概对」的，也别什么都不做。
pub fn pick_post_target(hit: usize, hit_pid: u32, own_pid: u32, hit_root: usize, top: usize, input_site: usize) -> usize {
    if pick_route(hit, hit_pid, own_pid, hit_root, top) == Route::Drop {
        return top;
    }
    if input_site != 0 {
        return input_site;
    }
    top
}

/// 按屏幕点解析这一发的 `(送达方式, 记进诊断的落点句柄, 类名标签, 兜底直投句柄)`。
///
/// **每次现问、不缓存子窗口句柄**：子窗口随内容增删而换，缓存下来的就是下一次同样的 bug
/// （input-site 那颗连句柄都是每轮换，`tmp/qs1-report.md §3`）。
/// 顶层缓存（[`hwnd`]）不受这条影响，它一个窗口生命周期里不会变。
/// 一次调用只问 `WindowFromPoint` **一遍**（两条通路共用同一份查询结果，绝不在降级路径上再问
/// 一次 —— 中间窗口树可能已经变了，那就是「日志说的目标」与「实际投的目标」不是一回事）。
///
/// 「记进诊断的落点」取 `WindowFromPoint` 的答案 —— 答的是「这一点上到底是哪个窗口」，
/// 与兜底**投给谁**（`post`，通常是 input-site）**故意不是同一颗**：日志里要看得见这两者不一致。
/// 只有 `hit == 0`（系统答不上来）时才退回投的那个句柄做诊断。
fn wheel_target(top: usize, at: Point) -> (Route, usize, &'static str, usize) {
    let hit = unsafe { WindowFromPoint(WinPoint { x: at.x, y: at.y }) } as usize;
    let mut hit_pid = 0u32;
    let mut root = 0usize;
    if hit != 0 {
        unsafe {
            GetWindowThreadProcessId(hit as *mut c_void, &mut hit_pid as *mut u32);
            root = GetAncestor(hit as *mut c_void, GA_ROOT) as usize;
        }
    }
    let own = unsafe { GetCurrentProcessId() };
    let route = pick_route(hit, hit_pid, own, root, top);
    // input-site 只在归属成立时才去枚举：Drop 那条路本来就不发，何必再走一遍窗口树。
    let site = if route == Route::Drop { 0 } else { input_site_child(top) };
    let post = pick_post_target(hit, hit_pid, own, root, top, site);
    let logged = if hit != 0 { hit } else { post };
    (route, logged, class_label_of(logged), post)
}

/// 往系统注入 `notches` 格滚轮：先把光标挪到落点，发完再挪回去。
///
/// 为什么非要挪光标：`SendInput` 的滚轮归谁由**注入那一刻的光标位置**决定（lParam 里的点
/// 只是给收消息的窗口看的参考值，系统不看它）。所以「落点」这件事只能靠光标实现。
/// 代价照实写在这里：
/// · 用户的光标会被瞬移一趟再移回来（`method=screen` 的截图抓不到硬件光标，观感上是一次抖动）；
/// · 挪过去那一下会给落点上的控件补一次真实 pointer-move ⇒ 途经控件的悬停档会闪一拍。
/// 这两条都比「看不见新气泡」轻，而且是分叉唯一的滚动出口带来的必然代价。
///
/// 每发事件固定一格（`±WHEEL_DELTA`），最多 [`MAX_BURST_NOTCHES`] 发 —— 与真实手势同形，
/// 也让 `ScrollViewer` 自己的夹取逐格生效（一次巨型 delta 会被当成一次甩，观感不同）。
/// 返回 `true` = `SendInput` **把请求的每一发都收了**（它返回真正插入的条数，只看 `!= 0` 会撒谎）。
fn inject_wheel(notches: i32, at: Point) -> bool {
    if notches == 0 {
        return false;
    }
    let count = notches.unsigned_abs().min(MAX_BURST_NOTCHES as u32) as usize;
    // 一次事件一格，符号跟 `notches` 一致（正 = 向尾端 = 负增量）。`mouse_data` 是 `DWORD`，
    // 但 `WM_MOUSEWHEEL` 那一发里装的是**有符号**增量 ⇒ 补码原样塞进去（`-120i32 as u32`）。
    let per = if notches > 0 { wheel_delta(1) } else { wheel_delta(-1) };
    let per = per as u32;
    let mut saved = WinPoint::default();
    let had_cursor = unsafe { GetCursorPos(&mut saved as *mut WinPoint) } != 0;
    if unsafe { SetCursorPos(at.x, at.y) } == 0 {
        return false;
    }
    let input = WinInput {
        input_type: INPUT_MOUSE,
        _pad: 0,
        mi: WinMouseInput {
            dx: 0,
            dy: 0,
            mouse_data: per,
            mouse_flags: MOUSEEVENTF_WHEEL,
            time: 0,
            // 手签：同一个 `WH_MOUSE_LL` 钩子会看见自己这一发，靠它把「我滚的」从贴底账本里摘出去
            // （判据见 [`is_own_wheel`]；为什么不能只看 `LLMHF_INJECTED` 也写在那儿）。
            extra_info: INJECT_TAG,
        },
    };
    let events = vec![input; count];
    let inserted = unsafe {
        SendInput(count as u32, events.as_ptr() as *const WinInput, std::mem::size_of::<WinInput>() as i32)
    };
    if had_cursor {
        // 光标复位失败不影响「这一发有没有滚出去」的判定，也不许把整发结果改写成失败。
        unsafe {
            SetCursorPos(saved.x, saved.y);
        }
    }
    inserted as usize == count
}

/// 补 `notches` 格滚轮。**没有可信落点就什么都不发**（见 [`Posted::NoTrustedPoint`]）。
///
/// * `at == Some(p)` —— 由调用方保证 `p` 是屏幕物理像素、且在要滚的那块内容里。
/// * `at == None` —— 用真实光标位置。它的前提是「光标正停在可滚区域上」；不成立时这一发
///   会滚错地方 ⇒ 调用方自己负责这个前提，本模块的默认通路是 [`post_wheel_at_window_anchor`]。
///
/// 点会用 `GetWindowRect` 校一遍：落在本窗口之外就拒发。这道检查存在的理由是 ——
/// 落点决定谁被滚，而分叉唯一能读到的窗口几何就是这个。
/// 校验过了之后，**送达方式与落点由 [`wheel_target`] 现问 `WindowFromPoint` 定**（#64）。
pub fn post_wheel(notches: i32, at: Option<Point>) -> Posted {
    post_wheel_inner(notches, at, false)
}

/// [`post_wheel`] 的贴底版：格数夹到 [`MAX_BURST_NOTCHES`] 而不是 12。
///
/// 只给 #89 那条「绝对贴底」通路用（判据见 [`Stick`] 与 [`stick_action`]）：分叉读不回
/// 视口位置 ⇒ 贴底只能靠**故意过滚** + `ScrollViewer` 自己的边界夹取。
pub fn post_wheel_burst(notches: i32, at: Option<Point>) -> Posted {
    post_wheel_inner(notches, at, true)
}

fn post_wheel_inner(notches: i32, at: Option<Point>, burst: bool) -> Posted {
    forget_target();
    let notches = if burst { clamp_burst(notches) } else { clamp_notches(notches) };
    if notches == 0 {
        return Posted::NothingToSend;
    }
    let Some(raw) = hwnd() else {
        return Posted::NoWindow;
    };
    let Some(point) = at.or_else(cursor) else {
        return Posted::NoTrustedPoint;
    };
    if !point_in_window(raw, point) {
        return Posted::NoTrustedPoint;
    }
    let (route, target, label, post) = wheel_target(raw, point);
    LAST_TARGET.with(|slot| slot.set((target, label, ROUTE_NONE)));
    if route == Route::Drop {
        // 槽里的句柄保持 `target`（= 回落的顶层），但路由擦成 `none`：这一发**没发**。
        return Posted::NotOurWindow;
    }
    if inject_wheel(notches, point) {
        LAST_TARGET.with(|slot| slot.set((target, label, ROUTE_INJECT)));
        return Posted::Sent { notches, at: point };
    }
    // 注入被挡（`SendInput` 少插了、或 `SetCursorPos` 失败）⇒ 退到旧通路，并照实记 route。
    // 用的还是**同一次** `WindowFromPoint` 的结果（`post`），绝不在这儿再问一遍：中间窗口树可能
    // 已经变了，那样日志里的目标就不是实际投的目标。
    let lp = wheel_l_param(point) as *mut c_void;
    let param = if burst { burst_w_param(notches) } else { wheel_w_param(notches) };
    LAST_TARGET.with(|slot| slot.set((post, class_label_of(post), ROUTE_POST)));
    if unsafe { PostMessageW(post as *mut c_void, WM_MOUSEWHEEL, param, lp) } == 0 {
        return Posted::Failed;
    }
    Posted::Sent { notches, at: point }
}

/// 用 [`anchor`]（真实指针事件记下来的窗口内坐标）换算落点再发。没记到锚点 = 什么都不发。
///
/// 这条是本模块对外的**默认**通路：它的落点是「指针确实在那块内容上」这件事的证据，
/// 而 `at: None` 那条只是「光标大概还在那儿」的推测。
pub fn post_wheel_at_window_anchor(notches: i32) -> Posted {
    let Some(window) = anchor() else {
        forget_target();
        return Posted::NoTrustedPoint;
    };
    let Some((origin, dpi)) = client_origin() else {
        forget_target();
        return Posted::NoWindow;
    };
    post_wheel(notches, Some(anchor_to_point(window, origin, dpi)))
}

/// 用**调用方给定的**窗口内 DIP 点补一发贴底滚轮 —— #89 贴底唯一出口（`main.rs` 的产品调用点）。
///
/// 为什么不要求指针证据（这是 #89 的关键）：主干 `ScrollTranscriptToBottom`（MW:5727）从不问光标
/// 在哪 —— 用户发 prompt 时指针恒在 composer 的输入框里，那时主干照样把新气泡拽进视野。
/// 所以贴底这一发**必须**能在没有指针证据时发出，否则「发完 prompt 看不见新气泡」这个 bug
/// 就原地不动。有证据时调用方喂进来的就是 [`AnchorSource::Chat`] 那颗锚点，走的还是这一条。
///
/// 代价照实写：落点的正确性从「指针证据」降级为「调用方的几何」⇒ 必须由 [`ChatBox::wheel_anchor`]
/// 给出**恒在正文带内**的点（含轨的命中区排除）。除此之外与 [`post_wheel`] 同路：照样过
/// [`point_in_window`] 的窗口校验、照样由 [`wheel_target`] 现问归属、照样 `Route::Drop` 时一个像素不发。
/// **不写 [`ANCHOR`]**：那是浮层滚轮的可信落点，塞进一个合成点会让
/// [`follow_palette_selection`] 那类读取方误以为指针在正文里 —— 分叉这里宁可两条通路并存。
pub fn post_wheel_burst_at_window_point(notches: i32, window: Point) -> Posted {
    let Some((origin, dpi)) = client_origin() else {
        forget_target();
        return Posted::NoWindow;
    };
    post_wheel_burst(notches, Some(anchor_to_point(window, origin, dpi)))
}

/// 点是否落在这个窗口的 `GetWindowRect` 里（含边界）。几何拿不到时按「不可信」处理。
fn point_in_window(hwnd: usize, point: Point) -> bool {
    let mut rect = WinRect::default();
    if unsafe { GetWindowRect(hwnd as *mut c_void, &mut rect as *mut WinRect) } == 0 {
        return false;
    }
    point.x >= rect.left && point.x <= rect.right && point.y >= rect.top && point.y <= rect.bottom
}

// ---------------------------------------------------------------- 用户滚轮观测（#89 的输入源）

/// 低级鼠标钩子看见的那一发滚轮 → 换算成本模块的符号（正 = 向尾端）。纯函数。
///
/// * `mouse_data` 的**低半字**才是增量（`GET_WHEEL_DELTA_WPARAM` 同一条口径，高半字是
///   历史遗留的转数），所以要过 `as u16 as i16` 而不是直接 `as i32`。
/// * **符号取负**：钩子给的是「轮子怎么转」（向下滚 = 负），本模块记的是「内容怎么走」（向尾端 = 正）。
/// * 不到一格的增量（`|delta| < 120`，触控板常见）→ `None`：账本只认整格，半格不记账，
///   免得攒够一格时算漏。
/// * 「这一发是不是分叉自己注的」**不在这里判** —— 见 [`is_own_wheel`]：只按 [`LLMHF_INJECTED`]
///   挡会把测试驱动的模拟滚轮一起挡掉，那条规则就当没实现过。
pub fn user_wheel_notches(mouse_data: u32) -> Option<i32> {
    let delta = ((mouse_data & 0xFFFF) as u16) as i16 as i32;
    let notches = -delta / WHEEL_DELTA;
    (notches != 0).then_some(notches)
}

/// 自家注入那几发带的手签（`MOUSEINPUT::dwExtraInfo`）。
///
/// 为什么不用「时间窗」之类的花活：钩子回调是在**装它的线程**上投递的，`SendInput` 返回之后才会
/// 跑，事件早就离开调用栈了，拿不到「是不是我刚发的那一发」的同步证据。带在手上的魔数才是硬证据。
const INJECT_TAG: usize = 0xB0DE_2F00_0000_0001;

/// 这一发是不是**分叉自己**注进来的（纯函数）。
///
/// 必须**两个条件同时成立**：`LLMHF_INJECTED` 说明它是注入的、`extra_info` 说明是**我们**注的。
/// 只认前者就够挡掉自家那一发，但那样一来 `tmp/sc2-wheel.ps1` 用 `SendInput` 模拟的「用户上翻」
/// 也会被一起挡掉 ⇒ #89 的「停止跟随」那条规则在真机上永远测不到（这是本模块刻意要避免的
/// 「假实现」：判据在，输入源被自己掐了）。
pub fn is_own_wheel(flags: u32, extra_info: usize) -> bool {
    flags & LLMHF_INJECTED != 0 && extra_info == INJECT_TAG
}

/// 钩子那一发**该不该记进贴底账本**：自家的不算、不到一格的不算。纯函数（[`is_own_wheel`] 与
/// [`user_wheel_notches`] 的组合，调用方只关心「记几格」）。
pub fn observed_wheel(flags: u32, mouse_data: u32, extra_info: usize) -> Option<i32> {
    if is_own_wheel(flags, extra_info) {
        return None;
    }
    user_wheel_notches(mouse_data)
}

/// 同一拍里连着滚了两下时合并：格数相加、落点取**最新**那一个。纯函数。
pub fn merge_user_wheel(prev: Option<(i32, Point)>, notches: i32, at: Point) -> (i32, Point) {
    match prev {
        Some((had, _)) => (had.saturating_add(notches), at),
        None => (notches, at),
    }
}

/// 装低级鼠标钩子（`WH_MOUSE_LL`）。返回 `false` = 没装上 ⇒ #89 的「用户上翻就停跟随」
/// 那一半就没有输入源，此时分叉的行为退回「一直跟随」（与主干拿不到滚动面时的
/// `IsTranscriptAtBottom() == true` 同一条回落，不是新 bug，但要在 `DIAG:` 里写明）。
///
/// 低级钩子是**系统级**的：`dwThreadId` 必须给 0、`hMod` 给本模块句柄（`tmp/probe-keys.rs.bak:185`
/// 实测 `ll=true` 的那一句就是这个形状），但回调仍然跑在**装它的那个线程**上 ⇒ 账本照样是
/// `thread_local!`。与 `keys.rs:300` 同一条约束：**只在 UI 线程装**，且退出时必须
/// [`uninstall_wheel_watch`]，漏掉 `UnhookWindowsHookEx` 就是把这一枪永远挂在系统钩子链上。
pub fn install_wheel_watch() -> bool {
    if WHEEL_HOOK.with(|slot| !slot.get().is_null()) {
        return true; // 已经挂着：重复装会多一条回调，格数会被记两遍
    }
    let module = unsafe { GetModuleHandleW(std::ptr::null()) };
    let handle = unsafe { SetWindowsHookExW(WH_MOUSE_LL, wheel_proc as *const c_void, module, 0) };
    WHEEL_HOOK.with(|slot| slot.set(handle));
    !handle.is_null()
}

/// 卸钩子。`true` = 真卸掉了（或压根没装过）。
pub fn uninstall_wheel_watch() -> bool {
    let handle = WHEEL_HOOK.with(|slot| slot.get());
    WHEEL_HOOK.with(|slot| slot.set(std::ptr::null_mut()));
    USER_WHEEL.with(|slot| slot.set(None));
    if handle.is_null() {
        return true;
    }
    unsafe { UnhookWindowsHookEx(handle) != 0 }
}

/// 钩子是否还挂着（诊断行用）。
pub fn wheel_watch_installed() -> bool {
    WHEEL_HOOK.with(|slot| !slot.get().is_null())
}

/// 取走并清空「用户自己滚的格数」。`None` = 这一拍没人滚过。
pub fn take_user_wheel() -> Option<(i32, Point)> {
    USER_WHEEL.with(|slot| slot.take())
}

/// 钩子入口。**任何路径都不许 panic**（穿过 `extern "system"` 边界 = 进程直接没了、
/// stdout 0 字节，`keys.rs` 头注同一条教训）：判定体整个套 `catch_unwind`，体内**不分配**，
/// 且无论如何都要把这一发**放行**（低级钩子返回 0 才是吞，这里永远 `CallNextHookEx`）。
extern "system" fn wheel_proc(code: i32, w_param: usize, l_param: *const c_void) -> usize {
    let _ = panic::catch_unwind(AssertUnwindSafe(|| {
        note_user_wheel(code, w_param, l_param);
    }));
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param) }
}

/// 钩子真正做的事：竖向滚轮 + 不是自家那一发 + 落点是自家窗口 ⇒ 记一格数。
///
/// 三条守卫一个都不能省：
/// * [`WM_MOUSEHWHEEL`]（横向）显式挡掉 —— 触控板横滑很常见，分叉所有滚动带都是竖向的，
///   混进账本就是「用户没上翻却被判成上翻」。
/// * [`observed_wheel`] 把自家注入的那一发剔掉（手签见 [`INJECT_TAG`]）。
/// * [`pick_route`] 复用发送侧同一条归属判定 —— 光标在别的进程上滚的那一发与本文的账本无关。
fn note_user_wheel(code: i32, w_param: usize, l_param: *const c_void) {
    if code != HC_ACTION || l_param.is_null() {
        return; // 被忽略的 code：低级钩子是全局热点，先返回再说话
    }
    if w_param == WM_MOUSEHWHEEL {
        return; // 横向滚轮：本模块只记竖向
    }
    if w_param != WM_MOUSEWHEEL as usize {
        return; // 移动/按键/IME…：一律早退
    }
    let raw = unsafe { &*(l_param as *const WinMouseHookStruct) };
    let Some(notches) = observed_wheel(raw.mouse_flags, raw.mouse_data, raw.extra_info) else {
        return;
    };
    let at = Point { x: raw.point.x, y: raw.point.y };
    let Some(top) = hwnd() else {
        return;
    };
    // 归属判定复用发送侧那同一条 [`pick_route`]：不是自家窗口上的滚轮与贴底账本无关。
    let hit = unsafe { WindowFromPoint(WinPoint { x: at.x, y: at.y }) } as usize;
    let mut hit_pid = 0u32;
    let mut root = 0usize;
    if hit != 0 {
        unsafe {
            GetWindowThreadProcessId(hit as *mut c_void, &mut hit_pid as *mut u32);
            root = GetAncestor(hit as *mut c_void, GA_ROOT) as usize;
        }
    }
    if pick_route(hit, hit_pid, unsafe { GetCurrentProcessId() }, root, top) != Route::Inject {
        return;
    }
    USER_WHEEL.with(|slot| slot.set(Some(merge_user_wheel(slot.get(), notches, at))));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 命令浮层那一档的几何（与 `main.rs` 的常量同源，这里独立写死一遍当交叉核对）。
    fn palette_band() -> Band {
        Band { row_dip: 26.0, page_dip: 210.0, dip_per_notch: 48.0 }
    }

    /// `wParam`：增量在高位字、符号在短整数里、低位字（`MK_*`）恒 0。
    ///
    /// 符号口径 = 探针实测那一发（`tmp/probe-keys.rs.bak`：`delta=-1200` 是「10 格**向尾端**」，
    /// UIA 读出 `VerticalScrollPercent 0 → 100`）。本模块对外的 `notches` 与增量**反号**。
    #[test]
    fn wheel_w_param_packs_signed_delta_in_high_word() {
        // 10 格向尾端 = 负增量（滚轮向下 = 轮子朝自己这侧转）。
        assert_eq!(wheel_delta(10), -1200);
        assert_eq!(wheel_delta(-10), 1200, "向头端（视觉上向上）才是正增量");
        assert_eq!(wheel_delta(0), 0);
        assert_eq!(wheel_w_param(0), 0);
        assert_eq!(wheel_w_param(10) >> 16, (-1200i32) as u16 as usize);
        assert_eq!(wheel_w_param(10) & 0xFFFF, 0, "低位字是 MK_* 修饰键位，必须留空");
        // 两半都要能被 `GET_WHEEL_DELTA_WPARAM`（`(short)HIWORD(wParam)`）原样读回。
        for notches in [-10i32, -1, 0, 1, 10, MAX_NOTCHES] {
            let packed = wheel_w_param(notches);
            let read_back = (((packed >> 16) & 0xFFFF) as u16) as i16;
            assert_eq!(read_back as i32, wheel_delta(notches), "notches={notches} 往返还号");
        }
        // 端点：夹到 MAX_NOTCHES，不越界。
        assert_eq!(wheel_delta(i32::MAX), -MAX_NOTCHES * WHEEL_DELTA);
        assert_eq!(wheel_delta(i32::MIN), MAX_NOTCHES * WHEEL_DELTA);
        // 逐格发的那一发（`inject_wheel` 每事件一格）也必须同号。
        assert_eq!(wheel_delta(1), -WHEEL_DELTA);
        assert_eq!(wheel_delta(-1), WHEEL_DELTA);
    }

    /// 贴底那一发的同一套打包口径 + 更宽的夹取：符号与 `wheel_delta` 一致，上限换 200 格。
    #[test]
    fn burst_delta_shares_the_sign_convention_and_a_wider_cap() {
        assert_eq!(burst_delta(128), -(128 * WHEEL_DELTA), "90 条 seed 历史那一发");
        assert_eq!(burst_delta(-128), 128 * WHEEL_DELTA);
        assert_eq!(burst_delta(i32::MAX), -MAX_BURST_NOTCHES * WHEEL_DELTA);
        assert_eq!(burst_delta(i32::MIN), MAX_BURST_NOTCHES * WHEEL_DELTA);
        // 12 格这一档两条通路必须给同一个数（贴底通路只是预算更大，口径不许分叉）。
        for notches in [-12, -1, 0, 1, 12] {
            assert_eq!(burst_delta(notches), wheel_delta(notches), "notches={notches}");
        }
        assert_eq!(burst_w_param(3), pack_delta(burst_delta(3)));
        assert_eq!(burst_w_param(3) & 0xFFFF, 0, "低位字同样留给 MK_*");
        assert!(MAX_BURST_NOTCHES > MAX_NOTCHES, "两条通路的差别就是预算");
    }

    /// 两套夹取各管各的：相对通路 12 格、绝对通路 200 格，符号与 0 都保留。
    #[test]
    fn clamp_burst_is_wider_than_clamp_notches() {
        assert_eq!(clamp_notches(100), MAX_NOTCHES);
        assert_eq!(clamp_burst(100), 100, "贴底那一发允许 100 格");
        assert_eq!(clamp_burst(MAX_BURST_NOTCHES), MAX_BURST_NOTCHES);
        assert_eq!(clamp_burst(MAX_BURST_NOTCHES + 1), MAX_BURST_NOTCHES);
        assert_eq!(clamp_burst(-MAX_BURST_NOTCHES - 1), -MAX_BURST_NOTCHES);
        assert_eq!(clamp_burst(0), 0);
        assert_eq!(clamp_burst(i32::MAX), MAX_BURST_NOTCHES);
        assert_eq!(clamp_burst(i32::MIN), -MAX_BURST_NOTCHES);
    }

    /// `SendInput` 的两个结构体必须逐字节对上 Win32 的 `INPUT` / `MOUSEINPUT`。
    ///
    /// 这条不是洁癖：`SendInput` 按 `cbSize` 逐条切数组，尺寸或偏移错了就是把后面几发的字段读歪
    /// —— 表现是「发了 12 格、只滚了 1 格」这种最难查的症状。
    #[test]
    fn sendinput_struct_layout_matches_the_win32_abi() {
        assert_eq!(std::mem::size_of::<WinMouseInput>(), 32);
        assert_eq!(std::mem::size_of::<WinInput>(), 40, "DWORD type + 4 补齐 + 32 字节 union");
        assert_eq!(std::mem::align_of::<WinInput>(), 8, "dwExtraInfo 是 ULONG_PTR");
        assert_eq!(std::mem::offset_of!(WinInput, mi), 8);
        assert_eq!(std::mem::offset_of!(WinMouseInput, mouse_data), 8);
        assert_eq!(std::mem::offset_of!(WinMouseInput, mouse_flags), 12);
        assert_eq!(std::mem::offset_of!(WinMouseInput, extra_info), 24);
        // `MSLLHOOKSTRUCT`：POINT(8) + DWORD + DWORD + DWORD + 补齐 + ULONG_PTR ⇒ extra_info 在 24。
        assert_eq!(std::mem::size_of::<WinMouseHookStruct>(), 32);
        assert_eq!(std::mem::offset_of!(WinMouseHookStruct, point), 0);
        assert_eq!(std::mem::offset_of!(WinMouseHookStruct, mouse_data), 8);
        assert_eq!(std::mem::offset_of!(WinMouseHookStruct, mouse_flags), 12);
        assert_eq!(std::mem::offset_of!(WinMouseHookStruct, time), 16);
        assert_eq!(std::mem::offset_of!(WinMouseHookStruct, extra_info), 24);
        // 竖向滚轮那一位没写错族（0x0800；0x0200/0x1000 是横向那一族）。
        assert_eq!(MOUSEEVENTF_WHEEL, 0x0800);
        assert_eq!(INPUT_MOUSE, 0);
        assert_eq!(WM_MOUSEWHEEL, 0x020A);
    }

    /// `lParam`：`SHORT` 补码，负坐标不许环绕；超界端点化。
    #[test]
    fn wheel_l_param_keeps_negative_coordinates() {
        let unpack = |packed: usize| {
            (
                ((packed & 0xFFFF) as u16) as i16,
                (((packed >> 16) & 0xFFFF) as u16) as i16,
            )
        };
        // 本机主屏 200%，副屏在左边 ⇒ 光标落在副屏就是负 x。
        assert_eq!(unpack(wheel_l_param(Point { x: -1920, y: 400 })), (-1920, 400));
        assert_eq!(unpack(wheel_l_param(Point { x: -32768, y: -32768 })), (i16::MIN, i16::MIN));
        assert_eq!(unpack(wheel_l_param(Point { x: 32767, y: 0 })), (i16::MAX, 0));
        // 探针实测那一发（正坐标）也得逐位对上：cursor=(1832,839) -> lp=0x03470728。
        assert_eq!(wheel_l_param(Point { x: 1832, y: 839 }), 0x0347_0728);
        // 超出 i16 的点是病态输入：钉端点，绝不静默环绕回正数。
        assert_eq!(unpack(wheel_l_param(Point { x: 40_000, y: -40_000 })), (i16::MAX, i16::MIN));
        assert_eq!(wheel_l_param(Point::default()), 0);
    }

    /// 格数夹取：符号保留、0 是 0、端点不环绕。
    #[test]
    fn notch_clamping_keeps_sign_and_zero() {
        assert_eq!(clamp_notches(0), 0);
        assert_eq!(clamp_notches(1), 1);
        assert_eq!(clamp_notches(-1), -1);
        assert_eq!(clamp_notches(MAX_NOTCHES), MAX_NOTCHES);
        assert_eq!(clamp_notches(-MAX_NOTCHES), -MAX_NOTCHES);
        assert_eq!(clamp_notches(MAX_NOTCHES + 1), MAX_NOTCHES);
        assert_eq!(clamp_notches(-MAX_NOTCHES - 1), -MAX_NOTCHES);
        assert_eq!(clamp_notches(i32::MAX), MAX_NOTCHES);
        assert_eq!(clamp_notches(i32::MIN), -MAX_NOTCHES);
    }

    /// 带子能装几行：整除向下取整，病态几何至少 1 行。
    #[test]
    fn rows_visible_counts_whole_rows() {
        assert_eq!(rows_visible(palette_band()), 8); // 210 / 26 = 8.07 -> 8
        assert_eq!(rows_visible(Band { row_dip: 26.0, page_dip: 26.0, dip_per_notch: 48.0 }), 1);
        assert_eq!(rows_visible(Band { row_dip: 26.0, page_dip: 25.0, dip_per_notch: 48.0 }), 1);
        assert_eq!(rows_visible(Band { row_dip: 0.0, page_dip: 210.0, dip_per_notch: 48.0 }), 1);
        assert_eq!(
            rows_visible(Band { row_dip: f64::NAN, page_dip: f64::INFINITY, dip_per_notch: 48.0 }),
            1
        );
    }

    /// 量化：DIP → 格数，向上取整、非零至少 1、0 DIP 是 0 格、每格 DIP 未知就一格不发。
    #[test]
    fn quantiser_rounds_up_and_stays_silent_on_unknown_step() {
        let band = palette_band();
        assert_eq!(notches_for_dip(0.0, band), 0);
        assert_eq!(notches_for_dip(1.0, band), 1, "1 DIP 也要一整格，滚轮没有半格");
        assert_eq!(notches_for_dip(48.0, band), 1, "恰好一格");
        assert_eq!(notches_for_dip(49.0, band), 2, "多 1 DIP 就是多一格");
        assert_eq!(notches_for_dip(-49.0, band), -2);
        assert_eq!(notches_for_dip(96.0, band), 2, "恰好两格");
        assert_eq!(notches_for_dip(10_000.0, band), MAX_NOTCHES, "越界夹住");
        assert_eq!(notches_for_dip(f64::NAN, band), 0);
        assert_eq!(notches_for_dip(f64::INFINITY, band), 0);
        for bad in [0.0, -48.0, f64::NAN] {
            assert_eq!(
                notches_for_dip(120.0, Band { row_dip: 26.0, page_dip: 210.0, dip_per_notch: bad }),
                0,
                "每格多少 DIP 不知道就不该发"
            );
        }
    }

    /// 可见带判定的三个边界：带内、恰好压最后一行、越界一行。
    #[test]
    fn keep_in_view_at_the_band_boundaries() {
        let band = palette_band(); // 8 行可见
        // 已经在带里（含首尾两行）⇒ 一个像素都不动。
        assert_eq!(keep_in_view(band, 0.0, 0, 20), None);
        assert_eq!(keep_in_view(band, 0.0, 7, 20), None, "第 7 行是可见第 8 行，压边也算看见");
        assert_eq!(keep_in_view(band, 4.0, 11, 20), None, "带中间");
        // 往尾端越界一行：走 1 行 = 26 DIP -> 1 格（48 DIP/格）。
        let one = keep_in_view(band, 0.0, 8, 20).expect("第 8 行在带外，必须补");
        assert_eq!(one.notches, 1);
        assert_eq!(one.top_rows, 48.0 / 26.0);
        // 往尾端越界很多：从 0 走到第 19 行（最后一行），要 12 行 = 312 DIP -> ceil(312/48) = 7 格。
        assert_eq!(keep_in_view(band, 0.0, 19, 20).map(|f| f.notches), Some(7));
        // 往头端：选中在第 12 行而视口停在第 16 行 ⇒ 4 行 = 104 DIP -> 负 3 格。
        let up = keep_in_view(band, 16.0, 12, 20).expect("选中在视口上方");
        assert_eq!(up.notches, -3);
        assert!(up.top_rows < 16.0, "{up:?}");
        assert!(up.top_rows > 6.0, "补过头也得让第 12 行回到带内：{up:?}");
        // 没有选中项 / 越界 / 几何坏了 ⇒ 什么都不发。
        assert_eq!(keep_in_view(band, 0.0, -1, 20), None, "主干收起态的 index=-1");
        assert_eq!(keep_in_view(band, 0.0, 0, 0), None, "零候选");
        assert_eq!(keep_in_view(band, 0.0, 20, 20), None);
        assert_eq!(keep_in_view(band, f64::NAN, 3, 20), None);
        assert_eq!(
            keep_in_view(Band { row_dip: 26.0, page_dip: 210.0, dip_per_notch: 0.0 }, 0.0, 8, 20),
            None,
            "每格 DIP 未知时宁可不滚"
        );
    }

    /// 一格顶不满一行时也必须前进（夹到至少 1 格），且 `top_rows` 单调、不会退成负数。
    #[test]
    fn follow_progresses_monotonically_toward_the_tail() {
        let band = palette_band();
        let mut top = 0.0;
        for index in 0..20i32 {
            if let Some(follow) = keep_in_view(band, top, index, 20) {
                assert!(follow.notches > 0, "index={index} 只可能往尾端走：{follow:?}");
                assert!(follow.top_rows > top, "补完必须真的往前走：{follow:?}");
                top = follow.top_rows;
            }
            // 走完一遍后，选中行始终落在估计带内。
            assert!((f64::from(index) - top) < rows_visible(band) as f64, "index={index} top={top}");
            assert!(top >= 0.0);
        }
    }

    /// DIP → 物理像素：200% 那一档必须翻倍，负坐标与坏输入不得环绕。
    #[test]
    fn dip_to_px_scales_by_window_dpi() {
        assert_eq!(dip_to_px(0.0, 96), 0);
        assert_eq!(dip_to_px(10.0, 96), 10);
        assert_eq!(dip_to_px(10.0, 192), 20, "本机主屏 200%");
        assert_eq!(dip_to_px(10.0, 144), 15, "150%：10*1.5=15");
        assert_eq!(dip_to_px(-10.0, 192), -20, "副屏在左边时的负偏移");
        assert_eq!(dip_to_px(10.5, 96), 11, "四舍五入，不截断");
        assert_eq!(dip_to_px(10.0, 0), 10, "问不到 DPI 时按 100% 回落");
        assert_eq!(dip_to_px(f64::NAN, 192), 0);
        assert_eq!(dip_to_px(f64::INFINITY, 192), 0);
    }

    /// 窗口内 DIP + 客户区原点 → 屏幕点；再把结果重新打包成 lParam，全程不丢符号。
    #[test]
    fn window_anchor_converts_to_a_signed_screen_point() {
        // 主屏 200%、客户区左上角在 (98,98)：指针在窗口内 (640, 700) DIP。
        let at = anchor_to_point(
            Point { x: 640, y: 700 },
            Point { x: 98, y: 98 },
            192,
        );
        assert_eq!(at, Point { x: 98 + 1280, y: 98 + 1400 });
        let packed = wheel_l_param(at);
        let x = ((packed & 0xFFFF) as u16) as i16;
        let y = (((packed >> 16) & 0xFFFF) as u16) as i16;
        assert_eq!((i32::from(x), i32::from(y)), (at.x, at.y), "往返必须同值");
        // 副屏在主屏左边：客户区原点本身是负的。
        let left = anchor_to_point(Point { x: 20, y: 10 }, Point { x: -1920, y: 0 }, 96);
        assert_eq!(left, Point { x: -1900, y: 10 });
        assert_eq!(
            ((wheel_l_param(left) & 0xFFFF) as u16) as i16,
            -1900,
            "负 x 在 lParam 里必须是补码"
        );
    }

    /// 这条线程上没有本进程的 WinUI 窗口 ⇒ 只能「拒绝」，不得 panic、不得发消息。
    ///
    /// 这一条同时是「不许 panic」那条纪律的离线守卫：`hwnd()` 会真调 `EnumThreadWindows` 并跑
    /// [`enum_proc`]，那是一处 `extern "system"` 回调。
    #[test]
    fn posting_without_a_window_refuses_instead_of_panicking() {
        forget_anchor(); // 线程池会复用线程：别依赖「谁先跑」
        assert!(hwnd().is_none(), "单测线程上不该有 WinUIDesktopWin32WindowClass");
        assert_eq!(post_wheel(3, Some(Point { x: 10, y: 10 })), Posted::NoWindow);
        assert_eq!(post_wheel(-3, None), Posted::NoWindow);
        assert_eq!(post_wheel_at_window_anchor(2), Posted::NoTrustedPoint, "没记到锚点");
        // 0 格在任何 Win32 之前就该回绝。
        assert_eq!(post_wheel(0, Some(Point { x: 10, y: 10 })), Posted::NothingToSend);
    }

    /// 锚点那三个入口的语义：记 / 读 / 抹，且抹掉之后换算通路立刻回绝。
    ///
    /// `anchor_source()` 这一维是 #89 的「这一发滚轮滚的是不是正文」的唯一证据 ⇒ 单独钉：
    /// 覆盖时必须连带来源一起换，不许出现「点是正文的、来源还写着浮层」那种半新半旧状态。
    #[test]
    fn anchor_note_read_forget_round_trip() {
        forget_anchor(); // 同上：线程池复用，先擦干净再断言初值
        assert!(anchor().is_none(), "初值必须是「没记过」");
        assert!(anchor_source().is_none(), "初值必须是「没记过」");
        note_anchor(AnchorSource::Chat, Point { x: -7, y: 33 });
        assert_eq!(anchor(), Some(Point { x: -7, y: 33 }));
        assert_eq!(anchor_source(), Some(AnchorSource::Chat));
        note_anchor(AnchorSource::Palette, Point { x: 1, y: 2 });
        assert_eq!(anchor(), Some(Point { x: 1, y: 2 }), "后记的覆盖先记的");
        assert_eq!(anchor_source(), Some(AnchorSource::Palette), "来源必须跟着一起覆盖");
        forget_anchor();
        assert!(anchor().is_none());
        assert!(anchor_source().is_none());
        assert_eq!(post_wheel_at_window_anchor(1), Posted::NoTrustedPoint);
        forget_anchor(); // 重复抹不炸
    }

    /// 只跑纯逻辑的运行时守卫：真实几何常量（26 / 210）下整张表的行为，包括「浮层一屏装 8 行」
    /// 这条断言 —— 它是 `main.rs` 那边估计口径的第二处定义，两边对不上就是有人在偷改几何。
    #[test]
    fn palette_geometry_quantiser_agrees_with_the_shipped_constants() {
        let band = Band { row_dip: 26.0, page_dip: 210.0, dip_per_notch: 48.0 };
        assert_eq!(rows_visible(band), 8);
        // 主干 ScrollIntoView 只在越界时动，这里也一样：前 8 行一次都不发。
        for index in 0..8i32 {
            assert_eq!(keep_in_view(band, 0.0, index, 20), None, "第 {index} 行该在一屏里");
        }
        assert_eq!(keep_in_view(band, 0.0, 8, 20).map(|f| f.notches), Some(1));
    }

    // ---- #64：投递目标（类名判定 + 归属校验 + 回落选择）的**纯**那半截 ---------------------
    //
    // ⚠ 划分线：`WindowFromPoint` / `GetWindowThreadProcessId` / `GetAncestor` /
    // `GetCurrentProcessId` / `GetClassNameW` / `SendInput` 都要活体窗口才答得出来，
    // [`wheel_target`] 与 [`inject_wheel`] 整体**离线测不了** ⇒ 这里只钉死它们调用的那几个纯函数
    // （`pick_route`、`pick_post_target`、`class_label`）。
    // 「注入到底滚不滚得动」是运行时结论，只能由 GUI 代理验（口径见 `tmp/sc2-wheel.ps1` +
    // `tmp/sc2-report.txt`；`PostMessage` 直投的目标态由 `tmp/qs1-report.md §3` 量出来 —— 只有 input-site 滚）。

    /// UTF-16 化一个类名（测试专用的输入构造器）。
    fn utf16(name: &str) -> Vec<u16> {
        name.encode_utf16().collect()
    }

    /// 归属成立 ⇒ 注入，并且兜底直投的是**能收到消息的那颗 input-site**（不是命中点、不是顶层）。
    ///
    /// 这一条钉 qs1 的实测结论：内容区上 `WindowFromPoint` 给的仍是 `DesktopChildSiteBridge`
    /// （归属判定照旧过），但直投的目标态是 input-site（`tmp/qs1-report.md §3`，唯一直投会滚的那颗）。
    /// 归属判据里**没有**类名，所以窗口树换脸也不会突然变成「什么都不发」。
    #[test]
    fn pick_route_injects_onto_a_trusted_child_window() {
        let (top, hit, own) = (0x1000usize, 0x2000usize, 4242u32);
        assert_eq!(pick_route(hit, own, own, top, top), Route::Inject, "本进程 + 挂在本顶层下 → 注入");
        assert_eq!(pick_post_target(hit, own, own, top, top, 0), top, "没找到 input-site → 回落顶层");
        // 命中的就是顶层本身（点到标题栏/边框/阴影）→ 照样注入，目标还是那颗 input-site。
        assert_eq!(pick_route(top, own, own, top, top), Route::Inject);
        assert_eq!(pick_post_target(top, own, own, top, top, 0), top);
        // 只有「注入 / 丢弃」两档：`Route::Post` 由调用方在注入被挡后才构造（见 post_wheel_inner）。
        assert_ne!(pick_route(hit, own, own, top, top), Route::Drop);
    }

    /// 目标态：直投兜底**先找 input-site**，找不到才回落顶层；命中点（bridge）不再是目标。
    ///
    /// qs1 逐颗 `PostMessage` 实测：只有 input-site 真滚（`dy=±1188`），顶层与 bridge 都 `dy=0`
    /// ⇒ 「投 hit」这个旧行为必须被钉死成过去式（`tmp/qs1-report.md §3`）。
    #[test]
    fn post_target_prefers_the_input_site_over_hit_and_top() {
        let (top, hit, site, own) = (0x1000usize, 0x2000usize, 0x3000usize, 4242u32);
        assert_eq!(pick_post_target(hit, own, own, top, top, site), site, "命中 bridge 也投 input-site");
        assert_eq!(pick_post_target(top, own, own, top, top, site), site, "点标题栏也投 input-site");
        assert_eq!(pick_post_target(site, own, own, top, top, site), site, "命中点恰好就是它 → 同一颗");
        assert_ne!(pick_post_target(hit, own, own, top, top, site), hit, "命中点不再是目标态");
        assert_ne!(pick_post_target(hit, own, own, top, top, site), top, "有 input-site 就不许回落顶层");
    }

    /// 回落次序（input-site → 顶层）**全穷举**：每一格归属证据都不许让目标变成「别人的句柄」。
    ///
    /// 同时钉住 [`input_site_child`] 的调用前提：`site` 是按构造挂在 `top` 之下的子孙
    /// （枚举就是从 `top` 往下走的），所以 `top == 0` 时它也只能是 0 ⇒ 不许凭空投出去。
    #[test]
    fn post_target_fallback_chain_is_exhaustive() {
        let (top, hit, site, own) = (0x1000usize, 0x2000usize, 0x3000usize, 4242u32);
        for (h, hp, hr, t, s) in [
            (hit, own, top, top, site), // 归属全过 → input-site
            (hit, own, top, top, 0),    // 没有 input-site → 顶层
            (hit, own, 0, top, site), // 祖先问不到 → Drop（顶层，且不发）
            (hit, own + 1, top, top, site), // 别人的窗口 → Drop
            (hit, own, top, 0, site), // 没有顶层 → Drop，句柄 0
            (0, own, top, top, site), // WindowFromPoint 答不上来 → Drop
            (hit, own, top, top, top), // 病态：site 与 top 同值 → 就是顶层，不炸
            (0, 0, 0, 0, 0),          // 全零：不许 panic
        ] {
            let got = pick_post_target(h, hp, own, hr, t, s);
            let dropped = pick_route(h, hp, own, hr, t) == Route::Drop;
            assert!(got == site || got == t, "{got:#x} 只能是 input-site 或顶层，不许是别的句柄");
            if dropped {
                assert_eq!(got, t, "Drop 时目标恒为顶层（且 post_wheel_inner 那儿根本不发）");
            } else {
                assert_eq!(got, if s == 0 { t } else { s }, "非 Drop 时按 input-site → 顶层回落");
            }
        }
    }

    /// `Route::Drop` 那半截安全条件**不许弱化**：点不在自家窗口上时，直投目标只能是顶层，
    /// 而且**一颗子窗口都不许被采纳** —— 注入会滚到别人窗口里，比不滚更糟。
    #[test]
    fn post_target_never_adopts_a_child_when_the_ownership_check_fails() {
        let (top, hit, site, own) = (0x1000usize, 0x2000usize, 0x3000usize, 4242u32);
        assert_eq!(pick_post_target(hit, own + 1, own, top, top, site), top, "别的进程 → 绝不采纳 input-site");
        assert_eq!(pick_post_target(hit, own, own, 0x9000, top, site), top, "祖先另有其人 → 同上");
        assert_eq!(pick_post_target(hit, own, own, 0, top, site), top, "问不到祖先 → 同上");
        assert_eq!(pick_post_target(0, own, own, top, top, site), top, "hit 为 0 → 不许猜子窗口");
        // 兜底句柄在 Drop 时回落顶层（宁可投一个「大概对」的），但发送侧已经一格都不发了。
        assert_eq!(pick_post_target(0, 0, 0, 0, 0, 0), 0, "病态输入不许 panic");
        assert_eq!(pick_post_target(0, 0, 0, 0, top, top), top);
    }

    /// 活体那半截的离线守卫：没有顶层句柄就**不去枚举窗口树**，也就不可能投出野句柄。
    ///
    /// [`input_site_child`] 真调 `EnumChildWindows`（一处 `extern "system"` 回调，见 [`input_site_proc`]），
    /// 单测线程上没有 WinUI 窗口 ⇒ 只能验到「`top == 0` 直接回 0」这一格 + 整条拒发通路。
    #[test]
    fn input_site_lookup_stays_empty_without_a_top_window() {
        forget_anchor();
        assert_eq!(input_site_child(0), 0, "top == 0 时连 FFI 都不该问");
        assert!(hwnd().is_none(), "单测线程上不该有自家顶层");
        // 自家没有窗口时，任何一条通路都在 Win32 之前就回绝，且诊断三件套全空。
        let posted = post_wheel(3, Some(Point { x: 10, y: 10 }));
        assert_eq!(posted, Posted::NoWindow);
        assert_eq!(posted.target_class(), CLASS_NONE);
        assert_eq!(posted.target_hwnd(), 0, "没投出去就不许有句柄");
        assert_eq!(posted.route(), ROUTE_NONE);
    }


    /// 四类已知窗口各归各位，且 [`KNOWN_CLASSES`] 里每一项都能自证（表和判定同源）。
    #[test]
    fn class_label_recognises_the_winui_window_tree() {
        assert_eq!(class_label(&utf16(WINDOW_CLASS)), WINDOW_CLASS, "顶层 → 顶层");
        assert_eq!(class_label(&utf16(INPUT_SITE_CLASS)), INPUT_SITE_CLASS);
        assert_eq!(class_label(&utf16(BRIDGE_CLASS)), BRIDGE_CLASS);
        assert_eq!(class_label(&utf16(NONCLIENT_CLASS)), NONCLIENT_CLASS);
        for known in KNOWN_CLASSES {
            assert_eq!(class_label(&utf16(known)), known, "{known} 必须映射到自己");
        }
        assert_eq!(class_label(&[]), CLASS_NONE, "读不到类名 = 没信息，不是 other");
    }

    /// input-site 那一档**长短两名都认**（本机实测短名，别处是长名），且认不出的一律 `other`。
    #[test]
    fn class_label_accepts_both_input_site_spellings() {
        // 探针在本机量到的短名（`tmp/keys-probe-report.txt`）。
        assert_eq!(class_label(&utf16("InputSiteWindowClass")), INPUT_SITE_CLASS);
        // WinUI 自述/别的运行时版本里的长名：靠 `InputSite` 那截子串认下来。
        assert_eq!(class_label(&utf16("Microsoft.UI.Input.InputSite.WindowClass")), INPUT_SITE_CLASS);
        // 不是 input-site 的窗口不许被子串误伤。
        assert_eq!(class_label(&utf16("Microsoft.UI.Input.SomethingElse.WindowClass")), CLASS_OTHER);
        for foreign in ["SysShadow", "WorkerW", "Windows.UI.Composition.DesktopWindowContentBridge"] {
            assert_eq!(class_label(&utf16(foreign)), CLASS_OTHER, "{foreign} 不是已知那张脸");
        }
        // 顶层的名字被截断（`GetClassNameW` 缓冲不够会这样）只能退化成 other，绝不假命中顶层。
        assert_eq!(class_label(&utf16("WinUIDesktopWin32WindowC")), CLASS_OTHER);
        assert_eq!(class_label(&utf16("")), CLASS_NONE, "空 = 「没读到信息」，与 &[] 同档");
    }

    /// 任何一条不可信证据（没命中 / PID 不是自己 / 不挂在这个顶层下）⇒ **一格都不发**。
    ///
    /// PID 那两条尤其要紧：注入是按屏幕点走的，判错就是往别的进程（甚至是系统窗口）里塞输入。
    /// 目标句柄那一侧的对应断言在 [`post_target_never_adopts_a_child_when_the_ownership_check_fails`]。
    #[test]
    fn pick_route_drops_on_every_untrustworthy_hit() {
        let (top, hit, own) = (0x1000usize, 0x2000usize, 4242u32);
        assert_eq!(pick_route(0, own, own, top, top), Route::Drop, "WindowFromPoint 返回 NULL");
        assert_eq!(pick_route(hit, own + 1, own, top, top), Route::Drop, "别的进程 → 绝不注入");
        assert_eq!(pick_route(hit, own.wrapping_sub(1), own, top, top), Route::Drop, "PID 差一格也算别人");
        assert_eq!(pick_route(hit, 0, own, top, top), Route::Drop, "问不到属主 PID = 不可信");
        assert_eq!(pick_route(hit, own, 0, top, top), Route::Drop, "问不到自己 PID = 不可信");
        assert_eq!(pick_route(hit, own, own, 0x9000, top), Route::Drop, "同进程但祖先另有其人");
        assert_eq!(pick_route(hit, own, own, 0, top), Route::Drop, "问不到祖先 = 不可信");
        assert_eq!(pick_route(hit, own, own, top, 0), Route::Drop, "顶层句柄为 0 = 没有自家窗口");
        assert_eq!(pick_route(0, own, own, top, top), Route::Drop, "hit 为 0 时连顶层都不许猜");
    }

    /// 诊断三件套（类名 / 句柄 / route）在**每一条拒发路径**上都必须是空值。
    ///
    /// 这条同时是 `forget_target()` 那个「每次进入先擦槽」的守卫 —— 槽里留着上一次的话，
    /// `DIAG` 行就会撒谎，而 #64 的定性**完全依赖**这几列日志（`route=` 是判据本体）。
    #[test]
    fn target_accessors_are_empty_on_every_refused_path() {
        forget_anchor();
        let posted = post_wheel(3, Some(Point { x: 10, y: 10 }));
        assert_eq!(posted, Posted::NoWindow, "单测线程上没有窗口");
        assert_eq!(posted.target_class(), CLASS_NONE);
        assert_eq!(posted.target_hwnd(), 0);
        assert_eq!(posted.route(), ROUTE_NONE, "没发出去就不许有 route");
        note_anchor(AnchorSource::Chat, Point { x: 1, y: 2 });
        let refused = post_wheel_at_window_anchor(2);
        assert_eq!(refused, Posted::NoWindow);
        assert_eq!(refused.target_class(), CLASS_NONE, "锚点通路被拒时也不许有目标");
        assert_eq!(refused.target_hwnd(), 0);
        assert_eq!(refused.route(), ROUTE_NONE);
        // 贴底通路（burst + 调用方给的点）同一条守卫：拒发时也不许留下上一次的影子。
        assert_eq!(post_wheel_burst_at_window_point(90, Point { x: 1, y: 2 }).route(), ROUTE_NONE);
        forget_anchor();
        assert_eq!(post_wheel(0, Some(Point { x: 1, y: 1 })), Posted::NothingToSend);
        // 变体形状没动：main.rs 的 `Posted::Sent { notches, at }` 解构照旧编得过（这里同形核对）。
        let sent = Posted::Sent { notches: 1, at: Point { x: 2, y: 3 } };
        assert_eq!(sent, Posted::Sent { notches: 1, at: Point { x: 2, y: 3 } });
        assert_eq!(
            format!("{:?}", Posted::NotOurWindow),
            "NotOurWindow",
            "#64 新增的「被别的窗口挡住」那一档要能在日志里认出来"
        );
    }

    /// route 标签的取值面：三个 `const` 互不相同、都不是空串 ⇒ 日志里不会出现两个同名档。
    #[test]
    fn route_labels_are_a_closed_set() {
        let all = [ROUTE_NONE, ROUTE_INJECT, ROUTE_POST];
        for (i, a) in all.iter().enumerate() {
            assert!(!a.is_empty());
            for (j, b) in all.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "{a} 与 {b} 撞名");
                }
            }
        }
        assert_eq!(ROUTE_INJECT, "sendinput");
        assert_eq!(ROUTE_POST, "postmessage");
        assert_eq!(ROUTE_NONE, "none");
    }

    // ---- #89 的输入源：用户滚轮观测（分叉版 `ViewChanged`）--------------------------------

    /// 钩子那一发的增量 → 本模块符号：**向尾端为正**，与 `wheel_delta` 恰好反号。
    #[test]
    fn observed_wheel_uses_the_module_sign_convention() {
        // 真滚轮向下（向尾端）：增量 -120 → 记 +1 格。
        assert_eq!(user_wheel_notches((-WHEEL_DELTA) as u32), Some(1));
        assert_eq!(user_wheel_notches((3 * WHEEL_DELTA) as u32), Some(-3), "向上翻 3 格");
        assert_eq!(user_wheel_notches((-3 * WHEEL_DELTA) as u32), Some(3));
        // 高半字（历史遗留的转数）不许混进低半字的增量里。
        let high_only = 0x0003_0000u32;
        assert_eq!(user_wheel_notches(high_only), None, "只填了高半字 = 竖向增量是 0");
        let mixed = (0x0002_0000u32 | ((-WHEEL_DELTA) as u16) as u32) as u32;
        assert_eq!(user_wheel_notches(mixed), Some(1), "低半字才是增量");
        // 不到一格（触控板）不记账：攒够一格再算，否则账本会漏。
        assert_eq!(user_wheel_notches((-60i32) as u32), None);
        assert_eq!(user_wheel_notches(0), None);
        // 一整手势（10 格）也在同一口径里。
        assert_eq!(user_wheel_notches((-1200i32) as u32), Some(10));
    }

    /// 「自家注入」必须**两个条件同时成立**才算：只认 `LLMHF_INJECTED` 会把测试驱动的模拟滚轮
    /// 一起当噪音丢掉 ⇒ #89 的「用户上翻就停跟随」在真机上永远测不到（假实现）。
    #[test]
    fn only_our_own_injected_wheel_is_ignored() {
        let down = (-WHEEL_DELTA) as u32;
        let up = (WHEEL_DELTA) as u32;
        // 硬件：flags=0 ⇒ 记。
        assert_eq!(observed_wheel(0, down, 0), Some(1));
        assert_eq!(observed_wheel(0, up, 0), Some(-1));
        // 自家注入（flags 带 INJECTED + 手签）⇒ 丢。
        assert_eq!(observed_wheel(LLMHF_INJECTED, down, INJECT_TAG), None);
        assert_eq!(is_own_wheel(LLMHF_INJECTED, INJECT_TAG), true);
        // 别的注入者（手签不是我们的，包括 0）⇒ 当成用户滚轮，照记。
        assert_eq!(observed_wheel(LLMHF_INJECTED, down, 0), Some(1));
        assert_eq!(observed_wheel(LLMHF_INJECTED, up, 0x1234), Some(-1));
        assert_eq!(is_own_wheel(LLMHF_INJECTED, 0), false);
        // 手签对上了但不是注入的（不可能发生，也要有确定行为）：不算自家。
        assert_eq!(is_own_wheel(0, INJECT_TAG), false);
        assert_eq!(observed_wheel(0, down, INJECT_TAG), Some(1));
    }

    /// 同一拍里连滚两下：格数相加（饱和）、落点取最新那颗。
    #[test]
    fn user_wheels_in_one_tick_merge_onto_the_newest_point() {
        let a = Point { x: 100, y: 200 };
        let b = Point { x: 104, y: 260 };
        assert_eq!(merge_user_wheel(None, 3, a), (3, a));
        assert_eq!(merge_user_wheel(Some((3, a)), -1, b), (2, b), "落点必须是最新那颗");
        assert_eq!(merge_user_wheel(Some((3, a)), 0, b), (3, b));
        assert_eq!(
            merge_user_wheel(Some((i32::MAX, a)), 1, b).0,
            i32::MAX,
            "饱和而不是环绕：绕成负数就是「用户往上翻」变成「用户往下滚」"
        );
        assert_eq!(merge_user_wheel(Some((i32::MIN, a)), -1, b).0, i32::MIN);
    }

    /// 钩子的安装状态：没装时读得到「没装」，重复卸载不炸（装钩子本身要活体线程消息泵，离线不测）。
    #[test]
    fn wheel_watch_state_is_offline_conservative() {
        uninstall_wheel_watch();
        assert!(!wheel_watch_installed(), "单测线程上不该挂着钩子");
        assert!(take_user_wheel().is_none(), "没滚过就是 None，不许是 (0, 0)");
        assert!(uninstall_wheel_watch(), "重复卸载按成功处理（幂等）");
    }

    /// `utf16_eq` / `utf16_contains`：零分配比较的取值域，含「needle 长过栈缓冲」这条路。
    #[test]
    fn utf16_comparators_are_exact_and_never_overmatch() {
        let name = utf16(INPUT_SITE_CLASS);
        assert!(utf16_eq(&name, INPUT_SITE_CLASS));
        assert!(!utf16_eq(&name, "InputSiteWindowClassX"));
        assert!(!utf16_eq(&name, "InputSiteWindowClas"));
        assert!(utf16_eq(&[], ""));
        assert!(utf16_contains(&name, "InputSite"));
        assert!(utf16_contains(&name, "WindowClass"), "子串在尾部也要命中");
        assert!(utf16_contains(&name, ""), "空 needle 恒真（调用方不该喂，但别崩）");
        assert!(!utf16_contains(&name, "Bridge"));
        assert!(!utf16_contains(&utf16("InputSit"), "InputSite"), "haystack 比 needle 短 → 不命中");
        // 长过 64 格的 needle 判不出来：明确回 false，绝不「大概像就算」。
        let huge = "x".repeat(80);
        assert!(!utf16_contains(&utf16(&format!("{huge}tail")), &huge));
        assert!(utf16_contains(&utf16(&huge), &huge[..60]));
        // BMP 之外的码点（代理对）：逐码点比较，不匹配就是不匹配。
        assert!(!utf16_eq(&utf16("\u{1F600}"), INPUT_SITE_CLASS));
    }

    // ---- #89：贴底状态机的三条规则，**各自一颗**测试 ----------------------------------------
    //
    // 主干的对应物是 `MainWindow.xaml.cs:5721-5775` 那三句：
    //   `if (!force && !IsTranscriptAtBottom()) return;`      —— 规则 1 / 4
    //   `sv.ScrollableHeight - sv.VerticalOffset <= 96`       —— 判据本体
    //   `PostUi(() => sv.ChangeView(null, sv.ScrollableHeight, …))` —— 规则 1 的「下一帧绝对定位」
    // 分叉没有绝对定位 ⇒ 规则 1 落在「按离底距离算格数 + 过滚」上（[`burst_for_gap`]）。

    /// 正文那一档的几何（与 `main.rs` 的 `CHAT_BAND` 同源：一行 64 DIP、一格 48 DIP）。
    fn chat_band() -> Band {
        Band { row_dip: 64.0, page_dip: 640.0, dip_per_notch: 48.0 }
    }

    /// **规则 1（什么时候跟随）**：离底在 96 DIP 以内就跟随，且补的格数按当前离底距离算、
    /// 带过滚余量；`force` 时无视账本硬拉（主干那五个 `ScrollTranscriptToBottom(force: true)` 调用点）。
    #[test]
    fn stick_follows_while_the_transcript_is_at_the_bottom() {
        let band = chat_band();
        // 绝对底部：仍然要发（逐字增高时底部在往下长，不发就是「最后一条看不见」）。
        match stick_action(Stick::bottom(), false, band) {
            StickAction::Roll { notches, gap_dip } => {
                assert_eq!(gap_dip, 0.0);
                assert_eq!(notches, 1 + OVER_ROLL_NOTCHES, "零间隙时也是「至少一格 + 过滚余量」");
                assert!(notches > 0);
            }
            other => panic!("贴底时必须跟随， got {other:?}"),
        }
        // 阈值**两端**：96 正好算贴底（主干是 `<=`），97 就不跟了。
        assert!(Stick::new(96.0).is_at_bottom());
        assert!(!Stick::new(96.5).is_at_bottom());
        assert_eq!(STICK_TO_BOTTOM_DIP, 96.0, "主干那个 96 不许被改动");
        match stick_action(Stick::new(96.0), false, band) {
            StickAction::Roll { notches, .. } => {
                // 96 / 48 = 2 格，再 + 过滚 2 格 = 4。
                assert_eq!(notches, 4, "{notches}");
            }
            other => panic!("96 DIP 以内必须跟随，got {other:?}"),
        }
        // 100 格的历史：一次性补到位（过滚 + 夹到 MAX_BURST_NOTCHES）。
        let far = stick_action(Stick::new(6100.0), true, band);
        match far {
            StickAction::Roll { notches, .. } => {
                assert!(notches >= 128, "6100 DIP / 48 要 128 格，got {notches}");
                assert!(notches <= MAX_BURST_NOTCHES);
            }
            other => panic!("force 必须滚，got {other:?}"),
        }
        // 跟随之后把账本抹平：`snapped()` 就是「过滚已被 ScrollViewer 夹到边界」这件事。
        assert_eq!(Stick::new(96.0).snapped(), Stick::bottom());
    }

    /// **规则 2（什么时候停止跟随）**：用户往头端滚 → `gap_dip` 抬过 96 → `Hold{user-up}`。
    ///
    /// 主干原话（`MainWindow.xaml.cs:5726`）：「上翻读旧消息不该被逐字增量拽回底部」。
    /// 这里刻意**不写闩锁**：停止跟随只是「判据这一次不成立」，所以下一条规则才可能白送。
    #[test]
    fn stick_stops_following_after_the_user_rolls_up() {
        let band = chat_band();
        let mut stick = Stick::bottom();
        // 上翻 3 格 = 144 DIP > 96 ⇒ 从此不跟随。
        stick = stick.user_roll(-3, band);
        assert_eq!(stick.gap_dip(), 144.0);
        assert!(!stick.is_at_bottom());
        assert_eq!(
            stick_action(stick, false, band),
            StickAction::Hold { reason: HOLD_USER_UP, gap_dip: 144.0 },
            "逐字增量不许把用户拽回底部"
        );
        // 翻得越远，gap 单调涨（滚轮没有「到底」以外的下界）。
        let deeper = stick.user_roll(-10, band);
        assert!(deeper.gap_dip() > stick.gap_dip());
        // 内容在**下方**继续长 ⇒ gap 变大，但绝不许把「已经不跟了」变成「更跟」或反过来。
        assert_eq!(deeper.content_grew(64.0).gap_dip(), deeper.gap_dip() + 64.0);
        // 上翻不足阈值的边角：2 格 = 96 DIP，**仍算贴底**（主干的 `<=` 就是这个意思）。
        let shallow = Stick::bottom().user_roll(-2, band);
        assert!(shallow.is_at_bottom(), "96 DIP 恰好贴边，仍跟");
        assert!(matches!(stick_action(shallow, false, band), StickAction::Roll { .. }));
        // 边界只夹「往尾端过头」那一侧：已经在绝对底部还往下滚，ScrollViewer 夹住偏移 ⇒ gap 仍 0；
        // 而**往上**翻永远有地方去（离底距离照实涨），不许被夹成 0 而假装用户没上翻。
        assert_eq!(Stick::bottom().user_roll(50, band), Stick::bottom(), "底部之外没有更多尾端内容");
        assert_eq!(Stick::bottom().user_roll(-50, band), Stick::new(2400.0), "上翻 50 格 = 2400 DIP");
    }

    /// **规则 3（什么时候恢复）**：用户滚回 96 以内 ⇒ 同一句判据再次成立，**不需要任何额外通路**。
    ///
    /// 这条测试存在的意义是「证明我们没有偷偷加闩锁」：如果实现里存在 `follow_disabled: bool`
    /// 之类的一次性标记，那么规则 2 之后规则 3 必然失败。
    #[test]
    fn stick_resumes_without_any_extra_path_when_the_user_rolls_back_down() {
        let band = chat_band();
        let away = Stick::bottom().user_roll(-6, band); // 288 DIP
        assert!(!away.is_at_bottom());
        let back = away.user_roll(5, band); // 288 - 240 = 48 <= 96 ⇒ 恢复
        assert!(back.is_at_bottom(), "gap={}", back.gap_dip());
        assert!(
            matches!(stick_action(back, false, band), StickAction::Roll { .. }),
            "滚回来就必须重新跟随，不许残留「用户不想跟」的标记"
        );
        // 一路滚回绝对底部也要成立。
        assert_eq!(back.user_roll(1, band).gap_dip(), 0.0);
        // 反方向同理：从远处往尾端滚够 6 格就恢复，不多不少。
        let far = Stick::new(480.0);
        assert!(!far.user_roll(7, band).is_at_bottom(), "336 DIP 还差得远");
        assert!(far.user_roll(8, band).is_at_bottom(), "96 DIP：恰好回阈值内");
        // 恢复之后再上翻 ⇒ 再停（判据是无状态的，来回多少次都一样）。
        let cycled = far.user_roll(8, band).user_roll(-8, band);
        assert!(!cycled.is_at_bottom());
    }

    /// `force` 抄主干那五个调用点：它**盖过**「用户上翻了」这件事，但不盖过「几何不可信」。
    #[test]
    fn force_overrides_the_ledger_but_not_broken_geometry() {
        let band = chat_band();
        let away = Stick::new(4800.0);
        assert!(matches!(stick_action(away, false, band), StickAction::Hold { reason: HOLD_USER_UP, .. }));
        match stick_action(away, true, band) {
            StickAction::Roll { notches, gap_dip } => {
                assert_eq!(gap_dip, 4800.0, "日志要能自证 force 当时看到的距离");
                assert_eq!(notches, 102, "4800/48 = 100 格 + 过滚 2 格");
            }
            other => panic!("force 必须滚，got {other:?}"),
        }
        // 每格 DIP 不知道 ⇒ 连 force 也不许发（主干同形：`ScrollableHeight > 0` 才 ChangeView）。
        for bad in [0.0, -48.0, f64::NAN, f64::INFINITY] {
            let broken = Band { row_dip: 64.0, page_dip: 640.0, dip_per_notch: bad };
            for force in [false, true] {
                assert_eq!(
                    stick_action(Stick::bottom(), force, broken),
                    StickAction::Hold { reason: HOLD_NO_GEOMETRY, gap_dip: 0.0 },
                    "force={force} dip_per_notch={bad}"
                );
            }
        }
        // `burst_for_gap` 的取值面：0 距离也至少 1 格（主干 ChangeView 是「绝对到底」，不是「不动」）。
        assert_eq!(burst_for_gap(0.0, band), Some(1 + OVER_ROLL_NOTCHES));
        assert_eq!(burst_for_gap(f64::NAN, band), Some(1 + OVER_ROLL_NOTCHES), "坏距离当 0 处理");
        assert_eq!(burst_for_gap(-100.0, band), Some(1 + OVER_ROLL_NOTCHES), "负距离当 0 处理");
        assert_eq!(burst_for_gap(f64::NEG_INFINITY, band), Some(1 + OVER_ROLL_NOTCHES));
        assert_eq!(burst_for_gap(1.0, band), Some(1 + OVER_ROLL_NOTCHES), "1 DIP 也要一整格");
        assert_eq!(burst_for_gap(f64::INFINITY, band), Some(MAX_BURST_NOTCHES), "夹在预算内");
        assert_eq!(burst_for_gap(1e12, band), Some(MAX_BURST_NOTCHES));
        // 一格 1 DIP 的病态几何：0 距离仍是「余量 + 至少 1 格」的下界。
        assert_eq!(
            burst_for_gap(0.0, Band { row_dip: 64.0, page_dip: 640.0, dip_per_notch: 1.0 }),
            Some(1 + OVER_ROLL_NOTCHES)
        );
    }

    /// 账本的算术卫生：非有限值、负值、零增量都不许把状态弄成「永远不跟随」或「永远跟随」。
    #[test]
    fn stick_ledger_survives_broken_arithmetic() {
        for bad in [f64::NAN, f64::NEG_INFINITY, -1.0, -96.0] {
            // 坏输入 → 当「贴底」处理（与主干拿不到滚动面时 `return true` 同一条回落）。
            assert_eq!(Stick::new(bad), Stick::bottom(), "{bad}");
            assert!(Stick::new(bad).is_at_bottom());
        }
        assert!(Stick::new(f64::INFINITY).gap_dip() > 0.0, "+inf 是「离得很远」，不是坏值");
        assert!(!Stick::new(f64::INFINITY).is_at_bottom());
        let band = chat_band();
        // 0 格 / 坏几何：账本原样返回（不许悄悄当成「滚过了」）。
        assert_eq!(Stick::new(200.0).user_roll(0, band), Stick::new(200.0));
        let broken = Band { row_dip: 64.0, page_dip: 640.0, dip_per_notch: 0.0 };
        assert_eq!(Stick::new(200.0).user_roll(-5, broken), Stick::new(200.0));
        // 增量为 0 / 负 / NaN 的内容高度都不改变账本。
        for grew in [0.0, -64.0, f64::NAN] {
            assert_eq!(Stick::new(200.0).content_grew(grew), Stick::new(200.0), "grew={grew}");
        }
        // 默认值 = 贴底（新会话第一屏不许因为「没账本」而不跟随）。
        assert_eq!(Stick::default(), Stick::bottom());
    }

    /// 规则 1 的时序细节：贴底时**同一拍的内容增高**不能把跟随判成「不跟」（主干的判据跑在
    /// 布局之前，所以「本轮长高」defeat 不了「本轮跟随」）。
    #[test]
    fn growth_while_at_bottom_is_absorbed_before_the_decision() {
        let band = chat_band();
        // 逐字输出：每次长 64 DIP，贴底的那本账始终被跟随抹平。
        let mut stick = Stick::bottom();
        for _ in 0..40 {
            stick = stick.content_grew(64.0);
            match stick_action(stick, false, band) {
                StickAction::Roll { .. } => stick = stick.snapped(),
                other => panic!("逐字输出中途不许停止跟随，got {other:?}"),
            }
        }
        assert_eq!(stick, Stick::bottom());
        // 同一串增量，但用户中途上翻 ⇒ 从此 gap 累计，且不再跟随（两条规则不打架）。
        let mut away = Stick::bottom().user_roll(-4, band); // 192 DIP
        for _ in 0..5 {
            away = away.content_grew(64.0);
            assert!(matches!(stick_action(away, false, band), StickAction::Hold { reason: HOLD_USER_UP, .. }));
        }
        assert_eq!(away.gap_dip(), 192.0 + 5.0 * 64.0, "不贴底时增量必须算进离底距离");
    }

    // ---- #89 落点几何（ChatBox）与逆 DPI 换算 ------------------------------------------

    /// 本机默认窗口那一档的客户区 DIP（2560x1658 物理 @200% ⇒ 1280x829），与
    /// `tmp/qa6-wheel-wheeldown-end.txt` 的 `client=109,96 size=2560x1658 dpi=192` 同源。
    fn default_box() -> ChatBox {
        ChatBox { left: 276.0, top: 52.0, right: 1280.0 - 12.0, bottom: 829.0 - 100.0 }
    }

    #[test]
    fn px_to_dip_inverts_dip_to_px_at_every_shipped_scale() {
        // 逐档往返：100/125/150/175/200/225/300%。取整误差最多 1 DIP 的零头，不许出现整档偏。
        for dpi in [96u32, 120, 144, 168, 192, 216, 288] {
            for dip in [0.0, 1.0, 12.0, 52.0, 276.0, 640.0, 1280.0, 4095.0] {
                let back = px_to_dip(dip_to_px(dip, dpi), dpi);
                assert!((back - dip).abs() <= 1.0, "dpi={dpi} dip={dip} 往返成 {back}");
            }
        }
        // `dpi == 0` = 问不到 ⇒ 与 `dip_to_px` 同一条回落（按 100% 处理），不许除零/出 NaN。
        assert_eq!(px_to_dip(500, 0), 500.0);
        assert_eq!(px_to_dip(-64, 96), -64.0);
        // 200% 是这台机器的主力档，钉死两个具体数当独立交叉核对。
        assert_eq!(px_to_dip(2560, 192), 1280.0);
        assert_eq!(px_to_dip(1658, 192), 829.0);
        // 负宽度（病态 Win32 返回值）不许变成正数。
        assert!(px_to_dip(-100, 192) < 0.0);
    }

    #[test]
    fn chat_box_usability_matches_the_mainline_extent_guard() {
        let band = default_box();
        assert!(band.is_usable());
        // 退化形状一律不可用：主干 `ScrollableHeight > 0` 那一刀的同形物。
        for bad in [
            ChatBox { right: 276.0, ..band },                        // 宽 0
            ChatBox { right: 200.0, ..band },                        // 宽负
            ChatBox { bottom: 52.0, ..band },                        // 高 0
            ChatBox { top: 900.0, ..band },                          // 高负（窗口极矮）
            ChatBox { left: f64::NAN, ..band },
            ChatBox { right: f64::INFINITY, ..band },
        ] {
            assert!(!bad.is_usable(), "{bad:?} 必须判不可用");
            assert_eq!(bad.wheel_anchor(1220.0), None, "不可用的带子不许给出落点");
        }
    }

    #[test]
    fn chat_wheel_anchor_stays_out_of_the_turn_rail_and_inside_the_band() {
        let band = default_box();
        let point = band.wheel_anchor(1268.0 - 40.0).expect("默认窗口必须有落点");
        // 落点必须在带内（这是「滚的是正文」这件事的全部依据）。
        assert!(band.contains_dip(f64::from(point.x), f64::from(point.y)), "落点掉出带子：{point:?}");
        // 且必须在轨左缘的**左边**至少一格安全内缩 —— 落进轨的命中区就会去滚轨自己。
        assert!(f64::from(point.x) <= 1228.0 - WHEEL_EDGE_INSET_DIP, "落点压进轮次轨：{point:?}");
        // 纵向取上半段：躲开「贴底时最后一条气泡正好压在下沿」。
        assert!(f64::from(point.y) > band.top && f64::from(point.y) < (band.top + band.bottom) / 2.0);
        // 轨不存在（拿不到横坐标 ⇒ NaN）时退回硬边界，仍然可用、仍然在带内。
        let no_rail = band.wheel_anchor(f64::NAN).expect("轨未知也要能给出落点");
        assert!(band.contains_dip(f64::from(no_rail.x), f64::from(no_rail.y)));
        assert!(no_rail.x >= point.x, "轨未知时可用段只会更宽");
        // 轨整段盖住带子（窗口极窄 / 轨比带子还宽）⇒ 宁可不发，也不去滚轨。
        assert_eq!(band.wheel_anchor(band.left + 4.0), None, "没地方放落点就必须 None");
        // 负无穷那种病态横坐标与 NaN 同归宿：退回硬边界，而不是「轨在窗口最左边」那种误读。
        assert_eq!(band.wheel_anchor(f64::NEG_INFINITY), band.wheel_anchor(f64::NAN));
        // 窗口拉宽：落点跟着往右走，但恒在带内、恒不碰轨。
        for right in [468.0, 768.0, 2268.0, 5268.0] {
            let wide = ChatBox { right, ..band };
            let hit = wide.wheel_anchor(right - 40.0).expect("拉宽了必有落点");
            assert!(wide.contains_dip(f64::from(hit.x), f64::from(hit.y)), "right={right} → {hit:?}");
            assert!(f64::from(hit.x) <= right - 40.0 - WHEEL_EDGE_INSET_DIP);
        }
    }

    #[test]
    fn chat_box_contains_is_the_user_wheel_membership_test() {
        let band = default_box();
        // 带内四边含边界。
        assert!(band.contains_dip(276.0, 52.0));
        assert!(band.contains_dip(1268.0, 729.0));
        // 左栏 / 顶条 / composer / 带子右界之外：一律不算「用户滚了正文」。
        assert!(!band.contains_dip(130.0, 400.0), "左侧栏");
        assert!(!band.contains_dip(700.0, 20.0), "顶条");
        assert!(!band.contains_dip(700.0, 800.0), "composer 那一档");
        assert!(!band.contains_dip(1275.0, 400.0), "带子右界之外");
        // 病态输入不许假命中。
        assert!(!band.contains_dip(f64::NAN, 400.0));
        assert!(!band.contains_dip(700.0, f64::INFINITY));
        assert!(!ChatBox { right: 5.0, ..band }.contains_dip(7.0, 7.0), "不可用的带子里没有点");
    }

}
