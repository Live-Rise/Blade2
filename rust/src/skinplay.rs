//! 视频皮肤**播放**腿：一条解码专线线程 + 一个与 reactor 解耦的帧槽。
//!
//! # 这条腿为什么长这样（根因，不是抄主干的形状）
//!
//! 主干的壁纸视频是 `MediaPlayerElement`（`MainWindow.xaml.cs:8875-9230`）：XAML 控件自己
//! 管解码、循环、静音、失焦暂停，壳只拨三颗开关。分叉**没有这条通路**，且不是「还没搬」，
//! 是搬不过来，三层各自独立成立：
//!
//! ① **reactor 0.100 货架上没有视频控件**。`MountedKind`（`generated.rs`）里没有
//!    `MediaPlayerElement` / `MediaElement`；`WebView2` 与 `SwapChainPanel` 两个名字在，
//!    但都是**只有 `new()` 与 `element_ref()` 的空壳**（`generated.rs:5384` / `:5412`）——
//!    没有 setter，连 `IsHitTestVisible` 都设不了（WebView2 的 content island 会抢指针，
//!    主干正是为此在 `MainWindow.Wallpaper.cs:33-36` 立了「网格里不放手播元素」的规矩）。
//!    `mod native` 是私有的（`lib.rs:7`）⇒ 声明式树里长不出任何能播视频的东西。
//! ② **逃生舱也堵着**。`reference.rs` 确实给了三个命令式口子（`Image::request_set_native_source`、
//!    `Grid::observe_composition_host` + `request_set_child_visual`、
//!    `SwapChainPanel::request_set_swap_chain`），但：
//!    * `request_set_native_source` 原生侧做 `source.cast::<ImageSource>()`
//!      （`native/winui/mod.rs:1710`），而 reactor 的 `ImageSource` 是 **XAML 类**
//!      （`bindings.rs:769`：`BitmapImage → BitmapSource → ImageSource → DependencyObject`）。
//!      WinRT 的 `Windows.Graphics.Imaging.SoftwareBitmapSource` 不派生自它 ⇒ QI 直接失败；
//!      而 `SoftwareBitmapSource` 在 `windows` 0.62.2 里**根本不存在**（`Graphics/Imaging`
//!      模块只有 `SoftwareBitmap`，无 `…Source`），本仓的 `windows` 也拿不到 WinUI 的
//!      `BitmapImage`（reactor 不 re-export）。
//!    * 另两个口子是 GPU 路（D2D composition surface / D3D swap chain），要为它拉
//!      `Win32_Graphics_Direct2D` + `Direct3D11` 并手写成套设备/表面管理 —— 那是另一条
//!      工程量级的活，不在「够用的最小形态」里。
//!    * 三个口子的签名都吃/回 `windows_core::IUnknown`，而 reactor 用的是 **0.100** 那份
//!      `windows-core`，本仓 `windows` 0.62.2 带的是 **0.62.2** 那份 —— 同名 crate 的两个
//!      版本在同一个二进制里是两个类型，要调就得再挂一个改名依赖。cost/benefit 不成立。
//!
//! ③ **reactor 没有窗口焦点事件**（`Window` 只有生命周期钩，没有 `on_activated`，
//!    `src/main.rs:150-153` 早有记载）⇒ 主干那颗「失焦暂停」(`shell-skin.videopause`)
//!    **依然做不了**。这不是本模块的疏漏，是框架货架的缺口；`shellfiles` 那四个
//!    `*_skin_video_pause` 纯函数因此继续零调用点。
//!
//! # 于是走哪条
//!
//! 走 **reactor 自己就有的编码帧通路**：`Image::source_data(EncodedImage)`。
//! `EncodedImage::new(bytes)` 收任意编码位图字节，原生侧 `start_encoded_image`
//! （`native/winui/mod.rs:3739-3781`）建 `InMemoryRandomAccessStream` → `DataWriter` 写入
//! → `StoreAsync` → `BitmapImage.SetSourceAsync`，且 `encoded_image_nodes`
//! （`mod.rs:154`）把这些节点登记成「外部持有」⇒ `enqueue()`（`mod.rs:3186`）**跳过**
//! `ImageOpened` / `ImageFailed` 反馈。整套就是为「高频换图」设计的，不产生反馈风暴。
//! 喂进去的字节 = [`skinvideo::bmp_bytes`] 的 32bpp top-down BMP（54 字节手写头 + 像素，
//! 零压缩成本，WIC 原生认）。
//!
//! 换图的节拍由本模块驱动：**解码专线线程**（`std::thread`，自己开 COM/MF 计数）按目标帧率
//! 解一帧、往帧槽里放一帧；壳侧用 `context.spawn_background` 睡到有帧可取，取到就回一条
//! `Msg` 触发换图。`spawn_background` 落在 **Windows 线程池**上（`windows_threading::submit`，
//! `core/component.rs:620`）⇒ 每次醒来可能是**另一根**池线程，所以 MF reader 绝不跨闭包传递，
//! 只活在专线线程里（COM 对象有线程亲和，跨线程不 marshall 就是 UB）。
//!
//! # 帧槽语义：只留最新一帧
//!
//! 槽里永远只有**最新**一帧（发布即覆盖，不排队）。壳侧取货慢于解码时，中间帧被丢弃 ——
//! 这正是视频要的语义（过期帧没有显示价值），也顺带封顶了内存（一帧的常驻量）。
//!
//! # 成本与档位（为何是这两个数）
//!
//! 每帧的常驻开销 = BGRA 行拷贝 + BMP 头拼接 + `DataWriter` 拷贝一份 + WIC 解码 + 贴图上传。
//! 4K 帧（33 MB BMP）在 24fps 下要把这条链跑满百兆字节/秒，UI 线程必卡；故
//! [`SKIN_VIDEO_MAX_EDGE`] 把长边压到 1280（720p 帧 BMP ≈ 3.7 MB），
//! [`SKIN_VIDEO_FPS`] 定 24（壁纸是氛围层，不需要 60）。两个都是编译期常量、
//! 单一真相，调档只改这一处。
//!
//! # 备案的偏差（对齐主干后仍不同的地方）
//!
//! * **无失焦暂停**：见上文 ③。代价 = 窗口失焦时解码循环照跑。
//! * **软件解码**：主干走 MediaPlayer 的硬件解码；本模块是 MF Source Reader + 视频处理器
//!   MFT 的软件腿（`skinvideo` 模块头实测备案：H.264 → RGB32 必须靠视频处理器）。分辨率
//!   上限因此也受 CPU 制约，1280 是兼顾清晰度与帧率的折中。
//! * **不接 Wallpaper Engine 源**：主干 `MainWindow.Wallpaper.cs` 那半是「从本机 WE 工坊目录
//!   拉清单 + 下载进槽位」，依赖 `@baiiii/dsh-wallpaper-local` 插件的服务端路由；本模块只负责
//!   **播放**已经躺在 `shell-skin.video` 里的文件（本地导入的那一路）。WE 源是另一条独立的
//!   接线，不塞进本模块。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::skinvideo::{self, Rgb32Reader};

/// 播放帧的长边上限（像素，保宽高比）。`0` 会让 4K 视频按原尺寸出帧，BMP 每帧 33 MB，
/// 24fps 下必卡 UI —— 见模块头「成本与档位」。720p 帧 BMP ≈ 3.7 MB。
pub const SKIN_VIDEO_MAX_EDGE: u32 = 1280;
/// 目标帧率。壁纸是氛围层，24 足够；再高只是把 UI 线程的换图成本线性推上去。
pub const SKIN_VIDEO_FPS: u32 = 24;
/// 壳侧一次 `spawn_background` 等帧的上限。超过它就回 `Idle` 让壳重上发条 —— 正常情况
/// 一帧 ~42 ms 内必到，250 ms 是「解码线程卡住/首帧还在装载」的容忍窗，不是节拍。
const FRAME_WAIT: Duration = Duration::from_millis(250);
/// 睡意的切片粒度：停泵要**及时**（槽文件等着被覆盖/删除），把整帧间隔切成小片轮询停止旗。
const SLEEP_SLICE: Duration = Duration::from_millis(4);

/// 壳侧一次取货的结果。
///
/// * [`FrameTake::Frame`] —— 拿到一帧（BMP 字节，可直接喂 `EncodedImage`）。
/// * [`FrameTake::Idle`] —— 等超时了还没新帧。不是错误：首帧装载、循环回绕的接缝、
///   某一帧解得慢，都会落到这一档。壳照常重上发条。
/// * [`FrameTake::Dead`] —— 泵**死了**，带死因。壳该停泵、回落海报，并不再重上发条。
#[derive(Clone, Debug)]
pub enum FrameTake {
    Frame(Arc<[u8]>),
    Idle,
    Dead(String),
}

/// 帧槽：解码线程写、壳侧读，**只留最新一帧**。
///
/// `Mutex` + `Condvar`（而非 channel）：channel 会排队，而这里过期帧没有价值；
/// 且「取货方要能被超时唤醒」用 `wait_timeout` 比 `recv_timeout` 更直白。
pub struct FrameSlot {
    state: Mutex<SlotState>,
    ready: Condvar,
}

struct SlotState {
    frame: Option<Arc<[u8]>>,
    dead: Option<String>,
}

impl FrameSlot {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(SlotState {
                frame: None,
                dead: None,
            }),
            ready: Condvar::new(),
        })
    }

    /// 壳侧取货：等到有新帧 / 槽死 / 超时三者先发生。
    pub fn take(&self, timeout: Duration) -> FrameTake {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(error) = state.dead.take() {
                return FrameTake::Dead(error);
            }
            if let Some(frame) = state.frame.take() {
                return FrameTake::Frame(frame);
            }
            let now = Instant::now();
            if now >= deadline {
                return FrameTake::Idle;
            }
            let (guard, _) = match self.ready.wait_timeout(state, deadline - now) {
                Ok(pair) => pair,
                Err(poisoned) => poisoned.into_inner(),
            };
            state = guard;
        }
    }

    /// 壳侧取货口（默认容忍窗）：等 [`FRAME_WAIT`] 这么久，看有没有下一帧。
    ///
    /// 超时口径住在 lib 侧这一枚（「一帧 ~42ms 内必到、250ms 是容忍窗」是帧泵自己的知识），
    /// 壳不该自己拍等多久 —— 壳只负责「取到就回一条 Msg」。壳侧只拿 `Arc<FrameSlot>`
    /// 这一枚进 worker 闭包（它 `Send`），把手本身不过线程池。
    pub fn take_frame(&self) -> FrameTake {
        self.take(FRAME_WAIT)
    }

    /// 解码线程发布一帧（覆盖旧的）。
    fn publish(&self, frame: Arc<[u8]>) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.frame = Some(frame);
        drop(state);
        self.ready.notify_all();
    }

    /// 解码线程报死：此后每次取货都先看到死因（取一次即清，壳停泵后不再有读者）。
    fn die(&self, error: String) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.dead = Some(error);
        state.frame = None;
        drop(state);
        self.ready.notify_all();
    }
}

/// 一枚活泵的把手。壳侧只认这两件：帧槽 + 停止旗。
///
/// `stop` 是 `Arc<AtomicBool>` 而不是 `JoinHandle`：停泵要**同步**（槽文件马上要被覆盖或
/// 删除 —— MF reader 持着文件时 `std::fs::copy` / `remove_file` 会撞共享冲突），所以
/// [`PumpHandle::stop`] 会 join。把手本身不暴露线程句柄，避免壳侧误用。
pub struct PumpHandle {
    slot: Arc<FrameSlot>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl PumpHandle {
    pub fn slot(&self) -> Arc<FrameSlot> {
        Arc::clone(&self.slot)
    }

    /// 停泵并**等线程收工**（reader  Drop、COM/MF 计数归零、文件句柄释放）。
    ///
    /// 等多久有上界：线程的睡意切成 [`SLEEP_SLICE`] 的片，所以最坏 = 当前这一帧的解码耗时
    /// 再加一片睡意。对一个明确的用户动作（导入/清除皮肤）来说，这点 hitch 可接受；
    /// 换来的是槽文件随即可写，不必和 MF 抢文件。
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Release);
        self.stop_common();
    }

    fn stop_common(&mut self) {
        if let Some(thread) = self.thread.take() {
            // join 失败只有一种情况：线程 panic 了。那时 reader 已被 unwind 丢掉，
            // 文件句柄同样释放，无需额外处置。
            let _ = thread.join();
        }
    }
}

impl Drop for PumpHandle {
    fn drop(&mut self) {
        // 壳把把手丢了（换泵、清皮肤、进程退出前的状态复位）：置旗 + join。
        // 若 `stop` 已经消费过把手，`thread` 已是 None，这里空转。
        self.stop.store(true, Ordering::Release);
        self.stop_common();
    }
}

/// 起一枚泵：专线线程解 `video`，按 [`SKIN_VIDEO_FPS`] 往槽里放帧。
///
/// 线程**立刻**返回把手，不等首帧 —— 首帧装载（容器解析 + decoder 起来）要几百毫秒，
/// 壳侧那一侧有 [`FRAME_WAIT`] 的超时兜底。
pub fn start_pump(video: PathBuf) -> PumpHandle {
    let slot = FrameSlot::new();
    let stop = Arc::new(AtomicBool::new(false));
    // 先各留一份：`spawn` 失败时闭包连同里面的 slot 一起被丢掉，报死得走外面这份。
    let pump_slot = Arc::clone(&slot);
    let pump_stop = Arc::clone(&stop);
    let spawned = std::thread::Builder::new()
        .name("skin-video-pump".to_string())
        .spawn(move || pump_loop(pump_slot, pump_stop, video));
    let thread = match spawned {
        Ok(thread) => Some(thread),
        Err(error) => {
            // 线程起不来不能 panic：壳跑在 `windows_subsystem = "windows"` 下，
            // panic 会把整个进程带走。退化成「槽已死」⇒ 壳侧照常回落海报。
            slot.die(format!("解不了：解码线程起不来：{error}"));
            None
        }
    };
    PumpHandle { slot, stop, thread }
}

/// 专线线程主体。任何错误都折成 `slot.die(...)` 后收工 —— 不 panic、不重试、
/// 不吞（壳侧要拿原因落 DIAG 并回落海报）。
fn pump_loop(slot: Arc<FrameSlot>, stop: Arc<AtomicBool>, video: PathBuf) {
    if let Err(error) = pump_run(&slot, &stop, &video) {
        slot.die(error);
    }
}

fn pump_run(slot: &Arc<FrameSlot>, stop: &AtomicBool, video: &std::path::Path) -> Result<(), String> {
    // COM/MF 计数在本线程自持一份：与壳的 UI 线程互不相干（各开各的、各关各的）。
    let _com = skinvideo::ComGuard::init()?;
    let _mf = skinvideo::MfGuard::init()?;
    let mut reader = Rgb32Reader::open(video, SKIN_VIDEO_MAX_EDGE)?;
    let (width, height) = reader.geometry();
    let interval = Duration::from_secs(1) / SKIN_VIDEO_FPS;
    loop {
        if stop.load(Ordering::Acquire) {
            return Ok(());
        }
        match reader.next_rows()? {
            Some(rows) => {
                let bmp = skinvideo::bmp_bytes(width, height, &rows);
                slot.publish(Arc::from(bmp));
            }
            None => {
                // 流尾：回起点循环。seek 失败 = 这文件不支持随机访问 ⇒ 报死，
                // 壳回落海报（比反复重开容器省事，也避免在坏档上打转）。
                reader.rewind()?;
                // seek 成功后 reader 要重 priming，第一帧可能又要几百毫秒 —— 睡一片，
                // 让壳侧的超时逻辑去等，别在这里空转。
                sleep_or_stop(interval, stop);
            }
        }
        sleep_or_stop(interval, stop);
    }
}

/// 把 `duration` 切成 [`SLEEP_SLICE`] 的片睡，任何一片发现停止旗就提前返回。
/// 停泵要及时（槽文件等着被覆盖），整段 `thread::sleep` 会让 join 白等一个帧间隔。
fn sleep_or_stop(duration: Duration, stop: &AtomicBool) {
    let deadline = Instant::now() + duration;
    loop {
        if stop.load(Ordering::Acquire) {
            return;
        }
        let now = Instant::now();
        if now >= deadline {
            return;
        }
        std::thread::sleep((deadline - now).min(SLEEP_SLICE));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_reports_the_latest_frame_and_drops_stale_ones() {
        let slot = FrameSlot::new();
        slot.publish(Arc::from(vec![1u8, 2, 3]));
        slot.publish(Arc::from(vec![9u8]));
        // 只该拿到最后那一帧：中间那帧被覆盖，不排队。
        match slot.take(Duration::from_millis(50)) {
            FrameTake::Frame(frame) => assert_eq!(&*frame, &[9u8]),
            other => panic!("该拿到帧，实际 {other:?}"),
        }
        // 槽空了：等超时归 Idle，不是 Dead。
        assert!(matches!(
            slot.take(Duration::from_millis(20)),
            FrameTake::Idle
        ));
    }

    #[test]
    fn a_dead_slot_survives_the_take() {
        let slot = FrameSlot::new();
        slot.die("解不了：假的".to_string());
        match slot.take(Duration::from_millis(20)) {
            FrameTake::Dead(error) => assert!(error.contains("解不了")),
            other => panic!("该报死，实际 {other:?}"),
        }
        // 死因取一次即清：壳停泵后不再有读者，不会反复弹同一条。
        assert!(matches!(
            slot.take(Duration::from_millis(20)),
            FrameTake::Idle
        ));
    }

    #[test]
    fn the_two_knobs_stay_in_the_budget() {
        // 720p 帧的 BMP 常驻量 ≈ 3.7 MB；4K 会到 33 MB。档位一动，模块头「成本与档位」
        // 那笔账就要重算 —— 这里只钉「长边确实被压住了」与「帧率在合理区间」。
        assert!(SKIN_VIDEO_MAX_EDGE <= 1920 && SKIN_VIDEO_MAX_EDGE >= 640);
        assert!(SKIN_VIDEO_FPS >= 12 && SKIN_VIDEO_FPS <= 60);
        assert!(FRAME_WAIT > Duration::from_secs(1) / SKIN_VIDEO_FPS);
    }

    /// 真机视频的**端到端**冒烟：起泵 → 专线线程开 COM/MF → 解一帧 → 过帧槽 → 壳侧取到
    /// 合法 BMP。前三件（帧槽语义、死因、档位）都是假件能测的，唯独「MF 长命 reader 在
    /// 专线线程上真能解出 BGRA、且 `bmp_bytes` 拼出来的头 WIC 认」必须拿真视频跑 ——
    /// 这条链上任何一环断了，界面上表现为「背景永远停在海报」，没有第二手证据。
    ///
    /// 没有真机视频槽就跳过（`skinvideo` 那颗同形冒烟同判据）：CI / 没导过视频的机器上
    /// 不能算失败。视频路径逐字照 `skinvideo::tests` 那颗（同一个 `DataHome` 槽位）。
    #[test]
    fn the_real_video_slot_smoke_pumps_a_legal_bmp_frame() {
        let Some(video) = std::env::var_os("LOCALAPPDATA")
            .map(|root| PathBuf::from(root).join("Blade2").join(skinvideo::SKIN_VIDEO_FILE))
        else {
            eprintln!("smoke 跳过：LOCALAPPDATA 不可用");
            return;
        };
        if !video.is_file() {
            eprintln!(
                "smoke 跳过：{} 不存在（真机视频皮肤未装）",
                video.display()
            );
            return;
        }
        let started = Instant::now();
        let handle = start_pump(video.clone());
        // 首帧要等容器装载 + 视频处理器 priming，38 MB 的档实测几百毫秒到一秒量级。
        // 给到 10s：超了就是「泵起不来」，按死讯处置而不是干等。
        let frame = match handle.slot().take(Duration::from_secs(10)) {
            FrameTake::Frame(frame) => frame,
            other => panic!("真机起泵后该拿到首帧，实际 {other:?}"),
        };
        // BMP 头逐字段核对（与 `skinvideo::tests` 那颗同一套口径，两份判据不许漂移）。
        assert_eq!(&frame[0..2], b"BM", "BMP 魔数");
        let bf_size = u32::from_le_bytes(frame[2..6].try_into().unwrap());
        assert_eq!(bf_size as usize, frame.len(), "bfSize = 字节数实际长度");
        let width = i32::from_le_bytes(frame[18..22].try_into().unwrap());
        let height = i32::from_le_bytes(frame[22..26].try_into().unwrap());
        assert_eq!(u16::from_le_bytes(frame[28..30].try_into().unwrap()), 32, "bpp=32");
        assert_eq!(u32::from_le_bytes(frame[30..34].try_into().unwrap()), 0, "BI_RGB");
        assert!(height < 0, "biHeight 必须为负（top-down）");
        assert_eq!(
            bf_size,
            (54 + (width as i64 * 4 * (-(height as i64))) as u32),
            "bfSize = 54 + width*4*|height|"
        );
        // 长边必须被 SKIN_VIDEO_MAX_EDGE 压住：这是「4K 不卡 UI」那条账的实处。
        assert!(
            width.max(-height) as u32 <= SKIN_VIDEO_MAX_EDGE,
            "长边 {}/{} 超过档位 {}",
            width,
            -height,
            SKIN_VIDEO_MAX_EDGE
        );
        eprintln!(
            "smoke OK：{} 泵出 {}x{} BMP {} 字节，首帧耗时 {:.1?}",
            video.display(),
            width,
            -height,
            frame.len(),
            started.elapsed()
        );
        // 停泵必须**及时**（壳侧同步 join 等它）：一个帧间隔 + 一片睡意是上界。
        // 拖成几秒 = 导入/清除皮肤时 UI 白冻住，正是这条腿最容易被重构破坏的地方。
        let stop_started = Instant::now();
        handle.stop();
        assert!(
            stop_started.elapsed() < Duration::from_secs(2),
            "停泵等了 {:.1?}：睡意没被停止旗切开，壳侧 join 会冻住 UI",
            stop_started.elapsed()
        );
    }
}
