#!/usr/bin/env bash
# Install everything this setup needs. Needs sudo (except --nix).
# Picks the package manager from /etc/os-release: apt (Debian/Ubuntu), pacman (Arch/CachyOS),
# nix profile (NixOS). Pass --nix to use a nix profile on any distro.
# Usage: scripts/packages.sh [--nix]
set -euo pipefail
. /etc/os-release
TS=$(date +%F-%H%M%S)
mkdir -p ~/.dotfiles-backup

if [ "${1:-}" = "--nix" ] || [ "$ID" = nixos ]; then
  DISTRO=nix
elif [[ " $ID ${ID_LIKE:-} " == *" arch "* ]]; then
  DISTRO=arch
elif [[ " $ID ${ID_LIKE:-} " =~ " "(debian|ubuntu)" " ]]; then
  DISTRO=debian
else
  echo "unsupported distro: $ID (pass --nix to use a nix profile)"; exit 1
fi
echo "packages for: $DISTRO"

case $DISTRO in
  debian)
    dpkg --get-selections > ~/.dotfiles-backup/pkgs-$TS.txt
    sudo apt update
    sudo apt install -y \
      fonts-ibm-plex starship neovim papirus-icon-theme \
      hyprland xdg-desktop-portal-hyprland hyprpolkitagent hyprlock hypridle hyprpaper \
      waybar fuzzel sway-notification-center grim slurp wl-clipboard cliphist jq curl \
      qt6ct nwg-look hyprland-qtutils \
      pavucontrol network-manager-gnome blueman
    ;;
  arch)
    pacman -Qqe > ~/.dotfiles-backup/pkgs-$TS.txt
    # ttf-ibmplex-mono-nerd is BlexMono Nerd Font; hyprland-guiutils replaced hyprland-qtutils
    sudo pacman -Syu --needed \
      ttf-ibm-plex ttf-ibmplex-mono-nerd starship neovim papirus-icon-theme \
      hyprland xdg-desktop-portal-hyprland hyprpolkitagent hyprlock hypridle hyprpaper \
      waybar fuzzel swaync grim slurp wl-clipboard cliphist jq curl \
      qt6ct nwg-look hyprland-guiutils \
      pavucontrol network-manager-applet blueman
    ;;
  nix)
    nix() { command nix --extra-experimental-features 'nix-command flakes' "$@"; }
    nix profile list > ~/.dotfiles-backup/pkgs-$TS.txt 2>/dev/null || true
    pkgs=(
      ibm-plex starship neovim papirus-icon-theme
      waybar fuzzel swaynotificationcenter grim slurp wl-clipboard cliphist jq curl
      qt6Packages.qt6ct nwg-look pavucontrol networkmanagerapplet blueman
    )
    # On NixOS the session, portal and polkit agent come from programs.hyprland, not a profile
    [ "$ID" = nixos ] || pkgs+=(hyprland xdg-desktop-portal-hyprland hyprland-qtutils)
    pkgs+=(hyprpolkitagent hyprlock hypridle hyprpaper)
    nix profile install "${pkgs[@]/#/nixpkgs#}"   # `install` still works where `add` is newer
    if [ "$ID" = nixos ]; then
      echo "NixOS: enable the session in configuration.nix, then nixos-rebuild switch:"
      echo "  programs.hyprland.enable = true;"
      echo "  fonts.packages = [ pkgs.ibm-plex pkgs.nerd-fonts.blex-mono ];"
    fi
    ;;
esac

# BlexMono Nerd Font (IBM Plex Mono + icons): from GitHub when no package provided it
if ! fc-list : family | grep -qi 'BlexMono Nerd Font'; then
  tmp=$(mktemp -d)
  url=$(curl -fsSL https://api.github.com/repos/ryanoasis/nerd-fonts/releases/latest \
        | grep -o 'https://[^"]*/IBMPlexMono.tar.xz' | head -1)
  curl -fsSL "$url" -o "$tmp/f.tar.xz"
  mkdir -p ~/.local/share/fonts/BlexMono
  tar xJf "$tmp/f.tar.xz" -C ~/.local/share/fonts/BlexMono --wildcards '*.ttf'
  rm -rf "$tmp"; fc-cache -f
fi
echo "Packages done. Previous package list: ~/.dotfiles-backup/pkgs-$TS.txt"
