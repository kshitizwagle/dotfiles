#!/usr/bin/env bash
# Super+Alt+V: cliphist history in Vicinae's UI (Aura-themed, searchable, quick look).
# cliphist stays the source of truth; Vicinae only renders the picker.
# Decode by ID: Vicinae hands the chosen row back with a space where cliphist had a tab,
# and `cliphist decode` on stdin would then try to parse the whole line as an ID and fail.
pkill -f 'vicinae dmenu' 2>/dev/null   # reap a picker left hanging by a previous run
sel=$(cliphist list | vicinae dmenu -n "Clipboard" -s "History · {count}" -p "Search clipboard…") || exit 0
[ -n "$sel" ] || exit 0
id=${sel%%[[:space:]]*}   # leading cliphist ID, tab- or space-separated
case $id in ""|*[!0-9]*) exit 0 ;; esac
cliphist decode "$id" | wl-copy
