#!/usr/bin/env bash
# start: hand the Hyprland env to systemd/dbus and bring up graphical-session.target.
# stop:  tear it all down and scrub the env, so the next GNOME login doesn't
#        see a "graphical session already running" and bounce to GDM.
VARS="WAYLAND_DISPLAY DISPLAY XDG_CURRENT_DESKTOP XDG_SESSION_DESKTOP XDG_SESSION_TYPE HYPRLAND_INSTANCE_SIGNATURE QT_QPA_PLATFORM QT_QPA_PLATFORMTHEME MOZ_ENABLE_WAYLAND XCURSOR_THEME XCURSOR_SIZE HYPRCURSOR_SIZE"
case "$1" in
  start)
    export XDG_SESSION_DESKTOP=Hyprland XDG_SESSION_TYPE=wayland
    systemctl --user import-environment $VARS
    dbus-update-activation-environment --systemd $VARS
    systemctl --user start hyprland-session.target
    ;;
  stop)
    systemctl --user stop hyprland-session.target graphical-session.target
    systemctl --user unset-environment $VARS
    systemctl --user reset-failed
    ;;
esac
