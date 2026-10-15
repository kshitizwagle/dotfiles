#!/usr/bin/env bash
# Power menu: wlogout (Penrose-sky style). Old fuzzel version: ~/.config/themes/penrose-sky/powermenu-fuzzel.sh.bak
pkill -x wlogout && exit 0
exec wlogout --buttons-per-row 2 --column-spacing 0 --row-spacing 0 \
  --margin-top 365 --margin-bottom 365 --margin-left 785 --margin-right 785 --protocol layer-shell
