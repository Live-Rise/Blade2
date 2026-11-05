$src = [System.IO.File]::ReadAllText('E:\Syncthing\DshWinUI\rust\src\main.rs')
function FnBody($text, $start, $end) {
  $i = $text.IndexOf($start)
  $rest = $text.Substring($i + $start.Length)
  $j = $rest.IndexOf($end)
  Write-Host ("start idx={0} end rel idx={1} window len={2}" -f $i, $j, $(if ($j -ge 0) { $j } else { $rest.Length }))
  return $rest.Substring(0, [Math]::Max(0, [Math]::Min($j, $rest.Length)))
}
$b = FnBody $src 'fn tool_line(' 'fn max_tokens_card('
$ms = [regex]::Matches($b, [regex]::Escape('opacity(0.8)'))
Write-Host ("hits=" + $ms.Count)
foreach ($m in $ms) {
  $s = [Math]::Max(0, $m.Index - 60)
  Write-Host ("..." + $b.Substring($s, 140).Replace("`n"," | ") + "...")
}
