#!/usr/bin/env bash
# Apply GNOME settings from gnome/dconf/*.ini and the extension on/off list.
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")/.."
mkdir -p ~/.dotfiles-backup && dconf dump / > ~/.dotfiles-backup/dconf-$(date +%F-%H%M%S).ini
for f in gnome/dconf/*.ini; do dconf load / < "$f"; echo "loaded $f"; done
gsettings reset org.gnome.shell.extensions.user-theme name 2>/dev/null || true
# Spec rejects these: magic-lamp (bouncy), ding (desktop icons)
for e in compiz-alike-magic-lamp-effect@hermes83.github.com ding@rastersoft.com; do
  gnome-extensions disable "$e" 2>/dev/null || true
done
while read -r e; do gnome-extensions enable "$e" 2>/dev/null || echo "not installed: $e"; done < gnome/extensions-enabled.txt
echo "GNOME done. Rollback: dconf load / < ~/.dotfiles-backup/dconf-<ts>.ini"
