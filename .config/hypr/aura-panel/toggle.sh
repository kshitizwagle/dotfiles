#!/usr/bin/env bash
# Super+D: start the panel (already raised) if needed, otherwise raise/lower its controls sidebar.
#   toggle.sh         -> peek (raise/lower)
#   toggle.sh --quit  -> stop it entirely
if [ "${1:-}" = --quit ]; then pkill -x aura-panel; exit 0; fi
if pid=$(pgrep -x aura-panel); then
  kill -USR1 $pid
else
  setsid -f ~/.config/hypr/aura-panel/run.sh --raised >/tmp/aura-panel-$USER.log 2>&1 </dev/null
fi
