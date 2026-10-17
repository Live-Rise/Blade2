//! #160 B 刀第一片（UC-K1）：**更新检查的「只算不画」纯层**。
//!
//! 主干那一发的本体是 `MainWindow.About.cs` 的 `CheckUpdateAsync` 与
//! `MainWindow.UpdateCheck.cs` 的 `CheckUpdateSilentlyAsync`（同一条腿抄了两遍）：
//! 裸 `HttpClient` GET 一次 GitHub Releases API → `JsonDocument` 取 `tag_name` →
//! `System.Version` 比四段 → 命中才露下载钮 / 弹 toast。UD1 的三判定案：分叉要保真这一发，
//! 零新依赖的唯一落点是 **WinHTTP 裸声明块 + OS 自带的 Schannel**（TLS 由 `winhttp.dll` 供货，
//! `Cargo.toml` 一颗 crate 都不涨）。本模块就是那条腿的分叉形：
//!
//! | 节 | 形态 | 是否触网 |
//! |---|---|---|
//! | 版本比较 [`update_verdict`] / [`parse_version_tag`] | 纯函数 | 否 |
//! | 回执解析 [`parse_release_receipt`] / [`pick_msix_asset`] | 纯函数 | 否 |
//! | 请求构造 [`build_release_request`] | 纯数据描述子 | 否 |
//! | 网络执行 [`fetch_release_body`] | 阻塞式 FFI | **是 —— 单测一律不碰** |
//!
//! 两条硬口径写死在这里：
//! 1. **本模块零 UI 调用点、零 `main.rs` 引用**。bin 不能被 lib 引用，故版本串一律**收 `&str` 入参**，
//!    由 main.rs 那一刀把 `SHELL_VERSION` 传进来（接线与本模块无关）。
//! 2. **TryParse 失败 ⇒ 判「无法比较」，绝不判「有更新」**（主干 `UpdateCheck.cs` 的口径：
//!    tag 不可比就直接 return，宁漏不误报）。
//!
//! 备案性偏离一条（超时）：主干这条腿**未设**超时 ⇒ 吃 `HttpClient` 默认 100 s。
//! 分叉走 WinHTTP，超时是**分段**语义（resolve / connect / send / receive 各自一档，
//! 且 receive 每读到一次数据即复位），没有「整发预算」这一档可抄 ⇒ 本模块显式设
//! [`REQUEST_TIMEOUTS_MS`] = 10/10/30/30 s：四段之和 80 s ≤ 主干的 100 s 整发预算，
//! 单段更严格小于主干默认，最坏不会比主干等得更久。理由与数值同时被单测
//! `uc9_lock_the_timeout_leg_is_explicit_and_bounded` 钉住。

use std::ffi::c_void;

// ---------------------------------------------------------------------------
// 一、端点与两串请求头（逐字对齐主干）
// ---------------------------------------------------------------------------

/// 主干 `MainWindow.About.cs:25` 的 `UpdateRepo` 常量值 —— 本模块只把 **repo 串当入参**，
/// 不 import main.rs 的 `ABOUT_UPDATE_REPO`（那是 bin 侧的家）。
pub const DEFAULT_UPDATE_REPO: &str = "Live-Rise/Blade2";

/// GitHub Releases API 的权威主机名（主干拼串的固定段）。
pub const API_HOST: &str = "api.github.com";

/// HTTPS 端口 —— 主干走 `HttpClient` 隐式 443，分叉要显式交给 `WinHttpConnect`。
pub const API_PORT: u16 = 443;

const API_SCHEME: &str = "https";
const API_PATH_PREFIX: &str = "/repos/";
const API_PATH_SUFFIX: &str = "/releases/latest";

/// 主干 `About.cs:176` / `UpdateCheck.cs:41`：`User-Agent: Blade2-WinUI`。
pub const UPDATE_CHECK_USER_AGENT: &str = "Blade2-WinUI";

/// 主干 `About.cs:177` / `UpdateCheck.cs:42`：`Accept: application/vnd.github+json`。
pub const UPDATE_CHECK_ACCEPT: &str = "application/vnd.github+json";

/// 超时四段（毫秒），顺序 = `WinHttpSetTimeouts` 的 resolve / connect / send / receive。
/// 取值的理由见模块头的「备案性偏离」。
pub const REQUEST_TIMEOUTS_MS: (u32, u32, u32, u32) = (10_000, 10_000, 30_000, 30_000);

/// 主干 `UpdateCheck.cs:17` 的静默延迟：启动后 6 s 才检查，不与内核引导抢 IO。
/// 分叉侧的线程调度在 main.rs 那一刀；这里只把常量给出去。
pub const UPDATE_CHECK_DELAY_MS: u64 = 6000;

/// `https://api.github.com/repos/{repo}/releases/latest`（主干 `About.cs:27-29`）。
/// 主干守卫：repo 空 ⇒ 空串 ⇒ `UpdateCheck.cs:36-39` 直接 return（**不联网**）。
pub fn release_api_url(repo: &str) -> String {
    let repo = repo.trim();
    if repo.is_empty() {
        return String::new();
    }
    format!("{API_SCHEME}://{API_HOST}{API_PATH_PREFIX}{repo}{API_PATH_SUFFIX}")
}

// ---------------------------------------------------------------------------
// 二、四段版本比较（`System.Version` 形制，不是语义化版本）
// ---------------------------------------------------------------------------

/// 与 .NET `System.Version` 同形的四段版本。缺段补 0（主干 `Version.Parse("1.2")` 的
/// Build/Revision 是 `-1`，而 `CompareTo` 把 `-1` 当 `0` 比 ⇒ 补 0 与主干等价）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version4 {
    pub major: u32,
    pub minor: u32,
    pub build: u32,
    pub revision: u32,
}

/// 判据三态 —— 关键设计：**没有布尔返回**。垃圾 tag 必须落在
/// [`UpdateVerdict::NotComparable`]，不能被压成「有更新」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateVerdict {
    /// `latest > current` —— 主干才露下载钮 / 才 toast。
    Newer,
    /// `latest <= current` —— 等值**不算**更新（主干是严格 `>`，不是语义化的 `>=`）。
    NotNewer,
    /// 任一侧不可解析 ⇒ 无法比较（主干 `TryParseVersion(tag) is not { }` ⇒ return）。
    NotComparable,
}

/// 主干 `About.cs:392-396` 的 `TryParseVersion`：`Trim()` → `TrimStart('v','V')` →
/// `Version.TryParse`。**带非数字后缀（`0.8.2.0-rc1`）⇒ None ⇒ 视为不可比**。
pub fn parse_version_tag(tag: &str) -> Option<Version4> {
    let t = tag.trim().trim_start_matches(['v', 'V']);
    dotnet_version_try_parse(t)
}

/// 当前版本侧：主干走 `Package.Current.Id.Version` 四段（无包身份回落程序集版本），
/// 分叉只有 `SHELL_VERSION` 那枚四段显示串 ⇒ 这里只 `trim()`，**不剥 `v` 前缀**。
pub fn parse_version(text: &str) -> Option<Version4> {
    dotnet_version_try_parse(text.trim())
}

/// 逐字对齐 `System.Version.TryParse` 的接受集：2..=4 段、段段非空且全 ASCII 数字、
/// 每段落进 `Int32`（故 `1.2.3.2147483648`、`+1.2`、`1.2.3.4.5`、`1..2`、`1` 一律 None）。
fn dotnet_version_try_parse(text: &str) -> Option<Version4> {
    let parts: Vec<&str> = text.split('.').collect();
    if parts.len() < 2 || parts.len() > 4 {
        return None;
    }
    let mut seg = [0u32; 4];
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let value: u32 = part.parse().ok()?;
        if value > i32::MAX as u32 {
            return None;
        }
        seg[i] = value;
    }
    Some(Version4 {
        major: seg[0],
        minor: seg[1],
        build: seg[2],
        revision: seg[3],
    })
}

/// 主干判据：`latest > current`（`About.cs:192-194`；静默侧 `UpdateCheck.cs:51-56` 反向 return）。
/// 两侧任一无效 ⇒ [`UpdateVerdict::NotComparable`]。
pub fn update_verdict(latest_tag: &str, current_version: &str) -> UpdateVerdict {
    let (Some(latest), Some(current)) = (parse_version_tag(latest_tag), parse_version(current_version))
    else {
        return UpdateVerdict::NotComparable;
    };
    if latest > current {
        UpdateVerdict::Newer
    } else {
        UpdateVerdict::NotNewer
    }
}

// ---------------------------------------------------------------------------
// 三、回执解析（GitHub `releases/latest` JSON → tag_name + 下载目标）
// ---------------------------------------------------------------------------

/// 主干 `PickMsixAsset`（`About.cs:332-371`）的三级回落层级。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetSource {
    /// `name` 以 `.msix` 结尾（**大小写无关**）的那颗 —— 主干的首选。
    Msix,
    /// 没有 `.msix` ⇒ 第一颗带非空 `browser_download_url` 的 asset。
    FirstAssetUrl,
    /// 连 asset 都没有 ⇒ 回落 `html_url`（Release 页面，只能开浏览器手动下载）。
    ReleasePage,
}

/// 一条命中：主干返回 `(Name, Url, Size)` 三元组，分叉同形 + 层级标注。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetHit {
    pub name: String,
    pub url: String,
    /// 字节数；主干口径：非数字或缺失 ⇒ `0`。
    pub size: i64,
    pub source: AssetSource,
}

impl AssetHit {
    /// 主干 `About.cs:198` 的 `_aboutDownloadIsDirect`：只有 **url** 以 `.msix` 结尾
    /// 才走「下载到 %TEMP% + Add-AppxPackage」，否则开浏览器。注意它查的是 url，
    /// 而 [`pick_msix_asset`] 查的是 `name` —— 主干这两处本就不同源，分叉忠实保留。
    pub fn is_direct_package_url(&self) -> bool {
        ends_with_msix(&self.url)
    }
}

/// 主干 `tag_name` 口径：属性存在**且必须是 JSON 字符串**，否则视作空串 ⇒
/// 「检查更新失败：Release 返回缺少 tag_name。」
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseReceipt {
    pub tag_name: Option<String>,
    pub asset: Option<AssetHit>,
}

impl ReleaseReceipt {
    /// 主干 `About.cs:182-185` 的取值面。
    pub fn tag(&self) -> &str {
        self.tag_name.as_deref().unwrap_or("")
    }
}

/// 解析整张回执。主干 `JsonDocument.Parse` 抛异常 ⇒ 被 `UpdateCheck.cs:68-72` 吞掉；
/// 分叉侧把「解析不了」如实返 Err，由调用方决定静默还是提示。
pub fn parse_release_receipt(body: &str) -> Result<ReleaseReceipt, String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("回执不是合法 JSON：{e}"))?;
    Ok(receipt_from_value(&value))
}

/// [`parse_release_receipt`] 的已解析形，纯函数、可单测。
pub fn receipt_from_value(value: &serde_json::Value) -> ReleaseReceipt {
    ReleaseReceipt {
        tag_name: string_field(value, "tag_name"),
        asset: pick_download_target(value),
    }
}

/// 主干 `PickMsixAsset` 的**第一级**：只看 `name` 的 `.msix` 后缀（OrdinalIgnoreCase），
/// 取第一颗命中；url 为空的 asset 主干先 `continue` 掉，故永不入选。
pub fn pick_msix_asset(value: &serde_json::Value) -> Option<AssetHit> {
    for asset in assets_of(value) {
        let url = string_field(asset, "browser_download_url").unwrap_or_default();
        if url.is_empty() {
            continue;
        }
        let name = string_field(asset, "name").unwrap_or_default();
        if ends_with_msix(&name) {
            return Some(AssetHit {
                size: int_field(asset, "size"),
                name,
                url,
                source: AssetSource::Msix,
            });
        }
    }
    None
}

/// 主干 `PickMsixAsset` 全形：`.msix` → 首个带 url 的 asset → `html_url` 页面。
/// 三级全空 ⇒ None（主干此时返 `("","",0)`，分叉如实给「无下载目标」）。
pub fn pick_download_target(value: &serde_json::Value) -> Option<AssetHit> {
    if let Some(hit) = pick_msix_asset(value) {
        return Some(hit);
    }
    for asset in assets_of(value) {
        let url = string_field(asset, "browser_download_url").unwrap_or_default();
        if url.is_empty() {
            continue;
        }
        return Some(AssetHit {
            size: int_field(asset, "size"),
            name: string_field(asset, "name").unwrap_or_default(),
            url,
            source: AssetSource::FirstAssetUrl,
        });
    }
    let html = string_field(value, "html_url").unwrap_or_default();
    if html.is_empty() {
        return None;
    }
    Some(AssetHit {
        name: String::new(),
        url: html,
        size: 0,
        source: AssetSource::ReleasePage,
    })
}

fn assets_of<'a>(value: &'a serde_json::Value) -> Vec<&'a serde_json::Value> {
    value
        .get("assets")
        .and_then(|a| a.as_array())
        .map(|arr| arr.iter().collect())
        .unwrap_or_default()
}

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

fn int_field(value: &serde_json::Value, key: &str) -> i64 {
    value.get(key).and_then(|v| v.as_i64()).unwrap_or(0)
}

/// `.msix` 后缀判据（等价主干 `EndsWith(".msix", OrdinalIgnoreCase)`，大小写无关）。
/// 用 `get(..)` 防非 ASCII 边界的字节切片 panic；边界不合法 ⇒ 本就不可能是 `.msix` 结尾。
fn ends_with_msix(text: &str) -> bool {
    match text.get(text.len().saturating_sub(5)..) {
        Some(tail) => tail.eq_ignore_ascii_case(".msix"),
        None => false,
    }
}

// ---------------------------------------------------------------------------
// 四、请求描述子（纯数据）+ WinHTTP 执行腿（真触网）
// ---------------------------------------------------------------------------

/// 一条「要发什么」的完整描述：**不含任何句柄、不含任何 unsafe**，所以能被单测逐字钉住；
/// [`fetch_release_body`] 只是把它翻译成 WinHTTP 调用序列。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestDescriptor {
    pub method: String,
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub path: String,
    /// 主干的两串头，按 `About.cs:176-177` 的写出次序。
    pub headers: Vec<(String, String)>,
    /// 见 [`REQUEST_TIMEOUTS_MS`] —— 分叉显式设，主干未设。
    pub timeouts_ms: (u32, u32, u32, u32),
}

/// 构造那颗 GET 的描述子。repo 空 ⇒ None（= 主干 `UpdateCheck.cs:36-39` 的「未配置 ⇒ 不联网」守卫）。
pub fn build_release_request(repo: &str) -> Option<RequestDescriptor> {
    let repo = repo.trim();
    if repo.is_empty() {
        return None;
    }
    Some(RequestDescriptor {
        method: "GET".to_string(),
        scheme: API_SCHEME.to_string(),
        host: API_HOST.to_string(),
        port: API_PORT,
        path: format!("{API_PATH_PREFIX}{repo}{API_PATH_SUFFIX}"),
        headers: vec![
            (
                "User-Agent".to_string(),
                UPDATE_CHECK_USER_AGENT.to_string(),
            ),
            ("Accept".to_string(), UPDATE_CHECK_ACCEPT.to_string()),
        ],
        timeouts_ms: REQUEST_TIMEOUTS_MS,
    })
}

/// 描述子上的两串头拼成 `WinHttpAddRequestHeaders` 要的 CRLF 分隔块。
pub fn header_block(req: &RequestDescriptor) -> String {
    let mut out = String::new();
    for (k, v) in &req.headers {
        out.push_str(k);
        out.push_str(": ");
        out.push_str(v);
        out.push_str("\r\n");
    }
    out
}

/// 「联网 + 解析 + 比较」三步合流的**纯**下半截：回执已到手，只判方向。
/// 主干口径：tag 缺失 / tag 不可比 / `latest <= current` ⇒ 一律「无新版」。
pub fn outcome_from_receipt(receipt: &ReleaseReceipt, current_version: &str) -> Option<UpdateOutcome> {
    if receipt.tag().is_empty() {
        return None;
    }
    if update_verdict(receipt.tag(), current_version) != UpdateVerdict::Newer {
        return None;
    }
    Some(UpdateOutcome {
        tag: receipt.tag().to_string(),
        asset_name: receipt.asset.as_ref().map(|a| a.name.clone()).unwrap_or_default(),
        download_url: receipt.asset.as_ref().map(|a| a.url.clone()).unwrap_or_default(),
        size: receipt.asset.as_ref().map(|a| a.size).unwrap_or(0),
        source: receipt.asset.as_ref().map(|a| a.source),
    })
}

/// 命中一次新版本的完整事实 —— 交给 main.rs 那一刀去画（toast / 下载钮 / 深链都不在本模块）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateOutcome {
    pub tag: String,
    pub asset_name: String,
    pub download_url: String,
    pub size: i64,
    pub source: Option<AssetSource>,
}

impl UpdateOutcome {
    /// 主干 `_aboutDownloadIsDirect`：直链 `.msix` 才走壳内安装。
    pub fn is_direct(&self) -> bool {
        self.source == Some(AssetSource::Msix) && ends_with_msix(&self.download_url)
    }
}

/// 主干 `MainWindow.About.cs` 的 `FormatBytes`：三档 `B` / ` KB` / ` MB`，档位边界是**严格小于**，
/// 第一档无小数、后两档一位小数。`F1` 在主干走当前区域（zh-CN 落 `.`），分叉**写死 `.`** ⇒ 可观察行为一致。
pub fn format_bytes(bytes: i64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// 主干 `MainWindow.UpdateCheck.cs` 的去重旗读点 `RemindedUpdateTag == tag` ⇒ 「要不要提醒」是它的
/// **纯全等取反**：无大小写折叠、无 `trim`、无 `v` 前缀剥离、无「空旗 = 没提醒过」特判（全等已覆盖）。
pub fn should_remind(tag: &str, reminded: &str) -> bool {
    reminded != tag
}

/// 端到端一发：构造 → **真联网** → 解析 → 比较。
/// ⚠ 阻塞式，只能在 worker 线程调；**单测一律不许碰它**（离线机 + 无网络权限 + 污染验收）。
pub fn check_update_now(repo: &str, current_version: &str) -> Result<Option<UpdateOutcome>, String> {
    let Some(req) = build_release_request(repo) else {
        return Ok(None); // 主干：未配置更新源 ⇒ 静默 return
    };
    let body = fetch_release_body(&req)?;
    let receipt = parse_release_receipt(&body)?;
    Ok(outcome_from_receipt(&receipt, current_version))
}

// --- WinHTTP 裸声明（零依赖成规：见 keys.rs:19-21 的策略原文）---------------------

const WINHTTP_ACCESS_TYPE_DEFAULT_PROXY: u32 = 0;
const WINHTTP_FLAG_SECURE: u32 = 0x0080_0000;
const WINHTTP_ADDREQ_FLAG_ADD: u32 = 0x2000_0000;
const WINHTTP_QUERY_STATUS_CODE: u32 = 19;
/// `WinHttpQueryHeaders` 的「把结果按整数取」旗标。SDK 逐字原值
/// （`winhttp.h:1220`）：`#define WINHTTP_QUERY_FLAG_NUMBER 0x20000000`。
///
/// ⚠ UC2 之前这里写成 `0x0000_2000` —— **同样那串十六进制数字、分组写反**（高位那格挪到了低位），
/// 而 `0x00002000` 恰好是同文件 `winhttp.h:1316` 的 `ICU_ESCAPE_AUTHORITY`
/// （`WinHttpCrackUrl` 家族）⇒ 一次「抓错行」的抄写事故。
///
/// 错值的实测后果（UPD1 本机零网络探针）：`WinHttpQueryHeaders(19 | 0x0000_2000)`
/// 的 info level 落 `0x00002013` ⇒ `lasterr=87`（**层级本身被判非法**）；
/// 正确层级 `0x20000013` ⇒ `lasterr=12019`（`ERROR_WINHTTP_INCORRECT_HANDLE_STATE`
/// = 「尚未收回执」的正常抱怨，说明层级已被接受）。⇒ 它与头长度那颗雷同刀才修得干净，
/// 否则日志只是从「头写不进去」换成「取不到状态码」。
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

/// 句柄的 RAII 壳：主干用 `using` 逐层释放，分叉靠它保证**任何早退路径都不漏句柄**
/// （UD1 列的「手工句柄泄漏纪律」这条代价在这里结清）。
struct Session(*mut c_void);

impl Drop for Session {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

fn to_utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn ok(code: i32) -> bool {
    code != 0
}

/// WinHTTP 的报错通道是标准 last-error（本机实测：`WinHttpAddRequestHeaders` 返 `FALSE` 时
/// `GetLastError()=87`，`WinHttpQueryHeaders` 层级非法亦 87，未收回执 12019）。
/// **零新依赖成规**：取码走 std 已有的通道，形状照仓内先例 `procguard.rs` 的同名函数逐字抄；
/// 绝不再开第二枚 `unsafe extern` 块去直连 `GetLastError`（本文件的 `uc8` 正把 extern 块枚数
/// 钉成恰一处，多一块 = 当场打红那把依赖锁）。
///
/// ⚠ 取码**时机**比取码通道更要命：`format!` / `String` 分配都可能插进别的 Win32 调用而把
/// last-error 冲掉 ⇒ 每个 FFI 返回点的**下一句**就抓成局部量，再进 `Err(...)`。
fn last_error() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

/// 从**带结尾 NUL 的 UTF-16 缓冲**求那行的长度：**WCHAR 字符数，不含结尾 NUL**。
///
/// 这是 `WinHttpAddRequestHeaders` 的 `dwHeadersLength` 唯一正确口径。三支柱：
/// 1. SDK SAL（`winhttp.h:1795-1796`）是 `_In_reads_(dwHeadersLength)` —— `_In_reads_`
///    **按被指类型元素计数**，这里元素是 `WCHAR`；真按字节计的参数用的是另一族
///    `_In_reads_bytes_opt_`（同文件 `WinHttpSendRequest` 的 `lpOptional`）⇒ 微软自己的
///    元数据把 `LPCWSTR` 的长度按字符、把 `LPVOID` 的长度按字节。
/// 2. 微软官方 `windows` crate 绑定传的是 `lpszheaders.len()`，即 `&[u16]` 的**元素数**。
/// 3. 本机零网络 P/Invoke 实测（UPD1）：同一 handle 同一串头只改长度 ——
///    字符数 `63 ⇒ TRUE`、字节数 `126 ⇒ FALSE lasterr=87`、`64 ⇒ 87`、`(DWORD)-1 ⇒ TRUE`；
///    反向对照组「字符数恰为 126 的头 + 传 126」⇒ TRUE ⇒ **字节口径被排除**。
///
/// 边界（同批实测）：`0 < 长度 <= 字符数` 才收。**短传不报错、只静默装进半截头** ⇒
/// 「顺手除个 2」这类改法会把硬失败降级成缺 `User-Agent`（GitHub 回 403），更难查。
///
/// 抽成纯函数的理由与本文件的 `format_bytes` / `should_remind` 同规：可算的东西先算成
/// 纯函数再单测，字节口径那种表达式不许住在 FFI 调用点上（那里没法测）。
fn utf16_line_len_chars(buf: &[u16]) -> u32 {
    (match buf.last() {
        Some(&0) => buf.len() - 1,
        _ => buf.len(),
    }) as u32
}

/// 执行 [`RequestDescriptor`]：真发一次 HTTPS GET，回正文。
///
/// 全程 unsafe FFI，句柄一律 `*mut c_void`、`BOOL` 一律当 `i32` 手判、TLS 由
/// `WINHTTP_FLAG_SECURE` 交给系统（这正是 `kernel.rs` 那套裸 `TcpStream` 缺的一颗）。
/// 非 2xx 一律 Err（对齐主干 `EnsureSuccessStatusCode`，限流 403 不会被当成回执去解析）。
pub fn fetch_release_body(req: &RequestDescriptor) -> Result<String, String> {
    if req.scheme != API_SCHEME {
        return Err(format!("本模块只走 https，收到 {}", req.scheme));
    }
    let agent = to_utf16(UPDATE_CHECK_USER_AGENT);
    let server = to_utf16(&req.host);
    let verb = to_utf16(&req.method);
    let object = to_utf16(&req.path);
    let headers = to_utf16(&header_block(req));
    let (resolve, connect, send, receive) = req.timeouts_ms;

    unsafe {
        // 每一枚 FFI 返回点都是同一句式：**调用 → 下一句立刻把 last-error 抓成局部量 → 再判 → 再拼串**。
        // （成功路径也会抓一次：那次读码无副作用，且省得在每个臂上分「该不该取」。）
        let opened = WinHttpOpen(agent.as_ptr(), WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, NO_PROXY, NO_PROXY, 0);
        let err = last_error();
        let session = Session(opened);
        if opened.is_null() {
            return Err(format!("WinHttpOpen 失败 err={err}：拿不到会话句柄"));
        }
        let timed = WinHttpSetTimeouts(session.0, resolve as i32, connect as i32, send as i32, receive as i32);
        let err = last_error();
        if !ok(timed) {
            return Err(format!("WinHttpSetTimeouts 失败 err={err}：分叉不允许无界等待"));
        }
        let connected = WinHttpConnect(session.0, server.as_ptr(), req.port, 0);
        let err = last_error();
        let connect_handle = Session(connected);
        if connected.is_null() {
            return Err(format!("WinHttpConnect 失败 err={err}：{}", req.host));
        }
        let requested = WinHttpOpenRequest(
            connect_handle.0,
            verb.as_ptr(),
            object.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        );
        let err = last_error();
        let request = Session(requested);
        if requested.is_null() {
            return Err(format!("WinHttpOpenRequest 失败 err={err}：https 请求句柄建不出来"));
        }
        // 长度实参只许走 `utf16_line_len_chars`（WCHAR 字符数口径）：见那枚纯函数的三支柱注释。
        // 曾经这里按**字节**算（字符数 × 2 再减 2）⇒ 恒 `FALSE` + `err=87`，检查更新两腿全死。
        let header_chars = utf16_line_len_chars(&headers);
        let added = WinHttpAddRequestHeaders(request.0, headers.as_ptr(), header_chars, WINHTTP_ADDREQ_FLAG_ADD);
        let err = last_error();
        if !ok(added) {
            return Err(format!(
                "WinHttpAddRequestHeaders 失败 err={err}：{header_chars} 字符的头块写不进去（87 = ERROR_INVALID_PARAMETER，先查长度是不是按字符数口径）"
            ));
        }
        // 主干把「发」与「收」混成一条短路 ⇒ TLS / 代理类问题最需要分辨的那一格丢了：这里拆成两臂。
        let sent = WinHttpSendRequest(request.0, std::ptr::null(), 0, std::ptr::null(), 0, 0, 0);
        let err = last_error();
        if !ok(sent) {
            return Err(format!("WinHttpSendRequest 失败 err={err}：发不出去 https://{}{}", req.host, req.path));
        }
        let received = WinHttpReceiveResponse(request.0, std::ptr::null_mut());
        let err = last_error();
        if !ok(received) {
            return Err(format!("WinHttpReceiveResponse 失败 err={err}：收不到回执 https://{}{}", req.host, req.path));
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
            return Err(format!(
                "WinHttpQueryHeaders 失败 err={err}：取不到 HTTP 状态码（12019 = 尚未收回执，87 = 层级或缓冲区非法）"
            ));
        }
        if !(200..300).contains(&status) {
            return Err(format!("GitHub 回执状态码非 2xx：{status}"));
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
        Ok(String::from_utf8_lossy(&body).into_owned())
    }
}

// ---------------------------------------------------------------------------
// 单测：只测「描述子 / 解析 / 比较」三节 —— 网络执行腿一次都不碰
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// 本模块自己的字节 —— 源码锁读它，绝不读别的文件（别家文件漂与本模块无关）。
    fn my_source() -> String {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/", "updatecheck.rs");
        std::fs::read_to_string(path).expect("源码锁读不到 updatecheck.rs")
    }

    fn value(json: &str) -> serde_json::Value {
        serde_json::from_str(json).expect("用例里的 JSON 必须是合法的")
    }

    // ① 四段比较：逐段数值比，`0.8.10.0` 必须大于 `0.8.2.0`（字典序会反）；等值不算更新。
    #[test]
    fn uc1_four_segment_compare_is_numeric_and_strictly_greater() {
        assert_eq!(
            update_verdict("0.8.10.0", "0.8.2.0"),
            UpdateVerdict::Newer,
            "第四段进位必须按数值比，不是字典序"
        );
        assert_eq!(update_verdict("0.9.0.0", "0.10.0.0"), UpdateVerdict::NotNewer);
        assert_eq!(update_verdict("0.8.2.0", "0.8.2.0"), UpdateVerdict::NotNewer, "等值**不算**更新（主干是严格 >）");
        assert_eq!(update_verdict("0.8.2.1", "0.8.2.0"), UpdateVerdict::Newer);
        assert_eq!(update_verdict("1.0.0.0", "0.99.99.99"), UpdateVerdict::Newer);
        assert_eq!(update_verdict("0.8", "0.8.0.0"), UpdateVerdict::NotNewer, "缺段补 0（主干 -1 当 0 比）");
        assert_eq!(
            parse_version_tag("1.2.3.4"),
            Some(Version4 {
                major: 1,
                minor: 2,
                build: 3,
                revision: 4
            })
        );
    }

    // ② `v` 前缀：主干 `Trim().TrimStart('v','V')` —— 大小写两形、外加首尾空白都要认。
    #[test]
    fn uc2_v_prefix_is_case_insensitive_and_trimmed() {
        assert_eq!(update_verdict("v0.8.10.0", "0.8.2.0"), UpdateVerdict::Newer);
        assert_eq!(update_verdict("V0.8.10.0", "0.8.2.0"), UpdateVerdict::Newer);
        assert_eq!(update_verdict("  vv0.8.10.0  ", "0.8.2.0"), UpdateVerdict::Newer);
        assert_eq!(update_verdict("release-v0.8.10.0", "0.8.2.0"), UpdateVerdict::NotComparable);
        assert_eq!(parse_version_tag("v0.8.2.0"), parse_version("0.8.2.0"));
        // 当前版本侧**不剥** v：主干那侧是 Package/Assembly 的四段，不是 tag。
        assert_eq!(parse_version("v0.8.2.0"), None);
    }

    // ③ 垃圾 tag ⇒ 无法比较，且绝不判成「有更新」（.NET TryParse 的接受集）。
    #[test]
    fn uc3_garbage_tag_is_not_comparable_never_newer() {
        for bad in [
            "0.8.2.0-rc1",        // 非数字后缀
            "0.8.2.0+build.7",    // 语义化 build 元数据
            "1.2.3.4.5",          // 五段
            "1",                  // 一段
            "1..2",               // 空段
            "1.2.",               // 尾点
            "+1.2",               // 带号
            "1.2.3.2147483648",   // 超 Int32
            "abc",
            "",
            "   ",
        ] {
            assert_eq!(
                update_verdict(bad, "0.8.2.0"),
                UpdateVerdict::NotComparable,
                "tag {bad:?} 必须落 NotComparable"
            );
        }
        assert_eq!(update_verdict("9.9.9.9", "不是版本"), UpdateVerdict::NotComparable);
        assert_eq!(parse_version("0.8.2.0"), parse_version_tag("v0.8.2.0"));
    }

    // ④ 多 asset 里挑 `.msix`：主干看的是 **name** 的后缀，大小写无关，且取第一颗命中。
    #[test]
    fn uc4_msix_asset_is_picked_by_name_case_insensitively_first_hit_wins() {
        let json = r#"{
            "tag_name":"v0.8.3.0",
            "assets":[
                {"name":"blade2-setup.exe","browser_download_url":"https://cdn/a.exe","size":11},
                {"name":"blade2-arm64.msixbundle","browser_download_url":"https://cdn/b.msixbundle","size":22},
                {"name":"blade2-x64.MSIX","browser_download_url":"https://cdn/c.MSIX","size":33},
                {"name":"blade2-x86.msix","browser_download_url":"https://cdn/d.msix","size":44}
            ]
        }"#;
        let hit = pick_msix_asset(&value(json)).expect("三颗里该挑出 .msix");
        assert_eq!(hit.name, "blade2-x64.MSIX");
        assert_eq!(hit.url, "https://cdn/c.MSIX");
        assert_eq!(hit.size, 33);
        assert_eq!(hit.source, AssetSource::Msix);
        // 主干的直链判据查的是 **url** 且大小写无关 ⇒ 这颗 `.MSIX` 也算直链。
        assert_eq!(hit.is_direct_package_url(), true);
        let json2 = r#"{"assets":[{"name":"a.msix","browser_download_url":"https://cdn/a.Msix","size":5}]}"#;
        let hit2 = pick_msix_asset(&value(json2)).expect("该挑出 a.msix");
        assert_eq!(hit2.is_direct_package_url(), true);
        assert_eq!(hit2.size, 5);
        // url 为空的 asset 主干先 continue，故永不入选。
        let json3 = r#"{"assets":[{"name":"a.msix","browser_download_url":"","size":9},{"name":"b.msix","browser_download_url":"https://cdn/b.msix","size":1}]}"#;
        assert_eq!(pick_msix_asset(&value(json3)).map(|h| h.name), Some("b.msix".to_string()));
    }

    // ⑤ 无 `.msix` ⇒ 明确不命中，并逐级回落（首个带 url 的 asset → html_url 页面 → 全空）。
    #[test]
    fn uc5_no_msix_is_explicitly_none_then_falls_back_two_levels() {
        let json = r#"{"assets":[{"name":"a.zip","browser_download_url":"https://cdn/a.zip","size":7}]}"#;
        assert_eq!(pick_msix_asset(&value(json)), None, "没有 .msix 就是没有，不许硬凑");
        let target = pick_download_target(&value(json)).expect("该回落到首个带 url 的 asset");
        assert_eq!(target.source, AssetSource::FirstAssetUrl);
        assert_eq!(target.url, "https://cdn/a.zip");

        let page = r#"{"html_url":"https://github.com/Live-Rise/Blade2/releases/tag/v0.8.3.0"}"#;
        assert_eq!(pick_msix_asset(&value(page)), None);
        let hit = pick_download_target(&value(page)).expect("无 asset 时该回落 Release 页面");
        assert_eq!(hit.source, AssetSource::ReleasePage);
        assert_eq!(hit.name, "");
        assert_eq!(hit.size, 0);

        assert_eq!(pick_download_target(&value(r#"{"tag_name":"v1.2.3.4"}"#)), None, "三级全空 ⇒ 明确无目标");
    }

    // ⑥ 请求描述子的三串：host / path / 两条头，逐字等值（这是「不联网也能验的那一半」）。
    #[test]
    fn uc6_request_descriptor_pins_host_path_and_the_two_headers() {
        let req = build_release_request(DEFAULT_UPDATE_REPO).expect("默认 repo 该构造出描述子");
        assert_eq!(req.host, "api.github.com");
        assert_eq!(req.path, "/repos/Live-Rise/Blade2/releases/latest");
        assert_eq!(req.method, "GET");
        assert_eq!(req.scheme, "https");
        assert_eq!(req.port, 443);
        assert_eq!(
            req.headers,
            vec![
                ("User-Agent".to_string(), "Blade2-WinUI".to_string()),
                ("Accept".to_string(), "application/vnd.github+json".to_string()),
            ]
        );
        assert_eq!(req.headers.len(), 2, "主干只写两串头，多一味都算偏离");
        assert_eq!(header_block(&req), "User-Agent: Blade2-WinUI\r\nAccept: application/vnd.github+json\r\n");
        assert_eq!(
            release_api_url(DEFAULT_UPDATE_REPO),
            "https://api.github.com/repos/Live-Rise/Blade2/releases/latest"
        );
        // 主干守卫：repo 空 ⇒ 不联网。
        assert_eq!(build_release_request("  "), None);
        assert_eq!(release_api_url(""), "");
    }

    // 回执解析 + 方向判定的合流（tag_name 必须字符串；等值/不可比 ⇒ 无新版）。
    #[test]
    fn uc7_receipt_tag_must_be_a_string_and_drives_the_outcome() {
        let ok = r#"{"tag_name":"v0.8.3.0","assets":[{"name":"a.msix","browser_download_url":"https://cdn/a.msix","size":123}]}"#;
        let receipt = parse_release_receipt(ok).expect("合法 JSON");
        assert_eq!(receipt.tag(), "v0.8.3.0");
        let hit = outcome_from_receipt(&receipt, "0.8.2.0").expect("该判成新版");
        assert_eq!(hit.tag, "v0.8.3.0");
        assert_eq!(hit.download_url, "https://cdn/a.msix");
        assert_eq!(hit.size, 123);
        assert_eq!(hit.is_direct(), true);

        // tag_name 是数字 ⇒ 主干 JsonValueKind.String 守卫 ⇒ 空 ⇒ 无新版。
        let wrong_kind = r#"{"tag_name":123,"assets":[]}"#;
        let r2 = parse_release_receipt(wrong_kind).expect("合法 JSON");
        assert_eq!(r2.tag_name, None);
        assert_eq!(outcome_from_receipt(&r2, "0.8.2.0"), None);

        // 等值 tag ⇒ 不提醒（静默腿那条「同 tag 只提醒一次」的去重旗归 shellfiles 侧，本刀不管）。
        let same = r#"{"tag_name":"0.8.2.0"}"#;
        assert_eq!(outcome_from_receipt(&parse_release_receipt(same).unwrap(), "0.8.2.0"), None);
        // 垃圾 tag ⇒ 不提醒。
        let junk = r#"{"tag_name":"nightly-latest"}"#;
        assert_eq!(outcome_from_receipt(&parse_release_receipt(junk).unwrap(), "0.8.2.0"), None);
        // 非 JSON ⇒ Err（主干是 catch 吞掉；分叉如实上报，由调用方决定静默）。
        assert_eq!(parse_release_receipt("<html>403</html>").is_err(), true);
        // 没有 asset 的新版：tag 命中、下载目标回落页面。
        let page = r#"{"tag_name":"0.9.0.0","html_url":"https://github.com/x/y/releases/tag/v0.9.0.0"}"#;
        let hit = outcome_from_receipt(&parse_release_receipt(page).unwrap(), "0.8.2.0").expect("该判成新版");
        assert_eq!(hit.source, Some(AssetSource::ReleasePage));
        assert_eq!(hit.is_direct(), false);
    }

    // 源码锁 一：本模块唯一的依赖入口就是那颗 winhttp 链接开关，别的一概不许有。
    #[test]
    fn uc8_lock_the_only_dependency_entry_is_the_winhttp_link_switch() {
        let src = my_source();
        // 禁串一律 concat! 拆词，防本锁自己的字面量被数进去。
        assert_eq!(
            src.matches(concat!("#", "[link(name = \"winhttp\")", "]")).count(),
            1,
            "winhttp 链接开关只许一处：多一处就是多引了一颗系统库"
        );
        for banned in [
            concat!("extern", " crate"),
            concat!("windows", "-sys"),
            concat!("windows", "-targets"),
            concat!("req", "west"),
            concat!("rust", "ls"),
            concat!("native", "-tls"),
            concat!("open", "ssl"),
            concat!("h", "yper"),
            concat!("toki", "o"),
        ] {
            assert_eq!(
                src.matches(banned).count(),
                0,
                "零新依赖前提被破：本模块出现了 {banned:?}"
            );
        }
        // 唯一的 `#[link]` 之外，全文件的 `unsafe extern` 也只这一块。
        assert_eq!(src.matches(concat!("unsafe ", "extern \"system\"")).count(), 1);
    }

    // 源码锁 二：只算不画 —— 零 UI 面、零 bin 侧引用（版本串必须走入参）。
    #[test]
    fn uc9b_lock_no_ui_surface_and_no_bin_side_import() {
        let src = my_source();
        for banned in [
            concat!("include", "_str!"),
            concat!("const ", "SHELL_VERSION"),
            concat!("crate::", "main"),
            concat!("super::", "main"),
            concat!("windows", "_reactor"),
            concat!("UI", "Element"),
            concat!("Shell", "Toast"),
            concat!("About", "CheckUpdateButton"),
        ] {
            assert_eq!(
                src.matches(banned).count(),
                0,
                "本模块越界了：出现了 {banned:?} —— 接线与画图是 main.rs 那一刀的事"
            );
        }
        // 公开入口一律收 &str 入参（不 import bin 侧常量的可验形制）。
        assert_eq!(
            src.matches(concat!("current_version: ", "&str")).count(),
            3,
            "update_verdict / outcome_from_receipt / check_update_now 三处收版本入参"
        );
    }

    // 源码锁 三 + 数值锁：超时腿必须**显式**设，且预算不超主干的隐式 100 s。
    #[test]
    fn uc9_lock_the_timeout_leg_is_explicit_and_bounded() {
        assert_eq!(REQUEST_TIMEOUTS_MS, (10_000, 10_000, 30_000, 30_000));
        let (r, c, s, rev) = REQUEST_TIMEOUTS_MS;
        assert_eq!(r + c + s + rev, 80_000, "四段之和钉死在 80 s ≤ 主干 HttpClient 默认 100 s");
        assert_eq!(UPDATE_CHECK_DELAY_MS, 6000, "主干 UpdateCheck.cs:17 的静默延迟");
        let src = my_source();
        // 超时腿的形制：一处 extern 声明 + 一处**真的打在会话句柄上**的调用，不多不少。
        assert_eq!(
            src.matches(concat!("fn ", "WinHttpSetTimeouts(")).count(),
            1,
            "extern 声明只许一处"
        );
        // ⚠ UC2 起这枚 needle 少了 `ok(` 前缀 —— 取码纪律要求「FFI 返回 → 同一句抓码 → 再判」，
        // `ok(WinHttpSetTimeouts(...))` 那种把调用裹在条件里的形状必然丢掉同句取码。
        // 判据不降级：合成「调用形制」+「返回值被绑」+「返回值被当 FALSE 判过」三枚，见下两把。
        assert_eq!(
            src.matches(concat!("WinHttpSetTimeouts(session.0, ", "resolve as i32")).count(),
            1,
            "超时没**照声明的四段**打在会话句柄上 = 退化成主干那种无界等待"
        );
        assert_eq!(
            src.matches(concat!("let timed = ", "WinHttpSetTimeouts(")).count(),
            1,
            "超时那一发的返回值没绑成局部量 = 取码会隔到分配（last_error 的时机口径）"
        );
        assert_eq!(
            src.matches(concat!("if !ok(", "timed)")).count(),
            1,
            "超时那一发的返回值确实被当 FALSE 判过 —— 不判就等于没设"
        );
        // 四处 = const 声明 / 执行腿 doc / OpenRequest 实参 / 本文件那条数值断言。
        assert_eq!(
            src.matches(concat!("WINHTTP_", "FLAG_SECURE")).count(),
            4,
            "TLS 开关只能声明一次、用一次（另两处是 doc 与数值断言）"
        );
    }

    // ⑦⑧ 形制：`ends_with_msix` 的多字节边界与 TLS/状态码常量口径（自定两条之一）。
    #[test]
    fn uc10_msix_suffix_is_byte_boundary_safe_and_status_shape_holds() {
        assert_eq!(ends_with_msix("中文包.msix"), true);
        assert_eq!(ends_with_msix("中文.msixbundle"), false);
        assert_eq!(ends_with_msix(".msix"), true);
        assert_eq!(ends_with_msix("msix"), false);
        assert_eq!(ends_with_msix(""), false);
        assert_eq!(ends_with_msix(".MSIx"), true);
        // 状态码常量与主干 EnsureSuccessStatusCode 的 2xx 口径同形。
        let ok_range = 200..300;
        assert_eq!(ok_range.contains(&200), true);
        assert_eq!(ok_range.contains(&403), false, "限流 403 必须走 Err，不能当回执去 parse");
        assert_eq!(WINHTTP_FLAG_SECURE, 0x0080_0000);
        assert_eq!(API_PORT, 443);
    }

    // 网络腿在单测里**零调用**：把这条纪律本身钉成锁。
    #[test]
    fn uc11_unit_tests_never_call_the_network_executor() {
        let src = my_source();
        let needle = concat!("fetch_", "release_body(");
        let total = src.matches(needle).count();
        // 出现处只许两处：本模块自己的定义 + `check_update_now` 的唯一调用点。
        assert_eq!(total, 2, "执行腿的引用面钉死为 2 处（定义 + 合流调用点）");
        let tests_start = src.find("mod tests").expect("单测模块必须存在");
        assert_eq!(
            src[tests_start..].matches(needle).count(),
            0,
            "单测里出现执行腿调用 = 会真联网，绝对禁止"
        );
        assert_eq!(
            src.matches(concat!("check_update", "_now(")).count(),
            1,
            "端到端合流只有定义处一处，测试不碰、main.rs 尚未接线"
        );
    }

    /// 主干源码锁的取材器：取 `pub fn {name}(` 到其收尾 `}`（列 0）之间那段**产品码本体**。
    /// 只钉 needle 与命中计数 —— **不钉文件总行数**（上一把刀的教训）。
    fn trunk_fn_body(name: &str) -> String {
        let src = my_source();
        let start = src
            .find(&format!("pub fn {name}("))
            .unwrap_or_else(|| panic!("本模块没有 pub fn {name}"));
        let rest = &src[start..];
        let end = rest.find("\n}").expect("函数体收尾 } 必须落在列 0");
        rest[..end].to_string()
    }

    // ⑫ 主干 `FormatBytes` 三档逐字抄：一档整数 `B`，二三档一位小数 ` KB` / ` MB`。
    //    期望串是手算的：1048575 / 1024.0 = 1023.9990234375 ⇒ 一位小数进位成 `1024.0`。
    #[test]
    fn uc12_format_bytes_prints_the_three_trunk_tiers_b_kb_mb() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(100), "100 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048575), "1024.0 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(2621440), "2.5 MB");
        assert_eq!(format_bytes(3_145_728), "3.0 MB");
        // 主干负值无特判：`-1 < 1024` 成立 ⇒ 原样落第一档。
        assert_eq!(format_bytes(-1), "-1 B");
        assert_eq!(format_bytes(-1024), "-1024 B");
    }

    // ⑬ 档位边界：主干是**严格小于**（`< 1024` / `< 1024 * 1024`），故两端点各自落入高一档。
    #[test]
    fn uc13_format_bytes_edges_are_strictly_less_than_1024_and_1048576() {
        assert_eq!(format_bytes(1023), "1023 B", "1023 是第一档的最大值");
        assert_eq!(format_bytes(1024), "1.0 KB", "1024 已不满足 < 1024 ⇒ 升 KB（写成 <= 就红）");
        assert_eq!(format_bytes(1048575), "1024.0 KB", "1048575 仍 < 1048576 ⇒ 留在 KB 档");
        assert_eq!(format_bytes(1048576), "1.0 MB", "1048576 起才进 MB 档");
        // 三档**封顶**：1 GiB 在主干仍打 MB ⇒ 行为面就杀死了「自造第四档」。
        assert_eq!(format_bytes(1_073_741_824), "1024.0 MB", "不许 GB / GiB 档");
        // 小数点写死 `.`（上面的逐串等值已覆盖：落 `,` 的 locale 分支当场红）；千位分隔符同理。
        assert_eq!(format_bytes(1048575), "1024.0 KB", "不许 `1,024.0 KB` 那种加塞");
    }

    // ⑭ 主干读点 `RemindedUpdateTag == tag` ⇒ 提醒与否就是这枚全等的**取反**，别的一律没有。
    #[test]
    fn uc14_should_remind_is_the_pure_negation_of_reminded_update_tag_equals_tag() {
        // 同 tag ⇒ 不提醒（主干 return）；异 tag ⇒ 提醒；空旗 ⇒ 提醒（全等天然覆盖，不许特判）。
        assert_eq!(should_remind("v0.8.3.0", "v0.8.3.0"), false);
        assert_eq!(should_remind("v0.8.3.0", "v0.8.2.0"), true);
        assert_eq!(should_remind("v0.8.3.0", ""), true);
        // 两串都空 ⇒ 主干那侧走不到（tag 空已被上游 return），但纯全等的答案是「不提醒」。
        assert_eq!(should_remind("", ""), false);
        // 不许「剥 v 前缀」：主干比的是整串。
        assert_eq!(should_remind("v0.8.3.0", "0.8.3.0"), true);
        // 不许大小写无关。
        assert_eq!(should_remind("V0.8.3.0", "v0.8.3.0"), true);
        // 不许 trim。
        assert_eq!(should_remind("v0.8.3.0", " v0.8.3.0 "), true);
    }

    // 源码锁 四：`FormatBytes` 的档数与小数位形状 —— 三档、两个 `{:.1}`、无第四档 / 无 KiB / 无取整。
    #[test]
    fn uc15_lock_format_bytes_body_has_exactly_three_tiers_and_no_fourth() {
        let body = trunk_fn_body("format_bytes");
        assert_eq!(body.matches(concat!("format", "!(")).count(), 3, "三档 = 三个 format!，第四档（GB）就是多做");
        assert_eq!(body.matches(concat!("{bytes}", " B")).count(), 1, "第一档：整数 + 空格 B");
        assert_eq!(body.matches(concat!(":.1}", " KB")).count(), 1, "第二档：一位小数");
        assert_eq!(body.matches(concat!(":.1}", " MB")).count(), 1, "第三档：一位小数");
        assert_eq!(body.matches("{:.1}").count(), 2, "只许两处一位小数；第一档带小数就算多做");
        assert_eq!(body.matches("< 1024").count(), 2, "两道**严格小于**的档位闸");
        for banned in [
            " GB",
            concat!("Ki", "B"),
            concat!("Mi", "B"),
            concat!("{:,", "}"),
            concat!("ceil", ""),
            concat!("round", ""),
            concat!("is_negative", ""),
            concat!("<=", " 1024"),
            concat!("1024", " * 1024 * 1024"),
            concat!("locale", ""),
        ] {
            assert_eq!(
                body.matches(banned).count(),
                0,
                "保真度缺陷（多做/边界写反/自造单位）：本体出现了 {banned:?}"
            );
        }
        // doc 指回主干串锚：定义**之前**的那一段里，`FormatBytes` 只许出现一次（就是那行 doc）。
        let src = my_source();
        let at = src.find("pub fn format_bytes(").expect("format_bytes 必须在模块里");
        assert_eq!(src[..at].matches(concat!("Format", "Bytes")).count(), 1);
    }

    // 源码锁 五：`should_remind` 本体只许一枚全等取反；「清旗」回路（§4 T-6）在本模块零出现。
    #[test]
    fn uc16_lock_reminded_update_tag_flag_body_is_pure_equality_and_never_cleared() {
        let body = trunk_fn_body("should_remind");
        assert_eq!(body.matches("reminded != tag").count(), 1, "本体唯一判据 = 全等取反");
        assert_eq!(body.matches("==").count(), 0, "不许出现正向全等分支（含 !(a == b) 那种改写）");
        for banned in [
            concat!("to_", "lowercase"),
            concat!("eq_ignore_ascii_case", ""),
            concat!("trim", ""),
            concat!("starts_with", ""),
            concat!("is_empty", ""),
        ] {
            assert_eq!(
                body.matches(banned).count(),
                0,
                "全等被改弱了：本体出现了 {banned:?} —— 主干就是 == 一枚"
            );
        }
        let src = my_source();
        // doc 指回主干串锚：`should_remind` 定义之前那一段里，`RemindedUpdateTag == tag` 只许出现一次。
        let at = src.find("pub fn should_remind(").expect("should_remind 必须在模块里");
        assert_eq!(src[..at].matches(concat!("RemindedUpdateTag ", "== tag")).count(), 1);
        // 分叉侧「清旗 / 改写旗」的回路一处都不许有（§4 T-6）：全文件既不重置也不加工那枚串。
        assert_eq!(
            src.matches(concat!("reminded", " = ")).count(),
            0,
            "reminded 一旦被重新赋值，就不再是主干那枚纯全等了"
        );
        assert_eq!(
            src.matches(concat!("clear", "_reminded")).count()
                + src.matches(concat!("reset", "_reminded")).count(),
            0,
            "主干没有任何复位/清空点，分叉也不许开"
        );
    }

    // -----------------------------------------------------------------------
    // UC2：WinHTTP 那两处「check 全绿、真机必死」的口径，本轮立成正向锁。
    // 判据口径：纯函数单测 + 源码静态锁；**不许**用真机联调来证明（离线机 / 无网络权限）。
    // -----------------------------------------------------------------------

    // ⑰ 长度口径本身（纯函数）。UPD1 实测：字符数 63 ⇒ TRUE，字节数 126 ⇒ FALSE + err=87。
    //     旧的 `header_block` 那把 `uc6` 锁钉的是**内容**，对传给 WinHTTP 的**长度实参**零覆盖 ——
    //     这正是「17 颗单测全绿而真机必死」的形状，这枚补的就是那一格。
    #[test]
    fn uc17_utf16_line_len_chars_counts_wchar_units_not_bytes_and_strips_the_nul() {
        let req = build_release_request(DEFAULT_UPDATE_REPO).expect("默认 repo 该构造出描述子");
        let block = header_block(&req);
        let buf = to_utf16(&block);
        // 真实那串头：63 个 UTF-16 元素 + 1 枚结尾 NUL = 缓冲 64 枚。
        assert_eq!(block.encode_utf16().count(), 63);
        assert_eq!(buf.len(), 64, "缓冲含结尾 NUL 共 64 枚");
        assert_eq!(utf16_line_len_chars(&buf), 63, "该传给 WinHTTP 的是**字符数 63**");
        // 反面 witness（旧盘那行就地算式，逐字复算一遍）：它给的是**字节数**，正解的两倍。
        let old_byte_caliber = (buf.len() * 2 - 2) as u32;
        assert_eq!(old_byte_caliber, 126, "旧实参 = 126，正是 UPD1 探针里恒 FALSE + lasterr=87 的那枚");
        assert_ne!(old_byte_caliber, utf16_line_len_chars(&buf), "两口径永不相等 ⇒ 选错就是真机必死");
        assert_eq!(old_byte_caliber, utf16_line_len_chars(&buf) * 2, "错值恰是正解的两倍：少个 ×2 的「顺手修法」也照样错");
        // 形制的其余分支。
        assert_eq!(utf16_line_len_chars(&[]), 0, "空缓冲 ⇒ 0（实测传 0 被拒 87，这里不伪造长度）");
        assert_eq!(utf16_line_len_chars(&[0]), 0, "只有结尾 NUL ⇒ 0 字符");
        assert_eq!(utf16_line_len_chars(&[0x41, 0]), 1, "『A\\0』⇒ 1");
        assert_eq!(utf16_line_len_chars(&[0x41, 0x42]), 2, "无结尾 NUL ⇒ 整串都是内容，不多剥也不少剥");
        assert_eq!(utf16_line_len_chars(&[0x41, 0, 0x42, 0]), 3, "只剥**最后**那一枚 NUL；中间的字符照常计入");
        // 非 BMP（代理对）按 UTF-16 元素计数 —— WinHTTP 的口径是 WCHAR 个数，不是 Rust char 数。
        let astral = "User-Agent: Blade2\u{1F600}\r\n";
        let astral_buf = to_utf16(astral);
        assert_eq!(utf16_line_len_chars(&astral_buf) as usize, astral.encode_utf16().count());
        assert_ne!(utf16_line_len_chars(&astral_buf) as usize, astral.chars().count(), "代理对占两元素，char 数会更小");
    }

    // ⑱ 长度口径的**接线**：纯函数写对 ≠ 调用点用了它。这把锁钉的就是「住不住在调用点」。
    #[test]
    fn uc18_lock_the_add_request_headers_length_argument_is_wired_to_the_char_count() {
        let body = trunk_fn_body("fetch_release_body");
        assert_eq!(
            body.matches(concat!("let header_chars = ", "utf16_line_len_chars(&headers);")).count(),
            1,
            "字符数在执行腿里只许算一处，且算的就是那串头"
        );
        assert_eq!(
            body.matches(concat!("headers.as_ptr(), ", "header_chars,")).count(),
            1,
            "长度实参必须是 header_chars：换回任何就地算式，纯函数再对也救不了真机"
        );
        // 字节口径的形状在执行腿里零容身（UPD1 建议的「禁 ×2 住在那枚实参位置」，这里按 ASCII 钉）。
        for banned in [concat!("len()", " * 2"), concat!("size_of", "::<u16>()")] {
            assert_eq!(
                body.matches(banned).count(),
                0,
                "字节口径表达式回到了执行腿：{banned:?} —— dwHeadersLength 要的是字符数"
            );
        }
        // 反手一笔：extern 声明里那枚参数名还叫 length（u32），别被它带回到「字节」的联想上。
        assert_eq!(
            my_source().matches(concat!("headers: *const u16, ", "length: u32")).count(),
            1,
            "extern 声明的形制不许被顺手改掉（改类型 = 改 ABI，属另一码事）"
        );
    }

    // ⑲ `WINHTTP_QUERY_FLAG_NUMBER` 的值锁。旧盘是同串十六进制数字**分组写反** ⇒ 单靠「值不等于旧值」
    //     那种负锁抓不住，这里正负两头都钉，并把合成层级钉成实测过的那个数。
    #[test]
    fn uc19_lock_query_flag_number_is_the_high_bit_and_is_composed_at_the_call() {
        // SDK 逐字原值（`winhttp.h:1220`）；微软官方绑定的十进制同值形（536870912）。
        assert_eq!(WINHTTP_QUERY_FLAG_NUMBER, 0x2000_0000);
        assert_eq!(WINHTTP_QUERY_FLAG_NUMBER, 536_870_912);
        // ⚠ 这一枚才是「少写 4 个 0」当场会杀的断言。
        assert_eq!(
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            0x2000_0013,
            "实测被接受的层级：12019（尚未收回执）；旧盘合成出 0x00002013 ⇒ 实测 87（层级被拒）"
        );
        assert_eq!(WINHTTP_QUERY_FLAG_NUMBER & 0x2000_0000, 0x2000_0000, "高位那格必须在");
        assert_eq!(WINHTTP_QUERY_FLAG_NUMBER & 0x0000_FFFF, 0, "低 16 位必须全空 —— 旧盘错值正落在那儿");
        assert_ne!(WINHTTP_QUERY_FLAG_NUMBER, 0x0000_2000, "那是隔壁 WinHttpCrackUrl 家族的 ICU_ESCAPE_AUTHORITY");
        assert_eq!(WINHTTP_QUERY_STATUS_CODE, 19, "SDK winhttp.h:1120");
        assert_eq!(WINHTTP_ADDREQ_FLAG_ADD, 0x2000_0000, "winhttp.h:1355：与 NUMBER 同值，本就各管各的参数位");
        let src = my_source();
        // 声明处的**写法**也钉住（防「同数字分组写反」被再翻一次）。
        assert_eq!(
            src.matches(concat!("WINHTTP_QUERY_FLAG_NUMBER: u32 = ", "0x2000_0000;")).count(),
            1,
            "旗标声明只许这一种写法，一处"
        );
        // 调用点必须真的把两枚合成一层：丢了 NUMBER 就当字符串取，缓冲区口径全变。
        assert_eq!(
            trunk_fn_body("fetch_release_body")
                .matches(concat!("WINHTTP_QUERY_STATUS_CODE | ", "WINHTTP_QUERY_FLAG_NUMBER,")).count(),
            1,
            "状态码那一发必须是「按整数取」的合成层级"
        );
    }

    // ⑳ 可观测性锁：9 枚 FFI 返回点逐枚同句取码，发/收不再短路合并。
    #[test]
    fn uc20_lock_every_ffi_return_point_grabs_the_last_error_into_its_err_arm() {
        let body = trunk_fn_body("fetch_release_body");
        // 取码通道 = 仓内已有先例的形状（procguard.rs 同名函数），零新依赖、零新 extern 块。
        assert_eq!(
            my_source().matches(concat!("fn ", "last_error() -> i32")).count(),
            1,
            "取码函数只许一枚定义"
        );
        assert_eq!(
            my_source().matches(concat!("std::io::Error::", "last_os_error()")).count(),
            1,
            "取码走 std 已有通道：想直连 GetLastError 就得新开 extern 块，当场打红 uc8"
        );
        // 9 = Open / SetTimeouts / Connect / OpenRequest / AddRequestHeaders / Send / Receive / QueryHeaders / ReadData
        assert_eq!(
            body.matches(concat!("let err = ", "last_error();")).count(),
            9,
            "FFI 返回点的同句取码：一枚都不能漏（漏一枚 = 又回到吞码的固定字面量）"
        );
        assert_eq!(
            body.matches(concat!("失败 err=", "{err}")).count(),
            9,
            "9 枚早退臂逐枚把码写进 Err 串（例：WinHttpAddRequestHeaders 失败 err=87）"
        );
        // 逐臂点名：少任何一枚，下面的循环就红。
        for arm in [
            "WinHttpOpen ",
            "WinHttpSetTimeouts ",
            "WinHttpConnect ",
            "WinHttpOpenRequest ",
            "WinHttpAddRequestHeaders ",
            "WinHttpSendRequest ",
            "WinHttpReceiveResponse ",
            "WinHttpQueryHeaders ",
            "WinHttpReadData ",
        ] {
            assert_eq!(
                body.matches(&format!("{arm}失败 err={{err}}")).count(),
                1,
                "早退臂 {arm:?} 没把错误码带进串里"
            );
        }
        // 形状级缺陷：把两次调用压成一枚条件（本轮拆掉的那颗）。
        assert_eq!(
            body.matches(concat!("|| !", "ok(")).count(),
            0,
            "发/收不许再合并成一枚条件 —— 短路后分不清是发不出去还是收不到回执"
        );
        assert_eq!(body.matches(concat!("WinHttpSendRequest", " 失败")).count(), 1);
        assert_eq!(body.matches(concat!("WinHttpReceiveResponse", " 失败")).count(), 1);
        // 「写死结论式文案」本身就是缺陷：旧串把注意力引向内容/条数，真凶是长度参数。
        assert_eq!(
            body.matches(concat!("两串头", "写不进去")).count(),
            0,
            "旧文案自带误导，不许原样留着"
        );
    }
}
