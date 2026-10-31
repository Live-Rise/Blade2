//! task #68 时代的「壳亡即杀」作业对象契约（`Kernel::start` 挂 `KillJob`、`shutdown()` 关句柄）
//! 已随**内核常驻**（主干 0.9.0.4 `Dsh/DshKernelHost.cs` 的移植，见 `kernel.rs` 模块头）整体推翻，
//! 本文件随之改钉**新契约**的四根桩：
//!
//! 1. `Kernel::start` 成功即把连接信息落 `DSH_HOME\blade2-resident-kernel.json`，
//!    五个 JSON 键与 C# `System.Text.Json` 逐字互通（`Home`/`pid`/`url`/`exe`/`ver`）；
//! 2. **Drop 不杀内核**（窗口正常关闭绝不能带走内核 —— 这是常驻的全部意义）；
//! 3. `try_attach_resident` 探活复用那颗活内核（pid/url/身份核对全过）；
//! 4. 显式终止只有 `kill_and_clear`（杀树 + 删状态文件），失效常驻（版本不符）按记录路径回收。
//!
//! 口径：这里 spawn 的是 `CARGO_BIN_EXE_fake_dsh`，**本用例自己起的进程**，收尾由本用例显式
//! 完成；不碰任何既有 PID。文件名保留 w4 一系（task 谱系可追溯），内容已换防。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use blade2_rs::kernel::{
    Kernel, Launch, ResidentState, delete_resident_state, read_resident_state, resident_state_path,
    try_attach_resident_with,
};
use blade2_rs::procguard::{kill_process_tree, process_image_path, same_program_file};

/// 收尾轮询的单拍与总预算：`TerminateProcess` 是异步的，判「死透」要给窗口。
const POLL_TICK: Duration = Duration::from_millis(50);
const POLL_BUDGET: Duration = Duration::from_secs(5);

fn fake_launch(dsh_home: &Path) -> Launch {
    Launch {
        exe: PathBuf::from(env!("CARGO_BIN_EXE_fake_dsh")),
        args: vec!["--pace=0".to_string()],
        // 假内核走正常 argv 列表（那颗 `--pace=0` 不含引号）⇒ 不走 `raw_arg`；`is_bundled`
        // 按「内置两颗探针的与」口径，这里不经 `Launch::from_env` 解析，不谎报内置在位。
        args_verbatim: false,
        is_bundled: false,
        dsh_home: Some(dsh_home.to_path_buf()),
        path_prepend: None,
        working_dir: None,
    }
}

/// 沙箱数据家：`std::env::temp_dir()` 下自建自清，绝不碰真 `%LOCALAPPDATA%\Blade2`。
struct Home(PathBuf);
impl Home {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "blade2-w4-resident-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::create_dir_all(&dir);
        Home(dir)
    }
    fn path(&self) -> &Path {
        &self.0
    }
    fn state_file(&self) -> PathBuf {
        resident_state_path(&self.0)
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.state_file());
        let _ = std::fs::remove_dir(&self.0);
    }
}

fn fake_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fake_dsh"))
}

fn alive_as(pid: u32, exe: &Path) -> bool {
    process_image_path(pid).is_some_and(|image| same_program_file(&image, exe))
}

fn wait_until_gone(pid: u32, what: &str) {
    let deadline = Instant::now() + POLL_BUDGET;
    while process_image_path(pid).is_some() {
        assert!(Instant::now() < deadline, "{what} 在 {POLL_BUDGET:?} 内没有退干净");
        std::thread::sleep(POLL_TICK);
    }
}

/// 兜底回收（断言失败也尽量不留活进程）：按状态文件里的记录杀树 + 清文件。
fn reap_from_state(home: &Home) {
    if let Some(state) = read_resident_state(home.path()) {
        if let (Ok(pid), Some(exe)) =
            (u32::try_from(state.pid), state.exe.as_deref().map(Path::new))
        {
            let _ = kill_process_tree(pid, Some(exe));
        }
    }
    delete_resident_state(home.path());
}

/// ①+④ URL 到手即登记常驻：五个键逐字对齐 C#（两侧共用一个数据家，键名就是协议），
/// `pid`/`url` 与本次连接一致，`exe` 正是 fake_dsh 那颗进程；`kill_and_clear` 收树 + 删文件。
#[test]
fn start_writes_the_mainline_shaped_state_and_kill_and_clear_reaps_it() {
    let home = Home::new("write");
    let mut kernel = Kernel::start(&fake_launch(home.path())).expect("假内核应完成 dsh web: 握手");
    let text = std::fs::read_to_string(home.state_file()).expect("URL 到手即落状态文件");
    let value: serde_json::Value = serde_json::from_str(&text).expect("状态文件是合法 JSON");
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("状态文件是 JSON 对象")
        .keys()
        .map(|key| key.as_str())
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["Home", "exe", "pid", "url", "ver"], "五个键一个不多不少");
    assert_eq!(value["Home"], serde_json::json!(home.path().display().to_string()));
    assert_eq!(value["pid"], serde_json::json!(kernel.pid().expect("pid 在")));
    assert_eq!(value["url"], serde_json::json!(kernel.url));
    assert!(value["ver"].is_string(), "ver 永远是字符串（读不到版本写空串）: {}", value["ver"]);
    assert!(
        value["exe"].as_str().is_some_and(|exe| same_program_file(Path::new(exe), &fake_exe())),
        "exe 必须是那颗 fake_dsh 进程的模块路径: {}",
        value["exe"]
    );

    let pid = kernel.pid().expect("pid 在");
    kernel.kill_and_clear();
    kernel.kill_and_clear(); // 幂等：收过一遍再调是空操作，不许炸
    assert!(!home.state_file().exists(), "kill_and_clear 要删状态文件");
    wait_until_gone(pid, "kill_and_clear 后的假内核");
}

/// ② 新契约的核心反转：**Drop 不杀内核、也不删状态文件**（旧契约是 Drop→shutdown→杀树）。
/// 内核活着 = 常驻的前提；它占着的会话写租约正是下次复用的本钱。
#[test]
fn dropping_the_kernel_keeps_it_alive_and_the_state_intact() {
    let home = Home::new("keepalive");
    let kernel = Kernel::start(&fake_launch(home.path())).expect("假内核应完成 dsh web: 握手");
    let pid = kernel.pid().expect("pid 在");
    drop(kernel);
    // 状态文件必须在（Drop 只交还本端句柄，一个字节都不动盘上的登记）。
    assert!(
        read_resident_state(home.path()).is_some(),
        "Drop 不许删状态文件 —— 删了下次启动就只能冷启"
    );
    // 进程必须在：1s 的窗口里反复核对身份，防「刚好还没退」的假阳。
    let window = Instant::now() + Duration::from_secs(1);
    while Instant::now() < window {
        assert!(
            alive_as(pid, &fake_exe()),
            "Drop 之后内核必须还活着（常驻契约被打破：正常退出带走了内核）"
        );
        std::thread::sleep(POLL_TICK);
    }
    reap_from_state(&home);
    wait_until_gone(pid, "兜底回收后的假内核");
}

/// ③+④ attach 复用链：壳重启（drop）后 `try_attach_resident` 探活接上**同一颗**内核
/// （pid/url 不变、RPC 面可用）；版本一岔（升级/降级）按记录路径杀树回收、状态文件清掉。
#[test]
fn try_attach_resident_reuses_the_live_kernel_and_version_skew_recycles_it() {
    let home = Home::new("attach");
    let kernel = Kernel::start(&fake_launch(home.path())).expect("假内核应完成 dsh web: 握手");
    let pid = kernel.pid().expect("pid 在");
    let url = kernel.url.clone();
    let ver = read_resident_state(home.path())
        .expect("start 已落状态")
        .ver;
    drop(kernel); // 模拟壳退出：内核留下
    assert!(alive_as(pid, &fake_exe()), "attach 前提：那颗内核还活着");

    // 复用：版本对上（这里用注入口喂记录值 —— 生产路径喂的是 bundled 版本，同一判据）。
    let mut attached = try_attach_resident_with(home.path(), Some(ver.clone()))
        .expect("活内核 + 版本一致 + 探活通过 ⇒ 必须复用而不是冷启");
    assert_eq!(attached.pid(), Some(pid), "复用的是同一颗进程");
    assert_eq!(attached.url, url);
    assert!(attached.is_attached(), "attach 来的内核本会话没有 spawn 过进程");
    let rows = attached.list_sessions().expect("复用后的 RPC 面可用");
    assert_eq!(rows.len(), 3, "fake_dsh 的三条会话台账");
    assert!(
        attached.log.iter().any(|line| line.contains("[resident] attached")),
        "复用要在诊断里留痕：{:?}",
        attached.log
    );
    drop(attached);
    assert!(alive_as(pid, &fake_exe()), "attach 完再退出同样不许杀内核");
    assert!(read_resident_state(home.path()).is_some(), "attach 周期不删状态文件");

    // 版本岔了（状态记 0.1.7、现在是 0.2.0 那一档）：按记录路径杀树回收 + 清状态，回 None 走冷启。
    let skewed = format!("{ver}-skewed");
    assert!(
        try_attach_resident_with(home.path(), Some(skewed)).is_none(),
        "版本不符必须拒绝复用"
    );
    assert!(!home.state_file().exists(), "回收支要清状态文件");
    wait_until_gone(pid, "版本不符被回收的假内核");
}

/// ④ 的另一半：pid 已死 / 状态缺失 ⇒ 只清状态、返回 None，不给冷启添堵。
#[test]
fn try_attach_resident_on_a_dead_or_absent_state_falls_back_to_cold_start() {
    let home = Home::new("cold");
    // 没有状态文件：直接 None。
    assert!(try_attach_resident_with(home.path(), Some("9.9.9".into())).is_none());

    // 状态文件在、但 pid 早已不存在：None + 文件被清（主干「pid gone -> cold」同位）。
    write_dead_state(home.path());
    assert!(try_attach_resident_with(home.path(), Some("9.9.9".into())).is_none());
    assert!(!home.state_file().exists(), "死 pid 的状态文件要被清掉");
}

/// 手搓一份「pid 绝不存在」的状态（65530 起往上是动态保留区，谁也占不了）。
fn write_dead_state(home: &Path) {
    blade2_rs::kernel::write_resident_state(
        home,
        &ResidentState {
            home: None,
            pid: 65530,
            url: "http://127.0.0.1:1/?token=dead".to_string(),
            exe: Some(fake_exe().display().to_string()),
            ver: "9.9.9".to_string(),
        },
    );
}
