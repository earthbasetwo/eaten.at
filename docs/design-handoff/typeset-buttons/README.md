# eaten.at — "Typeset" Button Style (6a)

Buttons as magazine type: no boxes, no fills. Actions are serif italic labels with an underline rule; the primary carries a vermilion arrow. Apply to the existing eaten.at Masthead system (ivory paper, ink text, vermilion accent).

## Tokens used
| Token | Hex |
|---|---|
| `ink` | `#1C1914` |
| `ink-soft` | `#4A4336` |
| `stone` | `#988D75` |
| `hairline` | `#DDD5C2` |
| `vermilion` | `#D8401F` |
| `paper` | `#F6F1E5` |

Font: **Newsreader** italic 400 (Google Fonts) for labels. Arrow is a plain `→` character.

## Variants

### Primary
- Newsreader italic, 16–18px, `ink`.
- 2px solid `vermilion` bottom rule, 2px padding-bottom.
- Trailing `→` in `vermilion`, separated by a space.
- One primary per view.

### Secondary
- Newsreader italic, 15–16px, `ink`.
- 1px solid `ink` bottom rule.
- No arrow.

### Tertiary / quiet
- Newsreader italic, `ink-soft` or `stone`.
- 1px solid `hairline` bottom rule.

### Inline/compact (inside feed rows)
- Same as primary/secondary at 14–15px, sits on the action row baseline next to mono counters (`♥ 84` etc., JetBrains Mono 11px `stone` — counters are NOT buttons styled this way; they stay plain mono text).

## States
- **Hover:** underline thickens (primary 2→3px, secondary 1→2px) and `→` translates 3px right. Transition 150ms ease-out. Implement the thickening with `box-shadow: inset 0 -Npx` or a pseudo-element so text doesn't shift — not border-width.
- **Active:** label color → `vermilion`.
- **Focus-visible:** 2px `vermilion` outline, 2px offset.
- **Disabled:** label + rule → `hairline`-tone (`#C9BFA8`), no hover.

## Accessibility / ergonomics
- Render as real `<button>`/`<a>`; the visual is text but the hit area must be ≥44px tall on touch — use padding, keep the underline tight to the text.
- Because affordance is subtle, reserve this style for actions in editorial context (feed rows, article pages); destructive confirmations may need stronger treatment.

## Reference CSS

```css
.btn-typeset {
  display: inline-block;
  font-family: 'Newsreader', serif;
  font-style: italic;
  font-size: 17px;
  color: #1C1914;
  background: none; border: none; cursor: pointer;
  padding: 12px 0 2px; /* top padding grows hit area */
  box-shadow: inset 0 -2px #D8401F;
  transition: box-shadow 150ms ease-out;
}
.btn-typeset .arrow { color: #D8401F; display: inline-block; transition: transform 150ms ease-out; }
.btn-typeset:hover { box-shadow: inset 0 -3px #D8401F; }
.btn-typeset:hover .arrow { transform: translateX(3px); }
.btn-typeset:active { color: #D8401F; }
.btn-typeset:focus-visible { outline: 2px solid #D8401F; outline-offset: 2px; }

.btn-typeset--secondary { font-size: 15px; box-shadow: inset 0 -1px #1C1914; }
.btn-typeset--secondary:hover { box-shadow: inset 0 -2px #1C1914; }
.btn-typeset--quiet { color: #4A4336; box-shadow: inset 0 -1px #DDD5C2; }
```

```html
<button class="btn-typeset">Write a review <span class="arrow">→</span></button>
<button class="btn-typeset btn-typeset--secondary">Follow</button>
```

## Files
- `Typeset Buttons Reference.dc.html` — visual specimen with all variants and states.
