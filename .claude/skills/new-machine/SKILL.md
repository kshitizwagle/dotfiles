---
name: new-machine
description: Bootstrap these dotfiles on a new or reinstalled Ubuntu machine, including creating its hosts/<hostname> entry. Use when the user sets up a new computer or reinstalls.
---

# Bootstrap a machine

1. **Audit read-only first** using the `desktop-auditor` agent: Ubuntu version, GNOME version, GPU(s), displays, and whether the packages are available in apt. If the Ubuntu release differs from 26.04, re-check every package in `scripts/packages.sh` with `apt-cache policy`.
2. **Create `hosts/$(hostname)/`:**
   - **GPUs:**
     ```bash
     ls -l /dev/dri/by-path/
     for c in /sys/class/drm/card*-*; do [ "$(cat $c/status)" = connected ] && echo "$(basename $c) $(readlink -f $c/device)"; done
     ```
     Write `gpus` with `dgpu=<by-path of the card driving the displays>` and, optionally, `igpu=...`.
   - **Monitors:** connector names, modes and rates from `gdbus call --session --dest org.gnome.Mutter.DisplayConfig --object-path /org/gnome/Mutter/DisplayConfig --method org.gnome.Mutter.DisplayConfig.GetCurrentState`. Keep all monitors at the same refresh rate on AMD if you see flicker.
   - **`hypr-host.conf`:** copy an existing host's file and edit it. Keep the `__HOME__` placeholder.
3. **Have the user run these in order** (they need sudo):
   1. `scripts/packages.sh`
   2. `./install.sh --dry-run`, then `./install.sh`
   3. `scripts/gnome.sh`
   4. `scripts/gdm-theme.sh`
4. **Verify** with the `config-verifier` agent. Then have the user log into Hyprland once (click their name at the login screen, the gear icon, then "Hyprland") and report back.
5. **Commit** `hosts/<hostname>/`.
