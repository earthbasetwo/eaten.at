# eaten.at — Design System ("Masthead")

Complete design system for **eaten.at**, a restaurant review platform on AT Protocol. Editorial-magazine register (New Yorker / Atlantic): written word on paper, clean with minimalist influence, retro carried by the Evantic logotype. Hierarchy from type and rules, not color or boxes.

## About the Design Files
`Masthead System Reference.dc.html` is a **design reference in HTML** — a style specimen, not production code. Apply this system to the existing app's codebase by translating tokens and rules into its theming mechanism (CSS variables, Tailwind config, etc.). Restyle existing screens; no layouts are specified.

## Fidelity
High-fidelity for style. All values below are final.

## Design Tokens

### Colors
| Token | Hex | Use |
|---|---|---|
| `paper` | `#F6F1E5` | App/page background (ivory stock) |
| `paper-bright` | `#FDFBF4` | Menus, checked boxes, rare raised surfaces |
| `ink` | `#1C1914` | Primary text, rules, logotype |
| `ink-body` | `#332E24` | Review/body text |
| `ink-soft` | `#4A4336` | Secondary body text, quiet actions |
| `stone` | `#988D75` | Metadata, handles, timestamps, placeholders, unselected states |
| `hairline` | `#DDD5C2` | Minor borders, dividers, quiet underlines |
| `disabled` | `#C9BFA8` | Disabled labels and rules |
| `vermilion` | `#D8401F` | THE accent: ratings, primary actions, focus, selection, links, marks |

Rules:
- Vermilion is scarce — never large fills or backgrounds.
- No other hues. No pure white/black/gray.
- Structure via rules: 1px `ink` above content blocks, 1px `hairline` for minor dividers, 3px double `ink` under mastheads.

### Typography
| Role | Font | Notes |
|---|---|---|
| Logotype ONLY | **Evantic Regular** (`Evantic-Regular.ttf`, bundled) | `eaten.at`, always lowercase. Never elsewhere. Personal-use license (project is non-commercial). |
| Headlines, restaurant names | **Newsreader** 500 (Google Fonts) | line-height 1.1–1.2 |
| Body / review text | **Newsreader** 400 | 14–16px, line-height 1.55–1.6 |
| Action labels (buttons, dropdowns) | **Newsreader** 400 *italic* | 15–18px |
| Metadata, handles, DIDs, ratings, field labels | **JetBrains Mono** 400 | 10–12px; 0.08–0.22em tracking on ALL-CAPS labels |

Scale (px): logotype 40–56 · page titles 28–34 · card titles 20–22 · body/inputs 14–17 · mono meta 10–11.

### Spacing & Shape
- 8px base grid. Section spacing 40–56px, content padding 16–26px, inline gaps 8–24px.
- **Square corners everywhere. No shadows** except the dropdown menu's hard offset (below). Flat paper; separation via rules.
- Reading measure ~600–680px.

## Components

### Masthead
Centered `eaten.at` in Evantic Regular (ink); wide-tracked mono tagline in `stone` beneath; closed by 3px double `ink` rule.

### Review entry (feed row — no card)
- 1px `ink` top rule (or `hairline` within groups); background stays `paper`.
- Name: Newsreader 500 20–22px left; rating right: **word only, caps mono, vermilion, 0.14em tracking** — POOR / FAIR / GOOD / GREAT / SUPERB.
- Meta: mono 10px `stone`: `@handle · cuisine · $$` (lowercase).
- Body: Newsreader 14–15px/1.6 `ink-body`.
- Actions: typeset buttons (below) + plain mono counters (`♥ 84  ⇄ 6`, 11px `stone`; active state vermilion). Counters are NOT buttons.

### Buttons — "Typeset" (no boxes, no fills)
- **Primary:** Newsreader italic 16–18px `ink`, 2px vermilion bottom rule, trailing `→` in vermilion. One per view.
- **Secondary:** italic 15–16px `ink`, 1px `ink` rule. No arrow.
- **Quiet:** italic, `ink-soft`/`stone`, 1px `hairline` rule.
- Hover: rule thickens +1px (use `box-shadow: inset 0 -Npx` so text doesn't shift), arrow +3px right, 150ms ease-out. Active: label → vermilion. Disabled: `disabled` tone. Focus-visible: 2px vermilion outline, 2px offset.
- Real `<button>`/`<a>` elements; pad to ≥44px touch height.

```css
.btn-typeset {
  display:inline-block; font-family:'Newsreader',serif; font-style:italic;
  font-size:17px; color:#1C1914; background:none; border:none; cursor:pointer;
  padding:12px 0 2px; box-shadow:inset 0 -2px #D8401F; transition:box-shadow 150ms ease-out;
}
.btn-typeset .arrow { color:#D8401F; display:inline-block; transition:transform 150ms ease-out; }
.btn-typeset:hover { box-shadow:inset 0 -3px #D8401F; }
.btn-typeset:hover .arrow { transform:translateX(3px); }
.btn-typeset:active { color:#D8401F; }
.btn-typeset:focus-visible { outline:2px solid #D8401F; outline-offset:2px; }
.btn-typeset--secondary { font-size:15px; box-shadow:inset 0 -1px #1C1914; }
.btn-typeset--secondary:hover { box-shadow:inset 0 -2px #1C1914; }
.btn-typeset--quiet { color:#4A4336; box-shadow:inset 0 -1px #DDD5C2; }
```

### Text input — "Bare rule"
- No box, no background: transparent input, 1px `ink` bottom rule (`box-shadow: inset 0 -1px #1C1914`), Newsreader 17px `ink`.
- Focus: rule → 2px vermilion. Placeholder: Newsreader italic `stone`.
- Label (optional): tracked mono caps 10px `stone` above the rule.
- Error: rule and message → vermilion.

```css
.input-rule {
  width:100%; background:transparent; border:none; outline:none;
  box-shadow:inset 0 -1px #1C1914; padding:6px 0;
  font-family:'Newsreader',serif; font-size:17px; color:#1C1914;
}
.input-rule:focus { box-shadow:inset 0 -2px #D8401F; }
.input-rule::placeholder { font-style:italic; color:#988D75; }
```

### Dropdown — typeset trigger + paper menu
- Trigger: Newsreader italic 15–17px `ink`, 1px `ink` underline, vermilion chevron `⌄` (rotates/flips to `⌃` open).
- Menu: `paper-bright`, 1px `ink` border, **hard offset shadow `3px 3px 0 #DDD5C2`** (printed, not floaty), square corners.
- Items: Newsreader italic 14–15px, 1px `hairline` between; selected item vermilion; hover/arrow-key focus: `paper` bg + vermilion text.

### Checkbox — hairline box + custom tailed check
- Box: 16×16px, 1px `ink` border, square, transparent at rest; `paper-bright` when checked.
- Mark: custom vermilion SVG check with horizontal tails flush to both edges (short left tail, longer top-right exit) — like a pen entering and leaving the box. Inline SVG or masked background; never a font glyph (metrics don't center).

```html
<svg width="16" height="16" viewBox="0 0 16 16">
  <path d="M 0 8.5 L 2.8 8.5 L 6.2 13.5 L 12.2 2.2 L 16 2.2"
        stroke="#D8401F" stroke-width="1.8" fill="none" stroke-linejoin="miter"/>
</svg>
```
- Label: Newsreader 16px `ink`. Whole row clickable. Scale box and SVG together (same viewBox).
- Multi-select filter chips (alternative for tag filters): typeset style — selected = `ink` text + 2px vermilion rule; unselected = `stone` + 1px `hairline` rule.

### AT Protocol identity
Handles/DIDs always mono `stone`, lowercase, quiet — metadata, not a feature.

### Links
Vermilion, no underline at rest, underline on hover.

## Interactions
Color/border/underline transitions only, 150–200ms ease-out. No motion, lifts, scales, or blur shadows. Focus-visible: 2px vermilion outline, 2px offset, on everything interactive.

## Assets
- `Evantic-Regular.ttf` — bundled logotype font: `@font-face { font-family:'Evantic'; src:url('Evantic-Regular.ttf') format('truetype'); }`
- Google Fonts: `https://fonts.googleapis.com/css2?family=Newsreader:ital,opsz,wght@0,6..72,400;0,6..72,500;1,6..72,400&family=JetBrains+Mono:wght@400;500&display=swap`

## Files
- `Masthead System Reference.dc.html` — visual specimen: tokens, type, and all components with live states.
- `Evantic-Regular.ttf` — logotype font.
