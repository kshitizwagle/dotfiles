# Aura Dark — design tokens

Based on the Aura theme by Dalton Menezes (github.com/daltonmenezes/aura-theme).

## Color
| Token | Hex | Use |
|---|---|---|
| bg | `#15141b` | Main background (windows, terminal, editor) |
| bg-deep | `#0f0e14` | Bars, headerbars, sidebars, login/lock background |
| surface | `#1d1c25` | Cards, dialogs, notifications |
| surface-raised | `#25242f` | Popovers, menus, tooltips |
| border | `#2e2c3a` | 1px hairlines, inactive window border |
| fg | `#edecee` | Primary text |
| muted | `#bdbdbd` | Secondary text |
| metadata | `#908f98` | Timestamps, inactive labels, clock |
| gray | `#6d6d6d` | Decoration only (placeholders, disabled) |
| accent | `#a277ff` | **The only saturated chrome color.** Focus, active workspace, active border, primary button |
| accent-soft | `#8464c6` | Hover on accent |
| accent-tint | `#3d375e` | Selection, hover backgrounds |
| green | `#61ffca` | Success |
| blue | `#82e2ff` | Info |
| warning | `#ffca85` | Warning |
| danger | `#ff6767` | Error, critical |
| pink | `#f694ff` | Rarely |

## Type
- IBM Plex Mono (terminal, bar, labels). BlexMono Nerd Font where icons are needed.
- System sans (Inter) for UI text.

## Shape
- Borders before shadows: 1px hairlines, shadows off.
- Radius 6px by default.
- No blur on windows. Blur is allowed only on the bar/panel and notifications.

## Motion
- ease-out `cubic-bezier(0.23, 1, 0.32, 1)`; ease-in-out `cubic-bezier(0.77, 0, 0.175, 1)`.
- Durations 160ms / 420ms / 600ms max. Small travel, no bounce.
- Hyprland speeds are in deciseconds: 160ms = `1.6`, 420ms = `4.2`.

## Tone
Minimalist and dark, with purple-tinted neutrals (never flat grey). One accent per view.

## Where each token is applied
| Token | GTK4/3 | Ghostty | Waybar | Hyprland | Neovim | swaync | hyprlock | GDM | fuzzel / starship |
|---|---|---|---|---|---|---|---|---|---|
| bg | window/view | background | n/a | n/a | official port | n/a | n/a | entry bg | fuzzel bg |
| bg-deep | headerbar/sidebar | n/a | bar 85% | misc bg | n/a | control center 85% | background | background | n/a |
| surface | card/dialog | n/a | n/a | n/a | n/a | notification | input inner | card | n/a |
| surface-raised | popover | n/a | tooltip | n/a | n/a | buttons | n/a | menus, buttons | n/a |
| border | border-color | n/a | bottom hairline | inactive border | n/a | 1px | n/a | card, entry | fuzzel border |
| fg | fg | foreground | text | n/a | Normal | text | text | text | text |
| metadata | n/a | n/a | inactive ws, title | n/a | comments | body/time | placeholder | clock | git branch |
| accent | accent | cursor, palette 4/5 | active ws | active border | keywords | DND on | input outer | focus, primary btn | match, dir, ❯ |
| accent-tint | n/a | selection | hover | n/a | Visual | hover | n/a | selected user | fuzzel selection |
| danger | n/a | palette 1 | critical | n/a | errors | critical border | fail | n/a | error ❯ |
