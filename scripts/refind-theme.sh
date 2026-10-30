#!/usr/bin/env bash
# Install refind/refind.conf and the Aura theme (refind/themes/aura) to the EFI partition. Needs sudo.
# Backs up the current refind.conf and theme to ~/.dotfiles-backup/refind-<ts>/ first.
# Rollback: sudo cp -r ~/.dotfiles-backup/refind-<ts>/aura <ESP>/EFI/refind/themes/
#           sudo cp ~/.dotfiles-backup/refind-<ts>/refind.conf <ESP>/EFI/refind/
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")/.."
# ESP is /boot on CachyOS/Arch, /boot/efi on Ubuntu. Both are root-only, so test with sudo.
ESP=
for d in /boot/EFI/refind /boot/efi/EFI/refind /efi/EFI/refind; do
  sudo test -d "$d" && { ESP=$d; break; }
done
[ -n "$ESP" ] || { echo "rEFInd not found under /boot, /boot/efi or /efi"; exit 1; }
B=~/.dotfiles-backup/refind-$(date +%F-%H%M%S); mkdir -p "$B"
if sudo test -d "$ESP/themes/aura"; then sudo cp -r "$ESP/themes/aura" "$B/"; fi
sudo cp "$ESP/refind.conf" "$B/"
sudo chown -R "$USER:" "$B"
sudo mkdir -p "$ESP/themes/aura"
sudo rm -rf "$ESP/themes/aura/icons"   # drop stale icons (e.g. os_ubuntu.png)
sudo cp -r refind/themes/aura/. "$ESP/themes/aura/"
sudo cp refind/refind.conf "$ESP/refind.conf"   # stock sample + timeout 20, use_nvram false, include theme
sudo ls "$ESP/themes/aura/icons"
echo "Installed. Backup: $B. Reboot to see it."
