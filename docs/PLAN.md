> Historical record of the original audit and rollout (2026-10-15). Later changes: both monitors at 165 Hz, Aura wallpaper dropped (back to the default Ubuntu wallpaper), Hyprland rules in 0.53 block syntax, GPU paths via host.conf. AGENTS.md has the current facts.

# Aura Dark Rice Plan — Ubuntu 26.04 / GNOME 50.1

Status as of 2026-10-15. Nothing below is applied except where marked DONE.

## Machine facts (from audit)

| Component | State |
|---|---|
| OS / kernel | Ubuntu 26.04.1 LTS (resolute), 7.0.0-38-generic, Wayland |
| GPU | RX 9070 (Navi 48, `card1`, 0000:03:00.0) drives both monitors; Raphael iGPU (`card0`) unused for display. amdgpu, Mesa 26.0.8 (Ubuntu archive) |
| Displays | MSI G244F E2 (DP-1) + HKC 24E3 (DP-2), 1920x1080, scale 1 |
| GNOME | Shell 50.1; WhiteSur-Light GTK + shell theme on `prefer-dark`; forced WhiteSur CSS in `~/.config/gtk-4.0`; accent `purple` |
| Extensions | blur-my-shell, just-perfection, magic-lamp, logomenu, vicinae, nepali-calendar, ding, ubuntu-dock, tiling-assistant, appindicators, snapd x2, web-search |
| Fonts | Inter (UI), Ubuntu Sans Mono; no IBM Plex, no Nerd Font |
| Terminal | Ghostty 1.3.0 (`aura-gnome` theme, Cascadia Code); default terminal is Ptyxis |
| Shell | zsh + oh-my-zsh `robbyrussell`; Homebrew present |
| Editor | vim, VS Code; no Neovim |
| Portals | xdg-desktop-portal 1.21.1, -gnome 50.0, -gtk 1.15.3; gnome-keyring 50.0 |
| Nix / dotfiles | none |
| Hyprland stack | All in Ubuntu archive: hyprland 0.53.3, waybar 0.15.0, hyprlock 0.9.2, hypridle 0.1.7, hyprpaper 0.8.3, xdph 1.3.11, hyprpolkitagent 0.1.3, swaync 0.12.4, fuzzel 1.12.0, grim, slurp, wl-clipboard, cliphist, qt6ct, nwg-look |

## Display fix — DONE

- Glitching above 60 Hz was caused by mismatched refresh rates (VRAM clock switching). Both monitors now at **1920x1080@165.001**, no glitching.
- Pending: revert the test setting (still `manual`). Run in a real terminal, or just reboot:
  ```bash
  echo auto | sudo tee /sys/class/drm/card1/device/power_dpm_force_performance_level
  ```
- If glitching returns on `auto`: add a udev rule forcing `high` (costs ~10–20 W idle).
- Confirm in the HKC's OSD that it really runs at 165 Hz.

## Gaps vs spec (most visible first)

1. Light WhiteSur shell/GTK3 theme on a dark scheme
2. Forced WhiteSur GTK4 CSS overrides libadwaita
3. Accent is `#9141ac` preset, not `#a277ff`
4. Magic Lamp — bouncy, violates motion spec
5. No IBM Plex Mono
6. Ghostty: full-accent selection, invisible cursor text
7. Default prompt, no Neovim, Ptyxis default terminal
8. Multicolor WhiteSur icons
9. Blur-my-shell may blur app windows (UNVERIFIED)

---

## Track A — GNOME

### A0 Backup (before every phase)
```bash
B=~/rice-backup/$(date +%F-%H%M); mkdir -p "$B"
dconf dump / > "$B/dconf.ini"
tar czf "$B/files.tgz" -C ~ .config/gtk-3.0 .config/gtk-4.0 .config/ghostty .config/monitors.xml .zshrc 2>/dev/null
```
Rollback: `dconf load / < "$B/dconf.ini" && tar xzf "$B/files.tgz" -C ~`, then log out/in.

### A2 GTK + shell: tint libadwaita, stop forcing a theme
```bash
mkdir -p ~/rice-backup/gtk4-whitesur
mv ~/.config/gtk-4.0/{gtk.css,gtk-dark.css,gtk-Dark.css,gtk-Light.css,assets,windows-assets} ~/rice-backup/gtk4-whitesur/
gsettings set org.gnome.desktop.interface gtk-theme 'Adwaita-dark'
gsettings reset org.gnome.shell.extensions.user-theme name
```
`~/.config/gtk-4.0/gtk.css` (libadwaita CSS variables only — colors, not a theme):
```css
:root {
  --accent-bg-color:#a277ff; --accent-color:#a277ff; --accent-fg-color:#15141b;
  --window-bg-color:#15141b; --window-fg-color:#edecee;
  --view-bg-color:#15141b;   --view-fg-color:#edecee;
  --headerbar-bg-color:#0f0e14; --headerbar-fg-color:#edecee;
  --sidebar-bg-color:#0f0e14; --secondary-sidebar-bg-color:#15141b;
  --card-bg-color:#1d1c25; --popover-bg-color:#25242f; --dialog-bg-color:#1d1c25;
  --border-color:#2e2c3a; --window-radius:6px;
}
```
`~/.config/gtk-3.0/gtk.css`:
```css
@define-color theme_bg_color #15141b;
@define-color theme_base_color #15141b;
@define-color theme_fg_color #edecee;
@define-color theme_text_color #edecee;
@define-color theme_selected_bg_color #a277ff;
@define-color theme_selected_fg_color #15141b;
@define-color borders #2e2c3a;
```
Accent stays `purple` (GNOME 50 has no custom-hex shell accent). A custom Aura shell theme is optional/EXPERIMENTAL.
Rollback: `rm ~/.config/gtk-{3,4}.0/gtk.css; mv ~/rice-backup/gtk4-whitesur/* ~/.config/gtk-4.0/; dconf load / < "$B/dconf.ini"`

### A3 Extensions + motion
```bash
gnome-extensions disable compiz-alike-magic-lamp-effect@hermes83.github.com
gnome-extensions disable ding@rastersoft.com
gsettings set org.gnome.shell.extensions.dash-to-dock dock-fixed false
gsettings set org.gnome.shell.extensions.dash-to-dock autohide true
gsettings set org.gnome.shell.extensions.dash-to-dock dash-max-icon-size 40
gsettings set org.gnome.shell.extensions.dash-to-dock transparency-mode 'FIXED'
gsettings set org.gnome.shell.extensions.dash-to-dock background-opacity 0.85
gsettings set org.gnome.shell.extensions.dash-to-dock custom-background-color true
gsettings set org.gnome.shell.extensions.dash-to-dock background-color '#0f0e14'
```
- Blur-my-shell prefs (GUI): Panel/Overview/Dash on, **Applications off**.
- Tiling: keep `tiling-assistant@ubuntu.com`. Tiling Shell skipped (GNOME 50 support UNVERIFIED).
- Window corners: no extension (libadwaita already rounds; Rounded Window Corners GNOME 50 UNVERIFIED).
- Mutter has no custom easing; keep stock animations.
Rollback: `gnome-extensions enable <uuid>`; `dconf load`.

### A4 Fonts, terminal, prompt, editor (shared with Hyprland)
```bash
sudo apt install fonts-ibm-plex starship neovim
gsettings set org.gnome.desktop.interface monospace-font-name 'IBM Plex Mono 11'
```
- Nerd glyphs: `IBMPlexMono` zip from https://github.com/ryanoasis/nerd-fonts/releases → `~/.local/share/fonts/BlexMono`, `fc-cache -f`.
- `~/.config/ghostty/config.ghostty`:
  ```
  theme = aura-gnome
  font-family = BlexMono Nerd Font Mono
  font-size = 11
  window-padding-x = 12
  window-padding-y = 10
  cursor-style = bar
  ```
  In the theme file: `selection-background = #3d375e`, `selection-foreground = #edecee`, `cursor-text = #15141b`.
- Default terminal: `ls /usr/share/applications | grep -i ghostty` → put that ID in `~/.config/xdg-terminals.list`.
- `.zshrc`: `ZSH_THEME=""`, append `eval "$(starship init zsh)"`.
- `~/.config/starship.toml`:
  ```toml
  format = "$directory$git_branch$git_status$character"
  [directory]
  style = "#a277ff"
  [git_branch]
  style = "#908f98"
  symbol = " "
  [git_status]
  style = "#ffca85"
  [character]
  success_symbol = "[❯](#a277ff)"
  error_symbol = "[❯](#ff6767)"
  ```
- Neovim: official port https://github.com/daltonmenezes/aura-theme/tree/main/packages/neovim → copy `colors/*` to `~/.config/nvim/colors/`, set colorscheme in `init.lua` (match the filename).
Rollback: restore `.zshrc`/ghostty from `$B`; `sudo apt remove starship neovim fonts-ibm-plex`.

### A5 Icons, wallpaper, lock screen
- Icons: Papirus-Dark (check `apt-cache policy papirus-icon-theme` first — UNVERIFIED).
- Cursor: keep WhiteSur-cursors.
- Wallpaper: https://github.com/daltonmenezes/aura-theme/tree/main/packages/wallpapers
  ```bash
  gsettings set org.gnome.desktop.background picture-uri-dark "file://$HOME/Pictures/aura.png"
  gsettings set org.gnome.desktop.screensaver picture-uri "file://$HOME/Pictures/aura.png"
  ```
- Lock screen / notifications: drawn by the shell; follow accent + shell theme.
Rollback: `dconf load`.

---

## Track B — Hyprland session

Risk: **low** (AMD + Mesa 26). Install from the Ubuntu archive; no PPA, no source build.

### B0 Backup
A0 plus `dpkg --get-selections > "$B/pkgs.txt"`.

### B1 Install
```bash
sudo apt install hyprland xdg-desktop-portal-hyprland hyprpolkitagent hyprlock hypridle hyprpaper \
  waybar fuzzel sway-notification-center grim slurp wl-clipboard cliphist qt6ct nwg-look hyprland-qtutils
```
Pick the session at GDM: click your user → gear icon (bottom right) → Hyprland / Ubuntu.

### B2 Config — `~/.config/hypr/hyprland.conf`
```ini
monitor = DP-1, 1920x1080@165, 0x0, 1
monitor = DP-2, 1920x1080@165, 1920x0, 1
env = AQ_DRM_DEVICES,/dev/dri/by-path/pci-0000:03:00.0-card
env = XCURSOR_THEME,WhiteSur-cursors
env = XCURSOR_SIZE,24
env = QT_QPA_PLATFORMTHEME,qt6ct

general {
  border_size = 1
  gaps_in = 4
  gaps_out = 8
  col.active_border = rgb(a277ff)
  col.inactive_border = rgb(2e2c3a)
}
decoration {
  rounding = 6
  blur { enabled = true; size = 6; passes = 2 }
  shadow { enabled = false }
}
animations {
  bezier = auraOut, 0.23, 1, 0.32, 1
  bezier = auraInOut, 0.77, 0, 0.175, 1
  animation = windows, 1, 1.6, auraOut, popin 96%
  animation = fade, 1, 1.6, auraOut
  animation = workspaces, 1, 4.2, auraInOut, slide
  animation = layers, 1, 1.6, auraOut, fade
}

# Blur only bar + notifications. 0.53 rule syntax UNVERIFIED — check with: hyprctl configerrors
layerrule = match:namespace waybar, blur on
layerrule = match:namespace swaync-control-center, blur on

exec-once = systemctl --user start hyprpolkitagent
exec-once = waybar
exec-once = swaync
exec-once = hyprpaper
exec-once = hypridle
exec-once = wl-paste --watch cliphist store

bind = SUPER, Return, exec, ghostty
bind = SUPER, Space, exec, fuzzel
bind = , Print, exec, grim -g "$(slurp)" - | wl-copy
```
- Waybar: start from official https://github.com/daltonmenezes/aura-theme/tree/main/packages/waybar; font BlexMono Nerd Font, bg `rgba(15,14,20,0.85)`, `border-bottom: 1px solid #2e2c3a`, accent only on active workspace.
- fuzzel.ini: `[colors] background=15141bf2 text=edeceeff match=a277ffff selection=3d375eff selection-text=edeceeff border=2e2c3aff`; `[border] radius=6 width=1`; `font=IBM Plex Mono:size=11`.
- swaync: bg `#1d1c25`, `1px solid #2e2c3a`, radius 6px, critical `#ff6767`.
- hyprlock: bg `#0f0e14` (no blur), input outer `rgb(a277ff)`, inner `rgb(1d1c25)`, font IBM Plex Mono.
- hypridle: lock 10 min, DPMS off 15 min.

### Coexistence
| Shared | Isolation |
|---|---|
| Portals | `~/.config/xdg-desktop-portal/hyprland-portals.conf`: `[preferred] default=hyprland;gtk`, `org.freedesktop.impl.portal.Secret=gnome-keyring` (only read when desktop = Hyprland) |
| Env vars | Keep `~/.config/environment.d` empty; use `env =` in hyprland.conf |
| Autostart | Hyprland-only apps via `exec-once`, never `~/.config/autostart` |
| Keyring | GDM PAM unlocks for both; fallback `exec-once = gnome-keyring-daemon --start --components=secrets` |
| Polkit | hyprpolkitagent only via `exec-once`; GNOME uses built-in |
| GTK | Shared dconf + gtk.css (intended). Don't let nwg-look write `settings.ini` |
| Cursor / Qt | `XCURSOR_*`, `QT_QPA_PLATFORMTHEME` only in Hyprland |
| Shared on purpose | Ghostty, zsh, starship, nvim, fonts, gtk.css |

---

## Token mapping

| Token | GTK | Ghostty | Waybar | Hyprland | Neovim | swaync | hyprlock | fuzzel / starship |
|---|---|---|---|---|---|---|---|---|
| bg `#15141b` | window/view bg | background | module bg | — | official port | — | — | fuzzel bg |
| bg-deep `#0f0e14` | headerbar, sidebar | palette 0 ≈ `#110f18` | bar bg 85% | — | — | — | background | — |
| surface `#1d1c25` | card, dialog | — | — | — | float bg | notif bg | input inner | — |
| surface-raised `#25242f` | popover | — | tooltip | — | pmenu | control center | — | — |
| border `#2e2c3a` | border-color | — | hairline | inactive border | separator | 1px border | — | fuzzel border |
| fg `#edecee` | fg | foreground | text | — | Normal fg | text | text | text |
| metadata `#908f98` | — | — | inactive ws | — | comments | timestamp | — | git_branch |
| accent `#a277ff` | accent | cursor, palette 4/5 | active ws | active border | keywords | — | input outer | match, dir, ❯ |
| accent-tint `#3d375e` | — | selection | hover | — | Visual | — | — | fuzzel selection |
| green `#61ffca` | — | palette 2/6 | ok | — | strings | — | check | — |
| warning `#ffca85` | — | palette 3 | warn | — | numbers | — | — | git_status |
| danger `#ff6767` | — | palette 1 | critical | — | errors | critical | fail | error ❯ |
| blue `#82e2ff`, pink `#f694ff` | — | — | — | — | official port only | — | — | unused |

## Inspiration (verified 2026-10-15 via GitHub API)

1. https://github.com/daltonmenezes/aura-theme — 3,825 stars, pushed 2025-12-20. Official neovim/waybar/vscode/wallpapers ports; sway port as Hyprland color reference.
2. https://github.com/end-4/dots-hyprland — 16,363 stars, pushed 2026-10-11. hyprlock layout, clipboard/screenshot binds only.
3. https://github.com/HyDE-Project/HyDE — 9,663 stars, pushed 2026-10-11. One-palette-feeds-all theming structure. Don't run its Arch installer.
4. https://github.com/mylinuxforwork/dotfiles — 5,073 stars, pushed 2026-10-12. Split `hypr/conf/*.conf` layout, hypridle defaults.
5. https://github.com/omacom/omarchy — 44,864 stars, pushed 2026-10-15. Restrained 1px borders, small gaps, minimal launcher. Arch-only scripts.

## Track C — Nix + Home Manager: not worth it now

Everything needed is in the Ubuntu archive with security updates. Nix-built Hyprland on Ubuntu needs nixGL wrappers for Mesa — extra failure points on a dual-GPU box — and adds a third package manager alongside apt and Homebrew. The real need is versioning ~a dozen config files: a git repo + GNU Stow or chezmoi does that. Revisit if moving to NixOS or syncing multiple machines.

## Risks and rollback

| Phase | Risk | Rollback |
|---|---|---|
| A2 | Some apps look off | Remove gtk.css files, restore WhiteSur CSS, `dconf load` |
| A3 | Lose desktop icons/effect | `gnome-extensions enable <uuid>` |
| A4 | Starship slow in huge repos | Restore `.zshrc`; `apt remove` |
| A5 | Cosmetic only | `dconf load` |
| B1 | Pulls Hyprland deps | `sudo apt remove hyprland xdg-desktop-portal-hyprland … && sudo apt autoremove`; delete `hyprland-portals.conf` |
| B2 | Black screen in Hyprland | Pick Ubuntu at GDM or Ctrl+Alt+F3, `mv ~/.config/hypr ~/.config/hypr.bad` |
| General | repo.radeon.com could override Mesa on upgrade | Check `apt-cache policy libgl1-mesa-dri` before `full-upgrade` |

## Next step

A0 (backup), then A2 — awaiting approval.
