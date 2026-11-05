$src = [System.IO.File]::ReadAllText('E:\Syncthing\DshWinUI\rust\src\main.rs')
function FnBody($text, $start, $end) {
  $i = $text.IndexOf($start); if ($i -lt 0) { return "MISSING" }
  $rest = $text.Substring($i + $start.Length)
  $j = $rest.IndexOf($end); if ($j -lt 0) { return $rest }
  return $rest.Substring(0, $j)
}
function Check($name, $cond) { Write-Host ("{0} {1}" -f ($(if ($cond) {'PASS'} else {'FAIL'})), $name) }
$b = FnBody $src 'fn tool_line(' 'fn max_tokens_card('
Check ('tool: no 0.8 border → hits=' + ([regex]::Matches($b, [regex]::Escape('opacity(0.8)')).Count)) (([regex]::Matches($b, [regex]::Escape('opacity(0.8)')).Count) -eq 0)
Check 'tool: chevron kept' ($b.Contains('mark_size(COMPACTION_CHEVRON'))
