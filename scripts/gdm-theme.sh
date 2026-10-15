#!/usr/bin/env bash
# Build + install the Aura login screen: Ubuntu's Yaru GDM theme + gnome/gdm/aura-overrides.css.
# Installed as a separate higher-priority alternative; Ubuntu's files are not modified.
# Usage: scripts/gdm-theme.sh            build + install
#        scripts/gdm-theme.sh --uninstall
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")/.."
YARU=/usr/share/gnome-shell/theme/Yaru
DEST=/usr/share/gnome-shell/theme/Aura-gdm
LINK=/usr/share/gnome-shell/gdm-theme.gresource

if [ "${1:-}" = "--uninstall" ]; then
  sudo update-alternatives --remove gdm-theme.gresource "$DEST/gnome-shell-theme.gresource" || true
  sudo rm -rf "$DEST"; echo "Removed. Back to Yaru."; exit 0
fi

# Undo a WhiteSur-style overwrite of Yaru if its backup is present
if [ -f "$YARU/gnome-shell-theme.gresource.bak" ]; then
  sudo mv "$YARU/gnome-shell-theme.gresource" "$YARU/gnome-shell-theme.gresource.whitesur"
  sudo mv "$YARU/gnome-shell-theme.gresource.bak" "$YARU/gnome-shell-theme.gresource"
  echo "Restored original Yaru gresource"
fi
dpkg --verify yaru-theme-gnome-shell || { echo "Yaru gresource is modified; reinstall yaru-theme-gnome-shell first"; exit 1; }

SRC=$YARU/gnome-shell-theme.gresource
W=$(mktemp -d); trap 'rm -rf "$W"' EXIT
for r in $(gresource list "$SRC"); do mkdir -p "$W/res$(dirname "$r")"; gresource extract "$SRC" "$r" > "$W/res$r"; done
cat gnome/gdm/aura-overrides.css >> "$W/res/org/gnome/shell/theme/gdm.css"
( cd "$W/res"
  { echo '<?xml version="1.0" encoding="UTF-8"?><gresources><gresource prefix="/">'
    find . -type f | sed 's|^\./||' | sort | sed 's|.*|<file>&</file>|'
    echo '</gresource></gresources>'; } > "$W/x.xml" )
glib-compile-resources --sourcedir="$W/res" --target="$W/aura.gresource" "$W/x.xml"

sudo install -Dm644 "$W/aura.gresource" "$DEST/gnome-shell-theme.gresource"
sudo update-alternatives --install "$LINK" gdm-theme.gresource "$DEST/gnome-shell-theme.gresource" 20
sudo update-alternatives --set gdm-theme.gresource "$DEST/gnome-shell-theme.gresource"
echo "Installed. Log out to see it. If broken: Ctrl+Alt+F3 -> scripts/gdm-theme.sh --uninstall && sudo systemctl restart gdm3"
