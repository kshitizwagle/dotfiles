#!/usr/bin/env bash
# Install refind/refind.conf and the Aura theme (refind/themes/aura) to the EFI partition. Needs sudo.
# Backs up the current refind.conf and theme to ~/.dotfiles-backup/refind-<ts>/ first.
# Rollback: sudo cp -r ~/.dotfiles-backup/refind-<ts>/aura /boot/efi/EFI/refind/themes/
#           sudo cp ~/.dotfiles-backup/refind-<ts>/refind.conf /boot/efi/EFI/refind/
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")/.."
ESP=/boot/efi/EFI/refind
[ -d "$ESP" ] || { echo "rEFInd not found at $ESP"; exit 1; }
B=~/.dotfiles-backup/refind-$(date +%F-%H%M%S); mkdir -p "$B"
[ -d "$ESP/themes/aura" ] && cp -r "$ESP/themes/aura" "$B/"
cp "$ESP/refind.conf" "$B/"
sudo mkdir -p "$ESP/themes/aura"
sudo cp -r refind/themes/aura/. "$ESP/themes/aura/"
sudo cp refind/refind.conf "$ESP/refind.conf"   # stock sample + timeout 20, use_nvram false, include theme
ls "$ESP/themes/aura/icons"
echo "Installed. Backup: $B. Reboot to see it."
