#!/usr/bin/env bash
# Workspace navigation on the focused monitor:  workspace.sh new | prev | next
# An open special workspace (e.g. the minimized-windows overlay) sits on top of the regular one,
# so a switch would happen invisibly underneath it. Close the overlay first.
special=$(hyprctl monitors -j | jq -r '.[] | select(.focused) | .specialWorkspace.name // ""')
[ -n "$special" ] && hyprctl dispatch togglespecialworkspace "${special#special:}" >/dev/null
case "${1:-}" in
  new)  # lowest unused number, opened on this monitor. Empty workspaces vanish when you leave them
        # (Hyprland default); it was the minimized overlay, not emptiness, that made windows disappear.
        n=$(hyprctl workspaces -j | jq '[.[].id | select(. > 0)] as $used | first(range(1; 100) | select(. as $n | $used | index($n) | not))')
        hyprctl dispatch workspace "$n" >/dev/null ;;
  prev) hyprctl dispatch workspace m-1 >/dev/null ;;
  next) hyprctl dispatch workspace m+1 >/dev/null ;;
esac
