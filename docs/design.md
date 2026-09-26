# Design language

How eaten.at looks and why. The direction is **Masthead**, adopted on
2026-09-14 from a design handoff (`docs/design-handoff/masthead/`), which
replaced Campari (`docs/design-handoff/campari/`, adopted two days
earlier): an editorial magazine's register, the written word on paper.
Ivory stock, near-black ink, and one vermilion accent; a text serif for
everything read and a mono for everything about it; the retro touch is the
Evantic logotype. Hierarchy comes from type and rules, not from colour,
boxes, or shadows.

The handoff fixes style, not layout. Its tokens are applied exactly; the
page structure is eaten.at's own and predates it. Buttons followed a
second handoff, Typeset (`docs/design-handoff/typeset-buttons/`, adopted
2026-09-15): actions set as magazine type on a rule, in place of
Masthead's boxed mono buttons.

The system was then gathered into one document,
`docs/design-handoff/system-v2/`, adopted 2026-09-17. It restates
Masthead and Typeset unchanged and settles the controls the first two
left open: a text field is a **bare rule** rather than a boxed well, a
dropdown is a typeset trigger over a printed paper menu, and a checkbox
is a hairline square with a pen-stroke check. Those three are what
2026-09-17 changed; everything else it says was already here.

The editor's two screens followed a fourth handoff, Write Pages
(`docs/design-handoff/write-pages/`, adopted 2026-09-20): **prose-forward
forms**. Choosing the place and editing a digest are set as typeset
paragraphs that happen to be editable: no boxed inputs, no field labels
but two kickers, no buttons that look like buttons; every value sits
inline in a sentence on a hairline, controls reveal themselves under the
pointer, and popovers are small paper cards. It fixes the editor alone;
the rest of the site is unchanged, except that its popover shadow (the
ink at a tenth, offset 4px) became the site's one menu shadow. See
**The editor** below.

A handoff's reference page opens in a browser as plain HTML. The viewer
runtime the design tool shipped with it (`support.js`) is not kept and is
not needed: hover states are `style-hover` attributes that runtime
applied, and the rest render as themselves. In the v2 page two specimens,
the dropdown and the checkbox, are driven by that runtime and show their
`{{ … }}` bindings instead of a control; the README carries every value
they would have demonstrated. A reference page's Google Fonts links are
the only thing it fetches.

The stylesheet is `crates/eaten-at-web/static/app.css`, a single file
in cascade layers with no build step. Everything named here is a token or
a class in that file.

## Principles

1. **Reading first.** The digest is the point. The visit's facts and
   every piece of chrome sit beside the text and stay out of its way.
2. **Two voices, and a signature.** Everything read is the serif
   (Newsreader): titles and names at weight 500, text at 400. Everything
   *about* the content (dates, handles, DIDs, URLs, labels, ratings) and
   a chip is small mono (JetBrains Mono), in capitals when it labels.
   Everything *pressed* is the serif in italic on a rule: an action is
   typeset, not boxed. The logotype alone is Evantic, always
   lowercase, and it is never used for anything else. A reader can tell
   content from chrome by typeface alone.
3. **Vermilion is scarce.** The accent is for the rating, links, active
   states, and the rule and arrow under one primary action per page. Never a large fill, a
   heading, a border, or a background (except a field's error state and
   the form error's rule).
4. **Rules, not boxes.** Things a reader picks between (digests,
   feeds, places, photos) are rows on the paper: a 1px ink rule
   above the first, hairlines between the rest. Sections open with a 1px
   ink rule; mastheads close with a 3px double one. A field is a rule
   too: what is typed sits on the paper with 1px of ink beneath it, not
   inside a well. The raised surface, `paper-bright`, is kept for the
   few things that really do sit on top of the page — a menu, a checked
   box, a sheet, the visit's fact box — with a rule and square corners.
   Nothing has a radius, a lift, or a scale, and the one shadow in the
   system is a menu's hard 4px offset in the ink at a tenth, which is a
   printed edge rather than a blur.
5. **Same shape everywhere.** Every page opens the same way: a small
   mono kicker, then the title, then the content. A listing row is a
   small document page. A chooser row is a small listing row.
6. **A feed recolours, never restructures.** An author's theme
   replaces paper, ink, and vermilion; layout, type, spacing, and shape
   are fixed.
7. **Server-rendered, no script.** Nothing here needs JavaScript. A few
   islands make pages quicker: the landing page's two Connect buttons
   become the sign-in and the lookup field in place, the sign-in,
   lookup, and landing pages suggest handles as you type from the
   Bluesky AppView (D42), the editor suggests places as you type through
   this site's own endpoint (D45), the author's home applies its find as
   you type, and the editor keeps a draft, dresses its controls (a
   calendar, paper menus, tags as chips, a live markdown editor), and
   manages photos in place, uploading each as it is picked. Without any
   of them, a handle typed in full, a Find button, and a plain form do
   the same work: the place's name and address typed by hand, a date
   field, radios, selects, a textarea, and the photos page. Nothing asks the browser
   for its location: the search looks near where the request is from
   (D44). Fonts are self-hosted and content-hashed, so no page makes a
   third-party request except the three handle pages' one call to the
   AppView, which their policy alone allows; the strict CSP holds
   everywhere else.
8. **Light only.** Masthead is one palette. The site does not follow the
   system's dark preference.

## Palette

The default palette is Masthead's, exact. There are no pure whites, pure
blacks, or grays: every neutral is in the ivory and umber family, and
there is no hue but vermilion.

| Token | Hex | Use |
|---|---|---|
| `--color-paper` | `#F6F1E5` | page ground (ivory stock) |
| `--color-paper-bright` | `#FDFBF4` | menus, a checked box, sheets, the visit's fact box, the text on ink and vermilion fills |
| `--color-ink` | `#1C1914` | titles, strong text, rules, a field's rule, the logotype, secondary buttons |
| `--color-ink-body` | `#332E24` | a digest's text, excerpts, comment text |
| `--color-ink-soft` | `#4A4336` | ledes, hints, field labels, notices, quiet links |
| `--color-stone` | `#988D75` | the mono metadata voice: dates, handles, URLs, kickers, placeholders |
| `--color-hairline` | `#DDD5C2` | minor rules, every bright surface's border |
| `--color-vermilion` | `#D8401F` | links, the primary action's rule and arrow, active states, the rating |
| `--color-disabled` | `#C9BFA8` | a disabled action's or field's label and rule; in the editor, the faint stone of idle affordances: unset rating steps, dashed borders, syntax marks |
| `--color-recessed` | `#EFE8D8` | recessed paper: an image's letterbox, a calendar day under the pointer, inline code in the editor |
| `--color-alert` | `#E02B1D` | the Yes of a destructive confirmation, and nothing else |

Contrast on the default grounds:

- `ink`, `ink-body`, and `ink-soft` pass 4.5:1 comfortably on both
  grounds (15.5, 12.0, and 8.7 on paper).
- `stone` does not (2.9:1 on paper, 3.2 on paper-bright). It is only
  for small mono metadata, never for anything a reader must read to use
  the page. A field hint is read, so it is `ink-soft`.
- **`vermilion` is under AA for normal text: 3.99:1 on paper and 4.34:1
  on paper-bright, and paper-bright on a vermilion fill is the same
  4.34.** The handoff calls its colours final and the palette is applied
  as given. It passes the 3:1 large-text bar everywhere, and every use
  of it is either short and tracked capitals (the rating), a link that
  underlines on hover, a link inside prose that keeps a faint underline
  at rest, or the rule and arrow under a primary action, which are not
  text; an action's label is ink. Darkening it to reach 4.5 is a one-token
  change if that is ever wanted; `theme.rs` has a test recording the
  ratio so the number does not drift unnoticed.

### Feed themes

A feed's `basicTheme` gives four colors. `theme.rs` clamps them to
WCAG AA and emits five channels on `:root[data-theme]` as bare integers:

| Channel | Source |
|---|---|
| `--theme-bg`, `--theme-fg`, `--theme-accent`, `--theme-accent-fg` | the author's colors, clamped |
| `--theme-raised` | the ground lifted toward white: seven tenths of the way on a light ground (paper lifts to paper-bright), 6% on a dark one |

The raised surface is derived in Rust because which way it moves
depends on whether the ground is light, which CSS cannot ask. Everything
else is mixed from the channels in the stylesheet: the ink shades are ink
mixed into paper (90% and 80%), stone is 46%, the hairline 14%. A dark
author palette therefore gets light shades and a light hairline
automatically.

The clamp checks ink and the accent against the ground and the raised
surface. Only an author's palette passes through it; the site's own does
not, which is how its vermilion stays as the handoff drew it.

## Type

| Face | Weights | Role |
|---|---|---|
| Evantic Regular | one face, bundled | the logotype `eaten.at`, lowercase, and nothing else |
| Newsreader | 200–800 variable with optical sizes, roman and italic; 500 and 400 used | titles, names, and the running head at 500; text, ledes, hints, notices, comment text, and fields at 400; actions in italic at 400 |
| JetBrains Mono | 400–500 | metadata, kickers, labels, chips, the rating, the document footer, the skip link |

Newsreader's optical size axis follows the font size, so small text gets
the text cut and titles the display cut without anything asking for it.
Sizes are fluid between a 390px and a 1080px viewport.

| Token | Mobile → desktop | Use |
|---|---|---|
| `--text-mono` | 10 → 11 | metadata lines, bylines, tag chips |
| `--text-label` | 10 → 11 | kickers and field labels: tracked capitals |
| `--text-button` | 11 → 12 | the rating, the document footer, the skip link |
| `--text-action` | 16 → 18 | the primary action |
| `--text-action-small` | 15 → 16 | every other action |
| `--text-field` | 16 → 17 | what is typed into a field and what a dropdown shows as chosen |
| `--text-small` | 14 | excerpts, comment text, hints, notices, the address |
| `--text-body` | 14 → 15 | the interface, choice labels, a comment's author |
| `--text-lede` | 16 | ledes, a feed's description |
| `--text-prose` | 16 | a digest's text and the editor's body field |
| `--text-card` | 20 → 22 | row titles, the place name, legends, the author's own nameplate, empty states |
| `--text-heading` | 22 → 24 | h2 inside prose |
| `--text-masthead` | 22 → 26 | the running head |
| `--text-title` | 28 → 34 | the page title (h1) |
| `--text-nameplate` | 32 → 40 | a feed's name on its own front page |
| `--text-logotype` | 40 → 56 | the logotype |

Rules:

- Titles are the serif at 500 with tight leading (1.1–1.15). Hierarchy
  comes from size; nothing is bolder than 500 except `strong` in prose
  (600).
- Text leading is 1.55; prose, excerpts, and comment text 1.6.
- Prose is 16px, the top of the handoff's 14–16 range, in `ink-body`, at
  `--measure` (640px).
- The mono voice is `stone` with `letter-spacing: 0.04em`. Capitals are
  always tracked: kickers and field labels at 0.16em, the skip link at
  0.1em, the rating at 0.14em. A **kicker** (`.kicker`) sits above a
  title or names a section; it is one of the few places capitals appear,
  with labels and the rating.
- Italic serif marks a *description* (a feed's description on its
  nameplate), an *absence* (an empty state), or an *action* (a button).

## Space and shape

An 8px grid: `--space-xs` 4, `-s` 8, `-sm` 12, `-m` 16, `-l` 24, `-xl` 40.
Section spacing is 40→56px (`--section-gap`), sheet padding 16→24px
(`--card-pad`), a row's padding above and below 16→20px (`--row-pad`),
inline gaps 8–12px, and text actions 18px apart (`--action-gap`).

| Token | Mobile → desktop | Use |
|---|---|---|
| `--page-top` / `--page-bottom` | 32/40 → 48/56 | main's vertical padding |
| `--page-inline` | 20 → 32 | side padding |
| `--masthead-top` / `--masthead-bottom` | 24/16 → 36/20 | above and below a running head; `--masthead-top` also above the landing page's wordmark |
| `--title-gap` | 24 → 32 | below a page title, before content |
| `--footer-gap` / `--footer-pad` | 32/16 → 40/24 | above and inside the document footer |
| `--card-gap` | 12 → 16 | between the parts of a row or sheet |

Shape: **square corners everywhere.** Three rules do the separating:

| Rule | Value | Use |
|---|---|---|
| `--rule-ink` | 1px solid ink | above a list of rows, above a section, a blockquote's edge |
| `--rule-hairline` | 1px solid hairline | between rows, around every bright surface |
| `--rule-double` | 3px double ink | under a running head and under a feed's nameplate |

A field draws its rule as an inset shadow rather than a border, the way
an action does, so thickening it moves no text: `--field-rule` (1px ink),
`--field-rule-strong` (2px ink, under the pointer), `--field-rule-focus`
(2px vermilion), `--field-rule-invalid` (1px vermilion), and
`--field-rule-disabled`.

Elevation: none, with one exception. A bright surface has a hairline and
nothing else; a menu or a popover — the only things the site draws over
the page — has a 1px ink rule and `--menu-shadow`, a hard `4px 4px 0`
offset in the ink at a tenth (the Write Pages handoff's value, which
replaced a 3px offset in the hairline on 2026-09-20). It is a printed
edge, not a blur: nothing in the system is lit from above.

Motion: 160ms ease-out (`--ease`) on color, background, border, and an
action's rule. Nothing lifts, scales, or slides, except the primary
action's arrow, which eases 3px to the right on hover (the Typeset
handoff's one movement; its 150ms is taken as the site's 160). The one
animation is Connect (below), which both of the landing page's ways in
are drawn with: pressed, the rule under the button grows in place over
280ms into the rule under the handle field, which arrives selected,
while the label fades out, the form fades in, and the block eases to
the form's height so nothing below jumps. A secondary's rule reddens
and thickens on the way, 1px of ink setting out and 2px of vermilion
landing.
`prefers-reduced-motion` removes the transitions, holds the arrow
still, and cuts Connect straight to the form.

## Components

**Running head** (`.site-header`). The document page alone carries a
masthead: the feed's name (`.site-name.running-head`, the serif at
500 and masthead size), centred, leading to the front page, closed by the
double rule. Every other page opens with its own content — no site chrome
above it.

**Wordmark** (`.wordmark`). The signed-out landing page alone opens with
the **logotype** (`.site-name.logotype`, Evantic at logotype size, leading
home), centred at the top of the page — it is the one page that has to say
what the site is. It is not a masthead bar: no tagline, no rule, and
nothing else beside it. Evantic sets the logotype
and nothing else, which is why a feed's name is the serif instead.

**Nameplate** (`.nameplate`). A feed's front page opens with its
own masthead in place of any site chrome: the name at `--text-nameplate`, centred; a mono
**dateline** (`.dateline`) of author, site, and rss separated by middle
dots; the description as an italic lede; the tag chips. The double rule
closes it.

**Page head** (`.page-head`). Every other page opens with an optional
kicker, the h1, and an optional lede (`.lede`, `ink-soft` serif at 16px)
or note (`.meta`). The document page's kicker is its date; the tag page's
is "Tag"; a status page's is "Error 404"; the chooser's is
"Feeds". The one exception is the editor for a new digest,
which has no heading until a place is chosen; editing an existing one
opens with an "Edit" kicker and the digest's title.

**Rows** (`.listing`, `.chooser`, `.photo-manage`). A column
of things to pick between, an ink rule above the first and a hairline
between each pair, padded above and below, on the paper. Nothing is
boxed and nothing changes on hover but the link.

**Listing row** (`.listing-item`). Rows in two variants that share one
text block: the **head** (`.listing-head`), the title in the serif at
row-title size on the left and the rating on the right, wrapping under it
when the line is short; the **metadata line** (`.listing-meta`), the date
and the place name in the mono voice separated by a middle dot (the name
is left out when it is the title, which it is by default); and the
excerpt in the serif at small size in `ink-body`. It is the document page
in miniature.

- *With photos* (`.has-photos`): the first photo leads (`.listing-photo`,
  the `card` rendition, cropped to 3:2, never upscaled). Under 40rem it
  spans the top of the row; from 40rem it fills the left 42%, at least
  3:2 of its width and as tall as the text. More than one photo puts a
  small mono chip at the photo's bottom-right corner on a translucent
  bright ground, "+3 photos" (`.listing-photo-count`). The photo links
  where the title does and is not a second tab stop; its `alt` is the
  photo's own, empty when empty.
- *Without photos* (`.no-photos`): the text block alone. Nothing stands
  in for a photo: a text-only listing reads as a plain journal, which is
  what it is.

**Chooser row** (`.chooser-item`). Name in the serif at row-title size,
description in `ink-body`, the feed's URL in the mono voice.
Settings reuses the same row, one of them, with its form inside.

**Photo grid** (`section.photos`, `.photo-grid`). Between the visit and
the prose: the visit's photos as square 400px thumbnails in an
auto-filling grid (8.5rem columns, 8px gaps, a hairline), each a link to
its full rendition. Nothing for a visit without photos.

**Visit** (`.visit-card`). The one boxed thing on a page: a bright sheet
with a hairline between the page title and the prose, a fact box beside
the text. The place name (`.place-name`) in the serif at 500; under it, in
the mono voice, the visit date, the meal, and the price band as dollar
signs, separated by middle dots (`.visit-meta`); then the address in the
serif at small size (`.place-address`); then the rating.

**Rating** (`.rating`). The verdict as its word alone, in tracked mono
capitals in vermilion: SOLID, RECOMMENDED, STRONGLY RECOMMENDED, CAN'T
MISS (D4's scale, D46's rendering). No marks, stars, dots, or numbers. The
word is written in the HTML as a word ("Recommended") and the stylesheet
sets the capitals, so it reads naturally aloud. An unrated visit shows
nothing. The editor's rating radios carry the same words.

**Prose** (`.prose`). The digest, at measure and at prose size, in
`ink-body`. Headings are the serif at 500 in `ink`. Links keep a faint
underline at rest, in the accent at 40%, because colour alone does not
mark a link inside a paragraph; this is the one place a link is underlined
before hover.

**Comments** (`section.comments`). Between the prose and the footer, an
ink rule above: a "Comments on Bluesky" kicker, then the replies as
hairlined rows. Each row is a byline over the text:

- the author's name in the serif at 500, in `ink`
- the `@handle` and the date (linked to the reply) in the mono voice
- the text in the serif at small size, in `ink-body`

Replies to replies are indented up to three levels. "No comments yet."
is the empty state, and "Reply on Bluesky →" is in the accent.

**Document footer** (`.doc-footer`). Mono, hairline above, actions 18px
apart. Row one: the "Links" label (stone) and links (accent), with the
comments link pushed right with an arrow. Row two: tag chips, then edit,
rss, and the author pushed right as quiet links in `ink-soft`.

**Tag chip** (`.tag`). A small square badge: a hairline, the mono voice
at metadata size, `ink-soft`, the tag as the author wrote it. On hover
the border and text turn `ink`. The same chip on the front page, the
author's home, and in the document footer.

**Buttons.** Typeset: an action is a label in the serif italic on a
rule, with no box and no fill (the Typeset handoff, 6a). The rule is an
inset shadow, not a border, so thickening it on hover moves no text.
Pressing turns the label vermilion. Disabled, label and rule go
`--color-disabled` and nothing happens on hover.
- **Primary** (`button`, `.button`): `--text-action`, `ink`, on a 2px
  vermilion rule, followed by a vermilion arrow the stylesheet adds with
  empty alternative text, so no primary is without one and none reads it
  aloud. At most one per page: it is the page's one action. On hover the
  rule goes to 3px and the arrow eases 3px right.
- **Secondary** (`.button-secondary`, `.button-link`, `.pagination a`,
  and a `.link-button` placed directly in an `.actions` row):
  `--text-action-small`, `ink`, on a 1px ink rule, no arrow. On hover the
  rule goes to 2px. Any second action is a secondary, with a leading
  arrow in its text when it goes back.
- **Quiet** (`.button-quiet`): a secondary in `ink-soft` on a hairline
  rule, for an action a page wants to offer without weight. Defined by
  the handoff; no page uses it yet.
- **Touch targets:** below 40rem every action carries an invisible halo
  that makes its tap 44px tall without moving the rule from the label.

Actions sit in an `.actions` row on a shared baseline, 24px apart. An
action beside a field (`.lookup-row`) shares the field's baseline, 16px
from it.

**Link button** (`.link-button`). A button dressed as an inline link, for
a POST that sits in a sentence (sign out, restore a draft, remove a row).

**Field** — "bare rule" (`input`, `textarea`). Not a box: what is typed
sits on the paper at `--text-field` in the serif, with nothing behind it
and 1px of ink under it, 6px below the text. Under the pointer the rule
goes to 2px of ink; with focus it goes to 2px of vermilion, and that is
the whole selected state — a field takes no ring, because its own rule is
the ring. Invalid, the rule and the message go vermilion, and focus still
doubles it. Disabled, the text and the rule go `--color-disabled`. The
placeholder is the serif in *italic* in `stone`: a placeholder is an
absence, and italic is how this system sets an absence. The size never
drops below 16px, so mobile browsers do not zoom on focus. A field's
**label** is a UI label: tracked mono capitals — in `ink-soft`, not the
handoff's `stone`, because a label has to be read (see Palette).

**Return mark** (`.return-rule` around an `input`). A field sent with
Return rather than a button says so at the end of its own rule: the key's
arrow at 0.9em in `stone`, an SVG mask (`--return-mark`) drawn to the
chevron's weight, faint because it is a note on the field and not its
value. The field is padded by the mark's width so what is typed never
runs under it, and the mark takes no pointer events — it is a label, not
a button. Only the handle fields a Connect block reveals wear it; every
other field keeps its button.

**Dropdown** (`.select-rule` around a `select`). A field whose answer is
chosen rather than typed, so the handoff sets the chosen value the way it
sets an action: the serif in *italic* on the same bare rule, with a
vermilion chevron at the rule's end. The chevron is an SVG mask filled
with `--color-vermilion`, so it follows a feed's accent; the
wrapper exists only to give it somewhere to sit. **The menu a native
`select` opens belongs to the browser and cannot be styled** — the
handoff's paper menu is drawn where the site draws its own list, the
combobox below. Choosing without script is worth more than a matching
popup (D3).

**Choice** (`.choice`). A checkbox or a radio and its label on one line,
the row clickable because the row *is* the label: the serif at 16px in
`ink`, 8px from the box. The **box** is 16px square (`--box-size`), 1px
of ink, empty on the paper; checked, it takes `paper-bright` and a
vermilion pen-stroke with a short tail in and a long tail out
(`--check-mark`, the handoff's path, drawn as an SVG because a font glyph
does not centre). A **radio** is the same box drawn round with a
vermilion dot instead of the stroke: the handoff draws only the checkbox,
and a browser-default radio beside a hand-drawn check would read as two
systems. Disabled, the box and the label go `--color-disabled`.
`--check-mark` is a data URL and so carries the vermilion literal — the
one colour in the stylesheet a feed theme does not reach. Nothing
themed carries a form, so it is never seen; the radio's dot, which is a
gradient, follows the theme as everything else does.

**Live find** (`.find`, `.find-results`). On the author's home, the
results under the find form are swapped in after a pause in typing,
without a reload; the address bar follows, and the form still submits
as a form. The results region is `aria-live`.

**Combobox** (`.combobox-list`, `.combobox-option`). A listbox an island
puts under a text field, and the only menu the site draws itself, so it
is the handoff's **paper menu**: `paper-bright` inside a 1px ink rule,
square, carried off the page by `--menu-shadow`. One option per row,
hairlined apart, the name in the serif and a detail (the handle) in the
mono voice; the option under the pointer or the arrow key takes the paper
and goes vermilion. The handoff sets a menu's items in italic, as
actions; a suggestion is a name, not an action, so these stay roman. It
appears only with JavaScript on and only while there are matches; the
field it sits under works without it. Return never swallows the send: it
takes the highlighted row when there is one and what was typed when there
is not, then submits the form either way.

**Filed under** (`.tags-sentence`, `.tag-field`, `.chip`). The editor's
tags as a sentence with a blank in it: "Filed under" in italic soft
ink, the blank an inline field on a hairline holding the comma list.
With script, an island files what is typed as chips before the blank on
Return or a comma, each a word on a hairline, commas between, vermilion
and struck through under the pointer, that takes itself out. The list
submits as the text it always was.

**Notice** (`.notice`). A bright sheet with a hairline, the serif at
small size in `ink-soft`. Its links are the accent like any other.

**Form error** (`.form-error`). A bright sheet with a hairline and a 3px
vermilion rule on the leading edge, `ink` text.

**Empty state** (`.empty`). One italic serif line in `ink-soft`, centred,
with room around it, where the content would have been.

**Pagination** (`.pagination`). Two secondary actions: "← Newest" left
and "Older →" right.

## Page recipes

| Page | Above the content | Opens with | Then |
|---|---|---|---|
| Landing `/`, signed out | wordmark | page head: h1 pitch, lede | connect: one primary "Connect to start writing", which becomes the sign-in form in place (that field carries the return mark and no button); a hairline; a second connect, drawn the same but secondary — one line in the lede's voice over a "Look up a friend" button that becomes the field for someone else's handle; both fields suggest handles as you type |
| Landing `/`, signed in | — | page head: the handle in the mono voice where a kicker goes, h1 "Where did you eat?" | one primary "Write a new digest"; "Your feed": a small nameplate (name linked to the front page, address and rss in the mono voice), the find form with a secondary "Find" button and the tag chips under it, a "Recent digests" (or "Matching “q”" with a secondary "Clear") kicker over the listing rows, "All digests →" when there are more; or, with no feed yet, one lede saying what it will be; then a hairline and one quiet line, Settings · About · Sign out |
| Feed front page | — | nameplate | listing, notice if truncated, pagination |
| About `/about` | — | page head: "About" kicker, h1 "About eaten.at", lede | prose: one paragraph on the protocol a digest lives on, then the credits — the place licences and the IP database, and nothing that is not asked for. The page is static |
| Tag page | — | page head: "Tag" kicker, h1 "Tagged “x”", scope note | listing, pagination |
| Document | running head | kicker date, h1 title | the visit, photos, prose, comments (when the document names a Bluesky post), footer |
| Chooser (`/at/{did}/`) | — | page head: "Feeds" kicker, h1 author, lede | chooser rows |
| No feeds | — | page head: "Feeds" kicker, h1 author | empty state |
| Interstitial | — | page head: "Content warning" kicker, h1 | the labels, note, actions |
| Lookup `/lookup` | — | page head: "Lookup" kicker, h1 ("Whose digests?", or "That doesn't look like a handle") | the form, with its error when there is one |
| Sign in | — | page head: "Sign in" kicker, h1, lede | handle form (the lookup form's shape) with "Start typing your handle…"; suggestions from Bluesky as you type; errors in place |
| Signing in | — | page head: "Signing in" kicker, h1 "Continuing to host" | one primary button; the page refreshes itself onward |
| Sign-in failed | — | page head: "Sign in" kicker, h1 | one line, secondary "← Try again" |
| Status page | — | page head: "Error nnn" kicker, h1 | detail, secondary "← Back to the start" |
| Editor `/write`, choosing | — | nothing: the screen is the input | the place's name as a 32px headline field ("St. John Bread and Wine" standing in), suggestions opening under it as it is typed; the line `at [address].`; one primary "Start writing", shown once there is a name |
| Editor `/write`, editing | — | nothing: the title is the heading | the title as the headline field (the place's name standing in), the line `at [name,] [address] — somewhere else`, the name shown there once the title is something else and the shrug the way to change the restaurant; `for [a meal] on [date]` under the place line, lowercase like `at`; then the digest's text between two faint rules; the teaser, folded; the rating (a clear box, four pluses, the word) left and the meal and price words right; the Photos kicker over the tiles; `Bluesky: …` left and `Elsewhere: …` right; one primary "Save changes" (or "Publish") and, for a record, Delete with its Yes / No in the same slot |
| Photos `/write/{rkey}/photos` | — | page head: "Photos" kicker, h1 "Photos of {place}", lede | the photos as rows (thumbnail, alt text field, "Move up", "Move down", "Remove" link buttons), or "No photos yet."; the file input with its hint about re-encoding; one primary "Add photos", secondary "Save alt text", and "← Back to the digest". The way photos are managed without script; the editor manages them in place otherwise |
| Delete `/write/{rkey}/delete` | — | page head: "Delete" kicker, h1 "Delete “title”?", lede saying what happens | a ticked choice "Also delete the Bluesky post" when there is one to delete (a note when this sign-in may not), one primary button, secondary "← Keep it" |
| Crosspost `/write/{rkey}/crosspost` | — | page head: "Bluesky" kicker, h1 "Post “title” to Bluesky" (or "… is on Bluesky"), lede | the post text field and one primary "Post to Bluesky"; or, before permission, one primary "Allow posting and continue"; secondary "← Skip for now" either way; posted: the thread link and a secondary way back |
| Settings `/settings` | — | page head: "Settings" kicker, h1 "Your feed", lede | one chooser row: the name (linked to the front page) and the current address in the metadata voice, or, before there is one, the name it would get and "Made when you save"; then the form: name, description, the two radio choices for where it lives, one primary "Save" ("Create it" the first time) |

### The editor

The editor is the Write Pages handoff (`docs/design-handoff/write-pages/`),
applied exactly: a 544px column (`.editor`, `34rem`) of prose at 17px on
1.55, read top to bottom, in which every editable value is an **inline
field**. Both screens are one `<form>` that works without script; the
islands dress it and fall away.

- **Inline field** (`.inline-field input`, `.headline`). The signature
  control: transparent, no rule of its own, the sentence's font, as
  wide as its text (`field-sizing: content`, measured against a mirror
  where that is not understood), underlined 1px at `0.14em`: hairline
  idle, ink under the pointer, vermilion with focus. Its placeholder is
  the serif in italic in stone. The **headline** is the same field at
  32px/500 with a transparent underline idle; its placeholder is a real
  value standing in (the place's name as the title, "St. John Bread and Wine" on the
  choosing screen), so it is roman, and it dims to `--color-disabled`
  when the field takes focus.
- **Prose connectives** (`.soft`) — "at", "From a visit on", "Filed
  under", "Bluesky:", "Elsewhere:" — are italic soft ink.
- **Buttons as prose.** The primary action is the site's typeset
  primary (Start writing, Save changes, Publish). A **hidden
  affordance** (`.hint-action`: add a link, write your own teaser, save
  it, never mind) is italic 13–14px stone on a hairline, darkening to
  soft ink. A **mark** (`.mark`: the title's reset ×, the change-place
  map) is a vermilion stroke icon at 0.7 opacity, 1 under the pointer,
  revealed only while its row is hovered (always shown where there is
  no pointer). A **word on a hairline** (the date, the meal, the price,
  a link's label, a tag) goes vermilion under the pointer; a tag also
  strikes itself through.
- **Popovers** (`.popover.paper`) are the site's paper menu: the bright
  surface, a 1px ink rule, `--menu-shadow`, square. One is open at a
  time; an outside click or Escape closes it.

**Choosing** (`.editor-choosing`). The place's name as the headline,
`at [address].` under it, "Start writing" 48px below, and nothing
else: the Overture and DB-IP credit the earlier choosing page carried
(plan 12) is gone with it, and is given on the about page instead. Suggestions (the combobox, positioned under the headline) open
while the name has three characters and matches; ↓/↑ cycle, Return takes
the highlighted row or leaves the field, Escape closes. A pick fills
both lines and arms Start writing as that pick, which the server reads
from the same cached search (`action=pick:N`); typing over either line
makes it a place by hand again (`action=manual`). Clearing the name
clears the address with it. Without script the two lines are typed and
Start writing takes them as written; there is no Search button and no
results page any more.

**Editing** (`.editor-write`). Top to bottom, with the handoff's gaps:

- **Title and place.** The title field's placeholder is the place's
  name (D29). While it stands in, the line under is `at [address].` and
  the change-place mark sits after the title; once the author writes a
  title, the place's name joins the line (`at [name], [address].`), the
  reset × appears after the title, and the mark moves down beside the
  address. The server renders whichever state the form is in; the
  island follows as the title is typed.
- **Date and tags** on one two-ended row. `From a visit on [date].` is
  the date input, which the island hides behind a word ("September 6";
  the year appended when it is not this year) over a 266px calendar:
  arrows and the month in italic, day initials in italic stone, 34×32
  cells, the chosen day ink on paper, today underlined in vermilion,
  recessed paper under the pointer. `Filed under [tags].` is right-set
  (see **Filed under**).
- **Digest.** The kicker, then the digest's text on the bright surface
  inside an ink rule (vermilion while active), padded 16px 18px, at
  least 384px tall. The textarea is the carrier; the island draws a
  live markdown editor over it (`.digest-editor`): each source line a
  block, formatted (h1 25px/500, h2 20px/500, h3 600; bullets as
  vermilion `•`; quotes on a 2px hairline in italic soft ink; bold,
  italic, `code` in the mono on recessed paper, links in vermilion),
  and the caret's line showing its raw markdown with the syntax marks
  in `--color-disabled`. Return splits a line (a list or quote prefix
  continues; Return on an empty one ends it), Backspace at the start
  merges up, ↑/↓ cross lines at their edges, Escape leaves, paste is
  plain text, Cmd/Ctrl-Z undoes. Anything the island does not draw is
  left as written for the server to render.
- **Teaser.** Folded, one italic stone line: "In listings, the piece
  opens with its first lines — or *write your own teaser*." Open: "In
  listings it opens:", a two-row transparent textarea whose placeholder
  is the first lines in quotes, and a note: "Drawn from the first lines
  — type to say it differently, or *leave it be*." while it is empty,
  "*never mind — use the first lines*" once written. The fold is a
  `<details>`, so it opens without script; it is open whenever there
  is a teaser.
- **Rating and notes** on one row. The rating (`.rating-control`) is a
  22px clear box with a stone ×, four `+` in the mono at 26px (filled
  vermilion up to the value, else `--color-disabled`, previewing under
  the pointer), and the verdict's word in tracked mono capitals
  (vermilion when rated, stone for UNRATED). The radios are the
  carrier and the stylesheet does the rest, so this needs no script:
  the steps are set in reverse so a sibling selector can fill the lower
  ones from the checked one. The **meal** ("a meal" in stone unset;
  the lexicon's Breakfast, Brunch, Lunch, Dinner, Late night) and the
  **price** (`$?` unset; `$` to `$$$$`) are selects the island redraws
  as words over small right-aligned paper menus (options at 15px on
  1.9, the chosen one ink and underlined, an italic "no note" to
  clear).
- **Photos.** The kicker, then four square tiles to a row with 10px
  gaps, the first wearing a COVER badge (mono 9px caps, ink on paper),
  a remove mark revealed on the tile, and an add tile (a dashed box
  with a `+`) last; or, with none, one dashed box: "Nothing to look at
  yet." that reads "add a photo" in vermilion under the pointer. Under
  the grid: "Drag to reorder — the first photo is the cover." A tile
  opens its detail over a scrim (the ink at 35%): the photo letterboxed
  on recessed paper, a centred caption field, "done" and "remove this
  photo". The same before and after the digest exists: a picked file
  is uploaded to the author's repository at once (`/write/upload`) and
  comes back as a blob reference, the list rides in the form as hidden
  fields like every other value, and Publish or Save writes it with
  the record (D37 amended). A tile is drawn from the author's own blob
  (`/write/photo/{cid}`), so it shows before any record lists it.
  Without script the tiles link to the photos page once there is a
  record, which stays the way photos are managed by hand.
- **Links** on one two-ended row. `Bluesky: see the thread.` once
  there is a post; before one, `Bluesky: [ ] post it too, saying
  [text].` `Elsewhere:` the place's links as words with commas ("nowhere
  yet" in stone for none) and "— add a link", shown while the row is
  hovered (always, when empty). Each link is a **card** (`.link-card`:
  paper, up to 420px, right-set) of two prose rows, `shown as [label]`
  and `pointing at [url]` with a chain-link mark to open it, and a foot
  of "save it · never mind" and "remove it". With script one card is
  open at a time and a word opens its own; Return saves, Escape cancels,
  saving without a URL cancels, an empty label falls back to the host.
  Without script every row's card is open, "add a link" and "remove it"
  are the server's row actions, and blank rows are skipped.
- **Actions.** "Save changes" (or "Publish" for a new digest), then
  **Delete**, which confirms in its own slot: pressed, it reads
  "Delete?" in italic soft ink followed by **Yes** and **No** (italic
  17px, letter-spaced 0.06em, a 1px rule hugging the text). Yes is
  `--color-alert`, the one place that red appears, and posts to the
  delete route with the Bluesky post's deletion asked for as the delete
  page asks it; No closes. The slot is a `<details>`, so it opens
  without script and "Delete?" closes it.
- **Problems** are `.field-error` lines under the line they belong to,
  and the form-error summary sits above the title. Return in any text
  field only re-renders the form (`action=keep`); nothing is sent by
  accident.
- **Drafts:** with JavaScript on, a draft kept on the device is offered
  back in a `.notice.restore` banner at the top of the form. It is one
  line and two link buttons, "Restore it" and "Discard it".

Under 36rem the two-ended rows stack, both ends at the left, and the
tiles go three to a row.

Signed in, the landing page is the author's home (plan 11). Its last
line, under a hairline, is the only place settings and sign-out appear:
"Settings · About · Sign out" in the metadata voice, the last a link button.
Signed out there is no account line: the page's primary action is
"Connect to start writing" (plan 09), which becomes the sign-in form in
place. The action carries its own reason — the phrase is the button, not
an aside beside it — so the line reads as one. Under the hairline,
reading is offered the same way and secondary, with one line above it
where the button alone could not carry the tone. Under a second hairline,
signed out and signed in alike, one quiet line leads to the about page,
which is where the credits the data and the fonts ask for are given.

## Accessibility and constraints

- Landmarks on every page: skip link, `header` where a page names
  something above its content, and `main#main`;
  `nav` elements carry an `aria-label` naming their scope ("Tags in this
  feed", "Links", "Pagination").
- Focus is a 2px ring of the accent at 55%, offset 3px, on every
  focusable thing but a field, which shows focus by doubling its own rule
  in vermilion. The handoff asks for the ring at full strength and 2px
  offset; it was quieted on 2026-09-15 and stays that way.
- Contrast: see the palette notes, including the vermilion shortfall.
  Feed themes are clamped to WCAG AA (`theme::MIN_CONTRAST`) on
  every surface their text sits on.
- `prefers-reduced-motion` disables the transitions, holds the primary
  arrow still, and cuts the one animation, Connect, to a plain swap.
- Newsreader and JetBrains Mono are subset to Latin and Latin Extended;
  Evantic is one file. All use `font-display: swap` and fall back to
  Georgia and the system mono. The licences are in
  `static/fonts/OFL.txt`; Evantic's terms, a personal-use licence the
  handoff judged fine for a non-commercial site, are noted in
  `static/fonts/EVANTIC.txt`.
- No page makes a third-party request and there are no iframes, except
  that the sign-in and landing pages call the Bluesky AppView for handle
  suggestions and their policy alone allows that origin (D42). Masthead's
  handoff names Google Fonts; the same files are served from `/static/`
  instead.
- Nothing scrolls horizontally at a 390px viewport.

## Do and don't

- Do put a date, a handle, a DID, or a URL in the mono voice. Don't set it
  in the serif at body size.
- Do make a thing a reader chooses between a row under a rule. Don't box
  it, and don't box a section of a page.
- Do let a field be a rule on the paper. Don't box it, fill it, or round
  it, and don't put a ring around it when it takes focus.
- Do use vermilion for a link, the primary action's rule and arrow, the
  rating, or an error. Don't use it for a heading, a fill, a border, a
  background, or an action's label.
- Do use size for hierarchy in the serif. Don't go bolder than 500.
- Do track capitals. Don't set capitals in the serif, or untracked.
- Do use Evantic for `eaten.at`, lowercase. Don't use it for anything
  else, and don't set the logotype in any other face.
- Do let a page be short. Don't fill an empty state with instructions.
- Do keep the column at 720px. Don't add a wide variant for a listing.

## What the handoff asks for and the site does not

Everything in the v2 handoff is applied except the following, each of
which is a decision this project already took and has not reversed.

| The handoff | The site | Why |
|---|---|---|
| A masthead bar: the logotype over a tracked mono tagline, closed by the double rule | No site chrome above a page; the signed-out landing page alone carries the logotype, with no tagline and no bar, and a document carries its feed's name | 2026-09-16: a reader on someone's write-up should see that publication, not this site |
| Ratings as POOR / FAIR / GOOD / GREAT / SUPERB | Solid / Recommended / Strongly Recommended / Can't Miss | D4: the scale is the record's, not the stylesheet's. The *rendering* — the word alone, tracked mono capitals in vermilion — is the handoff's, exactly |
| A feed row's metadata as `@handle · cuisine · $$` | The date and the place name | The listing is one author's feed, so the handle is not news on it; the price band is a fact of the visit and sits in the visit's fact box |
| Action row counters `♥ 84 ⇄ 6` in the mono voice | Nothing | D5 and D6: there are no likes or reposts here, and comments live on Bluesky |
| A field's label in `stone` | `ink-soft` | `stone` is 2.9:1 on paper. A label has to be read; `stone` is for metadata that is scanned |
| Write Pages: meal options Breakfast, Lunch, Dinner, A snack, Drinks | The lexicon's Breakfast, Brunch, Lunch, Dinner, Late night | D3/D31: the choices are the record's `knownValues`, not the stylesheet's; the handoff's list was a specimen |
| Write Pages: the editor rendered as a 784px card on a desk | The page itself, in the site's shell | The shell's 720px column and 48/32/56px padding are the card's measurements already; a second ground would make the editor the one page that is not on the paper |
| Write Pages: photo detail scrim over the card | Over the viewport | The card is the page |
| Focus-visible: a 2px vermilion outline at 2px offset | The accent at 55%, offset 3px | 2026-09-15 |
| Links with no underline at rest | A link inside prose keeps a faint underline | Colour alone does not mark a link in a paragraph |
| Multi-select filter chips, typeset, as an alternative for tag filters | The tag chip stays the small mono badge it is | The handoff offers the chip as an alternative, and nothing here filters by several tags at once: a tag chip is a link to a tag page |
| Google Fonts for Newsreader and JetBrains Mono | The same faces, subset and served from `/static/` | The CSP keeps every request same-origin |

## Composer trial — 2026-09-22

The `composer-refactor` trial amends the editor description above. The headline
is the title with the restaurant's name standing in, as before; the place is the
line under it, ending in “somewhere else”, which opens the chooser. Retitling and
changing the restaurant were one headline doing two jobs; now they are two lines
(C6, settled 2026-09-25). The chooser
uses St. John Bread and Wine, at 94–96 Commercial Street, as its example: a real
place whose name says food on its own (settled 2026-09-25). There is no DIGEST
kicker: the whole post is the digest, so the `For [a meal] on [date].` line is the
digest's head, on the faint rule the kicker used to sit on (settled 2026-09-25).
Nothing on the page explains markdown. The prompt in the empty digest is written in
it, with one bold word between faint asterisks, drawn as the editor draws the line
the caret is on, so the marks are seen in the place they are typed. A Formatting
disclosure, and then a pilcrow that disclosed the marks, were both tried and cut:
the prompt is enough (Ken, 2026-09-25).

Below that rule, the title says only “Title” when empty. It has no full-width rule
or visible optional-text hint; hover/focus underlines its text. The body beneath
it is transparent on the page’s paper. Its italic stone placeholder asks “What
did you eat? Was it good? What else happened?” A faint rule closes the
writing section and takes the accent on focus. Inactive lines remain formatted
even when the whole digest is selected.

Filed under sits below photos, full-width and left-aligned. Photos have no
kicker; the empty target says Add photos at rest. Elsewhere and its link cards
align left and wrap. Gaps below the digest are 16–24px. The composer has no Cancel
or Bluesky controls. The calendar, teaser, price, link editor, and inline Delete
confirmation remain. Snack joins the meal options; Late night is no longer offered (a record that has it still reads).

The composing photo lightbox grows to 960px when space allows. Its image height
adapts to the viewport, preserving room for the caption and actions; the panel
can scroll in very short windows. Responsive outside gutters keep it inset on
phones. Its hard 6px drop shadow is retained unchanged.

The composer’s date sentence now reads “For [a meal] on [date].” The meal
selector sits inline before the date, with lowercase names and “a snack”;
clearing it restores “a meal.” Price remains beside the rating below the digest.

The digest body starts at six lines (9.6em at its 1.6 line height, six rows for
the plain textarea), growing with the writing. This prioritizes writing room
over fitting the entire composer with photos above the fold on smaller laptops.

The restaurant headline is an underlined submit control opening the chooser;
the address is plain supporting text. Both remain editable in the chooser.
Photo text uses one “Describe this photo” field, stored as alt text. Digest
photo links show it once as a visible caption; listing images use it as alt text.

The post-publication trial puts a compact confirmation above the digest,
separated by a double rule. Published and saved have distinct messages; Copy
link, Share on Bluesky, and Dismiss form one wrapping row. Copy feedback is
announced inline, and a plain permalink remains available without JavaScript
or when clipboard access fails. Ordinary readers see the existing digest.
