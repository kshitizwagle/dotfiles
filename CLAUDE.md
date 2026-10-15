@AGENTS.md

## Claude Code specifics

- Skills in `.claude/skills/`:
  - `aura-theme`: theme a new app
  - `add-dotfile`: bring a config under management
  - `hyprland-config`: edit Hyprland safely
  - `new-machine`: bootstrap a host
- Agents in `.claude/agents/`:
  - `desktop-auditor`: read-only system audit
  - `config-verifier`: run every check before a commit
- sudo prompts can't be answered from the `!` prefix. Write root steps into `scripts/*.sh` and ask the user to run them in a real terminal.
- Use Context7 for current docs on Hyprland, Waybar, Ghostty and the rest, but trust the **installed** version's bundled example config over the latest wiki (see AGENTS.md).
