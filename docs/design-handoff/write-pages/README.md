# Handoff: Write Pages — Choose Place + Edit Write-up

## Overview
Two redesigned screens for eaten.at's writing flow:

1. **Choose the place** (`/write`) — the entry screen where the author names the place they ate before writing. Replaces the old labeled-form version.
2. **Edit a write-up** (`/write/…`) — the full editing surface for an existing write-up: title, place, date, tags, markdown body, teaser, rating, meal/price notes, photos, links, and save/delete actions.

Both screens share one design philosophy: **prose-forward forms**. There are no boxed inputs, no field labels (with one exception: small-caps kickers for Digest/Photos), no buttons that look like buttons. Every editable value sits inline inside a readable sentence, underlined; controls reveal themselves on hover; popovers are small paper cards. The page should read like a typeset paragraph that happens to be editable.

## About the Design Files
The files in this bundle are **design references created in HTML** — interactive prototypes showing intended look and behavior, not production code to copy. The task is to **recreate these designs in the target codebase's existing environment**, using its established patterns, components, and libraries. The app has old implementations of both screens; this handoff supersedes them — treat it as a redesign of those routes, not a new feature.

`Write Pages.dc.html` is the prototype source. It uses a proprietary component runtime (`support.js`), so read it for markup, styles, and logic — the templating syntax (`{{ }}`, `sc-if`, `sc-for`) maps 1:1 onto ordinary conditional rendering and list rendering in any framework.

## Fidelity
**High-fidelity.** Colors, typography, spacing, copy, and interactions are final. Recreate pixel-perfectly. All measurements below are exact.

## Design Tokens

Colors (the whole palette — do not introduce others):
- `#1C1914` ink — primary text, solid borders
- `#4A4336` soft ink — prose connectives ("at", "From a visit on"), secondary labels
- `#988D75` stone — placeholders, hints, metadata, idle affordances
- `#C9BFA8` faint stone — unselected rating pluses, dashed borders, focused-placeholder text
- `#DDD5C2` hairline — idle underlines, popover row dividers
- `#F6F1E5` paper — page/card background
- `#FDFBF4` bright paper — popovers, inputs-on-paper surfaces
- `#EFE8D8` recessed paper — image letterboxing, hover fill in calendar
- `#E9E2D2` desk — canvas behind the page (likely already the app shell background)
- `#D8401F` vermilion — accent: focus, hover, links, rating, cover badge
- `#E02B1D` alert red — destructive confirmation (Yes) only
- `rgba(28,25,20,0.1)` — popover offset shadow; `rgba(28,25,20,0.35)` — overlay scrim

Typography:
- **Newsreader** (Google Fonts, optical sizing, weights 400/500/600 + 400 italic) — all prose and inputs
- **JetBrains Mono** (400/500) — kickers, metadata, rating pluses
- Page title / place-name input: 32px, weight 500, line-height 1.2
- Body prose & inline inputs: 17px, line-height 1.55
- Markdown editor body: 16px lh 1.6; h1 25px/500, h2 20px/500, h3 600 weight
- Hints/teaser meta: 13–15px italic
- Kickers (DIGEST, PHOTOS): JetBrains Mono 11px, letter-spacing 0.16em, uppercase, `#4A4336`
- Popover metadata: JetBrains Mono 11px, letter-spacing 0.04em, `#988D75`

Spacing/shape:
- Page card: 784px wide, `#F6F1E5`, padding 48px 32px 56px; content column max-width 544px
- **No border radius anywhere. No soft shadows.** Popovers: 1px solid `#1C1914` border + hard offset shadow `4px 4px 0 rgba(28,25,20,0.1)` on `#FDFBF4`
- Transitions: 150–160ms ease-out on color/underline/opacity

The signature control — **inline underlined input**:
- Transparent background, no border/outline/padding; inherits the sentence's font
- Width fits content (`field-sizing: content` with a min width in `ch`; provide a JS mirror-span fallback where unsupported)
- Underline: `text-decoration underline`, 1px thickness, `text-underline-offset 0.14em`
- Idle underline `#DDD5C2` (or transparent for the 32px title) → hover `#1C1914` → focus `#D8401F`
- Placeholders: Newsreader italic `#988D75`; the two big non-italic placeholders (title/place name) dim to `#C9BFA8` on focus

Buttons-as-prose:
- Primary action: italic 18px ink text with a vermilion "underline" drawn as `box-shadow: inset 0 -2px #D8401F` (3px on hover) + a vermilion `→`
- Secondary (Delete, No): italic 16px ink, 1px ink inset underline (2px on hover)
- Hidden affordances (add a link, write your own teaser): italic 13–14px `#988D75`, hairline underline, darken to `#4A4336` on hover
- Icon buttons (reset ×, change place): 0.7 opacity vermilion, 1.0 on hover, revealed only while hovering their row

---

## Screen 1: Choose the place (`/write`)

### Layout
One card (784px, min-height 560px), content column 544px. No heading, no kicker — the screen IS the input.

### Components
1. **Place-name input** — the 32px/500 headline-style input, full column width. Placeholder is a real example: `St. John` (placeholder gray `#988D75`, non-italic, dims to `#C9BFA8` on focus). Underline transparent when idle → `#DDD5C2` hover → `#D8401F` focus.
2. **Suggestions popover** — absolutely positioned under the name input (top 100% + 8px, full input width, z-index above the address line). Paper card (`#FDFBF4`, ink border, hard shadow). Each row: flex baseline, wrap, gap 4px 8px, padding 9px 16px; rows after the first get a 1px `#DDD5C2` top border. Row content: place name (Newsreader 17px ink) + metadata (JetBrains Mono 11px `#988D75`): `12 Example Lane · 0.4 mi`. Highlighted row (hover or keyboard): background `#F6F1E5`, name turns `#D8401F`.
3. **Address line** — immediately below (4px gap), a sentence: italic soft-ink `at` + inline underlined input (17px), placeholder `26 St John Street, London`, then an italic period. Idle underline `#DDD5C2`.
4. **"Start writing →"** — primary prose button, 48px above-margin. Only rendered once the name field is non-empty. Navigates to the editor.

### Behavior
- Suggestions open when the input is focused AND non-empty AND there are matches. (Prototype filters a static list by substring; production queries the places API — Overture Maps data — biased by user location.)
- Keyboard: ↓/↑ cycle highlight (wrapping), Enter picks the highlighted row (if none, blurs and keeps the typed text), Escape closes/blurs. Mouse: hover highlights, click picks (use mousedown or equivalent so blur doesn't swallow the click).
- **Picking a suggestion fills both fields** (name + address) and closes the list. The list does not reopen while the current name+address exactly match the picked suggestion.
- **Manual entry is just typing.** No separate "enter it yourself" mode: an unmatched name plus a hand-typed (or empty) address is a valid custom place.
- **Clearing the name clears the address too** — both return to placeholders (prevents an orphaned address from an earlier pick).
- Address is optional; "Start writing" requires only a name.

## Screen 2: Edit a write-up (`/write/…`)

### Layout
One card (784px), stacked top to bottom:
title row → place line (4px) → date/tags row (24px) → Digest editor (24px) → teaser line (24px) → rating + meal/price row (32px) → Photos (32px) → links row (40px) → actions row (40px).

### 2.1 Title + place (adaptive)
- **Title input** (32px/500): value is the custom title; placeholder is the place name. Underline transparent/hover-hairline/focus-vermilion.
- **Place line** below: italic `at` + inline inputs + `.` — when NO custom title: just the address (`at 12 Example Lane.`); when a custom title exists: place name input + `,` + address input (`at Noodle House, 12 Example Lane.`).
- **Reset ×** (12px stroke-1.4 X svg, vermilion, 0.7→1 opacity): shown after the title text only while hovering the title row AND a custom title exists. Clicking clears the title (place name resumes as placeholder).
- **Change-place button** (folded-map icon, 15px, vermilion, same opacity treatment): after the title when no custom title (hover-revealed on title row); after the address line when a custom title exists (hover-revealed on place row). Opens the place chooser.

### 2.2 Date + tags row
Two-ended flex row (space-between, baseline).
- Left: `From a visit on` (italic soft ink) + **date button** (underlined ink, hover vermilion) + `.` Label format: `September 6` (year appended as `, 2025` only if not the current year).
- **Date popover**: 266px paper card under the button. Header: ← month/year (italic 15px) →. Grid: 7×34px columns, 1px gap; day-of-week initials S M T W T F S (italic 12px `#988D75`); day cells 34×32px Newsreader 14px — selected: ink background/paper text; today: vermilion underline; hover: `#EFE8D8` fill. Closes on select or outside click.
- Right: `Filed under` + tag list. Each tag is an underlined word followed by `, `; hovering a tag turns it vermilion with a line-through (click removes). Trailing inline input (placeholder `a tag` / `another tag`): Enter or comma files the draft, Backspace on empty deletes the last tag, blur files any draft. Dedup case-insensitively.

### 2.3 Digest (markdown body)
Kicker `DIGEST`, then an Obsidian-style **live markdown editor**: bright-paper box, ink border (vermilion while active), padding 16px 18px, min-height 384px.
- Content renders formatted (h1–h3, bullet lists with vermilion `•`, blockquotes with 2px `#DDD5C2` left border + italic soft ink, bold/em, `code` in JetBrains Mono 13px on `#EFE8D8`, links in vermilion underline).
- The line containing the caret shows its raw markdown: syntax tokens (`#`, `**`, `` ` ``, `[](…)`) appear inline in faint `#C9BFA8` while formatting is preserved. Leaving the line re-collapses tokens.
- Caret is vermilion. Enter splits lines (list/quote prefixes continue; Enter on an empty list/quote line clears it); Backspace at line start merges up; ↑/↓ cross line boundaries preserving horizontal position; Escape deactivates. Paste is flattened to plain text.
- Use whatever markdown-editor approach fits the codebase (CodeMirror with a live-preview extension, ProseMirror, or the existing editor) — the visual spec above is the requirement, not the prototype's hand-rolled implementation.

### 2.4 Teaser
Collapsed: italic 14px `#988D75`: `In listings, the piece opens with its first lines — or write your own teaser.` (the last phrase is a hairline-underlined action).
Expanded: hint line `In listings it opens:`, a 2-row transparent textarea (15px/1.6) whose placeholder is the auto-teaser (first lines, in quotes), and a context line below: if untouched, `Drawn from the first lines — type to say it differently, or leave it be.`; if custom text exists, `never mind — use the first lines` (resets and collapses).

### 2.5 Rating + meal/price row
Space-between flex row.
- Left: clear box (22px square, 1px `#C9BFA8` border, gray × inside; hover darkens border) + four `+` glyphs (JetBrains Mono 26px, gap 10px): filled `#D8401F` up to the shown value, rest `#C9BFA8`; hover previews, click sets. Label to the right (mono 11px caps, letter-spacing 0.14em): Unrated / Solid / Recommended / Strongly Recommended / Can't Miss — vermilion when > 0, stone otherwise.
- Right (baseline, gap 28px): **meal** button (`Dinner`, or `a meal` in stone when unset) and **price** button (`$$`, or `$?`) — underlined, hover vermilion. Each opens a small paper popover (right-aligned, min-width 124px/88px, padding 10px 16px): options as left-aligned text rows (15px, lh 1.9; selected = ink + underline, others stone→ink on hover), plus an italic 13px `no note` clear row. Meal options: Breakfast, Lunch, Dinner, A snack, Drinks. Price: `$`–`$$$$`. One popover at a time; outside click closes.

### 2.6 Photos
Kicker `PHOTOS`.
- Empty state: dashed `#C9BFA8` border box, padding 28px 24px, centered italic 15px `Nothing to look at yet.` — on hover the whole box is a button: text becomes vermilion `add a photo`, border darkens. Click opens the system file picker (images, multiple).
- Grid: 4 columns, 10px gap, square tiles, `object-fit: cover` on `#EFE8D8`. First tile gets a `COVER` badge (mono 9px caps, ink bg, paper text, bottom-left). Hovering a tile reveals a delete button (22px square, bright paper, ink border, vermilion ×, top-right). Last cell is always an add tile: dashed border, centered `+` (mono 24px stone; hover darkens).
- Drag to reorder (dragged tile: 0.35 opacity + dashed vermilion outline; reorder live on drag-enter). Hint below: italic 13px `Drag to reorder — the first photo is the cover.`
- Click a tile → **detail overlay**: scrim `rgba(28,25,20,0.35)` covering the card, centered paper panel (max 560px, padding 18px, hard shadow 6px 6px): image (max-height 440px, contain, `#EFE8D8` letterbox), centered underlined caption input (`add a caption…`), footer row `done` (left) / `remove this photo` (right, hover vermilion). Escape or scrim click closes.

### 2.7 Links row
Space-between, baseline, 40px top margin.
- Left: `Bluesky:` (italic soft ink) `see the thread.` — a plain link to the syndicated thread (underlined ink).
- Right, right-aligned: `Elsewhere:` + comma-separated link labels (underlined ink; hover vermilion; click opens the editor card) or italic stone `nowhere yet` when empty. An `— add a link` action (italic 14px stone) appears while hovering the row (always visible when empty).
- **Link editor card** (below, right-aligned, max 420px, paper, hard shadow, padding 16px 18px): two prose rows — `shown as` + label input (placeholder `Website`), `pointing at` + URL input (placeholder `https://…`) + an open-in-new-tab icon link (14px, stone→vermilion). Footer: `save it · never mind` (left), `remove it` (right, hover vermilion, only when editing an existing link). Enter saves, Escape cancels. Saving with an empty URL cancels; an empty label falls back to the URL's host.

### 2.8 Actions row
Baseline flex, gap 24px, 40px top margin.
- **Save changes →** — primary prose button.
- **Delete** with inline confirmation: clicking replaces it (same slot) with `Delete?` (italic 16px `#4A4336`, no underline) + **Yes** + **No**, gap 16px. Yes/No: italic 17px, letter-spacing 0.06em, underline via `inset 0 -1px` box-shadow with NO bottom padding (underline hugs the text; 2px on hover). **Yes is alert red `#E02B1D`** (text + underline) — this red is used nowhere else. No is ink. No cancels back to `Delete`; Yes deletes the write-up and navigates away.

## Interactions & Behavior (summary)
- All hover reveals (title ×, change-place, add-a-link, photo deletes, empty-photo CTA) are hidden until their row/tile is hovered; on touch devices make them always visible or reveal on first tap.
- All popovers (date, meal, price, suggestions) close on outside click and are mutually exclusive.
- All transitions 150–160ms ease-out; no motion beyond color/opacity/underline changes.
- Escape closes the topmost transient (popover, photo overlay, editor line focus).

## State Management
Choose-place: `name`, `address`, `suggestions` (async), `highlightIndex`, `listOpen`. Editor: `title`, `place`, `address`, `date`, `tags[]`, `tagDraft`, `body` (markdown), `teaser` (+ expanded flag), `rating` (0–4) + hover preview, `meal`, `price`, `photos[]` ({id, src, caption}, order = display order, first = cover), `links[]` ({url, label}) + edit-index, open-popover id, `deleteConfirming`. Persist on Save; the prototype does not model autosave — follow existing app conventions.

## Assets
- Fonts: Newsreader + JetBrains Mono (Google Fonts; self-host per app convention).
- Icons: all inline SVG, stroke-based, drawn in the prototype — × (2 crossing strokes, width 1.4–2), folded map (change place), chain link (open URL). Recreate as app icons at the sizes given; no external icon assets.
- Photos in the prototype are placeholder swatches; production uses user uploads.

## Files
- `Write Pages.dc.html` — prototype source: section `2a` = choose place, section `1b` = edit write-up. All exact styles are inline in the markup; all behavior is in the script block at the bottom.
- `support.js` — prototype runtime, included only so the HTML opens in a browser. Not relevant to implementation.
