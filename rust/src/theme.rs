//! 外壳调色板。
//!
//! `windows-reactor` 0.100 的 `ThemeBrush` 只有 8 档，主干那套语义画刷里能一一对上
//! 的直接沿用真实主题资源（跟着系统换肤/强调色自动走），对不上的按 WinUI 3
//! `generic.xaml`（WindowsAppSDK 1.6.250602001）逐项钉死 ARGB。
//!
//! 值来源（本机 NuGet 副本，与 `C:\Program Files\WindowsApps\Microsoft.WinUI_*` 同一份）：
//! `~/.nuget/packages/microsoft.windowsappsdk/1.6.250602001/lib/
//! net6.0-windows10.0.18362.0/Microsoft.WinUI/Themes/generic.xaml`
//! · Dark 词典 `x:Key="Default"` 起 L10，Light 词典 `x:Key="Light"` 起 L5561。
//! · 本轮（UI7）逐键复核的实测值，格式 `键：Light(行) / Dark(行)`：
//!   ControlFillColorDefault `#B3FFFFFF`(7527) / `#0FFFFFFF`(1976)
//!   ControlFillColorSecondary `#80F9F9F9`(7528) / `#15FFFFFF`(1977)
//!   ControlFillColorTertiary  `#4DF9F9F9`(7529) / `#08FFFFFF`(1978)
//!   ControlFillColorDisabled  `#4DF9F9F9`(7530) / `#0BFFFFFF`(1979)
//!   ControlStrokeColorDefault `#0F000000`(7550) / `#12FFFFFF`(1999)
//!   ControlStrokeColorSecondary `#29000000`(7551) / `#18FFFFFF`(2000)
//!   SubtleFillColorTransparent  `#00FFFFFF`(7536) / `#00FFFFFF`(1985)
//!   SubtleFillColorSecondary    `#09000000`(7537) / `#0FFFFFFF`(1986)
//!   SubtleFillColorTertiary     `#06000000`(7538) / `#0AFFFFFF`(1987)
//!   SubtleFillColorDisabled     `#00FFFFFF`(7539) / `#00FFFFFF`(1988)
//!   AccentFillColorDisabled   `#37000000`(7549) / `#28FFFFFF`(1998)
//!   ControlCornerRadius 恒为 `4,4,4,4`(2324/7878)，ControlContentThemeFontSize 恒 14(5590)
//! · 模板取档：`DefaultButtonStyle` L27347 —— 背景走 `ButtonBackground{,PointerOver,
//!   Pressed,Disabled}`（L355-358 那组 StaticResource = ControlFillColorDefault/Secondary/
//!   Tertiary/Disabled），描边走 `ButtonBorderBrush`= `ControlElevationBorderBrush`
//!   （L363-364，静止/悬停）与 `ControlStrokeColorDefaultBrush`（L365-366，按下/禁用），
//!   `ButtonBorderThemeThickness`=1（L125）。`ControlElevationBorderBrush` 是条 0→3px 的
//!   纵向渐变（L7681/2138：上端 ControlStrokeColorSecondary、下端 ControlStrokeColorDefault），
//!   reactor 没有 LinearGradientBrush ⇒ 静止/悬停档取它的**主色** ControlStrokeColorSecondary。
//! SDK 1.5/1.6/1.7 三套副本的这些键逐字节相同。
//!
//! ## 图标钮三态取哪条链：口径是**主干屏幕上真的渲出来的像素**
//! · **Control 链**（`control_*`）是默认 Button 模板的三档：`ButtonBackground{,PointerOver,
//!   Pressed}` = `ControlFillColorDefault/Secondary/Tertiary`。主干**没写**
//!   `Background`/`BorderThickness` 的裸 `Button`/`ToggleButton`（`MakeCopyButton`、轮尾分支钮、
//!   `MakeFeedbackToggle`、`MakeFeedbackButton`）整套都走这条，且带 1px 描边。
//! · 主干写了 `Background="Transparent"`（`IconButtonStyle` 全族 = 返回/搜索/工作区筛选/子代理
//!   回跳/灯箱关闭、会话行 `moreBtn`）或 `Background="{ThemeResource CardHoverBrush}"`
//!   （`ComposerAddButton`；`Tokens.xaml:164/249` 该令牌**就是** `SubtleFillColorSecondary`）
//!   **且** `BorderThickness="0"` 的那一族：**静置档**按主干写的值（分别是全透明 /
//!   Subtle Secondary 那层浅灰圆底），**悬停与按下两档却仍是 Control 链**。
//!   原因：模板 `CommonStates.PointerOver/Pressed` 的 Setter 抢的是 `ContentPresenter`
//!   自己的 `Background`，优先级高于实例上写的那支 `Background` 画刷
//!   （实例值只经 `TemplateBinding` 传进去，TemplateBinding 在模板这一层是最低档）
//!   ⇒ 主干那一族在屏幕上从来没渲出过 `subtle_hover`/`subtle_pressed`。
//!   上一轮按「设计意图」把这两档画成了 Subtle 链（浅色是**黑**叠加），方向与主干实渲相反
//!   （主干越悬停越白、分叉越悬停越灰），本轮按像素改回 Control 链，判据在
//!   `main.rs::Pill::ghost`/`Pill::card` 与 `icon_pills_render_the_mainline_control_chain`。
//! · `subtle_*` 四档仍然全部保留并按 generic.xaml 逐键钉死：除了当**静置底**
//!   （`subtle_rest` 给 ghost 一族、`subtle_hover` 给 `ComposerAddButton` 的圆底、
//!   `subtle_disabled` 给禁用档），也留给 `HyperlinkButton` 一族（generic.xaml:482-485 的
//!   `HyperlinkButtonBackground*` 才是真正走 Subtle 三档的模板）与 `overlay()` 那套叠档配方。
//! 方向记牢：**两条链都是 pressed 比 hover 浅一档**（Control 浅 #80→#4D、深 #15→#08；
//! Subtle 浅 #09→#06、深 #0F→#0A），别按「按下要比悬停更深」去改。
//!
//! 分链落在 `main.rs::Pill` 的四个构造器上（`ghost`/`card` = 透明或浅灰静置底 + Control 悬停/
//! 按下、`action` = Control 三档 + 1px 描边，`accent` = 强调色四态），一颗钮归哪档按主干 XAML
//! 原文定，实测逐态取证见 `tmp/ui8-shot.ps1` 与 `tmp/ui10-band.ps1`（几何+脸宽）：
//! 发送钮 rest `#C69E9E` → hover `#CCA8A8` → press `#D1B1B1`（强调色靠 0.9/0.8 不透明度
//! 逐档变浅，不是变深）。
//! 这一族还有一条硬要求：钮的悬停档**不能**和所在行/卡抢同一个 hover 槽，
//! 指针先进钮再冒泡进行，行那一档永远后到 ⇒ 钮的圆底一辈子画不出来
//! （会话行那颗 ⋯ 就是这么被行抢走的，`ui8-shot` 对它报 NO-TRACE）。

use windows_reactor::{Brush, Color, ThemeBrush};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scheme {
    Light,
    Dark,
}

const fn argb(a: u8, rgb: u32) -> Color {
    Color {
        a,
        r: (rgb >> 16) as u8,
        g: (rgb >> 8) as u8,
        b: rgb as u8,
    }
}

const SOLID: Brush = Brush::Theme(ThemeBrush::SolidBackground);
const CARD: Brush = Brush::Theme(ThemeBrush::CardBackground);
const STROKE: Brush = Brush::Theme(ThemeBrush::CardStroke);
const TEXT_PRIMARY: Brush = Brush::Theme(ThemeBrush::PrimaryText);
/// AccentFillColorDefaultBrush ← SystemAccentColorDark1(Light) / Light2(Dark)
const ACCENT: Brush = Brush::Theme(ThemeBrush::Accent);
/// InfoBrush 主干 = SystemFillColorAttentionBrush，同样是强调色派生
/// （Light SystemAccentColor / Dark SystemAccentColorLight2），没有固定常量，
/// 所以挂靠 Accent 主题资源而不是写死。
const INFO: Brush = ACCENT;
/// TextOnAccentFillColorPrimary：Light 白、Dark 黑（浅色用 Dark1 变体所以是反的）。
const fn on_accent(scheme: Scheme) -> Color {
    argb(
        255,
        match scheme {
            Scheme::Light => 0xFFFF_FF,
            Scheme::Dark => 0x00_0000,
        },
    )
}
const TRANSPARENT: Brush = Brush::Solid(Color {
    a: 0,
    r: 0,
    g: 0,
    b: 0,
});
/// `SubtleFillColorTransparent` 的字面值（浅 7536 / 深 1985 两词典都是 `#00FFFFFF`）：
/// alpha 为 0，RGB 保留令牌原色，别和 `TRANSPARENT`（`#00000000`）混成一个常量。
const SUBTLE_TRANSPARENT: Color = Color {
    a: 0,
    r: 255,
    g: 255,
    b: 255,
};

/// 两层同族叠加（source-over）：`over` 盖在 `under` 上，返回合成后的那一层实心色。
///
/// **前提：两档 RGB 相同**（Subtle 链内恒成立：浅词典四档全是 `#xxxxxx000000` 黑叠加、
/// 深词典全是 `#xxxxxxFFFFFF` 白叠加，只有 alpha 在动）。此时合成只需合 alpha：
/// `α = αo + αu·(1−αo)`，未预乘 RGB 原样继承 `over`。
/// 8bit 定点写法：浅色 Secondary 叠 Secondary = `#09`+`#09` → `#11`，Tertiary 叠 Secondary
/// → `#0E`；深色同理 `#0F`+`#0F` → `#1D`、`#0A` 叠 `#0F` → `#18`。
pub const fn overlay(over: Color, under: Color) -> Color {
    let ao = over.a as u32;
    let au = under.a as u32;
    Color {
        a: (ao + au * (255 - ao) / 255) as u8,
        r: over.r,
        g: over.g,
        b: over.b,
    }
}

/// 主干 `Theme/Tokens.xaml` 的语义画刷。
#[derive(Clone, Copy)]
pub struct Palette {
    pub scheme: Scheme,
    /// SurfaceBrush ← SolidBackgroundFillColorBase
    pub surface: Brush,
    /// SurfaceAltBrush ← LayerFillColorDefault（内容面底色）
    pub surface_alt: Brush,
    /// CardBrush ← CardBackgroundFillColorDefault
    pub card: Brush,
    /// CardSecondaryBrush ← CardBackgroundFillColorSecondary
    pub card_secondary: Brush,
    /// StrokeBrush ← CardStrokeColorDefault
    pub stroke: Brush,
    /// StrokeSubtleBrush ← DividerStrokeColorDefault
    pub stroke_subtle: Brush,
    pub text_primary: Brush,
    /// TextSecondaryBrush ← TextFillColorSecondary
    pub text_secondary: Brush,
    /// TextTertiaryBrush ← TextFillColorTertiary
    pub text_tertiary: Brush,
    /// TextDisabledBrush ← TextFillColorDisabled
    pub text_disabled: Brush,
    pub accent: Brush,
    /// OnAccentBrush ← TextOnAccentFillColorPrimary
    pub on_accent: Brush,
    /// 同上原色：`resource_overrides` 只吃 Color，强调色底板上的反白字形
    /// （`ButtonForeground*`）必须拿 Color，见 `main.rs::composer` 的 SendButton。
    pub on_accent_color: Color,
    /// ControlFillBrush ← ControlFillColorDefault（`Tokens.xaml:207/274`）。
    /// **默认 Button/ToggleButton 模板的静置档就是它**（generic.xaml:27348 `ButtonBackground`），
    /// 主干所有没写 `Background` 的小钮（`MakeCopyButton`、`MakeFeedbackToggle`、
    /// `MakeFeedbackButton`、分支/撤回编辑钮）静置态都是这一档，**不是透明**。
    /// 浅 #B3FFFFFF、深 #0FFFFFFF。
    pub control_fill: Brush,
    /// ControlHoverBrush ← ControlFillColorSecondary（`Tokens.xaml:208/275`）。
    /// 浅 `#80F9F9F9` / 深 `#15FFFFFF`：浅色档**本来就是白叠加**，只比静置的
    /// `#B3FFFFFF` 暗一档；所以它必须配 `control_fill` 的静置底才看得出悬停
    /// （静置透明 + 白叠加 = 在浅底上等于没变，这正是上一轮「筛选钮 hover 无变化」的成因）。
    pub control_hover: Brush,
    /// ControlPressedBrush ← ControlFillColorTertiary（`Tokens.xaml:209/276`，
    /// 浅 #4DF9F9F9、深 #08FFFFFF）。分叉的自绘底钮自己画状态层，拿得到三档才算凑齐
    /// 主干 `ButtonBackground/…PointerOver/…Pressed` 那一套。
    pub control_pressed: Brush,
    /// ControlFillColorDisabled：默认 Button 模板的 **Disabled** 档（generic.xaml 浅
    /// `#4DF9F9F9`、深 `#0BFFFFFF`）。分叉把模板四态底全钉透明了，禁用档也得自己画，
    /// 见 `main.rs::Pill::action`。
    pub control_disabled: Brush,
    /// ControlElevationBorderBrush 的主色（ControlStrokeColorSecondary，浅 `#29000000`、
    /// 深 `#18FFFFFF`）：默认 Button 模板**静止/悬停**那两档的 1px 描边
    /// （generic.xaml:363-364 + 7681 那条纵向渐变）。
    pub control_stroke: Brush,
    /// ControlStrokeColorDefault（浅 `#0F000000`、深 `#12FFFFFF`）：默认 Button 模板
    /// **按下/禁用**两档的描边（generic.xaml:365-366）。
    pub control_stroke_pressed: Brush,
    /// AccentFillColorDisabled：主干 AccentButtonStyle 的禁用档（generic.xaml:7549/1998，
    /// 浅 #37000000、深 #28FFFFFF）。强调色本体是主题画刷、运行时读不到分量，
    /// 但这一档 WinUI 自己也是写死的灰，可以照抄。
    pub accent_disabled: Brush,
    /// `resource_overrides` 只吃 Color，所以把 control_fill 再存一份原色
    pub control_fill_color: Color,
    /// CardHoverBrush ← SubtleFillColorSecondary（浅 `#09000000`、深 `#0FFFFFFF`；
    /// `Tokens.xaml:164/249`）。列表悬停/选中底，也是主干 `ComposerAddButton` 那颗 + 圆钮
    /// **静置**就写在身上的那层底（`MainWindow.xaml` 的
    /// `Background="{ThemeResource CardHoverBrush}"` + `BorderThickness="0"`）。
    /// Subtle 链的**悬停**档也是它，但主干那一族图标钮在屏幕上**取不到**这两档：
    /// 默认 Button 模板的 VisualState Setter 优先级高于实例 `Background`，悬停/按下被抢成
    /// `control_hover`/`control_pressed` ⇒ `Pill::ghost`/`Pill::card` 只有**静置**档用它。
    /// `subtle_hover`/`subtle_pressed` 本体仍按 generic.xaml 钉着，给 `HyperlinkButton`
    /// 那一族与 `overlay()` 叠档配方用。
    pub subtle_hover: Brush,
    /// Subtle 链**静置**档 = `SubtleFillColorTransparent`（浅/深都 `#00FFFFFF`，即全透明）。
    /// 主干 `Background="Transparent"` 的那一整族图标钮（`IconButtonStyle` 全族、会话行
    /// `moreBtn`）静置态就是它。
    pub subtle_rest: Brush,
    /// Subtle 链**按下**档 = `SubtleFillColorTertiary`（浅 `#06000000`、深 `#0AFFFFFF`）：
    /// 比悬停档浅一档，与 Control 链同方向（WinUI 两族都不是「按下更深」）。
    pub subtle_pressed: Brush,
    /// Subtle 链**禁用**档 = `SubtleFillColorDisabled`（浅/深都 `#00FFFFFF`）：
    /// generic.xaml:485 `HyperlinkButtonBackgroundDisabled` 就是这一档，Subtle 族禁用即隐。
    pub subtle_disabled: Brush,
    /// Subtle 链原色：`overlay()` 那种「状态层叠在静置层上」的配方要拿得到 alpha 才合成得动。
    /// 取用方是下面的 `subtle_disc_hover`/`subtle_disc_pressed`（`Pill::card` 本轮改回
    /// Control 链后这两个助手只由本文件的逐档断言守着，留着当备用配方）。
    pub subtle_hover_color: Color,
    /// 同上，按下档原色。
    pub subtle_pressed_color: Color,
    /// BubbleMicaBrush 的近似：reactor 没有 AcrylicBrush，用
    /// SolidBackgroundFillColorBase + 主干的 TintOpacity(0.6/0.7) 当不透明底板
    pub bubble: Brush,
    pub info: Brush,
    /// InfoBgBrush ← SystemFillColorAttentionBackground
    pub info_bg: Brush,
    /// SuccessBrush ← SystemFillColorSuccess
    pub success: Brush,
    /// WarningBrush ← SystemFillColorCaution
    pub warning: Brush,
    /// WarningBgBrush ← SystemFillColorCautionBackground
    pub warning_bg: Brush,
    /// ErrorBrush ← SystemFillColorCritical
    pub error: Brush,
    pub transparent: Brush,
}

impl Palette {
    pub const fn for_scheme(scheme: Scheme) -> Self {
        let light = matches!(scheme, Scheme::Light);
        Self {
            scheme,
            surface: SOLID,
            surface_alt: Brush::Solid(if light {
                argb(0x80, 0xFF_FF_FF)
            } else {
                argb(0x4C, 0x3A_3A_3A)
            }),
            card: CARD,
            card_secondary: Brush::Solid(if light {
                argb(0x80, 0xF6_F6_F6)
            } else {
                argb(0x08, 0xFF_FF_FF)
            }),
            stroke: STROKE,
            stroke_subtle: Brush::Solid(if light {
                argb(0x0F, 0x00_00_00)
            } else {
                argb(0x15, 0xFF_FF_FF)
            }),
            text_primary: TEXT_PRIMARY,
            text_secondary: Brush::Solid(if light {
                argb(0x9E, 0x00_00_00)
            } else {
                argb(0xC5, 0xFF_FF_FF)
            }),
            text_tertiary: Brush::Solid(if light {
                argb(0x72, 0x00_00_00)
            } else {
                argb(0x87, 0xFF_FF_FF)
            }),
            text_disabled: Brush::Solid(if light {
                argb(0x5C, 0x00_00_00)
            } else {
                argb(0x5D, 0xFF_FF_FF)
            }),
            accent: ACCENT,
            on_accent: Brush::Solid(on_accent(scheme)),
            on_accent_color: on_accent(scheme),
            control_fill: Brush::Solid(if light {
                argb(0xB3, 0xFF_FF_FF)
            } else {
                argb(0x0F, 0xFF_FF_FF)
            }),
            control_hover: Brush::Solid(if light {
                argb(0x80, 0xF9_F9_F9)
            } else {
                argb(0x15, 0xFF_FF_FF)
            }),
            control_pressed: Brush::Solid(if light {
                argb(0x4D, 0xF9_F9_F9)
            } else {
                argb(0x08, 0xFF_FF_FF)
            }),
            control_disabled: Brush::Solid(if light {
                argb(0x4D, 0xF9_F9_F9)
            } else {
                argb(0x0B, 0xFF_FF_FF)
            }),
            control_stroke: Brush::Solid(if light {
                argb(0x29, 0x00_00_00)
            } else {
                argb(0x18, 0xFF_FF_FF)
            }),
            control_stroke_pressed: Brush::Solid(if light {
                argb(0x0F, 0x00_00_00)
            } else {
                argb(0x12, 0xFF_FF_FF)
            }),
            accent_disabled: Brush::Solid(if light {
                argb(0x37, 0x00_00_00)
            } else {
                argb(0x28, 0xFF_FF_FF)
            }),
            control_fill_color: if light {
                argb(0xB3, 0xFF_FF_FF)
            } else {
                argb(0x0F, 0xFF_FF_FF)
            },
            subtle_hover_color: if light {
                argb(0x09, 0x00_00_00)
            } else {
                argb(0x0F, 0xFF_FF_FF)
            },
            subtle_pressed_color: if light {
                argb(0x06, 0x00_00_00)
            } else {
                argb(0x0A, 0xFF_FF_FF)
            },
            subtle_hover: Brush::Solid(if light {
                argb(0x09, 0x00_00_00)
            } else {
                argb(0x0F, 0xFF_FF_FF)
            }),
            subtle_rest: Brush::Solid(SUBTLE_TRANSPARENT),
            subtle_pressed: Brush::Solid(if light {
                argb(0x06, 0x00_00_00)
            } else {
                argb(0x0A, 0xFF_FF_FF)
            }),
            subtle_disabled: Brush::Solid(SUBTLE_TRANSPARENT),
            bubble: Brush::Solid(if light {
                argb(0x99, 0xF3_F3_F3)
            } else {
                argb(0xB3, 0x20_20_20)
            }),
            info: INFO,
            info_bg: Brush::Solid(if light {
                argb(0x80, 0xF6_F6_F6)
            } else {
                argb(0x08, 0xFF_FF_FF)
            }),
            success: Brush::Solid(if light {
                argb(0xFF, 0x0F_7B_0F)
            } else {
                argb(0xFF, 0x6C_CB_5F)
            }),
            warning: Brush::Solid(if light {
                argb(0xFF, 0x9D_5D_00)
            } else {
                argb(0xFF, 0xFC_E1_00)
            }),
            warning_bg: Brush::Solid(if light {
                argb(0xFF, 0xFF_F4_CE)
            } else {
                argb(0x1C, 0xFC_E1_00)
            }),
            error: Brush::Theme(ThemeBrush::SystemCritical),
            transparent: TRANSPARENT,
        }
    }

    /// Subtle 链里「静置身上**已经**有一层底」的那颗钮（主干只有 `ComposerAddButton`：
    /// `Background=CardHoverBrush` = Subtle Secondary）的**悬停**档 ——
    /// 状态层叠在静置层上（`overlay`），浅 `#11000000`、深 `#1DFFFFFF`。
    /// 不这么叠的话，悬停档与静置档同为 Secondary ⇒ 三态里少一态、逐态截图也没法作证。
    pub const fn subtle_disc_hover(&self) -> Brush {
        Brush::Solid(overlay(self.subtle_hover_color, self.subtle_hover_color))
    }

    /// 同一颗钮的**按下**档：Tertiary 叠在静置 Secondary 上，浅 `#0E000000`、深 `#18FFFFFF`
    /// ⇒ 比悬停浅一档、比静置深一档，方向与 generic.xaml 那张表一致。
    pub const fn subtle_disc_pressed(&self) -> Brush {
        Brush::Solid(overlay(self.subtle_pressed_color, self.subtle_hover_color))
    }
}

/// 主干图表色板（Tokens.xaml 写死的字面值，与主题资源无关）。
pub const CHART_LIGHT: [u32; 6] = [
    0xFF2563EB, 0xFF15803D, 0xFF7C3AED, 0xFFDC2626, 0xFFEA580C, 0xFF0891B2,
];
pub const CHART_DARK: [u32; 6] = [
    0xFF4C8DFF, 0xFF3FB950, 0xFFA371F7, 0xFFF85149, 0xFFF0883E, 0xFF39C5CF,
];
pub const CHART_HEAT_EMPTY: [u32; 2] = [0xFFE4E6EB, 0xFF2B2B2B];
pub const CHART_GRID: [u32; 2] = [0x22000000, 0x26FFFFFF];
/// markdown 代码块底色，主干按 IsDarkTheme 二选一（非令牌）。
pub const CODE_BLOCK_BG: [u32; 2] = [0xFFF0F0F4, 0xFF202024];

pub fn chart_palette(scheme: Scheme) -> Vec<Brush> {
    let raw: &[u32; 6] = match scheme {
        Scheme::Light => &CHART_LIGHT,
        Scheme::Dark => &CHART_DARK,
    };
    raw.iter()
        .map(|hex| {
            Brush::Solid(Color {
                a: (hex >> 24) as u8,
                r: (hex >> 16) as u8,
                g: (hex >> 8) as u8,
                b: *hex as u8,
            })
        })
        .collect()
}

pub fn code_block_brush(scheme: Scheme) -> Brush {
    let hex = CODE_BLOCK_BG[usize::from(matches!(scheme, Scheme::Dark))];
    Brush::Solid(Color {
        a: (hex >> 24) as u8,
        r: (hex >> 16) as u8,
        g: (hex >> 8) as u8,
        b: hex as u8,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argb_splits_alpha_and_rgb() {
        let c = argb(0x80, 0xF3F3F3);
        assert_eq!((c.a, c.r, c.g, c.b), (0x80, 0xF3, 0xF3, 0xF3));
    }

    #[test]
    fn on_accent_is_inverted_between_schemes() {
        assert_eq!(on_accent(Scheme::Light).a, 255);
        assert_eq!(on_accent(Scheme::Dark).r, 0);
    }

    #[test]
    fn layer_fill_is_transitive_not_opaque() {
        let light = Palette::for_scheme(Scheme::Light);
        // LayerFillColorDefault 是半透明白，不是 #F4F4F4
        assert_eq!(light.surface_alt, Brush::Solid(argb(0x80, 0xFFFFFF)));
    }

    #[test]
    fn control_tiers_match_generic_xaml() {
        let alpha = |brush: Brush| match brush {
            Brush::Solid(color) => color.a,
            Brush::Theme(_) => 255,
        };
        let light = Palette::for_scheme(Scheme::Light);
        // generic.xaml 浅色词典实测：7527/7528/7529/7530 + 7550/7551
        assert_eq!(light.control_fill, Brush::Solid(argb(0xB3, 0xFFFFFF)));
        assert_eq!(light.control_hover, Brush::Solid(argb(0x80, 0xF9F9F9)));
        assert_eq!(light.control_pressed, Brush::Solid(argb(0x4D, 0xF9F9F9)));
        assert_eq!(light.control_disabled, Brush::Solid(argb(0x4D, 0xF9F9F9)));
        assert_eq!(light.control_stroke, Brush::Solid(argb(0x29, 0x000000)));
        assert_eq!(
            light.control_stroke_pressed,
            Brush::Solid(argb(0x0F, 0x000000))
        );
        // 三档底必须「一样比一样暗」：白叠加的 alpha 递减，配 control_fill 的静置底才看得出
        assert!(alpha(light.control_fill) > alpha(light.control_hover));
        assert!(alpha(light.control_hover) > alpha(light.control_pressed));
        let dark = Palette::for_scheme(Scheme::Dark);
        // generic.xaml 深色(Default)词典实测：1976/1977/1978/1979 + 1999/2000
        assert_eq!(dark.control_fill, Brush::Solid(argb(0x0F, 0xFFFFFF)));
        assert_eq!(dark.control_hover, Brush::Solid(argb(0x15, 0xFFFFFF)));
        assert_eq!(dark.control_pressed, Brush::Solid(argb(0x08, 0xFFFFFF)));
        assert_eq!(dark.control_disabled, Brush::Solid(argb(0x0B, 0xFFFFFF)));
        assert_eq!(dark.control_stroke, Brush::Solid(argb(0x18, 0xFFFFFF)));
        assert_eq!(
            dark.control_stroke_pressed,
            Brush::Solid(argb(0x12, 0xFFFFFF))
        );
        // 深色是白叠加，静止→悬停变亮、悬停→按下变暗
        assert!(alpha(dark.control_fill) < alpha(dark.control_hover));
        assert!(alpha(dark.control_hover) > alpha(dark.control_pressed));
    }

    #[test]
    fn chart_palette_has_six_swatches() {
        assert_eq!(chart_palette(Scheme::Light).len(), 6);
        assert_eq!(chart_palette(Scheme::Dark).len(), 6);
    }

    /// Subtle 链四档逐键核对 generic.xaml（浅 7536-7539 / 深 1985-1988，
    /// 取档模板 = `HyperlinkButtonBackground*` L482-485），并核对方向：
    /// **悬停最深、按下比悬停浅一档**，静置/禁用都是全透明（`#00FFFFFF`，alpha 为 0）。
    #[test]
    fn subtle_tiers_match_generic_xaml() {
        let alpha = |brush: Brush| match brush {
            Brush::Solid(color) => color.a,
            Brush::Theme(_) => 255,
        };
        let rgb = |brush: Brush| match brush {
            Brush::Solid(color) => (color.r, color.g, color.b),
            Brush::Theme(_) => (0, 0, 0),
        };
        let light = Palette::for_scheme(Scheme::Light);
        assert_eq!(light.subtle_rest, Brush::Solid(argb(0x00, 0xFFFFFF)));
        assert_eq!(light.subtle_hover, Brush::Solid(argb(0x09, 0x000000)));
        assert_eq!(light.subtle_pressed, Brush::Solid(argb(0x06, 0x000000)));
        assert_eq!(light.subtle_disabled, Brush::Solid(argb(0x00, 0xFFFFFF)));
        // 浅色是**黑叠加**（RGB 全 0），且悬停最深、按下浅一档
        assert_eq!(rgb(light.subtle_hover), (0, 0, 0));
        assert!(alpha(light.subtle_rest) < alpha(light.subtle_hover));
        assert!(alpha(light.subtle_hover) > alpha(light.subtle_pressed));
        assert!(alpha(light.subtle_pressed) > alpha(light.subtle_disabled));
        let dark = Palette::for_scheme(Scheme::Dark);
        assert_eq!(dark.subtle_rest, Brush::Solid(argb(0x00, 0xFFFFFF)));
        assert_eq!(dark.subtle_hover, Brush::Solid(argb(0x0F, 0xFFFFFF)));
        assert_eq!(dark.subtle_pressed, Brush::Solid(argb(0x0A, 0xFFFFFF)));
        assert_eq!(dark.subtle_disabled, Brush::Solid(argb(0x00, 0xFFFFFF)));
        assert_eq!(rgb(dark.subtle_hover), (255, 255, 255));
        assert!(alpha(dark.subtle_hover) > alpha(dark.subtle_pressed));
        // 两条链的**令牌值**必须分家：`subtle_hover` 绝不能等于 `control_hover`。
        // 图标钮一族在屏幕上取的是 Control 档（见文件头），这里守的是「Subtle 档本身没被
        // 抄错」—— 两档撞值就说明有一档不是按 generic.xaml 逐键填的。
        assert_ne!(light.subtle_hover, light.control_hover);
        assert_ne!(light.subtle_pressed, light.control_pressed);
        assert_ne!(dark.subtle_hover, dark.control_hover);
    }

    /// Fluent **state-layer 叠档**的通用配方：身上已经带着一层底时，状态层是**叠**上去的，
    /// 不是替换。当前分叉里这条通路是备用配方（`Pill::card` 的悬停/按下本轮改回主干实渲的
    /// Control 链，见文件头），只由本文件的逐档断言守着；将来移植 `HyperlinkButton` 一族
    /// 或真需要「静置带底 + Subtle 叠加」的控件时直接取用。
    #[test]
    fn subtle_state_layers_stack_over_the_rest_disc() {
        assert_eq!(
            overlay(argb(0x09, 0x000000), argb(0x09, 0x000000)),
            argb(0x11, 0x000000)
        );
        assert_eq!(
            overlay(argb(0x06, 0x000000), argb(0x09, 0x000000)),
            argb(0x0E, 0x000000)
        );
        assert_eq!(
            overlay(argb(0x0F, 0xFFFFFF), argb(0x0F, 0xFFFFFF)),
            argb(0x1D, 0xFFFFFF)
        );
        assert_eq!(
            overlay(argb(0x0A, 0xFFFFFF), argb(0x0F, 0xFFFFFF)),
            argb(0x18, 0xFFFFFF)
        );
        // 同色透明层叠上去是恒等（alpha 为 0 时只继承 RGB ⇒ 两档必须同 RGB，见 `overlay`）
        assert_eq!(
            overlay(argb(0x00, 0x000000), argb(0x09, 0x000000)),
            argb(0x09, 0x000000)
        );
        for scheme in [Scheme::Light, Scheme::Dark] {
            let p = Palette::for_scheme(scheme);
            let (rest, hover, pressed) = (p.subtle_hover, p.subtle_disc_hover(), p.subtle_disc_pressed());
            let alpha = |brush: Brush| match brush {
                Brush::Solid(color) => color.a,
                Brush::Theme(_) => 255,
            };
            let want = match scheme {
                // 浅：静置 #09 → 悬停 #11（最深）→ 按下 #0E
                Scheme::Light => (0x09, 0x11, 0x0E),
                // 深：静置 #0F → 悬停 #1D（白叠加最多）→ 按下 #18
                Scheme::Dark => (0x0F, 0x1D, 0x18),
            };
            assert_eq!((alpha(rest), alpha(hover), alpha(pressed)), want);
            assert!(
                alpha(hover) > alpha(rest),
                "悬停必须比静置深，浅色下是黑叠加、圆不能「化掉」"
            );
            assert!(alpha(hover) > alpha(pressed), "按下要比悬停浅一档");
            assert_ne!(rest, hover);
            assert_ne!(hover, pressed);
            assert_ne!(rest, pressed);
        }
    }

    #[test]
    fn code_block_bg_follows_scheme() {
        assert_eq!(
            code_block_brush(Scheme::Light),
            Brush::Solid(argb(0xFF, 0xF0F0F4))
        );
        assert_eq!(
            code_block_brush(Scheme::Dark),
            Brush::Solid(argb(0xFF, 0x202024))
        );
    }
}
