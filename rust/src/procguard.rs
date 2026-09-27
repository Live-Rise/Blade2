//! 内核进程保底（task #68：修闪退残留内核占租约）。
//!
//! 主干那件保底是 `Dsh/DshKernelHost.cs` 在 0.8.1（`f35e6e3`）里长出来的，分叉这里对应两件事：
//!
//! · [`KillJob::attach`] —— 对应主干 `AttachToKillJob()`（`DshKernelHost.cs:170`），在 `Kernel`
//!   里**留住**作业句柄，句柄关闭即 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`（`:22`）带走内核**整棵树**
//!   （主干 `:155` 那句注释：「作业对象句柄关闭即触发 KILL_ON_JOB_CLOSE」；句柄只在 `Dispose` 收）。
//!   为什么必须有这层，主干在 `:164-167` 写得很清楚：**内核持有每条会话目录的写租约，只有内核进程
//!   死亡才释放**；壳 hard crash 时子进程不跟着退 ⇒ 每次崩溃留一个占着租约的孤儿 ⇒ 之后重进同一条
//!   会话必撞 `SessionAlreadyOwnedError`。分叉今天只有 `kernel.rs` 里那颗 `child.kill()`，
//!   杀的是内核**主进程一个 PID**，它自己起的 MCP 子进程（server-memory / playwright 等）全都活下来。
//!
//! · [`sweep_orphan_kernels`] —— 对应主干 `SweepOrphanKernels()`（`:253`）。
//!
//! ## 为什么这里的判定比主干**更窄**（这条是本模块存在的理由）
//!
//! 主干的枚举入口是 `Process.GetProcessesByName("node")`（`:275`）—— **按进程名**。这台机器上此刻
//! 就有 ~7 个 `node.exe`，而 agent 工具链本身就跑在 Node 上；照抄名字匹配等于给分叉一把能杀掉
//! 用户所有 node 进程的枪。所以这里两条硬条件各自都比主干窄：
//!
//! 1. **可执行文件的完整路径**必须正是本安装自带的内核（主干 `:288-291` 那半截本来就比名字窄，
//!    这里再收窄两点：判定目标只取 `kernel::bundled_kernel_exe()` = `kernel_dir_from(current_exe())`
//!    往上找到的那颗真实存在的 `Kernel\node.exe`，**不吃** `BLADE2_KERNEL_DIR`/`BLADE2_KERNEL_EXE`
//!    那两颗自测 env —— env 一旦把目标指到别家安装，就会把别人活着跑的内核判成孤儿；且比较走
//!    `canonicalize`（把链接/大小写/`..` 归一），**读不到模块路径一律不算命中**（`:287` 的
//!    `continue`，「无法确认身份就不碰」）。
//! 2. **父进程链上没有活着的本程序**（`:295` + `HasLiveAppAncestor` `:347`，深度封顶 16）。
//!    主干的判据是「链上没有活的 `Blade2`」（`:372` 按进程名），分叉自己叫 `blade2-rs.exe`，
//!    照抄那个名字就会把**主干正在跑的内核**当成孤儿（开发态两家用的是同一份 `Kernel\node.exe`）。
//!    这里的 owner 集是三者的并集：路径正是分叉自身 exe、或进程名是分叉自身 exe 的文件名、
//!    或进程名正是主干的 `Blade2`。三条全是**加保护**方向 ⇒ 只会少杀，不会多杀。
//!    另外 [`select_orphans`] 在 owner 集为空时直接返回空表：连一个「活着的本程序」都指认不出来，
//!    就说明枚举不可信（权限/快照被截），此时一个都不许杀。
//!
//! 还有一处比主干窄：主干对命中的进程下 `proc.Kill(entireProcessTree: true)`（`:300`），
//! 这里只对**自己两条判定都过了的那几个 PID** 发 `TerminateProcess`，不对整棵树广播。
//!
//! ## panic 口径
//!
//! 这里每个 `extern "system"` 边界都按「抛出去就是无声死进程」对待（分叉已经被 `0xC000027B` 那类
//! 静默死咬过；`keys.rs:22-26` 同一个教训）：不许 `unwrap()`/`expect()`，`BOOL` 一律按 0/非 0 判、
//! `HANDLE` 一律按 null/`INVALID_HANDLE_VALUE` 判，两个总入口 [`KillJob::attach`] 与
//! [`sweep_orphan_kernels`] 外面再各套一层 [`std::panic::catch_unwind`]。任何一步失败都只把原因
//! 写进 `note`，调用方照旧走今天的 `child.kill()` 保底 —— 少一层保底是缺陷，多杀一个进程是事故。

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Component, Path, PathBuf};

use crate::kernel;

/// Win32 句柄：统一按 `*mut c_void` 处理，null = 无效。
pub type Handle = *mut c_void;
/// Win32 `BOOL` 的原生宽度是 4 字节 `int`，所以按 `i32` 收，**不**当 `bool` 用。
type WinBool = i32;

const NULL_HANDLE: Handle = std::ptr::null_mut();
const INVALID_SNAPSHOT: Handle = usize::MAX as Handle;

/// `DshKernelHost.cs:22` / `:439`。
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
/// `DshKernelHost.cs:444`：`JOBOBJECTINFOCLASS::JobObjectExtendedLimitInformation`。
/// 主干注释点过同一个坑：类别与结构体长度不匹配（如 2 配 144 字节）时
/// `SetInformationJobObject` 以 `ERROR_BAD_LENGTH` **静默**失败 ⇒ 作业没套上、内核照旧留孤儿。
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: WinBool = 9;
/// `DshKernelHost.cs:440`。
const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
/// `PROCESS_QUERY_LIMITED_INFORMATION`：非提权进程读自身用户态进程的镜像路径够用。
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const PROCESS_TERMINATE: u32 = 0x0001;
/// `DshKernelHost.cs:350`：`HasLiveAppAncestor` 那个 `for (var depth = 0; depth < 16; depth++)`。
pub const MAX_ANCESTOR_DEPTH: usize = 16;
/// 主干的 app 进程名（`DshKernelHost.cs:372` `proc.ProcessName == "Blade2"`）。
/// 分叉把它**并进 owner 集**（不是拿它当命中条件）：开发态两家共用 `Kernel\node.exe`，
/// 不把主干活进程认成 owner，就会去杀 QA 正在用的那个内核。
const MAINLINE_APP_STEM: &str = "blade2";
/// `QueryFullProcessImageNameW` 的字符缓冲（含长路径）。
const IMAGE_PATH_CHARS: usize = 4096;

// ------------------------------------------------------------------ Win32 入口
//
// 零依赖口径与 `keys.rs:35-42` 一致：`#[link(name = "kernel32")]` + `unsafe extern "system"`
// 自己声明，**不新增 Cargo 依赖**（这里 `cargo` 跑 `--offline`，`Cargo.lock` 里没有 `windows` 主 crate）。

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateJobObjectW(job_attributes: *const c_void, name: *const u16) -> Handle;
    fn SetInformationJobObject(
        job: Handle,
        job_information_class: WinBool,
        job_information: *const c_void,
        job_information_length: u32,
    ) -> WinBool;
    fn AssignProcessToJobObject(job: Handle, process: Handle) -> WinBool;
    fn IsProcessInJob(process: Handle, job: Handle, result: *mut WinBool) -> WinBool;
    fn CloseHandle(object: Handle) -> WinBool;
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> Handle;
    fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry32W) -> WinBool;
    fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry32W) -> WinBool;
    fn OpenProcess(desired_access: u32, inherit_handle: WinBool, process_id: u32) -> Handle;
    fn QueryFullProcessImageNameW(
        process: Handle,
        flags: u32,
        exe_name: *mut u16,
        size: *mut u32,
    ) -> WinBool;
    fn TerminateProcess(process: Handle, exit_code: u32) -> WinBool;
}

/// `JOBOBJECT_BASIC_LIMIT_INFORMATION`。字段顺序与宽度按 x64 布局：`limit_flags` 之后那个
/// `usize`（`SIZE_T`）会把对齐垫到 8，与主干 `DshKernelHost.cs:400-412` 那份 `StructLayout` 同形。
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct JobObjectBasicLimitInformation {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct IoCounters {
    read_operation_count: u64,
    write_operation_count: u64,
    other_operation_count: u64,
    read_transfer_count: u64,
    write_transfer_count: u64,
    other_transfer_count: u64,
}

/// `JOBOBJECT_EXTENDED_LIMIT_INFORMATION`（x64 下 144 字节，与主干 `:441-443` 的注释同值）。
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct JobObjectExtendedLimitInformation {
    basic_limit_information: JobObjectBasicLimitInformation,
    io_info: IoCounters,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
}

/// `PROCESSENTRY32W`。`th32_default_heap_id` 是 `ULONG_PTR` ⇒ 后面那一串偏移全体移 4，
/// 所以这里必须是 `usize` 而不是 `u32`（主干那份 `IntPtr` 同理）。
#[repr(C)]
struct ProcessEntry32W {
    dw_size: u32,
    cnt_usage: u32,
    th32_process_id: u32,
    th32_default_heap_id: usize,
    th32_module_id: u32,
    cnt_threads: u32,
    th32_parent_process_id: u32,
    pc_pri_class_base: i32,
    dw_flags: u32,
    sz_exe_file: [u16; 260],
}

impl ProcessEntry32W {
    fn zeroed() -> Self {
        Self {
            dw_size: std::mem::size_of::<ProcessEntry32W>() as u32,
            cnt_usage: 0,
            th32_process_id: 0,
            th32_default_heap_id: 0,
            th32_module_id: 0,
            cnt_threads: 0,
            th32_parent_process_id: 0,
            pc_pri_class_base: 0,
            dw_flags: 0,
            sz_exe_file: [0; 260],
        }
    }

    /// `szExeFile` 是定长 UTF-16 缓冲区：截到第一个 NUL，转不动就当空名。
    fn exe_file(&self) -> String {
        let end = self
            .sz_exe_file
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(self.sz_exe_file.len());
        String::from_utf16_lossy(&self.sz_exe_file[..end])
    }
}

// ------------------------------------------------------------------ 纯判定
//
// 下面这些函数**一个 Win32 调用都没有**，全部只吃注入进来的数据：`cargo test` 测的就是它们，
// 所以整套判定逻辑不需要（也绝不允许）碰真 PID。

/// 快照里的一条进程记录（注入数据，测试直接手搓）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcInfo {
    pub pid: u32,
    pub parent_pid: u32,
    /// `PROCESSENTRY32W.szExeFile`，即不含目录的文件名（`"node.exe"`）。
    pub exe_file: String,
    /// `QueryFullProcessImageNameW` 的完整路径；`None` = 读不到（已退出/权限/32-64 位挡架）。
    pub image_path: Option<PathBuf>,
}

fn eq_ignore_case(left: &str, right: &str) -> bool {
    // ASCII 折叠：非 ASCII（中文目录名）大小写相同才判相同 ⇒ 只会「认不出同一份文件」而放它一马，
    // 不会反过来把两份不同文件认成一份。方向永远朝「不杀」。
    left.eq_ignore_ascii_case(right)
}

/// 去掉目录后的文件名（不含扩展名）再小写；`""` 表示拿不出可比的名字。
fn lower_stem(exe_file: &str) -> String {
    let name = exe_file.rsplit(['/', '\\']).next().unwrap_or(exe_file);
    let stem = match name.rfind('.') {
        Some(index) if index > 0 => &name[..index],
        _ => name,
    };
    stem.to_ascii_lowercase()
}

/// 不做 IO 的字面归一：分隔符统一成 `\`、吃掉 `.`、去掉末尾分隔符；
/// 含 `..` 或根本不是绝对路径 ⇒ `None`（**证明不了**是同一个文件，就当作不是）。
fn absolute_lexical(path: &Path) -> Option<String> {
    let mut out = String::new();
    let mut rooted = false;
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                out.push_str(&prefix.as_os_str().to_string_lossy());
                out.push('\\');
                rooted = true;
            }
            Component::RootDir => {
                out.push('\\');
                rooted = true;
            }
            Component::CurDir => {}
            Component::ParentDir => return None,
            Component::Normal(part) => {
                out.push_str(&part.to_string_lossy());
                out.push('\\');
            }
        }
    }
    if !rooted {
        return None;
    }
    while out.ends_with('\\') && out != "\\" {
        out.pop();
    }
    Some(out)
}

fn canonical_text(path: &Path) -> Option<String> {
    std::fs::canonicalize(path)
        .ok()
        .map(|full| full.to_string_lossy().into_owned())
}

/// 「这两个路径是不是同一个可执行文件」——主干 `:288-291` 那句
/// `Path.GetFullPath` + `TrimEndingDirectorySeparator` + `OrdinalIgnoreCase` 的加强版：
/// 两边都能 `canonicalize`（解符号链接/junction/8.3，顺带要求文件真实存在）就比规范形，
/// 否则退回**必须是绝对路径**的字面比较；比不出等号一律 `false`。
fn same_program_file(candidate: &Path, own: &Path) -> bool {
    if let (Some(left), Some(right)) = (canonical_text(candidate), canonical_text(own)) {
        return eq_ignore_case(&left, &right);
    }
    match (absolute_lexical(candidate), absolute_lexical(own)) {
        (Some(left), Some(right)) => eq_ignore_case(&left, &right),
        _ => false,
    }
}

/// **条件一**：候选的模块完整路径正是本安装自带的内核。
/// `candidate == None`（读不到模块路径）**必须**是 `false` —— 主干 `:287` 那个
/// `catch { continue; }` 的同一口径：身份确认不了的进程一个都不许碰。
pub fn is_same_install_kernel(candidate: Option<&Path>, own_kernel_path: &Path) -> bool {
    let Some(candidate) = candidate else {
        return false;
    };
    same_program_file(candidate, own_kernel_path)
}

/// owner 判据的名字那一半：候选进程名是分叉自身 exe 的文件名，**或**是主干的 `Blade2`。
/// 只用于「加保护」，绝不参与「该杀」的判定。
pub fn is_app_owner_name(exe_file: &str, own_exe_file: &str) -> bool {
    let stem = lower_stem(exe_file);
    if stem.is_empty() {
        return false;
    }
    stem == MAINLINE_APP_STEM || eq_ignore_case(&stem, &lower_stem(own_exe_file))
}

/// 子→父关系表（主干 `SnapshotProcessParents()` `:316` 的纯函数替身）。
pub fn parent_map(procs: &[ProcInfo]) -> HashMap<u32, u32> {
    let mut map = HashMap::with_capacity(procs.len());
    for proc in procs {
        map.insert(proc.pid, proc.parent_pid);
    }
    map
}

/// 活着的「本程序」PID：路径正是分叉自身 exe，**或**进程名对上分叉/主干的 exe 文件名。
/// 主干只有 `IsAppProcess`（`:365`，按名字 `Blade2`）；这里路径为主、名字兜底，
/// 三条命中全都朝「别杀」的方向偏。
pub fn owner_pids(procs: &[ProcInfo], own_exe_path: &Path) -> HashSet<u32> {
    let own_file = own_exe_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut owners = HashSet::new();
    for proc in procs {
        let path_hit = is_same_install_kernel(proc.image_path.as_deref(), own_exe_path);
        let name_hit = is_app_owner_name(&proc.exe_file, &own_file);
        if path_hit || name_hit {
            owners.insert(proc.pid);
        }
    }
    owners
}

/// 候选的整条祖先链（父、祖父……），主干 `HasLiveAppAncestor` 的循环体：
/// 深度封顶 [`MAX_ANCESTOR_DEPTH`]、`pid == 0`（System Idle）即停、见过就停（防 PID 回环）。
/// 父不在表里（父已退出）⇒ 链在这里断掉 ⇒ 返回**已走到的那一段**，可能为空。
pub fn ancestor_chain(
    pid: u32,
    parents: &HashMap<u32, u32>,
    max_depth: usize,
) -> Vec<u32> {
    let mut chain = Vec::new();
    let mut visited: HashSet<u32> = HashSet::new();
    let mut current = pid;
    visited.insert(current);
    for _ in 0..max_depth {
        let Some(&parent) = parents.get(&current) else {
            break;
        };
        if parent == 0 || parent == current || !visited.insert(parent) {
            break;
        }
        chain.push(parent);
        current = parent;
    }
    chain
}

/// **条件二**的第一半：祖先链上有没有活着的本程序。**空链**（父进程已经没了 = 崩溃残留的签名）
/// ⇒ `false` ⇒ 才可能进入命中。
pub fn has_live_owner(
    mut candidate_parents: impl Iterator<Item = u32>,
    owners: &HashSet<u32>,
) -> bool {
    candidate_parents.any(|pid| owners.contains(&pid))
}

/// 两条硬条件的**合取**：`sweep_orphan_kernels` 的循环体整条搬到这里，好让「缺一不杀」可测。
/// `owners` 为空时一个都不选：连一个活着的本程序都指认不出来，说明这次枚举不可信（权限不足/
/// 快照被截），这时候宁可不杀。`self_pid` 永不出现在结果里。
pub fn select_orphans(
    procs: &[ProcInfo],
    own_kernel_path: &Path,
    parents: &HashMap<u32, u32>,
    owners: &HashSet<u32>,
    self_pid: u32,
) -> Vec<u32> {
    if owners.is_empty() {
        return Vec::new();
    }
    let mut killed = Vec::new();
    for proc in procs {
        if proc.pid == self_pid || proc.pid == 0 {
            continue;
        }
        if !is_same_install_kernel(proc.image_path.as_deref(), own_kernel_path) {
            continue; // 条件一不过：路径对不上本安装内核，绝不动（别的 node 全在这一档被挡掉）
        }
        if has_live_owner(ancestor_chain(proc.pid, parents, MAX_ANCESTOR_DEPTH).into_iter(), owners)
        {
            continue; // 条件二不过：还有活着的本程序在撑它，不是孤儿
        }
        killed.push(proc.pid);
    }
    killed
}

/// **租约忙**的判据：主干 `IsSessionLeaseBusy`（`:240-243`）逐字对应，两个子串都认——
/// `SessionAlreadyOwnedError`（错误类型名，内核 `@deepseek-ai/dsh-session-persistence` 里就在抛）
/// 与 `already owned by an active write handle`（消息正文）。
pub fn is_lease_busy_error(text: &str) -> bool {
    !text.is_empty()
        && (text.contains("SessionAlreadyOwnedError")
            || text.contains("already owned by an active write handle"))
}

// ------------------------------------------------------------------ 作业对象

/// 一个「壳亡即杀」作业对象。句柄为 null = 这层保底不存在（调用方回落到 `child.kill()`）。
///
/// 只由持有 `Kernel` 的那条线程摆弄（分叉的 `Kernel` 一直待在 `Arc<Mutex<Kernel>>` 里），
/// 所以 `Send` 声明的是「跨线程传的是那把锁的所有权」，句柄本身不会有两个线程同时碰。
pub struct KillJob {
    handle: Handle,
    armed: bool,
    /// 一行诊断，进 `Kernel::log` ⇒ 最终从 `main.rs` 的 `push_log` 变成 stdout 的 `DIAG:` 行。
    pub note: String,
}

unsafe impl Send for KillJob {}

impl KillJob {
    /// 一个都不做的空实例（错误分支与测试用）。
    pub fn inactive(note: impl Into<String>) -> Self {
        Self {
            handle: NULL_HANDLE,
            armed: false,
            note: note.into(),
        }
    }

    /// 主干 `AttachToKillJob()`（`:170-233`）的对应物：建 → 设 `KILL_ON_JOB_CLOSE` → 指派 →
    /// **回头验**（`:213` 那句「套没上必须回头看：这类静默失败不抛异常」）。
    /// 任何一步失败都只是「这层保底没有」，句柄就地关掉，绝不 panic。
    pub fn attach(process_handle: Handle) -> Self {
        match panic::catch_unwind(AssertUnwindSafe(|| attach_inner(process_handle))) {
            Ok(job) => job,
            Err(_) => KillJob::inactive("作业对象走了非预期分支：已放弃，内核回收退化为 child.kill()"),
        }
    }

    /// 作业是否真的挂上了（`IsProcessInJob` 复查过才算）。
    pub fn is_armed(&self) -> bool {
        self.armed
    }

    /// 主干 `Dispose` 的 `finally`（`:152-160`）：**关掉最后一个句柄就是触发点**。
    /// 关掉之后 `Drop` 不会再关一次（`handle` 置 null）。
    pub fn close_now(&mut self) {
        if !self.handle.is_null() {
            let closed = unsafe { CloseHandle(self.handle) };
            if closed == 0 {
                self.note = format!(
                    "{}；作业句柄关闭失败({})，孤儿将由下次启动清扫兜底",
                    self.note,
                    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
                );
            }
            self.handle = NULL_HANDLE;
        }
        self.armed = false;
    }
}

impl Drop for KillJob {
    fn drop(&mut self) {
        self.close_now();
    }
}

fn last_error() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

fn attach_inner(process_handle: Handle) -> KillJob {
    if process_handle.is_null() {
        return KillJob::inactive("拿不到内核进程句柄：跳过作业对象，回收退化为 child.kill()");
    }
    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if job.is_null() {
        return KillJob::inactive(format!(
            "CreateJobObjectW 失败({})：跳过作业对象，回收退化为 child.kill()",
            last_error()
        ));
    }
    let mut info = JobObjectExtendedLimitInformation::default();
    info.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let length = std::mem::size_of::<JobObjectExtendedLimitInformation>() as u32;
    let set_ok = unsafe {
        SetInformationJobObject(
            job,
            JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
            &info as *const JobObjectExtendedLimitInformation as *const c_void,
            length,
        )
    };
    if set_ok == 0 {
        unsafe { CloseHandle(job) };
        return KillJob::inactive(format!(
            "SetInformationJobObject(KILL_ON_JOB_CLOSE,{length}B) 失败({})：作业未挂载",
            last_error()
        ));
    }
    let assigned = unsafe { AssignProcessToJobObject(job, process_handle) };
    if assigned == 0 {
        unsafe { CloseHandle(job) };
        return KillJob::inactive(format!(
            "AssignProcessToJobObject 失败({})：多半是本壳已经跑在别的作业对象里（终端/沙箱宿主），\
             子进程会继承那个作业；作业保底缺失，内核回收退化为 child.kill() + 启动清扫",
            last_error()
        ));
    }
    // 主干 `:213`：`IsProcessInJob` 复查。指派「返回 TRUE」不等于套得住，只有实机杀壳才暴露得出来。
    let mut in_job: WinBool = 0;
    let verified = unsafe { IsProcessInJob(process_handle, job, &mut in_job) };
    if verified == 0 || in_job == 0 {
        unsafe { CloseHandle(job) };
        return KillJob::inactive(format!(
            "IsProcessInJob 复查未通过(code={},in_job={})：作业已撤销，靠启动清扫兜底",
            last_error(),
            in_job
        ));
    }
    KillJob {
        handle: job,
        armed: true,
        note: format!(
            "作业对象已挂载并复查（KILL_ON_JOB_CLOSE, {length}B 扩展限额结构）：壳退出即带走内核整棵树"
        ),
    }
}

// ------------------------------------------------------------------ 进程枚举 / 收尾

/// 一次 toolhelp 快照（主干 `SnapshotProcessParents()` `:316-343` 的对应物）：
/// 只回 pid / 父 pid / 文件名，**不**读路径（路径要逐个 `OpenProcess`，只给候选读）。
fn snapshot_entries() -> Vec<(u32, u32, String)> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot.is_null() || snapshot == INVALID_SNAPSHOT {
        return Vec::new();
    }
    let mut rows = Vec::new();
    let mut entry = ProcessEntry32W::zeroed();
    let mut more = unsafe { Process32FirstW(snapshot, &mut entry) };
    while more != 0 {
        rows.push((
            entry.th32_process_id,
            entry.th32_parent_process_id,
            entry.exe_file(),
        ));
        more = unsafe { Process32NextW(snapshot, &mut entry) };
    }
    unsafe { CloseHandle(snapshot) };
    rows
}

/// 候选进程的模块完整路径（主干 `proc.MainModule?.FileName` `:282` 的对应物）。
/// 打不开、读不出、长度不对 ⇒ `None` = **不是**命中。
fn image_path_of(pid: u32) -> Option<PathBuf> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let mut buffer = vec![0u16; IMAGE_PATH_CHARS];
    let mut chars = IMAGE_PATH_CHARS as u32;
    let ok = unsafe {
        QueryFullProcessImageNameW(
            handle,
            0,
            buffer.as_mut_ptr(),
            &mut chars as *mut u32,
        )
    };
    unsafe { CloseHandle(handle) };
    let len = usize::try_from(chars).unwrap_or(usize::MAX);
    if ok == 0 || len == 0 || len > buffer.len() {
        return None;
    }
    Some(PathBuf::from(String::from_utf16_lossy(&buffer[..len])))
}

/// 主干 `proc.Kill(entireProcessTree: true)` 里**只针对这一个 PID** 的那半截
/// （整棵树那半截刻意不抄：见模块头的「比主干窄」）。
fn terminate(pid: u32) -> bool {
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
    if handle.is_null() {
        return false;
    }
    let ok = unsafe { TerminateProcess(handle, 1) };
    unsafe { CloseHandle(handle) };
    ok != 0
}

/// 一次清扫的结果（`note` 是给 `DIAG:` 行看的一行话）。
#[derive(Clone, Debug, Default)]
pub struct SweepOutcome {
    /// 实际杀掉（`TerminateProcess` 返回 TRUE）的内核主进程 PID。
    pub killed: Vec<u32>,
    /// 扫过的进程数。
    pub scanned: usize,
    /// 认出来的「活着的本程序」数（条件二的分母）。
    pub owners: usize,
    pub note: String,
}

impl SweepOutcome {
    /// 主干 `SweepOrphanKernels()` 的返回值口径：杀掉的主进程数。
    pub fn killed_count(&self) -> usize {
        self.killed.len()
    }
}

/// **启动清扫**（主干 `:253`，调用点在 `MainWindow.xaml.cs:2499`，用户可见效果只有 `:2501`
/// 那一行 `Debug.WriteLine`）。判定见模块头：**两条硬条件缺一不杀**，
/// 且两条都比主干的 `node` 名字匹配窄。带外异常（枚举/快照失败）一律不许冒到调用点——
/// 调用点可能是异常筛选器（主干 `:259-263` 同一个理由）。
pub fn sweep_orphan_kernels() -> SweepOutcome {
    match panic::catch_unwind(AssertUnwindSafe(sweep_inner)) {
        Ok(outcome) => outcome,
        Err(_) => SweepOutcome {
            note: "内核孤儿清扫: 走了非预期分支，本轮一个都没碰".to_string(),
            ..SweepOutcome::default()
        },
    }
}

fn sweep_inner() -> SweepOutcome {
    // 判定目标只认「本安装自带、且真实存在」的那颗 Kernel\node.exe（主干 `BundledNode` `:27-30`
    // 的 `File.Exists` 同口径），并且**不**吃 BLADE2_KERNEL_DIR / BLADE2_KERNEL_EXE。
    let Some(own_kernel) = kernel::bundled_kernel_exe() else {
        return SweepOutcome {
            note: "内核孤儿清扫: 跳过（本安装没有自带 Kernel\\node.exe）".to_string(),
            ..SweepOutcome::default()
        };
    };
    let Ok(own_exe) = std::env::current_exe() else {
        return SweepOutcome {
            note: "内核孤儿清扫: 跳过（拿不到自身路径，认不出哪些进程属于本程序）".to_string(),
            ..SweepOutcome::default()
        };
    };
    let rows = snapshot_entries();
    if rows.is_empty() {
        return SweepOutcome {
            note: "内核孤儿清扫: 跳过（toolhelp 快照为空）".to_string(),
            ..SweepOutcome::default()
        };
    }
    let procs: Vec<ProcInfo> = rows
        .into_iter()
        .map(|(pid, parent_pid, exe_file)| {
            // 只对**名字对得上内核**的进程去开句柄读路径：别的进程一次 `OpenProcess` 都不做。
            let gate = kernel_like_file_name(&exe_file);
            ProcInfo {
                pid,
                parent_pid,
                image_path: gate.then(|| image_path_of(pid)).flatten(),
                exe_file,
            }
        })
        .collect();
    let parents = parent_map(&procs);
    let owners = owner_pids(&procs, &own_exe);
    let self_pid = std::process::id();
    let scanned = procs.len();
    let owner_count = owners.len();
    let targets = select_orphans(&procs, &own_kernel, &parents, &owners, self_pid);
    let mut killed = Vec::new();
    for pid in targets {
        // 抢不动的（正在退出/权限不足）跳过：下一次启动还会再扫（主干 `:305` 同一口径）。
        if terminate(pid) {
            killed.push(pid);
        }
    }
    let note = sweep_note(&killed, scanned, owner_count, &own_kernel);
    SweepOutcome {
        killed,
        scanned,
        owners: owner_count,
        note,
    }
}

/// 那一行 `DIAG:` 的文案（纯函数，好让「杀了 0 个也要看得出扫过」这一条离线可测）。
fn sweep_note(killed: &[u32], scanned: usize, owners: usize, target: &Path) -> String {
    if killed.is_empty() {
        return format!(
            "内核孤儿清扫: 命中 0（扫描 {scanned} 个进程、认活壳 {owners} 个，目标 {}）",
            target.display()
        );
    }
    format!(
        "内核孤儿清扫: 收掉异常退出残留的内核进程 {} 个（{killed:?}，租约随进程死亡释放）",
        killed.len()
    )
}

/// 名字闸门：**只有**可能叫 `node.exe` 的进程才值得为它开一次进程句柄。
/// 这不是判定条件（判定只在 `select_orphans` 里那两条），纯粹省掉全机器几发 `OpenProcess`；
/// 它只会少读路径 ⇒ 只会少杀。
fn kernel_like_file_name(exe_file: &str) -> bool {
    matches!(lower_stem(exe_file).as_str(), "node")
}

// ------------------------------------------------------------------ 测试
//
// 这一批**全部只吃注入的数据**：没有任何一发用例枚举真进程、读真 PID、开真句柄，
// 更没有 TerminateProcess —— 这台机器上活着的 node 进程里有主线的内核和 agent 工具链自己，
// 跑一次真清扫就是事故。`cargo test` 验的是判定，Win32 那半截只能等实机。

#[cfg(test)]
mod tests {
    use super::*;

    /// 本安装自带的内核（判定条件一的目标）。用一个**保证不存在**的绝对路径：
    /// `same_program_file` 的 `canonicalize` 两边都失败 ⇒ 走字面归一那一支，用例因此完全离线、
    /// 也不会因为机器上真有个 `C:\Install` 而变味。
    fn own_kernel() -> PathBuf {
        PathBuf::from("C:\\blade2-task68-missing-install\\Kernel\\node.exe")
    }

    fn own_exe() -> PathBuf {
        PathBuf::from("C:\\blade2-task68-missing-install\\blade2-rs.exe")
    }

    fn proc(pid: u32, parent_pid: u32, exe_file: &str, image: Option<&str>) -> ProcInfo {
        ProcInfo {
            pid,
            parent_pid,
            exe_file: exe_file.to_string(),
            image_path: image.map(PathBuf::from),
        }
    }

    // ---------------------------------------------------------- 条件一：路径

    /// 同一个文件、大小写不同 ⇒ 命中（主干 `StringComparison.OrdinalIgnoreCase` 同口径）；
    /// 换盘符 / 换目录 / 只是前缀 ⇒ 一律不命中。**关键**：名字同为 `node.exe` 也不够，
    /// 主干那句 `Process.GetProcessesByName("node")` 在这一发上直接不成立。
    #[test]
    fn kernel_path_match_folds_case_but_never_folds_directory() {
        let own = own_kernel();
        assert!(
            is_same_install_kernel(
                Some(Path::new("c:\\blade2-TASK68-MISSING-INSTALL\\kernel\\NODE.EXE")),
                &own
            ),
            "大小写不同仍是同一份文件"
        );
        assert!(
            is_same_install_kernel(Some(&own.clone()), &own),
            "逐字相同当然命中"
        );
        assert!(
            !is_same_install_kernel(
                Some(Path::new("D:\\blade2-task68-missing-install\\Kernel\\node.exe")),
                &own
            ),
            "换了盘符就是另一份文件"
        );
        assert!(
            !is_same_install_kernel(Some(Path::new("C:\\blade2-task68-missing-install\\Kernel2\\node.exe")), &own),
            "换了目录就是另一份文件"
        );
        assert!(
            !is_same_install_kernel(
                Some(Path::new("C:\\blade2-task68-missing-install\\Kernel\\node.exe.bak")),
                &own
            ),
            "同前缀的另一个文件不许命中"
        );
        assert!(
            !is_same_install_kernel(Some(Path::new("C:\\Users\\me\\tools\\node.exe")), &own),
            "用户自己的 node.exe（agent 工具链就靠它）绝不许命中"
        );
    }

    /// 读不到模块路径 ⇒ **不是**命中（主干 `:287` 那个 `catch { continue; }`：
    /// 「无法确认身份就不碰」）。这一条是整个模块最要紧的负例，所以单独钉一发。
    #[test]
    fn unreadable_module_path_is_not_a_match() {
        assert!(!is_same_install_kernel(None, &own_kernel()));
    }

    /// 相对路径、含 `..` 的路径 ⇒ 证明不了是同一个文件，一律不命中（宁少杀不多杀）。
    #[test]
    fn relative_and_dotdot_paths_are_never_a_match() {
        let own = own_kernel();
        assert!(!is_same_install_kernel(Some(Path::new("Kernel/node.exe")), &own));
        assert!(
            !is_same_install_kernel(
                Some(Path::new("C:\\blade2-task68-missing-install\\Kernel\\..\\Kernel\\node.exe")),
                &own
            ),
            "带 .. 的字面路径不当作同一份文件"
        );
    }

    /// 同一份**真实存在**的文件，换一种拼法（多一层 `.`、大小写换掉）仍然命中：
    /// 这一发走的是 `canonicalize` 那一支（两边都是磁盘上真实的文件 ⇒ 解链接后比规范形）。
    #[test]
    fn same_existing_file_written_differently_still_matches() {
        let exe = std::env::current_exe().expect("测试进程的自身路径总是拿得到");
        assert!(is_same_install_kernel(Some(&exe), &exe), "同一个路径必判相同");
        let upper = PathBuf::from(exe.to_string_lossy().to_ascii_uppercase());
        assert!(
            is_same_install_kernel(Some(&upper), &exe),
            "全大写的同一份文件要认出来（Windows 路径大小写无关）"
        );
        let dir = exe.parent().expect("exe 总有父目录");
        let detoured = dir.join(".").join(exe.file_name().expect("exe 总有文件名"));
        assert!(
            is_same_install_kernel(Some(&detoured), &exe),
            "多写一层 `.` 的同一份文件要认出来"
        );
        // 反向：两份都真实存在、但不是同一个文件 ⇒ 不命中。
        assert!(
            !is_same_install_kernel(Some(&dir.join("definitely-not-this-exe.exe")), &exe),
            "目录相同文件名不同 ⇒ 不是同一份"
        );
    }

    // ---------------------------------------------------------- 条件二：活着的本程序

    /// 空祖先链（父进程已经没了 = 崩溃残留的签名）⇒ 没有活 owner。
    /// 这一发同时钉住 `ancestor_chain`：父不在表里就直接断，不会退化成「一路往上乱走」。
    #[test]
    fn empty_ancestor_chain_has_no_live_owner() {
        let mut owners = HashSet::new();
        owners.insert(7u32);
        assert!(!has_live_owner(std::iter::empty(), &owners));
        let parents: HashMap<u32, u32> = HashMap::new();
        assert!(ancestor_chain(4242, &parents, MAX_ANCESTOR_DEPTH).is_empty());
        assert!(!has_live_owner(
            ancestor_chain(4242, &parents, MAX_ANCESTOR_DEPTH).into_iter(),
            &owners
        ));
    }

    /// 父链一路走到活 owner ⇒ 有 owner；深度封顶 [`MAX_ANCESTOR_DEPTH`]（主干 `:350` 的 16），
    /// 且 PID 回环不会把这里转死。
    #[test]
    fn ancestor_chain_walks_caps_and_terminates() {
        let mut parents: HashMap<u32, u32> = HashMap::new();
        for pid in 1..40u32 {
            parents.insert(pid, pid + 1);
        }
        let chain = ancestor_chain(1, &parents, MAX_ANCESTOR_DEPTH);
        assert_eq!(chain.len(), MAX_ANCESTOR_DEPTH, "祖先链只往上走 16 层");
        assert_eq!(chain.first().copied(), Some(2));
        assert_eq!(chain.last().copied(), Some(17));

        // 回环：1→2→1。第二圈撞见见过的 pid 就停。
        let mut looped: HashMap<u32, u32> = HashMap::new();
        looped.insert(1, 2);
        looped.insert(2, 1);
        assert_eq!(ancestor_chain(1, &looped, MAX_ANCESTOR_DEPTH), vec![2]);

        // 自父 / System Idle(0) 都当场停。
        let mut self_parent: HashMap<u32, u32> = HashMap::new();
        self_parent.insert(5, 5);
        assert!(ancestor_chain(5, &self_parent, MAX_ANCESTOR_DEPTH).is_empty());
        let mut idle: HashMap<u32, u32> = HashMap::new();
        idle.insert(9, 0);
        assert!(ancestor_chain(9, &idle, MAX_ANCESTOR_DEPTH).is_empty());
    }

    /// owner 判据：路径正是分叉自身 exe ⇒ 是；进程名是 `Blade2`（主干）或 `blade2-rs` ⇒ 也算；
    /// 别的 node / powershell ⇒ 不算。三条命中**全部**只朝「保护自己的内核」方向偏。
    #[test]
    fn owner_names_are_the_fork_plus_the_mainline_app() {
        assert!(is_app_owner_name("Blade2.exe", "blade2-rs.exe"), "主干的活进程算 owner");
        assert!(
            is_app_owner_name("BLADE2-RS.EXE", "blade2-rs.exe"),
            "分叉自己的活进程算 owner"
        );
        assert!(!is_app_owner_name("node.exe", "blade2-rs.exe"));
        assert!(!is_app_owner_name("powershell.exe", "blade2-rs.exe"));
        assert!(!is_app_owner_name("", "blade2-rs.exe"));

        let procs = vec![
            proc(10, 4, "blade2-rs.exe", Some("C:\\blade2-task68-missing-install\\blade2-rs.exe")),
            proc(11, 4, "blade2-rs.exe", Some("D:\\Other\\copy\\blade2-rs.exe")),
            proc(12, 4, "Blade2.exe", Some("C:\\Windows\\Installer\\Blade2.exe")),
            proc(13, 10, "node.exe", Some("C:\\blade2-task68-missing-install\\Kernel\\node.exe")),
            proc(14, 11, "node.exe", Some("C:\\Users\\me\\tools\\node.exe")),
        ];
        let owners = owner_pids(&procs, &own_exe());
        assert_eq!(
            owners,
            HashSet::from([10u32, 11, 12]),
            "只有本程序/主干那三类算 owner；node 一律不算"
        );
    }

    // ---------------------------------------------------------- 两条的合取

    /// 「缺一不杀」的整条判据。这台机器上真会同时存在这五种进程，五种都要钉住：
    /// ①本壳正在用的内核、②兄弟分叉实例正在用的内核、③**主干**正在用的内核
    /// （父进程是 `Blade2.exe`，路径与分叉自带的完全一样 —— 开发态两家共用 `Kernel\node.exe`，
    /// 这条就是「绝不动 QA 那台 GUI」的保险丝）、④用户自己的 node、⑤真正的崩溃残留。
    #[test]
    fn only_the_two_condition_hit_is_selected_and_live_ones_are_spared() {
        let own = own_kernel();
        let self_pid = 10u32;
        let procs = vec![
            proc(10, 4, "blade2-rs.exe", Some("C:\\blade2-task68-missing-install\\blade2-rs.exe")),
            proc(11, 4, "blade2-rs.exe", Some("D:\\Other\\copy\\blade2-rs.exe")),
            proc(12, 4, "Blade2.exe", Some("C:\\Windows\\Installer\\Blade2.exe")),
            proc(13, 10, "node.exe", Some("C:\\blade2-task68-missing-install\\Kernel\\node.exe")), // ①
            proc(14, 11, "node.exe", Some("C:\\blade2-task68-missing-install\\Kernel\\node.exe")), // ②
            proc(15, 12, "node.exe", Some("C:\\blade2-task68-missing-install\\Kernel\\node.exe")), // ③
            proc(16, 13, "node.exe", Some("C:\\blade2-task68-missing-install\\Kernel\\node.exe")), // ①的子进程（MCP）
            proc(17, 99, "node.exe", Some("C:\\Users\\me\\tools\\node.exe")), // ④
            proc(18, 99, "node.exe", None), // ④' 读不到路径 ⇒ 身份不明 ⇒ 不碰
            proc(19, 999, "node.exe", Some("C:\\blade2-task68-missing-install\\Kernel\\node.exe")), // ⑤ 残留
            proc(20, 4, "node.exe", Some("C:\\blade2-task68-missing-install\\Kernel\\node.exe")), // ⑤' 父在表里但不是 owner
        ];
        let parents = parent_map(&procs);
        let owners = owner_pids(&procs, &own_exe());
        let picked = select_orphans(&procs, &own, &parents, &owners, self_pid);
        assert_eq!(
            picked,
            vec![19, 20],
            "只有「路径命中 + 祖先链上没有活着的本程序」那两条同时成立的才被选中"
        );
        assert!(!picked.contains(&self_pid));
    }

    /// owner 集为空（快照不可信 / 权限不足 / 只抓到一半）⇒ **一个都不选**。
    /// 这条是「枚举坏了就歇手」的总闸：条件二在这时候证明不了任何东西。
    #[test]
    fn empty_owner_set_disables_the_whole_sweep() {
        let own = own_kernel();
        let procs = vec![proc(19, 999, "node.exe", Some("C:\\blade2-task68-missing-install\\Kernel\\node.exe"))];
        let parents = parent_map(&procs);
        assert!(select_orphans(&procs, &own, &parents, &HashSet::new(), 1).is_empty());
    }

    /// 名字闸门：只有可能叫 `node.exe` 的进程才配开一次句柄。
    /// （它**不是**判定条件，只是少读路径 ⇒ 只会少杀，不会多杀。）
    #[test]
    fn name_gate_only_admits_node_and_is_not_a_kill_condition() {
        assert!(kernel_like_file_name("node.exe"));
        assert!(kernel_like_file_name("NODE.EXE"));
        assert!(
            kernel_like_file_name("node"),
            "闸门只看文件名（不含扩展名）；它不是判定条件 —— 两条硬条件里根本没有名字这一项"
        );
        assert!(!kernel_like_file_name("Blade2.exe"));
        assert!(!kernel_like_file_name("nodemon.exe"), "名字相近也不算");
        assert!(!kernel_like_file_name(""));
    }

    // ---------------------------------------------------------- 租约忙的判据

    /// 主干 `IsSessionLeaseBusy`（`DshKernelHost.cs:240-243`）两个子串逐字对应，
    /// 且**大小写敏感**（主干那两处是 `StringComparison.Ordinal`）：
    /// 认不出来的错误一律不许触发「扫进程 + 重发」。
    #[test]
    fn lease_busy_error_matches_exactly_the_two_mainline_substrings() {
        // 主干注释里的那句原文（`:236`）。
        let sample = "SessionAlreadyOwnedError: session \"s-1\" is already owned by an active write handle";
        assert!(is_lease_busy_error(sample));
        assert!(is_lease_busy_error("SessionAlreadyOwnedError"));
        assert!(
            is_lease_busy_error("resume failed: already owned by an active write handle"),
            "只冒出来消息正文那一半也要认"
        );
        // 分叉侧的实际形状：`Kernel::call` 拼的是 `"{code}: {message}"`（kernel.rs）。
        assert!(is_lease_busy_error(
            "session/prompt_failed: SessionAlreadyOwnedError: session \"s\" is already owned by an active write handle"
        ));
        assert!(!is_lease_busy_error(""));
        assert!(!is_lease_busy_error("gateway/lookup-not-found: 会话不存在"));
        assert!(!is_lease_busy_error("HTTP 500: internal error"));
        assert!(
            !is_lease_busy_error("already owned by AN ACTIVE WRITE HANDLE"),
            "主干按 Ordinal 比，这里不许放宽成大小写无关"
        );
    }

    // ---------------------------------------------------------- 结构体布局

    /// 布局只钉 64 位（分叉的唯一目标平台）；32 位上这些常量本来就不该过。
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn job_object_struct_layout_matches_the_win32_abi() {
        assert_eq!(
            std::mem::size_of::<JobObjectExtendedLimitInformation>(),
            144,
            "x64 上 JOBOBJECT_EXTENDED_LIMIT_INFORMATION 是 144 字节"
        );
        assert_eq!(
            std::mem::offset_of!(JobObjectExtendedLimitInformation, basic_limit_information),
            0
        );
        assert_eq!(
            std::mem::offset_of!(
                JobObjectExtendedLimitInformation,
                basic_limit_information
            ) + std::mem::offset_of!(JobObjectBasicLimitInformation, limit_flags),
            16
        );
        assert_eq!(std::mem::size_of::<ProcessEntry32W>(), 568);
        assert_eq!(
            std::mem::offset_of!(ProcessEntry32W, th32_parent_process_id),
            32,
            "父 PID 的偏移错了 = 整张祖先表都是歪的"
        );
        assert_eq!(std::mem::size_of::<ProcessEntry32W>() as u32, {
            ProcessEntry32W::zeroed().dw_size
        });
        assert_eq!(JOB_OBJECT_EXTENDED_LIMIT_INFORMATION, 9);
        assert_eq!(JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, 0x2000);
    }

    /// `attach(空句柄)` 不许 panic、也不许留下需要关闭的句柄：这就是「任何一发 Win32 失败
    /// 都退回今天那条 `child.kill()`」的落地形态（真句柄那一路只能实机验，见 tmp 报告）。
    #[test]
    fn attaching_without_a_handle_degrades_instead_of_panicking() {
        let job = KillJob::attach(NULL_HANDLE);
        assert!(!job.is_armed());
        assert!(!job.note.is_empty(), "失败原因要能写进 DIAG 行");
        assert!(job.handle.is_null());
        // 关两次：第二次必须是空操作（`Drop` 还会再调一次，「句柄只关一次」在这里钉住）。
        let mut job = job;
        job.close_now();
        job.close_now();
        assert!(!job.is_armed());
    }

    /// 清扫那一行诊断的文案：命中 0 也要看得出「扫过、目标是谁、认了几个活壳」，
    /// 命中 >0 要把 PID 报出来（主干 `MainWindow.xaml.cs:2501` 只有后半句那种信息）。
    ///
    /// **注意**：这里刻意**不**调 `sweep_orphan_kernels()` 本体 —— 那个函数会枚举全机进程并
    /// 发 `TerminateProcess`，而 `cargo test` 永远不许碰真 PID（本机此刻就活着主干的内核）。
    #[test]
    fn sweep_note_reports_both_the_noop_and_the_hit() {
        let target = own_kernel();
        let quiet = sweep_note(&[], 214, 3, &target);
        assert!(quiet.starts_with("内核孤儿清扫: 命中 0"), "{quiet}");
        assert!(quiet.contains("扫描 214 个进程"), "{quiet}");
        assert!(quiet.contains("认活壳 3 个"), "{quiet}");
        assert!(quiet.contains("node.exe"), "目标路径要出现在文案里：{quiet}");
        let hit = sweep_note(&[4242, 4243], 214, 3, &target);
        assert!(hit.contains("收掉异常退出残留的内核进程 2 个"), "{hit}");
        assert!(hit.contains("4242"), "{hit}");
    }
}
