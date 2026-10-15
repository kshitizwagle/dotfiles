# AGENTS.md

Guide for AI agents (Claude Code, Codex, etc.) working in this dotfiles repo.

## What this repo is

Personal dotfiles for Ubuntu 26.04 + GNOME 50 (daily driver) with an optional Hyprland 0.53 session, themed **Aura Dark**. The repo root mirrors `$HOME`: `./.config/ghostty/config.ghostty` is linked to `~/.config/ghostty/config.ghostty`.

`.zshenv` is macOS-only and `install.sh` never links it. `.ssh/config` is linked but gitignored (it names private hosts); the tracked `.ssh/config.example` is a sanitized copy that `install.sh` seeds it from. It holds host aliases and key **paths** only, and keys themselves must never be committed (see `.gitignore`).

## Layout

| Path | Purpose |
|---|---|
| `.zshrc`, `.config/**`, `Pictures/**` | Linked into `$HOME` by `install.sh` (every file under `.config` and `Pictures` is picked up automatically). Use `~` or relative paths, never `/home/<user>` |
| `hosts/<hostname>/` | Machine-specific: `hypr-host.conf` (monitors, GPU env), `gpus` (DRM device symlinks) |
| `gnome/dconf/*.ini` | GNOME settings subset, applied by `scripts/gnome.sh` |
| `gnome/extensions-enabled.txt` | Enabled GNOME Shell extensions |
| `gnome/gdm/aura-overrides.css` | Login-screen CSS appended to Ubuntu's Yaru GDM theme by `scripts/gdm-theme.sh` |
| `scripts/packages.sh` | apt packages + BlexMono Nerd Font |
| `refind/refind.conf`, `refind/themes/aura/` | rEFInd config and boot theme; `scripts/refind-theme.sh` installs both to the ESP (sudo) |
| `docs/aura-tokens.md` | The design spec. **Source of truth for every color, radius and timing** |
| `docs/PLAN.md` | Original audit + rollout plan, with the reasoning behind decisions |
| `.claude/skills/`, `.claude/agents/` | Reusable procedures for future setup work |

## Rules

1. **Read-only first.** Audit with commands before changing anything. Never assume versions: check `gnome-shell --version`, `Hyprland --version`, `apt-cache policy <pkg>`.
2. **Follow the spec** in `docs/aura-tokens.md`: one saturated accent (`#a277ff`), purple-tinted neutrals, 1px borders before shadows, 6px radius, no window blur (blur only on the bar, panel and notifications), short non-bouncy motion. If a popular rice choice conflicts with the spec, follow the spec and say what you rejected.
3. **Prefer official Aura ports** (github.com/daltonmenezes/aura-theme `packages/`) over hand-mapping.
4. **Every change must be reversible.** Back up before overwriting (`dconf dump`, copy to `~/.dotfiles-backup/`), and give a rollback command.
5. **No secrets.** Never commit or print keys, tokens, `.env`, browser profiles or keyrings. Scan with `git diff --cached | grep -iE 'token|secret|passw|api.?key'` before committing.
6. **Edit the repo, not `$HOME`.** Files in `$HOME` are symlinks into this repo, so edit here; changes are live immediately.
7. **Machine-specific values go in `hosts/<hostname>/`**, never in shared config (monitors, connector names, PCI paths).
8. **sudo is interactive.** Agents can't enter the password, so put root steps in a script under `scripts/` and ask the user to run it.
9. **Verify before you say it's done** (see Verification). Report failures honestly.

## Hard-won facts (don't relearn these)

- **Hyprland 0.53 (Ubuntu apt) uses block-style rules:** `layerrule { name = x; match:namespace = waybar; blur = true }`. The upstream wiki now documents a newer Lua config, which does **not** apply to 0.53. The reference is `/usr/share/hypr/hyprland.conf`.
- **`AQ_DRM_DEVICES` is `:`-separated**, and `/dev/dri/by-path/pci-0000:03:00.0-card` contains colons. Use the colon-free symlinks (`~/.config/hypr/dgpu`, `igpu`) that `install.sh` creates from `hosts/<host>/gpus`. Getting this wrong makes Hyprland crash at login (`CBackend::create() failed!`).
- **Hyprland `env =` does not expand `~` or `$HOME`.** `host.conf` is generated with absolute paths (`__HOME__` placeholder).
- **The Hyprland packages enable systemd user services** (waybar, swaync, hyprpaper, hypridle, hyprpolkitagent) on `graphical-session.target`, which also fires in GNOME. Each one has a drop-in `ConditionEnvironment=XDG_CURRENT_DESKTOP=Hyprland` under `.config/systemd/user/*.service.d/`. Hyprland starts them with `systemctl --user start` after `dbus-update-activation-environment`.
- **Mixed refresh rates on the RX 9070 cause flicker** (VRAM clock switching). Keep both monitors at the same rate (165 Hz).
- **libadwaita:** recolor with CSS variables in `~/.config/gtk-4.0/gtk.css` (`--accent-bg-color`, `--window-bg-color`, ...). Never force a full GTK4 theme.
- **GNOME 50 shell accent** is preset-only (`purple`), so there's no custom hex.
- **WhiteSur's installer overwrites** `/usr/share/gnome-shell/theme/Yaru/gnome-shell-theme.gresource`, keeping `.bak`. `scripts/gdm-theme.sh` restores it and installs Aura as a separate `update-alternatives` entry.
- **rEFInd theme** follows the "Boot Menu – Aura Dark" design canvas. rEFInd only draws the background, the icons, the selection tile behind the highlighted item, and its label (hidden here with `hideui ... label`), so there are no tiles behind unselected items. Tile = 9/8 × `big_icon_size` and 4/3 × `small_icon_size`: OS icons 178px (112px plain-white glyph) → 200px tile, tool icons 48px (26px glyph, `#bdbdbd`, 1.75 stroke on a 24 grid) → 64px tile. Any tool without an icon in `refind/themes/aura/icons/` falls back to rEFInd's stock **colored** icon, so add a matching one. `showtools` must list all 12 slots, or 0.14.2 leaves defaults behind and duplicates tools. Assets were rendered with headless Chrome from HTML, so re-render the same way rather than hand-drawing.
- **Ghostty:** keep `Ctrl+W`, `Ctrl+D`, `Ctrl+K` and `Ctrl+[` free; they are shell keys. Fullscreen is F11.

## Verification

```bash
ghostty +validate-config
Hyprland --verify-config -c ~/.config/hypr/hyprland.conf      # expect "config ok"
python3 -c "import json,re;json.loads(re.sub(r'//.*','',open('.config/waybar/config.jsonc').read()))"
nvim --headless +'lua print(vim.api.nvim_get_hl(0,{name="Normal"}).bg)' +qa   # 1381403 = #15141b
bash -n install.sh scripts/*.sh
./install.sh --dry-run
```

## Commits

Small, focused commits with imperative subjects (`hypr: blur swaync layers`). Push only when the user asks.
