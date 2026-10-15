#!/usr/bin/env bash
# The top-bar stash pill's picker: bring back windows parked in special:minimized.
#   stash.sh       pick one from the list (Vicinae dmenu, newest first)
#   stash.sh all   bring back everything
set -u
unminimize=~/.config/hypr/scripts/unminimize.sh

if [ "${1:-}" = all ]; then
  mapfile -t addrs < <(hyprctl clients -j | jq -r '[.[] | select(.workspace.name == "special:minimized")] | sort_by(.focusHistoryID) | .[].address')
  [ ${#addrs[@]} -gt 0 ] || exit 0
  for a in "${addrs[@]}"; do "$unminimize" "$a"; done
  exit 0
fi

# One row per window: "3  Ghostty — OC | Greeting". Vicinae may hand the row back with the
# tab normalised away, so the number -> address map lives in a snapshot instead of a tab.
snapshot=$(mktemp) || exit 0
trap 'rm -f "$snapshot"' EXIT
rows=(); n=0
while IFS=$'\t' read -r addr class title; do
  n=$((n + 1))
  printf '%d\t%s\n' "$n" "$addr" >>"$snapshot"
  rows+=("$(printf '%d  %s — %s' "$n" "$class" "$title")")
done < <(hyprctl clients -j | jq -r '[.[] | select(.workspace.name == "special:minimized")] | sort_by(.focusHistoryID) | .[] | [.address, .class, .title] | @tsv')
[ ${#rows[@]} -gt 0 ] || exit 0

pkill -f 'vicinae dmenu' 2>/dev/null   # reap a picker left hanging by a previous run
sel=$(printf '%s\n' "${rows[@]}" | vicinae dmenu -n "Stash" -s "Minimized · {count}" -p "Bring back…") || exit 0
[ -n "$sel" ] || exit 0
idx=${sel%%[[:space:]]*}
case $idx in ""|*[!0-9]*) exit 0 ;; esac
addr=$(awk -v i="$idx" -F'\t' '$1 == i { print $2 }' "$snapshot")
[ -n "$addr" ] || exit 0
exec "$unminimize" "$addr"
