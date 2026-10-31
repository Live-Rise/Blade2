//! 内核进程保底（task #68 起家）+ 常驻复用（0.9.0.4 移植）的进程侧原语。
//!
//! 主干那件保底是 `Dsh/DshKernelHost.cs` 在 0.8.1（`f35e6e3`）里长出来的，分叉这里对应两件事：
//!
//! · [`kill_process_tree`] —— 主干 `proc.Kill(entireProcessTree: true)` 的对应物（常驻改造后主干
//!   的 `RecycleResident` / `KillAndClear` / 启动超时三处都走它）。**task #68 时代的「壳亡即杀」
//!   作业对象（`KillJob`，`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`）已随常驻设计整体移除**：主干
//!   0.9.0.4 起壳退出必须**留下**内核（冷启 ~6.6s 的大头是 ~200 插件的模块装载风暴，进程级复用
//!   是唯一能到亚秒的路径；连接信息落 `DSH_HOME\blade2-resident-kernel.json`，见 `kernel.rs`），
//!   「句柄一关就带走整棵树」与常驻直接矛盾。杀内核的口只剩两个：引导超时
//!   （`kernel::Kernel::start` 内部）与重试清理（`kernel::Kernel::kill_and_clear`），两者都汇到
//!   [`kill_process_tree`]。内核持有每条会话目录的写租约、只有进程死亡才释放 —— 所以「杀」必须
//!   连它自己起的 MCP 子进程（server-memory / playwright 等）整棵树一起收，只杀主进程一样漏租约。
//!
//! · [`sweep_orphan_kernels`] / [`sweep_orphan_kernels_sparing`] —— 对应主干
//!   `SweepOrphanKernels(sparePid)`。无状态文件的崩溃残留仍由它兜底；**常驻内核的父进程是
//!   上一次的壳（已死），在清扫眼里就是孤儿** —— 会话中途的清扫调用必须走
//!   [`sweep_orphan_kernels_sparing`] 把当前内核 pid 点名豁免，否则常驻内核被误杀；引导期清扫
//!   发生在 attach 之后（要么已复用 ⇒ 不清扫，要么已回收 ⇒ 无需豁免），零参版够用。
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
//! 清扫还有一处比主干窄：主干对命中的进程下 `proc.Kill(entireProcessTree: true)`（`:300`），
//! 这里只对**自己两条判定都过了的那几个 PID** 发 `TerminateProcess`，不对整棵树广播。
//! （[`kill_process_tree`] 才是「整棵树」的那条路：它只吃调用方**点名**的 pid，先按记录路径核对
//! 身份再动手，与清扫的自动判定互不共用。）
//!
//! ## panic 口径
//!
//! 这里每个 `extern "system"` 边界都按「抛出去就是无声死进程」对待（分叉已经被 `0xC000027B` 那类
//! 静默死咬过；`keys.rs:22-26` 同一个教训）：不许 `unwrap()`/`expect()`，`BOOL` 一律按 0/非 0 判、
//! `HANDLE` 一律按 null/`INVALID_HANDLE_VALUE` 判，三个总入口 [`kill_process_tree`]、
//! [`sweep_orphan_kernels_sparing`]（零参版只是它的转发）与 [`process_image_path`] 外面再各套一层
//! [`std::panic::catch_unwind`]。任何一步失败都只体现在返回值/`note` 里 —— 少杀一个残留内核
//! 下次启动还会再扫，多杀一个进程是事故。

use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::c_void;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Component, Path, PathBuf};

use crate::kernel;

/// Win32 句柄：统一按 `*mut c_void` 处理，null = 无效。
pub type Handle = *mut c_void;
/// Win32 `BOOL` 的原生宽度是 4 字节 `int`，所以按 `i32` 收，**不**当 `bool` 用。
type WinBool = i32;

const INVALID_SNAPSHOT: Handle = usize::MAX as Handle;

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
///
/// 常驻复用（0.9.0.4 移植）后多了一类调用方：`kernel::try_attach_resident` 拿状态文件里记的
/// `exe` 对当前进程的模块路径做**逐字身份核对**（主干 `PathsEqual`，`DshKernelHost.cs:302-315`）
/// —— 防的是 pid 被无关进程复用后，凭一颗裸 pid 误杀无辜进程。
pub fn same_program_file(candidate: &Path, own: &Path) -> bool {
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
/// 快照被截），这时候宁可不杀。`self_pid`（清扫发起者自己）与 `spare_pid`（调用方点名豁免的
/// 常驻内核，0 = 不豁免）永不出现在结果里。
pub fn select_orphans(
    procs: &[ProcInfo],
    own_kernel_path: &Path,
    parents: &HashMap<u32, u32>,
    owners: &HashSet<u32>,
    self_pid: u32,
    spare_pid: u32,
) -> Vec<u32> {
    if owners.is_empty() {
        return Vec::new();
    }
    let mut killed = Vec::new();
    for proc in procs {
        if proc.pid == self_pid || proc.pid == spare_pid || proc.pid == 0 {
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

// ------------------------------------------------------------------ 杀树（常驻回收）

/// 主干 `proc.Kill(entireProcessTree: true)` 在常驻设计下的三个落点
/// （`Dsh/DshKernelHost.cs` 的 `RecycleResident` / `KillAndClear` / `StartAsync` 超时支）共用的
/// 那条腿：一次 toolhelp 快照里把 `pid` 的全部后代连同它自己一起 `TerminateProcess`。
/// 内核持有会话写租约、它自己起的 MCP 子进程（server-memory / playwright 等）各占各的 ——
/// 只杀主进程一样漏租约，所以「杀」必须是整棵树。
///
/// `expected_exe` 提供时**先核对身份再动手**（主干 `RecycleResident` `:265-288` 同一条纪律）：
/// 当前 `pid` 的模块路径与它对不上、或根本读不到，一律不动、返回 `false` —— 分叉没有
/// `Process` 句柄那种「进程对象保真」的机制，pid 复用到无关进程时全靠这一道核对不误杀。
/// 传 `None` 表示调用方已用别的方式担保了身份（例如手里就攥着该进程的
/// `std::process::Child` 句柄 —— `kernel::Kernel::start` 的超时支就是这种），才允许裸杀。
///
/// 返回值：根进程的 `TerminateProcess` 是否发出成功（后代各自尽力，抢不动的跳过）。
/// 进程已退出 ⇒ 读不到模块路径 ⇒ `false`（无可回收，主干 `catch { }` 同口径）。
pub fn kill_process_tree(pid: u32, expected_exe: Option<&Path>) -> bool {
    match panic::catch_unwind(AssertUnwindSafe(|| kill_tree_inner(pid, expected_exe))) {
        Ok(terminated) => terminated,
        Err(_) => false,
    }
}

fn kill_tree_inner(pid: u32, expected_exe: Option<&Path>) -> bool {
    if let Some(expected) = expected_exe {
        let Some(image) = image_path_of(pid) else {
            return false; // 已退出 / 读不到：身份证明不了就不碰
        };
        if !same_program_file(&image, expected) {
            return false; // pid 活着但不是那个内核（复用给了别的进程）：绝不误杀
        }
    }
    let rows = snapshot_entries();
    let parents: HashMap<u32, u32> = rows.iter().map(|(pid, parent, _)| (*pid, *parent)).collect();
    // 先杀根（停住它继续 spawn 的口），再按快照清单收后代 —— 快照是时点数据，
    // 根死后新出生的进程不在清单里，那类漏网由下次启动的清扫兜底（与主干同一取舍）。
    // 根那一发抢不动（正在退出/权限）时后代照收，返回值只报根。
    let terminated = terminate(pid);
    for descendant in descendants_of(pid, &parents) {
        terminate(descendant);
    }
    terminated
}

/// `pid` 的全部后代（子、孙……直到不在快照里），主干 `Kill(entireProcessTree)` 里
/// 「按父子边收集整棵树」那半截的纯函数替身。BFS + 已访问集，父子表里的回环走不死；
/// `pid` 自己不出现在结果里。
pub fn descendants_of(pid: u32, parents: &HashMap<u32, u32>) -> Vec<u32> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for (child, parent) in parents {
        children.entry(*parent).or_default().push(*child);
    }
    let mut out = Vec::new();
    let mut visited: HashSet<u32> = HashSet::from([pid]);
    let mut queue: VecDeque<u32> = VecDeque::from([pid]);
    while let Some(current) = queue.pop_front() {
        let Some(list) = children.get(&current) else {
            continue;
        };
        for child in list {
            if visited.insert(*child) {
                out.push(*child);
                queue.push_back(*child);
            }
        }
    }
    out
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

/// 候选进程的模块完整路径（主干 `proc.MainModule?.FileName` `:282` / `TryGetProcessPath`
/// `:290-300` 的对应物）。打不开、读不出、长度不对 ⇒ `None` —— 调用方各自的口径：
/// 清扫里 = 不是命中；常驻 attach 里 = pid 已死（主干 `Process.GetProcessById` 抛 `ArgumentException`
/// 那支与路径读不出那支在「只清状态、不杀」上同归，见 `kernel::try_attach_resident_with`）。
pub fn process_image_path(pid: u32) -> Option<PathBuf> {
    image_path_of(pid)
}

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

/// **启动清扫**（主干 `SweepOrphanKernels()`，零参调用点在 `main.rs` 引导与撞租约两处）。
/// 常驻设计下等价于 `sweep_orphan_kernels_sparing(0)` —— 引导期清扫发生在 attach 之后
/// （要么已复用 ⇒ 整段跳过，要么已回收 ⇒ 无需豁免），零参够用；**会话中途**的清扫调用
/// 必须走 [`sweep_orphan_kernels_sparing`] 把当前内核 pid 传进去，否则常驻内核（父进程是
/// 上一次已死的壳）会被当孤儿误杀。带外异常（枚举/快照失败）一律不许冒到调用点——
/// 调用点可能是异常筛选器（主干 `:456-458` 同一个理由）。
pub fn sweep_orphan_kernels() -> SweepOutcome {
    sweep_orphan_kernels_sparing(0)
}

/// [`sweep_orphan_kernels`] 的点名豁免版：`spare_pid` 是**本轮在用的常驻内核主进程 pid**
/// （主干 `SweepOrphanKernels(int sparePid = 0)` 的分叉对应物，`DshKernelHost.cs:448`）。
/// 其余判定一字不差 —— 豁免只是从候选表里划掉，不放松那两条硬条件。
pub fn sweep_orphan_kernels_sparing(spare_pid: u32) -> SweepOutcome {
    match panic::catch_unwind(AssertUnwindSafe(move || sweep_inner(spare_pid))) {
        Ok(outcome) => outcome,
        Err(_) => SweepOutcome {
            note: "内核孤儿清扫: 走了非预期分支，本轮一个都没碰".to_string(),
            ..SweepOutcome::default()
        },
    }
}

fn sweep_inner(spare_pid: u32) -> SweepOutcome {
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
    let targets = select_orphans(&procs, &own_kernel, &parents, &owners, self_pid, spare_pid);
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
        let picked = select_orphans(&procs, &own, &parents, &owners, self_pid, 0);
        assert_eq!(
            picked,
            vec![19, 20],
            "只有「路径命中 + 祖先链上没有活着的本程序」那两条同时成立的才被选中"
        );
        assert!(!picked.contains(&self_pid));
    }

    /// **常驻内核豁免**（主干 0.9.0.4 `SweepOrphanKernels(int sparePid)` 的移植）：常驻内核的
    /// 父进程是上一次的壳（已死），两条硬条件它全中 —— 不点名豁免就会被当孤儿收掉。
    /// `spare_pid = 0`（引导期零参版）时它照旧被选中：那份状态语义就是「没有豁免名额」。
    #[test]
    fn spare_pid_exempts_the_resident_kernel_and_zero_means_no_exemption() {
        let own = own_kernel();
        // pid 30 = 常驻内核：路径正是本安装内核，父进程 998 已死（上一次的壳）⇒ 两条全中。
        let procs = vec![
            proc(10, 4, "blade2-rs.exe", Some("C:\\blade2-task68-missing-install\\blade2-rs.exe")),
            proc(30, 998, "node.exe", Some("C:\\blade2-task68-missing-install\\Kernel\\node.exe")),
        ];
        let parents = parent_map(&procs);
        let owners = owner_pids(&procs, &own_exe());
        assert_eq!(
            select_orphans(&procs, &own, &parents, &owners, 10, 0),
            vec![30],
            "不豁免 ⇒ 常驻内核在清扫眼里就是孤儿（这正是必须传 spare_pid 的原因）"
        );
        assert!(
            select_orphans(&procs, &own, &parents, &owners, 10, 30).is_empty(),
            "点名豁免之后一个都不许选 —— 豁免只划掉候选，不放松两条硬条件之外的东西"
        );
    }

    /// owner 集为空（快照不可信 / 权限不足 / 只抓到一半）⇒ **一个都不选**。
    /// 这条是「枚举坏了就歇手」的总闸：条件二在这时候证明不了任何东西。
    #[test]
    fn empty_owner_set_disables_the_whole_sweep() {
        let own = own_kernel();
        let procs = vec![proc(19, 999, "node.exe", Some("C:\\blade2-task68-missing-install\\Kernel\\node.exe"))];
        let parents = parent_map(&procs);
        assert!(select_orphans(&procs, &own, &parents, &HashSet::new(), 1, 0).is_empty());
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

    // ---------------------------------------------------------- 杀树的纯函数半截
    //
    // `kill_process_tree` 的 Win32 那半截与清扫同一条纪律：`cargo test` 永远不许碰真 PID。
    // 这里钉的是「按父子边收集整棵树」那半截 —— 收漏了 = MCP 子进程占着租约活下来，
    // 收多了 = 把无辜进程带进终止清单。

    /// 树形、多叉、纵深全要收齐；`pid` 自己不在清单里；不在快照里的分支当场断掉。
    #[test]
    fn descendants_of_collects_the_whole_tree_without_the_root() {
        // 30(内核) → 31,32(双叉) → 33(31 的子) → 34(孙)；35 的父不在表里 ⇒ 断。
        let rows = [
            (31u32, 30u32),
            (32, 30),
            (33, 31),
            (34, 33),
            (35, 9999),
        ];
        let parents: HashMap<u32, u32> = rows.iter().copied().collect();
        let mut got = descendants_of(30, &parents);
        got.sort_unstable();
        assert_eq!(got, vec![31, 32, 33, 34], "四层全收，根自己除外");
        assert!(descendants_of(35, &parents).is_empty(), "父不在表里 ⇒ 没有后代可收");
        assert!(descendants_of(30, &HashMap::new()).is_empty());
    }

    /// 父子表里的回环（pid 复用后的陈旧快照会出现）不许把 BFS 转死，也不许重复收。
    #[test]
    fn descendants_of_terminates_on_cycles_without_duplicates() {
        let mut looped: HashMap<u32, u32> = HashMap::new();
        looped.insert(41, 42); // 41→42→41 回环
        looped.insert(42, 41);
        looped.insert(43, 42);
        let mut got = descendants_of(41, &looped);
        got.sort_unstable();
        assert_eq!(got, vec![42, 43], "回环当场断，43 照收、不重复");
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
    /// （task #68 的作业对象布局断言随 `KillJob` 一并移除 —— 常驻设计下那颗结构体已不存在。）
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn process_entry_layout_matches_the_win32_abi() {
        assert_eq!(std::mem::size_of::<ProcessEntry32W>(), 568);
        assert_eq!(
            std::mem::offset_of!(ProcessEntry32W, th32_parent_process_id),
            32,
            "父 PID 的偏移错了 = 整张祖先表都是歪的"
        );
        assert_eq!(std::mem::size_of::<ProcessEntry32W>() as u32, {
            ProcessEntry32W::zeroed().dw_size
        });
        assert_eq!(TH32CS_SNAPPROCESS, 0x2);
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

    // ------------------------------------------------------------------ #194 B1（PP3 K-锁3）
    //
    // 构造级/形状级锁：钉住「**候选表、父表、owner 表三张都出自同一次 toolhelp 快照**」这条
    // 构造不变式。这条不变式就是分叉不需要主干那枚 `bool?` 三态的唯一理由（PP3 §3）：
    // 主干在 T0 `SnapshotProcessParents()` 之后另起一次 `Process.GetProcessesByName("node")`（T1），
    // 两次之间出生的进程「进得了候选表、进不了父表」⇒ 必须靠 `null` 那一支不判死；
    // 分叉的 `sweep_inner` 只调一次 `snapshot_entries()`，同一份 `procs` 同时喂三张表 ⇒
    // 那种进程**连候选都不是**，净效果与主干的 `continue` 相同，第三态在此**结构上不可达**。
    //
    // 今天它靠形状成立、而**没有任何一条锁守着**：谁哪天为了「少读点路径」把候选过滤成单独
    // 一次枚举，或给 `select_orphans` 传一份过滤后的切片却传另一份完整 `parents`，分叉就立刻
    // 掉回主干 0.8.3.2 那个「凭祖先链未知误杀本轮刚拉起的新内核」的坑，而**现有测试全绿**。
    // 这条锁就是把那条论证换成闸门；零行为改动，也不往 `select_orphans` 的签名上加东西
    // （加 `bool?` 会打红 `ancestor_chain_walks_caps_and_terminates` 与
    // `empty_ancestor_chain_has_no_live_owner`，那是 PP3 §5 明确列为不该跟的）。

    /// 源形锁：`sweep_inner` 体内「一次快照 → 一份 `procs` → 三张表」的形状。
    /// 读的是自己的源码（仓内既有形制，见 `kernel.rs` 的 `include_str!` 那批），不枚举真进程。
    #[test]
    fn sweep_inner_builds_all_three_tables_from_one_toolhelp_snapshot() {
        const PG_SELF: &str =
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/procguard.rs"));
        let start = PG_SELF
            .find("fn sweep_inner(spare_pid: u32) -> SweepOutcome {")
            .expect("清扫主链还在");
        let end = start
            + PG_SELF[start..]
                .find("fn sweep_note(")
                .expect("清扫体的右边界（下一个函数）");
        let body = &PG_SELF[start..end];

        // ① 快照**恰好一次**：多一次就是第二次枚举，第二次枚举正是主干那枚 `null` 的成因。
        assert_eq!(
            body.matches("snapshot_entries()").count(),
            1,
            "清扫链里出现第二次进程枚举 ⇒ 候选表与父表不再同刻"
        );
        // ② 单一 `procs` 绑定，且它由那一发快照喂出来。
        assert_eq!(
            body.matches("let procs").count(),
            1,
            "清扫链里出现第二份进程表 ⇒ 候选侧与父侧可能吃不同数据"
        );
        let rows_at = body.find("snapshot_entries()").expect("那一发快照调用");
        let procs_at = body.find("let procs").expect("那份进程表");
        let parents_at = body.find("parent_map(&procs)").expect("父表由同一份 procs 建");
        let owners_at = body.find("owner_pids(&procs,").expect("owner 表由同一份 procs 建");
        let orphans_at = body.find("select_orphans(&procs,").expect("候选侧也吃同一份 procs");
        assert!(
            rows_at < procs_at
                && procs_at < parents_at
                && parents_at < owners_at
                && owners_at < orphans_at,
            "三张表必须在同一份 procs 之后、且都在它之上建起来：{rows_at}/{procs_at}/{parents_at}/{owners_at}/{orphans_at}"
        );

        // ③ 反向：清扫体内不得再自己开一颗枚举或换一条数据通道。
        for banned in [
            "CreateToolhelp32Snapshot",
            "Process32FirstW",
            "Process::all",
            "EnumProcesses",
        ] {
            assert!(
                !body.contains(banned),
                "清扫体内不该直接碰 Win32 枚举原语（只许经由那一发快照）：{banned}"
            );
        }
        // ④ `select_orphans` 的父表实参必须就是 `parent_map` 的返回值那颗绑定，
        //    而不是另建一份 —— 这一条防的是「两张表分别建、传错一个」。
        assert_eq!(
            body.matches("parent_map(&procs)").count(),
            1,
            "父表只许建一次；建两份就等于允许候选侧与父侧不同源"
        );
        assert!(
            body.contains("let parents = parent_map(&procs);"),
            "父表要绑进一个名字里再传下去，别在调用点上现建现传"
        );
        assert!(
            body.contains("&parents, &owners, self_pid, spare_pid"),
            "候选侧拿到的必须是同一对 parents/owners 绑定"
        );
    }
}
