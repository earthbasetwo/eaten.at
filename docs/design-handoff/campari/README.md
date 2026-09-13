# Handoff: Campari Design System

## Overview
A design system ("Campari") for a restaurant review platform built on AT Protocol. Warm "aperitivo hour" aesthetic: apricot-blush backgrounds, oxblood ink, Campari red accents, serif display type over a friendly sans body. Classy but approachable — a food-magazine voice with modern-app ergonomics.

## About the Design Files
The file in this bundle (`Campari Reference.dc.html`) is a **design reference created in HTML** — a style specimen showing intended look, not production code. The task is to **apply this design system to the existing app's codebase** (its framework, component library, and theming mechanism), translating these tokens and rules into the app's existing patterns (CSS variables, Tailwind config, styled-components theme, etc.).

## Fidelity
**High-fidelity for style, not layout.** Colors, typography, radii, and spacing values below are final and should be applied exactly. No screen layouts are specified — restyle the app's existing screens/components using these tokens and component rules.

## Design Tokens

### Colors
| Token | Hex | Use |
|---|---|---|
| `bg` | `#F7E7DC` | App/page background (apricot blush) |
| `bg-raised` | `#FCF3EB` | Cards, inputs, sheets, popovers |
| `bg-sunken` | `#F1DCCC` | Section bands, list backgrounds, wells |
| `ink` | `#40201C` | Primary text (oxblood) |
| `ink-secondary` | `#5E3C31` | Body/review text |
| `ink-muted` | `#7E574A` | Secondary text, captions |
| `ink-faint` | `#A87F6C` | Metadata, handles, timestamps, placeholders |
| `border` | `#E5C9B8` | All hairline borders and dividers |
| `accent` | `#C41E2F` | Campari red — primary actions, active states, ratings, links |
| `accent-contrast` | `#FCF3EB` | Text/icons on accent |
| `accent-soft` | `#E8804F` | Secondary accent — highlights, badges, illustration fills |

Rules:
- Accent red is for actions and ratings only — never large surface fills.
- Text on `bg`/`bg-raised` uses `ink` shades only (all pass 4.5:1 except `ink-faint`, which is for ≥small-caps metadata).
- No pure white, no pure black, no grays — every neutral is in the blush/brown family.

### Typography
| Role | Font | Weight | Notes |
|---|---|---|---|
| Display / headings / restaurant names | **Instrument Serif** (Google Fonts) | 400 | Tight line-height 1.0–1.1; use size, not weight, for hierarchy |
| Body / UI / buttons | **DM Sans** (Google Fonts) | 400 / 500 / 700 | Line-height 1.5–1.55 for body |
| Metadata / handles / DIDs | **JetBrains Mono** (Google Fonts) | 400 | 10–12px, `ink-faint`, optional 0.08em letter-spacing on labels |

Scale (px): 44 / 34 (page titles) · 22–20 (card titles, Instrument Serif) · 14–15 (body) · 12–13 (buttons, labels) · 10–11 (mono metadata).

### Spacing
- 8px base grid.
- Card padding: 16–20px. Section spacing: 40px. Inline gaps: 8–12px.

### Shape & Elevation
- Radius: 12px cards/inputs, 10px nested cards, 999px (pill) buttons and badges, 6–8px small chips/swatches.
- Borders over shadows: 1px `border` on every raised surface.
- Shadow (sparingly, cards only): `0 2px 8px rgba(64,32,28,0.06)`.

## Components

### Review card
- `bg-raised`, 1px `border`, radius 10–12px, padding 16–18px, optional soft shadow.
- Header row: restaurant name (Instrument Serif 20–22px, `ink`) left; rating right.
- Rating format: word + dots — e.g. `Superb ●●●●○` — DM Sans 700, 12px, `accent`. Words: Poor / Fair / Good / Great / Superb (1–5).
- Meta line: JetBrains Mono 10px `ink-faint`: `@handle · cuisine · $$`.
- Review text: DM Sans 13–14px, line-height 1.55, `ink-secondary`.
- Action row (flex, gap 8px): primary pill `accent` bg / `accent-contrast` text; secondary pills transparent with 1px `border`, `ink-muted` text. DM Sans 700/500, 11–12px, padding 5–6px 14–16px.

### Buttons
- Primary: pill, `accent` bg, `accent-contrast` text, DM Sans 700.
- Secondary: pill, transparent, 1px `border`, `ink-muted` text.
- Hover: darken accent ~8% / fill secondary with `bg-sunken`. Min touch target 44px on mobile.

### AT Protocol identity
- Handles (`@name.food.social`) and DIDs always in JetBrains Mono, `ink-faint`.
- Keep protocol chrome quiet — it's metadata, not a feature callout.

### Links
- `accent` color, no underline at rest, underline on hover.

## Interactions & Behavior
- Transitions: 150–200ms ease-out on color/background/border only. No large motion.
- Focus: 2px `accent` outline with 2px offset.
- Hover on cards: border darkens to `#D9B49C` (no lift/scale).

## State Management
None specified — this is a theming handoff; keep the app's existing state as is.

## Assets
No image assets. Fonts from Google Fonts:
`https://fonts.googleapis.com/css2?family=Instrument+Serif:ital@0;1&family=DM+Sans:opsz,wght@9..40,400;9..40,500;9..40,700&family=JetBrains+Mono:wght@400;500&display=swap`

## Files
- `Campari Reference.dc.html` — the visual specimen (open in a browser). Contains the palette, type samples, and a fully-styled review card matching the rules above.
