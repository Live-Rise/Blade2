#!/usr/bin/env bash
# 主干漂移基线：记录/比对主线（C#/XAML）文件的 sha256 与行数。
# rust 分叉的移植结论都以主干某一时刻为快照，收尾前用 --check 复查一次。
# 用法: bash rust/tests/mainline_snapshot.sh        # 写基线
#       bash rust/tests/mainline_snapshot.sh --check # 与基线比对
set -uo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
out="$root/rust/tmp/mainline-snapshot.txt"

snap() {
  local f
  for f in "$root"/*.xaml "$root"/*.cs "$root"/Theme/*.xaml "$root"/Dsh/*.cs "$root"/Assets/i18n/*.json; do
    [[ -f "$f" ]] || continue
    printf '%s\t%s\t%s\n' "$(sha256sum "$f" | cut -c1-16)" "$(wc -l <"$f")" "${f#"$root"/}"
  done
}

mkdir -p "$root/rust/tmp"
if [[ "${1:-}" == "--check" ]]; then
  if [[ ! -f "$out" ]]; then
    echo "NO-BASELINE 先不带 --check 跑一次"
    exit 2
  fi
  if diff "$out" <(snap) >/tmp/mainline-drift.diff 2>&1; then
    echo "MAINLINE-UNCHANGED"
  else
    echo "MAINLINE-DRIFTED:"
    grep -E '^[<>]' /tmp/mainline-drift.diff
  fi
else
  snap >"$out"
  echo "SNAPSHOT-WRITTEN $out ($(wc -l <"$out") files)"
fi
