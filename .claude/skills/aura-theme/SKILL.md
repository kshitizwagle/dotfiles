---
name: aura-theme
description: Theme a new app (or retheme an existing one) in Aura Dark. Use when the user wants an app, bar, launcher, editor, website or TUI to match the desktop, or asks to "make X look like the rest".
---

# Theme an app in Aura Dark

1. **Read the spec:** `docs/aura-tokens.md`. Every value must come from there.
2. **Look for an official port first.** Run `gh api repos/daltonmenezes/aura-theme/contents/packages --jq '.[].name'`. If the app is listed, use that port (put the files under the right `.config/` path and credit it in `docs/THIRD_PARTY.md`). Otherwise map the tokens yourself.
3. **Find the app's real config format for the installed version.** Check `<app> --version`, look for a bundled example under `/usr/share/<app>`, and use Context7 for docs. Don't guess option names.
4. **Map the tokens, following these rules:**
   - backgrounds: bg / bg-deep / surface / surface-raised. Never pure grey or black.
   - `#a277ff` only on the single most important active element (focus, selection edge, primary action).
   - selection and hover: `#3d375e`, never the full accent
   - 1px `#2e2c3a` borders, 6px radius, no drop shadows
   - text: fg, then metadata `#908f98` for anything secondary
   - status colors only for real status (danger for errors, and so on)
   - motion: ≤160ms ease-out `cubic-bezier(0.23,1,0.32,1)`, no bounce
   - font: IBM Plex Mono for monospace or labels, BlexMono Nerd Font if icons are needed
5. **Put a token comment at the top** of the theme file, listing the hexes used and their token names.
6. **Validate** with the app's own checker if it has one. Then add a row to the "Where each token is applied" table in `docs/aura-tokens.md`.
7. **If the app has no theming hook, or would need a forced theme** (as libadwaita does), say so and explain the tradeoff instead of hacking it.

Report what you rejected from popular rices (big blur, heavy rounding, bounce) and why.
