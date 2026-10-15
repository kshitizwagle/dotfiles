#!/usr/bin/env bash
# Yellow traffic light: park the clicked window in the minimized stash.
#   minimize.sh [address]  -> use the given window or find the one under the pointer
if [ -n "${1:-}" ]; then addr=$1; else addr=$(~/.config/hypr/scripts/bar-window.sh); fi
[ -n "$addr" ] || exit 0

client=$(hyprctl clients -j | jq -c --arg a "$addr" '.[] | select(.address == $a) | {workspace: .workspace.name}')
[ -n "$client" ] || exit 0
workspace=$(jq -r '.workspace // ""' <<<"$client")

# Moving a window that is already in this workspace is a no-op. Hide a visible minimized overlay
# either way, but never toggle it open when no monitor is showing it.
if [ "$workspace" != "special:minimized" ]; then
  hyprctl dispatch movetoworkspacesilent "special:minimized,address:$addr" >/dev/null
fi

monitors=$(hyprctl monitors -j)
overlay_monitor=$(jq -r '[.[] | select(.specialWorkspace.name == "special:minimized") | .id][0] // empty | tostring' <<<"$monitors")
[ -n "$overlay_monitor" ] || exit 0

focused_monitor=$(jq -r '.[] | select(.focused) | .id | tostring' <<<"$monitors")
if [ "$focused_monitor" != "$overlay_monitor" ]; then
  if [ -n "$focused_monitor" ]; then
    hyprctl --batch "dispatch focusmonitor $overlay_monitor ; dispatch togglespecialworkspace minimized ; dispatch focusmonitor $focused_monitor" >/dev/null
  else
    hyprctl --batch "dispatch focusmonitor $overlay_monitor ; dispatch togglespecialworkspace minimized" >/dev/null
  fi
else
  hyprctl dispatch togglespecialworkspace minimized >/dev/null
fi
