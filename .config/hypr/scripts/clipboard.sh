#!/usr/bin/env bash
# Super+Alt+V: cliphist history in Vicinae's UI (Aura-themed, searchable, quick look).
# cliphist stays the source of truth; Vicinae only renders the picker.
sel=$(cliphist list | vicinae dmenu -n "Clipboard" -s "History · {count}" -p "Search clipboard…") || exit 0
[ -n "$sel" ] && printf '%s' "$sel" | cliphist decode | wl-copy
