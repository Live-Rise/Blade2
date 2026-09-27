//! task #68 的第二发离线端到端用例：`Kernel::start` 到底有没有把「壳亡即杀」作业挂上，
//! 以及挂没挂上都**必须**留下一行可观测的诊断。
//!
//! 为什么单独开一个文件而不是塞进 `tests/ipc.rs`：那个文件此刻正被另一路改动占着。
//!
//! 口径：
//! · 这里 spawn 的是 `CARGO_BIN_EXE_fake_dsh`，**本用例自己起的进程**，收尾由 `Kernel::drop`
//!   （`shutdown()` → 关作业句柄 → `KILL_ON_JOB_CLOSE`）带走；不碰任何既有 PID。
//! · `armed` 的真假**不做断言**：宿主终端自己也用作业对象包进程（Windows Terminal / CI 那类），
//!   那时指派会拿 `ERROR_DUPLICATE_JOB_ASSOCIATION` 失败并安静降级 —— 那是设计里的分支，
//!   不是回归。这里只钉「诊断行在、且与 `armed` 自相一致」，实机跑出来的值打 stdout（`--nocapture`）。

use std::path::PathBuf;

use blade2_rs::kernel::{Kernel, Launch};

fn fake_launch() -> Launch {
    Launch {
        exe: PathBuf::from(env!("CARGO_BIN_EXE_fake_dsh")),
        args: vec!["--pace=0".to_string()],
        // 本用例只钉「作业挂载留没留诊断行」，起的是假内核：argv 是正常列表里的一颗无引号 token
        // ⇒ 不走 `raw_arg`；`is_bundled` 按 §1.7-1 只该等于「内置两颗探针的与」，而这里刻意不经
        // `Launch::from_env` 解析（真机 `Kernel/` 在不在测试目录下不确定），谎报 `true` 会把
        // 「内置在位」这个本用例并不成立的前提写进 `Launch`。
        args_verbatim: false,
        is_bundled: false,
        dsh_home: None,
        path_prepend: None,
        working_dir: None,
    }
}

#[test]
fn kernel_start_reports_the_kill_on_close_job_diagnostic() {
    let kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    let note = kernel
        .log
        .iter()
        .find(|line| line.starts_with("内核作业对象("))
        .cloned()
        .unwrap_or_default();
    assert!(
        !note.is_empty(),
        "`Kernel::log` 里必须有那行作业诊断（它经 `push_log` 变成 stdout 的 `DIAG:` 行）：{:?}",
        kernel.log
    );
    assert!(
        note.contains(&format!("pid {}", kernel.pid().unwrap_or(0))),
        "诊断行要带上被挂作业的那个内核 PID：{note}"
    );
    // 诊断文案与 `armed` 必须同真同假：运行时 observer 只看这一行，不能看着「已挂载」其实是失败。
    assert_eq!(
        note.contains("作业对象已挂载并复查"),
        kernel.job_armed(),
        "诊断行与 `job_armed()` 自相矛盾：{note}"
    );
    println!("PROBE armed={} :: {note}", kernel.job_armed());
}

/// 收尾路径（`shutdown()` → `KillJob::close_now()`）走完之后，同一颗 `Kernel` 再 `shutdown()`
/// 一次也不许炸 —— 作业句柄「只关一次」的语义在这里钉住（关第二次是空操作，见 `procguard` 的用例）。
#[test]
fn shutting_twice_is_a_no_op_and_still_reports_the_kernel_pid() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    assert!(kernel.pid().is_some(), "内核 PID 要拿得到（STATUS 行在用）");
    kernel.shutdown();
    assert!(!kernel.job_armed(), "关掉之后这层保底不再算挂着");
    kernel.shutdown();
    assert!(kernel.pid().is_none(), "child 已经交还，再问就是 None 而不是 panic");
}
