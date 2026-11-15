//! 附件链路的**纯逻辑层**（主干 `MainWindow.xaml.cs` 的 `AttachmentVm` /
//! `SniffImageMediaType` / `NormalizeImageAsync` / `TranscodeToPngAsync` 那一族）。
//!
//! 本模块不碰控件、不发 RPC、不开线程——与 [`crate::filespanel`] 同一条仓规：
//! 回执解析、判据、转码全部离线单测，`main.rs` 只做接线。
//!
//! # 管什么
//!
//! * [`Attachment`] —— 附件条目（对齐主干 `AttachmentVm`：图片带 base64 与 mediaType，
//!   文件带本地路径），发送侧 content 数组的构造原料。
//! * [`sniff_image_media_type`] —— 按魔数认 png/jpeg/webp/gif 四种（内核只收这四种，
//!   且按声明的 mediaType 与字节实际格式**严格比对**：把 BMP/DIB 按png 上报会被
//!   sharp 拒成 `session/attachment-invalid`——主干 `SniffImageMediaType` 的注释）。
//! * [`normalize_image`] —— 任意来源字节 → 内核必收的 `(bytes, mediaType, name)`；
//!   嗅探命中直返，BMP/DIB（剪贴位图流、.bmp 文件的常见形态）转码成 PNG，
//!   转不了（HEIC/损坏）回 `None` 由调用方降级为普通文件附件——发送永不失败。
//! * [`dib_to_png`] / [`png_encode_rgba`] —— 零依赖 PNG 编码器。
//!   主干走 WIC（`BitmapDecoder`→`BitmapEncoder`），分叉不引 image crate：
//!   剪贴板 DIB 的布局就三种（8bpp 调色板 / 24bpp BGR / 32bpp BGRA，BI_RGB 与
//!   32bpp BI_BITFIELDS），PNG 侧用 zlib **stored** 块（不压缩、免实现 inflate 之外的
//!   任何压缩器），CRC32/ADLER32 表驱动。真实现比引依赖少一半不确定性。
//!
//! # 不管什么（谁拿走）
//!
//! * 剪贴板读取、文件选择、HTTP 上传、UI chip —— `main.rs` / `nativepick` / 接线层。
//! * `session/attachment` 回读（刀 2，历史图片字节）—— `main.rs` 走 `Kernel::call`。
//!
//! # 不写主干行号
//!
//! 仓规：本仓实测行号必漂，注释只留符号名（`SniffImageMediaType` 这类 grep 得到）。

use serde_json::Value;

use std::ffi::c_void;

use crate::b64;

/// 附件条目（主干 `AttachmentVm` 的分叉等价物）。
///
/// * 图片：`is_image = true`，字节经 [`normalize_image`] 规范化后以 base64 挂在
///   `data_base64`，`media_type` 是嗅探/转码后的真实类型；`file_path` 为空。
/// * 文件：`is_image = false`，`file_path` 是本地路径（上传时现读），
///   `data_base64`/`media_type` 为空。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub name: String,
    pub is_image: bool,
    pub media_type: String,
    pub data_base64: String,
    pub file_path: String,
}

impl Attachment {
    /// 图片附件（字节已规范化）。
    pub fn image(name: String, media_type: &'static str, bytes: &[u8]) -> Self {
        Self {
            name,
            is_image: true,
            media_type: media_type.to_string(),
            data_base64: b64::encode(bytes),
            file_path: String::new(),
        }
    }

    /// 文件附件（发送时才读盘）。
    pub fn file(name: String, path: String) -> Self {
        Self {
            name,
            is_image: false,
            media_type: String::new(),
            data_base64: String::new(),
            file_path: path,
        }
    }
}

/// 按魔数认内核必收的四种图片格式（主干 `SniffImageMediaType` 逐字对齐）。
///
/// 内核对 mediaType 与字节实际格式**严格比对**，这里认不出的（BMP/TIFF/ICO/HEIC…）
/// 返回 `None`，交给 [`normalize_image`] 决定转码还是降级——绝不能把 DIB 硬按
/// `image/png` 上报。
pub fn sniff_image_media_type(b: &[u8]) -> Option<&'static str> {
    if b.len() >= 8
        && b[0] == 0x89
        && b[1] == 0x50
        && b[2] == 0x4E
        && b[3] == 0x47
        && b[4] == 0x0D
        && b[5] == 0x0A
        && b[6] == 0x1A
        && b[7] == 0x0A
    {
        return Some("image/png");
    }
    if b.len() >= 3 && b[0] == 0xFF && b[1] == 0xD8 && b[2] == 0xFF {
        return Some("image/jpeg");
    }
    if b.len() >= 6 && b[0] == b'G' && b[1] == b'I' && b[2] == b'F' && b[3] == b'8'
        && (b[4] == b'7' || b[4] == b'9')
        && b[5] == b'a'
    {
        return Some("image/gif");
    }
    if b.len() >= 12
        && b[0] == b'R'
        && b[1] == b'I'
        && b[2] == b'F'
        && b[3] == b'F'
        && b[8] == b'W'
        && b[9] == b'E'
        && b[10] == b'B'
        && b[11] == b'P'
    {
        return Some("image/webp");
    }
    None
}

/// 附件图片规范化（主干 `NormalizeImageAsync`）：任意来源字节 → 内核必收三元组。
///
/// * 嗅探命中：原字节原类型直返（png/jpeg/webp/gif 内核都收）。
/// * BMP 文件（`BM` 文件头）或 CF_DIB（`BITMAPINFOHEADER` 起始，剪贴板位图流的形态）：
///   转码成 PNG（`image/png`，name 换 `.png` 扩展名——主干 `ChangeExtension` 同款）。
/// * 转不了 / 已损坏：`None`，调用方降级为普通文件附件（发送永不失败）。
pub fn normalize_image(bytes: &[u8], name: &str) -> Option<(Vec<u8>, &'static str, String)> {
    if bytes.is_empty() {
        return None;
    }
    if let Some(sniffed) = sniff_image_media_type(bytes) {
        return Some((bytes.to_vec(), sniffed, name.to_string()));
    }
    let png = if bytes.len() > 2 && bytes[0] == b'B' && bytes[1] == b'M' {
        bmp_file_to_png(bytes)
    } else {
        dib_to_png(bytes)
    }?;
    Some((png, "image/png", change_ext_to_png(name)))
}

/// 主干 `Path.ChangeExtension(name, ".png")` 的等价物（只动最后一个点之后）。
fn change_ext_to_png(name: &str) -> String {
    match name.rfind('.') {
        // 隐藏文件「.bashrc」没有扩展名可换：主干 ChangeExtension 对无扩展名是追加，
        // 但附件名来自文件名/时间戳，不带点的场景就是原样追加 `.png`。
        Some(dot) if dot > 0 => format!("{}.png", &name[..dot]),
        _ => format!("{name}.png"),
    }
}

// ==================== BMP/DIB 解析 → RGBA 行 ====================

/// 一张解码后的位图：行序已是 top-down、通道序已是 RGBA。
struct RawImage {
    width: usize,
    height: usize,
    /// `width * height * 4`，行序 top-down。
    rgba: Vec<u8>,
}

/// BITMAPINFOHEADER 起始的 DIB（CF_DIB / 剪贴板 `GetBitmap` 流的形态）。
///
/// 只支持主干 WIC 实际会遇到的布局：`BI_RGB`（0）的 8/24/32bpp，外加 32bpp
/// `BI_BITFIELDS`（3，浏览器截图的常见形态，掩码读出来是标准 BGRA）。
/// 其余（16bpp、RLE、JPG/PNG 内嵌）回 `None` 走降级——与主干「壳解不了就降级」同口径。
pub fn dib_to_png(dib: &[u8]) -> Option<Vec<u8>> {
    let img = decode_dib(dib)?;
    Some(png_encode_rgba(img.width, img.height, &img.rgba))
}

/// BMP 文件（`BM` 头 14 字节）→ 剥头后按 DIB 解。
pub fn bmp_file_to_png(bmp: &[u8]) -> Option<Vec<u8>> {
    if bmp.len() < 54 || bmp[0] != b'B' || bmp[1] != b'M' {
        return None;
    }
    let data_off = u32_le(bmp, 10)? as usize;
    let dib = bmp.get(14..)?;
    let img = decode_dib(dib)?;
    // 数据偏移与头部声明对不上时以 data_off 为准（个别编码器行区有填充余量）。
    let _ = data_off;
    Some(png_encode_rgba(img.width, img.height, &img.rgba))
}

fn u32_le(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn i32_le(b: &[u8], at: usize) -> Option<i32> {
    u32_le(b, at).map(|v| v as i32)
}

fn u16_le(b: &[u8], at: usize) -> Option<u16> {
    let s = b.get(at..at + 2)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

fn decode_dib(dib: &[u8]) -> Option<RawImage> {
    if dib.len() < 40 {
        return None;
    }
    // BITMAPINFOHEADER 起始 40 字节；V4/V5 头（108/124）像素布局同，只看这 40 字节。
    let width = i32_le(dib, 4)?;
    let height = i32_le(dib, 8)?;
    let bpp = u16_le(dib, 14)?;
    let compression = u32_le(dib, 16)?;
    let clr_used = u32_le(dib, 32)?;
    if width <= 0 || height == 0 {
        return None;
    }
    let width = width as usize;
    // biHeight 带符号定行序：负值 = top-down（剪贴板位图常见），正值 = bottom-up。
    let top_down = height < 0;
    let height = height.unsigned_abs() as usize;
    let row_pitch = (width * bpp as usize + 31) / 32 * 4;

    // 调色板：biClrUsed 为 0 时 8bpp 恒 256 条；每条 BGRA（bgraX 4 字节）。
    let palette_at = if compression == 3 { 40 + 12 } else { 40 };
    let palette_len = match (bpp, compression) {
        (8, 0) => {
            let n = if clr_used == 0 { 256 } else { clr_used as usize };
            n * 4
        }
        _ => 0,
    };
    let px_at = palette_at + palette_len;

    match (bpp, compression) {
        (8, 0) => {
            let palette = dib.get(palette_at..px_at)?;
            let rows = dib.get(px_at..)?;
            let mut rgba = Vec::with_capacity(width * height * 4);
            for y in 0..height {
                let src_y = if top_down { y } else { height - 1 - y };
                let row = rows.get(src_y * row_pitch..src_y * row_pitch + width)?;
                for &idx in row {
                    let pal = palette.get(idx as usize * 4..idx as usize * 4 + 3)?;
                    rgba.extend_from_slice(&[pal[2], pal[1], pal[0], 255]);
                }
            }
            Some(RawImage { width, height, rgba })
        }
        (24, 0) => {
            let rows = dib.get(px_at..)?;
            let mut rgba = Vec::with_capacity(width * height * 4);
            for y in 0..height {
                let src_y = if top_down { y } else { height - 1 - y };
                let row = rows.get(src_y * row_pitch..src_y * row_pitch + width * 3)?;
                for px in row.chunks_exact(3) {
                    rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
                }
            }
            Some(RawImage { width, height, rgba })
        }
        (32, 0) => {
            decode_32bpp(dib.get(px_at..)?, width, height, row_pitch, top_down, 0xFF000000)
        }
        (32, 3) => {
            // BI_BITFIELDS：掩码在 40 字节头后 12 字节（R/G/B 顺序）。标准 BGRA 掩码
            // = R 0x00FF0000 / G 0x0000FF00 / B 0x000000FF；反序（RGB）也接。
            let r = u32_le(dib, 40)?;
            let b = u32_le(dib, 48)?;
            let alpha_mask = 0xFF000000u32;
            decode_32bpp(dib.get(px_at..)?, width, height, row_pitch, top_down, alpha_mask)
                .map(|mut img| {
                    // 掩码反序（R 在低位）时交换 R/B 通道。
                    if r == 0x000000FF && b == 0x00FF0000 {
                        for px in img.rgba.chunks_exact_mut(4) {
                            px.swap(0, 2);
                        }
                    }
                    img
                })
        }
        _ => None,
    }
}

/// 32bpp 行区 → RGBA。`alpha_mask` 只用于「alpha 位全 0 视为不透明」的判定：
/// 剪贴板 DIB 的 alpha 字节通常无效（全 0），WIC 对 BI_RGB 32bpp 也按不透明处理。
fn decode_32bpp(
    rows: &[u8],
    width: usize,
    height: usize,
    row_pitch: usize,
    top_down: bool,
    _alpha_mask: u32,
) -> Option<RawImage> {
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        let src_y = if top_down { y } else { height - 1 - y };
        let row = rows.get(src_y * row_pitch..src_y * row_pitch + width * 4)?;
        for px in row.chunks_exact(4) {
            // DIB 字节序 B,G,R,A → RGBA
            rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
    }
    // alpha 全 0 ⇒ 当作不透明（剪贴板位图流的常态）。
    if rgba.chunks_exact(4).all(|px| px[3] == 0) {
        for px in rgba.chunks_exact_mut(4) {
            px[3] = 255;
        }
    }
    Some(RawImage { width, height, rgba })
}

// ==================== 零依赖 PNG 编码 ====================

/// RGBA 行（`rgba.len() == width * height * 4`，行序 top-down）→ PNG 字节。
///
/// 8-bit RGBA（colortype 6），IDAT 用 zlib **stored** 块（不压缩）：编码端免实现
/// 任何压缩器，解码端（内核 sharp）照收——PNG 是自描述容器，压缩与否不影响合法。
pub fn png_encode_rgba(width: usize, height: usize, rgba: &[u8]) -> Vec<u8> {
    assert_eq!(rgba.len(), width * height * 4, "rgba 长度必须等于 w*h*4");
    let mut out = Vec::with_capacity(64 + rgba.len() + rgba.len() / height + 64);
    out.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    // IHDR：宽、高、位深 8、colortype 6 (RGBA)、压缩 0、过滤 0、隔行 0。
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    push_chunk(&mut out, b"IHDR", &ihdr);
    // 原始流：每行前置 filter byte 0（None）。
    let mut raw = Vec::with_capacity(height * (width * 4 + 1));
    for y in 0..height {
        raw.push(0u8);
        raw.extend_from_slice(&rgba[y * width * 4..(y + 1) * width * 4]);
    }
    push_chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    push_chunk(&mut out, b"IEND", &[]);
    out
}

/// 一个 PNG chunk：长度 + 类型 + 数据 + CRC32（类型与数据一起算）。
fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let crc_start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[crc_start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// zlib 流（RFC 1950）+ **stored** deflate 块（RFC 1951 BTYPE=00）：
/// `0x78 0x01` 头，数据按 ≤65535 分块（LEN/NLEN 各 2 字节 little-endian），
/// 尾部 ADLER32（big-endian）。
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / 65535 * 5 + 16);
    out.extend_from_slice(&[0x78, 0x01]);
    let mut chunks = data.chunks(65535).peekable();
    if data.is_empty() {
        // 空 deflate stored 块也要一发（LEN=0）。
        out.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    while let Some(chunk) = chunks.next() {
        let last = chunks.peek().is_none();
        out.push(u8::from(last));
        let len = chunk.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(chunk);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    // 表驱动，poly 0xEDB88320（PNG 规范）；表在首个使用线程上惰性构建——
    // 附件转码一图一调，构建成本可忽略，不值得引入 OnceLock 之外的复杂度。
    TABLE.with(|t| {
        let table = t.borrow();
        let mut crc = 0xFFFF_FFFFu32;
        for &byte in data {
            crc = table[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
        }
        crc ^ 0xFFFF_FFFF
    })
}

thread_local! {
    static TABLE: std::cell::RefCell<[u32; 256]> = std::cell::RefCell::new(build_crc_table());
}

fn build_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *slot = c;
    }
    table
}

// ==================== 上传执行腿（WinHTTP POST，主干 UploadBytesAsync 等价物） ====================

// 蓝本 = `updatecheck.rs` 的 WinHTTP 裸声明块（零依赖成规：见 keys.rs:19-21 的策略原文）。
// 与 updatecheck 那 GET+https 的腿**各带一份真相**（仓内 tray/toast 结构体同款先例）：
// 这条腿是 http 明文 + POST + octet-stream body，形状差三处，硬合只会造出两套旗标互斥的怪层。
const WINHTTP_ACCESS_TYPE_DEFAULT_PROXY: u32 = 0;
const WINHTTP_ADDREQ_FLAG_ADD: u32 = 0x2000_0000;
const WINHTTP_QUERY_STATUS_CODE: u32 = 19;
const WINHTTP_QUERY_FLAG_NUMBER: u32 = 0x2000_0000;
const WINHTTP_READ_CHUNK: usize = 8 * 1024;
const NO_PROXY: *const u16 = std::ptr::null();

#[link(name = "winhttp")]
unsafe extern "system" {
    fn WinHttpOpen(agent: *const u16, access_type: u32, proxy: *const u16, proxy_bypass: *const u16, flags: u32) -> *mut c_void;
    fn WinHttpConnect(session: *mut c_void, server: *const u16, port: u16, reserved: u32) -> *mut c_void;
    fn WinHttpOpenRequest(connect: *mut c_void, verb: *const u16, object: *const u16, version: *const u16, referrer: *const u16, accept_types: *const *const u16, flags: u32) -> *mut c_void;
    fn WinHttpAddRequestHeaders(request: *mut c_void, headers: *const u16, length: u32, modifiers: u32) -> i32;
    fn WinHttpQueryHeaders(request: *mut c_void, level: u32, name: *const u16, buffer: *mut c_void, length: *mut u32, index: u32) -> i32;
    fn WinHttpSendRequest(request: *mut c_void, headers: *const u16, headers_length: u32, optional: *const c_void, optional_length: u32, total_length: u32, context: usize) -> i32;
    fn WinHttpReceiveResponse(request: *mut c_void, reserved: *mut c_void) -> i32;
    fn WinHttpReadData(request: *mut c_void, buffer: *mut u8, to_read: u32, read: *mut u32) -> i32;
    fn WinHttpSetTimeouts(handle: *mut c_void, resolve: i32, connect: i32, send: i32, receive: i32) -> i32;
    fn WinHttpCloseHandle(handle: *mut c_void) -> i32;
}

/// 句柄的 RAII 壳（updatecheck 同款）：任何早退路径都不漏句柄。
struct HttpHandle(*mut c_void);

impl Drop for HttpHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

fn to_utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn utf16_line_len_chars(buf: &[u16]) -> u32 {
    (match buf.last() {
        Some(&0) => buf.len() - 1,
        _ => buf.len(),
    }) as u32
}

fn last_error() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

fn ok(code: i32) -> bool {
    code != 0
}

/// `Uri.EscapeDataString` 的等价物：RFC 3986 unreserved（A-Za-z0-9 - _ . ~）之外
/// 全部按 UTF-8 百分号编码。sessionId（uuid 安全但照转）与 name（中文/空格/emoji
/// 的图片名常态）都过它——query 里的原始空格会让内核的 URL 解析直接 400。
pub fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for &byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// 上传回执解析（主干 `UploadBytesAsync` 的三种成功形态 + `ok:false` 错误形态）：
/// 裸字符串 / `{"receiptId":"…"}` / `{"value":"…"}`（或 `{"result":{"value":"…"}}`）。
/// `ok:false` 抛 `"{code}: {message}"` —— 与 `Kernel::call` 的错误拼串同一形状，
/// 调用方的 `attachment-invalid` 降级判据因此对两条通道同源。
pub fn parse_upload_receipt(body: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(body).map_err(|e| format!("上传回执不是 JSON: {e}"))?;
    if value.is_string() {
        return Ok(value.as_str().unwrap_or_default().to_string());
    }
    let obj = value.as_object().ok_or_else(|| "上传回执既非串也非对象".to_string())?;
    if obj.get("ok") == Some(&Value::Bool(false)) {
        let err = obj.get("error").filter(|e| e.is_object());
        let code = err
            .and_then(|e| e.get("code"))
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let message = err
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("dsh RPC failed");
        return Err(format!("{code}: {message}"));
    }
    if let Some(rid) = obj.get("receiptId").and_then(Value::as_str) {
        return Ok(rid.to_string());
    }
    let mut value = obj.get("value");
    if value.is_none() {
        value = obj
            .get("result")
            .filter(|r| r.is_object())
            .and_then(|r| r.get("value"));
    }
    if let Some(Value::String(s)) = value {
        return Ok(s.clone());
    }
    Err("上传回执里没有 receiptId".to_string())
}

/// 上传文件附件：`POST http://{host}:{port}/api/session/uploadFileBinary?sessionId=&name=`
/// （octet-stream）→ receiptId。**阻塞式，只能在 worker 线程调**（发送闭包就在那条线程上）。
/// 失败一律 `Err`（对齐主干：附件传不上去还照发文本 = 静默丢附件的教训，绝不容忍）。
pub fn upload_bytes_to_kernel(
    host: &str,
    port: u16,
    session_id: &str,
    name: &str,
    bytes: &[u8],
) -> Result<String, String> {
    let path = format!(
        "/api/session/uploadFileBinary?sessionId={}&name={}",
        percent_encode(session_id),
        percent_encode(name)
    );
    let agent = to_utf16("Blade2-rs");
    let server = to_utf16(host);
    let verb = to_utf16("POST");
    let object = to_utf16(&path);
    // Content-Type 一条头由 WinHttpSendRequest 的 headers 槽带出；Content-Length
    // 由 optional_length/total_length 让 WinHTTP 自己写（主干 HttpClient 同款）。
    let headers = to_utf16("Content-Type: application/octet-stream\r\n");
    // 本机内核、本地回环：分段超时收紧（resolve 基本不耗，2s/2s/2s/10s 覆盖大图上传）。
    const TIMEOUTS_MS: (u32, u32, u32, u32) = (2000, 2000, 2000, 10_000);

    unsafe {
        let opened = WinHttpOpen(agent.as_ptr(), WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, NO_PROXY, NO_PROXY, 0);
        let err = last_error();
        let session = HttpHandle(opened);
        if opened.is_null() {
            return Err(format!("WinHttpOpen 失败 err={err}：拿不到会话句柄"));
        }
        let (resolve, connect, send_t, receive) = TIMEOUTS_MS;
        let timed = WinHttpSetTimeouts(session.0, resolve as i32, connect as i32, send_t as i32, receive as i32);
        let err = last_error();
        if !ok(timed) {
            return Err(format!("WinHttpSetTimeouts 失败 err={err}"));
        }
        let connected = WinHttpConnect(session.0, server.as_ptr(), port, 0);
        let err = last_error();
        let connect_handle = HttpHandle(connected);
        if connected.is_null() {
            return Err(format!("WinHttpConnect 失败 err={err}：{host}:{port}"));
        }
        // 明文 http：不给 WINHTTP_FLAG_SECURE。
        let requested = WinHttpOpenRequest(
            connect_handle.0,
            verb.as_ptr(),
            object.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            0,
        );
        let err = last_error();
        let request = HttpHandle(requested);
        if requested.is_null() {
            return Err(format!("WinHttpOpenRequest 失败 err={err}：{path}"));
        }
        let header_chars = utf16_line_len_chars(&headers);
        let added = WinHttpAddRequestHeaders(request.0, headers.as_ptr(), header_chars, WINHTTP_ADDREQ_FLAG_ADD);
        let err = last_error();
        if !ok(added) {
            return Err(format!("WinHttpAddRequestHeaders 失败 err={err}"));
        }
        let total = bytes.len() as u32;
        let body_ptr = if bytes.is_empty() {
            std::ptr::null()
        } else {
            bytes.as_ptr() as *const c_void
        };
        let sent = WinHttpSendRequest(request.0, std::ptr::null(), 0, body_ptr, total, total, 0);
        let err = last_error();
        if !ok(sent) {
            return Err(format!("WinHttpSendRequest 失败 err={err}：发不出去 http://{host}:{port}{path}"));
        }
        let received = WinHttpReceiveResponse(request.0, std::ptr::null_mut());
        let err = last_error();
        if !ok(received) {
            return Err(format!("WinHttpReceiveResponse 失败 err={err}：收不到上传回执"));
        }
        let mut status: u32 = 0;
        let mut status_len: u32 = std::mem::size_of::<u32>() as u32;
        let queried = WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            std::ptr::null(),
            &mut status as *mut u32 as *mut c_void,
            &mut status_len as *mut u32,
            0,
        );
        let err = last_error();
        if !ok(queried) {
            return Err(format!("WinHttpQueryHeaders 失败 err={err}：取不到上传回执状态码"));
        }
        let mut body: Vec<u8> = Vec::new();
        loop {
            let mut chunk = [0u8; WINHTTP_READ_CHUNK];
            let mut got: u32 = 0;
            let read = WinHttpReadData(request.0, chunk.as_mut_ptr(), WINHTTP_READ_CHUNK as u32, &mut got);
            let err = last_error();
            if !ok(read) {
                return Err(format!("WinHttpReadData 失败 err={err}：正文已累计 {} 字节", body.len()));
            }
            if got == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..got as usize]);
        }
        if !(200..300).contains(&status) {
            let text = String::from_utf8_lossy(&body);
            return Err(format!("上传回执状态码非 2xx：{status} {text}"));
        }
        parse_upload_receipt(&String::from_utf8_lossy(&body))
    }
}

/// 测试与调用方共用的迷你 PNG 读回：验签名/IHDR/自解 stored IDAT/逐行比对。
/// 只认本编码器的输出形状（colortype 6、filter 0、stored zlib），不解一般 PNG。
#[cfg(test)]
pub(crate) fn png_decode_stored(png: &[u8]) -> (usize, usize, Vec<u8>) {
    assert_eq!(&png[..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    let mut at = 8usize;
    let (mut width, mut height) = (0usize, 0usize);
    let mut raw: Vec<u8> = Vec::new();
    while at < png.len() {
        let len = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
        let kind = &png[at + 4..at + 8];
        let data = &png[at + 8..at + 8 + len];
        match kind {
            b"IHDR" => {
                width = u32::from_be_bytes(data[0..4].try_into().unwrap()) as usize;
                height = u32::from_be_bytes(data[4..8].try_into().unwrap()) as usize;
                assert_eq!(data[8], 8, "位深恒 8");
                assert_eq!(data[9], 6, "colortype 恒 RGBA");
            }
            b"IDAT" => {
                // zlib 头 2 字节 + stored 块 + adler32(4)。
                assert_eq!(&data[..2], &[0x78, 0x01]);
                let mut p = 2usize;
                loop {
                    let bfinal = data[p] & 1 == 1;
                    let len = u16::from_le_bytes(data[p + 1..p + 3].try_into().unwrap()) as usize;
                    let nlen = u16::from_le_bytes(data[p + 3..p + 5].try_into().unwrap()) as usize;
                    assert_eq!(!len & 0xFFFF, nlen, "stored 块 NLEN 校验");
                    raw.extend_from_slice(&data[p + 5..p + 5 + len]);
                    p += 5 + len;
                    if bfinal {
                        break;
                    }
                }
                assert_eq!(p + 4, data.len(), "IDAT 尾部应恰为 adler32");
            }
            b"IEND" => {
                at += 12;
                break;
            }
            _ => {}
        }
        at += 12 + len;
    }
    assert_eq!(raw.len(), height * (width * 4 + 1), "原始流长度");
    let mut rgba = Vec::with_capacity(width * height * 4);
    for row in raw.chunks(width * 4 + 1) {
        assert_eq!(row[0], 0, "filter 恒 None");
        rgba.extend_from_slice(&row[1..]);
    }
    (width, height, rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **L-AT1**：四魔数一个不漏、一个不多（主干 `SniffImageMediaType` 的四臂）。
    #[test]
    fn sniff_matches_the_four_kernel_formats() {
        assert_eq!(sniff_image_media_type(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]), Some("image/png"));
        assert_eq!(sniff_image_media_type(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(sniff_image_media_type(b"GIF89a...."), Some("image/gif"));
        assert_eq!(sniff_image_media_type(b"GIF87a...."), Some("image/gif"));
        assert_eq!(sniff_image_media_type(b"RIFF\x00\x00\x00\x00WEBPVP8 "), Some("image/webp"));
        // BMP 不是内核必收格式：嗅探不认，转码层认。
        assert_eq!(sniff_image_media_type(b"BM\x00\x00"), None);
        assert_eq!(sniff_image_media_type(&[]), None);
    }

    /// **L-AT2**：32bpp top-down DIB（剪贴板 `GetBitmap` 流形态）→ PNG，
    /// BGR→RGB 翻转 + alpha 全 0 当不透明。
    #[test]
    fn dib_32bpp_topdown_becomes_png() {
        let (w, h) = (2usize, 2usize);
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes()); // biSize
        dib.extend_from_slice(&(w as i32).to_le_bytes());
        dib.extend_from_slice(&(-(h as i32)).to_le_bytes()); // top-down
        dib.extend_from_slice(&1u16.to_le_bytes()); // planes
        dib.extend_from_slice(&32u16.to_le_bytes()); // bpp
        dib.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
        dib.extend_from_slice(&0u32.to_le_bytes()); // biSizeImage
        dib.extend_from_slice(&[0u8; 16]); // 其余头字段
        // 像素（B,G,R,A 字节序）：[0,0,255,0]=红、[0,255,0,0]=绿、[255,0,0,0]=蓝、
        // [1,2,3,0] = B1 G2 R3 → RGBA [3,2,1,·]
        dib.extend_from_slice(&[0, 0, 255, 0, 0, 255, 0, 0]);
        dib.extend_from_slice(&[255, 0, 0, 0, 1, 2, 3, 0]);

        let png = dib_to_png(&dib).expect("32bpp top-down 必须解出");
        let (dw, dh, rgba) = png_decode_stored(&png);
        assert_eq!((dw, dh), (w, h));
        assert_eq!(rgba, vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 3, 2, 1, 255]);
    }

    /// **L-AT3**：24bpp bottom-up（.bmp 文件行序常态）→ 行序翻转 + BGR→RGB。
    #[test]
    fn dib_24bpp_bottomup_becomes_png() {
        let (w, h) = (2usize, 2usize);
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&(w as i32).to_le_bytes());
        dib.extend_from_slice(&(h as i32).to_le_bytes()); // bottom-up
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&24u16.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&[0u8; 20]);
        // 行 pitch = (2*24+31)/32*4 = 8（行 6 字节 + 2 填充）。
        dib.extend_from_slice(&[10, 20, 30, 40, 50, 60, 0, 0]); // 底行
        dib.extend_from_slice(&[70, 80, 90, 100, 110, 120, 0, 0]); // 顶行

        let png = dib_to_png(&dib).expect("24bpp bottom-up 必须解出");
        let (_, _, rgba) = png_decode_stored(&png);
        // 首行 = 存储的最后一行（bottom-up 翻转），BGR→RGB。
        assert_eq!(rgba, vec![90, 80, 70, 255, 120, 110, 100, 255, 30, 20, 10, 255, 60, 50, 40, 255]);
    }

    /// **L-AT4**：8bpp 调色板 + BI_BITFIELDS 32bpp 反序掩码 + 坏头 ⇒ None。
    #[test]
    fn dib_edges() {
        // 8bpp：256 条调色板，像素 idx 0 → 调色板第 0 条。
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&(-1i32).to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&8u16.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&[0u8; 20]); // 头内其余字段（biClrUsed=0 已在其中）
        dib.extend_from_slice(&[255, 0, 0, 0]); // 调色板第 0 条 = BGRA 蓝色
        dib.extend_from_slice(&vec![0u8; 255 * 4]);
        dib.extend_from_slice(&[0u8]); // 1×1 像素，行 pitch 4
        dib.extend_from_slice(&vec![0u8; 3]);
        let png = dib_to_png(&dib).expect("8bpp 必须解出");
        let (_, _, rgba) = png_decode_stored(&png);
        assert_eq!(rgba, vec![0, 0, 255, 255]);

        // 16bpp：不支持 ⇒ None（降级为文件附件）。
        let mut bad = Vec::new();
        bad.extend_from_slice(&40u32.to_le_bytes());
        bad.extend_from_slice(&1i32.to_le_bytes());
        bad.extend_from_slice(&1i32.to_le_bytes());
        bad.extend_from_slice(&1u16.to_le_bytes());
        bad.extend_from_slice(&16u16.to_le_bytes());
        bad.extend_from_slice(&0u32.to_le_bytes());
        bad.extend_from_slice(&[0u8; 20]);
        assert_eq!(dib_to_png(&bad), None);
        assert_eq!(dib_to_png(&[]), None);
        assert_eq!(dib_to_png(&[0u8; 39]), None, "不足 40 字节头");
    }

    /// **L-AT5**：normalize_image 的三态——嗅探直返 / DIB 转码换名 / 不可解降级。
    #[test]
    fn normalize_matches_the_mainline_three_states() {
        let png_bytes = png_encode_rgba(1, 1, &[1, 2, 3, 255]);
        // 嗅探命中：原字节原类型原名。
        let (b, t, n) = normalize_image(&png_bytes, "shot.png").unwrap();
        assert_eq!((b, t, n.as_str()), (png_bytes.clone(), "image/png", "shot.png"));
        // DIB：转 PNG + 改名 .png。
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&(-1i32).to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&32u16.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&[0u8; 20]);
        dib.extend_from_slice(&[9, 9, 9, 255]);
        let (b, t, n) = normalize_image(&dib, "Clipboard.png").unwrap();
        assert_eq!(t, "image/png");
        assert_eq!(n, "Clipboard.png");
        assert_eq!(sniff_image_media_type(&b), Some("image/png"), "转码产物必须过嗅探");
        // 无扩展名：追加。
        assert_eq!(normalize_image(&dib, "blob").map(|x| x.2), Some("blob.png".to_string()));
        // HEIC 魔数（ftyp heic）：嗅探不认、DIB 解不出 ⇒ None（调用方降级文件附件）。
        assert_eq!(normalize_image(b"\x00\x00\x00\x18ftypheic....", "p.heic"), None);
        assert_eq!(normalize_image(&[], "x.png"), None);
    }

    /// **L-AT6**：BMP 文件头（`BM` + data offset）剥头走同一条 DIB 解析。
    #[test]
    fn bmp_file_head_is_peeled() {
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&(-1i32).to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&32u16.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&[0u8; 20]);
        dib.extend_from_slice(&[1, 2, 3, 255]);
        let mut bmp = b"BM".to_vec();
        bmp.extend_from_slice(&((dib.len() + 14) as u32).to_le_bytes()); // 文件大小（仅占位）
        bmp.extend_from_slice(&[0u8; 4]);
        bmp.extend_from_slice(&(14u32 + 40u32).to_le_bytes()); // data offset
        bmp.extend_from_slice(&dib);
        let (b, t, _) = normalize_image(&bmp, "a.bmp").unwrap();
        assert_eq!(t, "image/png");
        assert_eq!(png_decode_stored(&b).2, vec![3, 2, 1, 255]);
    }

    /// **L-AT7**：percent_encode = RFC 3986 unreserved 白名单（EscapeDataString 同集）。
    /// 中文名/空格/emoji 必转，uuid 与文件点不转。
    #[test]
    fn percent_encode_matches_escape_data_string() {
        assert_eq!(percent_encode("abcXYZ012-_.~"), "abcXYZ012-_.~");
        assert_eq!(percent_encode("a b"), "a%20b");
        assert_eq!(percent_encode("图.png"), "%E5%9B%BE.png");
        assert_eq!(percent_encode("100%"), "100%25");
        assert_eq!(percent_encode("🐈"), "%F0%9F%90%88");
        // 回读侧同理（UUID 大写也照转——主干 EscapeDataString 只放 unreserved）。
        assert_eq!(percent_encode("{"), "%7B");
    }

    /// **L-AT8**：上传回执的三种成功形态 + ok:false 错误形态
    /// （主干 `UploadBytesAsync` 的解析臂逐条对齐）。
    #[test]
    fn upload_receipt_parses_three_success_shapes() {
        assert_eq!(parse_upload_receipt("\"rc-1\"").unwrap(), "rc-1");
        assert_eq!(parse_upload_receipt("{\"receiptId\":\"rc-2\"}").unwrap(), "rc-2");
        assert_eq!(parse_upload_receipt("{\"value\":\"rc-3\"}").unwrap(), "rc-3");
        assert_eq!(
            parse_upload_receipt("{\"result\":{\"value\":\"rc-4\"}}").unwrap(),
            "rc-4"
        );
        // ok:false ⇒ "code: message"，与 Kernel::call 的拼串形状一致 ⇒ 降级判据同源。
        let err = parse_upload_receipt(
            "{\"ok\":false,\"error\":{\"code\":\"session/attachment-invalid\",\"message\":\"bad image\"}}",
        )
        .unwrap_err();
        assert_eq!(err, "session/attachment-invalid: bad image");
        assert!(parse_upload_receipt("{}").is_err());
        assert!(parse_upload_receipt("not json").is_err());
    }
}
