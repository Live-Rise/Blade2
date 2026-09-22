use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use blade2_rs::i18n::Catalog;
use blade2_rs::i18n::relative_time;
use blade2_rs::keys::{self, KeyAction, KeyOwner};
use blade2_rs::kernel::{
    CONTROL_FRAME_TYPES, CommandEntry, CommandReply, ControlState, Kernel, Launch, SessionInfo,
    WorkspaceTree, control_frame_type, escape_query, session_status_event,
};
use blade2_rs::mux::{Mux, MuxEvent};
use blade2_rs::theme::{
    self, BubbleMaterial, Palette, Scheme, WindowMaterial,
};
use blade2_rs::tokens::{
    DEFAULT_SETTINGS_SECTION, DESC_ABOUT, DESC_AGENT_PRESETS, DESC_BROWSER_CONTROL,
    DESC_COMPUTER_CONTROL, DESC_MEMORY, DESC_MODELS, DESC_PERSONALIZATION, DESC_PET, DESC_SKILLS,
    SETTINGS_LOCALES, SETTINGS_MATERIALS, SETTINGS_PERMISSIONS, SETTINGS_PLUGIN_CARDS,
    SETTINGS_SECTIONS, SETTINGS_THEMES, STATS_KPI_CARDS, TOOL_TABLE, boot as kernel_boot, camel,
    glyph, pad, radius, size, space, type_ramp,
};
use serde_json::{Map, Value, json};
use windows_reactor::*;

mod clipboard;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Chat,
    Settings,
}

#[derive(Clone)]
enum Msg {
    Nav(Page),
    /// 内核起来并带回会话台账（`start_kernel` 的后台结果，也是左栏「新会话」之外的复位点）。
    Connected(Arc<Mutex<Kernel>>, Vec<SessionInfo>),
    /// 引导失败 = 主线 `FailKernelBoot(reasonKey, detail)`（KC:138-156）：
    /// 前者是可本地化的失败定性（`内核启动失败` / `未找到内置内核；…`），后者是异常原文。
    Failed(String, String),
    /// 左栏「新会话」（主干 `NewSessionItem` → `CreateSessionAsync`）。
    NewSession,
    Input(String),
    Search(String),
    Scheme(Scheme),
    Select(String),
    Hover(Option<String>),
    /// 圆底钮**自己那一颗**的悬停档（`Some(key)` = 指针正在这颗钮上）。不能复用 `Hover`：
    /// 气泡卡的 Border 与钮内层 Border 都听 `PointerEntered`，冒泡顺序是内→外，同一个槽
    /// 会被外层那一枪覆盖 ⇒ 钮永远画不出悬停底（反馈钮的 hover 档就丢了）。
    PillHover(Option<String>),
    /// 圆底图标钮的按下态（`Some(id)` = 那颗钮正被按住）。主干由 Button 模板的
    /// Pressed 视觉态自己管，分叉的圆底是自己画的 ⇒ 状态得回到模型里，见 `pill_button`。
    Press(Option<String>),
    ToggleGroup(String),
    Cap(String, bool),
    Section(String),
    /// 插件配置页钻取：`Some(id)` 时设置页头换成二级页标题 + 返回钮（主干 `OpenSettingsSubPage`）。
    OpenSub(String),
    /// 返回上一级（主干 `CloseSettingsSubPage`；主干还绑在 Esc 上，分叉没有键事件）。
    CloseSub,
    Field(String, String),
    Number(String, f64),
    FontStep(f64),
    Pick(String, usize),
    Toggle(String, bool),
    Note(String),
    /// 操作行「复制消息」：(气泡键, 原文)。主干用 DispatcherQueue 定时器 1 秒后换回图标，
    /// reactor 没有定时器可借，改成指针离开该行时回落。
    Copy(String, String),
    /// 操作行「在新对话中分支」：主干 `session/fork` 的 `atSeq` 锚点。
    Branch(i64),
    /// 分叉回来的结果：主干 `RefreshSessionsAsync` 刷出的整张会话台账 + 子会话 id + 改名后的
    /// 标题（空串 = 源会话无标题或改名失败，主干两种都静默）。
    Branched(Result<(Vec<SessionInfo>, String, String), String>),
    /// 思考段折叠（主干 `ChatBubble.ReasoningExpanded`，默认收起）。
    Reason(String, bool),
    /// 交付物卡上的「打开 / 显示」：(交付事件的 seq, files 下标, open|reveal)。
    /// 主干走宿主路由 `POST /api/present.open`，不是 RPC，坐标三元组缺一不可。
    Present(i64, usize, String),
    /// 宿主路由回的状态码 + 正文（Err 是连不上这类传输层问题）。
    Presented(Result<(u16, String), String>),
    /// 轮尾「本轮文件改动」chip：把工作区路径交给宿主桌面（内核 `session/openWorkspacePath`）。
    OpenPath(String),
    /// 上面那条 RPC 的结果（成功主干什么都不说）。
    Opened(Result<(), String>),
    /// 反馈组赞/踩：(内核 message.id, `positive`|`negative`, 切换后的 IsChecked)。
    /// 主干 `RateAsync` 的「同评级即撤销」在这里改成 ToggleButton 的原生语义：
    /// `checked=true` ⇒ put、`checked=false` ⇒ delete，见 `update` 里那个幂等守卫。
    Rate(String, String, bool),
    /// 反馈组「撤销」：主干 `DeleteFeedbackAsync(messageId)`。
    Revoke(String),
    /// 反馈组「添加/编辑说明」：打开 `ContentDialog` 说明编辑器（主干 `EditFeedbackNoteAsync`）。
    OpenNote(String),
    /// 说明编辑器的当前文本（主干直接读 TextBox.Text，分叉得走消息）。
    NoteText(String),
    /// 说明编辑器关闭：`true` = 按了「保存」（主干只认 `ContentDialogResult.Primary`）。
    CloseNote(bool),
    /// `messageFeedback/list` 回灌：Ok(会话 id, items)，失败（含业务 ok:false）主干一律静默。
    FeedbackList(Result<(String, Vec<Value>), String>),
    /// 一次 put/delete 状态机（含冲突采纳重试、delete 前的 list 对齐）在后台跑完的落点。
    FeedbackApplied(FeedbackPatch),
    Send,
    Cancel,
    /// 主干 `Ctrl+Enter`（`TryHandleEnterSend` 的 ctrl 分支，MW:6856-6860）：空闲时等价于默认发送，
    /// 繁忙时取 `busyEnter` 的**反向档**。按键侧只产出「这是反向档那一发」（`keys::KeyAction`），
    /// 到底取哪个 mode 由 `ctrl_enter_mode()` 在模型里判（复用既有的 `running` 表与 `busy` 下拉，
    /// 不新造状态机）。
    SendAlternate,
    /// `commands/list` 的回填（主干 `EnsureCommandsAsync` 里那条 await，MW:17326）。
    /// `generation` = 发起那一发按键时的 `_paletteGeneration`：主干在 await 之后先比代次、
    /// 不符就整段丢弃（MW:17701「已有更新的按键」），但**缓存照写**（写缓存在代次判定之前）。
    CommandsLoaded {
        generation: u64,
        session: String,
        outcome: Result<Vec<CommandEntry>, String>,
    },
    /// `commands/execute` 的回填（主干 17889-17914 的四态 + `catch`）。
    /// `draft` = 主干那句 `submittedText`（只有 composer 那一发带得回来：命令成功且用户期间
    /// 没改过草稿时，输入框才被清空；从浮层采纳的无参命令不带草稿，见 `execute_command`）。
    CommandExecuted {
        line: String,
        session: String,
        outcome: Result<CommandReply, String>,
        draft: Option<String>,
    },
    /// 浮层 ↑/↓（主干 `HandlePaletteKey` 的 Up/Down → `MoveCommandSelection(±1)`）。
    PaletteMove(i32),
    /// 浮层采纳（Enter 与 Tab 同一发，主干 `AcceptCommandSelection`）。
    PaletteAccept,
    /// 浮层 Esc（主干 `HideCommandPalette`）。
    PaletteClose,
    /// 鼠标点浮层某一行（主干 `OnCommandItemClick`：先把 `_commandIndex` 对齐到点的那行再采纳）。
    PalettePick(usize),
    /// `session/prompt` 的 wire ack；Ok 带实际落到的会话 id 和这一路 follow 的 streamId
    /// （开流失败时 streamId 为空串，由 UI 侧补开）。
    Prompted(Result<(String, String), String>),
    /// `session/follow` 开流结果：Ok(sessionId, streamId)。
    FollowOpened(Result<(String, String), String>),
    /// `Mux` 连上并把三条长驻流开完的结果：`Ok((实例, session/control 的 streamId))`。
    /// 两句必须在同一条消息里落地 ⇒ 不存在「control 帧先到、streamId 还没记下」的窗口
    /// （那条窗口就是 spec6 R1 的「帧被 `WorkspaceTree::apply` 吃掉且不报错」）。
    MuxReady(Result<(Arc<Mutex<Mux>>, String), String>),
    MuxEvents(Vec<MuxEvent>),
    MuxFailed(String),
    /// 主线 `_kernelBootTick`（`DispatcherQueueTimer` 100ms，KC:186-196）在分叉的替身：
    /// `ComponentContext::spawn_background` 自续期（睡 100ms 再投一发）。携带的 `u64` 是
    /// `BootState::tick` 的序号 ⇒ 同一时刻永远只有一发在飞，撤卡/重试后迟到那发被守卫丢掉。
    BootTick(u64),
    /// 主线 `CompleteKernelBootProgress` → `Task.Delay(350ms)` → `HideKernelBootPanel`
    /// （MW:2627-2629）里那一发延时。携带 `BootState::seq`（轮次代号），重试过的轮次不再撤卡。
    BootDone(u64),
    /// 主线 `OnKernelBootRetryClick` → `RestartKernelBootAsync`（KC:270-305）。
    BootRetry,
    /// 「刚才那一下按下，键盘是谁吃的」——焦点归属的**推断**值，喂给 `Shell::key_owner`。
    /// 为什么要有这条：reactor 0.100.0 没有任何键盘回调也没有焦点查询 ⇒ Enter 只能靠
    /// user32 线程钩子拿（见 `blade2_rs::keys`），而钩子拿不到焦点，只能由「最近一次
    /// text-changed / 最近一次按下的那颗 `Border`」推。取 `Composer` 才把 Enter 判成发送。
    KeyFocus(KeyOwner),
}

impl Msg {
    /// 钩子算出的动作 → 消息。`Pass` 不投任何东西（键留给 TextBox / 系统）。
    /// 提交走既有的 `Msg::Send`（= `submit_input`，空文本与内核未起来都自带早退守卫），
    /// 返回上一级走既有的 `Msg::CloseSub`（`self.sub = None`）—— 不新造第二条通路。
    ///
    /// `ReturnToChat` 不同：它是主干 `OnRootPreviewKeyDown`（MW:9245-9276）里
    /// 「设置页可见且没有二级页 ⇒ `ShowChatPage()`」那一发，走的正是返回钮挂的同一条
    /// `Msg::Nav(Page::Chat)`（见 `drag_region` 里那颗 automation_name=「返回」）。
    ///
    /// `Palette*` 三档 = 浮层那几发（主干 `OnInputPreviewKeyDown` 的 `HandlePaletteKey` 优先，
    /// MW:6824），所以这一发**绝不**映射成 `Msg::Send`：浮层开着时 Enter 是采纳，不是发送。
    fn from_key_action(action: KeyAction) -> Option<Msg> {
        match action {
            KeyAction::Submit => Some(Msg::Send),
            KeyAction::SubmitAlternate => Some(Msg::SendAlternate),
            KeyAction::CloseSub => Some(Msg::CloseSub),
            KeyAction::ReturnToChat => Some(Msg::Nav(Page::Chat)),
            KeyAction::PaletteClose => Some(Msg::PaletteClose),
            KeyAction::PaletteMove(delta) => Some(Msg::PaletteMove(delta)),
            KeyAction::PaletteAccept => Some(Msg::PaletteAccept),
            KeyAction::Pass => None,
        }
    }
}

/// 引导线程 → 模型的一条阶段上报（主线 `ReportKernelBootStage` / `ReportKernelBootPlugins`）。
///
/// 为什么走 mpsc 而不是各占一个 `Msg` 臂：`Callback<T>` 与 `LocalSender` 都持 `Rc`（不可 `Send`），
/// 塞不进 `spawn_background` 闭包；而给每个事件单独发一条 `Msg` 会让 reactor 每事件重排一次
/// tick，与「单发在飞」的守卫冲突。所以引导线程只往通道里投，**由 100ms tick 在 UI 线程抽干**。
#[derive(Clone, Debug, PartialEq)]
enum BootEvent {
    /// 进入第 n 步（1 起）；未登记的步号一律忽略（KC:85-89「宁可不显示也不糊弄」）。
    Stage(i32),
    /// 第 1 步的插件安装子进度 (ready, total)；`total == 0` 视为无刻度（KC:100）。
    ///
    /// 生产者 = `report_plugin_ledger`（引导线程在第 1 段窗口里按 `PLUGIN_POLL_MS` 抽样内核的
    /// `plugin/installProgress`）。主线那一条来自壳自己的 `DshPluginBootstrap.EnsureAsync`
    /// （MW:2460-2470，扫 node_modules），分叉不装包 ⇒ 台账只能由内核报，**真内核没有这一发
    /// RPC 就一次报错收手**，第 1 段退回匀速爬满。已备案偏差见 `start_kernel` 的注释与
    /// `boot_plugin_ledger_producer_is_wired` 那条用例。
    Plugins(i32, i32),
}

/// 加载卡提示行的三种落点（KC:234-248 的优先级分支）。
#[derive(Clone, Debug, PartialEq)]
enum BootHint {
    /// 第 1 步且真的还有包没就绪：显示真实刻度，**不显示秒数**。
    Plugins(i32, i32),
    /// 否则报已用秒数；首秒（`seconds <= 0`）连文本一起收起，免得「0 秒」看着像坏了。
    Seconds(i32),
}

/// 加载卡一帧要画的全部数值（`BootState::paint`/`render` 的纯函数结果，视图与单测共用）。
#[derive(Clone, Debug, PartialEq)]
struct BootPaint {
    /// `KernelBootBar.Value`（0–100）。
    value: f64,
    /// `KernelBootStage` 的文案键：阶段名，失败后换成 `内核启动失败`。
    stage_key: &'static str,
    /// `KernelBootStep` 的三个参数：第几步 / 共几步 / 百分比。
    step: i32,
    percent: i64,
    /// `Some` = 画 `KernelBootHint`，`None` = 那一行收起（分叉=槽位缺席）。
    hint: Option<BootHint>,
    /// 失败态（reasonKey, 异常原文）：`Some` 时 Retry 钮上屏、Hint 换成这一句（KC:145-151）。
    failure: Option<(String, String)>,
}

/// 主线 `(int)Math.Round(_bootShown)`：.NET `Math.Round` 默认重载是 **MidpointRounding.ToEven**
/// （.5 落到偶数），而 `f64::round` 是「远离零」⇒ 半整数那帧会差 1%（规格 §7 存疑 6）。
/// 这里复刻 ToEven，只在非负区间用（进度值恒为 0..=100）。
fn round_half_even(value: f64) -> i64 {
    let floor = value.floor();
    let delta = value - floor;
    let lower = floor as i64;
    if delta > 0.5 {
        lower + 1
    } else if delta < 0.5 {
        lower
    } else if lower % 2 == 0 {
        lower
    } else {
        lower + 1
    }
}

/// 主线 `MainWindow.KernelBoot.cs` 那一组 `_bootStep/_bootTarget/_bootShown/_bootPlugins/
/// _kernelBootFailure/_kernelWaitHintShown` 字段的分叉对应：**纯状态机**，不碰 reactor、不读时钟、
/// 不发消息，所以每一步都能离线测（本轮不许开窗口，单测是唯一自证手段）。
/// 时钟（`_kernelBootWatch`）与 tick 代号留在 `Shell` 侧，秒数由 tick 写进 `seconds`。
#[derive(Clone, Debug, PartialEq)]
struct BootState {
    /// 卡片是否在树里（主线 `KernelBootPanel.Visibility == Visible`）。reactor 无 `Visibility`，
    /// 显隐只能靠 keyed 槽位在场与否。
    showing: bool,
    /// 当前阶段（1 起）；失败/就绪后停在原值，供步骤行显示「卡在第几步」。
    step: i32,
    target: f64,
    shown: f64,
    plugins: Option<(i32, i32)>,
    /// (reasonKey, 异常原文)：语言切换时按当前语言重刷整卡（KC:48-49）。
    failure: Option<(String, String)>,
    /// `CompleteKernelBootProgress` 已跑：条子钉在 100，等 350ms 驻留后撤卡。
    done: bool,
    /// 轮次代号：每次上屏 +1，用来丢弃「上一轮」迟到的收尾消息。
    seq: u64,
    /// 在飞的那发 `BootTick` 序号（只在 `arm_boot_tick` 递增）。
    tick: u64,
    /// 主线 `_kernelWaitHintShown`：每轮引导最多提示一次「内核还在加载中」，只在**上屏**时复位。
    hint_shown: bool,
    /// 主线 `(int)_kernelBootWatch.Elapsed.TotalSeconds`，由 tick 从 `Instant` 换算后写入。
    seconds: i32,
}

impl BootState {
    /// 未开卡的初版（等价主线字段声明期的默认值）。
    fn idle() -> Self {
        Self {
            showing: false,
            step: 1,
            target: 0.0,
            shown: 0.0,
            plugins: None,
            failure: None,
            done: false,
            seq: 0,
            tick: 0,
            hint_shown: false,
            seconds: 0,
        }
    }

    /// `ShowKernelBootPanel`（KC:54-77）：整卡重置并重新计时。重试也走这里，
    /// 所以条子直接归 0（KC:61），不经过回退动画。
    fn show(&mut self) {
        self.showing = true;
        self.hint_shown = false;
        self.failure = None;
        self.plugins = None;
        self.step = 1;
        self.target = kernel_boot::FIRST_TARGET;
        self.shown = 0.0;
        self.done = false;
        self.seconds = 0;
        self.seq += 1;
        self.tick = 0;
    }

    /// `HideKernelBootPanel`（KC:123-131）：撤卡即停臂（不再重新上发）。
    fn hide(&mut self) {
        self.showing = false;
        self.done = false;
    }

    /// `ReportKernelBootStage`（KC:83-95）：未登记的步号直接忽略。
    fn report_stage(&mut self, step: i32) {
        if Self::stage(step).is_none() {
            return;
        }
        self.step = step;
        self.recompute_target();
    }

    /// `ReportKernelBootPlugins`（KC:97-102）：`total == 0` 视为本轮无包要装。
    fn report_plugins(&mut self, ready: i32, total: i32) {
        self.plugins = if total > 0 {
            Some((ready, total))
        } else {
            None
        };
        self.recompute_target();
    }

    fn stage(step: i32) -> Option<(&'static str, i32, f64, f64)> {
        kernel_boot::STAGES
            .into_iter()
            .find(|(_, id, _, _)| *id == step)
    }

    /// `RecomputeBootTarget`（KC:104-112）：除第 1 步有真刻度插值外，每报一个阶段目标值直接取上沿。
    fn recompute_target(&mut self) {
        let clamped = self.step.clamp(1, kernel_boot::STEP_TOTAL);
        let Some((_, _, from, to)) = Self::stage(clamped) else {
            return;
        };
        let fraction = match self.plugins {
            Some((ready, total)) if self.step == 1 && total > 0 => {
                (ready as f64 / total as f64).clamp(0.0, 1.0)
            }
            _ => 1.0,
        };
        self.target = from + (to - from) * fraction;
    }

    /// `PaintKernelBootTick`（KC:218-249）：先把 `shown` 朝目标推一格，再出这一帧。
    fn paint(&mut self) -> BootPaint {
        let delta = self.target - self.shown;
        if delta.abs() < kernel_boot::SNAP {
            self.shown = self.target;
        } else {
            let rate = if delta > 0.0 {
                kernel_boot::RATE_FORWARD
            } else {
                kernel_boot::RATE_BACK
            };
            self.shown += delta.clamp(-rate, rate);
        }
        self.render()
    }

    /// 只读的一帧（主线把「插值」和「画」写在同一个方法里，这里拆开是因为分叉的视图不能改状态）。
    fn render(&self) -> BootPaint {
        // 失败态：Hint 那一行让给原因整句，秒数/插件刻度都不再出（KC:145-151、tick 已停）。
        let hint = match &self.failure {
            Some(_) => None,
            None => match self.plugins {
                Some((ready, total)) if self.step == 1 && ready < total => {
                    Some(BootHint::Plugins(ready, total))
                }
                _ => (self.seconds > 0).then_some(BootHint::Seconds(self.seconds)),
            },
        };
        BootPaint {
            value: self.shown,
            stage_key: self.stage_key(),
            step: self.step.clamp(1, kernel_boot::STEP_TOTAL),
            percent: round_half_even(self.shown),
            hint,
            failure: self.failure.clone(),
        }
    }

    /// 阶段标题的文案键；失败态换成 `内核启动失败`（KC:147）。
    fn stage_key(&self) -> &'static str {
        if self.failure.is_some() {
            return "内核启动失败";
        }
        let clamped = self.step.clamp(1, kernel_boot::STEP_TOTAL);
        Self::stage(clamped).map_or("正在启动内核…", |(key, _, _, _)| key)
    }

    /// `CompleteKernelBootProgress`（KC:114-120）：`shown` **也直接写成 100**——
    /// 最后 4% 不是补间出来的，这是主线现状（规格 §2.4 精度提示），照抄不自行插值。
    fn complete(&mut self) {
        self.target = 100.0;
        self.shown = 100.0;
        self.plugins = None;
        self.done = true;
    }

    /// `FailKernelBoot`（KC:138-156）：原卡就地转失败态，条子**停在失败瞬间的位置**、不归零。
    fn fail(&mut self, reason_key: &str, detail: &str) {
        self.failure = Some((reason_key.to_string(), detail.to_string()));
    }

    /// 该不该继续上臂：撤卡、失败都要停（KC:207-213 的 `sender.Stop()`）。
    /// 注意 `done` 不在这里——`CompleteKernelBootProgress` 之后条子仍要按 100 继续被画，
    /// 撤卡由 350ms 那一发 `Msg::BootDone` 负责（MW:2628-2629）。
    fn ticking(&self) -> bool {
        self.showing && self.failure.is_none()
    }

    /// `Msg::BootTick(seq)` 臂首行的守卫（纯函数，好让「撤卡后还在投递 = bug」这条可离线测）：
    /// 只认**当前在飞**的那一发序号。`show()` 把 `tick` 归零 ⇒ 重试后旧轮次那发必然不等；
    /// 撤卡/失败后 `ticking()` 为假 ⇒ 迟到那发被丢，也不再重新上臂。
    fn accepts_tick(&self, seq: u64) -> bool {
        seq == self.tick && self.ticking()
    }
}

/// 内核那一发插件台账 `{ready,total}` 的解析（纯函数，单测与 `tests/ipc.rs` 同口径）。
///
/// `total <= 0` ⇒ `None` = 主线 KC:100 的「本轮无包要装」：没有刻度就别拿 0/0 去插值。
/// 键缺/类型不对也回 `None`（真内核若哪天自己长出这一发但形状不同，宁可退回匀速爬满）。
fn plugin_ledger(value: &Value) -> Option<(i32, i32)> {
    let ready = value.get("ready")?.as_i64()?;
    let total = value.get("total")?.as_i64()?;
    (total > 0).then_some((ready as i32, total as i32))
}

/// `BootEvent::Plugins` 的**生产者**：主线 `ReportKernelBootPlugins(ready, total)`（KC:97-102）
/// 在分叉的等价物 —— 引导线程在第 1 段窗口里按 `PLUGIN_POLL_MS`(250ms) 抽内核的装载台账，
/// 抽到 `ready >= total`（或预算耗尽 / 问不出来）就收手。
///
/// 主线不需要问：它的 ready/total 是壳自己扫 `node_modules` 数出来的
/// （`DshPluginBootstrap.WatchInstall`/`ReportInstall`，DshPluginBootstrap.cs:168-198，同一个
/// 250ms 周期），因为**装包的活就是壳在引导线程里干的**。分叉不装包、内核是黑盒 ⇒ 台账只有
/// 内核知道，只能走 RPC：
/// · 假内核（`src/bin/fake_dsh.rs` 的 `plugin/installProgress`，`--plugins=N` 启用）给出真会
///   推进的刻度 ⇒ 第 1 段按 ready/total 插值爬，`正在安装插件 x/y` 那行上屏；
/// · 真内核没有这一发 ⇒ `call` 一次报错就 return ⇒ 与接上生产者之前**逐字一致**（第 1 段
///   匀速爬到 30、没有插件行），也就是主线「本轮无包要装」时同一形态。
fn report_plugin_ledger(tx: &mpsc::Sender<BootEvent>, kernel: &mut Kernel) {
    let started = Instant::now();
    loop {
        let Ok(value) = kernel.call(kernel_boot::PLUGIN_RPC, json!({})) else {
            return;
        };
        let Some((ready, total)) = plugin_ledger(&value) else {
            return;
        };
        // 通道那一端是 UI 线程的 tick 排水口（`drain_boot_events`）：投递失败 = 这一轮已经被
        // 重试/撤卡换掉，不必也不能在这里报错。
        if tx.send(BootEvent::Plugins(ready, total)).is_err() {
            return;
        }
        if ready >= total || started.elapsed() > Duration::from_millis(kernel_boot::PLUGIN_BUDGET_MS)
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(kernel_boot::PLUGIN_POLL_MS));
    }
}


/// 主干 `ChatBubble` 的最小对应：只带分叉现在渲染得到的字段。
#[derive(Clone, PartialEq)]
struct Bubble {
    role: &'static str,
    text: String,
    turn: i64,
    seq: i64,
    /// 操作行的四个来源字段：反馈/分支按内核 `message.id` 校验，`time` 出时钟，
    /// `model` 出「· 型号」，`tokens` 出「用量」。留空即主干「内核没报这个字段」⇒ 整段收起。
    id: String,
    time: i64,
    model: String,
    tokens: i64,
    /// 本轮 `turn/end.time − turn/start.time`，主干只挂在答案气泡上。
    duration_ms: i64,
    /// `tool/call` 的图标（`TOOL_TABLE` 查出来的字形），其余角色为 None。
    icon: Option<char>,
    /// 工具行的摘要段（主干把它和标题分成两段画，中间夹一个 0.5 透明度的「·」）。
    summary: String,
    /// 交付物行（`deliverables/presented`）的文件（原始下标, 路径, 说明），其余角色为空。
    /// 下标要按内核数组原样留：`present.open` 用 (sessionId, seq, index) 回查日志定位文件。
    files: Vec<(usize, String, String)>,
    /// 轮尾「本轮文件改动」chips（主干 `_producedByTurn` 在该答案气泡 seq 截断后的首现顺序）。
    produced: Vec<String>,
}

impl Bubble {
    fn new(role: &'static str, text: String, turn: i64, seq: i64) -> Self {
        Self {
            role,
            text,
            turn,
            seq,
            id: String::new(),
            time: 0,
            model: String::new(),
            tokens: 0,
            duration_ms: 0,
            icon: None,
            summary: String::new(),
            files: Vec::new(),
            produced: Vec::new(),
        }
    }
}

/// 气泡行的寻址键：hover 提亮、「已复制」态和思考折叠都按它记（分叉没有逐气泡的对象身份）。
/// 带 role 是因为主干允许思考段和正文共用同一个 (turn, seq)。
fn bubble_key(bubble: &Bubble) -> String {
    format!("{}-{}-{}", bubble.role, bubble.turn, bubble.seq)
}

/// 主干 `FeedbackItem`（`MainWindow.xaml.cs:2123`）：内核 `messageFeedback` 回的一条条目。
/// `rating` 只有 `positive`/`negative` 两个字面量；`version` 由内核每次 put 重新生成，
/// 壳只负责把观察到的值原样回传（乐观并发），本地不许自己造。
#[derive(Clone)]
struct FeedbackItem {
    rating: String,
    note: Option<String>,
    category: Option<String>,
    version: Option<String>,
}

/// 主干 `SetFeedbackStatus` 的三种落点：成功清成空、失败给文案，
/// 而 delete 那两条「本地已对齐」的提前 return 路径压根不碰 Status（旧文案留着）。
#[derive(Clone)]
enum FeedbackStatus {
    Keep,
    Clear,
    Text(String),
}

/// 一次 put/delete 后台状态机的最终落点，对应主干「写 `_feedback` + `RenderAllFeedbackRows()`
/// + `SetFeedbackStatus`」那一串副作用。分叉是声明式重渲染，所以整段折成一份补丁回 UI 线程。
/// 应用顺序固定：`aligned`（整表重建）→ `revoked`（摘掉这条）→ `adopted`（覆写这条）。
#[derive(Clone)]
struct FeedbackPatch {
    session: String,
    message: String,
    /// delete 前本地没有 version 时先做的 list 对齐（主干 `LoadFeedbackAsync` 同一步）。
    aligned: Option<Vec<Value>>,
    /// 内核权威条目：put 成功回的 item，或 version-conflict 采纳的 `error.current`。
    adopted: Option<Value>,
    /// 本地撤销这条：delete 成功 / 内核侧本就没有 / 采纳到 `current: null`。
    revoked: bool,
    status: FeedbackStatus,
}

/// `messageFeedback` 的业务失败：`error` 是内核原样回的 `{code, current?, maxBytes?, ...}`。
enum FeedbackFailure {
    Business(Value),
    /// 传输/网关层失败，字符串是 `Kernel::call` 拼的 `"<code>: <message>"`
    /// （主干那边是 `DshRpcException.Message`，同样只进 Status）。
    Transport(String),
}

impl FeedbackFailure {
    /// 主干 list 那条只 `catch` 到 `DshRpcException.Message`（`"<code>: <message>"`）并原样
    /// 写进状态，业务 `ok:false` 也是同一副面孔，所以这里不查表。
    fn code_text(&self) -> String {
        match self {
            Self::Transport(text) => text.clone(),
            Self::Business(error) => format!(
                "{}: {}",
                error["code"].as_str().unwrap_or_default(),
                error["message"].as_str().unwrap_or_default()
            ),
        }
    }
}

/// put/delete 的失败落进行内 Status：业务错误照主干 `FeedbackErrorText`（`xaml.cs:17001`）
/// 查表，传输层失败和主干一样直接上异常原文。
fn feedback_failure_message(catalog: &Catalog, failure: &FeedbackFailure) -> String {
    match failure {
        FeedbackFailure::Business(error) => feedback_error_text(catalog, error),
        FeedbackFailure::Transport(text) => text.clone(),
    }
}

/// 主干 `CallFeedbackAsync`（`MainWindow.xaml.cs:16787`）：**双层 ok**。外层信封
/// `{result:{ok,value}}` 由 `Kernel::call` 剥掉，剩下的 `value` 本身又是内核方法的返回体
/// `{ok:true,value:...}` / `{ok:false,error:...}`，所以 `call` 返回 Ok 并不等于业务成功。
/// 外层失败（gateway/鉴权）由 `call` 直接拼成 Err，这里归到 `Transport`。
fn feedback_call(shared: &Shared, method: &str, args: Value) -> Result<Value, FeedbackFailure> {
    let outcome = shared
        .lock()
        .map_err(|_| "内核状态不可用".to_string())
        .and_then(|mut kernel| kernel.call(method, args));
    let body = match outcome {
        Ok(body) => body,
        Err(error) => return Err(FeedbackFailure::Transport(error)),
    };
    if body["ok"].as_bool().unwrap_or(false) {
        return Ok(body.get("value").cloned().unwrap_or(Value::Null));
    }
    Err(FeedbackFailure::Business(
        body.get("error").cloned().unwrap_or(Value::Null),
    ))
}

/// 主干 `StoreFeedbackItem`（16811）：只认字符串字段，`messageId` 缺失整条丢弃。
fn parse_feedback(item: &Value) -> Option<(String, FeedbackItem)> {
    let id = item["messageId"].as_str()?;
    Some((
        id.to_string(),
        FeedbackItem {
            rating: item["rating"].as_str().unwrap_or_default().to_string(),
            note: item["note"].as_str().map(str::to_string),
            category: item["category"].as_str().map(str::to_string),
            version: item["version"].as_str().map(str::to_string),
        },
    ))
}

fn store_feedback(table: &mut Vec<(String, FeedbackItem)>, item: &Value) {
    if let Some((id, entry)) = parse_feedback(item) {
        set_pair(table, id, entry);
    }
}

fn feedback_of(table: &[(String, FeedbackItem)], id: &str) -> Option<FeedbackItem> {
    table
        .iter()
        .find(|(known, _)| known == id)
        .map(|(_, entry)| entry.clone())
}

/// 主干 `FeedbackErrorText`（16996）：文案逐字照抄，未知 code 走「失败（{0}）」，
/// 连 code 都没有就是「操作失败」（主干那边是 `GetProperty("error")` 抛异常，落到 `ex.Message`，
/// 分叉统一成同一句人话，不搬 .NET 异常文本）。
fn feedback_error_text(catalog: &Catalog, error: &Value) -> String {
    let code = error["code"].as_str();
    match code {
        Some("target-not-found") => catalog.l("该消息不在本会话日志里（子代理或已裁剪），无法反馈"),
        Some("session-not-found") => catalog.l("会话不存在或已归档"),
        Some("note-blank") => catalog.l("说明不能是空白"),
        Some("note-too-large") => match error["maxBytes"].as_i64() {
            Some(max) => catalog.lf("说明超长（上限 {0} 字节）", &[max.to_string()]),
            None => catalog.l("说明超长"),
        },
        Some("version-conflict") => catalog.l("反馈已被其他地方改动，请重试"),
        Some(other) => catalog.lf("失败（{0}）", &[other.to_string()]),
        None => catalog.l("操作失败"),
    }
}

/// 说明的字节上限：内核配置 `dsh-web-app/cordis.patch.yml:56` 的 `maxNoteBytes`，
/// 假内核 `rust/src/bin/fake_dsh.rs` 的 `MAX_NOTE_BYTES` 同值，超了回
/// `{code:"note-too-large",maxBytes:8192,actualBytes}`。行内编辑器拿它算字数提示，
/// **按 UTF-8 字节**（`str::len()`）而不是字符数 —— 与内核 `Buffer.byteLength` 同口径。
const MAX_NOTE_BYTES: i64 = 8192;

/// 主干 `FormatMessageClock`（`MainWindow.MessageActions.cs:769`）：同日 `HH:mm`、
/// 同年 `M月D日 HH:mm`、跨年再加年份。内核 `time` 是 epoch 毫秒。
fn message_clock(millis: i64) -> String {
    let value = DateTime::from_unix_millis(millis).to_local();
    let today = DateTime::now().to_local();
    let clock = format!("{:02}:{:02}", value.hour(), value.minute());
    if value.year() == today.year() && value.month() == today.month() && value.day() == today.day()
    {
        clock
    } else if value.year() == today.year() {
        format!("{}月{}日 {clock}", value.month(), value.day())
    } else {
        format!(
            "{}年{}月{}日 {clock}",
            value.year(),
            value.month(),
            value.day()
        )
    }
}

/// 主干 `{0:0.#}`：最多一位小数、不留尾随 0。
fn one_decimal(value: f64) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded:.1}")
    }
}

/// 主干 `FormatTokensCompact` 的中文档：亿 / 万 两级，其余出原值。
fn tokens_text(tokens: i64) -> String {
    if tokens >= 100_000_000 {
        format!("{} 亿", one_decimal(tokens as f64 / 100_000_000.0))
    } else if tokens >= 10_000 {
        format!("{} 万", one_decimal(tokens as f64 / 10_000.0))
    } else {
        format!("{tokens}")
    }
}

/// 主干 `FormatDuration`：`{0} 小时 {1} 分` / `{0} 分 {1} 秒` / `{0} 秒`。
fn duration_text(ms: i64) -> String {
    let secs = ms / 1000;
    if secs >= 3600 {
        format!("{} 小时 {} 分", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{} 分 {} 秒", secs / 60, secs % 60)
    } else {
        format!("{secs} 秒")
    }
}

/// 内核 usage 口径（`MainWindow.xaml.cs:13992`）：优先 totalTokens，否则四项相加。
fn usage_tokens(usage: &Value) -> i64 {
    let total = usage["totalTokens"].as_i64().unwrap_or(0);
    if total > 0 {
        return total;
    }
    [
        "inputTokens",
        "outputTokens",
        "cacheReadTokens",
        "cacheWriteTokens",
    ]
    .iter()
    .map(|key| usage[key].as_i64().unwrap_or(0))
    .sum()
}

/// 主干 `PathValue`：非空白路径按工具收到的原样拼写保留。
fn path_value<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

/// 主干 `ValidEditArgs`：old_string 非空、new_string 是字符串、两者不同，replace_all 缺省或布尔。
fn valid_edit_args(args: &Value) -> bool {
    let Some(old) = args.get("old_string").and_then(Value::as_str) else {
        return false;
    };
    let Some(new) = args.get("new_string").and_then(Value::as_str) else {
        return false;
    };
    old != new && !old.is_empty() && args.get("replace_all").is_none_or(|flag| flag.is_boolean())
}

/// 主干 `EditorMutationPath`：只有完整突变命令（create/str_replace/insert）才产出路径。
fn editor_mutation_path(args: &Value) -> Option<&str> {
    let path = path_value(args, "path")?;
    let complete = match args
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "create" => args.get("file_text").is_some_and(Value::is_string),
        "str_replace" => {
            path_value(args, "old_str").is_some_and(|old| !old.is_empty())
                && args.get("new_str").is_none_or(Value::is_string)
        }
        "insert" => {
            args.get("insert_line")
                .and_then(Value::as_i64)
                .is_some_and(|line| line >= 0)
                && args.get("new_str").is_some_and(Value::is_string)
        }
        _ => false,
    };
    complete.then_some(path)
}

/// 主干 `MutationPath`：一等突变工具（write / edit / str_replace_editor）才登记目标路径，
/// 其余工具或参数不完整都是 None（`tool/result` 成功时才计入本轮产出）。
fn mutation_path<'a>(name: &str, args: &'a Value) -> Option<&'a str> {
    if !args.is_object() {
        return None;
    }
    match name {
        "write" => args
            .get("content")
            .and_then(Value::as_str)
            .and_then(|_| path_value(args, "file_path")),
        "edit" => valid_edit_args(args)
            .then(|| path_value(args, "file_path"))
            .flatten(),
        "str_replace_editor" => editor_mutation_path(args),
        _ => None,
    }
}

/// 标题结尾的 `(N)` / `（N）`：返回括号前的前缀与序号，口径同主干的两条正则
/// （非贪婪前缀 + 数字 + 右括号到行尾，等价于从右往左找最后一对括号）。
fn bracketed_number(title: &str, open: char, close: char) -> Option<(String, i64)> {
    let body = title.trim_end().strip_suffix(close)?;
    let open_at = body.rfind(open)?;
    let digits = &body[open_at + open.len_utf8()..];
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((body[..open_at].to_string(), digits.parse().ok()?))
}

/// 主干 `SessionDisplayTitle`（`xaml.cs:2581`）：空白会话回字面「新会话」（主干这条不走
/// 本地化），否则标题 → cwd 末段 → sessionId。分支改名要用它，不能蹭分叉的 `row_title`。
fn display_title(row: &SessionInfo) -> String {
    if row.blank {
        return "新会话".to_string();
    }
    if !row.title.is_empty() {
        return row.title.clone();
    }
    let flattened = row.cwd.replace('\\', "/");
    flattened
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| row.id.clone())
}

/// 主干 `RenameForkedChildAsync` 的标题递增：全角 `（N）` 优先，其次半角 `(N)`，
/// 两者都没命中则补 `" (1)"`。
fn increased_title(title: &str) -> String {
    for (open, close) in [('（', '）'), ('(', ')')] {
        if let Some((prefix, number)) = bracketed_number(title, open, close) {
            return format!("{prefix}{open}{}{close}", number + 1);
        }
    }
    format!("{title} (1)")
}

/// 主干 `ToolRowModel`（`MainWindow.MessageActions.cs:660`）：查表出字形 + 标题，
/// 摘要按变体取键；未知名标题回落「工具调用」并把工具名并进摘要前缀。
fn tool_row(name: &str, args: &Value) -> (char, &'static str, String) {
    let line = |text: &str| text.lines().next().unwrap_or_default().to_string();
    let pick = |keys: &[&str]| -> String {
        keys.iter()
            .filter_map(|key| args[key].as_str())
            .next()
            .map(line)
            .unwrap_or_default()
    };
    let entry = TOOL_TABLE
        .iter()
        .find(|(tool, _, _, _)| *tool == name)
        .map(|(_, code, title, variant)| (*code, *title, *variant));
    let (code, title, variant) = entry.unwrap_or(('\u{e8a5}', "", "others"));
    let summary = match variant {
        "bash" => pick(&["description", "command"]),
        "search" => args["queries"]
            .as_array()
            .and_then(|list| list.first())
            .and_then(|item| item.as_str())
            .map(line)
            .unwrap_or_else(|| pick(&["path", "file_path", "url"])),
        "code" | "write" | "edit" => pick(&["description", "path", "file_path"]),
        _ => pick(&["path", "file_path", "url"]),
    };
    if title.is_empty() {
        let base = if summary.is_empty() {
            String::new()
        } else {
            format!("{name} · {summary}")
        };
        return (code, "工具调用", base);
    }
    (code, title, summary)
}

/// 主干 `LiveAttemptState`：正在按 token 追加的那一次尝试。
#[derive(Clone, PartialEq)]
struct LiveAttempt {
    session: String,
    attempt: String,
    turn: i64,
    text: String,
}

type Shared = Arc<Mutex<Kernel>>;

fn th(v: [f64; 4]) -> Thickness {
    Thickness::new(v[0], v[1], v[2], v[3])
}

/// 主干给左栏固定项显式 AutomationId（NewSessionItem / SettingsItem），会话行是 `Session_{id}`。
/// 分组行/展开行主干没有可抄的 id，退化成 `Nav_{key}` 只为自测可寻址。
fn nav_automation_id(key: &str) -> String {
    match key {
        "new-session" => "NewSessionItem".to_string(),
        "settings" => "SettingsItem".to_string(),
        _ if SETTINGS_SECTIONS.iter().any(|(id, _, _)| *id == key) => format!("Nav_{key}"),
        _ if key.starts_with("node-") || key.starts_with("cap-") => format!("Nav_{key}"),
        _ => format!("Session_{key}"),
    }
}

/// 主干 `requestId` 用 `c2-{Guid:N}`；分叉没有 uuid 依赖，用纳秒时间戳顶上（内核只要求非空字符串）。
fn request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|span| span.as_nanos())
        .unwrap_or(0);
    format!("rs-{nanos:x}")
}

/// `session/follow` 的入参，形状照主干 `DshRpcClient.OpenFollowStreamAsync`。
fn follow_args(session: &str) -> Value {
    json!({ "request": {
        "address": { "kind": "session", "sessionId": session },
        "assistantStream": true,
    }})
}

/// journal 的 `content` 是块数组，分叉只把 text 块拼起来。
fn content_text(blocks: Option<&Vec<Value>>) -> String {
    blocks
        .map(|items| {
            items
                .iter()
                .filter(|block| block["type"].as_str() == Some("text"))
                .map(|block| block["text"].as_str().unwrap_or_default())
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

fn toggle(list: &mut Vec<String>, key: &str) {
    match list.iter().position(|item| item == key) {
        Some(index) => {
            list.remove(index);
        }
        None => list.push(key.to_string()),
    }
}

/// 主干 `WorkspaceBasename()`：先去尾分隔符，再取最后一个分隔符之后。
fn workspace_basename(path: &str) -> &str {
    let trimmed = path.trim_end_matches(['/', '\\']);
    match trimmed.rfind(['/', '\\']) {
        Some(index) => &trimmed[index + 1..],
        None => trimmed,
    }
}

fn blank() -> View {
    StackPanel::new().children(())
}

/// 设置页那几张键值表（主干存在内核快照里）：写覆盖，缺键则追加。
fn set_pair<T>(table: &mut Vec<(String, T)>, key: String, value: T) {
    match table.iter_mut().find(|(name, _)| *name == key) {
        Some(entry) => entry.1 = value,
        None => table.push((key, value)),
    }
}

fn pair_value<T: Clone>(table: &[(String, T)], key: &str, fallback: T) -> T {
    table
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.clone())
        .unwrap_or(fallback)
}

/// 二级页标题：主干把分区名换成 `OpenSettingsSubPage` 传入的 title，与导航卡同名。
fn sub_title(id: &str) -> String {
    SETTINGS_PLUGIN_CARDS
        .iter()
        .find(|(card, _, _, _)| *card == id)
        .map(|(_, _, title, _)| (*title).to_string())
        .unwrap_or_else(|| "设置".to_string())
}

/// reactor 的 FontIcon 只有 glyph，字号/颜色一律继承父级（已知近似）。
fn mark(code: char) -> FontIcon {
    FontIcon::new().glyph(code.to_string())
}

/// 主干给每个 FontIcon 单独写 FontSize（10/12/14/16）；reactor 的 FontIcon 没有字号 setter，
/// 套一层 Viewbox 把默认 20px 的字形等比缩到目标方块，量出来的墨迹高度与主干一致。
/// Viewbox 一旦定了 Height，默认 Stretch 的对齐就退化成顶对齐 ⇒ 这里显式居中
/// （主干这些图标所在行都是 `VerticalAlignment=Center` 的语境）。
fn mark_size(code: char, side: f64) -> View {
    Viewbox::new()
        .width(side)
        .height(side)
        .vertical_alignment(VerticalAlignment::Center)
        .stretch(Stretch::Uniform)
        .slot(ViewboxSlot::Child, mark(code))
}

/// 居中挂进 Grid 单元格的字形方块（主干那些 `VerticalAlignment=Center` 的 FontIcon）。
/// `.content()` 之后就是 `View`，再挂 `grid_column` 之类会编译不过 ⇒ 顺序固定在这里。
fn mark_center(code: char, side: f64, column: i32) -> View {
    Border::new()
        .grid_column(column)
        .vertical_alignment(VerticalAlignment::Center)
        .content(mark_size(code, side))
}

/// 同上，但挂进 StackPanel（无列号）。Viewbox 定高后默认顶对齐，必须外面套一层才能居中。
fn mark_vcenter(code: char, side: f64) -> View {
    Border::new()
        .vertical_alignment(VerticalAlignment::Center)
        .content(mark_size(code, side))
}

/// 定了宽高就**两向都居中**的字形方块：`feedback_toggle` 那颗直接把它塞进负 margin 的
/// Border 里，没有 `ContentPresenter` 的 `HorizontalContentAlignment=Center` 兜着
/// （`pill_button` 那几颗有），Viewbox 定了 14×14 之后默认按左上摆 ⇒ 自己钉居中。
fn mark_cell(code: char, side: f64) -> View {
    Viewbox::new()
        .width(side)
        .height(side)
        .horizontal_alignment(HorizontalAlignment::Center)
        .vertical_alignment(VerticalAlignment::Center)
        .stretch(Stretch::Uniform)
        .slot(ViewboxSlot::Child, mark(code))
}

/// 主干小钮的共同外观：无描边 + 四态底色全透明（圆底由 `pill_button` 自己画）。
///
/// 「无描边」只能靠把 `ButtonBorderBrush` 四态覆成透明来实现：**绝不能**覆
/// `ButtonBorderThemeThickness` —— 那个主题资源在 WinUI 里是 `double`，而
/// `ResourceOverrides::set` 只吃得下 `Color|Thickness|CornerRadius`，塞一个盒装 `Thickness`
/// 进 Button 模板的 `{ThemeResource}` 槽位会让 Microsoft.UI.Xaml.dll 直接 fail-fast
/// （进程退出码 0xC000027B，stdout/stderr 全空，启动 1~3 秒内连窗口都没了就死）。
/// `ButtonBorderThickness` 则压根不是资源键（覆上去静默无效果）。
///
/// 圆角**不再**从这里走，两条实测结论：
/// 1. reactor 的 `Button` builder 没有 `corner_radius` 直设属性 —— 0.100.0 的
///    `generated.rs:490-550` 只给了 is_enabled / 两个 content_alignment /
///    resource_overrides / style / key_accelerators / on_click，主干那条
///    「在 Button 实例上直接写 CornerRadius=24」的通路在 reactor 里根本不存在
///    （WinUI 侧 `Microsoft.UI.Xaml.Controls.Button` 也没有 CornerRadius DP，
///    主干能写是因为 XAML 把它落到了模板的 ContentPresenter 上）。
/// 2. `ControlCornerRadius` 的资源覆盖对 Button 模板**完全不起作用**：
///    `set_resource_overrides`（native/winui/mod.rs:1057）只是往
///    `IFrameworkElement.Resources` 里塞一个 `IReference<CornerRadius>`，实测 28×28 的
///    ComposerAddButton 覆成 14 仍渲染成默认 4px 圆角方块（tmp/mycheck-add.png）。
/// ⇒ 于是四态底色全钉透明（免得模板那层 4px 方块高亮从圆底外面支出来），形状与颜色
/// 一起交给 `pill_button` 里那层 Border。
fn button_ghost(foreground: Option<Color>) -> ResourceOverrides {
    let overrides = ResourceOverrides::new()
        .set("ButtonBackground", Color::transparent())
        .set("ButtonBackgroundPointerOver", Color::transparent())
        .set("ButtonBackgroundPressed", Color::transparent())
        .set("ButtonBackgroundDisabled", Color::transparent())
        .set("ButtonBorderBrush", Color::transparent())
        .set("ButtonBorderBrushPointerOver", Color::transparent())
        .set("ButtonBorderBrushPressed", Color::transparent())
        .set("ButtonBorderBrushDisabled", Color::transparent());
    // 强调色底上的字形必须反白钉死四态（主干 AccentButtonForeground 那一套）；
    // 其余钮交给主题默认前景，别把禁用态的灰色也一起钉掉。
    match foreground {
        Some(fg) => overrides
            .set("ButtonForeground", fg)
            .set("ButtonForegroundPointerOver", fg)
            .set("ButtonForegroundPressed", fg)
            .set("ButtonForegroundDisabled", fg),
        None => overrides,
    }
}

/// 圆底/方底的一档外观：底色画刷 + 1px 描边画刷 + 整颗钮的不透明度。
/// 描边取 `Brush::Solid(a=0)`（`NO_STROKE`）= 主干写了 `BorderThickness="0"` 的那几族。
#[derive(Clone, Copy, Debug, PartialEq)]
struct PillFace {
    brush: Brush,
    stroke: Brush,
    opacity: f64,
}

/// 「无描边」的常量写法（`Palette` 的 `transparent` 字段同一个值，这里给 const fn 用）。
const NO_STROKE: Brush = Brush::Solid(Color {
    a: 0,
    r: 0,
    g: 0,
    b: 0,
});

impl PillFace {
    const fn solid(brush: Brush) -> Self {
        Self {
            brush,
            stroke: NO_STROKE,
            opacity: 1.0,
        }
    }

    /// 带 1px 描边的一档：主干裸 `Button` / `ToggleButton`（默认 `DefaultButtonStyle`）
    /// 的描边走 `ControlElevationBorderBrush`（静止/悬停）与 `ControlStrokeColorDefault`
    /// （按下/禁用），实测值见 `theme.rs` 文件头，reactor 没有渐变画刷 ⇒ 静止/悬停取
    /// 渐变主色 `control_stroke`。
    const fn outlined(brush: Brush, stroke: Brush) -> Self {
        Self {
            brush,
            stroke,
            opacity: 1.0,
        }
    }
}

/// 一颗自绘底噪图标钮的四态输入，对应主干 Button 实例上的 `Background` + 模板四态。
#[derive(Clone, Copy)]
struct Pill {
    /// 钮的边长（ComposerAddButtonSize=28 / ComposerSendButtonSize=34 / IconButtonStyle=32）。
    side: f64,
    /// 主干写在该钮上的 CornerRadius 令牌值（CornerPill=24 / CornerSmall=4），按短边一半夹。
    /// ⇒ **两档形状都由这一个数表达**：`radius::PILL(24)` 在 28/34 的钮上夹成正圆，
    /// `radius::SMALL(4)` 原样传下去就是主干那种小圆角方块。
    radius: f64,
    rest: PillFace,
    hover: PillFace,
    pressed: PillFace,
    /// 禁用档；`None` = 与静置同色（主干 IconButtonStyle 没有独立的禁用观感）。
    disabled: Option<PillFace>,
    /// 描边粗细（DIP）。主干默认模板是 1px（`ButtonBorderThemeThickness`=1）；
    /// 写了 `BorderThickness="0"` 的那几族（composer 的 +/发送、返回、工作区两颗筛选钮）给 0。
    border: f64,
    /// 钉死字形前景（强调色底上的反白字形）；`None` = 跟随主题默认。
    foreground: Option<Color>,
    /// 主干 `SendButton.IsEnabled = !readOnly` 那类可用性。
    enabled: bool,
}

impl Pill {
    /// 真正画出去的圆角：主干在 Button 实例上直接写 `CornerRadius=CornerPill(24)`，
    /// WinUI 自己夹到短边一半 ⇒ 28/34 的钮上就是正圆。分叉的圆是自己画的，
    /// 得自己夹（超过半高会把整块底吃成一个葫芦）。
    const fn radius_clamped(&self) -> f64 {
        let half = self.side / 2.0;
        if self.radius > half { half } else { self.radius }
    }

    /// 主干**显式**写了 `Background="Transparent" BorderThickness="0"` 的那些图标钮
    /// （`ShellBackButton`、`SearchIconButton`、`SubagentParentButton`、工作区段头那两枚
    /// `WorkspaceViewOptionsButton`/`AddWorkspaceButton`、会话行 `moreBtn`）：
    /// 静置 `SubtleFillColorTransparent`（全透明），悬停/按下吃 **Control 链**
    /// （`ControlFillColorSecondary` 浅 `#80F9F9F9` / `ControlFillColorTertiary` 浅 `#4DF9F9F9`）。
    ///
    /// 这一档的口径是**主干实渲的像素**，不是主干的设计意图：
    /// 主干写在这颗钮上的令牌（`Tokens.xaml` 的 `CardHoverBrush` / `IconButtonStyle` 那一族）
    /// 属于 Subtle 家，但 WinUI 默认 Button 模板的 VisualState Setter 优先级**高于实例上写的
    /// `Background`**（`generic.xaml` 的 `DefaultButtonStyle` → `CommonStates.PointerOver/Pressed`
    /// 把 `ContentPresenter.Background` 抢成 `ButtonBackgroundPointerOver/Pressed` =
    /// `ControlFillColorSecondary/Tertiary`）⇒ 主干屏幕上真正的两档就是 Control 链。
    /// 上一轮按「设计意图」改成 `subtle_hover/subtle_pressed`，浅色下悬停变成黑叠加
    /// （实测 `+` hover `#ECECEC` 比静置更深），与主干实渲方向相反，本轮按像素改回来。
    /// 方向记牢：**WinUI 两条链里 pressed 都比 hover 浅一档**（浅 `#80→#4D`、深 `#15→#08`），
    /// 别按「按下要比悬停更深」去改。
    const fn ghost(p: Palette) -> Self {
        Self {
            side: size::ICON_BUTTON,
            radius: radius::SMALL,
            rest: PillFace::solid(p.subtle_rest),
            hover: PillFace::solid(p.control_hover),
            pressed: PillFace::solid(p.control_pressed),
            disabled: Some(PillFace::solid(p.subtle_disabled)),
            border: 0.0,
            foreground: None,
            enabled: true,
        }
    }

    /// 主干 `MakeCopyButton` / `MakeWithdrawEditButton` / 轮尾分支钮（`MainWindow.MessageActions.cs`）
    /// 那一族：裸 `Button` + `Width=Height=SmallButtonSize(28)` + `MinWidth=0` + `Padding=0` +
    /// `CornerRadius=RadSmall(4)` ⇒ **小圆角方，不是正圆**（4 < 短边一半 14）。
    /// 它们**没有**写 `Background`/`BorderThickness`，所以默认模板的四档全套都在：
    /// 静置 `ControlFillColorDefault`（浅 #B3FFFFFF）+ 1px `ControlElevationBorderBrush`、
    /// 悬停 Secondary、按下 Tertiary（描边换成 ControlStrokeColorDefault）、禁用
    /// ControlFillColorDisabled（模板那层被 `button_ghost` 钉透明了，禁用档得自己画）。
    /// 字形禁用灰由 `ButtonForegroundDisabled` 的默认资源给（`button_ghost(None)` 不覆前景）。
    const fn action(p: Palette, enabled: bool) -> Self {
        Self {
            side: size::SMALL_BUTTON,
            radius: radius::SMALL,
            rest: PillFace::outlined(p.control_fill, p.control_stroke),
            hover: PillFace::outlined(p.control_hover, p.control_stroke),
            pressed: PillFace::outlined(p.control_pressed, p.control_stroke_pressed),
            disabled: Some(PillFace::outlined(
                p.control_disabled,
                p.control_stroke_pressed,
            )),
            border: size::STROKE,
            foreground: None,
            enabled,
        }
    }

    /// 主干 `ComposerAddButton`（`MainWindow.xaml:1059-1068` 那颗 28 正圆 `+`）：
    /// `Width/Height=ComposerAddButtonSize(28)` + `Padding=0` + `CornerRadius=CornerPill(24)`
    /// + `Background="{ThemeResource CardHoverBrush}"` + `BorderThickness="0"`。
    /// 静置档 = `CardHoverBrush` 的本体 `SubtleFillColorSecondary`（浅 `#09000000`、深
    /// `#0FFFFFFF`，`Tokens.xaml:164/249`）⇒ `+` 底下那层浅灰圆**rest 态就该看得见**
    /// （主干实渲 `#F5F4F5` 一档，本轮实测分叉 rest `#F4F4F4`，一致）。
    /// 悬停/按下与 `Pill::ghost` 同一条链：**Control 链**，因为默认模板的 VisualState Setter
    /// 优先级高于实例上的 `Background`，主干屏幕上这两档本来就是
    /// `ControlFillColorSecondary`/`ControlFillColorTertiary`（浅 `#80F9F9F9` / `#4DF9F9F9`）
    /// ⇒ 浅色下这两档都是**白叠加**，一路比静置更浅：实测顺序 静置 `#F4` < 悬停 `#FB` <
    /// 按下 `#FD`（数值越来越大 = 越来越白），与主干实渲同向。
    /// 上一轮按「Fluent state-layer 语义」把状态层**叠**在静置圆底上
    /// （`theme::subtle_disc_hover/pressed`，浅 `#11`/`#0E`）⇒ 悬停反而最深，本轮按像素改回来；
    /// 那两个叠档助手仍留在 `theme.rs`（有逐档断言守着的备用配方），只是这一族不再取用。
    /// 同族的 `ContextMeterRing`（`MainWindow.xaml:1219-1228`，也是 `CardHoverBrush` + 28 圆）
    /// 分叉暂未移植，移植时直接复用这一档。
    const fn card(p: Palette) -> Self {
        Self {
            side: size::COMPOSER_ADD_BUTTON,
            radius: radius::PILL,
            rest: PillFace::solid(p.subtle_hover),
            hover: PillFace::solid(p.control_hover),
            pressed: PillFace::solid(p.control_pressed),
            disabled: None,
            border: 0.0,
            foreground: None,
            enabled: true,
        }
    }

    /// 主干 `SendButton`（`AccentButtonStyle`）四态：同一支强调色画刷，悬停降到 0.9、
    /// 按下降到 0.8（generic.xaml:7631-7633 Light 词典，Dark 在 2080-2082 数值一致），
    /// 禁用换 `AccentFillColorDisabled`。
    /// 强调色本体走主题画刷 `AccentFillColorDefaultBrush`，跟着用户个性化色变，不写死。
    /// 描边给 0：主干那一圈是 `AccentControlElevationBorderBrush`（强调色加深渐变），
    /// reactor 拿不到运行时的强调色分量，画不出同色描边 ⇒ 宁缺不凑。
    const fn accent(p: Palette, side: f64) -> Self {
        Self {
            side,
            radius: radius::PILL,
            rest: PillFace::solid(p.accent),
            hover: PillFace {
                brush: p.accent,
                stroke: NO_STROKE,
                opacity: 0.9,
            },
            pressed: PillFace {
                brush: p.accent,
                stroke: NO_STROKE,
                opacity: 0.8,
            },
            disabled: Some(PillFace::solid(p.accent_disabled)),
            border: 0.0,
            foreground: Some(p.on_accent_color),
            enabled: true,
        }
    }
}

/// 主干那些「固定边长 + 圆角 + 无描边」图标钮的统一外壳：**所有**图标钮都过这里，
/// 圆角一旦无效就是全分叉的 pill 钮一起方，所以只留这一条通路。
///
/// 圆底画在 Button 的 `content` 里，而不是把 Button 套在 Border 里：
/// 外层 Border 画圆时，模板那层焦点框与内容都会从圆外支出去，得再把钮的四态全钉透明、
/// 指针命中区也得跟着挪（发送钮上一版就是这么凑的，代价是丢掉悬停/按下态）；
/// 画在 content 里则 Button 照常负责键盘、焦点与 UIA Invoke，Border 只当画刷 + 指针层。
/// reactor 里**只有 Border 暴露指针事件**（`EventId` 只有 `BorderPointer*` 一族，
/// generated.rs:14078-14084），所以状态层必须是 Border；字形挂在这层 Border 内部，
/// 指针事件由它冒泡上来（`nav_row` 的悬停高亮是同一条已验证通路）。
/// 自绘层要抵掉模板内缩，见下面的 `TEMPLATE_INSET`：主干那边写的是 `Padding="0"`。
/// 用户行 / 卡片行的悬停提亮走 `Shell::hover` 这一整窗单槽（主干把 PointerEntered 挂在
/// 卡上），行内那颗钮的悬停档走 `Shell::pill_hover`：两层都听 `PointerEntered`、内层先触发，
/// 共用一个槽必然被外层那一枪盖掉。`hover` 传进来的必须是**同槽**的那份值。
#[derive(Clone, Copy, PartialEq, Eq)]
enum HoverSlot {
    /// 写 `Shell::hover`：输入区/导航区这类外层没有「行级提亮」钩子的钮（既有五处的原行为）。
    Row,
    /// 写 `Shell::pill_hover`：气泡卡内的钮（反馈赞/踩）。
    Pill,
}

/// 负 margin 抵掉默认 Button 模板 `ContentPresenter` 的**两**层内缩，一次抵平：
/// · `Padding="{StaticResource ButtonPadding}"` = `11,5,11,6`（`generic.xaml:27353` + `8376`）
///   —— StaticResource 在样式解析期就定死，`resource_overrides` 压不住（QA D16）；
/// · `BorderThickness="{ThemeResource ButtonBorderThemeThickness}"` = 每边 1 DIP
///   （`generic.xaml:27352` + `125`）—— 这个键**绝不能**覆：它是主题资源，塞盒装 `Thickness`
///   进 Button 模板槽位会让 Microsoft.UI.Xaml.dll 直接 fail-fast（0xC000027B，见 `button_ghost`）。
/// 只抵 Padding 的话，脸比控件自己的 UIA 矩形每边小 1 DIP = 200% 下 2 px
/// （上一轮实测：28 DIP 的 `+` UIA 56 px 而脸只有 52 px、32 DIP 的筛选钮 64 px 只有 60 px，
/// 圆和方都小一档；本轮 `TEMPLATE_INSET` 把描边一起抵掉，实测见 tmp/ui10-band*.txt）。
/// 不抵 back 更狠：28×28 的钮只剩 6×17 的内容区，圆底会缩成一条横线
/// （用户实跑看到的 composer「-」就是这么来的）。主干那边写的是 `Padding="0"`。
const TEMPLATE_INSET: [f64; 4] = [-12.0, -6.0, -12.0, -7.0];

/// 设置·个性化「消息气泡」卡的两份事实的 state 键。材质档走 `picks`（下拉框下标）、
/// 不透明度走 `numbers`（滑杆值），所以两条通道都天然是活的：`Msg::Pick` / `Msg::Number`
/// 改的就是它们，气泡底每轮在 `view` 里按这两个值现算。
/// 键名沿用相邻「窗口材质」卡的 `shell_*` 前缀（主干这两项也在壳本地 shell.json，分叉不落盘）。
const BUBBLE_MATERIAL_KEY: &str = "shell_bubbleMaterial";
const BUBBLE_OPACITY_KEY: &str = "shell_bubbleOpacity";

/// 主干 `BubbleMaterialChoices`（`MainWindow.Personalization.cs:115-120`）的三档标签，
/// 序 = `BubbleMaterial::from_index` 的下标；第三档主干写的是全称「跟随窗口材质」。
const BUBBLE_MATERIALS: &[&str] = &["半透明", "亚克力", "跟随窗口材质"];

/// 主干 `Personalization.cs:159-166` 的滑杆量纲：域 0.2–1.0 按 100 满刻度铺成 20–100、
/// `StepFrequency=5`、`Width=220`，读数 `{0}%`（`MinWidth=48` + 横向 `Space12` 由
/// `settings_slider` 统一给）。
const BUBBLE_OPACITY_MIN: f64 = theme::BUBBLE_OPACITY_MIN * 100.0;
const BUBBLE_OPACITY_MAX: f64 = theme::BUBBLE_OPACITY_MAX * 100.0;
const BUBBLE_OPACITY_STEP: f64 = 5.0;
const BUBBLE_OPACITY_DEFAULT: f64 = theme::BUBBLE_OPACITY_DEFAULT * 100.0;
const BUBBLE_SLIDER_WIDTH: f64 = 220.0;

/// D1 取证：`BLADE2_PILLDBG=1` 时把每次重绘选到的档位与收到的状态消息打到 stderr。
fn pill_trace(text: String) {
    if env::var("BLADE2_PILLDBG").is_ok() {
        eprintln!("{text}");
    }
}

fn pill_button(
    button: Button,
    glyph: View,
    pill: Pill,
    key: &str,
    slot: HoverSlot,
    hover: Option<&str>,
    pressed: Option<&str>,
    context: &mut ViewContext<Shell>,
) -> View {
    let (face, tier) = if !pill.enabled {
        (pill.disabled.unwrap_or(pill.rest), "disabled")
    } else if pressed == Some(key) {
        (pill.pressed, "press")
    } else if hover == Some(key) {
        (pill.hover, "hover")
    } else {
        (pill.rest, "rest")
    };
    pill_trace(format!("PILL-VIEW {key} tier={tier} op={}", face.opacity));
    let entered = key.to_string();
    let (held, held_r, held_c, held_x) = (key.to_string(), key.to_string(), key.to_string(), key.to_string());
    let pill_slot = slot == HoverSlot::Pill;
    button
        .width(pill.side)
        .height(pill.side)
        .min_width(0.0)
        .min_height(0.0)
        .vertical_alignment(VerticalAlignment::Center)
        .is_enabled(pill.enabled)
        .resource_overrides(button_ghost(pill.foreground))
        .content(
            Border::new()
                .width(pill.side)
                .height(pill.side)
                .margin(th(TEMPLATE_INSET))
                .corner_radius(pill.radius_clamped())
                .background(face.brush)
                // 主干默认模板那 1px 描边（`ButtonBorderThemeThickness`=1）：写了
                // `BorderThickness="0"` 的族 `pill.border=0` ⇒ 透明描边零宽，等于不画。
                .border_brush(face.stroke)
                .border_thickness(th([pill.border; 4]))
                .opacity(face.opacity)
                .on_pointer_entered(context.callback(move |_| {
                    if pill_slot {
                        Msg::PillHover(Some(entered.clone()))
                    } else {
                        Msg::Hover(Some(entered.clone()))
                    }
                }))
                .on_pointer_exited(context.callback(move |_| {
                    if pill_slot {
                        Msg::PillHover(None)
                    } else {
                        Msg::Hover(None)
                    }
                }))
                .on_pointer_pressed(context.callback(move |info: PointerEventInfo| {
                    pill_trace(format!(
                        "PILL-EVT press {held} lb={} cap={}",
                        info.is_left_button_pressed, info.capture_succeeded
                    ));
                    Msg::Press(Some(held.clone()))
                }))
                .on_pointer_released(context.callback(move |info: PointerEventInfo| {
                    pill_trace(format!("PILL-EVT release {held_r} lb={}", info.is_left_button_pressed));
                    Msg::Press(None)
                }))
                .on_pointer_capture_lost(context.callback(move |_| {
                    pill_trace(format!("PILL-EVT caplost {held_c}"));
                    Msg::Press(None)
                }))
                .on_pointer_canceled(context.callback(move |_| {
                    pill_trace(format!("PILL-EVT cancel {held_x}"));
                    Msg::Press(None)
                }))
                .content(glyph),
        )
}

/// 品牌标（`Assets/Square44x44Logo.png`）：主干那两颗 `Image` 的共用体（空态 `HeroMarkImage`
/// 与加载卡 `KernelBootMark`，MX:687/703）。第三参 = 这枚 `Image` 的 automation_id，
/// 传空串就没有（主干只有 `HeroMarkImage` 那枚 x:Name，分叉的 UIA 侧不需要它）。
fn brand_mark(path: Option<&Path>, side: f64, automation_id: &str) -> View {
    let Some(path) = path else { return blank() };
    let image = match Image::new().source_file(path) {
        Ok(image) => image,
        Err(_) => return blank(),
    };
    let image = image
        .width(side)
        .height(side)
        .horizontal_alignment(HorizontalAlignment::Center)
        .vertical_alignment(VerticalAlignment::Center);
    if automation_id.is_empty() {
        image.into()
    } else {
        image.automation_id(automation_id).into()
    }
}

/// 自测标记的第二条腿：`BLADE2_RS_LOG=<文件路径>` 时，每条 `STATUS:` / `DIAG:` 行除了打 stdout，
/// 还按行 open→append→close 落进那个文件。
///
/// 为什么要自己落盘（2026-09-22 查「`qa3-app.out.txt` 恒 0 字节」查出来的）：驱动链里分叉那根
/// stdout 是 `tests/gui_uia.ps1` 用 `Start-Process -RedirectStandardOutput` 递进来的**继承句柄**，
/// 同前缀的第二轮跑法会先把那个文件 `Remove-Item` 再重建 ⇒ 分叉继续往已被 unlink 的旧文件对象里写，
/// 驱动手里那份同名新文件永远读不到标记。实测两份观测对不上就是这件事的铁证：分叉活着的时候
/// `Get-Item` 报 1 字节、按 FileShare=ReadWrite 打开同一个路径却读到 413 字节。
/// 自己按行开/追/关 ⇒ 没有长驻句柄、没有 delete-pending、也不受句柄继承摆布。
fn log_file() -> Option<&'static Path> {
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| env::var_os("BLADE2_RS_LOG").filter(|v| !v.is_empty()).map(PathBuf::from))
        .as_deref()
}

/// 一行诊断的总出口：stdout（手跑直接看得见）+ 可选的 `BLADE2_RS_LOG` 文件（驱动轮询用）。
/// 落盘失败一律忽略 —— 诊断通道不许把界面拖下水。
fn emit_line(line: &str) {
    println!("{line}");
    let Some(path) = log_file() else { return };
    use std::io::Write as _;
    use std::os::windows::fs::OpenOptionsExt as _;
    // share_mode = FILE_SHARE_READ|WRITE|DELETE：驱动每 400ms 开一次这份文件轮询，
    // 收尾还要能 Remove-Item 掉它，三个共享位少一个都会把对方挡成 IOException。
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .share_mode(0x1 | 0x2 | 0x4)
        .open(path)
    else {
        return;
    };
    let _ = writeln!(file, "{line}");
}

/// 一条 mux 流元素在分叉侧的去向。主干每条流各挂一个回调（会话 follow → 正文、
/// `session/control` → `OnControlFrame`（MW:15207）、`$events` 的 status → `OnSessionStatusAsync`、
/// `workspace/follow` → 树），而分叉只有一根 socket、一个事件泵 ⇒ 得自己先把流认出来。
#[derive(Clone, Debug, PartialEq, Eq)]
enum MuxSink {
    /// 某会话的 `session/follow`：正文帧，带这一路登记着的 sessionId。
    Follow(String),
    /// `session/control` 长驻流：按帧型灌 `ControlState` 那三张表。
    Control,
    /// `$events` 的 `api-session/status`（已解出的载荷：会话 id + 在跑）。
    Status(String, bool),
    /// 剩下的一切都算 `workspace/follow` 的表帧。
    Workspace,
}

/// 分流判据（纯函数：只吃 streamId 与帧，不碰 `Shell`）。
///
/// **先认流、再认帧型**，而且控制流那一支必须排在兜底之前：`baseline` 这个 `type` 在两条流上
/// 同名不同形（工作区读 `value.items`、控制面读 `value.queues/jobs/projections`），只看帧型
/// 必然串台；只开流不分流的话四种控制帧全会被 `WorkspaceTree::apply` 判成「不认」并返回
/// `false` ⇒ 三张表永不落地、一条错误都不报（spec6 R1）。
fn mux_frame_sink(
    stream: &str,
    control_stream: Option<&str>,
    follows: &[(String, String)],
    frame: &Value,
) -> MuxSink {
    if Some(stream) == control_stream {
        return MuxSink::Control;
    }
    if let Some((session, _)) = follows.iter().find(|(_, id)| id == stream) {
        return MuxSink::Follow(session.clone());
    }
    match session_status_event(frame) {
        Some((id, running)) => MuxSink::Status(id, running),
        None => MuxSink::Workspace,
    }
}

/// 一批 `session/control` 元素的汇总：攒完一批才出一行 DIAG（`poll_mux` 约每 250ms 一批，
/// 逐帧一行会把日志刷爆），口径同工作区流那句「工作区数: n」。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ControlTally {
    /// 内核那四型各到了几帧，键取自 `CONTROL_FRAME_TYPES`。
    kinds: BTreeMap<&'static str, usize>,
    /// 不认的 `type`（含 `type` 缺席）帧数。主干 `OnControlFrame` 的 `default: return`
    /// （MW:15336）是静默丢，分叉这边必须留个计数：否则「内核新增/改名一个帧型」和
    /// 「这一帧本来就是空的」在界面上同形。
    unknown: usize,
}

impl ControlTally {
    /// 记一帧，`kind` = `control_frame_type` 认出来的型别（`None` = 未识别）。
    fn record(&mut self, kind: Option<&'static str>) {
        match kind {
            Some(kind) => *self.kinds.entry(kind).or_default() += 1,
            None => self.unknown += 1,
        }
    }

    /// 这一批出声的那一行；一帧控制面元素都没见到就返回 `None`（不写空行）。
    /// `control` 一起报：帧数与三张表的实际规模并排，「帧到了但表是空的」才看得出来了。
    fn line(&self, control: &ControlState) -> Option<String> {
        let seen: usize = self.kinds.values().sum::<usize>() + self.unknown;
        if seen == 0 {
            return None;
        }
        let kinds = CONTROL_FRAME_TYPES
            .iter()
            .map(|kind| format!("{kind} {}", self.kinds.get(kind).copied().unwrap_or_default()))
            .collect::<Vec<_>>()
            .join(" ");
        Some(format!(
            "控制面 {seen} 帧: {kinds} 未识别 {} · 队列 {} 项 / 作业 {} 项 / 投影 {} 会话",
            self.unknown,
            control.queues.values().map(Vec::len).sum::<usize>(),
            control.jobs.values().map(Vec::len).sum::<usize>(),
            control.projections.len(),
        ))
    }
}

// ==================== 斜杠命令浮层（`CommandPalette`） ====================
//
// 主干那几份状态在这里合成一个 `PaletteState`：
// · `_commands` / `_commandsSession` / `_commandsError` / `_commandsErrorAt`（MW:2164-2170）
//   = 会话作用域的目录缓存，含那条 3 秒负缓存；
// · `_commandMatches` / `_commandIndex`（MW:2172-2174）= 浮层里这一批候选与选中行；
// · `_paletteGeneration`（MW:2175）= 「已有更新的按键」那道丢弃闸；
// · `CommandPalette.Visibility`（MX:746）= `showing`：reactor 没有 Visibility，浮层整棵由
//   `chat_page` 的 keyed 槽位增删（缺席即 Destroy）；
// · `CommandList.Visibility`（MW:17710 / 17729）= `list_visible`：目录取不到时列表收起、只留标题。

/// 主干 `_commandsErrorAt` 那条重试窗口（MW:17320-17323 `TimeSpan.FromSeconds(3)`）。
const COMMANDS_NEGATIVE_CACHE: Duration = Duration::from_secs(3);
/// 主干 `Take(20)`（MW:17680）。
const PALETTE_MATCH_LIMIT: usize = 20;
/// MX:748-752 的浮层几何：`MaxWidth=640` / `MaxHeight=264`；MX:774 列表 `MaxHeight=216`。
const PALETTE_MAX_WIDTH: f64 = 640.0;
const PALETTE_MAX_HEIGHT: f64 = 264.0;
const PALETTE_LIST_MAX_HEIGHT: f64 = 216.0;
/// MX:782 `ListViewItem Padding="8,5,8,5"`（同处还有 `MinHeight=0` ⇒ 行不给最小高）。
const PALETTE_ROW_PADDING: [f64; 4] = [8.0, 5.0, 8.0, 5.0];

/// 主干 `UpdateCommandPalette` 的触发判据（MW:17684-17694）的纯函数版：
/// 文本非空、首字符是 `/`、**整串**里没有一个空格/制表/回车/换行 ⇒ `Some(过滤词)`（`/` 之后那段）；
/// 否则 `None` = 收浮层。进入参数段（打了空格）浮层就让位给普通发送。
fn palette_trigger(text: &str) -> Option<&str> {
    if !text.starts_with('/') || text.contains([' ', '\t', '\r', '\n']) {
        return None;
    }
    Some(text.get(1..).unwrap_or_default())
}

/// 主干 `TryLeadingCommandName`（MW:6901-6912）：取前导斜杠命令名（`"/compact now"` → `compact`）。
/// 与上面那条的区别就是这里**允许**参数段——composer 里回车提交的那一发要能认出带参数的命令。
fn leading_command_name(text: &str) -> Option<&str> {
    let trimmed = text.trim_start();
    if trimmed.len() < 2 || !trimmed.starts_with('/') {
        return None;
    }
    let body = &trimmed[1..];
    let end = body
        .find([' ', '\t', '\r', '\n'])
        .unwrap_or(body.len());
    let name = body[..end].trim();
    (!name.is_empty()).then_some(name)
}

/// `StringComparer.OrdinalIgnoreCase` / `StringComparison.OrdinalIgnoreCase` 的等价物：
/// .NET 走**简单**大小写折叠（一字对一字，`ß` 仍是 `ß`），Rust 的 `to_lowercase` 是完整折叠
/// （`ß` → `ss`，长度都会变、序也就跟着变）⇒ 这里逐字折叠，且只在折叠结果仍是单个 char 时采纳。
/// 命令名几乎都是 ASCII，这段存在的唯一理由是别把 C# 的序判悄悄抄成 Rust 的序判。
fn fold(ch: char) -> char {
    let mut lowered = ch.to_lowercase();
    match lowered.next() {
        Some(one) if lowered.next().is_none() => one,
        _ => ch,
    }
}

fn eq_ignore_case(text: &str, other: &str) -> bool {
    text.chars().map(fold).eq(other.chars().map(fold))
}

fn cmp_ignore_case(text: &str, other: &str) -> Ordering {
    text.chars().map(fold).cmp(other.chars().map(fold))
}

fn contains_ignore_case(text: &str, needle: &str) -> bool {
    let needle: Vec<char> = needle.chars().map(fold).collect();
    if needle.is_empty() {
        return true;
    }
    let haystack: Vec<char> = text.chars().map(fold).collect();
    needle.len() <= haystack.len() && haystack.windows(needle.len()).any(|row| row == needle)
}

fn starts_with_ignore_case(text: &str, prefix: &str) -> bool {
    let mut chars = text.chars().map(fold);
    prefix.chars().map(fold).all(|head| chars.next() == Some(head))
}

/// 主干那段过滤（MW:17716-17722）逐字照抄：大小写无关 `Contains` → 前缀命中的排前面
/// （`OrderByDescending(StartsWith)`）→ 同档按名字 `OrdinalIgnoreCase` 升序 → `Take(20)`。
/// `sort_by` 是稳定排序，等价于 LINQ 的 `OrderBy`。零命中主干什么都不显示（无「无匹配」占位）。
fn filter_commands(entries: &[CommandEntry], query: &str) -> Vec<CommandEntry> {
    let mut matches: Vec<CommandEntry> = entries
        .iter()
        .filter(|entry| contains_ignore_case(&entry.name, query))
        .cloned()
        .collect();
    matches.sort_by(|a, b| {
        starts_with_ignore_case(&b.name, query)
            .cmp(&starts_with_ignore_case(&a.name, query))
            .then_with(|| cmp_ignore_case(&a.name, &b.name))
    });
    matches.truncate(PALETTE_MATCH_LIMIT);
    matches
}

/// 主干 `AcceptCommandSelection`（MW:17802-17819）的三种落点。
#[derive(Clone, Debug, PartialEq, Eq)]
enum PaletteAccept {
    /// 带参数的命令（`input` 是对象）：只把 `/名字` + 一个空格写回输入框，**不执行**
    /// （主干注释：「原版 leading claim 语义」）。 typed 前缀被整名替换、`/` 仍在。
    Fill(String),
    /// 无参命令：清空输入框并直接 `commands/execute`（这一发是自动发出的，不等再按 Enter）。
    Run(String),
    /// 选中行不存在（`_commandIndex` 越界，含「目录取不到」那一态的 -1）：只收浮层。
    Nothing,
}

/// 命令浮层的全部状态（见本节开头的逐字段主干对应）。
struct PaletteState {
    /// `_commands`：目录本体，按会话缓存。
    entries: Vec<CommandEntry>,
    /// `_commandsSession`：这份目录属于哪个会话（= 当时那次 `commands/list` 的 `agentId`）。
    session: String,
    /// `_commandsError` + `_commandsErrorAt`：负缓存，只活 [`COMMANDS_NEGATIVE_CACHE`]。
    error: Option<(String, Instant)>,
    /// 当前过滤词（`/` 之后那段）。
    query: String,
    /// `_commandMatches`。
    matches: Vec<CommandEntry>,
    /// `_commandIndex`：`-1` = 没有选中项（主干收起时就是这个值）。
    index: i32,
    /// `_paletteGeneration`。
    generation: u64,
    /// `CommandPalette.Visibility == Visible`。
    showing: bool,
    /// `CommandList.Visibility == Visible`（目录取不到时为假，标题那一行还在）。
    list_visible: bool,
}

impl PaletteState {
    /// 初值 = 主干 XAML 那句 `Visibility="Collapsed"`（MX:746）+ `_commandIndex = -1`。
    fn idle() -> Self {
        PaletteState {
            entries: Vec::new(),
            session: String::new(),
            error: None,
            query: String::new(),
            matches: Vec::new(),
            index: -1,
            generation: 0,
            showing: false,
            list_visible: true,
        }
    }

    /// 主干 `HideCommandPalette`（MW:17743-17751）：代次 +1（在途那一发就此作废）、
    /// 清候选与选中、收浮层。主干还会顺手收 @ 引用浮层——那棵分叉没有。
    fn hide(&mut self) {
        self.generation += 1;
        self.matches.clear();
        self.index = -1;
        self.showing = false;
        self.list_visible = true;
    }

    /// 主干 `EnsureCommandsAsync` 开头那三段守卫（MW:17310-17323）里「要不要打 RPC」的那一条。
    /// 语义逐字照抄，包括两条容易被「顺手改好」的：
    /// · 正缓存按会话，**没有**过期时间（成功过一次就不再问，直到切会话或出过一次错）；
    /// · 负缓存那道**不按会话判**（17320 那只比时间），所以切了会话也可能再等一会儿。
    fn needs_fetch(&self, session: &str, now: Instant) -> bool {
        if session.is_empty() {
            return false;
        }
        if self.session == session && self.error.is_none() {
            return false;
        }
        if let Some((_, at)) = &self.error {
            return now.saturating_duration_since(*at) >= COMMANDS_NEGATIVE_CACHE;
        }
        true
    }

    /// 回填（主干 17346-17357）：成功写目录并清错误；失败**清目录**、连会话一起记错误与时间戳。
    fn record(&mut self, session: &str, outcome: Result<Vec<CommandEntry>, String>, now: Instant) {
        match outcome {
            Ok(entries) => {
                self.entries = entries;
                self.session = session.to_string();
                self.error = None;
            }
            Err(message) => {
                self.entries.clear();
                self.session = session.to_string();
                self.error = Some((message, now));
            }
        }
    }

    /// 主干 `ShowCommandPaletteAsync` 里 await 之后那一段（MW:17699-17736）。三态逐条照抄：
    /// · 代次不符 ⇒ 整段丢弃（17701「已有更新的按键」）；
    /// · 目录取不到 ⇒ 列表收起、原因摊在标题上（17706-17714，**浮层照样上屏**，不静默）；
    /// · 零命中 ⇒ `HideCommandPalette()`（17722-17726）——主干没有「无匹配」占位，别自己加。
    ///
    /// `input` 那道触发复查是结构差异逼出来的：主干只在 `UpdateCommandPalette` 判过触发才调
    /// `ShowCommandPaletteAsync`，而这里的回填是异步的（composer 那条提交路径也会拉目录，
    /// 那时输入里已经带参数段、浮层早让位了）。
    fn apply(&mut self, generation: u64, input: &str) {
        if generation != self.generation || palette_trigger(input).is_none() {
            return;
        }
        if self.error.is_some() {
            self.matches.clear();
            self.index = -1;
            self.list_visible = false;
            self.showing = true;
            return;
        }
        let matches = filter_commands(&self.entries, &self.query);
        if matches.is_empty() {
            self.hide();
            return;
        }
        self.matches = matches;
        self.index = 0;
        self.list_visible = true;
        self.showing = true;
    }

    /// 主干 `MoveCommandSelection`（MW:17791-17801）：没有候选直接返回，越界夹到端点（不循环）。
    fn move_selection(&mut self, delta: i32) {
        let count = self.matches.len() as i32;
        if count == 0 {
            return;
        }
        self.index = (self.index + delta).clamp(0, count - 1);
    }

    /// 主干 `AcceptCommandSelection`（MW:17802-17819）里「读选中项 + 收浮层」那半截；
    /// 输入框与 `commands/execute` 由调用方落（那两处才是 `Shell` 的状态）。
    fn take_accept(&mut self) -> PaletteAccept {
        let picked = usize::try_from(self.index)
            .ok()
            .and_then(|index| self.matches.get(index).cloned());
        let Some(command) = picked else {
            // 主干 17806-17809：越界只收浮层。
            self.hide();
            return PaletteAccept::Nothing;
        };
        let accept = if command.has_input {
            PaletteAccept::Fill(format!("{} ", command.slash_name()))
        } else {
            PaletteAccept::Run(command.slash_name())
        };
        self.hide();
        accept
    }

    /// 标题那一行的三态（主干 `CommandPaletteHeader`：17712 错误态 / 17732-17734 两态）。
    fn header(&self, catalog: &Catalog) -> String {
        if let Some((error, _)) = &self.error {
            return catalog.lf("命令目录不可用：{0}", &[error.clone()]);
        }
        if self.query.is_empty() {
            return catalog.l("命令（↑↓ 选择，Enter 执行，Esc 关闭）");
        }
        catalog.lf(
            "“/{0}” 匹配 {1} 条（↑↓ 选择，Enter 执行，Esc 关闭）",
            &[self.query.clone(), self.matches.len().to_string()],
        )
    }
}

/// 主干 `MainWindow.xaml` 的壳层：48px 顶条 + 264px 侧栏 + SurfaceAlt 内容面。
struct Shell {
    page: Page,
    kernel: Option<Shared>,
    /// 上一次**打到 stdout** 的内核状态（`STATUS:` 行去重用；窗口里不再呈现，见 `set_status`）。
    status: String,
    rows: Vec<SessionInfo>,
    input: String,
    search: String,
    active: Option<String>,
    catalog: Catalog,
    scheme: Scheme,
    /// 主线 `MainWindow.KernelBoot.cs` 的内容面加载卡（替掉旧的整窗遮罩 `loading_layer`）。
    boot: BootState,
    /// 引导线程 → tick 排水口的阶段上报通道（`None` = 本轮没有后台任务在跑）。
    boot_events: Option<mpsc::Receiver<BootEvent>>,
    /// 主线 `_kernelBootWatch`（`Stopwatch`）：上屏时 `Restart`，tick 读整秒写进 `boot.seconds`。
    boot_started: Option<Instant>,
    /// 本轮引导的后台句柄。主线 `CancelKernelBoot` 的对应物：重试时先 `cancel()`
    /// （排队中的结果会被摘掉，跑完的那发也因 `TaskControl` 已置 Cancelled 而丢弃）。
    boot_task: Option<ComponentTask>,
    hover: Option<String>,
    /// 正被按住的圆底钮（`Msg::Press`）；任何非 Press 消息都会清掉，见 `update`。
    pressed: Option<String>,
    /// 圆底钮**自身**的悬停档（`Msg::PillHover`），与上面那个 `hover`（气泡/行级提亮）分槽：
    /// `pill_button` 画圆底的那层 Border 在气泡卡内部，指针进入时先触发内层（钮）、再冒泡到
    /// 外层（卡），同一个槽会被外层抢走 ⇒ 钮永远画不出悬停档。分槽后行级提亮与钮级高亮互不打架。
    pill_hover: Option<String>,
    mux: Option<Arc<Mutex<Mux>>>,
    tree: WorkspaceTree,
    expanded: Vec<String>,
    uncapped: Vec<String>,
    /// 设置页当前分区 id（主干默认 general）。
    section: String,
    /// 设置页当前钻取的二级页 id；`None` 即停在分区一级页（主干 `_settingsSubPages` 栈）。
    sub: Option<String>,
    /// 主干 `MakeTextBox` / `MakePasswordBox` 的当前值（内核快照回落，分叉只存内存）。
    fields: Vec<(String, String)>,
    /// 主干 `MakeNumberBox` 的当前值。
    numbers: Vec<(String, f64)>,
    /// 会话正文字号，主干 `ui-theme/fontSize`，区间 12–17。
    font_size: f64,
    /// 设置页里各下拉框的当前选项下标；主干靠 `settings/mutate` 持久化，分叉先只存在内存里。
    picks: Vec<(String, usize)>,
    /// 设置页里各开关的当前状态；主干这两组写 `%LOCALAPPDATA%\Blade2\shell.json`，分叉不落盘。
    switches: Vec<(String, bool)>,
    /// 最近一次从窗口读到的系统配色，供「跟随系统」回落。
    system_scheme: Scheme,
    /// 运行中的会话 id，只由 `$events` 的 api-session/status 驱动。
    running: Vec<String>,
    /// 会话正文（主干 `_messages`），只对应当前 active 会话那一段。
    bubbles: Vec<Bubble>,
    /// 已开过的 `session/follow` 流：sessionId → streamId。
    follows: Vec<(String, String)>,
    /// `session/control` 长驻流的 streamId（主干 `_controlStreamId`，MW:2022）；
    /// `None` = 还没开成，或这一路已经被网关判死。
    control_stream: Option<String>,
    /// 那条流的累积状态：各会话的排队项 / 作业 / 投影三张表（主干 `_queues`/`_jobs`/
    /// `_sessionProjections`，MW:2016-2021）。写它的只有 `apply_mux_events` 的 Control 那一支。
    control: ControlState,
    /// 正在按 token 追加的那次尝试，`assistant/message` 落地后清空。
    live: Option<LiveAttempt>,
    /// 主干 `_pendingUserBubble`：等 wire ack 回来才落进正文的那句话。
    echo: Option<String>,
    /// 主干 `_pendingBubble`：已发出但还没收到第一个 token。
    thinking: bool,
    /// 主干 `_currentModelId`：最近一条 `request/header` 记的模型 id，盖到之后的回答气泡。
    header_model: String,
    /// 主干 `_pendingUserBubble`：刚落、还没被 `request/header` 认领型号的那条用户气泡 seq。
    pending_user: Option<i64>,
    /// 主干 RunStats.SystemPromptSeen：同一会话第二条 system/message 起改说「系统提示词更新」。
    system_prompt_seen: Vec<String>,
    /// 主干 `_turnStartTicks`：turn → `turn/start` 的 time，`turn/end` 用来算用时。
    turn_starts: Vec<(i64, i64)>,
    /// 主干 `_openMutationPaths`：本轮在途 callId → 突变目标路径（非突变登记空串），
    /// `tool/call` 写、`tool/result` 读，`turn/start` 整表清空。
    mutations: Vec<(String, String)>,
    /// 主干 `_producedByTurn`：turn → 成功突变（结果信封 seq, 路径），按到达顺序累加。
    produced: Vec<(i64, Vec<(i64, String)>)>,
    /// 处于「已复制」态的气泡键（操作行图标换成对勾那一下）。
    copied: Option<String>,
    /// 展开了思考段的气泡键；不在表里即收起（主干 `ReasoningExpanded` 默认 false）。
    reasoning_open: Vec<String>,
    /// 主干 `_feedback`：当前会话已提交的反馈（messageId → 条目），list 拉取 + put/delete 回写。
    feedback: Vec<(String, FeedbackItem)>,
    /// 主干 `_feedbackBusy`：在途的反馈 RPC。只用来静默去重（防连点抖出 version-conflict），
    /// 主干也明确不禁用按钮，所以这份表不影响任何视觉态。
    feedback_busy: Vec<String>,
    /// 主干各行 `Status` 文本（messageId → 文案）：只在行内显示错误，不弹提示、不写系统消息行。
    feedback_status: Vec<(String, String)>,
    /// 说明编辑器（`ContentDialog`）的目标 messageId；`None` 即编辑器没开。
    note_for: Option<String>,
    /// 说明编辑器当前文本（主干直接读 TextBox.Text，分叉得把它镜像进 state 才能声明式取回）。
    note_text: String,
    /// 「最近一次谁在吃键盘」（`blade2_rs::keys` 的焦点替身，初值 `Other` = 谁都不认）。
    /// 每次 `update` 收尾连同 `sub` 一起推给钩子的 thread-local（`keys::sync`）：钩子在消息泵里
    /// 回调，拿不到 `&Shell`，只能读这份副本。失配场景见 `keys` 模块头的说明。
    key_owner: KeyOwner,
    /// 命令浮层（主干 `CommandPalette` 那一族字段，见本节开头逐条对应）。
    palette: PaletteState,
    /// 主干 `SubmitInputAsync`（MW:6885）里那条 `await EnsureCommandsAsync()`：目录还没取到时
    /// 这一发提交先挂着，`commands/list` 回来（成或败都算）后补发。`Some(None)` = 普通 Enter 那一发。
    pending_send: Option<Option<&'static str>>,
    /// 主干 `_commandSubmitting`（MW:17846）：一次只允许一发 `commands/execute` 在飞。
    command_submitting: bool,
    brand: Option<PathBuf>,
}

impl Shell {
    /// 诊断出口 = **stdout + 可选的 `BLADE2_RS_LOG` 文件**，界面上一个像素都不留。
    ///
    /// 两类行（`rust/tests/gui_uia.ps1 -WaitForLog` 与 `qa2_drive.ps1 -WaitForLog` 按子串轮询）：
    /// · `STATUS: 内核: 已连接 http://127.0.0.1:xxxx/?token=… (pid n)` —— 内核状态每次变化一行；
    /// · `DIAG: <一行诊断>` —— 过程信息（工作区数 / 会话数 / 打开失败 / 分叉失败 …）。
    ///
    /// 上一轮这些走的是窗口底部那条自测条（`debug_strip`）：它把聊天列往上顶掉一截，
    /// 用户明确要求「下巴」彻底去掉。轮询的文件有两份，口径见 `emit_line`：
    /// `<log>.out.txt` 是驱动用继承句柄收走的 stdout（滞后、可被同前缀的另一轮 unlink 掉），
    /// `<log>.rs.txt` 是分叉自己按行 open/append/close 写的那份（驱动一律先读它）。
    fn push_log(&mut self, line: impl Into<String>) {
        let line = line.into();
        if line.is_empty() {
            return;
        }
        for entry in line.lines() {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            emit_line(&format!("DIAG: {entry}"));
        }
    }

    /// 内核状态唯一写入口：赋值 + 打一行 `STATUS:`，取代自测条上那个常驻 TextBlock。
    fn set_status(&mut self, status: impl Into<String>) {
        let status = status.into();
        if status == self.status {
            return;
        }
        self.status = status;
        emit_line(&format!("STATUS: 内核: {}", self.status));
    }

    fn fail(&mut self, error: String) {
        self.set_status(format!("失败: {error}"));
        self.push_log(error);
    }

    /// 主线 `FailKernelBoot(reasonKey, detail)`（KC:138-156）：加载卡**就地**转失败态，
    /// 条子停在失败瞬间的位置，提示行给原因，重试钮出现。
    /// 分叉没有 `ShellToast` 原语 ⇒ 主线 KC:155 那句重复播报改成一行 `STATUS:`/`DIAG:` 诊断。
    fn kernel_boot_failed(&mut self, reason_key: &str, detail: &str) {
        self.boot.fail(reason_key, detail);
        let message = self.catalog.l(reason_key);
        let full = if detail.is_empty() {
            message
        } else {
            // 全角冒号 U+FF1A，与主线 KC:144 一致。
            format!("{message}：{detail}")
        };
        self.set_status(format!("失败: {full}"));
        self.push_log(full);
    }

    fn brand(&self) -> Option<&Path> {
        self.brand.as_deref()
    }

    /// 主干 `SessionDisplayTitle()` + 显示边界的 `新会话` 哨兵替换：
    /// blank 才是「新会话」，缺标题要退到 cwd 目录名，再退到 sessionId。
    fn row_title(&self, row: &SessionInfo) -> String {
        if row.blank || row.title == "新会话" {
            return self.catalog.l("新会话");
        }
        if !row.title.is_empty() {
            return row.title.clone();
        }
        let base = workspace_basename(&row.cwd);
        if base.is_empty() {
            row.id.clone()
        } else {
            base.to_string()
        }
    }

    /// 主干唯一的排除条件是 archived；工作区树为空时全部落在「未分组」。
    fn visible_rows(&self) -> Vec<&SessionInfo> {
        let needle = self.search.trim().to_lowercase();
        self.tree
            .visible(&self.rows)
            .into_iter()
            .filter(|row| {
                needle.is_empty()
                    || self.row_title(row).to_lowercase().contains(&needle)
                    || row.cwd.to_lowercase().contains(&needle)
            })
            .collect()
    }

    fn toggle_group(&mut self, key: &str) {
        toggle(&mut self.expanded, key);
    }

    fn set_cap(&mut self, key: &str, open: bool) {
        match (open, self.uncapped.iter().position(|item| item == key)) {
            (true, None) => self.uncapped.push(key.to_string()),
            (false, Some(index)) => {
                self.uncapped.remove(index);
            }
            _ => {}
        }
    }

    fn group_open(&self, key: &str) -> bool {
        self.expanded.iter().any(|item| item == key)
    }

    fn set_running(&mut self, id: &str, running: bool) {
        if running {
            if !self.running.iter().any(|item| item == id) {
                self.running.push(id.to_string());
            }
        } else {
            self.running.retain(|item| item != id);
        }
    }

    fn is_running(&self, id: &str) -> bool {
        self.running.iter().any(|item| item == id)
    }

    /// 设置页下拉框的当前选项；未选过就用主干默认值。
    fn pick(&self, key: &str, fallback: usize) -> usize {
        self.picks
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, index)| *index)
            .unwrap_or(fallback)
    }

    fn set_pick(&mut self, key: &str, index: usize) {
        match self.picks.iter_mut().find(|(name, _)| name == key) {
            Some(entry) => entry.1 = index,
            None => self.picks.push((key.to_string(), index)),
        }
    }

    fn is_on(&self, key: &str, fallback: bool) -> bool {
        self.switches
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, on)| *on)
            .unwrap_or(fallback)
    }

    fn set_on(&mut self, key: &str, on: bool) {
        match self.switches.iter_mut().find(|(name, _)| name == key) {
            Some(entry) => entry.1 = on,
            None => self.switches.push((key.to_string(), on)),
        }
    }

    /// 主干 `NsNumber(ns, field, 默认)`：内核快照读不到时的回落值。
    fn number(&self, key: &str, fallback: f64) -> f64 {
        pair_value(&self.numbers, key, fallback)
    }

    /// 主干 `NsString(ns, field, 默认)`。
    fn field(&self, key: &str, fallback: &str) -> String {
        match self.fields.iter().find(|(name, _)| name == key) {
            Some((_, value)) => value.clone(),
            None => fallback.to_string(),
        }
    }

    /// 当前会话是否在运行 —— 决定发送键画箭头还是停止方块。
    fn active_running(&self) -> bool {
        self.active.as_deref().is_some_and(|id| self.is_running(id))
    }

    /// 把键盘通道要的那份事实贴出去（钩子只读 thread-local 副本，绝不回碰 UI）。
    /// 焦点归属 = `key_owner`（模型里推，见 `Msg::KeyFocus` 那条），二级页开关 = `sub`，
    /// 在不在聊天页 = `page`（决定 Esc 是否等价于主干那颗「返回」）。
    /// `update` 每次收尾调一发，所以钩子读到的永远是本轮消息落完之后的状态。
    ///
    /// `palette_open` 现在写死 `false` —— 那是 #52（命令面板）的接缝：浮层一落地，把这里的
    /// 字面量换成模型里那个开关即可，`keys::classify` 那几档 `Palette*` 与
    /// `Msg::from_key_action` 的空臂立刻接上，本模块其余代码一行不用改。
    fn publish_keys(&self) {
        keys::sync(self.key_owner, self.sub.is_some());
        // `palette_open` = 浮层**真的在屏幕上**：它挂在 ChatPage 内容面那一格，切到设置页时整棵
        // 随页退役（keyed 槽位缺席即 Destroy），而主干那颗 InputBox 也在折叠的页里拿不到焦点。
        // 不叠这一条就会让「设置页 + 一次迟到的目录回填」把 Enter 判成采纳一个看不见的浮层。
        let on_chat = self.page == Page::Chat;
        keys::sync_pages(on_chat, on_chat && self.palette.showing);
    }

    /// 主干 `OpenSessionAsync` 清屏那串里的会话作用域三项（MW:4085-4088）：
    /// `_commandsSession = null` + `_commandsError/_commandsErrorAt = null` + `HideCommandPalette()`。
    /// **`_commands` 本体不清**——主干留着它，靠 `agentId` 对不上去逼下一次重新问；
    /// 分叉照抄（清了目录会让「切回去」那一下变成白屏 + 一次额外 RPC）。
    fn forget_commands(&mut self) {
        self.palette.session = String::new();
        self.palette.error = None;
        self.palette.hide();
    }

    /// 主干 `NsString("ui-conversation", "busyEnter", "queue")` 的分叉等价物：设置页 `busy`
    /// 那一行就是这条下拉（`["排队发送", "插话发送"]`，默认下标 0 = queue）。
    fn busy_enter_mode(&self) -> &'static str {
        if self.pick("ui-conversation_busyEnter", 0) == 1 {
            "steer"
        } else {
            "queue"
        }
    }

    /// 设置·个性化「消息气泡」那一支下拉选中的材质档（主干 `_bubbleMaterial`）。
    /// 默认下标 0 = 半透明，与主干 `BubbleMaterialTranslucent` 同档。
    fn bubble_material(&self) -> BubbleMaterial {
        BubbleMaterial::from_index(self.pick(BUBBLE_MATERIAL_KEY, 0))
    }

    /// 窗口材质档（主干 `_shellMaterial`）：只有「跟随窗口材质」那一档的气泡看它脸色。
    /// 主干这四项存 shell.json，分叉不落盘，读的就是「窗口材质」卡那颗下拉的当前下标。
    fn window_material(&self) -> WindowMaterial {
        WindowMaterial::from_index(self.pick("shell_material", 0))
    }

    /// 气泡不透明度（主干 `_bubbleOpacity`，0.2–1.0）：滑杆是 20–100 的百分刻度，
    /// 这里换算回系数并夹进合法域（主干 `SetBubbleOpacity` 的 `Math.Clamp` 同口径）。
    fn bubble_opacity(&self) -> f64 {
        let percent = self.number(BUBBLE_OPACITY_KEY, BUBBLE_OPACITY_DEFAULT);
        theme::clamp_percent_to_opacity(percent)
    }

    /// 主干 `AlternateBusyEnter()`（MW:6919-6920）：queue↔steer 取反向档；
    /// 而 `Ctrl+Enter` 在**空闲**时沿用默认发送（MW:6859 的 `forceMode: IsSessionBusy() ? … : null`）
    /// ⇒ 空闲就交回 `None`，让 `submit_input` 走与点发送钮一模一样的那条路。
    fn ctrl_enter_mode(&self) -> Option<&'static str> {
        if !self.active_running() {
            return None;
        }
        Some(if self.busy_enter_mode() == "steer" {
            "queue"
        } else {
            "steer"
        })
    }

    /// 主干 `SubmitInputAsync`：先清输入框，再走 `session/prompt`（无会话时先 `session/create`）。
    /// 用户气泡要等 wire ack 才落正文 —— 主干也是 await 之后才 AppendBubble。
    /// `force_mode` = 主干那个 `forceMode`（`Ctrl+Enter` 的反向档）；`None` 就是发送钮那条路。
    fn submit_input(&mut self, context: &ComponentContext<Shell>, force_mode: Option<&'static str>) {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return;
        }
        // 主干 `SubmitInputAsync` 开头那六行（MW:6883-6897）：前导 `/` 的命令名**命中会话命令目录**
        // ⇒ 走 `commands/execute`（整行原文当 `line`，含参数），不发消息、也不清框（清框由命令
        // 成功后那一发做）；不命中就照旧 `session/prompt`。目录这会儿还没取到 ⇒ 先取、把这发挂着。
        if leading_command_name(&text).is_some() && !self.catalog_ready() {
            self.pending_send = Some(force_mode);
            self.fetch_commands(context);
            return;
        }
        if let Some(name) = leading_command_name(&text) {
            if self
                .palette
                .entries
                .iter()
                .any(|entry| eq_ignore_case(&entry.name, name))
            {
                self.palette.hide();
                self.execute_command(text, true, context);
                return;
            }
        }
        let Some(shared) = self.kernel.clone() else {
            // 主线 `SendAsync`（MW:7007-7019）：`_rpc is null` = 内核还在后台引导 ⇒ 一次性提示，
            // 然后**直接 return**：消息不发、输入框里的原文一个字都不清（别让用户以为发出去了，
            // 也不弹错误级对话框）。所以清框挪到了这条守卫之后。
            self.kernel_wait_hint();
            return;
        };
        self.input.clear();
        self.echo = Some(text.clone());
        let session = self.active.clone().unwrap_or_default();
        let mux = self.mux.clone();
        // 分叉目前**恒**按 queue 发（主干那条「繁忙时读 busyEnter 设置」的分支整条链都还没移植，
        // 发送钮走的也是这里）。`Ctrl+Enter` 只多一个反向档：见 `ctrl_enter_mode`。
        let mode = force_mode.unwrap_or("queue").to_string();
        // 主干每次发送前都补一发 session/selectModel；分叉还没有模型选择器，拿不出可信的
        // provider/model，所以整步跳过，而不是瞎报一个把真内核的默认模型改掉。
        context.spawn_background(move |_token| {
            let mut kernel = match shared.lock() {
                Ok(guard) => guard,
                Err(_) => return Msg::Prompted(Err("内核状态不可用".to_string())),
            };
            let sid = if session.is_empty() {
                match kernel.create_session("") {
                    Ok(id) => id,
                    Err(error) => return Msg::Prompted(Err(error)),
                }
            } else {
                session
            };
            // 先把这一路的 follow 挂上再发 prompt：增量帧只推给当时在场的订阅者，
            // 晚开流就得靠 snapshot 回填，而回填不是所有内核都给。
            let stream = match &mux {
                Some(shared_mux) => shared_mux
                    .lock()
                    .ok()
                    .and_then(|mut guard| guard.open("session/follow", follow_args(&sid)).ok())
                    .unwrap_or_default(),
                None => String::new(),
            };
            let args = json!({ "request": {
                "requestId": request_id(),
                "sessionId": sid,
                "mode": mode,
                "content": [{ "type": "text", "text": text }],
            }});
            match kernel.call("session/prompt", args) {
                Ok(_) => Msg::Prompted(Ok((sid, stream))),
                Err(error) => Msg::Prompted(Err(error)),
            }
        });
    }

    /// 主干 `OnInputTextChanged` → `UpdateCommandPalette`（MW:17386-17392 + 17684-17694）：
    /// 判据不过就收浮层；过了就把过滤词记下、代次 +1（= 主干那句 `++_paletteGeneration`），
    /// 然后要目录。每次按键都可能把在途的那一发判成过期。
    fn refresh_palette(&mut self, context: &ComponentContext<Shell>) {
        let Some(query) = palette_trigger(&self.input) else {
            self.palette.hide();
            return;
        };
        self.palette.query = query.to_string();
        self.palette.generation += 1;
        self.ensure_commands(self.palette.generation, context);
    }

    /// 目录此刻能不能直接拿来判「命不命中」——主干 `SubmitInputAsync` 里那条 await 的等价物。
    /// `false` = 还得先打一发 `commands/list`。没内核 / 没会话算「能」：主干那两处早退之后
    /// `_commands` 就是空的，判定自然不命中 ⇒ 落回 `session/prompt`。
    fn catalog_ready(&self) -> bool {
        let Some(session) = self.active.as_deref() else {
            return true;
        };
        self.kernel.is_none() || !self.palette.needs_fetch(session, Instant::now())
    }

    /// 主干 `EnsureCommandsAsync`（MW:17309-17358）+ `ShowCommandPaletteAsync` 的那次 await：
    /// 缓存新鲜就直接刷浮层，否则打 RPC、由 [`Msg::CommandsLoaded`] 回来再刷。
    /// 拿不到内核 / 没有会话时主干是「什么都不做地 return」，而 return 之后 `ShowCommandPaletteAsync`
    /// 照样往下走 ⇒ 这里也照样刷一次（拿内存里那份可能过期的目录过滤）。
    fn ensure_commands(&mut self, generation: u64, context: &ComponentContext<Shell>) {
        let ready = match self.active.as_deref() {
            Some(session) if self.kernel.is_some() => {
                !self.palette.needs_fetch(session, Instant::now())
            }
            None => true,
            _ => false,
        };
        if ready {
            self.apply_palette(generation);
            return;
        }
        self.fetch_commands(context);
    }

    /// 单发 `commands/list`：params 就一个平铺 `agentId` = 活动会话 id（主干 17326），
    /// 走的是 [`Kernel::list_commands`]/`parse_commands` 这条既有通路，不另开第二条 RPC。
    fn fetch_commands(&mut self, context: &ComponentContext<Shell>) {
        let (Some(shared), Some(session)) = (self.kernel.clone(), self.active.clone()) else {
            return;
        };
        let generation = self.palette.generation;
        context.spawn_background(move |_| {
            let outcome = shared
                .lock()
                .map_err(|_| "内核状态不可用".to_string())
                .and_then(|mut kernel| kernel.list_commands(&session));
            Msg::CommandsLoaded {
                generation,
                session,
                outcome,
            }
        });
    }

    /// 把内存里那份目录摊成浮层内容 = 主干 `ShowCommandPaletteAsync`（MW:17696-17741）里
    /// await 之后的那一段（判定全在 [`PaletteState::apply`]，那里能不起 GUI 直接测）。
    fn apply_palette(&mut self, generation: u64) {
        let input = self.input.clone();
        self.palette.apply(generation, &input);
    }

    /// 主干 `MoveCommandSelection`（MW:17791-17801）。`ScrollIntoView` 那一发抄不过来：
    /// reactor 的 `ScrollViewer` 没有 `ElementRef` 也没有任何滚动入口（`generated.rs:2681-2711`
    /// 只有两根 scrollbar visibility 属性）⇒ 越界的选中行只能等用户自己滚。
    fn move_palette(&mut self, delta: i32) {
        self.palette.move_selection(delta);
    }

    /// 主干 `AcceptCommandSelection` + `OnCommandItemClick`（MW:17802-17836）：
    /// 带参数的只补全命令名（不执行），无参的清空输入框并**当场** `commands/execute`。
    /// 主干采纳后还把焦点还给 InputBox（`InputBox.Focus(Programmatic)`）——分叉的焦点本来就在
    /// 钩子认定的 composer 上，这里显式补一次同样的判定，鼠标路径才不会把 Enter 让给别人。
    fn accept_palette(&mut self, context: &ComponentContext<Shell>) {
        self.key_owner = KeyOwner::Composer;
        match self.palette.take_accept() {
            PaletteAccept::Fill(text) => self.input = text,
            PaletteAccept::Run(line) => {
                self.input.clear();
                self.execute_command(line, false, context);
            }
            PaletteAccept::Nothing => {}
        }
    }

    /// 主干 `ExecuteCommandAsync`（MW:17849-17945）。`submittedAttachments` 恒空数组：
    /// 分叉没移植附件（spec6 line 78 明确允许），参数形状仍是平铺三键。
    /// `from_composer` = 主干那个参数：只有 composer 回车那一发成功后才清草稿。
    fn execute_command(&mut self, line: String, from_composer: bool, context: &ComponentContext<Shell>) {
        // 主干 17851：`if (_commandSubmitting || _rpc is null) return;`——在途就不叠加。
        if self.command_submitting || self.kernel.is_none() {
            return;
        }
        let Some(shared) = self.kernel.clone() else {
            return;
        };
        let Some(session) = self.active.clone() else {
            // 主干 17854-17857：没有会话就摊一句系统消息，命令不发。
            self.append_system_message(
                self.catalog
                    .l("请先选择一个会话：命令在会话（agent）作用域内执行。"),
            );
            return;
        };
        self.command_submitting = true;
        // 主干 17860：`submittedText` 只在 composer 那一发带得回来。
        let draft = from_composer.then(|| self.input.clone());
        // 主干 17861：命令回显一行 `> /compact now`，走 system 小字行。
        self.append_system_message(format!("> {line}"));
        context.spawn_background(move |_| {
            let command_session = session.clone();
            let outcome = shared
                .lock()
                .map_err(|_| "内核状态不可用".to_string())
                .and_then(|mut kernel| kernel.execute_command(&session, &line, &[]));
            Msg::CommandExecuted {
                line,
                session: command_session,
                outcome,
                draft,
            }
        });
    }

    /// 主干 17889-17914 的四态落文案（`ok:false` 与传输异常都归「命令失败：…」，
    /// 因为分叉的 `Kernel::call` 把信封错误与线错误都并成 `Err`，与主干 `catch` 那支同口径）。
    fn report_command(&mut self, line: &str, outcome: Result<CommandReply, String>) {
        let text = match outcome {
            Err(failure) => self.catalog.lf("命令失败：{0}", &[failure]),
            Ok(reply) => match reply {
                CommandReply::Unknown => {
                    self.catalog.lf("未知或格式不正确的命令：{0}", &[line.to_string()])
                }
                CommandReply::NoImmediateReply => {
                    self.catalog.lf("{0} 已提交（内核未返回即时应答）", &[line.to_string()])
                }
                CommandReply::Result { kind, text } => match kind.as_str() {
                    // 主干 17911：`string.IsNullOrEmpty(text)` 才用兜底文案（空串也算没给）。
                    "success" => match text.filter(|text| !text.is_empty()) {
                        Some(body) => body,
                        None => self.catalog.lf("{0} 执行完成", &[line.to_string()]),
                    },
                    // 主干 17910：失败原文前面加 ⚠，没有原文（含空串）才用兜底文案。
                    "error" => match text.filter(|text| !text.is_empty()) {
                        Some(body) => format!("⚠ {body}"),
                        None => self.catalog.lf("{0} 执行失败", &[line.to_string()]),
                    },
                    other => format!("{line} → {other}"),
                },
            },
        };
        self.append_system_message(text);
    }

    /// 主线 MW:7013-7016 的一次性等待提示。`_kernelWaitHintShown` 的复位点**只有**
    /// `ShowKernelBootPanel` 入口第一行（KC:56），即每轮引导（首轮 / 按重试）各一次机会；
    /// 内核就绪撤卡时不复位，切会话也不复位。
    fn kernel_wait_hint(&mut self) {
        if self.boot.hint_shown {
            return;
        }
        self.boot.hint_shown = true;
        self.append_system_message(self.catalog.l("内核还在加载中，请稍候再发。"));
    }

    /// 主线 `AppendSystemMessage`（MW:17945-17958）：`Role = "system"` 的小字行。
    /// 模板选择器里 system 落到默认的 `ToolTpl`（`BubbleTemplateSelector.cs:32`）⇒ 分叉同一张
    /// 无图标灰行（`tool_line` 的 `icon == None` 那支）。turn 用 0：它不属于任何一轮。
    fn append_system_message(&mut self, text: String) {
        let seq = self.unique_seq("system", 0, 0);
        self.bubbles.push(Bubble::new("system", text, 0, seq));
    }

    /// 主线 `IsHeroState()`（MW:4367-4374）：没有活动会话，或会话里除了壳本地 system 行
    /// 还没别的对话内容。**壳本地 system 行不算内容**（那两条 /permission 回显曾把空态打掉）。
    /// 调用处还要再叠一条 `&& !boot.showing`（`UpdateEmptyState` MW:4380）。
    fn hero_state(&self) -> bool {
        self.active.is_none() || !self.bubbles.iter().any(|bubble| bubble.role != "system")
    }

    /// 主干 `StopActiveRunAsync`：发 `session/cancel`，收尾仍靠流里的 turn/end。
    fn cancel_run(&self, context: &ComponentContext<Shell>) {
        let (Some(shared), Some(session)) = (self.kernel.clone(), self.active.clone()) else {
            return;
        };
        context.spawn_background(move |_token| {
            let outcome = shared
                .lock()
                .map_err(|_| "内核状态不可用".to_string())
                .and_then(|mut kernel| {
                    kernel.call(
                        "session/cancel",
                        json!({ "request": { "sessionId": session } }),
                    )
                });
            match outcome {
                Ok(_) => Msg::Note("已请求停止".to_string()),
                Err(error) => Msg::Note(format!("停止失败: {error}")),
            }
        });
    }

    /// 主干 `ForkSessionAtAsync`：`session/fork` 带 `atSeq` ⇒ **`RefreshSessionsAsync`（`:814`）**
    /// 刷新会话台账 ⇒ 再 `RenameForkedChildAsync`（`:837`，标题取**刷新后**台账里的源会话）
    /// ⇒ 主干最后 `child.Title = increased` + `RebuildNavMenu`（就地改那一行）。
    /// 顺序错了就等于「rename 一条列表里还不存在的行」：改名白做、左栏见不到子会话。
    fn fork_at(&self, at_seq: i64, context: &ComponentContext<Shell>) {
        let (Some(shared), Some(session)) = (self.kernel.clone(), self.active.clone()) else {
            return;
        };
        // 壳侧台账里源会话现在的标题：刷新后的清单没这条时拿它兜底（见下面 `source_title`）。
        let shell_title = self
            .rows
            .iter()
            .find(|row| row.id == session)
            .map(display_title);
        context.spawn_background(move |_token| {
            let call = |method: &str, args: Value| {
                shared
                    .lock()
                    .map_err(|_| "内核状态不可用".to_string())
                    .and_then(|mut kernel| kernel.call(method, args))
            };
            let outcome = call(
                "session/fork",
                json!({ "request": { "sessionId": session, "atSeq": at_seq } }),
            )
            .map(|value| value["sessionId"].as_str().unwrap_or_default().to_string());
            let child = match outcome {
                Ok(child) if !child.is_empty() => child,
                Ok(_) => {
                    return Msg::Branched(Err("session/fork 没回 sessionId".to_string()));
                }
                Err(error) => return Msg::Branched(Err(error)),
            };
            // 主干 `RefreshSessionsAsync`：子会话先回台账，后面那步改名才找得到行。
            let mut rows = match shared
                .lock()
                .map_err(|_| "内核状态不可用".to_string())
                .and_then(|mut kernel| kernel.list_sessions())
            {
                Ok(rows) => rows,
                Err(error) => return Msg::Branched(Err(error)),
            };
            // 改名失败不打断分支：标题留内核继承值，与主干一样静默。
            // 标题口径取**刷新后**台账里的源会话（主干 `RenameForkedChildAsync` 读 `_sessions`）；
            // 内核清单还没跟上这条时退回壳侧台账那一条 —— 主干 `CreateSessionAsync`
            // （`MainWindow.xaml.cs` 的「刷新后列表里可能还没有这条（follow 流延迟）：本地合成兜底」）
            // 就是靠那条合成行让新建的会话在侧栏站得住，分叉的对应实现见 `ensure_local_session_row`。
            let source_title = rows
                .iter()
                .find(|row| row.id == session)
                .map(display_title)
                .or_else(|| shell_title.clone())
                .unwrap_or_default();
            // 主干 `RenameForkedChildAsync` 的两个前置：源会话与子会话**都**得在台账里
            // （`source is null || child is null || source.Title.Length == 0` 即整步跳过），
            // 给一条列表里不存在的行改名纯属白做。
            if source_title.is_empty() || !rows.iter().any(|row| row.id == child) {
                return Msg::Branched(Ok((rows, child, String::new())));
            }
            let title = increased_title(&source_title);
            let renamed = call(
                "session/rename",
                json!({ "request": { "sessionId": child, "title": title.clone() } }),
            );
            // 主干 `child.Title = increased`：成功才就地覆盖这一行，失败保持继承值。
            if renamed.is_err() {
                return Msg::Branched(Ok((rows, child, String::new())));
            }
            if let Some(row) = rows.iter_mut().find(|row| row.id == child) {
                row.title = title.clone();
            }
            Msg::Branched(Ok((rows, child, title)))
        });
    }

    /// 主干 `OpenPresentedFileAsync`：交付物只走宿主路由
    /// `POST /api/present.open?sessionId=&seq=&index=&action=open|reveal`，
    /// 200/204 即系统已接手（静默），其余状态码回一句人话。
    fn present_open(
        &self,
        seq: i64,
        index: usize,
        action: &str,
        context: &ComponentContext<Shell>,
    ) {
        let (Some(shared), Some(session)) = (self.kernel.clone(), self.active.clone()) else {
            return;
        };
        let path = format!(
            "/api/present.open?sessionId={}&seq={seq}&index={index}&action={action}",
            escape_query(&session)
        );
        context.spawn_background(move |_token| {
            let outcome = shared
                .lock()
                .map_err(|_| "内核状态不可用".to_string())
                .and_then(|mut kernel| kernel.post_route(&path));
            Msg::Presented(outcome)
        });
    }

    /// 主干 `OpenProducedPathAsync`：工作区路径交给宿主桌面（内核 `session/openWorkspacePath`，
    /// 与官方 `produced.open` 同一能力）。RPC 本身不校验路径存在性，失败一律走行内报错。
    fn open_path(&self, path: String, context: &ComponentContext<Shell>) {
        let Some(shared) = self.kernel.clone() else {
            return;
        };
        context.spawn_background(move |_token| {
            let outcome = shared
                .lock()
                .map_err(|_| "内核状态不可用".to_string())
                .and_then(|mut kernel| {
                    kernel
                        .call(
                            "session/openWorkspacePath",
                            json!({ "request": { "path": path } }),
                        )
                        .map(|_| ())
                });
            Msg::Opened(outcome)
        });
    }

    /// 主干 `ResetFeedbackState`（`MainWindow.xaml.cs:16403`）：反馈条目整族都是会话作用域，
    /// 换会话/断开必清。主干另有 `_feedbackRows` / `_rowsByBubble` 两张「气泡→行控件」表用来
    /// 就地重绘，分叉是声明式渲染，行由 `bubble_actions` 现算，没有对应物。
    fn reset_feedback(&mut self) {
        self.feedback.clear();
        self.feedback_busy.clear();
        self.feedback_status.clear();
        self.note_for = None;
        self.note_text.clear();
    }

    /// 主干 `LoadFeedbackAsync`（16754）：`messageFeedback/list {sessionId}` → `value.items[]`，
    /// 按 `item.messageId` 建表。触发点只有切会话流程末尾一次（不轮询）；业务 ok:false 与传输
    /// 异常主干全部吞掉，分叉同样只把消息发回来、由 `update` 丢弃 —— 拉不到反馈不许挡聊天。
    fn load_feedback(&self, session: &str, context: &ComponentContext<Shell>) {
        let Some(shared) = self.kernel.clone() else {
            return;
        };
        let session = session.to_string();
        context.spawn_background(move |_| {
            let outcome = feedback_call(
                &shared,
                "messageFeedback/list",
                json!({ "request": { "sessionId": session } }),
            )
            .map(|value| {
                let items = value["items"].as_array().cloned().unwrap_or_default();
                (session.clone(), items)
            })
            .map_err(|failure| failure.code_text());
            Msg::FeedbackList(outcome)
        });
    }

    /// 主干 `RateAsync`（16833）：同一评级再点一次 = 撤销（走 delete）；改判**不 delete**，
    /// 直接 put 新评级 —— 但要把旧 note 带上，因为 put 是整体替换，不带 `note` 键就把它丢了。
    /// 主干 `MakeFeedbackToggle` 那颗本来就是原生 `ToggleButton`：点一下控件自己翻
    /// `IsChecked`，再由 `IsCheckedChanged` 把**翻完之后**的态报上来（分叉用
    /// `windows_reactor::ToggleButton`，`generated.rs:2297`，`is_checked` 双向绑到原生
    /// `IsCheckedProperty`）。所以这里收的是「目标态」而不是「点击」，必须按当前反馈态再判一次：
    /// · `checked=true` 且当前不是这个评级 ⇒ put（换评级连带旧 note，口径同 `rate_feedback`）。
    /// · `checked=false` 且当前正是这个评级 ⇒ delete（= 主干「再点一次撤销」）。
    /// · 其余不动 —— 挡的是两种**回声**：内核回数据后重绘把 `is_checked` 回写原生、原生可能
    ///   再报一次 `IsCheckedChanged`；没这道守卫就会重复发 RPC。
    fn set_feedback_rating(
        &mut self,
        message: String,
        rating: String,
        checked: bool,
        context: &ComponentContext<Shell>,
    ) {
        let current = feedback_of(&self.feedback, &message).map(|item| item.rating);
        if checked {
            if current.as_deref() != Some(rating.as_str()) {
                self.rate_feedback(message, rating, context);
            }
        } else if current.as_deref() == Some(rating.as_str()) {
            self.delete_feedback(message, context);
        }
    }

    /// 主干 `RateAsync`：同评级再点 = 撤销，改判 = put 并把旧 note 带上。
    fn rate_feedback(
        &mut self,
        message: String,
        rating: String,
        context: &ComponentContext<Shell>,
    ) {
        let current = feedback_of(&self.feedback, &message);
        if current.as_ref().is_some_and(|item| item.rating == rating) {
            self.delete_feedback(message, context);
            return;
        }
        let note = current.and_then(|item| item.note);
        self.put_feedback(message, rating, note, context);
    }

    /// 主干 `PutFeedbackAsync`（16854）：`{sessionId, messageId, rating, ifVersion, [note], [category]}`。
    /// `note`/`category` 为空必须**整个键都不带**（内核 zod 是 `.optional()` 不是 nullable，给
    /// null 直接 input-invalid）；`ifVersion` 允许 null。version-conflict 且第一次尝试 ⇒
    /// `AdoptFeedbackConflict` 采纳 `error.current`（null 即本地已撤销）后用新版本重试一次。
    fn put_feedback(
        &mut self,
        message: String,
        rating: String,
        note: Option<String>,
        context: &ComponentContext<Shell>,
    ) {
        let (Some(shared), Some(session)) = (self.kernel.clone(), self.active.clone()) else {
            return;
        };
        if self.feedback_busy.iter().any(|busy| busy == &message) {
            return; // 主干 `!_feedbackBusy.Add(messageId)`：在途就静默 return，按钮不禁用
        }
        self.feedback_busy.push(message.clone());
        let known = feedback_of(&self.feedback, &message);
        let catalog = self.catalog.clone();
        context.spawn_background(move |_| {
            let mut patch = FeedbackPatch {
                session: session.clone(),
                message: message.clone(),
                aligned: None,
                adopted: None,
                revoked: false,
                status: FeedbackStatus::Keep,
            };
            // 主干每轮都重读 `_feedback[messageId]`；后台只带得动这一条，够 put 用。
            let mut current = known;
            for attempt in 0..2 {
                let version = current.as_ref().and_then(|item| item.version.clone());
                let category = current.as_ref().and_then(|item| item.category.clone());
                let mut request = Map::new();
                request.insert("sessionId".into(), Value::String(session.clone()));
                request.insert("messageId".into(), Value::String(message.clone()));
                request.insert("rating".into(), Value::String(rating.clone()));
                request.insert(
                    "ifVersion".into(),
                    version.map(Value::String).unwrap_or(Value::Null),
                );
                if let Some(text) = note.as_ref().filter(|text| !text.is_empty()) {
                    request.insert("note".into(), Value::String(text.clone()));
                }
                if let Some(category) = category.filter(|category| !category.is_empty()) {
                    request.insert("category".into(), Value::String(category));
                }
                match feedback_call(
                    &shared,
                    "messageFeedback/put",
                    json!({ "request": request }),
                ) {
                    // 成功：用内核回的整条 item 覆写本地表（主干 StoreFeedbackItem + RenderAll）
                    Ok(item) => {
                        patch.adopted = Some(item);
                        patch.revoked = false;
                        patch.status = FeedbackStatus::Clear;
                        return Msg::FeedbackApplied(patch);
                    }
                    Err(FeedbackFailure::Business(error)) => {
                        if error["code"].as_str() != Some("version-conflict") || attempt > 0 {
                            patch.status =
                                FeedbackStatus::Text(feedback_error_text(&catalog, &error));
                            return Msg::FeedbackApplied(patch);
                        }
                        let adopted = error
                            .get("current")
                            .filter(|current| current.is_object())
                            .cloned();
                        patch.revoked = adopted.is_none();
                        current = adopted
                            .as_ref()
                            .and_then(parse_feedback)
                            .map(|(_, item)| item);
                        patch.adopted = adopted;
                    }
                    Err(failure) => {
                        patch.status =
                            FeedbackStatus::Text(feedback_failure_message(&catalog, &failure));
                        return Msg::FeedbackApplied(patch);
                    }
                }
            }
            // 兜底：循环里两条出口都覆盖不到「二次仍冲突」以外的情况，主干这句是原话。
            patch.status = FeedbackStatus::Text(catalog.l("反馈已被其他地方改动，请重试"));
            Msg::FeedbackApplied(patch)
        });
    }

    /// 主干 `DeleteFeedbackAsync`（16917）：`{sessionId, messageId, ifVersion}`。
    /// 契约要点：delete 的 `ifVersion` 是 **required string**（put 才接受 null），缺了会被网关
    /// 拒成 gateway/input-invalid ⇒ 本地没观察到版本时先 list 对齐；内核侧也没有就直接本地撤销
    /// （幂等，且主干这条路径不碰 Status 文案，所以 status 留在 `Keep`）。
    fn delete_feedback(&mut self, message: String, context: &ComponentContext<Shell>) {
        let (Some(shared), Some(session)) = (self.kernel.clone(), self.active.clone()) else {
            return;
        };
        if self.feedback_busy.iter().any(|busy| busy == &message) {
            return;
        }
        self.feedback_busy.push(message.clone());
        let known = feedback_of(&self.feedback, &message);
        let catalog = self.catalog.clone();
        context.spawn_background(move |_| {
            let mut patch = FeedbackPatch {
                session: session.clone(),
                message: message.clone(),
                aligned: None,
                adopted: None,
                revoked: false,
                status: FeedbackStatus::Keep,
            };
            let mut current = known;
            let missing = current.as_ref().map_or(true, |item| item.version.is_none());
            if missing {
                if let Ok(value) = feedback_call(
                    &shared,
                    "messageFeedback/list",
                    json!({ "request": { "sessionId": session.clone() } }),
                ) {
                    let items = value["items"].as_array().cloned().unwrap_or_default();
                    current = items
                        .iter()
                        .find(|item| item["messageId"].as_str() == Some(message.as_str()))
                        .and_then(parse_feedback)
                        .map(|(_, item)| item);
                    patch.aligned = Some(items);
                }
                if current.as_ref().map_or(true, |item| item.version.is_none()) {
                    patch.revoked = true;
                    return Msg::FeedbackApplied(patch);
                }
            }
            for attempt in 0..2 {
                let Some(version) = current.as_ref().and_then(|item| item.version.clone()) else {
                    break;
                };
                let args = json!({ "request": {
                    "sessionId": session,
                    "messageId": message,
                    "ifVersion": version,
                }});
                match feedback_call(&shared, "messageFeedback/delete", args) {
                    Ok(_) => {
                        patch.adopted = None;
                        patch.revoked = true;
                        patch.status = FeedbackStatus::Clear;
                        return Msg::FeedbackApplied(patch);
                    }
                    Err(FeedbackFailure::Business(error)) => {
                        if error["code"].as_str() != Some("version-conflict") || attempt > 0 {
                            patch.status =
                                FeedbackStatus::Text(feedback_error_text(&catalog, &error));
                            return Msg::FeedbackApplied(patch);
                        }
                        let adopted = error
                            .get("current")
                            .filter(|current| current.is_object())
                            .cloned();
                        patch.revoked = adopted.is_none();
                        current = adopted
                            .as_ref()
                            .and_then(parse_feedback)
                            .map(|(_, item)| item);
                        patch.adopted = adopted;
                        if current.as_ref().map_or(true, |item| item.version.is_none()) {
                            return Msg::FeedbackApplied(patch); // 已被撤销：本地对齐就够，不报错
                        }
                    }
                    Err(failure) => {
                        patch.status =
                            FeedbackStatus::Text(feedback_failure_message(&catalog, &failure));
                        return Msg::FeedbackApplied(patch);
                    }
                }
            }
            patch.status = FeedbackStatus::Text(catalog.l("反馈已被其他地方改动，请重试"));
            Msg::FeedbackApplied(patch)
        });
    }

    /// 主干「写 `_feedback` + `RenderAllFeedbackRows()` + `SetFeedbackStatus`」那三段副作用。
    /// 会话不符就整份丢弃：主干是 UI 线程串行 await，切完会话不会再回灌；分叉的后台消息没有
    /// 顺序保证，不挡就会把上个会话的条目灌进新会话的表里。
    fn apply_feedback_patch(&mut self, patch: FeedbackPatch) {
        self.feedback_busy.retain(|busy| busy != &patch.message);
        if self.active.as_deref() != Some(patch.session.as_str()) {
            return;
        }
        if let Some(items) = &patch.aligned {
            self.feedback.clear();
            for item in items {
                store_feedback(&mut self.feedback, item);
            }
        }
        if patch.revoked {
            self.feedback.retain(|(id, _)| *id != patch.message);
        }
        if let Some(item) = &patch.adopted {
            store_feedback(&mut self.feedback, item);
        }
        match patch.status {
            FeedbackStatus::Keep => {}
            FeedbackStatus::Clear => self.feedback_status.retain(|(id, _)| *id != patch.message),
            FeedbackStatus::Text(text) => set_pair(&mut self.feedback_status, patch.message, text),
        }
    }

    /// 一个会话只开一条 `session/follow`。开流必须挪到后台：`collect` 会占着 mux 锁最长 1s。
    fn ensure_follow(&self, session: &str, context: &ComponentContext<Shell>) {
        if self.follows.iter().any(|(id, _)| id == session) {
            return;
        }
        let Some(mux) = self.mux.clone() else {
            return;
        };
        let session = session.to_string();
        context.spawn_background(move |_token| {
            let opened = mux
                .lock()
                .map_err(|_| "mux 状态不可用".to_string())
                .and_then(|mut guard| guard.open("session/follow", follow_args(&session)));
            match opened {
                Ok(stream) => Msg::FollowOpened(Ok((session, stream))),
                Err(error) => Msg::FollowOpened(Err(error)),
            }
        });
    }

    /// wire ack 回来：把等到的那句落进正文，并认下这一路 follow 流（开流失败的话补开一次）。
    fn on_prompted(&mut self, session: &str, stream: &str, context: &ComponentContext<Shell>) {
        let Some(text) = self.echo.take() else {
            return;
        };
        // 发送换到别的会话（空态直接发 = 内核新建一条）等同于一次「进会话」：
        // 反馈表是会话作用域，换了就得先复位再拉一次；同会话重发不碰，免得打断在途条目。
        let switched = self.active.as_deref() != Some(session);
        self.active = Some(session.to_string());
        self.page = Page::Chat;
        if switched {
            self.reset_feedback();
            self.load_feedback(session, context);
        }
        // 发送路径自己 `session/create` 出来的会话可能还不在清单里（真内核 follow 流延迟 /
        // 桩不登记）：先补一行台账，侧栏这一行与后面「分支→改名」的基准标题都靠它。
        self.ensure_local_session_row(session);
        self.bubbles.push(Bubble::new("user", text, 0, 0));
        self.trim_bubbles();
        self.thinking = true;
        if stream.is_empty() {
            self.ensure_follow(session, context);
        } else if !self.follows.iter().any(|(id, _)| id == session) {
            self.follows.push((session.to_string(), stream.to_string()));
        }
    }

    fn trim_bubbles(&mut self) {
        while self.bubbles.len() > 400 {
            self.bubbles.remove(0);
        }
    }

    /// 主干 `CreateSessionAsync` 的「本地合成兜底」（`MainWindow.xaml.cs`：刷新后列表里可能还没有
    /// 这条（follow 流延迟），本地合成一条 `Title="新会话"` + `Blank=true` 保证能选中并打开）。
    /// 分叉这边同一条台账就是左栏那一行：`session/create` 回来的 id 不在 `session/list` 里时先补
    /// 一行占位，别让它成了「在聊天但侧栏找不到自己」。
    /// 「分支→改名」也靠它：源会话在台账里查不到就取不到基准标题，`session/rename` 只能整步跳过，
    /// 子会话那行永远长不出 `(1)`（假内核的 `session/create` 桩正好是这条固定 `s-2001` 不登记的形状）。
    fn ensure_local_session_row(&mut self, id: &str) {
        if id.is_empty() || self.rows.iter().any(|row| row.id == id) {
            return;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|span| span.as_millis() as i64)
            .unwrap_or(0);
        self.rows.push(SessionInfo {
            id: id.to_string(),
            cwd: String::new(),
            title: String::new(),
            blank: true,
            updated_at: now,
            parent: None,
            origin: None,
        });
    }

    /// 合成行/写死 seq 的行的键唯一性兜底：这类行的 seq **不是 journal 坐标**（主干那边它既不带
    /// 时钟也不给分支钮），取值自由，但分叉的 `keyed_children` 要求 `role-turn-seq` 整列唯一
    /// —— 主干 ListView 按对象身份寻址、压根没这条约束，所以主干 `AppendBubble` 的「相邻去重」
    /// 搬到分叉会留下**同轮非相邻**的两条合成行（`tool-{turn}-0` 撞两次 ⇒ DuplicateKey 崩）。
    /// `wanted` 空着就照原样用（真实信封 seq 不动），已被占用才退到**负数区**：真实 seq 恒 ≥ 0
    /// ⇒ 合成行与 `tool/call` 那种取自信封的行天然互不撞；而 `seq > 0` 的三处门槛（分支钮、
    /// 轮尾 chips 截断、`is_turn_answer`）也照旧一律不触发，正合主干「这类行不带时钟不给分支钮」。
    /// 只在 push 时算一次并写进 `Bubble.seq` ⇒ 重渲染之间键稳定（hover 提亮、「已复制」、
    /// 思考折叠都按 `bubble_key` 记状态，渲染函数里现算长度会让状态错位）。
    fn unique_seq(&self, role: &str, turn: i64, wanted: i64) -> i64 {
        let taken = |candidate: i64| {
            self.bubbles
                .iter()
                .any(|bubble| bubble.role == role && bubble.turn == turn && bubble.seq == candidate)
        };
        if !taken(wanted) {
            return wanted;
        }
        let mut seq = -1;
        while taken(seq) {
            seq -= 1;
        }
        seq
    }

    /// 主干 `ApplyFollowFrameAsync` 的分叉版：只处理正文渲染得到的那几型帧。
    fn apply_follow_frame(&mut self, session: &str, frame: &Value) {
        match frame["type"].as_str().unwrap_or_default() {
            // snapshot.records 每条是 {"type":"event","event":{…}}，按同一套事件路径重放。
            "snapshot" => {
                if let Some(records) = frame["records"].as_array() {
                    for record in records {
                        self.apply_follow_frame(session, record);
                    }
                }
            }
            "event" => self.apply_journal_event(session, &frame["event"]),
            "assistant-stream" => self.apply_stream_frame(session, &frame["frame"]),
            _ => {}
        }
    }

    fn apply_journal_event(&mut self, session: &str, event: &Value) {
        let seq = event["seq"].as_i64().unwrap_or(0);
        let time = event["time"].as_i64().unwrap_or(0);
        let turn = event["data"]["turn"].as_i64().unwrap_or(0);
        match event["type"].as_str().unwrap_or_default() {
            "turn/start" => {
                self.set_running(session, true);
                self.thinking = true;
                if time > 0 && !self.turn_starts.iter().any(|(id, _)| *id == turn) {
                    self.turn_starts.push((turn, time));
                }
                // 主干文件改动累加器随轮重置：在途突变登记整表清空，本轮产出另起。
                if turn > 0 {
                    self.produced.retain(|(id, _)| *id != turn);
                }
                self.mutations.clear();
            }
            // 主干 `_lastHeaderModel`：本轮实际模型 id 由 request/header 报，之后落的都带上。
            // 已经挂出的那条用户气泡也在这里补认领（主干 `_pendingUserBubble` + RepaintUserActions）。
            "request/header" => {
                let model = event["data"]["header"]["config"]["model"]
                    .as_str()
                    .unwrap_or_default();
                if !model.is_empty() {
                    self.header_model = model.to_string();
                    if let Some(seq) = self.pending_user.take() {
                        if let Some(bubble) = self
                            .bubbles
                            .iter_mut()
                            .rev()
                            .find(|bubble| bubble.role == "user" && bubble.seq == seq)
                        {
                            bubble.model = model.to_string();
                        }
                    }
                }
            }
            "user/message" => {
                let text = content_text(event["data"]["content"].as_array());
                // 主干 FindLocalUserEcho：本地气泡（seq==0）认领这条，不重复画一句。
                match self
                    .bubbles
                    .iter_mut()
                    .rev()
                    .find(|bubble| bubble.role == "user" && bubble.seq == 0)
                {
                    Some(bubble) => {
                        bubble.seq = seq;
                        bubble.turn = turn;
                        bubble.text = text;
                        bubble.time = time;
                        bubble.model = self.header_model.clone();
                    }
                    None => {
                        let mut bubble = Bubble::new("user", text, turn, seq);
                        bubble.time = time;
                        bubble.model = self.header_model.clone();
                        self.bubbles.push(bubble);
                    }
                }
                self.pending_user = Some(seq);
            }
            "assistant/message" => {
                let blocks = event["data"]["message"]["content"].as_array();
                let text = content_text(blocks);
                self.live = None;
                self.thinking = false;
                let settled = self.bubbles.last().is_some_and(|last| last.seq == seq);
                if settled {
                    return;
                }
                // 主干把 content 里的 reasoning 块拆成独立一行（`reasoning` 模板 + 折叠开关）。
                let reasoning: String = blocks
                    .into_iter()
                    .flatten()
                    .filter(|block| block["type"].as_str() == Some("reasoning"))
                    .filter_map(|block| block["text"].as_str())
                    .collect();
                if !reasoning.is_empty() {
                    let mut bubble = Bubble::new("reasoning", reasoning, turn, seq);
                    bubble.time = time;
                    self.bubbles.push(bubble);
                }
                if !text.is_empty() {
                    let mut bubble = Bubble::new("assistant", text, turn, seq);
                    bubble.id = event["data"]["message"]["id"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string();
                    bubble.time = time;
                    bubble.model = self.header_model.clone();
                    bubble.tokens = usage_tokens(&event["data"]["usage"]);
                    bubble.produced = self.produced_paths(turn, seq);
                    self.bubbles.push(bubble);
                }
            }
            "tool/call" => {
                let name = event["data"]["name"].as_str().unwrap_or_default();
                let raw = &event["data"]["arguments"];
                // 内核把 arguments 发成 JSON 字符串，也允许直接是对象（主干同样两种都吃）。
                let args = match raw.as_str() {
                    Some(text) => serde_json::from_str::<Value>(text).unwrap_or(Value::Null),
                    None => raw.clone(),
                };
                let (code, title, summary) = tool_row(name, &args);
                // 「本轮文件改动」数据面：登记本调用的突变目标路径（非突变 = 空串）。
                let call_id = event["data"]["callId"].as_str().unwrap_or_default();
                if !call_id.is_empty() {
                    let path = mutation_path(name, &args).unwrap_or_default().to_string();
                    self.mutations.retain(|(id, _)| id != call_id);
                    self.mutations.push((call_id.to_string(), path));
                }
                // 信封 seq 缺席时上面按 0 处理 ⇒ 与同轮那条写死 0 的合成行撞键；这里过一道
                // `unique_seq`，正常数据照原样用信封 seq（它是 journal 坐标），只在真撞了时退负。
                let mut bubble = Bubble::new(
                    "tool",
                    title.to_string(),
                    turn,
                    self.unique_seq("tool", turn, seq),
                );
                bubble.time = time;
                bubble.icon = Some(code);
                bubble.summary = summary;
                self.bubbles.push(bubble);
            }
            // 主干 `tool/result`：结果非 error 且 callId 命中本轮突变登记，才按结果信封 seq 记一条产出。
            "tool/result" => {
                let message = &event["data"]["message"];
                let call_id = message["source"]["callId"].as_str().unwrap_or_default();
                // 官方只看 content[0] 的 isError。
                let is_error = message["content"]
                    .as_array()
                    .and_then(|blocks| blocks.first())
                    .is_some_and(|block| block["isError"].as_bool().unwrap_or(false));
                if is_error || call_id.is_empty() {
                    return;
                }
                let produced_path = self
                    .mutations
                    .iter()
                    .find(|(id, _)| id == call_id)
                    .map(|(_, path)| path.clone())
                    .unwrap_or_default();
                if produced_path.is_empty() {
                    return;
                }
                match self.produced.iter_mut().find(|(id, _)| *id == turn) {
                    Some((_, rows)) => rows.push((seq, produced_path)),
                    None => self.produced.push((turn, vec![(seq, produced_path)])),
                }
                self.sync_produced(turn);
            }
            // 主干 `system/message`：落成 ToolTpl 那种无图标灰行，同会话第一次报「系统提示词」，
            // 之后都是「系统提示词更新」。seq/time 一律不填 ⇒ 不带时钟、也不给分支钮。
            "system/message" => {
                let seen = self.system_prompt_seen.iter().any(|id| id == session);
                let text = self.catalog.l(if seen {
                    "系统提示词更新"
                } else {
                    "系统提示词"
                });
                if !seen {
                    self.system_prompt_seen.push(session.to_string());
                }
                let tail_is_same = self.bubbles.last().is_some_and(|last| {
                    last.role == "tool" && last.text == text && last.turn == turn
                });
                // 主干 `AppendBubble` 的相邻同 (role, text, turn) 去重照抄，但**唯一性另由
                // `unique_seq` 兜底**：同一 turn 里两条不相邻的合成行（假内核的「系统提示词」+
                // 「系统提示词更新」之间还夹着工具行）写死 seq=0 会撞成同一个 keyed 键 ⇒ 崩。
                if !tail_is_same {
                    let seq = self.unique_seq("tool", turn, 0);
                    self.bubbles.push(Bubble::new("tool", text, turn, seq));
                }
            }
            // 主干 `assistant/attempt`：流里 finish{reason.kind:"error"} 汇成一条 ⚠ 助手卡，
            // 多条失败只留最后一条；失败正文缺失时按「内核执行失败」兜底。
            "assistant/attempt" => {
                let mut failure: Option<(String, String)> = None;
                for piece in event["data"]["stream"].as_array().into_iter().flatten() {
                    let chunk = &piece["chunk"];
                    if chunk["type"].as_str() != Some("finish")
                        || chunk["reason"]["kind"].as_str() != Some("error")
                    {
                        continue;
                    }
                    let message = chunk["reason"]["failure"]["message"]
                        .as_str()
                        .filter(|text| !text.is_empty())
                        .unwrap_or("内核执行失败");
                    failure = Some((
                        chunk["reason"]["failure"]["code"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string(),
                        self.catalog.l(message),
                    ));
                }
                if let Some((code, message)) = failure {
                    // 这类气泡不占 journal seq（主干 `Seq` 留默认 0），行键只能靠 (role, turn, seq)
                    // ⇒ 同轮第二条失败同样得过 `unique_seq`，否则 keyed 键撞车。
                    let seq = self.unique_seq("assistant", turn, 0);
                    let mut bubble =
                        Bubble::new("assistant", format!("⚠ {code}: {message}"), turn, seq);
                    bubble.model = self.header_model.clone();
                    // 就地覆盖相邻的上一条失败卡（主干「多条 error 只留最后一条」的口径跨事件也成立）；
                    // `seq <= 0` 即「没认领过信封 seq 的合成 assistant 行」。
                    match self.bubbles.last_mut().filter(|last| {
                        last.role == "assistant" && last.seq <= 0 && last.turn == turn
                    }) {
                        Some(last) => *last = bubble,
                        None => self.bubbles.push(bubble),
                    }
                }
            }
            "deliverables/presented" => {
                let files: Vec<(usize, String, String)> = event["data"]["files"]
                    .as_array()
                    .map(|items| {
                        items
                            .iter()
                            .enumerate()
                            .filter_map(|(index, file)| {
                                file["path"].as_str().map(|path| {
                                    (
                                        index,
                                        path.to_string(),
                                        file["description"]
                                            .as_str()
                                            .unwrap_or_default()
                                            .to_string(),
                                    )
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if !files.is_empty() {
                    let joined = files
                        .iter()
                        .map(|(_, path, _)| path.as_str())
                        .collect::<Vec<_>>()
                        .join("、");
                    let mut bubble = Bubble::new("deliverable", joined, turn, seq);
                    bubble.time = time;
                    bubble.files = files;
                    self.bubbles.push(bubble);
                }
            }
            // 内核命名链（dsh-session-title）：首条消息先截断兜底，provider 随后追发正式名，
            // 侧栏就地改名（主干 ApplyTitleFromKernelAsync）。
            "session/title" => {
                if let Some(title) = event["data"]["title"]
                    .as_str()
                    .filter(|title| !title.is_empty())
                {
                    if let Some(row) = self.rows.iter_mut().find(|row| row.id == session) {
                        row.title = title.to_string();
                    }
                }
            }
            "turn/end" => {
                self.set_running(session, false);
                self.live = None;
                self.thinking = false;
                // 主干轮尾清掉没等到 header 的用户气泡：否则下一轮的 header 会标到上一轮的提问。
                self.pending_user = None;
                if let Some(index) = self.turn_starts.iter().position(|(id, _)| *id == turn) {
                    let start = self.turn_starts.remove(index).1;
                    if time > start {
                        if let Some(answer) = self
                            .bubbles
                            .iter_mut()
                            .rev()
                            .find(|bubble| bubble.turn == turn && bubble.role == "assistant")
                        {
                            answer.duration_ms = time - start;
                        }
                    }
                }
                // 主干 turn/end 后刷新产出：结果事件晚于答案落地的最后一批 chips 在这里补上。
                self.sync_produced(turn);
            }
            _ => {}
        }
        self.trim_bubbles();
    }

    /// 主干 `ApplyAssistantStreamFrame`：attemptId 空直接丢，只有 text-delta 进正文。
    fn apply_stream_frame(&mut self, session: &str, frame: &Value) {
        let attempt = frame["attemptId"].as_str().unwrap_or_default();
        if attempt.is_empty() {
            return;
        }
        match frame["type"].as_str().unwrap_or_default() {
            "start" => {
                self.live = Some(LiveAttempt {
                    session: session.to_string(),
                    attempt: attempt.to_string(),
                    turn: frame["turn"].as_i64().unwrap_or(0),
                    text: String::new(),
                });
            }
            "chunk" => {
                let chunk = &frame["chunk"];
                // 主干只认 text-delta / reasoning-delta；tool-call-delta、usage、finish 一律忽略。
                if chunk["type"].as_str() == Some("text-delta") {
                    let piece = chunk["text"].as_str().unwrap_or_default();
                    let turn = frame["turn"].as_i64().unwrap_or(0);
                    let mut live = self.live.take().unwrap_or(LiveAttempt {
                        session: session.to_string(),
                        attempt: attempt.to_string(),
                        turn,
                        text: String::new(),
                    });
                    // 换了 attemptId 就另起一条，不能把两次尝试的文本接在一起。
                    if live.attempt != attempt {
                        live = LiveAttempt {
                            session: session.to_string(),
                            attempt: attempt.to_string(),
                            turn,
                            text: String::new(),
                        };
                    }
                    live.text.push_str(piece);
                    self.live = Some(live);
                    self.thinking = false;
                }
            }
            "end" => {
                if frame["outcome"]["kind"].as_str() == Some("abandoned") {
                    self.live = None;
                }
                // committed 时先留着：最终文本由 assistant/message 落地，主干也是这个顺序。
            }
            _ => {}
        }
    }

    /// 主干 `ui-theme/preference`：选「跟随系统」时回落到窗口报上来的实际配色。
    fn apply_theme(&mut self) {
        self.scheme = match self.pick("ui-theme.preference", 2) {
            0 => Scheme::Light,
            1 => Scheme::Dark,
            _ => self.system_scheme,
        };
    }

    fn open_mux(&mut self, context: &ComponentContext<Shell>) {
        let Some(shared) = self.kernel.clone() else {
            return;
        };
        context.spawn_background(move |_token| {
            let address = shared
                .lock()
                .map(|kernel| {
                    (
                        kernel.endpoint().clone(),
                        kernel.cookie().map(str::to_string),
                    )
                })
                .map_err(|_| "内核状态不可用".to_string());
            let (endpoint, cookie) = match address {
                Ok(value) => value,
                Err(error) => return Msg::MuxReady(Err(error)),
            };
            let mux = match Mux::connect(&endpoint, cookie.as_deref()) {
                Ok(mux) => Arc::new(Mutex::new(mux)),
                Err(error) => return Msg::MuxReady(Err(error)),
            };
            let opened = mux
                .lock()
                .map_err(|_| "mux 状态不可用".to_string())
                .and_then(|mut guard| {
                    // 顺序照主干：先 follow 建表，再订 `$events`，反了会让 cwd 自动分组失效
                    // （MW:2614-2616 那条 0.7.1 时序注释）；第三发才是会话控制面，主干
                    // `OpenSessionControlStreamAsync` 也排在 RefreshWorkspaces / SubscribeEvents /
                    // RefreshSessions 之后（MW:2622）。三条流共用一根 socket，帧靠 streamId 分。
                    guard.open("workspace/follow", json!({}))?;
                    guard.open("$events", json!({}))?;
                    guard.open_session_control().map_err(|e| format!("控制面开流失败: {e}"))
                });
            match opened {
                Ok(control) => Msg::MuxReady(Ok((mux, control))),
                Err(error) => Msg::MuxReady(Err(error)),
            }
        });
    }

    fn poll_mux(&self, context: &ComponentContext<Shell>) {
        let Some(mux) = self.mux.clone() else {
            return;
        };
        context.spawn_background(move |_token| {
            let outcome = mux
                .lock()
                .map_err(|_| "mux 状态不可用".to_string())
                .and_then(|mut guard| guard.collect(std::time::Duration::from_secs(1)));
            match outcome {
                Ok(events) => Msg::MuxEvents(events),
                Err(error) => Msg::MuxFailed(error),
            }
        });
    }

    fn apply_mux_events(&mut self, events: Vec<MuxEvent>, context: &ComponentContext<Shell>) {
        let mut changed = false;
        let mut control = ControlTally::default();
        for event in events {
            match event {
                MuxEvent::Item {
                    value: Some(frame),
                    stream,
                } => {
                    // 一根 socket 上四条流（workspace/follow、$events、session/control、
                    // session/follow），先按 streamId 认流再按帧型兜底，判据见 `mux_frame_sink`。
                    let sink = mux_frame_sink(
                        &stream,
                        self.control_stream.as_deref(),
                        &self.follows,
                        &frame,
                    );
                    match sink {
                        MuxSink::Follow(session) => self.apply_follow_frame(&session, &frame),
                        MuxSink::Control => {
                            let kind = control_frame_type(&frame);
                            control.record(kind);
                            if kind.is_some() {
                                self.control.apply(&frame);
                            }
                        }
                        MuxSink::Status(id, running) => self.set_running(&id, running),
                        MuxSink::Workspace => changed |= self.tree.apply(&frame),
                    }
                }
                MuxEvent::Item { value: None, .. } => {}
                MuxEvent::Failure { stream, message } => {
                    if Some(&stream) == self.control_stream.as_ref() {
                        // 控制面这一路被判死：主干 `_controlStreamId = null`（MW:15206）同口径。
                        // 分叉的重开点在 `open_mux`（丢实例 = 丢流），所以这里只作废标记，
                        // 留着它下次重连的 streamId 会跟旧的对不上号。
                        self.control_stream = None;
                        self.push_log(format!("控制面流 {stream} 出错: {message}"));
                    } else {
                        self.push_log(format!("工作区流 {stream} 出错: {message}"));
                    }
                }
                MuxEvent::End { stream } | MuxEvent::Cancelled { stream } => {
                    if let Some(index) = self.follows.iter().position(|(_, id)| *id == stream) {
                        // 会话流结束只丢这一路的映射，别把整条 mux 拆掉重建。
                        self.follows.remove(index);
                        self.live = None;
                        self.push_log(format!("会话流 {stream} 结束"));
                        continue;
                    }
                    // 健康的 follow 流不会发 end；主干遇 end 就永久冻结树，这里选择整条重开。
                    // 重开前把两张流标记一起清掉：`Mux::next_id` 是每个实例自己的计数，
                    // 新连接的 streamId 从 rs1 重新编起 ⇒ 留着旧 id 就会把下一批帧串到别的流上。
                    self.mux = None;
                    self.follows.clear();
                    self.control_stream = None;
                    self.push_log(format!("工作区流 {stream} 结束，重开"));
                    self.open_mux(context);
                    return;
                }
            }
        }
        if changed {
            self.push_log(format!("工作区数: {}", self.tree.workspaces.len()));
        }
        if let Some(line) = control.line(&self.control) {
            self.push_log(line);
        }
        self.poll_mux(context);
    }

    /// 主线 `StartKernelBoot`（KC:255-262）：加载卡上屏、秒表起、tick 上臂，再把引导链整条丢到
    /// Windows 线程池 —— 界面一个字都不阻塞（旧的整窗遮罩就是在这里被换成内容面卡片的）。
    ///
    /// 四段阶段上报（KC:24-30）按规格 §7.1 的插入点落到分叉现有的链上，**没有压成两段**：
    /// ①准备内核组件 = `Launch::from_env` 之前；②启动内核 = `Kernel::start` 之前（握手最长 90s，
    /// `kernel.rs:12`，条子在这段停很久是预期行为）；③连接内核 = `Kernel::start` 返回后、第一发
    /// RPC（`list_sessions`，分叉的连接与鉴权就发生在它里面）之前；④加载工作区与会话 = UI 侧的
    /// `Connected` 臂在 `open_mux`（workspace/follow + `$events`，= 主线 RefreshWorkspaces/
    /// SubscribeEvents）之前直接调 `report_stage(4)`。①②③ 走 mpsc 由 tick 抽干，④ 本来就在 UI 线程。
    ///
    /// **已备案偏差（第 1 段的真刻度）**：主线第 1 段慢在壳自己装插件（`EnsureAsync` 跑 pnpm，
    /// 台账来自扫 node_modules），分叉不装包 ⇒ 唯一知道台账的是内核，而问内核要走 RPC ⇒ 内核
    /// 握手因此落在**第 1 段窗口内**（`Kernel::start` 之后才报 ②）。不这么做就只有两个坏选项：
    /// ① 把台账抽样放到第 2 段窗口 ⇒ 撞上模型那道 `step == 1` 闸门（主线 KC:234 的优先级），
    /// 第 1 段仍是恒 1.0 插值、`正在安装插件 x/y` 永不上屏；② 放宽模型那道闸门 ⇒ 直接改掉
    /// 主干的状态机。代价：分叉的「正在启动内核…」这一行只在第 1 段抽干后台账之后亮一下
    /// （真机 `Launch::from_env` + 握手 + 抽台账 = 第 1 段，与主线同一批活）。
    /// 锁这条偏差的用例 = `boot_plugin_ledger_producer_is_wired`（④ 那一段就是它）。
    fn start_kernel(&mut self, context: &ComponentContext<Shell>) {
        self.boot.show();
        self.boot_started = Some(Instant::now());
        let (tx, rx) = mpsc::channel();
        self.boot_events = Some(rx);
        self.set_status("启动中…");
        self.arm_boot_tick(context);
        self.boot_task = Some(context.spawn_background(move |_token| {
            let _ = tx.send(BootEvent::Stage(1));
            let launch = match Launch::from_env() {
                Err(error) => {
                    // 主线 MW:2483-2486：内置内核拿不出来时走的是 `FailKernelBoot(reasonKey)`
                    // **单参**重载 ⇒ 加载卡上只有那一句定性，没有「：异常原文」。原文只落 DIAG。
                    emit_line(&format!("DIAG: 内核组件解析失败: {error}"));
                    return Msg::Failed(
                        "未找到内置内核；请安装 npm 版 dsh 或重新安装 Blade²".to_string(),
                        String::new(),
                    );
                }
                Ok(launch) => launch,
            };
            let mut kernel = match Kernel::start(&launch) {
                Err(error) => return Msg::Failed("内核启动失败".to_string(), error),
                Ok(kernel) => kernel,
            };
            // 第 1 段的真刻度：抽内核的插件装载台账（`ready < total` 期间条子按 5→30 插值爬，
            // `正在安装插件 x/y` 那行同时上屏）。问不出来 ⇒ 立刻收手，与没有这一发之前一致。
            report_plugin_ledger(&tx, &mut kernel);
            let _ = tx.send(BootEvent::Stage(2));
            let _ = tx.send(BootEvent::Stage(3));
            let shared = Arc::new(Mutex::new(kernel));
            let listed = shared
                .lock()
                .map_err(|_| "内核状态不可用".to_string())
                .and_then(|mut kernel| kernel.list_sessions());
            match listed {
                Ok(rows) => Msg::Connected(shared, rows),
                Err(error) => {
                    // 主线 MW:2436：兜住的异常只取 `ex.Message`，分叉不自加「（内核已启动）」。
                    Msg::Failed("内核启动失败".to_string(), error)
                }
            }
        }));
    }

    /// 主线 `StartKernelBootTick`（KC:186-196）的替身：reactor 0.100.0 没有已发布的定时器
    /// （唯一的 `DispatcherQueueTimer` 用法在 `#[cfg(feature = "test")]` 里），所以用
    /// `ComponentContext::spawn_background` 自续期 —— **处理完上一发才上下一发**，
    /// 同一时刻永远只有一发在飞；`seq` 守卫负责丢掉撤卡/重试后的迟到那发（规格 §6.2）。
    fn arm_boot_tick(&mut self, context: &ComponentContext<Shell>) {
        self.boot.tick += 1;
        let seq = self.boot.tick;
        let _task = context.spawn_background(move |_token| {
            std::thread::sleep(Duration::from_millis(kernel_boot::TICK_MS));
            Msg::BootTick(seq)
        });
    }

    /// 主线 `OnKernelBootTick` 里那一发 100ms 的正文：抽干引导线程的阶段上报 → 失败态就地冻结
    /// （**不再上臂**，等价 `sender.Stop()`）→ 读秒 → 推一格 → 重新上臂。
    fn on_boot_tick(&mut self, seq: u64, context: &ComponentContext<Shell>) {
        if !self.boot.accepts_tick(seq) {
            return;
        }
        self.drain_boot_events();
        if self.boot.failure.is_some() {
            return;
        }
        self.boot.seconds = self
            .boot_started
            .map(|started| started.elapsed().as_secs() as i32)
            .unwrap_or(0);
        self.boot.paint();
        self.arm_boot_tick(context);
    }

    /// tick 兼任「引导线程 → 模型」的排水口：一次抽干，不逐事件重排 arm。
    fn drain_boot_events(&mut self) {
        let Some(receiver) = self.boot_events.as_ref() else {
            return;
        };
        while let Ok(event) = receiver.try_recv() {
            match event {
                BootEvent::Stage(step) => self.boot.report_stage(step),
                BootEvent::Plugins(ready, total) => self.boot.report_plugins(ready, total),
            }
        }
    }

    /// 主线 MW:2626-2629：引导链路全绿 → 条子推满（`shown` 也直接写 100，不补间）→ 驻留 350ms
    /// 让那一格被画上 → 撤卡。撤卡后空态/会话清单的显隐交回 `hero_state()`（UpdateEmptyState）。
    fn boot_succeed(&mut self, context: &ComponentContext<Shell>) {
        if !self.boot.showing || self.boot.done {
            return;
        }
        self.boot.complete();
        let seq = self.boot.seq;
        let _task = context.spawn_background(move |_token| {
            std::thread::sleep(Duration::from_millis(kernel_boot::DONE_DWELL_MS));
            Msg::BootDone(seq)
        });
    }

    /// 主线 `OnKernelBootRetryClick` + `RestartKernelBootAsync`（KC:270-305），逐步对应：
    /// · `CancelKernelBoot()` → `boot_task.cancel()`（`component.rs:534-541`：摘掉排队中的结果，
    ///   已跑完那发也因 `TaskControl` 进了 Cancelled 而丢弃）；
    /// · `_rpc = null` / `await staleRpc.DisposeAsync()` → `self.kernel = None`（`Kernel::drop`
    ///   就是 `shutdown()`，`kernel.rs:659`，旧进程随句柄归零收掉）；
    /// · `_kernel = new DshKernelHost()` → 下一轮 `start_kernel` 里 `Kernel::start` 新起的宿主；
    /// · 四个长驻流标记（`_workspaceStreamOpen` / `_controlStreamId` / `_sessionStreamId` /
    ///   `_businessStreamsResetPending`）→ 分叉的 workspace/follow 与 `$events` 流是 `Mux` 实例
    ///   自带的（`open_mux`），会话 follow 流记在 `follows` 里 ⇒ **丢实例 = 丢流**，等价那四件套。
    /// 进度态、失败态、`kernel_wait_hint_shown` 全由 `start_kernel` 里的 `boot.show()` 重置。
    fn restart_kernel_boot(&mut self, context: &ComponentContext<Shell>) {
        if let Some(task) = self.boot_task.take() {
            task.cancel();
        }
        self.kernel = None;
        self.mux = None;
        self.follows.clear();
        self.control_stream = None;
        self.boot_events = None;
        // 主线不清会话台账（`_rpc=null` 只让依赖内核的动作早退）；新一轮 `Connected`
        // 会整表覆盖 `rows`，所以留旧数据比抹白一排更贴近主线。
        self.start_kernel(context);
    }

}

impl Component for Shell {
    type Input = ();
    type Message = Msg;

    fn create(_input: &(), context: &ComponentContext<Self>) -> Self {
        let mut shell = Self {
            page: Page::Chat,
            kernel: None,
            // 空串 = 「还没打过 STATUS 行」，紧接着的 set_status("未连接") 因此必然出声。
            status: String::new(),
            rows: Vec::new(),
            input: String::new(),
            search: String::new(),
            active: None,
            catalog: Catalog::load("zh", None),
            scheme: Scheme::Dark,
            // 首轮引导在下面的 `start_kernel` 里才上卡，所以这里是「还没上屏」的初值。
            boot: BootState::idle(),
            boot_events: None,
            boot_started: None,
            boot_task: None,
            hover: None,
            pressed: None,
            pill_hover: None,
            mux: None,
            tree: WorkspaceTree::default(),
            // 主干只有「未分组」节点默认展开，真实工作区节点一律从折叠开始。
            expanded: vec![String::new()],
            uncapped: Vec::new(),
            section: DEFAULT_SETTINGS_SECTION.to_string(),
            sub: None,
            fields: Vec::new(),
            numbers: Vec::new(),
            font_size: type_ramp::BODY.size,
            picks: Vec::new(),
            switches: Vec::new(),
            system_scheme: Scheme::Dark,
            running: Vec::new(),
            bubbles: Vec::new(),
            follows: Vec::new(),
            control_stream: None,
            control: ControlState::default(),
            live: None,
            echo: None,
            thinking: false,
            header_model: String::new(),
            pending_user: None,
            system_prompt_seen: Vec::new(),
            turn_starts: Vec::new(),
            mutations: Vec::new(),
            produced: Vec::new(),
            copied: None,
            reasoning_open: Vec::new(),
            feedback: Vec::new(),
            feedback_busy: Vec::new(),
            feedback_status: Vec::new(),
            note_for: None,
            note_text: String::new(),
            // 初值 `Other`：没人在 composer 里打过字、也没按下过它 ⇒ 宁可放行一个 Enter，
            // 不可误发一条消息（Enter 退回旧分叉的行为：就是个换行/不做事）。
            key_owner: KeyOwner::Other,
            palette: PaletteState::idle(),
            pending_send: None,
            command_submitting: false,
            brand: brand_mark_path(),
        };
        // 诊断走 stdout + `BLADE2_RS_LOG` 那两份通道（界面上没有自测条）：这一行让「进程活着但内核
        // 还没起来」与「窗口压根没出现」在 `<log>.rs.txt` 里可区分。
        shell.set_status("未连接");
        // 主干在窗口构造末尾 `_ = StartKernelBoot()`（MainWindow.xaml.cs:2381）：内核自己起，
        // 界面上没有任何「连接」入口，启动期只有内容面正中那张加载卡（旧的整窗遮罩已删，见 `kernel_boot_panel`）。
        shell.start_kernel(context);
        // 键盘钩子 = 分叉唯一的按键通路（`blade2_rs::keys`）：**组件建好之后**装，回投句柄用
        // `context.sender()`（`Rc<RefCell<…>>`、非 `Send`，与钩子同线程）。
        // 闭包里的铁律：先 clone 句柄、把借用丢掉，**再** send —— 钩子在消息泵里会重入，
        // 跨着借用 send 就是 `BorrowMutError` panic = 穿 `extern "system"` 边界 = 进程没了。
        let sender = context.sender();
        let hooked = keys::install(move |action| {
            let Some(message) = Msg::from_key_action(action) else {
                return;
            };
            let sender = sender.clone();
            let _ = sender.send(message);
        });
        keys::sync(shell.key_owner, shell.sub.is_some());
        // 自测标记行：`DIAG: KBHOOK …`（stdout + `BLADE2_RS_LOG`，界面上不占一个像素）。
        shell.push_log(format!(
            "KBHOOK installed={hooked} owner={:?} sub_open=false",
            keys::owner()
        ));
        shell
    }

    fn update(&mut self, message: Msg, context: &ComponentContext<Self>) {
        match &message {
            Msg::Press(key) => pill_trace(format!("MSG Press {key:?}")),
            Msg::Hover(key) => pill_trace(format!("MSG Hover {key:?}")),
            Msg::PillHover(key) => pill_trace(format!("MSG PillHover {key:?}")),
            other => {
                let kind = std::any::type_name_of_val(other);
                pill_trace(format!("MSG OTHER {}", &kind[kind.rfind(':').map_or(0, |i| i + 1)..]));
            }
        }
        // 圆底钮的按下态**只**由那颗钮自己的指针事件管：`Press(Some)` 进档，`Press(None)`
        // 出档（抬起时那三个收尾事件实测连发：`PILL-EVT cancel` + `PILL-EVT caplost`）。
        // 这里原来还有一条「任何非 Press 消息一律清掉」的兜底，正是 D1 那一刀的成因：
        // 工作区 follow 流每来一批事件就发一发 `Msg::MuxEvents`（约 250ms 一发，
        // `BLADE2_PILLDBG=1` 下的 tmp/qa5-run2-app.err.txt:90-131 逐帧记着
        // `PILL-EVT press` → `tier=press` → `MSG OTHER` → `tier=hover`），
        // 按住不放的第一个心跳就把 tier 打回 hover ⇒ 按下档根本来不及上屏，
        // 量到的按下截图永远等于悬停截图。抬起时那三个事件本来就会清，兜底纯属多余。
        // 因此这里**不再**动 `self.pressed`，只留 `Msg::Press` 那一条 arm（见下方 match）。
        match message {
            Msg::Nav(page) => {
                // 主干 `ShowSettingsAsync` 每次进设置都 `_settingsActiveSection = ""` + `ActivateSectionAsync("general")`，
                // 即不记住上次分区；退出时 `ShowChatPage → ResetSettingsSubPages` 清掉二级页。
                self.sub = None;
                if page == Page::Settings {
                    self.section = DEFAULT_SETTINGS_SECTION.to_string();
                    // 设置页没有 composer ⇒ 焦点归属直接判给 `Other`，Enter 不可能被误判成发送
                    // （从聊天页切过来时那份「刚在输入框里打过字」的证据已经过期了）。
                    self.key_owner = KeyOwner::Other;
                }
                self.page = page;
            }
            Msg::Scheme(scheme) => {
                self.system_scheme = scheme;
                self.apply_theme();
            }
            Msg::Pick(key, index) => {
                self.set_pick(&key, index);
                if key == "ui-theme.preference" {
                    self.apply_theme();
                }
            }
            Msg::Toggle(key, on) => self.set_on(&key, on),
            Msg::Note(text) => self.push_log(text),
            // 发送钮与 Enter 汇到同一条路上（`keys::classify` 给的是 `KeyAction::Send` ⇒
            // `Msg::Send`，与那颗钮 `on_click` 挂的字面同一个变体）。
            Msg::Send => self.submit_input(context, None),
            Msg::SendAlternate => {
                let mode = self.ctrl_enter_mode();
                self.submit_input(context, mode);
            }
            Msg::CommandsLoaded {
                generation,
                session,
                outcome,
            } => {
                // 主干先写缓存（`EnsureCommandsAsync` 体内，MW:17346-17357），再按代次决定
                // 刷不刷浮层（17701）——顺序反了就等于让过期那一发盖掉新按键。
                self.palette.record(&session, outcome, Instant::now());
                self.apply_palette(generation);
                // 挂着等目录的那一发提交（主干那是条 `await`，分叉把落点挪到这条回填上）。
                if let Some(mode) = self.pending_send.take() {
                    self.submit_input(context, mode);
                }
            }
            Msg::CommandExecuted {
                line,
                session,
                outcome,
                draft,
            } => {
                self.command_submitting = false;
                let success = matches!(&outcome, Ok(reply) if reply.is_success());
                self.report_command(&line, outcome);
                // 主干 17915-17922（`fromComposer` 那一支）：成功 + 还在原来那个会话 + 草稿
                // 一字未改 ⇒ 才清输入框。分叉没移植附件，所以主干紧跟的「摘掉已提交附件」无对应物。
                if let Some(draft) = draft.filter(|_| success) {
                    if self.active.as_deref() == Some(session.as_str()) && self.input == draft {
                        self.input.clear();
                    }
                }
            }
            Msg::PaletteMove(delta) => self.move_palette(delta),
            Msg::PaletteAccept => self.accept_palette(context),
            Msg::PaletteClose => self.palette.hide(),
            Msg::PalettePick(index) => {
                // 主干 `OnCommandItemClick`（MW:17821-17836）：先把 `_commandIndex` 对齐到
                // 点的那一行（`IndexOf` 找不到给 -1），再走同一条采纳通路。
                self.palette.index = i32::try_from(index).unwrap_or(-1);
                self.accept_palette(context);
            }
            Msg::Cancel => self.cancel_run(context),
            Msg::Prompted(result) => match result {
                Ok((session, stream)) => self.on_prompted(&session, &stream, context),
                Err(error) => {
                    self.echo = None;
                    self.push_log(format!("发送失败: {error}"));
                }
            },
            Msg::FollowOpened(result) => match result {
                Ok((session, stream)) => {
                    if !self.follows.iter().any(|(id, _)| id == &session) {
                        self.follows.push((session, stream));
                    }
                }
                Err(error) => self.push_log(format!("会话流打开失败: {error}")),
            },
            Msg::Present(seq, index, action) => self.present_open(seq, index, &action, context),
            Msg::Presented(Ok((status, body))) => match status {
                // 204 = 内核已经把文件交给系统，主干这时直接 return，不写任何提示。
                200 | 204 => {}
                409 => self
                    .push_log(self.catalog.l(
                        "无法打开：宿主桌面不可用（内核 workspaceDesktop().available=false）。",
                    )),
                404 => self.push_log(
                    self.catalog
                        .l("无法打开：内核在会话日志里找不到该交付物（文件可能已被移动或删除）。"),
                ),
                422 => self.push_log(
                    self.catalog
                        .l("无法打开：该文件没有经过校验的宿主路径（可能在工作区沙箱之外）。"),
                ),
                other => self.push_log(self.catalog.lf(
                    "无法打开交付物（HTTP {0}）：{1}",
                    &[other.to_string(), body],
                )),
            },
            Msg::Presented(Err(error)) => {
                self.push_log(self.catalog.lf("无法打开交付物：{0}", &[error]));
            }
            Msg::OpenPath(path) => self.open_path(path, context),
            Msg::Opened(Err(error)) => {
                self.push_log(self.catalog.lf("打开失败：{0}", &[error]));
            }
            Msg::Opened(Ok(())) => {}
            Msg::Select(id) => {
                self.active = Some(id.clone());
                self.page = Page::Chat;
                // 换会话就换一屏：历史由这一路 follow 的 snapshot.records 重放回来。
                self.bubbles.clear();
                self.live = None;
                self.thinking = false;
                // 主干 `OpenSessionAsync`（`MainWindow.xaml.cs:3998`）：清屏那一串副作用里就有
                // `ResetFeedbackState()` —— 反馈条目整族是会话作用域，先把上一个会话的态清干净，
                // 再在流程末尾补上主干最后那句 `LoadFeedbackAsync`（= 下面的 `load_feedback`）。
                self.reset_feedback();
                self.turn_starts.clear();
                self.mutations.clear();
                self.produced.clear();
                self.header_model.clear();
                self.pending_user = None;
                self.copied = None;
                self.reasoning_open.clear();
                self.pill_hover = None;
                self.forget_commands();
                self.ensure_follow(&id, context);
                self.load_feedback(&id, context);
            }
            Msg::Search(text) => {
                self.search = text;
                // 会话搜索框在吃键盘 ⇒ Enter 只能放行（主干那颗框里 Enter 也不做事）。
                self.key_owner = KeyOwner::Other;
            }
            Msg::Hover(key) => {
                // 主干的「已复制」靠 1 秒 DispatcherQueue 定时器回落；reactor 借不到定时器，
                // 改成指针离开该行时回落（行本身就是那颗钮的容器，离开即代表这一眼已经看完）。
                if key.is_none() {
                    self.copied = None;
                }
                self.hover = key;
            }
            Msg::Press(key) => self.pressed = key,
            // 钮级悬停与行级提亮分槽，所以这条消息**不碰** `hover` / `copied`：
            // 指针从行内钮挪回正文时，那一行还得保持提亮（主干整卡 PointerEntered 只管卡）。
            Msg::PillHover(key) => self.pill_hover = key,
            Msg::Copy(key, text) => {
                // 主干整段 catch 掉异常、失败连图标都不换。
                if clipboard::copy_text(&text) {
                    self.copied = Some(key);
                }
            }
            Msg::Branch(seq) => self.fork_at(seq, context),
            Msg::Branched(Ok((rows, child, title))) => {
                // 主干 `RefreshSessionsAsync` 的结果先落表：子会话那一行就是这么出现的。
                self.rows = rows;
                self.active = Some(child.clone());
                self.page = Page::Chat;
                self.bubbles.clear();
                self.live = None;
                self.thinking = false;
                // 主干分叉完是 `OpenSessionAsync(child)` 那一整套：反馈表必随会话复位（父会话的
                // 条目不许跟进子会话），末尾再按子会话 id 拉一次。
                self.reset_feedback();
                self.turn_starts.clear();
                self.mutations.clear();
                self.produced.clear();
                self.header_model.clear();
                self.pending_user = None;
                if !title.is_empty() {
                    // 主干 `child.Title = increased` + `RebuildNavMenu()`：左栏那行换成新标题。
                    if let Some(row) = self.rows.iter_mut().find(|row| row.id == child) {
                        row.title = title;
                    }
                }
                self.ensure_follow(&child, context);
                self.load_feedback(&child, context);
                self.push_log(format!("已分叉到 {child}"));
            }
            Msg::Branched(Err(error)) => self.push_log(format!("分叉失败: {error}")),
            Msg::Rate(message, rating, checked) => {
                self.set_feedback_rating(message, rating, checked, context)
            }
            Msg::Revoke(message) => self.delete_feedback(message, context),
            Msg::OpenNote(message) => {
                self.note_text = self
                    .feedback
                    .iter()
                    .find(|(id, _)| *id == message)
                    .and_then(|(_, item)| item.note.clone())
                    .unwrap_or_default();
                self.note_for = Some(message);
                // 焦点证据（反向）：点开说明编辑器 = 下一发键大概率落进那颗多行框
                // （`note_editor` 的根是 `Grid`，框架里只有 `Border` 挂 `PointerPressed`，
                // 所以这条按「谁刚被打开」判，而不是新加一层容器）。
                self.key_owner = KeyOwner::Other;
            }
            Msg::NoteText(text) => {
                self.note_text = text;
                self.key_owner = KeyOwner::Other;
            }
            Msg::CloseNote(save) => {
                if let Some(message) = self.note_for.take() {
                    let rating = self
                        .feedback
                        .iter()
                        .find(|(id, _)| *id == message)
                        .map(|(_, item)| item.rating.clone());
                    // 主干 `EditFeedbackNoteAsync`：`box.Text.Trim()` 后空串按 null 送（= 整键不带），
                    // 保存沿用既有条目的原评级；没有既有条目主干压根不开编辑器。
                    let note = std::mem::take(&mut self.note_text).trim().to_string();
                    if save {
                        if let Some(rating) = rating {
                            self.put_feedback(message, rating, Some(note), context);
                        }
                    }
                }
            }
            Msg::FeedbackList(Ok((session, items))) => {
                if self.active.as_deref() == Some(session.as_str()) {
                    self.feedback.clear();
                    for item in &items {
                        store_feedback(&mut self.feedback, item);
                    }
                }
            }
            // 主干 list 失败一律静默：不进 Status、不写系统消息行。
            Msg::FeedbackList(Err(_)) => {}
            Msg::FeedbackApplied(patch) => self.apply_feedback_patch(patch),
            Msg::Reason(key, open) => {
                match self.reasoning_open.iter().position(|item| item == &key) {
                    Some(index) if !open => {
                        self.reasoning_open.remove(index);
                    }
                    None if open => self.reasoning_open.push(key),
                    _ => {}
                }
            }
            Msg::ToggleGroup(key) => self.toggle_group(&key),
            Msg::Cap(key, open) => self.set_cap(&key, open),
            Msg::Section(id) => {
                // 主干 `RenderSectionAsync` 开头就 `ResetSettingsSubPages()`：换分区必回一级页。
                self.section = id;
                self.sub = None;
            }
            Msg::OpenSub(id) => self.sub = Some(id),
            Msg::CloseSub => self.sub = None,
            Msg::Field(key, value) => {
                set_pair(&mut self.fields, key, value);
                self.key_owner = KeyOwner::Other;
            }
            Msg::KeyFocus(owner) => self.key_owner = owner,
            Msg::Number(key, value) => set_pair(&mut self.numbers, key, value),
            Msg::FontStep(step) => {
                self.font_size =
                    (self.font_size + step).clamp(size::FONT_SIZE_MIN, size::FONT_SIZE_MAX);
            }
            Msg::MuxReady(Ok((shared, control))) => {
                self.mux = Some(shared);
                // 流标记与 `mux` 实例同一条消息里落地：`poll_mux` 是这条消息末尾才上臂的，
                // 所以第一批控制帧必然认得出自己的流（先 polling 后记 id 就会串到兜底那一支）。
                self.control_stream = Some(control);
                self.poll_mux(context);
                // 主线 MW:2626-2630：第 4 段（RefreshWorkspaces / SubscribeEvents）跑完才算引导成功
                // ⇒ 条子推满、驻留 350ms、撤卡。卡已经不亮着（日后重开流）时 `boot_succeed` 自己早退。
                self.boot_succeed(context);
            }
            Msg::MuxReady(Err(error)) => {
                // 主线：第 4 段抛异常 → `RunKernelBootAsync` 兜住 → `FailKernelBoot("内核启动失败", msg)`。
                // 只有卡还亮着时才当引导失败；日常断流重开（`apply_mux_events` 那条路）不该把加载卡唤回来。
                if self.boot.showing && self.boot.failure.is_none() {
                    self.kernel_boot_failed("内核启动失败", &error);
                } else {
                    self.push_log(format!("工作区流未连接: {error}"));
                }
            }
            Msg::MuxFailed(error) => {
                // 丢实例 = 丢流（见 `restart_kernel_boot` 那条对应关系）：两张流标记跟着作废，
                // 否则下一根连接的 streamId 会顶着旧映射把帧分给别的流。
                self.mux = None;
                self.follows.clear();
                self.control_stream = None;
                self.push_log(format!("工作区流中断: {error}"));
            }
            Msg::MuxEvents(events) => self.apply_mux_events(events, context),
            // 焦点归属（钩子的 Composer/Other 二档）：XAML 真把字送进了哪颗框，就是那颗在吃键。
            Msg::Input(text) => {
                self.input = text;
                self.key_owner = KeyOwner::Composer;
                // 主干那颗 InputBox 的 `TextChanged` 只挂了一件事：`UpdateCommandPalette`
                // （+ @ 引用浮层，分叉未移植）——所以浮层完全由文本驱动，没有独立开关。
                self.refresh_palette(context);
            }
            Msg::Connected(shared, rows) => {
                // 先把引导线程那三段上报落地，再在 `open_mux` 之前报第 4 段
                // （主线 MW:2612 的 `ReportKernelBootStage("正在加载工作区与会话…")` 同一位置）。
                self.drain_boot_events();
                self.boot.report_stage(4);
                if let Ok(kernel) = shared.lock() {
                    self.set_status(format!(
                        "已连接 {} (pid {})",
                        kernel.url,
                        kernel.pid().unwrap_or(0)
                    ));
                    self.push_log(kernel.log.join("\n"));
                }
                self.kernel = Some(shared);
                self.rows = rows;
                self.push_log(format!("会话数: {}", self.rows.len()));
                self.open_mux(context);
            }
            Msg::Failed(reason, detail) => self.kernel_boot_failed(&reason, &detail),
            // 主线 `_kernelBootTick`（100ms）：守卫在 `on_boot_tick` 首行，撤卡/失败后不再上臂。
            Msg::BootTick(seq) => self.on_boot_tick(seq, context),
            Msg::BootDone(seq) => {
                // 主线 MW:2629 的撤卡：只认本轮代号，重试过的轮次那发延时直接丢。
                if seq == self.boot.seq && self.boot.done {
                    self.boot.hide();
                    self.boot_events = None;
                    self.boot_started = None;
                    self.push_log("加载卡撤下");
                }
            }
            Msg::BootRetry => self.restart_kernel_boot(context),
            Msg::NewSession => {
                let cwd = env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                let outcome = self.kernel.as_ref().map(|shared| {
                    shared
                        .lock()
                        .map_err(|_| "内核状态不可用".to_string())
                        .and_then(|mut kernel| kernel.create_session(&cwd))
                });
                match outcome {
                    None => self.fail("内核未连接".to_string()),
                    Some(Err(error)) => self.fail(error),
                    Some(Ok(id)) => {
                        self.active = Some(id.clone());
                        // 主干 `CreateSessionAsync` 末尾也是 `OpenSessionAsync`：新会话同样
                        // 「先复位再拉一次」，别把上一条会话的反馈态带进这个空白会话。
                        self.reset_feedback();
                        // 主干 `CreateSessionAsync`：清单里还没有这条就先本地合成一行。
                        self.ensure_local_session_row(&id);
                        self.forget_commands();
                        self.load_feedback(&id, context);
                        self.push_log(format!("已创建会话 {id}"));
                    }
                }
            }
        }
        // 钩子读的是 thread-local 副本：每条消息落完推一次（`sub` 会在这条消息里变，
        // `key_owner` 也是）。放收尾而不是各条 arm 里，是为了「二级页开着」这类事实永远不会漏。
        self.publish_keys();
    }

    fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
        let scheme = context.callback(|scheme: ColorScheme| {
            Msg::Scheme(match scheme {
                ColorScheme::Dark => Scheme::Dark,
                ColorScheme::Light => Scheme::Light,
            })
        });
        context.on_color_scheme(scheme);
        context.window_title("Blade²");
        context.window_visuals(
            WindowVisuals::new()
                .client_size(size::WINDOW_DEFAULT_WIDTH, size::WINDOW_DEFAULT_HEIGHT)
                .constraints(WindowConstraints {
                    min_width: Some(size::WINDOW_MIN_WIDTH),
                    min_height: Some(size::WINDOW_MIN_HEIGHT),
                    max_width: None,
                    max_height: None,
                })
                .backdrop(WindowBackdrop::Mica),
        );

        let mut palette = Palette::for_scheme(self.scheme);
        // 主干 `ApplyBubbleMaterial`（MW:18109）在构造里就先落一次、之后每改一次设置落一次：
        // 气泡底由「材质档 + 不透明度 + 窗口材质」现算并覆掉 Tokens.xaml 那支基线，
        // 用户/助手/交付物三处气泡读的都是 `p.bubble` ⇒ 改这两个控件就是全量即时生效。
        palette.bubble = palette.bubble_with(
            self.bubble_material(),
            self.window_material(),
            self.bubble_opacity(),
        );
        // 槽位按状态增删：缺席的 keyed cell 会被真正 Destroy（retire 路径发 RemoveChild + native Destroy）。
        // 这里**没有**启动遮罩：主线 `KernelBootPanel` 不是整窗覆盖层，它挂在 ChatPage 的内容面里
        // （`MainWindow.xaml:691-709` Row 1），见 `chat_page` / `kernel_boot_panel`。
        let cells: Vec<KeyedView> = vec![
            // 主干把 TitleBarDragRegion 垫在最底层，所以拖拽条必须是第一个子元素。
            KeyedView::new("drag", self.drag_region()),
            KeyedView::new("strip", self.strip(context, palette, 0)),
            KeyedView::new("body", self.body(context, palette, 1)),
        ];
        // 两行布局：48px 顶条 + 占满余下高度的正文。**没有第三行**——
        // 原先那条自测条（`debug_strip`）在窗口底部占一整行，把聊天列往上顶出一截
        // 「下巴」（用户 2026-09-22 的要求：界面上一个像素都不许留），
        // 诊断信息现在全走 stdout + `BLADE2_RS_LOG` 文件，见 `Shell::set_status` / `emit_line`。
        Grid::new()
            .rows([
                GridLength::Pixel(size::CAPTION_STRIP),
                GridLength::STAR,
            ])
            .keyed_children(cells)
    }
}

impl Shell {
    fn drag_region(&self) -> View {
        // 树里那份「零布局成本」的连接证据挂在这颗 TitleBar 上：它本来就出现在 UIA 树里
        // （加 Name 不会新增节点、也不把别的元素往下挤一层），而 WinUI 渲染的是它的 `Title`
        // （这里显式给 None），不是 AutomationProperties.Name ⇒ 界面上一个字都不多。
        // 于是老配方 `tests/gui_uia.ps1 -WaitFor '内核: 已连接'`（读树里的 Name）没被删掉自测条
        // 那一刀彻底弄断。
        TitleBar::new()
            .grid_row(0)
            .grid_column_span(2)
            .title_optional(None::<String>)
            .is_back_button_visible(false)
            .is_pane_toggle_button_visible(false)
            .preferred_height(WindowTitleBarHeight::Standard)
            .automation_name(format!("内核: {}", self.status))
            .into()
    }

    /// 顶条：`[Auto, Auto, *, 152]`，152 是系统 caption 三键预留。
    fn strip(&self, context: &mut ViewContext<Shell>, p: Palette, row: i32) -> View {
        let mut cells: Vec<KeyedView> = Vec::new();
        if self.page == Page::Settings {
            cells.push(KeyedView::new(
                "back",
                // 主干 `MainWindow.xaml:100-108`：ShellBackButton 走 IconButtonStyle
                // （32×32 / Padding 0 / MinWidth 0 / r=CornerSmall / 底色 Transparent），
                // 字形 E72B @ GlyphSizeBody=14。
                pill_button(
                    Button::new()
                        .grid_column(0)
                        .margin(th([8.0, 0.0, 0.0, 0.0]))
                        .automation_id("ShellBackButton")
                        .automation_name(self.catalog.l("返回"))
                        .on_click(context.message(Msg::Nav(Page::Chat))),
                    mark_size(glyph::BACK, size::GLYPH_BODY),
                    Pill {
                        side: size::SHELL_BACK_BUTTON,
                        ..Pill::ghost(p)
                    },
                    "shell-back",
                    HoverSlot::Row,
                    self.hover.as_deref(),
                    self.pressed.as_deref(),
                    context,
                ),
            ));
        }
        cells.push(KeyedView::new(
            "brand",
            StackPanel::new()
                .grid_column(1)
                .orientation(Orientation::Horizontal)
                .spacing(10.0)
                .margin(th([14.0, 0.0, 0.0, 0.0]))
                .vertical_alignment(VerticalAlignment::Center)
                .children((
                    brand_mark(self.brand(), size::TOP_BAR_MARK, ""),
                    TextBlock::new()
                        .text("Blade²")
                        .font_size(type_ramp::BODY.size)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .foreground(p.text_primary)
                        .vertical_alignment(VerticalAlignment::Center),
                )),
        ));
        cells.push(KeyedView::new(
            "search",
            Border::new()
                .grid_column(2)
                .width(size::SEARCH_MAX_WIDTH)
                .height(size::TOUCH_TARGET)
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center)
                .background(p.control_fill)
                .border_brush(p.stroke)
                .border_thickness(size::STROKE)
                .corner_radius(radius::PILL)
                // 焦点证据（反向）：按进搜索框那一颗 pill ⇒ Enter 只能放行。
                .on_pointer_pressed(context.callback(|_| Msg::KeyFocus(KeyOwner::Other)))
                .content(
                    Grid::new()
                        .columns([GridLength::Auto, GridLength::STAR])
                        .children((
                            // 主干 `MainWindow.xaml:165-172` SearchPillGlyph：E721 @ GlyphSizeBody=14、
                            // Margin 12,0,0,0、TextSecondaryBrush。
                            Border::new()
                                .grid_column(0)
                                .margin(th([12.0, 0.0, 0.0, 0.0]))
                                .vertical_alignment(VerticalAlignment::Center)
                                .opacity(0.72)
                                .content(mark_size(glyph::SEARCH, size::GLYPH_BODY)),
                            TextBox::new()
                                .grid_column(1)
                                .text(self.search.clone())
                                .placeholder_text(self.catalog.l("搜索会话…"))
                                .min_height(size::TOUCH_TARGET)
                                .background(p.transparent)
                                .border_thickness(0.0)
                                .margin(th([6.0, 0.0, 8.0, 0.0]))
                                .on_text_changed(context.callback(|text: String| Msg::Search(text)))
                                // 主干 `MainWindow.xaml` 那颗搜索框写死了
                                // `AutomationProperties.AutomationId="SessionSearchBox"`
                                // （自测矩阵按 id 取元素，只有 Name 时量到 MISSING）。
                                .automation_id("SessionSearchBox")
                                .automation_name("搜索会话"),
                        )),
                ),
        ));
        Grid::new()
            .grid_row(row)
            .columns([
                GridLength::Auto,
                GridLength::Auto,
                GridLength::STAR,
                GridLength::Pixel(size::CAPTION_RESERVED),
            ])
            .keyed_children(cells)
    }

    /// 内容区：264px 侧栏（无底板，透出 Mica）+ SurfaceAlt 内容面。
    fn body(&self, context: &mut ViewContext<Shell>, p: Palette, row: i32) -> View {
        let on_chat = self.page == Page::Chat;
        Grid::new()
            .grid_row(row)
            .columns([GridLength::Pixel(size::OPEN_PANE), GridLength::STAR])
            .children((
                Border::new()
                    .grid_column(0)
                    .content(self.nav(context, p)),
                Border::new()
                    .grid_column(1)
                    .background(p.surface_alt)
                    .content(self.either(
                        on_chat,
                        ("page-chat", self.chat_page(context, p)),
                        ("page-settings", self.settings_page(context, p)),
                    )),
            ))
    }

    /// 同一槽位二选一：只把选中的那棵放进 keyed_children，缺席那棵整棵退役。
    /// 不要用「两层常驻 + opacity(0)」隐藏 —— reactor 的 `opacity` 只映射到 SetOpacity，
    /// 透明层依然被布局且吃命中，隐藏层会把整块区域的点击全吞掉。
    fn either(&self, on_first: bool, first: (&str, View), second: (&str, View)) -> View {
        let (key, view) = if on_first { first } else { second };
        Grid::new().keyed_children(vec![KeyedView::new(key, view)])
    }

    /// NavigationView 的替身：主干把 pane 背景钉成 Transparent，自绘反而更保真。
    fn nav(&self, context: &mut ViewContext<Shell>, p: Palette) -> View {
        let on_chat = self.page == Page::Chat;
        self.either(
            on_chat,
            ("nav-sessions", self.nav_sessions(context, p)),
            ("nav-sections", self.nav_sections(context, p)),
        )
    }

    fn nav_sessions(&self, context: &mut ViewContext<Shell>, p: Palette) -> View {
        let rows = self.visible_rows();
        let mut items: Vec<KeyedView> = Vec::new();
        items.push(KeyedView::new(
            "new-session",
            self.nav_item(
                "new-session",
                self.catalog.l("新会话"),
                glyph::NEW_SESSION,
                false,
                // 主干 `OnNavItemInvoked` 的 `case "new-session": await CreateSessionAsync()`
                // —— 这颗钮**真的**建会话，不是「回到聊天页」。原先分叉只有自测条上那颗
                // 「新建会话」驱动 `Msg::NewSession`，自测条删掉后这条动线必须由真实 UI 接住。
                Msg::NewSession,
                context,
                p,
            ),
        ));
        items.push(KeyedView::new(
            "workspace",
            self.workspace_header(&rows, context, p),
        ));
        self.push_groups(&mut items, &rows, context, p);
        items.push(KeyedView::new(
            "settings",
            self.nav_item(
                "settings",
                self.catalog.l("设置"),
                glyph::SETTINGS,
                self.page == Page::Settings,
                Msg::Nav(Page::Settings),
                context,
                p,
            ),
        ));
        StackPanel::new()
            .orientation(Orientation::Vertical)
            // 左右各 0：4 DIP 侧留白由 `nav_row` 自己的 margin 给（主干 NavigationViewItemButtonMargin），
            // 这里再给 4 就变成 8（实测带侧留白 9 DIP = 面板 4 + 行 4 + 模板描边 1，是主干两倍多）。
            .margin(th([0.0, 4.0, 0.0, 4.0]))
            .keyed_children(items)
    }

    /// 主干进设置时 `ShowSettingsAsync` 把固定项折叠、往 `Nav.MenuItems` 里塞 12 个分区项，
    /// 底部「设置」项不隐藏（再点一次是切回聊天页）。分区项没有任何尺寸覆写，全靠模板默认值。
    fn nav_sections(&self, context: &mut ViewContext<Shell>, p: Palette) -> View {
        let mut items: Vec<KeyedView> = SETTINGS_SECTIONS
            .iter()
            .map(|(id, title, code)| {
                KeyedView::new(
                    format!("section-{id}"),
                    self.nav_item(
                        id,
                        self.catalog.l(title),
                        *code,
                        self.section == *id,
                        Msg::Section((*id).to_string()),
                        context,
                        p,
                    ),
                )
            })
            .collect();
        items.push(KeyedView::new(
            "settings",
            self.nav_item(
                "settings",
                self.catalog.l("设置"),
                glyph::SETTINGS,
                // 主干 FooterMenuItems 里的 SettingsItem 是 SelectsOnInvoked="False"，
                // 进设置时 Nav.SelectedItem 已被清成 null 并被分区项接管 → 它永远不会带选中条。
                false,
                Msg::Nav(Page::Chat),
                context,
                p,
            ),
        ));
        StackPanel::new()
            .orientation(Orientation::Vertical)
            // 左右各 0：4 DIP 侧留白由 `nav_row` 自己的 margin 给（主干 NavigationViewItemButtonMargin），
            // 这里再给 4 就变成 8（实测带侧留白 9 DIP = 面板 4 + 行 4 + 模板描边 1，是主干两倍多）。
            .margin(th([0.0, 4.0, 0.0, 4.0]))
            .keyed_children(items)
    }

    /// 主干：工作区节点来自 follow 流且默认折叠；没有工作区记录时会话全落在「未分组」。
    fn push_groups(
        &self,
        items: &mut Vec<KeyedView>,
        rows: &[&SessionInfo],
        context: &mut ViewContext<Shell>,
        p: Palette,
    ) {
        if self.tree.workspaces.is_empty() {
            self.push_group(
                items,
                "",
                self.catalog.l("未分组"),
                glyph::UNSORTED,
                rows,
                context,
                p,
            );
            return;
        }
        for workspace in &self.tree.workspaces {
            let group = self.tree.group_sessions(&workspace.id, rows);
            self.push_group(
                items,
                &workspace.id,
                workspace.title.clone(),
                glyph::WORKSPACE,
                &group,
                context,
                p,
            );
        }
        let filed = self.tree.filed_ids();
        let unfiled: Vec<&SessionInfo> = rows
            .iter()
            .copied()
            .filter(|row| !filed.contains(&row.id.as_str()))
            .collect();
        if !unfiled.is_empty() {
            self.push_group(
                items,
                "",
                self.catalog.l("未分组"),
                glyph::UNSORTED,
                &unfiled,
                context,
                p,
            );
        }
    }

    fn push_group(
        &self,
        items: &mut Vec<KeyedView>,
        key: &str,
        title: String,
        code: char,
        rows: &[&SessionInfo],
        context: &mut ViewContext<Shell>,
        p: Palette,
    ) {
        let nested = !key.is_empty();
        items.push(KeyedView::new(
            format!("group-{key}"),
            self.nav_item(
                &format!("node-{key}"),
                title,
                code,
                false,
                Msg::ToggleGroup(key.to_string()),
                context,
                p,
            ),
        ));
        if !self.group_open(key) {
            return;
        }
        let capped = rows.len() > size::SESSION_PREVIEW_LIMIT
            && !self.uncapped.iter().any(|item| item == key);
        let limit = if capped {
            size::SESSION_PREVIEW_LIMIT
        } else {
            rows.len()
        };
        for row in &rows[..limit] {
            let selected = self.active.as_deref() == Some(row.id.as_str());
            items.push(KeyedView::new(
                row.id.clone(),
                self.session_row(row, selected, nested, context, p),
            ));
        }
        if !capped {
            return;
        }
        let hidden = rows.len() - size::SESSION_PREVIEW_LIMIT;
        items.push(KeyedView::new(
            format!("expand-{key}"),
            self.nav_item(
                &format!("cap-{key}"),
                self.catalog
                    .lf("展开其余 {0} 个会话", &[hidden.to_string()]),
                glyph::EXPAND_GROUP,
                false,
                Msg::Cap(key.to_string(), true),
                context,
                p,
            ),
        ));
    }

    fn workspace_header(
        &self,
        rows: &[&SessionInfo],
        context: &mut ViewContext<Shell>,
        p: Palette,
    ) -> View {
        let empty = if rows.is_empty() {
            TextBlock::new()
                .text(self.catalog.l("暂无会话"))
                .font_size(type_ramp::CAPTION.size)
                .foreground(p.text_tertiary)
                .into()
        } else {
            blank()
        };
        // 主干 `MainWindow.xaml:229-270`：段头行 `[*,Auto,Auto]`、列距 Space2，右侧两枚
        // IconButtonStyle（32×32、Padding 0、r=CornerSmall、底 Transparent），字形 14。
        // 之前分叉只画了两枚裸 FontIcon（20px、无命中区），既撑大了段头也丢了 UIA 名。
        let mut section_button = |code: char, column: i32, id: &str, name: &str, note: &str| {
            pill_button(
                Button::new()
                    .grid_column(column)
                    .automation_id(id)
                    .automation_name(self.catalog.l(name))
                    .on_click(context.message(Msg::Note(self.catalog.l(note)))),
                mark_size(code, size::GLYPH_BODY),
                Pill::ghost(p),
                &format!("workspace-head-{id}"),
                HoverSlot::Row,
                self.hover.as_deref(),
                self.pressed.as_deref(),
                context,
            )
        };
        StackPanel::new()
            .orientation(Orientation::Vertical)
            .spacing(2.0)
            .margin(th([0.0, 10.0, 0.0, 0.0]))
            .children((
                Grid::new()
                    .columns([GridLength::STAR, GridLength::Auto, GridLength::Auto])
                    .column_spacing(2.0)
                    .children((
                        TextBlock::new()
                            .text(self.catalog.l("工作区"))
                            .font_size(type_ramp::CAPTION.size)
                            .foreground(p.text_secondary)
                            .vertical_alignment(VerticalAlignment::Center),
                        section_button(
                            glyph::MORE,
                            1,
                            "WorkspaceViewOptionsButton",
                            "视图选项",
                            "视图选项尚未移植",
                        ),
                        section_button(
                            glyph::NEW_SESSION,
                            2,
                            "AddWorkspaceButton",
                            "添加工作区",
                            "添加工作区尚未移植",
                        ),
                    )),
                empty,
            ))
    }

    /// NavigationViewItem 的可视替身：主干那颗行的模板根 `LayoutRoot` 写的是
    /// `Margin="{ThemeResource NavigationViewItemButtonMargin}"` = **`4,2`** +
    /// `MinHeight="{ThemeResource NavigationViewItemOnLeftMinHeight}"` = **36 DIP**
    /// （`generic.xaml:32078/32068/32187`），高亮就是 LayoutRoot 自己的 Background ⇒
    /// **带 = 该 item 的 UIA 矩形，侧留白 4 DIP、上下各 2 DIP，选中与悬停同一档同高**。
    /// 分叉这里三处对齐它：① 行 Button 的 margin 固定 `4,2,4,2`（侧留白只由这一处给，
    /// 外层 StackPanel 的左右 margin 已改成 0，见 `nav_sessions`/`nav_sections`；原先
    /// 「面板 4 + 行 4 + 模板描边 1 = 9 DIP」是主干的两倍多，实测侧留白 8 px = 4 DIP 已对上）；
    /// ② 内缩由 `TEMPLATE_INSET` 一次抵平 **且** content host 走 `VerticalContentAlignment=Stretch`
    /// 且不挂 `ButtonStyle::Subtle`（两条都实测过才写，见下面函数体里的注释），高亮才真铺满行的
    /// UIA 矩形：36 DIP = 200% 下 72 px，选中带与悬停带同源同色同高；
    /// ③ 缩进**只作用于行内容**（`indent` 加在高亮 Border 的左内边距上），不再把整行连同
    /// 高亮带往右推 —— 主干子代理行的 `row.Margin=(14,0,0,0)` 挂的也是行内容
    /// （`MainWindow.xaml.cs:3166`），高亮带仍然铺满 item。
    /// 选中时在左缘叠一条 3×16 的强调条（主干用 NavigationView 自带指示器）。
    /// 外壳用 Button 而非 Border 承接点击：主干的行是 NavigationViewItem，UIA 上有
    /// InvokePattern 且可键盘聚焦，自绘 Border 会让自动化测试和读屏都摸不到。
    fn nav_row(
        &self,
        key: &str,
        content: impl Into<View>,
        name: String,
        selected: bool,
        indent: f64,
        message: Msg,
        context: &mut ViewContext<Shell>,
        p: Palette,
    ) -> View {
        let hovered = self.hover.as_deref() == Some(key);
        let entered = key.to_string();
        // **不挂** `ButtonStyle::Subtle`：那颗样式自带的 PointerOver 视觉态会把整格 UIA 矩形
        // 刷成 `SubtleFillColorSecondary`（浅 `#09000000` ⇒ 实测 `#EAEAEA`），于是悬停行上叠出
        // 两层带（模板那层 72 px + 我们这层 48 px），选中行只有我们那层 48 px ⇒
        // 「选中带 47px、悬停带 71px 不同高」的真因就是这一层多出来的模板底。
        // 默认样式 + 下面那组 `ButtonBackground*` 透明覆写才是干净通路（`pill_button` 同一套覆写
        // 实测模板层什么都不画，见 tmp/ui10-bandA-cut-ComposerAddButton-hover.png 圆外无方角光晕）。
        Button::new()
            .resource_overrides(
                ResourceOverrides::new()
                    .set("ButtonBackground", Color::transparent())
                    .set("ButtonBackgroundPointerOver", Color::transparent())
                    .set("ButtonBackgroundPressed", Color::transparent())
                    .set("ButtonBackgroundDisabled", Color::transparent())
                    // 描边靠四态透明画刷抹掉；见 `button_flat` 注释，
                    // `ButtonBorderThemeThickness` 这个键一动就让 WinUI fail-fast（0xC000027B 静默崩）。
                    .set("ButtonBorderBrush", Color::transparent())
                    .set("ButtonBorderBrushPointerOver", Color::transparent())
                    .set("ButtonBorderBrushPressed", Color::transparent())
                    .set("ButtonBorderBrushDisabled", Color::transparent())
                    .set("ButtonPadding", Thickness::uniform(0.0)),
            )
            // 主干 NavigationViewItemButtonMargin = `4,2`：侧留白 4 DIP、上下各 2 DIP，
            // 由这一处独家给（外层面板不再给左右 margin）。
            .margin(th([4.0, 2.0, 4.0, 2.0]))
            .min_width(0.0)
            .min_height(size::NAV_ITEM_MIN_HEIGHT)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .horizontal_content_alignment(HorizontalAlignment::Stretch)
            // 纵向也必须 Stretch：默认 `VerticalContentAlignment=Center` 会让内容按**自身
            // desired 高度**（行里那颗 24 DIP 的 ⋯ 钮 = 48 px）居中，`TEMPLATE_INSET` 那圈负
            // margin 就白抵了 —— 实测带高只有 48 px 而行的 UIA 矩形是 72 px。
            .vertical_content_alignment(VerticalAlignment::Stretch)
            .automation_name(name)
            .automation_id(nav_automation_id(key))
            .on_click(context.message(message))
            .content(
                // 模板那两层内缩（Padding=11,5,11,6 是 StaticResource 样式 setter、BorderThickness 每边 1 DIP）
                // 都由 TEMPLATE_INSET 一次抵掉，自绘的行高亮**铺满**这颗钮的 UIA 矩形：
                // 主干 NavigationViewItem 的高亮带 == 该 item 的 UIA 矩形，侧留白由行自己的 margin 给。
                Grid::new()
                    .margin(th(TEMPLATE_INSET))
                    // 兜第二道：host 万一不认 `VerticalContentAlignment`，行的 desired 高度
                    // 也至少等于主干的 36 DIP，带不会被行里那颗 24 DIP 的 ⋯ 钮拽矮。
                    .min_height(size::NAV_ITEM_MIN_HEIGHT)
                    .children((
                        // 行内容的左内缩 = 基础 12 DIP + `indent`（子代理 14、组内 20）：
                        // 缩进加在内容上、高亮带仍铺满整行，与主干 `row.Margin=(14,0,0,0)` 同形。
                        Border::new()
                            .padding(th([12.0 + indent, 0.0, 14.0, 0.0]))
                            .corner_radius(radius::SMALL)
                            .background(if selected || hovered {
                                p.subtle_hover
                            } else {
                                p.transparent
                            })
                            .on_pointer_entered(
                                context.callback(move |_| Msg::Hover(Some(entered.clone()))),
                            )
                            .on_pointer_exited(context.callback(|_| Msg::Hover(None)))
                            .content(content),
                        Border::new()
                            .width(size::NAV_INDICATOR_WIDTH)
                            .height(size::NAV_INDICATOR_HEIGHT)
                            .corner_radius(size::NAV_INDICATOR_RADIUS)
                            .background(p.accent)
                            .horizontal_alignment(HorizontalAlignment::Left)
                            .vertical_alignment(VerticalAlignment::Center)
                            .opacity(if selected { 1.0 } else { 0.0 }),
                    )),
            )
    }

    fn nav_item(
        &self,
        key: &str,
        label: String,
        code: char,
        selected: bool,
        message: Msg,
        context: &mut ViewContext<Shell>,
        p: Palette,
    ) -> View {
        let name = match key {
            // 主干这行 Content 是「新会话」，AutomationProperties.Name 却是「新建会话」。
            "new-session" => self.catalog.l("新建会话"),
            _ => label.clone(),
        };
        let content = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(10.0)
            .children((
                // 主干 NavigationViewItem.Icon 不写 FontSize，但模板把图标夹在
                // `Viewbox Height=16` 里 ⇒ 实际墨迹 16，不是 FontIcon 默认的 20。
                mark_vcenter(code, size::GLYPH_NAV),
                TextBlock::new()
                    .text(label)
                    .font_size(type_ramp::BODY.size)
                    .font_weight(if selected {
                        FontWeight::SEMI_BOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .foreground(p.text_primary)
                    .vertical_alignment(VerticalAlignment::Center),
            ));
        // 固定项/分组头的缩进为 0：侧留白 4 DIP 已由 `nav_row` 自己的 margin 给（主干同形）。
        self.nav_row(key, content, name, selected, 0.0, message, context, p)
    }

    /// 主干 `MakeSessionItem()`：`[*, Auto, Auto]`、列距 6；子代理按 `origin` 判定，缩进 14 并加 `↳ `。
    fn session_row(
        &self,
        row: &SessionInfo,
        selected: bool,
        nested: bool,
        context: &mut ViewContext<Shell>,
        p: Palette,
    ) -> View {
        let is_child = row.is_subagent();
        let running = self.is_running(&row.id);
        let title = self.row_title(row);
        let shown = if is_child {
            format!("{} · 子代理", title)
        } else {
            title.clone()
        };
        let id = row.id.clone();
        // 那颗 ⋯ 钮自己也是一颗圆底钮。它**必须**走 `HoverSlot::Pill`（自己的那一格），
        // 不能和行高亮抢 `self.hover` 那一格：行 Border 是它的祖先，指针进入时先进钮、
        // 再冒泡进行 ⇒ 同一个槽里 `Msg::Hover(row.id)` 永远后到，钮的 `hover` 档被行抢走，
        // ⋯ 的圆底在行内**一辈子画不出来**（`ui8-shot` 对它报 NO-TRACE：三态里只有 rest）。
        // 反过来，行高亮要同时认 `pill_hover` 那颗钮的键，否则「进 ⋯ → 行判定为未悬停
        // → ⋯ 收起 → 又判定为悬停」会在两颗元素之间来回抖。
        let more_key = format!("session-more-{}", row.id);
        let hovered = self.hover.as_deref() == Some(row.id.as_str())
            || self.pill_hover.as_deref() == Some(more_key.as_str());
        // 主干 `MainWindow.xaml.cs` 的行内「…」钮（3252-3270 行）：24×24、`Padding=2`、
        // `Background=Transparent`、`BorderThickness=0`、`Opacity=0`（靠 `row.PointerEntered/Exited`
        // 在 3274/3275 行开合），**并且写了圆角**：
        //     CornerRadius rad = new(TokenDouble("RadiusPill", 24));   // 3269-3270 行
        // ⇒ 24 DIP 半径夹到边长一半 = 12 DIP，是一颗 **24 DIP 正圆**（200% 缩放 48 px 直径），
        // 不是小圆角方（`tokens.rs::ROW_MORE_BUTTON` 记着同一条）。`radius::PILL` 就是 24。
        // 无描边 / 圆角 / 四态底色全部走 `pill_button`（`button_ghost` 注释记着两条 fail-fast 成因）。
        let more: View = pill_button(
            Button::new()
                .grid_column(2)
                .opacity(if hovered || selected { 1.0 } else { 0.0 })
                .automation_id(format!("SessionMore_{}", row.id))
                .automation_name(self.catalog.lf("会话操作：{0}", &[title.clone()])),
            mark_size(glyph::MORE_12, size::GLYPH_CAPTION),
            Pill {
                side: size::ROW_MORE_BUTTON,
                radius: radius::PILL,
                // 主干靠 Opacity=0 隐藏，但隐藏态必须不可命中，否则 UIA 仍能 Invoke。
                enabled: hovered || selected,
                ..Pill::ghost(p)
            },
            &more_key,
            HoverSlot::Pill,
            self.pill_hover.as_deref(),
            self.pressed.as_deref(),
            context,
        );
        let content = Grid::new()
            .columns([GridLength::STAR, GridLength::Auto, GridLength::Auto])
            .column_spacing(6.0)
            .children((
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .children((
                        // 运行点只在 `$events` 报 true 时出现；不运行时宽度归零，不留 2px 缝。
                        Border::new()
                            .width(if running { size::STATUS_DOT } else { 0.0 })
                            .height(size::STATUS_DOT)
                            .corner_radius(size::STATUS_DOT / 2.0)
                            .background(p.info)
                            .margin(th([0.0, 0.0, if running { 2.0 } else { 0.0 }, 0.0]))
                            .vertical_alignment(VerticalAlignment::Center),
                        TextBlock::new()
                            .text(if is_child {
                                format!("↳ {title}")
                            } else {
                                title.clone()
                            })
                            .font_size(type_ramp::BODY.size)
                            .font_weight(if selected {
                                FontWeight::SEMI_BOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .foreground(p.text_primary)
                            .text_trimming(TextTrimming::CharacterEllipsis)
                            .vertical_alignment(VerticalAlignment::Center)
                            .margin(th([0.0, 0.0, 13.0, 0.0])),
                    )),
                TextBlock::new()
                    .text(relative_time(row.updated_at, &self.catalog))
                    .font_size(type_ramp::CAPTION.size)
                    .foreground(p.text_tertiary)
                    .vertical_alignment(VerticalAlignment::Center)
                    .grid_column(1),
                more,
            ));
        self.nav_row(
            &id,
            content,
            shown,
            selected,
            // 缩进加在行内容上（主干子代理 `row.Margin=(14,0,0,0)`、组内再缩一级），
            // 顶格行给 0 ⇒ 高亮带一律铺满整行、侧留白恒为 4 DIP。
            if is_child {
                size::SUBAGENT_INDENT
            } else if nested {
                size::NAV_GROUP_INDENT
            } else {
                0.0
            },
            Msg::Select(row.id.clone()),
            context,
            p,
        )
    }

    /// ChatPage：行 `[*, Auto]`；空态只有 34px 标记 + 28px 标题两件子元素。
    fn chat_page(&self, context: &mut ViewContext<Shell>, p: Palette) -> View {
        let empty_state = StackPanel::new()
            .orientation(Orientation::Vertical)
            .spacing(12.0)
            .horizontal_alignment(HorizontalAlignment::Center)
            .vertical_alignment(VerticalAlignment::Center)
            .children((
                brand_mark(self.brand(), size::EMPTY_STATE_MARK, ""),
                TextBlock::new()
                    .text(self.catalog.l("开始一段新的会话"))
                    .font_size(type_ramp::TITLE.size)
                    .font_weight(FontWeight::SEMI_BOLD)
                    .foreground(p.text_primary)
                    .text_wrapping(TextWrapping::Wrap)
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .automation_name("开始一段新的会话"),
            ));
        let stream = ScrollViewer::new()
            .margin(th(pad::CHAT_COLUMN))
            .content(self.message_stream(p, context));
        // 主线：`ChatList` 常驻，`ChatHero` 只在 `IsHeroState() && KernelBootPanel 不亮` 时盖上去
        // （`UpdateEmptyState` MW:4376-4388；`ShowKernelBootPanel` KC:66「加载态不摆开始一段新的会话」）。
        // 分叉这一格是二选一的 keyed 槽，所以把那条与条件直接抄成 `hero` 取反：
        // 加载卡亮着时空态一定让位，而卡片**不铺满、无背景**，底下 `ChatList` 照常可交互
        // （一次性提示那条 system 行就得画在它上面，否则 §4 的气泡根本没地方显示）。
        let hero = self.hero_state() && !self.boot.showing;
        let mut cells: Vec<KeyedView> = vec![
            KeyedView::new(
                "stream",
                self.either(
                    !hero,
                    ("stream-live", stream.into()),
                    ("stream-empty", empty_state.into()),
                ),
            ),
            KeyedView::new("composer", self.composer(context, p, 1)),
        ];
        // 主线 `KernelBootPanel`（MX:691-709）就在 `ChatViewport` 的 Row 1、与 ChatList/ChatHero
        // **同格叠放**，后入者在上 ⇒ 这张卡排在 `"stream"` 之后进树，且**只占内容面正中**。
        if self.boot.showing {
            cells.push(KeyedView::new("boot", self.kernel_boot_panel(context, p)));
        }
        // 主干 `CommandPalette`（MX:745-809）与 `ChatList`/`KernelBootPanel` 同格（ChatPage Row 1）、
        // 贴底（`VerticalAlignment=Bottom`）叠放，声明在最后 ⇒ 层级最高。分叉这一格同样是
        // 「后入者在上」，所以它排在 `stream`/`boot` 之后（见上面那条 keyed 槽位注释）。
        if self.palette.showing {
            cells.push(KeyedView::new("palette", self.command_palette(context, p)));
        }
        Grid::new()
            .rows([GridLength::STAR, GridLength::Auto])
            .keyed_children(cells)
    }

    /// 主干 `CommandPalette`（MX:745-809）：一张贴在内容面底部的浮层卡片。
    /// 主干那段 XAML 的注释就是这里的实现口径——「用普通元素做浮层而不是 Flyout/Popup：浮层不能
    /// 抢焦点，键盘全部由 InputBox 处理，鼠标点选后把焦点还给 InputBox」⇒ 分叉这里连一颗
    /// 可获得焦点的控件都不放（行是自绘 `Border`），按键走 `keys` 那条钩子通路。
    /// 几何逐属性照抄：`MaxWidth=640` / `MaxHeight=264` / `Margin=OverlayMargin(12,0,12,8)` /
    /// `CardBrush` 底 + `StrokeBrush` 1px 描边 / `CornerMedium=8`；内部两行 `[Auto, *]` =
    /// 标题（`HintTextStyle` + `OverlayHeaderMargin`）与列表（`MaxHeight=216` + `OverlayListPadding`）。
    ///
    /// 显隐：reactor 没有 `Visibility` ⇒ 整棵由 `chat_page` 那个 keyed 槽位增删；
    /// 「目录取不到」那一态（MW:17708-17714）列表区收起、只留标题那行原因，同样用槽位表达。
    fn command_palette(&self, context: &mut ViewContext<Shell>, p: Palette) -> View {
        let header_text = self.palette.header(&self.catalog);
        let header = TextBlock::new()
            .text(header_text.clone())
            .font_size(type_ramp::CAPTION.size)
            .foreground(p.text_tertiary.clone())
            .margin(th(pad::OVERLAY_HEADER))
            .grid_row(0)
            .automation_id("CommandPaletteHeader")
            .automation_name(header_text);
        let mut rows: Vec<KeyedView> = Vec::new();
        if self.palette.list_visible {
            for (index, command) in self.palette.matches.iter().enumerate() {
                let row: View = self.palette_row(index, command, p, context);
                rows.push(KeyedView::new(format!("command-{index}"), row));
            }
        }
        let list = ScrollViewer::new()
            .grid_row(1)
            .max_height(PALETTE_LIST_MAX_HEIGHT)
            .automation_id("CommandPaletteList")
            .content(
                // 主干 Padding 挂在 ListView 上；ScrollViewer 没有 padding ⇒ 内衬挪到里面这层
                // Border，量出来的行位置一致（4,0,4,6）。
                Border::new()
                    .padding(th(pad::OVERLAY_LIST))
                    .content(StackPanel::new().keyed_children(rows)),
            );
        Border::new()
            .grid_row(0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Bottom)
            .max_width(PALETTE_MAX_WIDTH)
            .max_height(PALETTE_MAX_HEIGHT)
            .margin(th(pad::OVERLAY))
            .background(p.card.clone())
            .border_brush(p.stroke.clone())
            .border_thickness(size::STROKE)
            .corner_radius(radius::MEDIUM)
            .automation_id("CommandPalette")
            .content(Grid::new().rows([GridLength::Auto, GridLength::STAR]).children((header, list)))
            .into()
    }

    /// 主干 `CommandList` 的一行（`ItemContainerStyle` + `DataTemplate`，MX:779-806）：
    /// 行容器 `Padding=8,5,8,5` / `MinHeight=0` / 内容横向 Stretch；三列 `[Auto, *, Auto]`、
    /// 列距 `Space10=10` = `/名字`（`CodeTextStyle` 12，不换行）+ 描述（`CaptionTextStyle` 12 +
    /// `TextSecondaryBrush` + `CharacterEllipsis`）+ `input.hint`（`HintTextStyle` 12 + Tertiary）。
    /// 选中档 = 主干 ListViewItem 的 Selected 底色，这里用分叉既有的行级提亮画刷（与左栏行同源），
    /// 悬停档同理。行的 UIA Name 照主干 `CommandVm.ToString()`：有描述就 `/名字  描述`（两个空格）。
    ///
    /// 为什么是 `Border` 而不是 `Button`/`ListViewItem`：reactor 没有 `ItemContainerStyle` 那一层，
    /// 默认容器模板的 Padding(11,5,11,6) 与最小高会把主干钉死的 26 DIP 行撑回 40；
    /// `CodeTextStyle` 的 `FontFamily=Consolas…` 也抄不过来（`TextBlock` 公开面没有字体族属性，
    /// 见 `settings_code_text` 同一处偏差）。
    fn palette_row(
        &self,
        index: usize,
        command: &CommandEntry,
        p: Palette,
        context: &mut ViewContext<Shell>,
    ) -> View {
        let key = format!("palette-row-{index}");
        let selected = self.palette.index >= 0 && self.palette.index as usize == index;
        let hovered = self.hover.as_deref() == Some(key.as_str());
        let name = command.slash_name();
        let automation_name = if command.description.is_empty() {
            name.clone()
        } else {
            format!("{name}  {}", command.description)
        };
        let description = command.description.clone();
        let hint = command.hint.clone();
        let entered = key;
        Border::new()
            .padding(th(PALETTE_ROW_PADDING))
            .background(if selected || hovered {
                p.subtle_hover.clone()
            } else {
                p.transparent.clone()
            })
            .on_pointer_entered(context.callback(move |_| {
                Msg::Hover(Some(entered.clone()))
            }))
            .on_pointer_exited(context.callback(|_| Msg::Hover(None)))
            .on_pointer_released(context.callback(move |_| Msg::PalettePick(index)))
            .automation_name(automation_name)
            .content(
                Grid::new()
                    .columns([GridLength::Auto, GridLength::STAR, GridLength::Auto])
                    .column_spacing(space::S10)
                    .children((
                        TextBlock::new()
                            .grid_column(0)
                            .text(name)
                            .font_size(type_ramp::CODE.size)
                            .foreground(p.text_primary.clone())
                            .text_wrapping(TextWrapping::NoWrap)
                            .vertical_alignment(VerticalAlignment::Center),
                        TextBlock::new()
                            .grid_column(1)
                            .text(description)
                            .font_size(type_ramp::CAPTION.size)
                            .foreground(p.text_secondary.clone())
                            .text_trimming(TextTrimming::CharacterEllipsis)
                            .text_wrapping(TextWrapping::NoWrap)
                            .vertical_alignment(VerticalAlignment::Center),
                        TextBlock::new()
                            .grid_column(2)
                            .text(hint)
                            .font_size(type_ramp::CAPTION.size)
                            .foreground(p.text_tertiary.clone())
                            .text_wrapping(TextWrapping::NoWrap)
                            .vertical_alignment(VerticalAlignment::Center),
                    )),
            )
            .into()
    }

    /// 主干 `ChatList`：渲染 `_messages`，末尾接在途的 live 气泡与「少女祈祷中」占位行。
    /// 行距来自 `ListViewItem Margin=0,3`（逐条挂在气泡上），所以面板本身不给 spacing。
    fn message_stream(&self, p: Palette, context: &mut ViewContext<Shell>) -> View {
        let mut lines: Vec<KeyedView> = self
            .bubbles
            .iter()
            .rev()
            .take(200)
            .rev()
            .map(|bubble| {
                let answer = self.is_turn_answer(bubble);
                KeyedView::new(
                    bubble_key(bubble),
                    self.bubble_view(bubble, answer, !self.is_open_turn(bubble.turn), p, context),
                )
            })
            .collect();
        if let Some(live) = &self.live {
            let bubble = Bubble::new("assistant", live.text.clone(), live.turn, -1);
            lines.push(KeyedView::new(
                "live",
                self.bubble_view(&bubble, false, false, p, context),
            ));
        }
        if self.thinking {
            lines.push(KeyedView::new("pending", self.pending_row(p)));
        }
        StackPanel::new()
            .orientation(Orientation::Vertical)
            .keyed_children(lines)
    }

    /// 该轮最后一条 assistant 正文（主干 `_transcriptAnswers`）：只有它能分支、才显用时。
    fn is_turn_answer(&self, bubble: &Bubble) -> bool {
        bubble.role == "assistant"
            && bubble.seq > 0
            && !self.bubbles.iter().any(|other| {
                other.role == "assistant" && other.turn == bubble.turn && other.seq > bubble.seq
            })
    }

    /// 主干 `ProducedPathsForTurn`：本轮产出按到达顺序去重（只留首现），并截到答案气泡自己的
    /// `seq`——答案之后才落的结果不属于这条回答。
    fn produced_paths(&self, turn: i64, closing_seq: i64) -> Vec<String> {
        if turn <= 0 {
            return Vec::new();
        }
        let Some((_, rows)) = self.produced.iter().find(|(id, _)| *id == turn) else {
            return Vec::new();
        };
        let mut paths: Vec<String> = Vec::new();
        for (seq, path) in rows {
            if (closing_seq > 0 && *seq > closing_seq) || paths.iter().any(|keep| keep == path) {
                continue;
            }
            paths.push(path.clone());
        }
        paths
    }

    /// 主干 `RefreshProducedChips`：产出晚于答案落地（或 `turn/end` 收尾）时重算那一行的 chips。
    fn sync_produced(&mut self, turn: i64) {
        let Some(index) = self.bubbles.iter().rposition(|bubble| {
            bubble.turn == turn && bubble.role == "assistant" && bubble.seq > 0
        }) else {
            return;
        };
        let closing = self.bubbles[index].seq;
        let paths = self.produced_paths(turn, closing);
        self.bubbles[index].produced = paths;
    }

    /// 主干 `_closedTranscriptTurns` 的反面：`turn/start` 进表、`turn/end` 摘表，还在表里
    /// 说明这一轮没收尾，分支钮得灰着。
    fn is_open_turn(&self, turn: i64) -> bool {
        self.turn_starts.iter().any(|(id, _)| *id == turn)
    }

    /// 主干 `HintTextStyle`（caption 12 + TextTertiary）：操作行的时钟/型号/用量/用时都走它。
    fn hint(&self, text: String, p: Palette) -> View {
        TextBlock::new()
            .text(text)
            .font_size(type_ramp::CAPTION.size)
            .foreground(p.text_tertiary.clone())
            .vertical_alignment(VerticalAlignment::Center)
            .into()
    }

    /// 主干操作行那颗钮（`MakeCopyButton` / `MakeWithdrawEditButton`，
    /// `MainWindow.MessageActions.cs`）：`SmallButtonSize(28)` 正方 + `MinWidth=0` +
    /// `Padding=0` + `CornerRadius=RadSmall(4)` + 字形 `GlyphBody(14)`。
    /// **是小圆角方，不是正圆**（4 远小于短边一半 14 ⇒ `radius_clamped` 原样传下去），
    /// 也必须走 `pill_button` 外壳：裸 Button 的模板描边与默认 4px 底在分叉里渲成
    /// 「带 1px 描边的方角默认按钮」，而且 UIA 量到 56×53px（=28×26.5 DIP）非正方
    /// —— 模板 `MinHeight=32` 没被抵掉，得靠外壳的 `.min_height(0.0)`。
    fn icon_button(
        &self,
        code: char,
        id: &str,
        name: &str,
        tip: &str,
        enabled: bool,
        key: &str,
        message: Msg,
        p: Palette,
        context: &mut ViewContext<Shell>,
    ) -> View {
        let tip = self.catalog.l(tip);
        // 同一张卡里两颗钮的悬停键得互不相同 ⇒ 键由调用方带上气泡 key 传进来（`tooltip`
        // 收尾同 `feedback_toggle`：链上先出钮再补 tip）。
        pill_button(
            Button::new()
                .on_click(context.message(message))
                .automation_id(id.to_string())
                .automation_name(self.catalog.l(name)),
            mark_size(code, size::GLYPH_BODY),
            Pill::action(p, enabled),
            key,
            HoverSlot::Pill,
            self.pill_hover.as_deref(),
            self.pressed.as_deref(),
            context,
        )
        .tooltip(tip)
    }

    /// 主干轮尾的用量段：E81E（12px、.8）+「用量 X tok」；内核没回 usage 时整段不出现。
    fn tokens_group(&self, bubble: &Bubble, p: Palette) -> View {
        StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(space::S6)
            .vertical_alignment(VerticalAlignment::Center)
            .children((
                Border::new()
                    .opacity(0.8)
                    .content(mark_size(glyph::USAGE, size::GLYPH_CAPTION)),
                self.hint(
                    self.catalog
                        .lf("用量 {0} tok", &[tokens_text(bubble.tokens)]),
                    p,
                ),
            ))
    }

    /// 反馈组的一行（主干 `BuildFeedbackRow` 的行首六件套，`MainWindow.xaml.cs:16569`）：
    /// **Like → Dislike → Note → NoteText → Revoke → Status**，全从 `self.feedback` 现算。
    /// 六个状态各自怎么落：
    /// · Like/Dislike：**真的 `ToggleButton`**（`feedback_toggle`），`rating` 命中那颗
    ///   `IsChecked=true` ⇒ 模板自己画强调色选中档，另一颗 `false` ⇒ 默认 ControlFill 静置档；
    ///   再点同一颗 = 原生翻回 false → `set_feedback_rating` 走 delete（撤回），
    ///   点另一颗 = true → put 改判且把旧 note 带上（互斥切换）。UIA 那侧因此带 Toggle 模式。
    /// · Note：主干只在「本条已有反馈」时 Visible，标签按有无 note 在「添加说明/编辑说明」间切；
    ///   点开即 `Msg::OpenNote` → `note_for` 置位 → 气泡下方长出行内编辑器。
    /// · NoteText：有 note 才出现的一行摘要（`说明：{0}`，MaxWidth 420、省略号截断）。
    /// · Revoke：有反馈才出现的「撤销」，走 `Msg::Revoke` → `delete_feedback`。
    /// · Status：`feedback_status` 里有文案才出现（put/delete 的业务错误、冲突重试都落这里）。
    /// 主干 `ApplyFeedbackOpacity`：本条已有反馈时**整行**常驻 1.0，不等悬停。
    fn feedback_cells(
        &self,
        bubble: &Bubble,
        p: Palette,
        context: &mut ViewContext<Shell>,
    ) -> Vec<View> {
        let message = bubble.id.as_str();
        let item = feedback_of(&self.feedback, message);
        let rating = item
            .as_ref()
            .map(|item| item.rating.as_str())
            .unwrap_or_default();
        let note = item
            .as_ref()
            .and_then(|item| item.note.clone())
            .filter(|note| !note.is_empty());
        let mut cells = vec![
            self.feedback_toggle(
                glyph::THUMB_UP,
                "positive",
                "FeedbackLike",
                "有帮助（再点一次撤销）",
                message,
                rating == "positive",
                context,
            ),
            self.feedback_toggle(
                glyph::THUMB_DOWN,
                "negative",
                "FeedbackDislike",
                "没帮助（再点一次撤销）",
                message,
                rating == "negative",
                context,
            ),
        ];
        if item.is_some() {
            cells.push(self.feedback_text_button(
                if note.is_some() { "编辑说明" } else { "添加说明" },
                &format!("FeedbackNote_{message}"),
                Msg::OpenNote(message.to_string()),
                context,
            ));
        }
        if let Some(note) = note {
            cells.push(
                TextBlock::new()
                    .text(self.catalog.lf("说明：{0}", &[note]))
                    .font_size(type_ramp::CAPTION.size)
                    .foreground(p.text_tertiary)
                    .max_width(size::FEEDBACK_NOTE_MAX_WIDTH)
                    .text_wrapping(TextWrapping::NoWrap)
                    .text_trimming(TextTrimming::CharacterEllipsis)
                    .vertical_alignment(VerticalAlignment::Center)
                    .into(),
            );
        }
        if item.is_some() {
            cells.push(self.feedback_text_button(
                "撤销",
                &format!("FeedbackRevoke_{message}"),
                Msg::Revoke(message.to_string()),
                context,
            ));
        }
        let status = pair_value(&self.feedback_status, message, String::new());
        if !status.is_empty() {
            cells.push(self.hint(status, p));
        }
        cells
    }

    /// 主干 `MakeFeedbackToggle`（`MainWindow.xaml.cs`）：那颗赞/踩是**裸 `ToggleButton`** ——
    /// `Width=Height=SmallButtonSize(28)` + `MinWidth=0` + `Padding=0` +
    /// `CornerRadius=RadSmall(4)` + 字形 `GlyphBody(14)`，四态与选中态全交给 WinUI 的
    /// `DefaultToggleButtonStyle`（generic.xaml:544-556：静置 `ControlFillColorDefault`、
    /// 悬停 Secondary、按下 Tertiary、禁用 Disabled、**Checked = AccentFillColorDefault +
    /// 反白字形**），并且对 UIA 暴露 **Toggle 模式**。
    /// 上一版分叉用普通 `Button` + `Pill::feedback` 自绘 ⇒ 静置整块透明、也没有 Toggle 模式。
    /// 这一版改回 reactor 的 `ToggleButton`（`generated.rs:2297`：只有 `is_checked` /
    /// `is_enabled` / `on_is_checked_changed` 三个自有 setter）：
    /// · 它**没有** `resource_overrides` —— 正好，这一族要的就是模板那套原生状态层，
    ///   不需要钉透明，也天然避开「`.style(Accent)` 与资源覆盖同挂一钮」那族 fail-fast。
    /// · 它也设不了 `CornerRadius`/`Padding` ⇒ 圆角吃模板默认 `ControlCornerRadius=4`
    ///   （generic.xaml:2324/7878，**与主干 RadSmall 同值**，不用另设），默认 `Padding`
    ///   11,5,11,6 用负 margin 抵掉（不抵 28 的钮只剩 6×17 内容区，字形成一条横线）。
    /// · 事件口径与幂等守卫见 `Shell::set_feedback_rating`。
    fn feedback_toggle(
        &self,
        code: char,
        rating: &str,
        prefix: &str,
        tip: &str,
        message: &str,
        checked: bool,
        context: &mut ViewContext<Shell>,
    ) -> View {
        let tip = self.catalog.l(tip);
        let (toggled_message, toggled_rating) = (message.to_string(), rating.to_string());
        ToggleButton::new()
            .width(size::SMALL_BUTTON)
            .height(size::SMALL_BUTTON)
            .min_width(0.0)
            .min_height(0.0)
            .vertical_alignment(VerticalAlignment::Center)
            .is_checked(checked)
            .on_is_checked_changed(context.callback(move |on: bool| {
                Msg::Rate(toggled_message.clone(), toggled_rating.clone(), on)
            }))
            .automation_id(format!("{prefix}_{message}"))
            .automation_name(tip.clone())
            .content(
                Border::new()
                    .margin(th(TEMPLATE_INSET))
                    .content(mark_cell(code, size::GLYPH_BODY)),
            )
            .tooltip(tip)
    }

    /// 主干 `MakeFeedbackButton`：高 28、`Padding=8,0,8,0`、`CornerRadius=RadSmall(4)`、
    /// **不设字号**（= Body 14）的**文字**钮（「添加说明 / 编辑说明 / 撤销」），而且它是
    /// 裸 `Button`：没写 `Background`/`BorderThickness` ⇒ 静置 `ControlFillColorDefault` +
    /// 1px `ControlElevationBorderBrush` 描边全由默认模板给（`ControlCornerRadius` 默认就是
    /// 4，与主干 RadSmall 同值）。没进 `pill_button`：那颗把钮钉成正方形，文字钮要按内容
    /// 自适应宽度；模板那套本来就对，自绘反而会错。
    /// 负 margin 那一步仍按 `pill_button` 的口径来，把模板 `Padding=11,5,11,6` 抵成主干的
    /// `8,0,8,0`，否则文字区只剩 6×17。
    fn feedback_text_button(
        &self,
        label: &str,
        id: &str,
        message: Msg,
        context: &mut ViewContext<Shell>,
    ) -> View {
        let label = self.catalog.l(label);
        Button::new()
            .height(size::SMALL_BUTTON)
            .min_width(0.0)
            .vertical_alignment(VerticalAlignment::Center)
            .automation_id(id.to_string())
            .automation_name(label.clone())
            .on_click(context.message(message))
            .content(
                Border::new()
                    .margin(th([
                        size::FEEDBACK_BUTTON_PADDING - 11.0,
                        -5.0,
                        size::FEEDBACK_BUTTON_PADDING - 11.0,
                        -6.0,
                    ]))
                    .padding(th([size::FEEDBACK_BUTTON_PADDING, 0.0, size::FEEDBACK_BUTTON_PADDING, 0.0]))
                    .content(
                        TextBlock::new()
                            .text(label)
                            // 主干 `MakeFeedbackButton` **不设字号** ⇒ 吃 Button 默认的
                            // `ControlContentThemeFontSize` = 14（generic.xaml:5590，Body 档），
                            // 不是 caption 12。上一版按 CAPTION 排，墨迹高实测只有 23px≈12DIP。
                            .font_size(type_ramp::BODY.size)
                            .vertical_alignment(VerticalAlignment::Center),
                    ),
            )
    }

    /// 便签编辑器：主干是 `ContentDialog`（`EditFeedbackNoteAsync`，`xaml.cs:17160`）里的一个
    /// `AcceptsReturn` 多行 TextBox + 保存/取消；分叉没有 ContentDialog ⇒ 展开在气泡下方，
    /// 形态仍是「多行输入 + 保存/取消 + 字数上限提示」。
    /// 上限按内核 `maxNoteBytes = 8192`（假内核回 `note-too-large{maxBytes,actualBytes}` 同值），
    /// **按 UTF-8 字节**算：超了先把「说明超长（上限 N 字节）」摆出来，保存不本地拦死，
    /// 仍让内核回业务错误、由行内 Status 落话（口径与主干一致）。
    fn note_editor(&self, p: Palette, context: &mut ViewContext<Shell>) -> View {
        let bytes = self.note_text.len() as i64;
        let over = bytes > MAX_NOTE_BYTES;
        let save = |label: &str, id: &str, accent: bool, column: i32, message: Msg| {
            let label = self.catalog.l(label);
            Button::new()
                // 强调色钮：`.style(ButtonStyle::Accent)` 只有在不和 `resource_overrides` 同挂
                // 一个 Button 时才安全（见文件头那条 fail-fast 复盘），这里两个都不碰资源。
                .style(if accent {
                    ButtonStyle::Accent
                } else {
                    ButtonStyle::Default
                })
                .min_height(size::COMPACT_BUTTON_MIN_HEIGHT)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(column)
                .automation_id(id.to_string())
                .automation_name(label.clone())
                .on_click(context.message(message))
                .content(
                    TextBlock::new()
                        .text(label)
                        .font_size(type_ramp::CAPTION.size),
                )
        };
        Grid::new()
            .rows([GridLength::Auto, GridLength::Auto])
            .row_spacing(space::S6)
            .margin(th([0.0, space::S4, 0.0, 0.0]))
            .automation_id("FeedbackNoteEditor")
            // 容器**不写** UIA Name：主干 `Aut(box, "FeedbackNoteTextBox", "反馈说明")` 里
            // 「反馈说明」这个名字属于那颗多行输入框，驱动脚本按 Name 取值，重名会抢错元素。
            .keyed_children(
                vec![
                    KeyedView::new(
                        "box",
                        TextBox::new()
                            .grid_row(0)
                            .text(self.note_text.clone())
                            .accepts_return(true)
                            .text_wrapping(TextWrapping::Wrap)
                            .min_height(size::FEEDBACK_NOTE_EDITOR_HEIGHT)
                            .max_height(size::FEEDBACK_NOTE_EDITOR_HEIGHT)
                            .placeholder_text(
                                self.catalog.l("这条回复哪里好 / 哪里不好（可选，纯文本）"),
                            )
                            .automation_id("FeedbackNoteTextBox")
                            .automation_name(self.catalog.l("反馈说明"))
                            .on_text_changed(context.callback(|text: String| Msg::NoteText(text))),
                    ),
                    KeyedView::new(
                        "row",
                        Grid::new()
                            .grid_row(1)
                            .columns([GridLength::STAR, GridLength::Auto, GridLength::Auto])
                            .column_spacing(space::S6)
                            .children((
                                StackPanel::new()
                                    .orientation(Orientation::Vertical)
                                    .grid_column(0)
                                    .vertical_alignment(VerticalAlignment::Center)
                                    .children((
                                        TextBlock::new()
                                            .text(self.catalog.lf(
                                                "{0}/{1} 字节",
                                                &[bytes.to_string(), MAX_NOTE_BYTES.to_string()],
                                            ))
                                            .font_size(type_ramp::CAPTION.size)
                                            .foreground(p.text_tertiary),
                                        if over {
                                            TextBlock::new()
                                                .text(self.catalog.lf(
                                                    "说明超长（上限 {0} 字节）",
                                                    &[MAX_NOTE_BYTES.to_string()],
                                                ))
                                                .font_size(type_ramp::CAPTION.size)
                                                .foreground(p.error)
                                                .into()
                                        } else {
                                            blank()
                                        },
                                    )),
                                save("保存", "FeedbackNoteSaveButton", true, 1, Msg::CloseNote(true)),
                                save(
                                    "取消",
                                    "FeedbackNoteCancelButton",
                                    false,
                                    2,
                                    Msg::CloseNote(false),
                                ),
                            )),
                    ),
                ]
                .into_iter()
                .collect::<Vec<KeyedView>>(),
            )
    }

    /// 气泡下的操作行（主干 `BuildFeedbackRow` 那一整行）：
    /// **反馈组（赞/踩/说明/说明摘要/撤销/状态）→ 复制 → 分支 → 时钟 → 型号 → 用量 → 用时**。
    /// 静置 0.35（用户行 0.62 且右对齐），指针进来或本条已有反馈时提到 1。
    fn bubble_actions(
        &self,
        bubble: &Bubble,
        is_answer: bool,
        closed: bool,
        p: Palette,
        context: &mut ViewContext<Shell>,
    ) -> View {
        let key = bubble_key(bubble);
        let user = bubble.role == "user";
        let copied = self.copied.as_deref() == Some(key.as_str());
        let lit = self.hover.as_deref() == Some(key.as_str());
        let mut cells: Vec<View> = Vec::new();
        if user {
            if bubble.time > 0 {
                cells.push(self.hint(message_clock(bubble.time), p));
            }
            if !bubble.model.is_empty() {
                cells.push(self.hint(format!("· {}", bubble.model), p));
            }
        }
        // 主干 `BuildFeedbackRow`：反馈组要按内核 `message.id` 寻址，无 id 的气泡（错误占位、
        // 用户行、工具/思考/交付物行）整组不出现；这一组排在复制之前（主干同一行的行首）。
        let feedback = !user && !bubble.id.is_empty();
        if feedback {
            cells.extend(self.feedback_cells(bubble, p, context));
        }
        // 主干 `ApplyFeedbackOpacity`：本条已有反馈 ⇒ 整行常驻 1.0，不等悬停。
        let rated = feedback && feedback_of(&self.feedback, &bubble.id).is_some();
        cells.push(self.icon_button(
            if copied { glyph::COPIED } else { glyph::COPY },
            "MessageCopyButton",
            "复制消息",
            if copied { "已复制" } else { "复制消息" },
            true,
            &format!("copy-{key}"),
            Msg::Copy(key.clone(), bubble.text.clone()),
            p,
            context,
        ));
        if !user {
            if is_answer && bubble.seq > 0 {
                cells.push(self.icon_button(
                    glyph::OPEN_IN_APP,
                    "MessageBranchButton",
                    "在新对话中分支",
                    if closed {
                        "在新对话中分支"
                    } else {
                        "仅可从已完成轮次的最后一条消息分支"
                    },
                    closed,
                    &format!("branch-{key}"),
                    Msg::Branch(bubble.seq),
                    p,
                    context,
                ));
            }
            if bubble.time > 0 {
                cells.push(self.hint(message_clock(bubble.time), p));
            }
            if !bubble.model.is_empty() {
                cells.push(self.hint(format!("· {}", bubble.model), p));
            }
            if bubble.tokens > 0 {
                cells.push(self.tokens_group(bubble, p));
            }
            if is_answer && bubble.duration_ms > 0 {
                cells.push(
                    self.hint(
                        self.catalog
                            .lf("用时 {0}", &[duration_text(bubble.duration_ms)]),
                        p,
                    ),
                );
            }
        }
        let idle = if user {
            size::ROW_OPACITY_USER
        } else {
            size::ROW_OPACITY_ASSISTANT
        };
        let row = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(space::S4)
            .margin(th([0.0, if user { 0.0 } else { space::S4 }, 0.0, 0.0]))
            .opacity(if lit || rated { 1.0 } else { idle })
            .keyed_children(
                cells
                    .into_iter()
                    .enumerate()
                    .map(|(index, cell)| KeyedView::new(format!("a{index}"), cell))
                    .collect::<Vec<KeyedView>>(),
            );
        // 助手行嵌在气泡 Border 里，主干的 hover 钩子也挂在那层（PointerEntered 会冒泡上来）；
        // 用户行是独立一段，主干把钩子挂在行本身，所以这里补一层 Border 当命中面。
        if !user {
            return row.into();
        }
        Border::new()
            .background(p.transparent)
            .horizontal_alignment(HorizontalAlignment::Right)
            .on_pointer_entered(context.callback(move |_| Msg::Hover(Some(key.clone()))))
            .on_pointer_exited(context.callback(|_| Msg::Hover(None)))
            .content(row)
    }

    /// 主干 `ToolCallTpl` 的行：图标 12/.8 + 标题 + 半个透明度的「·」+ .85 的摘要。
    /// 主干运行态还有一道 LinearGradientBrush 扫光（Storyboard），reactor 没有动画原语，抄不了。
    fn tool_line(&self, bubble: &Bubble, p: Palette) -> View {
        let Some(code) = bubble.icon else {
            // 主干 `ToolTpl`：system/message 那类灰行，没图标、没摘要段、可换行、0.62。
            return Border::new()
                .margin(th(pad::LIST_ITEM))
                .padding(th(pad::BUBBLE_TOOL))
                .content(
                    TextBlock::new()
                        .text(bubble.text.clone())
                        .font_size(type_ramp::CAPTION.size)
                        .foreground(p.text_primary.clone())
                        .opacity(0.62)
                        .text_wrapping(TextWrapping::Wrap)
                        .is_text_selection_enabled(true),
                );
        };
        let mut cells: Vec<View> = vec![
            Border::new()
                .opacity(0.8)
                .content(mark_size(code, size::GLYPH_CAPTION)),
            TextBlock::new()
                .text(self.catalog.l(bubble.text.as_str()))
                .font_size(type_ramp::CAPTION.size)
                .foreground(p.text_primary.clone())
                .vertical_alignment(VerticalAlignment::Center)
                .into(),
        ];
        if !bubble.summary.is_empty() {
            cells.push(
                TextBlock::new()
                    .text("·")
                    .font_size(type_ramp::CAPTION.size)
                    .foreground(p.text_primary.clone())
                    .opacity(0.5)
                    .vertical_alignment(VerticalAlignment::Center)
                    .into(),
            );
            cells.push(
                TextBlock::new()
                    .text(bubble.summary.clone())
                    .font_size(type_ramp::CAPTION.size)
                    .foreground(p.text_primary.clone())
                    .opacity(0.85)
                    .text_wrapping(TextWrapping::NoWrap)
                    .text_trimming(TextTrimming::CharacterEllipsis)
                    .vertical_alignment(VerticalAlignment::Center)
                    .into(),
            );
        }
        Border::new()
            .margin(th(pad::LIST_ITEM))
            .padding(th(pad::BUBBLE_TOOL))
            .content(
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(space::S6)
                    .opacity(0.72)
                    .automation_id("ToolCallRow")
                    .automation_name(self.catalog.l(bubble.text.as_str()))
                    .keyed_children(
                        cells
                            .into_iter()
                            .enumerate()
                            .map(|(index, cell)| KeyedView::new(format!("t{index}"), cell))
                            .collect::<Vec<KeyedView>>(),
                    ),
            )
    }

    /// 主干思考段行（`MessageActions.cs:428`）：`ToggleButton` 头（透明底、无边框）+ 收起态
    /// 摘要，展开才画正文。分叉用 Button 加透明覆盖件复刻头部（ToggleButton 没有背景/内边距
    /// setter，也拿不到 resource_overrides），展开态按气泡键记在 `reasoning_open`。
    fn reasoning_row(&self, bubble: &Bubble, p: Palette, context: &mut ViewContext<Shell>) -> View {
        let key = bubble_key(bubble);
        let open = self.reasoning_open.iter().any(|item| item == &key);
        let first = bubble.text.lines().next().unwrap_or_default().to_string();
        let header = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(space::S6)
            .children((
                mark_size(
                    if open {
                        glyph::THINK_OPEN
                    } else {
                        glyph::CHEVRON_RIGHT
                    },
                    size::GLYPH_MINI,
                ),
                TextBlock::new()
                    .text(self.catalog.l("思考"))
                    .font_size(type_ramp::CAPTION.size)
                    .foreground(p.text_primary.clone())
                    .vertical_alignment(VerticalAlignment::Center),
                TextBlock::new()
                    .text(first)
                    .font_size(type_ramp::CAPTION.size)
                    .foreground(p.text_tertiary.clone())
                    .max_width(480.0)
                    .text_wrapping(TextWrapping::NoWrap)
                    .text_trimming(TextTrimming::CharacterEllipsis)
                    .vertical_alignment(VerticalAlignment::Center),
            ));
        let mut lines: Vec<View> = vec![
            Button::new()
                .min_width(0.0)
                .resource_overrides(
                    // 主干是 ToggleButton{Background=Transparent, BorderThickness=0}。reactor 的
                    // ToggleButton 没暴露这两个 setter，所以退回 Button + 主题资源覆盖；
                    // 「无描边」只能靠四态透明画刷，`ButtonBorderThemeThickness` 是 double 型键，
                    // 用 Thickness 覆它会让 WinUI fail-fast（见 `button_flat` 注释）。
                    ResourceOverrides::new()
                        .set("ButtonBackground", Color::transparent())
                        .set("ButtonBackgroundPointerOver", Color::transparent())
                        .set("ButtonBackgroundPressed", Color::transparent())
                        .set("ButtonBackgroundDisabled", Color::transparent())
                        .set("ButtonBorderBrush", Color::transparent())
                        .set("ButtonBorderBrushPointerOver", Color::transparent())
                        .set("ButtonBorderBrushPressed", Color::transparent())
                        .set("ButtonBorderBrushDisabled", Color::transparent())
                        .set("ControlCornerRadius", CornerRadius::uniform(radius::SMALL)),
                )
                .horizontal_alignment(HorizontalAlignment::Left)
                .horizontal_content_alignment(HorizontalAlignment::Left)
                .automation_id("ReasoningToggle")
                .automation_name(self.catalog.l("思考"))
                .on_click(context.message(Msg::Reason(key.clone(), !open)))
                .content(header)
                .tooltip(self.catalog.l("思考"))
                .into(),
        ];
        if open {
            lines.push(
                TextBlock::new()
                    .text(bubble.text.clone())
                    .font_size(type_ramp::BODY.size)
                    .foreground(p.text_primary.clone())
                    .opacity(0.78)
                    .margin(th([20.0, space::S2, 0.0, 0.0]))
                    .text_wrapping(TextWrapping::Wrap)
                    .is_text_selection_enabled(true)
                    .into(),
            );
        }
        Border::new().margin(th(pad::LIST_ITEM)).content(
            StackPanel::new()
                .orientation(Orientation::Vertical)
                .keyed_children(
                    lines
                        .into_iter()
                        .enumerate()
                        .map(|(index, line)| KeyedView::new(format!("r{index}"), line))
                        .collect::<Vec<KeyedView>>(),
                ),
        )
    }

    /// 主干 `DeliverableTpl`：气泡材质的卡（r=12、padding 12,10）里嵌「交付物」标题 +
    /// 每个文件一张 r=8 的卡。打开/回显走宿主路由 `POST /api/present.open`（见 `present_open`）。
    fn deliverable_card(
        &self,
        bubble: &Bubble,
        p: Palette,
        context: &mut ViewContext<Shell>,
    ) -> View {
        let files: Vec<KeyedView> = bubble
            .files
            .iter()
            .map(|(index, raw, description)| {
                let name = raw
                    .rsplit(['/', '\\'])
                    .next()
                    .filter(|tail| !tail.is_empty())
                    .unwrap_or(raw.as_str())
                    .to_string();
                let path = raw.replace('\\', "/");
                let mut cells: Vec<KeyedView> = vec![
                    KeyedView::new(
                        "name",
                        TextBlock::new()
                            .text(name.clone())
                            .font_size(type_ramp::BODY.size)
                            .foreground(p.text_primary.clone())
                            .text_wrapping(TextWrapping::NoWrap)
                            .text_trimming(TextTrimming::CharacterEllipsis),
                    ),
                    KeyedView::new(
                        "path",
                        TextBlock::new()
                            .text(path.clone())
                            .font_size(type_ramp::CODE.size)
                            .foreground(p.text_primary.clone())
                            .text_wrapping(TextWrapping::NoWrap)
                            .text_trimming(TextTrimming::CharacterEllipsis)
                            .is_text_selection_enabled(true),
                    ),
                ];
                // 主干 DescriptionVisibility：说明为空整行收起，不占位。
                if !description.is_empty() {
                    cells.push(KeyedView::new(
                        "description",
                        self.hint(description.clone(), p),
                    ));
                }
                KeyedView::new(
                    format!("file-{index}"),
                    Border::new()
                        .background(p.card)
                        .border_brush(p.stroke)
                        .border_thickness(th([size::STROKE; 4]))
                        .corner_radius(radius::MEDIUM)
                        .padding(th([10.0, 8.0, 10.0, 8.0]))
                        .margin(th([0.0, 0.0, 0.0, space::S4]))
                        .content(
                            Grid::new()
                                .columns([GridLength::STAR, GridLength::Auto])
                                .column_spacing(space::S10)
                                .children((
                                    StackPanel::new()
                                        .orientation(Orientation::Vertical)
                                        .spacing(space::S2)
                                        .keyed_children(cells),
                                    StackPanel::new()
                                        .orientation(Orientation::Horizontal)
                                        .spacing(space::S6)
                                        .grid_column(1)
                                        .vertical_alignment(VerticalAlignment::Center)
                                        .children((
                                            self.text_button(
                                                "打开",
                                                "PresentedFileOpenButton",
                                                Msg::Present(bubble.seq, *index, "open".into()),
                                                context,
                                            ),
                                            self.text_button(
                                                "显示",
                                                "PresentedFileRevealButton",
                                                Msg::Present(bubble.seq, *index, "reveal".into()),
                                                context,
                                            ),
                                        )),
                                )),
                        ),
                )
            })
            .collect();
        Border::new()
            .margin(th(pad::LIST_ITEM))
            .background(p.bubble)
            .border_brush(p.stroke_subtle)
            .border_thickness(size::STROKE)
            .corner_radius(radius::LARGE)
            .padding(th([12.0, 10.0, 12.0, 10.0]))
            .content(
                StackPanel::new()
                    .orientation(Orientation::Vertical)
                    .spacing(space::S6)
                    .keyed_children(
                        vec![KeyedView::new(
                            "head",
                            StackPanel::new()
                                .orientation(Orientation::Horizontal)
                                .spacing(space::S6)
                                .children((
                                    mark_size(glyph::WORKSPACE, size::GLYPH_BODY),
                                    TextBlock::new()
                                        .text(self.catalog.l("交付物"))
                                        .font_size(type_ramp::CAPTION.size)
                                        .foreground(p.text_primary.clone())
                                        .text_wrapping(TextWrapping::NoWrap)
                                        .vertical_alignment(VerticalAlignment::Center)
                                        .automation_id("DeliverableCardTitle"),
                                )),
                        )]
                        .into_iter()
                        .chain(files)
                        .collect::<Vec<KeyedView>>(),
                    ),
            )
    }

    /// 主干 `MakeProducedFileChip`：文件图标 + 基名，点击经内核 `session/openWorkspacePath`
    /// 交宿主桌面打开。主干是 HyperlinkButton，reactor 的公开构造器没有 content/padding，
    /// 所以换成 TextLink 样式的 Button（外观同族：透明底 + 强调色文字）。
    fn produced_chip(&self, path: &str, context: &mut ViewContext<Shell>) -> View {
        let flattened = path.replace('\\', "/");
        let name = flattened
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or(flattened.as_str())
            .to_string();
        let tip = self.catalog.lf("打开 {0}", &[path.to_string()]);
        Button::new()
            .style(ButtonStyle::TextLink)
            .min_width(0.0)
            .vertical_alignment(VerticalAlignment::Center)
            .automation_id(format!("ProducedFileChip_{name}"))
            .automation_name(tip.clone())
            .on_click(context.message(Msg::OpenPath(path.to_string())))
            .content(
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(space::S6)
                    .vertical_alignment(VerticalAlignment::Center)
                    .children((
                        mark_size(glyph::PRODUCED_FILE, size::GLYPH_CAPTION),
                        TextBlock::new()
                            .text(name)
                            .font_size(type_ramp::BODY.size)
                            .vertical_alignment(VerticalAlignment::Center),
                    )),
            )
            .tooltip(tip)
    }

    /// 主干 `BuildProducedFilesRow`：轮尾「本轮文件改动」行（标签 `[Auto, *]` chips）。
    /// 主干用 `SimpleWrapPanel` 按实际宽度换行，reactor 没有换行面板，这里按 chip 的估算
    /// 宽度分行——同一批文件的换行位置可能差一颗，行数与全展开的口径一致。
    fn produced_row(&self, bubble: &Bubble, p: Palette, context: &mut ViewContext<Shell>) -> View {
        const LINE_WIDTH: f64 = 520.0;
        let mut lines: Vec<Vec<String>> = vec![Vec::new()];
        let mut used = 0.0;
        for path in &bubble.produced {
            let name = path.replace('\\', "/");
            let name = name.rsplit('/').next().unwrap_or(name.as_str());
            let width = size::GLYPH_CAPTION + space::S6 + name.chars().count() as f64 * 7.2;
            let first = lines.last().is_some_and(|line| line.is_empty());
            if !first && used + width > LINE_WIDTH {
                lines.push(Vec::new());
                used = 0.0;
            }
            used += width + space::S6;
            lines.last_mut().unwrap().push(path.clone());
        }
        let chips: Vec<KeyedView> = lines
            .into_iter()
            .filter(|line| !line.is_empty())
            .enumerate()
            .map(|(index, paths)| {
                let cells = paths
                    .iter()
                    .map(|path| {
                        KeyedView::new(format!("chip-{path}"), self.produced_chip(path, context))
                    })
                    .collect::<Vec<KeyedView>>();
                KeyedView::new(
                    format!("line-{index}"),
                    StackPanel::new()
                        .orientation(Orientation::Horizontal)
                        .spacing(space::S6)
                        .keyed_children(cells),
                )
            })
            .collect();
        let label = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(space::S6)
            .grid_column(0)
            .vertical_alignment(VerticalAlignment::Center)
            .children((TextBlock::new()
                .text(self.catalog.l("本轮文件改动"))
                .font_size(type_ramp::CAPTION.size)
                .opacity(0.7)
                .foreground(p.text_tertiary.clone())
                .vertical_alignment(VerticalAlignment::Center),));
        Grid::new()
            .columns([GridLength::Auto, GridLength::STAR])
            .column_spacing(space::S8)
            .margin(th([6.0, 2.0, 6.0, 2.0]))
            .automation_id("ProducedFilesRow")
            .automation_name(self.catalog.l("本轮文件改动"))
            .keyed_children(
                vec![
                    KeyedView::new("label", label),
                    KeyedView::new(
                        "chips",
                        StackPanel::new()
                            .orientation(Orientation::Vertical)
                            .spacing(space::S2)
                            .grid_column(1)
                            .keyed_children(chips),
                    ),
                ]
                .into_iter()
                .collect::<Vec<KeyedView>>(),
            )
    }

    /// 主干「打开/显示」那两颗没被改过样式的按钮：走 WinUI 默认度量（MinWidth 120 也保留）。
    fn text_button(
        &self,
        label: &str,
        id: &str,
        message: Msg,
        context: &ViewContext<Shell>,
    ) -> View {
        Button::new()
            .automation_id(id.to_string())
            .automation_name(self.catalog.l(label))
            .on_click(context.message(message))
            .content(
                TextBlock::new()
                    .text(self.catalog.l(label))
                    .font_size(type_ramp::BODY.size),
            )
    }

    /// 气泡照抄主干 `UserTpl` / `AssistantTpl` / `ToolCallTpl` / `ReasoningTpl` /
    /// `DeliverableTpl`：用户在右（MaxWidth 560、右下尖角胶囊），助手铺满成卡片，两者同材质
    /// （`BubbleMicaBrush` + `StrokeSubtle` 1px）。主干没有角色标签行，所以这里也不写「你/助手」。
    fn bubble_view(
        &self,
        bubble: &Bubble,
        is_answer: bool,
        closed: bool,
        p: Palette,
        context: &mut ViewContext<Shell>,
    ) -> View {
        let row = th(pad::LIST_ITEM);
        let body = TextBlock::new()
            .text(bubble.text.clone())
            .font_size(self.font_size)
            .foreground(p.text_primary.clone())
            .text_wrapping(TextWrapping::Wrap)
            .is_text_selection_enabled(true);
        match bubble.role {
            "user" => StackPanel::new()
                .horizontal_alignment(HorizontalAlignment::Right)
                .max_width(size::BUBBLE_MAX_WIDTH)
                .margin(row)
                .spacing(space::S2)
                .children((
                    Border::new()
                        .horizontal_alignment(HorizontalAlignment::Right)
                        .background(p.bubble)
                        .border_brush(p.stroke_subtle)
                        .border_thickness(size::STROKE)
                        .corner_radius(CornerRadius::new(
                            radius::XLARGE,
                            radius::XLARGE,
                            radius::SMALL,
                            radius::XLARGE,
                        ))
                        .padding(th(pad::BUBBLE_USER))
                        .content(body),
                    self.bubble_actions(bubble, false, closed, p, context),
                )),
            // 主线 `AppendSystemMessage` 的 Role=system 在模板选择器里落到 default ⇒ 同一个 `ToolTpl`
            // （`BubbleTemplateSelector.cs:22-32`）：无图标、无摘要的灰行。分叉给它单独一个 role 值，
            // 是为了让 `hero_state()` 把它当「非对话内容」排除掉（MW:4374 同口径）。
            "tool" | "system" => self.tool_line(bubble, p),
            "reasoning" => self.reasoning_row(bubble, p, context),
            "deliverable" => self.deliverable_card(bubble, p, context),
            _ => {
                let key = bubble_key(bubble);
                // 主干 turnTail 槽（本轮文件改动）渲染在动作行之前，且只挂轮尾答案。
                let produced = if is_answer && !bubble.produced.is_empty() {
                    Some(self.produced_row(bubble, p, context))
                } else {
                    None
                };
                let actions = self.bubble_actions(bubble, is_answer, closed, p, context);
                // 便签编辑器（主干是 `ContentDialog`，分叉没有对话框原语）只挂在编辑目标
                // 那一条气泡下方，跟着动作行之后 —— 主干 `note_for` 只有一个槽，同时最多开一个。
                let editor = (self.note_for.as_deref() == Some(bubble.id.as_str()))
                    .then(|| self.note_editor(p, context));
                Border::new()
                    .margin(th(pad::LIST_ITEM_TAIL))
                    .background(p.bubble)
                    .border_brush(p.stroke_subtle)
                    .border_thickness(size::STROKE)
                    .corner_radius(radius::LARGE)
                    .padding(th(pad::BUBBLE_ASSISTANT))
                    .on_pointer_entered(context.callback(move |_| Msg::Hover(Some(key.clone()))))
                    .on_pointer_exited(context.callback(|_| Msg::Hover(None)))
                    .content(
                        StackPanel::new()
                            .orientation(Orientation::Vertical)
                            .spacing(space::S2)
                            .keyed_children(
                                vec![KeyedView::new("body", body)]
                                    .into_iter()
                                    .chain(
                                        produced
                                            .map(|row| KeyedView::new("produced", row))
                                            .into_iter(),
                                    )
                                    .chain([KeyedView::new("actions", actions)])
                                    .chain(
                                        editor
                                            .map(|view| KeyedView::new("note-editor", view))
                                            .into_iter(),
                                    )
                                    .collect::<Vec<KeyedView>>(),
                            ),
                    )
            }
        }
    }

    /// `PendingTpl`：转圈 ProgressRing(14) + 「少女祈祷中…」。主干把 Padding 挂在 StackPanel 上，
    /// reactor 里只有 Border 有 `padding`，所以外层用 Border 承接内边距与 UIA 名称。
    fn pending_row(&self, p: Palette) -> View {
        Border::new()
            .margin(th(pad::LIST_ITEM_TAIL))
            .padding(th(pad::PENDING_ROW))
            .background(p.transparent)
            .automation_name(self.catalog.l("少女祈祷中"))
            .content(
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(space::S8)
                    .children((
                        ProgressRing::new()
                            .width(size::PENDING_RING)
                            .height(size::PENDING_RING)
                            .is_active(true)
                            .vertical_alignment(VerticalAlignment::Center),
                        TextBlock::new()
                            .text(self.catalog.l("少女祈祷中…"))
                            .font_size(type_ramp::CAPTION.size)
                            .foreground(p.text_tertiary.clone())
                            .vertical_alignment(VerticalAlignment::Center)
                            .automation_id("PendingThinkingText"),
                    )),
            )
    }

    /// ComposerCard：`Border(Card/Stroke/1/r=12, padding 0,8,4,0)` + 输入框 + 动作行。
    fn composer(&self, context: &mut ViewContext<Shell>, p: Palette, row: i32) -> View {
        // 主干的发送键是「发送/停止」二态切换，不因输入为空而禁用。
        let running = self.active_running();
        Border::new()
            .grid_row(row)
            .margin(th(pad::COMPOSER_BAR))
            .content(
                Border::new()
                    .background(p.card)
                    .border_brush(p.stroke)
                    .border_thickness(size::STROKE)
                    .corner_radius(radius::LARGE)
                    .padding(th(pad::COMPOSER_CARD))
                    .content(
                        StackPanel::new()
                            .orientation(Orientation::Vertical)
                            .children((
                                Border::new()
                                    .padding(th(pad::COMPOSER_INPUT))
                                    // 焦点证据（正向）：框架里只有 `Border` 挂了 `PointerPressed`，
                                    // 而指针按进 composer 那一格 = 主干那颗 InputBox 拿到焦点。
                                    // 只加事件回调，不加任何容器/尺寸 ⇒ 布局与观感零变化。
                                    .on_pointer_pressed(context.callback(|_| Msg::KeyFocus(KeyOwner::Composer)))
                                    .content(
                                    TextBox::new()
                                        .text(self.input.clone())
                                        .placeholder_text(
                                            self.catalog.l(
                                                "描述你想要构建的内容, / 调用指令, @ 文件或对话",
                                            ),
                                        )
                                        .accepts_return(true)
                                        .text_wrapping(TextWrapping::Wrap)
                                        .min_height(size::COMPOSER_INPUT_MIN_HEIGHT)
                                        .max_height(size::COMPOSER_INPUT_MAX_HEIGHT)
                                        .background(p.transparent)
                                        .border_thickness(0.0)
                                        .on_text_changed(
                                            context.callback(|text: String| Msg::Input(text)),
                                        )
                                        .automation_id("InputBox")
                                        .automation_name(self.catalog.l("消息输入框")),
                                ),
                                Border::new().padding(th(pad::COMPOSER_ROW)).content(
                                    Grid::new()
                                        .columns([
                                            GridLength::Auto,
                                            GridLength::STAR,
                                            GridLength::Auto,
                                        ])
                                        .column_spacing(8.0)
                                        .children((
                                            // 主干 `MainWindow.xaml:1055-1068` ComposerAddButton：
                                            // 28×28、Padding 0、CornerRadius=CornerPill(24)、
                                            // Background=CardHoverBrush、BorderThickness=0、
                                            // 字形 E710 @ GlyphSizeBody=14。
                                            // 旧写法用 `ButtonBorderThickness`（无效键）⇒ 留着一圈
                                            // 1px 灰描边；圆角走 `ControlCornerRadius` 覆盖 ⇒ 静默
                                            // 无效，渲染成 4px 圆角方块。现在整颗走 `pill_button`。
                                            pill_button(
                                                Button::new()
                                                    .on_click(context.message(Msg::Note(
                                                        self.catalog.l("附件尚未移植"),
                                                    )))
                                                    .automation_id("ComposerAddButton")
                                                    .automation_name(
                                                        self.catalog.l("添加附件或操作"),
                                                    ),
                                                mark_size(glyph::NEW_SESSION, size::GLYPH_BODY),
                                                Pill::card(p),
                                                "composer-add",
                                                HoverSlot::Row,
                                                self.hover.as_deref(),
                                                self.pressed.as_deref(),
                                                context,
                                            ),
                                            Border::new().grid_column(1),
                                            // 主干 `MainWindow.xaml:1254-1266` SendButton：
                                            // 34×34、Padding 0、CornerRadius=CornerPill(24)、
                                            // Style=AccentButtonStyle（强调色填充 + 反白前景 + 无描边）、
                                            // 字形 E74A/E71A @ GlyphSizeBody=14，注释明确「不重绑任何色值」。
                                            //
                                            // **`.style(ButtonStyle::Accent)` 在本分叉挂不上**：它与所有小钮
                                            // 都要用的 `resource_overrides` 同挂在一个 Button 上，启动即 native
                                            // fail-fast —— 退出码 0xC000027B、崩在 Microsoft.UI.Xaml.dll、
                                            // stdout/stderr 全 0 字节、连窗口都没了。2026-09-21 整树二分：
                                            // 只删那一行 `.style(...)`、其余一字未改，应用就正常起。
                                            // 触发条件是「同挂」而非 style 本身：`settings_button` 只挂
                                            // `.style(ButtonStyle::Accent)`、不带 resource_overrides，
                                            // 自测跑满 12 个分区都不崩。
                                            // 而 `pill_button` 一定要挂 `resource_overrides`（四态底色/描边
                                            // 全透明，否则模板那层 4px 方块高亮会从圆底外面支出去）⇒ 这里
                                            // 只能继续走 AccentButtonStyle 的**等值复刻**：底色仍是主题画刷
                                            // `p.accent`（AccentFillColorDefaultBrush，跟系统强调色走，
                                            // 不写死），悬停/按下按 generic.xaml:7631-7633 的
                                            // 「同色画刷降到 0.9 / 0.8」用 opacity 复刻，禁用换
                                            // `AccentFillColorDisabled`，反白前景四态钉死。
                                            pill_button(
                                                Button::new()
                                                    .grid_column(2)
                                                    .on_click(context.message(if running {
                                                        Msg::Cancel
                                                    } else {
                                                        Msg::Send
                                                    }))
                                                    .automation_id("SendButton")
                                                    .automation_name(self.catalog.l(if running
                                                    {
                                                        "停止运行"
                                                    } else {
                                                        "发送消息"
                                                    })),
                                                mark_size(
                                                    if running {
                                                        glyph::STOP
                                                    } else {
                                                        glyph::SEND
                                                    },
                                                    size::GLYPH_BODY,
                                                ),
                                                Pill::accent(p, size::COMPOSER_SEND_BUTTON),
                                                "composer-send",
                                                HoverSlot::Row,
                                                self.hover.as_deref(),
                                                self.pressed.as_deref(),
                                                context,
                                            ),
                                        )),
                                ),
                            )),
                    ),
            )
    }

    /// 主线 `KernelBootPanel`（`MainWindow.xaml:691-709` + `MainWindow.KernelBoot.cs`）：
    /// 内容面正中一张**无背景**的加载卡，替掉旧的整窗不透明遮罩 `loading_layer`。
    /// 层级逐属性对齐：Row 1（本函数 `.grid_row(0)`，因为分叉的 ChatPage 只有 `[*, Auto]` 两行）、
    /// H/V Center、`MaxWidth=360`、`Spacing=Space12(12)`；子元素六颗按 KC 的次序：
    /// Mark(34 `EmptyStateMarkSize`) / Stage(`EmptyStateTitleTextStyle` = Title 28 SemiBold) /
    /// Bar(`ProgressBar` 宽 260、值域 0–100，WinUI 默认条样式) / Step(`BodyStrongTextStyle` 14 SemiBold) /
    /// Hint(`HintTextStyle` = Caption 12 + `TextTertiaryBrush`) / Retry（框架默认 Button，**不套样式**）。
    ///
    /// 显隐：reactor 0.100.0 没有 `Visibility` ⇒ Hint / Retry 各占一枚 keyed 槽，缺席即 Destroy；
    /// 整颗卡由 `chat_page` 的那个 `if self.boot.showing` 决定进不进树。
    /// 已知偏差（规格 §6.3 ⑧）：`TextAlignment=Center` 抄不过来。**已按「Style/Setter 试一发」的
    /// 口径做过实测**（临时塞进本函数体编译后回滚）：9 条编译错误、进程根本没起来 ⇒ 谈不上
    /// 0xC000027B 那条风险。原样摘录：`no method named 'text_alignment' found for struct
    /// 'windows_reactor::TextBlock'`、`cannot find type 'Style'/'Setter'/'TextAlignment' in
    /// this scope`、`no method named 'style' found for struct 'windows_reactor::TextBlock'`、
    /// `no method named 'resource_overrides' found for struct 'windows_reactor::TextBlock'`。
    /// 公开面（`LayoutControl`：width/height/min-max/opacity/h-v_alignment/margin/exit_transition）
    /// 与 `TextBlock` 的 8 个属性里都没有文本对齐；框架内部那条 `<Style><Setter/></Style>` 的
    /// XAML 生成路（`native/winui/mod.rs:1027`）只喂 `ThemeStyle` 的四枚**画刷**槽，不外露。
    /// ⇒ 只能靠 `.horizontal_alignment(Center)` 把整块居中，多行时行内退化成左对齐。
    ///
    /// 六颗 `automation_id` 逐字取自 `MainWindow.xaml:703-708` 的 `x:Name`（WinUI 由 x:Name 生成
    /// AutomationId）：`KernelBootMark` / `KernelBootStage` / `KernelBootBar` / `KernelBootStep` /
    /// `KernelBootHint` / `KernelBootRetry` ⇒ UIA 侧按 id 反查两棵树得到同一套坐标。
    fn kernel_boot_panel(&self, context: &mut ViewContext<Shell>, p: Palette) -> View {
        let frame = self.boot.render();
        // 每颗子元素先落成 `View` 局部量：`KeyedView::new` 收 `impl Into<View>`，
        // 直接在实参里写 `.into()` 会让目标类型推不出来（E0283）。
        let mark: View = brand_mark(self.brand(), size::EMPTY_STATE_MARK, "KernelBootMark");
        // 三行文案各算一次：`.text()` 与 `.automation_name()` 必须同源 —— 主干那三颗 TextBlock
        // 没写 `AutomationProperties.Name`，UIA 的 Name 就是它自己的 Text（英文界面下 Name 也跟着
        // 变成英文）。分叉原先给 Step/Hint 硬写了「内核加载进度」那枚 Name，是把**面板**的 Name
        // （MX:708）抄到了子元素上，属于「文案多做」，这里一并收掉。
        let stage_text = self.catalog.l(frame.stage_key);
        let stage: View = TextBlock::new()
            .text(stage_text.clone())
            .font_size(type_ramp::TITLE.size)
            .font_weight(FontWeight::SEMI_BOLD)
            .foreground(p.text_primary.clone())
            .text_wrapping(TextWrapping::Wrap)
            .horizontal_alignment(HorizontalAlignment::Center)
            .automation_id("KernelBootStage")
            .automation_name(stage_text)
            .into();
        let bar: View = ProgressBar::new()
            .minimum(0.0)
            .maximum(100.0)
            .value(frame.value)
            .is_indeterminate(false)
            .width(size::BOOT_BAR_WIDTH)
            .horizontal_alignment(HorizontalAlignment::Center)
            .automation_id("KernelBootBar")
            .into();
        // 主线 `BootStepLine()`（KC:159-161）：第几步 / 共几步 / 当前百分比。
        let step_text = self.catalog.lf(
            "第 {0} 步，共 {1} 步 · {2}%",
            &[
                frame.step.to_string(),
                kernel_boot::STEP_TOTAL.to_string(),
                frame.percent.to_string(),
            ],
        );
        let step: View = TextBlock::new()
            .text(step_text.clone())
            .font_size(type_ramp::BODY_STRONG.size)
            .font_weight(FontWeight::SEMI_BOLD)
            .foreground(p.text_primary.clone())
            .text_wrapping(TextWrapping::Wrap)
            .horizontal_alignment(HorizontalAlignment::Center)
            .automation_id("KernelBootStep")
            .automation_name(step_text)
            .into();
        let mut slots: Vec<KeyedView> = vec![
            KeyedView::new("mark", mark),
            KeyedView::new("stage", stage),
            KeyedView::new("bar", bar),
            KeyedView::new("step", step),
        ];
        // 提示行三态（KC:234-248）＋失败态整句（KC:144-150）：失败时那一行让给原因，
        // 插件刻度 / 秒数都不再出（主线此刻 tick 已停，根本不会再画它们）。
        let hint = match &frame.failure {
            Some((reason, detail)) => {
                let message = self.catalog.l(reason);
                Some(if detail.is_empty() {
                    message
                } else {
                    format!("{message}：{detail}")
                })
            }
            None => frame.hint.as_ref().map(|hint| match hint {
                BootHint::Plugins(ready, total) => self.catalog.lf(
                    "正在安装插件 {0}/{1}",
                    &[ready.to_string(), total.to_string()],
                ),
                BootHint::Seconds(seconds) => self
                    .catalog
                    .lf("内核加载已进行 {0} 秒", &[seconds.to_string()]),
            }),
        };
        if let Some(text) = hint {
            let hint: View = TextBlock::new()
                .text(text.clone())
                .font_size(type_ramp::CAPTION.size)
                .foreground(p.text_tertiary.clone())
                .text_wrapping(TextWrapping::Wrap)
                .horizontal_alignment(HorizontalAlignment::Center)
                .automation_id("KernelBootHint")
                .automation_name(text)
                .into();
            slots.push(KeyedView::new("hint", hint));
        }
        // 重试钮只在 `FailKernelBoot` 之后出现（KC:151；`ShowKernelBootPanel` 每次都收回去）。
        if frame.failure.is_some() {
            let retry: View = Button::new()
                .on_click(context.message(Msg::BootRetry))
                .horizontal_alignment(HorizontalAlignment::Center)
                .automation_id("KernelBootRetry")
                .automation_name("重试")
                // 主线没给它套样式 ⇒ 框架默认 Button（底 `ControlFillColorDefault`、
                // 描边 `ControlStrokeColorDefault`、字 `TextFillColorPrimary`）。
                // 注意别在这颗钮上同时挂 `ButtonStyle::Accent` 和 `resource_overrides`
                // ——那是 0xC000027B 静默 fail-fast 的已知成因之一。
                .content(self.catalog.l("重试"));
            slots.push(KeyedView::new("retry", retry));
        }
        StackPanel::new()
            .grid_row(0)
            .orientation(Orientation::Vertical)
            .max_width(size::BOOT_CARD_MAX_WIDTH)
            .spacing(space::S12)
            .horizontal_alignment(HorizontalAlignment::Center)
            .vertical_alignment(VerticalAlignment::Center)
            .automation_id("KernelBootPanel")
            .automation_name("内核加载进度")
            .keyed_children(slots)
    }

    /// 主干 `SettingsHost`：页头只有分区名（TitleTextBlockStyle + 36,24,36,4），
    /// 正文是 `SettingsScroller`(36,12,36,36) 里一个 MaxWidth 1064、Spacing 12 的 StackPanel。
    /// 钻取二级页时页头换成二级页标题并多出返回钮，正文整棵换成 `SettingsSubHost`。
    fn settings_page(&self, context: &mut ViewContext<Shell>, p: Palette) -> View {
        let section_title = SETTINGS_SECTIONS
            .iter()
            .find(|(id, _, _)| *id == self.section)
            .map(|(_, label, _)| *label)
            .unwrap_or("设置");
        let (title, cells) = match self.sub.as_deref() {
            Some(id) => (sub_title(id), self.settings_sub(id, context, p)),
            None => (section_title.to_string(), self.settings_section(context, p)),
        };
        let back: View = self.settings_back_button(context);
        Grid::new()
            .rows([GridLength::Auto, GridLength::STAR])
            .children((
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(space::S8)
                    .margin(th(pad::PAGE_HEADER))
                    .vertical_alignment(VerticalAlignment::Center)
                    .keyed_children(vec![
                        KeyedView::new(
                            "back-slot",
                            self.either(
                                self.sub.is_some(),
                                ("sub-back", back),
                                ("no-back", blank()),
                            ),
                        ),
                        KeyedView::new(
                            "header",
                            TextBlock::new()
                                .text(self.catalog.l(&title))
                                .font_size(type_ramp::TITLE.size)
                                .font_weight(FontWeight::SEMI_BOLD)
                                .foreground(p.text_primary.clone())
                                .vertical_alignment(VerticalAlignment::Center)
                                .automation_name(title.clone()),
                        ),
                    ]),
                ScrollViewer::new().grid_row(1).content(
                    Border::new().padding(th(pad::PAGE_SCROLL)).content(
                        StackPanel::new()
                            .orientation(Orientation::Vertical)
                            .spacing(12.0)
                            .max_width(size::PAGE_MAX_WIDTH)
                            .keyed_children(cells),
                    ),
                ),
            ))
    }

    /// 主干 `SettingsSubBackButton`：32×32、无边框、CornerSmall、IconButtonStyle，Esc 等价。
    fn settings_back_button(&self, context: &mut ViewContext<Shell>) -> View {
        Button::new()
            .style(ButtonStyle::Subtle)
            .resource_overrides(
                ResourceOverrides::new()
                    .set("ButtonBackground", Color::transparent())
                    .set("ButtonBackgroundPointerOver", Color::transparent())
                    .set("ButtonBackgroundPressed", Color::transparent())
                    // 无描边靠四态透明画刷；`ButtonBorderThemeThickness` 是 double 型键，
                    // 用 Thickness 覆它会让 WinUI fail-fast（见 `button_flat` 注释）。
                    .set("ButtonBorderBrush", Color::transparent())
                    .set("ButtonBorderBrushPointerOver", Color::transparent())
                    .set("ButtonBorderBrushPressed", Color::transparent())
                    .set("ButtonBorderBrushDisabled", Color::transparent())
                    .set("ButtonPadding", Thickness::uniform(0.0)),
            )
            .width(size::SHELL_BACK_BUTTON)
            .height(size::SHELL_BACK_BUTTON)
            .min_width(0.0)
            .vertical_alignment(VerticalAlignment::Center)
            .automation_name(self.catalog.l("返回上一级"))
            .automation_id("SettingsSubBackButton")
            .on_click(context.message(Msg::CloseSub))
            .content(
                // 模板内缩抵回去，见 TEMPLATE_INSET。
                Grid::new()
                    .margin(th(TEMPLATE_INSET))
                    .keyed_children(vec![KeyedView::new(
                        "chrome",
                        Border::new()
                            .corner_radius(radius::SMALL)
                            .content(mark_size(glyph::BACK, size::GLYPH_BODY)),
                    )]),
            )
    }

    /// 分区一级页正文。主干其余分区要读内核快照，分叉目前只有 general/plugins 有内容。
    fn settings_section(&self, context: &mut ViewContext<Shell>, p: Palette) -> Vec<KeyedView> {
        match self.section.as_str() {
            "general" => self.settings_general(context, p),
            "personalization" => self.settings_personalization(context, p),
            "models" => self.settings_models(p),
            "plugins" => self.settings_plugins(context, p),
            "skills" => self.settings_skills(p),
            "memory" => self.settings_memory(context, p),
            "agent-presets" => self.settings_agent_presets(p),
            "computer-control" => self.settings_computer_control(context, p),
            "browser-control" => self.settings_browser_control(context, p),
            "usage" => self.settings_usage(p),
            "pet" => self.settings_pet(context, p),
            "about" => self.settings_about(p),
            _ => Vec::new(),
        }
    }

    /// 二级页正文：主干每次进页按当前快照现建，不缓存；这里同样按 id 分派。
    fn settings_sub(
        &self,
        id: &str,
        context: &mut ViewContext<Shell>,
        p: Palette,
    ) -> Vec<KeyedView> {
        match id {
            "shell" => self.sub_shell(context, p),
            "agent-loop" => self.sub_agent_loop(context, p),
            "subagent" => self.sub_subagent(context, p),
            "web-search" => self.sub_web_search(context, p),
            "auto-approval" => self.sub_auto_approval(context, p),
            "defaults" => self.sub_defaults(p),
            _ => Vec::new(),
        }
    }

    /// 插件配置一级页：`MakeSectionDesc` + 6 张钻取卡。
    /// 主干上面还有一条 `SelectorBar`（插件配置 / 插件列表），reactor 的 SelectorBar 没有加条目的
    /// API（只有 `on_selected_text_changed`），画不出 Tab 条，所以分叉只呈现默认的「插件配置」内容。
    fn settings_plugins(&self, context: &mut ViewContext<Shell>, p: Palette) -> Vec<KeyedView> {
        let mut cells = vec![KeyedView::new(
            "section-desc",
            Border::new()
                .margin(th(pad::CARD_DESCRIPTION))
                .content(self.settings_note("配置和查看本部署已安装的插件。", p)),
        )];
        for (id, code, title, desc) in SETTINGS_PLUGIN_CARDS {
            cells.push(self.settings_nav_card(id, *code, title, desc, context, p));
        }
        cells
    }

    /// 主干 `MakeSettingsNavCard`：卡片本身是 Button（Padding 16,12、RadiusMedium 8、CardBrush+Stroke 1px），
    /// 内容 `[Auto,*,Auto]` 列距 16 —— 字形、标题+描述（Spacing 2）、右向箭头 E76B。
    fn settings_nav_card(
        &self,
        id: &str,
        code: char,
        title: &str,
        desc: &str,
        context: &mut ViewContext<Shell>,
        p: Palette,
    ) -> KeyedView {
        let key = id.to_string();
        KeyedView::new(
            format!("nav-card-{id}"),
            Button::new()
                .style(ButtonStyle::Subtle)
                .resource_overrides(
                    ResourceOverrides::new()
                        .set("ButtonBackground", Color::transparent())
                        .set("ButtonBackgroundPointerOver", Color::transparent())
                        .set("ButtonBackgroundPressed", Color::transparent())
                        // 同 `button_flat`：描边用四态透明画刷抹，绝不覆 `ButtonBorderThemeThickness`。
                        .set("ButtonBorderBrush", Color::transparent())
                        .set("ButtonBorderBrushPointerOver", Color::transparent())
                        .set("ButtonBorderBrushPressed", Color::transparent())
                        .set("ButtonBorderBrushDisabled", Color::transparent())
                        .set("ButtonPadding", Thickness::uniform(0.0)),
                )
                .min_width(0.0)
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .horizontal_content_alignment(HorizontalAlignment::Stretch)
                .automation_name(self.catalog.l(title))
                .automation_id(format!("PluginsNav_{}", camel(id)))
                .on_click(context.message(Msg::OpenSub(key)))
                .content(
                    // 模板内缩（TEMPLATE_INSET）抵回去后，卡片外观与 16,12 内边距都落在内层 Border。
                    Grid::new()
                        .margin(th(TEMPLATE_INSET))
                        .keyed_children(vec![KeyedView::new(
                            "card",
                            Border::new()
                                .background(p.card.clone())
                                .border_brush(p.stroke.clone())
                                .border_thickness(th([size::STROKE; 4]))
                                .corner_radius(radius::MEDIUM)
                                .padding(th(pad::NAV_CARD))
                                .content(
                                    Grid::new()
                                        .columns([
                                            GridLength::Auto,
                                            GridLength::STAR,
                                            GridLength::Auto,
                                        ])
                                        .column_spacing(space::S16)
                                        .children((
                                            // 主干 `MakeSettingsNavCard`（xaml.cs:9279-9286）：
                                            // 图标 FontSize=GlyphSizeBody(14)。
                                            mark_center(code, size::GLYPH_BODY, 0),
                                            StackPanel::new()
                                                .orientation(Orientation::Vertical)
                                                .spacing(space::S2)
                                                .vertical_alignment(VerticalAlignment::Center)
                                                .grid_column(1)
                                                .children((
                                                    TextBlock::new()
                                                        .text(self.catalog.l(title))
                                                        .font_size(type_ramp::BODY.size)
                                                        .foreground(p.text_primary.clone()),
                                                    self.settings_note(desc, p),
                                                )),
                                            // 主干 xaml.cs:9302-9306：钻取 chevron 是 **E76C**（右向）、
                                            // FontSize=GlyphSizeCaption(12)、TextTertiaryBrush。
                                            // E76B 是左向，用错方向就反了。
                                            Border::new()
                                                .grid_column(2)
                                                .vertical_alignment(VerticalAlignment::Center)
                                                .opacity(0.72)
                                                .content(mark_size(
                                                    glyph::CHEVRON_RIGHT,
                                                    size::GLYPH_CAPTION,
                                                )),
                                        )),
                                ),
                        )]),
                ),
        )
    }

    /// 终端卡：命令超时 + 单流输出上限（`AddShellCard`）。
    fn sub_shell(&self, context: &mut ViewContext<Shell>, p: Palette) -> Vec<KeyedView> {
        vec![self.settings_card(
            "shell",
            "终端",
            "限制 agent 运行的每一条命令。",
            vec![
                self.settings_row(
                    "shell-timeoutMs",
                    "命令超时（毫秒）",
                    "单条命令允许运行多久，超时即终止。",
                    self.settings_number("shell_timeoutMs", 120_000.0, "命令超时（毫秒）", context),
                    p,
                ),
                self.settings_row(
                    "shell-maxOutputBytes",
                    "单流输出上限（字节）",
                    "超出部分会转存到临时文件，而不是被丢弃。",
                    self.settings_number(
                        "shell_maxOutputBytes",
                        64_000.0,
                        "单流输出上限（字节）",
                        context,
                    ),
                    p,
                ),
            ],
            p,
        )]
    }

    /// Agent 循环卡（`AddAgentLoopCard`）。
    fn sub_agent_loop(&self, context: &mut ViewContext<Shell>, p: Palette) -> Vec<KeyedView> {
        vec![self.settings_card(
            "agent-loop",
            "Agent 循环",
            "Agent 如何派发工具调用。",
            vec![self.settings_row(
                "agent-loop-parallel",
                "并行工具调用数",
                "同一步内最多同时运行多少个可并行的调用。",
                self.settings_number(
                    "agent-loop_maxParallelToolCalls",
                    10.0,
                    "并行工具调用数",
                    context,
                ),
                p,
            )],
            p,
        )]
    }

    /// Subagent 卡（`AddSubagentCard`）：主干 seedNote 取自 `agent-default-model` 快照，分叉按读不到回落。
    fn sub_subagent(&self, context: &mut ViewContext<Shell>, p: Palette) -> Vec<KeyedView> {
        vec![self.settings_card(
            "subagent",
            "Subagent",
            "控制 Agent 为 Subagent 选择模型的权限。",
            vec![self.settings_row(
                "subagent-enabled",
                "允许 Agent 为 Subagent 选择模型",
                "开启后，Agent 可以为每个 Subagent 选择提供方和模型。仅影响新会话。内核要求开启时至少有一个允许模型；当前读不到默认模型，开启可能被内核拒绝并在此提示。",
                self.settings_toggle(
                    "subagent-model-selection_enabled",
                    false,
                    "允许 Agent 为 Subagent 选择模型",
                    context,
                ),
                p,
            )],
            p,
        )]
    }

    /// 网页搜索卡（`AddWebSearchCard`）：API Key 行要问 `credentials/describe`，读不到就不画（与主干一致）。
    fn sub_web_search(&self, context: &mut ViewContext<Shell>, p: Palette) -> Vec<KeyedView> {
        vec![self.settings_card(
            "web-search",
            "网页搜索",
            "DeepSeek 搜索提供方。",
            vec![
                self.settings_row(
                    "web-search-baseURL",
                    "接口地址",
                    "留空则使用提供方默认地址。",
                    self.settings_text(
                        "web-search-deepseek_baseURL",
                        "",
                        "提供方默认",
                        "接口地址",
                        context,
                    ),
                    p,
                ),
                self.settings_row(
                    "web-search-maxUses",
                    "单次请求最多搜索次数",
                    "一次请求在必须作答前最多可以搜索多少次。",
                    self.settings_number(
                        "web-search-deepseek_maxUses",
                        5.0,
                        "单次请求最多搜索次数",
                        context,
                    ),
                    p,
                ),
            ],
            p,
        )]
    }

    /// 自动审批二级页（`AddAutoApprovalCardsAsync`）：判定模型卡要异步取目录，分叉先不画。
    fn sub_auto_approval(&self, context: &mut ViewContext<Shell>, p: Palette) -> Vec<KeyedView> {
        vec![
            KeyedView::new(
                "section-desc",
                Border::new()
                    .margin(th(pad::CARD_DESCRIPTION))
                    .content(self.settings_note(
                        "自动审批由内核插件 dsh-approval-gate 提供：当会话权限预设切到「自动审批」时，由判定模型预判每次写入/命令是否不可回补——安全则自动批准，涉及删除、凭据、系统配置等硬类别转人工确认。开关增删内核 profile patch 里的 auto-approve 预设，判定模型写在默认模型设置里，两者都由内核热重载即时生效。",
                        p,
                    )),
            ),
            self.settings_card(
                "auto-approval",
                "",
                "",
                vec![self.settings_row(
                    "auto-approval-enabled",
                    "自动审批",
                    "开启后权限模式里多出「自动审批」：判定模型预判越界请求，安全自动批准、有风险转人工。",
                    self.settings_toggle(
                        "auto-approval_enabled",
                        false,
                        "启用自动审批",
                        context,
                    ),
                    p,
                )],
                p,
            ),
            self.settings_card(
                "auto-approval-what",
                "切换后会发生什么",
                "",
                vec![KeyedView::new(
                    "row-consequence",
                    Border::new()
                        .padding(th(pad::SETTINGS_ROW))
                        .content(self.settings_note(
                            "开启：权限选择器（输入区左下角）里出现「自动审批」，新会话可选用它。\n关闭：该预设从选择器消失；正在使用它的会话回落到自定义权限，审批恢复人工确认。\n判定记录与学习状态不受影响，重新开启后继续生效。",
                            p,
                        )),
                )],
                p,
            ),
        ]
    }

    /// 默认插件卡（`AddDefaultPluginsCard`）：行来自 `DshPluginBootstrap.GetStatus(DataHome)` 扫盘，
    /// 分叉不扫盘，因此只剩卡头 —— 与主干读不到插件时的形态一致。
    fn sub_defaults(&self, p: Palette) -> Vec<KeyedView> {
        vec![self.settings_card(
            "defaults",
            "默认插件",
            "Blade² 随内核插件机制默认启用；安装由引导器幂等完成，失败时下次启动自动重试。",
            Vec::new(),
            p,
        )]
    }

    /// 段说明：主干 `MakeSectionDesc`，卡片列 index 0 的 Caption/TextSecondary 行。
    fn settings_section_desc(&self, text: &str, p: Palette) -> KeyedView {
        KeyedView::new(
            "section-desc",
            Border::new()
                .margin(th(pad::CARD_DESCRIPTION))
                .content(self.settings_note(text, p)),
        )
    }

    /// 卡内一行纯说明文本（主干直接往卡片 StackPanel 塞 TextBlock 的那些）。
    fn settings_line(&self, key: &str, text: &str, p: Palette) -> KeyedView {
        KeyedView::new(
            format!("line-{key}"),
            Border::new()
                .padding(th(pad::SETTINGS_ROW))
                .content(self.settings_note(text, p)),
        )
    }

    /// 主干 `CodeTextStyle` + 可选中：About 的版本号、Memory 的文件路径都走它。
    /// reactor 0.100 全框架没有 font_family setter，等宽只能落到默认字体上（已知抄不动）。
    fn settings_code_text(&self, text: String, p: Palette) -> View {
        TextBlock::new()
            .text(text)
            .font_size(type_ramp::CODE.size)
            .foreground(p.text_primary)
            .text_wrapping(TextWrapping::Wrap)
            .is_text_selection_enabled(true)
            .vertical_alignment(VerticalAlignment::Center)
            .into()
    }

    /// 滑杆 + 读数（主干 `Slider` + `TextBlock "{0} px"` / `"{0}%"`，横向 Spacing 12）。
    /// 单位串自己带不带空格照主干那两条原文：`{0}%` 传 `"%"`、`{0} px` 传 `" px"`。
    fn settings_slider(
        &self,
        id: &str,
        min: f64,
        max: f64,
        step: f64,
        fallback: f64,
        width: f64,
        unit: &str,
        automation_id: &str,
        name: &str,
        context: &mut ViewContext<Shell>,
        p: Palette,
    ) -> View {
        let key = id.to_string();
        let value = self.number(&key, fallback);
        let readout: View = TextBlock::new()
            .text(format!("{}{}", value as i32, unit))
            .font_size(type_ramp::BODY.size)
            .foreground(p.text_primary)
            .min_width(size::STEPPER_VALUE_MIN_WIDTH)
            .vertical_alignment(VerticalAlignment::Center)
            .into();
        StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(space::S12)
            .vertical_alignment(VerticalAlignment::Center)
            .children((
                Slider::new()
                    .minimum(min)
                    .maximum(max)
                    .step_frequency(step)
                    .value(value)
                    .width(width)
                    .automation_id(automation_id.to_string())
                    .automation_name(name.to_string())
                    .on_value_changed(context.callback(move |v: f64| Msg::Number(key.clone(), v))),
                readout,
            ))
    }

    /// 主干的 CompactButton / AccentButton。分叉里这些按钮的后端是文件对话框或内核 RPC，
    /// 没有可用通道，所以只保留视觉（不挂 on_click）。
    fn settings_button(&self, label: &str, id: &str, accent: bool) -> View {
        Button::new()
            .style(if accent {
                ButtonStyle::Accent
            } else {
                ButtonStyle::Default
            })
            .min_height(size::COMPACT_BUTTON_MIN_HEIGHT)
            .automation_id(id.to_string())
            .automation_name(self.catalog.l(label))
            .content(
                TextBlock::new()
                    .text(self.catalog.l(label))
                    .font_size(type_ramp::CAPTION.size),
            )
    }

    /// 主干 `AddMountToggleCard`：文字列在左（右缘留 12）、开关贴右，底下挂 `MountStateLine`。
    fn settings_mount_card(
        &self,
        key: &str,
        title: &str,
        desc: &str,
        toggle_id: &str,
        toggle_name: &str,
        fallback: bool,
        context: &mut ViewContext<Shell>,
        p: Palette,
    ) -> KeyedView {
        let state = if self.is_on(toggle_id, fallback) {
            "已启用（内核热重载后生效）"
        } else {
            "已停用"
        };
        let heading: View = TextBlock::new()
            .text(self.catalog.l(title))
            .font_size(type_ramp::BODY_STRONG.size)
            .font_weight(FontWeight::SEMI_BOLD)
            .foreground(p.text_primary)
            .text_wrapping(TextWrapping::Wrap)
            .into();
        KeyedView::new(
            format!("card-{key}"),
            Border::new()
                .background(p.card)
                .border_brush(p.stroke)
                .border_thickness(th([size::STROKE; 4]))
                .corner_radius(radius::MEDIUM)
                .padding(th(pad::CARD_BORDER))
                .content(
                    Border::new().padding(th(pad::SETTINGS_ROW)).content(
                        Grid::new()
                            .columns([GridLength::STAR, GridLength::Auto])
                            .column_spacing(space::S24)
                            .children((
                                StackPanel::new()
                                    .orientation(Orientation::Vertical)
                                    .spacing(space::S2)
                                    .margin(th([0.0, 0.0, 12.0, 0.0]))
                                    .keyed_children(vec![
                                        KeyedView::new("title", heading),
                                        KeyedView::new("desc", self.settings_note(desc, p)),
                                        KeyedView::new("state", self.settings_note(state, p)),
                                    ]),
                                Border::new()
                                    .grid_column(1)
                                    .vertical_alignment(VerticalAlignment::Center)
                                    .content(self.settings_toggle(
                                        toggle_id,
                                        fallback,
                                        toggle_name,
                                        context,
                                    )),
                            )),
                    ),
                ),
        )
    }

    /// 主干 `RenderMountStatusCard`：卡头 + 每个挂载一行「名称 …… 状态文案」。
    fn settings_mount_status_card(
        &self,
        key: &str,
        title: &str,
        desc: &str,
        rows: &[(&str, &str)],
        p: Palette,
    ) -> KeyedView {
        let cells: Vec<KeyedView> = rows
            .iter()
            .map(|(name, state)| {
                let label: View = TextBlock::new()
                    .text(self.catalog.l(name))
                    .font_size(type_ramp::BODY.size)
                    .foreground(p.text_primary)
                    .into();
                let status: View = TextBlock::new()
                    .text(self.catalog.l(state))
                    .font_size(type_ramp::CAPTION.size)
                    .foreground(p.text_secondary)
                    .grid_column(1)
                    .vertical_alignment(VerticalAlignment::Center)
                    .into();
                KeyedView::new(
                    format!("mount-{name}"),
                    Border::new().padding(th(pad::SETTINGS_ROW)).content(
                        Grid::new()
                            .columns([GridLength::STAR, GridLength::Auto])
                            .column_spacing(space::S16)
                            .children((label, status)),
                    ),
                )
            })
            .collect();
        self.settings_card(key, title, desc, cells, p)
    }

    /// 记忆：`RenderMemorySection()`（`MainWindow.xaml.cs:13073`）。无 RPC，读 profile patch 与 JSONL。
    fn settings_memory(&self, context: &mut ViewContext<Shell>, p: Palette) -> Vec<KeyedView> {
        vec![
            self.settings_section_desc(DESC_MEMORY, p),
            self.settings_mount_card(
                "memory",
                "持久记忆",
                "关闭后模型不再写入或读取记忆；已有记忆文件保留不动。",
                "memory_enabled",
                "启用持久记忆",
                true,
                context,
                p,
            ),
            self.settings_card(
                "memory-store",
                "记忆存储",
                "",
                vec![
                    self.settings_line("store-note", "知识图谱 JSONL（由记忆服务器自身管理，删除即清空记忆）：", p),
                    KeyedView::new(
                        "line-store-path",
                        Border::new()
                            .padding(th(pad::SETTINGS_ROW))
                            .content(self.settings_code_text("~/.dsh-mcp-reference-memory.jsonl".into(), p)),
                    ),
                ],
                p,
            ),
            self.settings_card(
                "memory-content",
                "记忆内容",
                "直接编辑记忆文件（JSONL，每行一个实体或关系），停止输入后自动保存；模型写入时自动重新载入。",
                vec![KeyedView::new(
                    "line-editor",
                    Border::new()
                        .padding(th(pad::SETTINGS_ROW))
                        // 焦点证据（反向）：这颗是记忆文件的编辑框，Enter 不该发送。
                        .on_pointer_pressed(context.callback(|_| Msg::KeyFocus(KeyOwner::Other)))
                        .content(
                        TextBox::new()
                            .accepts_return(true)
                            .text_wrapping(TextWrapping::NoWrap)
                            .min_height(size::MEMORY_MIN_HEIGHT)
                            .max_height(size::MEMORY_MAX_HEIGHT)
                            .placeholder_text(self.catalog.l("记忆文件内容"))
                            .automation_id("MemoryContentEditor")
                            .automation_name(self.catalog.l("记忆文件内容"))
                            .on_text_changed(context.callback(move |value: String| {
                                Msg::Field("memory_content".to_string(), value)
                            })),
                    ),
                )],
                p,
            ),
        ]
    }

    /// 电脑控制：`RenderComputerControlSection()`（`MainWindow.xaml.cs:13120`）。
    fn settings_computer_control(
        &self,
        context: &mut ViewContext<Shell>,
        p: Palette,
    ) -> Vec<KeyedView> {
        vec![
            self.settings_section_desc(DESC_COMPUTER_CONTROL, p),
            self.settings_mount_card(
                "computer-registry",
                "电脑控制",
                "开启后模型获得桌面操作能力（截图、鼠标、键盘）；关闭后相关工具从模型视野移除。",
                "computer-control_enabled",
                "启用电脑控制",
                true,
                context,
                p,
            ),
            self.settings_mount_card(
                "computer-cua",
                "Cua Driver（桌面驱动）",
                "电脑操作的执行驱动：需在本机安装 cua-driver 命令行工具后启用，默认停用。",
                "computer-use-cua-driver_enabled",
                "启用 Cua Driver 桌面驱动",
                false,
                context,
                p,
            ),
            self.settings_mount_status_card(
                "computer-status",
                "挂载状态",
                "Blade² 随内核插件机制默认启用；安装由引导器幂等完成，失败时下次启动自动重试。",
                &[
                    ("电脑控制注册表", "已启用"),
                    ("Cua Driver 驱动", "默认停用（需先安装 Cua Driver）"),
                ],
                p,
            ),
        ]
    }

    /// 浏览器控制：`RenderBrowserControlSection()`（`MainWindow.xaml.cs:13145`）。
    fn settings_browser_control(
        &self,
        context: &mut ViewContext<Shell>,
        p: Palette,
    ) -> Vec<KeyedView> {
        vec![
            self.settings_section_desc(DESC_BROWSER_CONTROL, p),
            self.settings_mount_card(
                "browser-registry",
                "浏览器控制",
                "开启后模型获得浏览器操作能力（打开网页、点击、填写、读取内容）；关闭后相关工具从模型视野移除。",
                "browser-control_enabled",
                "启用浏览器控制",
                true,
                context,
                p,
            ),
            self.settings_mount_card(
                "browser-playwright",
                "Playwright（浏览器驱动）",
                "浏览器操作的执行驱动：随包安装，launch 模式无头运行，默认启用。",
                "browser-use-playwright_enabled",
                "启用 Playwright 浏览器驱动",
                true,
                context,
                p,
            ),
            self.settings_mount_status_card(
                "browser-status",
                "挂载状态",
                "Blade² 随内核插件机制默认启用；安装由引导器幂等完成，失败时下次启动自动重试。",
                &[
                    ("浏览器控制注册表", "已启用"),
                    ("Playwright 驱动", "已启用"),
                ],
                p,
            ),
        ]
    }

    /// 个性化：`MainWindow.Personalization.cs`（自定义指令 / 窗口材质 / 背景皮肤）。
    fn settings_personalization(
        &self,
        context: &mut ViewContext<Shell>,
        p: Palette,
    ) -> Vec<KeyedView> {
        let buttons: View = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(space::S8)
            .children((
                self.settings_button("导入图片", "SkinImportButton", false),
                self.settings_button("清除皮肤", "SkinClearButton", false),
            ))
            .into();
        vec![
            self.settings_section_desc(DESC_PERSONALIZATION, p),
            self.settings_card(
                "instructions",
                "自定义指令",
                "写给所有对话的长期说明：怎么称呼你、用什么语气、有哪些固定约束。对所有会话始终生效。",
                vec![
                    KeyedView::new(
                        "line-editor",
                        Border::new()
                            .padding(th([0.0, 4.0, 0.0, 0.0]))
                            // 焦点证据（反向）：自定义指令编辑框，Enter 不该发送。
                            .on_pointer_pressed(context.callback(|_| Msg::KeyFocus(KeyOwner::Other)))
                            .content(
                            TextBox::new()
                                .accepts_return(true)
                                .text_wrapping(TextWrapping::Wrap)
                                .min_height(size::INSTRUCTIONS_MIN_HEIGHT)
                                .max_height(size::INSTRUCTIONS_MAX_HEIGHT)
                                .placeholder_text(self.catalog.l("指令内容"))
                                .automation_id("PersonalizationInstructionsEditor")
                                .automation_name(self.catalog.l("指令内容"))
                                .on_text_changed(context.callback(move |value: String| {
                                    Msg::Field("personalization_instructions".to_string(), value)
                                })),
                        ),
                    ),
                    self.settings_line("status", "共 0 字 · 停止输入后自动保存", p),
                ],
                p,
            ),
            self.settings_card(
                "material",
                "窗口材质",
                "侧栏与顶栏透出的系统材质：Mica 柔和、亚克力更透；系统不支持时自动回退。",
                vec![self.settings_row(
                    "material",
                    "材质",
                    "切换后立即生效，重启后保持",
                    self.settings_combo(
                        "shell_material",
                        SETTINGS_MATERIALS,
                        0,
                        "窗口材质",
                        context,
                    ),
                    p,
                )],
                p,
            ),
            self.settings_card(
                "bubble",
                "消息气泡",
                "气泡背景：半透明直接透出背后画面，亚克力是系统材质；两项都即时生效。",
                vec![
                    self.settings_row(
                        "bubble-material",
                        "气泡材质",
                        "半透明最透，亚克力是系统材质，跟随跟窗口材质走",
                        self.settings_combo(
                            BUBBLE_MATERIAL_KEY,
                            BUBBLE_MATERIALS,
                            0,
                            "气泡材质",
                            context,
                        ),
                        p,
                    ),
                    self.settings_row(
                        "bubble-opacity",
                        "气泡不透明度",
                        "数值越大气泡自身越实，背后画面透出越少",
                        self.settings_slider(
                            BUBBLE_OPACITY_KEY,
                            BUBBLE_OPACITY_MIN,
                            BUBBLE_OPACITY_MAX,
                            BUBBLE_OPACITY_STEP,
                            BUBBLE_OPACITY_DEFAULT,
                            BUBBLE_SLIDER_WIDTH,
                            "%",
                            "Setting_shell_bubbleOpacity",
                            "气泡不透明度",
                            context,
                            p,
                        ),
                        p,
                    ),
                ],
                p,
            ),
            self.settings_card(
                "skin",
                "背景皮肤",
                "导入图片作为写代码时的内容区背景，用滑杆调到既能看出图、又不吃正文的浓度",
                vec![
                    self.settings_row("skin-image", "背景图", "png / jpg / webp / bmp", buttons, p),
                    self.settings_row(
                        "skin-opacity",
                        "背景透明度",
                        "拖动即时生效；没有导入图片时先记下，导入后按这个浓度显示",
                        self.settings_slider(
                            "skin_opacity",
                            size::SKIN_OPACITY_MIN,
                            size::SKIN_OPACITY_MAX,
                            size::SKIN_OPACITY_STEP,
                            size::SKIN_OPACITY_DEFAULT,
                            size::SKIN_SLIDER_WIDTH,
                            "%",
                            "SkinOpacitySlider",
                            "背景透明度",
                            context,
                            p,
                        ),
                        p,
                    ),
                ],
                p,
            ),
        ]
    }

    /// 关于：`MainWindow.About.cs`。版本取自 MSIX/程序集，分叉两者都没有 ⇒ 「未知」。
    fn settings_about(&self, p: Palette) -> Vec<KeyedView> {
        let unknown =
            |text: &str, p: Palette| -> View { self.settings_code_text(text.to_string(), p) };
        let actions: View = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(space::S8)
            .children((self.settings_button("检查更新", "AboutCheckUpdateButton", false),))
            .into();
        vec![
            self.settings_section_desc(DESC_ABOUT, p),
            self.settings_card(
                "version",
                "版本",
                "",
                vec![
                    self.settings_row(
                        "shell-version",
                        "当前版本",
                        "壳版本（打包形态与安装包版本一致）",
                        unknown("未知", p),
                        p,
                    ),
                    self.settings_row(
                        "kernel-version",
                        "内核版本",
                        "随包发行的 dsh 内核版本",
                        unknown("未知", p),
                        p,
                    ),
                ],
                p,
            ),
            self.settings_card(
                "update",
                "更新",
                "",
                vec![
                    self.settings_line(
                        "repo",
                        "更新源尚未配置：发布到 GitHub 后填入仓库地址即可启用在线检查更新。",
                        p,
                    ),
                    self.settings_row("release", "GitHub Release", "", actions, p),
                ],
                p,
            ),
        ]
    }

    /// 宠物：`MainWindow.Pet.cs:189`。分叉没有 `/api/pet/*` 通道，因此停在主干
    /// 「数据未回」那一帧：控件用默认值、宠物库给占位行、诊断只保留与状态无关的指引。
    fn settings_pet(&self, context: &mut ViewContext<Shell>, p: Palette) -> Vec<KeyedView> {
        let command: View = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(space::S8)
            .children((
                self.settings_text(
                    "pet_command",
                    "",
                    "petdex install <宠物标识>",
                    "安装命令",
                    context,
                ),
                self.settings_button("安装", "PetInstallButton", true),
            ))
            .into();
        vec![
            self.settings_section_desc(DESC_PET, p),
            self.settings_card(
                "pet-display",
                "显示",
                "宠物显示在聊天窗口右下角，跟着模型的工作状态切换动画；点它可以逗一逗。",
                vec![
                    self.settings_row(
                        "pet-visible",
                        "显示宠物",
                        "关掉后窗口里不再显示，插件其余功能照常",
                        self.settings_toggle("pet_visible", false, "显示宠物", context),
                        p,
                    ),
                    self.settings_row(
                        "pet-size",
                        "宠物大小",
                        "单元格高度的像素值，拖动即时生效",
                        self.settings_slider(
                            "pet_size",
                            size::PET_CELL_MIN,
                            size::PET_CELL_MAX,
                            size::PET_CELL_STEP,
                            size::PET_CELL_DEFAULT,
                            size::PET_SLIDER_WIDTH,
                            // 主干原文是 `LF("{0} px")`：空格在单位串里（`settings_slider` 不再自己补）
                            " px",
                            "PetSizeSlider",
                            "宠物大小",
                            context,
                            p,
                        ),
                        p,
                    ),
                ],
                p,
            ),
            self.settings_card(
                "pet-library",
                "宠物库",
                "已收录的宠物（插件内置 + ~/.codex/pets + 本机安装的）。切换立即生效。",
                vec![self.settings_line("placeholder", "正在读取宠物清单…", p)],
                p,
            ),
            self.settings_card(
                "pet-install",
                "安装宠物",
                "兼容 Codex 宠物包（pet.json + 图集）。拖放 zip 到下方区域，或粘贴安装命令。",
                vec![
                    KeyedView::new(
                        "line-drop",
                        Border::new().padding(th(pad::SETTINGS_ROW)).content(
                            Border::new()
                                .background(p.card_secondary)
                                .border_brush(p.stroke)
                                .border_thickness(th([size::STROKE; 4]))
                                .corner_radius(radius::MEDIUM)
                                .padding(th([16.0; 4]))
                                .content(
                                    StackPanel::new()
                                        .orientation(Orientation::Vertical)
                                        .spacing(space::S8)
                                        .keyed_children(vec![
                                            KeyedView::new(
                                                "title",
                                                TextBlock::new()
                                                    .text(self.catalog.l("拖放以安装"))
                                                    .font_size(type_ramp::BODY_STRONG.size)
                                                    .font_weight(FontWeight::SEMI_BOLD)
                                                    .foreground(p.text_primary),
                                            ),
                                            KeyedView::new(
                                                "desc",
                                                self.settings_note(
                                                    "把 Codex 宠物 zip 从文件资源管理器拖到这里即可安装",
                                                    p,
                                                ),
                                            ),
                                            KeyedView::new(
                                                "button",
                                                self.settings_button("浏览并安装", "PetBrowseButton", true),
                                            ),
                                        ]),
                                ),
                        ),
                    ),
                    self.settings_row("pet-command", "安装命令", "", command, p),
                ],
                p,
            ),
            self.settings_card(
                "pet-diagnostics",
                "诊断",
                "宠物不显示时先看这里：插件在不在、注册表报了什么、壳能不能画。",
                vec![
                    self.settings_line("registry", "正在读取注册表诊断…", p),
                    self.settings_line(
                        "webp",
                        "若宠物位置显示不出来：Windows 需要 WebP 映像扩展才能解 .webp 图集，缺失时壳会退到预览 GIF，再不行就连精灵一起隐藏。",
                        p,
                    ),
                ],
                p,
            ),
        ]
    }

    /// 技能：`MainWindow.Skills.cs:83`。数据是本地目录扫描，分叉不扫盘 ⇒ 落到空态文案。
    fn settings_skills(&self, p: Palette) -> Vec<KeyedView> {
        vec![
            self.settings_section_desc(DESC_SKILLS, p),
            self.settings_card(
                "skills-overview",
                "已安装技能",
                "按内核 @deepseek-ai/dsh-skill-filesystem 的根目录与优先级扫描，与内核看到的是同一份真相。",
                vec![
                    self.settings_line(
                        "priority",
                        "同名技能只生效优先级最高的一个：项目 > 预设 > 用户；被遮蔽的条目在列表里标注。",
                        p,
                    ),
                    KeyedView::new(
                        "line-add",
                        Border::new()
                            .padding(th([0.0, 8.0, 0.0, 4.0]))
                            .content(self.settings_button("添加技能", "SkillsAddButton", true)),
                    ),
                    self.settings_line(
                        "empty",
                        "还没有安装任何技能。用上面的「添加技能」写第一个，或在项目的 .dsh/skills 目录里放一个。",
                        p,
                    ),
                ],
                p,
            ),
        ]
    }

    /// Agent 预设：`MainWindow.xaml.cs:12014`。清单来自 `agentPresets/list`，分叉拿不到
    /// ⇒ 只剩段说明与「预设目录」卡（主干在 RPC 失败时同样是这个形态）。
    fn settings_agent_presets(&self, p: Palette) -> Vec<KeyedView> {
        vec![
            self.settings_section_desc(DESC_AGENT_PRESETS, p),
            self.settings_card(
                "preset-dir",
                "预设目录",
                "本部署无法原生打开预设目录（settings/canOpenAgentPresetDirectory 返回 false）。",
                vec![self.settings_row(
                    "preset-dir-can-add",
                    "新增自定义预设",
                    "",
                    self.settings_code_text("不可用".into(), p),
                    p,
                )],
                p,
            ),
        ]
    }

    /// 模型：`RenderModelsSectionAsync`（`MainWindow.xaml.cs:9883`）。提供方清单走
    /// `llm/listConfigurableProviders` + `credentials/describe`，分叉没有这两条端点
    /// ⇒ 只剩段说明与「添加提供方」占位（主干的虚线描边 reactor 也画不出来）。
    fn settings_models(&self, p: Palette) -> Vec<KeyedView> {
        let add: View = Border::new()
            .background(p.card_secondary)
            .border_brush(p.stroke)
            .border_thickness(th([size::STROKE; 4]))
            .corner_radius(radius::MEDIUM)
            .min_height(44.0)
            .padding(th(pad::CARD_COMPACT))
            .content(
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(space::S6)
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .children((
                        // 主干 `MakeAddProviderButton`（xaml.cs:10542-10546）：E710 @ GlyphBody(14)
                        // + 默认 BodyTextStyle（14/主文本色）的「添加提供方」，两者都垂直居中。
                        mark_vcenter(glyph::NEW_SESSION, size::GLYPH_BODY),
                        TextBlock::new()
                            .text(self.catalog.l("添加提供方"))
                            .font_size(type_ramp::BODY.size)
                            .foreground(p.text_primary.clone())
                            .vertical_alignment(VerticalAlignment::Center),
                    )),
            )
            .into();
        vec![
            self.settings_section_desc(DESC_MODELS, p),
            KeyedView::new(
                "add-provider",
                Border::new().padding(th(pad::SETTINGS_ROW)).content(add),
            ),
        ]
    }

    /// 使用统计：`RenderUsageSectionAsync`（`MainWindow.xaml.cs:13689`）。
    /// 热力图与趋势图是逐像素画的 Canvas 子节点，分叉没有内核 journal 数据，只保留卡壳与空态文案；
    /// 两条 SelectorBar（统计口径 / 时间范围）reactor 0.100 装不进条目，已知抄不动。
    fn settings_usage(&self, p: Palette) -> Vec<KeyedView> {
        let mut cells: Vec<KeyedView> = Vec::new();
        let kpis: Vec<KeyedView> = STATS_KPI_CARDS
            .iter()
            .enumerate()
            .map(|(index, (title, code, id, hint))| {
                let label: View = TextBlock::new()
                    .text(self.catalog.l(title))
                    .font_size(type_ramp::CAPTION.size)
                    .foreground(p.text_secondary)
                    .vertical_alignment(VerticalAlignment::Center)
                    .into();
                KeyedView::new(
                    format!("kpi-{id}"),
                    Border::new()
                        .grid_column(index as i32)
                        .background(p.card)
                        .border_brush(p.stroke)
                        .border_thickness(th([size::STROKE; 4]))
                        .corner_radius(radius::MEDIUM)
                        .padding(th(pad::CARD_WIDE))
                        .content(
                            StackPanel::new()
                                .orientation(Orientation::Vertical)
                                .spacing(space::S6)
                                .min_height(size::KPI_MIN_HEIGHT)
                                .keyed_children(vec![
                                    KeyedView::new(
                                        "label",
                                        StackPanel::new()
                                            .orientation(Orientation::Horizontal)
                                            .spacing(space::S6)
                                            .children((
                                                // 主干 `MainWindow.xaml:1396-1399`：KPI 图标
                                                // FontSize=GlyphSizeBody(14)、VerticalAlignment Center。
                                                mark_vcenter(*code, size::GLYPH_BODY),
                                                label,
                                            )),
                                    ),
                                    KeyedView::new(
                                        "value",
                                        TextBlock::new()
                                            .text("—")
                                            .font_size(type_ramp::SUBTITLE.size)
                                            .foreground(p.text_primary)
                                            .margin(th([0.0, 10.0, 0.0, 0.0]))
                                            .automation_id((*id).to_string()),
                                    ),
                                    KeyedView::new(
                                        "hint",
                                        TextBlock::new()
                                            .text(self.catalog.l(hint))
                                            .font_size(type_ramp::CAPTION.size)
                                            .foreground(p.text_secondary)
                                            .automation_id(format!("{id}Hint")),
                                    ),
                                ]),
                        ),
                )
            })
            .collect();
        // 主干是一行 5 列 `Width="*"` 的 Grid，不是 5 张竖卡。
        cells.push(KeyedView::new(
            "kpi-row",
            Grid::new()
                .columns([GridLength::STAR; 5])
                .column_spacing(space::S12)
                .keyed_children(kpis),
        ));
        cells.push(self.settings_card(
            "heat",
            "Token 活动",
            "近 26 周的按日活动；口径可切每日 / 每周 / 累计。",
            vec![self.settings_line("heat-empty", "所选时间范围内暂无用量记录", p)],
            p,
        ));
        cells.push(self.settings_card(
            "trend",
            "每日 Token 趋势图",
            "",
            vec![self.settings_line("trend-empty", "所选时间范围内暂无用量记录", p)],
            p,
        ));
        cells.push(self.settings_card(
            "model-usage",
            "模型用量",
            "",
            vec![self.settings_line("model-empty", "所选时间范围内暂无用量记录", p)],
            p,
        ));
        cells.push(self.settings_line(
            "source",
            "数据来源：内核 journal（session/list + session/page）的用量记录；已聚合 0 个非空会话、0 条记录。「累计/峰值/连续天数」按全部历史，「时间范围」只作用于趋势图与模型用量；费用/金额内核未提供对应字段，故不展示。",
            p,
        ));
        cells
    }

    /// 主干 `MakeNumberBox`：MinWidth=MaxWidth=FieldWidth(320)、Compact 步进钮。
    /// reactor 的 NumberBox 没有 `spin_button_placement_mode` setter，步进钮只能吃默认 Hidden。
    fn settings_number(
        &self,
        id: &str,
        fallback: f64,
        name: &str,
        context: &mut ViewContext<Shell>,
    ) -> View {
        let key = id.to_string();
        NumberBox::new()
            .value(self.number(&key, fallback))
            .min_width(size::FIELD_WIDTH)
            .max_width(size::FIELD_WIDTH)
            .automation_id(format!("Setting_{id}"))
            .automation_name(name.to_string())
            .on_value_changed(
                context.callback(move |value: Option<f64>| {
                    Msg::Number(key.clone(), value.unwrap_or(0.0))
                }),
            )
            .into()
    }

    /// 主干 `MakeTextBox`：同档宽度 320，占位文案照抄。
    fn settings_text(
        &self,
        id: &str,
        current: &str,
        placeholder: &str,
        name: &str,
        context: &mut ViewContext<Shell>,
    ) -> View {
        let key = id.to_string();
        TextBox::new()
            .text(self.field(&key, current))
            .placeholder_text(self.catalog.l(placeholder))
            .min_width(size::FIELD_WIDTH)
            .max_width(size::FIELD_WIDTH)
            .automation_id(format!("Setting_{id}"))
            .automation_name(name.to_string())
            .on_text_changed(context.callback(move |value: String| Msg::Field(key.clone(), value)))
            .into()
    }

    /// 主干 `MakeRow()`：根 Grid `[1*,Auto]`、列距 24、行距 8、Padding 0,12,0,12；
    /// 左列是 Spacing 2 的垂直 StackPanel，先标题后描述；右控件仅 Column=1 + VerticalAlignment=Center。
    fn settings_row(
        &self,
        key: &str,
        title: &str,
        desc: &str,
        control: View,
        p: Palette,
    ) -> KeyedView {
        let heading: View = TextBlock::new()
            .text(self.catalog.l(title))
            .font_size(type_ramp::BODY.size)
            .foreground(p.text_primary)
            .text_wrapping(TextWrapping::Wrap)
            .into();
        let mut left: Vec<KeyedView> = vec![KeyedView::new("title", heading)];
        if !desc.is_empty() {
            left.push(KeyedView::new("desc", self.settings_note(desc, p)));
        }
        KeyedView::new(
            format!("row-{key}"),
            Border::new().padding(th(pad::SETTINGS_ROW)).content(
                Grid::new()
                    .columns([GridLength::STAR, GridLength::Auto])
                    .column_spacing(space::S24)
                    .children((
                        StackPanel::new()
                            .orientation(Orientation::Vertical)
                            .spacing(space::S2)
                            .vertical_alignment(VerticalAlignment::Center)
                            .keyed_children(left),
                        Border::new()
                            .grid_column(1)
                            .vertical_alignment(VerticalAlignment::Center)
                            .content(control),
                    )),
            ),
        )
    }

    /// `CaptionTextStyle` + TextSecondary + Wrap：行描述与卡片副标题共用，靠调用点给边距。
    fn settings_note(&self, text: &str, p: Palette) -> View {
        TextBlock::new()
            .text(self.catalog.l(text))
            .font_size(type_ramp::CAPTION.size)
            .foreground(p.text_secondary)
            .text_wrapping(TextWrapping::Wrap)
            .into()
    }

    /// 主干设置卡片：CardBackgroundFillColorDefault 底 + 1px 描边 + 8 圆角 + 16,5,16,5 内边距；
    /// 卡头 0,10,0,2、副标题 0,0,0,4，行与行之间一条通栏分隔线。
    fn settings_card(
        &self,
        key: &str,
        header: &str,
        subtitle: &str,
        rows: Vec<KeyedView>,
        p: Palette,
    ) -> KeyedView {
        let mut cells: Vec<KeyedView> = Vec::new();
        if !header.is_empty() {
            let heading: View = TextBlock::new()
                .text(self.catalog.l(header))
                .font_size(type_ramp::BODY.size)
                .font_weight(FontWeight::SEMI_BOLD)
                .foreground(p.text_primary)
                .margin(th(pad::CARD_HEADER))
                .into();
            cells.push(KeyedView::new("header", heading));
        }
        if !subtitle.is_empty() {
            let sub: View = Border::new()
                .margin(th(pad::CARD_DESCRIPTION))
                .content(self.settings_note(subtitle, p))
                .into();
            cells.push(KeyedView::new("subtitle", sub));
        }
        let count = rows.len();
        for (index, row) in rows.into_iter().enumerate() {
            cells.push(row);
            if index + 1 < count {
                cells.push(self.settings_divider(format!("div-{index}"), p));
            }
        }
        KeyedView::new(
            format!("card-{key}"),
            Border::new()
                .background(p.card)
                .border_brush(p.stroke)
                .border_thickness(th([size::STROKE; 4]))
                .corner_radius(radius::MEDIUM)
                .padding(th(pad::CARD_BORDER))
                .content(
                    StackPanel::new()
                        .orientation(Orientation::Vertical)
                        .keyed_children(cells),
                ),
        )
    }

    /// 卡片行分隔线：Rectangle 1px、StrokeSubtle、左右各 -16 抵消卡片内边距。
    fn settings_divider(&self, key: String, p: Palette) -> KeyedView {
        KeyedView::new(
            key,
            Border::new()
                .height(size::STROKE)
                .background(p.stroke_subtle)
                .margin(th(pad::DIVIDER)),
        )
    }

    /// 主干 `Aut(combo, "Setting_{ns}_{field}", 行标题)`：Name 落在控件上而不是标题上。
    fn settings_combo(
        &self,
        id: &str,
        options: &[&str],
        fallback: usize,
        name: &str,
        context: &mut ViewContext<Shell>,
    ) -> View {
        let key = id.to_string();
        let index = self.pick(&key, fallback);
        ComboBox::new()
            .items_source(options.iter().map(|item| (*item).to_string()))
            .selected_index(Some(index))
            .min_width(size::FIELD_MIN_WIDTH)
            .automation_id(format!("Setting_{id}"))
            .automation_name(name.to_string())
            .on_selection_changed(
                context.callback(move |value: Option<usize>| {
                    Msg::Pick(key.clone(), value.unwrap_or(0))
                }),
            )
            .into()
    }

    /// reactor 0.100 的 ToggleSwitch 没有 OnContent/OffContent setter，开关文案只能吃模板默认值。
    fn settings_toggle(
        &self,
        id: &str,
        fallback: bool,
        name: &str,
        context: &mut ViewContext<Shell>,
    ) -> View {
        let key = id.to_string();
        ToggleSwitch::new()
            .is_on(self.is_on(&key, fallback))
            .automation_id(format!("Setting_{id}"))
            .automation_name(name.to_string())
            .on_toggled(context.callback(move |value: bool| Msg::Toggle(key.clone(), value)))
            .into()
    }

    /// 通用设置：主干 `RenderGeneralSection()` 的 7 张卡（通知/托盘/维护卡也在内）。
    fn settings_general(&self, context: &mut ViewContext<Shell>, p: Palette) -> Vec<KeyedView> {
        let mut cells: Vec<KeyedView> = Vec::new();
        cells.push(KeyedView::new(
            "section-desc",
            Border::new()
                .margin(th(pad::CARD_DESCRIPTION))
                .content(self.settings_note("新会话的默认权限、外观与对话偏好。", p)),
        ));
        cells.push(self.settings_card(
            "permission",
            "权限",
            "选择新会话的默认访问模式",
            vec![self.settings_row(
                "permission",
                "默认权限",
                "仅可查看 / 工作区内修改 / 完全权限",
                self.settings_combo(
                    "permission_defaultPreset",
                    SETTINGS_PERMISSIONS,
                    1,
                    "默认权限",
                    context,
                ),
                p,
            )],
            p,
        ));
        cells.push(self.settings_card(
            "appearance",
            "外观",
            "主题与会话正文字号",
            vec![
                self.settings_row(
                    "theme",
                    "主题",
                    "浅色 / 深色 / 跟随系统",
                    self.settings_combo("ui-theme_preference", SETTINGS_THEMES, 2, "主题", context),
                    p,
                ),
                self.settings_row(
                    "font",
                    "字号大小",
                    "仅影响会话内容的字号",
                    self.font_stepper(context, p),
                    p,
                ),
            ],
            p,
        ));
        cells.push(self.settings_card(
            "chat",
            "对话",
            "已完成轮次的展示方式与繁忙时的发送行为",
            vec![
                self.settings_row(
                    "density",
                    "对话显示",
                    "标准显示过程内容；紧凑只保留结果",
                    self.settings_combo(
                        "ui-chat_transcriptView",
                        &["标准", "紧凑"],
                        1,
                        "对话显示",
                        context,
                    ),
                    p,
                ),
                self.settings_row(
                    "busy",
                    "繁忙时的发送行为",
                    "智能体运行时 Enter 键和发送按钮的行为；Ctrl+Enter 使用另一行为",
                    self.settings_combo(
                        "ui-conversation_busyEnter",
                        &["排队发送", "插话发送"],
                        0,
                        "繁忙时的发送行为",
                        context,
                    ),
                    p,
                ),
            ],
            p,
        ));
        cells.push(self.settings_card(
            "language",
            "语言",
            "界面显示语言",
            vec![self.settings_row(
                "language",
                "语言",
                "",
                self.settings_combo("locale_preference", SETTINGS_LOCALES, 0, "语言", context),
                p,
            )],
            p,
        ));
        cells.push(self.settings_card(
            "notifications",
            "系统通知",
            "任务完成、审批请求等关键时刻的 Windows 通知",
            vec![self.settings_row(
                "notifications",
                "启用系统通知",
                "窗口不在前台时提醒；点击通知可回到 Blade²",
                self.settings_toggle("tray_notifications", true, "启用系统通知", context),
                p,
            )],
            p,
        ));
        cells.push(self.settings_card(
            "tray",
            "托盘与退出",
            "最小化 / 关闭窗口时的去向",
            vec![
                self.settings_row(
                    "show-icon",
                    "显示托盘图标",
                    "关闭后仍可从托盘恢复窗口；托盘右键菜单可退出",
                    self.settings_toggle("tray_showIcon", true, "显示托盘图标", context),
                    p,
                ),
                self.settings_row(
                    "minimize",
                    "最小化时隐藏到托盘",
                    "点最小化按钮后窗口藏进托盘，不占任务栏",
                    self.settings_toggle(
                        "tray_minimizeToTray",
                        false,
                        "最小化时隐藏到托盘",
                        context,
                    ),
                    p,
                ),
                self.settings_row(
                    "close",
                    "关闭时最小化到托盘",
                    "点关闭按钮后窗口藏进托盘继续运行，用托盘菜单退出",
                    self.settings_toggle("tray_closeToTray", false, "关闭时最小化到托盘", context),
                    p,
                ),
            ],
            p,
        ));
        cells.push(self.settings_card(
            "maintenance",
            "设置文件",
            "",
            vec![
                self.settings_row(
                    "document",
                    "设置文档",
                    "把内核侧设置文档落到本地并用默认编辑器打开。",
                    self.compact_button(
                        "打开设置文档",
                        Msg::Note("分叉未接入 settings/openSettingsDocument".to_string()),
                        "OpenSettingsDocumentButton",
                        "打开设置文档",
                        context,
                    ),
                    p,
                ),
                self.settings_row(
                    "reset",
                    "恢复默认",
                    "清空本页用户设置：ui-theme、locale、ui-chat、ui-conversation、permission。",
                    self.compact_button(
                        "恢复本页默认",
                        Msg::Note("分叉未接入 settings/replace".to_string()),
                        "ResetSettingsSectionButton",
                        "恢复本页默认",
                        context,
                    ),
                    p,
                ),
            ],
            p,
        ));
        cells
    }

    /// 主干字号步进器：横向 StackPanel(Spacing 6)，`−` / `{n}px`(MinWidth 44 居中) / `＋`。
    fn font_stepper(&self, context: &mut ViewContext<Shell>, p: Palette) -> View {
        let value: View = Border::new()
            .min_width(size::STEPPER_VALUE_MIN_WIDTH)
            .content(
                TextBlock::new()
                    .text(format!("{}px", self.font_size as i32))
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .font_size(type_ramp::BODY.size)
                    .foreground(p.text_primary)
                    .automation_id("FontSizeValue")
                    .automation_name(format!("当前字号 {}px", self.font_size as i32)),
            )
            .into();
        StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(space::S6)
            .children((
                self.compact_button(
                    "−",
                    Msg::FontStep(-1.0),
                    "FontSizeDecreaseButton",
                    "减小字号",
                    context,
                ),
                value,
                self.compact_button(
                    "＋",
                    Msg::FontStep(1.0),
                    "FontSizeIncreaseButton",
                    "增大字号",
                    context,
                ),
            ))
    }

    /// 主干 `CompactButtonStyle`：字号 12、Padding 10,4,10,4、MinHeight 32、圆角 4、垂直居中。
    /// 模板不读 ButtonPadding 资源键，只能显式设字号并用负边距抵掉默认 11,5,11,6。
    fn compact_button(
        &self,
        label: &str,
        message: Msg,
        id: &str,
        name: &str,
        context: &mut ViewContext<Shell>,
    ) -> View {
        Button::new()
            .min_width(0.0)
            .min_height(size::TOUCH_TARGET)
            .vertical_alignment(VerticalAlignment::Center)
            .resource_overrides(
                ResourceOverrides::new()
                    .set("ControlCornerRadius", CornerRadius::uniform(radius::SMALL))
                    .set("ButtonBorderBrush", Color::transparent())
                    .set("ButtonBorderBrushPointerOver", Color::transparent()),
            )
            .automation_id(id.to_string())
            .automation_name(name.to_string())
            .on_click(context.message(message))
            .content(
                Border::new()
                    .margin(th([
                        pad::INLINE_BUTTON[0] - 11.0,
                        pad::INLINE_BUTTON[1] - 5.0,
                        pad::INLINE_BUTTON[2] - 11.0,
                        pad::INLINE_BUTTON[3] - 6.0,
                    ]))
                    .content(
                        TextBlock::new()
                            .text(self.catalog.l(label))
                            .font_size(type_ramp::CAPTION.size),
                    ),
            )
            .into()
    }
}

/// 主干 `BrandImageSource()` 的文件优先分支：向上找仓库 Assets。
fn brand_mark_path() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let mut dir: &Path = exe.parent()?;
    for _ in 0..6 {
        let candidate = dir.join("Assets").join("Square44x44Logo.png");
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = dir.parent()?;
    }
    None
}

fn main() {
    App::run_component::<Shell>(()).unwrap();
    // 窗口关了也要把钩子从链上摘掉（漏了它 = 这一枪永远挂在钩子链上）。`Unhooked` 三档见 `keys`：
    // 真卸 / 没装过 / 换了线程（线程钩子留给线程退出时由 OS 摘）。走 stdout，界面不沾。
    let unhooked = keys::uninstall();
    println!("KBHOOK unhooked={unhooked:?}");
}

#[cfg(test)]
mod pill_tests {
    use super::*;

    /// 圆底图标钮的圆角必须夹到短边一半 —— 这是「正圆」的全部定义：
    /// 主干在 Button 实例上写 `CornerRadius=CornerPill(24)`，WinUI 自己夹；
    /// 分叉的圆是自己画的 Border，漏夹就会渲染成圆角方块（tmp/mycheck-add.png 那次）。
    #[test]
    fn pill_radius_clamps_to_half_side() {
        let p = Palette::for_scheme(Scheme::Dark);
        let add = Pill::card(p);
        assert_eq!(add.radius, radius::PILL);
        assert_eq!(add.radius_clamped(), size::COMPOSER_ADD_BUTTON / 2.0);
        let send = Pill::accent(p, size::COMPOSER_SEND_BUTTON);
        assert_eq!(send.radius_clamped(), size::COMPOSER_SEND_BUTTON / 2.0);
        // `IconButtonStyle` 那一族（32 DIP 小圆角方）按自己的档位走，不被夹。
        assert_eq!(Pill::ghost(p).radius_clamped(), radius::SMALL);
        // 会话行那颗 `SessionMore`：主干 `MainWindow.xaml.cs:3269-3270` 写的是
        // `CornerRadius = new(TokenDouble("RadiusPill", 24))` 在 24×24 的钮上
        // ⇒ 夹到短边一半 = 12 DIP，200% 缩放下是 **48 px 直径的正圆**，不是 r=4 圆角方。
        let more = Pill {
            side: size::ROW_MORE_BUTTON,
            radius: radius::PILL,
            ..Pill::ghost(p)
        };
        assert_eq!(
            more.radius_clamped(),
            size::ROW_MORE_BUTTON / 2.0,
            "SessionMore 必须是正圆：半径 = 边长一半"
        );
        assert_eq!(
            more.radius_clamped() * 2.0 * 2.0,
            48.0,
            "半径 12 DIP ⇒ 直径 24 DIP ⇒ 200% 缩放（px = DIP×2）48 px 正圆"
        );
    }

    /// 发送钮四态：静置 1.0、悬停 0.9、按下 0.8（generic.xaml:7631-7633），
    /// 底色始终是同一支主题强调色画刷（不写死：跟用户个性化色走）。
    #[test]
    fn accent_pill_fades_its_four_states() {
        let p = Palette::for_scheme(Scheme::Dark);
        let send = Pill::accent(p, size::COMPOSER_SEND_BUTTON);
        assert_eq!(send.rest.brush, Brush::Theme(ThemeBrush::Accent));
        assert_eq!(send.hover.brush, send.rest.brush);
        assert_eq!(send.pressed.brush, send.rest.brush);
        assert_eq!((send.rest.opacity, send.hover.opacity, send.pressed.opacity), (1.0, 0.9, 0.8));
        assert_eq!(send.disabled, Some(PillFace::solid(p.accent_disabled)));
        assert_eq!(send.foreground, Some(p.on_accent_color));
    }

    /// 操作行那三颗裸 `Button`（copy / 分支 / 撤回编辑）= 主干默认模板的一整套四档：
    /// 静置 `ControlFillColorDefault` + 1px 描边、悬停 Secondary、按下 Tertiary（描边转
    /// ControlStrokeColorDefault）、禁用 Disabled，**28×28 小圆角方**（半径 4，不被夹成圆）。
    /// 上一版静置是全透明 ⇒ 浅色下悬停那层白叠加（#80F9F9F9）在浅底上等于没变，才「看不出 hover」。
    #[test]
    fn action_pill_carries_the_default_template_tiers() {
        for scheme in [Scheme::Light, Scheme::Dark] {
            let p = Palette::for_scheme(scheme);
            let action = Pill::action(p, true);
            assert_eq!(
                (action.side, action.radius, action.radius_clamped()),
                (size::SMALL_BUTTON, radius::SMALL, radius::SMALL),
                "主干 RadSmall(4) 在 28 的短边一半(14)以内 ⇒ 方角，不是正圆"
            );
            assert_eq!(action.border, size::STROKE);
            assert_eq!(action.rest, PillFace::outlined(p.control_fill, p.control_stroke));
            assert_eq!(action.hover, PillFace::outlined(p.control_hover, p.control_stroke));
            assert_eq!(
                action.pressed,
                PillFace::outlined(p.control_pressed, p.control_stroke_pressed)
            );
            assert_eq!(
                action.disabled,
                Some(PillFace::outlined(
                    p.control_disabled,
                    p.control_stroke_pressed
                ))
            );
            // 静置底不能是透明，否则浅色下整族钮「化」进背景里（本轮的实测教训）。
            assert_ne!(action.rest.brush, p.transparent);
            // 三档底互不相同，否则逐态截图没法作证。
            assert_ne!(action.rest.brush, action.hover.brush);
            assert_ne!(action.hover.brush, action.pressed.brush);
            // composer 的 + / 返回 / 工作区筛选钮那一族：**无描边 + 静置透明**，
            // 但悬停/按下是主干**实渲**的 Control 链（模板 VisualState 抢在实例 Background 之前）
            let ghost = Pill::ghost(p);
            assert_eq!(ghost.border, 0.0);
            assert_eq!(ghost.rest.brush, p.subtle_rest);
            assert_eq!(ghost.hover.brush, p.control_hover);
            assert_eq!(ghost.pressed.brush, p.control_pressed);
            assert_eq!(ghost.disabled, Some(PillFace::solid(p.subtle_disabled)));
            assert_eq!(
                ghost.rest.brush,
                Brush::Solid(Color {
                    a: 0,
                    r: 255,
                    g: 255,
                    b: 255
                })
            );
        }
    }

    /// **图标钮三态取哪条链**的判据。口径 = 主干**屏幕上真正渲染出来**的那两档，
    /// 不是主干 XAML 里写的令牌名（逐颗来源见 `Pill::ghost` / `Pill::card` 的注释）：
    /// · 主干写 `Background="Transparent"`（`IconButtonStyle` 全族 + 会话行 `moreBtn`）
    ///   或 `CardHoverBrush`（`ComposerAddButton`，`Tokens.xaml:164/249` = `SubtleFillColorSecondary`）
    ///   **且** `BorderThickness="0"` 的那些 ⇒ 静置按主干写的值（respectively 全透明 / Subtle Secondary），
    ///   但悬停、按下两档被默认 Button 模板的 VisualState Setter 抢走 =
    ///   `ControlFillColorSecondary` / `Tertiary`（浅 `#80F9F9F9` / `#4DF9F9F9`、
    ///   深 `#15FFFFFF` / `#08FFFFFF`）⇒ 分叉自绘层也必须走 `control_hover`/`control_pressed`。
    ///   上一轮按「设计意图」走了 `subtle_hover`/`subtle_pressed`（浅色是**黑**叠加），
    ///   方向与主干实渲相反（主干越悬停越白，分叉越悬停越灰），本轮按像素改回来。
    /// · Control 家族（主干**没写** `Background`/`BorderThickness` 的裸 `Button`/`ToggleButton`：
    ///   `MakeCopyButton`、轮尾 `MessageBranchButton`、`MakeWithdrawEditButton`）
    ///   ⇒ `Pill::action` 走 `control_*` + 1px 描边，本轮不动。
    #[test]
    fn icon_pills_render_the_mainline_control_chain() {
        let face = |brush: Brush| match brush {
            Brush::Solid(color) => (color.a, color.r, color.g, color.b),
            Brush::Theme(_) => (255, 0, 0, 0),
        };
        for scheme in [Scheme::Light, Scheme::Dark] {
            let p = Palette::for_scheme(scheme);
            // (静置, 悬停, 按下) 的 ARGB 全值：静置两族各异，悬停/按下共用 Control 链。
            let (want_ghost_rest, want_card_rest, want_hover, want_pressed) = match scheme {
                Scheme::Light => (
                    (0x00, 0xFF, 0xFF, 0xFF),
                    (0x09, 0x00, 0x00, 0x00),
                    (0x80, 0xF9, 0xF9, 0xF9),
                    (0x4D, 0xF9, 0xF9, 0xF9),
                ),
                Scheme::Dark => (
                    (0x00, 0xFF, 0xFF, 0xFF),
                    (0x0F, 0xFF, 0xFF, 0xFF),
                    (0x15, 0xFF, 0xFF, 0xFF),
                    (0x08, 0xFF, 0xFF, 0xFF),
                ),
            };
            let ghost = Pill::ghost(p);
            assert_eq!(face(ghost.rest.brush), want_ghost_rest);
            assert_eq!(face(ghost.hover.brush), want_hover);
            assert_eq!(face(ghost.pressed.brush), want_pressed);
            // ComposerAddButton 静置就带着主干那层浅灰圆底（`CardHoverBrush`）
            let card = Pill::card(p);
            assert_eq!(
                face(card.rest.brush),
                want_card_rest,
                "静置档 = 主干的 CardHoverBrush = SubtleFillColorSecondary"
            );
            assert_eq!(face(card.hover.brush), want_hover);
            assert_eq!(face(card.pressed.brush), want_pressed);
            for pill in [ghost, card] {
                assert_eq!(pill.border, 0.0, "这一族主干一律 BorderThickness=0");
                // 悬停/按下必须钉在 Control 链上，且**不得**退回 Subtle 链（上一轮的方向错误）
                assert_eq!(pill.hover.brush, p.control_hover);
                assert_eq!(pill.pressed.brush, p.control_pressed);
                assert_ne!(pill.hover.brush, p.subtle_hover, "不得退回 Subtle 悬停档");
                assert_ne!(pill.pressed.brush, p.subtle_pressed, "不得退回 Subtle 按下档");
                // 三态互不相同，逐态截图才作证得了
                assert_ne!(pill.rest, pill.hover);
                assert_ne!(pill.hover, pill.pressed);
                assert_ne!(pill.rest, pill.pressed);
                // Control 链方向：两条链都是白叠加，**按下比悬停浅一档**（alpha 递减）。
                // 别按「按下要比悬停更深」去改 —— 主干实渲就是这个方向。
                let (hover, pressed) = (face(pill.hover.brush), face(pill.pressed.brush));
                assert!(
                    hover.1 == hover.2 && hover.2 == hover.3 && pressed.1 == pressed.3,
                    "悬停/按下两档都是灰阶叠加：{hover:?} / {pressed:?}"
                );
                assert!(hover.0 > pressed.0, "pressed 必须比 hover 浅一档");
                assert!(hover.0 > face(pill.rest.brush).0);
            }
        }
        // Control 族那一头仍然钉在 control_*（本轮没顺手改坏它）
        let p = Palette::for_scheme(Scheme::Light);
        let action = Pill::action(p, true);
        assert_eq!(
            (action.hover.brush, action.pressed.brush),
            (p.control_hover, p.control_pressed)
        );
        assert_eq!(action.border, size::STROKE);
    }

    /// 自绘层抵内缩的那份常量（缺陷 2 的判据）：默认 Button 模板 `ContentPresenter` 的
    /// `Padding` = `11,5,11,6`（StaticResource，压不住）**加上**每边 1 DIP 的
    /// `ButtonBorderThemeThickness`（ThemeResource，绝不能覆 —— 塞盒装 `Thickness` 会
    /// 0xC000027B fail-fast）⇒ 要抵的是 `12,6,12,7`，不是上一版的 `11,5,11,6`。
    /// 少抵那 1 DIP，脸就比控件自己的 UIA 矩形每边小 2 px（200% 缩放），
    /// 上一轮实测：28 DIP 的 `+` UIA 56 px / 脸 52 px。
    /// 这一条查的是**源码本身**：五处自绘点必须统一取 `TEMPLATE_INSET`，
    /// 谁再手写一份字面量就当场红。
    #[test]
    fn self_drawn_pills_cancel_the_template_inset_once() {
        assert_eq!(TEMPLATE_INSET, [-12.0, -6.0, -12.0, -7.0]);
        // 常量 = -(Padding + 每边 1 DIP 描边)，逐边核对，四边都得是负的
        const PADDING: [f64; 4] = [11.0, 5.0, 11.0, 6.0];
        for side in 0..4 {
            assert_eq!(TEMPLATE_INSET[side], -(PADDING[side] + 1.0));
        }
        let src = include_str!("main.rs");
        // 五处：pill_button / nav_row / feedback_toggle / SettingsSubBack / PluginsNav_*
        // （探针字符串拼起来写，免得这条断言把自己也数进去）
        let probe = concat!("th(TEMPLATE_", "INSET)");
        assert_eq!(
            src.matches(probe).count(),
            5,
            "自绘点必须统一走 TEMPLATE_INSET，一处不多一处不少"
        );
        // 上一版那份只抵 Padding 的字面量不得复活（同样拼起来写，免得这条断言自己匹配自己）
        let stale = concat!("-11.0, -5.", "0, -11.0, -6.0");
        assert!(!src.contains(stale), "还有一处手写旧内缩：{stale}");
    }

    /// 行内说明编辑器的字节上限提示与内核那侧同源：文案走 `note-too-large{maxBytes}` 的查表结果，
    /// 计数按 **UTF-8 字节**（与假内核 `Buffer.byteLength` 口径一致），不是字符数。
    #[test]
    fn note_cap_is_counted_in_utf8_bytes() {
        let catalog = Catalog::load("zh", None);
        let error = json!({
            "code": "note-too-large",
            "maxBytes": MAX_NOTE_BYTES,
            "actualBytes": MAX_NOTE_BYTES + 1,
        });
        assert_eq!(
            feedback_error_text(&catalog, &error),
            format!("说明超长（上限 {MAX_NOTE_BYTES} 字节）")
        );
        assert_eq!(
            catalog.lf("{0}/{1} 字节", &["3".into(), MAX_NOTE_BYTES.to_string()]),
            "3/8192 字节"
        );
        let text = "说".repeat(2_731);
        assert_eq!((text.len(), text.chars().count()), (8_193, 2_731));
    }

    // ——— 加载卡 `KernelBootPanel` 的状态机（`BootState`）：不依赖 GUI 的自证 ———
    // 本轮禁止开窗口，所以把主线 `MainWindow.KernelBoot.cs` 的插值/区间/失败停留/`(seq)` 守卫
    // 全抽成纯函数后在这里逐条钉住。

    /// 四段区间表 `(5,30)(30,60)(60,82)(82,96)` 与步号归属；未登记的步号直接忽略（KC:85-89）。
    #[test]
    fn boot_stage_table_owns_four_intervals() {
        assert_eq!(
            kernel_boot::STAGES
                .iter()
                .map(|(_, step, from, to)| (*step, *from, *to))
                .collect::<Vec<(i32, f64, f64)>>(),
            vec![(1, 5.0, 30.0), (2, 30.0, 60.0), (3, 60.0, 82.0), (4, 82.0, 96.0)]
        );
        let mut boot = BootState::idle();
        boot.show();
        // 上屏瞬间：第 1 步、目标 = Stages[0].From = 5、已显示 = 0（KC:59-61）。
        assert_eq!((boot.step, boot.target, boot.shown), (1, 5.0, 0.0));
        assert!(boot.showing && !boot.done && boot.failure.is_none());
        for step in 2..=4 {
            boot.report_stage(step);
            let (_, _, from, to) = kernel_boot::STAGES[(step - 1) as usize];
            assert_eq!(boot.step, step);
            // 除第 1 步外没有中间量 ⇒ 目标值直接取区间**上沿**（KC:108-111）。
            assert_eq!(boot.target, to, "第 {step} 步目标值应为区间上沿 {to}，实际 {}", boot.target);
            assert_ne!(boot.target, from);
        }
        // 阶段名与步骤号同源（KC:91-93）：标题键必须就是表里那一行的 StageKey。
        assert_eq!(boot.stage_key(), "正在加载工作区与会话…");
        // 未登记的步号：不改步号、不动目标（宁可不显示也不糊弄）。
        boot.report_stage(9);
        assert_eq!((boot.step, boot.target), (4, 96.0));
        boot.report_stage(0);
        assert_eq!((boot.step, boot.target), (4, 96.0));
    }

    /// 100ms tick 的推进公式：每发最多 `RATE_FORWARD`(1.5)，到区间上沿后**吸住不再动**（KC:220-229）。
    #[test]
    fn boot_progress_crawls_at_forward_rate_and_snaps() {
        let mut boot = BootState::idle();
        boot.show();
        boot.report_stage(4); // target = 96
        let mut previous = boot.shown;
        let mut ticks = 0;
        while boot.shown < boot.target - kernel_boot::SNAP && ticks < 200 {
            let frame = boot.paint();
            // 单调前进，且单发步长不超过 1.5%
            let delta = boot.shown - previous;
            assert!(delta > 0.0 && delta <= kernel_boot::RATE_FORWARD + 1e-9, "第 {ticks} 发步长 {delta} 越界");
            assert_eq!(frame.value, boot.shown);
            assert_eq!(frame.step, 4);
            previous = boot.shown;
            ticks += 1;
        }
        // `show()` 把 shown 归零（KC:120），所以这一段是 0→96：按 1.5/发正好 64 发
        // ⇒ 条子是节奏器不是真进度（规格 §2.4）。
        assert_eq!(
            previous, 96.0,
            "1.5 是 2 的整除幂，累加到区间上沿必须是精确的 96，实际 {previous}"
        );
        assert!((63..=65).contains(&ticks), "爬满 96% 用了 {ticks} 发，预期约 64 发");
        // 吸住：|delta| < 0.05 时直接写成 target，之后不再动。
        boot.target = boot.shown + 0.02;
        boot.paint();
        assert_eq!(boot.shown, boot.target);
        let before = boot.shown;
        boot.paint();
        assert_eq!(boot.shown, before);
    }

    /// 第 1 步的插件子进度插值，以及 target 变小时的 `RATE_BACK`(3.0) 回退（KC:104-111）。
    #[test]
    fn boot_plugin_fraction_interpolates_and_backs_off_faster() {
        let mut boot = BootState::idle();
        boot.show();
        boot.report_plugins(4, 8);
        // 5 + (30-5) * 0.5 = 17.5
        assert_eq!(boot.target, 17.5);
        boot.report_plugins(1, 8); // 总数被重新统计 ⇒ 目标变小（主线唯一的回退场景）
        assert_eq!(boot.target, 5.0 + 25.0 * 0.125);
        boot.shown = 30.0;
        boot.paint();
        // 回退速率 3.0 = 前进的两倍：倒退刺眼，一两 tick 就消失（KC:33 注释）。
        assert_eq!(boot.shown, 27.0);
        // `total == 0` 视为本轮无包要装（KC:100）⇒ 回到「无中间量」，目标取上沿。
        boot.report_stage(2);
        boot.report_plugins(0, 0);
        assert_eq!(boot.plugins, None);
        assert_eq!(boot.target, 60.0);
        // 第 2 步不吃插件刻度：fraction 只在 step == 1 生效。
        boot.report_plugins(1, 4);
        assert_eq!(boot.target, 60.0);
    }

    /// 失败态：条子**停在失败瞬间的位置**、tick 停臂、Hint 让给原因整句、重试出现（KC:138-156）。
    #[test]
    fn boot_failure_freezes_the_bar_in_place() {
        let mut boot = BootState::idle();
        boot.show();
        boot.report_stage(2);
        boot.seconds = 7;
        for _ in 0..3 {
            boot.paint();
        }
        let frozen = boot.shown;
        assert!(frozen > 0.0);
        boot.fail("内核启动失败", "内核握手超时（90s 未见 dsh web: 行）");
        // tick 已停 ⇒ 再画也不动（主线 `OnKernelBootTick` 里 `sender.Stop()`，KC:209-213）。
        assert!(!boot.ticking());
        let frame = boot.render();
        assert_eq!(frame.value, frozen, "进度条不得归零：它告诉用户卡在第几步");
        assert_eq!(frame.stage_key, "内核启动失败");
        assert_eq!(frame.step, 2);
        assert!(frame.hint.is_none(), "失败态不再出秒数/插件刻度行");
        assert_eq!(
            frame.failure,
            Some((
                "内核启动失败".to_string(),
                "内核握手超时（90s 未见 dsh web: 行）".to_string()
            ))
        );
        // 语言切换只重刷文案、不动 Bar（KC:174-180）。
        assert_eq!(boot.render().value, frozen);
        // detail 为空的失败（主线 MW:2485 那条「未找到内置内核…」）也要能出整句。
        boot.fail("未找到内置内核；请安装 npm 版 dsh 或重新安装 Blade²", "");
        assert_eq!(
            boot.render().failure,
            Some((
                "未找到内置内核；请安装 npm 版 dsh 或重新安装 Blade²".to_string(),
                String::new()
            ))
        );
    }

    /// `(seq)` 守卫：重试后旧轮次那发、撤卡后那发、失败后那发统统丢掉（不忙等、不泄漏）。
    #[test]
    fn boot_tick_seq_guard_rejects_stale_and_retired() {
        let mut boot = BootState::idle();
        boot.show();
        assert_eq!(boot.tick, 0); // 还没上臂
        assert!(boot.accepts_tick(0), "首轮上臂那一发必须被认");
        boot.tick = 1;
        assert!(boot.accepts_tick(1));
        assert!(!boot.accepts_tick(2), "未来/重复的序号都不认");
        // 重试：`show()` 把 tick 归零、轮次代号 +1 ⇒ 旧轮次在飞那发（seq=1）必然不等。
        let seq_before = boot.seq;
        boot.show();
        assert_eq!(boot.tick, 0);
        assert!(!boot.accepts_tick(1), "重试后旧轮次那发必须被丢");
        assert_eq!(boot.seq, seq_before + 1, "轮次代号递增，供 BootDone 的迟到守卫用");
        // 失败即停臂。
        boot.tick = 1;
        boot.fail("内核启动失败", "x");
        assert!(!boot.accepts_tick(1));
        // 撤卡即停臂（`HideKernelBootPanel`，KC:125）。
        boot.failure = None;
        boot.hide();
        assert!(!boot.accepts_tick(0) && !boot.accepts_tick(1), "撤卡后还在投递就是 bug");
    }

    /// 96 → 100 的收尾：`shown` 也被直接写成 100（**不是补间**），撤卡要等 350ms 那一发（KC:114-120）。
    #[test]
    fn boot_completion_snaps_to_100_then_hides() {
        let mut boot = BootState::idle();
        boot.show();
        boot.report_stage(4);
        boot.paint(); // 爬到 ~1.5
        let mid = boot.shown;
        boot.complete();
        assert_eq!((boot.target, boot.shown, boot.plugins, boot.done), (100.0, 100.0, None, true));
        assert!(boot.ticking(), "收尾驻留期 tick 照常跑（主线没停，撤卡靠 BootDone）");
        let frame = boot.render();
        assert_eq!((frame.value, frame.percent), (100.0, 100));
        assert_eq!(frame.step, 4, "步骤行停在第 4 步");
        // done 不影响撤卡守卫：`hide()` 之后槽位缺席。
        boot.hide();
        assert!(!boot.showing && !boot.done);
        assert!(mid < 100.0);
    }

    /// `{2}%` 的取整口径：.NET `Math.Round` 默认重载是 ToEven，不是 Rust 的「远离零」（§7 存疑 6）。
    #[test]
    fn boot_percent_rounds_half_to_even() {
        for (value, want) in [
            (0.5, 0i64),
            (1.5, 2),
            (2.5, 2),
            (3.5, 4),
            (96.5, 96),
            (97.5, 98),
            (34.4, 34),
            (34.6, 35),
            (100.0, 100),
        ] {
            assert_eq!(round_half_even(value), want, "{value} 应取整到 {want}");
        }
        // 与 `f64::round` 的差异点（2.5 / 96.5）必须落在偶数那侧，否则步骤行会跟主线差 1%。
        assert_ne!(round_half_even(2.5), 2.5_f64.round() as i64);
    }

    /// 提示行三态（KC:234-248）：插件刻度优先，其次秒数，**首秒连文本一起收起**。
    #[test]
    fn boot_hint_prefers_plugins_then_seconds_and_hides_first_second() {
        let mut boot = BootState::idle();
        boot.show();
        boot.seconds = 0;
        assert_eq!(boot.paint().hint, None, "0 秒不显示，免得看着像坏了");
        boot.seconds = 3;
        assert_eq!(boot.paint().hint, Some(BootHint::Seconds(3)));
        boot.report_plugins(1, 5);
        assert_eq!(boot.paint().hint, Some(BootHint::Plugins(1, 5)));
        // 全部就绪 ⇒ 刻度行收起，回到秒数（`ready < total` 才显示）。
        boot.report_plugins(5, 5);
        assert_eq!(boot.paint().hint, Some(BootHint::Seconds(3)));
        // 第 2 步起不再报插件刻度（KC:234 的 `step == 1` 前置条件）。
        boot.report_stage(2);
        boot.report_plugins(1, 5);
        assert_eq!(boot.paint().hint, Some(BootHint::Seconds(3)));
    }

    /// §4 的一次性气泡开关：`hint_shown` 的复位点**只有**上屏（KC:56），
    /// 撤卡/失败/收尾都不复位 ⇒ 「每轮引导最多提示一次」，按重试才重新获得一次机会。
    #[test]
    fn wait_hint_flag_resets_only_on_show() {
        let mut boot = BootState::idle();
        boot.show();
        assert!(!boot.hint_shown);
        boot.hint_shown = true;
        boot.seconds = 5;
        boot.paint();
        boot.complete();
        assert!(boot.hint_shown, "内核就绪不复位");
        boot.hide();
        assert!(boot.hint_shown, "撤卡不复位");
        boot.fail("内核启动失败", "x");
        assert!(boot.hint_shown, "失败不复位");
        boot.show();
        assert!(!boot.hint_shown, "只有新一轮引导（首轮/重试）才复位");
    }

    /// 从源码里切出一段（两个锚点之间）。源码级断言专用：视图与引导线程都没法在单测里开窗口，
    /// 「这一发真的接上了 / 这个字面量真的不在了」这类事实只能这样锁。
    fn fn_slice<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
        let (_, rest) = source
            .split_once(start)
            .unwrap_or_else(|| panic!("源码里找不到锚点 {start}"));
        let body = rest.split_once(end).map_or(rest, |(body, _)| body);
        assert!(!body.trim().is_empty(), "{start} 到 {end} 之间是空的？");
        body
    }

    /// `BootEvent::Plugins` 的**生产者**接上了（缺口 #1）：
    /// ① 内核那发 `{ready,total}` 的解析口径（`total == 0` = 主线 KC:100 的「本轮无包要装」）；
    /// ② 抽样周期 = 主线 `WatchInstall` 的 250ms；
    /// ③ 模型侧第 1 段真的按 ready/total 插值（不是恒 1.0 爬满），且 `正在安装插件 x/y` 那行出得来；
    /// ④ 引导线程里这一发的位置：`Kernel::start` 之后、报 ② 之前 —— 已备案偏差，见 `start_kernel` doc。
    #[test]
    fn boot_plugin_ledger_producer_is_wired() {
        // ① 解析：缺键 / 类型不对 / `total <= 0` 都算「无刻度」，宁可不显示也不糊弄。
        assert_eq!(plugin_ledger(&json!({"ready": 2, "total": 6})), Some((2, 6)));
        assert_eq!(plugin_ledger(&json!({"ready": 6, "total": 6})), Some((6, 6)));
        assert_eq!(plugin_ledger(&json!({"ready": 0, "total": 0})), None);
        assert_eq!(plugin_ledger(&json!({"ready": 3, "total": -1})), None);
        assert_eq!(plugin_ledger(&json!({"ready": 1})), None);
        assert_eq!(plugin_ledger(&Value::Null), None);
        assert_eq!(plugin_ledger(&json!({"ready": "2", "total": 6})), None, "字符串不算数");
        // ② 周期与预算：250ms 与主线同数；预算够长，但绝不无限等（加载卡不能被钉死在第 1 段）。
        assert_eq!(kernel_boot::PLUGIN_POLL_MS, 250, "DshPluginBootstrap.cs:181 的那个 timer");
        assert!(kernel_boot::PLUGIN_BUDGET_MS >= 10_000);
        // ③ 第 1 段按真刻度爬：0/6 → 5（下沿钉住）、3/6 → 17.5、6/6 → 30（上沿）。
        let mut boot = BootState::idle();
        boot.show();
        boot.seconds = 1;
        boot.report_plugins(0, 6);
        assert_eq!(boot.target, 5.0);
        assert_eq!(boot.paint().hint, Some(BootHint::Plugins(0, 6)));
        boot.report_plugins(3, 6);
        assert_eq!(boot.target, 17.5);
        assert_eq!(boot.paint().hint, Some(BootHint::Plugins(3, 6)));
        boot.report_plugins(6, 6);
        assert_eq!(boot.target, 30.0);
        assert_eq!(
            boot.paint().hint,
            Some(BootHint::Seconds(1)),
            "ready == total ⇒ 刻度行收起，回到秒数（KC:234 的 ready < total 前置）"
        );
        // ④ 线程侧那一发的顺序（含已备案偏差：握手落在第 1 段窗口内）。
        let source = include_str!("main.rs");
        let chain = fn_slice(source, "fn start_kernel(", "fn arm_boot_tick");
        let at = |needle: &str| {
            chain
                .find(needle)
                .unwrap_or_else(|| panic!("引导链里找不到 {needle}"))
        };
        assert!(at("BootEvent::Stage(1)") < at("Launch::from_env()"));
        assert!(at("Launch::from_env()") < at("Kernel::start(&launch)"));
        assert!(
            at("Kernel::start(&launch)") < at("report_plugin_ledger(&tx, &mut kernel)"),
            "台账要在能问话之后问"
        );
        assert!(at("report_plugin_ledger(&tx, &mut kernel)") < at("BootEvent::Stage(2)"));
        assert!(at("BootEvent::Stage(2)") < at("BootEvent::Stage(3)"));
        assert!(at("BootEvent::Stage(3)") < at("list_sessions()"));
        // 生产者本体：真投递 + 问不出来就收手 + 预算上限。
        let producer = fn_slice(source, "fn report_plugin_ledger", "struct Bubble");
        assert!(producer.contains("BootEvent::Plugins(ready, total)"), "这一发得有生产者");
        assert!(producer.contains("kernel_boot::PLUGIN_RPC"));
        assert!(producer.contains("kernel_boot::PLUGIN_POLL_MS"));
        assert!(producer.contains("kernel_boot::PLUGIN_BUDGET_MS"));
        assert!(producer.contains("else {\n            return;"), "真内核 404 ⇒ 一次就收手");
        // 死码标记必须摘掉：留着它就等于宣称「这一发没人投」。
        let events = fn_slice(source, "enum BootEvent", "enum BootHint");
        assert!(!events.contains("allow(dead_code)"), "Plugins 分支已接上真上报");
    }

    /// 六颗 `automation_id` 逐字对齐 `MainWindow.xaml:703-708` 的 `x:Name`（缺口 #2）：
    /// 主干由 x:Name 生成 AutomationId，分叉只能显式写 —— 少一颗，UIA 侧就少一个可按 id 反查的点。
    #[test]
    fn boot_card_automation_ids_match_mainline_x_names() {
        let source = include_str!("main.rs");
        let card = fn_slice(source, "fn kernel_boot_panel", "fn settings_page");
        // Mark 那枚 id 走 `brand_mark` 的参数（Image 由它统一构造）。
        assert!(
            card.contains("brand_mark(self.brand(), size::EMPTY_STATE_MARK, \"KernelBootMark\")"),
            "KernelBootMark 得在卡片构造点上传进去"
        );
        for name in [
            "KernelBootPanel",
            "KernelBootStage",
            "KernelBootBar",
            "KernelBootStep",
            "KernelBootHint",
            "KernelBootRetry",
        ] {
            let needle = format!(".automation_id(\"{name}\")");
            assert!(card.contains(&needle), "UIA 反查缺这颗 id：{name}");
        }
        // 根 + 五颗子元素 = 卡片函数体里正好六处 `.automation_id(`（第七颗 Mark 在调用参数上）。
        assert_eq!(
            card.matches(".automation_id(").count(),
            6,
            "加载卡的 id 数量变了要连着核对 MX:703-708"
        );
    }

    /// 「文案多做」两处收掉（缺口 #3），逐字对齐主干：
    /// ① 内置内核拿不出来 = 主线 MW:2483-2486 的**单参** `FailKernelBoot(reasonKey)` ⇒ detail 为空，
    ///    Hint 只出「未找到内置内核；请安装 npm 版 dsh 或重新安装 Blade²」，不许多出「：原文」；
    ///    原文不丢，改落 `DIAG:` 行（主线那条走 `Debug.WriteLine`，分叉的对应通道）。
    /// ② 第一发 RPC 失败 = 主线 MW:2436 的 `FailKernelBoot("内核启动失败", ex.Message)`，
    ///    分叉自加的「（内核已启动）」后缀主干没有 ⇒ 删。
    #[test]
    fn boot_failure_texts_have_no_fork_extra_words() {
        let source = include_str!("main.rs");
        let code: String = source
            .lines()
            .map(|line| match line.find("//") {
                Some(cut) => &line[..cut],
                None => line,
            })
            .collect::<Vec<_>>()
            .join("\n");
        // 拼串：断言用的字面量不许原样出现在源码里（否则这条测试只是在 grep 自己）。
        let suffix = ["（内核已启", "动）"].concat();
        assert!(!code.contains(&suffix), "主干没有这个后缀，Hint 少一个字都不算 1:1");
        let chain = fn_slice(source, "fn start_kernel(", "fn arm_boot_tick");
        let at = chain
            .find("未找到内置内核；请安装 npm 版 dsh 或重新安装 Blade²")
            .expect("第 1 段那条失败定性要在");
        let arm = &chain[at..at + 200];
        assert!(
            arm.contains("String::new()"),
            "这一条主干只给 reasonKey，detail 必须为空：{arm}"
        );
        assert!(
            chain.contains("emit_line(&format!(\"DIAG: 内核组件解析失败: {error}\"))"),
            "detail 收掉了，原文得有 DIAG 行可查"
        );
        // RPC 失败那条：detail 逐字 = 异常原文，没有前后缀。
        assert!(
            chain.contains("Msg::Failed(\"内核启动失败\".to_string(), error)"),
            "detail 只能原样交给 ex.Message"
        );
        // 模型侧：detail 为空时 Hint 就是那一句（`kernel_boot_failed` 的全角冒号分支不进）。
        let mut boot = BootState::idle();
        boot.show();
        boot.fail("未找到内置内核；请安装 npm 版 dsh 或重新安装 Blade²", "");
        let frame = boot.render();
        assert_eq!(frame.failure.as_ref().unwrap().1, "");
        assert_eq!(frame.hint, None, "失败态那一行让给整句，由视图按 detail 是否为空拼");
    }

    /// 加载卡用到的 11 个键在分叉 i18n 表里必须**逐条存在**（EN 查得到译文）。
    /// 规格 §5 的第 12 个键 `Blade²` 只服务于主线 `ShellToast.Show`（KC:155），分叉没有 toast 原语
    /// ⇒ 那一发改成 stdout 的 `STATUS:` 行，不进本表断言。
    #[test]
    fn boot_card_keys_are_in_the_catalog() {
        let zh = Catalog::load("zh", None);
        let en = Catalog::load("en", None);
        let keys = [
            "正在准备内核组件…",
            "正在启动内核…",
            "正在连接内核…",
            "正在加载工作区与会话…",
            "重试",
            "内核启动失败",
            "第 {0} 步，共 {1} 步 · {2}%",
            "正在安装插件 {0}/{1}",
            "内核加载已进行 {0} 秒",
            "内核还在加载中，请稍候再发。",
            "未找到内置内核；请安装 npm 版 dsh 或重新安装 Blade²",
        ];
        for key in keys {
            assert_eq!(zh.l(key), key, "zh 必须原样回键名");
            assert_ne!(en.l(key), key, "缺 EN 译文：{key}");
        }
        // 占位符口径：主线 `LF` = string.Format(L(模板), args)。
        assert_eq!(
            zh.lf("第 {0} 步，共 {1} 步 · {2}%", &["2".into(), "4".into(), "34".into()]),
            "第 2 步，共 4 步 · 34%"
        );
        assert_eq!(zh.lf("内核加载已进行 {0} 秒", &["7".into()]), "内核加载已进行 7 秒");
        assert_eq!(zh.lf("正在安装插件 {0}/{1}", &["1".into(), "5".into()]), "正在安装插件 1/5");
    }

    /// 旧遮罩的残留必须清干净：`loading_layer` 与 `Shell::connecting` 都不许再有任何**代码**引用，
    /// 44px 手绘环也不许回到这张卡上（主线这张卡只有 `ProgressBar`）。
    #[test]
    fn loading_mask_is_gone() {
        let source = include_str!("main.rs");
        // 只看真代码：整行注释与行尾注释都截掉（doc 注释里提「替掉旧的 loading_layer」是有意义的
        // 历史说明，不算残留）。本文件没有被 `//` 截断后会漏检的字面量（URL 都在字符串里另起一行）。
        let code: String = source
            .lines()
            .map(|line| match line.find("//") {
                Some(cut) => &line[..cut],
                None => line,
            })
            .collect::<Vec<_>>()
            .join("\n");
        // 拼串：断言用的字面量不能原样出现在源码里，否则这条测试是在 grep 自己。
        let old_mask = ["loading", "_layer"].concat();
        let flag = ["conn", "ect"].concat();
        let old_title = ["正在启动 ds", "h 内核…"].concat();
        for banned in [&old_mask, &format!("self.{flag}"), &format!("{flag}:")] {
            assert!(!code.contains(banned.as_str()), "启动遮罩残留：代码里还能 grep 到 {banned}");
        }
        assert!(!code.contains(&old_title), "旧遮罩文案已换掉");
        assert!(source.contains("fn kernel_boot_panel"), "加载卡本体必须在");

        // 44px 手绘环的判据要**限定在卡片函数体内**：待发送气泡（PendingTpl）那枚 14px
        // ProgressRing 是主线有的控件，不能一起算成残留。
        let card = source
            .split_once("fn kernel_boot_panel")
            .map(|(_, rest)| rest.split_once("\n    fn ").map(|(body, _)| body).unwrap_or(rest))
            .expect("卡片函数必须存在");
        assert!(card.contains("ProgressBar::new()"), "进度条得用真控件");
        assert!(!card.contains("ProgressRing"), "卡片里不许出现手绘环/圈");
    }

    /// 控制面四型各一帧，形状照假内核 `control_opened` 推的那一串（会话 id 换成 `s-1`）。
    /// 分流的两条用例共用这一份 ⇒ 「每一型该落到哪张表」的期望只有一处定义。
    fn control_probe_frames() -> Vec<Value> {
        vec![
            json!({
                "type": "baseline",
                "value": {
                    "queues": {"s-1": [
                        {"id": "q-1", "placement": "queued",
                         "message": {"id": "m-1", "content": [{"type": "text", "text": "等这轮"}]}},
                    ]},
                    "jobs": {"s-1": [
                        {"id": "j-1", "kind": "bash-1", "label": "cargo check",
                         "status": "running", "startedAt": 100},
                    ]},
                    "projections": {"s-1": {"asOfSeq": 9, "values": {"title": "甲"}}},
                },
            }),
            json!({"type": "queue", "sessionId": "s-1", "items": [
                {"id": "q-2", "placement": "steering", "message": {"id": "m-2", "content": []}},
            ]}),
            json!({"type": "jobs", "sessionId": "s-1", "jobs": [
                {"id": "j-2", "kind": "subagent-2", "label": "查资料", "status": "completed",
                 "startedAt": 50, "finishedAt": 90},
            ]}),
            json!({"type": "projection", "sessionId": "s-1", "key": "plan", "seq": 12,
                   "value": {"active": true, "pending": true}}),
        ]
    }

    /// #60 / spec6 R1 的主判据：`session/control` 那四型在自己的 streamId 上必须全部走到
    /// `MuxSink::Control`，一帧都不许退回 `WorkspaceTree::apply` 那条兜底 —— 兜底读 `baseline`
    /// 的 `value.items`（缺 → 返回 `false` 且不报错），控制面的 `baseline` 读
    /// `value.queues/jobs/projections`，所以「只开流不分流」= 三张表永不落地且零错误。
    #[test]
    fn control_frames_route_to_control_state_and_never_to_the_tree() {
        // `open_mux` 的编号口径：rs1 = workspace/follow、rs2 = $events、rs3 = session/control；
        // 会话正文那一路从 rs4 起按会话补开（`ensure_follow`）。
        let follows = vec![("s-1".to_string(), "rs4".to_string())];
        let mut state = ControlState::default();
        let mut tree = WorkspaceTree::default();
        for frame in control_probe_frames() {
            assert_eq!(
                mux_frame_sink("rs3", Some("rs3"), &follows, &frame),
                MuxSink::Control,
                "控制帧被串到别的流上: {frame}"
            );
            assert!(
                !tree.apply(&frame),
                "R1 的静默丢帧没复现出来，兜底竟然认了这一帧: {frame}"
            );
            state.apply(&frame);
        }
        // 每一型各自落进自己那张表，且后到的整表替换 baseline 那一份（不是追加）。
        let queued = state.queue_items("s-1");
        assert_eq!((queued.len(), queued[0].item_id.as_str()), (1, "q-2"));
        let jobs = state.jobs_of("s-1");
        assert_eq!((jobs.len(), jobs[0].job_id.as_str()), (1, "j-2"));
        assert!(!jobs[0].is_live(), "jobs 帧整表替换：running 那条不再作数");
        let projections = state.projections("s-1").expect("baseline 的投影得在");
        assert_eq!(projections.title, "甲", "projection 增量只动它自己那一个键");
        assert_eq!(
            projections.plan.map(|plan| (plan.active, plan.pending)),
            Some((true, true)),
            "projection 帧落的必须是 plan 那一键"
        );
        assert_eq!(projections.as_of_seq, Some(9), "增量帧不许动游标");

        // 对照：分流臂没有动过其余三条流既有的去向。
        let emit = json!({"type": "emit", "event": "api-session/status", "args": ["s-1", true]});
        assert_eq!(
            mux_frame_sink("rs4", Some("rs3"), &follows, &emit),
            MuxSink::Follow("s-1".to_string())
        );
        assert_eq!(
            mux_frame_sink("rs2", Some("rs3"), &follows, &emit),
            MuxSink::Status("s-1".to_string(), true)
        );
        let workspace = json!({"type": "baseline", "value": {"items": [], "archivedSessionIds": []}});
        assert_eq!(
            mux_frame_sink("rs1", Some("rs3"), &follows, &workspace),
            MuxSink::Workspace
        );
        // 同名不同形的那一型：同一条 `baseline`，换条流就换张表 ⇒ 判据只能是 streamId。
        assert_eq!(
            mux_frame_sink("rs3", Some("rs3"), &follows, &workspace),
            MuxSink::Control
        );
        // 控制流还没开成（标记为 `None`）时，一条都不许被误认成控制帧。
        assert_eq!(
            mux_frame_sink("rs1", None, &follows, &workspace),
            MuxSink::Workspace
        );
    }

    /// 未识别的帧型不许静默消失（R1 的另一半）：`ControlTally` 按型计数、另开一桶「未识别」，
    /// 一批只出一行 DIAG。主干 `OnControlFrame` 的 `default: return`（MW:15336）在壳侧是
    /// 一个字都不写的，分叉这边至少得留下证据，否则「内核改名一个帧型」就等于「面板再也不刷」。
    #[test]
    fn control_tally_counts_types_and_speaks_once_per_batch() {
        let frames = control_probe_frames();
        let mut state = ControlState::default();
        for frame in &frames {
            state.apply(frame);
        }
        let mut tally = ControlTally::default();
        assert_eq!(
            tally.line(&state),
            None,
            "一批里一帧控制面元素都没见到 ⇒ 一行都不写"
        );
        for frame in &frames {
            assert_eq!(control_frame_type(frame), Some(frame["type"].as_str().unwrap()));
            tally.record(control_frame_type(frame));
        }
        for bogus in [
            // 内核还没发过的型别（`ready` 那一类心跳也算）。
            json!({"type": "heartbeat"}),
            // 判别键写错位置：`type` 整个缺席。
            json!({"kind": "queue", "sessionId": "s-1", "items": []}),
        ] {
            assert_eq!(control_frame_type(&bogus), None, "未识别型别必须报 None");
            let before = state.clone();
            assert!(!state.apply(&bogus).any());
            assert_eq!(state, before, "未识别的帧不许动状态");
            tally.record(control_frame_type(&bogus));
        }
        let line = tally.line(&state).expect("见过帧就得出声");
        for needle in [
            "控制面 6 帧:",
            "baseline 1",
            "queue 1",
            "jobs 1",
            "projection 1",
            "未识别 2",
            "队列 1 项 / 作业 1 项 / 投影 1 会话",
        ] {
            assert!(line.contains(needle), "少了 {needle}：{line}");
        }
    }

    /// R1 的「必须同批」约束：开流点与分流臂是一件事的两半，少一半就是静默丢帧。
    /// 单测里开不了窗口 ⇒ 照 `boot_plugin_ledger_producer_is_wired` 的口径在源码上锁顺序。
    #[test]
    fn control_stream_opens_and_demuxes_in_the_same_batch() {
        let source = include_str!("main.rs");
        let open = fn_slice(source, "fn open_mux(", "fn poll_mux(");
        assert!(
            open.contains("guard.open_session_control()"),
            "第三发没开在 open_mux 里 ⇒ 那个发起口又是零调用"
        );
        let at_open = |needle: &str| {
            open.find(needle)
                .unwrap_or_else(|| panic!("open_mux 里找不到 {needle}"))
        };
        assert!(
            at_open("\"workspace/follow\"") < at_open("\"$events\""),
            "主干 0.7.1 的时序：workspace 域必须最先触碰（MW:2614-2616）"
        );
        assert!(
            at_open("\"$events\"") < at_open("open_session_control()"),
            "控制面排在两者之后（MW:2622）"
        );

        let pump = fn_slice(source, "fn apply_mux_events(", "fn start_kernel(");
        let at_pump = |needle: &str| {
            pump.find(needle)
                .unwrap_or_else(|| panic!("事件泵里找不到 {needle}"))
        };
        assert!(
            at_pump("MuxSink::Control") < at_pump("self.tree.apply"),
            "分流臂必须排在兜底之前，否则 control 帧先被树吃掉"
        );
        assert!(
            at_pump("control.record(") < at_pump("self.control.apply("),
            "先认帧型再落表：未识别的那几帧不能悄悄算进已应用"
        );
        assert!(
            at_pump("control.line(&self.control)") > at_pump("MuxSink::Control"),
            "一批攒一行 DIAG（R1 要的「control 帧数 > 0」就靠这一行）"
        );
        // 丢实例 = 丢流：重开前两张流标记一起作废，否则新连接的 streamId 会顶着旧映射串台。
        assert!(pump.contains("self.control_stream = None"));
        assert!(pump.contains("self.follows.clear()"));

        let ready = fn_slice(source, "Msg::MuxReady(Ok((shared", "Msg::MuxReady(Err");
        assert!(
            ready.contains("self.control_stream = Some(control)"),
            "streamId 必须在 MuxReady 这一发里记下"
        );
        assert!(
            ready.find("self.mux = Some(shared)").unwrap()
                < ready.find("self.poll_mux(context)").unwrap(),
            "实例先落地，泵后上臂"
        );
        assert!(
            ready.find("self.control_stream = Some(control)").unwrap()
                < ready.find("self.poll_mux(context)").unwrap(),
            "流标记必须在第一次 poll 之前记下，不然首批控制帧无人认领"
        );
    }

    // ---------------- 斜杠命令浮层（`CommandPalette`，spec6 #52） ----------------

    fn command(name: &str, has_input: bool) -> CommandEntry {
        CommandEntry {
            name: name.to_string(),
            description: format!("{name} 的说明"),
            hint: if has_input { "<参数>".to_string() } else { String::new() },
            has_input,
        }
    }

    /// 主干 `UpdateCommandPalette` 的三个条件（MW:17686）：非空、首字符 `/`、**整串**不含
    /// 空格/制表/回车/换行。最后那条决定 Enter 归谁：进入参数段浮层就让位给普通发送。
    #[test]
    fn palette_trigger_requires_leading_slash_and_no_whitespace() {
        assert_eq!(palette_trigger(""), None);
        assert_eq!(palette_trigger("compact"), None);
        assert_eq!(palette_trigger("x /compact"), None);
        assert_eq!(palette_trigger("/"), Some(""));
        assert_eq!(palette_trigger("/com"), Some("com"));
        assert_eq!(palette_trigger("/COMPACT"), Some("COMPACT"));
        // 空白四型一律收（主干那句 `IndexOfAny([' ', '\t', '\r', '\n'])` 查的是整串）。
        assert_eq!(palette_trigger("/compact now"), None);
        assert_eq!(palette_trigger("/compact\tnow"), None);
        assert_eq!(palette_trigger("/a\r\nb"), None);
        assert_eq!(palette_trigger("/a "), None);
        // 采纳后写回的那串带尾随空格 ⇒ 浮层自己让位（与主干 `TextChanged` 同一判据）。
        assert_eq!(palette_trigger("/permission "), None);
        // 非 ASCII 不改判据（C# 的 `text[0]` 是首字符，这里按前缀判，结果一致）。
        assert_eq!(palette_trigger("/目标"), Some("目标"));
    }

    /// 主干 `TryLeadingCommandName`（MW:6901-6912）：composer 那一发**允许**参数段，
    /// 只取第一个空白前的名字；`"/"` 与 `"/ x"` 这种没有命令名的形态一律不算。
    #[test]
    fn leading_command_name_takes_the_first_token_only() {
        assert_eq!(leading_command_name("/compact"), Some("compact"));
        assert_eq!(leading_command_name("  /permission danger-full-access "), Some("permission"));
        assert_eq!(leading_command_name("/goal\tpause"), Some("goal"));
        assert_eq!(leading_command_name("/"), None);
        assert_eq!(leading_command_name("/ x"), None);
        assert_eq!(leading_command_name("compact"), None);
        assert_eq!(leading_command_name("看看 /compact"), None);
    }

    /// 主干那段过滤（MW:17716-17722）逐字口径：大小写无关 `Contains` → 前缀命中排前面
    /// （`OrderByDescending(StartsWith)`）→ 同档按名字 `OrdinalIgnoreCase` 升序 → `Take(20)`。
    #[test]
    fn palette_filter_orders_prefix_first_then_name_ordinal() {
        let entries = vec![
            command("zeta", false),
            command("Export", false),
            command("export-log", true),
            command("expose", false),
            command("alpha", false),
        ];
        let names: Vec<String> =
            filter_commands(&entries, "ex").into_iter().map(|entry| entry.name).collect();
        assert_eq!(
            names,
            vec!["Export", "export-log", "expose"],
            "前缀命中全部提前，组内 OrdinalIgnoreCase 升序（export < export-log < expose）"
        );
        // 大小写无关：换大写查询得到同一批、同一个序。
        let upper: Vec<String> =
            filter_commands(&entries, "EX").into_iter().map(|entry| entry.name).collect();
        assert_eq!(upper, names);
        // 中段命中（`Contains` 而不是 `StartsWith`）：`pose` 只命中 expose，没有前缀竞争者。
        assert_eq!(filter_commands(&entries, "pose").len(), 1);
        // 零命中：主干不放「无匹配」占位，直接收浮层。
        assert!(filter_commands(&entries, "zzz").is_empty());
        // 空前缀：`StartsWith("")` 对全体为真 ⇒ 前缀档无竞争者，序退化成按名升序。
        let all: Vec<String> =
            filter_commands(&entries, "").into_iter().map(|entry| entry.name).collect();
        assert_eq!(all, vec!["alpha", "Export", "export-log", "expose", "zeta"]);
        // Take(20)：25 条全命中也只留 20，且留的是排序后的前 20。
        let many: Vec<CommandEntry> =
            (0..25).map(|index| command(&format!("cmd{index:02}"), false)).collect();
        let hits = filter_commands(&many, "cmd");
        assert_eq!(hits.len(), 20, "Take(20)");
        assert_eq!(hits[0].name, "cmd00");
        assert_eq!(hits[19].name, "cmd19");
    }

    /// 3 秒负缓存（主干 17320-17323）：未知 agent 每键都问会把内核打穿，但**不能永久缓存**
    /// —— 旧会话的 agent 由内核惰性拉起，首问常报 `gateway/lookup-not-found`，到点要自愈。
    #[test]
    fn commands_negative_cache_expires_after_three_seconds() {
        let base = Instant::now();
        let at = |millis: u64| base + Duration::from_millis(millis);
        let mut palette = PaletteState::idle();
        assert!(!palette.needs_fetch("", base), "没有会话 = 主干那句早退，一个字都不问");
        assert!(palette.needs_fetch("s-1", base), "从没问过 ⇒ 问");

        palette.record("s-1", Err("gateway/lookup-not-found".to_string()), base);
        assert!(!palette.needs_fetch("s-1", at(2_999)), "3s 内不重复问");
        // 主干这条**不按会话判**（17320 那只比时间）：切了会话也照样等窗口。
        assert!(!palette.needs_fetch("s-2", at(1)), "负缓存跨会话生效（照抄主干）");
        assert!(palette.needs_fetch("s-1", at(3_000)), "到点自愈，再问一次");

        palette.record("s-1", Ok(vec![command("compact", false)]), at(3_000));
        assert!(!palette.needs_fetch("s-1", at(3_000) + Duration::from_secs(3600)), "正缓存不过期");
        assert!(palette.needs_fetch("s-2", at(3_000)), "正缓存按会话，切会话作废");

        // 失败连目录一起清（17353）；成功才留条目。
        palette.record("s-3", Err("boom".to_string()), at(4_000));
        assert!(palette.entries.is_empty(), "失败清 `_commands`");
        assert_eq!(palette.session, "s-3", "失败也记会话（17354）");
    }

    /// 整条链：钩子算出 `PaletteAccept` → `from_key_action` → 模型消息。
    /// **浮层开着时 Enter 绝不落到 `Msg::Send`** —— 主干 `OnInputPreviewKeyDown`（MW:6822-6841）
    /// 把 `HandlePaletteKey` 排在 `TryHandleEnterSend` 之前，顺序写反就是「选中即误发消息」。
    #[test]
    fn enter_never_sends_while_the_palette_is_open() {
        let open = keys::Scene { on_chat: true, sub_open: false, palette_open: true };
        let table = [
            (keys::VK_RETURN, KeyAction::PaletteAccept),
            (keys::VK_TAB, KeyAction::PaletteAccept),
            (keys::VK_UP, KeyAction::PaletteMove(-1)),
            (keys::VK_DOWN, KeyAction::PaletteMove(1)),
            (keys::VK_ESCAPE, KeyAction::PaletteClose),
        ];
        for (vk, want) in table {
            let action = keys::classify(vk, false, false, KeyOwner::Composer, open);
            assert_eq!(action, want, "vk={vk:#04x} 该走浮层");
            let message = Msg::from_key_action(action).expect("浮层那几档必须投消息，不许静默丢");
            assert!(
                !matches!(message, Msg::Send | Msg::SendAlternate),
                "浮层开着时这一发绝不能落成发送（vk={vk:#04x}）"
            );
        }
        // Ctrl+Enter 也被浮层吃掉（主干 `HandlePaletteKey` 的 Enter 分支只让 Shift）：
        // 采纳优先于「反向档发送」。
        assert_eq!(
            keys::classify(keys::VK_RETURN, false, true, KeyOwner::Composer, open),
            KeyAction::PaletteAccept
        );
        // Shift+Enter 仍是换行（放行给 TextBox，主干两处同口径）。
        assert_eq!(
            keys::classify(keys::VK_RETURN, true, false, KeyOwner::Composer, open),
            KeyAction::Pass
        );
        // 焦点不在 composer ⇒ 浮层那几档一概不开火（主干那些处理器只挂在 InputBox 上）。
        for (vk, _) in table {
            assert_eq!(
                keys::classify(vk, false, false, KeyOwner::Other, open),
                KeyAction::Pass,
                "vk={vk:#04x}"
            );
        }
        // 浮层关掉 ⇒ 同一发 Enter 回到发送那条老路。
        let closed = keys::Scene { palette_open: false, ..open };
        assert_eq!(
            keys::classify(keys::VK_RETURN, false, false, KeyOwner::Composer, closed),
            KeyAction::Submit
        );
        assert_eq!(
            keys::classify(keys::VK_TAB, false, false, KeyOwner::Composer, closed),
            KeyAction::Pass
        );
    }

    /// 采纳写回（主干 `AcceptCommandSelection`，MW:17802-17819）：
    /// · 带参数（`input` 是对象）⇒ 输入框整串换成 `/名字` + 一个空格，**不执行**（typed 前缀不保留）；
    /// · 无参 ⇒ 清空输入框并当场执行（这一发是自动发出的，不用再按 Enter）；
    /// · 两种都收浮层并把代次推走；越界（含「目录不可用」那态的 -1）只收浮层。
    #[test]
    fn palette_accept_fills_argumented_and_runs_plain() {
        let catalog = || vec![command("permission", true), command("compact", false)];
        let mut palette = PaletteState::idle();
        palette.matches = catalog();
        palette.index = 0;
        palette.showing = true;
        palette.generation = 7;
        assert_eq!(palette.take_accept(), PaletteAccept::Fill("/permission ".to_string()));
        assert!(!palette.showing && palette.matches.is_empty() && palette.index == -1);
        assert_eq!(palette.generation, 8, "采纳即 `HideCommandPalette`：在途回填就此作废");

        let mut palette = PaletteState::idle();
        palette.matches = catalog();
        palette.index = 1;
        assert_eq!(palette.take_accept(), PaletteAccept::Run("/compact".to_string()));

        let mut palette = PaletteState::idle();
        palette.matches = catalog();
        palette.index = 2;
        assert_eq!(palette.take_accept(), PaletteAccept::Nothing, "越界只收浮层（17806-17809）");
        let mut palette = PaletteState::idle();
        palette.showing = true;
        assert_eq!(palette.take_accept(), PaletteAccept::Nothing, "目录不可用那态 index = -1");
    }

    /// 浮层三态：代次过期 / 零命中 / 目录不可用（17699-17736）。
    #[test]
    fn palette_apply_covers_stale_zero_hit_and_error() {
        let mut palette = PaletteState::idle();
        palette.entries = vec![command("compact", false), command("plan", true)];
        palette.query = "com".to_string();
        palette.generation = 3;
        // 过期那一发：什么都不改。
        palette.apply(4, "/com");
        assert!(!palette.showing && palette.matches.is_empty());
        palette.apply(3, "/com");
        assert!(palette.showing && palette.list_visible && palette.index == 0);
        assert_eq!(palette.matches.len(), 1);
        let catalog = Catalog::load("zh", None);
        assert_eq!(
            palette.header(&catalog),
            "“/com” 匹配 1 条（↑↓ 选择，Enter 执行，Esc 关闭）"
        );
        // 零命中 ⇒ 收浮层、不出占位（hide 把代次推到 4）。
        palette.query = "zzz".to_string();
        palette.apply(3, "/zzz");
        assert!(!palette.showing && palette.matches.is_empty() && palette.generation == 4);
        // 输入里已经没有触发形态（参数段/别的文本）⇒ 回填不许把浮层掀回来。
        let mut stale = PaletteState::idle();
        stale.entries = vec![command("compact", false)];
        stale.generation = 1;
        stale.apply(1, "/compact now");
        assert!(!stale.showing);

        // 目录取不到：浮层照样上屏、列表区收起、原因摊在标题上（主干 17706-17714「不静默」）。
        let mut failed = PaletteState::idle();
        failed.generation = 1;
        failed.query = "c".to_string();
        failed.record("s-9", Err("gateway/lookup-not-found".to_string()), Instant::now());
        failed.apply(1, "/c");
        assert!(failed.showing && !failed.list_visible && failed.index == -1);
        assert_eq!(failed.header(&Catalog::load("zh", None)), "命令目录不可用：gateway/lookup-not-found");
        // 采纳这一态 = 越界 ⇒ Nothing（主干 `HandlePaletteKey` 仍吃 Enter，因为浮层可见）。
        assert_eq!(failed.take_accept(), PaletteAccept::Nothing);
    }

    /// `MoveCommandSelection`（MW:17791-17801）：夹到端点、不循环；没有候选时 `index` 原样不动。
    #[test]
    fn palette_move_clamps_and_ignores_empty() {
        let mut palette = PaletteState::idle();
        palette.move_selection(1);
        assert_eq!(palette.index, -1, "没有候选 = 主干那句 `if (_commandMatches.Count == 0) return`");
        palette.matches = (0..3).map(|index| command(&format!("c{index}"), false)).collect();
        palette.index = 0;
        palette.move_selection(-1);
        assert_eq!(palette.index, 0, "首行再往上不循环");
        for _ in 0..4 {
            palette.move_selection(1);
        }
        assert_eq!(palette.index, 2, "末行再往下不循环（Math.Clamp）");
    }

    /// 浮层真的挂在内容面那一格里：与 `stream`/`boot` 同一个 keyed 容器、后入者在上，
    /// 几何三个数值抄 MX:748-752 与 MX:774。这条是源码结构断言（分叉不能起 GUI 自测）。
    #[test]
    fn palette_overlay_is_mounted_in_the_chat_cell() {
        let source = include_str!("main.rs");
        let page = fn_slice(source, "fn chat_page(", "fn command_palette(");
        assert!(
            page.contains("if self.palette.showing"),
            "显隐走 keyed 槽位：reactor 没有 Visibility"
        );
        let at_stream = page.find("\"stream\",").expect("stream 槽位在");
        let at_boot = page.find("KeyedView::new(\"boot\"").expect("boot 槽位在");
        let at_palette = page.find("KeyedView::new(\"palette\"").expect("palette 槽位在");
        assert!(at_stream < at_boot && at_boot < at_palette, "同格叠放，后入者在上");
        assert!(page.contains(".rows([GridLength::STAR, GridLength::Auto])"));

        let shell = fn_slice(source, "fn command_palette(", "fn palette_row(");
        for fact in [
            ".grid_row(0)",
            ".vertical_alignment(VerticalAlignment::Bottom)",
            ".max_width(PALETTE_MAX_WIDTH)",
            ".max_height(PALETTE_MAX_HEIGHT)",
            ".margin(th(pad::OVERLAY))",
            ".corner_radius(radius::MEDIUM)",
            ".border_thickness(size::STROKE)",
            "automation_id(\"CommandPalette\")",
            "automation_id(\"CommandPaletteList\")",
            "automation_id(\"CommandPaletteHeader\")",
        ] {
            assert!(shell.contains(fact), "浮层几何少了 {fact}");
        }
        // 三个数值 = 主干 MX:748-752 / MX:774 的字面量。
        assert_eq!((PALETTE_MAX_WIDTH, PALETTE_MAX_HEIGHT, PALETTE_LIST_MAX_HEIGHT), (640.0, 264.0, 216.0));
        assert_eq!(pad::OVERLAY, [12.0, 0.0, 12.0, 8.0]);
        assert_eq!(radius::MEDIUM, 8.0);
        assert_eq!(PALETTE_ROW_PADDING, [8.0, 5.0, 8.0, 5.0]);
        assert_eq!(PALETTE_MATCH_LIMIT, 20);
        assert_eq!(COMMANDS_NEGATIVE_CACHE, Duration::from_secs(3));

        // 数据源仍是一条 RPC：`commands/list` 只经由 `Kernel::list_commands`。
        let fetch = fn_slice(source, "fn fetch_commands(", "fn apply_palette(");
        assert!(fetch.contains("kernel.list_commands(&session)"), "不许另开第二条 RPC 通路");
        assert!(fetch.contains("Msg::CommandsLoaded"));
        // 按键那条接缝真的接上了：`publish_keys` 报的是 `showing` 而不是写死的 false，
        // 并且只在浮层真的在屏上（停在聊天页）时才开火。
        let publish = fn_slice(source, "fn publish_keys(", "fn forget_commands(");
        assert!(publish.contains("keys::sync_pages(on_chat, on_chat && self.palette.showing)"));
    }
}
