//! 视频皮肤的海报帧**算子层**：从视频槽 `shell-skin.video`（Wallpaper Engine 导出的 MP4）
//! 用 Media Foundation 抽**第一帧**，写成 32bpp BMP 海报缓存，供上层拿现有
//! `Image::source_file` 通路铺底。
//!
//! # 备案 ——「静态帧」是刻意偏差，不是漏掉播放管线
//!
//! 主干的壁纸层认两枚槽：图片槽走 `ImageBrush`，视频槽 `shell-skin.video` 走
//! `MediaPlayerElement` 播放管线（主干 `MainWindow.Wallpaper.cs` / `DshWallpaperClient`）。
//! 分叉**整块没有视频皮肤**（`main.rs:130` 的既有记载：「分叉整块没有视频皮肤，那颗
//! toggle 也不存在 —— 随宠物宿主与视频皮肤移植一并接」），reactor 公共货架没有
//! `MediaPlayerElement` / `ImageBrush`（同 [`crate::main` 侧 `mw2_skin_backdrop`
//! 文档的备案：`main.rs:31785-31787`，`Brush` 只有 Theme/Solid 两档]），所以分叉用
//! `Image::source_file` + `Stretch::UniformToFill` 压成内容区底格。
//! 本模块就是那条通路的前半截：**视频槽 → 第一帧 → 本地 BMP 海报缓存**，上层照旧走
//! [`main.rs::mw2_skin_backdrop`] 的 `Image::source_file` —— 播放管线移植之前，
//! 视频皮肤在分叉里显示为静态海报帧，这是登记在案的偏差，勿当 bug 修。
//!
//! # 两层形状（全部纯函数、无全局态）
//!
//! * **算子层** [`poster_from_video`]：视频文件 → poster 旁的 meta（来源指纹）+ BMP 海报。
//!   COM/MF 生命周期全 RAII（[`ComGuard`] / [`MfGuard`]），HRESULT 折进 `String`。
//! * **组合层** [`ensure_poster`]：判槽（视频槽在 ∧ 图片槽不在）→ 判新鲜（[`poster_stale`]）
//!   → 不新鲜才抽。`Ok(None)` = 条件不满足（无视频 / 图片优先），`Err` = 尝试了但失败
//!   —— 调用方拿 Err 自行处置，本模块不吞错、不留全局 last_error。
//!
//! # meta 选型 —— poster 旁的 sidecar 小文本（**不污染 BMP 合法性**）
//!
//! 备选是「BMP 头后附加私有段」，弃用：私有段会让产物**不再是纯粹的合法 BMP**——
//! 上层 `Image::source_file` 与一切看图工具都按文件长度/头字段读 BMP，私货段轻则被
//! 忽略、重则判坏档回落「什么都不画」（`main.rs` 那条回落正是 `source_file` 解不动
//! ⇒ `None`）。sidecar（[`meta_path`]：`shell-skin.poster.meta`，与 poster 同 stem）
//! 零污染：BMP 恒合法，meta 缺失/坏档在 [`poster_stale`] 里一律判 stale ⇒ 重抽一次，
//! 代价自愈。格式一行：`<mtime 纳秒> <字节数>\n`（[`write_source_stamp`] /
//! [`read_source_stamp`] 读写各一枚，测试钉住）。
//!
//! # 与 shellfiles.rs 的关系 —— 槽名常量**不**在两处重复
//!
//! `shell-skin.img` / `shell-skin.video` **不在** [`crate::shellfiles`] 定义：那边模块头
//! （`shellfiles.rs:32-34`）白纸黑字「`shell-skin.video` 是壁纸**素材文件**……不是
//! 『壳本地配置』」，明确不收；`main.rs:32255-32258` 那对 `MW2_SKIN_*` 常量住在**二进制
//! crate**，lib 这边 `use` 不到（且那刀对 shellfiles 只读、本刀禁改 main.rs）。
//! 故两枚常量落在本模块（[`SKIN_IMG_FILE`] / [`SKIN_VIDEO_FILE`]），将来接线刀要么
//! 让 main.rs 改用本份、要么整队收编进 shellfiles，二选一，**不许第三份**。
//!
//! # 依赖版本备案
//!
//! `windows` 元 crate 在 crates.io 没有 0.100 发布（最新 0.62.2；0.100 只推到了
//! windows-core/collections/future/numerics 等 support crate，即 lock 里 windows-reactor
//! 的那份家族）。MF 绑定只在元 crate ⇒ 取 0.62.2，与 reactor 的 0.100 家族互不引用。
//! BMP 54 字节头**手写**（[`bmp_bytes`]），不为此拉 `Win32_Graphics_Gdi` feature。
//!
//! # MF 抽帧纪律（[`poster_from_video`] 的骨架）
//!
//! `CoInitializeEx(COINIT_MULTITHREADED)`（STA 已在即容忍 `RPC_E_CHANGED_MODE`，此时
//! 不 Uninit 别人的计数）→ `MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET)` → 结束
//! `MFShutdown` / `CoUninitialize`（Drop 对，绝不泄漏）。Reader 用
//! `MFCreateSourceReaderFromURL`，**必须带 `MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING`**
//! 属性创建；输出类型**一步切全类型**（major=Video + subtype=`MFVideoFormat_RGB32`），
//! 帧宽高从**输出**媒体类型 `MF_MT_FRAME_SIZE` 读。取第一帧非空 sample（连续空 sample
//! 上限 [`MAX_EMPTY_SAMPLES`]，EOS 判「没有视频流」）；stride 三档：`IMF2DBuffer::Lock2D`
//! （自带 stride）→ `ConvertToContiguousBuffer` + `MF_MT_DEFAULT_STRIDE` → `width*4`；
//! 行拷贝按 stride 逐行（负 stride = bottom-up 源，[`copy_strided_rows`] 统一摆正）。
//!
//! ⚠ 与简报「裸类型两步法」的偏差（实测定案，证据在本机 10.0.26220 + 真机 38 MB H.264
//! MP4）：① 半类型（只设 `MF_MT_MAJOR_TYPE`）的 `SetCurrentMediaType` 被 0x80070057 拒，
//! 开 AVP 也一样 —— 今日的 Source Reader 已不收半类型；② 不开 AVP 的 reader 连 RGB32
//! 全类型都拒（0xC00D36B4，H.264 decoder 只肯直出 NV12，而老式颜色转换 DMO 已缺席）；
//! 开 AVP 后一步全类型 RGB32 直接过（decoder 由 Source Reader 自动装载，冒烟实测输出
//! sub=RGB32、首帧非空）。故实现 = **AVP reader + 一步全类型**。
//!
//! # 错误消息约定
//!
//! 全部 `String`，HRESULT 以 `0x%08X` 折进去；三个分支从消息前缀可分辨：
//! 「解不了」（容器打不开 / decoder 起不来 / ReadSample 失败 / 锁像素失败）、
//! 「没有视频流」（GetNativeMediaType 落空 / EOS 无帧）、「写盘失败」（BMP 或 meta 落盘）。

use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use windows::core::{GUID, Interface, PCWSTR};
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::Media::MediaFoundation::{
    MFCreateAttributes, MFCreateMediaType, MFCreateSourceReaderFromURL, MFShutdown, MFStartup,
    MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
    MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING,
    MF_SOURCE_READER_FIRST_VIDEO_STREAM, MFSTARTUP_NOSOCKET, MF_VERSION, MFMediaType_Video,
    MFVideoFormat_RGB32, IMF2DBuffer, IMFAttributes, IMFMediaBuffer, IMFSourceReader, IMFSample,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

// ============================== 常量 ==============================

/// 主干 `xaml.cs:8572` / `main.rs:32257` 的图片槽。**为何不在 shellfiles.rs**：见模块头
/// 「与 shellfiles.rs 的关系」——那边明确不收素材文件，main.rs 那对是 bin 常量够不着。
pub const SKIN_IMG_FILE: &str = "shell-skin.img";
/// 主干 `xaml.cs:8569` / `main.rs:32258` 的视频槽（Wallpaper Engine 导出的 MP4）。
pub const SKIN_VIDEO_FILE: &str = "shell-skin.video";

/// 海报缓存目录（`DataHome\cache`，主干同款子目录，**已存在**：本模块不建也不判它）。
const CACHE_DIR: &str = "cache";
/// 海报缓存文件名。上层 `Image::source_file` 直接吃它。
pub const POSTER_FILE: &str = "shell-skin.poster.bmp";
/// meta sidecar 的扩展名：poster 同 stem（`shell-skin.poster.bmp` → `shell-skin.poster.meta`）。
const META_SUFFIX: &str = "meta";
/// 连续「ReadSample 成功但样本为空」的容忍上限（流头里换型/标记事件会先吐几发空）。
const MAX_EMPTY_SAMPLES: u32 = 64;
/// 第一视频流的流号（windows 0.62 把它包成 `MF_SOURCE_READER_CONSTANTS(i32)`，
/// 方法签名吃 `u32` ⇒ 常量期折一次，调用点不再各自 `as u32`）。
const FIRST_VIDEO_STREAM: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;

// ============================== 纯路径与指纹 ==============================

/// 海报缓存落点 = `data_home\cache\shell-skin.poster.bmp`（拼接一律 `join`，不手写分隔符）。
#[must_use]
pub fn poster_cache_path(data_home: &Path) -> PathBuf {
    data_home.join(CACHE_DIR).join(POSTER_FILE)
}

/// meta sidecar 落点：与 poster **同 stem** 换 [`META_SUFFIX`] 扩展名。
fn meta_path(poster: &Path) -> PathBuf {
    poster.with_extension(META_SUFFIX)
}

/// 视频槽的来源指纹：mtime（自 UNIX_EPOCH 的纳秒，可为负）+ 字节数。
/// 负值只出现在 1970 前的时间戳上，Windows 文件系统不会真给，但类型上保留符号。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceStamp {
    mtime_nanos: i128,
    bytes: u64,
}

/// [`SystemTime`] → 带符号纳秒（早于纪元取负，两支都不丢精度）。
fn system_time_nanos(t: SystemTime) -> i128 {
    match t.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => d.as_nanos() as i128,
        Err(e) => -(e.duration().as_nanos() as i128),
    }
}

/// 读视频槽的当前指纹（mtime + 字节数）。读不到 = 抽帧前置失败，消息归「解不了」族外
/// 的显式分支（video 槽在 [`ensure_poster`] 里已判 `is_file`，走到这里只有竞态/权限）。
fn source_stamp_of(video: &Path) -> Result<SourceStamp, String> {
    let md = std::fs::metadata(video)
        .map_err(|e| format!("解不了：读不到视频元数据 {}：{e}", video.display()))?;
    let mtime = md
        .modified()
        .map_err(|e| format!("解不了：视频 mtime 读不出 {}：{e}", video.display()))?;
    Ok(SourceStamp { mtime_nanos: system_time_nanos(mtime), bytes: md.len() })
}

/// **写** meta：一行 `<mtime 纳秒> <字节数>\n`（裸文本，无转义空间，同 shellfiles 的手写纪律）。
fn write_source_stamp(poster: &Path, stamp: SourceStamp) -> std::io::Result<()> {
    std::fs::write(meta_path(poster), format!("{} {}\n", stamp.mtime_nanos, stamp.bytes))
}

/// **读** meta：缺文件/坏行/解析不动一律 `None`（[`poster_stale`] 判 stale ⇒ 自愈重抽）。
fn read_source_stamp(poster: &Path) -> Option<SourceStamp> {
    let text = std::fs::read_to_string(meta_path(poster)).ok()?;
    let mut fields = text.split_whitespace();
    let mtime_nanos = fields.next()?.parse::<i128>().ok()?;
    let bytes = fields.next()?.parse::<u64>().ok()?;
    Some(SourceStamp { mtime_nanos, bytes })
}

/// poster 是否过期（**判据宁 stale 勿 fresh**，四档全 true）：
/// poster 不存在；meta 缺失/坏档；video 的 mtime 或字节数与 meta 记录不符；video 读不到。
#[must_use]
pub fn poster_stale(video: &Path, poster: &Path) -> bool {
    if !poster.is_file() {
        return true;
    }
    let Some(stamp) = read_source_stamp(poster) else {
        return true;
    };
    match std::fs::metadata(video) {
        Ok(md) => match md.modified() {
            Ok(t) => system_time_nanos(t) != stamp.mtime_nanos || md.len() != stamp.bytes,
            Err(_) => true,
        },
        Err(_) => true,
    }
}

// ============================== BMP 组装（纯核，测试逐字节钉） ==============================

/// 小端 u16/u32/i32 追加（BMP 头是 LE 定宽字段，手写 54 字节，见模块头「依赖版本备案」）。
fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put_i32(out: &mut Vec<u8>, v: i32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// `width × height` 的 **BGRA、top-down、行宽恰 width*4**（32bpp 无行尾填充）像素缓冲
/// → 完整 BMP 文件字节：`BITMAPFILEHEADER`(14B) + `BITMAPINFOHEADER`(40B) + 像素。
/// 头要点：**负高**（top-down，第 0 行 = 屏幕最上行）、planes=1、bpp=32、BI_RGB=0、
/// `bfSize = 54 + 像素`、`bfOffBits = 54`、`biSizeImage = 像素`。
/// 入参长度必须恰为 `width*4*height`（内部不变量，违者 panic 是调用方的 bug）。
///
/// `pub(crate)`：播放帧泵（[`crate::skinplay`]）拿它把每帧 BGRA 包成 BMP 字节喂
/// `Image::source_data` —— 与海报走**同一个** 54 字节手写头，不另起第二份拼法。
pub(crate) fn bmp_bytes(width: u32, height: u32, bgra_top_down: &[u8]) -> Vec<u8> {
    let expected = (width as usize) * 4 * (height as usize);
    assert_eq!(bgra_top_down.len(), expected, "像素缓冲必须恰为 width*4*height");
    let pixel_len = expected as u32;
    let mut out = Vec::with_capacity(54 + expected);
    // ---- BITMAPFILEHEADER（14 B）----
    out.extend_from_slice(b"BM");
    put_u32(&mut out, 54 + pixel_len); // bfSize
    put_u16(&mut out, 0); // bfReserved1
    put_u16(&mut out, 0); // bfReserved2
    put_u32(&mut out, 54); // bfOffBits
    // ---- BITMAPINFOHEADER（40 B）----
    put_u32(&mut out, 40); // biSize
    put_i32(&mut out, width as i32); // biWidth
    put_i32(&mut out, -(height as i32)); // biHeight：**负** = top-down
    put_u16(&mut out, 1); // biPlanes
    put_u16(&mut out, 32); // biBitCount
    put_u32(&mut out, 0); // biCompression = BI_RGB
    put_u32(&mut out, pixel_len); // biSizeImage
    put_i32(&mut out, 0); // biXPelsPerMeter
    put_i32(&mut out, 0); // biYPelsPerMeter
    put_u32(&mut out, 0); // biClrUsed
    put_u32(&mut out, 0); // biClrImportant
    // ---- 像素：行序与行内字节**原样**（来源已是 top-down BGRA）----
    out.extend_from_slice(bgra_top_down);
    out
}

/// 把「首行指针 + stride」的帧缓冲**逐行**拷成紧凑 top-down 缓冲（行宽 `width*4`）。
/// stride 为负 = bottom-up 源（第 0 行在内存末），按 `(height-1-y)` 反演取行 ——
/// 绝不假设内存连续，也不假设方向。
///
/// # Safety
/// `ptr` 必须可读 `|stride|*(height-1) + width*4` 字节（Lock2D/Lock 的契约）。
pub(crate) unsafe fn copy_strided_rows(
    ptr: *const u8,
    stride: isize,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let row = (width as usize) * 4;
    let stride_abs = stride.unsigned_abs() as usize;
    let mut out = vec![0u8; row * (height as usize)];
    for y in 0..height as usize {
        let offset = if stride < 0 { (height as usize - 1 - y) * stride_abs } else { y * stride_abs };
        let src = unsafe { std::slice::from_raw_parts(ptr.add(offset), row) };
        out[y * row..(y + 1) * row].copy_from_slice(src);
    }
    out
}

// ============================== COM / MF 生命周期（RAII，绝不泄漏） ==============================

/// `CoInitializeEx(COINIT_MULTITHREADED)` 的守卫：只有**本调用真的加了计数**（S_OK/S_FALSE）
/// 才在 Drop 里 `CoUninitialize`；线程已在 STA（`RPC_E_CHANGED_MODE`）时容忍并**不接管**
/// 别人的计数 —— MF 在 STA 线程上同样可用（SourceReader 不要求 MTA）。
///
/// `pub(crate)`：播放帧泵（[`crate::skinplay`]）的解码专线线程自己开一份 —— 那根线程
/// 由 `std::thread` 起，COM/MF 计数与壳的 UI 线程**互不相干**，各自成对。
pub(crate) struct ComGuard(pub(crate) bool);

impl ComGuard {
    pub(crate) fn init() -> Result<Self, String> {
        // SAFETY：CoInitializeEx 无特殊前提；返回值按 HRESULT 三档分派。
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr == RPC_E_CHANGED_MODE {
            Ok(Self(false))
        } else if hr.is_err() {
            Err(format!("解不了：CoInitializeEx 失败 0x{:08X}", hr.0 as u32))
        } else {
            Ok(Self(true))
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY：与上面 is_ok 分支的 CoInitializeEx 成对。
            unsafe { CoUninitialize() };
        }
    }
}

/// `MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET)` 的守卫：Drop `MFShutdown`（一一成对）。
/// `pub(crate)` 同 [`ComGuard`]：帧泵的解码线程自持一份。
pub(crate) struct MfGuard;

impl MfGuard {
    pub(crate) fn init() -> Result<Self, String> {
        // SAFETY：MFStartup 只要求 COM 已初始化（ComGuard 先行）。
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET) }
            .map_err(|e| format!("解不了：MFStartup 失败 {}", format_hresult(&e)))?;
        Ok(Self)
    }
}

impl Drop for MfGuard {
    fn drop(&mut self) {
        // SAFETY：与 init 里的 MFStartup 成对；Drop 无处报错，关停失败无可为（计数对齐即可）。
        let _ = unsafe { MFShutdown() };
    }
}

/// windows-core 错误 → `0x%08X + 文本`（HRESULT 十六进制带进消息，便于对 SDK 头查码）。
/// `pub(crate)` 同 [`ComGuard`]：帧泵的解码线程也要折 HRESULT。
pub(crate) fn format_hresult(e: &windows::core::Error) -> String {
    format!("0x{:08X}（{}）", e.code().0 as u32, e.message())
}

// ============================== 算子层：RGB32 reader（海报与播放共用） ==============================

/// 一枚**长命**的 RGB32 Source Reader：开一次容器、切一次输出类型，之后逐帧
/// [`Rgb32Reader::next_rows`]，播完 [`Rgb32Reader::rewind`] 回起点循环。
///
/// # 为何抽成公共件
///
/// 海报抽帧（[`poster_from_video`]）与视频播放（[`crate::skinplay`] 的帧泵）共用**同一套**
/// MF 开门纪律 —— 而那套纪律是拿真机 38 MB H.264 MP4 实测换来的，见模块头
/// 「MF 抽帧纪律」与「⚠ 与简报『裸类型两步法』的偏差」两条：① reader 必须带
/// `MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING`；② 输出类型必须**一步切全类型**
/// （major + subtype 一起设）。两份实现各抄一遍 = 两处可能各自漂移，故收在此处单一真相。
///
/// # `max_edge`
///
/// 输出帧的长边上限（像素，**保宽高比**缩放，由 MF 视频处理器 MFT 完成）。`0` = 不缩放，
/// 用容器原生尺寸。海报要全尺寸（静态展示的清晰度），播放要压档（每帧都要过一次
/// BMP 打包 + WIC 解码 + 贴图上传，4K 帧撑不住 24fps）。
pub(crate) struct Rgb32Reader {
    reader: IMFSourceReader,
    width: u32,
    height: u32,
    /// `MF_MT_DEFAULT_STRIDE` 的缓存（连续缓冲分支每次都要用，开 reader 时取一次即可）。
    default_stride: i32,
}

impl Rgb32Reader {
    /// 开容器 → 定流 → 切 RGB32 → 读帧尺寸。错误档同 [`poster_from_video`]：
    /// 「解不了」/「没有视频流」。
    pub(crate) fn open(video: &Path, max_edge: u32) -> Result<Self, String> {
        // PCWSTR：OsStr 宽字符直通（路径不必是 UTF-8），带 NUL。
        let wide: Vec<u16> = video
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // reader 属性：**必须**开 MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING。
        // 实测备案（本机 10.0.26220，38 MB H.264 MP4）：H.264 decoder 直出 NV12 正常，但
        // H.264→RGB32 的颜色转换腿在不开 AVP 时整条被拒（0xC00D36B4，老式颜色转换 DMO
        // 已缺席）—— RGB32 必须靠视频处理器 MFT，那需要这枚属性。
        let mut attrs_slot: Option<IMFAttributes> = None;
        unsafe { MFCreateAttributes(&mut attrs_slot, 0) }
            .map_err(|e| format!("解不了：MFCreateAttributes 失败 {}", format_hresult(&e)))?;
        let attrs = attrs_slot
            .ok_or_else(|| "解不了：MFCreateAttributes 没交回属性".to_string())?;
        // SAFETY：attrs 是上一步刚创建的本地接口。
        unsafe { attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1) }
            .map_err(|e| format!("解不了：设 AVP 属性失败 {}", format_hresult(&e)))?;

        // SAFETY：wide 以 NUL 结尾且生命周期盖过 reader 的构造；attrs 是本地有效接口。
        let reader = unsafe { MFCreateSourceReaderFromURL(PCWSTR::from_raw(wide.as_ptr()), &attrs) }
            .map_err(|e| {
                format!(
                    "解不了：打不开视频容器 {}：{}",
                    video.display(),
                    format_hresult(&e)
                )
            })?;

        // ① 流存在性：第一视频流的 native 类型拿不到 = 这容器里没有视频流。
        // SAFETY：reader 有效性由 MFCreateSourceReaderFromURL 的 Ok 保证。
        let native = unsafe { reader.GetNativeMediaType(FIRST_VIDEO_STREAM, 0) }
            .map_err(|_| format!("没有视频流：{} 里找不到第一视频流", video.display()))?;
        let native_size = unsafe { native.GetUINT64(&MF_MT_FRAME_SIZE) }.unwrap_or(0);
        drop(native);

        // ② 输出类型：**一步切全类型 RGB32**（major + subtype）。
        // ⚠ 偏差备案：简报 prescribe 的「裸类型两步法」（先只设 MF_MT_MAJOR_TYPE 让 decoder
        // 起来，再切 RGB32）在本机实测被 `SetCurrentMediaType` 以 0x80070057（参数错误）拒
        // —— 半类型已被今日的 Source Reader 拒收（AVP 开关无涉）。实测可行组合 = AVP reader
        // + 一步全类型：decoder 由 Source Reader 自动装载（冒烟证实：输出 sub=RGB32、帧尺寸
        // 可读、第一帧非空），decoder 加载这一步并不因省掉半类型而丢失。
        // SAFETY：以下 COM 调用只互传本流程取得的接口指针。
        let out_type = unsafe {
            let full = MFCreateMediaType()
                .map_err(|e| format!("解不了：MFCreateMediaType 失败 {}", format_hresult(&e)))?;
            full.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                .map_err(|e| format!("解不了：设 MF_MT_MAJOR_TYPE 失败 {}", format_hresult(&e)))?;
            full.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)
                .map_err(|e| format!("解不了：设 MF_MT_SUBTYPE=RGB32 失败 {}", format_hresult(&e)))?;
            // ③ 缩放：max_edge > 0 时把 MF_MT_FRAME_SIZE 设成保比例的收窄尺寸，
            //    视频处理器 MFT 在 SetCurrentMediaType 时一并接走。原生尺寸读不出来
            //    （unwrap_or(0)）⇒ 不设，让 reader 自己定，绝不瞎编一个尺寸。
            let (native_w, native_h) = ((native_size >> 32) as u32, native_size as u32);
            if max_edge > 0 && native_w > 0 && native_h > 0 {
                let (out_w, out_h) = fit_within(native_w, native_h, max_edge);
                if (out_w, out_h) != (native_w, native_h) {
                    let packed = ((out_w as u64) << 32) | out_h as u64;
                    full.SetUINT64(&MF_MT_FRAME_SIZE, packed)
                        .map_err(|e| format!("解不了：设输出帧尺寸失败 {}", format_hresult(&e)))?;
                }
            }
            reader
                .SetCurrentMediaType(FIRST_VIDEO_STREAM, None, &full)
                .map_err(|e| {
                    format!(
                        "解不了：切 RGB32 失败（容器/解码器不支持）{}",
                        format_hresult(&e)
                    )
                })?;
            reader
                .GetCurrentMediaType(FIRST_VIDEO_STREAM)
                .map_err(|e| format!("解不了：拿输出媒体类型失败 {}", format_hresult(&e)))?
        };

        // ④ 帧宽高从**输出**媒体类型读（decoder/视频处理器实际给的尺寸，可能与容器声明不同）。
        // SAFETY：out_type 是本流程独占的接口。
        let frame_size = unsafe { out_type.GetUINT64(&MF_MT_FRAME_SIZE) }
            .map_err(|e| format!("解不了：输出类型缺 MF_MT_FRAME_SIZE {}", format_hresult(&e)))?;
        let width = (frame_size >> 32) as u32;
        let height = frame_size as u32;
        if width == 0 || height == 0 {
            return Err(format!("解不了：输出帧尺寸非法 {width}x{height}"));
        }
        let default_stride = unsafe { out_type.GetUINT32(&MF_MT_DEFAULT_STRIDE) }
            .map(|stride| stride as i32)
            .unwrap_or((width * 4) as i32);
        drop(out_type);

        Ok(Self {
            reader,
            width,
            height,
            default_stride,
        })
    }

    /// 输出帧尺寸（宽, 高）。
    pub(crate) fn geometry(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// 读**下一帧**：紧凑 top-down BGRA（行宽 `width*4`）。`Ok(None)` = 流尾（EOS）。
    ///
    /// 空样本（换型/标记事件）连吐 [`MAX_EMPTY_SAMPLES` 发仍不给帧才算流坏。
    pub(crate) fn next_rows(&mut self) -> Result<Option<Vec<u8>>, String> {
        for _ in 0..MAX_EMPTY_SAMPLES {
            let mut flags = 0u32;
            let mut sample: Option<IMFSample> = None;
            // SAFETY：流号/旗标/样本指针都是本流程局部量。
            unsafe {
                self.reader
                    .ReadSample(FIRST_VIDEO_STREAM, 0, None, Some(&mut flags), None, Some(&mut sample))
            }
            .map_err(|e| format!("解不了：ReadSample 失败 {}", format_hresult(&e)))?;
            let Some(sample) = sample else {
                if flags & (MF_SOURCE_READERF_ENDOFSTREAM.0 as u32) != 0 {
                    return Ok(None);
                }
                continue;
            };
            // 像素三档 stride：Lock2D（自带 stride）→ 连续缓冲 + MF_MT_DEFAULT_STRIDE → width*4。
            // SAFETY：sample.GetBufferByIndex(0) 的缓冲是本流程独占；Lock/Unlock 严格成对，
            // Lock2D 分支只配 Unlock2D（不与 Lock/Unlock 混用，MF 契约如此）。
            let rows = unsafe {
                let buffer: IMFMediaBuffer = sample
                    .GetBufferByIndex(0)
                    .map_err(|e| format!("解不了：样本没有像素缓冲 {}", format_hresult(&e)))?;
                if let Ok(lock2d) = buffer.cast::<IMF2DBuffer>() {
                    let mut scanline0: *mut u8 = std::ptr::null_mut();
                    let mut stride: i32 = 0;
                    lock2d
                        .Lock2D(&mut scanline0, &mut stride)
                        .map_err(|e| format!("解不了：Lock2D 失败 {}", format_hresult(&e)))?;
                    let rows = copy_strided_rows(scanline0, stride as isize, self.width, self.height);
                    let _ = lock2d.Unlock2D();
                    rows
                } else {
                    let contiguous = sample
                        .ConvertToContiguousBuffer()
                        .map_err(|e| {
                            format!("解不了：ConvertToContiguousBuffer 失败 {}", format_hresult(&e))
                        })?;
                    let mut data: *mut u8 = std::ptr::null_mut();
                    let mut byte_len = 0u32;
                    contiguous
                        .Lock(&mut data, None, Some(&mut byte_len))
                        .map_err(|e| format!("解不了：缓冲 Lock 失败 {}", format_hresult(&e)))?;
                    // 负值 = bottom-up 源，copy_strided_rows 统一摆正。
                    let stride = if self.default_stride == 0 {
                        (self.width * 4) as i32
                    } else {
                        self.default_stride
                    };
                    let rows = copy_strided_rows(data, stride as isize, self.width, self.height);
                    let _ = contiguous.Unlock();
                    rows
                }
            };
            return Ok(Some(rows));
        }
        Err(format!(
            "解不了：连续 {MAX_EMPTY_SAMPLES} 发空样本，流头解不出帧"
        ))
    }

    /// 回起点（循环播放用）。`GUID_NULL` = 默认时间格式，位置 0 = 容器开头，
    /// 单位 100ns（`VT_I8`）。seek 失败不致命 —— 调用方可以就此停泵。
    pub(crate) fn rewind(&mut self) -> Result<(), String> {
        let position: PROPVARIANT = 0i64.into();
        // SAFETY：position 是本函数局部量，生命周期盖过这次调用；GUID_NULL 是常量。
        unsafe { self.reader.SetCurrentPosition(&GUID::zeroed(), &position) }
            .map_err(|e| format!("解不了：回起点失败 {}", format_hresult(&e)))
    }
}

/// 把 `w × h` 收进「长边 ≤ `max_edge`」的等比尺寸（向下取整；原图已合规则原样返回）。
/// 纯函数，无 MF 依赖 —— 单独可测。
fn fit_within(w: u32, h: u32, max_edge: u32) -> (u32, u32) {
    let long = w.max(h);
    if long <= max_edge {
        return (w, h);
    }
    // 长边对齐 max_edge，短边按比例缩；两边各自至少 1（缩放比 > 0，取整后不可能为 0，
    // 这里 .max(1) 只是把「取整成 0」这条理论路径堵死，不让下游判非法尺寸）。
    let scale = max_edge as f64 / long as f64;
    (
        ((w as f64 * scale).round() as u32).max(1),
        ((h as f64 * scale).round() as u32).max(1),
    )
}

// ============================== 算子层：抽第一帧 ==============================

/// 从 `video` 抽**第一帧**，写成 32bpp top-down BMP 到 `poster`，并把抽帧那一刻的
/// 来源指纹（mtime + 字节数）写进 poster 旁的 meta —— 两者**成对落盘**：任何一次成功
/// 调用后 [`poster_stale`] 即为 false。BMP/meta 任一写不进都归「写盘失败」。
///
/// 错误三档（前缀可分辨，见模块头）：「解不了」/「没有视频流」/「写盘失败」。
///
/// `max_edge = 0`：海报要**全尺寸**（静态展示的清晰度），不缩放。
pub fn poster_from_video(video: &Path, poster: &Path) -> Result<(), String> {
    // 指纹**先**取：抽的是这一刻的帧；若之后视频又变，mtime 记的是旧值 ⇒ 下轮判 stale 重抽，方向正确。
    let stamp = source_stamp_of(video)?;

    let _com = ComGuard::init()?;
    let _mf = MfGuard::init()?;

    // 开门 + 切 RGB32 + 读帧尺寸：与播放帧泵同一条纪律（见 Rgb32Reader 的「为何抽成公共件」）。
    let mut reader = Rgb32Reader::open(video, 0)?;
    let (width, height) = reader.geometry();

    // ⑤ 第一帧非空 sample：空样本只可能是换型/标记事件，连续超限 = 流坏。
    let Some(rows) = reader.next_rows()? else {
        return Err(format!("没有视频流：{} 读到流尾仍无任何帧", video.display()));
    };

    // ⑥ 落盘：BMP 成对 meta。写不进 = 「写盘失败」（cache 目录缺席也在这一档显形）。
    let bmp = bmp_bytes(width, height, &rows);
    std::fs::write(poster, bmp).map_err(|e| format!("写盘失败：{}：{e}", poster.display()))?;
    write_source_stamp(poster, stamp)
        .map_err(|e| format!("写盘失败：{}：{e}", meta_path(poster).display()))?;
    Ok(())
}

// ============================== 组合层：判槽 + 判新鲜 ==============================

/// 视频皮肤的海报就绪判定（组合层，**无全局态**，槽位判据 = 主干 `LoadSavedSkin` 的序）：
///
/// * 视频槽（`data_home\shell-skin.video`）不在 → `Ok(None)`：条件不满足，不是失败；
/// * 图片槽（`shell-skin.img`）在 → `Ok(None)`：**图片优先**（主干视频只在无图槽时接管）；
/// * poster 新鲜（[`poster_stale`] = false）→ `Ok(Some(poster))`，零 MF 调用；
/// * poster stale → 抽帧（[`poster_from_video`]）后 `Ok(Some(poster))`；
/// * 抽帧失败 → `Err`（调用方拿消息自处置：降级无背景 / 记日志，本模块不吞）。
///
/// 注意本函数**不建** `cache` 目录（契约：目录已存在；缺席会让 poster 写盘报「写盘失败」，
/// 显形好过静默兜底）。
pub fn ensure_poster(data_home: &Path) -> Result<Option<PathBuf>, String> {
    let video = data_home.join(SKIN_VIDEO_FILE);
    if !video.is_file() {
        return Ok(None);
    }
    let img = data_home.join(SKIN_IMG_FILE);
    if img.is_file() {
        return Ok(None);
    }
    let poster = poster_cache_path(data_home);
    if !poster_stale(&video, &poster) {
        return Ok(Some(poster));
    }
    poster_from_video(&video, &poster)?;
    Ok(Some(poster))
}

// ============================== 测试 ==============================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::FileTimes;
    use std::time::Duration;

    /// 落盘类测试走 `std::env::temp_dir()` 下「进程内唯一 + 每测唯一」的子目录，
    /// 语义仿 `shellfiles.rs::sandbox`（`sf1-<tag>-<pid>`），自建自删，
    /// **绝不碰真 `%LOCALAPPDATA%\Blade2`**（唯一例外见真机冒烟那颗，只读它的视频槽）。
    fn sandbox(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("skv-{tag}-{:?}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn write_bytes(path: &Path, bytes: &[u8]) {
        std::fs::write(path, bytes).expect("测试写盘失败");
    }

    /// 用 std::fs::FileTimes 把 mtime 拨到 `epoch + secs`（确定性，不赌写盘时钟粒度）。
    fn set_mtime(path: &Path, secs: u64) {
        let file = std::fs::OpenOptions::new().append(true).open(path).expect("打开失败");
        file.set_times(FileTimes::new().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(secs)))
            .expect("set_times 失败");
    }

    fn current_stamp(video: &Path) -> SourceStamp {
        source_stamp_of(video).expect("读指纹失败")
    }

    /// 2×3 假 BGRA：行内像素各不相同（行内序可验），三行内容互不相同（行序可验）。
    fn fake_bgra_2x3() -> Vec<u8> {
        let mut px = Vec::new();
        for y in 0u8..3 {
            for x in 0u8..2 {
                px.extend_from_slice(&[x * 10 + 1, y * 10 + 2, x + y + 3, 0xFF]);
            }
        }
        px
    }

    // ---------- 1. BMP 头 ----------

    /// 文件头 14 B（`BM` / bfSize / offbits=54）+ 信息头 40 B（负高 -3、planes=1、bpp=32、
    /// BI_RGB=0、biSizeImage=像素数），且像素区行序与行内 BGRA **原样**。
    #[test]
    fn the_bmp_header_carries_negative_height_and_32bpp() {
        let px = fake_bgra_2x3();
        let bmp = bmp_bytes(2, 3, &px);
        assert_eq!(bmp.len(), 54 + 24);
        // BITMAPFILEHEADER
        assert_eq!(&bmp[0..2], b"BM");
        assert_eq!(u32::from_le_bytes(bmp[2..6].try_into().unwrap()), 78, "bfSize = 54+24");
        assert_eq!(u16::from_le_bytes(bmp[6..8].try_into().unwrap()), 0);
        assert_eq!(u16::from_le_bytes(bmp[8..10].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(bmp[10..14].try_into().unwrap()), 54, "bfOffBits");
        // BITMAPINFOHEADER
        assert_eq!(u32::from_le_bytes(bmp[14..18].try_into().unwrap()), 40, "biSize");
        assert_eq!(i32::from_le_bytes(bmp[18..22].try_into().unwrap()), 2, "biWidth");
        assert_eq!(i32::from_le_bytes(bmp[22..26].try_into().unwrap()), -3, "负高 = top-down");
        assert_eq!(u16::from_le_bytes(bmp[26..28].try_into().unwrap()), 1, "biPlanes");
        assert_eq!(u16::from_le_bytes(bmp[28..30].try_into().unwrap()), 32, "biBitCount");
        assert_eq!(u32::from_le_bytes(bmp[30..34].try_into().unwrap()), 0, "BI_RGB");
        assert_eq!(u32::from_le_bytes(bmp[34..38].try_into().unwrap()), 24, "biSizeImage");
        assert!(bmp[38..54].iter().all(|&b| b == 0), "ppm/clr 四字段为 0");
        // 像素区：行序 + 行内字节原样
        assert_eq!(&bmp[54..], &px[..]);
    }

    /// 行拷贝按 stride 逐行：正 stride = 直取；负 stride（bottom-up 源）= 行序摆正，
    /// 两支都**不假设内存连续**（行尾的 stride 余量字节绝不混进产物）。
    #[test]
    fn strided_rows_copy_honours_both_stride_signs() {
        let row = [1u8, 2, 3, 4, 5, 6, 7, 8]; // 一行 2 px = 8 B
        // 正 stride：行尾垫 4 B 垃圾，必须被跳过
        let mut padded = row.to_vec();
        padded.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        let mut second = row.to_vec();
        second[0] = 9;
        padded.extend_from_slice(&second);
        let rows = unsafe { copy_strided_rows(padded.as_ptr(), 12, 2, 2) };
        assert_eq!(rows, [row.as_slice(), second.as_slice()].concat());
        // 负 stride：内存里第二行在前（bottom-up），产物按 top-down 摆正
        let mut bottom_up = second.clone();
        bottom_up.extend_from_slice(&row);
        let rows = unsafe { copy_strided_rows(bottom_up.as_ptr(), -8, 2, 2) };
        assert_eq!(rows, [row.as_slice(), second.as_slice()].concat());
    }

    // ---------- 2. meta sidecar 与 poster_stale ----------

    /// meta 同 stem（`.poster.meta`）、读写 round-trip 一字不差；poster 在而 meta 缺失
    /// ⇒ stale（宁 stale 勿 fresh）。
    #[test]
    fn the_meta_sidecar_round_trips_the_source_stamp() {
        let dir = sandbox("meta-roundtrip");
        let poster = dir.join(POSTER_FILE);
        write_bytes(&poster, b"BM-fake");
        assert_eq!(
            meta_path(&poster),
            dir.join("shell-skin.poster.meta"),
            "meta 与 poster 同 stem，只换扩展名"
        );
        let stamp = SourceStamp { mtime_nanos: 172_819_200_123_456_789, bytes: 38_848_798 };
        write_source_stamp(&poster, stamp).expect("写 meta 失败");
        assert!(meta_path(&poster).is_file());
        assert_eq!(read_source_stamp(&poster), Some(stamp));
        // 坏档 → None（判 stale 的自愈前提）
        write_bytes(&meta_path(&poster), b"not a stamp");
        assert_eq!(read_source_stamp(&poster), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 四档判据：缺 poster=true；mtime 变=true / 不变=false；video 换字节数=true（mtime 对齐
    /// 也要 stale，证明**字节数**独立入判）；video 读不到=true。
    #[test]
    fn the_poster_is_stale_when_missing_or_source_drifts() {
        let dir = sandbox("stale");
        let video = dir.join("v.mp4");
        let poster = dir.join(POSTER_FILE);
        // 缺 poster
        write_bytes(&video, b"one");
        assert!(poster_stale(&video, &poster), "poster 不存在 ⇒ stale");
        // 新鲜基线：poster + meta 对齐当前 mtime/字节数
        write_bytes(&poster, b"BM-fake");
        write_source_stamp(&poster, current_stamp(&video)).expect("写 meta 失败");
        assert!(!poster_stale(&video, &poster), "mtime/字节数都对齐 ⇒ 不 stale");
        // mtime 变（字节数不变）
        set_mtime(&video, 3_600);
        assert!(poster_stale(&video, &poster), "mtime 变 ⇒ stale");
        // 恢复新鲜，再只换字节数（stamp 的 mtime 用**新的**，字节数记旧的旧值）
        write_source_stamp(&poster, current_stamp(&video)).expect("写 meta 失败");
        write_bytes(&video, b"one-two");
        write_source_stamp(&poster, SourceStamp { mtime_nanos: current_stamp(&video).mtime_nanos, bytes: 3 })
            .expect("写 meta 失败");
        assert!(poster_stale(&video, &poster), "字节数对不上 ⇒ stale");
        // video 读不到
        assert!(poster_stale(&dir.join("absent.mp4"), &poster), "video 缺席 ⇒ stale");
        // meta 缺失（poster 在）
        let _ = std::fs::remove_file(meta_path(&poster));
        assert!(poster_stale(&video, &poster), "meta 缺失 ⇒ stale");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---------- 3. ensure_poster 条件 ----------

    /// 条件矩阵：无视频槽 → Ok(None)；视频+图片两槽都在 → Ok(None)（**图片优先**，不碰 MF）；
    /// 视频槽在、图片槽不在、poster 新鲜 → Ok(Some)（同样零 MF 调用）。
    #[test]
    fn ensure_poster_requires_video_slot_and_yields_to_image_slot() {
        let dir = sandbox("ensure");
        let cache = dir.join(CACHE_DIR);
        std::fs::create_dir_all(&cache).expect("建 cache 失败");
        let poster = cache.join(POSTER_FILE);
        // 无视频槽
        assert_eq!(ensure_poster(&dir), Ok(None), "无视频槽 = 条件不满足，非失败");
        // 视频槽在 + 图片槽在：图片优先
        write_bytes(&dir.join(SKIN_VIDEO_FILE), b"video-bytes");
        write_bytes(&dir.join(SKIN_IMG_FILE), b"img-bytes");
        assert_eq!(ensure_poster(&dir), Ok(None), "图片槽优先");
        // 图片槽撤掉 + poster 新鲜：直接回 Some，不抽帧
        let _ = std::fs::remove_file(dir.join(SKIN_IMG_FILE));
        write_bytes(&poster, b"BM-fake");
        write_source_stamp(&poster, current_stamp(&dir.join(SKIN_VIDEO_FILE))).expect("写 meta 失败");
        assert_eq!(ensure_poster(&dir), Ok(Some(poster.clone())));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---------- 4. 真机抽帧冒烟（条件执行） ----------

    /// `%LOCALAPPDATA%\Blade2\shell-skin.video`（真机约 38 MB 的 H.264 MP4）**存在时才执行**：
    /// 抽到沙箱 cache，断言产物是合法 BMP（头两字节 `BM`、`bfSize` = 文件长 = 54 + stride×|高|）。
    /// 文件不在 → 打一行说明提前 return，测试仍过（不造 38 MB 假货、不挂 CI）。
    #[test]
    fn the_real_video_slot_smoke_extracts_a_legal_bmp() {
        let Some(video) = std::env::var_os("LOCALAPPDATA").map(|root| {
            PathBuf::from(root).join("Blade2").join(SKIN_VIDEO_FILE)
        }) else {
            eprintln!("smoke 跳过：LOCALAPPDATA 不可用");
            return;
        };
        if !video.is_file() {
            eprintln!("smoke 跳过：{} 不存在（真机视频皮肤未装）", video.display());
            return;
        }
        let dir = sandbox("smoke");
        let cache = dir.join(CACHE_DIR);
        std::fs::create_dir_all(&cache).expect("建 cache 失败");
        // 真机视频**不拷**（38 MB）：沙箱里只放 poster 落点，直接测算子层
        // [`poster_from_video`] —— ensure_poster 的槽位判据由
        // `ensure_poster_requires_video_slot_and_yields_to_image_slot` 钉住，这里不重复。
        let poster = poster_cache_path(&dir);

        let started = std::time::Instant::now();
        let outcome = poster_from_video(&video, &poster);
        let elapsed = started.elapsed();
        outcome.unwrap_or_else(|e| panic!("真机抽帧失败：{e}"));

        let bmp = std::fs::read(&poster).expect("读海报失败");
        assert_eq!(&bmp[0..2], b"BM", "BMP 魔数");
        let bf_size = u32::from_le_bytes(bmp[2..6].try_into().unwrap());
        assert_eq!(bf_size as usize, bmp.len(), "bfSize = 文件实际长度");
        let width = i32::from_le_bytes(bmp[18..22].try_into().unwrap());
        let height = i32::from_le_bytes(bmp[22..26].try_into().unwrap());
        assert_eq!(u16::from_le_bytes(bmp[28..30].try_into().unwrap()), 32, "bpp=32");
        assert_eq!(u32::from_le_bytes(bmp[30..34].try_into().unwrap()), 0, "BI_RGB");
        let stride = width * 4;
        assert_eq!(
            bf_size,
            (54 + (stride as i64 * (-(height as i64))) as u32),
            "bfSize = 54 + stride×|高|"
        );
        // meta 必须随 BMP 成对落盘，且与真机视频对齐（下轮启动不再重抽）
        assert!(!poster_stale(&video, &poster), "抽完即新鲜");
        eprintln!(
            "smoke OK：{} 帧 {}x{}（height={}），BMP {} 字节，抽帧 + 落盘耗时 {:.1?}",
            video.display(),
            width,
            height.abs(),
            height,
            bmp.len(),
            elapsed
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
