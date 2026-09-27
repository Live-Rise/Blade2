//! P0-7「子代理目录 + 续跑 + 打断」那三发 `subagents/*` 的**纯逻辑层**。
//!
//! 只算不画、不发：条目模型、`subagents/list` 回执解析与标题补齐、可续跑/可打断判据、
//! 下级折叠态机、文案**选键**（不落第二份表）、三发的实参形状。零 `use crate::kernel::…`、
//! 零 `RpcCall` 构造、零控件、零派发 —— 宿主与派发在 `main.rs`（别人的地盘），
//! 三发的 ctor 在 `kernel.rs`（另一颗卡）。
//!
//! 规格来源 = 主干 `MainWindow.Subagents.cs` 这一颗 partial（**本批现测 662 行 /
//! md5 `d82613d4cdac468e61855bf3392cacbe`**）。下面注释里的 `:NNN` 都是**本批现测**的行号；
//! 本仓实测行号必漂，所以 §tests 的源码锁一律用**文本锚**开窗、不用行号（先例 `filespanel.rs`）。
//!
//! ## 三发的调用点与「回执那一层」
//!
//! * `subagents/list` → `:61`，实参 `new { parentSessionId }` ⇒ **返回 `value` 并解析**。
//!   `CallOkAsync` 的返回层现测在 `Dsh/DshRpcClient.cs:110-131`：HTTP `result`（`:104`）⇒ 断言
//!   `result.ok`（`:122`）⇒ **返回 `result.value`**（`:131`，缺 `value` 键给 `JsonValueKind.Undefined`）。
//!   ⇒ [`parse_catalog`] 的入参就是 `{ entries:[], parentAvailable:bool }` 那一层，
//!   ctor 落地后把手上的回执直接喂进来即可。
//! * `subagents/prompt` → `:122`、`subagents/interruptByParent` → `:146`：**两发都是 `await` 后丢弃**
//!   （现测全仓没有 `= await _rpc.CallOkAsync("subagents/prompt"` 这种赋值形态）。主干对这两发
//!   **没有任何回执判据** ⇒ 本模块只提供它们**发出去那一侧**的形状（[`PromptRequest::to_args`] /
//!   [`interrupt_args`]），不发明回执解析。
//!   错误码 `subagent/parent-unavailable`、`subagent/not-resumable` 只活在 `:114` 的注释里，
//!   代码里**没有分支**，只有 `:627`/`:657` 的 `catch (DshRpcException)` ⇒ `ShowErrorAsync` 一条路
//!   —— 分叉不得提前分档。
//!
//! ## one-shot / continuable：判据只有一条，而且从不比较 `"one-shot"`
//!
//! 头注 `:16-18`：`one-shot = 一次性（只读，不可续跑）；continuable = 可续跑（subagents/prompt）`。
//! 落地成代码后，**全部**四处判据都是 `Mode == "continuable"`（`:193`/`:396`/`:453`/`:476`），
//! 现测 `"one-shot"` 带引号的字面串在这颗文件里出现 **0 次**（`:475` 那个 `one-shot` 在注释里）。
//! ⇒ [`SubagentEntry::is_continuable`] 就是唯一判据；**非 continuable 一律按一次性对待**
//! （包括 `mode` 缺键、写成 `"foo"`、或干脆是数字）。内核 typert 现测确有
//! `'mode': z.literal("one-shot")`（`@deepseek-ai/dsh-subagent/lib/typert.host.js:18`），
//! 但主干不比较它 ⇒ 分叉多一个分支就是「发明」。
//!
//! ## 失败半径：只有「容器不是对象」会作废整次列目录
//!
//! 主干读回执用 `JsonElement`。`TryGetProperty` 对**非对象**元素抛（先例记录见 `filespanel.rs` 头注），
//! 但每个**字段**都先 `&& v.ValueKind == …` 设了门禁 ⇒ 字段类型不对是**静默回落**，不是抛。
//! 逐条回落方向抄在 [`parse_entry`] 里。唯一会抛的是 `entries` 的元素本身不是对象（如 `["x"]`）：
//! `SubagentsListAsync` 自己不带 `try`，抛出后由调用点的 `catch`（`:304-358`、`:540-550`）
//! 变成错误行 + 重试钮 ⇒ **整次作废**，绝不悄悄丢一条。
//! ⇒ [`parse_catalog`] 返回 [`Result`]，且只有 [`ParseError::EntryNotObject`] 一个臂。
//!
//! ## 文案：只选键，不落表
//!
//! 主干用 `L(中文)` 单语键（EN 表在 `MainWindow.xaml.cs:1886-1891` 一带），分叉对应物是
//! `crate::i18n::Catalog`。本模块**只引用 zh 键字面串**（`ZH_*` 那十一枚，现测 `rust/src/i18n.rs`
//! 已全部登记过英文 ⇒ 一颗都没新增），把 `L()` 的调用留给宿主；连 `Catalog` 都不 `use`，
//! 这样 i18n 那颗文件归谁改都不影响本模块编译。

use std::borrow::Cow;

use serde_json::{json, Value};

// ---------------------------------------------------------------- 主干钉死的字面量

/// `SubagentEntryVm.Kind` 的两个取值（`:27` 注释 `child | diagnostic`，分支在 `:89`）。
pub const KIND_CHILD: &str = "child";
pub const KIND_DIAGNOSTIC: &str = "diagnostic";

/// `Mode` 里唯一被比较的字面量（`:193`/`:396`/`:453`/`:476`）。
pub const MODE_CONTINUABLE: &str = "continuable";

/// `Activity` 里唯一被特殊对待的字面量（`:194`/`:397`/`:414`/`:476`）。
pub const ACTIVITY_RUNNING: &str = "running";

/// `activity` 缺键/非字符串时的回落值（`:105`，也是 `SubagentEntryVm.Activity` 的类型默认 `:32`）。
pub const ACTIVITY_INACTIVE: &str = "inactive";

/// `diagnostic.reason` 的三个有文案的取值（`:34` 注释 + `:368-373` 的 switch）。
pub const REASON_CORRUPT: &str = "corrupt";
pub const REASON_UNSUPPORTED: &str = "unsupported";
pub const REASON_UNAVAILABLE: &str = "unavailable";

/// `delivery` 的两个取值；归一化在 `:130`（`delivery is "steer" ? "steer" : "queue"`）。
pub const DELIVERY_STEER: &str = "steer";
pub const DELIVERY_QUEUE: &str = "queue";

/// `requestId` 的前缀（`:126` `$"c2-{Guid.NewGuid():N}"`）。与传输层 rpcId 同前缀
/// （`DshRpcClient.cs:95` 的 `$"c2-{id}"`），但**是两颗不同的 id**：这颗由壳现造 GUID。
pub const REQUEST_ID_PREFIX: &str = "c2-";

/// 每一层下级的左边距（`:375` 与 `:404` 两处 `new Thickness(depth * 16, 0, 0, 0)`）。
pub const INDENT_PER_DEPTH: f64 = 16.0;

/// 副行的分隔符（`:402` `string.Join(" · ", …)`）。
pub const SECONDARY_SEPARATOR: &str = " · ";

// ---- 主干 `L()` 的中文键原样（EN 表已有，本模块零新增；宿主拿去查表）----

/// `:396` 的 `L("可继续")`。
pub const ZH_MODE_CONTINUABLE: &str = "可继续";
/// `:396` 的另一臂 —— **非 continuable 全落这儿**。
pub const ZH_MODE_ONE_SHOT: &str = "一次性";
/// `:397` 的 `L("正在运行")`。
pub const ZH_ACTIVITY_RUNNING: &str = "正在运行";
/// `:397` 的另一臂 —— 非 running 全落这儿。
pub const ZH_ACTIVITY_INACTIVE: &str = "当前未运行";
/// `:402` 副行第三段的冠词：`L("父会话") + " " + parentText`。
pub const ZH_PARENT_SESSION: &str = "父会话";
/// `:400` 既是**比较字面量**又是文案键（同串两用）：父会话标题正好是「新会话」时改走 `L()`。
pub const ZH_NEW_SESSION: &str = "新会话";
/// `:370`/`:371`/`:372` diagnostic 的三条 reason 文案。
pub const ZH_REASON_CORRUPT: &str = "会话记录损坏";
pub const ZH_REASON_UNSUPPORTED: &str = "子代理记录版本不受支持";
pub const ZH_REASON_UNAVAILABLE: &str = "会话记录暂不可用";
/// `:511`/`:515` 展开钮的两态文案（同一颗钮换字）。
pub const ZH_EXPAND: &str = "展开";
pub const ZH_COLLAPSE: &str = "收起";

// ---------------------------------------------------------------- 解析失败半径

/// [`parse_catalog`] 唯一的失败臂 = 主干那一次未捕获的 `InvalidOperationException` 的半径。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// `entries` 的某个元素不是对象 ⇒ 主干对它调 `TryGetProperty`（`:87`）就抛 ⇒
    /// **整次列目录作废**（`:69` 的 `foreach` 没有 per-item 的 try；调用点在 `:304`/`:540`
    /// 兜成错误行 + 重试钮）。
    EntryNotObject,
}

// ---------------------------------------------------------------- 条目模型（:23-39）

/// `subagents/list` 的一条目：child 或 diagnostic（`:23-39` 的 `SubagentEntryVm`）。
///
/// 字段默认值照主干的**类型默认**（`:25-38`）：`kind` 缺省 `"child"`、`activity` 缺省
/// `"inactive"`、其余串缺省空 —— 于是 diagnostic 支（`:91-97` 只点四枚字段）的
/// `mode`/`label` 是 `""`、`activity` 是 `"inactive"`，不是 `null`。分叉不得把 `mode` 折成 enum：
/// 主干存字符串、只跟 `"continuable"` 比，折了就比主干更严。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubagentEntry {
    /// `id`：child 是子会话 id，diagnostic 是坏记录的身份串。缺键/非字符串 ⇒ `""`（`:88`）。
    pub id: String,
    /// `kind`（`:87`）。
    pub kind: String,
    /// `mode` —— 仅 child 支读（`:103`）；diagnostic 恒 `""`。
    pub mode: String,
    /// `label` —— 仅 child 支读（`:104`）。
    pub label: String,
    /// `activity`（`:105`），回落 `"inactive"`。
    pub activity: String,
    /// `hasChildren`（`:106`）：**只认字面 `true`**。
    pub has_children: bool,
    /// `diagnostic.reason`（`:95`），三条已知值见 [`REASON_CORRUPT`] 一族。
    pub reason: String,
    /// 不是回执字段：回填自**本次调用的 `parentSessionId`**（`:96`/`:107`）。
    pub parent_session_id: String,
    /// 不是回执字段：`session/list` 里同 id 那行的标题（`:75-81`）；缺失 ⇒ `""`，
    /// 显示串再走 [`SubagentEntry::display_title`]。
    pub title: String,
}

impl Default for SubagentEntry {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: KIND_CHILD.to_string(),
            mode: String::new(),
            label: String::new(),
            activity: ACTIVITY_INACTIVE.to_string(),
            has_children: false,
            reason: String::new(),
            parent_session_id: String::new(),
            title: String::new(),
        }
    }
}

impl SubagentEntry {
    /// `:89`/`:366` 的 `Kind == "diagnostic"`。
    pub fn is_diagnostic(&self) -> bool {
        self.kind == KIND_DIAGNOSTIC
    }

    /// `:186` 的 `e.Kind == "child"`。主干**没有**第三型：未知 `kind` 字面量在 `:87` 就被归成
    /// `"child"`，所以这条与 [`Self::is_diagnostic`] 互斥且穷尽。
    pub fn is_child(&self) -> bool {
        self.kind == KIND_CHILD
    }

    /// **可续跑判据**（`:193`/`:396`/`:453`/`:476` 四处全用这一形态）：只认 `Mode == "continuable"`。
    pub fn is_continuable(&self) -> bool {
        self.mode == MODE_CONTINUABLE
    }

    /// `:194`/`:397`/`:414`/`:476` 的活动位判据：只认 `"running"`，其余一律「当前未运行」。
    pub fn is_running(&self) -> bool {
        self.activity == ACTIVITY_RUNNING
    }

    /// 「续跑」钮的显据（`:195`、`:453`）= [`Self::is_continuable`]，与活动位**无关**。
    pub fn can_prompt(&self) -> bool {
        self.is_continuable()
    }

    /// 「打断」钮的显据（`:196`、`:476`）：`Mode == "continuable" || Activity == "running"`。
    /// 语义现测于 `:474-475` 的注释：continuable 常显（idle 是**接受型 no-op**，`:138`），
    /// one-shot **仅 running 时**显（一次性任务中途才需要打断）。
    pub fn can_interrupt(&self) -> bool {
        self.is_continuable() || self.is_running()
    }

    /// 出「展开」钮的条件（`:497`）—— 主干只看这一位，不预测下级数量。
    pub fn shows_expand(&self) -> bool {
        self.has_children
    }

    /// 标题三级回落（`:393-395`，同形态再用于 `:575-577`）：`Title` → `Label` → `Id`。
    pub fn display_title(&self) -> &str {
        if !self.title.is_empty() {
            &self.title
        } else if !self.label.is_empty() {
            &self.label
        } else {
            &self.id
        }
    }
}

// ---------------------------------------------------------------- 目录 + 解析

/// `subagents/list` 的结果（`:42-46` 的 `SubagentCatalogVm`）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SubagentCatalog {
    /// **追加序**，主干全程不排序（现测这颗文件 `.OrderBy`/`.ThenBy`/`.Sort(`/`Reverse(`/`GroupBy`
    /// 各 0 次），所以本模块也不提供排序：排序就是「发明」。见 `lock_no_sorting_no_grouping`。
    pub entries: Vec<SubagentEntry>,
    /// 现测全仓只出现 2 次（`:45` 声明、`:66` 赋值），**从未被读** ⇒ 分叉不得拿它当任何判据，
    /// 只照解析口径存下来（`:66` 只认字面 `true`）。
    pub parent_available: bool,
}

/// 主干 `:57` 的前置门禁：`parentSessionId` 为空 ⇒ **不发**、直接给空目录。
/// （主干那句还带 `_rpc is null`，那是派发面的事。）
pub fn list_scope_allowed(parent_session_id: &str) -> bool {
    !parent_session_id.is_empty()
}

/// 「取字符串字段」= 主干 `TryGetProperty(k, out v) && v.ValueKind == String ? v.GetString() ?? d : d`。
/// 缺键 / JSON `null` / **任何非字符串** ⇒ 回落 `d`（主干在每个字段上都先设了 `ValueKind` 门禁
/// ⇒ **静默**，与 `filespanel` 那族「非字符串就抛」的读法方向相反；照主干，不改仁慈）。
fn str_or(obj: &Value, key: &str, fallback: &str) -> String {
    match obj.get(key) {
        Some(Value::String(text)) => text.clone(),
        _ => fallback.to_string(),
    }
}

/// 「取真值字段」= 主干 `TryGetProperty(k, out v) && v.ValueKind == JsonValueKind.True`
/// （`:66`/`:106` 两处同形）：缺键、`false`、`1`、`"true"` 一律 false —— **只认字面 `true`**。
fn is_true(obj: &Value, key: &str) -> bool {
    matches!(obj.get(key), Some(Value::Bool(true)))
}

/// 一条 `entries` 元素 → [`SubagentEntry`]（`:85-109` 的 `ParseSubagentEntry`）。
///
/// 逐字段回落方向（现测）：
/// * `kind` 非串 ⇒ `"child"`（`:87`）；`id` 非串 ⇒ `""`（`:88`）
/// * `kind == "diagnostic"` ⇒ 早退支（`:89-98`）：只带 `id`/`kind`/`reason`/`parentSessionId`，
///   **`mode`/`label`/`activity`/`hasChildren` 不读回执**、保持类型默认
/// * child 支（`:99-108`）：`mode` 非串 ⇒ `""`（`:103`，于是 [`SubagentEntry::is_continuable`] 为假）、
///   `label` 非串 ⇒ `""`、`activity` 非串 ⇒ `"inactive"`（`:105`）、`hasChildren` 只认字面 `true`（`:106`）
///
/// 唯一的 `Err`：元素本身不是对象（`:87` 的 `item.TryGetProperty` 抛）⇒ 整次列目录作废。
pub fn parse_entry(item: &Value, parent_session_id: &str) -> Result<SubagentEntry, ParseError> {
    if !item.is_object() {
        return Err(ParseError::EntryNotObject);
    }
    let kind = str_or(item, "kind", KIND_CHILD);
    let id = str_or(item, "id", "");
    if kind == KIND_DIAGNOSTIC {
        return Ok(SubagentEntry {
            id,
            kind: KIND_DIAGNOSTIC.to_string(),
            reason: str_or(item, "reason", ""),
            parent_session_id: parent_session_id.to_string(),
            ..Default::default()
        });
    }
    Ok(SubagentEntry {
        id,
        kind: KIND_CHILD.to_string(),
        mode: str_or(item, "mode", ""),
        label: str_or(item, "label", ""),
        activity: str_or(item, "activity", ACTIVITY_INACTIVE),
        has_children: is_true(item, "hasChildren"),
        parent_session_id: parent_session_id.to_string(),
        ..Default::default()
    })
}

/// `subagents/list` 的 `value` 层 → [`SubagentCatalog`]（`:62-73`，不含 `:74-81` 的标题补齐）。
///
/// 三条主干回落，方向各不相同（**都不算 `Err`**）：
/// * `value` 不是对象（含 `CallOkAsync` 缺 `value` 键时给的 `Undefined`、`null`、数组、标量）
///   ⇒ **空目录**（`:62-65`）
/// * `entries` 缺键 / 不是数组 ⇒ 只剩 `parentAvailable`（`:67`）
/// * `parentAvailable` 只认字面 `true`（`:66`）
pub fn parse_catalog(value: &Value, parent_session_id: &str) -> Result<SubagentCatalog, ParseError> {
    let mut catalog = SubagentCatalog::default();
    if !value.is_object() {
        return Ok(catalog);
    }
    catalog.parent_available = is_true(value, "parentAvailable");
    if let Some(items) = value.get("entries").and_then(Value::as_array) {
        for item in items {
            catalog.entries.push(parse_entry(item, parent_session_id)?);
        }
    }
    Ok(catalog)
}

/// 标题补齐（`:74-81`）：逐条拿 `entry.Id` 去会话清单找**第一行**（主干 `FirstOrDefault`），
/// 找到就覆盖 `Title`。
///
/// 现测两条容易抄错的语义：① 循环覆盖**全部**条目，包括 diagnostic（`:75` 没有 kind 门禁），
/// 所以坏记录只要 id 撞上清单某行也会有标题；② `lookup` 返回 `None` 时 **`Title` 保持原值**
/// （主干在 `is { } vm` 不成立时不进赋值体），不是清空。
///
/// `lookup` 返回 `Option<String>` 而不是 `Option<&str>`：纯为签名诚实 —— 标题在主干是
/// `SessionVm.Title`（一个可变串），本模块不该对宿主的借用期做任何假设。
pub fn backfill_titles(entries: &mut [SubagentEntry], mut lookup: impl FnMut(&str) -> Option<String>) {
    for entry in entries.iter_mut() {
        if let Some(title) = lookup(&entry.id) {
            entry.title = title;
        }
    }
}

// ---------------------------------------------------------------- 会话头动作钮（:156-203 的纯半刀）

/// 会话头那两枚钮的显隐结果（`:195-196`）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeaderActions {
    pub prompt: bool,
    pub interrupt: bool,
}

impl HeaderActions {
    /// 主干 `:166-167` 在每次刷新开头**无条件**把两枚钮都收起来 ⇒ 门禁不过 / 目录里找不到自己
    /// / 目录加载失败（`:199-202` 吞异常）时交回的都是这一态。
    pub const HIDDEN: HeaderActions = HeaderActions {
        prompt: false,
        interrupt: false,
    };
}

/// 值不值得去查「自己在父目录里的那一条」（`:168` 的
/// `vm is not { IsSubagent: true, ParentSessionId: { Length: > 0 } }`）。
/// 同一形态在三枚 Click 里各用一次（`:168`、`:229`、`:246`）。
pub fn self_actions_gate(is_subagent: bool, parent_session_id: &str) -> bool {
    is_subagent && !parent_session_id.is_empty()
}

/// `:186` 的 `catalog.Entries.FirstOrDefault(e => e.Kind == "child" && e.Id == childId)`。
/// 只看**直接子代**那一层（嵌套层在 `:521-537` 另起一次目录，不参与会话头判定）。
pub fn find_self_entry<'a>(catalog: &'a SubagentCatalog, child_id: &str) -> Option<&'a SubagentEntry> {
    catalog
        .entries
        .iter()
        .find(|entry| entry.is_child() && entry.id == child_id)
}

/// 会话头两枚钮的显据（`:193-196`）：`continuable ⇒ 续跑`；`continuable || running ⇒ 打断`。
/// `None`（目录里没自己这一条，或整次目录加载失败）⇒ [`HeaderActions::HIDDEN`]。
pub fn header_actions(entry: Option<&SubagentEntry>) -> HeaderActions {
    match entry {
        Some(entry) => HeaderActions {
            prompt: entry.can_prompt(),
            interrupt: entry.can_interrupt(),
        },
        None => HeaderActions::HIDDEN,
    }
}

// ---------------------------------------------------------------- 文案选键（只选，不落表）

/// `:396` 的 `entry.Mode == "continuable" ? L("可继续") : L("一次性")` —— **非 continuable 全落一次性**。
pub fn mode_zh(mode: &str) -> &'static str {
    if mode == MODE_CONTINUABLE {
        ZH_MODE_CONTINUABLE
    } else {
        ZH_MODE_ONE_SHOT
    }
}

/// `:397` 的 `entry.Activity == "running" ? L("正在运行") : L("当前未运行")`。
pub fn activity_zh(activity: &str) -> &'static str {
    if activity == ACTIVITY_RUNNING {
        ZH_ACTIVITY_RUNNING
    } else {
        ZH_ACTIVITY_INACTIVE
    }
}

/// `:414` 状态点的色键选择（落哪儿是宿主的事；这条**不是文案**，是主题资源键）：
/// running ⇒ `InfoBrush`，其余 ⇒ `SuccessBrush`。diagnostic 支无条件 `ErrorBrush`（`:380`）。
pub fn status_brush_key(activity: &str) -> &'static str {
    if activity == ACTIVITY_RUNNING {
        "InfoBrush"
    } else {
        "SuccessBrush"
    }
}

/// `:368-374` 的 reason switch。**第四臂是原样透传**（`:373` `_ => entry.Reason`），
/// 包括空串 ⇒ 目录行会显示成 `id · `。分叉不得把未知 reason 折成前三臂之一。
pub fn diagnostic_label(reason: &str) -> Cow<'_, str> {
    match reason {
        REASON_CORRUPT => Cow::Borrowed(ZH_REASON_CORRUPT),
        REASON_UNSUPPORTED => Cow::Borrowed(ZH_REASON_UNSUPPORTED),
        REASON_UNAVAILABLE => Cow::Borrowed(ZH_REASON_UNAVAILABLE),
        other => Cow::Owned(other.to_string()),
    }
}

/// `:398-401` 父会话显示名：清单里有那行 ⇒ 它的标题，但标题正好是 `"新会话"` 时**改走 `L()`**
/// （主干那句是 `parentVm.Title == "新会话" ? L("新会话") : parentVm.Title`）；
/// 清单里没有 ⇒ 回落 `entry.ParentSessionId` 原文（就是那串 id）。
pub fn parent_display(parent_title: Option<&str>, parent_id: &str) -> String {
    match parent_title {
        Some(title) if title == ZH_NEW_SESSION => ZH_NEW_SESSION.to_string(),
        Some(title) => title.to_string(),
        None => parent_id.to_string(),
    }
}

/// `:402` 副行：`string.Join(" · ", new[] { modeText, activityText, L("父会话") + " " + parentText })`。
/// 三段、序固定、第三段内部是「冠词 + 单空格 + 显示名」；四个入参都由宿主查完 `L()` 再递进来。
pub fn secondary_line(mode_text: &str, activity_text: &str, parent_label: &str, parent_display: &str) -> String {
    let parent_part = format!("{parent_label} {parent_display}");
    [mode_text, activity_text, &parent_part].join(SECONDARY_SEPARATOR)
}

/// diagnostic 行的正文（`:385` `entry.Id + " · " + reason`；`:390` UIA 名同料、分隔符换成空格）。
pub fn diagnostic_line_text(id: &str, reason_label: &str) -> String {
    [id, reason_label].join(SECONDARY_SEPARATOR)
}

/// `:375`/`:404` 的 `new Thickness(depth * 16, 0, 0, 0)` —— 左缩进（顶层 `depth = 0`，`:300`/`:342`；
/// 下级 `depth + 1`，`:536`）。返回**左边距**，其余三边主干恒为 0。
pub fn indent_margin(depth: usize) -> f64 {
    depth as f64 * INDENT_PER_DEPTH
}

// ---------------------------------------------------------------- 下级折叠态机（:497-552）

/// `childrenHost` 的内容档位。主干判断「要不要重新发目录」用的是 `childrenHost.Children.Count == 0`
/// （`:516`），而**加载占位（`:518`）、空提示（`:525-530`）、错误提示（`:543-549`）都算「有内容」**
/// ⇒ 一旦渲染过任何一档，收起再展开就**永不重发**（含「加载途中收起」与「失败后收起」）。
/// 这是主干现状，分叉不得「修好」它 —— 见 `lock_expand_cache_quirk`。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NestedContent {
    /// `childrenHost` 还是空的（`:505` 初值：`Visibility.Collapsed` + 零 children）。
    #[default]
    Nothing,
    /// 已塞 `L("正在加载子代理…")`（`:518`）。
    Loading,
    /// 已逐条铺下行（`:534-537`）。
    Rows,
    /// 已铺 `L("当前会话没有子代理")`（`:523-530`）。
    EmptyHint,
    /// 已铺 `LF("无法加载子代理：{0}", ex.Message)`（`:540-549`）。
    ErrorHint,
}

/// [`NestedPanel::click`] 的三种后果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpandClick {
    /// 原本展开 ⇒ 收起，**children 一律不动**（`:508-513` 直接 `return`）。
    Collapse,
    /// 原本收起但已经渲染过 ⇒ 只展开，不发目录（`:516` 的 `Count == 0` 不成立）。
    ExpandFromCache,
    /// 原本收起且从未渲染 ⇒ 展开并发 `subagents/list(child 自己的 id)`（`:517-521`）。
    ExpandAndLoad,
}

/// 一颗「展开/收起」的纯状态位组（`:497-552` 的态机；派发与 children 容器归宿主）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NestedPanel {
    /// 对应 `childrenHost.Visibility`（`:505` 初值 Collapsed）。
    pub expanded: bool,
    /// 对应 `childrenHost.Children.Count == 0`（`:516`）。
    pub content: NestedContent,
}

impl NestedPanel {
    /// 初态 = 收起 + 从未渲染（`:505`）。
    pub const fn new() -> Self {
        Self {
            expanded: false,
            content: NestedContent::Nothing,
        }
    }

    /// 点一下这颗钮（`:506-552`）：先换 `expanded`，再决定要不要发目录。
    /// 走 [`ExpandClick::ExpandAndLoad`] 时顺手把 `content` 推到 `Loading`（`:518` 那句占位）。
    pub fn click(&mut self) -> ExpandClick {
        if self.expanded {
            self.expanded = false;
            return ExpandClick::Collapse;
        }
        self.expanded = true;
        if self.content == NestedContent::Nothing {
            self.content = NestedContent::Loading;
            ExpandClick::ExpandAndLoad
        } else {
            ExpandClick::ExpandFromCache
        }
    }

    /// 展开钮的字面（同一颗钮换字：`:511` `L("展开")` / `:515` `L("收起")`）。
    /// 语义随**点击之后**的 `expanded`，所以在 [`Self::click`] 之后调。
    pub fn label_zh(&self) -> &'static str {
        if self.expanded {
            ZH_COLLAPSE
        } else {
            ZH_EXPAND
        }
    }
}

// ---------------------------------------------------------------- 三发的实参形状（不建 RpcCall）

/// `subagents/prompt` 的 request 体（`:124-132`）。
///
/// `request_id` 由**调用方现造**：主干是 `$"c2-{Guid.NewGuid():N}"`（`c2-` + 32 位小写十六进制），
/// 分叉的熵源归 ctor/宿主那颗卡，本模块不碰随机数（也不许为此加依赖）。
/// `delivery` 走 [`normalize_delivery`]；`text` 原样进 `content[0].text` ——
/// 主干的 `Trim()` 与空串早退在 `:617-621`（对话框那一层，不在本模块）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PromptRequest<'a> {
    pub request_id: &'a str,
    pub parent_session_id: &'a str,
    pub child_session_id: &'a str,
    /// 原始值的来头（`:622`）：`NsString("ui-conversation", "busyEnter", "queue")` —— 读盘归宿主。
    pub delivery: &'a str,
    pub text: &'a str,
}

/// `:130` 的 `delivery is "steer" ? "steer" : "queue"` —— **非 `"steer"` 全落 `queue`**
/// （包括 `"queue"`、`"Steer"`、`"steer "`、空串）。
pub fn normalize_delivery(raw: &str) -> &'static str {
    if raw == DELIVERY_STEER {
        DELIVERY_STEER
    } else {
        DELIVERY_QUEUE
    }
}

impl PromptRequest<'_> {
    /// `mode` 恒为 `"continuable"`（`:129`）—— 主干**不看**条目实际的 `Mode`；
    /// one-shot 发过去就是 `:114` 注释里那句「拒收（not-resumable）」。
    pub const MODE: &'static str = MODE_CONTINUABLE;

    /// 出 `payload.args` 那一层：`{ request: { requestId, parentSessionId, childSessionId,
    /// mode, delivery, content:[{type:"text",text}] } }`。
    ///
    /// 形状逐字照 `:122-133`：外层**多一颗 `request`**（`list`/`interruptByParent` 都是平铺，
    /// 见 [`list_args`] 与 [`interrupt_args`]）。`content` 恒一颗元素、恒 `type:"text"`（`:131`）。
    /// 注释 `:113` 提到的 `clientTimeZone` 现测**从未被发出**（那颗字面串在这颗文件里只出现 1 次，
    /// 就是 `:113` 的注释本体）⇒ 本函数不造这颗。
    ///
    /// 注意：`serde_json` 默认 Map 是**按字典序**出键，主干匿名对象按声明序出。JSON 语义无差；
    /// ctor 若要字节一致得自己按声明序拼串 —— 那是 ctor 卡的事，不在本模块（已记入报告 §8）。
    pub fn to_args(&self) -> Value {
        json!({
            "request": {
                "requestId": self.request_id,
                "parentSessionId": self.parent_session_id,
                "childSessionId": self.child_session_id,
                "mode": Self::MODE,
                "delivery": normalize_delivery(self.delivery),
                "content": [ { "type": "text", "text": self.text } ],
            }
        })
    }
}

/// `subagents/interruptByParent` 的 `payload.args`（`:146-151`）：
/// `{ childSessionId, parentSessionId, mode:"continuable" }` —— **平铺、无外层包装**。
///
/// 现测两条：① 参数**顺序**是 child 在前、parent 在后（调用点 `:655` 也是
/// `SubagentsInterruptAsync(childSessionId, parentSessionId)`，与 `prompt` 那发的
/// `parent, child` **相反**，抄错就是打断错对象）；② `mode` 无条件写死 `"continuable"`（`:150`），
/// one-shot 跑中途也发这个值 —— 主干不分支，本函数也不分支。
pub fn interrupt_args(child_session_id: &str, parent_session_id: &str) -> Value {
    json!({
        "childSessionId": child_session_id,
        "parentSessionId": parent_session_id,
        "mode": MODE_CONTINUABLE,
    })
}

/// `subagents/list` 的 `payload.args`（`:61`）：`{ parentSessionId }`，一颗、平铺。
pub fn list_args(parent_session_id: &str) -> Value {
    json!({ "parentSessionId": parent_session_id })
}

// ---------------------------------------------------------------- 自检锁 + 单测

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------- fixture（手写 json!，形状照 §1.3 现测的 `value` 层） ----------------

    /// 一条 child + 一条 diagnostic 的完整回执（形状抄 `:52` 的头注与 `:87-107` 的读取字段）。
    fn sample_value() -> Value {
        json!({
            "parentAvailable": true,
            "entries": [
                { "kind": "child", "id": "c-1", "activity": "running", "hasChildren": true,
                  "mode": "continuable", "label": "查资料" },
                { "kind": "diagnostic", "id": "c-2", "reason": "corrupt" }
            ]
        })
    }

    // ---------------- 解析 ----------------

    #[test]
    fn cw1_parse_catalog_reads_two_entry_kinds() {
        let catalog = parse_catalog(&sample_value(), "p-1").expect("对象回执不该失败");
        assert!(catalog.parent_available);
        assert_eq!(catalog.entries.len(), 2);
        let child = &catalog.entries[0];
        assert_eq!((child.id.as_str(), child.kind.as_str()), ("c-1", KIND_CHILD));
        assert_eq!(child.mode, MODE_CONTINUABLE);
        assert_eq!(child.activity, ACTIVITY_RUNNING);
        assert!(child.has_children);
        assert_eq!(child.label, "查资料");
        assert_eq!(child.parent_session_id, "p-1");
        let diag = &catalog.entries[1];
        assert!(diag.is_diagnostic());
        assert_eq!(diag.reason, REASON_CORRUPT);
        // diagnostic 支不读 mode/label/activity/hasChildren ⇒ 类型默认（:91-97）
        assert_eq!(diag.mode, "");
        assert_eq!(diag.label, "");
        assert_eq!(diag.activity, ACTIVITY_INACTIVE);
        assert!(!diag.has_children);
    }

    #[test]
    fn cw1_parse_catalog_keeps_arrival_order() {
        let value = json!({ "entries": [
            { "id": "z" }, { "id": "a" }, { "kind": "diagnostic", "id": "m", "reason": "x" }
        ]});
        let catalog = parse_catalog(&value, "p").unwrap();
        let ids: Vec<&str> = catalog.entries.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["z", "a", "m"], "主干是追加序（:69-72），任何重排都是发明");
    }

    #[test]
    fn cw1_parse_catalog_non_object_is_empty_not_error() {
        // :62-65：非对象 ⇒ 空目录。CallOkAsync 缺 value 键那档就是 Undefined。
        for value in [Value::Null, json!([]), json!("x"), json!(1), json!(true)] {
            let catalog = parse_catalog(&value, "p").expect("非对象不是错误，是空目录");
            assert_eq!(catalog, SubagentCatalog::default(), "{value}");
        }
    }

    #[test]
    fn cw1_parse_catalog_parent_available_accepts_only_literal_true() {
        // :66：TryGetProperty && ValueKind == True
        for (raw, want) in [
            (json!(true), true),
            (json!(false), false),
            (json!(1), false),
            (json!("true"), false),
            (json!(null), false),
        ] {
            let value = json!({ "parentAvailable": raw });
            assert_eq!(parse_catalog(&value, "p").unwrap().parent_available, want, "{raw}");
        }
        // entries 缺键 / 非数组 ⇒ 只剩 parentAvailable（:67）
        assert!(parse_catalog(&json!({ "parentAvailable": true }), "p").unwrap().entries.is_empty());
        assert!(parse_catalog(&json!({ "entries": {} }), "p").unwrap().entries.is_empty());
        assert!(parse_catalog(&json!({ "entries": "x" }), "p").unwrap().entries.is_empty());
    }

    #[test]
    fn cw1_parse_entry_field_fallbacks_are_silent() {
        // **非字符串**才是回落臂（主干每个字段先设 ValueKind 门禁）；已知之外的**字符串原样存**。
        let entry = parse_entry(
            &json!({ "kind": 7, "id": 8, "mode": true, "label": null, "activity": 9,
                     "hasChildren": "yes" }),
            "p",
        )
        .unwrap();
        assert_eq!(entry.kind, KIND_CHILD, "非串 kind ⇒ \"child\"（:87）");
        assert_eq!(entry.id, "", "非串 id ⇒ \"\"（:88）");
        assert_eq!(entry.mode, "", "非串 mode ⇒ \"\"（:103）⇒ 判为一次性");
        assert_eq!(entry.label, "");
        assert_eq!(entry.activity, ACTIVITY_INACTIVE, "非串 activity ⇒ inactive（:105）");
        assert!(!entry.has_children, "hasChildren 只认字面 true（:106）");
        assert!(!entry.is_continuable());
        assert!(!entry.is_running());
        assert!(entry.is_child(), "未知 kind 在 :87 就被归成 child ⇒ 两支互斥穷尽");
        // 反方向：未知**字符串**照单全收（主干不校验字面集合），只在判据上落非 running
        let raw = parse_entry(&json!({ "activity": "nope", "mode": "nope", "kind": "nope" }), "p").unwrap();
        assert_eq!((raw.activity.as_str(), raw.mode.as_str()), ("nope", "nope"));
        assert_eq!(raw.kind, KIND_CHILD, "kind 是字符串但不是 diagnostic ⇒ 照样归 child（:102）");
        assert!(!raw.is_running() && !raw.is_continuable());
        assert_eq!(activity_zh(&raw.activity), ZH_ACTIVITY_INACTIVE);
        assert_eq!(mode_zh(&raw.mode), ZH_MODE_ONE_SHOT);
        // 缺键与 JSON null 同臂（都过不了 ValueKind == String）
        let missing = parse_entry(&json!({ "id": "c" }), "p").unwrap();
        assert_eq!(missing.activity, ACTIVITY_INACTIVE);
        assert_eq!(missing.mode, "");
    }

    #[test]
    fn cw1_parse_entry_non_object_item_voids_the_whole_load() {
        // :87 的 item.TryGetProperty 对非对象抛 ⇒ SubagentsListAsync 无 try ⇒ 整次作废
        for item in [json!("x"), json!(7), json!(null), json!([]), json!(true)] {
            assert_eq!(parse_entry(&item, "p"), Err(ParseError::EntryNotObject), "{item}");
        }
        let value = json!({ "entries": [ { "id": "ok" }, "boom" ] });
        let err = parse_catalog(&value, "p").expect_err("第二条不是对象 ⇒ 整次失败");
        assert_eq!(err, ParseError::EntryNotObject);
    }

    #[test]
    fn cw1_backfill_titles_first_match_wins_and_covers_diagnostic() {
        let mut catalog = parse_catalog(&sample_value(), "p-1").unwrap();
        // :77 FirstOrDefault ⇒ 同 id 多行时取第一行
        let sessions = [("c-1", "首个标题"), ("c-1", "第二个同名行"), ("c-2", "坏记录行")];
        let find = |id: &str| {
            sessions
                .iter()
                .find(|(s, _)| *s == id)
                .map(|(_, t)| (*t).to_string())
        };
        backfill_titles(&mut catalog.entries, find);
        assert_eq!(catalog.entries[0].title, "首个标题");
        assert_eq!(catalog.entries[1].title, "坏记录行", "diagnostic 也在补齐循环里（:75 无 kind 门禁）");
        // 未命中 ⇒ 保持原值，不是清空
        catalog.entries[0].title = "自定义".to_string();
        backfill_titles(&mut catalog.entries[0..1], |_| None);
        assert_eq!(catalog.entries[0].title, "自定义");
    }

    #[test]
    fn cw1_display_title_three_level_fallback() {
        // :393-395
        let base = || parse_entry(&json!({ "id": "c-1" }), "p").unwrap();
        assert_eq!(base().display_title(), "c-1");
        let mut by_label = base();
        by_label.label = "标签".to_string();
        assert_eq!(by_label.display_title(), "标签", "title 空 ⇒ 看 label，不是看 id");
        let mut by_title = by_label.clone();
        by_title.title = "会话标题".to_string();
        assert_eq!(by_title.display_title(), "会话标题");
    }

    // ---------------- 判据 ----------------

    #[test]
    fn cw1_continuable_is_the_only_mode_criterion() {
        let mut entry = parse_entry(&json!({ "id": "c", "mode": MODE_CONTINUABLE }), "p").unwrap();
        assert!(entry.can_prompt() && entry.can_interrupt());
        entry.activity = ACTIVITY_INACTIVE.to_string();
        assert!(entry.can_interrupt(), "continuable 常显（idle 是接受型 no-op，:474）");
        for mode in ["one-shot", "", "Continuable", "continuable ", "queued"] {
            entry.mode = mode.to_string();
            assert!(!entry.can_prompt(), "{mode} 不是 continuable ⇒ 无续跑钮");
            entry.activity = ACTIVITY_INACTIVE.to_string();
            assert!(!entry.can_interrupt(), "{mode} + 非 running ⇒ 无打断钮");
            entry.activity = ACTIVITY_RUNNING.to_string();
            assert!(entry.can_interrupt(), "{mode} + running ⇒ 一次性任务中途可打断（:475-476）");
        }
    }

    #[test]
    fn cw1_header_actions_gate_and_lookup() {
        assert!(self_actions_gate(true, "p-1"));
        assert!(!self_actions_gate(false, "p-1"), "origin != subagent ⇒ 不查（:168）");
        assert!(!self_actions_gate(true, ""), "parentSessionId 空串 ⇒ 不查（:168）");
        let catalog = parse_catalog(&sample_value(), "p-1").unwrap();
        let hit = find_self_entry(&catalog, "c-1").expect("直接子代里有这一条");
        assert_eq!(
            header_actions(Some(hit)),
            HeaderActions { prompt: true, interrupt: true }
        );
        assert!(find_self_entry(&catalog, "c-2").is_none(), "diagnostic 不参与会话头判定（:186）");
        assert_eq!(header_actions(find_self_entry(&catalog, "ghost")), HeaderActions::HIDDEN);
    }

    #[test]
    fn cw1_show_expand_uses_has_children_only() {
        let yes = parse_entry(&json!({ "id": "a", "hasChildren": true }), "p").unwrap();
        let no = parse_entry(&json!({ "id": "b", "hasChildren": false }), "p").unwrap();
        assert!(yes.shows_expand() && !no.shows_expand());
    }

    // ---------------- 文案选键 ----------------

    #[test]
    fn cw1_text_key_selection() {
        assert_eq!(mode_zh(MODE_CONTINUABLE), ZH_MODE_CONTINUABLE);
        assert_eq!(mode_zh("one-shot"), ZH_MODE_ONE_SHOT);
        assert_eq!(mode_zh(""), ZH_MODE_ONE_SHOT, "mode 缺键也落一次性（:396 的非真臂）");
        assert_eq!(activity_zh(ACTIVITY_RUNNING), ZH_ACTIVITY_RUNNING);
        assert_eq!(activity_zh("idle"), ZH_ACTIVITY_INACTIVE);
        assert_eq!(status_brush_key(ACTIVITY_RUNNING), "InfoBrush");
        assert_eq!(status_brush_key("inactive"), "SuccessBrush");
        assert_eq!(diagnostic_label(REASON_CORRUPT), Cow::Borrowed(ZH_REASON_CORRUPT));
        assert_eq!(diagnostic_label(REASON_UNSUPPORTED), Cow::Borrowed(ZH_REASON_UNSUPPORTED));
        assert_eq!(diagnostic_label(REASON_UNAVAILABLE), Cow::Borrowed(ZH_REASON_UNAVAILABLE));
        // :373 的 _ => entry.Reason 是原样透传，包括空串与大小写变体
        assert_eq!(diagnostic_label("Corrupt"), Cow::Borrowed("Corrupt"));
        assert_eq!(diagnostic_label(""), Cow::Borrowed(""));
        assert_eq!(
            secondary_line(ZH_MODE_ONE_SHOT, ZH_ACTIVITY_RUNNING, ZH_PARENT_SESSION, "p-1"),
            "一次性 · 正在运行 · 父会话 p-1"
        );
        assert_eq!(
            diagnostic_line_text("c-2", ZH_REASON_CORRUPT),
            format!("c-2{SECONDARY_SEPARATOR}会话记录损坏")
        );
    }

    #[test]
    fn cw1_parent_display_new_session_and_missing_row() {
        // :400 那颗「新会话」是**比较字面量**兼文案键，同串
        assert_eq!(parent_display(Some(ZH_NEW_SESSION), "p-1"), ZH_NEW_SESSION);
        assert_eq!(parent_display(Some("真实标题"), "p-1"), "真实标题");
        assert_eq!(parent_display(None, "p-1"), "p-1", "清单里没这行 ⇒ 回落 id（:401）");
        assert_eq!(parent_display(Some(""), "p-1"), "", "空标题主干不特殊对待");
    }

    #[test]
    fn cw1_indent_per_depth() {
        assert_eq!(indent_margin(0), 0.0);
        assert_eq!(indent_margin(1), INDENT_PER_DEPTH);
        assert_eq!(indent_margin(3), 48.0, ":375/:404 都是 depth * 16");
    }

    // ---------------- 折叠态机 ----------------

    #[test]
    fn cw1_nested_panel_loads_once_and_never_retries() {
        let mut panel = NestedPanel::new();
        assert!(!panel.expanded);
        assert_eq!(panel.label_zh(), ZH_EXPAND);
        assert_eq!(panel.click(), ExpandClick::ExpandAndLoad);
        assert!(panel.expanded);
        assert_eq!(panel.label_zh(), ZH_COLLAPSE);
        assert_eq!(panel.content, NestedContent::Loading, ":518 的占位");
        // 收起 ⇒ 只收，不动 children（:508-513）
        assert_eq!(panel.click(), ExpandClick::Collapse);
        assert!(!panel.expanded);
        assert_eq!(panel.content, NestedContent::Loading, "children 保留 ⇒ 占位也算内容");
        // 再展开 ⇒ 不重发（:516 的 Count == 0 不成立）—— 主干现状，不得"修好"
        assert_eq!(panel.click(), ExpandClick::ExpandFromCache);
        for content in [NestedContent::Rows, NestedContent::EmptyHint, NestedContent::ErrorHint] {
            let mut p = NestedPanel { expanded: true, content };
            assert_eq!(p.click(), ExpandClick::Collapse);
            assert_eq!(p.click(), ExpandClick::ExpandFromCache, "{content:?} 收起后再展开不重发");
        }
    }

    // ---------------- 实参形状 ----------------

    #[test]
    fn cw1_prompt_request_shape() {
        let req = PromptRequest {
            request_id: "c2-0123456789abcdef0123456789abcdef",
            parent_session_id: "p-1",
            child_session_id: "c-1",
            delivery: "steer",
            text: "继续",
        };
        let args = req.to_args();
        assert_eq!(
            args,
            json!({ "request": {
                "requestId": "c2-0123456789abcdef0123456789abcdef",
                "parentSessionId": "p-1",
                "childSessionId": "c-1",
                "mode": "continuable",
                "delivery": "steer",
                "content": [ { "type": "text", "text": "继续" } ],
            }})
        );
        // 外层多一颗 request（与 list/interruptByParent 的平铺相反）
        assert_eq!(args.as_object().map(|m| m.len()), Some(1));
        assert!(args["request"].get("clientTimeZone").is_none(), ":113 注释那颗字段从未被发出");
        assert!(args["request"]["requestId"].as_str().unwrap().starts_with(REQUEST_ID_PREFIX));
        assert_eq!(PromptRequest::MODE, MODE_CONTINUABLE, "mode 恒 continuable（:129）");
    }

    #[test]
    fn cw1_delivery_normalization_is_non_steer_to_queue() {
        assert_eq!(normalize_delivery(DELIVERY_STEER), DELIVERY_STEER);
        for raw in ["queue", "", "Steer", "STEER", "steer ", "whatever"] {
            assert_eq!(normalize_delivery(raw), DELIVERY_QUEUE, "{raw}");
        }
        let req = PromptRequest {
            request_id: "c2-x",
            parent_session_id: "p",
            child_session_id: "c",
            delivery: "Steer",
            text: "t",
        };
        assert_eq!(req.to_args()["request"]["delivery"], json!("queue"));
    }

    #[test]
    fn cw1_interrupt_and_list_args_are_flat_child_first() {
        let args = interrupt_args("c-1", "p-1");
        assert_eq!(
            args,
            json!({ "childSessionId": "c-1", "parentSessionId": "p-1", "mode": "continuable" })
        );
        // 平铺三颗、无包装；mode 无条件写死，不看条目实际 Mode（:150）
        assert_eq!(args.as_object().map(|m| m.len()), Some(3));
        assert_eq!(list_args("p-1"), json!({ "parentSessionId": "p-1" }));
        assert_eq!(list_args(""), json!({ "parentSessionId": "" }), "空串的形状不拦，门禁在 list_scope_allowed");
        assert!(!list_scope_allowed(""));
        assert!(list_scope_allowed("p"));
    }

    // ---------------- 主干源码锁（运行时 std::fs 读；读不到 ⇒ eprintln + skip） ----------------

    const MAINLINE: &str = "MainWindow.Subagents.cs";

    /// 运行时读主干源（**不** `include_str!`）；读不到 ⇒ 打印原因并让调用方 skip（不 panic）。
    /// 顺手把 CRLF 归一成 LF：现测这颗主干文件是纯 LF（`grep -c $'\r'` = 0），归一只是防漂。
    fn mainline() -> Option<String> {
        let path = format!("{}/../{}", env!("CARGO_MANIFEST_DIR"), MAINLINE);
        match std::fs::read_to_string(&path) {
            Ok(text) => Some(text.replace("\r\n", "\n")),
            Err(err) => {
                eprintln!("cw1 读不到主干源 {path}（{err}）—— 本锁跳过");
                None
            }
        }
    }

    /// 紧窗：`start` 首次出现 → 其后 `end` 首次出现。锚缺失 ⇒ `None`，
    /// **不退化**成「到文件末尾」（那正是恒真窗口的来源）。
    fn tight_window(src: &str, start: &str, end: &str) -> Option<String> {
        let at = src.find(start)? + start.len();
        let tail = src[at..].find(end)?;
        Some(src[at - start.len()..at + tail].to_string())
    }

    /// 打印窗口真实行数并判定它「紧」。
    fn report_window(label: &str, text: &str) {
        let lines = text.lines().count();
        eprintln!("cw1 锁[{label}] 窗口行数 = {lines}");
        assert!((1..=60).contains(&lines), "{label} 窗口松了（{lines} 行）");
    }

    /// 锚唯一性：`needle` 在整颗文件里必须出现 `want` 次（防「窗口恰好含它」式假锁）。
    fn assert_hits(src: &str, needle: &str, want: usize, label: &str) {
        let got = src.matches(needle).count();
        eprintln!("cw1 锁[{label}] {needle:?} 命中 {got} 次");
        assert_eq!(got, want, "{label}：{needle:?} 命中数不是现测的 {want}（实得 {got}）");
    }

    #[test]
    fn cw1_lock_entry_model_shape() {
        let Some(src) = mainline() else { return };
        let total = src.lines().count();
        eprintln!("cw1 主干现测总行数 = {total}");
        assert!(total > 600, "主干源读短了（{total} 行）");
        let win = tight_window(&src, "private sealed class SubagentEntryVm", "\n    }").expect("条目模型窗口锚丢失");
        report_window("SubagentEntryVm", &win);
        for (needle, label) in [
            (concat!("public string Id { get; init; } = \"", "\";"), "Id 默认空串"),
            (concat!("public string Kind { get; init; } = \"child", "\";"), "Kind 默认 child"),
            (concat!("public string Mode { get; init; } = \"", "\";"), "Mode 默认空串"),
            (concat!("public string Activity { get; init; } = \"inactive", "\";"), "Activity 默认 inactive"),
            ("public bool HasChildren { get; init; }", "HasChildren 是 bool"),
            ("public string ParentSessionId { get; init; }", "ParentSessionId"),
            ("public string Title { get; set; }", "Title 是可变补齐位"),
            ("public string Reason { get; init; }", "Reason"),
            ("public string Label { get; init; }", "Label"),
        ] {
            assert!(win.contains(needle), "{label} 不在条目模型窗口里：{needle}");
        }
        // 目录壳只有两枚字段（:42-46）
        let cat = tight_window(&src, "private sealed class SubagentCatalogVm", "\n    }").expect("目录壳窗口");
        report_window("SubagentCatalogVm", &cat);
        assert!(cat.contains(concat!("List<SubagentEntryVm> Entries { get; } = new", "();")));
        assert!(cat.contains("public bool ParentAvailable { get; set; }"));
        // ParentAvailable 全仓从未被读 ⇒ 本模块不得拿它当判据
        assert_hits(&src, "ParentAvailable", 2, "声明 + 赋值两处，无第三处");
    }

    #[test]
    fn cw1_lock_no_sorting_no_grouping() {
        let Some(src) = mainline() else { return };
        // 反发明锁：主干没有排序、没有分组 ⇒ 本模块不提供；将来主干加了就得改这条
        for (needle, label) in [
            (concat!(".", "OrderBy"), "OrderBy"),
            (concat!(".", "ThenBy"), "ThenBy"),
            (concat!(".", "Group", "By"), "GroupBy"),
            (concat!(".", "Sort", "("), "Sort"),
            (concat!("Reverse", "("), "Reverse"),
        ] {
            assert_eq!(src.matches(needle).count(), 0, "主干新增了 {label} ⇒ 分叉要重估：{needle}");
        }
        let win = tight_window(&src, "foreach (var item in entries.EnumerateArray())", "\n            }")
            .expect("追加序窗口锚丢失");
        report_window("追加序", &win);
        assert!(
            win.contains("catalog.Entries.Add(ParseSubagentEntry(item, parentSessionId));"),
            "条目不再是按回执顺序直接追加：{win}"
        );
        // 渲染面同样零重排：三处 foreach 都是直接扫 Entries（:75 补齐、:298 首屏、:340 重试）
        assert_hits(&src, "foreach (var entry in catalog.Entries)", 3, "补齐 + 首屏 + 重试各一处");
    }

    #[test]
    fn cw1_lock_continuable_is_the_only_mode_criterion() {
        let Some(src) = mainline() else { return };
        assert_hits(&src, "\"one-shot\"", 0, "主干从不把 one-shot 当字面量比较");
        assert_hits(&src, concat!("entry.Mode == ", "\"continuable\""), 3, ":396 文案 / :453 续跑 / :476 打断");
        assert_hits(&src, "entry.Mode ==", 3, "除这三处之外没有别的 Mode 比较");
        assert_hits(&src, concat!("entry is { Mode: \"", "continuable\" }"), 1, ":193 会话头");
        assert_hits(&src, "Mode = item.TryGetProperty", 1, "条目的 Mode 只从回执读一处（:103）");
        for needle in [
            concat!("? L(\"可继续\") : L(\"", "一次性\")"),
            "var running = entry is { Activity: \"running\" };",
            "continuable || running ? Visibility.Visible : Visibility.Collapsed",
        ] {
            assert!(src.contains(needle), "判据形态变了：{needle}");
        }
        let win = tight_window(&src, "if (entry.Mode == \"continuable\")", "\n").expect(":453");
        report_window("目录行续跑钮", &win);
        // 打断那处的主干注释（语义权威）
        let note = tight_window(&src, "// 打断：父会话权威", "打断）。").expect(":474-475 注释");
        report_window("打断注释", &note);
        assert!(note.contains("continuable 常显") && note.contains("one-shot 仅 running 时显"), "{note}");
    }

    #[test]
    fn cw1_lock_prompt_and_interrupt_shapes() {
        let Some(src) = mainline() else { return };
        let prompt = tight_window(&src, "CallOkAsync(\"subagents/prompt\"", "}, ct);").expect(":122");
        report_window("prompt 实参", &prompt);
        for (needle, label) in [
            (concat!("requestId = $\"c2-{Guid.", "NewGuid():N}\""), "requestId 前缀 + GUID(N)"),
            ("mode = \"continuable\",", "mode 恒 continuable"),
            (concat!("delivery is \"steer\" ? \"steer\" : \"", "queue\""), "delivery 归一化"),
            ("content = new object[] { new { type = \"text\", text } },", "content 单元素 text"),
            ("parentSessionId,", "父"),
            ("childSessionId,", "子"),
        ] {
            assert!(prompt.contains(needle), "{label} 不在 prompt 实参窗口里：{needle}");
        }
        // 主干不发 clientTimeZone（:113 注释提到、代码 0 处 ⇒ 本模块不造这颗）
        assert_hits(&src, "clientTimeZone", 1, "只在 :113 的注释里");
        assert!(!prompt.contains("clientTimeZone"));
        // 回执：主干这两发都是 await 后丢弃 ⇒ 分叉不得发明回执解析
        assert_hits(&src, "= await _rpc.CallOkAsync(\"subagents/prompt\"", 0, "prompt 无赋值");
        assert_hits(&src, "= await _rpc.CallOkAsync(\"subagents/interruptByParent\"", 0, "interrupt 无赋值");
        let interrupt =
            tight_window(&src, "CallOkAsync(\"subagents/interruptByParent\"", "}, ct);").expect(":146");
        report_window("interrupt 实参", &interrupt);
        for needle in ["childSessionId,", "parentSessionId,", "mode = \"continuable\","] {
            assert!(interrupt.contains(needle), "interrupt 实参少了 {needle}");
        }
        assert!(!interrupt.contains("entry.Mode"), "mode 是写死的，不看条目实际 Mode（:150）");
        // 参数顺序相反的现测证据：prompt 是 (parent, child)，interrupt 是 (child, parent)
        assert!(src.contains("SubagentsPromptAsync(string parentSessionId, string childSessionId"));
        assert!(src.contains("SubagentsInterruptAsync(string childSessionId, string parentSessionId"));
        // list 形状（`CallOkAsync("subagents/list"` 的实参窗 + `new { parentSessionId }`）**随内核 0.1.7
        // 下线**，本锁不再钉它：主干 `MainWindow.Subagents.cs` 现测该端点活调用点 0（只剩注释），
        // 内核 `Kernel/dsh/*.js` 亦零命中。上面 prompt/interruptByParent 两端的判据逐枚实测仍绿 ⇒ 只摘这一格。
        assert_hits(&src, "mode = \"continuable\"", 2, "prompt + interrupt 各写死一次");
    }

    #[test]
    fn cw1_lock_parse_fallback_directions() {
        let Some(src) = mainline() else { return };
        let win = tight_window(&src, "private SubagentEntryVm ParseSubagentEntry", "\n    }").expect(":85");
        report_window("ParseSubagentEntry", &win);
        for (needle, label) in [
            (concat!("TryGetProperty(\"kind\", out var k) && k.ValueKind"), "kind 先设 ValueKind 门禁"),
            ("? k.GetString() ?? \"child\" : \"child\"", "kind 回落 child"),
            ("?? \"inactive\" : \"inactive\"", "activity 回落 inactive"),
            ("? \"\" : \"\"", "id/mode/label/reason 回落空串"),
            (concat!("TryGetProperty(\"hasChildren\", out var hc) && hc."), "hasChildren"),
        ] {
            assert!(win.contains(needle), "{label} 不在解析窗口里：{needle}");
        }
        // hasChildren / parentAvailable 都只认字面 True（全仓 2 处，无第三处）
        assert_hits(&src, "JsonValueKind.True", 2, "只认字面 true 的两处");
        // diagnostic 早退支只带四枚字段（:89-98）
        let diag = tight_window(&src, "if (kind == \"diagnostic\")", "\n        }").expect(":89");
        report_window("diagnostic 支", &diag);
        assert!(diag.contains("Kind = \"diagnostic\","), "{diag}");
        assert!(!diag.contains("Activity ="), "diagnostic 不读 activity ⇒ 保持类型默认");
        assert!(!diag.contains("Mode ="), "diagnostic 不读 mode ⇒ 保持类型默认");
    }

    #[test]
    fn cw1_lock_title_parent_and_secondary() {
        let Some(src) = mainline() else { return };
        assert_hits(&src, "entry.Title.Length > 0 ? entry.Title", 2, ":393 行标题 + :575 打开会话");
        let win = tight_window(&src, "var modeText = ", "\n        var secondary = ").expect(":396");
        report_window("副行前三段", &win);
        assert!(win.contains("var activityText = entry.Activity == \"running\""));
        assert!(win.contains("var parentVm = _sessions.FirstOrDefault(s => s.SessionId == entry.ParentSessionId)"));
        assert_hits(&src, "parentVm.Title == \"新会话\"", 1, "那颗特判只一处");
        let sec = tight_window(&src, "var secondary = ", ";").expect(":402");
        report_window("副行 join", &sec);
        assert!(
            sec.contains("string.Join(\" · \", new[] { modeText, activityText, L(\"父会话\") + \" \" + parentText })"),
            "副行的段序/分隔符漂了：{sec}"
        );
        assert!(src.contains("_sessions.FirstOrDefault(s => s.SessionId == entry.Id)"), "补齐锚丢失");
        assert!(src.contains("var nested = await SubagentsListAsync(entry.Id)"), "下级换父地址的锚丢失");
    }

    #[test]
    fn cw1_lock_expand_cache_quirk() {
        let Some(src) = mainline() else { return };
        let win = tight_window(&src, "expand.Click += async", "\n            };").expect(":506");
        report_window("展开点击", &win);
        assert_hits(&src, "childrenHost.Children.Count == 0", 1, "唯一的「要不要重发」判据");
        for (needle, label) in [
            ("expand.Content = L(\"展开\");", "收起臂的换字"),
            ("expand.Content = L(\"收起\");", "展开臂的换字"),
            ("Text = L(\"正在加载子代理…\")", "占位也算 children"),
            ("Text = L(\"当前会话没有子代理\")", "空提示"),
            ("BuildSubagentCatalogRow(child, entry.Id, depth + 1)", "下级 depth+1"),
            ("childrenHost.Visibility = Visibility.Collapsed;", "收起臂"),
        ] {
            assert!(win.contains(needle), "{label} 不在展开点击窗口里：{needle}");
        }
        assert!(win.contains("return;"), "收起臂必须早退（:512）");
        assert_hits(&src, "new Thickness(depth * 16, 0, 0, 0)", 2, "两支各一处左缩进");
        assert_hits(&src, "Visibility = Visibility.Collapsed };", 1, "childrenHost 初值 Collapsed");
    }

    #[test]
    fn cw1_lock_header_gate_and_gen_discard() {
        let Some(src) = mainline() else { return };
        assert_hits(&src, "IsSubagent: true, ParentSessionId: { Length: > 0 }", 3, ":168/:229/:246 同一形态");
        let win =
            tight_window(&src, "private async Task LoadSubagentSelfActionsAsync", "\n    }").expect(":177");
        report_window("会话头自查询", &win);
        for needle in [
            "catalog.Entries.FirstOrDefault(e => e.Kind == \"child\" && e.Id == childId)",
            "var continuable = entry is { Mode: \"continuable\" };",
            "SubagentPromptButton.Visibility = continuable ? Visibility.Visible : Visibility.Collapsed;",
            "SubagentInterruptButton.Visibility = continuable || running ? Visibility.Visible : Visibility.Collapsed;",
        ] {
            assert!(win.contains(needle), "会话头判据变了：{needle}");
        }
        assert_hits(&src, "return; // 会话已切换", 1, "代次丢弃只一处（另一处在 PostUi 里）");
        // 两枚钮在刷新开头无条件收起（:166-167）⇒ 本模块的 HIDDEN 就是这个语义
        assert!(src.contains(concat!(
            "SubagentPromptButton.Visibility = Visibility.Collapsed;",
            "\n        SubagentInterruptButton"
        )));
    }

    #[test]
    fn cw1_lock_diagnostic_reason_switch() {
        let Some(src) = mainline() else { return };
        let win = tight_window(&src, "var reason = entry.Reason switch", "\n            };").expect(":368");
        report_window("reason switch", &win);
        for (needle, label) in [
            ("\"corrupt\" => L(\"会话记录损坏\")", "corrupt"),
            ("\"unsupported\" => L(\"子代理记录版本不受支持\")", "unsupported"),
            ("\"unavailable\" => L(\"会话记录暂不可用\")", "unavailable"),
            ("_ => entry.Reason,", "未知 reason 原样透传"),
        ] {
            assert!(win.contains(needle), "{label} 臂变了：{needle}");
        }
    }
}
