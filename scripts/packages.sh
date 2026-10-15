#!/usr/bin/env bash
# Install everything this setup needs (Ubuntu 26.04 archive). Needs sudo.
set -euo pipefail
mkdir -p ~/.dotfiles-backup && dpkg --get-selections > ~/.dotfiles-backup/pkgs-$(date +%F-%H%M%S).txt
sudo apt update
sudo apt install -y \
  fonts-ibm-plex starship neovim papirus-icon-theme \
  hyprland xdg-desktop-portal-hyprland hyprpolkitagent hyprlock hypridle hyprpaper \
  waybar fuzzel sway-notification-center grim slurp wl-clipboard cliphist jq curl \
  qt6ct nwg-look hyprland-qtutils \
  pavucontrol network-manager-gnome blueman

# BlexMono Nerd Font (IBM Plex Mono + icons) — not in apt
if ! fc-list : family | grep -qi 'BlexMono Nerd Font'; then
  tmp=$(mktemp -d)
  url=$(curl -fsSL https://api.github.com/repos/ryanoasis/nerd-fonts/releases/latest \
        | grep -o 'https://[^"]*/IBMPlexMono.tar.xz' | head -1)
  curl -fsSL "$url" -o "$tmp/f.tar.xz"
  mkdir -p ~/.local/share/fonts/BlexMono
  tar xJf "$tmp/f.tar.xz" -C ~/.local/share/fonts/BlexMono --wildcards '*.ttf'
  rm -rf "$tmp"; fc-cache -f
fi
echo "Packages done."
