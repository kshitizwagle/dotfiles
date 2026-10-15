#!/usr/bin/env bash
# Super+Shift+M: bring back the most recently minimized window onto the current workspace.
# (Showing the "minimized" special workspace as an overlay was a trap: new windows opened inside
# it and vanished with it on the next workspace switch.)
#   unminimize.sh            -> most recent
#   unminimize.sh <address>  -> that window (used by the dock)
mon=$(hyprctl monitors -j | jq -c '.[] | select(.focused)')
ws=$(jq -r '.activeWorkspace.id' <<<"$mon")
special=$(jq -r '.specialWorkspace.name // ""' <<<"$mon")
[ -n "$special" ] && hyprctl dispatch togglespecialworkspace "${special#special:}" >/dev/null
addr=${1:-$(hyprctl clients -j | jq -r '[.[] | select(.workspace.name == "special:minimized")] | sort_by(.focusHistoryID) | .[0].address // ""')}
[ -n "$addr" ] || exit 0
hyprctl --batch "dispatch movetoworkspacesilent $ws,address:$addr ; dispatch focuswindow address:$addr" >/dev/null
