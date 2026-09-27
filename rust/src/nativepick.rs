//! 原生文件选择器**原语**（**NP1 第 1 把刀**，台账 #160 B 刀第二片）：`comdlg32!GetOpenFileNameW`
//! 裸 FFI、零新依赖 —— 分叉缺的整条选择器链（`tmp/n40-report.md:207` 那格）就是这块货架。
//!
//! # 这枚原语的形状（两条主代理裁定，逐字落在这里）
//!
//! **裁定 1 —— 线程模型：本模块不创建线程、不碰 `CoInitialize*` 的归属。**
//! [`pick_file`] 就是「在**调用方线程**上跑 `GetOpenFileNameW` 的模态循环并阻塞返回」。理由（U5 §3.3 ②(b)
//! 交上来要裁的那格，现已裁）：owner/置顶/真模态是**可截图对比的可见面**，而「对话框开着期间后台回包
//! 延迟」只是时序、截图不可见，且代价自限于设置页那一格；另开 STA 线程那条路会把对话框变成可被主窗
//! 遮挡 = 另一种可见差。**⇒ 接线刀如要改线程模型，只需换调用点的线程（自己 `spawn_background` 出去调，
//! 或就在 UI 线程上调），本模块的签名一个字都不动。**
//!
//! **裁定 2 —— 本片不接 `on_click`，只交原语/货架。** `main.rs` 今天冻结，所以本模块**非测试读者 = 0**：
//! 全仓 `src/*.rs` 除本文件外没有任何一处引用 `nativepick`。这是**备案**不是遗漏（先例 = `src/toast.rs:26`
//! 那句「主干 11 发调用面**一颗都不接**（本刀只交原语）」），也**不许**为了让它「有用」去造一个假调用点。
//! 接线那把刀（`Msg::SkinPick` / `Msg::SkinPicked`）删掉本文件里的 `#[allow(dead_code)]` 即可。
//!
//! # 档位表 = 主干现测表（防「多做」的尺；取证命令见 `tmp/np1-report.md` §2）
//!
//! 主干全仓 `FileOpenPicker` 实测 **4 发**（不是简报说的 2 发 ⇒ 见报告 §4 纠正 1），所以这里就是
//! **4 档** [`PICK_PROFILES`]，**主干没有的一档都不预铺**：
//!
//! | 档 | 主干串锚 | filter 逐档原文 | 起始位置 |
//! |---|---|---|---|
//! | [`PickerId::SkinImage`] | `private async Task PickAndApplySkinAsync()` | `.png` `.jpg` `.jpeg` `.webp` `.bmp` | PicturesLibrary |
//! | [`PickerId::SkinVideo`] | `private async Task PickAndApplySkinVideoAsync()` | `.mp4` `.webm` `.mov` `.m4v` | VideosLibrary |
//! | [`PickerId::Attachment`] | `private async void OnAttachClick(object sender, RoutedEventArgs e)` | `*` | DocumentsLibrary |
//! | [`PickerId::PetZip`] | `private async Task PickAndInstallPetZipAsync()` | `.zip` | Downloads |
//!
//! # 本刀**不做**的（都已有主，别在这里长出第二份）
//!
//! * `IFileOpenDialog` / 任何 COM 路线 —— 仓内零 `windows` 主 crate（`Cargo.lock` 只有
//!   `windows-reactor` 的九颗传递依赖），COM 路线要新依赖 = 违反零依赖前提。
//! * **多选**（`OFN_ALLOWMULTISELECT`）与拖放 —— 见 [`PickerId::Attachment`] 那条「缺口备案」。
//! * 拷槽 / `fs::copy` / `shell-skin.img` 槽名常量 / 清 WE 标记 / 刷背景 —— **全是 `main.rs` 的活**：
//!   主干那两发 handler 从 picker 只读 `file.Path` **一个字符串**（U5 §1.3 的决定性事实），所以本模块
//!   的返回值就是一枚 `String`，后面那些一步都不该在这儿。
//! * 任何 i18n 新键、任何 UI 渲染、任何 `ContentDialog` 错误框（分叉侧那格在 `main.rs`）。
//! * 窗口 owner 句柄的**取得**：本模块只收 [`pick_file`] 的 `owner_hwnd` 入参（给 `null` 就是「无 owner」
//!   档，取舍归接线刀，见 U5 §3.3 与 pr2 §5 那条待裁项）。
//!
//! # 单测纪律
//!
//! 零前台、零真对话框、零联网。**任何测试都不许调 [`pick_file`]** —— `GetOpenFileNameW` 是模态阻塞的，
//! 一弹就把 CI 挂死（同 `toast.rs` 对 `Shell_NotifyIconW`、`updatecheck.rs` 对 `fetch_release_body` 的
//! 处理方式）。判据只落在纯层可判的形状上：buffer 长度算式、`l_struct_size` 与 `size_of` 一致、
//! filter 串的**字节**布局、profile 档位计数（串锚锁）、错误码→分支映射，以及 [`known_folder_units`]
//! 那族**唯一允许真调 Win32** 的只读交叉测试（GUID 不许凭记忆抄，见那颗 const 处的说明）。

use std::ffi::c_void;
use std::mem::size_of;
// ============================================================================
// 一、主干实测表的可执行形态：四档 profile
// ============================================================================

/// 主干 `FileOpenPicker` 的**实测发数**（= 本模块的档位上限）。
/// 复跑：`grep -rn "new Windows.Storage.Pickers.FileOpenPicker" --include=*.cs . | grep -v obj/` ⇒ 4 行。
pub const MAINLINE_PICKER_CALLSITES: usize = 4;

/// 档位身份。变体顺序 = 主干 handler 名的字典序，**不是**重要性顺序。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerId {
    /// 导入图片皮肤（主干单槽语义：换下视频）。
    SkinImage,
    /// 导入视频皮肤。
    SkinVideo,
    /// 输入区「＋」附件。
    Attachment,
    /// 宠物 zip 安装。
    PetZip,
}

/// 起始位置档：主干 `PickerLocationId` 在本仓实际用到的那四颗。
///
/// **为什么落 `FOLDERID_*` 而不是 `FOLDERID_*Library`**：`knownfolders.h` 里
/// `FOLDERID_Pictures`（`:153`）与 `FOLDERID_PicturesLibrary`（`:312`）是**两颗不同的 GUID**，
/// 后者解析出来是 `.library-ms` 那个聚合虚拟文件夹，`lpstrInitialDir` 要的是**文件系统目录**。
/// 主干 `PickerLocationId.PicturesLibrary` 在桌面端演成的就是用户「图片」目录 ⇒ 这里取前者。
/// 这条判据由 [`pictures_initial_dir_matches_the_user_folder`] 那族交叉测试钉住，不靠人眼。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartFolder {
    /// 主干 `PickerLocationId.PicturesLibrary`。
    Pictures,
    /// 主干 `PickerLocationId.VideosLibrary`。
    Videos,
    /// 主干 `PickerLocationId.DocumentsLibrary`。
    Documents,
    /// 主干 `PickerLocationId.Downloads`。
    Downloads,
}

/// 一档选择器的全部形状。**只有 filter 与起始目录两格** —— 主干那四发除了这两格和「取回后干什么」
/// 之外没有第三格可对齐（`title` / `defext` / 初始文件名主干都没有 ⇒ 一律不铺）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickProfile {
    pub id: PickerId,
    /// 主干那一发的**方法名串锚**（自持在货架里，让档位与主干的对应关系可被单测锁死）。
    pub mainline_anchor: &'static str,
    /// 主干 `FileTypeFilter.Add(...)` 的**逐档原文 + 顺序**（含前导点，`*` 那档没点）。
    /// 这是「尺」：分叉侧任何一档少 `.jpeg` 或少 `.m4v` 都是与主干不符（U5 §纠正 9）。
    pub mainline_filter: &'static [&'static str],
    /// 过滤器下拉那一行显示名。**分叉自造**并在此备案：主干走 WinRT，压根没有「显示名」这一格，
    /// 而 U5 §3.3(a) 已判「comdlg32 内部文案与 WinRT picker 不保证逐字一致，且取证不能（禁前台）」
    /// ⇒ 这串由 [`win32_filter_pattern`] 的同一批后缀机械生成，人不参与，永不与 filter 档漂移。
    pub filter_display: &'static str,
    /// 起始位置。
    pub start_folder: StartFolder,
}

/// 主干 `FileOpenPicker` 的 `FileTypeFilter` 是**后缀**列表，comdlg32 要的是 `*.png;*.jpg` 形状的
/// **模式**列表 ⇒ 唯一换算是「带前导点者前面补 `*`，裸 `*` 原样」。
/// 写成纯函数是为了让它可被单测逐字钉住（[`the_four_filter_patterns_are_byte_exact`]）。
pub fn win32_filter_pattern(entries: &[&str]) -> String {
    let mut out = String::new();
    for entry in entries {
        if !out.is_empty() {
            out.push(';');
        }
        if *entry == "*" {
            out.push('*');
        } else if entry.starts_with('.') {
            // `commdlg.h` 的模式是 glob：主干的 `.png` 要演成 `*.png` —— **点是模式的一部分**，
            // 只补 `*` 不补点会得到 `*png`，那一档什么都筛不到（第一次落笔就踩了，靠字节锁抓回）。
            out.push('*');
            out.push_str(entry);
        } else {
            // 主干没有这一档（四发的每一枚实参要么以点开头、要么就是 `*`）。
            // 真走到这里 = 有人在货架上添了主干没有的档 ⇒ 原样透出，交给字节锁去红。
            out.push_str(entry);
        }
    }
    out
}

/// 货架本体：**四档，一表看全**。顺序 = 上表顺序。
pub const PICK_PROFILES: [PickProfile; MAINLINE_PICKER_CALLSITES] = [
    PickProfile {
        id: PickerId::SkinImage,
        mainline_anchor: "private async Task PickAndApplySkinAsync()",
        mainline_filter: &[".png", ".jpg", ".jpeg", ".webp", ".bmp"],
        filter_display: "*.png;*.jpg;*.jpeg;*.webp;*.bmp",
        start_folder: StartFolder::Pictures,
    },
    PickProfile {
        id: PickerId::SkinVideo,
        mainline_anchor: "private async Task PickAndApplySkinVideoAsync()",
        mainline_filter: &[".mp4", ".webm", ".mov", ".m4v"],
        filter_display: "*.mp4;*.webm;*.mov;*.m4v",
        start_folder: StartFolder::Videos,
    },
    PickProfile {
        id: PickerId::Attachment,
        mainline_anchor: "private async void OnAttachClick(object sender, RoutedEventArgs e)",
        mainline_filter: &["*"],
        filter_display: "*",
        start_folder: StartFolder::Documents,
    },
    PickProfile {
        id: PickerId::PetZip,
        mainline_anchor: "private async Task PickAndInstallPetZipAsync()",
        mainline_filter: &[".zip"],
        filter_display: "*.zip",
        start_folder: StartFolder::Downloads,
    },
];

/// 按身份取档（接线刀用；线性扫四格，不建 map）。
pub fn profile_for(id: PickerId) -> Option<PickProfile> {
    PICK_PROFILES.into_iter().find(|p| p.id == id)
}

// ============================================================================
// 二、缓冲区纪律：UTF-16 宽字符 + 自持长度
// ============================================================================

/// `lpstrFile` 的缓冲容量，单位 = **UTF-16 码元（含结尾那颗 `\0`）**。
///
/// 取 `32_768` 的理由（不是随手抓的一档）：`GetOpenFileNameW` 的 `nMaxFile` 按码元计，
/// 而 Win32 路径的天花板是 `\\?\` 前缀那 32 767 字符一档 —— 取 `32_768` 就是
/// 「天花板 + 终止符」，于是**缓冲区这一侧永远不会成为截断原因**，剩下的截断只能来自 OS
/// 自己（那一档走 [`classify_failure`] 的 `FNERR_BUFFERTOOSMALL`，如实报、不静默）。
/// 主干走 WinRT，长度归 OS 管、没有这一枚常量 ⇒ **这枚数是分叉自造**，备案于此。
pub const FILE_BUFFER_UNITS: usize = 32_768;

/// 一整个 `\0` 结尾的宽串（内部工具，也供 filter 块复用）。
fn wide_units(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 组 `lpstrFilter` 那一块：**显示名 `\0` 模式 `\0`** 成对，末尾**再补一颗 `\0`** 收尾
/// （comdlg32 的双终止符规约：最后一对的结束位是空显示名）。
///
/// 返回的是**码元**，交给调用方保活到 `GetOpenFileNameW` 返回为止。
/// 判据一律走 [`filter_block_bytes`] 的字节断言，别拿字符串比。
pub fn build_filter_wide(display: &str, pattern: &str) -> Vec<u16> {
    let mut block = wide_units(display);
    block.extend(wide_units(pattern));
    block.push(0);
    block
}

/// [`build_filter_wide`] 的 UTF-8 视图，只为让「布局」这件事能在测试里按**字节**断言。
pub fn filter_block_bytes(display: &str, pattern: &str) -> Vec<u8> {
    let units = build_filter_wide(display, pattern);
    let mut bytes = Vec::with_capacity(units.len() * 2);
    for unit in units {
        bytes.extend(unit.to_le_bytes());
    }
    bytes
}

/// 从 `lpstrFile` 缓冲里取回那串路径。**只有两种可能，没有第三种**：
/// * `Some(text)` —— 缓冲里有终止符 ⇒ 取到的是**完整**路径（可能为空串，见下）。
/// * `None` —— 扫遍 `buffer.len()` 格都没等到 `\0` ⇒ 路径**超出**缓冲，
///   此时**如实报**（调用方给 [`PickOutcome::Failed`]），绝不静默截断成一条指不到真文件的错误路径。
///
/// 纯函数、不碰 OS ⇒ 两档都能在单测里真跑。
pub fn take_picked_path(buffer: &[u16]) -> Option<String> {
    let end = buffer.iter().position(|unit| *unit == 0)?;
    Some(String::from_utf16_lossy(&buffer[..end]))
}

// ============================================================================
// 三、`OPENFILENAMEW` 与 comdlg32 入口（裸声明，零新依赖）
// ============================================================================

// 形制先例 = `kernel.rs:3353-3366`（`shell32!SHGetKnownFolderPath` + `ole32!CoTaskMemFree` 那一对）、
// `toast.rs:36-39`、`updatecheck.rs:422-435`、`keys.rs` / `scroll.rs` / `clipboard.rs` /
// `procguard.rs` / `crashlog.rs`。链接前提实测见报告 §1（`Lib/10.0.22621.0/um/x64/comdlg32.lib` 在架）。
//
// 这里用 `//` 不用 `///`：rustc 不给 extern 块生成文档，`///` 会叫 `unused_doc_comments` 判一条警告。
#[link(name = "comdlg32")]
unsafe extern "system" {
    /// `commdlg.h:254` `WINCOMMDLGAPI BOOL APIENTRY GetOpenFileNameW(LPOPENFILENAMEW)`。
    /// **模态、阻塞、在调用线程上跑自己的消息循环** —— 裁定 1 的那条代价就是它。
    fn GetOpenFileNameW(open_file_name: *mut OpenFileNameW) -> i32;
    /// `commdlg.h:1158` `DWORD APIENTRY CommDlgExtendedError(VOID)`。
    /// **取消档与错误档的唯一分诊口**（`0` = 用户取消，见 [`classify_failure`]）。与上面同块声明。
    fn CommDlgExtendedError() -> u32;
}

/// 字段序 = `commdlg.h:203-233` `typedef struct tagOFNW { … } OPENFILENAMEW` **逐颗照抄**
/// （`#ifdef _MAC` 那两颗不算数；`_WIN32_WINNT >= 0x0500` 的尾部三颗在架）。
/// 名字一律 snake_case（本仓成规），C 名写在行尾注释里；布局由
/// [`open_file_name_w_layout_matches_the_sdk_header`] 按 `offset_of!` 逐颗钉。
/// 不 derive `Clone`：结构体里全是 borrowed 裸指针，复制一份等于复制一堆悬垂风险。
#[repr(C)]
#[derive(Debug, Default)]
pub struct OpenFileNameW {
    /// `DWORD lStructSize` —— **恒取 `size_of::<OpenFileNameW>() as u32`，写字面量就是 64 位/Unicode
    /// 的经典崩点**（comdlg32 按它做版本与宽度判定，猜错只回 `CDERR_STRUCTSIZE`）。见 [`pick_file`]。
    pub l_struct_size: u32, // lStructSize
    /// `HWND hwndOwner` —— 入参透传，本模块不取不造。
    pub hwnd_owner: *mut c_void, // hwndOwner
    /// `HINSTANCE hInstance` —— 不用 `OFN_ENABLETEMPLATE` ⇒ 恒 `null`。
    pub h_instance: *mut c_void, // hInstance
    /// `LPCWSTR lpstrFilter` —— [`build_filter_wide`] 的产物（双 `\0` 收尾）。
    pub lpstr_filter: *const u16, // lpstrFilter
    /// `LPWSTR lpstrCustomFilter` —— 主干无「记住上次过滤器」这一档 ⇒ 不铺，恒 `null`。
    pub lpstr_custom_filter: *mut u16, // lpstrCustomFilter
    /// `DWORD nMaxCustFilter` —— 同上，恒 0。
    pub n_max_custom_filter: u32, // nMaxCustFilter
    /// `DWORD nFilterIndex` —— 0 = 由 OS 记档。主干没有初值 ⇒ 不预铺。
    pub n_filter_index: u32, // nFilterIndex
    /// `LPWSTR lpstrFile` —— 可写缓冲（[`FILE_BUFFER_UNITS`] 码元，含终止符），调用前全 `\0`。
    pub lpstr_file: *mut u16, // lpstrFile
    /// `DWORD nMaxFile` —— **上面那格的容量**，两者必须同源于 [`FILE_BUFFER_UNITS`]。
    pub n_max_file: u32, // nMaxFile
    /// `LPWSTR lpstrFileTitle` —— 主干从不读文件名那一格（只读 `file.Path`）⇒ 不给缓冲。
    pub lpstr_file_title: *mut u16, // lpstrFileTitle
    /// `DWORD nMaxFileTitle` —— 同上，恒 0。
    pub n_max_file_title: u32, // nMaxFileTitle
    /// `LPCWSTR lpstrInitialDir` —— 已知文件夹解析结果；解析不出来 ⇒ `null`（= 交回 OS 记档）。
    pub lpstr_initial_dir: *const u16, // lpstrInitialDir
    /// `LPCWSTR lpstrTitle` —— 主干没有标题格（WinRT 无此 API）⇒ 恒 `null`，走 OS 默认。
    pub lpstr_title: *const u16, // lpstrTitle
    /// `DWORD Flags` —— 见 [`OFN_FLAGS`]。
    pub flags: u32, // Flags
    /// `WORD nFileOffset` —— **出参**，本模块不读 ⇒ 留 0。
    pub n_file_offset: u16, // nFileOffset
    /// `WORD nFileExtension` —— 出参，同上。
    pub n_file_extension: u16, // nFileExtension
    /// `LPCWSTR lpstrDefExt` —— 主干不补扩展名（`FileTypeFilter` 只是过滤器）⇒ 恒 `null`。
    pub lpstr_def_ext: *const u16, // lpstrDefExt
    /// `LPARAM lCustData` —— 无 hook ⇒ 恒 0。（`LPARAM` 是指针宽，故 `isize` 而非 `i32`。）
    pub l_cust_data: isize, // lCustData
    /// `LPOFNHOOKPROC lpfnHook` —— 不用 `OFN_ENABLEHOOK` ⇒ 恒 `null`。
    pub lpfn_hook: *mut c_void, // lpfnHook
    /// `LPCWSTR lpTemplateName` —— 不用 `OFN_ENABLETEMPLATE` ⇒ 恒 `null`。
    pub lp_template_name: *const u16, // lpTemplateName
    /// `void* pvReserved` —— `(_WIN32_WINNT >= 0x0500)` 尾部三颗之一。
    pub pv_reserved: *mut c_void, // pvReserved
    /// `DWORD dwReserved` —— 恒 0。
    pub dw_reserved: u32, // dwReserved
    /// `DWORD FlagsEx` —— `OFX_*` 一族主干无对应档 ⇒ 恒 0。
    pub flags_ex: u32, // FlagsEx
}

// ---- 旗（`commdlg.h:279-307` 现值）。**主干没有 OFN 旗可照抄**（它走 WinRT，仓内 0 枚 FOS_/OFN_），
// ---- 所以每一枚都是「主干那发的**可观察行为** ⇒ 哪面旗」的推导，逐枚注明，推导本身进报告 §4 备案。
// ---- 反向纪律：`OFN_OVERWRITEPROMPT` 不许加（主干覆盖无提示，`File.Copy(…, overwrite: true)` 直接覆），
// ---- `OFN_ALLOWMULTISELECT` 不许加（裁定不做），`OFN_ENABLEHOOK` / `_TEMPLATE` 一族不许加。
const OFN_HIDEREADONLY: u32 = 0x0000_0004; // commdlg.h:279
const OFN_NOCHANGEDIR: u32 = 0x0000_0008; // commdlg.h:280
const OFN_PATHMUSTEXIST: u32 = 0x0000_0800; // commdlg.h:288
const OFN_FILEMUSTEXIST: u32 = 0x0000_1000; // commdlg.h:289
const OFN_EXPLORER: u32 = 0x0008_0000; // commdlg.h:297
const OFN_ENABLESIZING: u32 = 0x0080_0000; // commdlg.h:303

/// 六枚，逐枚的**行为依据**（不是主干字面量 —— 那玩意儿不存在，见上面的备案）：
/// * `OFN_EXPLORER`：主干是 WinRT `IFileOpenDialog` 那一代的壳，legacy 三栏视图 = 一眼可辨的可见差。
/// * `OFN_HIDEREADONLY`：WinRT picker 的界面里没有「只读」勾选框。
/// * `OFN_PATHMUSTEXIST` + `OFN_FILEMUSTEXIST`：`PickSingleFileAsync` **只可能**回一个已存在的文件，
///   这两枚把 comdlg32 的自由输入面收拢到同一档（手工敲个不存在的名字 ⇒ 对话框内部拦，不放行）。
/// * `OFN_NOCHANGEDIR`：legacy 对话框默认会**改进程 CWD**，WinRT 不改 ⇒ 不加这枚就多出一条主干没有的
///   进程副作用（`kernel.rs` 那条「回落到进程 CWD = system32 ⇒ 工具全废」的反例就是同类坑）。
/// * `OFN_ENABLESIZING`：WinRT picker 可拉伸；此枚要求同时有 `OFN_EXPLORER`（头注原话）。
pub const OFN_FLAGS: u32 = OFN_EXPLORER
    | OFN_HIDEREADONLY
    | OFN_PATHMUSTEXIST
    | OFN_FILEMUSTEXIST
    | OFN_NOCHANGEDIR
    | OFN_ENABLESIZING;

// ---- cderr.h 里被 [`classify_failure`] / [`describe_extended_error`] 用到的那几枚。
// ---- 全表可复跑：`grep -n "define CDERR_\|define FNERR_" "…/shared/cderr.h"`
const CDERR_STRUCTSIZE: u32 = 0x0001; // cderr.h:21
const CDERR_INITIALIZATION: u32 = 0x0002; // cderr.h:22
const FNERR_INVALIDFILENAME: u32 = 0x3002; // cderr.h:54
const FNERR_BUFFERTOOSMALL: u32 = 0x3003; // cderr.h:55

/// 一次选择的结果三态。**取消与失败是两态**，不许合并（合并 = 取消时给用户出一句主干不会出的话）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickOutcome {
    /// 用户选中了一个文件，值 = 绝对路径（主干那发读的也就是这一个串）。
    Picked(String),
    /// 用户按了取消 / 关了对话框 ⇒ **静默**：调用方什么都不该做（主干 `if (file is null) return;`）。
    Cancelled,
    /// 真失败，值 = 一句**指得回原因**的诊断串（OS 错误码 + 名字）。
    Failed(String),
}

/// 失败码分诊：`GetOpenFileNameW` 回 0 之后，`CommDlgExtendedError()` 的值为 **0 ⇒ 用户取消**，
/// 非 0 ⇒ 真错误（裁定 4 的可执行形态）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancelOrError {
    Cancelled,
    Error(u32),
}

/// 纯函数 ⇒ 两档都能真红（[`cancel_and_error_are_two_distinct_branches`]）。
pub fn classify_failure(extended_error: u32) -> CancelOrError {
    if extended_error == 0 {
        CancelOrError::Cancelled
    } else {
        CancelOrError::Error(extended_error)
    }
}

/// 错误码 → 人话。**只有非零那一档才会走到这里**。
/// 已知码给名字，未知码给 `UNKNOWN`：宁可写得粗，不许把码吞了（吞掉就退化成「不知道为什么」）。
pub fn describe_extended_error(code: u32) -> &'static str {
    match code {
        CDERR_STRUCTSIZE => "CDERR_STRUCTSIZE（lStructSize 与结构体实际尺寸不符）",
        CDERR_INITIALIZATION => "CDERR_INITIALIZATION（comdlg32 初始化失败）",
        FNERR_INVALIDFILENAME => "FNERR_INVALIDFILENAME（文件名不合法）",
        FNERR_BUFFERTOOSMALL => "FNERR_BUFFERTOOSMALL（路径超出 lpstrFile 缓冲）",
        _ => "UNKNOWN（comdlg32 未列名的扩展错误码）",
    }
}

/// 把 [`CancelOrError`] 收成 [`PickOutcome`]：取消 ⇒ `Cancelled`（静默），错误 ⇒ 带码带名字的 `Failed`。
pub fn failure_outcome(extended_error: u32) -> PickOutcome {
    match classify_failure(extended_error) {
        CancelOrError::Cancelled => PickOutcome::Cancelled,
        CancelOrError::Error(code) => PickOutcome::Failed(format!(
            "GetOpenFileNameW 失败，CommDlgExtendedError=0x{code:04X} {}",
            describe_extended_error(code)
        )),
    }
}

// ============================================================================
// 四、已知文件夹：`lpstrInitialDir` 的取值
// ============================================================================

/// `GUID`（`knownfolders.h` 的 `struct IID` 布局：一个 u32 + 两个 u16 + 八个 u8）。
/// 与 `kernel.rs:3329-3334` 那颗同形（那枚是本模块私有的，`pub(crate)` 都没给，故这里自己声明一份）。
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct KnownFolderId {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

// ⚠ **以下四颗 GUID 一律从 `Include/10.0.22621.0/um/knownfolders.h` 现测逐字抄，零凭记忆。**
// 抄错的后果本仓有真实事故：`kernel.rs:3336-3345` 那段注释记的就是上一版 `FOLDERID_DOCUMENTS`
// 把 GUID 错抄成 `{FDD39AD0-1043-4CEE-BEBE-4DDC30CC8C21}`，而 `SHGetKnownFolderPath` 对**错的
// GUID 也回 `0x80070002`**（`ERROR_FILE_NOT_FOUND`）—— 函数照样「失败得很有道理」，调用点只看得到
// 「解析失败」，于是**整条修复静默失效**，最后靠 `SHGetFolderPathW(CSIDL_PERSONAL)` 那把**不吃这颗
// GUID 的第二条口径**交叉比对才被抓回来（`kernel.rs:3898-3930`）。
// ⇒ 本片每一颗新 GUID 都配一把同法交叉测试，且**断言两条真路径相等**（不是「都成功」）：
//   [`pictures_initial_dir_matches_the_user_folder`] /
//   [`videos_initial_dir_matches_the_user_folder`] /
//   [`documents_initial_dir_matches_the_user_folder`] /
//   [`downloads_initial_dir_matches_the_registry_oracle`]
//
// 取值的可复跑命令（cwd = `/c/Program Files (x86)/Windows Kits/10/Include/10.0.22621.0/um`）：
//   `grep -n "FOLDERID_Pictures,\|FOLDERID_Videos,\|FOLDERID_Documents,\|FOLDERID_Downloads," knownfolders.h`

/// `knownfolders.h:153`
/// `DEFINE_KNOWN_FOLDER(FOLDERID_Pictures, 0x33E28130, 0x4E1E, 0x4676, 0x83, 0x5A, 0x98, 0x39, 0x5C, 0x3B, 0xC3, 0xBB)`
const FOLDERID_PICTURES: KnownFolderId = KnownFolderId {
    data1: 0x33E2_8130,
    data2: 0x4E1E,
    data3: 0x4676,
    data4: [0x83, 0x5A, 0x98, 0x39, 0x5C, 0x3B, 0xC3, 0xBB],
};

/// `knownfolders.h:189`
/// `DEFINE_KNOWN_FOLDER(FOLDERID_Videos, 0x18989B1D, 0x99B5, 0x455B, 0x84, 0x1C, 0xAB, 0x7C, 0x74, 0xE4, 0xDD, 0xFC)`
const FOLDERID_VIDEOS: KnownFolderId = KnownFolderId {
    data1: 0x1898_9B1D,
    data2: 0x99B5,
    data3: 0x455B,
    data4: [0x84, 0x1C, 0xAB, 0x7C, 0x74, 0xE4, 0xDD, 0xFC],
};

/// `knownfolders.h:87`
/// `DEFINE_KNOWN_FOLDER(FOLDERID_Documents, 0xFDD39AD0, 0x238F, 0x46AF, 0xAD, 0xB4, 0x6C, 0x85, 0x48, 0x03, 0x69, 0xC7)`
/// （与 `kernel.rs:3346` 那颗同值，但**独立现测得来**：本模块不许 `use kernel::…`，那颗是私有的。）
const FOLDERID_DOCUMENTS: KnownFolderId = KnownFolderId {
    data1: 0xFDD3_9AD0,
    data2: 0x238F,
    data3: 0x46AF,
    data4: [0xAD, 0xB4, 0x6C, 0x85, 0x48, 0x03, 0x69, 0xC7],
};

/// `knownfolders.h:252`
/// `DEFINE_KNOWN_FOLDER(FOLDERID_Downloads, 0x374de290, 0x123f, 0x4565, 0x91, 0x64, 0x39, 0xc4, 0x92, 0x5e, 0x46, 0x7b)`
const FOLDERID_DOWNLOADS: KnownFolderId = KnownFolderId {
    data1: 0x374D_E290,
    data2: 0x123F,
    data3: 0x4565,
    data4: [0x91, 0x64, 0x39, 0xC4, 0x92, 0x5E, 0x46, 0x7B],
};

/// 档 → 那颗 GUID。**刻意不 `pub`**：`KnownFolderId` 是本模块私有的字节载体，把它透出到货架上
/// 只会让 `lib.rs` 长出一个主干没有的类型（`private_interfaces` 那条警告就是这一格）。
/// 同模块的 `#[cfg(test)]` 用 `use super::*` 照样能拿到它 ⇒ 交叉测试的强度不受影响。
fn known_folder_id(folder: StartFolder) -> KnownFolderId {
    match folder {
        StartFolder::Pictures => FOLDERID_PICTURES,
        StartFolder::Videos => FOLDERID_VIDEOS,
        StartFolder::Documents => FOLDERID_DOCUMENTS,
        StartFolder::Downloads => FOLDERID_DOWNLOADS,
    }
}

#[link(name = "shell32")]
unsafe extern "system" {
    /// `kernel.rs:3355` 同名的那颗（跨模块各声明一份是仓内成规；`#[link]` 只加一个链接开关）。
    fn SHGetKnownFolderPath(
        folder_id: *const KnownFolderId,
        flags: u32,
        token: *mut c_void,
        path: *mut *mut u16,
    ) -> i32;
}

#[link(name = "ole32")]
unsafe extern "system" {
    /// `kernel.rs:3365` 同名的那颗：`SHGetKnownFolderPath` 交出来的缓冲归 `CoTaskMemFree` 管。
    fn CoTaskMemFree(pointer: *mut c_void);
}

/// 起始目录的宽串（`\0` 结尾）；解析不出来 ⇒ `None` ⇒ `lpstrInitialDir` 给 `null`，
/// 让 OS 用自己的记档。**不**回落到 `%USERPROFILE%\Pictures` 那种字符串拼接：拼接式在
/// OneDrive 重定向 / 域环境里会指到别的目录上去 —— 这条口径是本仓 `kernel.rs:3369-3371`
/// 为「文档」那格立过的原话（本机 `Documents` 未被重定向，实测见 `tmp/np1-report.md` §2.3，
/// 但**别的机器上会** ⇒ 判定不靠本机运气，一律走 `SHGetKnownFolderPath`）。
///
/// 这枚函数**真调 Win32**，但只读、不弹窗 ⇒ 允许进单测（唯一被允许真调 Win32 的一族）。
pub fn known_folder_units(folder: StartFolder) -> Option<Vec<u16>> {
    let guid = known_folder_id(folder);
    let mut buffer: *mut u16 = std::ptr::null_mut();
    // flags = KF_FLAG_DEFAULT(0)、token = NULL ⇒ 当前进程令牌所属用户（同 kernel.rs:3372 的口径）。
    let code = unsafe {
        SHGetKnownFolderPath(
            &guid,
            0,
            std::ptr::null_mut(),
            &mut buffer,
        )
    };
    if code != 0 || buffer.is_null() {
        return None;
    }
    let mut units: Vec<u16> = Vec::new();
    let mut offset = 0usize;
    loop {
        let unit = unsafe { *buffer.add(offset) };
        if unit == 0 {
            break;
        }
        units.push(unit);
        offset += 1;
    }
    unsafe { CoTaskMemFree(buffer as *mut c_void) };
    (!units.is_empty()).then_some(units)
}

/// [`known_folder_units`] 的 `String` 视图（只为交叉测试好写断言与好打印诊断）。
pub fn known_folder_path(folder: StartFolder) -> Option<String> {
    let units = known_folder_units(folder)?;
    let text = String::from_utf16_lossy(&units);
    (!text.is_empty()).then_some(text)
}

// ============================================================================
// 五、原语本体（**单测一律不许调**：它是模态阻塞的）
// ============================================================================

impl PickProfile {
    /// 本档的 comdlg32 过滤器模式串（[`win32_filter_pattern`] 套在 [`PickProfile::mainline_filter`] 上）。
    pub fn filter_pattern(&self) -> String {
        win32_filter_pattern(self.mainline_filter)
    }
}

/// 把「`GetOpenFileNameW` 的返回值 + `CommDlgExtendedError()` + `lpstrFile` 缓冲」三元组收成
/// [`PickOutcome`] 的**纯分诊**。
///
/// 这半截从 [`pick_file`] 里抽出来的唯一理由：`pick_file` 永不能在单测里跑（模态阻塞），
/// 但它的**分支判断**可以。变异探针 M14（把成功/失败判反）第一版**全绿溜过** —— 就是这一格
/// 没被纯层包住 ⇒ 抽出来，由 [`the_return_and_error_pair_map_to_the_three_outcomes`] 钉死。
pub fn outcome_from(returned: i32, extended_error: u32, buffer: &[u16]) -> PickOutcome {
    if returned == 0 {
        return failure_outcome(extended_error);
    }
    match take_picked_path(buffer) {
        Some(path) if path.is_empty() => PickOutcome::Failed(
            "GetOpenFileNameW 回成功但 lpstrFile 是空串（OS 形状异常，不猜原因）".to_owned(),
        ),
        Some(path) => PickOutcome::Picked(path),
        None => PickOutcome::Failed(format!(
            "选中的路径超出 lpstrFile 缓冲（{} 码元），未截断返回",
            buffer.len()
        )),
    }
}

/// 在**调用方线程**上跑一发 `GetOpenFileNameW` 的模态循环，阻塞到用户给出选择或取消。
///
/// `owner_hwnd` 原样透传（`null_mut()` = 无 owner 那一档）。**本函数不建线程、不 `CoInitialize`。**
/// 见模块头「裁定 1」：要改线程模型 = 改调用点，签名不动。
///
/// 三条出口，逐条对应一条裁定（**判断全在 [`outcome_from`] 那半截纯层里**，所以它们都可测；
/// 本函数只剩「组载荷 + 一发 FFI + 把三元组交给纯层」，那一发才是永不可测的部分）：
/// * 回非 0 且缓冲里有终止符 ⇒ [`PickOutcome::Picked`]（完整路径）。
/// * 回非 0 但缓冲里没有终止符 ⇒ [`PickOutcome::Failed`]（超长**如实报**，绝不静默截断）。
/// * 回 0 ⇒ 交 [`failure_outcome`]：`CommDlgExtendedError()==0` 走 [`PickOutcome::Cancelled`]（静默），
///   非 0 才带错误串回来。
#[allow(dead_code)] // 非测试读者 = 0（裁定 2：本刀只交原语/货架，`main.rs` 今天冻结）。
                    // 唯一的读者 = #160 B 刀接线那一把的 `Msg::SkinPick` / `Msg::SkinPicked` 两臂；
                    // 形制先例 = `src/toast.rs:212`。接线落地后删这行属性即可，不必动任何逻辑。
pub fn pick_file(profile: &PickProfile, owner_hwnd: *mut c_void) -> PickOutcome {
    let filter = build_filter_wide(profile.filter_display, &profile.filter_pattern());
    let initial_dir = known_folder_units(profile.start_folder);
    let mut buffer = vec![0u16; FILE_BUFFER_UNITS];
    let mut ofn = OpenFileNameW {
        // 唯一合法写法：从 size_of 现取。**不许写字面量**（模块头与字段注释各钉了一遍）。
        l_struct_size: size_of::<OpenFileNameW>() as u32,
        hwnd_owner: owner_hwnd,
        lpstr_filter: filter.as_ptr(),
        lpstr_file: buffer.as_mut_ptr(),
        n_max_file: buffer.len() as u32,
        lpstr_initial_dir: initial_dir.as_ref().map_or(std::ptr::null(), |units| units.as_ptr()),
        flags: OFN_FLAGS,
        ..Default::default()
    };
    let chosen = unsafe { GetOpenFileNameW(&mut ofn) };
    let extended = if chosen == 0 {
        unsafe { CommDlgExtendedError() }
    } else {
        0
    };
    outcome_from(chosen, extended, &buffer)
}

// ============================================================================
// 六、单测：形状与判据（零前台 / 零真对话框 / 零联网）
// ============================================================================
//
// 裁定 2 的备案位就挂在 [`pick_file`] 上那一行 `#[allow(dead_code)]`（先例 = `toast.rs:212`）：
// 本模块**非测试读者 = 0**，接线刀落地时删那行属性即可，逻辑一行不动。这里不再另立任何
// 「为了让它有用」的假调用点 —— 那正是 `tests/mw2_deadbuttons.rs` 原罪护栏点名的形状。

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------ 档位计数 = 主干实测发数（串锚锁）

    /// 货架档位与主干那四发**一一对应**。主干的方法名在这里当**字面串**钉住：
    /// 少一档、多一档、顺序里的对应关系错一格，这枚就红。
    /// （故意**不**去读主干 `.cs` 文件：本仓 12 枚既有红全那样的「主干漂移锁」，第 13 枚不该由这把刀添。）
    #[test]
    fn the_four_profiles_are_locked_to_the_four_mainline_handler_names() {
        const MAINLINE_ANCHORS: [&str; 4] = [
            "private async Task PickAndApplySkinAsync()",
            "private async Task PickAndApplySkinVideoAsync()",
            "private async void OnAttachClick(object sender, RoutedEventArgs e)",
            "private async Task PickAndInstallPetZipAsync()",
        ];
        let anchors: Vec<&str> = PICK_PROFILES.iter().map(|p| p.mainline_anchor).collect();
        assert_eq!(anchors, MAINLINE_ANCHORS.to_vec(), "货架档位 ≠ 主干实测发数");
        assert_eq!(PICK_PROFILES.len(), MAINLINE_PICKER_CALLSITES);
        assert_eq!(MAINLINE_PICKER_CALLSITES, 4, "主干现测就是 4 发 FileOpenPicker，多一档就是预铺");
        // 档身份不许重复（重复 = 两档抢同一发主干）。
        let ids: Vec<PickerId> = PICK_PROFILES.iter().map(|p| p.id).collect();
        for (index, id) in ids.iter().enumerate() {
            assert_eq!(
                ids.iter().filter(|other| *other == id).count(),
                1,
                "档 {id:?} 出现了不止一次"
            );
            assert_eq!(profile_for(*id).map(|p| p.id), Some(*id), "profile_for 取不回 {id:?}");
            let _ = index;
        }
        assert_eq!(profile_for(PickerId::SkinImage).map(|p| p.mainline_anchor), Some(MAINLINE_ANCHORS[0]));
    }

    /// 每档的 filter **逐档原文**（顺序 + 大小写 + `.jpeg` / `.m4v` 一枚不许省）钉回主干现测值。
    #[test]
    fn each_profile_carries_the_mainline_extension_list_verbatim() {
        assert_eq!(
            PICK_PROFILES[0].mainline_filter,
            &[".png", ".jpg", ".jpeg", ".webp", ".bmp"],
            "图片档少一枚或换了顺序"
        );
        assert_eq!(
            PICK_PROFILES[1].mainline_filter,
            &[".mp4", ".webm", ".mov", ".m4v"],
            "视频档少 .m4v 就是与主干不符（U5 §纠正 9）"
        );
        assert_eq!(PICK_PROFILES[2].mainline_filter, &["*"], "附件档主干只有 `*` 一枚");
        assert_eq!(PICK_PROFILES[3].mainline_filter, &[".zip"], "宠物档主干只有 .zip 一枚");
        // 主干 11 枚 FileTypeFilter.Add ⇒ 四档合计 11 枚，一枚不多一枚不少。
        let total: usize = PICK_PROFILES.iter().map(|p| p.mainline_filter.len()).sum();
        assert_eq!(total, 11, "FileTypeFilter 档位总数与主干实测不符");
    }

    /// 每档的起始位置档（主干只有那四颗 `PickerLocationId`）。
    #[test]
    fn each_profile_starts_at_the_mainline_location() {
        let starts: Vec<StartFolder> = PICK_PROFILES.iter().map(|p| p.start_folder).collect();
        assert_eq!(
            starts,
            vec![
                StartFolder::Pictures,
                StartFolder::Videos,
                StartFolder::Documents,
                StartFolder::Downloads
            ]
        );
    }

    // ------------------------------------------------ filter 串的字节布局

    /// `lpstrFilter` 的布局：**显示名 `\0` 模式 `\0`** + 结尾**再一颗 `\0`**（双终止符）。
    /// 判据一律 `as_bytes()`，不拿字符串比 —— 字符串比较看不见多出来的那颗 `\0`。
    #[test]
    fn the_filter_block_is_pair_null_terminated_in_bytes() {
        let units = build_filter_wide("*.zip", "*.zip");
        let bytes: Vec<u8> = units.iter().flat_map(|unit| unit.to_le_bytes()).collect();
        assert_eq!(bytes, filter_block_bytes("*.zip", "*.zip"), "两条字节视图不同源");
        // 逐字节：'*.zip' = 5 码元 + 终止符 = 6 ⇒ 两块 12 + 收尾 1 = 13 码元 = 26 字节。
        assert_eq!(units.len(), 13, "码元数 = 显示名(5+1) + 模式(5+1) + 收尾(1)");
        assert_eq!(bytes.len(), 26);
        // 结尾**双** `\0`（两颗码元各 2 字节全零）—— 少一颗就是 comdlg32 越界读。
        assert_eq!(&bytes[bytes.len() - 4..], &[0, 0, 0, 0], "结尾少一颗码元零");
        // 对内部：第一块的终止符在第 6 颗码元。
        assert_eq!(units[5], 0, "显示名与模式之间必须有 `\\0`");
        assert_eq!(units[11], 0, "模式之后必须有 `\\0`");
        assert_eq!(units[12], 0, "整块末尾必须有第二颗 `\\0`");
        // 每颗码元的**高字节**（小端 ⇒ 奇数下标）都该是 0：全 ASCII 时漏出一位就是 UTF-16 转换写坏了。
        // （`skip(1).step_by(2)` 才是奇数下标；`step_by(2).skip(1)` 会退化成偶数下标 = 低字节，
        // 第一次写就反了，被这条自己抓红。）
        assert!(bytes.iter().skip(1).step_by(2).all(|high| *high == 0), "小端高字节不该有值");
        assert!(bytes.iter().step_by(2).any(|low| *low != 0), "低字节全零 = 整块是空串，判据是空的");
    }

    /// 四档的 comdlg32 模式串逐字钉住（`.png` → `*.png` 的换算只此一家，`*` 不许变 `**`）。
    #[test]
    fn the_four_filter_patterns_are_byte_exact() {
        let patterns: Vec<String> = PICK_PROFILES.iter().map(|p| p.filter_pattern()).collect();
        assert_eq!(
            patterns,
            vec![
                "*.png;*.jpg;*.jpeg;*.webp;*.bmp",
                "*.mp4;*.webm;*.mov;*.m4v",
                "*",
                "*.zip",
            ],
            "模式串与主干 FileTypeFilter 的换算漂了"
        );
        // 附件那档的 `*` 若被换算成 `**`，过滤器就什么都收了 —— 单独钉一刀。
        assert_eq!(win32_filter_pattern(&["*"]), "*");
        assert_eq!(win32_filter_pattern(&[]), "");
    }

    /// 显示名与模式**同源**：显示名里出现的后缀集合必须等于该档的后缀集合（防「按卡片描述建过滤器」）。
    #[test]
    fn display_names_never_drift_from_the_extension_list() {
        for profile in PICK_PROFILES.iter() {
            let pattern = profile.filter_pattern();
            if profile.id == PickerId::Attachment {
                assert_eq!(profile.filter_display, "*");
                continue;
            }
            for entry in profile.mainline_filter {
                assert!(
                    pattern.contains(entry) && profile.filter_display.contains(entry),
                    "{entry} 在 {:?} 这一档里没同时出现在模式与显示名",
                    profile.id
                );
            }
        }
        // 反向：主干那张卡的描述串少了 `.jpeg` 与 `.m4v`（U5 §纠正 9）⇒ 货架**不许**照描述建。
        assert!(PICK_PROFILES[0].filter_display.contains(".jpeg"));
        assert!(PICK_PROFILES[1].filter_display.contains(".m4v"));
    }

    // ------------------------------------------------ 结构体形状与缓冲长度算式

    /// `l_struct_size` 的唯一合法来源就是 `size_of`（写字面量 / 写 `size_of::<u32>()` / 写半截尺寸都红）。
    #[test]
    fn l_struct_size_comes_from_size_of_never_a_literal() {
        let ofn = OpenFileNameW {
            l_struct_size: size_of::<OpenFileNameW>() as u32,
            ..Default::default()
        };
        assert_eq!(ofn.l_struct_size, size_of::<OpenFileNameW>() as u32);
        assert_ne!(ofn.l_struct_size, 0, "为 0 就是 comdlg32 一句 CDERR_STRUCTSIZE");
        // 4.0 时代那枚「截断尺寸」不该出现在本模块里（`OPENFILENAME_SIZE_VERSION_400W` = 到 lpTemplateName 为止）。
        assert_ne!(
            ofn.l_struct_size,
            std::mem::offset_of!(OpenFileNameW, lp_template_name) as u32 + size_of::<*const u16>() as u32,
            "把 lStructSize 钉成 v4 截断尺寸 = 丢尾部三颗字段"
        );
    }

    /// **源码锁**（形制同 `dock.rs:1431` 的 `include_str!` 自读 + `kernel.rs:8289` 注的
    /// 「起始锚 → 结束锚的**首次出现**开窗」）：`pick_file` 那一截产品码里必须真的从 `size_of`
    /// 现取，且**一条线程/COM 归属都不许长出来**（裁定 1）。
    /// 上一枚测试只钉得住「值对」，钉不住「怎么算出来的」——而 64 位/Unicode 的经典崩点正是有人
    /// 把 `size_of` 换成一个手抄的数字：x64 上它今天也对，换 32 位就崩。
    #[test]
    fn the_shelf_source_never_hardcodes_the_struct_size() {
        // 只开**产品码**那一窗：模块头注里就写着 `CoInitialize*` / `IFileOpenDialog` / `spawn_background`
        // 这几个词（作为「不做」的备案），而本测试自己的禁串数组也写着字面量 —— 两头都得排除。
        let source = product_source();

        let assignment = "l_struct_size: size_of::<OpenFileNameW>() as u32";
        assert!(
            source.contains(assignment),
            "`pick_file` 里那行 `l_struct_size` 不再是 size_of 现取了"
        );
        // 反向：产品码那一窗里不许出现「把尺寸当字面量赋给这格」的写法。
        for forbidden in ["l_struct_size: 152", "l_struct_size: 88", "l_struct_size: size_of::<u32>()"] {
            assert!(
                !source.contains(forbidden),
                "货架上出现了把 lStructSize 钉成字面量的写法：{forbidden}"
            );
        }
        // 裁定 1 的形制护栏：这一窗里不许出现任何**调用形状**（带括号 / 带路径段）。
        for forbidden in [
            "CoInitialize(",
            "CoInitializeEx(",
            "CoUninitialize(",
            "thread::spawn(",
            "std::thread::Builder",
            "CoCreateInstance",
            "spawn_background(",
        ] {
            assert!(
                !source.contains(forbidden),
                "裁定 1 不许的形状长出来了：{forbidden}"
            );
        }
    }

    /// 全测试模块**一颗都不许真调** [`pick_file`]：`GetOpenFileNameW` 是模态阻塞的，弹一框就把 CI 挂死。
    /// 源码锁（读自己这文件的 `mod tests` 那一窗，同 [`product_source`] 的开窗手法）。
    ///
    /// ⚠ 禁串一律**分段现拼**，不许原样写在数组里：本测试自己的数组就在被扫的那一窗里，
    /// 字面禁串会被自己扫出来 ⇒ 假红（第一版就是这么自伤的 —— 这是本片第三次踩「锁扫到自己」，
    /// 前两次的教训分别在 [`product_source`] 的止锚与 [`the_filter_block_is_pair_null_terminated_in_bytes`]
    /// 的字节下标上）。
    #[test]
    fn no_test_ever_calls_the_modal_primitive() {
        fn needle(parts: &[&str]) -> String {
            parts.concat()
        }
        let whole = include_str!("nativepick.rs");
        let start = whole
            .find(&needle(&["#[cfg(test)]\n", "mod tests {"]))
            .expect("测试模块开窗锚没了");
        let tests = &whole[start..];
        assert!(tests.len() > 2_000, "测试窗太窄 = 锚落错位置，这条锁是空的");
        for forbidden in [
            needle(&["pick", "_file("]),
            needle(&["GetOpen", "FileNameW("]),
            needle(&["CommDlg", "ExtendedError("]),
        ] {
            assert!(
                !tests.contains(&forbidden),
                "测试里出现了会真弹模态框的调用：{forbidden}"
            );
        }
        // 正面：这枚模块**确实**有测试在跑（别哪天测试整块被删了还留一条永真锁）。
        let count = tests.matches("#[test]").count();
        assert!(count >= 22, "本模块的测试枚数掉到 {count} 了（定稿时是 22）");
    }

    /// [`the_shelf_source_never_hardcodes_the_struct_size`] 的开窗：
    /// 起锚 = 产品码第一行 `use std::ffi::c_void;`，止锚 = 测试模块那**两行整块**。两头都必要：
    /// * 不起锚 ⇒ 模块头注里那些「不做的东西」（`IFileOpenDialog` / `CoInitialize*`）会自己撞反锁；
    /// * 不止锚 ⇒ 本测试数组里那些禁串字面量会自己撞反锁（锁把自己锁红，第一次写就踩了）。
    ///
    /// ⚠ **止锚必须是结构串，不许是裸 `#[cfg(test)]`**：本文件 `known_folder_id` 的 doc 里就写着
    /// 「同模块的 `#[cfg(test)]` …」那句，裸 token 的首次出现落在 `pick_file` **之前**，窗口会被
    /// 截成一截空窗 ⇒ 正锁永假（第一次写就踩了，靠它自己报红才抓到）。
    /// 这与「读函数体必须读签名起、到下一个 `fn` 止」是同一条教训（`kernel.rs:8289` 那条注）。
    fn product_source() -> &'static str {
        let whole = include_str!("nativepick.rs");
        const START_ANCHOR: &str = "use std::ffi::c_void;";
        const END_ANCHOR: &str = "#[cfg(test)]\nmod tests {";
        let start = whole
            .find(START_ANCHOR)
            .unwrap_or_else(|| panic!("开窗起锚 {START_ANCHOR} 没了 = 文件头被改写"));
        let end = whole[start..]
            .find(END_ANCHOR)
            .unwrap_or_else(|| panic!("开窗止锚 {END_ANCHOR} 没了 = 测试模块被挪走"))
            + start;
        let window = &whole[start..end];
        // 开窗自检：窗口过窄（止锚落错位）就是一条永假锁，这里直接拒绝，不留「绿着的假锁」。
        assert!(
            window.len() > 4_000,
            "产品码窗口只有 {} 字节 = 止锚多半落错了位置，这条锁是空的",
            window.len()
        );
        window
    }

    /// 字段序 / 填充与 `commdlg.h:203-233` 逐颗对齐。任何一次「顺手调个字段顺序」都会撞红这里。
    #[test]
    fn open_file_name_w_layout_matches_the_sdk_header() {
        // 偏移按头注字段序 + x64 的 `#[repr(C)]` 规则**现算**（不抄任何人的表）。
        assert_eq!(std::mem::offset_of!(OpenFileNameW, l_struct_size), 0);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, hwnd_owner), 8);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, h_instance), 16);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, lpstr_filter), 24);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, lpstr_custom_filter), 32);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, n_max_custom_filter), 40);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, n_filter_index), 44);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, lpstr_file), 48);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, n_max_file), 56);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, lpstr_file_title), 64);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, n_max_file_title), 72);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, lpstr_initial_dir), 80);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, lpstr_title), 88);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, flags), 96);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, n_file_offset), 100);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, n_file_extension), 102);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, lpstr_def_ext), 104);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, l_cust_data), 112);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, lpfn_hook), 120);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, lp_template_name), 128);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, pv_reserved), 136);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, dw_reserved), 144);
        assert_eq!(std::mem::offset_of!(OpenFileNameW, flags_ex), 148);
        assert_eq!(size_of::<OpenFileNameW>(), 152, "x64 的 tagOFNW 就是 152 字节");
        assert_eq!(size_of::<*mut c_void>(), 8, "本锁只在 64 位下成立");
        // 字段**宽度**也得钉，光钉偏移会漏：`lCustData` 是 `LPARAM` = **指针宽**，写成 `i32` 在 x64 上
        // 偏移与总尺寸**一个字都不变**（后面那颗 `lpfnHook` 自带 8 字节对齐，把洞补上了），
        // 只有下面这条宽度锁抓得到 —— 变异探针 M4 实测就是从这条缝里溜过去的，故补锁。
        let widths = OpenFileNameW::default();
        assert_eq!(
            size_of_val(&widths.l_cust_data),
            size_of::<*mut c_void>(),
            "lCustData 必须是指针宽（`LPARAM` = `LONG_PTR`）；写成 i32 在 x64 上尺寸看不出来，换 32 位就错位"
        );
        assert_eq!(size_of_val(&widths.n_file_offset), 2, "nFileOffset 是 WORD");
        assert_eq!(size_of_val(&widths.n_file_extension), 2, "nFileExtension 是 WORD");
        assert_eq!(size_of_val(&widths.l_struct_size), 4, "lStructSize 是 DWORD");
        assert_eq!(size_of_val(&widths.flags_ex), 4, "FlagsEx 是 DWORD");
    }

    /// `nMaxFile` 与缓冲**同源于一个表达式**：两枚数字各写一份就是「改了容量忘了改声明」的现场。
    #[test]
    fn the_file_buffer_length_and_n_max_file_share_one_source() {
        let buffer = vec![0u16; FILE_BUFFER_UNITS];
        assert_eq!(buffer.len(), FILE_BUFFER_UNITS);
        assert_eq!(buffer.len() as u32, FILE_BUFFER_UNITS as u32, "nMaxFile 必须等于容量");
        assert!(FILE_BUFFER_UNITS > 260, "低于 MAX_PATH(260) 的容量会在普通路径上就报超长");
        // 调用前整块必须是 `\0`：头注要求「至少两字符的空串」，非零初值会被当成初始文件名。
        assert!(buffer.iter().all(|unit| *unit == 0), "lpstrFile 进函数前必须全零");
        assert_eq!(FILE_BUFFER_UNITS, 32_768, "取值理由写在常量doc上，改数先改理由");
    }

    /// **载荷组装源码锁**：`pick_file` 那一发的六颗字段必须真的接到该接的东西上。
    /// 形制先例 = `toast.rs:23-24` 那句「形制由两枚**源码锁**钉住」—— FFI 那一腿永不可执行，
    /// 能锁的只有文本。变异探针 M16（把 `lpstr_initial_dir` 换成裸 `null`）在加这条之前**全绿溜过**，
    /// 因为「起始目录到底有没有递进去」只有真弹一框才知道。
    #[test]
    fn the_payload_wiring_is_locked_by_source() {
        let source = product_source();
        for (label, needle) in [
            ("owner 必须原样透传入参（本模块不取不造窗口）", "hwnd_owner: owner_hwnd"),
            ("filter 必须指向 build_filter_wide 的产物", "lpstr_filter: filter.as_ptr()"),
            ("lpstrFile 必须是可写缓冲", "lpstr_file: buffer.as_mut_ptr()"),
            ("nMaxFile 必须与缓冲同源", "n_max_file: buffer.len() as u32"),
            ("起始目录必须真的递进去", "lpstr_initial_dir: initial_dir"),
            ("旗必须是那六枚", "flags: OFN_FLAGS"),
            ("成功/失败分诊必须走纯层", "outcome_from(chosen, extended, &buffer)"),
        ] {
            assert!(source.contains(needle), "{label}：找不到 {needle}");
        }
        // 反向：不许把初始目录写死成 null（那等于「起始位置」这一整格白做）。
        // （曾想顺手禁 `USERPROFILE` 这类拼接口径，但 `known_folder_units` 的 doc 里就得写着它来解释
        // 为什么不用拼接 —— 禁串会扫到自己的注释，这条片子里已经栽过两回了。）
        assert!(
            !source.contains("lpstr_initial_dir: std::ptr::null()"),
            "载荷里把 lpstrInitialDir 写死成 null = 起始位置那一格失效"
        );
    }

    /// 三态分诊的**整张表**（裁定 4 + 缓冲区纪律那两格合起来的可执行形态）。
    /// 这一格今天钉得住，全靠把分支从 `pick_file` 里抽成 [`outcome_from`] —— 变异探针 M14
    /// （把 `returned == 0` 翻成 `!= 0`）在抽取之前是**全绿溜过**的。
    #[test]
    fn the_return_and_error_pair_map_to_the_three_outcomes() {
        let mut picked = vec![0u16; 6];
        picked[0] = u16::from(b'D');
        picked[1] = 0;
        let full = vec![u16::from(b'x'); 6];
        let empty = vec![0u16; 6];

        // ① 回 0 + 扩展码 0 ⇒ 取消，**静默**（既不是 Failed 也不是 Picked）。
        assert_eq!(outcome_from(0, 0, &picked), PickOutcome::Cancelled);
        assert_eq!(outcome_from(0, 0, &full), PickOutcome::Cancelled, "取消档不许去看缓冲");
        // ② 回 0 + 扩展码非 0 ⇒ Failed，且串里带码带名字。
        match outcome_from(0, FNERR_BUFFERTOOSMALL, &picked) {
            PickOutcome::Failed(text) => assert!(text.contains("FNERR_BUFFERTOOSMALL"), "{text}"),
            other => panic!("回 0 且扩展码非 0 必须是 Failed，拿到 {other:?}"),
        }
        // ③ 回非 0 + 缓冲里有终止符 ⇒ Picked。
        assert_eq!(outcome_from(1, 0, &picked), PickOutcome::Picked("D".to_owned()));
        // ④ 回非 0 + 缓冲**无**终止符 ⇒ Failed 报超长，**绝不**返回一条截断路径。
        match outcome_from(1, 0, &full) {
            PickOutcome::Failed(text) => {
                assert!(text.contains("超出"), "超长那一档没如实报：{text}");
                assert!(text.contains('6'), "错误串里没带上真实容量：{text}");
            }
            other => panic!("无终止符必须是 Failed，拿到 {other:?}"),
        }
        // ⑤ 回非 0 + 首格就是终止符 ⇒ 空路径也如实报（不许当「用户选了个空串」成功回去）。
        assert!(matches!(outcome_from(1, 0, &empty), PickOutcome::Failed(_)));
        // 反向护栏：① 与 ③ 必须**不同态**（取消与成功合并 = 「选完什么都不干」的 bug 形状）。
        assert_ne!(outcome_from(0, 0, &picked), outcome_from(1, 0, &picked));
    }

    /// 超长档的「如实报」判据：缓冲里没有终止符 ⇒ `None`（调用方转 `Failed`，**绝不**返回截断路径）。
    #[test]
    fn an_overlong_path_is_reported_not_silently_truncated() {
        // 正常：终止符在第 3 格。
        let mut ok = vec![0u16; 8];
        ok[0] = u16::from(b'C');
        ok[1] = u16::from(b'X');
        ok[2] = 0;
        assert_eq!(take_picked_path(&ok).as_deref(), Some("CX"));
        // 终止符在第 0 格 = 空串（另一档，调用方另报）。
        let empty = vec![0u16; 8];
        assert_eq!(take_picked_path(&empty).as_deref(), Some(""));
        // 整块无终止符 = 路径把缓冲吃满了 ⇒ None。
        let full = vec![u16::from(b'x'); 8];
        assert_eq!(take_picked_path(&full), None, "无终止符必须报超长，不许返回截断串");
        // 非 UTF-16 合法序列不许 panic（lossy 是唯一允许的口径）。
        let mut lone = vec![0u16; 4];
        lone[0] = 0xD800;
        assert!(take_picked_path(&lone).is_some(), "孤立代理对不许 panic");
    }

    // ------------------------------------------------ 取消 / 错误 分诊

    /// 裁定 4 的可执行形态：`0` = 取消（静默），非 `0` = 错误（出串）。两档**不许合并**。
    #[test]
    fn cancel_and_error_are_two_distinct_branches() {
        assert_eq!(classify_failure(0), CancelOrError::Cancelled);
        assert_eq!(classify_failure(1), CancelOrError::Error(CDERR_STRUCTSIZE));
        assert_eq!(classify_failure(0x3003), CancelOrError::Error(FNERR_BUFFERTOOSMALL));
        // 取消那一档出去的 PickOutcome 必须是 Cancelled —— 不是 Failed("取消")，也不是 Picked("")。
        assert_eq!(failure_outcome(0), PickOutcome::Cancelled);
        let failed = failure_outcome(FNERR_BUFFERTOOSMALL);
        match &failed {
            PickOutcome::Failed(text) => {
                assert!(text.contains("0x3003"), "错误串里没带码 ⇒ 指不回原因：{text}");
                assert!(text.contains("FNERR_BUFFERTOOSMALL"), "错误串里没带名字：{text}");
            }
            other => panic!("超长必须是 Failed，拿到 {other:?}"),
        }
        // 未知码不许退化成取消（否则真失败被静默吞掉）。
        assert!(!matches!(failure_outcome(0xFFFF), PickOutcome::Cancelled));
        assert!(describe_extended_error(0x9999).contains("UNKNOWN"));
        // 已列名的码不许退化成 UNKNOWN。
        assert!(describe_extended_error(CDERR_INITIALIZATION).contains("CDERR_INITIALIZATION"));
        // 错误串的形状：码（十六进制四位）+ 名字两样都在，缺一就指不回原因。
        match failure_outcome(CDERR_INITIALIZATION) {
            PickOutcome::Failed(text) => {
                assert!(
                    text.starts_with("GetOpenFileNameW 失败，CommDlgExtendedError=0x0002 CDERR_INITIALIZATION"),
                    "错误串前缀漂了：{text}"
                );
            }
            other => panic!("非零码必须是 Failed，拿到 {other:?}"),
        }
        // 取消那一档不许长出任何一句用户可见的话（PickOutcome 里根本没有那个形状）。
        assert!(matches!(failure_outcome(0), PickOutcome::Cancelled));
    }

    // ------------------------------------------------ 旗的形状

    /// 六枚旗的值逐枚钉回 `commdlg.h` 现测值，并钉「三枚不该在的旗确实不在」。
    #[test]
    fn the_flag_set_is_the_derived_six_and_excludes_the_forbidden_three() {
        assert_eq!(OFN_EXPLORER, 0x0008_0000);
        assert_eq!(OFN_HIDEREADONLY, 0x0000_0004);
        assert_eq!(OFN_PATHMUSTEXIST, 0x0000_0800);
        assert_eq!(OFN_FILEMUSTEXIST, 0x0000_1000);
        assert_eq!(OFN_NOCHANGEDIR, 0x0000_0008);
        assert_eq!(OFN_ENABLESIZING, 0x0080_0000);
        assert_eq!(OFN_FLAGS, 0x0008_0004 | 0x0000_0800 | 0x0000_1000 | 0x0000_0008 | 0x0080_0000);
        // OFN_ENABLESIZING 头注要求同时有 OFN_EXPLORER。
        assert_ne!(OFN_FLAGS & OFN_EXPLORER, 0, "只给 ENABLESIZING 不给 EXPLORER 是无效组合");
        // 三枚禁项：多选（裁定不做）、覆盖提示（主干覆盖无提示）、模板/hook（全不用）。
        assert_eq!(OFN_FLAGS & 0x0000_0200, 0, "OFN_ALLOWMULTISELECT 不许出现");
        assert_eq!(OFN_FLAGS & 0x0000_0002, 0, "OFN_OVERWRITEPROMPT 不许出现");
        assert_eq!(OFN_FLAGS & (0x0000_0020 | 0x0000_0040 | 0x0000_0080), 0, "hook/template 一族不许出现");
    }

    /// 货架上「主干没有就不铺」那一判：除 filter/初始目录/旗之外，其余字段必须全零。
    #[test]
    fn nothing_on_the_shelf_is_pre_laid_beyond_the_mainline() {
        let idle = OpenFileNameW::default();
        assert_eq!(idle.l_struct_size, 0, "默认构造不许塞尺寸");
        assert!(idle.lpstr_custom_filter.is_null() && idle.n_max_custom_filter == 0, "主干无自定义过滤器档");
        assert!(idle.lpstr_file_title.is_null() && idle.n_max_file_title == 0, "主干只读 file.Path，不读文件名");
        assert!(idle.lpstr_title.is_null(), "主干 WinRT picker 没有标题 API");
        assert!(idle.lpstr_def_ext.is_null(), "主干不补默认扩展名");
        assert_eq!(idle.n_filter_index, 0, "主干没有初值档");
        assert_eq!(idle.flags_ex, 0, "OFX_* 主干无对应档");
        assert!(idle.lpfn_hook.is_null() && idle.lp_template_name.is_null() && idle.h_instance.is_null());
    }

    // ------------------------------------------------ GUID 交叉测试（唯一真调 Win32 的一族：只读、不弹窗）

    /// 图片档：`SHGetKnownFolderPath(FOLDERID_Pictures)` **必须等于** `SHGetFolderPathW(CSIDL_MYPICTURES)`。
    /// 断的是「两条真路径相等」，不是「两条都成功」—— 后者正是 `kernel.rs` 那次静默失效的漏网形状。
    #[test]
    fn pictures_initial_dir_matches_the_user_folder() {
        assert_same_path_via_both_apis(StartFolder::Pictures, CSIDL_MYPICTURES, "FOLDERID_Pictures");
    }

    /// 视频档：同法，`CSIDL_MYVIDEO`。
    #[test]
    fn videos_initial_dir_matches_the_user_folder() {
        assert_same_path_via_both_apis(StartFolder::Videos, CSIDL_MYVIDEO, "FOLDERID_VIDEOS");
    }

    /// 文档档：同法，`CSIDL_PERSONAL`。这颗 GUID 本仓已有一次抄错的血案（`kernel.rs:3336-3345`），
    /// 货架上另立一份 const 就必须另立一把锁，不许搭 `kernel.rs` 那把测试的便车（那颗 const 是私有的）。
    #[test]
    fn documents_initial_dir_matches_the_user_folder() {
        assert_same_path_via_both_apis(StartFolder::Documents, CSIDL_PERSONAL, "FOLDERID_DOCUMENTS");
    }

    /// 下载档：SDK 里**没有 `CSIDL_DOWNLOADS` 这一颗**（现测：
    /// `grep -rn "CSIDL_DOWNLOADS" "…/Include/10.0.22621.0/"` ⇒ 0 命中，`0x003d` 是
    /// `CSIDL_COMPUTERSNEARME`），所以交叉口径换成**注册表里那颗 GUID 名字串**
    /// （`HKCU\…\User Shell Folders\{374DE290-…}`）—— 它是 Explorer 自己的存储，
    /// 与本模块 Rust 侧那 16 个字节是**两次独立转写**，抄错一边就红。
    #[test]
    fn downloads_initial_dir_matches_the_registry_oracle() {
        let ours = known_folder_path(StartFolder::Downloads);
        let oracle = registry_user_shell_folder("{374DE290-123F-4565-9164-39C4925E467B}");
        if let Some(text) = oracle.as_deref() {
            // 反锁：口径自己得是**展干净**的绝对目录。留着 `%VARS%` 就当相等通过 ⇒ 这把锁是假的
            // （`RegGetValueW` 的自动展开行为见 [`registry_user_shell_folder`] 的旗标注释）。
            assert!(!text.contains('%'), "注册表口径没被展开：{text}");
            assert!(std::path::Path::new(text).is_absolute(), "注册表口径不是绝对路径：{text}");
        }
        match (ours.as_deref(), oracle.as_deref()) {
            (Some(path), Some(expected)) => assert_same_real_dir(path, expected, "FOLDERID_DOWNLOADS"),
            (Some(path), None) => {
                println!("NP1 环境诊断：注册表那颗 User Shell Folders 值读不到 ⇒ 只能单边判（本机不走这条）");
                assert!(is_real_dir(path), "{path} 必须是已存在的目录");
            }
            (None, Some(expected)) => panic!(
                "FOLDERID_DOWNLOADS 解析失败而注册表口径给了 {expected} ⇒ 十有八九是那颗 GUID 的字节抄错了 \
                 （错的 GUID 也回 0x80070002，看不出是 GUID 不对）"
            ),
            (None, None) => panic!("两条口径同败 = 本机 known folder 表不可用，这不算通过"),
        }
    }

    /// 每档绑到自己那颗 GUID，且**四颗 GUID 互不相同**（两档撞同一颗 = 起始目录抄串了）。
    #[test]
    fn each_start_folder_binds_to_its_own_guid() {
        let all = [
            StartFolder::Pictures,
            StartFolder::Videos,
            StartFolder::Documents,
            StartFolder::Downloads,
        ];
        let ids: Vec<KnownFolderId> = all.iter().copied().map(known_folder_id).collect();
        for index in 0..ids.len() {
            for other in 0..ids.len() {
                if index != other {
                    assert_ne!(ids[index], ids[other], "两档撞了同一颗 GUID");
                }
            }
        }
        // 逐颗字节 = knownfolders.h 现测值（这里当字面量钉，改 const 的人必须同时改这里 = 必须重新现测）。
        assert_eq!(
            ids[0],
            KnownFolderId { data1: 0x33E2_8130, data2: 0x4E1E, data3: 0x4676, data4: [0x83, 0x5A, 0x98, 0x39, 0x5C, 0x3B, 0xC3, 0xBB] }
        );
        assert_eq!(
            ids[1],
            KnownFolderId { data1: 0x1898_9B1D, data2: 0x99B5, data3: 0x455B, data4: [0x84, 0x1C, 0xAB, 0x7C, 0x74, 0xE4, 0xDD, 0xFC] }
        );
        assert_eq!(
            ids[2],
            KnownFolderId { data1: 0xFDD3_9AD0, data2: 0x238F, data3: 0x46AF, data4: [0xAD, 0xB4, 0x6C, 0x85, 0x48, 0x03, 0x69, 0xC7] }
        );
        assert_eq!(
            ids[3],
            KnownFolderId { data1: 0x374D_E290, data2: 0x123F, data3: 0x4565, data4: [0x91, 0x64, 0x39, 0xC4, 0x92, 0x5E, 0x46, 0x7B] }
        );
    }

    // -------------------------------------------------- 测试侧工具（真调 Win32，只读）

    const CSIDL_MYPICTURES: i32 = 0x0027; // ShlObj_core.h:845
    const CSIDL_MYVIDEO: i32 = 0x000E; // ShlObj_core.h:815
    const CSIDL_PERSONAL: i32 = 0x0005; // ShlObj_core.h:806

    // 第二条口径：老 CSIDL 那一族，它**不吃** `FOLDERID_*` 的字节 ⇒ 能当 GUID 的交叉验证。
    // （同 kernel.rs:3795 的手法与注释。）
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHGetFolderPathW(
            owner: *mut c_void,
            csidl: i32,
            token: *mut c_void,
            flags: u32,
            path: *mut u16,
        ) -> i32;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(
            key: *const c_void,
            subkey: *const u16,
            value: *const u16,
            flags: u32,
            kind: *mut u32,
            data: *mut u8,
            size: *mut u32,
        ) -> i32;
    }

    /// 老 CSIDL 口径的宽路径；失败 ⇒ `None`。`SHGFP_TYPE_CURRENT = 0`。
    fn csidl_path(csidl: i32) -> Option<String> {
        let mut buffer = [0u16; 260];
        let code = unsafe {
            SHGetFolderPathW(std::ptr::null_mut(), csidl, std::ptr::null_mut(), 0, buffer.as_mut_ptr())
        };
        if code != 0 {
            return None;
        }
        let units: Vec<u16> = buffer.iter().copied().take_while(|unit| *unit != 0).collect();
        let text = String::from_utf16_lossy(&units);
        (!text.is_empty()).then_some(text)
    }

    /// 注册表口径：`HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders`
    /// 下那颗名字就是 GUID 的 `REG_EXPAND_SZ`。
    ///
    /// 旗标取 `winreg.h:65-66` 现值的 `RRF_RT_REG_SZ(0x2) | RRF_RT_REG_EXPAND_SZ(0x4)` —— 头注原话：
    /// 不给 `RRF_NOEXPAND(0x1000_0000)` 时 **`RegGetValue` 自己把 `%VARS%` 展好**（`0x4` 单用反而
    /// 按头注要求必须配 `RRF_NOEXPAND`，否则 `ERROR_INVALID_PARAMETER`）。所以这里既不用自己拼
    /// `ExpandEnvironmentStringsW`、也不吃「没展干净」的隐性 bug；展没展干净交给下面那条 `%` 反锁。
    fn registry_user_shell_folder(value_name: &str) -> Option<String> {
        const HKEY_CURRENT_USER: usize = 0x8000_0001;
        const RRF_RT_REG_SZ_OR_EXPAND_SZ: u32 = 0x0000_0002 | 0x0000_0004;
        let subkey = wide_units("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\User Shell Folders");
        let value = wide_units(value_name);
        let mut size = 0u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER as *const c_void,
                subkey.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ_OR_EXPAND_SZ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut size,
            )
        };
        if status != 0 || size == 0 {
            return None;
        }
        let mut raw = vec![0u8; size as usize];
        let mut filled = size;
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER as *const c_void,
                subkey.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ_OR_EXPAND_SZ,
                std::ptr::null_mut(),
                raw.as_mut_ptr().cast(),
                &mut filled,
            )
        };
        if status != 0 {
            return None;
        }
        let units: Vec<u16> = raw[..filled as usize]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .take_while(|unit| *unit != 0)
            .collect();
        let text = String::from_utf16_lossy(&units);
        (!text.is_empty()).then_some(text)
    }

    fn is_real_dir(path: &str) -> bool {
        let candidate = std::path::Path::new(path);
        candidate.is_absolute() && candidate.is_dir()
    }

    /// 两条口径的**相等**判据（外加「确实是已存在的绝对目录」），这是本片唯一认的形状。
    fn assert_same_real_dir(ours: &str, oracle: &str, label: &str) {
        assert_eq!(
            ours, oracle,
            "{label} 的两条口径不相等 ⇒ 多半是那颗 GUID 的字节抄错了（错的 GUID 也回 0x80070002，\
             只断「都成功」抓不到）：ours={ours} oracle={oracle}"
        );
        assert!(is_real_dir(ours), "{ours} 必须是已存在的绝对目录");
        assert!(
            !ours.to_lowercase().contains("system32"),
            "{label} 指到 system32 就是 kernel.rs 那条「工具全废」的反例"
        );
        // 过了也留一行：报告要拿这行当「走强档且两条真路径相等」的可复跑证据（`-- --nocapture`）。
        println!("NP1 交叉比对通过：{label} 两条口径同为 {ours}");
    }

    /// 交叉测试的公共骨架。**不许**退化成「两边都 `is_some()`」。
    fn assert_same_path_via_both_apis(folder: StartFolder, csidl: i32, label: &str) {
        let ours = known_folder_path(folder);
        let oracle = csidl_path(csidl);
        match (ours.as_deref(), oracle.as_deref()) {
            (Some(ours), Some(oracle)) => assert_same_real_dir(ours, oracle, label),
            (None, Some(oracle)) => panic!(
                "{label} 解析失败而 CSIDL 口径给了 {oracle} ⇒ 十有八九是那颗 GUID 的字节抄错了 \
                 （`SHGetKnownFolderPath` 对错的 GUID 也回 0x80070002，函数照样「失败得很有道理」）"
            ),
            (Some(ours), None) => {
                println!("NP1 环境诊断：CSIDL 口径读不到 ⇒ 单边降强度（本机不走这条）");
                assert!(is_real_dir(ours), "{ours} 必须是已存在的目录");
            }
            (None, None) => panic!("两条口径同败 = 本机 known folder 表不可用，这不算通过"),
        }
    }
}
