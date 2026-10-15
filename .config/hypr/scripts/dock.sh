#!/usr/bin/env bash
# Waybar dock helper: pinned icons that act like macOS dock items.
#   dock.sh status <glyph> <class-regex>       -> JSON for a waybar custom module (adds class "running")
#   dock.sh open <class-regex> <launch cmd...>  -> focus/cycle the app's windows, or launch it if none
set -u
mode=$1; shift

case $mode in
  status)
    glyph=$1 re=$2
    # Waybar exports the output this bar is on; only count windows on that monitor
    mon=$(hyprctl monitors -j | jq --arg o "${WAYBAR_OUTPUT_NAME:-}" '[.[] | select(.name == $o) | .id][0] // -1')
    n=$(hyprctl clients -j | jq --arg re "$re" --argjson mon "$mon" \
      '[.[] | select((.class | test($re; "i")) and ($mon == -1 or .monitor == $mon))] | length')
    if [ "${n:-0}" -gt 0 ]; then
      printf '{"text":"%s","class":"running","tooltip":"%s window(s)"}\n' "$glyph" "$n"
    else
      printf '{"text":"%s","class":"idle"}\n' "$glyph"
    fi
    ;;
  open)
    re=$1; shift
    active=$(hyprctl activewindow -j | jq -r '.address // ""')
    # The dock clicked is on the focused monitor; only act on that monitor's windows
    mon=$(hyprctl monitors -j | jq '[.[] | select(.focused) | .id][0]')
    mapfile -t addrs < <(hyprctl clients -j | jq -r --arg re "$re" --argjson mon "$mon" \
      '[.[] | select((.class | test($re; "i")) and .monitor == $mon)] | sort_by(.focusHistoryID) | .[].address')
    if [ ${#addrs[@]} -eq 0 ]; then
      setsid -f "$@" >/dev/null 2>&1 </dev/null
      exit
    fi
    # Already focused on one of its windows -> cycle to the next, otherwise focus the most recent
    target=${addrs[0]}
    if [ "$target" = "$active" ] && [ ${#addrs[@]} -gt 1 ]; then target=${addrs[1]}; fi
    # a minimized window comes back onto the current workspace instead of opening the overlay
    if hyprctl clients -j | jq -e --arg a "$target" '.[] | select(.address == $a and .workspace.name == "special:minimized")' >/dev/null; then
      exec ~/.config/hypr/scripts/unminimize.sh "$target"
    fi
    hyprctl dispatch focuswindow "address:$target" >/dev/null
    ;;
esac
