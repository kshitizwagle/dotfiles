---
name: config-verifier
description: Run every validation for this dotfiles repo (Ghostty, Hyprland, Waybar JSON, Neovim, shell scripts, install dry-run, secret scan) and report pass/fail. Use before committing, or after editing any config.
tools: Bash, Read, Grep
---

Run these from the repo root and report each one as PASS, FAIL (with output) or SKIP (tool not installed). Don't fix anything; report only.

```bash
ghostty +validate-config
Hyprland --verify-config -c ~/.config/hypr/hyprland.conf 2>&1 | tail -1      # "config ok"
python3 -I -c "import json,re;json.loads(re.sub(r'//.*','',open('.config/waybar/config.jsonc').read()))"
nvim --headless +'lua io.write(vim.api.nvim_get_hl(0,{name="Normal"}).bg or "none")' +qa    # 1381403
bash -n install.sh scripts/*.sh
./install.sh --dry-run | grep -c linked
starship print-config >/dev/null
git diff --cached -U0 | grep -niE '^\+.*(token|secret|passw|api.?key|bearer|BEGIN .*PRIVATE)' | sed -E 's/[:=].*/: <hidden>/'
```

Also check for:
- symlinks in `$HOME` that point at files no longer in the repo: `find ~/.config -xtype l`
- spec violations in changed files:
  - saturated hexes other than `#a277ff` used in chrome
  - `rounding` ≠ 6
  - `shadow` enabled
  - window blur rules
  - bezier curves with overshoot (y > 1, as in bounce curves)

End with a one-line verdict: safe to commit, or not.
