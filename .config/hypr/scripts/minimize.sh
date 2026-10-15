#!/usr/bin/env bash
# Yellow traffic light: park the clicked window in the minimized stash.
addr=$(~/.config/hypr/scripts/bar-window.sh); [ -n "$addr" ] || exit 0
hyprctl dispatch movetoworkspacesilent "special:minimized,address:$addr" >/dev/null
