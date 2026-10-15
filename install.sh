#!/usr/bin/env bash
# Link dotfiles into $HOME (Linux). Existing files are moved to ~/.dotfiles-backup/<timestamp>/.
# Usage: ./install.sh [--dry-run]
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"
REPO=$PWD
DRY=${1:-}
TS=$(date +%F-%H%M%S)
BACKUP="$HOME/.dotfiles-backup/$TS"
HOST=$(hostname)

run() { if [ "$DRY" = "--dry-run" ]; then echo "[dry] $*"; else "$@"; fi; }

link() {  # link <repo-relative path>
  local src="$REPO/$1" dst="$HOME/$1"
  if [ -L "$dst" ] && [ "$(readlink -f "$dst")" = "$src" ]; then return; fi
  if [ -e "$dst" ] || [ -L "$dst" ]; then
    run mkdir -p "$BACKUP/$(dirname "$1")"
    run mv "$dst" "$BACKUP/$1"
    echo "backed up ~/$1"
  fi
  run mkdir -p "$(dirname "$dst")"
  run ln -s "$src" "$dst"
  echo "linked   ~/$1"
}

# Every managed file: .zshrc, .ssh/config, everything under .config and Pictures. (.zshenv is macOS-only.)
link .zshrc
run mkdir -p "$HOME/.ssh"; run chmod 700 "$HOME/.ssh"
# .ssh/config is gitignored (it names private hosts); seed it from the example on a fresh clone
[ -f "$REPO/.ssh/config" ] || run cp "$REPO/.ssh/config.example" "$REPO/.ssh/config"
run chmod 600 "$REPO/.ssh/config"   # ssh refuses group-writable configs; git doesn't keep modes
link .ssh/config
while IFS= read -r f; do link "${f#./}"; done < <(find ./.config -type f | sort)
while IFS= read -r f; do link "${f#./}"; done < <(find ./Pictures -type f | sort)   # wallpapers used by hyprpaper/hyprlock

# Per-host Hyprland settings (monitors, GPU order)
HOSTDIR="$REPO/hosts/$HOST"
if [ -d "$HOSTDIR" ]; then
  run sh -c "sed 's|__HOME__|$HOME|g' '$HOSTDIR/hypr-host.conf' > '$HOME/.config/hypr/host.conf'"
  if [ -f "$HOSTDIR/gpus" ]; then
    while IFS='=' read -r name dev; do
      [ -n "$name" ] && run ln -sfn "$dev" "$HOME/.config/hypr/$name"
    done < "$HOSTDIR/gpus"
  fi
  echo "host     $HOST: wrote ~/.config/hypr/host.conf"
else
  run sh -c "printf 'monitor = , preferred, auto, 1\n' > '$HOME/.config/hypr/host.conf'"
  echo "host     $HOST: no hosts/$HOST, wrote generic host.conf (add one, see AGENTS.md)"
fi

run systemctl --user daemon-reload || true
command -v ghostty >/dev/null && ghostty +validate-config >/dev/null && echo "ghostty  config ok"
command -v Hyprland >/dev/null && Hyprland --verify-config -c "$HOME/.config/hypr/hyprland.conf" 2>/dev/null | tail -1 | sed 's/^/hyprland /'
[ -d "$BACKUP" ] && echo "Backups: $BACKUP"
echo "Done. Optional: scripts/packages.sh, scripts/gnome.sh, scripts/gdm-theme.sh"
