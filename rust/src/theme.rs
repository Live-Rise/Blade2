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
    /// SolidBackgroundFillColorBase + 主干的 TintOpacity(0.6/0.7) 当不透明底板。
    /// **只是 Tokens.xaml 那支基线**：`main.rs::view` 每轮按「材质档 + 不透明度 + 窗口材质」
    /// 现算（[`bubble_brush`]）覆一次，设置·个性化的「消息气泡」卡因此是真生效而不是画的。
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

    /// 当前深浅主题下的气泡底：见 [`bubble_brush`]（材质档 / 窗口材质 / 不透明度三样事实）。
    pub const fn bubble_with(
        &self,
        material: BubbleMaterial,
        window: WindowMaterial,
        opacity: f64,
    ) -> Brush {
        bubble_brush(self.scheme, material, window, opacity)
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

/// 主干 `MainWindow.TraySettings.cs:30-32` 的 `shell.json` `bubbleMaterial` 三档
/// （序同设置·个性化「消息气泡」下拉的 `BubbleMaterialChoices`）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BubbleMaterial {
    /// 半透明：主题面色 + 不透明度直接当 alpha（主干唯一逐字抄得动的一档）。
    Translucent,
    /// 亚克力：主干是系统 `AcrylicBrush`。reactor 0.100 **没有元素级 Acrylic**
    /// （`WindowVisuals::backdrop` 只管窗口），所以这一档在分叉只能近似成实色板，
    /// 近似的口径见 [`bubble_brush`]。
    Acrylic,
    /// 跟随窗口材质：配方按 [`WindowMaterial`] 定，见 [`bubble_brush`]。
    Follow,
}

impl BubbleMaterial {
    /// 下拉框的当前下标 → 档位；越界按主干的兜底回落半透明
    /// （`SetBubbleMaterial` 里非三个 id 之一就是 `BubbleMaterialTranslucent`）。
    pub const fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Acrylic,
            2 => Self::Follow,
            _ => Self::Translucent,
        }
    }
}

/// 主干 `material` 字段的四档（序同 `tokens::SETTINGS_MATERIALS` 那四颗标签）。
/// 「跟随窗口材质」的气泡配方要看它，故单独一档一档列出来。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WindowMaterial {
    Mica,
    MicaAlt,
    Acrylic,
    /// 无（纯色）：主干给这条兜了个不透明底（`RootGrid.Background`），气泡也落纯色卡。
    None,
}

impl WindowMaterial {
    pub const fn from_index(index: usize) -> Self {
        match index {
            1 => Self::MicaAlt,
            2 => Self::Acrylic,
            3 => Self::None,
            _ => Self::Mica,
        }
    }
}

/// 主干 `BubbleOpacityMin/Max/Default`（`MainWindow.TraySettings.cs:35-37`）：
/// 合法域 0.2–1.0，默认 0.6。滑杆按 100 满刻度铺（`Minimum = Min*100` 那一层换算在调用侧）。
pub const BUBBLE_OPACITY_MIN: f64 = 0.2;
pub const BUBBLE_OPACITY_MAX: f64 = 1.0;
pub const BUBBLE_OPACITY_DEFAULT: f64 = 0.6;
/// 主干 `BuildBubbleBrush` 里亚克力玻璃的取样亮度档（`TintLuminosityOpacity`）：
/// 窗口材质 = acrylic 走 0.9（玻璃感、几乎不吃背后画面），mica / mica-alt 走 0.12（平面着色）。
const ACRYLIC_LUMINOSITY: f64 = 0.9;

const fn clamp_f64(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// 主干 `Scale`：同亮度下按系数收一档，四舍五入（.NET `Math.Round` 默认 ToEven，
/// 与这里差 ≤1， mica-alt 那档的冷移幅度本来就只是「可辨但不刺眼」）。
const fn scale_byte(value: u8, factor: f64) -> u8 {
    let rounded = (value as f64 * factor + 0.5) as i32;
    if rounded > 255 {
        255
    } else if rounded < 0 {
        0
    } else {
        rounded as u8
    }
}

/// 主干 `CoolShift`（mica-alt 的冷峻倾向）：红降 3%、绿降 1.5%、蓝不动。
const fn cool_shift(color: Color) -> Color {
    Color {
        a: color.a,
        r: scale_byte(color.r, 0.97),
        g: scale_byte(color.g, 0.985),
        b: color.b,
    }
}

/// `SurfaceBrush` 的原色（主干 `ThemeBrush("SurfaceBrush")` = `SolidBackgroundFillColorBase`）：
/// 气泡要按不透明度改 alpha 就得拿到分量，而 reactor 读不到主题画刷的分量 ⇒
/// 按 generic.xaml 的字面值钉住 RGB（与 `Tokens.xaml:184/253` 那支 `BubbleMicaBrush` 同色）。
pub const fn surface_color(scheme: Scheme) -> Color {
    match scheme {
        Scheme::Light => argb(0xFF, 0xF3_F3_F3),
        Scheme::Dark => argb(0xFF, 0x20_20_20),
    }
}

/// 滑杆那一头的百分数（20…100）换算回 0.2…1.0 并夹进合法域：主干是
/// `SetBubbleOpacity(slider.Value / 100)` + 里面的 `Math.Clamp(opacity, Min, Max)` 两条腿，
/// 分叉合成这一发（`main.rs::bubble_opacity`）。
pub const fn clamp_percent_to_opacity(percent: f64) -> f64 {
    clamp_f64(percent / 100.0, BUBBLE_OPACITY_MIN, BUBBLE_OPACITY_MAX)
}

/// 气泡底的不透明度：`0.2 … 1.0`，逐档口径同主干 `BuildBubbleBrush`。
pub const fn bubble_alpha(
    material: BubbleMaterial,
    window: WindowMaterial,
    opacity: f64,
) -> f64 {
    let value = clamp_f64(opacity, BUBBLE_OPACITY_MIN, BUBBLE_OPACITY_MAX);
    match material {
        // 半透明：滑杆就是 alpha，背后是视频/壁纸时画面直接透出来。
        BubbleMaterial::Translucent => value,
        // 亚克力：主干那层玻璃自带 0.9 的取样亮度（永远比半透明更实），
        // 分叉没有玻璃 ⇒ 近似成「不低于材质档自己的不透明度」的实色板。
        BubbleMaterial::Acrylic => {
            if value > ACRYLIC_LUMINOSITY {
                value
            } else {
                ACRYLIC_LUMINOSITY
            }
        }
        BubbleMaterial::Follow => match window {
            // 无材质：纯色卡，滑杆不参与（主干这条分支的 alpha 写死 1.0）。
            WindowMaterial::None => 1.0,
            WindowMaterial::Acrylic => {
                if value > ACRYLIC_LUMINOSITY {
                    value
                } else {
                    ACRYLIC_LUMINOSITY
                }
            }
            // mica / mica-alt 是平面着色层（取样亮度 0.12，本来就几乎不吃不透明度）⇒
            // 分叉按半透明那档透出 alpha，冷峻倾向只落在颜色上（见 `cool_shift`）。
            WindowMaterial::Mica | WindowMaterial::MicaAlt => value,
        },
    }
}

/// 气泡底：主干 `ApplyBubbleMaterial` 写进根网格 `BubbleMicaBrush` 的那支笔刷的等价物。
/// 三处气泡（用户 / 助手 / 交付物）都读同一支，所以换这一支就是全量生效。
pub const fn bubble_brush(
    scheme: Scheme,
    material: BubbleMaterial,
    window: WindowMaterial,
    opacity: f64,
) -> Brush {
    let tint = if matches!(material, BubbleMaterial::Follow) && matches!(window, WindowMaterial::MicaAlt)
    {
        cool_shift(surface_color(scheme))
    } else {
        surface_color(scheme)
    };
    Brush::Solid(Color {
        a: (bubble_alpha(material, window, opacity) * 255.0 + 0.5) as u8,
        r: tint.r,
        g: tint.g,
        b: tint.b,
    })
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

// ===================== 高对比度（HC）档：主题色全族 =====================
//
// 主干的 HC 档是**一份完整独立的第三档词典**：`Theme/Tokens.xaml:289-343`
// `<ResourceDictionary x:Key="HighContrast">`，36 发 `SolidColorBrush` + 6 发 `StaticResource`
// 别名，键集合与 Light(`:158-233`)/Dark(`:236-286`) 逐键对齐（`Tokens.xaml:14-15` 自己声明这条）。
//
// **这一档抄不到 ARGB 字面值，而且不该抄**：36 发键的取值全部写成
// `Color="{ThemeResource SystemColor<槽>Color}"`，而那八颗 `SystemColor*Color` 资源在
// `generic.xaml` 里**一处字面定义都没有**（逐槽 `grep -c 'x:Key="SystemColor…Color"'` = 0、
// `grep -cE '<Color x:Key="SystemColor'` = 0）；非 HC 词典里那八支同名**画刷**（`generic.xaml:2130-2137`）
// 全被框架写成品红 `#FF00FF` 占位，意思就是「非 HC 档别引这些键」。
// ⇒ 色值由 XAML 框架**运行时**从系统当前高对比度主题调色板注入。这里落的因此是
// 「逐键 → 系统槽」的**别名表**（`HcToken::slot`）+ 读那八个槽的 Win32 入口
// （`system_high_contrast`），不是一组自造常量。
//
// 词典怎么被选中：主干**从不**把 `RequestedTheme` 设成高对比度
// （`MainWindow.xaml.cs:18919` 只在 `ElementTheme.Dark`/`ElementTheme.Light` 之间切，
// `ElementTheme` 也没有 HighContrast 成员 —— 主干注释 `:15790` 明写这条），
// 全靠框架在系统 HC 生效时优先取 `x:Key="HighContrast"` 那一档。框架自己的
// `generic.xaml:2793` 那本 HC 词典用的就是同一个键名、同一族 `SystemColor*` 槽，是这条
// 机制的旁证。应用侧只消费一发布尔：`MainWindow.xaml.cs:15791-15801`
// `IsHighContrast()` = `new AccessibilitySettings().HighContrast`（读不到 catch ⇒ false），
// 三处消费点全在「关掉会吃掉对比的装饰」而不是配色：`:15653` 跳过趋势图面积渐变、
// `:15785-15788` 多序列改由虚线区分、`:18812-18826` 气泡清掉运行时笔刷回落词典纯色。
//
// 分叉为什么不能「挂靠主题资源自动走」：reactor 0.100.0 公开 surface 上 HC 相关读数/旋钮
// 全零（`element.rs` `grep -c 'Contrast'` = 0、`grep -c 'Accessibility'` = 0、
// `PropertyId` 257 发变体里 `grep -c 'Contrast' generated.rs` = 0、`ColorScheme`
// 只有 `Light`/`Dark`（`element.rs:1596-1600`）），`UIElement.HighContrastAdjustment` 只在
// **私有** `mod native`（`lib.rs:7`）的 vtable 里出现（`native/winui/bindings.rs:19597-19598`）
// ⇒ 够不着。所以 HC 档只能自己按 `HcToken::slot` 去读系统色表，见 `system_high_contrast`。

/// 系统高对比度主题的八个取色槽。名字与主干/框架词典里
/// `{ThemeResource SystemColor**<本槽>**Color}` 的那个中段一一对应（见 `resource_name`）。
///
/// 主干 HC 词典只用到前六颗；`ButtonFace`/`ButtonText` 是主干**没有**独立 HC 键的那几档
/// （`control_disabled`/`subtle_*`/`control_stroke*`）落到框架 HC 词典时需要的两颗，
/// 取证行号见 `hc_palette` 里的逐档注释。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HcSlot {
    /// `SystemColorWindowColor` ← `COLOR_WINDOW`(5)
    Window,
    /// `SystemColorWindowTextColor` ← `COLOR_WINDOWTEXT`(8)
    WindowText,
    /// `SystemColorGrayTextColor` ← `COLOR_GRAYTEXT`(17)
    GrayText,
    /// `SystemColorHighlightColor` ← `COLOR_HIGHLIGHT`(13)
    Highlight,
    /// `SystemColorHighlightTextColor` ← `COLOR_HIGHLIGHTTEXT`(14)
    HighlightText,
    /// `SystemColorHotlightColor` ← `COLOR_HOTLIGHT`(26)
    Hotlight,
    /// `SystemColorButtonFaceColor` ← `COLOR_BTNFACE`(15)
    ButtonFace,
    /// `SystemColorButtonTextColor` ← `COLOR_BTNTEXT`(18)
    ButtonText,
}

impl HcSlot {
    /// 表序即 `HighContrastColors` 的字段序，用例按它逐颗咬八槽的 `GetSysColor` 索引。
    pub const ALL: [Self; 8] = [
        Self::Window,
        Self::WindowText,
        Self::GrayText,
        Self::Highlight,
        Self::HighlightText,
        Self::Hotlight,
        Self::ButtonFace,
        Self::ButtonText,
    ];

    /// `{ThemeResource SystemColor…​Color}` 的中段名；主干词典与 `generic.xaml` 都拼这个。
    pub const fn resource_name(self) -> &'static str {
        match self {
            Self::Window => "Window",
            Self::WindowText => "WindowText",
            Self::GrayText => "GrayText",
            Self::Highlight => "Highlight",
            Self::HighlightText => "HighlightText",
            Self::Hotlight => "Hotlight",
            Self::ButtonFace => "ButtonFace",
            Self::ButtonText => "ButtonText",
        }
    }

    /// `GetSysColor(nIndex)` 的那发索引。数值取自本机
    /// `windows-sys-0.61.2/src/Windows/Win32/Graphics/Gdi/mod.rs:771-796`
    /// （`COLOR_BTNFACE`=15 `:771`、`COLOR_GRAYTEXT`=17 `:780`、`COLOR_HIGHLIGHT`=13 `:781`、
    /// `COLOR_HIGHLIGHTTEXT`=14 `:782`、`COLOR_HOTLIGHT`=26 `:783`、`COLOR_WINDOW`=5 `:794`、
    /// `COLOR_WINDOWTEXT`=8 `:796`、`COLOR_BTNTEXT`=18 `:775`）。
    /// ⚠ 经典索引不是从 1 连排的：`COLOR_WINDOW` 是 5 而不是直觉上的 1，
    /// `COLOR_HIGHLIGHTTEXT` 是 14 而不是 12 —— 别凭记忆改这两个数。
    pub const fn sys_color_index(self) -> i32 {
        match self {
            Self::Window => 5,
            Self::WindowText => 8,
            Self::GrayText => 17,
            Self::Highlight => 13,
            Self::HighlightText => 14,
            Self::Hotlight => 26,
            Self::ButtonFace => 15,
            Self::ButtonText => 18,
        }
    }
}

/// 系统 HC 主题的**当前实测**调色板：八颗槽各一个 ARGB。
///
/// 只有 `system_high_contrast()` 能填它（读系统），构造出来的值随用户换 HC 主题而变；
/// 本文件其余部分一律把它当输入参数，不假设任何一颗的具体分量
/// —— 主干那 36 发键同样不假设（它们只写槽名）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HighContrastColors {
    pub window: Color,
    pub window_text: Color,
    pub gray_text: Color,
    pub highlight: Color,
    pub highlight_text: Color,
    pub hotlight: Color,
    pub button_face: Color,
    pub button_text: Color,
}

impl HighContrastColors {
    /// 按槽取色。`HcToken::slot` 与 `HC_TOKEN_KEYS` 的全部下游都走这一发。
    pub const fn slot(&self, slot: HcSlot) -> Color {
        match slot {
            HcSlot::Window => self.window,
            HcSlot::WindowText => self.window_text,
            HcSlot::GrayText => self.gray_text,
            HcSlot::Highlight => self.highlight,
            HcSlot::HighlightText => self.highlight_text,
            HcSlot::Hotlight => self.hotlight,
            HcSlot::ButtonFace => self.button_face,
            HcSlot::ButtonText => self.button_text,
        }
    }
}

/// 主干 `Tokens.xaml` HC 词典的 36 发语义键。三份表（`slot()` / `name()` / `mainline_line()`）
/// 都按 `Self::ALL` 的同一序取档，用例 `hc_token_table_matches_the_mainline_dictionary`
/// 拿它们逐行回核主干词典本体，所以「改了一臂别名」「加一颗键但没登记行号」都会当场红。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HcToken {
    /// ← `SystemColorWindowColor`
    Surface,
    SurfaceAlt,
    Card,
    CardSecondary,
    /// 分叉 `subtle_hover` 的来源令牌（`Tokens.xaml` 的 `CardHoverBrush`，见 `Palette` 字段注释）
    CardHover,
    TextPrimary,
    TextSecondary,
    TextTertiary,
    TextDisabled,
    Stroke,
    StrokeSubtle,
    Accent,
    AccentHover,
    AccentPressed,
    OnAccent,
    BubbleMica,
    Success,
    SuccessBg,
    Warning,
    WarningBg,
    Error,
    ErrorBg,
    Info,
    InfoBg,
    FocusRing,
    ControlFill,
    ControlHover,
    ControlPressed,
    ChartSeries1,
    ChartSeries2,
    ChartSeries3,
    ChartSeries4,
    ChartSeries5,
    ChartSeries6,
    ChartHeatEmpty,
    ChartGrid,
}

impl HcToken {
    pub const ALL: [Self; 36] = [
        Self::Surface,
        Self::SurfaceAlt,
        Self::Card,
        Self::CardSecondary,
        Self::CardHover,
        Self::TextPrimary,
        Self::TextSecondary,
        Self::TextTertiary,
        Self::TextDisabled,
        Self::Stroke,
        Self::StrokeSubtle,
        Self::Accent,
        Self::AccentHover,
        Self::AccentPressed,
        Self::OnAccent,
        Self::BubbleMica,
        Self::Success,
        Self::SuccessBg,
        Self::Warning,
        Self::WarningBg,
        Self::Error,
        Self::ErrorBg,
        Self::Info,
        Self::InfoBg,
        Self::FocusRing,
        Self::ControlFill,
        Self::ControlHover,
        Self::ControlPressed,
        Self::ChartSeries1,
        Self::ChartSeries2,
        Self::ChartSeries3,
        Self::ChartSeries4,
        Self::ChartSeries5,
        Self::ChartSeries6,
        Self::ChartHeatEmpty,
        Self::ChartGrid,
    ];

    /// **本文件唯一的 HC 别名真相**：逐键 → 系统槽。
    /// 每臂行号 = `Theme/Tokens.xaml` HighContrast 词典里的取证行（`290-330`，词典本体 `289`）。
    /// 改任何一臂之前先去核那一行；`hc_token_table_matches_the_mainline_dictionary`
    /// 会把这张表整个对回主干词典，压平/串档当场红。
    pub const fn slot(self) -> HcSlot {
        match self {
            // 表面四档全落窗口底：HC 不做层级（Tokens.xaml:290/291/292/293）
            Self::Surface => HcSlot::Window, // :290
            Self::SurfaceAlt => HcSlot::Window, // :291
            Self::Card => HcSlot::Window, // :292
            Self::CardSecondary => HcSlot::Window, // :293
            Self::CardHover => HcSlot::Highlight, // :294
            // 文本两档同色：HC 里 secondary 不比 primary 淡（:295/:296），三档/禁用才落到 GrayText
            Self::TextPrimary => HcSlot::WindowText, // :295
            Self::TextSecondary => HcSlot::WindowText, // :296
            Self::TextTertiary => HcSlot::GrayText, // :297
            Self::TextDisabled => HcSlot::GrayText, // :298
            // 两条描边同色且**与正文同色**：HC 的描边就是要看得见（:299/:300）
            Self::Stroke => HcSlot::WindowText, // :299
            Self::StrokeSubtle => HcSlot::WindowText, // :300
            // 强调三态全落 Highlight：主干明写 HC 不用任何品牌色（:301/:302/:303）
            Self::Accent => HcSlot::Highlight, // :301
            Self::AccentHover => HcSlot::Highlight, // :302
            Self::AccentPressed => HcSlot::Highlight, // :303
            Self::OnAccent => HcSlot::HighlightText, // :304
            // 气泡回落纯色窗口底：半透明在 HC 里会吃掉对比（:306，配套 `ApplyBubbleMaterial`
            // 的 HC 分支 `MainWindow.xaml.cs:18812-18826`）
            Self::BubbleMica => HcSlot::Window, // :306
            // 状态色：成功/警告/信息全落窗口文本色，只有**错误**另走 Hotlight（:307-314）
            Self::Success => HcSlot::WindowText, // :307
            Self::SuccessBg => HcSlot::Window, // :308
            Self::Warning => HcSlot::WindowText, // :309
            Self::WarningBg => HcSlot::Window, // :310
            Self::Error => HcSlot::Hotlight, // :311
            Self::ErrorBg => HcSlot::Window, // :312
            Self::Info => HcSlot::WindowText, // :313
            Self::InfoBg => HcSlot::Window, // :314
            Self::FocusRing => HcSlot::WindowText, // :315
            // 控件填充：静置落窗口底、悬停与按下**同为** Highlight（:318/:319/:320）
            // ⇒ HC 档**故意**把 hover/pressed 压平（框架自己那三档也是压平成 ButtonFace，
            // `generic.xaml:4765/4766/4767`）。别按「三态得三颗色」去"修"它。
            Self::ControlFill => HcSlot::Window, // :318
            Self::ControlHover => HcSlot::Highlight, // :319
            Self::ControlPressed => HcSlot::Highlight, // :320
            // 数据色板六条序列**全压成同一颗**窗口文本色（:323-328）：多序列改由线型区分，
            // 见 `hc_series_dash` 与主干 `MainWindow.xaml.cs:15785-15788`
            Self::ChartSeries1 => HcSlot::WindowText, // :323
            Self::ChartSeries2 => HcSlot::WindowText, // :324
            Self::ChartSeries3 => HcSlot::WindowText, // :325
            Self::ChartSeries4 => HcSlot::WindowText, // :326
            Self::ChartSeries5 => HcSlot::WindowText, // :327
            Self::ChartSeries6 => HcSlot::WindowText, // :328
            Self::ChartHeatEmpty => HcSlot::Window, // :329
            Self::ChartGrid => HcSlot::WindowText, // :330
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::Surface => 0,
            Self::SurfaceAlt => 1,
            Self::Card => 2,
            Self::CardSecondary => 3,
            Self::CardHover => 4,
            Self::TextPrimary => 5,
            Self::TextSecondary => 6,
            Self::TextTertiary => 7,
            Self::TextDisabled => 8,
            Self::Stroke => 9,
            Self::StrokeSubtle => 10,
            Self::Accent => 11,
            Self::AccentHover => 12,
            Self::AccentPressed => 13,
            Self::OnAccent => 14,
            Self::BubbleMica => 15,
            Self::Success => 16,
            Self::SuccessBg => 17,
            Self::Warning => 18,
            Self::WarningBg => 19,
            Self::Error => 20,
            Self::ErrorBg => 21,
            Self::Info => 22,
            Self::InfoBg => 23,
            Self::FocusRing => 24,
            Self::ControlFill => 25,
            Self::ControlHover => 26,
            Self::ControlPressed => 27,
            Self::ChartSeries1 => 28,
            Self::ChartSeries2 => 29,
            Self::ChartSeries3 => 30,
            Self::ChartSeries4 => 31,
            Self::ChartSeries5 => 32,
            Self::ChartSeries6 => 33,
            Self::ChartHeatEmpty => 34,
            Self::ChartGrid => 35,
        }
    }

    /// 某槽在主干 HC 词典里对应的那句 `{ThemeResource SystemColor…Color}` 原文片段，
    /// 给用例拿去和 `Tokens.xaml` 的那一行比对。
    pub const fn mainline_expectation(self) -> (&'static str, &'static str) {
        (self.name(), self.slot().resource_name())
    }

    /// 主干资源键名（`Tokens.xaml` 里的 `x:Key`），与 `Self::ALL` 同序。
    pub const fn name(self) -> &'static str {
        match self {
            Self::Surface => "SurfaceBrush",
            Self::SurfaceAlt => "SurfaceAltBrush",
            Self::Card => "CardBrush",
            Self::CardSecondary => "CardSecondaryBrush",
            Self::CardHover => "CardHoverBrush",
            Self::TextPrimary => "TextPrimaryBrush",
            Self::TextSecondary => "TextSecondaryBrush",
            Self::TextTertiary => "TextTertiaryBrush",
            Self::TextDisabled => "TextDisabledBrush",
            Self::Stroke => "StrokeBrush",
            Self::StrokeSubtle => "StrokeSubtleBrush",
            Self::Accent => "AccentBrush",
            Self::AccentHover => "AccentHoverBrush",
            Self::AccentPressed => "AccentPressedBrush",
            Self::OnAccent => "OnAccentBrush",
            Self::BubbleMica => "BubbleMicaBrush",
            Self::Success => "SuccessBrush",
            Self::SuccessBg => "SuccessBgBrush",
            Self::Warning => "WarningBrush",
            Self::WarningBg => "WarningBgBrush",
            Self::Error => "ErrorBrush",
            Self::ErrorBg => "ErrorBgBrush",
            Self::Info => "InfoBrush",
            Self::InfoBg => "InfoBgBrush",
            Self::FocusRing => "FocusRingBrush",
            Self::ControlFill => "ControlFillBrush",
            Self::ControlHover => "ControlHoverBrush",
            Self::ControlPressed => "ControlPressedBrush",
            Self::ChartSeries1 => "ChartSeries1Brush",
            Self::ChartSeries2 => "ChartSeries2Brush",
            Self::ChartSeries3 => "ChartSeries3Brush",
            Self::ChartSeries4 => "ChartSeries4Brush",
            Self::ChartSeries5 => "ChartSeries5Brush",
            Self::ChartSeries6 => "ChartSeries6Brush",
            Self::ChartHeatEmpty => "ChartHeatEmptyBrush",
            Self::ChartGrid => "ChartGridBrush",
        }
    }

    /// 取证行号（`Theme/Tokens.xaml`）。
    pub const fn mainline_line(self) -> &'static str {
        match self {
            Self::Surface => "Tokens.xaml:290",
            Self::SurfaceAlt => "Tokens.xaml:291",
            Self::Card => "Tokens.xaml:292",
            Self::CardSecondary => "Tokens.xaml:293",
            Self::CardHover => "Tokens.xaml:294",
            Self::TextPrimary => "Tokens.xaml:295",
            Self::TextSecondary => "Tokens.xaml:296",
            Self::TextTertiary => "Tokens.xaml:297",
            Self::TextDisabled => "Tokens.xaml:298",
            Self::Stroke => "Tokens.xaml:299",
            Self::StrokeSubtle => "Tokens.xaml:300",
            Self::Accent => "Tokens.xaml:301",
            Self::AccentHover => "Tokens.xaml:302",
            Self::AccentPressed => "Tokens.xaml:303",
            Self::OnAccent => "Tokens.xaml:304",
            Self::BubbleMica => "Tokens.xaml:306",
            Self::Success => "Tokens.xaml:307",
            Self::SuccessBg => "Tokens.xaml:308",
            Self::Warning => "Tokens.xaml:309",
            Self::WarningBg => "Tokens.xaml:310",
            Self::Error => "Tokens.xaml:311",
            Self::ErrorBg => "Tokens.xaml:312",
            Self::Info => "Tokens.xaml:313",
            Self::InfoBg => "Tokens.xaml:314",
            Self::FocusRing => "Tokens.xaml:315",
            Self::ControlFill => "Tokens.xaml:318",
            Self::ControlHover => "Tokens.xaml:319",
            Self::ControlPressed => "Tokens.xaml:320",
            Self::ChartSeries1 => "Tokens.xaml:323",
            Self::ChartSeries2 => "Tokens.xaml:324",
            Self::ChartSeries3 => "Tokens.xaml:325",
            Self::ChartSeries4 => "Tokens.xaml:326",
            Self::ChartSeries5 => "Tokens.xaml:327",
            Self::ChartSeries6 => "Tokens.xaml:328",
            Self::ChartHeatEmpty => "Tokens.xaml:329",
            Self::ChartGrid => "Tokens.xaml:330",
        }
    }

    /// 逐键取色：`HcToken::slot` 的便利外壳，也是 `hc_palette` 唯一的取色通道。
    pub const fn color(self, colors: &HighContrastColors) -> Color {
        colors.slot(self.slot())
    }
}

/// HC 档的整套语义画刷：把主干 Light/Dark 那本 `Palette` 的**每一档**改写成
/// 「系统槽的当前实测值」，别名关系逐键照抄 `Tokens.xaml` 的 HighContrast 词典。
///
/// `scheme` 参数**不参与取色**（HC 词典里没有任何一档按深浅分叉），只填进
/// `Palette::scheme` 让下游那些「按深浅兜底」的分支不至于读到 `Default`；
/// 传调用方原本的 `self.scheme` 即可。气泡在 HC 档按主干口径落回纯色
/// （`Tokens.xaml:306`），所以 [`Palette::bubble_with`] 在这套笔刷上**不该再被调用**
/// —— 主干 `ApplyBubbleMaterial` 的 HC 分支（`MainWindow.xaml.cs:18812-18826`）就是
/// 清掉运行时笔刷、回落词典纯色，接线侧必须同样跳过那次覆写。
///
/// 主干**没有**独立 HC 键、只能落框架 HC 词典的那六档，注释里各挂 `generic.xaml` 的行号：
/// · `control_disabled` = `ControlFillColorDisabled` → ButtonFace（`generic.xaml:4768`）
/// · `subtle_rest` = `SubtleFillColorTransparent` → 字面 `Transparent`（`:4774`）
/// · `subtle_pressed` = `SubtleFillColorTertiary` → ButtonFace（`:4776`）
/// · `subtle_disabled` = `SubtleFillColorDisabled` → ButtonFace（`:4777`）
/// · `control_stroke` = `ControlStrokeColorSecondary` → ButtonText（`:4793`）
/// · `control_stroke_pressed` = `ControlStrokeColorDefault` → ButtonText（`:4792`）
/// · `accent_disabled` = `AccentFillColorDisabled` → Window（`:4791`）
///
/// ⚠ 与 Light/Dark 那条「Subtle 链静置/禁用是全透明」的观感**相反**：HC 档里
/// `subtle_pressed`/`subtle_disabled` 是**实心** ButtonFace。这不是笔误，
/// 框架就是这么定的那两档（上面两个行号）。
pub fn hc_palette(scheme: Scheme, colors: &HighContrastColors) -> Palette {
    let solid = |token: HcToken| Brush::Solid(token.color(colors));
    let c = |token: HcToken| token.color(colors);
    let on_accent = c(HcToken::OnAccent);
    Palette {
        scheme,
        surface: solid(HcToken::Surface),
        surface_alt: solid(HcToken::SurfaceAlt),
        card: solid(HcToken::Card),
        card_secondary: solid(HcToken::CardSecondary),
        stroke: solid(HcToken::Stroke),
        stroke_subtle: solid(HcToken::StrokeSubtle),
        text_primary: solid(HcToken::TextPrimary),
        text_secondary: solid(HcToken::TextSecondary),
        text_tertiary: solid(HcToken::TextTertiary),
        text_disabled: solid(HcToken::TextDisabled),
        accent: solid(HcToken::Accent),
        on_accent: Brush::Solid(on_accent),
        on_accent_color: on_accent,
        control_fill: solid(HcToken::ControlFill),
        control_hover: solid(HcToken::ControlHover),
        control_pressed: solid(HcToken::ControlPressed),
        // 主干无 HC 键：框架 HC 词典 ControlFillColorDisabled = ButtonFace（generic.xaml:4768）
        control_disabled: Brush::Solid(colors.slot(HcSlot::ButtonFace)),
        control_stroke: Brush::Solid(colors.slot(HcSlot::ButtonText)), // generic.xaml:4793
        control_stroke_pressed: Brush::Solid(colors.slot(HcSlot::ButtonText)), // generic.xaml:4792
        accent_disabled: Brush::Solid(colors.slot(HcSlot::Window)), // generic.xaml:4791
        control_fill_color: c(HcToken::ControlFill),
        subtle_hover_color: c(HcToken::CardHover),
        subtle_pressed_color: colors.slot(HcSlot::ButtonFace), // generic.xaml:4776
        // 主干 CardHoverBrush = Highlight（Tokens.xaml:294），压过框架那档 ButtonFace（:4775）
        subtle_hover: solid(HcToken::CardHover),
        // SubtleFillColorTransparent 在框架 HC 词典里就是字面 Transparent（:4774）
        subtle_rest: TRANSPARENT,
        subtle_pressed: Brush::Solid(colors.slot(HcSlot::ButtonFace)), // generic.xaml:4776
        subtle_disabled: Brush::Solid(colors.slot(HcSlot::ButtonFace)), // generic.xaml:4777
        bubble: solid(HcToken::BubbleMica),
        info: solid(HcToken::Info),
        info_bg: solid(HcToken::InfoBg),
        success: solid(HcToken::Success),
        warning: solid(HcToken::Warning),
        warning_bg: solid(HcToken::WarningBg),
        error: solid(HcToken::Error),
        transparent: TRANSPARENT,
    }
}

/// HC 档的图表色板：主干把六条序列**全压成同一颗**窗口文本色（`Tokens.xaml:323-328`），
/// 所以这里六颗**必须**一样，靠 [`hc_series_dash`] 的线型区分序列。
/// 别「顺手」给它们配六色 —— 那是主干明确不要的（词典上方注释 `:321-322`）。
pub fn hc_chart_palette(colors: &HighContrastColors) -> Vec<Brush> {
    [
        HcToken::ChartSeries1,
        HcToken::ChartSeries2,
        HcToken::ChartSeries3,
        HcToken::ChartSeries4,
        HcToken::ChartSeries5,
        HcToken::ChartSeries6,
    ]
    .iter()
    .map(|token| Brush::Solid(token.color(colors)))
    .collect()
}

/// 多序列的线型档：主干 `MainWindow.xaml.cs:15785-15788` 的 `SeriesDash`
/// （`IsHighContrast() && index % 3 > 0 ? new DoubleCollection { 3 * (index % 3), 2 } : null`）。
/// HC 下序列色全同 ⇒ 序列 1（index 0）实线，index%3>0 的按 `{3*(i%3), 2}` 加虚线。
/// 只在 HC 档调用：非 HC 档主干一律给 `null`，不加虚线。
pub const fn hc_series_dash(index: usize) -> Option<[f64; 2]> {
    let phase = index % 3;
    if phase > 0 {
        Some([3.0 * phase as f64, 2.0])
    } else {
        None
    }
}

/// 代码块底色：主干这一档**不是令牌**（`theme.rs:576` 那颗 `CODE_BLOCK_BG` 按
/// `IsDarkTheme` 二选一），HC 词典里没有对应物 ⇒ 没有可抄的行号。
/// 这里落到 `SystemColorWindowColor`，与 `SurfaceBrush` 同槽（`Tokens.xaml:290`），
/// 是为了「底板不比窗口更亮、字形仍是窗口文本色」这条 HC 底线。
/// ⚠ 这一发是**推断**、不是主干取证，见报告 §6。
pub fn hc_code_block_bg(colors: &HighContrastColors) -> Brush {
    Brush::Solid(colors.slot(HcSlot::Window))
}

// ---- 运行时读数：八颗槽从哪来 ----

#[repr(C)]
#[allow(dead_code, reason = "`cbSize`/`lpszDefaultScheme` 只喂给 user32 与由它写回，Rust 侧只读 `dwFlags`")]
struct HighContrastW {
    cb_size: u32,
    dw_flags: u32,
    lpsz_default_scheme: *mut u16,
}

/// `SPI_GETHIGHCONTRAST`（本机 `windows-sys-0.61.2/.../UI/WindowsAndMessaging/mod.rs:3082` = 66）
const SPI_GETHIGHCONTRAST: u32 = 66;
/// `HCF_HIGHCONTRASTON`（本机 `windows-sys-0.61.2/.../UI/Accessibility/mod.rs:444` = 1）
const HCF_HIGHCONTRASTON: u32 = 1;

// 零依赖口径同 `keys.rs:35-42`：裸 `extern "system"` + `#[link]` 自声明，不新增 Cargo 依赖。
// `GetSysColor` 在 **user32**（`windows-sys-0.61.2/.../Graphics/Gdi/mod.rs:213`），不在 gdi32。
#[link(name = "user32")]
unsafe extern "system" {
    fn GetSysColor(n_index: i32) -> u32;
    fn SystemParametersInfoW(
        ui_action: u32,
        ui_param: u32,
        pv_param: *mut core::ffi::c_void,
        ui_win_ini: u32,
    ) -> i32;
}

/// 某一槽的当前系统色。`GetSysColor` 返回 `0x00BBGGRR`（没有 alpha 那一字节），
/// HC 档全程实心 ⇒ alpha 一律 255，与主干 HC 词典「不做任何半透明」的约定同向
/// （`Tokens.xaml:316-317`、`MainWindow.xaml.cs:15652` 那条注释）。
pub fn sys_color(slot: HcSlot) -> Color {
    let bgr = unsafe { GetSysColor(slot.sys_color_index()) };
    Color {
        a: 255,
        r: (bgr & 0xFF) as u8,
        g: ((bgr >> 8) & 0xFF) as u8,
        b: (bgr >> 16) as u8,
    }
}

/// 系统高对比度开关现在是不是**开着**。读不到就按「没开」处理 ——
/// 与主干 `MainWindow.xaml.cs:15797-15799` 那个 `catch (Exception) => false` 同方向。
pub fn high_contrast_active() -> bool {
    let mut info = HighContrastW {
        cb_size: core::mem::size_of::<HighContrastW>() as u32,
        dw_flags: 0,
        lpsz_default_scheme: core::ptr::null_mut(),
    };
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            info.cb_size,
            &mut info as *mut HighContrastW as *mut core::ffi::c_void,
            0,
        )
    };
    ok != 0 && (info.dw_flags & HCF_HIGHCONTRASTON) != 0
}

/// 整套系统 HC 调色板的当前实测值；**开关没开就返回 `None`**（此时分叉该继续用
/// `Palette::for_scheme`，与主干「HC 词典只在系统 HC 生效时被选中」同构）。
pub fn system_high_contrast() -> Option<HighContrastColors> {
    if !high_contrast_active() {
        return None;
    }
    Some(HighContrastColors {
        window: sys_color(HcSlot::Window),
        window_text: sys_color(HcSlot::WindowText),
        gray_text: sys_color(HcSlot::GrayText),
        highlight: sys_color(HcSlot::Highlight),
        highlight_text: sys_color(HcSlot::HighlightText),
        hotlight: sys_color(HcSlot::Hotlight),
        button_face: sys_color(HcSlot::ButtonFace),
        button_text: sys_color(HcSlot::ButtonText),
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

    /// 下拉的三档与越界兜底：序 = 主干 `BubbleMaterialChoices`（半透明/亚克力/跟随窗口材质），
    /// 窗口材质那四颗的序 = `tokens::SETTINGS_MATERIALS`。越界一律回落默认档（主干
    /// `SetBubbleMaterial` 对未知 id 的处理就是这个口径）。
    #[test]
    fn bubble_material_indexes_line_up_with_the_dropdown() {
        assert_eq!(
            [
                BubbleMaterial::from_index(0),
                BubbleMaterial::from_index(1),
                BubbleMaterial::from_index(2),
                BubbleMaterial::from_index(9),
            ],
            [
                BubbleMaterial::Translucent,
                BubbleMaterial::Acrylic,
                BubbleMaterial::Follow,
                BubbleMaterial::Translucent
            ]
        );
        assert_eq!(
            [
                WindowMaterial::from_index(0),
                WindowMaterial::from_index(1),
                WindowMaterial::from_index(2),
                WindowMaterial::from_index(3),
                WindowMaterial::from_index(9),
            ],
            [
                WindowMaterial::Mica,
                WindowMaterial::MicaAlt,
                WindowMaterial::Acrylic,
                WindowMaterial::None,
                WindowMaterial::Mica
            ]
        );
    }

    /// 主干 `BuildBubbleBrush` 的三条口径逐条核对：
    /// ① 半透明档默认 0.6 ⇒ alpha `#99`，与 `Tokens.xaml:184` 那支基线**同值**；
    /// ② 不透明度只吃 0.2–1.0，越界夹住（滑杆 `Minimum/Maximum` 就是这两端的 100 倍）；
    /// ③ 亚克力档吃材质自身的取样亮度 0.9 当下限，切档必然比半透明更实（观感得看得出）。
    #[test]
    fn bubble_tiers_follow_the_mainline_recipe() {
        let alpha = |brush: Brush| match brush {
            Brush::Solid(color) => color.a,
            Brush::Theme(_) => 255,
        };
        // ① 默认档：半透明 0.6，浅色 = Tokens.xaml 基线 #99F3F3F3
        assert_eq!(
            bubble_brush(Scheme::Light, BubbleMaterial::Translucent, WindowMaterial::Mica, 0.6),
            Brush::Solid(argb(0x99, 0xF3F3F3))
        );
        // ② 两端 + 越界夹取：0.2 ⇒ #33、1.0 ⇒ #FF，滑杆外溢的 0.05 / 1.4 落回端点
        let solid = |opacity: f64| alpha(bubble_brush(Scheme::Light, BubbleMaterial::Translucent, WindowMaterial::Mica, opacity));
        assert_eq!(solid(BUBBLE_OPACITY_MIN), 0x33);
        assert_eq!(solid(BUBBLE_OPACITY_MAX), 0xFF);
        assert_eq!(solid(0.05), 0x33);
        assert_eq!(solid(1.4), 0xFF);
        // ③ 同不透明度下三档必须分得开：半透明 0.6 < 亚克力 0.9；跟随 + 无材质 = 不透明
        let translucent = solid(0.6);
        let acrylic = alpha(bubble_brush(Scheme::Light, BubbleMaterial::Acrylic, WindowMaterial::Mica, 0.6));
        let follow_none = alpha(bubble_brush(Scheme::Dark, BubbleMaterial::Follow, WindowMaterial::None, 0.2));
        assert_eq!((translucent, acrylic, follow_none), (0x99, 0xE6, 0xFF));
        assert!(acrylic > translucent, "切到亚克力档得真的更实");
        // 亚克力的取样亮度只是**下限**：滑杆推到 1.0 时两档收敛（主干同样收敛）
        assert_eq!(
            alpha(bubble_brush(Scheme::Light, BubbleMaterial::Acrylic, WindowMaterial::Mica, 1.0)),
            0xFF
        );
        // 跟随 = mica：与半透明同 alpha（主干那条分支的取样亮度只吃 0.12，本来就几乎不吃滑杆）
        assert_eq!(
            alpha(bubble_brush(Scheme::Light, BubbleMaterial::Follow, WindowMaterial::Mica, 0.4)),
            alpha(bubble_brush(Scheme::Light, BubbleMaterial::Translucent, WindowMaterial::Mica, 0.4))
        );
    }

    /// 跟随 + mica-alt 才有主干 `CoolShift` 那笔冷移（红降 3%、绿降 1.5%、蓝不动）：
    /// 浅 #F3F3F3 → #ECEFF3、深 #202020 → #1F2020。其余档一律原色面色。
    #[test]
    fn bubble_mica_alt_cool_shift_only_colors_the_follow_tier() {
        let rgb = |brush: Brush| match brush {
            Brush::Solid(color) => (color.r, color.g, color.b),
            Brush::Theme(_) => (0, 0, 0),
        };
        assert_eq!(
            rgb(bubble_brush(Scheme::Light, BubbleMaterial::Follow, WindowMaterial::MicaAlt, 0.6)),
            (0xEC, 0xEF, 0xF3)
        );
        assert_eq!(
            rgb(bubble_brush(Scheme::Dark, BubbleMaterial::Follow, WindowMaterial::MicaAlt, 0.6)),
            (0x1F, 0x20, 0x20)
        );
        // 不跟随（亚克力档）时窗口材质不参与：主干 `BuildBubbleBrush` 里 material 被钉成 acrylic
        assert_eq!(
            rgb(bubble_brush(Scheme::Light, BubbleMaterial::Acrylic, WindowMaterial::MicaAlt, 0.6)),
            (0xF3, 0xF3, 0xF3)
        );
    }

    // ===================== HC 档 =====================
    //
    // 八槽的**夹具值**：每颗 RGB 互不相同、alpha 一律 255（HC 全程实心）。
    // 逐键断言全部拿这套夹具去核「等于哪一颗槽」，**不核任何一颗 ARGB 的真值** ——
    // 真值由 `GetSysColor` 在运行时给（见文件头 HC 段），在这里钉死就等于自造值。
    fn hc_fixture() -> HighContrastColors {
        HighContrastColors {
            window: argb(0xFF, 0x0A_0B_0C),
            window_text: argb(0xFF, 0x1D_1E_1F),
            gray_text: argb(0xFF, 0x2E_2F_30),
            highlight: argb(0xFF, 0x3F_40_41),
            highlight_text: argb(0xFF, 0x4C_4D_4E),
            hotlight: argb(0xFF, 0x5A_5B_5C),
            button_face: argb(0xFF, 0x6B_6C_6D),
            button_text: argb(0xFF, 0x7C_7D_7E),
        }
    }

    /// `GetSysColor` 那一发索引必须逐颗等于本机 `windows-sys-0.61.2` 里的常量
    /// （`Graphics/Gdi/mod.rs:771-796`）：经典索引**不是从 1 连排的**，
    /// `COLOR_WINDOW`=5 不是 1、`COLOR_HIGHLIGHTTEXT`=14 不是 12 —— 记错了这里就红。
    #[test]
    fn hc_slots_carry_distinct_syscolor_indices() {
        let want = [
            (HcSlot::Window, "Window", 5),
            (HcSlot::WindowText, "WindowText", 8),
            (HcSlot::GrayText, "GrayText", 17),
            (HcSlot::Highlight, "Highlight", 13),
            (HcSlot::HighlightText, "HighlightText", 14),
            (HcSlot::Hotlight, "Hotlight", 26),
            (HcSlot::ButtonFace, "ButtonFace", 15),
            (HcSlot::ButtonText, "ButtonText", 18),
        ];
        assert_eq!(HcSlot::ALL.len(), 8);
        for (slot, name, index) in want {
            assert_eq!(slot.resource_name(), name, "槽名拼错 = 词典里引不到那一句");
            assert_eq!(slot.sys_color_index(), index, "COLOR_* 索引记错了：{name}");
        }
        // 反向半边：八颗索引两两不同（撞号就是两档被并成一颗系统色）
        for (i, a) in HcSlot::ALL.iter().enumerate() {
            for (j, b) in HcSlot::ALL.iter().enumerate() {
                if i != j {
                    assert_ne!(a.sys_color_index(), b.sys_color_index());
                }
            }
        }
    }

    /// 逐键回核主干词典本体：`Theme/Tokens.xaml` 的 HighContrast 词典里那一行
    /// 必须**同时**对上①键名 ②`HcToken::slot()` 给的槽 ③`mainline_line()` 给的行号。
    /// 三样里任一样漂了就红 —— 主干词典是权威，本文件那张表只是它的抄本。
    #[test]
    fn hc_token_table_matches_the_mainline_dictionary() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../Theme/Tokens.xaml");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("读主干词典失败 {path:?}：{e}"));
        let opener = r#"<ResourceDictionary x:Key="HighContrast">"#;
        let start = text
            .find(opener)
            .expect("主干 Tokens.xaml 里没有 x:Key=\"HighContrast\" 词典");
        let block_start = text[..start].matches('\n').count() + 1;
        let block = &text[start..start + text[start..].find("</ResourceDictionary>").expect("HC 词典没收尾")];
        // 词典里画刷行的总数 = 表长：主干加一颗而分叉没登记 ⇒ 这里红
        let solid_rows = block.lines().filter(|l| l.contains("<SolidColorBrush")).count();
        assert_eq!(solid_rows, HcToken::ALL.len(), "主干 HC 词典的画刷数与本表不齐");
        for token in HcToken::ALL {
            let key = format!("x:Key=\"{}\"", token.name());
            let slot = format!("{{ThemeResource SystemColor{}Color}}", token.slot().resource_name());
            let hit = block
                .lines()
                .enumerate()
                .find(|(_, line)| line.contains(&key))
                .unwrap_or_else(|| panic!("主干 HC 词典里没有 {}", token.name()));
            let line = block_start + hit.0;
            let recorded: usize = token
                .mainline_line()
                .strip_prefix("Tokens.xaml:")
                .unwrap_or_else(|| panic!("行号串形状不对：{}", token.mainline_line()))
                .parse()
                .expect("主干行号不是数字");
            assert_eq!(
                line, recorded,
                "{} 的行号漂了：本表记 {}、主干在 L{line}",
                token.name(),
                token.mainline_line(),
            );
            assert!(
                hit.1.contains(&slot),
                "{} 的槽对不上：本表给 {}，主干那行是 {}",
                token.name(),
                slot,
                hit.1.trim(),
            );
        }
    }

    /// 逐键 ARGB 逐字节：`hc_palette` 每一档必须**正好**等于它声明的那颗槽的夹具值，
    /// 一字节都不能差。反向半边两头都咬：
    /// ① 别名不同（三档灰 ≠ 两档正文）的两档必须真的不等 —— 压平就红；
    /// ② 主干**故意**压平的两档（HC 的 hover/pressed 同为 Highlight，`Tokens.xaml:319/320`）
    ///    必须相等 —— 有人"顺手修成三态三颗色"也红。
    #[test]
    fn hc_palette_is_byte_equal_to_its_declared_slot() {
        let c = hc_fixture();
        let p = hc_palette(Scheme::Dark, &c);
        let face = |brush: Brush| match brush {
            Brush::Solid(color) => (color.a, color.r, color.g, color.b),
            Brush::Theme(_) => panic!("HC 档不许留主题画刷：它跟着的是非 HC 词典"),
        };
        let want = |slot: HcSlot| {
            let color = c.slot(slot);
            (color.a, color.r, color.g, color.b)
        };
        // 主干 36 键里落在 Palette 上的每一档
        assert_eq!(face(p.surface), want(HcSlot::Window)); // Tokens.xaml:290
        assert_eq!(face(p.surface_alt), want(HcSlot::Window)); // :291
        assert_eq!(face(p.card), want(HcSlot::Window)); // :292
        assert_eq!(face(p.card_secondary), want(HcSlot::Window)); // :293
        assert_eq!(face(p.subtle_hover), want(HcSlot::Highlight)); // :294
        assert_eq!(face(p.text_primary), want(HcSlot::WindowText)); // :295
        assert_eq!(face(p.text_secondary), want(HcSlot::WindowText)); // :296
        assert_eq!(face(p.text_tertiary), want(HcSlot::GrayText)); // :297
        assert_eq!(face(p.text_disabled), want(HcSlot::GrayText)); // :298
        assert_eq!(face(p.stroke), want(HcSlot::WindowText)); // :299
        assert_eq!(face(p.stroke_subtle), want(HcSlot::WindowText)); // :300
        assert_eq!(face(p.accent), want(HcSlot::Highlight)); // :301
        assert_eq!(face(p.on_accent), want(HcSlot::HighlightText)); // :304
        assert_eq!(face(p.bubble), want(HcSlot::Window)); // :306
        assert_eq!(face(p.success), want(HcSlot::WindowText)); // :307
        assert_eq!(face(p.warning), want(HcSlot::WindowText)); // :309
        assert_eq!(face(p.warning_bg), want(HcSlot::Window)); // :310
        assert_eq!(face(p.error), want(HcSlot::Hotlight)); // :311
        assert_eq!(face(p.info), want(HcSlot::WindowText)); // :313
        assert_eq!(face(p.info_bg), want(HcSlot::Window)); // :314
        assert_eq!(face(p.control_fill), want(HcSlot::Window)); // :318
        assert_eq!(face(p.control_hover), want(HcSlot::Highlight)); // :319
        assert_eq!(face(p.control_pressed), want(HcSlot::Highlight)); // :320
        // 主干无 HC 键、落框架 HC 词典的那五档
        assert_eq!(face(p.control_disabled), want(HcSlot::ButtonFace)); // generic.xaml:4768
        assert_eq!(face(p.control_stroke), want(HcSlot::ButtonText)); // :4793
        assert_eq!(face(p.control_stroke_pressed), want(HcSlot::ButtonText)); // :4792
        assert_eq!(face(p.accent_disabled), want(HcSlot::Window)); // :4791
        assert_eq!(face(p.subtle_pressed), want(HcSlot::ButtonFace)); // :4776
        assert_eq!(face(p.subtle_disabled), want(HcSlot::ButtonFace)); // :4777
        // Color 那几档也得同槽（resource_overrides 只吃 Color）
        assert_eq!(p.on_accent_color, c.slot(HcSlot::HighlightText));
        assert_eq!(p.control_fill_color, c.slot(HcSlot::Window));
        assert_eq!(p.subtle_hover_color, c.slot(HcSlot::Highlight));
        assert_eq!(p.subtle_pressed_color, c.slot(HcSlot::ButtonFace));
        // ① 反向半边：别名不同的档必须不等
        assert_ne!(face(p.text_tertiary), face(p.text_primary), "三档灰被压成正文了");
        assert_ne!(face(p.error), face(p.success), "Hotlight 被压成窗口文本色了");
        assert_ne!(face(p.on_accent), face(p.accent), "反白字被压成底色了");
        assert_ne!(face(p.control_fill), face(p.control_hover), "静置被压成悬停了");
        assert_ne!(face(p.subtle_hover), face(p.subtle_disabled));
        assert_ne!(face(p.control_stroke), face(p.control_fill), "描边化进底里了");
        // ② 主干故意压平的档必须相等（引号里是主干行号）
        assert_eq!(face(p.control_hover), face(p.control_pressed), "Tokens.xaml:319/320 本就同颗");
        assert_eq!(face(p.text_primary), face(p.text_secondary), "Tokens.xaml:295/296 本就同颗");
        assert_eq!(face(p.stroke), face(p.stroke_subtle), "Tokens.xaml:299/300 本就同颗");
    }

    /// **三档不压平**：同一颗键在 Light / Dark / HC 三档下必须各有其值。
    /// 判据用 HC 夹具的 alpha=255 + 独有 RGB 去撞浅深两档的字面常量，
    /// 任一档被当成另一档抄（例如 HC 忘了覆写、落回 `Palette::for_scheme`）就红。
    /// 表里只挑**浅深两档都是实心且互不相同**的键：`stroke`/`error` 那种两档同为
    /// `Brush::Theme` 的不能进来，否则 `face()` 把它们都折成同一颗，三档判据成假绿。
    #[test]
    fn hc_light_and_dark_are_three_distinct_tiers() {
        let c = hc_fixture();
        let hc = hc_palette(Scheme::Dark, &c);
        let light = Palette::for_scheme(Scheme::Light);
        let dark = Palette::for_scheme(Scheme::Dark);
        let face = |brush: Brush| match brush {
            Brush::Solid(color) => (color.a, color.r, color.g, color.b),
            Brush::Theme(_) => panic!("这一档浅/深两档必须都是实心，否则表里的判据是假的"),
        };
        // 每一行 = (HC, Light, Dark) 三档同键；HC 那颗来自夹具
        let rows: [(&str, (u8, u8, u8, u8), (u8, u8, u8, u8), (u8, u8, u8, u8)); 6] = [
            (
                "surface_alt",
                face(hc.surface_alt),
                face(light.surface_alt),
                face(dark.surface_alt),
            ),
            (
                "card_secondary",
                face(hc.card_secondary),
                face(light.card_secondary),
                face(dark.card_secondary),
            ),
            (
                "text_tertiary",
                face(hc.text_tertiary),
                face(light.text_tertiary),
                face(dark.text_tertiary),
            ),
            (
                "control_hover",
                face(hc.control_hover),
                face(light.control_hover),
                face(dark.control_hover),
            ),
            ("bubble", face(hc.bubble), face(light.bubble), face(dark.bubble)),
            (
                "warning_bg",
                face(hc.warning_bg),
                face(light.warning_bg),
                face(dark.warning_bg),
            ),
        ];
        for (key, hc_face, light_face, dark_face) in rows {
            assert_ne!(hc_face, light_face, "{key}：HC 档与 Light 档压平了");
            assert_ne!(hc_face, dark_face, "{key}：HC 档与 Dark 档压平了");
            // 反向半边：浅深两档本身也不许被并成一颗（并了的话上面两条会同时假绿）
            assert_ne!(light_face, dark_face, "{key}：Light 与 Dark 档压平了");
        }
        // 重点核：HC 的三档灰就是系统灰字色（`Tokens.xaml:297`），浅深两档都是带
        // alpha 的叠加黑/白 —— 夹具那颗撞不上任何字面常量
        assert_eq!(hc.text_tertiary, Brush::Solid(c.slot(HcSlot::GrayText)));
        // 反向半边：HC 忘了覆写、整包落回 `for_scheme` 就红
        assert_ne!(hc.bubble, dark.bubble, "HC 气泡落回深色档常量了");
        assert_ne!(hc.bubble, light.bubble, "HC 气泡落回浅色档常量了");
        assert_ne!(hc.control_hover, light.control_hover);
        assert_ne!(hc.control_hover, dark.control_hover);
    }

    /// HC 档整套笔刷里**一支都不许留** `Brush::Theme`：主题画刷跟着的是非 HC 词典，
    /// 在 HC 档留着它就是那一档没被系统色接管（`surface`/`card`/`stroke`/`text_primary`/
    /// `accent`/`info`/`error` 六档在 Light/Dark 里原本全是 Theme 画刷）。
    /// 反向半边：Light 档**确实**有 Theme 画刷，否则这个循环是空转。
    #[test]
    fn hc_palette_leaves_no_theme_brush_behind() {
        let c = hc_fixture();
        let p = hc_palette(Scheme::Light, &c);
        let brushes = [
            p.surface,
            p.surface_alt,
            p.card,
            p.card_secondary,
            p.stroke,
            p.stroke_subtle,
            p.text_primary,
            p.text_secondary,
            p.text_tertiary,
            p.text_disabled,
            p.accent,
            p.on_accent,
            p.control_fill,
            p.control_hover,
            p.control_pressed,
            p.control_disabled,
            p.control_stroke,
            p.control_stroke_pressed,
            p.accent_disabled,
            p.subtle_hover,
            p.subtle_rest,
            p.subtle_pressed,
            p.subtle_disabled,
            p.bubble,
            p.info,
            p.info_bg,
            p.success,
            p.warning,
            p.warning_bg,
            p.error,
            p.transparent,
        ];
        assert_eq!(brushes.len(), 31, "Palette 的 31 支画刷得全在这");
        for brush in brushes {
            assert!(matches!(brush, Brush::Solid(_)), "HC 档漏了一支主题画刷");
        }
        // 实心那半边：除 `transparent`/`subtle_rest`（框架 HC 词典就是字面 Transparent，
        // generic.xaml:4774）之外，所有档 alpha 必须 255 —— HC 全程不做半透明
        // （`Tokens.xaml:316-317` 那两行注释、`MainWindow.xaml.cs:15652` 同一条约定）。
        let solid_only = [p.surface, p.card, p.text_tertiary, p.error, p.control_hover];
        assert!(
            solid_only.iter().all(|b| matches!(b, Brush::Solid(x) if x.a == 255)),
            "HC 档出现了带 alpha 的档"
        );
        assert!(matches!(p.subtle_rest, Brush::Solid(x) if x.a == 0));
        // 反向半边：Light 档有 Theme 画刷 ⇒ 上面那个循环不是空转
        let light = Palette::for_scheme(Scheme::Light);
        assert!(matches!(light.surface, Brush::Theme(_)));
        assert!(matches!(light.accent, Brush::Theme(_)));
        assert!(matches!(light.error, Brush::Theme(_)));
    }

    /// 数据色板：主干 HC 词典把六条序列**全压成同一颗**窗口文本色（`Tokens.xaml:323-328`），
    /// 代价由线型补：`hc_series_dash` 照抄主干 `SeriesDash`（`MainWindow.xaml.cs:15785-15788`）。
    /// 反向半边两头：六颗必须**全等**（有人顺手配六色就红），线型必须**不全等**。
    #[test]
    fn hc_chart_series_flatten_to_one_color_and_separate_by_dash() {
        let c = hc_fixture();
        let series = hc_chart_palette(&c);
        assert_eq!(series.len(), 6);
        let face = |brush: Brush| match brush {
            Brush::Solid(color) => (color.a, color.r, color.g, color.b),
            Brush::Theme(_) => panic!("HC 图表色板不许是主题画刷"),
        };
        let one = face(c.slot(HcSlot::WindowText).into());
        for brush in &series {
            assert_eq!(face(*brush), one, "六条序列在 HC 必须同色（Tokens.xaml:323-328）");
        }
        assert_eq!(face(hc_chart_palette(&c)[0]), face(hc_chart_palette(&c)[5]));
        // 线型档：index 0/3/6 实线，其余 {3*(i%3), 2}
        assert_eq!(hc_series_dash(0), None);
        assert_eq!(hc_series_dash(3), None);
        assert_eq!(hc_series_dash(6), None);
        assert_eq!(hc_series_dash(1), Some([3.0, 2.0]));
        assert_eq!(hc_series_dash(2), Some([6.0, 2.0]));
        assert_eq!(hc_series_dash(4), Some([3.0, 2.0]));
        assert_eq!(hc_series_dash(5), Some([6.0, 2.0]));
        assert_ne!(hc_series_dash(1), hc_series_dash(2), "序列 2/3 撞线型了就分不开");
        // 发数与浅深档对齐（`main.rs` 那侧按索引取色，少发就错位）
        assert_eq!(
            series.len(),
            chart_palette(Scheme::Light).len(),
            "HC 色板发数与浅深档不齐"
        );
        assert_eq!(
            chart_palette(Scheme::Light).len(),
            chart_palette(Scheme::Dark).len()
        );
        // 反向半边：浅深两档的六条序列**不**同色，否则上面那个"HC 全压成一颗"没参照
        let light_series = chart_palette(Scheme::Light);
        assert_ne!(
            face(light_series[0]),
            face(light_series[1]),
            "浅档自己就压平了，HC 的压平判据成假绿"
        );
        assert_ne!(face(light_series[0]), one, "HC 序列色撞进浅档常量了");
        // 热力图空档与网格线：一落窗口底、一落窗口文本（Tokens.xaml:329/330）
        assert_eq!(HcToken::ChartHeatEmpty.color(&c), c.slot(HcSlot::Window));
        assert_eq!(HcToken::ChartGrid.color(&c), c.slot(HcSlot::WindowText));
        assert_eq!(face(HcToken::ChartGrid.color(&c).into()), one);
        // 反向半边：空档与网格线在 HC 也必须分得开（一底一线，压平了图上就看不见网格）
        assert_ne!(
            HcToken::ChartHeatEmpty.color(&c),
            HcToken::ChartGrid.color(&c)
        );
        assert_eq!(HcToken::ChartSeries1.slot(), HcSlot::WindowText);
        assert_eq!(HcToken::ChartSeries6.slot(), HcSlot::WindowText);
    }
}
