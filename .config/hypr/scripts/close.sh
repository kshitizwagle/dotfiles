#!/usr/bin/env bash
# Close a window — except apps that quit when their window closes (Spotify): those are parked in
# the "minimized" special workspace instead (Super+Shift+M or the dock brings them back).
#   close.sh        -> focused window (Super+Q)
#   close.sh bar    -> the window whose titlebar was clicked (red traffic light)
if [ "${1:-}" = bar ]; then addr=$(~/.config/hypr/scripts/bar-window.sh); else addr=$(hyprctl activewindow -j | jq -r '.address // ""'); fi
[ -n "$addr" ] || exit 0
class=$(hyprctl clients -j | jq -r --arg a "$addr" '.[] | select(.address == $a) | .class')
case "${class,,}" in
  spotify) exec ~/.config/hypr/scripts/minimize.sh "$addr" ;;
  *) hyprctl dispatch closewindow "address:$addr" ;;
esac
