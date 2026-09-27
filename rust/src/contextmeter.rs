//! #57「上下文容量圈 ContextMeter」的**纯算子层**（台账 #57，分叉代号 CR1）。
//!
//! 这里只有数值、枚举与字符串变换，一个 `View` 都不碰：分子/分母的折叠、占用比例、
//! 弧面三态、弧几何、tooltip/无障碍名的中文串 —— 全部可离线单测。渲染与接线在
//! `main.rs`（本轮不接 UI：`main.rs` 由另一枚代理在写，本模块**零 main.rs 依赖**）。
//!
//! 规格来源 = 本地主干工作树（2026-09-25 抄，逐格对应在下面的常量与文档里）：
//! · `CM:n` = [`MainWindow.ContextMeter.cs`]（真名 `ContextMeterState` / `TrackContextMeter` /
//!   `PressureTokensOf` / `UsageLong` / `ContextOccupancy` / `ResetContextMeter` /
//!   `UpdateContextMeterUi` / `SetContextMeterRing` / `ShowContextPanel`）
//! · `MX:n`  = [`MainWindow.xaml`]（`ContextMeterRing` 那颗 28×28 + `ContextMeterArc` +
//!   `ContextMeterFullRing`）
//! · `MCS:n` = [`MainWindow.xaml.cs`]（`UpdateContextMeterUi` 的调用侧：会话切换清零
//!   `MCS:4502`、`RenderEventCore` 折叠入口 `MCS:5992`、动作行宽度预算 `MCS:9631`）
//!
//! 数据源**不在本模块**：主干把折叠挂在 `RenderEventCore`（page 回放 + follow 实时流共用，
//! `MCS:5992`），分叉那边由 `main.rs` 的事件循环把 `(event_type, data)` 喂进 [`ContextMeterBook`]。
//! 官方主端的 `projectedTokens`（采样 + surface 折叠增量）主干明确不做（`CM:20-21`），
//! 本模块同样只做**裸采样**口径。

use crate::i18n::Catalog;
use serde_json::Value;
use std::collections::HashMap;

// ---------------------------------------------------------------- 事件类型与 usage 字段

/// 分母来源：`request/context` 的 `data.contextWindow`（`CM:59-65`）。
pub const EVENT_REQUEST_CONTEXT: &str = "request/context";
/// 分子来源之一：`assistant/message` —— 以 `data.usage` 起步、被同事件 stream 里的
/// `type == "usage"` chunk 覆盖（`CM:66-68` + `CM:91-95`）。
pub const EVENT_ASSISTANT_MESSAGE: &str = "assistant/message";
/// 分子来源之二：`assistant/attempt` —— **只认** stream 里的 usage chunk，
/// 官方对 attempt 不读 `data.usage`（`CM:68` 的 `type == "assistant/message"` 实参）。
pub const EVENT_ASSISTANT_ATTEMPT: &str = "assistant/attempt";

/// 分子 = prompt 侧 token 三项之和（官方 `pressureFrom` 口径，`CM:118-120`）。
/// **不含** `outputTokens` —— 那是用量条的事，不是容量分母的分子。
pub const PRESSURE_USAGE_FIELDS: [&str; 3] = ["inputTokens", "cacheReadTokens", "cacheWriteTokens"];

// ---------------------------------------------------------------- 环几何（MX:1317-1331 + CM:149-152）

/// 官方 viewBox `0 0 14 14` → 盒子 14（`MX:1317` 那颗内层 `Grid Width/Height="14"`）。
pub const RING_BOX: f64 = 14.0;
/// 圆心（`CM:151` `RingCenter = 7.0`）。
pub const RING_CENTER: f64 = 7.0;
/// 半径（`CM:152` `RingRadius = 6.0`）。12×12 的圆 + 居中线宽 2 ⇒ 描边带落在 5..7、不出盒。
pub const RING_RADIUS: f64 = 6.0;
/// 描边线宽（`MX:1322/1325/1329` 三处 `StrokeThickness="2"`）。
pub const RING_STROKE: f64 = 2.0;
/// 按钮本体（`MX:1307` `Width="28" Height="28"`）。
pub const RING_BUTTON: f64 = 28.0;
/// 满值门槛：percent 到这里就是整圆（`CM:132` 的 `Math.Min(100, …)` 与 `CM:189` 的 `p >= 100`）。
pub const FULL_PERCENT: i32 = 100;

/// 圈在动作行里预留的固定宽度：圈 28 + 一条 8dip 列间距 = 36（`MCS:9631`）。
pub const ACTION_ROW_RESERVE: f64 = 36.0;

/// 可见性翻转的连带账：圈在场就多扣 36dip 的选择器组可用宽（`MCS:9631` 的三元式）。
pub const fn row_reserve(visible: bool) -> f64 {
    if visible {
        ACTION_ROW_RESERVE
    } else {
        0.0
    }
}

// ---------------------------------------------------------------- 文案（CM:168-170 + MX:1313-1314）

/// tooltip / 无障碍名的中文模板键（`CM:168` 的 `LF("上下文已用 {0}%")`）。
/// `i18n::EN` 已有这条（`i18n.rs:1094` → `{0}% of context used`），**不需要新键**。
pub const ARIA_TEMPLATE: &str = "上下文已用 {0}%";
/// 数据不齐时的静态名（`MX:1314` 的 `AutomationProperties.Name="上下文容量"`）：
/// 主干 `occ is null` 那一支不覆盖 XAML 里写死的这枚，所以它仍是无障碍树里的现值。
/// `i18n.rs:1096` 已有 → `Context capacity`。
pub const ARIA_FALLBACK: &str = "上下文容量";

// ---------------------------------------------------------------- 状态：单会话的容量折叠

/// 单会话的容量折叠状态 = 主干 `ContextMeterState`（`CM:28-32`）。
///
/// 两个字段都是 `Option`：`None` = 「这类帧没采到」。可见性判据（[`ContextMeterState::is_visible`]）
/// 要求**两者齐备**，缺一即不画 —— 与官方「数据不齐不显示」一致（`CM:128-133`）。
/// `Copy`：一份状态就是一对 `Option<i64>`，主干那边是引用类型但从不原地共享，值语义等价。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContextMeterState {
    /// 分母：最近一条 `request/context` 的 `data.contextWindow`（`CM:30`）。
    pub context_window: Option<i64>,
    /// 分子：最近一次 usage 采样的 prompt 侧 token（`CM:31`）。
    pub pressure_tokens: Option<i64>,
}

/// 主干 `UsageLong`（`CM:123-126`）：读不到 / 非数 / 负数 ⇒ `None`（调用侧按 0 计）。
///
/// 口径照抄：`v.ValueKind == Number && v.GetDouble() >= 0 ? (long)v.GetDouble() : null`
/// —— 闸门开在**double 值**上、截断在后（`-0.5` 直接算没有，`3.7` 算 3）。
#[must_use]
pub fn usage_long(usage: &Value, key: &str) -> Option<i64> {
    usage
        .get(key)
        .and_then(Value::as_f64)
        .filter(|v| *v >= 0.0)
        .map(|v| v as i64)
}

/// usage 采样 → prompt 侧 token = 主干 `PressureTokensOf`（`CM:87-121`）。
///
/// `read_direct_usage` 对应 `CM:68` 那个实参（`type == "assistant/message"`）：
/// · `true` ⇒ 先取 `data.usage`，再被 stream 里 `type == "usage"` 的 chunk **覆盖**（后到者胜）。
/// · `false` ⇒ 完全无视 `data.usage`，只扫 stream（`assistant/attempt` 案）。
/// · 两处都没采到 ⇒ `None`（主干 `CM:114-117`：不给值 ⇒ 调用侧保留上一份）。
///
/// 一处**照抄下来的**分支：主干 `CM:106` 是 `ct.GetString() == "usage"`，`chunk.type` 存在但
/// 不是字符串时 `GetString()` 抛 `InvalidOperationException`，被 `CM:79` 的外层 `catch` 吞掉 ⇒
/// **整帧作废**（连 `data.usage` 那份也不写）。这里用 `TypeTag::Poisoned` 复现同一观察结果，
/// 而不是「跳过这块继续扫」——后者会让分叉多记一帧。
#[must_use]
pub fn pressure_tokens_of(data: &Value, read_direct_usage: bool) -> PressureProbe {
    let mut usage: Option<&Value> = None;
    let mut found = false;
    if read_direct_usage {
        if let Some(direct) = data.get("usage").filter(|u| u.is_object()) {
            usage = Some(direct);
            found = true;
        }
    }
    if let Some(pieces) = data.get("stream").and_then(Value::as_array) {
        for piece in pieces {
            let Some(chunk) = piece.get("chunk").filter(|c| c.is_object()) else {
                continue;
            };
            match chunk.get("type") {
                None => continue,
                Some(Value::String(tag)) if tag == "usage" => {}
                Some(Value::String(_)) => continue,
                // 主干在此抛异常 ⇒ 整帧丢弃（见函数文档）。
                Some(_) => return PressureProbe::Poisoned,
            }
            if let Some(cu) = chunk.get("usage").filter(|u| u.is_object()) {
                usage = Some(cu);
                found = true;
            }
        }
    }
    let Some(usage) = usage.filter(|_| found) else {
        return PressureProbe::Absent;
    };
    // 主干 `CM:118-120`：三项各自 `?? 0` 再相加。
    PressureProbe::Tokens(
        PRESSURE_USAGE_FIELDS
            .iter()
            .map(|field| usage_long(usage, field).unwrap_or(0))
            .sum(),
    )
}

/// [`pressure_tokens_of`] 的三态返回，对应主干的三条出口。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PressureProbe {
    /// 采到了（值可以是 0 —— 全零 usage 是合法采样，主干照样写进状态）。
    Tokens(i64),
    /// 没采到：`data.usage` 缺失/非对象且 stream 里没有 usage chunk（`CM:114-117`）。
    Absent,
    /// 帧内有毒数据（`chunk.type` 非字符串）⇒ 主干抛异常、整帧作废（`CM:79` + `CM:106`）。
    Poisoned,
}

impl ContextMeterState {
    /// 折叠一条 journal 事件，对应主干 `TrackContextMeter`（`CM:47-80`）的状态改动那半截。
    ///
    /// 返回 `true` = 这枚事件类型被认账，主干会接着调 `UpdateContextMeterUi()`；
    /// `false` = `data` 不是对象（`CM:52-55`）或类型落在 `CM:74` 的 `default: return` ⇒
    /// 主干直接返回、连刷新都不做。**值有没有真的变**不参与返回值：主干是无条件刷新的。
    ///
    /// 会话维度不在这里（主干从 `Volatile.Read(ref _activeSessionId)` 取，`CM:51`），
    /// 由 [`ContextMeterBook::track`] 负责，包括「`sid` 空 ⇒ 整帧丢弃」那一条。
    pub fn apply_event(&mut self, event_type: &str, data: &Value) -> bool {
        if !data.is_object() {
            return false;
        }
        match event_type {
            EVENT_REQUEST_CONTEXT => {
                // 主干 `CM:60-64`：闸门在 `> 0` 的 double 上，写进去的是 `(long)` 截断值。
                // 所以 0 / 负数 / 非数是「忽略」（保留上一份正数），而 `0.5` 这类
                // 「大于 0 但截断成 0」的值会**把上一份正分母覆盖成 0** ⇒ 圈收起。
                let window = data
                    .get("contextWindow")
                    .and_then(Value::as_f64)
                    .filter(|v| *v > 0.0)
                    .map(|v| v as i64);
                if let Some(window) = window {
                    self.context_window = Some(window);
                }
                true
            }
            EVENT_ASSISTANT_MESSAGE => self.apply_probe(pressure_tokens_of(data, true)),
            EVENT_ASSISTANT_ATTEMPT => self.apply_probe(pressure_tokens_of(data, false)),
            _ => false,
        }
    }

    /// 采样落账：`Tokens` 才写（`CM:69-72` 的 `if (pressure is not null)`），
    /// `Absent` / `Poisoned` 都不动状态 —— 后者主干连刷新都不给，但那是调用侧的事。
    fn apply_probe(&mut self, probe: PressureProbe) -> bool {
        if let PressureProbe::Tokens(tokens) = probe {
            self.pressure_tokens = Some(tokens);
        }
        true
    }

    /// 占用比例 = 主干 `ContextOccupancy`（`CM:130-133`）：**分子分母齐备且分母 > 0** 才给数。
    #[must_use]
    pub fn occupancy(&self) -> Option<ContextOccupancy> {
        occupancy(self)
    }

    /// 「数据不齐 ⇒ 隐藏」这条判据（`MCS`/`CM:162-164`：`visible = occ is not null`）。
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.occupancy().is_some()
    }
}

// ---------------------------------------------------------------- 占用比例

/// 一次占用读数 = 主干 `(long Used, long Window, int Percent)?`（`CM:130`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextOccupancy {
    /// 分子（裸 token 数，未夹紧：可以大于 [`ContextOccupancy::window`]）。
    pub used: i64,
    /// 分母（保证 > 0 —— 见 [`occupancy`]）。
    pub window: i64,
    /// 0..=100 的整数百分比，口径见 [`percent_of`]。
    pub percent: i32,
}

/// 分子/分母 → 百分比：主干 `CM:132` 的
/// `Math.Min(100, (int)Math.Round(100.0 * used / win, MidpointRounding.AwayFromZero))`。
///
/// 逐格抄：`100.0 * used / win` 的**乘法先于除法**（先 `100.0*used` 再除 `win`，与主干同一
/// 结合序 ⇒ 同一舍入前像），四舍五入按「半数远离零」，最后 `min(100)` 封顶。
///
/// 为什么不用整数精确舍入（`(200*used + win) / (2*win)`）：那是**数学**真值，而主干算的是
/// double 商的最近值。两者只在「真值略小于 .5 但 double 商恰好落在 .5」这种亚 ulp 场景分岔
/// （需要 1e16 量级的 token 数才碰得到），本仓口径要求跟主干而不是跟数学 ⇒ 走 double。
#[must_use]
pub fn percent_of(used: i64, window: i64) -> i32 {
    let raw = 100.0 * used as f64 / window as f64;
    round_half_away_from_zero(raw).min(FULL_PERCENT as i64) as i32
}

/// `Math.Round(value, MidpointRounding.AwayFromZero)`：整数部分截断，余数 **>= 0.5 就远离零**。
/// 负数侧同样远离零（`-0.5 → -1`），与 .NET 同向；主干拿不到负数（分子是非负和），
/// 但这里不靠那个前提。
#[must_use]
pub fn round_half_away_from_zero(value: f64) -> i64 {
    let truncated = value.trunc();
    let residual = (value - truncated).abs();
    let magnitude = if residual >= 0.5 {
        truncated.abs() + 1.0
    } else {
        truncated.abs()
    };
    (if value < 0.0 { -magnitude } else { magnitude }) as i64
}

/// 主干 `ContextOccupancy`（`CM:130-133`）的自由函数形态。
///
/// 判据：`st is { PressureTokens: { } used, ContextWindow: { } win } && win > 0` ——
/// 分子先读、分母后读，但两者都是 `Option`，读序不影响结果。
/// 分母 <= 0 的守卫在这里（状态本身只收 `> 0` 的 double，截断后仍可能出现 0，见
/// [`ContextMeterState::apply_event`] 的 `0.5` 那案）。
#[must_use]
pub fn occupancy(state: &ContextMeterState) -> Option<ContextOccupancy> {
    let used = state.pressure_tokens?;
    let window = state.context_window.filter(|w| *w > 0)?;
    Some(ContextOccupancy {
        used,
        window,
        percent: percent_of(used, window),
    })
}

// ---------------------------------------------------------------- 弧面三态与几何

/// 环面三态 = 主干 `SetContextMeterRing`（`CM:184-211`）里那两段 `Visibility` + 一段几何：
/// 圆环（轨道 `Ellipse`）恒在，**已用弧 / 整圆 / 什么都不加**三选一。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RingFace {
    /// `percent == 0`：弧 `Collapsed`、整圆也 `Collapsed` ⇒ 只剩轨道（`CM:187-192`）。
    /// 注意这**不是**「隐藏整颗圈」——`used = 0` 是合法读数，圈照旧在场（`CM:163`）。
    TrackOnly,
    /// `0 < percent < 100`：画扫过弧（`CM:187` 的 `p is > 0 and < 100`）。
    Arc(ArcGeometry),
    /// `percent >= 100`：整圆（`CM:189` 的 `p >= 100`）。主干走另一颗 `Ellipse`，
    /// 因为「弧起终重合」是退化几何、渲染结果不确定（`CM:182-183`）。
    Full,
}

/// 一段 `ArcSegment`，字段与主干 `CM:195-207` 一一对应。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcGeometry {
    /// 弧起点：恒在 12 点 = `(7, 7 - 6)`（`CM:197`）。
    pub start_x: f64,
    pub start_y: f64,
    /// 弧终点：`(7 + 6·sin θ, 7 − 6·cos θ)`（`CM:201-203`）。
    pub end_x: f64,
    pub end_y: f64,
    /// `Size(6, 6)`（`CM:204`）。
    pub radius_x: f64,
    pub radius_y: f64,
    /// `IsLargeArc = angle > π`，即扫过 > 180°（`CM:206`）。恰好 180°（percent = 50）不算大弧。
    pub large_arc: bool,
    /// 扫过角（弧度）= `percent × 3.6°`（`CM:194` 的 `p * Math.PI / 50.0`）。
    /// 主干 `SweepDirection` 恒为 `Clockwise`（`CM:205`），所以这里不再带方向布尔。
    pub sweep_radians: f64,
}

/// 主干 `CM:186` 的 `Math.Clamp(percent, 0, 100)`。
#[must_use]
pub fn clamp_percent(percent: i32) -> i32 {
    percent.clamp(0, FULL_PERCENT)
}

/// 扫过角（弧度）：`p × π / 50`（`CM:194`），乘法先于除法以复现同一浮点结合序。
#[must_use]
pub fn sweep_radians(percent: i32) -> f64 {
    clamp_percent(percent) as f64 * std::f64::consts::PI / 50.0
}

/// percent → 环面。分档阈值只有 0 与 100 两道（主干没有按颜色分档：弧与整圆的描边
/// 都是同一枚 `TextTertiaryBrush`，`MX:1321-1330`）。
#[must_use]
pub fn face_of(percent: i32) -> RingFace {
    let p = clamp_percent(percent);
    if p >= FULL_PERCENT {
        return RingFace::Full;
    }
    if p <= 0 {
        return RingFace::TrackOnly;
    }
    let angle = sweep_radians(p);
    RingFace::Arc(ArcGeometry {
        start_x: RING_CENTER,
        start_y: RING_CENTER - RING_RADIUS,
        end_x: RING_CENTER + RING_RADIUS * angle.sin(),
        end_y: RING_CENTER - RING_RADIUS * angle.cos(),
        radius_x: RING_RADIUS,
        radius_y: RING_RADIUS,
        large_arc: angle > std::f64::consts::PI,
        sweep_radians: angle,
    })
}

// ---------------------------------------------------------------- 按会话的账本

/// 会话 → 容量折叠，对应主干的 `_contextMeters`（`CM:34`，`StringComparer.Ordinal`）。
/// Rust 的 `HashMap<String, _>` 按字节相等 ⇒ 与 Ordinal 同口径（大小写敏感、不做规范化）。
#[derive(Clone, Debug, Default)]
pub struct ContextMeterBook {
    meters: HashMap<String, ContextMeterState>,
}

impl ContextMeterBook {
    /// 空账本。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 折叠入口 = 主干 `TrackContextMeter`（`CM:47-80`）的可测形态。
    ///
    /// `session_id` 为空 ⇒ 整帧丢弃并返回 `false`（`CM:51-55` 的
    /// `string.IsNullOrEmpty(sid) ⇒ return`）——**不会**建出一格空状态，这点与主干一致
    /// （主干在建格之前就返回了）。
    /// 返回值的其余语义见 [`ContextMeterState::apply_event`]。
    pub fn track(&mut self, session_id: &str, event_type: &str, data: &Value) -> bool {
        if session_id.is_empty() {
            return false;
        }
        self.meters
            .entry(session_id.to_string())
            .or_default()
            .apply_event(event_type, data)
    }

    /// 取某会话的折叠状态（`CM:161` 的 `TryGetValue`：没有那一格就是没数据）。
    #[must_use]
    pub fn state(&self, session_id: &str) -> Option<ContextMeterState> {
        self.meters.get(session_id).copied()
    }

    /// 某会话的占用读数；没有该会话的格子 ⇒ `None`（= 隐藏）。
    #[must_use]
    pub fn occupancy(&self, session_id: &str) -> Option<ContextOccupancy> {
        self.state(session_id).and_then(|state| occupancy(&state))
    }

    /// 某会话的圈在不在场（`CM:163`）。
    #[must_use]
    pub fn is_visible(&self, session_id: &str) -> bool {
        self.occupancy(session_id).is_some()
    }

    /// 会话切换清零 = 主干 `ResetContextMeter`（`CM:137-145`）：**整格删掉**，
    /// 状态由随后那趟完整 transcript 回放重建（`MCS:4502`）。
    ///
    /// 返回 `true` = 确实删掉了一格。主干不看这个返回值、无条件刷新 UI，
    /// 所以调用侧也要无条件刷新（不刷新的话上个会话的圈会留在屏幕上）。
    pub fn reset(&mut self, session_id: &str) -> bool {
        if session_id.is_empty() {
            return false;
        }
        self.meters.remove(session_id).is_some()
    }

    /// 账本里的会话格数（主干 `CM:36-44` 的 `GetOrCreate` 会为任何非空 sid 建格，
    /// 包括事件类型不认账的那些 —— 分叉照抄，别拿 `len() == 有数据的会话数` 做假设）。
    #[must_use]
    pub fn len(&self) -> usize {
        self.meters.len()
    }

    /// 见 [`ContextMeterBook::len`]。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.meters.is_empty()
    }
}

// ---------------------------------------------------------------- 文案算子

/// tooltip / 无障碍名：主干 `CM:168-170` 把**同一枚串**同时发给
/// `ToolTipService.SetToolTip` 与 `AutomationProperties.SetName`，所以这里只出一个函数。
/// 走 `crate::i18n::Catalog::lf`（模板键见 [`ARIA_TEMPLATE`]）。
#[must_use]
pub fn tooltip_and_aria(catalog: &Catalog, occupancy: ContextOccupancy) -> String {
    catalog.lf(ARIA_TEMPLATE, &[occupancy.percent.to_string()])
}

/// 数据不齐时留在无障碍树里的静态名（`MX:1314`，见 [`ARIA_FALLBACK`]）。
#[must_use]
pub fn static_aria(catalog: &Catalog) -> String {
    catalog.l(ARIA_FALLBACK)
}

// ---------------------------------------------------------------- 离线单测

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 主干 `CM:59-64` 那一条 `request/context`。
    fn context_event(window: Value) -> Value {
        json!({ "turn": 1, "provider": "openai", "model": "gpt-x", "contextWindow": window })
    }

    /// 主干 `CM:118-120` 的三项之和（`outputTokens` 故意塞个显眼的值，用它被漏进来就能看见）。
    fn usage(input: i64, cache_read: i64, cache_write: i64) -> Value {
        json!({
            "inputTokens": input,
            "outputTokens": 999,
            "cacheReadTokens": cache_read,
            "cacheWriteTokens": cache_write
        })
    }

    fn approx(actual: f64, expect: f64) -> bool {
        (actual - expect).abs() < 1e-9
    }

    // ------------------------------------------------------------ 可见性真值表（数据不齐 ⇒ 隐藏）

    #[test]
    fn no_sample_and_no_capacity_hides_the_ring() {
        let st = ContextMeterState::default();
        assert_eq!(st.pressure_tokens, None);
        assert_eq!(st.context_window, None);
        assert_eq!(occupancy(&st), None);
        assert!(!st.is_visible(), "CM:130-133：两个字段都不齐 ⇒ 不画");
    }

    #[test]
    fn sample_without_capacity_hides_the_ring() {
        let mut st = ContextMeterState::default();
        assert!(st.apply_event(
            EVENT_ASSISTANT_MESSAGE,
            &json!({ "usage": usage(1_000, 0, 0) })
        ));
        assert_eq!(st.pressure_tokens, Some(1_000));
        assert_eq!(st.context_window, None);
        assert_eq!(occupancy(&st), None);
        assert!(!st.is_visible(), "只有分子 ⇒ CM:163 的 visible = false");
    }

    #[test]
    fn capacity_without_sample_hides_the_ring() {
        let mut st = ContextMeterState::default();
        assert!(st.apply_event(EVENT_REQUEST_CONTEXT, &context_event(json!(200_000))));
        assert_eq!(st.context_window, Some(200_000));
        assert_eq!(st.pressure_tokens, None);
        assert_eq!(occupancy(&st), None);
        assert!(!st.is_visible());
    }

    #[test]
    fn zero_and_negative_capacity_are_ignored_and_keep_the_ring_hidden() {
        // 主干 CM:61 的 `cw.GetDouble() > 0`：0 与负数**根本不写进状态**。
        let mut st = ContextMeterState::default();
        for window in [json!(0), json!(-5), json!("200000"), json!(null)] {
            assert!(st.apply_event(EVENT_REQUEST_CONTEXT, &context_event(window.clone())));
            assert_eq!(st.context_window, None, "非正/非数分母被忽略：{window}");
            assert!(!st.is_visible());
        }
    }

    #[test]
    fn sub_one_capacity_overwrites_a_good_denominator_with_zero() {
        // 主干 CM:61-63 的口径分岔：闸门看 double（0.5 > 0 过），写入的是 `(long)` 截断（= 0），
        // 于是上一份正分母被 0 覆盖 ⇒ occupancy 的 `win > 0` 挡下 ⇒ 圈收起。
        let mut st = ContextMeterState::default();
        st.apply_event(EVENT_REQUEST_CONTEXT, &context_event(json!(100_000)));
        st.apply_event(
            EVENT_ASSISTANT_MESSAGE,
            &json!({ "usage": usage(1_200, 0, 0) }),
        );
        assert!(st.is_visible());
        st.apply_event(EVENT_REQUEST_CONTEXT, &context_event(json!(0.5)));
        assert_eq!(st.context_window, Some(0));
        assert_eq!(occupancy(&st), None);
        assert!(!st.is_visible(), "截断成 0 的分母 ⇒ CM:131 的 win > 0 不成立");
    }

    #[test]
    fn fractional_denominator_truncates_toward_zero() {
        let mut st = ContextMeterState::default();
        st.apply_event(
            EVENT_REQUEST_CONTEXT,
            &context_event(json!(200_000.9)),
        );
        assert_eq!(st.context_window, Some(200_000), "CM:63 的 (long) 是截断不是四舍五入");
    }

    #[test]
    fn zero_used_keeps_the_ring_visible_with_a_track_only_face() {
        // CM:131 的 `PressureTokens: { } used` 只看「采到没采到」，0 是采到了。
        let mut st = ContextMeterState::default();
        st.apply_event(EVENT_REQUEST_CONTEXT, &context_event(json!(100_000)));
        st.apply_event(EVENT_ASSISTANT_MESSAGE, &json!({ "usage": usage(0, 0, 0) }));
        assert_eq!(
            occupancy(&st),
            Some(ContextOccupancy {
                used: 0,
                window: 100_000,
                percent: 0
            })
        );
        assert!(st.is_visible());
        assert_eq!(face_of(0), RingFace::TrackOnly, "0% 收起弧与整圆，但圈还在");
    }

    // ------------------------------------------------------------ 百分比：四舍五入与封顶

    #[test]
    fn percent_rounds_half_away_from_zero() {
        // 主干 CM:132 的 MidpointRounding.AwayFromZero，逐档钉一遍。
        assert_eq!(percent_of(1, 8), 13, "12.5 → 13（正好一半远离零）");
        assert_eq!(percent_of(3, 8), 38, "37.5 → 38");
        assert_eq!(percent_of(1, 200), 1, "0.5 → 1");
        assert_eq!(percent_of(1, 300), 0, "0.333 → 0（有读数、但弧收起）");
        assert_eq!(percent_of(1, 3), 33, "33.33 → 33");
        assert_eq!(percent_of(2, 3), 67, "66.67 → 67");
        assert_eq!(percent_of(49_999, 100_000), 50, "49.999 → 50");
    }

    #[test]
    fn over_capacity_caps_at_one_hundred() {
        let mut st = ContextMeterState::default();
        st.apply_event(EVENT_REQUEST_CONTEXT, &context_event(json!(200_000)));
        st.apply_event(
            EVENT_ASSISTANT_MESSAGE,
            &json!({ "usage": usage(300_000, 50_000, 0) }),
        );
        let occ = occupancy(&st).expect("分子分母齐备");
        assert_eq!(occ.used, 350_000, "分子是未夹紧的和，CM:132 只夹 percent");
        assert_eq!(occ.percent, 100);
        assert_eq!(face_of(occ.percent), RingFace::Full);
    }

    #[test]
    fn exactly_full_reads_one_hundred() {
        let mut st = ContextMeterState::default();
        st.apply_event(EVENT_REQUEST_CONTEXT, &context_event(json!(128_000)));
        st.apply_event(
            EVENT_ASSISTANT_MESSAGE,
            &json!({ "usage": usage(128_000, 0, 0) }),
        );
        assert_eq!(
            occupancy(&st),
            Some(ContextOccupancy {
                used: 128_000,
                window: 128_000,
                percent: 100
            })
        );
    }

    #[test]
    fn rounding_helper_mirrors_the_trunk_midpoint_rule() {
        assert_eq!(round_half_away_from_zero(0.5), 1);
        assert_eq!(round_half_away_from_zero(1.5), 2);
        assert_eq!(round_half_away_from_zero(2.4999), 2);
        assert_eq!(round_half_away_from_zero(-0.5), -1, "远离零，不是 toward 负无穷");
        assert_eq!(round_half_away_from_zero(-1.5), -2);
    }

    // ------------------------------------------------------------ 弧几何与分档

    #[test]
    fn arc_starts_at_twelve_and_sweeps_clockwise() {
        let RingFace::Arc(g) = face_of(25) else {
            panic!("25% 应是弧");
        };
        assert_eq!((g.start_x, g.start_y), (7.0, 1.0), "CM:197 恒在 12 点");
        assert!(approx(g.end_x, 13.0) && approx(g.end_y, 7.0), "25% → 3 点");
        assert!(approx(g.sweep_radians, std::f64::consts::FRAC_PI_2));
        assert!(!g.large_arc);
        assert_eq!((g.radius_x, g.radius_y), (6.0, 6.0));
    }

    #[test]
    fn large_arc_flag_flips_just_past_one_eighty() {
        // CM:206 的 `IsLargeArc = angle > π`：percent = 50 正好 π ⇒ 不算大弧。
        let RingFace::Arc(at_50) = face_of(50) else {
            panic!("50% 应是弧");
        };
        assert!(!at_50.large_arc, "180° 整不是大弧（CM:206 用的是严格 >）");
        assert!(approx(at_50.end_x, 7.0) && approx(at_50.end_y, 13.0), "50% → 6 点");
        let RingFace::Arc(at_51) = face_of(51) else {
            panic!("51% 应是弧");
        };
        assert!(at_51.large_arc, "183.6° 起才是大弧");
        assert!(approx(at_51.sweep_radians, 51.0 * std::f64::consts::PI / 50.0));
    }

    #[test]
    fn arc_boundaries_pick_the_three_faces() {
        let at_one = face_of(1);
        assert!(matches!(at_one, RingFace::Arc(_)), "1% 仍是弧");
        assert!(matches!(face_of(99), RingFace::Arc(_)));
        assert_eq!(face_of(0), RingFace::TrackOnly);
        assert_eq!(face_of(100), RingFace::Full);
        // CM:186 的 Math.Clamp 兜住越界输入（occupancy 给不出越界值，但调用侧可能手搓）。
        assert_eq!(face_of(-7), RingFace::TrackOnly);
        assert_eq!(face_of(4000), RingFace::Full);
        assert_eq!(clamp_percent(-7), 0);
        assert_eq!(clamp_percent(4000), 100);
    }

    #[test]
    fn sweep_is_three_point_six_degrees_per_percent() {
        assert!(approx(sweep_radians(100), std::f64::consts::TAU), "100% = 一整圈");
        assert!(approx(sweep_radians(1), std::f64::consts::PI / 50.0));
    }

    // ------------------------------------------------------------ 折叠：usage 采样口径

    #[test]
    fn assistant_message_reads_direct_usage_then_lets_stream_win() {
        // CM:91-95 起步 + CM:96-112 覆盖（后到者胜）。
        let data = json!({
            "usage": usage(100, 0, 0),
            "stream": [
                { "chunk": { "type": "usage", "usage": usage(200, 30, 0) } },
                { "chunk": { "type": "text", "text": "hi" } },
                { "chunk": { "type": "usage", "usage": usage(300, 40, 5) } }
            ]
        });
        assert_eq!(
            pressure_tokens_of(&data, true),
            PressureProbe::Tokens(345),
            "最后一枚 usage chunk 赢：300 + 40 + 5"
        );
        let mut st = ContextMeterState::default();
        assert!(st.apply_event(EVENT_ASSISTANT_MESSAGE, &data));
        assert_eq!(st.pressure_tokens, Some(345));
    }

    #[test]
    fn assistant_attempt_ignores_direct_usage() {
        // CM:68：`readDirectUsage` 只对 assistant/message 为真。
        let data = json!({ "usage": usage(100, 0, 0) });
        assert_eq!(pressure_tokens_of(&data, false), PressureProbe::Absent);
        assert_eq!(pressure_tokens_of(&data, true), PressureProbe::Tokens(100));
        let mut st = ContextMeterState::default();
        assert!(st.apply_event(EVENT_ASSISTANT_ATTEMPT, &data));
        assert_eq!(st.pressure_tokens, None, "attempt 帧不许写分子（CM:69 的 is not null）");
    }

    #[test]
    fn attempt_still_reads_stream_usage_chunks() {
        let data = json!({
            "usage": usage(999, 999, 999),
            "stream": [ { "chunk": { "type": "usage", "usage": usage(7, 3, 2) } } ]
        });
        assert_eq!(pressure_tokens_of(&data, false), PressureProbe::Tokens(12));
    }

    #[test]
    fn missing_and_negative_usage_fields_count_as_zero() {
        // CM:118-120 的 `?? 0` + CM:124 的 `>= 0` 闸门。
        let data = json!({ "usage": { "inputTokens": 10, "cacheReadTokens": -3 } });
        assert_eq!(pressure_tokens_of(&data, true), PressureProbe::Tokens(10));
        assert_eq!(pressure_tokens_of(&json!({}), true), PressureProbe::Absent);
        assert_eq!(
            pressure_tokens_of(&json!({ "usage": "none" }), true),
            PressureProbe::Absent,
            "非对象的 data.usage 不算采样（CM:91）"
        );
        assert_eq!(usage_long(&json!({ "inputTokens": 3.9 }), "inputTokens"), Some(3));
        assert_eq!(usage_long(&json!({}), "inputTokens"), None);
    }

    #[test]
    fn a_non_string_chunk_type_voids_the_whole_frame() {
        // CM:106 的 ct.GetString() 在非字符串 type 上抛异常 ⇒ CM:79 吞掉 ⇒ 状态不动。
        let data = json!({
            "usage": usage(100, 0, 0),
            "stream": [ { "chunk": { "type": 7, "usage": usage(500, 0, 0) } } ]
        });
        assert_eq!(pressure_tokens_of(&data, true), PressureProbe::Poisoned);
        let mut st = ContextMeterState::default();
        st.pressure_tokens = Some(1);
        st.apply_event(EVENT_ASSISTANT_MESSAGE, &data);
        assert_eq!(st.pressure_tokens, Some(1), "整帧作废，连 data.usage 都不写");
    }

    #[test]
    fn unhandled_event_types_touch_neither_state_nor_ui() {
        let mut st = ContextMeterState::default();
        st.apply_event(EVENT_REQUEST_CONTEXT, &context_event(json!(100_000)));
        for type_name in ["turn/end", "session/update", "system/message", ""] {
            assert!(
                !st.apply_event(type_name, &json!({ "contextWindow": 1 })),
                "CM:74 default: return ⇒ {type_name} 不认账"
            );
        }
        assert_eq!(st.context_window, Some(100_000));
        // data 不是对象（含 null / 数组）也不认账（CM:52-55）。
        assert!(!st.apply_event(EVENT_REQUEST_CONTEXT, &json!(null)));
        assert!(!st.apply_event(EVENT_ASSISTANT_MESSAGE, &json!([1, 2])));
    }

    // ------------------------------------------------------------ 按会话：隔离与复位

    #[test]
    fn book_isolates_sessions_and_drops_an_empty_id() {
        let mut book = ContextMeterBook::new();
        assert!(!book.track("", EVENT_REQUEST_CONTEXT, &context_event(json!(9)))) ;
        assert_eq!(book.len(), 0, "CM:51-55：空 sid 连格子都不建");
        assert!(book.track("s-a", EVENT_REQUEST_CONTEXT, &context_event(json!(100_000))));
        assert!(book.track("s-a", EVENT_ASSISTANT_MESSAGE, &json!({ "usage": usage(25_000, 0, 0) })));
        assert!(book.track("s-B", EVENT_REQUEST_CONTEXT, &context_event(json!(8_000))));
        assert!(book.is_visible("s-a"));
        assert!(!book.is_visible("s-B"), "B 只有分母");
        assert_eq!(book.occupancy("s-a").map(|o| o.percent), Some(25));
        assert_eq!(book.occupancy("s-B"), None);
        assert_eq!(book.occupancy("missing"), None, "没这一格 = 没数据 = 隐藏");
        // Ordinal 比较：大小写敏感（CM:34 的 StringComparer.Ordinal）。
        assert!(!book.is_visible("S-A"));
    }

    #[test]
    fn reset_drops_the_session_and_the_ring_goes_away() {
        // MCS:4502：切会话先清零，再由回放重建 —— 不残留上个会话。
        let mut book = ContextMeterBook::new();
        book.track("old", EVENT_REQUEST_CONTEXT, &context_event(json!(100_000)));
        book.track("old", EVENT_ASSISTANT_MESSAGE, &json!({ "usage": usage(90_000, 0, 0) }));
        assert!(book.is_visible("old"));
        assert!(book.reset("old"));
        assert!(!book.is_visible("old"), "复位后必须没有残留读数");
        assert_eq!(book.occupancy("old"), None);
        assert_eq!(book.state("old"), None, "整格删掉，不是把字段刷成 0");
        assert!(!book.reset("old"), "第二次没有可删的了");
        assert!(!book.reset(""), "空 sid 不动账本（CM:140）");
        assert_eq!(book.len(), 0);
    }

    #[test]
    fn replay_after_reset_is_last_wins_and_idempotent() {
        // CM:22-23：last-wins 无累计 ⇒ 同一趟回放喂两遍不会翻倍。
        let frames: Vec<(&str, Value)> = vec![
            (EVENT_REQUEST_CONTEXT, context_event(json!(200_000))),
            (
                EVENT_ASSISTANT_MESSAGE,
                json!({ "usage": usage(1_000, 500, 0) }),
            ),
            (EVENT_REQUEST_CONTEXT, context_event(json!(128_000))),
            (
                "assistant/attempt",
                json!({ "stream": [ { "chunk": { "type": "usage", "usage": usage(9_000, 0, 0) } } ] }),
            ),
        ];
        let mut book = ContextMeterBook::new();
        for (type_name, data) in &frames {
            book.track("s", type_name, data);
        }
        let once = book.occupancy("s").expect("回放后有读数");
        assert_eq!(
            once,
            ContextOccupancy {
                used: 9_000,
                window: 128_000,
                percent: percent_of(9_000, 128_000)
            },
            "后到的 request/context 与 attempt 采样各自覆盖前一枚"
        );
        // 切会话：清零 + 重放同一趟 ⇒ 读数一模一样（不残留、也不翻倍）。
        book.reset("s");
        assert!(!book.is_visible("s"));
        for (type_name, data) in &frames {
            book.track("s", type_name, data);
        }
        assert_eq!(book.occupancy("s"), Some(once));
    }

    #[test]
    fn track_returns_false_for_an_unhandled_frame_even_through_the_book() {
        let mut book = ContextMeterBook::new();
        assert!(!book.track("s", "turn/end", &json!({ "x": 1 })));
        assert_eq!(book.len(), 1, "格子照样建（主干 GetOrCreate 在类型判断之前）");
        assert_eq!(book.state("s"), Some(ContextMeterState::default()));
    }

    // ------------------------------------------------------------ 文案

    #[test]
    fn tooltip_and_aria_share_one_string_from_the_trunk_template() {
        let zh = Catalog::load("zh", None);
        let occ = ContextOccupancy {
            used: 37_500,
            window: 100_000,
            percent: 38,
        };
        assert_eq!(tooltip_and_aria(&zh, occ), "上下文已用 38%");
        assert_eq!(static_aria(&zh), "上下文容量");
        // 非中文语种走 i18n::EN 既有两条键（i18n.rs:1094 / :1096），本轮不需要新键。
        let en = Catalog::load("en", None);
        assert_eq!(tooltip_and_aria(&en, occ), "38% of context used");
        assert_eq!(static_aria(&en), "Context capacity");
    }

    #[test]
    fn width_budget_follows_visibility() {
        assert_eq!(row_reserve(true), 36.0, "MCS:9631：圈 28 + 一条 8dip 列间距");
        assert_eq!(row_reserve(false), 0.0);
        assert_eq!(RING_CENTER * 2.0, RING_BOX);
        assert_eq!(RING_BUTTON, 28.0);
    }
}
