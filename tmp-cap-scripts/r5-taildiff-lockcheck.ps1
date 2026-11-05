# R5 视觉尾差四件：新增/受影响锁的同窗预演（不跑 cargo test，语义逐字同 main.rs 里的窗函数）。
# 用法：pwsh -File tmp-cap-scripts\r5-taildiff-lockcheck.ps1
# 退出码 0 = 全绿；非 0 = 有锁翻红。末段做四发变异，证明每颗新锁咬得住。

$ErrorActionPreference = 'Stop'
$src = Get-Content -LiteralPath 'E:\Syncthing\DshWinUI\rust\src\main.rs' -Raw

# window/fn_body 的复刻：start 首次出现之后、其后的 end 首次出现之前（不含 end）；
# fnBody 找不到 end 就吃到底（map_or(rest)），window 找不到就报错。
function Get-Window([string]$text, [string]$start, [string]$end) {
    $i = $text.IndexOf($start)
    if ($i -lt 0) { throw "锚点找不到: $start" }
    $rest = $text.Substring($i + $start.Length)
    if ($end -eq '') { return $rest }
    $j = $rest.IndexOf($end)
    if ($j -lt 0) { throw "锚点 $start 之后找不到 $end" }
    return $rest.Substring(0, $j)
}
function Get-FnBody([string]$text, [string]$start, [string]$end) {
    $i = $text.IndexOf($start)
    if ($i -lt 0) { throw "锚点找不到: $start" }
    $rest = $text.Substring($i + $start.Length)
    $j = $rest.IndexOf($end)
    if ($j -lt 0) { return $rest }
    return $rest.Substring(0, $j)
}

$script:fail = 0
function Check([string]$name, [bool]$ok, [string]$why) {
    if ($ok) { Write-Host ("PASS  " + $name) }
    else { $script:fail++; Write-Host ("FAIL  " + $name + "  <- " + $why) }
}

# ---- 新增四颗（项11） -------------------------------------------------------

$t = Get-FnBody $src 'fn settings_toggle(' 'fn settings_general('
Check 'the_settings_toggle_box_hugs_its_track' ($t.Contains('.min_width(0.0)')) '开关盒 154px 富余回来了'

$pet = Get-FnBody $src 'fn settings_pet(' 'fn pet_library_rows('
$plainCount = ([regex]::Matches($pet, [regex]::Escape('self.settings_card_plain('))).Count
Check 'the_pet_cards_carry_no_dividers(1)' (-not $pet.Contains('self.settings_card(')) '宠物区用回了带 divider 的卡壳'
Check 'the_pet_cards_carry_no_dividers(2)' ($plainCount -eq 4) ("settings_card_plain 调用数 = $plainCount（要 4）")
$plain = Get-FnBody $src 'fn settings_card_plain(' 'fn settings_combo('
Check 'the_pet_cards_carry_no_dividers(3)' (-not $plain.Contains('settings_divider')) '裸卡体自己长出分隔线'

$res = Get-FnBody $src 'fn search_input_resources(' 'struct PillFace {'
foreach ($k in @('TextControlBackground','TextControlBackgroundPointerOver','TextControlBackgroundFocused',
                 'TextControlBorderBrush','TextControlBorderBrushPointerOver','TextControlBorderBrushFocused')) {
    Check "the_search_input_kills_the_focused_chrome($k)" $res.Contains('"'+$k+'",') "六键缺 $k"
}
Check 'the_search_input_kills_the_focused_chrome(padding)' $res.Contains('"ButtonPadding"') '载体钮 Padding 没归零'
$strip = Get-FnBody $src 'fn strip(' 'const TRAJECTORY_FILTERS'
Check 'the_search_input_kills_the_focused_chrome(carrier)' $strip.Contains('resource_overrides(search_input_resources())') '输入框没包资源载体钮'
Check 'the_search_input_kills_the_focused_chrome(a11y)' $strip.Contains('"SessionSearchBox"') 'AutomationId 丢了'

$desc = Get-FnBody $src 'fn settings_section_desc(' 'fn settings_line('
Check 'the_section_desc_leaves_the_title_gap(1)' $desc.Contains('.margin(th([0.0, 8.0, 0.0, 4.0]))') '段说明顶距 8 丢了'
$gen = Get-FnBody $src 'fn settings_general(' 'fn font_stepper('
Check 'the_section_desc_leaves_the_title_gap(2)' $gen.Contains('self.settings_section_desc(') 'general 段说明又内联了'
$secCount = ([regex]::Matches($src, [regex]::Escape('"section-desc"'))).Count
Check 'the_section_desc_leaves_the_title_gap(3)' ($secCount -eq 1) ("section-desc 字面量 = $secCount（要 1）")

# ---- 受影响的既有锁（窗口跨到本次改动的一律复跑） ---------------------------

Check 'the_toggle_suppresses_the_native_on_off_content(1)' (($t.Contains('TextBlock::new()')) -and ($t.Contains('if on { "开" } else { "关" }'))) '左侧状态字没了'
Check 'the_toggle_suppresses_the_native_on_off_content(2)' ($t.Contains('SlotView::new(ToggleSwitchSlot::OnContent')) 'OnContent 槽没清空'
Check 'the_toggle_suppresses_the_native_on_off_content(3)' ($t.Contains('SlotView::new(ToggleSwitchSlot::OffContent')) 'OffContent 槽没清空'
$tbCount = ([regex]::Matches($t, [regex]::Escape('TextBlock::new()'))).Count
Check 'the_toggle_suppresses_the_native_on_off_content(4)' ($tbCount -ge 3) "TextBlock::new() = $tbCount（要 >=3）"

$lib = Get-FnBody (Get-FnBody $src 'fn settings_pet(' 'fn pet_library_rows(') '"pet-library"' 'self.pet_library_rows('
Check 'the_pet_library_card_renders_headerless(1)' (-not $lib.Contains('"宠物库"')) '宠物库卡头回来了'
Check 'the_pet_library_card_renders_headerless(2)' (-not $lib.Contains('"已收录的宠物"')) '描述行回来了'

$cardFn = Get-Window $src 'fn settings_card(' 'fn settings_divider('
$cardLines = ($cardFn -split "`n").Count
Check 'the_model_total…(window lines)' (($cardLines -ge 45) -and ($cardLines -le 90)) "settings_card 窗口 = $cardLines 行（要 45..90）"
Check 'the_model_total…(None 右槽)' $cardFn.Contains('settings_card_with_trailing(key, header, subtitle, None') '薄壳右槽不是 None'
Check 'the_model_total…(STAR Auto)' $cardFn.Contains('GridLength::STAR, GridLength::Auto') '卡头右槽网格漂了'
Check 'the_model_total…(None => heading)' $cardFn.Contains('None => heading') '无右槽卡被硬塞 Grid'
Check 'subtitle 通道在册' $cardFn.Contains('if !subtitle.is_empty()') '副标题通道被删'

$named = Get-Window $src 'fn usage_named_card(' 'fn settings_number('
Check 'usage_named_card 透传右槽' $named.Contains('settings_card_with_trailing(') '右槽没透传'
$usage = Get-Window $src 'fn settings_usage(' 'fn usage_heat_rows('
Check '模型合计挂卡头右列' $usage.Contains('model_total,') '合计不在右槽'
$rowsWin = Get-Window $src 'fn usage_model_rows(' 'fn usage_named_card('
Check '合计不占卡体首行' (-not $rowsWin.Contains('"model-total",')) '合计回到卡体'

$star = Get-FnBody $src 'fn settings_card_with_trailing(' 'fn settings_divider('
Check 'the_star_auto_grids…(卡头右槽)' $star.Contains('Border::new().grid_column(1).content(trailing)') '右槽没钉列'
$range = Get-FnBody $src 'fn usage_range_row(' 'fn usage_heat_rows('
Check 'the_star_auto_grids…(时间范围行)' $range.Contains('Border::new().grid_column(1).content(bar)') '范围行没钉列'
$nav = Get-FnBody $src 'fn nav_item_menu(' 'fn session_row('
Check 'the_star_auto_grids…(nav chevron)' $nav.Contains('.grid_column(1)') 'chevron 没钉列'

$pill = Get-Window $src '"search",' 'Grid::new()'
Check 'L-C1 圆角夹取' $pill.Contains('.corner_radius(radius::clamp(radius::PILL, size::TOUCH_TARGET))') '夹取没了'
Check 'L-C1 裸 PILL 未回流' (-not $pill.Contains('.corner_radius(radius::PILL)')) '裸 PILL 回来了'
Check 'L-C1 高宽基准' (($pill.Contains('.height(size::TOUCH_TARGET)')) -and ($pill.Contains('.width(size::SEARCH_MAX_WIDTH)'))) '高/宽基准丢了'

$aboutWin = Get-Window $src 'fn settings_about(' 'fn about_beta_notice('
Check '内测声明单发' (([regex]::Matches($aboutWin, [regex]::Escape('self.about_beta_notice(p)'))).Count -eq 1) '声明被拆多发'
$notice = Get-Window $src 'fn about_beta_notice(' 'fn settings_pet('
Check '内测声明无手写 divider' (-not $notice.Contains('settings_divider')) '手写了分隔线'

# ---- 变异演练：四颗新锁必须咬得住（在内存副本上动手，不落盘） -----------------
# 口径：把**原锁判据**原样跑在变异后的源上，判据翻 false = 咬住。

function Lock-ToggleBox([string]$text) {
    (Get-FnBody $text 'fn settings_toggle(' 'fn settings_general(').Contains('.min_width(0.0)')
}
function Lock-PetPlain([string]$text) {
    $pet = Get-FnBody $text 'fn settings_pet(' 'fn pet_library_rows('
    $plain = Get-FnBody $text 'fn settings_card_plain(' 'fn settings_combo('
    (-not $pet.Contains('self.settings_card(')) -and
    (([regex]::Matches($pet, [regex]::Escape('self.settings_card_plain('))).Count -eq 4) -and
    (-not $plain.Contains('settings_divider'))
}
function Lock-SearchFocused([string]$text) {
    (Get-FnBody $text 'fn search_input_resources(' 'struct PillFace {').Contains('"TextControlBackgroundFocused",')
}
function Lock-DescGap([string]$text) {
    (Get-FnBody $text 'fn settings_section_desc(' 'fn settings_line(').Contains('.margin(th([0.0, 8.0, 0.0, 4.0]))')
}

Write-Host ''
Write-Host '== 变异演练（锁判据应当翻 false = 咬住） =='
$m1 = $src.Replace('.min_width(0.0)', '')
Check 'MUT toggle 摘 min_width ⇒ 咬住' (-not (Lock-ToggleBox $m1)) '锁没咬住'

$m2 = $src.Replace('self.settings_card_plain(
                "pet-display",', 'self.settings_card(
                "pet-display",')
Check 'MUT pet-display 换回 settings_card ⇒ 咬住' (-not (Lock-PetPlain $m2)) '锁没咬住'

$m3 = $src.Replace('        .set("TextControlBackgroundFocused", transparent)', '')
Check 'MUT 摘 Focused 资源键 ⇒ 咬住' (-not (Lock-SearchFocused $m3)) '锁没咬住'

$m4 = $src.Replace('.margin(th([0.0, 8.0, 0.0, 4.0]))', '.margin(th([0.0, 0.0, 0.0, 4.0]))')
Check 'MUT 顶距 8 改回 0 ⇒ 咬住' (-not (Lock-DescGap $m4)) '锁没咬住'

if ($script:fail -gt 0) { Write-Host ("`n结果：{0} 条翻红" -f $script:fail); exit 1 }
Write-Host "`n结果：全绿"
exit 0
