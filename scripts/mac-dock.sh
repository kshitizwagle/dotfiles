#!/usr/bin/env bash
# macOS-style dock for GNOME: Dash2Dock Animated (hover magnification) + MacTahoe icons.
# No sudo: both install under ~/.local/share. On Wayland a newly installed extension
# only loads after logging out and back in.
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")/.."
uuid=dash2dock-lite@icedman.github.com
ext=~/.local/share/gnome-shell/extensions/$uuid
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT

# Upstream main plus two open PRs: #385 (gnome-shell abort when dock items are removed)
# and #386 (macOS-shaped magnification falloff). Upstream doesn't declare GNOME 51 yet (#377).
git clone -q https://github.com/icedman/dash2dock-lite "$tmp/d2d"
for pr in 385 386; do
  curl -fsSL "https://github.com/icedman/dash2dock-lite/pull/$pr.diff" | git -C "$tmp/d2d" apply 2>/dev/null \
    || echo "PR #$pr doesn't apply (merged upstream?), skipping"
done
jq '."shell-version" |= (. + ["51"] | unique)' "$tmp/d2d/metadata.json" > "$tmp/m" && mv "$tmp/m" "$tmp/d2d/metadata.json"
glib-compile-schemas --strict --targetdir="$tmp/d2d/schemas" "$tmp/d2d/schemas"
rm -rf "$ext" && mkdir -p "$ext"
cp -R "$tmp/d2d"/* "$ext"/
rm -rf "$ext"/{build,Makefile,tests,tools,screenshots,node_modules,agents,eslint.config.js,package*.json}
echo "installed $uuid"

git clone -q --depth 1 https://github.com/vinceliuice/MacTahoe-icon-theme "$tmp/icons"
(cd "$tmp/icons" && ./install.sh -t purple -d ~/.local/share/icons >/dev/null)
echo "installed MacTahoe-purple icons"

mkdir -p ~/.dotfiles-backup && dconf dump / > ~/.dotfiles-backup/dconf-$(date +%F-%H%M%S).ini
dconf load / < gnome/dconf/dash2dock-lite.ini
dconf load / < gnome/dconf/interface.ini
gnome-extensions disable dash-to-dock@micxgx.gmail.com 2>/dev/null || true
gnome-extensions enable "$uuid" 2>/dev/null || echo "log out and back in, then: gnome-extensions enable $uuid"
echo "Rollback: gnome-extensions disable $uuid; gnome-extensions enable dash-to-dock@micxgx.gmail.com; dconf load / < ~/.dotfiles-backup/dconf-<ts>.ini"
