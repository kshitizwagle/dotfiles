---
name: hyprland-config
description: Change the Hyprland setup (keybinds, rules, animations, monitors, autostart, Waybar/fuzzel/swaync/hyprlock) safely for Hyprland 0.53 on Ubuntu. Use for any edit under .config/hypr or the Hyprland stack.
---

# Edit Hyprland safely

**Version matters.** Ubuntu ships 0.53.x; check with `Hyprland --version`. The upstream wiki now documents a newer Lua config. For 0.53, the syntax reference is **`/usr/share/hypr/hyprland.conf`**: grep it before writing anything new.

## 0.53 syntax cheatsheet
```ini
windowrule {
    name = float-pavucontrol
    match:class = ^(pavucontrol)$
    float = yes
}
layerrule {
    name = blur-waybar
    match:namespace = waybar
    blur = true
}
bind = $mod ALT, V, exec, <cmd>          # binds unchanged
animation = windows, 1, 1.6, auraOut, popin 96%   # speed in deciseconds
```

## Rules
- **Shared vs per-machine:**
  - shared config: `.config/hypr/hyprland.conf`, `rules.conf`
  - per-machine (monitors, GPU env): `hosts/<hostname>/hypr-host.conf`, then rerun `./install.sh`
- **`env =` does not expand `~` or `$HOME`.** `AQ_DRM_DEVICES` is `:`-separated, so never put `/dev/dri/by-path/...` in it directly; use the `dgpu`/`igpu` symlinks.
- **Starting background services:**
  - start daemons via `systemctl --user start <unit>` in `exec-once`, never by running the binary directly (that causes duplicates)
  - any new unit needs a `.config/systemd/user/<unit>.service.d/hyprland-only.conf` drop-in so it doesn't run in GNOME
  - Hyprland-only apps go in `exec-once`, never in `~/.config/autostart`
- **Check keybind conflicts:** `grep -n 'bind.*<KEY>' .config/hypr/hyprland.conf`.
- **Follow the spec:** 1px borders, rounding 6, shadows off, window blur off (layer blur only for the bar and notifications), no bouncy curves, durations ≤ 6.

## Verify
```bash
Hyprland --verify-config -c ~/.config/hypr/hyprland.conf   # must print "config ok"
```
If the user is inside Hyprland: `hyprctl reload && hyprctl configerrors`.

If Hyprland crashes at login, read `journalctl -b --since "-10 min" | grep -iE 'hyprland|aquamarine'` and the newest `~/.cache/hyprland/hyprlandCrashReport*.txt`.
