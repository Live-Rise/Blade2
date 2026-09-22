//! 键盘钩子：把主干那三件事（Enter 发送 / Shift+Enter 换行 / Esc 返回上一级）搬进分叉。
//!
//! 为什么只能走钩子：windows-reactor 0.100.0 **一个键盘事件回调都没有**（`generated.rs` 里
//! `on_preview_key_down` / `on_key_down` 各 0 命中），也没有焦点查询与焦点变更事件
//! （`reference.rs` 只有 `request_focus` / `request_focus_result`，全 crate 搜不到 `FocusedElement`
//! / `has_focus`，`native` 模块私有 ⇒ 拿不到 `FocusManager`）。主干靠 `PreviewKeyDown` 挂在输入框
//! 上天然知道「键是谁吃的」，分叉不知道 ⇒ 焦点归属只能自己在模型里推（见 [`KeyOwner`] 与
//! `main.rs` 的 `Shell::key_owner`）。
//!
//! 实测口径（证据 `tmp/keys-probe-report.txt`，探针已挪出构建、存在 `tmp/probe-keys.rs.bak`）：
//! · `HOOKS installed thread=true ll=true module=true` —— 钩子装得上；
//! · `KEY src=thread vk=0x58 up=false was_down=0 lparam=0x1` —— `WH_KEYBOARD` **线程**钩子能收到
//!   XAML `TextBox` 里的真实按键；
//! · `SWALLOW src=thread vk=0x5A('Z') -> returning 1` 之后 UIA 读回 `TEXT-from-xaml="x"` ——
//!   proc 里 `return 1` 确实吞键（= 主干的 `e.Handled = true`），**放行**就是 `CallNextHookEx`；
//! · 钩子 proc 在 UI 线程上被调用 ⇒ 可以直接用 `ComponentContext::sender()`（`Rc<RefCell<…>>`、
//!   非 `Send`、只属于 UI 线程），不需要跨线程通道。
//!
//! 零依赖：所有 Win32 入口都用 `extern "system"` + `#[link(name = "user32")]` 自己声明，
//! **不新增 Cargo 依赖**（`--offline` 构建必须继续可用，`Cargo.lock` 里没有 `windows` 主 crate）。
//!
//! 不许 panic：钩子 proc 里任何路径抛异常都会穿过 `extern "system"` 边界 ⇒ 进程直接没了、
//! stdout/stderr 0 字节（主干在 `OnInputPreviewKeyDown` 外那层 `try/catch`，注释写着
//! 「浮层按键处理异常不上抛（0xc000027b 教训）」，同一个教训）。这里三道保险：
//! proc 体整个套 [`std::panic::catch_unwind`]；决策本身是无分配纯函数、状态只读 [`Cell`]；
//! 回投消息严守「不持借用跨过 send」（见 [`dispatch`]）。任何一道出问题都**放行**这一发键。

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;

// ---------------------------------------------------------------- Win32 入口

#[link(name = "user32")]
unsafe extern "system" {
    fn SetWindowsHookExW(id_hook: i32, lp_fn: *const c_void, h_mod: *const c_void, thread: u32) -> *mut c_void;
    fn CallNextHookEx(hhk: *mut c_void, code: i32, w_param: usize, l_param: *const c_void) -> usize;
    fn UnhookWindowsHookEx(hhk: *mut c_void) -> i32;
    fn GetCurrentThreadId() -> u32;
    fn GetKeyState(n_virt_key: i32) -> i16;
}

/// 线程级键盘钩子：`WM_KEYDOWN` / `WM_KEYUP` 进**本线程**队列时回调，`h_mod` 传 null、`thread`
/// 传本线程 id。系统级 `WH_KEYBOARD_LL` 会连别的程序的按键一起吃掉，不用。
const WH_KEYBOARD: i32 = 2;

/// 虚拟键码：主干 `TryHandleEnterSend` 只认 Enter，`OnRootPreviewKeyDown` 只认 Escape，
/// `HandlePaletteKey`（#52 的接缝）再加 ↑/↓/Tab 三档。
pub const VK_RETURN: u32 = 0x0D;
pub const VK_ESCAPE: u32 = 0x1B;
pub const VK_TAB: u32 = 0x09;
pub const VK_UP: u32 = 0x26;
pub const VK_DOWN: u32 = 0x28;
/// `GetKeyState` 的修饰键索引；返回值高位（`0x8000`，即 `i16` 为负）= 按下。
pub const VK_SHIFT: i32 = 0x10;
pub const VK_CONTROL: i32 = 0x11;

/// 「最近一次谁在吃键盘」——分叉自己推出来的焦点替身。
///
/// 只有 `Composer` 才把 Enter 判成发送；`Other`（会话搜索框 / 设置项文本框 / 反馈说明框，
/// 以及一切说不清的场合）时 Enter 一律放行 —— 主干在那些框里 Enter 本来也不做事。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyOwner {
    /// composer 那颗 `AcceptsReturn` 多行输入框（`main.rs` 的 `InputBox`）。
    Composer,
    /// 别的字段，或「说不清」。初值与回落值都走这支：宁可放行一个 Enter，不可误发一条消息。
    Other,
}

/// 一次按键换出来的动作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    /// 吞键 + 提交（= 主干 `e.Handled = true; _ = SubmitInputAsync()`）。
    Submit,
    /// 吞键 + 提交，但走主干那个 `forceMode: AlternateBusyEnter()` 反向档
    /// （= 主干 `e.Handled = true; _ = SubmitInputAsync(forceMode: busy ? 反向 : null)`）。
    /// 这里只说「这是 Ctrl 那一发」，到底取哪个 mode 由模型判（`Shell::ctrl_enter_mode`）。
    SubmitAlternate,
    /// 吞键 + 返回上一级（= 主干 `e.Handled = true; CloseSettingsSubPage()`）。
    CloseSub,
    /// 吞键 + 回聊天页（= 主干 `OnRootPreviewKeyDown` 里 `SettingsPage` 可见、`InSettingsSubPage`
    /// 为假时那一发 `e.Handled = true; ShowChatPage()`）。#52 之外的既有语义，跟焦点无关。
    ReturnToChat,
    /// 吞键 + 关命令面板（= 主干 `HandlePaletteKey` 的 `Escape`：`HideCommandPalette()`）。
    /// 由 `main.rs` 的 `Msg::PaletteClose` 落；开关是 `Scene::palette_open`（见 [`sync_pages`]）。
    PaletteClose,
    /// 吞键 + 移动浮层选中项（= 主干 `MoveCommandSelection(±1)`，`Up` 传 `-1`、`Down` 传 `1`）。
    /// 这是全表里唯一允许长按自动重复逐发投递的动作 —— 主干按住 ↑ 就是连着一格一格挪。
    PaletteMove(i32),
    /// 吞键 + 采纳浮层选中项（= 主干 `AcceptCommandSelection()`，`Tab` 与不带 Shift 的 `Enter`）。
    /// **这一档吃掉的那一发就是「不发送」**：主干 `OnInputPreviewKeyDown`（MW:6824）把
    /// `HandlePaletteKey` 排在 `TryHandleEnterSend` 之前，浮层开着时 Enter 绝不落到发消息那条路。
    PaletteAccept,
    /// 放行（`CallNextHookEx`）：Shift+Enter 换行、别的框里的 Enter、没有二级页时的 Esc。
    Pass,
}

/// 判键要看的三份「场景」事实（模型推出来、经 [`sync_pages`] 贴进 thread-local）。
///
/// 默认值刻意等于**没有浮层、停在聊天页** —— 也就是 #52 之前主线的全部行为，接缝装上但不开火。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scene {
    /// 当前停在聊天页。主干 `OnRootPreviewKeyDown` 只在 `SettingsPage` 可见时把 Esc 判成
    /// 返回聊天页，所以这条决定 [`KeyAction::ReturnToChat`] 那一档开不开。
    pub on_chat: bool,
    /// 二级页（`self.sub`）开着 —— 与 [`sync`] 第二个参数同源。
    pub sub_open: bool,
    /// 命令面板开着（主干 `CommandPalette.Visibility == Visible`，含「目录取不到」那一态：
    /// 主干那时列表收起、浮层还在，Enter 照样被 `HandlePaletteKey` 吃掉）。
    /// 由 `main.rs: Shell::publish_keys` 用 `PaletteState::showing` 喂真值。
    pub palette_open: bool,
}

impl Default for Scene {
    fn default() -> Self {
        Scene { on_chat: true, sub_open: false, palette_open: false }
    }
}

/// **键 → 动作的纯映射**（整张表离线单测覆盖，不用起 GUI）。
///
/// 逐条对着主干 `MainWindow.xaml.cs:6819`（`OnInputPreviewKeyDown`）与 `:6847`
/// （`TryHandleEnterSend`）：
/// · Shift+Enter ⇒ `Pass`（放行给 `TextBox` 自己插换行，主干也是 `return false`）；
/// · 纯 Enter 且焦点在 composer ⇒ `Submit`（必须在控件的编辑逻辑之前吞，否则先插了换行）；
/// · Ctrl+Enter 且焦点在 composer ⇒ `SubmitAlternate`（主干那一发的 `forceMode` 就是
///   `IsSessionBusy() ? AlternateBusyEnter() : null`，queue↔steer 反向档）；
/// · Enter 且焦点在别的字段 ⇒ `Pass`（主干那条 `PreviewKeyDown` 只挂在 InputBox 上，压根收不到）；
/// · Esc 且二级页开着 ⇒ `CloseSub`，否则 `Pass`（分叉只在 `self.sub.is_some()` 时消费 Esc）。
///
/// 反向档**不在本模块判忙**：忙态是模型里的 `Shell::running`（`api-session/status` 驱动），
/// 而 `busyEnter` 设置是模型里那行 `ui-conversation_busyEnter` 下拉 ⇒ 两者都由
/// `main.rs: Shell::ctrl_enter_mode()` 读，钩子只说「这是 Ctrl 那一发」。这样这张表保持纯函数、
/// 不起 GUI 就能全测，也不为这条键新造任何状态。
///
/// 这条四参数入口是**旧口径的窄门面**（焦点 + 二级页），等价于
/// `classify(vk, shift, ctrl, owner, Scene { on_chat: true, sub_open, palette_open: false })`
/// —— 也就是「没有浮层、停在聊天页」时的那份子集。要拿到 #52 那几档（`ReturnToChat` /
/// `Palette*`）请直接用 [`classify`]。
pub fn action_for(vk: u32, shift: bool, ctrl: bool, owner: KeyOwner, sub_open: bool) -> KeyAction {
    classify(vk, shift, ctrl, owner, Scene { on_chat: true, sub_open, palette_open: false })
}

/// **完整那张表**（含 #52 的接缝档）：`action_for` 的窄口径 + 场景两份事实。
///
/// 逐条对着主干：
/// · `OnInputPreviewKeyDown`（`MainWindow.xaml.cs:6819`）先问浮层、再问 Enter 发送 ⇒
///   浮层那几档排在 `Submit` 之前；
/// · `TryHandleEnterSend`（`:6847`）：Shift ⇒ `return false`（换行放行），
///   否则 `e.Handled = true` + `SubmitInputAsync(forceMode: ctrl && busy ? 反向档 : null)`；
/// · `HandlePaletteKey`（`:17755`）：`Escape` 收浮层、`Up`/`Down` 移选中项（-1/+1）、
///   `Tab` 与不带 Shift 的 `Enter` 采纳；焦点始终留在输入框 ⇒ 这几档都要求 `KeyOwner::Composer`；
/// · `OnRootPreviewKeyDown`（`:9245`）：只认 `Escape`，浮层可见时**让路**（`return`，不抢），
///   `SettingsPage` 可见时「有二级页先收二级页，否则 `ShowChatPage()`」⇒ 本表里
///   `CloseSub` 排在 `ReturnToChat` 之前，且 `ReturnToChat` 只在 `on_chat == false` 时开火；
/// · 其余键（含浮层没开时的 `Tab`/`↑`/`↓`，主干在输入框里根本不处理）一律 `Pass`。
pub fn classify(vk: u32, shift: bool, ctrl: bool, owner: KeyOwner, scene: Scene) -> KeyAction {
    let in_composer = owner == KeyOwner::Composer;
    match vk {
        // Shift 那一发 Enter 永远放行：主干两处同口径（`TryHandleEnterSend` 开头 `return false`；
        // `HandlePaletteKey` 的 Enter 分支注释「Shift+Enter 仍是换行」）。
        VK_RETURN if shift => KeyAction::Pass,
        // 浮层优先（`OnInputPreviewKeyDown` 里那两条 `if Visible && Handle…Key(e)` 排在发送之前）。
        VK_RETURN | VK_TAB if scene.palette_open && in_composer => KeyAction::PaletteAccept,
        VK_UP if scene.palette_open && in_composer => KeyAction::PaletteMove(-1),
        VK_DOWN if scene.palette_open && in_composer => KeyAction::PaletteMove(1),
        // Esc 的让路顺序照根隧道：浮层开着时根上不抢（交给输入框那档收浮层），
        // 其次二级页，最后才是「非聊天页 ⇒ 回聊天页」。
        VK_ESCAPE if scene.palette_open && in_composer => KeyAction::PaletteClose,
        VK_ESCAPE if scene.sub_open => KeyAction::CloseSub,
        VK_ESCAPE if !scene.on_chat => KeyAction::ReturnToChat,
        // 纯 Enter / Ctrl+Enter 且焦点在 composer ⇒ 发送（Ctrl 那一发只标「反向档」，
        // 忙不忙、取哪个 mode 由 `Shell::ctrl_enter_mode()` 判）。
        VK_RETURN if in_composer && ctrl => KeyAction::SubmitAlternate,
        VK_RETURN if in_composer => KeyAction::Submit,
        _ => KeyAction::Pass,
    }
}

/// 主干 `GetKeyStateForCurrentThread(VirtualKey.Shift)` 的等价物。
pub fn shift_down() -> bool {
    key_flag(VK_SHIFT)
}

/// 主干 `GetKeyStateForCurrentThread(VirtualKey.Control)` 的等价物。
pub fn ctrl_down() -> bool {
    key_flag(VK_CONTROL)
}

fn key_flag(vk: i32) -> bool {
    // 单测里可以钉死（见 [`PinnedMods`]）：`GetKeyState` 读的是**真机器**的修饰键，
    // 有人正按着 Shift（或别的自测正 SendInput 一发 Shift+Enter）时，真值断言会假阴性。
    if let Some((shift, ctrl)) = pinned_mods() {
        return match vk {
            VK_SHIFT => shift,
            VK_CONTROL => ctrl,
            _ => false,
        };
    }
    unsafe { GetKeyState(vk) < 0 }
}

/// 非测试构建里恒为 `None` ⇒ 这一问被优化掉，`GetKeyState` 照旧。
#[cfg(not(test))]
#[inline]
fn pinned_mods() -> Option<(bool, bool)> {
    None
}

#[cfg(test)]
fn pinned_mods() -> Option<(bool, bool)> {
    PINNED.with(|slot| slot.get())
}

// ---------------------------------------------------------------- 线程内状态
//
// 钩子 proc 的签名字带不了用户数据 ⇒ 状态只能放 `thread_local!`。装钩子、回调、卸钩子都在同一条
// UI 线程上，所以这些槽不需要锁、也不需要 `Send`。

/// 可克隆的发送句柄：`main.rs` 拿 `context.sender()` 造一个 `Rc<dyn Fn(KeyAction)>` 交进来，
/// 闭包里负责「先 clone 句柄、丢掉借用、再 send」。
type Sink = Rc<dyn Fn(KeyAction)>;

thread_local! {
    static OWNER: Cell<KeyOwner> = const { Cell::new(KeyOwner::Other) };
    static SUB_OPEN: Cell<bool> = const { Cell::new(false) };
    static ON_CHAT: Cell<bool> = const { Cell::new(true) };
    static PALETTE_OPEN: Cell<bool> = const { Cell::new(false) };
    static HOOK: Cell<*mut c_void> = const { Cell::new(std::ptr::null_mut()) };
    static INSTALLER_TID: Cell<u32> = const { Cell::new(0) };
    static SINK: RefCell<Option<Sink>> = const { RefCell::new(None) };
}

// 单测专用：钉死 `GetKeyState` 的两问（生产构建里 `pinned_mods()` 恒 `None`，这一槽不会被创建）。
#[cfg(test)]
thread_local! {
    static PINNED: Cell<Option<(bool, bool)>> = const { Cell::new(None) };
}

/// 测试里把修饰键读值钉成 `(shift, ctrl)`，作用域结束自动恢复。
///
/// 为什么必须有：`decide` 走的是真 `GetKeyState`，跑单测的人（或同时在跑的另一个 GUI 自测）
/// 此刻正按着 Shift ⇒ `VK_RETURN if shift => Pass` 抢先命中 ⇒ 「Enter 被吞 + 投一条 Submit」
/// 那类断言假阴性。凡是从测试里调 [`decide`] 的断言都先钉一发。
#[cfg(test)]
pub(crate) struct PinnedMods;

#[cfg(test)]
impl PinnedMods {
    pub(crate) fn set(shift: bool, ctrl: bool) -> Self {
        PINNED.with(|slot| slot.set(Some((shift, ctrl))));
        PinnedMods
    }
}

#[cfg(test)]
impl Drop for PinnedMods {
    fn drop(&mut self) {
        PINNED.with(|slot| slot.set(None));
    }
}

/// 把模型推过来的两份状态写进钩子槽（`Shell::update` 每次收尾调一次；钩子在消息泵里回调，
/// 拿不到 `&Shell`，只能读这份副本）。
pub fn sync(owner: KeyOwner, sub_open: bool) {
    OWNER.with(|slot| slot.set(owner));
    SUB_OPEN.with(|slot| slot.set(sub_open));
}

/// 场景那两份事实（`Scene::on_chat` / `Scene::palette_open`）的独立入口 —— 单独一条而不并进
/// [`sync`]：那条的签名已经钉在 `main.rs` 的 `publish_keys` 上。`palette_open` 现在由
/// `main.rs` 报真值（= 命令浮层 `showing`），浮层开着时 [`classify`] 走 `Palette*` 那几档。
pub fn sync_pages(on_chat: bool, palette_open: bool) {
    ON_CHAT.with(|slot| slot.set(on_chat));
    PALETTE_OPEN.with(|slot| slot.set(palette_open));
}

/// 读当前场景（自测日志用：`KBHOOK scene=…`）。
pub fn scene() -> Scene {
    Scene {
        on_chat: ON_CHAT.with(|slot| slot.get()),
        sub_open: SUB_OPEN.with(|slot| slot.get()),
        palette_open: PALETTE_OPEN.with(|slot| slot.get()),
    }
}

/// 钩子当前看到的焦点归属（自测日志用）。
pub fn owner() -> KeyOwner {
    OWNER.with(|slot| slot.get())
}

/// 钩子是否还挂着。
pub fn is_installed() -> bool {
    HOOK.with(|slot| !slot.get().is_null())
}

/// 装钩子并把回投句柄存进 TLS。返回 `false` = `SetWindowsHookExW` 失败 ⇒ 一个键都不拦，
/// 整套行为退回「没有键盘事件」的旧分叉，界面上一个像素都不变。
pub fn install(sink: impl Fn(KeyAction) + 'static) -> bool {
    let sink: Sink = Rc::new(sink);
    SINK.with(|slot| *slot.borrow_mut() = Some(sink));
    let handle = unsafe {
        SetWindowsHookExW(WH_KEYBOARD, kbd_proc as *const c_void, std::ptr::null(), GetCurrentThreadId())
    };
    HOOK.with(|slot| slot.set(handle));
    INSTALLER_TID.with(|slot| slot.set(unsafe { GetCurrentThreadId() }));
    !handle.is_null()
}

/// 卸钩子的三种落点（自测日志按这个区分「真卸了」与「交给 OS 收尾」）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unhooked {
    /// `UnhookWindowsHookEx` 返回非 0。
    Removed,
    /// 压根没装上（`SetWindowsHookExW` 当时就失败，或已经卸过一次）。
    NotInstalled,
    /// 调用线程 ≠ 装钩子线程：跨线程卸线程钩子不保证安全，留给线程退出时由 OS 摘。
    SkippedThread,
}

/// 卸钩子。漏掉 `UnhookWindowsHookEx` 会把这一枪永远挂在钩子链上，所以 `main()` 在
/// `App::run_component` 返回后第一件事就是它。只在**装钩子那条线程**上真卸（见 [`Unhooked`]）。
pub fn uninstall() -> Unhooked {
    let handle = HOOK.with(|slot| slot.get());
    if handle.is_null() {
        return Unhooked::NotInstalled;
    }
    let installer = INSTALLER_TID.with(|slot| slot.get());
    if installer != 0 && installer != unsafe { GetCurrentThreadId() } {
        return Unhooked::SkippedThread;
    }
    HOOK.with(|slot| slot.set(std::ptr::null_mut()));
    SINK.with(|slot| drop(slot.borrow_mut().take()));
    OWNER.with(|slot| slot.set(KeyOwner::Other));
    SUB_OPEN.with(|slot| slot.set(false));
    ON_CHAT.with(|slot| slot.set(true));
    PALETTE_OPEN.with(|slot| slot.set(false));
    if unsafe { UnhookWindowsHookEx(handle) != 0 } {
        Unhooked::Removed
    } else {
        Unhooked::NotInstalled
    }
}

/// 取证开关：`BLADE2_KBDBG=1` 时把钩子每一发**过表键**的决策打到 stdout（与 `DIAG: KBHOOK`
/// 同一条通道，界面上不占一个像素）。默认关 —— 钩子 proc 是全局热点，只在自测注键时开。
/// 值只算一次（`OnceLock`），所以关掉时连 `get_env` 都不会走。
fn kb_dbg() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("BLADE2_KBDBG").is_ok())
}

/// `WM_KEYDOWN` / `WM_KEYUP` 的 lParam 位段：bit31 = 转移态（1 = 正在松开）。
fn transition_up(l_param: *const c_void) -> bool {
    key_flag_bit(l_param, 31)
}

/// 动作 → 回投。`Pass` 不投消息（钩子那边只是不拦键）。
fn dispatch(action: KeyAction) {
    // 铁律：先 clone 出句柄、把 `RefCell` 借用丢掉，**再** send。钩子在消息泵里会重入，
    // 跨着借用 send 就是 `BorrowMutError` panic = 穿 `extern "system"` 边界 = 进程没了。
    let sink = SINK.with(|slot| slot.borrow().clone());
    if let Some(sink) = sink {
        sink(action);
    }
}

/// 钩子入口。**任何路径都不许 panic**：决策体整个套 `catch_unwind`，`Ok(false)` 与 `Err(_)`
/// 都走 `CallNextHookEx`（放行）—— 决策出问题时宁可让键落进 `TextBox`，也绝不吞用户的按键。
extern "system" fn kbd_proc(code: i32, w_param: usize, l_param: *const c_void) -> usize {
    let swallow = panic::catch_unwind(AssertUnwindSafe(|| decide(code, w_param, l_param))).unwrap_or(false);
    if swallow {
        return 1;
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param) }
}

/// `WM_KEYDOWN` / `WM_KEYUP` 的 lParam 位段：bit30 = 上一次的状态（1 = 已经按着 ⇒ 这是长按自动
/// 重复补发的那一发），bit31 = 转移态（1 = 正在松开）。
fn key_flag_bit(l_param: *const c_void, shift: u32) -> bool {
    ((l_param as i64) >> shift) & 1 == 1
}

/// 长按自动重复补发的按下（bit30=1 且 bit31=0）。
fn auto_repeat(l_param: *const c_void) -> bool {
    key_flag_bit(l_param, 30) && !transition_up(l_param)
}

/// 钩子真正要过表的键：其余键（字母、数字、Backspace、方向键里的左右…）走快路径直接放行，
/// 连 `GetKeyState` 都不问 —— 钩子 proc 是全局热点，`LowlevelHooksTimeout` 那类预算经不起浪费。
const HANDLED_KEYS: [u32; 5] = [VK_RETURN, VK_ESCAPE, VK_TAB, VK_UP, VK_DOWN];

/// 只有浮层上下移动允许长按连发（主干按住 ↑ 就是一格一格挪）；发送/收页/采纳这类**一次性的
/// 动作**在长按重复时只吞键不再投递。
fn repeat_dispatches(action: KeyAction) -> bool {
    matches!(action, KeyAction::PaletteMove(_))
}

/// 返回 `true` = 吞掉这一发键（proc 里 `return 1`）。
fn decide(code: i32, w_param: usize, l_param: *const c_void) -> bool {
    if code < 0 {
        return false;
    }
    let vk = w_param as u32;
    // 取证要连「被早退还回去的那一发」一起看见（尤其 `up=` 这一位：它决定这发算按下还是松开），
    // 所以放在两道早退**之前**。表外键（字母、Backspace…）不占日志。
    let traced = kb_dbg() && HANDLED_KEYS.contains(&vk);
    if traced {
        println!(
            "DIAG: KBKEY vk={vk:#04x} up={} rep={} lparam={:#x}",
            transition_up(l_param),
            auto_repeat(l_param),
            l_param as usize
        );
    }
    // 只管按下；松开那一发放行（探针实测：吞掉 0x5A 的按下之后，它的松开不会让 XAML 插字符）。
    if transition_up(l_param) {
        return false;
    }
    if !HANDLED_KEYS.contains(&vk) {
        return false; // 快路径：表外键连 GetKeyState 都不问
    }
    let owner = OWNER.with(|s| s.get());
    let scene_now = scene();
    let action = classify(vk, shift_down(), ctrl_down(), owner, scene_now);
    if traced {
        println!(
            "DIAG: KBKEY vk={vk:#04x} shift={} ctrl={} owner={owner:?} scene={scene_now:?} action={action:?}",
            shift_down(),
            ctrl_down()
        );
    }
    if action == KeyAction::Pass {
        return false;
    }
    // 长按重复：该吞还得吞（composer 那颗框是 `AcceptsReturn` 的，漏一发 Enter 就多一个换行），
    // 但除了浮层移动之外不再重复投递消息（按住 Enter 不该刷出 N 条发送）。
    if !(auto_repeat(l_param) && !repeat_dispatches(action)) {
        dispatch(action);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 整张映射表（不起 GUI 就能钉死）：逐行对着主干 `OnInputPreviewKeyDown` / `TryHandleEnterSend`。
    #[test]
    fn key_action_table_matches_mainline() {
        // Enter + Composer ⇒ 吞键发送。
        assert_eq!(
            action_for(VK_RETURN, false, false, KeyOwner::Composer, false),
            KeyAction::Submit
        );
        // Enter + Shift ⇒ 放行，让 TextBox 自己插换行。
        assert_eq!(
            action_for(VK_RETURN, true, false, KeyOwner::Composer, false),
            KeyAction::Pass
        );
        // Enter + Ctrl ⇒ 反向档那一发（忙不忙、取哪个 mode 由模型判，见 `ctrl_enter_mode`）。
        assert_eq!(
            action_for(VK_RETURN, false, true, KeyOwner::Composer, false),
            KeyAction::SubmitAlternate
        );
        assert_eq!(
            action_for(VK_RETURN, true, true, KeyOwner::Composer, false),
            KeyAction::Pass,
            "Ctrl 不许翻掉 Shift 的换行档"
        );
        // Enter + 别的字段 ⇒ 一律放行（主干在那些框里 Enter 不做事）。
        assert_eq!(action_for(VK_RETURN, false, false, KeyOwner::Other, false), KeyAction::Pass);
        assert_eq!(action_for(VK_RETURN, false, true, KeyOwner::Other, true), KeyAction::Pass);
        // Esc：只有二级页开着才消费，跟焦点归属无关（主干那条绑在根上）。
        assert_eq!(action_for(VK_ESCAPE, false, false, KeyOwner::Composer, true), KeyAction::CloseSub);
        assert_eq!(action_for(VK_ESCAPE, false, false, KeyOwner::Other, true), KeyAction::CloseSub);
        assert_eq!(action_for(VK_ESCAPE, false, false, KeyOwner::Composer, false), KeyAction::Pass);
        assert_eq!(action_for(VK_ESCAPE, true, true, KeyOwner::Other, false), KeyAction::Pass);
        // 普通字符键（A = 0x41）、方向键（左 0x27 / 上 0x26）、Tab（0x09）任何场合都放行。
        for owner in [KeyOwner::Composer, KeyOwner::Other] {
            for sub in [false, true] {
                assert_eq!(action_for(0x41, false, false, owner, sub), KeyAction::Pass);
                assert_eq!(action_for(0x27, false, false, owner, sub), KeyAction::Pass);
                assert_eq!(action_for(0x26, true, true, owner, sub), KeyAction::Pass);
                assert_eq!(action_for(0x09, false, false, owner, sub), KeyAction::Pass, "Tab");
            }
        }
    }

    /// 钩子决策体的纯逻辑（不装钩子也能验）：`code < 0`、松开那一发、表外键一律放行。
    #[test]
    fn decide_only_swallows_what_the_table_says() {
        let _pin = PinnedMods::set(false, false);
        sync_pages(true, false); // 同上：先把场景擦回「停在聊天页、没浮层」
        let down = 1usize as *const c_void; // bit31=0（按下）、bit30=0（之前没按下）
        let up = 0xC000_0001usize as *const c_void; // bit31=1 = 松开
        sync(KeyOwner::Other, false);
        assert!(!decide(-1, VK_RETURN as usize, down), "code<0 必须原样放行");
        assert!(!decide(0, 0x41, down), "字符键放行");
        assert!(!decide(0, VK_RETURN as usize, up), "Enter 的松开那一发放行");
        assert!(!decide(0, VK_ESCAPE as usize, down), "没开二级页时 Esc 放行");
        sync(KeyOwner::Composer, true);
        assert!(decide(0, VK_RETURN as usize, down), "焦点在 composer 的 Enter 被吞");
        assert!(decide(0, VK_ESCAPE as usize, down), "二级页开着时 Esc 被吞");
        assert!(!decide(0, VK_RETURN as usize, up), "松开那一发永远不吞");
        sync(KeyOwner::Other, false);
        assert!(!decide(0, VK_RETURN as usize, down));
        assert!(!decide(0, VK_ESCAPE as usize, down));
    }

    /// 没有回投句柄时决策体不得崩（进程在钩子里 panic = 直接没了、stdout 0 字节）。
    #[test]
    fn dispatch_without_sink_is_inert() {
        let _pin = PinnedMods::set(false, false);
        SINK.with(|slot| *slot.borrow_mut() = None);
        sync(KeyOwner::Composer, false);
        assert!(decide(0, VK_RETURN as usize, 1usize as *const c_void));
    }

    /// #52 那几档的完整表（纯函数，浮层本身还没做也先钉死语义）。
    #[test]
    fn seam_table_matches_mainline_palette_and_root_esc() {
        let palette = Scene { on_chat: true, sub_open: false, palette_open: true };
        // HandlePaletteKey(:17755)：Esc 收浮层、↑/↓ 移选中项、Tab 与 Enter 采纳。
        assert_eq!(classify(VK_RETURN, false, false, KeyOwner::Composer, palette), KeyAction::PaletteAccept);
        assert_eq!(classify(VK_TAB, false, false, KeyOwner::Composer, palette), KeyAction::PaletteAccept);
        assert_eq!(classify(VK_UP, false, false, KeyOwner::Composer, palette), KeyAction::PaletteMove(-1));
        assert_eq!(classify(VK_DOWN, false, false, KeyOwner::Composer, palette), KeyAction::PaletteMove(1));
        assert_eq!(classify(VK_ESCAPE, false, false, KeyOwner::Composer, palette), KeyAction::PaletteClose);
        // Shift+Enter 在浮层里也仍是换行（那一支 `return false`，回到 TryHandleEnterSend 又
        // 因为 Shift `return false`）⇒ 两遍都是放行。
        assert_eq!(classify(VK_RETURN, true, false, KeyOwner::Composer, palette), KeyAction::Pass);
        // 浮层挂在输入框的 PreviewKeyDown 上 ⇒ 焦点不在 composer 时这几档一概不开火。
        for vk in [VK_RETURN, VK_TAB, VK_UP, VK_DOWN, VK_ESCAPE] {
            assert_eq!(
                classify(vk, false, false, KeyOwner::Other, palette),
                KeyAction::Pass,
                "焦点不在 composer 时 {vk:#04x} 不该被浮层吃掉"
            );
        }
        // 浮层优先于二级页：根那条 Esc 见浮层可见就让路（:9262 `return`）。
        let both = Scene { on_chat: false, sub_open: true, palette_open: true };
        assert_eq!(classify(VK_ESCAPE, false, false, KeyOwner::Composer, both), KeyAction::PaletteClose);
        // OnRootPreviewKeyDown(:9245)：非聊天页 + 没二级页 ⇒ 回聊天页；有二级页先收二级页。
        let settings = Scene { on_chat: false, sub_open: false, palette_open: false };
        for owner in [KeyOwner::Composer, KeyOwner::Other] {
            assert_eq!(classify(VK_ESCAPE, false, false, owner, settings), KeyAction::ReturnToChat);
        }
        let settings_sub = Scene { on_chat: false, sub_open: true, palette_open: false };
        assert_eq!(classify(VK_ESCAPE, false, false, KeyOwner::Other, settings_sub), KeyAction::CloseSub);
        // 聊天页、没浮层 ⇒ Esc 什么都不做（主干根上那条只认 SettingsPage 可见）。
        assert_eq!(classify(VK_ESCAPE, false, false, KeyOwner::Other, Scene::default()), KeyAction::Pass);
        // 浮层没开时 Tab/↑/↓ 永远放行（跟焦点、跟页面、跟修饰键都无关）。
        for owner in [KeyOwner::Composer, KeyOwner::Other] {
            for scene in [Scene::default(), settings, settings_sub] {
                for vk in [VK_TAB, VK_UP, VK_DOWN] {
                    assert_eq!(classify(vk, true, true, owner, scene), KeyAction::Pass, "{scene:?}");
                }
            }
        }
        // Enter 发送那几档不受场景影响（浮层没开时）：`action_for` 那个窄门面与 `classify`
        // 必须逐键给出同样的答案 —— 门面不许漂移出第二套语义。
        for shift in [false, true] {
            for ctrl in [false, true] {
                for owner in [KeyOwner::Composer, KeyOwner::Other] {
                    for sub in [false, true] {
                        for vk in [VK_RETURN, VK_ESCAPE, VK_TAB, VK_UP, VK_DOWN, 0x41] {
                            assert_eq!(
                                action_for(vk, shift, ctrl, owner, sub),
                                classify(
                                    vk,
                                    shift,
                                    ctrl,
                                    owner,
                                    Scene { on_chat: true, sub_open: sub, palette_open: false }
                                ),
                                "门面与全表在 {vk:#04x} shift={shift} ctrl={ctrl} {owner:?} sub={sub} 上不一致"
                            );
                        }
                    }
                }
            }
        }
    }

    /// 接缝确实是「装上但不开火」：默认场景下整张表产不出 `Palette*`，也产不出 `ReturnToChat`。
    #[test]
    fn seam_stays_inert_until_mainline_ships() {
        // TLS 是**每线程**的，而测试跑在线程池上 ⇒ 同一根线程上先后两发测试会互相看得见。
        // 开头先把这份槽擦回默认，别依赖「谁先跑」。
        sync(KeyOwner::Other, false);
        sync_pages(true, false);
        assert_eq!(Scene::default(), scene());
        for shift in [false, true] {
            for ctrl in [false, true] {
                for owner in [KeyOwner::Composer, KeyOwner::Other] {
                    for vk in [VK_RETURN, VK_ESCAPE, VK_TAB, VK_UP, VK_DOWN, 0x41, 0x1C] {
                        let action = classify(vk, shift, ctrl, owner, Scene::default());
                        assert!(
                            !matches!(
                                action,
                                KeyAction::PaletteClose | KeyAction::PaletteMove(_) | KeyAction::PaletteAccept
                            ),
                            "默认场景不该冒出浮层动作：{vk:#04x} {shift} {ctrl} {owner:?} -> {action:?}"
                        );
                        if matches!(action, KeyAction::ReturnToChat) {
                            panic!("默认场景（停在聊天页）不该冒出返回聊天页：{vk:#04x} {owner:?}");
                        }
                    }
                }
            }
        }
    }

    /// 长按 Enter：吞重复那发（`AcceptsReturn` 的框漏一发就多一个换行）但**不再投递**消息；
    /// 浮层的 ↑ 相反，每发都要投（主干按住就是一格一格挪）。
    #[test]
    fn held_key_swallows_without_reposting() {
        let _pin = PinnedMods::set(false, false);
        let fired = SinkLog::new();

        let first = 1usize as *const c_void; // bit30=0 bit31=0
        let repeat = 0x4000_0001usize as *const c_void; // bit30=1 bit31=0
        let release = 0xC000_FFFFusize as *const c_void; // bit30=1 bit31=1
        sync(KeyOwner::Composer, false);
        sync_pages(true, false);
        assert!(decide(0, VK_RETURN as usize, first));
        assert!(decide(0, VK_RETURN as usize, repeat), "重复那发仍要吞（不吞就多插一个换行）");
        assert!(!decide(0, VK_RETURN as usize, release), "松开永远放行");
        assert_eq!(fired.drain(), vec![KeyAction::Submit], "按住 Enter 只投一条发送");

        // 浮层档：把接缝当已经开着验一次语义（不改 `main.rs` 的调用，只在测试里造场景）。
        let palette = Scene { on_chat: true, sub_open: false, palette_open: true };
        assert_eq!(classify(VK_UP, false, false, KeyOwner::Composer, palette), KeyAction::PaletteMove(-1));
        assert!(repeat_dispatches(KeyAction::PaletteMove(-1)), "↑ 长按必须连发");
        for action in [
            KeyAction::Submit,
            KeyAction::SubmitAlternate,
            KeyAction::CloseSub,
            KeyAction::ReturnToChat,
            KeyAction::PaletteAccept,
            KeyAction::PaletteClose,
            KeyAction::Pass,
        ] {
            assert!(!repeat_dispatches(action), "{action:?} 长按不许连发");
        }
        fired.off();
    }

    /// 场景贴进 thread-local 后钩子读得到的就是那份（`main.rs` 每轮 `update` 收尾会刷它）。
    #[test]
    fn sync_pages_feeds_the_hook() {
        let _pin = PinnedMods::set(false, false);
        let fired = SinkLog::new();
        sync(KeyOwner::Other, false);
        sync_pages(false, false);
        assert_eq!(scene(), Scene { on_chat: false, sub_open: false, palette_open: false });
        // 设置页 + 二级页也开着 ⇒ 先收二级页（主干 `InSettingsSubPage ? CloseSettingsSubPage()`）。
        sync(KeyOwner::Other, true);
        assert_eq!(scene(), Scene { on_chat: false, sub_open: true, palette_open: false });
        assert!(decide(0, VK_ESCAPE as usize, 1usize as *const c_void));
        assert_eq!(fired.drain(), vec![KeyAction::CloseSub]);
        // 关掉二级页 ⇒ 同一发 Esc 变成「回聊天页」（主干 `else ShowChatPage()`）。
        sync(KeyOwner::Other, false);
        assert!(decide(0, VK_ESCAPE as usize, 1usize as *const c_void));
        assert_eq!(fired.drain(), vec![KeyAction::ReturnToChat]);
        // 浮层接缝开火：`sync_pages` 报上浮层开着 ⇒ Enter 变采纳、↑ 变移动（都是吞键 + 投消息）。
        sync(KeyOwner::Composer, false);
        sync_pages(true, true);
        assert!(decide(0, VK_RETURN as usize, 1usize as *const c_void));
        assert!(decide(0, VK_UP as usize, 1usize as *const c_void));
        assert_eq!(fired.drain(), vec![KeyAction::PaletteAccept, KeyAction::PaletteMove(-1)]);
        // 回落到默认场景，别把这份 TLS 留给别的断言。
        sync(KeyOwner::Other, false);
        sync_pages(true, false);
        fired.off();
    }

    /// `SINK` 的测试替身：记录钩子回投过哪些动作（`Rc` 够用 —— 钩子与测试在同一条线程上）。
    struct SinkLog(Rc<RefCell<Vec<KeyAction>>>);

    impl SinkLog {
        fn new() -> Self {
            let log = SinkLog(Rc::new(RefCell::new(Vec::new())));
            let sink = {
                let inner = log.0.clone();
                Rc::new(move |a: KeyAction| inner.borrow_mut().push(a)) as Rc<dyn Fn(KeyAction)>
            };
            SINK.with(|slot| *slot.borrow_mut() = Some(sink));
            log
        }

        fn drain(&self) -> Vec<KeyAction> {
            std::mem::take(&mut *self.0.borrow_mut())
        }

        fn off(&self) {
            SINK.with(|slot| *slot.borrow_mut() = None);
            self.drain();
        }
    }
}
