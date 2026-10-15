#!/usr/bin/env bash
# Open a mixer: pavucontrol if installed, else wiremix/alsamixer-ish fallback in terminal
if command -v pavucontrol >/dev/null; then exec pavucontrol
else exec ghostty -e wpctl status; fi
