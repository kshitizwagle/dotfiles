---
name: add-dotfile
description: Bring a config file or app under dotfiles management. Use when the user installs or configures a new tool and wants it tracked, or says "add X to my dotfiles".
---

# Add a config to the dotfiles

1. **Locate the live file(s).** They are usually `~/.config/<app>/...`. Skip caches, state, history and anything machine-generated.
2. **Check for secrets:**
   ```bash
   grep -nEi 'token|secret|passw|api.?key|bearer|cookie|private' <file> | sed -E 's/[:=].*/: <hidden>/'
   ```
   Never print the values. If a secret is present, move it to an untracked file the app can include (`~/.config/<app>/local.*`), add that to `.gitignore`, and tell the user.
3. **Check for machine-specific values** (monitor names, PCI paths, absolute `/home/<user>`). Put those in `hosts/<hostname>/` and have the config include them, as `.config/hypr/hyprland.conf` does with `host.conf`.
4. **Copy it into the repo** at the same path relative to `$HOME`: `mkdir -p .config/<app> && cp ~/.config/<app>/<file> .config/<app>/`.
5. **Run `./install.sh --dry-run`.** Every file under `.config/` is linked automatically. Files outside `.config/` need a `link <path>` line in `install.sh`.
6. **Run `./install.sh`.** The original is backed up to `~/.dotfiles-backup/<ts>/` and replaced with a symlink.
7. **Validate** the app's config, then run the `config-verifier` agent.
8. **Update the docs:** README "What's here", and AGENTS.md if you learned a gotcha.
9. **Commit** with `git add <paths>` (never `-A` blindly) and a message like `ghostty: add quick-terminal keybind`.
