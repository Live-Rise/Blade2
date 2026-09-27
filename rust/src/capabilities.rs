//! CP1「目标（Goals）+ 技能账本（Skills）」两族的**纯逻辑层**：只算不画、不发。
//!
//! 落的东西：目标条目模型 + 两副回执面孔的逐字段回落、目标钮的状态机门禁、阶段选键；
//! `skills/list` 回执解析（丢一条 / 作废整次分开）、技能账本的**同名遮蔽**标记。
//! 不落的东西（**刻意为之，不是没做完**）：宿主与派发（在 `main.rs`，别人的地盘）、
//! 两发的 `RpcCall` ctor（在 `kernel.rs`，与 main.rs 永不同批）、任何控件、任何发帧。
//! 本模块零 `use crate::kernel::…`、零 `RpcCall`、零 `crate::i18n::…`。
//!
//! 规格来源 = 本地工作树三颗 partial，**本批现测**的行数 / md5：
//!
//! * `MainWindow.Capabilities.cs` 442 行 / `7402bd599109ff1a519b5f6a1037fc13` —— 两发 RPC 的宿主
//!   确在同一颗（现测 `:199` `goals/get`、`:391` `skills/list`）。
//! * `MainWindow.xaml.cs` 34676 行 / `38c25660ede49fd5f6475353b8488caf`
//!   —— `goals/get` 的**第二副面孔** `ParseGoalSummary`（`:16413`）与阶段文案（`:16467`）。
//! * `MainWindow.Skills.cs` 922 行 / `78f30878481fc07424358d2f7f69a846` —— 遮蔽态的**唯一**出处
//!   （`MarkShadowed` `:531`）。⚠ 它不是 `skills/list` 的下游，见下。
//!
//! 下面注释里的 `:NNN` 都是**本批现测**行号。本仓实测行号必漂，所以 §tests 的源码锁一律用
//! **文本锚**开窗、不用行号（先例 `subagents.rs` / `filespanel.rs`）。
//!
//! ## 入参 = 回执的 `value` 那一层
//!
//! `Dsh/DshRpcClient.cs:110-132`（现测）：`CallOkAsync` ⇒ `CallAsync` 取 HTTP `result`（`:104`）⇒
//! 断言 `result.ok`（`:127`）⇒ **返回 `result.value`**；`value` 键缺失给 `default`，即
//! `JsonValueKind.Undefined`。⇒ [`parse_goal_panel`] / [`parse_goal_summary`] / [`parse_skills_list`]
//! 收的都是 `value` 那一层的 `serde_json::Value`，ctor 落地后原样喂进来即可。
//!
//! ## 目标两副面孔：回落方向不一样，不许合成一副
//!
//! * 面板（[`parse_goal_panel`]，`Capabilities.cs:131-143` + `:204-212`）：容器非对象 ⇒ **抛**；
//!   `revision` / `roundsStarted` / `maxGoalRounds` 三根用 `GetProperty`（**不校验类型**），
//!   **缺任一根 ⇒ 整次作废**；`phase` / `activation` / `objective` 走 `CapabilityString`（`:114-116`
//!   三 AND），缺或类型不符一律留空串。
//! * 目标条（[`parse_goal_summary`]，`xaml.cs:16413-16431`）：容器非对象 ⇒ `null`（收起，绝不抛）；
//!   `phase` 不在 `active|paused|blocked` ⇒ `null` **整条作废**；其余逐字段留空值/0。
//!   但 `roundsStarted` 用的是 `GetInt32()` ⇒ 数字却带小数或超 Int32 时**抛**，
//!   且它**不是** `DshRpcException`，内层 `catch`（`:16394`）拦不住，落到外层 `catch (Exception)`
//!   （`:16404`）⇒ 那一格 [`GoalBarRead::InvalidRoundsStarted`] 的语义是「**摘要不写、保持上一次的值**」，
//!   与 [`GoalBarRead::Absent`]（收起）**不是一回事**，宿主不得合并。
//!
//! ## 技能账本：`skills/list` 里没有遮蔽位
//!
//! `MainWindow.Skills.cs:19-25` 头注明说内核那条 RPC 只回 `isUserInvocable` 的元数据、
//! 既无路径也看不见被关闭的技能，所以「技能」分区**自己扫目录**；遮蔽是
//! `MarkShadowed`（`:531-538`）按 rank 升序 + `HashSet(StringComparer.Ordinal)` 算出来的
//! —— 同名技能只生效 rank 小的那个，**被遮蔽的条目主干照样列出来**（`:195` 只是打个标记），
//! 所以这里也只提供标记、不提供过滤。
//! ⇒ [`mark_shadowed`] 的入参是**扫描结果**（带 rank 的条目序列），而 [`parse_skills_list`]
//! 的输出里**没有** `shadowed` 这一格：分叉不得把两者接成一条链，
//! 也不得从回执里"解析"出一个主干不发的字段。
//!
//! ## 失败半径（与 `subagents` 那族不同）
//!
//! `skills/list`：只有「容器不是对象 / 没有 `skills` 键 / `skills` 不是数组」三档作废整次
//! （`Capabilities.cs:393-394` 抛）。数组元素本身不是对象（`{"skills":["x"]}`）**不抛** ——
//! `CapabilityString` 的 `ValueKind == Object` 门禁先把它变成空串，再命中 `:409` 的 `continue`
//! ⇒ **坏条目丢、坏容器作废**。
//!
//! 主干对这份回执**无排序、无去重、无分组**（`:406` `EnumerateArray()` 原序，现测窗口内 `OrderBy`
//! 命中 0 次）⇒ 本模块一律不加。
//!
//! ## 文案：只选键，不落表
//!
//! 目标阶段五臂的 zh 键（[`ZH_GOAL_STANDBY`] 等）现测**已**在 `rust/src/i18n.rs` 登记
//! （`目标待命` / `目标进行中` / `目标已暂停` / `目标受阻` / `目标` 各命中 1 次），本批**零新增**。
//! 其余文案（`尚未设置目标`、`没有可用技能。`、`技能列表响应格式不正确。`、`共 {0} 个技能` 之类）
//! 都长在宿主那一侧的渲染里，本模块不落串，见报告 §3。

use std::collections::HashSet;

use serde_json::Value;

// ---------------------------------------------------------------- 主干钉死的字面量

/// `phase` 的四取值。前三根是目标条的存活集（`xaml.cs:16420`），第四根 `complete` 只在面板
/// 的钮门禁里当"未完结"判据用（`Capabilities.cs:182/185/187`）。
pub const PHASE_ACTIVE: &str = "active";
pub const PHASE_PAUSED: &str = "paused";
pub const PHASE_BLOCKED: &str = "blocked";
pub const PHASE_COMPLETE: &str = "complete";

/// `activation` 两取值（`Capabilities.cs:183-184`）。目标条那一型**天生没有**这根
/// （`xaml.cs:16410-16412` 注释：`activation` 仅 `goals/get` 有，投影增量里没有）。
pub const ACTIVATION_ARMED: &str = "armed";
pub const ACTIVATION_DISARMED: &str = "disarmed";

/// 目标能力面板的七枚钮（钮表 `Capabilities.cs:274-280`，六发 mutation 打在 `:248`）。
/// 只是**门禁的键名**；发不发、怎么发不在本模块。
pub const VERB_GET: &str = "get";
pub const VERB_CREATE: &str = "create";
pub const VERB_EDIT: &str = "edit";
pub const VERB_PAUSE: &str = "pause";
pub const VERB_RESUME: &str = "resume";
pub const VERB_COMPLETE: &str = "complete";
pub const VERB_CLEAR: &str = "clear";

/// 技能根 rank（`MainWindow.Skills.cs:428/429/443/476/483` 现测）。数字小者胜出。
pub const RANK_PROJECT_DSH: i32 = 100;
pub const RANK_PROJECT_AGENTS: i32 = 200;
pub const RANK_PRESET: i32 = 300;
pub const RANK_USER_DSH: i32 = 400;
pub const RANK_USER_AGENTS: i32 = 500;

/// 阶段五臂的 zh 选键（`xaml.cs:16469-16473`，`L()` 单语键）。值就是主干那五根中文原样，
/// 本批现测它们在分叉 `i18n.rs` 里都已登记 ⇒ 零新增；查表与出串留给宿主。
pub const ZH_GOAL_STANDBY: &str = "目标待命";
pub const ZH_GOAL_IN_PROGRESS: &str = "目标进行中";
pub const ZH_GOAL_PAUSED: &str = "目标已暂停";
pub const ZH_GOAL_BLOCKED: &str = "目标受阻";
pub const ZH_GOAL_PLAIN: &str = "目标";

// ---------------------------------------------------------------- 逐字段读法（主干 `CapabilityString`）

/// `Capabilities.cs:114-116` 的三 AND：是对象 ∧ 有该键 ∧ 值是字符串 ⇒ 串，否则**空串**。
/// 缺键与类型不符**同路**（都是空串，不丢不抛）。`TryGetProperty` 与 serde 的键查都是
/// 序数精确匹配 ⇒ 大小写敏感的键名不折叠。
fn capability_string(value: &Value, key: &str) -> String {
    value
        .as_object()
        .and_then(|map| map.get(key))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// 主干 `GetProperty(key)` 的读法：**只**在键缺失时失败，**完全不校验类型**
/// （`JsonElement.ToString()` / `.Clone()` 都原样带着走）。
fn strict_get<'a>(value: &'a Value, key: &'static str) -> Result<&'a Value, GoalPanelError> {
    value
        .as_object()
        .and_then(|map| map.get(key))
        .ok_or(GoalPanelError::MissingKey(key))
}

// ---------------------------------------------------------------- 目标：条目模型

/// 目标条摘要 —— `xaml.cs:16413` `ParseGoalSummary` 返回的那枚六元组的具名版。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GoalSummary {
    /// 缺键/非串 ⇒ `""`（**保留**，不丢条目）。
    pub id: String,
    /// `Number ? (long)GetDouble() : 0` —— 非数字（含字符串 `"3"`）一律 0；小数**向零截断**。
    pub revision: i64,
    /// 过了 `active|paused|blocked` 门禁才会构造本结构。
    pub phase: String,
    /// `goals/get` 才有；投影增量那一型缺这根 ⇒ `""`。
    pub activation: String,
    pub objective: String,
    /// `Number ? GetInt32() : 0`；非整数值/超 Int32 走 [`GoalBarRead::InvalidRoundsStarted`]。
    pub rounds_started: i32,
}

/// 目标条读 `goals/get` 的三档落点（对应主干「返回 null」/「给出摘要」/「抛」）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalBarRead {
    /// 容器不是对象（含 `Undefined` = 信封缺 `value` 键、Null、数组、串、数字）**或**
    /// `phase` 不在 `active|paused|blocked` ⇒ 主干 `return null` ⇒ 目标条**整条收起**。
    Absent,
    /// 有目标。
    Summary(GoalSummary),
    /// `roundsStarted` 是数字但不是 Int32 装得下的整数 ⇒ 主干 `GetInt32()` 抛 ⇒
    /// 外层 `catch (Exception)`（`xaml.cs:16404`）兜住 ⇒ **`_goalSummary` 不写、保持上一次的值**。
    /// 与 [`GoalBarRead::Absent`] 的区别就是「保持旧值」vs「收起」。
    InvalidRoundsStarted,
}

/// 目标能力面板读到的那一格（`Capabilities.cs:137-143` + `:211-212` 的字段全集）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GoalPanelView {
    pub phase: String,
    pub activation: String,
    pub objective: String,
    /// 显示用的 `id`（`CapabilityString`）：缺键/非串 ⇒ `""`。
    pub id: String,
    /// `id` 这根键**在不在**（`:244` 的 `GetProperty("id")` 只看这个）。
    pub has_id_key: bool,
    /// `id` 键在**且**是字符串。
    pub has_string_id: bool,
    /// `revision` **原样**（主干 `GetProperty(...).Clone()` 不校验类型）。
    pub revision: Value,
    /// `roundsStarted` **原样**（`:139` 直接插值，`.ToString()` 是渲染侧）。
    pub rounds_started: Value,
    /// `maxGoalRounds` **原样**（`:212` `.ToString()`）。
    pub max_goal_rounds: Value,
    /// `Some(..)` ⇔ `blockedReason` **键在**（主干 `TryGetProperty` 命中就追加一行，
    /// 哪怕值不是对象 ⇒ 追的是空行）。`message` 非串 ⇒ `""`。
    pub blocked_reason: Option<GoalBlockedReason>,
}

/// `blockedReason` 里主干**只读** `message`（`:141`）；`code` 一个字都没读 ⇒ 不落。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GoalBlockedReason {
    pub message: String,
}

/// 面板侧解析的失败档（都会变成主干那句 in-dialog 错误行，`Capabilities.cs:256-258`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalPanelError {
    /// 容器既不是对象也不是 `Undefined`/`Null` ⇒ `Capabilities.cs:135-136` 抛
    /// `"Unexpected goals/get response."`。
    NotObject,
    /// `GetProperty` 的键缺失：`revision`（`:138`）→ `roundsStarted`（`:139`）→
    /// `maxGoalRounds`（`:212`）→ `id`（`:244`，只在发 mutation 时才读）。
    MissingKey(&'static str),
    /// `GetProperty("id").GetString()`（`:244`）：`id` 键在但不是字符串 ⇒ 主干抛。
    IdNotString,
}

/// 目标钮门禁 —— `Capabilities.cs:171-193` `UpdateButtons` 里 `switch` 的七臂。
/// 只落 `allowed` 那半；`!busy && !closed && _activeSessionId == sessionId`（`:190`）是宿主的
/// 会话生命周期，不在本模块。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GoalActionGate {
    pub get: bool,
    pub create: bool,
    pub edit: bool,
    pub pause: bool,
    pub resume: bool,
    pub complete: bool,
    pub clear: bool,
}

impl GoalActionGate {
    /// 钮名取门禁（`:178-189` 的 `switch` 就是按字符串分的）。未知名 ⇒ `false`（`_ => false`）。
    pub fn enabled(&self, verb: &str) -> bool {
        match verb {
            VERB_GET => self.get,
            VERB_CREATE => self.create,
            VERB_EDIT => self.edit,
            VERB_PAUSE => self.pause,
            VERB_RESUME => self.resume,
            VERB_COMPLETE => self.complete,
            VERB_CLEAR => self.clear,
            _ => false,
        }
    }
}

/// 目标阶段五臂 —— `xaml.cs:16467-16474` 的 `switch`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalPhaseState {
    /// `("active", "disarmed")`
    Standby,
    /// `("active", _)` —— `activation` 为空（只拿到投影增量）也落在这一臂
    InProgress,
    /// `("paused", _)`
    Paused,
    /// `("blocked", _)`
    Blocked,
    /// `_` —— 兜底臂，主干给的是裸 `L("目标")`
    Generic,
}

// ---------------------------------------------------------------- 目标：解析

/// 目标条那一副面孔（`xaml.cs:16413-16431`）。
///
/// 门禁顺序照主干：先看容器，再看 `phase`，最后才逐字段填。
pub fn parse_goal_summary(goal: &Value) -> GoalBarRead {
    // :16415 —— 非对象（含 Undefined/Null）一律 null，不抛
    let Some(map) = goal.as_object() else {
        return GoalBarRead::Absent;
    };
    // :16419-16423 —— 缺键与非串都先变 ""，再被三值门禁挡掉 ⇒ 整条作废
    let phase = capability_string(goal, "phase");
    if phase != PHASE_ACTIVE && phase != PHASE_PAUSED && phase != PHASE_BLOCKED {
        return GoalBarRead::Absent;
    }
    // :16426 revision：只有 Number 才要，(long) 向零截断。
    // 现测主干是 (long)r.GetDouble()；超 i64 的数在 C# 是 unchecked 回绕、在 Rust 是饱和，
    // 但内核描述符把 revision 钉成「正安全整数」（`fake_dsh.rs:4744` 一带的原注），够不到那一档。
    let revision = map
        .get("revision")
        .and_then(Value::as_f64)
        .map(|v| v.trunc() as i64)
        .unwrap_or(0);
    // :16430 roundsStarted：GetInt32() 对「带小数」与「超 Int32」都抛
    let rounds = match map.get("roundsStarted") {
        Some(num) if num.is_number() => match num.as_f64() {
            Some(f) if f.fract() == 0.0 && f >= i32::MIN as f64 && f <= i32::MAX as f64 => f as i32,
            _ => return GoalBarRead::InvalidRoundsStarted,
        },
        _ => 0,
    };
    GoalBarRead::Summary(GoalSummary {
        id: capability_string(goal, "id"),
        revision,
        phase,
        activation: capability_string(goal, "activation"),
        objective: capability_string(goal, "objective"),
        rounds_started: rounds,
    })
}

/// 面板那一副面孔（`Capabilities.cs:131-143` + `:204-212` 的读法合成一次）。
///
/// `Ok(None)` = 主干的「尚未设置目标」档（`:133-134`，仅在 `Undefined`/`Null`）；
/// `Err(..)` = 整次作废。
pub fn parse_goal_panel(goal: &Value) -> Result<Option<GoalPanelView>, GoalPanelError> {
    // :133-134 —— 只有 Undefined / Null 走「未设置」；`false` / `0` / `""` 都是「非对象」⇒ 抛
    if goal.is_null() {
        return Ok(None);
    }
    let map = goal.as_object().ok_or(GoalPanelError::NotObject)?;
    // :138 → :139 的 GetProperty 顺序照抄（先 revision，再 roundsStarted）
    let revision = strict_get(goal, "revision")?;
    let rounds_started = strict_get(goal, "roundsStarted")?;
    // :212（Refresh 里、TargetPanelText 之后）—— 缺 maxGoalRounds 同样是整次失败
    let max_goal_rounds = strict_get(goal, "maxGoalRounds")?;
    let id_value = map.get("id");
    Ok(Some(GoalPanelView {
        phase: capability_string(goal, "phase"),
        activation: capability_string(goal, "activation"),
        objective: capability_string(goal, "objective"),
        id: capability_string(goal, "id"),
        has_id_key: id_value.is_some(),
        has_string_id: id_value.and_then(Value::as_str).is_some(),
        revision: revision.clone(),
        rounds_started: rounds_started.clone(),
        max_goal_rounds: max_goal_rounds.clone(),
        // :140-141 —— 键在就追加一行，值是不是对象只影响 message 是否为空
        blocked_reason: map
            .get("blockedReason")
            .map(|reason| GoalBlockedReason { message: capability_string(reason, "message") }),
    }))
}

impl GoalPanelView {
    /// 发 mutation 时那一格 `ref` 的两半（`Capabilities.cs:244`：`id` 要 `GetString()` 得动、
    /// `revision` 原样 `.Clone()`）。**只给数据**：不碰 `RpcCall`、不碰 args 装配、不碰派发。
    pub fn ref_fields(&self) -> Result<(String, Value), GoalPanelError> {
        if !self.has_id_key {
            return Err(GoalPanelError::MissingKey("id"));
        }
        if !self.has_string_id {
            return Err(GoalPanelError::IdNotString);
        }
        Ok((self.id.clone(), self.revision.clone()))
    }
}

/// 七臂钮门禁（`Capabilities.cs:173-189`）。
///
/// * `loaded` —— `:197/203` 的 `loaded` 位（刷过且没被置否）
/// * `has_goal` —— `goal.ValueKind == JsonValueKind.Object`（`:173`；`Null`/`Undefined` 都算没有）
///
/// ⚠ `resume` 臂主干写的是 `phase == "paused" || phase == "active" && activation == "disarmed"`，
/// C# 里 `&&` 优先于 `||` ⇒ 语义是 `paused || (active && disarmed)`（`:184` 现测），
/// 不是 `(paused || active) && disarmed`。
pub fn goal_action_gate(loaded: bool, has_goal: bool, phase: &str, activation: &str) -> GoalActionGate {
    let unfinished = has_goal && phase != PHASE_COMPLETE;
    GoalActionGate {
        get: true,
        create: loaded && !has_goal,
        edit: loaded && unfinished,
        pause: loaded && phase == PHASE_ACTIVE && activation == ACTIVATION_ARMED,
        resume: loaded && (phase == PHASE_PAUSED || (phase == PHASE_ACTIVE && activation == ACTIVATION_DISARMED)),
        complete: loaded && unfinished,
        clear: loaded && unfinished,
    }
}

/// 阶段五臂判据（`xaml.cs:16467-16474`）。
pub fn goal_phase_state(phase: &str, activation: &str) -> GoalPhaseState {
    match (phase, activation) {
        (PHASE_ACTIVE, ACTIVATION_DISARMED) => GoalPhaseState::Standby,
        (PHASE_ACTIVE, _) => GoalPhaseState::InProgress,
        (PHASE_PAUSED, _) => GoalPhaseState::Paused,
        (PHASE_BLOCKED, _) => GoalPhaseState::Blocked,
        _ => GoalPhaseState::Generic,
    }
}

/// 阶段 → zh 选键（**只给键名**，查表出串在宿主）。
pub fn goal_phase_state_zh(state: GoalPhaseState) -> &'static str {
    match state {
        GoalPhaseState::Standby => ZH_GOAL_STANDBY,
        GoalPhaseState::InProgress => ZH_GOAL_IN_PROGRESS,
        GoalPhaseState::Paused => ZH_GOAL_PAUSED,
        GoalPhaseState::Blocked => ZH_GOAL_BLOCKED,
        GoalPhaseState::Generic => ZH_GOAL_PLAIN,
    }
}

// ---------------------------------------------------------------- 技能：模型

/// `skills/list` 回执里的一项。
///
/// 主干只读两根（`Capabilities.cs:408/411`）：`name` 与 `description`。
/// 桩里多给的 `whenToUse` / `modelInvocable`（`fake_dsh.rs` 的 `skill_catalog`）这条链
/// **一个字都没读** ⇒ 不落，别照着桩的形状发明字段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillItem {
    pub name: String,
    /// 缺键/非串 ⇒ `""`（此时钮面只有 `/name`，`:414` 的条件拼接）。
    pub description: String,
}

/// `skills/list` 的整次作废档（`Capabilities.cs:393`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillsListError {
    /// `result.ValueKind != Object`
    NotObject,
    /// 没有 `skills` 键
    MissingSkills,
    /// `skills` 在但不是数组
    SkillsNotArray,
}

/// 扫描出来的一个技能条目（磁盘真相，不是回执）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEntry {
    pub name: String,
    /// 同名技能在更小 rank 的根里已出现 ⇒ 内核只会用那个，本条目被忽略（`Skills.cs:63-64`）。
    pub shadowed: bool,
}

/// 一个技能根（`Skills.cs:69-80` 的最小面：只有 `rank` 与条目参与遮蔽计算）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillRootGroup {
    pub rank: i32,
    pub entries: Vec<SkillEntry>,
}

impl SkillRootGroup {
    /// 便捷构造：`shadowed` 一律先置 `false`，交给 [`mark_shadowed`]。
    pub fn new(rank: i32, names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        SkillRootGroup {
            rank,
            entries: names
                .into_iter()
                .map(|n| SkillEntry { name: n.into(), shadowed: false })
                .collect(),
        }
    }
}

// ---------------------------------------------------------------- 技能：算子

/// `name` 的三条丢弃判据（`Capabilities.cs:409`）：全空白 / 含任意空白 / 以 `/` 开头。
/// 命中即 `continue` —— **丢这一条，不作废整次**。
///
/// 主干先 `string.IsNullOrWhiteSpace(name)`（空串与纯空白都算）再 `name.Any(char.IsWhiteSpace)`；
/// 两者结果同为丢弃、且后者严格包含前者（除空串外），故合并成一个谓词。
pub fn skill_name_rejected(name: &str) -> bool {
    name.is_empty()
        || name.chars().all(char::is_whitespace)
        || name.contains(char::is_whitespace)
        || name.starts_with('/')
}

/// `skills/list` 回执 → 条目序列（`Capabilities.cs:393-409`）。
///
/// 返回 `Vec` 的**顺序就是回执顺序**：主干无排序、无去重、无分组。
pub fn parse_skills_list(result: &Value) -> Result<Vec<SkillItem>, SkillsListError> {
    let map = result.as_object().ok_or(SkillsListError::NotObject)?;
    let skills = map.get("skills").ok_or(SkillsListError::MissingSkills)?;
    let items = skills.as_array().ok_or(SkillsListError::SkillsNotArray)?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        // 元素不是对象 ⇒ capability_string 的 Object 门禁先给 ""（不抛）⇒ 被丢掉
        let name = capability_string(item, "name");
        if skill_name_rejected(&name) {
            continue;
        }
        out.push(SkillItem {
            name,
            description: capability_string(item, "description"),
        });
    }
    Ok(out)
}

/// 同名遮蔽标记（`Skills.cs:531-538`）。
///
/// 逐点照抄：
/// * 顺序 = 按 `rank` **升序**、同 rank 保持入参顺序（主干 `roots.OrderBy(r => r.Rank)` 用的是
///   LINQ `OrderBy`，**稳定**；`sort_by_key` 在 Rust 标准库同样稳定）。
/// * 集合 = `HashSet<string>(StringComparer.Ordinal)` ⇒ 大小写敏感、不做 Unicode 折叠：
///   `Foo` 与 `foo` **不**互相遮蔽。
/// * 判定 = `!seen.Add(name)` ⇒ **第一个**见到的不算遮蔽，之后同名（含同根内重名）都算。
/// * 只打标记、**不过滤**：主干把被遮蔽的条目照样列进设置页（`Skills.cs:195`）。
pub fn mark_shadowed(roots: &mut [SkillRootGroup]) {
    let mut order: Vec<usize> = (0..roots.len()).collect();
    order.sort_by_key(|&index| roots[index].rank);
    let mut seen: HashSet<String> = HashSet::new();
    for &index in &order {
        for entry in roots[index].entries.iter_mut() {
            let first_time = seen.insert(entry.name.clone());
            entry.shadowed = !first_time;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 本地 fixture：照主干 `goals/get` 的**默认那一型**（含 `activation`）。
    /// 取值抄自桩 `src/bin/fake_dsh.rs` 的 `goal_view`（本批现测三型），只当输入数据用。
    fn goal_full() -> Value {
        json!({
            "id": "goal-1",
            "revision": 3,
            "objective": "把 session/control 的六型投影桩补齐",
            "phase": "active",
            "maxGoalRounds": 8,
            "roundsStarted": 2,
            "createdAt": 1_700_000_000_000_i64,
            "updatedAt": 1_700_000_600_000_i64,
            "activation": "armed",
        })
    }

    /// 投影增量那一型：**整根 `activation` 键都不发**（不是 null）。
    fn goal_bare() -> Value {
        let mut v = goal_full();
        v.as_object_mut().unwrap().remove("activation");
        v
    }

    // -------------------------------- 目标条那副面孔（ParseGoalSummary） --------------------------------

    #[test]
    fn cp1_goal_summary_full_shape() {
        let GoalBarRead::Summary(summary) = parse_goal_summary(&goal_full()) else {
            panic!("全量型应当产出摘要");
        };
        assert_eq!(summary.id, "goal-1");
        assert_eq!(summary.revision, 3);
        assert_eq!(summary.phase, PHASE_ACTIVE);
        assert_eq!(summary.activation, ACTIVATION_ARMED);
        assert_eq!(summary.objective, "把 session/control 的六型投影桩补齐");
        assert_eq!(summary.rounds_started, 2);
        // 主干这副面孔**没读** createdAt / updatedAt / maxGoalRounds / blockedReason
        // ⇒ GoalSummary 只有 :16424-16430 那六格，一格都不许多（多出来的字段由下面的
        //   cp1_lock_skills/goal_bar 源码锁与类型定义本身共同把住）。
    }

    #[test]
    fn cp1_goal_summary_bare_shape_keeps_activation_empty() {
        let GoalBarRead::Summary(summary) = parse_goal_summary(&goal_bare()) else {
            panic!("缺 activation 不影响成摘要");
        };
        assert_eq!(summary.activation, "", "缺键留空值，不丢条目");
        assert_eq!(summary.revision, 3);
    }

    #[test]
    fn cp1_goal_summary_blocked_shape() {
        let mut goal = goal_full();
        goal["phase"] = json!("blocked");
        goal["activation"] = json!("disarmed");
        goal["roundsStarted"] = json!(8);
        goal["blockedReason"] = json!({"code": "round-budget-exhausted", "message": "已用完轮次"});
        let GoalBarRead::Summary(summary) = parse_goal_summary(&goal) else {
            panic!("blocked 是三值门禁里的活档");
        };
        assert_eq!(summary.phase, PHASE_BLOCKED);
        // 主干 ParseGoalSummary 一个 blockedReason 字都没读 ⇒ 摘要里没有这一格
        assert_eq!(goal_phase_state(&summary.phase, &summary.activation), GoalPhaseState::Blocked);
    }

    #[test]
    fn cp1_goal_summary_absent_arms() {
        // phase 缺键 / 非串 / complete / 拼错 一律整条作废
        for bad in [json!({}), json!({"phase": 5, "revision": 1}), json!({"phase": "complete"}), json!({"phase": "ACTIVE"})] {
            assert_eq!(parse_goal_summary(&bad), GoalBarRead::Absent, "{bad}");
        }
        // 容器不是对象：主干 :16415 一律 null，**绝不抛**
        for bad in [Value::Null, json!([]), json!("goal"), json!(7), json!(false)] {
            assert_eq!(parse_goal_summary(&bad), GoalBarRead::Absent, "{bad}");
        }
        // Value::Null 就是分叉侧对「信封缺 value 键」（主干那侧是 JsonValueKind.Undefined）的表示
    }

    #[test]
    fn cp1_goal_summary_revision_is_number_only() {
        let cases: [(&str, Value, i64); 5] = [
            ("整数", json!(3), 3),
            ("字符串数字 ⇒ 非 Number ⇒ 0", json!("3"), 0),
            ("小数向零截断", json!(3.9), 3),
            ("负小数同样向零截断", json!(-3.9), -3),
            ("null ⇒ 0", json!(null), 0),
        ];
        for (label, revision, want) in cases {
            let mut goal = goal_full();
            goal["revision"] = revision.clone();
            let GoalBarRead::Summary(summary) = parse_goal_summary(&goal) else {
                panic!("{label} 不该掉出摘要档");
            };
            assert_eq!(summary.revision, want, "{label}：{revision}");
        }
    }

    #[test]
    fn cp1_goal_summary_rounds_started_throw_arm() {
        // 非数字 ⇒ 静默 0（主干三元 ?: 0）
        for value in [json!("2"), json!(null), json!(true), json!([])] {
            let mut goal = goal_full();
            goal["roundsStarted"] = value.clone();
            let GoalBarRead::Summary(summary) = parse_goal_summary(&goal) else {
                panic!("非数字走 0，不抛：{value}");
            };
            assert_eq!(summary.rounds_started, 0, "{value}");
        }
        // 缺键 ⇒ 0
        let mut goal = goal_bare();
        goal.as_object_mut().unwrap().remove("roundsStarted");
        assert!(matches!(parse_goal_summary(&goal), GoalBarRead::Summary(s) if s.rounds_started == 0));
        // 数字但 Int32 装不下 ⇒ 主干 GetInt32() 抛（**不是** DshRpcException）
        for value in [json!(2.5), json!(3_000_000_000_i64), json!(-2_147_483_649_i64)] {
            let mut goal = goal_full();
            goal["roundsStarted"] = value.clone();
            assert_eq!(parse_goal_summary(&goal), GoalBarRead::InvalidRoundsStarted, "{value}");
        }
        // 整数值的小数写法（2.0）主干认 ⇒ 我们也不能丢
        let mut goal = goal_full();
        goal["roundsStarted"] = json!(2.0);
        assert!(matches!(parse_goal_summary(&goal), GoalBarRead::Summary(s) if s.rounds_started == 2));
    }

    // -------------------------------- 面板那副面孔（TargetPanelText + Refresh + ref） --------------------------------

    #[test]
    fn cp1_goal_panel_null_and_undefined_are_not_set() {
        assert_eq!(parse_goal_panel(&Value::Null), Ok(None));
        // 主干 :133-134 只把 Undefined/Null 当「尚未设置目标」，其余非对象一律抛
        for bad in [json!(false), json!(0), json!(""), json!([]), json!("goal")] {
            assert_eq!(parse_goal_panel(&bad), Err(GoalPanelError::NotObject), "{bad}");
        }
    }

    #[test]
    fn cp1_goal_panel_missing_strict_key_voids_whole_call() {
        for key in ["revision", "roundsStarted", "maxGoalRounds"] {
            let mut goal = goal_full();
            goal.as_object_mut().unwrap().remove(key);
            assert_eq!(
                parse_goal_panel(&goal),
                Err(GoalPanelError::MissingKey(key)),
                "缺 {key} 必须整次作废，不是丢这一格"
            );
        }
        // 三根都缺时主干先撞 revision（:138 早于 :139 早于 :212）
        let mut goal = goal_full();
        let map = goal.as_object_mut().unwrap();
        map.remove("revision");
        map.remove("roundsStarted");
        map.remove("maxGoalRounds");
        assert_eq!(parse_goal_panel(&goal), Err(GoalPanelError::MissingKey("revision")));
    }

    #[test]
    fn cp1_goal_panel_does_not_type_check_strict_keys() {
        let goal = json!({
            "id": "goal-1",
            "revision": "3",              // 字符串：主干不校验，原样带进 ref
            "roundsStarted": null,        // null：同样原样
            "maxGoalRounds": {"a": 1},    // 对象：同样原样
            "phase": "active",
        });
        let Some(view) = parse_goal_panel(&goal).unwrap() else { panic!("是对象就该有货") };
        assert_eq!(view.revision, json!("3"));
        assert_eq!(view.rounds_started, json!(null));
        assert_eq!(view.max_goal_rounds, json!({"a": 1}));
        assert_eq!(view.ref_fields().unwrap(), ("goal-1".to_string(), json!("3")));
    }

    #[test]
    fn cp1_goal_panel_soft_keys_fall_back_to_empty_string() {
        let goal = json!({
            "revision": 1, "roundsStarted": 1, "maxGoalRounds": 1,
            "phase": 5,          // 类型不符
            "objective": null,   // 类型不符
            // activation / id 缺键
        });
        let Some(view) = parse_goal_panel(&goal).unwrap() else { panic!("是对象就该有货") };
        assert_eq!(view.phase, "");
        assert_eq!(view.objective, "");
        assert_eq!(view.activation, "");
        assert_eq!(view.id, "");
        assert!(!view.has_id_key);
        assert!(!view.has_string_id);
        assert!(view.blocked_reason.is_none());
    }

    #[test]
    fn cp1_goal_panel_blocked_reason_is_presence_not_type() {
        for value in [json!(5), json!("text"), json!(null), json!([]), json!({"message": 9})] {
            let mut goal = goal_full();
            goal["blockedReason"] = value.clone();
            let Some(view) = parse_goal_panel(&goal).unwrap() else { panic!("{value}") };
            assert_eq!(
                view.blocked_reason.as_ref().map(|r| r.message.as_str()),
                Some(""),
                "键在 ⇒ 主干照样追加一行（哪怕内容是空）：{value}"
            );
        }
        let mut goal = goal_full();
        goal["blockedReason"] = json!({"code": "c", "message": "已用完轮次"});
        let Some(view) = parse_goal_panel(&goal).unwrap() else { panic!() };
        assert_eq!(view.blocked_reason.unwrap().message, "已用完轮次");
    }

    #[test]
    fn cp1_goal_panel_ref_fields_arms_for_id() {
        // id 缺键 ⇒ 主干 :244 GetProperty("id") 抛
        let mut goal = goal_full();
        goal.as_object_mut().unwrap().remove("id");
        let view = parse_goal_panel(&goal).unwrap().unwrap();
        assert_eq!(view.ref_fields(), Err(GoalPanelError::MissingKey("id")));
        // id 在但不是字符串 ⇒ GetString() 抛
        goal["id"] = json!(7);
        let view = parse_goal_panel(&goal).unwrap().unwrap();
        assert_eq!(view.ref_fields(), Err(GoalPanelError::IdNotString));
    }

    // -------------------------------- 钮状态机与阶段选键 --------------------------------

    #[test]
    fn cp1_goal_action_gate_truth_table() {
        // loaded=false（还没刷过）：除 get 全灭
        let gate = goal_action_gate(false, true, PHASE_ACTIVE, ACTIVATION_ARMED);
        assert_eq!(
            gate,
            GoalActionGate { get: true, create: false, edit: false, pause: false, resume: false, complete: false, clear: false }
        );
        // 没目标：只有 create
        assert_eq!(goal_action_gate(true, false, "", ""), GoalActionGate { get: true, create: true, ..Default::default() });
        // active + armed
        let gate = goal_action_gate(true, true, PHASE_ACTIVE, ACTIVATION_ARMED);
        assert!(gate.enabled(VERB_EDIT) && gate.enabled(VERB_PAUSE) && gate.enabled(VERB_COMPLETE) && gate.enabled(VERB_CLEAR));
        assert!(!gate.enabled(VERB_RESUME) && !gate.enabled(VERB_CREATE));
        // active + disarmed（待命）：可恢复、不可暂停
        let gate = goal_action_gate(true, true, PHASE_ACTIVE, ACTIVATION_DISARMED);
        assert!(gate.enabled(VERB_RESUME) && !gate.enabled(VERB_PAUSE));
        // paused：可恢复；不可暂停（不看 activation）
        assert!(goal_action_gate(true, true, PHASE_PAUSED, "").enabled(VERB_RESUME));
        // complete：edit/complete/clear 全灭，pause/resume 也灭（phase 不是 active/paused）
        let gate = goal_action_gate(true, true, PHASE_COMPLETE, ACTIVATION_ARMED);
        assert_eq!(gate, GoalActionGate { get: true, ..Default::default() });
        // 未知钮名 ⇒ false（`_ => false`）
        assert!(!goal_action_gate(true, true, PHASE_ACTIVE, ACTIVATION_ARMED).enabled("delete"));
    }

    #[test]
    fn cp1_goal_action_gate_resume_operator_precedence() {
        // 主干 :184 是 `paused || (active && disarmed)`：
        // (paused, armed) ⇒ true；(active, armed) ⇒ false；(active, "") ⇒ false（投影那一型没 activation）
        assert!(goal_action_gate(true, true, PHASE_PAUSED, ACTIVATION_ARMED).resume);
        assert!(!goal_action_gate(true, true, PHASE_ACTIVE, ACTIVATION_ARMED).resume);
        assert!(!goal_action_gate(true, true, PHASE_ACTIVE, "").resume, "activation 空串不算 disarmed");
        // 反证：若把 && / || 的优先级记成 `(paused || active) && disarmed`，
        // (paused, armed) 就会被判成不可恢复 —— 主干 :184 现测是 true。
        assert!(!goal_action_gate(true, true, PHASE_PAUSED, ACTIVATION_ARMED).pause, "paused 也不该能暂停");
        assert!(goal_action_gate(true, true, PHASE_PAUSED, "").resume, "paused 不看 activation");
    }

    #[test]
    fn cp1_goal_phase_state_five_arms() {
        assert_eq!(goal_phase_state(PHASE_ACTIVE, ACTIVATION_DISARMED), GoalPhaseState::Standby);
        assert_eq!(goal_phase_state(PHASE_ACTIVE, ACTIVATION_ARMED), GoalPhaseState::InProgress);
        assert_eq!(goal_phase_state(PHASE_ACTIVE, ""), GoalPhaseState::InProgress, "投影增量没 activation ⇒ 按进行中");
        assert_eq!(goal_phase_state(PHASE_PAUSED, ACTIVATION_ARMED), GoalPhaseState::Paused);
        assert_eq!(goal_phase_state(PHASE_BLOCKED, ""), GoalPhaseState::Blocked);
        assert_eq!(goal_phase_state(PHASE_COMPLETE, ACTIVATION_ARMED), GoalPhaseState::Generic);
        assert_eq!(goal_phase_state("", ""), GoalPhaseState::Generic);
        assert_eq!(goal_phase_state_zh(GoalPhaseState::Standby), ZH_GOAL_STANDBY);
        assert_eq!(goal_phase_state_zh(GoalPhaseState::Generic), ZH_GOAL_PLAIN);
        let keys = [
            goal_phase_state_zh(GoalPhaseState::Standby),
            goal_phase_state_zh(GoalPhaseState::InProgress),
            goal_phase_state_zh(GoalPhaseState::Paused),
            goal_phase_state_zh(GoalPhaseState::Blocked),
            goal_phase_state_zh(GoalPhaseState::Generic),
        ];
        assert_eq!(keys.iter().collect::<std::collections::HashSet<_>>().len(), 5, "五臂五把键不许撞");
    }

    // -------------------------------- skills/list --------------------------------

    #[test]
    fn cp1_skills_list_voids_whole_call_three_arms() {
        assert_eq!(parse_skills_list(&json!([])), Err(SkillsListError::NotObject));
        assert_eq!(parse_skills_list(&Value::Null), Err(SkillsListError::NotObject));
        assert_eq!(parse_skills_list(&json!("x")), Err(SkillsListError::NotObject));
        assert_eq!(parse_skills_list(&json!({})), Err(SkillsListError::MissingSkills));
        // `{"skills": null}`：主干 TryGetProperty 命中（ValueKind=Null）⇒ 走「不是数组」那一档
        assert_eq!(parse_skills_list(&json!({"skills": null})), Err(SkillsListError::SkillsNotArray));
        assert_eq!(parse_skills_list(&json!({"skills": {"a": 1}})), Err(SkillsListError::SkillsNotArray));
        assert_eq!(parse_skills_list(&json!({"skills": []})), Ok(vec![]));
    }

    #[test]
    fn cp1_skills_list_drops_bad_names_and_keeps_order() {
        let value = json!({"skills": [
            {"name": "commit-writer", "description": "写提交信息"},
            {"name": "has space"},
            {"name": "/leading"},
            {"name": "  "},
            {"name": ""},
            {"name": 5},
            {"description": "没名字"},
            {"name": "release-notes"},
            {"name": "带\t制表符"},
            {"name": "ok-2", "description": null},
        ]});
        let items = parse_skills_list(&value).unwrap();
        assert_eq!(
            items,
            vec![
                SkillItem { name: "commit-writer".into(), description: "写提交信息".into() },
                SkillItem { name: "release-notes".into(), description: "".into() },
                SkillItem { name: "ok-2".into(), description: "".into() },
            ],
            "顺序 = 回执顺序：主干无排序无去重"
        );
    }

    #[test]
    fn cp1_skills_list_bad_element_drops_instead_of_throwing() {
        // 与 subagents 那族**相反**的失败半径：坏条目丢、坏容器作废
        let items = parse_skills_list(&json!({"skills": ["x", 5, null, [], {"name": "fine"}]})).unwrap();
        assert_eq!(items, vec![SkillItem { name: "fine".into(), description: "".into() }]);
    }

    #[test]
    fn cp1_skill_name_rejected_arms() {
        for bad in ["", " ", "\u{00a0}", "a b", "a\tb", "a\nb", "/x", "/"] {
            assert!(skill_name_rejected(bad), "{bad:?} 该丢");
        }
        for good in ["a", "a-b", "技能", "a_b", "x/", "commit-writer"] {
            assert!(!skill_name_rejected(good), "{good:?} 该留");
        }
        // C# char.IsWhiteSpace 认 U+00A0（不换行的空格）⇒ Rust 的 char::is_whitespace 同认
        assert!(skill_name_rejected("a\u{00a0}b"), "非断行空格也算空白");
        assert!(skill_name_rejected("中文 名字"), "含空白的中文名同样丢");
        assert!(!skill_name_rejected("技能"), "主干没有 kebab-case 门禁：中文名留得住");
    }

    // -------------------------------- 账本遮蔽标记 --------------------------------

    #[test]
    fn cp1_mark_shadowed_ranks_and_stability() {
        // 主干 rank：100 项目.dsh / 200 项目.agents / 300 预设 / 400 用户 / 500 用户.agents
        let mut roots = vec![
            SkillRootGroup::new(RANK_USER_DSH, ["same-name", "user-only"]),
            SkillRootGroup::new(RANK_PROJECT_DSH, ["same-name"]),
            SkillRootGroup::new(RANK_PRESET, ["preset-only", "same-name"]),
        ];
        mark_shadowed(&mut roots);
        assert!(!roots[1].entries[0].shadowed, "rank 100 那个胜出");
        assert!(roots[2].entries[1].shadowed, "rank 300 的同名被遮蔽");
        assert!(roots[0].entries[0].shadowed, "rank 400 的同名被遮蔽");
        assert!(!roots[2].entries[0].shadowed && !roots[0].entries[1].shadowed, "不同名不受影响");
        // 只打标记、不过滤：条目数一根不少（主干 :195 照样列出来）
        assert_eq!(roots.iter().map(|r| r.entries.len()).sum::<usize>(), 5);
    }

    #[test]
    fn cp1_mark_shadowed_is_ordinal() {
        let mut roots = vec![
            SkillRootGroup::new(RANK_PROJECT_DSH, ["Kebab-Case"]),
            SkillRootGroup::new(RANK_USER_DSH, ["kebab-case"]),
        ];
        mark_shadowed(&mut roots);
        assert!(!roots[0].entries[0].shadowed);
        assert!(!roots[1].entries[0].shadowed, "StringComparer.Ordinal ⇒ 大小写不同不互相遮蔽");
    }

    #[test]
    fn cp1_mark_shadowed_same_root_duplicate_and_input_order() {
        // 同根内重名（目录包与扁平 .md 同名）：主干 :536 跨全部条目分先后 ⇒ 第二个仍算遮蔽
        let mut roots = vec![SkillRootGroup::new(RANK_USER_DSH, ["dup", "dup"])];
        mark_shadowed(&mut roots);
        assert!(!roots[0].entries[0].shadowed);
        assert!(roots[0].entries[1].shadowed);
        // 入参乱序也要按 rank 升序判定（同 rank 保持入参顺序 = 稳定排序）
        let mut roots = vec![
            SkillRootGroup::new(RANK_USER_AGENTS, ["x"]),
            SkillRootGroup::new(RANK_PROJECT_AGENTS, ["x", "x"]),
        ];
        mark_shadowed(&mut roots);
        assert!(!roots[1].entries[0].shadowed, "rank 200 第一个胜出");
        assert!(roots[1].entries[1].shadowed, "同 rank 内第二个仍被遮蔽");
        assert!(roots[0].entries[0].shadowed);
        mark_shadowed(&mut []); // 空根表不许炸
    }

    // -------------------------------- 主干源码锁（运行时 std::fs；读不到 ⇒ eprintln + skip） ------------------

    const CAPS: &str = "MainWindow.Capabilities.cs";
    const XAML: &str = "MainWindow.xaml.cs";
    const SKILLS: &str = "MainWindow.Skills.cs";
    const CLIENT: &str = "Dsh/DshRpcClient.cs";

    /// 运行时读主干源（**不** `include_str!`）；读不到 ⇒ 打印原因并让调用方 skip（不 panic）。
    /// 顺手把 CRLF 归一成 LF：现测这几颗是纯 LF（`grep -c $'\r'` = 0），归一只是防漂。
    fn mainline(rel: &str) -> Option<String> {
        let path = format!("{}/../{}", env!("CARGO_MANIFEST_DIR"), rel);
        match std::fs::read_to_string(&path) {
            Ok(text) => Some(text.replace("\r\n", "\n")),
            Err(err) => {
                eprintln!("cp1 读不到主干源 {path}（{err}）—— 本锁跳过");
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
        report_window_up_to(label, text, 60);
    }

    fn report_window_up_to(label: &str, text: &str, max: usize) {
        let lines = text.lines().count();
        eprintln!("cp1 锁[{label}] 窗口行数 = {lines}");
        assert!((1..=max).contains(&lines), "{label} 窗口松了（{lines} 行）");
    }

    /// 锚唯一性：`needle` 在整颗文件里必须出现 `want` 次（防「窗口恰好含它」式假锁）。
    fn assert_hits(src: &str, needle: &str, want: usize, label: &str) {
        let got = src.matches(needle).count();
        eprintln!("cp1 锁[{label}] {needle:?} 命中 {got} 次");
        assert_eq!(got, want, "{label}：{needle:?} 命中数不是现测的 {want}（实得 {got}）");
    }

    fn window_of(rel: &str, start: &str, end: &str, label: &str) -> Option<String> {
        let Some(src) = mainline(rel) else { return None };
        let Some(win) = tight_window(&src, start, end) else {
            panic!("{label}：窗口锚丢了（{rel} / {start:?} → {end:?}）");
        };
        report_window(label, &win);
        Some(win)
    }

    #[test]
    fn cp1_lock_receipt_layer_is_the_value_key() {
        let Some(src) = mainline(CLIENT) else { return };
        let win = tight_window(&src, "public async Task<JsonElement> CallOkAsync", "\n    /// <summary>服务器基地址")
            .expect("CallOkAsync 窗口锚丢失");
        report_window("CallOkAsync", &win);
        assert!(win.contains("if (!result.TryGetProperty(\"ok\", out var okEl) || okEl.ValueKind != JsonValueKind.True)"));
        assert_hits(&src, "return result.TryGetProperty(\"value\", out var value) ? value.Clone() : default;", 1, "返回 value 那一层");
    }

    #[test]
    fn cp1_lock_goal_panel_container_and_soft_field_reads() {
        let win = match window_of(CAPS, "private string TargetPanelText(JsonElement goal)", "\n    }", "TargetPanelText") {
            Some(win) => win,
            None => return,
        };
        for needle in [
            concat!("goal.ValueKind is JsonValueKind.Undefined ", "or JsonValueKind.Null"),
            concat!("CapabilityString(goal, \"phase\")} / {", "CapabilityString(goal, \"activation\")"),
            concat!("ID: {CapabilityString(goal, \"id\")}  revision: ", "{goal.GetProperty(\"revision\")}"),
            "CapabilityString(reason, \"message\")",
        ] {
            assert!(win.contains(needle), "面板读法缺针：{needle}");
        }
        let Some(src) = mainline(CAPS) else { return };
        assert_hits(&src, "Unexpected goals/get response.", 1, "非对象那一抛");
        assert_hits(&src, "JsonValueKind.Undefined or JsonValueKind.Null", 2, ":133 与 :205 两档");
        assert_hits(&src, "TryGetProperty(\"blockedReason\"", 1, "blockedReason 只看键在不在");
        // CapabilityString 的三 AND 定义
        let helper = tight_window(&src, "private static string CapabilityString(JsonElement value, string key) =>", "\n    private string CapabilitySession")
            .expect("CapabilityString 窗口");
        report_window("CapabilityString", &helper);
        for needle in ["value.ValueKind == JsonValueKind.Object", "field.ValueKind == JsonValueKind.String", "? field.GetString() ?? \"\" : \"\";"] {
            assert!(helper.contains(needle), "CapabilityString 缺针：{needle}");
        }
    }

    #[test]
    fn cp1_lock_goal_panel_strict_keys_void_the_call() {
        let win = match window_of(CAPS, "async Task Refresh()", "\n        async Task Run(string verb)", "Refresh") {
            Some(win) => win,
            None => return,
        };
        assert!(win.contains(concat!("var next = await rpc.CallOkAsync(\"goals/get\", new ", "{ agentId = sessionId }, lifetime.Token);")));
        assert!(win.contains("goal.GetProperty(\"maxGoalRounds\").ToString()"));
        assert!(win.contains("objective.Text = CapabilityString(goal, \"objective\");"));
        let Some(src) = mainline(CAPS) else { return };
        // 面板侧的三根 GetProperty（不带 Try）＝ 缺键就整次作废
        assert_hits(&src, "goal.GetProperty(\"revision\")", 2, "显示一次 + 发 ref 一次");
        assert_hits(&src, "goal.GetProperty(\"roundsStarted\")", 1, "面板插值");
        assert_hits(&src, "goal.GetProperty(\"maxGoalRounds\")", 1, "面板回填");
    }

    #[test]
    fn cp1_lock_goal_mutation_ref_reads_id_strictly() {
        let win = match window_of(
            CAPS,
            "var args = new Dictionary<string, object> { [\"agentId\"] = sessionId };",
            "mutationAttempted = true;",
            "mutation args",
        ) {
            Some(win) => win,
            None => return,
        };
        assert!(win.contains(concat!(
            "args[\"ref\"] = new { id = goal.GetProperty(\"id\").GetString(), ",
            "revision = goal.GetProperty(\"revision\").Clone() };"
        )));
        assert!(win.contains("if (verb is \"create\" or \"edit\") args[\"request\"] = request;"));
        let Some(src) = mainline(CAPS) else { return };
        // 六发 mutation 的回执主干直接丢弃（`:248` 是 `await`，不是 `= await`）⇒ 本模块不给它们做解析
        assert_hits(&src, "await rpc.CallOkAsync(\"goals/\" + verb, args, lifetime.Token);", 1, "goals/<verb> 一发");
        let assigned = src.matches("= await rpc.CallOkAsync(\"goals/\" + verb").count();
        eprintln!("cp1 锁[mutation 回执] 赋值形态命中 {assigned} 次");
        assert_eq!(assigned, 0, "主干一旦开始读 mutation 回执，本模块要跟着补解析");
    }

    #[test]
    fn cp1_lock_goal_action_gate_seven_arms() {
        let win = match window_of(CAPS, "void UpdateButtons()", "\n            objective.IsEnabled", "UpdateButtons") {
            Some(win) => win,
            None => return,
        };
        for (arm, label) in [
            ("\"get\" => true,", "get 恒允许"),
            ("\"create\" => loaded && !hasGoal,", "create 只在无目标"),
            ("\"edit\" => loaded && hasGoal && phase != \"complete\",", "edit"),
            ("\"pause\" => loaded && phase == \"active\" && activation == \"armed\",", "pause"),
            ("\"resume\" => loaded && (phase == \"paused\" || phase == \"active\" && activation == \"disarmed\"),", "resume 的 && 优先"),
            ("\"complete\" => loaded && hasGoal && phase != \"complete\",", "complete"),
            ("\"clear\" => loaded && hasGoal && phase != \"complete\",", "clear"),
            ("_ => false,", "兜底"),
        ] {
            assert!(win.contains(arm), "{label} 那臂不在了：{arm}");
        }
        assert!(win.contains("bool hasGoal = goal.ValueKind == JsonValueKind.Object;"));
        // busy / closed / 会话切换三格属于宿主，本模块不落
        assert!(win.contains("pair.Value.IsEnabled = !busy && !closed && _activeSessionId == sessionId && allowed;"));
    }

    #[test]
    fn cp1_lock_goal_bar_parse_phase_gate_and_fallbacks() {
        let win = match window_of(XAML, "private static (string Id, long Revision, string Phase, string Activation, string Objective, int RoundsStarted)? ParseGoalSummary", "\n    private void ApplyGoalBar()", "ParseGoalSummary") {
            Some(win) => win,
            None => return,
        };
        assert!(win.contains("if (goal.ValueKind != JsonValueKind.Object)"));
        assert!(win.contains("phase is not (\"active\" or \"paused\" or \"blocked\")"));
        assert!(win.contains("r.ValueKind == JsonValueKind.Number ? (long)r.GetDouble() : 0"));
        assert!(win.contains("rs.ValueKind == JsonValueKind.Number ? rs.GetInt32() : 0"));
        let Some(src) = mainline(XAML) else { return };
        assert_hits(&src, "ParseGoalSummary(JsonElement goal)", 1, "只有一个定义");
        assert_hits(&src, "ParseGoalSummary(goal);", 1, "goals/get 那一头");
        assert_hits(&src, "ParseGoalSummary(inner)", 1, "投影那一头（复用同一副面孔）");
        assert_hits(&src, "(long)r.GetDouble()", 1, "revision 的 Number 门禁");
        assert_hits(&src, "rs.GetInt32()", 1, "roundsStarted 会抛那一档");
    }

    #[test]
    fn cp1_lock_goal_bar_catch_radius_and_session_guard() {
        let win = match window_of(XAML, "private async Task RefreshGoalBarAsync(string sessionId)", "\n    /// <summary>goals/get 响应", "RefreshGoalBarAsync") {
            Some(win) => win,
            None => return,
        };
        assert!(win.contains("catch (DshRpcException)"), "RPC 失败按无目标静默收起");
        assert!(win.contains("catch (Exception)"), "外层兜底：GetInt32 那一抛落在这里");
        assert!(win.contains("if (Volatile.Read(ref _activeSessionId) == sessionId)"), "过期响应丢弃（宿主级，本模块不落）");
        let hits = win.matches("catch (DshRpcException)").count();
        eprintln!("cp1 锁[RefreshGoalBarAsync] catch (DshRpcException) 窗口内命中 {hits} 次");
        assert_eq!(hits, 1);
    }

    #[test]
    fn cp1_lock_goal_phase_label_five_arms() {
        let win = match window_of(XAML, "private string GoalPhaseLabel(string phase, string activation) => (phase, activation) switch", "\n    /// <summary>目标条「管理」入口", "GoalPhaseLabel") {
            Some(win) => win,
            None => return,
        };
        for arm in [
            "(\"active\", \"disarmed\") => L(\"目标待命\"),",
            "(\"active\", _) => L(\"目标进行中\"),",
            "(\"paused\", _) => L(\"目标已暂停\"),",
            "(\"blocked\", _) => L(\"目标受阻\"),",
            "_ => L(\"目标\"),",
        ] {
            assert!(win.contains(arm), "阶段臂不在了：{arm}");
        }
    }

    #[test]
    fn cp1_lock_skills_list_void_and_drop_radius() {
        let gate = match window_of(
            CAPS,
            "var result = await _rpc!.CallOkAsync(\"skills/list\", new { request = new { sessionId } });",
            "string? selected = null;",
            "skills/list 门禁",
        ) {
            Some(win) => win,
            None => return,
        };
        assert!(gate.contains(concat!(
            "if (result.ValueKind != JsonValueKind.Object || !result.TryGetProperty(\"skills\", out var skills) ",
            "|| skills.ValueKind != JsonValueKind.Array)"
        )));
        let items = match window_of(CAPS, "foreach (var skill in skills.EnumerateArray())", "\n        if (count == 0)", "skills 条目") {
            Some(win) => win,
            None => return,
        };
        assert!(items.contains(concat!(
            "if (string.IsNullOrWhiteSpace(name) || name.Any(char.IsWhiteSpace) || name.StartsWith('/')) continue;"
        )));
        assert!(items.contains("var description = CapabilityString(skill, \"description\");"));
        assert!(items.contains("var name = CapabilityString(skill, \"name\");"));
        // 反发明：这条链上的回执只读两根
        let Some(src) = mainline(CAPS) else { return };
        assert_hits(&src, "CapabilityString(skill, \"name\")", 1, "name");
        assert_hits(&src, "CapabilityString(skill, \"description\")", 1, "description");
        for needle in ["whenToUse", "modelInvocable", "Shadowed", "shadow"] {
            assert_hits(&src, needle, 0, "skills/list 链上没有这一格");
        }
        // 反发明：主干对这份回执不排序、不去重、不分组
        let sorting = items.matches("OrderBy").count() + gate.matches("OrderBy").count();
        eprintln!("cp1 锁[skills 窗口] OrderBy 命中 {sorting} 次");
        assert_eq!(sorting, 0, "主干加了排序就得连本模块一起改");
    }

    #[test]
    fn cp1_lock_shadowing_is_a_disk_scan_product_not_a_receipt_field() {
        // 反发明锁：整颗 skills/list 宿主文件里一根 shadow 都没有 ⇒ 遮蔽不是回执字段
        let Some(src) = mainline(CAPS) else { return };
        eprintln!("cp1 主干现测 {CAPS} 总行数 = {}", src.lines().count());
        assert_hits(&src, "Shadow", 0, "大写法");
        assert_hits(&src, "shadow", 0, "小写法");
        let Some(skills) = mainline(SKILLS) else { return };
        eprintln!("cp1 主干现测 {SKILLS} 总行数 = {}", skills.lines().count());
        let shadow = tight_window(&skills, "private static void MarkShadowed(List<SkillRoot> roots)", "\n    /// <summary>读一个根目录下的全部技能")
            .expect("MarkShadowed 窗口");
        report_window("MarkShadowed", &shadow);
        for needle in [
            "var seen = new HashSet<string>(StringComparer.Ordinal);",
            "foreach (var entry in roots.OrderBy(r => r.Rank).SelectMany(r => r.Entries))",
            "entry.Shadowed = !seen.Add(entry.Name);",
        ] {
            assert!(shadow.contains(needle), "遮蔽判据不在了：{needle}");
        }
        assert_hits(&skills, "entry.Shadowed = !seen.Add(entry.Name);", 1, "唯一一处遮蔽赋值");
        // 被遮蔽的条目主干照样列出来（只打标记，不过滤）
        assert!(skills.contains("MakeSkillReferenceChip(skill.Name, shadowed: skill.Shadowed)"));
    }

    #[test]
    fn cp1_lock_skill_root_rank_table() {
        let Some(src) = mainline(SKILLS) else { return };
        let win = tight_window(&src, "var roots = new List<SkillRoot>();", "\n        MarkShadowed(roots);")
            .expect("根表窗口");
        report_window_up_to("ScanSkillRoots 根表", &win, 80);
        for needle in [
            "MakeRoot(L(\"项目\"), Path.Combine(projectRoot, \".dsh\", \"skills\"), 100)",
            "MakeRoot(L(\"项目（兼容）\"), Path.Combine(projectRoot, \".agents\", \"skills\"), 200)",
            "MakeRoot(L(\"预设\"), \"\", 300, writable: false)",
            "MakeRoot(L(\"用户\"), Path.Combine(dshHome, \"skills\"), 400)",
            "MakeRoot(L(\"用户（兼容）\"), Path.Combine(agentsHome, \"skills\"), 500)",
        ] {
            assert!(win.contains(needle), "根表缺臂：{needle}");
        }
        assert_eq!(
            [RANK_PROJECT_DSH, RANK_PROJECT_AGENTS, RANK_PRESET, RANK_USER_DSH, RANK_USER_AGENTS],
            [100, 200, 300, 400, 500],
            "主干改了 rank 表就得同时改本模块"
        );
    }
}
