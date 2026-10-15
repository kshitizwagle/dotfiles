#!/usr/bin/env bash
# Print the address of the window whose titlebar is under the pointer.
# hyprbars buttons run a plain command with no window argument, and the focused window is not
# always the one you clicked (an XWayland app like Spotify may not take focus from hovering its
# bar), so the traffic lights must find their own window instead of acting on "activewindow".
BAR=30  # hyprbars bar_height (28) + a little slack
read -r cx cy < <(hyprctl cursorpos | tr -d ',')
visible=$(hyprctl monitors -j | jq -r '[.[] | .activeWorkspace.id, (.specialWorkspace.id // empty)] | map(tostring) | join(" ")')
hyprctl clients -j | jq -r --argjson x "$cx" --argjson y "$cy" --arg vis " $visible " '
  [ .[] | . as $c | select(($vis | contains(" \($c.workspace.id) ")) and $c.mapped and ($c.hidden | not))
        | select($x >= .at[0] and $x <= .at[0] + .size[0] and $y >= .at[1] - '"$BAR"' and $y <= .at[1] + .size[1]) ]
  | sort_by(((.workspace.name // "") | startswith("special:") | not), (.floating | not), .focusHistoryID)
  | .[0].address // ""'
