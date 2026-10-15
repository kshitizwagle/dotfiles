#!/usr/bin/env bash
# Green traffic light, macOS-style: the window gets its own workspace on this monitor, filled.
# Press again to return it to the workspace it came from (the now-empty workspace vanishes).
# The origin is remembered per window in $XDG_RUNTIME_DIR/hypr-fs-origin/<address>.
dir="${XDG_RUNTIME_DIR:-/tmp}/hypr-fs-origin"; mkdir -p "$dir"
# act on the window whose green button was clicked (not merely the focused one)
addr=$(~/.config/hypr/scripts/bar-window.sh); [ -n "$addr" ] || exit 0
hyprctl dispatch focuswindow "address:$addr" >/dev/null
win=$(hyprctl clients -j | jq -c --arg a "$addr" '.[] | select(.address == $a)')
if [ -f "$dir/$addr" ]; then
  origin=$(cat "$dir/$addr"); rm -f "$dir/$addr"
  [ "$(jq -r '.fullscreen' <<<"$win")" != 0 ] && hyprctl dispatch fullscreen 1 >/dev/null
  hyprctl dispatch movetoworkspace "$origin,address:$addr" >/dev/null
else
  jq -r '.workspace.id' <<<"$win" >"$dir/$addr"
  n=$(hyprctl workspaces -j | jq '[.[].id | select(. > 0)] as $used | first(range(1; 100) | select(. as $n | $used | index($n) | not))')
  hyprctl --batch "dispatch movetoworkspace $n,address:$addr ; dispatch fullscreen 1" >/dev/null
fi
