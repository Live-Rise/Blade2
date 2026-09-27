//! 崩溃落盘（缺口 #85 第一条 / 审计 P2-1）：`%TEMP%\blade2_unhandled.txt`
//!
//! ## 主干那份的四个事实（逐字抄，别猜）
//!
//! 主干有**三个**写手，全部走 `File.AppendAllText(Path.Combine(Path.GetTempPath(), "blade2_unhandled.txt"), …)`：
//!
//! | 出口 | 主干位置 | 时机 | 记录形状（`$"…"` 原文） |
//! |---|---|---|---|
//! | `[app]` | `App.xaml.cs:18-28` | UI 线程未处理异常（`Application.UnhandledException`），写完把 `e.Handled = true` | `\n=== {MM-dd HH:mm:ss} [app] handled={e.Handled} ===\n{e.Message}\n{e.Exception}\n---STACK---\n{e.Exception?.StackTrace}\n---UI-THREAD---\n{Environment.StackTrace}` |
//! | `[appdomain]` | `App.xaml.cs:30-40` | 任意线程的裸异常（`AppDomain.CurrentDomain.UnhandledException`），只留档 | `\n=== {MM-dd HH:mm:ss} [appdomain] fatal={e.IsTerminating} ===\n{ex}\n---STACK---\n{ex?.StackTrace}` |
//! | `[ui-post]` | `MainWindow.xaml.cs:18670-18680`（`LogUiFault`，由 `PostUi` 的 `catch` 调） | DispatcherQueue 回调里**被自吞**的那发（0xc000027b 教训） | `\n=== {MM-dd HH:mm:ss} [ui-post] ===\n{ex}\n---STACK---\n{ex.StackTrace}` |
//!
//! 四条口径全在这张表里：**文件名** = `blade2_unhandled.txt`；**目录** = `Path.GetTempPath()`（= `%TEMP%`）；
//! **时机** = 异常一出口立刻，写完继续（`[app]` 甚至把 `Handled` 置真让界面活着）；**是否追加** = 是
//! （`AppendAllText`，缺文件新建），且整段包在 `try { } catch { }` 里 —— **写失败静默**，绝不让记日志这件事
//! 反过来把进程带死。时间戳是 `DateTime.Now:MM-dd HH:mm:ss`（**本地时间**、不含年份），C# 的 `bool` 插值
//! 出来的字面量是 `True`/`False`（首字母大写）——这两点都影响逐字比对，分叉照抄，不改成 ISO。
//!
//! ## 分叉这一发覆盖到哪、覆盖不到哪（这条是必答项，实测结论）
//!
//! 分叉装的是 `std::panic::set_hook`，记录形状走 `[appdomain]` 那一支（它正是主干「任意线程 + 只留档」那支）。
//!
//! · **抓得到**：任何 Rust panic —— 含 UI 线程、`spawn_background` 工作线程，以及** reactor 回调内部**那一发。
//!   最后这条有源码凭据：`windows-reactor-0.100.0/src/native/winui/mod.rs:4166-4169` 的 `invoke_callback()`
//!   写的是「`catch_unwind(callback.call(value))` 失败 ⇒ `std::process::abort()`」，而 panic hook 在 unwind
//!   之前就跑完了 ⇒ `on_click` / `on_text_changed` 那类回调里炸出来的 panic **能**先落盘再死。
//!
//! · **抓不到（一）：Rust panic 机器之外的原生崩**。reactor 自己的 `std::process::abort()`
//!   （`app.rs:1196-1206` `pump_error()` 的 `NativeApplyFailed` 分支 —— 它连 panic 都没有，只
//!   `eprintln!` 一行就 abort）不经过 panic hook，一个字也写不出来。仓库里的现成旁证：
//!   `rust/tmp/hs-matrix.txt` 三发 `exit=0xC000027B` 全是 **stdout/stderr 0 字节**，
//!   `src/main.rs:1174` 那条注释记的是同一件事（「进程退出码 0xC000027B，stdout/stderr 全空」）。
//!   —— 这一片现在由下面那张实测表决定捞回多少。
//!
//! ## VEH / UEF 能补到哪一步：实测，不是猜（终审那条必答题）
//!
//! 本模块自带的五发死法（`BLADE2_CRASH_TEST=panic|abort|fastfail|stowed|av`，
//! `BLADE2_CRASH_PROBE_GUARD=1` 时额外装探针 VEH）逐条量，原始记录在
//! `rust/tmp/cd1-crashprobe-outcome.txt`：
//!
//! | 死法 | 退出码（实测） | panic hook | VEH(first-chance) | UEF(last-chance) |
//! |---|---|---|---|---|
//! | Rust `panic!` | 0x65 = 101 | **捞到**（含完整栈，stderr 那份输出一字未变） | 捞到（0xE06D7363：Rust 在 Windows 上抛 panic 就走这个 C++ 形状码） | 不走（正常退出） |
//! | `std::process::abort()` | 0xC0000409 | 漏 | 漏 | **漏** |
//! | `RaiseFailFastException` | 0xC0000602 | 漏 | 漏 | **漏** |
//! | `RaiseException(0xC000027B)` | 0xC000027B | 漏 | 捞到 | **捞到** |
//! | 写空指针 | 0xC0000005 | 漏 | 捞到 | **捞到** |
//!
//! · 结论一：**主干/分叉那类 XAML stowed exception（0xc000027b）UEF 捞得到** —— 而它正是分叉真挨过
//!   的那发（`tmp/hs-matrix.txt` 的退出码就是 0xC000027B）⇒ 所以 UEF 装进了默认路径
//!   （[`install_lastchance_guard`]）。UEF 只在「一路没人接、马上要 WER」那一发上跑 ⇒ 零噪声。
//! · 结论二：**VEH 不进默认路径**。它是 first-chance 出口，WinUI/XAML 一天要抛一堆**已被接住**的
//!   C++ 异常（0xE06D7363 那一族，实测里连我们自己的 panic 都以它露面），条条落盘 = 把
//!   「界面活得好好的」误报成崩了 ⇒ 只留在自测那一发里当量具。
//! · 结论三（**补不上的那片，明写备案**）：**fail-fast 一律捞不到**。`std::process::abort()` 与
//!   `RaiseFailFastException`（0xc0000409）走 `__fastfail`，内核**不做异常分发** ⇒ VEH / UEF 全被跳过；
//!   而 reactor 那两条 catch 不住的出口（`pump_error` 的 abort、`invoke_callback` abort 之后）正是这一族。
//!   ⇒ 分叉**抓不到**的部分精确为：「reactor 自己 `eprintln!` + abort 的原生 fail-fast」，
//!   现场只剩 stderr 那一行（真 0 字节时就是全黑）。主干同位置也只到 `[app]`/`[appdomain]` 为止
//!   —— 托管层同样抓不到 fail-fast，这不是分叉独有的坑，是同一堵墙。

use std::backtrace::Backtrace;
use std::env;
use std::ffi::c_void;
use std::io::Write;
use std::panic::{self, AssertUnwindSafe, PanicHookInfo};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::thread::ThreadId;

/// 主干那三个写手共用的文件名（`App.xaml.cs:23` / `:36`、`MainWindow.xaml.cs:18678`）。
pub const FILE_NAME: &str = "blade2_unhandled.txt";
/// 自测出口的环境变量（**默认路径上永不触发**）：取值见 [`SELF_TEST_MODES`]。
pub const SELF_TEST_ENV: &str = "BLADE2_CRASH_TEST";
/// 探针 VEH 的开关：`=1` 时 [`self_test_if_asked`] 先装一发 first-chance 出口再死。
/// 只用来量「VEH 与 UEF 各能捞到哪一类」，默认路径不装（噪声理由见模块头注结论二）。
pub const PROBE_GUARD_ENV: &str = "BLADE2_CRASH_PROBE_GUARD";

/// 主干 `[appdomain]` 那一支的 tag。
const TAG: &str = "appdomain";

/// 自测支持的死法（逐条对应「分叉真会出现的那几类」）。
pub const SELF_TEST_MODES: [&str; 5] = ["panic", "abort", "fastfail", "stowed", "av"];

// ---------------------------------------------------------------- Win32 入口
// 零依赖口径同 `keys.rs:35-42`、`procguard.rs:83-109`：`#[link(name = "kernel32")]` 自己声明，
// **不新增 Cargo 依赖**（`--offline` 构建必须继续可用，`Cargo.lock` 里没有 `windows` 主 crate）。

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct SystemTime {
    w_year: u16,
    w_month: u16,
    w_day_of_week: u16,
    w_day: u16,
    w_hour: u16,
    w_minute: u16,
    w_second: u16,
    w_milliseconds: u16,
}

/// `EXCEPTION_POINTERS`：只用得上 `ExceptionRecord->ExceptionCode` 与 `->ExceptionAddress`。
#[repr(C)]
struct ExceptionPointers {
    exception_record: *mut ExceptionRecord,
    exception_context: *mut c_void,
}

#[repr(C)]
struct ExceptionRecord {
    exception_code: u32,
    exception_flags: u32,
    exception_record: *mut ExceptionRecord,
    exception_address: *mut c_void,
    number_parameters: u32,
    padding: u32,
    exception_information: [usize; 15],
}

/// VEH/UEF 的返回值：只留档，绝不吞别人（继续往外找处理器）。
const EXCEPTION_CONTINUE_SEARCH: i32 = 0;
/// 主干/分叉事件日志里那个 stowed exception 码（`docs/DESIGN.zh.md:21` 记的同一发）。
const STATUS_STOWED_EXCEPTION: u32 = 0xC000_027B;

/// `LPTOP_LEVEL_EXCEPTION_FILTER`：`SetUnhandledExceptionFilter` 的槽位类型，可空。
type TopLevelExceptionFilter = Option<unsafe extern "system" fn(*mut ExceptionPointers) -> i32>;
/// `PVECTORED_EXCEPTION_HANDLER`。
type VectoredExceptionHandler = Option<unsafe extern "system" fn(*mut ExceptionPointers) -> i32>;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetLocalTime(system_time: *mut SystemTime);
    fn AddVectoredExceptionHandler(first: u32, handler: VectoredExceptionHandler) -> *mut c_void;
    fn SetUnhandledExceptionFilter(filter: TopLevelExceptionFilter) -> TopLevelExceptionFilter;
    fn RaiseException(code: u32, flags: u32, number_parameters: u32, arguments: *const usize);
    fn RaiseFailFastException(record: *const c_void, context: *const c_void, flags: u32);
}

// ---------------------------------------------------------------- 路径与时间戳

/// 主干 `Path.GetTempPath()`：先认 `%TEMP%`，拿不到/空串就退回 `std::env::temp_dir()`。
pub fn temp_dir() -> PathBuf {
    dir_from(env::var("TEMP").ok().as_deref(), &env::temp_dir())
}

/// [`temp_dir`] 的纯函数内核（单测不碰进程环境也能把两条分支钉死）。
fn dir_from(temp: Option<&str>, fallback: &Path) -> PathBuf {
    match temp.map(str::trim).filter(|value| !value.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => fallback.to_path_buf(),
    }
}

/// 那份文件的完整路径 = `Path.Combine(Path.GetTempPath(), "blade2_unhandled.txt")`。
pub fn path() -> PathBuf {
    temp_dir().join(FILE_NAME)
}

/// 主干 `DateTime.Now:MM-dd HH:mm:ss`：本地时间、无年份、每段两位。
fn local_stamp() -> String {
    let mut raw = SystemTime::default();
    unsafe { GetLocalTime(&mut raw) };
    format!(
        "{}-{} {}:{}:{}",
        pad2(raw.w_month),
        pad2(raw.w_day),
        pad2(raw.w_hour),
        pad2(raw.w_minute),
        pad2(raw.w_second)
    )
}

fn pad2(value: u16) -> String {
    format!("{value:02}")
}

/// C# 的 `bool` 插值（`$"{e.IsTerminating}"`）出来是 `True`/`False`，逐字比对要一致。
fn bool_text(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

// ---------------------------------------------------------------- 记录形状

/// 主干 `[appdomain]` 那一支的完整形状（纯函数，单测逐字钉死）。
///
/// `description` 占主干 `{ex}`（`Exception.ToString()`）那一格，`stack` 占 `{ex?.StackTrace}` 那一格。
pub fn appdomain_record(stamp: &str, terminating: bool, description: &str, stack: &str) -> String {
    format!(
        "\n=== {stamp} [{TAG}] fatal={} ===\n{description}\n---STACK---\n{stack}",
        bool_text(terminating)
    )
}

/// panic 负载那一格的取法：`String` / `&str` 都认，别的负载给一句定黑话。
fn payload_text(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("<非字符串 panic 负载>")
}

/// panic → 主干 `{ex}` 那一格：一行结论 + 一行出处。
fn panic_description(info: &PanicHookInfo<'_>) -> String {
    let message = payload_text(info.payload());
    match info.location() {
        Some(location) => {
            format!("panic: {message}\n   at {}:{}", location.file(), location.line())
        }
        None => format!("panic: {message}"),
    }
}

/// 原生异常 → 同一格（只有探针那一发用得上）。
fn native_description(code: u32, address: usize, lane: &str) -> String {
    format!("native exception 0x{code:08X} at 0x{address:X} on {lane}")
}

// ---------------------------------------------------------------- 落盘

/// 主干 `File.AppendAllText(...)` + `try { } catch { }` 的等价物：追加、缺文件就新建、失败静默。
/// 体内**不许 panic**（只用 `let _ =`），记日志绝不反过来把进程带死。
fn append(text: &str) {
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path())
    else {
        return;
    };
    let _ = file.write_all(text.as_bytes());
    let _ = file.flush();
}

/// 一条记录的落盘（形状 = 主干 `[appdomain]`）。
fn record(description: &str, stack: &str, terminating: bool) {
    append(&appdomain_record(&local_stamp(), terminating, description, stack));
}

// ---------------------------------------------------------------- 安装

static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();

fn is_main_thread() -> bool {
    MAIN_THREAD
        .get()
        .is_some_and(|id| *id == std::thread::current().id())
}

/// 线程标签：主干 `[app]` 那支区分 UI/非 UI 线程，分叉把同一份事实写进描述那一格。
/// （`ThreadId::as_u64()` 在稳定版还没有 ⇒ 只能按 `Debug` 形状打出来，够定位就成。）
fn thread_label() -> String {
    let current = std::thread::current();
    format!("线程 {:?} {:?}", current.name().unwrap_or("<无名>"), current.id())
}

/// 装崩溃出口：在 `main()` 极早期调一次（早于 reactor 起壳、早于任何后台线程）。两发一起装：
/// · `std::panic::set_hook` —— Rust 侧一切 panic（含 reactor 回调内部那一发）；
/// · `SetUnhandledExceptionFilter` —— 一路没人接的**原生**异常（0xc000027b / 0xc0000005 这一类）。
///
/// 旧 hook 先留着、落盘后照 call；上一层 UEF 也留着、留完档交回去 ⇒ 分叉原先那份 stderr panic
/// 输出（`tmp/hs-matrix.txt` 一类取证脚本读的就是它）一个字都不变。
pub fn install() {
    let _ = MAIN_THREAD.set(std::thread::current().id());
    install_lastchance_guard();
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        // 整个体套 `catch_unwind`、体内只用 `let _ =`：hook 里再 panic 就是递归 ⇒ 当场死。
        let _ = panic::catch_unwind(AssertUnwindSafe(|| {
            let stack = Backtrace::force_capture().to_string();
            record(&panic_description(info), &stack, is_main_thread());
        }));
        let _ = panic::catch_unwind(AssertUnwindSafe(|| previous(info)));
    }));
}

/// 上一层 UEF（可能是别的库或 CRT 装的）：留完档原样交回去，不抢别人的槽位。
static PREVIOUS_UEF: OnceLock<TopLevelExceptionFilter> = OnceLock::new();

/// 最后机会处理器（UEF）：进程**真要死于没人接的原生异常时**才被叫一次。
///
/// 为什么这一条装在默认路径上、而 VEH 不装（实测数据见模块头注与
/// `rust/tmp/cd1-crashprobe-outcome.txt`）：
/// · UEF 只在「异常一路没人接、马上要 WER」这一发上跑 ⇒ **零噪声**，不会像 VEH 那样
///   在 WinUI/XAML 那一大堆**已被处理的 first-chance 异常**（C++ 0xE06D7363 一族）上
///   条条落盘、把「界面活得好好的」误报成崩了；
/// · 分叉真实挨过的那两类无声死（0xc000027b stowed / 0xc0000005 访问违规）UEF 都捞得到；
/// · fail-fast 那一族（`std::process::abort()` / `RaiseFailFastException`）内核不做分发，
///   UEF 与 VEH **一律**捞不到 —— 这一片是真补不上，已备案，不自欺。
fn install_lastchance_guard() {
    let previous = unsafe { SetUnhandledExceptionFilter(Some(lastchance_uef)) };
    if previous.is_some() {
        let _ = PREVIOUS_UEF.set(previous);
    }
}

unsafe extern "system" fn lastchance_uef(pointers: *mut ExceptionPointers) -> i32 {
    let _ = panic::catch_unwind(AssertUnwindSafe(|| unsafe { note_exception(pointers, "UEF") }));
    // 交还给上一层（没有就继续正常派发），绝不把别人的死法改成自己的取舍。
    match PREVIOUS_UEF.get().copied().flatten() {
        Some(previous) => unsafe { previous(pointers) },
        None => EXCEPTION_CONTINUE_SEARCH,
    }
}

/// 探针版 VEH（**只在自测那一发装**）：它是 first-chance 出口，装了会给每一条
/// 「其实已被接住」的异常落一行 ⇒ 只拿来量「VEH 与 UEF 各能捞到哪一类」，不进默认路径。
pub fn install_probe_guard() {
    unsafe {
        AddVectoredExceptionHandler(1, Some(probe_veh));
    }
}

unsafe extern "system" fn probe_veh(pointers: *mut ExceptionPointers) -> i32 {
    let _ = panic::catch_unwind(AssertUnwindSafe(|| unsafe { note_exception(pointers, "VEH") }));
    EXCEPTION_CONTINUE_SEARCH
}

unsafe fn note_exception(pointers: *mut ExceptionPointers, lane: &str) {
    let raw = if pointers.is_null() {
        std::ptr::null_mut()
    } else {
        unsafe { (*pointers).exception_record }
    };
    let (code, address) = if raw.is_null() {
        (0, 0usize)
    } else {
        let fetched = unsafe { &*raw };
        (fetched.exception_code, fetched.exception_address as usize)
    };
    record(
        &native_description(code, address, &format!("{lane} 捞到，{}", thread_label())),
        &format!("{lane}: 崩溃线程栈不解 unwind info（只留码与地址）"),
        true,
    );
}

// ---------------------------------------------------------------- 自测出口

/// 按 `BLADE2_CRASH_TEST` 的值当场死一次（**默认路径上什么都不做**：变量没设或不认 ⇒ 直接 `None`）。
/// `BLADE2_CRASH_PROBE_GUARD=1` 时先装探针，答案就落在同一份文件里。
pub fn self_test_if_asked() -> Option<&'static str> {
    let requested = env::var(SELF_TEST_ENV).ok().filter(|value| !value.is_empty())?;
    let chosen: &'static str = SELF_TEST_MODES.into_iter().find(|mode| *mode == requested)?;
    if env::var(PROBE_GUARD_ENV).is_ok_and(|value| value == "1") {
        install_probe_guard();
    }
    // 先留一行「谁按下的」，再看各层出口跟不跟得上（fastfail 那一发只有这一行留得下来）。
    record(
        &format!("BLADE2_CRASH_TEST={chosen}（自测触发）"),
        &format!("触发线程：{}", thread_label()),
        true,
    );
    match chosen {
        "panic" => panic!("BLADE2_CRASH_TEST=panic：自测那一发"),
        "abort" => std::process::abort(),
        "fastfail" => unsafe { RaiseFailFastException(std::ptr::null(), std::ptr::null(), 0) },
        "stowed" => unsafe {
            RaiseException(STATUS_STOWED_EXCEPTION, 0, 0, std::ptr::null());
        }
        "av" => unsafe { std::ptr::write_volatile(std::ptr::null_mut::<u8>(), 1) },
        other => record(&format!("未知的自测死法 {other}"), "未死", false),
    }
    Some(chosen)
}

// ---------------------------------------------------------------- 单测

#[cfg(test)]
mod tests {
    use super::*;

    /// 主干 `App.xaml.cs:36-37` 那发 `$"\n=== {…} [appdomain] fatal={e.IsTerminating} ===\n{ex}\n---STACK---\n{ex?.StackTrace}"`
    /// 逐字对形状：换行开头、tag 与 fatal 同行、两块正文之间正好一个 `---STACK---`。
    #[test]
    fn appdomain_record_matches_mainline_shape() {
        assert_eq!(
            appdomain_record(
                "09-23 14:02:11",
                true,
                "panic: boom\n   at src/main.rs:1",
                "   0: frame"
            ),
            "\n=== 09-23 14:02:11 [appdomain] fatal=True ===\npanic: boom\n   at src/main.rs:1\n---STACK---\n   0: frame"
        );
    }

    /// C# 的 bool 插值是大写首字母 —— 逐字比对时最容易写错的一处。
    #[test]
    fn fatal_uses_csharp_bool_casing() {
        assert!(appdomain_record("01-02 03:04:05", false, "x", "y").contains("fatal=False ==="));
        assert!(appdomain_record("01-02 03:04:05", true, "x", "y").contains("fatal=True ==="));
        assert!(!appdomain_record("01-02 03:04:05", true, "x", "y").contains("fatal=true"));
    }

    /// 主干用 `File.AppendAllText`：每条以一个换行**开头**（不是结尾），文件不存在时新建。
    #[test]
    fn record_leads_with_newline_and_has_no_trailing_newline() {
        let record = appdomain_record("09-23 14:02:11", true, "a", "b");
        assert!(record.starts_with("\n=== "), "{record}");
        assert!(!record.ends_with('\n'), "{record}");
        assert_eq!(record.matches("---STACK---").count(), 1);
    }

    /// `%TEMP%` 优先；空串/纯空白退回 `std::env::temp_dir()` 那一支。
    #[test]
    fn temp_dir_prefers_env_and_falls_back() {
        let fallback = Path::new(r"C:\Windows\Temp");
        assert_eq!(dir_from(Some(r"D:\tmp"), fallback), PathBuf::from(r"D:\tmp"));
        assert_eq!(dir_from(Some("   "), fallback), fallback.to_path_buf());
        assert_eq!(dir_from(None, fallback), fallback.to_path_buf());
    }

    /// 文件名与所在目录逐字对齐主干。
    #[test]
    fn file_name_matches_mainline() {
        assert_eq!(FILE_NAME, "blade2_unhandled.txt");
        assert_eq!(path().file_name().and_then(|v| v.to_str()), Some(FILE_NAME));
        assert_eq!(path().parent(), Some(temp_dir().as_path()));
    }

    /// 时间戳形状 = `MM-dd HH:mm:ss`（本地时间、无年份、每段两位、分隔符位置固定）。
    #[test]
    fn local_stamp_uses_mainline_format() {
        let stamp = local_stamp();
        assert_eq!(stamp.len(), 14, "{stamp}");
        let bytes = stamp.as_bytes();
        for (index, expected) in [(2, b'-'), (5, b' '), (8, b':'), (11, b':')] {
            assert_eq!(bytes[index], expected, "{stamp}");
        }
        assert!(
            bytes.iter().enumerate().all(|(index, byte)| {
                matches!(byte, b'0'..=b'9') || [2, 5, 8, 11].contains(&index)
            }),
            "{stamp}"
        );
        let month: u8 = stamp[0..2].parse().expect("月份两位");
        let day: u8 = stamp[3..5].parse().expect("日两位");
        assert!((1..=12).contains(&month), "{stamp}");
        assert!((1..=31).contains(&day), "{stamp}");
    }

    /// 负载那一格：`String` / `&str` 都认，别的负载走定黑话（不许 panic）。
    #[test]
    fn payload_text_handles_all_three_shapes() {
        let owned = "带外的一发".to_string();
        assert_eq!(payload_text(&owned), "带外的一发");
        let borrowed: &str = "字面量那一发";
        assert_eq!(payload_text(&borrowed), "字面量那一发");
        assert_eq!(payload_text(&7u32), "<非字符串 panic 负载>");
    }

    /// 自测那张表是闭集：默认路径（没设 / 设成不认的值）必须一个字都不做。
    #[test]
    fn self_test_modes_are_a_closed_set() {
        assert_eq!(SELF_TEST_ENV, "BLADE2_CRASH_TEST");
        assert_eq!(SELF_TEST_MODES, ["panic", "abort", "fastfail", "stowed", "av"]);
        // 未知值 ⇒ `self_test_if_asked` 走 `find` 失配那条 `?`，既不装探针也不死。
        assert!(!SELF_TEST_MODES.contains(&"window-closed"));
        assert!(!SELF_TEST_MODES.contains(&""));
    }

    /// 接线：`main()` 的**头两发**必须是「装出口」再「问一次自测开关」，且都早于
    /// `App::run_component`（晚了就漏掉最早炸的那几发：内核后台闭包与 view 首绘）。
    #[test]
    fn install_is_wired_first_thing_in_main() {
        let source = include_str!("main.rs");
        let (_, rest) = source.split_once("fn main() {").expect("main.rs 里找得到 fn main()");
        let body = rest.split_once("\n}").map_or(rest, |(body, _)| body);
        let install_at = body.find("crashlog::install()").expect("main() 里装了 crashlog::install()");
        let selftest_at = body
            .find("crashlog::self_test_if_asked()")
            .expect("main() 里问了自测开关");
        let run_at = body
            .find("App::run_component::<Shell>(())")
            .expect("main() 里起了 reactor");
        assert!(install_at < selftest_at, "先装出口再问开关：{body}");
        assert!(selftest_at < run_at, "两发都得早于起壳：{body}");
        assert_eq!(install_at, body.find("crashlog::").expect("第一条语句就是它"), "{body}");
    }
}
