#!/usr/bin/env bash
# Ctrl+Shift+M: hide every window on the focused monitor's workspace, or bring them back.
# Windows are parked in a per-workspace special workspace ("stash<id>"), so each screen toggles
# independently and windows return to the same workspace.
mon=$(hyprctl monitors -j | jq -c '.[] | select(.focused)')
ws=$(jq -r '.activeWorkspace.id' <<<"$mon")
special=$(jq -r '.specialWorkspace.name // ""' <<<"$mon")  # e.g. special:minimized shown on top
stash="special:stash$ws"
clients=$(hyprctl clients -j)
mapfile -t hidden < <(jq -r --arg s "$stash" '.[] | select(.workspace.name == $s) | .address' <<<"$clients")
cmds=()
if ((${#hidden[@]})); then
  for a in "${hidden[@]}"; do cmds+=("dispatch movetoworkspacesilent $ws,address:$a"); done
else
  # an open overlay (minimized windows, scratchpad…) counts as "everything on this screen" too:
  # close it rather than stash its windows, so it keeps its own contents
  [ -n "$special" ] && cmds+=("dispatch togglespecialworkspace ${special#special:}")
  mapfile -t shown < <(jq -r --argjson w "$ws" '.[] | select(.workspace.id == $w) | .address' <<<"$clients")
  for a in "${shown[@]}"; do cmds+=("dispatch movetoworkspacesilent $stash,address:$a"); done
fi
# hyprctl --batch needs " ; " (with spaces) between commands
((${#cmds[@]})) || exit 0
batch=$(printf '%s ; ' "${cmds[@]}")
[ -n "${DRY_RUN:-}" ] && { echo "$batch"; exit 0; }
hyprctl --batch "${batch% ; }" >/dev/null
