$src = [System.IO.File]::ReadAllText('E:\Syncthing\DshWinUI\rust\src\main.rs')
$tok = [System.IO.File]::ReadAllText('E:\Syncthing\DshWinUI\rust\src\tokens.rs')
function FnBody($text, $start, $end) {
  $i = $text.IndexOf($start); if ($i -lt 0) { return "MISSING-START $start" }
  $rest = $text.Substring($i + $start.Length)
  $j = $rest.IndexOf($end); if ($j -lt 0) { return $rest }
  return $rest.Substring(0, $j)
}
function Check($name, $cond) { Write-Host ("{0} {1}" -f ($(if ($cond) {'PASS'} else {'FAIL'})), $name) }

# 1 toggle
$b = FnBody $src 'fn settings_toggle(' 'fn settings_general('
Check 'toggle: left label'   ($b.Contains('if on { "开" } else { "关" }'))
Check 'toggle: OnContent slot' ($b.Contains('SlotView::new(ToggleSwitchSlot::OnContent'))
Check 'toggle: OffContent slot' ($b.Contains('SlotView::new(ToggleSwitchSlot::OffContent'))
Check ('toggle: TextBlock count>=3 → ' + ([regex]::Matches($b, [regex]::Escape('TextBlock::new()')).Count)) (([regex]::Matches($b, [regex]::Escape('TextBlock::new()')).Count) -ge 3)

# 2 video skin base
$b = FnBody $src 'let skin_laid = backdrop.is_some();' 'let colsync = layout::sync_layout('
Check 'skin: match tuple' ($b.Contains('match (skin_laid, self.skin_video.is_some())'))
Check 'skin: image transparent' ($b.Contains('(true, false) => p.transparent'))
Check 'skin: video surface_alt' ($b.Contains('_ => p.surface_alt'))

# 3 nav indent
Check 'tokens: 30.8' ($tok.Contains('pub const NAV_GROUP_INDENT: f64 = 30.8;'))
$b = FnBody $src 'fn nav_item_menu(' 'fn session_row('
Check 'nav: spacing 16' ($b.Contains('.spacing(16.0)'))
Check 'nav: grid_column(1)' ($b.Contains('.grid_column(1)'))

# 4 star-auto pins
$b = FnBody $src 'fn settings_card_with_trailing(' 'fn settings_divider('
Check 'card: trailing pin' ($b.Contains('Border::new().grid_column(1).content(trailing)'))
$b = FnBody $src 'fn usage_range_row(' 'fn usage_heat_rows('
Check 'range: bar pin' ($b.Contains('Border::new().grid_column(1).content(bar)'))

# 5 heat body
$b = FnBody $src 'fn usage_heat_cells(' 'fn usage_heat_legend('
Check 'heat: body row' ($b.Contains('"heat-body"'))
Check 'heat: chart layer' ($b.Contains('"heat-chart"'))
Check 'heat: centered' ($b.Contains('horizontal_alignment(HorizontalAlignment::Center)'))
Check 'heat: top margin' ($b.Contains('.margin(th([0.0, 10.0, 0.0, 0.0]))'))

# 6 pet library headerless
$b = FnBody $src 'fn settings_pet(' 'fn pet_library_rows('
$b2 = FnBody $b '"pet-library"' 'self.pet_library_rows('
Check ('pet: no quoted 宠物库 → hits=' + ([regex]::Matches($b2, '"宠物库"').Count)) (([regex]::Matches($b2, '"宠物库"').Count) -eq 0)
Check ('pet: no 已收录的宠物 → hits=' + ([regex]::Matches($b2, '已收录的宠物').Count)) (([regex]::Matches($b2, '已收录的宠物').Count) -eq 0)

# 7 memory tight
$b = FnBody $src 'fn settings_memory(' 'fn settings_computer_control('
Check 'memory: store-body' ($b.Contains('"store-body"'))
Check ('memory: zero SETTINGS_ROW → ' + ([regex]::Matches($b, [regex]::Escape('pad::SETTINGS_ROW')).Count)) (([regex]::Matches($b, [regex]::Escape('pad::SETTINGS_ROW')).Count) -eq 0)
Check 'memory: line-state' ($b.Contains('"line-state"'))

# 8 instructions fallback
$b = FnBody $src 'fn instructions_status(' 'fn ctrl_enter_mode('
Check 'instr: same fallback' ($b.Contains('self.field(INSTRUCTIONS_KEY, &self.instructions_saved)'))
Check 'instr: count line' ($b.Contains('字数 {0} / {1}'))

# 9 provider list
$b = FnBody $src 'fn settings_models(' 'fn provider_card('
Check 'models: provider-list' ($b.Contains('"provider-list"'))
Check 'models: spacing S4' ($b.Contains('.spacing(space::S4)'))

# 10 tool line icon
$b = FnBody $src 'fn tool_line(' 'fn tool_duration_text('
Check ('tool: no 0.8 border → hits=' + ([regex]::Matches($b, [regex]::Escape('opacity(0.8)')).Count)) (([regex]::Matches($b, [regex]::Escape('opacity(0.8)')).Count) -eq 0)
Check 'tool: chevron kept' ($b.Contains('mark_size(COMPACTION_CHEVRON'))

# 11 heat legend painted gate (window: legend fn → trend fn)
$b = FnBody $src 'fn usage_heat_legend(' 'fn usage_trend_rows('
Check 'legend: (0..5)' ($b.Contains('(0..5)'))
Check 'legend: painted gate' ($b.Contains('if !painted'))
Check 'legend: right aligned' ($b.Contains('HorizontalAlignment::Right'))
Check 'legend: empty brush' ($b.Contains('heat_empty_brush(self.scheme)'))
Check 'legend: opacity ramp' ($b.Contains('heat_opacity(level)'))
Check 'legend: 12 side' ($b.Contains('STATS_HEAT_LEGEND'))
