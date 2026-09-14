# Handoff: eaten.at "Masthead" Design System

## Overview
Design system for **eaten.at**, a restaurant review platform on AT Protocol. Editorial-magazine direction (New Yorker / Atlantic register): written word on paper, clean with minimalist influence, retro touch carried by the Evantic logotype. Ivory stock, near-black ink, one vermilion accent — hierarchy comes from type and rules, not color.

## About the Design Files
`Masthead Reference.dc.html` is a **design reference in HTML** — a style specimen, not production code. The task is to **apply this system to the existing app's codebase**, translating tokens and rules into its theming mechanism (CSS variables, Tailwind config, etc.). Restyle existing screens; no new layouts are specified.

## Fidelity
High-fidelity for style. Colors, fonts, type scale, radii, and rules below are final.

## Design Tokens

### Colors
| Token | Hex | Use |
|---|---|---|
| `paper` | `#F6F1E5` | App/page background (ivory stock) |
| `paper-bright` | `#FDFBF4` | Raised surfaces: cards, inputs, sheets |
| `ink` | `#1C1914` | Primary text, rules, logotype |
| `ink-body` | `#332E24` | Review/body text |
| `ink-soft` | `#4A4336` | Secondary body text |
| `stone` | `#988D75` | Metadata, handles, timestamps, placeholders |
| `hairline` | `#DDD5C2` | Borders, dividers |
| `vermilion` | `#D8401F` | THE accent: ratings, primary actions, links, active states |

Rules:
- Vermilion is scarce — ratings, one primary action per view, links. Never large fills, never backgrounds.
- No other hues. No pure white/black/gray.
- Structure via rules: 1px `ink` rules above content blocks, 1px `hairline` for minor dividers, 3px double `ink` rule under mastheads/section heads.

### Typography
| Role | Font | Notes |
|---|---|---|
| Logotype ONLY | **Evantic Italic** (`Evantic-Italic.ttf`, bundled) | `eaten.at`, always lowercase. Never for anything else. Personal-use license — fine here (non-commercial). |
| Headlines, restaurant names, titles | **Newsreader** 500 (Google Fonts) | Serif; line-height 1.1–1.2 |
| Body / review text | **Newsreader** 400 | 14–16px, line-height 1.55–1.6 |
| UI labels, buttons, metadata, handles, DIDs, ratings | **JetBrains Mono** 400 (Google Fonts) | 10–12px; letter-spacing 0.08–0.22em on ALL-CAPS labels |

Scale (px): logotype 40–52 · page titles 28–34 (Newsreader 500) · card titles 20–22 · body 14–15 · mono meta 10–11 · small-caps mono labels 10–11 with wide tracking.

### Spacing & Shape
- 8px base grid. Card padding 16–24px, section spacing 40–56px, inline gaps 8–18px.
- **Square corners** (0px radius) everywhere except tiny badge pills if unavoidable. No shadows — flat paper; separation via rules and `paper-bright`.
- Generous whitespace; single-column reading measure ~600–680px for review text.

## Components

### Masthead / app header
Centered `eaten.at` in Evantic (ink), under it a wide-tracked mono tagline in `stone` (e.g. `THE FEDERATED TABLE`), closed by a 3px double `ink` rule.

### Review entry (feed row, not a card)
- Separated by 1px `ink` top rule (or `hairline` between grouped rows). Background stays `paper`.
- Restaurant name: Newsreader 500, 20–22px, `ink`, left; rating right.
- **Rating: word only, ALL CAPS mono, vermilion, 0.14em tracking** — `POOR / FAIR / GOOD / GREAT / SUPERB`. No stars, no dots, no numbers.
- Meta line: mono 10px `stone`: `@handle · cuisine · $$` (lowercase).
- Review text: Newsreader 14px/1.6 `ink-body`.
- Actions: plain mono text links, 11px `stone`, gap 18px: `♥ 84  ↩ 12  ⇄ 6`; active/liked state vermilion. No pill buttons.

### Buttons
- Primary: rectangular (0 radius), `ink` background, `paper-bright` text, mono 11–12px uppercase tracked. Vermilion background only for the single most important action per view.
- Secondary: 1px `ink` border, transparent, `ink` text.
- Hover: invert (fill ↔ outline), 150ms. Min 44px touch targets on mobile.

### AT Protocol identity
Handles/DIDs always mono `stone`, lowercase, quiet. Provenance is metadata, not a feature.

### Links
Vermilion, no underline at rest, underline on hover.

## Interactions
Color/border transitions only, 150–200ms ease-out. No motion, lifts, or scales. Focus: 2px vermilion outline, 2px offset.

## Assets
- `Evantic-Italic.ttf` — bundled; load via `@font-face { font-family:'Evantic'; src:url(...) format('truetype'); }`
- Google Fonts: `https://fonts.googleapis.com/css2?family=Newsreader:ital,opsz,wght@0,6..72,400;0,6..72,500;1,6..72,400&family=JetBrains+Mono:wght@400;500&display=swap`

## Files
- `Masthead Reference.dc.html` — visual specimen (open in browser)
- `Evantic-Italic.ttf` — logotype font
