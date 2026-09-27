//! #76「工作区文件右栏（Files 面板）」那六发 `workspaceFiles/*` 的**纯逻辑层**。
//!
//! 这里只有回执解析、条目/树模型、预览截断判据与 scope-id 态机 —— 一个控件都不碰、
//! 一颗 RPC 都不发、一颗 ctor 都不建。规格来源是主干 `Pages/FilesPanel.xaml.cs` +
//! `Pages/FilesPanel.Preview.cs` 这一组 partial（**逐条现测**于本批，见
//! `rust/tmp/fp1-report.md` §0.1：六发的调用点 7/7 全在这两颗文件里，`MainWindow.xaml.cs`
//! 只在注释里提过这一族，所以这条边界是全仓唯一干净的）。
//! 刻意**不写主干行号**：本仓实测行号必漂，只留符号名（`ListDirectoryAsync` 这类 grep 得到）。
//!
//! ## 本模块管什么、不管什么
//!
//! | 在 | 不在（谁拿走） |
//! |---|---|
//! | `list`/`stat`/`read` 回执解析、条目模型与排序 | 六发的 args ctor ⇒ `kernel.rs`（KW3） |
//! | 预览截断判据（按行分页 + 字节分段两条口径） | 六发派发与回写 ⇒ `main.rs`（MR2） |
//! | `readBytes`/`readAll`/`readRelated` 共用的 base64 校验 | base64 → 位图/PDF/HTML 的出图 ⇒ 另立一族 |
//! | `workspaceFileScopeId` 与三枚 generation 的态机 | `workspaceFiles/changes` **流协议**（第七发）⇒ 流侧 |
//!
//! ## 一条贯穿全模块的纪律：判据的**方向**照主干，不加仁慈
//!
//! 主干读回执用的是 `System.Text.Json` 的 `JsonElement`，它对「类型不对」的处理是**抛**，
//! 不是给默认值。现测（.NET 10 实跑，`System.Text.Json` 与主干 net8.0 同一实现族）：
//!
//! - `GetString()`：`String` 给串、`Null` 给 `null`，**`Number`/`True`/`Object`/`Array` 一律抛
//!   `InvalidOperationException`**。⇒ 主干 `t.GetString() ?? "other"` 那行只在**键不存在**时才落
//!   `"other"`；`"type": 1` 这种回帧是**整次列目录失败**，不是这一行退化成 `other`。
//! - `GetInt64()`：`1.5` / `1e2` / 超 i64 范围**抛**；字符串 `"3"` **抛**。
//! - `TryGetProperty`：元素不是对象（含 `Null`）**抛**。
//!
//! ⇒ 本模块所有解析口都返回 [`Result`]，且**失败半径**逐处对齐主干：主干把整段 `try` 包在
//! `ListDirectoryAsync` / `ShowPreviewAsync` 外面 ⇒ 一处类型不对就作废**整页**，绝不悄悄丢条目。
//! 反方向也一样：主干**缺键**时的回落（`truncated` 缺 ⇒ 未截断、`version` 缺 ⇒ 保持旧值）
//! 也不许升级成报错。两类方向混起来就是本仓定义的缺陷类。
//!
//! ## 文案：一律走 `crate::i18n::Catalog`，本模块不写第二份表
//!
//! 主干 Files 面板用的是 `MainWindow.TL(中文)` / `TLF(中文, args)` 这一族**单语键**
//! （不是 `L(中, 英)` 那种内联双语），所以分叉的对应物是 [`Catalog::l`] + [`Catalog::lf`]
//! （先例：`crate::contextmeter` 的 `tooltip_and_aria`），**不是** `bt`/`btf`。
//! 本模块只**引用** zh 键字面串；英文在 `i18n::EN` 里（IN3 已铺好这一族，逐条见报告 §3）。

use std::cmp::Ordering;

use serde_json::Value;

use crate::i18n::Catalog;

// ---------------------------------------------------------------- 主干钉死的常量

/// `workspaceFiles/list` 条目 `type` 的三个取值（内核 typert 现测是
/// `union([literal("file"), literal("directory"), literal("other")])`）。
/// 主干 `FileNode.Type` 存的是**字符串**，判目录只跟 `"directory"` 比 ⇒ 分叉照抄，
/// 不折成 enum（折了就比内核的字面集合更严，见 [`FileEntry::is_directory`]）。
pub const KIND_FILE: &str = "file";
pub const KIND_DIRECTORY: &str = "directory";
pub const KIND_OTHER: &str = "other";

/// 文本预览的单页行数（主干 `FilesPanel.xaml.cs` 的 `private const int PreviewLineLimit = 2000`）。
/// 这一枚同时是「文件较长」那句里的 `{1}` ⇒ 它是**判据**，不只是文案参数。
pub const PREVIEW_LINE_LIMIT: i64 = 2000;

/// 字节分段的步长（主干 `Preview.cs` 的 `ByteChunkLength = 256 * 1024`）。
pub const BYTE_CHUNK_LENGTH: i64 = 256 * 1024;

/// HTML 附属资源最多读几项（主干 `MaxHtmlRelated = 12`，`refs.Take(MaxHtmlRelated)`）。
pub const MAX_HTML_RELATED: usize = 12;

/// 主干 `ShowPreviewPlaceholder` / `ShowError` 两张表里出现的那句内核上限口径：
/// 「2 MiB / 5000 行」。分叉**不拿它当判据**（判据是 `PREVIEW_LINE_LIMIT` + `eof`），
/// 它只活在文案里；记在这儿是为了别让人把 5000 误接成分页上限。
pub const KERNEL_LINE_CAP_MENTIONED_IN_TEXT: i64 = 5000;

// ---------------------------------------------------------------- 回执解析的失败半径

/// 一处类型不对 = 主干那一次 `catch` 的范围 = 本模块的 `Err`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// 主干 `GetProperty("entries")` 缺键即抛。注意**只有**这一族是「缺键就抛」：
    /// 主干其余字段都走 `TryGetProperty`，缺键是回落。
    MissingField(&'static str),
    /// 主干在对象外调 `GetProperty`/`TryGetProperty` ⇒ `InvalidOperationException`。
    NotObject(&'static str),
    /// `GetString()` 对 `Number`/`True`/`Object`/`Array` 抛（`Null` 不抛，给 `null`）。
    NotString(&'static str),
    /// `GetInt64()` 对小数、超范围数、字符串抛。
    NotInt64(&'static str),
}

/// 主干那三类「取一个字段」的写法各自回落方向不同，下面三颗小函数一一对上：
/// `get_str` 的缺键与 JSON `null` 合流成 `None`（主干 `?? fallback`），但**非字符串是抛**，
/// 这条不合流的差异在 `stat`（缺键写 null）与 `read`（缺键保持旧值）两处才见分晓，
/// 见 [`PreviewState::apply_stat`] 与 [`TextPager::apply_page`]。

/// 「取字符串字段」= 主干 `TryGetProperty` + `GetString()`：缺键 / JSON null ⇒ `None`，
/// 非字符串 ⇒ [`ParseError::NotString`]（**抛**，不是回落）。
fn get_str(obj: &Value, key: &'static str) -> Result<Option<String>, ParseError> {
    match obj.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(ParseError::NotString(key)),
    }
}

/// 「取可空整数」= 主干 `TryGetProperty(k, out v) && v.ValueKind == Number ? v.GetInt64() : null`：
/// 非数字（串/布尔/对象）**静默给 None**，是数字但不是整数（`1.5`、`1e2`、超范围）**抛**。
fn get_i64(obj: &Value, key: &'static str) -> Result<Option<i64>, ParseError> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(_)) | Some(Value::String(_)) | Some(Value::Object(_)) | Some(Value::Array(_)) => {
            Ok(None)
        }
        Some(number) => number.as_i64().map(Some).ok_or(ParseError::NotInt64(key)),
    }
}

/// 「取布尔真值」= 主干 `TryGetProperty(k, out v) && v.ValueKind == True`：
/// 缺键 ⇒ false，写成 `1` / `"true"` ⇒ 也是 false（主干只认 `JsonValueKind.True`）。
fn is_true(obj: &Value, key: &'static str) -> bool {
    matches!(obj.get(key), Some(Value::Bool(true)))
}

fn require_object<'a>(value: &'a Value, what: &'static str) -> Result<&'a Value, ParseError> {
    if value.is_object() {
        Ok(value)
    } else {
        Err(ParseError::NotObject(what))
    }
}

// ---------------------------------------------------------------- 条目 / 树模型

/// 主干 `FileNode`（`TreeViewNode.Content`）的数据半颗：`Name`/`Path`/`Type`/`Size`/`IsPlaceholder`。
/// `Glyph`、`RowOpacity`、`ToString()` 那三样是画与无障碍，留 `main.rs`。
#[derive(Debug, Clone, PartialEq)]
pub struct FileEntry {
    pub name: String,
    /// 绝对路径 `"工作区根/子/孙"`，`/` 分隔（主干 `JoinPath` 造的形态）。
    pub path: String,
    /// 原样存内核给的 `type` 字面串；缺键 / JSON null ⇒ [`KIND_OTHER`]（主干 `?? "other"`）。
    pub kind: String,
    pub size: Option<i64>,
    pub is_placeholder: bool,
}

impl FileEntry {
    /// 主干 `IsDirectory => Type == "directory"`：只比这一颗，`"other"` 与未知串都算非目录。
    #[must_use]
    pub fn is_directory(&self) -> bool {
        self.kind == KIND_DIRECTORY
    }

    /// 占位行（「（空目录）」）：主干 `FillNode` 在 `entries.Count == 0` 时插的那一颗。
    /// 它 `Path` 为空串、`Type` 走默认 `"file"` ⇒ `IsDirectory` 为假，所以
    /// [`collect_expanded`] 与文件动作菜单都会把它筛掉。
    #[must_use]
    pub fn placeholder(name: String) -> Self {
        Self {
            name,
            path: String::new(),
            kind: "file".to_string(),
            size: None,
            is_placeholder: true,
        }
    }
}

/// `workspaceFiles/list` 的一次目录回执（解析后）。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DirectoryListing {
    pub entries: Vec<FileEntry>,
    /// 主干 `truncated`：缺键 ⇒ false；只有 JSON `true` 才算截断。
    pub truncated: bool,
}

/// 「主干不读」的一记：`list` 回执里的 `path` 键内核是**必有**的（typert 现测 required），
/// 但主干 `ListDirectoryAsync` 一次都没读过它 ⇒ 分叉也别为它建第二真相，缺了不报错。
pub const LIST_REPLY_PATH_IS_NOT_CONSUMED: &str = "path";

/// `workspaceFiles/list` 回执 → 排好序的条目。
///
/// 主干 `ListDirectoryAsync` 的完整口径：`GetProperty("entries")`（缺键即抛）→ 逐元素
/// `TryGetProperty`（元素非对象即抛）→ `name` 缺键/非串/空串 ⇒ **整条丢掉**（`continue`）→
/// `truncated` 为真才弹那条横幅 → 末尾 `[.. OrderByDescending(IsDirectory).ThenBy(Name, OrdinalIgnoreCase)]`。
/// `path` 由**入参**拼（`JoinPath(path, name)`），回执里那颗不算。
///
/// 返回的 `Vec` 已排序，等价于主干那一行；`truncated` 只如实带回，弹不弹横幅是宿主的事。
pub fn parse_list_reply(value: &Value, parent_path: &str) -> Result<DirectoryListing, ParseError> {
    let root = require_object(value, "list 回执")?;
    let raw_entries = root
        .get("entries")
        .ok_or(ParseError::MissingField("entries"))?;
    let Value::Array(items) = raw_entries else {
        // 主干 `EnumerateArray()` 对非数组抛 InvalidOperationException
        return Err(ParseError::NotObject("entries"));
    };
    let mut entries = Vec::with_capacity(items.len());
    for item in items {
        let entry = require_object(item, "entries 元素")?;
        let name = get_str(entry, "name")?.unwrap_or_default();
        if name.is_empty() {
            continue; // 主干：空名直接丢，不给它建行
        }
        entries.push(FileEntry {
            path: join_path(parent_path, &name),
            name,
            // 主干 `TryGetProperty("type") ? GetString() ?? "other" : "other"`：
            // 缺键 ⇒ other；JSON null ⇒ other；**非串 ⇒ 抛**（GetString 的口径）
            kind: match entry.get("type") {
                None | Some(Value::Null) => KIND_OTHER.to_string(),
                Some(Value::String(text)) => text.clone(),
                Some(_) => return Err(ParseError::NotString("type")),
            },
            size: get_i64(entry, "size")?,
            is_placeholder: false,
        });
    }
    sort_entries(&mut entries);
    Ok(DirectoryListing {
        entries,
        truncated: is_true(root, "truncated"),
    })
}

/// 主干那一行排序：目录在前（`OrderByDescending(IsDirectory)`），同档按名字
/// `StringComparer.OrdinalIgnoreCase` 升序。LINQ 的 `OrderBy` 是**稳定**排序 ⇒ 这里也必须稳。
pub fn sort_entries(entries: &mut [FileEntry]) {
    entries.sort_by(|a, b| {
        b.is_directory()
            .cmp(&a.is_directory())
            .then_with(|| ordinal_ignore_case_cmp(&a.name, &b.name))
    });
}

/// 主干 `FillNode` 的数据半颗：空目录 ⇒ 唯一一颗占位行，否则原样给出条目。
#[must_use]
pub fn children_of(listing: &DirectoryListing, empty_dir_name: &str) -> Vec<FileEntry> {
    if listing.entries.is_empty() {
        vec![FileEntry::placeholder(empty_dir_name.to_string())]
    } else {
        listing.entries.clone()
    }
}

/// 主干 `CollectExpanded`：只有「是目录 + 非占位 + 已展开」的行才进恢复列表，
/// 且**先登记自己再递归**（⇒ 列表天然带浅到深的序，`RefreshAsync` 靠这个 `Skip(1)` 掉根）。
#[must_use]
pub fn collect_expanded(is_directory: bool, is_placeholder: bool, is_expanded: bool) -> bool {
    is_directory && !is_placeholder && is_expanded
}

// ---------------------------------------------------------------- 大小写折叠（主干 OrdinalIgnoreCase）

/// .NET `StringComparer.OrdinalIgnoreCase` 走**简单**一字对一字折叠（`ß` 仍是 `ß`），
/// Rust 的 `to_lowercase` 是完整折叠（`ß` → `ss`，长度与序都会跟着变）⇒ 逐字折叠，
/// 只在折叠结果仍是单个 char 时采纳。与 `main.rs` 里那枚同族的私有实现同式（报告 §3 记了这笔重复）。
fn fold(ch: char) -> char {
    let mut lowered = ch.to_lowercase();
    match lowered.next() {
        Some(one) if lowered.next().is_none() => one,
        _ => ch,
    }
}

#[must_use]
pub fn ordinal_ignore_case_eq(text: &str, other: &str) -> bool {
    text.chars().map(fold).eq(other.chars().map(fold))
}

#[must_use]
pub fn ordinal_ignore_case_cmp(text: &str, other: &str) -> Ordering {
    text.chars().map(fold).cmp(other.chars().map(fold))
}

// ---------------------------------------------------------------- 路径与显示串

/// 主干 `NormalizeRoot`：空白串 ⇒ `None`；`\` → `/`；去**所有**尾斜杠；结果空 ⇒ `None`。
#[must_use]
pub fn normalize_root(raw: Option<&str>) -> Option<String> {
    let text = raw?;
    if text.trim().is_empty() {
        return None;
    }
    let normalized = text.replace('\\', "/").trim_end_matches('/').to_string();
    (!normalized.is_empty()).then_some(normalized)
}

/// 主干 `JoinPath`：`$"{parent.TrimEnd('/')}/{name}"`。
#[must_use]
pub fn join_path(parent: &str, name: &str) -> String {
    format!("{}/{}", parent.trim_end_matches('/'), name)
}

/// 主干 `DisplayNameOf`：去尾斜杠 → 取最后一段 → 最后一段为空则**回落成整串**。
#[must_use]
pub fn display_name_of(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    let name = match trimmed.rfind('/') {
        Some(index) => &trimmed[index + 1..],
        None => trimmed,
    };
    if name.is_empty() {
        trimmed.to_string()
    } else {
        name.to_string()
    }
}

/// 主干 `Short(version)`：`null` / 空串 ⇒ `"-"`，否则取前 8 个 **UTF-16 码元**。
/// 按码元切是照 `version[..Math.Min(8, version.Length)]`（C# 的 `Length` 就是码元数）；
/// 内核的 `version` 是内容哈希（纯 ASCII）⇒ 这条差异今天打不到，留着是为了口径不漂。
#[must_use]
pub fn short_version(version: Option<&str>) -> String {
    match version {
        None | Some("") => "-".to_string(),
        Some(text) => {
            let units: Vec<u16> = text.encode_utf16().take(8).collect();
            String::from_utf16_lossy(&units)
        }
    }
}

/// 主干 `FormatSize`：`null` ⇒ 键「大小未知」；`< 1024` ⇒ `"{b} B"`；
/// `< 1 MiB` ⇒ `"{b/1024:0.#} KiB"`（**一位**小数、零尾去掉）；其余 ⇒ `"{b/1MiB:0.##} MiB"`（两位）。
///
/// 两处照主干的坑：① .NET 的 `"0.#"` 用 **CurrentCulture** 的小数点，壳跑什么区域就是什么，
/// 分叉固定成不变文化（`.`）——这是唯一一处**已知偏离**，理由是分叉拿不到 UI 线程的 CurrentCulture，
/// 报告 §3 有记；② .NET Core 3.0 起数值格式化走**中点远离零**舍入，不是银行家舍入。
#[must_use]
pub fn format_size(bytes: Option<i64>, catalog: &Catalog) -> String {
    let Some(value) = bytes else {
        return catalog.l("大小未知");
    };
    if value < 1024 {
        return format!("{value} B");
    }
    if value < 1024 * 1024 {
        return format!("{} KiB", format_decimal(value as f64 / 1024.0, 1));
    }
    format!("{} MiB", format_decimal(value as f64 / (1024.0 * 1024.0), 2))
}

/// C# `"0.##"` 族：四舍五入到 `digits` 位（中点在零之外），再把尾随 0 与悬空小数点去掉。
fn format_decimal(value: f64, digits: usize) -> String {
    let factor = 10f64.powi(digits as i32);
    let scaled = if value >= 0.0 {
        (value * factor + 0.5).floor()
    } else {
        (value * factor - 0.5).ceil()
    };
    let rounded = scaled / factor;
    let mut text = format!("{rounded:.digits$}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    text
}

// ---------------------------------------------------------------- `System.IO.Path.GetExtension`

/// 主干 `FaceOf` / `IsHashCommentFile` 都吃 `System.IO.Path.GetExtension(...).ToLowerInvariant()`，
/// 而 .NET 那位的口径**不是**「文件名的扩展名」：它从**整串末尾**往回扫，撞到 `/` 或 `\` 才停，
/// 找到的第一颗 `.` 只要不是最后一格就把它到结尾整段当扩展名。
/// ⇒ `GetExtension(".gitignore") == ".gitignore"`（这正是主干 Code 表里为什么有 `.gitignore`/
/// `.editorconfig` 这类「点开头」的项）、`GetExtension("archive.") == ""`。
#[must_use]
pub fn get_extension(path: &str) -> String {
    let units: Vec<(usize, char)> = path.char_indices().collect();
    let last = units.len().saturating_sub(1);
    for (position, (byte_index, ch)) in units.iter().enumerate().rev() {
        match ch {
            '.' => {
                return if position == last {
                    String::new() // 末尾那颗点：主干给空串
                } else {
                    path[*byte_index..].to_string()
                };
            }
            '/' | '\\' => break,
            _ => {}
        }
    }
    String::new()
}

// ---------------------------------------------------------------- 六形态判定

/// 主干 `PreviewFace` 六档。档位决定**发哪一发读**：图/PDF 走 `readBytes`、HTML 走 `read` +
/// `readRelated`、文本三档走 `read`；`readAll` 只在「复制全文而还没读到 eof」时才发。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewFace {
    PlainText,
    Code,
    Markdown,
    Image,
    Pdf,
    Html,
}

/// 主干 `FaceOf`：按 [`get_extension`] 的小写查那张表，查不到 ⇒ `PlainText`。
/// 表里的 `.cs` 主干写了两遍（switch 臂重复，C# 允许），这里去重不影响判定。
#[must_use]
pub fn face_of(path: &str) -> PreviewFace {
    match get_extension(path).to_lowercase().as_str() {
        ".md" | ".markdown" => PreviewFace::Markdown,
        ".png" | ".jpg" | ".jpeg" | ".gif" | ".webp" | ".bmp" | ".ico" | ".svg" => PreviewFace::Image,
        ".pdf" => PreviewFace::Pdf,
        ".html" | ".htm" => PreviewFace::Html,
        ".cs" | ".js" | ".mjs" | ".cjs" | ".ts" | ".tsx" | ".jsx" | ".py" | ".rs" | ".go" | ".java"
        | ".c" | ".h" | ".cpp" | ".hpp" | ".xaml" | ".json" | ".yml" | ".yaml" | ".toml" | ".xml"
        | ".css" | ".scss" | ".less" | ".sql" | ".sh" | ".ps1" | ".bat" | ".cmd" | ".php" | ".rb"
        | ".swift" | ".kt" | ".scala" | ".vue" | ".svelte" | ".dart" | ".r" | ".gradle"
        | ".dockerfile" | ".ini" | ".cfg" | ".conf" | ".lock" | ".gitignore" | ".editorconfig" => {
            PreviewFace::Code
        }
        _ => PreviewFace::PlainText,
    }
}

/// 主干 `IsTextFace`：四档（`Html` 也算文本档 —— 它是「源码 + DOM」双态，`CopyAll` 要给它开）。
/// 注意主干还有一处更窄的判据（`WrapToggle` / `CopySelection`）用
/// `face is PlainText or Code || htmlSource` ⇒ **Markdown 与 Html 都拿不到**「复制选中」，
/// 那是画的问题，留宿主；这里只留 `IsTextFace` 这一颗。
#[must_use]
pub fn is_text_face(face: PreviewFace) -> bool {
    matches!(
        face,
        PreviewFace::PlainText | PreviewFace::Code | PreviewFace::Markdown | PreviewFace::Html
    )
}

/// 主干 `IsHashCommentFile`：只有这八种扩展名把 `#` 当注释。
#[must_use]
pub fn is_hash_comment_file(path: Option<&str>) -> bool {
    let ext = match path {
        None => return false,
        Some(text) => get_extension(text).to_lowercase(),
    };
    matches!(
        ext.as_str(),
        ".py" | ".sh" | ".yml" | ".yaml" | ".toml" | ".rb" | ".ps1" | ".r"
    )
}

// ---------------------------------------------------------------- stat（两处调用）

/// `workspaceFiles/stat` 回执（内核 typert：`{absolutePath, version, bytes?}`，
/// **没有** `offset`/`eof`）。主干两处调用**只读** `version` 与 `bytes` 两颗，
/// `absolutePath` 一次都没从 `stat` 读过（它只从 `read`/`readAll`/`readBytes` 读）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileStat {
    /// `None` = 缺键（主干那行给 `null`）；键在但非串 ⇒ 解析已经抛出去了。
    pub version: Option<String>,
    pub bytes: Option<i64>,
}

/// 解析 `stat` 回执。两处调用的**差集**不在这里，在 [`PreviewState::apply_stat`] 与
/// [`PreviewState::changed_banner_from`] —— 它们的 args 形状逐字相同（`{workspaceFileScopeId, path}`），
/// 差的是 path 的来源与**读哪几颗键**（报告 §1.3）。
pub fn parse_stat_reply(value: &Value) -> Result<FileStat, ParseError> {
    let root = require_object(value, "stat 回执")?;
    Ok(FileStat {
        version: get_str(root, "version")?,
        bytes: get_i64(root, "bytes")?,
    })
}

// ---------------------------------------------------------------- read（文本分页 = 截断判据）

/// `workspaceFiles/read` 回执里主干**真读**的四加三颗：
/// `text` / `lines` / `eof` / `version` / `absolutePath` /（`bytes` 只有 `stat` 给的那份在用）。
/// 回执还有 `offset`（内核必发）—— 主干不读，分叉也不建第二真相。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReadPage {
    pub text: String,
    pub lines: i64,
    pub eof: bool,
    pub version: Option<String>,
    pub absolute_path: Option<String>,
}

pub fn parse_read_reply(value: &Value) -> Result<ReadPage, ParseError> {
    let root = require_object(value, "read 回执")?;
    Ok(ReadPage {
        // 主干 `TryGetProperty("text") ? GetString() ?? "" : ""` ⇒ 缺键/null 都是空串
        text: get_str(root, "text")?.unwrap_or_default(),
        // `lines` 被 ValueKind==Number 夹过 ⇒ 缺键给 0（不是「不推进」以外的任何事）
        lines: get_i64(root, "lines")?.unwrap_or(0),
        eof: is_true(root, "eof"),
        // 关键区别：version / absolutePath 的 `:` 分支回落的是**旧值**，不是 null
        version: get_str(root, "version")?,
        absolute_path: get_str(root, "absolutePath")?,
    })
}

/// 主干 `Preview.cs` 的三个游标字段：`_previewBuffer` / `_previewNextLine` / `_previewEof`。
/// 文本分页的**全部**截断判据都在这颗上。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextPager {
    /// 主干 `_previewNextLine`：**1 基**行号，下一页 `range.offset` 就是它。
    pub next_line: i64,
    pub eof: bool,
    pub buffer: String,
}

impl Default for TextPager {
    fn default() -> Self {
        Self {
            next_line: 1,
            eof: false,
            buffer: String::new(),
        }
    }
}

impl TextPager {
    /// 主干 `ResetPreviewBody` 里属于本层的三件套。
    pub fn reset(&mut self) {
        self.buffer = String::new();
        self.next_line = 1;
        self.eof = false;
    }

    /// 吃完一页后的状态迁移，返回主干用来画行号的那颗 `pageStartLine`。
    ///
    /// 逐条照 `LoadTextPageAsync`：先落 `eof`，再拼缓冲（三分支），
    /// **然后**才记 `pageStartLine`，**最后**`if (lines > 0) next_line += lines`。
    /// `lines == 0` 时游标**不动** ⇒ 主干靠 `LoadMoreButton` 的禁用位挡死原地打转，
    /// 这一条是分叉不许「顺手修」的地方（报告 §3 有记）。
    pub fn apply_page(&mut self, page: &ReadPage, append: bool) -> i64 {
        self.eof = page.eof;
        if append && !self.buffer.is_empty() && !page.text.is_empty() {
            self.buffer.push('\n');
            self.buffer.push_str(&page.text);
        } else if !append {
            self.buffer = page.text.clone();
        } else {
            self.buffer.push_str(&page.text);
        }
        let page_start = self.next_line;
        if page.lines > 0 {
            self.next_line += page.lines;
        }
        page_start
    }

    /// 主干那句「仅显示前 {0} 行」里的 `{0}` = `_previewNextLine - 1`，
    /// 而 `PreviewMeta` 用的是 `Math.Max(0, _previewNextLine - 1)`。两处的 0 位口径不同：
    /// 横幅那处**不夹**（只在 `!eof` 时才弹，进得来就至少是 0 以上），meta 那处夹。
    #[must_use]
    pub fn shown_lines_for_banner(&self) -> i64 {
        self.next_line - 1
    }

    #[must_use]
    pub fn shown_lines_for_meta(&self) -> i64 {
        self.next_line.saturating_sub(1).max(0)
    }

    /// 主干 `LoadMoreButton.Visibility = _previewEof ? Collapsed : Visible`。
    #[must_use]
    pub fn shows_load_more(&self) -> bool {
        !self.eof
    }

    /// 主干 `ShowPreviewAsync` 文本档的**截断判据**：读完第一页、且还没 `eof` ⇒ 弹横幅。
    /// 判据是「**有没有 eof**」，不是「行数够不够 2000」——内核可以在第 3 行就给
    /// `too-large`（那是异常，走 [`preview_notice_for`]），也可以一页给满 2000 行同时 `eof=true`。
    #[must_use]
    pub fn truncation_advice(&self) -> Truncation {
        if self.eof {
            Truncation::Whole
        } else {
            Truncation::Truncated {
                shown_lines: self.shown_lines_for_banner(),
                limit: PREVIEW_LINE_LIMIT,
            }
        }
    }
}

/// 一页文本读完后的两态。`Truncated` 里带的是**要显示**的两个数，不是判据本身。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Truncation {
    /// 已经读到 `eof`：不弹横幅、不挂「加载更多」。
    Whole,
    /// 没读到 `eof`：弹「文件较长，仅显示前 {0} 行（内核单页上限 {1} 行 / 2 MiB）。」
    Truncated { shown_lines: i64, limit: i64 },
}

/// 「追加读」时主干到底增量渲染还是整段重画（`LoadTextPageAsync` 末尾那个 if）。
/// Markdown 即使 `append` 也走整段重画；`Html` 的**源码态**算在增量档里。
#[must_use]
pub fn renders_incrementally(face: PreviewFace, append: bool) -> bool {
    append
        && matches!(
            face,
            PreviewFace::Code | PreviewFace::Html | PreviewFace::PlainText
        )
}

// ---------------------------------------------------------------- 字节的三发（readAll / readBytes / readRelated）

/// 主干 `Preview.cs` 里 `DecodeB64` 的等价物：三发共用的那一颗。
///
/// 口径逐条照抄：
/// - `data` **缺键** / JSON `null` ⇒ 空串 ⇒ **短路**成空字节（`data.Length == 0 ? []`），
///   所以「空文件」在主干这里根本不是错。
/// - `data` 是数字/布尔/对象/数组 ⇒ 主干 `GetString()` **抛**，整个预览面转 [`preview_notice_for`]。
/// - 非空但不是合法 base64 ⇒ `Convert.FromBase64String` 抛 ⇒ 同一条路。
pub fn decode_data_field(value: &Value) -> Result<Vec<u8>, DataFieldError> {
    let root = require_object(value, "read* 回执").map_err(DataFieldError::Shape)?;
    match root.get("data") {
        // 主干：缺键 / null → "" → `[]`（短路，不进转换器）
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::String(text)) if text.is_empty() => Ok(Vec::new()),
        Some(Value::String(text)) => decode_standard_base64(text).map_err(DataFieldError::Base64),
        Some(_) => Err(DataFieldError::Shape(ParseError::NotString("data"))),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataFieldError {
    /// 回执形状不对（不是对象 / `data` 不是字符串）⇒ 主干抛在该次 `try` 里。
    Shape(ParseError),
    /// base64 串本身不合法 ⇒ 主干 `Convert.FromBase64String` 抛 `FormatException`。
    Base64(Base64Error),
}

/// `readBytes` 的取整循环（主干 `ReadFileBytesAsync`）：从 `offset = 0` 起，每段
/// `length = BYTE_CHUNK_LENGTH`，`offset += chunk.Length`，直到 `eof` 为真**或**这一段的
/// 解出来是 0 字节。⇒ 字节族**没有「只显示前 N」这种截断**，它是读到完；截断口径只存在于文本族。
#[must_use]
pub fn byte_fetch_done(eof: bool, chunk_len: usize) -> bool {
    eof || chunk_len == 0
}

/// 下一段的 `range.offset`。主干用的是**累计解出来的字节数**，不是内核回执里的 `offset`
/// （内核那个 `offset` 三发都不读）。
#[must_use]
pub fn byte_fetch_next_offset(cursor: i64, chunk_len: usize) -> i64 {
    cursor + chunk_len as i64
}

// ---------------------------------------------------------------- base64 校验（逐条实测 .NET）

/// 标准字母表 base64（带 `=` 补位）的校验 + 解码，判据逐条实测自
/// `System.Convert.FromBase64String`（.NET 10 实跑；与主干 net8.0 同一实现族）。
///
/// 现测出来的五条口径，少一条就不是主干那把尺：
/// 1. **空白被跳过**：`0x20` `0x09` `0x0A` `0x0D` 四个字符 anywhere 剔除 ⇒
///    `"aGVs bG8="` 合法、`"  "` 剔完变空串 ⇒ 空、`"aGVsbG8 "`（剔完剩 7 字符）非法。
/// 2. 剔除后长度必须 `% 4 == 0` ⇒ `"aGVsbG8"`（11 字符，Node 那种「省补位」写法）**抛**。
/// 3. 字母表只有 `A-Za-z0-9+/`：`-` 与 `_`（URL-safe）**抛** ⇒ 这就是分叉**不重用**
///    `kernel` 里那颗 URL-safe 解码机（`b64_urlsafe_decode`，字母表相反）的理由。
/// 4. `=` 只能出现在**最后一组**的**末尾**、最多两颗：`"aa=aa"`、`"=AAA"`、`"AA=A"`、
///    `"A==="`、`"===="` 全抛；`"AB=="`、`"aGVsbG8="` 过。
/// 5. 尾组多出来的位**不校验是否为零**（非规范化输入照收）：`"AAB="` 与 `"AAA="` 都给 `00 00`。
///    比主干严会让合法帧变错误帧 ⇒ 分叉同样放行。
#[must_use]
pub fn decode_standard_base64(input: &str) -> Result<Vec<u8>, Base64Error> {
    let mut chars = String::with_capacity(input.len());
    for ch in input.chars() {
        if matches!(ch, ' ' | '\t' | '\n' | '\r') {
            continue;
        }
        chars.push(ch);
    }
    if chars.len() % 4 != 0 {
        return Err(Base64Error::NotGrouped);
    }
    let bytes: Vec<u8> = chars.into_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let total = bytes.len() / 4;
    for (index, group) in bytes.chunks_exact(4).enumerate() {
        // 末组才允许补位：两颗 ⇒ 留 2 位数据，一颗 ⇒ 留 3 位，其余四位全数据。
        let kept = if index + 1 == total {
            match group[3] {
                b'=' if group[2] == b'=' => 2,
                b'=' => 3,
                _ => 4,
            }
        } else {
            4
        };
        let mut acc = 0u32;
        for slot in 0..4 {
            let ch = group[slot];
            if slot < kept {
                acc = (acc << 6) | data_value(ch)?;
            } else if ch == b'=' {
                acc <<= 6;
            } else {
                // 补位之后又出现数据位（`"AA=A"` 这一型）
                return Err(Base64Error::MisplacedPadding);
            }
        }
        match kept {
            2 => out.push((acc >> 16) as u8),
            3 => out.extend([(acc >> 16) as u8, (acc >> 8) as u8]),
            _ => out.extend([(acc >> 16) as u8, (acc >> 8) as u8, (acc & 0xFF) as u8]),
        }
    }
    Ok(out)
}

/// 一个数据位：字母表外的字符报 [`Base64Error::IllegalChar`]，但 `=` 落在数据位上是
/// 「补位位置不对」那一型（`"A==="` / `"aa=aa"` 在四条 4 倍数里都是 `=` 抢占数据位）。
fn data_value(ch: u8) -> Result<u32, Base64Error> {
    match value_of(ch) {
        Some(six) => Ok(six),
        None if ch == b'=' => Err(Base64Error::MisplacedPadding),
        None => Err(Base64Error::IllegalChar(ch)),
    }
}

/// 一个 base64 字符的 6 位值；`=` 与字母表外都不在这儿处理。
fn value_of(ch: u8) -> Option<u32> {
    match ch {
        b'A'..=b'Z' => Some((ch - b'A') as u32),
        b'a'..=b'z' => Some((ch - b'a' + 26) as u32),
        b'0'..=b'9' => Some((ch - b'0' + 52) as u32),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base64Error {
    /// 剔除空白后长度不是 4 的整倍数（含「省了补位」这一型）。
    NotGrouped,
    /// 字母表外的字符（含 `-` / `_`：标准 base64 不吃 URL-safe）。
    IllegalChar(u8),
    /// `=` 出现在非尾部补位槽，或末组补位超过两颗。
    MisplacedPadding,
}

// ---------------------------------------------------------------- 位图闸（出图另立一族）

/// 主干 `LoadImageFaceAsync` 里「能不能当位图」的判据。注意主干**不看魔数**、
/// 也不按扩展名再判一遍（扩展名只在上一步 [`face_of`] 决定发不发这发）：
/// 它的判据只有「字节数是不是 0」+「平台解码器抛不抛」。
#[must_use]
pub fn image_gate(bytes: &[u8]) -> BitmapGate {
    if bytes.is_empty() {
        BitmapGate::EmptyFile
    } else {
        BitmapGate::HandToDecoder
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitmapGate {
    /// 主干：`ShowPreviewErrorFace("空文件", "文件大小为 0，没有可显示的图像。")`，**不**抛。
    EmptyFile,
    /// 交给系统位图解码器。解码结果由宿主回填给 [`image_face_outcome`]。
    HandToDecoder,
}

/// 平台解码器的两种回执映射到主干的两个面。分叉**不自己解码**（出图是另一族），
/// 只把「解码失败 ⇒ 那句『已读取 {0} 字节，但系统位图解码器无法识别该格式』」这条口径钉住。
#[must_use]
pub fn image_face_outcome(gate: BitmapGate, decoded: bool, byte_len: usize) -> ImageFace {
    match gate {
        BitmapGate::EmptyFile => ImageFace::EmptyFile,
        BitmapGate::HandToDecoder if decoded => ImageFace::Bitmap { bytes: byte_len },
        BitmapGate::HandToDecoder => ImageFace::Undecodable { bytes: byte_len },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFace {
    Bitmap { bytes: usize },
    EmptyFile,
    /// SVG 与损坏字节在主干是**同一个**面（那句「SVG 等矢量图不在壳内渲染」只是文案里点名）。
    Undecodable { bytes: usize },
}

/// 主干 `LooksLikeEncryptedPdf`：字节数 < 8 直接 false；否则只看**尾部 64 KiB**，
/// 按 **Latin-1** 铺成串，找 `"/Encrypt"`（Ordinal 含）。这是启发式，主干注释自己认了。
#[must_use]
pub fn looks_like_encrypted_pdf(bytes: &[u8]) -> bool {
    if bytes.len() < 8 {
        return false;
    }
    let start = bytes.len().saturating_sub(64 * 1024);
    let window: String = bytes[start..].iter().map(|byte| *byte as char).collect();
    window.contains("/Encrypt")
}

// ---------------------------------------------------------------- HTML 附属（readRelated 的实参来源）

/// 主干 `DiscoverHtmlRelated`：从 `read` 拿到的源码缓冲里收集**相对路径**的 `src`/`href`，
/// 顺序 = 首次出现序，去重按 OrdinalIgnoreCase。七道过滤逐条照抄，过滤顺序不许动：
/// `Trim` 后空 ⇒ 跳；`data:` 前缀（忽略大小写）⇒ 跳；`#` 开头 ⇒ 跳；
/// `^[a-zA-Z][a-zA-Z0-9+.-]*:` 这种**任何**协议 ⇒ 跳（含 `javascript:`/`mailto:`）；
/// `/` 或 `\` 开头 ⇒ 跳；切掉第一个 `?`/`#` 之后做百分号解码、`\`→`/`；
/// 结果为空或**含 `..`** ⇒ 跳。截断口径 = [`MAX_HTML_RELATED`]，且**只截读取**，
/// 超了另起一句「…共 {0} 项」。
#[must_use]
pub fn discover_html_related(html: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for raw in scan_src_or_href(html) {
        let trimmed = raw.trim();
        if trimmed.is_empty()
            || trimmed
                .get(..5)
                .is_some_and(|head| head.eq_ignore_ascii_case("data:"))
            || trimmed.starts_with('#')
            || has_uri_scheme(trimmed)
            || trimmed.starts_with('/')
            || trimmed.starts_with('\\')
        {
            continue;
        }
        let cut = trimmed.find(['?', '#']);
        let sliced = match cut {
            Some(index) => &trimmed[..index],
            None => trimmed,
        };
        let path = percent_decode(sliced).replace('\\', "/");
        if path.is_empty() || path.contains("..") {
            continue;
        }
        if seen.iter().any(|held| ordinal_ignore_case_eq(held, &path)) {
            continue;
        }
        seen.push(path.clone());
        found.push(path);
    }
    found
}

/// 主干那条正则 `(?:src|href)\s*=\s*["']([^"']+)[""]`（IgnoreCase）的等价扫描。
fn scan_src_or_href(html: &str) -> Vec<String> {
    let units: Vec<char> = html.chars().collect();
    let mut hits = Vec::new();
    let mut index = 0;
    while index + 3 < units.len() {
        let is_src = matches!(&units[index..index + 3], [s, r, c]
            if s.eq_ignore_ascii_case(&'s') && r.eq_ignore_ascii_case(&'r') && c.eq_ignore_ascii_case(&'c'));
        let is_href = index + 4 < units.len()
            && matches!(&units[index..index + 4], [h, r, e, f]
                if h.eq_ignore_ascii_case(&'h') && r.eq_ignore_ascii_case(&'r')
                    && e.eq_ignore_ascii_case(&'e') && f.eq_ignore_ascii_case(&'f'));
        if !is_src && !is_href {
            index += 1;
            continue;
        }
        let mut cursor = index + if is_src { 3 } else { 4 };
        // \s*=\s*
        while units.get(cursor).is_some_and(|c| c.is_whitespace()) {
            cursor += 1;
        }
        if units.get(cursor) != Some(&'=') {
            index += 1;
            continue;
        }
        cursor += 1;
        while units.get(cursor).is_some_and(|c| c.is_whitespace()) {
            cursor += 1;
        }
        let quote = match units.get(cursor) {
            Some('\'') | Some('"') => units[cursor],
            _ => {
                index += 1;
                continue;
            }
        };
        cursor += 1;
        let start = cursor;
        while units.get(cursor).is_some_and(|c| *c != quote) {
            cursor += 1;
        }
        if cursor >= units.len() {
            break;
        }
        hits.push(units[start..cursor].iter().collect());
        index = cursor + 1;
    }
    hits
}

/// 主干 `^[a-zA-Z][a-zA-Z0-9+.-]*:` —— 首字符是字母，后面**只**允许字母/数字/`+`/`-`/`.`，
/// 撞到 `:` 才算带协议。字母漏掉会让 `http:`/`mailto:` 整批漏判（现测踩过一次）。
fn has_uri_scheme(raw: &str) -> bool {
    let mut chars = raw.chars();
    let Some(head) = chars.next() else { return false };
    if !head.is_ascii_alphabetic() {
        return false;
    }
    for ch in chars {
        match ch {
            ':' => return true,
            '0'..='9' | '+' | '-' | '.' => continue,
            ch if ch.is_ascii_alphabetic() => continue,
            _ => return false,
        }
    }
    false
}

/// `Uri.UnescapeDataString` 的等价物：只解 `%XX`（UTF-8 字节序列），非法序列**原样留着**。
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    let mut pending: Vec<u8> = Vec::new();
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3])
                .ok()
                .and_then(|pair| u8::from_str_radix(pair, 16).ok());
            if let Some(byte) = hex {
                pending.push(byte);
                index += 3;
                continue;
            }
        }
        flush_pending(&mut pending, &mut out);
        // 主干按 UTF-16 逐字符解码，非 ASCII 的原样字符在这里等价透传。
        let ch_len = utf8_len(bytes[index]);
        out.push_str(&text[index..index + ch_len]);
        index += ch_len;
    }
    flush_pending(&mut pending, &mut out);
    out
}

fn utf8_len(head: u8) -> usize {
    if head < 0x80 {
        1
    } else if head >> 5 == 0b110 {
        2
    } else if head >> 4 == 0b1110 {
        3
    } else {
        4
    }
}

fn flush_pending(pending: &mut Vec<u8>, out: &mut String) {
    if pending.is_empty() {
        return;
    }
    out.push_str(&String::from_utf8_lossy(pending));
    pending.clear();
}

// ---------------------------------------------------------------- 错误码表（两张，主干本来就是两张）

/// 主干 `InfoBarSeverity` 的四档（值名照主干，弹法留宿主）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Informational,
    Success,
    Warning,
    Error,
}

/// 预览态的失败面：主干 `ShowPreviewPlaceholder` 那张表（六臂 + `_`）。
/// 「`_` ⇒ `(读取失败, ex.Message, Error)`」的 `ex.Message` 拿不到（异常在本模块外），
/// 所以那臂的 `detail_key` 给 `None`，让宿主把异常本体填进去。
#[must_use]
pub fn preview_notice_for(code: &str) -> (&'static str, &'static str, Severity) {
    match code {
        "workspace-file/not-text" => ("不支持预览", "该文件是二进制或非 UTF-8 文本（含 NUL 字节），壳内只预览文本。", Severity::Informational),
        "workspace-file/too-large" => ("文件过大", "超出内核单页读取上限（2 MiB / 5000 行），无法在这里完整预览。", Severity::Warning),
        "workspace-file/not-found" => ("文件不存在", "路径已消失（可能被移动或删除）。", Severity::Warning),
        "workspace-file/not-regular-file" => ("不支持预览", "该路径不是普通文件。", Severity::Informational),
        "workspace-file/outside-workspace" => ("超出工作区", "该路径不在当前会话的工作区内。", Severity::Warning),
        "gateway/lookup-not-found" => ("会话不可用", "该会话没有活跃 agent，内核无法解析工作区。", Severity::Warning),
        _ => ("读取失败", "", Severity::Error),
    }
}

/// `_` 那一臂：主干把 `ex.Message` 当 detail ⇒ 本模块给不出串，宿主负责。
pub const PREVIEW_FALLBACK_TITLE: &str = "读取失败";

/// 树/列表侧的友好串：主干 `ShowError` 那张表（六臂 + `_`）。
/// 与预览那张**不同名也不同臂** —— 现测差异：
/// 这张有 `not-directory`（预览那张没有）、有 `not-found` 但串是「路径不存在（可能已被移动或删除）。」、
/// 预览那张多一颗 `not-regular-file`；内核现测一共八颗 `workspace-file/*`，
/// `unknown-workspace` 与 `unsupported-address` **两张表都没有** ⇒ 落 `_`。
/// 分叉不许把两张表合成一张「更完整」的表（那是第二真相 + 比主干更聪明）。
#[must_use]
pub fn list_error_friendly(code: &str) -> Option<&'static str> {
    Some(match code {
        "workspace-file/not-found" => "路径不存在（可能已被移动或删除）。",
        "workspace-file/not-directory" => "该路径不是目录。",
        "workspace-file/outside-workspace" => "路径超出该会话工作区。",
        "workspace-file/too-large" => "内容超出内核单页上限。",
        "workspace-file/not-text" => "不是 UTF-8 文本，无法预览。",
        "gateway/lookup-not-found" => "该会话没有活跃 agent（会话未打开或已归档）。",
        _ => return None,
    })
}

/// 主干异常本体上能读出的那一颗码：`ex is DshRpcException rpc ? rpc.Code : ""`。
/// 非 RPC 异常（含解析抛出的 `NotString`/`NotInt64`）一律走 `_` 臂。
#[must_use]
pub fn friendly_or(code: Option<&str>, ex_message: &str, catalog: &Catalog) -> String {
    match code.and_then(list_error_friendly) {
        Some(key) => catalog.l(key),
        None => ex_message.to_string(),
    }
}

// ---------------------------------------------------------------- 文案（只引用 zh 键，英文在 i18n::EN）

/// 主干 `TLF("目录项超过内核上限，仅显示前 {0} 项。", list.Count)` 的 `{0}` = **过滤后的条目数**。
#[must_use]
pub fn truncated_directory_notice(count: usize, catalog: &Catalog) -> String {
    catalog.lf(
        "目录项超过内核上限，仅显示前 {0} 项。",
        &[count.to_string()],
    )
}

/// 主干「文件较长」那句：`{0}` = `_previewNextLine - 1`，`{1}` = `PreviewLineLimit`。
#[must_use]
pub fn long_file_notice(shown_lines: i64, catalog: &Catalog) -> String {
    catalog.lf(
        "文件较长，仅显示前 {0} 行（内核单页上限 {1} 行 / 2 MiB）。",
        &[shown_lines.to_string(), PREVIEW_LINE_LIMIT.to_string()],
    )
}

/// 文本四档的 meta 行：主干 `TLF("{0} · {1} 行 · 版本 {2}", FormatSize(_previewStatBytes), max(0,next-1), Short(version))`。
/// 注意 `{0}` 用的是 **`stat` 那次的 bytes**，不是 `read` 回执里的数 —— 这是一处真差异（报告 §1.3）。
#[must_use]
pub fn meta_text_line(stat_bytes: Option<i64>, pager: &TextPager, version: Option<&str>, catalog: &Catalog) -> String {
    catalog.lf(
        "{0} · {1} 行 · 版本 {2}",
        &[
            format_size(stat_bytes, catalog),
            pager.shown_lines_for_meta().to_string(),
            short_version(version),
        ],
    )
}

/// 图 / PDF 的 meta 行：`TLF("{0} · 版本 {1}", FormatSize(bytes.LongLength), Short(version))` —— `{0}` 换成了**实际拿到的字节数**。
#[must_use]
pub fn meta_bytes_line(bytes_len: usize, version: Option<&str>, catalog: &Catalog) -> String {
    catalog.lf(
        "{0} · 版本 {1}",
        &[format_size(Some(bytes_len as i64), catalog), short_version(version)],
    )
}

/// HTML DOM 态的 meta 行（多一颗 ` · DOM`）。
#[must_use]
pub fn meta_html_dom_line(stat_bytes: Option<i64>, version: Option<&str>, catalog: &Catalog) -> String {
    catalog.lf(
        "{0} · 版本 {1} · DOM",
        &[format_size(stat_bytes, catalog), short_version(version)],
    )
}

// ---------------------------------------------------------------- scope-id 态机

/// provider 的三态：主干 `ReloadAsync` 里 `_sessionIdProvider is null ? _session : provider()`
/// 那个二选一 —— 「没挂 provider」与「provider 给了 null」是**两件事**，必须分得开。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider<'a> {
    /// 主干 `_sessionIdProvider is null` / `_workspaceRootProvider is null` ⇒ 用已挂住的值。
    NoProvider,
    /// provider 在场：根还要过一次 [`normalize_root`]（主干就是这么干的）。
    Provided(Option<&'a str>),
}

/// 主干 `FilesPanel` 的会话/scope 与三枚 generation。
///
/// **`workspaceFileScopeId` 到底谁持有**（现测）：主干六发的 args 里那颗
/// `workspaceFileScopeId` 传的**就是会话 id**（`_session`），壳侧不持有任何「解析后的 scope」；
/// 内核 typert 现测这颗参数的来源是 `source: "lookup"` + `lookup: "workspaceFileScope"`，
/// 由会话 `header.cwd` 在工作区注册表里解出根 ⇒ **解析在内核侧**。壳只保证三件事：
/// ①`sid` 为 `null` 时**一发都不发**（走「未选择会话」占位）；②根只用于拼 `path`，
/// 且拿不到根时回落成 `"."`；③换会话 = 整棵状态重建（generation 全跳、流收掉）。
#[derive(Debug, Clone, Default)]
pub struct PanelScope {
    /// 主干 `_session`
    pub session: Option<String>,
    /// 主干 `_root`：归一化后的工作区根，`None` = 没有根
    pub root: Option<String>,
    /// 主干 `_generation`：作废旧树请求及其回写
    pub generation: i64,
    /// 主干 `_previewGeneration`：作废旧预览（含返回后仍在途的读取）
    pub preview_generation: i64,
    /// 主干 `_streamGeneration`：作废已关闭/重连前的流回调
    pub stream_generation: i64,
    /// 主干 `_busy`：当前树请求是否进行中
    pub busy: bool,
    /// 主干 `_changesStreamId is not null`（流 id 本体归流侧，这里只留「有没有」）
    pub changes_stream_open: bool,
    /// 主干 `_changesOpening`
    pub changes_opening: bool,
    /// 主干 `_changesReady`
    pub changes_ready: bool,
}

impl PanelScope {
    /// 真正发到内核的那颗 `workspaceFileScopeId`。`None` ⇒ 一发都不该发。
    #[must_use]
    pub fn scope_id(&self) -> Option<&str> {
        self.session.as_deref()
    }

    /// 主干 `var root = _root ?? "."; // cwd 缺失时让内核按 session 的 sandbox 根解析`
    /// 与 `RefreshAsync` 里同一行 —— 这是**回落**，不是错误。
    #[must_use]
    pub fn list_root(&self) -> String {
        self.root.clone().unwrap_or_else(|| ".".to_string())
    }

    /// 主干 `SetSession` 的相等门：`sessionId == _session && root == _root` 则**直接 return**
    /// （注意比的是归一化后的根、**原始**的会话 id）。返回 `true` = 真的重建了。
    pub fn set_session(&mut self, session_id: Option<&str>, raw_root: Option<&str>) -> bool {
        let root = normalize_root(raw_root);
        if session_id == self.session.as_deref() && root == self.root {
            return false;
        }
        self.reset_session(session_id.map(str::to_string), root);
        true
    }

    /// 主干 `ResetSession`：`_generation++`、`_busy=false`、换会话/换根、`CloseChangesStream()`、
    /// `ShowTree()`（后者里 `_previewGeneration++` 并清预览位）。
    pub fn reset_session(&mut self, session_id: Option<String>, root: Option<String>) {
        self.generation += 1;
        self.busy = false;
        self.session = session_id;
        self.root = root;
        self.close_changes();
        self.preview_generation += 1;
    }

    /// 主干 `ReloadAsync` 的 provider 对齐：两枚 provider 各自「在场就用它」，
    /// 任一不同就 `ResetSession`。返回 `true` = 重建过。
    pub fn align_to_providers(&mut self, session_provider: Provider<'_>, root_provider: Provider<'_>) -> bool {
        let session = match session_provider {
            Provider::NoProvider => self.session.clone(),
            Provider::Provided(value) => value.map(str::to_string),
        };
        let root = match root_provider {
            Provider::NoProvider => self.root.clone(),
            Provider::Provided(value) => normalize_root(value),
        };
        if session != self.session || root != self.root {
            self.reset_session(session, root);
            return true;
        }
        false
    }

    /// 主干 `ReloadCurrentAsync` / `RefreshAsync` 的开头：`var generation = ++_generation; _busy = true;`
    /// 返回 `(generation, sid)`；`sid` 为 `None` 时主干**当场只挂占位、一发不发**。
    pub fn begin_tree_request(&mut self) -> (i64, Option<String>, String) {
        let session = self.session.clone();
        let root = self.list_root();
        self.generation += 1;
        self.busy = true;
        (self.generation, session, root)
    }

    /// 主干 `IsCurrent(generation, sid) => generation == _generation && sid == _session`。
    #[must_use]
    pub fn is_current(&self, generation: i64, session_id: Option<&str>) -> bool {
        generation == self.generation && session_id == self.session.as_deref()
    }

    /// 预览开始 = 主干 `var previewGeneration = ++_previewGeneration;`。
    pub fn bump_preview_generation(&mut self) -> i64 {
        self.preview_generation += 1;
        self.preview_generation
    }

    #[must_use]
    pub fn is_preview_current(&self, preview_generation: i64) -> bool {
        preview_generation == self.preview_generation
    }

    /// 主干 `EnsureChangesStreamAsync` 的四条件闸（第七发，流侧；态在壳这颗还是要照抄）：
    /// `rpc` 在、`_session == sid`、面板可见、且「还没有流 id 也没在开流」。
    #[must_use]
    pub fn may_open_changes(&self, session_id: &str, visible: bool) -> bool {
        self.session.as_deref() == Some(session_id)
            && visible
            && !self.changes_stream_open
            && !self.changes_opening
    }

    /// 主干 `CloseChangesStream`：`_streamGeneration++`、opening/ready 清零、流 id 归零、停去抖。
    pub fn close_changes(&mut self) {
        self.stream_generation += 1;
        self.changes_opening = false;
        self.changes_ready = false;
        self.changes_stream_open = false;
    }

    /// 主干 `Attach`：换 rpc 时先摘旧流回调，再 `_generation++`、`_previewGeneration++`、`_busy=false`
    /// （**注意它不动 `_session`/`_root`**：重挂 rpc 不该把会话忘掉）。
    pub fn reattach(&mut self) {
        self.generation += 1;
        self.preview_generation += 1;
        self.busy = false;
    }

    /// 主干 `OnVisibilityChanged(false)`：两枚 generation 各跳一次、`_busy=false`、收流。
    pub fn on_hidden(&mut self) {
        self.generation += 1;
        self.preview_generation += 1;
        self.busy = false;
        self.close_changes();
    }
}

// ---------------------------------------------------------------- 预览态（stat 两处的消费差）

/// 主干 `_previewVersion` / `_previewStatBytes` / `_previewPath` 三颗。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PreviewState {
    pub path: Option<String>,
    pub version: Option<String>,
    pub stat_bytes: Option<i64>,
    pub absolute_path: Option<String>,
}

impl PreviewState {
    /// **第一发 `stat`**（主干 `ShowPreviewAsync`）：`path` 取自**被点的条目** `file.Path`，
    /// 回执里 `version` 与 `bytes` **两颗都读**，而且两颗都是**无条件覆盖**
    /// （`_previewVersion = version`；缺键 ⇒ 直接写 `null`，不保持旧值）。
    /// 同时 `ResetPreviewBody()` 已把 `_previewStatBytes` 清成 `null`。
    pub fn apply_stat(&mut self, stat: &FileStat) {
        self.version = stat.version.clone();
        self.stat_bytes = stat.bytes;
    }

    /// **第二发 `stat`**（主干 `RefreshPreviewAsync`）：`path` 取自**存着的** `_previewPath`，
    /// 回执**只读 `version` 一颗**（`bytes` 读了也不写回去 ⇒ `_previewStatBytes` 停在第一次的值），
    /// 且**不改** `_previewVersion`（所以下一次复查还是跟第一次比，横幅不会自己消失）。
    /// 相等判据 = `string.Equals(version, _previewVersion, StringComparison.Ordinal)`，
    /// .NET 那位的 `(null, null)` 是**相等** ⇒ 两边都缺键时不弹横幅。
    pub fn changed_banner_from(&self, stat: &FileStat) -> bool {
        !stat_version_equals(stat.version.as_deref(), self.version.as_deref())
    }
}

/// 主干 `string.Equals(a, b, StringComparison.Ordinal)`：两边都 `null` 也算相等。
#[must_use]
pub fn stat_version_equals(fresh: Option<&str>, held: Option<&str>) -> bool {
    match (fresh, held) {
        (None, None) => true,
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// 主干 `RefreshPreviewAsync` 的失败口径（现测，别当疏忽）：**只有**
/// `workspace-file/not-found` 才换占位面，其余异常**整颗吞掉**（连横幅都不弹）。
#[must_use]
pub fn refresh_preview_failure_visible(code: Option<&str>) -> bool {
    code == Some("workspace-file/not-found")
}

/// 变更流去抖的窗长（主干 `DispatcherTimer(Interval = 450ms)`）。放这儿是因为
/// 「ready 前来的 change 也要刷一次」这条态机跟 scope 绑在一起。
pub const CHANGES_DEBOUNCE_MS: u64 = 450;

// ---------------------------------------------------------------- 测试

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn zh() -> Catalog {
        Catalog::load("zh", None)
    }

    fn en() -> Catalog {
        Catalog::load("en", None)
    }

    /// 主干 `ListDirectoryAsync` 的正常档：字段名、层级、`size` 可选、排序、`path` 由入参拼。
    #[test]
    fn list_reply_shape_dirs_first_and_paths_come_from_the_argument() {
        let value = json!({
            "path": "E:/demo/alpha",
            "entries": [
                {"name": "zeta.rs", "type": "file", "size": 12},
                {"name": "src", "type": "directory"},
                {"name": "Alpha.md", "type": "file", "size": 1},
                {"name": "node", "type": "other"}
            ],
            "truncated": false
        });
        let listing = parse_list_reply(&value, "E:/demo/alpha").expect("正常回执不该失败");
        let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["src", "Alpha.md", "node", "zeta.rs"], "目录必须在前，同档名字序");
        assert_eq!(listing.entries[0].path, "E:/demo/alpha/src", "path = JoinPath(入参根, 名)");
        assert_eq!(listing.entries[0].kind, KIND_DIRECTORY);
        assert_eq!(listing.entries[0].size, None, "size 缺键 ⇒ None，不是 0");
        assert_eq!(listing.entries[2].name, "node");
        assert_eq!(listing.entries[2].kind, KIND_OTHER, "type=other 原样存");
        assert!(!listing.truncated);
    }

    /// 空数组 vs 缺键：`entries: []` 是合法空目录；**缺 `entries` 这颗**必须失败（主干 GetProperty 抛）。
    #[test]
    fn empty_array_is_a_directory_but_a_missing_entries_key_fails() {
        let empty = json!({"path": "r", "entries": [], "truncated": false});
        let listing = parse_list_reply(&empty, "r").expect("空数组不该失败");
        assert!(listing.entries.is_empty());
        let kids = children_of(&listing, "（空目录）");
        assert_eq!(kids.len(), 1, "FillNode 在空目录上插唯一一颗占位行");
        assert!(kids[0].is_placeholder && !kids[0].is_directory());
        let missing = json!({"path": "r", "truncated": false});
        assert_eq!(
            parse_list_reply(&missing, "r"),
            Err(ParseError::MissingField("entries")),
            "缺键不许当成空目录"
        );
    }

    /// `truncated` 的三态：缺键 ⇒ false、JSON `true` ⇒ true、写成 1 或 "true" ⇒ **还是 false**
    /// （主干 `&& tr.ValueKind == JsonValueKind.True`）。
    #[test]
    fn truncated_only_acks_the_json_true_literal() {
        let base = json!({"entries": []});
        assert!(!parse_list_reply(&base, "r").unwrap().truncated, "缺键 ⇒ false");
        for (probe, want) in [(json!(true), true), (json!(1), false), (json!("true"), false), (json!(Value::Null), false)] {
            let mut value = base.clone();
            value["truncated"] = probe.clone();
            let got = parse_list_reply(&value, "r").unwrap().truncated;
            assert_eq!(got, want, "truncated = {probe} 时");
        }
    }

    /// 「不许比主干更聪明」这一型最典型的三条：类型不对是**整次失败**，不是这一行回落。
    #[test]
    fn wrong_typed_fields_abort_the_whole_listing_the_way_system_text_json_throws() {
        // name 是数字 ⇒ 主干 GetString() 抛 ⇒ 整个目录失败（连其它条目都不该留下）
        let value = json!({"entries": [{"name": "ok.txt"}, {"name": 7, "type": "file"}]});
        assert_eq!(parse_list_reply(&value, "r"), Err(ParseError::NotString("name")));
        // type 是数字 ⇒ 同上；**不是**退化成 "other"
        let value = json!({"entries": [{"name": "ok.txt", "type": 2}]});
        assert_eq!(parse_list_reply(&value, "r"), Err(ParseError::NotString("type")));
        // 元素不是对象 ⇒ 主干 TryGetProperty 抛
        let value = json!({"entries": [42]});
        assert_eq!(parse_list_reply(&value, "r"), Err(ParseError::NotObject("entries 元素")));
        // size 是小数 ⇒ 主干 GetInt64() 抛
        let value = json!({"entries": [{"name": "a", "size": 1.5}]});
        assert_eq!(parse_list_reply(&value, "r"), Err(ParseError::NotInt64("size")));
        // 但 size 是字符串 ⇒ 主干被 ValueKind==Number 夹住，静默给 None（这条回落不许升级）
        let value = json!({"entries": [{"name": "a", "size": "12"}]});
        assert_eq!(parse_list_reply(&value, "r").unwrap().entries[0].size, None);
    }

    /// 空名条目被整颗丢掉（主干 `continue`），而不是生成一个无名行。
    #[test]
    fn empty_and_missing_and_null_names_all_drop_the_row() {
        let value = json!({"entries": [
            {"name": "", "type": "file"},
            {"name": Value::Null, "type": "file"},
            {"type": "file"},
            {"name": "keep", "type": "file"}
        ]});
        let listing = parse_list_reply(&value, "r").unwrap();
        assert_eq!(listing.entries.len(), 1);
        assert_eq!(listing.entries[0].name, "keep");
    }

    /// OrdinalIgnoreCase 是**简单**折叠：`ß` 不许被当成 `ss`（那是 Rust 完整折叠的口径）。
    #[test]
    fn ordinal_ignore_case_folding_is_simple_not_full() {
        assert_eq!(ordinal_ignore_case_cmp("B", "a"), Ordering::Greater);
        assert!(ordinal_ignore_case_eq("ReadMe.MD", "readme.md"));
        // ß 与 "ss" 在主干 OrdinalIgnoreCase 下**不等**
        assert!(!ordinal_ignore_case_eq("\u{7df}", "ss"), "折叠必须是简单档");
        assert_eq!(
            ordinal_ignore_case_cmp("\u{7df}", "ss"),
            '\u{7df}'.cmp(&'s'),
            "ß 当单字比较"
        );
    }

    #[test]
    fn path_helpers_follow_the_mainline_pair() {
        assert_eq!(normalize_root(Some("E:\\demo\\alpha\\\\")).as_deref(), Some("E:/demo/alpha"));
        assert_eq!(normalize_root(Some("   ")), None, "IsNullOrWhiteSpace ⇒ null");
        assert_eq!(normalize_root(Some("/")), None, "全是斜杠 ⇒ 空 ⇒ null");
        assert_eq!(normalize_root(None), None);
        assert_eq!(join_path("E:/demo/", "a"), "E:/demo/a");
        assert_eq!(join_path(".", "a"), "./a", "根回落成 \".\" 时拼出来就是这个形状");
        assert_eq!(display_name_of("E:/demo/alpha/"), "alpha");
        assert_eq!(display_name_of("."), ".");
        assert_eq!(display_name_of("/"), "", "整串就是斜杠 ⇒ 主干回落到 trimmed = \"\"");
        assert_eq!(display_name_of("plain"), "plain");
    }

    #[test]
    fn version_shortening_and_size_formatting() {
        assert_eq!(short_version(None), "-");
        assert_eq!(short_version(Some("")), "-");
        assert_eq!(short_version(Some("0123456789ab")), "01234567");
        assert_eq!(short_version(Some("abc")), "abc");
        let catalog = zh();
        assert_eq!(format_size(None, &catalog), "大小未知");
        assert_eq!(format_size(Some(512), &catalog), "512 B");
        assert_eq!(format_size(Some(1024), &catalog), "1 KiB", "0.# 的零尾要去掉");
        assert_eq!(format_size(Some(1536), &catalog), "1.5 KiB");
        assert_eq!(format_size(Some(1024 * 1024), &catalog), "1 MiB");
        assert_eq!(format_size(Some(1024 * 1024 + 1024 * 1024 / 2), &catalog), "1.5 MiB");
        assert_eq!(format_decimal(1.5, 1), "1.5");
        assert_eq!(format_decimal(1.0, 1), "1", "0.# 的零尾与悬空点都要去掉");
        assert_eq!(format_decimal(1.126, 2), "1.13", "中点远离零，不是银行家舍入");
        // 真实入参永远是 bytes/1024^k ⇒ 二进制精确，落不到 .005 那类中点上
        assert_eq!(format_decimal(1.125, 2), "1.13", "恰好中点：远离零");
    }

    /// `GetExtension` 的三条反直觉现测口径（它们决定了发哪一发读）。
    #[test]
    fn get_extension_is_not_the_filename_extension() {
        assert_eq!(get_extension("a/b/c.rs"), ".rs");
        assert_eq!(get_extension(".gitignore"), ".gitignore", "点开头整串算扩展名 ⇒ 主干 Code 表里为什么有它");
        assert_eq!(get_extension("archive."), "", "末尾那颗点不算");
        assert_eq!(get_extension("a.tar.gz"), ".gz");
        assert_eq!(get_extension("dir/file"), "");
        assert_eq!(get_extension("a\\b.png"), ".png");
        assert_eq!(face_of("E:/w/NOTES.MD"), PreviewFace::Markdown, "大小写折叠");
        assert_eq!(face_of("E:/w/x.svg"), PreviewFace::Image);
        assert_eq!(face_of("E:/w/x.pdf"), PreviewFace::Pdf);
        assert_eq!(face_of("E:/w/.gitignore"), PreviewFace::Code);
        assert_eq!(face_of("E:/w/noext"), PreviewFace::PlainText);
        assert!(is_text_face(PreviewFace::Html), "Html 算文本档（复制全文要用）");
        assert!(!is_text_face(PreviewFace::Image));
        assert!(is_hash_comment_file(Some("a/b.py")));
        assert!(!is_hash_comment_file(Some("a/b.js")));
    }

    /// stat 两处的**消费差**：同形状 args、第一处覆盖 version+bytes，第二处只看 version 且不回写。
    #[test]
    fn the_two_stat_calls_differ_in_consumption_not_in_arguments() {
        let stat = parse_stat_reply(&json!({"absolutePath": "E:/x", "version": "v1", "bytes": 33}))
            .expect("合法 stat");
        assert_eq!(stat.version.as_deref(), Some("v1"));
        assert_eq!(stat.bytes, Some(33));
        // absolutePath 主干不从 stat 读 ⇒ FileStat 里根本没有这颗
        let mut state = PreviewState::default();
        state.apply_stat(&stat);
        assert_eq!(state.version.as_deref(), Some("v1"));
        assert_eq!(state.stat_bytes, Some(33));
        // 第二次：内核又给了新 bytes ⇒ 不许写回（主干那句压根没读）
        let second = parse_stat_reply(&json!({"version": "v2", "bytes": 99})).unwrap();
        assert!(state.changed_banner_from(&second));
        assert_eq!(state.stat_bytes, Some(33), "第二处不更新 bytes");
        assert_eq!(state.version.as_deref(), Some("v1"), "第二处也不回写 version ⇒ 横幅不会自愈");
    }

    /// 缺键方向的两处不同：`stat` 的 version 缺键 ⇒ **null**（无条件覆盖），
    /// `read` 的 version 缺键 ⇒ **保持旧值**（主干那三处 `: _previewVersion`）。
    #[test]
    fn missing_version_key_falls_back_differently_at_the_two_sites() {
        let mut state = PreviewState {
            version: Some("old".to_string()),
            ..Default::default()
        };
        state.apply_stat(&parse_stat_reply(&json!({"bytes": 1})).unwrap());
        assert_eq!(state.version, None, "stat 那处：缺键 ⇒ 写 null（主干 `? v.GetString() : null`）");

        let mut held = Some("old".to_string());
        let page = parse_read_reply(&json!({"text": "hi", "lines": 1})).unwrap();
        if page.version.is_some() {
            held = page.version.clone();
        }
        assert_eq!(held.as_deref(), Some("old"), "read 那处：缺键 ⇒ 保持旧值");
    }

    /// `ReadPage` 的失败半径与回落：`text` 缺键 ⇒ 空串，`lines` 缺键 ⇒ 0，
    /// 但 `text` 是数字 ⇒ 抛。
    #[test]
    fn read_page_defaults_and_throw_directions() {
        let bare = parse_read_reply(&json!({})).expect("read 全缺键不该失败");
        assert_eq!(bare.text, "");
        assert_eq!(bare.lines, 0);
        assert!(!bare.eof);
        assert_eq!(bare.version, None);
        let bad = json!({"text": 5});
        assert_eq!(parse_read_reply(&bad), Err(ParseError::NotString("text")));
        // `lines` 被 ValueKind==Number 夹过 ⇒ 布尔/串是静默 0；只有「是数但不是整数」才抛
        let bool_lines = json!({"lines": true});
        assert_eq!(parse_read_reply(&bool_lines).unwrap().lines, 0);
        let string_lines = json!({"lines": "3"});
        assert_eq!(parse_read_reply(&string_lines).unwrap().lines, 0);
        let bad = json!({"lines": 2.5});
        assert_eq!(parse_read_reply(&bad), Err(ParseError::NotInt64("lines")));
    }

    /// 截断判据本体：**看 eof，不看行数**。
    #[test]
    fn truncation_is_decided_by_eof_not_by_the_line_count() {
        let mut pager = TextPager::default();
        // 内核一页给满 2000 行且 eof=true ⇒ 主干不弹横幅
        pager.apply_page(&ReadPage { text: "x".into(), lines: PREVIEW_LINE_LIMIT, eof: true, version: None, absolute_path: None }, false);
        assert_eq!(pager.truncation_advice(), Truncation::Whole);
        assert!(!pager.shows_load_more());
        assert_eq!(pager.next_line, 1 + PREVIEW_LINE_LIMIT);
        // 只给 3 行但 eof=false ⇒ 弹，且 `{0}` = next_line - 1 = 3
        let mut pager = TextPager::default();
        pager.apply_page(&ReadPage { text: "a\nb\nc".into(), lines: 3, eof: false, version: None, absolute_path: None }, false);
        assert_eq!(
            pager.truncation_advice(),
            Truncation::Truncated { shown_lines: 3, limit: PREVIEW_LINE_LIMIT }
        );
        assert!(pager.shows_load_more());
    }

    /// 分页游标 + 缓冲拼接的三分支，含「`lines == 0` 时游标不动」这条主干现状。
    #[test]
    fn pager_cursor_and_buffer_concat_follow_the_three_branches() {
        let mut pager = TextPager::default();
        let start = pager.apply_page(&ReadPage { text: "one".into(), lines: 1, eof: false, ..Default::default() }, false);
        assert_eq!(start, 1, "首页 pageStartLine = 1");
        pager.apply_page(&ReadPage { text: "two".into(), lines: 1, eof: false, ..Default::default() }, true);
        assert_eq!(pager.buffer, "one\ntwo", "append 且两边非空 ⇒ 补一个裸 \\n");
        assert_eq!(pager.next_line, 3);
        // 空页：append + text 空 ⇒ 走 else ⇒ 直接 += （不补 \n），且 lines=0 ⇒ 游标不动
        let before = pager.next_line;
        let start = pager.apply_page(&ReadPage::default(), true);
        assert_eq!(pager.buffer, "one\ntwo");
        assert_eq!(before, pager.next_line, "lines=0 不许推进游标（主干靠按钮禁用位挡原地）");
        assert_eq!(start, 3);
        // append 但缓冲为空 ⇒ 也走 +=（等价于直接落）
        let mut fresh = TextPager::default();
        fresh.apply_page(&ReadPage { text: "head".into(), lines: 1, eof: false, ..Default::default() }, true);
        assert_eq!(fresh.buffer, "head");
        // 非 append ⇒ 整段覆盖，且 pageStartLine 不参与增量渲染
        fresh.apply_page(&ReadPage { text: "replaced".into(), lines: 1, eof: false, ..Default::default() }, false);
        assert_eq!(fresh.buffer, "replaced");
        assert!(renders_incrementally(PreviewFace::Code, true));
        assert!(!renders_incrementally(PreviewFace::Markdown, true), "Markdown 永远整段重画");
        assert!(!renders_incrementally(PreviewFace::Code, false));
    }

    /// 文案两档实测：zh 直通、en 必须从 `i18n::EN` 取到（不在 EN 里就露出中文 ⇒ 报告 §5 会记）。
    #[test]
    fn the_notice_templates_resolve_through_the_catalog_not_a_second_table() {
        let catalog = zh();
        assert_eq!(truncated_directory_notice(200, &catalog), "目录项超过内核上限，仅显示前 200 项。");
        assert_eq!(
            long_file_notice(2000, &catalog),
            "文件较长，仅显示前 2000 行（内核单页上限 2000 行 / 2 MiB）。"
        );
        let pager = TextPager::default();
        assert_eq!(meta_text_line(Some(1536), &pager, Some("abcdefghij"), &catalog), "1.5 KiB · 0 行 · 版本 abcdefgh");
        assert_eq!(meta_bytes_line(1024, None, &catalog), "1 KiB · 版本 -");
        assert_eq!(meta_html_dom_line(Some(2048), Some("v"), &catalog), "2 KiB · 版本 v · DOM");

        let english = en();
        for (label, rendered) in [
            ("truncated-dir", truncated_directory_notice(300, &english)),
            ("long-file", long_file_notice(2000, &english)),
        ] {
            assert!(
                !rendered.chars().any(|c| c as u32 >= 0x4e00 && (c as u32) <= 0x9fff),
                "{label} 在英文档露出了中文：{rendered}"
            );
        }
    }

    /// base64：逐条钉 §2.2 那五条实测口径。
    #[test]
    fn base64_validators_match_the_measured_convert_frombase64string() {
        assert_eq!(decode_standard_base64(""), Ok(vec![]));
        assert_eq!(decode_standard_base64("YQ=="), Ok(vec![b'a']));
        assert_eq!(decode_standard_base64("aGVsbG8="), Ok(b"hello".to_vec()));
        // ① 空白四处剔除
        assert_eq!(decode_standard_base64("aGVs bG8="), Ok(b"hello".to_vec()));
        assert_eq!(decode_standard_base64("  "), Ok(vec![]));
        assert_eq!(decode_standard_base64("aGVsbG8=\t"), Ok(b"hello".to_vec()));
        assert_eq!(
            decode_standard_base64("aGVsbG8 ").unwrap_err(),
            Base64Error::NotGrouped,
            "剔掉空格剩 7 字符 ⇒ 抛（主干同一条）"
        );
        // ② 必须 4 的整倍数：Node 省补位那种写法主干不吃
        assert_eq!(decode_standard_base64("aGVsbG8").unwrap_err(), Base64Error::NotGrouped);
        for probe in ["aa=aa", "YQ===", "A"] {
            assert_eq!(
                decode_standard_base64(probe).unwrap_err(),
                Base64Error::NotGrouped,
                "{probe} 剔除空白后不是 4 的倍数"
            );
        }
        // ③ 标准字母表：URL-safe 的 -/_ 不吃 ⇒ 也是分叉不复用 kernel 那颗 URL-safe 解码机的理由
        assert_eq!(decode_standard_base64("-").unwrap_err(), Base64Error::NotGrouped);
        assert_eq!(
            decode_standard_base64("----").unwrap_err(),
            Base64Error::IllegalChar(b'-')
        );
        assert_eq!(
            decode_standard_base64("____").unwrap_err(),
            Base64Error::IllegalChar(b'_')
        );
        assert_eq!(
            decode_standard_base64("aGVs*G8=").unwrap_err(),
            Base64Error::IllegalChar(b'*')
        );
        // ④ 补位只能在末组末尾、最多两颗
        for probe in ["=AAA", "AA=A", "A===", "====", "AA=A=AAA"] {
            assert_eq!(
                decode_standard_base64(probe).unwrap_err(),
                Base64Error::MisplacedPadding,
                "{probe} 必须被拒"
            );
        }
        // ⑤ 尾组多出来的位不校验是否为零（比主干严 = 让合法帧变错误帧）
        assert_eq!(decode_standard_base64("AAB="), decode_standard_base64("AAA="), "非规范化照收");
        assert_eq!(decode_standard_base64("AAB=").unwrap(), vec![0, 0]);
        assert_eq!(decode_standard_base64("////").unwrap(), vec![0xff, 0xff, 0xff]);
    }

    /// `DecodeB64` 的三态：缺键/null/空串 ⇒ 空字节（不校验），非串 ⇒ 形状错。
    #[test]
    fn the_data_field_short_circuits_on_absence_but_throws_on_wrong_type() {
        assert_eq!(decode_data_field(&json!({})), Ok(vec![]));
        assert_eq!(decode_data_field(&json!({"data": Value::Null})), Ok(vec![]));
        assert_eq!(decode_data_field(&json!({"data": ""})), Ok(vec![]));
        assert_eq!(decode_data_field(&json!({"data": "YQ=="})).unwrap(), vec![b'a']);
        assert_eq!(
            decode_data_field(&json!({"data": 5})),
            Err(DataFieldError::Shape(ParseError::NotString("data")))
        );
        assert!(matches!(
            decode_data_field(&json!({"data": "!!!!"})),
            Err(DataFieldError::Base64(Base64Error::IllegalChar(b'!')))
        ));
    }

    #[test]
    fn byte_pages_accumulate_by_decoded_length_until_eof_or_an_empty_chunk() {
        let mut cursor = 0;
        let mut total = Vec::new();
        assert!(!byte_fetch_done(false, 10));
        for chunk in [BYTE_CHUNK_LENGTH, BYTE_CHUNK_LENGTH, 7] {
            total.push(chunk);
            cursor = byte_fetch_next_offset(cursor, chunk as usize);
            assert!(!byte_fetch_done(false, chunk as usize));
        }
        assert_eq!(cursor, 2 * BYTE_CHUNK_LENGTH + 7);
        assert!(byte_fetch_done(true, 0));
        assert!(byte_fetch_done(false, 0), "0 字节段是主干的第二条终止位（内核 eof 迟到的兜）");
        assert_eq!(total.len(), 3);
    }

    #[test]
    fn bitmap_gate_and_pdf_heuristic_are_only_two_questions() {
        // 主干不看魔数：判据只有「空不空」+「平台解码器抛不抛」
        assert_eq!(image_gate(&[]), BitmapGate::EmptyFile);
        assert_eq!(image_gate(&[0x89, b'P']), BitmapGate::HandToDecoder);
        assert_eq!(image_face_outcome(BitmapGate::EmptyFile, false, 0), ImageFace::EmptyFile);
        assert_eq!(image_face_outcome(BitmapGate::HandToDecoder, false, 4096), ImageFace::Undecodable { bytes: 4096 });
        assert_eq!(image_face_outcome(BitmapGate::HandToDecoder, true, 12), ImageFace::Bitmap { bytes: 12 });
        assert!(!looks_like_encrypted_pdf(&[]));
        assert!(!looks_like_encrypted_pdf(b"%PDF-1.7"), "少于 8 字节直接 false");
        let mut tail = b"%PDF-1.7 rest".to_vec();
        tail.extend_from_slice(b"trailer << /Encrypt 5 0 R >>");
        assert!(looks_like_encrypted_pdf(&tail));
        let mut plain = b"%PDF-1.7 body".to_vec();
        plain.extend_from_slice(b"/Pages 1 0 R");
        assert!(!looks_like_encrypted_pdf(&plain));
    }

    /// 两张错误码表是两张，不许合；且 `_` 臂的方向也不许改。
    #[test]
    fn the_two_error_code_tables_stay_two_tables() {
        assert_eq!(preview_notice_for("workspace-file/not-text").2, Severity::Informational);
        assert_eq!(preview_notice_for("workspace-file/too-large").2, Severity::Warning);
        assert_eq!(preview_notice_for("workspace-file/not-regular-file").0, "不支持预览");
        // 预览表里有 not-regular-file，树表里没有 ⇒ 后者落 None（= 主干的 ex.Message 臂）
        assert_eq!(list_error_friendly("workspace-file/not-regular-file"), None);
        assert!(list_error_friendly("workspace-file/not-directory").is_some());
        assert_eq!(preview_notice_for("anything-else").0, PREVIEW_FALLBACK_TITLE);
        assert_eq!(preview_notice_for("anything-else").2, Severity::Error);
        // 内核现测一共八颗，这两颗两张表都没收 ⇒ 必须落 `_`，分叉不替主干补表
        for orphan in ["workspace-file/unknown-workspace", "workspace-file/unsupported-address"] {
            assert_eq!(list_error_friendly(orphan), None, "{orphan} 不许被顺手补进表");
            assert_eq!(preview_notice_for(orphan).0, PREVIEW_FALLBACK_TITLE);
        }
        let catalog = zh();
        assert_eq!(friendly_or(Some("workspace-file/not-directory"), "raw", &catalog), "该路径不是目录。");
        assert_eq!(friendly_or(None, "raw", &catalog), "raw", "_ 臂 = 异常本体");
        assert_eq!(friendly_or(Some("gateway/lookup-not-found"), "raw", &catalog), "该会话没有活跃 agent（会话未打开或已归档）。");
        assert!(refresh_preview_failure_visible(Some("workspace-file/not-found")));
        assert!(!refresh_preview_failure_visible(Some("workspace-file/too-large")), "第二处只认 not-found");
        assert!(!refresh_preview_failure_visible(None));
    }

    #[test]
    fn html_related_discovery_keeps_order_dedupe_and_the_seven_filters() {
        let html = r##"<img src="a.png"><img src='A.png'><link href="./b.css">
            <script src="c.js?v=1#x"></script><a href="#top">n</a>
            <a href="http://x/y">web</a><a href="mailto:z@q">mail</a>
            <img src="data:image/png;base64,AA"><img src="/rooted.png">
            <img src="../up.png"><img src="sub\win.png"><img src="   ">
            <link href=unquoted.css><img src="%E2%9C%93d.png">"##;
        let found = discover_html_related(html);
        assert_eq!(
            found,
            vec!["a.png", "./b.css", "c.js", "sub/win.png", "\u{2713}d.png"],
            "首现序 + 忽略大小写去重 + 七道过滤；裸值（无引号）主干正则不吃，分叉也不吃"
        );
        assert_eq!(MAX_HTML_RELATED, 12);
        assert_eq!(percent_decode("%E2%9C%93"), "\u{2713}");
        assert_eq!(percent_decode("100%"), "100%", "非法序列原样留着");
        assert!(has_uri_scheme("javascript:x"));
        assert!(!has_uri_scheme("1javascript:x"));
        assert!(!has_uri_scheme("a/b"));
    }

    #[test]
    fn scope_machine_holds_only_the_session_id_and_falls_back_to_dot() {
        let mut scope = PanelScope::default();
        assert_eq!(scope.scope_id(), None, "没会话 ⇒ 一发都不该发");
        assert_eq!(scope.list_root(), ".", "主干 `_root ?? \".\"` 的回落");
        // SetSession 的相等门
        scope.set_session(Some("s-1"), Some("E:\\demo\\"));
        assert_eq!(scope.scope_id(), Some("s-1"));
        assert_eq!(scope.root.as_deref(), Some("E:/demo"));
        let before = scope.generation;
        assert!(!scope.set_session(Some("s-1"), Some("E:/demo//")), "同会话同根 ⇒ 直接 return");
        assert_eq!(scope.generation, before);
        // 换会话 = 整棵重建
        assert!(scope.set_session(Some("s-2"), Some("E:/demo")));
        assert!(scope.generation > before);
        assert!(!scope.busy);
        // begin 会占 busy 并跳 generation
        scope.busy = false;
        let (generation, sid, root) = scope.begin_tree_request();
        assert_eq!((sid.as_deref(), root.as_str()), (Some("s-2"), "E:/demo"));
        assert!(scope.is_current(generation, Some("s-2")));
        assert!(!scope.is_current(generation - 1, Some("s-2")), "旧代必须作废");
        assert!(!scope.is_current(generation, Some("s-1")), "换会话后旧回写全废");
        assert!(scope.busy);
    }

    #[test]
    fn providers_absent_and_providers_returning_null_are_two_different_things() {
        let mut scope = PanelScope { session: Some("s-9".into()), root: Some("E:/held".into()), ..Default::default() };
        // 主干 `provider is null ? _session : provider()` —— 没挂 provider ⇒ 保持
        assert!(!scope.align_to_providers(Provider::NoProvider, Provider::NoProvider));
        assert_eq!(scope.scope_id(), Some("s-9"));
        // provider 在场但给 null ⇒ 会话被清成 null（下一发就不该发了），根走 normalize_root
        assert!(scope.align_to_providers(Provider::Provided(None), Provider::Provided(None)));
        assert_eq!(scope.scope_id(), None);
        assert_eq!(scope.root, None);
        assert_eq!(scope.list_root(), ".");
        // provider 给带反斜杠的根 ⇒ 过 normalize_root（不是原样存）
        scope.align_to_providers(Provider::Provided(Some("s-3")), Provider::Provided(Some("C:\\ws\\")));
        assert_eq!(scope.root.as_deref(), Some("C:/ws"));
    }

    #[test]
    fn changes_gate_and_the_three_generations_move_the_way_the_panel_does() {
        let mut scope = PanelScope::default();
        scope.set_session(Some("s-1"), Some("E:/w"));
        assert!(scope.may_open_changes("s-1", true));
        assert!(!scope.may_open_changes("s-1", false), "面板藏起来就不留长驻流");
        assert!(!scope.may_open_changes("other", true), "会话不符不开");
        scope.changes_opening = true;
        assert!(!scope.may_open_changes("s-1", true), "在途开流时不许再开一发");
        scope.changes_opening = false;
        scope.changes_stream_open = true;
        assert!(!scope.may_open_changes("s-1", true), "已有流就不重复开");
        let stream_before = scope.stream_generation;
        scope.close_changes();
        assert_eq!(scope.stream_generation, stream_before + 1, "收流要跳代，作废在途回调");
        assert!(!scope.changes_ready && !scope.changes_stream_open);
        // reset_session 顺带把预览代也跳了（主干 ShowTree() 里那一发 ++）
        let preview_before = scope.preview_generation;
        scope.reset_session(Some("s-2".to_string()), Some("E:/w".to_string()));
        assert!(scope.preview_generation > preview_before);
        // OnVisibilityChanged(false) 与 Attach 都是「两代各跳一次 + 收流」
        let (g, p, s) = (scope.generation, scope.preview_generation, scope.stream_generation);
        scope.on_hidden();
        assert_eq!((scope.generation > g, scope.preview_generation > p, scope.stream_generation > s), (true, true, true));
        let (g, p) = (scope.generation, scope.preview_generation);
        scope.reattach();
        assert!((scope.generation > g) && (scope.preview_generation > p));
        assert_eq!(scope.scope_id(), Some("s-2"), "重挂 rpc 不该把会话忘掉（主干 Attach 不动 _session）");
        assert_eq!(CHANGES_DEBOUNCE_MS, 450);
    }

    /// 主干源对表：运行时**相对路径**现读（照 `tests/rt6_turnrail.rs` 的形），
    /// 读不到就 `eprintln!` + skip，绝不 panic。锁的是「本模块抄的那几颗判据字面串还在主干上」。
    #[test]
    fn fp1_judgments_still_match_the_mainline_source() {
        let Some(list_src) = mainline("Pages/FilesPanel.xaml.cs") else {
            eprintln!("fp1 skip：读不到主干 FilesPanel.xaml.cs，本把锁跳过");
            return;
        };
        let Some(preview_src) = mainline("Pages/FilesPanel.Preview.cs") else {
            eprintln!("fp1 skip：读不到主干 FilesPanel.Preview.cs，本把锁跳过");
            return;
        };
        // ① 单页上限那枚局部常量：值 + 它是 `const int` 而不是可配置项
        let block = tight_window(&list_src, "private const int PreviewLineLimit", "\n").unwrap_or_default();
        report_window("fp1", "PreviewLineLimit", &block);
        assert!(block.contains("2000"), "主干单页上限不再是 2000：{block}");
        assert_eq!(PREVIEW_LINE_LIMIT, 2000);
        // ② 截断判据的**方向**：主干弹横幅的条件是 `!_previewEof`，不是行数比较
        let banner = tight_window(&list_src, "if (previewGeneration == _previewGeneration && ", "\n")
            .unwrap_or_default();
        assert!(banner.contains("!_previewEof"), "截断判据挪窝了：{banner}");
        // ③ 字节段步长 + 附属上限
        let chunk = tight_window(&preview_src, "private const int ByteChunkLength", ";").unwrap_or_default();
        assert!(chunk.contains("256 * 1024"), "ByteChunkLength 漂了：{chunk}");
        assert_eq!(BYTE_CHUNK_LENGTH, 256 * 1024);
        let related = tight_window(&preview_src, "private const int MaxHtmlRelated", ";").unwrap_or_default();
        assert!(related.contains("12"), "MaxHtmlRelated 漂了：{related}");
        // ④ 四发的实参形状（逐字对：list/stat 两颗、read 带 range{offset,limit}、readBytes 带字节分段）。
        //    第五枚 `relativePath,`（readRelated 的第三颗实参）**已摘**：主干 `readRelated` 自内核 0.1.7
        //    起移除 ⇒ 两颗主干文件里 `relativePath,`（带尾逗号那形）现测各 0 命中，`Preview.cs` 全文件
        //    连 `relativePath` 都零。背书：分叉产品码 `workspaceFiles/readAll` / `readRelated` **零命中**
        //    ⇒ 分叉今天就没有这两发，红的只是「台账抄主干抄多了」，**不是「分叉功能要删」**，故不需裁定。
        for (needle, label) in [
            ("new { workspaceFileScopeId = sid, path }", "list/readAll 的两颗实参"),
            ("new { workspaceFileScopeId = sid, path = file.Path }", "stat 第一处"),
            ("range = new { offset = _previewNextLine, limit = PreviewLineLimit }", "read 的行分页"),
            ("range = new { offset, length = ByteChunkLength }", "readBytes 的字节分段"),
        ] {
            let in_list = list_src.contains(needle);
            let in_preview = preview_src.contains(needle);
            assert!(in_list || in_preview, "{label} 的实参形状在对不上主干：{needle}");
        }
        // ⑤ 那两发的 version 回落方向：`? v.GetString() : _previewVersion`（保持旧值），
        //    而 stat 那处是 `: null`（无条件覆盖）—— 两处口径不同，本模块两条锁各守一头。
        //    下界从 3 降到 **2**：第三发随主干 `readRelated` 下线一起没了（现测 `Preview.cs` 两处）。
        let keep_old = preview_src.matches(": _previewVersion;").count();
        assert!(keep_old >= 2, "Preview.cs 里「保持旧值」那两处少了一处：{keep_old}");
        eprintln!("fp1 主干现测：keep-old = {keep_old} 处");
        // ⑥ DecodeB64 的短路：主干确实是「空串直接给空数组」，不进转换器
        assert!(
            preview_src.contains("data.Length == 0 ? []"),
            "主干 DecodeB64 的短路方向变了"
        );
    }

    /// 运行时读主干源；读不到 ⇒ 打印原因并让调用方 skip（**不 panic**）。
    fn mainline(relative: &str) -> Option<String> {
        let path = format!("{}/../{}", env!("CARGO_MANIFEST_DIR"), relative);
        match std::fs::read_to_string(&path) {
            Ok(text) => Some(text),
            Err(err) => {
                eprintln!("fp1 读不到主干源 {path}（{err}）");
                None
            }
        }
    }

    /// 紧窗：`start` 首次出现 → 其后 `end` 首次出现。锚点缺失 ⇒ `None`，
    /// **不退化**成「到文件末尾」（那正是恒真窗口的来源）。
    fn tight_window(src: &str, start: &str, end: &str) -> Option<String> {
        let at = src.find(start)? + start.len();
        let tail = src[at..].find(end)?;
        Some(src[at - start.len()..at + tail].to_string())
    }

    /// 打印窗口真实行数，并判定它「紧」。
    fn report_window(lock_name: &str, label: &str, text: &str) {
        let lines = text.lines().count();
        eprintln!("{lock_name} {label} 窗口行数 = {lines}");
        assert!((1..=80).contains(&lines), "{label} 窗口松了（{lines} 行）");
    }
}
