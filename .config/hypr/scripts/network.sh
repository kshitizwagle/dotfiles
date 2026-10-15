#!/usr/bin/env bash
# Wi-Fi picker: nmtui in a floating terminal (nm-applet in the tray is the GUI option)
exec ghostty --class=aura.float -e nmtui
